//! Stage-attributed frontier probe for the admitted-accelerator lane.
//!
//! `dclutch-dealer-accelerator-sbf` collapses every authentication refusal to
//! one opaque code (`DealerAcceleratorSbfErrorV4::InvalidInvocation`, 0xD001),
//! so a ProgramTest can say only *that* the lane refused, never *where*. This
//! probe calls the same `authenticate_accelerator_invocation_v4` the real ELF
//! calls, in-process, against a hand-built frame -- and `TradingSbfError`
//! carries one distinct code per authentication concern, so the refusal names
//! the stage that produced it.
//!
//! It is an instrument, not a proof of acceptance. What it pins is the exact
//! depth the lane currently reaches and the identity of the next gate.
//!
//! As of this probe the lane clears, in order:
//!
//! 1. `HotFrameV3::parse_accelerator_readonly` -- 39 pairwise-distinct
//!    read-only fixed accounts with the real Rent and Instructions identities;
//! 2. `authenticate_accelerator_top_level_v4` -- the top-level instruction read
//!    back out of the Instructions sysvar, with `fixed(39) ++ evidence(8) ++
//!    caller_authority` in canonical positions and only the root writable;
//! 3. the root-prestate digest against `envelope.root_prestate_digest()`;
//! 4. `authenticate_accelerator_activation_v4` -- the activation cache at its
//!    real PDA, five activated roles, and the Core and Trading Loader V3
//!    deployments re-observed against the releases the cache activated;
//! 5. `authenticate_market` -- a real `CoreState` at `STATE_BYTES`, canonical
//!    under its own encoder, whose identity re-derives its
//!    `MarketCoreStateSeedsV2` PDA and joins the envelope's release set,
//!    generation and Market;
//! 6. `TradingFamilyContextV1::authenticate` -- a real `CapabilityRootHeaderV1`
//!    at the PDA its own seeds derive under Trading, carrying the release set
//!    the activation cache activated and the Market and generation the
//!    envelope commits.
//!
//! and stops in `authenticate_product_runtime_v3`, on the Product graph: four
//! finalized Registry records (Product, ResultDomain, Portfolio, linked basis)
//! whose bodies must hash to the identities the Market's own `CoreState` and
//! the selection name. That is the head of the unwritten chain fixture staged
//! in `crate::dealer_chain`.
//!
//! Stages 5 and 6 were bought by DLR-HOT. Note what the two of them cost: a
//! `CoreState` and a root header are each about thirty lines of a probe that
//! owns both sides of the derivation. They sat unbought not because they were
//! expensive but because nothing had asked for them, which is the same reason
//! four always-refuses defects lived on this path for a whole wave.

use std::vec::Vec;

use dclutch_capability_program_contract::hot_v3::{
    HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3,
    HOT_CORE_PROGRAMDATA_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
    HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
    HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
    HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
    HOT_TRADING_PROGRAMDATA_ACCOUNT_V3, HotExecutionEnvelopeV3,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
};
use dclutch_core_contract::ContentId;
use dclutch_dealer_accelerator_test_caller_sbf::dealer_accelerator_test_caller_authority_v1;
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_REQUEST_HEADER_BYTES_V2, AcceleratorRequestV2, RequestTransportV2,
};
use dclutch_market_core_codec::{
    CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
    STATE_BYTES, StateBumpsV1,
};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ArtifactActivationInputV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1,
    activate_execution_role_into_v1, initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CapabilityExecutionSelectionV1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_trading_sbf::TradingSbfError;
use dclutch_trading_sbf::admitted_composition_v3::{
    ADMITTED_ACCELERATOR_RUNTIME_ACCOUNTS_START_V4, ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4,
};
use dclutch_trading_sbf::hot_v3::authenticate_accelerator_invocation_v4;
use solana_program::{
    account_info::AccountInfo, hash::hash, program_error::ProgramError, pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

/// One distinct authentication concern, as the code Trading actually returns.
///
/// These mirror `TradingSbfError`, which the accelerator crate does not
/// re-export; naming them here is what turns an opaque `Custom(_)` back into
/// the stage that produced it.
// DERIVED, never typed. `assert!(text.contains("Custom(3)"))` also accepts
// `Custom(30)`, and a hand-copied hex literal in a test is the same mistake one
// step earlier: it keeps passing after the enum it claims to name has moved.
const TRADING_UNSUPPORTED_CONTENT: u32 = TradingSbfError::UnsupportedContent as u32;
const TRADING_RELEASE: u32 = TradingSbfError::Release as u32;
const TRADING_ROOT: u32 = TradingSbfError::Root as u32;
const TRADING_CONTENT: u32 = TradingSbfError::Content as u32;
const TRADING_NATIVE_SIGNATURE: u32 = TradingSbfError::NativeSignature as u32;
const TRADING_ACCELERATOR_FRAME: u32 = TradingSbfError::AcceleratorFrame as u32;
const TRADING_ACCELERATOR_RELEASE: u32 = TradingSbfError::AcceleratorRelease as u32;
const TRADING_ACCELERATOR_ARTIFACT: u32 = TradingSbfError::AcceleratorArtifact as u32;
const TRADING_ACCELERATOR_RUNTIME_VIEW: u32 = TradingSbfError::AcceleratorRuntimeView as u32;

fn stage_name(error: &ProgramError) -> String {
    match error {
        ProgramError::Custom(TRADING_UNSUPPORTED_CONTENT) => {
            "UnsupportedContent (0x4000)".to_owned()
        }
        ProgramError::Custom(TRADING_RELEASE) => {
            "Release (0x4001) -- activation cache, role deployments, caller authority".to_owned()
        }
        ProgramError::Custom(TRADING_ROOT) => {
            "Root (0x4002) -- root prestate digest or TradingFamilyContextV1".to_owned()
        }
        ProgramError::Custom(TRADING_CONTENT) => {
            "Content (0x4003) -- frame shape, records, strategy, request join".to_owned()
        }
        ProgramError::Custom(TRADING_NATIVE_SIGNATURE) => {
            "NativeSignature (0x4006) -- Instructions sysvar / top-level metas".to_owned()
        }
        ProgramError::Custom(TRADING_ACCELERATOR_FRAME) => {
            "AcceleratorFrame (0x401A) -- callback account frame or request transport".to_owned()
        }
        ProgramError::Custom(TRADING_ACCELERATOR_RELEASE) => {
            "AcceleratorRelease (0x401B) -- Market or Rent rejoin".to_owned()
        }
        ProgramError::Custom(TRADING_ACCELERATOR_ARTIFACT) => {
            "AcceleratorArtifact (0x401C) -- Registry records, descriptor, strategy".to_owned()
        }
        ProgramError::Custom(TRADING_ACCELERATOR_RUNTIME_VIEW) => {
            "AcceleratorRuntimeView (0x401D) -- tail, spans, geometry, transcript".to_owned()
        }
        other => format!("{other:?}"),
    }
}

/// One owned account body the probe lends to the authentication chain.
struct Slot {
    key: Pubkey,
    lamports: u64,
    data: Vec<u8>,
    owner: Pubkey,
    executable: bool,
    signer: bool,
}

impl Slot {
    fn new(key: Pubkey, owner: Pubkey, data: Vec<u8>) -> Self {
        Self {
            key,
            lamports: 1,
            data,
            owner,
            executable: false,
            signer: false,
        }
    }

    fn program(key: Pubkey) -> Self {
        Self {
            key,
            lamports: 1,
            data: Vec::new(),
            owner: solana_sdk_ids::bpf_loader_upgradeable::ID,
            executable: true,
            signer: false,
        }
    }
}

fn infos<'a>(slots: &'a mut [Slot]) -> Vec<AccountInfo<'a>> {
    slots
        .iter_mut()
        .map(|slot| {
            let Slot {
                key,
                lamports,
                data,
                owner,
                executable,
                signer,
            } = slot;
            AccountInfo::new(key, *signer, false, lamports, data, owner, *executable)
        })
        .collect()
}

fn key(tag: u8, index: usize) -> Pubkey {
    let mut bytes = [tag; 32];
    let index = u32::try_from(index).expect("frame index width");
    let tail = index.to_le_bytes();
    for (slot, byte) in bytes
        .get_mut(28..32)
        .expect("pubkey tail")
        .iter_mut()
        .zip(tail)
    {
        *slot = byte;
    }
    Pubkey::new_from_array(bytes)
}

/// Serialize one instruction into the Instructions-sysvar wire layout that
/// `SysvarInstructionV1::read` decodes.
fn instructions_sysvar(
    program_id: &Pubkey,
    metas: &[(Pubkey, bool, bool)],
    data: &[u8],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    // One offset entry, pointing just past the count and the offset table.
    let offset = u16::try_from(bytes.len() + 2).expect("sysvar offset width");
    bytes.extend_from_slice(&offset.to_le_bytes());
    bytes.extend_from_slice(
        &u16::try_from(metas.len())
            .expect("sysvar meta count")
            .to_le_bytes(),
    );
    for (pubkey, is_signer, is_writable) in metas {
        let mut privileges = 0_u8;
        if *is_signer {
            privileges |= 1;
        }
        if *is_writable {
            privileges |= 1 << 1;
        }
        bytes.push(privileges);
        bytes.extend_from_slice(pubkey.as_ref());
    }
    bytes.extend_from_slice(program_id.as_ref());
    bytes.extend_from_slice(
        &u16::try_from(data.len())
            .expect("sysvar data width")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(data);
    // Trailing current-instruction index.
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes
}

/// Rent sysvar body, as `Rent::from_account_info` expects it.
///
/// The three fields are deprecated in the SDK and still exactly what the
/// sysvar account carries on the wire, so a probe that has to synthesize that
/// account reads them.
#[allow(deprecated)]
fn rent_sysvar_data() -> Vec<u8> {
    let rent = solana_program::rent::Rent::default();
    let mut bytes = Vec::with_capacity(17);
    bytes.extend_from_slice(&rent.lamports_per_byte_year.to_le_bytes());
    bytes.extend_from_slice(&rent.exemption_threshold.to_le_bytes());
    bytes.push(rent.burn_percent);
    bytes
}

/// A complete activated release set and the two role deployments it binds.
///
/// This is the release waist `direct-hot/src/waist.rs:194` installs, reduced to
/// what `authenticate_accelerator_activation_v4` re-observes: five activated
/// roles, and real Loader V3 Program/ProgramData bodies for Core and Trading.
/// The "ELF" behind each is arbitrary bytes -- the activation cache records
/// `hash(elf)` and the deployment check recomputes it, so a probe that owns
/// both sides needs no real artifact.
struct Waist {
    release_set_id: [u8; 32],
    activation: Pubkey,
    cache: Vec<u8>,
    core_elf: Vec<u8>,
    trading_elf: Vec<u8>,
}

fn programdata_key(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn artifact_release(program: Pubkey, semantic: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        ProgramIdentityV1::new(program.to_bytes()).expect("nonzero program identity"),
        ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader identity"),
        programdata_key(program).to_bytes(),
        ContentId::new([semantic; 32]).expect("semantic release"),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("immutable artifact release")
}

fn artifact_id(value: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&value.to_bytes()).to_bytes()).expect("artifact identity")
}

fn activation_input(value: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    ArtifactActivationInputV1::new(
        artifact_id(value),
        value,
        DeploymentObservationV1::new(
            value.program().to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            value.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            value.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            value.deployment_slot(),
            value.elf_digest(),
            value.upgrade_authority(),
        )
        .expect("current immutable deployment observation"),
    )
}

/// Loader V3 Program account body: variant two, then the ProgramData address.
fn loader_program_slot(program: Pubkey) -> Slot {
    let mut data = Vec::with_capacity(36);
    data.extend_from_slice(&2_u32.to_le_bytes());
    data.extend_from_slice(programdata_key(program).as_ref());
    Slot {
        key: program,
        lamports: 1,
        data,
        owner: bpf_loader_upgradeable::ID,
        executable: true,
        signer: false,
    }
}

/// Loader V3 ProgramData body with no upgrade authority, then the ELF tail.
fn loader_programdata_slot(program: Pubkey, elf: &[u8]) -> Slot {
    let mut data = vec![0_u8; 45 + elf.len()];
    data.get_mut(..4)
        .expect("loader variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    data.get_mut(45..).expect("ELF tail").copy_from_slice(elf);
    Slot::new(programdata_key(program), bpf_loader_upgradeable::ID, data)
}

fn release_waist(core: Pubkey, trading: Pubkey, registry: Pubkey) -> Waist {
    let core_elf = vec![0xc0_u8; 64];
    let trading_elf = vec![0x77_u8; 64];
    let claims_elf = vec![0xc1_u8; 64];
    let custody_elf = vec![0xcd_u8; 64];
    let core_release = artifact_release(core, 0x31, &core_elf);
    let trading_release = artifact_release(trading, 0x33, &trading_elf);
    let claims_release = artifact_release(key(0xc5, 0), 0x32, &claims_elf);
    let custody_release = artifact_release(key(0xc6, 0), 0x34, &custody_elf);
    let release_set = ExecutionReleaseSetV1::new(
        binding(core_release),
        binding(claims_release),
        binding(trading_release),
        binding(core_release),
        binding(custody_release),
    )
    .expect("complete execution release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let release_set_content = ContentId::new(release_set_id).expect("release set identity");
    let mut cache = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut cache, release_set_content).expect("activation cache");
    for (role, selected) in [
        (ExecutionRoleV1::Core, core_release),
        (ExecutionRoleV1::Claims, claims_release),
        (ExecutionRoleV1::Trading, trading_release),
        (ExecutionRoleV1::Resolution, core_release),
        (ExecutionRoleV1::Custody, custody_release),
    ] {
        activate_execution_role_into_v1(
            &mut cache,
            release_set_content,
            &release_set,
            role,
            &activation_input(selected),
        )
        .expect("activate exact role");
    }
    let activation =
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set_id], &registry).0;
    Waist {
        release_set_id,
        activation,
        cache,
        core_elf,
        trading_elf,
    }
}

fn binding(value: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(value.program(), artifact_id(value))
}

/// The generation the envelope, the Market and the root all commit.
const GENERATION: u64 = 9;

/// One live Core Market: the account body market-core writes, at its own PDA.
///
/// `authenticate_market` wants `STATE_BYTES` owned by the Core program, a body
/// that is canonical under `CoreState`'s own encoder, an identity that
/// re-derives its `MarketCoreStateSeedsV2` address, and a `market_id` equal to
/// that address. The seed projection excludes `market_id` -- that is what makes
/// this constructible at all: derive from the other eight coordinates, then
/// write the derived address back as the ninth.
///
/// `realm_tag` exists so one caller can ask for a Market whose identity is
/// well-formed but is not the one at this address. That is a substitution the
/// PDA check must catch, and it catches it *after* the decode and the canonical
/// re-encode, so nothing shallower can be what refuses.
fn core_market(
    core: Pubkey,
    registry: Pubkey,
    release_set: [u8; 32],
    realm_tag: u8,
) -> (Pubkey, Vec<u8>) {
    let identity = |tag: u8| CoreIdentity::new([tag; 32]).expect("nonzero core identity");
    let mut market_identity = MarketIdentity {
        market_id: identity(0xa1),
        realm_id: identity(realm_tag),
        product_record: identity(0xa3),
        product_id: identity(0xa4),
        resolution_policy: identity(0xa5),
        capability_manifest: identity(0xa6),
        selected_release_set: CoreIdentity::new(release_set).expect("nonzero release set"),
        registry_program: CoreIdentity::new(registry.to_bytes()).expect("nonzero registry"),
        generation: GENERATION,
    };
    let key = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market_identity).as_slices(),
        &core,
    )
    .0;
    market_identity.market_id = CoreIdentity::new(key.to_bytes()).expect("derived market id");
    let body = CoreState {
        phase: Phase::Open,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity: market_identity,
        outstanding_capabilities: 1,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: identity(0xa9),
        terminal_receipt: None,
        bumps: StateBumpsV1::UNRECORDED,
    }
    .encode()
    .expect("canonical Open Market state");
    assert_eq!(body.len(), STATE_BYTES, "the Market body is the chain's");
    (key, body.to_vec())
}

/// One live Trading capability root: the header at the PDA its own seeds derive.
///
/// `TradingFamilyContextV1::authenticate` decodes the header out of the front
/// of the account, requires the account be owned by Trading and wider than the
/// header, requires the activation's Trading receipt to carry the header's own
/// release set, and requires the header's seeds to derive the account's key.
/// `authenticate_accelerator_invocation_v4` then rejoins the header's Market,
/// release set and generation to the envelope's.
///
/// `generation` is a parameter so one caller can ask for a root that is
/// well-formed and is not this invocation's: it moves both the derived address
/// and the envelope join at once, which is what a stale root actually looks
/// like.
fn trading_root(
    trading: Pubkey,
    release_set: [u8; 32],
    market: Pubkey,
    generation: u64,
) -> (Pubkey, Vec<u8>) {
    let content = |tag: u8| ContentId::new([tag; 32]).expect("nonzero selection content");
    let selection = CapabilityExecutionSelectionV1::new(
        0,
        content(0xb1),
        content(0xb2),
        content(0xb3),
        content(0xb4),
    )
    .expect("canonical execution selection")
    .with_capability_release_record_bumps(255, 254);
    let header = CapabilityRootHeaderV1::new(
        ContentId::new(release_set).expect("nonzero release set"),
        market.to_bytes(),
        generation,
        selection,
        SelectedRecordBumpsV1::from_bytes([255, 254, 253, 252]),
    )
    .expect("canonical root header");
    let key = Pubkey::find_program_address(&header.seeds().as_slices(), &trading).0;
    let mut body = header.to_bytes().to_vec();
    // The header is the immutable prefix; the family tail is what the
    // descriptor's root schema names, and nothing this deep in the lane reads
    // it yet. It only has to exist, because a root account exactly the width of
    // its own header is refused as having no state at all.
    body.extend_from_slice(&[0x5a_u8; 96]);
    assert!(
        body.len() > CAPABILITY_ROOT_HEADER_BYTES_V1,
        "a root account must carry a family tail past its header"
    );
    (key, body)
}

/// Everything the probe needs to hand one invocation to Trading.
struct Frame {
    slots: Vec<Slot>,
    request_bytes: Vec<u8>,
    accelerator: Pubkey,
}

/// One deliberate break, so a cleared stage can be shown cleared rather than
/// assumed cleared.
///
/// A single probe that stops at stage N proves only that stages after N are
/// unreached. Re-running it with an earlier stage broken and watching the
/// refusal move BACK is what shows the earlier stage was being executed and
/// passed, not skipped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Break {
    /// Nothing broken: the deepest frame this lane can currently reach.
    None,
    /// The top-level instruction claims a program that is not Trading.
    TopLevel,
    /// The Hot envelope commits a root prestate the root does not hash to.
    RootPrestate,
    /// The activation cache body is not the one the release set activated.
    Activation,
    /// The Core Market body is a canonical `CoreState` for a different Market.
    Market,
    /// The Trading root is a canonical header for a different generation.
    RootHeader,
}

/// Build the deepest well-formed frame this lane can currently reach.
///
/// Shape-complete for `parse_accelerator_readonly` and for
/// `authenticate_accelerator_top_level_v4` -- 39 pairwise-distinct read-only
/// fixed accounts with the real sysvar identities, the eight admitted
/// strategy-evidence accounts, one caller authority, the five fixed runtime
/// coordinates -- and content-complete through the release waist: five
/// activated roles in a real activation cache at its real PDA, with real
/// Loader V3 Program and ProgramData bodies for Core and Trading.
///
/// Everything past that -- the Core Market, the finalized records, the
/// AdmittedAot strategy, the Dealer scenario state -- is the unwritten chain
/// fixture staged in `crate::dealer_chain`, so the probe stops where that
/// begins.
fn frame(broken: Break) -> Frame {
    let accelerator = key(0xd1, 0);
    let trading = key(0xd2, 0);
    let core = key(0xd3, 0);
    let registry = key(0xd4, 0);
    let family_request = [7_u8; 4];
    let mut waist = release_waist(core, trading, registry);
    if broken == Break::Activation {
        // Flip one byte of an activated role rather than truncating: the cache
        // stays the right width at the right PDA, so what refuses is the
        // activation content and nothing shallower.
        if let Some(byte) = waist.cache.last_mut() {
            *byte ^= 0xff;
        }
    }

    let mut fixed = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
        .map(|index| {
            let body = u8::try_from(index % 251).expect("placeholder body byte");
            Slot::new(key(0x10, index), system_program::ID, vec![body; 8])
        })
        .collect::<Vec<_>>();

    // The identities `parse_accelerator_readonly` pins by name, and the two
    // role deployments `authenticate_accelerator_activation_v4` re-observes.
    set(
        &mut fixed,
        HOT_TRADING_PROGRAM_ACCOUNT_V3,
        loader_program_slot(trading),
    );
    set(
        &mut fixed,
        HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
        loader_programdata_slot(trading, &waist.trading_elf),
    );
    set(
        &mut fixed,
        HOT_CORE_PROGRAM_ACCOUNT_V3,
        loader_program_slot(core),
    );
    set(
        &mut fixed,
        HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
        loader_programdata_slot(core, &waist.core_elf),
    );
    set(
        &mut fixed,
        HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
        Slot::program(registry),
    );
    set(
        &mut fixed,
        HOT_ACTIVATION_CACHE_ACCOUNT_V3,
        Slot::new(waist.activation, registry, waist.cache.clone()),
    );
    set(
        &mut fixed,
        HOT_RENT_SYSVAR_ACCOUNT_V3,
        Slot::new(sysvar::rent::ID, sysvar::ID, rent_sysvar_data()),
    );
    // The sysvar's own identity is one of the metas it will carry, so the slot
    // has to hold its real key before the meta list is taken from the frame.
    set(
        &mut fixed,
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
        Slot::new(sysvar::instructions::ID, sysvar::ID, Vec::new()),
    );

    // The Market comes before the root: the root header commits the Market, so
    // the Market's address is an input to the root's address.
    let (market, canonical_market) = core_market(core, registry, waist.release_set_id, 0xa2);
    let market_body = if broken == Break::Market {
        // A canonical `CoreState` -- for a Market whose Realm differs, so its
        // own seeds derive a different address. Same width, same magic, same
        // canonical re-encode: the only thing wrong with this body is that it
        // is not the body of the Market at this address, which is precisely
        // what the PDA re-derivation exists to catch and nothing shallower can.
        core_market(core, registry, waist.release_set_id, 0xb7).1
    } else {
        canonical_market
    };
    set(
        &mut fixed,
        HOT_MARKET_ACCOUNT_V3,
        Slot::new(market, core, market_body),
    );

    let root_generation = if broken == Break::RootHeader {
        GENERATION + 1
    } else {
        GENERATION
    };
    // Installed at whichever address its own seeds derive, so the broken case
    // is not a misplaced root: it is a real root for a generation this
    // invocation is not executing, and the join to the envelope is what refuses.
    let (root_key, root_data) =
        trading_root(trading, waist.release_set_id, market, root_generation);
    set(
        &mut fixed,
        HOT_ROOT_ACCOUNT_V3,
        Slot::new(root_key, trading, root_data.clone()),
    );

    let root_prestate = if broken == Break::RootPrestate {
        hash(b"a prestate this root never had").to_bytes()
    } else {
        hash(&root_data).to_bytes()
    };
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(family_request.len()).expect("family request width"),
        waist.release_set_id,
        market.to_bytes(),
        GENERATION,
        root_prestate,
    )
    .expect("Hot envelope");
    let mut hot_instruction = envelope.to_bytes().to_vec();
    hot_instruction.extend_from_slice(&family_request);

    let bank = [0_u8; 8];
    let request = AcceleratorRequestV2::new(
        RequestTransportV2::Inline,
        content(1),
        content(2),
        content(3),
        content(4),
        ContentId::new(hash(&bank).to_bytes()).expect("input bank digest"),
        1,
        1,
        0,
        0,
        &bank,
    )
    .expect("canonical request");
    let mut request_bytes = vec![0_u8; ACCELERATOR_REQUEST_HEADER_BYTES_V2 + bank.len()];
    request
        .encode_into(&mut request_bytes)
        .expect("request encoding");

    let root_key = fixed.get(HOT_ROOT_ACCOUNT_V3).expect("root slot").key;
    let (authority, _, _) = dealer_accelerator_test_caller_authority_v1(
        &trading,
        &hot_instruction,
        &root_key,
        &request_bytes,
    )
    .expect("canonical caller authority");

    // The last two evidence accounts are the accelerator's own Loader V3
    // deployment, and they carry REAL BODIES because the callback now attests
    // it: `2b8f87a0` made `authenticate_accelerator_invocation_v4` parse the
    // ProgramData metadata -- deployment slot and upgrade authority -- into the
    // authenticated caller, so the immutability of the program being invoked is
    // part of what the caller authority binds.
    //
    // This fixture staged eight bytes of `0x20` there. `ProgramDataMetadataV3View::parse`
    // wants forty-five and a variant tag of three, so it refused `InvalidLength`
    // and the probe published `Release` (0x4001) from the `acc-toplevel` block
    // -- BEFORE the root-prestate compare. Every stage this file claims to
    // clear from 3 onward was being asserted against a refusal that never
    // reached it, and `Break::Activation` passed for the wrong reason, because
    // `Release` is also what a corrupted activation cache raises. The stale
    // side was the fixture; the program's law is unchanged and the expectations
    // below always described it correctly.
    //
    // Positions are DERIVED from the evidence width, the same subtraction the
    // program does, so a widened evidence frame moves both sides together.
    let accelerator_elf = vec![0xda_u8; 64];
    let program_index = ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4 - 2;
    let programdata_index = ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4 - 1;
    let evidence = (0..ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4)
        .map(|index| {
            if index == program_index {
                loader_program_slot(accelerator)
            } else if index == programdata_index {
                loader_programdata_slot(accelerator, &accelerator_elf)
            } else {
                Slot::new(key(0x20, index), system_program::ID, vec![0x20; 8])
            }
        })
        .collect::<Vec<_>>();

    // Canonical top-level layout: fixed(39) ++ evidence(8) ++ authority(1),
    // with only the root writable. This is exactly the span
    // `authenticate_accelerator_top_level_v4` walks.
    let mut metas = fixed
        .iter()
        .enumerate()
        .map(|(index, slot)| (slot.key, false, index == HOT_ROOT_ACCOUNT_V3))
        .collect::<Vec<_>>();
    metas.extend(evidence.iter().map(|slot| (slot.key, false, false)));
    metas.push((authority, false, false));

    // The top-level instruction must name Trading (or the Registry); naming a
    // third program is what `authenticate_accelerator_top_level_v4` refuses.
    let top_level_program = if broken == Break::TopLevel {
        key(0xee, 0)
    } else {
        trading
    };
    let sysvar_data = instructions_sysvar(&top_level_program, &metas, &hot_instruction);
    set(
        &mut fixed,
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
        Slot::new(sysvar::instructions::ID, sysvar::ID, sysvar_data),
    );

    let runtime_coordinates = [
        HOT_ROOT_ACCOUNT_V3,
        HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
    ];

    let mut slots = Vec::with_capacity(
        ADMITTED_ACCELERATOR_RUNTIME_ACCOUNTS_START_V4 + runtime_coordinates.len(),
    );
    let mut caller = Slot::new(authority, system_program::ID, Vec::new());
    caller.signer = true;
    slots.push(caller);
    for slot in &fixed {
        slots.push(clone_slot(slot));
    }
    for slot in &evidence {
        slots.push(clone_slot(slot));
    }
    for coordinate in runtime_coordinates {
        slots.push(clone_slot(
            fixed.get(coordinate).expect("runtime coordinate"),
        ));
    }

    Frame {
        slots,
        request_bytes,
        accelerator,
    }
}

fn clone_slot(slot: &Slot) -> Slot {
    Slot {
        key: slot.key,
        lamports: slot.lamports,
        data: slot.data.clone(),
        owner: slot.owner,
        executable: slot.executable,
        signer: slot.signer,
    }
}

fn set(slots: &mut [Slot], index: usize, value: Slot) {
    *slots.get_mut(index).expect("fixed slot") = value;
}

fn content(value: u8) -> ContentId {
    ContentId::new([value; 32]).expect("nonzero fixture content")
}

/// Run one probe and return the refusal Trading produced.
fn probe(broken: Break) -> ProgramError {
    let mut fixture = frame(broken);
    let accelerator = fixture.accelerator;
    let request_bytes = fixture.request_bytes.clone();
    let accounts = infos(&mut fixture.slots);
    let outcome = authenticate_accelerator_invocation_v4(&accelerator, &accounts, &request_bytes);
    assert!(
        outcome.is_err(),
        "the admitted lane authenticated a probe frame that has no Market, no \
         finalized records and no AdmittedAot strategy behind it"
    );
    outcome.err().expect("the refusal just asserted")
}

/// Pin the exact depth the admitted lane reaches, and name the next gate.
///
/// This is a frontier marker, not an acceptance test -- reaching
/// `AcceleratorDispositionV2::Accepted` needs the whole Dealer scenario chain.
/// What it pins is that the refusal has moved off frame *geometry*, where the
/// lane sat for its entire history, and onto chain *content*.
///
/// The three deliberate breaks are what make the clearing measured rather than
/// assumed: each one moves the refusal back to the stage it broke, which is
/// only possible if that stage was being executed and passed in the unbroken
/// run.
#[test]
fn admitted_authentication_clears_geometry_and_the_release_waist() {
    let unbroken = probe(Break::None);
    println!(
        "admitted authentication frontier: {}",
        stage_name(&unbroken)
    );

    assert_eq!(
        probe(Break::TopLevel),
        ProgramError::Custom(TRADING_NATIVE_SIGNATURE),
        "a top-level instruction that is not Trading must refuse in \
         authenticate_accelerator_top_level_v4"
    );
    assert_eq!(
        probe(Break::RootPrestate),
        ProgramError::Custom(TRADING_ROOT),
        "an envelope whose root prestate the root does not hash to must refuse \
         on the root, after the top-level metas have already joined"
    );
    assert_eq!(
        probe(Break::Activation),
        ProgramError::Custom(TRADING_RELEASE),
        "a corrupted activation cache must refuse in \
         authenticate_accelerator_activation_v4, after the root prestate has \
         already joined"
    );
    assert_eq!(
        probe(Break::RootHeader),
        ProgramError::Custom(TRADING_ROOT),
        "a canonical root header for another generation must refuse on the \
         family context, after the Market has already joined"
    );

    // `Break::RootHeader` is what certifies stage 5. The family context is
    // strictly downstream of `authenticate_market`, and in that break the root
    // PRESTATE is intact -- the root body is real, and the digest the envelope
    // commits is taken from it -- so the only thing left upstream that raises
    // Root has already passed. A Root refusal there is therefore reachable only
    // by executing the Market check and passing it.
    //
    // `Break::Market` USED TO BE unreadable off the code alone, because the
    // Market rejoin and the frontier both refused `Content` -- 2,126 sites --
    // and this comment said so rather than papering over it. The four-conjunct
    // split makes it readable: the Market is the release waist this callback
    // rejoins, the frontier is the artifact graph beyond it, and they now have
    // different names. A substituted Market is the exact defect class this lane
    // has already paid for twice, and it is now asserted positively.
    let substituted_market = probe(Break::Market);
    assert_eq!(
        substituted_market,
        ProgramError::Custom(TRADING_ACCELERATOR_RELEASE),
        "a canonical CoreState for another Market must refuse on the release \
         waist rejoin; observed {}",
        stage_name(&substituted_market)
    );

    // The frontier is now the Product graph: `authenticate_product_runtime_v3`
    // wants four finalized Registry records whose bodies hash to the identities
    // the Market's own CoreState and the selection name. That is the head of
    // the Dealer chain fixture, and no waist can buy it either.
    assert_eq!(
        unbroken,
        ProgramError::Custom(TRADING_ACCELERATOR_ARTIFACT),
        "the frontier must be the Product graph, not anything shallower; \
         observed {}",
        stage_name(&unbroken)
    );
    assert_ne!(
        unbroken,
        ProgramError::Custom(TRADING_ACCELERATOR_RELEASE),
        "the Market rejoin is bought; an AcceleratorRelease refusal here is a \
         regression in it"
    );
    assert_ne!(
        unbroken,
        ProgramError::Custom(TRADING_ACCELERATOR_FRAME),
        "the callback frame is bought; an AcceleratorFrame refusal here is a \
         regression in it"
    );
    assert_ne!(
        unbroken,
        ProgramError::Custom(TRADING_UNSUPPORTED_CONTENT),
        "the lane must not be refusing on an unsupported content profile"
    );
    assert_ne!(
        unbroken,
        ProgramError::Custom(TRADING_ROOT),
        "the Market and the root are bought; a Root refusal here is a \
         regression in one of them"
    );
    assert_ne!(
        unbroken,
        ProgramError::Custom(TRADING_RELEASE),
        "the release waist is bought; a Release refusal here is a regression"
    );
}
