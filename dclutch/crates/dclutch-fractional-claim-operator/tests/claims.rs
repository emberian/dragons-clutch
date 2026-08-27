//! Chain-derived Fractional lowering through the canonical Claims waist.

#![allow(clippy::panic, clippy::unwrap_used)]

mod support;

use dclutch_claims_svm::{
    frame_spec_v1::SignedDeltaFrameSpecV3,
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LiabilityBasisMarketInputV2,
        LiabilityBasisPositionInputV2, encode_liability_basis_market_into_v2,
        encode_liability_basis_position_into_v2,
    },
    protocol_position_v2::ProtocolPositionSeedsV2,
    signed_delta_v3::{SignedDeltaPlanV3, SignedDeltaReceiptV3},
};
use dclutch_fractional_claim_contract::{FractionalActionV1, NO_TERMINAL_OUTCOME_V1};
use dclutch_fractional_claim_kernel::FractionalPhaseV1;
use dclutch_fractional_claim_operator::{
    Error, FractionalActionObservationV1, FractionalClaimsAccountRuleV1,
    FractionalClaimsPositionSnapshotV1, FractionalIntentV1,
    FractionalSignedDeltaChainObservationV1, build_fractional_signed_delta_instruction_v1,
    lower_fractional_action_to_signed_delta_v1, plan_fractional_action_v1,
    validate_fractional_signed_delta_chain_result_v1,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use support::FractionalChainFixtureV1;

fn compiler_frame() -> [FractionalClaimsAccountRuleV1; 1] {
    [FractionalClaimsAccountRuleV1 {
        signer: false,
        writable: false,
        executable: true,
        data_length: 0,
    }]
}

struct ClaimsState {
    market_account: Pubkey,
    market: Vec<u8>,
    reserve_account: Pubkey,
    reserve: Vec<u8>,
    actor_account: Pubkey,
    actor: Vec<u8>,
    linked_basis_record_digest: [u8; 32],
}

fn claims_state(fixture: &FractionalChainFixtureV1, claim_count: usize) -> ClaimsState {
    let prepared = fixture.prepare();
    let claims_program = prepared.checked_release().claims_program();
    let market_account = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, fixture.market.key.as_ref()],
        &claims_program,
    )
    .0;
    let basis = prepared.product_join().liability_basis_id.to_bytes();
    let supplies = vec![1_000_u64; claim_count];
    let mut market = vec![0; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + claim_count * 8];
    encode_liability_basis_market_into_v2(
        LiabilityBasisMarketInputV2 {
            revision: 19,
            logical_market: fixture.market.key.to_bytes(),
            release_set: prepared.request_context().release_set,
            registry_program: fixture.registry_program.key.to_bytes(),
            product_instance_id: prepared.product_join().product_id.to_bytes(),
            basis_id: basis,
            realm_id: [32; 32],
            custody_context: [91; 32],
            generation: 7,
        },
        &supplies,
        &mut market,
    )
    .unwrap();
    let reserve_balances: Vec<u64> = fixture
        .reserves
        .iter()
        .map(|row| row.locked_native_claims)
        .collect();
    let actor_balances = vec![9_u64; claim_count];
    let position = |owner: Pubkey, revision: u64, balances: &[u64]| {
        let seeds =
            ProtocolPositionSeedsV2::new(market_account.to_bytes(), owner.to_bytes()).unwrap();
        let account = Pubkey::find_program_address(&seeds.as_slices(), &claims_program).0;
        let mut bytes = vec![0; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + claim_count * 8];
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision,
                market_account: market_account.to_bytes(),
                owner: owner.to_bytes(),
                basis_id: basis,
            },
            balances,
            &mut bytes,
        )
        .unwrap();
        (account, bytes)
    };
    let (reserve_account, reserve) = position(prepared.root_key(), 23, &reserve_balances);
    let (actor_account, actor) = position(fixture.owner, 29, &actor_balances);
    ClaimsState {
        market_account,
        market,
        reserve_account,
        reserve,
        actor_account,
        actor,
        linked_basis_record_digest: [92; 32],
    }
}

fn wrap_action(
    fixture: &FractionalChainFixtureV1,
    outcome: u32,
) -> dclutch_fractional_claim_operator::FractionalActionPlanV1 {
    let prepared = fixture.prepare();
    plan_fractional_action_v1(
        prepared.terms(),
        prepared.request_context(),
        FractionalIntentV1 {
            action: FractionalActionV1::Wrap,
            outcome,
            quantity: 2,
        },
        FractionalActionObservationV1 {
            observation: fixture.observation,
            revision: prepared.root().input().revision,
            phase: FractionalPhaseV1::Open,
            terminal_digest: [0; 32],
            terminal_outcome: NO_TERMINAL_OUTCOME_V1,
            reserves: &fixture.reserves,
            owner: fixture.owner,
            source_token_account: Pubkey::default(),
            destination_token_account: Pubkey::new_from_array([71; 32]),
            actor_native_claims: 9,
            source_shards: 0,
            destination_shards: 3,
        },
    )
    .unwrap()
}

fn observation<'a>(state: &'a ClaimsState) -> FractionalSignedDeltaChainObservationV1<'a> {
    FractionalSignedDeltaChainObservationV1 {
        market_account: state.market_account,
        market_bytes: &state.market,
        linked_basis_record_digest: state.linked_basis_record_digest,
        reserve: FractionalClaimsPositionSnapshotV1 {
            account: state.reserve_account,
            bytes: &state.reserve,
        },
        actor: Some(FractionalClaimsPositionSnapshotV1 {
            account: state.actor_account,
            bytes: &state.actor,
        }),
    }
}

#[test]
fn chain_derived_wrap_builds_exact_frame_and_validates_the_sole_claims_receipt() {
    let fixture =
        FractionalChainFixtureV1::new(FractionalActionV1::Wrap, [62; 32], &compiler_frame());
    let state = claims_state(&fixture, 3);
    let prepared = fixture.prepare();
    let action = wrap_action(&fixture, 0);
    let plan =
        lower_fractional_action_to_signed_delta_v1(prepared, &action, observation(&state)).unwrap();
    let decoded = SignedDeltaPlanV3::decode(plan.packet()).unwrap();
    assert_eq!(decoded.claim_count(), 3);
    assert_eq!(decoded.position_count(), 2);
    assert_eq!(decoded.request_id(), plan.lowering().request_digest());

    let spec = SignedDeltaFrameSpecV3::new(decoded.position_count()).unwrap();
    let mut metas = Vec::with_capacity(usize::from(spec.account_count().unwrap()));
    for index in 0..spec.account_count().unwrap() {
        let privileges = spec.account(index).unwrap().privileges();
        let key = match index {
            1 => state.market_account,
            14 => fixture.trading_program.key,
            16 => fixture.claims_program.key,
            position if position >= 20 => *plan
                .ordered_position_accounts()
                .get(usize::from(position - 20))
                .unwrap(),
            _ => Pubkey::new_from_array([u8::try_from(index).unwrap().saturating_add(100); 32]),
        };
        metas.push(if privileges.writable() {
            AccountMeta::new(key, privileges.signer())
        } else {
            AccountMeta::new_readonly(key, privileges.signer())
        });
    }
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        decoded.release_set(),
        decoded.market(),
        ExecutionRoleV1::Trading,
        decoded.request_id(),
        plan.lowering().packet_digest(),
    )
    .unwrap();
    metas.first_mut().unwrap().pubkey =
        Pubkey::find_program_address(&authority_seeds.as_slices(), &fixture.trading_program.key).0;
    let instruction = build_fractional_signed_delta_instruction_v1(&plan, &metas).unwrap();
    assert_eq!(instruction.program_id, fixture.claims_program.key);
    assert_eq!(instruction.data, plan.packet());

    let receipt = SignedDeltaReceiptV3::new(
        decoded,
        plan.lowering().packet_digest(),
        plan.lowering().table_digest(),
        fixture.claims_program.key.to_bytes(),
        plan.lowering().post_resource_digest(),
        plan.lowering().post_market_revision(),
    )
    .unwrap()
    .to_bytes();
    let post_positions: Vec<&[u8]> = plan
        .expected_post_positions()
        .iter()
        .map(Vec::as_slice)
        .collect();
    assert_eq!(
        validate_fractional_signed_delta_chain_result_v1(
            &plan,
            &receipt,
            plan.expected_post_market(),
            &post_positions,
        ),
        Ok(())
    );
}

#[test]
fn runtime_width_258_and_hostile_market_position_or_poststate_substitution_refuse() {
    let fixture = FractionalChainFixtureV1::new_with_outcomes(
        FractionalActionV1::Wrap,
        [62; 32],
        &compiler_frame(),
        258,
    );
    let state = claims_state(&fixture, 258);
    let action = wrap_action(&fixture, 257);
    let plan =
        lower_fractional_action_to_signed_delta_v1(fixture.prepare(), &action, observation(&state))
            .unwrap();
    assert_eq!(
        SignedDeltaPlanV3::decode(plan.packet())
            .unwrap()
            .claim_count(),
        258
    );

    let mut wrong_market = observation(&state);
    wrong_market.market_account = Pubkey::new_unique();
    assert!(matches!(
        lower_fractional_action_to_signed_delta_v1(fixture.prepare(), &action, wrong_market),
        Err(Error::Claims)
    ));
    let mut wrong_position = observation(&state);
    wrong_position.reserve.account = Pubkey::new_unique();
    assert!(matches!(
        lower_fractional_action_to_signed_delta_v1(fixture.prepare(), &action, wrong_position),
        Err(Error::Claims)
    ));

    let decoded = SignedDeltaPlanV3::decode(plan.packet()).unwrap();
    let receipt = SignedDeltaReceiptV3::new(
        decoded,
        plan.lowering().packet_digest(),
        plan.lowering().table_digest(),
        fixture.claims_program.key.to_bytes(),
        plan.lowering().post_resource_digest(),
        plan.lowering().post_market_revision(),
    )
    .unwrap()
    .to_bytes();
    let mut substituted = plan.expected_post_positions().to_vec();
    *substituted.first_mut().unwrap().last_mut().unwrap() ^= 1;
    let substituted_refs: Vec<&[u8]> = substituted.iter().map(Vec::as_slice).collect();
    assert_eq!(
        validate_fractional_signed_delta_chain_result_v1(
            &plan,
            &receipt,
            plan.expected_post_market(),
            &substituted_refs,
        ),
        Err(Error::Claims)
    );
}
