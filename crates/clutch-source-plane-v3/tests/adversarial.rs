use clutch_source_plane_v3::*;

fn id(byte: u8) -> ContentId {
    ContentId::from_bytes([byte; 32])
}

fn source_plane() -> SourcePlaneProgramV3 {
    SourcePlaneProgramV3 {
        release_id: id(1),
        source_plane_version: 3,
        raw_page_codec_version: 1,
        window_codec_version: 1,
        statistic_result_codec_version: 1,
        capabilities: CAP_SOURCE_ONLY_HEAD
            | CAP_REUSABLE_RAW_PAGES
            | CAP_REALM_NEUTRAL_FEED
            | CAP_STATISTIC_RESULTS,
    }
}

fn summary() -> SummaryProgramV3 {
    SummaryProgramV3 {
        evaluator_release_id: id(3),
        evaluator_version: 1,
        feature_mask: FEATURE_TERMINAL_INTERVAL | FEATURE_DRAWDOWN_INTERVAL,
    }
}

fn payouts() -> PayoutTableV3 {
    let mut vectors = [PayoutVectorV3::ZERO; MAX_PAYOUTS];
    let mut outcome = 0;
    while outcome < 4 {
        let mut weights = [0; MAX_OUTCOMES];
        weights[outcome] = 1_000;
        vectors[outcome] = PayoutVectorV3 {
            denominator: 1_000,
            weights,
        };
        outcome += 1;
    }
    let mut uniform = [0; MAX_OUTCOMES];
    uniform[..4].fill(250);
    vectors[4] = PayoutVectorV3 {
        denominator: 1_000,
        weights: uniform,
    };
    PayoutTableV3 {
        outcome_count: 4,
        payout_count: 5,
        failure_payout_index: 4,
        payouts: vectors,
    }
}

fn partition() -> PartitionViewV3 {
    PartitionViewV3 {
        partition_id: id(4),
        outcome_count: 4,
    }
}

fn template(statistic: StatisticKindV3) -> ProductTemplateV3 {
    ProductTemplateV3 {
        source_plane_program_id: source_plane().id().unwrap(),
        source_spec_id: id(2),
        summary_program_id: summary().id().unwrap(),
        partition_id: partition().partition_id,
        payout_table_id: payouts().id().unwrap(),
        settlement_policy_id: id(5),
        compiler_version: 1,
        statistic,
        coverage_policy_id: 1,
        failure_policy_id: FAILURE_UNIFORM_REFUND_01,
        repair_policy_id: 1,
        window_span_buckets: 4,
        maturity_grace_buckets: 2,
        repair_generation: 0,
        coverage_policy_parameter: 0,
    }
}

fn work() -> WorkEnvelopeV3 {
    WorkEnvelopeV3 {
        version: 1,
        creation_lamports: 10,
        liveness_lamports: 20,
    }
}

fn liquidity() -> LiquidityEnvelopeV3 {
    LiquidityEnvelopeV3 {
        liquidity_policy_id: id(6),
        version: 1,
        collateral_per_instance: 100,
    }
}

fn series(first_start_bucket: u64) -> SeriesPlanV3 {
    let template = template(StatisticKindV3::TerminalInterval);
    SeriesPlanV3 {
        template_id: template.id().unwrap(),
        realm_id: id(7),
        profile_id: id(8),
        price_grid_id: id(9),
        fee_policy_id: id(10),
        work_envelope_id: work().id().unwrap(),
        liquidity_envelope_id: liquidity().id().unwrap(),
        first_start_bucket,
        stride_buckets: 10,
        instance_count: 3,
        creation_lead_buckets: 5,
        market_collateral_cap: 200,
    }
}

fn record(value: u128, sequence: u64, slot: u64, time: u64) -> RawRecordV3 {
    RawRecordV3::observation(value, value, sequence, slot, time)
}

fn sealed_page(head: SourceHeadV3, records: &[RawRecordV3]) -> (RawPageV3, SourceHeadV3) {
    let mut open = head.open_page().unwrap();
    for record in records {
        open = open.append_observation(*record).unwrap();
    }
    let page = open.seal().unwrap();
    let next = head.commit_page(&page).unwrap();
    (page, next)
}

fn window_fixture() -> (WindowSpecV3, WindowSealV3, RawPageV3, SourceHeadV3) {
    let head = SourceHeadV3::new(id(2), 100, 0).unwrap();
    let repeated = record(100, 10, 20, 30);
    let records = [
        repeated,
        repeated,
        record(120, 11, 21, 31),
        record(90, 12, 22, 32),
        record(110, 13, 23, 33),
        record(70, 14, 24, 34),
    ];
    let (page, head) = sealed_page(head, &records);
    let window = WindowSpecV3 {
        source_spec_id: id(2),
        source_plane_program_id: source_plane().id().unwrap(),
        start_bucket: 100,
        end_bucket_exclusive: 104,
        maturity_bucket_exclusive: 106,
        repair_generation: 0,
        coverage_policy_id: 1,
        coverage_policy_parameter: 0,
    };
    let work = WindowWorkV3::new(&window)
        .unwrap()
        .push_page(&window, &page)
        .unwrap();
    let closure = WindowClosureReceiptV3::from_page(&source_plane(), &window, &page).unwrap();
    let seal = work.finish(&window, &closure).unwrap();
    (window, seal, page, head)
}

fn assert_codec<T>(value: &T)
where
    T: FixedCodec + core::fmt::Debug + Eq,
{
    let mut dirty = vec![0xa5; T::ENCODED_LEN];
    value.encode_into(&mut dirty).unwrap();
    assert_eq!(&T::decode(&dirty).unwrap(), value);

    let mut canonical = vec![0; T::ENCODED_LEN];
    value.encode_into(&mut canonical).unwrap();
    assert_eq!(dirty, canonical);
    let mut reencoded = vec![0xa5; T::ENCODED_LEN];
    T::decode(&canonical)
        .unwrap()
        .encode_into(&mut reencoded)
        .unwrap();
    assert_eq!(canonical, reencoded);

    assert_eq!(
        T::decode(&canonical[..canonical.len() - 1]),
        Err(Error::Truncated)
    );
    let mut trailing = canonical.clone();
    trailing.push(0);
    assert_eq!(T::decode(&trailing), Err(Error::TrailingBytes));
    let mut bad_magic = canonical;
    bad_magic[0] ^= 1;
    assert_eq!(T::decode(&bad_magic), Err(Error::BadMagic));
}

#[test]
fn every_fixed_codec_round_trips_and_refuses_length_and_magic() {
    let plane = source_plane();
    let summary = summary();
    let payouts = payouts();
    let template = template(StatisticKindV3::TerminalInterval);
    let work = work();
    let liquidity = liquidity();
    let series = series(100);
    let funding =
        SeriesFundingV3::activate(&series, &template, &work, &liquidity, 30, 60, 300).unwrap();
    let compiled = compile_instance(
        &plane,
        &summary,
        &payouts,
        &partition(),
        &template,
        &work,
        &liquidity,
        &series,
        0,
    )
    .unwrap();
    let (window, seal, page, head) = window_fixture();
    let open = head.open_page().unwrap();
    let window_work = WindowWorkV3::new(&window)
        .unwrap()
        .push_page(&window, &page)
        .unwrap();
    let closure = WindowClosureReceiptV3::from_page(&plane, &window, &page).unwrap();
    let key = StatisticKeyV3 {
        window_id: window.id().unwrap(),
        summary_program_id: summary.id().unwrap(),
        statistic: StatisticKindV3::TerminalInterval,
    };
    let result = StatisticResultV3::terminal(&key, &summary, &seal, &window, 90, 120).unwrap();
    let drawdown = DrawdownSummaryV3::singleton(100, 99, 101).unwrap();

    assert_codec(&plane);
    assert_codec(&head);
    assert_codec(&open);
    assert_codec(&page);
    assert_codec(&window);
    assert_codec(&window_work);
    assert_codec(&closure);
    assert_codec(&seal);
    assert_codec(&summary);
    assert_codec(&key);
    assert_codec(&result);
    assert_codec(&drawdown);
    assert_codec(&payouts);
    assert_codec(&template);
    assert_codec(&work);
    assert_codec(&liquidity);
    assert_codec(&series);
    assert_codec(&funding);
    assert_codec(&compiled.descriptor());
}

#[test]
fn exact_versions_reserved_bytes_and_closed_policies_refuse() {
    let plane = source_plane();
    let mut bytes = vec![0; SOURCE_PLANE_PROGRAM_BYTES];
    plane.encode_into(&mut bytes).unwrap();
    bytes[40..42].copy_from_slice(&4_u16.to_le_bytes());
    assert_eq!(SourcePlaneProgramV3::decode(&bytes), Err(Error::BadVersion));
    plane.encode_into(&mut bytes).unwrap();
    bytes[52] = 1;
    assert_eq!(
        SourcePlaneProgramV3::decode(&bytes),
        Err(Error::NonCanonicalReserved)
    );

    let mut unsupported = template(StatisticKindV3::TerminalInterval);
    unsupported.failure_policy_id = EXTENDED_WINDOW_02;
    assert_eq!(unsupported.validate_shape(), Err(Error::UnsupportedPolicy));
    let mut gapped = template(StatisticKindV3::TerminalInterval);
    gapped.coverage_policy_id = 2;
    gapped.coverage_policy_parameter = 1;
    assert_eq!(gapped.validate_shape(), Err(Error::UnsupportedPolicy));
}

#[test]
fn every_reserved_region_is_enforced_on_hostile_decode() {
    macro_rules! dirty_reserved {
        ($value:expr, $ty:ty, $offset:expr) => {{
            let value = $value;
            let mut bytes = vec![0; <$ty>::ENCODED_LEN];
            value.encode_into(&mut bytes).unwrap();
            bytes[$offset] = 1;
            assert_eq!(
                <$ty>::decode(&bytes),
                Err(Error::NonCanonicalReserved),
                "{} offset {}",
                stringify!($ty),
                $offset
            );
        }};
    }
    let (window, seal, page, head) = window_fixture();
    let open = head.open_page().unwrap();
    let window_work = WindowWorkV3::new(&window)
        .unwrap()
        .push_page(&window, &page)
        .unwrap();
    let closure = WindowClosureReceiptV3::from_page(&source_plane(), &window, &page).unwrap();
    let key = StatisticKeyV3 {
        window_id: window.id().unwrap(),
        summary_program_id: summary().id().unwrap(),
        statistic: StatisticKindV3::TerminalInterval,
    };
    let result = StatisticResultV3::terminal(&key, &summary(), &seal, &window, 90, 120).unwrap();
    let template = template(StatisticKindV3::TerminalInterval);
    let series = series(100);
    let funding =
        SeriesFundingV3::activate(&series, &template, &work(), &liquidity(), 30, 60, 300).unwrap();

    dirty_reserved!(source_plane(), SourcePlaneProgramV3, 52);
    dirty_reserved!(head, SourceHeadV3, 152);
    dirty_reserved!(open, OpenRawPageV3, 65);
    dirty_reserved!(page, RawPageV3, 65);
    dirty_reserved!(window, WindowSpecV3, 106);
    dirty_reserved!(window_work, WindowWorkV3, 174);
    dirty_reserved!(closure, WindowClosureReceiptV3, 120);
    dirty_reserved!(seal, WindowSealV3, 188);
    dirty_reserved!(summary(), SummaryProgramV3, 44);
    dirty_reserved!(key, StatisticKeyV3, 74);
    dirty_reserved!(result, StatisticResultV3, 75);
    dirty_reserved!(payouts(), PayoutTableV3, 11);
    dirty_reserved!(template, ProductTemplateV3, 206);
    dirty_reserved!(work(), WorkEnvelopeV3, 12);
    dirty_reserved!(liquidity(), LiquidityEnvelopeV3, 44);
    dirty_reserved!(series, SeriesPlanV3, 252);
    dirty_reserved!(funding, SeriesFundingV3, 44);
}

#[test]
fn open_page_matches_real_ingestion_and_immutable_tail_does_not_move() {
    let head = SourceHeadV3::new(id(2), 100, 0).unwrap();
    let same = record(100, 50, 70, 90);
    let mut open = head.open_page().unwrap();
    open = open.append_observation(same).unwrap();
    // Current V2 may use one witness for consecutive boundaries.
    open = open.append_observation(same).unwrap();
    // Source-native sequences may jump; the core does not invent `+1`.
    open = open.append_observation(record(110, 900, 800, 700)).unwrap();

    let before = open;
    assert_eq!(
        open.append_observation(record(111, 899, 801, 701)),
        Err(Error::DiscontinuousPage)
    );
    assert_eq!(open, before);
    assert_eq!(
        open.append_observation(record(101, 50, 70, 90)),
        Err(Error::DiscontinuousPage)
    );

    let page = open.seal().unwrap();
    let page_id = page.id().unwrap();
    let head = head.commit_page(&page).unwrap();
    let (window, seal, _, _) = window_fixture();

    let (_, later_head) = sealed_page(head, &[record(120, 901, 801, 701)]);
    assert_ne!(later_head.latest_page_id, page_id);
    assert_eq!(page.id().unwrap(), page_id);
    assert_eq!(seal.window_id, window.id().unwrap());
}

#[test]
fn forged_gap_padding_page_forks_and_repair_mismatch_refuse() {
    let (_, _, page, head) = window_fixture();
    let mut bytes = vec![0; RAW_PAGE_BYTES];
    page.encode_into(&mut bytes).unwrap();
    // First record kind at the 104-byte immutable page header.
    bytes[104] = RawRecordKindV3::Gap as u8;
    bytes[112..168].fill(0);
    assert_eq!(RawPageV3::decode(&bytes), Err(Error::UnsupportedPolicy));

    page.encode_into(&mut bytes).unwrap();
    let first_padding = 104 + usize::from(page.record_count) * RAW_RECORD_BYTES;
    bytes[first_padding] = RawRecordKindV3::Observation as u8;
    assert_eq!(RawPageV3::decode(&bytes), Err(Error::NonCanonicalPadding));

    let mut fork = page;
    fork.page_index = 1;
    fork.previous_page_id = id(99);
    assert_eq!(head.commit_page(&fork), Err(Error::DiscontinuousPage));

    let (window, _, page, _) = window_fixture();
    let mut wrong_generation = page;
    wrong_generation.repair_generation = 1;
    assert_eq!(
        WindowWorkV3::new(&window)
            .unwrap()
            .push_page(&window, &wrong_generation),
        Err(Error::MismatchedArtifact)
    );
}

#[test]
fn window_work_is_resumable_exact_and_maturity_is_not_decorative() {
    let (window, seal, page, _) = window_fixture();
    seal.validate_against(&window).unwrap();

    let premature = WindowSpecV3 {
        maturity_bucket_exclusive: 107,
        ..window
    };
    let work = WindowWorkV3::new(&premature)
        .unwrap()
        .push_page(&premature, &page)
        .unwrap();
    let closure = WindowClosureReceiptV3::from_page(&source_plane(), &window, &page).unwrap();
    assert_eq!(
        work.finish(&premature, &closure),
        Err(Error::MismatchedArtifact)
    );

    let mut work_bytes = vec![0; WINDOW_WORK_BYTES];
    WindowWorkV3::new(&window)
        .unwrap()
        .push_page(&window, &page)
        .unwrap()
        .encode_into(&mut work_bytes)
        .unwrap();
    let resumed = WindowWorkV3::decode(&work_bytes).unwrap();
    let closure = WindowClosureReceiptV3::from_page(&source_plane(), &window, &page).unwrap();
    assert_eq!(resumed.finish(&window, &closure).unwrap(), seal);

    let overlapping = WindowSpecV3 {
        start_bucket: 102,
        end_bucket_exclusive: 106,
        maturity_bucket_exclusive: 106,
        ..window
    };
    let overlap_seal = WindowWorkV3::new(&overlapping)
        .unwrap()
        .push_page(&overlapping, &page)
        .unwrap()
        .finish(
            &overlapping,
            &WindowClosureReceiptV3::from_page(&source_plane(), &overlapping, &page).unwrap(),
        )
        .unwrap();
    assert_eq!(overlap_seal.first_page_id, seal.first_page_id);
    assert_ne!(overlap_seal.window_id, seal.window_id);
}

#[test]
fn statistic_key_is_not_result_content_and_closed_constructors_bind_evaluator() {
    let (window, seal, _, _) = window_fixture();
    let summary = summary();
    let key = StatisticKeyV3 {
        window_id: window.id().unwrap(),
        summary_program_id: summary.id().unwrap(),
        statistic: StatisticKindV3::TerminalInterval,
    };
    let result_a = StatisticResultV3::terminal(&key, &summary, &seal, &window, 90, 120).unwrap();
    let result_b = StatisticResultV3::terminal(&key, &summary, &seal, &window, 91, 120).unwrap();
    assert_eq!(result_a.statistic_key_id(), result_b.statistic_key_id());
    assert_ne!(result_a.id().unwrap(), result_b.id().unwrap());
    result_a
        .validate_against(&key, &summary, &seal, &window)
        .unwrap();
    let wrong_window = WindowSpecV3 {
        start_bucket: 101,
        end_bucket_exclusive: 105,
        maturity_bucket_exclusive: 106,
        ..window
    };
    assert_eq!(
        result_a.validate_against(&key, &summary, &seal, &wrong_window),
        Err(Error::MismatchedArtifact)
    );

    let no_terminal = SummaryProgramV3 {
        feature_mask: FEATURE_DRAWDOWN_INTERVAL,
        ..summary
    };
    let wrong_key = StatisticKeyV3 {
        summary_program_id: no_terminal.id().unwrap(),
        ..key
    };
    assert_eq!(
        StatisticResultV3::terminal(&wrong_key, &no_terminal, &seal, &window, 90, 120),
        Err(Error::MismatchedArtifact)
    );

    let mut bytes = vec![0; STATISTIC_RESULT_BYTES];
    result_a.encode_into(&mut bytes).unwrap();
    // Success status with a nonzero refusal code.
    bytes[76..80].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        StatisticResultV3::decode(&bytes),
        Err(Error::InvalidParameter)
    );
    result_a.encode_into(&mut bytes).unwrap();
    // Unknown status.
    bytes[74] = 9;
    assert_eq!(
        StatisticResultV3::decode(&bytes),
        Err(Error::InvalidParameter)
    );
}

#[test]
fn failure_uniformity_is_data_validation_not_a_policy_name() {
    let good = payouts();
    good.validate_failure_policy(FAILURE_UNIFORM_REFUND_01)
        .unwrap();

    let mut false_uniform = good;
    false_uniform.payouts[4].weights[0] = 249;
    false_uniform.payouts[4].weights[3] = 251;
    false_uniform.validate().unwrap();
    assert_eq!(
        false_uniform.validate_failure_policy(FAILURE_UNIFORM_REFUND_01),
        Err(Error::FailurePayoutNotUniform)
    );

    let mut indivisible = good;
    indivisible.payouts[4].denominator = 1_002;
    indivisible.payouts[4].weights[..4].copy_from_slice(&[250, 250, 250, 252]);
    for payout in &mut indivisible.payouts[..4] {
        payout.denominator = 1_002;
        payout.weights.fill(0);
    }
    indivisible.payouts[0].weights[0] = 1_002;
    indivisible.payouts[1].weights[1] = 1_002;
    indivisible.payouts[2].weights[2] = 1_002;
    indivisible.payouts[3].weights[3] = 1_002;
    indivisible.validate().unwrap();
    assert_eq!(
        indivisible.validate_failure_policy(FAILURE_UNIFORM_REFUND_01),
        Err(Error::FailurePayoutNotUniform)
    );
}

#[test]
fn recurring_lowering_converges_and_validates_last_maturity() {
    let plane = source_plane();
    let summary = summary();
    let payouts = payouts();
    let partition = partition();
    let template = template(StatisticKindV3::TerminalInterval);
    let work = work();
    let liquidity = liquidity();
    let series_a = series(100);
    let series_b = series(90);
    let a = compile_instance(
        &plane, &summary, &payouts, &partition, &template, &work, &liquidity, &series_a, 0,
    )
    .unwrap();
    let b = compile_instance(
        &plane, &summary, &payouts, &partition, &template, &work, &liquidity, &series_b, 1,
    )
    .unwrap();
    assert_eq!(a.instance_id(), b.instance_id());
    assert_eq!(a.descriptor(), b.descriptor());
    assert_eq!(a.window(), b.window());
    assert_ne!(a.series_id(), b.series_id());
    a.validate_against(
        &plane, &summary, &payouts, &partition, &template, &work, &liquidity, &series_a,
    )
    .unwrap();

    let mut long_template = template;
    long_template.window_span_buckets = 60;
    long_template.maturity_grace_buckets = 10;
    let mut last_ok = series_a;
    last_ok.template_id = long_template.id().unwrap();
    last_ok.first_start_bucket = u64::MAX - 90;
    last_ok.stride_buckets = 10;
    last_ok
        .validate_bindings(&long_template, &work, &liquidity)
        .unwrap();
    let mut overflow = last_ok;
    overflow.first_start_bucket = u64::MAX - 89;
    assert_eq!(
        overflow.validate_bindings(&long_template, &work, &liquidity),
        Err(Error::ArithmeticOverflow)
    );
}

#[test]
fn segregated_prepaid_recurrence_has_exact_intervals_and_no_spend_advances() {
    let plane = source_plane();
    let summary = summary();
    let payouts = payouts();
    let partition = partition();
    let template = template(StatisticKindV3::TerminalInterval);
    let work = work();
    let liquidity = liquidity();
    let series_a = series(100);
    assert_eq!(
        SeriesFundingV3::activate(&series_a, &template, &work, &liquidity, 30, 59, 300),
        Err(Error::InsufficientPrepayment)
    );
    let funding =
        SeriesFundingV3::activate(&series_a, &template, &work, &liquidity, 30, 60, 300).unwrap();
    assert_eq!(
        funding.instantiate_next(
            &plane, &summary, &payouts, &partition, &template, &work, &liquidity, &series_a, 1, 99,
        ),
        Err(Error::WrongOrdinal)
    );
    assert_eq!(
        funding.instantiate_next(
            &plane, &summary, &payouts, &partition, &template, &work, &liquidity, &series_a, 0, 94,
        ),
        Err(Error::NotEligible)
    );
    let (spent, _) = funding
        .instantiate_next(
            &plane, &summary, &payouts, &partition, &template, &work, &liquidity, &series_a, 0, 95,
        )
        .unwrap();
    assert_eq!(
        (spent.creation_lamports(), spent.liveness_lamports()),
        (20, 40)
    );

    let series_b = series(90);
    let existing = compile_instance(
        &plane, &summary, &payouts, &partition, &template, &work, &liquidity, &series_b, 1,
    )
    .unwrap();
    let advanced = funding
        .advance_existing(
            &plane, &summary, &payouts, &partition, &template, &work, &liquidity, &series_a,
            &existing, 99,
        )
        .unwrap();
    assert_eq!(advanced.next_ordinal(), 1);
    assert_eq!(advanced.creation_lamports(), 30);
    let lapsed = funding
        .lapse_next(&series_a, &template, &work, &liquidity, 100)
        .unwrap();
    assert_eq!(lapsed.next_ordinal(), 1);
    assert_eq!(lapsed.liquidity_collateral(), 300);
}

#[test]
fn drawdown_is_ordered_conservative_associative_and_exactly_rounded() {
    fn fold(values: &[(u128, u128)]) -> DrawdownSummaryV3 {
        let mut summary = DrawdownSummaryV3::singleton(0, values[0].0, values[0].1).unwrap();
        for (index, (low, high)) in values.iter().copied().enumerate().skip(1) {
            summary = summary
                .combine(DrawdownSummaryV3::singleton(index as u64, low, high).unwrap())
                .unwrap();
        }
        summary
    }
    assert_eq!(fold(&[(100, 100), (80, 80)]).interval().low_ppm, 200_000);
    assert_eq!(fold(&[(3, 3), (2, 2)]).interval().low_ppm, 333_334);
    assert_eq!(fold(&[(1, 1), (0, 0)]).interval().low_ppm, 1_000_000);
    assert_eq!(
        fold(&[(1_000_000, 1_000_000), (999_999, 999_999)])
            .interval()
            .low_ppm,
        1
    );
    assert_eq!(
        fold(&[(99, 101), (79, 81)]).interval(),
        DrawdownIntervalV3 {
            low_ppm: 181_819,
            high_ppm: 217_822,
        }
    );
    let points = [(100, 100), (120, 120), (90, 90), (110, 110), (70, 70)];
    let left = fold(&points);
    let a = fold(&points[..2]);
    let b = fold(&points[2..]);
    // Rebase the second range to its actual buckets before combining.
    let b0 = DrawdownSummaryV3::singleton(2, 90, 90)
        .unwrap()
        .combine(DrawdownSummaryV3::singleton(3, 110, 110).unwrap())
        .unwrap()
        .combine(DrawdownSummaryV3::singleton(4, 70, 70).unwrap())
        .unwrap();
    assert_eq!(left, a.combine(b0).unwrap());
    assert_eq!(left.interval().low_ppm, 416_667);
    assert_eq!(
        fold(&[(99, 101), (119, 121), (89, 91), (109, 111), (69, 71)]).interval(),
        DrawdownIntervalV3 {
            low_ppm: 403_362,
            high_ppm: 429_753,
        }
    );
    assert_eq!(
        fold(&[
            (MAX_SOURCE_VALUE, MAX_SOURCE_VALUE),
            (MAX_SOURCE_VALUE - 1, MAX_SOURCE_VALUE - 1)
        ])
        .interval()
        .low_ppm,
        1
    );
    assert_eq!(
        DrawdownSummaryV3::singleton(0, 1, 1)
            .unwrap()
            .combine(DrawdownSummaryV3::singleton(2, 0, 0).unwrap()),
        Err(Error::DiscontinuousPage)
    );
    let _ = b;
}

#[test]
fn frozen_reference_manifest_matches_canonical_bytes_and_domains() {
    fn hex(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use core::fmt::Write;
            write!(&mut out, "{byte:02x}").unwrap();
        }
        out
    }
    fn encoded<T: FixedCodec>(value: &T) -> Vec<u8> {
        let mut bytes = vec![0; T::ENCODED_LEN];
        value.encode_into(&mut bytes).unwrap();
        bytes
    }
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../vectors/source-plane-v3.json")).unwrap();
    let plane = source_plane();
    let template = template(StatisticKindV3::TerminalInterval);
    let series = series(100);
    let instance = compile_instance(
        &plane,
        &summary(),
        &payouts(),
        &partition(),
        &template,
        &work(),
        &liquidity(),
        &series,
        0,
    )
    .unwrap();
    let (window, seal, page, _) = window_fixture();
    let key = StatisticKeyV3 {
        window_id: window.id().unwrap(),
        summary_program_id: summary().id().unwrap(),
        statistic: StatisticKindV3::TerminalInterval,
    };
    let result = StatisticResultV3::terminal(&key, &summary(), &seal, &window, 90, 120).unwrap();
    let vectors = &manifest["vectors"];
    assert_eq!(
        vectors["source_plane_program"]["body_hex"],
        hex(&encoded(&plane))
    );
    assert_eq!(
        vectors["source_plane_program"]["id"],
        hex(&plane.id().unwrap().bytes())
    );
    assert_eq!(vectors["template"]["body_hex"], hex(&encoded(&template)));
    assert_eq!(
        vectors["template"]["id"],
        hex(&template.id().unwrap().bytes())
    );
    assert_eq!(vectors["series"]["id"], hex(&series.id().unwrap().bytes()));
    assert_eq!(vectors["window"]["body_hex"], hex(&encoded(&window)));
    assert_eq!(vectors["window"]["id"], hex(&window.id().unwrap().bytes()));
    assert_eq!(vectors["page"]["id"], hex(&page.id().unwrap().bytes()));
    assert_eq!(
        vectors["window_seal"]["id"],
        hex(&seal.id().unwrap().bytes())
    );
    assert_eq!(vectors["statistic_key"]["body_hex"], hex(&encoded(&key)));
    assert_eq!(
        vectors["statistic_key"]["id"],
        hex(&key.id().unwrap().bytes())
    );
    assert_eq!(
        vectors["statistic_result"]["id"],
        hex(&result.id().unwrap().bytes())
    );
    assert_eq!(
        vectors["instance"]["body_hex"],
        hex(&encoded(&instance.descriptor()))
    );
    assert_eq!(
        vectors["instance"]["id"],
        hex(&instance.instance_id().bytes())
    );
}
