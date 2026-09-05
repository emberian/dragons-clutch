//! One decodable Market, capability root and activation cache, shared by every
//! family's bump-hint positive control.
//!
//! # Why this module exists
//!
//! Five families in this crate mine `HotBumpHintsV1` off chain, and each one
//! owns a CORPUS -- which coordinate of its own hot frame is the Market, which
//! is the root, which account names the Custody deployment, and which program
//! key each is derived under. The DERIVATION is shared
//! (`dclutch_hot_bump_miner_v1`) and has its own tests; the corpus is the part
//! that is per-family and that nothing tested until 2026-09-03.
//!
//! A wrong corpus is not a hazard -- the reader rebuilds every address with
//! `create_program_address` and refuses -- but it is INVISIBLE, which is worse
//! for a lane. Every family's existing fixture fills its Market and root
//! accounts with constant bytes, so `CoreState::decode` and
//! `CapabilityRootHeaderV1::decode` both fail, every slot degrades to zero, and
//! a builder that read the wrong coordinate would emit exactly the same
//! all-zero block as one that read the right one. "Nothing fired" and "my
//! instrument was disconnected" log identically.
//!
//! So this module stages bodies that DO decode, and hands each family's test
//! the three bumps derived independently, from the seed constructors, in the
//! shape `browser_bump_hint_vector` uses for the Direct route. Two authors: the
//! builder decodes account bodies and re-derives; this side keeps what fell out
//! of the `find_program_address` it made the fixture from.

use dclutch_market::capability_program::{
    CapabilityRootHeaderV1, SelectedRecordBumpsV1, hot_v3::HotBumpHintsV1,
};
use dclutch_core_contract::ContentId;
use dclutch_custody::CustodyAuthoritySeedsV1;
use dclutch_market::{
    CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness, StateBumpsV1,
};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ArtifactActivationInputV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1,
    activate_execution_role_into_v1, initialize_activation_cache_v1, put_activation_cache_bump_v1,
};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, CapabilityExecutionSelectionV1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
};
use solana_program::{hash::hash, pubkey::Pubkey};

use crate::{Finality, Observation, ObservedAccount, direct_inline_v3::ObservedAccountMetaV3};

/// Distinct constant fills, supplied as encoder INPUTS and never as answers.
const CORE_PROGRAM: [u8; 32] = [0x61; 32];
const CLAIMS_PROGRAM: [u8; 32] = [0x62; 32];
const CUSTODY_PROGRAM: [u8; 32] = [0x63; 32];
const REGISTRY_PROGRAM: [u8; 32] = [0x64; 32];
const LOADER_PROGRAM: [u8; 32] = [0x65; 32];
const MARKET_KEY: [u8; 32] = [0x66; 32];
const MARKET_ID: [u8; 32] = [0x67; 32];
const REALM: [u8; 32] = [0x68; 32];
const PRODUCT_RECORD: [u8; 32] = [0x69; 32];
const PRODUCT_ID: [u8; 32] = [0x6a; 32];
const RESOLUTION_POLICY: [u8; 32] = [0x6b; 32];
const CAPABILITY_MANIFEST: [u8; 32] = [0x6c; 32];
const RENT_BENEFICIARY: [u8; 32] = [0x6d; 32];
const SELECTION_KIND: [u8; 32] = [0x6e; 32];
const SELECTION_RELEASE: [u8; 32] = [0x6f; 32];
const SELECTION_CONFIG: [u8; 32] = [0x70; 32];

/// Immutable Market generation the staged root header and Market state agree on.
pub(crate) const GENERATION: u64 = 23;

/// Address of the Market ACCOUNT in the staged frame.
///
/// Distinct from the Market's own `market_id` identity on purpose: the Custody
/// transfer authority is seeded by the account address while the Core state PDA
/// is seeded by the identity, so a corpus that confused them would go red here
/// rather than merely mine a byte nobody spends.
pub(crate) fn market_key() -> Pubkey {
    Pubkey::new_from_array(MARKET_KEY)
}

/// Core program the Market state PDA is derived under -- frame coordinate 23.
pub(crate) fn core_program() -> Pubkey {
    Pubkey::new_from_array(CORE_PROGRAM)
}

fn identity(value: [u8; 32]) -> Identity {
    Identity::new(value).expect("distinct nonzero identity fill")
}

/// The one immutable execution release set every staged role resolves under.
fn release_set() -> ([u8; 32], ExecutionReleaseSetV1) {
    let role = |program: [u8; 32], semantic: u8| {
        ArtifactReleaseV1::new(
            ProgramIdentityV1::new(program).expect("program identity"),
            ProgramIdentityV1::new(LOADER_PROGRAM).expect("loader identity"),
            [semantic.wrapping_add(0x80); 32],
            ContentId::new([semantic; 32]).expect("semantic release"),
            [semantic.wrapping_add(0x90); 32],
            u64::from(semantic),
            ArtifactUpgradePolicyV1::Immutable,
            None,
        )
        .expect("canonical immutable artifact release")
    };
    let core = role(CORE_PROGRAM, 0x11);
    let claims = role(CLAIMS_PROGRAM, 0x12);
    let trading = role(trading_program().to_bytes(), 0x13);
    let custody = role(CUSTODY_PROGRAM, 0x14);
    let binding = |value: ArtifactReleaseV1| {
        ExecutionRoleBindingV1::new(
            value.program(),
            ArtifactReleaseIdV1::new(hash(&value.to_bytes()).to_bytes())
                .expect("artifact identity"),
        )
    };
    let set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(trading),
        binding(core),
        binding(custody),
    )
    .expect("five-role execution release set");
    (hash(&set.to_bytes()).to_bytes(), set)
}

/// Immutable release-set content identity the staged frame selects.
pub(crate) fn release_set_id() -> [u8; 32] {
    release_set().0
}

/// Trading program the capability root is derived under -- frame coordinate 25.
pub(crate) fn trading_program() -> Pubkey {
    Pubkey::new_from_array([0x71; 32])
}

/// Canonical open Core Market state, with its own bump UNRECORDED.
///
/// A Market that records its own bump makes the `market` hint inert --
/// `state.bumps.market.or(hint)` reaches the record and never the wire -- so a
/// corpus staged against a recorded bump would prove nothing about the slot.
/// This is the pre-tail shape, which is the case the hint still buys.
pub(crate) fn market_state_bytes() -> Vec<u8> {
    CoreState {
        phase: Phase::Open,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity: MarketIdentity {
            market_id: identity(MARKET_ID),
            realm_id: identity(REALM),
            product_record: identity(PRODUCT_RECORD),
            product_id: identity(PRODUCT_ID),
            resolution_policy: identity(RESOLUTION_POLICY),
            capability_manifest: identity(CAPABILITY_MANIFEST),
            selected_release_set: identity(release_set_id()),
            registry_program: identity(REGISTRY_PROGRAM),
            generation: GENERATION,
        },
        outstanding_capabilities: 1,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: identity(RENT_BENEFICIARY),
        terminal_receipt: None,
        bumps: StateBumpsV1::UNRECORDED,
    }
    .encode()
    .expect("canonical open Core Market state")
    .to_vec()
}

/// Canonical Trading capability root header, which carries its own seeds.
pub(crate) fn root_header_bytes() -> Vec<u8> {
    CapabilityRootHeaderV1::new(
        ContentId::new(release_set_id()).expect("release set"),
        MARKET_KEY,
        GENERATION,
        CapabilityExecutionSelectionV1::new(
            3,
            ContentId::new(CAPABILITY_MANIFEST).expect("manifest"),
            ContentId::new(SELECTION_KIND).expect("kind"),
            ContentId::new(SELECTION_RELEASE).expect("capability release"),
            ContentId::new(SELECTION_CONFIG).expect("config"),
        )
        .expect("execution selection")
        .with_capability_release_record_bumps(0xfd, 0xfc),
        SelectedRecordBumpsV1::new(0xff, 0xfe, 0xfb, 0xfa),
    )
    .expect("capability root header")
    .to_bytes()
    .to_vec()
}

/// Canonical Registry activation cache, which names the release set's Custody
/// deployment. Frame coordinate 22; Custody itself is not in the frame at all.
pub(crate) fn activation_cache_bytes() -> Vec<u8> {
    let (set_id, set) = release_set();
    let content = ContentId::new(set_id).expect("release set content");
    let mut cache = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut cache, content).expect("activation cache");
    let role = |program: [u8; 32], semantic: u8| {
        ArtifactReleaseV1::new(
            ProgramIdentityV1::new(program).expect("program identity"),
            ProgramIdentityV1::new(LOADER_PROGRAM).expect("loader identity"),
            [semantic.wrapping_add(0x80); 32],
            ContentId::new([semantic; 32]).expect("semantic release"),
            [semantic.wrapping_add(0x90); 32],
            u64::from(semantic),
            ArtifactUpgradePolicyV1::Immutable,
            None,
        )
        .expect("canonical immutable artifact release")
    };
    for (execution_role, program, semantic) in [
        (ExecutionRoleV1::Core, CORE_PROGRAM, 0x11_u8),
        (ExecutionRoleV1::Claims, CLAIMS_PROGRAM, 0x12),
        (ExecutionRoleV1::Trading, trading_program().to_bytes(), 0x13),
        (ExecutionRoleV1::Resolution, CORE_PROGRAM, 0x11),
        (ExecutionRoleV1::Custody, CUSTODY_PROGRAM, 0x14),
    ] {
        let release = role(program, semantic);
        let input = ArtifactActivationInputV1::new(
            ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes())
                .expect("artifact identity"),
            release,
            DeploymentObservationV1::new(
                release.program().to_bytes(),
                LOADER_PROGRAM,
                true,
                release.programdata(),
                LOADER_PROGRAM,
                false,
                release.programdata(),
                LOADER_PROGRAM,
                release.deployment_slot(),
                release.elf_digest(),
                release.upgrade_authority(),
            )
            .expect("current immutable deployment observation"),
        );
        activate_execution_role_into_v1(&mut cache, content, &set, execution_role, &input)
            .expect("activate exact role");
    }
    let (_, bump) = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &set_id],
        &Pubkey::new_from_array(REGISTRY_PROGRAM),
    );
    put_activation_cache_bump_v1(&mut cache, bump).expect("activation cache bump");
    cache
}

/// The three family-neutral slots, derived from the seeds this module BUILT
/// rather than from the bodies it emitted. The builder's side decodes those
/// bodies; a disagreement is a corpus that read the wrong coordinate.
pub(crate) fn expected_hints() -> HotBumpHintsV1 {
    let state = CoreState::decode(&market_state_bytes()).expect("staged Market state decodes");
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        &core_program(),
    )
    .1;
    let header =
        CapabilityRootHeaderV1::decode(&root_header_bytes()).expect("staged root header decodes");
    let root = Pubkey::find_program_address(&header.seeds().as_slices(), &trading_program()).1;
    let transfer_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(MARKET_KEY, release_set_id()).as_slices(),
        &Pubkey::new_from_array(CUSTODY_PROGRAM),
    )
    .1;
    HotBumpHintsV1 {
        market,
        root,
        child_relay: [0, transfer_authority],
        ..HotBumpHintsV1::ABSENT
    }
}

/// One observed account at an address, with the exact bytes a corpus decodes.
pub(crate) fn observed(key: Pubkey, data: Vec<u8>) -> ObservedAccountMetaV3 {
    ObservedAccountMetaV3 {
        account: ObservedAccount {
            observation: Observation {
                slot: 1,
                unix_timestamp: 0,
                finality: Finality::Finalized,
            },
            key,
            owner: Pubkey::default(),
            lamports: 1,
            executable: false,
            data,
        },
        is_signer: false,
        is_writable: false,
    }
}

/// A complete fixed frame whose Market, root, activation cache and Core program
/// coordinates decode, and whose other coordinates are distinct filler.
///
/// Every coordinate is named through the contract's own constant, so this
/// stages the frame the corpus functions read rather than a table of numbers.
pub(crate) fn fixed_frame() -> Vec<ObservedAccountMetaV3> {
    use dclutch_market::capability_program::hot_v3::{
        HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
        HOT_MARKET_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
    };
    let mut frame = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
        .map(|index| {
            let byte = u8::try_from(index).expect("frame index fits in a byte");
            observed(
                Pubkey::new_from_array([byte.wrapping_add(1); 32]),
                Vec::new(),
            )
        })
        .collect::<Vec<_>>();
    let mut put = |coordinate: usize, value: ObservedAccountMetaV3| {
        *frame
            .get_mut(coordinate)
            .expect("staged frame coordinate exists") = value;
    };
    put(
        HOT_MARKET_ACCOUNT_V3,
        observed(market_key(), market_state_bytes()),
    );
    put(
        HOT_ROOT_ACCOUNT_V3,
        observed(Pubkey::new_from_array([0x72; 32]), root_header_bytes()),
    );
    put(
        HOT_ACTIVATION_CACHE_ACCOUNT_V3,
        observed(Pubkey::new_from_array([0x73; 32]), activation_cache_bytes()),
    );
    put(
        HOT_CORE_PROGRAM_ACCOUNT_V3,
        observed(core_program(), Vec::new()),
    );
    put(
        HOT_TRADING_PROGRAM_ACCOUNT_V3,
        observed(trading_program(), Vec::new()),
    );
    frame
}

#[test]
fn the_staged_corpus_mines_every_slot_a_family_neutral_producer_can_reach() {
    // The positive control on the control. If this ever mines an absent block
    // the five family tests below it become vacuous -- each would compare two
    // searches and call it agreement.
    let expected = expected_hints();
    assert_ne!(expected, HotBumpHintsV1::ABSENT);
    assert_ne!(expected.market, 0);
    assert_ne!(expected.root, 0);
    assert_ne!(expected.child_relay[1], 0);
    // And the slots a family-neutral corpus cannot reach stay absent, so a
    // producer that started filling one without its family test following goes
    // red rather than silently disagreeing with the reader.
    assert_eq!(expected.lifecycle, [0, 0]);
    assert_eq!(expected.child_caller, [0, 0]);
    assert_eq!(expected.child_relay[0], 0);
}
