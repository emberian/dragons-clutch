//! Redeem the fund's Claims through Claims' terminal owner. The only collateral
//! destination is the same TradingPrincipal vault; native founder-bond proceeds
//! enter the fund account separately and never increase collateral cash.

extern crate alloc;
use alloc::vec::Vec;

use dclutch_claims::{
    CallerRole,
    terminal_settlement_v3::{
        TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3, TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_WITH_FOUNDER_BOND_ACCOUNT_COUNT_V3, TerminalRecipientV3,
        TerminalSettlementReceiptV3, TerminalSettlementRequestV3,
        terminal_settlement_privileges_v3,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_custody::token_svm::TokenAccount;
use dclutch_market::Phase;
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading::scoring_rule::{
    generated as g, potential,
    records_v1::DealerFundV1,
    requests_v1::{DealerRedeemRequestV1, DealerRouteV1, redeem_privileges_v1},
};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use super::{
    ScoringDealerErrorV1 as Error, authenticate_fund_v1, authenticate_market_v1,
    authenticate_release_v1, authenticate_rule_v1, authenticate_vault_v1, commit_fund_v1,
    emit_receipt_v1, get, parse_prefix,
};

/// Permissionless terminal redemption, with the fund's optimistic revision.
#[inline(never)]
pub fn process_dealer_redeem_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    bytes: &[u8],
) -> Result<(), ProgramError> {
    let parent_bytes = bytes.get(..g::REDEEM_REQUEST_BYTES).ok_or(Error::Request)?;
    let parent = DealerRedeemRequestV1::decode(parent_bytes).map_err(|_| Error::Request)?;
    let child_bytes = bytes.get(g::REDEEM_REQUEST_BYTES..).ok_or(Error::Request)?;
    let child = TerminalSettlementRequestV3::decode(child_bytes).map_err(|_| Error::Request)?;
    let child_width = accounts
        .len()
        .checked_sub(g::REDEEM_ACCOUNT_COUNT)
        .ok_or(Error::Frame)?;
    if ![
        TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3,
        TERMINAL_SETTLEMENT_WITH_FOUNDER_BOND_ACCOUNT_COUNT_V3,
    ]
    .contains(&child_width)
    {
        return Err(Error::Frame.into());
    }
    let (prefix, window) = parse_prefix(
        accounts,
        g::REDEEM_ACCOUNT_COUNT,
        child_width,
        redeem_privileges_v1,
    )?;
    let fund_account = get(prefix, g::REDEEM_FUND_ACCOUNT)?;
    let fund = authenticate_fund_v1(
        program_id,
        fund_account,
        parent.market,
        parent.dealer_id,
        parent.expected_fund_revision,
    )?;
    let rule = authenticate_rule_v1(program_id, get(prefix, g::REDEEM_RULE_ACCOUNT)?, fund)?;
    let facts = authenticate_market_v1(
        get(prefix, g::REDEEM_MARKET_ACCOUNT)?,
        parent.market,
        &[Phase::Terminal, Phase::Retiring],
    )?;
    let claims = get(prefix, g::REDEEM_CLAIMS_PROGRAM_ACCOUNT)?;
    if authenticate_release_v1(
        program_id,
        get(prefix, g::REDEEM_REGISTRY_PROGRAM_ACCOUNT)?,
        get(prefix, g::REDEEM_ACTIVATION_CACHE_ACCOUNT)?,
        facts,
    )? != *claims.key
        || !claims.executable
    {
        return Err(Error::Release.into());
    }
    let custody = get(prefix, g::REDEEM_CUSTODY_PROGRAM_ACCOUNT)?;
    let vault = get(prefix, g::REDEEM_VAULT_ACCOUNT)?;
    authenticate_vault_v1(
        custody.key,
        vault,
        fund,
        fund_account.key,
        facts.release_set,
    )?;
    authenticate_redemption_child(
        child,
        fund,
        fund_account.key.to_bytes(),
        facts.release_set,
        facts.realm,
        facts.generation,
        claims.key.to_bytes(),
        custody.key.to_bytes(),
    )?;
    if get(window, TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3)?.key != vault.key {
        return Err(Error::RedeemDestination.into());
    }
    let before_cash = token_amount(vault)?;
    if before_cash < fund.cash {
        return Err(Error::Custody.into());
    }
    let payout = invoke_terminal_child(program_id, claims, window, child_bytes, &child)?;
    let after_cash = token_amount(vault)?;
    if before_cash.checked_add(payout) != Some(after_cash) {
        return Err(Error::RedeemCashPoststate.into());
    }
    let inventory = super::quote::read_inventory_for_withdraw_v1(
        claims.key,
        get(window, 20)?,
        fund_account.key,
        fund.outcome_count,
    )?;
    let w = potential(rule.parameters, &inventory).map_err(Error::from)?;
    let next = DealerFundV1 {
        cash: fund.cash.checked_add(payout).ok_or(Error::Overflow)?,
        inventory_minimum: w.minimum,
        liquidity_cost: w.cost,
        revision: fund.revision.checked_add(1).ok_or(Error::Overflow)?,
        ..fund
    };
    let old = fund.to_bytes().map_err(|_| Error::Commit)?;
    let committed = commit_fund_v1(fund_account, hash(&old).to_bytes(), next)?;
    // The receipt's flow fields are fill-only claim units. Terminal atoms
    // are evidenced by the child receipt and the resulting fund cash.
    emit_receipt_v1(DealerRouteV1::Redeem, bytes, next, &committed, 0, 0)
}

/// Keep the child request/receipt and CPI seed temporaries out of the fund
/// accounting frame. Claims remains the only payout evaluator.
#[inline(never)]
fn invoke_terminal_child(
    program_id: &Pubkey,
    claims: &AccountInfo<'_>,
    window: &[AccountInfo<'_>],
    child_bytes: &[u8],
    child: &TerminalSettlementRequestV3,
) -> Result<u64, ProgramError> {
    let child_width = window.len();
    let input = child.input();
    let child_digest = hash(child_bytes).to_bytes();
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(input.release_set).map_err(|_| Error::Release)?,
        input.market,
        ExecutionRoleV1::Trading,
        input.parent_context,
        child_digest,
    )
    .map_err(|_| Error::Claims)?;
    let (authority, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if get(window, 0)?.key != &authority {
        return Err(Error::Claims.into());
    }
    let bond = child_width == TERMINAL_SETTLEMENT_WITH_FOUNDER_BOND_ACCOUNT_COUNT_V3;
    let metas = window
        .iter()
        .enumerate()
        .map(|(index, account)| {
            let (writable, signer) =
                terminal_settlement_privileges_v3(index, bond).ok_or(Error::Frame)?;
            if writable && !account.is_writable {
                return Err(Error::Frame);
            }
            Ok(if writable {
                AccountMeta::new(*account.key, signer)
            } else {
                AccountMeta::new_readonly(*account.key, signer)
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let instruction = Instruction {
        program_id: *claims.key,
        accounts: metas,
        data: child_bytes.to_vec(),
    };
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = seeds.as_slices();
    invoke_signed(
        &instruction,
        window,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(crate::child_refused_v1)?;
    let (return_program, receipt_bytes) = get_return_data().ok_or(Error::Claims)?;
    let receipt = TerminalSettlementReceiptV3::decode(&receipt_bytes).map_err(|_| Error::Claims)?;
    if return_program != *claims.key
        || receipt.request() != *child
        || receipt.evidence().request_digest != child_digest
    {
        return Err(Error::Claims.into());
    }
    Ok(receipt.evidence().payout)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_redemption_child(
    child: TerminalSettlementRequestV3,
    fund: DealerFundV1,
    fund_key: [u8; 32],
    release_set: [u8; 32],
    realm: [u8; 32],
    generation: u64,
    claims: [u8; 32],
    custody: [u8; 32],
) -> Result<(), ProgramError> {
    let input = child.input();
    if child.recipient_mode() != TerminalRecipientV3::TradingPrincipal
        || input.owner != fund_key
        || input.recipient_token_account != fund.vault
    {
        return Err(Error::RedeemDestination.into());
    }
    if input.caller_role != CallerRole::Trading
        || input.parent_context != fund_key
        || input.market != fund.market
        || input.release_set != release_set
        || input.realm != realm
        || input.generation != generation
        || input.claims_program != claims
        || input.custody_program != custody
    {
        return Err(Error::RedeemContext.into());
    }
    Ok(())
}

fn token_amount(account: &AccountInfo<'_>) -> Result<u64, ProgramError> {
    let bytes = account.try_borrow_data().map_err(|_| Error::Custody)?;
    TokenAccount::parse_base_or_immutable_owner(&bytes)
        .map(|token| token.amount)
        .map_err(|_| Error::Custody.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_claims::terminal_settlement_v3::TerminalSettlementRequestInputV3;
    use dclutch_trading::scoring_rule::records_v1::FundPhaseV1;

    fn fixture() -> (DealerFundV1, TerminalSettlementRequestV3) {
        let fund = DealerFundV1 {
            outcome_count: 2,
            phase: FundPhaseV1::Open,
            market: [1; 32],
            dealer_id: [2; 32],
            sponsor: [3; 32],
            rule_digest: [4; 32],
            vault: [5; 32],
            claim_unit_atoms: 1,
            cash: 100,
            inventory_minimum: 0,
            liquidity_cost: 101,
            revision: 4,
            bump: 255,
        };
        let child = TerminalSettlementRequestV3::new(TerminalSettlementRequestInputV3 {
            caller_role: CallerRole::Trading,
            release_set: [6; 32],
            market: fund.market,
            realm: [7; 32],
            parent_context: [8; 32],
            product_record_digest: [9; 32],
            exposure_id: [10; 32],
            exposure_digest: [11; 32],
            terminal_record_digest: [12; 32],
            owner: [8; 32],
            position: [13; 32],
            recipient_owner: [14; 32],
            recipient_token_account: fund.vault,
            claims_program: [15; 32],
            custody_program: [16; 32],
            collateral_mint: [17; 32],
            token_program: [18; 32],
            semantic_basis_id: [19; 32],
            linked_basis_record_digest: [20; 32],
            generation: 1,
            expected_market_revision: 2,
            expected_position_revision: 3,
            expected_custody_revision: 4,
            quantity: 9,
            claim_index: 1,
            transfer_index: 0,
        })
        .unwrap()
        .to_trading_principal()
        .unwrap();
        (fund, child)
    }

    fn authenticate(
        child: TerminalSettlementRequestV3,
        fund: DealerFundV1,
    ) -> Result<(), ProgramError> {
        authenticate_redemption_child(
            child, fund, [8; 32], [6; 32], [7; 32], 1, [15; 32], [16; 32],
        )
    }

    #[test]
    fn redemption_child_returns_capital_to_its_own_fund() {
        let (fund, child) = fixture();
        assert_eq!(authenticate(child, fund), Ok(()));
        let external = TerminalSettlementRequestV3::new(child.input()).unwrap();
        assert_eq!(
            authenticate(external, fund),
            Err(Error::RedeemDestination.into())
        );
        for substitution in [0, 1] {
            let mut input = child.input();
            if substitution == 0 {
                input.owner = [21; 32];
            } else {
                input.recipient_token_account = [21; 32];
            }
            let wrong = TerminalSettlementRequestV3::new(input)
                .unwrap()
                .to_trading_principal()
                .unwrap();
            assert_eq!(
                authenticate(wrong, fund),
                Err(Error::RedeemDestination.into())
            );
        }
    }

    #[test]
    fn redemption_child_binds_context_release_market_and_programs() {
        let (fund, child) = fixture();
        for substitution in 0..7 {
            let mut input = child.input();
            match substitution {
                0 => input.parent_context = [21; 32],
                1 => input.release_set = [21; 32],
                2 => input.market = [21; 32],
                3 => input.realm = [21; 32],
                4 => input.generation += 1,
                5 => input.claims_program = [21; 32],
                _ => input.custody_program = [21; 32],
            }
            let wrong = TerminalSettlementRequestV3::new(input)
                .unwrap()
                .to_trading_principal()
                .unwrap();
            assert_eq!(authenticate(wrong, fund), Err(Error::RedeemContext.into()));
        }
    }
}
