//! Canonical persistent record for the categorical Pyth V1 kernel policy.
//!
//! This module owns only the exact byte representation. Semantic validity is
//! owned solely by [`CategoricalPythV1Policy`]; construction and decoding both
//! pass every field through that kernel validator.

use dclutch_resolution_policy_kernel::categorical_pyth_v1::{
    CategoricalPythV1Policy, CategoricalPythV1PolicyInput, MAX_PRICE_CELLS,
};

use crate::{Error, Result, array, zero};

/// Exact byte width of [`CategoricalPythPolicyRecordV1`].
pub const POLICY_BYTES: usize = 384;
/// Categorical Pyth policy-record magic.
pub const POLICY_MAGIC: [u8; 8] = *b"DCLTPY01";
/// Implemented categorical Pyth policy-record schema.
pub const POLICY_SCHEMA_VERSION: u16 = 1;

const RELEASE_ID_OFFSET: usize = 16;
const FEED_PROFILE_ID_OFFSET: usize = 48;
const TARGET_TIME_OFFSET: usize = 80;
const GRACE_OFFSET: usize = 88;
const WINDOW_OFFSET: usize = 92;
const MAX_CROSSING_LAG_OFFSET: usize = 96;
const MAX_AGE_OFFSET: usize = 100;
const MAX_FUTURE_SKEW_OFFSET: usize = 104;
const CONFIDENCE_MULTIPLIER_OFFSET: usize = 108;
const MAX_CONFIDENCE_BPS_OFFSET: usize = 110;
const MAX_NORMALIZED_CONFIDENCE_OFFSET: usize = 112;
const NORMALIZED_DECIMALS_OFFSET: usize = 128;
const PRICE_CELL_COUNT_OFFSET: usize = 130;
const FAILURE_OUTCOME_OFFSET: usize = 132;
const UPPER_EDGES_OFFSET: usize = 144;

/// Private-field V1 persistence record for one validated categorical Pyth policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalPythPolicyRecordV1 {
    release_id: [u8; 32],
    feed_profile_id: [u8; 32],
    target_time: i64,
    grace: u32,
    window: u32,
    max_crossing_lag: u32,
    max_age: u32,
    max_future_skew: u32,
    confidence_multiplier: u16,
    max_confidence_bps: u16,
    max_normalized_confidence_atoms: u128,
    normalized_decimals: u8,
    price_cell_count: u16,
    failure_outcome_index: u16,
    upper_edges: [u128; MAX_PRICE_CELLS],
}

impl CategoricalPythPolicyRecordV1 {
    /// Construct a record only after the kernel accepts every policy field.
    pub fn new(input: CategoricalPythV1PolicyInput) -> Result<Self> {
        validate_kernel(input)?;
        Ok(Self {
            release_id: input.pyth_release_id,
            feed_profile_id: input.feed_profile_id,
            target_time: input.target_time,
            grace: input.grace,
            window: input.window,
            max_crossing_lag: input.max_crossing_lag,
            max_age: input.max_age,
            max_future_skew: input.max_future_skew,
            confidence_multiplier: input.confidence_multiplier,
            max_confidence_bps: input.max_confidence_bps,
            max_normalized_confidence_atoms: input.max_normalized_confidence_atoms,
            normalized_decimals: input.normalized_decimals,
            price_cell_count: input.price_cell_count,
            failure_outcome_index: input.failure_outcome_index,
            upper_edges: input.upper_edges,
        })
    }

    /// Decode one exact canonical record and validate it through the kernel.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != POLICY_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != POLICY_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array::<2>(bytes, 8)?) != POLICY_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if !zero(bytes.get(10..16).ok_or(Error::InvalidLength)?)
            || bytes.get(129).copied().ok_or(Error::InvalidLength)? != 0
            || !zero(bytes.get(134..144).ok_or(Error::InvalidLength)?)
        {
            return Err(Error::NonCanonicalReservedBytes);
        }

        let mut upper_edges = [0u128; MAX_PRICE_CELLS];
        let mut index = 0usize;
        while index < MAX_PRICE_CELLS {
            let edge_offset = index
                .checked_mul(16)
                .and_then(|relative| UPPER_EDGES_OFFSET.checked_add(relative))
                .ok_or(Error::ArithmeticOverflow)?;
            let destination = upper_edges
                .get_mut(index)
                .ok_or(Error::ArithmeticOverflow)?;
            *destination = u128::from_le_bytes(array::<16>(bytes, edge_offset)?);
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }

        Self::new(CategoricalPythV1PolicyInput {
            pyth_release_id: array(bytes, RELEASE_ID_OFFSET)?,
            feed_profile_id: array(bytes, FEED_PROFILE_ID_OFFSET)?,
            target_time: i64::from_le_bytes(array(bytes, TARGET_TIME_OFFSET)?),
            grace: u32::from_le_bytes(array(bytes, GRACE_OFFSET)?),
            window: u32::from_le_bytes(array(bytes, WINDOW_OFFSET)?),
            max_crossing_lag: u32::from_le_bytes(array(bytes, MAX_CROSSING_LAG_OFFSET)?),
            max_age: u32::from_le_bytes(array(bytes, MAX_AGE_OFFSET)?),
            max_future_skew: u32::from_le_bytes(array(bytes, MAX_FUTURE_SKEW_OFFSET)?),
            confidence_multiplier: u16::from_le_bytes(array(bytes, CONFIDENCE_MULTIPLIER_OFFSET)?),
            max_confidence_bps: u16::from_le_bytes(array(bytes, MAX_CONFIDENCE_BPS_OFFSET)?),
            max_normalized_confidence_atoms: u128::from_le_bytes(array(
                bytes,
                MAX_NORMALIZED_CONFIDENCE_OFFSET,
            )?),
            normalized_decimals: bytes
                .get(NORMALIZED_DECIMALS_OFFSET)
                .copied()
                .ok_or(Error::InvalidLength)?,
            price_cell_count: u16::from_le_bytes(array(bytes, PRICE_CELL_COUNT_OFFSET)?),
            upper_edges,
            failure_outcome_index: u16::from_le_bytes(array(bytes, FAILURE_OUTCOME_OFFSET)?),
        })
    }

    /// Return the exact canonical fixed-width bytes.
    pub fn to_bytes(self) -> [u8; POLICY_BYTES] {
        let mut output = [0u8; POLICY_BYTES];
        put(&mut output, 0, &POLICY_MAGIC);
        put(&mut output, 8, &POLICY_SCHEMA_VERSION.to_le_bytes());
        put(&mut output, RELEASE_ID_OFFSET, &self.release_id);
        put(&mut output, FEED_PROFILE_ID_OFFSET, &self.feed_profile_id);
        put(
            &mut output,
            TARGET_TIME_OFFSET,
            &self.target_time.to_le_bytes(),
        );
        put(&mut output, GRACE_OFFSET, &self.grace.to_le_bytes());
        put(&mut output, WINDOW_OFFSET, &self.window.to_le_bytes());
        put(
            &mut output,
            MAX_CROSSING_LAG_OFFSET,
            &self.max_crossing_lag.to_le_bytes(),
        );
        put(&mut output, MAX_AGE_OFFSET, &self.max_age.to_le_bytes());
        put(
            &mut output,
            MAX_FUTURE_SKEW_OFFSET,
            &self.max_future_skew.to_le_bytes(),
        );
        put(
            &mut output,
            CONFIDENCE_MULTIPLIER_OFFSET,
            &self.confidence_multiplier.to_le_bytes(),
        );
        put(
            &mut output,
            MAX_CONFIDENCE_BPS_OFFSET,
            &self.max_confidence_bps.to_le_bytes(),
        );
        put(
            &mut output,
            MAX_NORMALIZED_CONFIDENCE_OFFSET,
            &self.max_normalized_confidence_atoms.to_le_bytes(),
        );
        put(
            &mut output,
            NORMALIZED_DECIMALS_OFFSET,
            &[self.normalized_decimals],
        );
        put(
            &mut output,
            PRICE_CELL_COUNT_OFFSET,
            &self.price_cell_count.to_le_bytes(),
        );
        put(
            &mut output,
            FAILURE_OUTCOME_OFFSET,
            &self.failure_outcome_index.to_le_bytes(),
        );
        for (index, edge) in self.upper_edges.iter().enumerate() {
            if let Some(relative) = index.checked_mul(16)
                && let Some(offset) = UPPER_EDGES_OFFSET.checked_add(relative)
            {
                put(&mut output, offset, &edge.to_le_bytes());
            }
        }
        output
    }

    /// Encode into an exact-width caller buffer without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != POLICY_BYTES {
            return Err(Error::OutputLength);
        }
        self.to_kernel_policy()?;
        output.copy_from_slice(&self.to_bytes());
        Ok(())
    }

    /// Reconstruct the sole semantic policy owner and validate it again.
    pub fn to_kernel_policy(&self) -> Result<CategoricalPythV1Policy> {
        validate_kernel(self.kernel_input())
    }

    /// Return the opaque immutable adapter release identifier.
    pub const fn release_id(&self) -> &[u8; 32] {
        &self.release_id
    }

    /// Return the opaque immutable feed-profile identifier.
    pub const fn feed_profile_id(&self) -> &[u8; 32] {
        &self.feed_profile_id
    }

    /// Return the target Unix timestamp.
    pub const fn target_time(&self) -> i64 {
        self.target_time
    }

    /// Return the grace duration in seconds.
    pub const fn grace(&self) -> u32 {
        self.grace
    }

    /// Return the inclusive resolution-window duration in seconds.
    pub const fn window(&self) -> u32 {
        self.window
    }

    /// Return the maximum target-crossing publication lag in seconds.
    pub const fn max_crossing_lag(&self) -> u32 {
        self.max_crossing_lag
    }

    /// Return the maximum target-crossing publication lag in seconds.
    pub const fn max_lag(&self) -> u32 {
        self.max_crossing_lag
    }

    /// Return the maximum allowed observation age in seconds.
    pub const fn max_age(&self) -> u32 {
        self.max_age
    }

    /// Return the maximum allowed publication lead in seconds.
    pub const fn max_future_skew(&self) -> u32 {
        self.max_future_skew
    }

    /// Return the provider-confidence multiplier.
    pub const fn confidence_multiplier(&self) -> u16 {
        self.confidence_multiplier
    }

    /// Return the maximum normalized relative confidence in basis points.
    pub const fn max_confidence_bps(&self) -> u16 {
        self.max_confidence_bps
    }

    /// Return the maximum normalized confidence half-width in price atoms.
    pub const fn max_normalized_confidence_atoms(&self) -> u128 {
        self.max_normalized_confidence_atoms
    }

    /// Return the normalized price decimal precision.
    pub const fn normalized_decimals(&self) -> u8 {
        self.normalized_decimals
    }

    /// Return the number of price cells before the failure outcome.
    pub const fn price_cell_count(&self) -> u16 {
        self.price_cell_count
    }

    /// Return the explicit failure outcome index.
    pub const fn failure_outcome_index(&self) -> u16 {
        self.failure_outcome_index
    }

    /// Return the explicit failure outcome index.
    pub const fn failure_index(&self) -> u16 {
        self.failure_outcome_index
    }

    /// Return all fixed-layout upper edges, including the canonical zero tail.
    pub const fn upper_edges(&self) -> &[u128; MAX_PRICE_CELLS] {
        &self.upper_edges
    }

    const fn kernel_input(&self) -> CategoricalPythV1PolicyInput {
        CategoricalPythV1PolicyInput {
            pyth_release_id: self.release_id,
            feed_profile_id: self.feed_profile_id,
            target_time: self.target_time,
            grace: self.grace,
            window: self.window,
            max_crossing_lag: self.max_crossing_lag,
            max_age: self.max_age,
            max_future_skew: self.max_future_skew,
            confidence_multiplier: self.confidence_multiplier,
            max_confidence_bps: self.max_confidence_bps,
            max_normalized_confidence_atoms: self.max_normalized_confidence_atoms,
            normalized_decimals: self.normalized_decimals,
            price_cell_count: self.price_cell_count,
            upper_edges: self.upper_edges,
            failure_outcome_index: self.failure_outcome_index,
        }
    }
}

fn validate_kernel(input: CategoricalPythV1PolicyInput) -> Result<CategoricalPythV1Policy> {
    CategoricalPythV1Policy::new(input).map_err(|error| Error::InvalidPolicy { error })
}

fn put<const N: usize>(output: &mut [u8; N], offset: usize, input: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(input) {
        *destination = *source;
    }
}

#[cfg(test)]
mod tests {
    use dclutch_resolution_policy_kernel::categorical_pyth_v1::PythV1Error;

    use super::*;

    fn input() -> CategoricalPythV1PolicyInput {
        let mut upper_edges = [0u128; MAX_PRICE_CELLS];
        if let Some(edge) = upper_edges.get_mut(0) {
            *edge = 0x100f_0e0d_0c0b_0a09_0807_0605_0403_0201;
        }
        CategoricalPythV1PolicyInput {
            pyth_release_id: [1; 32],
            feed_profile_id: [2; 32],
            target_time: -3,
            grace: 0x0403_0201,
            window: 0x0807_0605,
            max_crossing_lag: 0x0c0b_0a09,
            max_age: 0x100f_0e0d,
            max_future_skew: 0x1413_1211,
            confidence_multiplier: 0x1615,
            max_confidence_bps: 10_000,
            max_normalized_confidence_atoms: 0x201f_1e1d_1c1b_1a19_1817_1615_1413_1211,
            normalized_decimals: 18,
            price_cell_count: 2,
            upper_edges,
            failure_outcome_index: 2,
        }
    }

    #[test]
    fn exact_width_offsets_and_round_trip_are_canonical() -> Result<()> {
        assert_eq!(POLICY_BYTES, 384);
        let record = CategoricalPythPolicyRecordV1::new(input())?;
        let bytes = record.to_bytes();
        assert_eq!(bytes.get(0..8), Some(&POLICY_MAGIC[..]));
        assert_eq!(bytes.get(8..10), Some(&1u16.to_le_bytes()[..]));
        assert_eq!(bytes.get(10..16), Some(&[0; 6][..]));
        assert_eq!(bytes.get(16..48), Some(&[1; 32][..]));
        assert_eq!(bytes.get(48..80), Some(&[2; 32][..]));
        assert_eq!(bytes.get(80..88), Some(&(-3i64).to_le_bytes()[..]));
        assert_eq!(bytes.get(88..92), Some(&0x0403_0201u32.to_le_bytes()[..]));
        assert_eq!(bytes.get(92..96), Some(&0x0807_0605u32.to_le_bytes()[..]));
        assert_eq!(bytes.get(96..100), Some(&0x0c0b_0a09u32.to_le_bytes()[..]));
        assert_eq!(bytes.get(100..104), Some(&0x100f_0e0du32.to_le_bytes()[..]));
        assert_eq!(bytes.get(104..108), Some(&0x1413_1211u32.to_le_bytes()[..]));
        assert_eq!(bytes.get(108..110), Some(&0x1615u16.to_le_bytes()[..]));
        assert_eq!(bytes.get(110..112), Some(&10_000u16.to_le_bytes()[..]));
        assert_eq!(
            bytes.get(112..128),
            Some(&0x201f_1e1d_1c1b_1a19_1817_1615_1413_1211u128.to_le_bytes()[..])
        );
        assert_eq!(bytes.get(128), Some(&18));
        assert_eq!(bytes.get(129), Some(&0));
        assert_eq!(bytes.get(130..132), Some(&2u16.to_le_bytes()[..]));
        assert_eq!(bytes.get(132..134), Some(&2u16.to_le_bytes()[..]));
        assert_eq!(bytes.get(134..144), Some(&[0; 10][..]));
        assert_eq!(
            bytes.get(144..160),
            Some(&input().upper_edges[0].to_le_bytes()[..])
        );
        assert_eq!(CategoricalPythPolicyRecordV1::decode(&bytes), Ok(record));
        assert_eq!(record.to_kernel_policy()?.price_cell_count(), 2);
        assert_eq!(record.release_id(), &[1; 32]);
        assert_eq!(record.feed_profile_id(), &[2; 32]);
        assert_eq!(record.target_time(), -3);
        assert_eq!(record.grace(), 0x0403_0201);
        assert_eq!(record.window(), 0x0807_0605);
        assert_eq!(record.max_lag(), 0x0c0b_0a09);
        assert_eq!(record.max_age(), 0x100f_0e0d);
        assert_eq!(record.max_future_skew(), 0x1413_1211);
        assert_eq!(record.confidence_multiplier(), 0x1615);
        assert_eq!(record.max_confidence_bps(), 10_000);
        assert_eq!(
            record.max_normalized_confidence_atoms(),
            0x201f_1e1d_1c1b_1a19_1817_1615_1413_1211
        );
        assert_eq!(record.normalized_decimals(), 18);
        assert_eq!(record.price_cell_count(), 2);
        assert_eq!(record.failure_index(), 2);
        Ok(())
    }

    #[test]
    fn hostile_headers_reserved_bytes_and_lengths_refuse_atomically() -> Result<()> {
        let record = CategoricalPythPolicyRecordV1::new(input())?;
        let bytes = record.to_bytes();
        for length in 0..POLICY_BYTES {
            let short = bytes.get(..length).ok_or(Error::InvalidLength)?;
            assert_eq!(
                CategoricalPythPolicyRecordV1::decode(short),
                Err(Error::InvalidLength)
            );
        }
        let long = [0u8; POLICY_BYTES + 1];
        assert_eq!(
            CategoricalPythPolicyRecordV1::decode(&long),
            Err(Error::InvalidLength)
        );
        for offset in [10usize, 15, 129, 134, 143] {
            let mut hostile = bytes;
            let byte = hostile.get_mut(offset).ok_or(Error::InvalidLength)?;
            *byte = 1;
            assert_eq!(
                CategoricalPythPolicyRecordV1::decode(&hostile),
                Err(Error::NonCanonicalReservedBytes)
            );
        }
        let mut bad_magic = bytes;
        *bad_magic.get_mut(0).ok_or(Error::InvalidLength)? ^= 0xff;
        assert_eq!(
            CategoricalPythPolicyRecordV1::decode(&bad_magic),
            Err(Error::InvalidMagic)
        );
        let mut bad_schema = bytes;
        put(&mut bad_schema, 8, &2u16.to_le_bytes());
        assert_eq!(
            CategoricalPythPolicyRecordV1::decode(&bad_schema),
            Err(Error::UnsupportedSchema)
        );
        let before = [0x5a; POLICY_BYTES - 1];
        let mut wrong = before;
        assert_eq!(record.encode(&mut wrong), Err(Error::OutputLength));
        assert_eq!(wrong, before);
        Ok(())
    }

    #[test]
    fn every_semantic_refusal_comes_from_the_kernel_validator() -> Result<()> {
        let mut zero_release = input();
        zero_release.pyth_release_id = [0; 32];
        assert_eq!(
            CategoricalPythPolicyRecordV1::new(zero_release),
            Err(Error::InvalidPolicy {
                error: PythV1Error::ZeroIdentifier
            })
        );

        let mut hostile = CategoricalPythPolicyRecordV1::new(input())?.to_bytes();
        put(
            &mut hostile,
            CONFIDENCE_MULTIPLIER_OFFSET,
            &0u16.to_le_bytes(),
        );
        assert_eq!(
            CategoricalPythPolicyRecordV1::decode(&hostile),
            Err(Error::InvalidPolicy {
                error: PythV1Error::ZeroConfidenceMultiplier
            })
        );
        let mut hostile = CategoricalPythPolicyRecordV1::new(input())?.to_bytes();
        put(&mut hostile, UPPER_EDGES_OFFSET + 16, &1u128.to_le_bytes());
        assert_eq!(
            CategoricalPythPolicyRecordV1::decode(&hostile),
            Err(Error::InvalidPolicy {
                error: PythV1Error::NonzeroEdgeTail
            })
        );
        Ok(())
    }
}
