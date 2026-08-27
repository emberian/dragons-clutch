use clutch_bspline::{BasisSpec, EdgePolicy, MAX_KNOTS, UNIFORM_SPACING_NONE};
use clutch_bspline_accumulator::{
    BasisDomain, FinalizationMode, SequentialSummaryBuilder, BASIS_EVALUATOR_VERSION,
    OCCUPATION_SUMMARY_VERSION,
};
use clutch_resolution_work_v1::*;

#[derive(Clone, Debug)]
struct ArchiveFixture {
    config: ArchiveAccountConfigV1,
    archive: ArchiveAccountV1,
    receipt: ArchiveReceiptV1,
    records: Vec<ArchiveRecordV1>,
}

impl ArchiveFixture {
    fn new(start: u64, observations: &[ArchiveObservationV1]) -> Self {
        let end = start + observations.len() as u64;
        let records: Vec<_> = observations
            .iter()
            .enumerate()
            .map(|(offset, observation)| ArchiveRecordV1 {
                bucket: start + offset as u64,
                observation: *observation,
            })
            .collect();
        let config = ArchiveAccountConfigV1 {
            account_key: id(0x24),
            owner: id(0x25),
            data_len: SOURCE_ARCHIVE_ACCOUNT_V1_BYTES,
            executable: false,
            receipt_version: ARCHIVE_RECEIPT_VERSION,
            source_spec_digest: id(0x21),
            archive_domain_digest: id(0x22),
            grid_identity: id(0x23),
            bucket_duration: 60,
            archive_generation: 7,
            start_bucket: start,
            end_bucket_exclusive: end,
        };
        let mut archive = ArchiveAccountV1::new(config);
        for observation in observations {
            archive.append(*observation).unwrap();
        }
        let receipt = archive.seal().unwrap();
        Self {
            config,
            archive,
            receipt,
            records,
        }
    }

    fn fold_request(&self, work: &ResolutionWorkV1, offset: usize, count: usize) -> FoldRequestV1 {
        assert!(count <= MAX_FOLD_RECORDS);
        FoldRequestV1 {
            work_commitment: work.work_commitment(),
            archive_account: self.receipt.archive_account,
            archive_digest: self.receipt.archive_digest,
            expected_cursor: self.records[offset].bucket,
            record_count: count as u8,
        }
    }
}

fn id(tag: u8) -> Id {
    let mut result = [tag; ID_BYTES];
    result[31] ^= 0x5a;
    result
}

fn knots(values: &[u128]) -> [u128; MAX_KNOTS] {
    let mut result = [0; MAX_KNOTS];
    result[..values.len()].copy_from_slice(values);
    result
}

fn spec(degree: u8, denominator: u64) -> BasisSpec {
    match degree {
        0 => BasisSpec {
            outcome_count: 3,
            degree,
            knot_count: 2,
            uniform_log2_spacing: UNIFORM_SPACING_NONE,
            denominator,
            domain_max: 24,
            edge_policy: EdgePolicy::Clamp,
            knots: knots(&[8, 16]),
        },
        1 => BasisSpec {
            outcome_count: 3,
            degree,
            knot_count: 3,
            uniform_log2_spacing: 3,
            denominator,
            domain_max: 16,
            edge_policy: EdgePolicy::Clamp,
            knots: knots(&[0, 8, 16]),
        },
        2 | 3 => BasisSpec {
            outcome_count: 2 + degree,
            degree,
            knot_count: 3,
            uniform_log2_spacing: 3,
            denominator,
            domain_max: 16,
            edge_policy: EdgePolicy::Clamp,
            knots: knots(&[0, 8, 16]),
        },
        _ => unreachable!(),
    }
}

fn costs() -> CostScheduleV1 {
    CostScheduleV1 {
        version: RESOLUTION_WORK_VERSION,
        work_state_bytes: 1_024,
        rent_reserve: 10_000,
        minimum_lifetime_slots: 8,
        begin_charge: 101,
        fold_base_charge: 17,
        fold_per_record_charge: 23,
        fold_base_reward: 29,
        fold_per_record_reward: 31,
        finalize_charge: 37,
        finalize_reward: 41,
        abort_charge: 43,
        abort_reward: 47,
    }
}

fn request(basis: BasisSpec, fixture: &ArchiveFixture, mode: FinalizationMode) -> BeginRequestV1 {
    let schedule = costs();
    let deposit = schedule
        .minimum_deposit(fixture.receipt.record_count)
        .unwrap()
        + 777;
    BeginRequestV1 {
        market: MarketBindingV4 {
            market: id(0x11),
            terms_digest: id(0x12),
            resolution_target: id(0x13),
            program_owner: fixture.receipt.archive_owner,
            archive_account: fixture.receipt.archive_account,
            basis_spec_digest: basis_spec_digest(&basis),
            source_spec_digest: fixture.receipt.source_spec_digest,
            archive_digest: fixture.receipt.archive_digest,
            archive_domain_digest: fixture.receipt.archive_domain_digest,
            archive_generation: fixture.receipt.archive_generation,
            start_bucket: fixture.receipt.start_bucket,
            end_bucket_exclusive: fixture.receipt.end_bucket_exclusive,
            basis_evaluator_version: BASIS_EVALUATOR_VERSION,
            occupation_summary_version: OCCUPATION_SUMMARY_VERSION,
            resolution_version: RESOLUTION_V4_VERSION,
        },
        basis_spec: basis,
        archive: fixture.receipt,
        finalization_mode: mode,
        costs: schedule,
        deposit,
        payer: id(0x14),
        prepaid_reserve: id(0x16),
        work_nonce: id(0x15),
        current_slot: 100,
        expires_slot: 200,
    }
}

fn fold_partition(work: &mut ResolutionWorkV1, fixture: &ArchiveFixture, sizes: &[usize]) {
    let mut offset = 0;
    for (call, size) in sizes.iter().enumerate() {
        let receipt = work
            .fold(
                fixture.fold_request(work, offset, *size),
                &fixture.archive,
                id(0x70 + call as u8),
                101 + call as u64,
            )
            .unwrap();
        assert_eq!(receipt.start_bucket, fixture.records[offset].bucket);
        assert_eq!(
            receipt.end_bucket_exclusive,
            receipt.start_bucket + *size as u64
        );
        offset += *size;
    }
    assert_eq!(offset, fixture.records.len());
}

#[test]
fn basis_projection_matches_the_existing_host_artifact_golden() {
    let mut golden = spec(2, 2);
    golden.uniform_log2_spacing = 2;
    golden.domain_max = 8;
    golden.knots = knots(&[0, 4, 8]);
    let bytes = encode_basis_spec_v1(&golden);
    assert_eq!(bytes.len(), BASIS_SPEC_BYTES_V1);
    assert_eq!(&bytes[0..8], b"DCBASV01");
    assert_eq!(&bytes[8..12], &[1, 0, 1, 0]);
    assert_eq!(&bytes[12..18], &[1, 4, 2, 3, 2, 1]);
    assert_eq!(&bytes[18..24], &[0; 6]);
    assert_eq!(&bytes[24..32], &2_u64.to_le_bytes());
    assert_eq!(&bytes[32..48], &8_u128.to_le_bytes());
    assert_eq!(&bytes[48..64], &0_u128.to_le_bytes());
    assert_eq!(&bytes[64..80], &4_u128.to_le_bytes());
    assert_eq!(&bytes[80..96], &8_u128.to_le_bytes());
    assert!(bytes[96..].iter().all(|byte| *byte == 0));
    assert_eq!(
        basis_spec_digest(&golden),
        [
            0x5a, 0x38, 0x6d, 0x58, 0x16, 0x4a, 0xf5, 0xdc, 0x97, 0x59, 0xfd, 0x14, 0xe4, 0xfd,
            0x24, 0x74, 0x2e, 0x71, 0x07, 0x2f, 0xd2, 0xe9, 0xa4, 0x59, 0x8d, 0x65, 0x85, 0x3a,
            0x83, 0x00, 0x4e, 0xe6,
        ]
    );
}

fn every_bounded_partition(total: usize) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    for separators in 0..(1_usize << (total - 1)) {
        let mut partition = Vec::new();
        let mut run = 1_usize;
        let mut valid = true;
        for boundary in 0..(total - 1) {
            if ((separators >> boundary) & 1) == 1 {
                valid &= run <= MAX_FOLD_RECORDS;
                partition.push(run);
                run = 1;
            } else {
                run += 1;
            }
        }
        valid &= run <= MAX_FOLD_RECORDS;
        partition.push(run);
        if valid {
            result.push(partition);
        }
    }
    result
}

#[test]
fn chunk_partitions_match_monolithic_accumulator_for_every_degree() {
    let observations = [
        ArchiveObservationV1::Accepted(0),
        ArchiveObservationV1::Accepted(4),
        ArchiveObservationV1::Accepted(8),
        ArchiveObservationV1::Accepted(12),
        ArchiveObservationV1::Accepted(16),
    ];
    let fixture = ArchiveFixture::new(40, &observations);
    for degree in 0..=3 {
        let basis = spec(degree, 257);
        let domain = BasisDomain::new(
            basis_spec_digest(&basis),
            fixture.receipt.grid_identity,
            fixture.receipt.bucket_duration,
            basis,
        )
        .unwrap();
        let mut monolithic = SequentialSummaryBuilder::new(domain).unwrap();
        for record in &fixture.records {
            let ArchiveObservationV1::Accepted(point) = record.observation else {
                unreachable!()
            };
            monolithic.append_accepted(record.bucket, point).unwrap();
        }
        let monolithic = monolithic.finish();
        let expected = monolithic
            .finalize(FinalizationMode::LargestRemainderV1)
            .unwrap();

        for partition in every_bounded_partition(observations.len()) {
            let begin = request(basis, &fixture, FinalizationMode::LargestRemainderV1);
            let deposited = begin.deposit;
            let mut work = ResolutionWorkV1::begin(begin, &fixture.archive).unwrap();
            fold_partition(&mut work, &fixture, &partition);
            assert_eq!(work.summary(), monolithic);
            let finalized = work.finalize(id(0x66), 200).unwrap();
            assert_eq!(finalized.payer, begin.payer);
            assert_eq!(
                finalized.resolution.payout.active_len,
                expected.active_len()
            );
            assert_eq!(
                finalized.resolution.payout.denominator,
                expected.denominator()
            );
            assert_eq!(finalized.resolution.payout.weights, expected.weights());
            assert_eq!(work.status(), WorkStatusV1::Finalized);
            let funding = work.funding();
            assert_eq!(
                funding.charges_paid() + funding.rewards_paid() + funding.refund_paid(),
                deposited
            );
            assert_eq!(funding.rent_locked(), 0);
            assert_eq!(funding.prepaid_remaining(), 0);
        }
    }
}

#[test]
fn aggregation_is_commutative_but_protocol_order_and_archive_identity_are_not() {
    let forward = ArchiveFixture::new(
        8,
        &[
            ArchiveObservationV1::Accepted(1),
            ArchiveObservationV1::Accepted(7),
            ArchiveObservationV1::Accepted(13),
        ],
    );
    let reverse = ArchiveFixture::new(
        8,
        &[
            ArchiveObservationV1::Accepted(13),
            ArchiveObservationV1::Accepted(7),
            ArchiveObservationV1::Accepted(1),
        ],
    );
    assert_ne!(
        forward.receipt.archive_digest,
        reverse.receipt.archive_digest
    );

    let basis = spec(3, 257);
    let mut first = ResolutionWorkV1::begin(
        request(basis, &forward, FinalizationMode::LargestRemainderV1),
        &forward.archive,
    )
    .unwrap();
    let mut second = ResolutionWorkV1::begin(
        request(basis, &reverse, FinalizationMode::LargestRemainderV1),
        &reverse.archive,
    )
    .unwrap();
    fold_partition(&mut first, &forward, &[3]);
    fold_partition(&mut second, &reverse, &[1, 2]);
    let first_payout = first.finalize(id(0x61), 200).unwrap().resolution.payout;
    let second_payout = second.finalize(id(0x62), 200).unwrap().resolution.payout;
    assert_eq!(first_payout, second_payout);
    assert_ne!(
        first.resolution().unwrap().resolution_commitment,
        second.resolution().unwrap().resolution_commitment
    );
}

#[test]
fn alternate_archive_same_domain_cannot_substitute() {
    let original = ArchiveFixture::new(
        30,
        &[
            ArchiveObservationV1::Accepted(1),
            ArchiveObservationV1::Accepted(2),
            ArchiveObservationV1::Accepted(3),
        ],
    );
    let alternate = ArchiveFixture::new(
        30,
        &[
            ArchiveObservationV1::Accepted(1),
            ArchiveObservationV1::Accepted(9),
            ArchiveObservationV1::Accepted(3),
        ],
    );
    assert_eq!(
        original.receipt.archive_domain_digest,
        alternate.receipt.archive_domain_digest
    );
    assert_ne!(
        original.receipt.archive_digest,
        alternate.receipt.archive_digest
    );
    let mut work = ResolutionWorkV1::begin(
        request(
            spec(2, 257),
            &original,
            FinalizationMode::LargestRemainderV1,
        ),
        &original.archive,
    )
    .unwrap();
    let before = work.clone();
    let alternate_request = alternate.fold_request(&work, 0, 1);
    assert_eq!(
        work.fold(alternate_request, &alternate.archive, id(0x51), 110),
        Err(Error::BindingMismatch)
    );
    assert_eq!(work, before);

    let forged_request = original.fold_request(&work, 0, 2);
    let before = work.clone();
    assert_eq!(
        work.fold(forged_request, &alternate.archive, id(0x51), 110),
        Err(Error::BindingMismatch)
    );
    assert_eq!(work, before);
}

#[test]
fn wrong_cursor_bounds_identity_and_replay_are_atomic() {
    let fixture = ArchiveFixture::new(
        100,
        &[
            ArchiveObservationV1::Accepted(0),
            ArchiveObservationV1::Accepted(4),
            ArchiveObservationV1::Accepted(8),
            ArchiveObservationV1::Accepted(12),
        ],
    );
    let mut work = ResolutionWorkV1::begin(
        request(spec(1, 257), &fixture, FinalizationMode::LargestRemainderV1),
        &fixture.archive,
    )
    .unwrap();

    let mut wrong_cursor = fixture.fold_request(&work, 0, 1);
    wrong_cursor.expected_cursor += 1;
    let before = work.clone();
    assert_eq!(
        work.fold(wrong_cursor, &fixture.archive, id(0x50), 110),
        Err(Error::WrongCursor)
    );
    assert_eq!(work, before);

    let mut zero = fixture.fold_request(&work, 0, 1);
    zero.record_count = 0;
    let before = work.clone();
    assert_eq!(
        work.fold(zero, &fixture.archive, id(0x50), 110),
        Err(Error::InvalidChunk)
    );
    assert_eq!(work, before);

    let mut oversized = fixture.fold_request(&work, 0, 1);
    oversized.record_count = (MAX_FOLD_RECORDS + 1) as u8;
    let before = work.clone();
    assert_eq!(
        work.fold(oversized, &fixture.archive, id(0x50), 110),
        Err(Error::InvalidChunk)
    );
    assert_eq!(work, before);

    let mut wrong_work = fixture.fold_request(&work, 0, 1);
    wrong_work.work_commitment = id(0xee);
    let before = work.clone();
    assert_eq!(
        work.fold(wrong_work, &fixture.archive, id(0x50), 110),
        Err(Error::BindingMismatch)
    );
    assert_eq!(work, before);

    let mut replay_work = ResolutionWorkV1::begin(
        request(spec(1, 257), &fixture, FinalizationMode::LargestRemainderV1),
        &fixture.archive,
    )
    .unwrap();
    let first = fixture.fold_request(&replay_work, 0, 1);
    replay_work
        .fold(first, &fixture.archive, id(0x50), 110)
        .unwrap();
    let after_first = replay_work.clone();
    assert_eq!(
        replay_work.fold(first, &fixture.archive, id(0x50), 111),
        Err(Error::WrongCursor)
    );
    assert_eq!(replay_work, after_first);

    work.fold(
        fixture.fold_request(&work, 0, 3),
        &fixture.archive,
        id(0x50),
        110,
    )
    .unwrap();
    let mut past_end = fixture.fold_request(&work, 3, 1);
    past_end.record_count = 2;
    let before = work.clone();
    assert_eq!(
        work.fold(past_end, &fixture.archive, id(0x50), 111),
        Err(Error::WrongRecordOrder)
    );
    assert_eq!(work, before);
}

#[test]
fn mutable_basis_receipt_and_versions_refuse_at_begin() {
    let fixture = ArchiveFixture::new(
        1,
        &[
            ArchiveObservationV1::Accepted(4),
            ArchiveObservationV1::Accepted(8),
        ],
    );
    let original = spec(2, 257);

    let mut changed_basis = request(original, &fixture, FinalizationMode::LargestRemainderV1);
    changed_basis.basis_spec.denominator = 263;
    assert_eq!(
        ResolutionWorkV1::begin(changed_basis, &fixture.archive),
        Err(Error::BindingMismatch)
    );

    let mut changed_version = request(original, &fixture, FinalizationMode::LargestRemainderV1);
    changed_version.market.basis_evaluator_version += 1;
    assert_eq!(
        ResolutionWorkV1::begin(changed_version, &fixture.archive),
        Err(Error::UnsupportedVersion)
    );

    let mut changed_receipt = request(original, &fixture, FinalizationMode::LargestRemainderV1);
    changed_receipt.archive.grid_identity = id(0xa1);
    assert_eq!(
        ResolutionWorkV1::begin(changed_receipt, &fixture.archive),
        Err(Error::BindingMismatch)
    );

    let mut changed_receipt_version =
        request(original, &fixture, FinalizationMode::LargestRemainderV1);
    changed_receipt_version.archive.receipt_version += 1;
    assert_eq!(
        ResolutionWorkV1::begin(changed_receipt_version, &fixture.archive),
        Err(Error::UnsupportedVersion)
    );

    let mut changed_summary_version =
        request(original, &fixture, FinalizationMode::LargestRemainderV1);
    changed_summary_version.market.occupation_summary_version += 1;
    assert_eq!(
        ResolutionWorkV1::begin(changed_summary_version, &fixture.archive),
        Err(Error::UnsupportedVersion)
    );

    let mut changed_resolution_version =
        request(original, &fixture, FinalizationMode::LargestRemainderV1);
    changed_resolution_version.market.resolution_version += 1;
    assert_eq!(
        ResolutionWorkV1::begin(changed_resolution_version, &fixture.archive),
        Err(Error::UnsupportedVersion)
    );

    let unsealed = request(original, &fixture, FinalizationMode::LargestRemainderV1);
    let mut unsealed_archive = ArchiveAccountV1::new(fixture.config);
    for record in &fixture.records {
        unsealed_archive.append(record.observation).unwrap();
    }
    assert_eq!(
        ResolutionWorkV1::begin(unsealed, &unsealed_archive),
        Err(Error::InvalidArchive)
    );
}

#[test]
fn underfunding_and_cost_overflow_refuse_without_a_work_state() {
    let fixture = ArchiveFixture::new(1, &[ArchiveObservationV1::Accepted(4)]);
    let mut underfunded = request(spec(1, 257), &fixture, FinalizationMode::LargestRemainderV1);
    underfunded.deposit = underfunded
        .costs
        .minimum_deposit(fixture.receipt.record_count)
        .unwrap()
        - 1;
    assert_eq!(
        ResolutionWorkV1::begin(underfunded, &fixture.archive),
        Err(Error::Underfunded)
    );

    let mut overflowing = costs();
    overflowing.fold_base_charge = u64::MAX;
    assert_eq!(
        overflowing.minimum_deposit(2),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(
        costs().minimum_deposit(u64::MAX),
        Err(Error::ArithmeticOverflow)
    );
}

#[test]
fn premature_finalize_successful_finalize_and_terminal_replays_are_atomic() {
    let fixture = ArchiveFixture::new(
        4,
        &[
            ArchiveObservationV1::Accepted(4),
            ArchiveObservationV1::Accepted(12),
        ],
    );
    let mut work = ResolutionWorkV1::begin(
        request(spec(3, 256), &fixture, FinalizationMode::LargestRemainderV1),
        &fixture.archive,
    )
    .unwrap();
    let before = work.clone();
    assert_eq!(work.finalize(id(0x55), 110), Err(Error::NotAtEnd));
    assert_eq!(work, before);

    fold_partition(&mut work, &fixture, &[1, 1]);
    let before = work.clone();
    assert_eq!(
        work.finalize([0; ID_BYTES], 300),
        Err(Error::InvalidIdentity)
    );
    assert_eq!(work, before);
    assert_eq!(work.finalize(id(0x55), 100), Err(Error::InvalidSlot));
    assert_eq!(work, before);
    let finalized = work.finalize(id(0x55), 300).unwrap();
    assert_ne!(finalized.resolution.resolution_commitment, [0; ID_BYTES]);
    let terminal = work.clone();
    assert_eq!(work.finalize(id(0x55), 301), Err(Error::AlreadyTerminal));
    assert_eq!(work, terminal);
    let terminal_fold = fixture.fold_request(&work, 0, 1);
    assert_eq!(
        work.fold(terminal_fold, &fixture.archive, id(0x56), 301),
        Err(Error::AlreadyTerminal)
    );
    assert_eq!(work, terminal);
    assert_eq!(work.abort(id(0x57), 301), Err(Error::AlreadyTerminal));
    assert_eq!(work, terminal);
}

#[test]
fn gaps_and_refused_points_are_explicit_and_never_become_a_payout() {
    let gaps = ArchiveFixture::new(
        20,
        &[
            ArchiveObservationV1::Accepted(4),
            ArchiveObservationV1::Missing(MissingReasonV1::ConfidenceRefused),
            ArchiveObservationV1::Accepted(12),
        ],
    );
    let mut work = ResolutionWorkV1::begin(
        request(spec(2, 257), &gaps, FinalizationMode::LargestRemainderV1),
        &gaps.archive,
    )
    .unwrap();
    fold_partition(&mut work, &gaps, &[3]);
    assert_eq!(work.summary().gap_count(), 1);
    let before = work.clone();
    assert_eq!(work.finalize(id(0x61), 200), Err(Error::IncompleteCoverage));
    assert_eq!(work, before);
    let aborted = work.abort(id(0x62), 200).unwrap();
    assert_eq!(aborted.reason, AbortReasonV1::CompleteWithGaps);
    assert_eq!(work.status(), WorkStatusV1::Aborted);

    let refused = ArchiveFixture::new(9, &[ArchiveObservationV1::Accepted(17)]);
    let mut refusing_basis = spec(3, 257);
    refusing_basis.edge_policy = EdgePolicy::Refuse;
    let mut refusing = ResolutionWorkV1::begin(
        request(
            refusing_basis,
            &refused,
            FinalizationMode::LargestRemainderV1,
        ),
        &refused.archive,
    )
    .unwrap();
    let before = refusing.clone();
    let refusing_fold = refused.fold_request(&refusing, 0, 1);
    assert_eq!(
        refusing.fold(refusing_fold, &refused.archive, id(0x63), 110),
        Err(Error::PointRefused)
    );
    assert_eq!(refusing, before);
}

#[test]
fn abort_is_narrow_and_refunds_only_from_the_prepaid_ledger() {
    let fixture = ArchiveFixture::new(
        5,
        &[
            ArchiveObservationV1::Accepted(1),
            ArchiveObservationV1::Accepted(2),
        ],
    );
    let begin = request(spec(1, 257), &fixture, FinalizationMode::LargestRemainderV1);
    let deposit = begin.deposit;
    let mut unstarted = ResolutionWorkV1::begin(begin, &fixture.archive).unwrap();
    let aborted = unstarted.abort(begin.payer, 100).unwrap();
    assert_eq!(aborted.reason, AbortReasonV1::Unstarted);
    assert_eq!(aborted.payer, begin.payer);
    let ledger = unstarted.funding();
    assert_eq!(
        ledger.charges_paid() + ledger.rewards_paid() + ledger.refund_paid(),
        deposit
    );

    let mut partial = ResolutionWorkV1::begin(begin, &fixture.archive).unwrap();
    partial
        .fold(
            fixture.fold_request(&partial, 0, 1),
            &fixture.archive,
            id(0x72),
            110,
        )
        .unwrap();
    let before = partial.clone();
    assert_eq!(partial.abort(id(0x73), 111), Err(Error::AbortForbidden));
    assert_eq!(partial, before);

    partial
        .fold(
            fixture.fold_request(&partial, 1, 1),
            &fixture.archive,
            id(0x74),
            112,
        )
        .unwrap();
    let before = partial.clone();
    assert_eq!(partial.abort(id(0x75), 201), Err(Error::AbortForbidden));
    assert_eq!(partial, before);
}

#[test]
fn expiry_slot_rollback_and_refund_authority_are_exact() {
    let fixture = ArchiveFixture::new(
        15,
        &[
            ArchiveObservationV1::Accepted(1),
            ArchiveObservationV1::Accepted(2),
            ArchiveObservationV1::Accepted(3),
        ],
    );
    let begin = request(spec(1, 257), &fixture, FinalizationMode::LargestRemainderV1);

    let mut unstarted = ResolutionWorkV1::begin(begin, &fixture.archive).unwrap();
    let before = unstarted.clone();
    assert_eq!(unstarted.abort(id(0xd1), 150), Err(Error::AbortForbidden));
    assert_eq!(unstarted, before);
    let expired_unstarted = unstarted.abort(id(0xd1), 201).unwrap();
    assert_eq!(expired_unstarted.reason, AbortReasonV1::Unstarted);
    assert_eq!(expired_unstarted.payer, begin.payer);

    let mut partial = ResolutionWorkV1::begin(begin, &fixture.archive).unwrap();
    partial
        .fold(
            fixture.fold_request(&partial, 0, 1),
            &fixture.archive,
            id(0xd2),
            150,
        )
        .unwrap();
    let second = fixture.fold_request(&partial, 1, 1);
    let before = partial.clone();
    assert_eq!(
        partial.fold(second, &fixture.archive, id(0xd3), 149),
        Err(Error::InvalidSlot)
    );
    assert_eq!(partial, before);
    assert_eq!(
        partial.fold(second, &fixture.archive, id(0xd3), 201),
        Err(Error::Expired)
    );
    assert_eq!(partial, before);
    let expired = partial.abort(id(0xd4), 201).unwrap();
    assert_eq!(expired.reason, AbortReasonV1::ExpiredIncomplete);
    assert_eq!(expired.payer, begin.payer);

    let mut short = begin;
    short.expires_slot = short.current_slot + short.costs.minimum_lifetime_slots - 1;
    assert_eq!(
        ResolutionWorkV1::begin(short, &fixture.archive),
        Err(Error::InvalidSlot)
    );
}

#[test]
fn exact_only_inexact_completion_can_close_as_a_named_refusal() {
    let fixture = ArchiveFixture::new(
        50,
        &[
            ArchiveObservationV1::Accepted(4),
            ArchiveObservationV1::Accepted(8),
        ],
    );
    let mut work = ResolutionWorkV1::begin(
        request(spec(1, 257), &fixture, FinalizationMode::ExactOnly),
        &fixture.archive,
    )
    .unwrap();
    fold_partition(&mut work, &fixture, &[2]);
    assert_eq!(work.finalize(id(0x81), 200), Err(Error::InexactAverage));
    let aborted = work.abort(id(0x82), 200).unwrap();
    assert_eq!(aborted.reason, AbortReasonV1::CompleteInexactAverage);
}

#[test]
fn sealed_archive_has_no_postseal_record_mutation_path() {
    let fixture = ArchiveFixture::new(
        70,
        &[
            ArchiveObservationV1::Accepted(2),
            ArchiveObservationV1::Accepted(6),
        ],
    );
    let mut sealed = fixture.archive.clone();
    let before = sealed.clone();
    assert_eq!(
        sealed.append(ArchiveObservationV1::Accepted(3)),
        Err(Error::InvalidArchive)
    );
    assert_eq!(sealed, before);
    assert_eq!(sealed.seal(), Err(Error::InvalidArchive));
    assert_eq!(sealed, before);

    let mut work = ResolutionWorkV1::begin(
        request(spec(2, 257), &fixture, FinalizationMode::LargestRemainderV1),
        &fixture.archive,
    )
    .unwrap();
    work.fold(
        fixture.fold_request(&work, 0, 2),
        &fixture.archive,
        id(0x91),
        110,
    )
    .unwrap();
    assert_eq!(work.summary().sample_count(), 2);
}

#[test]
fn archive_runtime_metadata_owner_and_explicit_capacity_refuse() {
    let fixture = ArchiveFixture::new(
        70,
        &[
            ArchiveObservationV1::Accepted(2),
            ArchiveObservationV1::Accepted(6),
        ],
    );

    let mut wrong_length_config = fixture.config;
    wrong_length_config.data_len -= 1;
    let mut wrong_length = ArchiveAccountV1::new(wrong_length_config);
    wrong_length
        .append(ArchiveObservationV1::Accepted(2))
        .unwrap();
    wrong_length
        .append(ArchiveObservationV1::Accepted(6))
        .unwrap();
    let before = wrong_length.clone();
    assert_eq!(wrong_length.seal(), Err(Error::InvalidArchive));
    assert_eq!(wrong_length, before);

    let mut executable_config = fixture.config;
    executable_config.executable = true;
    let mut executable = ArchiveAccountV1::new(executable_config);
    executable
        .append(ArchiveObservationV1::Accepted(2))
        .unwrap();
    executable
        .append(ArchiveObservationV1::Accepted(6))
        .unwrap();
    assert_eq!(executable.seal(), Err(Error::InvalidArchive));

    let mut wrong_owner_config = fixture.config;
    wrong_owner_config.owner = id(0xe1);
    let mut wrong_owner = ArchiveAccountV1::new(wrong_owner_config);
    wrong_owner
        .append(ArchiveObservationV1::Accepted(2))
        .unwrap();
    wrong_owner
        .append(ArchiveObservationV1::Accepted(6))
        .unwrap();
    wrong_owner.seal().unwrap();
    assert_eq!(
        ResolutionWorkV1::begin(
            request(spec(2, 257), &fixture, FinalizationMode::LargestRemainderV1,),
            &wrong_owner,
        ),
        Err(Error::BindingMismatch)
    );

    let mut too_wide_config = fixture.config;
    too_wide_config.start_bucket = 0;
    too_wide_config.end_bucket_exclusive = SOURCE_ARCHIVE_MAX_RECORDS_V1 as u64 + 1;
    let mut too_wide = ArchiveAccountV1::new(too_wide_config);
    for _ in 0..SOURCE_ARCHIVE_MAX_RECORDS_V1 {
        too_wide.append(ArchiveObservationV1::Accepted(1)).unwrap();
    }
    assert_eq!(
        too_wide.append(ArchiveObservationV1::Accepted(1)),
        Err(Error::InvalidArchive)
    );
    assert_eq!(too_wide.seal(), Err(Error::InvalidArchive));
}

#[test]
fn batching_saves_only_frozen_per_call_cost_and_never_changes_semantics() {
    let fixture = ArchiveFixture::new(
        90,
        &[
            ArchiveObservationV1::Accepted(0),
            ArchiveObservationV1::Accepted(4),
            ArchiveObservationV1::Accepted(8),
            ArchiveObservationV1::Accepted(12),
        ],
    );
    let begin = request(spec(3, 257), &fixture, FinalizationMode::LargestRemainderV1);
    let mut singleton = ResolutionWorkV1::begin(begin, &fixture.archive).unwrap();
    let mut batched = ResolutionWorkV1::begin(begin, &fixture.archive).unwrap();
    fold_partition(&mut singleton, &fixture, &[1, 1, 1, 1]);
    fold_partition(&mut batched, &fixture, &[4]);
    let singleton_output = singleton.finalize(id(0xa1), 200).unwrap();
    let batched_output = batched.finalize(id(0xa2), 200).unwrap();
    assert_eq!(
        singleton_output.resolution.payout,
        batched_output.resolution.payout
    );
    let saved_calls = 3;
    let expected_saving =
        saved_calls * (begin.costs.fold_base_charge + begin.costs.fold_base_reward);
    assert_eq!(
        batched.funding().refund_paid() - singleton.funding().refund_paid(),
        expected_saving
    );
}

#[test]
fn largest_representable_end_cursor_is_processed_without_wraparound() {
    let fixture = ArchiveFixture::new(
        u64::MAX - 2,
        &[
            ArchiveObservationV1::Accepted(4),
            ArchiveObservationV1::Accepted(8),
        ],
    );
    assert_eq!(fixture.receipt.end_bucket_exclusive, u64::MAX);
    let mut work = ResolutionWorkV1::begin(
        request(spec(1, 256), &fixture, FinalizationMode::LargestRemainderV1),
        &fixture.archive,
    )
    .unwrap();
    fold_partition(&mut work, &fixture, &[1, 1]);
    assert_eq!(work.next_bucket(), u64::MAX);
    work.finalize(id(0xc1), 200).unwrap();
}
