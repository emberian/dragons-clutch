//! Exact Fractional action planning across the complete lifecycle.

#![allow(clippy::panic, clippy::unwrap_used)]

use dclutch_fractional_claim_contract::{FractionalActionV1, NO_TERMINAL_OUTCOME_V1};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_TERMS_HEADER_BYTES_V1, FRACTIONAL_TERMS_MAGIC_V1, FRACTIONAL_TERMS_MINT_BYTES_V1,
    FRACTIONAL_TERMS_SCHEMA_ID_V1, FractionalPhaseV1, FractionalTermsAdmissionV1,
    FractionalTermsV1, OutcomeReserveV1, SCHEMA_VERSION_V1,
};
use dclutch_fractional_claim_operator::{
    Error, FractionalActionObservationV1, FractionalIntentV1, FractionalRequestContextV1,
    plan_fractional_action_v1,
};
use dclutch_resolution_core_v3_operator::{Finality, Observation};
use solana_program::pubkey::Pubkey;

const OUTCOMES: u32 = 3;
const DENOMINATOR: u64 = 10;
const TERMS_ID: [u8; 32] = [91; 32];

fn key(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    output
        .get_mut(offset..offset + value.len())
        .expect("fixture destination")
        .copy_from_slice(value);
}

fn terms_bytes() -> Vec<u8> {
    let mut output = vec![
        0;
        FRACTIONAL_TERMS_HEADER_BYTES_V1
            + usize::try_from(OUTCOMES).unwrap() * FRACTIONAL_TERMS_MINT_BYTES_V1
    ];
    put(&mut output, 0, &FRACTIONAL_TERMS_MAGIC_V1);
    put(&mut output, 8, &SCHEMA_VERSION_V1.to_le_bytes());
    for (offset, seed) in [(16, 1_u8), (48, 2), (80, 3), (112, 4), (144, 5)] {
        put(&mut output, offset, &[seed; 32]);
    }
    put(&mut output, 176, &OUTCOMES.to_le_bytes());
    put(&mut output, 184, &DENOMINATOR.to_le_bytes());
    for outcome in 0..OUTCOMES {
        let offset = FRACTIONAL_TERMS_HEADER_BYTES_V1 + usize::try_from(outcome).unwrap() * 32;
        put(
            &mut output,
            offset,
            &[u8::try_from(outcome + 11).unwrap(); 32],
        );
    }
    output
}

fn terms(bytes: &[u8]) -> FractionalTermsV1<'_> {
    FractionalTermsV1::decode(
        bytes,
        FractionalTermsAdmissionV1 {
            selected_schema_id: FRACTIONAL_TERMS_SCHEMA_ID_V1,
            finalized_schema_id: FRACTIONAL_TERMS_SCHEMA_ID_V1,
            selected_terms_id: TERMS_ID,
            finalized_terms_id: TERMS_ID,
            recomputed_terms_digest: TERMS_ID,
            finalized_terms_digest: TERMS_ID,
            record_authenticated: true,
        },
    )
    .unwrap()
}

fn context() -> FractionalRequestContextV1 {
    FractionalRequestContextV1 {
        release_set: [3; 32],
        market: [1; 32],
        product_record: [6; 32],
        result_domain: [2; 32],
        terms: TERMS_ID,
        token_behavior: [5; 32],
    }
}

fn observation() -> Observation {
    Observation {
        slot: 900,
        unix_timestamp: 1_800_000_000,
        finality: Finality::Finalized,
    }
}

fn open_rows() -> [OutcomeReserveV1; 3] {
    [
        OutcomeReserveV1 {
            locked_native_claims: 2,
            shard_supply: 20,
        },
        OutcomeReserveV1 {
            locked_native_claims: 3,
            shard_supply: 30,
        },
        OutcomeReserveV1 {
            locked_native_claims: 4,
            shard_supply: 40,
        },
    ]
}

fn terminal_rows() -> [OutcomeReserveV1; 3] {
    [
        OutcomeReserveV1 {
            locked_native_claims: 2,
            shard_supply: 13,
        },
        OutcomeReserveV1 {
            locked_native_claims: 3,
            shard_supply: 30,
        },
        OutcomeReserveV1 {
            locked_native_claims: 4,
            shard_supply: 0,
        },
    ]
}

fn observed<'a>(
    phase: FractionalPhaseV1,
    rows: &'a [OutcomeReserveV1],
) -> FractionalActionObservationV1<'a> {
    let terminal_outcome = match phase {
        FractionalPhaseV1::Terminal { winning_outcome } => winning_outcome,
        _ => NO_TERMINAL_OUTCOME_V1,
    };
    FractionalActionObservationV1 {
        observation: observation(),
        revision: 7,
        phase,
        terminal_digest: if terminal_outcome == NO_TERMINAL_OUTCOME_V1 {
            [0; 32]
        } else {
            [8; 32]
        },
        terminal_outcome,
        reserves: rows,
        owner: key(20),
        source_token_account: key(21),
        destination_token_account: Pubkey::default(),
        actor_native_claims: 9,
        source_shards: 23,
        destination_shards: 4,
    }
}

fn intent(action: FractionalActionV1, outcome: u32, quantity: u64) -> FractionalIntentV1 {
    FractionalIntentV1 {
        action,
        outcome,
        quantity,
    }
}

#[test]
fn wrap_transfer_and_whole_unwrap_are_exact_and_mint_is_terms_owned() {
    let bytes = terms_bytes();
    let terms = terms(&bytes);
    let rows = open_rows();
    let mut wrap_observation = observed(FractionalPhaseV1::Open, &rows);
    wrap_observation.source_token_account = Pubkey::default();
    wrap_observation.destination_token_account = key(22);
    wrap_observation.destination_shards = 3;
    let wrap = plan_fractional_action_v1(
        terms,
        context(),
        intent(FractionalActionV1::Wrap, 0, 2),
        wrap_observation,
    )
    .unwrap();
    assert_eq!(wrap.shard_mint, [11; 32]);
    assert_eq!(wrap.native_claims, 2);
    assert_eq!(wrap.consumed_shards, 20);
    assert_eq!(wrap.post_destination_shards, 23);

    let mut transfer_observation = observed(FractionalPhaseV1::Open, &rows);
    transfer_observation.destination_token_account = key(22);
    transfer_observation.source_shards = 13;
    let transfer = plan_fractional_action_v1(
        terms,
        context(),
        intent(FractionalActionV1::Transfer, 0, 7),
        transfer_observation,
    )
    .unwrap();
    assert_eq!(transfer.post_source_shards, 6);
    assert_eq!(transfer.post_destination_shards, 11);
    assert_eq!(transfer.post_revision, 7);

    let unwrap = plan_fractional_action_v1(
        terms,
        context(),
        intent(FractionalActionV1::WholeUnwrap, 1, 23),
        observed(FractionalPhaseV1::Open, &rows),
    )
    .unwrap();
    assert_eq!(unwrap.consumed_shards, 20);
    assert_eq!(unwrap.change_shards, 3);
    assert_eq!(unwrap.post_source_shards, 3);
    assert_eq!(unwrap.native_claims, 2);
    assert_eq!(unwrap.collateral_atoms, 0);
    assert_eq!(unwrap.request.input().destination_token_account, [0; 32]);
}

#[test]
fn terminalize_then_winning_and_losing_paths_bind_exact_terminal_evidence() {
    let bytes = terms_bytes();
    let terms = terms(&bytes);
    let open = open_rows();
    let mut terminalize_observation = observed(FractionalPhaseV1::Open, &open);
    terminalize_observation.owner = Pubkey::default();
    terminalize_observation.source_token_account = Pubkey::default();
    terminalize_observation.terminal_digest = [8; 32];
    terminalize_observation.terminal_outcome = 1;
    let terminalize = plan_fractional_action_v1(
        terms,
        context(),
        intent(FractionalActionV1::Terminalize, 1, 0),
        terminalize_observation,
    )
    .unwrap();
    assert_eq!(terminalize.post_revision, 8);
    assert_eq!(terminalize.request.input().terminal_outcome, 1);

    let terminal = terminal_rows();
    let winning = plan_fractional_action_v1(
        terms,
        context(),
        intent(FractionalActionV1::WinningRedeem, 1, 23),
        observed(
            FractionalPhaseV1::Terminal { winning_outcome: 1 },
            &terminal,
        ),
    )
    .unwrap();
    assert_eq!(winning.consumed_shards, 20);
    assert_eq!(winning.change_shards, 3);
    assert_eq!(winning.collateral_atoms, 2);
    assert_eq!(winning.native_claims, 2);

    let mut losing_observation = observed(
        FractionalPhaseV1::Terminal { winning_outcome: 1 },
        &terminal,
    );
    losing_observation.source_shards = 13;
    let losing = plan_fractional_action_v1(
        terms,
        context(),
        intent(FractionalActionV1::LosingZeroBurn, 0, 7),
        losing_observation,
    )
    .unwrap();
    assert_eq!(losing.consumed_shards, 7);
    assert_eq!(losing.native_claims, 0);
    assert_eq!(losing.collateral_atoms, 0);
}

#[test]
fn zero_supply_retirement_derives_loser_native_burns_without_shadow_supply() {
    let bytes = terms_bytes();
    let terms = terms(&bytes);
    let rows = [
        OutcomeReserveV1 {
            locked_native_claims: 2,
            shard_supply: 0,
        },
        OutcomeReserveV1 {
            locked_native_claims: 0,
            shard_supply: 0,
        },
        OutcomeReserveV1 {
            locked_native_claims: 4,
            shard_supply: 0,
        },
    ];
    let mut retirement = observed(FractionalPhaseV1::Terminal { winning_outcome: 1 }, &rows);
    retirement.owner = Pubkey::default();
    retirement.source_token_account = Pubkey::default();
    retirement.source_shards = 0;
    let plan = plan_fractional_action_v1(
        terms,
        context(),
        intent(
            FractionalActionV1::ZeroSupplyRetire,
            NO_TERMINAL_OUTCOME_V1,
            0,
        ),
        retirement,
    )
    .unwrap();
    assert_eq!(plan.retirement_native_burns, vec![2, 0, 4]);
    assert_eq!(plan.post_revision, 8);
}

#[test]
fn terminal_substitution_and_nonwhole_redeem_refuse() {
    let bytes = terms_bytes();
    let terms = terms(&bytes);
    let rows = terminal_rows();
    let mut substituted = observed(FractionalPhaseV1::Terminal { winning_outcome: 1 }, &rows);
    substituted.terminal_outcome = 2;
    assert_eq!(
        plan_fractional_action_v1(
            terms,
            context(),
            intent(FractionalActionV1::WinningRedeem, 1, 23),
            substituted,
        ),
        Err(Error::Action)
    );
    let mut nonwhole = observed(FractionalPhaseV1::Terminal { winning_outcome: 1 }, &rows);
    nonwhole.source_shards = 3;
    assert_eq!(
        plan_fractional_action_v1(
            terms,
            context(),
            intent(FractionalActionV1::WinningRedeem, 1, 3),
            nonwhole,
        ),
        Err(Error::Kernel)
    );
}
