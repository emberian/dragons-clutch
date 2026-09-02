//! Generic authoritative admitted-AOT execution for the common Trading V3 outer.
//!
//! The current Registry-authenticated Strategy, Certificate, Admission,
//! ArtifactRelease, and Loader V3 observation are supplied by
//! [`crate::execution_strategy_v2`]. This module gives the selected stateless
//! accelerator no account-write or child-CPI authority: every accelerator CPI
//! account is read-only, its complete candidate bank is returned in canonical
//! authenticated chunks, and Trading remains the sole EffectProgram projector,
//! child caller, and commit-last state writer.
//!
//! ## Every pre-CPI refusal here is named, and none of them is `Content`
//!
//! `Content` has 2,126 sites in this program, and until 2026-09-01 the honest
//! equity Add and a hostile Position substitution both refused with it, from
//! this route, before the accelerator was invoked. That makes the hostile
//! assertion at `accepted.rs` a universal donor -- it passes on whatever the
//! transaction refuses first -- and it makes the honest wall unlocalizable.
//!
//! Decision 0007 says split rather than weaken. So the pre-CPI boundary now
//! carries three codes, and which one a log names says which boundary refused:
//!
//! - `AdmittedFrame` (0x4017): the authenticated frame, and the five Registry
//!   record pairs beneath it.
//! - `AdmittedTransport` (0x4018): register-bank encoding, chunk
//!   classification, caller-authority width, the authenticated input scratch
//!   pages, and the request that is handed to the CPI.
//! - `AdmittedContext` (0x4019): the invocation-context digest.
//!
//! `Content` survives here only AFTER the accelerator has answered, in
//! `decode_register_bank`, where it means the returned bank is malformed.

extern crate alloc;


use alloc::{vec, vec::Vec};

use dclutch_capability_program_contract::{
    hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3, v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::{
    admitted_v3::{AdmittedInvocationContextV3, admitted_invocation_context_digest_v3},
    v2::{
        ACCELERATOR_ACK_HEADER_BYTES_V2, ACCELERATOR_REQUEST_HEADER_BYTES_V2, AcceleratorAckV2,
        AcceleratorDispositionV2, AcceleratorRequestV2, AcceleratorTransportProfileV2,
        AuthenticatedScratchPageV2, BankTransportV2, EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2,
        EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
        ExecutionCandidateV2, RequestTransportV2, StrategyDispositionV2,
        classify_bank_transport_v2, register_bank_bytes_v2, resolve_execution_candidate_v2,
    },
};
use dclutch_record_contract::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_registry_contract::ARTIFACT_RELEASE_SCHEMA_ID_V1;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::sysvar;

use crate::{TradingSbfError, execution_strategy_v2::AuthenticatedExecutionStrategyV2};

const ADMITTED_ACK_TRANSCRIPT_DOMAIN_V3: &[u8] = b"dclutch:hot-admitted-ack:v3";

/// Caller authority in the authenticated accelerator V4 CPI frame.
pub const ADMITTED_ACCELERATOR_CALLER_AUTHORITY_ACCOUNT_V4: usize = 0;
/// First account of the exact common Hot fixed frame.
pub const ADMITTED_ACCELERATOR_HOT_FIXED_START_V4: usize = 1;
/// Exact number of common Hot fixed accounts carried read-only into the accelerator.
///
/// This is the common Hot fixed frame, entire — so it is DERIVED from the
/// contract's own count and never written as a literal again. It was a literal
/// `38`, correct when `b280850f` wrote it and wrong from `ca5e5f14` onward: that
/// later commit added `HOT_CAPABILITY_SEAL_ACCOUNT_V3` at index 38 and moved
/// `HOT_FIXED_ACCOUNT_COUNT_V3` to 39, and this copy did not follow.
///
/// A one-off drift, but it closed the admitted-accelerator lane from BOTH ends:
/// the producer hands `validate_authenticated_frame` exactly
/// `HOT_FIXED_ACCOUNT_COUNT_V3` accounts and the length check refused all 39 of
/// them, while the accelerator's own `authenticate_accelerator_invocation_v4`
/// sliced 38 out and handed them to `parse_accelerator_readonly`, which demands
/// 39 (and reads index 38 for the capability seal, so 38 could never suffice).
pub const ADMITTED_ACCELERATOR_HOT_FIXED_COUNT_V4: usize = HOT_FIXED_ACCOUNT_COUNT_V3;
/// First strategy-owned Certificate/Admission/Artifact/deployment evidence account.
pub const ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_START_V4: usize =
    ADMITTED_ACCELERATOR_HOT_FIXED_START_V4 + ADMITTED_ACCELERATOR_HOT_FIXED_COUNT_V4;
/// Exact eight-account admitted strategy suffix, including Program/ProgramData.
pub const ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4: usize = 8;
/// First expanded AccountProfile-ordered logical runtime observation.
pub const ADMITTED_ACCELERATOR_RUNTIME_ACCOUNTS_START_V4: usize =
    ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_START_V4
        + ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4;

/// Exact fixed evidence passed before the AccountProfile-ordered runtime slice.
#[derive(Clone, Copy)]
pub struct AdmittedCpiFrameV3<'a, 'info> {
    /// One release-pinned Trading authority per canonical output chunk.
    pub caller_authorities: &'a [AccountInfo<'info>],
    /// Exact common Hot fixed frame, downgraded read-only for the accelerator.
    pub hot_fixed_accounts: &'a [AccountInfo<'info>],
    /// Current release-set activation cache.
    pub activation: &'a AccountInfo<'info>,
    /// Immutable executable Registry.
    pub registry: &'a AccountInfo<'info>,
    /// Current Rent sysvar.
    pub rent: &'a AccountInfo<'info>,
    /// Instructions sysvar containing the exact top-level Trading request.
    pub instructions: &'a AccountInfo<'info>,
    /// Current executable Trading program.
    pub trading_program: &'a AccountInfo<'info>,
    /// Current Trading ProgramData.
    pub trading_programdata: &'a AccountInfo<'info>,
    /// Finalized CapabilityProgram raw record.
    pub capability_raw: &'a AccountInfo<'info>,
    /// Vacant CapabilityProgram staging cursor.
    pub capability_staging: &'a AccountInfo<'info>,
    /// Finalized ExecutionStrategy raw record.
    pub strategy_raw: &'a AccountInfo<'info>,
    /// Vacant ExecutionStrategy staging cursor.
    pub strategy_staging: &'a AccountInfo<'info>,
    /// Finalized translation Certificate raw record.
    pub certificate_raw: &'a AccountInfo<'info>,
    /// Vacant Certificate staging cursor.
    pub certificate_staging: &'a AccountInfo<'info>,
    /// Finalized Registry Admission raw record.
    pub admission_raw: &'a AccountInfo<'info>,
    /// Vacant Admission staging cursor.
    pub admission_staging: &'a AccountInfo<'info>,
    /// Finalized immutable ArtifactRelease raw record.
    pub artifact_raw: &'a AccountInfo<'info>,
    /// Vacant ArtifactRelease staging cursor.
    pub artifact_staging: &'a AccountInfo<'info>,
    /// Exact admitted executable accelerator program.
    pub accelerator_program: &'a AccountInfo<'info>,
    /// Exact admitted accelerator ProgramData.
    pub accelerator_programdata: &'a AccountInfo<'info>,
}

/// Complete authoritative admitted candidate plus its ordered CPI transcript.
pub struct AdmittedExecutionV3 {
    /// Authoritative complete scalar candidate bank.
    pub scalars: Vec<u64>,
    /// Authoritative complete identity candidate bank.
    pub identities: Vec<[u8; 32]>,
    /// Ordered commitment to every exact accelerator request and acknowledgement.
    pub transcript_digest: [u8; 32],
}

/// Return the exact caller-authority count for an admitted candidate bank.
pub fn admitted_caller_authority_count_v3(
    scalar_count: u32,
    identity_count: u32,
) -> Result<usize, ProgramError> {
    match classify_bank_transport_v2(scalar_count, identity_count)
        .map_err(|_| TradingSbfError::AdmittedTransport)?
    {
        BankTransportV2::InlineReturnData { bank_bytes } => {
            if bank_bytes == 0 {
                Err(TradingSbfError::AdmittedTransport.into())
            } else {
                Ok(1)
            }
        }
        BankTransportV2::AuthenticatedScratchPages { page_count, .. } => {
            usize::try_from(page_count).map_err(|_| TradingSbfError::AdmittedTransport.into())
        }
    }
}

/// Execute an admitted accelerator as the sole candidate authority.
///
/// `input_scratch_pages` is empty for an inline bank. For a multi-chunk bank it
/// must contain the complete ordered Trading-owned input page sequence, and
/// every page must also occur in `runtime_accounts` so the accelerator observes
/// the exact AccountProfile transcript rather than a hidden side channel.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn execute_admitted_aot_v3<'info>(
    program_id: &Pubkey,
    frame: AdmittedCpiFrameV3<'_, 'info>,
    runtime_accounts: &[&AccountInfo<'info>],
    input_scratch_pages: &[&AccountInfo<'info>],
    context: &AdmittedInvocationContextV3,
    authenticated: AuthenticatedExecutionStrategyV2,
    input_scalars: &[u64],
    input_identities: &[[u8; 32]],
) -> Result<AdmittedExecutionV3, ProgramError> {
    let invocation_context =
        admitted_invocation_context_digest_v3(*context).map_err(|_| TradingSbfError::AdmittedContext)?;
    validate_authenticated_frame(program_id, frame, runtime_accounts, context, authenticated)?;
    let scalar_count = u32::try_from(input_scalars.len()).map_err(|_| TradingSbfError::AdmittedTransport)?;
    let identity_count =
        u32::try_from(input_identities.len()).map_err(|_| TradingSbfError::AdmittedTransport)?;
    if scalar_count != context.scalar_count || identity_count != context.identity_count {
        return Err(TradingSbfError::AdmittedTransport.into());
    }
    let input_bank = encode_register_bank(input_scalars, input_identities)?;
    let input_bank_digest = content(&input_bank)?;
    let chunk_count = match classify_bank_transport_v2(scalar_count, identity_count)
        .map_err(|_| TradingSbfError::AdmittedTransport)?
    {
        BankTransportV2::InlineReturnData { bank_bytes } => {
            if bank_bytes == 0 {
                return Err(TradingSbfError::AdmittedTransport.into());
            }
            1_usize
        }
        BankTransportV2::AuthenticatedScratchPages {
            bank_bytes,
            page_count,
        } => {
            if usize::try_from(bank_bytes).map_err(|_| TradingSbfError::AdmittedTransport)?
                != input_bank.len()
            {
                return Err(TradingSbfError::AdmittedTransport.into());
            }
            usize::try_from(page_count).map_err(|_| TradingSbfError::AdmittedTransport)?
        }
    };
    // Input transport and output return-data chunking are orthogonal. The V2
    // contract deliberately permits a complete inline input bank to yield
    // several bounded output acknowledgements. Scratch input is selected only
    // when the authenticated outer supplies its complete canonical page set.
    let transport = if input_scratch_pages.is_empty() {
        RequestTransportV2::Inline
    } else {
        RequestTransportV2::ScratchPages
    };
    if frame.caller_authorities.len() != chunk_count {
        return Err(TradingSbfError::AdmittedTransport.into());
    }
    let first_request = accelerator_request(
        transport,
        authenticated,
        invocation_context,
        input_bank_digest,
        context.tail_count,
        scalar_count,
        identity_count,
        0,
        &input_bank,
    )?;
    if transport == RequestTransportV2::ScratchPages {
        validate_input_scratch_pages(
            program_id,
            runtime_accounts,
            input_scratch_pages,
            first_request,
            &input_bank,
        )?;
    }

    let mut candidate = vec![0_u8; input_bank.len()];
    let mut accepted_digest = None;
    let mut transcript = hash(ADMITTED_ACK_TRANSCRIPT_DOMAIN_V3).to_bytes();
    for chunk_index in 0..chunk_count {
        let chunk_index_u32 = u32::try_from(chunk_index).map_err(|_| TradingSbfError::AdmittedTransport)?;
        let request = accelerator_request(
            transport,
            authenticated,
            invocation_context,
            input_bank_digest,
            context.tail_count,
            scalar_count,
            identity_count,
            chunk_index_u32,
            &input_bank,
        )?;
        let authority = frame
            .caller_authorities
            .get(chunk_index)
            .ok_or(TradingSbfError::AdmittedTransport)?;
        let (ack_bytes, request_digest) = invoke_admitted_chunk(
            program_id,
            frame,
            runtime_accounts,
            authority,
            context,
            request,
        )?;
        let ack = AcceleratorAckV2::decode(&ack_bytes).map_err(|_| TradingSbfError::Transition)?;
        ack.validate_request(request, request_digest)
            .map_err(|_| TradingSbfError::Transition)?;
        if ack.disposition() != AcceleratorDispositionV2::Accepted {
            return Err(TradingSbfError::Transition.into());
        }
        let bank_digest = ack.total_bank_digest().ok_or(TradingSbfError::Transition)?;
        if accepted_digest.is_some_and(|expected| expected != bank_digest) {
            return Err(TradingSbfError::Transition.into());
        }
        accepted_digest = Some(bank_digest);
        let start = usize::try_from(ack.chunk_offset()).map_err(|_| TradingSbfError::Transition)?;
        let end = start
            .checked_add(ack.payload().len())
            .ok_or(TradingSbfError::Transition)?;
        candidate
            .get_mut(start..end)
            .ok_or(TradingSbfError::Transition)?
            .copy_from_slice(ack.payload());
        transcript = hashv(&[
            ADMITTED_ACK_TRANSCRIPT_DOMAIN_V3,
            &transcript,
            ack.request_digest().as_bytes(),
            &ack_bytes,
        ])
        .to_bytes();
    }
    if accepted_digest != Some(content(&candidate)?) {
        return Err(TradingSbfError::Transition.into());
    }
    let admitted = authenticated
        .admitted_authorization()
        .ok_or(TradingSbfError::AdmittedFrame)?;
    let resolved = resolve_execution_candidate_v2(
        StrategyDispositionV2::AdmittedAot,
        None,
        Some(ExecutionCandidateV2::Accepted(&candidate)),
        Some(admitted),
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let ExecutionCandidateV2::Accepted(bank) = resolved else {
        return Err(TradingSbfError::Transition.into());
    };
    let (scalars, identities) = decode_register_bank(bank, scalar_count, identity_count)?;
    Ok(AdmittedExecutionV3 {
        scalars,
        identities,
        transcript_digest: transcript,
    })
}

#[allow(clippy::too_many_arguments)]
fn accelerator_request<'a>(
    transport: RequestTransportV2,
    authenticated: AuthenticatedExecutionStrategyV2,
    invocation_context: ContentId,
    input_bank_digest: ContentId,
    tail_count: u32,
    scalar_count: u32,
    identity_count: u32,
    chunk_index: u32,
    input_bank: &'a [u8],
) -> Result<AcceleratorRequestV2<'a>, ProgramError> {
    AcceleratorRequestV2::new(
        transport,
        authenticated.strategy_program_id(),
        authenticated
            .certificate_program_id()
            .ok_or(TradingSbfError::AdmittedTransport)?,
        authenticated.capability_program_id(),
        invocation_context,
        input_bank_digest,
        tail_count,
        scalar_count,
        identity_count,
        chunk_index,
        match transport {
            RequestTransportV2::Inline => input_bank,
            RequestTransportV2::ScratchPages => &[],
        },
    )
    .map_err(|_| TradingSbfError::AdmittedTransport.into())
}

#[inline(never)]
fn invoke_admitted_chunk<'info>(
    program_id: &Pubkey,
    frame: AdmittedCpiFrameV3<'_, 'info>,
    runtime_accounts: &[&AccountInfo<'info>],
    caller_authority: &AccountInfo<'info>,
    context: &AdmittedInvocationContextV3,
    request: AcceleratorRequestV2<'_>,
) -> Result<(Vec<u8>, ContentId), ProgramError> {
    let request_len = ACCELERATOR_REQUEST_HEADER_BYTES_V2
        .checked_add(request.inline_bank().len())
        .ok_or(TradingSbfError::AdmittedTransport)?;
    let mut request_bytes = vec![0_u8; request_len];
    request
        .encode_into(&mut request_bytes)
        .map_err(|_| TradingSbfError::AdmittedTransport)?;
    let request_digest = content(&request_bytes)?;
    let authority_seeds = CallerAuthoritySeedsV1::new(
        context.release_set,
        context.market.to_bytes(),
        ExecutionRoleV1::Trading,
        context.root.to_bytes(),
        request_digest.to_bytes(),
    )
    .map_err(|_| TradingSbfError::AdmittedTransport)?;
    let (expected_authority, bump) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    if caller_authority.key != &expected_authority
        || caller_authority.is_signer
        || caller_authority.is_writable
        || caller_authority.executable
    {
        return Err(TradingSbfError::Release.into());
    }
    let mut metas = Vec::with_capacity(
        ADMITTED_ACCELERATOR_RUNTIME_ACCOUNTS_START_V4
            .checked_add(runtime_accounts.len())
            .ok_or(TradingSbfError::AdmittedTransport)?,
    );
    metas.extend(
        fixed_cpi_accounts(frame, caller_authority)
            .enumerate()
            .map(|(index, account)| AccountMeta::new_readonly(*account.key, index == 0)),
    );
    metas.extend(
        runtime_accounts
            .iter()
            .map(|account| AccountMeta::new_readonly(*account.key, false)),
    );
    let instruction = Instruction {
        program_id: *frame.accelerator_program.key,
        accounts: metas,
        data: request_bytes,
    };
    let mut infos = Vec::with_capacity(
        ADMITTED_ACCELERATOR_RUNTIME_ACCOUNTS_START_V4
            .checked_add(runtime_accounts.len())
            .ok_or(TradingSbfError::AdmittedTransport)?,
    );
    infos.extend(fixed_cpi_accounts(frame, caller_authority).cloned());
    infos.extend(runtime_accounts.iter().map(|account| (*account).clone()));
    let bump_seed = [bump];
    let [domain, release, market, role, authority_context, digest] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[
            domain,
            release,
            market,
            role,
            authority_context,
            digest,
            &bump_seed,
        ]],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let (producer, ack_bytes) = get_return_data().ok_or(TradingSbfError::Transition)?;
    if producer != *frame.accelerator_program.key
        || ack_bytes.len() < ACCELERATOR_ACK_HEADER_BYTES_V2
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok((ack_bytes, request_digest))
}

fn fixed_cpi_accounts<'a, 'info>(
    frame: AdmittedCpiFrameV3<'a, 'info>,
    authority: &'a AccountInfo<'info>,
) -> impl Iterator<Item = &'a AccountInfo<'info>> {
    core::iter::once(authority)
        .chain(frame.hot_fixed_accounts.iter())
        .chain([
            frame.certificate_raw,
            frame.certificate_staging,
            frame.admission_raw,
            frame.admission_staging,
            frame.artifact_raw,
            frame.artifact_staging,
            frame.accelerator_program,
            frame.accelerator_programdata,
        ])
}

fn validate_authenticated_frame(
    program_id: &Pubkey,
    frame: AdmittedCpiFrameV3<'_, '_>,
    runtime_accounts: &[&AccountInfo<'_>],
    context: &AdmittedInvocationContextV3,
    authenticated: AuthenticatedExecutionStrategyV2,
) -> Result<(), ProgramError> {
    let descriptor = authenticated.capability_program();
    let strategy = authenticated.strategy();
    let artifact = authenticated
        .artifact_release()
        .ok_or(TradingSbfError::AdmittedFrame)?;
    if frame.hot_fixed_accounts.len() != ADMITTED_ACCELERATOR_HOT_FIXED_COUNT_V4
        || strategy.disposition() != StrategyDispositionV2::AdmittedAot
        || strategy
            .transport_profile()
            .map_err(|_| TradingSbfError::AdmittedFrame)?
            != AcceleratorTransportProfileV2::ChunkedBankV2
        || authenticated.admitted_authorization().is_none()
        || context.registry_program.to_bytes() != frame.registry.key.to_bytes()
        || context.trading_program.to_bytes() != program_id.to_bytes()
        || context.accelerator_program.to_bytes() != frame.accelerator_program.key.to_bytes()
        || context.capability_program != authenticated.capability_program_id()
        || context.account_profile != descriptor.account_profile().program()
        || context.request_profile != descriptor.request_profile().program()
        || context.transition != strategy.transition_program()
        || context.effect != descriptor.effect().program()
        || context.lifecycle != descriptor.derivation_policy()
        || context.strategy != authenticated.strategy_program_id()
        || Some(context.certificate) != authenticated.certificate_program_id()
        || Some(context.admission) != authenticated.admission_program_id()
        || context.artifact_release
            != authenticated
                .artifact_release_id()
                .ok_or(TradingSbfError::AdmittedFrame)?
        || !exact_deployment_keys_v3(
            artifact.program().to_bytes(),
            artifact.programdata(),
            frame.accelerator_program.key.to_bytes(),
            frame.accelerator_programdata.key.to_bytes(),
        )
        || usize::try_from(context.account_count).map_err(|_| TradingSbfError::AdmittedFrame)?
            != runtime_accounts.len()
        || !frame.registry.executable
        || frame.registry.is_signer
        || frame.registry.is_writable
        || frame.rent.key != &sysvar::rent::ID
        || frame.instructions.key != &sysvar::instructions::ID
        || frame.trading_program.key != program_id
        || !frame.trading_program.executable
        || frame.trading_program.is_signer
        || frame.trading_program.is_writable
        || frame.trading_programdata.executable
        || frame.trading_programdata.is_signer
        || frame.trading_programdata.is_writable
        || !frame.accelerator_program.executable
        || frame.accelerator_program.is_signer
        || frame.accelerator_program.is_writable
        || frame.accelerator_programdata.executable
        || frame.accelerator_programdata.is_signer
        || frame.accelerator_programdata.is_writable
    {
        return Err(TradingSbfError::Release.into());
    }
    // Each raw record and its staging cursor are ONE Registry identity under two
    // domains, so they are checked as a pair and can no longer be given halves of
    // it. See `require_record_pair`.
    require_record_pair(
        frame.registry.key,
        frame.capability_raw.key,
        frame.capability_staging.key,
        CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        authenticated.capability_program_id().to_bytes(),
    )?;
    require_record_pair(
        frame.registry.key,
        frame.strategy_raw.key,
        frame.strategy_staging.key,
        EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
        authenticated.strategy_program_id().to_bytes(),
    )?;
    require_record_pair(
        frame.registry.key,
        frame.certificate_raw.key,
        frame.certificate_staging.key,
        EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
        context.certificate.to_bytes(),
    )?;
    require_record_pair(
        frame.registry.key,
        frame.admission_raw.key,
        frame.admission_staging.key,
        EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2,
        context.admission.to_bytes(),
    )?;
    require_record_pair(
        frame.registry.key,
        frame.artifact_raw.key,
        frame.artifact_staging.key,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        context.artifact_release.to_bytes(),
    )?;
    Ok(())
}

fn exact_deployment_keys_v3(
    admitted_program: [u8; 32],
    admitted_programdata: [u8; 32],
    observed_program: [u8; 32],
    observed_programdata: [u8; 32],
) -> bool {
    admitted_program == observed_program && admitted_programdata == observed_programdata
}

/// Address of one Registry record under the seed order the Registry itself uses.
///
/// The seeds are taken from [`RecordKeyV1`] rather than spelled here, because a
/// Registry record address is the Registry's fact and this program is not a
/// second author of it.
fn record_address(registry: &Pubkey, seeds: RecordPdaSeedsV1) -> Pubkey {
    Pubkey::find_program_address(
        &[
            seeds.domain(),
            seeds.schema_release_id().as_bytes(),
            seeds.expected_digest().as_bytes(),
        ],
        registry,
    )
    .0
}

/// Require one finalized raw record and its vacant staging cursor to sit at the
/// two addresses the Registry derives for a single `(schema, digest)` identity.
///
/// **A record is keyed by schema/release AND digest, and both domains take the
/// same pair.** This took a schema-less `identity` and a bare `domain` until
/// 2026-08-29, deriving `[domain, identity]` where the Registry derives
/// `[domain, schema, digest]` (`dclutch_record_contract::raw_record_pda_seeds`,
/// `programs/dclutch-registry-sbf/src/record_v1.rs:537-551`). Because Solana
/// concatenates seed segments, a two-seed address commits 32 bytes of identity
/// material where the Registry commits 64, so no record the Registry can create
/// was ever at the address this checked — while `execution_strategy_v2`'s
/// `authenticate_finalized_record` and `hot_v3`'s `borrow_finalized_record_at`
/// derived the same records correctly in the same invocation.
///
/// The ten calls were also splitting one identity in half: the raw call passed
/// the digest and the staging call passed the schema, so neither had a whole
/// one. Taking the pair in a single argument list is what makes that
/// unexpressible rather than merely fixed.
fn require_record_pair(
    registry: &Pubkey,
    raw: &Pubkey,
    staging: &Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<(), ProgramError> {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema).map_err(|_| TradingSbfError::AdmittedFrame)?,
        ContentDigest::new(digest).map_err(|_| TradingSbfError::AdmittedFrame)?,
    );
    if raw != &record_address(registry, key.raw_record_pda_seeds())
        || staging != &record_address(registry, key.staging_cursor_pda_seeds())
    {
        return Err(TradingSbfError::AdmittedFrame.into());
    }
    Ok(())
}

fn validate_input_scratch_pages(
    program_id: &Pubkey,
    runtime_accounts: &[&AccountInfo<'_>],
    pages: &[&AccountInfo<'_>],
    request: AcceleratorRequestV2<'_>,
    input_bank: &[u8],
) -> Result<(), ProgramError> {
    if pages.len()
        != usize::try_from(request.chunk_count()).map_err(|_| TradingSbfError::AdmittedTransport)?
    {
        return Err(TradingSbfError::AdmittedTransport.into());
    }
    let trading = ContentId::new(program_id.to_bytes()).map_err(|_| TradingSbfError::AdmittedTransport)?;
    let mut cursor = 0_usize;
    for (index, account) in pages.iter().enumerate() {
        if account.owner != program_id
            || account.is_signer
            || account.executable
            || runtime_accounts
                .iter()
                .filter(|runtime| runtime.key == account.key)
                .count()
                != 1
        {
            return Err(TradingSbfError::AdmittedTransport.into());
        }
        let data = account
            .try_borrow_data()
            .map_err(|_| TradingSbfError::AdmittedTransport)?;
        let page =
            AuthenticatedScratchPageV2::decode(&data).map_err(|_| TradingSbfError::AdmittedTransport)?;
        page.validate_request_input(trading, request)
            .map_err(|_| TradingSbfError::AdmittedTransport)?;
        if usize::try_from(page.chunk_index()).map_err(|_| TradingSbfError::AdmittedTransport)? != index
            || usize::try_from(page.chunk_offset()).map_err(|_| TradingSbfError::AdmittedTransport)? != cursor
        {
            return Err(TradingSbfError::AdmittedTransport.into());
        }
        let end = cursor
            .checked_add(page.payload().len())
            .ok_or(TradingSbfError::AdmittedTransport)?;
        if input_bank.get(cursor..end) != Some(page.payload()) {
            return Err(TradingSbfError::AdmittedTransport.into());
        }
        cursor = end;
    }
    if cursor != input_bank.len() {
        return Err(TradingSbfError::AdmittedTransport.into());
    }
    Ok(())
}

fn encode_register_bank(scalars: &[u64], identities: &[[u8; 32]]) -> Result<Vec<u8>, ProgramError> {
    let expected = register_bank_bytes_v2(
        u32::try_from(scalars.len()).map_err(|_| TradingSbfError::AdmittedTransport)?,
        u32::try_from(identities.len()).map_err(|_| TradingSbfError::AdmittedTransport)?,
    )
    .map_err(|_| TradingSbfError::AdmittedTransport)?;
    let mut output =
        Vec::with_capacity(usize::try_from(expected).map_err(|_| TradingSbfError::AdmittedTransport)?);
    for scalar in scalars {
        output.extend_from_slice(&scalar.to_le_bytes());
    }
    for identity in identities {
        output.extend_from_slice(identity);
    }
    if output.len() != usize::try_from(expected).map_err(|_| TradingSbfError::AdmittedTransport)? {
        return Err(TradingSbfError::AdmittedTransport.into());
    }
    Ok(output)
}

fn decode_register_bank(
    bank: &[u8],
    scalar_count: u32,
    identity_count: u32,
) -> Result<(Vec<u64>, Vec<[u8; 32]>), ProgramError> {
    let expected = register_bank_bytes_v2(scalar_count, identity_count)
        .map_err(|_| TradingSbfError::Content)?;
    if usize::try_from(expected).map_err(|_| TradingSbfError::Content)? != bank.len() {
        return Err(TradingSbfError::Content.into());
    }
    let scalar_bytes = usize::try_from(scalar_count)
        .map_err(|_| TradingSbfError::Content)?
        .checked_mul(8)
        .ok_or(TradingSbfError::Content)?;
    let mut scalars =
        Vec::with_capacity(usize::try_from(scalar_count).map_err(|_| TradingSbfError::Content)?);
    for bytes in bank
        .get(..scalar_bytes)
        .ok_or(TradingSbfError::Content)?
        .chunks_exact(8)
    {
        scalars.push(u64::from_le_bytes(
            bytes.try_into().map_err(|_| TradingSbfError::Content)?,
        ));
    }
    let mut identities =
        Vec::with_capacity(usize::try_from(identity_count).map_err(|_| TradingSbfError::Content)?);
    for bytes in bank
        .get(scalar_bytes..)
        .ok_or(TradingSbfError::Content)?
        .chunks_exact(32)
    {
        identities.push(bytes.try_into().map_err(|_| TradingSbfError::Content)?);
    }
    if scalars.len() != usize::try_from(scalar_count).map_err(|_| TradingSbfError::Content)?
        || identities.len()
            != usize::try_from(identity_count).map_err(|_| TradingSbfError::Content)?
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok((scalars, identities))
}

fn content(bytes: &[u8]) -> Result<ContentId, ProgramError> {
    ContentId::new(hash(bytes).to_bytes()).map_err(|_| TradingSbfError::Content.into())
}

#[cfg(test)]
mod tests {
    use dclutch_record_contract::RAW_RECORD_PDA_SEED_V1;

    use super::*;

    #[test]
    fn authenticated_accelerator_v4_frame_is_exact_and_nonoverlapping() {
        assert_eq!(ADMITTED_ACCELERATOR_CALLER_AUTHORITY_ACCOUNT_V4, 0);
        assert_eq!(ADMITTED_ACCELERATOR_HOT_FIXED_START_V4, 1);
        assert_eq!(ADMITTED_ACCELERATOR_HOT_FIXED_COUNT_V4, 39);
        assert_eq!(ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_START_V4, 40);
        assert_eq!(ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4, 8);
        assert_eq!(ADMITTED_ACCELERATOR_RUNTIME_ACCOUNTS_START_V4, 48);

        // The literals above pin the layout. THIS is the assertion that pins it
        // to something: the accelerator carries the common Hot fixed frame
        // entire, so its count is the contract's count and not a number that
        // happens to match today.
        //
        // The literals alone were what let the frame count drift. They read as
        // a careful layout check and were three copies of one stale number, so
        // when `HOT_FIXED_ACCOUNT_COUNT_V3` went 38 -> 39 this test did not
        // catch the break — it asserted it, and stayed green over an
        // authentication path that could no longer succeed from either end.
        assert_eq!(
            ADMITTED_ACCELERATOR_HOT_FIXED_COUNT_V4,
            HOT_FIXED_ACCOUNT_COUNT_V3
        );
        assert_eq!(
            ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_START_V4,
            ADMITTED_ACCELERATOR_HOT_FIXED_START_V4 + HOT_FIXED_ACCOUNT_COUNT_V3
        );
    }

    #[test]
    fn register_bank_roundtrips_scalar_then_identity_without_narrowing() {
        let scalars = [0_u64, 1, u64::MAX];
        let identities = [[3_u8; 32], [4_u8; 32]];
        let bytes = encode_register_bank(&scalars, &identities).expect("encode");
        assert_eq!(
            decode_register_bank(&bytes, 3, 2).expect("decode"),
            (scalars.to_vec(), identities.to_vec())
        );
        let truncated = bytes
            .get(..bytes.len().saturating_sub(1))
            .expect("truncated register bank");
        assert!(decode_register_bank(truncated, 3, 2).is_err());
    }

    #[test]
    fn authority_count_tracks_canonical_return_data_chunking() {
        assert_eq!(admitted_caller_authority_count_v3(1, 1), Ok(1));
        assert!(admitted_caller_authority_count_v3(0, 0).is_err());
        assert!(admitted_caller_authority_count_v3(120, 2).expect("multi chunk") > 1);
    }

    #[test]
    fn program_programdata_and_finalized_record_substitution_refuse() {
        assert!(exact_deployment_keys_v3([1; 32], [2; 32], [1; 32], [2; 32]));
        assert!(!exact_deployment_keys_v3(
            [1; 32], [2; 32], [3; 32], [2; 32]
        ));
        assert!(!exact_deployment_keys_v3(
            [1; 32], [2; 32], [1; 32], [3; 32]
        ));

        // The addresses below come from `dclutch_record_contract`, not from
        // seeds re-spelled here. The previous version of this test derived its
        // own "canonical" address with the checker's own spelling and then
        // asserted the checker accepted it -- a tautology that held for any
        // spelling and was blind to the only thing that mattered, which is why
        // a two-seed derivation sat here undetected.
        let registry = Pubkey::new_unique();
        let schema = [7_u8; 32];
        let digest = [9_u8; 32];
        let key = RecordKeyV1::new(
            SchemaReleaseId::new(schema).expect("nonzero schema"),
            ContentDigest::new(digest).expect("nonzero digest"),
        );
        let raw = record_address(&registry, key.raw_record_pda_seeds());
        let staging = record_address(&registry, key.staging_cursor_pda_seeds());
        assert_eq!(
            require_record_pair(&registry, &raw, &staging, schema, digest),
            Ok(())
        );

        // The exact defect: a record at the Registry's three-seed address must
        // NOT satisfy a two-seed check, and the two-seed address must not be
        // mistaken for the record.
        let two_seed =
            Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &digest], &registry).0;
        assert_ne!(raw, two_seed);
        assert!(require_record_pair(&registry, &two_seed, &staging, schema, digest).is_err());

        // Swapping schema and digest names a different record.
        assert!(require_record_pair(&registry, &raw, &staging, digest, schema).is_err());
        // A raw record paired with another identity's staging cursor refuses.
        assert!(
            require_record_pair(&registry, &raw, &Pubkey::new_unique(), schema, digest).is_err()
        );
        assert!(
            require_record_pair(&registry, &Pubkey::new_unique(), &staging, schema, digest)
                .is_err()
        );
        // A zero schema or digest is not an identity at all.
        assert!(require_record_pair(&registry, &raw, &staging, [0; 32], digest).is_err());
        assert!(require_record_pair(&registry, &raw, &staging, schema, [0; 32]).is_err());
    }
}
