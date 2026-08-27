//! Product-to-Claims agreement, hostile, and atomic-candidate evidence.

use core::convert::TryInto;

use dclutch_liability_basis_v2_kernel::product_claims::{
    AdmittedBasisV2, BASIS_MAGIC_V2, BASIS_PRODUCT_LINK_END_V2, BASIS_PRODUCT_LINK_OFFSET_V2,
    BasisKindV2, CAPPED_RAMP_BASIS_BYTES_V2, CATEGORICAL_BASIS_BYTES_V2, CappedRampBasisInputV2,
    CategoricalBasisInputV2, ClaimsOperationV2, ContentIdV2, LINKED_CAPPED_RAMP_BASIS_BYTES_V2,
    LinkedBasisRecordV2, ProductClaimsErrorV2, TerminalResultV2, encode_capped_ramp_basis_v2,
    encode_categorical_basis_v2, encode_linked_basis_record_v2, semantic_basis_preimage_v2,
};
use dclutch_liability_basis_v2_kernel::{
    AGREEMENT_CASES_V2, RAMP_COORDINATE_DENOMINATOR_OFFSET_V2, RAMP_COORDINATE_NUMERATOR_OFFSET_V2,
    RAMP_KNOT_DENOMINATOR_OFFSET_V2, RAMP_LEFT_NUMERATOR_OFFSET_V2, RAMP_RIGHT_NUMERATOR_OFFSET_V2,
    RAMP_SCALE_OFFSET_V2,
};

fn id(fill: u8) -> ContentIdV2 {
    ContentIdV2::new([fill; 32]).expect("nonzero test identity")
}

fn categorical(width: u32) -> (AdmittedBasisV2, [u8; CATEGORICAL_BASIS_BYTES_V2]) {
    let mut bytes = [0_u8; CATEGORICAL_BASIS_BYTES_V2];
    encode_categorical_basis_v2(
        CategoricalBasisInputV2 {
            product_instance_id: id(1),
            claim_count: width,
        },
        &mut bytes,
    )
    .expect("categorical record");
    let admitted = AdmittedBasisV2::admit(&bytes, id(2), id(2), id(1)).expect("admitted");
    (admitted, bytes)
}

fn ramp(
    scale: u64,
    knot_denominator: u32,
    left_numerator: i64,
    right_numerator: i64,
) -> (AdmittedBasisV2, [u8; CAPPED_RAMP_BASIS_BYTES_V2]) {
    let mut bytes = [0_u8; CAPPED_RAMP_BASIS_BYTES_V2];
    encode_capped_ramp_basis_v2(
        CappedRampBasisInputV2 {
            product_instance_id: id(1),
            knot_denominator,
            left_numerator,
            right_numerator,
            scale,
        },
        &mut bytes,
    )
    .expect("ramp record");
    let admitted = AdmittedBasisV2::admit(&bytes, id(2), id(2), id(1)).expect("admitted");
    (admitted, bytes)
}

#[test]
fn categorical_q_one_is_the_runtime_width_embedding() {
    let (basis, _) = categorical(257);
    assert_eq!(basis.kind(), BasisKindV2::CategoricalQ1);
    assert_eq!(basis.claim_count(), 257);
    assert_eq!(basis.scale(), 1);
    assert_eq!(basis.basis_id(), id(2));
    assert_eq!(basis.product_instance_id(), id(1));

    let mut payouts = [9_u64; 257];
    basis
        .evaluate_terminal_into(TerminalResultV2::Categorical { winner: 193 }, &mut payouts)
        .expect("one hot");
    assert_eq!(payouts.iter().sum::<u64>(), 1);
    assert_eq!(payouts[193], 1);
    assert!(
        payouts
            .iter()
            .enumerate()
            .all(|(index, payout)| *payout == u64::from(index == 193))
    );
}

#[test]
fn content_bound_ramp_exactly_matches_every_lean_emitted_case() {
    for (case_index, case) in AGREEMENT_CASES_V2.iter().enumerate() {
        let request = &case.request;
        let scale = u32::from_le_bytes(
            request[RAMP_SCALE_OFFSET_V2..RAMP_SCALE_OFFSET_V2 + 4]
                .try_into()
                .expect("scale bytes"),
        );
        let knot_denominator = u32::from_le_bytes(
            request[RAMP_KNOT_DENOMINATOR_OFFSET_V2..RAMP_KNOT_DENOMINATOR_OFFSET_V2 + 4]
                .try_into()
                .expect("knot denominator bytes"),
        );
        let left_numerator = i64::from_le_bytes(
            request[RAMP_LEFT_NUMERATOR_OFFSET_V2..RAMP_LEFT_NUMERATOR_OFFSET_V2 + 8]
                .try_into()
                .expect("left bytes"),
        );
        let right_numerator = i64::from_le_bytes(
            request[RAMP_RIGHT_NUMERATOR_OFFSET_V2..RAMP_RIGHT_NUMERATOR_OFFSET_V2 + 8]
                .try_into()
                .expect("right bytes"),
        );
        let coordinate_numerator = i64::from_le_bytes(
            request[RAMP_COORDINATE_NUMERATOR_OFFSET_V2..RAMP_COORDINATE_NUMERATOR_OFFSET_V2 + 8]
                .try_into()
                .expect("coordinate bytes"),
        );
        let coordinate_denominator = u32::from_le_bytes(
            request
                [RAMP_COORDINATE_DENOMINATOR_OFFSET_V2..RAMP_COORDINATE_DENOMINATOR_OFFSET_V2 + 4]
                .try_into()
                .expect("coordinate denominator bytes"),
        );
        let (basis, _) = ramp(
            u64::from(scale),
            knot_denominator,
            left_numerator,
            right_numerator,
        );
        let mut payout = [u64::MAX; 2];
        let evaluation = basis.evaluate_terminal_into(
            TerminalResultV2::RationalCoordinate {
                numerator: coordinate_numerator,
                denominator: coordinate_denominator,
            },
            &mut payout,
        );
        assert_eq!(evaluation, Ok(()), "case {case_index}");
        assert_eq!(payout, case.expected, "case {case_index}");
        assert_eq!(payout[0] + payout[1], u64::from(scale));
    }
}

#[test]
fn split_and_merge_emit_complete_exact_solvency_candidates() {
    let (basis, _) = ramp(100, 1, 0, 10);
    let aggregate = [11_u64, 7];
    let position = [3_u64, 3];
    let mut split_aggregate = [99_u64; 2];
    let mut split_position = [99_u64; 2];
    let split = basis
        .plan_split_into(
            &aggregate,
            &position,
            4,
            1_100,
            &mut split_aggregate,
            &mut split_position,
        )
        .expect("solvent split");
    assert_eq!(split.operation(), ClaimsOperationV2::Split);
    assert_eq!(split.basis_id(), id(2));
    assert_eq!(split.product_instance_id(), id(1));
    assert_eq!(split.claim_count(), 2);
    assert_eq!(split.quantity(), 4);
    assert_eq!(split.collateral_in(), 400);
    assert_eq!(split.collateral_out(), 0);
    assert_eq!(split.hoard_before(), 1_100);
    assert_eq!(split.hoard_after(), 1_500);
    assert_eq!(split.liability_before(), 1_100);
    assert_eq!(split.liability_after(), 1_500);
    assert_eq!(split_aggregate, [15, 11]);
    assert_eq!(split_position, [7, 7]);

    let mut merged_aggregate = [99_u64; 2];
    let mut merged_position = [99_u64; 2];
    let merge = basis
        .plan_merge_into(
            &split_aggregate,
            &split_position,
            4,
            split.hoard_after(),
            &mut merged_aggregate,
            &mut merged_position,
        )
        .expect("solvent merge");
    assert_eq!(merge.operation(), ClaimsOperationV2::Merge);
    assert_eq!(merge.collateral_in(), 0);
    assert_eq!(merge.collateral_out(), 400);
    assert_eq!(merge.liability_after(), 1_100);
    assert_eq!(merged_aggregate, aggregate);
    assert_eq!(merged_position, position);
}

#[test]
fn terminal_redemption_uses_the_named_floor_once_and_exact_complement() {
    let (basis, _) = ramp(10, 1, 0, 1);
    let result = TerminalResultV2::RationalCoordinate {
        numerator: 1,
        denominator: 3,
    };
    let mut payouts = [99_u64; 2];
    basis
        .evaluate_terminal_into(result, &mut payouts)
        .expect("terminal partition");
    assert_eq!(payouts, [3, 7]);

    let aggregate = [5_u64, 7];
    let position = [2_u64, 3];
    let mut candidate_aggregate = [99_u64; 2];
    let mut candidate_position = [99_u64; 2];
    let plan = basis
        .plan_terminal_redeem_into(
            result,
            1,
            &aggregate,
            &position,
            3,
            64,
            &mut candidate_aggregate,
            &mut candidate_position,
        )
        .expect("exact terminal redeem");
    assert_eq!(plan.operation(), ClaimsOperationV2::TerminalRedeem);
    assert_eq!(plan.collateral_out(), 21);
    assert_eq!(plan.liability_before(), 64);
    assert_eq!(plan.liability_after(), 43);
    assert_eq!(plan.hoard_after(), 43);
    assert_eq!(candidate_aggregate, [5, 4]);
    assert_eq!(candidate_position, [2, 0]);
}

#[test]
fn categorical_terminal_redeem_is_the_same_claims_path() {
    let (basis, _) = categorical(4);
    let aggregate = [3_u64, 5, 7, 11];
    let position = [1_u64, 2, 3, 4];
    let mut candidate_aggregate = [99_u64; 4];
    let mut candidate_position = [99_u64; 4];
    let plan = basis
        .plan_terminal_redeem_into(
            TerminalResultV2::Categorical { winner: 2 },
            2,
            &aggregate,
            &position,
            3,
            7,
            &mut candidate_aggregate,
            &mut candidate_position,
        )
        .expect("categorical redeem");
    assert_eq!(plan.collateral_out(), 3);
    assert_eq!(plan.liability_before(), 7);
    assert_eq!(plan.liability_after(), 4);
    assert_eq!(candidate_aggregate, [3, 5, 4, 11]);
    assert_eq!(candidate_position, [1, 2, 0, 4]);
}

#[test]
fn hostile_record_and_identity_mutations_refuse() {
    assert_eq!(
        ContentIdV2::new([0; 32]),
        Err(ProductClaimsErrorV2::ZeroIdentifier)
    );
    let (_, categorical_bytes) = categorical(4);
    assert_eq!(
        AdmittedBasisV2::admit(&categorical_bytes, id(2), id(3), id(1)),
        Err(ProductClaimsErrorV2::IdentityMismatch)
    );
    assert_eq!(
        AdmittedBasisV2::admit(&categorical_bytes, id(2), id(2), id(3)),
        Err(ProductClaimsErrorV2::IdentityMismatch)
    );
    for length in 0..CATEGORICAL_BASIS_BYTES_V2 {
        let prefix = categorical_bytes.get(..length).expect("bounded prefix");
        assert_eq!(
            AdmittedBasisV2::admit(prefix, id(2), id(2), id(1)),
            Err(ProductClaimsErrorV2::InvalidLength)
        );
    }
    let mut extended = [0_u8; CATEGORICAL_BASIS_BYTES_V2 + 1];
    extended[..CATEGORICAL_BASIS_BYTES_V2].copy_from_slice(&categorical_bytes);
    assert_eq!(
        AdmittedBasisV2::admit(&extended, id(2), id(2), id(1)),
        Err(ProductClaimsErrorV2::InvalidLength)
    );

    let mutations = [
        (0_usize, ProductClaimsErrorV2::InvalidMagic),
        (8, ProductClaimsErrorV2::UnsupportedSchema),
        (10, ProductClaimsErrorV2::UnsupportedProfile),
        (13, ProductClaimsErrorV2::NonCanonicalRecord),
        (16, ProductClaimsErrorV2::EmptyBasis),
        (24, ProductClaimsErrorV2::NonCanonicalRecord),
        (32, ProductClaimsErrorV2::ZeroIdentifier),
        (64, ProductClaimsErrorV2::NonCanonicalRecord),
        (96, ProductClaimsErrorV2::NonCanonicalRecord),
    ];
    for (offset, expected) in mutations {
        let mut hostile = categorical_bytes;
        if offset == 16 || offset == 24 || offset == 32 {
            let width = if offset == 16 {
                4
            } else if offset == 24 {
                8
            } else {
                32
            };
            if let Some(region) = hostile.get_mut(offset..offset + width) {
                region.fill(0);
            }
        } else if let Some(byte) = hostile.get_mut(offset) {
            *byte ^= 1;
        }
        assert_eq!(
            AdmittedBasisV2::admit(&hostile, id(2), id(2), id(1)),
            Err(expected),
            "offset {offset}"
        );
    }

    let (_, ramp_bytes) = ramp(10, 1, -2, 3);
    for offset in 132..136 {
        let mut hostile = ramp_bytes;
        if let Some(byte) = hostile.get_mut(offset) {
            *byte = 1;
        }
        assert_eq!(
            AdmittedBasisV2::admit(&hostile, id(2), id(2), id(1)),
            Err(ProductClaimsErrorV2::NonCanonicalRecord)
        );
    }
    let mut hostile = ramp_bytes;
    hostile[128..132].fill(0);
    assert_eq!(
        AdmittedBasisV2::admit(&hostile, id(2), id(2), id(1)),
        Err(ProductClaimsErrorV2::NonCanonicalRecord)
    );
    let mut hostile = ramp_bytes;
    hostile[136..144].copy_from_slice(&4_i64.to_le_bytes());
    assert_eq!(
        AdmittedBasisV2::admit(&hostile, id(2), id(2), id(1)),
        Err(ProductClaimsErrorV2::NonCanonicalRecord)
    );
    assert_eq!(&ramp_bytes[..8], &BASIS_MAGIC_V2);
}

#[test]
fn encoders_and_terminal_evaluation_leave_outputs_unchanged_on_refusal() {
    let mut categorical_output = [0xa5_u8; CATEGORICAL_BASIS_BYTES_V2];
    assert_eq!(
        encode_categorical_basis_v2(
            CategoricalBasisInputV2 {
                product_instance_id: id(1),
                claim_count: 0,
            },
            &mut categorical_output,
        ),
        Err(ProductClaimsErrorV2::EmptyBasis)
    );
    assert_eq!(categorical_output, [0xa5; CATEGORICAL_BASIS_BYTES_V2]);

    let mut ramp_output = [0x5a_u8; CAPPED_RAMP_BASIS_BYTES_V2];
    assert_eq!(
        encode_capped_ramp_basis_v2(
            CappedRampBasisInputV2 {
                product_instance_id: id(1),
                knot_denominator: 1,
                left_numerator: 3,
                right_numerator: 3,
                scale: 10,
            },
            &mut ramp_output,
        ),
        Err(ProductClaimsErrorV2::NonCanonicalRecord)
    );
    assert_eq!(ramp_output, [0x5a; CAPPED_RAMP_BASIS_BYTES_V2]);

    let (basis, _) = ramp(10, 1, 0, 1);
    let mut payout = [77_u64; 2];
    assert_eq!(
        basis.evaluate_terminal_into(
            TerminalResultV2::RationalCoordinate {
                numerator: 1,
                denominator: 0,
            },
            &mut payout,
        ),
        Err(ProductClaimsErrorV2::InvalidTerminalResult)
    );
    assert_eq!(payout, [77, 77]);
    assert_eq!(
        basis.evaluate_terminal_into(TerminalResultV2::Categorical { winner: 0 }, &mut payout),
        Err(ProductClaimsErrorV2::InvalidTerminalResult)
    );
    assert_eq!(payout, [77, 77]);
}

#[test]
fn failed_claims_plans_are_atomic_under_hostile_state() {
    let (basis, _) = ramp(10, 1, 0, 1);
    let mut aggregate_after = [0xa5_u64; 2];
    let mut position_after = [0x5a_u64; 2];
    assert_eq!(
        basis.plan_split_into(
            &[2, 1],
            &[1, 1],
            1,
            19,
            &mut aggregate_after,
            &mut position_after,
        ),
        Err(ProductClaimsErrorV2::Insolvent)
    );
    assert_eq!(aggregate_after, [0xa5; 2]);
    assert_eq!(position_after, [0x5a; 2]);

    assert_eq!(
        basis.plan_split_into(
            &[2, 1],
            &[1, 2],
            1,
            20,
            &mut aggregate_after,
            &mut position_after,
        ),
        Err(ProductClaimsErrorV2::PositionExceedsSupply)
    );
    assert_eq!(aggregate_after, [0xa5; 2]);
    assert_eq!(position_after, [0x5a; 2]);

    assert_eq!(
        basis.plan_split_into(
            &[u64::MAX, 1],
            &[1, 1],
            1,
            u64::MAX,
            &mut aggregate_after,
            &mut position_after,
        ),
        Err(ProductClaimsErrorV2::ArithmeticOverflow)
    );
    assert_eq!(aggregate_after, [0xa5; 2]);
    assert_eq!(position_after, [0x5a; 2]);

    assert_eq!(
        basis.plan_merge_into(
            &[2, 1],
            &[1, 1],
            2,
            20,
            &mut aggregate_after,
            &mut position_after,
        ),
        Err(ProductClaimsErrorV2::InsufficientBalance)
    );
    assert_eq!(aggregate_after, [0xa5; 2]);
    assert_eq!(position_after, [0x5a; 2]);

    assert_eq!(
        basis.plan_terminal_redeem_into(
            TerminalResultV2::RationalCoordinate {
                numerator: 1,
                denominator: 2,
            },
            1,
            &[2, 1],
            &[1, 1],
            2,
            20,
            &mut aggregate_after,
            &mut position_after,
        ),
        Err(ProductClaimsErrorV2::InsufficientBalance)
    );
    assert_eq!(aggregate_after, [0xa5; 2]);
    assert_eq!(position_after, [0x5a; 2]);
}

#[test]
fn semantic_basis_identity_and_finalized_product_link_are_distinct_authorities() {
    let semantic_basis_id = id(9);
    let mut first_basis = [0_u8; CAPPED_RAMP_BASIS_BYTES_V2];
    let mut second_basis = [0_u8; CAPPED_RAMP_BASIS_BYTES_V2];
    for (product_instance_id, output) in [(id(1), &mut first_basis), (id(3), &mut second_basis)] {
        encode_capped_ramp_basis_v2(
            CappedRampBasisInputV2 {
                product_instance_id,
                knot_denominator: 7,
                left_numerator: -11,
                right_numerator: 19,
                scale: 101,
            },
            output,
        )
        .expect("canonical embedded basis");
    }
    assert_ne!(
        first_basis, second_basis,
        "Product link is present in raw bytes"
    );
    let first_semantic = semantic_basis_preimage_v2(&first_basis).expect("first semantic preimage");
    let second_semantic =
        semantic_basis_preimage_v2(&second_basis).expect("second semantic preimage");
    assert_eq!(BASIS_PRODUCT_LINK_OFFSET_V2, 32);
    assert_eq!(BASIS_PRODUCT_LINK_END_V2, 64);
    assert_eq!(first_semantic.prefix(), second_semantic.prefix());
    assert_eq!(first_semantic.suffix(), second_semantic.suffix());

    let mut noncanonical = first_basis;
    noncanonical[14] = 1;
    assert_eq!(
        semantic_basis_preimage_v2(&noncanonical),
        Err(ProductClaimsErrorV2::NonCanonicalRecord)
    );

    let mut first_link = [0_u8; LINKED_CAPPED_RAMP_BASIS_BYTES_V2];
    let mut second_link = [0_u8; LINKED_CAPPED_RAMP_BASIS_BYTES_V2];
    encode_linked_basis_record_v2(id(1), semantic_basis_id, &first_basis, &mut first_link)
        .expect("first linked record");
    encode_linked_basis_record_v2(id(3), semantic_basis_id, &second_basis, &mut second_link)
        .expect("second linked record");
    let first = LinkedBasisRecordV2::decode(&first_link).expect("first link");
    let second = LinkedBasisRecordV2::decode(&second_link).expect("second link");
    assert_eq!(first.semantic_basis_id(), second.semantic_basis_id());
    assert_ne!(first.product_instance_id(), second.product_instance_id());
    assert_ne!(
        first_link, second_link,
        "linked raw-record identities differ"
    );

    let mut refused = [0xa5_u8; LINKED_CAPPED_RAMP_BASIS_BYTES_V2];
    assert_eq!(
        encode_linked_basis_record_v2(id(3), semantic_basis_id, &first_basis, &mut refused),
        Err(ProductClaimsErrorV2::IdentityMismatch)
    );
    assert_eq!(refused, [0xa5; LINKED_CAPPED_RAMP_BASIS_BYTES_V2]);
}
