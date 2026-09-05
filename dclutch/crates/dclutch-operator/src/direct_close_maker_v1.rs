//! Chain-derived unsigned Direct maker-replay close construction.
//!
//! This module performs no RPC, wallet access, signing, or submission. It
//! reauthenticates one same-finalized snapshot, regenerates the complete
//! canonical five-entry lifecycle release from its ordinary witness, and
//! either returns the exact permissionless 22-account Trading instruction that
//! closes one maker replay or reports that the replay is already gone.
//!
//! # Why this exists
//!
//! The `DCLTDMC1` route landed with a program, a codec, and a bank-driven
//! program test. A route reachable only from a bank is not a route the cut can
//! crank, and `CloseMakerReplay` is permissionless -- a claim about strangers.
//! This is the operator half: the thing that turns a live cluster's account
//! graph into the exact unsigned instruction, and, more importantly, the thing
//! that says *no* before a transaction is ever built.
//!
//! # The two refusals, at plan time rather than send time
//!
//! [`close_maker_replay_v2`] is the semantic close. The chain calls it, and so
//! does this builder -- the same function over the same authenticated bytes,
//! which is why the refusals here cannot drift from the refusals on chain.
//! Two of them are the ones a cut day will actually meet:
//!
//! * a replay that still owes its Direct fee refuses as
//!   [`DirectCloseMakerPlanErrorV1::FeeOutstanding`], the plan-time mirror of
//!   `CloseMakerFeeOutstanding` (`0x4011`). The replay is the sole record of
//!   the FEE-TX2 receivable, so a close that ignored it would erase a debt with
//!   no residue. Fee settlement is deliberately phase-free: settle, then close.
//! * a replay with registered live intents refuses as
//!   [`DirectCloseMakerPlanErrorV1::LiveIntents`], the plan-time mirror of
//!   `CloseMakerLiveIntents` (`0x4012`).
//!
//! Both are reachable states of a real market, not hostile input, which is why
//! they are named refusals rather than one opaque `InvalidReplay`. An operator
//! reading `FeeOutstanding` at plan time knows the remedy; an operator reading
//! `0x4011` in a failed devnet transaction has already spent the fee and the
//! slot to learn the same thing.
//!
//! # What the caller may say and what it may never decide
//!
//! The wire carries a coordinate -- market, maker, generation -- and nothing
//! economic. Neither does this builder. The rent beneficiary, the principal,
//! the donation slice, and the resulting maker-root count are all read off
//! authenticated chain bytes and projected through the shared semantic
//! function. The caller names the rent-owner *coordinate* because a message
//! cannot be addressed without it, and the plan stage then proves that
//! coordinate equals the `rent_owner` the replay itself recorded (O-016:
//! caller input never becomes authority). A caller that names the wrong
//! beneficiary gets [`DirectCloseMakerPlanErrorV1::InvalidRentOwner`], not a
//! redirected refund.

use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_market::capability_manifest::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1};
use dclutch_market::capability_program::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1,
    CapabilityRootHeaderV1,
    set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
};
use dclutch_trading::{
    close_maker_bundle_v1::{
        direct_close_maker_account_profile_schema_v1, direct_close_maker_descriptor_schema_v1,
        direct_close_maker_effect_schema_v1,
    },
    close_maker_v1::{
        DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1, DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1,
        DIRECT_CLOSE_MAKER_RECEIPT_BYTES_V1, DIRECT_CLOSE_MAKER_REQUEST_BYTES_V1,
        DirectCloseMakerReceiptV1, DirectCloseMakerRequestV1,
        direct_close_maker_account_privileges_v1,
    },
    ordinary_bundle_v4::DirectInlineOrdinaryHotBundleV4,
    program_set_v4::build_direct_inline_ordinary_lifecycle_program_set_v1,
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_MAKER_REPLAY_BYTES_V1,
        DIRECT_MAKER_REPLAY_LEGACY_BYTES_V1, DIRECT_ROOT_STATE_BYTES_V1, DirectCoordinatesV1,
        DirectExecutionConfigV1, DirectRootPhaseV1, DirectRootStateV1, MakerReplayRootV1,
        MakerReplaySeedsV1, SuccessorError, close_maker_replay_v2,
    },
};
use dclutch_market::{CoreState, MarketCoreStateSeedsV2, Phase, STATE_BYTES};
use dclutch_registry::record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1, DeploymentObservationV1,
};
use dclutch_registry::svm::{ProgramDataV3View, ProgramV3View};
use dclutch_source::relay::{SOLANA_DEVNET_GENESIS_HASH_V1, SOLANA_MAINNET_GENESIS_HASH_V1};
use dclutch_registry::release_set::ExecutionRoleV1;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};

use crate::{
    Finality, Observation, ObservedAccount,
    observation::{FinalizedRecordProof, authenticate_finalized_record},
};

/// ProgramSet entry index of the maker-replay close.
///
/// The five-entry Direct lifecycle set is ordered by ascending `u32` selector:
/// ordinary, begin-retiring (`0xffff_ff00`), native-close (`0xffff_ff01`),
/// activation (`0xffff_ff02`), maker close (`0xffff_ff04`). This constant is
/// asserted against the regenerated release rather than trusted.
pub const DIRECT_CLOSE_MAKER_PROGRAM_SET_ENTRY_V1: u16 = 4;

/// Which cluster a snapshot claims to have come from.
///
/// The model builder for this family admits devnet and nothing else, which is
/// correct for a route that only ever ran there. This one is driven from a
/// loopback validator as well, so the cluster is a claim the caller makes and
/// this builder checks, rather than a comment. Mainnet is refused under both
/// arms: no arm of this builder has a reason to reach it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCloseMakerClusterV1 {
    /// Solana devnet, admitted only against devnet's own genesis hash.
    Devnet,
    /// A validator the operator owns, admitted only against a genesis hash
    /// that is neither devnet's nor mainnet's.
    OwnedLoopback,
}

impl DirectCloseMakerClusterV1 {
    /// Admit one observed genesis hash for this cluster claim, or refuse it.
    fn admit(self, genesis_hash: [u8; 32]) -> Result<(), DirectCloseMakerPlanErrorV1> {
        if genesis_hash == SOLANA_MAINNET_GENESIS_HASH_V1 {
            return Err(DirectCloseMakerPlanErrorV1::ClusterRefused);
        }
        let admitted = match self {
            Self::Devnet => genesis_hash == SOLANA_DEVNET_GENESIS_HASH_V1,
            Self::OwnedLoopback => genesis_hash != SOLANA_DEVNET_GENESIS_HASH_V1,
        };
        if admitted {
            Ok(())
        } else {
            Err(DirectCloseMakerPlanErrorV1::ClusterRefused)
        }
    }
}

/// Immutable inputs sufficient to derive the stage's ordered account metas.
///
/// This value is deliberately not finalized evidence. It contains no account
/// bytes, owners, balances, executable bits, slot, or commitment label. A
/// caller may persist it early as an ALT-coordinate closure, but must still run
/// [`plan_direct_close_maker_v1`] against a fresh finalized snapshot before
/// submitting anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCloseMakerCoordinateInputV1 {
    /// Canonical request committing the market/maker/generation coordinate.
    pub request: DirectCloseMakerRequestV1,
    /// Close descriptor content identity selected by the ProgramSet.
    pub descriptor: [u8; 32],
    /// Close AccountProfile content identity selected by the descriptor.
    pub account_profile: [u8; 32],
    /// Close EffectProgram content identity selected by the descriptor.
    pub effect: [u8; 32],
    /// Composite Direct root, whose address is derived from the root header
    /// rather than from the request; the plan stage proves the join.
    pub root: Pubkey,
    /// Root-selected persistent CapabilityManifest content identity.
    pub manifest: [u8; 32],
    /// Three-selector Direct ProgramSet content identity.
    pub program_set: [u8; 32],
    /// Root-selected Direct config content identity.
    pub config: [u8; 32],
    /// Execution release set selected by the Market.
    pub release_set: [u8; 32],
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
    /// The maker replay being closed.
    ///
    /// Named rather than derived because the chain authenticates it against
    /// the bump the account itself records, not against the canonical one.
    pub maker_replay: Pubkey,
    /// The recorded rent beneficiary the balance reaches.
    ///
    /// A message cannot be addressed without naming it, so the caller does.
    /// It is never believed: the plan stage refuses unless it equals the
    /// `rent_owner` the replay recorded at first use.
    pub rent_owner: Pubkey,
}

/// Message-placement class owned by the DCLTDMC1 account-frame semantic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCloseMakerMetaClassV1 {
    /// A durable state, record, ProgramData, or sysvar coordinate admitted to a lookup table.
    LookupStable,
    /// A signer that must remain in the static message key set.
    ///
    /// The close has none. The variant exists so this family's class vocabulary
    /// matches its siblings' rather than quietly omitting the case a reader
    /// would look for to confirm the route is permissionless.
    InlineSigner,
    /// An executable program-account identity that must remain inline.
    InlineProgram,
    /// A request-derived ephemeral coordinate that must remain inline.
    InlineRequestBound,
}

/// Exact placement classes for the canonical 22-account DCLTDMC1 frame.
///
/// Indices 0..=19 are the begin-retiring frame verbatim; the close appends the
/// two coordinates that are specific to one maker and therefore must not be
/// assumed by a lookup table built for the market as a whole.
pub const DIRECT_CLOSE_MAKER_META_CLASSES_V1: [DirectCloseMakerMetaClassV1;
    DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1] = [
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::InlineProgram,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::InlineProgram,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::InlineProgram,
    DirectCloseMakerMetaClassV1::LookupStable,
    DirectCloseMakerMetaClassV1::InlineRequestBound,
    DirectCloseMakerMetaClassV1::InlineRequestBound,
];

/// Non-finalized exact ordered meta closure for one DCLTDMC1 request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCloseMakerMetaClosureV1 {
    /// Trading program receiving the top-level instruction.
    pub program_id: Pubkey,
    /// Exact canonical request whose identities derive the account coordinates.
    pub request: DirectCloseMakerRequestV1,
    /// Exact 22 account metas in top-level wire order.
    pub accounts: [AccountMeta; DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1],
    /// Exact per-meta message-placement classes in the same wire order.
    pub classes: [DirectCloseMakerMetaClassV1; DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1],
}

/// Stable refusal from coordinate-only closure construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCloseMakerCoordinateErrorV1 {
    /// The request or one required artifact/program identity was zero or malformed.
    InvalidIdentity,
    /// Two logical coordinates aliased one physical account.
    AliasedCoordinate,
    /// `dclutch_trading` refused; the cause is its own.
    DirectCloseMaker(dclutch_trading::close_maker_v1::DirectCloseMakerErrorV1),
    /// `dclutch_registry::record` refused; the cause is its own.
    Record(dclutch_registry::record::Error),
}

/// Derive the exact non-finalized ordered metas from immutable identities only.
///
/// The writable/executable membrane is not restated here. It is read out of
/// [`direct_close_maker_account_privileges_v1`], the codec function the route
/// itself uses, so the two cannot disagree.
pub fn derive_direct_close_maker_meta_closure_v1(
    input: DirectCloseMakerCoordinateInputV1,
) -> Result<DirectCloseMakerMetaClosureV1, DirectCloseMakerCoordinateErrorV1> {
    let request = input
        .request
        .new()
        .map_err(DirectCloseMakerCoordinateErrorV1::DirectCloseMaker)?;
    if [
        input.descriptor,
        input.account_profile,
        input.effect,
        input.manifest,
        input.program_set,
        input.config,
        input.release_set,
    ]
    .iter()
    .any(|identity| identity.iter().all(|byte| *byte == 0))
        || [
            input.root,
            input.registry_program,
            input.core_program,
            input.core_programdata,
            input.trading_program,
            input.trading_programdata,
            input.maker_replay,
            input.rent_owner,
        ]
        .iter()
        .any(|identity| *identity == Pubkey::default())
    {
        return Err(DirectCloseMakerCoordinateErrorV1::InvalidIdentity);
    }
    let manifest = record_raw(
        input.registry_program,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        input.manifest,
    )?;
    let (program_set, program_set_staging) = record_pair(
        input.registry_program,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        input.program_set,
    )?;
    let (descriptor, descriptor_staging) = record_pair(
        input.registry_program,
        direct_close_maker_descriptor_schema_v1(),
        input.descriptor,
    )?;
    let (config, config_staging) = record_pair(
        input.registry_program,
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
        input.config,
    )?;
    let (account_profile, account_profile_staging) = record_pair(
        input.registry_program,
        direct_close_maker_account_profile_schema_v1(),
        input.account_profile,
    )?;
    let (effect, effect_staging) = record_pair(
        input.registry_program,
        direct_close_maker_effect_schema_v1(),
        input.effect,
    )?;
    let activation_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &input.release_set],
        &input.registry_program,
    )
    .0;
    let keys = [
        input.root,
        Pubkey::new_from_array(request.market),
        manifest,
        program_set,
        program_set_staging,
        descriptor,
        descriptor_staging,
        config,
        config_staging,
        account_profile,
        account_profile_staging,
        effect,
        effect_staging,
        activation_cache,
        input.core_program,
        input.core_programdata,
        input.trading_program,
        input.trading_programdata,
        input.registry_program,
        solana_sdk_ids::sysvar::rent::ID,
        input.maker_replay,
        input.rent_owner,
    ];
    let mut accounts = core::array::from_fn(|index| AccountMeta::new_readonly(keys[index], false));
    for (index, meta) in accounts.iter_mut().enumerate() {
        let (writable, _) = direct_close_maker_account_privileges_v1(index)
            .ok_or(DirectCloseMakerCoordinateErrorV1::InvalidIdentity)?;
        meta.is_writable = writable;
    }
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .get(index.saturating_add(1)..)
            .is_some_and(|suffix| suffix.iter().any(|other| other.pubkey == account.pubkey))
        {
            return Err(DirectCloseMakerCoordinateErrorV1::AliasedCoordinate);
        }
    }
    Ok(DirectCloseMakerMetaClosureV1 {
        program_id: input.trading_program,
        request,
        accounts,
        classes: DIRECT_CLOSE_MAKER_META_CLASSES_V1,
    })
}

/// One record's canonical identity, or a refusal if either half is zero.
///
/// The seed TUPLE is not spelled here. `dclutch-registry::record` owns both
/// domains and exports the constructors that place them, so a second spelling
/// in this crate would be a second source of truth for an address the chain
/// derives its own way (`DOMAIN_RAW_RESTATEMENT`).
fn record_key(
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<RecordKeyV1, DirectCloseMakerCoordinateErrorV1> {
    Ok(RecordKeyV1::new(
        SchemaReleaseId::new(schema).map_err(DirectCloseMakerCoordinateErrorV1::Record)?,
        ContentDigest::new(digest).map_err(DirectCloseMakerCoordinateErrorV1::Record)?,
    ))
}

/// One record address from the contract-owned seed material.
fn record_address(seeds: RecordPdaSeedsV1, registry: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            seeds.domain(),
            seeds.schema_release_id().as_bytes(),
            seeds.expected_digest().as_bytes(),
        ],
        &registry,
    )
    .0
}

fn record_raw(
    registry: Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<Pubkey, DirectCloseMakerCoordinateErrorV1> {
    Ok(record_address(
        record_key(schema, digest)?.raw_record_pda_seeds(),
        registry,
    ))
}

fn record_pair(
    registry: Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<(Pubkey, Pubkey), DirectCloseMakerCoordinateErrorV1> {
    let key = record_key(schema, digest)?;
    Ok((
        record_address(key.raw_record_pda_seeds(), registry),
        record_address(key.staging_cursor_pda_seeds(), registry),
    ))
}

/// One exact finalized account graph for the permissionless Trading outer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCloseMakerSnapshotV1 {
    /// Which cluster this graph claims to be from.
    pub cluster: DirectCloseMakerClusterV1,
    /// Genesis hash observed on that cluster.
    pub genesis_hash: [u8; 32],
    /// Canonical ordinary bundle used only as a witness to regenerate the
    /// complete lifecycle release selected by the root.
    pub ordinary_release_witness: DirectInlineOrdinaryHotBundleV4,
    /// Existing composite Direct root; writable.
    pub root: ObservedAccount,
    /// Canonical Retiring Core Market.
    pub market: ObservedAccount,
    /// Root-selected persistent CapabilityManifest raw record.
    pub capability_manifest: ObservedAccount,
    /// Finalized five-entry Direct ProgramSet raw record.
    pub program_set: ObservedAccount,
    /// Vacant ProgramSet staging cursor.
    pub program_set_staging: ObservedAccount,
    /// Finalized close descriptor raw record.
    pub descriptor: ObservedAccount,
    /// Vacant close descriptor staging cursor.
    pub descriptor_staging: ObservedAccount,
    /// Finalized root-selected Direct config raw record.
    pub config: ObservedAccount,
    /// Vacant Direct config staging cursor.
    pub config_staging: ObservedAccount,
    /// Finalized close AccountProfile raw record.
    pub account_profile: ObservedAccount,
    /// Vacant close AccountProfile staging cursor.
    pub account_profile_staging: ObservedAccount,
    /// Finalized close EffectProgram raw record.
    pub effect: ObservedAccount,
    /// Vacant close EffectProgram staging cursor.
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
    /// The maker whose replay this snapshot was gathered to close.
    ///
    /// A caller must name this to derive the replay coordinate at all, so it
    /// is an input. It is never authority: the submit path refuses unless the
    /// replay's own recorded `maker` equals it, so a caller that named the
    /// wrong maker gets a refusal rather than somebody else's close.
    pub maker: Pubkey,
    /// The maker replay being closed; writable, and drained to nothing.
    pub maker_replay: ObservedAccount,
    /// The recorded rent beneficiary; writable, and credited.
    pub rent_owner: ObservedAccount,
}

/// Exact unsigned submission and independently predicted successful response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCloseMakerSubmitV1 {
    /// Permissionless exact 22-account Trading instruction.
    pub instruction: Instruction,
    /// Coordinate-only closure the fresh finalized report reproduced exactly.
    pub meta_closure: DirectCloseMakerMetaClosureV1,
    /// Finalized observation shared by every input account.
    pub observation: Observation,
    /// Typed canonical DCLTDMC1 request.
    pub request: DirectCloseMakerRequestV1,
    /// Exact fixed request bytes used as instruction data.
    pub request_body: [u8; DIRECT_CLOSE_MAKER_REQUEST_BYTES_V1],
    /// SHA-256 of the exact request bytes.
    pub request_digest: [u8; 32],
    /// Exact root bytes authenticated as the prestate.
    pub expected_pre_root_data: Vec<u8>,
    /// Exact root bytes expected after the count decrement.
    pub expected_post_root_data: Vec<u8>,
    /// SHA-256 of the exact successful root bytes.
    pub expected_post_root_digest: [u8; 32],
    /// Open maker roots still standing after this close.
    pub expected_remaining_open_maker_roots: u64,
    /// The beneficiary's exact lamports after the credit.
    pub expected_rent_owner_lamports: u64,
    /// Historical account-rent principal, exactly as the replay recorded it.
    pub rent_principal: u64,
    /// Lamports above principal, explicitly not fees or reserves.
    pub unclassified_donation: u64,
    /// The permissionless closer's carve, out of the donation slice alone.
    pub closer_reward: u64,
    /// Exact total lamports credited to the beneficiary.
    pub total_credit: u64,
    /// Exact program required to produce immediate return data.
    pub expected_receipt_producer: Pubkey,
    /// Typed receipt predicted from the request and exact poststate.
    pub expected_receipt: DirectCloseMakerReceiptV1,
    /// Exact DCLTDMX1 return-data body predicted from authenticated inputs.
    pub expected_receipt_body: [u8; DIRECT_CLOSE_MAKER_RECEIPT_BYTES_V1],
}

/// Authenticated exact poststate proving the close needs no resubmission.
///
/// `CloseMakerReplay` is permissionless and racing is expected, so a replay
/// that is already gone is the ordinary outcome of losing a race rather than
/// an error. The chain says the same thing by absence (`0x4010`); this says it
/// before a fee is spent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCloseMakerCompleteV1 {
    /// Finalized observation shared by every input account.
    pub observation: Observation,
    /// Canonical Core Market.
    pub market: Pubkey,
    /// Canonical composite Direct root.
    pub root: Pubkey,
    /// The maker whose replay is already closed.
    pub maker: Pubkey,
    /// The closed replay coordinate, observed vacant.
    pub maker_replay: Pubkey,
    /// Exact observed root bytes.
    pub observed_root_data: Vec<u8>,
    /// SHA-256 of the exact observed root bytes.
    pub observed_root_digest: [u8; 32],
    /// Open maker roots standing at this observation.
    pub observed_open_maker_roots: u64,
}

/// Resumable result of authenticating one finalized close snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DirectCloseMakerPlanV1 {
    /// The replay stands, owes nothing, holds no live intent, and needs one
    /// submission.
    Submit(Box<DirectCloseMakerSubmitV1>),
    /// The replay is already gone.
    Complete(Box<DirectCloseMakerCompleteV1>),
}

/// Stable refusal from hostile, stale, wrong-cluster, or noncanonical evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCloseMakerPlanErrorV1 {
    /// The observed genesis hash did not match the claimed cluster, or was mainnet.
    ClusterRefused,
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
    /// The release did not equal the canonical five-entry Direct build.
    InvalidLifecycleRelease,
    /// The Direct root was not Retiring, or its maker count was already drained.
    InvalidRootState,
    /// The replay account's bytes, owner, PDA, rent, or coordinate refused.
    InvalidReplay,
    /// The named beneficiary was not the recorded `rent_owner`, or was not a
    /// plain empty System wallet.
    InvalidRentOwner,
    /// The replay still owes its Direct fee. Mirrors `CloseMakerFeeOutstanding`
    /// (`0x4011`): settle the fee first, then close.
    FeeOutstanding,
    /// The replay still has registered live intents. Mirrors
    /// `CloseMakerLiveIntents` (`0x4012`): close them first, then close it.
    LiveIntents,
    /// Canonical request, receipt, or exact account-frame construction refused.
    InvalidPlan,
    /// `dclutch_market` refused; the cause is its own.
    MarketCore(dclutch_market::Error),
    /// `dclutch_registry` refused; the cause is its own.
    Registry(dclutch_registry::Error),
    /// `dclutch_registry::svm` refused; the cause is its own.
    RegistrySvm(dclutch_registry::svm::Error),
    /// `dclutch_market::capability_program` refused; the cause is its own.
    CapabilityProgram(dclutch_market::capability_program::Error),
    /// `dclutch_trading` refused; the cause is its own.
    Successor(dclutch_trading::successor::SuccessorError),
    /// `dclutch_market::capability_manifest` refused; the cause is its own.
    Capability(dclutch_market::capability_manifest::Error),
    /// `dclutch_operator` refused; the cause is its own.
    Observation(crate::observation::ObservationError),
    /// `dclutch_trading` refused; the cause is its own.
    DirectProgramSet(dclutch_trading::program_set_v4::DirectProgramSetErrorV4),
    /// `dclutch_market::capability_program` refused; the cause is its own.
    ProgramSet(dclutch_market::capability_program::set_v2::ProgramSetErrorV2),
    /// `dclutch_operator` refused; the cause is its own.
    DirectCloseMakerCoordinate(crate::direct_close_maker_v1::DirectCloseMakerCoordinateErrorV1),
    /// `dclutch_trading` refused; the cause is its own.
    DirectCloseMaker(dclutch_trading::close_maker_v1::DirectCloseMakerErrorV1),
}

struct AuthenticatedCloseV1 {
    observation: Observation,
    market: CoreState,
    header: CapabilityRootHeaderV1,
    root_state: DirectRootStateV1,
}

/// Reauthenticate one exact finalized snapshot and build its unsigned plan.
pub fn plan_direct_close_maker_v1(
    snapshot: &DirectCloseMakerSnapshotV1,
) -> Result<DirectCloseMakerPlanV1, DirectCloseMakerPlanErrorV1> {
    snapshot.cluster.admit(snapshot.genesis_hash)?;
    let observation = same_finalized_observation(snapshot)?;
    authenticate_infrastructure(snapshot)?;
    let market = authenticate_market_and_release(snapshot)?;
    let (header, root_state) = authenticate_root_and_artifacts(snapshot, market)?;
    assemble_plan(
        snapshot,
        AuthenticatedCloseV1 {
            observation,
            market,
            header,
            root_state,
        },
    )
}

fn same_finalized_observation(
    snapshot: &DirectCloseMakerSnapshotV1,
) -> Result<Observation, DirectCloseMakerPlanErrorV1> {
    let accounts = frame_accounts(snapshot);
    let observation = accounts
        .first()
        .ok_or(DirectCloseMakerPlanErrorV1::InvalidObservation)?
        .observation;
    if observation.slot == 0
        || observation.finality != Finality::Finalized
        || accounts
            .iter()
            .any(|account| account.observation != observation)
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidObservation);
    }
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .get(index.saturating_add(1)..)
            .is_some_and(|suffix| suffix.iter().any(|other| other.key == account.key))
        {
            return Err(DirectCloseMakerPlanErrorV1::InvalidObservation);
        }
    }
    Ok(observation)
}

fn frame_accounts(
    snapshot: &DirectCloseMakerSnapshotV1,
) -> [&ObservedAccount; DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1] {
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
        &snapshot.maker_replay,
        &snapshot.rent_owner,
    ]
}

fn authenticate_infrastructure(
    snapshot: &DirectCloseMakerSnapshotV1,
) -> Result<(), DirectCloseMakerPlanErrorV1> {
    if snapshot.registry_program.owner != bpf_loader_upgradeable::ID
        || !snapshot.registry_program.executable
        || ProgramV3View::parse(&snapshot.registry_program.data).is_err()
        || snapshot.rent_sysvar.executable
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidInfrastructure);
    }
    for (index, account) in frame_accounts(snapshot).iter().enumerate() {
        let (_, expected_executable) = direct_close_maker_account_privileges_v1(index)
            .ok_or(DirectCloseMakerPlanErrorV1::InvalidInfrastructure)?;
        if account.executable != expected_executable {
            return Err(DirectCloseMakerPlanErrorV1::InvalidInfrastructure);
        }
    }
    Ok(())
}

fn authenticate_market_and_release(
    snapshot: &DirectCloseMakerSnapshotV1,
) -> Result<CoreState, DirectCloseMakerPlanErrorV1> {
    if snapshot.market.owner != snapshot.core_program.key
        || snapshot.market.data.len() != STATE_BYTES
        || !funded_rent_persists_v1(snapshot.market.lamports)
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidMarket);
    }
    let market = CoreState::decode(&snapshot.market.data)
        .map_err(DirectCloseMakerPlanErrorV1::MarketCore)?;
    let expected_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market.identity).as_slices(),
        &snapshot.core_program.key,
    )
    .0;
    if market
        .encode()
        .map_err(DirectCloseMakerPlanErrorV1::MarketCore)?
        .as_slice()
        != snapshot.market.data
        || market.phase != Phase::Retiring
        || snapshot.market.key != expected_market
        || market.identity.market_id.to_bytes() != snapshot.market.key.to_bytes()
        || market.identity.registry_program.to_bytes() != snapshot.registry_program.key.to_bytes()
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidMarket);
    }

    let release_set = market.identity.selected_release_set.to_bytes();
    if snapshot.activation_cache.owner != snapshot.registry_program.key
        || snapshot.activation_cache.executable
        || snapshot.activation_cache.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || !funded_rent_persists_v1(snapshot.activation_cache.lamports)
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidRelease);
    }
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &snapshot.registry_program.key,
    )
    .0;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&snapshot.activation_cache.data)
        .map_err(DirectCloseMakerPlanErrorV1::Registry)?;
    if snapshot.activation_cache.key != expected_cache
        || activated
            .execution_release_set_id()
            .map_err(DirectCloseMakerPlanErrorV1::Registry)?
            .to_bytes()
            != release_set
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidRelease);
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
) -> Result<(), DirectCloseMakerPlanErrorV1> {
    let selected = activated
        .role(role)
        .map_err(DirectCloseMakerPlanErrorV1::Registry)?;
    let observation = deployment_observation(program, programdata, selected.release())?;
    selected
        .authenticate_current_deployment(observation)
        .map_err(DirectCloseMakerPlanErrorV1::Registry)
}

fn deployment_observation(
    program: &ObservedAccount,
    programdata: &ObservedAccount,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1, DirectCloseMakerPlanErrorV1> {
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || program.key.to_bytes() != release.program().to_bytes()
        || programdata.key.to_bytes() != release.programdata()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidRelease);
    }
    let program_view =
        ProgramV3View::parse(&program.data).map_err(DirectCloseMakerPlanErrorV1::RegistrySvm)?;
    let expected_programdata =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != programdata.key.to_bytes()
        || programdata.key != expected_programdata
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidRelease);
    }
    let data = ProgramDataV3View::parse(&programdata.data)
        .map_err(DirectCloseMakerPlanErrorV1::RegistrySvm)?;
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
    .map_err(DirectCloseMakerPlanErrorV1::Registry)
}

fn authenticate_root_and_artifacts(
    snapshot: &DirectCloseMakerSnapshotV1,
    market: CoreState,
) -> Result<(CapabilityRootHeaderV1, DirectRootStateV1), DirectCloseMakerPlanErrorV1> {
    let root_width = CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(DIRECT_ROOT_STATE_BYTES_V1)
        .ok_or(DirectCloseMakerPlanErrorV1::InvalidRoot)?;
    if snapshot.root.owner != snapshot.trading_program.key
        || snapshot.root.data.len() != root_width
        || !funded_rent_persists_v1(snapshot.root.lamports)
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidRoot);
    }
    let header_bytes = snapshot
        .root
        .data
        .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
        .ok_or(DirectCloseMakerPlanErrorV1::InvalidRoot)?;
    let header = CapabilityRootHeaderV1::decode(header_bytes)
        .map_err(DirectCloseMakerPlanErrorV1::CapabilityProgram)?;
    let root_seeds = header.seeds();
    let expected_root =
        Pubkey::find_program_address(&root_seeds.as_slices(), &snapshot.trading_program.key).0;
    let release_set = market.identity.selected_release_set.to_bytes();
    if header.to_bytes().as_slice() != header_bytes
        || snapshot.root.key != expected_root
        || header.market() != snapshot.market.key.to_bytes()
        || header.generation() != market.identity.generation
        || header.release_set().to_bytes() != release_set
        || header.selection().manifest().to_bytes()
            != market.identity.capability_manifest.to_bytes()
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidRoot);
    }
    let root_state = DirectRootStateV1::decode(
        snapshot
            .root
            .data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(DirectCloseMakerPlanErrorV1::InvalidRoot)?,
    )
    .map_err(DirectCloseMakerPlanErrorV1::Successor)?;

    let selection = header.selection();
    authenticate_persisted_raw(
        snapshot.registry_program.key,
        &snapshot.capability_manifest,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        selection.manifest().to_bytes(),
        header.record_bumps().manifest_raw(),
    )?;
    let manifest = CapabilityManifestV1::decode(&snapshot.capability_manifest.data)
        .map_err(DirectCloseMakerPlanErrorV1::Capability)?;
    let entry = manifest
        .entry(selection.entry_index())
        .map_err(DirectCloseMakerPlanErrorV1::Capability)?;
    if entry.kind_id() != selection.kind()
        || entry.release_id() != selection.capability_release()
        || entry.config_id() != selection.config()
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidManifest);
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
            direct_close_maker_descriptor_schema_v1(),
        ),
        (
            &snapshot.config,
            &snapshot.config_staging,
            DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
        ),
        (
            &snapshot.account_profile,
            &snapshot.account_profile_staging,
            direct_close_maker_account_profile_schema_v1(),
        ),
        (
            &snapshot.effect,
            &snapshot.effect_staging,
            direct_close_maker_effect_schema_v1(),
        ),
    ] {
        authenticate_finalized_record(
            snapshot.registry_program.key,
            raw,
            &FinalizedRecordProof {
                schema_release_id: schema,
                staging_cursor: staging.clone(),
            },
        )
        .map_err(DirectCloseMakerPlanErrorV1::Observation)?;
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
    .map_err(DirectCloseMakerPlanErrorV1::DirectProgramSet)?;
    if release.program_set_id != selection.capability_release().to_bytes()
        || snapshot.program_set.data != release.program_set
        || snapshot.descriptor.data != release.close_maker.descriptor
        || snapshot.account_profile.data != release.close_maker.account_profile
        || snapshot.effect.data != release.close_maker.effect
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidLifecycleRelease);
    }
    let set = CapabilityProgramSetV2::decode_selected(
        selection.capability_release().to_bytes(),
        hash(&snapshot.program_set.data).to_bytes(),
        &snapshot.program_set.data,
    )
    .map_err(DirectCloseMakerPlanErrorV1::ProgramSet)?;
    let descriptor_reference = set
        .entry(DIRECT_CLOSE_MAKER_PROGRAM_SET_ENTRY_V1)
        .map_err(DirectCloseMakerPlanErrorV1::ProgramSet)?
        .descriptor();
    if descriptor_reference.schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
        || descriptor_reference.program().to_bytes() != hash(&snapshot.descriptor.data).to_bytes()
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidLifecycleRelease);
    }
    let descriptor = CapabilityProgramV1::decode(&snapshot.descriptor.data)
        .map_err(DirectCloseMakerPlanErrorV1::CapabilityProgram)?;
    descriptor
        .validate_selection(selection, entry)
        .map_err(DirectCloseMakerPlanErrorV1::CapabilityProgram)?;
    DirectExecutionConfigV1::decode_selected(
        selection.config().to_bytes(),
        hash(&snapshot.config.data).to_bytes(),
        &snapshot.config.data,
    )
    .map_err(DirectCloseMakerPlanErrorV1::Successor)?;
    Ok((header, root_state))
}

/// One record address from the contract-owned seed material, under a bump the
/// chain recorded rather than one this crate searched for.
fn record_address_at_bump(
    seeds: RecordPdaSeedsV1,
    registry: Pubkey,
    bump: u8,
) -> Result<Pubkey, DirectCloseMakerPlanErrorV1> {
    Pubkey::create_program_address(
        &[
            seeds.domain(),
            seeds.schema_release_id().as_bytes(),
            seeds.expected_digest().as_bytes(),
            &[bump],
        ],
        &registry,
    )
    .map_err(|_| DirectCloseMakerPlanErrorV1::InvalidRecord)
}

fn authenticate_persisted_raw(
    registry: Pubkey,
    account: &ObservedAccount,
    schema: [u8; 32],
    digest: [u8; 32],
    bump: u8,
) -> Result<(), DirectCloseMakerPlanErrorV1> {
    let key = record_key(schema, digest)
        .map_err(DirectCloseMakerPlanErrorV1::DirectCloseMakerCoordinate)?;
    let expected = record_address_at_bump(key.raw_record_pda_seeds(), registry, bump)?;
    if account.key != expected
        || account.owner != registry
        || account.executable
        || hash(&account.data).to_bytes() != digest
        || !funded_rent_persists_v1(account.lamports)
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidRecord);
    }
    Ok(())
}

fn require_persisted_pair_bumps(
    registry: Pubkey,
    raw: &ObservedAccount,
    staging: &ObservedAccount,
    schema: [u8; 32],
    digest: [u8; 32],
    raw_bump: u8,
    staging_bump: u8,
) -> Result<(), DirectCloseMakerPlanErrorV1> {
    let key = record_key(schema, digest)
        .map_err(DirectCloseMakerPlanErrorV1::DirectCloseMakerCoordinate)?;
    let expected_raw = record_address_at_bump(key.raw_record_pda_seeds(), registry, raw_bump)?;
    let expected_staging =
        record_address_at_bump(key.staging_cursor_pda_seeds(), registry, staging_bump)?;
    if raw.key != expected_raw || staging.key != expected_staging {
        return Err(DirectCloseMakerPlanErrorV1::InvalidRecord);
    }
    Ok(())
}

/// Decide whether the observed replay account is vacant.
///
/// A closed replay is System-owned with no lamports and no bytes. Anything
/// partially matching that is not "already closed"; it is an account this
/// builder does not understand, and it refuses rather than guessing.
fn replay_is_vacant(account: &ObservedAccount) -> Result<bool, DirectCloseMakerPlanErrorV1> {
    let vacant = account.owner == system_program::ID
        && account.lamports == 0
        && account.data.is_empty()
        && !account.executable;
    let occupied = account.data.len() == DIRECT_MAKER_REPLAY_BYTES_V1
        || account.data.len() == DIRECT_MAKER_REPLAY_LEGACY_BYTES_V1;
    if vacant {
        Ok(true)
    } else if occupied {
        Ok(false)
    } else {
        Err(DirectCloseMakerPlanErrorV1::InvalidReplay)
    }
}

fn assemble_plan(
    snapshot: &DirectCloseMakerSnapshotV1,
    authenticated: AuthenticatedCloseV1,
) -> Result<DirectCloseMakerPlanV1, DirectCloseMakerPlanErrorV1> {
    let selection = authenticated.header.selection();
    let release_set = authenticated.header.release_set().to_bytes();
    let market_key = snapshot.market.key;
    let generation = authenticated.market.identity.generation;

    // The root phase gates BOTH answers. `close_maker_replay_v2` refuses a
    // non-Retiring root on the submit path, and checking it here too means a
    // vacant replay under an Open root reports the operator's actual mistake --
    // this market has not begun retiring -- instead of a soothing `Complete`
    // for a close that was never possible.
    if authenticated.root_state.phase() != DirectRootPhaseV1::Retiring {
        return Err(DirectCloseMakerPlanErrorV1::InvalidRootState);
    }

    if replay_is_vacant(&snapshot.maker_replay)? {
        return Ok(DirectCloseMakerPlanV1::Complete(Box::new(
            DirectCloseMakerCompleteV1 {
                observation: authenticated.observation,
                market: market_key,
                root: snapshot.root.key,
                // A vacant account records nothing, so the maker here is the
                // coordinate the caller asked about rather than an
                // authenticated fact. It is reported to identify WHICH close
                // is already done, and the caller supplied it.
                maker: snapshot.maker,
                maker_replay: snapshot.maker_replay.key,
                observed_root_digest: hash(&snapshot.root.data).to_bytes(),
                observed_root_data: snapshot.root.data.clone(),
                observed_open_maker_roots: authenticated.root_state.open_maker_root_count(),
            },
        )));
    }

    // The replay stands. Authenticate it exactly as the chain does: its own
    // recorded bump must reproduce its own address under the Trading program.
    let maker_root = MakerReplayRootV1::decode(&snapshot.maker_replay.data)
        .map_err(DirectCloseMakerPlanErrorV1::Successor)?;
    let coordinates = DirectCoordinatesV1::new(market_key.to_bytes(), generation)
        .map_err(DirectCloseMakerPlanErrorV1::Successor)?;
    let seeds = MakerReplaySeedsV1::new(coordinates, maker_root.maker())
        .map_err(DirectCloseMakerPlanErrorV1::Successor)?;
    let [domain, market_seed, generation_seed, maker_seed] = seeds.as_slices();
    let bump = [maker_root.bump()];
    let expected_replay = Pubkey::create_program_address(
        &[domain, market_seed, generation_seed, maker_seed, &bump],
        &snapshot.trading_program.key,
    )
    .map_err(|_| DirectCloseMakerPlanErrorV1::InvalidReplay)?;
    if snapshot.maker_replay.owner != snapshot.trading_program.key
        || snapshot.maker_replay.key != expected_replay
        || maker_root.market() != market_key.to_bytes()
        || maker_root.generation() != generation
        // The caller named a maker to find this account; the account itself is
        // what says whose it is. Disagreement is a refusal, never a silent
        // adoption of the caller's answer.
        || maker_root.maker() != snapshot.maker.to_bytes()
        || !funded_rent_persists_v1(snapshot.maker_replay.lamports)
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidReplay);
    }

    // The beneficiary is program-recorded, never caller-chosen. The frame's
    // account must BE the recorded one, and must be a plain empty System
    // wallet: crediting a program-owned account is a write this route has no
    // authority to make.
    if snapshot.rent_owner.key.to_bytes() != maker_root.rent_owner()
        || snapshot.rent_owner.owner != system_program::ID
        || snapshot.rent_owner.executable
        || !snapshot.rent_owner.data.is_empty()
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidRentOwner);
    }

    // The shared semantic close. Calling it is what makes the two refusals
    // below the SAME refusals the chain raises, rather than a second opinion
    // about them.
    let closed = close_maker_replay_v2(
        authenticated.root_state,
        maker_root,
        snapshot.maker_replay.lamports,
        DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1,
    )
    .map_err(|error| match error {
        SuccessorError::FeeOwedOutstanding => DirectCloseMakerPlanErrorV1::FeeOutstanding,
        SuccessorError::LiveCountInvariant => DirectCloseMakerPlanErrorV1::LiveIntents,
        SuccessorError::InvalidRootPhase | SuccessorError::MakerRootCountInvariant => {
            DirectCloseMakerPlanErrorV1::InvalidRootState
        }
        _ => DirectCloseMakerPlanErrorV1::InvalidReplay,
    })?;

    let mut post_root = snapshot.root.data.clone();
    post_root
        .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(DirectCloseMakerPlanErrorV1::InvalidRoot)?
        .copy_from_slice(&closed.root.encode());
    let post_root_digest = hash(&post_root).to_bytes();

    let request = DirectCloseMakerRequestV1 {
        market: market_key.to_bytes(),
        maker: maker_root.maker(),
        generation,
    }
    .new()
    .map_err(DirectCloseMakerPlanErrorV1::DirectCloseMaker)?;
    let request_body = request
        .to_bytes()
        .map_err(DirectCloseMakerPlanErrorV1::DirectCloseMaker)?;
    let request_digest = hash(&request_body).to_bytes();

    let expected_receipt = DirectCloseMakerReceiptV1 {
        request_digest,
        market: market_key.to_bytes(),
        maker: maker_root.maker(),
        maker_root: snapshot.maker_replay.key.to_bytes(),
        rent_owner: closed.plan.rent_owner,
        post_root_digest,
        rent_principal: closed.plan.rent_principal,
        unclassified_donation: closed.plan.unclassified_donation,
        closer_reward: closed.plan.closer_reward,
        total_credit: closed.plan.total_credit,
        remaining_open_maker_roots: closed.root.open_maker_root_count(),
    }
    .new()
    .map_err(DirectCloseMakerPlanErrorV1::DirectCloseMaker)?;
    let expected_receipt_body = expected_receipt
        .to_bytes()
        .map_err(DirectCloseMakerPlanErrorV1::DirectCloseMaker)?;

    let meta_closure =
        derive_direct_close_maker_meta_closure_v1(DirectCloseMakerCoordinateInputV1 {
            request,
            descriptor: hash(&snapshot.descriptor.data).to_bytes(),
            account_profile: hash(&snapshot.account_profile.data).to_bytes(),
            effect: hash(&snapshot.effect.data).to_bytes(),
            root: snapshot.root.key,
            manifest: selection.manifest().to_bytes(),
            program_set: selection.capability_release().to_bytes(),
            config: selection.config().to_bytes(),
            release_set,
            registry_program: snapshot.registry_program.key,
            core_program: snapshot.core_program.key,
            core_programdata: snapshot.core_programdata.key,
            trading_program: snapshot.trading_program.key,
            trading_programdata: snapshot.trading_programdata.key,
            maker_replay: snapshot.maker_replay.key,
            rent_owner: snapshot.rent_owner.key,
        })
        .map_err(DirectCloseMakerPlanErrorV1::DirectCloseMakerCoordinate)?;

    if frame_accounts(snapshot)
        .iter()
        .zip(meta_closure.accounts.iter())
        .any(|(observed, expected)| observed.key != expected.pubkey)
        || meta_closure
            .accounts
            .iter()
            .any(|account| account.is_signer)
        || meta_closure.classes != DIRECT_CLOSE_MAKER_META_CLASSES_V1
    {
        return Err(DirectCloseMakerPlanErrorV1::InvalidPlan);
    }
    for (index, account) in meta_closure.accounts.iter().enumerate() {
        let (writable, _) = direct_close_maker_account_privileges_v1(index)
            .ok_or(DirectCloseMakerPlanErrorV1::InvalidPlan)?;
        if account.is_writable != writable {
            return Err(DirectCloseMakerPlanErrorV1::InvalidPlan);
        }
    }

    let expected_rent_owner_lamports = snapshot
        .rent_owner
        .lamports
        .checked_add(closed.plan.total_credit)
        .ok_or(DirectCloseMakerPlanErrorV1::InvalidPlan)?;

    Ok(DirectCloseMakerPlanV1::Submit(Box::new(
        DirectCloseMakerSubmitV1 {
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
            expected_remaining_open_maker_roots: closed.root.open_maker_root_count(),
            expected_rent_owner_lamports,
            rent_principal: closed.plan.rent_principal,
            unclassified_donation: closed.plan.unclassified_donation,
            closer_reward: closed.plan.closer_reward,
            total_credit: closed.plan.total_credit,
            expected_receipt_producer: snapshot.trading_program.key,
            expected_receipt,
            expected_receipt_body,
        },
    )))
}

#[cfg(test)]
mod tests {
    use solana_program::rent::Rent;

    use dclutch_market::capability_program::SelectedRecordBumpsV1;
    use dclutch_core_contract::ContentId;
    use dclutch_trading::successor::{
        DirectMakerReplayLayoutV1 as MakerLayout, DirectRootStateLayoutV1 as RootLayout,
    };
    use dclutch_trading::{
        ordinary_account_artifacts_v3::DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3,
        ordinary_artifacts_v3::DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V2_BYTES_V3,
        ordinary_bundle_v4::{
            DIRECT_INLINE_ORDINARY_DESCRIPTOR_BYTES_V4, DIRECT_INLINE_ORDINARY_STRATEGY_BYTES_V3,
        },
        ordinary_effect_artifacts_v3::DIRECT_INLINE_ORDINARY_EFFECT_BYTES_V4,
        ordinary_v3::DIRECT_ORDINARY_TRANSITION_BYTES_V3,
        state_artifacts_v3::DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5,
    };
    use dclutch_market::{Identity, MarketIdentity, Readiness, StateBumpsV1};
    use dclutch_registry::release_set::CapabilityExecutionSelectionV1;

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

    fn meta(closure: &DirectCloseMakerMetaClosureV1, index: usize) -> Pubkey {
        closure.accounts.get(index).expect("account meta").pubkey
    }

    /// A Retiring root tail carrying an exact open-maker count.
    ///
    /// There is no public constructor that sets the count, so this patches the
    /// one documented word into canonically encoded bytes and decodes them
    /// back. A layout change breaks `decode` loudly rather than silently
    /// producing a root this builder would misread.
    fn retiring_root(open_maker_root_count: u64) -> DirectRootStateV1 {
        let mut bytes = DirectRootStateV1::new()
            .begin_retiring()
            .expect("retiring")
            .encode()
            .to_vec();
        bytes[RootLayout::OPEN_MAKER_ROOT_COUNT..RootLayout::OPEN_MAKER_ROOT_COUNT + 8]
            .copy_from_slice(&open_maker_root_count.to_le_bytes());
        DirectRootStateV1::decode(&bytes).expect("root tail")
    }

    /// Exact canonical maker replay bytes.
    ///
    /// The ABI version word is lifted out of a canonically encoded root tail
    /// rather than restated, because both wires carry the same successor ABI
    /// version and only one of them has a public encoder.
    #[allow(clippy::too_many_arguments)]
    fn replay_wire(
        market: Pubkey,
        generation: u64,
        maker: Pubkey,
        rent_owner: Pubkey,
        rent_principal: u64,
        bump: u8,
        live_count: u64,
        fee_owed: u64,
    ) -> Vec<u8> {
        let version = DirectRootStateV1::new().encode();
        let mut wire = vec![0_u8; DIRECT_MAKER_REPLAY_BYTES_V1];
        wire[MakerLayout::MAGIC..MakerLayout::MAGIC + 8]
            .copy_from_slice(&MakerLayout::MAGIC_WORD.to_le_bytes());
        wire[8..10].copy_from_slice(&version[8..10]);
        wire[MakerLayout::BUMP] = bump;
        wire[MakerLayout::MARKET..MakerLayout::MARKET + 32].copy_from_slice(&market.to_bytes());
        wire[MakerLayout::GENERATION..MakerLayout::GENERATION + 8]
            .copy_from_slice(&generation.to_le_bytes());
        wire[MakerLayout::MAKER..MakerLayout::MAKER + 32].copy_from_slice(&maker.to_bytes());
        // One nonce has been consumed, so a nonzero `live_count` is a state
        // `validate` admits rather than one it rejects as `live > next`.
        wire[MakerLayout::NEXT_NONCE..MakerLayout::NEXT_NONCE + 8]
            .copy_from_slice(&1_u64.to_le_bytes());
        wire[MakerLayout::LIVE_COUNT..MakerLayout::LIVE_COUNT + 8]
            .copy_from_slice(&live_count.to_le_bytes());
        wire[MakerLayout::RENT_OWNER..MakerLayout::RENT_OWNER + 32]
            .copy_from_slice(&rent_owner.to_bytes());
        wire[MakerLayout::RENT_PRINCIPAL..MakerLayout::RENT_PRINCIPAL + 8]
            .copy_from_slice(&rent_principal.to_le_bytes());
        wire[MakerLayout::FEE_OWED..MakerLayout::FEE_OWED + 8]
            .copy_from_slice(&fee_owed.to_le_bytes());
        wire
    }

    struct Fixture {
        snapshot: DirectCloseMakerSnapshotV1,
        authenticated: AuthenticatedCloseV1,
        donation: u64,
        rent_principal: u64,
    }

    /// One complete close-ready world.
    ///
    /// `live_count` and `fee_owed` are parameters because they are the two
    /// facts the named refusals turn on, and a fixture that could not express
    /// them could not red-proof either one.
    fn fixture(open_maker_root_count: u64, live_count: u64, fee_owed: u64) -> Fixture {
        let registry = key(60);
        let core = key(61);
        let trading = key(62);
        let maker = key(70);
        let rent_owner_key = key(71);
        let rent = Rent::default();
        let root_state = retiring_root(open_maker_root_count);

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
            bumps: StateBumpsV1::UNRECORDED,
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
        let root_key = Pubkey::find_program_address(&header.seeds().as_slices(), &trading).0;
        let mut root_data = header.to_bytes().to_vec();
        root_data.extend_from_slice(&root_state.encode());

        // The replay address is the canonical PDA, and the bump the wire
        // records is the one that derives it -- exactly what the chain checks.
        let coordinates =
            DirectCoordinatesV1::new(market_key.to_bytes(), market_identity.generation)
                .expect("coordinates");
        let seeds = MakerReplaySeedsV1::new(coordinates, maker.to_bytes()).expect("seeds");
        let (replay_key, replay_bump) = Pubkey::find_program_address(&seeds.as_slices(), &trading);
        let rent_principal = rent.minimum_balance(DIRECT_MAKER_REPLAY_BYTES_V1);
        let donation = 4_242;

        let mut snapshot = DirectCloseMakerSnapshotV1 {
            cluster: DirectCloseMakerClusterV1::Devnet,
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
            maker: maker,
            maker_replay: observed(40),
            rent_owner: observed(41),
        };
        snapshot.root.key = root_key;
        snapshot.root.owner = trading;
        snapshot.root.data = root_data;
        snapshot.root.lamports = rent.minimum_balance(snapshot.root.data.len());
        snapshot.market.key = market_key;
        snapshot.market.owner = core;
        snapshot.market.data = market.encode().expect("market").to_vec();
        snapshot.market.lamports = rent.minimum_balance(snapshot.market.data.len());
        snapshot.core_program.key = core;
        snapshot.core_program.executable = true;
        snapshot.trading_program.key = trading;
        snapshot.trading_program.executable = true;
        snapshot.registry_program.key = registry;
        snapshot.registry_program.executable = true;
        snapshot.maker_replay.key = replay_key;
        snapshot.maker_replay.owner = trading;
        snapshot.maker_replay.data = replay_wire(
            market_key,
            market_identity.generation,
            maker,
            rent_owner_key,
            rent_principal,
            replay_bump,
            live_count,
            fee_owed,
        );
        snapshot.maker_replay.lamports = rent_principal + donation;
        snapshot.rent_owner.key = rent_owner_key;
        snapshot.rent_owner.owner = system_program::ID;
        snapshot.rent_owner.lamports = 1_000;

        let closure =
            derive_direct_close_maker_meta_closure_v1(DirectCloseMakerCoordinateInputV1 {
                request: DirectCloseMakerRequestV1 {
                    market: market_key.to_bytes(),
                    maker: maker.to_bytes(),
                    generation: market_identity.generation,
                },
                descriptor: hash(&snapshot.descriptor.data).to_bytes(),
                account_profile: hash(&snapshot.account_profile.data).to_bytes(),
                effect: hash(&snapshot.effect.data).to_bytes(),
                root: root_key,
                manifest: header.selection().manifest().to_bytes(),
                program_set: header.selection().capability_release().to_bytes(),
                config: header.selection().config().to_bytes(),
                release_set: header.release_set().to_bytes(),
                registry_program: registry,
                core_program: core,
                core_programdata: snapshot.core_programdata.key,
                trading_program: trading,
                trading_programdata: snapshot.trading_programdata.key,
                maker_replay: replay_key,
                rent_owner: rent_owner_key,
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

        Fixture {
            snapshot,
            authenticated: AuthenticatedCloseV1 {
                observation: OBSERVATION,
                market,
                header,
                root_state,
            },
            donation,
            rent_principal,
        }
    }

    fn submit(
        fixture: &Fixture,
    ) -> Result<Box<DirectCloseMakerSubmitV1>, DirectCloseMakerPlanErrorV1> {
        match assemble_plan(
            &fixture.snapshot,
            AuthenticatedCloseV1 {
                ..fixture.authenticated
            },
        )? {
            DirectCloseMakerPlanV1::Submit(report) => Ok(report),
            DirectCloseMakerPlanV1::Complete(_) => {
                panic!("a standing replay must not report Complete")
            }
        }
    }

    /// The whole point: a clean replay under a Retiring root produces the exact
    /// permissionless 22-account outer, and a receipt whose refund arithmetic
    /// conserves the observed balance.
    #[test]
    fn clean_replay_emits_exact_unsigned_outer_and_authenticated_receipt() {
        let fixture = fixture(3, 0, 0);
        let report = submit(&fixture).expect("submit");

        assert_eq!(
            report.instruction.accounts.len(),
            DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1
        );
        assert_eq!(
            report.instruction.program_id,
            fixture.snapshot.trading_program.key
        );
        assert_eq!(
            report.instruction.data.len(),
            DIRECT_CLOSE_MAKER_REQUEST_BYTES_V1
        );

        // Permissionless is a property of the frame, not a promise in a doc
        // comment: no account in it asks for a signature.
        assert!(
            report
                .instruction
                .accounts
                .iter()
                .all(|meta| !meta.is_signer)
        );

        // The writable membrane is the codec's, not a second opinion.
        for (index, meta) in report.instruction.accounts.iter().enumerate() {
            let (writable, _) =
                direct_close_maker_account_privileges_v1(index).expect("privileges");
            assert_eq!(meta.is_writable, writable, "account {index} writability");
        }

        // Nothing economic was invented: the principal is the replay's own
        // recorded number and the donation is exactly the excess balance.
        assert_eq!(report.rent_principal, fixture.rent_principal);
        assert_eq!(report.unclassified_donation, fixture.donation);
        assert_eq!(
            report.total_credit,
            fixture.rent_principal + fixture.donation
        );
        assert_eq!(
            report.expected_rent_owner_lamports,
            fixture.snapshot.rent_owner.lamports + report.total_credit
        );

        // The count decrements by exactly one, and the projected poststate
        // digest is the digest of the projected poststate bytes.
        assert_eq!(report.expected_remaining_open_maker_roots, 2);
        assert_eq!(
            report.expected_post_root_digest,
            hash(&report.expected_post_root_data).to_bytes()
        );
        assert_ne!(
            report.expected_post_root_data,
            report.expected_pre_root_data
        );

        // The receipt is the one the chain will produce, decoded back from the
        // exact bytes rather than compared field-by-field to itself.
        let decoded = DirectCloseMakerReceiptV1::decode(&report.expected_receipt_body)
            .expect("receipt round trip");
        assert_eq!(decoded, report.expected_receipt);
        assert_eq!(
            decoded.rent_owner,
            fixture.snapshot.rent_owner.key.to_bytes()
        );
        assert_eq!(
            decoded.maker_root,
            fixture.snapshot.maker_replay.key.to_bytes()
        );
        assert_eq!(
            decoded.request_digest,
            hash(&report.instruction.data).to_bytes()
        );
        assert_eq!(decoded.post_root_digest, report.expected_post_root_digest);
        assert_eq!(
            decoded.rent_principal + decoded.unclassified_donation,
            decoded.total_credit
        );
    }

    /// A replay that still owes its Direct fee refuses AT PLAN TIME, by the
    /// name the chain uses for it (`CloseMakerFeeOutstanding`, `0x4011`).
    #[test]
    fn debtor_replay_refuses_by_name_before_any_transaction_exists() {
        let fixture = fixture(3, 0, 9_950);
        assert_eq!(
            submit(&fixture).expect_err("a debtor replay must not plan"),
            DirectCloseMakerPlanErrorV1::FeeOutstanding
        );
    }

    /// A replay with registered live intents refuses at plan time, by the name
    /// the chain uses for it (`CloseMakerLiveIntents`, `0x4012`).
    #[test]
    fn live_intent_replay_refuses_by_name_before_any_transaction_exists() {
        let fixture = fixture(3, 1, 0);
        assert_eq!(
            submit(&fixture).expect_err("a live replay must not plan"),
            DirectCloseMakerPlanErrorV1::LiveIntents
        );
    }

    /// Red-proof of the refusal ORDER, which is not arbitrary.
    ///
    /// `close_maker_replay_v2` tests `live_count` before `fee_owed`, so a
    /// replay that is both live and in debt is `0x4012` on chain. A builder
    /// that reported `FeeOutstanding` here would send an operator to settle a
    /// fee that was not the blocker.
    #[test]
    fn a_live_and_indebted_replay_reports_live_intents_not_the_fee() {
        let fixture = fixture(3, 1, 9_950);
        assert_eq!(
            submit(&fixture).expect_err("neither condition may plan"),
            DirectCloseMakerPlanErrorV1::LiveIntents
        );
    }

    /// Both refusals clear the moment their condition does, so neither is a
    /// dead end -- the remedy this builder implies actually works.
    #[test]
    fn settling_the_fee_and_closing_the_intents_makes_the_close_plannable() {
        assert!(submit(&fixture(3, 0, 1)).is_err());
        assert!(submit(&fixture(3, 1, 0)).is_err());
        assert!(submit(&fixture(3, 0, 0)).is_ok());
    }

    /// A replay that is already gone is a lost race, not an error. The chain
    /// says so by absence; this says so before a fee is spent.
    #[test]
    fn vacant_replay_is_complete_without_fabricating_a_submission() {
        let mut fixture = fixture(3, 0, 0);
        fixture.snapshot.maker_replay.owner = system_program::ID;
        fixture.snapshot.maker_replay.lamports = 0;
        fixture.snapshot.maker_replay.data = Vec::new();
        match assemble_plan(
            &fixture.snapshot,
            AuthenticatedCloseV1 {
                ..fixture.authenticated
            },
        )
        .expect("complete")
        {
            DirectCloseMakerPlanV1::Complete(report) => {
                assert_eq!(report.maker_replay, fixture.snapshot.maker_replay.key);
                assert_eq!(report.maker, fixture.snapshot.maker);
                assert_eq!(report.observed_open_maker_roots, 3);
            }
            DirectCloseMakerPlanV1::Submit(_) => panic!("a vacant replay must not plan a close"),
        }
    }

    /// A half-vacant account is not a closed one, and this refuses rather than
    /// deciding which half to believe.
    #[test]
    fn a_partially_drained_replay_refuses_rather_than_being_read_either_way() {
        let mut fixture = fixture(3, 0, 0);
        fixture.snapshot.maker_replay.data = vec![0_u8; 7];
        assert_eq!(
            submit(&fixture).expect_err("an unrecognized width must refuse"),
            DirectCloseMakerPlanErrorV1::InvalidReplay
        );
    }

    /// O-016, red-proofed both ways: the caller names a maker and a rent owner
    /// because a message needs addresses, and neither name becomes authority.
    #[test]
    fn the_named_maker_and_rent_owner_are_verified_never_believed() {
        let mut wrong_maker = fixture(3, 0, 0);
        wrong_maker.snapshot.maker = key(99);
        assert_eq!(
            submit(&wrong_maker).expect_err("a mismatched maker must refuse"),
            DirectCloseMakerPlanErrorV1::InvalidReplay
        );

        // Substituting the beneficiary must not redirect the refund.
        let mut wrong_owner = fixture(3, 0, 0);
        wrong_owner.snapshot.rent_owner.key = key(98);
        assert_eq!(
            submit(&wrong_owner).expect_err("a substituted beneficiary must refuse"),
            DirectCloseMakerPlanErrorV1::InvalidRentOwner
        );

        // A program-owned beneficiary is an account whose bytes mean something
        // to somebody; crediting one is a write this route cannot authorize.
        let mut program_owned = fixture(3, 0, 0);
        program_owned.snapshot.rent_owner.owner = key(62);
        assert_eq!(
            submit(&program_owned).expect_err("a program-owned beneficiary must refuse"),
            DirectCloseMakerPlanErrorV1::InvalidRentOwner
        );
    }

    /// A drained count cannot decrement, and a root that never began retiring
    /// cannot close anything.
    #[test]
    fn a_drained_count_and_a_non_retiring_root_both_refuse_as_root_state() {
        let drained = fixture(0, 0, 0);
        assert_eq!(
            submit(&drained).expect_err("a drained count must refuse"),
            DirectCloseMakerPlanErrorV1::InvalidRootState
        );

        let mut open = fixture(3, 0, 0);
        open.authenticated.root_state = DirectRootStateV1::new();
        assert_eq!(
            submit(&open).expect_err("an Open root must refuse"),
            DirectCloseMakerPlanErrorV1::InvalidRootState
        );
    }

    /// The root-phase gate's own reason to exist.
    ///
    /// On the submit path a non-Retiring root is caught anyway, by the shared
    /// semantic function. The gate earns its place on the OTHER path: a vacant
    /// replay under an Open root must report that this market has not begun
    /// retiring, not a soothing `Complete` for a close that was never possible.
    /// Without the gate this case returns `Complete` and an operator concludes
    /// the work is done.
    #[test]
    fn a_vacant_replay_under_an_open_root_is_a_mistake_not_a_completed_close() {
        let mut fixture = fixture(3, 0, 0);
        fixture.authenticated.root_state = DirectRootStateV1::new();
        fixture.snapshot.maker_replay.owner = system_program::ID;
        fixture.snapshot.maker_replay.lamports = 0;
        fixture.snapshot.maker_replay.data = Vec::new();
        match assemble_plan(
            &fixture.snapshot,
            AuthenticatedCloseV1 {
                ..fixture.authenticated
            },
        ) {
            Err(DirectCloseMakerPlanErrorV1::InvalidRootState) => {}
            other => panic!("an Open root must refuse, not report Complete: {other:?}"),
        }
    }

    /// The closure owns its placement classes and refuses hostile identities
    /// and aliased coordinates without consulting any chain state.
    #[test]
    fn coordinate_closure_owns_placement_classes_and_refuses_hostile_identities() {
        let fixture = fixture(3, 0, 0);
        let report = submit(&fixture).expect("submit");
        assert_eq!(
            report.meta_closure.classes,
            DIRECT_CLOSE_MAKER_META_CLASSES_V1
        );
        assert_eq!(
            DIRECT_CLOSE_MAKER_META_CLASSES_V1.len(),
            DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1
        );
        // The route is permissionless, so no coordinate may be classed as a
        // signer that must stay in the static key set.
        assert!(
            !DIRECT_CLOSE_MAKER_META_CLASSES_V1
                .iter()
                .any(|class| *class == DirectCloseMakerMetaClassV1::InlineSigner)
        );
        // The two per-close coordinates must not be assumed by a lookup table
        // built for the market as a whole.
        assert_eq!(
            DIRECT_CLOSE_MAKER_META_CLASSES_V1[20],
            DirectCloseMakerMetaClassV1::InlineRequestBound
        );
        assert_eq!(
            DIRECT_CLOSE_MAKER_META_CLASSES_V1[21],
            DirectCloseMakerMetaClassV1::InlineRequestBound
        );

        let mut input = DirectCloseMakerCoordinateInputV1 {
            request: report.request,
            descriptor: [7; 32],
            account_profile: [8; 32],
            effect: [9; 32],
            root: key(20),
            manifest: [10; 32],
            program_set: [11; 32],
            config: [12; 32],
            release_set: [13; 32],
            registry_program: key(60),
            core_program: key(61),
            core_programdata: key(63),
            trading_program: key(62),
            trading_programdata: key(64),
            maker_replay: key(65),
            rent_owner: key(66),
        };
        derive_direct_close_maker_meta_closure_v1(input).expect("well formed identities");

        let mut zeroed = input;
        zeroed.effect = [0; 32];
        assert_eq!(
            derive_direct_close_maker_meta_closure_v1(zeroed).expect_err("zero identity"),
            DirectCloseMakerCoordinateErrorV1::InvalidIdentity
        );

        input.rent_owner = input.maker_replay;
        assert_eq!(
            derive_direct_close_maker_meta_closure_v1(input).expect_err("aliased coordinate"),
            DirectCloseMakerCoordinateErrorV1::AliasedCoordinate
        );
    }

    /// Cluster admission, observation freshness, and aliasing all refuse before
    /// any hostile bytes are decoded.
    #[test]
    fn wrong_cluster_stale_observation_and_alias_refuse_before_hostile_decode() {
        // A devnet claim over a loopback genesis, and a loopback claim over
        // devnet's: each is a typo the operator must hear about.
        let mut mislabelled = fixture(3, 0, 0);
        mislabelled.snapshot.genesis_hash = [3; 32];
        assert_eq!(
            plan_direct_close_maker_v1(&mislabelled.snapshot)
                .expect_err("a devnet claim must match devnet"),
            DirectCloseMakerPlanErrorV1::ClusterRefused
        );

        let mut loopback = fixture(3, 0, 0);
        loopback.snapshot.cluster = DirectCloseMakerClusterV1::OwnedLoopback;
        assert_eq!(
            plan_direct_close_maker_v1(&loopback.snapshot)
                .expect_err("a loopback claim must not admit devnet"),
            DirectCloseMakerPlanErrorV1::ClusterRefused
        );

        // Mainnet is refused under BOTH arms; no arm of this builder has a
        // reason to reach it.
        for cluster in [
            DirectCloseMakerClusterV1::Devnet,
            DirectCloseMakerClusterV1::OwnedLoopback,
        ] {
            let mut mainnet = fixture(3, 0, 0);
            mainnet.snapshot.cluster = cluster;
            mainnet.snapshot.genesis_hash = SOLANA_MAINNET_GENESIS_HASH_V1;
            assert_eq!(
                plan_direct_close_maker_v1(&mainnet.snapshot)
                    .expect_err("mainnet is never planned"),
                DirectCloseMakerPlanErrorV1::ClusterRefused
            );
        }

        let mut unfinalized = fixture(3, 0, 0);
        unfinalized.snapshot.maker_replay.observation.finality = Finality::Confirmed;
        assert_eq!(
            plan_direct_close_maker_v1(&unfinalized.snapshot)
                .expect_err("one unfinalized account taints the graph"),
            DirectCloseMakerPlanErrorV1::InvalidObservation
        );

        let mut aliased = fixture(3, 0, 0);
        aliased.snapshot.rent_owner.key = aliased.snapshot.maker_replay.key;
        assert_eq!(
            plan_direct_close_maker_v1(&aliased.snapshot).expect_err("aliased frame"),
            DirectCloseMakerPlanErrorV1::InvalidObservation
        );
    }
}
