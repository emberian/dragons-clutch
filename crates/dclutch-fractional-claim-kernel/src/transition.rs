//! Exact lifecycle plans over authenticated Claims and Token observations.

use crate::abi::{
    Error, FractionalPhaseV1, FractionalProjectionV1, FractionalTermsV1, OutcomeReserveV1, Result,
    exact_shard_capacity,
};

/// One explicit Token-owned claim-shard instrument.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimShardInstrumentV1 {
    /// Finalized terms selecting denominator and outcome ordering.
    pub terms_id: [u8; 32],
    /// Product-owned categorical outcome.
    pub outcome: u32,
    /// Immutable Token shard Mint for this exact outcome.
    pub shard_mint: [u8; 32],
    /// Exact raw Token base units.
    pub shard_atoms: u64,
}

/// Result of the sole claim-shard quotient/remainder boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimShardDivisionV1 {
    /// Exact selected shard input in raw Token base units.
    pub input_shards: ClaimShardInstrumentV1,
    /// Whole native categorical claims represented by the input.
    pub whole_native_claims: u64,
    /// Exact whole-denominator shard multiple which may be burned.
    pub consumed_shards: ClaimShardInstrumentV1,
    /// Explicit same-Mint change which remains Token-owned and transferable.
    pub change_shards: ClaimShardInstrumentV1,
}

/// Divide one same-Mint shard input at the protocol's sole quotient/remainder boundary.
///
/// This function never burns or mints change. A physical adapter burns only
/// `consumed_shards`; `change_shards` remains an ordinary balance of the exact
/// input Mint.
pub fn divide_claim_shards_v1(
    terms: FractionalTermsV1<'_>,
    outcome: u32,
    input_shards: u64,
) -> Result<ClaimShardDivisionV1> {
    if input_shards == 0 {
        return Err(Error::ZeroQuantity);
    }
    let denominator = terms.denominator();
    // This is the one named quotient/remainder boundary in the kernel.
    let whole_native_claims = input_shards / denominator;
    let change = input_shards % denominator;
    let consumed = denominator
        .checked_mul(whole_native_claims)
        .ok_or(Error::ArithmeticOverflow)?;
    let shard_mint = terms.shard_mint(outcome)?;
    let instrument = |shard_atoms| ClaimShardInstrumentV1 {
        terms_id: terms.terms_id(),
        outcome,
        shard_mint,
        shard_atoms,
    };
    Ok(ClaimShardDivisionV1 {
        input_shards: instrument(input_shards),
        whole_native_claims,
        consumed_shards: instrument(consumed),
        change_shards: instrument(change),
    })
}

/// Exact native-to-shard wrap plan for one open categorical outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WrapPlanV1 {
    /// Product-owned outcome whose native claims enter custody.
    pub outcome: u32,
    /// Native claims debited from the actor and credited to canonical custody.
    pub native_claims_to_lock: u64,
    /// Exact shard atoms minted to the actor.
    pub shards_to_mint: ClaimShardInstrumentV1,
    /// Reserve row after both physical effects succeed atomically.
    pub post_reserve: OutcomeReserveV1,
    /// Actor native-claim balance after the exact debit.
    pub post_actor_native_claims: u64,
    /// Actor shard balance after the exact mint.
    pub post_actor_shards: u64,
    /// Required next wrapper replay revision.
    pub next_revision: u64,
}

/// Prepare exact denomination of native claims into same-outcome shard atoms.
pub fn prepare_wrap_v1(
    terms: FractionalTermsV1<'_>,
    projection: FractionalProjectionV1<'_>,
    outcome: u32,
    native_claims: u64,
    actor_native_claims: u64,
    actor_shards: u64,
) -> Result<WrapPlanV1> {
    require_open(projection)?;
    require_nonzero(native_claims)?;
    let reserve = projection.reserve(outcome)?;
    validate_holder_balance(actor_shards, reserve.shard_supply)?;
    let post_actor_native_claims = actor_native_claims
        .checked_sub(native_claims)
        .ok_or(Error::InsufficientBalance)?;
    let minted = exact_shard_capacity(terms.denominator(), native_claims)?;
    let post_reserve = OutcomeReserveV1 {
        locked_native_claims: reserve
            .locked_native_claims
            .checked_add(native_claims)
            .ok_or(Error::ArithmeticOverflow)?,
        shard_supply: reserve
            .shard_supply
            .checked_add(minted)
            .ok_or(Error::ArithmeticOverflow)?,
    };
    require_exact_reserve(terms.denominator(), post_reserve)?;
    let post_actor_shards = actor_shards
        .checked_add(minted)
        .ok_or(Error::ArithmeticOverflow)?;
    if post_actor_shards > post_reserve.shard_supply {
        return Err(Error::InsufficientBalance);
    }
    Ok(WrapPlanV1 {
        outcome,
        native_claims_to_lock: native_claims,
        shards_to_mint: instrument(terms, outcome, minted)?,
        post_reserve,
        post_actor_native_claims,
        post_actor_shards,
        next_revision: next_revision(projection)?,
    })
}

/// Exact Token observations needed to prepare one ordinary transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferObservationV1 {
    /// Token source identity authenticated by the physical adapter.
    pub source_account: [u8; 32],
    /// Token destination identity authenticated by the physical adapter.
    pub destination_account: [u8; 32],
    /// Exact observed source balance in raw Token base units.
    pub source_shards: u64,
    /// Exact observed destination balance in raw Token base units.
    pub destination_shards: u64,
}

/// Ordinary Token transfer plan which creates no wrapper-owned balance truth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferPlanV1 {
    /// Exact same-Mint shard instrument transferred.
    pub shards_to_transfer: ClaimShardInstrumentV1,
    /// Token source identity authenticated by the physical adapter.
    pub source_account: [u8; 32],
    /// Token destination identity authenticated by the physical adapter.
    pub destination_account: [u8; 32],
    /// Expected source balance after Token transfer.
    pub post_source_shards: u64,
    /// Expected destination balance after Token transfer.
    pub post_destination_shards: u64,
    /// Wrapper replay is unchanged because Token remains the sole holder ledger.
    pub unchanged_revision: u64,
}

/// Prepare an ordinary transfer of exact shard base units.
pub fn prepare_transfer_v1(
    terms: FractionalTermsV1<'_>,
    projection: FractionalProjectionV1<'_>,
    outcome: u32,
    shard_atoms: u64,
    observation: TransferObservationV1,
) -> Result<TransferPlanV1> {
    if projection.phase() == FractionalPhaseV1::Retired {
        return Err(Error::InvalidPhase);
    }
    require_nonzero(shard_atoms)?;
    if is_zero(&observation.source_account) || is_zero(&observation.destination_account) {
        return Err(Error::ZeroIdentity);
    }
    if observation.source_account == observation.destination_account {
        return Err(Error::AccountAlias);
    }
    let reserve = projection.reserve(outcome)?;
    validate_holder_balance(observation.source_shards, reserve.shard_supply)?;
    validate_holder_balance(observation.destination_shards, reserve.shard_supply)?;
    let observed_pair = observation
        .source_shards
        .checked_add(observation.destination_shards)
        .ok_or(Error::ArithmeticOverflow)?;
    if observed_pair > reserve.shard_supply {
        return Err(Error::InsufficientBalance);
    }
    let post_source_shards = observation
        .source_shards
        .checked_sub(shard_atoms)
        .ok_or(Error::InsufficientBalance)?;
    let post_destination_shards = observation
        .destination_shards
        .checked_add(shard_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    if post_source_shards
        .checked_add(post_destination_shards)
        .ok_or(Error::ArithmeticOverflow)?
        != observed_pair
    {
        return Err(Error::ArithmeticOverflow);
    }
    Ok(TransferPlanV1 {
        shards_to_transfer: instrument(terms, outcome, shard_atoms)?,
        source_account: observation.source_account,
        destination_account: observation.destination_account,
        post_source_shards,
        post_destination_shards,
        unchanged_revision: projection.revision(),
    })
}

/// Exact open unwrap or terminal redemption plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnwrapPlanV1 {
    /// Product-owned outcome being unwrapped or redeemed.
    pub outcome: u32,
    /// Sole quotient/remainder result for the selected Token-owned input.
    pub division: ClaimShardDivisionV1,
    /// Reserve row after burning only the whole-denominator multiple.
    pub post_reserve: OutcomeReserveV1,
    /// Actor shard balance after the exact burn; explicit change remains here.
    pub post_actor_shards: u64,
    /// Native claims returned to the actor before resolution.
    pub native_claims_to_actor: u64,
    /// Realm collateral atoms paid for terminal winning claims.
    pub collateral_atoms_to_actor: u64,
    /// Required next wrapper replay revision.
    pub next_revision: u64,
}

/// Prepare open reconstitution of arbitrary selected shards into whole native claims and change.
pub fn prepare_open_unwrap_v1(
    terms: FractionalTermsV1<'_>,
    projection: FractionalProjectionV1<'_>,
    outcome: u32,
    input_shards: u64,
    actor_shards: u64,
) -> Result<UnwrapPlanV1> {
    require_open(projection)?;
    prepare_whole_burn(terms, projection, outcome, input_shards, actor_shards, true)
}

/// Prepare terminal redemption of winning shards into categorical collateral and change.
pub fn prepare_terminal_redeem_v1(
    terms: FractionalTermsV1<'_>,
    projection: FractionalProjectionV1<'_>,
    outcome: u32,
    input_shards: u64,
    actor_shards: u64,
) -> Result<UnwrapPlanV1> {
    match projection.phase() {
        FractionalPhaseV1::Terminal { winning_outcome } if outcome == winning_outcome => {}
        _ => return Err(Error::InvalidPhase),
    }
    prepare_whole_burn(
        terms,
        projection,
        outcome,
        input_shards,
        actor_shards,
        false,
    )
}

/// Exact zero-payout burn plan for one authenticated losing outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ZeroBurnPlanV1 {
    /// Product-owned losing outcome.
    pub outcome: u32,
    /// Arbitrary exact losing shard base units burned by Token.
    pub shards_to_burn: ClaimShardInstrumentV1,
    /// Reserve after the zero-payout burn; native zero claims remain until retirement.
    pub post_reserve: OutcomeReserveV1,
    /// Actor shard balance after the exact burn.
    pub post_actor_shards: u64,
    /// Derived total losing shard atoms burned since terminalization.
    pub cumulative_zero_burned_shards: u64,
    /// Required next wrapper replay revision.
    pub next_revision: u64,
}

/// Prepare an arbitrary losing-shard zero-payout burn after authenticated resolution.
pub fn prepare_terminal_zero_burn_v1(
    terms: FractionalTermsV1<'_>,
    projection: FractionalProjectionV1<'_>,
    outcome: u32,
    shard_atoms: u64,
    actor_shards: u64,
) -> Result<ZeroBurnPlanV1> {
    match projection.phase() {
        FractionalPhaseV1::Terminal { winning_outcome } if outcome != winning_outcome => {}
        _ => return Err(Error::InvalidPhase),
    }
    require_nonzero(shard_atoms)?;
    let reserve = projection.reserve(outcome)?;
    validate_holder_balance(actor_shards, reserve.shard_supply)?;
    let post_reserve = OutcomeReserveV1 {
        locked_native_claims: reserve.locked_native_claims,
        shard_supply: reserve
            .shard_supply
            .checked_sub(shard_atoms)
            .ok_or(Error::InsufficientBalance)?,
    };
    let capacity = exact_shard_capacity(terms.denominator(), post_reserve.locked_native_claims)?;
    if post_reserve.shard_supply > capacity {
        return Err(Error::ReserveMismatch);
    }
    let post_actor_shards = actor_shards
        .checked_sub(shard_atoms)
        .ok_or(Error::InsufficientBalance)?;
    let cumulative_zero_burned_shards = capacity
        .checked_sub(post_reserve.shard_supply)
        .ok_or(Error::ReserveMismatch)?;
    Ok(ZeroBurnPlanV1 {
        outcome,
        shards_to_burn: instrument(terms, outcome, shard_atoms)?,
        post_reserve,
        post_actor_shards,
        cumulative_zero_burned_shards,
        next_revision: next_revision(projection)?,
    })
}

/// Pure phase transition plan from open to one authenticated terminal winner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalizePlanV1 {
    /// Product-owned categorical winning outcome.
    pub winning_outcome: u32,
    /// Required next wrapper replay revision.
    pub next_revision: u64,
}

/// Prepare a no-supply-change transition to an authenticated terminal winner.
pub fn prepare_terminalize_v1(
    projection: FractionalProjectionV1<'_>,
    winning_outcome: u32,
) -> Result<TerminalizePlanV1> {
    require_open(projection)?;
    if winning_outcome >= projection.outcome_count() {
        return Err(Error::InvalidOutcome);
    }
    Ok(TerminalizePlanV1 {
        winning_outcome,
        next_revision: next_revision(projection)?,
    })
}

/// Retirement effects after every Token shard Mint supply is zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirePlanV1<'a> {
    terms: FractionalTermsV1<'a>,
    projection: FractionalProjectionV1<'a>,
    winning_outcome: u32,
    next_revision: u64,
}

impl RetirePlanV1<'_> {
    /// Winning outcome selected by the authenticated terminal Market.
    pub const fn winning_outcome(self) -> u32 {
        self.winning_outcome
    }

    /// Required next wrapper replay revision.
    pub const fn next_revision(self) -> u64 {
        self.next_revision
    }

    /// Exact zero-payout native claims which Claims must burn for one losing outcome.
    pub fn zero_payout_native_claims_to_burn(self, outcome: u32) -> Result<u64> {
        if outcome >= self.projection.outcome_count() {
            return Err(Error::InvalidOutcome);
        }
        if outcome == self.winning_outcome {
            return Ok(0);
        }
        self.projection
            .reserve(outcome)
            .map(|reserve| reserve.locked_native_claims)
    }

    /// Exact shard Mint selected for the retirement check of one outcome.
    pub fn shard_mint(self, outcome: u32) -> Result<[u8; 32]> {
        self.terms.shard_mint(outcome)
    }
}

/// Prepare final retirement after all shard supplies are physically zero.
pub fn prepare_retire_v1<'a>(
    terms: FractionalTermsV1<'a>,
    projection: FractionalProjectionV1<'a>,
) -> Result<RetirePlanV1<'a>> {
    let winning_outcome = match projection.phase() {
        FractionalPhaseV1::Terminal { winning_outcome } => winning_outcome,
        _ => return Err(Error::InvalidPhase),
    };
    let mut outcome = 0_u32;
    while outcome < projection.outcome_count() {
        let reserve = projection.reserve(outcome)?;
        if reserve.shard_supply != 0 {
            return Err(Error::OutstandingShardSupply);
        }
        if outcome == winning_outcome && reserve.locked_native_claims != 0 {
            return Err(Error::OutstandingWinningClaims);
        }
        outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(RetirePlanV1 {
        terms,
        projection,
        winning_outcome,
        next_revision: next_revision(projection)?,
    })
}

fn prepare_whole_burn(
    terms: FractionalTermsV1<'_>,
    projection: FractionalProjectionV1<'_>,
    outcome: u32,
    input_shards: u64,
    actor_shards: u64,
    return_native: bool,
) -> Result<UnwrapPlanV1> {
    let reserve = projection.reserve(outcome)?;
    validate_holder_balance(actor_shards, reserve.shard_supply)?;
    if input_shards > actor_shards {
        return Err(Error::InsufficientBalance);
    }
    let division = divide_claim_shards_v1(terms, outcome, input_shards)?;
    if division.whole_native_claims == 0 {
        return Err(Error::NoWholeClaim);
    }
    let post_reserve = OutcomeReserveV1 {
        locked_native_claims: reserve
            .locked_native_claims
            .checked_sub(division.whole_native_claims)
            .ok_or(Error::InsufficientBalance)?,
        shard_supply: reserve
            .shard_supply
            .checked_sub(division.consumed_shards.shard_atoms)
            .ok_or(Error::InsufficientBalance)?,
    };
    require_exact_reserve(terms.denominator(), post_reserve)?;
    let post_actor_shards = actor_shards
        .checked_sub(division.consumed_shards.shard_atoms)
        .ok_or(Error::InsufficientBalance)?;
    Ok(UnwrapPlanV1 {
        outcome,
        division,
        post_reserve,
        post_actor_shards,
        native_claims_to_actor: if return_native {
            division.whole_native_claims
        } else {
            0
        },
        collateral_atoms_to_actor: if return_native {
            0
        } else {
            division.whole_native_claims
        },
        next_revision: next_revision(projection)?,
    })
}

fn instrument(
    terms: FractionalTermsV1<'_>,
    outcome: u32,
    shard_atoms: u64,
) -> Result<ClaimShardInstrumentV1> {
    Ok(ClaimShardInstrumentV1 {
        terms_id: terms.terms_id(),
        outcome,
        shard_mint: terms.shard_mint(outcome)?,
        shard_atoms,
    })
}

fn require_open(projection: FractionalProjectionV1<'_>) -> Result<()> {
    if projection.phase() != FractionalPhaseV1::Open {
        return Err(Error::InvalidPhase);
    }
    Ok(())
}

fn require_nonzero(quantity: u64) -> Result<()> {
    if quantity == 0 {
        return Err(Error::ZeroQuantity);
    }
    Ok(())
}

fn validate_holder_balance(balance: u64, supply: u64) -> Result<()> {
    if balance > supply {
        return Err(Error::InsufficientBalance);
    }
    Ok(())
}

fn require_exact_reserve(denominator: u64, reserve: OutcomeReserveV1) -> Result<()> {
    if reserve.shard_supply != exact_shard_capacity(denominator, reserve.locked_native_claims)? {
        return Err(Error::ReserveMismatch);
    }
    Ok(())
}

fn next_revision(projection: FractionalProjectionV1<'_>) -> Result<u64> {
    projection
        .revision()
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)
}

fn is_zero(identity: &[u8; 32]) -> bool {
    identity.iter().all(|byte| *byte == 0)
}
