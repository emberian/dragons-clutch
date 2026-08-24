//! SourcePlane V3 mutation handlers entered only by exact capability tuples.
//!
//! The central SourceSeries 77/v2 coordinates are allocated and admitted only
//! as one complete family by the central profile. This module owns their inner execution: semantic outputs are
//! checked before instruction success, predictable accounts are prefund-safe,
//! mutable postimages advance their durable lineage in the same rollback
//! domain, immutable accounts retain an explicit payer/donation rent partition,
//! and every paid transition emits the exact Source receipt plus liveness
//! intent. Every lifecycle child is capitalized from the schedule-selected
//! program-derived custody; keepers sign calls but never supply account rent.

use std::vec;
use std::vec::Vec;

use clutch_liveness::runtime_adapter_v1::{
    plan_runtime_transition_v1, RuntimeAtomicTransitionV1, RuntimePersistedAccountViewV1,
    RuntimeReceiptObservationV1, RuntimeTransferRoleV1, RuntimeTransitionIntentV1,
};
use clutch_liveness::Id as LivenessId;
use clutch_liveness::runtime_v1::{
    PresentFundingSourceV1, PresentFundingV1, RuntimeCompartmentAdmissionV1,
    RuntimeCompartmentIdentityV1, RuntimeCompartmentKindV1, RuntimeCompartmentV1,
    RuntimeLivenessPolicyV1, RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
    RUNTIME_LIVENESS_POLICY_BYTES_V1,
};
use clutch_source_plane_v3::{
    ContentId, FixedCodec, OpenRawPageV3, SourceHeadV3, SourcePlaneProgramV3, StatisticKeyV3,
    StatisticResultV3, SummaryProgramV3, WindowSpecV3, WindowWorkV3,
    OPEN_RAW_PAGE_BYTES, RAW_PAGE_BYTES, SOURCE_HEAD_BYTES, STATISTIC_KEY_BYTES,
    STATISTIC_RESULT_BYTES, SUMMARY_PROGRAM_BYTES, WINDOW_SEAL_BYTES, WINDOW_SPEC_BYTES,
    WINDOW_WORK_BYTES,
};
use clutch_product_series::{CompiledSourceOccurrenceV3, FixedCodec as ProductFixedCodec};
use clutch_source_plane_v3_adapter::PdaRecipeV3;
pub use clutch_source_plane_v3_runtime::SourcePolicyHandoffJoinV1;
use clutch_source_plane_v3_runtime::{
    account_data_id, advance_lineage_state, authenticate_persisted_source_policy_handoff,
    authenticate_source_failure_terminal,
    authenticate_source_no_reopen_terminal, authenticate_source_work_receipt_account,
    authorize_reopen, close_lineage_generation, retire_never_created_lineage,
    decode_runtime_account, encode_runtime_account, initialize_source_head,
    open_lineage_generation, plan_runtime_account_close_from_header,
    plan_source_account_creation, AccountCloseFundingV1, AccountCreationFundingV1,
    AuthenticatedBoundaryV1, AuthenticatedClockBucketV1, AuthenticatedEvaluationV1,
    AuthenticatedOpenRawPageV1, AuthenticatedRawPageV1, AuthenticatedReceiverRouteV2,
    AuthenticatedPersistedSourcePolicyHandoffV1, AuthenticatedReopenLineageV1,
    AuthenticatedSourceGenerationV1, AuthenticatedSourceHeadV1, AuthenticatedSourceRouteV1,
    AuthenticatedSourceFailureTerminalV1, AuthenticatedSourceNoReopenTerminalV1,
    AuthenticatedSourceWorkReceiptV1,
    AuthenticatedStatisticResultAbsenceV1, AuthenticatedStatisticResultAccountV1,
    AuthenticatedWindowEvidenceV1, AuthenticatedWindowWorkV1, BoundaryBatchV1, ClockPolicyV1,
    ClockSnapshotV1, EvaluationReleaseBindingV1, FailurePolicySourceHandoffV1, IngestBatchOutputV1,
    LineageFamilyV1, RentExemptionQuoteV1, ReopenLineageV1, RuntimeAccountBodyV1,
    RuntimeAccountHeaderV1, RuntimeAccountViewV1, RuntimeKey, SealBatchModeV1,
    SourceFailureTerminalAccessV1, SourceFailureTerminalV1,
    SourcePolicyHandoffAccessV1, SourcePolicyHandoffAccountV1, SourceReleaseManifestV2,
    SourceNoReopenTerminalAccessV1, SourceNoReopenTerminalV1, SourceReceiptDispositionV1,
    SourceGenerationRequestV1, SourceReopenGenerationRequestV1,
    SourceTerminalAuthorizationV1, SourceTerminalOutcomeV1,
    SourceWorkAuthorizationV1, SourceWorkKindV1, SourceWorkReceiptAccessV1,
    SourceWorkReceiptAccountV1, SourceWorkScheduleBindingV1, SuccessfulEvaluationHandoffV1,
    source_runtime_liveness_policy_id_v1,
    REOPEN_LINEAGE_BYTES, RUNTIME_ACCOUNT_HEADER_BYTES,
    SourceFundingCustodyLedgerV1, SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES,
    SOURCE_FAILURE_TERMINAL_BYTES, SOURCE_NO_REOPEN_TERMINAL_BYTES,
    SOURCE_REOPEN_GENERATION_REQUEST_BYTES,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    allocate_data, assign_data, create_pda_account, read_rent, require_creatable,
    require_system_program, RentParameters, SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use crate::source_plane_v3::{
    derive_runtime_pda, invoke_parser_boundary, invoke_statistic_evaluator,
    authenticate_reopen_generation_request_before_close,
    project_liveness_receipt, project_liveness_terminal_intent, project_liveness_work_intent,
    runtime_key, SourceV3SbfError,
};
use clutch_solana_layout::artifact::ArtifactKind;

const SOURCE_FUNDING_CUSTODY_AUTH_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/authenticated-source-funding-custody/v1";
const SOURCE_FUNDING_CUSTODY_BOOTSTRAP_AUTH_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/authenticated-source-funding-custody-bootstrap/v1";
const SOURCE_FUNDING_CUSTODY_PHYSICAL_TRANSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-funding-custody-physical-transition/v1";

/// Exact program-owned prepaid custody ledger selected by one Source schedule.
///
/// This receipt is constructed only after route/schedule/PDA/owner/privilege
/// authentication. It contains no caller-selected amount and cannot authorize
/// a transfer except through the typed Source account constructors below.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceFundingCustodyV1 {
    id: ContentId,
    account: RuntimeKey,
    lifecycle_id: ContentId,
    source_work_schedule_id: ContentId,
    account_data_id: ContentId,
    ledger: SourceFundingCustodyLedgerV1,
}

impl AuthenticatedSourceFundingCustodyV1 {
    /// Exact program-owned custody-ledger PDA.
    pub(crate) const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Immutable Source lifecycle selecting the PDA.
    pub(crate) const fn lifecycle_id(self) -> ContentId {
        self.lifecycle_id
    }

    /// Exact heterogeneous schedule selecting this custody.
    pub(crate) const fn source_work_schedule_id(self) -> ContentId {
        self.source_work_schedule_id
    }

    /// Exact hostile-decoded principal/donation body.
    pub(crate) const fn ledger(self) -> SourceFundingCustodyLedgerV1 {
        self.ledger
    }

    /// Digest of the exact current ledger account bytes.
    pub(crate) const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    /// Complete route/schedule/PDA authentication identity.
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }
}

/// Private exact post-transfer bootstrap. It can only be consumed by the
/// immediate capitalization-receipt binding below and is never returned by a
/// dispatcher or lifecycle composer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceFundingCustodyBootstrapV1 {
    id: ContentId,
    account: RuntimeKey,
    account_data_id: ContentId,
    ledger: SourceFundingCustodyLedgerV1,
}

impl AuthenticatedSourceFundingCustodyBootstrapV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    pub(crate) const fn ledger(self) -> SourceFundingCustodyLedgerV1 {
        self.ledger
    }
}

/// Authenticate a permissionless prepaid Source rent custody. The program-
/// owned fixed body is the sole semantic owner of remaining principal and
/// observed donations; it is writable but never a transaction signer.
pub(crate) fn authenticate_source_funding_custody_v1(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSourceFundingCustodyV1> {
    schedule.validate_against(route).map_err(source_runtime)?;
    let (address, _) =
        seeds::source_funding_custody_pda(program_id, &schedule.lifecycle_id().bytes());
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let ledger = SourceFundingCustodyLedgerV1::decode(&data).map_err(source_runtime)?;
    let account_data_id = account_data_id(runtime_key(account.key), &data).map_err(source_runtime)?;
    let explained_balance = ledger
        .remaining_principal_lamports
        .checked_add(ledger.donation_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        schedule.payer() == runtime_key(account.key)
            && account.key == &address
            && account.owner == program_id
            && account.is_writable
            && !account.is_signer
            && !account.executable,
        ClutchError::MismatchedState,
    )?;
    require(
        ledger.adapter_program == runtime_key(program_id)
            && ledger.is_live()
            && ledger.release_manifest_id == route.release_manifest_id()
            && ledger.route_id == route.route_id()
            && ledger.source_work_schedule_id == schedule.source_work_schedule_id()
            && ledger.lifecycle_id == schedule.lifecycle_id()
            && ledger.custody_account == runtime_key(account.key)
            && ledger.neutral_sink == route.neutral_sink()
            && account.lamports() >= explained_balance,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_FUNDING_CUSTODY_AUTH_DOMAIN_V1,
            &route.route_id().bytes(),
            &schedule.source_work_schedule_id().bytes(),
            &schedule.lifecycle_id().bytes(),
            account.key.as_ref(),
            &account_data_id.bytes(),
            &ledger.id().map_err(source_runtime)?.bytes(),
            &account.lamports().to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceFundingCustodyV1 {
        id,
        account: runtime_key(account.key),
        lifecycle_id: schedule.lifecycle_id(),
        source_work_schedule_id: schedule.source_work_schedule_id(),
        account_data_id,
        ledger,
    })
}

/// Assign the capitalized PDA to the program and write its initial exact
/// principal ledger. The Product preauthorization identity and immutable
/// FundingTerms refund are private composer inputs, never instruction bytes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn initialize_source_funding_custody_bootstrap_v1(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    capitalization_authority_id: ContentId,
    principal_refund: RuntimeKey,
    allocated_principal_lamports: u64,
    custody_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent: &RentParameters,
) -> Outcome<AuthenticatedSourceFundingCustodyBootstrapV1> {
    require_system_program(system_program)?;
    schedule.validate_against(route).map_err(source_runtime)?;
    let lifecycle = schedule.lifecycle_id().bytes();
    let (expected, bump) = seeds::source_funding_custody_pda(program_id, &lifecycle);
    require(
        custody_account.key == &expected
            && custody_account.owner == &SYSTEM_PROGRAM_ID
            && custody_account.data_is_empty()
            && custody_account.is_writable
            && !custody_account.is_signer
            && !custody_account.executable
            && custody_account.lamports() == allocated_principal_lamports
            && allocated_principal_lamports
                >= rent.minimum_balance(SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES)?,
        ClutchError::MismatchedState,
    )?;
    let ledger = SourceFundingCustodyLedgerV1::new_bootstrap(
        runtime_key(program_id),
        route.release_manifest_id(),
        route.route_id(),
        schedule.source_work_schedule_id(),
        schedule.lifecycle_id(),
        runtime_key(custody_account.key),
        principal_refund,
        route.neutral_sink(),
        allocated_principal_lamports,
        capitalization_authority_id,
    )
    .map_err(source_runtime)?;
    let bump_seed = [bump];
    let signer_seeds: &[&[u8]] = &[
        seeds::SEED_SOURCE_FUNDING_CUSTODY_V1,
        &lifecycle,
        &bump_seed,
    ];
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES),
        vec![AccountMeta::new(*custody_account.key, true)],
    );
    invoke_signed(
        &allocate,
        &[custody_account.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*custody_account.key, true)],
    );
    invoke_signed(
        &assign,
        &[custody_account.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    write_exact_account_data(
        custody_account,
        &ledger.encode().map_err(source_runtime)?,
    )?;
    let data = custody_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let reopened = SourceFundingCustodyLedgerV1::decode(&data).map_err(source_runtime)?;
    let account_data_id = account_data_id(runtime_key(custody_account.key), &data)
        .map_err(source_runtime)?;
    require(
        reopened == ledger
            && !reopened.is_live()
            && custody_account.owner == program_id
            && custody_account.is_writable
            && !custody_account.is_signer
            && !custody_account.executable
            && data.len() == SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES
            && custody_account.lamports() == allocated_principal_lamports
            && reopened.capitalization_authority_id == capitalization_authority_id,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_FUNDING_CUSTODY_BOOTSTRAP_AUTH_DOMAIN_V1,
            &route.route_id().bytes(),
            &schedule.source_work_schedule_id().bytes(),
            &schedule.lifecycle_id().bytes(),
            custody_account.key.as_ref(),
            &account_data_id.bytes(),
            &reopened.id().map_err(source_runtime)?.bytes(),
            &custody_account.lamports().to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceFundingCustodyBootstrapV1 {
        id,
        account: runtime_key(custody_account.key),
        account_data_id,
        ledger: reopened,
    })
}

/// Complete the one-way bootstrap transition and return the sole live custody
/// authority. The caller passes the receipt it just computed over `bootstrap`;
/// any later transaction sees only the live body.
pub(crate) fn bind_source_funding_custody_capitalization_v1(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    bootstrap: AuthenticatedSourceFundingCustodyBootstrapV1,
    capitalization_receipt_id: ContentId,
    custody_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSourceFundingCustodyV1> {
    let data = custody_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let reopened = SourceFundingCustodyLedgerV1::decode(&data).map_err(source_runtime)?;
    let account_data_id = account_data_id(runtime_key(custody_account.key), &data)
        .map_err(source_runtime)?;
    require(
        bootstrap.account == runtime_key(custody_account.key)
            && bootstrap.account_data_id == account_data_id
            && bootstrap.ledger == reopened
            && !reopened.is_live(),
        ClutchError::MismatchedState,
    )?;
    drop(data);
    let live = reopened
        .bind_capitalization_receipt(capitalization_receipt_id)
        .map_err(source_runtime)?;
    write_exact_account_data(custody_account, &live.encode().map_err(source_runtime)?)?;
    let authenticated =
        authenticate_source_funding_custody_v1(program_id, route, schedule, custody_account)?;
    require(
        authenticated.ledger() == live
            && authenticated.ledger().capitalization_receipt_id == capitalization_receipt_id,
        ClutchError::MismatchedState,
    )?;
    Ok(authenticated)
}

/// Private postwrite proving the release-selected generic liveness policy and
/// its exact Source compartment were created from the prepaid lifecycle
/// custody. Product consumes this receipt before admitting the occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceLifecycleAdmissionV1 {
    policy_account: RuntimeKey,
    policy_account_data_id: ContentId,
    compartment_account: RuntimeKey,
    compartment_account_data_id: ContentId,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_balance_after: u64,
    id: ContentId,
}

/// Exact SourceWork lamport requirement consumed by Product's current QuoteV5.
/// The reserve deliberately budgets the largest possible child+lineage pair
/// for every admitted call, so no valid schedule can be stranded by a later
/// family choice. Unused principal remains in the authenticated custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceLifecycleCapitalizationQuoteV1 {
    pub(crate) liveness_work_lamports: u64,
    pub(crate) permanent_and_child_rent_lamports: u64,
    pub(crate) total_lamports: u64,
    pub(crate) id: ContentId,
}

/// Derive the exact fully-prepaid SourceWork quote from the immutable schedule
/// and live Rent sysvar. No target account prefund discounts this requirement.
pub(crate) fn quote_source_lifecycle_capitalization_v1(
    schedule: SourceWorkScheduleBindingV1,
    rent: &RentParameters,
) -> Outcome<SourceLifecycleCapitalizationQuoteV1> {
    fn add_total(total: u64, value: u64) -> Outcome<u64> {
        total
            .checked_add(value)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
    }
    let pre_root_spaces = [
        SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES,
        RUNTIME_LIVENESS_POLICY_BYTES_V1,
        RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
        clutch_product_series::SOURCE_OCCURRENCE_RECORD_BYTES,
        WINDOW_SPEC_BYTES,
        SUMMARY_PROGRAM_BYTES,
        STATISTIC_KEY_BYTES,
        clutch_source_plane_v3_runtime::SOURCE_GENERATION_REQUEST_BYTES,
        REOPEN_LINEAGE_BYTES,
    ];
    let mut rent_total = 0_u64;
    for space in pre_root_spaces {
        rent_total = add_total(rent_total, rent.minimum_balance(space)?)?;
    }
    let largest_runtime_body = OPEN_RAW_PAGE_BYTES
        .max(RAW_PAGE_BYTES)
        .max(SOURCE_HEAD_BYTES)
        .max(WINDOW_WORK_BYTES)
        .max(WINDOW_SEAL_BYTES)
        .max(STATISTIC_RESULT_BYTES);
    let per_call_rent = add_total(
        rent.minimum_balance(RUNTIME_ACCOUNT_HEADER_BYTES + largest_runtime_body)?,
        rent.minimum_balance(REOPEN_LINEAGE_BYTES)?,
    )?;
    let per_call_rent = add_total(
        per_call_rent,
        rent.minimum_balance(clutch_source_plane_v3_runtime::SOURCE_WORK_RECEIPT_ACCOUNT_BYTES)?,
    )?;
    rent_total = add_total(
        rent_total,
        per_call_rent
            .checked_mul(u64::from(schedule.maximum_calls()))
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
    )?;
    let terminal_policy_space = SOURCE_NO_REOPEN_TERMINAL_BYTES
        .max(SOURCE_REOPEN_GENERATION_REQUEST_BYTES)
        .max(SOURCE_FAILURE_TERMINAL_BYTES);
    rent_total = add_total(rent_total, rent.minimum_balance(terminal_policy_space)?)?;
    rent_total = add_total(
        rent_total,
        rent.minimum_balance(clutch_source_plane_v3_runtime::SOURCE_WORK_RECEIPT_ACCOUNT_BYTES)?,
    )?;
    let total_lamports = add_total(schedule.work_capital_lamports(), rent_total)?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/sbf/source-lifecycle-capitalization-quote/v1",
            &schedule.source_work_schedule_id().bytes(),
            &schedule.lifecycle_id().bytes(),
            &schedule.work_capital_lamports().to_le_bytes(),
            &rent_total.to_le_bytes(),
            &total_lamports.to_le_bytes(),
            &rent.lamports_per_byte_year.to_le_bytes(),
            &rent.exemption_threshold.to_bits().to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(SourceLifecycleCapitalizationQuoteV1 {
        liveness_work_lamports: schedule.work_capital_lamports(),
        permanent_and_child_rent_lamports: rent_total,
        total_lamports,
        id,
    })
}

impl AuthenticatedSourceLifecycleAdmissionV1 {
    pub(crate) const fn policy_account(self) -> RuntimeKey {
        self.policy_account
    }

    pub(crate) const fn policy_account_data_id(self) -> ContentId {
        self.policy_account_data_id
    }

    pub(crate) const fn compartment_account(self) -> RuntimeKey {
        self.compartment_account
    }

    pub(crate) const fn compartment_account_data_id(self) -> ContentId {
        self.compartment_account_data_id
    }

    pub(crate) const fn custody(self) -> AuthenticatedSourceFundingCustodyV1 {
        self.custody
    }

    pub(crate) const fn custody_balance_after(self) -> u64 {
        self.custody_balance_after
    }

    pub(crate) const fn id(self) -> ContentId {
        self.id
    }
}

/// Persist the exact content-addressed runtime-liveness policy and admit its
/// Source compartment from the same lifecycle custody. The full policy body
/// is self-authenticating and already selected by SourceReleaseManifestV2;
/// Product additionally checks its Realm before it calls this private owner.
#[allow(clippy::too_many_arguments)]
pub(crate) fn admit_source_lifecycle_v1(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    policy: RuntimeLivenessPolicyV1,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    policy_account: &AccountInfo<'_>,
    compartment_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSourceLifecycleAdmissionV1> {
    schedule.validate_against(route).map_err(source_runtime)?;
    let policy_id = source_runtime_liveness_policy_id_v1(policy).map_err(source_runtime)?;
    let source_policy = policy.compartment(RuntimeCompartmentKindV1::Source);
    require(
        policy_id == route.liveness_policy_id()
            && policy.policy_id.bytes() == route.liveness_policy_id().bytes()
            && policy.neutral_sink.bytes() == route.neutral_sink().bytes()
            && source_policy.quote_schedule_id.bytes()
                == schedule.source_work_schedule_id().bytes()
            && source_policy.receipt_program_id.bytes() == program_id.to_bytes()
            && source_policy.maximum_calls == schedule.maximum_calls()
            && source_policy.maximum_lamports_per_call
                == schedule.maximum_lamports_per_call()
            && source_policy.work_capital_lamports == schedule.work_capital_lamports()
            && source_policy.account_rent_principal_lamports
                == schedule.rent_principal_lamports()
            && route.source_compartment_owner() == runtime_key(program_id)
            && custody.account() == runtime_key(custody_account.key)
            && custody.lifecycle_id() == schedule.lifecycle_id(),
        ClutchError::MismatchedState,
    )?;
    let terminal_calls = schedule.terminal_path_calls();
    let terminal_work = schedule.terminal_path_work_lamports();
    let mut path_index = 0_usize;
    while path_index < policy.terminal_paths.len() {
        require(
            policy.terminal_paths[path_index].calls_for(RuntimeCompartmentKindV1::Source)
                == terminal_calls[path_index]
                && policy.terminal_paths[path_index]
                    .work_lamports_for(RuntimeCompartmentKindV1::Source)
                    == terminal_work[path_index],
            ClutchError::MismatchedState,
        )?;
        path_index += 1;
    }
    let rent = read_rent(rent_sysvar)?;
    let policy_minimum = rent.minimum_balance(RUNTIME_LIVENESS_POLICY_BYTES_V1)?;
    let compartment_minimum = rent.minimum_balance(RUNTIME_LIVENESS_ACCOUNT_BYTES_V1)?;
    require(
        compartment_minimum == schedule.rent_principal_lamports()
            && policy_account.is_writable
            && !policy_account.is_signer
            && !policy_account.executable
            && compartment_account.is_writable
            && !compartment_account.is_signer
            && !compartment_account.executable
            && compartment_account.lamports() == 0
            && compartment_account.data_is_empty()
            && policy_account.key != compartment_account.key
            && policy_account.key != custody_account.key
            && compartment_account.key != custody_account.key,
        ClutchError::MismatchedState,
    )?;
    let (expected_policy, policy_bump) =
        seeds::source_liveness_policy_pda(program_id, &policy_id.bytes());
    let (expected_compartment, compartment_bump) =
        seeds::source_compartment_pda(program_id, &schedule.lifecycle_id().bytes());
    require(
        policy_account.key == &expected_policy
            && compartment_account.key == &expected_compartment
            && runtime_key(compartment_account.key) == route.source_compartment_account(),
        ClutchError::WrongPda,
    )?;
    let policy_bump_seed = [policy_bump];
    create_with_raw_seeds_from_custody(
        program_id,
        custody,
        custody_account,
        policy_account,
        system_program,
        &rent,
        RUNTIME_LIVENESS_POLICY_BYTES_V1,
        &[
            seeds::SEED_SOURCE_LIVENESS_POLICY_V1,
            &policy_id.bytes(),
            &policy_bump_seed,
        ],
    )?;
    let mut policy_bytes = [0_u8; RUNTIME_LIVENESS_POLICY_BYTES_V1];
    policy
        .encode(&mut policy_bytes)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_exact_account_data(policy_account, &policy_bytes)?;

    let compartment_bump_seed = [compartment_bump];
    create_with_raw_seeds_from_custody(
        program_id,
        custody,
        custody_account,
        compartment_account,
        system_program,
        &rent,
        RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
        &[
            seeds::SEED_SOURCE_COMPARTMENT_V1,
            &schedule.lifecycle_id().bytes(),
            &compartment_bump_seed,
        ],
    )?;
    transfer_from_source_custody_v1(
        program_id,
        custody,
        custody_account,
        compartment_account,
        system_program,
        schedule.work_capital_lamports(),
    )?;
    require(
        compartment_account.lamports()
            == schedule
                .work_capital_lamports()
                .checked_add(compartment_minimum)
                .ok_or(ClutchError::Arithmetic)?,
        ClutchError::MismatchedState,
    )?;
    let state = RuntimeCompartmentV1::admit(
        policy,
        RuntimeCompartmentAdmissionV1 {
            kind: RuntimeCompartmentKindV1::Source,
            identity: RuntimeCompartmentIdentityV1 {
                policy_id: policy.policy_id,
                lifecycle_id: LivenessId::from_bytes(schedule.lifecycle_id().bytes()),
                account_id: LivenessId::from_bytes(compartment_account.key.to_bytes()),
                owner: LivenessId::from_bytes(route.source_compartment_owner().bytes()),
                payer: LivenessId::from_bytes(custody.account().bytes()),
                neutral_sink: policy.neutral_sink,
                generation: schedule.generation(),
            },
            funding: PresentFundingV1 {
                payer: LivenessId::from_bytes(custody.account().bytes()),
                source: PresentFundingSourceV1::PrecapitalizedLivenessEndowment,
                payer_debit_lamports: schedule
                    .work_capital_lamports()
                    .checked_add(compartment_minimum)
                    .ok_or(ClutchError::Arithmetic)?,
                account_balance_before: 0,
                account_balance_after: compartment_account.lamports(),
            },
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut compartment_bytes = [0_u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1];
    state
        .encode(&mut compartment_bytes)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_exact_account_data(compartment_account, &compartment_bytes)?;
    let policy_data_id = account_data_id(runtime_key(policy_account.key), &policy_bytes)
        .map_err(source_runtime)?;
    let compartment_data_id = account_data_id(
        runtime_key(compartment_account.key),
        &compartment_bytes,
    )
    .map_err(source_runtime)?;
    let custody_balance_after = custody_account.lamports();
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/sbf/authenticated-source-lifecycle-admission/v1",
            &route.route_id().bytes(),
            &schedule.source_work_schedule_id().bytes(),
            &policy_id.bytes(),
            &policy_account.key.to_bytes(),
            &policy_data_id.bytes(),
            &compartment_account.key.to_bytes(),
            &compartment_data_id.bytes(),
            &custody.id().bytes(),
            &policy_minimum.to_le_bytes(),
            &custody_balance_after.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceLifecycleAdmissionV1 {
        policy_account: runtime_key(policy_account.key),
        policy_account_data_id: policy_data_id,
        compartment_account: runtime_key(compartment_account.key),
        compartment_account_data_id: compartment_data_id,
        custody,
        custody_balance_after,
        id,
    })
}

/// Complete open-account postimage committed with one lineage postimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenRuntimeAccountResultV1 {
    /// Exact prefund-safe payer/donation partition.
    pub funding: AccountCreationFundingV1,
    /// Permanent lineage-account rent funding when this action bootstrapped
    /// the lineage; absent for an already-authenticated reopen.
    pub lineage_funding: Option<ImmutableAccountFundingV1>,
    /// Globally tagged account header written onchain.
    pub header: RuntimeAccountHeaderV1,
    /// Digest of the complete account postimage.
    pub account_data_id: ContentId,
    /// Durable lineage postimage written atomically.
    pub lineage_after: ReopenLineageV1,
}

/// Permanent never-opened StatisticResult lineage created with the Product
/// occurrence. Its existence lets action 10 distinguish true mature absence
/// from a missing or substituted lineage account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreallocatedStatisticResultLineageV1 {
    funding: ImmutableAccountFundingV1,
    authenticated: AuthenticatedReopenLineageV1,
    id: ContentId,
}

impl PreallocatedStatisticResultLineageV1 {
    pub(crate) const fn authenticated(self) -> AuthenticatedReopenLineageV1 {
        self.authenticated
    }

    pub(crate) const fn funding(self) -> ImmutableAccountFundingV1 {
        self.funding
    }

    pub(crate) const fn id(self) -> ContentId {
        self.id
    }
}

/// Complete in-place state/lineage compare-and-swap result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MutateRuntimeAccountResultV1 {
    /// Digest of the complete account preimage.
    pub account_data_before_id: ContentId,
    /// Digest of the complete account postimage.
    pub account_data_after_id: ContentId,
    /// Durable lineage postimage written atomically.
    pub lineage_after: ReopenLineageV1,
}

/// Exact action-4 semantic result plus the committed account CAS receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistedIngestBoundaryBatchV1 {
    /// Pure Source V3 ingest result.
    pub semantic: IngestBatchOutputV1,
    /// Exact OpenRawPage preimage/postimage and lineage mutation receipt.
    pub mutation: MutateRuntimeAccountResultV1,
}

/// Exact action-7 semantic fold plus committed WindowWork compare-and-swap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistedFoldWindowPagesV1 {
    /// Pure ordered page-fold result.
    pub semantic: clutch_source_plane_v3_runtime::FoldPagesOutputV1,
    /// Exact WindowWork preimage/postimage and lineage mutation receipt.
    pub mutation: MutateRuntimeAccountResultV1,
}

/// Complete close split and lineage tombstone postimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseRuntimeAccountResultV1 {
    /// Exact payer-principal versus neutral-surplus split.
    pub funding: AccountCloseFundingV1,
    /// Durable closed lineage postimage.
    pub lineage_after: ReopenLineageV1,
}

/// Permanent tombstone of an exact never-created StatisticResult lineage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedAbsentStatisticResultLineageRetirementV1 {
    id: ContentId,
    lineage_account: RuntimeKey,
    lineage_authentication_before_id: ContentId,
    lineage_state_before_id: ContentId,
    lineage_state_after_id: ContentId,
    lineage_after: ReopenLineageV1,
}

impl AuthenticatedAbsentStatisticResultLineageRetirementV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn lineage_account(self) -> RuntimeKey {
        self.lineage_account
    }

    pub(crate) const fn lineage_authentication_before_id(self) -> ContentId {
        self.lineage_authentication_before_id
    }

    pub(crate) const fn lineage_state_before_id(self) -> ContentId {
        self.lineage_state_before_id
    }

    pub(crate) const fn lineage_state_after_id(self) -> ContentId {
        self.lineage_state_after_id
    }

    pub(crate) const fn lineage_after(self) -> ReopenLineageV1 {
        self.lineage_after
    }
}

/// Explicit rent ownership for a permanent immutable Source evidence account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImmutableAccountFundingV1 {
    /// Physical immutable account.
    pub account: RuntimeKey,
    /// Payer of the exact rent shortfall, or zero for a fully prefunded PDA.
    pub payer: RuntimeKey,
    /// Exact shortfall supplied by `payer`.
    pub payer_debit_lamports: u64,
    /// Pre-existing lamports classified as neutral donation.
    pub donation_lamports: u64,
    /// Runtime Rent-sysvar identity.
    pub rent_sysvar_id: ContentId,
    /// Exact rent-exempt floor for the allocated bytes.
    pub rent_exempt_minimum_lamports: u64,
    /// Post-create balance.
    pub account_balance_after: u64,
}

/// Exact permanent funding observation for one raw immutable Source semantic
/// input account. These accounts have no terminal close or refund path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImmutableSourceInputFundingV1 {
    /// Physical content-addressed account.
    pub account: RuntimeKey,
    /// Payer of this call's exact rent shortfall, or zero for exact-existing/prefund.
    pub payer: RuntimeKey,
    /// Exact lamports supplied by this call.
    pub payer_debit_lamports: u64,
    /// Lamports already present before creation or observed on exact-existing input.
    pub permanent_prefund_lamports: u64,
    /// Digest of the complete canonical raw semantic body.
    pub account_data_id: ContentId,
    /// Exact semantic identity used by the content-addressed PDA.
    pub semantic_id: ContentId,
}

/// Private postwrite receipt for the three immutable Source semantic inputs of
/// one authenticated Product occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublishedSourceSemanticInputsV1 {
    /// Private Product/Profile/Bundle publication authority consumed.
    publication_authorization_id: ContentId,
    /// Canonical WindowSpec publication observation.
    window: ImmutableSourceInputFundingV1,
    /// Reviewed SummaryProgram publication observation.
    summary: ImmutableSourceInputFundingV1,
    /// Predictable StatisticKey publication observation.
    statistic_key: ImmutableSourceInputFundingV1,
    /// Identity of the exact three-account postwrite join.
    receipt_id: ContentId,
}

/// Private Product-owned immutable occurrence postwrite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PublishedSourceOccurrenceV1 {
    occurrence: CompiledSourceOccurrenceV3,
    funding: ImmutableSourceInputFundingV1,
    id: ContentId,
}

impl PublishedSourceOccurrenceV1 {
    pub(crate) const fn occurrence(self) -> CompiledSourceOccurrenceV3 {
        self.occurrence
    }

    pub(crate) const fn account(self) -> RuntimeKey {
        self.funding.account
    }

    pub(crate) const fn funding(self) -> ImmutableSourceInputFundingV1 {
        self.funding
    }

    pub(crate) const fn id(self) -> ContentId {
        self.id
    }
}

/// Immutable Product-owned initial/repair GenerationAuthority request written
/// under the release-selected authority before action 2 becomes callable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PersistedSourceGenerationRequestV1 {
    funding: ImmutableSourceInputFundingV1,
    request: SourceGenerationRequestV1,
    id: ContentId,
}

impl PersistedSourceGenerationRequestV1 {
    pub(crate) const fn funding(self) -> ImmutableSourceInputFundingV1 {
        self.funding
    }

    pub(crate) const fn request(self) -> SourceGenerationRequestV1 {
        self.request
    }

    pub(crate) const fn id(self) -> ContentId {
        self.id
    }
}

/// Exact durable action-10 postwrite and its permanent rent observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistedSourcePolicyHandoffV1 {
    funding: ImmutableSourceInputFundingV1,
    authenticated: AuthenticatedPersistedSourcePolicyHandoffV1,
}

/// Exact durable no-reopen decision and its permanent rent observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistedSourceNoReopenTerminalV1 {
    funding: ImmutableSourceInputFundingV1,
    authenticated: AuthenticatedSourceNoReopenTerminalV1,
}

/// Exact durable Source failure-terminal decision and rent observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PersistedSourceFailureTerminalV1 {
    funding: ImmutableSourceInputFundingV1,
    authenticated: AuthenticatedSourceFailureTerminalV1,
}

impl PersistedSourceFailureTerminalV1 {
    pub(crate) const fn funding(self) -> ImmutableSourceInputFundingV1 {
        self.funding
    }

    pub(crate) const fn authenticated(self) -> AuthenticatedSourceFailureTerminalV1 {
        self.authenticated
    }
}

/// Exact immutable GenerationAuthority reopen request published before the
/// deterministic action-12 close which produces its expected lineage state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistedSourceReopenGenerationRequestV1 {
    funding: ImmutableSourceInputFundingV1,
    request: SourceReopenGenerationRequestV1,
    postwrite_id: ContentId,
}

impl PersistedSourceReopenGenerationRequestV1 {
    /// Permanent prefund/rent observation for the request PDA.
    pub const fn funding(self) -> ImmutableSourceInputFundingV1 {
        self.funding
    }

    /// Exact reconstructed request body consumed by action 11.
    pub const fn request(self) -> SourceReopenGenerationRequestV1 {
        self.request
    }

    /// Exact GenerationAuthority owner/PDA/body postwrite identity.
    pub const fn id(self) -> ContentId {
        self.postwrite_id
    }
}

/// Private exhaustive terminal semantic accepted by the sole receipt minter.
/// Construction requires a physically authenticated no-reopen account or an
/// exact GenerationAuthority request postwrite; raw content IDs are refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceTerminalSemanticV1 {
    semantic_id: ContentId,
    persistence_authentication_id: ContentId,
}

impl AuthenticatedSourceTerminalSemanticV1 {
    /// Bind one durable explicit no-reopen postwrite.
    pub(crate) fn no_reopen(value: PersistedSourceNoReopenTerminalV1) -> Outcome<Self> {
        let semantic_id = value.authenticated().terminal_id().map_err(source_runtime)?;
        let persistence_authentication_id = value.authenticated().id();
        require(
            !semantic_id.is_zero() && !persistence_authentication_id.is_zero(),
            ClutchError::MismatchedState,
        )?;
        Ok(Self {
            semantic_id,
            persistence_authentication_id,
        })
    }

    /// Bind one exact GenerationAuthority request postwrite. Its terminal
    /// semantic is the non-circular policy ID embedded in the request.
    pub(crate) fn reopen_request(
        value: PersistedSourceReopenGenerationRequestV1,
    ) -> Outcome<Self> {
        let semantic_id = value.request().generation_policy_id();
        let persistence_authentication_id = value.id();
        require(
            !semantic_id.is_zero() && !persistence_authentication_id.is_zero(),
            ClutchError::MismatchedState,
        )?;
        Ok(Self {
            semantic_id,
            persistence_authentication_id,
        })
    }

    /// Bind one durable failure-terminal postwrite. The raw body ID never
    /// enters this constructor without exact owner/PDA/body authentication.
    pub(crate) fn source_failure(value: PersistedSourceFailureTerminalV1) -> Outcome<Self> {
        let semantic_id = value
            .authenticated()
            .body()
            .id()
            .map_err(source_runtime)?;
        let persistence_authentication_id = value.authenticated().id();
        require(
            !semantic_id.is_zero() && !persistence_authentication_id.is_zero(),
            ClutchError::MismatchedState,
        )?;
        Ok(Self {
            semantic_id,
            persistence_authentication_id,
        })
    }

    /// Exact semantic terminal identity carried into action 12.
    pub(crate) const fn semantic_id(self) -> ContentId {
        self.semantic_id
    }

    /// Exact prior persistence postwrite required to mint this semantic.
    pub(crate) const fn persistence_authentication_id(self) -> ContentId {
        self.persistence_authentication_id
    }
}

impl PersistedSourceNoReopenTerminalV1 {
    /// Permanent prefund/rent observation for the immutable terminal record.
    pub const fn funding(self) -> ImmutableSourceInputFundingV1 {
        self.funding
    }

    /// Exact owner/PDA/body postwrite authentication.
    pub const fn authenticated(self) -> AuthenticatedSourceNoReopenTerminalV1 {
        self.authenticated
    }
}

impl PersistedSourcePolicyHandoffV1 {
    /// Permanent content-addressed handoff-account funding.
    pub const fn funding(self) -> ImmutableSourceInputFundingV1 {
        self.funding
    }

    /// Exact owner/PDA/body postwrite receipt for Product consumption.
    pub const fn authenticated(self) -> AuthenticatedPersistedSourcePolicyHandoffV1 {
        self.authenticated
    }
}

impl PublishedSourceSemanticInputsV1 {
    /// Private Product/Profile/Bundle publication authority consumed.
    pub const fn publication_authorization_id(self) -> ContentId {
        self.publication_authorization_id
    }

    /// Canonical WindowSpec publication observation.
    pub const fn window(self) -> ImmutableSourceInputFundingV1 {
        self.window
    }

    /// Reviewed SummaryProgram publication observation.
    pub const fn summary(self) -> ImmutableSourceInputFundingV1 {
        self.summary
    }

    /// Predictable StatisticKey publication observation.
    pub const fn statistic_key(self) -> ImmutableSourceInputFundingV1 {
        self.statistic_key
    }

    /// Identity of the exact three-account postwrite join.
    pub const fn id(self) -> ContentId {
        self.receipt_id
    }
}

/// One persisted paid-work receipt and its sole liveness transition intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceWorkExecutionV1 {
    /// Canonical immutable 0x92 receipt body.
    pub receipt: SourceWorkReceiptAccountV1,
    /// Exact prefund/rent partition for the immutable receipt account.
    pub receipt_funding: ImmutableAccountFundingV1,
    /// Exact authenticated receipt observation consumed by liveness.
    pub observation: RuntimeReceiptObservationV1,
    /// Sole Source-compartment spend intent; it performs no second debit here.
    pub intent: RuntimeTransitionIntentV1,
    /// Same-call CreatedMutable authentication for private Source composers.
    authenticated_receipt: AuthenticatedSourceWorkReceiptV1,
}

impl SourceWorkExecutionV1 {
    /// Exact newly written receipt authenticated under CreatedMutable access.
    pub(crate) const fn authenticated_receipt(self) -> AuthenticatedSourceWorkReceiptV1 {
        self.authenticated_receipt
    }
}

/// One persisted terminal receipt and the sole liveness close intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceTerminalExecutionV1 {
    /// Canonical immutable 0x92 terminal receipt body.
    pub receipt: SourceWorkReceiptAccountV1,
    /// Exact prefund/rent partition for the immutable receipt account.
    pub receipt_funding: ImmutableAccountFundingV1,
    /// Exact authenticated terminal observation consumed by liveness.
    pub observation: RuntimeReceiptObservationV1,
    /// Sole Source-compartment terminal intent.
    pub intent: RuntimeTransitionIntentV1,
    /// Same-instruction authentication of the newly persisted receipt. The
    /// private terminal close consumes this CreatedMutable capability rather
    /// than reopening a writable creation account as read-only.
    authenticated_receipt: AuthenticatedSourceWorkReceiptV1,
}

impl SourceTerminalExecutionV1 {
    /// Exact CreatedMutable receipt retained for the same-call close.
    pub(crate) const fn authenticated_receipt(self) -> AuthenticatedSourceWorkReceiptV1 {
        self.authenticated_receipt
    }
}

/// Apply the Source work receipt's sole liveness debit in the same SBF
/// instruction as its parser/evaluator CPI and Source state mutation.
/// Any later refusal rolls every preceding CPI-visible write back.
#[allow(clippy::too_many_arguments)]
pub fn apply_source_work_liveness(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    execution: SourceWorkExecutionV1,
    policy_account: &AccountInfo<'_>,
    compartment_account: &AccountInfo<'_>,
    keeper: &AccountInfo<'_>,
    payer_refund: &AccountInfo<'_>,
) -> Outcome<RuntimeAtomicTransitionV1> {
    require(
        policy_account.owner == program_id
            && !policy_account.is_writable
            && !policy_account.is_signer
            && !policy_account.executable
            && compartment_account.owner == program_id
            && compartment_account.is_writable
            && !compartment_account.is_signer
            && !compartment_account.executable
            && keeper.is_writable
            && payer_refund.is_writable
            && !keeper.executable
            && !payer_refund.executable,
        ClutchError::MismatchedState,
    )?;
    require(
        runtime_key(compartment_account.key) == route.source_compartment_account()
            && policy_account.key != compartment_account.key
            && policy_account.key != keeper.key
            && policy_account.key != payer_refund.key
            && compartment_account.key != keeper.key
            && compartment_account.key != payer_refund.key,
        ClutchError::AccountAlias,
    )?;
    expect_pda(
        policy_account.key,
        seeds::source_liveness_policy_pda(program_id, &route.liveness_policy_id().bytes()),
        None,
    )?;
    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let compartment_data = compartment_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let balance_after = compartment_account
        .lamports()
        .checked_sub(execution.intent.call_ceiling_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let transition = plan_runtime_transition_v1(
        LivenessId::from_bytes(program_id.to_bytes()),
        LivenessId::from_bytes(policy_account.key.to_bytes()),
        RuntimePersistedAccountViewV1 {
            account_id: LivenessId::from_bytes(policy_account.key.to_bytes()),
            owner_program_id: LivenessId::from_bytes(policy_account.owner.to_bytes()),
            lamports: policy_account.lamports(),
            data: &policy_data,
            writable: policy_account.is_writable,
        },
        RuntimePersistedAccountViewV1 {
            account_id: LivenessId::from_bytes(compartment_account.key.to_bytes()),
            owner_program_id: LivenessId::from_bytes(compartment_account.owner.to_bytes()),
            lamports: compartment_account.lamports(),
            data: &compartment_data,
            writable: compartment_account.is_writable,
        },
        execution.intent,
        Some(execution.observation),
        balance_after,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(compartment_data);
    drop(policy_data);
    require(
        transition.write_account_data
            && !transition.close_account
            && transition.account_balance_before == compartment_account.lamports()
            && transition.account_balance_after == balance_after,
        ClutchError::MismatchedState,
    )?;
    let mut keeper_lamports = 0_u64;
    let mut payer_lamports = 0_u64;
    for movement in transition.transfers() {
        match movement.role {
            RuntimeTransferRoleV1::KeeperPayment => {
                require(
                    movement.destination == LivenessId::from_bytes(keeper.key.to_bytes())
                        && keeper_lamports == 0,
                    ClutchError::MismatchedState,
                )?;
                keeper_lamports = movement.lamports;
            }
            RuntimeTransferRoleV1::PayerWorkRefund => {
                require(
                    movement.destination == LivenessId::from_bytes(payer_refund.key.to_bytes())
                        && payer_lamports == 0,
                    ClutchError::MismatchedState,
                )?;
                payer_lamports = movement.lamports;
            }
            _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
        }
    }
    require(
        keeper_lamports == execution.intent.keeper_payment_lamports
            && payer_lamports
                == execution
                    .intent
                    .call_ceiling_lamports
                    .checked_sub(execution.intent.keeper_payment_lamports)
                    .ok_or(ClutchError::Arithmetic)?,
        ClutchError::MismatchedState,
    )?;
    let coalesced_recipient = keeper.key == payer_refund.key;
    let keeper_after = keeper
        .lamports()
        .checked_add(keeper_lamports)
        .and_then(|balance| {
            if coalesced_recipient {
                balance.checked_add(payer_lamports)
            } else {
                Some(balance)
            }
        })
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let payer_after = if coalesced_recipient {
        keeper_after
    } else {
        payer_refund
            .lamports()
            .checked_add(payer_lamports)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
    };
    {
        let mut data = compartment_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(
            data.len() == transition.post_account_data.len(),
            ClutchError::WrongDataLength,
        )?;
        data.copy_from_slice(&transition.post_account_data);
    }
    {
        let mut compartment_lamports = compartment_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **compartment_lamports = balance_after;
        if coalesced_recipient {
            let mut recipient_balance = keeper
                .try_borrow_mut_lamports()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            **recipient_balance = keeper_after;
        } else {
            let mut keeper_balance = keeper
                .try_borrow_mut_lamports()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            let mut payer_balance = payer_refund
                .try_borrow_mut_lamports()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            **keeper_balance = keeper_after;
            **payer_balance = payer_after;
        }
    }
    if payer_lamports != 0 {
        credit_source_funding_custody_ledger_from_close_v1(
            program_id,
            route,
            payer_refund,
            payer_lamports,
            execution.authenticated_receipt().id(),
        )?;
    }
    Ok(transition)
}

/// Exact one-shot post-terminal reopen payment from the lifecycle custody.
///
/// The terminal liveness transition has already returned every unused Source
/// work lamport to this custody. A persisted GenerationAuthority request and
/// its closed lineage make the reopen one-shot, while the immutable work
/// receipt binds the exact reopened postimage. The keeper signs only to accept
/// responsibility for this call; it never funds rent or becomes principal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedPostterminalSourceWorkV1 {
    id: ContentId,
    work_receipt_authentication_id: ContentId,
    custody: RuntimeKey,
    keeper: RuntimeKey,
    custody_balance_before: u64,
    custody_balance_after: u64,
    keeper_balance_before: u64,
    keeper_balance_after: u64,
    payment_lamports: u64,
}

impl AuthenticatedPostterminalSourceWorkV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }
}

/// Pay the exact release-selected TerminalLifecycle ceiling only after the
/// caller has persisted the work receipt for a successful deterministic
/// reopen. No generic liveness account is accepted after its terminal close.
pub(crate) fn apply_postterminal_source_work_from_custody_v1(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    work: SourceWorkExecutionV1,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    keeper: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
) -> Outcome<AuthenticatedPostterminalSourceWorkV1> {
    let receipt = work.authenticated_receipt();
    let payment_lamports = schedule.ceiling_for(SourceWorkKindV1::TerminalLifecycle);
    require(
        schedule.payer() == custody.account()
            && custody.account() == runtime_key(custody_account.key)
            && receipt.schedule() == schedule
            && receipt.receipt().work_kind() == Some(SourceWorkKindV1::TerminalLifecycle)
            && work.intent.call_ceiling_lamports == payment_lamports
            && work.intent.keeper_payment_lamports == payment_lamports
            && work.intent.keeper == LivenessId::from_bytes(keeper.key.to_bytes())
            && keeper.is_writable
            && keeper.is_signer
            && !keeper.executable
            && keeper.key != custody_account.key,
        ClutchError::MismatchedState,
    )?;
    let custody_balance_before = custody_account.lamports();
    let keeper_balance_before = keeper.lamports();
    transfer_from_source_custody_v1(
        program_id,
        custody,
        custody_account,
        keeper,
        system_program,
        payment_lamports,
    )?;
    let custody_balance_after = custody_account.lamports();
    let keeper_balance_after = keeper.lamports();
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/sbf/authenticated-postterminal-source-work/v1",
            &route.route_id().bytes(),
            &schedule.source_work_schedule_id().bytes(),
            &receipt.id().bytes(),
            &custody.id().bytes(),
            custody_account.key.as_ref(),
            keeper.key.as_ref(),
            &custody_balance_before.to_le_bytes(),
            &custody_balance_after.to_le_bytes(),
            &keeper_balance_before.to_le_bytes(),
            &keeper_balance_after.to_le_bytes(),
            &payment_lamports.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedPostterminalSourceWorkV1 {
        id,
        work_receipt_authentication_id: receipt.id(),
        custody: custody.account(),
        keeper: runtime_key(keeper.key),
        custody_balance_before,
        custody_balance_after,
        keeper_balance_before,
        keeper_balance_after,
        payment_lamports,
    })
}

/// Apply the sole Source terminal receipt and close its liveness compartment
/// in the same instruction as the private Product/Failure terminal composer.
/// Only recorded payer principal is refunded; all prefund/donation surplus is
/// transferred to the immutable Source route's neutral sink.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_source_terminal_liveness(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    execution: SourceTerminalExecutionV1,
    policy_account: &AccountInfo<'_>,
    compartment_account: &AccountInfo<'_>,
    payer_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
) -> Outcome<RuntimeAtomicTransitionV1> {
    require(
        policy_account.owner == program_id
            && !policy_account.is_writable
            && !policy_account.is_signer
            && !policy_account.executable
            && compartment_account.owner == program_id
            && compartment_account.is_writable
            && !compartment_account.is_signer
            && !compartment_account.executable
            && payer_refund.is_writable
            && !payer_refund.executable
            && neutral_sink.is_writable
            && !neutral_sink.executable,
        ClutchError::MismatchedState,
    )?;
    require(
        runtime_key(compartment_account.key) == route.source_compartment_account()
            && runtime_key(neutral_sink.key) == route.neutral_sink()
            && policy_account.key != compartment_account.key
            && policy_account.key != payer_refund.key
            && policy_account.key != neutral_sink.key
            && compartment_account.key != payer_refund.key
            && compartment_account.key != neutral_sink.key
            && payer_refund.key != neutral_sink.key,
        ClutchError::AccountAlias,
    )?;
    expect_pda(
        policy_account.key,
        seeds::source_liveness_policy_pda(program_id, &route.liveness_policy_id().bytes()),
        None,
    )?;
    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let compartment_data = compartment_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let transition = plan_runtime_transition_v1(
        LivenessId::from_bytes(program_id.to_bytes()),
        LivenessId::from_bytes(policy_account.key.to_bytes()),
        RuntimePersistedAccountViewV1 {
            account_id: LivenessId::from_bytes(policy_account.key.to_bytes()),
            owner_program_id: LivenessId::from_bytes(policy_account.owner.to_bytes()),
            lamports: policy_account.lamports(),
            data: &policy_data,
            writable: policy_account.is_writable,
        },
        RuntimePersistedAccountViewV1 {
            account_id: LivenessId::from_bytes(compartment_account.key.to_bytes()),
            owner_program_id: LivenessId::from_bytes(compartment_account.owner.to_bytes()),
            lamports: compartment_account.lamports(),
            data: &compartment_data,
            writable: compartment_account.is_writable,
        },
        execution.intent,
        Some(execution.observation),
        0,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(compartment_data);
    drop(policy_data);
    require(
        transition.close_account
            && !transition.write_account_data
            && transition.account_balance_before == compartment_account.lamports()
            && transition.account_balance_after == 0,
        ClutchError::MismatchedState,
    )?;
    let mut payer_lamports = 0_u64;
    let mut neutral_lamports = 0_u64;
    for movement in transition.transfers() {
        match movement.role {
            RuntimeTransferRoleV1::PayerTerminalRefund => {
                require(
                    movement.destination == LivenessId::from_bytes(payer_refund.key.to_bytes())
                        && payer_lamports == 0,
                    ClutchError::MismatchedState,
                )?;
                payer_lamports = movement.lamports;
            }
            RuntimeTransferRoleV1::NeutralTerminalSink => {
                require(
                    movement.destination == LivenessId::from_bytes(neutral_sink.key.to_bytes())
                        && neutral_lamports == 0,
                    ClutchError::MismatchedState,
                )?;
                neutral_lamports = movement.lamports;
            }
            _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
        }
    }
    require(
        transition.transfers().len()
                == usize::from(payer_lamports != 0) + usize::from(neutral_lamports != 0)
            && payer_lamports
                .checked_add(neutral_lamports)
                .ok_or(ClutchError::Arithmetic)?
                == compartment_account.lamports(),
        ClutchError::MismatchedState,
    )?;
    let payer_after = payer_refund
        .lamports()
        .checked_add(payer_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let neutral_after = neutral_sink
        .lamports()
        .checked_add(neutral_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    {
        let mut compartment_lamports = compartment_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut payer_balance = payer_refund
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut neutral_balance = neutral_sink
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **compartment_lamports = 0;
        **payer_balance = payer_after;
        **neutral_balance = neutral_after;
    }
    if payer_lamports != 0 {
        credit_source_funding_custody_ledger_from_close_v1(
            program_id,
            route,
            payer_refund,
            payer_lamports,
            execution.authenticated_receipt().id(),
        )?;
    }
    compartment_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    compartment_account.assign(&SYSTEM_PROGRAM_ID);
    Ok(transition)
}

/// Authenticate action 12's exact persisted terminal policy against its
/// currently-open lineage and project the sole admitted closed-lineage bytes.
/// The account codec, not an instruction discriminator, selects the exhaustive
/// no-reopen or reopen-request branch.
pub(crate) fn authenticate_source_terminal_policy_for_close(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    terminal_semantic_id: ContentId,
    policy_account: &AccountInfo<'_>,
) -> Outcome<ContentId> {
    let state = lineage.lineage();
    require(
        !terminal_semantic_id.is_zero()
            && lineage.access() == clutch_source_plane_v3_runtime::LineageAccessV1::Mutable
            && state.is_open,
        ClutchError::MismatchedState,
    )?;
    let expected_closed_lineage_state_id = if policy_account.data_len()
        == SOURCE_NO_REOPEN_TERMINAL_BYTES
    {
        let data = policy_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let body = SourceNoReopenTerminalV1::decode(&data).map_err(source_core)?;
        let body_id = body.id().map_err(source_runtime)?;
        let recipe = PdaRecipeV3::source_no_reopen_terminal(body_id).map_err(source_pda)?;
        let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
        let authenticated = authenticate_source_no_reopen_terminal(
            route,
            body,
            RuntimeAccountViewV1 {
                key: runtime_key(policy_account.key),
                owner: runtime_key(policy_account.owner),
                lamports: policy_account.lamports(),
                executable: policy_account.executable,
                writable: policy_account.is_writable,
                signer: policy_account.is_signer,
                data: &data,
            },
            derived,
            SourceNoReopenTerminalAccessV1::ExistingReadOnly,
        )
        .map_err(source_runtime)?;
        require(
            authenticated.terminal_id().map_err(source_runtime)? == terminal_semantic_id
                && body.lineage_authentication_id() == lineage.id()
                && body.expected_lineage_state_id() == lineage.account_data_id()
                && body.lineage_account() == state.lineage_account
                && body.target_account() == state.active_account
                && no_reopen_lineage_family(body.family()) == state.family,
            ClutchError::MismatchedState,
        )?;
        let projected = close_lineage_generation(
            state,
            state.active_account,
            state.latest_generation,
            state.last_opened_state_id,
            terminal_semantic_id,
        )
        .map_err(source_runtime)?;
        let bytes = projected.encode().map_err(source_runtime)?;
        account_data_id(state.lineage_account, &bytes).map_err(source_runtime)?
    } else if policy_account.data_len() == SOURCE_REOPEN_GENERATION_REQUEST_BYTES {
        let authenticated = authenticate_reopen_generation_request_before_close(
            route,
            policy_account,
            lineage,
            terminal_semantic_id,
        )
        .map_err(Refusal::from)?;
        require(
            authenticated.generation_policy_id() == terminal_semantic_id
                && no_reopen_lineage_family(authenticated.family()) == state.family
                && !authenticated.id().is_zero(),
            ClutchError::MismatchedState,
        )?;
        authenticated.projected_closed_lineage_state_id()
    } else {
        return Err(Refusal::Adapter(ClutchError::MismatchedState));
    };
    require(
        !expected_closed_lineage_state_id.is_zero(),
        ClutchError::MismatchedState,
    )?;
    Ok(expected_closed_lineage_state_id)
}

fn no_reopen_lineage_family(
    family: clutch_source_plane_v3_runtime::SourceReopenFamilyV1,
) -> LineageFamilyV1 {
    match family {
        clutch_source_plane_v3_runtime::SourceReopenFamilyV1::SourceHead => {
            LineageFamilyV1::SourceHead
        }
        clutch_source_plane_v3_runtime::SourceReopenFamilyV1::OpenRawPage => {
            LineageFamilyV1::OpenRawPage
        }
        clutch_source_plane_v3_runtime::SourceReopenFamilyV1::WindowWork => {
            LineageFamilyV1::WindowWork
        }
        clutch_source_plane_v3_runtime::SourceReopenFamilyV1::StatisticResult => {
            LineageFamilyV1::StatisticResult
        }
    }
}

/// Complete action-4 parser, Source mutation, receipt, and liveness result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicBoundaryIngestExecutionV1 {
    /// Exact parser-authenticated boundary consumed by the mutation.
    pub boundary: AuthenticatedBoundaryV1,
    /// Exact open-page mutation receipt.
    pub ingest: PersistedIngestBoundaryBatchV1,
    /// Persisted paid-work receipt and intent.
    pub work: SourceWorkExecutionV1,
    /// Applied Source-compartment debit/refund transition.
    pub liveness: RuntimeAtomicTransitionV1,
}

/// Execute one real parser CPI and atomically append its authenticated output,
/// persist the paid-work receipt, and debit Source liveness custody.
#[allow(clippy::too_many_arguments)]
pub fn ingest_parser_boundary_atomic(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    receiver: AuthenticatedReceiverRouteV2,
    clock: AuthenticatedClockBucketV1,
    head: AuthenticatedSourceHeadV1,
    open: AuthenticatedOpenRawPageV1,
    open_lineage: AuthenticatedReopenLineageV1,
    feed: &AccountInfo<'_>,
    parser_instruction: &Instruction,
    parser_accounts: &[AccountInfo<'_>],
    open_account: &AccountInfo<'_>,
    open_lineage_account: &AccountInfo<'_>,
    schedule: SourceWorkScheduleBindingV1,
    receipt_account: &AccountInfo<'_>,
    call_ordinal: u32,
    call_ceiling_lamports: u64,
    keeper: &AccountInfo<'_>,
    keeper_payment_lamports: u64,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    payer_refund: &AccountInfo<'_>,
    policy_account: &AccountInfo<'_>,
    compartment_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<AtomicBoundaryIngestExecutionV1> {
    let open_state = open.open();
    let expected_bucket = open_state
        .start_bucket
        .checked_add(u64::from(open_state.record_count))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let boundary = invoke_parser_boundary(
        route,
        receiver,
        clock,
        feed,
        expected_bucket,
        open_state.repair_generation,
        parser_instruction,
        parser_accounts,
    )
    .map_err(Refusal::from)?;
    let batch = BoundaryBatchV1::new(&[boundary]).map_err(source_runtime)?;
    let ingest = ingest_boundary_batch(
        route,
        head,
        open,
        &batch,
        open_account,
        open_lineage_account,
        open_lineage,
    )?;
    let work = bind_work_execution(
        program_id,
        route,
        schedule,
        SourceWorkKindV1::AppendBoundaryBatch,
        ingest.semantic.transition_receipt_id,
        receipt_account,
        call_ordinal,
        call_ceiling_lamports,
        keeper.key,
        keeper_payment_lamports,
        custody,
        custody_account,
        system_program,
        rent_sysvar,
    )?;
    let liveness = apply_source_work_liveness(
        program_id,
        route,
        work,
        policy_account,
        compartment_account,
        keeper,
        payer_refund,
    )?;
    Ok(AtomicBoundaryIngestExecutionV1 {
        boundary,
        ingest,
        work,
        liveness,
    })
}

/// Complete action-9 evaluator, persistence, receipt, and liveness result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicEvaluationExecutionV1 {
    /// Release-selected evaluator output authenticated after CPI.
    pub evaluation: AuthenticatedEvaluationV1,
    /// Persisted StatisticResult and bootstrapped lineage.
    pub result: OpenRuntimeAccountResultV1,
    /// Persisted paid-work receipt and intent.
    pub work: SourceWorkExecutionV1,
    /// Applied Source-compartment debit/refund transition.
    pub liveness: RuntimeAtomicTransitionV1,
}

/// Execute the release-selected evaluator CPI and atomically persist its
/// result, work receipt, and Source liveness debit.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_statistic_atomic(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    binding: EvaluationReleaseBindingV1,
    summary: SummaryProgramV3,
    evaluator_program: &AccountInfo<'_>,
    evaluator_programdata: &AccountInfo<'_>,
    clock: ClockSnapshotV1,
    window: &WindowSpecV3,
    key: &StatisticKeyV3,
    evidence: AuthenticatedWindowEvidenceV1,
    evaluator_instruction: &Instruction,
    evaluator_accounts: &[AccountInfo<'_>],
    result_account: &AccountInfo<'_>,
    result_lineage_account: &AccountInfo<'_>,
    schedule: SourceWorkScheduleBindingV1,
    receipt_account: &AccountInfo<'_>,
    call_ordinal: u32,
    call_ceiling_lamports: u64,
    keeper: &AccountInfo<'_>,
    keeper_payment_lamports: u64,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    payer_refund: &AccountInfo<'_>,
    policy_account: &AccountInfo<'_>,
    compartment_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<AtomicEvaluationExecutionV1> {
    let evaluation = invoke_statistic_evaluator(
        route,
        binding,
        summary,
        evaluator_program,
        evaluator_programdata,
        clock,
        window,
        key,
        evidence,
        evaluator_instruction,
        evaluator_accounts,
    )
    .map_err(Refusal::from)?;
    let result = persist_evaluation_result(
        program_id,
        route,
        key,
        evidence,
        evaluation,
        custody,
        custody_account,
        result_account,
        result_lineage_account,
        system_program,
        rent_sysvar,
    )?;
    let work = bind_work_execution(
        program_id,
        route,
        schedule,
        SourceWorkKindV1::EvaluateStatistic,
        result.account_data_id,
        receipt_account,
        call_ordinal,
        call_ceiling_lamports,
        keeper.key,
        keeper_payment_lamports,
        custody,
        custody_account,
        system_program,
        rent_sysvar,
    )?;
    let liveness = apply_source_work_liveness(
        program_id,
        route,
        work,
        policy_account,
        compartment_account,
        keeper,
        payer_refund,
    )?;
    Ok(AtomicEvaluationExecutionV1 {
        evaluation,
        result,
        work,
        liveness,
    })
}

/// Atomic raw-page seal outputs across the immutable page, head CAS, and
/// consumed open-page generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealRawPageExecutionV1 {
    /// Semantic page/head transition.
    pub semantic: clutch_source_plane_v3_runtime::SealOpenPageOutputV1,
    /// Exact immutable RawPage rent postimage.
    pub page_funding: ImmutableAccountFundingV1,
    /// Exact immutable RawPage envelope persisted by this instruction.
    pub page_header: RuntimeAccountHeaderV1,
    /// Digest of the complete immutable RawPage account postimage.
    pub page_account_data_id: ContentId,
    /// SourceHead postimage and lineage CAS.
    pub head: MutateRuntimeAccountResultV1,
    /// OpenRawPage close/refund/sink postimage.
    pub open_close: CloseRuntimeAccountResultV1,
}

/// Atomic Window seal outputs across immutable evidence and consumed work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealWindowExecutionV1 {
    /// Exact authenticated semantic Window evidence.
    pub evidence: AuthenticatedWindowEvidenceV1,
    /// Immutable WindowSeal rent postimage.
    pub seal_funding: ImmutableAccountFundingV1,
    /// Exact immutable WindowSeal envelope persisted by this instruction.
    pub seal_header: RuntimeAccountHeaderV1,
    /// Digest of the complete immutable WindowSeal account postimage.
    pub seal_account_data_id: ContentId,
    /// WindowWork close/refund/sink postimage.
    pub work_close: CloseRuntimeAccountResultV1,
}

/// Bind one semantic transition to a predictable persisted work receipt and
/// the sole liveness Source-compartment spend.
#[allow(clippy::too_many_arguments)]
pub fn bind_work_execution(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    kind: SourceWorkKindV1,
    semantic_receipt_id: ContentId,
    receipt_account: &AccountInfo<'_>,
    call_ordinal: u32,
    call_ceiling_lamports: u64,
    keeper: &Pubkey,
    keeper_payment_lamports: u64,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<SourceWorkExecutionV1> {
    let slot_id = SourceWorkAuthorizationV1::receipt_slot_id(
        route,
        schedule,
        kind,
        call_ordinal,
        semantic_receipt_id,
    )
    .map_err(source_runtime)?;
    let recipe = PdaRecipeV3::source_work_receipt(slot_id).map_err(source_pda)?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    require(
        runtime_key(receipt_account.key) == derived.address,
        ClutchError::WrongPda,
    )?;
    let authorization = SourceWorkAuthorizationV1::new(
        route,
        schedule,
        kind,
        runtime_key(receipt_account.key),
        call_ordinal,
        call_ceiling_lamports,
        semantic_receipt_id,
    )
    .map_err(source_runtime)?;
    let receipt =
        SourceWorkReceiptAccountV1::from_work(route, authorization).map_err(source_runtime)?;

    // The account is authenticated from the exact postimage below. It is
    // deliberately writable only during this atomic creation instruction.
    let (receipt_funding, authenticated) = create_work_receipt_account(
        program_id,
        route,
        schedule,
        receipt,
        custody,
        custody_account,
        receipt_account,
        system_program,
        rent_sysvar,
    )?;
    let observation = project_liveness_receipt(authenticated);
    let intent = project_liveness_work_intent(authenticated, keeper, keeper_payment_lamports)
        .map_err(Refusal::from)?;
    Ok(SourceWorkExecutionV1 {
        receipt,
        receipt_funding,
        observation,
        intent,
        authenticated_receipt: authenticated,
    })
}

/// Bind one authenticated persisted Source terminal fact to its predictable
/// receipt and the sole liveness close-success intent.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bind_terminal_execution(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    terminal_semantic: AuthenticatedSourceTerminalSemanticV1,
    receipt_account: &AccountInfo<'_>,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<SourceTerminalExecutionV1> {
    let semantic_terminal_receipt_id = terminal_semantic.semantic_id();
    let slot_id = SourceTerminalAuthorizationV1::receipt_slot_id(
        route,
        schedule,
        SourceTerminalOutcomeV1::Success,
        semantic_terminal_receipt_id,
    )
    .map_err(source_runtime)?;
    let recipe = PdaRecipeV3::source_work_receipt(slot_id).map_err(source_pda)?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    require(
        runtime_key(receipt_account.key) == derived.address,
        ClutchError::WrongPda,
    )?;
    let authorization = SourceTerminalAuthorizationV1::new(
        route,
        schedule,
        SourceTerminalOutcomeV1::Success,
        runtime_key(receipt_account.key),
        semantic_terminal_receipt_id,
    )
    .map_err(source_runtime)?;
    let receipt =
        SourceWorkReceiptAccountV1::from_terminal(route, authorization).map_err(source_runtime)?;
    let (receipt_funding, authenticated) = create_work_receipt_account(
        program_id,
        route,
        schedule,
        receipt,
        custody,
        custody_account,
        receipt_account,
        system_program,
        rent_sysvar,
    )?;
    Ok(SourceTerminalExecutionV1 {
        receipt,
        receipt_funding,
        observation: project_liveness_receipt(authenticated),
        intent: project_liveness_terminal_intent(authenticated).map_err(Refusal::from)?,
        authenticated_receipt: authenticated,
    })
}

/// Register action 1: persist the exact content-addressed reviewed release.
///
/// Release evidence is permanent registry state. Its creator sponsors only the
/// observed rent shortfall; a prefund never becomes creator principal or
/// authority, and this account deliberately has no close/refund action.
#[allow(clippy::too_many_arguments)]
pub fn register_release(
    program_id: &Pubkey,
    manifest: &SourceReleaseManifestV2,
    payer: &AccountInfo<'_>,
    release_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<ImmutableAccountFundingV1> {
    require_creation_roles(program_id, payer, release_account, system_program)?;
    let recipe =
        PdaRecipeV3::source_release(manifest.id().map_err(source_runtime)?).map_err(source_pda)?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    require(
        derived.address == runtime_key(release_account.key),
        ClutchError::WrongPda,
    )?;
    let bytes = manifest.encode().map_err(source_runtime)?;
    let rent = read_rent(rent_sysvar)?;
    let minimum = rent.minimum_balance(bytes.len())?;
    let before = release_account.lamports();
    let debit = minimum.saturating_sub(before);
    create_with_recipe(
        program_id,
        payer,
        release_account,
        system_program,
        &rent,
        bytes.len(),
        &recipe,
        derived.bump,
    )?;
    write_exact_account_data(release_account, &bytes)?;
    Ok(ImmutableAccountFundingV1 {
        account: runtime_key(release_account.key),
        payer: if debit == 0 {
            RuntimeKey::ZERO
        } else {
            runtime_key(payer.key)
        },
        payer_debit_lamports: debit,
        donation_lamports: before,
        rent_sysvar_id: rent_sysvar_id(rent_sysvar)?,
        rent_exempt_minimum_lamports: minimum,
        account_balance_after: before
            .checked_add(debit)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
    })
}

/// Register action 1 from the sealed content-addressed artifact transport.
/// The 1,296-byte successor manifest never appears in instruction data. Repeating the
/// same registration converges on the exact existing 0x8a account; any byte,
/// owner, or PDA mismatch refuses.
#[allow(clippy::too_many_arguments)]
pub fn register_release_from_artifact(
    program_id: &Pubkey,
    expected_manifest_id: ContentId,
    artifact_account: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    release_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<ImmutableAccountFundingV1> {
    require(!expected_manifest_id.is_zero(), ClutchError::MismatchedState)?;
    require(
        artifact_account.owner == program_id
            && !artifact_account.is_signer
            && !artifact_account.is_writable
            && !artifact_account.executable
            && artifact_account.data_len()
                == clutch_source_plane_v3_runtime::SOURCE_RELEASE_MANIFEST_BYTES,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        artifact_account.key,
        seeds::product_artifact_pda(
            program_id,
            ArtifactKind::SourceReleaseManifestV2.byte(),
            &expected_manifest_id.bytes(),
        ),
        None,
    )?;
    let artifact_data = artifact_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let manifest = SourceReleaseManifestV2::decode(&artifact_data).map_err(source_runtime)?;
    require(
        manifest.id().map_err(source_runtime)? == expected_manifest_id,
        ClutchError::MismatchedState,
    )?;
    let expected_bytes = manifest.encode().map_err(source_runtime)?;
    drop(artifact_data);

    if release_account.owner == program_id && release_account.data_len() == expected_bytes.len() {
        require(
            release_account.is_writable
                && !release_account.is_signer
                && !release_account.executable,
            ClutchError::MismatchedState,
        )?;
        let recipe = PdaRecipeV3::source_release(expected_manifest_id).map_err(source_pda)?;
        let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
        require(
            derived.address == runtime_key(release_account.key),
            ClutchError::WrongPda,
        )?;
        let release_data = release_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(
            &*release_data == expected_bytes.as_slice(),
            ClutchError::MismatchedState,
        )?;
        let rent = read_rent(rent_sysvar)?;
        let minimum = rent.minimum_balance(expected_bytes.len())?;
        require(
            release_account.lamports() >= minimum,
            ClutchError::MismatchedState,
        )?;
        return Ok(ImmutableAccountFundingV1 {
            account: runtime_key(release_account.key),
            payer: RuntimeKey::ZERO,
            payer_debit_lamports: 0,
            donation_lamports: release_account.lamports(),
            rent_sysvar_id: rent_sysvar_id(rent_sysvar)?,
            rent_exempt_minimum_lamports: minimum,
            account_balance_after: release_account.lamports(),
        });
    }
    register_release(
        program_id,
        &manifest,
        payer,
        release_account,
        system_program,
        rent_sysvar,
    )
}

/// Authenticate the sole content-addressed paid-work schedule selected by the
/// immutable Source release. Callers supply only the physical account; they
/// cannot substitute a different schedule body with the same instruction.
pub fn authenticate_source_work_schedule_artifact(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule_account: &AccountInfo<'_>,
) -> Outcome<SourceWorkScheduleBindingV1> {
    require(
        schedule_account.owner == program_id
            && !schedule_account.is_signer
            && !schedule_account.is_writable
            && !schedule_account.executable
            && schedule_account.data_len()
                == clutch_source_plane_v3_runtime::SOURCE_WORK_SCHEDULE_BYTES,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        schedule_account.key,
        seeds::product_artifact_pda(
            program_id,
            ArtifactKind::SourceWorkScheduleV1.byte(),
            &route.source_work_schedule_id().bytes(),
        ),
        None,
    )?;
    let data = schedule_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let schedule = SourceWorkScheduleBindingV1::decode(&data).map_err(source_runtime)?;
    require(
        schedule.id().map_err(source_runtime)? == route.source_work_schedule_id()
            && schedule.liveness_policy_id() == route.liveness_policy_id()
            && schedule.source_compartment_account() == route.source_compartment_account()
            && schedule.source_compartment_owner() == route.source_compartment_owner()
            && schedule.receipt_account_owner_program() == route.adapter_program(),
        ClutchError::MismatchedState,
    )?;
    Ok(schedule)
}

/// Initialize action 2: atomically bootstrap the never-created release-bound
/// lineage and persist the exact first SourceHead generation.
#[allow(clippy::too_many_arguments)]
pub fn initialize_head(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    authorization: AuthenticatedSourceGenerationV1,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<OpenRuntimeAccountResultV1> {
    let body = initialize_source_head(route, authorization).map_err(source_runtime)?;
    let recipe = PdaRecipeV3::source_head(
        route.source_plane_contract_id(),
        route.source_spec_id(),
        body.repair_generation,
    )
    .map_err(source_pda)?;
    let semantic_binding_id = recipe.id().map_err(source_pda)?;
    bootstrap_runtime_account(
        program_id,
        route,
        LineageFamilyV1::SourceHead,
        semantic_binding_id,
        &recipe,
        &body,
        custody,
        custody_account,
        target,
        lineage_account,
        system_program,
        rent_sysvar,
    )
}

/// Open action 3: create the state-assigned page derived only from SourceHead.
#[allow(clippy::too_many_arguments)]
pub fn open_raw_page(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    head: AuthenticatedSourceHeadV1,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<OpenRuntimeAccountResultV1> {
    let body = head.head().open_page().map_err(source_core)?;
    let recipe = PdaRecipeV3::open_raw_page(
        route.source_plane_contract_id(),
        route.source_spec_id(),
        body.repair_generation,
        body.page_index,
    )
    .map_err(source_pda)?;
    let semantic_binding_id = recipe.id().map_err(source_pda)?;
    bootstrap_runtime_account(
        program_id,
        route,
        LineageFamilyV1::OpenRawPage,
        semantic_binding_id,
        &recipe,
        &body,
        custody,
        custody_account,
        target,
        lineage_account,
        system_program,
        rent_sysvar,
    )
}

/// Ingest action 4: append an already-authenticated bounded batch and persist
/// the open-page compare-and-swap. Sealing is action 5 and is refused here.
pub fn ingest_boundary_batch(
    route: AuthenticatedSourceRouteV1,
    head: AuthenticatedSourceHeadV1,
    open: AuthenticatedOpenRawPageV1,
    batch: &BoundaryBatchV1,
    open_account: &AccountInfo<'_>,
    open_lineage_account: &AccountInfo<'_>,
    open_lineage: AuthenticatedReopenLineageV1,
) -> Outcome<PersistedIngestBoundaryBatchV1> {
    let output = clutch_source_plane_v3_runtime::ingest_boundary_batch(
        route,
        head,
        open,
        batch,
        SealBatchModeV1::KeepOpen,
    )
    .map_err(source_runtime)?;
    let open_after = output
        .open_after
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let mutation = mutate_runtime_account(
        route,
        open_lineage,
        open_account,
        open_lineage_account,
        &open_after,
    )?;
    Ok(PersistedIngestBoundaryBatchV1 {
        semantic: output,
        mutation,
    })
}

/// Initialize action 6: create one predictable WindowWork generation.
#[allow(clippy::too_many_arguments)]
pub fn initialize_window_work(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    window: &WindowSpecV3,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<OpenRuntimeAccountResultV1> {
    let body = WindowWorkV3::new(window).map_err(source_core)?;
    let window_id = window.id().map_err(source_core)?;
    let recipe = PdaRecipeV3::window_work(window_id).map_err(source_pda)?;
    bootstrap_runtime_account(
        program_id,
        route,
        LineageFamilyV1::WindowWork,
        window_id,
        &recipe,
        &body,
        custody,
        custody_account,
        target,
        lineage_account,
        system_program,
        rent_sysvar,
    )
}

/// Fold action 7: persist the exact bounded WindowWork postimage and lineage CAS.
pub fn fold_window_pages(
    route: AuthenticatedSourceRouteV1,
    window: &WindowSpecV3,
    work: AuthenticatedWindowWorkV1,
    pages: &[clutch_source_plane_v3_runtime::AuthenticatedRawPageV1],
    work_account: &AccountInfo<'_>,
    work_lineage_account: &AccountInfo<'_>,
    work_lineage: AuthenticatedReopenLineageV1,
) -> Outcome<PersistedFoldWindowPagesV1> {
    let output =
        clutch_source_plane_v3_runtime::fold_authenticated_pages(route, window, work, pages)
            .map_err(source_runtime)?;
    let mutation = mutate_runtime_account(
        route,
        work_lineage,
        work_account,
        work_lineage_account,
        &output.work_after,
    )?;
    Ok(PersistedFoldWindowPagesV1 {
        semantic: output,
        mutation,
    })
}

/// Seal action 5: create the immutable RawPage, advance SourceHead, then close
/// the consumed open generation. Every semantic check and funding split is
/// computed before its corresponding write; any later refusal rolls the whole
/// Solana instruction back.
#[allow(clippy::too_many_arguments)]
pub fn seal_raw_page(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    head: AuthenticatedSourceHeadV1,
    open: AuthenticatedOpenRawPageV1,
    head_lineage: AuthenticatedReopenLineageV1,
    open_lineage: AuthenticatedReopenLineageV1,
    head_account: &AccountInfo<'_>,
    open_account: &AccountInfo<'_>,
    head_lineage_account: &AccountInfo<'_>,
    open_lineage_account: &AccountInfo<'_>,
    page_account: &AccountInfo<'_>,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    open_principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<SealRawPageExecutionV1> {
    let semantic = clutch_source_plane_v3_runtime::seal_authenticated_open_page(route, head, open)
        .map_err(source_runtime)?;
    let page_recipe = PdaRecipeV3::raw_page(
        route.source_plane_contract_id(),
        semantic.sealed_page.id().map_err(source_core)?,
    )
    .map_err(source_pda)?;
    let (page_funding, page_header, page_account_data_id) = create_immutable_runtime_account(
        program_id,
        route,
        &page_recipe,
        &semantic.sealed_page,
        custody,
        custody_account,
        page_account,
        system_program,
        rent_sysvar,
    )?;
    let head_postimage = mutate_runtime_account(
        route,
        head_lineage,
        head_account,
        head_lineage_account,
        &semantic.head_after,
    )?;
    let open_close = close_runtime_account::<OpenRawPageV3>(
        program_id,
        route,
        open_lineage,
        open_account,
        open_lineage_account,
        open_principal_refund,
        neutral_sink,
        semantic.transition_receipt_id,
    )?;
    Ok(SealRawPageExecutionV1 {
        semantic,
        page_funding,
        page_header,
        page_account_data_id,
        head: head_postimage,
        open_close,
    })
}

/// Seal action 8: freeze exact Window evidence and close its consumed work
/// generation with the persisted payer/donation split.
#[allow(clippy::too_many_arguments)]
pub fn seal_window(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    source_plane: &SourcePlaneProgramV3,
    clock_policy: &ClockPolicyV1,
    clock: ClockSnapshotV1,
    window: &WindowSpecV3,
    work: AuthenticatedWindowWorkV1,
    maturity_page: AuthenticatedRawPageV1,
    work_lineage: AuthenticatedReopenLineageV1,
    work_account: &AccountInfo<'_>,
    work_lineage_account: &AccountInfo<'_>,
    seal_account: &AccountInfo<'_>,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    work_principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<SealWindowExecutionV1> {
    let evidence = clutch_source_plane_v3_runtime::seal_authenticated_window(
        route,
        source_plane,
        clock_policy,
        clock,
        window,
        work,
        maturity_page,
    )
    .map_err(source_runtime)?;
    let recipe = PdaRecipeV3::window_seal(window.id().map_err(source_core)?).map_err(source_pda)?;
    let (seal_funding, seal_header, seal_account_data_id) = create_immutable_runtime_account(
        program_id,
        route,
        &recipe,
        &evidence.seal(),
        custody,
        custody_account,
        seal_account,
        system_program,
        rent_sysvar,
    )?;
    let work_close = close_runtime_account::<WindowWorkV3>(
        program_id,
        route,
        work_lineage,
        work_account,
        work_lineage_account,
        work_principal_refund,
        neutral_sink,
        evidence.id(),
    )?;
    Ok(SealWindowExecutionV1 {
        evidence,
        seal_funding,
        seal_header,
        seal_account_data_id,
        work_close,
    })
}

/// Evaluate action 9 persistence half: consume only a release-authenticated
/// evaluator result and open its exact StatisticKey generation. The CPI-facing
/// adapter constructs `evaluation`; this function owns the durable postimage.
#[allow(clippy::too_many_arguments)]
pub fn persist_evaluation_result(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    key: &StatisticKeyV3,
    evidence: AuthenticatedWindowEvidenceV1,
    evaluation: AuthenticatedEvaluationV1,
    lineage: AuthenticatedReopenLineageV1,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    result_account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<OpenRuntimeAccountResultV1> {
    let key_id = key.id().map_err(source_core)?;
    require(
        evaluation.statistic_key_id() == key_id && evaluation.window_evidence_id() == evidence.id(),
        ClutchError::MismatchedState,
    )?;
    let recipe = PdaRecipeV3::statistic_result(key_id).map_err(source_pda)?;
    reopen_runtime_account(
        program_id,
        route,
        lineage,
        LineageFamilyV1::StatisticResult,
        key_id,
        &recipe,
        &evaluation.result(),
        custody,
        custody_account,
        result_account,
        lineage_account,
        system_program,
        rent_sysvar,
    )
}

/// Preallocate the exact never-opened StatisticResult lineage in the same
/// Product-owned transaction that publishes its StatisticKey. Mature absence
/// can therefore be authenticated without trusting a caller-created tombstone,
/// while action 9 later opens generation one through the ordinary reopen CAS.
#[allow(clippy::too_many_arguments)]
pub(crate) fn preallocate_statistic_result_lineage_v1(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    key: &StatisticKeyV3,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<PreallocatedStatisticResultLineageV1> {
    require_custody_creation_roles(custody, custody_account, lineage_account, system_program)?;
    let key_id = key.id().map_err(source_core)?;
    let recipe_id = ReopenLineageV1::recipe_id_for(
        route.adapter_program(),
        route.release_manifest_id(),
        route.route_id(),
        LineageFamilyV1::StatisticResult,
        key_id,
        route.source_work_schedule_id(),
    )
    .map_err(source_runtime)?;
    let recipe = PdaRecipeV3::reopen_lineage(recipe_id).map_err(source_pda)?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    require(
        derived.address == runtime_key(lineage_account.key),
        ClutchError::WrongPda,
    )?;
    let lineage = ReopenLineageV1::new(
        route.adapter_program(),
        route.release_manifest_id(),
        route.route_id(),
        key_id,
        runtime_key(lineage_account.key),
        LineageFamilyV1::StatisticResult,
        route.source_work_schedule_id(),
        route.neutral_sink(),
    )
    .map_err(source_runtime)?;
    let bytes = lineage.encode().map_err(source_runtime)?;
    let rent = read_rent(rent_sysvar)?;
    let rent_id = rent_sysvar_id(rent_sysvar)?;
    let minimum = rent.minimum_balance(REOPEN_LINEAGE_BYTES)?;
    let before = lineage_account.lamports();
    let debit = minimum.saturating_sub(before);
    create_with_recipe_from_custody(
        program_id,
        custody,
        custody_account,
        lineage_account,
        system_program,
        &rent,
        REOPEN_LINEAGE_BYTES,
        &recipe,
        derived.bump,
    )?;
    write_exact_account_data(lineage_account, &bytes)?;
    let data = lineage_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let authenticated = clutch_source_plane_v3_runtime::authenticate_reopen_lineage_account(
        route,
        runtime_account_view(lineage_account, &data),
        derived,
        LineageAccessV1::Mutable,
    )
    .map_err(source_runtime)?;
    drop(data);
    let funding = ImmutableAccountFundingV1 {
        account: runtime_key(lineage_account.key),
        payer: if debit == 0 { RuntimeKey::ZERO } else { custody.account() },
        payer_debit_lamports: debit,
        donation_lamports: before,
        rent_sysvar_id: rent_id,
        rent_exempt_minimum_lamports: minimum,
        account_balance_after: lineage_account.lamports(),
    };
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/sbf/preallocated-statistic-result-lineage/v1",
            &route.route_id().bytes(),
            &key_id.bytes(),
            &authenticated.id().bytes(),
            &authenticated.account_data_id().bytes(),
            &custody.id().bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(PreallocatedStatisticResultLineageV1 {
        funding,
        authenticated,
        id,
    })
}

#[allow(clippy::too_many_arguments)]
fn publish_immutable_source_input<T: FixedCodec>(
    program_id: &Pubkey,
    body: &T,
    semantic_id: ContentId,
    recipe: &PdaRecipeV3,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent: &RentParameters,
) -> Outcome<ImmutableSourceInputFundingV1> {
    require(!semantic_id.is_zero(), ClutchError::MismatchedState)?;
    require_system_program(system_program)?;
    require(
        target.is_writable
            && !target.is_signer
            && !target.executable
            && custody.account() == runtime_key(custody_account.key)
            && custody_account.key != target.key,
        ClutchError::MismatchedState,
    )?;
    let derived = derive_runtime_pda(program_id, recipe).map_err(Refusal::from)?;
    require(
        derived.address == runtime_key(target.key),
        ClutchError::WrongPda,
    )?;
    let mut bytes = vec![0_u8; T::ENCODED_LEN];
    body.encode_into(&mut bytes).map_err(source_core)?;
    let minimum = rent.minimum_balance(bytes.len())?;
    if target.owner == program_id {
        require(
            target.data_len() == bytes.len() && target.lamports() >= minimum,
            ClutchError::MismatchedState,
        )?;
        let data = target
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(&*data == bytes.as_slice(), ClutchError::MismatchedState)?;
        let data_id = account_data_id(runtime_key(target.key), &data).map_err(source_runtime)?;
        return Ok(ImmutableSourceInputFundingV1 {
            account: runtime_key(target.key),
            payer: RuntimeKey::ZERO,
            payer_debit_lamports: 0,
            permanent_prefund_lamports: target.lamports(),
            account_data_id: data_id,
            semantic_id,
        });
    }
    require_custody_creation_roles(custody, custody_account, target, system_program)?;
    let before = target.lamports();
    let debit = minimum.saturating_sub(before);
    create_with_recipe_from_custody(
        program_id,
        custody,
        custody_account,
        target,
        system_program,
        rent,
        bytes.len(),
        recipe,
        derived.bump,
    )?;
    write_exact_account_data(target, &bytes)?;
    let data_id = account_data_id(runtime_key(target.key), &bytes).map_err(source_runtime)?;
    Ok(ImmutableSourceInputFundingV1 {
        account: runtime_key(target.key),
        payer: if debit == 0 {
            RuntimeKey::ZERO
        } else {
            custody.account()
        },
        payer_debit_lamports: debit,
        permanent_prefund_lamports: before,
        account_data_id: data_id,
        semantic_id,
    })
}

/// Publish the exact three immutable Source semantic accounts selected by one
/// private current Product/Profile/Bundle authorization.
///
/// This is crate-private: no instruction decoder can supply the authorization
/// digest or semantic bodies directly. The Product authority module must mint
/// and consume its private capability in the same call.
#[allow(clippy::too_many_arguments)]
pub(crate) fn publish_authenticated_source_semantic_inputs(
    program_id: &Pubkey,
    publication_authorization_id: ContentId,
    window: WindowSpecV3,
    summary: SummaryProgramV3,
    statistic_key: StatisticKeyV3,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    window_account: &AccountInfo<'_>,
    summary_account: &AccountInfo<'_>,
    statistic_key_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<PublishedSourceSemanticInputsV1> {
    require(
        !publication_authorization_id.is_zero(),
        ClutchError::MismatchedState,
    )?;
    window.validate().map_err(source_core)?;
    summary.validate().map_err(source_core)?;
    statistic_key.validate().map_err(source_core)?;
    let window_id = window.id().map_err(source_core)?;
    let summary_id = summary.id().map_err(source_core)?;
    let statistic_key_id = statistic_key.id().map_err(source_core)?;
    require(
        statistic_key.window_id == window_id
            && statistic_key.summary_program_id == summary_id
            && summary.supports(statistic_key.statistic)
            && window_account.key != summary_account.key
            && window_account.key != statistic_key_account.key
            && summary_account.key != statistic_key_account.key,
        ClutchError::MismatchedState,
    )?;
    let rent = read_rent(rent_sysvar)?;
    let window_recipe = PdaRecipeV3::window_spec(window_id).map_err(source_pda)?;
    let summary_recipe = PdaRecipeV3::summary_program(summary_id).map_err(source_pda)?;
    let statistic_key_recipe =
        PdaRecipeV3::statistic_key(statistic_key_id).map_err(source_pda)?;
    let window_funding = publish_immutable_source_input(
        program_id,
        &window,
        window_id,
        &window_recipe,
        custody,
        custody_account,
        window_account,
        system_program,
        &rent,
    )?;
    let summary_funding = publish_immutable_source_input(
        program_id,
        &summary,
        summary_id,
        &summary_recipe,
        custody,
        custody_account,
        summary_account,
        system_program,
        &rent,
    )?;
    let statistic_key_funding = publish_immutable_source_input(
        program_id,
        &statistic_key,
        statistic_key_id,
        &statistic_key_recipe,
        custody,
        custody_account,
        statistic_key_account,
        system_program,
        &rent,
    )?;
    let receipt_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/published-source-semantic-inputs/v1",
            &publication_authorization_id.bytes(),
            &window_funding.account.bytes(),
            &window_funding.account_data_id.bytes(),
            &summary_funding.account.bytes(),
            &summary_funding.account_data_id.bytes(),
            &statistic_key_funding.account.bytes(),
            &statistic_key_funding.account_data_id.bytes(),
        ])
        .to_bytes(),
    );
    require(receipt_id != ContentId::ZERO, ClutchError::MismatchedState)?;
    Ok(PublishedSourceSemanticInputsV1 {
        publication_authorization_id,
        window: window_funding,
        summary: summary_funding,
        statistic_key: statistic_key_funding,
        receipt_id,
    })
}

/// Persist Product's exact compiled occurrence under its content-addressed
/// PDA. Construction is private to the current Product/Profile/Bundle owner;
/// the public Source dispatcher cannot supply an occurrence body.
#[allow(clippy::too_many_arguments)]
pub(crate) fn publish_source_occurrence_v1(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    publication_authorization_id: ContentId,
    occurrence: CompiledSourceOccurrenceV3,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    occurrence_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<PublishedSourceOccurrenceV1> {
    require(
        !publication_authorization_id.is_zero()
            && route.generation_authority_program() == runtime_key(program_id)
            && route.generation_authority_program() == route.adapter_program(),
        ClutchError::AuthorizationUnavailable,
    )?;
    let occurrence_id = ContentId::from_bytes(
        occurrence
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
    );
    let (expected, bump) = seeds::source_occurrence_pda(program_id, &occurrence_id.bytes());
    require(occurrence_account.key == &expected, ClutchError::WrongPda)?;
    require_custody_creation_roles(custody, custody_account, occurrence_account, system_program)?;
    let mut bytes = [0_u8; clutch_product_series::SOURCE_OCCURRENCE_RECORD_BYTES];
    occurrence
        .encode_into(&mut bytes)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rent = read_rent(rent_sysvar)?;
    let minimum = rent.minimum_balance(bytes.len())?;
    let before = occurrence_account.lamports();
    let debit = minimum.saturating_sub(before);
    let bump_seed = [bump];
    create_with_raw_seeds_from_custody(
        program_id,
        custody,
        custody_account,
        occurrence_account,
        system_program,
        &rent,
        bytes.len(),
        &[
            seeds::SEED_SOURCE_OCCURRENCE_V1,
            &occurrence_id.bytes(),
            &bump_seed,
        ],
    )?;
    write_exact_account_data(occurrence_account, &bytes)?;
    let data_id = account_data_id(runtime_key(occurrence_account.key), &bytes)
        .map_err(source_runtime)?;
    let funding = ImmutableSourceInputFundingV1 {
        account: runtime_key(occurrence_account.key),
        payer: if debit == 0 { RuntimeKey::ZERO } else { custody.account() },
        payer_debit_lamports: debit,
        permanent_prefund_lamports: before,
        account_data_id: data_id,
        semantic_id: occurrence_id,
    };
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/sbf/published-source-occurrence/v1",
            &publication_authorization_id.bytes(),
            &route.route_id().bytes(),
            &occurrence_account.key.to_bytes(),
            &occurrence_id.bytes(),
            &data_id.bytes(),
            &custody.id().bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(PublishedSourceOccurrenceV1 {
        occurrence,
        funding,
        id,
    })
}

/// Persist action 10's exact Source-only policy handoff as an immutable
/// content-addressed account and re-authenticate its postimage before return.
#[allow(clippy::too_many_arguments)]
pub fn persist_source_policy_handoff(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    join: SourcePolicyHandoffJoinV1,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    handoff_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<PersistedSourcePolicyHandoffV1> {
    let body = SourcePolicyHandoffAccountV1::from_join(join).map_err(source_runtime)?;
    let recipe = PdaRecipeV3::source_policy_handoff(join.id()).map_err(source_pda)?;
    let rent = read_rent(rent_sysvar)?;
    let funding = publish_immutable_source_input(
        program_id,
        &body,
        join.id(),
        &recipe,
        custody,
        custody_account,
        handoff_account,
        system_program,
        &rent,
    )?;
    let data = handoff_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    let authenticated = authenticate_persisted_source_policy_handoff(
        route,
        join,
        RuntimeAccountViewV1 {
            key: runtime_key(handoff_account.key),
            owner: runtime_key(handoff_account.owner),
            lamports: handoff_account.lamports(),
            executable: handoff_account.executable,
            writable: handoff_account.is_writable,
            signer: handoff_account.is_signer,
            data: &data,
        },
        derived,
        SourcePolicyHandoffAccessV1::CreatedMutable,
    )
    .map_err(source_runtime)?;
    require(
        authenticated.account_data_id() == funding.account_data_id
            && authenticated.account() == funding.account
            && authenticated.source_policy_handoff_join_id() == join.id(),
        ClutchError::MismatchedState,
    )?;
    Ok(PersistedSourcePolicyHandoffV1 {
        funding,
        authenticated,
    })
}

/// Re-authenticate a previously persisted action-10 handoff for the future
/// shared Product ResolutionV5 writer.
pub fn authenticate_persisted_source_policy_handoff_account(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    join: SourcePolicyHandoffJoinV1,
    handoff_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedPersistedSourcePolicyHandoffV1> {
    let recipe = PdaRecipeV3::source_policy_handoff(join.id()).map_err(source_pda)?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    let data = handoff_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    authenticate_persisted_source_policy_handoff(
        route,
        join,
        RuntimeAccountViewV1 {
            key: runtime_key(handoff_account.key),
            owner: runtime_key(handoff_account.owner),
            lamports: handoff_account.lamports(),
            executable: handoff_account.executable,
            writable: handoff_account.is_writable,
            signer: handoff_account.is_signer,
            data: &data,
        },
        derived,
        SourcePolicyHandoffAccessV1::ExistingReadOnly,
    )
    .map_err(source_runtime)
}

/// Persist the private terminal composer's explicit no-reopen decision and
/// hostile-reauthenticate its complete content-addressed postimage.
///
/// This function is crate-private and has no instruction decoder. The record
/// can enter it only from the Source terminal composer after Product's source
/// input, Failure's exact resolved receipt, and the final ResolutionV5
/// postwrite capability have all joined.
#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_source_no_reopen_terminal(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    body: SourceNoReopenTerminalV1,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    terminal_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<PersistedSourceNoReopenTerminalV1> {
    let terminal_id = body.id().map_err(source_runtime)?;
    let recipe =
        PdaRecipeV3::source_no_reopen_terminal(terminal_id).map_err(source_pda)?;
    let rent = read_rent(rent_sysvar)?;
    let funding = publish_immutable_source_input(
        program_id,
        &body,
        terminal_id,
        &recipe,
        custody,
        custody_account,
        terminal_account,
        system_program,
        &rent,
    )?;
    let data = terminal_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    let authenticated = authenticate_source_no_reopen_terminal(
        route,
        body,
        RuntimeAccountViewV1 {
            key: runtime_key(terminal_account.key),
            owner: runtime_key(terminal_account.owner),
            lamports: terminal_account.lamports(),
            executable: terminal_account.executable,
            writable: terminal_account.is_writable,
            signer: terminal_account.is_signer,
            data: &data,
        },
        derived,
        SourceNoReopenTerminalAccessV1::CreatedMutable,
    )
    .map_err(source_runtime)?;
    require(
        authenticated.account() == funding.account
            && authenticated.account_data_id() == funding.account_data_id
            && authenticated.terminal_id().map_err(source_runtime)? == terminal_id,
        ClutchError::MismatchedState,
    )?;
    Ok(PersistedSourceNoReopenTerminalV1 {
        funding,
        authenticated,
    })
}

/// Persist and hostile-reauthenticate the exact Source-owned terminal record
/// for either mature absence or a stable refused Result.
#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_source_failure_terminal_v1(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    body: SourceFailureTerminalV1,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    terminal_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<PersistedSourceFailureTerminalV1> {
    let terminal_id = body.id().map_err(source_runtime)?;
    let recipe = PdaRecipeV3::source_no_reopen_terminal(terminal_id).map_err(source_pda)?;
    let rent = read_rent(rent_sysvar)?;
    let funding = publish_immutable_source_input(
        program_id,
        &body,
        terminal_id,
        &recipe,
        custody,
        custody_account,
        terminal_account,
        system_program,
        &rent,
    )?;
    let data = terminal_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    let authenticated = authenticate_source_failure_terminal(
        route,
        body,
        RuntimeAccountViewV1 {
            key: runtime_key(terminal_account.key),
            owner: runtime_key(terminal_account.owner),
            lamports: terminal_account.lamports(),
            executable: terminal_account.executable,
            writable: terminal_account.is_writable,
            signer: terminal_account.is_signer,
            data: &data,
        },
        derived,
        SourceFailureTerminalAccessV1::CreatedMutable,
    )
    .map_err(source_runtime)?;
    require(
        authenticated.account() == funding.account
            && authenticated.account_data_id() == funding.account_data_id
            && authenticated.body() == body,
        ClutchError::MismatchedState,
    )?;
    Ok(PersistedSourceFailureTerminalV1 {
        funding,
        authenticated,
    })
}

/// Publish one exact reconstructed action-11 request under the release-selected
/// GenerationAuthority PDA, then hostile-reopen its complete postimage.
///
/// Current Source releases admit this direct publication only when the
/// GenerationAuthority is the exact already-authenticated adapter deployment.
/// An unrelated or merely address-selected external program cannot become an
/// authority through this path; a future external authority requires its own
/// pinned Program/ProgramData/config release and CPI contract.
#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_source_reopen_generation_request(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    request: SourceReopenGenerationRequestV1,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    request_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<PersistedSourceReopenGenerationRequestV1> {
    require(
        route.generation_authority_program() == runtime_key(program_id)
            && route.generation_authority_program() == route.adapter_program(),
        ClutchError::AuthorizationUnavailable,
    )?;
    require_system_program(system_program)?;
    require(
        request_account.is_writable
            && !request_account.is_signer
            && !request_account.executable
            && custody.account() == runtime_key(custody_account.key)
            && custody_account.key != request_account.key,
        ClutchError::MismatchedState,
    )?;
    let request_id = request.id().map_err(source_runtime)?;
    let bytes = request.encode().map_err(source_runtime)?;
    let authority = Pubkey::new_from_array(route.generation_authority_program().bytes());
    let (expected, bump) = crate::seeds::find(
        &authority,
        &[seeds::SEED_SOURCE_REOPEN_REQUEST_V1, &request_id.bytes()],
    );
    require(request_account.key == &expected, ClutchError::WrongPda)?;
    let rent = read_rent(rent_sysvar)?;
    let minimum = rent.minimum_balance(bytes.len())?;
    let before = request_account.lamports();
    let debit = minimum.saturating_sub(before);
    if request_account.owner == program_id {
        require(
            request_account.data_len() == bytes.len() && before >= minimum,
            ClutchError::MismatchedState,
        )?;
        let data = request_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(&*data == bytes.as_slice(), ClutchError::MismatchedState)?;
    } else {
        require_custody_creation_roles(custody, custody_account, request_account, system_program)?;
        let bump_seed = [bump];
        create_with_raw_seeds_from_custody(
            program_id,
            custody,
            custody_account,
            request_account,
            system_program,
            &rent,
            bytes.len(),
            &[
                seeds::SEED_SOURCE_REOPEN_REQUEST_V1,
                &request_id.bytes(),
                &bump_seed,
            ],
        )?;
        write_exact_account_data(request_account, &bytes)?;
    }
    let data = request_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        request_account.owner == program_id
            && request_account.data_len() == bytes.len()
            && request_account.lamports() >= minimum
            && &*data == bytes.as_slice()
            && SourceReopenGenerationRequestV1::decode(&data).map_err(source_runtime)? == request,
        ClutchError::MismatchedState,
    )?;
    let data_id = account_data_id(runtime_key(request_account.key), &data).map_err(source_runtime)?;
    let funding = ImmutableSourceInputFundingV1 {
        account: runtime_key(request_account.key),
        payer: if debit == 0 {
            RuntimeKey::ZERO
        } else {
            custody.account()
        },
        payer_debit_lamports: debit,
        permanent_prefund_lamports: before,
        account_data_id: data_id,
        semantic_id: request_id,
    };
    let postwrite_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/sbf/source-reopen-request-postwrite/v1",
            &route.route_id().bytes(),
            &route.adapter_deployment_id().bytes(),
            &request_account.key.to_bytes(),
            &data_id.bytes(),
            &request_id.bytes(),
            &request.generation_policy_id().bytes(),
            &request.expected_lineage_state_id().bytes(),
            &request.target().body_id().map_err(source_runtime)?.bytes(),
        ])
        .to_bytes(),
    );
    require(!postwrite_id.is_zero(), ClutchError::MismatchedState)?;
    Ok(PersistedSourceReopenGenerationRequestV1 {
        funding,
        request,
        postwrite_id,
    })
}

/// Persist the exact initial/repair GenerationAuthority request reconstructed
/// by Product from its authenticated occurrence/window graph. The current
/// release fixes the authority program, schedule, SourceSpec, and contract;
/// the caller cannot publish a differently scoped request at this address.
#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_source_generation_request_v1(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    request: SourceGenerationRequestV1,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    request_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<PersistedSourceGenerationRequestV1> {
    request.validate().map_err(source_runtime)?;
    require(
        route.generation_authority_program() == runtime_key(program_id)
            && route.generation_authority_program() == route.adapter_program()
            && request.source_plane_contract_id == route.source_plane_contract_id()
            && request.source_spec_id == route.source_spec_id()
            && request.source_work_schedule_id == route.source_work_schedule_id(),
        ClutchError::AuthorizationUnavailable,
    )?;
    require_system_program(system_program)?;
    require_custody_creation_roles(custody, custody_account, request_account, system_program)?;
    let request_id = request.id().map_err(source_runtime)?;
    let bytes = request.encode().map_err(source_runtime)?;
    let authority = Pubkey::new_from_array(route.generation_authority_program().bytes());
    let (expected, bump) = crate::seeds::find(
        &authority,
        &[seeds::SEED_SOURCE_GENERATION_REQUEST_V1, &request_id.bytes()],
    );
    require(request_account.key == &expected, ClutchError::WrongPda)?;
    let rent = read_rent(rent_sysvar)?;
    let minimum = rent.minimum_balance(bytes.len())?;
    let before = request_account.lamports();
    let debit = minimum.saturating_sub(before);
    let bump_seed = [bump];
    create_with_raw_seeds_from_custody(
        program_id,
        custody,
        custody_account,
        request_account,
        system_program,
        &rent,
        bytes.len(),
        &[
            seeds::SEED_SOURCE_GENERATION_REQUEST_V1,
            &request_id.bytes(),
            &bump_seed,
        ],
    )?;
    write_exact_account_data(request_account, &bytes)?;
    let data = request_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let data_id = account_data_id(runtime_key(request_account.key), &data).map_err(source_runtime)?;
    let funding = ImmutableSourceInputFundingV1 {
        account: runtime_key(request_account.key),
        payer: if debit == 0 { RuntimeKey::ZERO } else { custody.account() },
        payer_debit_lamports: debit,
        permanent_prefund_lamports: before,
        account_data_id: data_id,
        semantic_id: request_id,
    };
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/sbf/source-generation-request-postwrite/v1",
            &route.route_id().bytes(),
            &route.adapter_deployment_id().bytes(),
            &request_account.key.to_bytes(),
            &data_id.bytes(),
            &request_id.bytes(),
            &request.generation_policy_id.bytes(),
            &custody.id().bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(PersistedSourceGenerationRequestV1 {
        funding,
        request,
        id,
    })
}

/// Persist one immutable RawPage or WindowSeal account with its exact header
/// and explicit payer/donation rent partition.
#[allow(clippy::too_many_arguments)]
fn create_immutable_runtime_account<T: RuntimeAccountBodyV1>(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    recipe: &PdaRecipeV3,
    body: &T,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<(ImmutableAccountFundingV1, RuntimeAccountHeaderV1, ContentId)> {
    require_custody_creation_roles(custody, custody_account, target, system_program)?;
    let derived = derive_runtime_pda(program_id, recipe).map_err(Refusal::from)?;
    require(
        derived.address == runtime_key(target.key),
        ClutchError::WrongPda,
    )?;
    let space = RUNTIME_ACCOUNT_HEADER_BYTES
        .checked_add(T::ENCODED_LEN)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let rent = read_rent(rent_sysvar)?;
    let rent_id = rent_sysvar_id(rent_sysvar)?;
    let minimum = rent.minimum_balance(space)?;
    let before = target.lamports();
    let debit = minimum.saturating_sub(before);
    let after = before
        .checked_add(debit)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let payer_key = if debit == 0 {
        RuntimeKey::ZERO
    } else {
        custody.account()
    };
    let header = RuntimeAccountHeaderV1 {
        family: T::FAMILY,
        bump: derived.bump,
        principal_recipient: payer_key,
        payer_principal_lamports: debit,
        donation_floor_lamports: before,
        generation: 1,
    };
    let mut postimage = vec![0_u8; space];
    encode_runtime_account(header, body, route.neutral_sink(), &mut postimage)
        .map_err(source_runtime)?;
    let data_id = account_data_id(runtime_key(target.key), &postimage).map_err(source_runtime)?;
    create_with_recipe_from_custody(
        program_id,
        custody,
        custody_account,
        target,
        system_program,
        &rent,
        space,
        recipe,
        derived.bump,
    )?;
    write_exact_account_data(target, &postimage)?;
    Ok((
        ImmutableAccountFundingV1 {
            account: runtime_key(target.key),
            payer: payer_key,
            payer_debit_lamports: debit,
            donation_lamports: before,
            rent_sysvar_id: rent_id,
            rent_exempt_minimum_lamports: minimum,
            account_balance_after: after,
        },
        header,
        data_id,
    ))
}

/// Persist one immutable 0x92 work receipt. Its rent ownership is returned
/// explicitly; liveness work principal remains in the separate Source account.
#[allow(clippy::too_many_arguments)]
pub fn create_work_receipt_account(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    receipt: SourceWorkReceiptAccountV1,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<(ImmutableAccountFundingV1, AuthenticatedSourceWorkReceiptV1)> {
    require_custody_creation_roles(custody, custody_account, target, system_program)?;
    let slot = receipt
        .receipt_slot_id(route, schedule)
        .map_err(source_runtime)?;
    let recipe = PdaRecipeV3::source_work_receipt(slot).map_err(source_pda)?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    require(
        derived.address == runtime_key(target.key)
            && receipt.receipt_account_id() == runtime_key(target.key),
        ClutchError::WrongPda,
    )?;
    let bytes = receipt.encode().map_err(source_runtime)?;
    let rent = read_rent(rent_sysvar)?;
    let rent_id = rent_sysvar_id(rent_sysvar)?;
    let minimum = rent.minimum_balance(bytes.len())?;
    let before = target.lamports();
    let debit = minimum.saturating_sub(before);
    let after = before
        .checked_add(debit)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    create_with_recipe_from_custody(
        program_id,
        custody,
        custody_account,
        target,
        system_program,
        &rent,
        bytes.len(),
        &recipe,
        derived.bump,
    )?;
    write_exact_account_data(target, &bytes)?;
    let funding = ImmutableAccountFundingV1 {
        account: runtime_key(target.key),
        payer: if debit == 0 {
            RuntimeKey::ZERO
        } else {
            custody.account()
        },
        payer_debit_lamports: debit,
        donation_lamports: before,
        rent_sysvar_id: rent_id,
        rent_exempt_minimum_lamports: minimum,
        account_balance_after: after,
    };
    let data = target
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let authenticated = authenticate_source_work_receipt_account(
        route,
        schedule,
        RuntimeAccountViewV1 {
            key: runtime_key(target.key),
            owner: runtime_key(target.owner),
            lamports: target.lamports(),
            executable: target.executable,
            writable: target.is_writable,
            signer: target.is_signer,
            data: &data,
        },
        derived,
        SourceWorkReceiptAccessV1::CreatedMutable,
    )
    .map_err(source_runtime)?;
    Ok((funding, authenticated))
}

/// Emit action 10 absence path: bind the mature absence handoff to its three
/// physical evidence accounts and the persisted paid-work receipt.
pub fn join_failure_absence_handoff(
    route: AuthenticatedSourceRouteV1,
    handoff: FailurePolicySourceHandoffV1,
    absence: AuthenticatedStatisticResultAbsenceV1,
    work_receipt: AuthenticatedSourceWorkReceiptV1,
) -> Outcome<SourcePolicyHandoffJoinV1> {
    SourcePolicyHandoffJoinV1::failure_absence(route, handoff, absence, work_receipt)
        .map_err(source_runtime)
}

/// Emit action 10 refusal path: bind the durable refused result account and
/// exact failure handoff to one persisted paid-work receipt.
pub fn join_failure_result_handoff(
    route: AuthenticatedSourceRouteV1,
    handoff: FailurePolicySourceHandoffV1,
    result: AuthenticatedStatisticResultAccountV1,
    work_receipt: AuthenticatedSourceWorkReceiptV1,
) -> Outcome<SourcePolicyHandoffJoinV1> {
    SourcePolicyHandoffJoinV1::failure_result(route, handoff, result, work_receipt)
        .map_err(source_runtime)
}

/// Emit action 10 success path: bind successful source evidence for downstream
/// relation review without allowing Source to classify the relation outcome.
pub fn join_successful_evaluation_handoff(
    route: AuthenticatedSourceRouteV1,
    handoff: SuccessfulEvaluationHandoffV1,
    result: AuthenticatedStatisticResultAccountV1,
    work_receipt: AuthenticatedSourceWorkReceiptV1,
) -> Outcome<SourcePolicyHandoffJoinV1> {
    SourcePolicyHandoffJoinV1::successful_evaluation(route, handoff, result, work_receipt)
        .map_err(source_runtime)
}

/// Generic action 11 engine: open the exact next generation and persist body
/// plus lineage as one rollback-safe postimage pair.
#[allow(clippy::too_many_arguments)]
pub fn reopen_runtime_account<T: RuntimeAccountBodyV1>(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    family: LineageFamilyV1,
    semantic_binding_id: ContentId,
    recipe: &PdaRecipeV3,
    body: &T,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<OpenRuntimeAccountResultV1> {
    require_custody_creation_roles(custody, custody_account, target, system_program)?;
    require_lineage_account(route, lineage, lineage_account)?;
    require(!lineage.lineage().is_open, ClutchError::MismatchedState)?;
    let derived = derive_runtime_pda(program_id, recipe).map_err(Refusal::from)?;
    let reopen = authorize_reopen(
        route,
        lineage,
        family,
        semantic_binding_id,
        recipe.id().map_err(source_pda)?,
        runtime_key(target.key),
        derived,
    )
    .map_err(source_runtime)?;
    let space = RUNTIME_ACCOUNT_HEADER_BYTES
        .checked_add(T::ENCODED_LEN)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let rent = read_rent(rent_sysvar)?;
    let minimum = rent.minimum_balance(space)?;
    let before = target.lamports();
    let debit = minimum.saturating_sub(before);
    let after = before
        .checked_add(debit)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let funding = plan_source_account_creation(
        route,
        reopen,
        RentExemptionQuoteV1 {
            rent_sysvar_id: rent_sysvar_id(rent_sysvar)?,
            account: runtime_key(target.key),
            data_len: u32::try_from(space)
                .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?,
            minimum_balance_lamports: minimum,
        },
        custody.account(),
        before,
        debit,
        after,
    )
    .map_err(source_runtime)?;
    let header = RuntimeAccountHeaderV1 {
        family: T::FAMILY,
        bump: derived.bump,
        principal_recipient: funding.ledger.principal_recipient,
        payer_principal_lamports: funding.ledger.payer_principal_lamports,
        donation_floor_lamports: funding.ledger.donation_lamports,
        generation: funding.ledger.generation,
    };
    let mut postimage = vec![0_u8; space];
    encode_runtime_account(header, body, route.neutral_sink(), &mut postimage)
        .map_err(source_runtime)?;
    let data_id = account_data_id(runtime_key(target.key), &postimage).map_err(source_runtime)?;
    let lineage_after =
        open_lineage_generation(lineage.lineage(), reopen, data_id).map_err(source_runtime)?;
    let lineage_bytes = lineage_after.encode().map_err(source_runtime)?;

    create_with_recipe_from_custody(
        program_id,
        custody,
        custody_account,
        target,
        system_program,
        &rent,
        space,
        recipe,
        derived.bump,
    )?;
    write_exact_account_data(target, &postimage)?;
    write_exact_account_data(lineage_account, &lineage_bytes)?;
    Ok(OpenRuntimeAccountResultV1 {
        funding,
        lineage_funding: None,
        header,
        account_data_id: data_id,
        lineage_after,
    })
}

/// Create a permanent release/route-bound lineage and its first runtime
/// generation as one rollback-safe instruction. This is intentionally used
/// only for action 2; action 11 requires an authenticated closed lineage.
#[allow(clippy::too_many_arguments)]
fn bootstrap_runtime_account<T: RuntimeAccountBodyV1>(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    family: LineageFamilyV1,
    semantic_binding_id: ContentId,
    recipe: &PdaRecipeV3,
    body: &T,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<OpenRuntimeAccountResultV1> {
    require_custody_creation_roles(custody, custody_account, target, system_program)?;
    require_custody_creation_roles(custody, custody_account, lineage_account, system_program)?;
    require(target.key != lineage_account.key, ClutchError::AccountAlias)?;
    let target_derived = derive_runtime_pda(program_id, recipe).map_err(Refusal::from)?;
    require(
        target_derived.address == runtime_key(target.key),
        ClutchError::WrongPda,
    )?;
    let lineage_recipe_id = ReopenLineageV1::recipe_id_for(
        route.adapter_program(),
        route.release_manifest_id(),
        route.route_id(),
        family,
        semantic_binding_id,
        route.source_work_schedule_id(),
    )
    .map_err(source_runtime)?;
    let lineage_recipe = PdaRecipeV3::reopen_lineage(lineage_recipe_id).map_err(source_pda)?;
    let lineage_derived = derive_runtime_pda(program_id, &lineage_recipe).map_err(Refusal::from)?;
    require(
        lineage_derived.address == runtime_key(lineage_account.key),
        ClutchError::WrongPda,
    )?;
    let lineage = ReopenLineageV1::new(
        route.adapter_program(),
        route.release_manifest_id(),
        route.route_id(),
        semantic_binding_id,
        runtime_key(lineage_account.key),
        family,
        route.source_work_schedule_id(),
        route.neutral_sink(),
    )
    .map_err(source_runtime)?;
    let reopen = authorize_reopen(
        route,
        lineage,
        family,
        semantic_binding_id,
        recipe.id().map_err(source_pda)?,
        runtime_key(target.key),
        target_derived,
    )
    .map_err(source_runtime)?;
    let target_space = RUNTIME_ACCOUNT_HEADER_BYTES
        .checked_add(T::ENCODED_LEN)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let rent = read_rent(rent_sysvar)?;
    let rent_id = rent_sysvar_id(rent_sysvar)?;
    let target_minimum = rent.minimum_balance(target_space)?;
    let target_before = target.lamports();
    let target_debit = target_minimum.saturating_sub(target_before);
    let target_after = target_before
        .checked_add(target_debit)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let funding = plan_source_account_creation(
        route,
        reopen,
        RentExemptionQuoteV1 {
            rent_sysvar_id: rent_id,
            account: runtime_key(target.key),
            data_len: u32::try_from(target_space)
                .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?,
            minimum_balance_lamports: target_minimum,
        },
        custody.account(),
        target_before,
        target_debit,
        target_after,
    )
    .map_err(source_runtime)?;
    let header = RuntimeAccountHeaderV1 {
        family: T::FAMILY,
        bump: target_derived.bump,
        principal_recipient: funding.ledger.principal_recipient,
        payer_principal_lamports: funding.ledger.payer_principal_lamports,
        donation_floor_lamports: funding.ledger.donation_lamports,
        generation: funding.ledger.generation,
    };
    let mut target_postimage = vec![0_u8; target_space];
    encode_runtime_account(header, body, route.neutral_sink(), &mut target_postimage)
        .map_err(source_runtime)?;
    let target_data_id =
        account_data_id(runtime_key(target.key), &target_postimage).map_err(source_runtime)?;
    let lineage_after =
        open_lineage_generation(lineage, reopen, target_data_id).map_err(source_runtime)?;
    let lineage_postimage = lineage_after.encode().map_err(source_runtime)?;
    let lineage_minimum = rent.minimum_balance(REOPEN_LINEAGE_BYTES)?;
    let lineage_before = lineage_account.lamports();
    let lineage_debit = lineage_minimum.saturating_sub(lineage_before);
    let lineage_after_balance = lineage_before
        .checked_add(lineage_debit)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let lineage_funding = ImmutableAccountFundingV1 {
        account: runtime_key(lineage_account.key),
        payer: if lineage_debit == 0 {
            RuntimeKey::ZERO
        } else {
            custody.account()
        },
        payer_debit_lamports: lineage_debit,
        donation_lamports: lineage_before,
        rent_sysvar_id: rent_id,
        rent_exempt_minimum_lamports: lineage_minimum,
        account_balance_after: lineage_after_balance,
    };

    create_with_recipe_from_custody(
        program_id,
        custody,
        custody_account,
        lineage_account,
        system_program,
        &rent,
        REOPEN_LINEAGE_BYTES,
        &lineage_recipe,
        lineage_derived.bump,
    )?;
    create_with_recipe_from_custody(
        program_id,
        custody,
        custody_account,
        target,
        system_program,
        &rent,
        target_space,
        recipe,
        target_derived.bump,
    )?;
    write_exact_account_data(lineage_account, &lineage_postimage)?;
    write_exact_account_data(target, &target_postimage)?;
    Ok(OpenRuntimeAccountResultV1 {
        funding,
        lineage_funding: Some(lineage_funding),
        header,
        account_data_id: target_data_id,
        lineage_after,
    })
}

/// Persist one typed runtime body and advance the exact open lineage CAS.
fn mutate_runtime_account<T: RuntimeAccountBodyV1>(
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    body_after: &T,
) -> Outcome<MutateRuntimeAccountResultV1> {
    require_lineage_account(route, lineage, lineage_account)?;
    require(
        account.owner == &Pubkey::new_from_array(route.adapter_program().bytes())
            && account.is_writable
            && !account.executable
            && !account.is_signer,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (header, _) =
        decode_runtime_account::<T>(&data, route.neutral_sink()).map_err(source_runtime)?;
    let before_id = account_data_id(runtime_key(account.key), &data).map_err(source_runtime)?;
    drop(data);
    let state = lineage.lineage();
    require(
        state.is_open
            && state.active_account == runtime_key(account.key)
            && state.latest_generation == header.generation
            && state.last_opened_state_id == before_id,
        ClutchError::MismatchedState,
    )?;
    let mut postimage = vec![0_u8; RUNTIME_ACCOUNT_HEADER_BYTES + T::ENCODED_LEN];
    encode_runtime_account(header, body_after, route.neutral_sink(), &mut postimage)
        .map_err(source_runtime)?;
    let after_id = account_data_id(runtime_key(account.key), &postimage).map_err(source_runtime)?;
    let lineage_after = advance_lineage_state(
        state,
        runtime_key(account.key),
        header.generation,
        before_id,
        after_id,
    )
    .map_err(source_runtime)?;
    let lineage_bytes = lineage_after.encode().map_err(source_runtime)?;
    write_exact_account_data(account, &postimage)?;
    write_exact_account_data(lineage_account, &lineage_bytes)?;
    Ok(MutateRuntimeAccountResultV1 {
        account_data_before_id: before_id,
        account_data_after_id: after_id,
        lineage_after,
    })
}

/// Permanently tombstone action 10's exact never-created StatisticResult
/// lineage. No Result account is created or closed on this branch.
pub(crate) fn retire_absent_statistic_result_lineage_v1(
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    lineage_account: &AccountInfo<'_>,
    statistic_key_id: ContentId,
    terminal_semantic_id: ContentId,
) -> Outcome<AuthenticatedAbsentStatisticResultLineageRetirementV1> {
    require_lineage_account(route, lineage, lineage_account)?;
    let state = lineage.lineage();
    let statistic_result_recipe_id = PdaRecipeV3::statistic_result(statistic_key_id)
        .and_then(|recipe| recipe.id())
        .map_err(source_pda)?;
    require(
        state.family == LineageFamilyV1::StatisticResult
            && state.semantic_binding_id == statistic_result_recipe_id
            && state.latest_generation == 0
            && !state.is_open
            && state.active_account.is_zero()
            && state.last_opened_state_id.is_zero()
            && state.last_close_receipt_id.is_zero()
            && !terminal_semantic_id.is_zero(),
        ClutchError::MismatchedState,
    )?;
    let lineage_after = retire_never_created_lineage(
        lineage,
        LineageFamilyV1::StatisticResult,
        statistic_result_recipe_id,
        terminal_semantic_id,
    )
    .map_err(source_runtime)?;
    let lineage_bytes = lineage_after.encode().map_err(source_runtime)?;
    let lineage_state_after_id =
        account_data_id(runtime_key(lineage_account.key), &lineage_bytes)
            .map_err(source_runtime)?;
    write_exact_account_data(lineage_account, &lineage_bytes)?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/sbf/absent-statistic-result-lineage-retirement/v1",
            &route.route_id().bytes(),
            &statistic_key_id.bytes(),
            &statistic_result_recipe_id.bytes(),
            lineage_account.key.as_ref(),
            &lineage.id().bytes(),
            &lineage.account_data_id().bytes(),
            &lineage_state_after_id.bytes(),
            &terminal_semantic_id.bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedAbsentStatisticResultLineageRetirementV1 {
        id,
        lineage_account: runtime_key(lineage_account.key),
        lineage_authentication_before_id: lineage.id(),
        lineage_state_before_id: lineage.account_data_id(),
        lineage_state_after_id,
        lineage_after,
    })
}

/// Close action 12 for a SourceHead generation.
#[allow(clippy::too_many_arguments)]
pub fn close_head_generation(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    terminal_receipt: AuthenticatedSourceWorkReceiptV1,
) -> Outcome<CloseRuntimeAccountResultV1> {
    close_terminal_runtime_account::<SourceHeadV3>(
        program_id,
        route,
        lineage,
        account,
        lineage_account,
        principal_refund,
        neutral_sink,
        terminal_receipt,
    )
}

/// Close action 12 for an OpenRawPage generation.
#[allow(clippy::too_many_arguments)]
pub fn close_open_page_generation(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    terminal_receipt: AuthenticatedSourceWorkReceiptV1,
) -> Outcome<CloseRuntimeAccountResultV1> {
    close_terminal_runtime_account::<OpenRawPageV3>(
        program_id,
        route,
        lineage,
        account,
        lineage_account,
        principal_refund,
        neutral_sink,
        terminal_receipt,
    )
}

/// Close action 12 for a WindowWork generation.
#[allow(clippy::too_many_arguments)]
pub fn close_window_work_generation(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    terminal_receipt: AuthenticatedSourceWorkReceiptV1,
) -> Outcome<CloseRuntimeAccountResultV1> {
    close_terminal_runtime_account::<WindowWorkV3>(
        program_id,
        route,
        lineage,
        account,
        lineage_account,
        principal_refund,
        neutral_sink,
        terminal_receipt,
    )
}

/// Close action 12 for a persisted StatisticResult repair generation.
#[allow(clippy::too_many_arguments)]
pub fn close_statistic_result_generation(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    terminal_receipt: AuthenticatedSourceWorkReceiptV1,
) -> Outcome<CloseRuntimeAccountResultV1> {
    close_terminal_runtime_account::<StatisticResultV3>(
        program_id,
        route,
        lineage,
        account,
        lineage_account,
        principal_refund,
        neutral_sink,
        terminal_receipt,
    )
}

/// Consume only the authenticated terminal-success receipt minted after the
/// shared Product ResolutionV5 write. Caller-supplied terminal IDs never enter
/// the close engine as authority.
#[allow(clippy::too_many_arguments)]
fn close_terminal_runtime_account<T: RuntimeAccountBodyV1>(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    terminal_receipt: AuthenticatedSourceWorkReceiptV1,
) -> Outcome<CloseRuntimeAccountResultV1> {
    let receipt = terminal_receipt.receipt();
    require(
        receipt.route_id() == route.route_id()
            && receipt.disposition() == SourceReceiptDispositionV1::TerminalSuccess
            && receipt.work_kind().is_none()
            && receipt.call_ordinal() == 0
            && receipt.call_ceiling_lamports() == 0,
        ClutchError::MismatchedState,
    )?;
    close_runtime_account::<T>(
        program_id,
        route,
        lineage,
        account,
        lineage_account,
        principal_refund,
        neutral_sink,
        receipt.semantic_receipt_id(),
    )
}

/// Generic action 12 engine: close one mutable generation, refund only stored
/// payer principal, sink every donation/surplus lamport, and retain lineage.
fn close_runtime_account<T: RuntimeAccountBodyV1>(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    semantic_terminal_receipt_id: ContentId,
) -> Outcome<CloseRuntimeAccountResultV1> {
    require_lineage_account(route, lineage, lineage_account)?;
    require(
        account.owner == program_id
            && account.is_writable
            && principal_refund.is_writable
            && neutral_sink.is_writable
            && !account.executable
            && !principal_refund.executable
            && !neutral_sink.executable,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (header, _) =
        decode_runtime_account::<T>(&data, route.neutral_sink()).map_err(source_runtime)?;
    let final_state_id =
        account_data_id(runtime_key(account.key), &data).map_err(source_runtime)?;
    drop(data);
    require(
        runtime_key(principal_refund.key) == header.principal_recipient
            || (header.payer_principal_lamports == 0
                && runtime_key(principal_refund.key) != route.neutral_sink()),
        ClutchError::MismatchedState,
    )?;
    require(
        runtime_key(neutral_sink.key) == route.neutral_sink()
            && account.key != principal_refund.key
            && account.key != neutral_sink.key
            && principal_refund.key != neutral_sink.key,
        ClutchError::AccountAlias,
    )?;
    let funding = plan_runtime_account_close_from_header(
        runtime_key(account.key),
        header,
        route.neutral_sink(),
        account.lamports(),
        semantic_terminal_receipt_id,
    )
    .map_err(source_runtime)?;
    let lineage_after = close_lineage_generation(
        lineage.lineage(),
        runtime_key(account.key),
        header.generation,
        final_state_id,
        semantic_terminal_receipt_id,
    )
    .map_err(source_runtime)?;
    let lineage_bytes = lineage_after.encode().map_err(source_runtime)?;
    let refund_after = principal_refund
        .lamports()
        .checked_add(funding.payer_refund_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let sink_after = neutral_sink
        .lamports()
        .checked_add(funding.neutral_surplus_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;

    debit_all(account)?;
    credit_lamports(principal_refund, funding.payer_refund_lamports)?;
    credit_lamports(neutral_sink, funding.neutral_surplus_lamports)?;
    if funding.payer_refund_lamports != 0 {
        credit_source_funding_custody_ledger_from_close_v1(
            program_id,
            route,
            principal_refund,
            funding.payer_refund_lamports,
            funding.close_receipt_id,
        )?;
    }
    require(
        account.lamports() == 0
            && principal_refund.lamports() == refund_after
            && neutral_sink.lamports() == sink_after,
        ClutchError::MismatchedState,
    )?;
    account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    account.assign(&SYSTEM_PROGRAM_ID);
    write_exact_account_data(lineage_account, &lineage_bytes)?;
    Ok(CloseRuntimeAccountResultV1 {
        funding,
        lineage_after,
    })
}

fn require_creation_roles(
    program_id: &Pubkey,
    payer: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
) -> Outcome<()> {
    require_system_program(system_program)?;
    require_creatable(target)?;
    require(
        payer.is_signer
            && payer.is_writable
            && !payer.executable
            && payer.key != target.key
            && program_id != &SYSTEM_PROGRAM_ID,
        ClutchError::MismatchedState,
    )
}

fn require_custody_creation_roles(
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
) -> Outcome<()> {
    require_system_program(system_program)?;
    require_creatable(target)?;
    require(
        custody.account() == runtime_key(custody_account.key)
            && custody_account.owner == &Pubkey::new_from_array(custody.ledger().adapter_program.bytes())
            && custody_account.data_len() == SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES
            && custody_account.is_writable
            && !custody_account.is_signer
            && !custody_account.executable
            && custody_account.key != target.key,
        ClutchError::MismatchedState,
    )
}

fn require_lineage_account(
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
) -> Outcome<()> {
    require(
        lineage.access() == clutch_source_plane_v3_runtime::LineageAccessV1::Mutable
            && runtime_key(account.key) == lineage.lineage().lineage_account
            && account.owner == &Pubkey::new_from_array(route.adapter_program().bytes())
            && account.is_writable
            && !account.is_signer
            && !account.executable,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn create_with_recipe<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    recipe: &PdaRecipeV3,
    bump: u8,
) -> Outcome<()> {
    let mut seeds: [&[u8]; clutch_source_plane_v3_adapter::MAX_PDA_SEEDS + 1] =
        [&[]; clutch_source_plane_v3_adapter::MAX_PDA_SEEDS + 1];
    let count = usize::from(recipe.seed_count());
    let mut index = 0_usize;
    while index < count {
        seeds[index] = recipe.seed(index).map_err(source_pda)?;
        index += 1;
    }
    let bump_seed = [bump];
    seeds[count] = &bump_seed;
    create_pda_account(
        program_id,
        payer,
        target,
        system_program,
        rent,
        space,
        &seeds[..=count],
    )
}

/// Allocate one Source PDA from the exact prepaid lifecycle custody.
///
/// The custody and target are both PDAs. Program ownership permits an exact
/// ledgered principal debit without a signer; the target signs only
/// Allocate/Assign. Existing target prefunds remain neutral donation.
#[allow(clippy::too_many_arguments)]
fn create_with_recipe_from_custody<'a>(
    program_id: &Pubkey,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    recipe: &PdaRecipeV3,
    target_bump: u8,
) -> Outcome<()> {
    require_system_program(system_program)?;
    require_creatable(target)?;
    require(
        custody.account() == runtime_key(custody_account.key)
            && custody_account.owner == program_id
            && custody_account.data_len() == SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES
            && custody_account.is_writable
            && !custody_account.is_signer
            && !custody_account.executable
            && custody_account.key != target.key,
        ClutchError::MismatchedState,
    )?;
    let minimum = rent.minimum_balance(space)?;
    let before = target.lamports();
    if before < minimum {
        let shortfall = minimum
            .checked_sub(before)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let custody_before = custody_account.lamports();
        let expected_custody_after = custody_before
            .checked_sub(shortfall)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let expected_target_after = before
            .checked_add(shortfall)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        debit_lamports(custody_account, shortfall)?;
        credit_lamports(target, shortfall)?;
        require(
            custody_account.lamports() == expected_custody_after
                && target.lamports() == expected_target_after,
            ClutchError::AccountCreationFailed,
        )?;
        let semantic_id = source_custody_physical_transition_id(
            custody,
            runtime_key(target.key),
            shortfall,
            0,
            recipe.id().map_err(source_pda)?,
        );
        transition_source_funding_custody_ledger_v1(
            custody,
            custody_account,
            shortfall,
            0,
            semantic_id,
        )?;
    }
    let funded = target.lamports();
    require(funded >= minimum, ClutchError::AccountCreationFailed)?;

    let mut target_seeds: [&[u8]; clutch_source_plane_v3_adapter::MAX_PDA_SEEDS + 1] =
        [&[]; clutch_source_plane_v3_adapter::MAX_PDA_SEEDS + 1];
    let target_seed_count = usize::from(recipe.seed_count());
    let mut index = 0_usize;
    while index < target_seed_count {
        target_seeds[index] = recipe.seed(index).map_err(source_pda)?;
        index += 1;
    }
    let target_bump_seed = [target_bump];
    target_seeds[target_seed_count] = &target_bump_seed;
    let signer_seeds = &target_seeds[..=target_seed_count];
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(space),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.owner == program_id && target.data_len() == space && target.lamports() == funded,
        ClutchError::AccountCreationFailed,
    )
}

fn transfer_from_source_custody_v1<'a>(
    program_id: &Pubkey,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    lamports: u64,
) -> Outcome<()> {
    require_system_program(system_program)?;
    require(
        lamports != 0
            && custody.account() == runtime_key(custody_account.key)
            && custody_account.owner == program_id
            && custody_account.data_len() == SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES
            && custody_account.is_writable
            && !custody_account.is_signer
            && !custody_account.executable
            && destination.is_writable
            && custody_account.key != destination.key,
        ClutchError::MismatchedState,
    )?;
    let custody_before = custody_account.lamports();
    let destination_before = destination.lamports();
    debit_lamports(custody_account, lamports)?;
    credit_lamports(destination, lamports)?;
    require(
        custody_account.lamports()
            == custody_before
                .checked_sub(lamports)
                .ok_or(ClutchError::Arithmetic)?
            && destination.lamports()
                == destination_before
                    .checked_add(lamports)
                    .ok_or(ClutchError::Arithmetic)?,
        ClutchError::AccountCreationFailed,
    )?;
    let semantic_id = source_custody_physical_transition_id(
        custody,
        runtime_key(destination.key),
        lamports,
        0,
        ContentId::from_bytes(solana_sha256_hasher::hashv(&[
            b"dragons-clutch/sbf/source-custody-direct-payment/v1",
            destination.key.as_ref(),
            &lamports.to_le_bytes(),
        ]).to_bytes()),
    );
    transition_source_funding_custody_ledger_v1(
        custody,
        custody_account,
        lamports,
        0,
        semantic_id,
    )
}

#[allow(clippy::too_many_arguments)]
fn create_with_raw_seeds_from_custody<'a>(
    program_id: &Pubkey,
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    target_signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_custody_creation_roles(custody, custody_account, target, system_program)?;
    let minimum = rent.minimum_balance(space)?;
    let before = target.lamports();
    if before < minimum {
        let shortfall = minimum
            .checked_sub(before)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let custody_before = custody_account.lamports();
        debit_lamports(custody_account, shortfall)?;
        credit_lamports(target, shortfall)?;
        require(
            custody_account.lamports()
                == custody_before
                    .checked_sub(shortfall)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
                && target.lamports()
                    == before
                        .checked_add(shortfall)
                        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
            ClutchError::AccountCreationFailed,
        )?;
        let raw_seed_id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[
            b"dragons-clutch/sbf/source-custody-raw-seed-target/v1",
            target.key.as_ref(),
            &u64::try_from(space)
                .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?
                .to_le_bytes(),
        ]).to_bytes());
        let semantic_id = source_custody_physical_transition_id(
            custody,
            runtime_key(target.key),
            shortfall,
            0,
            raw_seed_id,
        );
        transition_source_funding_custody_ledger_v1(
            custody,
            custody_account,
            shortfall,
            0,
            semantic_id,
        )?;
    }
    let funded = target.lamports();
    require(funded >= minimum, ClutchError::AccountCreationFailed)?;
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(space),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[target_signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[target_signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.owner == program_id && target.data_len() == space && target.lamports() == funded,
        ClutchError::AccountCreationFailed,
    )
}

fn write_exact_account_data(account: &AccountInfo<'_>, bytes: &[u8]) -> Outcome<()> {
    require(
        account.is_writable && account.data_len() == bytes.len(),
        ClutchError::WrongDataLength,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.copy_from_slice(bytes);
    Ok(())
}

fn source_custody_physical_transition_id(
    custody: AuthenticatedSourceFundingCustodyV1,
    counterparty: RuntimeKey,
    principal_debit_lamports: u64,
    principal_credit_lamports: u64,
    semantic_id: ContentId,
) -> ContentId {
    ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_FUNDING_CUSTODY_PHYSICAL_TRANSITION_DOMAIN_V1,
            &custody.id().bytes(),
            &custody.account_data_id().bytes(),
            &counterparty.bytes(),
            &principal_debit_lamports.to_le_bytes(),
            &principal_credit_lamports.to_le_bytes(),
            &semantic_id.bytes(),
        ])
        .to_bytes(),
    )
}

fn transition_source_funding_custody_ledger_v1(
    custody: AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    principal_debit_lamports: u64,
    principal_credit_lamports: u64,
    semantic_postwrite_id: ContentId,
) -> Outcome<SourceFundingCustodyLedgerV1> {
    require(
        custody.account() == runtime_key(custody_account.key)
            && custody_account.owner
                == &Pubkey::new_from_array(custody.ledger().adapter_program.bytes())
            && custody_account.data_len() == SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES
            && custody_account.is_writable
            && !custody_account.is_signer
            && !custody_account.executable,
        ClutchError::MismatchedState,
    )?;
    let data = custody_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let before = SourceFundingCustodyLedgerV1::decode(&data).map_err(source_runtime)?;
    drop(data);
    let immutable = custody.ledger();
    require(
        before.adapter_program == immutable.adapter_program
            && before.release_manifest_id == immutable.release_manifest_id
            && before.route_id == immutable.route_id
            && before.source_work_schedule_id == immutable.source_work_schedule_id
            && before.lifecycle_id == immutable.lifecycle_id
            && before.custody_account == immutable.custody_account
            && before.principal_refund == immutable.principal_refund
            && before.neutral_sink == immutable.neutral_sink
            && before.allocated_principal_lamports == immutable.allocated_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let after = before
        .transition(
            principal_debit_lamports,
            principal_credit_lamports,
            custody_account.lamports(),
            semantic_postwrite_id,
        )
        .map_err(source_runtime)?;
    write_exact_account_data(
        custody_account,
        &after.encode().map_err(source_runtime)?,
    )?;
    Ok(after)
}

fn credit_source_funding_custody_ledger_from_close_v1(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    custody_account: &AccountInfo<'_>,
    principal_credit_lamports: u64,
    close_receipt_id: ContentId,
) -> Outcome<SourceFundingCustodyLedgerV1> {
    require(
        custody_account.owner == program_id
            && custody_account.data_len() == SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES
            && custody_account.is_writable
            && !custody_account.is_signer
            && !custody_account.executable
            && principal_credit_lamports != 0
            && !close_receipt_id.is_zero(),
        ClutchError::MismatchedState,
    )?;
    let data = custody_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let before = SourceFundingCustodyLedgerV1::decode(&data).map_err(source_runtime)?;
    drop(data);
    require(
        before.adapter_program == runtime_key(program_id)
            && before.release_manifest_id == route.release_manifest_id()
            && before.route_id == route.route_id()
            && before.source_work_schedule_id == route.source_work_schedule_id()
            && before.custody_account == runtime_key(custody_account.key)
            && before.neutral_sink == route.neutral_sink(),
        ClutchError::MismatchedState,
    )?;
    let after = before
        .transition(
            0,
            principal_credit_lamports,
            custody_account.lamports(),
            close_receipt_id,
        )
        .map_err(source_runtime)?;
    write_exact_account_data(
        custody_account,
        &after.encode().map_err(source_runtime)?,
    )?;
    Ok(after)
}

fn rent_sysvar_id(rent_sysvar: &AccountInfo<'_>) -> Outcome<ContentId> {
    let data = rent_sysvar
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    account_data_id(runtime_key(rent_sysvar.key), &data).map_err(source_runtime)
}

fn debit_all(account: &AccountInfo<'_>) -> Outcome<()> {
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = 0;
    Ok(())
}

fn debit_lamports(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = (**lamports)
        .checked_sub(amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

fn credit_lamports(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = (**lamports)
        .checked_add(amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

fn source_runtime(error: clutch_source_plane_v3_runtime::Error) -> Refusal {
    Refusal::from(SourceV3SbfError::Runtime(error))
}

fn source_core(error: clutch_source_plane_v3::Error) -> Refusal {
    source_runtime(clutch_source_plane_v3_runtime::Error::Core(error))
}

fn source_pda(error: clutch_source_plane_v3_adapter::Error) -> Refusal {
    Refusal::from(SourceV3SbfError::Pda(error))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn key(byte: u8) -> RuntimeKey {
        RuntimeKey::from_bytes([byte; 32])
    }

    fn schedule() -> SourceWorkScheduleBindingV1 {
        SourceWorkScheduleBindingV1::new(
            id(1),
            id(2),
            key(3),
            key(4),
            key(5),
            key(6),
            1,
            8,
            10,
            80,
            1,
            [1; 8],
            [10; 8],
            [1; 4],
            [10; 4],
        )
        .unwrap()
    }

    #[test]
    fn lifecycle_quote_reserves_every_child_in_addition_to_work_capital() {
        let rent = RentParameters {
            lamports_per_byte_year: 3_480,
            exemption_threshold: 2.0,
        };
        let value = quote_source_lifecycle_capitalization_v1(schedule(), &rent).unwrap();
        assert_eq!(value.liveness_work_lamports, 80);
        assert!(value.permanent_and_child_rent_lamports > 0);
        assert_eq!(
            value.total_lamports,
            value.liveness_work_lamports + value.permanent_and_child_rent_lamports
        );
        assert!(!value.id.is_zero());

        let hostile_rent = RentParameters {
            lamports_per_byte_year: 3_481,
            exemption_threshold: 2.0,
        };
        assert_ne!(
            quote_source_lifecycle_capitalization_v1(schedule(), &hostile_rent)
                .unwrap()
                .id,
            value.id
        );
    }
}
