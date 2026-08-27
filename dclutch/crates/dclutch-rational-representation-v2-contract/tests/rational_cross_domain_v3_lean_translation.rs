//! Translation checks for Lean-owned independent Rational K / Product N facts.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

#[allow(dead_code, missing_docs)]
#[path = "support/generated_rational_cross_domain_v3.rs"]
mod generated;

use dclutch_rational_representation_v2_contract::{
    ABSENT_REVISION, ASSET_BYTES_V2, AssetV2, CallerRoleV2, Error,
    RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3, RATIONAL_TERMINAL_SCALAR_OUTCOME_COUNT_V3,
    RATIONAL_TERMINAL_SCALAR_PRODUCT_OUTCOME_COUNT_V3, RationalTerminalHotRequestV3,
    RepresentationActionV2, RepresentationRequestHeaderV2, RepresentationRequestV2,
};
use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CrossDomainObservation {
    basis_width: u32,
    terminal_width: u32,
    claim_coordinate: u32,
    terminal_selector: u32,
}

fn read_u16(input: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        input.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn read_u32(input: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        input.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn decode_corpus(input: &[u8]) -> Option<CrossDomainObservation> {
    if input.len() != generated::RATIONAL_CROSS_DOMAIN_BYTES_V3
        || input.get(
            generated::RATIONAL_CROSS_DOMAIN_MAGIC_OFFSET_V3
                ..generated::RATIONAL_CROSS_DOMAIN_MAGIC_OFFSET_V3 + 8,
        )? != generated::RATIONAL_CROSS_DOMAIN_MAGIC_V3
        || read_u16(input, generated::RATIONAL_CROSS_DOMAIN_VERSION_OFFSET_V3)?
            != generated::RATIONAL_CROSS_DOMAIN_VERSION_V3
        || input
            .get(
                generated::RATIONAL_CROSS_DOMAIN_RESERVED_OFFSET_V3
                    ..generated::RATIONAL_CROSS_DOMAIN_RESERVED_OFFSET_V3 + 2,
            )?
            .iter()
            .any(|byte| *byte != 0)
    {
        return None;
    }
    let value = CrossDomainObservation {
        basis_width: read_u32(
            input,
            generated::RATIONAL_CROSS_DOMAIN_BASIS_WIDTH_OFFSET_V3,
        )?,
        terminal_width: read_u32(
            input,
            generated::RATIONAL_CROSS_DOMAIN_TERMINAL_WIDTH_OFFSET_V3,
        )?,
        claim_coordinate: read_u32(
            input,
            generated::RATIONAL_CROSS_DOMAIN_CLAIM_COORDINATE_OFFSET_V3,
        )?,
        terminal_selector: read_u32(
            input,
            generated::RATIONAL_CROSS_DOMAIN_TERMINAL_SELECTOR_OFFSET_V3,
        )?,
    };
    if value.basis_width < 2
        || value.terminal_width < 2
        || value.claim_coordinate >= value.basis_width
        || value.terminal_selector >= value.terminal_width
    {
        return None;
    }
    Some(value)
}

fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

fn family_for(
    observation: CrossDomainObservation,
) -> Result<[u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3], Error> {
    let mut asset = [0_u8; ASSET_BYTES_V2];
    AssetV2 {
        shard_mint: id(20),
        actor_shard_account: id(21),
        structured_custody_account: id(22),
        claims_custody_owner: id(23),
        coefficient: 1,
        expected_shard_supply: 10,
        expected_actor_shards: 10,
        expected_structured_shards: 0,
    }
    .encode_into(&mut asset)?;
    let child = RepresentationRequestV2::new(
        RepresentationRequestHeaderV2 {
            action: RepresentationActionV2::RedeemTerminal,
            caller_role: CallerRoleV2::Trading,
            release_set: id(1),
            market: id(2),
            graph_id: id(3),
            descriptor_id: id(4),
            parent_context: id(5),
            actor: id(6),
            receipt_mint: id(7),
            receipt_account: [0; 32],
            representation_authority: id(8),
            token_program: TOKEN_2022_PROGRAM_ID,
            realm: id(9),
            collateral_recipient: id(10),
            expected_representation_revision: 1,
            expected_claims_market_revision: 2,
            expected_actor_position_revision: ABSENT_REVISION,
            expected_custody_position_revision: 3,
            expected_custody_replay_revision: 4,
            generation: 5,
            quantity: 1,
            denominator: 10,
            expected_receipt_supply: 0,
            outcome_count: observation.basis_width,
            selected_outcome: observation.claim_coordinate,
            asset_count: 1,
        },
        &asset,
    )?;
    let mut family = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
    RationalTerminalHotRequestV3::from_child_into(child, &mut family)?;
    Ok(family)
}

#[test]
fn lean_k3_n9_and_k3_n258_witnesses_preserve_independent_registers() {
    for bytes in [
        generated::RATIONAL_CROSS_DOMAIN_K3_N9_WITNESS.as_slice(),
        generated::RATIONAL_CROSS_DOMAIN_K3_N258_WITNESS.as_slice(),
    ] {
        let observation = decode_corpus(bytes).expect("Lean canonical witness");
        assert_eq!(observation.basis_width, 3);
        assert_ne!(observation.basis_width, observation.terminal_width);
        let family_bytes = family_for(observation).expect("claim coordinate is bounded by K");
        let family = RationalTerminalHotRequestV3::decode(&family_bytes).expect("Hot request");
        let registers = family
            .project_registers(id(90), observation.terminal_width)
            .expect("independent Product width");
        assert_eq!(
            registers.scalar(RATIONAL_TERMINAL_SCALAR_OUTCOME_COUNT_V3),
            Ok(u64::from(observation.basis_width))
        );
        assert_eq!(
            registers.scalar(RATIONAL_TERMINAL_SCALAR_PRODUCT_OUTCOME_COUNT_V3),
            Ok(u64::from(observation.terminal_width))
        );
    }
}

#[test]
fn lean_refusal_corpus_rejects_cross_domain_and_boundary_substitution() {
    for hostile in [
        generated::RATIONAL_CROSS_DOMAIN_CLAIM_AT_K_REFUSAL.as_slice(),
        generated::RATIONAL_CROSS_DOMAIN_TERMINAL_AT_N9_REFUSAL.as_slice(),
        generated::RATIONAL_CROSS_DOMAIN_TERMINAL_AS_CLAIM_REFUSAL.as_slice(),
        generated::RATIONAL_CROSS_DOMAIN_TERMINAL_AT_N258_REFUSAL.as_slice(),
        generated::RATIONAL_CROSS_DOMAIN_ZERO_BASIS_REFUSAL.as_slice(),
        generated::RATIONAL_CROSS_DOMAIN_ZERO_TERMINAL_REFUSAL.as_slice(),
        generated::RATIONAL_CROSS_DOMAIN_RESERVED_REFUSAL.as_slice(),
    ] {
        assert_eq!(decode_corpus(hostile), None);
    }
}

#[test]
fn live_hot_refuses_claim_at_k_and_invalid_product_width_independently() {
    let valid = decode_corpus(&generated::RATIONAL_CROSS_DOMAIN_K3_N9_WITNESS)
        .expect("Lean canonical witness");
    let family_bytes = family_for(valid).expect("claim coordinate is bounded by K");
    let family = RationalTerminalHotRequestV3::decode(&family_bytes).expect("Hot request");
    assert_eq!(
        family.project_registers(id(90), 1),
        Err(Error::InvalidWidth)
    );

    let mut invalid_claim = valid;
    invalid_claim.claim_coordinate = invalid_claim.basis_width;
    assert!(
        family_for(invalid_claim).is_err(),
        "request constructor must refuse claim == K"
    );
}
