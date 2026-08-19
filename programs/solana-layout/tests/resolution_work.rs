#![forbid(unsafe_code)]

#[path = "../src/resolution_work.rs"]
mod resolution_work;

use resolution_work::*;

fn id(byte: u8) -> [u8; HASH_BYTES] {
    [byte; HASH_BYTES]
}

fn basis_artifact() -> [u8; BASIS_SPEC_BYTES_V1] {
    let mut bytes = [0; BASIS_SPEC_BYTES_V1];
    bytes[..8].copy_from_slice(b"DCBASV01");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&BASIS_EVALUATOR_VERSION_V1.to_le_bytes());
    bytes[12] = 1;
    bytes[13] = 3;
    bytes[14] = 1;
    bytes[15] = 3;
    bytes[16] = 3;
    bytes[17] = 1;
    bytes[24..32].copy_from_slice(&257_u64.to_le_bytes());
    bytes[32..48].copy_from_slice(&16_u128.to_le_bytes());
    for (index, knot) in [0_u128, 8, 16].into_iter().enumerate() {
        let at = 48 + (index * 16);
        bytes[at..at + 16].copy_from_slice(&knot.to_le_bytes());
    }
    bytes
}

fn costs() -> ResolutionWorkCostScheduleV1 {
    ResolutionWorkCostScheduleV1 {
        version: RESOLUTION_WORK_COST_VERSION_V1,
        work_state_bytes: RESOLUTION_WORK_ACCOUNT_BYTES as u32,
        rent_reserve: 100,
        minimum_lifetime_slots: 10,
        begin_charge: 3,
        fold_base_charge: 2,
        fold_per_record_charge: 1,
        fold_base_reward: 1,
        fold_per_record_reward: 1,
        finalize_charge: 5,
        finalize_reward: 2,
        abort_charge: 4,
        abort_reward: 1,
    }
}

fn account() -> ResolutionWorkAccountV1 {
    let mut masses = [0; MAX_OUTCOMES];
    masses[0] = 257;
    masses[1] = 128;
    masses[2] = 129;
    ResolutionWorkAccountV1 {
        work_id: id(1),
        payer: id(2),
        prepaid_reserve: id(3),
        work_nonce: id(4),
        market: id(5),
        terms_digest: id(6),
        resolution_target: id(7),
        program_owner: id(8),
        archive_account: id(9),
        basis_spec_digest: id(10),
        source_spec_digest: id(11),
        archive_commitment: id(12),
        archive_domain_digest: id(13),
        grid_identity: id(14),
        basis_spec_artifact: basis_artifact(),
        archive_generation: 7,
        bucket_duration: 60,
        start_bucket: 40,
        end_bucket_exclusive: 44,
        opened_slot: 10,
        expires_slot: 20,
        last_progress_slot: 12,
        next_bucket: 42,
        fold_count: 1,
        completion_slot: 0,
        sample_count: 2,
        coverage_count: 2,
        denominator: 257,
        masses,
        costs: costs(),
        cost_schedule_digest: id(15),
        funding: ResolutionWorkFundingV1 {
            deposited: 200,
            rent_locked: 100,
            prepaid_remaining: 90,
            charges_paid: 7,
            rewards_paid: 3,
        },
        status: WORK_STATUS_ACTIVE,
        finalization_mode: FINALIZATION_LARGEST_REMAINDER_V1,
        outcome_count: 3,
        archive_record_count: 4,
        basis_evaluator_version: BASIS_EVALUATOR_VERSION_V1,
        occupation_summary_version: OCCUPATION_SUMMARY_VERSION_V1,
        resolution_version: OCCUPATION_RESOLUTION_VERSION_V4,
        stored_bump: 201,
        reserve_bump: 202,
        flags: 0,
        reserved: [0; 3],
    }
}

#[test]
fn proposed_tags_do_not_collide_with_the_committed_registry_at_isolation() {
    // Account tags committed when this isolated codec was cut. Artifact-stage
    // tag 0x21 (33) is intentionally included even though the dense account
    // registry otherwise ends at direct-window tag 21.
    let committed_account_tags = [
        1_u8, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 33,
    ];
    assert!(!committed_account_tags.contains(&RESOLUTION_WORK_ACCOUNT_TAG));
    assert_eq!(RESOLUTION_WORK_ACCOUNT_TAG, 22);

    let proposed_intent_tags = [
        BEGIN_RESOLUTION_WORK_TAG,
        FOLD_RESOLUTION_WORK_TAG,
        FINALIZE_RESOLUTION_WORK_TAG,
        ABORT_RESOLUTION_WORK_TAG,
    ];
    for committed in 1_u8..=31 {
        assert!(!proposed_intent_tags.contains(&committed));
    }
    assert_eq!(proposed_intent_tags, [32, 33, 34, 35]);
}

#[test]
fn active_account_round_trips_at_the_pinned_1296_byte_shape() {
    let value = account();
    value.validate().unwrap();
    let mut bytes = [0; RESOLUTION_WORK_ACCOUNT_BYTES];
    assert_eq!(value.encode(&mut bytes).unwrap(), bytes.len());
    assert_eq!(bytes[0], RESOLUTION_WORK_ACCOUNT_TAG);
    assert_eq!(bytes[1], RESOLUTION_WORK_ACCOUNT_VERSION);
    assert_eq!(ResolutionWorkAccountV1::decode(&bytes).unwrap(), value);

    assert_eq!(
        value.encode(&mut bytes[..RESOLUTION_WORK_ACCOUNT_BYTES - 1]),
        Err(ResolutionWorkCodecError::OutputTooSmall)
    );
    assert_eq!(
        ResolutionWorkAccountV1::decode(&bytes[..bytes.len() - 1]),
        Err(ResolutionWorkCodecError::Truncated)
    );
    let mut trailing = [0; RESOLUTION_WORK_ACCOUNT_BYTES + 1];
    trailing[..RESOLUTION_WORK_ACCOUNT_BYTES].copy_from_slice(&bytes);
    assert_eq!(
        ResolutionWorkAccountV1::decode(&trailing),
        Err(ResolutionWorkCodecError::TrailingBytes)
    );
}

#[test]
fn account_refuses_terminal_status_wrong_header_and_noncanonical_padding() {
    let mut value = account();
    value.status = 2;
    assert_eq!(value.validate(), Err(ResolutionWorkCodecError::InvalidEnum));
    value = account();
    value.reserved[2] = 1;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::NonCanonicalPadding)
    );

    let mut bytes = [0; RESOLUTION_WORK_ACCOUNT_BYTES];
    account().encode(&mut bytes).unwrap();
    bytes[0] ^= 1;
    assert_eq!(
        ResolutionWorkAccountV1::decode(&bytes),
        Err(ResolutionWorkCodecError::WrongTag)
    );
    bytes[0] = RESOLUTION_WORK_ACCOUNT_TAG;
    bytes[1] = 2;
    assert_eq!(
        ResolutionWorkAccountV1::decode(&bytes),
        Err(ResolutionWorkCodecError::WrongVersion)
    );
}

#[test]
fn cursor_count_order_completion_and_lifetime_mutants_refuse() {
    let mut value = account();
    value.next_bucket = 43;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::InvalidCount)
    );
    value = account();
    value.sample_count = 3;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::InvalidCount)
    );
    value = account();
    value.fold_count = 3;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::InvalidCount)
    );
    value = account();
    value.completion_slot = 12;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::MismatchedBinding)
    );
    value = account();
    value.expires_slot = 19;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::InvalidWindow)
    );

    value = account();
    value.next_bucket = value.end_bucket_exclusive;
    value.sample_count = 4;
    value.coverage_count = 4;
    value.masses[0] += 257;
    value.masses[1] += 128;
    value.masses[2] += 129;
    value.funding.charges_paid = 9;
    value.funding.rewards_paid = 5;
    value.funding.prepaid_remaining = 86;
    value.completion_slot = value.last_progress_slot;
    value.validate().unwrap();
    value.completion_slot -= 1;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::MismatchedBinding)
    );
}

#[test]
fn checked_mass_basis_and_inactive_padding_mutants_refuse() {
    let mut value = account();
    value.masses[0] += 1;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::MismatchedBinding)
    );
    value = account();
    value.masses[3] = 257;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::NonCanonicalPadding)
    );
    value = account();
    value.masses[0] = u128::MAX;
    value.masses[1] = 1;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::ArithmeticOverflow)
    );
    value = account();
    value.basis_spec_artifact[10] = 2;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::WrongVersion)
    );
    value = account();
    value.basis_spec_artifact[18] = 1;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::NonCanonicalPadding)
    );
    value = account();
    value.basis_spec_artifact[24..32].copy_from_slice(&258_u64.to_le_bytes());
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::MismatchedBinding)
    );
}

#[test]
fn prefund_schedule_and_exact_ledger_are_not_advisory() {
    assert_eq!(costs().minimum_deposit(4).unwrap(), 130);
    let mut value = account();
    value.funding.deposited = 129;
    value.funding.prepaid_remaining = 19;
    assert_eq!(value.validate(), Err(ResolutionWorkCodecError::Underfunded));
    value = account();
    value.funding.charges_paid += 1;
    value.funding.prepaid_remaining -= 1;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::MismatchedBinding)
    );
    value = account();
    value.costs.work_state_bytes -= 1;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::MismatchedBinding)
    );
    value = account();
    value.costs.fold_base_charge = u64::MAX;
    assert_eq!(
        value.validate(),
        Err(ResolutionWorkCodecError::ArithmeticOverflow)
    );
}

#[test]
fn all_four_intents_round_trip_at_exact_pinned_lengths() {
    let begin = BeginResolutionWorkV1 {
        work_nonce: id(1),
        finalization_mode: FINALIZATION_EXACT_ONLY,
        expires_slot: 55,
        declared_deposit: 200,
        cost_schedule_digest: id(2),
    };
    let mut begin_bytes = [0; BEGIN_RESOLUTION_WORK_BYTES];
    assert_eq!(begin.encode(&mut begin_bytes).unwrap(), 83);
    assert_eq!(BeginResolutionWorkV1::decode(&begin_bytes).unwrap(), begin);

    let fold = FoldResolutionWorkV1 {
        work_id: id(3),
        archive_account: id(4),
        archive_commitment: id(5),
        expected_cursor: 40,
        record_count: MAX_FOLD_RECORDS_V1,
    };
    let mut fold_bytes = [0; FOLD_RESOLUTION_WORK_BYTES];
    assert_eq!(fold.encode(&mut fold_bytes).unwrap(), 107);
    assert_eq!(FoldResolutionWorkV1::decode(&fold_bytes).unwrap(), fold);

    let finalize = FinalizeResolutionWorkV1 {
        work_id: id(6),
        expected_cursor: 44,
        expected_archive_commitment: id(7),
    };
    let mut finalize_bytes = [0; FINALIZE_RESOLUTION_WORK_BYTES];
    assert_eq!(finalize.encode(&mut finalize_bytes).unwrap(), 74);
    assert_eq!(
        FinalizeResolutionWorkV1::decode(&finalize_bytes).unwrap(),
        finalize
    );

    let abort = AbortResolutionWorkV1 {
        work_id: id(8),
        expected_cursor: 42,
        expected_archive_commitment: id(9),
    };
    let mut abort_bytes = [0; ABORT_RESOLUTION_WORK_BYTES];
    assert_eq!(abort.encode(&mut abort_bytes).unwrap(), 74);
    assert_eq!(AbortResolutionWorkV1::decode(&abort_bytes).unwrap(), abort);
}

#[test]
fn fold_has_no_caller_data_channel_and_terminal_tags_do_not_alias() {
    let fold = FoldResolutionWorkV1 {
        work_id: id(1),
        archive_account: id(2),
        archive_commitment: id(3),
        expected_cursor: 40,
        record_count: 1,
    };
    let mut bytes = [0; FOLD_RESOLUTION_WORK_BYTES];
    fold.encode(&mut bytes).unwrap();
    assert_eq!(bytes.len(), 2 + (3 * HASH_BYTES) + 8 + 1);
    assert_eq!(
        FoldResolutionWorkV1::decode(&bytes[..bytes.len() - 1]),
        Err(ResolutionWorkCodecError::Truncated)
    );
    bytes[1] = RESOLUTION_WORK_INTENT_VERSION - 1;
    assert_eq!(
        FoldResolutionWorkV1::decode(&bytes),
        Err(ResolutionWorkCodecError::WrongVersion)
    );

    let mut invalid = fold;
    invalid.record_count = 0;
    assert_eq!(
        invalid.encode(&mut bytes),
        Err(ResolutionWorkCodecError::InvalidCount)
    );
    invalid.record_count = MAX_FOLD_RECORDS_V1 + 1;
    assert_eq!(
        invalid.encode(&mut bytes),
        Err(ResolutionWorkCodecError::InvalidCount)
    );

    let abort = AbortResolutionWorkV1 {
        work_id: id(4),
        expected_cursor: 42,
        expected_archive_commitment: id(5),
    };
    let mut terminal = [0; ABORT_RESOLUTION_WORK_BYTES];
    abort.encode(&mut terminal).unwrap();
    assert_eq!(
        FinalizeResolutionWorkV1::decode(&terminal),
        Err(ResolutionWorkCodecError::WrongTag)
    );
}

#[test]
fn full_archive_identity_and_commitment_are_distinct_optimistic_guards() {
    let first = FoldResolutionWorkV1 {
        work_id: id(1),
        archive_account: id(2),
        archive_commitment: id(3),
        expected_cursor: 40,
        record_count: 2,
    };
    let mut alternate_account = first;
    alternate_account.archive_account = id(4);
    let mut alternate_commitment = first;
    alternate_commitment.archive_commitment = id(5);
    let mut first_bytes = [0; FOLD_RESOLUTION_WORK_BYTES];
    let mut account_bytes = [0; FOLD_RESOLUTION_WORK_BYTES];
    let mut commitment_bytes = [0; FOLD_RESOLUTION_WORK_BYTES];
    first.encode(&mut first_bytes).unwrap();
    alternate_account.encode(&mut account_bytes).unwrap();
    alternate_commitment.encode(&mut commitment_bytes).unwrap();
    assert_ne!(first_bytes, account_bytes);
    assert_ne!(first_bytes, commitment_bytes);
    assert_eq!(
        FoldResolutionWorkV1::decode(&first_bytes)
            .unwrap()
            .archive_account,
        id(2)
    );
    assert_eq!(
        FoldResolutionWorkV1::decode(&first_bytes)
            .unwrap()
            .archive_commitment,
        id(3)
    );
}
