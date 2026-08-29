//! Controls for the selectable General release.
//!
//! Fixed-index reads into the seven-action arrays are what these controls are
//! written in terms of, and the arrays are compile-time sized, so the indexing
//! lint is disabled here rather than every assertion being restated through
//! `get`.
#![allow(clippy::indexing_slicing)]

use super::*;

use dclutch_general_adapter_contract::artifacts_v3::authenticate_general_artifacts_v3;

/// Widths a release selects for the external accounts Profile13 names.
///
/// The same values the accelerator campaign ran against, so this fixture is a
/// release shape that has actually executed rather than a plausible one.
fn external_widths() -> GeneralExternalAccountWidthsV3 {
    GeneralExternalAccountWidthsV3 {
        linked_basis_prefix: 256,
        result_domain: 192,
        rent_sysvar: 17,
        core_market: 320,
        activation_cache: 160,
        upgradeable_program: 36,
        trading_programdata_prefix: 45,
        claims_programdata_prefix: 45,
        core_programdata_prefix: 45,
        realm_record: 112,
        rent_credit: 48,
    }
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

/// The headline: seven actions become one release the family's own verifier
/// accepts, named by a publication a Market can select.
#[test]
fn the_seven_actions_compile_into_one_release_the_family_verifier_accepts() {
    let release = general_selected_release_v1(input()).expect("General release");

    assert_eq!(release.bundles.len(), GENERAL_SELECTED_ACTION_COUNT_V1);
    for (bundle, action) in release.bundles.iter().zip(GENERAL_ACTIONS_V3) {
        assert_eq!(bundle.action, action);
        assert!(!bundle.descriptor.is_empty());
        assert!(!bundle.lifecycle_policy.is_empty());
    }

    // `general_selected_release_v1` already ran the seven-action join, so this
    // is the independent restatement of the claim rather than the claim itself:
    // each action also authenticates on its own against the published set.
    for action in GENERAL_ACTIONS_V3 {
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
    assert_eq!(usize::from(publication.action_count), GENERAL_ACTIONS_V3.len());
    let set = CapabilityProgramSetV2::decode(&release.program_set).expect("set");
    assert_eq!(usize::from(set.entry_count()), GENERAL_ACTIONS_V3.len());
    assert_eq!(
        general_selected_release_profile_v1(),
        GeneralReleaseProfileV1::SettlementOnly
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
    for action in GENERAL_ACTIONS_V3 {
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
    for action in GENERAL_ACTIONS_V3 {
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
    assert_ne!(other.publication.program_set_id, first.publication.program_set_id);

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
