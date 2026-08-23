use clutch_retirement::{
    EpochChildCountsV1, EpochRetirementTailV1, Identity32V1, MarketEpochCursorV1,
    PositionRetirementTailV1, RentSplitV2, ReservationCountTailV1,
};
use clutch_retirement_adapter::{
    DirectReservationAccountV6, GeneralEpochAccountV5, GeneralReservationAccountV5,
    MarketAccountV2, PositionAccountV2,
};
use clutch_solana_layout::{
    canonical_epoch_id, canonical_order_id, canonical_outcome_id,
    direct_selection_v3::{DirectFundingLedgerV3, DirectReservationV2Account},
    reservation::{ReservationAccount, ReservationPlan},
    EpochAccount, Hash32, MarketAccount, PositionAccount, EPOCH_PHASE_OPEN, MAX_OUTCOMES,
    ORDER_KIND_SINGLE, RELATION_VERSION,
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

pub fn direct_sink() -> Hash32 {
    h(90)
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
