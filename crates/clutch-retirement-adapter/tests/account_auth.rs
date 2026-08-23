mod common;

use clutch_retirement::{GeneralEpochTombstoneV1, PositionTombstoneV1, RetirementErrorV1};
use clutch_retirement_adapter::{
    authenticate_counted_child, authenticate_direct_reservation_v6,
    authenticate_general_epoch_tombstone_v1, authenticate_general_epoch_v5,
    authenticate_general_reservation_v5, authenticate_market_v2,
    authenticate_position_tombstone_v1, authenticate_position_v2, AccountViewV1, CanonicalPdaV1,
    CountedChildSchemaV1, RetirementAdapterErrorV1,
};

fn view<'a>(data: &'a [u8], bump: u8) -> (AccountViewV1<'a>, CanonicalPdaV1) {
    let address = common::id(101);
    (
        AccountViewV1 {
            address,
            owner: common::id(100),
            data,
            is_writable: true,
        },
        CanonicalPdaV1::after_derivation(address, bump),
    )
}

#[test]
fn every_promoted_root_family_authenticates_owner_pda_length_header_and_bump() {
    let position = common::position_v2().encode().unwrap();
    let (position_view, position_pda) = view(&position, 9);
    assert!(authenticate_position_v2(position_view, common::id(100), position_pda).is_ok());

    let market = common::market_v2().encode().unwrap();
    let (market_view, market_pda) = view(&market, 10);
    assert!(authenticate_market_v2(market_view, common::id(100), market_pda).is_ok());

    let epoch = common::epoch_v5().encode().unwrap();
    let (epoch_view, epoch_pda) = view(&epoch, 12);
    assert!(authenticate_general_epoch_v5(epoch_view, common::id(100), epoch_pda).is_ok());

    let general = common::general_reservation_v5().encode().unwrap();
    let (general_view, general_pda) = view(&general, 13);
    assert!(
        authenticate_general_reservation_v5(general_view, common::id(100), general_pda).is_ok()
    );

    let direct = common::direct_reservation_v6()
        .encode(common::direct_sink())
        .unwrap();
    let (direct_view, direct_pda) = view(&direct, 14);
    assert!(authenticate_direct_reservation_v6(direct_view, common::id(100), direct_pda).is_ok());

    let position_tombstone = PositionTombstoneV1 {
        market: common::id(1),
        owner: common::id(2),
        generation: 7,
        stored_bump: 15,
    }
    .encode()
    .unwrap();
    let (position_tombstone_view, position_tombstone_pda) = view(&position_tombstone, 15);
    assert!(authenticate_position_tombstone_v1(
        position_tombstone_view,
        common::id(100),
        position_tombstone_pda
    )
    .is_ok());

    let epoch_tombstone = GeneralEpochTombstoneV1 {
        epoch: common::id(3),
        market: common::id(1),
        epoch_index: 7,
        epoch_generation: 8,
        stored_bump: 16,
    }
    .encode()
    .unwrap();
    let (epoch_tombstone_view, epoch_tombstone_pda) = view(&epoch_tombstone, 16);
    assert!(authenticate_general_epoch_tombstone_v1(
        epoch_tombstone_view,
        common::id(100),
        epoch_tombstone_pda
    )
    .is_ok());
}

#[test]
fn metadata_and_header_failures_refuse_before_semantic_decode() {
    let position = common::position_v2().encode().unwrap();
    let (good, pda) = view(&position, 9);

    let mut wrong = good;
    wrong.address = common::id(102);
    assert_eq!(
        authenticate_position_v2(wrong, common::id(100), pda),
        Err(RetirementAdapterErrorV1::WrongPda)
    );
    wrong = good;
    wrong.owner = common::id(103);
    assert_eq!(
        authenticate_position_v2(wrong, common::id(100), pda),
        Err(RetirementAdapterErrorV1::WrongOwner)
    );
    wrong = good;
    wrong.is_writable = false;
    assert_eq!(
        authenticate_position_v2(wrong, common::id(100), pda),
        Err(RetirementAdapterErrorV1::NotWritable)
    );
    assert_eq!(
        authenticate_position_v2(
            good,
            common::id(100),
            CanonicalPdaV1::after_derivation(good.address, 8),
        ),
        Err(RetirementAdapterErrorV1::WrongBump)
    );

    let mut hostile = position;
    hostile[0] = hostile[0].wrapping_add(1);
    let (hostile_view, hostile_pda) = view(&hostile, 9);
    assert_eq!(
        authenticate_position_v2(hostile_view, common::id(100), hostile_pda),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::WrongTag
        ))
    );
    let (short, short_pda) = view(&position[..position.len() - 1], 9);
    assert_eq!(
        authenticate_position_v2(short, common::id(100), short_pda),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::Truncated
        ))
    );
}

#[test]
fn registry_supplied_child_geometry_gets_the_same_metadata_checks() {
    let schema = CountedChildSchemaV1::after_registry_allocation(33, 4, 5, 12, 10).unwrap();
    let mut data = [0u8; 20];
    data[0] = 33;
    data[1] = 5;
    data[10] = 17;
    data[12..].copy_from_slice(&8u64.to_le_bytes());
    let (account, pda) = view(&data, 17);
    assert!(authenticate_counted_child(account, common::id(100), pda, schema).is_ok());

    assert_eq!(
        CountedChildSchemaV1::after_registry_allocation(33, 4, 4, 12, 10),
        Err(RetirementAdapterErrorV1::InvalidSchema)
    );
    assert_eq!(
        CountedChildSchemaV1::after_registry_allocation(33, 4, 5, 12, 12),
        Err(RetirementAdapterErrorV1::InvalidSchema)
    );
}
