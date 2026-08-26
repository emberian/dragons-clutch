use dclutch_product_payoff_v2_codec::runtime_v3::{
    BASIS_HEADER_BYTES_V3, BasisInputV3, BasisKindV3, compile_basis_v3,
};
use dclutch_rational_representation_v2_contract::RepresentationActionV2;
use dclutch_rational_representation_v2_contract::{
    AuthenticatedTokenBehaviorV2, TokenBehaviorRecordAdmissionV2, authenticate_token_behavior_v2,
};
use dclutch_rational_representation_v2_kernel::{
    DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V3,
    DescriptorAdmissionV2, RepresentationDescriptorV2,
};
use dclutch_token_svm::{
    TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, TokenBehaviorSelectionV2,
};
use solana_program::hash::hash;

use crate::{
    RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3, RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3,
    RationalOpenCapabilityProgramSetInputV3, RationalOpenCapabilityProgramSetV3,
    RationalOpenSelectedHotBundleInputV3, RationalOpenSelectedHotBundleV3,
    RationalOpenStructuredHotBundleInputV3, RationalOpenStructuredHotBundleV3,
    build_rational_open_capability_program_set_v3, build_rational_open_selected_hot_bundle_v3,
    build_rational_open_structured_hot_bundle_v3,
};

pub(crate) struct OpenArtifactFixtureV3 {
    pub(crate) token_behavior: AuthenticatedTokenBehaviorV2,
    pub(crate) denominate: RationalOpenSelectedHotBundleV3,
    pub(crate) reconstitute: RationalOpenSelectedHotBundleV3,
    pub(crate) issue: RationalOpenStructuredHotBundleV3,
    pub(crate) unwrap: RationalOpenStructuredHotBundleV3,
    pub(crate) set: RationalOpenCapabilityProgramSetV3,
}

pub(crate) fn open_artifact_fixture_v3(
    realm: [u8; 32],
    release_set: [u8; 32],
    outcome_count: u32,
) -> OpenArtifactFixtureV3 {
    let token_behavior = authenticated_token_behavior_v3(id(4), realm, release_set, outcome_count);
    let basis = basis(outcome_count);
    let mut selected_lengths = [0_u32; RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3 as usize];
    selected_lengths[4] = u32::try_from(basis.len()).expect("basis length");
    selected_lengths[29] = u32::try_from(basis.len()).expect("basis alias length");
    let mut structured_lengths = [0_u32; RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3 as usize];
    structured_lengths[4] = u32::try_from(basis.len()).expect("basis length");
    structured_lengths[29] = u32::try_from(basis.len()).expect("basis alias length");

    let selected = |action| {
        build_rational_open_selected_hot_bundle_v3(RationalOpenSelectedHotBundleInputV3 {
            action,
            logical_data_lengths: &selected_lengths,
            product_basis: &basis,
            kind: id(10),
            authenticated_token_behavior: token_behavior,
            root_schema: id(11),
            derivation_policy: id(12),
            capacity_profile: id(13),
            root_state_bytes: 8,
        })
        .expect("selected artifact")
    };
    let structured = |action| {
        build_rational_open_structured_hot_bundle_v3(RationalOpenStructuredHotBundleInputV3 {
            action,
            fixed_data_lengths: &structured_lengths,
            item_data_lengths: [64, 82, 165, 165],
            product_basis: &basis,
            kind: id(10),
            authenticated_token_behavior: token_behavior,
            root_schema: id(11),
            derivation_policy: id(12),
            capacity_profile: id(13),
            root_state_bytes: 8,
        })
        .expect("structured artifact")
    };
    let denominate = selected(RepresentationActionV2::Denominate);
    let reconstitute = selected(RepresentationActionV2::Reconstitute);
    let issue = structured(RepresentationActionV2::IssueStructured);
    let unwrap = structured(RepresentationActionV2::UnwrapStructured);
    let set =
        build_rational_open_capability_program_set_v3(RationalOpenCapabilityProgramSetInputV3 {
            authenticated_token_behavior: token_behavior,
            denominate: &denominate,
            reconstitute: &reconstitute,
            issue_structured: &issue,
            unwrap_structured: &unwrap,
        })
        .expect("capability set");
    OpenArtifactFixtureV3 {
        token_behavior,
        denominate,
        reconstitute,
        issue,
        unwrap,
        set,
    }
}

pub(crate) fn authenticated_token_behavior_v3(
    descriptor_id: [u8; 32],
    realm: [u8; 32],
    release_set: [u8; 32],
    outcome_count: u32,
) -> AuthenticatedTokenBehaviorV2 {
    let outcome_count = usize::try_from(outcome_count).expect("outcome count");
    let mut descriptor_bytes =
        vec![0_u8; DESCRIPTOR_HEADER_BYTES + outcome_count * DESCRIPTOR_COEFFICIENT_BYTES];
    put(&mut descriptor_bytes, 0, &DESCRIPTOR_MAGIC_V3);
    put(&mut descriptor_bytes, 8, &3_u16.to_le_bytes());
    put(&mut descriptor_bytes, 16, &id(20));
    put(&mut descriptor_bytes, 48, &id(21));
    put(&mut descriptor_bytes, 80, &id(22));
    put(&mut descriptor_bytes, 112, &id(23));
    put(&mut descriptor_bytes, 144, &release_set);
    put(&mut descriptor_bytes, 176, &id(24));
    put(&mut descriptor_bytes, 208, &TOKEN_2022_PROGRAM_ID);
    put(
        &mut descriptor_bytes,
        240,
        &u32::try_from(outcome_count)
            .expect("outcome count")
            .to_le_bytes(),
    );
    put(&mut descriptor_bytes, 248, &10_u64.to_le_bytes());
    put(
        &mut descriptor_bytes,
        DESCRIPTOR_HEADER_BYTES,
        &10_u64.to_le_bytes(),
    );
    let descriptor = RepresentationDescriptorV2::decode(
        Box::leak(descriptor_bytes.into_boxed_slice()),
        DescriptorAdmissionV2 {
            selected_descriptor_id: descriptor_id,
            finalized_descriptor_id: descriptor_id,
            recomputed_descriptor_digest: descriptor_id,
            finalized_descriptor_digest: descriptor_id,
            record_authenticated: true,
            derived_representation_authority: id(25),
            authority_derivation_authenticated: true,
        },
    )
    .expect("representation descriptor");
    let selection = TokenBehaviorSelectionV2::new(realm, release_set)
        .expect("selection")
        .to_bytes();
    let selection_id = hash(&selection).to_bytes();
    authenticate_token_behavior_v2(
        descriptor,
        realm,
        &selection,
        TokenBehaviorRecordAdmissionV2 {
            selected_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            finalized_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            selected_content_digest: selection_id,
            finalized_content_digest: selection_id,
            recomputed_content_digest: selection_id,
            record_authenticated: true,
            market_realm_authenticated: true,
        },
    )
    .expect("authenticated Token behavior")
}

fn basis(width: u32) -> [u8; BASIS_HEADER_BYTES_V3] {
    let mut output = [0_u8; BASIS_HEADER_BYTES_V3];
    compile_basis_v3(
        BasisInputV3 {
            kind: BasisKindV3::CategoricalQ1,
            product_id: id(1),
            result_domain_id: id(2),
            coordinate_domain_id: id(3),
            result_unit_id: id(4),
            evaluator_release_id: id(5),
            basis_width: width,
            payout_scale: 1,
            knot_denominator: 1,
            knots: &[],
            terms: &[],
            failure_payouts: &[],
        },
        &mut output,
    )
    .expect("basis");
    output
}

fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    output
        .get_mut(offset..offset + input.len())
        .expect("fixture offset")
        .copy_from_slice(input);
}

#[test]
fn four_action_set_retains_each_distinct_descriptor() {
    let artifacts = open_artifact_fixture_v3(id(15), id(16), 258);
    assert_ne!(
        artifacts.denominate.descriptor,
        artifacts.reconstitute.descriptor
    );
    assert_ne!(artifacts.issue.descriptor, artifacts.unwrap.descriptor);
}
