//! Chain-derived unsigned workflows for the General physical controller.
//!
//! These builders consume one same-slot finalized observation, hostile-decode
//! every General-owned account, reauthenticate the activated Trading release
//! and its Loader V3 deployment, and return unsigned instructions. They never
//! perform RPC, access keys, sign, submit, or construct settlement-child calls.

use dclutch_general_adapter_contract::{
    CandidateVerifierV1, GENERAL_CANDIDATE_PDA_DOMAIN_V1, GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
    GENERAL_PAGE_PDA_DOMAIN_V1, GENERAL_POLICY_PDA_DOMAIN_V1, GENERAL_SELECTION_PDA_DOMAIN_V1,
    GENERAL_SETTLEMENT_PDA_DOMAIN_V1, GENERAL_VERIFICATION_PDA_DOMAIN_V1,
    VERIFICATION_CURSOR_BYTES_V1, VERIFIED_CANDIDATE_BYTES_V1, VerifiedCandidateV1,
    consider_verified, freeze_selection, initialize_settlement,
};
use dclutch_general_codec::{
    Action, CANDIDATE_BYTES, CandidateV1, ControllerRequestV1, PAGE_BYTES, PageViewV1,
    SELECTION_CURSOR_BYTES, SELECTION_POLICY_BYTES, SETTLEMENT_CURSOR_BYTES, SelectionCursorV1,
    SelectionPolicyV1,
};
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_hash::Hash;
use solana_message::{VersionedMessage, v0};
use solana_program::{
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
