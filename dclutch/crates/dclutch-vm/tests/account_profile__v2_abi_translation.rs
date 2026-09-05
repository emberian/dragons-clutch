//! Lean-owned AccountProfile V2 coordinates, pinned against their literals.
//!
//! `v2` derives all of its named constants from `generated_abi.rs`, so
//! asserting the two against each other would compare a name with itself. What
//! derivation cannot give away is whether Lean still says the numbers this wire
//! committed to, so every emitted constant is pinned here. A renumbered opcode,
//! a moved field, a changed header cut point or a flipped admissibility cell
//! reds this file.

#[allow(dead_code, missing_docs)]
#[path = "../src/account_profile/v2/generated_abi.rs"]
mod generated;

#[test]
fn lean_v2_coordinates_are_the_pinned_account_profile_abi() {
    assert_eq!(generated::ACCOUNT_PROFILE_V2_MAGIC, *b"DCLTAP02");
    assert_eq!(generated::ACCOUNT_PROFILE_V2_VERSION, 2);
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_SCHEMA_RELEASE_PREIMAGE,
        b"dclutch/schema/account-profile-v2"
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_SCHEMA_RELEASE_ID,
        [
            0x4b, 0x66, 0x56, 0x93, 0x89, 0x0c, 0x76, 0x23, 0xb5, 0x65, 0x2b, 0x82, 0xe8, 0x5b,
            0x26, 0x4a, 0xc1, 0xa5, 0x26, 0xe7, 0x6a, 0x3d, 0x8e, 0x3c, 0x8c, 0x1d, 0xd4, 0xd4,
            0x6c, 0xc8, 0xe7, 0xfc
        ]
    );
    assert_eq!(generated::ACCOUNT_PROFILE_V2_ARTIFACT_PROFILE, 2);
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_SELECTED_WINDOW_ARTIFACT_PROFILE,
        3
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TYPED_SCALAR_ARTIFACT_PROFILE,
        4
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE,
        5
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_LIFECYCLE_PRESTATE_ARTIFACT_PROFILE,
        6
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_ADAPTER_AUTHENTICATED_VARIABLE_DATA_ARTIFACT_PROFILE,
        7
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE,
        8
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE,
        9
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE,
        10
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE,
        11
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE,
        12
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE,
        13
    );
    assert_eq!(generated::ACCOUNT_PROFILE_V2_HEADER_BYTES, 32);
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES,
        36
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES,
        40
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_DYNAMIC_FIXED_SPAN_HEADER_BYTES,
        48
    );
    assert_eq!(generated::ACCOUNT_RULE_V2_BYTES, 16);
    assert_eq!(generated::ACCOUNT_OPERATION_V2_BYTES, 16);
    assert_eq!(generated::DYNAMIC_FIXED_SPAN_V2_ENTRY_BYTES, 20);
    assert_eq!(generated::ACCOUNT_PROFILE_V2_MAGIC_OFFSET, 0);
    assert_eq!(generated::ACCOUNT_PROFILE_V2_VERSION_OFFSET, 8);
    assert_eq!(generated::ACCOUNT_PROFILE_V2_ARTIFACT_PROFILE_OFFSET, 10);
    assert_eq!(generated::ACCOUNT_PROFILE_V2_FIXED_ACCOUNTS_OFFSET, 12);
    assert_eq!(generated::ACCOUNT_PROFILE_V2_ITEM_ACCOUNT_STRIDE_OFFSET, 14);
    assert_eq!(generated::ACCOUNT_PROFILE_V2_FIXED_OPERATIONS_OFFSET, 16);
    assert_eq!(generated::ACCOUNT_PROFILE_V2_ITEM_OPERATIONS_OFFSET, 18);
    assert_eq!(generated::ACCOUNT_PROFILE_V2_COMMON_SCALARS_OFFSET, 20);
    assert_eq!(generated::ACCOUNT_PROFILE_V2_ITEM_SCALAR_STRIDE_OFFSET, 22);
    assert_eq!(generated::ACCOUNT_PROFILE_V2_COMMON_IDENTITIES_OFFSET, 24);
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_ITEM_IDENTITY_STRIDE_OFFSET,
        26
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_ENVIRONMENT_SCALAR_OFFSET,
        28
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_ENVIRONMENT_KIND_OFFSET,
        30
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_ENVIRONMENT_RESERVED_OFFSET,
        31
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_EXECUTING_PROGRAM_IDENTITY_OFFSET,
        32
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_EXECUTING_PROGRAM_KIND_OFFSET,
        34
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_EXECUTING_PROGRAM_RESERVED_OFFSET,
        35
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_BUILTIN_IDENTITY_OFFSET,
        36
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_BUILTIN_KIND_OFFSET,
        38
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_BUILTIN_RESERVED_OFFSET,
        39
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_DYNAMIC_FIXED_SPAN_COUNT_OFFSET,
        40
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_DYNAMIC_FIXED_SPAN_RESERVED_OFFSET,
        42
    );
    assert_eq!(generated::ACCOUNT_RULE_V2_PRIVILEGES_OFFSET, 0);
    assert_eq!(generated::ACCOUNT_RULE_V2_EFFECT_PERMISSIONS_OFFSET, 1);
    assert_eq!(generated::ACCOUNT_RULE_V2_ALIAS_KIND_OFFSET, 2);
    assert_eq!(generated::ACCOUNT_RULE_V2_PRESTATE_OFFSET, 3);
    assert_eq!(generated::ACCOUNT_RULE_V2_ALIAS_INDEX_OFFSET, 4);
    assert_eq!(generated::ACCOUNT_RULE_V2_RESERVED_OFFSET, 6);
    assert_eq!(generated::ACCOUNT_RULE_V2_DATA_LENGTH_OFFSET, 8);
    assert_eq!(generated::ACCOUNT_RULE_V2_DATA_ITEM_STRIDE_OFFSET, 12);
    assert_eq!(generated::ACCOUNT_OPERATION_V2_OPCODE_OFFSET, 0);
    assert_eq!(generated::ACCOUNT_OPERATION_V2_ACCOUNT_SPACE_OFFSET, 1);
    assert_eq!(generated::ACCOUNT_OPERATION_V2_ACCOUNT_OFFSET, 2);
    assert_eq!(generated::ACCOUNT_OPERATION_V2_REGISTER_SPACE_OFFSET, 4);
    assert_eq!(generated::ACCOUNT_OPERATION_V2_RESERVED_OFFSET, 5);
    assert_eq!(generated::ACCOUNT_OPERATION_V2_REGISTER_OFFSET, 6);
    assert_eq!(generated::ACCOUNT_OPERATION_V2_DATA_OFFSET_OFFSET, 8);
    assert_eq!(generated::ACCOUNT_OPERATION_V2_DATA_STRIDE_OFFSET, 12);
    assert_eq!(generated::DYNAMIC_FIXED_SPAN_V2_ENTRY_INSERTION_OFFSET, 0);
    assert_eq!(
        generated::DYNAMIC_FIXED_SPAN_V2_ENTRY_COUNT_SCALAR_OFFSET,
        2
    );
    assert_eq!(generated::DYNAMIC_FIXED_SPAN_V2_ENTRY_RULE_START_OFFSET, 4);
    assert_eq!(generated::DYNAMIC_FIXED_SPAN_V2_ENTRY_RULE_STRIDE_OFFSET, 6);
    assert_eq!(generated::DYNAMIC_FIXED_SPAN_V2_ENTRY_MIN_OFFSET, 8);
    assert_eq!(generated::DYNAMIC_FIXED_SPAN_V2_ENTRY_MAX_OFFSET, 12);
    assert_eq!(generated::DYNAMIC_FIXED_SPAN_V2_ENTRY_STEP_OFFSET, 16);
    assert_eq!(generated::OP_REQUIRE_KEY_V2, 0);
    assert_eq!(generated::OP_REQUIRE_OWNER_V2, 1);
    assert_eq!(generated::OP_PROJECT_KEY_V2, 2);
    assert_eq!(generated::OP_PROJECT_OWNER_V2, 3);
    assert_eq!(generated::OP_PROJECT_LAMPORTS_V2, 4);
    assert_eq!(generated::OP_PROJECT_DATA_U64_V2, 5);
    assert_eq!(generated::OP_PROJECT_DATA_IDENTITY_V2, 6);
    assert_eq!(generated::OP_PROJECT_DATA_U32_V2, 7);
    assert_eq!(generated::OP_PROJECT_TAIL_COUNT_U32_V2, 8);
    assert_eq!(generated::OP_PROJECT_DATA_U64_AFFINE_V2, 9);
    assert_eq!(generated::OP_PROJECT_DATA_IDENTITY_AFFINE_V2, 10);
    assert_eq!(generated::OP_SELECT_DATA_WINDOW_V2, 11);
    assert_eq!(generated::OP_PROJECT_DATA_U64_SELECTED_V2, 12);
    assert_eq!(generated::OP_PROJECT_DATA_IDENTITY_SELECTED_V2, 13);
    assert_eq!(generated::OP_PROJECT_DATA_U64_SELECTED_AFFINE_V2, 14);
    assert_eq!(generated::OP_PROJECT_DATA_IDENTITY_SELECTED_AFFINE_V2, 15);
    assert_eq!(generated::OP_PROJECT_DATA_U16_V2, 16);
    assert_eq!(generated::OP_PROJECT_DATA_U8_V2, 17);
    assert_eq!(generated::OP_PROJECT_NONZERO_U64_TAIL_COUNT_V2, 18);
    assert_eq!(generated::OP_PROJECT_NONZERO_U64_TAIL_ROWS_V2, 19);
    assert_eq!(generated::OP_PROJECT_DATA_DIGEST_V2, 20);
    assert_eq!(generated::ACCOUNT_PROFILE_V2_TRUSTED_ENVIRONMENT_NONE, 0);
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_ENVIRONMENT_CURRENT_SLOT,
        1
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_EXECUTING_PROGRAM_NONE,
        0
    );
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_EXECUTING_PROGRAM_CURRENT,
        1
    );
    assert_eq!(generated::ACCOUNT_PROFILE_V2_TRUSTED_BUILTIN_NONE, 0);
    assert_eq!(
        generated::ACCOUNT_PROFILE_V2_TRUSTED_BUILTIN_SYSTEM_PROGRAM,
        1
    );
    assert_eq!(generated::ACCOUNT_ALIAS_V2_SELF_COORDINATE, 0);
    assert_eq!(generated::ACCOUNT_ALIAS_V2_FIXED, 1);
    assert_eq!(generated::ACCOUNT_ALIAS_V2_SAME_ITEM, 2);
    assert_eq!(generated::ACCOUNT_REGISTER_SPACE_V2_COMMON, 0);
    assert_eq!(generated::ACCOUNT_REGISTER_SPACE_V2_ITEM, 1);
    assert_eq!(generated::ACCOUNT_PRESTATE_V2_EXACT, 0);
    assert_eq!(generated::ACCOUNT_PRESTATE_V2_LIFECYCLE_BOUND, 1);
    assert_eq!(
        generated::ACCOUNT_PRESTATE_V2_ADAPTER_AUTHENTICATED_VARIABLE_DATA,
        2
    );
    assert_eq!(
        generated::ACCOUNT_PRESTATE_V2_ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS,
        3
    );
    assert_eq!(generated::ACCOUNT_PRESTATE_V2_AUTHENTICATED_ROUTE_ALIAS, 4);
    assert_eq!(
        generated::ACCOUNT_PRESTATE_V2_AUTHENTICATED_OPAQUE_READONLY_DATA,
        5
    );
    assert_eq!(generated::ACCOUNT_PRESTATE_V2_MIN_ARTIFACT_PROFILE, 2);
    assert_eq!(generated::ACCOUNT_PRESTATE_V2_TAG_COUNT, 6);
    assert_eq!(
        generated::ACCOUNT_PRESTATE_V2_REFUSAL_NON_CANONICAL_RESERVED,
        0
    );
    assert_eq!(
        generated::ACCOUNT_PRESTATE_V2_REFUSAL_INVALID_LIFECYCLE_PRESTATE,
        1
    );
    assert_eq!(
        generated::ACCOUNT_PRESTATE_V2_REFUSAL_INVALID_VARIABLE_DATA_PRESTATE,
        2
    );
    assert_eq!(
        generated::ACCOUNT_PRESTATE_V2_ADMISSIBLE,
        [
            [true, false, false, false, false, false],
            [true, false, false, false, false, false],
            [true, false, false, false, false, false],
            [true, false, false, false, false, false],
            [true, true, false, false, false, false],
            [true, true, true, false, false, false],
            [true, true, true, false, false, false],
            [true, true, true, true, false, false],
            [true, true, true, true, false, false],
            [true, true, true, true, true, false],
            [true, true, true, true, false, false],
            [true, true, true, false, true, true],
            [true, true, true, false, true, true],
        ]
    );
    assert_eq!(
        generated::ACCOUNT_PRESTATE_V2_REFUSAL_CLASS,
        [0, 0, 0, 0, 1, 2, 2, 2, 2, 2, 2, 2, 2]
    );
}
