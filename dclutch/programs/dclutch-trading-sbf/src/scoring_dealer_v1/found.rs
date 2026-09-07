//! `DealerFound` (`DCLSFDR1`): seal a rule, open a fund, admit the Dealer's
//! Position, write the first quote. Anyone may found; the founder is the
//! sponsor of record (BUILD_DEALER.md §4.1–2).
//!
//! Frame: the Lean's `foundFrame` (21), then four windows in this order:
//!
//! | window | width | what |
//! |---|---|---|
//! | Custody `InitializeReplay` | 13 | the replay cursor for the fund's context (the fund PDA) |
//! | Custody `OpenVault` | 16 | the fund's `TradingPrincipal` vault |
//! | Custody `Transfer` | 14 | sponsor → vault, the deposit |
//! | Claims `Admit` | 26 | the Dealer's Position, owned by the fund PDA |
//!
//! The order is the only one Custody admits (`InitializeReplay` at revision
//! 0 → 1, `OpenVault` 1 → 2, `Transfer` 2 → 3), and the Position must exist
//! before the first quote reads it.

extern crate alloc;

use dclutch_claims::liability_basis_state_v2::LIABILITY_BASIS_POSITION_HEADER_BYTES_V2;
use dclutch_claims::position_admission::{
    USER_POSITION_ADMISSION_CHILD_ACCOUNT_COUNT_V1, UserPositionAdmissionRequestV1,
};
use dclutch_claims::protocol_position_v2::{
    PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionActionV2, ProtocolPositionOwnerKindV2,
    ProtocolPositionPresenceV2, ProtocolPositionRequestV2,
};
use dclutch_custody::{
    CUSTODY_BUMP_RELAY_BYTES_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1,
    CustodyRequestV1, INITIALIZE_REPLAY_ACCOUNT_COUNT_V1, OPEN_VAULT_ACCOUNT_COUNT_V1, OperationV1,
    TRANSFER_ACCOUNT_COUNT_V1,
};
use dclutch_market::Phase;
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading::scoring_rule::records_v1::{
    DealerFundV1, DealerQuoteV1, FundPhaseV1, ScoringRuleRecordV1,
};
use dclutch_trading::scoring_rule::requests_v1::{
    DealerFoundRequestV1, DealerRouteV1, FOUND_FRAME_ACCOUNTS, found_privileges_v1,
};
use dclutch_trading::scoring_rule::{ZERO_VECTOR, admit_founding, generated, prices_of};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::hash,
    instruction::Instruction,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::create_account;

use super::{
    CustodyLegV1, MarketFactsV1, ScoringDealerErrorV1, authenticate_claim_unit_v1,
    authenticate_market_v1, authenticate_release_v1, authenticate_vault_v1, child_metas_v1,
    claims_admit_privileges_v1, custody_privileges_v1, emit_receipt_v1, get,
    invoke_custody_transfer_v1, parse_prefix, read_replay_v1, write_quote_v1,
};
use crate::hot_v3::hot_cu_checkpoint_macro as hot_cu_checkpoint;

/// The windows after the prefix, in order.
pub const FOUND_REPLAY_WINDOW_ACCOUNTS: usize = INITIALIZE_REPLAY_ACCOUNT_COUNT_V1 as usize;
/// See [`FOUND_REPLAY_WINDOW_ACCOUNTS`].
pub const FOUND_VAULT_WINDOW_ACCOUNTS: usize = OPEN_VAULT_ACCOUNT_COUNT_V1 as usize;
/// See [`FOUND_REPLAY_WINDOW_ACCOUNTS`].
pub const FOUND_DEPOSIT_WINDOW_ACCOUNTS: usize = TRANSFER_ACCOUNT_COUNT_V1 as usize;
/// See [`FOUND_REPLAY_WINDOW_ACCOUNTS`].
pub const FOUND_ADMIT_WINDOW_ACCOUNTS: usize = USER_POSITION_ADMISSION_CHILD_ACCOUNT_COUNT_V1;
/// Claims' own `Admit` frame coordinates, as `ClaimsFrameSpecV1` states them
/// (`dclutch-claims/src/frame_spec_v1.rs:490-517`). Named here because this
/// route both forwards the window and reads two of its accounts itself.
pub const ADMIT_AUTHORITY_ACCOUNT: usize = 0;
/// See [`ADMIT_AUTHORITY_ACCOUNT`].
pub const ADMIT_AGGREGATE_ACCOUNT: usize = 1;
/// See [`ADMIT_AUTHORITY_ACCOUNT`].
pub const ADMIT_POSITION_ACCOUNT: usize = 2;
/// See [`ADMIT_AUTHORITY_ACCOUNT`].
pub const ADMIT_ADMISSION_ACCOUNT: usize = 3;
/// See [`ADMIT_AUTHORITY_ACCOUNT`].
pub const ADMIT_BASIS_RECORD_ACCOUNT: usize = 4;
/// See [`ADMIT_AUTHORITY_ACCOUNT`].
pub const ADMIT_RENT_CREDIT_ACCOUNT: usize = 24;
/// See [`ADMIT_AUTHORITY_ACCOUNT`].
pub const ADMIT_RENT_PROGRAM_ACCOUNT: usize = 25;

/// Every window after the prefix.
pub const FOUND_WINDOW_ACCOUNTS: usize = FOUND_REPLAY_WINDOW_ACCOUNTS
    + FOUND_VAULT_WINDOW_ACCOUNTS
    + FOUND_DEPOSIT_WINDOW_ACCOUNTS
    + FOUND_ADMIT_WINDOW_ACCOUNTS;

/// Execute one founding.
#[inline(never)]
pub fn process_dealer_found_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let request = DealerFoundRequestV1::decode(instruction_data)
        .map_err(|_| ScoringDealerErrorV1::Request)?;
    let (prefix, windows) = parse_prefix(
        accounts,
        FOUND_FRAME_ACCOUNTS,
        FOUND_WINDOW_ACCOUNTS,
        found_privileges_v1,
    )?;
    hot_cu_checkpoint!("scoring-dealer:found:frame");
    let sponsor = get(prefix, generated::FOUND_SPONSOR_ACCOUNT)?;
    let facts = authenticate_market_v1(
        get(prefix, generated::FOUND_MARKET_ACCOUNT)?,
        request.market,
        &[Phase::Open],
    )?;
    if facts.release_set != request.release_set || facts.generation != request.generation {
        return Err(ScoringDealerErrorV1::Market.into());
    }
    let released_claims = authenticate_release_v1(
        program_id,
        get(prefix, generated::FOUND_REGISTRY_PROGRAM_ACCOUNT)?,
        get(prefix, generated::FOUND_ACTIVATION_CACHE_ACCOUNT)?,
        facts,
    )?;
    hot_cu_checkpoint!("scoring-dealer:found:authenticated");

    let (replay_window, rest) = windows.split_at(FOUND_REPLAY_WINDOW_ACCOUNTS);
    let (vault_window, rest) = rest.split_at(FOUND_VAULT_WINDOW_ACCOUNTS);
    let (deposit_window, admit_window) = rest.split_at(FOUND_DEPOSIT_WINDOW_ACCOUNTS);

    // The four Claims coordinates the prefix declares and the Admit window
    // carries are ONE account each. Nothing bound them, so this route could
    // authenticate one aggregate and hand Claims another.
    let aggregate = get(prefix, generated::FOUND_AGGREGATE_ACCOUNT)?;
    for (prefix_account, window_account) in [
        (
            generated::FOUND_CLAIMS_AUTHORITY_ACCOUNT,
            ADMIT_AUTHORITY_ACCOUNT,
        ),
        (generated::FOUND_AGGREGATE_ACCOUNT, ADMIT_AGGREGATE_ACCOUNT),
        (
            generated::FOUND_DEALER_POSITION_ACCOUNT,
            ADMIT_POSITION_ACCOUNT,
        ),
        (
            generated::FOUND_DEALER_ADMISSION_ACCOUNT,
            ADMIT_ADMISSION_ACCOUNT,
        ),
    ] {
        if get(prefix, prefix_account)?.key != get(admit_window, window_account)?.key {
            return Err(ScoringDealerErrorV1::Frame.into());
        }
    }

    // The two numbers no later route can question: the claim-unit conversion
    // and the width a fill mints across. Both are the Market's, not the
    // sponsor's, and the record that says so is already in the Admit window.
    authenticate_claim_unit_v1(
        aggregate,
        get(admit_window, ADMIT_BASIS_RECORD_ACCOUNT)?,
        request.claim_unit_atoms,
        request.parameters.outcome_count,
    )?;
    hot_cu_checkpoint!("scoring-dealer:found:claim-unit");

    // The rule's arithmetic: parameters admitted, the subsidy computed once,
    // the deposit at least the subsidy.
    let deposit_claims = request.deposit / request.claim_unit_atoms;
    let subsidy = admit_founding(
        request.parameters,
        dclutch_trading::scoring_rule::subsidy_of(request.parameters)
            .map_err(ScoringDealerErrorV1::from)?,
        deposit_claims,
    )
    .map_err(ScoringDealerErrorV1::from)?;
    let rule = request.rule(subsidy);
    let rule_bytes = rule
        .to_bytes()
        .map_err(|_| ScoringDealerErrorV1::Parameters)?;
    hot_cu_checkpoint!("scoring-dealer:found:subsidy");

    // The three PDAs, created under the sponsor's lamports.
    let fund_account = get(prefix, generated::FOUND_FUND_ACCOUNT)?;
    let rule_account = get(prefix, generated::FOUND_RULE_ACCOUNT)?;
    let quote_account = get(prefix, generated::FOUND_QUOTE_ACCOUNT)?;
    let system = get(prefix, generated::FOUND_SYSTEM_PROGRAM_ACCOUNT)?;
    if system.key != &system_program::ID {
        return Err(ScoringDealerErrorV1::Frame.into());
    }
    let rent = Rent::get().map_err(|_| ScoringDealerErrorV1::Frame)?;
    let fund_bump = create_pda_v1(
        program_id,
        sponsor,
        fund_account,
        system,
        &rent,
        &DealerFundV1::seeds(&request.market, &request.dealer_id),
        generated::FUND_BYTES,
        ScoringDealerErrorV1::Fund,
    )?;
    create_pda_v1(
        program_id,
        sponsor,
        rule_account,
        system,
        &rent,
        &ScoringRuleRecordV1::seeds(&request.market, &request.dealer_id),
        generated::RULE_BYTES,
        ScoringDealerErrorV1::RuleSeal,
    )?;
    let quote_bump = create_pda_v1(
        program_id,
        sponsor,
        quote_account,
        system,
        &rent,
        &DealerQuoteV1::seeds(&request.market, &request.dealer_id),
        generated::QUOTE_BYTES,
        ScoringDealerErrorV1::Quote,
    )?;
    rule_account
        .try_borrow_mut_data()
        .map_err(|_| ScoringDealerErrorV1::RuleSeal)?
        .copy_from_slice(&rule_bytes);
    hot_cu_checkpoint!("scoring-dealer:found:records");

    // Custody: the replay for the fund context, the vault, the deposit.
    let custody_program = get(prefix, generated::FOUND_CUSTODY_PROGRAM_ACCOUNT)?;
    let vault = get(prefix, generated::FOUND_VAULT_ACCOUNT)?;
    let context = fund_account.key.to_bytes();
    let parent = hash(instruction_data).to_bytes();
    let replay_rent = rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1);
    invoke_custody_v1(
        program_id,
        custody_program,
        replay_window,
        CustodyRequestV1 {
            operation: OperationV1::InitializeReplay,
            caller_role: CallerRoleV1::Trading,
            source_compartment: CompartmentV1::None,
            destination_compartment: CompartmentV1::None,
            release_set: facts.release_set,
            market: request.market,
            realm: facts.realm,
            context,
            caller_program: program_id.to_bytes(),
            semantic: semantic_v1(parent, facts, 0),
            source: [0; 32],
            destination: [0; 32],
            source_vault_context: [0; 32],
            destination_vault_context: [0; 32],
            mint: [0; 32],
            token_program: [0; 32],
            payer: sponsor.key.to_bytes(),
            rent_refund: sponsor.key.to_bytes(),
            expected_revision: 0,
            resulting_revision: 1,
            amount: 0,
            rent_lamports: replay_rent,
        },
        context,
        facts,
        request.market,
    )?;
    let mint = get(prefix, generated::FOUND_MINT_ACCOUNT)?;
    let token_program = get(prefix, generated::FOUND_TOKEN_PROGRAM_ACCOUNT)?;
    invoke_custody_v1(
        program_id,
        custody_program,
        vault_window,
        CustodyRequestV1 {
            operation: OperationV1::OpenVault,
            caller_role: CallerRoleV1::Trading,
            source_compartment: CompartmentV1::None,
            destination_compartment: CompartmentV1::TradingPrincipal,
            release_set: facts.release_set,
            market: request.market,
            realm: facts.realm,
            context,
            caller_program: program_id.to_bytes(),
            semantic: semantic_v1(parent, facts, 1),
            source: [0; 32],
            destination: vault.key.to_bytes(),
            source_vault_context: [0; 32],
            destination_vault_context: context,
            mint: mint.key.to_bytes(),
            token_program: token_program.key.to_bytes(),
            payer: sponsor.key.to_bytes(),
            rent_refund: sponsor.key.to_bytes(),
            expected_revision: 1,
            resulting_revision: 2,
            amount: 0,
            rent_lamports: rent.minimum_balance(dclutch_custody::token_svm::ACCOUNT_BYTES),
        },
        context,
        facts,
        request.market,
    )?;
    hot_cu_checkpoint!("scoring-dealer:found:vault");
    let fund_seed = DealerFundV1 {
        outcome_count: request.parameters.outcome_count,
        phase: FundPhaseV1::Open,
        market: request.market,
        dealer_id: request.dealer_id,
        sponsor: sponsor.key.to_bytes(),
        rule_digest: hash(&rule_bytes).to_bytes(),
        vault: vault.key.to_bytes(),
        claim_unit_atoms: request.claim_unit_atoms,
        cash: 0,
        inventory_minimum: 0,
        liquidity_cost: subsidy,
        revision: 0,
        bump: fund_bump,
    };
    authenticate_vault_v1(
        custody_program.key,
        vault,
        fund_seed,
        fund_account.key,
        facts.release_set,
    )?;
    let replay = read_replay_v1(deposit_window)?;
    invoke_custody_transfer_v1(
        program_id,
        CustodyLegV1 {
            window: deposit_window,
            custody_program,
            replay,
            facts,
            market: request.market,
            context,
            source_compartment: CompartmentV1::External,
            destination_compartment: CompartmentV1::TradingPrincipal,
            source_owner: sponsor.key.to_bytes(),
            destination_owner: [0; 32],
            source_vault_context: [0; 32],
            destination_vault_context: context,
            parent_request_digest: parent,
            transfer_index: 2,
            amount: request.deposit,
        },
    )?;
    hot_cu_checkpoint!("scoring-dealer:found:deposit");

    // Claims: the Dealer's Position, owned by the fund PDA. The frame's Claims
    // program is the release's, not the caller's word for it.
    let claims_program = get(prefix, generated::FOUND_CLAIMS_PROGRAM_ACCOUNT)?;
    if claims_program.key != &released_claims {
        return Err(ScoringDealerErrorV1::Release.into());
    }
    invoke_claims_admit_v1(
        program_id,
        claims_program,
        admit_window,
        sponsor,
        system,
        facts,
        request,
        fund_account.key,
        parent,
    )?;
    hot_cu_checkpoint!("scoring-dealer:found:position");

    // The fund and the first quote.
    let fund = DealerFundV1 {
        cash: request.deposit,
        ..fund_seed
    };
    let fund_bytes = fund.to_bytes().map_err(|_| ScoringDealerErrorV1::Commit)?;
    fund_account
        .try_borrow_mut_data()
        .map_err(|_| ScoringDealerErrorV1::Commit)?
        .copy_from_slice(&fund_bytes);
    let prices = prices_of(request.parameters, &ZERO_VECTOR).map_err(ScoringDealerErrorV1::from)?;
    write_quote_v1(
        quote_account,
        DealerQuoteV1 {
            outcome_count: request.parameters.outcome_count,
            market: request.market,
            dealer_id: request.dealer_id,
            fund_revision: 0,
            slot: Clock::get().map_err(|_| ScoringDealerErrorV1::Quote)?.slot,
            scale: request.parameters.scale,
            prices,
            bump: quote_bump,
        },
    )?;
    emit_receipt_v1(
        DealerRouteV1::Found,
        instruction_data,
        fund,
        &fund_bytes,
        0,
        0,
    )
}

const fn semantic_v1(parent: [u8; 32], facts: MarketFactsV1, transfer_index: u16) -> ContextV1 {
    ContextV1 {
        candidate: [0; 32],
        source_owner: [0; 32],
        destination_owner: [0; 32],
        order: [0; 32],
        parent_request_digest: parent,
        order_nonce: 0,
        generation: facts.generation,
        page_index: 0,
        execution_index: 0,
        transfer_index,
    }
}

/// Create one Trading-owned PDA under the sponsor's lamports, returning its
/// bump. Refuses an account that already exists: a founding is once.
#[allow(clippy::too_many_arguments)]
fn create_pda_v1<'info>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    account: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    rent: &Rent,
    seeds: &[&[u8]; 3],
    space: usize,
    refusal: ScoringDealerErrorV1,
) -> Result<u8, ProgramError> {
    let (expected, bump) = Pubkey::find_program_address(seeds, program_id);
    if expected != *account.key || account.owner != &system_program::ID || account.data_len() != 0 {
        return Err(refusal.into());
    }
    let lamports = rent.minimum_balance(space);
    let instruction = create_account(
        payer.key,
        account.key,
        lamports,
        u64::try_from(space).map_err(|_| refusal)?,
        program_id,
    );
    let bump_seed = [bump];
    invoke_signed(
        &instruction,
        &[payer.clone(), account.clone(), system.clone()],
        &[&[seeds[0], seeds[1], seeds[2], &bump_seed]],
    )
    .map_err(crate::child_refused_v1)?;
    Ok(bump)
}

/// Bring one vacant, system-owned PDA up to the rent its allocation will need,
/// from the sponsor, and only by the shortfall.
///
/// A plain `system_instruction::transfer` and not a `create_account`: the
/// account is Claims' to allocate and assign, and this route must not take
/// that from it. Nothing happens when the balance already covers the
/// principal, which is what makes a stranger's donation to a keyless address
/// harmless rather than a second thing to reconcile.
fn prefund_v1<'info>(
    sponsor: &AccountInfo<'info>,
    account: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    principal: u64,
) -> Result<(), ProgramError> {
    let Some(shortfall) = principal.checked_sub(account.lamports()) else {
        return Ok(());
    };
    if shortfall == 0 {
        return Ok(());
    }
    invoke(
        &solana_system_interface::instruction::transfer(sponsor.key, account.key, shortfall),
        &[sponsor.clone(), account.clone(), system.clone()],
    )
    .map_err(crate::child_refused_v1)
}

/// Compose and invoke one non-transfer Custody request (InitializeReplay,
/// OpenVault) under this program's caller authority for `context`.
#[inline(never)]
fn invoke_custody_v1<'info>(
    program_id: &Pubkey,
    custody_program: &AccountInfo<'info>,
    window: &[AccountInfo<'info>],
    request: CustodyRequestV1,
    context: [u8; 32],
    facts: MarketFactsV1,
    market: [u8; 32],
) -> Result<(), ProgramError> {
    let request_bytes = request
        .to_bytes()
        .map_err(|_| ScoringDealerErrorV1::Custody)?;
    let request_digest = hash(&request_bytes).to_bytes();
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        facts.release_set,
        market,
        ExecutionRoleV1::Trading,
        context,
        request_digest,
    )
    .map_err(|_| ScoringDealerErrorV1::Release)?;
    let (authority, bump) = Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    if get(window, 0)?.key != &authority {
        return Err(ScoringDealerErrorV1::Release.into());
    }
    let mut data =
        alloc::vec::Vec::with_capacity(request_bytes.len() + CUSTODY_BUMP_RELAY_BYTES_V1);
    data.extend_from_slice(&request_bytes);
    data.extend_from_slice(&[0_u8; CUSTODY_BUMP_RELAY_BYTES_V1]);
    let metas = child_metas_v1(window, custody_privileges_v1(request.operation))?;
    let instruction = Instruction {
        program_id: *custody_program.key,
        accounts: metas,
        data,
    };
    let mut infos = alloc::vec::Vec::with_capacity(window.len() + 1);
    infos.extend_from_slice(window);
    infos.push(custody_program.clone());
    let bump_seed = [bump];
    let [domain, release, market_seed, role, context_seed, digest] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[
            domain,
            release,
            market_seed,
            role,
            context_seed,
            digest,
            &bump_seed,
        ]],
    )
    .map_err(crate::child_refused_v1)
}

/// Admit the Dealer's Position through Claims' protocol-Position lifecycle,
/// the fund PDA as owner (`TradingRecord`), this program signing the
/// request-bound caller authority exactly as `user_position_admission_v1`
/// does for a wallet.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn invoke_claims_admit_v1<'info>(
    program_id: &Pubkey,
    claims_program: &AccountInfo<'info>,
    window: &[AccountInfo<'info>],
    sponsor: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    facts: MarketFactsV1,
    request: DealerFoundRequestV1,
    fund_key: &Pubkey,
    parent: [u8; 32],
) -> Result<(), ProgramError> {
    if window.len() != FOUND_ADMIT_WINDOW_ACCOUNTS {
        return Err(ScoringDealerErrorV1::Frame.into());
    }
    let aggregate = get(window, ADMIT_AGGREGATE_ACCOUNT)?;
    let position = get(window, ADMIT_POSITION_ACCOUNT)?;
    let admission = get(window, ADMIT_ADMISSION_ACCOUNT)?;
    let rent_credit = get(window, ADMIT_RENT_CREDIT_ACCOUNT)?;
    let rent_program = get(window, ADMIT_RENT_PROGRAM_ACCOUNT)?;
    let (aggregate_revision, claim_count) = {
        let data = aggregate
            .try_borrow_data()
            .map_err(|_| ScoringDealerErrorV1::Claims)?;
        let view =
            dclutch_claims::liability_basis_state_v2::LiabilityBasisMarketViewV2::decode(&data)
                .map_err(|_| ScoringDealerErrorV1::Claims)?;
        (view.revision, view.claim_count)
    };
    let rent = Rent::get().map_err(|_| ScoringDealerErrorV1::Claims)?;
    // The rent principals are the widths the accounts will HAVE, not the ones
    // they have now.
    //
    // Both are vacant, system-owned, zero-length PDAs at this point, so
    // `minimum_balance(data_len())` is `minimum_balance(0)` -- and Claims
    // computes the same two numbers from `vector_width(header, claim_count)`
    // and `PROTOCOL_POSITION_ADMISSION_BYTES_V2` and refuses `Rent` on any
    // disagreement (`protocol_position_v2.rs:420`). Declaring the current
    // width would have refused every founding, and the request's own
    // `validate()` would have refused it one step earlier for a zero
    // principal.
    let position_bytes = dclutch_claims::liability_basis_state_v2::liability_basis_vector_width_v2(
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        claim_count,
    )
    .map_err(|_| ScoringDealerErrorV1::Claims)?;
    let position_rent = rent.minimum_balance(position_bytes);
    let admission_rent = rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2);
    // And the accounts have to HOLD it before Claims allocates them: Claims
    // allocates and assigns and performs no lamport arithmetic, so nothing
    // downstream pays this rent if the founding does not. The sponsor does,
    // and only the shortfall -- a stranger's donation to a keyless PDA is
    // already counted, which is why Claims takes a floor here and not an
    // equality.
    prefund_v1(sponsor, position, system, position_rent)?;
    prefund_v1(sponsor, admission, system, admission_rent)?;
    let claims_request = ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Admit,
        owner_kind: ProtocolPositionOwnerKindV2::TradingRecord,
        presence: ProtocolPositionPresenceV2::Vacant,
        release_set: facts.release_set,
        market: request.market,
        position_owner: fund_key.to_bytes(),
        parent_request_digest: parent,
        rent_credit: rent_credit.key.to_bytes(),
        rent_program: rent_program.key.to_bytes(),
        generation: facts.generation,
        expected_market_revision: aggregate_revision,
        expected_position_revision: 0,
        observed_position_lamports: position.lamports(),
        observed_admission_lamports: admission.lamports(),
        position_rent_principal: position_rent,
        admission_rent_principal: admission_rent,
        capability_descriptor: [0; 32],
        capability_outcome: 0,
    }
    .new()
    .map_err(|_| ScoringDealerErrorV1::Claims)?;
    let outer = UserPositionAdmissionRequestV1::new(claims_request)
        .map_err(|_| ScoringDealerErrorV1::Claims)?;
    let child_data = outer
        .claims_request_bytes()
        .map_err(|_| ScoringDealerErrorV1::Claims)?;
    let request_digest = hash(&child_data).to_bytes();
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        facts.release_set,
        request.market,
        ExecutionRoleV1::Trading,
        fund_key.to_bytes(),
        request_digest,
    )
    .map_err(|_| ScoringDealerErrorV1::Release)?;
    let (authority, bump) = Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    if get(window, 0)?.key != &authority {
        return Err(ScoringDealerErrorV1::Release.into());
    }
    let metas = child_metas_v1(window, claims_admit_privileges_v1)?;
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: child_data.to_vec(),
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
    let (producer, receipt) =
        solana_program::program::get_return_data().ok_or(ScoringDealerErrorV1::Claims)?;
    if producer != *claims_program.key {
        return Err(ScoringDealerErrorV1::Claims.into());
    }
    outer
        .validate_claims_receipt(
            &receipt,
            request_digest,
            claims_program.key.to_bytes(),
            program_id.to_bytes(),
        )
        .map_err(|_| ScoringDealerErrorV1::Claims)?;
    let _ = invoke;
    Ok(())
}
