use super::{
    buy_escrow::*, complementary::*, inline::*, lifecycle::*, physical::*, sell_escrow::*,
};
use dclutch_account_profile_contract::lifecycle_v3::{
    AuthenticateStatePlanV3, CloseStatePlanV3, CreateStatePlanV3, StateLifecyclePlanV3,
};
use dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1;
use dclutch_claims_svm::{
    CallerRole as ClaimsCallerRole,
    affine_batch_v2::{
        AffineBatchPlanInputV2, AffineBatchPlanV2, AffineBatchPositionV2, AffineBatchReceiptV2,
        AffineBatchRowInputV2, AffineBatchRowV2, DeltaDirectionV2, SignedMagnitudeV2,
    },
    protocol_position_v2::{
        ProtocolPositionAdmissionEvidenceV2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionAdmissionV2, ProtocolPositionCloseEvidenceV2,
        ProtocolPositionCloseReceiptV2, ProtocolPositionOwnerKindV2, ProtocolPositionSeedsV2,
    },
    sparse_native_transfer_v1::{SparseNativeTransferReceiptV1, SparseNativeTransferV1},
};
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_VAULT_PDA_DOMAIN_V1, CallerRoleV1, CompartmentV1,
    CustodyReceiptV1, CustodyReplaySeedsV1, CustodyReplayV1, DelegatedCustodyReceiptV2,
    OperationV1, ReceiptEvidenceV1,
};
use dclutch_direct_codec::{
    intent_v2::CompactIntentV2,
    successor::{
        AuthenticatedCompactIntentV2, ComplementaryActionV2, ComplementaryInputV2,
        ComplementaryParticipantsV2, ComplementarySettlementV2, DIRECT_MAKER_REPLAY_BYTES_V1,
        DIRECT_REGISTERED_RECORD_BYTES_V2, DIRECT_ROOT_STATE_BYTES_V1, DirectCoordinatesV1,
        DirectExecutionConfigV1, DirectRegisteredIntentV2, DirectRootStateV1, InlineExecutionV2,
        InlineOrdinaryInputV2, InlineParticipantV2, MakerReplayFirstUseV1,
        MakerReplayObservationV1, MakerReplayRootV1, MakerReplaySeedsV1, MakerReplayVacancyV1,
        NonceConsumptionV2, RegisteredExecutionV2, RegisteredFillCandidateV2,
        RegisteredFillInputV2, RegisteredIntentCreationV2, RegisteredIntentSeedsV2,
        RegisteredOrdinaryInputV2, RegisteredParticipantV2, RegisteredRecordAfterFillV2,
        RegisteredRecordFirstUseV2, RegisteredTerminalEvidenceV2, consume_nonce_v2,
        preview_registered_fill_v2, register_intent_v2, settle_inline_ordinary_v2,
        settle_registered_complementary_v2, terminate_registered_intent_v2,
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

fn register_canonical(
    root: DirectRootStateV1,
    maker: [u8; 32],
    signed: CompactIntentV2,
    selected: DirectExecutionConfigV1,
) -> RegisteredIntentCreationV2 {
    let authenticated = AuthenticatedCompactIntentV2::from_adjacent_ed25519(maker, signed)
        .expect("authenticated canonical registration");
    let record_seeds = RegisteredIntentSeedsV2::new(authenticated).expect("record seeds");
    let (_, record_bump) = derive_pda(id(10), &record_seeds.as_slices());
    let maker_seeds =
        MakerReplaySeedsV1::new(authenticated.replay().expect("replay").coordinates(), maker)
            .expect("maker seeds");
    let (_, maker_bump) = derive_pda(id(10), &maker_seeds.as_slices());
    register_intent_v2(
        root,
        MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(maker_bump, 3)),
        authenticated,
        selected,
        3,
        Some(MakerReplayFirstUseV1 {
            rent_owner: id(90),
            rent_principal: 100,
        }),
        RegisteredRecordFirstUseV2 {
            bump: record_bump,
            observed_lamports: 7,
            rent_owner: id(91),
            rent_principal: 100,
        },
    )
    .expect("canonical registration")
}

fn ordinary_fixture(fee_basis_points: u16) -> RegisteredOrdinaryInputV2 {
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
    }
}

#[test]
fn state_candidate_is_commit_last_for_partial_and_terminal_records() {
    let input = ordinary_fixture(1_000);
    let selected = input.execution.config;
    let width = input.execution.outcome_count;
    let partial = dclutch_direct_codec::successor::settle_registered_ordinary_v2(input)
        .expect("partial ordinary settlement");
    let mut seller_maker = [0xa5; DIRECT_MAKER_REPLAY_BYTES_V1];
    let mut buyer_maker = [0xa5; DIRECT_MAKER_REPLAY_BYTES_V1];
    let mut seller_scratch = [0; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let mut buyer_scratch = [0; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let mut seller_record = [0xa5; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let mut buyer_record = [0xa5; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let candidate = encode_registered_ordinary_state_candidate_v2(
        partial,
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
        Ok(partial.seller.maker_root)
    );
    assert_eq!(
        MakerReplayRootV1::decode(&buyer_maker),
        Ok(partial.buyer.maker_root)
    );
    assert_eq!(
        DirectRegisteredIntentV2::decode_selected(selected, width, &seller_record),
        match partial.seller.record {
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
    let terminal = dclutch_direct_codec::successor::settle_registered_ordinary_v2(terminal_input)
        .expect("terminal ordinary settlement");
    let mut terminal_seller_maker = [0xa5; DIRECT_MAKER_REPLAY_BYTES_V1];
    let mut terminal_buyer_maker = [0xa5; DIRECT_MAKER_REPLAY_BYTES_V1];
    let mut terminal_seller_scratch = [0; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let mut terminal_buyer_scratch = [0; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let mut terminal_seller_record = [0xa5; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let mut terminal_buyer_record = [0xa5; DIRECT_REGISTERED_RECORD_BYTES_V2];
    let closed_record_before = terminal_seller_record;
    let terminal_candidate = encode_registered_ordinary_state_candidate_v2(
        terminal,
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
            terminal,
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
    let (custody_authority, _) =
        derive_pda(id(20), &[CUSTODY_AUTHORITY_PDA_DOMAIN_V1, &id(1), &id(13)]);
    DirectComplementaryPhysicalContextV2 {
        trading_program: id(10),
        direct_root: id(84),
        core_market: core_market_view(3),
        custody_authority,
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
    let seed = register_canonical(
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
    );
    let mut roots = [seed.maker_root; 3];
    let mut records = [seed.record; 3];
    let mut root = DirectRootStateV1::new();
    for (index, price) in prices.iter().copied().enumerate() {
        let maker = id(u8::try_from(index + 2).expect("maker"));
        let outcome = u32::try_from(index).expect("outcome");
        let created = register_canonical(
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
    let record_seeds = RegisteredIntentSeedsV2::from_record(buy_record);
    let (record, bump) = derive_pda(id(10), &record_seeds.as_slices());
    assert_eq!(bump, buy_record.bump());
    let market = id(1);
    let release_set = id(13);
    let custody_program = id(20);
    let (custody_authority, _) = derive_pda(
        custody_program,
        &[CUSTODY_AUTHORITY_PDA_DOMAIN_V1, &market, &release_set],
    );
    let (replay, _) = derive_pda(
        custody_program,
        &CustodyReplaySeedsV1::new(market, release_set, CallerRoleV1::Trading, record).as_slices(),
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
    let escrow = DirectComplementaryBuyEscrowV2 {
        accounts,
        replay: CustodyReplayV1 {
            caller_role: dclutch_custody_contract::CallerRoleV1::Trading,
            release_set,
            market,
            realm: id(14),
            context: record,
            caller_program: id(10),
            rent_refund: buy_record.rent_owner(),
            open_vault_count: 1,
            next_revision: 10,
            generation: 4,
            last_request_digest: id(70),
            last_poststate_commitment: id(71),
        },
        vault_balance: buy_record.reserved_collateral(),
        refund_destination: DirectExternalCollateralV2 {
            account: buy_record.intent().collateral_account,
            owner: buy_record.maker(),
            balance: 50,
        },
        vault_rent_lamports: 30,
        replay_rent_lamports: 20,
    };
    let participant = DirectComplementaryParticipantV2 {
        maker_root: id(51),
        record,
        collateral: DirectComplementaryCollateralV2::BuyEscrow(&escrow),
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
    assert_eq!(principal.request.source, vault);
    assert_eq!(principal.request.destination, id(41));
    assert_eq!(principal.request.destination_vault_context, id(40));
    assert_ne!(principal.request.destination_vault_context, id(1));
    assert_eq!(principal.request.semantic.transfer_index, 0);
    assert_eq!(principal.request.expected_revision, 10);
    assert_eq!(principal.buy_vault_after, Some(3));
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
    assert_eq!(fee.buy_vault_after, Some(0));

    assert_eq!(
        project_complementary_custody_effect_v2(projection(
            DirectComplementaryCustodyRouteV2::Residual,
            participant,
        )),
        Ok(None)
    );
    let close_vault = project_complementary_custody_effect_v2(projection(
        DirectComplementaryCustodyRouteV2::CloseBuyVault,
        participant,
    ))
    .expect("close projection")
    .expect("terminal Vault close");
    assert_eq!(close_vault.request.operation, OperationV1::CloseVault);
    assert_eq!(close_vault.request.expected_revision, 12);
    let close_replay = project_complementary_custody_effect_v2(projection(
        DirectComplementaryCustodyRouteV2::CloseBuyReplay,
        participant,
    ))
    .expect("close projection")
    .expect("terminal replay close");
    assert_eq!(close_replay.request.operation, OperationV1::CloseReplay);
    assert_eq!(close_replay.request.expected_revision, 13);

    let hostile_escrow = DirectComplementaryBuyEscrowV2 {
        accounts: DirectBuyEscrowAccountsV2 {
            vault: id(99),
            ..accounts
        },
        ..escrow
    };
    let hostile = DirectComplementaryParticipantV2 {
        collateral: DirectComplementaryCollateralV2::BuyEscrow(&hostile_escrow),
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
        trading_program: id(10),
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
                record: derive_pda(
                    id(10),
                    &RegisteredIntentSeedsV2::from_record(*records.get(slot).expect("record"))
                        .as_slices(),
                )
                .0,
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
    let record = *records.get(1).expect("record");
    let candidate = *candidates.get(1).expect("candidate");
    assert_eq!(
        validate_complementary_claims_item_v2(
            ComplementaryActionV2::Split,
            context,
            plan,
            1,
            DirectComplementaryClaimsParticipantV2 {
                record_before: record,
                candidate,
                record: derive_pda(
                    id(10),
                    &RegisteredIntentSeedsV2::from_record(record).as_slices(),
                )
                .0,
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

#[test]
fn complementary_merge_claims_debits_record_positions_not_makers() {
    let context = claims_context(100);
    let (_roots, records, candidates) = complementary_candidates(0);
    let record_keys = core::array::from_fn(|index| {
        let record = *records.get(index).expect("record");
        derive_pda(
            context.trading_program,
            &RegisteredIntentSeedsV2::from_record(record).as_slices(),
        )
        .0
    });
    let revisions = [30_u64, 31, 32];
    let packet = complementary_claims_packet(
        ComplementaryActionV2::Merge,
        context,
        record_keys,
        revisions,
    );
    let plan =
        validate_complementary_claims_plan_v2(ComplementaryActionV2::Merge, context, &packet)
            .expect("merge plan");
    for outcome in 0..3_u32 {
        let slot = usize::try_from(outcome).expect("slot");
        validate_complementary_claims_item_v2(
            ComplementaryActionV2::Merge,
            context,
            plan,
            outcome,
            DirectComplementaryClaimsParticipantV2 {
                record_before: *records.get(slot).expect("record"),
                candidate: *candidates.get(slot).expect("candidate"),
                record: *record_keys.get(slot).expect("record key"),
                expected_position_revision: *revisions.get(slot).expect("revision"),
            },
        )
        .expect("record Position merge debit");
    }

    let maker_owners = core::array::from_fn(|index| records.get(index).expect("record").maker());
    let hostile = complementary_claims_packet(
        ComplementaryActionV2::Merge,
        context,
        maker_owners,
        revisions,
    );
    let hostile_plan =
        validate_complementary_claims_plan_v2(ComplementaryActionV2::Merge, context, &hostile)
            .expect("structurally valid hostile merge");
    assert_eq!(
        validate_complementary_claims_item_v2(
            ComplementaryActionV2::Merge,
            context,
            hostile_plan,
            0,
            DirectComplementaryClaimsParticipantV2 {
                record_before: *records.first().expect("record"),
                candidate: *candidates.first().expect("candidate"),
                record: *record_keys.first().expect("record key"),
                expected_position_revision: revisions[0],
            },
        ),
        Err(DirectPhysicalError::Binding)
    );
}

fn sell_escrow_fixture() -> (
    RegisteredIntentCreationV2,
    DirectSellPositionAccountsV2,
    DirectPositionFundingV2,
    DirectSellEscrowContextV2,
) {
    let creation = register_canonical(
        DirectRootStateV1::new(),
        id(2),
        intent(0, 0, id(20), 1_000),
        config(1_000, id(6)),
    );
    let record_seeds = RegisteredIntentSeedsV2::from_record(creation.record);
    let (record, bump) = derive_pda(id(10), &record_seeds.as_slices());
    assert_eq!(bump, creation.record.bump());
    let aggregate = core_market_view(3).claims_aggregate().to_bytes();
    let position_seeds = ProtocolPositionSeedsV2::new(aggregate, record).expect("Position seeds");
    let admission_seeds =
        ProtocolPositionAdmissionSeedsV2::new(aggregate, record).expect("admission seeds");
    let (position, _) = derive_pda(id(11), &position_seeds.as_slices());
    let (admission, _) = derive_pda(id(11), &admission_seeds.as_slices());
    (
        creation,
        DirectSellPositionAccountsV2 {
            record,
            position,
            admission,
        },
        DirectPositionFundingV2 {
            position_lamports: 105,
            admission_lamports: 57,
            position_rent_principal: 100,
            admission_rent_principal: 50,
        },
        DirectSellEscrowContextV2 {
            core_market: core_market_view(3),
            direct_root: id(84),
            trading_program: id(10),
            claims_program: id(11),
            rent_program: id(81),
            parent_request_digest: id(17),
            linked_basis_record_digest: id(39),
            claims_market_revision: 7,
        },
    )
}

fn sell_affine_packet(
    context: DirectSellEscrowContextV2,
    source: [u8; 32],
    source_revision: u64,
    destination: [u8; 32],
    destination_revision: u64,
    outcome: u32,
    quantity: u64,
) -> [u8; 384] {
    let positions = [
        AffineBatchPositionV2::new(source, source_revision).expect("source Position"),
        AffineBatchPositionV2::new(destination, destination_revision)
            .expect("destination Position"),
    ];
    let rows = [AffineBatchRowV2::new(
        AffineBatchRowInputV2 {
            source_present: true,
            destination_present: true,
            outcome,
            source_position_index: 0,
            destination_position_index: 1,
            aggregate_delta: signed(DeltaDirectionV2::Neutral, 0),
            source_delta: signed(DeltaDirectionV2::Debit, quantity),
            destination_delta: signed(DeltaDirectionV2::Credit, quantity),
        },
        context.core_market.product().outcome_count,
        2,
    )
    .expect("affine row")];
    let mut output = [0; 384];
    AffineBatchPlanV2::encode_into(
        AffineBatchPlanInputV2 {
            caller_role: ClaimsCallerRole::Trading,
            release_set: context.core_market.release_set().release_set_id.to_bytes(),
            market: context.core_market.market().to_bytes(),
            request_id: context.parent_request_digest,
            product_record_digest: context.core_market.product().product_record.to_bytes(),
            semantic_basis_id: context.core_market.product().liability_basis.to_bytes(),
            linked_basis_record_digest: context.linked_basis_record_digest,
            expected_market_revision: context.claims_market_revision,
            outcome_count: context.core_market.product().outcome_count,
        },
        &positions,
        &rows,
        &mut output,
    )
    .expect("affine packet");
    output
}

fn sell_admission(
    request: dclutch_claims_svm::protocol_position_v2::ProtocolPositionRequestV2,
    context: DirectSellEscrowContextV2,
) -> ProtocolPositionAdmissionV2 {
    let request_bytes = request.to_bytes().expect("request bytes");
    ProtocolPositionAdmissionV2::new(
        request,
        ProtocolPositionAdmissionEvidenceV2 {
            product_record_digest: context.core_market.product().product_record.to_bytes(),
            semantic_basis_id: context.core_market.product().liability_basis.to_bytes(),
            linked_basis_record_digest: context.linked_basis_record_digest,
            request_digest: hash(&request_bytes).to_bytes(),
            claims_program: context.claims_program,
            trading_program: context.trading_program,
            capability_descriptor: [0; 32],
            capability_outcome: 0,
            outcome_count: context.core_market.product().outcome_count,
        },
    )
    .expect("admission")
}

fn registered_creation_lifecycle(
    creation: RegisteredIntentCreationV2,
    trading_program: [u8; 32],
    record: [u8; 32],
) -> DirectRegisteredCreationLifecycleV3 {
    let coordinates = DirectCoordinatesV1::new(
        creation.record.intent().market,
        creation.record.intent().generation,
    )
    .expect("coordinates");
    let maker_seeds =
        MakerReplaySeedsV1::new(coordinates, creation.record.maker()).expect("maker seeds");
    let maker = derive_pda(trading_program, &maker_seeds.as_slices()).0;
    let maker_plan = match creation.maker_creation {
        None => StateLifecyclePlanV3::Authenticate(AuthenticateStatePlanV3 {
            state: maker,
            data_bytes: u32::try_from(DIRECT_MAKER_REPLAY_BYTES_V1).expect("maker bytes"),
            lamports: creation.maker_root.rent_principal(),
            bump: creation.maker_root.bump(),
        }),
        Some(candidate) => state_create_plan(
            maker,
            DIRECT_MAKER_REPLAY_BYTES_V1,
            creation.maker_root.rent_owner(),
            creation.maker_root.rent_principal(),
            creation.maker_root.bump(),
            candidate,
            80,
        ),
    };
    DirectRegisteredCreationLifecycleV3 {
        root: StateLifecyclePlanV3::Authenticate(AuthenticateStatePlanV3 {
            state: id(84),
            data_bytes: u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1)
                .expect("root bytes"),
            lamports: 100,
            bump: 1,
        }),
        maker: maker_plan,
        record: state_create_plan(
            record,
            DIRECT_REGISTERED_RECORD_BYTES_V2,
            creation.record.rent_owner(),
            creation.record.rent_principal(),
            creation.record.bump(),
            creation.record_creation,
            82,
        ),
    }
}

fn state_create_plan(
    state: [u8; 32],
    data_bytes: usize,
    beneficiary: [u8; 32],
    principal: u64,
    bump: u8,
    creation: dclutch_direct_codec::successor::MakerReplayCreationPlanV1,
    payer_byte: u8,
) -> StateLifecyclePlanV3 {
    StateLifecyclePlanV3::Create(CreateStatePlanV3 {
        state,
        payer: id(payer_byte),
        rent_credit: id(payer_byte + 1),
        beneficiary,
        target_data_bytes: u32::try_from(data_bytes).expect("state bytes"),
        historical_rent_principal: principal,
        state_before: creation.observed_lamports,
        state_after: creation.post_lamports,
        payer_debit: creation.top_up_lamports,
        payer_after: 1_000 - creation.top_up_lamports,
        bump,
    })
}

fn record_close_lifecycle(
    record: DirectRegisteredIntentV2,
    close: dclutch_direct_codec::successor::RegisteredRecordCloseV2,
    trading_program: [u8; 32],
) -> StateLifecyclePlanV3 {
    let seeds = RegisteredIntentSeedsV2::from_record(record);
    let (state, bump) = derive_pda(trading_program, &seeds.as_slices());
    StateLifecyclePlanV3::Close(CloseStatePlanV3 {
        state,
        rent_credit: id(85),
        beneficiary: close.rent_owner,
        source_data_bytes: u32::try_from(DIRECT_REGISTERED_RECORD_BYTES_V2).expect("record bytes"),
        historical_rent_principal: close.rent_principal,
        source_before: close.total_rent_credit,
        source_after: 0,
        rent_credit_before: 1_000,
        rent_credit_after: 1_000 + close.total_rent_credit,
        bump,
    })
}

#[test]
fn sell_registration_admits_record_position_and_reserves_exact_claims() {
    let (creation, accounts, funding, context) = sell_escrow_fixture();
    let lifecycle =
        registered_creation_lifecycle(creation, context.trading_program, accounts.record);
    let plan = prepare_sell_registration_v2(creation, accounts, funding, context, lifecycle)
        .expect("Sell registration");
    assert_eq!(plan.admission.position_owner, accounts.record);
    assert_eq!(plan.admission.rent_credit, creation.record.rent_owner());
    assert_eq!(plan.reserved_claims, 100);
    let receipt = sell_admission(plan.admission, context)
        .to_receipt_bytes()
        .expect("admission receipt");
    verify_sell_admission_receipt_v2(plan.admission, context, &receipt)
        .expect("exact admission receipt");

    let packet = sell_affine_packet(
        context,
        creation.record.maker(),
        4,
        accounts.record,
        0,
        creation.record.intent().outcome,
        plan.reserved_claims,
    );
    let expectation = DirectSellAffineExpectationV2 {
        action: DirectSellAffineActionV2::Register,
        record_before: creation.record,
        record: accounts.record,
        user: creation.record.maker(),
        record_position_revision: 0,
        user_position_revision: 4,
        quantity: plan.reserved_claims,
    };
    validate_sell_affine_plan_v2(expectation, context, &packet).expect("maker-to-record reserve");

    let hostile_accounts = DirectSellPositionAccountsV2 {
        position: id(99),
        ..accounts
    };
    assert_eq!(
        prepare_sell_registration_v2(creation, hostile_accounts, funding, context, lifecycle),
        Err(DirectPhysicalError::Binding)
    );

    let mut hostile_lifecycle = lifecycle;
    let StateLifecyclePlanV3::Create(record_create) = hostile_lifecycle.record else {
        panic!("record create")
    };
    hostile_lifecycle.record = StateLifecyclePlanV3::Create(CreateStatePlanV3 {
        beneficiary: id(99),
        ..record_create
    });
    assert_eq!(
        prepare_sell_registration_v2(creation, accounts, funding, context, hostile_lifecycle,),
        Err(DirectPhysicalError::State)
    );
}

#[test]
fn sell_partial_fill_releases_only_from_record_position() {
    let (creation, accounts, funding, context) = sell_escrow_fixture();
    let buyer = id(77);
    let aggregate = context.core_market.claims_aggregate().to_bytes();
    let buyer_position = ProtocolPositionSeedsV2::new(aggregate, buyer).expect("buyer Position");
    let buyer_admission =
        ProtocolPositionAdmissionSeedsV2::new(aggregate, buyer).expect("buyer admission");
    let buyer_accounts = DirectUserPositionAccountsV2 {
        owner: buyer,
        position: derive_pda(context.claims_program, &buyer_position.as_slices()).0,
        admission: derive_pda(context.claims_program, &buyer_admission.as_slices()).0,
    };
    let admission = prepare_sell_user_admission_v2(buyer_accounts, id(82), funding, context)
        .expect("vacant buyer admission");
    assert_eq!(admission.owner_kind, ProtocolPositionOwnerKindV2::User);
    assert_eq!(admission.position_owner, buyer);
    assert_eq!(admission.rent_credit, id(82));
    assert_eq!(
        prepare_sell_user_admission_v2(
            DirectUserPositionAccountsV2 {
                position: id(99),
                ..buyer_accounts
            },
            id(82),
            funding,
            context,
        ),
        Err(DirectPhysicalError::Binding)
    );
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
            fill: 20,
            execution_price: 50,
        },
    })
    .expect("partial Sell fill");
    let expectation =
        sell_fill_expectation_v2(creation.record, accounts.record, candidate, buyer, 1, 3)
            .expect("Sell affine expectation");
    let packet = sell_affine_packet(
        context,
        accounts.record,
        1,
        buyer,
        3,
        creation.record.intent().outcome,
        20,
    );
    let plan =
        validate_sell_affine_plan_v2(expectation, context, &packet).expect("record-to-buyer fill");
    let (positions, rows) = plan.table_bytes();
    let receipt = AffineBatchReceiptV2::new(
        plan,
        hash(&packet).to_bytes(),
        hashv(&[positions, rows]).to_bytes(),
        context.claims_program,
        id(93),
        8,
    )
    .expect("affine receipt")
    .to_bytes();
    verify_sell_affine_receipt_v2(expectation, context, &packet, &receipt, id(93))
        .expect("fill receipt");

    let hostile = sell_affine_packet(
        context,
        creation.record.maker(),
        1,
        buyer,
        3,
        creation.record.intent().outcome,
        20,
    );
    assert_eq!(
        validate_sell_affine_plan_v2(expectation, context, &hostile),
        Err(DirectPhysicalError::Binding)
    );
}

#[test]
fn sell_unwind_refunds_residual_then_closes_to_persisted_rent_credit() {
    let (creation, accounts, funding, context) = sell_escrow_fixture();
    let registration = prepare_sell_registration_v2(
        creation,
        accounts,
        funding,
        context,
        registered_creation_lifecycle(creation, context.trading_program, accounts.record),
    )
    .expect("Sell registration");
    let admission = sell_admission(registration.admission, context);
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
    assert_eq!(terminal.close.claim_refund, 100);
    let refund_expectation = DirectSellAffineExpectationV2 {
        action: DirectSellAffineActionV2::Unwind,
        record_before: creation.record,
        record: accounts.record,
        user: creation.record.maker(),
        record_position_revision: 1,
        user_position_revision: 5,
        quantity: terminal.close.claim_refund,
    };
    let refund = sell_affine_packet(
        context,
        accounts.record,
        1,
        creation.record.maker(),
        5,
        creation.record.intent().outcome,
        100,
    );
    validate_sell_affine_plan_v2(refund_expectation, context, &refund)
        .expect("record-to-maker refund");

    let close_lifecycle =
        record_close_lifecycle(creation.record, terminal.close, context.trading_program);
    let close = prepare_sell_close_v2(DirectSellCloseInputV2 {
        record_before: creation.record,
        close: terminal.close,
        accounts,
        admission,
        post_affine_position_revision: 2,
        current_funding: funding,
        context,
        lifecycle: close_lifecycle,
    })
    .expect("zero Position close");
    let StateLifecyclePlanV3::Close(close_plan) = close_lifecycle else {
        panic!("record close")
    };
    let hostile_lifecycle = StateLifecyclePlanV3::Close(CloseStatePlanV3 {
        beneficiary: id(99),
        ..close_plan
    });
    assert_eq!(
        prepare_sell_close_v2(DirectSellCloseInputV2 {
            record_before: creation.record,
            close: terminal.close,
            accounts,
            admission,
            post_affine_position_revision: 2,
            current_funding: funding,
            context,
            lifecycle: hostile_lifecycle,
        }),
        Err(DirectPhysicalError::State)
    );
    let admission_state = admission.to_state_bytes().expect("admission state");
    let close_bytes = close.to_bytes().expect("close bytes");
    let receipt = ProtocolPositionCloseReceiptV2::new(
        close,
        ProtocolPositionCloseEvidenceV2 {
            request_digest: hash(&close_bytes).to_bytes(),
            admission_digest: hash(&admission_state).to_bytes(),
            claims_program: context.claims_program,
            post_resource_digest: id(94),
            rent_credit_before: 1_000,
            rent_credit_after: 1_162,
        },
    )
    .expect("close receipt")
    .to_bytes()
    .expect("close receipt bytes");
    verify_sell_close_receipt_v2(close, context, 1_000, id(94), &admission_state, &receipt)
        .expect("close receipt verification");

    let mut hostile_receipt = receipt;
    *hostile_receipt.get_mut(304).expect("resource digest") ^= 1;
    assert_eq!(
        verify_sell_close_receipt_v2(
            close,
            context,
            1_000,
            id(94),
            &admission_state,
            &hostile_receipt,
        ),
        Err(DirectPhysicalError::Postcondition)
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
    let creation = register_canonical(DirectRootStateV1::new(), maker, signed, selected);
    assert_eq!(record_bump, creation.record.bump());
    let market = id(1);
    let release_set = id(13);
    let custody_program = id(20);
    let (custody_authority, _) = derive_pda(
        custody_program,
        &[CUSTODY_AUTHORITY_PDA_DOMAIN_V1, &market, &release_set],
    );
    let (replay, _) = derive_pda(
        custody_program,
        &CustodyReplaySeedsV1::new(market, release_set, CallerRoleV1::Trading, record).as_slices(),
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
            direct_root: id(84),
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
    let lifecycle =
        registered_creation_lifecycle(creation, context.trading_program, accounts.record);
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
        lifecycle,
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
            lifecycle,
        }),
        Err(DirectPhysicalError::Binding)
    );

    let mut hostile_lifecycle = lifecycle;
    let StateLifecyclePlanV3::Create(record_create) = hostile_lifecycle.record else {
        panic!("record create")
    };
    hostile_lifecycle.record = StateLifecyclePlanV3::Create(CreateStatePlanV3 {
        historical_rent_principal: record_create.historical_rent_principal + 1,
        ..record_create
    });
    assert_eq!(
        prepare_buy_escrow_registration_v2(DirectBuyEscrowRegistrationInputV2 {
            creation,
            accounts,
            source,
            funding: DirectBuyEscrowCreationFundingV2 {
                payer: id(80),
                replay_rent_lamports: 20,
                vault_rent_lamports: 30,
            },
            context,
            lifecycle: hostile_lifecycle,
        }),
        Err(DirectPhysicalError::State)
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
        &DirectBuyEscrowTerminalObservationV2 {
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
            record_lifecycle: record_close_lifecycle(
                creation.record,
                terminal.close,
                context.trading_program,
            ),
            context,
        },
        terminal,
    )
    .expect("terminal escrow plan");
    assert_eq!(plan.request_count, 3);
    let refund = plan.requests[0];
    assert_eq!(refund.operation, OperationV1::Transfer);
    assert_eq!(refund.source, accounts.vault);
    assert_eq!(refund.amount, creation.record.reserved_collateral());
    assert_eq!(plan.requests[1].operation, OperationV1::CloseVault);
    assert_eq!(plan.requests[2].operation, OperationV1::CloseReplay);
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
        &DirectBuyEscrowTerminalObservationV2 {
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
            record_lifecycle: record_close_lifecycle(
                creation.record,
                expired.close,
                context.trading_program,
            ),
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
    let close = match candidate.record {
        RegisteredRecordAfterFillV2::Closed(close) => {
            assert_eq!(close.collateral_refund, 0);
            assert_eq!(close.claim_refund, 0);
            close
        }
        RegisteredRecordAfterFillV2::Live(_) => panic!("full fill must close"),
    };
    let plan = prepare_buy_escrow_full_fill_v2(
        &DirectBuyEscrowTerminalObservationV2 {
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
            record_lifecycle: record_close_lifecycle(
                creation.record,
                close,
                context.trading_program,
            ),
            context,
        },
        candidate,
    )
    .expect("full-fill close");
    assert_eq!(plan.request_count, 2);
    assert_eq!(plan.requests[0].operation, OperationV1::CloseVault);
    assert_eq!(plan.requests[1].operation, OperationV1::CloseReplay);
    assert_eq!(plan.requests.len(), 2);

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
    let direct = RegisteredOrdinaryInputV2 {
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
    };
    let settlement = dclutch_direct_codec::successor::settle_registered_ordinary_v2(direct)
        .expect("ordinary settlement");
    let record_lifecycle = match settlement.buyer.record {
        RegisteredRecordAfterFillV2::Live(_) => None,
        RegisteredRecordAfterFillV2::Closed(close) => Some(record_close_lifecycle(
            buyer.record,
            close,
            context.trading_program,
        )),
    };
    DirectBuyEscrowFillInputV2 {
        direct,
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
        record_lifecycle,
        context,
    }
}

#[test]
fn partial_buy_fill_spends_record_vault_and_keeps_lifecycle_live() {
    let input = ordinary_buy_escrow_input(20, 50);
    assert_eq!(input.record_lifecycle, None);
    let plan = prepare_buy_escrow_fill_v2(&input).expect("partial escrow fill");
    assert_eq!(plan.request_count, 2);
    assert!(!plan.closes_escrow);
    assert_eq!(plan.vault_after, 55);
    assert_eq!(plan.seller_destination_after, 39);
    assert_eq!(plan.fee_destination_after, 42);
    assert_eq!(plan.buyer_refund_destination_after, 34);
    let net = plan.requests[0];
    assert_eq!(net.operation, OperationV1::Transfer);
    assert_eq!(net.source_compartment, CompartmentV1::TradingPrincipal);
    assert_eq!(net.source, input.accounts.vault);
    assert_eq!(net.amount, 9);
    assert_eq!(net.expected_revision, 3);
    let fee = plan.requests[1];
    assert_eq!(fee.amount, 2);
    assert_eq!(fee.expected_revision, 4);
    assert_eq!(plan.requests.len(), 2);

    let mut high_revision = input;
    high_revision.replay.next_revision = 70_000;
    let high_revision_plan =
        prepare_buy_escrow_fill_v2(&high_revision).expect("u64 Custody replay revision");
    let high_net = high_revision_plan.requests[0];
    assert_eq!(high_net.expected_revision, 70_000);
    assert_eq!(high_net.semantic.transfer_index, 0);

    let underfunded = DirectBuyEscrowFillInputV2 {
        vault_balance: input.vault_balance - 1,
        ..input
    };
    assert_eq!(
        prepare_buy_escrow_fill_v2(&underfunded),
        Err(DirectPhysicalError::Binding)
    );
}

#[test]
fn terminal_price_improved_buy_fill_refunds_and_closes_after_transfers() {
    let input = ordinary_buy_escrow_input(100, 50);
    assert!(matches!(
        input.record_lifecycle,
        Some(StateLifecyclePlanV3::Close(_))
    ));
    let mut missing_close = input;
    missing_close.record_lifecycle = None;
    assert_eq!(
        prepare_buy_escrow_fill_v2(&missing_close),
        Err(DirectPhysicalError::State)
    );
    let plan = prepare_buy_escrow_fill_v2(&input).expect("terminal escrow fill");
    assert_eq!(plan.request_count, 5);
    assert!(plan.closes_escrow);
    assert_eq!(plan.vault_after, 0);
    assert_eq!(plan.seller_destination_after, 75);
    assert_eq!(plan.fee_destination_after, 50);
    assert_eq!(plan.buyer_refund_destination_after, 45);
    assert_eq!(plan.requests[0].amount, 45);
    assert_eq!(plan.requests[1].amount, 10);
    assert_eq!(plan.requests[2].amount, 11);
    assert_eq!(plan.requests[3].operation, OperationV1::CloseVault);
    assert_eq!(plan.requests[4].operation, OperationV1::CloseReplay);
}

fn inline_compact(
    side: u8,
    lifecycle: u8,
    nonce: u64,
    collateral_account: [u8; 32],
) -> CompactIntentV2 {
    CompactIntentV2 {
        side,
        lifecycle,
        outcome: 1,
        market: id(1),
        generation: 4,
        nonce,
        valid_from: 2,
        valid_through: 20,
        maximum_fill: 100,
        limit_price: if side == 0 { 40 } else { 60 },
        fee_basis_points: 1_000,
        collateral_account,
    }
}

fn existing_inline_participants(
    seller_intent: CompactIntentV2,
    buyer_intent: CompactIntentV2,
) -> (
    DirectRootStateV1,
    InlineParticipantV2,
    InlineParticipantV2,
    [u8; 32],
    [u8; 32],
) {
    let coordinates = DirectCoordinatesV1::new(id(1), 4).expect("coordinates");
    let seller_seeds = MakerReplaySeedsV1::new(coordinates, id(2)).expect("seller seeds");
    let buyer_seeds = MakerReplaySeedsV1::new(coordinates, id(3)).expect("buyer seeds");
    let (seller_key, seller_bump) = derive_pda(id(10), &seller_seeds.as_slices());
    let (buyer_key, buyer_bump) = derive_pda(id(10), &buyer_seeds.as_slices());
    let seller_nonce_zero = AuthenticatedCompactIntentV2::from_adjacent_ed25519(
        id(2),
        CompactIntentV2 {
            nonce: 0,
            ..seller_intent
        },
    )
    .expect("seller nonce zero");
    let seller_created = consume_nonce_v2(
        DirectRootStateV1::new(),
        MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(seller_bump, 7)),
        seller_nonce_zero.replay().expect("seller replay"),
        NonceConsumptionV2::Inline,
        Some(MakerReplayFirstUseV1 {
            rent_owner: id(90),
            rent_principal: 100,
        }),
    )
    .expect("existing seller root");
    let buyer_nonce_zero = AuthenticatedCompactIntentV2::from_adjacent_ed25519(
        id(3),
        CompactIntentV2 {
            nonce: 0,
            ..buyer_intent
        },
    )
    .expect("buyer nonce zero");
    let buyer_created = consume_nonce_v2(
        seller_created.root,
        MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(buyer_bump, 9)),
        buyer_nonce_zero.replay().expect("buyer replay"),
        NonceConsumptionV2::Inline,
        Some(MakerReplayFirstUseV1 {
            rent_owner: id(91),
            rent_principal: 100,
        }),
    )
    .expect("existing buyer root");
    (
        buyer_created.root,
        InlineParticipantV2 {
            authenticated: AuthenticatedCompactIntentV2::from_adjacent_ed25519(
                id(2),
                seller_intent,
            )
            .expect("seller"),
            maker_replay: MakerReplayObservationV1::Existing(seller_created.maker_root),
            first_use: None,
        },
        InlineParticipantV2 {
            authenticated: AuthenticatedCompactIntentV2::from_adjacent_ed25519(id(3), buyer_intent)
                .expect("buyer"),
            maker_replay: MakerReplayObservationV1::Existing(buyer_created.maker_root),
            first_use: None,
        },
        seller_key,
        buyer_key,
    )
}

fn inline_physical_fixture(
    fill: u64,
    lifecycle: u8,
    delegated_amount: u64,
) -> (
    InlineOrdinaryInputV2,
    DirectInlinePhysicalContextV2,
    DirectInlineCollateralFrameV2,
) {
    let seller_intent = inline_compact(0, lifecycle, 1, id(30));
    let buyer_intent = inline_compact(1, lifecycle, 1, id(31));
    let (root, seller, buyer, seller_key, buyer_key) =
        existing_inline_participants(seller_intent, buyer_intent);
    let core_market = core_market_view(3);
    let release = core_market.release_set().release_set_id.to_bytes();
    let (custody_authority, _) =
        derive_pda(id(20), &[CUSTODY_AUTHORITY_PDA_DOMAIN_V1, &id(1), &release]);
    let (custody_replay, _) = derive_pda(
        id(20),
        &CustodyReplaySeedsV1::new(id(1), release, CallerRoleV1::Trading, buyer_key).as_slices(),
    );
    (
        InlineOrdinaryInputV2 {
            root,
            seller,
            buyer,
            execution: InlineExecutionV2 {
                config: config(1_000, id(6)),
                outcome_count: 3,
                slot: 7,
                fill,
                execution_price: 50,
            },
        },
        DirectInlinePhysicalContextV2 {
            core_market,
            trading_program: id(10),
            claims_program: id(11),
            direct_root: id(50),
            seller_maker_root: seller_key,
            buyer_maker_root: buyer_key,
            custody_replay,
            custody_replay_state: CustodyReplayV1 {
                caller_role: CallerRoleV1::Trading,
                release_set: release,
                market: id(1),
                realm: id(14),
                context: buyer_key,
                caller_program: id(10),
                rent_refund: id(91),
                open_vault_count: 0,
                next_revision: 7,
                generation: 4,
                last_request_digest: id(70),
                last_poststate_commitment: id(71),
            },
            custody_authority,
            parent_request_digest: id(17),
            linked_basis_record_digest: id(39),
            claims_market_revision: 8,
            seller_position_revision: 9,
            buyer_position_revision: 10,
        },
        DirectInlineCollateralFrameV2 {
            buyer_source: DirectExternalDebitV2 {
                account: id(31),
                owner: id(3),
                delegate: custody_authority,
                delegated_amount,
                balance: 100,
            },
            seller_destination: DirectExternalCollateralV2 {
                account: id(30),
                owner: id(2),
                balance: 30,
            },
            fee_destination: DirectExternalCollateralV2 {
                account: id(32),
                owner: id(6),
                balance: 40,
            },
        },
    )
}

fn inline_lifecycle_plans(
    direct: InlineOrdinaryInputV2,
    context: DirectInlinePhysicalContextV2,
) -> DirectInlineLifecyclePlansV3 {
    let settlement = settle_inline_ordinary_v2(direct).expect("inline settlement");
    DirectInlineLifecyclePlansV3 {
        root: StateLifecyclePlanV3::Authenticate(AuthenticateStatePlanV3 {
            state: context.direct_root,
            data_bytes: u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1)
                .expect("root bytes"),
            lamports: 100,
            bump: 1,
        }),
        seller_maker: maker_lifecycle_plan(
            context.seller_maker_root,
            direct.seller.first_use,
            settlement.seller_creation,
            settlement.seller_maker_root,
        ),
        buyer_maker: maker_lifecycle_plan(
            context.buyer_maker_root,
            direct.buyer.first_use,
            settlement.buyer_creation,
            settlement.buyer_maker_root,
        ),
    }
}

fn maker_lifecycle_plan(
    state: [u8; 32],
    first_use: Option<MakerReplayFirstUseV1>,
    creation: Option<dclutch_direct_codec::successor::MakerReplayCreationPlanV1>,
    maker: MakerReplayRootV1,
) -> StateLifecyclePlanV3 {
    match (first_use, creation) {
        (None, None) => StateLifecyclePlanV3::Authenticate(AuthenticateStatePlanV3 {
            state,
            data_bytes: u32::try_from(DIRECT_MAKER_REPLAY_BYTES_V1).expect("maker bytes"),
            lamports: maker.rent_principal(),
            bump: maker.bump(),
        }),
        (Some(first_use), Some(creation)) => StateLifecyclePlanV3::Create(CreateStatePlanV3 {
            state,
            payer: id(88),
            rent_credit: id(89),
            beneficiary: first_use.rent_owner,
            target_data_bytes: u32::try_from(DIRECT_MAKER_REPLAY_BYTES_V1).expect("maker bytes"),
            historical_rent_principal: first_use.rent_principal,
            state_before: creation.observed_lamports,
            state_after: creation.post_lamports,
            payer_debit: creation.top_up_lamports,
            payer_after: 1_000 - creation.top_up_lamports,
            bump: maker.bump(),
        }),
        _ => panic!("semantic lifecycle pair"),
    }
}

#[test]
fn inline_ioc_projects_sparse_claims_and_exhausts_exact_atomic_delegate() {
    let (direct, context, collateral) = inline_physical_fixture(40, 1, 22);
    let mut claims_scratch = [0_u8; DIRECT_INLINE_CLAIMS_REQUEST_BYTES_V2];
    let mut claims_output = [0xa5_u8; DIRECT_INLINE_CLAIMS_REQUEST_BYTES_V2];
    let plan = prepare_inline_ordinary_physical_v2(
        direct,
        context,
        inline_lifecycle_plans(direct, context),
        collateral,
        &mut claims_scratch,
        &mut claims_output,
    )
    .expect("inline physical plan");
    assert_eq!(plan.custody_count, 2);
    assert_eq!(plan.buyer_source_after, 78);
    assert_eq!(plan.buyer_delegated_after, 0);
    assert_eq!(plan.seller_destination_after, 48);
    assert_eq!(plan.fee_destination_after, 44);
    let net = plan.custody[0].expect("seller net");
    assert_eq!(net.request.custody.amount, 18);
    assert_eq!(net.request.custody.expected_revision, 7);
    assert_eq!(net.request.custody.semantic.transfer_index, 0);
    assert_eq!(
        net.request.custody.semantic.order,
        context.parent_request_digest
    );
    assert_eq!(
        net.request.custody.semantic.parent_request_digest,
        context.parent_request_digest
    );
    assert!(net.request.starts_atomic_debit);
    assert!(!net.request.terminal);
    assert_eq!(net.request.allowance_before, 22);
    assert_eq!(net.request.allowance_after, 4);
    assert_eq!(net.delegated_after, 4);
    let fee = plan.custody[1].expect("combined fee");
    assert_eq!(fee.request.custody.amount, 4);
    assert_eq!(fee.request.custody.expected_revision, 8);
    assert_eq!(fee.request.custody.semantic.transfer_index, 1);
    assert!(!fee.request.starts_atomic_debit);
    assert!(fee.request.terminal);
    assert_eq!(fee.request.allowance_before, 4);
    assert_eq!(fee.request.allowance_after, 0);
    assert_eq!(fee.delegated_after, 0);
    let claims = SparseNativeTransferV1::decode(&claims_output).expect("Claims transfer");
    let claims = claims.input();
    assert_eq!(claims.source_owner, id(2));
    assert_eq!(claims.destination_owner, id(3));
    assert_eq!(claims.outcome, 1);
    assert_eq!(claims.claim_count, 3);
    assert_eq!(claims.quantity, 40);

    let mut root = [0xa5; DIRECT_ROOT_STATE_BYTES_V1];
    let mut seller = [0xa5; DIRECT_MAKER_REPLAY_BYTES_V1];
    let mut buyer = [0xa5; DIRECT_MAKER_REPLAY_BYTES_V1];
    encode_inline_state_candidate_v2(
        plan.settlement,
        DirectInlineStateBuffersV2 {
            root_output: &mut root,
            seller_maker_output: &mut seller,
            buyer_maker_output: &mut buyer,
        },
    )
    .expect("state candidates");
    assert_eq!(DirectRootStateV1::decode(&root), Ok(plan.settlement.root));
    assert_eq!(
        MakerReplayRootV1::decode(&seller),
        Ok(plan.settlement.seller_maker_root)
    );
    assert_eq!(
        MakerReplayRootV1::decode(&buyer),
        Ok(plan.settlement.buyer_maker_root)
    );
}

#[test]
fn inline_fok_price_improvement_exhausts_exact_actual_allowance() {
    let (direct, context, collateral) = inline_physical_fixture(100, 0, 55);
    let mut claims_scratch = [0_u8; DIRECT_INLINE_CLAIMS_REQUEST_BYTES_V2];
    let mut claims_output = [0_u8; DIRECT_INLINE_CLAIMS_REQUEST_BYTES_V2];
    let plan = prepare_inline_ordinary_physical_v2(
        direct,
        context,
        inline_lifecycle_plans(direct, context),
        collateral,
        &mut claims_scratch,
        &mut claims_output,
    )
    .expect("price-improved FOK");
    assert_eq!(plan.settlement.effects.gross_collateral, 50);
    assert_eq!(plan.settlement.effects.buyer_collateral_debit, 55);
    assert_eq!(plan.buyer_source_after, 45);
    assert_eq!(plan.buyer_delegated_after, 0);
}

#[test]
fn inline_receipts_bind_exact_claims_custody_and_delegate_poststate() {
    let (direct, context, collateral) = inline_physical_fixture(40, 1, 22);
    let mut claims_scratch = [0_u8; DIRECT_INLINE_CLAIMS_REQUEST_BYTES_V2];
    let mut claims_output = [0_u8; DIRECT_INLINE_CLAIMS_REQUEST_BYTES_V2];
    let plan = prepare_inline_ordinary_physical_v2(
        direct,
        context,
        inline_lifecycle_plans(direct, context),
        collateral,
        &mut claims_scratch,
        &mut claims_output,
    )
    .expect("plan");
    let claims = SparseNativeTransferV1::decode(&claims_output).expect("claims");
    let post_resource_digest = id(77);
    let claims_input = claims.input();
    let claims_receipt = SparseNativeTransferReceiptV1::new(
        claims,
        hash(&claims_output).to_bytes(),
        context.claims_program,
        post_resource_digest,
        claims_input.expected_market_revision + 1,
        claims_input.expected_source_revision + 1,
        claims_input.expected_destination_revision + 1,
    )
    .expect("claims receipt")
    .to_bytes();
    verify_inline_claims_receipt_v2(
        context,
        &claims_output,
        &claims_receipt,
        post_resource_digest,
    )
    .expect("claims receipt verification");
    assert_eq!(
        verify_inline_claims_receipt_v2(context, &claims_output, &claims_receipt, id(76)),
        Err(DirectPhysicalError::Postcondition)
    );

    let net = plan.custody[0].expect("net");
    let request_bytes = net.request.encode().expect("request");
    let poststate = id(78);
    let custody_receipt = CustodyReceiptV1::new(
        net.request.custody,
        hash(&request_bytes).to_bytes(),
        ReceiptEvidenceV1 {
            source_before: collateral.buyer_source.balance,
            source_after: net.source_after,
            destination_before: collateral.seller_destination.balance,
            destination_after: net.destination_after,
            poststate_commitment: poststate,
            replay_state_digest: id(79),
        },
    )
    .expect("custody receipt");
    let custody_receipt = DelegatedCustodyReceiptV2 {
        custody: custody_receipt,
        starts_atomic_debit: net.request.starts_atomic_debit,
        terminal: net.request.terminal,
        delegate_before: net.request.delegate_before,
        delegate_after: net.request.delegate_after,
        total_debit: net.request.total_debit,
        allowance_before: net.request.allowance_before,
        allowance_after: net.request.allowance_after,
    }
    .encode()
    .expect("delegated receipt bytes");
    verify_inline_custody_receipt_v2(net, &custody_receipt, id(79), net.delegated_after)
        .expect("Custody receipt verification");
    let mut divergent_order = net;
    divergent_order.request.custody.semantic.order = id(80);
    assert_eq!(
        verify_inline_custody_receipt_v2(
            divergent_order,
            &custody_receipt,
            id(79),
            net.delegated_after,
        ),
        Err(DirectPhysicalError::Custody)
    );
    assert_eq!(
        verify_inline_custody_receipt_v2(net, &custody_receipt, id(79), net.delegated_after + 1,),
        Err(DirectPhysicalError::Postcondition)
    );
}

#[test]
fn inline_refuses_underallowance_alias_replay_and_lifecycle_substitution() {
    let (direct, context, collateral) = inline_physical_fixture(40, 1, 21);
    let mut claims_scratch = [0_u8; DIRECT_INLINE_CLAIMS_REQUEST_BYTES_V2];
    let mut claims_output = [0xa5_u8; DIRECT_INLINE_CLAIMS_REQUEST_BYTES_V2];
    let before = claims_output;
    assert_eq!(
        prepare_inline_ordinary_physical_v2(
            direct,
            context,
            inline_lifecycle_plans(direct, context),
            collateral,
            &mut claims_scratch,
            &mut claims_output,
        ),
        Err(DirectPhysicalError::Binding)
    );
    assert_eq!(claims_output, before);

    let overallowance = DirectInlineCollateralFrameV2 {
        buyer_source: DirectExternalDebitV2 {
            delegated_amount: 23,
            ..collateral.buyer_source
        },
        ..collateral
    };
    assert_eq!(
        prepare_inline_ordinary_physical_v2(
            direct,
            context,
            inline_lifecycle_plans(direct, context),
            overallowance,
            &mut claims_scratch,
            &mut claims_output,
        ),
        Err(DirectPhysicalError::Binding)
    );
    assert_eq!(claims_output, before);

    let mut bad_replay = context;
    bad_replay.custody_replay_state.next_revision = 0;
    assert_eq!(
        prepare_inline_ordinary_physical_v2(
            direct,
            bad_replay,
            inline_lifecycle_plans(direct, bad_replay),
            DirectInlineCollateralFrameV2 {
                buyer_source: DirectExternalDebitV2 {
                    delegated_amount: 22,
                    ..collateral.buyer_source
                },
                ..collateral
            },
            &mut claims_scratch,
            &mut claims_output,
        ),
        Err(DirectPhysicalError::Binding)
    );

    let aliased = DirectInlineCollateralFrameV2 {
        fee_destination: DirectExternalCollateralV2 {
            account: collateral.buyer_source.account,
            owner: id(6),
            balance: 40,
        },
        buyer_source: DirectExternalDebitV2 {
            delegated_amount: 22,
            ..collateral.buyer_source
        },
        ..collateral
    };
    assert_eq!(
        prepare_inline_ordinary_physical_v2(
            direct,
            context,
            inline_lifecycle_plans(direct, context),
            aliased,
            &mut claims_scratch,
            &mut claims_output,
        ),
        Err(DirectPhysicalError::Binding)
    );

    let coordinates = DirectCoordinatesV1::new(id(1), 4).expect("coordinates");
    let seller_bump = derive_pda(
        context.trading_program,
        &MakerReplaySeedsV1::new(coordinates, id(2))
            .expect("seller seeds")
            .as_slices(),
    )
    .1;
    let buyer_bump = derive_pda(
        context.trading_program,
        &MakerReplaySeedsV1::new(coordinates, id(3))
            .expect("buyer seeds")
            .as_slices(),
    )
    .1;
    let first_use = InlineOrdinaryInputV2 {
        root: DirectRootStateV1::new(),
        seller: InlineParticipantV2 {
            maker_replay: MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(
                seller_bump,
                0,
            )),
            first_use: Some(MakerReplayFirstUseV1 {
                rent_owner: id(90),
                rent_principal: 100,
            }),
            authenticated: AuthenticatedCompactIntentV2::from_adjacent_ed25519(
                id(2),
                CompactIntentV2 {
                    nonce: 0,
                    ..direct.seller.authenticated.intent()
                },
            )
            .expect("first seller"),
        },
        buyer: InlineParticipantV2 {
            maker_replay: MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(
                buyer_bump, 0,
            )),
            first_use: Some(MakerReplayFirstUseV1 {
                rent_owner: id(91),
                rent_principal: 100,
            }),
            authenticated: AuthenticatedCompactIntentV2::from_adjacent_ed25519(
                id(3),
                CompactIntentV2 {
                    nonce: 0,
                    ..direct.buyer.authenticated.intent()
                },
            )
            .expect("first buyer"),
        },
        ..direct
    };
    let first_lifecycle = inline_lifecycle_plans(first_use, context);
    let funded_collateral = DirectInlineCollateralFrameV2 {
        buyer_source: DirectExternalDebitV2 {
            delegated_amount: 22,
            ..collateral.buyer_source
        },
        ..collateral
    };
    let first_plan = prepare_inline_ordinary_physical_v2(
        first_use,
        context,
        first_lifecycle,
        funded_collateral,
        &mut claims_scratch,
        &mut claims_output,
    )
    .expect("generic lifecycle admits maker first use");
    assert!(matches!(
        first_plan.lifecycle.seller_maker,
        StateLifecyclePlanV3::Create(_)
    ));
    assert!(matches!(
        first_plan.lifecycle.buyer_maker,
        StateLifecyclePlanV3::Create(_)
    ));
    assert_eq!(first_plan.settlement.root.open_maker_root_count(), 2);

    let mut substituted = first_lifecycle;
    let StateLifecyclePlanV3::Create(seller_create) = substituted.seller_maker else {
        panic!("seller create")
    };
    substituted.seller_maker = StateLifecyclePlanV3::Create(CreateStatePlanV3 {
        beneficiary: id(92),
        ..seller_create
    });
    let before = claims_output;
    assert_eq!(
        prepare_inline_ordinary_physical_v2(
            first_use,
            context,
            substituted,
            funded_collateral,
            &mut claims_scratch,
            &mut claims_output,
        ),
        Err(DirectPhysicalError::State)
    );
    assert_eq!(claims_output, before);
}
