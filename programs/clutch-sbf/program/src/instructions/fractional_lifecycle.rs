//! Atomic Product-owned admission and retirement of the Fractional family.
//!
//! These handlers are always compiled so the central successor profile can
//! admit all ten actions only as one coherent lifecycle with its complete
//! Product, collateral, claim-release, and registry dependency closure.

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
use clutch_general_v2_contract::MarketBindingV4;
use clutch_product_series::{
    ContentId, MarketFoundationAccountGraphV4, MarketFoundationScheduleV4,
    MarketFoundationSlotV4, MarketLifecycleRootV3, SeriesFundingQuoteV6,
    MARKET_FOUNDATION_CORE_SLOT_COUNT_V4, MARKET_FOUNDATION_MAX_OUTCOMES_V4,
    MARKET_FOUNDATION_SLOT_COUNT_V4,
};
use clutch_retirement::{DeletableRentOwnerV1, Identity32V1, PositionPurposeV3};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, SeriesMarketLinkAccountV3,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::collateral_position_v3::{
    authenticate_general_market_value_authority_v4, authenticate_resolution_v5,
};
use super::fractional_redemption::{
    authenticate_fractional_family_admission_postwrite_v1,
    authenticate_fractional_family_terminal_postwrite_v1,
    authenticate_fractional_runtime_release_v1,
    consume_fractional_family_terminal_postwrite_v2,
    execute_fractional_family_physical_terminal_v2,
    prepare_fractional_family_physical_terminal_v2,
};
use super::fractional_product_consumer::{
    commit_fractional_admission_v3, prepare_fractional_admission_v3,
};
use super::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    SYSTEM_PROGRAM_ID,
};
use super::product_artifact::authenticate_product_artifact_v1;
use super::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, authenticate_series_market_link_v3,
    AuthenticatedMarketLifecycleRootV3, AuthenticatedSeriesMarketLinkV3,
};
use super::product_market_family_capability_current::
    authenticate_current_market_family_capability_policy_v1;
use super::product_market_replay_current::authenticate_market_lifecycle_replay_v2;
use super::product_series_current::{
    authenticate_registry_capability_v5, authenticate_series_registry_account_v4,
};

const ROOT: usize = 0;
const MARKET_BINDING: usize = 1;
const MARKET_RUNTIME: usize = 2;
const HOARD: usize = 3;
const CLAIM_LEDGER: usize = 4;
const RESOLUTION: usize = 10;
const POLICY: usize = 11;
const LEDGER: usize = 12;
const PRODUCT_REPLAY: usize = 13;
/// Role 14 carries the hostile current family-policy artifact. The graph's
/// slot-14 Hoard token account is a canonical PDA derived below.
const FAMILY_POLICY_ARTIFACT: usize = 14;
const FIRST_OUTCOME_MINT: usize = MARKET_FOUNDATION_CORE_SLOT_COUNT_V4;
const MAX_FRACTIONAL_LIFECYCLE_ACCOUNTS: usize =
    MARKET_FOUNDATION_CORE_SLOT_COUNT_V4 + MARKET_FOUNDATION_MAX_OUTCOMES_V4 + 17;

const INITIALIZE_AUX_ACCOUNTS: usize = 17;
const TERMINAL_AUX_ACCOUNTS: usize = 17;

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
    pub const REFUND_OWNER: usize = 15;
    pub const NEUTRAL_SINK: usize = 16;
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
        outcomes != 0 && outcomes <= MARKET_FOUNDATION_MAX_OUTCOMES_V4,
        ClutchError::NonCanonical,
    )?;
    MARKET_FOUNDATION_CORE_SLOT_COUNT_V4
        .checked_add(outcomes)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))
}

fn decode_root_probe(accounts: &[AccountInfo<'_>]) -> Outcome<Box<MarketLifecycleRootAccountV3>> {
    require(
        accounts.len() >= MARKET_FOUNDATION_CORE_SLOT_COUNT_V4,
        ClutchError::AccountCount,
    )?;
    let account = &accounts[ROOT];
    require(
        account.is_writable
            && !account.is_signer
            && !account.executable
            && account.data_len()
                == clutch_solana_layout::product_series::MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V3,
        ClutchError::MismatchedState,
    )?;
    let mut output = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    MarketLifecycleRootAccountV3::decode_into(&data, &mut output)?;
    drop(data);
    Ok(output)
}

fn permitted_claim_loader_alias(
    initialize: bool,
    graph_count: usize,
    first: usize,
    second: usize,
) -> bool {
    if initialize {
        (first == graph_count + init_aux::COLLATERAL_TOKEN_PROGRAM
            && second == graph_count + init_aux::CLAIM_TOKEN_PROGRAM)
            || (first == graph_count + init_aux::COLLATERAL_TOKEN_PROGRAMDATA
                && second == graph_count + init_aux::CLAIM_TOKEN_PROGRAMDATA)
    } else {
        (first == graph_count + terminal_aux::COLLATERAL_TOKEN_PROGRAM
            && second == graph_count + terminal_aux::CLAIM_TOKEN_PROGRAM)
            || (first == graph_count + terminal_aux::COLLATERAL_TOKEN_PROGRAMDATA
                && second == graph_count + terminal_aux::CLAIM_TOKEN_PROGRAMDATA)
    }
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
    require(
        accounts.len() == expected && expected <= MAX_FRACTIONAL_LIFECYCLE_ACCOUNTS,
        ClutchError::AccountCount,
    )?;
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
            let claim_program_alias =
                permitted_claim_loader_alias(initialize, aux, index, other);
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
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    schedule: &MarketFoundationScheduleV4,
    market: clutch_product_series::MarketInstanceV2Id,
    generation: u64,
    outcome_count: u8,
    revenue_binding: &MarketBindingV4,
    family_policy_in_role_14: bool,
) -> Outcome<MarketFoundationAccountGraphV4> {
    let outcomes = usize::from(outcome_count);
    let mut account_ids = [ContentId::ZERO; MARKET_FOUNDATION_SLOT_COUNT_V4];
    let mut core = 0usize;
    while core < FAMILY_POLICY_ARTIFACT {
        account_ids[core] = ContentId::from_bytes(accounts[core].key.to_bytes());
        core += 1;
    }
    let hoard_slot = MarketFoundationSlotV4::HoardCollateralVault
        .index()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    account_ids[hoard_slot] = if family_policy_in_role_14 {
        ContentId::from_bytes(
            seeds::hoard_token_v2_pda(program_id, &market.bytes()).0.to_bytes(),
        )
    } else {
        ContentId::from_bytes(accounts[hoard_slot].key.to_bytes())
    };
    let mut outcome = 0usize;
    while outcome < outcomes {
        account_ids[MARKET_FOUNDATION_CORE_SLOT_COUNT_V4 + outcome] =
            ContentId::from_bytes(accounts[FIRST_OUTCOME_MINT + outcome].key.to_bytes());
        let outcome_index = u8::try_from(outcome).map_err(|_| ClutchError::Arithmetic)?;
        let custody =
            seeds::outcome_custody_v1_pda(program_id, &market.bytes(), generation, outcome_index).0;
        account_ids[MARKET_FOUNDATION_CORE_SLOT_COUNT_V4
            + MARKET_FOUNDATION_MAX_OUTCOMES_V4
            + outcome] = ContentId::from_bytes(custody.to_bytes());
        outcome += 1;
    }
    let revenue = revenue_binding.authority();
    let runtime = accounts[MARKET_RUNTIME].key.to_bytes();
    let treasury_owner = revenue.treasury_owner().bytes();
    let treasury_position = seeds::position_v3_pda(
        program_id,
        &market.bytes(),
        &treasury_owner,
        PositionPurposeV3::General,
        &runtime,
    )
    .0;
    let treasury_replay = seeds::purpose_replay_v3_pda(
        program_id,
        &treasury_position.to_bytes(),
        PositionPurposeV3::General,
        &runtime,
    )
    .0;
    let treasury_service =
        seeds::treasury_service_ledger_v1_pda(program_id, &market.bytes(), &treasury_position).0;
    require(
        revenue.treasury_position_account().bytes() == treasury_position.to_bytes()
            && revenue.treasury_service_ledger_account().bytes() == treasury_service.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    account_ids[MarketFoundationSlotV4::GeneralTreasuryPosition.index()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?] =
        ContentId::from_bytes(treasury_position.to_bytes());
    account_ids[MarketFoundationSlotV4::GeneralTreasuryReplay.index()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?] =
        ContentId::from_bytes(treasury_replay.to_bytes());
    account_ids[MarketFoundationSlotV4::TreasuryServiceLedger.index()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?] =
        ContentId::from_bytes(treasury_service.to_bytes());
    let graph = MarketFoundationAccountGraphV4 {
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
fn authenticate_schedule_and_series<'link>(
    program_id: &Pubkey,
    root: &AuthenticatedMarketLifecycleRootV3<'_>,
    founder_link_account: &AccountInfo<'_>,
    funding_quote_account: &AccountInfo<'_>,
    link_output: &'link mut SeriesMarketLinkAccountV3,
) -> Outcome<(
    MarketFoundationScheduleV4,
    clutch_product_series::SeriesPlanV5Id,
    clutch_product_series::SeriesFundingTermsV2Id,
    clutch_product_series::CompiledProductSeriesBundleV7Id,
    AuthenticatedSeriesMarketLinkV3<'link>,
)> {
    let data = founder_link_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV3::decode_into(&data, link_output)?;
    drop(data);
    let link_binding = link_output.state.binding();
    let authenticated = authenticate_series_market_link_v3(
        program_id,
        founder_link_account,
        link_binding.series_plan_id,
        link_binding.ordinal,
        root.state().binding().market_instance_id,
        root.state().binding().generation,
        root.account(),
        false,
        link_output,
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
    let quote = authenticate_product_artifact_v1::<SeriesFundingQuoteV6>(
        program_id,
        funding_quote_account,
        link_binding.funding_quote_id.content_id(),
    )?;
    Ok((
        quote.value().foundation,
        link_binding.series_plan_id,
        link_binding.funding_terms_id,
        link_binding.compiler_bundle_id,
        authenticated,
    ))
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

#[derive(Clone, Copy)]
struct FractionalFoundationPrefundingV4 {
    payer: Pubkey,
    neutral: Pubkey,
    principal: u64,
    donation: u64,
}

impl FractionalFoundationPrefundingV4 {
    const fn rent_refund_owner(self) -> Pubkey { self.payer }
    const fn neutral_lamport_sink(self) -> Pubkey { self.neutral }
    const fn principal_lamports(self) -> u64 { self.principal }
    const fn donation_lamports(self) -> u64 { self.donation }
}

fn authenticate_fractional_foundation_preallocation_v4(
    root: &AuthenticatedMarketLifecycleRootV3<'_>,
    account: &AccountInfo<'_>,
    schedule: &MarketFoundationScheduleV4,
    graph: &MarketFoundationAccountGraphV4,
    slot: MarketFoundationSlotV4,
) -> Outcome<FractionalFoundationPrefundingV4> {
    schedule.validate().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    graph.validate(schedule).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let binding = root.state().binding();
    let index = slot.index().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let bit = 1_u64.checked_shl(u32::try_from(index).map_err(|_| ClutchError::Arithmetic)?)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        matches!(slot, MarketFoundationSlotV4::FractionalPolicy | MarketFoundationSlotV4::FractionalLedger)
            && root.is_writable()
            && root.state().phase() == clutch_product_series::MarketLifecyclePhaseV3::Active
            && binding.foundation_schedule_id == schedule.id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && binding.foundation_account_graph_id == graph.id(schedule)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && graph.market_instance_id == binding.market_instance_id
            && graph.generation == binding.generation
            && graph.account(slot).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?.bytes()
                == account.key.to_bytes()
            && root.state().foundation().initialized_bitmap & bit != 0
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && account.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && account.data_len() == 0,
        ClutchError::MismatchedState,
    )?;
    let principal = schedule.slot_principal_lamports[index];
    let donation = account.lamports().checked_sub(principal)
        .ok_or(ClutchError::MismatchedState)?;
    let capital = root.state().capital();
    Ok(FractionalFoundationPrefundingV4 {
        payer: Pubkey::new_from_array(capital.rent_refund_owner.bytes()),
        neutral: Pubkey::new_from_array(capital.neutral_lamport_sink.bytes()),
        principal,
        donation,
    })
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

    let value = authenticate_general_market_value_authority_v4(
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
        binding.market_instance_id == liabilities.market_binding.base().base().market_instance_v2_id
            && binding.generation == intent.domain_generation
            && binding.generation == resolution.resolution.facts.generation
            && binding.resolution_account_id.bytes() == accounts[RESOLUTION].key.to_bytes()
            && root_before.state.resolution_data_id().bytes() == resolution.data_id.bytes()
            && binding.native_claim_basis_id.bytes()
                == liabilities.claim_ledger.native_claim_basis_id.bytes()
            && binding.outcome_count == liabilities.claim_ledger.outcome_count,
        ClutchError::MismatchedState,
    )?;
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        &accounts[ROOT],
        binding.market_instance_id,
        binding.generation,
        true,
        &mut root_before,
    )?;
    let mut link_body = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let (schedule, series_plan_id, funding_terms_id, compiler_bundle_id, link) =
        authenticate_schedule_and_series(
            program_id,
            &root,
            &accounts[aux + init_aux::FOUNDER_LINK],
            &accounts[aux + init_aux::FUNDING_QUOTE],
            &mut link_body,
        )?;
    let graph = build_graph(
        program_id,
        accounts,
        &schedule,
        binding.market_instance_id,
        binding.generation,
        outcome_count,
        &liabilities.market_binding,
        true,
    )?;
    require(
        graph.id(&schedule)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == binding.foundation_account_graph_id,
        ClutchError::MismatchedState,
    )?;
    let replay = authenticate_market_lifecycle_replay_v2(
        program_id,
        &accounts[PRODUCT_REPLAY],
        binding.market_instance_id,
        false,
    )?;
    let family_policy = authenticate_current_market_family_capability_policy_v1(
        program_id,
        &root,
        &replay,
        &accounts[FAMILY_POLICY_ARTIFACT],
    )?;
    let policy_funding = authenticate_fractional_foundation_preallocation_v4(
        &root,
        &accounts[POLICY],
        &schedule,
        &graph,
        MarketFoundationSlotV4::FractionalPolicy,
    )?;
    let ledger_funding = authenticate_fractional_foundation_preallocation_v4(
        &root,
        &accounts[LEDGER],
        &schedule,
        &graph,
        MarketFoundationSlotV4::FractionalLedger,
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

    let registry_account = authenticate_series_registry_account_v4(
        program_id,
        &accounts[aux + init_aux::SERIES_REGISTRY],
        series_plan_id,
        false,
    )?;
    let registry = authenticate_registry_capability_v5(
        program_id,
        registry_account,
        &accounts[aux + init_aux::PROGRAM],
        &accounts[aux + init_aux::PROGRAMDATA],
        &accounts[aux + init_aux::RELEASE_ARTIFACT],
        &accounts[aux + init_aux::PROFILE_ARTIFACT],
    )?;
    require(
        registry.funding_terms_id() == funding_terms_id
            && registry.compiler_bundle_id() == compiler_bundle_id,
        ClutchError::MismatchedState,
    )?;
    let runtime_release = authenticate_fractional_runtime_release_v1(
        program_id,
        &registry,
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
    let mut product_successor = Box::new(MarketLifecycleRootV3::decode_buffer());
    let (product_admission_plan, fractional_admission_prewrite) =
        prepare_fractional_admission_v3(
            program_id,
            &root,
            &replay,
            &family_policy,
            &link,
            &schedule,
            &graph,
            &runtime_release,
            &resolution,
            ContentId::from_bytes(liabilities.claim_ledger.native_claim_basis_id.bytes()),
            &plan,
            &mut product_successor,
        )?;

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
        resolution,
        plan,
        &accounts[POLICY],
        &accounts[LEDGER],
        &accounts[CLAIM_LEDGER],
    )?;
    let mut product_before = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let mut product_after = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let (_, accepted) = commit_fractional_admission_v3(
        program_id,
        &accounts[ROOT],
        product_admission_plan,
        fractional_admission_prewrite,
        postwrite,
        &mut product_before,
        &mut product_successor,
        &mut product_after,
    )?;
    require(accepted.id() != ContentId::ZERO, ClutchError::MismatchedState)
}

/// Execute action 10, physically closing Fractional before Product consumes
/// the sole move-only terminal receipt.
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
    let value = authenticate_general_market_value_authority_v4(
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
    let claim_release = authenticate_claim_issuance_release_with_programdata_v1(
        liabilities.bound,
        &accounts[aux + terminal_aux::CLAIM_TOKEN_PROGRAM],
        &accounts[aux + terminal_aux::CLAIM_TOKEN_PROGRAMDATA],
    )?;
    require(
        claim_release.bound().binding_id().bytes() == binding.claim_issuance_binding_id.bytes(),
        ClutchError::AuthorizationUnavailable,
    )?;
    let resolution = authenticate_resolution_v5(program_id, &accounts[RESOLUTION], liabilities)?;
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        &accounts[ROOT],
        binding.market_instance_id,
        binding.generation,
        true,
        &mut root_before,
    )?;
    let mut link_body = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let (schedule, series_plan_id, funding_terms_id, compiler_bundle_id, link) =
        authenticate_schedule_and_series(
            program_id,
            &root,
            &accounts[aux + terminal_aux::FOUNDER_LINK],
            &accounts[aux + terminal_aux::FUNDING_QUOTE],
            &mut link_body,
        )?;
    let graph = build_graph(
        program_id,
        accounts,
        &schedule,
        binding.market_instance_id,
        binding.generation,
        outcome_count,
        &liabilities.market_binding,
        false,
    )?;
    require(
        graph.id(&schedule)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == binding.foundation_account_graph_id,
        ClutchError::MismatchedState,
    )?;
    let registry_account = authenticate_series_registry_account_v4(
        program_id,
        &accounts[aux + terminal_aux::SERIES_REGISTRY],
        series_plan_id,
        false,
    )?;
    let registry = authenticate_registry_capability_v5(
        program_id,
        registry_account,
        &accounts[aux + terminal_aux::PROGRAM],
        &accounts[aux + terminal_aux::PROGRAMDATA],
        &accounts[aux + terminal_aux::RELEASE_ARTIFACT],
        &accounts[aux + terminal_aux::PROFILE_ARTIFACT],
    )?;
    require(
        registry.funding_terms_id() == funding_terms_id
            && registry.compiler_bundle_id() == compiler_bundle_id,
        ClutchError::MismatchedState,
    )?;
    let runtime_release = authenticate_fractional_runtime_release_v1(
        program_id,
        &registry,
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
            && policy.claim_issuance_binding.bytes()
                == claim_release.bound().binding_id().bytes()
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
    let refund = &accounts[aux + terminal_aux::REFUND_OWNER];
    let sink = &accounts[aux + terminal_aux::NEUTRAL_SINK];
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
        identity(claim_release.receipt_id().bytes())?,
        close,
        &accounts[POLICY],
        &accounts[LEDGER],
        &accounts[CLAIM_LEDGER],
    )?;
    let prepared = prepare_fractional_family_physical_terminal_v2(
        postwrite,
        &accounts[POLICY],
        &accounts[LEDGER],
        refund,
        sink,
    )?;
    let physical = execute_fractional_family_physical_terminal_v2(
        prepared,
        &accounts[POLICY],
        &accounts[LEDGER],
        refund,
        sink,
    )?;
    let mut product_before = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let mut product_successor = Box::new(MarketLifecycleRootV3::decode_buffer());
    let mut product_after = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let accepted = consume_fractional_family_terminal_postwrite_v2(
        program_id,
        &accounts[ROOT],
        physical,
        &link,
        &schedule,
        &graph,
        &mut product_before,
        &mut product_successor,
        &mut product_after,
    )?;
    require(accepted.id() != ContentId::ZERO, ClutchError::MismatchedState)
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    #[test]
    fn outer_geometry_is_exact_and_bounded() {
        assert_eq!(graph_account_count(1), Ok(16));
        assert_eq!(graph_account_count(16), Ok(31));
        assert_eq!(MAX_FRACTIONAL_LIFECYCLE_ACCOUNTS, 48);
        assert!(graph_account_count(0).is_err());
        assert!(graph_account_count(17).is_err());
    }

    #[test]
    fn omitted_graph_roles_are_derived_from_current_authorities() {
        let source = include_str!("fractional_lifecycle.rs");
        assert!(source.contains("seeds::hoard_token_v2_pda("));
        assert!(source.contains("seeds::outcome_custody_v1_pda("));
        assert!(source.contains("revenue.treasury_position_account()"));
        assert!(source.contains("seeds::purpose_replay_v3_pda("));
        assert!(source.contains("revenue.treasury_service_ledger_account()"));
        assert!(!source.contains("outcomes.checked_mul(2)"));
    }

    #[test]
    fn action_one_prepares_product_before_physical_writes_and_commits_root_last() {
        let source = include_str!("fractional_lifecycle.rs");
        let start = source
            .find("pub(super) fn process_initialize")
            .expect("action-1 handler");
        let end = source[start..]
            .find("pub(super) fn process_close_empty_ledger")
            .map(|offset| start + offset)
            .expect("action-1 handler end");
        let body = &source[start..end];
        let prepare = body
            .find("prepare_fractional_admission_v3(")
            .expect("generic Product family preauthorization");
        let first_physical_write = body
            .find("allocate_prefunded_pda(")
            .expect("first physical Fractional write");
        let physical_postwrite = body
            .find("authenticate_fractional_family_admission_postwrite_v1(")
            .expect("hostile Fractional postwrite");
        let product_commit = body
            .find("commit_fractional_admission_v3(")
            .expect("RootV3 commit");
        assert!(prepare < first_physical_write);
        assert!(first_physical_write < physical_postwrite);
        assert!(physical_postwrite < product_commit);
        assert!(body.contains("authenticate_current_market_family_capability_policy_v1("));
        assert!(body.contains("authenticate_market_lifecycle_replay_v2("));
    }

    #[test]
    fn action_one_and_ten_have_distinct_exact_auxiliary_surfaces() {
        assert_eq!(INITIALIZE_AUX_ACCOUNTS, 17);
        assert_eq!(TERMINAL_AUX_ACCOUNTS, 17);
        assert_eq!(init_aux::CLAIM_TOKEN_PROGRAM, terminal_aux::CLAIM_TOKEN_PROGRAM);
        assert_eq!(
            init_aux::CLAIM_TOKEN_PROGRAMDATA,
            terminal_aux::CLAIM_TOKEN_PROGRAMDATA
        );
        assert_ne!(terminal_aux::CLAIM_TOKEN_PROGRAM, terminal_aux::REFUND_OWNER);
    }

    #[test]
    fn action_ten_permits_only_correlated_claim_loader_alias_roles() {
        let graph = 17;
        assert!(permitted_claim_loader_alias(
            false,
            graph,
            graph + terminal_aux::COLLATERAL_TOKEN_PROGRAM,
            graph + terminal_aux::CLAIM_TOKEN_PROGRAM,
        ));
        assert!(permitted_claim_loader_alias(
            false,
            graph,
            graph + terminal_aux::COLLATERAL_TOKEN_PROGRAMDATA,
            graph + terminal_aux::CLAIM_TOKEN_PROGRAMDATA,
        ));
        assert!(!permitted_claim_loader_alias(
            false,
            graph,
            graph + terminal_aux::COLLATERAL_TOKEN_PROGRAM,
            graph + terminal_aux::CLAIM_TOKEN_PROGRAMDATA,
        ));
        assert!(!permitted_claim_loader_alias(
            false,
            graph,
            graph + terminal_aux::COLLATERAL_TOKEN_PROGRAMDATA,
            graph + terminal_aux::CLAIM_TOKEN_PROGRAM,
        ));
    }

    #[test]
    fn action_ten_closes_fractional_accounts_before_product_consumption() {
        let source = include_str!("fractional_lifecycle.rs");
        let start = source
            .find("pub(super) fn process_close_empty_ledger")
            .expect("action-10 handler");
        let body = &source[start..];
        let physical_close = body
            .find("execute_fractional_family_physical_terminal_v2(")
            .expect("physical terminal execution");
        let product_consume = body
            .find("consume_fractional_family_terminal_postwrite_v2(")
            .expect("Product terminal consumption");
        assert!(physical_close < product_consume);
        assert!(body.contains("prepare_fractional_family_physical_terminal_v2("));
        assert!(!body[..product_consume].contains("accepted.id()"));
    }
}
