#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::indexing_slicing)]

//! Registry-authenticated, verifier-oriented General SBF adapter.
//!
//! The currently executable slice streams candidate verification, freezes the
//! best valid submitted candidate, and initializes settlement. Physical Claims
//! and Custody actions refuse until their separately owned canonical child
//! adapters are linked; no provisional child wire exists here.

extern crate std;

use dclutch_general_adapter_contract::{
    CandidateVerifierV1, GENERAL_CANDIDATE_PDA_DOMAIN_V1, GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
    GENERAL_PAGE_PDA_DOMAIN_V1, GENERAL_POLICY_PDA_DOMAIN_V1, GENERAL_SELECTION_PDA_DOMAIN_V1,
    GENERAL_SETTLEMENT_PDA_DOMAIN_V1, GENERAL_VERIFICATION_PDA_DOMAIN_V1,
    VERIFICATION_CURSOR_BYTES_V1, VERIFIED_CANDIDATE_BYTES_V1, VerifiedCandidateV1,
    consider_verified, freeze_selection, initialize_settlement,
};
use dclutch_general_codec::{
    Action, CANDIDATE_BYTES, CONTROLLER_REQUEST_BYTES, CandidateV1, ControllerRequestV1,
    PAGE_BYTES, SELECTION_CURSOR_BYTES, SELECTION_POLICY_BYTES, SETTLEMENT_CURSOR_BYTES,
    SelectionCursorV1, SelectionPolicyV1,
};
use dclutch_general_config_contract::{
    GENERAL_ACTIVATION_REQUEST_BYTES_V2, GENERAL_CONFIG_BYTES_V2, GENERAL_ROOT_PDA_DOMAIN_V2,
    GeneralConfigV2, GeneralLifecycleV2, GeneralRootV2,
};
use dclutch_market_core_codec::{CORE_EFFECT_ENVELOPE_BYTES_V1, CORE_EFFECT_MAGIC_V1};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, hash::hash, program_error::ProgramError,
    pubkey::Pubkey,
};

mod activation_handler;

/// Exact account count for streamed candidate consideration.
pub const CONSIDER_ACCOUNT_COUNT_V2: usize = 10;
/// Exact account count for selection freeze.
pub const FREEZE_ACCOUNT_COUNT_V2: usize = 4;
/// Exact account count for settlement initialization.
pub const INITIALIZE_ACCOUNT_COUNT_V2: usize = 7;
/// Exact reserved account count for physical settlement actions.
pub const SETTLEMENT_ACCOUNT_COUNT_V2: usize = 22;
/// Exact account count for Core-authenticated General activation.
pub const ACTIVATE_ACCOUNT_COUNT_V2: usize = 8;
/// Exact instruction width for a Core envelope plus General activation request.
pub const ACTIVATE_INSTRUCTION_BYTES_V2: usize =
    CORE_EFFECT_ENVELOPE_BYTES_V1 + GENERAL_ACTIVATION_REQUEST_BYTES_V2;

const MARKET: usize = 0;
const GENERAL_ROOT: usize = 1;
const GENERAL_CONFIG: usize = 2;

/// Stable physical-adapter refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralSbfError {
    /// The exact 64-byte controller request refused.
    Instruction = 0,
    /// Account count, order, aliasing, owner, or privilege refused.
    AccountFrame = 1,
    /// Registry activation cache, Trading deployment, or receipt refused.
    ReleaseAdmission = 2,
    /// Candidate, policy, or page bytes/PDA refused.
    ImmutableInput = 3,
    /// Verification cursor bytes/PDA or page progression refused.
    Verification = 4,
    /// Selection cursor bytes/PDA or transition refused.
    Selection = 5,
    /// Verified certificate bytes/PDA refused.
    Certificate = 6,
    /// Settlement cursor bytes/PDA or initialization refused.
    Settlement = 7,
    /// An account-data borrow refused.
    Borrow = 8,
    /// Canonical Claims/Custody child integration is not linked yet.
    ChildUnavailable = 9,
    /// Core-authenticated root activation or exact replay refused.
    RootActivation = 10,
    /// A root write, allocation, assignment, or acknowledgement refused.
    Commit = 11,
}

impl From<GeneralSbfError> for ProgramError {
    fn from(value: GeneralSbfError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(program_entrypoint);

#[cfg(not(feature = "no-entrypoint"))]
fn program_entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    process_general_family(program_id, accounts, instruction_data)
}

/// Execute the reusable General family behind the canonical Trading entrypoint.
///
/// The standalone wrapper is measurement-only. The release build calls this
/// handler after its shared Trading-family dispatch and admission boundary.
#[inline(never)]
pub fn process_general_family(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() == ACTIVATE_INSTRUCTION_BYTES_V2
        && instruction_data.get(..CORE_EFFECT_MAGIC_V1.len())
            == Some(CORE_EFFECT_MAGIC_V1.as_slice())
    {
        return activation_handler::process(program_id, accounts, instruction_data);
    }
    if instruction_data.len() != CONTROLLER_REQUEST_BYTES {
        return Err(GeneralSbfError::Instruction.into());
    }
    let request =
        ControllerRequestV1::decode(instruction_data).map_err(|_| GeneralSbfError::Instruction)?;
    let config = validate_hot_context(program_id, accounts, request.action)?;
    process_hot(program_id, accounts, request, config)
}

/// Execute after exact activated-root and content-addressed-config validation.
///
/// This function is public so host tests can exercise state rollback without
/// replacing Solana account loading.
#[inline(never)]
pub fn process_hot(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ControllerRequestV1,
    config: GeneralConfigV2,
) -> ProgramResult {
    match request.action {
        Action::Consider => process_consider(program_id, accounts, request, config),
        Action::Freeze => process_freeze(program_id, accounts, request, config),
        Action::InitializeSettlement => process_initialize(program_id, accounts, request, config),
        Action::Collect | Action::Materialize | Action::Distribute | Action::Close => {
            if accounts.len() != SETTLEMENT_ACCOUNT_COUNT_V2 {
                Err(GeneralSbfError::AccountFrame.into())
            } else {
                Err(GeneralSbfError::ChildUnavailable.into())
            }
        }
    }
}

fn validate_hot_context(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    action: Action,
) -> Result<GeneralConfigV2, ProgramError> {
    let expected = match action {
        Action::Consider => CONSIDER_ACCOUNT_COUNT_V2,
        Action::Freeze => FREEZE_ACCOUNT_COUNT_V2,
        Action::InitializeSettlement => INITIALIZE_ACCOUNT_COUNT_V2,
        Action::Collect | Action::Materialize | Action::Distribute | Action::Close => {
            SETTLEMENT_ACCOUNT_COUNT_V2
        }
    };
    if accounts.len() != expected
        || accounts.iter().any(|account| account.is_signer)
        || accounts[MARKET].executable
        || accounts[MARKET].is_writable
        || accounts[GENERAL_ROOT].owner != program_id
        || accounts[GENERAL_ROOT].executable
        || accounts[GENERAL_ROOT].is_writable
        || accounts[GENERAL_CONFIG].executable
        || accounts[GENERAL_CONFIG].is_writable
    {
        return Err(GeneralSbfError::AccountFrame.into());
    }
    let root = {
        let bytes = accounts[GENERAL_ROOT]
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        GeneralRootV2::decode(&bytes).map_err(|_| GeneralSbfError::ImmutableInput)?
    };
    let config = {
        let bytes = accounts[GENERAL_CONFIG]
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        if bytes.len() != GENERAL_CONFIG_BYTES_V2 || hash(&bytes).to_bytes() != root.config_id() {
            return Err(GeneralSbfError::ImmutableInput.into());
        }
        GeneralConfigV2::decode(&bytes).map_err(|_| GeneralSbfError::ImmutableInput)?
    };
    if root.lifecycle() != GeneralLifecycleV2::Active
        || root.market() != accounts[MARKET].key.to_bytes()
        || root.generation() != config.generation()
    {
        return Err(GeneralSbfError::ImmutableInput.into());
    }
    let generation = root.generation().to_le_bytes();
    require_pda(
        program_id,
        &accounts[GENERAL_ROOT],
        &[
            GENERAL_ROOT_PDA_DOMAIN_V2,
            accounts[MARKET].key.as_ref(),
            &generation,
            &root.config_id(),
        ],
        GeneralSbfError::ImmutableInput,
    )?;
    Ok(config)
}

#[inline(never)]
#[cfg(any())]
fn process_core_activation(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != ACTIVATE_ACCOUNT_COUNT_V2 {
        return Err(GeneralSbfError::AccountFrame.into());
    }
    let envelope_bytes = instruction_data
        .get(..CORE_EFFECT_ENVELOPE_BYTES_V1)
        .ok_or(GeneralSbfError::Instruction)?;
    let request_bytes = instruction_data
        .get(CORE_EFFECT_ENVELOPE_BYTES_V1..)
        .ok_or(GeneralSbfError::Instruction)?;
    let envelope =
        CoreEffectEnvelopeV1::decode(envelope_bytes).map_err(|_| GeneralSbfError::Instruction)?;
    let request = GeneralActivationRequestV2::decode(request_bytes)
        .map_err(|_| GeneralSbfError::Instruction)?;
    let request_digest = identity(hash(request_bytes).to_bytes())?;
    envelope
        .validate_role_request(request_bytes.len(), request_digest)
        .map_err(|_| GeneralSbfError::RootActivation)?;
    validate_activation_frame(program_id, accounts, envelope, request)?;

    let config = {
        let config_bytes = accounts[3]
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        if hash(&config_bytes).to_bytes() != request.config_id() {
            return Err(GeneralSbfError::RootActivation.into());
        }
        GeneralConfigV2::decode(&config_bytes).map_err(|_| GeneralSbfError::RootActivation)?
    };
    if config.generation() != envelope.generation() {
        return Err(GeneralSbfError::RootActivation.into());
    }

    let market = envelope.market().to_bytes();
    let generation = envelope.generation().to_le_bytes();
    let config_id = request.config_id();
    let (expected_root, bump) = Pubkey::find_program_address(
        &[GENERAL_ROOT_PDA_DOMAIN_V2, &market, &generation, &config_id],
        program_id,
    );
    if accounts[2].key != &expected_root || request.root() != expected_root.to_bytes() {
        return Err(GeneralSbfError::RootActivation.into());
    }
    request
        .require_normalized_root_lamports(accounts[2].lamports())
        .map_err(|_| GeneralSbfError::RootActivation)?;

    let expected = GeneralRootV2::active(market, config_id, envelope.generation())
        .map_err(|_| GeneralSbfError::RootActivation)?;
    if accounts[2].owner == program_id {
        let root_bytes = accounts[2]
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        let present =
            GeneralRootV2::decode(&root_bytes).map_err(|_| GeneralSbfError::RootActivation)?;
        if present != expected {
            return Err(GeneralSbfError::RootActivation.into());
        }
    } else {
        if accounts[2].owner != &system_program::ID || accounts[2].data_len() != 0 {
            return Err(GeneralSbfError::RootActivation.into());
        }
        let bump_seed = [bump];
        let signer = [
            GENERAL_ROOT_PDA_DOMAIN_V2,
            market.as_slice(),
            generation.as_slice(),
            config_id.as_slice(),
            bump_seed.as_slice(),
        ];
        let root_space =
            u64::try_from(GENERAL_ROOT_BYTES_V2).map_err(|_| GeneralSbfError::Commit)?;
        invoke_signed(
            &allocate(accounts[2].key, root_space),
            &[accounts[2].clone(), accounts[4].clone()],
            &[&signer],
        )
        .map_err(|_| GeneralSbfError::Commit)?;
        invoke_signed(
            &assign(accounts[2].key, program_id),
            &[accounts[2].clone(), accounts[4].clone()],
            &[&signer],
        )
        .map_err(|_| GeneralSbfError::Commit)?;
        if accounts[2].owner != program_id || accounts[2].data_len() != GENERAL_ROOT_BYTES_V2 {
            return Err(GeneralSbfError::Commit.into());
        }
        let mut root_bytes = accounts[2]
            .try_borrow_mut_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        if root_bytes.iter().any(|byte| *byte != 0) {
            return Err(GeneralSbfError::Commit.into());
        }
        root_bytes.copy_from_slice(&expected.to_bytes());
    }

    let acknowledgement = activation_ack(
        program_id,
        envelope,
        envelope_bytes,
        request_bytes,
        expected,
    )?;
    set_return_data(
        &acknowledgement
            .encode()
            .map_err(|_| GeneralSbfError::Commit)?,
    );
    Ok(())
}

#[cfg(any())]
fn activation_ack(
    program_id: &Pubkey,
    envelope: CoreEffectEnvelopeV1,
    envelope_bytes: &[u8],
    request_bytes: &[u8],
    root: GeneralRootV2,
) -> Result<CoreEffectAckV1, ProgramError> {
    let envelope_length = u32::try_from(envelope_bytes.len())
        .map_err(|_| GeneralSbfError::Instruction)?
        .to_le_bytes();
    let request_length = u32::try_from(request_bytes.len())
        .map_err(|_| GeneralSbfError::Instruction)?
        .to_le_bytes();
    let effect_digest = identity(
        hashv(&[
            &CORE_EFFECT_DIGEST_DOMAIN_V1,
            &envelope_length,
            envelope_bytes,
            &request_length,
            request_bytes,
        ])
        .to_bytes(),
    )?;
    CoreEffectAckV1::new(
        envelope.action(),
        envelope.target_role(),
        identity(program_id.to_bytes())?,
        envelope.release_set(),
        envelope.market(),
        envelope.context(),
        effect_digest,
        identity(hash(&root.to_bytes()).to_bytes())?,
        0,
        root.revision(),
        0,
        0,
    )
    .map_err(|_| GeneralSbfError::Commit.into())
}

#[cfg(any())]
fn validate_activation_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope: CoreEffectEnvelopeV1,
    request: GeneralActivationRequestV2,
) -> ProgramResult {
    let authority = &accounts[0];
    let core_program = &accounts[1];
    let root = &accounts[2];
    let config = &accounts[3];
    let system = &accounts[4];
    if !authority.is_signer
        || authority.is_writable
        || authority.executable
        || !core_program.executable
        || core_program.is_signer
        || core_program.is_writable
        || !root.is_writable
        || root.is_signer
        || root.executable
        || config.is_writable
        || config.is_signer
        || config.executable
        || system.key != &system_program::ID
        || !system.executable
        || system.is_signer
        || system.is_writable
        || accounts.iter().enumerate().any(|(left, account)| {
            accounts
                .iter()
                .skip(left + 1)
                .any(|other| other.key == account.key)
        })
    {
        return Err(GeneralSbfError::AccountFrame.into());
    }
    if envelope.action() != CoreEffectActionV1::ActivateCapability
        || envelope.caller_program().to_bytes() != core_program.key.to_bytes()
        || envelope.caller_authority().to_bytes() != authority.key.to_bytes()
        || envelope.expected_resource_a_revision() != 0
        || envelope.expected_resource_b_revision() != 0
        || request.root() != root.key.to_bytes()
    {
        return Err(GeneralSbfError::RootActivation.into());
    }
    validate_core_caller_authority(authority, core_program, envelope)?;
    validate_capability_implementation(program_id, core_program.key, envelope.target_role())
}

#[cfg(any())]
fn validate_core_caller_authority(
    authority: &AccountInfo<'_>,
    core_program: &AccountInfo<'_>,
    envelope: CoreEffectEnvelopeV1,
) -> ProgramResult {
    let authority_seeds = envelope
        .caller_authority_seeds()
        .map_err(|_| GeneralSbfError::ReleaseAdmission)?;
    let (expected_authority, _) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), core_program.key);
    if authority.key == &expected_authority {
        Ok(())
    } else {
        Err(GeneralSbfError::ReleaseAdmission.into())
    }
}

/// Deliberately isolated until the Registry representation of multiple
/// capability implementations behind the Trading role is frozen.
#[cfg(any())]
fn validate_capability_implementation(
    program_id: &Pubkey,
    core_program_id: &Pubkey,
    target_role: Role,
) -> ProgramResult {
    if program_id != core_program_id && target_role != Role::Core {
        Ok(())
    } else {
        Err(GeneralSbfError::ReleaseAdmission.into())
    }
}

#[cfg(any())]
fn identity(bytes: [u8; 32]) -> Result<Identity, ProgramError> {
    Identity::new(bytes).map_err(|_| GeneralSbfError::RootActivation.into())
}

#[inline(never)]
fn process_consider(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ControllerRequestV1,
    config: GeneralConfigV2,
) -> ProgramResult {
    if accounts.len() != CONSIDER_ACCOUNT_COUNT_V2 {
        return Err(GeneralSbfError::AccountFrame.into());
    }
    let selection = &accounts[3];
    let verification = &accounts[4];
    let certificate = &accounts[5];
    let candidate_account = &accounts[6];
    let policy_account = &accounts[7];
    let page_account = &accounts[8];
    let incumbent_account = &accounts[9];
    require_owned_state(program_id, selection, SELECTION_CURSOR_BYTES, true)?;
    require_owned_state(program_id, verification, VERIFICATION_CURSOR_BYTES_V1, true)?;
    require_owned_state(program_id, certificate, VERIFIED_CANDIDATE_BYTES_V1, true)?;
    require_owned_state(program_id, candidate_account, CANDIDATE_BYTES, false)?;
    require_owned_state(program_id, policy_account, SELECTION_POLICY_BYTES, false)?;
    require_owned_state(program_id, page_account, PAGE_BYTES, false)?;

    let candidate = decode_candidate(candidate_account)?;
    let policy = decode_policy(policy_account)?;
    config
        .require_selection_policy(policy.policy_id)
        .map_err(|_| GeneralSbfError::ImmutableInput)?;
    let candidate_id = request.candidate_id.ok_or(GeneralSbfError::Instruction)?;
    if candidate.candidate_id != candidate_id {
        return Err(GeneralSbfError::ImmutableInput.into());
    }
    require_general_data_pdas(
        program_id,
        accounts[MARKET].key,
        candidate_account,
        policy_account,
        page_account,
        candidate,
        policy,
        request.page_index,
    )?;
    require_pda(
        program_id,
        selection,
        &[
            GENERAL_SELECTION_PDA_DOMAIN_V1,
            accounts[MARKET].key.as_ref(),
            &candidate.batch_id,
        ],
        GeneralSbfError::Selection,
    )?;
    require_pda(
        program_id,
        verification,
        &[
            GENERAL_VERIFICATION_PDA_DOMAIN_V1,
            accounts[MARKET].key.as_ref(),
            &candidate.candidate_id,
        ],
        GeneralSbfError::Verification,
    )?;
    require_pda(
        program_id,
        certificate,
        &[
            GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
            accounts[MARKET].key.as_ref(),
            &candidate.candidate_id,
        ],
        GeneralSbfError::Certificate,
    )?;

    let mut verifier = load_verifier(verification, candidate, request)?;
    ingest_verifier(&mut verifier, page_account, request.expected_revision)?;
    config
        .require_candidate_envelope(
            candidate.outcome_count,
            candidate.page_count,
            candidate.price_scale,
            verifier.order_count(),
        )
        .map_err(|_| GeneralSbfError::Verification)?;

    if verifier.is_complete() {
        let verified = finish_verifier(verifier)?;
        finalize_consider(
            program_id,
            accounts,
            selection,
            certificate,
            incumbent_account,
            candidate,
            policy,
            verified,
        )?;
    } else {
        require_zero_account(certificate, GeneralSbfError::Certificate)?;
    }
    store_verifier(verification, verifier)
}

#[inline(never)]
fn load_verifier(
    verification: &AccountInfo<'_>,
    candidate: CandidateV1,
    request: ControllerRequestV1,
) -> Result<CandidateVerifierV1, ProgramError> {
    let bytes = verification
        .try_borrow_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    if bytes.iter().all(|byte| *byte == 0) {
        if request.page_index != 0 || request.expected_revision != 0 {
            return Err(GeneralSbfError::Verification.into());
        }
        return Ok(CandidateVerifierV1::begin(candidate));
    }
    let verifier =
        CandidateVerifierV1::decode(&bytes).map_err(|_| GeneralSbfError::Verification)?;
    if verifier.candidate() != candidate || verifier.next_page() != request.page_index {
        return Err(GeneralSbfError::Verification.into());
    }
    Ok(verifier)
}

#[inline(never)]
fn ingest_verifier(
    verifier: &mut CandidateVerifierV1,
    page_account: &AccountInfo<'_>,
    expected_revision: u64,
) -> ProgramResult {
    let page = page_account
        .try_borrow_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    verifier
        .ingest_page_at(&page, expected_revision)
        .map_err(|_| GeneralSbfError::Verification.into())
}

#[inline(never)]
fn finish_verifier(verifier: CandidateVerifierV1) -> Result<VerifiedCandidateV1, ProgramError> {
    verifier
        .finish()
        .map_err(|_| GeneralSbfError::Verification.into())
}

#[inline(never)]
fn store_verifier(verification: &AccountInfo<'_>, verifier: CandidateVerifierV1) -> ProgramResult {
    let bytes = verifier
        .to_bytes()
        .map_err(|_| GeneralSbfError::Verification)?;
    verification
        .try_borrow_mut_data()
        .map_err(|_| GeneralSbfError::Borrow)?
        .copy_from_slice(&bytes);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn finalize_consider(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selection: &AccountInfo<'_>,
    certificate: &AccountInfo<'_>,
    incumbent_account: &AccountInfo<'_>,
    candidate: CandidateV1,
    policy: SelectionPolicyV1,
    verified: VerifiedCandidateV1,
) -> ProgramResult {
    let mut selection_bytes = [0_u8; SELECTION_CURSOR_BYTES];
    {
        let source = selection
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        selection_bytes.copy_from_slice(&source);
    }
    let (selection_revision, best) = if selection_bytes.iter().all(|byte| *byte == 0) {
        (0, None)
    } else {
        let cursor =
            SelectionCursorV1::decode(&selection_bytes).map_err(|_| GeneralSbfError::Selection)?;
        (cursor.revision, cursor.best_candidate_id)
    };
    let incumbent = match best {
        None => {
            if incumbent_account.key != accounts[MARKET].key {
                return Err(GeneralSbfError::AccountFrame.into());
            }
            None
        }
        Some(best_id) => {
            require_owned_state(
                program_id,
                incumbent_account,
                VERIFIED_CANDIDATE_BYTES_V1,
                false,
            )?;
            require_pda(
                program_id,
                incumbent_account,
                &[
                    GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
                    accounts[MARKET].key.as_ref(),
                    &best_id,
                ],
                GeneralSbfError::Certificate,
            )?;
            let bytes = incumbent_account
                .try_borrow_data()
                .map_err(|_| GeneralSbfError::Borrow)?;
            Some(VerifiedCandidateV1::decode(&bytes).map_err(|_| GeneralSbfError::Certificate)?)
        }
    };
    let mut certificate_bytes = [0_u8; VERIFIED_CANDIDATE_BYTES_V1];
    {
        let source = certificate
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        certificate_bytes.copy_from_slice(&source);
    }
    consider_verified(
        &mut selection_bytes,
        &mut certificate_bytes,
        &candidate,
        &policy,
        verified,
        incumbent.as_ref(),
        selection_revision,
    )
    .map_err(|_| GeneralSbfError::Selection)?;
    selection
        .try_borrow_mut_data()
        .map_err(|_| GeneralSbfError::Borrow)?
        .copy_from_slice(&selection_bytes);
    certificate
        .try_borrow_mut_data()
        .map_err(|_| GeneralSbfError::Borrow)?
        .copy_from_slice(&certificate_bytes);
    Ok(())
}

#[inline(never)]
fn process_freeze(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ControllerRequestV1,
    config: GeneralConfigV2,
) -> ProgramResult {
    if accounts.len() != FREEZE_ACCOUNT_COUNT_V2 {
        return Err(GeneralSbfError::AccountFrame.into());
    }
    let selection = &accounts[3];
    require_owned_state(program_id, selection, SELECTION_CURSOR_BYTES, true)?;
    let mut bytes = [0_u8; SELECTION_CURSOR_BYTES];
    {
        let source = selection
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        bytes.copy_from_slice(&source);
    }
    let cursor = SelectionCursorV1::decode(&bytes).map_err(|_| GeneralSbfError::Selection)?;
    config
        .require_selection_policy(cursor.policy_id)
        .map_err(|_| GeneralSbfError::Selection)?;
    require_pda(
        program_id,
        selection,
        &[
            GENERAL_SELECTION_PDA_DOMAIN_V1,
            accounts[MARKET].key.as_ref(),
            &cursor.batch_id,
        ],
        GeneralSbfError::Selection,
    )?;
    freeze_selection(&mut bytes, request.expected_revision)
        .map_err(|_| GeneralSbfError::Selection)?;
    selection
        .try_borrow_mut_data()
        .map_err(|_| GeneralSbfError::Borrow)?
        .copy_from_slice(&bytes);
    Ok(())
}

#[inline(never)]
fn process_initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ControllerRequestV1,
    config: GeneralConfigV2,
) -> ProgramResult {
    if accounts.len() != INITIALIZE_ACCOUNT_COUNT_V2 {
        return Err(GeneralSbfError::AccountFrame.into());
    }
    let selection = &accounts[3];
    let settlement = &accounts[4];
    let certificate = &accounts[5];
    let candidate_account = &accounts[6];
    require_owned_state(program_id, selection, SELECTION_CURSOR_BYTES, false)?;
    require_owned_state(program_id, settlement, SETTLEMENT_CURSOR_BYTES, true)?;
    require_owned_state(program_id, certificate, VERIFIED_CANDIDATE_BYTES_V1, false)?;
    require_owned_state(program_id, candidate_account, CANDIDATE_BYTES, false)?;
    let candidate = decode_candidate(candidate_account)?;
    if request.candidate_id != Some(candidate.candidate_id) {
        return Err(GeneralSbfError::Instruction.into());
    }
    config
        .require_candidate_envelope(
            candidate.outcome_count,
            candidate.page_count,
            candidate.price_scale,
            0,
        )
        .map_err(|_| GeneralSbfError::ImmutableInput)?;
    require_pda(
        program_id,
        selection,
        &[
            GENERAL_SELECTION_PDA_DOMAIN_V1,
            accounts[MARKET].key.as_ref(),
            &candidate.batch_id,
        ],
        GeneralSbfError::Selection,
    )?;
    require_pda(
        program_id,
        certificate,
        &[
            GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
            accounts[MARKET].key.as_ref(),
            &candidate.candidate_id,
        ],
        GeneralSbfError::Certificate,
    )?;
    require_pda(
        program_id,
        settlement,
        &[
            GENERAL_SETTLEMENT_PDA_DOMAIN_V1,
            accounts[MARKET].key.as_ref(),
            &candidate.candidate_id,
        ],
        GeneralSbfError::Settlement,
    )?;
    let verified = {
        let bytes = certificate
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        VerifiedCandidateV1::decode(&bytes).map_err(|_| GeneralSbfError::Certificate)?
    };
    if verified.candidate_id != candidate.candidate_id {
        return Err(GeneralSbfError::Certificate.into());
    }
    let selection_bytes = selection
        .try_borrow_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    let selection_cursor =
        SelectionCursorV1::decode(&selection_bytes).map_err(|_| GeneralSbfError::Selection)?;
    config
        .require_selection_policy(selection_cursor.policy_id)
        .map_err(|_| GeneralSbfError::Selection)?;
    let mut settlement_bytes = [0_u8; SETTLEMENT_CURSOR_BYTES];
    {
        let source = settlement
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        settlement_bytes.copy_from_slice(&source);
    }
    initialize_settlement(
        &mut settlement_bytes,
        &selection_bytes,
        &verified,
        request.expected_revision,
    )
    .map_err(|_| GeneralSbfError::Settlement)?;
    drop(selection_bytes);
    settlement
        .try_borrow_mut_data()
        .map_err(|_| GeneralSbfError::Borrow)?
        .copy_from_slice(&settlement_bytes);
    Ok(())
}

fn decode_candidate(account: &AccountInfo<'_>) -> Result<CandidateV1, ProgramError> {
    let bytes = account
        .try_borrow_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    CandidateV1::decode(&bytes).map_err(|_| GeneralSbfError::ImmutableInput.into())
}

fn decode_policy(account: &AccountInfo<'_>) -> Result<SelectionPolicyV1, ProgramError> {
    let bytes = account
        .try_borrow_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    SelectionPolicyV1::decode(&bytes).map_err(|_| GeneralSbfError::ImmutableInput.into())
}

#[allow(clippy::too_many_arguments)]
fn require_general_data_pdas(
    program_id: &Pubkey,
    market: &Pubkey,
    candidate_account: &AccountInfo<'_>,
    policy_account: &AccountInfo<'_>,
    page_account: &AccountInfo<'_>,
    candidate: CandidateV1,
    policy: SelectionPolicyV1,
    page_index: u32,
) -> ProgramResult {
    let page_bytes = page_index.to_le_bytes();
    require_pda(
        program_id,
        candidate_account,
        &[
            GENERAL_CANDIDATE_PDA_DOMAIN_V1,
            market.as_ref(),
            &candidate.candidate_id,
        ],
        GeneralSbfError::ImmutableInput,
    )?;
    require_pda(
        program_id,
        policy_account,
        &[
            GENERAL_POLICY_PDA_DOMAIN_V1,
            market.as_ref(),
            &policy.policy_id,
        ],
        GeneralSbfError::ImmutableInput,
    )?;
    require_pda(
        program_id,
        page_account,
        &[
            GENERAL_PAGE_PDA_DOMAIN_V1,
            market.as_ref(),
            &candidate.candidate_id,
            &page_bytes,
        ],
        GeneralSbfError::ImmutableInput,
    )
}

fn require_owned_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    width: usize,
    writable: bool,
) -> ProgramResult {
    if account.owner != program_id
        || account.data_len() != width
        || account.is_writable != writable
        || account.is_signer
        || account.executable
    {
        Err(GeneralSbfError::AccountFrame.into())
    } else {
        Ok(())
    }
}

fn require_zero_account(account: &AccountInfo<'_>, error: GeneralSbfError) -> ProgramResult {
    let data = account
        .try_borrow_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    if data.iter().all(|byte| *byte == 0) {
        Ok(())
    } else {
        Err(error.into())
    }
}

fn require_pda(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    seeds: &[&[u8]],
    error: GeneralSbfError,
) -> ProgramResult {
    let (expected, _) = Pubkey::find_program_address(seeds, program_id);
    if account.key == &expected {
        Ok(())
    } else {
        Err(error.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityFundingDerivationV1,
        CapabilityManifestV1, CompartmentFundingV1, ContentId, FundingAmountsV1,
        FundingCustodyObservationV1, FundingQuoteV1, FundingStateV1, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_general_codec::{
        ExecutionV1, MAX_EXECUTIONS_PER_PAGE, MAX_OUTCOMES, MAX_SELECTION_CRITERIA, PageV1, Phase,
        SelectionCriterion, SettlementCursorV1,
    };
    use dclutch_general_config_contract::{
        GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_CAPABILITY_RELEASE_ID_V2,
        GENERAL_CHILD_DERIVATION_ID_V2, GENERAL_CHILD_SCHEMA_ID_V2, GeneralActivationRequestV2,
        GeneralConfigV2Input,
    };
    use dclutch_market_core_codec::{
        CORE_EFFECT_DIGEST_DOMAIN_V1, CoreEffectActionV1, CoreEffectEnvelopeV1, Identity, Role,
    };
    use solana_program::hash::hashv;
    use solana_sdk_ids::system_program;
    use std::{boxed::Box, vec, vec::Vec};

    fn id(low: u8) -> [u8; 32] {
        let mut value = [0_u8; 32];
        value[0] = low;
        value
    }

    fn vector(first: u64, second: u64) -> [u64; MAX_OUTCOMES] {
        let mut values = [0_u64; MAX_OUTCOMES];
        values[0] = first;
        values[1] = second;
        values
    }

    fn account(
        key: Pubkey,
        writable: bool,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        account_with(key, false, writable, 1, data, owner, executable)
    }

    fn account_with(
        key: Pubkey,
        signer: bool,
        writable: bool,
        lamports: u64,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(lamports)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn core_identity(bytes: [u8; 32]) -> Identity {
        Identity::new(bytes).expect("nonzero identity")
    }

    fn content(bytes: [u8; 32]) -> ContentId {
        ContentId::new(bytes).expect("nonzero content identity")
    }

    fn candidate() -> CandidateV1 {
        CandidateV1 {
            outcome_count: 2,
            candidate_id: id(21),
            product_id: id(31),
            batch_id: id(41),
            page_count: 1,
            price_scale: 2,
            prices: vector(1, 1),
        }
    }

    fn policy() -> SelectionPolicyV1 {
        let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
        criteria[1] = SelectionCriterion::MinimizeQuoteSurplus;
        criteria[2] = SelectionCriterion::MinimizeCandidateId;
        SelectionPolicyV1 {
            policy_id: id(51),
            criterion_count: 3,
            criteria,
        }
    }

    fn config() -> GeneralConfigV2 {
        GeneralConfigV2::new(GeneralConfigV2Input {
            capacity_profile_id: id(61),
            claim_basis_id: id(62),
            capability_release_id: GENERAL_CAPABILITY_RELEASE_ID_V2,
            generation: 7,
            price_scale: 2,
            collection_slots: 10,
            selection_slots: 11,
            settlement_slots: 12,
            max_orders_per_candidate: 32,
            max_pages_per_candidate: 1,
            continuation_reward_lamports: 5,
            selection_policy_id: policy().policy_id,
            outcome_count: 2,
            quote_surplus_beneficiary: id(63),
        })
        .expect("config")
    }

    fn page() -> [u8; PAGE_BYTES] {
        let mut rows = [ExecutionV1::EMPTY; MAX_EXECUTIONS_PER_PAGE];
        rows[0] = ExecutionV1 {
            order_id: id(1),
            owner_id: id(11),
            nonce: 1,
            max_lots: 1,
            max_quote_debit_per_lot: 1,
            lots: 1,
            quote_debit: 1,
            quote_credit: 0,
            receive_per_lot: vector(1, 0),
            deliver_per_lot: [0; MAX_OUTCOMES],
        };
        rows[1] = ExecutionV1 {
            order_id: id(2),
            owner_id: id(12),
            nonce: 1,
            max_lots: 1,
            max_quote_debit_per_lot: 1,
            lots: 1,
            quote_debit: 1,
            quote_credit: 0,
            receive_per_lot: vector(0, 1),
            deliver_per_lot: [0; MAX_OUTCOMES],
        };
        PageV1 {
            outcome_count: 2,
            candidate_id: candidate().candidate_id,
            page_index: 0,
            page_count: 1,
            execution_count: 2,
            executions: rows,
        }
        .to_bytes()
        .expect("page")
    }

    fn common(program_id: Pubkey, market: Pubkey) -> Vec<AccountInfo<'static>> {
        let config = config();
        let config_bytes = config.to_bytes();
        let config_id = hash(&config_bytes).to_bytes();
        let generation = config.generation().to_le_bytes();
        let root_key = Pubkey::find_program_address(
            &[
                GENERAL_ROOT_PDA_DOMAIN_V2,
                market.as_ref(),
                &generation,
                &config_id,
            ],
            &program_id,
        )
        .0;
        let root =
            GeneralRootV2::active(market.to_bytes(), config_id, config.generation()).expect("root");
        vec![
            account(market, false, Vec::new(), Pubkey::new_unique(), false),
            account(root_key, false, root.to_bytes().to_vec(), program_id, false),
            account(
                Pubkey::new_unique(),
                false,
                config_bytes.to_vec(),
                Pubkey::new_unique(),
                false,
            ),
        ]
    }

    fn consider_frame(program_id: Pubkey, market: Pubkey) -> Vec<AccountInfo<'static>> {
        let candidate = candidate();
        let policy = policy();
        let mut frame = common(program_id, market);
        let selection = Pubkey::find_program_address(
            &[
                GENERAL_SELECTION_PDA_DOMAIN_V1,
                market.as_ref(),
                &candidate.batch_id,
            ],
            &program_id,
        )
        .0;
        let verification = Pubkey::find_program_address(
            &[
                GENERAL_VERIFICATION_PDA_DOMAIN_V1,
                market.as_ref(),
                &candidate.candidate_id,
            ],
            &program_id,
        )
        .0;
        let certificate = Pubkey::find_program_address(
            &[
                GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
                market.as_ref(),
                &candidate.candidate_id,
            ],
            &program_id,
        )
        .0;
        let candidate_key = Pubkey::find_program_address(
            &[
                GENERAL_CANDIDATE_PDA_DOMAIN_V1,
                market.as_ref(),
                &candidate.candidate_id,
            ],
            &program_id,
        )
        .0;
        let policy_key = Pubkey::find_program_address(
            &[
                GENERAL_POLICY_PDA_DOMAIN_V1,
                market.as_ref(),
                &policy.policy_id,
            ],
            &program_id,
        )
        .0;
        let page_key = Pubkey::find_program_address(
            &[
                GENERAL_PAGE_PDA_DOMAIN_V1,
                market.as_ref(),
                &candidate.candidate_id,
                &0_u32.to_le_bytes(),
            ],
            &program_id,
        )
        .0;
        frame.extend([
            account(
                selection,
                true,
                vec![0; SELECTION_CURSOR_BYTES],
                program_id,
                false,
            ),
            account(
                verification,
                true,
                vec![0; VERIFICATION_CURSOR_BYTES_V1],
                program_id,
                false,
            ),
            account(
                certificate,
                true,
                vec![0; VERIFIED_CANDIDATE_BYTES_V1],
                program_id,
                false,
            ),
            account(
                candidate_key,
                false,
                candidate.to_bytes().expect("candidate").to_vec(),
                program_id,
                false,
            ),
            account(
                policy_key,
                false,
                policy.to_bytes().expect("policy").to_vec(),
                program_id,
                false,
            ),
            account(page_key, false, page().to_vec(), program_id, false),
            frame[MARKET].clone(),
        ]);
        frame
    }

    fn consider_request() -> ControllerRequestV1 {
        ControllerRequestV1 {
            action: Action::Consider,
            expected_revision: 0,
            candidate_id: Some(candidate().candidate_id),
            page_index: 0,
            execution_index: 0,
        }
    }

    fn execute(
        program_id: &Pubkey,
        accounts: &[AccountInfo<'_>],
        request: ControllerRequestV1,
    ) -> ProgramResult {
        process_instruction(
            program_id,
            accounts,
            &request.to_bytes().expect("request bytes"),
        )
    }

    fn process_instruction(
        program_id: &Pubkey,
        accounts: &[AccountInfo<'_>],
        instruction_data: &[u8],
    ) -> ProgramResult {
        process_general_family(program_id, accounts, instruction_data)
    }

    #[test]
    fn authenticated_consider_streams_and_commits_exact_certificate() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let frame = consider_frame(program_id, market);
        execute(&program_id, &frame, consider_request()).expect("consider");
        let selection =
            SelectionCursorV1::decode(&frame[3].try_borrow_data().expect("selection borrow"))
                .expect("selection");
        assert_eq!(selection.best_candidate_id, Some(candidate().candidate_id));
        assert_eq!(selection.revision, 1);
        let certificate =
            VerifiedCandidateV1::decode(&frame[5].try_borrow_data().expect("certificate borrow"))
                .expect("certificate");
        assert_eq!(certificate.complete_set_quantity, 1);
        assert_eq!(certificate.quote_surplus, 1);
        let verifier =
            CandidateVerifierV1::decode(&frame[4].try_borrow_data().expect("verification borrow"))
                .expect("verification");
        assert!(verifier.is_complete());
    }

    #[test]
    fn substituted_config_policy_and_inactive_root_refuse_before_state_change() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();

        let config_substitution = consider_frame(program_id, market);
        let before = [
            config_substitution[3]
                .try_borrow_data()
                .expect("selection")
                .to_vec(),
            config_substitution[4]
                .try_borrow_data()
                .expect("verification")
                .to_vec(),
            config_substitution[5]
                .try_borrow_data()
                .expect("certificate")
                .to_vec(),
        ];
        config_substitution[2]
            .try_borrow_mut_data()
            .expect("config")
            .get_mut(168)
            .expect("selection policy byte")
            .clone_from(&52);
        assert_eq!(
            execute(&program_id, &config_substitution, consider_request()),
            Err(GeneralSbfError::ImmutableInput.into())
        );
        for (index, expected) in [(3, &before[0]), (4, &before[1]), (5, &before[2])] {
            assert_eq!(
                config_substitution[index]
                    .try_borrow_data()
                    .expect("unchanged state")
                    .as_ref(),
                expected.as_slice()
            );
        }

        let mut policy_substitution = consider_frame(program_id, market);
        let mut substituted_policy = policy();
        substituted_policy.policy_id = id(52);
        let substituted_policy_key = Pubkey::find_program_address(
            &[
                GENERAL_POLICY_PDA_DOMAIN_V1,
                market.as_ref(),
                &substituted_policy.policy_id,
            ],
            &program_id,
        )
        .0;
        policy_substitution[7] = account(
            substituted_policy_key,
            false,
            substituted_policy
                .to_bytes()
                .expect("substituted policy")
                .to_vec(),
            program_id,
            false,
        );
        let verification_before = policy_substitution[4]
            .try_borrow_data()
            .expect("verification")
            .to_vec();
        assert_eq!(
            execute(&program_id, &policy_substitution, consider_request()),
            Err(GeneralSbfError::ImmutableInput.into())
        );
        assert_eq!(
            policy_substitution[4]
                .try_borrow_data()
                .expect("unchanged verification")
                .as_ref(),
            verification_before.as_slice()
        );

        let inactive = consider_frame(program_id, market);
        let mut root = GeneralRootV2::decode(&inactive[1].try_borrow_data().expect("root"))
            .expect("active root");
        root.begin_retiring(1).expect("retiring root");
        inactive[1]
            .try_borrow_mut_data()
            .expect("root")
            .copy_from_slice(&root.to_bytes());
        assert_eq!(
            execute(&program_id, &inactive, consider_request()),
            Err(GeneralSbfError::ImmutableInput.into())
        );
    }

    #[test]
    fn hostile_page_and_stale_replay_preserve_all_general_state() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let frame = consider_frame(program_id, market);
        let selection_before = frame[3].try_borrow_data().expect("selection").to_vec();
        let verification_before = frame[4].try_borrow_data().expect("verification").to_vec();
        let certificate_before = frame[5].try_borrow_data().expect("certificate").to_vec();
        frame[8].try_borrow_mut_data().expect("page")[16] ^= 1;
        assert_eq!(
            execute(&program_id, &frame, consider_request()),
            Err(GeneralSbfError::Verification.into())
        );
        assert_eq!(
            frame[3].try_borrow_data().expect("selection").as_ref(),
            selection_before.as_slice()
        );
        assert_eq!(
            frame[4].try_borrow_data().expect("verification").as_ref(),
            verification_before.as_slice()
        );
        assert_eq!(
            frame[5].try_borrow_data().expect("certificate").as_ref(),
            certificate_before.as_slice()
        );

        frame[8].try_borrow_mut_data().expect("page")[16] ^= 1;
        execute(&program_id, &frame, consider_request()).expect("first consider");
        let snapshot = frame[3].try_borrow_data().expect("selection").to_vec();
        assert_eq!(
            execute(&program_id, &frame, consider_request()),
            Err(GeneralSbfError::Verification.into())
        );
        assert_eq!(
            frame[3].try_borrow_data().expect("selection").as_ref(),
            snapshot.as_slice()
        );
    }

    #[test]
    fn freeze_then_initialize_enters_zero_inventory_collecting_phase() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let consider = consider_frame(program_id, market);
        execute(&program_id, &consider, consider_request()).expect("consider");

        let selection_bytes = consider[3].try_borrow_data().expect("selection").to_vec();
        let selection_key = *consider[3].key;
        let mut freeze = common(program_id, market);
        freeze.push(account(
            selection_key,
            true,
            selection_bytes,
            program_id,
            false,
        ));
        let freeze_request = ControllerRequestV1 {
            action: Action::Freeze,
            expected_revision: 1,
            candidate_id: None,
            page_index: 0,
            execution_index: 0,
        };
        execute(&program_id, &freeze, freeze_request).expect("freeze");

        let candidate = candidate();
        let settlement_key = Pubkey::find_program_address(
            &[
                GENERAL_SETTLEMENT_PDA_DOMAIN_V1,
                market.as_ref(),
                &candidate.candidate_id,
            ],
            &program_id,
        )
        .0;
        let mut initialize = common(program_id, market);
        initialize.extend([
            account(
                selection_key,
                false,
                freeze[3].try_borrow_data().expect("selection").to_vec(),
                program_id,
                false,
            ),
            account(
                settlement_key,
                true,
                vec![0; SETTLEMENT_CURSOR_BYTES],
                program_id,
                false,
            ),
            account(
                *consider[5].key,
                false,
                consider[5].try_borrow_data().expect("certificate").to_vec(),
                program_id,
                false,
            ),
            account(
                *consider[6].key,
                false,
                consider[6].try_borrow_data().expect("candidate").to_vec(),
                program_id,
                false,
            ),
        ]);
        let request = ControllerRequestV1 {
            action: Action::InitializeSettlement,
            expected_revision: 0,
            candidate_id: Some(candidate.candidate_id),
            page_index: 0,
            execution_index: 0,
        };
        execute(&program_id, &initialize, request).expect("initialize");
        let settlement =
            SettlementCursorV1::decode(&initialize[4].try_borrow_data().expect("settlement"))
                .expect("settlement cursor");
        assert_eq!(settlement.phase, Phase::Collecting);
        assert_eq!(settlement.next_page, 0);
        assert_eq!(settlement.next_execution, 0);
        assert_eq!(settlement.claim_inventory, [0; MAX_OUTCOMES]);
        assert_eq!(settlement.quote_inventory, 0);
    }

    fn activation_fixture(
        program_id: Pubkey,
        core_program: Pubkey,
        market: Pubkey,
    ) -> (
        Vec<AccountInfo<'static>>,
        Vec<u8>,
        CoreEffectEnvelopeV1,
        GeneralActivationRequestV2,
    ) {
        let config = config();
        let config_bytes = config.to_bytes();
        let config_id = hash(&config_bytes).to_bytes();
        let generation = config.generation().to_le_bytes();
        let root_key = Pubkey::find_program_address(
            &[
                GENERAL_ROOT_PDA_DOMAIN_V2,
                market.as_ref(),
                &generation,
                &config_id,
            ],
            &program_id,
        )
        .0;
        let native_rent = CompartmentFundingV1::native_lamports(1).expect("native Rent");
        let not_applicable = CompartmentFundingV1::not_applicable();
        let quote = FundingQuoteV1::new(
            FundingAmountsV1::new(
                native_rent,
                not_applicable,
                not_applicable,
                not_applicable,
                not_applicable,
                not_applicable,
                not_applicable,
            )
            .expect("funding amounts"),
            None,
        )
        .expect("funding quote");
        let entry = CapabilityEntryV1::new(
            content(GENERAL_CAPABILITY_KIND_ID_V1),
            content(GENERAL_CAPABILITY_RELEASE_ID_V2),
            content(config_id),
            content(config.capacity_profile_id()),
            content(GENERAL_CHILD_SCHEMA_ID_V2),
            content(GENERAL_CHILD_DERIVATION_ID_V2),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote,
        )
        .expect("General entry");
        let mut manifest_bytes = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes).expect("manifest encodes");
        let manifest_id = content(hash(&manifest_bytes).to_bytes());
        let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest");
        let pending_custody =
            FundingCustodyObservationV1::native_only(2, 1).expect("pending funding custody");
        let mut funding = FundingStateV1::new(manifest_id, manifest, 0, pending_custody)
            .expect("pending funding");
        funding
            .activate(manifest_id, manifest, pending_custody, 44)
            .expect("active funding");
        let funding_derivation = CapabilityFundingDerivationV1::new(
            market.to_bytes(),
            config.generation(),
            manifest_id,
            manifest,
            funding,
        )
        .expect("funding derivation");
        let funding_key =
            Pubkey::find_program_address(&funding_derivation.seed_components(), &program_id).0;
        let rent_credit = Pubkey::new_unique();
        let request = GeneralActivationRequestV2::new(
            root_key.to_bytes(),
            config_id,
            manifest_id.to_bytes(),
            funding_key.to_bytes(),
            rent_credit.to_bytes(),
            0,
            44,
            1,
            1,
        )
        .expect("activation request");
        let request_bytes = request.to_bytes();
        let request_digest = core_identity(hash(&request_bytes).to_bytes());
        let release = core_identity(id(71));
        let context = core_identity(id(72));
        let parent = core_identity(id(73));
        let provisional = CoreEffectEnvelopeV1::new(
            CoreEffectActionV1::ActivateCapability,
            Role::Trading,
            core_identity(core_program.to_bytes()),
            core_identity(id(74)),
            release,
            core_identity(market.to_bytes()),
            context,
            parent,
            request_digest,
            config.generation(),
            0,
            0,
            u32::try_from(request_bytes.len()).expect("request width"),
        )
        .expect("provisional envelope");
        let authority = Pubkey::find_program_address(
            &provisional
                .caller_authority_seeds()
                .expect("authority seeds")
                .as_slices(),
            &core_program,
        )
        .0;
        let envelope = CoreEffectEnvelopeV1::new(
            CoreEffectActionV1::ActivateCapability,
            Role::Trading,
            core_identity(core_program.to_bytes()),
            core_identity(authority.to_bytes()),
            release,
            core_identity(market.to_bytes()),
            context,
            parent,
            request_digest,
            config.generation(),
            0,
            0,
            u32::try_from(request_bytes.len()).expect("request width"),
        )
        .expect("envelope");
        let root =
            GeneralRootV2::active(market.to_bytes(), config_id, config.generation()).expect("root");
        let accounts = vec![
            account_with(
                authority,
                true,
                false,
                1,
                Vec::new(),
                system_program::ID,
                false,
            ),
            account_with(
                core_program,
                false,
                false,
                1,
                Vec::new(),
                Pubkey::new_unique(),
                true,
            ),
            account_with(
                root_key,
                false,
                true,
                1,
                root.to_bytes().to_vec(),
                program_id,
                false,
            ),
            account_with(
                Pubkey::new_unique(),
                false,
                false,
                1,
                config_bytes.to_vec(),
                Pubkey::new_unique(),
                false,
            ),
            account_with(
                Pubkey::new_unique(),
                false,
                false,
                1,
                manifest_bytes,
                Pubkey::new_unique(),
                false,
            ),
            account_with(
                funding_key,
                false,
                true,
                1,
                funding.to_bytes().to_vec(),
                program_id,
                false,
            ),
            account_with(
                rent_credit,
                false,
                true,
                1,
                Vec::new(),
                system_program::ID,
                false,
            ),
            account_with(
                system_program::ID,
                false,
                false,
                1,
                Vec::new(),
                Pubkey::new_unique(),
                true,
            ),
        ];
        let mut instruction = Vec::from(envelope.encode().expect("envelope bytes"));
        instruction.extend_from_slice(&request_bytes);
        (accounts, instruction, envelope, request)
    }

    #[test]
    fn exact_core_activation_replay_returns_authenticated_ack() {
        let program_id = Pubkey::new_unique();
        let core_program = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let (accounts, instruction, envelope, _request) =
            activation_fixture(program_id, core_program, market);
        let root_before = accounts[2].try_borrow_data().expect("root").to_vec();
        process_instruction(&program_id, &accounts, &instruction).expect("activation replay");
        assert_eq!(
            accounts[2].try_borrow_data().expect("root").as_ref(),
            root_before.as_slice()
        );
        let root = GeneralRootV2::decode(&accounts[2].try_borrow_data().expect("root"))
            .expect("decode root");
        let funding = FundingStateV1::decode(&accounts[5].try_borrow_data().expect("funding"))
            .expect("decode funding");
        let ack = activation_handler::activation_ack(
            &program_id,
            envelope,
            &instruction[..CORE_EFFECT_ENVELOPE_BYTES_V1],
            &instruction[CORE_EFFECT_ENVELOPE_BYTES_V1..],
            root,
            funding,
        )
        .expect("activation ack");
        let envelope_length = u32::try_from(CORE_EFFECT_ENVELOPE_BYTES_V1)
            .expect("envelope width")
            .to_le_bytes();
        let request_length = u32::try_from(GENERAL_ACTIVATION_REQUEST_BYTES_V2)
            .expect("request width")
            .to_le_bytes();
        let effect_digest = core_identity(
            hashv(&[
                &CORE_EFFECT_DIGEST_DOMAIN_V1,
                &envelope_length,
                &instruction[..CORE_EFFECT_ENVELOPE_BYTES_V1],
                &request_length,
                &instruction[CORE_EFFECT_ENVELOPE_BYTES_V1..],
            ])
            .to_bytes(),
        );
        ack.validate_for(
            envelope,
            core_identity(program_id.to_bytes()),
            effect_digest,
        )
        .expect("ack validates");
        assert_eq!(ack.pre_resource_a_revision(), 0);
        assert_eq!(ack.post_resource_a_revision(), 1);
        assert_eq!(ack.pre_resource_b_revision(), 0);
        assert_eq!(ack.post_resource_b_revision(), 1);
    }

    #[test]
    fn hostile_core_authority_manifest_config_and_rent_refuse_without_state_change() {
        let program_id = Pubkey::new_unique();
        let core_program = Pubkey::new_unique();
        let market = Pubkey::new_unique();

        let (mut authority_substitution, instruction, _, _) =
            activation_fixture(program_id, core_program, market);
        let root_before = authority_substitution[2]
            .try_borrow_data()
            .expect("root")
            .to_vec();
        let funding_before = authority_substitution[5]
            .try_borrow_data()
            .expect("funding")
            .to_vec();
        authority_substitution[0] = account_with(
            Pubkey::new_unique(),
            true,
            false,
            1,
            Vec::new(),
            system_program::ID,
            false,
        );
        assert_eq!(
            process_instruction(&program_id, &authority_substitution, &instruction),
            Err(GeneralSbfError::RootActivation.into())
        );
        assert_eq!(
            authority_substitution[2]
                .try_borrow_data()
                .expect("unchanged root")
                .as_ref(),
            root_before.as_slice()
        );
        assert_eq!(
            authority_substitution[5]
                .try_borrow_data()
                .expect("unchanged funding")
                .as_ref(),
            funding_before.as_slice()
        );

        let (manifest_substitution, instruction, _, _) =
            activation_fixture(program_id, core_program, market);
        let root_before = manifest_substitution[2]
            .try_borrow_data()
            .expect("root")
            .to_vec();
        let funding_before = manifest_substitution[5]
            .try_borrow_data()
            .expect("funding")
            .to_vec();
        manifest_substitution[4]
            .try_borrow_mut_data()
            .expect("manifest")
            .get_mut(16)
            .expect("manifest entry")
            .clone_from(&0x7f);
        assert_eq!(
            process_instruction(&program_id, &manifest_substitution, &instruction),
            Err(GeneralSbfError::RootActivation.into())
        );
        assert_eq!(
            manifest_substitution[2]
                .try_borrow_data()
                .expect("unchanged root")
                .as_ref(),
            root_before.as_slice()
        );
        assert_eq!(
            manifest_substitution[5]
                .try_borrow_data()
                .expect("unchanged funding")
                .as_ref(),
            funding_before.as_slice()
        );

        let (config_substitution, instruction, _, _) =
            activation_fixture(program_id, core_program, market);
        let root_before = config_substitution[2]
            .try_borrow_data()
            .expect("root")
            .to_vec();
        let funding_before = config_substitution[5]
            .try_borrow_data()
            .expect("funding")
            .to_vec();
        config_substitution[3]
            .try_borrow_mut_data()
            .expect("config")
            .get_mut(168)
            .expect("policy byte")
            .clone_from(&52);
        assert_eq!(
            process_instruction(&program_id, &config_substitution, &instruction),
            Err(GeneralSbfError::RootActivation.into())
        );
        assert_eq!(
            config_substitution[2]
                .try_borrow_data()
                .expect("unchanged root")
                .as_ref(),
            root_before.as_slice()
        );
        assert_eq!(
            config_substitution[5]
                .try_borrow_data()
                .expect("unchanged funding")
                .as_ref(),
            funding_before.as_slice()
        );

        let (mut rent_substitution, instruction, _, _) =
            activation_fixture(program_id, core_program, market);
        let rent_root_key = *rent_substitution[2].key;
        let rent_root_bytes = rent_substitution[2]
            .try_borrow_data()
            .expect("root")
            .to_vec();
        rent_substitution[2] = account_with(
            rent_root_key,
            false,
            true,
            2,
            rent_root_bytes,
            program_id,
            false,
        );
        let root_before = rent_substitution[2]
            .try_borrow_data()
            .expect("root")
            .to_vec();
        let funding_before = rent_substitution[5]
            .try_borrow_data()
            .expect("funding")
            .to_vec();
        assert_eq!(
            process_instruction(&program_id, &rent_substitution, &instruction),
            Err(GeneralSbfError::RootActivation.into())
        );
        assert_eq!(
            rent_substitution[2]
                .try_borrow_data()
                .expect("unchanged root")
                .as_ref(),
            root_before.as_slice()
        );
        assert_eq!(
            rent_substitution[5]
                .try_borrow_data()
                .expect("unchanged funding")
                .as_ref(),
            funding_before.as_slice()
        );
    }
}
