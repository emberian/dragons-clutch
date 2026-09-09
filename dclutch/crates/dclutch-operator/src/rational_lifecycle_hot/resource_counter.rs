//! The Structured root's resource obligation is updated in the same atomic
//! effect as its native Claims lifecycle child. No caller count is projected.

use super::{Error, Result};
use dclutch_claims::rational_lifecycle::LifecycleActionV2;
use dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1;
use dclutch_trading::structured_root_v2::*;
use dclutch_vm::account_profile::v2::encode::{
    AccountCoordinateV2, AccountEffectPermissionsV2, AccountOperationInputV2, AccountRuleInputV2,
    IdentityCoordinateV2, ScalarCoordinateV2,
};
use dclutch_vm::effect::v3::encode::{
    AccountCoordinateV3, EffectInstructionV3, ScalarCoordinateV3,
};
use dclutch_vm::v3::{InstructionV3, ScalarRegisterV3};

// Appended registers: observed count, observed magic, observed header,
// transition scratch, successor count. Width follows the u64 native owner.
const EXTRA: usize = 5;

pub(super) fn enabled(schema: [u8; 32], bytes: u32) -> Result<bool> {
    if schema != STRUCTURED_CAPABILITY_ROOT_SCHEMA_ID_V2 {
        return Ok(false);
    }
    if usize::try_from(bytes).ok() != Some(STRUCTURED_CAPABILITY_ROOT_BYTES_V2) {
        return Err(Error::ArtifactGeometry);
    }
    Ok(true)
}

pub(super) fn scalar_count(base: usize, enabled: bool) -> Result<usize> {
    base.checked_add(if enabled { EXTRA } else { 0 })
        .ok_or(Error::InvalidLength)
}

pub(super) fn identity_count(base: usize, enabled: bool) -> Result<usize> {
    base.checked_add(usize::from(enabled))
        .ok_or(Error::InvalidLength)
}

pub(super) fn trusted_identity(
    base: usize,
    enabled: bool,
) -> Result<dclutch_vm::account_profile::v2::TrustedIdentityEnvironmentV2> {
    use dclutch_vm::account_profile::v2::TrustedIdentityEnvironmentV2;
    Ok(if enabled {
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: n(base)?,
        }
    } else {
        TrustedIdentityEnvironmentV2::None
    })
}

pub(super) fn root_rule(rule: &mut AccountRuleInputV2, enabled: bool) -> Result<()> {
    if enabled {
        if usize::try_from(rule.data_length).ok()
            != Some(CAPABILITY_ROOT_HEADER_BYTES_V1 + STRUCTURED_CAPABILITY_ROOT_BYTES_V2)
        {
            return Err(Error::AccountObservation);
        }
        rule.effect_permissions = AccountEffectPermissionsV2::new(false, false, true);
    }
    Ok(())
}

fn n(value: usize) -> Result<u16> {
    u16::try_from(value).map_err(|_| Error::InvalidLength)
}
fn offset(tail: usize) -> Result<u32> {
    u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1 + tail).map_err(|_| Error::InvalidLength)
}

pub(super) fn project(
    ops: &mut Vec<AccountOperationInputV2>,
    base: usize,
    base_identities: usize,
    enabled: bool,
) -> Result<()> {
    if enabled {
        ops.push(AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(0),
            expected: IdentityCoordinateV2::common(n(base_identities)?),
        });
        for (index, tail) in [STRUCTURED_RESOURCE_GROUP_COUNT_OFFSET_V2, 0, 8]
            .into_iter()
            .enumerate()
        {
            ops.push(AccountOperationInputV2::ProjectDataU64 {
                account: AccountCoordinateV2::fixed(0),
                destination: ScalarCoordinateV2::common(n(base + index)?),
                data_offset: offset(tail)?,
            });
        }
    }
    Ok(())
}

pub(super) fn transition(
    ops: &mut Vec<InstructionV3>,
    action: LifecycleActionV2,
    base: usize,
    enabled: bool,
) -> Result<()> {
    if !enabled {
        return Ok(());
    }
    let reg = |i| n(base + i).map(ScalarRegisterV3::common);
    for (i, expected) in [
        (1, STRUCTURED_CAPABILITY_ROOT_MAGIC_WORD_V2),
        (2, STRUCTURED_CAPABILITY_ROOT_HEADER_WORD_V2),
    ] {
        ops.push(InstructionV3::load_const(reg(3)?, expected));
        ops.push(InstructionV3::scalar_eq(reg(i)?, reg(3)?));
    }
    if action.retires() {
        ops.push(InstructionV3::load_const(reg(3)?, 1));
        ops.push(InstructionV3::sub_into(reg(0)?, reg(3)?, reg(4)?));
    } else {
        ops.push(InstructionV3::increment_into(reg(0)?, reg(4)?));
    }
    Ok(())
}

pub(super) fn effect(ops: &mut Vec<EffectInstructionV3>, base: usize, enabled: bool) -> Result<()> {
    if enabled {
        ops.push(EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(0),
            offset(STRUCTURED_RESOURCE_GROUP_COUNT_OFFSET_V2)?,
            ScalarCoordinateV3::common(n(base + 4)?),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_vm::v3::{self as vm, ProgramGeometryV3, RegisterInput, RegisterOutput};

    fn run(action: LifecycleActionV2, bytes: &[u8], output: &mut [u64; EXTRA]) -> vm::Result<()> {
        let mut ops = Vec::new();
        transition(&mut ops, action, 0, true).expect("counter instructions");
        let mut code = vec![0; vm::HEADER_BYTES + ops.len() * vm::INSTRUCTION_BYTES];
        let mut scratch_code = code.clone();
        vm::encode_program_atomic(
            ProgramGeometryV3 {
                common_scalars: u16::try_from(EXTRA).expect("width"),
                item_scalar_stride: 0,
                common_identities: 1,
                item_identity_stride: 0,
            },
            &ops,
            &[],
            &[],
            &mut scratch_code,
            &mut code,
        )
        .expect("encode");
        let word =
            |offset: usize| u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("word"));
        let scalars = [word(16), word(0), word(8), 0, 0];
        let mut scratch = [0; EXTRA];
        let mut scratch_ids = [[0; 32]];
        let mut output_ids = [[0; 32]];
        vm::execute_fold_atomic(
            vm::ProgramV3::decode(&code).expect("decode"),
            0,
            RegisterInput {
                scalars: &scalars,
                identities: &[[0; 32]],
            },
            RegisterOutput {
                scalars: &mut scratch,
                identities: &mut scratch_ids,
            },
            RegisterOutput {
                scalars: output,
                identities: &mut output_ids,
            },
        )
    }

    #[test]
    fn structured_counter_spans_receipts_and_coordinates_and_matches_native_successor() {
        let mut root = StructuredCapabilityRootV2::default();
        // Distinct descriptors may have outstanding groups simultaneously.
        for action in [
            LifecycleActionV2::ActivateReceipt,
            LifecycleActionV2::ActivateReceipt,
            LifecycleActionV2::ActivateCoordinate,
            LifecycleActionV2::RetireReceipt,
            LifecycleActionV2::RetireCoordinate,
            LifecycleActionV2::RetireReceipt,
        ] {
            let mut output = [u64::MAX; EXTRA];
            run(action, &root.encode(), &mut output).expect("VM transition");
            root = if action.retires() {
                root.retire()
            } else {
                root.activate()
            }
            .expect("native transition");
            assert_eq!(output[4], root.outstanding());
            assert_eq!(
                root.require_closeable(),
                if root.outstanding() == 0 {
                    Ok(())
                } else {
                    Err(StructuredRootErrorV2::OutstandingResources)
                }
            );
        }
        assert_eq!(root.require_closeable(), Ok(()));
    }

    #[test]
    fn structured_counter_refuses_underflow_overflow_and_header_substitution_atomically() {
        for (action, bytes, error) in [
            (
                LifecycleActionV2::RetireReceipt,
                STRUCTURED_CAPABILITY_ROOT_TAIL_V2,
                vm::Error::CheckFailed,
            ),
            (
                LifecycleActionV2::ActivateCoordinate,
                {
                    let mut b = STRUCTURED_CAPABILITY_ROOT_TAIL_V2;
                    b[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
                    b
                },
                vm::Error::ArithmeticOverflow,
            ),
            (
                LifecycleActionV2::ActivateReceipt,
                {
                    let mut b = STRUCTURED_CAPABILITY_ROOT_TAIL_V2;
                    b[0] ^= 1;
                    b
                },
                vm::Error::CheckFailed,
            ),
            (
                LifecycleActionV2::ActivateReceipt,
                {
                    let mut b = STRUCTURED_CAPABILITY_ROOT_TAIL_V2;
                    b[15] = 1;
                    b
                },
                vm::Error::CheckFailed,
            ),
        ] {
            let mut output = [42; EXTRA];
            assert_eq!(run(action, &bytes, &mut output), Err(error));
            assert_eq!(output, [42; EXTRA]);
        }
    }
}
