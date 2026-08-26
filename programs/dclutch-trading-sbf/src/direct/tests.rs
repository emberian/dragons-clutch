use super::{buy_escrow::*, complementary::*, physical::*};
use dclutch_claims_svm::{
    CallerRole as ClaimsCallerRole, ClaimsPlanV1, ClaimsReceiptV1,
    affine_batch_v2::{
        AffineBatchPlanInputV2, AffineBatchPlanV2, AffineBatchPositionV2, AffineBatchReceiptV2,
        AffineBatchRowInputV2, AffineBatchRowV2, DeltaDirectionV2, SignedMagnitudeV2,
    },
};
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_REPLAY_PDA_DOMAIN_V1, CUSTODY_VAULT_PDA_DOMAIN_V1,
    CompartmentV1, CustodyReplayV1, OperationV1,
};
use dclutch_direct_codec::{
    intent_v2::CompactIntentV2,
    successor::{
        AuthenticatedCompactIntentV2, ComplementaryActionV2, ComplementaryInputV2,
        ComplementaryParticipantsV2, ComplementarySettlementV2, DIRECT_MAKER_REPLAY_BYTES_V1,
        DIRECT_REGISTERED_RECORD_BYTES_V2, DirectExecutionConfigV1, DirectRegisteredIntentV2,
        DirectRootStateV1, MakerReplayFirstUseV1, MakerReplayObservationV1, MakerReplayRootV1,
        MakerReplayVacancyV1, RegisteredExecutionV2, RegisteredFillCandidateV2,
        RegisteredFillInputV2, RegisteredIntentCreationV2, RegisteredIntentSeedsV2,
        RegisteredOrdinaryInputV2, RegisteredParticipantV2, RegisteredRecordAfterFillV2,
        RegisteredRecordFirstUseV2, RegisteredTerminalEvidenceV2, preview_registered_fill_v2,
        register_intent_v2, settle_registered_complementary_v2, terminate_registered_intent_v2,
    },
};
use dclutch_market_core_codec::{
    Binding, CoreMarketViewV1, CoreReferenceObservationV1, CoreState, Identity, MarketIdentity,
    Phase, Product, Readiness, Realm, ReleaseSet,
};
use solana_program::hash::{hash, hashv};
use solana_program::pubkey::Pubkey;

fn id(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn identity(byte: u8) -> Identity {
    Identity::new(id(byte)).expect("identity")
}

fn binding(program: u8, artifact: u8, semantic: u8) -> Binding {
    Binding {
        program: identity(program),
        artifact_release: identity(artifact),
        semantic_release: identity(semantic),
    }
}

fn core_market_view(outcome_count: u32) -> CoreMarketViewV1 {
    let release_set = ReleaseSet {
        release_set_id: identity(13),
        bindings: [
            binding(9, 50, 60),
            binding(11, 51, 61),
            binding(10, 52, 62),
            binding(12, 53, 63),
            binding(20, 54, 64),
        ],
    };
    let product = Product {
        product_record: identity(24),
        product_id: identity(25),
        result_domain: identity(31),
        portfolio: identity(32),
        coordinate_domain: identity(33),
        result_unit: identity(34),
        claim_basis: identity(35),
        liability_basis: identity(36),
        representation_release: identity(37),
        mapping_release: identity(38),
        outcome_count,
    };
    let realm = Realm {
        realm_id: identity(14),
        collateral_mint: identity(15),
        token_program: identity(16),
        collateral_release: identity(29),
    };
    let state = CoreState {
        phase: Phase::Open,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity: MarketIdentity {
            market_id: identity(1),
            realm_id: realm.realm_id,
            product_record: product.product_record,
            product_id: product.product_id,
            resolution_policy: identity(26),
            capability_manifest: identity(27),
            selected_release_set: release_set.release_set_id,
            registry_program: identity(8),
            generation: 4,
        },
        outstanding_capabilities: 0,
        rent_beneficiary: identity(90),
        terminal_receipt: None,
    };
    CoreMarketViewV1::authenticate(
        state,
        identity(1),
        identity(40),
        CoreReferenceObservationV1 {
            realm,
            product,
            release_set,
            realm_record_authenticated: true,
            product_graph_authenticated: true,
            release_set_record_authenticated: true,
            claims_aggregate_derivation_authenticated: true,
        },
    )
    .expect("Core Market view")
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

fn fixture(fee_basis_points: u16) -> (RegisteredOrdinaryInputV2, DirectOrdinaryClaimsContextV2) {
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
        DirectOrdinaryClaimsContextV2 {
            core_market: core_market_view(3),
            trading_program: id(10),
            claims_program: id(11),
            parent_request_digest: id(17),
            claims_market_revision: 7,
            seller_position_revision: 8,
            buyer_position_revision: 9,
        },
    )
}

fn prepare(
    input: RegisteredOrdinaryInputV2,
    context: DirectOrdinaryClaimsContextV2,
) -> (DirectOrdinaryClaimsPlanV2, [u8; 232]) {
    let mut quantities = [0_u8; 24];
    let mut scratch = [0_u8; 232];
    let mut output = [0xa5_u8; 232];
    let plan = prepare_registered_ordinary_claims_v2(
        input,
        context,
        &mut quantities,
        &mut scratch,
        &mut output,
    )
    .expect("physical plan");
    (plan, output)
}

#[test]
fn ordinary_claims_projection_binds_runtime_width_and_settlement() {
    let (input, context) = fixture(1_000);
    let (plan, claims_bytes) = prepare(input, context);
    assert_eq!(plan.settlement.gross_collateral, 10);
    assert_eq!(plan.settlement.seller_net_collateral_credit, 9);
    assert_eq!(plan.settlement.total_fee_transfer, 2);
    assert_eq!(plan.settlement.buyer_collateral_debit, 11);
    let claims = ClaimsPlanV1::decode(&claims_bytes).expect("Claims plan");
    assert_eq!(claims.source_owner(), id(2));
    assert_eq!(claims.destination_owner(), id(3));
    assert_eq!(claims.quantity(0), Ok(0));
    assert_eq!(claims.quantity(1), Ok(20));
    assert_eq!(claims.quantity(2), Ok(0));
}

#[test]
fn hostile_core_width_and_output_refuse_atomically() {
    let (input, context) = fixture(1_000);
    let mut quantities = [0_u8; 24];
    let mut scratch = [0_u8; 232];
    let mut output = [0xa5_u8; 232];
    let before = output;
    assert_eq!(
        prepare_registered_ordinary_claims_v2(
            input,
            context,
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
    let (input, context) = fixture(1_000);
    let (_plan, claims_bytes) = prepare(input, context);
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
}

#[test]
fn state_candidate_is_commit_last_for_partial_and_terminal_records() {
    let (input, context) = fixture(1_000);
    let selected = input.execution.config;
    let width = input.execution.outcome_count;
    let (partial, _) = prepare(input, context);
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
    let (terminal, _) = prepare(terminal_input, context);
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

fn complementary_context() -> DirectComplementaryPhysicalContextV2 {
    DirectComplementaryPhysicalContextV2 {
        trading_program: id(10),
        core_market: core_market_view(3),
        custody_authority: id(23),
        parent_request_digest: id(17),
        hoard_token_account: id(41),
        hoard_balance: 500,
        fee_destination: DirectExternalCollateralV2 {
            account: id(22),
            owner: id(6),
            balance: 100,
        },
    }
}

fn complementary_candidates(
    side: u8,
) -> (
    [MakerReplayRootV1; 3],
    [DirectRegisteredIntentV2; 3],
    [RegisteredFillCandidateV2; 3],
) {
    let prices = [20_u64, 30, 50];
    let seed = register(
        DirectRootStateV1::new(),
        id(70),
        CompactIntentV2 {
            side,
            lifecycle: 2,
            outcome: 0,
            market: id(1),
            generation: 4,
            nonce: 0,
            valid_from: 2,
            valid_through: 20,
            maximum_fill: 100,
            limit_price: 20,
            fee_basis_points: 1_000,
            collateral_account: id(30),
        },
        config(1_000, id(6)),
        1,
    );
    let mut roots = [seed.maker_root; 3];
    let mut records = [seed.record; 3];
    let mut root = DirectRootStateV1::new();
    for (index, price) in prices.iter().copied().enumerate() {
        let maker = id(u8::try_from(index + 2).expect("maker"));
        let outcome = u32::try_from(index).expect("outcome");
        let created = register(
            root,
            maker,
            CompactIntentV2 {
                side,
                lifecycle: 2,
                outcome,
                market: id(1),
                generation: 4,
                nonce: 0,
                valid_from: 2,
                valid_through: 20,
                maximum_fill: 100,
                limit_price: price,
                fee_basis_points: 1_000,
                collateral_account: id(u8::try_from(index + 30).expect("collateral")),
            },
            config(1_000, id(6)),
            u8::try_from(index + 2).expect("bump"),
        );
        root = created.root;
        *roots.get_mut(index).expect("root slot") = created.maker_root;
        *records.get_mut(index).expect("record slot") = created.record;
    }
    let first = preview_registered_fill_v2(RegisteredFillInputV2 {
        root,
        participant: RegisteredParticipantV2 {
            maker_root: *roots.first().expect("first root"),
            record: *records.first().expect("first record"),
            observed_record_lamports: 100,
        },
        execution: RegisteredExecutionV2 {
            config: config(1_000, id(6)),
            outcome_count: 3,
            slot: 5,
            fill: 100,
            execution_price: 20,
        },
    })
    .expect("first candidate");
    let mut scratch = [first; 3];
    settle_registered_complementary_v2(ComplementaryInputV2 {
        action: if side == 1 {
            ComplementaryActionV2::Split
        } else {
            ComplementaryActionV2::Merge
        },
        root,
        participants: ComplementaryParticipantsV2 {
            maker_roots: &roots,
            records: &records,
            record_lamports: &[100; 3],
            execution_prices: &prices,
        },
        scratch: &mut scratch,
        config: config(1_000, id(6)),
        outcome_count: 3,
        slot: 5,
        fill: 100,
    })
    .expect("complementary settlement");
    (roots, records, scratch)
}

#[test]
fn complementary_custody_routes_are_affine_ordered_and_hostile_bound() {
    let split_aggregate = validate_complementary_custody_aggregate_v2(
        ComplementaryActionV2::Split,
        ComplementarySettlementV2 {
            market_vault_transfer: 100,
            total_fee_transfer: 10,
        },
        config(1_000, id(6)),
        complementary_context(),
    )
    .expect("split aggregate preflight");
    assert_eq!(split_aggregate.hoard_after, 600);
    assert_eq!(split_aggregate.fee_after, 110);
    let (_buy_roots, buy_records, buy_candidates) = complementary_candidates(1);
    let buy_record = *buy_records.get(1).expect("buy record");
    let buy_candidate = *buy_candidates.get(1).expect("buy candidate");
    let buy_source = DirectExternalDebitV2 {
        account: buy_record.intent().collateral_account,
        owner: buy_record.maker(),
        delegate: id(23),
        delegated_amount: buy_record.reserved_collateral(),
        balance: 100,
    };
    let participant = DirectComplementaryParticipantV2 {
        maker_root: id(51),
        record: id(61),
        collateral: DirectComplementaryCollateralV2::BuySource(buy_source),
        custody_replay_revision: 10,
    };
    let projection = |route, participant| DirectComplementaryProjectionInputV2 {
        action: ComplementaryActionV2::Split,
        route,
        participant_index: 1,
        record_before: buy_record,
        candidate: buy_candidate,
        participant,
        config: config(1_000, id(6)),
        context: complementary_context(),
    };
    let principal = project_complementary_custody_effect_v2(projection(
        DirectComplementaryCustodyRouteV2::PrincipalOrNet,
        participant,
    ))
    .expect("split principal")
    .expect("positive split principal");
    assert_eq!(principal.request.amount, 30);
    assert_eq!(principal.request.source, buy_source.account);
    assert_eq!(principal.request.destination, id(41));
    assert_eq!(principal.request.destination_vault_context, id(40));
    assert_ne!(principal.request.destination_vault_context, id(1));
    assert_eq!(principal.request.semantic.transfer_index, 0);
    assert_eq!(principal.request.expected_revision, 10);
    assert_eq!(principal.terminal_delegated_amount, Some(0));
    let mut market_as_hoard_context = principal.request;
    market_as_hoard_context.destination_vault_context = id(1);
    assert_eq!(
        validate_complementary_custody_request_v2(principal, market_as_hoard_context),
        Err(DirectPhysicalError::Binding)
    );
    let fee = project_complementary_custody_effect_v2(projection(
        DirectComplementaryCustodyRouteV2::Fee,
        participant,
    ))
    .expect("split fee")
    .expect("positive split fee");
    assert_eq!(fee.request.amount, 3);
    assert_eq!(fee.request.destination, id(22));
    assert_eq!(fee.request.semantic.transfer_index, 1);
    assert_eq!(fee.request.expected_revision, 11);

    let hostile = DirectComplementaryParticipantV2 {
        collateral: DirectComplementaryCollateralV2::BuySource(DirectExternalDebitV2 {
            delegate: id(99),
            ..buy_source
        }),
        ..participant
    };
    assert_eq!(
        project_complementary_custody_effect_v2(projection(
            DirectComplementaryCustodyRouteV2::PrincipalOrNet,
            hostile,
        )),
        Err(DirectPhysicalError::Binding)
    );

    let merge_aggregate = validate_complementary_custody_aggregate_v2(
        ComplementaryActionV2::Merge,
        ComplementarySettlementV2 {
            market_vault_transfer: 100,
            total_fee_transfer: 10,
        },
        config(1_000, id(6)),
        complementary_context(),
    )
    .expect("merge aggregate preflight");
    assert_eq!(merge_aggregate.hoard_after, 400);
    let underfunded = DirectComplementaryPhysicalContextV2 {
        hoard_balance: 99,
        ..complementary_context()
    };
    assert_eq!(
        validate_complementary_custody_aggregate_v2(
            ComplementaryActionV2::Merge,
            ComplementarySettlementV2 {
                market_vault_transfer: 100,
                total_fee_transfer: 10,
            },
            config(1_000, id(6)),
            underfunded,
        ),
        Err(DirectPhysicalError::Arithmetic)
    );

    let (_sell_roots, sell_records, sell_candidates) = complementary_candidates(0);
    let sell_record = *sell_records.get(2).expect("sell record");
    let sell_candidate = *sell_candidates.get(2).expect("sell candidate");
    let seller = DirectExternalCollateralV2 {
        account: sell_record.intent().collateral_account,
        owner: sell_record.maker(),
        balance: 5,
    };
    let seller_participant = DirectComplementaryParticipantV2 {
        maker_root: id(52),
        record: id(62),
        collateral: DirectComplementaryCollateralV2::SellDestination(seller),
        custody_replay_revision: 20,
    };
    let merge_projection = |route| DirectComplementaryProjectionInputV2 {
        action: ComplementaryActionV2::Merge,
        route,
        participant_index: 2,
        record_before: sell_record,
        candidate: sell_candidate,
        participant: seller_participant,
        config: config(1_000, id(6)),
        context: complementary_context(),
    };
    let net = project_complementary_custody_effect_v2(merge_projection(
        DirectComplementaryCustodyRouteV2::PrincipalOrNet,
    ))
    .expect("merge net")
    .expect("positive merge net");
    assert_eq!(net.request.amount, 45);
    assert_eq!(net.request.source, id(41));
    assert_eq!(net.request.destination, seller.account);
    assert_eq!(net.request.source_vault_context, id(40));
    assert_eq!(net.request.semantic.transfer_index, 0);
    let merge_fee = project_complementary_custody_effect_v2(merge_projection(
        DirectComplementaryCustodyRouteV2::Fee,
    ))
    .expect("merge fee")
    .expect("positive merge fee");
    assert_eq!(merge_fee.request.amount, 5);
    assert_eq!(merge_fee.request.destination, id(22));
    assert_eq!(merge_fee.request.semantic.transfer_index, 1);
}

fn claims_context(fill: u64) -> DirectComplementaryClaimsContextV2 {
    DirectComplementaryClaimsContextV2 {
        core_market: core_market_view(3),
        claims_program: id(11),
        parent_request_digest: id(17),
        linked_basis_record_digest: id(39),
        claims_market_revision: 7,
        fill,
    }
}

fn signed(direction: DeltaDirectionV2, magnitude: u64) -> SignedMagnitudeV2 {
    SignedMagnitudeV2::new(direction, magnitude).expect("canonical delta")
}

fn complementary_claims_packet(
    action: ComplementaryActionV2,
    context: DirectComplementaryClaimsContextV2,
    position_owners: [[u8; 32]; 3],
    position_revisions: [u64; 3],
) -> [u8; 552] {
    let positions = [
        AffineBatchPositionV2::new(position_owners[0], position_revisions[0]).expect("Position 0"),
        AffineBatchPositionV2::new(position_owners[1], position_revisions[1]).expect("Position 1"),
        AffineBatchPositionV2::new(position_owners[2], position_revisions[2]).expect("Position 2"),
    ];
    let rows: [AffineBatchRowV2; 3] = core::array::from_fn(|index| {
        let outcome = u32::try_from(index).expect("outcome");
        let (source_present, destination_present, source_delta, destination_delta, aggregate) =
            match action {
                ComplementaryActionV2::Split => (
                    false,
                    true,
                    signed(DeltaDirectionV2::Neutral, 0),
                    signed(DeltaDirectionV2::Credit, context.fill),
                    signed(DeltaDirectionV2::Credit, context.fill),
                ),
                ComplementaryActionV2::Merge => (
                    true,
                    false,
                    signed(DeltaDirectionV2::Debit, context.fill),
                    signed(DeltaDirectionV2::Neutral, 0),
                    signed(DeltaDirectionV2::Debit, context.fill),
                ),
            };
        AffineBatchRowV2::new(
            AffineBatchRowInputV2 {
                source_present,
                destination_present,
                outcome,
                source_position_index: if source_present { outcome } else { 0 },
                destination_position_index: if destination_present { outcome } else { 0 },
                aggregate_delta: aggregate,
                source_delta,
                destination_delta,
            },
            3,
            3,
        )
        .expect("affine row")
    });
    let mut bytes = [0_u8; 552];
    let product = context.core_market.product();
    AffineBatchPlanV2::encode_into(
        AffineBatchPlanInputV2 {
            caller_role: ClaimsCallerRole::Trading,
            release_set: context.core_market.release_set().release_set_id.to_bytes(),
            market: context.core_market.market().to_bytes(),
            request_id: context.parent_request_digest,
            product_record_digest: product.product_record.to_bytes(),
            semantic_basis_id: product.liability_basis.to_bytes(),
            linked_basis_record_digest: context.linked_basis_record_digest,
            expected_market_revision: context.claims_market_revision,
            outcome_count: 3,
        },
        &positions,
        &rows,
        &mut bytes,
    )
    .expect("affine packet");
    bytes
}

#[test]
fn complementary_claims_batch_binds_every_runtime_row_and_receipt() {
    let context = claims_context(100);
    let (_roots, records, candidates) = complementary_candidates(1);
    let owners = core::array::from_fn(|index| records.get(index).expect("record").maker());
    let revisions = [30_u64, 31, 32];
    let packet =
        complementary_claims_packet(ComplementaryActionV2::Split, context, owners, revisions);
    let plan =
        validate_complementary_claims_plan_v2(ComplementaryActionV2::Split, context, &packet)
            .expect("Direct-bound affine plan");
    for index in 0..3_u32 {
        let slot = usize::try_from(index).expect("slot");
        validate_complementary_claims_item_v2(
            ComplementaryActionV2::Split,
            context,
            plan,
            index,
            DirectComplementaryClaimsParticipantV2 {
                record_before: *records.get(slot).expect("record"),
                candidate: *candidates.get(slot).expect("candidate"),
                expected_position_revision: *revisions.get(slot).expect("revision"),
            },
        )
        .expect("item binding");
    }

    let (positions, rows) = plan.table_bytes();
    let receipt = AffineBatchReceiptV2::new(
        plan,
        hash(&packet).to_bytes(),
        hashv(&[positions, rows]).to_bytes(),
        id(11),
        id(99),
        8,
    )
    .expect("receipt")
    .to_bytes();
    verify_direct_complementary_claims_receipt_v2(
        ComplementaryActionV2::Split,
        context,
        &packet,
        &receipt,
        id(99),
    )
    .expect("receipt binding");
    assert_eq!(
        verify_direct_complementary_claims_receipt_v2(
            ComplementaryActionV2::Split,
            context,
            &packet,
            &receipt,
            id(98),
        ),
        Err(DirectPhysicalError::Postcondition)
    );
}

#[test]
fn complementary_claims_substitution_and_wrong_action_refuse() {
    let context = claims_context(100);
    let (_roots, records, candidates) = complementary_candidates(1);
    let owners = core::array::from_fn(|index| records.get(index).expect("record").maker());
    let revisions = [30_u64, 31, 32];
    let packet =
        complementary_claims_packet(ComplementaryActionV2::Split, context, owners, revisions);
    assert_eq!(
        validate_complementary_claims_plan_v2(ComplementaryActionV2::Merge, context, &packet,),
        Err(DirectPhysicalError::Binding)
    );
    let plan =
        validate_complementary_claims_plan_v2(ComplementaryActionV2::Split, context, &packet)
            .expect("split plan");
    assert_eq!(
        validate_complementary_claims_item_v2(
            ComplementaryActionV2::Split,
            context,
            plan,
            1,
            DirectComplementaryClaimsParticipantV2 {
                record_before: records[1],
                candidate: candidates[1],
                expected_position_revision: 99,
            },
        ),
        Err(DirectPhysicalError::Binding)
    );

    let hostile_context = DirectComplementaryClaimsContextV2 {
        linked_basis_record_digest: id(77),
        ..context
    };
    assert_eq!(
        validate_complementary_claims_plan_v2(
            ComplementaryActionV2::Split,
            hostile_context,
            &packet,
        ),
        Err(DirectPhysicalError::Binding)
    );
}

fn derive_pda(program: [u8; 32], seeds: &[&[u8]]) -> ([u8; 32], u8) {
    let (address, bump) = Pubkey::find_program_address(seeds, &Pubkey::new_from_array(program));
    (address.to_bytes(), bump)
}

fn buy_escrow_fixture() -> (
    RegisteredIntentCreationV2,
    DirectBuyEscrowContextV2,
    DirectBuyEscrowAccountsV2,
    DirectExternalDebitV2,
) {
    let selected = config(1_000, id(6));
    let maker = id(3);
    let signed = intent(1, 0, id(21), 1_000);
    let authenticated = AuthenticatedCompactIntentV2::from_adjacent_ed25519(maker, signed)
        .expect("authenticated Buy");
    let record_seeds = RegisteredIntentSeedsV2::new(authenticated).expect("record seeds");
    let (record, record_bump) = derive_pda(id(10), &record_seeds.as_slices());
    let creation = register(
        DirectRootStateV1::new(),
        maker,
        signed,
        selected,
        record_bump,
    );
    let market = id(1);
    let release_set = id(13);
    let custody_program = id(20);
    let (custody_authority, _) = derive_pda(
        custody_program,
        &[CUSTODY_AUTHORITY_PDA_DOMAIN_V1, &market, &release_set],
    );
    let (replay, _) = derive_pda(
        custody_program,
        &[CUSTODY_REPLAY_PDA_DOMAIN_V1, &market, &release_set, &record],
    );
    let compartment = [CompartmentV1::TradingPrincipal.tag()];
    let (vault, _) = derive_pda(
        custody_program,
        &[
            CUSTODY_VAULT_PDA_DOMAIN_V1,
            &market,
            &release_set,
            &record,
            &compartment,
        ],
    );
    let accounts = DirectBuyEscrowAccountsV2 {
        record,
        replay,
        vault,
        custody_authority,
    };
    let source = DirectExternalDebitV2 {
        account: signed.collateral_account,
        owner: maker,
        delegate: custody_authority,
        delegated_amount: creation.record.reserved_collateral(),
        balance: 100,
    };
    (
        creation,
        DirectBuyEscrowContextV2 {
            core_market: core_market_view(3),
            trading_program: id(10),
            parent_request_digest: id(17),
        },
        accounts,
        source,
    )
}

fn live_buy_replay(
    creation: RegisteredIntentCreationV2,
    context: DirectBuyEscrowContextV2,
    accounts: DirectBuyEscrowAccountsV2,
) -> CustodyReplayV1 {
    CustodyReplayV1 {
        caller_role: dclutch_custody_contract::CallerRoleV1::Trading,
        release_set: context.core_market.release_set().release_set_id.to_bytes(),
        market: context.core_market.market().to_bytes(),
        realm: context.core_market.realm().realm_id.to_bytes(),
        context: accounts.record,
        caller_program: context.trading_program,
        rent_refund: creation.record.rent_owner(),
        open_vault_count: 1,
        next_revision: 3,
        generation: creation.record.intent().generation,
        last_request_digest: id(70),
        last_poststate_commitment: id(71),
    }
}

#[test]
fn registered_buy_deposits_exact_reserve_into_record_keyed_custody() {
    let (creation, context, accounts, source) = buy_escrow_fixture();
    let plan = prepare_buy_escrow_registration_v2(DirectBuyEscrowRegistrationInputV2 {
        creation,
        accounts,
        source,
        funding: DirectBuyEscrowCreationFundingV2 {
            payer: id(80),
            replay_rent_lamports: 20,
            vault_rent_lamports: 30,
        },
        context,
    })
    .expect("funded Buy registration");
    assert_eq!(plan.requests[0].operation, OperationV1::InitializeReplay);
    assert_eq!(plan.requests[1].operation, OperationV1::OpenVault);
    assert_eq!(plan.requests[2].operation, OperationV1::Transfer);
    assert_eq!(plan.requests[0].context, accounts.record);
    assert_eq!(plan.requests[1].destination, accounts.vault);
    assert_eq!(plan.requests[1].destination_vault_context, accounts.record);
    assert_eq!(plan.requests[2].source, source.account);
    assert_eq!(plan.requests[2].destination, accounts.vault);
    assert_eq!(
        plan.requests[2].amount,
        creation.record.reserved_collateral()
    );
    assert_eq!(plan.delegated_after, 0);
    assert_eq!(plan.vault_after, creation.record.reserved_collateral());

    let hostile_accounts = DirectBuyEscrowAccountsV2 {
        vault: id(99),
        ..accounts
    };
    assert_eq!(
        prepare_buy_escrow_registration_v2(DirectBuyEscrowRegistrationInputV2 {
            accounts: hostile_accounts,
            creation,
            source,
            funding: DirectBuyEscrowCreationFundingV2 {
                payer: id(80),
                replay_rent_lamports: 20,
                vault_rent_lamports: 30,
            },
            context,
        }),
        Err(DirectPhysicalError::Binding)
    );
}

#[test]
fn buy_cancel_refunds_then_closes_vault_and_replay() {
    let (creation, context, accounts, _source) = buy_escrow_fixture();
    let terminal = terminate_registered_intent_v2(
        creation.root,
        creation.maker_root,
        creation.record,
        config(1_000, id(6)),
        3,
        RegisteredTerminalEvidenceV2::Cancel(
            AuthenticatedCompactIntentV2::from_adjacent_ed25519(
                creation.record.maker(),
                creation.record.intent(),
            )
            .expect("cancel signature"),
        ),
        100,
    )
    .expect("cancel");
    let plan = prepare_buy_escrow_unwind_v2(
        DirectBuyEscrowTerminalObservationV2 {
            record_before: creation.record,
            accounts,
            replay: live_buy_replay(creation, context, accounts),
            vault_balance: creation.record.reserved_collateral(),
            refund_destination: DirectExternalCollateralV2 {
                account: creation.record.intent().collateral_account,
                owner: creation.record.maker(),
                balance: 10,
            },
            vault_rent_lamports: 30,
            replay_rent_lamports: 20,
            context,
        },
        terminal,
    )
    .expect("terminal escrow plan");
    assert_eq!(plan.request_count, 3);
    let refund = plan.requests[0].expect("refund");
    assert_eq!(refund.operation, OperationV1::Transfer);
    assert_eq!(refund.source, accounts.vault);
    assert_eq!(refund.amount, creation.record.reserved_collateral());
    assert_eq!(
        plan.requests[1].expect("close Vault").operation,
        OperationV1::CloseVault
    );
    assert_eq!(
        plan.requests[2].expect("close replay").operation,
        OperationV1::CloseReplay
    );
    assert_eq!(plan.refund_destination_after, 76);

    let expired = terminate_registered_intent_v2(
        creation.root,
        creation.maker_root,
        creation.record,
        config(1_000, id(6)),
        3,
        RegisteredTerminalEvidenceV2::Expire { slot: 21 },
        100,
    )
    .expect("strictly post-interval expiry");
    let expired_plan = prepare_buy_escrow_unwind_v2(
        DirectBuyEscrowTerminalObservationV2 {
            record_before: creation.record,
            accounts,
            replay: live_buy_replay(creation, context, accounts),
            vault_balance: creation.record.reserved_collateral(),
            refund_destination: DirectExternalCollateralV2 {
                account: creation.record.intent().collateral_account,
                owner: creation.record.maker(),
                balance: 10,
            },
            vault_rent_lamports: 30,
            replay_rent_lamports: 20,
            context,
        },
        expired,
    )
    .expect("expiry escrow plan");
    assert_eq!(expired_plan.requests, plan.requests);
}

#[test]
fn full_buy_fill_with_zero_residual_closes_without_refund_transfer() {
    let (creation, context, accounts, _source) = buy_escrow_fixture();
    let candidate = preview_registered_fill_v2(RegisteredFillInputV2 {
        root: creation.root,
        participant: RegisteredParticipantV2 {
            maker_root: creation.maker_root,
            record: creation.record,
            observed_record_lamports: 100,
        },
        execution: RegisteredExecutionV2 {
            config: config(1_000, id(6)),
            outcome_count: 3,
            slot: 5,
            fill: 100,
            execution_price: 60,
        },
    })
    .expect("full Buy fill");
    match candidate.record {
        RegisteredRecordAfterFillV2::Closed(close) => {
            assert_eq!(close.collateral_refund, 0);
            assert_eq!(close.claim_refund, 0);
        }
        RegisteredRecordAfterFillV2::Live(_) => panic!("full fill must close"),
    }
    let plan = prepare_buy_escrow_full_fill_v2(
        DirectBuyEscrowTerminalObservationV2 {
            record_before: creation.record,
            accounts,
            replay: live_buy_replay(creation, context, accounts),
            vault_balance: 0,
            refund_destination: DirectExternalCollateralV2 {
                account: creation.record.intent().collateral_account,
                owner: creation.record.maker(),
                balance: 10,
            },
            vault_rent_lamports: 30,
            replay_rent_lamports: 20,
            context,
        },
        candidate,
    )
    .expect("full-fill close");
    assert_eq!(plan.request_count, 2);
    assert_eq!(
        plan.requests[0].expect("close Vault").operation,
        OperationV1::CloseVault
    );
    assert_eq!(
        plan.requests[1].expect("close replay").operation,
        OperationV1::CloseReplay
    );
    assert_eq!(plan.requests[2], None);

    assert_eq!(plan.refund_destination_after, 10);
}

fn ordinary_buy_escrow_input(fill: u64, execution_price: u64) -> DirectBuyEscrowFillInputV2 {
    let (buyer, context, accounts, source) = buy_escrow_fixture();
    let seller = register(
        buyer.root,
        id(2),
        intent(0, 0, id(20), 1_000),
        config(1_000, id(6)),
        7,
    );
    DirectBuyEscrowFillInputV2 {
        direct: RegisteredOrdinaryInputV2 {
            root: seller.root,
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
                config: config(1_000, id(6)),
                outcome_count: 3,
                slot: 5,
                fill,
                execution_price,
            },
        },
        accounts,
        replay: live_buy_replay(buyer, context, accounts),
        vault_balance: buyer.record.reserved_collateral(),
        seller_destination: DirectExternalCollateralV2 {
            account: seller.record.intent().collateral_account,
            owner: seller.record.maker(),
            balance: 30,
        },
        fee_destination: DirectExternalCollateralV2 {
            account: id(22),
            owner: id(6),
            balance: 40,
        },
        buyer_refund_destination: DirectExternalCollateralV2 {
            account: source.account,
            owner: source.owner,
            balance: source
                .balance
                .checked_sub(buyer.record.reserved_collateral())
                .expect("post-registration source"),
        },
        vault_rent_lamports: 30,
        replay_rent_lamports: 20,
        context,
    }
}

#[test]
fn partial_buy_fill_spends_record_vault_and_keeps_lifecycle_live() {
    let input = ordinary_buy_escrow_input(20, 50);
    let plan = prepare_buy_escrow_fill_v2(input).expect("partial escrow fill");
    assert_eq!(plan.request_count, 2);
    assert!(!plan.closes_escrow);
    assert_eq!(plan.vault_after, 55);
    assert_eq!(plan.seller_destination_after, 39);
    assert_eq!(plan.fee_destination_after, 42);
    assert_eq!(plan.buyer_refund_destination_after, 34);
    let net = plan.requests[0].expect("seller net");
    assert_eq!(net.operation, OperationV1::Transfer);
    assert_eq!(net.source_compartment, CompartmentV1::TradingPrincipal);
    assert_eq!(net.source, input.accounts.vault);
    assert_eq!(net.amount, 9);
    assert_eq!(net.expected_revision, 3);
    let fee = plan.requests[1].expect("combined fee");
    assert_eq!(fee.amount, 2);
    assert_eq!(fee.expected_revision, 4);
    assert_eq!(plan.requests[2], None);

    let mut high_revision = input;
    high_revision.replay.next_revision = 70_000;
    let high_revision_plan =
        prepare_buy_escrow_fill_v2(high_revision).expect("u64 Custody replay revision");
    let high_net = high_revision_plan.requests[0].expect("high-revision net");
    assert_eq!(high_net.expected_revision, 70_000);
    assert_eq!(high_net.semantic.transfer_index, 0);

    let underfunded = DirectBuyEscrowFillInputV2 {
        vault_balance: input.vault_balance - 1,
        ..input
    };
    assert_eq!(
        prepare_buy_escrow_fill_v2(underfunded),
        Err(DirectPhysicalError::Binding)
    );
}

#[test]
fn terminal_price_improved_buy_fill_refunds_and_closes_after_transfers() {
    let input = ordinary_buy_escrow_input(100, 50);
    let plan = prepare_buy_escrow_fill_v2(input).expect("terminal escrow fill");
    assert_eq!(plan.request_count, 5);
    assert!(plan.closes_escrow);
    assert_eq!(plan.vault_after, 0);
    assert_eq!(plan.seller_destination_after, 75);
    assert_eq!(plan.fee_destination_after, 50);
    assert_eq!(plan.buyer_refund_destination_after, 45);
    assert_eq!(plan.requests[0].expect("net").amount, 45);
    assert_eq!(plan.requests[1].expect("fees").amount, 10);
    assert_eq!(plan.requests[2].expect("residual").amount, 11);
    assert_eq!(
        plan.requests[3].expect("close Vault").operation,
        OperationV1::CloseVault
    );
    assert_eq!(
        plan.requests[4].expect("close replay").operation,
        OperationV1::CloseReplay
    );
}
