//! Physical close of a drained terminal Dealer through each resource's owner.
extern crate alloc;
use super::{MarketFactsV1, ScoringDealerErrorV1 as Error, get};
use dclutch_claims::{
    frame_spec_v1::ClaimsFrameSpecV1,
    protocol_position_v2::{
        PROTOCOL_POSITION_REQUEST_BYTES_V2, ProtocolPositionActionV2,
        ProtocolPositionCloseReceiptV2, ProtocolPositionOwnerKindV2, ProtocolPositionRequestV2,
    },
};
use dclutch_custody::{
    CLOSE_REPLAY_ACCOUNT_COUNT_V1, CLOSE_VAULT_ACCOUNT_COUNT_V1, CallerRoleV1, CompartmentV1,
    ContextV1, CustodyReceiptV1, CustodyReplayV1, CustodyRequestV1,
};
use dclutch_market::Phase;
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading::scoring_rule::{
    generated as g,
    records_v1::{DealerFundV1, DealerQuoteV1},
    requests_v1::{DealerCloseRequestV1, DealerRouteV1, close_privileges_v1},
};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::Instruction,
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

/// Claims owns the physical Position-close frame.
pub const CLAIMS_CLOSE_ACCOUNTS: usize =
    dclutch_claims::frame_spec_v1::PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V1 as usize;
/// Two existing Custody operations follow Claims Close.
pub const CLOSE_WINDOW_ACCOUNTS: usize = CLAIMS_CLOSE_ACCOUNTS
    + CLOSE_VAULT_ACCOUNT_COUNT_V1 as usize
    + CLOSE_REPLAY_ACCOUNT_COUNT_V1 as usize;

/// Close requires all economic assets gone before rent-bearing records disappear.
#[inline(never)]
pub fn process_dealer_close_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    bytes: &[u8],
) -> Result<(), ProgramError> {
    if bytes.len() != g::CLOSE_REQUEST_BYTES + PROTOCOL_POSITION_REQUEST_BYTES_V2 {
        return Err(Error::Request.into());
    }
    let parent = DealerCloseRequestV1::decode(&bytes[..g::CLOSE_REQUEST_BYTES])
        .map_err(|_| Error::Request)?;
    let child_bytes = &bytes[g::CLOSE_REQUEST_BYTES..];
    let child = ProtocolPositionRequestV2::decode(child_bytes).map_err(|_| Error::Request)?;
    let (prefix, windows) = super::parse_prefix(
        accounts,
        g::CLOSE_ACCOUNT_COUNT,
        CLOSE_WINDOW_ACCOUNTS,
        close_privileges_v1,
    )?;
    let fund_account = get(prefix, g::CLOSE_FUND_ACCOUNT)?;
    let fund = super::authenticate_fund_v1(
        program_id,
        fund_account,
        parent.market,
        parent.dealer_id,
        parent.expected_fund_revision,
    )?;
    let facts = super::authenticate_market_v1(
        get(prefix, g::CLOSE_MARKET_ACCOUNT)?,
        parent.market,
        &[Phase::Terminal, Phase::Retiring],
    )?;
    let claims = get(prefix, g::CLOSE_CLAIMS_PROGRAM_ACCOUNT)?;
    if super::authenticate_release_v1(
        program_id,
        get(prefix, g::CLOSE_REGISTRY_PROGRAM_ACCOUNT)?,
        get(prefix, g::CLOSE_ACTIVATION_CACHE_ACCOUNT)?,
        facts,
    )? != *claims.key
        || !claims.executable
    {
        return Err(Error::Release.into());
    }
    let rule = get(prefix, g::CLOSE_RULE_ACCOUNT)?;
    super::authenticate_rule_v1(program_id, rule, fund)?;
    let quote = get(prefix, g::CLOSE_QUOTE_ACCOUNT)?;
    authenticate_quote(program_id, quote, fund)?;
    let sponsor = get(prefix, g::CLOSE_SPONSOR_ACCOUNT)?;
    let credit = get(prefix, g::CLOSE_RENT_CREDIT_ACCOUNT)?;
    if sponsor.key.to_bytes() != fund.sponsor
        || credit.key.to_bytes() != facts.rent_beneficiary
        || sponsor.key == credit.key
    {
        return Err(Error::CloseBeneficiary.into());
    }
    let vault = get(prefix, g::CLOSE_VAULT_ACCOUNT)?;
    let custody = get(prefix, g::CLOSE_CUSTODY_PROGRAM_ACCOUNT)?;
    super::authenticate_vault_v1(
        custody.key,
        vault,
        fund,
        fund_account.key,
        facts.release_set,
    )?;
    let actual_cash = super::redeem::token_amount(vault)?;
    require_cash_closed(fund.cash, actual_cash)?;
    let (claims_window, custody_windows) = windows.split_at(CLAIMS_CLOSE_ACCOUNTS);
    let inventory = super::quote::read_inventory_for_withdraw_v1(
        claims.key,
        get(claims_window, 2)?,
        fund_account.key,
        fund.outcome_count,
    )?;
    if inventory.iter().any(|v| *v != 0) {
        return Err(Error::CloseInventory.into());
    }
    authenticate_child(
        child,
        fund,
        fund_account.key.to_bytes(),
        facts,
        credit.key.to_bytes(),
        get(prefix, g::CLOSE_RENT_PROGRAM_ACCOUNT)?.key.to_bytes(),
    )?;
    invoke_claims_close(program_id, claims, claims_window, child_bytes, child)?;
    let (vault_window, replay_window) =
        custody_windows.split_at(CLOSE_VAULT_ACCOUNT_COUNT_V1 as usize);
    let digest = hash(bytes).to_bytes();
    close_custody(
        program_id,
        custody,
        vault_window,
        prefix,
        fund_account.key,
        facts,
        digest,
        true,
    )?;
    close_custody(
        program_id,
        custody,
        replay_window,
        prefix,
        fund_account.key,
        facts,
        digest,
        false,
    )?;
    // Only the fund's sponsor paid for these Trading records. Native proceeds
    // held by the fund return separately from the already-withdrawn token cash.
    close_record(rule, sponsor, program_id)?;
    close_record(quote, sponsor, program_id)?;
    close_record(fund_account, sponsor, program_id)?;
    super::emit_receipt_v1(
        DealerRouteV1::Close,
        bytes,
        DealerFundV1 {
            revision: fund.revision.checked_add(1).ok_or(Error::Overflow)?,
            ..fund
        },
        &[],
        0,
        0,
    )
}

fn require_cash_closed(recorded: u64, actual: u64) -> Result<(), ProgramError> {
    if recorded != 0 || actual != 0 {
        Err(Error::CloseCash.into())
    } else {
        Ok(())
    }
}

#[inline(never)]
fn authenticate_quote(
    program: &Pubkey,
    account: &AccountInfo<'_>,
    fund: DealerFundV1,
) -> Result<(), ProgramError> {
    if account.owner != program {
        return Err(Error::CloseRecord.into());
    }
    let value =
        DealerQuoteV1::decode(&account.try_borrow_data()?).map_err(|_| Error::CloseRecord)?;
    let expected = Pubkey::find_program_address(
        &DealerQuoteV1::seeds(&fund.market, &fund.dealer_id),
        program,
    )
    .0;
    if *account.key != expected || value.market != fund.market || value.dealer_id != fund.dealer_id
    {
        return Err(Error::CloseRecord.into());
    }
    Ok(())
}

fn authenticate_child(
    child: ProtocolPositionRequestV2,
    fund: DealerFundV1,
    key: [u8; 32],
    facts: MarketFactsV1,
    credit: [u8; 32],
    rent_program: [u8; 32],
) -> Result<(), ProgramError> {
    if child.action != ProtocolPositionActionV2::Close
        || child.owner_kind != ProtocolPositionOwnerKindV2::TradingRecord
        || child.market != fund.market
        || child.position_owner != key
        || child.release_set != facts.release_set
        || child.generation != facts.generation
        || child.rent_credit != credit
        || child.rent_program != rent_program
    {
        return Err(Error::CloseChild.into());
    }
    Ok(())
}

fn claims_close_privileges(index: usize) -> Option<(bool, bool)> {
    let p = ClaimsFrameSpecV1::protocol_position(ProtocolPositionActionV2::Close)
        .account(u16::try_from(index).ok()?)
        .ok()?
        .privileges();
    Some((p.signer(), p.writable()))
}

#[inline(never)]
fn invoke_claims_close(
    program: &Pubkey,
    claims: &AccountInfo<'_>,
    window: &[AccountInfo<'_>],
    bytes: &[u8],
    request: ProtocolPositionRequestV2,
) -> Result<(), ProgramError> {
    let digest = hash(bytes).to_bytes();
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set,
        request.market,
        ExecutionRoleV1::Trading,
        request.position_owner,
        digest,
    )
    .map_err(|_| Error::Release)?;
    let (authority, bump) = Pubkey::find_program_address(&seeds.as_slices(), program);
    if get(window, 0)?.key != &authority {
        return Err(Error::Release.into());
    }
    let position = get(window, 2)?;
    let admission = get(window, 3)?;
    let credit = get(window, 13)?;
    let position_rent = position.lamports();
    let admission_rent = admission.lamports();
    let before = credit.lamports();
    let ix = Instruction {
        program_id: *claims.key,
        accounts: super::child_metas_v1(window, claims_close_privileges)?,
        data: bytes.to_vec(),
    };
    let bump_seed = [bump];
    let [d, r, m, role, c, h] = seeds.as_slices();
    invoke_signed(&ix, window, &[&[d, r, m, role, c, h, &bump_seed]])
        .map_err(crate::child_refused_v1)?;
    let (producer, raw) = get_return_data().ok_or(Error::CloseChild)?;
    let receipt = ProtocolPositionCloseReceiptV2::decode(&raw).map_err(|_| Error::CloseChild)?;
    receipt
        .validate_request(request, digest, claims.key.to_bytes())
        .map_err(|_| Error::CloseChild)?;
    if producer != *claims.key
        || receipt.position_lamports() != position_rent
        || receipt.admission_lamports() != admission_rent
        || receipt.rent_credit_before() != before
        || receipt.rent_credit_after() != credit.lamports()
    {
        return Err(Error::ClosePoststate.into());
    }
    require_closed(position)?;
    require_closed(admission)
}

/// One projection of existing Custody close semantics, shared with the operator.
/// Inputs are authenticated account observations; no economic formula is added.
#[allow(clippy::too_many_arguments)]
pub fn custody_close_request(
    program: [u8; 32],
    release: [u8; 32],
    market: [u8; 32],
    realm: [u8; 32],
    generation: u64,
    context: [u8; 32],
    parent: [u8; 32],
    credit: [u8; 32],
    revision: u64,
    rent: u64,
    vault: Option<([u8; 32], [u8; 32], [u8; 32])>,
) -> Result<CustodyRequestV1, ProgramError> {
    let (operation, source_compartment, source, source_vault_context, mint, token_program, index) =
        match vault {
            Some((v, m, t)) => (
                dclutch_custody::OperationV1::CloseVault,
                CompartmentV1::TradingPrincipal,
                v,
                context,
                m,
                t,
                0,
            ),
            None => (
                dclutch_custody::OperationV1::CloseReplay,
                CompartmentV1::None,
                [0; 32],
                [0; 32],
                [0; 32],
                [0; 32],
                1,
            ),
        };
    let request = CustodyRequestV1 {
        operation,
        caller_role: CallerRoleV1::Trading,
        source_compartment,
        destination_compartment: CompartmentV1::None,
        release_set: release,
        market,
        realm,
        context,
        caller_program: program,
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order: [0; 32],
            parent_request_digest: parent,
            order_nonce: 0,
            generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: index,
        },
        source,
        destination: [0; 32],
        source_vault_context,
        destination_vault_context: [0; 32],
        mint,
        token_program,
        payer: [0; 32],
        rent_refund: credit,
        expected_revision: revision,
        resulting_revision: revision.checked_add(1).ok_or(Error::Overflow)?,
        amount: 0,
        rent_lamports: rent,
    };
    request.validate().map_err(|_| Error::Custody)?;
    Ok(request)
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn close_custody<'info>(
    program: &Pubkey,
    custody: &AccountInfo<'info>,
    window: &[AccountInfo<'info>],
    prefix: &[AccountInfo<'info>],
    fund: &Pubkey,
    facts: MarketFactsV1,
    parent: [u8; 32],
    vault_leg: bool,
) -> Result<(), ProgramError> {
    let market = get(prefix, g::CLOSE_MARKET_ACCOUNT)?.key.to_bytes();
    let replay_account = get(window, 8)?;
    let replay =
        CustodyReplayV1::decode(&replay_account.try_borrow_data()?).map_err(|_| Error::Custody)?;
    let resource = if vault_leg {
        get(prefix, g::CLOSE_VAULT_ACCOUNT)?
    } else {
        replay_account
    };
    let credit = get(prefix, g::CLOSE_RENT_CREDIT_ACCOUNT)?;
    let before = credit.lamports();
    let rent = resource.lamports();
    let request = custody_close_request(
        program.to_bytes(),
        facts.release_set,
        market,
        facts.realm,
        facts.generation,
        fund.to_bytes(),
        parent,
        credit.key.to_bytes(),
        replay.next_revision,
        rent,
        if vault_leg {
            Some((
                resource.key.to_bytes(),
                get(prefix, g::CLOSE_MINT_ACCOUNT)?.key.to_bytes(),
                get(prefix, g::CLOSE_TOKEN_PROGRAM_ACCOUNT)?.key.to_bytes(),
            ))
        } else {
            None
        },
    )?;
    super::found::invoke_custody_v1(
        program,
        custody,
        window,
        request,
        fund.to_bytes(),
        facts,
        market,
    )?;
    let (producer, bytes) = get_return_data().ok_or(Error::CloseChild)?;
    let receipt = CustodyReceiptV1::decode(&bytes).map_err(|_| Error::CloseChild)?;
    let replay_digest = hash(&replay_account.try_borrow_data()?).to_bytes();
    receipt
        .verify_for(
            request,
            hash(&request.to_bytes().map_err(|_| Error::Custody)?).to_bytes(),
            replay_digest,
        )
        .map_err(|_| Error::CloseChild)?;
    if producer != *custody.key || before.checked_add(rent) != Some(credit.lamports()) {
        return Err(Error::ClosePoststate.into());
    }
    require_closed(resource)
}

fn require_closed(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.lamports() != 0 || !account.data_is_empty() || account.owner != &system_program::ID {
        return Err(Error::ClosePoststate.into());
    }
    Ok(())
}

fn close_record(
    account: &AccountInfo<'_>,
    recipient: &AccountInfo<'_>,
    owner: &Pubkey,
) -> Result<(), ProgramError> {
    if account.owner != owner || account.key == recipient.key {
        return Err(Error::CloseRecord.into());
    }
    let after = recipient
        .lamports()
        .checked_add(account.lamports())
        .ok_or(Error::Overflow)?;
    **recipient.try_borrow_mut_lamports()? = after;
    **account.try_borrow_mut_lamports()? = 0;
    account.resize(0)?;
    account.assign(&system_program::ID);
    require_closed(account)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custody_closure_preserves_namespace_refund_and_revision() {
        let request = custody_close_request(
            [1; 32],
            [2; 32],
            [3; 32],
            [4; 32],
            5,
            [6; 32],
            [7; 32],
            [8; 32],
            9,
            10,
            Some(([11; 32], [12; 32], [13; 32])),
        )
        .unwrap();
        assert_eq!(request.operation, dclutch_custody::OperationV1::CloseVault);
        assert_eq!(request.source_compartment, CompartmentV1::TradingPrincipal);
        assert_eq!(request.context, [6; 32]);
        assert_eq!(request.source_vault_context, request.context);
        assert_eq!(request.rent_refund, [8; 32]);
        assert_eq!(
            (request.expected_revision, request.resulting_revision),
            (9, 10)
        );
        assert_eq!(request.semantic.parent_request_digest, [7; 32]);
        let replay = custody_close_request(
            [1; 32], [2; 32], [3; 32], [4; 32], 5, [6; 32], [7; 32], [8; 32], 10, 11, None,
        )
        .unwrap();
        assert_eq!(replay.operation, dclutch_custody::OperationV1::CloseReplay);
        assert_eq!(replay.source, [0; 32]);
        assert_eq!(replay.rent_refund, request.rent_refund);
        assert_eq!(
            custody_close_request(
                [1; 32],
                [2; 32],
                [3; 32],
                [4; 32],
                5,
                [6; 32],
                [7; 32],
                [8; 32],
                u64::MAX,
                11,
                None
            ),
            Err(Error::Overflow.into())
        );
    }

    #[test]
    fn claims_close_binds_fund_and_its_recorded_rent_owner() {
        use dclutch_claims::protocol_position_v2::ProtocolPositionPresenceV2;
        use dclutch_trading::scoring_rule::records_v1::FundPhaseV1;
        let fund = DealerFundV1 {
            outcome_count: 2,
            phase: FundPhaseV1::Open,
            market: [1; 32],
            dealer_id: [2; 32],
            sponsor: [3; 32],
            rule_digest: [4; 32],
            vault: [5; 32],
            claim_unit_atoms: 1,
            cash: 0,
            inventory_minimum: 0,
            liquidity_cost: 1,
            revision: 2,
            bump: 255,
        };
        let facts = MarketFactsV1 {
            release_set: [6; 32],
            registry: [7; 32],
            realm: [8; 32],
            generation: 1,
            rent_beneficiary: [9; 32],
            phase: Phase::Terminal,
        };
        let child = ProtocolPositionRequestV2 {
            action: ProtocolPositionActionV2::Close,
            owner_kind: ProtocolPositionOwnerKindV2::TradingRecord,
            presence: ProtocolPositionPresenceV2::Existing,
            release_set: facts.release_set,
            market: fund.market,
            position_owner: [10; 32],
            parent_request_digest: [11; 32],
            rent_credit: facts.rent_beneficiary,
            rent_program: [12; 32],
            generation: 1,
            expected_market_revision: 2,
            expected_position_revision: 3,
            observed_position_lamports: 4,
            observed_admission_lamports: 5,
            position_rent_principal: 4,
            admission_rent_principal: 5,
            capability_descriptor: [0; 32],
            capability_outcome: 0,
        }
        .new()
        .unwrap();
        authenticate_child(child, fund, [10; 32], facts, [9; 32], [12; 32]).unwrap();
        for hostile in [
            ProtocolPositionRequestV2 {
                position_owner: [13; 32],
                ..child
            },
            ProtocolPositionRequestV2 {
                rent_credit: [13; 32],
                ..child
            },
            ProtocolPositionRequestV2 {
                market: [13; 32],
                ..child
            },
            ProtocolPositionRequestV2 {
                owner_kind: ProtocolPositionOwnerKindV2::User,
                ..child
            },
            ProtocolPositionRequestV2 {
                generation: 2,
                ..child
            },
        ] {
            assert_eq!(
                authenticate_child(hostile, fund, [10; 32], facts, [9; 32], [12; 32]),
                Err(Error::CloseChild.into())
            );
        }
    }

    #[test]
    fn closure_refuses_recorded_and_unrecorded_cash() {
        require_cash_closed(0, 0).unwrap();
        for (recorded, actual) in [(1, 0), (0, 1), (1, 1)] {
            assert_eq!(
                require_cash_closed(recorded, actual),
                Err(Error::CloseCash.into())
            );
        }
    }
}
