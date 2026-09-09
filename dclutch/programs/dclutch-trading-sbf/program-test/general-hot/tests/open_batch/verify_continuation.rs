//! The submitted campaign's first verification row, using its actual bank state.

use super::*;
use dclutch_operator::general_session_v1::{GeneralSubjectV1, general_subject_states_v1};
use dclutch_trading::general::{
    candidate_v1::{
        CandidateVerifyRowBuffersV1, CandidateVerifyRowSummaryV1, CandidateVerifyRowViewV1,
        GeneralCandidateErrorV1, candidate_certificate_len_v1, candidate_verifier_len_v1,
        candidate_verify_manifest_orders_v1, verify_candidate_row_v1,
    },
    runtime_manifest::settlement_manifest_len_v2,
    runtime_verify::{RuntimeCandidateVerifierV2, RuntimeVerifyErrorV2},
    runtime_width::{
        ExecutionHeaderV2, ExecutionV2, PageHeaderV2, PageV2, VerifiedCandidateV2, execution_len,
        page_len,
    },
};

/// Solve this campaign's singleton Buy at its marginal limit. An empty fill
/// must still enumerate the admitted order with its complete authenticated
/// terms. At a lower price it would ration an order strictly inside its limit.
/// This is a zero-fill certificate, not evidence of a settled trade.
pub(super) fn single_buy_zero_fill_prices(order_bytes: &[u8], price_scale: u64) -> Vec<u64> {
    let order = GeneralOrderV2::decode(order_bytes).expect("observed Order");
    let header = order.header();
    assert_eq!(order.state().phase, GeneralOrderPhaseV1::Placed);
    assert_eq!(header.side, OrderSideV2::Buy);
    assert_eq!(
        (header.outcome_lo, header.outcome_hi, header.claims_per_lot),
        (0, 0, 1)
    );
    // The order cap is quote atoms per lot, while the simplex uses scaled
    // prices. One unit claim pays at most one atom, not price_scale atoms.
    assert_eq!(header.max_quote_debit_per_lot, header.claims_per_lot);
    let mut prices = vec![0; usize::try_from(header.outcome_count).expect("outcome width")];
    prices[0] = u64::try_from(
        u128::from(header.max_quote_debit_per_lot) * u128::from(price_scale)
            / u128::from(header.claims_per_lot),
    )
    .expect("marginal price in simplex scale units");
    prices
}

fn solver_page(submission: GeneralCandidateV1, order_bytes: &[u8]) -> Vec<u8> {
    let order = GeneralOrderV2::decode(order_bytes).expect("observed escrowed Order");
    let header = order.header();
    let opening = submission.opening();
    assert_eq!(opening.row_count, 1);
    assert_eq!(opening.page_count, 1);
    assert_eq!(header.outcome_count, opening.outcome_count);
    assert_eq!(header.batch_id, opening.batch_id);
    let receive = (0..header.outcome_count)
        .map(|i| order.receive_per_lot(i).expect("order receive"))
        .collect::<Vec<_>>();
    let deliver = (0..header.outcome_count)
        .map(|i| order.deliver_per_lot(i).expect("order deliver"))
        .collect::<Vec<_>>();
    let mut row = vec![0; execution_len(header.outcome_count).expect("execution width")];
    ExecutionV2::encode_into(
        ExecutionHeaderV2 {
            outcome_count: header.outcome_count,
            page_coordinate: 1,
            execution_coordinate: 1,
            nonce: header.nonce,
            order_id: order.order_id(),
            owner_id: header.owner_id,
            max_lots: header.max_lots,
            lots: 0,
        },
        &receive,
        &deliver,
        &mut row,
    )
    .expect("canonical zero-fill enumeration row");
    let mut page = vec![0; page_len(header.outcome_count, 1).expect("one-row page")];
    PageV2::encode_into(
        PageHeaderV2 {
            outcome_count: header.outcome_count,
            page_coordinate: 1,
            page_count: opening.page_count,
            revision: opening.page_revision,
            candidate_id: opening.candidate_id,
        },
        &[&row],
        &mut page,
    )
    .expect("solver page pinned to the actual submission");
    page
}

struct RowOutputs {
    cursor: Vec<u8>,
    certificate: Vec<u8>,
    manifest: Vec<u8>,
}

/// The protocol verifier is the output author, also used by the operator's
/// General Verify derivation. Sentinels prove all three outputs stay atomic
/// when the former uniform-price candidate is refused.
fn replay_row(
    batch: GeneralBatchV2,
    submission: GeneralCandidateV1,
    candidate: &[u8],
    page: &[u8],
    order: &[u8],
) -> (
    Result<CandidateVerifyRowSummaryV1, GeneralCandidateErrorV1>,
    RowOutputs,
) {
    let cursor_len = candidate_verifier_len_v1(submission).expect("verifier width");
    let certificate_len = candidate_certificate_len_v1(submission).expect("certificate width");
    let view = CandidateVerifyRowViewV1 {
        batch,
        submission,
        candidate,
        page,
        order,
        cursor_before: &[],
        verified_before: &[],
        expected_page_index: 0,
        expected_row_index: 0,
        expected_revision: 0,
    };
    let manifest_count = candidate_verify_manifest_orders_v1(&view).expect("manifest row count");
    let manifest_len =
        settlement_manifest_len_v2(submission.opening().outcome_count, manifest_count)
            .expect("manifest width");
    let mut output = RowOutputs {
        cursor: vec![0xa5; cursor_len],
        certificate: vec![0xa5; certificate_len],
        manifest: vec![0xa5; manifest_len],
    };
    let result = verify_candidate_row_v1(
        view,
        CandidateVerifyRowBuffersV1 {
            cursor_scratch: &mut vec![0; cursor_len],
            cursor_output: &mut output.cursor,
            verified_scratch: &mut vec![0; certificate_len],
            verified_output: &mut output.certificate,
            manifest_scratch: &mut vec![0; manifest_len],
            manifest_output: &mut output.manifest,
        },
    );
    (result, output)
}

fn assert_uniform_price_refusal(
    batch: GeneralBatchV2,
    submission: GeneralCandidateV1,
    candidate: &[u8],
    order: &[u8],
) {
    let header = CandidateV2::decode(candidate)
        .expect("candidate image")
        .header();
    let width = u64::from(header.outcome_count);
    let per_outcome = header.price_scale / width;
    let mut prices = vec![per_outcome; usize::try_from(width).expect("outcome width")];
    prices[0] += header.price_scale - per_outcome * width;
    let mut image = vec![0; candidate.len()];
    CandidateV2::encode_into(header, &prices, &mut image).expect("former uniform candidate");
    let candidate_id = general_candidate_identity_v1(&image).expect("uniform identity");
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            candidate_id,
            ..header
        },
        &prices,
        &mut image,
    )
    .expect("addressed former uniform candidate");
    let opening = submission.opening();
    let uniform_submission = GeneralCandidateV1::submit(
        batch,
        CandidateV2::decode(&image).expect("uniform image"),
        opening.page_revision,
        opening.row_count,
        opening.reward_rate_lamports,
        opening.solver_id,
        opening.work_capacity().expect("work capacity"),
        opening.submitted_slot,
    )
    .expect("uniform submission is admitted before verification");
    let page = solver_page(uniform_submission, order);
    let (result, output) = replay_row(batch, uniform_submission, &image, &page, order);
    assert_eq!(
        result,
        Err(GeneralCandidateErrorV1::Verify(
            RuntimeVerifyErrorV2::RationedInsideLimit
        ))
    );
    for bytes in [&output.cursor, &output.certificate, &output.manifest] {
        assert!(
            bytes.iter().all(|byte| *byte == 0xa5),
            "refusal changed an authoritative row output"
        );
    }
}

/// Continue the same ProgramTest bank only after SubmitCandidate has executed.
/// No lifecycle state is installed: the cursor and certificate must be created
/// by the actual transaction. Immutable solver publications are installed by
/// the existing campaign installer, as they were for SubmitCandidate.
#[allow(clippy::too_many_arguments)]
pub(super) async fn verify_first_row(
    context: &mut solana_program_test::ProgramTestContext,
    campaign: &CampaignV1,
    submit: &HostCase,
    batch_key: Pubkey,
    order_key: Pubkey,
    output_page: Pubkey,
    cranker: &Keypair,
    fee_payer: &Keypair,
    seal_payer: &Keypair,
    candidate_image: &[u8],
) {
    let candidate = observed_binding(context, submit.primary_state).await;
    let batch = observed_binding(context, batch_key).await;
    let order = observed_binding(context, order_key).await;
    let candidate_local =
        GeneralLocalStateV3::decode(built_bytes(&candidate)).expect("actual Candidate");
    assert_eq!(
        candidate_local.header().kind,
        GeneralLocalStateKindV3::Candidate
    );
    let submission = GeneralCandidateV1::decode(candidate_local.body()).expect("actual submission");
    assert_eq!(
        submission.state().status,
        GeneralCandidateStatusV1::Submitted
    );
    let (_, closed_batch) = decode_batch(&batch.account);
    assert_eq!(closed_batch.live_order_count(), 1);
    let order_local = GeneralLocalStateV3::decode(built_bytes(&order)).expect("actual Order");
    assert_eq!(order_local.header().kind, GeneralLocalStateKindV3::Order);
    let page = solver_page(submission, order_local.body());
    assert_uniform_price_refusal(
        closed_batch,
        submission,
        candidate_image,
        order_local.body(),
    );
    let (expected, outputs) = replay_row(
        closed_batch,
        submission,
        candidate_image,
        &page,
        order_local.body(),
    );
    let expected = expected.expect("authentic solver row must pass the protocol verifier");
    assert!(expected.complete);
    assert_eq!(
        (
            expected.revision,
            expected.order_count,
            expected.manifest_order_count
        ),
        (1, 1, 0)
    );
    let evidence = EvidenceCorpusV1 {
        closed_batch: Some(batch.clone()),
        candidate_image: Some(staged_record(&campaign.rent, candidate_image.to_vec())),
        candidate_page: Some(staged_record(&campaign.rent, page)),
        order_account: Some(order.clone()),
        settlement_manifest: Some(staged_record(&campaign.rent, outputs.manifest)),
        ..EvidenceCorpusV1::default()
    };
    let chain = ChainPrestateV1 {
        market: observed_binding(context, campaign.state.market.key).await,
        root: observed_binding(context, submit.root).await,
        rent_credit: observed_binding(context, submit.rent_credit).await,
        payer: observed_binding(context, cranker.pubkey()).await,
        output_page: observed_binding(context, output_page).await,
        primary_state: Some(candidate.clone()),
    };
    let root = root_tail_of(&chain.root.account);
    let states = general_subject_states_v1(
        Action::VerifyCandidateRow,
        waist::TRADING_PROGRAM_ID,
        submit.root,
        root,
        GeneralConfigV3::decode(&campaign.release.config).expect("config"),
        root.config_id(),
        campaign.product.product_id,
        campaign.outcome_count,
        GeneralSubjectV1 {
            candidate_id: Some(submission.opening().candidate_id),
            ..GeneralSubjectV1::default()
        },
    )
    .expect("operator-derived Verify state addresses");
    assert_eq!(states.primary.0, submit.primary_state);
    let cursor_key = states.secondary.expect("verifier cursor").0;
    let certificate_key = states.result.expect("verified certificate").0;
    assert_eq!(chain_account(context, cursor_key).await, absent_account());
    assert_eq!(
        chain_account(context, certificate_key).await,
        absent_account()
    );
    let extra_bindings = general_frame_sources_v1(Action::VerifyCandidateRow)
        .expect("operator frame sources")
        .into_iter()
        .filter_map(|(coordinate, source)| {
            let key = match source {
                GeneralFrameSourceV1::SecondaryState => cursor_key,
                GeneralFrameSourceV1::ResultState => certificate_key,
                _ => return None,
            };
            Some((usize::from(coordinate), vacant(key)))
        })
        .collect::<Vec<_>>();
    let slot = context
        .banks_client
        .get_sysvar::<solana_program::clock::Clock>()
        .await
        .expect("bank clock")
        .slot;
    let verify = build_action_case_with_evidence_and_bindings(
        campaign,
        Action::VerifyCandidateRow,
        &chain,
        slot,
        fee_payer.pubkey(),
        &evidence,
        &extra_bindings,
    )
    .expect("Verify bundle from observed Candidate, Batch, Order and solver page");
    let installed = install_absent(context, &verify, &[verify.built.bundle.artifacts.seal]).await;
    let (seal, seal_cu) = produce_seal(context, &verify, seal_payer, fee_payer).await;
    assert_eq!(seal, verify.built.bundle.artifacts.seal);
    waist::set_lookup_table(context, &verify.lookup_addresses);
    assert_frame_control(context, &verify).await;
    let mut before = Vec::new();
    for meta in &verify.built.bundle.hot_instruction.accounts {
        if meta.is_writable && !before.iter().any(|(key, _)| *key == meta.pubkey) {
            before.push((meta.pubkey, chain_account(context, meta.pubkey).await));
        }
    }
    let execution = match waist::submit_v0_observed(
        context,
        &verify.instructions,
        verify.lookup_addresses.clone(),
        Some(fee_payer),
        &[cranker],
    )
    .await
    {
        Ok(execution) => execution,
        Err(refused) => {
            for (key, account) in before {
                assert_eq!(
                    chain_account(context, key).await,
                    account,
                    "Verify refusal failed rollback at {key}"
                );
            }
            eprintln!(
                "general-campaign verify-first-row refused code={:?} cu={} error={:?}",
                refusal_code(&refused.error),
                refused.compute_units_consumed,
                refused.error
            );
            for line in refused.logs {
                eprintln!("{line}");
            }
            panic!("actual VerifyCandidateRow refused after atomic rollback");
        }
    };
    assert_eq!(
        execution
            .logs
            .iter()
            .filter(|line| line.contains(&format!("Program {ACCELERATOR_PROGRAM} invoke")))
            .count(),
        1
    );
    let candidate_after = chain_account(context, submit.primary_state).await;
    let candidate_local =
        GeneralLocalStateV3::decode(&candidate_after.data).expect("advanced Candidate");
    assert_eq!(
        GeneralCandidateV1::decode(candidate_local.body()).expect("advanced submission"),
        expected.submission
    );
    assert_eq!(
        expected.submission.state().status,
        GeneralCandidateStatusV1::Verified
    );
    assert_eq!(
        candidate_after.lamports,
        candidate
            .account
            .lamports
            .checked_sub(expected.reward.lamports)
            .expect("funded reward")
    );
    let cursor = chain_account(context, cursor_key).await;
    assert_eq!(cursor.owner, waist::TRADING_PROGRAM_ID);
    let cursor_local = GeneralLocalStateV3::decode(&cursor.data).expect("created verifier");
    assert_eq!(
        cursor_local.header().kind,
        GeneralLocalStateKindV3::Verifier
    );
    assert_eq!(cursor_local.body(), outputs.cursor);
    let cursor_header = RuntimeCandidateVerifierV2::decode(cursor_local.body())
        .expect("verified cursor")
        .header();
    assert_eq!(
        (
            cursor_header.revision,
            cursor_header.next_page_index,
            cursor_header.next_row_index
        ),
        (1, 1, 0)
    );
    let certificate = chain_account(context, certificate_key).await;
    assert_eq!(certificate.owner, waist::TRADING_PROGRAM_ID);
    // The result lifecycle writes the raw immutable certificate; only
    // Candidate and Verifier carry GeneralLocalState envelopes.
    assert_eq!(certificate.data, outputs.certificate);
    let verified =
        VerifiedCandidateV2::decode(&certificate.data).expect("protocol-produced certificate");
    let cranker_after = chain_account(context, cranker.pubkey()).await;
    assert_eq!(
        cranker_after
            .lamports
            .checked_add(cursor.lamports)
            .and_then(|sum| sum.checked_add(certificate.lamports))
            .expect("created state and cranker balance"),
        chain
            .payer
            .account
            .lamports
            .checked_add(expected.reward.lamports)
            .expect("cranker funding plus earned reward"),
        "the cranker receives its reward and funds exactly the two created states"
    );
    assert_eq!(verified.header().filled_lots, 0);
    assert_eq!(
        verified.header().candidate_id,
        submission.opening().candidate_id
    );
    assert_eq!(chain_account(context, batch_key).await, batch.account);
    assert_eq!(chain_account(context, order_key).await, order.account);
    eprintln!(
        "general-campaign verify-first-row cu={} accounts={} candidate={} cursor={} certificate={} revision=0=>1 filled_lots=0 reward={} installed_records={installed} seal_cu={seal_cu}",
        execution.compute_units_consumed,
        verify.built.bundle.hot_instruction.accounts.len(),
        submit.primary_state,
        cursor_key,
        certificate_key,
        expected.reward.lamports
    );
}

/// Isolate the two evaluator-owned variable bodies while using the exact
/// published action rules and the same unmarked observations as Hot.
fn assert_verify_evidence_projects(width: u32, page: &[u8], manifest: &[u8]) {
    use dclutch_trading::general::account_rules_v3::general_account_profile_rule_v3;
    use dclutch_trading::general::state_artifacts_v3::general_readonly_evidence_count_v3;
    use dclutch_vm::account_profile::AccountObservationV1;
    use dclutch_vm::account_profile::v2::{
        AccountProfileV2, DYNAMIC_FIXED_SPAN_HEADER_BYTES, ProjectionRegistersV2, RULE_BYTES,
        TrustedBuiltinIdentityV2, TrustedEnvironmentV2, TrustedIdentityEnvironmentV2,
        encode::{RegisterGeometryV2, encode_account_profile_with_dynamic_fixed_span_v2_atomic},
        project_dynamic_fixed_spans_atomic,
    };
    let rules = [
        GeneralReadonlyEvidenceKindV3::CandidatePage,
        GeneralReadonlyEvidenceKindV3::SettlementManifest,
    ]
    .map(|kind| {
        let coordinate = (0..general_readonly_evidence_count_v3(Action::VerifyCandidateRow))
            .map(|i| general_readonly_evidence_v3(Action::VerifyCandidateRow, i).expect("evidence"))
            .find(|e| e.kind == kind)
            .expect("selected evidence")
            .coordinate;
        general_account_profile_rule_v3(
            Action::VerifyCandidateRow,
            coordinate,
            general_external_account_widths_v3(64, 192),
        )
        .expect("published rule")
    });
    let mut bytes = vec![0; DYNAMIC_FIXED_SPAN_HEADER_BYTES + 2 * RULE_BYTES];
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::None,
        TrustedIdentityEnvironmentV2::None,
        TrustedBuiltinIdentityV2::None,
        &[],
        &rules,
        &[],
        &[],
        RegisterGeometryV2 {
            common_scalars: 1,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        },
        &mut vec![0; bytes.len()],
        &mut bytes,
    )
    .expect("two evidence rules");
    let observations = [
        AccountObservationV1::new(&[1; 32], &[3; 32], 1, page, false, false, false),
        AccountObservationV1::new(&[2; 32], &[3; 32], 1, manifest, false, false, false),
    ];
    let mut output = [0xa5];
    assert_eq!(
        project_dynamic_fixed_spans_atomic(
            AccountProfileV2::decode(&bytes).expect("profile"),
            width,
            &[],
            &observations,
            ProjectionRegistersV2 {
                input_scalars: &[7],
                input_identities: &[],
                scratch_scalars: &mut [0],
                scratch_identities: &mut [],
                output_scalars: &mut output,
                output_identities: &mut []
            },
            None,
        ),
        Ok(()),
        "actual Page/Manifest bytes must reach their semantic evaluator without forged outer authentication"
    );
    assert_eq!(output, [7]);
}

/// Native control of the exact economic corpus, independent of preceding SBF
/// admission. The old cap and old uniform price are separate counterexamples.
#[test]
fn singleton_buy_marginal_prices_verify_and_refuse_the_two_old_corpora() {
    use dclutch_trading::general::collection_v1::{GeneralBatchOpeningV1, MakerFundingV1};
    for width in [2, 258] {
        for cap in [1, PRICE_SCALE] {
            let mut root =
                GeneralRootV2::active([0x11; 32], [0x12; 32], GENERATION).expect("control root");
            let revision = root.revision();
            let mut batch = GeneralBatchV2::open(
                &mut root,
                GeneralBatchOpeningV1 {
                    outcome_count: width,
                    sequence: 0,
                    generation: GENERATION,
                    market: [0x11; 32],
                    product_id: [0x13; 32],
                    config_id: [0x12; 32],
                    price_scale: PRICE_SCALE,
                    collection_close_slot: 100,
                    settlement_close_slot: 200,
                    max_orders: 1,
                },
                revision,
                10,
            )
            .expect("control batch");
            let header = GeneralOrderHeaderV2 {
                outcome_count: width,
                nonce: 1,
                owner_id: [0x14; 32],
                market: [0x11; 32],
                batch_id: batch.batch_id(),
                generation: GENERATION,
                max_lots: 1,
                max_quote_debit_per_lot: cap,
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
            let mut order_bytes = vec![0; general_order_len_v2(width).expect("order width")];
            GeneralOrderV2::encode_into(
                header,
                &rows.iter().map(|row| row.0).collect::<Vec<_>>(),
                &rows.iter().map(|row| row.1).collect::<Vec<_>>(),
                GeneralOrderStateV1 {
                    phase: GeneralOrderPhaseV1::Placed,
                    admitted_slot: 10,
                    released_slot: 0,
                },
                &mut order_bytes,
            )
            .expect("signed control order");
            batch
                .admit(
                    GeneralOrderV2::decode(&order_bytes).expect("order"),
                    MakerFundingV1 {
                        owner_id: header.owner_id,
                        available_quote: cap,
                        available_claims: &vec![0; usize::try_from(width).expect("width")],
                    },
                    10,
                )
                .expect("funded control order");
            let revision = root.revision();
            batch
                .close(&mut root, revision)
                .expect("closed control batch");
            let mut prices = vec![0; usize::try_from(width).expect("width")];
            prices[0] = PRICE_SCALE;
            if cap == 1 {
                assert_eq!(
                    single_buy_zero_fill_prices(&order_bytes, PRICE_SCALE),
                    prices
                );
            }
            let header = CandidateHeaderV2 {
                outcome_count: width,
                page_count: 1,
                candidate_coordinate: 1,
                price_scale: PRICE_SCALE,
                candidate_id: [0x15; 32],
                product_id: batch.opening().product_id,
                batch_id: batch.batch_id(),
                live_order_count: batch.live_order_count(),
            };
            let mut image = vec![0; candidate_len(width).expect("candidate width")];
            CandidateV2::encode_into(header, &prices, &mut image).expect("draft image");
            let candidate_id = general_candidate_identity_v1(&image).expect("candidate identity");
            CandidateV2::encode_into(
                CandidateHeaderV2 {
                    candidate_id,
                    ..header
                },
                &prices,
                &mut image,
            )
            .expect("canonical candidate");
            let opening = GeneralCandidateOpeningV1 {
                outcome_count: width,
                page_count: 1,
                page_revision: CANDIDATE_PAGE_REVISION,
                submitted_slot: 100,
                candidate_id,
                batch_id: batch.batch_id(),
                solver_id: SOLVER,
                row_count: 1,
                reward_rate_lamports: CRANK_REWARD_LAMPORTS,
            };
            let submission = GeneralCandidateV1::submit(
                batch,
                CandidateV2::decode(&image).expect("image"),
                opening.page_revision,
                opening.row_count,
                opening.reward_rate_lamports,
                opening.solver_id,
                opening.work_capacity().expect("work capacity"),
                opening.submitted_slot,
            )
            .expect("submission");
            let page = solver_page(submission, &order_bytes);
            let (result, outputs) = replay_row(batch, submission, &image, &page, &order_bytes);
            if cap == 1 {
                let result = result.expect("marginal singleton zero-fill row");
                assert_verify_evidence_projects(width, &page, &outputs.manifest);
                assert!(result.complete);
                assert_eq!(
                    result.submission.state().status,
                    GeneralCandidateStatusV1::Verified
                );
                assert_eq!(
                    VerifiedCandidateV2::decode(&outputs.certificate)
                        .expect("certificate")
                        .header()
                        .filled_lots,
                    0
                );
                assert_uniform_price_refusal(batch, submission, &image, &order_bytes);
            } else {
                assert_eq!(
                    result,
                    Err(GeneralCandidateErrorV1::Verify(
                        RuntimeVerifyErrorV2::RationedInsideLimit
                    ))
                );
                for bytes in [&outputs.cursor, &outputs.certificate, &outputs.manifest] {
                    assert!(bytes.iter().all(|byte| *byte == 0xa5));
                }
            }
        }
    }
}
