//! Two authenticated counterpart orders and exact nonzero settlement inputs.
//!
//! Integration into the existing bank (the parent owns these edits):
//! 1. Declare this child module; add a distinct seller wallet at genesis.
//! 2. Before the first PlaceOrder, call `opposing_sell` and `funding_corpora`.
//!    Install its seller position/token and Hoard alongside the buyer corpus;
//!    the buyer's outcome-zero starting balance becomes three, not four.
//! 3. Execute the Buy and Sell through the existing PlaceOrder builder, reading
//!    the common Batch/Root/Claims aggregate back between transactions. The
//!    seller signs the second placement; its quote reserve is zero.
//! 4. Close that two-order Batch, then call `prepare_candidate` with the actual
//!    two Order bodies and actual closed Batch. Submit its image/submission.
//! 5. Verify both sorted rows in `verification`, reading actual Candidate and
//!    Verifier poststates between calls. Use each row's matching Order account.
//! 6. Consider/Freeze the produced certificate through existing operator
//!    constructors, initialize settlement, and execute `settlement` in order.
//!    Its six effects move one quote atom and one claim between distinct owners.
//!    Compare actual cursor/effects and final accounts against these outputs.
//!
//! The parent EvidenceCorpus also needs `verifier_account` for row two,
//! `verified_candidate` and `selection_policy` for Consider, and observed
//! selection/verifier bindings for initialization. Preserve their actual
//! lifecycle envelopes: copying an inner body into a staged account would
//! conceal an AccountProfile/adapter width disagreement. Collect/Distribute
//! use each step's manifest; Materialize/Close must supply no manifest.
//!
//! This module supplies corpus and native expected outputs, never bank writes,
//! transaction builders, synthetic accepted state, or a second campaign.

use super::*;
use dclutch_operator::general_session_v1::{general_custody_authority_v1, general_hoard_vault_v1};
use dclutch_trading::general::{
    candidate_v1::{
        CandidateVerifyRowBuffersV1, CandidateVerifyRowViewV1, candidate_certificate_len_v1,
        candidate_verifier_len_v1, candidate_verify_manifest_orders_v1, verify_candidate_row_v1,
    },
    runtime_manifest::{SettlementManifestV2, settlement_manifest_len_v2},
    runtime_settlement::{
        RuntimeSettlementActionV2, RuntimeSettlementBuffersV2, RuntimeSettlementEffectPlanV2,
        RuntimeSettlementViewV2, evaluate_runtime_settlement_v2, initialize_runtime_settlement_v2,
        runtime_settlement_effect_len_v2,
    },
    runtime_verify::{
        RuntimeCompleteSetMoveV2, runtime_identity_precedes_v2, runtime_verified_balance_v2,
    },
    runtime_width::{
        ExecutionHeaderV2, ExecutionV2, PageHeaderV2, PageV2, SettlementCursorV2,
        SettlementPhaseV2, VerifiedCandidateV2, execution_len, page_len, settlement_cursor_len,
    },
};

/// The exact immutable terms to submit with the seller's real signature.
pub(super) struct OpposingSellV1 {
    pub order: Vec<u8>,
    pub signed_terms: Vec<u8>,
    pub order_id: [u8; 32],
}

pub(super) fn opposing_sell(
    buy_bytes: &[u8],
    seller: Pubkey,
    admitted_slot: u64,
) -> OpposingSellV1 {
    let buy = GeneralOrderV2::decode(buy_bytes).expect("observed Buy");
    let header = buy.header();
    assert_eq!(header.side, OrderSideV2::Buy);
    assert_eq!(
        (
            header.max_lots,
            header.claims_per_lot,
            header.max_quote_debit_per_lot
        ),
        (1, 1, 1)
    );
    assert_eq!((header.outcome_lo, header.outcome_hi), (0, 0));
    assert_ne!(
        header.owner_id,
        seller.to_bytes(),
        "counterparties are distinct"
    );
    let sell_header = GeneralOrderHeaderV2 {
        owner_id: seller.to_bytes(),
        side: OrderSideV2::Sell,
        max_quote_debit_per_lot: 0,
        min_quote_credit_per_lot: 1,
        ..header
    };
    let rows = (0..header.outcome_count)
        .map(|i| sell_header.derived_row(i))
        .collect::<Vec<_>>();
    let mut order = vec![0; general_order_len_v2(header.outcome_count).expect("Order width")];
    GeneralOrderV2::encode_into(
        sell_header,
        &rows.iter().map(|row| row.0).collect::<Vec<_>>(),
        &rows.iter().map(|row| row.1).collect::<Vec<_>>(),
        GeneralOrderStateV1 {
            phase: GeneralOrderPhaseV1::Placed,
            admitted_slot,
            released_slot: 0,
        },
        &mut order,
    )
    .expect("canonical opposing Sell");
    let decoded = GeneralOrderV2::decode(&order).expect("Sell");
    assert_eq!(decoded.quote_reserve().expect("Sell quote reserve"), 0);
    let order_id = decoded.order_id();
    let mut signed_terms =
        vec![0; general_signed_order_terms_len_v2(header.outcome_count).expect("terms width")];
    decoded
        .encode_signed_terms_into(&mut signed_terms)
        .expect("seller terms");
    OpposingSellV1 {
        order,
        signed_terms,
        order_id,
    }
}

/// These are genesis fixtures with conserved supply, not executed founding.
/// Four existing complete sets remain backed by four quote atoms in the Hoard.
/// One additional quote atom belongs to the buyer and is exchanged for the
/// seller's one already-existing outcome-zero claim.
pub(super) struct MatchedFundingV1 {
    pub buyer: PlaceOrderCorpusV1,
    pub seller: PlaceOrderCorpusV1,
    pub hoard: BuiltAccountV1,
}

pub(super) fn funding_corpora(
    campaign: &CampaignV1,
    buyer_chain: &ChainPrestateV1,
    buyer_order: [u8; 32],
    seller_chain: &ChainPrestateV1,
    seller_order: [u8; 32],
) -> MatchedFundingV1 {
    assert_ne!(buyer_chain.payer.key, seller_chain.payer.key);
    assert_eq!(buyer_chain.market.key, seller_chain.market.key);
    let mut buyer = place_order_corpus(campaign, buyer_chain, buyer_order, 1);
    let mut seller = place_order_corpus(campaign, seller_chain, seller_order, 0);
    let count = usize::try_from(campaign.outcome_count).expect("width");
    let mut buyer_claims = vec![4; count];
    buyer_claims[0] = 3;
    let mut seller_claims = vec![0; count];
    seller_claims[0] = 1;
    for (corpus, owner, balances) in [
        (&mut buyer, buyer_chain.payer.key, &buyer_claims),
        (&mut seller, seller_chain.payer.key, &seller_claims),
    ] {
        let seeds =
            ProtocolPositionSeedsV2::new(corpus.claims_market.key.to_bytes(), owner.to_bytes())
                .expect("maker position seeds");
        let (key, bump) =
            Pubkey::find_program_address(&seeds.as_slices(), &waist::CLAIMS_PROGRAM_ID);
        assert_eq!(key, corpus.maker_position.key);
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision: 1,
                market_account: corpus.claims_market.key.to_bytes(),
                owner: owner.to_bytes(),
                basis_id: campaign.product.semantic_basis,
            },
            balances,
            &mut corpus.maker_position.account.data,
        )
        .expect("conserved party holdings");
        put_liability_basis_position_bump_v2(&mut corpus.maker_position.account.data, bump)
            .expect("maker bump");
        // The existing token fixture is the canonical extension-free 82-byte
        // mint. Hoard principal plus buyer spendable balance equals its supply.
        corpus.mint.account.data = token_mint_bytes(5);
    }
    let market = buyer_chain.market.key.to_bytes();
    let authority = general_custody_authority_v1(
        waist::CUSTODY_PROGRAM_ID,
        market,
        campaign.releases.release_set,
    );
    let hoard_key = general_hoard_vault_v1(
        waist::CUSTODY_PROGRAM_ID,
        market,
        campaign.releases.release_set,
    );
    let hoard = data_account(
        &campaign.rent,
        hoard_key,
        GENERAL_TOKEN_PROGRAM,
        token_account_bytes(GENERAL_COLLATERAL_MINT, authority, 4, authority, 0),
    );
    MatchedFundingV1 {
        buyer,
        seller,
        hoard,
    }
}

pub(super) struct VerificationRowV1 {
    /// Which actual Order account belongs at EscrowedOrder for this row.
    pub order_id: [u8; 32],
    pub cursor_after: Vec<u8>,
    pub certificate_after: Vec<u8>,
    pub manifest: Vec<u8>,
    pub submission_after: GeneralCandidateV1,
}

pub(super) struct SettlementStepV1 {
    pub action: RuntimeSettlementActionV2,
    pub manifest_order_index: u32,
    pub manifest: Option<Vec<u8>>,
    pub cursor_after: Vec<u8>,
    pub effect: Vec<u8>,
}

pub(super) struct MatchedCandidateV1 {
    pub image: Vec<u8>,
    pub page: Vec<u8>,
    pub submission: GeneralCandidateV1,
    pub verification: Vec<VerificationRowV1>,
    pub initial_settlement: Vec<u8>,
    pub settlement: Vec<SettlementStepV1>,
}

/// Prepare from actual closed Batch and actually admitted Order bodies. All
/// identities, prices, rows, manifests and effect bytes use their protocol
/// authors. The row sort uses the protocol's identity ordering, not Pubkey's.
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_candidate(
    batch: GeneralBatchV2,
    buy_bytes: &[u8],
    sell_bytes: &[u8],
    solver: Pubkey,
    submitted_slot: u64,
    page_revision: u64,
    reward: u64,
    surplus_beneficiary: [u8; 32],
) -> MatchedCandidateV1 {
    assert_eq!(batch.live_order_count(), 2);
    let buy = GeneralOrderV2::decode(buy_bytes).expect("actual Buy");
    let sell = GeneralOrderV2::decode(sell_bytes).expect("actual Sell");
    assert_eq!(buy.header().side, OrderSideV2::Buy);
    assert_eq!(sell.header().side, OrderSideV2::Sell);
    assert_ne!(buy.header().owner_id, sell.header().owner_id);
    for order in [buy, sell] {
        assert_eq!(order.header().batch_id, batch.batch_id());
        assert_eq!(
            (order.header().outcome_lo, order.header().outcome_hi),
            (0, 0)
        );
        assert_eq!(
            (order.header().max_lots, order.header().claims_per_lot),
            (1, 1)
        );
    }
    assert_eq!(buy.header().max_quote_debit_per_lot, 1);
    assert_eq!(sell.header().min_quote_credit_per_lot, 1);
    let width = batch.opening().outcome_count;
    let mut prices = vec![0; usize::try_from(width).expect("width")];
    prices[0] = batch.opening().price_scale;
    let header = CandidateHeaderV2 {
        outcome_count: width,
        page_count: 1,
        candidate_coordinate: 1,
        price_scale: batch.opening().price_scale,
        candidate_id: [0x7c; 32],
        product_id: batch.opening().product_id,
        batch_id: batch.batch_id(),
        live_order_count: batch.live_order_count(),
    };
    let mut image = vec![0; candidate_len(width).expect("candidate width")];
    CandidateV2::encode_into(header, &prices, &mut image).expect("draft candidate");
    let candidate_id = general_candidate_identity_v1(&image).expect("candidate identity");
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            candidate_id,
            ..header
        },
        &prices,
        &mut image,
    )
    .expect("content addressed candidate");
    let mut orders = [buy, sell];
    if runtime_identity_precedes_v2(&orders[1].order_id(), &orders[0].order_id()) {
        orders.swap(0, 1);
    }
    let mut rows = Vec::new();
    for (index, order) in orders.iter().enumerate() {
        let header = order.header();
        let mut row = vec![0; execution_len(width).expect("row width")];
        ExecutionV2::encode_into(
            ExecutionHeaderV2 {
                outcome_count: width,
                page_coordinate: 1,
                execution_coordinate: u32::try_from(index).expect("row index") + 1,
                nonce: header.nonce,
                order_id: order.order_id(),
                owner_id: header.owner_id,
                max_lots: header.max_lots,
                lots: 1,
            },
            &(0..width)
                .map(|i| order.receive_per_lot(i).expect("receive"))
                .collect::<Vec<_>>(),
            &(0..width)
                .map(|i| order.deliver_per_lot(i).expect("deliver"))
                .collect::<Vec<_>>(),
            &mut row,
        )
        .expect("canonical filled execution");
        rows.push(row);
    }
    let mut page = vec![0; page_len(width, 2).expect("two-row page")];
    PageV2::encode_into(
        PageHeaderV2 {
            outcome_count: width,
            page_coordinate: 1,
            page_count: 1,
            revision: page_revision,
            candidate_id,
        },
        &rows.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        &mut page,
    )
    .expect("solver page");
    let opening = GeneralCandidateOpeningV1 {
        outcome_count: width,
        page_count: 1,
        page_revision,
        submitted_slot,
        candidate_id,
        batch_id: batch.batch_id(),
        solver_id: solver.to_bytes(),
        row_count: 2,
        reward_rate_lamports: reward,
    };
    let submission = GeneralCandidateV1::submit(
        batch,
        CandidateV2::decode(&image).expect("image"),
        page_revision,
        2,
        reward,
        solver.to_bytes(),
        opening.work_capacity().expect("work capacity"),
        submitted_slot,
    )
    .expect("funded two-row submission");
    let mut current_submission = submission;
    let cursor_len = candidate_verifier_len_v1(submission).expect("verifier width");
    let certificate_len = candidate_certificate_len_v1(submission).expect("certificate width");
    let mut cursor = Vec::new();
    let mut verification = Vec::new();
    for (index, order) in orders.iter().enumerate() {
        let order_bytes = if order.order_id() == buy.order_id() {
            buy_bytes
        } else {
            sell_bytes
        };
        let view = CandidateVerifyRowViewV1 {
            batch,
            submission: current_submission,
            candidate: &image,
            page: &page,
            order: order_bytes,
            cursor_before: &cursor,
            verified_before: &[],
            expected_page_index: 0,
            expected_row_index: u32::try_from(index).expect("row index"),
            expected_revision: u64::try_from(index).expect("revision"),
        };
        let count = candidate_verify_manifest_orders_v1(&view).expect("manifest row count");
        let len = settlement_manifest_len_v2(width, count).expect("manifest width");
        let mut next = vec![0; cursor_len];
        let mut certificate = vec![0; certificate_len];
        let mut manifest = vec![0; len];
        let result = verify_candidate_row_v1(
            view,
            CandidateVerifyRowBuffersV1 {
                cursor_scratch: &mut vec![0; cursor_len],
                cursor_output: &mut next,
                verified_scratch: &mut vec![0; certificate_len],
                verified_output: &mut certificate,
                manifest_scratch: &mut vec![0; len],
                manifest_output: &mut manifest,
            },
        )
        .expect("authenticated fully filled row");
        assert_eq!(result.complete, index == 1);
        current_submission = result.submission;
        cursor = next.clone();
        verification.push(VerificationRowV1 {
            order_id: order.order_id(),
            cursor_after: next,
            certificate_after: certificate,
            manifest,
            submission_after: current_submission,
        });
    }
    let terminal = verification.last().expect("terminal verification");
    let verified =
        VerifiedCandidateV2::decode(&terminal.certificate_after).expect("nonempty certificate");
    assert_eq!(verified.header().filled_lots, 2);
    assert_eq!(
        (
            verified.header().quote_debit,
            verified.header().quote_credit
        ),
        (1, 1)
    );
    for outcome in 0..width {
        let expected = u64::from(outcome == 0);
        assert_eq!(
            verified.claim_input(outcome).expect("claim input"),
            expected
        );
        assert_eq!(
            verified.claim_output(outcome).expect("claim output"),
            expected
        );
    }
    let balance =
        runtime_verified_balance_v2(&terminal.certificate_after).expect("balanced certificate");
    assert_eq!(balance.complete_set_move, RuntimeCompleteSetMoveV2::None);
    assert_eq!(
        (balance.complete_set_quantity, balance.quote_surplus),
        (0, 0)
    );
    assert_eq!(
        SettlementManifestV2::decode(&terminal.manifest)
            .expect("manifest")
            .header()
            .order_count,
        2
    );
    let settlement_len = settlement_cursor_len(width).expect("settlement width");
    let inventory_len = usize::try_from(width).expect("width") * 8;
    let mut initial_settlement = vec![0; settlement_len];
    initialize_runtime_settlement_v2(
        &terminal.cursor_after,
        &terminal.certificate_after,
        0,
        &mut vec![0; inventory_len],
        &mut vec![0; settlement_len],
        &mut initial_settlement,
    )
    .expect("initialize settlement from actual verifier output");
    let mut settlement_cursor = initial_settlement.clone();
    let mut settlement = Vec::new();
    for (action, index) in [
        (RuntimeSettlementActionV2::Collect, 0),
        (RuntimeSettlementActionV2::Collect, 1),
        (RuntimeSettlementActionV2::Materialize, 0),
        (RuntimeSettlementActionV2::Distribute, 0),
        (RuntimeSettlementActionV2::Distribute, 1),
        (RuntimeSettlementActionV2::Close, 0),
    ] {
        let row_action = matches!(
            action,
            RuntimeSettlementActionV2::Collect | RuntimeSettlementActionV2::Distribute
        );
        let manifest = row_action.then(|| terminal.manifest.clone());
        let mut cursor_after = vec![0; settlement_len];
        let effect_len = runtime_settlement_effect_len_v2(width).expect("effect width");
        let mut effect = vec![0; effect_len];
        evaluate_runtime_settlement_v2(
            RuntimeSettlementViewV2 {
                action,
                cursor_before: &settlement_cursor,
                verified: &terminal.certificate_after,
                manifest: manifest.as_deref(),
                manifest_order_index: index,
                expected_revision: SettlementCursorV2::decode(&settlement_cursor)
                    .expect("cursor")
                    .header()
                    .revision,
                surplus_beneficiary: (action == RuntimeSettlementActionV2::Close)
                    .then_some(surplus_beneficiary),
            },
            RuntimeSettlementBuffersV2 {
                cursor_scratch: &mut vec![0; settlement_len],
                cursor_output: &mut cursor_after,
                inventory_scratch: &mut vec![0; inventory_len],
                effect_scratch: &mut vec![0; effect_len],
                effect_output: &mut effect,
            },
        )
        .expect("permissionless nonzero settlement step");
        settlement_cursor = cursor_after.clone();
        settlement.push(SettlementStepV1 {
            action,
            manifest_order_index: index,
            manifest,
            cursor_after,
            effect,
        });
    }
    assert_nonzero_economics(
        &settlement,
        buy.header().owner_id,
        sell.header().owner_id,
        width,
    );
    MatchedCandidateV1 {
        image,
        page,
        submission,
        verification,
        initial_settlement,
        settlement,
    }
}

/// Exact economic deltas from the protocol's effect plans, separate from rent
/// and transaction fees in lamports. The extension-free mint has no token
/// transfer fee and this matched certificate has no quote surplus.
fn assert_nonzero_economics(
    steps: &[SettlementStepV1],
    buyer: [u8; 32],
    seller: [u8; 32],
    width: u32,
) {
    let mut buyer_quote_debit = 0;
    let mut seller_quote_credit = 0;
    let mut seller_claims_collected = 0;
    let mut buyer_claims_distributed = 0;
    for step in steps {
        let effect = RuntimeSettlementEffectPlanV2::decode(&step.effect).expect("canonical effect");
        let header = effect.header();
        match step.action {
            RuntimeSettlementActionV2::Collect => {
                if header.owner_id == buyer {
                    buyer_quote_debit += header.quote_quantity;
                } else {
                    assert_eq!(header.owner_id, seller);
                    seller_claims_collected += effect.quantity(0).expect("claim quantity");
                }
            }
            RuntimeSettlementActionV2::Distribute => {
                if header.owner_id == seller {
                    seller_quote_credit += header.quote_quantity;
                } else {
                    assert_eq!(header.owner_id, buyer);
                    buyer_claims_distributed += effect.quantity(0).expect("claim quantity");
                }
            }
            RuntimeSettlementActionV2::Materialize | RuntimeSettlementActionV2::Close => {
                assert_eq!(header.quote_quantity, 0);
                assert_eq!(header.complete_set_quantity, 0);
            }
        }
        for outcome in 1..width {
            assert_eq!(effect.quantity(outcome).expect("other claims"), 0);
        }
    }
    assert_eq!(
        (
            buyer_quote_debit,
            seller_quote_credit,
            seller_claims_collected,
            buyer_claims_distributed
        ),
        (1, 1, 1, 1)
    );
    let terminal = SettlementCursorV2::decode(&steps.last().expect("Close").cursor_after)
        .expect("terminal cursor");
    assert_eq!(terminal.header().phase, SettlementPhaseV2::Terminal);
    assert_eq!(terminal.header().quote_inventory, 0);
    for outcome in 0..width {
        assert_eq!(terminal.inventory(outcome).expect("terminal inventory"), 0);
    }
}

/// Final balances for bank assertions after the six settlement actions.
/// Rent, work rewards and transaction fees are SOL lamports and must be
/// asserted separately; none of them debits these collateral token balances.
pub(super) struct MatchedFinalBalancesV1 {
    pub buyer_quote: u64,
    pub seller_quote: u64,
    pub hoard_quote: u64,
    pub collateral_mint_supply: u64,
    pub buyer_claims: Vec<u64>,
    pub seller_claims: Vec<u64>,
    pub aggregate_claim_supply: Vec<u64>,
    pub order_escrow_quote: [u64; 2],
    pub order_escrow_claims: Vec<u64>,
    pub settlement_quote: u64,
    pub settlement_claims: Vec<u64>,
    pub collateral_fees: u64,
    pub quote_surplus: u64,
}

pub(super) fn final_balances(width: u32) -> MatchedFinalBalancesV1 {
    let count = usize::try_from(width).expect("outcome width");
    MatchedFinalBalancesV1 {
        buyer_quote: 0,
        seller_quote: 1,
        hoard_quote: 4,
        collateral_mint_supply: 5,
        buyer_claims: vec![4; count],
        seller_claims: vec![0; count],
        aggregate_claim_supply: vec![4; count],
        order_escrow_quote: [0, 0],
        order_escrow_claims: vec![0; count],
        settlement_quote: 0,
        settlement_claims: vec![0; count],
        collateral_fees: 0,
        quote_surplus: 0,
    }
}

#[test]
fn matched_buy_sell_replays_nonzero_claim_and_quote_through_terminal_settlement() {
    use dclutch_trading::general::collection_v1::{GeneralBatchOpeningV1, MakerFundingV1};
    for width in [2, 258] {
        let market = [0x21; 32];
        let buyer = Pubkey::new_from_array([0x22; 32]);
        let seller = Pubkey::new_from_array([0x23; 32]);
        let mut root = GeneralRootV2::active(market, [0x24; 32], GENERATION).expect("root");
        let revision = root.revision();
        let config_id = root.config_id();
        let mut batch = GeneralBatchV2::open(
            &mut root,
            GeneralBatchOpeningV1 {
                outcome_count: width,
                sequence: 0,
                generation: GENERATION,
                market,
                product_id: [0x25; 32],
                config_id,
                price_scale: PRICE_SCALE,
                collection_close_slot: 100,
                settlement_close_slot: 200,
                max_orders: 2,
            },
            revision,
            10,
        )
        .expect("open two-order batch");
        let header = GeneralOrderHeaderV2 {
            outcome_count: width,
            nonce: 1,
            owner_id: buyer.to_bytes(),
            market,
            batch_id: batch.batch_id(),
            generation: GENERATION,
            max_lots: 1,
            max_quote_debit_per_lot: 1,
            min_quote_credit_per_lot: 0,
            valid_until_slot: 200,
            side: OrderSideV2::Buy,
            outcome_lo: 0,
            outcome_hi: 0,
            claims_per_lot: 1,
        };
        let rows = (0..width)
            .map(|outcome| header.derived_row(outcome))
            .collect::<Vec<_>>();
        let mut buy = vec![0; general_order_len_v2(width).expect("Order width")];
        GeneralOrderV2::encode_into(
            header,
            &rows.iter().map(|row| row.0).collect::<Vec<_>>(),
            &rows.iter().map(|row| row.1).collect::<Vec<_>>(),
            GeneralOrderStateV1 {
                phase: GeneralOrderPhaseV1::Placed,
                admitted_slot: 10,
                released_slot: 0,
            },
            &mut buy,
        )
        .expect("Buy");
        let sell = opposing_sell(&buy, seller, 11);
        let mut seller_claims = vec![0; usize::try_from(width).expect("width")];
        seller_claims[0] = 1;
        for (bytes, owner, quote, claims, slot) in [
            (
                &buy,
                buyer,
                1,
                vec![0; usize::try_from(width).expect("width")],
                10,
            ),
            (&sell.order, seller, 0, seller_claims, 11),
        ] {
            batch
                .admit(
                    GeneralOrderV2::decode(bytes).expect("Order"),
                    MakerFundingV1 {
                        owner_id: owner.to_bytes(),
                        available_quote: quote,
                        available_claims: &claims,
                    },
                    slot,
                )
                .expect("genuinely funded order");
        }
        let revision = root.revision();
        batch
            .close(&mut root, revision)
            .expect("closed two-order batch");
        let candidate = prepare_candidate(
            batch,
            &buy,
            &sell.order,
            Pubkey::new_from_array(SOLVER),
            100,
            CANDIDATE_PAGE_REVISION,
            CRANK_REWARD_LAMPORTS,
            [0x26; 32],
        );
        assert_eq!(candidate.verification.len(), 2);
        assert_eq!(candidate.settlement.len(), 6);
        assert_eq!(
            PageV2::decode(&candidate.page).expect("page").row_count(),
            2
        );
        assert_eq!(candidate.submission.opening().row_count, 2);
        assert_eq!(
            candidate.verification[0].submission_after.state().status,
            GeneralCandidateStatusV1::Submitted
        );
        assert_eq!(
            candidate.verification[1].submission_after.state().status,
            GeneralCandidateStatusV1::Verified
        );
        let balances = final_balances(width);
        assert_eq!(
            balances.buyer_quote + balances.seller_quote + balances.hoard_quote,
            balances.collateral_mint_supply
        );
        for outcome in 0..usize::try_from(width).expect("width") {
            assert_eq!(
                balances.buyer_claims[outcome] + balances.seller_claims[outcome],
                balances.aggregate_claim_supply[outcome]
            );
        }
        assert_eq!(balances.collateral_fees, 0);
        assert_eq!(balances.quote_surplus, 0);
    }
}
