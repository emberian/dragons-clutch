#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Narrow immutable capability configuration for the General successor.
//!
//! Layout constants and the canonical fixture are generated from
//! `DClutchSemantics.GeneralConfigAbi`. This crate owns neither Solana account
//! access nor hashing. An adapter authenticates the exact config content ID
//! through the Market capability manifest before trusting any field.

#[cfg(test)]
extern crate std;

#[allow(missing_docs)]
mod generated;
/// Minimal persistent root and canonical activation plan.
pub mod root;

pub use root::*;

/// Exact canonical `GeneralConfigV2` byte width.
pub const GENERAL_CONFIG_BYTES_V2: usize = generated::GENERAL_CONFIG_BYTES_V2;
/// Provisional physical outcome-width profile selected by this release.
pub const MAX_OUTCOMES_V2: usize = generated::MAX_OUTCOMES_V2;
/// Provisional physical execution count per page selected by this release.
pub const MAX_EXECUTIONS_PER_PAGE_V2: u32 = generated::MAX_EXECUTIONS_PER_PAGE_V2;
/// Provisional physical page count per candidate selected by this release.
pub const MAX_PAGES_PER_CANDIDATE_V2: u32 = generated::MAX_PAGES_PER_CANDIDATE_V2;
/// Domain label for the V2 immutable config schema.
pub const GENERAL_CONFIG_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/schema/general-config-v2";
/// SHA-256 of [`GENERAL_CONFIG_SCHEMA_PREIMAGE_V2`].
pub const GENERAL_CONFIG_SCHEMA_ID_V2: [u8; 32] = [
    0xa4, 0x55, 0x3f, 0x77, 0x39, 0x6e, 0x5b, 0x16, 0xee, 0xd6, 0x42, 0x1d, 0x83, 0x54, 0x8b, 0x50,
    0xa1, 0x4b, 0xa3, 0x9c, 0x88, 0xe6, 0x7c, 0x83, 0x33, 0xb7, 0x46, 0xb1, 0x17, 0x0e, 0xe8, 0x20,
];
/// Domain label for the reviewed General successor capability release.
pub const GENERAL_CAPABILITY_RELEASE_PREIMAGE_V2: &[u8] =
    b"dclutch/general/frequent-batch-release/v2";
/// SHA-256 of [`GENERAL_CAPABILITY_RELEASE_PREIMAGE_V2`].
pub const GENERAL_CAPABILITY_RELEASE_ID_V2: [u8; 32] = [
    0x13, 0xe0, 0x07, 0xac, 0x87, 0x42, 0x4f, 0x29, 0x76, 0x74, 0x7c, 0x86, 0x71, 0x42, 0x8a, 0x89,
    0x7d, 0xe9, 0xde, 0x39, 0x66, 0x2c, 0x13, 0xc1, 0xed, 0x4d, 0xa1, 0x8c, 0xc6, 0x89, 0xfb, 0x8b,
];

/// Explicit refusal from hostile config bytes or invalid immutable semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The account did not contain exactly 200 bytes.
    InvalidLength,
    /// Magic, schema version, or artifact profile differed.
    UnsupportedSchema,
    /// Reserved bytes were nonzero.
    NonCanonicalReservedBytes,
    /// An immutable content identity or authority was zero.
    ZeroIdentity,
    /// The capability release was not the reviewed V2 release.
    UnrecognizedCapability,
    /// Outcome width was outside this explicitly bounded physical profile.
    InvalidOutcomeCount,
    /// A required capacity, duration, scale, or reward was zero.
    ZeroCapacity,
    /// Candidate capacity exceeded page capacity or arithmetic overflowed.
    CapacityExceeded,
    /// Market, candidate, or verifier coordinates differed from config.
    CoordinateMismatch,
    /// An operational surplus destination had another parsed token owner.
    BeneficiaryMismatch,
}

/// Result alias for immutable General config operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Inputs for one canonical immutable General capability configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralConfigV2Input {
    /// Identity of the selected liftable capacity profile.
    pub capacity_profile_id: [u8; 32],
    /// Exact ClaimBasis content identity.
    pub claim_basis_id: [u8; 32],
    /// Reviewed General capability release.
    pub capability_release_id: [u8; 32],
    /// Immutable Market occurrence generation.
    pub generation: u64,
    /// Positive simplex denominator.
    pub price_scale: u64,
    /// Positive order collection window in slots.
    pub collection_slots: u64,
    /// Positive candidate selection window in slots.
    pub selection_slots: u64,
    /// Positive settlement window in slots.
    pub settlement_slots: u64,
    /// Maximum execution rows in one candidate.
    pub max_orders_per_candidate: u32,
    /// Maximum authenticated pages in one candidate.
    pub max_pages_per_candidate: u32,
    /// Exact prepaid native continuation reward, never collateral.
    pub continuation_reward_lamports: u64,
    /// Content identity of the immutable interpreted selection policy.
    pub selection_policy_id: [u8; 32],
    /// Exact finite ClaimBasis width.
    pub outcome_count: u16,
    /// Immutable authority owning any quote-surplus destination token account.
    pub quote_surplus_beneficiary: [u8; 32],
}

/// Canonical immutable General capability configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralConfigV2 {
    input: GeneralConfigV2Input,
}

impl GeneralConfigV2 {
    /// Validate and construct one immutable configuration.
    pub fn new(input: GeneralConfigV2Input) -> Result<Self> {
        if [
            input.capacity_profile_id,
            input.claim_basis_id,
            input.selection_policy_id,
            input.quote_surplus_beneficiary,
        ]
        .iter()
        .any(is_zero)
        {
            return Err(Error::ZeroIdentity);
        }
        if input.capability_release_id != GENERAL_CAPABILITY_RELEASE_ID_V2 {
            return Err(Error::UnrecognizedCapability);
        }
        if !(2..=u16::try_from(MAX_OUTCOMES_V2).map_err(|_| Error::InvalidOutcomeCount)?)
            .contains(&input.outcome_count)
        {
            return Err(Error::InvalidOutcomeCount);
        }
        if input.price_scale == 0
            || input.collection_slots == 0
            || input.selection_slots == 0
            || input.settlement_slots == 0
            || input.max_orders_per_candidate == 0
            || input.max_pages_per_candidate == 0
            || input.continuation_reward_lamports == 0
        {
            return Err(Error::ZeroCapacity);
        }
        if input.max_pages_per_candidate > MAX_PAGES_PER_CANDIDATE_V2
            || u64::from(input.max_orders_per_candidate)
                > u64::from(input.max_pages_per_candidate)
                    .checked_mul(u64::from(MAX_EXECUTIONS_PER_PAGE_V2))
                    .ok_or(Error::CapacityExceeded)?
        {
            return Err(Error::CapacityExceeded);
        }
        Ok(Self { input })
    }

    /// Hostile-decode one exact Lean-owned config wire.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != GENERAL_CONFIG_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != generated::GENERAL_CONFIG_MAGIC_V2
            || read_u16(bytes, generated::CONFIG_VERSION_OFFSET)? != generated::ABI_VERSION_V2
            || read_u16(bytes, generated::CONFIG_ARTIFACT_PROFILE_OFFSET)?
                != generated::ARTIFACT_PROFILE_V2
        {
            return Err(Error::UnsupportedSchema);
        }
        if bytes
            .get(generated::CONFIG_RESERVED_OFFSET..generated::CONFIG_RESERVED_OFFSET + 2)
            .ok_or(Error::InvalidLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalReservedBytes);
        }
        Self::new(GeneralConfigV2Input {
            outcome_count: read_u16(bytes, generated::CONFIG_OUTCOME_COUNT_OFFSET)?,
            capacity_profile_id: read_array(bytes, generated::CONFIG_CAPACITY_PROFILE_ID_OFFSET)?,
            claim_basis_id: read_array(bytes, generated::CONFIG_CLAIM_BASIS_ID_OFFSET)?,
            capability_release_id: read_array(
                bytes,
                generated::CONFIG_CAPABILITY_RELEASE_ID_OFFSET,
            )?,
            generation: read_u64(bytes, generated::CONFIG_GENERATION_OFFSET)?,
            price_scale: read_u64(bytes, generated::CONFIG_PRICE_SCALE_OFFSET)?,
            collection_slots: read_u64(bytes, generated::CONFIG_COLLECTION_SLOTS_OFFSET)?,
            selection_slots: read_u64(bytes, generated::CONFIG_SELECTION_SLOTS_OFFSET)?,
            settlement_slots: read_u64(bytes, generated::CONFIG_SETTLEMENT_SLOTS_OFFSET)?,
            max_orders_per_candidate: read_u32(
                bytes,
                generated::CONFIG_MAX_ORDERS_PER_CANDIDATE_OFFSET,
            )?,
            max_pages_per_candidate: read_u32(
                bytes,
                generated::CONFIG_MAX_PAGES_PER_CANDIDATE_OFFSET,
            )?,
            continuation_reward_lamports: read_u64(
                bytes,
                generated::CONFIG_CONTINUATION_REWARD_LAMPORTS_OFFSET,
            )?,
            selection_policy_id: read_array(bytes, generated::CONFIG_SELECTION_POLICY_ID_OFFSET)?,
            quote_surplus_beneficiary: read_array(
                bytes,
                generated::CONFIG_QUOTE_SURPLUS_BENEFICIARY_OFFSET,
            )?,
        })
    }

    /// Encode the exact canonical config preimage.
    #[must_use]
    pub fn to_bytes(self) -> [u8; GENERAL_CONFIG_BYTES_V2] {
        let mut output = [0_u8; GENERAL_CONFIG_BYTES_V2];
        put(&mut output, 0, &generated::GENERAL_CONFIG_MAGIC_V2);
        put(
            &mut output,
            generated::CONFIG_VERSION_OFFSET,
            &generated::ABI_VERSION_V2.to_le_bytes(),
        );
        put(
            &mut output,
            generated::CONFIG_ARTIFACT_PROFILE_OFFSET,
            &generated::ARTIFACT_PROFILE_V2.to_le_bytes(),
        );
        put(
            &mut output,
            generated::CONFIG_OUTCOME_COUNT_OFFSET,
            &self.input.outcome_count.to_le_bytes(),
        );
        for (offset, value) in [
            (
                generated::CONFIG_CAPACITY_PROFILE_ID_OFFSET,
                self.input.capacity_profile_id,
            ),
            (
                generated::CONFIG_CLAIM_BASIS_ID_OFFSET,
                self.input.claim_basis_id,
            ),
            (
                generated::CONFIG_CAPABILITY_RELEASE_ID_OFFSET,
                self.input.capability_release_id,
            ),
            (
                generated::CONFIG_SELECTION_POLICY_ID_OFFSET,
                self.input.selection_policy_id,
            ),
            (
                generated::CONFIG_QUOTE_SURPLUS_BENEFICIARY_OFFSET,
                self.input.quote_surplus_beneficiary,
            ),
        ] {
            put(&mut output, offset, &value);
        }
        for (offset, value) in [
            (generated::CONFIG_GENERATION_OFFSET, self.input.generation),
            (generated::CONFIG_PRICE_SCALE_OFFSET, self.input.price_scale),
            (
                generated::CONFIG_COLLECTION_SLOTS_OFFSET,
                self.input.collection_slots,
            ),
            (
                generated::CONFIG_SELECTION_SLOTS_OFFSET,
                self.input.selection_slots,
            ),
            (
                generated::CONFIG_SETTLEMENT_SLOTS_OFFSET,
                self.input.settlement_slots,
            ),
            (
                generated::CONFIG_CONTINUATION_REWARD_LAMPORTS_OFFSET,
                self.input.continuation_reward_lamports,
            ),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        for (offset, value) in [
            (
                generated::CONFIG_MAX_ORDERS_PER_CANDIDATE_OFFSET,
                self.input.max_orders_per_candidate,
            ),
            (
                generated::CONFIG_MAX_PAGES_PER_CANDIDATE_OFFSET,
                self.input.max_pages_per_candidate,
            ),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        output
    }

    /// Immutable capacity-profile identity.
    #[must_use]
    pub const fn capacity_profile_id(self) -> [u8; 32] {
        self.input.capacity_profile_id
    }

    /// Exact ClaimBasis identity.
    #[must_use]
    pub const fn claim_basis_id(self) -> [u8; 32] {
        self.input.claim_basis_id
    }

    /// Reviewed General successor capability release.
    #[must_use]
    pub const fn capability_release_id(self) -> [u8; 32] {
        self.input.capability_release_id
    }

    /// Immutable occurrence generation.
    #[must_use]
    pub const fn generation(self) -> u64 {
        self.input.generation
    }

    /// Positive simplex denominator.
    #[must_use]
    pub const fn price_scale(self) -> u64 {
        self.input.price_scale
    }

    /// Collection-window width.
    #[must_use]
    pub const fn collection_slots(self) -> u64 {
        self.input.collection_slots
    }

    /// Candidate-selection window width.
    #[must_use]
    pub const fn selection_slots(self) -> u64 {
        self.input.selection_slots
    }

    /// Settlement-window width.
    #[must_use]
    pub const fn settlement_slots(self) -> u64 {
        self.input.settlement_slots
    }

    /// Maximum rows in one candidate.
    #[must_use]
    pub const fn max_orders_per_candidate(self) -> u32 {
        self.input.max_orders_per_candidate
    }

    /// Maximum authenticated pages in one candidate.
    #[must_use]
    pub const fn max_pages_per_candidate(self) -> u32 {
        self.input.max_pages_per_candidate
    }

    /// Exact prepaid native continuation reward.
    #[must_use]
    pub const fn continuation_reward_lamports(self) -> u64 {
        self.input.continuation_reward_lamports
    }

    /// Immutable interpreted selection-policy content identity.
    #[must_use]
    pub const fn selection_policy_id(self) -> [u8; 32] {
        self.input.selection_policy_id
    }

    /// Exact finite ClaimBasis width.
    #[must_use]
    pub const fn outcome_count(self) -> u16 {
        self.input.outcome_count
    }

    /// Immutable token-owner authority for quote-surplus routing.
    #[must_use]
    pub const fn quote_surplus_beneficiary(self) -> [u8; 32] {
        self.input.quote_surplus_beneficiary
    }

    /// Bind the authenticated Core Market occurrence to this immutable config.
    pub fn require_market_coordinates(
        self,
        generation: u64,
        claim_basis_id: [u8; 32],
    ) -> Result<()> {
        if generation == self.input.generation && claim_basis_id == self.input.claim_basis_id {
            Ok(())
        } else {
            Err(Error::CoordinateMismatch)
        }
    }

    /// Bind one candidate and its streamed distinct-order count to this config.
    pub fn require_candidate_envelope(
        self,
        outcome_count: u8,
        page_count: u32,
        price_scale: u64,
        distinct_order_count: u32,
    ) -> Result<()> {
        if u16::from(outcome_count) != self.input.outcome_count
            || price_scale != self.input.price_scale
        {
            return Err(Error::CoordinateMismatch);
        }
        if page_count == 0
            || page_count > self.input.max_pages_per_candidate
            || distinct_order_count > self.input.max_orders_per_candidate
        {
            return Err(Error::CapacityExceeded);
        }
        Ok(())
    }

    /// Require an operational destination token account to have the immutable
    /// surplus-beneficiary owner.
    pub fn require_quote_surplus_owner(self, token_owner: [u8; 32]) -> Result<()> {
        if token_owner == self.input.quote_surplus_beneficiary {
            Ok(())
        } else {
            Err(Error::BeneficiaryMismatch)
        }
    }

    /// Require a policy account/cursor to use the capability-selected identity.
    pub fn require_selection_policy(self, policy_id: [u8; 32]) -> Result<()> {
        if policy_id == self.input.selection_policy_id {
            Ok(())
        } else {
            Err(Error::CoordinateMismatch)
        }
    }
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    input
        .get(offset..offset.checked_add(N).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset + value.len()) {
        target.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use super::*;
    use sha2::{Digest, Sha256};
    use std::vec::Vec;

    fn input() -> GeneralConfigV2Input {
        let mut capacity = [0_u8; 32];
        capacity[0] = 0x11;
        let mut claim_basis = [0_u8; 32];
        claim_basis[0] = 0x22;
        let mut beneficiary = [0_u8; 32];
        beneficiary[0] = 0x44;
        GeneralConfigV2Input {
            capacity_profile_id: capacity,
            claim_basis_id: claim_basis,
            capability_release_id: GENERAL_CAPABILITY_RELEASE_ID_V2,
            generation: 7,
            price_scale: 100,
            collection_slots: 10,
            selection_slots: 11,
            settlement_slots: 12,
            max_orders_per_candidate: 64,
            max_pages_per_candidate: 2,
            continuation_reward_lamports: 5,
            selection_policy_id: id(0x33),
            outcome_count: 2,
            quote_surplus_beneficiary: beneficiary,
        }
    }

    #[test]
    fn lean_fixture_round_trips_exactly() {
        let config = GeneralConfigV2::new(input())
            .unwrap_or_else(|error| std::panic!("fixture construction failed: {error:?}"));
        assert_eq!(config.to_bytes(), generated::GENERAL_CONFIG_EXAMPLE_V2);
        assert_eq!(GeneralConfigV2::decode(&config.to_bytes()), Ok(config));
        assert_eq!(
            Sha256::digest(GENERAL_CONFIG_SCHEMA_PREIMAGE_V2).as_slice(),
            GENERAL_CONFIG_SCHEMA_ID_V2
        );
        assert_eq!(
            Sha256::digest(GENERAL_CAPABILITY_RELEASE_PREIMAGE_V2).as_slice(),
            GENERAL_CAPABILITY_RELEASE_ID_V2
        );
    }

    #[test]
    fn hostile_width_header_identity_capacity_and_beneficiary_refuse() {
        let canonical = GeneralConfigV2::new(input())
            .unwrap_or_else(|error| std::panic!("fixture failed: {error:?}"))
            .to_bytes();
        for width in 0..GENERAL_CONFIG_BYTES_V2 {
            let truncated = canonical
                .get(..width)
                .unwrap_or_else(|| std::panic!("fixture width out of range"));
            assert_eq!(
                GeneralConfigV2::decode(truncated),
                Err(Error::InvalidLength)
            );
        }
        let mut extended = Vec::from(canonical);
        extended.push(0);
        assert_eq!(
            GeneralConfigV2::decode(&extended),
            Err(Error::InvalidLength)
        );
        for (offset, expected) in [
            (0, Error::UnsupportedSchema),
            (generated::CONFIG_VERSION_OFFSET, Error::UnsupportedSchema),
            (
                generated::CONFIG_ARTIFACT_PROFILE_OFFSET,
                Error::UnsupportedSchema,
            ),
            (
                generated::CONFIG_RESERVED_OFFSET,
                Error::NonCanonicalReservedBytes,
            ),
            (
                generated::CONFIG_CAPABILITY_RELEASE_ID_OFFSET,
                Error::UnrecognizedCapability,
            ),
        ] {
            let mut hostile = canonical;
            let byte = hostile
                .get_mut(offset)
                .unwrap_or_else(|| std::panic!("fixture offset out of range"));
            *byte ^= 1;
            assert_eq!(GeneralConfigV2::decode(&hostile), Err(expected));
        }
        for offset in [
            generated::CONFIG_CAPACITY_PROFILE_ID_OFFSET,
            generated::CONFIG_CLAIM_BASIS_ID_OFFSET,
            generated::CONFIG_SELECTION_POLICY_ID_OFFSET,
            generated::CONFIG_QUOTE_SURPLUS_BENEFICIARY_OFFSET,
        ] {
            let mut hostile = canonical;
            hostile
                .get_mut(offset..offset + 32)
                .unwrap_or_else(|| std::panic!("fixture identity out of range"))
                .fill(0);
            assert_eq!(GeneralConfigV2::decode(&hostile), Err(Error::ZeroIdentity));
        }
        for outcome_count in [0_u16, 1, 17, u16::MAX] {
            let mut hostile = canonical;
            hostile
                .get_mut(
                    generated::CONFIG_OUTCOME_COUNT_OFFSET
                        ..generated::CONFIG_OUTCOME_COUNT_OFFSET + 2,
                )
                .unwrap_or_else(|| std::panic!("fixture outcome out of range"))
                .copy_from_slice(&outcome_count.to_le_bytes());
            assert_eq!(
                GeneralConfigV2::decode(&hostile),
                Err(Error::InvalidOutcomeCount)
            );
        }
        for (offset, width) in [
            (generated::CONFIG_PRICE_SCALE_OFFSET, 8),
            (generated::CONFIG_COLLECTION_SLOTS_OFFSET, 8),
            (generated::CONFIG_SELECTION_SLOTS_OFFSET, 8),
            (generated::CONFIG_SETTLEMENT_SLOTS_OFFSET, 8),
            (generated::CONFIG_MAX_ORDERS_PER_CANDIDATE_OFFSET, 4),
            (generated::CONFIG_MAX_PAGES_PER_CANDIDATE_OFFSET, 4),
            (generated::CONFIG_CONTINUATION_REWARD_LAMPORTS_OFFSET, 8),
        ] {
            let mut hostile = canonical;
            hostile
                .get_mut(offset..offset + width)
                .unwrap_or_else(|| std::panic!("fixture scalar out of range"))
                .fill(0);
            assert_eq!(GeneralConfigV2::decode(&hostile), Err(Error::ZeroCapacity));
        }
        let mut over_capacity = input();
        over_capacity.max_orders_per_candidate = 65;
        assert_eq!(
            GeneralConfigV2::new(over_capacity),
            Err(Error::CapacityExceeded)
        );
        over_capacity = input();
        over_capacity.max_pages_per_candidate = MAX_PAGES_PER_CANDIDATE_V2 + 1;
        assert_eq!(
            GeneralConfigV2::new(over_capacity),
            Err(Error::CapacityExceeded)
        );
    }

    #[test]
    fn operational_surplus_account_is_owner_bound_not_immortal() {
        let config = GeneralConfigV2::new(input())
            .unwrap_or_else(|error| std::panic!("fixture failed: {error:?}"));
        assert_eq!(
            config.require_quote_surplus_owner(config.quote_surplus_beneficiary()),
            Ok(())
        );
        let mut substitute = config.quote_surplus_beneficiary();
        substitute[0] ^= 1;
        assert_eq!(
            config.require_quote_surplus_owner(substitute),
            Err(Error::BeneficiaryMismatch)
        );
    }

    #[test]
    fn market_candidate_and_streamed_order_coordinates_are_config_bound() {
        let config = GeneralConfigV2::new(input())
            .unwrap_or_else(|error| std::panic!("fixture failed: {error:?}"));
        assert_eq!(config.require_market_coordinates(7, id(0x22)), Ok(()));
        assert_eq!(config.require_candidate_envelope(2, 2, 100, 64), Ok(()));
        assert_eq!(config.require_selection_policy(id(0x33)), Ok(()));
        assert_eq!(
            config.require_market_coordinates(8, id(0x22)),
            Err(Error::CoordinateMismatch)
        );
        assert_eq!(
            config.require_market_coordinates(7, id(0x23)),
            Err(Error::CoordinateMismatch)
        );
        assert_eq!(
            config.require_candidate_envelope(3, 2, 100, 64),
            Err(Error::CoordinateMismatch)
        );
        assert_eq!(
            config.require_candidate_envelope(2, 3, 100, 65),
            Err(Error::CapacityExceeded)
        );
        assert_eq!(
            config.require_selection_policy(id(0x34)),
            Err(Error::CoordinateMismatch)
        );
    }

    fn id(low: u8) -> [u8; 32] {
        let mut value = [0_u8; 32];
        value[0] = low;
        value
    }
}
