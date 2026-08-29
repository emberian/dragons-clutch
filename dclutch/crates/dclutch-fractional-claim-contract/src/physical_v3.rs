//! Exact selected-coordinate Fractional physical topology.
//!
//! Fractional V3 deliberately does not adopt Rational's receipt Mint or
//! coefficient vector. The immutable Fractional terms remain the only owner of
//! `K` shard Mints and the exact denominator; the allocation kernel remains the
//! only quotient/remainder boundary.

use dclutch_fractional_claim_kernel::{FractionalExposureTermsV2, divide_exposure_shards_v2};

use crate::{FractionalExposureActionV2, FractionalExposureRequestV2};

/// Exact child route required by one selected action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalChildRouteV3 {
    /// One Claims child call atomically mutates native Claims and invokes Token-2022.
    ClaimsTokenAtomic,
    /// Holder-signed ordinary Token-2022 `TransferChecked`; no protocol mutation.
    Token2022DirectTransfer,
    /// One Claims terminal call atomically evaluates a zero payout and burns shards.
    ClaimsTerminalTokenAtomic,
    /// One Claims terminal call atomically burns shards and invokes Custody.
    ClaimsTerminalTokenCustodyAtomic,
    /// Permissionless state-only terminal binding in the Fractional caller.
    FractionalStateOnly,
}

/// Exact authority required by the outer transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalSignerRoleV3 {
    /// Request owner/holder signs the action.
    Holder,
    /// No family authority signs; the transaction fee payer is not protocol authority.
    Permissionless,
}

/// Exact arithmetic and route topology for one bounded selected-coordinate action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalPhysicalPlanV3 {
    /// Selected action.
    pub action: FractionalExposureActionV2,
    /// Exact selected representation coordinate, or the canonical absent value.
    pub representation_coordinate: u32,
    /// Terms-owned shard Mint, absent only for state-only terminalization.
    pub shard_mint: Option<[u8; 32]>,
    /// Whole Claims coordinate quantity locked, unlocked, or redeemed.
    pub whole_claims: u64,
    /// Raw shard atoms minted, transferred, or burned.
    pub consumed_shards: u64,
    /// Raw same-Mint remainder that stays in the source Token account.
    pub change_shards: u64,
    /// Exact atomic child topology.
    pub route: FractionalChildRouteV3,
    /// Exact outer authority role.
    pub signer: FractionalSignerRoleV3,
}

/// Derive selected-coordinate physical arithmetic without a second denominator owner.
pub fn plan_fractional_physical_v3(
    terms: FractionalExposureTermsV2<'_>,
    request: FractionalExposureRequestV2,
) -> Result<FractionalPhysicalPlanV3> {
    let request = request
        .bind_terms(terms)
        .map_err(|_| FractionalPhysicalErrorV3::IdentityMismatch)?;
    let input = request.input();
    let action = request.action();
    match action {
        FractionalExposureActionV2::Wrap => {
            let consumed_shards = input
                .quantity
                .checked_mul(terms.denominator())
                .ok_or(FractionalPhysicalErrorV3::Arithmetic)?;
            Ok(FractionalPhysicalPlanV3 {
                action,
                representation_coordinate: input.representation_coordinate,
                shard_mint: Some(
                    terms
                        .shard_mint(input.representation_coordinate)
                        .map_err(|_| FractionalPhysicalErrorV3::IdentityMismatch)?,
                ),
                whole_claims: input.quantity,
                consumed_shards,
                change_shards: 0,
                route: FractionalChildRouteV3::ClaimsTokenAtomic,
                signer: FractionalSignerRoleV3::Holder,
            })
        }
        FractionalExposureActionV2::Transfer => Ok(FractionalPhysicalPlanV3 {
            action,
            representation_coordinate: input.representation_coordinate,
            shard_mint: Some(
                terms
                    .shard_mint(input.representation_coordinate)
                    .map_err(|_| FractionalPhysicalErrorV3::IdentityMismatch)?,
            ),
            whole_claims: 0,
            consumed_shards: input.quantity,
            change_shards: 0,
            route: FractionalChildRouteV3::Token2022DirectTransfer,
            signer: FractionalSignerRoleV3::Holder,
        }),
        FractionalExposureActionV2::WholeUnwrap
        | FractionalExposureActionV2::TerminalRedeem
        | FractionalExposureActionV2::TerminalZeroBurn => {
            let division =
                divide_exposure_shards_v2(terms, input.representation_coordinate, input.quantity)
                    .map_err(|_| FractionalPhysicalErrorV3::Division)?;
            Ok(FractionalPhysicalPlanV3 {
                action,
                representation_coordinate: input.representation_coordinate,
                shard_mint: Some(division.input.shard_mint),
                whole_claims: division.whole_claims,
                consumed_shards: division.consumed.shard_atoms,
                change_shards: division.change.shard_atoms,
                route: if action == FractionalExposureActionV2::TerminalRedeem {
                    FractionalChildRouteV3::ClaimsTerminalTokenCustodyAtomic
                } else if action == FractionalExposureActionV2::TerminalZeroBurn {
                    FractionalChildRouteV3::ClaimsTerminalTokenAtomic
                } else {
                    FractionalChildRouteV3::ClaimsTokenAtomic
                },
                signer: FractionalSignerRoleV3::Holder,
            })
        }
        FractionalExposureActionV2::Terminalize => Ok(FractionalPhysicalPlanV3 {
            action,
            representation_coordinate: input.representation_coordinate,
            shard_mint: None,
            whole_claims: 0,
            consumed_shards: 0,
            change_shards: 0,
            route: FractionalChildRouteV3::FractionalStateOnly,
            signer: FractionalSignerRoleV3::Permissionless,
        }),
        FractionalExposureActionV2::ZeroSupplyRetire => {
            Err(FractionalPhysicalErrorV3::UseOrderedRetirement)
        }
    }
}

/// Stable selected-topology refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalPhysicalErrorV3 {
    /// Request identities differed from authenticated Fractional terms.
    IdentityMismatch,
    /// The sole kernel quotient/remainder boundary refused the quantity.
    Division,
    /// Denominator scaling overflowed.
    Arithmetic,
    /// The V2 all-K retirement action must use the ordered V3 cursor route.
    UseOrderedRetirement,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, FractionalPhysicalErrorV3>;
