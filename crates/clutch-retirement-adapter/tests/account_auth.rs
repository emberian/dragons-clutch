mod common;

use clutch_retirement::{
    DirectEpochLifecyclePhaseV1, GeneralEpochTombstoneV1, PositionTombstoneV1, RetirementErrorV1,
    RetirementErrorV2, GENERAL_EPOCH_TOMBSTONE_TAG, GENERAL_EPOCH_TOMBSTONE_VERSION_V1,
    POSITION_TOMBSTONE_TAG, POSITION_TOMBSTONE_VERSION_V1, PROJECTED_REPLAY_SUCCESSOR_BYTES,
};
use clutch_retirement_adapter::{
    authenticate_counted_child, authenticate_direct_epoch_v4, authenticate_direct_reservation_v6,
    authenticate_direct_reservation_v8, authenticate_general_epoch_tombstone_v1,
    authenticate_general_epoch_v5, authenticate_general_epoch_v5_exact,
    authenticate_general_indexed_settlement_root_v1_exact,
    authenticate_general_reservation_v5, authenticate_general_reservation_v7,
    authenticate_market_v2, authenticate_market_v2_exact, authenticate_position_tombstone_v1,
    authenticate_position_v2, authenticate_position_v2_exact,
    authenticate_replay_successor_v1_exact, authenticate_runtime_executable_v2,
    project_authenticated_direct_epoch_v4, project_authenticated_position_v2,
    project_authenticated_replay_successor_v1, AccountAccessV2, AccountViewV1, AccountViewV2,
    CanonicalPdaV1, CountedChildSchemaV1, RetirementAdapterErrorV1, RetirementAdapterErrorV2,
};
use clutch_general_v2_contract::{
    INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION, INDEXED_SETTLEMENT_ROOT_BYTES_V1,
    SETTLEMENT_ROOT_ACCOUNT_BYTES, SETTLEMENT_ROOT_ACCOUNT_TAG,
};
use clutch_solana_layout::direct_selection_v3::DIRECT_EPOCH_V4_BYTES;
use clutch_solana_layout::registry::{
    AllocationCoordinates, AllocationStatus, WireNamespace, CENTRAL_COLLISION_LEDGER,
    REPLAY_SUCCESSOR_ACCOUNT_TAG, REPLAY_SUCCESSOR_ACCOUNT_VERSION,
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

fn view_v2<'a>(data: &'a [u8], bump: u8) -> (AccountViewV2<'a>, CanonicalPdaV1) {
    let address = common::id(101);
    (
        AccountViewV2 {
            address,
            owner: common::id(100),
            data,
            is_writable: true,
            is_executable: false,
        },
        CanonicalPdaV1::after_derivation(address, bump),
    )
}

#[test]
fn successor_authentication_is_exact_about_access_and_executable_state() {
    let position = common::position_v2().encode().unwrap();
    let (position_view, position_pda) = view_v2(&position, 9);
    let authenticated = authenticate_position_v2_exact(
        position_view,
        common::id(100),
        position_pda,
        AccountAccessV2::Writable,
    )
    .unwrap();
    assert!(project_authenticated_position_v2(authenticated).is_ok());

    let mut read_only = position_view;
    read_only.is_writable = false;
    assert!(authenticate_position_v2_exact(
        read_only,
        common::id(100),
        position_pda,
        AccountAccessV2::ReadOnly,
    )
    .is_ok());
    assert_eq!(
        authenticate_position_v2_exact(
            position_view,
            common::id(100),
            position_pda,
            AccountAccessV2::ReadOnly,
        ),
        Err(RetirementAdapterErrorV2::UnexpectedWritable)
    );
    assert_eq!(
        authenticate_position_v2_exact(
            read_only,
            common::id(100),
            position_pda,
            AccountAccessV2::Writable,
        ),
        Err(RetirementAdapterErrorV2::NotWritable)
    );
    let mut executable = position_view;
    executable.is_executable = true;
    assert_eq!(
        authenticate_position_v2_exact(
            executable,
            common::id(100),
            position_pda,
            AccountAccessV2::Writable,
        ),
        Err(RetirementAdapterErrorV2::ExecutableAccount)
    );

    let market = common::market_v2().encode().unwrap();
    let (mut market_view, market_pda) = view_v2(&market, 10);
    market_view.is_writable = false;
    assert!(authenticate_market_v2_exact(
        market_view,
        common::id(100),
        market_pda,
        AccountAccessV2::ReadOnly,
    )
    .is_ok());

    let epoch = common::epoch_v5().encode().unwrap();
    let (epoch_view, epoch_pda) = view_v2(&epoch, 12);
    assert!(authenticate_general_epoch_v5_exact(
        epoch_view,
        common::id(100),
        epoch_pda,
        AccountAccessV2::Writable,
    )
    .is_ok());

    let replay = common::replay_successor_v1().encode().unwrap();
    let (replay_view, replay_pda) = view_v2(&replay, 18);
    let authenticated = authenticate_replay_successor_v1_exact(
        replay_view,
        common::id(100),
        replay_pda,
        AccountAccessV2::Writable,
    )
    .unwrap();
    assert!(project_authenticated_replay_successor_v1(authenticated).is_ok());

    let mut program = AccountViewV2 {
        address: common::id(110),
        owner: common::id(111),
        data: &[],
        is_writable: false,
        is_executable: true,
    };
    assert!(authenticate_runtime_executable_v2(program, common::id(110)).is_ok());
    program.is_executable = false;
    assert_eq!(
        authenticate_runtime_executable_v2(program, common::id(110)),
        Err(RetirementAdapterErrorV2::NotExecutable)
    );
    program.is_executable = true;
    program.is_writable = true;
    assert_eq!(
        authenticate_runtime_executable_v2(program, common::id(110)),
        Err(RetirementAdapterErrorV2::UnexpectedWritable)
    );
    assert_eq!(
        authenticate_runtime_executable_v2(program, common::id(112)),
        Err(RetirementAdapterErrorV2::WrongProgramAddress)
    );
}

#[test]
fn indexed_settlement_root_authentication_refuses_legacy_geometry() {
    let mut indexed = [0u8; INDEXED_SETTLEMENT_ROOT_BYTES_V1];
    indexed[0] = SETTLEMENT_ROOT_ACCOUNT_TAG;
    indexed[1] = INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION;
    let bump_offset = 16 + SETTLEMENT_ROOT_ACCOUNT_BYTES - 4;
    indexed[bump_offset] = 19;
    let (indexed_view, indexed_pda) = view_v2(&indexed, 19);
    assert!(authenticate_general_indexed_settlement_root_v1_exact(
        indexed_view,
        common::id(100),
        indexed_pda,
        AccountAccessV2::Writable,
    )
    .is_ok());

    let mut legacy_version = indexed;
    legacy_version[1] = 1;
    let (legacy_view, legacy_pda) = view_v2(&legacy_version, 19);
    assert_eq!(
        authenticate_general_indexed_settlement_root_v1_exact(
            legacy_view,
            common::id(100),
            legacy_pda,
            AccountAccessV2::Writable,
        ),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::WrongVersion,
        )),
    );

    let (short_view, short_pda) = view_v2(&indexed[..SETTLEMENT_ROOT_ACCOUNT_BYTES], 19);
    assert_eq!(
        authenticate_general_indexed_settlement_root_v1_exact(
            short_view,
            common::id(100),
            short_pda,
            AccountAccessV2::Writable,
        ),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::Truncated,
        )),
    );

    assert_eq!(
        authenticate_general_indexed_settlement_root_v1_exact(
            indexed_view,
            common::id(100),
            CanonicalPdaV1::after_derivation(indexed_view.address, 18),
            AccountAccessV2::Writable,
        ),
        Err(RetirementAdapterErrorV2::WrongBump),
    );
}

#[test]
fn retirement_coordinates_match_central_reserved_disabled_registry_entries() {
    let expected = [
        (
            POSITION_TOMBSTONE_TAG,
            POSITION_TOMBSTONE_VERSION_V1,
            "retirement-provisional-position-tombstone-v1-account",
            AllocationStatus::ReservedDisabled,
        ),
        (
            GENERAL_EPOCH_TOMBSTONE_TAG,
            GENERAL_EPOCH_TOMBSTONE_VERSION_V1,
            "retirement-provisional-general-epoch-tombstone-v1-account",
            AllocationStatus::ReservedDisabled,
        ),
        (
            REPLAY_SUCCESSOR_ACCOUNT_TAG,
            REPLAY_SUCCESSOR_ACCOUNT_VERSION,
            "replay-successor-v1-account",
            AllocationStatus::ReservedDisabled,
        ),
    ];
    for (tag, version, name, status) in expected {
        let mut matches = 0u8;
        for entry in CENTRAL_COLLISION_LEDGER {
            if entry.coordinates
                == (AllocationCoordinates::Exact {
                    namespace: WireNamespace::MainAccount,
                    tag,
                    version,
                })
            {
                matches += 1;
                assert_eq!(entry.status, status);
                assert_eq!(entry.name, name);
            }
        }
        assert_eq!(matches, 1);
    }
    assert_eq!(REPLAY_SUCCESSOR_ACCOUNT_TAG, 0x7a);
    assert_eq!(REPLAY_SUCCESSOR_ACCOUNT_VERSION, 1);
    assert_eq!(PROJECTED_REPLAY_SUCCESSOR_BYTES, 132);
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

    let direct_epoch = common::direct_epoch_v4(7);
    let mut direct_epoch_bytes = [0u8; DIRECT_EPOCH_V4_BYTES];
    assert_eq!(
        direct_epoch.encode(&mut direct_epoch_bytes),
        Ok(DIRECT_EPOCH_V4_BYTES)
    );
    let (direct_epoch_view, direct_epoch_pda) = view(&direct_epoch_bytes, 17);
    assert!(
        authenticate_direct_epoch_v4(direct_epoch_view, common::id(100), direct_epoch_pda).is_ok()
    );

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

    let general = common::general_reservation_v7().encode().unwrap();
    let (general_view, general_pda) = view(&general, 13);
    assert!(
        authenticate_general_reservation_v7(general_view, common::id(100), general_pda).is_ok()
    );

    let direct = common::direct_reservation_v8()
        .encode(common::direct_sink())
        .unwrap();
    let (direct_view, direct_pda) = view(&direct, 14);
    assert!(authenticate_direct_reservation_v8(direct_view, common::id(100), direct_pda).is_ok());

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
fn direct_epoch_v4_authentication_projects_canonical_generation_and_persisted_sink() {
    let direct_epoch = common::direct_epoch_v4(7);
    let mut bytes = [0u8; DIRECT_EPOCH_V4_BYTES];
    direct_epoch.encode(&mut bytes).unwrap();
    let (account, pda) = view(&bytes, 17);
    let authenticated = authenticate_direct_epoch_v4(account, common::id(100), pda).unwrap();
    let (parent, sink) = project_authenticated_direct_epoch_v4(authenticated).unwrap();
    assert_eq!(parent.account, common::id(101));
    assert_eq!(parent.market, common::id(1));
    assert_eq!(
        parent.epoch.bytes(),
        direct_epoch.direct.common.epoch.bytes()
    );
    assert_eq!(parent.epoch_index, 7);
    assert_eq!(
        parent.lifecycle_phase,
        DirectEpochLifecyclePhaseV1::PrefreezeOpen
    );
    assert_eq!(parent.reservation_generation(), Ok(8));
    assert_eq!(sink.market, common::id(1));
    assert_eq!(sink.neutral_sink, common::id(90));
    let mut read_only = account;
    read_only.is_writable = false;
    assert!(authenticate_direct_epoch_v4(read_only, common::id(100), pda).is_ok());
    assert_eq!(
        authenticate_direct_epoch_v4(
            account,
            common::id(100),
            CanonicalPdaV1::after_derivation(account.address, 16),
        ),
        Err(RetirementAdapterErrorV2::WrongBump)
    );

    let lifecycle_cases = [
        (
            common::direct_epoch_v4_frozen_empty(7),
            DirectEpochLifecyclePhaseV1::FrozenEmpty,
        ),
        (
            common::direct_epoch_v4_selected(7),
            DirectEpochLifecyclePhaseV1::Selected,
        ),
        (
            common::direct_epoch_v4_prefreeze_aborted(7),
            DirectEpochLifecyclePhaseV1::Terminal,
        ),
        (
            common::direct_epoch_v4_settled(7),
            DirectEpochLifecyclePhaseV1::Terminal,
        ),
    ];
    for (direct_epoch, expected_phase) in lifecycle_cases {
        direct_epoch.encode(&mut bytes).unwrap();
        let (account, pda) = view(&bytes, 17);
        let authenticated = authenticate_direct_epoch_v4(account, common::id(100), pda).unwrap();
        let (parent, _) = project_authenticated_direct_epoch_v4(authenticated).unwrap();
        assert_eq!(parent.lifecycle_phase, expected_phase);
    }

    let exhausted = common::direct_epoch_v4(u64::MAX);
    exhausted.encode(&mut bytes).unwrap();
    let (account, pda) = view(&bytes, 17);
    let authenticated = authenticate_direct_epoch_v4(account, common::id(100), pda).unwrap();
    let (parent, _) = project_authenticated_direct_epoch_v4(authenticated).unwrap();
    assert_eq!(
        parent.reservation_generation(),
        Err(RetirementErrorV2::EpochIndexExhausted)
    );

    let mut legacy = bytes;
    legacy[1] = clutch_solana_layout::direct_selection::DIRECT_EPOCH_VERSION;
    let (account, pda) = view(&legacy, 17);
    assert_eq!(
        authenticate_direct_epoch_v4(account, common::id(100), pda),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::WrongVersion
        ))
    );
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
