use std::io::{self, Write};

use crate::{
    record::{cigar::op::Kind, Cigar},
    writer::write_int,
};

pub fn write_cigar<W>(writer: &mut W, cigar: &Cigar) -> io::Result<()>
where
    W: Write,
{
    use super::MISSING;

    if cigar.is_empty() {
        writer.write_all(&[MISSING])?;
    } else {
        for op in cigar.iter() {
            write_int(writer, op.len())?;

            let c = match op.kind() {
                Kind::Match => b'M',
                Kind::Insertion => b'I',
                Kind::Deletion => b'D',
                Kind::Skip => b'N',
                Kind::SoftClip => b'S',
                Kind::HardClip => b'H',
                Kind::Pad => b'P',
                Kind::SequenceMatch => b'=',
                Kind::SequenceMismatch => b'X',
            };

            writer.write_all(&[c])?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_cigar() -> io::Result<()> {
        use crate::record::cigar::Op;

        let mut buf = Vec::new();
        write_cigar(&mut buf, &Cigar::default())?;
        assert_eq!(buf, b"*");

        buf.clear();
        let cigar = Cigar::from(vec![Op::new(Kind::Match, 8)]);
        write_cigar(&mut buf, &cigar)?;
        assert_eq!(buf, b"8M");

        Ok(())
    }
}
