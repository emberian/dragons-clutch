//! Exact Structured V2 lifecycle plans over authenticated Token observations.

use dclutch_fractional_claim_kernel::{
    Error as FractionalError, FractionalExposureTermsV2, divide_exposure_shards_v2,
};

use crate::abi::{Error, Result, StructuredPhaseV2, StructuredProjectionV2, StructuredTermsV2};

/// Exact movement of one coordinate's shard atoms into or out of Structured custody.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ShardMovementV2 {
    /// Claims representation coordinate in `[0,K)`.
    pub representation_coordinate: u32,
    /// Shard-terms-owned Mint for this exact coordinate.
    pub shard_mint: [u8; 32],
    /// Exact raw Token base units moved; zero for an inert coefficient row.
    pub shard_atoms: u64,
    /// Exact required Structured custody for this coordinate after the action.
    pub post_required_custody: u64,
    /// Named unowned donation above the exact backing; never moved by any plan.
    pub surplus_shard_custody: u64,
}

/// Exact receipt-side effect shared by every supply-changing action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceiptEffectV2 {
    /// Token-owned Structured receipt Mint.
    pub receipt_mint: [u8; 32],
    /// Exact receipt atoms minted or burned.
    pub receipt_atoms: u64,
    /// Receipt Mint supply before the effect.
    pub pre_receipt_supply: u64,
    /// Receipt Mint supply after the effect.
    pub post_receipt_supply: u64,
    /// Actor receipt balance after the effect.
    pub post_actor_receipts: u64,
    /// Required next Structured replay revision.
    pub next_revision: u64,
}

/// Exact plan for locking the shard basket and minting receipt atoms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredIssuePlanV2 {
    /// Receipt-side effect.
    pub receipt: ReceiptEffectV2,
    /// Total shard atoms locked across every coordinate.
    pub total_shard_atoms: u64,
}

/// Exact plan for burning receipt atoms and releasing the shard basket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredReleasePlanV2 {
    /// Receipt-side effect.
    pub receipt: ReceiptEffectV2,
    /// Total shard atoms released across every coordinate.
    pub total_shard_atoms: u64,
}

/// Exact per-coordinate terminal settlement derived from the released basket.
///
/// This is a checked projection of what the released shard atoms yield at the
/// claim-shard layer.  Structured commits no collateral; the shard layer stays
/// the sole owner of native claim redemption.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StructuredSettlementRowV2 {
    /// Claims representation coordinate in `[0,K)`.
    pub representation_coordinate: u32,
    /// Shard-terms-owned Mint for this exact coordinate.
    pub shard_mint: [u8; 32],
    /// Exact shard atoms released from Structured custody.
    pub released_shards: u64,
    /// Whole native claims represented by the released shards.
    pub whole_claims: u64,
    /// Exact whole-denominator multiple redeemable at the shard layer.
    pub burned_shards: u64,
    /// Explicit same-Mint change that stays transferable and aggregable.
    pub change_shards: u64,
    /// Authenticated collateral atoms per whole native claim.
    pub payout_per_claim: u64,
    /// Exact collateral atoms; zero is a valid, honest result.
    pub collateral_atoms: u64,
}

/// Exact plan for terminal redemption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredTerminalPlanV2 {
    /// The underlying release; terminal redemption releases the same exact basket.
    pub release: StructuredReleasePlanV2,
    /// Exact total collateral atoms the released basket yields at the shard layer.
    pub total_collateral_atoms: u64,
}

/// Exact plan for closing a zero-supply, zero-custody Structured node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredRetirePlanV2 {
    /// Token-owned Structured receipt Mint to close.
    pub receipt_mint: [u8; 32],
    /// Exact representation width whose custody accounts must all close.
    pub representation_width: u32,
    /// Required next Structured replay revision.
    pub next_revision: u64,
}

/// Prepare exact denomination of a shard basket into Structured receipt atoms.
///
/// `movements` is caller-owned fixed storage of exactly `K` entries and is
/// written only after every checked product and balance succeeds.
pub fn plan_structured_issue_v2(
    terms: StructuredTermsV2<'_>,
    shard_terms: FractionalExposureTermsV2<'_>,
    projection: StructuredProjectionV2<'_>,
    receipt_atoms: u64,
    actor_receipts: u64,
    movements: &mut [ShardMovementV2],
) -> Result<StructuredIssuePlanV2> {
    let context = prepare(terms, shard_terms, projection, receipt_atoms, movements)?;
    if projection.phase() != StructuredPhaseV2::Open {
        return Err(Error::InvalidPhase);
    }
    let post_receipt_supply = projection
        .receipt_supply()
        .checked_add(receipt_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    let post_actor_receipts = actor_receipts
        .checked_add(receipt_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    if actor_receipts > projection.receipt_supply() || post_actor_receipts > post_receipt_supply {
        return Err(Error::InsufficientBalance);
    }
    let total_shard_atoms = commit(
        terms,
        shard_terms,
        projection,
        receipt_atoms,
        post_receipt_supply,
        movements,
        context,
    )?;
    Ok(StructuredIssuePlanV2 {
        receipt: ReceiptEffectV2 {
            receipt_mint: terms.receipt_mint(),
            receipt_atoms,
            pre_receipt_supply: projection.receipt_supply(),
            post_receipt_supply,
            post_actor_receipts,
            next_revision: next_revision(projection)?,
        },
        total_shard_atoms,
    })
}

/// Prepare exact reconstitution of receipt atoms into their shard basket while
/// the Market is open.
pub fn plan_structured_unwrap_v2(
    terms: StructuredTermsV2<'_>,
    shard_terms: FractionalExposureTermsV2<'_>,
    projection: StructuredProjectionV2<'_>,
    receipt_atoms: u64,
    actor_receipts: u64,
    movements: &mut [ShardMovementV2],
) -> Result<StructuredReleasePlanV2> {
    if projection.phase() != StructuredPhaseV2::Open {
        return Err(Error::InvalidPhase);
    }
    plan_release(
        terms,
        shard_terms,
        projection,
        receipt_atoms,
        actor_receipts,
        movements,
    )
}

/// Prepare exact terminal redemption after an authenticated terminal result.
///
/// `settlement` is caller-owned fixed storage of exactly `K` entries. Every row
/// is derived through [`divide_exposure_shards_v2`], the protocol's sole
/// quotient/remainder boundary; Structured introduces no rounding of its own.
pub fn plan_structured_terminal_redeem_v2(
    terms: StructuredTermsV2<'_>,
    shard_terms: FractionalExposureTermsV2<'_>,
    projection: StructuredProjectionV2<'_>,
    receipt_atoms: u64,
    actor_receipts: u64,
    movements: &mut [ShardMovementV2],
    settlement: &mut [StructuredSettlementRowV2],
) -> Result<StructuredTerminalPlanV2> {
    if projection.phase() != StructuredPhaseV2::Terminal {
        return Err(Error::InvalidPhase);
    }
    if settlement.len() != width(terms)? {
        return Err(Error::InvalidLength);
    }
    let release = plan_release(
        terms,
        shard_terms,
        projection,
        receipt_atoms,
        actor_receipts,
        movements,
    )?;
    let mut total_collateral_atoms = 0_u64;
    let mut coordinate = 0_u32;
    while coordinate < terms.representation_width() {
        let index = usize::try_from(coordinate).map_err(|_| Error::InvalidCoordinate)?;
        let movement = *movements.get(index).ok_or(Error::InvalidLength)?;
        let payout_per_claim = projection.observation(coordinate)?.payout_per_claim;
        let (whole_claims, change_shards) =
            divide_released_shards(shard_terms, coordinate, movement.shard_atoms)?;
        let burned_shards = whole_claims
            .checked_mul(terms.denominator())
            .ok_or(Error::ArithmeticOverflow)?;
        let collateral_atoms = whole_claims
            .checked_mul(payout_per_claim)
            .ok_or(Error::ArithmeticOverflow)?;
        total_collateral_atoms = total_collateral_atoms
            .checked_add(collateral_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        *settlement.get_mut(index).ok_or(Error::InvalidLength)? = StructuredSettlementRowV2 {
            representation_coordinate: coordinate,
            shard_mint: movement.shard_mint,
            released_shards: movement.shard_atoms,
            whole_claims,
            burned_shards,
            change_shards,
            payout_per_claim,
            collateral_atoms,
        };
        coordinate = coordinate.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(StructuredTerminalPlanV2 {
        release,
        total_collateral_atoms,
    })
}

/// Prepare final retirement after receipt supply and all observed custody,
/// including donated surplus, are physically zero.
pub fn plan_structured_retire_v2(
    terms: StructuredTermsV2<'_>,
    shard_terms: FractionalExposureTermsV2<'_>,
    projection: StructuredProjectionV2<'_>,
) -> Result<StructuredRetirePlanV2> {
    join(terms, shard_terms, projection)?;
    if projection.phase() != StructuredPhaseV2::Terminal {
        return Err(Error::InvalidPhase);
    }
    if projection.receipt_supply() != 0 {
        return Err(Error::OutstandingReceiptSupply);
    }
    let mut coordinate = 0_u32;
    while coordinate < terms.representation_width() {
        if projection.observation(coordinate)?.observed_shard_custody != 0 {
            return Err(Error::OutstandingShardCustody);
        }
        coordinate = coordinate.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(StructuredRetirePlanV2 {
        receipt_mint: terms.receipt_mint(),
        representation_width: terms.representation_width(),
        next_revision: next_revision(projection)?,
    })
}

/// Divide released shard atoms at the claim-shard layer's sole boundary.
///
/// `divide_exposure_shards_v2` is total and owns the division.  Its two
/// documented sub-lot refusals mathematically force a zero whole-claim result,
/// which this helper restates rather than recomputing: a zero release yields
/// nothing, and a release below the denominator is entirely explicit change.
fn divide_released_shards(
    shard_terms: FractionalExposureTermsV2<'_>,
    representation_coordinate: u32,
    released_shards: u64,
) -> Result<(u64, u64)> {
    match divide_exposure_shards_v2(shard_terms, representation_coordinate, released_shards) {
        Ok(division) => Ok((division.whole_claims, division.change.shard_atoms)),
        Err(FractionalError::ZeroQuantity) => Ok((0, 0)),
        Err(FractionalError::NoWholeClaim) => Ok((0, released_shards)),
        Err(FractionalError::ArithmeticOverflow) => Err(Error::ArithmeticOverflow),
        Err(FractionalError::InvalidOutcome) => Err(Error::InvalidCoordinate),
        Err(_) => Err(Error::ShardLayerMismatch),
    }
}

fn plan_release(
    terms: StructuredTermsV2<'_>,
    shard_terms: FractionalExposureTermsV2<'_>,
    projection: StructuredProjectionV2<'_>,
    receipt_atoms: u64,
    actor_receipts: u64,
    movements: &mut [ShardMovementV2],
) -> Result<StructuredReleasePlanV2> {
    let context = prepare(terms, shard_terms, projection, receipt_atoms, movements)?;
    if actor_receipts > projection.receipt_supply() {
        return Err(Error::InsufficientBalance);
    }
    if receipt_atoms > actor_receipts {
        return Err(Error::InsufficientBalance);
    }
    let post_receipt_supply = projection
        .receipt_supply()
        .checked_sub(receipt_atoms)
        .ok_or(Error::InsufficientBalance)?;
    let post_actor_receipts = actor_receipts
        .checked_sub(receipt_atoms)
        .ok_or(Error::InsufficientBalance)?;
    let total_shard_atoms = commit(
        terms,
        shard_terms,
        projection,
        receipt_atoms,
        post_receipt_supply,
        movements,
        context,
    )?;
    Ok(StructuredReleasePlanV2 {
        receipt: ReceiptEffectV2 {
            receipt_mint: terms.receipt_mint(),
            receipt_atoms,
            pre_receipt_supply: projection.receipt_supply(),
            post_receipt_supply,
            post_actor_receipts,
            next_revision: next_revision(projection)?,
        },
        total_shard_atoms,
    })
}

/// Nonzero quantity, joined identities, and exact caller storage width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreparedV2 {
    width: usize,
}

fn prepare(
    terms: StructuredTermsV2<'_>,
    shard_terms: FractionalExposureTermsV2<'_>,
    projection: StructuredProjectionV2<'_>,
    receipt_atoms: u64,
    movements: &[ShardMovementV2],
) -> Result<PreparedV2> {
    join(terms, shard_terms, projection)?;
    if receipt_atoms == 0 {
        return Err(Error::ZeroQuantity);
    }
    let width = width(terms)?;
    if movements.len() != width {
        return Err(Error::InvalidLength);
    }
    Ok(PreparedV2 { width })
}

fn commit(
    terms: StructuredTermsV2<'_>,
    shard_terms: FractionalExposureTermsV2<'_>,
    projection: StructuredProjectionV2<'_>,
    receipt_atoms: u64,
    post_receipt_supply: u64,
    movements: &mut [ShardMovementV2],
    context: PreparedV2,
) -> Result<u64> {
    if movements.len() != context.width {
        return Err(Error::InvalidLength);
    }
    let mut total_shard_atoms = 0_u64;
    let mut coordinate = 0_u32;
    while coordinate < terms.representation_width() {
        let index = usize::try_from(coordinate).map_err(|_| Error::InvalidCoordinate)?;
        let shard_atoms = terms.required_shard_custody(coordinate, receipt_atoms)?;
        let post_required_custody =
            terms.required_shard_custody(coordinate, post_receipt_supply)?;
        let surplus_shard_custody = projection.surplus_shard_custody(terms, coordinate)?;
        // Solvency after the action: the observed balance still covers the exact
        // required backing, and the named surplus is unchanged either way.
        post_required_custody
            .checked_add(surplus_shard_custody)
            .ok_or(Error::ArithmeticOverflow)?;
        total_shard_atoms = total_shard_atoms
            .checked_add(shard_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        *movements.get_mut(index).ok_or(Error::InvalidLength)? = ShardMovementV2 {
            representation_coordinate: coordinate,
            shard_mint: shard_terms
                .shard_mint(coordinate)
                .map_err(|_| Error::ShardLayerMismatch)?,
            shard_atoms,
            post_required_custody,
            surplus_shard_custody,
        };
        coordinate = coordinate.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(total_shard_atoms)
}

fn join(
    terms: StructuredTermsV2<'_>,
    shard_terms: FractionalExposureTermsV2<'_>,
    projection: StructuredProjectionV2<'_>,
) -> Result<()> {
    terms.bind_shard_terms(shard_terms)?;
    if projection.terms_id() != terms.terms_id()
        || projection.market() != terms.market()
        || projection.representation_width() != terms.representation_width()
    {
        return Err(Error::AdmissionMismatch);
    }
    if projection.shard_terms() != terms.shard_terms() {
        return Err(Error::ShardLayerMismatch);
    }
    if projection.denominator() != terms.denominator() {
        return Err(Error::NonFractionalDenominator);
    }
    Ok(())
}

fn width(terms: StructuredTermsV2<'_>) -> Result<usize> {
    usize::try_from(terms.representation_width()).map_err(|_| Error::InvalidCoordinate)
}

fn next_revision(projection: StructuredProjectionV2<'_>) -> Result<u64> {
    projection
        .revision()
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)
}
