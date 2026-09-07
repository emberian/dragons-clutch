//! General-specific construction over the family-neutral Hot bundle builder.
//!
//! This module owns no General arithmetic. It joins the current semantic owners
//! into one exact pre-executable request, PER ACTION:
//!
//! - [`GeneralRootV2`] supplies the active occurrence generation, revision and
//!   next sequence;
//! - [`GeneralConfigV3`] supplies the immutable price scale and order bound;
//! - [`GeneralBatchOccurrenceTermsV1`] supplies the slot-independent occurrence
//!   identity, while [`GeneralStateAddressSeedsV3`] supplies the published
//!   lifecycle seed order for its Batch PDA;
//! - for an action that names a state the chain already holds,
//!   [`GeneralLocalStateV3`] supplies the exact lifecycle envelope and its
//!   semantic body, decoded here rather than by the campaign. A campaign that
//!   spelled `data[HEADER..]` itself would be a second author for the physical
//!   envelope, which is exactly the debt the seam register forbids.
//!
//! The returned request is then consumed by [`build_general_action_bundle_v1`],
//! which selects the emitted descriptor through the published ProgramSet and
//! executes the ordinary admitted-AOT builder with that action's accelerator-
//! owned candidate projector. A campaign may supply semantic chain corpus, but
//! it cannot type a state identity, bump, accelerator request, caller
//! authority, span width, or account topology.
//!
//! ## Why per action rather than per call site
//!
//! Until 2026-09-04 this module was pinned to `OpenBatch` at two entry points,
//! and a General market could therefore execute exactly one action in any
//! harness. The fifteen actions differ in the STATE they name and in the
//! projector that owns their candidate, and in nothing else this module cares
//! about -- so the dispatch is two matches over [`Action`] and every other line
//! is shared. Actions this module does not derive yet refuse
//! [`BuilderError::UnsupportedRoute`] at a named line rather than being built
//! wrong.

use dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1;
use dclutch_market::execution_strategy::{decode_register_bank_into, encode_register_bank_into};
use dclutch_trading::general::{
    artifacts_v3::{GeneralDecodedRequestV3, GeneralRequestWireV3, decode_general_request_v3},
    candidate_v1::GeneralCandidateV1,
    candidate_v1::{CandidateVerifyRowViewV1, candidate_verifier_len_v1},
    collection_v1::{
        GeneralBatchOccurrenceTermsV1, GeneralBatchOpeningV1, GeneralBatchV1, GeneralOrderV1,
        GeneralSignedOrderTermsV1,
    },
    hot_candidate_v3::{
        GeneralHotCandidateErrorV3, GeneralHotEnvironmentV3,
        authenticate_general_close_candidate_v3, general_hot_candidate_bank_len_v3,
        general_hot_environment_from_bank_v3, project_general_cancel_order_candidate_in_place_v3,
        project_general_close_batch_candidate_in_place_v3,
        project_general_hot_candidate_in_place_v3,
        project_general_initialize_candidate_in_place_v3,
        project_general_open_batch_candidate_in_place_v3,
        project_general_place_order_candidate_in_place_v3,
        project_general_release_order_candidate_in_place_v3,
        project_general_selection_candidate_in_place_v3,
        project_general_submit_candidate_in_place_v3,
        project_general_verify_candidate_workspace_v3,
    },
    local_state_v3::{GeneralLocalStateKindV3, GeneralLocalStateV3},
    runtime_manifest::SettlementManifestV2,
    runtime_selection::{
        RUNTIME_SELECTION_CURSOR_BYTES_V2, RuntimeSelectionCursorV2,
        consider_verified_candidate_v2, freeze_selection_v2,
    },
    runtime_settlement::{
        RuntimeSettlementActionV2, RuntimeSettlementViewV2,
        evaluate_runtime_settlement_in_place_v2, initialize_runtime_settlement_in_place_v2,
        runtime_settlement_effect_len_v2,
    },
    runtime_verify::RuntimeCandidateVerifierV2,
    runtime_width::{CandidateV2, SettlementCursorV2, VerifiedCandidateV2, settlement_cursor_len},
    state_seeds_v3::{GeneralStateAddressSeedsV3, GeneralStateRecipeV3},
};
use dclutch_trading::general_codec::{Action, SelectionPolicyV1};
use dclutch_trading::general_config::{GeneralRootV2, v3::GeneralConfigV3};
use solana_program::pubkey::Pubkey;

use crate::{
    BuilderError,
    admitted::AdmittedAotInputV1,
    bundle::{BuiltAdmittedBundleV1, BundleInputV1, build_admitted_bundle_with_candidate_v1},
};

/// Chain-authenticated facts needed to derive one General action's request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralRequestInputV1<'a> {
    /// The action to derive. The ProgramSet remains the authority for the
    /// descriptor join; this is what the request will ASK for.
    pub action: Action,
    /// Exact current General root tail decoded from the composite root.
    pub root: GeneralRootV2,
    /// Address of that composite root, used by the lifecycle seed program.
    pub root_address: Pubkey,
    /// Exact selected config bytes.
    pub config: &'a [u8],
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
    /// Content identity carried in the selected Product record's own
    /// `product_id` field, which is what the AccountProfile projects into
    /// `identity::SELECTION_PRODUCT` and what the batch occurrence therefore
    /// commits to. It is NOT the finalized record digest.
    pub product_id: [u8; 32],
    /// Trading program that owns the root and every General state PDA.
    pub trading_program: Pubkey,
    /// EXACT ACCOUNT DATA of the primary state this action names, as the chain
    /// holds it, for an action that operates on a state that already exists.
    ///
    /// The whole account, not its body: the lifecycle envelope is decoded here
    /// so that one author states where a General state's semantic bytes begin.
    /// `None` for an action whose primary state this execution creates.
    pub primary_state_account: Option<&'a [u8]>,
    /// The authenticated records this action reads that are NOT its primary
    /// state.
    pub evidence: GeneralRequestEvidenceV1<'a>,
}

/// The authenticated records one action reads that are not its primary state.
///
/// Every field names a record the action's own published profile already
/// declares -- one of its `GeneralReadonlyEvidenceKindV3` rows, or the
/// secondary state its lifecycle shape carries -- so nothing here is a
/// coordinate the chain does not hold. Each arm of the derivation takes
/// exactly the fields its action names and refuses `Binding` at a named line
/// for one it does not have. `None` is not "may be omitted": for the two
/// records whose account is legitimately vacant before its first execution
/// (the selection cursor a first `Consider` creates, the verifier cursor a
/// first `VerifyCandidateRow` creates) `None` IS the vacancy, and the
/// coordinate it implies is stated at that arm.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GeneralRequestEvidenceV1<'a> {
    /// Exact signed immutable order terms (`PlaceOrder`).
    pub signed_order_terms: Option<&'a [u8]>,
    /// Live Order lifecycle account (`CancelOrder`).
    pub order_account: Option<&'a [u8]>,
    /// Exact immutable candidate image (`SubmitCandidate`).
    pub candidate_image: Option<&'a [u8]>,
    /// Exact VerifiedCandidate record (`Consider`, `InitializeSettlement`).
    pub verified_candidate: Option<&'a [u8]>,
    /// Live verifier-cursor lifecycle account (`VerifyCandidateRow`); `None`
    /// is the vacant cursor, which is that candidate's first row.
    pub verifier_account: Option<&'a [u8]>,
    /// Exact verifier-emitted settlement manifest (`Collect`, `Distribute`),
    /// and the manifest chunk `VerifyCandidateRow` is required to emit.
    pub settlement_manifest: Option<&'a [u8]>,
    /// Live Batch lifecycle account, for the four actions that read a closed
    /// batch as EVIDENCE rather than as their primary state
    /// (`SubmitCandidate`, `VerifyCandidateRow`, `CloseCandidate`).
    pub batch_account: Option<&'a [u8]>,
    /// Exact canonical submission record this execution writes
    /// (`SubmitCandidate`).
    pub submitted_candidate: Option<&'a [u8]>,
    /// Exact immutable candidate Page holding the next row
    /// (`VerifyCandidateRow`).
    pub candidate_page: Option<&'a [u8]>,
    /// Exact immutable selection-policy record (`Consider`).
    pub selection_policy: Option<&'a [u8]>,
}

/// Exact subject, lifecycle coordinates and V3 request derived for one action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralRequestV1 {
    /// The action this request asks for.
    pub action: Action,
    /// Slot-independent subject identity carried as the request subject.
    /// `None` only for selection `Freeze`, which the codec forbids one.
    pub subject_id: Option<[u8; 32]>,
    /// Canonical primary-state PDA selected by the published lifecycle recipe.
    pub primary_state: Pubkey,
    /// Canonical PDA bump encoded in the request.
    pub primary_state_bump: u8,
    /// Canonical secondary-state PDA, for the four actions whose lifecycle
    /// shape declares one.
    pub secondary_state: Option<Pubkey>,
    /// Its canonical bump, zero where there is no secondary state.
    pub secondary_state_bump: u8,
    /// Canonical conditional result-state PDA; only `VerifyCandidateRow`.
    pub result_state: Option<Pubkey>,
    /// Its canonical bump, zero where there is no result state.
    pub result_state_bump: u8,
    /// Exact canonical 64-byte V3 request.
    pub request:
        [u8; dclutch_trading::general_codec::successor_request_v3::CONTROLLER_REQUEST_BYTES_V3],
}

/// Decode one live General state account and check it is the kind expected.
///
/// The kind is checked because a lifecycle envelope carries six of them and a
/// campaign that handed the Order account to a Batch action would otherwise
/// reach the record decoder with bytes of the wrong width and be told only
/// that they did not decode.
fn live_state(
    account: &[u8],
    kind: GeneralLocalStateKindV3,
) -> Result<GeneralLocalStateV3<'_>, BuilderError> {
    let envelope = GeneralLocalStateV3::decode(account).map_err(|_| BuilderError::Artifact)?;
    if envelope.header().kind != kind {
        return Err(BuilderError::Binding(line!()));
    }
    Ok(envelope)
}

/// Decode the exact live Batch a request or projector names.
fn live_batch(account: &[u8]) -> Result<(GeneralLocalStateV3<'_>, GeneralBatchV1), BuilderError> {
    let envelope = live_state(account, GeneralLocalStateKindV3::Batch)?;
    let batch = GeneralBatchV1::decode(envelope.body()).map_err(|_| BuilderError::Artifact)?;
    Ok((envelope, batch))
}

/// Decode the exact live Order a request names.
fn live_order(
    account: &[u8],
) -> Result<(GeneralLocalStateV3<'_>, GeneralOrderV1<'_>), BuilderError> {
    let envelope = live_state(account, GeneralLocalStateKindV3::Order)?;
    let order = GeneralOrderV1::decode(envelope.body()).map_err(|_| BuilderError::Artifact)?;
    Ok((envelope, order))
}

/// Derive one General state address from a set of seed coordinates.
fn state_address(
    seeds: GeneralStateAddressSeedsV3,
    trading_program: Pubkey,
) -> Result<(Pubkey, u8), BuilderError> {
    let slices = seeds.as_slices().map_err(|_| BuilderError::Artifact)?;
    Ok(Pubkey::find_program_address(
        slices.as_slice(),
        &trading_program,
    ))
}

/// The Batch PDA and its canonical bump for one occurrence under one root.
fn batch_address(
    root_address: Pubkey,
    trading_program: Pubkey,
    occurrence_id: [u8; 32],
) -> Result<(Pubkey, u8), BuilderError> {
    let seeds = GeneralStateAddressSeedsV3::batch(root_address.to_bytes(), occurrence_id)
        .map_err(|_| BuilderError::Artifact)?;
    state_address(seeds, trading_program)
}

/// The seed coordinates the FAMILY POLICY selects for this action's primary
/// state.
///
/// [`GeneralStateRecipeV3::primary_for_action`] is the one author for which of
/// the eight recipes an action's primary state uses; this function is only the
/// join from that answer to the coordinate the caller derived. Nothing here
/// names a seed literal, an order, or a bump offset -- the recipe table does.
fn primary_state_seeds(
    action: Action,
    root_address: Pubkey,
    coordinate: [u8; 32],
) -> Result<GeneralStateAddressSeedsV3, BuilderError> {
    let root = root_address.to_bytes();
    match GeneralStateRecipeV3::primary_for_action(action) {
        GeneralStateRecipeV3::Batch => GeneralStateAddressSeedsV3::batch(root, coordinate),
        GeneralStateRecipeV3::Order => GeneralStateAddressSeedsV3::order(root, coordinate),
        GeneralStateRecipeV3::Candidate => GeneralStateAddressSeedsV3::candidate(root, coordinate),
        GeneralStateRecipeV3::Selection => GeneralStateAddressSeedsV3::selection(root, coordinate),
        GeneralStateRecipeV3::Settlement => {
            GeneralStateAddressSeedsV3::settlement(root, coordinate)
        }
        // `primary_for_action` returns five of the eight recipes; the other
        // three are somebody's SECONDARY or RESULT state and never a primary
        // one. A recipe table that changed that would arrive here rather than
        // deriving a Terminal address for an action that names no coordinate
        // for it.
        GeneralStateRecipeV3::Terminal
        | GeneralStateRecipeV3::Verifier
        | GeneralStateRecipeV3::VerifiedCandidate => return Err(BuilderError::Binding(line!())),
    }
    .map_err(|_| BuilderError::Artifact)
}

/// Join one live Batch back to the root, config and Product this request names.
///
/// THE SUBJECT IS READ, NOT PREDICTED. `GeneralBatchV1::batch_id` recomputes
/// the occurrence identity from the batch's own immutable opening, so the
/// request names the batch the chain holds even where a host-side prediction
/// of the opening would have differed. The persisted canonical bump and the
/// rederived one are two independent authors for the same byte and are joined
/// here, so a recipe that changed under the market cannot pass a stale witness.
fn joined_batch(
    input: &GeneralRequestInputV1<'_>,
    account: &[u8],
) -> Result<[u8; 32], BuilderError> {
    let (envelope, batch) = live_batch(account)?;
    let batch_id = batch.batch_id();
    let (_, bump) = batch_address(input.root_address, input.trading_program, batch_id)?;
    let opening = batch.opening();
    if envelope.header().bump != bump
        || opening.outcome_count != input.outcome_count
        || opening.market != input.root.market()
        || opening.config_id != input.root.config_id()
        || opening.generation != input.root.generation()
        || opening.product_id != input.product_id
    {
        return Err(BuilderError::Binding(line!()));
    }
    Ok(batch_id)
}

/// Everything one action's request carries that the fifteen do not share.
///
/// Assembled by the match in [`derive_general_request_v1`] and spent once, so
/// the encode below states each wire field exactly once for every action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DerivedCoordinatesV1 {
    subject_id: Option<[u8; 32]>,
    /// The identity the primary recipe's second seed slot takes.
    primary_coordinate: [u8; 32],
    /// The bump the live primary state persists, where one exists, joined
    /// against the rederived one at a single site below.
    persisted_primary_bump: Option<u8>,
    expected_revision: u64,
    page_index: u32,
    execution_index: u8,
    manifest_order_index: u8,
    secondary: Option<GeneralStateAddressSeedsV3>,
    result: Option<GeneralStateAddressSeedsV3>,
}

impl DerivedCoordinatesV1 {
    /// The shape ten of the fifteen actions have: one subject, one coordinate,
    /// no page, no secondary state, and a revision their own arm supplies.
    const fn new(subject_id: Option<[u8; 32]>, primary_coordinate: [u8; 32]) -> Self {
        Self {
            subject_id,
            primary_coordinate,
            persisted_primary_bump: None,
            expected_revision: 0,
            page_index: 0,
            execution_index: 0,
            manifest_order_index: 0,
            secondary: None,
            result: None,
        }
    }
}

/// Derive the exact pre-executable request for one General action.
///
/// The joins every action shares are checked once, here: nonzero corpus, the
/// root's config identity against the exact selected config bytes, and the
/// root's generation against the config's. What differs per action is the
/// SUBJECT, the STATE, and the cursor coordinate, and that is the whole of the
/// match below.
///
/// EVERY COORDINATE IS READ OFF A LIVE RECORD. A settlement page is the
/// cursor's `next_order` joined through the manifest that names it, a verify
/// row is the verifier cursor's own `next_page/next_row`, a `Consider`'s page
/// is the submitted certificate's candidate coordinate. None of the three is a
/// number a campaign chooses, because a campaign that chose one would be a
/// second author for a coordinate the chain already advances.
#[allow(clippy::too_many_lines)]
pub fn derive_general_request_v1(
    input: GeneralRequestInputV1<'_>,
) -> Result<GeneralRequestV1, BuilderError> {
    let config = GeneralConfigV3::decode(input.config).map_err(|_| BuilderError::Artifact)?;
    if input.root_address == Pubkey::default()
        || input.trading_program == Pubkey::default()
        || input.outcome_count == 0
        || input.product_id == [0; 32]
        || input.root.config_id() != solana_program::hash::hash(input.config).to_bytes()
        || input.root.generation() != config.generation()
    {
        return Err(BuilderError::Binding(line!()));
    }
    let evidence = input.evidence;
    let root_seed = input.root_address.to_bytes();
    let created = || -> Result<(), BuilderError> {
        // The state this execution CREATES: there is nothing on chain to read,
        // and a campaign that supplied one is describing a different execution
        // than the one it asked for.
        if input.primary_state_account.is_some() {
            return Err(BuilderError::Binding(line!()));
        }
        Ok(())
    };
    let live = || -> Result<&[u8], BuilderError> {
        input
            .primary_state_account
            .ok_or(BuilderError::Binding(line!()))
    };
    let coordinates = match input.action {
        Action::OpenBatch => {
            created()?;
            let occurrence = GeneralBatchOccurrenceTermsV1::new(GeneralBatchOpeningV1 {
                outcome_count: input.outcome_count,
                sequence: input.root.next_batch_sequence(),
                generation: input.root.generation(),
                market: input.root.market(),
                product_id: input.product_id,
                config_id: input.root.config_id(),
                price_scale: config.price_scale(),
                collection_close_slot: 0,
                settlement_close_slot: 0,
                max_orders: config.max_orders_per_candidate(),
            })
            .map_err(|_| BuilderError::Artifact)?;
            let occurrence_id = occurrence.occurrence_id();
            DerivedCoordinatesV1 {
                expected_revision: input.root.revision(),
                ..DerivedCoordinatesV1::new(Some(occurrence_id), occurrence_id)
            }
        }
        Action::CloseBatch => {
            let batch_id = joined_batch(&input, live()?)?;
            DerivedCoordinatesV1 {
                expected_revision: input.root.revision(),
                // Joined inside `joined_batch`, which is the only reader of the
                // envelope this arm decodes.
                ..DerivedCoordinatesV1::new(Some(batch_id), batch_id)
            }
        }
        Action::PlaceOrder => {
            // THE ORDER'S IDENTITY IS THE BYTES THE MAKER SIGNED. The signed
            // terms ARE the masked identity preimage, so `order_id` is a digest
            // of the evidence account rather than a host reconstruction of it,
            // and a campaign that changed one term names a different order.
            let batch_id = joined_batch(&input, live()?)?;
            let terms = GeneralSignedOrderTermsV1::decode(
                evidence
                    .signed_order_terms
                    .ok_or(BuilderError::Binding(line!()))?,
            )
            .map_err(|_| BuilderError::Artifact)?;
            let header = terms.header();
            if header.batch_id != batch_id
                || header.outcome_count != input.outcome_count
                || header.market != input.root.market()
                || header.generation != input.root.generation()
            {
                return Err(BuilderError::Binding(line!()));
            }
            let order_id = terms.order_id();
            DerivedCoordinatesV1 {
                secondary: Some(
                    GeneralStateAddressSeedsV3::order(root_seed, order_id)
                        .map_err(|_| BuilderError::Artifact)?,
                ),
                ..DerivedCoordinatesV1::new(Some(order_id), batch_id)
            }
        }
        Action::CancelOrder => {
            let batch_id = joined_batch(&input, live()?)?;
            let account = evidence
                .order_account
                .ok_or(BuilderError::Binding(line!()))?;
            let (envelope, order) = live_order(account)?;
            let order_id = order.order_id();
            let seeds = GeneralStateAddressSeedsV3::order(root_seed, order_id)
                .map_err(|_| BuilderError::Artifact)?;
            let (_, bump) = state_address(seeds, input.trading_program)?;
            let header = order.header();
            if envelope.header().bump != bump
                || header.batch_id != batch_id
                || header.outcome_count != input.outcome_count
                || header.market != input.root.market()
                || header.generation != input.root.generation()
            {
                return Err(BuilderError::Binding(line!()));
            }
            DerivedCoordinatesV1 {
                secondary: Some(seeds),
                ..DerivedCoordinatesV1::new(Some(order_id), batch_id)
            }
        }
        Action::ReleaseOrder => {
            // Batch-free: the window gate is a constant of the record the maker
            // signed, so the order IS the primary state and there is no second
            // coordinate to join it to.
            let (envelope, order) = live_order(live()?)?;
            let header = order.header();
            if header.outcome_count != input.outcome_count
                || header.market != input.root.market()
                || header.generation != input.root.generation()
            {
                return Err(BuilderError::Binding(line!()));
            }
            let order_id = order.order_id();
            DerivedCoordinatesV1 {
                persisted_primary_bump: Some(envelope.header().bump),
                ..DerivedCoordinatesV1::new(Some(order_id), order_id)
            }
        }
        Action::SubmitCandidate => {
            created()?;
            let image = CandidateV2::decode(
                evidence
                    .candidate_image
                    .ok_or(BuilderError::Binding(line!()))?,
            )
            .map_err(|_| BuilderError::Artifact)?;
            let header = image.header();
            if header.outcome_count != input.outcome_count || header.product_id != input.product_id
            {
                return Err(BuilderError::Binding(line!()));
            }
            DerivedCoordinatesV1::new(Some(header.candidate_id), header.candidate_id)
        }
        Action::VerifyCandidateRow => {
            let envelope = live_state(live()?, GeneralLocalStateKindV3::Candidate)?;
            let submission =
                GeneralCandidateV1::decode(envelope.body()).map_err(|_| BuilderError::Artifact)?;
            let opening = submission.opening();
            if opening.outcome_count != input.outcome_count {
                return Err(BuilderError::Binding(line!()));
            }
            let candidate_id = opening.candidate_id;
            // THE ROW IS THE CURSOR'S, AND A VACANT CURSOR IS ROW ZERO. The
            // runtime accepts `(0, 0, 0)` only against an all-zero cursor and
            // the persisted triple otherwise, so this is the one coordinate
            // choice the chain makes and the host reads back.
            let (page_index, execution_index, expected_revision) = match evidence.verifier_account {
                None => (0, 0, 0),
                Some(account) => {
                    let cursor_state = live_state(account, GeneralLocalStateKindV3::Verifier)?;
                    let cursor = RuntimeCandidateVerifierV2::decode(cursor_state.body())
                        .map_err(|_| BuilderError::Artifact)?;
                    let header = cursor.header();
                    if header.candidate_id != candidate_id
                        || header.outcome_count != input.outcome_count
                    {
                        return Err(BuilderError::Binding(line!()));
                    }
                    (
                        header.next_page_index,
                        u8::try_from(header.next_row_index)
                            .map_err(|_| BuilderError::Arithmetic)?,
                        header.revision,
                    )
                }
            };
            DerivedCoordinatesV1 {
                persisted_primary_bump: Some(envelope.header().bump),
                expected_revision,
                page_index,
                execution_index,
                secondary: Some(
                    GeneralStateAddressSeedsV3::verifier(root_seed, candidate_id)
                        .map_err(|_| BuilderError::Artifact)?,
                ),
                result: Some(
                    GeneralStateAddressSeedsV3::verified_candidate(root_seed, candidate_id)
                        .map_err(|_| BuilderError::Artifact)?,
                ),
                ..DerivedCoordinatesV1::new(Some(candidate_id), candidate_id)
            }
        }
        Action::CloseCandidate => {
            let envelope = live_state(live()?, GeneralLocalStateKindV3::Candidate)?;
            let submission =
                GeneralCandidateV1::decode(envelope.body()).map_err(|_| BuilderError::Artifact)?;
            let opening = submission.opening();
            if opening.outcome_count != input.outcome_count {
                return Err(BuilderError::Binding(line!()));
            }
            DerivedCoordinatesV1 {
                persisted_primary_bump: Some(envelope.header().bump),
                ..DerivedCoordinatesV1::new(Some(opening.candidate_id), opening.candidate_id)
            }
        }
        Action::Consider => {
            // THE BATCH COMES FROM THE AUTHENTICATED RECORD THE ACTION READS.
            // The first `Consider` of a batch CREATES the cursor, so the batch
            // identity its address is keyed on cannot come from the cursor --
            // it comes from the submitted certificate, exactly as the emitted
            // profile's own operation projects it.
            let verified = VerifiedCandidateV2::decode(
                evidence
                    .verified_candidate
                    .ok_or(BuilderError::Binding(line!()))?,
            )
            .map_err(|_| BuilderError::Artifact)?;
            let header = verified.header();
            if header.outcome_count != input.outcome_count
                || header.product_id != input.product_id
                || header.price_scale != config.price_scale()
            {
                return Err(BuilderError::Binding(line!()));
            }
            let (expected_revision, persisted_primary_bump) = match input.primary_state_account {
                None => (0, None),
                Some(account) => {
                    let cursor_state = live_state(account, GeneralLocalStateKindV3::Selection)?;
                    let cursor = RuntimeSelectionCursorV2::decode(cursor_state.body())
                        .map_err(|_| BuilderError::Artifact)?;
                    if cursor.header().batch_id != header.batch_id {
                        return Err(BuilderError::Binding(line!()));
                    }
                    (cursor.header().revision, Some(cursor_state.header().bump))
                }
            };
            DerivedCoordinatesV1 {
                persisted_primary_bump,
                expected_revision,
                page_index: header.candidate_coordinate,
                ..DerivedCoordinatesV1::new(Some(header.candidate_id), header.batch_id)
            }
        }
        Action::Freeze => {
            // The one action the codec forbids a subject: its whole coordinate
            // is the cursor it closes, and that cursor's own address carries
            // the batch identity since the per-batch selection landed.
            let cursor_state = live_state(live()?, GeneralLocalStateKindV3::Selection)?;
            let cursor = RuntimeSelectionCursorV2::decode(cursor_state.body())
                .map_err(|_| BuilderError::Artifact)?;
            let header = cursor.header();
            if header.outcome_count != input.outcome_count
                || header.product_id != input.product_id
                || header.policy_id != config.selection_policy_id()
                || header.price_scale != config.price_scale()
            {
                return Err(BuilderError::Binding(line!()));
            }
            DerivedCoordinatesV1 {
                persisted_primary_bump: Some(cursor_state.header().bump),
                expected_revision: header.revision,
                ..DerivedCoordinatesV1::new(None, header.batch_id)
            }
        }
        Action::InitializeSettlement => {
            created()?;
            let verified = VerifiedCandidateV2::decode(
                evidence
                    .verified_candidate
                    .ok_or(BuilderError::Binding(line!()))?,
            )
            .map_err(|_| BuilderError::Artifact)?;
            let header = verified.header();
            if header.outcome_count != input.outcome_count
                || header.product_id != input.product_id
                || header.price_scale != config.price_scale()
            {
                return Err(BuilderError::Binding(line!()));
            }
            // The initializer's revision is required to be zero by the cursor's
            // own opening, not by this module.
            DerivedCoordinatesV1::new(Some(header.candidate_id), header.candidate_id)
        }
        Action::Collect | Action::Distribute | Action::Materialize | Action::Close => {
            let cursor_state = live_state(live()?, GeneralLocalStateKindV3::Settlement)?;
            let cursor = SettlementCursorV2::decode(cursor_state.body())
                .map_err(|_| BuilderError::Artifact)?;
            let header = cursor.header();
            if header.outcome_count != input.outcome_count {
                return Err(BuilderError::Binding(line!()));
            }
            let candidate_id = header.candidate_id;
            let base = DerivedCoordinatesV1 {
                persisted_primary_bump: Some(cursor_state.header().bump),
                expected_revision: header.revision,
                ..DerivedCoordinatesV1::new(Some(candidate_id), candidate_id)
            };
            match input.action {
                // THE ROW ACTIONS SELECT THEIR MANIFEST ENTRY BY THE CURSOR'S
                // OWN NEXT COORDINATE. The manifest is content-addressed and
                // its entries carry one-based `order_coordinate`s; the runtime
                // admits exactly the entry whose coordinate is `next_order + 1`
                // and then requires the request's page and row to equal that
                // entry's source coordinates. So all three fields are one
                // lookup, and none of them is a campaign's choice.
                Action::Collect | Action::Distribute => {
                    let manifest = SettlementManifestV2::decode(
                        evidence
                            .settlement_manifest
                            .ok_or(BuilderError::Binding(line!()))?,
                    )
                    .map_err(|_| BuilderError::Artifact)?;
                    let expected_order = header
                        .next_order
                        .checked_add(1)
                        .ok_or(BuilderError::Arithmetic)?;
                    let mut selected = None;
                    for index in 0..manifest.header().order_count {
                        let order = manifest.order(index).map_err(|_| BuilderError::Artifact)?;
                        if order.header().order_coordinate == expected_order {
                            selected = Some((index, order.header()));
                            break;
                        }
                    }
                    let (index, order_header) = selected.ok_or(BuilderError::Binding(line!()))?;
                    if order_header.candidate_id != candidate_id
                        || order_header.outcome_count != input.outcome_count
                    {
                        return Err(BuilderError::Binding(line!()));
                    }
                    DerivedCoordinatesV1 {
                        page_index: order_header.source_page_index,
                        execution_index: u8::try_from(order_header.source_execution_index)
                            .map_err(|_| BuilderError::Arithmetic)?,
                        manifest_order_index: u8::try_from(index)
                            .map_err(|_| BuilderError::Arithmetic)?,
                        ..base
                    }
                }
                // `Close` also names the terminal record it is about to create,
                // and its coordinate is the revision this close consumes plus
                // one -- the same expression `runtime_settlement::close` uses,
                // so the address and the record cannot disagree about which
                // terminal event this is.
                Action::Close => DerivedCoordinatesV1 {
                    secondary: Some(
                        GeneralStateAddressSeedsV3::terminal(
                            root_seed,
                            candidate_id,
                            header
                                .revision
                                .checked_add(1)
                                .ok_or(BuilderError::Arithmetic)?,
                        )
                        .map_err(|_| BuilderError::Artifact)?,
                    ),
                    ..base
                },
                _ => base,
            }
        }
    };
    let (primary_state, primary_state_bump) = state_address(
        primary_state_seeds(
            input.action,
            input.root_address,
            coordinates.primary_coordinate,
        )?,
        input.trading_program,
    )?;
    // THE PERSISTED BUMP AND THE REDERIVED ONE ARE TWO AUTHORS FOR ONE BYTE,
    // and the request witnesses it to the accelerator's admission. Joining them
    // here means a recipe that changed under the market cannot pass a stale
    // witness -- for every action with a live primary state, not only the two
    // that had a deriver before.
    if coordinates
        .persisted_primary_bump
        .is_some_and(|persisted| persisted != primary_state_bump)
    {
        return Err(BuilderError::Binding(line!()));
    }
    let secondary = coordinates
        .secondary
        .map(|seeds| state_address(seeds, input.trading_program))
        .transpose()?;
    let result = coordinates
        .result
        .map(|seeds| state_address(seeds, input.trading_program))
        .transpose()?;
    // THE WIRE GENERATION IS THE ACTION'S, AND ONE FUNCTION ALREADY KNOWS WHICH.
    //
    // General ships TWO 64-byte request wires: the seven settlement and
    // selection tags kept `DCGREQ02` and the eight front-half tags speak
    // `DCGREQ03`, and each action's own emitted RequestProfile revalidates
    // exactly one of them. A deriver that encoded V3 for all fifteen builds
    // seven requests that decode, round-trip, and are then refused by their own
    // profile with `CheckFailed` -- measured here on `Consider` the first time
    // this module derived one.
    //
    // So the generation is not restated: `decode_general_request_v3` is the
    // chain's own reader and its selector rule is the authority. This encodes
    // the semantic request in each generation and keeps the one that reader
    // accepts as that generation, which cannot drift the day a sixteenth action
    // lands on the other side of the boundary.
    let semantic = |wire| GeneralDecodedRequestV3 {
        wire,
        action: input.action,
        expected_revision: coordinates.expected_revision,
        candidate_id: coordinates.subject_id,
        page_index: coordinates.page_index,
        execution_index: coordinates.execution_index,
        manifest_order_index: coordinates.manifest_order_index,
        state_bump: primary_state_bump,
        terminal_record_bump: secondary.map_or(0, |(_, bump)| bump),
        result_state_bump: result.map_or(0, |(_, bump)| bump),
    };
    let request = [GeneralRequestWireV3::V2, GeneralRequestWireV3::V3]
        .into_iter()
        .find_map(|wire| {
            let bytes = semantic(wire).to_bytes().ok()?;
            (decode_general_request_v3(&bytes).ok()?.wire == wire).then_some(bytes)
        })
        .ok_or(BuilderError::Artifact)?;
    Ok(GeneralRequestV1 {
        action: input.action,
        subject_id: coordinates.subject_id,
        primary_state,
        primary_state_bump,
        secondary_state: secondary.map(|(address, _)| address),
        secondary_state_bump: secondary.map_or(0, |(_, bump)| bump),
        result_state: result.map(|(address, _)| address),
        result_state_bump: result.map_or(0, |(_, bump)| bump),
        request,
    })
}

/// Semantic chain prestate one action's candidate projector reads.
///
/// A campaign supplies the exact bytes the bank holds; the projector's semantic
/// owner decodes them. Nothing here is optional in the sense of "may be
/// omitted": each action requires exactly what its own AccountProfile declares,
/// and an action handed the wrong shape refuses at a named line.
///
/// The evidence is the SAME value the request derivation takes, because it is
/// the same question -- which records does this action read -- asked at the two
/// ends of one execution. A campaign builds it once.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GeneralActionPrestateV1<'a> {
    /// Exact account data of the primary state this action operates on, as the
    /// chain holds it, or `None` where this execution creates it.
    pub primary_state_account: Option<&'a [u8]>,
    /// The authenticated records this action reads that are not its primary
    /// state.
    pub evidence: GeneralRequestEvidenceV1<'a>,
}

/// Build one admitted-AOT General Hot instruction from current artifacts.
///
/// The caller must have obtained `input.scenario.family_request` from
/// [`derive_general_request_v1`]. The selected ProgramSet remains the authority
/// for the descriptor/action join; the guard below merely prevents a General
/// campaign helper from silently being used for another catalogue row, and the
/// action it derives is the one the request asks for rather than one this
/// module names.
///
/// ## The projector is the accelerator's, not a second interpreter
///
/// Every arm below calls the SAME `hot_candidate_v3` entry point
/// `dclutch-general-accelerator-sbf` calls, over the same semantic transitions,
/// with the campaign's chain corpus in place of the runtime's `AccountInfo`s.
/// Nothing here recomputes a candidate: where the accelerator runs
/// `consider_verified_candidate_v2` before projecting, so does this, and where
/// it declines to write the bank at all -- `CloseCandidate` authenticates and
/// returns -- so does this.
pub fn build_general_action_bundle_v1(
    input: &BundleInputV1<'_>,
    admitted: AdmittedAotInputV1<'_>,
    prestate: GeneralActionPrestateV1<'_>,
) -> Result<BuiltAdmittedBundleV1, BuilderError> {
    // THE READER IS THE CHAIN'S, AND IT SELECTS THE GENERATION. Seven actions
    // kept the `DCGREQ02` wire; decoding every family request as V3 refused
    // them all at `Artifact` with no word for which half of the catalogue the
    // caller was in.
    let request = decode_general_request_v3(input.scenario.family_request)
        .map_err(|_| BuilderError::Artifact)?;
    let action = request.action;
    let config = GeneralConfigV3::decode(input.set.config).map_err(|_| BuilderError::Artifact)?;
    let root_tail = input
        .fixed
        .root
        .account
        .data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(BuilderError::Binding(line!()))?;
    GeneralRootV2::decode(root_tail).map_err(|_| BuilderError::Binding(line!()))?;
    let outcome_count = input.scenario.tail_count;
    let bank_len = general_hot_candidate_bank_len_v3(action, outcome_count)
        .map_err(|_| BuilderError::Projection("general-bank-width"))?;
    // The semantic prestate is decoded ONCE, outside the projector closure: the
    // adoption loop runs the projector up to four times and a body re-decoded
    // per round would make a corpus refusal look like a divergence.
    let corpus = GeneralActionCorpusV1::decode(action, prestate)?;
    let projector =
        |scalars: &mut [u64], identities: &mut [[u8; 32]]| -> Result<(), BuilderError> {
            let mut bank = vec![0_u8; bank_len];
            encode_register_bank_into(scalars, identities, &mut bank)
                .map_err(|_| BuilderError::Projection("general-bank-encode"))?;
            let environment = general_hot_environment_from_bank_v3(action, &bank, outcome_count)
                .map_err(|_| BuilderError::Projection("general-environment"))?;
            corpus.project(
                GeneralProjectionInputV1 {
                    action,
                    root_tail,
                    config,
                    outcome_count,
                    environment,
                    expected_revision: request.expected_revision,
                    subject_id: request.candidate_id,
                    manifest_order_index: u32::from(request.manifest_order_index),
                    page_index: request.page_index,
                    execution_index: u32::from(request.execution_index),
                    family_request: input.scenario.family_request,
                },
                &mut bank,
            )?;
            decode_register_bank_into(&bank, scalars, identities)
                .map_err(|_| BuilderError::Projection("general-bank-decode"))
        };
    let built = build_admitted_bundle_with_candidate_v1(input, admitted, &projector)?;
    if built.bundle.artifacts.action != u32::from(action as u8) {
        return Err(BuilderError::Artifact);
    }
    Ok(built)
}

/// Ask whether one action's prestate is the shape that action reads.
///
/// [`build_general_action_bundle_v1`] decodes the corpus before it builds
/// anything, so a campaign that supplied the wrong record pays a whole bundle
/// construction to be told `Binding` at a line. This is that decode alone, and
/// it exists because it is also the only part of the thirteen new projector
/// arms a test without a chain frame can execute: WHICH RECORD GOES TO WHICH
/// PARAMETER is the plumbing, and the projectors it hands them to are the
/// accelerator's own and already have tests.
pub fn general_action_prestate_shape_v1(
    action: Action,
    prestate: GeneralActionPrestateV1<'_>,
) -> Result<(), BuilderError> {
    GeneralActionCorpusV1::decode(action, prestate).map(|_| ())
}

/// The request coordinates and joins every projector arm shares.
#[derive(Clone, Copy)]
struct GeneralProjectionInputV1<'a> {
    action: Action,
    root_tail: &'a [u8],
    config: GeneralConfigV3,
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    expected_revision: u64,
    subject_id: Option<[u8; 32]>,
    manifest_order_index: u32,
    page_index: u32,
    execution_index: u32,
    family_request: &'a [u8],
}

/// One action's decoded semantic corpus, decoded once before the adoption loop.
///
/// The variants are the shapes the fifteen actions actually have, not fifteen
/// arms: five of them read a Batch body, four read a settlement cursor and a
/// certificate, and two read a selection cursor. An action handed a record its
/// own profile does not declare never reaches its projector.
#[derive(Clone, Copy)]
enum GeneralActionCorpusV1<'a> {
    /// `OpenBatch`: creates every state it names.
    OpenBatch,
    /// `CloseBatch`: the live Batch body.
    CloseBatch { batch: &'a [u8] },
    /// `PlaceOrder`: the live Batch body and the exact signed order terms.
    PlaceOrder { batch: &'a [u8], terms: &'a [u8] },
    /// `CancelOrder`: the live Batch body and the live Order body.
    CancelOrder { batch: &'a [u8], order: &'a [u8] },
    /// `ReleaseOrder`: the live Order body alone.
    ReleaseOrder { order: &'a [u8] },
    /// `SubmitCandidate`: the closed Batch, the candidate image, and the exact
    /// submission record this execution writes.
    SubmitCandidate {
        batch: &'a [u8],
        image: &'a [u8],
        submission: &'a [u8],
    },
    /// `VerifyCandidateRow`: the whole row-verification view's corpus.
    VerifyCandidateRow {
        batch: GeneralBatchV1,
        submission: GeneralCandidateV1,
        image: &'a [u8],
        page: &'a [u8],
        order: &'a [u8],
        cursor_before: &'a [u8],
        manifest: &'a [u8],
    },
    /// `CloseCandidate`: the live Candidate and the closed Batch it settles.
    CloseCandidate {
        batch: GeneralBatchV1,
        submission: GeneralCandidateV1,
    },
    /// `Consider`: the selection policy, the submitted certificate, and the
    /// cursor prestate -- vacant for the first consideration of a batch.
    Consider {
        policy: SelectionPolicyV1,
        verified: &'a [u8],
        cursor_before: Option<&'a [u8]>,
    },
    /// `Freeze`: the live selection cursor.
    Freeze { cursor_before: &'a [u8] },
    /// `InitializeSettlement`: the completed verifier and its certificate.
    InitializeSettlement {
        verifier: &'a [u8],
        verified: &'a [u8],
    },
    /// The settlement four: the live cursor, the certificate, and the manifest
    /// the two row actions select from.
    Settlement {
        cursor_before: &'a [u8],
        verified: &'a [u8],
        manifest: Option<&'a [u8]>,
    },
}

/// Borrow one live state account's semantic body, checking its kind.
fn live_body(
    account: Option<&[u8]>,
    kind: GeneralLocalStateKindV3,
    line: u32,
) -> Result<&[u8], BuilderError> {
    let account = account.ok_or(BuilderError::Binding(line))?;
    Ok(live_state(account, kind)?.body())
}

impl<'a> GeneralActionCorpusV1<'a> {
    /// Decode exactly what this action names, once, before any projection.
    fn decode(action: Action, prestate: GeneralActionPrestateV1<'a>) -> Result<Self, BuilderError> {
        let evidence = prestate.evidence;
        let primary = prestate.primary_state_account;
        let created = |account: Option<&[u8]>| -> Result<(), BuilderError> {
            if account.is_some() {
                return Err(BuilderError::Binding(line!()));
            }
            Ok(())
        };
        Ok(match action {
            Action::OpenBatch => {
                created(primary)?;
                Self::OpenBatch
            }
            Action::CloseBatch => Self::CloseBatch {
                batch: live_body(primary, GeneralLocalStateKindV3::Batch, line!())?,
            },
            Action::PlaceOrder => Self::PlaceOrder {
                batch: live_body(primary, GeneralLocalStateKindV3::Batch, line!())?,
                terms: evidence
                    .signed_order_terms
                    .ok_or(BuilderError::Binding(line!()))?,
            },
            Action::CancelOrder => Self::CancelOrder {
                batch: live_body(primary, GeneralLocalStateKindV3::Batch, line!())?,
                order: live_body(
                    evidence.order_account,
                    GeneralLocalStateKindV3::Order,
                    line!(),
                )?,
            },
            Action::ReleaseOrder => Self::ReleaseOrder {
                order: live_body(primary, GeneralLocalStateKindV3::Order, line!())?,
            },
            Action::SubmitCandidate => {
                created(primary)?;
                Self::SubmitCandidate {
                    batch: live_body(
                        evidence.batch_account,
                        GeneralLocalStateKindV3::Batch,
                        line!(),
                    )?,
                    image: evidence
                        .candidate_image
                        .ok_or(BuilderError::Binding(line!()))?,
                    submission: evidence
                        .submitted_candidate
                        .ok_or(BuilderError::Binding(line!()))?,
                }
            }
            Action::VerifyCandidateRow => Self::VerifyCandidateRow {
                batch: GeneralBatchV1::decode(live_body(
                    evidence.batch_account,
                    GeneralLocalStateKindV3::Batch,
                    line!(),
                )?)
                .map_err(|_| BuilderError::Artifact)?,
                submission: GeneralCandidateV1::decode(live_body(
                    primary,
                    GeneralLocalStateKindV3::Candidate,
                    line!(),
                )?)
                .map_err(|_| BuilderError::Artifact)?,
                image: evidence
                    .candidate_image
                    .ok_or(BuilderError::Binding(line!()))?,
                page: evidence
                    .candidate_page
                    .ok_or(BuilderError::Binding(line!()))?,
                order: live_body(
                    evidence.order_account,
                    GeneralLocalStateKindV3::Order,
                    line!(),
                )?,
                // A VACANT verifier is this candidate's first row, and the
                // empty slice is exactly what the runtime reads for one.
                cursor_before: match evidence.verifier_account {
                    None => &[],
                    Some(account) => live_state(account, GeneralLocalStateKindV3::Verifier)?.body(),
                },
                manifest: evidence
                    .settlement_manifest
                    .ok_or(BuilderError::Binding(line!()))?,
            },
            Action::CloseCandidate => Self::CloseCandidate {
                batch: GeneralBatchV1::decode(live_body(
                    evidence.batch_account,
                    GeneralLocalStateKindV3::Batch,
                    line!(),
                )?)
                .map_err(|_| BuilderError::Artifact)?,
                submission: GeneralCandidateV1::decode(live_body(
                    primary,
                    GeneralLocalStateKindV3::Candidate,
                    line!(),
                )?)
                .map_err(|_| BuilderError::Artifact)?,
            },
            Action::Consider => Self::Consider {
                policy: SelectionPolicyV1::decode(
                    evidence
                        .selection_policy
                        .ok_or(BuilderError::Binding(line!()))?,
                )
                .map_err(|_| BuilderError::Artifact)?,
                verified: evidence
                    .verified_candidate
                    .ok_or(BuilderError::Binding(line!()))?,
                cursor_before: match primary {
                    None => None,
                    Some(account) => {
                        Some(live_state(account, GeneralLocalStateKindV3::Selection)?.body())
                    }
                },
            },
            Action::Freeze => Self::Freeze {
                cursor_before: live_body(primary, GeneralLocalStateKindV3::Selection, line!())?,
            },
            Action::InitializeSettlement => {
                created(primary)?;
                Self::InitializeSettlement {
                    verifier: live_body(
                        evidence.verifier_account,
                        GeneralLocalStateKindV3::Verifier,
                        line!(),
                    )?,
                    verified: evidence
                        .verified_candidate
                        .ok_or(BuilderError::Binding(line!()))?,
                }
            }
            Action::Collect | Action::Materialize | Action::Distribute | Action::Close => {
                // The row actions read a manifest and the other two must not
                // present one: `runtime_settlement` refuses the mismatch, and
                // stating it here means a campaign that supplied one anyway is
                // told which action it named rather than which conjunct failed.
                let row = matches!(action, Action::Collect | Action::Distribute);
                if row != evidence.settlement_manifest.is_some() {
                    return Err(BuilderError::Binding(line!()));
                }
                Self::Settlement {
                    cursor_before: live_body(
                        primary,
                        GeneralLocalStateKindV3::Settlement,
                        line!(),
                    )?,
                    verified: evidence
                        .verified_candidate
                        .ok_or(BuilderError::Binding(line!()))?,
                    manifest: evidence.settlement_manifest,
                }
            }
        })
    }

    /// Run this action's own accelerator-owned projector over the bank.
    ///
    /// One arm per shape, each calling exactly what
    /// `dclutch-general-accelerator-sbf` calls for that action, in the same
    /// order and over the same semantic transitions.
    #[allow(clippy::too_many_lines)]
    fn project(
        self,
        input: GeneralProjectionInputV1<'_>,
        bank: &mut [u8],
    ) -> Result<(), BuilderError> {
        let action = input.action;
        let outcome_count = input.outcome_count;
        let environment = input.environment;
        match self {
            Self::OpenBatch => project_general_open_batch_candidate_in_place_v3(
                input.root_tail,
                input.config,
                outcome_count,
                environment,
                input.expected_revision,
                input.subject_id,
                bank,
            )
            .map_err(projection_refusal("general-open-batch", action)),
            Self::CloseBatch { batch } => project_general_close_batch_candidate_in_place_v3(
                input.root_tail,
                batch,
                input.config,
                outcome_count,
                environment,
                input.expected_revision,
                input.subject_id,
                bank,
            )
            .map_err(projection_refusal("general-close-batch", action)),
            Self::PlaceOrder { batch, terms } => project_general_place_order_candidate_in_place_v3(
                input.root_tail,
                batch,
                input.config,
                outcome_count,
                environment,
                input.subject_id,
                terms,
                bank,
            )
            .map_err(projection_refusal("general-place-order", action)),
            Self::CancelOrder { batch, order } => {
                project_general_cancel_order_candidate_in_place_v3(
                    input.root_tail,
                    batch,
                    order,
                    input.config,
                    outcome_count,
                    environment,
                    input.subject_id,
                    bank,
                )
                .map_err(projection_refusal("general-cancel-order", action))
            }
            Self::ReleaseOrder { order } => project_general_release_order_candidate_in_place_v3(
                input.root_tail,
                order,
                input.config,
                outcome_count,
                environment,
                input.subject_id,
                bank,
            )
            .map_err(projection_refusal("general-release-order", action)),
            Self::SubmitCandidate {
                batch,
                image,
                submission,
            } => project_general_submit_candidate_in_place_v3(
                input.root_tail,
                batch,
                input.config,
                image,
                submission,
                outcome_count,
                environment,
                input.subject_id,
                bank,
            )
            .map_err(projection_refusal("general-submit-candidate", action)),
            Self::VerifyCandidateRow {
                batch,
                submission,
                image,
                page,
                order,
                cursor_before,
                manifest,
            } => {
                let verifier_len = candidate_verifier_len_v1(submission)
                    .map_err(|_| BuilderError::Projection("general-verify-width"))?;
                let mut cursor_workspace = vec![0_u8; verifier_len];
                project_general_verify_candidate_workspace_v3(
                    CandidateVerifyRowViewV1 {
                        batch,
                        submission,
                        candidate: image,
                        page,
                        order,
                        cursor_before,
                        verified_before: &[],
                        expected_page_index: input.page_index,
                        expected_row_index: input.execution_index,
                        expected_revision: input.expected_revision,
                    },
                    outcome_count,
                    bank,
                    &mut cursor_workspace,
                    manifest,
                )
                .map(|_| ())
                .map_err(projection_refusal("general-verify-candidate-row", action))
            }
            // THE ONE ACTION THAT WRITES NO CANDIDATE. `CloseCandidate`
            // authenticates its lamport poststate against the hostile-decoded
            // Candidate and Batch and returns a plan the generic Effect spends;
            // the accelerator leaves the bank exactly as Trading presented it,
            // and a projector that wrote here would be inventing a divergence.
            Self::CloseCandidate { batch, submission } => authenticate_general_close_candidate_v3(
                input.family_request,
                batch,
                submission,
                outcome_count,
                environment,
                bank,
            )
            .map(|_| ())
            .map_err(projection_refusal("general-close-candidate", action)),
            Self::Consider {
                policy,
                verified,
                cursor_before,
            } => {
                let submitted = VerifiedCandidateV2::decode(verified)
                    .map_err(|_| BuilderError::Projection("general-consider-certificate"))?;
                let header = submitted.header();
                if policy.policy_id != input.config.selection_policy_id()
                    || input.subject_id != Some(header.candidate_id)
                    || input.page_index != header.candidate_coordinate
                    || header.outcome_count != outcome_count
                    || header.product_id != environment.product_record_digest
                    || header.price_scale != input.config.price_scale()
                {
                    return Err(BuilderError::Projection("general-consider-state"));
                }
                let vacant = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
                let mut scratch = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
                let mut output = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
                consider_verified_candidate_v2(
                    policy,
                    cursor_before.unwrap_or(&vacant),
                    verified,
                    input.expected_revision,
                    &mut scratch,
                    &mut output,
                )
                .map_err(|_| BuilderError::Projection("general-consider-transition"))?;
                project_general_selection_candidate_in_place_v3(
                    action,
                    &output,
                    outcome_count,
                    bank,
                )
                .map_err(projection_refusal("general-consider", action))
            }
            Self::Freeze { cursor_before } => {
                let selected = RuntimeSelectionCursorV2::decode(cursor_before)
                    .map_err(|_| BuilderError::Projection("general-freeze-cursor"))?;
                let header = selected.header();
                if header.outcome_count != outcome_count
                    || header.policy_id != input.config.selection_policy_id()
                    || header.product_id != environment.product_record_digest
                    || header.price_scale != input.config.price_scale()
                {
                    return Err(BuilderError::Projection("general-freeze-state"));
                }
                let mut scratch = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
                let mut output = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
                freeze_selection_v2(
                    cursor_before,
                    input.expected_revision,
                    &mut scratch,
                    &mut output,
                )
                .map_err(|_| BuilderError::Projection("general-freeze-transition"))?;
                project_general_selection_candidate_in_place_v3(
                    action,
                    &output,
                    outcome_count,
                    bank,
                )
                .map_err(projection_refusal("general-freeze", action))
            }
            Self::InitializeSettlement { verifier, verified } => {
                let cursor_bytes = settlement_cursor_len(outcome_count)
                    .map_err(|_| BuilderError::Projection("general-initialize-width"))?;
                let mut cursor_output = vec![0_u8; cursor_bytes];
                initialize_runtime_settlement_in_place_v2(
                    verifier,
                    verified,
                    input.expected_revision,
                    &mut cursor_output,
                )
                .map_err(|_| BuilderError::Projection("general-initialize-transition"))?;
                project_general_initialize_candidate_in_place_v3(
                    &cursor_output,
                    outcome_count,
                    environment,
                    bank,
                )
                .map_err(projection_refusal("general-initialize-settlement", action))
            }
            Self::Settlement {
                cursor_before,
                verified,
                manifest,
            } => {
                let settlement_action = match action {
                    Action::Collect => RuntimeSettlementActionV2::Collect,
                    Action::Materialize => RuntimeSettlementActionV2::Materialize,
                    Action::Distribute => RuntimeSettlementActionV2::Distribute,
                    Action::Close => RuntimeSettlementActionV2::Close,
                    _ => return Err(BuilderError::Binding(line!())),
                };
                let cursor_bytes = cursor_before.len();
                let effect_bytes = runtime_settlement_effect_len_v2(outcome_count)
                    .map_err(|_| BuilderError::Projection("general-settlement-width"))?;
                let inventory_bytes = usize::try_from(outcome_count)
                    .ok()
                    .and_then(|count| count.checked_mul(8))
                    .ok_or(BuilderError::Arithmetic)?;
                let mut cursor_workspace = vec![0_u8; cursor_bytes];
                let mut inventory_workspace = vec![0_u8; inventory_bytes];
                let mut effect_workspace = vec![0_u8; effect_bytes];
                evaluate_runtime_settlement_in_place_v2(
                    RuntimeSettlementViewV2 {
                        action: settlement_action,
                        cursor_before,
                        verified,
                        manifest,
                        manifest_order_index: input.manifest_order_index,
                        expected_revision: input.expected_revision,
                        // The beneficiary is the config's, and only `Close` may
                        // name one at all.
                        surplus_beneficiary: (settlement_action
                            == RuntimeSettlementActionV2::Close)
                            .then(|| input.config.quote_surplus_beneficiary()),
                    },
                    &mut cursor_workspace,
                    &mut inventory_workspace,
                    &mut effect_workspace,
                )
                .map_err(|_| BuilderError::Projection("general-settlement-transition"))?;
                project_general_hot_candidate_in_place_v3(
                    action,
                    &effect_workspace,
                    &cursor_workspace,
                    outcome_count,
                    environment,
                    bank,
                )
                .map_err(projection_refusal("general-settlement", action))
            }
        }
    }
}

/// Surface the accelerator's own projector refusal into the validator log.
///
/// THE WIRE CANNOT CARRY THE CAUSE AND THE LOG CAN. `BuilderError::Projection`
/// is one `&'static str`, so `GeneralHotCandidateErrorV3` -- which
/// distinguishes a capacity, a stride, a coordinate and a plan -- would
/// otherwise be discarded at the one boundary where it is the whole answer.
/// A campaign reads a validator log first.
fn projection_refusal(
    stage: &'static str,
    action: Action,
) -> impl Fn(GeneralHotCandidateErrorV3) -> BuilderError {
    move |error| {
        std::eprintln!("general candidate projection refused at {stage} for {action:?}: {error:?}");
        BuilderError::Projection(stage)
    }
}
