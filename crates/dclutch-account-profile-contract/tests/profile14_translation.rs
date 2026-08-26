//! Translation agreement between Lean-owned Profile 14 coordinates and Rust.

#[allow(dead_code, missing_docs)]
#[path = "../src/v2/generated_profile14.rs"]
mod generated;

use dclutch_account_profile_contract::v2::{
    AccountPrestateV2, AccountProfileV2, FIXED_DATA_PREDICATE_ARTIFACT_PROFILE,
    FIXED_DATA_PREDICATE_BYTES, FIXED_DATA_PREDICATE_HEADER_BYTES, FIXED_DATA_PREDICATE_PROFILE_ID,
    FIXED_DATA_PREDICATE_PROFILE_PREIMAGE,
    encode::{
        AccountAliasInputV2, AccountEffectPermissionsV2, AccountPrivilegesV2, AccountRuleInputV2,
        AccountRuleWithPrestateInputV2, FixedDataPredicateInputV2, RegisterGeometryV2,
        encode_account_profile_with_fixed_data_predicates_v2_atomic,
    },
};

fn exact_rule(data_length: u32) -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(false, false, false),
            effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::Exact,
    }
}

#[test]
fn generated_constants_equal_the_live_profile14_kernel() {
    assert_eq!(
        generated::FIXED_DATA_PREDICATE_ARTIFACT_PROFILE,
        FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
    );
    assert_eq!(
        generated::FIXED_DATA_PREDICATE_HEADER_BYTES,
        FIXED_DATA_PREDICATE_HEADER_BYTES
    );
    assert_eq!(
        generated::FIXED_DATA_PREDICATE_BYTES,
        FIXED_DATA_PREDICATE_BYTES
    );
    assert_eq!(
        generated::FIXED_DATA_PREDICATE_PROFILE_PREIMAGE,
        FIXED_DATA_PREDICATE_PROFILE_PREIMAGE
    );
    assert_eq!(
        generated::FIXED_DATA_PREDICATE_PROFILE_ID,
        FIXED_DATA_PREDICATE_PROFILE_ID
    );
    assert_eq!(generated::FIXED_DATA_PREDICATE_DYNAMIC_SPAN_ENTRY_BYTES, 20);
    assert_eq!(
        generated::FIXED_DATA_PREDICATE_DYNAMIC_SPAN_COUNT_OFFSET,
        40
    );
    assert_eq!(generated::FIXED_DATA_PREDICATE_COUNT_OFFSET, 42);
    assert_eq!(generated::FIXED_DATA_PREDICATE_HEADER_RESERVED_OFFSET, 44);
    assert_eq!(
        [
            generated::FIXED_DATA_PREDICATE_OPCODE_OFFSET_V2,
            generated::FIXED_DATA_PREDICATE_RESERVED_OFFSET_V2,
            generated::FIXED_DATA_PREDICATE_ACCOUNT_OFFSET_V2,
            generated::FIXED_DATA_PREDICATE_DATA_OFFSET_V2,
            generated::FIXED_DATA_PREDICATE_PAYLOAD_OFFSET_V2,
        ],
        [0, 1, 2, 4, 8]
    );
}

#[test]
fn every_lean_owned_predicate_kind_round_trips_through_rust() {
    let predicates = [
        FixedDataPredicateInputV2::RequireDataU8 {
            account: 0,
            data_offset: 0,
            value: 1,
        },
        FixedDataPredicateInputV2::RequireDataU16 {
            account: 0,
            data_offset: 1,
            value: 2,
        },
        FixedDataPredicateInputV2::RequireDataU32 {
            account: 0,
            data_offset: 3,
            value: 3,
        },
        FixedDataPredicateInputV2::RequireDataU64 {
            account: 0,
            data_offset: 7,
            value: 4,
        },
        FixedDataPredicateInputV2::RequireZeroRange {
            account: 0,
            data_offset: 15,
            length: 1,
        },
    ];
    let width =
        FIXED_DATA_PREDICATE_HEADER_BYTES + predicates.len() * FIXED_DATA_PREDICATE_BYTES + 16;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_account_profile_with_fixed_data_predicates_v2_atomic(
        dclutch_account_profile_contract::v2::TrustedEnvironmentV2::None,
        dclutch_account_profile_contract::v2::TrustedIdentityEnvironmentV2::None,
        dclutch_account_profile_contract::v2::TrustedBuiltinIdentityV2::None,
        &[],
        &predicates,
        &[exact_rule(16)],
        &[],
        &[],
        RegisterGeometryV2 {
            common_scalars: 1,
            item_scalar_stride: 0,
            common_identities: 1,
            item_identity_stride: 0,
        },
        &mut scratch,
        &mut output,
    )
    .expect("encode all Profile14 predicate kinds");
    let decoded = AccountProfileV2::decode(&output).expect("decode all predicate kinds");
    assert_eq!(decoded.fixed_data_predicate_count(), 5);
    assert!(decoded.uses_dynamic_fixed_spans());
    assert!(decoded.supports_route_alias_packing());
    assert!(decoded.uses_fixed_data_predicates());
    for (index, opcode) in [1_u8, 2, 3, 4, 5].into_iter().enumerate() {
        let offset = FIXED_DATA_PREDICATE_HEADER_BYTES + index * FIXED_DATA_PREDICATE_BYTES;
        assert_eq!(output.get(offset).copied(), Some(opcode));
    }
}
