//! Translation agreement between Lean-owned V5 coordinates and the safe Rust kernel.

#[allow(missing_docs)]
#[path = "../src/lifecycle_v3/generated_v5.rs"]
mod generated;

use dclutch_account_profile_contract::lifecycle_v3::{
    ACTION_PLAN_BYTES, CURRENT_RENT_QUOTE_ARTIFACT_PROFILE_V5, CURRENT_RENT_QUOTE_BYTES_V5,
    CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5, CURRENT_RENT_QUOTE_SCHEMA_RELEASE_PREIMAGE_V5,
    HEADER_BYTES, MAGIC, MAX_CURRENT_RENT_QUOTES_V5, PROTECTED_OUTPUT_BYTES, RECIPE_BYTES,
    SEED_BYTES, StateLifecyclePolicyV5, VERSION,
    encode::{
        LifecycleAccountCoordinateV3, LifecycleCurrentRentQuoteInputV5, LifecycleGuardInputV3,
        LifecycleOperationInputV3, LifecyclePlanInputV3, LifecycleRecipeInputV3,
        LifecycleSeedInputV3, encode_lifecycle_policy_v5_atomic,
    },
};

const POLICY_ID: [u8; 32] = [0x71; 32];

fn policy_with_quote() -> Vec<u8> {
    let recipes = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(0),
        seed_start: 0,
        seed_count: 2,
        bump_offset: 1,
        data_base: 8,
        data_stride: 0,
    }];
    let seeds = [
        LifecycleSeedInputV3::Literal(b"lean-v5-abi"),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = [LifecyclePlanInputV3 {
        action: 1,
        operation: LifecycleOperationInputV3::Authenticate,
        recipe: 0,
        payer: None,
        rent_credit: None,
        principal: None,
        beneficiary: None,
        guard: LifecycleGuardInputV3::Always,
    }];
    let quotes = [LifecycleCurrentRentQuoteInputV5 {
        exact_data_len: 512,
        scalar_destination: 39,
    }];
    let width = HEADER_BYTES
        + RECIPE_BYTES
        + 2 * SEED_BYTES
        + ACTION_PLAN_BYTES
        + PROTECTED_OUTPUT_BYTES
        + CURRENT_RENT_QUOTE_BYTES_V5;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_lifecycle_policy_v5_atomic(
        &recipes,
        &seeds,
        &plans,
        &[None],
        &[],
        &quotes,
        &mut scratch,
        &mut output,
    )
    .expect("safe Rust V5 encode");
    output
}

#[test]
fn generated_constants_equal_the_live_safe_kernel() {
    assert_eq!(generated::STATE_LIFECYCLE_V5_HEADER_BYTES, HEADER_BYTES);
    assert_eq!(
        generated::STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_BYTES,
        CURRENT_RENT_QUOTE_BYTES_V5
    );
    assert_eq!(
        generated::STATE_LIFECYCLE_V5_MAX_CURRENT_RENT_QUOTES,
        MAX_CURRENT_RENT_QUOTES_V5
    );
    assert_eq!(generated::STATE_LIFECYCLE_V5_SCHEMA_VERSION, VERSION);
    assert_eq!(
        generated::STATE_LIFECYCLE_V5_ARTIFACT_PROFILE,
        CURRENT_RENT_QUOTE_ARTIFACT_PROFILE_V5
    );
    assert_eq!(
        generated::STATE_LIFECYCLE_V5_SCHEMA_RELEASE_PREIMAGE,
        CURRENT_RENT_QUOTE_SCHEMA_RELEASE_PREIMAGE_V5
    );
    assert_eq!(
        generated::STATE_LIFECYCLE_V5_SCHEMA_RELEASE_ID,
        CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5
    );
    assert_eq!(generated::STATE_LIFECYCLE_V5_MAGIC, MAGIC);
    assert_eq!(
        [
            generated::STATE_LIFECYCLE_V5_MAGIC_OFFSET,
            generated::STATE_LIFECYCLE_V5_SCHEMA_VERSION_OFFSET,
            generated::STATE_LIFECYCLE_V5_ARTIFACT_PROFILE_OFFSET,
            generated::STATE_LIFECYCLE_V5_RECIPE_COUNT_OFFSET,
            generated::STATE_LIFECYCLE_V5_SEED_COUNT_OFFSET,
            generated::STATE_LIFECYCLE_V5_PLAN_COUNT_OFFSET,
            generated::STATE_LIFECYCLE_V5_PROTECTED_OUTPUT_COUNT_OFFSET,
            generated::STATE_LIFECYCLE_V5_IMMUTABLE_IDENTITY_BINDING_COUNT_OFFSET,
            generated::STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_COUNT_OFFSET,
            generated::STATE_LIFECYCLE_V5_RESERVED_OFFSET,
        ],
        [0, 8, 10, 12, 14, 16, 18, 20, 22, 24]
    );
    assert_eq!(
        [
            generated::STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_EXACT_DATA_LEN_OFFSET,
            generated::STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_SCALAR_DESTINATION_OFFSET,
            generated::STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_RESERVED_OFFSET,
        ],
        [0, 4, 6]
    );
}

#[test]
fn generated_empty_and_quote_bytes_round_trip_through_rust() {
    let mut scratch = [0_u8; HEADER_BYTES];
    let mut empty = [0_u8; HEADER_BYTES];
    encode_lifecycle_policy_v5_atomic(&[], &[], &[], &[], &[], &[], &mut scratch, &mut empty)
        .expect("safe Rust empty V5 encode");
    assert_eq!(empty, generated::STATE_LIFECYCLE_V5_CANONICAL_EMPTY_HEADER);
    assert!(
        StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &empty)
            .expect("safe Rust empty V5 decode")
            .is_empty()
    );

    let policy = policy_with_quote();
    let quote_offset = policy.len() - CURRENT_RENT_QUOTE_BYTES_V5;
    assert_eq!(
        policy.get(quote_offset..),
        Some(generated::STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_AGREEMENT.as_slice())
    );
    let decoded = StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &policy)
        .expect("safe Rust generated-coordinate decode");
    assert_eq!(
        decoded
            .current_rent_quote(0)
            .map(|quote| (quote.exact_data_len(), quote.scalar_destination().index())),
        Ok((512, 39))
    );

    for hostile_quote in generated::STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_REFUSAL_CORPUS {
        let mut hostile = policy.get(..quote_offset).expect("quote prefix").to_vec();
        hostile.extend_from_slice(hostile_quote);
        assert!(StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &hostile).is_err());
    }
}
