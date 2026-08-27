use dclutch_direct_codec::ordinary_v3::*;
use dclutch_transition_vm::v3::{
    ProgramV3, RegisterInput as VmInput, RegisterOutput as VmOutput, execute_fold_atomic,
};

use super::*;

const N: u32 = 3;
const SCALARS: usize =
    DIRECT_ORDINARY_COMMON_SCALARS_V3 + 3 * (DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3 as usize);

fn input() -> (
    [u64; SCALARS],
    [[u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3],
) {
    let mut scalars = [0_u64; SCALARS];
    for (index, value) in [
        (SCALAR_SLOT_V3, 100),
        (SCALAR_SELLER_VALID_FROM_V3, 90),
        (SCALAR_SELLER_VALID_THROUGH_V3, 110),
        (SCALAR_BUYER_VALID_FROM_V3, 90),
        (SCALAR_BUYER_VALID_THROUGH_V3, 110),
        (SCALAR_BUYER_SIDE_V3, 1),
        (SCALAR_SELLER_GENERATION_V3, 7),
        (SCALAR_BUYER_GENERATION_V3, 7),
        (SCALAR_MARKET_GENERATION_V3, 7),
        (SCALAR_SELLER_OUTCOME_V3, 1),
        (SCALAR_BUYER_OUTCOME_V3, 1),
        (SCALAR_OUTCOME_COUNT_V3, u64::from(N)),
        (SCALAR_SELLER_LIFECYCLE_V3, 1),
        (SCALAR_SELLER_MAXIMUM_V3, 10),
        (SCALAR_BUYER_LIFECYCLE_V3, 1),
        (SCALAR_BUYER_MAXIMUM_V3, 10),
        (SCALAR_SELLER_NONCE_V3, 1),
        (SCALAR_BUYER_NONCE_V3, 2),
        (SCALAR_SELLER_NEXT_NONCE_V3, 1),
        (SCALAR_BUYER_NEXT_NONCE_V3, 2),
        (SCALAR_SELLER_LIMIT_V3, 40),
        (SCALAR_EXECUTION_PRICE_V3, 50),
        (SCALAR_BUYER_LIMIT_V3, 60),
        (SCALAR_PRICE_SCALE_V3, 100),
        (SCALAR_SELLER_FEE_BPS_V3, 100),
        (SCALAR_BUYER_FEE_BPS_V3, 100),
        (SCALAR_POLICY_FEE_BPS_V3, 100),
        (SCALAR_FILL_V3, 10),
        (SCALAR_CUSTODY_REVISION_V3, 3),
        (SCALAR_ROOT_OPEN_COUNT_V3, 2),
    ] {
        *scalars.get_mut(index).expect("scalar") = value;
    }
    for item in 0..usize::try_from(N).expect("N") {
        let base = DIRECT_ORDINARY_COMMON_SCALARS_V3
            + item * usize::from(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3);
        *scalars.get_mut(base).expect("item index") = u64::try_from(item).expect("item");
    }
    let mut identities = [[1_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
    identities[IDENTITY_MARKET_V3] = [2; 32];
    identities[IDENTITY_SELLER_INTENT_MARKET_V3] = [2; 32];
    identities[IDENTITY_BUYER_INTENT_MARKET_V3] = [2; 32];
    identities[IDENTITY_SELLER_NATIVE_SIGNER_V3] = [3; 32];
    identities[IDENTITY_SELLER_REQUEST_MAKER_V3] = [3; 32];
    identities[IDENTITY_BUYER_NATIVE_SIGNER_V3] = [4; 32];
    identities[IDENTITY_BUYER_REQUEST_MAKER_V3] = [4; 32];
    identities[IDENTITY_SELLER_COLLATERAL_REQUEST_V3] = [5; 32];
    identities[IDENTITY_SELLER_TOKEN_ACCOUNT_V3] = [5; 32];
    identities[IDENTITY_BUYER_COLLATERAL_REQUEST_V3] = [6; 32];
    identities[IDENTITY_BUYER_TOKEN_ACCOUNT_V3] = [6; 32];
    identities[IDENTITY_TRADING_PROGRAM_V3] = [7; 32];
    identities[IDENTITY_SELLER_STATE_OWNER_V3] = [7; 32];
    identities[IDENTITY_BUYER_STATE_OWNER_V3] = [7; 32];
    (scalars, identities)
}

fn transition() -> [u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3] {
    let mut scratch = [0_u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3];
    let mut output = [0_u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3];
    dclutch_direct_codec::ordinary_v3::encode_direct_ordinary_transition_v3(
        &mut scratch,
        &mut output,
    )
    .expect("transition");
    output
}

#[test]
fn accepted_bank_is_byte_exact_with_transition_vm() {
    let (input_scalars, input_identities) = input();
    let mut aot_scratch_scalars = input_scalars;
    let mut aot_scratch_identities = input_identities;
    let mut aot_scalars = input_scalars;
    let mut aot_identities = input_identities;
    execute_inline_ordinary_atomic(
        N,
        RegisterInput {
            scalars: &input_scalars,
            identities: &input_identities,
        },
        RegisterOutput {
            scalars: &mut aot_scratch_scalars,
            identities: &mut aot_scratch_identities,
        },
        RegisterOutput {
            scalars: &mut aot_scalars,
            identities: &mut aot_identities,
        },
    )
    .expect("AOT");

    let transition_bytes = transition();
    let program = ProgramV3::decode(&transition_bytes).expect("program");
    let mut vm_scratch_scalars = input_scalars;
    let mut vm_scratch_identities = input_identities;
    let mut vm_scalars = input_scalars;
    let mut vm_identities = input_identities;
    execute_fold_atomic(
        program,
        N,
        VmInput {
            scalars: &input_scalars,
            identities: &input_identities,
        },
        VmOutput {
            scalars: &mut vm_scratch_scalars,
            identities: &mut vm_scratch_identities,
        },
        VmOutput {
            scalars: &mut vm_scalars,
            identities: &mut vm_identities,
        },
    )
    .expect("VM");
    assert_eq!(aot_scalars, vm_scalars);
    assert_eq!(aot_identities, vm_identities);
}

#[test]
fn hostile_corpus_refuses_in_both_and_preserves_outputs() {
    let transition_bytes = transition();
    let program = ProgramV3::decode(&transition_bytes).expect("program");
    for mutation in 0..4 {
        let (mut input_scalars, mut input_identities) = input();
        match mutation {
            0 => input_scalars[SCALAR_FILL_V3] = 0,
            1 => input_scalars[SCALAR_EXECUTION_PRICE_V3] = 51,
            2 => input_identities[IDENTITY_BUYER_INTENT_MARKET_V3] = [9; 32],
            _ => input_scalars[SCALAR_SELLER_LIFECYCLE_V3] = 9,
        }
        let mut aot_scratch_scalars = input_scalars;
        let mut aot_scratch_identities = input_identities;
        let mut aot_scalars = [0x55_u64; SCALARS];
        let mut aot_identities = [[0x55_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
        let aot_scalars_before = aot_scalars;
        let aot_identities_before = aot_identities;
        assert!(
            execute_inline_ordinary_atomic(
                N,
                RegisterInput {
                    scalars: &input_scalars,
                    identities: &input_identities,
                },
                RegisterOutput {
                    scalars: &mut aot_scratch_scalars,
                    identities: &mut aot_scratch_identities,
                },
                RegisterOutput {
                    scalars: &mut aot_scalars,
                    identities: &mut aot_identities,
                },
            )
            .is_err()
        );
        assert_eq!(aot_scalars, aot_scalars_before);
        assert_eq!(aot_identities, aot_identities_before);

        let mut vm_scratch_scalars = input_scalars;
        let mut vm_scratch_identities = input_identities;
        let mut vm_scalars = [0x55_u64; SCALARS];
        let mut vm_identities = [[0x55_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
        assert!(
            execute_fold_atomic(
                program,
                N,
                VmInput {
                    scalars: &input_scalars,
                    identities: &input_identities,
                },
                VmOutput {
                    scalars: &mut vm_scratch_scalars,
                    identities: &mut vm_scratch_identities,
                },
                VmOutput {
                    scalars: &mut vm_scalars,
                    identities: &mut vm_identities,
                },
            )
            .is_err()
        );
    }
}
