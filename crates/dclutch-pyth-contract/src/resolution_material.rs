//! Exact immutable materialization of one categorical Pyth resolution policy.
//!
//! [`CategoricalPythResolutionMaterialV1`] packages the canonical policy and
//! the feed-semantics preimage it commits. It is suitable for one immutable
//! account supplied during Market founding. This SDK-free contract performs
//! no hashing: a composing adapter must hash the policy bytes against the
//! Market's resolution-policy content identity and hash the feed-profile bytes
//! against [`CategoricalPythPolicyRecordV1::feed_profile_id`].

use crate::{
    Error, Result, array,
    feed_profile::{FEED_PROFILE_BYTES, PythFeedProfileV1},
    policy::{CategoricalPythPolicyRecordV1, POLICY_BYTES},
    zero,
};

/// Canonical materialization magic.
pub const RESOLUTION_MATERIAL_MAGIC: [u8; 8] = *b"DCLTRSM1";
/// Implemented materialization schema version.
pub const RESOLUTION_MATERIAL_SCHEMA_VERSION: u16 = 1;
/// Exact materialization byte width.
pub const RESOLUTION_MATERIAL_BYTES: usize = 16 + POLICY_BYTES + FEED_PROFILE_BYTES;

const RESERVED_OFFSET: usize = 10;
const RESERVED_BYTES: usize = 6;
const POLICY_OFFSET: usize = 16;
const FEED_PROFILE_OFFSET: usize = POLICY_OFFSET + POLICY_BYTES;

/// One exact policy plus the canonical feed-semantics preimage it names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalPythResolutionMaterialV1 {
    policy: CategoricalPythPolicyRecordV1,
    feed_profile: PythFeedProfileV1,
}

impl CategoricalPythResolutionMaterialV1 {
    /// Construct from separately canonical policy and feed-profile records.
    ///
    /// Hash correspondence is intentionally outside this crate and must be
    /// checked by the composing adapter before accepting these bytes.
    pub fn new(
        policy: CategoricalPythPolicyRecordV1,
        feed_profile: PythFeedProfileV1,
    ) -> Result<Self> {
        policy.to_kernel_policy()?;
        feed_profile.validate()?;
        Ok(Self {
            policy,
            feed_profile,
        })
    }

    /// Decode one exact hostile input and revalidate both owned records.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != RESOLUTION_MATERIAL_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != RESOLUTION_MATERIAL_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != RESOLUTION_MATERIAL_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if !zero(
            bytes
                .get(RESERVED_OFFSET..RESERVED_OFFSET + RESERVED_BYTES)
                .ok_or(Error::InvalidLength)?,
        ) {
            return Err(Error::NonCanonicalReservedBytes);
        }
        let policy = CategoricalPythPolicyRecordV1::decode(
            bytes
                .get(POLICY_OFFSET..FEED_PROFILE_OFFSET)
                .ok_or(Error::InvalidLength)?,
        )?;
        let feed_profile = PythFeedProfileV1::decode(
            bytes
                .get(FEED_PROFILE_OFFSET..RESOLUTION_MATERIAL_BYTES)
                .ok_or(Error::InvalidLength)?,
        )?;
        Self::new(policy, feed_profile)
    }

    /// Return exact canonical bytes.
    pub fn to_bytes(self) -> [u8; RESOLUTION_MATERIAL_BYTES] {
        let mut output = [0; RESOLUTION_MATERIAL_BYTES];
        put(&mut output, 0, &RESOLUTION_MATERIAL_MAGIC);
        put(
            &mut output,
            8,
            &RESOLUTION_MATERIAL_SCHEMA_VERSION.to_le_bytes(),
        );
        put(&mut output, POLICY_OFFSET, &self.policy.to_bytes());
        put(
            &mut output,
            FEED_PROFILE_OFFSET,
            &self.feed_profile.to_bytes(),
        );
        output
    }

    /// Encode into an exact caller-owned buffer without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != RESOLUTION_MATERIAL_BYTES {
            return Err(Error::OutputLength);
        }
        output.copy_from_slice(&self.to_bytes());
        Ok(())
    }

    /// Borrow the canonical Product resolution policy.
    pub const fn policy(&self) -> &CategoricalPythPolicyRecordV1 {
        &self.policy
    }

    /// Borrow the canonical feed-semantics dependency preimage.
    pub const fn feed_profile(&self) -> &PythFeedProfileV1 {
        &self.feed_profile
    }
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(input) {
        *destination = *source;
    }
}

#[cfg(test)]
mod tests {
    use dclutch_kernel::resolution::categorical_pyth_v1::{
        CategoricalPythV1PolicyInput, MAX_PRICE_CELLS,
    };

    use super::*;

    fn material() -> Result<CategoricalPythResolutionMaterialV1> {
        let profile = PythFeedProfileV1::new([1; 32], [2; 32], [3; 32])?;
        let policy = CategoricalPythPolicyRecordV1::new(CategoricalPythV1PolicyInput {
            pyth_release_id: [4; 32],
            feed_profile_id: [5; 32],
            target_time: 10,
            grace: 2,
            window: 4,
            max_crossing_lag: 3,
            max_age: 5,
            max_future_skew: 1,
            confidence_multiplier: 2,
            max_confidence_bps: 100,
            max_normalized_confidence_atoms: 10,
            normalized_decimals: 6,
            price_cell_count: 1,
            upper_edges: [0; MAX_PRICE_CELLS],
            failure_outcome_index: 1,
        })?;
        CategoricalPythResolutionMaterialV1::new(policy, profile)
    }

    #[test]
    fn exact_layout_round_trips_both_records() -> Result<()> {
        let material = material()?;
        let bytes = material.to_bytes();
        assert_eq!(RESOLUTION_MATERIAL_BYTES, 506);
        assert_eq!(bytes.get(0..8), Some(&RESOLUTION_MATERIAL_MAGIC[..]));
        assert_eq!(
            bytes.get(POLICY_OFFSET..POLICY_OFFSET + 8),
            Some(&crate::policy::POLICY_MAGIC[..])
        );
        assert_eq!(
            bytes.get(FEED_PROFILE_OFFSET..FEED_PROFILE_OFFSET + 8),
            Some(&crate::feed_profile::FEED_PROFILE_MAGIC[..])
        );
        assert_eq!(
            CategoricalPythResolutionMaterialV1::decode(&bytes),
            Ok(material)
        );
        Ok(())
    }

    #[test]
    fn hostile_envelopes_and_output_lengths_refuse_atomically() -> Result<()> {
        let material = material()?;
        let bytes = material.to_bytes();
        for length in 0..RESOLUTION_MATERIAL_BYTES {
            let prefix = bytes.get(..length).ok_or(Error::InvalidLength)?;
            assert_eq!(
                CategoricalPythResolutionMaterialV1::decode(prefix),
                Err(Error::InvalidLength)
            );
        }
        assert_eq!(
            CategoricalPythResolutionMaterialV1::decode(&[0; RESOLUTION_MATERIAL_BYTES + 1]),
            Err(Error::InvalidLength)
        );
        for (offset, expected) in [
            (0, Error::InvalidMagic),
            (8, Error::UnsupportedSchema),
            (RESERVED_OFFSET, Error::NonCanonicalReservedBytes),
        ] {
            let mut changed = bytes;
            let byte = changed.get_mut(offset).ok_or(Error::InvalidLength)?;
            *byte ^= 1;
            assert_eq!(
                CategoricalPythResolutionMaterialV1::decode(&changed),
                Err(expected)
            );
        }
        let before = [0x5a; RESOLUTION_MATERIAL_BYTES - 1];
        let mut output = before;
        assert_eq!(material.encode(&mut output), Err(Error::OutputLength));
        assert_eq!(output, before);
        Ok(())
    }
}
