use clutch_dealer_runtime_contract::*;

pub fn id(byte: u8) -> Id {
    Id::from_bytes([byte; 32])
}

pub fn child_rent() -> DeletableRentOwnerV1 {
    DeletableRentOwnerV1 {
        payer: id(240),
        neutral_sink: id(72),
        refundable_principal: 1_000,
        donation_floor: 7,
    }
}

pub fn root_rent() -> RootRentOwnerV1 {
    RootRentOwnerV1 {
        payer: id(241),
        neutral_sink: id(72),
        refundable_live_principal: 2_000,
        permanent_tombstone_principal: 500,
        donation_floor: 9,
    }
}

pub fn policy() -> DealerPolicyV1 {
    let mut unit_eggs = [0; MAX_OUTCOMES];
    unit_eggs[0] = 10;
    unit_eggs[1] = 10;
    let mut weights = [0; MAX_OUTCOMES];
    weights[0] = 1;
    weights[1] = 1;
    let mut buy = [0; MAX_OUTCOMES];
    buy[0] = 100;
    buy[1] = 100;
    let mut sell = [0; MAX_OUTCOMES];
    sell[0] = 100;
    sell[1] = 100;
    DealerPolicyV1 {
        realm_id: id(1),
        profile_id: id(2),
        market_instance_v2_id: id(3),
        claim_basis_id: id(4),
        collateral_mint: id(5),
        token_program: id(6),
        hoard_custody_semantics_id: id(7),
        relation_v2_id: id(8),
        price_measure_policy_id: id(9),
        curve_policy_id: id(10),
        curve_price_certificate_policy_id: id(11),
        fee_policy_id: id(12),
        liveness_policy_id: id(13),
        retirement_policy_id: id(14),
        neutral_sink: id(72),
        quote_authority: id(15),
        outcome_count: 2,
        payout_denominator: 10,
        capital_unit_cash_atoms: 10,
        capital_unit_eggs: unit_eggs,
        initial_price_denominator: 2,
        initial_price_weights: weights,
        depth_atoms: 1_000,
        max_net_buy: buy,
        max_net_sell: sell,
        minimum_lp_shares: 10,
        maximum_lp_shares: 100,
        funding_deadline_slot: 100,
        trading_open_slot: 100,
        trading_close_slot: 1_000,
        maturity_slot: 2_000,
        shutdown_queue_numerator: 1,
        shutdown_queue_denominator: 2,
        maximum_lp_pages: 4,
    }
}

pub fn funding_state() -> DealerStateV1 {
    DealerStateV1 {
        policy_id: policy().policy_id().unwrap(),
        facility_id: id(20),
        facility_position_id: id(21),
        facility_position_account_id: id(26),
        facility_replay_account_id: id(27),
        sponsor: id(22),
        sponsor_refund_recipient: id(23),
        lp_page_head_id: id(24),
        lp_page_set_root: id(25),
        active_epoch_id: Id::ZERO,
        active_lease_id: Id::ZERO,
        phase: DealerPhaseV1::Funding,
        sponsor_capital_disposition: SponsorCapitalDispositionV1::Refundable,
        outcome_count: 2,
        generation: 0,
        child_sequence: 3,
        total_shares: 10,
        queued_shares: 0,
        terminal_claimed_shares: 0,
        sponsor_capital_atoms: 500,
        net_sold: [0; MAX_OUTCOMES],
        children: DealerChildCountsV1 {
            facility_positions: 1,
            facility_replays: 1,
            lp_pages: 1,
            live_lp_positions: 1,
            unclaimed_lp_positions: 0,
            epoch_bindings: 0,
            leases: 0,
            settlement_pots: 0,
            fee_budgets: 1,
            liveness_budgets: 1,
            resolution_claim_work: 0,
        },
        rent: root_rent(),
    }
}

pub fn trading_state() -> DealerStateV1 {
    let mut state = funding_state();
    state.phase = DealerPhaseV1::Trading;
    state.sponsor_capital_disposition = SponsorCapitalDispositionV1::Donated;
    state.generation = 7;
    state.child_sequence = 8;
    state.net_sold[0] = 10;
    state.net_sold[1] = -10;
    state.active_epoch_id = id(28);
    state.active_lease_id = id(29);
    state.children.epoch_bindings = 1;
    state.children.leases = 1;
    state.children.settlement_pots = 1;
    state
}

pub fn lp_page() -> LpPageV1 {
    let mut entries = [LpEntryV1::EMPTY; LP_ENTRIES_PER_PAGE];
    entries[0] = LpEntryV1 {
        owner: id(40),
        shares: 10,
        queued_shares: 0,
        terminal_claim_atoms: 0,
        claimed: false,
    };
    LpPageV1 {
        policy_id: policy().policy_id().unwrap(),
        facility_id: id(20),
        counted_generation: 0,
        page_ordinal: 0,
        next_page_ordinal: NO_NEXT_LP_PAGE,
        entry_count: 1,
        sealed: false,
        terminal_allocated: false,
        revision: 1,
        entries,
        rent: child_rent(),
    }
}

pub fn lease() -> DealerLeaseV1 {
    let state = trading_state();
    DealerLeaseV1 {
        policy_id: policy().policy_id().unwrap(),
        facility_id: state.facility_id,
        dealer_state_account_id: id(60),
        facility_position_pre_id: state.facility_position_id,
        lease_account_id: state.active_lease_id,
        market_instance_v2_id: policy().market_instance_v2_id,
        epoch_id: state.active_epoch_id,
        settlement_candidate_id: id(51),
        upstream_economic_candidate_id: id(61),
        quote_id: id(52),
        dealer_leg_verdict_id: id(53),
        curve_price_certificate_id: id(54),
        settlement_rows_root: id(55),
        settlement_pot_id: id(56),
        fee_budget_id: id(57),
        liveness_budget_id: id(58),
        pre_generation: 7,
        post_generation: 8,
        created_slot: 200,
        collect_deadline_slot: 210,
        deliver_deadline_slot: 220,
        outcome_count: 2,
        row_count: 2,
        rent: child_rent(),
    }
}

pub fn collect_pot() -> SettlementPotV1 {
    let lease = lease();
    let mut eggs_in = [0; MAX_OUTCOMES];
    eggs_in[0] = 10;
    let mut eggs_out = [0; MAX_OUTCOMES];
    eggs_out[1] = 10;
    SettlementPotV1 {
        policy_id: lease.policy_id,
        facility_id: lease.facility_id,
        lease_id: lease.lease_id().unwrap(),
        epoch_id: lease.epoch_id,
        settlement_candidate_id: lease.settlement_candidate_id,
        aggregate_verdict_id: lease.dealer_leg_verdict_id,
        curve_price_certificate_id: lease.curve_price_certificate_id,
        facility_position_pre_id: lease.facility_position_pre_id,
        facility_position_post_id: id(59),
        settlement_rows_root: lease.settlement_rows_root,
        fee_budget_id: lease.fee_budget_id,
        liveness_budget_id: lease.liveness_budget_id,
        phase: SettlementPotPhaseV1::Collecting,
        outcome_count: 2,
        pre_generation: 7,
        post_generation: 8,
        row_count: 2,
        collect_cursor: 0,
        deliver_cursor: 0,
        user_cash_in_atoms: 20,
        user_cash_out_atoms: 21,
        dealer_net_cash_in_atoms: 0,
        dealer_net_cash_out_atoms: 1,
        facility_buy_eggs: eggs_in,
        facility_sell_eggs: eggs_out,
        fee_liability_atoms: 1,
        liveness_liability_atoms: 1,
        collected_user_cash_atoms: 0,
        collected_user_eggs: [0; MAX_OUTCOMES],
        delivered_user_cash_atoms: 0,
        delivered_user_eggs: [0; MAX_OUTCOMES],
        rent: child_rent(),
    }
}

pub fn deliver_pot() -> SettlementPotV1 {
    let mut pot = collect_pot();
    pot.phase = SettlementPotPhaseV1::Delivering;
    pot.collect_cursor = pot.row_count;
    pot.collected_user_cash_atoms = pot.user_cash_in_atoms;
    pot.collected_user_eggs = pot.facility_buy_eggs;
    pot
}

pub fn finalizing_pot() -> SettlementPotV1 {
    let mut pot = deliver_pot();
    pot.phase = SettlementPotPhaseV1::Finalizing;
    pot.deliver_cursor = pot.row_count;
    pot.delivered_user_cash_atoms = pot.user_cash_out_atoms;
    pot.delivered_user_eggs = pot.facility_sell_eggs;
    pot
}

pub fn fee_budget() -> FeeBudgetV1 {
    FeeBudgetV1 {
        policy_id: policy().policy_id().unwrap(),
        facility_id: id(20),
        fee_policy_id: id(12),
        principal_payer: id(70),
        principal_refund_recipient: id(71),
        neutral_sink: id(72),
        counted_generation: 0,
        principal_atoms: 100,
        available_atoms: 80,
        reserved_liability_atoms: 10,
        spent_atoms: 10,
        refunded_atoms: 0,
        sinked_atoms: 0,
        liability_count: 1,
        phase: BudgetPhaseV1::Open,
        rent: child_rent(),
    }
}

pub fn liveness_budget() -> LivenessBudgetV1 {
    let fee = fee_budget();
    LivenessBudgetV1 {
        policy_id: fee.policy_id,
        facility_id: fee.facility_id,
        liveness_policy_id: id(13),
        principal_payer: id(73),
        principal_refund_recipient: id(74),
        neutral_sink: fee.neutral_sink,
        counted_generation: fee.counted_generation,
        principal_atoms: fee.principal_atoms,
        available_atoms: fee.available_atoms,
        reserved_liability_atoms: fee.reserved_liability_atoms,
        spent_atoms: fee.spent_atoms,
        refunded_atoms: fee.refunded_atoms,
        sinked_atoms: fee.sinked_atoms,
        liability_count: fee.liability_count,
        phase: fee.phase,
        rent: fee.rent,
    }
}
