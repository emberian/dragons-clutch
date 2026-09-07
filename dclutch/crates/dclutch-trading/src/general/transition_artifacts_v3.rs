//! Action-selected TransitionVM programs for General V3.
//!
//! These programs are the data-defined semantic gate paired with the admitted
//! read-only accelerator. They derive canonical action, local-state, cursor,
//! replay, and child-request coordinates in the same register bank consumed by
//! the common EffectProgram. The Product-owned tail is folded at runtime; no
//! outcome width is compiled into an artifact.
//!
//! **All fourteen programs are Lean-authored.**
//! `formal/dclutch-semantics/DClutchSemantics/GeneralTransitionV3.lean` states
//! every conjunct; `EmitGeneralTransitionV3Rust.lean` emits
//! `generated_transition_programs_v3.rs`; and
//! `every_authored_program_is_byte_identical_to_the_lean_authored_one` below
//! requires the builder in this module to reproduce those bytes exactly, for
//! all fourteen actions.
//!
//! Until this landed the General family had no Lean counterpart for its
//! transition artifacts at all -- the same gap `73f0793` closed for Direct --
//! and the imperative builder below was the sole authority for what a General
//! release admits. The pre-existing thirteen retain byte identity; Verify adds
//! the fourteenth program without changing any existing artifact digest.
//!
//! Deleting the imperative builder in favour of the emitted arrays is remaining
//! cleanup; the byte gate makes that change safe to take one action at a time.
use crate::general_codec::Action;
use crate::general_config::GeneralLifecycleV2;
use dclutch_claims::affine_batch_v2::DeltaDirectionV2;
use dclutch_custody::OperationV1;
use dclutch_vm::v3::{
    HEADER_BYTES, INSTRUCTION_BYTES, IdentityRegisterV3, InstructionV3, ProgramGeometryV3,
    ProgramV3, ScalarRegisterV3, encode_program_atomic,
};

use crate::general::{
    candidate_v1::{GeneralCandidateLayoutV1, GeneralCandidateStatusV1},
    collection_v1::{
        BatchStatusV1, GeneralBatchLayoutV1, GeneralOrderLayoutV1, GeneralOrderPhaseV1,
    },
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, GENERAL_HOT_COMMON_SCALARS_V3,
        GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3, general_hot_item_scalar_stride_v3, identity,
        item_scalar, scalar,
    },
    local_state_v3::{GeneralLocalStateKindV3, GeneralLocalStateLayoutV3},
    runtime_selection::{RuntimeSelectionLayoutV2, RuntimeSelectionPhaseV2},
    runtime_width::{SettlementCursorLayoutV2, SettlementPhaseV2},
};

/// Stable General transition-artifact refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralTransitionArtifactErrorV3 {
    /// Caller-owned workspaces or checked widths differed.
    Geometry,
    /// The canonical TransitionVM encoder refused the complete program.
    Transition(dclutch_vm::v3::Error),
}

/// Result alias for General transition artifacts.
pub type Result<T> = core::result::Result<T, GeneralTransitionArtifactErrorV3>;

#[rustfmt::skip]
#[allow(missing_docs)]
#[path = "generated_transition_programs_v3.rs"]
mod generated;

pub use generated::*;

/// Borrow the exact Lean-authored TransitionVM program for one General action.
///
#[must_use]
pub const fn general_transition_program_bytes_lean_v3(action: Action) -> &'static [u8] {
    match action {
        Action::SubmitCandidate => &GENERAL_SUBMIT_CANDIDATE_TRANSITION_V3,
        Action::VerifyCandidateRow => &GENERAL_VERIFY_CANDIDATE_ROW_TRANSITION_V3,
        Action::OpenBatch => &GENERAL_OPEN_BATCH_TRANSITION_V3,
        Action::CloseBatch => &GENERAL_CLOSE_BATCH_TRANSITION_V3,
        Action::PlaceOrder => &GENERAL_PLACE_ORDER_TRANSITION_V3,
        Action::CancelOrder => &GENERAL_CANCEL_ORDER_TRANSITION_V3,
        Action::ReleaseOrder => &GENERAL_RELEASE_ORDER_TRANSITION_V3,
        Action::CloseCandidate => &GENERAL_CLOSE_CANDIDATE_TRANSITION_V3,
        Action::Consider => &GENERAL_CONSIDER_TRANSITION_V3,
        Action::Freeze => &GENERAL_FREEZE_TRANSITION_V3,
        Action::InitializeSettlement => &GENERAL_INITIALIZE_TRANSITION_V3,
        Action::Collect => &GENERAL_COLLECT_TRANSITION_V3,
        Action::Materialize => &GENERAL_MATERIALIZE_TRANSITION_V3,
        Action::Distribute => &GENERAL_DISTRIBUTE_TRANSITION_V3,
        Action::Close => &GENERAL_CLOSE_TRANSITION_V3,
    }
}

/// Harmless initializer for caller-owned instruction workspaces.
pub const GENERAL_TRANSITION_INSTRUCTION_PLACEHOLDER_V3: InstructionV3 =
    InstructionV3::load_const(ScalarRegisterV3::common(0), 0);

/// Return exact `(prelude, item, epilogue)` instruction counts.
///
/// Every prelude carries the two-instruction root-lifecycle conjunct added by
/// [`append_common`], so every authored count moved together when it landed.
#[must_use]
pub const fn general_transition_instruction_count_v3(action: Action) -> (usize, usize, usize) {
    match action {
        Action::SubmitCandidate => (46, 1, 0),
        Action::VerifyCandidateRow => (23, 1, 0),
        // Zero item instructions: no tail, so no bound check. See `append_item`.
        Action::OpenBatch => (26, 0, 0),
        Action::CloseBatch => (27, 0, 0),
        Action::PlaceOrder => (46, 4, 0),
        Action::CancelOrder => (50, 4, 0),
        Action::ReleaseOrder => (42, 4, 0),
        Action::CloseCandidate => (34, 1, 0),
        Action::Consider => (15, 1, 0),
        Action::Freeze => (21, 1, 0),
        Action::InitializeSettlement => (21, 2, 0),
        // Two more than before this lane, on each of the three actions whose
        // Custody direction is fixed at authoring time: the vault a transfer
        // draws on must be keyed by the identity the row names. See
        // `append_vault_context_binds`.
        Action::Collect | Action::Distribute => (21, 4, 0),
        Action::Materialize => (16, 1, 0),
        Action::Close => (27, 6, 0),
    }
}

/// Exact finalized program byte width for one action.
pub fn general_transition_program_bytes_v3(action: Action) -> Result<usize> {
    let (prelude, item, epilogue) = general_transition_instruction_count_v3(action);
    prelude
        .checked_add(item)
        .and_then(|value| value.checked_add(epilogue))
        .and_then(|count| count.checked_mul(INSTRUCTION_BYTES))
        .and_then(|body| HEADER_BYTES.checked_add(body))
        .ok_or(GeneralTransitionArtifactErrorV3::Geometry)
}

/// Encode one complete action-selected TransitionVM program atomically.
pub fn encode_general_transition_program_v3_atomic(
    action: Action,
    instruction_workspace: &mut [InstructionV3],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let (prelude_count, item_count, epilogue_count) =
        general_transition_instruction_count_v3(action);
    let instruction_count = prelude_count
        .checked_add(item_count)
        .and_then(|value| value.checked_add(epilogue_count))
        .ok_or(GeneralTransitionArtifactErrorV3::Geometry)?;
    let expected_bytes = general_transition_program_bytes_v3(action)?;
    if instruction_workspace.len() != instruction_count
        || scratch.len() != expected_bytes
        || output.len() != expected_bytes
    {
        return Err(GeneralTransitionArtifactErrorV3::Geometry);
    }

    let mut cursor = 0_usize;
    append_common(action, instruction_workspace, &mut cursor)?;
    append_action(action, instruction_workspace, &mut cursor)?;
    if cursor != prelude_count {
        return Err(GeneralTransitionArtifactErrorV3::Geometry);
    }
    append_item(action, instruction_workspace, &mut cursor)?;
    if cursor != instruction_count {
        return Err(GeneralTransitionArtifactErrorV3::Geometry);
    }

    let (prelude, rest) = instruction_workspace.split_at(prelude_count);
    let (item, epilogue) = rest.split_at(item_count);
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: narrow(GENERAL_HOT_COMMON_SCALARS_V3)?,
            item_scalar_stride: narrow(general_hot_item_scalar_stride_v3(action))?,
            common_identities: narrow(GENERAL_HOT_COMMON_IDENTITIES_V3)?,
            item_identity_stride: narrow(GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3)?,
        },
        prelude,
        item,
        epilogue,
        scratch,
        output,
    )
    .map_err(GeneralTransitionArtifactErrorV3::Transition)?;
    ProgramV3::decode(output).map_err(GeneralTransitionArtifactErrorV3::Transition)?;
    Ok(())
}

fn append_common(action: Action, output: &mut [InstructionV3], cursor: &mut usize) -> Result<()> {
    // The kind each action's PRIMARY state envelope carries; the arms mirror
    // `DClutchSemantics.GeneralTransitionV3.stateKind` exactly, and the byte
    // gate is what keeps them mirrored.
    let kind = match action {
        Action::Consider | Action::Freeze => GeneralLocalStateKindV3::Selection,
        // The register feeds the envelope an action CREATES (or, for a
        // non-creating action, nothing at all): PlaceOrder creates the ORDER
        // envelope even though its primary derived state is the batch window.
        Action::OpenBatch | Action::CancelOrder | Action::CloseBatch => {
            GeneralLocalStateKindV3::Batch
        }
        Action::SubmitCandidate | Action::VerifyCandidateRow | Action::CloseCandidate => {
            GeneralLocalStateKindV3::Candidate
        }
        Action::PlaceOrder | Action::ReleaseOrder => GeneralLocalStateKindV3::Order,
        _ => GeneralLocalStateKindV3::Settlement,
    };
    for instruction in [
        InstructionV3::load_const(s(scalar::ACTION)?, u64::from(action as u8)),
        // The capability must still accept work. `ROOT_LIFECYCLE_OBSERVATION`
        // is the AccountProfile's projection of the composite root's
        // `GeneralRootV2` lifecycle byte; this pair is the only thing on the
        // runtime-width path that reads it. An artifact that never projects it
        // leaves the register at zero, which is not `Active`, so the omission
        // refuses instead of passing.
        InstructionV3::load_const(
            s(scalar::ROOT_LIFECYCLE_ACTIVE)?,
            u64::from(GeneralLifecycleV2::Active.tag()),
        ),
        InstructionV3::scalar_eq(
            s(scalar::ROOT_LIFECYCLE_OBSERVATION)?,
            s(scalar::ROOT_LIFECYCLE_ACTIVE)?,
        ),
        InstructionV3::load_const(
            s(scalar::LOCAL_STATE_MAGIC)?,
            GeneralLocalStateLayoutV3::magic_u64(),
        ),
        InstructionV3::load_const(
            s(scalar::LOCAL_STATE_VERSION)?,
            u64::from(GeneralLocalStateLayoutV3::version_value()),
        ),
        InstructionV3::load_const(s(scalar::LOCAL_STATE_KIND)?, u64::from(kind.tag())),
        InstructionV3::nonzero(s(scalar::OUTCOME_COUNT)?),
        InstructionV3::scalar_eq(s(scalar::ZERO)?, s(scalar::OUTCOME_COUNT)?),
        InstructionV3::scalar_eq(s(scalar::STATE_BUMP)?, s(scalar::PRIMARY_CANONICAL_BUMP)?),
        InstructionV3::identity_eq(i(identity::PRIMARY_OWNER)?, i(identity::TRADING_PROGRAM)?),
        InstructionV3::nonzero(s(scalar::PRIMARY_RENT_PRINCIPAL)?),
    ] {
        push(output, cursor, instruction)?;
    }
    Ok(())
}

fn append_action(action: Action, output: &mut [InstructionV3], cursor: &mut usize) -> Result<()> {
    match action {
        Action::SubmitCandidate => {
            for instruction in [
                InstructionV3::load_const(s(scalar::ONE)?, 1),
                InstructionV3::nonzero(s(scalar::PRIMARY_CREATED)?),
                InstructionV3::identity_eq(i(identity::PRIMARY_BENEFICIARY)?, i(identity::OWNER)?),
                InstructionV3::identity_eq(i(identity::PAYER)?, i(identity::OWNER)?),
                InstructionV3::identity_eq(
                    i(identity::CANDIDATE)?,
                    i(identity::BEST_VERIFIED_DIGEST)?,
                ),
                InstructionV3::identity_eq(
                    i(identity::SELECTION_POLICY)?,
                    i(identity::SELECTION_BATCH)?,
                ),
                InstructionV3::identity_eq(i(identity::ORDER)?, i(identity::SELECTION_PRODUCT)?),
                InstructionV3::scalar_eq(
                    s(scalar::SELECTION_PRICE_SCALE)?,
                    s(scalar::ORDER_MAX_LOTS)?,
                ),
                InstructionV3::scalar_eq(s(scalar::ZERO)?, s(scalar::OUTCOME_COUNT)?),
                InstructionV3::scalar_eq(
                    s(scalar::BATCH_POST_ORDER_COUNT)?,
                    s(scalar::OUTCOME_COUNT)?,
                ),
                InstructionV3::identity_eq(
                    i(identity::RESULT_BENEFICIARY_OBSERVATION)?,
                    i(identity::CANDIDATE)?,
                ),
                InstructionV3::identity_eq(
                    i(identity::BENEFICIARY)?,
                    i(identity::SELECTION_BATCH)?,
                ),
                InstructionV3::scalar_eq(
                    s(scalar::VERIFY_POST_ORDER_COUNT)?,
                    s(scalar::OUTCOME_COUNT)?,
                ),
                InstructionV3::scalar_eq(
                    s(scalar::VERIFY_POST_PAGE)?,
                    s(scalar::CANDIDATE_PAGE_COUNT)?,
                ),
                InstructionV3::load_const(
                    s(scalar::CANDIDATE_POST_STATUS)?,
                    u64::from(GeneralCandidateStatusV1::Submitted.tag()),
                ),
                InstructionV3::scalar_eq(
                    s(scalar::CANDIDATE_STATUS_OBSERVATION)?,
                    s(scalar::CANDIDATE_POST_STATUS)?,
                ),
                InstructionV3::nonzero(s(scalar::SELECTION_BEST_CANDIDATE_COORDINATE)?),
                InstructionV3::nonzero(s(scalar::CANDIDATE_PAGE_REVISION)?),
                InstructionV3::nonzero(s(scalar::CANDIDATE_ROW_COUNT)?),
                InstructionV3::nonzero(s(scalar::CANDIDATE_REWARD_RATE)?),
                InstructionV3::scalar_le(
                    s(scalar::CANDIDATE_PAGE_COUNT)?,
                    s(scalar::CANDIDATE_ROW_COUNT)?,
                ),
                InstructionV3::load_const(
                    s(scalar::SCRATCH_A)?,
                    u64::from(BatchStatusV1::Closed.tag()),
                ),
                InstructionV3::scalar_eq(
                    s(scalar::BATCH_STATUS_OBSERVATION)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::scalar_le(
                    s(scalar::BATCH_COLLECTION_CLOSE_SLOT)?,
                    s(scalar::CURRENT_SLOT)?,
                ),
                InstructionV3::scalar_lt(
                    s(scalar::CURRENT_SLOT)?,
                    s(scalar::BATCH_SETTLEMENT_CLOSE_SLOT)?,
                ),
                InstructionV3::scalar_eq(
                    s(scalar::CANDIDATE_SUBMITTED_SLOT)?,
                    s(scalar::CURRENT_SLOT)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::CANDIDATE_ROW_COUNT)?,
                    s(scalar::CANDIDATE_POST_VERIFICATION_REMAINING)?,
                ),
                InstructionV3::checked_mul_into(
                    s(scalar::CANDIDATE_POST_VERIFICATION_REMAINING)?,
                    s(scalar::CANDIDATE_REWARD_RATE)?,
                    s(scalar::CANDIDATE_POST_VERIFICATION_REMAINING)?,
                ),
                InstructionV3::copy_scalar(
                    s(scalar::CANDIDATE_REWARD_RATE)?,
                    s(scalar::CANDIDATE_POST_CLEANUP_REMAINING)?,
                ),
                InstructionV3::scalar_eq(
                    s(scalar::CANDIDATE_VERIFICATION_REMAINING_OBSERVATION)?,
                    s(scalar::CANDIDATE_POST_VERIFICATION_REMAINING)?,
                ),
                InstructionV3::scalar_eq(
                    s(scalar::CANDIDATE_CLEANUP_REMAINING_OBSERVATION)?,
                    s(scalar::CANDIDATE_POST_CLEANUP_REMAINING)?,
                ),
                InstructionV3::checked_add_into(
                    s(scalar::CANDIDATE_POST_VERIFICATION_REMAINING)?,
                    s(scalar::CANDIDATE_POST_CLEANUP_REMAINING)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::checked_add_into(
                    s(scalar::SCRATCH_A)?,
                    s(scalar::PRIMARY_RENT_PRINCIPAL)?,
                    s(scalar::SCRATCH_B)?,
                ),
                InstructionV3::load_const(
                    s(scalar::VERIFY_REVISION_OBSERVATION)?,
                    u64::from_le_bytes(GeneralCandidateLayoutV1::MAGIC),
                ),
                InstructionV3::load_const(
                    s(scalar::VERIFY_POST_REVISION)?,
                    u64::from(GeneralCandidateLayoutV1::PHASE),
                ),
            ] {
                push(output, cursor, instruction)?;
            }
        }
        Action::VerifyCandidateRow => {
            for instruction in [
                // Candidate is an authenticated existing envelope and only its
                // Submitted phase admits one verifier row.
                InstructionV3::load_const(s(scalar::ONE)?, 1),
                InstructionV3::load_const(
                    s(scalar::SCRATCH_A)?,
                    u64::from(GeneralCandidateStatusV1::Submitted.tag()),
                ),
                InstructionV3::scalar_eq(
                    s(scalar::CANDIDATE_STATUS_OBSERVATION)?,
                    s(scalar::SCRATCH_A)?,
                ),
                // The request subject and optimistic coordinates are exact
                // joins to authenticated Candidate/verifier evidence. The
                // request profile maps expected_revision to scalar 94 because
                // scalar zero is overwritten with ACTION before this fold.
                InstructionV3::identity_eq(
                    i(identity::PARENT_REQUEST_DIGEST)?,
                    i(identity::CANDIDATE)?,
                ),
                InstructionV3::scalar_eq(
                    s(scalar::ROOT_EXPECTED_REVISION)?,
                    s(scalar::VERIFY_REVISION_OBSERVATION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::VERIFY_REVISION_OBSERVATION)?,
                    s(scalar::VERIFY_POST_REVISION)?,
                ),
                InstructionV3::scalar_eq(
                    s(scalar::COMPLETE_SET_MOVE)?,
                    s(scalar::VERIFY_PAGE_OBSERVATION)?,
                ),
                InstructionV3::scalar_eq(
                    s(scalar::CLAIMS_AFFINE_ACTIVE)?,
                    s(scalar::VERIFY_ROW_OBSERVATION)?,
                ),
                // A row may continue the current globally grouped order or
                // begin exactly one new order; it may neither erase nor skip.
                InstructionV3::scalar_le(
                    s(scalar::VERIFY_ORDER_COUNT_OBSERVATION)?,
                    s(scalar::VERIFY_POST_ORDER_COUNT)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::VERIFY_ORDER_COUNT_OBSERVATION)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::scalar_le(
                    s(scalar::VERIFY_POST_ORDER_COUNT)?,
                    s(scalar::SCRATCH_A)?,
                ),
                // Lifecycle V5 alone consumes this boolean as its raw
                // certificate Create guard; no manifest or economics are
                // written by this transition.
                InstructionV3::scalar_le(s(scalar::VERIFY_TERMINAL)?, s(scalar::ONE)?),
            ] {
                push(output, cursor, instruction)?;
            }
        }
        Action::CloseCandidate => {
            for instruction in [
                InstructionV3::load_const(s(scalar::ONE)?, 1),
                InstructionV3::identity_eq(
                    i(identity::PARENT_REQUEST_DIGEST)?,
                    i(identity::CANDIDATE)?,
                ),
                InstructionV3::identity_eq(i(identity::PRIMARY_BENEFICIARY)?, i(identity::OWNER)?),
                InstructionV3::identity_eq(i(identity::RENT_CREDIT)?, i(identity::OWNER)?),
                InstructionV3::nonzero(s(scalar::CANDIDATE_REWARD_RATE)?),
                InstructionV3::scalar_eq(
                    s(scalar::CANDIDATE_CLEANUP_REMAINING_OBSERVATION)?,
                    s(scalar::CANDIDATE_REWARD_RATE)?,
                ),
                InstructionV3::checked_add_into(
                    s(scalar::CANDIDATE_VERIFICATION_REMAINING_OBSERVATION)?,
                    s(scalar::CANDIDATE_CLEANUP_REMAINING_OBSERVATION)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::checked_add_into(
                    s(scalar::SCRATCH_A)?,
                    s(scalar::PRIMARY_RENT_PRINCIPAL)?,
                    s(scalar::SCRATCH_B)?,
                ),
                InstructionV3::scalar_eq(
                    s(scalar::OBSERVED_POSITION_LAMPORTS)?,
                    s(scalar::SCRATCH_B)?,
                ),
                InstructionV3::load_const(
                    s(scalar::SCRATCH_A)?,
                    u64::from(GeneralCandidateStatusV1::Considered.tag()),
                ),
                InstructionV3::nonzero(s(scalar::CANDIDATE_STATUS_OBSERVATION)?),
                InstructionV3::scalar_le(
                    s(scalar::CANDIDATE_STATUS_OBSERVATION)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::min_into(
                    s(scalar::CANDIDATE_STATUS_OBSERVATION)?,
                    s(scalar::SCRATCH_A)?,
                    s(scalar::SCRATCH_B)?,
                ),
                InstructionV3::sub_into(
                    s(scalar::SCRATCH_A)?,
                    s(scalar::SCRATCH_B)?,
                    s(scalar::SCRATCH_B)?,
                ),
                InstructionV3::min_into(
                    s(scalar::SCRATCH_B)?,
                    s(scalar::ONE)?,
                    s(scalar::SCRATCH_B)?,
                ),
                InstructionV3::min_into(
                    s(scalar::CURRENT_SLOT)?,
                    s(scalar::BATCH_SETTLEMENT_CLOSE_SLOT)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::sub_into(
                    s(scalar::BATCH_SETTLEMENT_CLOSE_SLOT)?,
                    s(scalar::SCRATCH_A)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::min_into(
                    s(scalar::SCRATCH_A)?,
                    s(scalar::ONE)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::checked_mul_into(
                    s(scalar::SCRATCH_A)?,
                    s(scalar::SCRATCH_B)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::load_const(s(scalar::SCRATCH_B)?, 0),
                InstructionV3::scalar_eq(s(scalar::SCRATCH_A)?, s(scalar::SCRATCH_B)?),
                InstructionV3::load_const(
                    s(scalar::SCRATCH_A)?,
                    u64::from(BatchStatusV1::Closed.tag()),
                ),
                InstructionV3::scalar_eq(
                    s(scalar::BATCH_STATUS_OBSERVATION)?,
                    s(scalar::SCRATCH_A)?,
                ),
            ] {
                push(output, cursor, instruction)?;
            }
        }
        Action::Consider => {
            for instruction in [
                InstructionV3::load_const(
                    s(scalar::SELECTION_MAGIC)?,
                    RuntimeSelectionLayoutV2::magic_u64(),
                ),
                InstructionV3::load_const(
                    s(scalar::RUNTIME_WIDTH_VERSION)?,
                    u64::from(RuntimeSelectionLayoutV2::version_value()),
                ),
                InstructionV3::load_const(
                    s(scalar::SELECTION_PHASE)?,
                    u64::from(RuntimeSelectionPhaseV2::Open.tag()),
                ),
                InstructionV3::nonzero(s(scalar::SELECTION_REVISION)?),
            ] {
                push(output, cursor, instruction)?;
            }
        }
        Action::Freeze => {
            for instruction in [
                InstructionV3::load_const(
                    s(scalar::SELECTION_MAGIC)?,
                    RuntimeSelectionLayoutV2::magic_u64(),
                ),
                InstructionV3::load_const(
                    s(scalar::RUNTIME_WIDTH_VERSION)?,
                    u64::from(RuntimeSelectionLayoutV2::version_value()),
                ),
                InstructionV3::load_const(
                    s(scalar::SELECTION_PHASE)?,
                    u64::from(RuntimeSelectionPhaseV2::Frozen.tag()),
                ),
                InstructionV3::nonzero(s(scalar::SELECTION_REVISION)?),
                InstructionV3::nonzero(s(scalar::SELECTION_BEST_CANDIDATE_COORDINATE)?),
                InstructionV3::nonzero(s(scalar::SELECTION_BEST_VERIFIED_REVISION)?),
                // The selection window has to be over. See the `.freeze` arm of
                // `GeneralTransitionV3.lean`: the deadline is the batch's own
                // collection close plus the config's selection window, which is
                // the same sum `OpenBatch` forms when it derives the settlement
                // close, and the two `nonzero`s are fail-closed guards on the
                // projections that source it rather than restatements of what
                // the config's constructor already refuses.
                InstructionV3::nonzero(s(scalar::BATCH_COLLECTION_CLOSE_SLOT)?),
                InstructionV3::nonzero(s(scalar::CONFIG_SELECTION_SLOTS)?),
                InstructionV3::checked_add_into(
                    s(scalar::BATCH_COLLECTION_CLOSE_SLOT)?,
                    s(scalar::CONFIG_SELECTION_SLOTS)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::scalar_le(s(scalar::SCRATCH_A)?, s(scalar::CURRENT_SLOT)?),
            ] {
                push(output, cursor, instruction)?;
            }
        }
        Action::InitializeSettlement => {
            for instruction in [
                InstructionV3::load_const(s(scalar::ZERO)?, 0),
                InstructionV3::load_const(
                    s(scalar::CURSOR_MAGIC)?,
                    SettlementCursorLayoutV2::magic_u64(),
                ),
                InstructionV3::load_const(
                    s(scalar::RUNTIME_WIDTH_VERSION)?,
                    u64::from(SettlementCursorLayoutV2::version_value()),
                ),
                InstructionV3::load_const(
                    s(scalar::CURSOR_PHASE)?,
                    u64::from(SettlementPhaseV2::Collecting.tag()),
                ),
                InstructionV3::load_const(s(scalar::CURSOR_NEXT_ORDER)?, 0),
                InstructionV3::load_const(s(scalar::CURSOR_RESULTING_REVISION)?, 1),
                InstructionV3::load_const(s(scalar::CURSOR_QUOTE_INVENTORY)?, 0),
                InstructionV3::load_const(s(scalar::CURSOR_TERMINAL_COORDINATE)?, 0),
                InstructionV3::increment_into(
                    s(scalar::CUSTODY_EXPECTED_REVISION)?,
                    s(scalar::CUSTODY_RESULTING_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::CLAIMS_MARKET_REVISION)?,
                    s(scalar::CLAIMS_POST_MARKET_REVISION)?,
                ),
            ] {
                push(output, cursor, instruction)?;
            }
        }
        Action::Collect | Action::Distribute => {
            append_row_action(output, cursor)?;
            append_vault_context_binds(action, output, cursor)?;
        }
        Action::Materialize => {
            for instruction in [
                InstructionV3::nonzero(s(scalar::SETTLEMENT_POSITION_PRESENT)?),
                InstructionV3::increment_into(
                    s(scalar::SETTLEMENT_REVISION)?,
                    s(scalar::CURSOR_RESULTING_REVISION)?,
                ),
                InstructionV3::load_const(s(scalar::CUSTODY_OPERATION)?, 2),
                InstructionV3::increment_into(
                    s(scalar::CLAIMS_MARKET_REVISION)?,
                    s(scalar::CLAIMS_POST_MARKET_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::SETTLEMENT_POSITION_REVISION)?,
                    s(scalar::SETTLEMENT_POST_POSITION_REVISION)?,
                ),
            ] {
                push(output, cursor, instruction)?;
            }
        }
        Action::Close => {
            for instruction in [
                InstructionV3::nonzero(s(scalar::SETTLEMENT_POSITION_PRESENT)?),
                InstructionV3::increment_into(
                    s(scalar::SETTLEMENT_REVISION)?,
                    s(scalar::CURSOR_RESULTING_REVISION)?,
                ),
                InstructionV3::load_const(s(scalar::TERMINAL)?, 1),
                InstructionV3::load_const(
                    s(scalar::CURSOR_PHASE)?,
                    u64::from(SettlementPhaseV2::Terminal.tag()),
                ),
                InstructionV3::scalar_eq(
                    s(scalar::TERMINAL_RECORD_BUMP)?,
                    s(scalar::TERMINAL_CANONICAL_BUMP)?,
                ),
                InstructionV3::identity_eq(
                    i(identity::TERMINAL_OWNER)?,
                    i(identity::TRADING_PROGRAM)?,
                ),
                InstructionV3::nonzero(s(scalar::TERMINAL_RENT_PRINCIPAL)?),
                InstructionV3::load_const(s(scalar::ZERO)?, 0),
                InstructionV3::scalar_eq(s(scalar::POSITION_TABLE_COUNT)?, s(scalar::ZERO)?),
                InstructionV3::increment_into(
                    s(scalar::CUSTODY_EXPECTED_REVISION)?,
                    s(scalar::CUSTODY_CLOSE_VAULT_EXPECTED_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::CUSTODY_CLOSE_VAULT_EXPECTED_REVISION)?,
                    s(scalar::CUSTODY_CLOSE_VAULT_RESULTING_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::CUSTODY_CLOSE_VAULT_RESULTING_REVISION)?,
                    s(scalar::CUSTODY_CLOSE_REPLAY_RESULTING_REVISION)?,
                ),
                InstructionV3::nonzero(s(scalar::CURSOR_TERMINAL_COORDINATE)?),
                InstructionV3::nonzero(s(scalar::TERMINAL_COORDINATE)?),
            ] {
                push(output, cursor, instruction)?;
            }
            append_vault_context_binds(action, output, cursor)?;
        }
        Action::OpenBatch => {
            for instruction in [
                // `GeneralRootV2::open_batch` as conjuncts: the caller's
                // optimistic revision must be the observed one, and the three
                // root advances are exact. The EffectProgram writes the
                // successor root tail from the POST registers, so a skipped
                // increment is a root that replays.
                InstructionV3::scalar_eq(
                    s(scalar::ROOT_EXPECTED_REVISION)?,
                    s(scalar::ROOT_REVISION_OBSERVATION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::ROOT_REVISION_OBSERVATION)?,
                    s(scalar::ROOT_POST_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::ROOT_NEXT_BATCH_SEQUENCE_OBSERVATION)?,
                    s(scalar::ROOT_POST_BATCH_SEQUENCE)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::ROOT_OPEN_BATCHES_OBSERVATION)?,
                    s(scalar::ROOT_POST_OPEN_BATCHES)?,
                ),
                // A zero window or admission bound is a batch that can admit
                // nothing and never close by fullness.
                InstructionV3::nonzero(s(scalar::CONFIG_COLLECTION_SLOTS)?),
                InstructionV3::nonzero(s(scalar::CONFIG_SELECTION_SLOTS)?),
                InstructionV3::nonzero(s(scalar::CONFIG_SETTLEMENT_SLOTS)?),
                InstructionV3::nonzero(s(scalar::CONFIG_MAX_ORDERS)?),
                // The windows are config-derived, never caller-chosen.
                InstructionV3::checked_add_into(
                    s(scalar::CURRENT_SLOT)?,
                    s(scalar::CONFIG_COLLECTION_SLOTS)?,
                    s(scalar::BATCH_COLLECTION_CLOSE_SLOT)?,
                ),
                InstructionV3::checked_add_into(
                    s(scalar::BATCH_COLLECTION_CLOSE_SLOT)?,
                    s(scalar::CONFIG_SELECTION_SLOTS)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::checked_add_into(
                    s(scalar::SCRATCH_A)?,
                    s(scalar::CONFIG_SETTLEMENT_SLOTS)?,
                    s(scalar::BATCH_SETTLEMENT_CLOSE_SLOT)?,
                ),
                // Record constants the EffectProgram writes into the vacant
                // account. `ONE` doubles as the record version, which is 1.
                InstructionV3::load_const(
                    s(scalar::BATCH_POST_STATUS)?,
                    u64::from(BatchStatusV1::Collecting.tag()),
                ),
                InstructionV3::load_const(
                    s(scalar::ONE)?,
                    u64::from(GeneralBatchLayoutV1::version_value()),
                ),
                InstructionV3::load_const(s(scalar::SCRATCH_A)?, GeneralBatchLayoutV1::magic_u64()),
                InstructionV3::load_const(
                    s(scalar::SCRATCH_B)?,
                    u64::from(GeneralBatchLayoutV1::phase_value()),
                ),
            ] {
                push(output, cursor, instruction)?;
            }
        }
        Action::CloseBatch => {
            for instruction in [
                // `GeneralRootV2::close_batch` as conjuncts: revision +1 and
                // an exact open-batch decrement -- `sub_into` refuses at zero,
                // which is the root refusing a close it never opened.
                InstructionV3::scalar_eq(
                    s(scalar::ROOT_EXPECTED_REVISION)?,
                    s(scalar::ROOT_REVISION_OBSERVATION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::ROOT_REVISION_OBSERVATION)?,
                    s(scalar::ROOT_POST_REVISION)?,
                ),
                InstructionV3::load_const(s(scalar::ONE)?, 1),
                InstructionV3::sub_into(
                    s(scalar::ROOT_OPEN_BATCHES_OBSERVATION)?,
                    s(scalar::ONE)?,
                    s(scalar::ROOT_POST_OPEN_BATCHES)?,
                ),
                // Only a collecting batch closes, and it closes to Closed.
                InstructionV3::load_const(
                    s(scalar::SCRATCH_A)?,
                    u64::from(BatchStatusV1::Collecting.tag()),
                ),
                InstructionV3::scalar_eq(
                    s(scalar::BATCH_STATUS_OBSERVATION)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::load_const(
                    s(scalar::BATCH_POST_STATUS)?,
                    u64::from(BatchStatusV1::Closed.tag()),
                ),
                // `close_is_permissionless`: the window is over OR the batch
                // is full. A disjunction over an all-conjunct vocabulary:
                //   d1 := close - min(now, close)   (zero iff the window is over)
                //   d2 := bound - min(count, bound) (zero iff the batch is full)
                //   min(d1,1) * min(d2,1) = 0
                // Anything else truncates a maker's window.
                InstructionV3::min_into(
                    s(scalar::CURRENT_SLOT)?,
                    s(scalar::BATCH_COLLECTION_CLOSE_SLOT)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::sub_into(
                    s(scalar::BATCH_COLLECTION_CLOSE_SLOT)?,
                    s(scalar::SCRATCH_A)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::min_into(
                    s(scalar::SCRATCH_A)?,
                    s(scalar::ONE)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::min_into(
                    s(scalar::BATCH_ORDER_COUNT_OBSERVATION)?,
                    s(scalar::CONFIG_MAX_ORDERS)?,
                    s(scalar::SCRATCH_B)?,
                ),
                InstructionV3::sub_into(
                    s(scalar::CONFIG_MAX_ORDERS)?,
                    s(scalar::SCRATCH_B)?,
                    s(scalar::SCRATCH_B)?,
                ),
                InstructionV3::min_into(
                    s(scalar::SCRATCH_B)?,
                    s(scalar::ONE)?,
                    s(scalar::SCRATCH_B)?,
                ),
                InstructionV3::checked_mul_into(
                    s(scalar::SCRATCH_A)?,
                    s(scalar::SCRATCH_B)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::load_const(s(scalar::SCRATCH_B)?, 0),
                InstructionV3::scalar_eq(s(scalar::SCRATCH_A)?, s(scalar::SCRATCH_B)?),
            ] {
                push(output, cursor, instruction)?;
            }
        }
        Action::PlaceOrder => {
            for instruction in [
                // The second derived state -- the order record this admission
                // CREATES -- anchored exactly as a created secondary always is.
                InstructionV3::scalar_eq(
                    s(scalar::TERMINAL_RECORD_BUMP)?,
                    s(scalar::TERMINAL_CANONICAL_BUMP)?,
                ),
                InstructionV3::identity_eq(
                    i(identity::TERMINAL_OWNER)?,
                    i(identity::TRADING_PROGRAM)?,
                ),
                InstructionV3::nonzero(s(scalar::TERMINAL_RENT_PRINCIPAL)?),
                // The signed terms' width must be the Product width the
                // prelude already equated with the batch's.
                InstructionV3::scalar_eq(s(scalar::SCRATCH_A)?, s(scalar::OUTCOME_COUNT)?),
                // Admission: a COLLECTING batch, inside its window, under its
                // bound.
                InstructionV3::load_const(s(scalar::ONE)?, 1),
                InstructionV3::scalar_eq(s(scalar::BATCH_STATUS_OBSERVATION)?, s(scalar::ONE)?),
                InstructionV3::scalar_lt(
                    s(scalar::CURRENT_SLOT)?,
                    s(scalar::BATCH_COLLECTION_CLOSE_SLOT)?,
                ),
                InstructionV3::scalar_lt(
                    s(scalar::BATCH_ORDER_COUNT_OBSERVATION)?,
                    s(scalar::CONFIG_MAX_ORDERS)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::BATCH_ORDER_COUNT_OBSERVATION)?,
                    s(scalar::BATCH_POST_ORDER_COUNT)?,
                ),
                // THE EXPIRY PIN (recorded choice 6): the signed
                // valid_until_slot IS the batch's settlement close, exactly.
                InstructionV3::scalar_eq(
                    s(scalar::ORDER_VALID_UNTIL_SLOT)?,
                    s(scalar::BATCH_SETTLEMENT_CLOSE_SLOT)?,
                ),
                // The escrow the admission MOVES: the exact worst case, into
                // the order's own vault, and the batch commits exactly that.
                InstructionV3::nonzero(s(scalar::ORDER_MAX_LOTS)?),
                InstructionV3::checked_mul_into(
                    s(scalar::ORDER_MAX_LOTS)?,
                    s(scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT)?,
                    s(scalar::ORDER_QUOTE_RESERVE)?,
                ),
                InstructionV3::copy_scalar(
                    s(scalar::ORDER_QUOTE_RESERVE)?,
                    s(scalar::CUSTODY_AMOUNT)?,
                ),
                InstructionV3::checked_add_into(
                    s(scalar::BATCH_QUOTE_RESERVE_OBSERVATION)?,
                    s(scalar::ORDER_QUOTE_RESERVE)?,
                    s(scalar::BATCH_POST_QUOTE_RESERVE)?,
                ),
                // Record constants the EffectProgram writes into the vacant
                // account. SCRATCH_A carried the signed terms' width until the
                // equality above consumed it; from here it carries the record
                // magic.
                InstructionV3::load_const(
                    s(scalar::ORDER_POST_PHASE)?,
                    u64::from(GeneralOrderPhaseV1::Placed.tag()),
                ),
                InstructionV3::load_const(
                    s(scalar::SCRATCH_B)?,
                    u64::from(GeneralOrderLayoutV1::phase_value()),
                ),
                InstructionV3::load_const(s(scalar::SCRATCH_A)?, GeneralOrderLayoutV1::magic_u64()),
                // The quote-deposit route's guard is a PROVEN consequence of
                // the signed terms: active exactly when the reserve is
                // nonzero.
                InstructionV3::min_into(
                    s(scalar::ORDER_QUOTE_RESERVE)?,
                    s(scalar::ONE)?,
                    s(scalar::CUSTODY_ACTIVE)?,
                ),
                // The claims escrow-in: maker source (index zero) to the
                // freshly admitted escrow Position (index one), nothing
                // minted. A Position admit does not advance the Claims market,
                // so the escrow transfer expects the same observation the
                // admit did; a freshly admitted Position's revision is ZERO.
                InstructionV3::increment_into(
                    s(scalar::CLAIMS_MARKET_REVISION)?,
                    s(scalar::CLAIMS_POST_MARKET_REVISION)?,
                ),
                InstructionV3::load_const(s(scalar::POSITION_ONE_REVISION)?, 0),
                InstructionV3::load_const(s(scalar::CLAIMS_SOURCE_PRESENT)?, 1),
                InstructionV3::load_const(s(scalar::CLAIMS_DESTINATION_PRESENT)?, 1),
                InstructionV3::load_const(s(scalar::CLAIMS_SOURCE_POSITION_INDEX)?, 0),
                InstructionV3::load_const(s(scalar::CLAIMS_DESTINATION_POSITION_INDEX)?, 1),
                InstructionV3::load_const(
                    s(scalar::CLAIMS_AGGREGATE_DIRECTION)?,
                    DeltaDirectionV2::Neutral as u64,
                ),
                InstructionV3::load_const(
                    s(scalar::CLAIMS_SOURCE_DIRECTION)?,
                    DeltaDirectionV2::Debit as u64,
                ),
                InstructionV3::load_const(
                    s(scalar::CLAIMS_DESTINATION_DIRECTION)?,
                    DeltaDirectionV2::Credit as u64,
                ),
                // The 0010 SS2a addressing discipline, deposit direction:
                // atoms leave the MAKER's external account and claims leave
                // the MAKER's Position, and both arrive at addresses keyed by
                // the order's own identity.
                InstructionV3::identity_eq(
                    i(identity::DESTINATION_VAULT_CONTEXT)?,
                    i(identity::ORDER)?,
                ),
                InstructionV3::identity_eq(i(identity::CUSTODY_SOURCE_OWNER)?, i(identity::OWNER)?),
                InstructionV3::identity_eq(i(identity::POSITION_ZERO_OWNER)?, i(identity::OWNER)?),
                InstructionV3::identity_eq(i(identity::POSITION_ONE_OWNER)?, i(identity::ORDER)?),
                InstructionV3::identity_eq(
                    i(identity::SETTLEMENT_POSITION_OWNER)?,
                    i(identity::ORDER)?,
                ),
                InstructionV3::identity_eq(i(identity::RENT_CREDIT)?, i(identity::OWNER)?),
                InstructionV3::identity_eq(i(identity::PAYER)?, i(identity::OWNER)?),
                InstructionV3::load_const(
                    s(scalar::CUSTODY_OPERATION)?,
                    OperationV1::Transfer as u64,
                ),
            ] {
                push(output, cursor, instruction)?;
            }
        }
        Action::CancelOrder => {
            for instruction in [
                // The second derived state: the order the maker is cancelling,
                // anchored the way settlement Close anchors its terminal
                // record.
                InstructionV3::scalar_eq(
                    s(scalar::TERMINAL_RECORD_BUMP)?,
                    s(scalar::TERMINAL_CANONICAL_BUMP)?,
                ),
                InstructionV3::identity_eq(
                    i(identity::TERMINAL_OWNER)?,
                    i(identity::TRADING_PROGRAM)?,
                ),
                InstructionV3::nonzero(s(scalar::TERMINAL_RENT_PRINCIPAL)?),
                // The order record's width must be the Product width the
                // prelude already equated with the batch's.
                InstructionV3::scalar_eq(s(scalar::SCRATCH_A)?, s(scalar::OUTCOME_COUNT)?),
                // Only while the batch COLLECTS, and only a PLACED order.
                // `Collecting.tag()` and `Placed.tag()` are both one by
                // construction, so one constant serves both conjuncts.
                InstructionV3::load_const(
                    s(scalar::SCRATCH_B)?,
                    u64::from(BatchStatusV1::Collecting.tag()),
                ),
                InstructionV3::scalar_eq(
                    s(scalar::BATCH_STATUS_OBSERVATION)?,
                    s(scalar::SCRATCH_B)?,
                ),
                InstructionV3::scalar_eq(
                    s(scalar::ORDER_PHASE_OBSERVATION)?,
                    s(scalar::SCRATCH_B)?,
                ),
                InstructionV3::scalar_lt(
                    s(scalar::CURRENT_SLOT)?,
                    s(scalar::BATCH_COLLECTION_CLOSE_SLOT)?,
                ),
                InstructionV3::load_const(
                    s(scalar::ORDER_POST_PHASE)?,
                    u64::from(GeneralOrderPhaseV1::Cancelled.tag()),
                ),
                InstructionV3::copy_scalar(
                    s(scalar::CURRENT_SLOT)?,
                    s(scalar::ORDER_POST_RELEASED_SLOT)?,
                ),
                InstructionV3::scalar_le(
                    s(scalar::ORDER_ADMITTED_SLOT_OBSERVATION)?,
                    s(scalar::CURRENT_SLOT)?,
                ),
                // The refund is the WHOLE reserve, exactly; the batch counter
                // surrenders exactly what admission committed.
                InstructionV3::checked_mul_into(
                    s(scalar::ORDER_MAX_LOTS)?,
                    s(scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT)?,
                    s(scalar::ORDER_QUOTE_RESERVE)?,
                ),
                InstructionV3::copy_scalar(
                    s(scalar::ORDER_QUOTE_RESERVE)?,
                    s(scalar::CUSTODY_AMOUNT)?,
                ),
                InstructionV3::sub_into(
                    s(scalar::BATCH_QUOTE_RESERVE_OBSERVATION)?,
                    s(scalar::ORDER_QUOTE_RESERVE)?,
                    s(scalar::BATCH_POST_QUOTE_RESERVE)?,
                ),
                // One more cancellation, never more than admissions.
                InstructionV3::scalar_lt(
                    s(scalar::BATCH_CANCELLED_COUNT_OBSERVATION)?,
                    s(scalar::BATCH_ORDER_COUNT_OBSERVATION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::BATCH_CANCELLED_COUNT_OBSERVATION)?,
                    s(scalar::BATCH_POST_CANCELLED_COUNT)?,
                ),
                // The teardown chain and the claims refund, exactly as
                // ReleaseOrder's.
                InstructionV3::increment_into(
                    s(scalar::CUSTODY_EXPECTED_REVISION)?,
                    s(scalar::CUSTODY_RESULTING_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::CUSTODY_RESULTING_REVISION)?,
                    s(scalar::CUSTODY_CLOSE_VAULT_EXPECTED_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::CUSTODY_CLOSE_VAULT_EXPECTED_REVISION)?,
                    s(scalar::CUSTODY_CLOSE_VAULT_RESULTING_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::CUSTODY_CLOSE_VAULT_RESULTING_REVISION)?,
                    s(scalar::CUSTODY_CLOSE_REPLAY_RESULTING_REVISION)?,
                ),
                InstructionV3::load_const(
                    s(scalar::CUSTODY_OPERATION)?,
                    OperationV1::Transfer as u64,
                ),
                InstructionV3::increment_into(
                    s(scalar::CLAIMS_MARKET_REVISION)?,
                    s(scalar::CLAIMS_POST_MARKET_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::POSITION_ZERO_REVISION)?,
                    s(scalar::SETTLEMENT_POSITION_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::SETTLEMENT_POSITION_REVISION)?,
                    s(scalar::SETTLEMENT_POST_POSITION_REVISION)?,
                ),
                InstructionV3::load_const(s(scalar::CLAIMS_SOURCE_PRESENT)?, 1),
                InstructionV3::load_const(s(scalar::CLAIMS_DESTINATION_PRESENT)?, 1),
                InstructionV3::load_const(s(scalar::CLAIMS_SOURCE_POSITION_INDEX)?, 0),
                InstructionV3::load_const(s(scalar::CLAIMS_DESTINATION_POSITION_INDEX)?, 1),
                InstructionV3::load_const(
                    s(scalar::CLAIMS_AGGREGATE_DIRECTION)?,
                    DeltaDirectionV2::Neutral as u64,
                ),
                InstructionV3::load_const(
                    s(scalar::CLAIMS_SOURCE_DIRECTION)?,
                    DeltaDirectionV2::Debit as u64,
                ),
                InstructionV3::load_const(
                    s(scalar::CLAIMS_DESTINATION_DIRECTION)?,
                    DeltaDirectionV2::Credit as u64,
                ),
                // The 0010 SS2a addressing discipline, identical to
                // ReleaseOrder's.
                InstructionV3::identity_eq(i(identity::SOURCE_VAULT_CONTEXT)?, i(identity::ORDER)?),
                InstructionV3::identity_eq(
                    i(identity::CUSTODY_DESTINATION_OWNER)?,
                    i(identity::OWNER)?,
                ),
                InstructionV3::identity_eq(i(identity::POSITION_ZERO_OWNER)?, i(identity::ORDER)?),
                InstructionV3::identity_eq(i(identity::POSITION_ONE_OWNER)?, i(identity::OWNER)?),
                InstructionV3::identity_eq(
                    i(identity::SETTLEMENT_POSITION_OWNER)?,
                    i(identity::ORDER)?,
                ),
                InstructionV3::identity_eq(i(identity::RENT_CREDIT)?, i(identity::OWNER)?),
                InstructionV3::identity_eq(i(identity::RENT_REFUND)?, i(identity::OWNER)?),
                InstructionV3::identity_eq(i(identity::PAYER)?, i(identity::OWNER)?),
            ] {
                push(output, cursor, instruction)?;
            }
        }
        Action::ReleaseOrder => {
            for instruction in [
                // Only a placed order releases, and it releases to Released. A
                // vacant state account projects phase zero, which is not
                // Placed, so a release aimed at an unoccupied address refuses.
                InstructionV3::load_const(
                    s(scalar::SCRATCH_A)?,
                    u64::from(GeneralOrderPhaseV1::Placed.tag()),
                ),
                InstructionV3::scalar_eq(
                    s(scalar::ORDER_PHASE_OBSERVATION)?,
                    s(scalar::SCRATCH_A)?,
                ),
                InstructionV3::load_const(
                    s(scalar::ORDER_POST_PHASE)?,
                    u64::from(GeneralOrderPhaseV1::Released.tag()),
                ),
                // The window gate, from the order alone: PlaceOrder pins the
                // signed valid_until_slot to the batch's settlement close
                // EXACTLY. Batch release admits the pinned close slot itself,
                // so the order gate uses the same inclusive boundary without
                // bringing a batch account into the frame.
                InstructionV3::scalar_le(
                    s(scalar::ORDER_VALID_UNTIL_SLOT)?,
                    s(scalar::CURRENT_SLOT)?,
                ),
                InstructionV3::copy_scalar(
                    s(scalar::CURRENT_SLOT)?,
                    s(scalar::ORDER_POST_RELEASED_SLOT)?,
                ),
                InstructionV3::scalar_le(
                    s(scalar::ORDER_ADMITTED_SLOT_OBSERVATION)?,
                    s(scalar::CURRENT_SLOT)?,
                ),
                // The residual is the OBSERVED vault balance -- never computed
                // -- and it can never exceed the exact worst case admission
                // escrowed.
                InstructionV3::checked_mul_into(
                    s(scalar::ORDER_MAX_LOTS)?,
                    s(scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT)?,
                    s(scalar::ORDER_QUOTE_RESERVE)?,
                ),
                InstructionV3::scalar_le(
                    s(scalar::ESCROW_BALANCE_OBSERVATION)?,
                    s(scalar::ORDER_QUOTE_RESERVE)?,
                ),
                InstructionV3::copy_scalar(
                    s(scalar::ESCROW_BALANCE_OBSERVATION)?,
                    s(scalar::CUSTODY_AMOUNT)?,
                ),
                // The four-route close suite advances one revision per Custody
                // operation, the same chain settlement Close carries.
                InstructionV3::increment_into(
                    s(scalar::CUSTODY_EXPECTED_REVISION)?,
                    s(scalar::CUSTODY_RESULTING_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::CUSTODY_RESULTING_REVISION)?,
                    s(scalar::CUSTODY_CLOSE_VAULT_EXPECTED_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::CUSTODY_CLOSE_VAULT_EXPECTED_REVISION)?,
                    s(scalar::CUSTODY_CLOSE_VAULT_RESULTING_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::CUSTODY_CLOSE_VAULT_RESULTING_REVISION)?,
                    s(scalar::CUSTODY_CLOSE_REPLAY_RESULTING_REVISION)?,
                ),
                InstructionV3::load_const(
                    s(scalar::CUSTODY_OPERATION)?,
                    OperationV1::Transfer as u64,
                ),
                // The claims residual advances the Claims market once; the
                // escrow Position close that follows expects that successor,
                // and the Position's close-time revision is its post-affine
                // successor.
                InstructionV3::increment_into(
                    s(scalar::CLAIMS_MARKET_REVISION)?,
                    s(scalar::CLAIMS_POST_MARKET_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::POSITION_ZERO_REVISION)?,
                    s(scalar::SETTLEMENT_POSITION_REVISION)?,
                ),
                InstructionV3::increment_into(
                    s(scalar::SETTLEMENT_POSITION_REVISION)?,
                    s(scalar::SETTLEMENT_POST_POSITION_REVISION)?,
                ),
                // Constant residual row plumbing: the escrow Position is the
                // sole source (index zero), the maker's the sole destination
                // (index one), and a transfer mints nothing. The row count is
                // deliberately unpinned: an omitted row leaves the Position
                // nonzero and the Position close refuses -- fail-closed.
                InstructionV3::load_const(s(scalar::CLAIMS_SOURCE_PRESENT)?, 1),
                InstructionV3::load_const(s(scalar::CLAIMS_DESTINATION_PRESENT)?, 1),
                InstructionV3::load_const(s(scalar::CLAIMS_SOURCE_POSITION_INDEX)?, 0),
                InstructionV3::load_const(s(scalar::CLAIMS_DESTINATION_POSITION_INDEX)?, 1),
                InstructionV3::load_const(
                    s(scalar::CLAIMS_AGGREGATE_DIRECTION)?,
                    DeltaDirectionV2::Neutral as u64,
                ),
                InstructionV3::load_const(
                    s(scalar::CLAIMS_SOURCE_DIRECTION)?,
                    DeltaDirectionV2::Debit as u64,
                ),
                InstructionV3::load_const(
                    s(scalar::CLAIMS_DESTINATION_DIRECTION)?,
                    DeltaDirectionV2::Credit as u64,
                ),
                // The 0010 SS2a addressing discipline for every leg: the vault
                // drawn on is the ORDER's own, the refunded owner is the
                // record's maker, the closed Position is the order's, and
                // every rent credit is the maker's.
                InstructionV3::identity_eq(i(identity::SOURCE_VAULT_CONTEXT)?, i(identity::ORDER)?),
                InstructionV3::identity_eq(
                    i(identity::CUSTODY_DESTINATION_OWNER)?,
                    i(identity::OWNER)?,
                ),
                InstructionV3::identity_eq(i(identity::POSITION_ZERO_OWNER)?, i(identity::ORDER)?),
                InstructionV3::identity_eq(i(identity::POSITION_ONE_OWNER)?, i(identity::OWNER)?),
                InstructionV3::identity_eq(
                    i(identity::SETTLEMENT_POSITION_OWNER)?,
                    i(identity::ORDER)?,
                ),
                InstructionV3::identity_eq(i(identity::RENT_CREDIT)?, i(identity::OWNER)?),
                InstructionV3::identity_eq(i(identity::RENT_REFUND)?, i(identity::OWNER)?),
            ] {
                push(output, cursor, instruction)?;
            }
        }
    }
    Ok(())
}

/// Require the Custody vault a transfer touches to be the one the row names.
///
/// **This is the on-chain half of the escrow's addressing, and it did not
/// exist.** Decision 0010 §2 argues that "a maker can never be paid more than
/// they escrowed" is a property of the address, because the vault context is the
/// order's own content identity. That argument holds only if something requires
/// the vault in the frame to BE the one the row names, and nothing did: the
/// vault context reaches the register bank from the AccountProfile's projection
/// of whatever Custody accounts the caller supplied, while the order and
/// candidate identities reach it from the authenticated manifest row. A
/// `Collect` could name order A in its semantics and draw on order B's vault.
///
/// The comparison is only expressible where the direction is fixed at authoring
/// time. `Materialize` patches its compartments at runtime from the
/// authenticated complete-set move, so which side is the candidate and which the
/// Hoard is not a constant of its artifact; it is named in the omission index
/// rather than half-checked here.
fn append_vault_context_binds(
    action: Action,
    output: &mut [InstructionV3],
    cursor: &mut usize,
) -> Result<()> {
    let instructions = match action {
        // The escrow leg: the source vault is the ORDER's, the destination the
        // candidate's settlement inventory.
        Action::Collect => [
            InstructionV3::identity_eq(i(identity::SOURCE_VAULT_CONTEXT)?, i(identity::ORDER)?),
            InstructionV3::identity_eq(
                i(identity::DESTINATION_VAULT_CONTEXT)?,
                i(identity::CANDIDATE)?,
            ),
        ],
        // The payout leg: out of the candidate's inventory, to the row's own
        // maker and to no other external owner.
        Action::Distribute => [
            InstructionV3::identity_eq(i(identity::SOURCE_VAULT_CONTEXT)?, i(identity::CANDIDATE)?),
            InstructionV3::identity_eq(
                i(identity::CUSTODY_DESTINATION_OWNER)?,
                i(identity::OWNER)?,
            ),
        ],
        // The terminal surplus: out of the candidate's inventory, to the
        // immutable configured beneficiary.
        Action::Close => [
            InstructionV3::identity_eq(i(identity::SOURCE_VAULT_CONTEXT)?, i(identity::CANDIDATE)?),
            InstructionV3::identity_eq(
                i(identity::CUSTODY_DESTINATION_OWNER)?,
                i(identity::BENEFICIARY)?,
            ),
        ],
        _ => return Err(GeneralTransitionArtifactErrorV3::Geometry),
    };
    for instruction in instructions {
        push(output, cursor, instruction)?;
    }
    Ok(())
}

fn append_row_action(output: &mut [InstructionV3], cursor: &mut usize) -> Result<()> {
    for instruction in [
        InstructionV3::nonzero(s(scalar::SETTLEMENT_POSITION_PRESENT)?),
        InstructionV3::nonzero(s(scalar::ORDER_COORDINATE)?),
        InstructionV3::increment_into(
            s(scalar::SETTLEMENT_REVISION)?,
            s(scalar::CURSOR_RESULTING_REVISION)?,
        ),
        InstructionV3::increment_into(
            s(scalar::CLAIMS_MARKET_REVISION)?,
            s(scalar::CLAIMS_POST_MARKET_REVISION)?,
        ),
        InstructionV3::increment_into(
            s(scalar::SETTLEMENT_POSITION_REVISION)?,
            s(scalar::SETTLEMENT_POST_POSITION_REVISION)?,
        ),
        InstructionV3::increment_into(
            s(scalar::CUSTODY_EXPECTED_REVISION)?,
            s(scalar::CUSTODY_RESULTING_REVISION)?,
        ),
        InstructionV3::load_const(s(scalar::CUSTODY_OPERATION)?, 2),
        InstructionV3::scalar_eq(s(scalar::CLAIMS_ROW_COUNT)?, s(scalar::OUTCOME_COUNT)?),
    ] {
        push(output, cursor, instruction)?;
    }
    Ok(())
}

fn append_item(action: Action, output: &mut [InstructionV3], cursor: &mut usize) -> Result<()> {
    // AN ACTION WITH NO TAIL EMITS NO ITEM SECTION, not even the bound check --
    // there is no `OUTCOME` register left for it to read. `InstructionV3`
    // bounds every item operand by the declared stride, so a zero stride with a
    // non-empty item body is not a program. Lean states the same thing both
    // ways in `a_zero_stride_action_emits_no_item_body`.
    if general_hot_item_scalar_stride_v3(action) == 0 {
        return Ok(());
    }
    push(
        output,
        cursor,
        InstructionV3::scalar_lt(is(item_scalar::OUTCOME)?, s(scalar::OUTCOME_COUNT)?),
    )?;
    match action {
        Action::SubmitCandidate | Action::VerifyCandidateRow | Action::CloseCandidate => {}
        Action::Consider | Action::Freeze | Action::Materialize => {}
        Action::OpenBatch | Action::CloseBatch => {}
        Action::InitializeSettlement => push(
            output,
            cursor,
            InstructionV3::load_const(is(item_scalar::CURSOR_INVENTORY)?, 0),
        )?,
        // `ReleaseOrder` moves its residual claims through the same
        // two-Position transfer shape as the settlement rows: nothing minted,
        // source and destination magnitudes exactly the row quantity.
        // `placeOrder` derives its escrow row from the signed terms: the
        // claim reserve at each outcome is deliver-per-lot times the order's
        // maximum fill, moved whole from the maker to the escrow.
        Action::PlaceOrder => {
            for instruction in [
                InstructionV3::load_const(is(item_scalar::CLAIMS_AGGREGATE_MAGNITUDE)?, 0),
                InstructionV3::checked_mul_into(
                    is(item_scalar::QUANTITY)?,
                    s(scalar::ORDER_MAX_LOTS)?,
                    is(item_scalar::CLAIMS_SOURCE_MAGNITUDE)?,
                ),
                InstructionV3::copy_scalar(
                    is(item_scalar::CLAIMS_SOURCE_MAGNITUDE)?,
                    is(item_scalar::CLAIMS_DESTINATION_MAGNITUDE)?,
                ),
            ] {
                push(output, cursor, instruction)?;
            }
        }
        Action::Collect | Action::Distribute | Action::CancelOrder | Action::ReleaseOrder => {
            for instruction in [
                InstructionV3::load_const(is(item_scalar::CLAIMS_AGGREGATE_MAGNITUDE)?, 0),
                InstructionV3::scalar_eq(
                    is(item_scalar::CLAIMS_SOURCE_MAGNITUDE)?,
                    is(item_scalar::QUANTITY)?,
                ),
                InstructionV3::scalar_eq(
                    is(item_scalar::CLAIMS_DESTINATION_MAGNITUDE)?,
                    is(item_scalar::QUANTITY)?,
                ),
            ] {
                push(output, cursor, instruction)?;
            }
        }
        Action::Close => {
            for coordinate in [
                item_scalar::QUANTITY,
                item_scalar::CLAIMS_AGGREGATE_MAGNITUDE,
                item_scalar::CLAIMS_SOURCE_MAGNITUDE,
                item_scalar::CLAIMS_DESTINATION_MAGNITUDE,
                item_scalar::CURSOR_INVENTORY,
            ] {
                push(
                    output,
                    cursor,
                    InstructionV3::load_const(is(coordinate)?, 0),
                )?;
            }
        }
    }
    Ok(())
}

fn push(output: &mut [InstructionV3], cursor: &mut usize, value: InstructionV3) -> Result<()> {
    *output
        .get_mut(*cursor)
        .ok_or(GeneralTransitionArtifactErrorV3::Geometry)? = value;
    *cursor = cursor
        .checked_add(1)
        .ok_or(GeneralTransitionArtifactErrorV3::Geometry)?;
    Ok(())
}

fn narrow(value: u32) -> Result<u16> {
    u16::try_from(value).map_err(|_| GeneralTransitionArtifactErrorV3::Geometry)
}

fn s(value: u32) -> Result<ScalarRegisterV3> {
    Ok(ScalarRegisterV3::common(narrow(value)?))
}

fn is(value: u32) -> Result<ScalarRegisterV3> {
    Ok(ScalarRegisterV3::item(narrow(value)?))
}

fn i(value: u32) -> Result<IdentityRegisterV3> {
    Ok(IdentityRegisterV3::common(narrow(value)?))
}

#[cfg(test)]
mod tests {
    extern crate std;

    use dclutch_vm::v3::{RegisterInput, RegisterOutput, execute_fold_atomic};
    use std::{format, vec};

    use super::*;

    const ACTIVE_LIFECYCLE: u64 = GeneralLifecycleV2::Active.tag() as u64;

    const ACTIONS: [Action; 15] = [
        Action::Consider,
        Action::Freeze,
        Action::InitializeSettlement,
        Action::Collect,
        Action::Materialize,
        Action::Distribute,
        Action::Close,
        Action::OpenBatch,
        Action::PlaceOrder,
        Action::CancelOrder,
        Action::CloseBatch,
        Action::SubmitCandidate,
        Action::VerifyCandidateRow,
        Action::ReleaseOrder,
        Action::CloseCandidate,
    ];

    fn artifact(action: Action) -> std::vec::Vec<u8> {
        let (prelude, item, epilogue) = general_transition_instruction_count_v3(action);
        let mut instructions =
            vec![GENERAL_TRANSITION_INSTRUCTION_PLACEHOLDER_V3; prelude + item + epilogue];
        let bytes = general_transition_program_bytes_v3(action).expect("bytes");
        let mut scratch = vec![0_u8; bytes];
        let mut output = vec![0_u8; bytes];
        encode_general_transition_program_v3_atomic(
            action,
            &mut instructions,
            &mut scratch,
            &mut output,
        )
        .expect("program");
        output
    }

    /// **The byte gate.** Every program this module builds is exactly the one
    /// Lean authored.
    ///
    /// This is the whole point of the Lean module: the builder below is
    /// imperative Rust, and until now it was the sole authority for what a
    /// General release admits — an emitter and an authenticator that share an
    /// author are not two authorities (ADR-0010 §5), and here they were not even
    /// two files. Byte-identity is the strongest available statement that the
    /// transcription is faithful, and because it holds, nothing regenerated and
    /// no artifact digest moved when the Lean module landed.
    #[test]
    fn every_authored_program_is_byte_identical_to_the_lean_authored_one() {
        for action in ACTIONS {
            let built = artifact(action);
            let authored = general_transition_program_bytes_lean_v3(action);
            assert_eq!(
                built.as_slice(),
                authored,
                "{action:?} built a program the Lean module did not author",
            );
            assert!(!authored.is_empty());
        }
    }

    /// The emitted section counts and widths are the ones this module declares.
    ///
    /// Two authorities state the same geometry — the Rust `match` and the Lean
    /// program's own section lengths — so this is what stops a regenerated
    /// program disagreeing with the count that sizes its caller's workspace.
    #[test]
    fn the_emitted_geometry_agrees_with_the_declared_instruction_counts() {
        for (action, (prelude, item, epilogue), bytes) in [
            (
                Action::Consider,
                (
                    GENERAL_CONSIDER_PRELUDE_INSTRUCTIONS_V3,
                    GENERAL_CONSIDER_ITEM_INSTRUCTIONS_V3,
                    GENERAL_CONSIDER_EPILOGUE_INSTRUCTIONS_V3,
                ),
                GENERAL_CONSIDER_TRANSITION_BYTES_V3,
            ),
            (
                Action::Freeze,
                (
                    GENERAL_FREEZE_PRELUDE_INSTRUCTIONS_V3,
                    GENERAL_FREEZE_ITEM_INSTRUCTIONS_V3,
                    GENERAL_FREEZE_EPILOGUE_INSTRUCTIONS_V3,
                ),
                GENERAL_FREEZE_TRANSITION_BYTES_V3,
            ),
            (
                Action::InitializeSettlement,
                (
                    GENERAL_INITIALIZE_PRELUDE_INSTRUCTIONS_V3,
                    GENERAL_INITIALIZE_ITEM_INSTRUCTIONS_V3,
                    GENERAL_INITIALIZE_EPILOGUE_INSTRUCTIONS_V3,
                ),
                GENERAL_INITIALIZE_TRANSITION_BYTES_V3,
            ),
            (
                Action::Collect,
                (
                    GENERAL_COLLECT_PRELUDE_INSTRUCTIONS_V3,
                    GENERAL_COLLECT_ITEM_INSTRUCTIONS_V3,
                    GENERAL_COLLECT_EPILOGUE_INSTRUCTIONS_V3,
                ),
                GENERAL_COLLECT_TRANSITION_BYTES_V3,
            ),
            (
                Action::Materialize,
                (
                    GENERAL_MATERIALIZE_PRELUDE_INSTRUCTIONS_V3,
                    GENERAL_MATERIALIZE_ITEM_INSTRUCTIONS_V3,
                    GENERAL_MATERIALIZE_EPILOGUE_INSTRUCTIONS_V3,
                ),
                GENERAL_MATERIALIZE_TRANSITION_BYTES_V3,
            ),
            (
                Action::Distribute,
                (
                    GENERAL_DISTRIBUTE_PRELUDE_INSTRUCTIONS_V3,
                    GENERAL_DISTRIBUTE_ITEM_INSTRUCTIONS_V3,
                    GENERAL_DISTRIBUTE_EPILOGUE_INSTRUCTIONS_V3,
                ),
                GENERAL_DISTRIBUTE_TRANSITION_BYTES_V3,
            ),
            (
                Action::Close,
                (
                    GENERAL_CLOSE_PRELUDE_INSTRUCTIONS_V3,
                    GENERAL_CLOSE_ITEM_INSTRUCTIONS_V3,
                    GENERAL_CLOSE_EPILOGUE_INSTRUCTIONS_V3,
                ),
                GENERAL_CLOSE_TRANSITION_BYTES_V3,
            ),
            (
                Action::OpenBatch,
                (
                    GENERAL_OPEN_BATCH_PRELUDE_INSTRUCTIONS_V3,
                    GENERAL_OPEN_BATCH_ITEM_INSTRUCTIONS_V3,
                    GENERAL_OPEN_BATCH_EPILOGUE_INSTRUCTIONS_V3,
                ),
                GENERAL_OPEN_BATCH_TRANSITION_BYTES_V3,
            ),
            (
                Action::CloseBatch,
                (
                    GENERAL_CLOSE_BATCH_PRELUDE_INSTRUCTIONS_V3,
                    GENERAL_CLOSE_BATCH_ITEM_INSTRUCTIONS_V3,
                    GENERAL_CLOSE_BATCH_EPILOGUE_INSTRUCTIONS_V3,
                ),
                GENERAL_CLOSE_BATCH_TRANSITION_BYTES_V3,
            ),
            (
                Action::PlaceOrder,
                (
                    GENERAL_PLACE_ORDER_PRELUDE_INSTRUCTIONS_V3,
                    GENERAL_PLACE_ORDER_ITEM_INSTRUCTIONS_V3,
                    GENERAL_PLACE_ORDER_EPILOGUE_INSTRUCTIONS_V3,
                ),
                GENERAL_PLACE_ORDER_TRANSITION_BYTES_V3,
            ),
            (
                Action::CancelOrder,
                (
                    GENERAL_CANCEL_ORDER_PRELUDE_INSTRUCTIONS_V3,
                    GENERAL_CANCEL_ORDER_ITEM_INSTRUCTIONS_V3,
                    GENERAL_CANCEL_ORDER_EPILOGUE_INSTRUCTIONS_V3,
                ),
                GENERAL_CANCEL_ORDER_TRANSITION_BYTES_V3,
            ),
            (
                Action::SubmitCandidate,
                (
                    GENERAL_SUBMIT_CANDIDATE_PRELUDE_INSTRUCTIONS_V3,
                    GENERAL_SUBMIT_CANDIDATE_ITEM_INSTRUCTIONS_V3,
                    GENERAL_SUBMIT_CANDIDATE_EPILOGUE_INSTRUCTIONS_V3,
                ),
                GENERAL_SUBMIT_CANDIDATE_TRANSITION_BYTES_V3,
            ),
            (
                Action::VerifyCandidateRow,
                (
                    GENERAL_VERIFY_CANDIDATE_ROW_PRELUDE_INSTRUCTIONS_V3,
                    GENERAL_VERIFY_CANDIDATE_ROW_ITEM_INSTRUCTIONS_V3,
                    GENERAL_VERIFY_CANDIDATE_ROW_EPILOGUE_INSTRUCTIONS_V3,
                ),
                GENERAL_VERIFY_CANDIDATE_ROW_TRANSITION_BYTES_V3,
            ),
            (
                Action::ReleaseOrder,
                (
                    GENERAL_RELEASE_ORDER_PRELUDE_INSTRUCTIONS_V3,
                    GENERAL_RELEASE_ORDER_ITEM_INSTRUCTIONS_V3,
                    GENERAL_RELEASE_ORDER_EPILOGUE_INSTRUCTIONS_V3,
                ),
                GENERAL_RELEASE_ORDER_TRANSITION_BYTES_V3,
            ),
        ] {
            assert_eq!(
                general_transition_instruction_count_v3(action),
                (prelude, item, epilogue),
                "{action:?} section counts",
            );
            assert_eq!(
                general_transition_program_bytes_v3(action).expect("bytes"),
                bytes,
                "{action:?} encoded width",
            );
            assert_eq!(
                bytes,
                HEADER_BYTES + (prelude + item + epilogue) * INSTRUCTION_BYTES,
                "{action:?} width is not its own header plus its own instructions",
            );
        }
    }

    /// The Lean register schema is the one the Rust bank declares.
    ///
    /// `GeneralTransitionV3.lean` types the register space as three constructor
    /// lists whose order IS the wire index, and `hot_candidate_v3.rs` remains
    /// the name authority. This is the join: if either side renumbers, the
    /// emitted programs address a bank the other side does not have, and the
    /// byte gate above would refuse for a reason that reads as a transcription
    /// error rather than as the schema move it actually is.
    #[test]
    fn the_lean_register_schema_is_the_one_the_rust_bank_declares() {
        assert_eq!(
            GENERAL_TRANSITION_COMMON_SCALARS_V3,
            GENERAL_HOT_COMMON_SCALARS_V3
        );
        assert_eq!(
            GENERAL_TRANSITION_ITEM_SCALAR_STRIDE_V3,
            crate::general::hot_candidate_v3::GENERAL_HOT_ITEM_SCALAR_STRIDE_V3
        );
        assert_eq!(
            GENERAL_TRANSITION_COMMON_IDENTITIES_V3,
            GENERAL_HOT_COMMON_IDENTITIES_V3
        );
        assert_eq!(
            GENERAL_TRANSITION_ITEM_IDENTITY_STRIDE_V3,
            GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3
        );
        assert_eq!(GENERAL_AUTHORED_TRANSITION_ACTION_COUNT_V3, ACTIONS.len());
        // The two highest coordinates each bank names must be inside the width
        // the emitted programs were encoded against, or an instruction that
        // addresses one is decodable and out of range at fold time.
        const _: () = assert!(scalar::RESULT_RENT_PRINCIPAL < GENERAL_TRANSITION_COMMON_SCALARS_V3);
        const _: () =
            assert!(item_scalar::CURSOR_INVENTORY < GENERAL_TRANSITION_ITEM_SCALAR_STRIDE_V3);
        const _: () = assert!(identity::RESULT_OWNER < GENERAL_TRANSITION_COMMON_IDENTITIES_V3);
    }

    #[test]
    fn all_authored_actions_emit_distinct_nontrivial_runtime_tail_programs() {
        let mut prior: Option<std::vec::Vec<u8>> = None;
        for action in ACTIONS {
            let bytes = artifact(action);
            assert!(bytes.len() >= HEADER_BYTES + 13 * INSTRUCTION_BYTES);
            let program = ProgramV3::decode(&bytes).expect("decode");
            assert_eq!(
                program.common_scalar_count(),
                u16::try_from(GENERAL_HOT_COMMON_SCALARS_V3).expect("scalar count")
            );
            // THE ACTION'S stride, not the enum's. Restating the flat constant
            // here would assert that no action may declare a narrower tail,
            // which is exactly what OpenBatch and CloseBatch now do.
            assert_eq!(
                program.item_scalar_stride(),
                u16::try_from(general_hot_item_scalar_stride_v3(action)).expect("item stride")
            );
            if let Some(previous) = prior.replace(bytes) {
                assert_ne!(prior.as_ref().expect("current"), &previous);
            }
        }
    }

    /// Exact accepted Consider input bank at one runtime width.
    ///
    /// `lifecycle` is the observed capability-root lifecycle byte the
    /// AccountProfile would have projected into
    /// `scalar::ROOT_LIFECYCLE_OBSERVATION`.
    fn consider_input_bank(
        count: u32,
        lifecycle: u64,
    ) -> (std::vec::Vec<u64>, std::vec::Vec<[u8; 32]>) {
        let scalar_count = usize::try_from(
            GENERAL_HOT_COMMON_SCALARS_V3
                + count * crate::general::hot_candidate_v3::GENERAL_HOT_ITEM_SCALAR_STRIDE_V3,
        )
        .expect("scalar width");
        let mut input_scalars = vec![0_u64; scalar_count];
        input_scalars[usize::try_from(scalar::OUTCOME_COUNT).expect("outcome register")] =
            u64::from(count);
        input_scalars[usize::try_from(scalar::ZERO).expect("persisted outcome width")] =
            u64::from(count);
        input_scalars[usize::try_from(scalar::STATE_BUMP).expect("bump")] = 7;
        input_scalars[usize::try_from(scalar::PRIMARY_CANONICAL_BUMP).expect("canonical bump")] = 7;
        input_scalars[usize::try_from(scalar::PRIMARY_RENT_PRINCIPAL).expect("rent principal")] = 1;
        input_scalars[usize::try_from(scalar::SELECTION_REVISION).expect("revision")] = 1;
        input_scalars[usize::try_from(scalar::ROOT_LIFECYCLE_OBSERVATION).expect("lifecycle")] =
            lifecycle;
        for item in 0..count {
            let base = GENERAL_HOT_COMMON_SCALARS_V3
                + item * crate::general::hot_candidate_v3::GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            input_scalars[usize::try_from(base + item_scalar::OUTCOME).expect("item")] =
                u64::from(item);
        }
        let mut input_identities = vec![
            [0_u8; 32];
            usize::try_from(GENERAL_HOT_COMMON_IDENTITIES_V3)
                .expect("identity width")
        ];
        input_identities[usize::try_from(identity::PRIMARY_OWNER).expect("owner")] = [9; 32];
        input_identities[usize::try_from(identity::TRADING_PROGRAM).expect("program")] = [9; 32];
        (input_scalars, input_identities)
    }

    fn fold(
        program: ProgramV3<'_>,
        count: u32,
        input_scalars: &[u64],
        input_identities: &[[u8; 32]],
    ) -> Result<std::vec::Vec<u64>> {
        let mut scalar_scratch = vec![0_u64; input_scalars.len()];
        let mut scalar_output = vec![0_u64; input_scalars.len()];
        let mut identity_scratch = vec![[0_u8; 32]; input_identities.len()];
        let mut identity_output = vec![[0_u8; 32]; input_identities.len()];
        execute_fold_atomic(
            program,
            count,
            RegisterInput {
                scalars: input_scalars,
                identities: input_identities,
            },
            RegisterOutput {
                scalars: &mut scalar_scratch,
                identities: &mut identity_scratch,
            },
            RegisterOutput {
                scalars: &mut scalar_output,
                identities: &mut identity_output,
            },
        )
        .map_err(GeneralTransitionArtifactErrorV3::Transition)?;
        Ok(scalar_output)
    }

    /// Exact AccountProfile projection image accepted by SubmitCandidate.
    ///
    /// The two runtime widths exercise both the singleton boundary and the
    /// first three-byte outcome index. Every value below has one semantic
    /// owner in the candidate/batch/submission records or lifecycle result;
    /// the transition merely joins them and derives the two escrow tranches.
    fn submit_candidate_input_bank(count: u32) -> (std::vec::Vec<u64>, std::vec::Vec<[u8; 32]>) {
        let scalar_count = usize::try_from(
            GENERAL_HOT_COMMON_SCALARS_V3
                + count * crate::general::hot_candidate_v3::GENERAL_HOT_ITEM_SCALAR_STRIDE_V3,
        )
        .expect("scalar width");
        let mut scalars = vec![0_u64; scalar_count];
        let mut put = |coordinate: u32, value: u64| {
            scalars[usize::try_from(coordinate).expect("common scalar")] = value;
        };
        let row_count = u64::from(count);
        let page_count = row_count;
        let reward_rate = 7_u64;
        let verification = (row_count + 1) * reward_rate;
        put(scalar::OUTCOME_COUNT, u64::from(count));
        put(scalar::ZERO, u64::from(count));
        put(scalar::STATE_BUMP, 42);
        put(scalar::PRIMARY_CANONICAL_BUMP, 42);
        put(scalar::PRIMARY_RENT_PRINCIPAL, 1_000);
        put(scalar::ROOT_LIFECYCLE_OBSERVATION, ACTIVE_LIFECYCLE);
        put(scalar::PRIMARY_CREATED, 1);
        put(scalar::SELECTION_PRICE_SCALE, 1_000);
        put(scalar::ORDER_MAX_LOTS, 1_000);
        put(scalar::BATCH_POST_ORDER_COUNT, u64::from(count));
        put(scalar::VERIFY_POST_ORDER_COUNT, u64::from(count));
        put(scalar::VERIFY_POST_PAGE, page_count);
        put(scalar::CANDIDATE_PAGE_COUNT, page_count);
        put(scalar::CANDIDATE_STATUS_OBSERVATION, 1);
        put(scalar::SELECTION_BEST_CANDIDATE_COORDINATE, 1);
        put(scalar::CANDIDATE_PAGE_REVISION, 9);
        put(scalar::CANDIDATE_ROW_COUNT, row_count);
        put(scalar::CANDIDATE_REWARD_RATE, reward_rate);
        put(
            scalar::BATCH_STATUS_OBSERVATION,
            u64::from(BatchStatusV1::Closed.tag()),
        );
        put(scalar::BATCH_COLLECTION_CLOSE_SLOT, 99);
        put(scalar::CURRENT_SLOT, 100);
        put(scalar::BATCH_SETTLEMENT_CLOSE_SLOT, 101);
        put(scalar::CANDIDATE_SUBMITTED_SLOT, 100);
        put(
            scalar::CANDIDATE_VERIFICATION_REMAINING_OBSERVATION,
            verification,
        );
        put(scalar::CANDIDATE_CLEANUP_REMAINING_OBSERVATION, reward_rate);
        for item in 0..count {
            let base = GENERAL_HOT_COMMON_SCALARS_V3
                + item * crate::general::hot_candidate_v3::GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            scalars[usize::try_from(base + item_scalar::OUTCOME).expect("item outcome")] =
                u64::from(item);
        }

        let mut identities = vec![
            [0_u8; 32];
            usize::try_from(GENERAL_HOT_COMMON_IDENTITIES_V3)
                .expect("identity width")
        ];
        let mut put_id = |coordinate: u32, value: [u8; 32]| {
            identities[usize::try_from(coordinate).expect("common identity")] = value;
        };
        let trading = [0x11; 32];
        let solver = [0x22; 32];
        let candidate = [0x33; 32];
        let batch = [0x44; 32];
        let product = [0x55; 32];
        put_id(identity::PRIMARY_OWNER, trading);
        put_id(identity::TRADING_PROGRAM, trading);
        put_id(identity::PRIMARY_BENEFICIARY, solver);
        put_id(identity::OWNER, solver);
        // The solver who pays is the solver the candidate names: the
        // AccountProfile projects the creation payer's key here, and the
        // transition is where the two are joined.
        put_id(identity::PAYER, solver);
        put_id(identity::CANDIDATE, candidate);
        put_id(identity::BEST_VERIFIED_DIGEST, candidate);
        put_id(identity::RESULT_BENEFICIARY_OBSERVATION, candidate);
        put_id(identity::SELECTION_POLICY, batch);
        put_id(identity::SELECTION_BATCH, batch);
        put_id(identity::BENEFICIARY, batch);
        put_id(identity::ORDER, product);
        put_id(identity::SELECTION_PRODUCT, product);
        (scalars, identities)
    }

    #[test]
    fn submit_candidate_executes_at_one_and_258_outcomes() {
        let bytes = artifact(Action::SubmitCandidate);
        let program = ProgramV3::decode(&bytes).expect("SubmitCandidate program");
        for count in [1_u32, 258] {
            let (scalars, identities) = submit_candidate_input_bank(count);
            let output =
                fold(program, count, &scalars, &identities).expect("valid submitted candidate");
            assert_eq!(
                output[usize::try_from(scalar::ACTION).expect("action")],
                u64::from(Action::SubmitCandidate as u8),
            );
            assert_eq!(
                output[usize::try_from(scalar::CANDIDATE_POST_VERIFICATION_REMAINING)
                    .expect("verification escrow")],
                (u64::from(count) + 1) * 7,
            );
            assert_eq!(
                output[usize::try_from(scalar::CANDIDATE_POST_CLEANUP_REMAINING)
                    .expect("cleanup escrow")],
                7,
            );
        }
    }

    #[test]
    fn submit_candidate_refuses_each_authority_or_economic_join_when_substituted() {
        let bytes = artifact(Action::SubmitCandidate);
        let program = ProgramV3::decode(&bytes).expect("SubmitCandidate program");
        for count in [1_u32, 258] {
            let (accepted_scalars, accepted_identities) = submit_candidate_input_bank(count);
            for (coordinate, hostile) in [
                (scalar::BATCH_STATUS_OBSERVATION, 0),
                (scalar::BATCH_COLLECTION_CLOSE_SLOT, 101),
                (scalar::BATCH_SETTLEMENT_CLOSE_SLOT, 100),
                (scalar::CANDIDATE_SUBMITTED_SLOT, 99),
                (scalar::CANDIDATE_PAGE_COUNT, u64::from(count) + 1),
                (scalar::CANDIDATE_PAGE_REVISION, 0),
                (scalar::CANDIDATE_REWARD_RATE, 0),
                (
                    scalar::CANDIDATE_VERIFICATION_REMAINING_OBSERVATION,
                    (u64::from(count) + 1) * 7 - 1,
                ),
                (scalar::CANDIDATE_CLEANUP_REMAINING_OBSERVATION, 6),
            ] {
                let mut scalars = accepted_scalars.clone();
                scalars[usize::try_from(coordinate).expect("hostile scalar")] = hostile;
                assert!(
                    fold(program, count, &scalars, &accepted_identities).is_err(),
                    "width {count} accepted hostile scalar {coordinate}",
                );
            }
            for coordinate in [
                identity::PRIMARY_BENEFICIARY,
                identity::BEST_VERIFIED_DIGEST,
                identity::SELECTION_POLICY,
                identity::ORDER,
                identity::RESULT_BENEFICIARY_OBSERVATION,
                identity::BENEFICIARY,
            ] {
                let mut identities = accepted_identities.clone();
                identities[usize::try_from(coordinate).expect("hostile identity")] = [0xee; 32];
                assert!(
                    fold(program, count, &accepted_scalars, &identities).is_err(),
                    "width {count} accepted hostile identity {coordinate}",
                );
            }
        }
    }

    /// Exact authenticated bank accepted by VerifyCandidateRow.
    ///
    /// The request-profile inputs are `ROOT_EXPECTED_REVISION` (scalar 94),
    /// `COMPLETE_SET_MOVE` (1), `CLAIMS_AFFINE_ACTIVE` (2), and
    /// `PARENT_REQUEST_DIGEST` (identity 0). The observation/post coordinates
    /// are independently projected from authenticated state and the sole
    /// candidate verifier.
    fn verify_candidate_row_input_bank(
        count: u32,
        terminal: u64,
        post_order_count: u64,
    ) -> (std::vec::Vec<u64>, std::vec::Vec<[u8; 32]>) {
        let (mut scalars, mut identities) = consider_input_bank(count, ACTIVE_LIFECYCLE);
        let mut put = |coordinate: u32, value: u64| {
            scalars[usize::try_from(coordinate).expect("Verify common scalar")] = value;
        };
        put(
            scalar::CANDIDATE_STATUS_OBSERVATION,
            u64::from(GeneralCandidateStatusV1::Submitted.tag()),
        );
        put(scalar::ROOT_EXPECTED_REVISION, 5);
        put(scalar::VERIFY_REVISION_OBSERVATION, 5);
        put(scalar::VERIFY_POST_REVISION, 6);
        put(scalar::COMPLETE_SET_MOVE, 2);
        put(scalar::VERIFY_PAGE_OBSERVATION, 2);
        put(scalar::CLAIMS_AFFINE_ACTIVE, 3);
        put(scalar::VERIFY_ROW_OBSERVATION, 3);
        put(scalar::VERIFY_ORDER_COUNT_OBSERVATION, 4);
        put(scalar::VERIFY_POST_ORDER_COUNT, post_order_count);
        put(scalar::VERIFY_TERMINAL, terminal);
        let candidate = [0x33; 32];
        identities[usize::try_from(identity::PARENT_REQUEST_DIGEST).expect("request subject")] =
            candidate;
        identities[usize::try_from(identity::CANDIDATE).expect("candidate identity")] = candidate;
        (scalars, identities)
    }

    fn assert_verify_check_failed(
        program: ProgramV3<'_>,
        count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
        context: &str,
    ) {
        assert_eq!(
            fold(program, count, scalars, identities),
            Err(GeneralTransitionArtifactErrorV3::Transition(
                dclutch_vm::v3::Error::CheckFailed,
            )),
            "{context}",
        );
    }

    #[test]
    fn verify_candidate_row_executes_at_one_and_258_outcomes() {
        let bytes = artifact(Action::VerifyCandidateRow);
        assert_eq!(bytes.len(), 608);
        let program = ProgramV3::decode(&bytes).expect("VerifyCandidateRow program");
        for count in [1_u32, 258] {
            for (terminal, post_order_count) in [(0_u64, 4_u64), (1, 5)] {
                let (scalars, identities) =
                    verify_candidate_row_input_bank(count, terminal, post_order_count);
                let output = fold(program, count, &scalars, &identities)
                    .expect("exact authenticated verifier row");
                assert_eq!(
                    output[usize::try_from(scalar::ACTION).expect("action")],
                    u64::from(Action::VerifyCandidateRow as u8),
                );
                assert_eq!(
                    output[usize::try_from(scalar::VERIFY_POST_REVISION).expect("post revision")],
                    6,
                );
                assert_eq!(
                    output[usize::try_from(scalar::VERIFY_POST_ORDER_COUNT)
                        .expect("post order count")],
                    post_order_count,
                );
                assert_eq!(
                    output[usize::try_from(scalar::VERIFY_TERMINAL).expect("terminal")],
                    terminal,
                );
            }
        }
    }

    #[test]
    fn verify_candidate_row_refuses_substituted_revision_page_row_or_terminal() {
        let bytes = artifact(Action::VerifyCandidateRow);
        let program = ProgramV3::decode(&bytes).expect("VerifyCandidateRow program");
        for count in [1_u32, 258] {
            let (accepted_scalars, accepted_identities) =
                verify_candidate_row_input_bank(count, 1, 5);
            for (coordinate, hostile, label) in [
                (scalar::ROOT_EXPECTED_REVISION, 4, "request revision"),
                (scalar::COMPLETE_SET_MOVE, 1, "request page"),
                (scalar::CLAIMS_AFFINE_ACTIVE, 2, "request row"),
                (scalar::VERIFY_TERMINAL, 2, "terminal"),
                (scalar::CANDIDATE_STATUS_OBSERVATION, 2, "candidate status"),
                (scalar::VERIFY_POST_ORDER_COUNT, 3, "decreasing order count"),
                (scalar::VERIFY_POST_ORDER_COUNT, 6, "skipped order count"),
            ] {
                let mut scalars = accepted_scalars.clone();
                scalars[usize::try_from(coordinate).expect("hostile scalar")] = hostile;
                assert_verify_check_failed(
                    program,
                    count,
                    &scalars,
                    &accepted_identities,
                    &format!("width {count} accepted substituted {label}"),
                );
            }
            let mut identities = accepted_identities.clone();
            identities[usize::try_from(identity::PARENT_REQUEST_DIGEST).expect("subject")] =
                [0xee; 32];
            assert_verify_check_failed(
                program,
                count,
                &accepted_scalars,
                &identities,
                &format!("width {count} accepted a substituted Candidate subject"),
            );
        }
    }

    fn release_order_input_bank(
        count: u32,
        current_slot: u64,
    ) -> (std::vec::Vec<u64>, std::vec::Vec<[u8; 32]>) {
        let (mut scalars, mut identities) = consider_input_bank(count, ACTIVE_LIFECYCLE);
        scalars[usize::try_from(scalar::ORDER_PHASE_OBSERVATION).expect("phase")] =
            u64::from(GeneralOrderPhaseV1::Placed.tag());
        scalars[usize::try_from(scalar::ORDER_VALID_UNTIL_SLOT).expect("valid until")] = 500;
        scalars[usize::try_from(scalar::CURRENT_SLOT).expect("current slot")] = current_slot;
        scalars[usize::try_from(scalar::ORDER_ADMITTED_SLOT_OBSERVATION).expect("admitted")] = 10;
        scalars[usize::try_from(scalar::ORDER_MAX_LOTS).expect("lots")] = 6;
        scalars[usize::try_from(scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT).expect("quote cap")] = 7;
        scalars[usize::try_from(scalar::ESCROW_BALANCE_OBSERVATION).expect("residual")] = 40;
        scalars[usize::try_from(scalar::CUSTODY_EXPECTED_REVISION).expect("custody revision")] = 5;
        scalars[usize::try_from(scalar::CLAIMS_MARKET_REVISION).expect("market revision")] = 11;
        scalars[usize::try_from(scalar::POSITION_ZERO_REVISION).expect("position revision")] = 3;
        let order = [0x33; 32];
        let maker = [0x44; 32];
        for coordinate in [
            identity::ORDER,
            identity::SOURCE_VAULT_CONTEXT,
            identity::POSITION_ZERO_OWNER,
            identity::SETTLEMENT_POSITION_OWNER,
        ] {
            identities[usize::try_from(coordinate).expect("order identity")] = order;
        }
        for coordinate in [
            identity::OWNER,
            identity::CUSTODY_DESTINATION_OWNER,
            identity::POSITION_ONE_OWNER,
            identity::RENT_CREDIT,
            identity::RENT_REFUND,
        ] {
            identities[usize::try_from(coordinate).expect("maker identity")] = maker;
        }
        (scalars, identities)
    }

    #[test]
    fn release_order_accepts_the_pinned_close_slot_and_refuses_one_slot_early() {
        let bytes = artifact(Action::ReleaseOrder);
        let program = ProgramV3::decode(&bytes).expect("ReleaseOrder program");
        for count in [1_u32, 258] {
            let (at_close, identities) = release_order_input_bank(count, 500);
            fold(program, count, &at_close, &identities)
                .expect("the pinned settlement close is the release boundary");
            let (one_slot_early, identities) = release_order_input_bank(count, 499);
            assert!(
                fold(program, count, &one_slot_early, &identities).is_err(),
                "width {count} released one slot before the pinned close",
            );
        }
    }

    #[test]
    fn consider_fold_accepts_product_widths_one_and_258() {
        let bytes = artifact(Action::Consider);
        let program = ProgramV3::decode(&bytes).expect("decode");
        for count in [1_u32, 258] {
            let (input_scalars, input_identities) = consider_input_bank(count, ACTIVE_LIFECYCLE);
            let scalar_output = fold(program, count, &input_scalars, &input_identities)
                .expect("runtime-width fold");
            assert_eq!(
                scalar_output[usize::try_from(scalar::ACTION).expect("action")],
                u64::from(Action::Consider as u8)
            );
        }
    }

    /// U-003(a), executed on the emitted artifact rather than argued.
    ///
    /// The composite root's immutable header is byte-identical for a live and a
    /// retired capability, so this conjunct is the only thing on the
    /// runtime-width path that can tell them apart. Every General action carries
    /// it, because it lives in the shared prelude.
    #[test]
    fn every_action_refuses_a_root_that_is_not_active() {
        for action in ACTIONS {
            let bytes = artifact(action);
            let program = ProgramV3::decode(&bytes).expect("decode");
            for count in [1_u32, 258] {
                // Retiring, Retired, an unwritten register, and an unknown byte.
                for lifecycle in [0_u64, 2, 3, 4, 255] {
                    let (input_scalars, input_identities) = consider_input_bank(count, lifecycle);
                    assert!(
                        fold(program, count, &input_scalars, &input_identities).is_err(),
                        "action {action:?} at width {count} accepted lifecycle {lifecycle}",
                    );
                }
            }
        }
    }

    /// The refusal is the lifecycle conjunct, not some other unmet prelude
    /// requirement: the same bank at `Active` reaches the action-specific
    /// conjuncts, which is where the other actions legitimately refuse.
    /// A FREEZE ONE SLOT BEFORE THE SELECTION WINDOW ENDS REFUSES.
    ///
    /// This test used to be
    /// `freeze_admits_at_every_slot_because_its_transition_names_no_clock`,
    /// and it was a pin on a live defect: `Freeze` closes selection around the
    /// current best valid submitted candidate, and its transition named the
    /// clock nowhere, so a freeze one slot after the batch opened admitted
    /// exactly as a freeze after the window closed. Whoever moved first ended
    /// selection and every candidate not yet submitted was lost, and
    /// `closed_selection_is_immutable` then protected that outcome.
    /// `f66dbb078` measured it: the same bank folded at slot 0 and at
    /// `u64::MAX / 2` both admitted, with outputs differing in
    /// `scalar::CURRENT_SLOT` alone. The pin promised to go red naming itself
    /// the day a slot conjunct landed. It did, and this is what replaced it.
    ///
    /// The deadline is the batch's own: `collection_close + selectionSlots`,
    /// which is exactly the sum `OpenBatch` forms when it derives the
    /// settlement close, so the two programs cannot drift about when selection
    /// ends. The boundary is `scalarLe`, so the first ADMITTING slot is the
    /// deadline itself; one slot below it refuses.
    #[test]
    fn freeze_refuses_one_slot_before_the_selection_window_ends_and_admits_at_it() {
        let bytes = artifact(Action::Freeze);
        let program = ProgramV3::decode(&bytes).expect("decode");
        let collection_close = 4_096_u64;
        let selection_slots = 64_u64;
        let deadline = collection_close + selection_slots;
        let bank = |slot: u64, close: u64, slots: u64| {
            let (mut scalars, identities) = consider_input_bank(1, ACTIVE_LIFECYCLE);
            for (coordinate, value) in [
                (scalar::CURRENT_SLOT, slot),
                (scalar::SELECTION_BEST_CANDIDATE_COORDINATE, 1),
                (scalar::SELECTION_BEST_VERIFIED_REVISION, 1),
                (scalar::BATCH_COLLECTION_CLOSE_SLOT, close),
                (scalar::CONFIG_SELECTION_SLOTS, slots),
            ] {
                scalars[usize::try_from(coordinate).expect("register")] = value;
            }
            (scalars, identities)
        };
        let fold_at = |slot: u64, close: u64, slots: u64| {
            let (scalars, identities) = bank(slot, close, slots);
            fold(program, 1, &scalars, &identities)
        };
        assert!(
            fold_at(deadline - 1, collection_close, selection_slots).is_err(),
            "a freeze one slot before the selection window ends must refuse",
        );
        assert!(
            fold_at(deadline, collection_close, selection_slots).is_ok(),
            "a freeze at the deadline must admit, or the boundary is off by one",
        );
        assert!(
            fold_at(u64::MAX / 2, collection_close, selection_slots).is_ok(),
            "a freeze well past the window must admit",
        );
        // THE SLOT IS NOW LOAD-BEARING, which is the whole claim. The old pin
        // proved the opposite by folding two slots and comparing outputs; the
        // successor statement is that two slots on either side of one deadline
        // disagree about whether the fold succeeds at all.
        //
        // AND THE TWO SOURCES ARE FAIL-CLOSED. A profile that lost either
        // projection leaves a well-formed zero, and a zero deadline is a freeze
        // admitted at every slot -- the exact state this conjunct ended. Both
        // `nonzero`s are proved here rather than trusted from the source.
        assert!(
            fold_at(deadline, 0, selection_slots).is_err(),
            "a zero collection close is an unsourced projection, not a deadline",
        );
        assert!(
            fold_at(deadline, collection_close, 0).is_err(),
            "a zero selection window is an unsourced projection, not a deadline",
        );
        // The positive control the old pin carried, kept: the same early bank
        // through `CloseBatch`, which has always named the clock, must refuse
        // at slot zero. Without it, "the fixture refused" and "the fixture
        // never reached a window conjunct" read identically.
        let close_bytes = artifact(Action::CloseBatch);
        let close = ProgramV3::decode(&close_bytes).expect("decode");
        let (early_scalars, early_identities) = bank(0, collection_close, selection_slots);
        assert!(
            fold(close, 1, &early_scalars, &early_identities).is_err(),
            "CloseBatch must refuse at slot zero, or this fixture reaches no window conjunct"
        );
    }

    #[test]
    fn the_active_bank_passes_the_lifecycle_conjunct_for_every_action() {
        for action in ACTIONS {
            let bytes = artifact(action);
            let program = ProgramV3::decode(&bytes).expect("decode");
            let (mut input_scalars, input_identities) = consider_input_bank(1, ACTIVE_LIFECYCLE);
            let observation =
                usize::try_from(scalar::ROOT_LIFECYCLE_OBSERVATION).expect("lifecycle");
            let active = fold(program, 1, &input_scalars, &input_identities);
            input_scalars[observation] = u64::from(GeneralLifecycleV2::Retired.tag());
            let retired = fold(program, 1, &input_scalars, &input_identities);
            assert!(
                retired.is_err(),
                "action {action:?} accepted a retired root"
            );
            if action == Action::Consider {
                assert!(active.is_ok(), "the Active Consider bank must accept");
            }
        }
    }

    /// One accepted `Collect` bank: every conjunct satisfied, at one width.
    ///
    /// Built by extending the Consider bank rather than by restating it, so a
    /// change to the shared prelude reaches both.
    fn collect_input_bank(count: u32) -> (std::vec::Vec<u64>, std::vec::Vec<[u8; 32]>) {
        let (mut scalars, mut identities) = consider_input_bank(count, ACTIVE_LIFECYCLE);
        for (coordinate, value) in [
            (scalar::SETTLEMENT_POSITION_PRESENT, 1),
            (scalar::ORDER_COORDINATE, 1),
            (scalar::CLAIMS_ROW_COUNT, u64::from(count)),
        ] {
            scalars[usize::try_from(coordinate).expect("scalar coordinate")] = value;
        }
        for item in 0..count {
            let base = GENERAL_HOT_COMMON_SCALARS_V3
                + item * crate::general::hot_candidate_v3::GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            for coordinate in [
                item_scalar::QUANTITY,
                item_scalar::CLAIMS_SOURCE_MAGNITUDE,
                item_scalar::CLAIMS_DESTINATION_MAGNITUDE,
            ] {
                scalars[usize::try_from(base + coordinate).expect("item coordinate")] = 5;
            }
        }
        for (coordinate, value) in [
            (identity::ORDER, [3_u8; 32]),
            (identity::SOURCE_VAULT_CONTEXT, [3_u8; 32]),
            (identity::CANDIDATE, [4_u8; 32]),
            (identity::DESTINATION_VAULT_CONTEXT, [4_u8; 32]),
        ] {
            identities[usize::try_from(coordinate).expect("identity coordinate")] = value;
        }
        (scalars, identities)
    }

    /// **The escrow's addressing, checked on chain rather than argued.**
    ///
    /// Decision 0010 §2 rests "a maker can never be paid more than they
    /// escrowed" on the vault being keyed by the order's own identity. Nothing
    /// required the vault in the frame to be that one: the context arrives from
    /// the AccountProfile's projection of caller-supplied Custody accounts, and
    /// the order identity from the authenticated manifest row. A `Collect` could
    /// name one order and draw on another's vault.
    #[test]
    fn collect_refuses_a_vault_that_is_not_the_one_its_row_names() {
        let bytes = artifact(Action::Collect);
        let program = ProgramV3::decode(&bytes).expect("decode");
        for count in [1_u32, 258] {
            let (scalars, identities) = collect_input_bank(count);
            fold(program, count, &scalars, &identities).expect("the named vaults accept");
            for coordinate in [
                identity::SOURCE_VAULT_CONTEXT,
                identity::DESTINATION_VAULT_CONTEXT,
            ] {
                let (scalars, mut identities) = collect_input_bank(count);
                // A real neighbouring vault, not a zero: this is the substitution
                // an adversary can actually present.
                identities[usize::try_from(coordinate).expect("identity coordinate")] = [9; 32];
                assert!(
                    fold(program, count, &scalars, &identities).is_err(),
                    "Collect at width {count} accepted a substituted vault at {coordinate}",
                );
            }
        }
    }

    #[test]
    fn nonexact_capacities_preserve_output() {
        let action = Action::Close;
        let (prelude, item, epilogue) = general_transition_instruction_count_v3(action);
        let mut instructions =
            vec![GENERAL_TRANSITION_INSTRUCTION_PLACEHOLDER_V3; prelude + item + epilogue];
        let bytes = general_transition_program_bytes_v3(action).expect("bytes");
        let mut scratch = vec![0_u8; bytes];
        let mut output = vec![0x55_u8; bytes - 1];
        let before = output.clone();
        assert_eq!(
            encode_general_transition_program_v3_atomic(
                action,
                &mut instructions,
                &mut scratch,
                &mut output,
            ),
            Err(GeneralTransitionArtifactErrorV3::Geometry)
        );
        assert_eq!(output, before);
    }
}
