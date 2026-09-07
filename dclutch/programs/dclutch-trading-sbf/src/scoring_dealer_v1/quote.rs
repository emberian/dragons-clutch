//! `DealerQuote` (`DCLSQTR1`): write `p̂(inv)` into the Dealer's quote
//! account, permissionlessly. The price series becomes a chain fact
//! (decision 0031 §5 defect 6: it lived only in a candidate account).
//!
//! Frame: the Lean's `quoteFrame` (6) and no child window. The quote is
//! derived from the Position the chain holds, never from the fund's cache,
//! and bound to the fund revision so a reader can tell fresh from stale.

use dclutch_market::Phase;
use dclutch_trading::scoring_rule::records_v1::DealerQuoteV1;
use dclutch_trading::scoring_rule::requests_v1::{
    DealerQuoteRequestV1, DealerRouteV1, QUOTE_FRAME_ACCOUNTS, quote_privileges_v1,
};
use dclutch_trading::scoring_rule::{generated, prices_of};
use solana_program::{
    account_info::AccountInfo, clock::Clock, hash::hash, program_error::ProgramError,
    pubkey::Pubkey, sysvar::Sysvar,
};

use super::{
    ScoringDealerErrorV1, authenticate_fund_v1, authenticate_market_v1, authenticate_release_v1,
    authenticate_rule_v1, emit_receipt_v1, get, parse_prefix, write_quote_v1,
};
use crate::hot_v3::hot_cu_checkpoint_macro as hot_cu_checkpoint;

/// Execute one quote.
#[inline(never)]
pub fn process_dealer_quote_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let request = DealerQuoteRequestV1::decode(instruction_data)
        .map_err(|_| ScoringDealerErrorV1::Request)?;
    let (prefix, _) = parse_prefix(accounts, QUOTE_FRAME_ACCOUNTS, 0, quote_privileges_v1)?;
    hot_cu_checkpoint!("scoring-dealer:quote:frame");
    let fund_account = get(prefix, generated::QUOTE_FUND_ACCOUNT)?;
    let fund = authenticate_fund_v1(
        program_id,
        fund_account,
        request.market,
        request.dealer_id,
        request.expected_fund_revision,
    )?;
    let rule = authenticate_rule_v1(
        program_id,
        get(prefix, generated::QUOTE_RULE_ACCOUNT)?,
        fund,
    )?;
    // A quote is a projection: the Market may be Open or past it; a retired
    // Market quotes nothing because its Claims are gone, which the Position
    // read refuses for us.
    let facts = authenticate_market_v1(
        get(prefix, generated::QUOTE_MARKET_ACCOUNT)?,
        request.market,
        &[Phase::Open, Phase::Terminal, Phase::Retiring],
    )?;
    // The price this route writes is a CHAIN fact, so the account it derives it
    // from has to be a Claims Position and not merely shaped like one. The
    // release waist is what says which program owns Positions here.
    let claims_program = authenticate_release_v1(
        program_id,
        get(prefix, generated::QUOTE_REGISTRY_PROGRAM_ACCOUNT)?,
        get(prefix, generated::QUOTE_ACTIVATION_CACHE_ACCOUNT)?,
        facts,
    )?;
    hot_cu_checkpoint!("scoring-dealer:quote:authenticated");
    // The quote frame carries no aggregate: the Position's own `market_account`
    // is the aggregate it belongs to, and the Position is authenticated
    // against the fund's owner PDA under the Claims program that owns it.
    let position = get(prefix, generated::QUOTE_DEALER_POSITION_ACCOUNT)?;
    let inventory = read_inventory_for_withdraw_v1(
        &claims_program,
        position,
        fund_account.key,
        fund.outcome_count,
    )?;
    let prices = prices_of(rule.parameters, &inventory).map_err(ScoringDealerErrorV1::from)?;
    hot_cu_checkpoint!("scoring-dealer:quote:priced");
    let quote_account = get(prefix, generated::QUOTE_QUOTE_ACCOUNT)?;
    let expected = Pubkey::find_program_address(
        &DealerQuoteV1::seeds(&fund.market, &fund.dealer_id),
        program_id,
    );
    if quote_account.owner != program_id || expected.0 != *quote_account.key {
        return Err(ScoringDealerErrorV1::Quote.into());
    }
    let slot = Clock::get().map_err(|_| ScoringDealerErrorV1::Quote)?.slot;
    write_quote_v1(
        quote_account,
        DealerQuoteV1 {
            outcome_count: fund.outcome_count,
            market: fund.market,
            dealer_id: fund.dealer_id,
            fund_revision: fund.revision,
            slot,
            scale: rule.parameters.scale,
            prices,
            bump: expected.1,
        },
    )?;
    let fund_bytes = fund.to_bytes().map_err(|_| ScoringDealerErrorV1::Commit)?;
    debug_assert_eq!(
        hash(&fund_bytes).to_bytes(),
        hash(
            &fund_account
                .try_borrow_data()
                .map_err(|_| ScoringDealerErrorV1::Commit)?
        )
        .to_bytes()
    );
    emit_receipt_v1(
        DealerRouteV1::Quote,
        instruction_data,
        fund,
        &fund_bytes,
        0,
        0,
    )
}

/// The inventory read the quote and withdraw frames afford: the Position
/// alone, its aggregate named by its own header. Everything `read_inventory_v1`
/// proves except the aggregate's width, which the Position's `claim_count`
/// carries.
///
/// `claims_program` is the release's Claims role, not the account's own
/// `owner`. Deriving the Position's address under the owner it claims for
/// itself proves nothing: any program can mint an account that satisfies a
/// derivation under itself, and for `DealerWithdraw` that account decides the
/// floor -- a forged inventory raises `Φ` and lets the sponsor take the whole
/// vault, subsidy and all.
pub(super) fn read_inventory_for_withdraw_v1(
    claims_program: &Pubkey,
    position: &AccountInfo<'_>,
    fund_key: &Pubkey,
    outcome_count: u8,
) -> Result<dclutch_trading::scoring_rule::Vector, ProgramError> {
    use dclutch_claims::liability_basis_state_v2::LiabilityBasisPositionViewV2;
    use dclutch_claims::protocol_position_v2::ProtocolPositionSeedsV2;
    if position.owner != claims_program {
        return Err(ScoringDealerErrorV1::Position.into());
    }
    let data = position
        .try_borrow_data()
        .map_err(|_| ScoringDealerErrorV1::Position)?;
    let view =
        LiabilityBasisPositionViewV2::decode(&data).map_err(|_| ScoringDealerErrorV1::Position)?;
    if view.owner != fund_key.to_bytes() || view.claim_count < u32::from(outcome_count) {
        return Err(ScoringDealerErrorV1::Position.into());
    }
    let seeds = ProtocolPositionSeedsV2::new(view.market_account, fund_key.to_bytes())
        .map_err(|_| ScoringDealerErrorV1::Position)?;
    if Pubkey::find_program_address(&seeds.as_slices(), claims_program).0 != *position.key {
        return Err(ScoringDealerErrorV1::Position.into());
    }
    let mut inventory = dclutch_trading::scoring_rule::ZERO_VECTOR;
    for (index, slot) in inventory
        .iter_mut()
        .enumerate()
        .take(usize::from(outcome_count))
    {
        let claim = u32::try_from(index).map_err(|_| ScoringDealerErrorV1::Overflow)?;
        *slot = view
            .balance(&data, claim)
            .map_err(|_| ScoringDealerErrorV1::Position)?;
    }
    Ok(inventory)
}
