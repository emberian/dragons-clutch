//! Chain-derived General V3 capability-activation tests.
//!
//! Every fixture is assembled from semantic-owner encoders and real PDA
//! derivations; nothing here hand-writes a root, a funding derivation, or a
//! selection. Two of these tests used to be the executable form of the blockers
//! this module's header named; both blockers are closed, and the two tests now
//! pin the premises the fixes rest on.

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityFundingDerivationV1,
    CompartmentFundingV1, FundingAmountsV1, FundingQuoteV1, FundingStatus, MANIFEST_HEADER_BYTES,
    MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_capability_program_contract::set_v2::{
    CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, SelectorWidthV2,
    encode_program_set_v2, encoded_program_set_bytes_v2,
};
use dclutch_capability_program_contract::v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID;
use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET, CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET,
    CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET, CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET,
    CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET, CAPABILITY_PROGRAM_HEADER_BYTES_V1,
    CAPABILITY_PROGRAM_KIND_OFFSET, CAPABILITY_PROGRAM_MAGIC_V1, CAPABILITY_PROGRAM_PROFILE_OFFSET,
    CAPABILITY_PROGRAM_PROFILE_V2, CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET,
    CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET, CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
    CapabilityProgramV1, initialize_root_account_v1,
};
use dclutch_general_config_contract::v3::GeneralConfigV3Input;
use dclutch_market_core_codec::{Identity, MarketIdentity, Readiness};

use super::*;

const CORE_PROGRAM: Pubkey = Pubkey::new_from_array([0x72; 32]);
const TRADING_PROGRAM: Pubkey = Pubkey::new_from_array([0x71; 32]);
const GENERATION: u64 = 7;
const ROOT_RENT: u64 = 3_000_000;
const FUNDING_RENT: u64 = 2_500_000;
const SLOT: u64 = 5_000;

fn observation() -> Observation {
    Observation {
        slot: SLOT,
        unix_timestamp: 1_800_000_000,
        finality: Finality::Finalized,
    }
}

fn account(key: Pubkey, owner: Pubkey, lamports: u64, data: Vec<u8>) -> ObservedAccount {
    ObservedAccount {
        observation: observation(),
        key,
        owner,
        lamports,
        executable: false,
        data,
    }
}

fn identity(bytes: [u8; 32]) -> Identity {
    Identity::new(bytes).expect("nonzero identity")
}

/// Exact seven-action General ProgramSet, so `entry.release_id` is what the
/// runtime-width hot path will later demand at `selection.capability_release`.
fn program_set() -> Vec<u8> {
    let entries = (0..7_u32)
        .map(|selector| {
            CapabilityProgramSetEntryV2::new(
                selector,
                CapabilityDescriptorReferenceV2::new(
                    ContentId::new(CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID).expect("schema"),
                    ContentId::new(
                        [0xd0_u8.wrapping_add(u8::try_from(selector).expect("selector byte")); 32],
                    )
                    .expect("descriptor"),
                ),
            )
        })
        .collect::<Vec<_>>();
    let mut output = vec![0_u8; encoded_program_set_bytes_v2(entries.len()).expect("set width")];
    encode_program_set_v2(10, SelectorWidthV2::U8, &entries, &mut output).expect("program set");
    output
}

fn config_bytes(program_set_id: [u8; 32]) -> Vec<u8> {
    GeneralConfigV3::new(GeneralConfigV3Input {
        capacity_profile_id: [0x41; 32],
        claim_basis_id: [0x42; 32],
        program_set_id,
        generation: GENERATION,
        price_scale: 1_000,
        collection_slots: 16,
        selection_slots: 16,
        settlement_slots: 64,
        max_orders_per_candidate: 32,
        max_pages_per_candidate: 32,
        continuation_reward_lamports: 1,
        selection_policy_id: [0x43; 32],
        quote_surplus_beneficiary: [0x44; 32],
    })
    .expect("config")
    .to_bytes()
    .to_vec()
}

fn general_entry(config_id: ContentId, program_set_id: [u8; 32]) -> CapabilityEntryV1 {
    let amounts = FundingAmountsV1::new(
        CompartmentFundingV1::native_lamports(ROOT_RENT).expect("root rent quote"),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
    )
    .expect("amounts");
    CapabilityEntryV1::new(
        ContentId::new(GENERAL_CAPABILITY_KIND_ID_V1).expect("kind"),
        ContentId::new(program_set_id).expect("program set"),
        config_id,
        ContentId::new([0x41; 32]).expect("capacity"),
        ContentId::new(GENERAL_ROOT_SCHEMA_ID_V2).expect("root schema"),
        ContentId::new([0x82; 32]).expect("derivation"),
        ActivationPolicy::PrepaidLazy,
        1_000_000,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(amounts, None).expect("quote"),
    )
    .expect("entry")
}

struct Fixture {
    state: GeneralActivationStateV3,
    manifest_id: ContentId,
    config_id: ContentId,
    program_set: Vec<u8>,
}

fn fixture(phase: Phase, entries: &[CapabilityEntryV1]) -> Fixture {
    let program_set = program_set();
    let program_set_id = hash(&program_set).to_bytes();
    let config = config_bytes(program_set_id);
    let config_id = ContentId::new(hash(&config).to_bytes()).expect("config id");

    let mut manifest_bytes =
        vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(entries, &mut manifest_bytes).expect("manifest");
    let manifest_id = ContentId::new(hash(&manifest_bytes).to_bytes()).expect("manifest id");

    // `CoreState::valid_static` binds phase to readiness and terminal receipt;
    // the fixture takes both from the phase rather than asserting a shape the
    // codec would refuse.
    let readiness = match phase {
        Phase::Founding => Readiness::Prepaid,
        _ => Readiness::Consumed,
    };
    let terminal_receipt = match phase {
        Phase::Founding | Phase::Open => None,
        _ => Some(identity([0x29; 32])),
    };
    let mut core = CoreState {
        phase,
        readiness,
        terminal_winner: 0,
        identity: MarketIdentity {
            market_id: identity([0x21; 32]),
            realm_id: identity([0x22; 32]),
            product_record: identity([0x23; 32]),
            product_id: identity([0x24; 32]),
            resolution_policy: identity([0x25; 32]),
            capability_manifest: identity(manifest_id.to_bytes()),
            selected_release_set: identity([0x26; 32]),
            registry_program: identity([0x27; 32]),
            generation: GENERATION,
        },
        outstanding_capabilities: 0,
        rent_beneficiary: identity([0x28; 32]),
        terminal_receipt,
    };
    let market_key = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        &CORE_PROGRAM,
    )
    .0;
    core.identity.market_id = identity(market_key.to_bytes());
    let market_bytes = core.encode().expect("core state").to_vec();

    let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest decode");
    let entry_index = manifest_general_entry_index(manifest, config_id);
    let quoted = manifest
        .entry(entry_index)
        .expect("quoted entry")
        .funding_quote()
        .native_lamports_total();
    let custody = FundingCustodyObservationV1::native_only(FUNDING_RENT + quoted, FUNDING_RENT)
        .expect("custody");
    let funding =
        FundingStateV1::new(manifest_id, manifest, entry_index, custody).expect("funding");
    let funding_key = Pubkey::find_program_address(
        &CapabilityFundingDerivationV1::new(
            market_key.to_bytes(),
            GENERATION,
            manifest_id,
            manifest,
            funding,
        )
        .expect("derivation")
        .seed_components(),
        &TRADING_PROGRAM,
    )
    .0;

    let entry = manifest.entry(entry_index).expect("entry");
    let selection = CapabilityExecutionSelectionV1::new(
        entry_index,
        manifest_id,
        entry.kind_id(),
        entry.release_id(),
        config_id,
    )
    .expect("selection");
    let header = CapabilityRootHeaderV1::new(
        ContentId::new(core.identity.selected_release_set.to_bytes()).expect("release set"),
        market_key.to_bytes(),
        GENERATION,
        selection,
    )
    .expect("header");
    let root_key = general_capability_root_address_v3(header, &TRADING_PROGRAM).0;

    Fixture {
        state: GeneralActivationStateV3 {
            market: account(market_key, CORE_PROGRAM, 1_000_000, market_bytes),
            manifest_record: account(
                Pubkey::new_from_array([0xa2; 32]),
                Pubkey::new_from_array([0x27; 32]),
                1,
                manifest_bytes,
            ),
            config_record: account(
                Pubkey::new_from_array([0xa3; 32]),
                Pubkey::new_from_array([0x27; 32]),
                1,
                config,
            ),
            funding_state: account(
                funding_key,
                TRADING_PROGRAM,
                FUNDING_RENT + quoted,
                funding.to_bytes().to_vec(),
            ),
            capability_root: account(root_key, system_program::ID, 0, Vec::new()),
            core_program: CORE_PROGRAM,
            trading_program: TRADING_PROGRAM,
            exact_root_rent_lamports: ROOT_RENT,
            exact_funding_rent_lamports: FUNDING_RENT,
            current_slot: SLOT,
            minimum_finalized_slot: SLOT,
        },
        manifest_id,
        config_id,
        program_set,
    }
}

/// Fixture-side index lookup; zero when the manifest selects no General entry,
/// so the no-General case still builds a coherent FundingState to refuse on.
fn manifest_general_entry_index(manifest: CapabilityManifestV1<'_>, config_id: ContentId) -> u16 {
    let kind = ContentId::new(GENERAL_CAPABILITY_KIND_ID_V1).expect("kind");
    let mut index = 0_u16;
    while index < manifest.entry_count() {
        let entry = manifest.entry(index).expect("entry");
        if entry.kind_id() == kind && entry.config_id() == config_id {
            return index;
        }
        index += 1;
    }
    0
}

/// Three entries in the manifest's canonical ascending kind order.
///
/// `GENERAL_CAPABILITY_KIND_ID_V1` begins `0xcb`, so the two neighbours are
/// chosen to sit on either side of it. The General entry is deliberately not at
/// index zero: the planner must find it rather than assume a position.
fn one_general_entry(config_id: ContentId, program_set_id: [u8; 32]) -> Vec<CapabilityEntryV1> {
    vec![
        other_entry(0x51),
        general_entry(config_id, program_set_id),
        other_entry(0xf1),
    ]
}

fn other_entry(seed: u8) -> CapabilityEntryV1 {
    let amounts = FundingAmountsV1::new(
        CompartmentFundingV1::native_lamports(11).expect("rent"),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
    )
    .expect("amounts");
    CapabilityEntryV1::new(
        ContentId::new([seed; 32]).expect("kind"),
        ContentId::new([seed.wrapping_add(1); 32]).expect("release"),
        ContentId::new([seed.wrapping_add(2); 32]).expect("config"),
        ContentId::new([seed.wrapping_add(3); 32]).expect("capacity"),
        ContentId::new([seed.wrapping_add(4); 32]).expect("child schema"),
        ContentId::new([seed.wrapping_add(5); 32]).expect("derivation"),
        ActivationPolicy::PrepaidLazy,
        1_000_000,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(amounts, None).expect("quote"),
    )
    .expect("entry")
}

fn open_fixture() -> Fixture {
    // Two passes: the entry needs the config identity, and the config needs the
    // ProgramSet identity, so build the identities first with a throwaway.
    let program_set = program_set();
    let program_set_id = hash(&program_set).to_bytes();
    let config_id = ContentId::new(hash(&config_bytes(program_set_id)).to_bytes()).expect("config");
    fixture(Phase::Open, &one_general_entry(config_id, program_set_id))
}

#[test]
fn an_open_market_plans_an_exact_composite_general_root() {
    let fixture = open_fixture();
    let plan = plan_general_capability_activation_v3(&fixture.state).expect("activation plan");

    assert_eq!(plan.disposition, GeneralActivationDispositionV2::Create);
    assert_eq!(plan.entry_index, 1);
    assert_eq!(plan.root, fixture.state.capability_root.key);
    assert_eq!(plan.composite_root.len(), GENERAL_COMPOSITE_ROOT_BYTES_V3);
    assert_eq!(plan.composite_root.len(), 360);

    // The account decodes as exactly what the hot path authenticates: the
    // immutable header at the front, a live General tail behind it.
    let header = CapabilityRootHeaderV1::decode(
        plan.composite_root
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .expect("header"),
    )
    .expect("header decodes");
    assert_eq!(header, plan.root_header);
    assert_eq!(header.selection(), plan.selection);
    assert_eq!(header.generation(), GENERATION);
    let tail = GeneralRootV2::decode(
        plan.composite_root
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .expect("tail"),
    )
    .expect("tail decodes");
    assert_eq!(tail, plan.root_state);
    assert_eq!(tail.lifecycle(), GeneralLifecycleV2::Active);
    assert_eq!(tail.market(), fixture.state.market.key.to_bytes());
    assert_eq!(tail.config_id(), fixture.config_id.to_bytes());
    assert_eq!(tail.revision(), 1);

    // The selection is bound to the authenticated manifest entry, never to a
    // caller parameter: kind and capability release come from the entry.
    assert_eq!(plan.selection.manifest(), fixture.manifest_id);
    assert_eq!(
        plan.selection.kind().to_bytes(),
        GENERAL_CAPABILITY_KIND_ID_V1
    );
    assert_eq!(
        plan.selection.capability_release().to_bytes(),
        hash(&fixture.program_set).to_bytes()
    );

    assert_eq!(plan.funding_after.status(), FundingStatus::Active);
    assert_eq!(plan.funding_after.activation_slot(), SLOT);
    assert_eq!(plan.funding_after.remaining().rent().amount(), 0);
}

/// A minimal decodable `CapabilityProgramV1` carrying one root-state width.
///
/// This is the descriptor generation `outer.rs::process_activation` and
/// `initialize_root_account_v1` speak; it exists here only as the reference
/// composer, never as a General artifact.
fn v1_descriptor_with_root_width(root_state_bytes: usize) -> Vec<u8> {
    let mut transition = vec![0_u8; 40];
    transition
        .get_mut(..4)
        .expect("transition magic")
        .copy_from_slice(b"DCTV");
    *transition.get_mut(4).expect("transition version") = 2;
    for (offset, value) in [(6_usize, 1_u16), (8, 8), (10, 12)] {
        transition
            .get_mut(offset..offset + 2)
            .expect("transition header")
            .copy_from_slice(&value.to_le_bytes());
    }

    let mut descriptor = vec![0_u8; CAPABILITY_PROGRAM_HEADER_BYTES_V1 + transition.len()];
    descriptor
        .get_mut(..CAPABILITY_PROGRAM_MAGIC_V1.len())
        .expect("magic")
        .copy_from_slice(&CAPABILITY_PROGRAM_MAGIC_V1);
    descriptor
        .get_mut(8..10)
        .expect("version")
        .copy_from_slice(&1_u16.to_le_bytes());
    descriptor
        .get_mut(CAPABILITY_PROGRAM_PROFILE_OFFSET..CAPABILITY_PROGRAM_PROFILE_OFFSET + 2)
        .expect("profile")
        .copy_from_slice(&CAPABILITY_PROGRAM_PROFILE_V2.to_le_bytes());
    for (offset, seed) in [
        (CAPABILITY_PROGRAM_KIND_OFFSET, 0x11_u8),
        (CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET, 0x12),
        (CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET, 0x13),
        (CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET, 0x14),
        (CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET, 0x15),
        (CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET, 0x16),
        (CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET, 0x17),
        (CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET, 0x18),
    ] {
        descriptor
            .get_mut(offset..offset + 32)
            .expect("content field")
            .copy_from_slice(&[seed; 32]);
    }
    descriptor
        .get_mut(
            CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET
                ..CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET + 4,
        )
        .expect("root state bytes")
        .copy_from_slice(
            &u32::try_from(root_state_bytes)
                .expect("root width")
                .to_le_bytes(),
        );
    descriptor
        .get_mut(CAPABILITY_PROGRAM_HEADER_BYTES_V1..)
        .expect("transition body")
        .copy_from_slice(&transition);
    descriptor
}

#[test]
fn composition_is_byte_identical_to_initialize_root_account_v1() {
    let fixture = open_fixture();
    let plan = plan_general_capability_activation_v3(&fixture.state).expect("plan");

    let descriptor = v1_descriptor_with_root_width(GENERAL_ROOT_BYTES_V2);
    let program = CapabilityProgramV1::decode(&descriptor).expect("V1 descriptor");

    let mut reference = vec![0_u8; GENERAL_COMPOSITE_ROOT_BYTES_V3];
    initialize_root_account_v1(
        &mut reference,
        plan.root_header,
        program,
        &plan.root_state.to_bytes(),
    )
    .expect("reference composition");
    assert_eq!(reference, plan.composite_root);
}

/// Why the activation seam had to learn the release generation from an address.
///
/// A General V3 selection names a `CapabilityProgramSetV2` at
/// `capability_release`, and those bytes are not a `CapabilityProgramV1` under
/// any decoder. So the seam could not tell the two generations apart by trying
/// to decode the record, and `bc5da76` made it read the generation off the raw
/// record's own PDA -- `[RAW_RECORD_PDA_SEED_V1, schema, digest]` -- which is a
/// fact about a finalized record rather than a kind branch. This pins the
/// premise that argument rests on.
#[test]
fn a_v3_capability_release_is_a_program_set_and_not_a_flat_descriptor() {
    let fixture = open_fixture();
    let plan = plan_general_capability_activation_v3(&fixture.state).expect("plan");
    assert_eq!(
        plan.selection.capability_release().to_bytes(),
        hash(&fixture.program_set).to_bytes()
    );
    assert!(CapabilityProgramV1::decode(&fixture.program_set).is_err());
}

/// Why the seam refuses an activation that projects nothing into the tail.
///
/// `outer.rs` used to write `vec![0; root_state_bytes]` as the family tail.
/// `GeneralRootV2` is refused at its magic, so that root was a bricked
/// capability with a perfectly well-formed header -- which is exactly why the
/// header alone can never be the admission argument, and why the seam's
/// all-zero-tail refusal (`outer.rs`, `TradingSbfError::Root`) is the right
/// rule rather than a convenience.
#[test]
fn an_all_zero_tail_is_not_a_general_root_behind_a_valid_header() {
    let zero_tail = vec![0_u8; GENERAL_ROOT_BYTES_V2];
    assert!(GeneralRootV2::decode(&zero_tail).is_err());

    let fixture = open_fixture();
    let plan = plan_general_capability_activation_v3(&fixture.state).expect("plan");
    let mut bricked = plan.composite_root.clone();
    bricked
        .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .expect("tail")
        .fill(0);
    assert_ne!(bricked, plan.composite_root);
    assert!(
        CapabilityRootHeaderV1::decode(
            bricked
                .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
                .expect("header")
        )
        .is_ok(),
        "the header is well formed; only the tail is unusable"
    );
}

#[test]
fn an_exact_prior_activation_replays_idempotently_and_a_moved_one_refuses() {
    let fixture = open_fixture();
    let plan = plan_general_capability_activation_v3(&fixture.state).expect("plan");

    let mut replay = fixture.state.clone();
    replay.capability_root = account(
        plan.root,
        TRADING_PROGRAM,
        ROOT_RENT,
        plan.composite_root.clone(),
    );
    replay.funding_state = account(
        fixture.state.funding_state.key,
        TRADING_PROGRAM,
        FUNDING_RENT,
        plan.funding_after.to_bytes().to_vec(),
    );
    let replayed = plan_general_capability_activation_v3(&replay).expect("idempotent replay");
    assert_eq!(
        replayed.disposition,
        GeneralActivationDispositionV2::Idempotent
    );
    assert_eq!(replayed.composite_root, plan.composite_root);

    // A root whose tail advanced is not the exact activation result.
    let mut advanced_state = plan.root_state;
    advanced_state
        .open_batch(
            advanced_state.revision(),
            advanced_state.next_batch_sequence(),
        )
        .expect("open one batch");
    let mut advanced = replay.clone();
    advanced.capability_root = account(
        plan.root,
        TRADING_PROGRAM,
        ROOT_RENT,
        compose_general_root_v3(plan.root_header, advanced_state),
    );
    assert!(plan_general_capability_activation_v3(&advanced).is_err());
}

#[test]
fn a_retired_or_retiring_root_still_decodes_and_keeps_its_immutable_header() {
    let fixture = open_fixture();
    let plan = plan_general_capability_activation_v3(&fixture.state).expect("plan");
    for terminal in [GeneralLifecycleV2::Retiring, GeneralLifecycleV2::Retired] {
        let zombie = retire_planned_general_root_v3(&plan, terminal).expect("zombie root");
        assert_eq!(zombie.root_state.lifecycle(), terminal);
        assert_eq!(zombie.composite_root.len(), GENERAL_COMPOSITE_ROOT_BYTES_V3);
        // Byte-identical immutable header: this is exactly why the header alone
        // cannot decide whether the capability still accepts work.
        assert_eq!(
            zombie.composite_root.get(..CAPABILITY_ROOT_HEADER_BYTES_V1),
            plan.composite_root.get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
        );
        assert_ne!(zombie.composite_root, plan.composite_root);
        let tail = GeneralRootV2::decode(
            zombie
                .composite_root
                .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
                .expect("tail"),
        )
        .expect("zombie tail decodes");
        assert_eq!(tail.lifecycle(), terminal);
    }
}

#[test]
fn substituted_coordinates_refuse_without_producing_a_plan() {
    let base = open_fixture();
    assert!(plan_general_capability_activation_v3(&base.state).is_ok());

    let mut moved_root = base.state.clone();
    moved_root.capability_root = account(
        Pubkey::new_from_array([0xb1; 32]),
        system_program::ID,
        0,
        Vec::new(),
    );
    assert_eq!(
        plan_general_capability_activation_v3(&moved_root),
        Err(GeneralActivationErrorV3::Root)
    );

    let mut moved_funding = base.state.clone();
    moved_funding.funding_state.key = Pubkey::new_from_array([0xb2; 32]);
    assert_eq!(
        plan_general_capability_activation_v3(&moved_funding),
        Err(GeneralActivationErrorV3::Funding)
    );

    let mut substituted_manifest = base.state.clone();
    if let Some(byte) = substituted_manifest.manifest_record.data.last_mut() {
        *byte ^= 1;
    }
    assert_eq!(
        plan_general_capability_activation_v3(&substituted_manifest),
        Err(GeneralActivationErrorV3::Manifest)
    );

    let mut substituted_config = base.state.clone();
    if let Some(byte) = substituted_config.config_record.data.last_mut() {
        *byte ^= 1;
    }
    assert!(plan_general_capability_activation_v3(&substituted_config).is_err());

    let mut foreign_owner = base.state.clone();
    foreign_owner.funding_state.owner = Pubkey::new_from_array([0xb3; 32]);
    assert_eq!(
        plan_general_capability_activation_v3(&foreign_owner),
        Err(GeneralActivationErrorV3::Funding)
    );

    let mut stale = base.state.clone();
    stale.capability_root.observation.slot = SLOT - 1;
    assert_eq!(
        plan_general_capability_activation_v3(&stale),
        Err(GeneralActivationErrorV3::Snapshot)
    );

    let mut unfinalized = base.state.clone();
    unfinalized.market.observation.finality = Finality::Confirmed;
    assert_eq!(
        plan_general_capability_activation_v3(&unfinalized),
        Err(GeneralActivationErrorV3::Snapshot)
    );
}

#[test]
fn an_absent_general_entry_refuses_and_a_duplicated_one_cannot_be_encoded() {
    let program_set = program_set();
    let program_set_id = hash(&program_set).to_bytes();
    let config_id = ContentId::new(hash(&config_bytes(program_set_id)).to_bytes()).expect("config");

    // A Market whose manifest selects no General capability at all.
    let absent = fixture(Phase::Open, &[other_entry(0x51), other_entry(0xf1)]);
    assert_eq!(
        plan_general_capability_activation_v3(&absent.state),
        Err(GeneralActivationErrorV3::Entry)
    );

    // Ambiguity is refused one layer earlier than the planner: the manifest
    // itself admits each kind exactly once, so two General entries are not an
    // encodable manifest. The planner's ambiguity arm is defence in depth over
    // a hostile-decoded record, not the only guard.
    let duplicated = [
        general_entry(config_id, program_set_id),
        general_entry(config_id, program_set_id),
    ];
    let mut bytes = vec![0_u8; MANIFEST_HEADER_BYTES + duplicated.len() * CAPABILITY_ENTRY_BYTES];
    assert!(CapabilityManifestV1::encode_into(&duplicated, &mut bytes).is_err());
}

#[test]
fn a_terminal_market_phase_refuses_a_prepaid_lazy_activation() {
    let program_set = program_set();
    let program_set_id = hash(&program_set).to_bytes();
    let config_id = ContentId::new(hash(&config_bytes(program_set_id)).to_bytes()).expect("config");
    let terminal = fixture(
        Phase::Terminal,
        &one_general_entry(config_id, program_set_id),
    );
    assert_eq!(
        plan_general_capability_activation_v3(&terminal.state),
        Err(GeneralActivationErrorV3::Phase)
    );
}
