//! Canonical account-membership manifest for a paged Dealer evaluation.
//!
//! A selected evaluator derives one exact deduplicated account set from the
//! admitted Dealer frame, sorts it by public key, and partitions it into six
//! balanced nonempty pages. Trading checks each page against this immutable
//! producer-owned manifest and also enforces strict ordering across page
//! boundaries, so omission, substitution, duplication, and page mixing do not
//! collapse into a caller-selected transcript.

use super::{Error as CodecError, array_at, put, require_zero};

/// Exact number of canonical membership pages.
pub const DEALER_SCENARIO_MEMBERSHIP_PAGES_V1: usize = 6;
/// Maximum accounts admitted by one page transaction.
pub const DEALER_SCENARIO_MEMBERSHIP_PAGE_MAX_ACCOUNTS_V1: u8 = 48;
/// Exact membership-manifest wire width.
pub const DEALER_SCENARIO_MEMBERSHIP_MANIFEST_BYTES_V1: usize = 320;
/// Canonical manifest magic.
pub const DEALER_SCENARIO_MEMBERSHIP_MANIFEST_MAGIC_V1: [u8; 8] = *b"DCLTDMM1";
/// Implemented manifest schema version.
pub const DEALER_SCENARIO_MEMBERSHIP_MANIFEST_VERSION_V1: u16 = 1;
/// Producer-owned PDA domain for one checkpoint-scoped manifest.
pub const DEALER_SCENARIO_MEMBERSHIP_MANIFEST_PDA_DOMAIN_V1: &[u8] = b"dclutch:dealer-members:v1";

const _: () = assert!(
    DEALER_SCENARIO_MEMBERSHIP_MANIFEST_PDA_DOMAIN_V1.len()
        <= crate::scenario_custody_reservation_v1::MAX_PDA_SEED_BYTES_V1,
    "the membership manifest domain must be a usable PDA seed"
);
/// Domain for one exact ordered membership page.
pub const DEALER_SCENARIO_MEMBERSHIP_PAGE_DOMAIN_V1: &[u8] = b"dclutch:dealer-members-page:v1";

const VERSION_OFFSET: usize = 8;
const RESERVED_OFFSET: usize = 10;
const RESERVED_BYTES: usize = 6;
const PRODUCER_OFFSET: usize = 16;
const CHECKPOINT_OFFSET: usize = 48;
const REQUEST_OFFSET: usize = 80;
const TOTAL_COUNT_OFFSET: usize = 112;
const PAGE_COUNTS_OFFSET: usize = 114;
const PAGE_RESERVED_OFFSET: usize = 120;
const PAGE_RESERVED_BYTES: usize = 8;
const PAGE_DIGESTS_OFFSET: usize = 128;

/// One producer-bound canonical account partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioMembershipManifestV1 {
    /// Release-selected evaluator which owns the manifest PDA.
    pub producer_program: [u8; 32],
    /// Trading checkpoint whose pages this manifest defines.
    pub checkpoint: [u8; 32],
    /// Exact Dealer request digest.
    pub request_digest: [u8; 32],
    /// Total distinct accounts across all pages.
    pub total_account_count: u16,
    /// Exact nonzero account count in each ordered page.
    pub page_account_counts: [u8; DEALER_SCENARIO_MEMBERSHIP_PAGES_V1],
    /// Ordered membership digest for each page.
    pub page_membership_digests: [[u8; 32]; DEALER_SCENARIO_MEMBERSHIP_PAGES_V1],
}

/// Stable hostile-decoding refusal for a membership manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioMembershipManifestErrorV1 {
    /// Fixed-layout bytes were malformed.
    Codec(CodecError),
    /// An identity, count, or digest was not canonical.
    Coordinate,
    /// Checked count arithmetic overflowed.
    Arithmetic,
}

impl From<CodecError> for DealerScenarioMembershipManifestErrorV1 {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

impl DealerScenarioMembershipManifestV1 {
    /// Hostile-decode one exact manifest body.
    pub fn decode(bytes: &[u8]) -> Result<Self, DealerScenarioMembershipManifestErrorV1> {
        if bytes.len() != DEALER_SCENARIO_MEMBERSHIP_MANIFEST_BYTES_V1 {
            return Err(DealerScenarioMembershipManifestErrorV1::Codec(
                CodecError::InvalidLength,
            ));
        }
        if bytes.get(..8) != Some(DEALER_SCENARIO_MEMBERSHIP_MANIFEST_MAGIC_V1.as_slice()) {
            return Err(DealerScenarioMembershipManifestErrorV1::Codec(
                CodecError::InvalidMagic,
            ));
        }
        let version = bytes
            .get(VERSION_OFFSET..VERSION_OFFSET + 2)
            .ok_or(CodecError::InvalidLength)?;
        if version != DEALER_SCENARIO_MEMBERSHIP_MANIFEST_VERSION_V1.to_le_bytes() {
            return Err(DealerScenarioMembershipManifestErrorV1::Codec(
                CodecError::UnsupportedVersion,
            ));
        }
        require_zero(bytes, RESERVED_OFFSET, RESERVED_BYTES)?;
        require_zero(bytes, PAGE_RESERVED_OFFSET, PAGE_RESERVED_BYTES)?;
        let mut counts = [0_u8; DEALER_SCENARIO_MEMBERSHIP_PAGES_V1];
        counts.copy_from_slice(
            bytes
                .get(PAGE_COUNTS_OFFSET..PAGE_COUNTS_OFFSET + DEALER_SCENARIO_MEMBERSHIP_PAGES_V1)
                .ok_or(CodecError::InvalidLength)?,
        );
        let mut digests = [[0_u8; 32]; DEALER_SCENARIO_MEMBERSHIP_PAGES_V1];
        for (index, digest) in digests.iter_mut().enumerate() {
            *digest = array_at(bytes, PAGE_DIGESTS_OFFSET + index * 32)?;
        }
        let total_bytes = bytes
            .get(TOTAL_COUNT_OFFSET..TOTAL_COUNT_OFFSET + 2)
            .ok_or(CodecError::InvalidLength)?;
        let mut total = [0_u8; 2];
        total.copy_from_slice(total_bytes);
        let manifest = Self {
            producer_program: array_at(bytes, PRODUCER_OFFSET)?,
            checkpoint: array_at(bytes, CHECKPOINT_OFFSET)?,
            request_digest: array_at(bytes, REQUEST_OFFSET)?,
            total_account_count: u16::from_le_bytes(total),
            page_account_counts: counts,
            page_membership_digests: digests,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Encode one exact manifest body.
    pub fn encode(
        self,
    ) -> Result<
        [u8; DEALER_SCENARIO_MEMBERSHIP_MANIFEST_BYTES_V1],
        DealerScenarioMembershipManifestErrorV1,
    > {
        self.validate()?;
        let mut bytes = [0_u8; DEALER_SCENARIO_MEMBERSHIP_MANIFEST_BYTES_V1];
        put(&mut bytes, 0, &DEALER_SCENARIO_MEMBERSHIP_MANIFEST_MAGIC_V1)?;
        put(
            &mut bytes,
            VERSION_OFFSET,
            &DEALER_SCENARIO_MEMBERSHIP_MANIFEST_VERSION_V1.to_le_bytes(),
        )?;
        put(&mut bytes, PRODUCER_OFFSET, &self.producer_program)?;
        put(&mut bytes, CHECKPOINT_OFFSET, &self.checkpoint)?;
        put(&mut bytes, REQUEST_OFFSET, &self.request_digest)?;
        put(
            &mut bytes,
            TOTAL_COUNT_OFFSET,
            &self.total_account_count.to_le_bytes(),
        )?;
        put(&mut bytes, PAGE_COUNTS_OFFSET, &self.page_account_counts)?;
        for (index, digest) in self.page_membership_digests.iter().enumerate() {
            put(&mut bytes, PAGE_DIGESTS_OFFSET + index * 32, digest)?;
        }
        Ok(bytes)
    }

    fn validate(self) -> Result<(), DealerScenarioMembershipManifestErrorV1> {
        if [self.producer_program, self.checkpoint, self.request_digest].contains(&[0; 32])
            || self.page_membership_digests.contains(&[0; 32])
            || self.page_account_counts.iter().any(|count| {
                *count == 0 || *count > DEALER_SCENARIO_MEMBERSHIP_PAGE_MAX_ACCOUNTS_V1
            })
        {
            return Err(DealerScenarioMembershipManifestErrorV1::Coordinate);
        }
        let total = self
            .page_account_counts
            .iter()
            .try_fold(0_u16, |sum, count| sum.checked_add(u16::from(*count)))
            .ok_or(DealerScenarioMembershipManifestErrorV1::Arithmetic)?;
        if total != self.total_account_count {
            return Err(DealerScenarioMembershipManifestErrorV1::Coordinate);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> DealerScenarioMembershipManifestV1 {
        DealerScenarioMembershipManifestV1 {
            producer_program: [1; 32],
            checkpoint: [2; 32],
            request_digest: [3; 32],
            total_account_count: 15,
            page_account_counts: [3, 3, 3, 2, 2, 2],
            page_membership_digests: [[4; 32], [5; 32], [6; 32], [7; 32], [8; 32], [9; 32]],
        }
    }

    #[test]
    fn manifest_round_trips_and_refuses_count_or_digest_substitution() {
        let value = manifest();
        let bytes = value.encode().expect("manifest");
        assert_eq!(
            DealerScenarioMembershipManifestV1::decode(&bytes),
            Ok(value)
        );
        let mut hostile = value;
        hostile.total_account_count = 16;
        assert_eq!(
            hostile.encode(),
            Err(DealerScenarioMembershipManifestErrorV1::Coordinate)
        );
        hostile = value;
        hostile.page_membership_digests[3] = [0; 32];
        assert_eq!(
            hostile.encode(),
            Err(DealerScenarioMembershipManifestErrorV1::Coordinate)
        );
    }
}
