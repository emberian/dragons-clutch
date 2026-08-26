use dclutch_direct_codec::registered_fill_artifacts_v4::*;
use dclutch_transition_vm::v3::{
    ProgramV3, RegisterInput as VmInput, RegisterOutput as VmOutput, execute_fold_atomic,
};

use super::*;

const N: u32 = 3;

fn valid_scalars() -> [u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4] {
    let mut scalars = [0_u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4];
    for (index, value) in [
        (FILL_SCALAR_SLOT_V4, 100),
        (FILL_SCALAR_OUTCOME_COUNT_V4, u64::from(N)),
        (FILL_SCALAR_MARKET_GENERATION_V4, 7),
        (FILL_SCALAR_PRICE_SCALE_V4, 100),
        (FILL_SCALAR_POLICY_FEE_BPS_V4, 100),
        (FILL_SCALAR_QUANTITY_V4, 10),
        (FILL_SCALAR_EXECUTION_PRICE_V4, 50),
        (FILL_SCALAR_ROOT_OPEN_COUNT_V4, 2),
        (FILL_SCALAR_SELLER_SIDE_V4, 0),
        (FILL_SCALAR_SELLER_LIFECYCLE_V4, 2),
        (FILL_SCALAR_SELLER_OUTCOME_V4, 1),
        (FILL_SCALAR_SELLER_GENERATION_V4, 7),
        (FILL_SCALAR_SELLER_NONCE_V4, 0),
        (FILL_SCALAR_SELLER_VALID_FROM_V4, 90),
        (FILL_SCALAR_SELLER_VALID_THROUGH_V4, 110),
        (FILL_SCALAR_SELLER_MAXIMUM_V4, 20),
        (FILL_SCALAR_SELLER_LIMIT_V4, 40),
        (FILL_SCALAR_SELLER_FEE_BPS_V4, 100),
        (FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4, 20),
        (FILL_SCALAR_SELLER_NEXT_NONCE_V4, 1),
        (FILL_SCALAR_SELLER_LIVE_COUNT_V4, 1),
        (FILL_SCALAR_SELLER_MAKER_GENERATION_V4, 7),
        (FILL_SCALAR_BUYER_SIDE_V4, 1),
        (FILL_SCALAR_BUYER_LIFECYCLE_V4, 2),
        (FILL_SCALAR_BUYER_OUTCOME_V4, 1),
        (FILL_SCALAR_BUYER_GENERATION_V4, 7),
        (FILL_SCALAR_BUYER_NONCE_V4, 0),
        (FILL_SCALAR_BUYER_VALID_FROM_V4, 90),
        (FILL_SCALAR_BUYER_VALID_THROUGH_V4, 110),
        (FILL_SCALAR_BUYER_MAXIMUM_V4, 20),
        (FILL_SCALAR_BUYER_LIMIT_V4, 60),
        (FILL_SCALAR_BUYER_FEE_BPS_V4, 100),
        (FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4, 12),
        (FILL_SCALAR_BUYER_NEXT_NONCE_V4, 1),
        (FILL_SCALAR_BUYER_LIVE_COUNT_V4, 1),
        (FILL_SCALAR_BUYER_MAKER_GENERATION_V4, 7),
        (FILL_SCALAR_CLAIM_SOURCE_REVISION_V4, 4),
        (FILL_SCALAR_CLAIM_DESTINATION_REVISION_V4, 9),
        (FILL_SCALAR_CUSTODY_REVISION_V4, 3),
        (FILL_SCALAR_SELLER_MAKER_RENT_PRINCIPAL_V4, 1),
        (FILL_SCALAR_SELLER_RECORD_RENT_PRINCIPAL_V4, 1),
        (FILL_SCALAR_BUYER_MAKER_RENT_PRINCIPAL_V4, 1),
        (FILL_SCALAR_BUYER_RECORD_RENT_PRINCIPAL_V4, 1),
    ] {
        *scalars.get_mut(index).expect("scalar") = value;
    }
    scalars
}

fn valid_identities() -> [[u8; 32]; DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4] {
    let mut identities = [[1_u8; 32]; DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4];
    for index in [
        FILL_IDENTITY_MARKET_V4,
        FILL_IDENTITY_SELLER_INTENT_MARKET_V4,
        FILL_IDENTITY_BUYER_INTENT_MARKET_V4,
        FILL_IDENTITY_SELLER_MAKER_MARKET_V4,
        FILL_IDENTITY_BUYER_MAKER_MARKET_V4,
    ] {
        *identities.get_mut(index).expect("Market identity") = [2; 32];
    }
    identities[FILL_IDENTITY_SELLER_MAKER_V4] = [3; 32];
    identities[FILL_IDENTITY_SELLER_MAKER_REPLAY_OWNER_V4] = [3; 32];
    identities[FILL_IDENTITY_BUYER_MAKER_V4] = [4; 32];
    identities[FILL_IDENTITY_BUYER_MAKER_REPLAY_OWNER_V4] = [4; 32];
    identities
}

fn transition() -> [u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4] {
    let mut scratch = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
    let mut output = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
    encode_direct_registered_fill_transition_v4_atomic(&mut scratch, &mut output)
        .expect("transition");
    output
}

fn assert_equivalent(
    scalars: &[u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4],
    identities: &[[u8; 32]; DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4],
) {
    let transition_bytes = transition();
    let program = ProgramV3::decode(&transition_bytes).expect("decode transition");
    let mut aot_scratch_scalars = *scalars;
    let mut aot_scratch_identities = *identities;
    let mut aot_scalars = [0x55_u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4];
    let mut aot_identities = [[0x55_u8; 32]; DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4];
    let aot_before = aot_scalars;
    let aot_identities_before = aot_identities;
    let aot = execute_registered_ordinary_fill_atomic(
        N,
        RegisterInput {
            scalars,
            identities,
        },
        RegisterOutput {
            scalars: &mut aot_scratch_scalars,
            identities: &mut aot_scratch_identities,
        },
        RegisterOutput {
            scalars: &mut aot_scalars,
            identities: &mut aot_identities,
        },
    );

    let mut vm_scratch_scalars = *scalars;
    let mut vm_scratch_identities = *identities;
    let mut vm_scalars = [0x55_u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4];
    let mut vm_identities = [[0x55_u8; 32]; DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4];
    let vm = execute_fold_atomic(
        program,
        N,
        VmInput {
            scalars,
            identities,
        },
        VmOutput {
            scalars: &mut vm_scratch_scalars,
            identities: &mut vm_scratch_identities,
        },
        VmOutput {
            scalars: &mut vm_scalars,
            identities: &mut vm_identities,
        },
    );
    assert_eq!(aot.is_ok(), vm.is_ok());
    if aot.is_ok() {
        assert_eq!(aot_scalars, vm_scalars);
        assert_eq!(aot_identities, vm_identities);
    } else {
        assert_eq!(aot_scalars, aot_before);
        assert_eq!(aot_identities, aot_identities_before);
    }
}

#[test]
fn accepted_partial_and_terminal_banks_are_exact() {
    let identities = valid_identities();
    let partial = valid_scalars();
    assert_equivalent(&partial, &identities);

    let mut terminal = valid_scalars();
    terminal[FILL_SCALAR_QUANTITY_V4] = 20;
    assert_equivalent(&terminal, &identities);
}

#[test]
fn hostile_corpus_matches_refusal_and_preserves_output() {
    for mutation in 0..8 {
        let mut scalars = valid_scalars();
        let mut identities = valid_identities();
        match mutation {
            0 => scalars[FILL_SCALAR_QUANTITY_V4] = 0,
            1 => scalars[FILL_SCALAR_EXECUTION_PRICE_V4] = 51,
            2 => identities[FILL_IDENTITY_BUYER_INTENT_MARKET_V4] = [9; 32],
            3 => scalars[FILL_SCALAR_BUYER_LIFECYCLE_V4] = 1,
            4 => scalars[FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4] = 11,
            5 => scalars[FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4] = 19,
            6 => scalars[FILL_SCALAR_CLAIM_SOURCE_REVISION_V4] = u64::MAX,
            _ => scalars[FILL_SCALAR_BUYER_VALID_THROUGH_V4] = 99,
        }
        assert_equivalent(&scalars, &identities);
    }
}
