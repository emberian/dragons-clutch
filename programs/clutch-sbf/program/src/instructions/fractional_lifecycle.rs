//! Atomic Product-owned admission and retirement of the Fractional family.
//!
//! These handlers are compiled so the complete account/CPI boundary can be
//! reviewed, but the central capability table continues to refuse every action
//! from 1 through 10. Neither lifecycle function can be reached through checked
//! dispatch until all ten Fractional actions are enabled as one coherent
//! lifecycle.

use crate::accounts::{require, Outcome};
use crate::claim_release::authenticate_claim_issuance_release_with_programdata_v1;
use crate::error::{ClutchError, Refusal};
use crate::{claim_truth, seeds};
use clutch_collateral_adapter_v2::CLAIM_LEDGER_V3_BYTES;
use clutch_fractional_redemption_runtime::{
    bind_fractional_internal_context_v1, close_empty_ledger_v1,
    initialize_fractional_ledger_v1, Error as FractionalError, FractionalInitializeIntentV1,
    FractionalLedgerV1, FractionalPolicyV3, FractionalRedemptionActionV1,
    FractionalTerminalIntentV1, PayoutVectorV1, TerminalRemainderPolicyV1,
    FRACTIONAL_LEDGER_ACCOUNT_BYTES, FRACTIONAL_POLICY_ACCOUNT_BYTES,
};
use clutch_product_series::{
    ContentId, MarketFoundationAccountGraphV2, MarketFoundationScheduleV2,
    MarketFoundationSlotV2, SeriesFundingQuoteV4, MARKET_FOUNDATION_CORE_SLOT_COUNT_V2,
    MARKET_FOUNDATION_MAX_OUTCOMES_V2, MARKET_FOUNDATION_SLOT_COUNT_V2,
};
use clutch_retirement::{DeletableRentOwnerV1, Identity32V1};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::collateral_position_v3::{
    authenticate_general_market_value_authority_v2, authenticate_resolution_v5,
};
use super::fractional_redemption::{
    authenticate_fractional_family_admission_postwrite_v1,
    authenticate_fractional_family_terminal_postwrite_v1,
    authenticate_fractional_runtime_release_v1,
    consume_fractional_family_admission_postwrite_v1,
    consume_fractional_family_terminal_postwrite_v1,
};
use super::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    SYSTEM_PROGRAM_ID,
};
use super::product_artifact::{
    authenticate_product_artifact_v1, authenticate_registry_capability_v3,
    authenticate_series_registry_capability_refs_v2,
};
use super::product_market::{
    authenticate_market_foundation_preallocation_v2, authenticate_market_lifecycle_root_v1,
    authenticate_series_market_link_v1,
};

const ROOT: usize = 0;
const MARKET_BINDING: usize = 1;
const MARKET_RUNTIME: usize = 2;
const HOARD: usize = 3;
const CLAIM_LEDGER: usize = 4;
const RESOLUTION: usize = 10;
const POLICY: usize = 11;
const LEDGER: usize = 12;
const FIRST_OUTCOME_MINT: usize = MARKET_FOUNDATION_CORE_SLOT_COUNT_V2;

const INITIALIZE_AUX_ACCOUNTS: usize = 17;
const TERMINAL_AUX_ACCOUNTS: usize = 15;

mod init_aux {
    pub const REALM: usize = 0;
    pub const PROFILE: usize = 1;
    pub const COLLATERAL_POLICY: usize = 2;
    pub const COLLATERAL_TOKEN_PROGRAM: usize = 3;
    pub const COLLATERAL_TOKEN_PROGRAMDATA: usize = 4;
    pub const CLAIM_TOKEN_PROGRAM: usize = 5;
    pub const CLAIM_TOKEN_PROGRAMDATA: usize = 6;
    pub const MARKET_INSTANCE: usize = 7;
    pub const FOUNDER_LINK: usize = 8;
    pub const FUNDING_QUOTE: usize = 9;
    pub const SERIES_REGISTRY: usize = 10;
    pub const PROGRAM: usize = 11;
    pub const PROGRAMDATA: usize = 12;
    pub const RELEASE_ARTIFACT: usize = 13;
    pub const PROFILE_ARTIFACT: usize = 14;
    pub const SYSTEM_PROGRAM: usize = 15;
    pub const RENT: usize = 16;
}

mod terminal_aux {
    pub const REALM: usize = 0;
    pub const PROFILE: usize = 1;
    pub const COLLATERAL_POLICY: usize = 2;
    pub const COLLATERAL_TOKEN_PROGRAM: usize = 3;
    pub const COLLATERAL_TOKEN_PROGRAMDATA: usize = 4;
    pub const MARKET_INSTANCE: usize = 5;
    pub const FOUNDER_LINK: usize = 6;
    pub const FUNDING_QUOTE: usize = 7;
    pub const SERIES_REGISTRY: usize = 8;
    pub const PROGRAM: usize = 9;
    pub const PROGRAMDATA: usize = 10;
    pub const RELEASE_ARTIFACT: usize = 11;
    pub const PROFILE_ARTIFACT: usize = 12;
    pub const REFUND_OWNER: usize = 13;
    pub const NEUTRAL_SINK: usize = 14;
}

fn map_fractional(error: FractionalError) -> Refusal {
    match error {
        FractionalError::ReplayMismatch | FractionalError::ReplayRefused => {
            Refusal::Adapter(ClutchError::Replay)
        }
        FractionalError::Arithmetic => Refusal::Adapter(ClutchError::Arithmetic),
        FractionalError::Truncated
        | FractionalError::TrailingBytes
        | FractionalError::WrongTag
        | FractionalError::WrongVersion
        | FractionalError::NonCanonicalPadding => Refusal::Adapter(ClutchError::NonCanonical),
        _ => Refusal::Adapter(ClutchError::MismatchedState),
    }
}

fn identity(bytes: [u8; 32]) -> Outcome<Identity32V1> {
    Identity32V1::new(bytes).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn graph_account_count(outcome_count: u8) -> Outcome<usize> {
    let outcomes = usize::from(outcome_count);
    require(
        outcomes != 0 && outcomes <= MARKET_FOUNDATION_MAX_OUTCOMES_V2,
        ClutchError::NonCanonical,
    )?;
    MARKET_FOUNDATION_CORE_SLOT_COUNT_V2
        .checked_add(outcomes.checked_mul(2).ok_or(ClutchError::Arithmetic)?)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))
}

fn decode_root_probe(accounts: &[AccountInfo<'_>]) -> Outcome<Box<MarketLifecycleRootAccountV1>> {
    require(
        accounts.len() >= MARKET_FOUNDATION_CORE_SLOT_COUNT_V2,
        ClutchError::AccountCount,
    )?;
    let account = &accounts[ROOT];
    require(
        account.is_writable
            && !account.is_signer
            && !account.executable
            && account.data_len()
                == clutch_solana_layout::product_series::MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let mut output = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    MarketLifecycleRootAccountV1::decode_into(&data, &mut output)?;
    drop(data);
    Ok(output)
}

fn require_outer_contract(
    accounts: &[AccountInfo<'_>],
    graph_count: usize,
    aux_count: usize,
    initialize: bool,
) -> Outcome<()> {
    let expected = graph_count
        .checked_add(aux_count)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(accounts.len() == expected, ClutchError::AccountCount)?;
    let aux = graph_count;
    let mut index = 0usize;
    while index < accounts.len() {
        let expected_writable = matches!(index, ROOT | CLAIM_LEDGER | POLICY | LEDGER)
            || (!initialize
                && index >= graph_count
                && matches!(
                    index - graph_count,
                    terminal_aux::REFUND_OWNER | terminal_aux::NEUTRAL_SINK
                ));
        require(
            accounts[index].is_writable == expected_writable,
            if expected_writable {
                ClutchError::NotWritable
            } else {
                ClutchError::UnexpectedWritable
            },
        )?;
        require(!accounts[index].is_signer, ClutchError::MismatchedState)?;
        if index < graph_count {
            require(!accounts[index].executable, ClutchError::ExecutableAccount)?;
        }
        let mut other = index + 1;
        while other < accounts.len() {
            let claim_program_alias = initialize
                && ((index == aux + init_aux::COLLATERAL_TOKEN_PROGRAM
                    && other == aux + init_aux::CLAIM_TOKEN_PROGRAM)
                    || (index == aux + init_aux::COLLATERAL_TOKEN_PROGRAMDATA
                        && other == aux + init_aux::CLAIM_TOKEN_PROGRAMDATA));
            if !claim_program_alias {
                require(accounts[index].key != accounts[other].key, ClutchError::AccountAlias)?;
            }
            other += 1;
        }
        index += 1;
    }
    Ok(())
}

fn build_graph(
    accounts: &[AccountInfo<'_>],
    schedule: &MarketFoundationScheduleV2,
    market: clutch_product_series::MarketInstanceV2Id,
    generation: u64,
    outcome_count: u8,
) -> Outcome<MarketFoundationAccountGraphV2> {
    let outcomes = usize::from(outcome_count);
    let mut account_ids = [ContentId::ZERO; MARKET_FOUNDATION_SLOT_COUNT_V2];
    let mut core = 0usize;
    while core < MARKET_FOUNDATION_CORE_SLOT_COUNT_V2 {
        account_ids[core] = ContentId::from_bytes(accounts[core].key.to_bytes());
        core += 1;
    }
    let mut outcome = 0usize;
    while outcome < outcomes {
        account_ids[MARKET_FOUNDATION_CORE_SLOT_COUNT_V2 + outcome] =
            ContentId::from_bytes(accounts[FIRST_OUTCOME_MINT + outcome].key.to_bytes());
        account_ids[MARKET_FOUNDATION_CORE_SLOT_COUNT_V2
            + MARKET_FOUNDATION_MAX_OUTCOMES_V2
            + outcome] = ContentId::from_bytes(
            accounts[FIRST_OUTCOME_MINT + outcomes + outcome]
                .key
                .to_bytes(),
        );
        outcome += 1;
    }
    let graph = MarketFoundationAccountGraphV2 {
        market_instance_id: market,
        generation,
        foundation_schedule_id: schedule
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        account_ids,
    };
    graph
        .validate(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(graph)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_schedule_and_series(
    program_id: &Pubkey,
    root: super::product_market::AuthenticatedMarketLifecycleRootV1<'_>,
    founder_link_account: &AccountInfo<'_>,
    funding_quote_account: &AccountInfo<'_>,
) -> Outcome<(MarketFoundationScheduleV2, clutch_product_series::SeriesPlanV5Id)> {
    let mut link_body = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    let data = founder_link_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV1::decode_into(&data, &mut link_body)?;
    drop(data);
    let link_binding = link_body.state.binding();
    let authenticated = authenticate_series_market_link_v1(
        program_id,
        founder_link_account,
        link_binding.series_plan_id,
        link_binding.ordinal,
        root.state().binding().market_instance_id,
        root.state().binding().generation,
        root.account(),
        false,
        &mut link_body,
    )?;
    let semantic_id = authenticated
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        semantic_id == root.state().capital().founder_link_id.content_id()
            && link_binding.market_instance_id == root.state().binding().market_instance_id
            && link_binding.generation == root.state().binding().generation
            && link_binding.rent_refund_owner == root.state().capital().rent_refund_owner
            && link_binding.neutral_lamport_sink == root.state().capital().neutral_lamport_sink,
        ClutchError::MismatchedState,
    )?;
    let quote = authenticate_product_artifact_v1::<SeriesFundingQuoteV4>(
        program_id,
        funding_quote_account,
        link_binding.funding_quote_id.content_id(),
    )?;
    Ok((quote.value().foundation, link_binding.series_plan_id))
}

fn allocate_prefunded_pda<'a>(
    program_id: &Pubkey,
    account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    space: usize,
    signer: &[&[u8]],
) -> Outcome<()> {
    require_creatable(account)?;
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(space),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &allocate,
        &[account.clone(), system_program.clone()],
        &[signer],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(&assign, &[account.clone(), system_program.clone()], &[signer])
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        account.owner == program_id && account.data_len() == space,
        ClutchError::AccountCreationFailed,
    )
}

/// Execute action 1 over Product-prefunded a4/a5 prestates.
#[inline(never)]
pub(super) fn process_initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope_sequence: u64,
    intent: FractionalInitializeIntentV1,
) -> Outcome<()> {
    require(envelope_sequence == 0, ClutchError::Replay)?;
    let mut root_before = decode_root_probe(accounts)?;
    let binding = root_before.state.binding();
    let outcome_count = binding.outcome_count;
    let graph_count = graph_account_count(outcome_count)?;
    require_outer_contract(accounts, graph_count, INITIALIZE_AUX_ACCOUNTS, true)?;
    let aux = graph_count;
    require_system_program(&accounts[aux + init_aux::SYSTEM_PROGRAM])?;
    let rent = read_rent(&accounts[aux + init_aux::RENT])?;

    let value = authenticate_general_market_value_authority_v2(
        program_id,
        &accounts[aux + init_aux::REALM],
        &accounts[aux + init_aux::PROFILE],
        &accounts[aux + init_aux::COLLATERAL_POLICY],
        &accounts[aux + init_aux::COLLATERAL_TOKEN_PROGRAM],
        &accounts[aux + init_aux::COLLATERAL_TOKEN_PROGRAMDATA],
        &accounts[MARKET_BINDING],
        &accounts[MARKET_RUNTIME],
        &accounts[aux + init_aux::MARKET_INSTANCE],
        &accounts[HOARD],
        &accounts[CLAIM_LEDGER],
        false,
        true,
    )?;
    let liabilities = value.liabilities;
    let resolution = authenticate_resolution_v5(program_id, &accounts[RESOLUTION], liabilities)?;
    require(
        binding.market_instance_id == liabilities.market_binding.base().market_instance_v2_id
            && binding.generation == intent.domain_generation
            && binding.generation == resolution.resolution.facts.generation
            && binding.resolution_account_id.bytes() == accounts[RESOLUTION].key.to_bytes()
            && root_before.state.resolution_data_id().bytes() == resolution.data_id.bytes()
            && binding.native_claim_basis_id.bytes()
                == liabilities.claim_ledger.native_claim_basis_id.bytes()
            && binding.outcome_count == liabilities.claim_ledger.outcome_count,
        ClutchError::MismatchedState,
    )?;
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        &accounts[ROOT],
        binding.market_instance_id,
        binding.generation,
        true,
        &mut root_before,
    )?;
    let (schedule, series_plan_id) = authenticate_schedule_and_series(
        program_id,
        root,
        &accounts[aux + init_aux::FOUNDER_LINK],
        &accounts[aux + init_aux::FUNDING_QUOTE],
    )?;
    let graph = build_graph(
        accounts,
        &schedule,
        binding.market_instance_id,
        binding.generation,
        outcome_count,
    )?;
    require(
        graph.id(&schedule)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == binding.foundation_account_graph_id,
        ClutchError::MismatchedState,
    )?;
    let policy_funding = authenticate_market_foundation_preallocation_v2(
        root,
        &accounts[POLICY],
        &schedule,
        &graph,
        MarketFoundationSlotV2::FractionalPolicy,
    )?;
    let ledger_funding = authenticate_market_foundation_preallocation_v2(
        root,
        &accounts[LEDGER],
        &schedule,
        &graph,
        MarketFoundationSlotV2::FractionalLedger,
    )?;
    require(
        policy_funding.rent_refund_owner() == ledger_funding.rent_refund_owner()
            && policy_funding.neutral_lamport_sink() == ledger_funding.neutral_lamport_sink()
            && policy_funding.principal_lamports()
                == rent.minimum_balance(FRACTIONAL_POLICY_ACCOUNT_BYTES)?
            && ledger_funding.principal_lamports()
                == rent.minimum_balance(FRACTIONAL_LEDGER_ACCOUNT_BYTES)?,
        ClutchError::MismatchedState,
    )?;

    let registry_refs = authenticate_series_registry_capability_refs_v2(
        program_id,
        &accounts[aux + init_aux::SERIES_REGISTRY],
        series_plan_id,
    )?;
    let registry = authenticate_registry_capability_v3(
        program_id,
        registry_refs,
        &accounts[aux + init_aux::PROGRAM],
        &accounts[aux + init_aux::PROGRAMDATA],
        &accounts[aux + init_aux::RELEASE_ARTIFACT],
        &accounts[aux + init_aux::PROFILE_ARTIFACT],
    )?;
    let runtime_release = authenticate_fractional_runtime_release_v1(
        program_id,
        registry,
        FractionalRedemptionActionV1::Initialize,
    )?;
    let claim_release = authenticate_claim_issuance_release_with_programdata_v1(
        liabilities.bound,
        &accounts[aux + init_aux::CLAIM_TOKEN_PROGRAM],
        &accounts[aux + init_aux::CLAIM_TOKEN_PROGRAMDATA],
    )?;
    require(
        claim_release.bound().binding_id().bytes() == binding.claim_issuance_binding_id.bytes(),
        ClutchError::AuthorizationUnavailable,
    )?;
    let observed = claim_truth::observe_outcome_mints_v2(
        program_id,
        accounts,
        FIRST_OUTCOME_MINT,
        *accounts[MARKET_RUNTIME].key,
        binding.market_instance_id.bytes(),
        outcome_count,
        None,
    )?;
    require(
        observed.values == liabilities.claim_ledger.aggregate_materialized_supply,
        ClutchError::MismatchedState,
    )?;

    let payout = PayoutVectorV1::from_resolution_v5(resolution.resolution).map_err(map_fractional)?;
    require(payout.common_lot().map_err(map_fractional)? == intent.common_lot, ClutchError::MismatchedState)?;
    let (expected_policy, policy_bump) = seeds::fractional_policy_v3_pda(
        program_id,
        &binding.market_instance_id.bytes(),
        &accounts[RESOLUTION].key.to_bytes(),
    );
    let (expected_ledger, ledger_bump) =
        seeds::fractional_ledger_v1_pda(program_id, &accounts[POLICY].key.to_bytes());
    require(
        expected_policy == *accounts[POLICY].key
            && expected_ledger == *accounts[LEDGER].key
            && policy_bump == intent.policy_bump
            && ledger_bump == intent.ledger_bump,
        ClutchError::WrongBump,
    )?;
    let policy_rent = DeletableRentOwnerV1::from_persisted(
        identity(policy_funding.rent_refund_owner().to_bytes())?,
        policy_funding.principal_lamports(),
        policy_funding.donation_lamports(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let ledger_rent = DeletableRentOwnerV1::from_persisted(
        identity(ledger_funding.rent_refund_owner().to_bytes())?,
        ledger_funding.principal_lamports(),
        ledger_funding.donation_lamports(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let policy = FractionalPolicyV3 {
        market_instance: identity(binding.market_instance_id.bytes())?,
        resolution_account: identity(accounts[RESOLUTION].key.to_bytes())?,
        resolution_data_id: identity(resolution.data_id.bytes())?,
        realm: identity(liabilities.hoard.realm_id.bytes())?,
        collateral_policy: identity(liabilities.bound.policy_id().bytes())?,
        collateral_release: identity(
            liabilities
                .bound
                .release()
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?
                .bytes(),
        )?,
        claim_issuance_binding: identity(claim_release.bound().binding_id().bytes())?,
        domain_generation: binding.generation,
        common_lot: intent.common_lot,
        outcome_count,
        terminal_policy: TerminalRemainderPolicyV1::RetainUntilExactAggregation,
        stored_bump: policy_bump,
        rent: policy_rent,
    };
    let plan = initialize_fractional_ledger_v1(
        identity(accounts[POLICY].key.to_bytes())?,
        policy,
        identity(accounts[LEDGER].key.to_bytes())?,
        identity(accounts[CLAIM_LEDGER].key.to_bytes())?,
        liabilities.claim_ledger,
        ledger_bump,
        ledger_rent,
    )
    .map_err(map_fractional)?;

    let policy_bump_seed = [policy_bump];
    let market_seed = binding.market_instance_id.bytes();
    let resolution_seed = accounts[RESOLUTION].key.to_bytes();
    let policy_signer: [&[u8]; 4] = [
        seeds::SEED_FRACTIONAL_POLICY_V3,
        &market_seed,
        &resolution_seed,
        &policy_bump_seed,
    ];
    allocate_prefunded_pda(
        program_id,
        &accounts[POLICY],
        &accounts[aux + init_aux::SYSTEM_PROGRAM],
        FRACTIONAL_POLICY_ACCOUNT_BYTES,
        &policy_signer,
    )?;
    let ledger_bump_seed = [ledger_bump];
    let policy_account_seed = accounts[POLICY].key.to_bytes();
    let ledger_signer: [&[u8]; 3] = [
        seeds::SEED_FRACTIONAL_LEDGER_V1,
        &policy_account_seed,
        &ledger_bump_seed,
    ];
    allocate_prefunded_pda(
        program_id,
        &accounts[LEDGER],
        &accounts[aux + init_aux::SYSTEM_PROGRAM],
        FRACTIONAL_LEDGER_ACCOUNT_BYTES,
        &ledger_signer,
    )?;
    accounts[POLICY]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&policy.encode().map_err(map_fractional)?);
    accounts[LEDGER]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&plan.ledger_after.encode().map_err(map_fractional)?);
    plan.claim_ledger
        .claim_ledger_after()
        .encode(
            &mut accounts[CLAIM_LEDGER]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let postwrite = authenticate_fractional_family_admission_postwrite_v1(
        program_id,
        runtime_release,
        Identity32V1::new(resolution.semantic_id.bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        plan,
        &accounts[POLICY],
        &accounts[LEDGER],
        &accounts[CLAIM_LEDGER],
    )?;
    let mut product_before = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let mut product_after = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let accepted = consume_fractional_family_admission_postwrite_v1(
        program_id,
        &accounts[ROOT],
        postwrite,
        &schedule,
        &graph,
        &mut product_before,
        &mut product_after,
    )?;
    require(accepted.id() != ContentId::ZERO, ClutchError::MismatchedState)
}

fn set_lamports(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    let mut value = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **value = amount;
    Ok(())
}

/// Execute action 10, consuming Product terminality before either deletion.
#[inline(never)]
pub(super) fn process_close_empty_ledger(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope_sequence: u64,
    intent: FractionalTerminalIntentV1,
) -> Outcome<()> {
    require(envelope_sequence == intent.expected_ledger_sequence, ClutchError::Replay)?;
    let mut root_before = decode_root_probe(accounts)?;
    let binding = root_before.state.binding();
    let outcome_count = binding.outcome_count;
    let graph_count = graph_account_count(outcome_count)?;
    require_outer_contract(accounts, graph_count, TERMINAL_AUX_ACCOUNTS, false)?;
    let aux = graph_count;
    let value = authenticate_general_market_value_authority_v2(
        program_id,
        &accounts[aux + terminal_aux::REALM],
        &accounts[aux + terminal_aux::PROFILE],
        &accounts[aux + terminal_aux::COLLATERAL_POLICY],
        &accounts[aux + terminal_aux::COLLATERAL_TOKEN_PROGRAM],
        &accounts[aux + terminal_aux::COLLATERAL_TOKEN_PROGRAMDATA],
        &accounts[MARKET_BINDING],
        &accounts[MARKET_RUNTIME],
        &accounts[aux + terminal_aux::MARKET_INSTANCE],
        &accounts[HOARD],
        &accounts[CLAIM_LEDGER],
        false,
        true,
    )?;
    let liabilities = value.liabilities;
    let resolution = authenticate_resolution_v5(program_id, &accounts[RESOLUTION], liabilities)?;
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        &accounts[ROOT],
        binding.market_instance_id,
        binding.generation,
        true,
        &mut root_before,
    )?;
    let (schedule, series_plan_id) = authenticate_schedule_and_series(
        program_id,
        root,
        &accounts[aux + terminal_aux::FOUNDER_LINK],
        &accounts[aux + terminal_aux::FUNDING_QUOTE],
    )?;
    let graph = build_graph(
        accounts,
        &schedule,
        binding.market_instance_id,
        binding.generation,
        outcome_count,
    )?;
    require(
        graph.id(&schedule)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == binding.foundation_account_graph_id,
        ClutchError::MismatchedState,
    )?;
    let registry_refs = authenticate_series_registry_capability_refs_v2(
        program_id,
        &accounts[aux + terminal_aux::SERIES_REGISTRY],
        series_plan_id,
    )?;
    let registry = authenticate_registry_capability_v3(
        program_id,
        registry_refs,
        &accounts[aux + terminal_aux::PROGRAM],
        &accounts[aux + terminal_aux::PROGRAMDATA],
        &accounts[aux + terminal_aux::RELEASE_ARTIFACT],
        &accounts[aux + terminal_aux::PROFILE_ARTIFACT],
    )?;
    let runtime_release = authenticate_fractional_runtime_release_v1(
        program_id,
        registry,
        FractionalRedemptionActionV1::CloseEmptyLedger,
    )?;
    require(
        accounts[POLICY].owner == program_id
            && accounts[LEDGER].owner == program_id
            && accounts[POLICY].data_len() == FRACTIONAL_POLICY_ACCOUNT_BYTES
            && accounts[LEDGER].data_len() == FRACTIONAL_LEDGER_ACCOUNT_BYTES
            && accounts[CLAIM_LEDGER].data_len() == CLAIM_LEDGER_V3_BYTES,
        ClutchError::MismatchedState,
    )?;
    let policy = FractionalPolicyV3::decode(
        &accounts[POLICY]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )
    .map_err(map_fractional)?;
    let ledger = FractionalLedgerV1::decode(
        &accounts[LEDGER]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )
    .map_err(map_fractional)?;
    let (expected_policy, policy_bump) = seeds::fractional_policy_v3_pda(
        program_id,
        &policy.market_instance.bytes(),
        &policy.resolution_account.bytes(),
    );
    let (expected_ledger, ledger_bump) =
        seeds::fractional_ledger_v1_pda(program_id, &accounts[POLICY].key.to_bytes());
    require(
        expected_policy == *accounts[POLICY].key
            && expected_ledger == *accounts[LEDGER].key
            && policy.stored_bump == policy_bump
            && ledger.stored_bump == ledger_bump
            && policy.market_instance.bytes() == binding.market_instance_id.bytes()
            && policy.resolution_account.bytes() == accounts[RESOLUTION].key.to_bytes()
            && policy.resolution_data_id.bytes() == resolution.data_id.bytes()
            && ledger.claim_ledger_account.bytes() == accounts[CLAIM_LEDGER].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed = claim_truth::observe_outcome_mints_v2(
        program_id,
        accounts,
        FIRST_OUTCOME_MINT,
        *accounts[MARKET_RUNTIME].key,
        binding.market_instance_id.bytes(),
        outcome_count,
        None,
    )?;
    require(
        observed.values == liabilities.claim_ledger.aggregate_materialized_supply,
        ClutchError::MismatchedState,
    )?;
    let context = bind_fractional_internal_context_v1(
        identity(accounts[POLICY].key.to_bytes())?,
        policy,
        identity(accounts[LEDGER].key.to_bytes())?,
        ledger,
        identity(accounts[CLAIM_LEDGER].key.to_bytes())?,
        liabilities.claim_ledger,
        liabilities.hoard,
        resolution.resolution,
        liabilities.bound,
    )
    .map_err(map_fractional)?;
    let neutral = identity(accounts[aux + terminal_aux::NEUTRAL_SINK].key.to_bytes())?;
    let close = close_empty_ledger_v1(
        context,
        intent.expected_ledger_sequence,
        accounts[POLICY].lamports(),
        accounts[LEDGER].lamports(),
        neutral,
    )
    .map_err(map_fractional)?;
    let policy_funding = close.policy_funding();
    let ledger_funding = close.ledger_funding();
    let refund = &accounts[aux + terminal_aux::REFUND_OWNER];
    let sink = &accounts[aux + terminal_aux::NEUTRAL_SINK];
    require(
        refund.key.to_bytes() == policy_funding.payer().bytes()
            && refund.key.to_bytes() == ledger_funding.payer().bytes()
            && sink.key.to_bytes() == policy_funding.neutral_sink().bytes()
            && sink.key.to_bytes() == ledger_funding.neutral_sink().bytes()
            && refund.owner == &SYSTEM_PROGRAM_ID
            && sink.owner == &SYSTEM_PROGRAM_ID
            && refund.data_is_empty()
            && sink.data_is_empty()
            && !refund.executable
            && !sink.executable,
        ClutchError::MismatchedState,
    )?;
    close
        .claim_ledger_after()
        .claim_ledger_after()
        .encode(
            &mut accounts[CLAIM_LEDGER]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let postwrite = authenticate_fractional_family_terminal_postwrite_v1(
        program_id,
        runtime_release,
        close,
        &accounts[POLICY],
        &accounts[LEDGER],
        &accounts[CLAIM_LEDGER],
    )?;
    let mut product_before = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let mut product_after = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let accepted = consume_fractional_family_terminal_postwrite_v1(
        program_id,
        &accounts[ROOT],
        postwrite,
        &schedule,
        &graph,
        &mut product_before,
        &mut product_after,
    )?;
    require(accepted.id() != ContentId::ZERO, ClutchError::MismatchedState)?;

    let refund_amount = policy_funding
        .payer_refund_lamports()
        .checked_add(ledger_funding.payer_refund_lamports())
        .ok_or(ClutchError::Arithmetic)?;
    let neutral_amount = policy_funding
        .neutral_lamports()
        .checked_add(ledger_funding.neutral_lamports())
        .ok_or(ClutchError::Arithmetic)?;
    let refund_after = refund
        .lamports()
        .checked_add(refund_amount)
        .ok_or(ClutchError::Arithmetic)?;
    let sink_after = sink
        .lamports()
        .checked_add(neutral_amount)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        refund_amount
            .checked_add(neutral_amount)
            .ok_or(ClutchError::Arithmetic)?
            == accounts[POLICY]
                .lamports()
                .checked_add(accounts[LEDGER].lamports())
                .ok_or(ClutchError::Arithmetic)?,
        ClutchError::MismatchedState,
    )?;
    set_lamports(&accounts[POLICY], 0)?;
    set_lamports(&accounts[LEDGER], 0)?;
    set_lamports(refund, refund_after)?;
    set_lamports(sink, sink_after)?;
    accounts[POLICY]
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    accounts[POLICY].assign(&SYSTEM_PROGRAM_ID);
    accounts[LEDGER]
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    accounts[LEDGER].assign(&SYSTEM_PROGRAM_ID);
    require(
        accounts[POLICY].lamports() == 0
            && accounts[LEDGER].lamports() == 0
            && accounts[POLICY].data_is_empty()
            && accounts[LEDGER].data_is_empty()
            && accounts[POLICY].owner == &SYSTEM_PROGRAM_ID
            && accounts[LEDGER].owner == &SYSTEM_PROGRAM_ID
            && refund.lamports() == refund_after
            && sink.lamports() == sink_after,
        ClutchError::MismatchedState,
    )
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    #[test]
    fn outer_geometry_is_exact_and_bounded() {
        assert_eq!(graph_account_count(1), Ok(16));
        assert_eq!(graph_account_count(16), Ok(46));
        assert!(graph_account_count(0).is_err());
        assert!(graph_account_count(17).is_err());
    }

    #[test]
    fn action_one_and_ten_have_distinct_exact_auxiliary_surfaces() {
        assert_eq!(INITIALIZE_AUX_ACCOUNTS, 17);
        assert_eq!(TERMINAL_AUX_ACCOUNTS, 15);
        assert_ne!(init_aux::CLAIM_TOKEN_PROGRAM, terminal_aux::REFUND_OWNER);
    }
}
