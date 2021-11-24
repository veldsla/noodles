use std::io::{self, Write};

use byteorder::{LittleEndian, WriteBytesExt};

use crate::{validate, Record};

// § 4.2.3 SEQ and QUAL encoding (2021-06-03)
const NULL_QUALITY_SCORE: u8 = 255;

pub(super) fn write_record<W>(writer: &mut W, record: &Record) -> io::Result<()>
where
    W: Write,
{
    validate(record)?;

    let block_size = u32::try_from(record.block_size())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    writer.write_u32::<LittleEndian>(block_size)?;

    writer.write_i32::<LittleEndian>(record.ref_id)?;
    writer.write_i32::<LittleEndian>(record.pos)?;

    let l_read_name = u8::try_from(record.read_name.len())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    writer.write_u8(l_read_name)?;

    let mapq = u8::from(record.mapping_quality());
    writer.write_u8(mapq)?;

    writer.write_u16::<LittleEndian>(record.bin())?;

    let n_cigar_op = u16::try_from(record.cigar().len())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    writer.write_u16::<LittleEndian>(n_cigar_op)?;

    let flag = u16::from(record.flags());
    writer.write_u16::<LittleEndian>(flag)?;

    let l_seq = u32::try_from(record.sequence().len())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    writer.write_u32::<LittleEndian>(l_seq)?;

    writer.write_i32::<LittleEndian>(record.next_ref_id)?;
    writer.write_i32::<LittleEndian>(record.next_pos)?;

    writer.write_i32::<LittleEndian>(record.template_length())?;

    writer.write_all(&record.read_name)?;

    for &raw_op in record.cigar().as_ref().iter() {
        writer.write_u32::<LittleEndian>(raw_op)?;
    }

    writer.write_all(record.sequence().as_ref())?;
    if record.quality_scores().is_empty() {
        for _ in 0..l_seq {
            writer.write_u8(NULL_QUALITY_SCORE)?;
        }
    } else {
        writer.write_all(record.quality_scores().as_ref())?;
    }

    writer.write_all(record.data().as_ref())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use noodles_sam as sam;

    use super::*;

    #[test]
    fn test_write_record_with_default_fields() -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = Vec::new();
        let record = Record::default();
        write_record(&mut buf, &record)?;

        let expected = [
            0x22, 0x00, 0x00, 0x00, // block_size = 34
            0xff, 0xff, 0xff, 0xff, // ref_id = -1
            0xff, 0xff, 0xff, 0xff, // pos = -1
            0x02, // l_read_name = 2
            0xff, // mapq = 255
            0x48, 0x12, // bin = 4680
            0x00, 0x00, // n_cigar_op = 0
            0x04, 0x00, // flag = 4
            0x00, 0x00, 0x00, 0x00, // l_seq = 0
            0xff, 0xff, 0xff, 0xff, // next_ref_id = -1
            0xff, 0xff, 0xff, 0xff, // next_pos = -1
            0x00, 0x00, 0x00, 0x00, // tlen = 0
            0x2a, 0x00, // read_name = "*\x00"
        ];

        assert_eq!(buf, expected);

        Ok(())
    }

    #[test]
    fn test_write_record_with_all_fields() -> Result<(), Box<dyn std::error::Error>> {
        use sam::record::{
            cigar::op::Kind, data::field::Tag, quality_scores::Score, Flags, MappingQuality,
        };

        use crate::record::{
            cigar::Op,
            data::{field::Value, Field},
            sequence::Base,
        };

        let mut record = Record::default();

        record.ref_id = 1;
        record.pos = 8; // 0-based
        *record.mapping_quality_mut() = MappingQuality::from(13);
        *record.bin_mut() = 6765;
        *record.flags_mut() = Flags::PAIRED | Flags::READ_1;
        record.next_ref_id = 1;
        record.next_pos = 21; // 0-based
        *record.template_length_mut() = 144;

        record.read_name.clear();
        record.read_name.extend_from_slice(b"r0\x00");

        record.cigar_mut().push(Op::new(Kind::Match, 3)?);
        record.cigar_mut().push(Op::new(Kind::SoftClip, 1)?);

        record.sequence_mut().push(Base::A);
        record.sequence_mut().push(Base::C);
        record.sequence_mut().push(Base::G);

        record.quality_scores_mut().push(Score::try_from('N')?);
        record.quality_scores_mut().push(Score::try_from('D')?);
        record.quality_scores_mut().push(Score::try_from('L')?);

        record
            .data_mut()
            .insert(Field::new(Tag::AlignmentHitCount, Value::UInt8(1)));

        //trigger cigar length error
        let mut buf = Vec::new();
        assert!(write_record(&mut buf, &record).is_err());
        buf.clear();

        //fix sequence
        record.sequence_mut().push(Base::T);

        //trigger remaining quality length error
        let mut buf = Vec::new();
        assert!(write_record(&mut buf, &record).is_err());
        buf.clear();

        //fix quality
        record.quality_scores_mut().push(Score::try_from('S')?);

        write_record(&mut buf, &record)?;

        let expected = [
            0x35, 0x00, 0x00, 0x00, // block_size = 53
            0x01, 0x00, 0x00, 0x00, // ref_id = 1
            0x08, 0x00, 0x00, 0x00, // pos = 8
            0x03, // l_read_name = 3
            0x0d, // mapq = 13
            0x6d, 0x1a, // bin = 6765
            0x02, 0x00, // n_cigar_op = 2
            0x41, 0x00, // flag = 65
            0x04, 0x00, 0x00, 0x00, // l_seq = 4
            0x01, 0x00, 0x00, 0x00, // next_ref_id = 1
            0x15, 0x00, 0x00, 0x00, // next_pos = 21
            0x90, 0x00, 0x00, 0x00, // tlen = 144
            b'r', b'0', 0x00, // read_name = "r0\x00"
            0x30, 0x00, 0x00, 0x00, // cigar[0] = 3M
            0x14, 0x00, 0x00, 0x00, // cigar[1] = 1S
            0x12, 0x48, // seq = ACGT
            0x2d, 0x23, 0x2b, 0x32, // qual = NDLS
            b'N', b'H', b'C', 0x01, // data[0] = NH:i:1
        ];

        assert_eq!(buf, expected);

        Ok(())
    }
}
