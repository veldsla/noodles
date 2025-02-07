//! Coordinate-sorted index (CSI) reference sequence and fields.

pub mod bin;
mod metadata;

pub use self::{bin::Bin, metadata::Metadata};

use std::{io, ops::RangeBounds};

use bit_vec::BitVec;
use noodles_bgzf as bgzf;
use noodles_core::Position;

use super::resolve_interval;
use crate::binning_index::ReferenceSequenceExt;

/// A CSI reference sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceSequence {
    bins: Vec<Bin>,
    metadata: Option<Metadata>,
}

impl ReferenceSequence {
    pub(super) fn max_position(min_shift: u8, depth: u8) -> io::Result<Position> {
        assert!(min_shift > 0);
        let n = (1 << (usize::from(min_shift) + 3 * usize::from(depth))) - 1;
        Position::try_from(n).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
    }

    /// Creates a CSI reference sequence.
    ///
    /// # Examples
    ///
    /// ```
    /// use noodles_csi::index::ReferenceSequence;
    /// let reference_sequence = ReferenceSequence::new(Vec::new(), None);
    /// ```
    pub fn new(bins: Vec<Bin>, metadata: Option<Metadata>) -> Self {
        Self { bins, metadata }
    }

    /// Returns the list of bins in the reference sequence.
    ///
    /// This list does not include the metadata pseudo-bin. Use [`Self::metadata`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use noodles_csi::index::ReferenceSequence;
    /// let reference_sequence = ReferenceSequence::new(Vec::new(), None);
    /// assert!(reference_sequence.bins().is_empty());
    /// ```
    pub fn bins(&self) -> &[Bin] {
        &self.bins
    }

    /// Returns a list of bins in this reference sequence that intersects the given range.
    ///
    /// The interval values are 1-based.
    ///
    /// # Examples
    ///
    /// ```
    /// use noodles_core::Position;
    /// use noodles_csi::index::ReferenceSequence;
    ///
    /// let reference_sequence = ReferenceSequence::new(Vec::new(), None);
    /// let start = Position::try_from(8)?;
    /// let end = Position::try_from(13)?;
    ///
    /// let query_bins = reference_sequence.query(14, 5, start..=end)?;
    /// assert!(query_bins.is_empty());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn query<B>(&self, min_shift: u8, depth: u8, interval: B) -> io::Result<Vec<&Bin>>
    where
        B: RangeBounds<Position>,
    {
        let (start, end) = resolve_interval(min_shift, depth, interval)?;

        let max_bin_id = Bin::max_id(depth);
        let mut region_bins = BitVec::from_elem(max_bin_id as usize, false);

        reg2bins(start, end, min_shift, depth, &mut region_bins);

        let query_bins = self
            .bins()
            .iter()
            .filter(|b| region_bins[b.id() as usize])
            .collect();

        Ok(query_bins)
    }
}

impl ReferenceSequenceExt for ReferenceSequence {
    /// Returns the optional metadata for the reference sequence.
    ///
    /// # Examples
    ///
    /// ```
    /// use noodles_bgzf as bgzf;
    /// use noodles_csi::{
    ///     binning_index::ReferenceSequenceExt,
    ///     index::{reference_sequence::Metadata, ReferenceSequence},
    /// };
    ///
    /// let reference_sequence = ReferenceSequence::new(Vec::new(), Some(Metadata::new(
    ///     bgzf::VirtualPosition::from(610),
    ///     bgzf::VirtualPosition::from(1597),
    ///     55,
    ///     0,
    /// )));
    ///
    /// assert!(reference_sequence.metadata().is_some());
    /// ```
    fn metadata(&self) -> Option<&Metadata> {
        self.metadata.as_ref()
    }

    /// Returns the start position of the first record in the last linear bin.
    ///
    /// # Examples
    ///
    /// ```
    /// use noodles_bgzf as bgzf;
    /// use noodles_csi::{
    ///     binning_index::ReferenceSequenceExt,
    ///     index::{reference_sequence::Bin, ReferenceSequence},
    /// };
    ///
    /// let reference_sequence = ReferenceSequence::new(Vec::new(), None);
    /// assert!(reference_sequence.first_record_in_last_linear_bin_start_position().is_none());
    ///
    /// let bins = vec![
    ///     Bin::new(0, bgzf::VirtualPosition::from(8), Vec::new()),
    ///     Bin::new(1, bgzf::VirtualPosition::from(13), Vec::new()),
    /// ];
    /// let reference_sequence = ReferenceSequence::new(bins, None);
    /// assert_eq!(
    ///     reference_sequence.first_record_in_last_linear_bin_start_position(),
    ///     Some(bgzf::VirtualPosition::from(13))
    /// );
    /// ```
    fn first_record_in_last_linear_bin_start_position(&self) -> Option<bgzf::VirtualPosition> {
        self.bins().last().map(|bin| bin.loffset())
    }
}

// `CSIv1.pdf` (2020-07-21)
#[allow(clippy::many_single_char_names)]
fn reg2bins(start: Position, end: Position, min_shift: u8, depth: u8, bins: &mut BitVec) {
    // [beg, end), 0-based
    let beg = usize::from(start) - 1;
    let end = usize::from(end) - 1;

    let mut l = 0;
    let mut t = 0;
    let mut s = i32::from(min_shift) + i32::from(depth) * 3;

    while l <= depth {
        let b = t + (beg >> s);
        let e = t + (end >> s);

        for i in b..=e {
            bins.set(i as usize, true);
        }

        s -= 3;
        t += 1 << (l * 3);
        l += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN_SHIFT: u8 = 14;
    const DEPTH: u8 = 5;

    #[test]
    fn test_max_position() -> Result<(), Box<dyn std::error::Error>> {
        let actual = ReferenceSequence::max_position(MIN_SHIFT, DEPTH)?;
        let expected = Position::try_from(536870911)?;
        assert_eq!(actual, expected);
        Ok(())
    }

    #[cfg(not(target_pointer_width = "16"))]
    #[test]
    fn test_query() -> Result<(), noodles_core::position::TryFromIntError> {
        let reference_sequence = ReferenceSequence::new(Vec::new(), None);
        let end = usize::try_from(i32::MAX).and_then(Position::try_from)?;

        assert!(matches!(
            reference_sequence.query(MIN_SHIFT, DEPTH, ..=end),
            Err(e) if e.kind() == io::ErrorKind::InvalidInput,
        ));

        Ok(())
    }

    #[test]
    fn test_reg2bins() -> Result<(), noodles_core::position::TryFromIntError> {
        // +------------------------------------------------------------------------------------...
        // | 0                                                                                  ...
        // | 0-1023                                                                             ...
        // +-------------------------------------------------------------------------+----------...
        // | 1                                                                       | 2        ...
        // | 0-127                                                                   | 128-255  ...
        // +--------+--------+--------+--------+--------+--------+---------+---------+---------+...
        // | 9      | 10     | 11     | 12     | 13     | 14     | 15      | 16      | 17      |...
        // | 0-15   | 16-31  | 32-47  | 48-63  | 64-79  | 80-95  | 96-111  | 112-127 | 128-143 |...
        // +--------+--------+--------+--------+--------+--------+---------+---------+---------+...

        const MIN_SHIFT: u8 = 4;
        const DEPTH: u8 = 2;

        fn t(start: Position, end: Position, expected_bin_ids: &[u32]) {
            let max_bin_id = Bin::max_id(DEPTH);

            let mut actual = BitVec::from_elem(max_bin_id as usize, false);
            reg2bins(start, end, MIN_SHIFT, DEPTH, &mut actual);

            let mut expected = BitVec::from_elem(max_bin_id as usize, false);

            for &bin_id in expected_bin_ids {
                expected.set(bin_id as usize, true);
            }

            assert_eq!(actual, expected);
        }

        t(Position::try_from(1)?, Position::try_from(16)?, &[0, 1, 9]);
        t(Position::try_from(9)?, Position::try_from(13)?, &[0, 1, 9]);

        t(
            Position::try_from(36)?,
            Position::try_from(67)?,
            &[0, 1, 11, 12, 13],
        );

        t(
            Position::try_from(49)?,
            Position::try_from(143)?,
            &[0, 1, 2, 12, 13, 14, 15, 16, 17],
        );

        Ok(())
    }
}
