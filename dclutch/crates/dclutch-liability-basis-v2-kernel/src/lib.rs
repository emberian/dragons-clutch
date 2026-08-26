#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Runtime-width kernel for nonnegative integer partitions of unity.
//!
//! The semantic theorems and the sole capped-ramp apportionment boundary live
//! in `DClutchSemantics.LiabilityBasisV2`. This crate is an independent,
//! handwritten physical implementation checked against Lean-emitted cases.

use core::convert::TryInto;

/// Content-bound Product admission and pure Claims transition candidates.
pub mod product_claims;

#[rustfmt::skip]
#[allow(missing_docs)]
mod generated;

pub use generated::{
    AGREEMENT_CASES_V2, RAMP_COORDINATE_DENOMINATOR_OFFSET_V2, RAMP_COORDINATE_NUMERATOR_OFFSET_V2,
    RAMP_KNOT_DENOMINATOR_OFFSET_V2, RAMP_LEFT_NUMERATOR_OFFSET_V2, RAMP_MAGIC_OFFSET_V2,
    RAMP_MAGIC_V2, RAMP_PROFILE_OFFSET_V2, RAMP_PROFILE_V2, RAMP_REQUEST_BYTES_V2,
    RAMP_RESERVED_BYTES_V2, RAMP_RESERVED_OFFSET_V2, RAMP_RIGHT_NUMERATOR_OFFSET_V2,
    RAMP_SCALE_OFFSET_V2, RAMP_SCHEMA_VERSION_V2, RAMP_VERSION_OFFSET_V2, REFUSAL_CASES_V2,
};

/// One Lean-emitted accepted request and its exact two-claim payout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgreementCaseV2 {
    /// Exact canonical request bytes.
    pub request: [u8; RAMP_REQUEST_BYTES_V2],
    /// Exact primary and complement payouts.
    pub expected: [u64; 2],
}

/// One Lean-emitted hostile request and the stable expected refusal tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RefusalCaseV2 {
    /// Exact hostile request bytes.
    pub request: [u8; RAMP_REQUEST_BYTES_V2],
    /// Stable [`Error::tag`] value.
    pub error_tag: u8,
}

/// Stable refusal from hostile decoding or checked economic arithmetic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input did not have the sole exact physical width.
    InvalidLength,
    /// Magic selected another record family.
    InvalidMagic,
    /// Schema selected another semantic layout.
    UnsupportedSchema,
    /// Profile selected another physical integer envelope.
    UnsupportedProfile,
    /// Reserved bytes were not all zero.
    NonCanonicalReserved,
    /// A payout scale was zero.
    ZeroScale,
    /// A rational denominator was zero.
    ZeroDenominator,
    /// The two exact rational knots were not strictly ordered.
    UnorderedKnots,
    /// A runtime basis had no claim coordinate.
    EmptyBasis,
    /// Supply and payout vectors had different runtime widths.
    WidthMismatch,
    /// Payouts were not a nonnegative integer partition of the named scale.
    NonPartition,
    /// Checked physical arithmetic exceeded its `u64`, `u128`, or `i128` profile.
    ArithmeticOverflow,
    /// Incoming Hoard collateral did not cover the evaluated liability.
    Insolvent,
    /// A categorical winner or claim coordinate was outside the runtime width.
    OutcomeOutOfRange,
}

impl Error {
    /// Stable generated-corpus tag. Tags one through seven are ABI refusals.
    pub const fn tag(self) -> u8 {
        match self {
            Self::InvalidLength => 0,
            Self::InvalidMagic => 1,
            Self::UnsupportedSchema => 2,
            Self::UnsupportedProfile => 3,
            Self::NonCanonicalReserved => 4,
            Self::ZeroScale => 5,
            Self::ZeroDenominator => 6,
            Self::UnorderedKnots => 7,
            Self::EmptyBasis => 8,
            Self::WidthMismatch => 9,
            Self::NonPartition => 10,
            Self::ArithmeticOverflow => 11,
            Self::Insolvent => 12,
            Self::OutcomeOutOfRange => 13,
        }
    }
}

/// Result alias for this kernel.
pub type Result<T> = core::result::Result<T, Error>;

/// Hostile-decoded provisional two-claim capped-ramp request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RampRequestV2 {
    scale: u32,
    knot_denominator: u32,
    left_numerator: i64,
    right_numerator: i64,
    coordinate_numerator: i64,
    coordinate_denominator: u32,
}

impl RampRequestV2 {
    /// Decode exactly one canonical request and validate all structural facts.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != RAMP_REQUEST_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if bytes::<8>(input, RAMP_MAGIC_OFFSET_V2)? != RAMP_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(input, RAMP_VERSION_OFFSET_V2)? != RAMP_SCHEMA_VERSION_V2 {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(input, RAMP_PROFILE_OFFSET_V2)? != RAMP_PROFILE_V2 {
            return Err(Error::UnsupportedProfile);
        }
        if slice(input, RAMP_RESERVED_OFFSET_V2, RAMP_RESERVED_BYTES_V2)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalReserved);
        }
        let value = Self {
            scale: read_u32(input, RAMP_SCALE_OFFSET_V2)?,
            knot_denominator: read_u32(input, RAMP_KNOT_DENOMINATOR_OFFSET_V2)?,
            left_numerator: read_i64(input, RAMP_LEFT_NUMERATOR_OFFSET_V2)?,
            right_numerator: read_i64(input, RAMP_RIGHT_NUMERATOR_OFFSET_V2)?,
            coordinate_numerator: read_i64(input, RAMP_COORDINATE_NUMERATOR_OFFSET_V2)?,
            coordinate_denominator: read_u32(input, RAMP_COORDINATE_DENOMINATOR_OFFSET_V2)?,
        };
        if value.scale == 0 {
            return Err(Error::ZeroScale);
        }
        if value.knot_denominator == 0 || value.coordinate_denominator == 0 {
            return Err(Error::ZeroDenominator);
        }
        if value.left_numerator >= value.right_numerator {
            return Err(Error::UnorderedKnots);
        }
        Ok(value)
    }

    /// Evaluate the exact signed-rational coordinate into primary and exact
    /// complement payouts.
    pub fn evaluate(self) -> Result<[u64; 2]> {
        let observed = i128::from(self.coordinate_numerator)
            .checked_mul(i128::from(self.knot_denominator))
            .ok_or(Error::ArithmeticOverflow)?;
        let left = i128::from(self.left_numerator)
            .checked_mul(i128::from(self.coordinate_denominator))
            .ok_or(Error::ArithmeticOverflow)?;
        let right = i128::from(self.right_numerator)
            .checked_mul(i128::from(self.coordinate_denominator))
            .ok_or(Error::ArithmeticOverflow)?;
        let scale = u64::from(self.scale);
        let primary = if observed <= left {
            0
        } else if right <= observed {
            scale
        } else {
            capped_ramp_complement_floor_boundary_v2(scale, observed - left, right - left)?
        };
        let complement = scale
            .checked_sub(primary)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok([primary, complement])
    }

    /// Return the positive integer payout scale.
    pub const fn scale(self) -> u32 {
        self.scale
    }
}

/// The sole physical capped-ramp apportionment boundary: floor the final
/// positive rational interpolation, and never an intermediate coordinate.
pub fn capped_ramp_complement_floor_boundary_v2(
    scale: u64,
    elapsed: i128,
    width: i128,
) -> Result<u64> {
    if scale == 0 {
        return Err(Error::ZeroScale);
    }
    if elapsed <= 0 || width <= 0 || elapsed >= width {
        return Err(Error::ArithmeticOverflow);
    }
    let elapsed = u128::try_from(elapsed).map_err(|_| Error::ArithmeticOverflow)?;
    let width = u128::try_from(width).map_err(|_| Error::ArithmeticOverflow)?;
    let numerator = u128::from(scale)
        .checked_mul(elapsed)
        .ok_or(Error::ArithmeticOverflow)?;
    let payout = numerator / width;
    u64::try_from(payout).map_err(|_| Error::ArithmeticOverflow)
}

/// Require a nonempty runtime-width payout vector to be an exact nonnegative
/// integer partition of the positive scale.
pub fn validate_partition(payouts: &[u64], scale: u64) -> Result<()> {
    if payouts.is_empty() {
        return Err(Error::EmptyBasis);
    }
    if scale == 0 {
        return Err(Error::ZeroScale);
    }
    let mut sum = 0_u64;
    for payout in payouts {
        if *payout > scale {
            return Err(Error::NonPartition);
        }
        sum = sum.checked_add(*payout).ok_or(Error::ArithmeticOverflow)?;
    }
    if sum != scale {
        return Err(Error::NonPartition);
    }
    Ok(())
}

/// Evaluate one runtime-width liability dot product in collateral atoms.
pub fn liability(supplies: &[u64], payouts: &[u64]) -> Result<u64> {
    require_same_nonempty_width(supplies, payouts)?;
    let mut total = 0_u128;
    for (supply, payout) in supplies.iter().zip(payouts) {
        let term = u128::from(*supply)
            .checked_mul(u128::from(*payout))
            .ok_or(Error::ArithmeticOverflow)?;
        total = total.checked_add(term).ok_or(Error::ArithmeticOverflow)?;
    }
    u64::try_from(total).map_err(|_| Error::ArithmeticOverflow)
}

/// Pure checked complete-set split plan. The caller may stage each candidate
/// supply with [`SplitPlanV2::candidate_supply`] and commit only after its
/// separately authenticated physical effects succeed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplitPlanV2 {
    quantity: u64,
    collateral_delta: u64,
    hoard_before: u64,
    hoard_after: u64,
    liability_before: u64,
    liability_after: u64,
}

impl SplitPlanV2 {
    /// Return the split quantity added to every elementary supply.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }

    /// Return the exact `quantity * Q` collateral increase.
    pub const fn collateral_delta(self) -> u64 {
        self.collateral_delta
    }

    /// Return incoming Hoard collateral.
    pub const fn hoard_before(self) -> u64 {
        self.hoard_before
    }

    /// Return candidate Hoard collateral.
    pub const fn hoard_after(self) -> u64 {
        self.hoard_after
    }

    /// Return incoming liability at the evaluated result.
    pub const fn liability_before(self) -> u64 {
        self.liability_before
    }

    /// Return candidate liability at the evaluated result.
    pub const fn liability_after(self) -> u64 {
        self.liability_after
    }

    /// Derive one candidate supply without mutating caller state.
    pub fn candidate_supply(self, current: u64) -> Result<u64> {
        current
            .checked_add(self.quantity)
            .ok_or(Error::ArithmeticOverflow)
    }
}

/// Check an exact complete-set split and derive its immutable candidate plan.
pub fn plan_complete_set_split(
    supplies: &[u64],
    payouts: &[u64],
    scale: u64,
    quantity: u64,
    hoard: u64,
) -> Result<SplitPlanV2> {
    require_same_nonempty_width(supplies, payouts)?;
    validate_partition(payouts, scale)?;
    let liability_before = liability(supplies, payouts)?;
    if liability_before > hoard {
        return Err(Error::Insolvent);
    }
    let collateral_delta = quantity
        .checked_mul(scale)
        .ok_or(Error::ArithmeticOverflow)?;
    let hoard_after = hoard
        .checked_add(collateral_delta)
        .ok_or(Error::ArithmeticOverflow)?;
    let liability_after = liability_after_split(supplies, payouts, quantity)?;
    if liability_after
        != liability_before
            .checked_add(collateral_delta)
            .ok_or(Error::ArithmeticOverflow)?
        || liability_after > hoard_after
    {
        return Err(Error::NonPartition);
    }
    for supply in supplies {
        supply
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(SplitPlanV2 {
        quantity,
        collateral_delta,
        hoard_before: hoard,
        hoard_after,
        liability_before,
        liability_after,
    })
}

/// Evaluate categorical `Q=1` one-hot payout at a runtime-width coordinate.
pub fn categorical_payout_at(width: usize, winner: usize, claim: usize) -> Result<u64> {
    if width == 0 {
        return Err(Error::EmptyBasis);
    }
    if winner >= width || claim >= width {
        return Err(Error::OutcomeOutOfRange);
    }
    Ok(u64::from(winner == claim))
}

fn liability_after_split(supplies: &[u64], payouts: &[u64], quantity: u64) -> Result<u64> {
    let mut total = 0_u128;
    for (supply, payout) in supplies.iter().zip(payouts) {
        let candidate = supply
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        let term = u128::from(candidate)
            .checked_mul(u128::from(*payout))
            .ok_or(Error::ArithmeticOverflow)?;
        total = total.checked_add(term).ok_or(Error::ArithmeticOverflow)?;
    }
    u64::try_from(total).map_err(|_| Error::ArithmeticOverflow)
}

fn require_same_nonempty_width(left: &[u64], right: &[u64]) -> Result<()> {
    if left.is_empty() || right.is_empty() {
        return Err(Error::EmptyBasis);
    }
    if left.len() != right.len() {
        return Err(Error::WidthMismatch);
    }
    Ok(())
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(bytes(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(bytes(input, offset)?))
}

fn read_i64(input: &[u8], offset: usize) -> Result<i64> {
    Ok(i64::from_le_bytes(bytes(input, offset)?))
}

fn bytes<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    slice(input, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    input.get(offset..end).ok_or(Error::InvalidLength)
}
