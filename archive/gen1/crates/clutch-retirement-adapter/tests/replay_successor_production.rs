use clutch_retirement::{
    admit_deletable_rent, admit_reopen_rent_split, plan_position_replay_retirement,
    plan_position_replay_retirement_v2, reopen_position_with_replay,
    AdapterNeutralSinkBindingProjectionV1,
    AdapterPositionAccountProjectionV1, AdapterReplayAbsenceProjectionV1,
    AdapterReplayAccountProjectionV1, DeletableRentOwnerV1, Identity32V1, PositionEconomicStateV1,
    PositionLifecycleStateV2, PositionReplayAccountsV1, PositionReplayReopenAccountsV1,
    PositionReplayReopenRequestV1, PositionReplayRetirementPlanV1,
    PositionReplayRetirementRequestV1, PositionReplayRetirementRequestV2,
    PositionRetirementTailV1, PositionTombstoneV1,
    RecipientBalanceBookV1, RecipientBalanceV1, RentSplitV2, ReplayLifecycleStateV1,
    RetirementErrorV2, PROJECTED_REPLAY_SUCCESSOR_BYTES,
};
use clutch_retirement_adapter::{
    authenticate_position_v2_exact, authenticate_replay_absence_v1_exact,
    authenticate_replay_successor_v1_exact, project_authenticated_position_v2,
    project_authenticated_replay_successor_v1, AbsentAccountViewV1, AccountAccessV2, AccountViewV2,
    CanonicalPdaV1, PositionAccountV2, ReplaySuccessorAccountV1, RetirementAdapterErrorV2,
};
use clutch_solana_layout::{Hash32, PositionAccount, MAX_OUTCOMES};
use clutch_solana_reference::{Error as ReferenceError, ReplayAccount};

const POSITION_ACCOUNT: u8 = 40;
const PRIOR_REPLAY_ACCOUNT: u8 = 41;
const NEXT_REPLAY_ACCOUNT: u8 = 42;
const PROGRAM_ID: u8 = 100;
const NEUTRAL_SINK: u8 = 250;

const REPLAY_MARKET_OFFSET: usize = 2;
const REPLAY_OWNER_OFFSET: usize = REPLAY_MARKET_OFFSET + 32;
const REPLAY_GENERATION_OFFSET: usize = REPLAY_OWNER_OFFSET + 32;
const REPLAY_SEQUENCE_OFFSET: usize = REPLAY_GENERATION_OFFSET + 8;
const REPLAY_STORED_BUMP_OFFSET: usize = REPLAY_SEQUENCE_OFFSET + 8;
const REPLAY_RENT_OFFSET: usize = 84;
const REPLAY_PAYER_OFFSET: usize = REPLAY_RENT_OFFSET;
const REPLAY_PRINCIPAL_OFFSET: usize = REPLAY_PAYER_OFFSET + 32;
const REPLAY_DONATION_OFFSET: usize = REPLAY_PRINCIPAL_OFFSET + 8;

fn id(byte: u8) -> Identity32V1 {
    Identity32V1::new([byte; 32]).unwrap()
}

fn position_v2() -> PositionAccountV2 {
    PositionAccountV2 {
        base: PositionAccount {
            market: Hash32::from_bytes([1; 32]),
            owner: Hash32::from_bytes([2; 32]),
            generation: 7,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: 0,
            reserved_cash_atoms: 0,
            stored_bump: 9,
            close_state: 0,
        },
        retirement: PositionRetirementTailV1 {
            outstanding_reservations: 0,
            rent: RentSplitV2 {
                payer: id(70),
                refundable_live_principal: 11,
                permanent_tombstone_principal: 7,
                donation_floor: 5,
            },
        },
    }
}

fn replay_successor_v1() -> ReplaySuccessorAccountV1 {
    ReplaySuccessorAccountV1 {
        base: ReplayAccount {
            market: Hash32::from_bytes([1; 32]),
            owner: Hash32::from_bytes([2; 32]),
            position_generation: 7,
            sequence: 11,
            stored_bump: 18,
            flags: 0,
        },
        rent: DeletableRentOwnerV1::from_persisted(id(71), 13, 5).unwrap(),
    }
}

fn writable_view<'a>(address: Identity32V1, data: &'a [u8]) -> AccountViewV2<'a> {
    AccountViewV2 {
        address,
        owner: id(PROGRAM_ID),
        data,
        is_writable: true,
        is_executable: false,
    }
}

fn project_position(
    bytes: &[u8],
) -> Result<(PositionLifecycleStateV2, PositionEconomicStateV1), RetirementAdapterErrorV2> {
    let address = id(POSITION_ACCOUNT);
    let authenticated = authenticate_position_v2_exact(
        writable_view(address, bytes),
        id(PROGRAM_ID),
        CanonicalPdaV1::after_derivation(address, 9),
        AccountAccessV2::Writable,
    )?;
    project_authenticated_position_v2(authenticated)
}

fn project_replay(bytes: &[u8]) -> Result<ReplayLifecycleStateV1, RetirementAdapterErrorV2> {
    let address = id(PRIOR_REPLAY_ACCOUNT);
    let authenticated = authenticate_replay_successor_v1_exact(
        writable_view(address, bytes),
        id(PROGRAM_ID),
        CanonicalPdaV1::after_derivation(address, 18),
        AccountAccessV2::Writable,
    )?;
    project_authenticated_replay_successor_v1(authenticated)
}

fn recipients() -> RecipientBalanceBookV1 {
    RecipientBalanceBookV1 {
        entries: [
            Some(RecipientBalanceV1 {
                recipient: id(70),
                balance_before: 100,
            }),
            Some(RecipientBalanceV1 {
                recipient: id(71),
                balance_before: 200,
            }),
            Some(RecipientBalanceV1 {
                recipient: id(NEUTRAL_SINK),
                balance_before: 300,
            }),
            None,
        ],
    }
}

fn close_request(
    position: PositionLifecycleStateV2,
    economic: PositionEconomicStateV1,
    replay: ReplayLifecycleStateV1,
) -> PositionReplayRetirementRequestV1 {
    PositionReplayRetirementRequestV1 {
        position,
        replay,
        economic,
        position_balance: 29,
        replay_balance: 20,
        neutral_sink: id(NEUTRAL_SINK),
        neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1 {
            market: id(1),
            neutral_sink: id(NEUTRAL_SINK),
        },
        accounts: PositionReplayAccountsV1 {
            position: AdapterPositionAccountProjectionV1 {
                account: id(POSITION_ACCOUNT),
                market: id(1),
                owner: id(2),
            },
            replay: AdapterReplayAccountProjectionV1 {
                account: id(PRIOR_REPLAY_ACCOUNT),
                market: id(1),
                owner: id(2),
                position_generation: 7,
            },
        },
        recipient_balances: recipients(),
    }
}

fn authenticated_close() -> PositionReplayRetirementPlanV1 {
    let position_bytes = position_v2().encode().unwrap();
    let replay_bytes = replay_successor_v1().encode().unwrap();
    let (position, economic) = project_position(&position_bytes).unwrap();
    let replay = project_replay(&replay_bytes).unwrap();
    plan_position_replay_retirement(close_request(position, economic, replay)).unwrap()
}

fn authenticated_absence(
    market: Identity32V1,
    owner: Identity32V1,
    position_generation: u64,
) -> AdapterReplayAbsenceProjectionV1 {
    let address = id(PRIOR_REPLAY_ACCOUNT);
    authenticate_replay_absence_v1_exact(
        AbsentAccountViewV1 {
            address,
            owner: [0; 32],
            data_len: 0,
            is_writable: false,
            is_executable: false,
        },
        CanonicalPdaV1::after_derivation(address, 18),
        market,
        owner,
        position_generation,
    )
    .unwrap()
}

fn reopen_request(position: PositionLifecycleStateV2) -> PositionReplayReopenRequestV1 {
    let tombstone = match position {
        PositionLifecycleStateV2::Tombstone(tombstone) => tombstone,
        PositionLifecycleStateV2::Live(_) => panic!("reopen fixture requires a tombstone"),
    };
    PositionReplayReopenRequestV1 {
        position,
        prior_replay: authenticated_absence(
            tombstone.market,
            tombstone.owner,
            tombstone.generation,
        ),
        position_funding: admit_reopen_rent_split(
            id(POSITION_ACCOUNT),
            id(70),
            11,
            7,
            7,
            1_000,
            id(NEUTRAL_SINK),
        )
        .unwrap(),
        replay_stored_bump: 19,
        replay_funding: admit_deletable_rent(
            id(NEXT_REPLAY_ACCOUNT),
            id(71),
            13,
            0,
            1_000,
            id(NEUTRAL_SINK),
        )
        .unwrap(),
        neutral_sink: id(NEUTRAL_SINK),
        neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1 {
            market: tombstone.market,
            neutral_sink: id(NEUTRAL_SINK),
        },
        accounts: PositionReplayReopenAccountsV1 {
            position: AdapterPositionAccountProjectionV1 {
                account: id(POSITION_ACCOUNT),
                market: tombstone.market,
                owner: tombstone.owner,
            },
            next_replay: AdapterReplayAccountProjectionV1 {
                account: id(NEXT_REPLAY_ACCOUNT),
                market: tombstone.market,
                owner: tombstone.owner,
                position_generation: tombstone.generation.saturating_add(1),
            },
        },
    }
}

#[test]
fn replay_absence_authentication_is_exact_and_accepts_founding_generation_zero() {
    let address = id(PRIOR_REPLAY_ACCOUNT);
    let canonical = CanonicalPdaV1::after_derivation(address, 18);
    let baseline = AbsentAccountViewV1 {
        address,
        owner: [0; 32],
        data_len: 0,
        is_writable: false,
        is_executable: false,
    };
    let founding =
        authenticate_replay_absence_v1_exact(baseline, canonical, id(1), id(2), 0).unwrap();
    assert_eq!(founding.account, address);
    assert_eq!(founding.market, id(1));
    assert_eq!(founding.owner, id(2));
    assert_eq!(founding.position_generation, 0);

    assert_eq!(
        authenticate_replay_absence_v1_exact(
            baseline,
            CanonicalPdaV1::after_derivation(id(99), 18),
            id(1),
            id(2),
            0,
        ),
        Err(RetirementAdapterErrorV2::WrongPda)
    );

    let mut hostile = baseline;
    hostile.is_writable = true;
    assert_eq!(
        authenticate_replay_absence_v1_exact(hostile, canonical, id(1), id(2), 0),
        Err(RetirementAdapterErrorV2::UnexpectedWritable)
    );

    hostile = baseline;
    hostile.is_executable = true;
    assert_eq!(
        authenticate_replay_absence_v1_exact(hostile, canonical, id(1), id(2), 0),
        Err(RetirementAdapterErrorV2::AccountNotAbsent)
    );

    hostile = baseline;
    hostile.owner = [9; 32];
    assert_eq!(
        authenticate_replay_absence_v1_exact(hostile, canonical, id(1), id(2), 0),
        Err(RetirementAdapterErrorV2::AccountNotAbsent)
    );

    hostile = baseline;
    hostile.data_len = 1;
    assert_eq!(
        authenticate_replay_absence_v1_exact(hostile, canonical, id(1), id(2), 0),
        Err(RetirementAdapterErrorV2::AccountNotAbsent)
    );
}

#[test]
fn exact_authenticated_projection_reconstructs_the_identical_successor_image() {
    let value = replay_successor_v1();
    let bytes = value.encode().unwrap();
    assert_eq!(bytes.len(), PROJECTED_REPLAY_SUCCESSOR_BYTES);

    let projected = project_replay(&bytes).unwrap();
    let ReplayLifecycleStateV1::Live(projected) = projected else {
        panic!("an authenticated successor projected as absent")
    };
    let reconstructed = ReplaySuccessorAccountV1 {
        base: ReplayAccount {
            market: Hash32::from_bytes(projected.market.bytes()),
            owner: Hash32::from_bytes(projected.owner.bytes()),
            position_generation: projected.position_generation,
            sequence: projected.sequence,
            stored_bump: projected.stored_bump,
            flags: 0,
        },
        rent: projected.rent,
    };
    assert_eq!(reconstructed, value);
    assert_eq!(reconstructed.encode().unwrap(), bytes);
    assert_eq!(ReplaySuccessorAccountV1::decode(&bytes), Ok(value));
}

#[test]
fn hostile_successor_identity_and_funding_bytes_refuse_before_value_can_move() {
    let original = replay_successor_v1().encode().unwrap();

    let mut zero_market = original;
    zero_market[REPLAY_MARKET_OFFSET..REPLAY_OWNER_OFFSET].fill(0);
    assert_eq!(
        project_replay(&zero_market),
        Err(RetirementAdapterErrorV2::ReferenceCodec(
            ReferenceError::NonCanonical
        ))
    );

    let mut zero_owner = original;
    zero_owner[REPLAY_OWNER_OFFSET..REPLAY_GENERATION_OFFSET].fill(0);
    assert_eq!(
        project_replay(&zero_owner),
        Err(RetirementAdapterErrorV2::ReferenceCodec(
            ReferenceError::NonCanonical
        ))
    );

    let mut zero_payer = original;
    zero_payer[REPLAY_PAYER_OFFSET..REPLAY_PRINCIPAL_OFFSET].fill(0);
    assert_eq!(
        project_replay(&zero_payer),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::ZeroIdentity
        ))
    );

    let mut zero_principal = original;
    zero_principal[REPLAY_PRINCIPAL_OFFSET..REPLAY_DONATION_OFFSET].fill(0);
    assert_eq!(
        project_replay(&zero_principal),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::NonCanonicalState
        ))
    );

    let mut overflowing_donation = original;
    overflowing_donation[REPLAY_DONATION_OFFSET..].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        project_replay(&overflowing_donation),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::ArithmeticOverflow
        ))
    );

    assert_eq!(
        project_replay(&original[..original.len() - 1]),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::Truncated
        ))
    );
    let mut trailing = original.to_vec();
    trailing.push(0);
    assert_eq!(
        project_replay(&trailing),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::TrailingBytes
        ))
    );
}

#[test]
fn authenticated_close_cross_binds_identity_generation_payer_and_balance_geometry() {
    let position_bytes = position_v2().encode().unwrap();
    let replay_bytes = replay_successor_v1().encode().unwrap();
    let (position, economic) = project_position(&position_bytes).unwrap();

    let mut wrong_market = replay_bytes;
    wrong_market[REPLAY_MARKET_OFFSET..REPLAY_OWNER_OFFSET].copy_from_slice(&id(9).bytes());
    let replay = project_replay(&wrong_market).unwrap();
    assert_eq!(
        plan_position_replay_retirement(close_request(position, economic, replay)),
        Err(RetirementErrorV2::ReplayMismatch)
    );

    let mut wrong_owner = replay_bytes;
    wrong_owner[REPLAY_OWNER_OFFSET..REPLAY_GENERATION_OFFSET].copy_from_slice(&id(9).bytes());
    let replay = project_replay(&wrong_owner).unwrap();
    assert_eq!(
        plan_position_replay_retirement(close_request(position, economic, replay)),
        Err(RetirementErrorV2::ReplayMismatch)
    );

    let mut wrong_generation = replay_bytes;
    wrong_generation[REPLAY_GENERATION_OFFSET..REPLAY_SEQUENCE_OFFSET]
        .copy_from_slice(&8u64.to_le_bytes());
    let replay = project_replay(&wrong_generation).unwrap();
    assert_eq!(
        plan_position_replay_retirement(close_request(position, economic, replay)),
        Err(RetirementErrorV2::ReplayMismatch)
    );

    let mut sink_as_payer = replay_bytes;
    sink_as_payer[REPLAY_PAYER_OFFSET..REPLAY_PRINCIPAL_OFFSET]
        .copy_from_slice(&id(NEUTRAL_SINK).bytes());
    let replay = project_replay(&sink_as_payer).unwrap();
    assert_eq!(
        plan_position_replay_retirement(close_request(position, economic, replay)),
        Err(RetirementErrorV2::PayerIsNeutralSink)
    );

    let mut excessive_principal = replay_bytes;
    excessive_principal[REPLAY_PRINCIPAL_OFFSET..REPLAY_DONATION_OFFSET]
        .copy_from_slice(&21u64.to_le_bytes());
    let replay = project_replay(&excessive_principal).unwrap();
    assert_eq!(
        plan_position_replay_retirement(close_request(position, economic, replay)),
        Err(RetirementErrorV2::AccountBalanceShortfall)
    );

    let mut excessive_donation = replay_bytes;
    excessive_donation[REPLAY_DONATION_OFFSET..].copy_from_slice(&19u64.to_le_bytes());
    let replay = project_replay(&excessive_donation).unwrap();
    assert_eq!(
        plan_position_replay_retirement(close_request(position, economic, replay)),
        Err(RetirementErrorV2::AccountBalanceShortfall)
    );
}

#[test]
fn production_close_entry_point_binds_the_signed_replay_sequence() {
    let position_bytes = position_v2().encode().unwrap();
    let mut replay_bytes = replay_successor_v1().encode().unwrap();
    let (position, economic) = project_position(&position_bytes).unwrap();

    replay_bytes[REPLAY_SEQUENCE_OFFSET..REPLAY_STORED_BUMP_OFFSET]
        .copy_from_slice(&12u64.to_le_bytes());
    let replay = project_replay(&replay_bytes).unwrap();
    let ReplayLifecycleStateV1::Live(live) = replay else {
        panic!("an authenticated successor projected as absent")
    };
    assert_eq!(live.sequence, 12);

    let retirement = close_request(position, economic, ReplayLifecycleStateV1::Live(live));
    assert!(plan_position_replay_retirement(retirement).is_ok());
    assert_eq!(
        plan_position_replay_retirement_v2(PositionReplayRetirementRequestV2 {
            retirement,
            signed_sequence: 11,
        }),
        Err(RetirementErrorV2::ReplayMismatch)
    );
    assert!(plan_position_replay_retirement_v2(PositionReplayRetirementRequestV2 {
        retirement,
        signed_sequence: 12,
    })
    .is_ok());
}

#[test]
fn exact_absence_projection_reopens_once_at_the_next_generation_and_sequence_zero() {
    let closed = authenticated_close();
    assert_eq!(closed.replay_post_state, ReplayLifecycleStateV1::Absent);

    let request = reopen_request(closed.position_post_state);
    let reopened = reopen_position_with_replay(request).unwrap();
    let PositionLifecycleStateV2::Live(position) = reopened.position_post_state else {
        panic!("Position remained tombstoned")
    };
    let ReplayLifecycleStateV1::Live(replay) = reopened.replay_post_state else {
        panic!("next-generation Replay was not constructed")
    };
    assert_eq!(position.generation, 8);
    assert_eq!(replay.position_generation, 8);
    assert_eq!(replay.sequence, 0);
    assert_eq!(replay.stored_bump, 19);

    let mut wrong_prior_identity = request;
    wrong_prior_identity.prior_replay.owner = id(9);
    assert_eq!(
        reopen_position_with_replay(wrong_prior_identity),
        Err(RetirementErrorV2::ReplayMismatch)
    );

    let mut wrong_prior_generation = request;
    wrong_prior_generation.prior_replay.position_generation = 6;
    assert_eq!(
        reopen_position_with_replay(wrong_prior_generation),
        Err(RetirementErrorV2::ReplayMismatch)
    );

    let live_request = PositionReplayReopenRequestV1 {
        position: reopened.position_post_state,
        ..request
    };
    assert_eq!(
        reopen_position_with_replay(live_request),
        Err(RetirementErrorV2::WrongPhase)
    );

    let exhausted = PositionLifecycleStateV2::Tombstone(PositionTombstoneV1 {
        market: id(1),
        owner: id(2),
        generation: u64::MAX,
        stored_bump: 9,
    });
    let exhausted_request = reopen_request(exhausted);
    assert_eq!(
        reopen_position_with_replay(exhausted_request),
        Err(RetirementErrorV2::ArithmeticOverflow)
    );
}
