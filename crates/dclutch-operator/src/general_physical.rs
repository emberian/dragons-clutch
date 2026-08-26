//! Chain-derived unsigned workflows for the General physical controller.
//!
//! These builders consume one same-slot finalized observation, hostile-decode
//! every General-owned account, reauthenticate the activated Trading release
//! and its Loader V3 deployment, and return unsigned instructions. They never
//! perform RPC, access keys, sign, or submit. Settlement builders independently
//! reconstruct the exact Claims and Custody child packets used onchain.

use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_ACCOUNT_MAX_BYTES_V1, CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};

use dclutch_core_contract::ContentId;
use dclutch_custody_contract::CustodyReplayV1;
use dclutch_economic_slice_kernel::{market_revision, position_owner, position_revision};
use dclutch_general_adapter_contract::{
    AggregateReplayContextV1, CandidateVerifierV1, ChildExecutionError, ExecutionContextV1,
    GENERAL_CANDIDATE_PDA_DOMAIN_V1, GENERAL_CERTIFICATE_PDA_DOMAIN_V1, GENERAL_PAGE_PDA_DOMAIN_V1,
    GENERAL_POLICY_PDA_DOMAIN_V1, GENERAL_SELECTION_PDA_DOMAIN_V1,
    GENERAL_SETTLEMENT_PDA_DOMAIN_V1, GENERAL_VERIFICATION_PDA_DOMAIN_V1, QuoteSurplusRouteV2,
    RowReplayContextV1, SettlementChildrenV1, VERIFICATION_CURSOR_BYTES_V1,
    VERIFIED_CANDIDATE_BYTES_V1, VerifiedCandidateV1,
    child_packets::{
        ClaimsPacketV2, ClaimsResourcesV2, CustodyPacketV2, CustodyResourcesV2,
        build_materialize_packets_v2, build_row_packets_v2, build_surplus_packet_v2,
    },
    close, collect_execution, consider_verified, distribute_execution, freeze_selection,
    initialize_settlement, materialize,
};
use dclutch_general_codec::{
    Action, CANDIDATE_BYTES, CandidateV1, ControllerRequestV1, PAGE_BYTES, PageViewV1,
    SELECTION_CURSOR_BYTES, SELECTION_POLICY_BYTES, SETTLEMENT_CURSOR_BYTES, SelectionCursorV1,
    SelectionPolicyV1, SettlementCursorV1,
};
use dclutch_general_config_contract::{GENERAL_CONFIG_SCHEMA_ID_V2, GeneralConfigV2};
use dclutch_record_contract::RAW_RECORD_PDA_SEED_V1;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_hash::Hash;
use solana_message::{VersionedMessage, v0};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::{
    Finality, Observation, ObservedAccount,
    registry::{
        RegistryReauthenticationState, TRANSACTION_COMPUTE_UNIT_LIMIT_V1,
        build_registry_reauthentication_v1,
    },
    versioned::PACKET_DATA_BYTES,
};

/// Exact account count for streamed candidate consideration.
pub const GENERAL_CONSIDER_ACCOUNT_COUNT_V1: usize = 12;
/// Exact account count for freezing the best valid submitted candidate.
pub const GENERAL_FREEZE_ACCOUNT_COUNT_V1: usize = 6;
/// Exact account count for settlement-cursor initialization.
pub const GENERAL_INITIALIZE_ACCOUNT_COUNT_V1: usize = 9;
/// Exact account count for every physical settlement continuation.
pub const GENERAL_SETTLEMENT_ACCOUNT_COUNT_V1: usize = 28;

/// Finalized common authority observations for every General physical route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralCommonStateV1 {
    /// Readonly Market identity used in every market-scoped General PDA.
    pub market: ObservedAccount,
    /// Registry cache and current Trading Loader V3 deployment.
    pub trading_release: RegistryReauthenticationState,
}

/// Same-finalized observations for one streamed candidate page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralConsiderStateV1 {
    /// Common Market and activated Trading authority.
    pub common: GeneralCommonStateV1,
    /// Writable batch selection cursor, either canonical bytes or all-zero initial storage.
    pub selection: ObservedAccount,
    /// Writable candidate verification cursor, either canonical bytes or all-zero initial storage.
    pub verification: ObservedAccount,
    /// Writable all-zero destination for the candidate certificate.
    pub certificate: ObservedAccount,
    /// Immutable candidate header.
    pub candidate: ObservedAccount,
    /// Immutable candidate-selection policy.
    pub policy: ObservedAccount,
    /// Immutable next page selected by the verification cursor.
    pub page: ObservedAccount,
    /// Incumbent certificate, or an exact alias of `common.market` when selection is empty.
    pub incumbent_certificate: ObservedAccount,
}

/// Same-finalized observations for freezing one selection cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralFreezeStateV1 {
    /// Common Market and activated Trading authority.
    pub common: GeneralCommonStateV1,
    /// Writable open, nonempty selection cursor.
    pub selection: ObservedAccount,
}

/// Same-finalized observations for initializing settlement state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralInitializeStateV1 {
    /// Common Market and activated Trading authority.
    pub common: GeneralCommonStateV1,
    /// Readonly frozen selection cursor.
    pub selection: ObservedAccount,
    /// Writable all-zero settlement cursor destination.
    pub settlement: ObservedAccount,
    /// Readonly certificate of the selected candidate.
    pub certificate: ObservedAccount,
    /// Readonly immutable selected candidate header.
    pub candidate: ObservedAccount,
}

/// Same-finalized observations for one two-pass physical settlement action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralSettlementStateV1 {
    /// Common Market and activated Trading authority.
    pub common: GeneralCommonStateV1,
    /// Readonly composite Trading root carrying the immutable selected config.
    pub general_root: ObservedAccount,
    /// Current Core role release admission and deployment observations.
    pub core_release: RegistryReauthenticationState,
    /// Current Claims role release admission and deployment observations.
    pub claims_release: RegistryReauthenticationState,
    /// Current Custody role release admission and deployment observations.
    pub custody_release: RegistryReauthenticationState,
    /// Packet-derived Trading-to-Claims caller-authority PDA, or Claims sentinel.
    pub claims_caller_authority: ObservedAccount,
    /// Packet-derived Trading-to-Custody caller-authority PDA, or Custody sentinel.
    pub custody_caller_authority: ObservedAccount,
    /// Writable canonical settlement cursor.
    pub settlement: ObservedAccount,
    /// Program-derived selected-candidate certificate.
    pub certificate: ObservedAccount,
    /// Immutable selected candidate header.
    pub candidate: ObservedAccount,
    /// Exact next immutable page, or the Market sentinel for aggregate actions.
    pub page: ObservedAccount,
    /// Canonical Claims Market state.
    pub claims_market: ObservedAccount,
    /// Row-owner Claims Position, or a distinct inert sentinel when unused.
    pub owner_position: ObservedAccount,
    /// General settlement Claims Position.
    pub settlement_position: ObservedAccount,
    /// Immutable Realm/collateral binding.
    pub realm: ObservedAccount,
    /// Canonical vacant Realm staging PDA authenticated by Custody.
    pub realm_staging: ObservedAccount,
    /// Candidate-scoped Custody replay state.
    pub custody_replay: ObservedAccount,
    /// Realm-selected collateral Mint.
    pub mint: ObservedAccount,
    /// Source collateral account for the selected effect.
    pub collateral_source: ObservedAccount,
    /// Destination collateral account for the selected effect.
    pub collateral_destination: ObservedAccount,
    /// Canonical Custody transfer-authority PDA.
    pub custody_authority: ObservedAccount,
    /// Realm-selected Token or Token-2022 program.
    pub token_program: ObservedAccount,
    /// Selected immutable raw GeneralConfigV2 record.
    pub general_config: ObservedAccount,
}

/// Compute-relevant facts authenticated for one General action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralComputeEvidenceV1 {
    /// Complete Trading ELF-tail bytes authenticated by Registry reauthentication.
    pub trading_elf_bytes_hashed: usize,
    /// Matching measured compute cost, absent until an exact General ELF/profile is measured.
    pub matching_measured_compute_units: Option<u32>,
}

/// Fully checked unsigned General instruction and its chain authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralActionReportV1 {
    /// Exact unsigned General instruction.
    pub instruction: Instruction,
    /// General action selected by the instruction bytes and account frame.
    pub action: Action,
    /// Shared finalized observation selecting every input.
    pub observation: Observation,
    /// Registry-authenticated execution-release-set identity.
    pub execution_release_set_id: [u8; 32],
    /// Candidate identity for Consider/Initialize, absent for Freeze.
    pub candidate_id: Option<[u8; 32]>,
    /// Optimistic revision encoded in the canonical request.
    pub expected_revision: u64,
    /// Compute-relevant authenticated evidence.
    pub compute: GeneralComputeEvidenceV1,
}

/// Unsigned packet-safe v0 plan with an explicit compute limit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralPacketPlanV0 {
    /// Unsigned v0 message containing Compute Budget then General action.
    pub message: VersionedMessage,
    /// Exact number of final transaction signature slots.
    pub required_signatures: u8,
    /// Fully signed serialized transaction bytes.
    pub wire_bytes: usize,
    /// Explicit transaction compute-unit limit encoded in the message.
    pub compute_unit_limit: u32,
    /// Matching measured cost, absent until an exact General ELF/profile is measured.
    pub matching_measured_compute_units: Option<u32>,
}

/// Refusal from stale, substituted, aliased, malformed, or oversized inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// At least one observation was not finalized.
    ObservationNotFinalized,
    /// Inputs were not observed at one exact slot/time/finality.
    ObservationMismatch,
    /// Equal keys carried conflicting facts or an unapproved semantic alias occurred.
    AccountAlias,
    /// Registry activation or current Trading Loader V3 deployment refused.
    ReleaseAdmission,
    /// Market account shape refused.
    Market,
    /// General-owned account owner, executable bit, or exact width refused.
    AccountShape,
    /// Candidate, page, or policy bytes/bindings refused.
    ImmutableInput,
    /// Selection bytes, PDA, state, or revision refused.
    Selection,
    /// Verification bytes, PDA, page coordinate, or revision refused.
    Verification,
    /// Certificate bytes, PDA, incumbent binding, or destination vacancy refused.
    Certificate,
    /// Settlement bytes, PDA, or initialization state refused.
    Settlement,
    /// Canonical request encoding or account-frame construction refused.
    Encoding,
    /// Fee payer aliased a semantic account.
    FeePayerAlias,
    /// Compute limit was zero, above the chain profile, or below a matching measurement.
    InvalidComputeLimit,
    /// Fully signed serialized transaction exceeds the current packet limit.
    PacketTooLarge,
}

/// Build one exact 12-account streamed Consider instruction.
pub fn build_general_consider_v1(
    state: &GeneralConsiderStateV1,
) -> Result<GeneralActionReportV1, Error> {
    let accounts = [
        &state.common.market,
        &state.common.trading_release.cache,
        &state.common.trading_release.registry_program,
        &state.common.trading_release.role_program,
        &state.common.trading_release.role_programdata,
        &state.selection,
        &state.verification,
        &state.certificate,
        &state.candidate,
        &state.policy,
        &state.page,
        &state.incumbent_certificate,
    ];
    let observation = same_observation(&accounts)?;
    authenticate_aliases(&accounts, Some((0, 11)))?;
    let authority = authenticate_common(&state.common)?;
    let program_id = authority.program_id;
    require_owned(&state.selection, program_id, SELECTION_CURSOR_BYTES)?;
    require_owned(
        &state.verification,
        program_id,
        VERIFICATION_CURSOR_BYTES_V1,
    )?;
    require_owned(&state.certificate, program_id, VERIFIED_CANDIDATE_BYTES_V1)?;
    require_owned(&state.candidate, program_id, CANDIDATE_BYTES)?;
    require_owned(&state.policy, program_id, SELECTION_POLICY_BYTES)?;
    require_owned(&state.page, program_id, PAGE_BYTES)?;

    let candidate =
        CandidateV1::decode(&state.candidate.data).map_err(|_| Error::ImmutableInput)?;
    let policy =
        SelectionPolicyV1::decode(&state.policy.data).map_err(|_| Error::ImmutableInput)?;
    let page = PageViewV1::decode(&state.page.data).map_err(|_| Error::ImmutableInput)?;
    if page.candidate_id() != candidate.candidate_id
        || page.outcome_count() != candidate.outcome_count
        || page.page_count() != candidate.page_count
    {
        return Err(Error::ImmutableInput);
    }
    require_pda(
        program_id,
        state.common.market.key,
        state.candidate.key,
        &[GENERAL_CANDIDATE_PDA_DOMAIN_V1, &candidate.candidate_id],
        Error::ImmutableInput,
    )?;
    require_pda(
        program_id,
        state.common.market.key,
        state.policy.key,
        &[GENERAL_POLICY_PDA_DOMAIN_V1, &policy.policy_id],
        Error::ImmutableInput,
    )?;
    let page_seed = page.page_index().to_le_bytes();
    require_pda(
        program_id,
        state.common.market.key,
        state.page.key,
        &[
            GENERAL_PAGE_PDA_DOMAIN_V1,
            &candidate.candidate_id,
            &page_seed,
        ],
        Error::ImmutableInput,
    )?;
    require_pda(
        program_id,
        state.common.market.key,
        state.selection.key,
        &[GENERAL_SELECTION_PDA_DOMAIN_V1, &candidate.batch_id],
        Error::Selection,
    )?;
    require_pda(
        program_id,
        state.common.market.key,
        state.verification.key,
        &[GENERAL_VERIFICATION_PDA_DOMAIN_V1, &candidate.candidate_id],
        Error::Verification,
    )?;
    require_pda(
        program_id,
        state.common.market.key,
        state.certificate.key,
        &[GENERAL_CERTIFICATE_PDA_DOMAIN_V1, &candidate.candidate_id],
        Error::Certificate,
    )?;

    let mut verifier = if is_zero(&state.verification.data) {
        CandidateVerifierV1::begin(candidate)
    } else {
        let value = CandidateVerifierV1::decode(&state.verification.data)
            .map_err(|_| Error::Verification)?;
        if value.candidate() != candidate {
            return Err(Error::Verification);
        }
        value
    };
    if verifier.is_complete() || verifier.next_page() != page.page_index() {
        return Err(Error::Verification);
    }
    let expected_revision = verifier.revision();
    verifier
        .ingest_page_at(&state.page.data, expected_revision)
        .map_err(|_| Error::Verification)?;

    let (selection_revision, incumbent) =
        authenticate_consider_selection(state, program_id, candidate, policy)?;
    if !is_zero(&state.certificate.data) {
        return Err(Error::Certificate);
    }
    if verifier.is_complete() {
        let verified = verifier.finish().map_err(|_| Error::Verification)?;
        let mut selection = state.selection.data.clone();
        let mut certificate = state.certificate.data.clone();
        consider_verified(
            &mut selection,
            &mut certificate,
            &candidate,
            &policy,
            verified,
            incumbent.as_ref(),
            selection_revision,
        )
        .map_err(|_| Error::Selection)?;
    }

    let request = ControllerRequestV1 {
        action: Action::Consider,
        expected_revision,
        candidate_id: Some(candidate.candidate_id),
        page_index: page.page_index(),
        execution_index: 0,
    };
    let instruction = Instruction {
        program_id,
        accounts: vec![
            readonly(state.common.market.key),
            readonly(state.common.trading_release.cache.key),
            readonly(state.common.trading_release.registry_program.key),
            readonly(state.common.trading_release.role_program.key),
            readonly(state.common.trading_release.role_programdata.key),
            writable(state.selection.key),
            writable(state.verification.key),
            writable(state.certificate.key),
            readonly(state.candidate.key),
            readonly(state.policy.key),
            readonly(state.page.key),
            readonly(state.incumbent_certificate.key),
        ],
        data: request.to_bytes().map_err(|_| Error::Encoding)?.to_vec(),
    };
    report(
        instruction,
        Action::Consider,
        observation,
        authority,
        Some(candidate.candidate_id),
        expected_revision,
        GENERAL_CONSIDER_ACCOUNT_COUNT_V1,
    )
}

/// Build one exact six-account Freeze instruction.
pub fn build_general_freeze_v1(
    state: &GeneralFreezeStateV1,
) -> Result<GeneralActionReportV1, Error> {
    let accounts = [
        &state.common.market,
        &state.common.trading_release.cache,
        &state.common.trading_release.registry_program,
        &state.common.trading_release.role_program,
        &state.common.trading_release.role_programdata,
        &state.selection,
    ];
    let observation = same_observation(&accounts)?;
    authenticate_aliases(&accounts, None)?;
    let authority = authenticate_common(&state.common)?;
    let program_id = authority.program_id;
    require_owned(&state.selection, program_id, SELECTION_CURSOR_BYTES)?;
    let selection =
        SelectionCursorV1::decode(&state.selection.data).map_err(|_| Error::Selection)?;
    require_pda(
        program_id,
        state.common.market.key,
        state.selection.key,
        &[GENERAL_SELECTION_PDA_DOMAIN_V1, &selection.batch_id],
        Error::Selection,
    )?;
    let expected_revision = selection.revision;
    let mut staged = state.selection.data.clone();
    freeze_selection(&mut staged, expected_revision).map_err(|_| Error::Selection)?;
    let request = ControllerRequestV1 {
        action: Action::Freeze,
        expected_revision,
        candidate_id: None,
        page_index: 0,
        execution_index: 0,
    };
    let instruction = Instruction {
        program_id,
        accounts: vec![
            readonly(state.common.market.key),
            readonly(state.common.trading_release.cache.key),
            readonly(state.common.trading_release.registry_program.key),
            readonly(state.common.trading_release.role_program.key),
            readonly(state.common.trading_release.role_programdata.key),
            writable(state.selection.key),
        ],
        data: request.to_bytes().map_err(|_| Error::Encoding)?.to_vec(),
    };
    report(
        instruction,
        Action::Freeze,
        observation,
        authority,
        None,
        expected_revision,
        GENERAL_FREEZE_ACCOUNT_COUNT_V1,
    )
}

/// Build one exact nine-account InitializeSettlement instruction.
pub fn build_general_initialize_v1(
    state: &GeneralInitializeStateV1,
) -> Result<GeneralActionReportV1, Error> {
    let accounts = [
        &state.common.market,
        &state.common.trading_release.cache,
        &state.common.trading_release.registry_program,
        &state.common.trading_release.role_program,
        &state.common.trading_release.role_programdata,
        &state.selection,
        &state.settlement,
        &state.certificate,
        &state.candidate,
    ];
    let observation = same_observation(&accounts)?;
    authenticate_aliases(&accounts, None)?;
    let authority = authenticate_common(&state.common)?;
    let program_id = authority.program_id;
    require_owned(&state.selection, program_id, SELECTION_CURSOR_BYTES)?;
    require_owned(&state.settlement, program_id, SETTLEMENT_CURSOR_BYTES)?;
    require_owned(&state.certificate, program_id, VERIFIED_CANDIDATE_BYTES_V1)?;
    require_owned(&state.candidate, program_id, CANDIDATE_BYTES)?;
    let selection =
        SelectionCursorV1::decode(&state.selection.data).map_err(|_| Error::Selection)?;
    let candidate =
        CandidateV1::decode(&state.candidate.data).map_err(|_| Error::ImmutableInput)?;
    let verified =
        VerifiedCandidateV1::decode(&state.certificate.data).map_err(|_| Error::Certificate)?;
    if verified.candidate_id != candidate.candidate_id
        || verified.product_id != candidate.product_id
        || verified.batch_id != candidate.batch_id
        || verified.outcome_count != candidate.outcome_count
        || verified.page_count != candidate.page_count
    {
        return Err(Error::Certificate);
    }
    if !selection.closed
        || selection.best_candidate_id != Some(candidate.candidate_id)
        || selection.batch_id != candidate.batch_id
    {
        return Err(Error::Selection);
    }
    require_pda(
        program_id,
        state.common.market.key,
        state.selection.key,
        &[GENERAL_SELECTION_PDA_DOMAIN_V1, &candidate.batch_id],
        Error::Selection,
    )?;
    require_pda(
        program_id,
        state.common.market.key,
        state.candidate.key,
        &[GENERAL_CANDIDATE_PDA_DOMAIN_V1, &candidate.candidate_id],
        Error::ImmutableInput,
    )?;
    require_pda(
        program_id,
        state.common.market.key,
        state.certificate.key,
        &[GENERAL_CERTIFICATE_PDA_DOMAIN_V1, &candidate.candidate_id],
        Error::Certificate,
    )?;
    require_pda(
        program_id,
        state.common.market.key,
        state.settlement.key,
        &[GENERAL_SETTLEMENT_PDA_DOMAIN_V1, &candidate.candidate_id],
        Error::Settlement,
    )?;
    if !is_zero(&state.settlement.data) {
        return Err(Error::Settlement);
    }
    let mut staged = state.settlement.data.clone();
    initialize_settlement(&mut staged, &state.selection.data, &verified, 0)
        .map_err(|_| Error::Settlement)?;
    let request = ControllerRequestV1 {
        action: Action::InitializeSettlement,
        expected_revision: 0,
        candidate_id: Some(candidate.candidate_id),
        page_index: 0,
        execution_index: 0,
    };
    let instruction = Instruction {
        program_id,
        accounts: vec![
            readonly(state.common.market.key),
            readonly(state.common.trading_release.cache.key),
            readonly(state.common.trading_release.registry_program.key),
            readonly(state.common.trading_release.role_program.key),
            readonly(state.common.trading_release.role_programdata.key),
            readonly(state.selection.key),
            writable(state.settlement.key),
            readonly(state.certificate.key),
            readonly(state.candidate.key),
        ],
        data: request.to_bytes().map_err(|_| Error::Encoding)?.to_vec(),
    };
    report(
        instruction,
        Action::InitializeSettlement,
        observation,
        authority,
        Some(candidate.candidate_id),
        0,
        GENERAL_INITIALIZE_ACCOUNT_COUNT_V1,
    )
}

/// Build one exact permissionless two-pass settlement continuation.
///
/// The builder hostile-decodes every General-owned fact, reauthenticates
/// Trading, Claims, and Custody from one finalized observation, dry-runs the
/// pure settlement transition, and emits no signer requirement. Child packet
/// bytes are derived by the onchain adapter from the same immutable page.
pub fn build_general_settlement_v1(
    state: &GeneralSettlementStateV1,
    action: Action,
) -> Result<GeneralActionReportV1, Error> {
    if !matches!(
        action,
        Action::Collect | Action::Materialize | Action::Distribute | Action::Close
    ) {
        return Err(Error::Encoding);
    }
    let accounts = settlement_observations(state);
    let observation = same_observation(&accounts)?;
    authenticate_settlement_aliases(&accounts, action)?;
    let authority = authenticate_common(&state.common)?;
    authenticate_child_release(&state.common, &state.core_release, ExecutionRoleV1::Core)?;
    authenticate_child_release(
        &state.common,
        &state.claims_release,
        ExecutionRoleV1::Claims,
    )?;
    authenticate_child_release(
        &state.common,
        &state.custody_release,
        ExecutionRoleV1::Custody,
    )?;
    let root = authenticate_general_root(state, authority)?;
    let program_id = authority.program_id;
    require_owned(&state.settlement, program_id, SETTLEMENT_CURSOR_BYTES)?;
    require_owned(&state.certificate, program_id, VERIFIED_CANDIDATE_BYTES_V1)?;
    require_owned(&state.candidate, program_id, CANDIDATE_BYTES)?;
    let cursor =
        SettlementCursorV1::decode(&state.settlement.data).map_err(|_| Error::Settlement)?;
    let candidate =
        CandidateV1::decode(&state.candidate.data).map_err(|_| Error::ImmutableInput)?;
    let verified =
        VerifiedCandidateV1::decode(&state.certificate.data).map_err(|_| Error::Certificate)?;
    require_candidate_certificate(candidate, verified)?;
    if cursor.candidate_id != candidate.candidate_id
        || cursor.outcome_count != candidate.outcome_count
        || cursor.page_count != candidate.page_count
    {
        return Err(Error::Settlement);
    }
    require_pda(
        program_id,
        state.common.market.key,
        state.settlement.key,
        &[GENERAL_SETTLEMENT_PDA_DOMAIN_V1, &candidate.candidate_id],
        Error::Settlement,
    )?;
    require_pda(
        program_id,
        state.common.market.key,
        state.candidate.key,
        &[GENERAL_CANDIDATE_PDA_DOMAIN_V1, &candidate.candidate_id],
        Error::ImmutableInput,
    )?;
    require_pda(
        program_id,
        state.common.market.key,
        state.certificate.key,
        &[GENERAL_CERTIFICATE_PDA_DOMAIN_V1, &candidate.candidate_id],
        Error::Certificate,
    )?;
    let page_bytes = if matches!(action, Action::Collect | Action::Distribute) {
        require_owned(&state.page, program_id, PAGE_BYTES)?;
        let page = PageViewV1::decode(&state.page.data).map_err(|_| Error::ImmutableInput)?;
        if page.candidate_id() != candidate.candidate_id
            || page.outcome_count() != candidate.outcome_count
            || page.page_count() != candidate.page_count
            || page.page_index() != cursor.next_page
        {
            return Err(Error::ImmutableInput);
        }
        let page_index = cursor.next_page.to_le_bytes();
        require_pda(
            program_id,
            state.common.market.key,
            state.page.key,
            &[
                GENERAL_PAGE_PDA_DOMAIN_V1,
                &candidate.candidate_id,
                &page_index,
            ],
            Error::ImmutableInput,
        )?;
        state.page.data.as_slice()
    } else {
        if state.page != state.common.market {
            return Err(Error::AccountAlias);
        }
        &[]
    };

    let expected_revision = cursor.revision;
    let context = ExecutionContextV1 {
        market_id: state.common.market.key.to_bytes(),
        release_set_id: authority.release_set_id,
    };
    let mut staged = state.settlement.data.clone();
    let config = authenticate_general_config(state, root)?;
    let mut children = OperatorChildren::new(state, verified, config, authority)?;
    match action {
        Action::Collect => collect_execution(
            &mut staged,
            context,
            &verified,
            page_bytes,
            expected_revision,
            &mut children,
        ),
        Action::Materialize => materialize(
            &mut staged,
            context,
            &verified,
            expected_revision,
            &mut children,
        ),
        Action::Distribute => distribute_execution(
            &mut staged,
            context,
            &verified,
            page_bytes,
            expected_revision,
            &mut children,
        ),
        Action::Close => close(
            &mut staged,
            context,
            &verified,
            expected_revision,
            &mut children,
        ),
        _ => return Err(Error::Encoding),
    }
    .map_err(|_| Error::Settlement)?;
    children.require_success()?;
    authenticate_packet_authorities(state, authority, children.claims, children.custody)?;

    let request = ControllerRequestV1 {
        action,
        expected_revision,
        candidate_id: Some(candidate.candidate_id),
        page_index: if matches!(action, Action::Collect | Action::Distribute) {
            cursor.next_page
        } else {
            0
        },
        execution_index: if matches!(action, Action::Collect | Action::Distribute) {
            cursor.next_execution
        } else {
            0
        },
    };
    let instruction = Instruction {
        program_id,
        accounts: general_settlement_suffix_metas(state, action),
        data: request.to_bytes().map_err(|_| Error::Encoding)?.to_vec(),
    };
    report(
        instruction,
        action,
        observation,
        authority,
        Some(candidate.candidate_id),
        expected_revision,
        GENERAL_SETTLEMENT_ACCOUNT_COUNT_V1,
    )
}

/// Compile an authenticated General action into a packet-safe unsigned v0 message.
pub fn compile_general_packet_v0(
    report: &GeneralActionReportV1,
    fee_payer: Pubkey,
    recent_blockhash: Hash,
    compute_unit_limit: u32,
) -> Result<GeneralPacketPlanV0, Error> {
    if report
        .instruction
        .accounts
        .iter()
        .any(|account| account.pubkey == fee_payer)
        || report.instruction.program_id == fee_payer
    {
        return Err(Error::FeePayerAlias);
    }
    if compute_unit_limit == 0
        || compute_unit_limit > TRANSACTION_COMPUTE_UNIT_LIMIT_V1
        || report
            .compute
            .matching_measured_compute_units
            .is_some_and(|measured| compute_unit_limit < measured)
    {
        return Err(Error::InvalidComputeLimit);
    }
    let message = v0::Message::try_compile(
        &fee_payer,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(compute_unit_limit),
            report.instruction.clone(),
        ],
        &[],
        recent_blockhash,
    )
    .map_err(|_| Error::Encoding)?;
    let required_signatures = message.header.num_required_signatures;
    let signature_count = usize::from(required_signatures);
    let wire_bytes = short_vec_prefix_bytes(signature_count)
        .checked_add(signature_count.checked_mul(64).ok_or(Error::Encoding)?)
        .and_then(|value| value.checked_add(message.serialize().len()))
        .ok_or(Error::Encoding)?;
    if wire_bytes > PACKET_DATA_BYTES {
        return Err(Error::PacketTooLarge);
    }
    Ok(GeneralPacketPlanV0 {
        message: VersionedMessage::V0(message),
        required_signatures,
        wire_bytes,
        compute_unit_limit,
        matching_measured_compute_units: report.compute.matching_measured_compute_units,
    })
}

#[derive(Clone, Copy)]
struct AuthenticatedCommon {
    program_id: Pubkey,
    release_set_id: [u8; 32],
    elf_bytes: usize,
}

fn authenticate_common(state: &GeneralCommonStateV1) -> Result<AuthenticatedCommon, Error> {
    if state.market.executable {
        return Err(Error::Market);
    }
    let authority =
        build_registry_reauthentication_v1(&state.trading_release, ExecutionRoleV1::Trading)
            .map_err(|_| Error::ReleaseAdmission)?;
    Ok(AuthenticatedCommon {
        program_id: authority.role_program,
        release_set_id: authority.execution_release_set_id.to_bytes(),
        elf_bytes: authority.compute.elf_bytes_hashed,
    })
}

fn authenticate_child_release(
    common: &GeneralCommonStateV1,
    child: &RegistryReauthenticationState,
    role: ExecutionRoleV1,
) -> Result<(), Error> {
    if child.registry_program != common.trading_release.registry_program
        || child.cache != common.trading_release.cache
    {
        return Err(Error::ReleaseAdmission);
    }
    let authority =
        build_registry_reauthentication_v1(child, role).map_err(|_| Error::ReleaseAdmission)?;
    let trading =
        build_registry_reauthentication_v1(&common.trading_release, ExecutionRoleV1::Trading)
            .map_err(|_| Error::ReleaseAdmission)?;
    if authority.execution_release_set_id != trading.execution_release_set_id {
        return Err(Error::ReleaseAdmission);
    }
    Ok(())
}

fn require_candidate_certificate(
    candidate: CandidateV1,
    verified: VerifiedCandidateV1,
) -> Result<(), Error> {
    if verified.candidate_id != candidate.candidate_id
        || verified.product_id != candidate.product_id
        || verified.batch_id != candidate.batch_id
        || verified.outcome_count != candidate.outcome_count
        || verified.page_count != candidate.page_count
    {
        Err(Error::Certificate)
    } else {
        Ok(())
    }
}

fn authenticate_general_root(
    state: &GeneralSettlementStateV1,
    authority: AuthenticatedCommon,
) -> Result<CapabilityRootHeaderV1, Error> {
    if state.general_root.owner != authority.program_id
        || state.general_root.executable
        || state.general_root.data.len() <= CAPABILITY_ROOT_HEADER_BYTES_V1
        || state.general_root.data.len() > CAPABILITY_ROOT_ACCOUNT_MAX_BYTES_V1
    {
        return Err(Error::AccountShape);
    }
    let header = CapabilityRootHeaderV1::decode(
        state
            .general_root
            .data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(Error::AccountShape)?,
    )
    .map_err(|_| Error::AccountShape)?;
    let release_set =
        ContentId::new(authority.release_set_id).map_err(|_| Error::ReleaseAdmission)?;
    let seeds = header.seeds();
    let expected = Pubkey::find_program_address(&seeds.as_slices(), &authority.program_id).0;
    if state.general_root.key != expected
        || header.market() != state.common.market.key.to_bytes()
        || header.release_set() != release_set
    {
        return Err(Error::AccountShape);
    }
    Ok(header)
}

fn authenticate_general_config(
    state: &GeneralSettlementStateV1,
    root: CapabilityRootHeaderV1,
) -> Result<GeneralConfigV2, Error> {
    if state.general_config.owner != state.common.trading_release.registry_program.key
        || state.general_config.executable
    {
        return Err(Error::AccountShape);
    }
    let digest = hash(&state.general_config.data).to_bytes();
    let expected = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &GENERAL_CONFIG_SCHEMA_ID_V2,
            &digest,
        ],
        &state.common.trading_release.registry_program.key,
    )
    .0;
    if state.general_config.key != expected || digest != root.selection().config().to_bytes() {
        return Err(Error::AccountShape);
    }
    let config =
        GeneralConfigV2::decode(&state.general_config.data).map_err(|_| Error::AccountShape)?;
    if config.generation() != root.generation() {
        return Err(Error::AccountShape);
    }
    Ok(config)
}

fn authenticate_packet_authorities(
    state: &GeneralSettlementStateV1,
    authority: AuthenticatedCommon,
    claims: Option<ClaimsPacketV2>,
    custody: Option<CustodyPacketV2>,
) -> Result<(), Error> {
    let (claims_expected, custody_expected) =
        packet_authority_keys(state, authority, claims, custody)?;
    if state.claims_caller_authority.key != claims_expected
        || state.custody_caller_authority.key != custody_expected
        || (claims.is_some() && state.claims_caller_authority.executable)
        || (custody.is_some() && state.custody_caller_authority.executable)
        || (claims.is_some()
            && custody.is_some()
            && state.claims_caller_authority.key == state.custody_caller_authority.key)
    {
        return Err(Error::AccountShape);
    }
    Ok(())
}

fn packet_authority_keys(
    state: &GeneralSettlementStateV1,
    authority: AuthenticatedCommon,
    claims: Option<ClaimsPacketV2>,
    custody: Option<CustodyPacketV2>,
) -> Result<(Pubkey, Pubkey), Error> {
    let release = ContentId::new(authority.release_set_id).map_err(|_| Error::ReleaseAdmission)?;
    let claims_expected = match claims {
        Some(packet) => {
            let plan = packet.plan().map_err(|_| Error::Settlement)?;
            let seeds = CallerAuthoritySeedsV1::new(
                release,
                state.common.market.key.to_bytes(),
                ExecutionRoleV1::Trading,
                plan.request_id(),
                packet.digest(),
            )
            .map_err(|_| Error::Settlement)?;
            Pubkey::find_program_address(&seeds.as_slices(), &authority.program_id).0
        }
        None => state.claims_release.role_program.key,
    };
    let custody_expected = match custody {
        Some(packet) => {
            let request = packet.request();
            let seeds = CallerAuthoritySeedsV1::new(
                release,
                state.common.market.key.to_bytes(),
                ExecutionRoleV1::Trading,
                request.context,
                packet.digest(),
            )
            .map_err(|_| Error::Settlement)?;
            Pubkey::find_program_address(&seeds.as_slices(), &authority.program_id).0
        }
        None => state.custody_release.role_program.key,
    };
    Ok((claims_expected, custody_expected))
}

fn settlement_observations(state: &GeneralSettlementStateV1) -> [&ObservedAccount; 30] {
    [
        &state.common.market,
        &state.common.trading_release.cache,
        &state.common.trading_release.registry_program,
        &state.common.trading_release.role_program,
        &state.common.trading_release.role_programdata,
        &state.core_release.role_program,
        &state.core_release.role_programdata,
        &state.claims_release.role_program,
        &state.claims_release.role_programdata,
        &state.custody_release.role_program,
        &state.custody_release.role_programdata,
        &state.claims_caller_authority,
        &state.custody_caller_authority,
        &state.settlement,
        &state.certificate,
        &state.candidate,
        &state.page,
        &state.claims_market,
        &state.owner_position,
        &state.settlement_position,
        &state.realm,
        &state.realm_staging,
        &state.custody_replay,
        &state.mint,
        &state.collateral_source,
        &state.collateral_destination,
        &state.custody_authority,
        &state.token_program,
        &state.general_config,
        &state.general_root,
    ]
}

fn authenticate_settlement_aliases(
    accounts: &[&ObservedAccount; 30],
    action: Action,
) -> Result<(), Error> {
    let _ = action;
    for (left_index, left) in accounts.iter().enumerate() {
        for (right_index, right) in accounts.iter().enumerate().skip(left_index + 1) {
            if left.key != right.key {
                continue;
            }
            let permitted_shared_fact = left == right
                && ((left_index <= 10 && right_index <= 10)
                    || left.executable
                    || (left_index == 0 && right_index == 16)
                    || (left_index == 7 && [11, 18].contains(&right_index))
                    || (left_index == 9 && right_index == 12));
            if !permitted_shared_fact {
                return Err(Error::AccountAlias);
            }
        }
    }
    Ok(())
}

/// Return the exact General-family suffix consumed after the future common
/// authenticated hot prefix. This function deliberately does not invent that
/// prefix while its CapabilityProgram V3 codec remains under construction.
fn general_settlement_suffix_metas(
    state: &GeneralSettlementStateV1,
    action: Action,
) -> Vec<AccountMeta> {
    vec![
        readonly(state.common.market.key),
        readonly(state.common.trading_release.cache.key),
        readonly(state.common.trading_release.registry_program.key),
        readonly(state.common.trading_release.role_program.key),
        readonly(state.common.trading_release.role_programdata.key),
        readonly(state.core_release.role_program.key),
        readonly(state.core_release.role_programdata.key),
        readonly(state.claims_release.role_program.key),
        readonly(state.claims_release.role_programdata.key),
        readonly(state.custody_release.role_program.key),
        readonly(state.claims_caller_authority.key),
        readonly(state.custody_caller_authority.key),
        writable(state.settlement.key),
        readonly(state.certificate.key),
        readonly(state.candidate.key),
        readonly(state.page.key),
        writable(state.claims_market.key),
        if matches!(action, Action::Collect | Action::Distribute) {
            writable(state.owner_position.key)
        } else {
            readonly(state.owner_position.key)
        },
        writable(state.settlement_position.key),
        readonly(state.realm.key),
        readonly(state.realm_staging.key),
        writable(state.custody_replay.key),
        readonly(state.mint.key),
        writable(state.collateral_source.key),
        writable(state.collateral_destination.key),
        readonly(state.custody_authority.key),
        readonly(state.token_program.key),
        readonly(state.general_config.key),
    ]
}

struct OperatorChildren<'a> {
    state: &'a GeneralSettlementStateV1,
    verified: VerifiedCandidateV1,
    config: GeneralConfigV2,
    authority: AuthenticatedCommon,
    claims: Option<ClaimsPacketV2>,
    custody: Option<CustodyPacketV2>,
    failed: bool,
}

impl<'a> OperatorChildren<'a> {
    fn new(
        state: &'a GeneralSettlementStateV1,
        verified: VerifiedCandidateV1,
        config: GeneralConfigV2,
        authority: AuthenticatedCommon,
    ) -> Result<Self, Error> {
        if config.generation()
            != CustodyReplayV1::decode(&state.custody_replay.data)
                .map_err(|_| Error::AccountShape)?
                .generation
        {
            return Err(Error::AccountShape);
        }
        Ok(Self {
            state,
            verified,
            config,
            authority,
            claims: None,
            custody: None,
            failed: false,
        })
    }

    fn claims_resources(&self, row_owner: Option<[u8; 32]>) -> Result<ClaimsResourcesV2, Error> {
        let count = u32::from(self.verified.outcome_count);
        let market_revision =
            market_revision(&self.state.claims_market.data).map_err(|_| Error::AccountShape)?;
        let settlement_owner = position_owner(&self.state.settlement_position.data, count)
            .map_err(|_| Error::AccountShape)?;
        let settlement_position_revision =
            position_revision(&self.state.settlement_position.data, count)
                .map_err(|_| Error::AccountShape)?;
        let owner_position_revision = match row_owner {
            Some(expected)
                if position_owner(&self.state.owner_position.data, count)
                    .map_err(|_| Error::AccountShape)?
                    == expected =>
            {
                position_revision(&self.state.owner_position.data, count)
                    .map_err(|_| Error::AccountShape)?
            }
            Some(_) => return Err(Error::AccountShape),
            None => 0,
        };
        Ok(ClaimsResourcesV2 {
            settlement_owner,
            market_revision,
            owner_position_revision,
            settlement_position_revision,
        })
    }

    fn custody_resources(
        &self,
        source_owner: [u8; 32],
        destination_owner: [u8; 32],
        source_vault_context: [u8; 32],
        destination_vault_context: [u8; 32],
    ) -> Result<CustodyResourcesV2, Error> {
        let replay = CustodyReplayV1::decode(&self.state.custody_replay.data)
            .map_err(|_| Error::AccountShape)?;
        if replay.release_set != self.authority.release_set_id
            || replay.market != self.state.common.market.key.to_bytes()
            || replay.context != self.verified.candidate_id
            || replay.caller_program != self.authority.program_id.to_bytes()
        {
            return Err(Error::AccountShape);
        }
        Ok(CustodyResourcesV2 {
            realm: replay.realm,
            trading_program: self.authority.program_id.to_bytes(),
            generation: replay.generation,
            source: self.state.collateral_source.key.to_bytes(),
            destination: self.state.collateral_destination.key.to_bytes(),
            source_owner,
            destination_owner,
            source_vault_context,
            destination_vault_context,
            mint: self.state.mint.key.to_bytes(),
            token_program: self.state.token_program.key.to_bytes(),
            replay_revision: replay.next_revision,
            transfer_index: 0,
        })
    }

    fn record(
        &mut self,
        packets: core::result::Result<
            dclutch_general_adapter_contract::child_packets::GeneralChildPacketsV2,
            dclutch_general_adapter_contract::child_packets::ChildPacketError,
        >,
    ) -> core::result::Result<(), ChildExecutionError> {
        let packets = match packets {
            Ok(value) => value,
            Err(_) => {
                self.failed = true;
                return Err(ChildExecutionError::Refused);
            }
        };
        if (packets.claims.is_some() && self.claims.is_some())
            || (packets.custody.is_some() && self.custody.is_some())
        {
            self.failed = true;
            return Err(ChildExecutionError::Refused);
        }
        self.claims = packets.claims.or(self.claims);
        self.custody = packets.custody.or(self.custody);
        Ok(())
    }

    fn require_success(&self) -> Result<(), Error> {
        if self.failed {
            Err(Error::Settlement)
        } else {
            Ok(())
        }
    }
}

impl SettlementChildrenV1 for OperatorChildren<'_> {
    fn collect_claims(
        &mut self,
        context: RowReplayContextV1,
        outcome_count: u8,
        quantities: &[u64; dclutch_general_codec::MAX_OUTCOMES],
    ) -> core::result::Result<(), ChildExecutionError> {
        let claims = self.claims_resources(Some(context.owner_id)).map_err(|_| {
            self.failed = true;
            ChildExecutionError::Refused
        })?;
        self.record(build_row_packets_v2(
            dclutch_general_adapter_contract::GeneralChildEffectV1::CollectClaims,
            context,
            outcome_count,
            quantities,
            claims,
            None,
        ))
    }

    fn collect_collateral(
        &mut self,
        context: RowReplayContextV1,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        let mut quantities = [0; dclutch_general_codec::MAX_OUTCOMES];
        quantities[0] = quantity;
        let custody = self
            .custody_resources(context.owner_id, [0; 32], [0; 32], context.candidate_id)
            .map_err(|_| {
                self.failed = true;
                ChildExecutionError::Refused
            })?;
        self.record(build_row_packets_v2(
            dclutch_general_adapter_contract::GeneralChildEffectV1::CollectCollateral,
            context,
            1,
            &quantities,
            ClaimsResourcesV2 {
                settlement_owner: context.candidate_id,
                market_revision: 0,
                owner_position_revision: 0,
                settlement_position_revision: 0,
            },
            Some(custody),
        ))
    }

    fn mint_complete_set(
        &mut self,
        context: AggregateReplayContextV1,
        outcome_count: u8,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        let claims = self.claims_resources(None).map_err(|_| {
            self.failed = true;
            ChildExecutionError::Refused
        })?;
        let custody = self
            .custody_resources(
                [0; 32],
                [0; 32],
                context.candidate_id,
                context.execution.market_id,
            )
            .map_err(|_| {
                self.failed = true;
                ChildExecutionError::Refused
            })?;
        self.record(build_materialize_packets_v2(
            true,
            context,
            outcome_count,
            quantity,
            claims,
            custody,
        ))
    }

    fn merge_complete_set(
        &mut self,
        context: AggregateReplayContextV1,
        outcome_count: u8,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        let claims = self.claims_resources(None).map_err(|_| {
            self.failed = true;
            ChildExecutionError::Refused
        })?;
        let custody = self
            .custody_resources(
                [0; 32],
                [0; 32],
                context.execution.market_id,
                context.candidate_id,
            )
            .map_err(|_| {
                self.failed = true;
                ChildExecutionError::Refused
            })?;
        self.record(build_materialize_packets_v2(
            false,
            context,
            outcome_count,
            quantity,
            claims,
            custody,
        ))
    }

    fn distribute_claims(
        &mut self,
        context: RowReplayContextV1,
        outcome_count: u8,
        quantities: &[u64; dclutch_general_codec::MAX_OUTCOMES],
    ) -> core::result::Result<(), ChildExecutionError> {
        let claims = self.claims_resources(Some(context.owner_id)).map_err(|_| {
            self.failed = true;
            ChildExecutionError::Refused
        })?;
        self.record(build_row_packets_v2(
            dclutch_general_adapter_contract::GeneralChildEffectV1::DistributeClaims,
            context,
            outcome_count,
            quantities,
            claims,
            None,
        ))
    }

    fn distribute_collateral(
        &mut self,
        context: RowReplayContextV1,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        let mut quantities = [0; dclutch_general_codec::MAX_OUTCOMES];
        quantities[0] = quantity;
        let custody = self
            .custody_resources([0; 32], context.owner_id, context.candidate_id, [0; 32])
            .map_err(|_| {
                self.failed = true;
                ChildExecutionError::Refused
            })?;
        self.record(build_row_packets_v2(
            dclutch_general_adapter_contract::GeneralChildEffectV1::DistributeCollateral,
            context,
            1,
            &quantities,
            ClaimsResourcesV2 {
                settlement_owner: context.candidate_id,
                market_revision: 0,
                owner_position_revision: 0,
                settlement_position_revision: 0,
            },
            Some(custody),
        ))
    }

    fn pay_surplus(
        &mut self,
        context: AggregateReplayContextV1,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        let route = QuoteSurplusRouteV2 {
            destination_account: self.state.collateral_destination.key.to_bytes(),
            beneficiary: self.config.quote_surplus_beneficiary(),
        };
        let custody = self
            .custody_resources([0; 32], route.beneficiary, context.candidate_id, [0; 32])
            .map_err(|_| {
                self.failed = true;
                ChildExecutionError::Refused
            })?;
        self.record(build_surplus_packet_v2(context, quantity, route, custody))
    }
}

fn authenticate_consider_selection(
    state: &GeneralConsiderStateV1,
    program_id: Pubkey,
    candidate: CandidateV1,
    policy: SelectionPolicyV1,
) -> Result<(u64, Option<VerifiedCandidateV1>), Error> {
    if is_zero(&state.selection.data) {
        if state.incumbent_certificate != state.common.market {
            return Err(Error::AccountAlias);
        }
        return Ok((0, None));
    }
    let selection =
        SelectionCursorV1::decode(&state.selection.data).map_err(|_| Error::Selection)?;
    if selection.closed
        || selection.batch_id != candidate.batch_id
        || selection.policy_id != policy.policy_id
    {
        return Err(Error::Selection);
    }
    match selection.best_candidate_id {
        None => {
            if state.incumbent_certificate != state.common.market {
                return Err(Error::AccountAlias);
            }
            Ok((selection.revision, None))
        }
        Some(best) => {
            require_owned(
                &state.incumbent_certificate,
                program_id,
                VERIFIED_CANDIDATE_BYTES_V1,
            )?;
            require_pda(
                program_id,
                state.common.market.key,
                state.incumbent_certificate.key,
                &[GENERAL_CERTIFICATE_PDA_DOMAIN_V1, &best],
                Error::Certificate,
            )?;
            let incumbent = VerifiedCandidateV1::decode(&state.incumbent_certificate.data)
                .map_err(|_| Error::Certificate)?;
            if incumbent.candidate_id != best {
                return Err(Error::Certificate);
            }
            Ok((selection.revision, Some(incumbent)))
        }
    }
}

fn report(
    instruction: Instruction,
    action: Action,
    observation: Observation,
    authority: AuthenticatedCommon,
    candidate_id: Option<[u8; 32]>,
    expected_revision: u64,
    expected_accounts: usize,
) -> Result<GeneralActionReportV1, Error> {
    if instruction.accounts.len() != expected_accounts
        || instruction.accounts.iter().any(|account| account.is_signer)
    {
        return Err(Error::Encoding);
    }
    Ok(GeneralActionReportV1 {
        instruction,
        action,
        observation,
        execution_release_set_id: authority.release_set_id,
        candidate_id,
        expected_revision,
        compute: GeneralComputeEvidenceV1 {
            trading_elf_bytes_hashed: authority.elf_bytes,
            matching_measured_compute_units: None,
        },
    })
}

fn require_owned(account: &ObservedAccount, owner: Pubkey, width: usize) -> Result<(), Error> {
    if account.owner != owner || account.executable || account.data.len() != width {
        return Err(Error::AccountShape);
    }
    Ok(())
}

fn require_pda(
    program_id: Pubkey,
    market: Pubkey,
    actual: Pubkey,
    suffix: &[&[u8]],
    error: Error,
) -> Result<(), Error> {
    let mut seeds = Vec::with_capacity(suffix.len().saturating_add(1));
    let market_bytes = market.to_bytes();
    let domain = suffix.first().copied().ok_or(Error::Encoding)?;
    seeds.push(domain);
    seeds.push(market_bytes.as_slice());
    seeds.extend(suffix.iter().skip(1).copied());
    if Pubkey::find_program_address(&seeds, &program_id).0 != actual {
        return Err(error);
    }
    Ok(())
}

fn same_observation(accounts: &[&ObservedAccount]) -> Result<Observation, Error> {
    let observation = accounts
        .first()
        .map(|account| account.observation)
        .ok_or(Error::ObservationMismatch)?;
    if accounts
        .iter()
        .any(|account| account.observation.finality != Finality::Finalized)
    {
        return Err(Error::ObservationNotFinalized);
    }
    if accounts
        .iter()
        .any(|account| account.observation != observation)
    {
        return Err(Error::ObservationMismatch);
    }
    Ok(observation)
}

fn authenticate_aliases(
    accounts: &[&ObservedAccount],
    allowed_alias: Option<(usize, usize)>,
) -> Result<(), Error> {
    for (left_index, left) in accounts.iter().enumerate() {
        for (right_index, right) in accounts
            .iter()
            .enumerate()
            .skip(left_index.saturating_add(1))
        {
            if left.key != right.key {
                continue;
            }
            if left != right || allowed_alias != Some((left_index, right_index)) {
                return Err(Error::AccountAlias);
            }
        }
    }
    Ok(())
}

fn is_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

fn readonly(pubkey: Pubkey) -> AccountMeta {
    AccountMeta::new_readonly(pubkey, false)
}

fn writable(pubkey: Pubkey) -> AccountMeta {
    AccountMeta::new(pubkey, false)
}

fn short_vec_prefix_bytes(value: usize) -> usize {
    if value < 128 {
        1
    } else if value < 16_384 {
        2
    } else {
        3
    }
}

#[cfg(test)]
mod tests;
