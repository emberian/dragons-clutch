use super::physical::*;
use dclutch_claims_svm::{ClaimsPlanV1, ClaimsReceiptV1};
use dclutch_custody_contract::{CustodyReceiptV1, ReceiptEvidenceV1};
use dclutch_direct_codec::{
    intent_v2::CompactIntentV2,
    successor::{
        AuthenticatedCompactIntentV2, DIRECT_MAKER_REPLAY_BYTES_V1,
        DIRECT_REGISTERED_RECORD_BYTES_V2, DirectExecutionConfigV1, DirectRegisteredIntentV2,
        DirectRootStateV1, MakerReplayFirstUseV1, MakerReplayObservationV1, MakerReplayRootV1,
        MakerReplayVacancyV1, RegisteredExecutionV2, RegisteredIntentCreationV2,
        RegisteredOrdinaryInputV2, RegisteredParticipantV2, RegisteredRecordAfterFillV2,
        RegisteredRecordFirstUseV2, register_intent_v2,
    },
};
use solana_program::hash::hash;

fn id(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn config(fee_basis_points: u16, fee_recipient: [u8; 32]) -> DirectExecutionConfigV1 {
    DirectExecutionConfigV1::new(100, fee_basis_points, fee_recipient).expect("config")
}

fn intent(
    side: u8,
    maker_nonce: u64,
    collateral_account: [u8; 32],
    fee_basis_points: u16,
) -> CompactIntentV2 {
    CompactIntentV2 {
        side,
        lifecycle: 2,
        outcome: 1,
        market: id(1),
        generation: 4,
        nonce: maker_nonce,
        valid_from: 2,
        valid_through: 20,
        maximum_fill: 100,
        limit_price: if side == 0 { 40 } else { 60 },
        fee_basis_points,
        collateral_account,
    }
}

fn register(
    root: DirectRootStateV1,
    maker: [u8; 32],
    signed: CompactIntentV2,
    selected: DirectExecutionConfigV1,
    bump: u8,
) -> RegisteredIntentCreationV2 {
    register_intent_v2(
        root,
        MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(bump, 3)),
        AuthenticatedCompactIntentV2::from_adjacent_ed25519(maker, signed)
            .expect("authenticated intent"),
        selected,
        3,
        Some(MakerReplayFirstUseV1 {
            rent_owner: id(90),
            rent_principal: 100,
        }),
        RegisteredRecordFirstUseV2 {
            bump,
            observed_lamports: 7,
            rent_owner: id(91),
            rent_principal: 100,
        },
    )
    .expect("registration")
}

fn fixture(
    fee_basis_points: u16,
) -> (
    RegisteredOrdinaryInputV2,
    DirectOrdinaryPhysicalContextV2,
    DirectOrdinaryCollateralFrameV2,
) {
    let selected = config(fee_basis_points, id(6));
    let seller = register(
        DirectRootStateV1::new(),
        id(2),
        intent(0, 0, id(20), fee_basis_points),
        selected,
        2,
    );
    let buyer = register(
        seller.root,
        id(3),
        intent(1, 0, id(21), fee_basis_points),
        selected,
        3,
    );
    (
        RegisteredOrdinaryInputV2 {
            root: buyer.root,
            seller: RegisteredParticipantV2 {
                maker_root: seller.maker_root,
                record: seller.record,
                observed_record_lamports: 100,
            },
            buyer: RegisteredParticipantV2 {
                maker_root: buyer.maker_root,
                record: buyer.record,
                observed_record_lamports: 100,
            },
            execution: RegisteredExecutionV2 {
                config: selected,
                outcome_count: 3,
                slot: 5,
                fill: 20,
                execution_price: 50,
            },
        },
        DirectOrdinaryPhysicalContextV2 {
            trading_program: id(10),
            claims_program: id(11),
            custody_program: id(12),
            custody_authority: id(23),
            release_set: id(13),
            market: id(1),
            realm: id(14),
            mint: id(15),
            token_program: id(16),
            parent_request_digest: id(17),
            buyer_maker_root: id(18),
            buyer_record: id(19),
            generation: 4,
            claims_market_revision: 7,
            seller_position_revision: 8,
            buyer_position_revision: 9,
            custody_replay_revision: 5,
        },
        DirectOrdinaryCollateralFrameV2 {
            buyer_source: DirectExternalDebitV2 {
                account: id(21),
                owner: id(3),
                delegate: id(23),
                delegated_amount: buyer.record.reserved_collateral(),
                balance: 100,
            },
            seller_destination: DirectExternalCollateralV2 {
                account: id(20),
                owner: id(2),
                balance: 30,
            },
            fee_destination: DirectExternalCollateralV2 {
                account: id(22),
                owner: id(6),
                balance: 40,
            },
        },
    )
}

fn prepare(
    input: RegisteredOrdinaryInputV2,
    context: DirectOrdinaryPhysicalContextV2,
    collateral: DirectOrdinaryCollateralFrameV2,
) -> (DirectOrdinaryPhysicalPlanV2, [u8; 232]) {
    let mut quantities = [0_u8; 24];
    let mut scratch = [0_u8; 232];
    let mut output = [0xa5_u8; 232];
    let plan = prepare_registered_ordinary_physical_v2(
        input,
        context,
        collateral,
        &mut quantities,
        &mut scratch,
        &mut output,
    )
    .expect("physical plan");
    (plan, output)
}

fn custody_effect(plan: DirectOrdinaryPhysicalPlanV2, index: usize) -> DirectCustodyEffectV2 {
    plan.custody
        .get(index)
        .copied()
        .flatten()
        .expect("positive canonical Custody effect")
}

#[test]
fn ordinary_projection_conserves_quote_and_orders_net_before_combined_fee() {
    let (input, context, collateral) = fixture(1_000);
    let (plan, claims_bytes) = prepare(input, context, collateral);
    assert_eq!(plan.settlement.gross_collateral, 10);
    assert_eq!(plan.settlement.seller_net_collateral_credit, 9);
    assert_eq!(plan.settlement.total_fee_transfer, 2);
    assert_eq!(plan.settlement.buyer_collateral_debit, 11);
    assert_eq!(plan.custody_count, 2);
    assert_eq!(plan.buyer_source_after, 89);
    assert_eq!(plan.buyer_delegated_after, 55);
    assert_eq!(plan.seller_destination_after, 39);
    assert_eq!(plan.fee_destination_after, 42);

    let net = custody_effect(plan, 0);
    assert_eq!(net.request.amount, 9);
    assert_eq!(net.request.semantic.transfer_index, 0);
    assert_eq!(net.request.expected_revision, 5);
    assert_eq!(net.request.resulting_revision, 6);
    assert_eq!(net.request.source, collateral.buyer_source.account);
    assert_eq!(
        net.request.destination,
        collateral.seller_destination.account
    );
    assert_eq!(net.request.semantic.source_owner, id(3));
    assert_eq!(net.request.semantic.destination_owner, id(2));

    let fee = custody_effect(plan, 1);
    assert_eq!(fee.request.amount, 2);
    assert_eq!(fee.request.semantic.transfer_index, 1);
    assert_eq!(fee.request.expected_revision, 6);
    assert_eq!(fee.request.resulting_revision, 7);
    assert_eq!(fee.request.destination, collateral.fee_destination.account);
    assert_eq!(fee.request.semantic.destination_owner, id(6));

    let claims = ClaimsPlanV1::decode(&claims_bytes).expect("Claims plan");
    assert_eq!(claims.source_owner(), id(2));
    assert_eq!(claims.destination_owner(), id(3));
    assert_eq!(claims.quantity(0), Ok(0));
    assert_eq!(claims.quantity(1), Ok(20));
    assert_eq!(claims.quantity(2), Ok(0));
}

#[test]
fn zero_and_full_fee_profiles_emit_only_the_positive_canonical_transfer() {
    let (input, context, collateral) = fixture(0);
    let (zero_fee, _) = prepare(input, context, collateral);
    assert_eq!(zero_fee.custody_count, 1);
    assert_eq!(custody_effect(zero_fee, 0).request.amount, 10);
    assert_eq!(zero_fee.custody.get(1), Some(&None));

    let (input, context, collateral) = fixture(10_000);
    let (full_fee, _) = prepare(input, context, collateral);
    assert_eq!(full_fee.settlement.seller_net_collateral_credit, 0);
    assert_eq!(full_fee.settlement.total_fee_transfer, 20);
    assert_eq!(full_fee.custody_count, 1);
    let fee = custody_effect(full_fee, 0);
    assert_eq!(fee.request.amount, 20);
    assert_eq!(fee.request.semantic.transfer_index, 0);
    assert_eq!(fee.request.destination, collateral.fee_destination.account);
    assert_eq!(full_fee.custody.get(1), Some(&None));
}

#[test]
fn hostile_endpoint_owner_balance_width_and_output_refuse_atomically() {
    let (input, context, collateral) = fixture(1_000);
    let mut quantities = [0_u8; 24];
    let mut scratch = [0_u8; 232];
    let mut output = [0xa5_u8; 232];
    let before = output;
    let wrong_owner = DirectOrdinaryCollateralFrameV2 {
        buyer_source: DirectExternalDebitV2 {
            owner: id(99),
            ..collateral.buyer_source
        },
        ..collateral
    };
    assert_eq!(
        prepare_registered_ordinary_physical_v2(
            input,
            context,
            wrong_owner,
            &mut quantities,
            &mut scratch,
            &mut output,
        ),
        Err(DirectPhysicalError::Binding)
    );
    assert_eq!(output, before);

    let wrong_delegate = DirectOrdinaryCollateralFrameV2 {
        buyer_source: DirectExternalDebitV2 {
            delegate: id(98),
            ..collateral.buyer_source
        },
        ..collateral
    };
    assert_eq!(
        prepare_registered_ordinary_physical_v2(
            input,
            context,
            wrong_delegate,
            &mut quantities,
            &mut scratch,
            &mut output,
        ),
        Err(DirectPhysicalError::Binding)
    );
    assert_eq!(output, before);

    let allowance_substitution = DirectOrdinaryCollateralFrameV2 {
        buyer_source: DirectExternalDebitV2 {
            delegated_amount: collateral
                .buyer_source
                .delegated_amount
                .checked_add(1)
                .expect("hostile allowance"),
            ..collateral.buyer_source
        },
        ..collateral
    };
    assert_eq!(
        prepare_registered_ordinary_physical_v2(
            input,
            context,
            allowance_substitution,
            &mut quantities,
            &mut scratch,
            &mut output,
        ),
        Err(DirectPhysicalError::Binding)
    );
    assert_eq!(output, before);

    let underfunded = DirectOrdinaryCollateralFrameV2 {
        buyer_source: DirectExternalDebitV2 {
            balance: 10,
            ..collateral.buyer_source
        },
        ..collateral
    };
    assert_eq!(
        prepare_registered_ordinary_physical_v2(
            input,
            context,
            underfunded,
            &mut quantities,
            &mut scratch,
            &mut output,
        ),
        Err(DirectPhysicalError::Arithmetic)
    );
    assert_eq!(output, before);

    assert_eq!(
        prepare_registered_ordinary_physical_v2(
            input,
            context,
            collateral,
            &mut quantities[..16],
            &mut scratch,
            &mut output,
        ),
        Err(DirectPhysicalError::Width)
    );
    assert_eq!(output, before);
}

#[test]
fn exact_child_receipts_accept_and_substitutions_refuse() {
    let (input, context, collateral) = fixture(1_000);
    let (plan, claims_bytes) = prepare(input, context, collateral);
    let claims_plan = ClaimsPlanV1::decode(&claims_bytes).expect("Claims plan");
    let claims_receipt = ClaimsReceiptV1::new(
        claims_plan,
        hash(&claims_bytes).to_bytes(),
        context.claims_program,
        context.claims_market_revision + 1,
        context.seller_position_revision + 1,
        context.buyer_position_revision + 1,
        0,
        id(70),
    )
    .expect("Claims receipt")
    .to_bytes();
    verify_direct_claims_receipt_v2(context, &claims_bytes, &claims_receipt)
        .expect("Claims acknowledgement");
    let mut hostile_claims = claims_receipt;
    *hostile_claims.get_mut(80).expect("hostile digest byte") ^= 1;
    assert!(verify_direct_claims_receipt_v2(context, &claims_bytes, &hostile_claims).is_err());

    for effect in plan.custody.into_iter().flatten() {
        let request_bytes = effect.request.to_bytes().expect("Custody request");
        let poststate = id(71);
        let receipt = CustodyReceiptV1::new(
            effect.request,
            hash(&request_bytes).to_bytes(),
            ReceiptEvidenceV1 {
                source_before: effect
                    .source_after
                    .checked_add(effect.request.amount)
                    .expect("source before"),
                source_after: effect.source_after,
                destination_before: effect
                    .destination_after
                    .checked_sub(effect.request.amount)
                    .expect("destination before"),
                destination_after: effect.destination_after,
                poststate_commitment: id(72),
                replay_state_digest: poststate,
            },
        )
        .expect("Custody receipt")
        .to_bytes()
        .expect("Custody receipt bytes");
        verify_direct_custody_receipt_v2(effect, &receipt, poststate)
            .expect("Custody acknowledgement");
        assert_eq!(
            verify_direct_custody_receipt_v2(effect, &receipt, id(99)),
            Err(DirectPhysicalError::Custody)
        );
    }
}

#[test]
fn state_candidate_is_commit_last_for_partial_and_terminal_records() {
    let (input, context, collateral) = fixture(1_000);
    let selected = input.execution.config;
    let width = input.execution.outcome_count;
    let (partial, _) = prepare(input, context, collateral);
    let mut seller_maker = [0xa5; DIRECT_MAKER_REPLAY_BYTES_V1];
    let mut buyer_maker = [0xa5; DIRECT_MAKER_REPLAY_BYTES_V1];
    let mut seller_scratch = [0; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let mut buyer_scratch = [0; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let mut seller_record = [0xa5; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let mut buyer_record = [0xa5; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let candidate = encode_registered_ordinary_state_candidate_v2(
        partial.settlement,
        selected,
        width,
        DirectOrdinaryStateBuffersV2 {
            seller_maker_output: &mut seller_maker,
            buyer_maker_output: &mut buyer_maker,
            seller_record_scratch: &mut seller_scratch,
            buyer_record_scratch: &mut buyer_scratch,
            seller_record_output: &mut seller_record,
            buyer_record_output: &mut buyer_record,
        },
    )
    .expect("partial state candidate");
    assert_eq!(candidate.seller_record, DirectRecordCommitV2::WriteLive);
    assert_eq!(candidate.buyer_record, DirectRecordCommitV2::WriteLive);
    assert_eq!(
        MakerReplayRootV1::decode(&seller_maker),
        Ok(partial.settlement.seller.maker_root)
    );
    assert_eq!(
        MakerReplayRootV1::decode(&buyer_maker),
        Ok(partial.settlement.buyer.maker_root)
    );
    assert_eq!(
        DirectRegisteredIntentV2::decode_selected(selected, width, &seller_record),
        match partial.settlement.seller.record {
            RegisteredRecordAfterFillV2::Live(record) => Ok(record),
            RegisteredRecordAfterFillV2::Closed(_) => panic!("partial seller must remain live"),
        }
    );

    let terminal_input = RegisteredOrdinaryInputV2 {
        execution: RegisteredExecutionV2 {
            fill: 100,
            ..input.execution
        },
        ..input
    };
    let (terminal, _) = prepare(terminal_input, context, collateral);
    assert_eq!(terminal.buyer_delegated_after, 11);
    let mut terminal_seller_maker = [0xa5; DIRECT_MAKER_REPLAY_BYTES_V1];
    let mut terminal_buyer_maker = [0xa5; DIRECT_MAKER_REPLAY_BYTES_V1];
    let mut terminal_seller_scratch = [0; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let mut terminal_buyer_scratch = [0; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let mut terminal_seller_record = [0xa5; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let mut terminal_buyer_record = [0xa5; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let closed_record_before = terminal_seller_record;
    let terminal_candidate = encode_registered_ordinary_state_candidate_v2(
        terminal.settlement,
        selected,
        width,
        DirectOrdinaryStateBuffersV2 {
            seller_maker_output: &mut terminal_seller_maker,
            buyer_maker_output: &mut terminal_buyer_maker,
            seller_record_scratch: &mut terminal_seller_scratch,
            buyer_record_scratch: &mut terminal_buyer_scratch,
            seller_record_output: &mut terminal_seller_record,
            buyer_record_output: &mut terminal_buyer_record,
        },
    )
    .expect("terminal state candidate");
    assert!(matches!(
        terminal_candidate.seller_record,
        DirectRecordCommitV2::Close(_)
    ));
    assert!(matches!(
        terminal_candidate.buyer_record,
        DirectRecordCommitV2::Close(_)
    ));
    assert_eq!(terminal_seller_record, closed_record_before);
    assert_eq!(terminal_buyer_record, closed_record_before);

    let seller_before = terminal_seller_maker;
    let buyer_before = terminal_buyer_maker;
    let seller_record_before = terminal_seller_record;
    let buyer_record_before = terminal_buyer_record;
    assert_eq!(
        encode_registered_ordinary_state_candidate_v2(
            terminal.settlement,
            selected,
            width,
            DirectOrdinaryStateBuffersV2 {
                seller_maker_output: terminal_seller_maker
                    .get_mut(..16)
                    .expect("short hostile maker output"),
                buyer_maker_output: &mut terminal_buyer_maker,
                seller_record_scratch: &mut terminal_seller_scratch,
                buyer_record_scratch: &mut terminal_buyer_scratch,
                seller_record_output: &mut terminal_seller_record,
                buyer_record_output: &mut terminal_buyer_record,
            },
        ),
        Err(DirectPhysicalError::Width)
    );
    assert_eq!(terminal_seller_maker.get(..16), seller_before.get(..16));
    assert_eq!(terminal_buyer_maker, buyer_before);
    assert_eq!(terminal_seller_record, seller_record_before);
    assert_eq!(terminal_buyer_record, buyer_record_before);
}

#[test]
fn consistent_seller_fee_alias_accumulates_both_ordered_credits() {
    let (input, context, collateral) = fixture(1_000);
    let aliased = DirectOrdinaryCollateralFrameV2 {
        seller_destination: DirectExternalCollateralV2 {
            account: id(22),
            owner: id(6),
            balance: 40,
        },
        fee_destination: DirectExternalCollateralV2 {
            account: id(22),
            owner: id(6),
            balance: 40,
        },
        ..collateral
    };
    let seller_intent = CompactIntentV2 {
        collateral_account: id(22),
        ..input.seller.record.intent()
    };
    let selected = input.execution.config;
    let seller = register(DirectRootStateV1::new(), id(6), seller_intent, selected, 4);
    let buyer = register(seller.root, id(3), input.buyer.record.intent(), selected, 5);
    let aliased_input = RegisteredOrdinaryInputV2 {
        root: buyer.root,
        seller: RegisteredParticipantV2 {
            maker_root: seller.maker_root,
            record: seller.record,
            observed_record_lamports: 100,
        },
        buyer: RegisteredParticipantV2 {
            maker_root: buyer.maker_root,
            record: buyer.record,
            observed_record_lamports: 100,
        },
        ..input
    };
    let (plan, _) = prepare(aliased_input, context, aliased);
    assert_eq!(plan.seller_destination_after, 51);
    assert_eq!(plan.fee_destination_after, 51);
}
