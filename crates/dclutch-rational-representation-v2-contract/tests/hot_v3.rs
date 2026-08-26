//! Adversarial corpus for the Rational terminal Hot V3 specialization.

use dclutch_rational_representation_v2_contract::{
    ABSENT_REVISION, ASSET_BYTES_V2, AssetV2, CallerRoleV2, Error, RATIONAL_TERMINAL_HOT_MAGIC_V3,
    RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3, RationalTerminalHotRequestV3, RepresentationActionV2,
    RepresentationRequestHeaderV2, RepresentationRequestV2,
};
use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

fn terminal_child<'a>(asset_bytes: &'a [u8]) -> RepresentationRequestV2<'a> {
    RepresentationRequestV2::new(
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
            expected_representation_revision: 4,
            expected_claims_market_revision: 11,
            expected_actor_position_revision: ABSENT_REVISION,
            expected_custody_position_revision: 12,
            expected_custody_replay_revision: 13,
            generation: 14,
            quantity: 2,
            denominator: 10,
            expected_receipt_supply: 0,
            outcome_count: 258,
            selected_outcome: 257,
            asset_count: 1,
        },
        asset_bytes,
    )
    .expect("terminal child")
}

fn asset_bytes() -> [u8; ASSET_BYTES_V2] {
    let mut output = [0_u8; ASSET_BYTES_V2];
    AssetV2 {
        shard_mint: id(20),
        actor_shard_account: id(21),
        structured_custody_account: id(22),
        claims_custody_owner: id(23),
        coefficient: 1,
        expected_shard_supply: 100,
        expected_actor_shards: 30,
        expected_structured_shards: 0,
    }
    .encode_into(&mut output)
    .expect("asset");
    output
}

#[test]
fn terminal_hot_specializes_exact_child_without_fixed_point() {
    let asset = asset_bytes();
    let child = terminal_child(&asset);
    let mut family_bytes = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
    let family = RationalTerminalHotRequestV3::from_child_into(child, &mut family_bytes)
        .expect("family request");
    assert_eq!(&family.as_bytes()[..8], &RATIONAL_TERMINAL_HOT_MAGIC_V3);
    assert_eq!(&family.as_bytes()[144..176], &[0_u8; 32]);

    let family_digest = id(91);
    let mut child_bytes = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
    let specialized = family
        .specialize_child_into(family_digest, &mut child_bytes)
        .expect("specialized child");
    assert_eq!(specialized.header().parent_context, family_digest);
    assert_eq!(specialized.header().outcome_count, 258);
    assert_eq!(specialized.header().selected_outcome, 257);
    assert_eq!(specialized.asset(0).expect("asset").shard_mint, id(20));
}

#[test]
fn family_refuses_noncanonical_parent_and_substitutions() {
    let asset = asset_bytes();
    let child = terminal_child(&asset);
    let mut bytes = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
    RationalTerminalHotRequestV3::from_child_into(child, &mut bytes).expect("family request");

    bytes[144] = 1;
    assert_eq!(
        RationalTerminalHotRequestV3::decode(&bytes),
        Err(Error::NonCanonical)
    );
    bytes[144] = 0;
    bytes[10] = 4;
    assert_eq!(
        RationalTerminalHotRequestV3::decode(&bytes),
        Err(Error::InvalidActionShape)
    );
    bytes[10] = 5;
    bytes[0] ^= 1;
    assert_eq!(
        RationalTerminalHotRequestV3::decode(&bytes),
        Err(Error::InvalidMagic)
    );
}

#[test]
fn specialization_refuses_zero_digest_and_wrong_width() {
    let asset = asset_bytes();
    let child = terminal_child(&asset);
    let mut bytes = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
    let family =
        RationalTerminalHotRequestV3::from_child_into(child, &mut bytes).expect("family request");
    let mut output = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
    assert_eq!(
        family.specialize_child_into([0; 32], &mut output),
        Err(Error::ZeroIdentity)
    );
    assert_eq!(
        family.specialize_child_into(id(1), &mut output[..647]),
        Err(Error::InvalidLength)
    );
}
