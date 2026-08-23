use clutch_product_compiler_v1::*;
use clutch_solana_layout::{
    canonical_price_grid_id, Hash32, PriceGridAccount, ProfileAccount, RealmAccount,
    PROFILE_FLAG_POLICY_FROZEN, UNIFORM_SPACING_NONE,
};

fn id(byte: u8) -> Id {
    [byte; 32]
}

fn source() -> SourceSpecViewV1 {
    SourceSpecViewV1 {
        source_spec_id: id(1),
        source_adapter_id: id(2),
        source_version: 2,
        grid_family_id: 7,
        grid_version: 3,
        bucket_seconds: 60,
    }
}

fn summary() -> SummaryProgramV1 {
    SummaryProgramV1 {
        evaluator_program_id: id(3),
        evaluator_version: 4,
        feature_mask: FEATURE_TERMINAL_INTERVAL | FEATURE_MAXIMUM_DRAWDOWN_INTERVAL,
    }
}

fn hatchery() -> HatcheryProgramV1 {
    HatcheryProgramV1 {
        release_id: id(14),
        source_plane_version: 3,
        raw_page_version: 1,
        window_result_version: 1,
        max_window_records: 1_000,
        capabilities: HATCHERY_RECURRING_REQUIRED,
    }
}

fn template(statistic: StatisticProgramV1) -> TemplateV1 {
    let (mut payouts, _, payout_map) = categorical_one_hot_payouts(4, 1_000).unwrap();
    payouts[4] = clutch_solana_layout::PayoutVectorBytes {
        denominator: 1_000,
        weights: [250, 250, 250, 250, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    };
    let mut knots = [0_u128; 16];
    knots[..3].copy_from_slice(match statistic {
        StatisticProgramV1::TerminalInterval => &[80_000_000, 100_000_000, 120_000_000],
        StatisticProgramV1::MaximumDrawdownInterval => &[100_000, 200_000, 300_000],
    });
    TemplateV1 {
        source_spec_id: source().source_spec_id,
        hatchery_program_id: hatchery().id().unwrap(),
        summary_program_id: summary().id().unwrap(),
        presentation_digest: id(4),
        compiler_version: 1,
        window_span_buckets: 60,
        repair_grace_buckets: 10,
        repair_generation: 0,
        coverage_policy_id: 1,
        coverage_policy_parameter: 0,
        statistic,
        ambiguity_policy_id: 1,
        edge_policy_id: 1,
        repair_policy_id: 1,
        failure_policy_id: 1,
        basis_degree: 0,
        outcome_count: 4,
        payout_count: 5,
        payouts,
        knot_count: 3,
        uniform_log2_spacing: UNIFORM_SPACING_NONE,
        failure_payout_index: 4,
        payout_map,
        knots,
    }
}

fn work() -> WorkEnvelopeV1 {
    WorkEnvelopeV1 {
        version: 1,
        creation_lamports: 20_000_000,
        liveness_lamports: 50_000_000,
    }
}

fn liquidity(t: &TemplateV1) -> LiquidityBlueprintV1 {
    let mut max_inventory = [0_u64; 16];
    max_inventory[..4].copy_from_slice(&[10_000, 20_000, 20_000, 10_000]);
    LiquidityBlueprintV1 {
        template_id: t.id().unwrap(),
        payoff_region_digest: id(5),
        quote_schedule_compiler_id: id(6),
        max_inventory,
        collateral_cap: 250_000,
        batch_start: 0,
        batch_end: 9,
        fee_policy_id: id(7),
        withdrawal_policy_id: id(8),
        compiler_version: 1,
    }
}

fn price_grid(realm: Id) -> PriceGridAccount {
    let mut ticks = [0_u64; 64];
    ticks[..5].copy_from_slice(&[0, 250, 500, 750, 1_000]);
    let mut value = PriceGridAccount {
        grid: Hash32::ZERO,
        realm: Hash32::from_bytes(realm),
        price_scale: 1_000,
        tick_count: 5,
        ticks,
        stored_bump: 0,
        flags: 0,
    };
    value.grid = value.recomputed_grid_id().unwrap();
    value
}

fn series(t: &TemplateV1, l: &LiquidityBlueprintV1) -> SeriesPlanV1 {
    let grid = price_grid(id(9));
    SeriesPlanV1 {
        template_id: t.id().unwrap(),
        realm: id(9),
        profile: id(10),
        price_grid: grid.grid.bytes(),
        fee_policy_id: id(11),
        work_envelope_id: work().id().unwrap(),
        liquidity_blueprint_id: l.id(t).unwrap(),
        first_start_bucket: 1_000,
        stride_buckets: 120,
        instance_count: 3,
        creation_lead_buckets: 20,
        market_collateral_cap: 1_000_000,
    }
}

fn funded(p: &SeriesPlanV1, t: &TemplateV1, l: &LiquidityBlueprintV1) -> SeriesFundingV1 {
    SeriesFundingV1::activate(p, t, &work(), l, 60_000_000, 150_000_000, 750_000).unwrap()
}

fn fold_points(points: &[u128]) -> DrawdownSummaryV1 {
    let mut summary = DrawdownSummaryV1::observation(0, points[0], points[0]).unwrap();
    for (index, point) in points.iter().copied().enumerate().skip(1) {
        summary = summary
            .combine(DrawdownSummaryV1::observation(index as u64, point, point).unwrap())
            .unwrap();
    }
    summary
}

fn hex(bytes: Id) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn terminal_series_lowers_to_current_terms_market_and_liquidity_policy() {
    let t = template(StatisticProgramV1::TerminalInterval);
    let l = liquidity(&t);
    let p = series(&t, &l);
    let f = funded(&p, &t, &l);
    let (next, mut instance) = f
        .instantiate_next(
            &p,
            &t,
            &source(),
            &summary(),
            &hatchery(),
            &work(),
            &l,
            0,
            990,
        )
        .unwrap();
    assert_eq!(next.next_ordinal, 1);
    assert_eq!(next.creation_lamports, 40_000_000);
    assert_eq!(next.liveness_lamports, 100_000_000);
    assert_eq!(next.liquidity_collateral, 500_000);
    assert_eq!(instance.start_bucket, 1_000);
    assert_eq!(instance.end_bucket_exclusive, 1_060);
    assert_eq!(instance.maturity_bucket_exclusive, 1_070);

    let terms = lower_current_terms(
        &mut instance,
        &p,
        &t,
        &source(),
        &summary(),
        &hatchery(),
        &work(),
        &l,
    )
    .unwrap();
    terms.validate().unwrap();
    let market = current_market_projection(&instance, &p, &t, 1, 2, 55).unwrap();
    terms.binds_market(&market).unwrap();

    let policy = l
        .bind_current_policy(&t, &instance, &terms, id(12))
        .unwrap();
    policy.validate().unwrap();
    assert_eq!(policy.terms.market, market.market.bytes());
    assert_eq!(policy.terms.terms_digest, terms.terms.bytes());
    assert_eq!(policy.collateral_cap, instance.liquidity_collateral);
}

#[test]
fn terminal_and_drawdown_share_hatchery_window_but_not_template_semantics() {
    let terminal = template(StatisticProgramV1::TerminalInterval);
    let drawdown = template(StatisticProgramV1::MaximumDrawdownInterval);
    assert_ne!(terminal.id().unwrap(), drawdown.id().unwrap());

    let terminal_liquidity = liquidity(&terminal);
    let drawdown_liquidity = liquidity(&drawdown);
    let terminal_series = series(&terminal, &terminal_liquidity);
    let drawdown_series = series(&drawdown, &drawdown_liquidity);
    let terminal_instance = compile_instance(
        &terminal_series,
        &terminal,
        &source(),
        &summary(),
        &hatchery(),
        &work(),
        &terminal_liquidity,
        0,
    )
    .unwrap();
    let mut drawdown_instance = compile_instance(
        &drawdown_series,
        &drawdown,
        &source(),
        &summary(),
        &hatchery(),
        &work(),
        &drawdown_liquidity,
        0,
    )
    .unwrap();
    assert_eq!(
        terminal_instance.hatchery_window_id,
        drawdown_instance.hatchery_window_id
    );
    assert_ne!(
        terminal_instance.statistic_result_id,
        drawdown_instance.statistic_result_id
    );
    assert_eq!(
        terminal_instance.current_window_id,
        drawdown_instance.current_window_id
    );
    assert_eq!(
        lower_current_terms(
            &mut drawdown_instance,
            &drawdown_series,
            &drawdown,
            &source(),
            &summary(),
            &hatchery(),
            &work(),
            &drawdown_liquidity,
        ),
        Err(Error::UnsupportedCurrentLowering)
    );
}

#[test]
fn permissionless_recurrence_refuses_choice_early_late_and_underfunding() {
    let t = template(StatisticProgramV1::TerminalInterval);
    let l = liquidity(&t);
    let p = series(&t, &l);
    let f = funded(&p, &t, &l);
    assert_eq!(
        f.instantiate_next(
            &p,
            &t,
            &source(),
            &summary(),
            &hatchery(),
            &work(),
            &l,
            1,
            990,
        ),
        Err(Error::WrongOrdinal)
    );
    assert_eq!(
        f.instantiate_next(
            &p,
            &t,
            &source(),
            &summary(),
            &hatchery(),
            &work(),
            &l,
            0,
            979,
        ),
        Err(Error::NotEligible)
    );
    assert_eq!(
        f.instantiate_next(
            &p,
            &t,
            &source(),
            &summary(),
            &hatchery(),
            &work(),
            &l,
            0,
            1_000,
        ),
        Err(Error::NotEligible)
    );
    let short = SeriesFundingV1 {
        creation_lamports: work().creation_lamports,
        ..f
    };
    assert_eq!(
        short.instantiate_next(
            &p,
            &t,
            &source(),
            &summary(),
            &hatchery(),
            &work(),
            &l,
            0,
            990,
        ),
        Err(Error::InsufficientPrepayment)
    );
    assert_eq!(
        SeriesFundingV1::activate(&p, &t, &work(), &l, 60_000_000, 149_999_999, 750_000,),
        Err(Error::InsufficientPrepayment)
    );
}

#[test]
fn expired_ordinal_lapses_without_spending_another_instances_money() {
    let t = template(StatisticProgramV1::TerminalInterval);
    let l = liquidity(&t);
    let p = series(&t, &l);
    let f = funded(&p, &t, &l);
    let lapsed = f.lapse_next(&p, &t, &work(), &l, 1_000).unwrap();
    assert_eq!(lapsed.next_ordinal, 1);
    assert_eq!(lapsed.creation_lamports, f.creation_lamports);
    assert_eq!(lapsed.liveness_lamports, f.liveness_lamports);
    assert_eq!(lapsed.liquidity_collateral, f.liquidity_collateral);
    let (next, instance) = lapsed
        .instantiate_next(
            &p,
            &t,
            &source(),
            &summary(),
            &hatchery(),
            &work(),
            &l,
            1,
            1_110,
        )
        .unwrap();
    assert_eq!(instance.start_bucket, 1_120);
    assert_eq!(next.next_ordinal, 2);
}

#[test]
fn source_summary_padding_and_nonce_drift_fail_closed() {
    let t = template(StatisticProgramV1::TerminalInterval);
    let l = liquidity(&t);
    let p = series(&t, &l);

    let mut wrong_source = source();
    wrong_source.source_spec_id[0] ^= 1;
    assert_eq!(
        compile_instance(
            &p,
            &t,
            &wrong_source,
            &summary(),
            &hatchery(),
            &work(),
            &l,
            0,
        ),
        Err(Error::MismatchedArtifact)
    );
    let terminal_only = SummaryProgramV1 {
        feature_mask: FEATURE_TERMINAL_INTERVAL,
        ..summary()
    };
    let drawdown = template(StatisticProgramV1::MaximumDrawdownInterval);
    let drawdown_liquidity = liquidity(&drawdown);
    let drawdown_series = series(&drawdown, &drawdown_liquidity);
    assert_eq!(
        compile_instance(
            &drawdown_series,
            &drawdown,
            &source(),
            &terminal_only,
            &hatchery(),
            &work(),
            &drawdown_liquidity,
            0,
        ),
        Err(Error::MismatchedArtifact)
    );

    let mut padded = t;
    padded.knots[15] = 7;
    assert!(matches!(padded.validate(), Err(Error::CurrentLayout(_))));

    let mut false_uniform = t;
    false_uniform.payouts[4].weights[..4].copy_from_slice(&[1_000, 0, 0, 0]);
    assert_eq!(false_uniform.validate(), Err(Error::InvalidParameter));

    let mut drawdown_out_of_unit = drawdown;
    drawdown_out_of_unit.knots[2] = u128::from(DRAWDOWN_PPM_SCALE) + 1;
    assert_eq!(
        drawdown_out_of_unit.validate(),
        Err(Error::InvalidParameter)
    );

    let mut overflow = p;
    overflow.first_start_bucket = u64::MAX - 1;
    assert_eq!(
        overflow.validate(&t, &work(), &l),
        Err(Error::ArithmeticOverflow)
    );
}

#[test]
fn current_realm_profile_and_grid_are_joined_not_trusted_by_label() {
    let t = template(StatisticProgramV1::TerminalInterval);
    let l = liquidity(&t);
    let p = series(&t, &l);
    let realm = RealmAccount {
        realm: Hash32::from_bytes(p.realm),
        profile: Hash32::from_bytes(p.profile),
        max_outcomes: 16,
        profile_version: 1,
        stored_bump: 0,
        flags: 0,
    };
    let profile = ProfileAccount {
        profile: realm.profile,
        realm: realm.realm,
        collateral_policy_digest: Hash32::from_bytes(id(13)),
        version: 1,
        flags: PROFILE_FLAG_POLICY_FROZEN,
    };
    let grid = price_grid(p.realm);
    assert_eq!(grid.grid, Hash32::from_bytes(p.price_grid));
    p.validate_current_accounts(&realm, &profile, &grid)
        .unwrap();

    let mut wrong_grid = grid;
    wrong_grid.ticks[1] = 200;
    wrong_grid.grid = canonical_price_grid_id(&{
        let mut body = Vec::new();
        body.extend_from_slice(&wrong_grid.realm.bytes());
        body.extend_from_slice(&wrong_grid.price_scale.to_le_bytes());
        body.push(wrong_grid.tick_count);
        for tick in wrong_grid.ticks {
            body.extend_from_slice(&tick.to_le_bytes());
        }
        body
    });
    assert_eq!(
        p.validate_current_accounts(&realm, &profile, &wrong_grid),
        Err(Error::MismatchedArtifact)
    );
}

#[test]
fn drawdown_feature_is_ordered_conservative_and_associative() {
    let cases = [
        (&[100, 100][..], 0),
        (&[100, 80][..], 200_000),
        (&[100, 120, 90][..], 250_000),
        (&[100, 80, 120][..], 200_000),
        (&[80, 100, 90][..], 100_000),
        (&[3, 2][..], 333_334),
        (&[3, 1][..], 666_667),
        (&[0, 0][..], 0),
        (&[0, 1, 0][..], 1_000_000),
        (&[100, 120, 90, 110, 70][..], 416_667),
    ];
    for (points, expected) in cases {
        assert_eq!(
            fold_points(points).result(),
            DrawdownIntervalV1 {
                low_ppm: expected,
                high_ppm: expected,
            }
        );
    }

    // Equal unordered extrema are insufficient: chronology changes drawdown.
    assert_eq!(fold_points(&[100, 80, 120]).result().high_ppm, 200_000);
    assert_eq!(fold_points(&[100, 120, 80]).result().high_ppm, 333_334);

    let interval = DrawdownSummaryV1::observation(0, 99, 101)
        .unwrap()
        .combine(DrawdownSummaryV1::observation(1, 79, 81).unwrap())
        .unwrap();
    assert_eq!(
        interval.result(),
        DrawdownIntervalV1 {
            low_ppm: 181_819,
            high_ppm: 217_822,
        }
    );
    let rising = DrawdownSummaryV1::observation(0, 90, 110)
        .unwrap()
        .combine(DrawdownSummaryV1::observation(1, 100, 120).unwrap())
        .unwrap();
    assert_eq!(
        rising.result(),
        DrawdownIntervalV1 {
            low_ppm: 0,
            high_ppm: 90_910,
        }
    );

    let a = DrawdownSummaryV1::observation(0, 100, 100).unwrap();
    let b = DrawdownSummaryV1::observation(1, 120, 120).unwrap();
    let c = DrawdownSummaryV1::observation(2, 90, 90).unwrap();
    assert_eq!(
        a.combine(b).unwrap().combine(c).unwrap(),
        a.combine(b.combine(c).unwrap()).unwrap()
    );
    assert_eq!(
        a.combine(DrawdownSummaryV1::observation(2, 90, 90).unwrap()),
        Err(Error::InvalidParameter)
    );

    let p0 = DrawdownSummaryV1::observation(0, 100, 100).unwrap();
    let p1 = DrawdownSummaryV1::observation(1, 120, 120).unwrap();
    let p2 = DrawdownSummaryV1::observation(2, 90, 90).unwrap();
    let p3 = DrawdownSummaryV1::observation(3, 110, 110).unwrap();
    let p4 = DrawdownSummaryV1::observation(4, 70, 70).unwrap();
    let left = p0
        .combine(p1)
        .unwrap()
        .combine(p2)
        .unwrap()
        .combine(p3)
        .unwrap()
        .combine(p4)
        .unwrap();
    let balanced = p0
        .combine(p1)
        .unwrap()
        .combine(p2.combine(p3).unwrap())
        .unwrap()
        .combine(p4)
        .unwrap();
    let right = p0
        .combine(
            p1.combine(p2.combine(p3.combine(p4).unwrap()).unwrap())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(left, balanced);
    assert_eq!(left, right);
}

#[test]
fn presentation_does_not_fork_semantics_and_current_source_plane_refuses_series() {
    let semantic = template(StatisticProgramV1::TerminalInterval);
    let mut relabeled = semantic;
    relabeled.presentation_digest = id(44);
    assert_eq!(semantic.id().unwrap(), relabeled.id().unwrap());
    assert_ne!(
        semantic.presentation_id().unwrap(),
        relabeled.presentation_id().unwrap()
    );

    let legacy = HatcheryProgramV1 {
        source_plane_version: 2,
        capabilities: HATCHERY_SOURCE_ONLY_HEAD,
        ..hatchery()
    };
    assert_eq!(
        legacy.validate_recurring(),
        Err(Error::CurrentSourcePlaneNotRecurring)
    );
}

#[test]
fn identical_instances_converge_across_series_and_advance_without_spending() {
    let t = template(StatisticProgramV1::TerminalInterval);
    let l = liquidity(&t);
    let first = series(&t, &l);
    let mut second = first;
    second.stride_buckets = 240;
    second.instance_count = 2;
    assert_ne!(
        first.id(&t, &work(), &l).unwrap(),
        second.id(&t, &work(), &l).unwrap()
    );
    let existing = compile_instance(
        &first,
        &t,
        &source(),
        &summary(),
        &hatchery(),
        &work(),
        &l,
        0,
    )
    .unwrap();
    let expected = compile_instance(
        &second,
        &t,
        &source(),
        &summary(),
        &hatchery(),
        &work(),
        &l,
        0,
    )
    .unwrap();
    assert_eq!(existing.instance_id, expected.instance_id);
    assert_eq!(existing.market_id, expected.market_id);

    let funding =
        SeriesFundingV1::activate(&second, &t, &work(), &l, 40_000_000, 100_000_000, 500_000)
            .unwrap();
    let advanced = funding
        .advance_existing(
            &second,
            &t,
            &source(),
            &summary(),
            &hatchery(),
            &work(),
            &l,
            &existing,
            990,
        )
        .unwrap();
    assert_eq!(advanced.next_ordinal, 1);
    assert_eq!(advanced.creation_lamports, funding.creation_lamports);
    assert_eq!(advanced.liveness_lamports, funding.liveness_lamports);
    assert_eq!(advanced.liquidity_collateral, funding.liquidity_collateral);
}

#[test]
fn instance_owns_a_monotone_epoch_identity_cursor() {
    let instance_id = id(90);
    let cursor = InstanceEpochCursorV1::new(instance_id).unwrap();
    assert_eq!(cursor.next_epoch_index(), 0);
    assert_eq!(cursor.create_next(1), Err(Error::WrongEpoch));
    let (cursor, first) = cursor.create_next(0).unwrap();
    assert_eq!(cursor.next_epoch_index(), 1);
    assert_eq!(cursor.create_next(0), Err(Error::WrongEpoch));
    let (cursor, second) = cursor.create_next(1).unwrap();
    assert_eq!(cursor.next_epoch_index(), 2);
    assert_ne!(first, second);
    let outcome_zero = canonical_instance_outcome_id(instance_id, 4, 0).unwrap();
    let outcome_one = canonical_instance_outcome_id(instance_id, 4, 1).unwrap();
    assert_ne!(outcome_zero, outcome_one);
    assert_eq!(
        canonical_instance_outcome_id(instance_id, 4, 4),
        Err(Error::InvalidParameter)
    );
}

#[test]
fn reference_identity_vector_is_frozen() {
    let terminal = template(StatisticProgramV1::TerminalInterval);
    let drawdown = template(StatisticProgramV1::MaximumDrawdownInterval);
    let terminal_liquidity = liquidity(&terminal);
    let terminal_series = series(&terminal, &terminal_liquidity);
    let mut terminal_instance = compile_instance(
        &terminal_series,
        &terminal,
        &source(),
        &summary(),
        &hatchery(),
        &work(),
        &terminal_liquidity,
        0,
    )
    .unwrap();
    let terms = lower_current_terms(
        &mut terminal_instance,
        &terminal_series,
        &terminal,
        &source(),
        &summary(),
        &hatchery(),
        &work(),
        &terminal_liquidity,
    )
    .unwrap();
    assert_eq!(
        hex(terminal.id().unwrap()),
        "5f8b8b0829d353f716d598e76829368d2770a94463e0bafe162ab50da66ba20b"
    );
    assert_eq!(
        hex(drawdown.id().unwrap()),
        "e433e62101951359a7e09449181d3314e395818f2d3177b7cdbc801e6a6a5e65"
    );
    assert_eq!(
        hex(terminal_series
            .id(&terminal, &work(), &terminal_liquidity)
            .unwrap()),
        "4fa9658e1fc7e63e9294fca971f029a8d800f25fa55288861314329e08abd3f6"
    );
    assert_eq!(
        hex(terminal_instance.instance_id),
        "95f6df0466f42bfde92e87252e647e241c4ef7b0947a392c2be41b1db2f3f88e"
    );
    assert_eq!(
        hex(terminal_instance.hatchery_window_id),
        "67ac2c2d4001ae7556306f24103766d8729acf2f1ffb96153446759d1b036330"
    );
    assert_eq!(
        hex(terminal_instance.statistic_result_id),
        "f4a2c7dfd21edc4bc7314f1f6468a8f57d9fbf6cd097d1abc4fbd912ba8b530d"
    );
    assert_eq!(
        hex(terminal_instance.current_window_id),
        "3b1e5363443bec7060833c31e1cd7b9e11e99def898900aa76eae2f128a8de2b"
    );
    assert_eq!(
        hex(terms.terms.bytes()),
        "e85a899570149a36a679b27722c4b05fb50afc8534cffe2acd6bd17e4facd2c9"
    );
}
