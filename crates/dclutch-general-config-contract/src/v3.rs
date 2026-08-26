//! Runtime-width immutable configuration for the General successor.
//!
//! V3 binds the complete action-selected CapabilityProgramSet. Product owns
//! the finite result-domain width, so this wire contains no outcome count and
//! cannot reinstate the prototype's `N <= 16` restriction. Page and order
//! counts are positive market policy ceilings; they do not constrain page
//! balance or encode a fixed physical page capacity.

use crate::generated_v3;

/// Exact canonical General V3 config width.
pub const GENERAL_CONFIG_BYTES_V3: usize = generated_v3::GENERAL_CONFIG_BYTES_V3;
/// Domain label for the V3 immutable config schema.
pub const GENERAL_CONFIG_SCHEMA_PREIMAGE_V3: &[u8] = b"dclutch/schema/general-config-v3";
/// SHA-256 of [`GENERAL_CONFIG_SCHEMA_PREIMAGE_V3`].
pub const GENERAL_CONFIG_SCHEMA_ID_V3: [u8; 32] = [
    0x02, 0xe5, 0x7e, 0x85, 0x6d, 0xc4, 0x3a, 0xdc, 0x2e, 0xe2, 0x6d, 0x49, 0x01, 0x5e, 0xac, 0x5f,
    0xa0, 0xd6, 0xc0, 0x4b, 0x87, 0x00, 0x5a, 0x05, 0x07, 0x34, 0xf1, 0x60, 0x93, 0x51, 0xf1, 0x21,
];
const GENERAL_CONFIG_MAGIC_V3: [u8; 8] = *b"DCGCFG03";

/// Stable hostile-decode or semantic refusal from GeneralConfigV3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralConfigErrorV3 {
    /// Input had another exact byte width.
    InvalidLength,
    /// Magic, ABI version, or artifact profile selected another schema.
    UnsupportedSchema,
    /// Reserved bytes were nonzero.
    NonCanonicalReserved,
    /// One content identity or beneficiary was zero.
    ZeroIdentity,
    /// A duration, capacity, scale, or prepaid reward was zero.
    ZeroCapacity,
    /// Market, Product, ProgramSet, policy, or beneficiary coordinates differed.
    CoordinateMismatch,
}

/// Result alias for GeneralConfigV3.
pub type GeneralConfigResultV3<T> = core::result::Result<T, GeneralConfigErrorV3>;

/// Construction inputs for one immutable runtime-width General config.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralConfigV3Input {
    /// Identity of the selected liftable capacity profile.
    pub capacity_profile_id: [u8; 32],
    /// Exact ClaimBasis identity selected by the Market Product.
    pub claim_basis_id: [u8; 32],
    /// Exact complete CapabilityProgramSet content identity.
    pub program_set_id: [u8; 32],
    /// Immutable Market occurrence generation.
    pub generation: u64,
    /// Positive exact simplex denominator.
    pub price_scale: u64,
    /// Positive order-collection window in slots.
    pub collection_slots: u64,
    /// Positive candidate-selection window in slots.
    pub selection_slots: u64,
    /// Positive settlement window in slots.
    pub settlement_slots: u64,
    /// Positive candidate-wide order policy ceiling.
    pub max_orders_per_candidate: u32,
    /// Positive candidate-wide authenticated page policy ceiling.
    pub max_pages_per_candidate: u32,
    /// Exact prepaid continuation reward, never collateral or future fees.
    pub continuation_reward_lamports: u64,
    /// Immutable interpreted selection-policy content identity.
    pub selection_policy_id: [u8; 32],
    /// Immutable authority owning the replaceable quote-surplus token account.
    pub quote_surplus_beneficiary: [u8; 32],
}

/// Canonical runtime-width General configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralConfigV3 {
    input: GeneralConfigV3Input,
}

impl GeneralConfigV3 {
    /// Validate and construct one immutable V3 configuration.
    pub fn new(input: GeneralConfigV3Input) -> GeneralConfigResultV3<Self> {
        if [
            input.capacity_profile_id,
            input.claim_basis_id,
            input.program_set_id,
            input.selection_policy_id,
            input.quote_surplus_beneficiary,
        ]
        .iter()
        .any(zero)
        {
            return Err(GeneralConfigErrorV3::ZeroIdentity);
        }
        if input.generation == 0
            || input.price_scale == 0
            || input.collection_slots == 0
            || input.selection_slots == 0
            || input.settlement_slots == 0
            || input.max_orders_per_candidate == 0
            || input.max_pages_per_candidate == 0
            || input.continuation_reward_lamports == 0
        {
            return Err(GeneralConfigErrorV3::ZeroCapacity);
        }
        Ok(Self { input })
    }

    /// Hostile-decode one exact canonical V3 config.
    pub fn decode(bytes: &[u8]) -> GeneralConfigResultV3<Self> {
        if bytes.len() != GENERAL_CONFIG_BYTES_V3 {
            return Err(GeneralConfigErrorV3::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != GENERAL_CONFIG_MAGIC_V3
            || read_u16(bytes, generated_v3::CONFIG_VERSION_OFFSET_V3)?
                != generated_v3::ABI_VERSION_V3
            || read_u16(bytes, generated_v3::CONFIG_ARTIFACT_PROFILE_OFFSET_V3)?
                != generated_v3::ARTIFACT_PROFILE_V3
        {
            return Err(GeneralConfigErrorV3::UnsupportedSchema);
        }
        if bytes
            .get(
                generated_v3::CONFIG_RESERVED_OFFSET_V3
                    ..generated_v3::CONFIG_RESERVED_OFFSET_V3
                        .checked_add(4)
                        .ok_or(GeneralConfigErrorV3::InvalidLength)?,
            )
            .ok_or(GeneralConfigErrorV3::InvalidLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(GeneralConfigErrorV3::NonCanonicalReserved);
        }
        Self::new(GeneralConfigV3Input {
            capacity_profile_id: read_array(
                bytes,
                generated_v3::CONFIG_CAPACITY_PROFILE_ID_OFFSET_V3,
            )?,
            claim_basis_id: read_array(bytes, generated_v3::CONFIG_CLAIM_BASIS_ID_OFFSET_V3)?,
            program_set_id: read_array(bytes, generated_v3::CONFIG_PROGRAM_SET_ID_OFFSET_V3)?,
            generation: read_u64(bytes, generated_v3::CONFIG_GENERATION_OFFSET_V3)?,
            price_scale: read_u64(bytes, generated_v3::CONFIG_PRICE_SCALE_OFFSET_V3)?,
            collection_slots: read_u64(bytes, generated_v3::CONFIG_COLLECTION_SLOTS_OFFSET_V3)?,
            selection_slots: read_u64(bytes, generated_v3::CONFIG_SELECTION_SLOTS_OFFSET_V3)?,
            settlement_slots: read_u64(bytes, generated_v3::CONFIG_SETTLEMENT_SLOTS_OFFSET_V3)?,
            max_orders_per_candidate: read_u32(
                bytes,
                generated_v3::CONFIG_MAX_ORDERS_PER_CANDIDATE_OFFSET_V3,
            )?,
            max_pages_per_candidate: read_u32(
                bytes,
                generated_v3::CONFIG_MAX_PAGES_PER_CANDIDATE_OFFSET_V3,
            )?,
            continuation_reward_lamports: read_u64(
                bytes,
                generated_v3::CONFIG_CONTINUATION_REWARD_LAMPORTS_OFFSET_V3,
            )?,
            selection_policy_id: read_array(
                bytes,
                generated_v3::CONFIG_SELECTION_POLICY_ID_OFFSET_V3,
            )?,
            quote_surplus_beneficiary: read_array(
                bytes,
                generated_v3::CONFIG_QUOTE_SURPLUS_BENEFICIARY_OFFSET_V3,
            )?,
        })
    }

    /// Encode the exact canonical V3 preimage.
    #[must_use]
    pub fn to_bytes(self) -> [u8; GENERAL_CONFIG_BYTES_V3] {
        let mut output = [0_u8; GENERAL_CONFIG_BYTES_V3];
        put(&mut output, 0, &GENERAL_CONFIG_MAGIC_V3);
        put(
            &mut output,
            generated_v3::CONFIG_VERSION_OFFSET_V3,
            &generated_v3::ABI_VERSION_V3.to_le_bytes(),
        );
        put(
            &mut output,
            generated_v3::CONFIG_ARTIFACT_PROFILE_OFFSET_V3,
            &generated_v3::ARTIFACT_PROFILE_V3.to_le_bytes(),
        );
        for (offset, value) in [
            (
                generated_v3::CONFIG_CAPACITY_PROFILE_ID_OFFSET_V3,
                self.input.capacity_profile_id,
            ),
            (
                generated_v3::CONFIG_CLAIM_BASIS_ID_OFFSET_V3,
                self.input.claim_basis_id,
            ),
            (
                generated_v3::CONFIG_PROGRAM_SET_ID_OFFSET_V3,
                self.input.program_set_id,
            ),
            (
                generated_v3::CONFIG_SELECTION_POLICY_ID_OFFSET_V3,
                self.input.selection_policy_id,
            ),
            (
                generated_v3::CONFIG_QUOTE_SURPLUS_BENEFICIARY_OFFSET_V3,
                self.input.quote_surplus_beneficiary,
            ),
        ] {
            put(&mut output, offset, &value);
        }
        for (offset, value) in [
            (
                generated_v3::CONFIG_GENERATION_OFFSET_V3,
                self.input.generation,
            ),
            (
                generated_v3::CONFIG_PRICE_SCALE_OFFSET_V3,
                self.input.price_scale,
            ),
            (
                generated_v3::CONFIG_COLLECTION_SLOTS_OFFSET_V3,
                self.input.collection_slots,
            ),
            (
                generated_v3::CONFIG_SELECTION_SLOTS_OFFSET_V3,
                self.input.selection_slots,
            ),
            (
                generated_v3::CONFIG_SETTLEMENT_SLOTS_OFFSET_V3,
                self.input.settlement_slots,
            ),
            (
                generated_v3::CONFIG_CONTINUATION_REWARD_LAMPORTS_OFFSET_V3,
                self.input.continuation_reward_lamports,
            ),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        for (offset, value) in [
            (
                generated_v3::CONFIG_MAX_ORDERS_PER_CANDIDATE_OFFSET_V3,
                self.input.max_orders_per_candidate,
            ),
            (
                generated_v3::CONFIG_MAX_PAGES_PER_CANDIDATE_OFFSET_V3,
                self.input.max_pages_per_candidate,
            ),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        output
    }

    /// Selected liftable capacity profile identity.
    pub const fn capacity_profile_id(self) -> [u8; 32] {
        self.input.capacity_profile_id
    }

    /// Market ClaimBasis identity.
    pub const fn claim_basis_id(self) -> [u8; 32] {
        self.input.claim_basis_id
    }

    /// Complete action-selected CapabilityProgramSet identity.
    pub const fn program_set_id(self) -> [u8; 32] {
        self.input.program_set_id
    }

    /// Immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.input.generation
    }

    /// Exact simplex price scale.
    pub const fn price_scale(self) -> u64 {
        self.input.price_scale
    }

    /// Order-collection window in slots.
    pub const fn collection_slots(self) -> u64 {
        self.input.collection_slots
    }

    /// Candidate-selection window in slots.
    pub const fn selection_slots(self) -> u64 {
        self.input.selection_slots
    }

    /// Settlement window in slots.
    pub const fn settlement_slots(self) -> u64 {
        self.input.settlement_slots
    }

    /// Candidate-wide order policy ceiling.
    pub const fn max_orders_per_candidate(self) -> u32 {
        self.input.max_orders_per_candidate
    }

    /// Candidate-wide authenticated page policy ceiling.
    pub const fn max_pages_per_candidate(self) -> u32 {
        self.input.max_pages_per_candidate
    }

    /// Exact prepaid continuation reward.
    pub const fn continuation_reward_lamports(self) -> u64 {
        self.input.continuation_reward_lamports
    }

    /// Interpreted selection-policy identity.
    pub const fn selection_policy_id(self) -> [u8; 32] {
        self.input.selection_policy_id
    }

    /// Immutable surplus token owner.
    pub const fn quote_surplus_beneficiary(self) -> [u8; 32] {
        self.input.quote_surplus_beneficiary
    }

    /// Require exact Market generation and ClaimBasis coordinates.
    pub fn require_market(
        self,
        generation: u64,
        claim_basis_id: [u8; 32],
    ) -> GeneralConfigResultV3<()> {
        if generation != self.input.generation || claim_basis_id != self.input.claim_basis_id {
            Err(GeneralConfigErrorV3::CoordinateMismatch)
        } else {
            Ok(())
        }
    }

    /// Require the complete manifest-selected ProgramSet identity.
    pub fn require_program_set(self, program_set_id: [u8; 32]) -> GeneralConfigResultV3<()> {
        if program_set_id != self.input.program_set_id {
            Err(GeneralConfigErrorV3::CoordinateMismatch)
        } else {
            Ok(())
        }
    }

    /// Require one candidate against policy ceilings without page balance.
    pub fn require_candidate(
        self,
        page_count: u32,
        order_count: u32,
        price_scale: u64,
    ) -> GeneralConfigResultV3<()> {
        if page_count == 0
            || page_count > self.input.max_pages_per_candidate
            || order_count > self.input.max_orders_per_candidate
            || price_scale != self.input.price_scale
        {
            Err(GeneralConfigErrorV3::CoordinateMismatch)
        } else {
            Ok(())
        }
    }

    /// Require the parsed owner of the replaceable quote-surplus account.
    pub fn require_surplus_owner(self, owner: [u8; 32]) -> GeneralConfigResultV3<()> {
        if owner != self.input.quote_surplus_beneficiary {
            Err(GeneralConfigErrorV3::CoordinateMismatch)
        } else {
            Ok(())
        }
    }
}

fn zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> GeneralConfigResultV3<[u8; N]> {
    let end = offset
        .checked_add(N)
        .ok_or(GeneralConfigErrorV3::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(GeneralConfigErrorV3::InvalidLength)?
        .try_into()
        .map_err(|_| GeneralConfigErrorV3::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> GeneralConfigResultV3<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> GeneralConfigResultV3<u32> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> GeneralConfigResultV3<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(end) = offset.checked_add(value.len())
        && let Some(target) = output.get_mut(offset..end)
    {
        target.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn input() -> GeneralConfigV3Input {
        GeneralConfigV3Input {
            capacity_profile_id: id(1),
            claim_basis_id: id(2),
            program_set_id: id(3),
            generation: 7,
            price_scale: 1_000,
            collection_slots: 10,
            selection_slots: 11,
            settlement_slots: 12,
            max_orders_per_candidate: u32::MAX,
            max_pages_per_candidate: u32::MAX,
            continuation_reward_lamports: 5,
            selection_policy_id: id(4),
            quote_surplus_beneficiary: id(5),
        }
    }

    #[test]
    fn exact_wire_round_trips_without_width_or_page_shape_cap() {
        let config = GeneralConfigV3::new(input()).expect("runtime-width config");
        let bytes = config.to_bytes();
        assert_eq!(GeneralConfigV3::decode(&bytes), Ok(config));
        assert_eq!(bytes.len(), 232);
        assert_eq!(
            Sha256::digest(GENERAL_CONFIG_SCHEMA_PREIMAGE_V3).as_slice(),
            GENERAL_CONFIG_SCHEMA_ID_V3
        );
        assert_eq!(config.require_candidate(u32::MAX, u32::MAX, 1_000), Ok(()));
        // Product widths N=1 and N=258 do not enter config admission.
        for product_width in [1_u32, 258] {
            assert!(product_width > 0);
            assert_eq!(config.require_program_set(id(3)), Ok(()));
        }
    }

    #[test]
    fn hostile_headers_reserved_program_set_and_zero_policy_refuse() {
        let config = GeneralConfigV3::new(input()).expect("config");
        let canonical = config.to_bytes();
        for width in [0_usize, 12, GENERAL_CONFIG_BYTES_V3 - 1] {
            assert_eq!(
                GeneralConfigV3::decode(canonical.get(..width).expect("bounded prefix")),
                Err(GeneralConfigErrorV3::InvalidLength)
            );
        }
        for offset in [0_usize, 8, 10, 12] {
            let mut hostile = canonical;
            *hostile.get_mut(offset).expect("hostile byte") ^= 1;
            assert!(GeneralConfigV3::decode(&hostile).is_err());
        }
        assert_eq!(
            config.require_program_set(id(9)),
            Err(GeneralConfigErrorV3::CoordinateMismatch)
        );
        let mut zero_policy = input();
        zero_policy.selection_policy_id = [0; 32];
        assert_eq!(
            GeneralConfigV3::new(zero_policy),
            Err(GeneralConfigErrorV3::ZeroIdentity)
        );
    }
}
