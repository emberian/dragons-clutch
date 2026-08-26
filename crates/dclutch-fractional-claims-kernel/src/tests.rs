#![allow(clippy::panic, clippy::unwrap_used)]

extern crate std;

use std::{vec, vec::Vec};

use dclutch_claims_svm::{
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        LiabilityBasisMarketInputV2, LiabilityBasisMarketViewV2, LiabilityBasisPositionInputV2,
        LiabilityBasisPositionViewV2, encode_liability_basis_market_into_v2,
        encode_liability_basis_position_into_v2,
    },
    signed_delta_v3::{
        DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SignedDeltaPlanV3,
        SignedDeltaReceiptV3, SignedDeltaV3,
    },
};
use dclutch_fractional_claim_contract::{
    FractionalActionV1, FractionalFamilyRequestInputV1, FractionalFamilyRequestV1,
    NO_TERMINAL_OUTCOME_V1,
};

use super::*;

const CLAIMS: [u8; 32] = [1; 32];
const MARKET_ACCOUNT: [u8; 32] = [2; 32];
const LOGICAL_MARKET: [u8; 32] = [3; 32];
const RELEASE: [u8; 32] = [4; 32];
const PRODUCT: [u8; 32] = [5; 32];
const PRODUCT_RECORD: [u8; 32] = [6; 32];
const BASIS: [u8; 32] = [7; 32];
const LINKED_BASIS: [u8; 32] = [8; 32];
const RESERVE: [u8; 32] = [9; 32];
const ACTOR: [u8; 32] = [10; 32];

struct State {
    market: Vec<u8>,
    reserve: Vec<u8>,
    actor: Vec<u8>,
}

fn state(supplies: &[u64], reserve: &[u64], actor: &[u64]) -> State {
    let mut market = vec![0; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + supplies.len() * 8];
    encode_liability_basis_market_into_v2(
        LiabilityBasisMarketInputV2 {
            revision: 11,
            logical_market: LOGICAL_MARKET,
            release_set: RELEASE,
            registry_program: [11; 32],
            product_instance_id: PRODUCT,
            basis_id: BASIS,
            realm_id: [12; 32],
            custody_context: [13; 32],
            generation: 7,
        },
        supplies,
        &mut market,
    )
    .unwrap();
    let position = |owner, balances: &[u64]| {
        let mut bytes = vec![0; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + balances.len() * 8];
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision: if owner == RESERVE { 20 } else { 30 },
                market_account: MARKET_ACCOUNT,
                owner,
                basis_id: BASIS,
            },
            balances,
            &mut bytes,
        )
        .unwrap();
        bytes
    };
    State {
        market,
        reserve: position(RESERVE, reserve),
        actor: position(ACTOR, actor),
    }
}

fn request(action: FractionalActionV1, outcome: u32, quantity: u64) -> FractionalFamilyRequestV1 {
    let (owner, source_token_account, destination_token_account) = match action {
        FractionalActionV1::Wrap => (ACTOR, [0; 32], [18; 32]),
        FractionalActionV1::Transfer => (ACTOR, [17; 32], [18; 32]),
        FractionalActionV1::WholeUnwrap
        | FractionalActionV1::WinningRedeem
        | FractionalActionV1::LosingZeroBurn => (ACTOR, [17; 32], [0; 32]),
        FractionalActionV1::Terminalize | FractionalActionV1::ZeroSupplyRetire => {
            ([0; 32], [0; 32], [0; 32])
        }
    };
    let terminal_outcome = match action {
        FractionalActionV1::WinningRedeem | FractionalActionV1::Terminalize => outcome,
        FractionalActionV1::LosingZeroBurn => outcome.checked_add(1).unwrap(),
        FractionalActionV1::ZeroSupplyRetire => 1,
        FractionalActionV1::Wrap
        | FractionalActionV1::Transfer
        | FractionalActionV1::WholeUnwrap => NO_TERMINAL_OUTCOME_V1,
    };
    FractionalFamilyRequestV1::new(
        action,
        FractionalFamilyRequestInputV1 {
            release_set: RELEASE,
            market: LOGICAL_MARKET,
            product_record: PRODUCT_RECORD,
            result_domain: [14; 32],
            terms: [15; 32],
            token_behavior: [16; 32],
            owner,
            source_token_account,
            destination_token_account,
            terminal_digest: if action.requires_terminal() {
                [19; 32]
            } else {
                [0; 32]
            },
            expected_revision: 40,
            quantity,
            outcome,
            terminal_outcome,
        },
    )
    .unwrap()
}

fn input<'a>(
    state: &'a State,
    action: FractionalActionV1,
    outcome: u32,
    quantity: u64,
) -> FractionalSignedDeltaInputV1<'a> {
    let (native, collateral, post, burns, actor) = match action {
        FractionalActionV1::Wrap => (2, 0, Some(5), &[][..], Some(state.actor.as_slice())),
        FractionalActionV1::WholeUnwrap => (2, 0, Some(1), &[][..], Some(state.actor.as_slice())),
        FractionalActionV1::WinningRedeem => (2, 2, Some(1), &[][..], None),
        FractionalActionV1::ZeroSupplyRetire => (0, 0, None, &[3, 0, 5][..], None),
        _ => (0, 0, None, &[][..], None),
    };
    FractionalSignedDeltaInputV1 {
        request: request(action, outcome, quantity),
        semantic_product_id: PRODUCT,
        market_account: MARKET_ACCOUNT,
        market_bytes: &state.market,
        linked_basis_record_digest: LINKED_BASIS,
        claims_program: CLAIMS,
        reserve_owner: RESERVE,
        reserve_position_bytes: &state.reserve,
        actor_position_bytes: actor,
        native_claims: native,
        collateral_atoms: collateral,
        expected_post_reserve_native_claims: post,
        retirement_native_burns: burns,
        post_fractional_revision: 41,
    }
}

fn dummy_row(claim_count: u32) -> PositionDeltaV3 {
    PositionDeltaV3::new(
        PositionDeltaInputV3 {
            position_index: 0,
            outcome: 0,
            delta: SignedDeltaV3::new(DeltaDirectionV3::Debit, 1).unwrap(),
        },
        2,
        claim_count,
    )
    .unwrap()
}

fn lower(
    input: FractionalSignedDeltaInputV1<'_>,
) -> (
    FractionalSignedDeltaLoweringV1,
    Vec<u8>,
    Vec<u8>,
    Vec<Vec<u8>>,
) {
    let shape = fractional_signed_delta_shape_v1(input).unwrap();
    let neutral = SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).unwrap();
    let mut aggregates = vec![neutral; usize::try_from(shape.claim_count()).unwrap()];
    let mut rows = vec![
        dummy_row(shape.claim_count());
        usize::try_from(shape.position_delta_count()).unwrap()
    ];
    let mut scratch = vec![0; shape.packet_bytes()];
    let mut packet = vec![0; shape.packet_bytes()];
    let mut market = vec![0; input.market_bytes.len()];
    let mut positions = vec![
        vec![0; input.reserve_position_bytes.len()];
        usize::try_from(shape.position_count()).unwrap()
    ];
    let mut position_refs: Vec<&mut [u8]> = positions.iter_mut().map(Vec::as_mut_slice).collect();
    let lowered = lower_fractional_signed_delta_v1(
        input,
        &mut aggregates,
        &mut rows,
        &mut scratch,
        &mut packet,
        &mut market,
        &mut position_refs,
    )
    .unwrap();
    (lowered, packet, market, positions)
}

#[test]
fn wrap_and_whole_unwrap_lower_to_canonical_sorted_transfers() {
    let state = state(&[20, 30, 40], &[3, 4, 5], &[7, 8, 9]);
    for action in [FractionalActionV1::Wrap, FractionalActionV1::WholeUnwrap] {
        let (lowered, packet, post_market, positions) = lower(input(&state, action, 0, 2));
        let plan = SignedDeltaPlanV3::decode(&packet).unwrap();
        assert_eq!(plan.position_count(), 2);
        assert_eq!(plan.position(0).unwrap().owner(), RESERVE);
        assert_eq!(plan.position(1).unwrap().owner(), ACTOR);
        assert_eq!(
            plan.aggregate_delta(0).unwrap().direction(),
            DeltaDirectionV3::Neutral
        );
        let market = LiabilityBasisMarketViewV2::decode(&post_market).unwrap();
        assert_eq!(market.revision, 12);
        assert_eq!(market.supply(&post_market, 0), Ok(20));
        let reserve_bytes = positions.first().unwrap();
        let actor_bytes = positions.get(1).unwrap();
        let reserve = LiabilityBasisPositionViewV2::decode(reserve_bytes).unwrap();
        let actor = LiabilityBasisPositionViewV2::decode(actor_bytes).unwrap();
        let expected = if action == FractionalActionV1::Wrap {
            (5, 5)
        } else {
            (1, 9)
        };
        assert_eq!(reserve.balance(reserve_bytes, 0), Ok(expected.0));
        assert_eq!(actor.balance(actor_bytes, 0), Ok(expected.1));
        assert_eq!(lowered.post_fractional_revision(), 41);
    }
}

#[test]
fn winning_and_runtime_width_retirement_debit_supply_and_reserve_exactly() {
    let winning_state = state(&[20, 30, 40], &[3, 4, 5], &[7, 8, 9]);
    let (winning, packet, market, positions) = lower(input(
        &winning_state,
        FractionalActionV1::WinningRedeem,
        0,
        23,
    ));
    let plan = SignedDeltaPlanV3::decode(&packet).unwrap();
    assert_eq!(
        plan.aggregate_delta(0).unwrap().direction(),
        DeltaDirectionV3::Debit
    );
    assert_eq!(plan.aggregate_delta(0).unwrap().magnitude(), 2);
    assert_eq!(winning.collateral_atoms(), 2);
    assert_eq!(
        LiabilityBasisMarketViewV2::decode(&market)
            .unwrap()
            .supply(&market, 0),
        Ok(18)
    );
    let winning_reserve_bytes = positions.first().unwrap();
    assert_eq!(
        LiabilityBasisPositionViewV2::decode(winning_reserve_bytes)
            .unwrap()
            .balance(winning_reserve_bytes, 0),
        Ok(1)
    );

    let retire_state = state(&[10, 20, 30], &[3, 0, 5], &[7, 8, 9]);
    let (retire, packet, market, positions) = lower(input(
        &retire_state,
        FractionalActionV1::ZeroSupplyRetire,
        NO_TERMINAL_OUTCOME_V1,
        0,
    ));
    let plan = SignedDeltaPlanV3::decode(&packet).unwrap();
    assert_eq!(plan.position_delta_count(), 2);
    assert_eq!(retire.shape().claim_count(), 3);
    let market_view = LiabilityBasisMarketViewV2::decode(&market).unwrap();
    let reserve_bytes = positions.first().unwrap();
    let reserve = LiabilityBasisPositionViewV2::decode(reserve_bytes).unwrap();
    assert_eq!(market_view.supply(&market, 0), Ok(7));
    assert_eq!(market_view.supply(&market, 2), Ok(25));
    assert_eq!(reserve.balance(reserve_bytes, 0), Ok(0));
    assert_eq!(reserve.balance(reserve_bytes, 2), Ok(0));
}

#[test]
fn canonical_receipt_and_exact_post_resources_validate_without_fractional_receipt() {
    let state = state(&[20, 30, 40], &[3, 4, 5], &[7, 8, 9]);
    let (lowered, packet, market, positions) =
        lower(input(&state, FractionalActionV1::WinningRedeem, 0, 23));
    let plan = SignedDeltaPlanV3::decode(&packet).unwrap();
    let receipt = SignedDeltaReceiptV3::new(
        plan,
        lowered.packet_digest(),
        lowered.table_digest(),
        CLAIMS,
        lowered.post_resource_digest(),
        lowered.post_market_revision(),
    )
    .unwrap()
    .to_bytes();
    let position_refs: Vec<&[u8]> = positions.iter().map(Vec::as_slice).collect();
    assert_eq!(
        validate_fractional_signed_delta_postcondition_v1(
            lowered,
            &packet,
            &receipt,
            &market,
            &position_refs,
        ),
        Ok(())
    );
    let mut substituted = positions.clone();
    *substituted.first_mut().unwrap().last_mut().unwrap() ^= 1;
    let substituted_refs: Vec<&[u8]> = substituted.iter().map(Vec::as_slice).collect();
    assert_eq!(
        validate_fractional_signed_delta_postcondition_v1(
            lowered,
            &packet,
            &receipt,
            &market,
            &substituted_refs,
        ),
        Err(Error::ReceiptMismatch)
    );
}

#[test]
fn no_native_effect_alias_stale_economics_and_substitution_refuse() {
    let state = state(&[20, 30, 40], &[3, 4, 5], &[7, 8, 9]);
    for action in [
        FractionalActionV1::Transfer,
        FractionalActionV1::LosingZeroBurn,
        FractionalActionV1::Terminalize,
    ] {
        let quantity = if action.carries_quantity() { 1 } else { 0 };
        assert_eq!(
            fractional_signed_delta_shape_v1(input(&state, action, 0, quantity)),
            Err(Error::NoClaimsMutation)
        );
    }
    let mut aliased = input(&state, FractionalActionV1::Wrap, 0, 2);
    aliased.reserve_owner = ACTOR;
    assert_eq!(
        fractional_signed_delta_shape_v1(aliased),
        Err(Error::EconomicMismatch)
    );
    let mut stale = input(&state, FractionalActionV1::Wrap, 0, 2);
    stale.post_fractional_revision = 42;
    assert_eq!(
        fractional_signed_delta_shape_v1(stale),
        Err(Error::IdentityMismatch)
    );
    let mut wrong_product = input(&state, FractionalActionV1::Wrap, 0, 2);
    wrong_product.semantic_product_id = [99; 32];
    assert_eq!(
        fractional_signed_delta_shape_v1(wrong_product),
        Err(Error::IdentityMismatch)
    );
    let mut wrong_reserve = input(&state, FractionalActionV1::Wrap, 0, 2);
    wrong_reserve.expected_post_reserve_native_claims = Some(6);
    let shape = fractional_signed_delta_shape_v1(wrong_reserve).unwrap();
    let neutral = SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).unwrap();
    let mut aggregates = vec![neutral; usize::try_from(shape.claim_count()).unwrap()];
    let mut rows = vec![dummy_row(shape.claim_count()); 2];
    let mut scratch = vec![0xa5; shape.packet_bytes()];
    let mut output = vec![0x5a; shape.packet_bytes()];
    let output_before = output.clone();
    let mut market = vec![0; wrong_reserve.market_bytes.len()];
    let mut p0 = vec![0; wrong_reserve.reserve_position_bytes.len()];
    let mut p1 = vec![0; wrong_reserve.reserve_position_bytes.len()];
    assert_eq!(
        lower_fractional_signed_delta_v1(
            wrong_reserve,
            &mut aggregates,
            &mut rows,
            &mut scratch,
            &mut output,
            &mut market,
            &mut [p0.as_mut_slice(), p1.as_mut_slice()],
        ),
        Err(Error::EconomicMismatch)
    );
    assert_eq!(output, output_before);
}

#[test]
fn full_u64_overflow_refuses_explicitly() {
    let state = state(&[u64::MAX, 1, 1], &[u64::MAX, 0, 0], &[u64::MAX, 0, 0]);
    let input = input(&state, FractionalActionV1::Wrap, 0, 2);
    let shape = fractional_signed_delta_shape_v1(input).unwrap();
    let neutral = SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).unwrap();
    let mut aggregates = vec![neutral; 3];
    let mut rows = vec![dummy_row(3); 2];
    let mut scratch = vec![0; shape.packet_bytes()];
    let mut output = vec![0; shape.packet_bytes()];
    let mut market = vec![0; state.market.len()];
    let mut p0 = vec![0; state.reserve.len()];
    let mut p1 = vec![0; state.reserve.len()];
    assert_eq!(
        lower_fractional_signed_delta_v1(
            input,
            &mut aggregates,
            &mut rows,
            &mut scratch,
            &mut output,
            &mut market,
            &mut [p0.as_mut_slice(), p1.as_mut_slice()],
        ),
        Err(Error::Arithmetic)
    );
}
