//! Permissionless runtime-width two-pass settlement over verified order manifests.
//!
//! Verification emits one immutable per-order manifest row only after applying
//! candidate-wide rounding and signed debit limits. Settlement consumes those
//! rows twice: collect delivered claims and quote debits, perform the unique
//! complete-set materialization, distribute received claims and quote credits,
//! then route the exact surplus and enter a terminal state. This evaluator owns
//! no account or CPI authority; it returns one complete effect-plan candidate
//! and one complete cursor candidate for generic Trading to execute and commit.

use crate::runtime_manifest::{SettlementManifestV2, SettlementOrderV2};
use crate::runtime_verify::{
    RuntimeCandidateVerifierV2, RuntimeCompleteSetMoveV2, runtime_verified_balance_v2,
};
use crate::runtime_width::{
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
    let verifier = RuntimeCandidateVerifierV2::decode(verifier_bytes)
        .map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    let verified =
        VerifiedCandidateV2::decode(verified_bytes).map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    let verifier_header = verifier.header();
    let verified_header = verified.header();
    let count = inventory_len(verified_header.outcome_count)?;
    if expected_revision != 0
        || !verifier.is_complete()
        || verifier_header.has_current_order
        || verifier_header.order_count == 0
        || verifier_header.outcome_count != verified_header.outcome_count
        || verifier_header.candidate_coordinate != verified_header.candidate_coordinate
        || verifier_header.candidate_id != verified_header.candidate_id
        || verifier_header.product_id != verified_header.product_id
        || verifier_header.batch_id != verified_header.batch_id
        || verifier_header.revision != verified_header.revision
        || inventory_scratch.len() != count
        || cursor_scratch.len()
            != settlement_cursor_len(verified_header.outcome_count)
                .map_err(|_| RuntimeSettlementErrorV2::Codec)?
        || cursor_output.len() != cursor_scratch.len()
        || cursor_output.iter().any(|byte| *byte != 0)
    {
        return Err(RuntimeSettlementErrorV2::CoordinateMismatch);
    }
    let balance =
        runtime_verified_balance_v2(verified_bytes).map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    inventory_scratch.fill(0);
    SettlementCursorV2::encode_le_inventory_into(
        SettlementCursorHeaderV2 {
            outcome_count: verified_header.outcome_count,
            order_count: verifier_header.order_count,
            next_order: 0,
            revision: 1,
            candidate_id: verified_header.candidate_id,
            quote_inventory: 0,
            complete_set_quantity: balance.complete_set_quantity,
            terminal_coordinate: 0,
            phase: SettlementPhaseV2::Collecting,
        },
        inventory_scratch,
        cursor_scratch,
    )
    .map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    cursor_output.copy_from_slice(cursor_scratch);
    Ok(())
}

/// Evaluate one complete runtime settlement action failure-atomically.
#[inline(never)]
pub fn evaluate_runtime_settlement_v2(
    view: RuntimeSettlementViewV2<'_>,
    buffers: RuntimeSettlementBuffersV2<'_>,
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
        || buffers.cursor_scratch.len() != view.cursor_before.len()
        || buffers.cursor_output.len() != view.cursor_before.len()
        || buffers.inventory_scratch.len() != inventory_bytes
        || buffers.effect_scratch.len() != effect_bytes
        || buffers.effect_output.len() != effect_bytes
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
    copy_inventory(cursor, buffers.inventory_scratch)?;
    let effect_header = match view.action {
        RuntimeSettlementActionV2::Collect => collect(
            order.ok_or(RuntimeSettlementErrorV2::CoordinateMismatch)?,
            &mut successor,
            buffers.inventory_scratch,
            consumed_revision,
        )?,
        RuntimeSettlementActionV2::Materialize => materialize(
            view.verified,
            &mut successor,
            buffers.inventory_scratch,
            consumed_revision,
        )?,
        RuntimeSettlementActionV2::Distribute => distribute(
            order.ok_or(RuntimeSettlementErrorV2::CoordinateMismatch)?,
            &mut successor,
            buffers.inventory_scratch,
            consumed_revision,
        )?,
        RuntimeSettlementActionV2::Close => close(
            view.verified,
            &mut successor,
            buffers.inventory_scratch,
            view.surplus_beneficiary
                .ok_or(RuntimeSettlementErrorV2::CoordinateMismatch)?,
            consumed_revision,
        )?,
    };
    SettlementCursorV2::encode_le_inventory_into(
        successor,
        buffers.inventory_scratch,
        buffers.cursor_scratch,
    )
    .map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    encode_effect_plan(effect_header, order, view.verified, buffers.effect_scratch)?;
    RuntimeSettlementEffectPlanV2::decode(buffers.effect_scratch)?;
    buffers
        .cursor_output
        .copy_from_slice(buffers.cursor_scratch);
    buffers
        .effect_output
        .copy_from_slice(buffers.effect_scratch);
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
    cursor.phase = SettlementPhaseV2::Distributing;
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

fn close(
    verified: &[u8],
    cursor: &mut SettlementCursorHeaderV2,
    inventory: &[u8],
    beneficiary: [u8; 32],
    consumed_revision: u64,
) -> RuntimeSettlementResultV2<RuntimeSettlementEffectHeaderV2> {
    let terminal_coordinate = consumed_revision
        .checked_add(1)
        .ok_or(RuntimeSettlementErrorV2::ArithmeticOverflow)?;
    if cursor.phase != SettlementPhaseV2::ReadyToClose
        || cursor.next_order != cursor.order_count
        || zero_identity(&beneficiary)
        || inventory.iter().any(|byte| *byte != 0)
    {
        return Err(RuntimeSettlementErrorV2::InvalidPhase);
    }
    let balance =
        runtime_verified_balance_v2(verified).map_err(|_| RuntimeSettlementErrorV2::Codec)?;
    if cursor.quote_inventory != balance.quote_surplus {
        return Err(RuntimeSettlementErrorV2::Inventory);
    }
    let surplus = cursor.quote_inventory;
    cursor.quote_inventory = 0;
    cursor.terminal_coordinate = terminal_coordinate;
    cursor.phase = SettlementPhaseV2::Terminal;
    Ok(aggregate_effect(
        RuntimeSettlementActionV2::Close,
        cursor,
        RuntimeCompleteSetMoveV2::None,
        surplus,
        beneficiary,
        terminal_coordinate,
        consumed_revision,
    ))
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
            RuntimeSettlementActionV2::Close => 0,
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
mod tests {
    extern crate std;

    use super::*;
    use crate::runtime_candidate::{
        GENERAL_SETTLEMENT_BENEFICIARY_IDENTITY_V2, GENERAL_SETTLEMENT_COMMON_IDENTITIES_V2,
        GENERAL_SETTLEMENT_COMMON_SCALARS_V2, GENERAL_SETTLEMENT_ITEM_SCALAR_STRIDE_V2,
        GENERAL_SETTLEMENT_MOVE_SCALAR_V2, general_settlement_candidate_bank_len_v2,
        general_settlement_scalar_count_v2, project_general_settlement_candidate_v2,
    };
    use crate::runtime_manifest::settlement_manifest_len_v2;
    use crate::runtime_verify::{
        AuthenticatedOrderTermsV2, RuntimeConsiderRowBuffersV2, RuntimeConsiderRowViewV2,
        RuntimeManifestBuffersV2, evaluate_runtime_consider_row_with_manifest_v2,
        runtime_verifier_len_v2,
    };
    use crate::runtime_width::{
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

    fn row(
        width: u32,
        page_coordinate: u32,
        order_low: u8,
        lots: u64,
        receive: &[u64],
        deliver: &[u64],
        debit_limit: u64,
    ) -> (Vec<u8>, AuthenticatedOrderTermsV2) {
        let id = order_id(order_low);
        let terms = AuthenticatedOrderTermsV2 {
            order_id: id,
            owner_id: OWNER,
            nonce: u64::from(order_low),
            max_lots: 10,
            max_quote_debit_per_lot: debit_limit,
        };
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
            receive,
            deliver,
            &mut bytes,
        )
        .expect("execution");
        (bytes, terms)
    }

    fn terminal_fixture(width: u32) -> TerminalFixture {
        let count = usize::try_from(width).expect("test width");
        let ones = vec![1; count];
        let zeros = vec![0; count];
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
            },
            &ones,
            &mut candidate,
        )
        .expect("candidate");

        // Every page is intentionally unbalanced. Only the complete Candidate
        // has the uniform complete-set relation required for settlement.
        let rows = [
            row(width, 1, 1, 2, &ones, &zeros, 2),
            row(width, 2, 2, 1, &zeros, &ones, 0),
            row(width, 3, 3, 2, &ones, &zeros, 2),
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
        assert!((0..16).all(|outcome| collected.inventory(outcome).expect("inventory") == 1));

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
        let candidate = project_general_settlement_candidate_v2(
            &effect,
            16,
            &mut bank_scratch,
            &mut bank_output,
        )
        .expect("Strategy candidate");
        assert!(matches!(
            candidate,
            dclutch_execution_strategy_contract::v2::ExecutionCandidateV2::Accepted(_)
        ));
        let candidate_bytes = match candidate {
            dclutch_execution_strategy_contract::v2::ExecutionCandidateV2::Accepted(bytes) => bytes,
            dclutch_execution_strategy_contract::v2::ExecutionCandidateV2::Refused => &[],
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
            + usize::try_from(GENERAL_SETTLEMENT_BENEFICIARY_IDENTITY_V2)
                .expect("identity coordinate")
                * 32;
        assert_eq!(
            candidate_bytes
                .get(beneficiary_offset..beneficiary_offset + 32)
                .expect("beneficiary register"),
            [0_u8; 32]
        );
        assert_eq!(GENERAL_SETTLEMENT_ITEM_SCALAR_STRIDE_V2, 1);
        assert_eq!(GENERAL_SETTLEMENT_COMMON_IDENTITIES_V2, 4);
        cursor = next;
        let materialized = SettlementCursorV2::decode(&cursor).expect("materialized cursor");
        assert_eq!(materialized.header().quote_inventory, 1);
        assert!((0..16).all(|outcome| materialized.inventory(outcome).expect("inventory") == 4));

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
        assert!((0..16).all(|outcome| ready.inventory(outcome).expect("inventory") == 0));
        let terminal_coordinate = ready
            .header()
            .revision
            .checked_add(1)
            .expect("terminal successor revision");

        let (terminal_bytes, effect) =
            settle(&fixture, &cursor, RuntimeSettlementActionV2::Close, None, 0);
        let close_effect = RuntimeSettlementEffectPlanV2::decode(&effect).expect("close effect");
        assert!(close_effect.header().terminal);
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
            let adjusted =
                usize::try_from(isize::try_from(exact).expect("bank length fits") + delta)
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
}
