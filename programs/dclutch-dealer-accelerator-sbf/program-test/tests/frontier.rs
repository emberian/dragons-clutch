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
//!    deployments re-observed against the releases the cache activated.
//!
//! and stops at `authenticate_market`, on the Core Market body. That is the
//! first gate the release waist cannot buy: it needs a real `CoreState` whose
//! identity re-derives its own `MarketCoreStateSeedsV2` PDA, which is the head
//! of the unwritten chain fixture staged in `crate::dealer_chain`.

use std::vec::Vec;

use dclutch_capability_program_contract::hot_v3::{
    HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
    HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
    HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3,
    HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3,
    HOT_ROOT_ACCOUNT_V3, HOT_TRADING_PROGRAMDATA_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
    HotExecutionEnvelopeV3,
};
use dclutch_core_contract::ContentId;
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ArtifactActivationInputV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1,
    activate_execution_role_into_v1, initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    ProgramIdentityV1,
};
use dclutch_dealer_accelerator_test_caller_sbf::dealer_accelerator_test_caller_authority_v1;
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_REQUEST_HEADER_BYTES_V2, AcceleratorRequestV2, RequestTransportV2,
};
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
const TRADING_UNSUPPORTED_CONTENT: u32 = 0x4000;
const TRADING_RELEASE: u32 = 0x4001;
const TRADING_ROOT: u32 = 0x4002;
const TRADING_CONTENT: u32 = 0x4003;
const TRADING_NATIVE_SIGNATURE: u32 = 0x4006;

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
    data.get_mut(45..)
        .expect("ELF tail")
        .copy_from_slice(elf);
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
    set(&mut fixed, HOT_CORE_PROGRAM_ACCOUNT_V3, loader_program_slot(core));
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

    let root_data = vec![0x5a_u8; 96];
    set(
        &mut fixed,
        HOT_ROOT_ACCOUNT_V3,
        Slot::new(key(0x11, 0), trading, root_data.clone()),
    );

    let market = key(0x12, 0);
    set(
        &mut fixed,
        HOT_MARKET_ACCOUNT_V3,
        Slot::new(market, core, vec![0x11; 64]),
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
        9,
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

    let root_key = fixed
        .get(HOT_ROOT_ACCOUNT_V3)
        .expect("root slot")
        .key;
    let (authority, _, _) = dealer_accelerator_test_caller_authority_v1(
        &trading,
        &hot_instruction,
        &root_key,
        &request_bytes,
    )
    .expect("canonical caller authority");

    let evidence = (0..ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4)
        .map(|index| {
            if index == 6 {
                Slot::program(accelerator)
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
        slots.push(clone_slot(fixed.get(coordinate).expect("runtime coordinate")));
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
    println!("admitted authentication frontier: {}", stage_name(&unbroken));

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

    // Each break above refuses at its own stage, so the unbroken run cleared
    // all three. What remains is the Core Market body: `authenticate_market`
    // (hot_v3.rs) refuses a Market whose width is not `STATE_BYTES` and whose
    // identity does not re-derive its own MarketCoreStateSeedsV2 PDA. That is
    // the first gate the release waist cannot buy -- it needs the chain.
    assert_eq!(
        unbroken,
        ProgramError::Custom(TRADING_CONTENT),
        "the frontier must be the Core Market body, not anything shallower; \
         observed {}",
        stage_name(&unbroken)
    );
    assert_ne!(
        unbroken,
        ProgramError::Custom(TRADING_UNSUPPORTED_CONTENT),
        "the lane must not be refusing on an unsupported content profile"
    );
}
