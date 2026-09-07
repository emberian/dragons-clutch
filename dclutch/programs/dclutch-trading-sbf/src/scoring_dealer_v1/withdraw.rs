//! `DealerWithdraw` (`DCLSWDR1`): the sponsor takes cash out down to the
//! floor `Φ = cash + Ŵ(inv)` and not one atom more (`withdraw_floor`).
//!
//! Frame: the Lean's `withdrawFrame` (14) then one Custody `Transfer` window
//! (14): the fund's `TradingPrincipal` vault → the sponsor's token account.
//!
//! Admitted while the Market is Open, where the floor is Φ (profit, or the
//! subsidy the inventory no longer needs, never the part the next fill's R3
//! has already promised), and after it is terminal, where there is no next
//! fill and the floor is zero -- the residue, paid to the sponsor of record
//! (BUILD_DEALER.md §4.2).

use dclutch_custody::CompartmentV1;
use dclutch_market::Phase;
use dclutch_trading::scoring_rule::records_v1::DealerFundV1;
use dclutch_trading::scoring_rule::requests_v1::{
    DealerRouteV1, DealerWithdrawRequestV1, WITHDRAW_FRAME_ACCOUNTS, withdraw_privileges_v1,
};
use dclutch_trading::scoring_rule::{generated, potential, withdraw_admissible};
use solana_program::{
    account_info::AccountInfo, hash::hash, program_error::ProgramError, pubkey::Pubkey,
};

use super::{
    CustodyLegV1, ScoringDealerErrorV1, authenticate_fund_v1, authenticate_market_v1,
    authenticate_release_v1, authenticate_rule_v1, authenticate_vault_v1, commit_fund_v1,
    emit_receipt_v1, get, invoke_custody_transfer_v1, parse_prefix, read_replay_v1,
};
use crate::hot_v3::hot_cu_checkpoint_macro as hot_cu_checkpoint;

/// The one Custody window this route carries.
pub const WITHDRAW_CUSTODY_WINDOW_ACCOUNTS: usize =
    dclutch_custody::TRANSFER_ACCOUNT_COUNT_V1 as usize;

/// Execute one withdrawal.
#[inline(never)]
pub fn process_dealer_withdraw_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let request = DealerWithdrawRequestV1::decode(instruction_data)
        .map_err(|_| ScoringDealerErrorV1::Request)?;
    let (prefix, windows) = parse_prefix(
        accounts,
        WITHDRAW_FRAME_ACCOUNTS,
        WITHDRAW_CUSTODY_WINDOW_ACCOUNTS,
        withdraw_privileges_v1,
    )?;
    hot_cu_checkpoint!("scoring-dealer:withdraw:frame");
    let sponsor = get(prefix, generated::WITHDRAW_SPONSOR_ACCOUNT)?;
    let fund_account = get(prefix, generated::WITHDRAW_FUND_ACCOUNT)?;
    let fund = authenticate_fund_v1(
        program_id,
        fund_account,
        request.market,
        request.dealer_id,
        request.expected_fund_revision,
    )?;
    if sponsor.key.to_bytes() != fund.sponsor {
        return Err(ScoringDealerErrorV1::Sponsor.into());
    }
    let rule = authenticate_rule_v1(
        program_id,
        get(prefix, generated::WITHDRAW_RULE_ACCOUNT)?,
        fund,
    )?;
    let facts = authenticate_market_v1(
        get(prefix, generated::WITHDRAW_MARKET_ACCOUNT)?,
        request.market,
        &[
            Phase::Open,
            Phase::Terminal,
            Phase::Retiring,
            Phase::Retired,
        ],
    )?;
    let claims_program = authenticate_release_v1(
        program_id,
        get(prefix, generated::WITHDRAW_REGISTRY_PROGRAM_ACCOUNT)?,
        get(prefix, generated::WITHDRAW_ACTIVATION_CACHE_ACCOUNT)?,
        facts,
    )?;
    let custody_program = get(prefix, generated::WITHDRAW_CUSTODY_PROGRAM_ACCOUNT)?;
    let vault = get(prefix, generated::WITHDRAW_VAULT_ACCOUNT)?;
    authenticate_vault_v1(
        custody_program.key,
        vault,
        fund,
        fund_account.key,
        facts.release_set,
    )?;
    hot_cu_checkpoint!("scoring-dealer:withdraw:authenticated");

    // The floor, and the one phase that lifts it.
    //
    // While the Market is Open the floor is `Φ = cash + Ŵ(inv)`: `Ŵ` is what
    // the NEXT fill's debit is measured against (R3), so cash below Φ is cash
    // the rule has already promised. Once the Market is terminal there is no
    // next fill -- `DealerFill` admits `Phase::Open` and nothing else -- and
    // Ŵ of the redeemed, empty inventory is `−Ŝ`, so a floor of Φ would strand
    // exactly the subsidy in the vault for the life of the chain with no party
    // it could ever be paid to. The residue is the whole balance
    // (BUILD_DEALER.md §4.2; the note's §3(e) "never the committed part" is a
    // statement about a market that can still trade).
    //
    // PROVISIONAL RULING, convergence lane, reversible: the floor is Φ under
    // `Phase::Open` and zero under `Terminal`/`Retiring`/`Retired`.
    let inventory = super::quote::read_inventory_for_withdraw_v1(
        &claims_program,
        get(prefix, generated::WITHDRAW_DEALER_POSITION_ACCOUNT)?,
        fund_account.key,
        fund.outcome_count,
    )?;
    let w = potential(rule.parameters, &inventory).map_err(ScoringDealerErrorV1::from)?;
    let amount_claims = request.amount / fund.claim_unit_atoms;
    // Round the request UP to claim units when it is not a multiple, so a
    // withdrawal can never take a fraction of a claim unit the floor did not
    // cover.
    let amount_claims = if amount_claims
        .checked_mul(fund.claim_unit_atoms)
        .ok_or(ScoringDealerErrorV1::Overflow)?
        < request.amount
    {
        amount_claims
            .checked_add(1)
            .ok_or(ScoringDealerErrorV1::Overflow)?
    } else {
        amount_claims
    };
    if facts.phase == Phase::Open {
        withdraw_admissible(fund.cash_claims(), w, amount_claims)
            .map_err(ScoringDealerErrorV1::from)?;
    }
    hot_cu_checkpoint!("scoring-dealer:withdraw:floor");

    let replay = read_replay_v1(windows)?;
    invoke_custody_transfer_v1(
        program_id,
        CustodyLegV1 {
            window: windows,
            custody_program,
            replay,
            facts,
            market: fund.market,
            context: fund_account.key.to_bytes(),
            source_compartment: CompartmentV1::TradingPrincipal,
            destination_compartment: CompartmentV1::External,
            source_owner: [0; 32],
            destination_owner: sponsor.key.to_bytes(),
            source_vault_context: fund_account.key.to_bytes(),
            destination_vault_context: [0; 32],
            parent_request_digest: hash(instruction_data).to_bytes(),
            transfer_index: 0,
            amount: request.amount,
        },
    )?;
    hot_cu_checkpoint!("scoring-dealer:withdraw:custody");

    let before = fund.to_bytes().map_err(|_| ScoringDealerErrorV1::Commit)?;
    let next = DealerFundV1 {
        cash: fund
            .cash
            .checked_sub(request.amount)
            .ok_or(ScoringDealerErrorV1::WithdrawBelowFloor)?,
        inventory_minimum: w.minimum,
        liquidity_cost: w.cost,
        revision: fund
            .revision
            .checked_add(1)
            .ok_or(ScoringDealerErrorV1::Overflow)?,
        ..fund
    };
    let fund_bytes = commit_fund_v1(fund_account, hash(&before).to_bytes(), next)?;
    emit_receipt_v1(
        DealerRouteV1::Withdraw,
        instruction_data,
        next,
        &fund_bytes,
        0,
        0,
    )
}
