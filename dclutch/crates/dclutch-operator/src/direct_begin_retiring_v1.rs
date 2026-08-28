//! Chain-derived unsigned Direct Open-to-Retiring construction.
//!
//! This module performs no RPC, wallet access, signing, or submission. It
//! reauthenticates one same-finalized devnet snapshot, regenerates the complete
//! canonical ordinary/begin-retiring/native-close release from its ordinary
//! witness, and either returns the exact permissionless 20-account Trading
//! instruction or reports that the exact Retiring/zero-maker poststate is
//! already present.

use dclutch_capability_contract::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1};
use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1,
    CapabilityRootHeaderV1,
    set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
};
use dclutch_direct_codec::{
    begin_retiring_bundle_v1::{
        direct_begin_retiring_account_profile_schema_v1,
        direct_begin_retiring_descriptor_schema_v1, direct_begin_retiring_effect_schema_v1,
    },
    ordinary_bundle_v4::DirectInlineOrdinaryHotBundleV4,
    program_set_v4::build_direct_inline_ordinary_lifecycle_program_set_v1,
    retirement_v1::{
        DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1, DirectBeginRetiringReceiptV1,
        DirectBeginRetiringRequestV1, direct_begin_retiring_context_v1,
    },
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_ROOT_STATE_BYTES_V1, DirectExecutionConfigV1,
        DirectRootPhaseV1, DirectRootStateV1,
    },
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, Phase, STATE_BYTES};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1, DeploymentObservationV1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_relay_contract::SOLANA_DEVNET_GENESIS_HASH_V1;
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::bpf_loader_upgradeable;

use crate::{
    Finality, Observation, ObservedAccount,
    observation::{FinalizedRecordProof, authenticate_finalized_record, decode_rent},
};

/// Canonical persisted evidence label for the three-selector ProgramSet.
pub const DIRECT_PROGRAM_SET_RECORD_LABEL_V1: &str = "direct_program_set_record";
/// Canonical persisted evidence label for the begin-retiring descriptor.
pub const DIRECT_BEGIN_RETIRING_DESCRIPTOR_RECORD_LABEL_V1: &str =
    "direct_begin_retiring_descriptor_record";
/// Canonical persisted evidence label for the begin-retiring AccountProfile.
pub const DIRECT_BEGIN_RETIRING_PROFILE_RECORD_LABEL_V1: &str =
    "direct_begin_retiring_account_profile_record";
/// Canonical persisted evidence label for the begin-retiring EffectProgram.
pub const DIRECT_BEGIN_RETIRING_EFFECT_RECORD_LABEL_V1: &str =
    "direct_begin_retiring_effect_record";

/// Immutable inputs sufficient to derive the stage's ordered account metas.
///
/// This value is deliberately not finalized evidence. It contains no account
/// bytes, owners, balances, executable bits, slot, or commitment label. A
/// caller may persist it early as an ALT-coordinate closure, but must still run
/// [`plan_direct_begin_retiring_v1`] against a fresh finalized snapshot before
/// submitting anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectBeginRetiringCoordinateInputV1 {
    /// Canonical request committing Market/root/release/record identities.
    pub request: DirectBeginRetiringRequestV1,
    /// Begin-retiring descriptor content identity selected by the ProgramSet.
    pub descriptor: [u8; 32],
    /// Begin-retiring AccountProfile content identity selected by the descriptor.
    pub account_profile: [u8; 32],
    /// Begin-retiring EffectProgram content identity selected by the descriptor.
    pub effect: [u8; 32],
    /// Market-selected Registry program.
    pub registry_program: Pubkey,
    /// Release-selected Core program.
    pub core_program: Pubkey,
    /// Release-selected Core ProgramData.
    pub core_programdata: Pubkey,
    /// Release-selected Trading program.
    pub trading_program: Pubkey,
    /// Release-selected Trading ProgramData.
    pub trading_programdata: Pubkey,
}

/// Message-placement class owned by the DCLTDBR1 account-frame semantic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectBeginRetiringMetaClassV1 {
    /// A durable state, record, ProgramData, or sysvar coordinate admitted to a lookup table.
    LookupStable,
    /// A signer that must remain in the static message key set.
    InlineSigner,
    /// An executable program-account identity that must remain inline.
    InlineProgram,
    /// A request-derived ephemeral coordinate that must remain inline.
    InlineRequestBound,
}

/// Exact placement classes for the canonical 20-account DCLTDBR1 frame.
pub const DIRECT_BEGIN_RETIRING_META_CLASSES_V1: [DirectBeginRetiringMetaClassV1;
    DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1] = [
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::InlineProgram,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::InlineProgram,
    DirectBeginRetiringMetaClassV1::LookupStable,
    DirectBeginRetiringMetaClassV1::InlineProgram,
    DirectBeginRetiringMetaClassV1::LookupStable,
];

/// Non-finalized exact ordered meta closure for one DCLTDBR1 request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectBeginRetiringMetaClosureV1 {
    /// Trading program receiving the top-level instruction.
    pub program_id: Pubkey,
    /// Exact canonical request whose identities derive the account coordinates.
    pub request: DirectBeginRetiringRequestV1,
    /// Exact 20 account metas in top-level wire order.
    pub accounts: [AccountMeta; DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1],
    /// Exact per-meta message-placement classes in the same wire order.
    pub classes: [DirectBeginRetiringMetaClassV1; DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1],
}

/// Stable refusal from coordinate-only closure construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectBeginRetiringCoordinateErrorV1 {
    /// The request or one required artifact/program identity was zero or malformed.
    InvalidIdentity,
    /// Two logical coordinates aliased one physical account.
    AliasedCoordinate,
}

/// Derive the exact non-finalized ordered metas from immutable identities only.
pub fn derive_direct_begin_retiring_meta_closure_v1(
    input: DirectBeginRetiringCoordinateInputV1,
) -> Result<DirectBeginRetiringMetaClosureV1, DirectBeginRetiringCoordinateErrorV1> {
    let request = input
        .request
        .new()
        .map_err(|_| DirectBeginRetiringCoordinateErrorV1::InvalidIdentity)?;
    if [input.descriptor, input.account_profile, input.effect]
        .iter()
        .any(|identity| identity.iter().all(|byte| *byte == 0))
        || [
            input.registry_program,
            input.core_program,
            input.core_programdata,
            input.trading_program,
            input.trading_programdata,
        ]
        .iter()
        .any(|identity| *identity == Pubkey::default())
    {
        return Err(DirectBeginRetiringCoordinateErrorV1::InvalidIdentity);
    }
    let manifest = record_raw(
        input.registry_program,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        request.manifest,
    );
    let (program_set, program_set_staging) = record_pair(
        input.registry_program,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        request.program_set,
    );
    let (descriptor, descriptor_staging) = record_pair(
        input.registry_program,
        direct_begin_retiring_descriptor_schema_v1(),
        input.descriptor,
    );
    let (config, config_staging) = record_pair(
        input.registry_program,
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
        request.config,
    );
    let (account_profile, account_profile_staging) = record_pair(
        input.registry_program,
        direct_begin_retiring_account_profile_schema_v1(),
        input.account_profile,
    );
    let (effect, effect_staging) = record_pair(
        input.registry_program,
        direct_begin_retiring_effect_schema_v1(),
        input.effect,
    );
    let activation_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &request.release_set],
        &input.registry_program,
    )
    .0;
    let accounts = [
        AccountMeta::new(Pubkey::new_from_array(request.root), false),
        AccountMeta::new_readonly(Pubkey::new_from_array(request.market), false),
        AccountMeta::new_readonly(manifest, false),
        AccountMeta::new_readonly(program_set, false),
        AccountMeta::new_readonly(program_set_staging, false),
        AccountMeta::new_readonly(descriptor, false),
        AccountMeta::new_readonly(descriptor_staging, false),
        AccountMeta::new_readonly(config, false),
        AccountMeta::new_readonly(config_staging, false),
        AccountMeta::new_readonly(account_profile, false),
        AccountMeta::new_readonly(account_profile_staging, false),
        AccountMeta::new_readonly(effect, false),
        AccountMeta::new_readonly(effect_staging, false),
        AccountMeta::new_readonly(activation_cache, false),
        AccountMeta::new_readonly(input.core_program, false),
        AccountMeta::new_readonly(input.core_programdata, false),
        AccountMeta::new_readonly(input.trading_program, false),
        AccountMeta::new_readonly(input.trading_programdata, false),
        AccountMeta::new_readonly(input.registry_program, false),
        AccountMeta::new_readonly(solana_sdk_ids::sysvar::rent::ID, false),
    ];
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .get(index.saturating_add(1)..)
            .is_some_and(|suffix| suffix.iter().any(|other| other.pubkey == account.pubkey))
        {
            return Err(DirectBeginRetiringCoordinateErrorV1::AliasedCoordinate);
        }
    }
    Ok(DirectBeginRetiringMetaClosureV1 {
        program_id: input.trading_program,
        request,
        accounts,
        classes: DIRECT_BEGIN_RETIRING_META_CLASSES_V1,
    })
}

fn record_raw(registry: Pubkey, schema: [u8; 32], digest: [u8; 32]) -> Pubkey {
    Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0
}

fn record_pair(registry: Pubkey, schema: [u8; 32], digest: [u8; 32]) -> (Pubkey, Pubkey) {
    (
        record_raw(registry, schema, digest),
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0,
    )
}

/// One exact finalized account graph for the permissionless Trading outer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectBeginRetiringSnapshotV1 {
    /// Cluster genesis hash reported with this snapshot.
    pub genesis_hash: [u8; 32],
    /// Canonical ordinary bundle used only as a witness to regenerate the
    /// complete lifecycle release selected by the root.
    pub ordinary_release_witness: DirectInlineOrdinaryHotBundleV4,
    /// Existing composite Direct root; the sole writable outer account.
    pub root: ObservedAccount,
    /// Canonical Retiring Core Market.
    pub market: ObservedAccount,
    /// Root-selected persistent CapabilityManifest raw record.
    pub capability_manifest: ObservedAccount,
    /// Finalized three-selector Direct ProgramSet raw record.
    pub program_set: ObservedAccount,
    /// Vacant ProgramSet staging cursor.
    pub program_set_staging: ObservedAccount,
    /// Finalized begin-retiring descriptor raw record.
    pub descriptor: ObservedAccount,
    /// Vacant begin-retiring descriptor staging cursor.
    pub descriptor_staging: ObservedAccount,
    /// Finalized root-selected Direct config raw record.
    pub config: ObservedAccount,
    /// Vacant Direct config staging cursor.
    pub config_staging: ObservedAccount,
    /// Finalized begin-retiring AccountProfile raw record.
    pub account_profile: ObservedAccount,
    /// Vacant begin-retiring AccountProfile staging cursor.
    pub account_profile_staging: ObservedAccount,
    /// Finalized begin-retiring EffectProgram raw record.
    pub effect: ObservedAccount,
    /// Vacant begin-retiring EffectProgram staging cursor.
    pub effect_staging: ObservedAccount,
    /// Registry-owned activation cache selected by the Market.
    pub activation_cache: ObservedAccount,
    /// Current executable Core program.
    pub core_program: ObservedAccount,
    /// Current Core upgradeable-loader ProgramData and complete ELF tail.
    pub core_programdata: ObservedAccount,
    /// Current executable Trading program.
    pub trading_program: ObservedAccount,
    /// Current Trading upgradeable-loader ProgramData and complete ELF tail.
    pub trading_programdata: ObservedAccount,
    /// Market-selected executable Registry program.
    pub registry_program: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
}

/// Exact unsigned submission and independently predicted successful response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectBeginRetiringSubmitV1 {
    /// Permissionless exact 20-account Trading instruction.
    pub instruction: Instruction,
    /// Coordinate-only closure the fresh finalized report reproduced exactly.
    pub meta_closure: DirectBeginRetiringMetaClosureV1,
    /// Finalized observation shared by every input account.
    pub observation: Observation,
    /// Typed canonical DCLTDBR1 request.
    pub request: DirectBeginRetiringRequestV1,
    /// Exact fixed request bytes used as instruction data.
    pub request_body: [u8; 320],
    /// SHA-256 of the exact request bytes.
    pub request_digest: [u8; 32],
    /// Exact Open root bytes authenticated by the request.
    pub expected_pre_root_data: Vec<u8>,
    /// Exact Retiring root bytes expected after success.
    pub expected_post_root_data: Vec<u8>,
    /// SHA-256 of the exact successful root bytes.
    pub expected_post_root_digest: [u8; 32],
    /// Exact program required to produce immediate return data.
    pub expected_receipt_producer: Pubkey,
    /// Typed receipt predicted from the request and exact poststate.
    pub expected_receipt: DirectBeginRetiringReceiptV1,
    /// Exact DCLTDRR1 return-data body predicted from authenticated inputs.
    pub expected_receipt_body: [u8; 320],
}

/// Authenticated exact poststate proving the stage needs no resubmission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectBeginRetiringCompleteV1 {
    /// Finalized observation shared by every input account.
    pub observation: Observation,
    /// Canonical Core Market.
    pub market: Pubkey,
    /// Canonical composite Direct root.
    pub root: Pubkey,
    /// Exact observed Retiring/zero-maker root bytes.
    pub observed_post_root_data: Vec<u8>,
    /// SHA-256 of the exact observed poststate.
    pub observed_post_root_digest: [u8; 32],
    /// Selected execution release set.
    pub release_set: [u8; 32],
    /// Selected three-selector Direct ProgramSet.
    pub program_set: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Selected manifest entry index.
    pub entry_index: u16,
}

/// Resumable result of authenticating one finalized lifecycle snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DirectBeginRetiringPlanV1 {
    /// The root is exactly Open with zero makers and needs one submission.
    Submit(Box<DirectBeginRetiringSubmitV1>),
    /// The root is already exactly Retiring with zero makers.
    Complete(Box<DirectBeginRetiringCompleteV1>),
}

/// Stable refusal from hostile, stale, non-devnet, or noncanonical evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectBeginRetiringPlanErrorV1 {
    /// The supplied cluster identity was not Solana devnet.
    DevnetOnly,
    /// Accounts were not unique and from one nonzero finalized observation.
    InvalidObservation,
    /// Rent, Registry, or executable account shape refused.
    InvalidInfrastructure,
    /// The activation cache or current Core/Trading deployment refused.
    InvalidRelease,
    /// Core Market bytes, PDA, phase, release, or Registry join refused.
    InvalidMarket,
    /// Direct root bytes, PDA, owner, rent, or immutable header refused.
    InvalidRoot,
    /// Manifest bytes, persisted address, selected entry, or root join refused.
    InvalidManifest,
    /// ProgramSet/config/descriptor/profile/effect publication refused.
    InvalidRecord,
    /// The release did not equal the canonical three-selector Direct build.
    InvalidLifecycleRelease,
    /// The Direct root was not Open/zero or Retiring/zero.
    InvalidRootState,
    /// Canonical request, receipt, or exact account-frame construction refused.
    InvalidPlan,
}

struct AuthenticatedLifecycleV1 {
    observation: Observation,
    market: CoreState,
    header: CapabilityRootHeaderV1,
    root_state: DirectRootStateV1,
}

/// Reauthenticate one exact finalized devnet snapshot and build its unsigned plan.
pub fn plan_direct_begin_retiring_v1(
    snapshot: &DirectBeginRetiringSnapshotV1,
) -> Result<DirectBeginRetiringPlanV1, DirectBeginRetiringPlanErrorV1> {
    if snapshot.genesis_hash != SOLANA_DEVNET_GENESIS_HASH_V1 {
        return Err(DirectBeginRetiringPlanErrorV1::DevnetOnly);
    }
    let observation = same_finalized_observation(snapshot)?;
    let rent = decode_rent(&snapshot.rent_sysvar)
        .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidInfrastructure)?;
    authenticate_infrastructure(snapshot)?;
    let market = authenticate_market_and_release(snapshot, &rent)?;
    let (header, root_state) = authenticate_root_and_artifacts(snapshot, &rent, market)?;
    assemble_plan(
        snapshot,
        AuthenticatedLifecycleV1 {
            observation,
            market,
            header,
            root_state,
        },
    )
}

fn same_finalized_observation(
    snapshot: &DirectBeginRetiringSnapshotV1,
) -> Result<Observation, DirectBeginRetiringPlanErrorV1> {
    let accounts = frame_accounts(snapshot);
    let observation = accounts
        .first()
        .ok_or(DirectBeginRetiringPlanErrorV1::InvalidObservation)?
        .observation;
    if observation.slot == 0
        || observation.finality != Finality::Finalized
        || accounts
            .iter()
            .any(|account| account.observation != observation)
    {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidObservation);
    }
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .get(index.saturating_add(1)..)
            .is_some_and(|suffix| suffix.iter().any(|other| other.key == account.key))
        {
            return Err(DirectBeginRetiringPlanErrorV1::InvalidObservation);
        }
    }
    Ok(observation)
}

fn frame_accounts(snapshot: &DirectBeginRetiringSnapshotV1) -> [&ObservedAccount; 20] {
    [
        &snapshot.root,
        &snapshot.market,
        &snapshot.capability_manifest,
        &snapshot.program_set,
        &snapshot.program_set_staging,
        &snapshot.descriptor,
        &snapshot.descriptor_staging,
        &snapshot.config,
        &snapshot.config_staging,
        &snapshot.account_profile,
        &snapshot.account_profile_staging,
        &snapshot.effect,
        &snapshot.effect_staging,
        &snapshot.activation_cache,
        &snapshot.core_program,
        &snapshot.core_programdata,
        &snapshot.trading_program,
        &snapshot.trading_programdata,
        &snapshot.registry_program,
        &snapshot.rent_sysvar,
    ]
}

fn authenticate_infrastructure(
    snapshot: &DirectBeginRetiringSnapshotV1,
) -> Result<(), DirectBeginRetiringPlanErrorV1> {
    if snapshot.registry_program.owner != bpf_loader_upgradeable::ID
        || !snapshot.registry_program.executable
        || ProgramV3View::parse(&snapshot.registry_program.data).is_err()
        || snapshot.rent_sysvar.executable
    {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidInfrastructure);
    }
    for account in frame_accounts(snapshot) {
        let expected_executable = account.key == snapshot.core_program.key
            || account.key == snapshot.trading_program.key
            || account.key == snapshot.registry_program.key;
        if account.executable != expected_executable {
            return Err(DirectBeginRetiringPlanErrorV1::InvalidInfrastructure);
        }
    }
    Ok(())
}

fn authenticate_market_and_release(
    snapshot: &DirectBeginRetiringSnapshotV1,
    rent: &Rent,
) -> Result<CoreState, DirectBeginRetiringPlanErrorV1> {
    if snapshot.market.owner != snapshot.core_program.key
        || snapshot.market.data.len() != STATE_BYTES
        || !rent.is_exempt(snapshot.market.lamports, snapshot.market.data.len())
    {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidMarket);
    }
    let market = CoreState::decode(&snapshot.market.data)
        .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidMarket)?;
    let expected_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market.identity).as_slices(),
        &snapshot.core_program.key,
    )
    .0;
    if market
        .encode()
        .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidMarket)?
        .as_slice()
        != snapshot.market.data
        || market.phase != Phase::Retiring
        || snapshot.market.key != expected_market
        || market.identity.market_id.to_bytes() != snapshot.market.key.to_bytes()
        || market.identity.registry_program.to_bytes() != snapshot.registry_program.key.to_bytes()
    {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidMarket);
    }

    let release_set = market.identity.selected_release_set.to_bytes();
    if snapshot.activation_cache.owner != snapshot.registry_program.key
        || snapshot.activation_cache.executable
        || snapshot.activation_cache.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || !rent.is_exempt(
            snapshot.activation_cache.lamports,
            snapshot.activation_cache.data.len(),
        )
    {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidRelease);
    }
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &snapshot.registry_program.key,
    )
    .0;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&snapshot.activation_cache.data)
        .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidRelease)?;
    if snapshot.activation_cache.key != expected_cache
        || activated
            .execution_release_set_id()
            .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidRelease)?
            .to_bytes()
            != release_set
    {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidRelease);
    }
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Core,
            &snapshot.core_program,
            &snapshot.core_programdata,
        ),
        (
            ExecutionRoleV1::Trading,
            &snapshot.trading_program,
            &snapshot.trading_programdata,
        ),
    ] {
        authenticate_role_deployment(activated, role, program, programdata)?;
    }
    Ok(market)
}

fn authenticate_role_deployment(
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
    role: ExecutionRoleV1,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<(), DirectBeginRetiringPlanErrorV1> {
    let selected = activated
        .role(role)
        .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidRelease)?;
    let observation = deployment_observation(program, programdata, selected.release())?;
    selected
        .authenticate_current_deployment(observation)
        .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidRelease)
}

fn deployment_observation(
    program: &ObservedAccount,
    programdata: &ObservedAccount,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1, DirectBeginRetiringPlanErrorV1> {
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || program.key.to_bytes() != release.program().to_bytes()
        || programdata.key.to_bytes() != release.programdata()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidRelease);
    }
    let program_view = ProgramV3View::parse(&program.data)
        .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidRelease)?;
    let expected_programdata =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != programdata.key.to_bytes()
        || programdata.key != expected_programdata
    {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidRelease);
    }
    let data = ProgramDataV3View::parse(&programdata.data)
        .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidRelease)?;
    DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        data.deployment_slot(),
        hash(data.elf()).to_bytes(),
        data.upgrade_authority(),
    )
    .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidRelease)
}

fn authenticate_root_and_artifacts(
    snapshot: &DirectBeginRetiringSnapshotV1,
    rent: &Rent,
    market: CoreState,
) -> Result<(CapabilityRootHeaderV1, DirectRootStateV1), DirectBeginRetiringPlanErrorV1> {
    let root_width = CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(DIRECT_ROOT_STATE_BYTES_V1)
        .ok_or(DirectBeginRetiringPlanErrorV1::InvalidRoot)?;
    if snapshot.root.owner != snapshot.trading_program.key
        || snapshot.root.data.len() != root_width
        || !rent.is_exempt(snapshot.root.lamports, snapshot.root.data.len())
    {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidRoot);
    }
    let header = CapabilityRootHeaderV1::decode(
        snapshot
            .root
            .data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(DirectBeginRetiringPlanErrorV1::InvalidRoot)?,
    )
    .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidRoot)?;
    let root_seeds = header.seeds();
    let expected_root =
        Pubkey::find_program_address(&root_seeds.as_slices(), &snapshot.trading_program.key).0;
    let release_set = market.identity.selected_release_set.to_bytes();
    if header.to_bytes().as_slice()
        != snapshot
            .root
            .data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(DirectBeginRetiringPlanErrorV1::InvalidRoot)?
        || snapshot.root.key != expected_root
        || header.market() != snapshot.market.key.to_bytes()
        || header.generation() != market.identity.generation
        || header.release_set().to_bytes() != release_set
        || header.selection().manifest().to_bytes()
            != market.identity.capability_manifest.to_bytes()
    {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidRoot);
    }
    let root_state = DirectRootStateV1::decode(
        snapshot
            .root
            .data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(DirectBeginRetiringPlanErrorV1::InvalidRoot)?,
    )
    .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidRootState)?;
    if root_state.open_maker_root_count() != 0 {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidRootState);
    }

    let selection = header.selection();
    authenticate_persisted_raw(
        snapshot.registry_program.key,
        rent,
        &snapshot.capability_manifest,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        selection.manifest().to_bytes(),
        header.record_bumps().manifest_raw(),
    )?;
    let manifest = CapabilityManifestV1::decode(&snapshot.capability_manifest.data)
        .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidManifest)?;
    let entry = manifest
        .entry(selection.entry_index())
        .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidManifest)?;
    if entry.kind_id() != selection.kind()
        || entry.release_id() != selection.capability_release()
        || entry.config_id() != selection.config()
    {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidManifest);
    }

    for (raw, staging, schema) in [
        (
            &snapshot.program_set,
            &snapshot.program_set_staging,
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        ),
        (
            &snapshot.descriptor,
            &snapshot.descriptor_staging,
            direct_begin_retiring_descriptor_schema_v1(),
        ),
        (
            &snapshot.config,
            &snapshot.config_staging,
            DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
        ),
        (
            &snapshot.account_profile,
            &snapshot.account_profile_staging,
            direct_begin_retiring_account_profile_schema_v1(),
        ),
        (
            &snapshot.effect,
            &snapshot.effect_staging,
            direct_begin_retiring_effect_schema_v1(),
        ),
    ] {
        authenticate_finalized_record(
            snapshot.registry_program.key,
            rent,
            raw,
            &FinalizedRecordProof {
                schema_release_id: schema,
                staging_cursor: staging.clone(),
            },
        )
        .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidRecord)?;
    }
    require_persisted_pair_bumps(
        snapshot.registry_program.key,
        &snapshot.program_set,
        &snapshot.program_set_staging,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        selection.capability_release().to_bytes(),
        selection.capability_release_raw_bump(),
        selection.capability_release_staging_bump(),
    )?;
    require_persisted_pair_bumps(
        snapshot.registry_program.key,
        &snapshot.config,
        &snapshot.config_staging,
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
        selection.config().to_bytes(),
        header.record_bumps().config_raw(),
        header.record_bumps().config_staging(),
    )?;

    let release = build_direct_inline_ordinary_lifecycle_program_set_v1(
        snapshot.ordinary_release_witness,
        entry.capacity_profile_id().to_bytes(),
    )
    .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidLifecycleRelease)?;
    if release.program_set_id != selection.capability_release().to_bytes()
        || snapshot.program_set.data != release.program_set
        || snapshot.descriptor.data != release.begin_retiring.descriptor
        || snapshot.account_profile.data != release.begin_retiring.account_profile
        || snapshot.effect.data != release.begin_retiring.effect
    {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidLifecycleRelease);
    }
    let set = CapabilityProgramSetV2::decode_selected(
        selection.capability_release().to_bytes(),
        hash(&snapshot.program_set.data).to_bytes(),
        &snapshot.program_set.data,
    )
    .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidLifecycleRelease)?;
    let descriptor_reference = set
        .entry(1)
        .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidLifecycleRelease)?
        .descriptor();
    if descriptor_reference.schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
        || descriptor_reference.program().to_bytes() != hash(&snapshot.descriptor.data).to_bytes()
    {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidLifecycleRelease);
    }
    let descriptor = CapabilityProgramV1::decode(&snapshot.descriptor.data)
        .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidLifecycleRelease)?;
    descriptor
        .validate_selection(selection, entry)
        .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidLifecycleRelease)?;
    DirectExecutionConfigV1::decode_selected(
        selection.config().to_bytes(),
        hash(&snapshot.config.data).to_bytes(),
        &snapshot.config.data,
    )
    .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidRecord)?;
    Ok((header, root_state))
}

fn authenticate_persisted_raw(
    registry: Pubkey,
    rent: &Rent,
    raw: &ObservedAccount,
    schema: [u8; 32],
    digest: [u8; 32],
    bump: u8,
) -> Result<(), DirectBeginRetiringPlanErrorV1> {
    let bump = [bump];
    let expected = Pubkey::create_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest, &bump],
        &registry,
    )
    .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidManifest)?;
    if raw.key != expected
        || raw.owner != registry
        || raw.executable
        || hash(&raw.data).to_bytes() != digest
        || !rent.is_exempt(raw.lamports, raw.data.len())
    {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidManifest);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn require_persisted_pair_bumps(
    registry: Pubkey,
    raw: &ObservedAccount,
    staging: &ObservedAccount,
    schema: [u8; 32],
    digest: [u8; 32],
    raw_bump: u8,
    staging_bump: u8,
) -> Result<(), DirectBeginRetiringPlanErrorV1> {
    let raw_bump = [raw_bump];
    let staging_bump = [staging_bump];
    let expected_raw = Pubkey::create_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest, &raw_bump],
        &registry,
    )
    .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidRecord)?;
    let expected_staging = Pubkey::create_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest, &staging_bump],
        &registry,
    )
    .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidRecord)?;
    if raw.key != expected_raw || staging.key != expected_staging {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidRecord);
    }
    Ok(())
}

fn assemble_plan(
    snapshot: &DirectBeginRetiringSnapshotV1,
    authenticated: AuthenticatedLifecycleV1,
) -> Result<DirectBeginRetiringPlanV1, DirectBeginRetiringPlanErrorV1> {
    if authenticated.root_state.open_maker_root_count() != 0 {
        return Err(DirectBeginRetiringPlanErrorV1::InvalidRootState);
    }
    let selection = authenticated.header.selection();
    let release_set = authenticated.header.release_set().to_bytes();
    match authenticated.root_state.phase() {
        DirectRootPhaseV1::Retiring => Ok(DirectBeginRetiringPlanV1::Complete(Box::new(
            DirectBeginRetiringCompleteV1 {
                observation: authenticated.observation,
                market: snapshot.market.key,
                root: snapshot.root.key,
                observed_post_root_digest: hash(&snapshot.root.data).to_bytes(),
                observed_post_root_data: snapshot.root.data.clone(),
                release_set,
                program_set: selection.capability_release().to_bytes(),
                generation: authenticated.market.identity.generation,
                entry_index: selection.entry_index(),
            },
        ))),
        DirectRootPhaseV1::Open => {
            let retiring = authenticated
                .root_state
                .begin_retiring()
                .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidRootState)?;
            let mut post_root = authenticated.header.to_bytes().to_vec();
            post_root.extend_from_slice(&retiring.encode());
            let post_root_digest = hash(&post_root).to_bytes();
            let market_digest = hash(&snapshot.market.data).to_bytes();
            let root_digest = hash(&snapshot.root.data).to_bytes();
            let market = snapshot.market.key.to_bytes();
            let root = snapshot.root.key.to_bytes();
            let manifest = selection.manifest().to_bytes();
            let program_set = selection.capability_release().to_bytes();
            let config = selection.config().to_bytes();
            let generation = authenticated.market.identity.generation;
            let entry_index = selection.entry_index();
            let request = DirectBeginRetiringRequestV1 {
                release_set,
                market,
                context: direct_begin_retiring_context_v1(
                    release_set,
                    market,
                    root,
                    manifest,
                    program_set,
                    config,
                    generation,
                    entry_index,
                ),
                root,
                manifest,
                program_set,
                config,
                expected_market_digest: market_digest,
                expected_root_digest: root_digest,
                generation,
                entry_index,
            }
            .new()
            .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidPlan)?;
            let request_body = request
                .to_bytes()
                .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidPlan)?;
            let request_digest = hash(&request_body).to_bytes();
            let expected_receipt = DirectBeginRetiringReceiptV1::new(
                request,
                request_digest,
                post_root_digest,
                snapshot.trading_program.key.to_bytes(),
            )
            .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidPlan)?;
            let expected_receipt_body = expected_receipt
                .to_bytes()
                .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidPlan)?;
            let meta_closure = derive_direct_begin_retiring_meta_closure_v1(
                DirectBeginRetiringCoordinateInputV1 {
                    request,
                    descriptor: hash(&snapshot.descriptor.data).to_bytes(),
                    account_profile: hash(&snapshot.account_profile.data).to_bytes(),
                    effect: hash(&snapshot.effect.data).to_bytes(),
                    registry_program: snapshot.registry_program.key,
                    core_program: snapshot.core_program.key,
                    core_programdata: snapshot.core_programdata.key,
                    trading_program: snapshot.trading_program.key,
                    trading_programdata: snapshot.trading_programdata.key,
                },
            )
            .map_err(|_| DirectBeginRetiringPlanErrorV1::InvalidPlan)?;
            if frame_accounts(snapshot)
                .iter()
                .zip(meta_closure.accounts.iter())
                .any(|(observed, expected)| observed.key != expected.pubkey)
                || meta_closure
                    .accounts
                    .iter()
                    .any(|account| account.is_signer)
                || meta_closure
                    .accounts
                    .iter()
                    .enumerate()
                    .any(|(index, account)| account.is_writable != (index == 0))
                || meta_closure.classes != DIRECT_BEGIN_RETIRING_META_CLASSES_V1
            {
                return Err(DirectBeginRetiringPlanErrorV1::InvalidPlan);
            }
            Ok(DirectBeginRetiringPlanV1::Submit(Box::new(
                DirectBeginRetiringSubmitV1 {
                    instruction: Instruction {
                        program_id: meta_closure.program_id,
                        accounts: meta_closure.accounts.to_vec(),
                        data: request_body.to_vec(),
                    },
                    meta_closure,
                    observation: authenticated.observation,
                    request,
                    request_body,
                    request_digest,
                    expected_pre_root_data: snapshot.root.data.clone(),
                    expected_post_root_data: post_root,
                    expected_post_root_digest: post_root_digest,
                    expected_receipt_producer: snapshot.trading_program.key,
                    expected_receipt,
                    expected_receipt_body,
                },
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use dclutch_capability_program_contract::SelectedRecordBumpsV1;
    use dclutch_core_contract::ContentId;
    use dclutch_direct_codec::{
        ordinary_account_artifacts_v3::DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3,
        ordinary_artifacts_v3::DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V2_BYTES_V3,
        ordinary_bundle_v4::{
            DIRECT_INLINE_ORDINARY_DESCRIPTOR_BYTES_V4, DIRECT_INLINE_ORDINARY_STRATEGY_BYTES_V3,
        },
        ordinary_effect_artifacts_v3::DIRECT_INLINE_ORDINARY_EFFECT_BYTES_V4,
        ordinary_v3::DIRECT_ORDINARY_TRANSITION_BYTES_V3,
        state_artifacts_v3::DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5,
        successor::DirectRootStateLayoutV1,
    };
    use dclutch_market_core_codec::{Identity, MarketIdentity, Readiness};
    use dclutch_release_set_contract::CapabilityExecutionSelectionV1;
    use solana_sdk_ids::system_program;

    use super::*;

    const OBSERVATION: Observation = Observation {
        slot: 700,
        unix_timestamp: 1_788_000_000,
        finality: Finality::Finalized,
    };

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn identity(value: u8) -> Identity {
        Identity::new([value; 32]).expect("identity")
    }

    fn content(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("content")
    }

    fn observed(value: u8) -> ObservedAccount {
        ObservedAccount {
            observation: OBSERVATION,
            key: key(value),
            owner: system_program::ID,
            lamports: 0,
            executable: false,
            data: Vec::new(),
        }
    }

    fn ordinary_witness() -> DirectInlineOrdinaryHotBundleV4 {
        DirectInlineOrdinaryHotBundleV4 {
            account_profile: [0; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3],
            lifecycle_policy: [0; DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5],
            request_profile: [0; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V2_BYTES_V3],
            transition: [0; DIRECT_ORDINARY_TRANSITION_BYTES_V3],
            strategy: [0; DIRECT_INLINE_ORDINARY_STRATEGY_BYTES_V3],
            effect: [0; DIRECT_INLINE_ORDINARY_EFFECT_BYTES_V4],
            descriptor: [0; DIRECT_INLINE_ORDINARY_DESCRIPTOR_BYTES_V4],
        }
    }

    fn meta(closure: &DirectBeginRetiringMetaClosureV1, index: usize) -> Pubkey {
        closure.accounts.get(index).expect("account meta").pubkey
    }

    fn fixture(
        root_state: DirectRootStateV1,
    ) -> (DirectBeginRetiringSnapshotV1, AuthenticatedLifecycleV1) {
        let registry = key(60);
        let core = key(61);
        let trading = key(62);
        let mut market_identity = MarketIdentity {
            market_id: identity(1),
            realm_id: identity(2),
            product_record: identity(3),
            product_id: identity(4),
            resolution_policy: identity(5),
            capability_manifest: identity(6),
            selected_release_set: identity(7),
            registry_program: Identity::new(registry.to_bytes()).expect("registry"),
            generation: 8,
        };
        let market_key = Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(market_identity).as_slices(),
            &core,
        )
        .0;
        market_identity.market_id = Identity::new(market_key.to_bytes()).expect("canonical market");
        let market = CoreState {
            phase: Phase::Retiring,
            readiness: Readiness::Consumed,
            terminal_winner: 1,
            identity: market_identity,
            outstanding_capabilities: 1,
            principal_cap_sets: 10,
            rent_beneficiary: identity(9),
            terminal_receipt: Some(identity(10)),
        };
        let selection = CapabilityExecutionSelectionV1::new(
            0,
            content(6),
            content(11),
            content(12),
            content(13),
        )
        .expect("selection")
        .with_capability_release_record_bumps(1, 2);
        let header = CapabilityRootHeaderV1::new(
            content(7),
            market_key.to_bytes(),
            market_identity.generation,
            selection,
            SelectedRecordBumpsV1::new(3, 4, 5, 6),
        )
        .expect("header");
        let root_seeds = header.seeds();
        let root_key = Pubkey::find_program_address(&root_seeds.as_slices(), &trading).0;
        let mut root_data = header.to_bytes().to_vec();
        root_data.extend_from_slice(&root_state.encode());
        let mut snapshot = DirectBeginRetiringSnapshotV1 {
            genesis_hash: SOLANA_DEVNET_GENESIS_HASH_V1,
            ordinary_release_witness: ordinary_witness(),
            root: observed(20),
            market: observed(21),
            capability_manifest: observed(22),
            program_set: observed(23),
            program_set_staging: observed(24),
            descriptor: observed(25),
            descriptor_staging: observed(26),
            config: observed(27),
            config_staging: observed(28),
            account_profile: observed(29),
            account_profile_staging: observed(30),
            effect: observed(31),
            effect_staging: observed(32),
            activation_cache: observed(33),
            core_program: observed(34),
            core_programdata: observed(35),
            trading_program: observed(36),
            trading_programdata: observed(37),
            registry_program: observed(38),
            rent_sysvar: observed(39),
        };
        snapshot.root.key = root_key;
        snapshot.root.owner = trading;
        snapshot.root.data = root_data;
        snapshot.market.key = market_key;
        snapshot.market.owner = core;
        snapshot.market.data = market.encode().expect("market").to_vec();
        snapshot.core_program.key = core;
        snapshot.core_program.executable = true;
        snapshot.trading_program.key = trading;
        snapshot.trading_program.executable = true;
        snapshot.registry_program.key = registry;
        snapshot.registry_program.executable = true;
        let release_set = header.release_set().to_bytes();
        let selection = header.selection();
        let market_digest = hash(&snapshot.market.data).to_bytes();
        let root_digest = hash(&snapshot.root.data).to_bytes();
        let request = DirectBeginRetiringRequestV1 {
            release_set,
            market: market_key.to_bytes(),
            context: direct_begin_retiring_context_v1(
                release_set,
                market_key.to_bytes(),
                root_key.to_bytes(),
                selection.manifest().to_bytes(),
                selection.capability_release().to_bytes(),
                selection.config().to_bytes(),
                market.identity.generation,
                selection.entry_index(),
            ),
            root: root_key.to_bytes(),
            manifest: selection.manifest().to_bytes(),
            program_set: selection.capability_release().to_bytes(),
            config: selection.config().to_bytes(),
            expected_market_digest: market_digest,
            expected_root_digest: root_digest,
            generation: market.identity.generation,
            entry_index: selection.entry_index(),
        };
        let closure =
            derive_direct_begin_retiring_meta_closure_v1(DirectBeginRetiringCoordinateInputV1 {
                request,
                descriptor: hash(&snapshot.descriptor.data).to_bytes(),
                account_profile: hash(&snapshot.account_profile.data).to_bytes(),
                effect: hash(&snapshot.effect.data).to_bytes(),
                registry_program: registry,
                core_program: core,
                core_programdata: snapshot.core_programdata.key,
                trading_program: trading,
                trading_programdata: snapshot.trading_programdata.key,
            })
            .expect("coordinate closure");
        snapshot.capability_manifest.key = meta(&closure, 2);
        snapshot.program_set.key = meta(&closure, 3);
        snapshot.program_set_staging.key = meta(&closure, 4);
        snapshot.descriptor.key = meta(&closure, 5);
        snapshot.descriptor_staging.key = meta(&closure, 6);
        snapshot.config.key = meta(&closure, 7);
        snapshot.config_staging.key = meta(&closure, 8);
        snapshot.account_profile.key = meta(&closure, 9);
        snapshot.account_profile_staging.key = meta(&closure, 10);
        snapshot.effect.key = meta(&closure, 11);
        snapshot.effect_staging.key = meta(&closure, 12);
        snapshot.activation_cache.key = meta(&closure, 13);
        snapshot.rent_sysvar.key = meta(&closure, 19);
        (
            snapshot,
            AuthenticatedLifecycleV1 {
                observation: OBSERVATION,
                market,
                header,
                root_state,
            },
        )
    }

    #[test]
    fn open_zero_root_emits_exact_unsigned_outer_and_authenticated_receipt()
    -> Result<(), &'static str> {
        let (snapshot, authenticated) = fixture(DirectRootStateV1::new());
        let plan = match assemble_plan(&snapshot, authenticated).expect("submit") {
            DirectBeginRetiringPlanV1::Submit(plan) => plan,
            DirectBeginRetiringPlanV1::Complete(_) => return Err("Open root must submit"),
        };
        assert_eq!(plan.instruction.program_id, snapshot.trading_program.key);
        assert_eq!(plan.instruction.accounts, plan.meta_closure.accounts);
        assert_eq!(
            plan.meta_closure.classes,
            DIRECT_BEGIN_RETIRING_META_CLASSES_V1
        );
        assert_eq!(
            plan.instruction.accounts.len(),
            DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1
        );
        assert!(
            plan.instruction
                .accounts
                .first()
                .expect("root meta")
                .is_writable
        );
        assert!(
            plan.instruction
                .accounts
                .iter()
                .skip(1)
                .all(|account| !account.is_writable)
        );
        assert!(
            plan.instruction
                .accounts
                .iter()
                .all(|account| !account.is_signer)
        );
        assert_eq!(plan.instruction.data, plan.request_body);
        assert_eq!(
            plan.request.expected_root_digest,
            hash(&plan.expected_pre_root_data).to_bytes()
        );
        assert_eq!(
            DirectRootStateV1::decode(
                plan.expected_post_root_data
                    .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
                    .expect("post tail")
            )
            .expect("post root")
            .phase(),
            DirectRootPhaseV1::Retiring
        );
        DirectBeginRetiringReceiptV1::decode(&plan.expected_receipt_body)
            .expect("receipt")
            .authenticate_for_request(
                &plan.request_body,
                plan.expected_post_root_digest,
                snapshot.trading_program.key.to_bytes(),
            )
            .expect("request-authenticated receipt");
        Ok(())
    }

    #[test]
    fn coordinate_closure_owns_placement_classes_and_refuses_hostile_identities() {
        let (snapshot, authenticated) = fixture(DirectRootStateV1::new());
        let request = match assemble_plan(&snapshot, authenticated).expect("submit") {
            DirectBeginRetiringPlanV1::Submit(plan) => plan.request,
            DirectBeginRetiringPlanV1::Complete(_) => return,
        };
        let input = DirectBeginRetiringCoordinateInputV1 {
            request,
            descriptor: hash(&snapshot.descriptor.data).to_bytes(),
            account_profile: hash(&snapshot.account_profile.data).to_bytes(),
            effect: hash(&snapshot.effect.data).to_bytes(),
            registry_program: snapshot.registry_program.key,
            core_program: snapshot.core_program.key,
            core_programdata: snapshot.core_programdata.key,
            trading_program: snapshot.trading_program.key,
            trading_programdata: snapshot.trading_programdata.key,
        };
        let closure = derive_direct_begin_retiring_meta_closure_v1(input).expect("closure");
        assert_eq!(closure.classes, DIRECT_BEGIN_RETIRING_META_CLASSES_V1);
        assert_eq!(
            closure
                .classes
                .iter()
                .filter(|class| **class == DirectBeginRetiringMetaClassV1::InlineProgram)
                .count(),
            3
        );
        assert!(closure.classes.iter().all(|class| {
            matches!(
                class,
                DirectBeginRetiringMetaClassV1::LookupStable
                    | DirectBeginRetiringMetaClassV1::InlineProgram
            )
        }));
        assert_eq!(
            derive_direct_begin_retiring_meta_closure_v1(DirectBeginRetiringCoordinateInputV1 {
                effect: [0; 32],
                ..input
            }),
            Err(DirectBeginRetiringCoordinateErrorV1::InvalidIdentity)
        );
        assert_eq!(
            derive_direct_begin_retiring_meta_closure_v1(DirectBeginRetiringCoordinateInputV1 {
                core_program: input.trading_program,
                ..input
            }),
            Err(DirectBeginRetiringCoordinateErrorV1::AliasedCoordinate)
        );
    }

    #[test]
    fn exact_retiring_zero_root_is_complete_without_fabricating_submission()
    -> Result<(), &'static str> {
        let state = DirectRootStateV1::new().begin_retiring().expect("Retiring");
        let (snapshot, authenticated) = fixture(state);
        let complete = match assemble_plan(&snapshot, authenticated).expect("complete") {
            DirectBeginRetiringPlanV1::Complete(complete) => complete,
            DirectBeginRetiringPlanV1::Submit(_) => return Err("Retiring root must be complete"),
        };
        assert_eq!(complete.observed_post_root_data, snapshot.root.data);
        assert_eq!(
            complete.observed_post_root_digest,
            hash(&snapshot.root.data).to_bytes()
        );
        assert_eq!(complete.root, snapshot.root.key);
        assert_eq!(complete.market, snapshot.market.key);
        Ok(())
    }

    #[test]
    fn mixed_observation_alias_and_non_devnet_refuse_before_hostile_decode() {
        let (mut snapshot, _) = fixture(DirectRootStateV1::new());
        snapshot.effect_staging.observation.slot += 1;
        assert_eq!(
            same_finalized_observation(&snapshot),
            Err(DirectBeginRetiringPlanErrorV1::InvalidObservation)
        );
        snapshot.effect_staging.observation = OBSERVATION;
        snapshot.effect_staging.key = snapshot.effect.key;
        assert_eq!(
            same_finalized_observation(&snapshot),
            Err(DirectBeginRetiringPlanErrorV1::InvalidObservation)
        );
        snapshot.genesis_hash = [99; 32];
        assert_eq!(
            plan_direct_begin_retiring_v1(&snapshot),
            Err(DirectBeginRetiringPlanErrorV1::DevnetOnly)
        );
    }

    #[test]
    fn nonzero_maker_count_refuses_even_after_authenticated_fact_construction() {
        let mut bytes = DirectRootStateV1::new().encode();
        bytes
            .get_mut(
                DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT
                    ..DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT + 8,
            )
            .expect("maker count")
            .copy_from_slice(&1_u64.to_le_bytes());
        let root_state = DirectRootStateV1::decode(&bytes).expect("valid nonzero root");
        let (mut snapshot, mut authenticated) = fixture(DirectRootStateV1::new());
        authenticated.root_state = root_state;
        snapshot
            .root
            .data
            .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .expect("root tail")
            .copy_from_slice(&bytes);
        assert_eq!(
            assemble_plan(&snapshot, authenticated),
            Err(DirectBeginRetiringPlanErrorV1::InvalidRootState)
        );
    }
}
