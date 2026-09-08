//! `DealerFill` (`DCLSFLR1`): a taker's fill against the Dealer -- the batch
//! of two, Direct as an RFQ.
//!
//! Frame: the Lean's `fillFrame` (19), then four windows in this order:
//!
//! | window | width | what |
//! |---|---|---|
//! | Claims signed delta | 20 + 2 | the aggregate moves by `mint` on every ordinary outcome; the Dealer's Position by `receive − deliver`; the taker's by `mint + deliver − receive` |
//! | Custody `Transfer` A | 14 | fund → Hoard (the Dealer's share of the mint's par) |
//! | Custody `Transfer` B | 14 | taker → Hoard (the taker's share) |
//! | Custody `Transfer` C | 14 | fund → taker, or taker → fund (whichever way the net goes) |
//!
//! The legs are `ScoringRuleAbiV1.cashLegs`: the Hoard receives exactly the
//! mint's par, the fund moves by exactly the Dealer's debit
//! (`hoard_receives_the_mint`, `fund_moves_by_the_debit`). A leg of zero is
//! not invoked. The rule is checked FIRST -- R0–R3 over the inventory the
//! Position holds -- and nothing moves unless it admits.

extern crate alloc;

use dclutch_claims::CallerRole;
use dclutch_claims::frame_spec_v1::SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3;
use dclutch_claims::signed_delta_v3::{
    DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SignedDeltaPlanInputV3,
    SignedDeltaPlanV3, SignedDeltaPositionV3, SignedDeltaReceiptV3, SignedDeltaV3,
    ValidatedSignedDeltaConstructionV3,
};
use dclutch_custody::{CompartmentV1, CustodyVaultSeedsV1, TRANSFER_ACCOUNT_COUNT_V1};
use dclutch_market::Phase;
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading::scoring_rule::records_v1::DealerFundV1;
use dclutch_trading::scoring_rule::requests_v1::{
    DealerFillRequestV1, DealerRouteV1, FILL_FRAME_ACCOUNTS, fill_privileges_v1,
};
use dclutch_trading::scoring_rule::{Potential, RuleParameters, admit_fill, generated};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::Instruction,
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use super::{
    CustodyLegV1, MarketFactsV1, ScoringDealerErrorV1, authenticate_fund_v1,
    authenticate_market_v1, authenticate_release_v1, authenticate_rule_v1, authenticate_vault_v1,
    child_metas_v1, claims_signed_delta_privileges_v1, commit_fund_v1, emit_receipt_v1, get,
    invoke_custody_transfer_v1, parse_prefix, read_inventory_v1, read_replay_v1,
};
use crate::hot_v3::hot_cu_checkpoint_macro as hot_cu_checkpoint;

/// The Claims window: the fixed signed-delta frame plus the two Positions.
pub const FILL_CLAIMS_WINDOW_ACCOUNTS: usize = SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 as usize + 2;
/// Claims' own signed-delta coordinates this route both forwards and reads
/// (`dclutch-claims/src/frame_spec_v1.rs:448-472`): the aggregate, then the
/// two `SignedDeltaPosition` slots in the order the Position table names them.
pub const DELTA_AGGREGATE_ACCOUNT: usize = 1;
/// See [`DELTA_AGGREGATE_ACCOUNT`]. Position table order: the Dealer, then the
/// taker, exactly as `positions` is built below.
pub const DELTA_DEALER_POSITION_ACCOUNT: usize = SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 as usize;
/// See [`DELTA_AGGREGATE_ACCOUNT`].
pub const DELTA_TAKER_POSITION_ACCOUNT: usize = SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 as usize + 1;
/// One Custody `Transfer` window.
pub const FILL_CUSTODY_WINDOW_ACCOUNTS: usize = TRANSFER_ACCOUNT_COUNT_V1 as usize;
/// Every window after the prefix.
pub const FILL_WINDOW_ACCOUNTS: usize =
    FILL_CLAIMS_WINDOW_ACCOUNTS + 3 * FILL_CUSTODY_WINDOW_ACCOUNTS;

/// The cash legs, atoms, as `cashLegs` derives them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CashLegsV1 {
    /// Taker → Hoard.
    pub taker_to_hoard: u64,
    /// Fund → Hoard.
    pub fund_to_hoard: u64,
    /// Fund → taker.
    pub fund_to_taker: u64,
    /// Taker → fund.
    pub taker_to_fund: u64,
}

/// `ScoringRuleAbiV1.cashLegs`, verbatim.
pub fn cash_legs_v1(
    mint_atoms: u64,
    dealer_pays_atoms: u64,
    dealer_receives_atoms: u64,
) -> CashLegsV1 {
    if dealer_receives_atoms == 0 {
        let fund_to_hoard = dealer_pays_atoms.min(mint_atoms);
        CashLegsV1 {
            taker_to_hoard: mint_atoms - fund_to_hoard,
            fund_to_hoard,
            fund_to_taker: dealer_pays_atoms - fund_to_hoard,
            taker_to_fund: 0,
        }
    } else {
        CashLegsV1 {
            taker_to_hoard: mint_atoms,
            fund_to_hoard: 0,
            fund_to_taker: 0,
            taker_to_fund: dealer_receives_atoms,
        }
    }
}

/// Execute one fill.
#[inline(never)]
pub fn process_dealer_fill_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let request =
        DealerFillRequestV1::decode(instruction_data).map_err(|_| ScoringDealerErrorV1::Request)?;
    let (prefix, windows) = parse_prefix(
        accounts,
        FILL_FRAME_ACCOUNTS,
        FILL_WINDOW_ACCOUNTS,
        fill_privileges_v1,
    )?;
    hot_cu_checkpoint!("scoring-dealer:fill:frame");
    let taker = get(prefix, generated::FILL_TAKER_ACCOUNT)?;
    if taker.key.to_bytes() != request.taker {
        return Err(ScoringDealerErrorV1::Frame.into());
    }
    let fund_account = get(prefix, generated::FILL_FUND_ACCOUNT)?;
    let fund = authenticate_fund_v1(
        program_id,
        fund_account,
        request.market,
        request.dealer_id,
        request.expected_fund_revision,
    )?;
    if fund.outcome_count != request.outcome_count {
        return Err(ScoringDealerErrorV1::Width.into());
    }
    let rule = authenticate_rule_v1(program_id, get(prefix, generated::FILL_RULE_ACCOUNT)?, fund)?;
    let facts = authenticate_market_v1(
        get(prefix, generated::FILL_MARKET_ACCOUNT)?,
        request.market,
        &[Phase::Open],
    )?;
    let released_claims = authenticate_release_v1(
        program_id,
        get(prefix, generated::FILL_REGISTRY_PROGRAM_ACCOUNT)?,
        get(prefix, generated::FILL_ACTIVATION_CACHE_ACCOUNT)?,
        facts,
    )?;
    // The frame's Claims program is the release's, not the caller's word for
    // it: `read_inventory_v1` takes it as the owner a Position must have.
    let claims_program = get(prefix, generated::FILL_CLAIMS_PROGRAM_ACCOUNT)?;
    if claims_program.key != &released_claims {
        return Err(ScoringDealerErrorV1::Release.into());
    }
    let custody_program = get(prefix, generated::FILL_CUSTODY_PROGRAM_ACCOUNT)?;
    let vault = get(prefix, generated::FILL_VAULT_ACCOUNT)?;
    authenticate_vault_v1(
        custody_program.key,
        vault,
        fund,
        fund_account.key,
        facts.release_set,
    )?;
    hot_cu_checkpoint!("scoring-dealer:fill:authenticated");

    // The rule, over the inventory the chain holds.
    let aggregate = get(prefix, generated::FILL_AGGREGATE_ACCOUNT)?;
    let dealer_position = get(prefix, generated::FILL_DEALER_POSITION_ACCOUNT)?;
    let admission = admit_fill_v1(
        claims_program.key,
        aggregate,
        dealer_position,
        fund_account.key,
        rule.parameters,
        fund.outcome_count,
        &request,
    )?;
    hot_cu_checkpoint!("scoring-dealer:fill:admitted");

    // The cash, atoms.
    let (pays_atoms, receives_atoms) = fund
        .debit_atoms(admission.dealer_pays, admission.dealer_receives)
        .map_err(ScoringDealerErrorV1::from)?;
    let mint_atoms = fund
        .atoms(request.mint)
        .map_err(ScoringDealerErrorV1::from)?;
    let legs = cash_legs_v1(mint_atoms, pays_atoms, receives_atoms);
    if legs.fund_to_hoard + legs.fund_to_taker > fund.cash {
        // `solvent` says this cannot happen while Φ ≥ 0; it is named anyway.
        return Err(ScoringDealerErrorV1::Uncovered.into());
    }

    // The claims.
    let (claims_window, rest) = windows.split_at(FILL_CLAIMS_WINDOW_ACCOUNTS);
    let taker_position = get(prefix, generated::FILL_TAKER_POSITION_ACCOUNT)?;
    // The three Claims accounts the prefix names and the window carries are
    // ONE account each. Without this the route could price the fill against one
    // Position and hand Claims another, and Claims -- authenticating its own
    // frame, correctly -- would have no way to know the difference.
    let (dealer_index, _) =
        signed_delta_position_indexes_v1(fund_account.key.to_bytes(), request.taker);
    let (dealer_window_account, taker_window_account) =
        signed_delta_window_accounts_v1(dealer_index);
    for (prefix_account, window_account) in [
        (aggregate, DELTA_AGGREGATE_ACCOUNT),
        (dealer_position, dealer_window_account),
        (taker_position, taker_window_account),
    ] {
        if prefix_account.key != get(claims_window, window_account)?.key {
            return Err(ScoringDealerErrorV1::Frame.into());
        }
    }
    let taker_revision = {
        let data = taker_position
            .try_borrow_data()
            .map_err(|_| ScoringDealerErrorV1::Claims)?;
        let view =
            dclutch_claims::liability_basis_state_v2::LiabilityBasisPositionViewV2::decode(&data)
                .map_err(|_| ScoringDealerErrorV1::Claims)?;
        if view.owner != request.taker || view.market_account != aggregate.key.to_bytes() {
            return Err(ScoringDealerErrorV1::Claims.into());
        }
        view.revision
    };
    invoke_claims_fill_delta_v1(
        program_id,
        claims_program,
        claims_window,
        facts,
        fund,
        fund_account.key,
        admission,
        taker_revision,
        request,
        instruction_data,
    )?;
    hot_cu_checkpoint!("scoring-dealer:fill:claims");

    // The cash legs.
    settle_cash_legs_v1(
        program_id,
        custody_program,
        get(prefix, generated::FILL_HOARD_ACCOUNT)?,
        rest,
        facts,
        fund.market,
        fund_account.key.to_bytes(),
        taker,
        request.taker,
        hash(instruction_data).to_bytes(),
        legs,
    )?;
    hot_cu_checkpoint!("scoring-dealer:fill:custody");

    // The fund: cash by the debit, Ŵ carried for the next admission (the
    // carrier of §6 -- a cache the next fill re-derives, never an author).
    let before = fund.to_bytes().map_err(|_| ScoringDealerErrorV1::Commit)?;
    let next = next_fund_v1(fund, admission.potential_after, pays_atoms, receives_atoms)?;
    let fund_bytes = commit_fund_v1(fund_account, hash(&before).to_bytes(), next)?;
    emit_receipt_v1(
        DealerRouteV1::Fill,
        instruction_data,
        next,
        &fund_bytes,
        admission.dealer_pays,
        admission.dealer_receives,
    )
}

/// The Hoard vault the mint's par lands in, and the custody context its
/// founding named.
///
/// Zero when neither Hoard leg runs: nothing derives, nothing is checked, and
/// the value is never read.
#[inline(never)]
fn hoard_context_v1(
    custody_program: &Pubkey,
    hoard: &AccountInfo<'_>,
    window: &[AccountInfo<'_>],
    market: [u8; 32],
    release_set: [u8; 32],
) -> Result<[u8; 32], ProgramError> {
    // The Hoard's replay names the founding's custody context; the Hoard
    // vault must derive from it under `HoardPrincipal`.
    let context = read_replay_v1(window)?.context;
    let seeds =
        CustodyVaultSeedsV1::new(market, release_set, context, CompartmentV1::HoardPrincipal);
    if Pubkey::find_program_address(&seeds.as_slices(), custody_program).0 != *hoard.key {
        return Err(ScoringDealerErrorV1::Custody.into());
    }
    Ok(context)
}

/// The fill's three cash legs, in the order `cashLegs` names them, each built
/// and invoked out of line.
///
/// One [`CustodyLegV1`] is over three hundred bytes and three of them were
/// live in the route's own frame at once; `invoke_custody_transfer_v1` takes
/// the leg by value, so passing it by reference only moves the pointer and
/// leaves the temporary exactly where it was (measured: the frame did not
/// change by one byte). What moves it is the call boundary -- a stage's
/// locals are its own frame's, and this stage's are the three legs.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn settle_cash_legs_v1<'info>(
    program_id: &Pubkey,
    custody_program: &AccountInfo<'info>,
    hoard: &AccountInfo<'info>,
    windows: &[AccountInfo<'info>],
    facts: MarketFactsV1,
    market: [u8; 32],
    fund_context: [u8; 32],
    taker_account: &AccountInfo<'info>,
    taker: [u8; 32],
    parent: [u8; 32],
    legs: CashLegsV1,
) -> Result<(), ProgramError> {
    let (window_a, rest) = windows.split_at(FILL_CUSTODY_WINDOW_ACCOUNTS);
    let (window_b, window_c) = rest.split_at(FILL_CUSTODY_WINDOW_ACCOUNTS);
    let hoard_context = if legs.taker_to_hoard + legs.fund_to_hoard > 0 {
        hoard_context_v1(
            custody_program.key,
            hoard,
            window_b,
            market,
            facts.release_set,
        )?
    } else {
        [0; 32]
    };
    fund_to_hoard_leg_v1(
        program_id,
        custody_program,
        window_a,
        facts,
        market,
        fund_context,
        hoard_context,
        parent,
        legs.fund_to_hoard,
    )?;
    if legs.taker_to_hoard > 0 {
        taker_to_hoard_leg_v1(
            program_id,
            custody_program,
            window_b,
            facts,
            market,
            hoard_context,
            taker_account,
            taker,
            parent,
            legs.taker_to_hoard,
        )?;
    }
    if legs.fund_to_taker + legs.taker_to_fund > 0 {
        net_leg_v1(
            program_id,
            custody_program,
            window_c,
            facts,
            market,
            fund_context,
            taker_account,
            taker,
            parent,
            legs,
        )?;
    }
    Ok(())
}

/// Leg A: the fund's share of the mint's par, fund vault to Hoard.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn fund_to_hoard_leg_v1<'info>(
    program_id: &Pubkey,
    custody_program: &AccountInfo<'info>,
    window: &[AccountInfo<'info>],
    facts: MarketFactsV1,
    market: [u8; 32],
    fund_context: [u8; 32],
    hoard_context: [u8; 32],
    parent: [u8; 32],
    amount: u64,
) -> Result<(), ProgramError> {
    let replay = read_replay_v1(window)?;
    invoke_custody_transfer_v1(
        program_id,
        CustodyLegV1 {
            window,
            custody_program,
            replay,
            facts,
            market,
            context: fund_context,
            source_compartment: CompartmentV1::TradingPrincipal,
            destination_compartment: CompartmentV1::HoardPrincipal,
            source_owner: [0; 32],
            destination_owner: [0; 32],
            source_vault_context: fund_context,
            destination_vault_context: hoard_context,
            parent_request_digest: parent,
            transfer_index: 0,
            amount,
        },
        None,
    )
}

/// Leg B: the taker's share of the mint's par, taker to Hoard.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn taker_to_hoard_leg_v1<'info>(
    program_id: &Pubkey,
    custody_program: &AccountInfo<'info>,
    window: &[AccountInfo<'info>],
    facts: MarketFactsV1,
    market: [u8; 32],
    hoard_context: [u8; 32],
    taker_account: &AccountInfo<'info>,
    taker: [u8; 32],
    parent: [u8; 32],
    amount: u64,
) -> Result<(), ProgramError> {
    let replay = read_replay_v1(window)?;
    invoke_custody_transfer_v1(
        program_id,
        CustodyLegV1 {
            window,
            custody_program,
            replay,
            facts,
            market,
            context: hoard_context,
            source_compartment: CompartmentV1::External,
            destination_compartment: CompartmentV1::HoardPrincipal,
            source_owner: taker,
            destination_owner: [0; 32],
            source_vault_context: [0; 32],
            destination_vault_context: hoard_context,
            parent_request_digest: parent,
            transfer_index: 1,
            amount,
        },
        Some(taker_account),
    )
}

/// Leg C: the net, whichever way it goes -- fund to taker, or taker to fund.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn net_leg_v1<'info>(
    program_id: &Pubkey,
    custody_program: &AccountInfo<'info>,
    window: &[AccountInfo<'info>],
    facts: MarketFactsV1,
    market: [u8; 32],
    fund_context: [u8; 32],
    taker_account: &AccountInfo<'info>,
    taker: [u8; 32],
    parent: [u8; 32],
    legs: CashLegsV1,
) -> Result<(), ProgramError> {
    // The fund's replay advanced if leg A ran; re-read it.
    let replay = read_replay_v1(window)?;
    let to_taker = legs.fund_to_taker > 0;
    invoke_custody_transfer_v1(
        program_id,
        CustodyLegV1 {
            window,
            custody_program,
            replay,
            facts,
            market,
            context: fund_context,
            source_compartment: if to_taker {
                CompartmentV1::TradingPrincipal
            } else {
                CompartmentV1::External
            },
            destination_compartment: if to_taker {
                CompartmentV1::External
            } else {
                CompartmentV1::TradingPrincipal
            },
            source_owner: if to_taker { [0; 32] } else { taker },
            destination_owner: if to_taker { taker } else { [0; 32] },
            source_vault_context: if to_taker { fund_context } else { [0; 32] },
            destination_vault_context: if to_taker { [0; 32] } else { fund_context },
            parent_request_digest: parent,
            transfer_index: 2,
            amount: legs.fund_to_taker.max(legs.taker_to_fund),
        },
        if to_taker { None } else { Some(taker_account) },
    )
}

/// What the rule admitted and the Claims coordinates the fill priced against:
/// everything the route reads after the kernel runs, and nothing wider than a
/// word except the basis identity.
///
/// The inventory, [`AdmittedFill`] and the two decoded Claims views are
/// together well over half a kilobyte, and NONE of them is read once the rule
/// has admitted -- only these seven values are. Holding them in the route's
/// own frame is part of what put it over the 4,096 bytes SBPF v0 gives every
/// call (`super::authenticate_market_v1` states the bound); [`admit_fill_v1`]
/// is the boundary that keeps them in a frame of their own.
#[derive(Clone, Copy)]
struct FillAdmissionV1 {
    /// Claim units the Dealer pays for the fill.
    dealer_pays: u64,
    /// Claim units the Dealer receives.
    dealer_receives: u64,
    /// `Ŵ(inv′)`, the pair the fund carries forward.
    potential_after: Potential,
    /// The Dealer Position revision the rule priced against.
    dealer_revision: u64,
    /// The aggregate revision the rule priced against.
    aggregate_revision: u64,
    /// The aggregate's runtime claim count.
    aggregate_claim_count: u32,
    /// The aggregate's semantic `LiabilityBasisV2` identity.
    basis_id: [u8; 32],
}

/// Read the inventory the chain holds and put the fill to the rule, out of
/// line. See [`FillAdmissionV1`] for why the boundary is here.
#[inline(never)]
fn admit_fill_v1(
    claims_program: &Pubkey,
    aggregate: &AccountInfo<'_>,
    dealer_position: &AccountInfo<'_>,
    fund_key: &Pubkey,
    parameters: RuleParameters,
    outcome_count: u8,
    request: &DealerFillRequestV1,
) -> Result<FillAdmissionV1, ProgramError> {
    let (inventory, dealer_view, aggregate_view) = read_inventory_v1(
        claims_program,
        aggregate,
        dealer_position,
        fund_key,
        outcome_count,
    )?;
    let admitted = admit_fill(
        parameters,
        &inventory,
        &request.receive,
        &request.deliver,
        &request.prices,
    )
    .map_err(ScoringDealerErrorV1::from)?;
    Ok(FillAdmissionV1 {
        dealer_pays: admitted.dealer_pays,
        dealer_receives: admitted.dealer_receives,
        potential_after: admitted.potential_after,
        dealer_revision: dealer_view.revision,
        aggregate_revision: aggregate_view.revision,
        aggregate_claim_count: aggregate_view.claim_count,
        basis_id: aggregate_view.basis_id,
    })
}

fn next_fund_v1(
    fund: DealerFundV1,
    potential_after: Potential,
    pays_atoms: u64,
    receives_atoms: u64,
) -> Result<DealerFundV1, ProgramError> {
    let cash = fund
        .cash
        .checked_sub(pays_atoms)
        .ok_or(ScoringDealerErrorV1::Uncovered)?
        .checked_add(receives_atoms)
        .ok_or(ScoringDealerErrorV1::Overflow)?;
    Ok(DealerFundV1 {
        cash,
        inventory_minimum: potential_after.minimum,
        liquidity_cost: potential_after.cost,
        revision: fund
            .revision
            .checked_add(1)
            .ok_or(ScoringDealerErrorV1::Overflow)?,
        ..fund
    })
}

fn delta(value: i128) -> Result<SignedDeltaV3, ProgramError> {
    let magnitude =
        u64::try_from(value.unsigned_abs()).map_err(|_| ScoringDealerErrorV1::Overflow)?;
    let direction = if value > 0 {
        DeltaDirectionV3::Credit
    } else if value < 0 {
        DeltaDirectionV3::Debit
    } else {
        DeltaDirectionV3::Neutral
    };
    SignedDeltaV3::new(direction, magnitude).map_err(|_| ScoringDealerErrorV1::Claims.into())
}

/// Compose and invoke the one Claims signed delta of the fill.
///
/// The positions table is `[Dealer (owner = fund), taker]`; the aggregate
/// moves by `mint` on every ordinary outcome and not at all on the failure
/// coordinate (decision 0025 seats that column in the escrow at founding --
/// whether a mint must ALSO seat `mint` failure claims there is the
/// failure-arm family's ruling and is named in BUILD_DEALER.md §3); the row
/// deltas are `receive − deliver` and `mint + deliver − receive`.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn invoke_claims_fill_delta_v1<'info>(
    program_id: &Pubkey,
    claims_program: &AccountInfo<'info>,
    window: &[AccountInfo<'info>],
    facts: MarketFactsV1,
    fund: DealerFundV1,
    fund_key: &Pubkey,
    admission: FillAdmissionV1,
    taker_revision: u64,
    request: DealerFillRequestV1,
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let request_id = hash(instruction_data).to_bytes();
    let product_record_digest = {
        let data = get(window, 4)?
            .try_borrow_data()
            .map_err(|_| ScoringDealerErrorV1::Claims)?;
        hash(&data).to_bytes()
    };
    let linked_basis_record_digest = {
        let data = get(window, 2)?
            .try_borrow_data()
            .map_err(|_| ScoringDealerErrorV1::Claims)?;
        hash(&data).to_bytes()
    };
    let input = SignedDeltaPlanInputV3 {
        caller_role: CallerRole::Trading,
        release_set: facts.release_set,
        market: fund.market,
        request_id,
        product_record_digest,
        semantic_basis_id: admission.basis_id,
        linked_basis_record_digest,
        expected_market_revision: admission.aggregate_revision,
        claim_count: admission.aggregate_claim_count,
    };
    let dealer_position =
        SignedDeltaPositionV3::new(fund_key.to_bytes(), admission.dealer_revision)
            .map_err(|_| ScoringDealerErrorV1::Claims)?;
    let taker_position = SignedDeltaPositionV3::new(request.taker, taker_revision)
        .map_err(|_| ScoringDealerErrorV1::Claims)?;
    let (dealer_index, taker_index) =
        signed_delta_position_indexes_v1(dealer_position.owner(), taker_position.owner());
    let positions = if dealer_index == 0 {
        [dealer_position, taker_position]
    } else {
        [taker_position, dealer_position]
    };
    let width = usize::from(fund.outcome_count);
    let claim_count = admission.aggregate_claim_count;
    let mut aggregate_deltas = alloc::vec::Vec::with_capacity(claim_count as usize);
    for outcome in 0..claim_count as usize {
        aggregate_deltas.push(delta(if outcome < width {
            i128::from(request.mint)
        } else {
            0
        })?);
    }
    let mut rows = alloc::vec::Vec::with_capacity(2 * width);
    for outcome in 0..width {
        let outcome_index = u32::try_from(outcome).map_err(|_| ScoringDealerErrorV1::Overflow)?;
        for (position_index, value) in [
            (dealer_index, request.dealer_delta(outcome)),
            (taker_index, request.taker_delta(outcome)),
        ] {
            if value != 0 {
                rows.push(
                    PositionDeltaV3::new(
                        PositionDeltaInputV3 {
                            position_index,
                            outcome: outcome_index,
                            delta: delta(value)?,
                        },
                        2,
                        claim_count,
                    )
                    .map_err(|_| ScoringDealerErrorV1::Claims)?,
                );
            }
        }
    }
    if rows.is_empty() {
        // The null fill moves no claim; Claims is not invoked and the fund
        // still advances (the quote is what changed, and it is a projection).
        return Ok(());
    }
    let construction =
        ValidatedSignedDeltaConstructionV3::new(input, &positions, &aggregate_deltas, &rows)
            .map_err(|_| ScoringDealerErrorV1::Claims)?;
    let packet_bytes = construction
        .encoded_bytes()
        .map_err(|_| ScoringDealerErrorV1::Claims)?;
    let mut packet = alloc::vec![0_u8; packet_bytes];
    construction
        .encode_into(&mut packet)
        .map_err(|_| ScoringDealerErrorV1::Claims)?;
    let packet_digest = hash(&packet).to_bytes();
    // Claims authenticates `plan.request_id()` as the caller-authority context.
    // The plan binds that field to this Fill's exact instruction digest, so the
    // PDA we sign must use the same digest.  The fund key is a distinct
    // coordinate: putting it here made every non-null fill fail Claims Release
    // before any delta could be applied.
    let authority_seeds =
        claims_fill_authority_seeds_v1(facts.release_set, fund.market, request_id, packet_digest)?;
    let (authority, bump) = Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    if get(window, 0)?.key != &authority {
        return Err(ScoringDealerErrorV1::Release.into());
    }
    let metas = child_metas_v1(window, claims_signed_delta_privileges_v1)?;
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: packet.clone(),
    };
    let mut infos = alloc::vec::Vec::with_capacity(window.len() + 1);
    infos.extend_from_slice(window);
    infos.push(claims_program.clone());
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(crate::child_refused_v1)?;
    let (producer, receipt) = get_return_data().ok_or(ScoringDealerErrorV1::Claims)?;
    if producer != *claims_program.key {
        return Err(ScoringDealerErrorV1::Claims.into());
    }
    let receipt =
        SignedDeltaReceiptV3::decode(&receipt).map_err(|_| ScoringDealerErrorV1::Claims)?;
    let plan = SignedDeltaPlanV3::decode(&packet).map_err(|_| ScoringDealerErrorV1::Claims)?;
    receipt
        .validate_plan(plan)
        .map_err(|_| ScoringDealerErrorV1::Claims)?;
    if receipt.packet_digest() != packet_digest
        || receipt.claims_program() != claims_program.key.to_bytes()
    {
        return Err(ScoringDealerErrorV1::Claims.into());
    }
    Ok(())
}

fn claims_fill_authority_seeds_v1(
    release_set: [u8; 32],
    market: [u8; 32],
    fill_request_digest: [u8; 32],
    packet_digest: [u8; 32],
) -> Result<CallerAuthoritySeedsV1, ProgramError> {
    CallerAuthoritySeedsV1::from_bytes(
        release_set,
        market,
        ExecutionRoleV1::Trading,
        fill_request_digest,
        packet_digest,
    )
    .map_err(|_| ScoringDealerErrorV1::Release.into())
}

/// Map Dealer and taker semantics onto Claims' required ascending-owner table.
fn signed_delta_position_indexes_v1(dealer_owner: [u8; 32], taker_owner: [u8; 32]) -> (u32, u32) {
    if dealer_owner < taker_owner {
        (0, 1)
    } else {
        (1, 0)
    }
}

/// Claims window slots follow semantic-owner order, never Position PDA order.
const fn signed_delta_window_accounts_v1(dealer_index: u32) -> (usize, usize) {
    if dealer_index == 0 {
        (DELTA_DEALER_POSITION_ACCOUNT, DELTA_TAKER_POSITION_ACCOUNT)
    } else {
        (DELTA_TAKER_POSITION_ACCOUNT, DELTA_DEALER_POSITION_ACCOUNT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_delta_positions_are_canonical_in_both_semantic_orders() {
        assert_eq!(signed_delta_position_indexes_v1([1; 32], [2; 32]), (0, 1));
        assert_eq!(signed_delta_position_indexes_v1([2; 32], [1; 32]), (1, 0));
    }

    #[test]
    fn delta_window_follows_owners_when_position_pdas_sort_oppositely() {
        let (dealer_index, _) = signed_delta_position_indexes_v1([1; 32], [2; 32]);
        let dealer_position_pda = [9; 32];
        let taker_position_pda = [3; 32];
        assert!(dealer_position_pda > taker_position_pda);
        assert_eq!(
            signed_delta_window_accounts_v1(dealer_index),
            (DELTA_DEALER_POSITION_ACCOUNT, DELTA_TAKER_POSITION_ACCOUNT)
        );
    }

    #[test]
    fn claims_authority_context_is_the_fill_request_not_the_fund() {
        let release_set = [1; 32];
        let market = [2; 32];
        let fill_request = [3; 32];
        let fund = [4; 32];
        let packet = [5; 32];
        let program = Pubkey::new_unique();
        let accepted = claims_fill_authority_seeds_v1(release_set, market, fill_request, packet)
            .expect("the exact Fill request digest is a caller-authority context");
        let stale_fund_context = CallerAuthoritySeedsV1::from_bytes(
            release_set,
            market,
            ExecutionRoleV1::Trading,
            fund,
            packet,
        )
        .expect("the hostile alternative is structurally encodable");
        assert_ne!(
            Pubkey::find_program_address(&accepted.as_slices(), &program).0,
            Pubkey::find_program_address(&stale_fund_context.as_slices(), &program).0,
            "a fund-context PDA cannot satisfy Claims' request-id authentication"
        );
    }

    #[test]
    fn the_cash_legs_are_the_leans() {
        assert_eq!(
            cash_legs_v1(300, 100, 0),
            CashLegsV1 {
                taker_to_hoard: 200,
                fund_to_hoard: 100,
                fund_to_taker: 0,
                taker_to_fund: 0
            }
        );
        assert_eq!(
            cash_legs_v1(300, 400, 0),
            CashLegsV1 {
                taker_to_hoard: 0,
                fund_to_hoard: 300,
                fund_to_taker: 100,
                taker_to_fund: 0
            }
        );
        assert_eq!(
            cash_legs_v1(300, 0, 50),
            CashLegsV1 {
                taker_to_hoard: 300,
                fund_to_hoard: 0,
                fund_to_taker: 0,
                taker_to_fund: 50
            }
        );
        for (m, p) in [(0_u64, 0_u64), (7, 3), (3, 7), (1_000, 1_000)] {
            let legs = cash_legs_v1(m, p, 0);
            assert_eq!(legs.taker_to_hoard + legs.fund_to_hoard, m);
            assert_eq!(legs.fund_to_hoard + legs.fund_to_taker, p);
        }
    }
}
