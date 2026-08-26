//! Product-free ordered Source recovery policy V2.

use core::convert::TryInto;

use super::{
    ContentId, Error, Result,
    generated_source_recovery_policy_v2::{
        RECOVERY_ATTEMPT_BYTES_V2, RECOVERY_ATTEMPT_V2_DEADLINE_OFFSET,
        RECOVERY_ATTEMPT_V2_FUNDING_ALLOCATION_OFFSET, RECOVERY_ATTEMPT_V2_PROVIDER_RELEASE_OFFSET,
        RECOVERY_ATTEMPT_V2_RESERVED_OFFSET, RECOVERY_ATTEMPT_V2_SOURCE_SPEC_OFFSET,
        RECOVERY_POLICY_BYTES_V2, RECOVERY_POLICY_MAGIC_V2, RECOVERY_POLICY_MAX_ATTEMPTS_V2,
        RECOVERY_POLICY_SCHEMA_VERSION_V2, RECOVERY_POLICY_V2_ATTEMPT_0_OFFSET,
        RECOVERY_POLICY_V2_ATTEMPT_1_OFFSET, RECOVERY_POLICY_V2_ATTEMPT_2_OFFSET,
        RECOVERY_POLICY_V2_ATTEMPT_3_OFFSET, RECOVERY_POLICY_V2_ATTEMPT_COUNT_OFFSET,
        RECOVERY_POLICY_V2_CAPACITY_PROFILE_OFFSET, RECOVERY_POLICY_V2_MAGIC_OFFSET,
        RECOVERY_POLICY_V2_RESERVED_OFFSET, RECOVERY_POLICY_V2_VERSION_OFFSET,
    },
};

const HEADER_RESERVED_BYTES: usize = 5;
const ATTEMPT_RESERVED_BYTES: usize = 8;

/// One immutable funded recovery attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryAttemptV2 {
    source_spec_id: ContentId,
    provider_release_id: ContentId,
    deadline_unix_seconds: i64,
    funding_allocation_id: ContentId,
}

impl RecoveryAttemptV2 {
    /// Construct one exact positive-deadline attempt.
    pub fn new(
        source_spec_id: ContentId,
        provider_release_id: ContentId,
        deadline_unix_seconds: i64,
        funding_allocation_id: ContentId,
    ) -> Result<Self> {
        if deadline_unix_seconds <= 0 {
            return Err(Error::NonCanonicalRecoveryOrder);
        }
        Ok(Self {
            source_spec_id,
            provider_release_id,
            deadline_unix_seconds,
            funding_allocation_id,
        })
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != RECOVERY_ATTEMPT_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        require_zero(
            bytes,
            RECOVERY_ATTEMPT_V2_RESERVED_OFFSET,
            ATTEMPT_RESERVED_BYTES,
        )?;
        Self::new(
            content(bytes, RECOVERY_ATTEMPT_V2_SOURCE_SPEC_OFFSET)?,
            content(bytes, RECOVERY_ATTEMPT_V2_PROVIDER_RELEASE_OFFSET)?,
            i64::from_le_bytes(array(bytes, RECOVERY_ATTEMPT_V2_DEADLINE_OFFSET)?),
            content(bytes, RECOVERY_ATTEMPT_V2_FUNDING_ALLOCATION_OFFSET)?,
        )
    }

    fn to_bytes(self) -> [u8; RECOVERY_ATTEMPT_BYTES_V2] {
        let mut output = [0_u8; RECOVERY_ATTEMPT_BYTES_V2];
        put(
            &mut output,
            RECOVERY_ATTEMPT_V2_SOURCE_SPEC_OFFSET,
            self.source_spec_id.as_bytes(),
        );
        put(
            &mut output,
            RECOVERY_ATTEMPT_V2_PROVIDER_RELEASE_OFFSET,
            self.provider_release_id.as_bytes(),
        );
        put(
            &mut output,
            RECOVERY_ATTEMPT_V2_DEADLINE_OFFSET,
            &self.deadline_unix_seconds.to_le_bytes(),
        );
        put(
            &mut output,
            RECOVERY_ATTEMPT_V2_FUNDING_ALLOCATION_OFFSET,
            self.funding_allocation_id.as_bytes(),
        );
        output
    }

    /// Exact SourceSpec content identity for this attempt.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// Exact selected provider-release content identity.
    pub const fn provider_release_id(self) -> ContentId {
        self.provider_release_id
    }

    /// Inclusive attempt deadline in Unix seconds.
    pub const fn deadline_unix_seconds(self) -> i64 {
        self.deadline_unix_seconds
    }

    /// Exact capability funding-allocation identity.
    pub const fn funding_allocation_id(self) -> ContentId {
        self.funding_allocation_id
    }
}

/// Product-free finite ordered recovery policy selected by `SourceMaterialV2`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryPolicyV2 {
    capacity_profile_id: ContentId,
    attempts: [Option<RecoveryAttemptV2>; RECOVERY_POLICY_MAX_ATTEMPTS_V2],
    attempt_count: u8,
}

impl RecoveryPolicyV2 {
    /// Construct one canonical finite ordered recovery policy.
    pub fn new(
        capacity_profile_id: ContentId,
        attempts: [Option<RecoveryAttemptV2>; RECOVERY_POLICY_MAX_ATTEMPTS_V2],
        attempt_count: u8,
    ) -> Result<Self> {
        let value = Self {
            capacity_profile_id,
            attempts,
            attempt_count,
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Hostile-decode one exact 496-byte finalized policy body.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != RECOVERY_POLICY_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, RECOVERY_POLICY_V2_MAGIC_OFFSET)? != RECOVERY_POLICY_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, RECOVERY_POLICY_V2_VERSION_OFFSET)?)
            != RECOVERY_POLICY_SCHEMA_VERSION_V2
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(
            bytes,
            RECOVERY_POLICY_V2_RESERVED_OFFSET,
            HEADER_RESERVED_BYTES,
        )?;
        let attempt_count = byte(bytes, RECOVERY_POLICY_V2_ATTEMPT_COUNT_OFFSET)?;
        if attempt_count == 0 || usize::from(attempt_count) > RECOVERY_POLICY_MAX_ATTEMPTS_V2 {
            return Err(Error::RecoveryExceedsCapacity);
        }
        let mut attempts = [None; RECOVERY_POLICY_MAX_ATTEMPTS_V2];
        let mut index = 0_usize;
        while index < RECOVERY_POLICY_MAX_ATTEMPTS_V2 {
            let offset = attempt_offset(index)?;
            let slot = bytes
                .get(
                    offset
                        ..offset
                            .checked_add(RECOVERY_ATTEMPT_BYTES_V2)
                            .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::InvalidLength)?;
            if index < usize::from(attempt_count) {
                let destination = attempts.get_mut(index).ok_or(Error::ArithmeticOverflow)?;
                *destination = Some(RecoveryAttemptV2::decode(slot)?);
            } else {
                require_zero(slot, 0, RECOVERY_ATTEMPT_BYTES_V2)?;
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Self::new(
            content(bytes, RECOVERY_POLICY_V2_CAPACITY_PROFILE_OFFSET)?,
            attempts,
            attempt_count,
        )
    }

    /// Encode the exact canonical finalized policy body.
    #[must_use]
    pub fn to_bytes(self) -> [u8; RECOVERY_POLICY_BYTES_V2] {
        let mut output = [0_u8; RECOVERY_POLICY_BYTES_V2];
        put(
            &mut output,
            RECOVERY_POLICY_V2_MAGIC_OFFSET,
            &RECOVERY_POLICY_MAGIC_V2,
        );
        put(
            &mut output,
            RECOVERY_POLICY_V2_VERSION_OFFSET,
            &RECOVERY_POLICY_SCHEMA_VERSION_V2.to_le_bytes(),
        );
        output[RECOVERY_POLICY_V2_ATTEMPT_COUNT_OFFSET] = self.attempt_count;
        put(
            &mut output,
            RECOVERY_POLICY_V2_CAPACITY_PROFILE_OFFSET,
            self.capacity_profile_id.as_bytes(),
        );
        for (index, attempt) in self.attempts.iter().copied().enumerate() {
            if let Some(attempt) = attempt {
                let offset = RECOVERY_POLICY_V2_ATTEMPT_0_OFFSET
                    .saturating_add(index.saturating_mul(RECOVERY_ATTEMPT_BYTES_V2));
                put(&mut output, offset, &attempt.to_bytes());
            }
        }
        output
    }

    /// Require the independently authenticated Source capacity-profile identity.
    pub fn validate_capacity_profile(self, expected: ContentId) -> Result<()> {
        if self.capacity_profile_id == expected {
            Ok(())
        } else {
            Err(Error::LinkageMismatch)
        }
    }

    /// Exact Source capacity-profile content identity.
    pub const fn capacity_profile_id(self) -> ContentId {
        self.capacity_profile_id
    }

    /// Number of ordered funded attempts.
    pub const fn attempt_count(self) -> u8 {
        self.attempt_count
    }

    /// Return one exact ordered attempt.
    pub fn attempt(self, index: u8) -> Result<RecoveryAttemptV2> {
        if index >= self.attempt_count {
            return Err(Error::RecoveryExceedsCapacity);
        }
        self.attempts
            .get(usize::from(index))
            .copied()
            .flatten()
            .ok_or(Error::NonCanonicalReservedBytes)
    }

    fn validate_shape(self) -> Result<()> {
        if self.attempt_count == 0
            || usize::from(self.attempt_count) > RECOVERY_POLICY_MAX_ATTEMPTS_V2
        {
            return Err(Error::RecoveryExceedsCapacity);
        }
        let mut prior_deadline = 0_i64;
        for (index, attempt) in self.attempts.iter().copied().enumerate() {
            if index < usize::from(self.attempt_count) {
                let attempt = attempt.ok_or(Error::NonCanonicalReservedBytes)?;
                if attempt.deadline_unix_seconds() <= prior_deadline {
                    return Err(Error::NonCanonicalRecoveryOrder);
                }
                prior_deadline = attempt.deadline_unix_seconds();
            } else if attempt.is_some() {
                return Err(Error::NonCanonicalReservedBytes);
            }
        }
        Ok(())
    }
}

fn attempt_offset(index: usize) -> Result<usize> {
    [
        RECOVERY_POLICY_V2_ATTEMPT_0_OFFSET,
        RECOVERY_POLICY_V2_ATTEMPT_1_OFFSET,
        RECOVERY_POLICY_V2_ATTEMPT_2_OFFSET,
        RECOVERY_POLICY_V2_ATTEMPT_3_OFFSET,
    ]
    .get(index)
    .copied()
    .ok_or(Error::ArithmeticOverflow)
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn content(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(array(bytes, offset)?)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if bytes
        .get(offset..offset.checked_add(width).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0)
    {
        Ok(())
    } else {
        Err(Error::NonCanonicalReservedBytes)
    }
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(value.iter().copied()) {
        *destination = source;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_source_recovery_policy_v2::{
        RECOVERY_POLICY_V2_EXAMPLE, RECOVERY_POLICY_V2_REFUSAL_CORPUS,
        RECOVERY_POLICY_V2_REFUSAL_COUNT,
    };

    fn id(tag: u8) -> ContentId {
        let mut bytes = [0_u8; 32];
        bytes[0] = tag;
        ContentId::new(bytes).expect("nonzero")
    }

    #[test]
    fn lean_generated_policy_and_refusals_agree() {
        let expected = RecoveryPolicyV2::new(
            id(1),
            [
                Some(RecoveryAttemptV2::new(id(2), id(3), 100, id(4)).expect("attempt")),
                Some(RecoveryAttemptV2::new(id(5), id(6), 200, id(7)).expect("attempt")),
                None,
                None,
            ],
            2,
        )
        .expect("policy");
        assert_eq!(expected.to_bytes(), RECOVERY_POLICY_V2_EXAMPLE);
        assert_eq!(
            RecoveryPolicyV2::decode(&RECOVERY_POLICY_V2_EXAMPLE),
            Ok(expected)
        );
        assert_eq!(
            RECOVERY_POLICY_V2_REFUSAL_CORPUS.len(),
            RECOVERY_POLICY_V2_REFUSAL_COUNT
        );
        for hostile in RECOVERY_POLICY_V2_REFUSAL_CORPUS {
            assert!(RecoveryPolicyV2::decode(&hostile).is_err());
        }
    }

    #[test]
    fn product_is_not_a_recovery_policy_coordinate() {
        let policy = RecoveryPolicyV2::decode(&RECOVERY_POLICY_V2_EXAMPLE).expect("policy");
        assert_eq!(policy.capacity_profile_id(), id(1));
        assert_eq!(policy.attempt_count(), 2);
        assert_eq!(
            policy.attempt(0).expect("first").funding_allocation_id(),
            id(4)
        );
        assert_eq!(
            policy.attempt(1).expect("second").provider_release_id(),
            id(6)
        );
        assert_eq!(policy.attempt(2), Err(Error::RecoveryExceedsCapacity));
    }

    #[test]
    fn late_failure_does_not_mutate_the_candidate() {
        let first = RecoveryAttemptV2::new(id(2), id(3), 200, id(4)).expect("attempt");
        let second = RecoveryAttemptV2::new(id(5), id(6), 100, id(7)).expect("attempt");
        let attempts = [Some(first), Some(second), None, None];
        assert_eq!(
            RecoveryPolicyV2::new(id(1), attempts, 2),
            Err(Error::NonCanonicalRecoveryOrder)
        );
        assert_eq!(attempts, [Some(first), Some(second), None, None]);
    }
}
