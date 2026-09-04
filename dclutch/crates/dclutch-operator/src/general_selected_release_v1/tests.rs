//! Controls for the selectable General release.
//!
//! Fixed-index reads into the complete action arrays are what these controls are
//! written in terms of, and the arrays are compile-time sized, so the indexing
//! lint is disabled here rather than every assertion being restated through
//! `get`.
#![allow(clippy::indexing_slicing)]

use super::*;

use dclutch_general_adapter_contract::activation_bundle_v1::build_general_activation_capable_program_set_v1;
use dclutch_general_adapter_contract::artifacts_v3::authenticate_general_artifacts_v3;
use dclutch_general_adapter_contract::release_v3::{
    GENERAL_ACTIONS_V3, GENERAL_ACTIONS_V5, authenticate_general_program_set_v3,
};

/// Widths a release selects for the external accounts Profile13 names.
///
/// THE PRODUCER'S, NOT A FIXTURE'S. This function used to spell eleven
/// literals under a comment claiming they were "the same values the accelerator
/// campaign ran against"; three of them were not, and the whole block was
/// transcribed twice into the successor's General compilers, where it published
/// `Exact(48)` for a RentCredit the protocol only makes at 128. A test fixture
/// that spells a width its producer derives is a second author for it, and this
/// is what a second author costs.
///
/// The two Product-derived widths are the four-outcome graded basis record and
/// the two-cut result-domain record this fixture's Product graph would compile.
fn external_widths() -> GeneralExternalAccountWidthsV3 {
    general_external_account_widths_v3(256, 192)
}

/// The eleven widths, named, so a contract that moves one goes red HERE.
///
/// Deriving from the contracts is what stops a transcription; asserting the
/// values is what stops a silent re-founding. The three that were wrong in
/// cohort-14 are the first three named.
#[test]
fn the_published_external_widths_are_the_protocol_constants() {
    let widths = general_external_account_widths_v3(256, 192);
    assert_eq!(widths.rent_credit, 128, "LIFECYCLE_RENT_CREDIT_BYTES_V2");
    assert_eq!(
        widths.activation_cache, 1_288,
        "ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1"
    );
    assert_eq!(widths.core_market, 368, "market-core STATE_BYTES");
    assert_eq!(widths.realm_record, 112, "REALM_BYTES");
    assert_eq!(widths.rent_sysvar, 17);
    assert_eq!(widths.upgradeable_program, 36, "LOADER_V3_PROGRAM_BYTES");
    for prefix in [
        widths.trading_programdata_prefix,
        widths.claims_programdata_prefix,
        widths.core_programdata_prefix,
    ] {
        assert_eq!(prefix, 45, "LOADER_V3_PROGRAMDATA_METADATA_BYTES");
    }
    assert_eq!(
        (widths.linked_basis_prefix, widths.result_domain),
        (256, 192)
    );
    // The three cohort-14 published, kept here as the counterexample rather
    // than as a comment: none of them is what the protocol produces.
    assert_ne!(widths.rent_credit, 48);
    assert_ne!(widths.activation_cache, 160);
    assert_ne!(widths.core_market, 320);
}

fn deployment() -> GeneralDeploymentFactsV1 {
    GeneralDeploymentFactsV1 {
        accelerator_artifact_release: [0x51; 32],
        compiler_release: [0x52; 32],
        toolchain: [0x53; 32],
        translation_validation: [0x54; 32],
    }
}

fn input() -> GeneralSelectedReleaseInputV1 {
    GeneralSelectedReleaseInputV1 {
        capacity_profile: [0x41; 32],
        claim_basis: [0x42; 32],
        selection_policy: [0x43; 32],
        quote_surplus_beneficiary: [0x44; 32],
        generation: 9,
        price_scale: 1_000_000,
        windows: GeneralConfigWindowsV1 {
            collection_slots: 16,
            selection_slots: 16,
            settlement_slots: 64,
            max_orders_per_candidate: 32,
            max_pages_per_candidate: 32,
            continuation_reward_lamports: 1,
        },
        outcome_count: 4,
        external_widths: external_widths(),
        token_account_bytes: 165,
        deployment: deployment(),
    }
}

/// The headline: all fifteen actions become one release the family's own verifier
/// accepts, named by a publication a Market can select.
#[test]
fn every_current_action_compiles_into_one_release_the_family_verifier_accepts() {
    for action in GENERAL_ACTIONS_V5 {
        encode_account_profile(input().external_widths, action)
            .unwrap_or_else(|error| panic!("{action:?} account profile refused: {error:?}"));
        encode_lifecycle(input(), action)
            .unwrap_or_else(|error| panic!("{action:?} lifecycle refused: {error:?}"));
        encode_transition(action)
            .unwrap_or_else(|error| panic!("{action:?} transition refused: {error:?}"));
        encode_effect(action)
            .unwrap_or_else(|error| panic!("{action:?} effect refused: {error:?}"));
        canonical_request(action)
            .unwrap_or_else(|error| panic!("{action:?} request refused: {error:?}"));
        compile_bundle(input(), action)
            .unwrap_or_else(|error| panic!("{action:?} bundle refused: {error:?}"));
    }
    let release = general_selected_release_v1(input()).expect("General release");

    assert_eq!(release.bundles.len(), GENERAL_SELECTED_ACTION_COUNT_V1);
    for (bundle, action) in release.bundles.iter().zip(GENERAL_ACTIONS_V5) {
        assert_eq!(bundle.action, action);
        assert!(!bundle.descriptor.is_empty());
        assert!(!bundle.lifecycle_policy.is_empty());
    }

    // `general_selected_release_v1` already ran the complete join, so this
    // is the independent restatement of the claim rather than the claim itself:
    // each action also authenticates on its own against the published set.
    for action in GENERAL_ACTIONS_V5 {
        let index = usize::from(action as u8);
        let request = canonical_request(action).expect("canonical request");
        authenticate_general_artifacts_v3(
            release.selection(),
            bundle_bytes(&release, index).expect("bundle bytes"),
            &request,
            input().outcome_count,
        )
        .expect("action authenticates against the published release");
    }
}

/// The selected release has one catalogue: V5, with the historical seven as an
/// exact prefix rather than a second independently authored order.
#[test]
fn the_current_catalogue_is_dense_exhaustive_and_preserves_the_historical_prefix() {
    assert_eq!(
        GENERAL_ACTIONS_V5
            .get(..GENERAL_ACTIONS_V3.len())
            .expect("V3 prefix"),
        GENERAL_ACTIONS_V3
    );
    for (tag, action) in GENERAL_ACTIONS_V5.into_iter().enumerate() {
        assert_eq!(usize::from(action as u8), tag, "catalogue tag {tag} moved");
    }

    let release = general_selected_release_v1(input()).expect("current release");
    assert_eq!(
        release
            .bundles
            .iter()
            .map(|bundle| bundle.action)
            .collect::<Vec<_>>(),
        GENERAL_ACTIONS_V5
    );
    let current_identity = digest(&release.program_set);
    let (_, current_profile) = authenticate_general_program_set_v3(
        current_identity,
        current_identity,
        &release.program_set,
    )
    .expect("current set");
    assert_eq!(
        current_profile,
        GeneralReleaseProfileV1::CompleteV2WithActivation
    );

    let historical = build_general_activation_capable_program_set_v1(
        release
            .publication
            .descriptors
            .get(..GENERAL_ACTIONS_V3.len())
            .expect("historical descriptor prefix"),
        release.activation.descriptor_id,
    )
    .expect("historical set remains constructible");
    let historical_identity = digest(&historical);
    let (_, historical_profile) =
        authenticate_general_program_set_v3(historical_identity, historical_identity, &historical)
            .expect("historical set remains authenticated");
    assert_eq!(
        historical_profile,
        GeneralReleaseProfileV1::SettlementWithActivation
    );
}

/// Every publication field is derived; none is a free parameter.
#[test]
fn every_published_fact_is_read_back_off_the_release_it_describes() {
    let release = general_selected_release_v1(input()).expect("General release");
    let publication = release.publication;

    assert_eq!(publication.program_set_id, digest(&release.program_set));
    assert_eq!(publication.config_id, digest(&release.config));
    assert_eq!(publication.kind_id, GENERAL_CAPABILITY_KIND_ID_V1);
    for (index, descriptor) in publication.descriptors.into_iter().enumerate() {
        assert_eq!(descriptor, digest(&release.bundles[index].descriptor));
    }

    // The named deployment facts arrive unchanged -- they are the only inputs a
    // derivation cannot produce, which is exactly why they are required fields.
    assert_eq!(publication.accelerator_artifact_release, [0x51; 32]);
    assert_eq!(publication.compiler_release, [0x52; 32]);
    assert_eq!(publication.toolchain, [0x53; 32]);
    assert_eq!(publication.translation_validation, [0x54; 32]);

    let bytes = publication.to_bytes();
    assert_eq!(bytes.len(), GENERAL_SELECTED_PUBLICATION_BYTES_V1);
    assert_eq!(&bytes[..8], &GENERAL_SELECTED_PUBLICATION_MAGIC_V1);
    assert_eq!(publication.publication_id(), digest(&bytes));
}

/// The six facts `general_market_selection_requirements_v1` names, checked
/// against the publication rather than against prose.
#[test]
fn the_publication_satisfies_the_market_selection_hook() {
    let release = general_selected_release_v1(input()).expect("General release");
    let publication = release.publication;

    // (2) the capability release is a ProgramSetV2 whose selector offset is 10.
    assert_eq!(publication.selector_offset, 10);
    assert_eq!(
        publication.selector_offset,
        GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3
    );
    assert_eq!(publication.selector_width, 1);

    // (3) one entry per authored action.
    assert_eq!(
        usize::from(publication.action_count),
        GENERAL_ACTIONS_V5.len()
    );
    let set = CapabilityProgramSetV2::decode(&release.program_set).expect("set");
    assert_eq!(
        usize::from(set.entry_count()),
        GENERAL_ACTIONS_V5.len() + 1,
        "fifteen actions and the activation coordinate"
    );
    assert_eq!(
        general_selected_release_profile_v1(),
        GeneralReleaseProfileV1::CompleteV2WithActivation
    );
    assert_eq!(
        usize::from(set.entry_count()),
        GENERAL_SELECTED_ENTRY_COUNT_V1
    );

    // (4) the executor role is Trading.
    assert_eq!(publication.executor_role, ExecutionRoleV1::Trading as u8);

    // (1) the manifest entry's three identities are all published.
    assert_ne!(publication.kind_id, [0; 32]);
    assert_ne!(publication.program_set_id, [0; 32]);
    assert_ne!(publication.config_id, [0; 32]);

    // (5) and (6) -- the V4 effect envelope and the AdmittedAot disposition --
    // are enforced by `authenticate_general_release_v3`, which the compiler ran
    // before returning. A bundle failing either could not have been published.
    for action in GENERAL_ACTIONS_V5 {
        let index = usize::from(action as u8);
        let strategy = ExecutionStrategyProgramV2::decode(&release.bundles[index].strategy)
            .expect("strategy decodes");
        assert_eq!(strategy.disposition(), StrategyDispositionV2::AdmittedAot);
    }
}

/// Every action routes to its own descriptor, and no two share one.
#[test]
fn each_action_selects_its_own_descriptor_and_none_is_shared() {
    let release = general_selected_release_v1(input()).expect("General release");
    let set = CapabilityProgramSetV2::decode(&release.program_set).expect("set");
    for action in GENERAL_ACTIONS_V5 {
        let probe = action_selector_probe(action).expect("probe");
        let selected = set.select_descriptor(&probe).expect("selected descriptor");
        let index = usize::from(action as u8);
        assert_eq!(
            selected.program().to_bytes(),
            release.publication.descriptors[index]
        );
    }
    for (index, left) in release.publication.descriptors.into_iter().enumerate() {
        for (other, right) in release.publication.descriptors.into_iter().enumerate() {
            if index != other {
                assert_ne!(left, right, "two actions must not share one descriptor");
            }
        }
    }
}

/// Compilation is a function of its input: same input, same bytes.
#[test]
fn the_release_is_reproducible_and_moves_with_every_named_fact() {
    let first = general_selected_release_v1(input()).expect("first");
    let second = general_selected_release_v1(input()).expect("second");
    assert_eq!(first, second);

    // A different deployment fact must move the release identity, or the
    // publication would name a build it does not describe.
    let mut moved = input();
    moved.deployment.toolchain = [0x99; 32];
    let other = general_selected_release_v1(moved).expect("moved");
    assert_ne!(
        other.publication.publication_id(),
        first.publication.publication_id()
    );
    assert_ne!(
        other.publication.program_set_id,
        first.publication.program_set_id
    );

    // So must a config coordinate, which travels through the config digest.
    let mut regenerated = input();
    regenerated.generation = 10;
    let third = general_selected_release_v1(regenerated).expect("generation");
    assert_ne!(third.publication.config_id, first.publication.config_id);
}

/// Hostiles: every one names the exact refusal it must produce.
#[test]
fn zero_identities_and_nonpositive_windows_refuse_before_compiling() {
    for mutate in [
        (|value: &mut GeneralSelectedReleaseInputV1| value.capacity_profile = [0; 32])
            as fn(&mut GeneralSelectedReleaseInputV1),
        |value| value.claim_basis = [0; 32],
        |value| value.selection_policy = [0; 32],
        |value| value.quote_surplus_beneficiary = [0; 32],
        |value| value.deployment.accelerator_artifact_release = [0; 32],
        |value| value.deployment.compiler_release = [0; 32],
        |value| value.deployment.toolchain = [0; 32],
        |value| value.deployment.translation_validation = [0; 32],
        |value| value.outcome_count = 0,
        |value| value.token_account_bytes = 0,
        |value| value.price_scale = 0,
        |value| value.generation = 0,
        |value| value.windows.collection_slots = 0,
        |value| value.windows.selection_slots = 0,
        |value| value.windows.settlement_slots = 0,
        |value| value.windows.max_orders_per_candidate = 0,
        |value| value.windows.max_pages_per_candidate = 0,
    ] {
        let mut hostile = input();
        mutate(&mut hostile);
        assert_eq!(
            general_selected_release_v1(hostile).err(),
            Some(GeneralSelectedReleaseErrorV1::Input),
            "an unnamed or nonpositive release coordinate must refuse"
        );
    }
}

/// A substituted bundle refuses, because validation rebuilds rather than inspects.
#[test]
fn a_substituted_bundle_program_set_or_config_refuses() {
    let canonical = general_selected_release_v1(input()).expect("release");

    // One action's bundle replaced by another real action's bundle: every
    // artifact is individually well formed, and the release is still refused.
    let mut swapped = canonical.clone();
    swapped.bundles[3] = canonical.bundles[4].clone();
    assert_eq!(
        validate_general_selected_release_v1(&swapped, input()).err(),
        Some(GeneralSelectedReleaseErrorV1::Release)
    );

    // The newly admitted half is not a looser suffix. A valid late-action
    // bundle substituted at CloseCandidate's coordinate is refused before its
    // individually well-formed artifacts can be treated as tag 14.
    let mut swapped_late = canonical.clone();
    swapped_late.bundles[14] = canonical.bundles[11].clone();
    assert_eq!(
        validate_general_selected_release_v1(&swapped_late, input()).err(),
        Some(GeneralSelectedReleaseErrorV1::Release)
    );

    // A bundle whose ACTION tag is right but whose lifecycle policy came from a
    // different action -- the substitution a wrong-seed release would make.
    let mut relifecycled = canonical.clone();
    relifecycled.bundles[0].lifecycle_policy = canonical.bundles[2].lifecycle_policy.clone();
    assert_eq!(
        validate_general_selected_release_v1(&relifecycled, input()).err(),
        Some(GeneralSelectedReleaseErrorV1::Release)
    );

    // ProgramSet bytes from a genuinely different release.
    let mut moved = input();
    moved.deployment.toolchain = [0x99; 32];
    let other = general_selected_release_v1(moved).expect("other");
    let mut foreign_set = canonical.clone();
    foreign_set.program_set = other.program_set.clone();
    assert_eq!(
        validate_general_selected_release_v1(&foreign_set, input()).err(),
        Some(GeneralSelectedReleaseErrorV1::ProgramSet)
    );

    let mut foreign_config = canonical.clone();
    foreign_config.config = other.config.clone();
    assert_eq!(
        validate_general_selected_release_v1(&foreign_config, input()).err(),
        Some(GeneralSelectedReleaseErrorV1::ProgramSet)
    );
}

/// A publication that names anything but the release it travels with refuses.
#[test]
fn a_publication_naming_another_release_refuses() {
    let canonical = general_selected_release_v1(input()).expect("release");
    let mut moved = input();
    moved.deployment.compiler_release = [0x9a; 32];
    let other = general_selected_release_v1(moved).expect("other");

    let mut swapped = canonical.clone();
    swapped.publication = other.publication;
    assert_eq!(
        validate_general_selected_release_v1(&swapped, input()).err(),
        Some(GeneralSelectedReleaseErrorV1::Publication)
    );

    // One descriptor identity moved: the publication still decodes, still has
    // the right shape, and still describes a release that does not exist.
    let mut relabelled = canonical.clone();
    relabelled.publication.descriptors[6] = [0x77; 32];
    assert_eq!(
        validate_general_selected_release_v1(&relabelled, input()).err(),
        Some(GeneralSelectedReleaseErrorV1::Publication)
    );

    // A publication claiming a different generation than the config pins.
    let mut regenerated = canonical.clone();
    regenerated.publication.generation = 11;
    assert_eq!(
        validate_general_selected_release_v1(&regenerated, input()).err(),
        Some(GeneralSelectedReleaseErrorV1::Publication)
    );
}

/// A release validated against input it was not compiled from refuses.
#[test]
fn a_release_rejoined_to_foreign_input_refuses() {
    let canonical = general_selected_release_v1(input()).expect("release");
    let mut foreign = input();
    foreign.outcome_count = 8;
    assert!(validate_general_selected_release_v1(&canonical, foreign).is_err());

    let mut rewidthed = input();
    rewidthed.token_account_bytes = 170;
    assert!(validate_general_selected_release_v1(&canonical, rewidthed).is_err());
}

/// The compiler names no seed, which is what keeps a wrong-seed release
/// inexpressible rather than merely unlikely.
#[test]
fn the_publication_width_is_derived_from_its_own_field_table() {
    assert_eq!(
        GENERAL_SELECTED_PUBLICATION_BYTES_V1,
        PUBLICATION_IDENTITY_START_V1
            + PUBLICATION_IDENTITY_COUNT_V1 * 32
            + PUBLICATION_SCALAR_BYTES_V1
    );
    assert_eq!(
        PUBLICATION_IDENTITY_COUNT_V1,
        PUBLICATION_FIXED_IDENTITY_COUNT_V1 + GENERAL_SELECTED_ACTION_COUNT_V1
    );
    let release = general_selected_release_v1(input()).expect("release");
    assert_eq!(
        release.publication.identities().len(),
        PUBLICATION_IDENTITY_COUNT_V1
    );
}

/// The publication chain gets every record, and every schema is DERIVED.
///
/// This is the shape SEL-SEAM's capability-neutral seam reads. The property
/// that matters is not that the list is complete but that no schema in it was
/// typed here: each is read off the artifact that names it, so a plan cannot
/// finalize a record under a schema the release does not actually select.
#[test]
fn the_publication_record_list_names_every_record_under_a_derived_schema() {
    let release = general_selected_release_v1(input()).expect("release");
    let records = release.publication_records().expect("records");

    // program-set + config, nine artifacts per action, then the activation
    // triple: the profile, the effect and the descriptor the seam authenticates.
    assert_eq!(records.len(), 2 + 9 * GENERAL_SELECTED_ACTION_COUNT_V1 + 3);
    assert_eq!(records[0].label, "program-set");
    assert_eq!(records[0].body, release.program_set.as_slice());
    assert_eq!(records[1].label, "config");
    assert_eq!(records[1].body, release.config.as_slice());

    // No record may be finalized under the zero schema, and the content id must
    // be the digest of the exact bytes the record carries.
    for record in &records {
        assert_ne!(record.schema, [0; 32], "{} has no schema", record.label);
        assert!(!record.body.is_empty(), "{} is empty", record.label);
        assert_eq!(record.content_id(), digest(record.body));
    }

    // The descriptor records are exactly the identities the publication names,
    // so the seam can join the manifest entry to a record in this list.
    let descriptors: Vec<[u8; 32]> = records
        .iter()
        .filter(|record| record.label == "descriptor")
        .map(GeneralPublicationRecordV1::content_id)
        .collect();
    assert_eq!(descriptors.as_slice(), release.publication.descriptors);

    // The config record's schema comes from the descriptor's own
    // `config_schema`, which is what the on-chain activation route authenticates
    // the config raw record under -- not a constant restated here.
    let first = CapabilityProgramV4::decode(&release.bundles[0].descriptor).expect("descriptor");
    assert_eq!(records[1].schema, first.config_schema().to_bytes());
    assert_eq!(
        first.config_schema().to_bytes(),
        GENERAL_CONFIG_SCHEMA_ID_V3
    );

    // And the lifecycle record travels under the V5 selected-lifecycle schema
    // the descriptor names, which is the artifact carrying the seed order.
    let lifecycle = records
        .iter()
        .find(|record| record.label == "lifecycle-policy")
        .expect("lifecycle record");
    assert_eq!(
        lifecycle.schema,
        first.artifacts().lifecycle.schema().to_bytes()
    );
}

/// The publication closure: the three records a Trading activation frame reads.
///
/// This is the whole of what "General's publication closure is not wired" meant.
/// The activation route borrows exactly three finalized records beyond the set
/// and the config -- `PROFILE_RAW`, `EFFECT_RAW` and `SET_DESCRIPTOR_RAW` -- and
/// authenticates each against an identity the DESCRIPTOR carries. So this does
/// not check that three records exist; it checks that the identities the
/// published descriptor names are the digests of the other two published
/// bodies, and that the set entry names the digest of the descriptor body. A
/// release whose three records do not close that triangle publishes a capability
/// no Market can activate, and the failure would be a live refusal rather than a
/// compile error.
#[test]
fn the_three_activation_records_close_the_triangle_the_seam_authenticates() {
    use dclutch_capability_activation_codec::{
        activation_account_profile_schema_v1, activation_effect_schema_v1,
    };
    use dclutch_capability_program_contract::CapabilityProgramV1;

    let release = general_selected_release_v1(input()).expect("release");
    let records = release.publication_records().expect("records");
    let named = |label: &str| {
        records
            .iter()
            .find(|record| record.label == label)
            .unwrap_or_else(|| panic!("{label} record"))
    };

    let profile = named("activation-account-profile");
    let effect = named("activation-effect");
    let descriptor = named("activation-descriptor");

    assert_eq!(profile.body, release.activation.account_profile.as_slice());
    assert_eq!(effect.body, release.activation.effect.as_slice());
    assert_eq!(descriptor.body, release.activation.descriptor.as_slice());

    // The three schemas are the codec's, which is the seam's own authority for
    // them: `process_activation` authenticates the profile under
    // `ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1`, the effect under the effect
    // kernel's V2 schema, and admits the selected descriptor only under
    // `CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1`.
    assert_eq!(profile.schema, activation_account_profile_schema_v1());
    assert_eq!(effect.schema, activation_effect_schema_v1());
    assert_eq!(descriptor.schema, general_activation_descriptor_schema_v1());
    assert_ne!(
        descriptor.schema, CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
        "the activation descriptor must NOT carry the schema every action carries"
    );

    // The triangle. `authenticate_finalized_record` is handed each body and the
    // identity the descriptor names; these are those identities.
    let decoded = CapabilityProgramV1::decode(descriptor.body).expect("activation descriptor");
    assert_eq!(decoded.account_profile().to_bytes(), profile.content_id());
    assert_eq!(decoded.effect_schema().to_bytes(), effect.content_id());
    assert_eq!(
        release.activation.descriptor_id,
        descriptor.content_id(),
        "the set entry names the digest of the published descriptor body"
    );
    assert_eq!(
        release.publication.activation_descriptor,
        descriptor.content_id()
    );

    // And the four manifest-joined coordinates are the ones all actions
    // publish, because `validate_selection` joins the descriptor to the manifest
    // entry the release's own action descriptors authored.
    let action = CapabilityProgramV4::decode(&release.bundles[0].descriptor).expect("action");
    assert_eq!(decoded.kind().to_bytes(), action.kind().to_bytes());
    assert_eq!(
        decoded.capacity_profile().to_bytes(),
        action.capacity_profile().to_bytes()
    );
    assert_eq!(
        decoded.root_schema().to_bytes(),
        action.root_schema().to_bytes()
    );
    assert_eq!(
        decoded.derivation_policy().to_bytes(),
        action.derivation_policy().to_bytes()
    );
    assert_eq!(decoded.root_state_bytes(), action.root_state_bytes());
}

/// The published set activates, and no action request can reach that entry.
#[test]
fn the_published_set_selects_the_activation_descriptor_and_no_action_does() {
    let release = general_selected_release_v1(input()).expect("release");
    let set = CapabilityProgramSetV2::decode(&release.program_set).expect("set");

    let request = general_activation_request_v1().expect("activation request");
    let selected = set.select_descriptor(&request).expect("activation entry");
    assert_eq!(
        selected.program().to_bytes(),
        release.activation.descriptor_id
    );
    assert_eq!(
        selected.schema().to_bytes(),
        general_activation_descriptor_schema_v1()
    );

    for action in GENERAL_ACTIONS_V5 {
        let probe = action_selector_probe(action).expect("probe");
        let action_selected = set.select_descriptor(&probe).expect("action entry");
        assert_ne!(
            action_selected.program().to_bytes(),
            release.activation.descriptor_id,
            "an ordinary controller request must not reach the activation entry"
        );
        assert_eq!(
            action_selected.schema().to_bytes(),
            CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID
        );
    }
}

/// What the published activation actually WRITES, read with General's decoder.
///
/// The review standard the template sets: a data-defined activation is judged by
/// running the real effect kernel over the artifacts a release publishes and
/// decoding the answer with the family's own type -- never by reading the
/// effect's instruction list. So this projects the PUBLISHED bundle for the
/// PUBLISHED config identity and requires `GeneralRootV2::decode` to accept it
/// and to equal `GeneralRootV2::active`.
#[test]
fn the_published_activation_composes_a_real_general_root() {
    use dclutch_general_adapter_contract::activation_bundle_v1::project_general_root_tail_v1;
    use dclutch_general_config_contract::GeneralRootV2;

    let release = general_selected_release_v1(input()).expect("release");
    let config_id = release.publication.config_id;
    let generation = release.publication.generation;
    for market in [[0x2a; 32], [0xd7; 32]] {
        let tail = project_general_root_tail_v1(
            &release.activation,
            market,
            config_id,
            generation,
            2_672_640,
        )
        .expect("projected General root tail");
        let root = GeneralRootV2::decode(&tail).expect("the projection decodes as a General root");
        assert_eq!(
            root,
            GeneralRootV2::active(market, config_id, generation).expect("canonical active root")
        );
    }
}

/// A substituted activation record refuses, because validation rebuilds it.
#[test]
fn a_substituted_activation_record_refuses() {
    let canonical = general_selected_release_v1(input()).expect("release");

    for mutate in [
        (|release: &mut GeneralSelectedReleaseV1| {
            release.activation.account_profile = release.bundles[0].account_profile.clone();
        }) as fn(&mut GeneralSelectedReleaseV1),
        |release| release.activation.effect = release.bundles[0].effect.clone(),
        |release| release.activation.descriptor = release.bundles[0].descriptor.clone(),
        |release| release.activation.descriptor_id = [0x5c; 32],
        |release| release.activation.account_profile_id = [0x5c; 32],
        |release| release.activation.effect_id = [0x5c; 32],
    ] {
        let mut hostile = canonical.clone();
        mutate(&mut hostile);
        assert_eq!(
            validate_general_selected_release_v1(&hostile, input()).err(),
            Some(GeneralSelectedReleaseErrorV1::Activation),
            "the activation triple is rebuilt, not inspected"
        );
    }

    // A publication naming another activation descriptor is a publication fault,
    // not an activation one: the bundle is the canonical one and only the
    // summary disagrees.
    let mut relabelled = canonical.clone();
    relabelled.publication.activation_descriptor = [0x77; 32];
    assert_eq!(
        validate_general_selected_release_v1(&relabelled, input()).err(),
        Some(GeneralSelectedReleaseErrorV1::Publication)
    );

    // And the historical seven-action-plus-activation set remains decodable,
    // but cannot masquerade as the current fifteen-action release.
    let mut narrowed = canonical.clone();
    narrowed.program_set = build_general_activation_capable_program_set_v1(
        canonical
            .publication
            .descriptors
            .get(..GENERAL_ACTIONS_V3.len())
            .expect("historical descriptor prefix"),
        canonical.activation.descriptor_id,
    )
    .expect("historical activation-capable set still encodes");
    assert_eq!(
        validate_general_selected_release_v1(&narrowed, input()).err(),
        Some(GeneralSelectedReleaseErrorV1::ProgramSet)
    );
}

/// FIFTEEN ACTIONS, FIFTEEN DERIVATION POLICIES, AND A MANIFEST ENTRY HOLDS ONE.
///
/// `CapabilityProgramV4::validate_selection` — the function
/// `reauthenticate_top_level_root_roles_v3` runs over the SELECTED action's
/// descriptor, and the one behind `TradingSbfError::DescriptorManifestEntry`
/// `0x4015` — requires `descriptor.derivation_policy() ==
/// entry.child_derivation_id()`. A Market's capability manifest carries ONE
/// entry per capability root, so it can hold exactly one such id.
///
/// `compile_bundle` sets each action descriptor's `derivation_policy` to
/// `digest(lifecycle_policy)`, and `encode_lifecycle` compiles the lifecycle
/// PER ACTION because the child rent widths are per action. The General
/// family's own `validate_descriptor` then pins `derivation_policy ==
/// lifecycle().program()`, so the per-action value is not incidental — it is
/// required. The two rules are jointly satisfiable for one action at a time.
///
/// Measured on devnet 2026-09-04: cohort-15's General market activated (the
/// activation descriptor carries the FIRST action's policy, and so does the
/// manifest entry the founding compiled from `bundles.first()`) and its
/// OpenBatch simulation refused `0x4015` after 128,724 CU, on this conjunct
/// alone. Re-founding cannot repair it; it can only choose which single action
/// the Market is able to run. The Direct family does not have this shape: its
/// non-ordinary bundles carry `ordinary.derivation_policy()`, so one entry binds
/// every Direct action.
#[test]
fn every_action_descriptor_carries_its_own_derivation_policy() {
    let release = general_selected_release_v1(input()).expect("release");
    let policies: Vec<[u8; 32]> = release
        .bundles
        .iter()
        .map(|bundle| {
            CapabilityProgramV4::decode(&bundle.descriptor)
                .expect("action descriptor")
                .derivation_policy()
                .to_bytes()
        })
        .collect();
    assert_eq!(policies.len(), GENERAL_SELECTED_ACTION_COUNT_V1);
    for (left, left_policy) in policies.iter().enumerate() {
        for (right, right_policy) in policies.iter().enumerate() {
            if left != right {
                assert_ne!(
                    left_policy, right_policy,
                    "actions {:?} and {:?} share a derivation policy",
                    release.bundles[left].action, release.bundles[right].action
                );
            }
        }
    }

    // The pair cohort-15 measured, by name rather than by index arithmetic.
    let policy_for = |wanted: Action| {
        release
            .bundles
            .iter()
            .position(|bundle| bundle.action == wanted)
            .map(|index| policies[index])
            .expect("the release carries this action")
    };
    assert_ne!(
        policy_for(GENERAL_ACTIONS_V5[0]),
        policy_for(Action::OpenBatch),
        "a manifest entry compiled from the first bundle cannot bind OpenBatch"
    );

    // That the activation descriptor carries the FIRST bundle's policy — which
    // is why the Market activated and its OpenBatch could not — is already
    // pinned by `the_three_activation_records_close_the_triangle_the_seam_authenticates`
    // above (`CapabilityProgramV1`, not V4 — the activation descriptor carries
    // its own schema); this test is the other half: no second action shares it.
}
