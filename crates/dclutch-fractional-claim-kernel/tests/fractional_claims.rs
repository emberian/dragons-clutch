//! Adversarial exact-shard, explicit-change, and lifecycle tests.

#![allow(clippy::panic, clippy::unwrap_used)]

use dclutch_fractional_claim_kernel::{
    Error, FRACTIONAL_PROJECTION_HEADER_BYTES_V1, FRACTIONAL_PROJECTION_MAGIC_V1,
    FRACTIONAL_PROJECTION_ROW_BYTES_V1, FRACTIONAL_TERMS_HEADER_BYTES_V1,
    FRACTIONAL_TERMS_MAGIC_V1, FRACTIONAL_TERMS_MINT_BYTES_V1, FractionalPhaseV1,
    FractionalProjectionV1, FractionalTermsAdmissionV1, FractionalTermsV1, SCHEMA_VERSION_V1,
    TransferObservationV1, divide_claim_shards_v1, prepare_open_unwrap_v1, prepare_retire_v1,
    prepare_terminal_redeem_v1, prepare_terminal_zero_burn_v1, prepare_terminalize_v1,
    prepare_transfer_v1, prepare_wrap_v1,
};

const OUTCOMES: u32 = 3;
const DENOMINATOR: u64 = 10;
const TERMS_ID: [u8; 32] = [91; 32];

#[derive(Clone, Copy)]
enum PhaseFixture {
    Open,
    Terminal(u32),
    Retired,
}

fn identity(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    let end = offset.checked_add(value.len()).expect("fixture width");
    output
        .get_mut(offset..end)
        .expect("fixture destination")
        .copy_from_slice(value);
}

fn terms_bytes(denominator: u64) -> Vec<u8> {
    let tail = usize::try_from(OUTCOMES)
        .expect("outcome width")
        .checked_mul(FRACTIONAL_TERMS_MINT_BYTES_V1)
        .expect("mint tail");
    let mut output = vec![0_u8; FRACTIONAL_TERMS_HEADER_BYTES_V1 + tail];
    put(&mut output, 0, &FRACTIONAL_TERMS_MAGIC_V1);
    put(&mut output, 8, &SCHEMA_VERSION_V1.to_le_bytes());
    put(&mut output, 16, &identity(1));
    put(&mut output, 48, &identity(2));
    put(&mut output, 80, &identity(3));
    put(&mut output, 112, &identity(4));
    put(&mut output, 144, &identity(5));
    put(&mut output, 176, &OUTCOMES.to_le_bytes());
    put(&mut output, 184, &denominator.to_le_bytes());
    for outcome in 0..OUTCOMES {
        let offset = FRACTIONAL_TERMS_HEADER_BYTES_V1
            + usize::try_from(outcome).expect("outcome") * FRACTIONAL_TERMS_MINT_BYTES_V1;
        put(
            &mut output,
            offset,
            &identity(u8::try_from(outcome + 11).expect("mint seed")),
        );
    }
    output
}

fn admission() -> FractionalTermsAdmissionV1 {
    FractionalTermsAdmissionV1 {
        selected_terms_id: TERMS_ID,
        finalized_terms_id: TERMS_ID,
        recomputed_terms_digest: TERMS_ID,
        finalized_terms_digest: TERMS_ID,
        record_authenticated: true,
    }
}

fn decode_terms<'a>(bytes: &'a [u8]) -> FractionalTermsV1<'a> {
    FractionalTermsV1::decode(bytes, admission()).expect("valid terms")
}

fn projection_bytes(phase: PhaseFixture, revision: u64, rows: &[(u64, u64)]) -> Vec<u8> {
    let tail = rows
        .len()
        .checked_mul(FRACTIONAL_PROJECTION_ROW_BYTES_V1)
        .expect("row tail");
    let mut output = vec![0_u8; FRACTIONAL_PROJECTION_HEADER_BYTES_V1 + tail];
    put(&mut output, 0, &FRACTIONAL_PROJECTION_MAGIC_V1);
    put(&mut output, 8, &SCHEMA_VERSION_V1.to_le_bytes());
    let (tag, terminal) = match phase {
        PhaseFixture::Open => (0_u8, u32::MAX),
        PhaseFixture::Terminal(winner) => (1_u8, winner),
        PhaseFixture::Retired => (2_u8, u32::MAX),
    };
    put(&mut output, 10, &[tag]);
    put(&mut output, 16, &TERMS_ID);
    put(&mut output, 48, &identity(1));
    put(
        &mut output,
        80,
        &u32::try_from(rows.len()).expect("row count").to_le_bytes(),
    );
    put(&mut output, 84, &terminal.to_le_bytes());
    put(&mut output, 88, &revision.to_le_bytes());
    for (outcome, (native, supply)) in rows.iter().copied().enumerate() {
        let offset =
            FRACTIONAL_PROJECTION_HEADER_BYTES_V1 + outcome * FRACTIONAL_PROJECTION_ROW_BYTES_V1;
        put(&mut output, offset, &native.to_le_bytes());
        put(&mut output, offset + 8, &supply.to_le_bytes());
    }
    output
}

#[test]
fn immutable_terms_bind_every_authority_and_unique_outcome_mint() {
    let bytes = terms_bytes(DENOMINATOR);
    let terms = decode_terms(&bytes);
    assert_eq!(terms.terms_id(), TERMS_ID);
    assert_eq!(terms.market_id(), identity(1));
    assert_eq!(terms.result_domain_id(), identity(2));
    assert_eq!(terms.release_set_id(), identity(3));
    assert_eq!(terms.token_program(), identity(4));
    assert_eq!(terms.token_behavior_selection_id(), identity(5));
    assert_eq!(terms.outcome_count(), OUTCOMES);
    assert_eq!(terms.denominator(), DENOMINATOR);
    assert_eq!(terms.shard_mint(2), Ok(identity(13)));
    assert_eq!(terms.shard_mint(3), Err(Error::InvalidOutcome));
}

#[test]
fn hostile_terms_refuse_substitution_duplicates_and_noncanonical_bytes() {
    let bytes = terms_bytes(DENOMINATOR);
    let mut substituted = admission();
    substituted.finalized_terms_digest = identity(77);
    assert_eq!(
        FractionalTermsV1::decode(&bytes, substituted),
        Err(Error::AdmissionMismatch)
    );
    let mut unauthenticated = admission();
    unauthenticated.record_authenticated = false;
    assert_eq!(
        FractionalTermsV1::decode(&bytes, unauthenticated),
        Err(Error::UnauthenticatedRecord)
    );

    let mut duplicate = bytes.clone();
    let first = duplicate
        .get(FRACTIONAL_TERMS_HEADER_BYTES_V1..FRACTIONAL_TERMS_HEADER_BYTES_V1 + 32)
        .expect("first mint")
        .to_vec();
    put(
        &mut duplicate,
        FRACTIONAL_TERMS_HEADER_BYTES_V1 + 32,
        &first,
    );
    assert_eq!(
        FractionalTermsV1::decode(&duplicate, admission()),
        Err(Error::DuplicateShardMint)
    );

    let mut reserved = bytes.clone();
    *reserved.get_mut(11).expect("reserved byte") = 1;
    assert_eq!(
        FractionalTermsV1::decode(&reserved, admission()),
        Err(Error::NonCanonical)
    );
    assert_eq!(
        FractionalTermsV1::decode(&terms_bytes(1), admission()),
        Err(Error::NonFractionalDenominator)
    );
    assert_eq!(
        FractionalTermsV1::decode(
            bytes.get(..bytes.len() - 1).expect("truncated fixture"),
            admission()
        ),
        Err(Error::InvalidLength)
    );
}

#[test]
fn phase_dependent_projection_checks_every_outcome_reserve() {
    let terms_bytes = terms_bytes(DENOMINATOR);
    let terms = decode_terms(&terms_bytes);
    let open_bytes = projection_bytes(PhaseFixture::Open, 7, &[(2, 20), (3, 30), (0, 0)]);
    let open = FractionalProjectionV1::decode(&open_bytes, terms).expect("open projection");
    assert_eq!(open.phase(), FractionalPhaseV1::Open);
    assert_eq!(open.revision(), 7);
    assert_eq!(open.reserve(1).expect("reserve").locked_native_claims, 3);

    let bad_open = projection_bytes(PhaseFixture::Open, 7, &[(2, 19), (3, 30), (0, 0)]);
    assert_eq!(
        FractionalProjectionV1::decode(&bad_open, terms),
        Err(Error::ReserveMismatch)
    );

    let terminal_bytes =
        projection_bytes(PhaseFixture::Terminal(1), 8, &[(2, 13), (3, 30), (4, 0)]);
    let terminal =
        FractionalProjectionV1::decode(&terminal_bytes, terms).expect("terminal projection");
    assert_eq!(
        terminal.phase(),
        FractionalPhaseV1::Terminal { winning_outcome: 1 }
    );

    let bad_winner = projection_bytes(PhaseFixture::Terminal(1), 8, &[(2, 13), (3, 29), (4, 0)]);
    assert_eq!(
        FractionalProjectionV1::decode(&bad_winner, terms),
        Err(Error::ReserveMismatch)
    );
    let overissued_loser =
        projection_bytes(PhaseFixture::Terminal(1), 8, &[(2, 21), (3, 30), (4, 0)]);
    assert_eq!(
        FractionalProjectionV1::decode(&overissued_loser, terms),
        Err(Error::ReserveMismatch)
    );
}

#[test]
fn wrap_mints_exact_denominator_multiple_and_preserves_reserve() {
    let terms_bytes = terms_bytes(DENOMINATOR);
    let terms = decode_terms(&terms_bytes);
    let projection_bytes = projection_bytes(PhaseFixture::Open, 7, &[(2, 20), (3, 30), (0, 0)]);
    let projection = FractionalProjectionV1::decode(&projection_bytes, terms).expect("projection");
    let plan = prepare_wrap_v1(terms, projection, 0, 2, 5, 3).expect("wrap");
    assert_eq!(plan.native_claims_to_lock, 2);
    assert_eq!(plan.shards_to_mint.shard_atoms, 20);
    assert_eq!(plan.shards_to_mint.shard_mint, identity(11));
    assert_eq!(plan.post_reserve.locked_native_claims, 4);
    assert_eq!(plan.post_reserve.shard_supply, 40);
    assert_eq!(plan.post_actor_native_claims, 3);
    assert_eq!(plan.post_actor_shards, 23);
    assert_eq!(plan.next_revision, 8);
    assert_eq!(
        prepare_wrap_v1(terms, projection, 0, 6, 5, 3),
        Err(Error::InsufficientBalance)
    );
}

#[test]
fn transfer_uses_token_balances_without_mutating_wrapper_revision_or_supply() {
    let terms_bytes = terms_bytes(DENOMINATOR);
    let terms = decode_terms(&terms_bytes);
    let projection_bytes = projection_bytes(PhaseFixture::Open, 7, &[(2, 20), (3, 30), (0, 0)]);
    let projection = FractionalProjectionV1::decode(&projection_bytes, terms).expect("projection");
    let plan = prepare_transfer_v1(
        terms,
        projection,
        0,
        7,
        TransferObservationV1 {
            source_account: identity(30),
            destination_account: identity(31),
            source_shards: 13,
            destination_shards: 4,
        },
    )
    .expect("transfer");
    assert_eq!(plan.post_source_shards, 6);
    assert_eq!(plan.post_destination_shards, 11);
    assert_eq!(plan.unchanged_revision, 7);
    assert_eq!(plan.shards_to_transfer.shard_atoms, 7);
    assert_eq!(
        prepare_transfer_v1(
            terms,
            projection,
            0,
            1,
            TransferObservationV1 {
                source_account: identity(30),
                destination_account: identity(30),
                source_shards: 13,
                destination_shards: 4,
            },
        ),
        Err(Error::AccountAlias)
    );
}

#[test]
fn sole_division_boundary_returns_explicit_same_mint_change() {
    let terms_bytes = terms_bytes(DENOMINATOR);
    let terms = decode_terms(&terms_bytes);
    let division = divide_claim_shards_v1(terms, 2, 27).expect("division");
    assert_eq!(division.whole_native_claims, 2);
    assert_eq!(division.consumed_shards.shard_atoms, 20);
    assert_eq!(division.change_shards.shard_atoms, 7);
    assert_eq!(division.input_shards.shard_mint, identity(13));
    assert_eq!(division.consumed_shards.shard_mint, identity(13));
    assert_eq!(division.change_shards.shard_mint, identity(13));
    assert_eq!(
        division.input_shards.shard_atoms,
        division.consumed_shards.shard_atoms + division.change_shards.shard_atoms
    );
}

#[test]
fn open_unwrap_burns_only_whole_multiple_and_leaves_change_token_owned() {
    let terms_bytes = terms_bytes(DENOMINATOR);
    let terms = decode_terms(&terms_bytes);
    let projection_bytes = projection_bytes(PhaseFixture::Open, 9, &[(4, 40), (3, 30), (0, 0)]);
    let projection = FractionalProjectionV1::decode(&projection_bytes, terms).expect("projection");
    let plan = prepare_open_unwrap_v1(terms, projection, 0, 17, 25).expect("unwrap");
    assert_eq!(plan.division.whole_native_claims, 1);
    assert_eq!(plan.division.consumed_shards.shard_atoms, 10);
    assert_eq!(plan.division.change_shards.shard_atoms, 7);
    assert_eq!(plan.post_actor_shards, 15);
    assert_eq!(plan.post_reserve.locked_native_claims, 3);
    assert_eq!(plan.post_reserve.shard_supply, 30);
    assert_eq!(plan.native_claims_to_actor, 1);
    assert_eq!(plan.collateral_atoms_to_actor, 0);
    assert_eq!(
        prepare_open_unwrap_v1(terms, projection, 0, 9, 25),
        Err(Error::NoWholeClaim)
    );
}

#[test]
fn terminal_winner_redeems_whole_claims_and_loser_burns_only_for_zero() {
    let terms_bytes = terms_bytes(DENOMINATOR);
    let terms = decode_terms(&terms_bytes);
    let projection_bytes =
        projection_bytes(PhaseFixture::Terminal(1), 12, &[(2, 13), (3, 30), (4, 40)]);
    let projection = FractionalProjectionV1::decode(&projection_bytes, terms).expect("terminal");

    let winning = prepare_terminal_redeem_v1(terms, projection, 1, 19, 25).expect("winning redeem");
    assert_eq!(winning.division.consumed_shards.shard_atoms, 10);
    assert_eq!(winning.division.change_shards.shard_atoms, 9);
    assert_eq!(winning.post_reserve.locked_native_claims, 2);
    assert_eq!(winning.post_reserve.shard_supply, 20);
    assert_eq!(winning.native_claims_to_actor, 0);
    assert_eq!(winning.collateral_atoms_to_actor, 1);

    assert_eq!(
        prepare_terminal_redeem_v1(terms, projection, 0, 10, 13),
        Err(Error::InvalidPhase)
    );
    let losing = prepare_terminal_zero_burn_v1(terms, projection, 0, 7, 13).expect("losing burn");
    assert_eq!(losing.shards_to_burn.shard_atoms, 7);
    assert_eq!(losing.post_reserve.locked_native_claims, 2);
    assert_eq!(losing.post_reserve.shard_supply, 6);
    assert_eq!(losing.cumulative_zero_burned_shards, 14);
    assert_eq!(
        prepare_terminal_zero_burn_v1(terms, projection, 1, 1, 25),
        Err(Error::InvalidPhase)
    );
}

#[test]
fn terminalization_preserves_reserves_and_retirement_requires_zero_supply() {
    let terms_bytes = terms_bytes(DENOMINATOR);
    let terms = decode_terms(&terms_bytes);
    let open_bytes = projection_bytes(PhaseFixture::Open, 7, &[(2, 20), (3, 30), (4, 40)]);
    let open = FractionalProjectionV1::decode(&open_bytes, terms).expect("open");
    let terminalize = prepare_terminalize_v1(open, 1).expect("terminalize");
    assert_eq!(terminalize.winning_outcome, 1);
    assert_eq!(terminalize.next_revision, 8);

    let outstanding_bytes =
        projection_bytes(PhaseFixture::Terminal(1), 8, &[(2, 0), (1, 10), (4, 0)]);
    let outstanding =
        FractionalProjectionV1::decode(&outstanding_bytes, terms).expect("outstanding");
    assert!(matches!(
        prepare_retire_v1(terms, outstanding),
        Err(Error::OutstandingShardSupply)
    ));

    let ready_bytes = projection_bytes(PhaseFixture::Terminal(1), 9, &[(2, 0), (0, 0), (4, 0)]);
    let ready = FractionalProjectionV1::decode(&ready_bytes, terms).expect("ready");
    let retire = prepare_retire_v1(terms, ready).expect("retire");
    assert_eq!(retire.winning_outcome(), 1);
    assert_eq!(retire.next_revision(), 10);
    assert_eq!(retire.zero_payout_native_claims_to_burn(0), Ok(2));
    assert_eq!(retire.zero_payout_native_claims_to_burn(1), Ok(0));
    assert_eq!(retire.zero_payout_native_claims_to_burn(2), Ok(4));
    assert_eq!(retire.shard_mint(2), Ok(identity(13)));

    let retired_bytes = projection_bytes(PhaseFixture::Retired, 10, &[(0, 0), (0, 0), (0, 0)]);
    assert!(FractionalProjectionV1::decode(&retired_bytes, terms).is_ok());
    let bad_retired = projection_bytes(PhaseFixture::Retired, 10, &[(1, 0), (0, 0), (0, 0)]);
    assert_eq!(
        FractionalProjectionV1::decode(&bad_retired, terms),
        Err(Error::ReserveMismatch)
    );
}

#[test]
fn terminal_outcome_and_revision_are_total_and_checked() {
    let terms_bytes = terms_bytes(DENOMINATOR);
    let terms = decode_terms(&terms_bytes);
    let open_bytes = projection_bytes(PhaseFixture::Open, u64::MAX, &[(2, 20), (3, 30), (4, 40)]);
    let open = FractionalProjectionV1::decode(&open_bytes, terms).expect("open");
    assert_eq!(prepare_terminalize_v1(open, 3), Err(Error::InvalidOutcome));
    assert_eq!(
        prepare_terminalize_v1(open, 1),
        Err(Error::ArithmeticOverflow)
    );

    let mut noncanonical = projection_bytes(PhaseFixture::Open, 0, &[(2, 20), (3, 30), (4, 40)]);
    put(&mut noncanonical, 84, &0_u32.to_le_bytes());
    assert_eq!(
        FractionalProjectionV1::decode(&noncanonical, terms),
        Err(Error::NonCanonical)
    );
}

#[test]
fn denominator_overflow_and_shadow_balance_observations_are_refused() {
    let large_terms_bytes = terms_bytes(u64::MAX);
    let large_terms = decode_terms(&large_terms_bytes);
    let overflow_projection = projection_bytes(PhaseFixture::Open, 0, &[(2, 0), (0, 0), (0, 0)]);
    assert_eq!(
        FractionalProjectionV1::decode(&overflow_projection, large_terms),
        Err(Error::ArithmeticOverflow)
    );

    let terms_bytes = terms_bytes(DENOMINATOR);
    let terms = decode_terms(&terms_bytes);
    let projection_bytes = projection_bytes(PhaseFixture::Open, 0, &[(2, 20), (3, 30), (4, 40)]);
    let projection = FractionalProjectionV1::decode(&projection_bytes, terms).expect("projection");
    assert_eq!(
        prepare_transfer_v1(
            terms,
            projection,
            0,
            1,
            TransferObservationV1 {
                source_account: identity(30),
                destination_account: identity(31),
                source_shards: 21,
                destination_shards: 0,
            },
        ),
        Err(Error::InsufficientBalance)
    );
    assert_eq!(
        prepare_transfer_v1(
            terms,
            projection,
            0,
            1,
            TransferObservationV1 {
                source_account: identity(30),
                destination_account: identity(31),
                source_shards: 15,
                destination_shards: 6,
            },
        ),
        Err(Error::InsufficientBalance)
    );
}
