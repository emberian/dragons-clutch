//! Lean-owned V5 coordinates, pinned, and their agreement with the safe kernel.
//!
//! `lifecycle_v3` now *derives* its V5 constants from `generated_v5.rs`, so
//! asserting the two against each other would compare a name with itself. What
//! is left to check is the part derivation cannot give away: that Lean's
//! coordinates are still the numbers this protocol committed to, that the safe
//! encoder reproduces Lean's witness bytes, and that Lean's hostile corpus is
//! refused with the exact code each row earns.

#[allow(missing_docs)]
#[path = "../src/lifecycle_v3/generated_v5.rs"]
mod generated;

use dclutch_account_profile_contract::lifecycle_v3::{
    ACTION_PLAN_BYTES, CURRENT_RENT_QUOTE_BYTES_V5, Error, HEADER_BYTES, PROTECTED_OUTPUT_BYTES,
    RECIPE_BYTES, SEED_BYTES, StateLifecyclePolicyV5,
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
        action: None,
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
fn generated_constants_are_the_pinned_lean_coordinates() {
    assert_eq!(generated::STATE_LIFECYCLE_V5_HEADER_BYTES, 40);
    assert_eq!(generated::STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_BYTES, 16);
    assert_eq!(generated::STATE_LIFECYCLE_V5_MAX_CURRENT_RENT_QUOTES, 16);
    assert_eq!(generated::STATE_LIFECYCLE_V5_SCHEMA_VERSION, 3);
    assert_eq!(generated::STATE_LIFECYCLE_V5_ARTIFACT_PROFILE, 4);
    assert_eq!(
        generated::STATE_LIFECYCLE_V5_SCHEMA_RELEASE_PREIMAGE,
        b"dclutch/schema/state-lifecycle-policy-v5-current-rent-quotes-v1"
    );
    // The digest is proven to be this preimage's SHA-256 by
    // `lifecycle_v5_generator_fresh::lifecycle_v5_schema_id_is_the_preimage_sha256`.
    // Pinned here as well because a finalized-record identity changing is a
    // release event, not a refactor.
    assert_eq!(
        generated::STATE_LIFECYCLE_V5_SCHEMA_RELEASE_ID,
        [
            0x10, 0xfb, 0xed, 0x6c, 0x13, 0x26, 0x12, 0x7c, 0xf7, 0xe5, 0x47, 0x83, 0xb1, 0xa5,
            0x97, 0xd7, 0x7c, 0xa3, 0xe7, 0x6b, 0x53, 0xde, 0x97, 0xc0, 0x8f, 0x27, 0x3f, 0x5e,
            0x67, 0xe3, 0x98, 0x3b,
        ]
    );
    assert_eq!(generated::STATE_LIFECYCLE_V5_MAGIC, *b"DCLTDP03");
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
            generated::STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_ACTION_SCOPE_OFFSET,
            generated::STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_ACTION_OFFSET,
            generated::STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_RESERVED_OFFSET,
        ],
        // The action tag came out of the front of the old ten-byte reserved run,
        // which is why the quote is still 16 bytes wide and every artifact
        // written before it existed is byte-identical: those two fields were
        // already zeros. `Lean current_rent_quote_coordinates_are_canonical` is
        // the authority for this row.
        [0, 4, 6, 7, 11]
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

    // A bare `is_err()` here would accept whatever the artifact refuses first,
    // which for the truncated row is a length check reached before any quote is
    // read. Each row is named by the accusation it earns.
    let expected = [
        // Fifteen bytes: the artifact is one byte short of its declared shape.
        Error::InvalidLength,
        // Zero `exact_data_len`.
        Error::InvalidRentQuote,
        // Nonzero byte inside the five reserved bytes after the action.
        Error::NonCanonicalReserved,
        // Scope tag `2`, which this build does not understand.
        Error::InvalidRentQuote,
        // Unscoped quote carrying a nonzero action, a second encoding of
        // "every action".
        Error::InvalidRentQuote,
    ];
    assert_eq!(
        generated::STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_REFUSAL_CORPUS.len(),
        expected.len(),
        "every Lean corpus row must name its refusal"
    );
    for (row, expected) in generated::STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_REFUSAL_CORPUS
        .into_iter()
        .zip(expected)
    {
        let mut hostile = policy.get(..quote_offset).expect("quote prefix").to_vec();
        hostile.extend_from_slice(row);
        assert_eq!(
            StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &hostile).err(),
            Some(expected),
            "hostile row {row:02x?}"
        );
    }
}
