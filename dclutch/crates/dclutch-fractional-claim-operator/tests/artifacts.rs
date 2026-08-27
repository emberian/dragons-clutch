//! Canonical action-specialized generic artifact emission.

#![allow(clippy::panic, clippy::unwrap_used)]

use dclutch_fractional_claim_contract::{
    ArtifactAdmissionV1, FRACTIONAL_FAMILY_REQUEST_BYTES_V1, FractionalActionV1,
    FractionalArtifactAdmissionsV1, FractionalArtifactBytesV1, FractionalArtifactSelectionV1,
    FractionalChildProgramsV1, FractionalFamilyRequestInputV1, FractionalFamilyRequestV1,
    NO_TERMINAL_OUTCOME_V1, authenticate_fractional_artifact_bundle_v1,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_TERMS_HEADER_BYTES_V1, FRACTIONAL_TERMS_MAGIC_V1, FRACTIONAL_TERMS_MINT_BYTES_V1,
    SCHEMA_VERSION_V1,
};
use dclutch_fractional_claim_operator::{
    FractionalClaimsAccountRuleV1, build_fractional_finalized_artifact_bundle_v1,
};
use dclutch_request_profile_contract::RequestProfileV1;
use dclutch_token_svm::{TOKEN_2022_PROGRAM_ID, TokenBehaviorSelectionV2};
use sha2::{Digest, Sha256};

fn claims_frame() -> [FractionalClaimsAccountRuleV1; 3] {
    [
        FractionalClaimsAccountRuleV1 {
            signer: false,
            writable: false,
            executable: true,
            data_length: 36,
        },
        FractionalClaimsAccountRuleV1 {
            signer: false,
            writable: true,
            executable: false,
            data_length: 512,
        },
        FractionalClaimsAccountRuleV1 {
            signer: false,
            writable: false,
            executable: false,
            data_length: 144,
        },
    ]
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    output
        .get_mut(offset..offset + value.len())
        .expect("fixture destination")
        .copy_from_slice(value);
}

fn terms_bytes(token_behavior: [u8; 32]) -> Vec<u8> {
    const OUTCOMES: u32 = 3;
    let mut output = vec![
        0;
        FRACTIONAL_TERMS_HEADER_BYTES_V1
            + usize::try_from(OUTCOMES).unwrap() * FRACTIONAL_TERMS_MINT_BYTES_V1
    ];
    put(&mut output, 0, &FRACTIONAL_TERMS_MAGIC_V1);
    put(&mut output, 8, &SCHEMA_VERSION_V1.to_le_bytes());
    put(&mut output, 16, &[1; 32]);
    put(&mut output, 48, &[5; 32]);
    put(&mut output, 80, &[3; 32]);
    put(&mut output, 112, &TOKEN_2022_PROGRAM_ID);
    put(&mut output, 144, &token_behavior);
    put(&mut output, 176, &OUTCOMES.to_le_bytes());
    put(&mut output, 184, &10_u64.to_le_bytes());
    for outcome in 0..OUTCOMES {
        let offset = FRACTIONAL_TERMS_HEADER_BYTES_V1 + usize::try_from(outcome).unwrap() * 32;
        put(
            &mut output,
            offset,
            &[u8::try_from(outcome + 20).unwrap(); 32],
        );
    }
    output
}

#[test]
fn every_action_emits_a_distinct_self_consistent_finalized_bundle() {
    let mut descriptor_ids = Vec::new();
    for action in [
        FractionalActionV1::Wrap,
        FractionalActionV1::Transfer,
        FractionalActionV1::WholeUnwrap,
        FractionalActionV1::WinningRedeem,
        FractionalActionV1::LosingZeroBurn,
        FractionalActionV1::Terminalize,
        FractionalActionV1::ZeroSupplyRetire,
    ] {
        let bundle =
            build_fractional_finalized_artifact_bundle_v1(action, [44; 32], &claims_frame())
                .unwrap();
        let request_id: [u8; 32] = Sha256::digest(&bundle.request_profile).into();
        let request =
            RequestProfileV1::decode_selected(request_id, request_id, &bundle.request_profile)
                .unwrap();
        assert_eq!(
            request.request_bytes(0).unwrap(),
            FRACTIONAL_FAMILY_REQUEST_BYTES_V1
        );
        descriptor_ids.push(<[u8; 32]>::from(Sha256::digest(bundle.descriptor)));
    }
    descriptor_ids.sort_unstable();
    descriptor_ids.dedup();
    assert_eq!(descriptor_ids.len(), 7);
}

#[test]
fn empty_claims_frame_and_zero_physical_profile_refuse() {
    assert!(
        build_fractional_finalized_artifact_bundle_v1(FractionalActionV1::Wrap, [44; 32], &[],)
            .is_err()
    );
    assert!(
        build_fractional_finalized_artifact_bundle_v1(
            FractionalActionV1::Wrap,
            [0; 32],
            &claims_frame(),
        )
        .is_err()
    );
}

#[test]
fn complete_finalized_bundle_authenticates_and_substituted_effect_refuses() {
    let realm = [2; 32];
    let release = [3; 32];
    let token_behavior = TokenBehaviorSelectionV2::new(realm, release)
        .unwrap()
        .to_bytes();
    let token_behavior_id = digest(&token_behavior);
    let terms = terms_bytes(token_behavior_id);
    let terms_id = digest(&terms);
    let emitted = build_fractional_finalized_artifact_bundle_v1(
        FractionalActionV1::Wrap,
        [44; 32],
        &claims_frame(),
    )
    .unwrap();
    let request = FractionalFamilyRequestV1::new(
        FractionalActionV1::Wrap,
        FractionalFamilyRequestInputV1 {
            release_set: release,
            market: [1; 32],
            product_record: [4; 32],
            result_domain: [5; 32],
            terms: terms_id,
            token_behavior: token_behavior_id,
            owner: [8; 32],
            source_token_account: [0; 32],
            destination_token_account: [9; 32],
            terminal_digest: [0; 32],
            expected_revision: 7,
            quantity: 2,
            outcome: 0,
            terminal_outcome: NO_TERMINAL_OUTCOME_V1,
        },
    )
    .unwrap();
    let descriptor_id = digest(&emitted.descriptor);
    let admission = |bytes: &[u8]| ArtifactAdmissionV1 {
        finalized_digest: digest(bytes),
        record_authenticated: true,
    };
    let selection = FractionalArtifactSelectionV1 {
        descriptor_id,
        terms_id,
        market: [1; 32],
        market_realm: realm,
        product_record: [4; 32],
        result_domain: [5; 32],
        outcome_count: 3,
        release_set: release,
        children: FractionalChildProgramsV1 {
            claims: [6; 32],
            custody: [7; 32],
            token: TOKEN_2022_PROGRAM_ID,
            physical_profile: [44; 32],
            release_authenticated: true,
        },
        semantic_selection_authenticated: true,
    };
    let admissions = FractionalArtifactAdmissionsV1 {
        descriptor: admission(&emitted.descriptor),
        terms: admission(&terms),
        token_behavior: admission(&token_behavior),
        account_profile: admission(&emitted.account_profile),
        lifecycle: admission(&emitted.lifecycle),
        request: admission(&emitted.request_profile),
        strategy: admission(&emitted.strategy),
        transition: admission(&emitted.transition),
        effect: admission(&emitted.effect),
    };
    let authenticated = authenticate_fractional_artifact_bundle_v1(
        selection,
        admissions,
        FractionalArtifactBytesV1 {
            descriptor: &emitted.descriptor,
            terms: &terms,
            token_behavior: &token_behavior,
            account_profile: &emitted.account_profile,
            lifecycle: &emitted.lifecycle,
            request: &emitted.request_profile,
            strategy: &emitted.strategy,
            transition: &emitted.transition,
            effect: &emitted.effect,
        },
        &request.to_bytes(),
    )
    .unwrap();
    assert_eq!(authenticated.family_request, request);

    let mut substituted_effect = emitted.effect.clone();
    *substituted_effect.last_mut().unwrap() ^= 1;
    assert!(
        authenticate_fractional_artifact_bundle_v1(
            selection,
            admissions,
            FractionalArtifactBytesV1 {
                descriptor: &emitted.descriptor,
                terms: &terms,
                token_behavior: &token_behavior,
                account_profile: &emitted.account_profile,
                lifecycle: &emitted.lifecycle,
                request: &emitted.request_profile,
                strategy: &emitted.strategy,
                transition: &emitted.transition,
                effect: &substituted_effect,
            },
            &request.to_bytes(),
        )
        .is_err()
    );
}
