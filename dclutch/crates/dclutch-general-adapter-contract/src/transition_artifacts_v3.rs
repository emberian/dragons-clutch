//! Action-selected TransitionVM programs for General V3.
//!
//! These programs are the data-defined semantic gate paired with the admitted
//! read-only accelerator. They derive canonical action, local-state, cursor,
//! replay, and child-request coordinates in the same register bank consumed by
//! the common EffectProgram. The Product-owned tail is folded at runtime; no
//! outcome width is compiled into an artifact.
//!
//! **These seven programs are now Lean-authored.**
//! `formal/dclutch-semantics/DClutchSemantics/GeneralTransitionV3.lean` states
//! every conjunct; `EmitGeneralTransitionV3Rust.lean` emits
//! `generated_transition_programs_v3.rs`; and
//! `every_authored_program_is_byte_identical_to_the_lean_authored_one` below
//! requires the builder in this module to reproduce those bytes exactly, for
//! all seven actions.
//!
//! Until this landed the General family had no Lean counterpart for its
//! transition artifacts at all -- the same gap `73f0793` closed for Direct --
//! and the imperative builder below was the sole authority for what a General
//! release admits. Nothing changed shape: the gate is byte-identity with what
//! the builder already produced, so no artifact digest moved.
//!
//! The seven collection and candidate actions have no program in either place.
//! Authoring theirs in Lean, and deleting the builder in favour of the emitted
//! arrays, is the remaining work; the byte gate is what makes the second step
//! safe to take one action at a time.

use crate::effect_artifacts_v3::unauthored_actions;
use dclutch_claims_svm::affine_batch_v2::DeltaDirectionV2;
use dclutch_custody_contract::OperationV1;
use dclutch_general_codec::Action;
use dclutch_general_config_contract::GeneralLifecycleV2;
use dclutch_transition_vm::v3::{
    HEADER_BYTES, INSTRUCTION_BYTES, IdentityRegisterV3, InstructionV3, ProgramGeometryV3,
    ProgramV3, ScalarRegisterV3, encode_program_atomic,
};

use crate::{
    collection_v1::{
        BatchStatusV1, GeneralBatchLayoutV1, GeneralOrderLayoutV1, GeneralOrderPhaseV1,
    },
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, GENERAL_HOT_COMMON_SCALARS_V3,
        GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3, GENERAL_HOT_ITEM_SCALAR_STRIDE_V3, identity,
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
    Transition(dclutch_transition_vm::v3::Error),
    /// The action is a declared protocol selector with no authored artifacts.
    UnauthoredAction,
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
/// The empty slice is not a usable program: `ProgramV3::decode` refuses it, so
/// an unauthored action cannot be admitted with any program at all. It is the
/// same fail-closed shape `general_request_profile_bytes_v1` uses.
#[must_use]
pub const fn general_transition_program_bytes_lean_v3(action: Action) -> &'static [u8] {
    match action {
        unauthored_actions!() => &[],
        Action::OpenBatch => &GENERAL_OPEN_BATCH_TRANSITION_V3,
        Action::CloseBatch => &GENERAL_CLOSE_BATCH_TRANSITION_V3,
        Action::PlaceOrder => &GENERAL_PLACE_ORDER_TRANSITION_V3,
        Action::CancelOrder => &GENERAL_CANCEL_ORDER_TRANSITION_V3,
        Action::ReleaseOrder => &GENERAL_RELEASE_ORDER_TRANSITION_V3,
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
/// [`append_common`], so all seven counts moved together when it landed.
#[must_use]
pub const fn general_transition_instruction_count_v3(action: Action) -> (usize, usize, usize) {
    match action {
        unauthored_actions!() => (0, 0, 0),
        Action::OpenBatch => (26, 1, 0),
        Action::CloseBatch => (27, 1, 0),
        Action::PlaceOrder => (45, 4, 0),
        Action::CancelOrder => (49, 4, 0),
        Action::ReleaseOrder => (42, 4, 0),
        Action::Consider => (15, 1, 0),
        Action::Freeze => (17, 1, 0),
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
    if !crate::effect_artifacts_v3::general_action_artifacts_authored_v3(action) {
        return Err(GeneralTransitionArtifactErrorV3::UnauthoredAction);
    }
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
            item_scalar_stride: narrow(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)?,
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
        Action::SubmitCandidate | Action::VerifyCandidateRow => GeneralLocalStateKindV3::Candidate,
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
        unauthored_actions!() => {
            return Err(GeneralTransitionArtifactErrorV3::UnauthoredAction);
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
                // EXACTLY, so strictly-after-valid is strictly-after-the-window
                // and no batch account enters the frame.
                InstructionV3::scalar_lt(
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
    push(
        output,
        cursor,
        InstructionV3::scalar_lt(is(item_scalar::OUTCOME)?, s(scalar::OUTCOME_COUNT)?),
    )?;
    match action {
        unauthored_actions!() => {
            return Err(GeneralTransitionArtifactErrorV3::UnauthoredAction);
        }
        Action::Consider | Action::Freeze | Action::Materialize => {}
        // The batch record has no per-outcome tail; the bound check alone.
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

    use dclutch_transition_vm::v3::{RegisterInput, RegisterOutput, execute_fold_atomic};
    use std::vec;

    use super::*;

    const ACTIVE_LIFECYCLE: u64 = GeneralLifecycleV2::Active.tag() as u64;

    const ACTIONS: [Action; 12] = [
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
        Action::ReleaseOrder,
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
            GENERAL_HOT_ITEM_SCALAR_STRIDE_V3
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
        assert!(scalar::ROOT_LIFECYCLE_ACTIVE < GENERAL_TRANSITION_COMMON_SCALARS_V3);
        assert!(item_scalar::CURSOR_INVENTORY < GENERAL_TRANSITION_ITEM_SCALAR_STRIDE_V3);
        assert!(identity::TERMINAL_OWNER < GENERAL_TRANSITION_COMMON_IDENTITIES_V3);
    }

    /// An unauthored action borrows no program, and the empty slice is refused
    /// by the decoder rather than treated as a permissive one.
    #[test]
    fn an_unauthored_action_borrows_no_lean_authored_program() {
        for action in [Action::SubmitCandidate, Action::VerifyCandidateRow] {
            let bytes = general_transition_program_bytes_lean_v3(action);
            assert!(bytes.is_empty(), "{action:?} borrowed a program");
            assert!(ProgramV3::decode(bytes).is_err());
        }
    }

    #[test]
    fn all_seven_actions_emit_distinct_nontrivial_runtime_tail_programs() {
        let mut prior: Option<std::vec::Vec<u8>> = None;
        for action in ACTIONS {
            let bytes = artifact(action);
            assert!(bytes.len() >= HEADER_BYTES + 13 * INSTRUCTION_BYTES);
            let program = ProgramV3::decode(&bytes).expect("decode");
            assert_eq!(
                program.common_scalar_count(),
                u16::try_from(GENERAL_HOT_COMMON_SCALARS_V3).expect("scalar count")
            );
            assert_eq!(
                program.item_scalar_stride(),
                u16::try_from(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3).expect("item stride")
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
            GENERAL_HOT_COMMON_SCALARS_V3 + count * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3,
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
            let base = GENERAL_HOT_COMMON_SCALARS_V3 + item * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
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
            let base = GENERAL_HOT_COMMON_SCALARS_V3 + item * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
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
