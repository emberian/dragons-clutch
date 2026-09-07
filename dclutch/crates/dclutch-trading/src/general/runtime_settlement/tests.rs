//! Hostile coverage for runtime-width two-pass settlement.
//!
//! Every refusal below names the exact `RuntimeSettlementErrorV2` variant, so a
//! change that keeps settlement refusing for a different reason is a visible
//! failure rather than a silent reinterpretation.

extern crate std;

use super::*;
use crate::general::runtime_candidate::{
    GENERAL_SETTLEMENT_BENEFICIARY_IDENTITY_V2, GENERAL_SETTLEMENT_COMMON_IDENTITIES_V2,
    GENERAL_SETTLEMENT_COMMON_SCALARS_V2, GENERAL_SETTLEMENT_ITEM_IDENTITY_STRIDE_V2,
    GENERAL_SETTLEMENT_ITEM_SCALAR_STRIDE_V2, GENERAL_SETTLEMENT_MOVE_SCALAR_V2,
    general_settlement_candidate_bank_len_v2, general_settlement_scalar_count_v2,
    project_general_settlement_candidate_v2,
};
use crate::general::runtime_manifest::settlement_manifest_len_v2;
use crate::general::runtime_verify::{
    AuthenticatedOrderTermsV2, OrderSideV2, RuntimeConsiderRowBuffersV2, RuntimeConsiderRowViewV2,
    RuntimeManifestBuffersV2, evaluate_runtime_consider_row_with_manifest_v2,
    runtime_verifier_len_v2,
};
use crate::general::runtime_width::{
    CandidateHeaderV2, CandidateV2, ExecutionHeaderV2, ExecutionV2, PageHeaderV2, PageV2,
    candidate_len, execution_len, page_len, verified_candidate_len,
};
use std::vec;
use std::vec::Vec;

const CANDIDATE: [u8; 32] = [1; 32];
const PRODUCT: [u8; 32] = [2; 32];
const BATCH: [u8; 32] = [3; 32];
const OWNER: [u8; 32] = [4; 32];
const BENEFICIARY: [u8; 32] = [9; 32];

struct TerminalFixture {
    width: u32,
    verifier: Vec<u8>,
    verified: Vec<u8>,
    manifests: Vec<Vec<u8>>,
}

fn order_id(low: u8) -> [u8; 32] {
    let mut value = [0_u8; 32];
    value[0] = low;
    value
}

/// One order's shape: side, the single outcome it moves claims at, and how
/// many it moves per lot.
type Shape = (OrderSideV2, u32, u64);

fn row(
    width: u32,
    page_coordinate: u32,
    order_low: u8,
    lots: u64,
    shape: Shape,
    max_lots: u64,
    debit_limit: u64,
) -> (Vec<u8>, AuthenticatedOrderTermsV2) {
    let (side, outcome, claims_per_lot) = shape;
    let id = order_id(order_low);
    let terms = AuthenticatedOrderTermsV2 {
        order_id: id,
        owner_id: OWNER,
        nonce: u64::from(order_low),
        max_lots,
        max_quote_debit_per_lot: debit_limit,
        min_quote_credit_per_lot: 0,
        side,
        outcome_lo: outcome,
        outcome_hi: outcome,
        claims_per_lot,
    };
    let receive: Vec<u64> = (0..width).map(|index| terms.derived_row(index).0).collect();
    let deliver: Vec<u64> = (0..width).map(|index| terms.derived_row(index).1).collect();
    let mut bytes = vec![0; execution_len(width).expect("execution length")];
    ExecutionV2::encode_into(
        ExecutionHeaderV2 {
            outcome_count: width,
            page_coordinate,
            execution_coordinate: 1,
            nonce: terms.nonce,
            order_id: terms.order_id,
            owner_id: terms.owner_id,
            max_lots: terms.max_lots,
            lots,
        },
        &receive,
        &deliver,
        &mut bytes,
    )
    .expect("execution");
    (bytes, terms)
}

/// Three orders on ONE outcome: two buys of two lots and a sell of one, each
/// filling its whole signed maximum.
///
/// Since the joint clearing every row moves claims at exactly one coordinate,
/// so the clearing is concentrated rather than uniform. At outcome zero the
/// net flow is `4 - 1 = 3`, which is the complete-set mint; at every other
/// outcome it is zero, and a residual of three sits there. That is admissible
/// only where the price is zero, so the whole scale lands on outcome zero --
/// and the close STRANDS the residual instead of refusing it.
fn terminal_fixture(width: u32) -> TerminalFixture {
    let count = usize::try_from(width).expect("test width");
    let mut prices = vec![0_u64; count];
    prices[0] = u64::from(width);
    let mut candidate = vec![0; candidate_len(width).expect("candidate length")];
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            outcome_count: width,
            page_count: 3,
            candidate_coordinate: 1,
            price_scale: u64::from(width),
            candidate_id: CANDIDATE,
            product_id: PRODUCT,
            batch_id: BATCH,
            live_order_count: 3,
        },
        &prices,
        &mut candidate,
    )
    .expect("candidate");

    // Every page is intentionally unbalanced. Only the complete Candidate
    // has the complete-set relation required for settlement.
    let rows = [
        row(width, 1, 1, 2, (OrderSideV2::Buy, 0, 1), 2, 2),
        row(width, 2, 2, 1, (OrderSideV2::Sell, 0, 1), 1, 0),
        row(width, 3, 3, 2, (OrderSideV2::Buy, 0, 1), 2, 2),
    ];
    let manifest_counts = [0_u32, 1, 2];
    let cursor_len = runtime_verifier_len_v2(width).expect("verifier length");
    let verified_len = verified_candidate_len(width).expect("verified length");
    let zero_verified = vec![0; verified_len];
    let mut cursor = vec![0; cursor_len];
    let mut verified = zero_verified.clone();
    let mut manifests = Vec::new();

    for (index, (row, terms)) in rows.iter().enumerate() {
        let page_coordinate = u32::try_from(index).expect("page index") + 1;
        let mut page = vec![0; page_len(width, 1).expect("page length")];
        PageV2::encode_into(
            PageHeaderV2 {
                outcome_count: width,
                page_coordinate,
                page_count: 3,
                revision: 11 + u64::try_from(index).expect("page revision"),
                candidate_id: CANDIDATE,
            },
            &[row],
            &mut page,
        )
        .expect("page");
        let mut cursor_scratch = vec![0; cursor_len];
        let mut cursor_output = vec![0xa5; cursor_len];
        let mut verified_scratch = vec![0; verified_len];
        let mut verified_output = zero_verified.clone();
        let manifest_len =
            settlement_manifest_len_v2(width, manifest_counts[index]).expect("manifest length");
        let mut manifest_scratch = vec![0; manifest_len];
        let mut manifest_output = vec![0xa5; manifest_len];
        let summary = evaluate_runtime_consider_row_with_manifest_v2(
            RuntimeConsiderRowViewV2 {
                candidate: &candidate,
                page: &page,
                cursor_before: &cursor,
                verified_before: &zero_verified,
                authenticated_order: *terms,
                expected_page_index: u32::try_from(index).expect("page index"),
                expected_row_index: 0,
                expected_page_revision: 11 + u64::try_from(index).expect("page revision"),
                expected_revision: u64::try_from(index).expect("revision"),
                max_orders: 3,
            },
            RuntimeConsiderRowBuffersV2 {
                cursor_scratch: &mut cursor_scratch,
                cursor_output: &mut cursor_output,
                verified_scratch: &mut verified_scratch,
                verified_output: &mut verified_output,
            },
            RuntimeManifestBuffersV2 {
                manifest_scratch: &mut manifest_scratch,
                manifest_output: &mut manifest_output,
            },
        )
        .expect("verified row");
        assert_eq!(summary.complete, index == 2);
        cursor = cursor_output;
        if manifest_counts[index] != 0 {
            manifests.push(manifest_output);
        }
        if summary.complete {
            verified = verified_output;
        }
    }
    assert_eq!(manifests.len(), 2);
    assert_eq!(
        SettlementManifestV2::decode(&manifests[0])
            .expect("first manifest")
            .header()
            .order_count,
        1
    );
    assert_eq!(
        SettlementManifestV2::decode(&manifests[1])
            .expect("final manifest")
            .header()
            .order_count,
        2
    );
    TerminalFixture {
        width,
        verifier: cursor,
        verified,
        manifests,
    }
}

fn initialized_cursor(fixture: &TerminalFixture) -> Vec<u8> {
    let cursor_len = settlement_cursor_len(fixture.width).expect("cursor length");
    let mut inventory_scratch =
        vec![0; usize::try_from(fixture.width).expect("inventory width") * 8];
    let mut cursor_scratch = vec![0; cursor_len];
    let mut cursor_output = vec![0; cursor_len];
    initialize_runtime_settlement_v2(
        &fixture.verifier,
        &fixture.verified,
        0,
        &mut inventory_scratch,
        &mut cursor_scratch,
        &mut cursor_output,
    )
    .expect("initialize settlement");
    cursor_output
}

#[test]
fn in_place_initialization_matches_three_bank_contract_at_runtime_widths() {
    for width in [1_u32, 258] {
        let fixture = terminal_fixture(width);
        let expected = initialized_cursor(&fixture);
        let mut output = vec![0_u8; expected.len()];
        initialize_runtime_settlement_in_place_v2(
            &fixture.verifier,
            &fixture.verified,
            0,
            &mut output,
        )
        .expect("in-place initialization");
        assert_eq!(output, expected);
    }
}

fn settle(
    fixture: &TerminalFixture,
    cursor: &[u8],
    action: RuntimeSettlementActionV2,
    manifest: Option<&[u8]>,
    manifest_order_index: u32,
) -> (Vec<u8>, Vec<u8>) {
    let cursor_value = SettlementCursorV2::decode(cursor).expect("cursor");
    let cursor_len = cursor.len();
    let effect_len = runtime_settlement_effect_len_v2(fixture.width).expect("effect length");
    let mut cursor_scratch = vec![0; cursor_len];
    let mut cursor_output = vec![0xa5; cursor_len];
    let mut inventory_scratch =
        vec![0; usize::try_from(fixture.width).expect("inventory width") * 8];
    let mut effect_scratch = vec![0; effect_len];
    let mut effect_output = vec![0xa5; effect_len];
    evaluate_runtime_settlement_v2(
        RuntimeSettlementViewV2 {
            action,
            cursor_before: cursor,
            verified: &fixture.verified,
            manifest,
            manifest_order_index,
            expected_revision: cursor_value.header().revision,
            surplus_beneficiary: (action == RuntimeSettlementActionV2::Close)
                .then_some(BENEFICIARY),
        },
        RuntimeSettlementBuffersV2 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_output,
            inventory_scratch: &mut inventory_scratch,
            effect_scratch: &mut effect_scratch,
            effect_output: &mut effect_output,
        },
    )
    .expect("settlement action");
    (cursor_output, effect_output)
}

#[test]
fn hostile_n16_runs_collect_materialize_distribute_and_terminal_across_chunks() {
    let fixture = terminal_fixture(16);
    let first_manifest =
        SettlementManifestV2::decode(&fixture.manifests[0]).expect("first manifest");
    let final_manifest =
        SettlementManifestV2::decode(&fixture.manifests[1]).expect("final manifest");
    let rows = [
        (first_manifest.as_bytes(), 0),
        (final_manifest.as_bytes(), 0),
        (final_manifest.as_bytes(), 1),
    ];
    let mut cursor = initialized_cursor(&fixture);
    let initial = SettlementCursorV2::decode(&cursor).expect("initial cursor");
    assert_eq!(initial.header().order_count, 3);
    assert_eq!(initial.header().phase, SettlementPhaseV2::Collecting);

    for (coordinate, (manifest, index)) in rows.iter().enumerate() {
        let (next, effect) = settle(
            &fixture,
            &cursor,
            RuntimeSettlementActionV2::Collect,
            Some(manifest),
            *index,
        );
        let plan = RuntimeSettlementEffectPlanV2::decode(&effect).expect("collect effect");
        assert_eq!(
            plan.header().order_coordinate,
            u32::try_from(coordinate).expect("order coordinate") + 1
        );
        cursor = next;
    }
    let collected = SettlementCursorV2::decode(&cursor).expect("collected cursor");
    assert_eq!(collected.header().phase, SettlementPhaseV2::Materializing);
    assert_eq!(collected.header().quote_inventory, 4);
    // The sole seller delivered one claim at outcome zero and nothing anywhere
    // else: the collected inventory is the clearing's own concentration.
    assert_eq!(collected.inventory(0).expect("inventory"), 1);
    assert!((1..16).all(|outcome| collected.inventory(outcome).expect("inventory") == 0));

    let (next, effect) = settle(
        &fixture,
        &cursor,
        RuntimeSettlementActionV2::Materialize,
        None,
        0,
    );
    let plan = RuntimeSettlementEffectPlanV2::decode(&effect).expect("materialize effect");
    assert_eq!(
        plan.header().complete_set_move,
        RuntimeCompleteSetMoveV2::Mint
    );
    assert_eq!(plan.header().complete_set_quantity, 3);
    assert!((0..16).all(|outcome| plan.quantity(outcome).expect("effect quantity") == 3));

    let bank_len = general_settlement_candidate_bank_len_v2(16).expect("candidate bank");
    let mut bank_scratch = vec![0; bank_len];
    let mut bank_output = vec![0xa5; bank_len];
    let candidate =
        project_general_settlement_candidate_v2(&effect, 16, &mut bank_scratch, &mut bank_output)
            .expect("Strategy candidate");
    assert!(matches!(
        candidate,
        dclutch_market::execution_strategy::v2::ExecutionCandidateV2::Accepted(_)
    ));
    let candidate_bytes = match candidate {
        dclutch_market::execution_strategy::v2::ExecutionCandidateV2::Accepted(bytes) => bytes,
        dclutch_market::execution_strategy::v2::ExecutionCandidateV2::Refused => &[],
    };
    assert_eq!(
        read_u64(
            candidate_bytes,
            usize::try_from(GENERAL_SETTLEMENT_MOVE_SCALAR_V2).expect("coordinate") * 8,
        )
        .expect("move register"),
        1
    );
    let first_quantity =
        usize::try_from(GENERAL_SETTLEMENT_COMMON_SCALARS_V2).expect("quantity coordinate") * 8;
    assert_eq!(
        read_u64(candidate_bytes, first_quantity).expect("quantity"),
        3
    );
    let scalar_count = general_settlement_scalar_count_v2(16).expect("scalar count");
    let beneficiary_offset = usize::try_from(scalar_count).expect("scalar count") * 8
        + usize::try_from(GENERAL_SETTLEMENT_BENEFICIARY_IDENTITY_V2).expect("identity coordinate")
            * 32;
    assert_eq!(
        candidate_bytes
            .get(beneficiary_offset..beneficiary_offset + 32)
            .expect("beneficiary register"),
        [0_u8; 32]
    );
    assert_eq!(GENERAL_SETTLEMENT_ITEM_SCALAR_STRIDE_V2, 1);
    assert_eq!(GENERAL_SETTLEMENT_ITEM_IDENTITY_STRIDE_V2, 0);
    assert_eq!(GENERAL_SETTLEMENT_COMMON_IDENTITIES_V2, 4);
    cursor = next;
    let materialized = SettlementCursorV2::decode(&cursor).expect("materialized cursor");
    assert_eq!(materialized.header().quote_inventory, 1);
    // A mint produces three of EVERY outcome, on top of the one claim the
    // seller delivered at outcome zero.
    assert_eq!(materialized.inventory(0).expect("inventory"), 4);
    assert!((1..16).all(|outcome| materialized.inventory(outcome).expect("inventory") == 3));

    for (manifest, index) in rows {
        (cursor, _) = settle(
            &fixture,
            &cursor,
            RuntimeSettlementActionV2::Distribute,
            Some(manifest),
            index,
        );
    }
    let ready = SettlementCursorV2::decode(&cursor).expect("ready cursor");
    assert_eq!(ready.header().phase, SettlementPhaseV2::ReadyToClose);
    assert_eq!(ready.header().quote_inventory, 0);
    // THE RESIDUAL SURVIVES DISTRIBUTION. The two buyers were paid the four
    // claims the clearing priced at outcome zero; the three complete sets'
    // worth at every other outcome is `M - net_i`, which no maker bought.
    assert_eq!(ready.inventory(0).expect("inventory"), 0);
    assert!((1..16).all(|outcome| ready.inventory(outcome).expect("inventory") == 3));
    let terminal_coordinate = ready
        .header()
        .revision
        .checked_add(1)
        .expect("terminal successor revision");

    let (terminal_bytes, effect) =
        settle(&fixture, &cursor, RuntimeSettlementActionV2::Close, None, 0);
    let close_effect = RuntimeSettlementEffectPlanV2::decode(&effect).expect("close effect");
    assert!(close_effect.header().terminal);
    // The close no longer REFUSES a leftover claim inventory: it requires the
    // inventory to be exactly the certificate's residual at every outcome and
    // STRANDS it, which is what `claims_active` on the close effect says.
    assert!(close_effect.header().claims_active);
    assert_eq!(close_effect.header().beneficiary, BENEFICIARY);
    assert_eq!(
        close_effect.header().terminal_coordinate,
        terminal_coordinate
    );
    let terminal = SettlementCursorV2::decode(&terminal_bytes).expect("terminal cursor");
    assert_eq!(terminal.header().phase, SettlementPhaseV2::Terminal);
    assert_eq!(terminal.header().terminal_coordinate, terminal_coordinate);
}

#[test]
fn substituted_order_early_close_and_nonexact_banks_preserve_outputs() {
    let fixture = terminal_fixture(16);
    let cursor = initialized_cursor(&fixture);
    let cursor_len = cursor.len();
    let effect_len = runtime_settlement_effect_len_v2(16).expect("effect length");
    let mut substituted = fixture.manifests[0].clone();
    substituted[32..64].fill(8);
    substituted[96..128].fill(8);
    SettlementManifestV2::decode(&substituted).expect("valid alternate manifest");

    for (action, manifest, effect_delta) in [
        (
            RuntimeSettlementActionV2::Collect,
            Some(substituted.as_slice()),
            0_isize,
        ),
        (RuntimeSettlementActionV2::Close, None, 0),
        (
            RuntimeSettlementActionV2::Collect,
            Some(fixture.manifests[0].as_slice()),
            -1,
        ),
        (
            RuntimeSettlementActionV2::Collect,
            Some(fixture.manifests[0].as_slice()),
            1,
        ),
    ] {
        let mut cursor_scratch = vec![0; cursor_len];
        let mut cursor_output = vec![0x5a; cursor_len];
        let before_cursor_output = cursor_output.clone();
        let mut inventory_scratch = vec![0; 16 * 8];
        let adjusted = usize::try_from(
            isize::try_from(effect_len).expect("effect length fits") + effect_delta,
        )
        .expect("adjusted effect length");
        let mut effect_scratch = vec![0; adjusted];
        let mut effect_output = vec![0x5a; adjusted];
        let before_effect_output = effect_output.clone();
        let result = evaluate_runtime_settlement_v2(
            RuntimeSettlementViewV2 {
                action,
                cursor_before: &cursor,
                verified: &fixture.verified,
                manifest,
                manifest_order_index: 0,
                expected_revision: 1,
                surplus_beneficiary: (action == RuntimeSettlementActionV2::Close)
                    .then_some(BENEFICIARY),
            },
            RuntimeSettlementBuffersV2 {
                cursor_scratch: &mut cursor_scratch,
                cursor_output: &mut cursor_output,
                inventory_scratch: &mut inventory_scratch,
                effect_scratch: &mut effect_scratch,
                effect_output: &mut effect_output,
            },
        );
        assert!(result.is_err());
        assert_eq!(cursor_output, before_cursor_output);
        assert_eq!(effect_output, before_effect_output);
    }

    let (_, materialize_effect) = {
        let first_manifest =
            SettlementManifestV2::decode(&fixture.manifests[0]).expect("first manifest");
        let final_manifest =
            SettlementManifestV2::decode(&fixture.manifests[1]).expect("final manifest");
        let mut collected = cursor.clone();
        for (manifest, index) in [
            (first_manifest.as_bytes(), 0),
            (final_manifest.as_bytes(), 0),
            (final_manifest.as_bytes(), 1),
        ] {
            (collected, _) = settle(
                &fixture,
                &collected,
                RuntimeSettlementActionV2::Collect,
                Some(manifest),
                index,
            );
        }
        settle(
            &fixture,
            &collected,
            RuntimeSettlementActionV2::Materialize,
            None,
            0,
        )
    };
    let exact = general_settlement_candidate_bank_len_v2(16).expect("candidate bank");
    for delta in [-1_isize, 1] {
        let adjusted = usize::try_from(isize::try_from(exact).expect("bank length fits") + delta)
            .expect("adjusted bank");
        let mut bank_scratch = vec![0; adjusted];
        let mut bank_output = vec![0x5a; adjusted];
        let before = bank_output.clone();
        assert!(
            project_general_settlement_candidate_v2(
                &materialize_effect,
                16,
                &mut bank_scratch,
                &mut bank_output,
            )
            .is_err()
        );
        assert_eq!(bank_output, before);
    }
    let mut bank_scratch = vec![0; exact];
    let mut bank_output = vec![0x5a; exact];
    let before = bank_output.clone();
    assert!(
        project_general_settlement_candidate_v2(
            &materialize_effect,
            15,
            &mut bank_scratch,
            &mut bank_output,
        )
        .is_err()
    );
    assert_eq!(bank_output, before);
}
