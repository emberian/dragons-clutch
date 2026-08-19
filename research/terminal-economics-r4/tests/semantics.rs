use clutch_terminal_economics_r4::*;

fn id(byte: u8) -> Id {
    [byte; 32]
}

fn config() -> CreationConfig {
    CreationConfig {
        market: id(1),
        generation: 7,
        terminal_authority: id(2),
        rent_payer: id(3),
        rent_refund_to: id(4),
        neutral_sink: id(5),
        outcomes: 2,
        refundable_rent_principal: 1,
        credit_vault_rent_principal: 1,
        replay_rent_principal: 1,
        keeper_budget: 10,
    }
}

fn funding(owner: Id) -> CreditRentFunding {
    CreditRentFunding {
        payer: owner,
        refund_to: owner,
        principal: 1,
        prefund_donation: 0,
    }
}

fn resolution() -> Resolution {
    Resolution {
        denominator: 2,
        weights: [1, 1, 0, 0],
        receipt: id(90),
    }
}

fn observations(entries: &[(usize, Id, u64)]) -> [ObservedBearer; MAX_BEARERS] {
    let mut values = [ObservedBearer::EMPTY; MAX_BEARERS];
    for (slot, token_account, amount) in entries {
        values[*slot] = ObservedBearer {
            present: true,
            token_account: *token_account,
            amount: *amount,
        };
    }
    values
}

#[test]
fn internal_external_and_authoritative_supply_are_distinct() {
    let mut registry = Registry::empty();
    let mut model = Model::new(&mut registry, config()).unwrap();
    model
        .issue_position(0, 0, id(10), 3, id(10), id(10), 1, 0)
        .unwrap();
    assert_eq!(model.supply[0].internal, 3);
    assert_eq!(model.supply[0].external, 0);
    assert_eq!(model.mints[0].authoritative_supply, 0);

    model.materialize(1, 0, 0, id(10), id(20), 0, 2).unwrap();
    assert_eq!(model.supply[0].internal, 1);
    assert_eq!(model.supply[0].external, 2);
    assert_eq!(model.mints[0].authoritative_supply, 2);
    assert_eq!(model.hoard.issuance_in, 3);
    assert_eq!(model.hoard.balance, 3);
    model.validate().unwrap();
}

#[test]
fn observed_direct_burn_requires_matching_account_and_mint_deltas() {
    let mut registry = Registry::empty();
    let mut model = Model::new(&mut registry, config()).unwrap();
    model
        .issue_position(0, 0, id(10), 3, id(10), id(10), 1, 0)
        .unwrap();
    model.materialize(1, 0, 0, id(10), id(20), 0, 3).unwrap();

    let before = model;
    assert_eq!(
        model.reconcile_observed_direct_burns(2, 0, 0, observations(&[(0, id(20), 2)])),
        Err(Error::Invariant)
    );
    assert_eq!(model, before);
    assert_eq!(
        model.reconcile_observed_direct_burns(2, 0, 2, observations(&[(0, id(21), 2)])),
        Err(Error::Identity)
    );
    assert_eq!(model, before);

    model
        .reconcile_observed_direct_burns(2, 0, 2, observations(&[(0, id(20), 2)]))
        .unwrap();
    assert_eq!(model.bearers[0].amount, 2);
    assert_eq!(model.supply[0].external, 2);
    assert_eq!(model.supply[0].direct_burned, 1);
    assert_eq!(model.mints[0].authoritative_supply, 2);
    assert_eq!(model.hoard.balance, 3);
}

#[test]
fn one_complete_observation_reconciles_two_bearer_burns() {
    let mut registry = Registry::empty();
    let mut model = Model::new(&mut registry, config()).unwrap();
    model
        .issue_position(0, 0, id(10), 2, id(10), id(10), 1, 0)
        .unwrap();
    model.materialize(1, 0, 0, id(10), id(20), 0, 1).unwrap();
    model.materialize(2, 0, 1, id(10), id(21), 0, 1).unwrap();
    model
        .reconcile_observed_direct_burns(3, 0, 0, observations(&[(0, id(20), 0), (1, id(21), 0)]))
        .unwrap();
    assert_eq!(model.supply[0].external, 0);
    assert_eq!(model.supply[0].direct_burned, 2);
    assert_eq!(model.mints[0].authoritative_supply, 0);
    model.validate().unwrap();
}

#[test]
fn active_registry_reservation_and_early_mint_close_refuse() {
    let mut registry = Registry::empty();
    let mut model = Model::new(&mut registry, config()).unwrap();
    assert_eq!(Model::new(&mut registry, config()), Err(Error::Replay));
    model
        .issue_position(0, 0, id(10), 1, id(10), id(10), 1, 0)
        .unwrap();
    model.resolve(1, resolution()).unwrap();
    let before = model;
    assert_eq!(model.close_mint(2, 0, id(2)), Err(Error::OutstandingClaims));
    assert_eq!(model, before);
}

#[test]
fn derived_sink_alias_and_hostile_role_refuse_without_panic() {
    let mut aliased = config();
    let mut role_alias = aliased.market;
    role_alias[31] ^= Role::CreditVault.code();
    aliased.neutral_sink = role_alias;
    assert_eq!(
        Model::new(&mut Registry::empty(), aliased),
        Err(Error::Identity)
    );

    assert_eq!(
        RentRecord::new(
            Role::Credit(255),
            AccountClass::ExternalOwnerState,
            id(1),
            1,
            id(2),
            id(3),
            id(4),
            1,
            0,
        ),
        Err(Error::Rent)
    );
}

#[test]
fn same_owner_fractional_redemptions_merge_exactly() {
    let mut registry = Registry::empty();
    let mut model = Model::new(&mut registry, config()).unwrap();
    model
        .issue_position(0, 0, id(10), 1, id(10), id(10), 1, 0)
        .unwrap();
    model.materialize(1, 0, 0, id(10), id(20), 0, 1).unwrap();
    model.resolve(2, resolution()).unwrap();

    let first = model
        .redeem_external(3, 0, id(10), id(20), 1, 0, 0, 0, funding(id(10)))
        .unwrap();
    assert_eq!(first.paid_atoms, 0);
    assert_eq!(first.credit_numerator, 1);
    let second = model
        .redeem_internal(4, 0, id(10), 1, 1, 0, funding(id(10)))
        .unwrap();
    assert_eq!(second.paid_atoms, 1);
    assert_eq!(second.credit_numerator, 0);
    assert_eq!(model.credit_numerator_total, 0);
    assert_eq!(model.hoard.balance, 0);
    assert_eq!(model.hoard.redemption_out, 1);
    model.validate().unwrap();
}

fn terminal_with_credit_and_donation() -> (Model, Registry) {
    let mut registry = Registry::empty();
    let mut model = Model::new(&mut registry, config()).unwrap();
    model
        .issue_position(0, 0, id(10), 1, id(10), id(10), 1, 2)
        .unwrap();
    model.materialize(1, 0, 0, id(10), id(20), 0, 1).unwrap();
    model.materialize(2, 0, 1, id(10), id(21), 1, 1).unwrap();
    model.donate_collateral(3, 3).unwrap();
    model.resolve(4, resolution()).unwrap();
    model
        .redeem_external(5, 0, id(10), id(20), 1, 0, 0, 0, funding(id(10)))
        .unwrap();
    model
        .reconcile_observed_direct_burns(6, 1, 0, observations(&[(1, id(21), 0)]))
        .unwrap();
    let position_close = model.close_position(7, 0, id(10)).unwrap();
    assert_eq!(position_close.refund_atoms, 1);
    assert_eq!(position_close.donation_sink_atoms, 2);
    model.close_mint(8, 0, id(2)).unwrap();
    model.close_mint(9, 1, id(2)).unwrap();
    assert_eq!(model.seal_credit_vault(10).unwrap(), 1);
    assert_eq!(model.dispose_hoard_surplus(11).unwrap(), 3);
    model.close_market_graph(&mut registry, 12, id(2)).unwrap();
    (model, registry)
}

#[test]
fn terminal_graph_closes_but_fractional_credit_state_persists() {
    let (model, mut registry) = terminal_with_credit_and_donation();
    assert_eq!(model.phase, Phase::Terminal);
    assert_eq!(model.credit_vault.balance, 1);
    assert_eq!(model.credit_vault.credit_numerator_total, 1);
    assert_eq!(model.credits[0].numerator, 1);
    assert_eq!(model.terminal_slack_equation().unwrap(), (7, 7));
    model.registry_matches(&registry).unwrap();
    assert_eq!(
        Model::new(&mut registry, config()),
        Err(Error::Replay),
        "the creation-time replay receipt outlives the closed market graph"
    );
}

#[test]
fn owner_authorized_credit_forfeiture_releases_only_excess_vault_atoms() {
    let (mut model, registry) = terminal_with_credit_and_donation();
    assert_eq!(model.forfeit_credit(0, 0, id(10), 1).unwrap(), 1);
    assert_eq!(model.credit_vault.balance, 0);
    assert_eq!(model.credit_vault.forfeiture_sink_out, 1);
    assert_eq!(model.terminal_slack_equation().unwrap(), (8, 8));
    let effect = model.close_credit(1, 0, id(10), id(10)).unwrap();
    assert_eq!(effect.refund_atoms, 1);
    assert_eq!(effect.donation_sink_atoms, 0);
    model.validate().unwrap();
    model.registry_matches(&registry).unwrap();
}

#[test]
fn vault_donations_are_not_credit_backing_or_rounding_slack() {
    let (mut model, registry) = terminal_with_credit_and_donation();
    model
        .reconcile_terminal_credit_vault_donation(0, 2)
        .unwrap();
    assert_eq!(model.credit_vault.balance, 1);
    assert_eq!(model.credit_vault.donation_balance, 2);
    assert_eq!(model.credit_vault.rounding_slack_numerator(2), Ok(1));
    assert_eq!(model.terminal_slack_equation(), Ok((11, 11)));
    assert_eq!(model.dispose_terminal_credit_vault_donations(1), Ok(2));
    assert_eq!(model.credit_vault.donation_balance, 0);
    assert_eq!(model.credit_vault.donation_sink_out, 2);

    model
        .rents
        .get_mut(Role::Replay)
        .unwrap()
        .donate(4)
        .unwrap();
    model.validate().unwrap();
    model.registry_matches(&registry).unwrap();
}

#[test]
fn overfunded_credit_backing_and_tombstone_omission_refuse() {
    let (model, mut registry) = terminal_with_credit_and_donation();
    let mut overfunded = model;
    overfunded.credit_vault.balance += 1;
    overfunded.credit_vault.ingress += 1;
    overfunded.hoard.credit_vault_out += 1;
    overfunded.hoard.surplus_sink_out -= 1;
    assert_eq!(overfunded.validate(), Err(Error::Invariant));

    registry.tombstone.rent.closed_role_bits &= !(1 << 8);
    assert!(registry.tombstone.validate().is_err());
    assert!(model.registry_matches(&registry).is_err());
}

#[test]
fn terminal_recipient_can_prefund_and_accept_a_credit_transfer() {
    let (mut model, registry) = terminal_with_credit_and_donation();
    model
        .open_terminal_credit(0, 1, id(11), funding(id(11)))
        .unwrap();
    assert_eq!(model.transfer_credit(1, 0, id(10), 1, id(11), 1), Ok(0));
    assert_eq!(model.credits[0].numerator, 0);
    assert_eq!(model.credits[1].numerator, 1);
    assert_eq!(model.credit_vault.balance, 1);
    model.registry_matches(&registry).unwrap();
}

#[test]
fn two_owners_need_an_authenticated_merge_to_cross_one_atom() {
    let mut registry = Registry::empty();
    let mut model = Model::new(&mut registry, config()).unwrap();
    for (nonce, slot, owner) in [(0, 0, id(10)), (1, 1, id(11))] {
        model
            .issue_position(nonce, slot, owner, 1, owner, owner, 1, 0)
            .unwrap();
    }
    model.materialize(2, 0, 0, id(10), id(20), 0, 1).unwrap();
    model.materialize(3, 0, 1, id(10), id(21), 1, 1).unwrap();
    model.materialize(4, 1, 2, id(11), id(22), 0, 1).unwrap();
    model.materialize(5, 1, 3, id(11), id(23), 1, 1).unwrap();
    model.resolve(6, resolution()).unwrap();
    model
        .redeem_external(7, 0, id(10), id(20), 1, 0, 1, 0, funding(id(10)))
        .unwrap();
    model
        .reconcile_observed_direct_burns(8, 1, 1, observations(&[(1, id(21), 0), (3, id(23), 1)]))
        .unwrap();
    model
        .reconcile_observed_direct_burns(9, 0, 0, observations(&[(0, id(20), 0), (2, id(22), 0)]))
        .unwrap();
    model
        .redeem_external(10, 3, id(11), id(23), 1, 0, 0, 1, funding(id(11)))
        .unwrap();
    model.close_position(11, 0, id(10)).unwrap();
    model.close_position(12, 1, id(11)).unwrap();
    model.close_mint(13, 0, id(2)).unwrap();
    model.close_mint(14, 1, id(2)).unwrap();
    assert_eq!(model.seal_credit_vault(15).unwrap(), 1);
    assert_eq!(model.dispose_hoard_surplus(16).unwrap(), 1);
    model.close_market_graph(&mut registry, 17, id(2)).unwrap();

    let before = model;
    assert_eq!(
        model.transfer_credit(0, 0, id(99), 1, id(11), 1),
        Err(Error::Authority)
    );
    assert_eq!(model, before);
    assert_eq!(model.transfer_credit(0, 0, id(10), 1, id(11), 1), Ok(1));
    assert_eq!(model.credit_vault.balance, 0);
    assert_eq!(model.credit_vault.payout_out, 1);
    assert_eq!(model.credits[0].numerator, 0);
    assert_eq!(model.credits[1].numerator, 0);
    assert_eq!(model.terminal_slack_equation().unwrap(), (2, 2));
}

#[test]
fn rent_records_are_funded_and_close_principal_and_donation_once() {
    let mut record = RentRecord::new(
        Role::Position(0),
        AccountClass::RefundableTransient,
        id(1),
        1,
        id(2),
        id(3),
        id(4),
        100,
        7,
    )
    .unwrap();
    record.donate(5).unwrap();
    let effect = record.close(id(3), id(4)).unwrap();
    assert_eq!(effect.refund_atoms, 100);
    assert_eq!(effect.donation_sink_atoms, 12);
    assert_eq!(record.close(id(3), id(4)), Err(Error::Replay));
    assert_eq!(
        RentRecord::new(
            Role::Position(9),
            AccountClass::RefundableTransient,
            id(1),
            1,
            id(2),
            id(3),
            id(4),
            1,
            0,
        ),
        Err(Error::Rent)
    );

    let mut capped = RentRecord::new(
        Role::Position(1),
        AccountClass::RefundableTransient,
        id(1),
        1,
        id(2),
        id(3),
        id(4),
        MAX_ATOMS,
        0,
    )
    .unwrap();
    let before = capped;
    assert_eq!(capped.donate(1), Err(Error::Rent));
    assert_eq!(capped, before);
}

#[test]
fn legacy_mints_are_never_relabelled_closeable() {
    assert_eq!(
        classify_mint(MintVersion::LegacyNoClose, None),
        Ok(AccountClass::PermanentInfra)
    );
    assert_eq!(migrate_legacy_mint_in_place(), Err(Error::MigrationStop));
    assert_eq!(
        classify_mint(MintVersion::R4Closeable, None),
        Err(Error::Authority)
    );
}

#[test]
fn codecs_round_trip_and_reject_noncanonical_bytes() {
    let policy = PolicyWireV4 {
        market: id(1),
        generation: 7,
        outcomes: 2,
        terminal_authority: id(2),
        rent_refund_to: id(3),
        neutral_sink: id(4),
        credit_vault: id(5),
        replay_tombstone: id(6),
        keeper_budget_atoms: 10,
        minimum_credit_rent: 1,
    };
    let mut policy_bytes = [0_u8; POLICY_WIRE_BYTES];
    policy.encode(&mut policy_bytes).unwrap();
    assert_eq!(PolicyWireV4::decode(&policy_bytes), Ok(policy));
    policy_bytes[6] = 1;
    assert_eq!(
        PolicyWireV4::decode(&policy_bytes),
        Err(DecodeError::Padding)
    );
    let mut policy_alias = policy;
    policy_alias.credit_vault = policy_alias.market;
    assert_eq!(policy_alias.validate(), Err(DecodeError::Shape));

    let credit = CreditWireV1 {
        market: id(1),
        generation: 7,
        owner: id(10),
        numerator: 1,
        denominator: 2,
        rent_account: id(11),
        nonce: 9,
        closed: false,
    };
    let mut credit_bytes = [0_u8; CREDIT_WIRE_BYTES];
    credit.encode(&mut credit_bytes).unwrap();
    assert_eq!(CreditWireV1::decode(&credit_bytes), Ok(credit));
    credit_bytes[5] = 2;
    assert_eq!(CreditWireV1::decode(&credit_bytes), Err(DecodeError::Shape));

    let root = CreditRootWireV1 {
        market: id(1),
        generation: 7,
        phase: Phase::Terminal,
        credit_vault: id(12),
        denominator: 2,
        credit_numerator_total: 1,
        forfeited_numerator: 1,
        nonce: 3,
        terminal_market_nonce: 12,
    };
    let mut root_bytes = [0_u8; CREDIT_ROOT_WIRE_BYTES];
    root.encode(&mut root_bytes).unwrap();
    assert_eq!(CreditRootWireV1::decode(&root_bytes), Ok(root));
    root_bytes[5] = Phase::Active as u8;
    assert_eq!(
        CreditRootWireV1::decode(&root_bytes),
        Err(DecodeError::Shape)
    );
    let mut root_alias = root;
    root_alias.credit_vault = root_alias.market;
    assert_eq!(root_alias.validate(), Err(DecodeError::Shape));

    let rent = RentWireV1 {
        record: RentRecord::new(
            Role::Credit(0),
            AccountClass::ExternalOwnerState,
            id(1),
            7,
            id(10),
            id(10),
            id(4),
            1,
            2,
        )
        .unwrap(),
    };
    let mut rent_bytes = [0_u8; RENT_WIRE_BYTES];
    rent.encode(&mut rent_bytes).unwrap();
    assert_eq!(RentWireV1::decode(&rent_bytes), Ok(rent));
    let mut rent_alias = rent;
    rent_alias.record.account = rent_alias.record.refund_to;
    assert_eq!(rent_alias.encode(&mut rent_bytes), Err(DecodeError::Shape));
    let mut over_cap_rent = rent;
    over_cap_rent.record.principal = MAX_ATOMS;
    over_cap_rent.record.donations = 1;
    over_cap_rent.record.balance = MAX_ATOMS + 1;
    assert_eq!(
        over_cap_rent.encode(&mut rent_bytes),
        Err(DecodeError::Shape)
    );

    let tombstone = TombstoneWireV1 {
        tombstone: Tombstone {
            present: true,
            market: id(1),
            generation: 7,
            outcomes: 2,
            terminal_receipt: id(90),
            final_market_nonce: 12,
            replay_account: id(91),
            credit_vault_account: id(92),
            rent: RentSummary {
                closed_role_bits: 0x3f | (1 << 8) | (1 << 9),
                permanent_role_bits: (1 << 12) | (1 << 21),
                open_external_role_bits: 1 << 13,
                principal_refunded: 10,
                donations_sunk: 2,
                permanent_principal: 3,
                permanent_donations: 4,
                external_principal_live: 5,
                external_donations_live: 6,
            },
            keeper_deposit: 10,
            keeper_rewards_paid: 2,
            keeper_refund_paid: 8,
            keeper_donations_sunk: 1,
        },
    };
    let mut tombstone_bytes = [0_u8; TOMBSTONE_WIRE_BYTES];
    tombstone.encode(&mut tombstone_bytes).unwrap();
    assert_eq!(TombstoneWireV1::decode(&tombstone_bytes), Ok(tombstone));
    let mut phantom_tombstone = tombstone;
    phantom_tombstone.tombstone.rent.closed_role_bits |= 1 << 22;
    assert_eq!(
        phantom_tombstone.encode(&mut tombstone_bytes),
        Err(DecodeError::Shape)
    );
    let mut inactive_mint_tombstone = tombstone;
    inactive_mint_tombstone.tombstone.rent.closed_role_bits |= 1 << 10;
    assert_eq!(
        inactive_mint_tombstone.encode(&mut tombstone_bytes),
        Err(DecodeError::Shape)
    );
    let mut reclassified_position = tombstone;
    reclassified_position.tombstone.rent.closed_role_bits &= !(1 << 5);
    reclassified_position.tombstone.rent.permanent_role_bits |= 1 << 5;
    assert_eq!(
        reclassified_position.encode(&mut tombstone_bytes),
        Err(DecodeError::Shape)
    );
    let mut impossible_keeper = tombstone;
    impossible_keeper.tombstone.keeper_donations_sunk = MAX_ATOMS;
    assert_eq!(
        impossible_keeper.encode(&mut tombstone_bytes),
        Err(DecodeError::Shape)
    );

    let mint = MintBindingWireV4 {
        market: id(1),
        generation: 7,
        mint: id(20),
        terminal_authority: id(2),
        outcome: 0,
        authoritative_supply: 3,
    };
    let mut mint_bytes = [0_u8; MINT_BINDING_WIRE_BYTES];
    mint.encode(&mut mint_bytes).unwrap();
    assert_eq!(MintBindingWireV4::decode(&mint_bytes), Ok(mint));
    mint_bytes[112] = 1;
    assert_eq!(
        MintBindingWireV4::decode(&mint_bytes),
        Err(DecodeError::Padding)
    );
}

#[test]
fn mutated_state_and_stale_nonce_refuse_atomically() {
    let mut registry = Registry::empty();
    let mut model = Model::new(&mut registry, config()).unwrap();
    model
        .issue_position(0, 0, id(10), 1, id(10), id(10), 1, 0)
        .unwrap();
    let before = model;
    assert_eq!(model.donate_collateral(0, 1), Err(Error::Replay));
    assert_eq!(model, before);

    let mut corrupted = model;
    corrupted.rents.records[Role::Position(0).index()].closed = true;
    assert!(corrupted.validate().is_err());

    let mut wrong_market = model;
    wrong_market.rents.records[Role::Position(0).index()].market = id(99);
    assert_eq!(wrong_market.validate(), Err(Error::Rent));
}
