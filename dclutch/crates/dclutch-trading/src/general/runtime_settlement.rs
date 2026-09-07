//! Permissionless runtime-width two-pass settlement over verified order manifests.
//!
//! Verification emits one immutable per-order manifest row only after applying
//! candidate-wide rounding and signed debit limits. Settlement consumes those
//! rows twice: collect delivered claims and quote debits, perform the unique
//! complete-set materialization, distribute received claims and quote credits,
//! then route the exact surplus and enter a terminal state. This evaluator owns
//! no account or CPI authority; it returns one complete effect-plan candidate
//! and one complete cursor candidate for generic Trading to execute and commit.
//!
//! # The strand (cohort-18, decision 0032 §2a)
//!
//! A joint clearing may leave the settlement holding claims after every row
//! is distributed: the residual `M − net_i` at an outcome the batch priced at
//! zero (`JointClearingV1.residual_worth_nothing`). Until the joint arm the
//! close REFUSED a nonzero inventory; now it requires the inventory to be
//! exactly that residual, re-derived from the verified certificate's own
//! prices, and STRANDS it -- the close's effect plan carries the residual as
//! its per-outcome quantities and the Claims leg burns them out of the
//! candidate's settlement Position (`ClaimsAction::StrandResidual`,
//! `EconomicKernel.strandPost`), with no atom leaving the Hoard.
//!
//! THE BURN HAS NO ROUTE YET, and this evaluator is the reason that is safe
//! rather than silent. `Action::Close` declares four child frames
//! (`effect_artifacts_v3.rs:189-210`) and none of them is a ProtocolPosition
//! mutation, so `hot_candidate_v3::position_geometry` refuses every Close
//! whose plan says `claims_active`. A close that strands therefore REFUSES
//! today instead of zeroing its cursor while the Position still holds the
//! claims. The clearing's own publication (`ClearingPriceV1`) is blocked on
//! the same wall from the other side: the batch account is not in the Close
//! frame at all, so the record's tail is written off chain by
//! `dclutch_operator::general_joint_clearing_v1::publish_clearing_v1` until a
//! frame carries it.
//!
//! An order the certificate left unfilled emitted no manifest row, so the
//! cursor's `order_count` is the verifier's FILLED count and a batch whose
//! book did not cross at all settles as `InitializeSettlement`, `Materialize`
//! (a no-op move) and `Close`.

use crate::general::runtime_manifest::{SettlementManifestV2, SettlementOrderV2};
use crate::general::runtime_verify::{
    RuntimeCandidateVerifierV2, RuntimeCompleteSetMoveV2, runtime_verified_balance_v2,
    runtime_verified_residual_v2,
};
use crate::general::runtime_width::{
    SettlementCursorHeaderV2, SettlementCursorV2, SettlementPhaseV2, VerifiedCandidateV2,
    settlement_cursor_len,
};

/// Exact fixed bytes before one runtime-width settlement quantity vector.
pub const RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2: usize = 192;

const EFFECT_MAGIC: [u8; 8] = *b"DCGFXP02";
const VERSION: u16 = 2;

/// Stable runtime settlement action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RuntimeSettlementActionV2 {
    /// Collect one verifier-derived order into settlement inventory.
    Collect = 1,
    /// Perform the certificate's unique complete-set movement.
    Materialize = 2,
    /// Distribute one verifier-derived order from settlement inventory.
    Distribute = 3,
    /// Route the exact surplus and make the cursor terminal.
    Close = 4,
}

impl RuntimeSettlementActionV2 {
    fn decode(value: u8) -> RuntimeSettlementResultV2<Self> {
        match value {
            1 => Ok(Self::Collect),
            2 => Ok(Self::Materialize),
            3 => Ok(Self::Distribute),
            4 => Ok(Self::Close),
            _ => Err(RuntimeSettlementErrorV2::InvalidEffect),
        }
    }
}

/// Stable refusal from runtime-width settlement evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSettlementErrorV2 {
    /// Cursor, certificate, verifier, manifest, or effect bytes refused.
    Codec,
    /// A caller-owned candidate or scratch bank had another exact width.
    InvalidLength,
    /// Candidate, order, revision, or terminal coordinates differed.
    CoordinateMismatch,
    /// The cursor phase did not admit the selected action.
    InvalidPhase,
    /// Claims or quote inventory could not fund the exact transition.
    Inventory,
    /// A checked quantity, revision, or byte calculation overflowed.
    ArithmeticOverflow,
    /// Effect-plan magic, tags, flags, or inactive fields were noncanonical.
    InvalidEffect,
}

/// Result alias for runtime-width settlement evaluation.
pub type RuntimeSettlementResultV2<T> = core::result::Result<T, RuntimeSettlementErrorV2>;

/// Fixed fields in one complete generic-Trading settlement effect candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSettlementEffectHeaderV2 {
    /// Runtime outcome width.
    pub outcome_count: u32,
    /// Settlement action selecting interpretation of the quantity tail.
    pub action: RuntimeSettlementActionV2,
    /// Complete-set direction, meaningful only for Materialize.
    pub complete_set_move: RuntimeCompleteSetMoveV2,
    /// Whether a Claims movement is active.
    pub claims_active: bool,
    /// Whether a Custody movement is active.
    pub custody_active: bool,
    /// Whether this transition commits terminal state.
    pub terminal: bool,
    /// One-based order coordinate for row actions; zero otherwise.
    pub order_coordinate: u32,
    /// Exact settlement revision consumed by the effect.
    pub revision: u64,
    /// Signed order nonce for row actions.
    pub nonce: u64,
    /// Candidate content identity.
    pub candidate_id: [u8; 32],
    /// Row owner identity, absent for aggregate actions.
    pub owner_id: [u8; 32],
    /// Row order identity, absent for aggregate actions.
    pub order_id: [u8; 32],
    /// Immutable close beneficiary, absent for non-Close actions.
    pub beneficiary: [u8; 32],
    /// Exact quote debit, credit, materialization principal, or surplus.
    pub quote_quantity: u64,
    /// Uniform complete-set quantity, zero outside Materialize.
    pub complete_set_quantity: u64,
    /// Nonzero terminal coordinate only for Close.
    pub terminal_coordinate: u64,
}

/// Borrowed complete settlement effect candidate with one `u64[N]` tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSettlementEffectPlanV2<'a> {
    bytes: &'a [u8],
    header: RuntimeSettlementEffectHeaderV2,
}

impl<'a> RuntimeSettlementEffectPlanV2<'a> {
    /// Hostile-decode one exact `192 + 8N` effect candidate.
    pub fn decode(bytes: &'a [u8]) -> RuntimeSettlementResultV2<Self> {
        if bytes.len() < RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2
            || bytes.get(..8) != Some(EFFECT_MAGIC.as_slice())
            || read_u16(bytes, 8)? != VERSION
        {
            return Err(RuntimeSettlementErrorV2::InvalidEffect);
        }
        let action = RuntimeSettlementActionV2::decode(read_byte(bytes, 10)?)?;
        let complete_set_move = decode_move(read_byte(bytes, 11)?)?;
        let flags = read_byte(bytes, 16)?;
        if flags & !0b111 != 0 || !zero(bytes, 17, 3)? {
            return Err(RuntimeSettlementErrorV2::InvalidEffect);
        }
        let header = RuntimeSettlementEffectHeaderV2 {
            outcome_count: read_u32(bytes, 12)?,
            action,
            complete_set_move,
            claims_active: flags & 1 != 0,
            custody_active: flags & 2 != 0,
            terminal: flags & 4 != 0,
            order_coordinate: read_u32(bytes, 20)?,
            revision: read_u64(bytes, 24)?,
            nonce: read_u64(bytes, 32)?,
            candidate_id: read_array32(bytes, 40)?,
            owner_id: read_array32(bytes, 72)?,
            order_id: read_array32(bytes, 104)?,
            beneficiary: read_array32(bytes, 136)?,
            quote_quantity: read_u64(bytes, 168)?,
            complete_set_quantity: read_u64(bytes, 176)?,
            terminal_coordinate: read_u64(bytes, 184)?,
        };
        if bytes.len() != runtime_settlement_effect_len_v2(header.outcome_count)? {
            return Err(RuntimeSettlementErrorV2::InvalidLength);
        }
        validate_effect(bytes, header)?;
        Ok(Self { bytes, header })
    }

    /// Return fixed effect coordinates and active roles.
    pub const fn header(self) -> RuntimeSettlementEffectHeaderV2 {
        self.header
    }

    /// Return one checked claim quantity for the selected action.
    pub fn quantity(self, index: u32) -> RuntimeSettlementResultV2<u64> {
        if index >= self.header.outcome_count {
            return Err(RuntimeSettlementErrorV2::InvalidLength);
        }
        let item = usize::try_from(index)
            .map_err(|_| RuntimeSettlementErrorV2::ArithmeticOverflow)?
            .checked_mul(8)
            .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
        read_u64(
            self.bytes,
            RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2
                .checked_add(item)
                .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?,
        )
    }

    /// Return exact canonical effect-candidate bytes.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Failure-atomic candidate banks for one settlement action.
pub struct RuntimeSettlementBuffersV2<'a> {
    /// Non-authoritative complete cursor scratch.
    pub cursor_scratch: &'a mut [u8],
    /// Cursor candidate unchanged on refusal.
    pub cursor_output: &'a mut [u8],
    /// Exact `8N` successor inventory scratch.
    pub inventory_scratch: &'a mut [u8],
    /// Non-authoritative complete effect scratch.
    pub effect_scratch: &'a mut [u8],
    /// Effect candidate unchanged on refusal.
    pub effect_output: &'a mut [u8],
}

/// Readonly inputs for one collect, materialize, distribute, or close action.
pub struct RuntimeSettlementViewV2<'a> {
    /// Selected settlement action.
    pub action: RuntimeSettlementActionV2,
    /// Canonical settlement cursor prestate.
    pub cursor_before: &'a [u8],
    /// Program-derived verified-candidate record.
    pub verified: &'a [u8],
    /// Verifier-emitted manifest chunk for row actions only.
    pub manifest: Option<&'a [u8]>,
    /// Selected row inside the supplied manifest chunk.
    pub manifest_order_index: u32,
    /// Exact optimistic settlement revision.
    pub expected_revision: u64,
    /// Immutable config-selected surplus beneficiary for Close only.
    pub surplus_beneficiary: Option<[u8; 32]>,
}

/// Return exact `192 + 8N` bytes for one settlement effect candidate.
pub fn runtime_settlement_effect_len_v2(outcome_count: u32) -> RuntimeSettlementResultV2<usize> {
    if outcome_count == 0 {
        return Err(RuntimeSettlementErrorV2::InvalidLength);
    }
    let count =
        usize::try_from(outcome_count).map_err(|_| RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2
        .checked_add(
            count
                .checked_mul(8)
                .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)
}

/// Initialize the permissionless settlement cursor from terminal verification.
#[inline(never)]
pub fn initialize_runtime_settlement_v2(
    verifier_bytes: &[u8],
    verified_bytes: &[u8],
    expected_revision: u64,
    inventory_scratch: &mut [u8],
    cursor_scratch: &mut [u8],
    cursor_output: &mut [u8],
) -> RuntimeSettlementResultV2<()> {
    let header = initial_settlement_header_v2(verifier_bytes, verified_bytes, expected_revision)?;
    let count = inventory_len(header.outcome_count)?;
    if inventory_scratch.len() != count
        || cursor_scratch.len()
            != settlement_cursor_len(header.outcome_count)
                .map_err(|_| RuntimeSettlementErrorV2::Codec)?
        || cursor_output.len() != cursor_scratch.len()
        || cursor_output.iter().any(|byte| *byte != 0)
    {
        return Err(RuntimeSettlementErrorV2::CoordinateMismatch);
    }
    inventory_scratch.fill(0);
    SettlementCursorV2::encode_le_inventory_into(header, inventory_scratch, cursor_scratch)
        .map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    cursor_output.copy_from_slice(cursor_scratch);
    Ok(())
}

/// Initialize directly into one exact zeroed cursor candidate.
///
/// This is semantically identical to [`initialize_runtime_settlement_v2`] but
/// avoids two additional runtime-width scratch banks. It is intended for
/// readonly SBF accelerators, where the output remains non-authoritative until
/// common Trading accepts the whole candidate digest.
#[inline(never)]
pub fn initialize_runtime_settlement_in_place_v2(
    verifier_bytes: &[u8],
    verified_bytes: &[u8],
    expected_revision: u64,
    cursor_output: &mut [u8],
) -> RuntimeSettlementResultV2<()> {
    let header = initial_settlement_header_v2(verifier_bytes, verified_bytes, expected_revision)?;
    if cursor_output.len()
        != settlement_cursor_len(header.outcome_count)
            .map_err(|_| RuntimeSettlementErrorV2::Codec)?
        || cursor_output.iter().any(|byte| *byte != 0)
    {
        return Err(RuntimeSettlementErrorV2::CoordinateMismatch);
    }
    SettlementCursorV2::encode_zero_inventory_into(header, cursor_output)
        .map_err(|_| RuntimeSettlementErrorV2::Codec)
}

fn initial_settlement_header_v2(
    verifier_bytes: &[u8],
    verified_bytes: &[u8],
    expected_revision: u64,
) -> RuntimeSettlementResultV2<SettlementCursorHeaderV2> {
    let verifier = RuntimeCandidateVerifierV2::decode(verifier_bytes)
        .map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    let verified =
        VerifiedCandidateV2::decode(verified_bytes).map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    let verifier_header = verifier.header();
    let verified_header = verified.header();
    if expected_revision != 0
        || !verifier.is_complete()
        || verifier_header.has_current_order
        || verifier_header.order_count == 0
        || verifier_header.filled_order_count > verifier_header.order_count
        || verifier_header.outcome_count != verified_header.outcome_count
        || verifier_header.candidate_coordinate != verified_header.candidate_coordinate
        || verifier_header.candidate_id != verified_header.candidate_id
        || verifier_header.product_id != verified_header.product_id
        || verifier_header.batch_id != verified_header.batch_id
        || verifier_header.revision != verified_header.revision
    {
        return Err(RuntimeSettlementErrorV2::CoordinateMismatch);
    }
    let balance =
        runtime_verified_balance_v2(verified_bytes).map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    // Only FILLED orders have rows to collect and distribute; a book that did
    // not cross settles straight into its (no-op) materialization.
    let order_count = verifier_header.filled_order_count;
    Ok(SettlementCursorHeaderV2 {
        outcome_count: verified_header.outcome_count,
        order_count,
        next_order: 0,
        revision: 1,
        candidate_id: verified_header.candidate_id,
        quote_inventory: 0,
        complete_set_quantity: balance.complete_set_quantity,
        terminal_coordinate: 0,
        phase: if order_count == 0 {
            SettlementPhaseV2::Materializing
        } else {
            SettlementPhaseV2::Collecting
        },
    })
}

/// Evaluate one complete runtime settlement action failure-atomically.
#[inline(never)]
pub fn evaluate_runtime_settlement_v2(
    view: RuntimeSettlementViewV2<'_>,
    buffers: RuntimeSettlementBuffersV2<'_>,
) -> RuntimeSettlementResultV2<()> {
    let effect_bytes = runtime_settlement_effect_len_v2(
        SettlementCursorV2::decode(view.cursor_before)
            .map_err(|_| RuntimeSettlementErrorV2::Codec)?
            .header()
            .outcome_count,
    )?;
    if buffers.cursor_output.len() != view.cursor_before.len()
        || buffers.effect_output.len() != effect_bytes
    {
        return Err(RuntimeSettlementErrorV2::CoordinateMismatch);
    }
    evaluate_runtime_settlement_in_place_v2(
        view,
        buffers.cursor_scratch,
        buffers.inventory_scratch,
        buffers.effect_scratch,
    )?;
    buffers
        .cursor_output
        .copy_from_slice(buffers.cursor_scratch);
    buffers
        .effect_output
        .copy_from_slice(buffers.effect_scratch);
    Ok(())
}

/// Evaluate into non-authoritative workspaces without duplicate output banks.
///
/// The caller must discard all three workspaces on error. This is the bounded
/// SBF accelerator path: common Trading accepts nothing until the complete
/// candidate-bank acknowledgement authenticates, so partially changed scratch
/// has no authority. [`evaluate_runtime_settlement_v2`] remains the atomic
/// output API for callers that expose separate candidate buffers.
#[inline(never)]
pub fn evaluate_runtime_settlement_in_place_v2(
    view: RuntimeSettlementViewV2<'_>,
    cursor_workspace: &mut [u8],
    inventory_workspace: &mut [u8],
    effect_workspace: &mut [u8],
) -> RuntimeSettlementResultV2<()> {
    let cursor = SettlementCursorV2::decode(view.cursor_before)
        .map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    let verified =
        VerifiedCandidateV2::decode(view.verified).map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    let cursor_header = cursor.header();
    let verified_header = verified.header();
    let inventory_bytes = inventory_len(cursor_header.outcome_count)?;
    let effect_bytes = runtime_settlement_effect_len_v2(cursor_header.outcome_count)?;
    if cursor_header.outcome_count != verified_header.outcome_count
        || cursor_header.candidate_id != verified_header.candidate_id
        || cursor_header.revision != view.expected_revision
        || cursor_workspace.len() != view.cursor_before.len()
        || inventory_workspace.len() != inventory_bytes
        || effect_workspace.len() != effect_bytes
    {
        return Err(RuntimeSettlementErrorV2::CoordinateMismatch);
    }
    let order = selected_order(&view, cursor_header)?;
    let mut successor = cursor_header;
    let consumed_revision = successor.revision;
    successor.revision = successor
        .revision
        .checked_add(1)
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    copy_inventory(cursor, inventory_workspace)?;
    let effect_header = match view.action {
        RuntimeSettlementActionV2::Collect => collect(
            order.ok_or(RuntimeSettlementErrorV2::CoordinateMismatch)?,
            &mut successor,
            inventory_workspace,
            consumed_revision,
        )?,
        RuntimeSettlementActionV2::Materialize => materialize(
            view.verified,
            &mut successor,
            inventory_workspace,
            consumed_revision,
        )?,
        RuntimeSettlementActionV2::Distribute => distribute(
            order.ok_or(RuntimeSettlementErrorV2::CoordinateMismatch)?,
            &mut successor,
            inventory_workspace,
            consumed_revision,
        )?,
        RuntimeSettlementActionV2::Close => close(
            view.verified,
            &mut successor,
            inventory_workspace,
            view.surplus_beneficiary
                .ok_or(RuntimeSettlementErrorV2::CoordinateMismatch)?,
            consumed_revision,
        )?,
    };
    SettlementCursorV2::encode_le_inventory_into(successor, inventory_workspace, cursor_workspace)
        .map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    encode_effect_plan(effect_header, order, view.verified, effect_workspace)?;
    RuntimeSettlementEffectPlanV2::decode(effect_workspace)?;
    Ok(())
}

fn selected_order<'a>(
    view: &RuntimeSettlementViewV2<'a>,
    cursor: SettlementCursorHeaderV2,
) -> RuntimeSettlementResultV2<Option<SettlementOrderV2<'a>>> {
    let row_action = matches!(
        view.action,
        RuntimeSettlementActionV2::Collect | RuntimeSettlementActionV2::Distribute
    );
    if row_action != view.manifest.is_some()
        || (!row_action && view.manifest_order_index != 0)
        || (view.action != RuntimeSettlementActionV2::Close && view.surplus_beneficiary.is_some())
    {
        return Err(RuntimeSettlementErrorV2::CoordinateMismatch);
    }
    let Some(bytes) = view.manifest else {
        return Ok(None);
    };
    let manifest =
        SettlementManifestV2::decode(bytes).map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    let order = manifest
        .order(view.manifest_order_index)
        .map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    let header = order.header();
    let expected_order = cursor
        .next_order
        .checked_add(1)
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    if header.outcome_count != cursor.outcome_count
        || header.candidate_id != cursor.candidate_id
        || header.order_coordinate != expected_order
    {
        return Err(RuntimeSettlementErrorV2::CoordinateMismatch);
    }
    Ok(Some(order))
}

fn collect(
    order: SettlementOrderV2<'_>,
    cursor: &mut SettlementCursorHeaderV2,
    inventory: &mut [u8],
    consumed_revision: u64,
) -> RuntimeSettlementResultV2<RuntimeSettlementEffectHeaderV2> {
    if cursor.phase != SettlementPhaseV2::Collecting {
        return Err(RuntimeSettlementErrorV2::InvalidPhase);
    }
    let order_header = order.header();
    for outcome in 0..cursor.outcome_count {
        write_inventory(
            inventory,
            outcome,
            add(
                read_inventory(inventory, outcome)?,
                order
                    .claim_input(outcome)
                    .map_err(|_| RuntimeSettlementErrorV2::Codec)?,
            )?,
        )?;
    }
    cursor.quote_inventory = add(cursor.quote_inventory, order_header.quote_debit)?;
    advance(cursor, SettlementPhaseV2::Materializing)?;
    row_effect(RuntimeSettlementActionV2::Collect, order, consumed_revision)
}

fn materialize(
    verified: &[u8],
    cursor: &mut SettlementCursorHeaderV2,
    inventory: &mut [u8],
    consumed_revision: u64,
) -> RuntimeSettlementResultV2<RuntimeSettlementEffectHeaderV2> {
    if cursor.phase != SettlementPhaseV2::Materializing || cursor.next_order != cursor.order_count {
        return Err(RuntimeSettlementErrorV2::InvalidPhase);
    }
    let balance =
        runtime_verified_balance_v2(verified).map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    if cursor.complete_set_quantity != balance.complete_set_quantity {
        return Err(RuntimeSettlementErrorV2::CoordinateMismatch);
    }
    match balance.complete_set_move {
        RuntimeCompleteSetMoveV2::None => {}
        RuntimeCompleteSetMoveV2::Mint => {
            cursor.quote_inventory = cursor
                .quote_inventory
                .checked_sub(balance.complete_set_quantity)
                .ok_or(RuntimeSettlementErrorV2::Inventory)?;
            for outcome in 0..cursor.outcome_count {
                write_inventory(
                    inventory,
                    outcome,
                    add(
                        read_inventory(inventory, outcome)?,
                        balance.complete_set_quantity,
                    )?,
                )?;
            }
        }
        RuntimeCompleteSetMoveV2::Merge => {
            for outcome in 0..cursor.outcome_count {
                let successor = read_inventory(inventory, outcome)?
                    .checked_sub(balance.complete_set_quantity)
                    .ok_or(RuntimeSettlementErrorV2::Inventory)?;
                write_inventory(inventory, outcome, successor)?;
            }
            cursor.quote_inventory = add(cursor.quote_inventory, balance.complete_set_quantity)?;
        }
    }
    cursor.phase = if cursor.order_count == 0 {
        SettlementPhaseV2::ReadyToClose
    } else {
        SettlementPhaseV2::Distributing
    };
    cursor.next_order = 0;
    Ok(aggregate_effect(
        RuntimeSettlementActionV2::Materialize,
        cursor,
        balance.complete_set_move,
        balance.complete_set_quantity,
        [0; 32],
        0,
        consumed_revision,
    ))
}

fn distribute(
    order: SettlementOrderV2<'_>,
    cursor: &mut SettlementCursorHeaderV2,
    inventory: &mut [u8],
    consumed_revision: u64,
) -> RuntimeSettlementResultV2<RuntimeSettlementEffectHeaderV2> {
    if cursor.phase != SettlementPhaseV2::Distributing {
        return Err(RuntimeSettlementErrorV2::InvalidPhase);
    }
    let order_header = order.header();
    for outcome in 0..cursor.outcome_count {
        let successor = read_inventory(inventory, outcome)?
            .checked_sub(
                order
                    .claim_output(outcome)
                    .map_err(|_| RuntimeSettlementErrorV2::Codec)?,
            )
            .ok_or(RuntimeSettlementErrorV2::Inventory)?;
        write_inventory(inventory, outcome, successor)?;
    }
    cursor.quote_inventory = cursor
        .quote_inventory
        .checked_sub(order_header.quote_credit)
        .ok_or(RuntimeSettlementErrorV2::Inventory)?;
    advance(cursor, SettlementPhaseV2::ReadyToClose)?;
    row_effect(
        RuntimeSettlementActionV2::Distribute,
        order,
        consumed_revision,
    )
}

/// THE CLOSE STRANDS THE RESIDUAL. What the settlement still holds after
/// every row was distributed must be exactly the certificate's residual at
/// each outcome -- `M − net_i`, nonzero only where the price is zero -- and
/// the close burns it: the effect's per-outcome quantities are the burn, its
/// Claims leg is `StrandResidual`, and no Custody movement accompanies them.
/// The surplus route is unchanged.
fn close(
    verified: &[u8],
    cursor: &mut SettlementCursorHeaderV2,
    inventory: &mut [u8],
    beneficiary: [u8; 32],
    consumed_revision: u64,
) -> RuntimeSettlementResultV2<RuntimeSettlementEffectHeaderV2> {
    let terminal_coordinate = consumed_revision
        .checked_add(1)
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    if cursor.phase != SettlementPhaseV2::ReadyToClose
        || cursor.next_order != cursor.order_count
        || zero_identity(&beneficiary)
    {
        return Err(RuntimeSettlementErrorV2::InvalidPhase);
    }
    let balance =
        runtime_verified_balance_v2(verified).map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    if cursor.quote_inventory != balance.quote_surplus {
        return Err(RuntimeSettlementErrorV2::Inventory);
    }
    let mut strands = false;
    for outcome in 0..cursor.outcome_count {
        let residual = runtime_verified_residual_v2(verified, outcome)
            .map_err(|_| RuntimeSettlementErrorV2::Codec)?;
        if read_inventory(inventory, outcome)? != residual {
            return Err(RuntimeSettlementErrorV2::Inventory);
        }
        strands |= residual != 0;
        write_inventory(inventory, outcome, 0)?;
    }
    let surplus = cursor.quote_inventory;
    cursor.quote_inventory = 0;
    cursor.terminal_coordinate = terminal_coordinate;
    cursor.phase = SettlementPhaseV2::Terminal;
    let mut header = aggregate_effect(
        RuntimeSettlementActionV2::Close,
        cursor,
        RuntimeCompleteSetMoveV2::None,
        surplus,
        beneficiary,
        terminal_coordinate,
        consumed_revision,
    );
    header.claims_active = strands;
    Ok(header)
}

fn advance(
    cursor: &mut SettlementCursorHeaderV2,
    final_phase: SettlementPhaseV2,
) -> RuntimeSettlementResultV2<()> {
    cursor.next_order = cursor
        .next_order
        .checked_add(1)
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    if cursor.next_order == cursor.order_count {
        cursor.phase = final_phase;
    } else if cursor.next_order > cursor.order_count {
        return Err(RuntimeSettlementErrorV2::CoordinateMismatch);
    }
    Ok(())
}

fn row_effect(
    action: RuntimeSettlementActionV2,
    order: SettlementOrderV2<'_>,
    revision: u64,
) -> RuntimeSettlementResultV2<RuntimeSettlementEffectHeaderV2> {
    let order_header = order.header();
    let collect = action == RuntimeSettlementActionV2::Collect;
    let mut claims_active = false;
    for outcome in 0..order_header.outcome_count {
        let quantity = if collect {
            order
                .claim_input(outcome)
                .map_err(|_| RuntimeSettlementErrorV2::Codec)?
        } else {
            order
                .claim_output(outcome)
                .map_err(|_| RuntimeSettlementErrorV2::Codec)?
        };
        claims_active |= quantity != 0;
    }
    Ok(RuntimeSettlementEffectHeaderV2 {
        outcome_count: order_header.outcome_count,
        action,
        complete_set_move: RuntimeCompleteSetMoveV2::None,
        claims_active,
        custody_active: if collect {
            order_header.quote_debit != 0
        } else {
            order_header.quote_credit != 0
        },
        terminal: false,
        order_coordinate: order_header.order_coordinate,
        revision,
        nonce: order_header.nonce,
        candidate_id: order_header.candidate_id,
        owner_id: order_header.owner_id,
        order_id: order_header.order_id,
        beneficiary: [0; 32],
        quote_quantity: if collect {
            order_header.quote_debit
        } else {
            order_header.quote_credit
        },
        complete_set_quantity: 0,
        terminal_coordinate: 0,
    })
}

fn aggregate_effect(
    action: RuntimeSettlementActionV2,
    cursor: &SettlementCursorHeaderV2,
    movement: RuntimeCompleteSetMoveV2,
    quantity: u64,
    beneficiary: [u8; 32],
    terminal_coordinate: u64,
    consumed_revision: u64,
) -> RuntimeSettlementEffectHeaderV2 {
    let materialize = action == RuntimeSettlementActionV2::Materialize;
    RuntimeSettlementEffectHeaderV2 {
        outcome_count: cursor.outcome_count,
        action,
        complete_set_move: movement,
        claims_active: materialize && movement != RuntimeCompleteSetMoveV2::None,
        custody_active: quantity != 0,
        terminal: action == RuntimeSettlementActionV2::Close,
        order_coordinate: 0,
        revision: consumed_revision,
        nonce: 0,
        candidate_id: cursor.candidate_id,
        owner_id: [0; 32],
        order_id: [0; 32],
        beneficiary,
        quote_quantity: quantity,
        complete_set_quantity: if materialize { quantity } else { 0 },
        terminal_coordinate,
    }
}

fn encode_effect_plan(
    header: RuntimeSettlementEffectHeaderV2,
    order: Option<SettlementOrderV2<'_>>,
    verified: &[u8],
    output: &mut [u8],
) -> RuntimeSettlementResultV2<()> {
    if output.len() != runtime_settlement_effect_len_v2(header.outcome_count)? {
        return Err(RuntimeSettlementErrorV2::InvalidLength);
    }
    output.fill(0);
    put(output, 0, &EFFECT_MAGIC)?;
    put_u16(output, 8, VERSION)?;
    put_byte(output, 10, header.action as u8)?;
    put_byte(output, 11, move_tag(header.complete_set_move))?;
    put_u32(output, 12, header.outcome_count)?;
    let flags = u8::from(header.claims_active)
        | (u8::from(header.custody_active) << 1)
        | (u8::from(header.terminal) << 2);
    put_byte(output, 16, flags)?;
    put_u32(output, 20, header.order_coordinate)?;
    put_u64(output, 24, header.revision)?;
    put_u64(output, 32, header.nonce)?;
    put(output, 40, &header.candidate_id)?;
    put(output, 72, &header.owner_id)?;
    put(output, 104, &header.order_id)?;
    put(output, 136, &header.beneficiary)?;
    put_u64(output, 168, header.quote_quantity)?;
    put_u64(output, 176, header.complete_set_quantity)?;
    put_u64(output, 184, header.terminal_coordinate)?;
    for outcome in 0..header.outcome_count {
        let quantity = match header.action {
            RuntimeSettlementActionV2::Collect => order
                .ok_or(RuntimeSettlementErrorV2::CoordinateMismatch)?
                .claim_input(outcome)
                .map_err(|_| RuntimeSettlementErrorV2::Codec)?,
            RuntimeSettlementActionV2::Distribute => order
                .ok_or(RuntimeSettlementErrorV2::CoordinateMismatch)?
                .claim_output(outcome)
                .map_err(|_| RuntimeSettlementErrorV2::Codec)?,
            RuntimeSettlementActionV2::Materialize => header.complete_set_quantity,
            // The close's quantities are the residual it strands.
            RuntimeSettlementActionV2::Close => runtime_verified_residual_v2(verified, outcome)
                .map_err(|_| RuntimeSettlementErrorV2::Codec)?,
        };
        write_effect_quantity(output, outcome, quantity)?;
    }
    let verified_header = VerifiedCandidateV2::decode(verified)
        .map_err(|_| RuntimeSettlementErrorV2::Codec)?
        .header();
    if verified_header.candidate_id != header.candidate_id
        || verified_header.outcome_count != header.outcome_count
    {
        return Err(RuntimeSettlementErrorV2::CoordinateMismatch);
    }
    Ok(())
}

fn validate_effect(
    bytes: &[u8],
    header: RuntimeSettlementEffectHeaderV2,
) -> RuntimeSettlementResultV2<()> {
    if header.outcome_count == 0 || header.revision == 0 || zero_identity(&header.candidate_id) {
        return Err(RuntimeSettlementErrorV2::InvalidEffect);
    }
    let row = matches!(
        header.action,
        RuntimeSettlementActionV2::Collect | RuntimeSettlementActionV2::Distribute
    );
    if row
        != (header.order_coordinate != 0
            && !zero_identity(&header.owner_id)
            && !zero_identity(&header.order_id))
        || (!row
            && (!zero_identity(&header.owner_id)
                || !zero_identity(&header.order_id)
                || header.nonce != 0))
        || (header.action == RuntimeSettlementActionV2::Close)
            != (header.terminal
                && header.terminal_coordinate != 0
                && !zero_identity(&header.beneficiary))
        || (header.action != RuntimeSettlementActionV2::Close
            && (!zero_identity(&header.beneficiary) || header.terminal_coordinate != 0))
    {
        return Err(RuntimeSettlementErrorV2::InvalidEffect);
    }
    match header.action {
        RuntimeSettlementActionV2::Collect | RuntimeSettlementActionV2::Distribute => {
            if header.complete_set_move != RuntimeCompleteSetMoveV2::None
                || header.complete_set_quantity != 0
                || header.terminal
            {
                return Err(RuntimeSettlementErrorV2::InvalidEffect);
            }
        }
        RuntimeSettlementActionV2::Materialize => {
            let canonical = match header.complete_set_move {
                RuntimeCompleteSetMoveV2::None => header.complete_set_quantity == 0,
                RuntimeCompleteSetMoveV2::Mint | RuntimeCompleteSetMoveV2::Merge => {
                    header.complete_set_quantity != 0
                }
            };
            if !canonical || header.terminal {
                return Err(RuntimeSettlementErrorV2::InvalidEffect);
            }
        }
        RuntimeSettlementActionV2::Close => {
            if header.complete_set_move != RuntimeCompleteSetMoveV2::None
                || header.complete_set_quantity != 0
            {
                return Err(RuntimeSettlementErrorV2::InvalidEffect);
            }
        }
    }
    let mut any_claim = false;
    for outcome in 0..header.outcome_count {
        any_claim |= read_effect_quantity(bytes, outcome)? != 0;
    }
    if any_claim != header.claims_active || (header.quote_quantity != 0) != header.custody_active {
        return Err(RuntimeSettlementErrorV2::InvalidEffect);
    }
    Ok(())
}

fn copy_inventory(
    cursor: SettlementCursorV2<'_>,
    output: &mut [u8],
) -> RuntimeSettlementResultV2<()> {
    for outcome in 0..cursor.header().outcome_count {
        write_inventory(
            output,
            outcome,
            cursor
                .inventory(outcome)
                .map_err(|_| RuntimeSettlementErrorV2::Codec)?,
        )?;
    }
    Ok(())
}

fn inventory_len(count: u32) -> RuntimeSettlementResultV2<usize> {
    usize::try_from(count)
        .map_err(|_| RuntimeSettlementErrorV2::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)
}

fn read_inventory(bytes: &[u8], index: u32) -> RuntimeSettlementResultV2<u64> {
    let offset = usize::try_from(index)
        .map_err(|_| RuntimeSettlementErrorV2::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    read_u64(bytes, offset)
}

fn write_inventory(bytes: &mut [u8], index: u32, value: u64) -> RuntimeSettlementResultV2<()> {
    let offset = usize::try_from(index)
        .map_err(|_| RuntimeSettlementErrorV2::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    put_u64(bytes, offset, value)
}

fn read_effect_quantity(bytes: &[u8], index: u32) -> RuntimeSettlementResultV2<u64> {
    let offset = usize::try_from(index)
        .map_err(|_| RuntimeSettlementErrorV2::ArithmeticOverflow)?
        .checked_mul(8)
        .and_then(|item| RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2.checked_add(item))
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    read_u64(bytes, offset)
}

fn write_effect_quantity(
    bytes: &mut [u8],
    index: u32,
    value: u64,
) -> RuntimeSettlementResultV2<()> {
    let offset = usize::try_from(index)
        .map_err(|_| RuntimeSettlementErrorV2::ArithmeticOverflow)?
        .checked_mul(8)
        .and_then(|item| RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2.checked_add(item))
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    put_u64(bytes, offset, value)
}

fn move_tag(value: RuntimeCompleteSetMoveV2) -> u8 {
    match value {
        RuntimeCompleteSetMoveV2::None => 0,
        RuntimeCompleteSetMoveV2::Mint => 1,
        RuntimeCompleteSetMoveV2::Merge => 2,
    }
}

fn decode_move(value: u8) -> RuntimeSettlementResultV2<RuntimeCompleteSetMoveV2> {
    match value {
        0 => Ok(RuntimeCompleteSetMoveV2::None),
        1 => Ok(RuntimeCompleteSetMoveV2::Mint),
        2 => Ok(RuntimeCompleteSetMoveV2::Merge),
        _ => Err(RuntimeSettlementErrorV2::InvalidEffect),
    }
}

fn zero_identity(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn add(left: u64, right: u64) -> RuntimeSettlementResultV2<u64> {
    left.checked_add(right)
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)
}

fn read_byte(bytes: &[u8], offset: usize) -> RuntimeSettlementResultV2<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(RuntimeSettlementErrorV2::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> RuntimeSettlementResultV2<u16> {
    let end = offset
        .checked_add(2)
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    let array = <[u8; 2]>::try_from(
        bytes
            .get(offset..end)
            .ok_or(RuntimeSettlementErrorV2::InvalidLength)?,
    )
    .map_err(|_| RuntimeSettlementErrorV2::InvalidLength)?;
    Ok(u16::from_le_bytes(array))
}

fn read_u32(bytes: &[u8], offset: usize) -> RuntimeSettlementResultV2<u32> {
    let end = offset
        .checked_add(4)
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    let array = <[u8; 4]>::try_from(
        bytes
            .get(offset..end)
            .ok_or(RuntimeSettlementErrorV2::InvalidLength)?,
    )
    .map_err(|_| RuntimeSettlementErrorV2::InvalidLength)?;
    Ok(u32::from_le_bytes(array))
}

fn read_u64(bytes: &[u8], offset: usize) -> RuntimeSettlementResultV2<u64> {
    let end = offset
        .checked_add(8)
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    let array = <[u8; 8]>::try_from(
        bytes
            .get(offset..end)
            .ok_or(RuntimeSettlementErrorV2::InvalidLength)?,
    )
    .map_err(|_| RuntimeSettlementErrorV2::InvalidLength)?;
    Ok(u64::from_le_bytes(array))
}

fn read_array32(bytes: &[u8], offset: usize) -> RuntimeSettlementResultV2<[u8; 32]> {
    let end = offset
        .checked_add(32)
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    <[u8; 32]>::try_from(
        bytes
            .get(offset..end)
            .ok_or(RuntimeSettlementErrorV2::InvalidLength)?,
    )
    .map_err(|_| RuntimeSettlementErrorV2::InvalidLength)
}

fn zero(bytes: &[u8], offset: usize, length: usize) -> RuntimeSettlementResultV2<bool> {
    let end = offset
        .checked_add(length)
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    Ok(bytes
        .get(offset..end)
        .ok_or(RuntimeSettlementErrorV2::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0))
}

fn put(bytes: &mut [u8], offset: usize, value: &[u8]) -> RuntimeSettlementResultV2<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    bytes
        .get_mut(offset..end)
        .ok_or(RuntimeSettlementErrorV2::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(bytes: &mut [u8], offset: usize, value: u8) -> RuntimeSettlementResultV2<()> {
    *bytes
        .get_mut(offset)
        .ok_or(RuntimeSettlementErrorV2::InvalidLength)? = value;
    Ok(())
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) -> RuntimeSettlementResultV2<()> {
    put(bytes, offset, &value.to_le_bytes())
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) -> RuntimeSettlementResultV2<()> {
    put(bytes, offset, &value.to_le_bytes())
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) -> RuntimeSettlementResultV2<()> {
    put(bytes, offset, &value.to_le_bytes())
}

#[cfg(test)]
mod tests;
