//! Action-selected TransitionVM programs for General V3.
//!
//! These programs are the data-defined semantic gate paired with the admitted
//! read-only accelerator. They derive canonical action, local-state, cursor,
//! replay, and child-request coordinates in the same register bank consumed by
//! the common EffectProgram. The Product-owned tail is folded at runtime; no
//! outcome width is compiled into an artifact.

use dclutch_general_codec::Action;
use dclutch_transition_vm::v3::{
    HEADER_BYTES, INSTRUCTION_BYTES, IdentityRegisterV3, InstructionV3, ProgramGeometryV3,
    ProgramV3, ScalarRegisterV3, encode_program_atomic,
};

use crate::{
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
}

/// Result alias for General transition artifacts.
pub type Result<T> = core::result::Result<T, GeneralTransitionArtifactErrorV3>;

/// Harmless initializer for caller-owned instruction workspaces.
pub const GENERAL_TRANSITION_INSTRUCTION_PLACEHOLDER_V3: InstructionV3 =
    InstructionV3::load_const(ScalarRegisterV3::common(0), 0);

/// Return exact `(prelude, item, epilogue)` instruction counts.
#[must_use]
pub const fn general_transition_instruction_count_v3(action: Action) -> (usize, usize, usize) {
    match action {
        Action::Consider => (12, 1, 0),
        Action::Freeze => (14, 1, 0),
        Action::InitializeSettlement => (18, 2, 0),
        Action::Collect | Action::Distribute => (16, 4, 0),
        Action::Materialize => (13, 1, 0),
        Action::Close => (21, 6, 0),
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
    let kind = if matches!(action, Action::Consider | Action::Freeze) {
        GeneralLocalStateKindV3::Selection
    } else {
        GeneralLocalStateKindV3::Settlement
    };
    for instruction in [
        InstructionV3::load_const(s(scalar::ACTION)?, u64::from(action as u8)),
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
        Action::Collect | Action::Distribute => append_row_action(output, cursor)?,
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
        }
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
        Action::Consider | Action::Freeze | Action::Materialize => {}
        Action::InitializeSettlement => push(
            output,
            cursor,
            InstructionV3::load_const(is(item_scalar::CURSOR_INVENTORY)?, 0),
        )?,
        Action::Collect | Action::Distribute => {
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

    const ACTIONS: [Action; 7] = [
        Action::Consider,
        Action::Freeze,
        Action::InitializeSettlement,
        Action::Collect,
        Action::Materialize,
        Action::Distribute,
        Action::Close,
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

    #[test]
    fn consider_fold_accepts_product_widths_one_and_258() {
        let bytes = artifact(Action::Consider);
        let program = ProgramV3::decode(&bytes).expect("decode");
        for count in [1_u32, 258] {
            let scalar_count = usize::try_from(
                GENERAL_HOT_COMMON_SCALARS_V3 + count * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3,
            )
            .expect("scalar width");
            let mut input_scalars = vec![0_u64; scalar_count];
            input_scalars[usize::try_from(scalar::OUTCOME_COUNT).expect("outcome register")] =
                u64::from(count);
            input_scalars[usize::try_from(scalar::STATE_BUMP).expect("bump")] = 7;
            input_scalars
                [usize::try_from(scalar::PRIMARY_CANONICAL_BUMP).expect("canonical bump")] = 7;
            input_scalars
                [usize::try_from(scalar::PRIMARY_RENT_PRINCIPAL).expect("rent principal")] = 1;
            input_scalars[usize::try_from(scalar::SELECTION_REVISION).expect("revision")] = 1;
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
            input_identities[usize::try_from(identity::TRADING_PROGRAM).expect("program")] =
                [9; 32];
            let mut scalar_scratch = vec![0_u64; scalar_count];
            let mut scalar_output = vec![0_u64; scalar_count];
            let mut identity_scratch = vec![[0_u8; 32]; input_identities.len()];
            let mut identity_output = vec![[0_u8; 32]; input_identities.len()];
            execute_fold_atomic(
                program,
                count,
                RegisterInput {
                    scalars: &input_scalars,
                    identities: &input_identities,
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
            .expect("runtime-width fold");
            assert_eq!(
                scalar_output[usize::try_from(scalar::ACTION).expect("action")],
                u64::from(Action::Consider as u8)
            );
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
