//! Token-owned physical effects, exact denominator path, and RentV2 closure.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

mod support;

use dclutch_fractional_claim_contract::{FractionalActionV1, NO_TERMINAL_OUTCOME_V1};
use dclutch_fractional_claim_kernel::{FractionalPhaseV1, OutcomeReserveV1};
use dclutch_fractional_claim_operator::{
    Error, FractionalActionObservationV1, FractionalClaimsAccountRuleV1,
    FractionalDenominatorExecutionV1, FractionalIntentV1, FractionalMintSnapshotV1,
    FractionalPhysicalTokenEffectsV1, FractionalPhysicalTokenObservationV1,
    FractionalTokenAccountSnapshotV1, FractionalTokenActionSnapshotV1, FractionalTokenEffectV1,
    build_fractional_physical_unsigned_v0_from_chain_v1, plan_fractional_action_v1,
    plan_fractional_lifecycle_rent_close_v2, plan_fractional_retirement_token_effects_v1,
    plan_fractional_token_effect_v1,
};
use dclutch_market_core_codec::{RetirementReceiptInputV1, RetirementReceiptV1};
use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{LifecycleAccountIdV2, LifecycleRentCreditV2},
};
use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;
use solana_hash::Hash;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use support::FractionalChainFixtureV1;

fn key(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn claims_frame() -> [FractionalClaimsAccountRuleV1; 1] {
    [FractionalClaimsAccountRuleV1 {
        signer: false,
        writable: false,
        executable: true,
        data_length: 0,
    }]
}

fn observed<'a>(
    fixture: &'a FractionalChainFixtureV1,
    phase: FractionalPhaseV1,
    rows: &'a [OutcomeReserveV1],
    outcome: u32,
) -> FractionalActionObservationV1<'a> {
    let terminal_outcome = match phase {
        FractionalPhaseV1::Terminal { winning_outcome } => winning_outcome,
        FractionalPhaseV1::Open | FractionalPhaseV1::Retired => NO_TERMINAL_OUTCOME_V1,
    };
    FractionalActionObservationV1 {
        observation: fixture.observation,
        revision: 7,
        phase,
        terminal_digest: if terminal_outcome == NO_TERMINAL_OUTCOME_V1 {
            [0; 32]
        } else {
            [88; 32]
        },
        terminal_outcome,
        reserves: rows,
        owner: fixture.owner,
        source_token_account: key(81),
        destination_token_account: key(82),
        actor_native_claims: 9,
        source_shards: if outcome == 0 { 13 } else { 23 },
        destination_shards: 3,
    }
}

fn intent(action: FractionalActionV1, outcome: u32, quantity: u64) -> FractionalIntentV1 {
    FractionalIntentV1 {
        action,
        outcome,
        quantity,
    }
}

fn token_account(mint: Pubkey, owner: Pubkey, amount: u64) -> [u8; 165] {
    let mut data = [0; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    data
}

fn behavior_mint(controller: Pubkey, supply: u64, decimals: u8) -> Vec<u8> {
    let mut data = vec![0; 238];
    data[0..4].copy_from_slice(&1_u32.to_le_bytes());
    data[4..36].copy_from_slice(controller.as_ref());
    data[36..44].copy_from_slice(&supply.to_le_bytes());
    data[44] = decimals;
    data[45] = 1;
    data[165] = 1;
    data[166..168].copy_from_slice(&3_u16.to_le_bytes());
    data[168..170].copy_from_slice(&32_u16.to_le_bytes());
    data[170..202].copy_from_slice(controller.as_ref());
    data[202..204].copy_from_slice(&28_u16.to_le_bytes());
    data[204..206].copy_from_slice(&32_u16.to_le_bytes());
    data[206..238].copy_from_slice(controller.as_ref());
    data
}

fn account_snapshot<'a>(key: Pubkey, data: &'a [u8]) -> FractionalTokenAccountSnapshotV1<'a> {
    FractionalTokenAccountSnapshotV1 {
        key,
        program_owner: Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
        data,
    }
}

#[test]
fn wrap_transfer_and_burn_emit_exact_token_2022_effects() {
    let wrap_fixture =
        FractionalChainFixtureV1::new(FractionalActionV1::Wrap, [62; 32], &claims_frame());
    let wrap_prepared = wrap_fixture.prepare();
    let rows = wrap_fixture.reserves.clone();
    let mut wrap_observed = observed(&wrap_fixture, FractionalPhaseV1::Open, &rows, 0);
    wrap_observed.source_token_account = Pubkey::default();
    let wrap_action = plan_fractional_action_v1(
        wrap_prepared.terms(),
        wrap_prepared.request_context(),
        intent(FractionalActionV1::Wrap, 0, 2),
        wrap_observed,
    )
    .expect("wrap action");
    let wrap_mint = behavior_mint(wrap_prepared.root_key(), 20, u8::MAX);
    let wrap_destination = token_account(
        Pubkey::new_from_array(wrap_action.shard_mint),
        wrap_fixture.owner,
        3,
    );
    let wrap = plan_fractional_token_effect_v1(
        wrap_prepared,
        &wrap_action,
        wrap_observed,
        FractionalTokenActionSnapshotV1 {
            mint: Some(account_snapshot(
                Pubkey::new_from_array(wrap_action.shard_mint),
                &wrap_mint,
            )),
            source: None,
            destination: Some(account_snapshot(key(82), &wrap_destination)),
        },
    )
    .expect("Token-2022 wrap effect");
    assert!(matches!(wrap.effect(), FractionalTokenEffectV1::Mint(_)));
    assert_eq!((wrap.pre_supply(), wrap.post_supply()), (20, 40));
    assert_eq!((wrap.pre_destination(), wrap.post_destination()), (3, 23));
    assert_eq!(wrap.display_decimals(), u8::MAX);

    let accounts = [
        AccountMeta::new_readonly(wrap_fixture.owner, true),
        AccountMeta::new_readonly(wrap_fixture.claims_program.key, false),
        AccountMeta::new_readonly(wrap_fixture.custody_program.key, false),
        AccountMeta::new_readonly(wrap_fixture.token_program.key, false),
    ];
    let packet_token = FractionalTokenActionSnapshotV1 {
        mint: Some(account_snapshot(
            Pubkey::new_from_array(wrap_action.shard_mint),
            &wrap_mint,
        )),
        source: None,
        destination: Some(account_snapshot(key(82), &wrap_destination)),
    };
    let packet = build_fractional_physical_unsigned_v0_from_chain_v1(
        wrap_prepared,
        intent(FractionalActionV1::Wrap, 0, 2),
        wrap_observed,
        FractionalPhysicalTokenObservationV1::Action(&packet_token),
        wrap_fixture.payer,
        Hash::new_from_array([93; 32]),
        &accounts,
        &[],
    )
    .expect("chain-derived packet plus Token effects");
    assert_eq!(packet.unsigned.action, wrap_action);
    assert!(matches!(
        packet.token,
        FractionalPhysicalTokenEffectsV1::Action(_)
    ));

    let transfer_fixture =
        FractionalChainFixtureV1::new(FractionalActionV1::Transfer, [62; 32], &claims_frame());
    let transfer_prepared = transfer_fixture.prepare();
    let rows = transfer_fixture.reserves.clone();
    let transfer_observed = observed(&transfer_fixture, FractionalPhaseV1::Open, &rows, 0);
    let transfer_action = plan_fractional_action_v1(
        transfer_prepared.terms(),
        transfer_prepared.request_context(),
        intent(FractionalActionV1::Transfer, 0, 7),
        transfer_observed,
    )
    .expect("transfer action");
    let transfer_mint = behavior_mint(transfer_prepared.root_key(), 20, 6);
    let source = token_account(
        Pubkey::new_from_array(transfer_action.shard_mint),
        transfer_fixture.owner,
        13,
    );
    let destination = token_account(
        Pubkey::new_from_array(transfer_action.shard_mint),
        key(90),
        3,
    );
    let transfer = plan_fractional_token_effect_v1(
        transfer_prepared,
        &transfer_action,
        transfer_observed,
        FractionalTokenActionSnapshotV1 {
            mint: Some(account_snapshot(
                Pubkey::new_from_array(transfer_action.shard_mint),
                &transfer_mint,
            )),
            source: Some(account_snapshot(key(81), &source)),
            destination: Some(account_snapshot(key(82), &destination)),
        },
    )
    .expect("Token-2022 transfer effect");
    assert!(matches!(
        transfer.effect(),
        FractionalTokenEffectV1::Transfer(_)
    ));
    assert_eq!((transfer.pre_supply(), transfer.post_supply()), (20, 20));
    assert_eq!((transfer.pre_source(), transfer.post_source()), (13, 6));
    assert_eq!(
        (transfer.pre_destination(), transfer.post_destination()),
        (3, 10)
    );

    let losing_fixture = FractionalChainFixtureV1::new(
        FractionalActionV1::LosingZeroBurn,
        [62; 32],
        &claims_frame(),
    );
    let losing_prepared = losing_fixture.prepare();
    let rows = losing_fixture.reserves.clone();
    let mut losing_observed = observed(
        &losing_fixture,
        FractionalPhaseV1::Terminal { winning_outcome: 1 },
        &rows,
        0,
    );
    losing_observed.destination_token_account = Pubkey::default();
    let losing_action = plan_fractional_action_v1(
        losing_prepared.terms(),
        losing_prepared.request_context(),
        intent(FractionalActionV1::LosingZeroBurn, 0, 7),
        losing_observed,
    )
    .expect("losing action");
    let losing_mint = behavior_mint(losing_prepared.root_key(), 20, 0);
    let losing_source = token_account(
        Pubkey::new_from_array(losing_action.shard_mint),
        losing_fixture.owner,
        13,
    );
    let losing = plan_fractional_token_effect_v1(
        losing_prepared,
        &losing_action,
        losing_observed,
        FractionalTokenActionSnapshotV1 {
            mint: Some(account_snapshot(
                Pubkey::new_from_array(losing_action.shard_mint),
                &losing_mint,
            )),
            source: Some(account_snapshot(key(81), &losing_source)),
            destination: None,
        },
    )
    .expect("Token-2022 losing burn");
    assert!(matches!(losing.effect(), FractionalTokenEffectV1::Burn(_)));
    assert_eq!((losing.pre_supply(), losing.post_supply()), (20, 13));
}

#[test]
fn exact_denominator_fast_path_and_same_mint_change_are_distinct() {
    let fixture =
        FractionalChainFixtureV1::new(FractionalActionV1::WholeUnwrap, [62; 32], &claims_frame());
    let prepared = fixture.prepare();
    let rows = fixture.reserves.clone();
    let mut action_observed = observed(&fixture, FractionalPhaseV1::Open, &rows, 1);
    action_observed.destination_token_account = Pubkey::default();
    let mint_key = Pubkey::new_from_array(prepared.terms().shard_mint(1).expect("Mint"));
    let mint = behavior_mint(prepared.root_key(), 30, 9);
    let source = token_account(mint_key, fixture.owner, 23);

    for (quantity, expected) in [
        (
            20,
            FractionalDenominatorExecutionV1::ExactWhole { denominator: 10 },
        ),
        (
            23,
            FractionalDenominatorExecutionV1::WholeWithSameMintChange {
                denominator: 10,
                change_shards: 3,
            },
        ),
    ] {
        let action = plan_fractional_action_v1(
            prepared.terms(),
            prepared.request_context(),
            intent(FractionalActionV1::WholeUnwrap, 1, quantity),
            action_observed,
        )
        .expect("whole unwrap");
        let plan = plan_fractional_token_effect_v1(
            prepared,
            &action,
            action_observed,
            FractionalTokenActionSnapshotV1 {
                mint: Some(account_snapshot(mint_key, &mint)),
                source: Some(account_snapshot(key(81), &source)),
                destination: None,
            },
        )
        .expect("exact burn plan");
        assert_eq!(plan.denominator_execution(), expected);
        assert_eq!(plan.post_source(), 3);
        assert_eq!(plan.post_supply(), 10);
    }
}

#[test]
fn substituted_token_state_and_aliases_refuse() {
    let fixture =
        FractionalChainFixtureV1::new(FractionalActionV1::Transfer, [62; 32], &claims_frame());
    let prepared = fixture.prepare();
    let rows = fixture.reserves.clone();
    let action_observed = observed(&fixture, FractionalPhaseV1::Open, &rows, 0);
    let action = plan_fractional_action_v1(
        prepared.terms(),
        prepared.request_context(),
        intent(FractionalActionV1::Transfer, 0, 7),
        action_observed,
    )
    .expect("transfer action");
    let mint = behavior_mint(prepared.root_key(), 20, 0);
    let source = token_account(Pubkey::new_from_array(action.shard_mint), fixture.owner, 13);
    let destination = token_account(Pubkey::new_from_array(action.shard_mint), key(90), 3);
    let canonical = FractionalTokenActionSnapshotV1 {
        mint: Some(account_snapshot(
            Pubkey::new_from_array(action.shard_mint),
            &mint,
        )),
        source: Some(account_snapshot(key(81), &source)),
        destination: Some(account_snapshot(key(82), &destination)),
    };

    let mut wrong_owner = canonical;
    wrong_owner.mint.as_mut().expect("Mint").program_owner = key(99);
    assert_eq!(
        plan_fractional_token_effect_v1(prepared, &action, action_observed, wrong_owner),
        Err(Error::Token)
    );
    let mut wrong_mint = canonical;
    wrong_mint.mint.as_mut().expect("Mint").key = key(99);
    assert_eq!(
        plan_fractional_token_effect_v1(prepared, &action, action_observed, wrong_mint),
        Err(Error::Token)
    );
    let mut alias = canonical;
    alias.destination.as_mut().expect("destination").key = key(81);
    assert_eq!(
        plan_fractional_token_effect_v1(prepared, &action, action_observed, alias),
        Err(Error::Token)
    );
}

#[test]
fn zero_supply_mints_close_in_terms_order_then_canonical_rent_v2_closes() {
    let fixture = FractionalChainFixtureV1::new(
        FractionalActionV1::ZeroSupplyRetire,
        [62; 32],
        &claims_frame(),
    );
    let prepared = fixture.prepare();
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
    let mut action_observed = observed(
        &fixture,
        FractionalPhaseV1::Terminal { winning_outcome: 1 },
        &rows,
        NO_TERMINAL_OUTCOME_V1,
    );
    action_observed.owner = Pubkey::default();
    action_observed.source_token_account = Pubkey::default();
    action_observed.destination_token_account = Pubkey::default();
    action_observed.source_shards = 0;
    let action = plan_fractional_action_v1(
        prepared.terms(),
        prepared.request_context(),
        intent(
            FractionalActionV1::ZeroSupplyRetire,
            NO_TERMINAL_OUTCOME_V1,
            0,
        ),
        action_observed,
    )
    .expect("retirement action");

    let mint_data: Vec<Vec<u8>> = (0..3)
        .map(|_| behavior_mint(prepared.root_key(), 0, 0))
        .collect();
    let mints: Vec<FractionalMintSnapshotV1<'_>> = mint_data
        .iter()
        .enumerate()
        .map(|(index, data)| {
            let outcome = u32::try_from(index).expect("outcome");
            FractionalMintSnapshotV1 {
                outcome,
                mint: account_snapshot(
                    Pubkey::new_from_array(prepared.terms().shard_mint(outcome).expect("Mint")),
                    data,
                ),
            }
        })
        .collect();
    let retirement =
        plan_fractional_retirement_token_effects_v1(prepared, &action, action_observed, &mints)
            .expect("ordered zero-supply Mint closures");
    assert_eq!(retirement.instructions().len(), 3);
    assert_eq!(retirement.post_revision(), 8);
    for (instruction, mint) in retirement.instructions().iter().zip(mints.iter()) {
        assert_eq!(instruction.program_id.to_bytes(), TOKEN_2022_PROGRAM_ID);
        assert_eq!(instruction.accounts[0].pubkey, mint.mint.key);
        assert_eq!(instruction.accounts[1].pubkey, retirement.rent_credit());
    }

    let credit_id = LifecycleAccountIdV2::new(retirement.rent_credit().to_bytes())
        .expect("RentCredit identity");
    let credit = LifecycleRentCreditV2::new(
        RefundAuthority::new([91; 32]).expect("refund wallet"),
        LifecycleAccountIdV2::new(retirement.market()).expect("Market"),
        LifecycleAccountIdV2::new(retirement.release_set()).expect("release"),
        7,
        9,
    )
    .expect("lifecycle credit");
    let core_program = fixture.checked.core_program().to_bytes();
    let core_receipt = RetirementReceiptV1::new(RetirementReceiptInputV1 {
        core_program,
        market: retirement.market(),
        release_set: retirement.release_set(),
        rent_credit: retirement.rent_credit().to_bytes(),
        bundle_digest: [1; 32],
        source_receipt_digest: [2; 32],
        claims_receipt_digest: [3; 32],
        custody_close_vault_receipt_digest: [4; 32],
        custody_close_replay_receipt_digest: [5; 32],
        pre_state_digest: [6; 32],
        retired_candidate_digest: [7; 32],
        post_resource_digest: [8; 32],
        generation: 7,
        source_closure_revision: 1,
        claims_post_revision: 2,
        custody_post_revision: 3,
        core_refund_lamports: 4,
        claims_refund_lamports: 5,
        custody_refund_lamports: 6,
    })
    .expect("Core retirement receipt");
    let rent = plan_fractional_lifecycle_rent_close_v2(
        &retirement,
        Pubkey::new_from_array(credit_id.to_bytes()),
        &credit.to_bytes(),
        1_000,
        2_000,
        &core_receipt.to_bytes(),
        true,
    )
    .expect("canonical RentV2 closure");
    assert_eq!(rent.plan.closed_lamports(), 1_000);
    assert_eq!(rent.plan.wallet_after(), 3_000);
    assert_eq!(rent.plan.post_resource_digest(), [8; 32]);
    assert_eq!(rent.receipt.input().post_resource_digest, [8; 32]);

    let mut substituted = core_receipt.to_bytes();
    substituted[80] ^= 1;
    assert_eq!(
        plan_fractional_lifecycle_rent_close_v2(
            &retirement,
            Pubkey::new_from_array(credit_id.to_bytes()),
            &credit.to_bytes(),
            1_000,
            2_000,
            &substituted,
            true,
        ),
        Err(Error::Rent)
    );
}
