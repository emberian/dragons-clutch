use clutch_batch_policy_identity::{batch_policy_digest, direct_window_v1::DIRECT_POLICY_V1};
use clutch_retirement::{
    DeletableRentOwnerV1, EpochChildCountsV1, EpochRetirementTailV1, Identity32V1,
    MarketEpochCursorV1, PositionRetirementTailV1, RentSplitV2, ReservationCountTailV1,
    ReservationRetirementTailV2,
};
use clutch_retirement_adapter::{
    DirectReservationAccountV6, DirectReservationAccountV8, GeneralEpochAccountV5,
    GeneralReservationAccountV5, GeneralReservationAccountV7, MarketAccountV2, PositionAccountV2,
};
use clutch_solana_layout::{
    canonical_epoch_id, canonical_order_id, canonical_outcome_id,
    direct_selection::{canonical_direct_remainder_seed, DirectEpochV3Account},
    direct_selection_v3::{
        DirectBatchPolicyV3, DirectEpochV4Account, DirectFundingLedgerV3,
        DirectReservationV2Account, DirectTerminalReceiptV3, DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY,
        DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN, DIRECT_LIFECYCLE_PHASE_SELECTED,
        DIRECT_LIFECYCLE_PHASE_TERMINAL, DIRECT_TERMINAL_REASON_PREFREEZE_ABORT,
        DIRECT_TERMINAL_REASON_SETTLED,
    },
    reservation::{ReservationAccount, ReservationPlan},
    EpochAccount, Hash32, MarketAccount, PositionAccount, EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN,
    EPOCH_PHASE_OPEN, EPOCH_PHASE_SETTLED, MAX_OUTCOMES, ORDER_KIND_SINGLE, RELATION_VERSION,
};

pub fn h(byte: u8) -> Hash32 {
    Hash32::from_bytes([byte; 32])
}

pub fn id(byte: u8) -> Identity32V1 {
    Identity32V1::new([byte; 32]).unwrap()
}

pub fn rent() -> RentSplitV2 {
    RentSplitV2 {
        payer: id(70),
        refundable_live_principal: 11,
        permanent_tombstone_principal: 7,
        donation_floor: 5,
    }
}

pub fn deletable_rent() -> DeletableRentOwnerV1 {
    DeletableRentOwnerV1::from_persisted(id(71), 13, 5).unwrap()
}

pub fn position_v2() -> PositionAccountV2 {
    PositionAccountV2 {
        base: PositionAccount {
            market: h(1),
            owner: h(2),
            generation: 7,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: 0,
            reserved_cash_atoms: 0,
            stored_bump: 9,
            close_state: 0,
        },
        retirement: PositionRetirementTailV1 {
            outstanding_reservations: 0,
            rent: rent(),
        },
    }
}

pub fn market_v2() -> MarketAccountV2 {
    let market = h(1);
    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    outcomes[0] = canonical_outcome_id(market, 0);
    outcomes[1] = canonical_outcome_id(market, 1);
    MarketAccountV2 {
        base: MarketAccount {
            market,
            realm: h(2),
            profile: h(3),
            terms: h(4),
            outcome_count: 2,
            lifecycle: 0,
            stored_bump: 10,
            hoard_bump: 11,
            outcomes,
            feed: h(5),
            collateral_cap: 1_000,
            created_slot: 99,
            reserved: Hash32::ZERO,
        },
        cursor: MarketEpochCursorV1 {
            next_general_epoch_index: 8,
        },
    }
}

pub fn epoch_v5() -> GeneralEpochAccountV5 {
    let market = h(1);
    GeneralEpochAccountV5 {
        base: EpochAccount {
            epoch: canonical_epoch_id(market, 7),
            market,
            book: h(2),
            terms: h(3),
            price_grid: h(4),
            policy: h(5),
            order_set: Hash32::ZERO,
            first_order_id: Hash32::ZERO,
            last_order_id: Hash32::ZERO,
            epoch_index: 7,
            relation_version: RELATION_VERSION,
            price_scale: 10_000,
            remainder_seed: 13,
            owner_count: 1,
            page_count: 0,
            order_count: 0,
            outcome_count: 2,
            basis_degree: 1,
            phase: EPOCH_PHASE_OPEN,
            stored_bump: 12,
            flags: 0,
        },
        retirement: EpochRetirementTailV1 {
            epoch_generation: 8,
            children: EpochChildCountsV1::default(),
            rent: rent(),
        },
    }
}

fn reservation_base(stored_bump: u8) -> ReservationAccount {
    let mut internal = [0u64; MAX_OUTCOMES];
    internal[0] = 4;
    ReservationAccount::active(
        h(1),
        h(2),
        h(3),
        canonical_order_id(1),
        h(4),
        h(5),
        h(6),
        7,
        8,
        0,
        stored_bump,
        ReservationPlan {
            cash_atoms: 0,
            internal,
            max_fee_atoms: 0,
            outcome_count: 2,
            order_kind: ORDER_KIND_SINGLE,
            side: 1,
        },
    )
    .unwrap()
}

pub fn general_reservation_v5() -> GeneralReservationAccountV5 {
    GeneralReservationAccountV5 {
        base: reservation_base(13),
        count: ReservationCountTailV1 {
            epoch_generation: 8,
            position_counted: true,
        },
    }
}

pub fn general_reservation_v7() -> GeneralReservationAccountV7 {
    GeneralReservationAccountV7 {
        base: reservation_base(13),
        retirement: ReservationRetirementTailV2 {
            count: ReservationCountTailV1 {
                epoch_generation: 8,
                position_counted: true,
            },
            rent: deletable_rent(),
        },
    }
}

pub fn direct_sink() -> Hash32 {
    h(90)
}

pub fn direct_epoch_v4(epoch_index: u64) -> DirectEpochV4Account {
    let market = h(1);
    let epoch = canonical_epoch_id(market, epoch_index);
    let verifier_release_id = h(80);
    let direct_policy = DirectBatchPolicyV3::direct(verifier_release_id).unwrap();
    let relation_policy = Hash32::from_bytes(batch_policy_digest(&DIRECT_POLICY_V1).unwrap().0);
    let common = EpochAccount {
        epoch,
        market,
        book: clutch_solana_layout::direct_selection::canonical_direct_book_id(epoch),
        terms: h(2),
        price_grid: h(3),
        policy: relation_policy,
        order_set: Hash32::ZERO,
        first_order_id: Hash32::ZERO,
        last_order_id: Hash32::ZERO,
        epoch_index,
        relation_version: RELATION_VERSION,
        price_scale: 10_000,
        remainder_seed: canonical_direct_remainder_seed(epoch),
        owner_count: 1,
        page_count: 0,
        order_count: 0,
        outcome_count: 2,
        basis_degree: 1,
        phase: EPOCH_PHASE_OPEN,
        stored_bump: 17,
        flags: 0,
    };
    DirectEpochV4Account {
        direct: DirectEpochV3Account {
            common,
            submission_opens_slot: 100,
            submission_closes_slot: 110,
        },
        selection_deadline_slot: 120,
        settlement_deadline_slot: 140,
        lifecycle_phase: DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN,
        terminal: DirectTerminalReceiptV3::EMPTY,
        neutral_lamport_sink: direct_sink(),
        verifier_release_id,
        direct_policy_v3_id: direct_policy.digest_for_epoch(epoch).unwrap(),
        epoch_funding: DirectFundingLedgerV3 {
            payer: h(24),
            payer_principal_lamports: 1_024,
            prior_donation_lamports: 24,
        },
        page_funding: DirectFundingLedgerV3::ZERO,
        reserved: [0; 4],
    }
}

#[allow(dead_code)]
fn direct_epoch_v4_with_frozen_book(epoch_index: u64) -> DirectEpochV4Account {
    let mut epoch = direct_epoch_v4(epoch_index);
    epoch.direct.common.order_set = h(5);
    epoch.direct.common.first_order_id = canonical_order_id(1);
    epoch.direct.common.last_order_id = canonical_order_id(2);
    epoch.direct.common.owner_count = 2;
    epoch.direct.common.page_count = 1;
    epoch.direct.common.order_count = 2;
    epoch.page_funding = DirectFundingLedgerV3 {
        payer: h(25),
        payer_principal_lamports: 1_025,
        prior_donation_lamports: 25,
    };
    epoch
}

#[allow(dead_code)]
pub fn direct_epoch_v4_frozen_empty(epoch_index: u64) -> DirectEpochV4Account {
    let mut epoch = direct_epoch_v4_with_frozen_book(epoch_index);
    epoch.direct.common.phase = EPOCH_PHASE_FROZEN;
    epoch.lifecycle_phase = DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY;
    epoch
}

#[allow(dead_code)]
pub fn direct_epoch_v4_selected(epoch_index: u64) -> DirectEpochV4Account {
    let mut epoch = direct_epoch_v4_with_frozen_book(epoch_index);
    epoch.direct.common.phase = EPOCH_PHASE_CLEARED;
    epoch.lifecycle_phase = DIRECT_LIFECYCLE_PHASE_SELECTED;
    epoch.terminal.selected_slot = 115;
    epoch.terminal.candidate = h(40);
    epoch.terminal.relation_candidate_digest = h(41);
    epoch
}

#[allow(dead_code)]
pub fn direct_epoch_v4_prefreeze_aborted(epoch_index: u64) -> DirectEpochV4Account {
    let mut epoch = direct_epoch_v4(epoch_index);
    epoch.lifecycle_phase = DIRECT_LIFECYCLE_PHASE_TERMINAL;
    epoch.terminal.reason = DIRECT_TERMINAL_REASON_PREFREEZE_ABORT;
    epoch.terminal.terminal_slot = epoch.direct.submission_opens_slot;
    epoch
}

#[allow(dead_code)]
pub fn direct_epoch_v4_settled(epoch_index: u64) -> DirectEpochV4Account {
    let mut epoch = direct_epoch_v4_with_frozen_book(epoch_index);
    epoch.direct.common.phase = EPOCH_PHASE_SETTLED;
    epoch.lifecycle_phase = DIRECT_LIFECYCLE_PHASE_TERMINAL;
    epoch.terminal = DirectTerminalReceiptV3 {
        reason: DIRECT_TERMINAL_REASON_SETTLED,
        outcome: 1,
        terminal_reservation_count: 2,
        selected_slot: 115,
        candidate: h(40),
        relation_candidate_digest: h(41),
        quantity: 4,
        price: 7_500,
        consideration_price_units: 30_000,
        terminal_slot: 130,
    };
    epoch
}

pub fn direct_reservation_v6() -> DirectReservationAccountV6 {
    DirectReservationAccountV6 {
        base: DirectReservationV2Account {
            reservation: reservation_base(14),
            funding: DirectFundingLedgerV3 {
                payer: h(91),
                payer_principal_lamports: 1_000,
                prior_donation_lamports: 3,
            },
        },
        count: ReservationCountTailV1 {
            epoch_generation: 8,
            position_counted: true,
        },
    }
}

pub fn direct_reservation_v8() -> DirectReservationAccountV8 {
    DirectReservationAccountV8 {
        base: direct_reservation_v6().base,
        retirement: ReservationRetirementTailV2 {
            count: ReservationCountTailV1 {
                epoch_generation: 8,
                position_counted: true,
            },
            rent: DeletableRentOwnerV1::from_persisted(id(91), 1_000, 3).unwrap(),
        },
    }
}
