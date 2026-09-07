//! The General settlement tier, built the way the protocol builds it.
//!
//! # Why this is not a stub
//!
//! The first executed campaign drove all seven General actions and only the two
//! selection actions came back accepted. The other five returned typed
//! refusals, and the reason was honest but unsatisfying: the campaign supplied
//! only a config, a Product record and a selection cursor, so every settlement
//! action found `DUMMY` where its evidence belonged.
//!
//! The missing evidence cannot be fabricated. A settlement action reads a
//! **runtime verifier** — a cursor produced by running the protocol's own
//! verification verb over candidate rows, each row authenticated against the
//! escrowed order record it names — and a **settlement manifest** those same
//! verifications emit. Neither has a constructor that takes a shape; both are
//! outputs of a real collection half. So this module runs the collection half:
//! it opens a batch against a root, admits three signed portfolio orders into
//! it, closes it, addresses a candidate by its own digest, funds a submission,
//! and verifies each row through `verify_candidate_row_v1`.
//!
//! Every artifact the campaign then puts on chain is an output of that, not a
//! literal. That is the difference between a fixture that proves the
//! accelerator agrees with the protocol and one that proves the accelerator
//! agrees with whatever bytes the test happened to type.
//!
//! # Lifted, not reinvented
//!
//! The construction mirrors `terminal_fixture` in
//! `programs/dclutch-general-accelerator-sbf/program-test/tests/lifecycle.rs`,
//! which is the ProgramTest tier's ground truth for the same seven actions. The
//! deliberate differences are two:
//!
//! - the host-side escrow ledger is dropped. It exists there to assert that
//!   lamports and quote atoms are conserved across the collection half, and
//!   those are assertions about the fixture, not inputs to the accelerator. The
//!   authentications that *do* constrain the artifacts — the candidate is its
//!   own digest, the candidate authenticates against the closed batch, each row
//!   authenticates against its order record — are all kept.
//! - the settlement chain is driven natively here to produce the *input* state
//!   for each on-chain step, exactly as the ProgramTest does. The chain is a
//!   sequence with an evolving cursor, not five independent shots, and a caller
//!   that treated it as five would be refused at the second one.

use dclutch_trading::general::{
    candidate_v1::{
        CandidateVerifyRowBuffersV1, CandidateVerifyRowViewV1, GeneralCandidateOpeningV1,
        GeneralCandidateV1, authenticate_candidate_identity_v1, general_candidate_identity_v1,
        verify_candidate_row_v1,
    },
    collection_v1::{
        GeneralBatchOpeningV1, GeneralBatchV1, GeneralOrderHeaderV1, GeneralOrderPhaseV1,
        GeneralOrderStateV1, GeneralOrderV1, MakerFundingV1, authenticate_batch_candidate_v1,
        authenticate_order_execution_v1, general_order_len_v1,
    },
    runtime_manifest::{SettlementManifestV2, settlement_manifest_len_v2},
    runtime_selection::{RUNTIME_SELECTION_CURSOR_BYTES_V2, freeze_selection_v2},
    runtime_settlement::{
        RuntimeSettlementActionV2, RuntimeSettlementBuffersV2, RuntimeSettlementViewV2,
        evaluate_runtime_settlement_v2, initialize_runtime_settlement_v2,
        runtime_settlement_effect_len_v2,
    },
    runtime_verify::runtime_verifier_len_v2,
    runtime_width::{
        CandidateHeaderV2, CandidateV2, ExecutionHeaderV2, ExecutionV2, PageHeaderV2, PageV2,
        SettlementCursorV2, candidate_len, execution_len, page_len, settlement_cursor_len,
        verified_candidate_len,
    },
};
use dclutch_trading::general_config::root::GeneralRootV2;

use crate::{Error, Result};

/// Market identity the fixture's root is bound to.
pub(crate) const FIXTURE_MARKET_V1: [u8; 32] = [0xb2; 32];
/// Config identity the root and batch agree on.
pub(crate) const FIXTURE_CONFIG_IDENTITY_V1: [u8; 32] = [0xb6; 32];
/// Generation the root is active at.
pub(crate) const FIXTURE_GENERATION_V1: u64 = 7;
/// Slot at which orders are admitted and the batch opened.
pub(crate) const FIXTURE_ADMISSION_SLOT_V1: u64 = 10;
/// Slot at which collection closes.
pub(crate) const FIXTURE_COLLECTION_CLOSE_SLOT_V1: u64 = 1_000;
/// Slot at which settlement closes, and each order's validity horizon.
pub(crate) const FIXTURE_SETTLEMENT_CLOSE_SLOT_V1: u64 = 2_000;
/// Slot at which the candidate is submitted.
pub(crate) const FIXTURE_SUBMISSION_SLOT_V1: u64 = 1_100;
/// Maximum orders the batch admits.
pub(crate) const FIXTURE_BATCH_MAX_ORDERS_V1: u32 = 8;
/// Revision the submission pins its candidate pages at.
pub(crate) const FIXTURE_CANDIDATE_PAGE_REVISION_V1: u64 = 11;
/// Lamports paid per verification crank.
pub(crate) const FIXTURE_CRANK_REWARD_LAMPORTS_V1: u64 = 5_000;
/// The one maker every order in the fixture belongs to.
pub(crate) const FIXTURE_OWNER_V1: [u8; 32] = [0xc1; 32];
/// The solver funding the candidate submission.
pub(crate) const FIXTURE_SOLVER_V1: [u8; 32] = [0xc3; 32];
/// Beneficiary of any quote surplus at terminal close.
pub(crate) const FIXTURE_BENEFICIARY_V1: [u8; 32] = [0xc2; 32];
/// The candidate identity drafted before the digest replaces it.
const FIXTURE_DRAFT_CANDIDATE_V1: [u8; 32] = [0xb5; 32];
/// Number of order rows the candidate carries.
const FIXTURE_ROW_COUNT_V1: u32 = 3;

/// The complete settlement-tier fixture for one runtime width.
pub(crate) struct GeneralTerminalFixtureV1 {
    /// Runtime width every artifact is encoded at.
    pub(crate) width: u32,
    /// The verifier cursor the three row verifications produced.
    pub(crate) verifier: Vec<u8>,
    /// The verified-candidate certificate those verifications minted.
    pub(crate) verified: Vec<u8>,
    /// The settlement manifests emitted along the way.
    pub(crate) manifests: Vec<Vec<u8>>,
    /// The candidate's own content identity.
    pub(crate) candidate_id: [u8; 32],
}

/// One maker's signed portfolio order.
struct OrderSpecV1 {
    nonce: u64,
    lots: u64,
    receive: Vec<u64>,
    deliver: Vec<u64>,
    debit_limit: u64,
}

/// The batch opening every order and candidate is bound to.
fn fixture_batch_opening_v1(width: u32, product_id: [u8; 32]) -> GeneralBatchOpeningV1 {
    GeneralBatchOpeningV1 {
        outcome_count: width,
        sequence: 0,
        generation: FIXTURE_GENERATION_V1,
        market: FIXTURE_MARKET_V1,
        product_id,
        config_id: FIXTURE_CONFIG_IDENTITY_V1,
        price_scale: u64::from(width),
        collection_close_slot: FIXTURE_COLLECTION_CLOSE_SLOT_V1,
        settlement_close_slot: FIXTURE_SETTLEMENT_CLOSE_SLOT_V1,
        max_orders: FIXTURE_BATCH_MAX_ORDERS_V1,
    }
}

/// Open one real batch against one real active root.
fn opened_batch_v1(width: u32, product_id: [u8; 32]) -> Result<(GeneralRootV2, GeneralBatchV1)> {
    let mut root = GeneralRootV2::active(
        FIXTURE_MARKET_V1,
        FIXTURE_CONFIG_IDENTITY_V1,
        FIXTURE_GENERATION_V1,
    )
    .map_err(|error| Error::new(format!("active General root: {error:?}")))?;
    let revision = root.revision();
    let batch = GeneralBatchV1::open(
        &mut root,
        fixture_batch_opening_v1(width, product_id),
        revision,
        FIXTURE_ADMISSION_SLOT_V1,
    )
    .map_err(|error| Error::new(format!("open batch: {error:?}")))?;
    Ok((root, batch))
}

/// Encode one order record as it would be placed into the batch.
fn order_record_v1(width: u32, batch_id: [u8; 32], spec: &OrderSpecV1) -> Result<Vec<u8>> {
    let mut bytes = vec![
        0_u8;
        general_order_len_v1(width)
            .map_err(|error| Error::new(format!("order width: {error:?}")))?
    ];
    GeneralOrderV1::encode_into(
        GeneralOrderHeaderV1 {
            outcome_count: width,
            nonce: spec.nonce,
            owner_id: FIXTURE_OWNER_V1,
            market: FIXTURE_MARKET_V1,
            batch_id,
            generation: FIXTURE_GENERATION_V1,
            max_lots: 10,
            max_quote_debit_per_lot: spec.debit_limit,
            // The seller's floor, zero in this fixture: see the same note in
            // the accelerator program-test's `order_record`.
            min_quote_credit_per_lot: 0,
            valid_until_slot: FIXTURE_SETTLEMENT_CLOSE_SLOT_V1,
        },
        &spec.receive,
        &spec.deliver,
        GeneralOrderStateV1 {
            phase: GeneralOrderPhaseV1::Placed,
            admitted_slot: FIXTURE_ADMISSION_SLOT_V1,
            released_slot: 0,
        },
        &mut bytes,
    )
    .map_err(|error| Error::new(format!("order record: {error:?}")))?;
    Ok(bytes)
}

/// Build one compact Execution row from the immutable order it names.
///
/// The row is authenticated as a whole record, tails included: the per-lot
/// vectors are part of what the order record binds, so a header-only check
/// could not see a substituted portfolio.
fn execution_row_v1(
    width: u32,
    page_coordinate: u32,
    batch: GeneralBatchV1,
    order_bytes: &[u8],
    lots: u64,
) -> Result<Vec<u8>> {
    let order = GeneralOrderV1::decode(order_bytes)
        .map_err(|error| Error::new(format!("order record: {error:?}")))?;
    let header = order.header();
    let mut receive = Vec::with_capacity(usize::try_from(width).unwrap_or_default());
    let mut deliver = Vec::with_capacity(usize::try_from(width).unwrap_or_default());
    for index in 0..width {
        receive.push(
            order
                .receive_per_lot(index)
                .map_err(|error| Error::new(format!("receive per lot: {error:?}")))?,
        );
        deliver.push(
            order
                .deliver_per_lot(index)
                .map_err(|error| Error::new(format!("deliver per lot: {error:?}")))?,
        );
    }
    let mut bytes = vec![
        0_u8;
        execution_len(width)
            .map_err(|error| Error::new(format!("execution width: {error:?}")))?
    ];
    ExecutionV2::encode_into(
        ExecutionHeaderV2 {
            outcome_count: width,
            page_coordinate,
            execution_coordinate: 1,
            nonce: header.nonce,
            order_id: order.order_id(),
            owner_id: header.owner_id,
            max_lots: header.max_lots,
            lots,
        },
        &receive,
        &deliver,
        &mut bytes,
    )
    .map_err(|error| Error::new(format!("execution row: {error:?}")))?;
    authenticate_order_execution_v1(
        batch,
        order,
        ExecutionV2::decode(&bytes)
            .map_err(|error| Error::new(format!("row decode: {error:?}")))?,
    )
    .map_err(|error| {
        Error::new(format!(
            "row does not authenticate against its order: {error:?}"
        ))
    })?;
    Ok(bytes)
}

/// Run the whole collection half and return what settlement reads.
pub(crate) fn terminal_fixture_v1(
    width: u32,
    product_id: [u8; 32],
) -> Result<GeneralTerminalFixtureV1> {
    let count = usize::try_from(width).map_err(|_| Error::new("runtime width"))?;
    let ones = vec![1_u64; count];
    let zeros = vec![0_u64; count];

    let (mut root, mut batch) = opened_batch_v1(width, product_id)?;
    let specs = [
        OrderSpecV1 {
            nonce: 1,
            lots: 2,
            receive: ones.clone(),
            deliver: zeros.clone(),
            debit_limit: 2,
        },
        OrderSpecV1 {
            nonce: 2,
            lots: 1,
            receive: zeros.clone(),
            deliver: ones.clone(),
            debit_limit: 0,
        },
        OrderSpecV1 {
            nonce: 3,
            lots: 2,
            receive: ones.clone(),
            deliver: zeros,
            debit_limit: 2,
        },
    ];
    let identity = batch.batch_id();
    let claims = vec![u64::MAX / 4; count];
    let mut placed: Vec<(Vec<u8>, u64)> = Vec::new();
    for spec in &specs {
        let bytes = order_record_v1(width, identity, spec)?;
        let order = GeneralOrderV1::decode(&bytes)
            .map_err(|error| Error::new(format!("order record: {error:?}")))?;
        batch
            .admit(
                order,
                MakerFundingV1 {
                    owner_id: FIXTURE_OWNER_V1,
                    available_quote: u64::MAX / 4,
                    available_claims: &claims,
                },
                FIXTURE_ADMISSION_SLOT_V1,
            )
            .map_err(|error| Error::new(format!("admit order: {error:?}")))?;
        placed.push((bytes, spec.lots));
    }
    let revision = root.revision();
    let closed = batch
        .close(&mut root, revision)
        .map_err(|error| Error::new(format!("close batch: {error:?}")))?;
    if closed != identity {
        return Err(Error::new("closing the batch changed its identity"));
    }

    // Candidate rows are globally grouped by increasing order identity, and the
    // verifier reads a 32-byte identity as a LITTLE-ENDIAN 256-bit integer,
    // which is not the lexicographic order of `[u8; 32]`. Sorting the other way
    // refuses with `NonCanonicalOrder`, and a fixture whose identities happened
    // to have zero high bytes could not tell the two apart.
    let mut sort_error = None;
    placed.sort_by(|left, right| {
        let left_id = GeneralOrderV1::decode(&left.0).map(|order| order.order_id());
        let right_id = GeneralOrderV1::decode(&right.0).map(|order| order.order_id());
        match (left_id, right_id) {
            (Ok(left_id), Ok(right_id)) => left_id.iter().rev().cmp(right_id.iter().rev()),
            _ => {
                sort_error = Some("an admitted order stopped decoding");
                core::cmp::Ordering::Equal
            }
        }
    });
    if let Some(message) = sort_error {
        return Err(Error::new(message));
    }

    // The candidate carries its OWN digest as its identity, and `CandidateV2`
    // checks nothing about that field. Encode once to fix every other byte,
    // then re-encode with the digest those bytes produce.
    let mut candidate = vec![
        0_u8;
        candidate_len(width).map_err(|error| Error::new(format!(
            "candidate width: {error:?}"
        )))?
    ];
    let header = CandidateHeaderV2 {
        outcome_count: width,
        page_count: FIXTURE_ROW_COUNT_V1,
        candidate_coordinate: 2,
        price_scale: u64::from(width),
        candidate_id: FIXTURE_DRAFT_CANDIDATE_V1,
        product_id,
        batch_id: identity,
    };
    CandidateV2::encode_into(header, &ones, &mut candidate)
        .map_err(|error| Error::new(format!("draft candidate: {error:?}")))?;
    let candidate_id = general_candidate_identity_v1(&candidate)
        .map_err(|error| Error::new(format!("candidate identity: {error:?}")))?;
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            candidate_id,
            ..header
        },
        &ones,
        &mut candidate,
    )
    .map_err(|error| Error::new(format!("addressed candidate: {error:?}")))?;
    let decoded_candidate = CandidateV2::decode(&candidate)
        .map_err(|error| Error::new(format!("candidate decode: {error:?}")))?;
    authenticate_candidate_identity_v1(decoded_candidate)
        .map_err(|error| Error::new(format!("candidate is not its own digest: {error:?}")))?;
    authenticate_batch_candidate_v1(batch, decoded_candidate.header()).map_err(|error| {
        Error::new(format!(
            "candidate does not authenticate against the closed batch: {error:?}"
        ))
    })?;

    let opening = GeneralCandidateOpeningV1 {
        outcome_count: width,
        page_count: FIXTURE_ROW_COUNT_V1,
        page_revision: FIXTURE_CANDIDATE_PAGE_REVISION_V1,
        submitted_slot: FIXTURE_SUBMISSION_SLOT_V1,
        candidate_id,
        batch_id: identity,
        solver_id: FIXTURE_SOLVER_V1,
        row_count: FIXTURE_ROW_COUNT_V1,
        reward_rate_lamports: FIXTURE_CRANK_REWARD_LAMPORTS_V1,
    };
    let mut submission = GeneralCandidateV1::submit(
        batch,
        decoded_candidate,
        FIXTURE_CANDIDATE_PAGE_REVISION_V1,
        FIXTURE_ROW_COUNT_V1,
        FIXTURE_CRANK_REWARD_LAMPORTS_V1,
        FIXTURE_SOLVER_V1,
        opening
            .work_capacity()
            .map_err(|error| Error::new(format!("work capacity: {error:?}")))?,
        FIXTURE_SUBMISSION_SLOT_V1,
    )
    .map_err(|error| Error::new(format!("submit candidate: {error:?}")))?;

    // Each page is deliberately unbalanced; only the complete candidate has the
    // uniform relation a complete-set materialization requires.
    let manifest_counts = [0_u32, 1, 2];
    let cursor_len = runtime_verifier_len_v2(width)
        .map_err(|error| Error::new(format!("verifier width: {error:?}")))?;
    let verified_len = verified_candidate_len(width)
        .map_err(|error| Error::new(format!("verified width: {error:?}")))?;
    let zero_verified = vec![0_u8; verified_len];
    let mut cursor = vec![0_u8; cursor_len];
    let mut verified = zero_verified.clone();
    let mut manifests = Vec::new();
    for index in 0..usize::try_from(FIXTURE_ROW_COUNT_V1).unwrap_or_default() {
        let (order_bytes, lots) = placed
            .get(index)
            .ok_or_else(|| Error::new("placed order"))?;
        let page_coordinate = u32::try_from(index)
            .map_err(|_| Error::new("page coordinate"))?
            .checked_add(1)
            .ok_or_else(|| Error::new("page coordinate overflow"))?;
        let row = execution_row_v1(width, page_coordinate, batch, order_bytes, *lots)?;
        let mut page = vec![
            0_u8;
            page_len(width, 1)
                .map_err(|error| Error::new(format!("page width: {error:?}")))?
        ];
        PageV2::encode_into(
            PageHeaderV2 {
                outcome_count: width,
                page_coordinate,
                page_count: FIXTURE_ROW_COUNT_V1,
                revision: FIXTURE_CANDIDATE_PAGE_REVISION_V1,
                candidate_id,
            },
            &[row.as_slice()],
            &mut page,
        )
        .map_err(|error| Error::new(format!("page: {error:?}")))?;

        let manifest_count = *manifest_counts
            .get(index)
            .ok_or_else(|| Error::new("manifest count"))?;
        let manifest_len = settlement_manifest_len_v2(width, manifest_count)
            .map_err(|error| Error::new(format!("manifest width: {error:?}")))?;
        let mut cursor_scratch = vec![0_u8; cursor_len];
        let mut cursor_output = vec![0xa5; cursor_len];
        let mut verified_scratch = vec![0_u8; verified_len];
        let mut verified_output = zero_verified.clone();
        let mut manifest_scratch = vec![0_u8; manifest_len];
        let mut manifest_output = vec![0xa5; manifest_len];
        // The protocol's own verification verb: it binds the page to this
        // submission's candidate at the pinned revision, authenticates the row
        // against the escrowed order record it names, and pays one crank.
        let summary = verify_candidate_row_v1(
            CandidateVerifyRowViewV1 {
                batch,
                submission,
                candidate: &candidate,
                page: &page,
                order: order_bytes,
                cursor_before: &cursor,
                verified_before: &zero_verified,
                expected_page_index: u32::try_from(index).map_err(|_| Error::new("page index"))?,
                expected_row_index: 0,
                expected_revision: u64::try_from(index).map_err(|_| Error::new("revision"))?,
            },
            CandidateVerifyRowBuffersV1 {
                cursor_scratch: &mut cursor_scratch,
                cursor_output: &mut cursor_output,
                verified_scratch: &mut verified_scratch,
                verified_output: &mut verified_output,
                manifest_scratch: &mut manifest_scratch,
                manifest_output: &mut manifest_output,
            },
        )
        .map_err(|error| Error::new(format!("verify candidate row {index}: {error:?}")))?;
        // The submission is consumed and returned by every crank: it carries
        // the work capacity each verification draws down. Not carrying it
        // forward makes row one refuse `Uncapitalized`, which is the escrow
        // accounting working exactly as designed.
        submission = summary.submission;
        cursor = cursor_output;
        if manifest_count != 0 {
            manifests.push(manifest_output);
        }
        if summary.complete {
            verified = verified_output;
        }
    }
    if manifests.len() != 2 {
        return Err(Error::new(format!(
            "the collection half emitted {} settlement manifests, not the two the chain reads",
            manifests.len()
        )));
    }
    if verified == zero_verified {
        return Err(Error::new(
            "no row verification completed the candidate, so there is no certificate to settle",
        ));
    }
    Ok(GeneralTerminalFixtureV1 {
        width,
        verifier: cursor,
        verified,
        manifests,
        candidate_id,
    })
}

/// The settlement cursor `InitializeSettlement` produces.
pub(crate) fn initialized_cursor_v1(fixture: &GeneralTerminalFixtureV1) -> Result<Vec<u8>> {
    let cursor_len = settlement_cursor_len(fixture.width)
        .map_err(|error| Error::new(format!("cursor width: {error:?}")))?;
    let inventory_len = usize::try_from(fixture.width)
        .map_err(|_| Error::new("runtime width"))?
        .checked_mul(8)
        .ok_or_else(|| Error::new("inventory width overflow"))?;
    let mut inventory = vec![0_u8; inventory_len];
    let mut scratch = vec![0_u8; cursor_len];
    let mut output = vec![0_u8; cursor_len];
    initialize_runtime_settlement_v2(
        &fixture.verifier,
        &fixture.verified,
        0,
        &mut inventory,
        &mut scratch,
        &mut output,
    )
    .map_err(|error| Error::new(format!("initialize settlement: {error:?}")))?;
    Ok(output)
}

/// Advance the settlement chain natively by one transition.
///
/// The host runs this to produce the *input* state each on-chain step reads.
/// The chain is a sequence: a caller that submitted Materialize against the
/// cursor Collect started from would be refused, correctly.
pub(crate) fn settle_native_v1(
    fixture: &GeneralTerminalFixtureV1,
    cursor: &[u8],
    action: RuntimeSettlementActionV2,
    manifest: Option<&[u8]>,
    manifest_order_index: u32,
) -> Result<Vec<u8>> {
    let cursor_value = SettlementCursorV2::decode(cursor)
        .map_err(|error| Error::new(format!("settlement cursor: {error:?}")))?;
    let effect_len = runtime_settlement_effect_len_v2(fixture.width)
        .map_err(|error| Error::new(format!("effect width: {error:?}")))?;
    let inventory_len = usize::try_from(fixture.width)
        .map_err(|_| Error::new("runtime width"))?
        .checked_mul(8)
        .ok_or_else(|| Error::new("inventory width overflow"))?;
    let mut cursor_scratch = vec![0_u8; cursor.len()];
    let mut cursor_output = vec![0xa5; cursor.len()];
    let mut inventory = vec![0_u8; inventory_len];
    let mut effect_scratch = vec![0_u8; effect_len];
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
                .then_some(FIXTURE_BENEFICIARY_V1),
        },
        RuntimeSettlementBuffersV2 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_output,
            inventory_scratch: &mut inventory,
            effect_scratch: &mut effect_scratch,
            effect_output: &mut effect_output,
        },
    )
    .map_err(|error| Error::new(format!("native {action:?} transition: {error:?}")))?;
    Ok(cursor_output)
}

/// The frozen selection `InitializeSettlement` reads.
pub(crate) fn frozen_selection_v1(
    opened: &[u8; RUNTIME_SELECTION_CURSOR_BYTES_V2],
) -> Result<[u8; RUNTIME_SELECTION_CURSOR_BYTES_V2]> {
    let mut scratch = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let mut frozen = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    freeze_selection_v2(opened, 1, &mut scratch, &mut frozen)
        .map_err(|error| Error::new(format!("frozen selection: {error:?}")))?;
    Ok(frozen)
}

/// One settlement row, as the manifest itself describes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SettlementRowV1 {
    /// Which of the fixture's manifests carries this row.
    pub(crate) manifest_index: usize,
    /// The row's ordinal within that manifest.
    pub(crate) manifest_order_index: u8,
    /// The candidate page the row originated on.
    pub(crate) page_index: u32,
    /// The execution coordinate within that page.
    pub(crate) execution_index: u8,
}

/// The three settlement rows, in the order the chain must consume them.
///
/// The ordinal and the source coordinates are DISTINCT authenticated facts.
/// Row zero's ordinal is zero and it originated on source page zero; deriving
/// one from the other — which an earlier one-based convention did — is refused
/// on chain without any runtime write. So both are read from the manifest
/// rather than computed here.
pub(crate) fn settlement_rows_v1(
    fixture: &GeneralTerminalFixtureV1,
) -> Result<Vec<SettlementRowV1>> {
    let sources = [(0_usize, 0_u8), (1, 0), (1, 1)];
    let mut rows = Vec::with_capacity(sources.len());
    for (manifest_index, manifest_order_index) in sources {
        let bytes = fixture
            .manifests
            .get(manifest_index)
            .ok_or_else(|| Error::new(format!("manifest {manifest_index} is absent")))?;
        let manifest = SettlementManifestV2::decode(bytes)
            .map_err(|error| Error::new(format!("manifest {manifest_index}: {error:?}")))?;
        let selected = manifest
            .order(u32::from(manifest_order_index))
            .map_err(|error| Error::new(format!("manifest row: {error:?}")))?;
        rows.push(SettlementRowV1 {
            manifest_index,
            manifest_order_index,
            page_index: selected.header().source_page_index,
            execution_index: u8::try_from(selected.header().source_execution_index)
                .map_err(|_| Error::new("source execution index does not fit the request"))?,
        });
    }
    Ok(rows)
}

/// The revision a settlement cursor is currently at.
pub(crate) fn settlement_revision_v1(cursor: &[u8]) -> Result<u64> {
    Ok(SettlementCursorV2::decode(cursor)
        .map_err(|error| Error::new(format!("settlement cursor: {error:?}")))?
        .header()
        .revision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn product_id() -> [u8; 32] {
        Sha256::digest(vec![0xb1_u8; 64]).into()
    }

    /// The collection half really runs, and its outputs are not empty shapes.
    #[test]
    fn the_collection_half_produces_a_verifier_a_certificate_and_two_manifests() {
        for width in [1_u32, 4] {
            let fixture = terminal_fixture_v1(width, product_id()).expect("fixture");
            assert_eq!(fixture.width, width);
            assert_eq!(fixture.manifests.len(), 2, "width {width}");
            assert_eq!(
                fixture.verifier.len(),
                runtime_verifier_len_v2(width).expect("verifier width")
            );
            assert_eq!(
                fixture.verified.len(),
                verified_candidate_len(width).expect("verified width")
            );
            // The candidate addressed itself; a zero identity would mean the
            // second encode never happened.
            assert_ne!(fixture.candidate_id, [0; 32]);
            assert_ne!(fixture.verified, vec![0_u8; fixture.verified.len()]);
        }
    }

    /// The settlement chain advances, and each step changes the revision.
    #[test]
    fn the_settlement_chain_advances_through_every_transition() {
        let fixture = terminal_fixture_v1(1, product_id()).expect("fixture");
        let mut cursor = initialized_cursor_v1(&fixture).expect("initialized cursor");
        let opening = settlement_revision_v1(&cursor).expect("revision");
        let manifests: Vec<&[u8]> = fixture.manifests.iter().map(Vec::as_slice).collect();
        // Three Collect rows: manifest zero row zero, then manifest one rows
        // zero and one. This ordering is the manifest's own, not a guess.
        let rows: [(usize, u32); 3] = [(0, 0), (1, 0), (1, 1)];
        for (manifest_index, order_index) in rows {
            cursor = settle_native_v1(
                &fixture,
                &cursor,
                RuntimeSettlementActionV2::Collect,
                Some(manifests[manifest_index]),
                order_index,
            )
            .expect("collect");
        }
        cursor = settle_native_v1(
            &fixture,
            &cursor,
            RuntimeSettlementActionV2::Materialize,
            None,
            0,
        )
        .expect("materialize");
        for (manifest_index, order_index) in rows {
            cursor = settle_native_v1(
                &fixture,
                &cursor,
                RuntimeSettlementActionV2::Distribute,
                Some(manifests[manifest_index]),
                order_index,
            )
            .expect("distribute");
        }
        cursor = settle_native_v1(&fixture, &cursor, RuntimeSettlementActionV2::Close, None, 0)
            .expect("close");
        assert!(settlement_revision_v1(&cursor).expect("revision") > opening);
    }
}

#[cfg(test)]
mod frozen_selection_tests {
    use super::*;
    use dclutch_trading::general::{
        runtime_selection::{RuntimeSelectionCursorV2, RuntimeSelectionPhaseV2},
        runtime_width::VerifiedCandidateV2,
    };
    use sha2::{Digest, Sha256};

    fn product_id() -> [u8; 32] {
        Sha256::digest(vec![0xb1_u8; 64]).into()
    }

    /// The frozen selection must satisfy the accelerator's own join, field by
    /// field. Written as separate assertions because a single boolean tells you
    /// only that something is wrong, and this join has fourteen conjuncts.
    #[test]
    fn the_frozen_selection_satisfies_every_conjunct_the_accelerator_checks() {
        let width = 1_u32;
        let product = product_id();
        let fixture = terminal_fixture_v1(width, product).expect("fixture");
        let policy_id = [0xb3_u8; 32];
        let price_scale = u64::from(width);

        let opened = crate::family_hot_campaign::selection_body_for_tests_v1(&fixture.verified)
            .expect("opened selection");
        let frozen_bytes = frozen_selection_v1(&opened).expect("frozen");

        let verified = VerifiedCandidateV2::decode(&fixture.verified).expect("verified");
        let vh = verified.header();
        let frozen = RuntimeSelectionCursorV2::decode(&frozen_bytes).expect("frozen cursor");
        let fh = frozen.header();
        let verified_digest: [u8; 32] = Sha256::digest(&fixture.verified).into();

        assert_eq!(fh.phase, RuntimeSelectionPhaseV2::Frozen, "phase");
        assert_eq!(fh.outcome_count, width, "frozen outcome_count");
        assert_eq!(fh.policy_id, policy_id, "policy_id");
        assert_eq!(fh.product_id, product, "frozen product_id");
        assert_eq!(fh.price_scale, price_scale, "frozen price_scale");
        assert_eq!(fh.best_candidate_id, vh.candidate_id, "best_candidate_id");
        assert_eq!(
            fh.best_candidate_coordinate, vh.candidate_coordinate,
            "best_candidate_coordinate"
        );
        assert_eq!(
            fh.best_verified_revision, vh.revision,
            "best_verified_revision"
        );
        assert_eq!(fh.product_id, vh.product_id, "product agreement");
        assert_eq!(fh.batch_id, vh.batch_id, "batch agreement");
        assert_eq!(
            fh.best_verified_digest, verified_digest,
            "best_verified_digest"
        );
        assert_eq!(vh.outcome_count, width, "verified outcome_count");
        assert_eq!(vh.product_id, product, "verified product_id");
        assert_eq!(vh.price_scale, price_scale, "verified price_scale");
    }
}
