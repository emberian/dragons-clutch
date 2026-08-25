//! Content-bound Product admission and pure Claims transition candidates.
//!
//! This module does not own accounts, tokens, hashing policy, or custody. It
//! consumes an adapter-authenticated content identity, checks that the exact
//! basis record is the Product-bound record Claims expected, and produces
//! complete candidate state in caller-owned fixed buffers. No candidate bytes
//! are written until every check succeeds.

use core::convert::{TryFrom, TryInto};

use crate::{Error as KernelError, capped_ramp_complement_floor_boundary_v2};

/// Exact byte width of an opaque nonzero content identity.
pub const CONTENT_ID_BYTES_V2: usize = 32;
/// Fixed header width shared by every LiabilityBasisV2 record.
pub const BASIS_HEADER_BYTES_V2: usize = 128;
/// Exact categorical-Q=1 record width.
pub const CATEGORICAL_BASIS_BYTES_V2: usize = BASIS_HEADER_BYTES_V2;
/// Exact capped-ramp/complement record width.
pub const CAPPED_RAMP_BASIS_BYTES_V2: usize = BASIS_HEADER_BYTES_V2 + 24;
/// Canonical LiabilityBasisV2 record magic.
pub const BASIS_MAGIC_V2: [u8; 8] = *b"DCLTLBV2";
/// Implemented LiabilityBasisV2 record schema.
pub const BASIS_SCHEMA_V2: u16 = 2;
/// Safe checked-integer physical profile.
pub const BASIS_PHYSICAL_PROFILE_V2: u16 = 1;

/// Categorical evaluator release preimage.
pub const CATEGORICAL_EVALUATOR_RELEASE_PREIMAGE_V2: &[u8] = b"dclutch/evaluator/categorical-q1-v2";
/// SHA-256 identity of [`CATEGORICAL_EVALUATOR_RELEASE_PREIMAGE_V2`].
pub const CATEGORICAL_EVALUATOR_RELEASE_ID_V2: [u8; 32] = [
    0x07, 0x04, 0x1d, 0xa4, 0xfb, 0x89, 0x93, 0xc0, 0x64, 0x13, 0x68, 0x65, 0x48, 0x35, 0xff, 0x0d,
    0x8a, 0xf9, 0x6f, 0xfb, 0xc1, 0x80, 0x80, 0xde, 0x62, 0x9e, 0xd5, 0xcb, 0x9a, 0x72, 0xfb, 0x5c,
];
/// Capped-ramp/complement evaluator release preimage.
pub const CAPPED_RAMP_EVALUATOR_RELEASE_PREIMAGE_V2: &[u8] =
    b"dclutch/evaluator/capped-ramp-complement-v2";
/// SHA-256 identity of [`CAPPED_RAMP_EVALUATOR_RELEASE_PREIMAGE_V2`].
pub const CAPPED_RAMP_EVALUATOR_RELEASE_ID_V2: [u8; 32] = [
    0xb5, 0x79, 0x7b, 0x7e, 0x1a, 0xee, 0x61, 0x5b, 0x85, 0xd2, 0x5b, 0x2d, 0xf4, 0xe6, 0x2d, 0xe2,
    0x38, 0xc4, 0x5b, 0x8e, 0x30, 0x2c, 0x0b, 0xf3, 0x87, 0x09, 0x8d, 0x51, 0xca, 0xdc, 0xe2, 0x5e,
];
/// Exact categorical no-rounding boundary preimage.
pub const CATEGORICAL_EXACT_BOUNDARY_PREIMAGE_V2: &[u8] = b"dclutch/rounding/exact-no-rounding-v2";
/// SHA-256 identity of [`CATEGORICAL_EXACT_BOUNDARY_PREIMAGE_V2`].
pub const CATEGORICAL_EXACT_BOUNDARY_ID_V2: [u8; 32] = [
    0x10, 0xeb, 0x39, 0xdd, 0xaf, 0xe4, 0x7d, 0xdb, 0xf2, 0x90, 0xab, 0x65, 0x9f, 0x24, 0x01, 0x38,
    0x9d, 0xf4, 0x55, 0xe9, 0xb3, 0xa7, 0x98, 0x3a, 0x07, 0x4a, 0x61, 0x28, 0x1b, 0x2b, 0x33, 0xcd,
];
/// Sole capped-ramp/complement floor-boundary preimage.
pub const CAPPED_RAMP_COMPLEMENT_FLOOR_BOUNDARY_PREIMAGE_V2: &[u8] =
    b"dclutch/rounding/capped-ramp-complement-floor-v2";
/// SHA-256 identity of [`CAPPED_RAMP_COMPLEMENT_FLOOR_BOUNDARY_PREIMAGE_V2`].
pub const CAPPED_RAMP_COMPLEMENT_FLOOR_BOUNDARY_ID_V2: [u8; 32] = [
    0xdb, 0xc0, 0x28, 0x5d, 0xd2, 0x8a, 0xe1, 0x1c, 0xd2, 0xae, 0xcb, 0x84, 0xab, 0x48, 0x2d, 0xe5,
    0x03, 0x32, 0x81, 0x10, 0x4d, 0x92, 0xcc, 0x07, 0xd7, 0x09, 0x17, 0x04, 0xf0, 0x01, 0x40, 0x8e,
];

const KIND_OFFSET: usize = 12;
const CLAIM_COUNT_OFFSET: usize = 16;
const BODY_BYTES_OFFSET: usize = 20;
const SCALE_OFFSET: usize = 24;
const PRODUCT_INSTANCE_ID_OFFSET: usize = 32;
const EVALUATOR_RELEASE_ID_OFFSET: usize = 64;
const ROUNDING_BOUNDARY_ID_OFFSET: usize = 96;
const CATEGORICAL_KIND_V2: u8 = 1;
const CAPPED_RAMP_KIND_V2: u8 = 2;
const CAPPED_RAMP_BODY_BYTES_V2: u32 = 24;
const RAMP_KNOT_DENOMINATOR_OFFSET: usize = 128;
const RAMP_BODY_RESERVED_OFFSET: usize = 132;
const RAMP_LEFT_NUMERATOR_OFFSET: usize = 136;
const RAMP_RIGHT_NUMERATOR_OFFSET: usize = 144;

/// Refusal from LiabilityBasisV2 Product admission or Claims planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductClaimsErrorV2 {
    /// An input or output did not have its one exact width.
    InvalidLength,
    /// A record selected another record family.
    InvalidMagic,
    /// A record selected another semantic schema.
    UnsupportedSchema,
    /// A record selected another checked-integer physical profile.
    UnsupportedProfile,
    /// A record selected an unknown evaluator kind.
    UnknownEvaluator,
    /// Reserved bytes or kind-specific constants were noncanonical.
    NonCanonicalRecord,
    /// A required content identity used the all-zero sentinel.
    ZeroIdentifier,
    /// Authenticated basis or Product instance content did not match expectation.
    IdentityMismatch,
    /// A runtime claim basis had zero width.
    EmptyBasis,
    /// Caller state or candidate buffers did not have the admitted runtime width.
    WidthMismatch,
    /// A split, merge, or redemption quantity was zero.
    ZeroQuantity,
    /// A position balance exceeded aggregate outstanding supply.
    PositionExceedsSupply,
    /// A merge or redemption tried to burn unavailable claims or collateral.
    InsufficientBalance,
    /// Incoming Hoard collateral did not cover the applicable exact liability.
    Insolvent,
    /// A terminal result selected another evaluator or an out-of-range coordinate.
    InvalidTerminalResult,
    /// Checked exact integer arithmetic exceeded this physical profile.
    ArithmeticOverflow,
    /// The handwritten payout kernel refused an otherwise structured operation.
    Kernel(KernelError),
}

/// Result alias for Product-to-Claims LiabilityBasisV2 operations.
pub type ProductClaimsResultV2<T> = core::result::Result<T, ProductClaimsErrorV2>;

/// Validated nonzero opaque content identity.
///
/// Hash derivation and byte authentication remain adapter policy. Admission
/// only accepts the identity supplied by that boundary and checks exact links.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ContentIdV2([u8; CONTENT_ID_BYTES_V2]);

impl ContentIdV2 {
    /// Validate and construct a nonzero content identity.
    pub fn new(bytes: [u8; CONTENT_ID_BYTES_V2]) -> ProductClaimsResultV2<Self> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(ProductClaimsErrorV2::ZeroIdentifier);
        }
        Ok(Self(bytes))
    }

    /// Return the exact identity bytes.
    pub const fn to_bytes(self) -> [u8; CONTENT_ID_BYTES_V2] {
        self.0
    }

    /// Borrow the exact identity bytes.
    pub const fn as_bytes(&self) -> &[u8; CONTENT_ID_BYTES_V2] {
        &self.0
    }
}

/// Runtime evaluator selected by an admitted LiabilityBasisV2 record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BasisKindV2 {
    /// Arbitrary runtime width, scale one, one-hot terminal payout.
    CategoricalQ1,
    /// Exactly two claims using capped ramp and exact complement.
    CappedRampComplement,
}

/// Canonical categorical-Q=1 Product input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalBasisInputV2 {
    /// Product instance whose terminal semantics this basis consumes.
    pub product_instance_id: ContentIdV2,
    /// Runtime number of elementary claims and ordered terminal cells.
    pub claim_count: u32,
}

/// Canonical capped-ramp/complement Product input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CappedRampBasisInputV2 {
    /// Product instance whose terminal coordinate this basis consumes.
    pub product_instance_id: ContentIdV2,
    /// Exact positive common denominator of the two ramp knots.
    pub knot_denominator: u32,
    /// Exact signed numerator of the zero-payout knot.
    pub left_numerator: i64,
    /// Exact signed numerator of the full-payout knot.
    pub right_numerator: i64,
    /// Positive exact integer payout scale `Q`.
    pub scale: u64,
}

/// Encode one canonical categorical-Q=1 basis record atomically.
pub fn encode_categorical_basis_v2(
    input: CategoricalBasisInputV2,
    output: &mut [u8],
) -> ProductClaimsResultV2<()> {
    if output.len() != CATEGORICAL_BASIS_BYTES_V2 {
        return Err(ProductClaimsErrorV2::InvalidLength);
    }
    if input.claim_count == 0 {
        return Err(ProductClaimsErrorV2::EmptyBasis);
    }
    let mut candidate = [0_u8; CATEGORICAL_BASIS_BYTES_V2];
    put(&mut candidate, 0, &BASIS_MAGIC_V2)?;
    put(&mut candidate, 8, &BASIS_SCHEMA_V2.to_le_bytes())?;
    put(&mut candidate, 10, &BASIS_PHYSICAL_PROFILE_V2.to_le_bytes())?;
    put(&mut candidate, KIND_OFFSET, &[CATEGORICAL_KIND_V2])?;
    put(
        &mut candidate,
        CLAIM_COUNT_OFFSET,
        &input.claim_count.to_le_bytes(),
    )?;
    put(&mut candidate, SCALE_OFFSET, &1_u64.to_le_bytes())?;
    put(
        &mut candidate,
        PRODUCT_INSTANCE_ID_OFFSET,
        input.product_instance_id.as_bytes(),
    )?;
    put(
        &mut candidate,
        EVALUATOR_RELEASE_ID_OFFSET,
        &CATEGORICAL_EVALUATOR_RELEASE_ID_V2,
    )?;
    put(
        &mut candidate,
        ROUNDING_BOUNDARY_ID_OFFSET,
        &CATEGORICAL_EXACT_BOUNDARY_ID_V2,
    )?;
    output.copy_from_slice(&candidate);
    Ok(())
}

/// Encode one canonical two-claim capped-ramp/complement basis atomically.
pub fn encode_capped_ramp_basis_v2(
    input: CappedRampBasisInputV2,
    output: &mut [u8],
) -> ProductClaimsResultV2<()> {
    if output.len() != CAPPED_RAMP_BASIS_BYTES_V2 {
        return Err(ProductClaimsErrorV2::InvalidLength);
    }
    validate_ramp(input)?;
    let mut candidate = [0_u8; CAPPED_RAMP_BASIS_BYTES_V2];
    put(&mut candidate, 0, &BASIS_MAGIC_V2)?;
    put(&mut candidate, 8, &BASIS_SCHEMA_V2.to_le_bytes())?;
    put(&mut candidate, 10, &BASIS_PHYSICAL_PROFILE_V2.to_le_bytes())?;
    put(&mut candidate, KIND_OFFSET, &[CAPPED_RAMP_KIND_V2])?;
    put(&mut candidate, CLAIM_COUNT_OFFSET, &2_u32.to_le_bytes())?;
    put(
        &mut candidate,
        BODY_BYTES_OFFSET,
        &CAPPED_RAMP_BODY_BYTES_V2.to_le_bytes(),
    )?;
    put(&mut candidate, SCALE_OFFSET, &input.scale.to_le_bytes())?;
    put(
        &mut candidate,
        PRODUCT_INSTANCE_ID_OFFSET,
        input.product_instance_id.as_bytes(),
    )?;
    put(
        &mut candidate,
        EVALUATOR_RELEASE_ID_OFFSET,
        &CAPPED_RAMP_EVALUATOR_RELEASE_ID_V2,
    )?;
    put(
        &mut candidate,
        ROUNDING_BOUNDARY_ID_OFFSET,
        &CAPPED_RAMP_COMPLEMENT_FLOOR_BOUNDARY_ID_V2,
    )?;
    put(
        &mut candidate,
        RAMP_KNOT_DENOMINATOR_OFFSET,
        &input.knot_denominator.to_le_bytes(),
    )?;
    put(
        &mut candidate,
        RAMP_LEFT_NUMERATOR_OFFSET,
        &input.left_numerator.to_le_bytes(),
    )?;
    put(
        &mut candidate,
        RAMP_RIGHT_NUMERATOR_OFFSET,
        &input.right_numerator.to_le_bytes(),
    )?;
    output.copy_from_slice(&candidate);
    Ok(())
}

/// Adapter-authenticated and Product-linked executable liability basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedBasisV2 {
    basis_id: ContentIdV2,
    product_instance_id: ContentIdV2,
    kind: BasisKindV2,
    claim_count: u32,
    scale: u64,
    knot_denominator: u32,
    left_numerator: i64,
    right_numerator: i64,
}

impl AdmittedBasisV2 {
    /// Decode exact canonical bytes and require authenticated Product and basis links.
    ///
    /// `authenticated_basis_id` is the identity the adapter authenticated for
    /// `bytes`; `expected_basis_id` and `expected_product_instance_id` are the
    /// identities committed by Claims and Product respectively.
    pub fn admit(
        bytes: &[u8],
        authenticated_basis_id: ContentIdV2,
        expected_basis_id: ContentIdV2,
        expected_product_instance_id: ContentIdV2,
    ) -> ProductClaimsResultV2<Self> {
        if authenticated_basis_id != expected_basis_id {
            return Err(ProductClaimsErrorV2::IdentityMismatch);
        }
        let value = decode_basis(bytes, authenticated_basis_id)?;
        if value.product_instance_id != expected_product_instance_id {
            return Err(ProductClaimsErrorV2::IdentityMismatch);
        }
        Ok(value)
    }

    /// Return the authenticated basis content identity.
    pub const fn basis_id(self) -> ContentIdV2 {
        self.basis_id
    }

    /// Return the Product instance identity bound into the canonical record.
    pub const fn product_instance_id(self) -> ContentIdV2 {
        self.product_instance_id
    }

    /// Return the admitted evaluator kind.
    pub const fn kind(self) -> BasisKindV2 {
        self.kind
    }

    /// Return the runtime claim width.
    pub const fn claim_count(self) -> u32 {
        self.claim_count
    }

    /// Return the positive exact integer payout scale `Q`.
    pub const fn scale(self) -> u64 {
        self.scale
    }

    /// Evaluate the terminal payout partition into an exact-width caller buffer.
    ///
    /// The buffer is unchanged on every refusal.
    pub fn evaluate_terminal_into(
        self,
        result: TerminalResultV2,
        output: &mut [u64],
    ) -> ProductClaimsResultV2<()> {
        let width = self.runtime_width()?;
        if output.len() != width {
            return Err(ProductClaimsErrorV2::WidthMismatch);
        }
        match (self.kind, result) {
            (BasisKindV2::CategoricalQ1, TerminalResultV2::Categorical { winner }) => {
                let winner = usize::try_from(winner)
                    .map_err(|_| ProductClaimsErrorV2::InvalidTerminalResult)?;
                if winner >= width {
                    return Err(ProductClaimsErrorV2::InvalidTerminalResult);
                }
                output.fill(0);
                if let Some(winning_payout) = output.get_mut(winner) {
                    *winning_payout = 1;
                }
            }
            (
                BasisKindV2::CappedRampComplement,
                TerminalResultV2::RationalCoordinate {
                    numerator,
                    denominator,
                },
            ) => {
                let payout = self.evaluate_ramp(numerator, denominator)?;
                output.copy_from_slice(&payout);
            }
            _ => return Err(ProductClaimsErrorV2::InvalidTerminalResult),
        }
        Ok(())
    }

    /// Build complete split candidates after checking the exact worst-case liability.
    ///
    /// Both output buffers remain unchanged on refusal. For the two admitted
    /// evaluator families, `Q * max(supply)` is the exact maximum liability:
    /// categorical winners and the two ramp endpoints each attain a vertex.
    pub fn plan_split_into(
        self,
        aggregate_before: &[u64],
        position_before: &[u64],
        quantity: u64,
        hoard_before: u64,
        aggregate_after: &mut [u64],
        position_after: &mut [u64],
    ) -> ProductClaimsResultV2<ClaimsCandidateV2> {
        self.validate_state_buffers(
            aggregate_before,
            position_before,
            aggregate_after,
            position_after,
        )?;
        require_positive_quantity(quantity)?;
        let liability_before = self.maximum_pre_resolution_liability(aggregate_before)?;
        if liability_before > hoard_before {
            return Err(ProductClaimsErrorV2::Insolvent);
        }
        let collateral_in = quantity
            .checked_mul(self.scale)
            .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
        let hoard_after = hoard_before
            .checked_add(collateral_in)
            .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
        let mut maximum_after = 0_u64;
        for (supply, position) in aggregate_before.iter().zip(position_before) {
            let supply = supply
                .checked_add(quantity)
                .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
            position
                .checked_add(quantity)
                .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
            maximum_after = maximum_after.max(supply);
        }
        let liability_after = maximum_after
            .checked_mul(self.scale)
            .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
        if liability_after
            != liability_before
                .checked_add(collateral_in)
                .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?
            || liability_after > hoard_after
        {
            return Err(ProductClaimsErrorV2::Insolvent);
        }
        for (candidate, current) in aggregate_after.iter_mut().zip(aggregate_before) {
            *candidate = current.saturating_add(quantity);
        }
        for (candidate, current) in position_after.iter_mut().zip(position_before) {
            *candidate = current.saturating_add(quantity);
        }
        Ok(self.candidate(
            ClaimsOperationV2::Split,
            quantity,
            collateral_in,
            0,
            hoard_before,
            hoard_after,
            liability_before,
            liability_after,
        ))
    }

    /// Build complete merge candidates after checking balances and solvency.
    ///
    /// Both output buffers remain unchanged on refusal.
    pub fn plan_merge_into(
        self,
        aggregate_before: &[u64],
        position_before: &[u64],
        quantity: u64,
        hoard_before: u64,
        aggregate_after: &mut [u64],
        position_after: &mut [u64],
    ) -> ProductClaimsResultV2<ClaimsCandidateV2> {
        self.validate_state_buffers(
            aggregate_before,
            position_before,
            aggregate_after,
            position_after,
        )?;
        require_positive_quantity(quantity)?;
        let liability_before = self.maximum_pre_resolution_liability(aggregate_before)?;
        if liability_before > hoard_before {
            return Err(ProductClaimsErrorV2::Insolvent);
        }
        let collateral_out = quantity
            .checked_mul(self.scale)
            .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
        let hoard_after = hoard_before
            .checked_sub(collateral_out)
            .ok_or(ProductClaimsErrorV2::InsufficientBalance)?;
        let mut maximum_after = 0_u64;
        for (supply, position) in aggregate_before.iter().zip(position_before) {
            let supply = supply
                .checked_sub(quantity)
                .ok_or(ProductClaimsErrorV2::InsufficientBalance)?;
            position
                .checked_sub(quantity)
                .ok_or(ProductClaimsErrorV2::InsufficientBalance)?;
            maximum_after = maximum_after.max(supply);
        }
        let liability_after = maximum_after
            .checked_mul(self.scale)
            .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
        if liability_before
            .checked_sub(collateral_out)
            .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?
            != liability_after
            || liability_after > hoard_after
        {
            return Err(ProductClaimsErrorV2::Insolvent);
        }
        for (candidate, current) in aggregate_after.iter_mut().zip(aggregate_before) {
            *candidate = current.saturating_sub(quantity);
        }
        for (candidate, current) in position_after.iter_mut().zip(position_before) {
            *candidate = current.saturating_sub(quantity);
        }
        Ok(self.candidate(
            ClaimsOperationV2::Merge,
            quantity,
            0,
            collateral_out,
            hoard_before,
            hoard_after,
            liability_before,
            liability_after,
        ))
    }

    /// Build complete terminal-redemption candidates at one admitted result.
    ///
    /// Terminal solvency uses the exact evaluated dot product, not the
    /// pre-resolution vertex bound. Both buffers remain unchanged on refusal.
    #[allow(clippy::too_many_arguments)]
    pub fn plan_terminal_redeem_into(
        self,
        result: TerminalResultV2,
        claim_index: u32,
        aggregate_before: &[u64],
        position_before: &[u64],
        quantity: u64,
        hoard_before: u64,
        aggregate_after: &mut [u64],
        position_after: &mut [u64],
    ) -> ProductClaimsResultV2<ClaimsCandidateV2> {
        let width = self.validate_state_buffers(
            aggregate_before,
            position_before,
            aggregate_after,
            position_after,
        )?;
        require_positive_quantity(quantity)?;
        let claim_index = usize::try_from(claim_index)
            .map_err(|_| ProductClaimsErrorV2::InvalidTerminalResult)?;
        if claim_index >= width {
            return Err(ProductClaimsErrorV2::InvalidTerminalResult);
        }
        let liability_before = self.terminal_liability(result, aggregate_before)?;
        if liability_before > hoard_before {
            return Err(ProductClaimsErrorV2::Insolvent);
        }
        let payout_per_claim = self.payout_at(result, claim_index)?;
        let collateral_out = quantity
            .checked_mul(payout_per_claim)
            .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
        let candidate_supply = aggregate_before
            .get(claim_index)
            .copied()
            .ok_or(ProductClaimsErrorV2::InvalidTerminalResult)?
            .checked_sub(quantity)
            .ok_or(ProductClaimsErrorV2::InsufficientBalance)?;
        let candidate_position = position_before
            .get(claim_index)
            .copied()
            .ok_or(ProductClaimsErrorV2::InvalidTerminalResult)?
            .checked_sub(quantity)
            .ok_or(ProductClaimsErrorV2::InsufficientBalance)?;
        let hoard_after = hoard_before
            .checked_sub(collateral_out)
            .ok_or(ProductClaimsErrorV2::InsufficientBalance)?;
        let liability_after = liability_before
            .checked_sub(collateral_out)
            .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
        if liability_after > hoard_after {
            return Err(ProductClaimsErrorV2::Insolvent);
        }
        aggregate_after.copy_from_slice(aggregate_before);
        position_after.copy_from_slice(position_before);
        for (index, (aggregate, position)) in aggregate_after
            .iter_mut()
            .zip(position_after.iter_mut())
            .enumerate()
        {
            if index == claim_index {
                *aggregate = candidate_supply;
                *position = candidate_position;
            }
        }
        Ok(self.candidate(
            ClaimsOperationV2::TerminalRedeem,
            quantity,
            0,
            collateral_out,
            hoard_before,
            hoard_after,
            liability_before,
            liability_after,
        ))
    }

    fn runtime_width(self) -> ProductClaimsResultV2<usize> {
        usize::try_from(self.claim_count).map_err(|_| ProductClaimsErrorV2::WidthMismatch)
    }

    fn validate_state_buffers(
        self,
        aggregate_before: &[u64],
        position_before: &[u64],
        aggregate_after: &[u64],
        position_after: &[u64],
    ) -> ProductClaimsResultV2<usize> {
        let width = self.runtime_width()?;
        if aggregate_before.len() != width
            || position_before.len() != width
            || aggregate_after.len() != width
            || position_after.len() != width
        {
            return Err(ProductClaimsErrorV2::WidthMismatch);
        }
        for (position, aggregate) in position_before.iter().zip(aggregate_before) {
            if position > aggregate {
                return Err(ProductClaimsErrorV2::PositionExceedsSupply);
            }
        }
        Ok(width)
    }

    fn maximum_pre_resolution_liability(self, supplies: &[u64]) -> ProductClaimsResultV2<u64> {
        let maximum = supplies
            .iter()
            .copied()
            .max()
            .ok_or(ProductClaimsErrorV2::EmptyBasis)?;
        maximum
            .checked_mul(self.scale)
            .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)
    }

    fn terminal_liability(
        self,
        result: TerminalResultV2,
        supplies: &[u64],
    ) -> ProductClaimsResultV2<u64> {
        let mut total = 0_u128;
        for (index, supply) in supplies.iter().copied().enumerate() {
            let payout = self.payout_at(result, index)?;
            let term = u128::from(supply)
                .checked_mul(u128::from(payout))
                .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
            total = total
                .checked_add(term)
                .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
        }
        u64::try_from(total).map_err(|_| ProductClaimsErrorV2::ArithmeticOverflow)
    }

    fn payout_at(self, result: TerminalResultV2, claim_index: usize) -> ProductClaimsResultV2<u64> {
        match (self.kind, result) {
            (BasisKindV2::CategoricalQ1, TerminalResultV2::Categorical { winner }) => {
                let winner = usize::try_from(winner)
                    .map_err(|_| ProductClaimsErrorV2::InvalidTerminalResult)?;
                let width = self.runtime_width()?;
                if winner >= width || claim_index >= width {
                    return Err(ProductClaimsErrorV2::InvalidTerminalResult);
                }
                Ok(u64::from(winner == claim_index))
            }
            (
                BasisKindV2::CappedRampComplement,
                TerminalResultV2::RationalCoordinate {
                    numerator,
                    denominator,
                },
            ) => self
                .evaluate_ramp(numerator, denominator)?
                .get(claim_index)
                .copied()
                .ok_or(ProductClaimsErrorV2::InvalidTerminalResult),
            _ => Err(ProductClaimsErrorV2::InvalidTerminalResult),
        }
    }

    fn evaluate_ramp(
        self,
        coordinate_numerator: i64,
        coordinate_denominator: u32,
    ) -> ProductClaimsResultV2<[u64; 2]> {
        if coordinate_denominator == 0 {
            return Err(ProductClaimsErrorV2::InvalidTerminalResult);
        }
        let observed = i128::from(coordinate_numerator)
            .checked_mul(i128::from(self.knot_denominator))
            .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
        let left = i128::from(self.left_numerator)
            .checked_mul(i128::from(coordinate_denominator))
            .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
        let right = i128::from(self.right_numerator)
            .checked_mul(i128::from(coordinate_denominator))
            .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
        let primary = if observed <= left {
            0
        } else if right <= observed {
            self.scale
        } else {
            capped_ramp_complement_floor_boundary_v2(self.scale, observed - left, right - left)
                .map_err(ProductClaimsErrorV2::Kernel)?
        };
        let complement = self
            .scale
            .checked_sub(primary)
            .ok_or(ProductClaimsErrorV2::ArithmeticOverflow)?;
        Ok([primary, complement])
    }

    #[allow(clippy::too_many_arguments)]
    const fn candidate(
        self,
        operation: ClaimsOperationV2,
        quantity: u64,
        collateral_in: u64,
        collateral_out: u64,
        hoard_before: u64,
        hoard_after: u64,
        liability_before: u64,
        liability_after: u64,
    ) -> ClaimsCandidateV2 {
        ClaimsCandidateV2 {
            basis_id: self.basis_id,
            product_instance_id: self.product_instance_id,
            operation,
            claim_count: self.claim_count,
            quantity,
            collateral_in,
            collateral_out,
            hoard_before,
            hoard_after,
            liability_before,
            liability_after,
        }
    }
}

/// Terminal observation supplied after Product resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalResultV2 {
    /// Canonically ordered categorical terminal cell.
    Categorical {
        /// Zero-based winner coordinate.
        winner: u32,
    },
    /// Exact signed rational coordinate consumed by the capped ramp.
    RationalCoordinate {
        /// Signed coordinate numerator.
        numerator: i64,
        /// Positive coordinate denominator.
        denominator: u32,
    },
}

/// Pure Claims transition represented by complete caller-owned candidate buffers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsCandidateV2 {
    basis_id: ContentIdV2,
    product_instance_id: ContentIdV2,
    operation: ClaimsOperationV2,
    claim_count: u32,
    quantity: u64,
    collateral_in: u64,
    collateral_out: u64,
    hoard_before: u64,
    hoard_after: u64,
    liability_before: u64,
    liability_after: u64,
}

impl ClaimsCandidateV2 {
    /// Return the authenticated basis identity.
    pub const fn basis_id(self) -> ContentIdV2 {
        self.basis_id
    }

    /// Return the Product instance identity bound by admission.
    pub const fn product_instance_id(self) -> ContentIdV2 {
        self.product_instance_id
    }

    /// Return the planned Claims operation.
    pub const fn operation(self) -> ClaimsOperationV2 {
        self.operation
    }

    /// Return the exact runtime candidate width.
    pub const fn claim_count(self) -> u32 {
        self.claim_count
    }

    /// Return the exact split, merge, or redemption quantity.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }

    /// Return collateral atoms entering the Hoard.
    pub const fn collateral_in(self) -> u64 {
        self.collateral_in
    }

    /// Return collateral atoms leaving the Hoard.
    pub const fn collateral_out(self) -> u64 {
        self.collateral_out
    }

    /// Return incoming Hoard collateral atoms.
    pub const fn hoard_before(self) -> u64 {
        self.hoard_before
    }

    /// Return candidate Hoard collateral atoms.
    pub const fn hoard_after(self) -> u64 {
        self.hoard_after
    }

    /// Return incoming applicable liability atoms.
    pub const fn liability_before(self) -> u64 {
        self.liability_before
    }

    /// Return candidate applicable liability atoms.
    pub const fn liability_after(self) -> u64 {
        self.liability_after
    }
}

/// Operation named by a [`ClaimsCandidateV2`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimsOperationV2 {
    /// Deposit `quantity * Q` and mint every elementary claim.
    Split,
    /// Burn every elementary claim and return `quantity * Q`.
    Merge,
    /// Burn one terminal claim and return its exact evaluated payout.
    TerminalRedeem,
}

fn decode_basis(bytes: &[u8], basis_id: ContentIdV2) -> ProductClaimsResultV2<AdmittedBasisV2> {
    if bytes.len() != CATEGORICAL_BASIS_BYTES_V2 && bytes.len() != CAPPED_RAMP_BASIS_BYTES_V2 {
        return Err(ProductClaimsErrorV2::InvalidLength);
    }
    if read_array::<8>(bytes, 0)? != BASIS_MAGIC_V2 {
        return Err(ProductClaimsErrorV2::InvalidMagic);
    }
    if read_u16(bytes, 8)? != BASIS_SCHEMA_V2 {
        return Err(ProductClaimsErrorV2::UnsupportedSchema);
    }
    if read_u16(bytes, 10)? != BASIS_PHYSICAL_PROFILE_V2 {
        return Err(ProductClaimsErrorV2::UnsupportedProfile);
    }
    require_zero(bytes, 13, 3)?;
    let claim_count = read_u32(bytes, CLAIM_COUNT_OFFSET)?;
    if claim_count == 0 {
        return Err(ProductClaimsErrorV2::EmptyBasis);
    }
    let scale = read_u64(bytes, SCALE_OFFSET)?;
    if scale == 0 {
        return Err(ProductClaimsErrorV2::NonCanonicalRecord);
    }
    let product_instance_id = ContentIdV2::new(read_array(bytes, PRODUCT_INSTANCE_ID_OFFSET)?)?;
    let evaluator_release_id = read_array::<32>(bytes, EVALUATOR_RELEASE_ID_OFFSET)?;
    let rounding_boundary_id = read_array::<32>(bytes, ROUNDING_BOUNDARY_ID_OFFSET)?;
    let body_bytes = read_u32(bytes, BODY_BYTES_OFFSET)?;
    let evaluator_kind = bytes
        .get(KIND_OFFSET)
        .copied()
        .ok_or(ProductClaimsErrorV2::InvalidLength)?;
    let (kind, knot_denominator, left_numerator, right_numerator) = match evaluator_kind {
        CATEGORICAL_KIND_V2 => {
            if bytes.len() != CATEGORICAL_BASIS_BYTES_V2
                || body_bytes != 0
                || scale != 1
                || evaluator_release_id != CATEGORICAL_EVALUATOR_RELEASE_ID_V2
                || rounding_boundary_id != CATEGORICAL_EXACT_BOUNDARY_ID_V2
            {
                return Err(ProductClaimsErrorV2::NonCanonicalRecord);
            }
            (BasisKindV2::CategoricalQ1, 0, 0, 0)
        }
        CAPPED_RAMP_KIND_V2 => {
            if bytes.len() != CAPPED_RAMP_BASIS_BYTES_V2
                || claim_count != 2
                || body_bytes != CAPPED_RAMP_BODY_BYTES_V2
                || evaluator_release_id != CAPPED_RAMP_EVALUATOR_RELEASE_ID_V2
                || rounding_boundary_id != CAPPED_RAMP_COMPLEMENT_FLOOR_BOUNDARY_ID_V2
            {
                return Err(ProductClaimsErrorV2::NonCanonicalRecord);
            }
            require_zero(bytes, RAMP_BODY_RESERVED_OFFSET, 4)?;
            let input = CappedRampBasisInputV2 {
                product_instance_id,
                knot_denominator: read_u32(bytes, RAMP_KNOT_DENOMINATOR_OFFSET)?,
                left_numerator: read_i64(bytes, RAMP_LEFT_NUMERATOR_OFFSET)?,
                right_numerator: read_i64(bytes, RAMP_RIGHT_NUMERATOR_OFFSET)?,
                scale,
            };
            validate_ramp(input)?;
            (
                BasisKindV2::CappedRampComplement,
                input.knot_denominator,
                input.left_numerator,
                input.right_numerator,
            )
        }
        _ => return Err(ProductClaimsErrorV2::UnknownEvaluator),
    };
    Ok(AdmittedBasisV2 {
        basis_id,
        product_instance_id,
        kind,
        claim_count,
        scale,
        knot_denominator,
        left_numerator,
        right_numerator,
    })
}

fn validate_ramp(input: CappedRampBasisInputV2) -> ProductClaimsResultV2<()> {
    if input.scale == 0 || input.knot_denominator == 0 {
        return Err(ProductClaimsErrorV2::NonCanonicalRecord);
    }
    if input.left_numerator >= input.right_numerator {
        return Err(ProductClaimsErrorV2::NonCanonicalRecord);
    }
    Ok(())
}

fn require_positive_quantity(quantity: u64) -> ProductClaimsResultV2<()> {
    if quantity == 0 {
        return Err(ProductClaimsErrorV2::ZeroQuantity);
    }
    Ok(())
}

fn read_u16(input: &[u8], offset: usize) -> ProductClaimsResultV2<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> ProductClaimsResultV2<u32> {
    Ok(u32::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> ProductClaimsResultV2<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_i64(input: &[u8], offset: usize) -> ProductClaimsResultV2<i64> {
    Ok(i64::from_le_bytes(read_array(input, offset)?))
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> ProductClaimsResultV2<[u8; N]> {
    let end = offset
        .checked_add(N)
        .ok_or(ProductClaimsErrorV2::InvalidLength)?;
    input
        .get(offset..end)
        .ok_or(ProductClaimsErrorV2::InvalidLength)?
        .try_into()
        .map_err(|_| ProductClaimsErrorV2::InvalidLength)
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> ProductClaimsResultV2<()> {
    let end = offset
        .checked_add(width)
        .ok_or(ProductClaimsErrorV2::InvalidLength)?;
    if input
        .get(offset..end)
        .ok_or(ProductClaimsErrorV2::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(ProductClaimsErrorV2::NonCanonicalRecord);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> ProductClaimsResultV2<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(ProductClaimsErrorV2::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(ProductClaimsErrorV2::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}
