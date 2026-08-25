extern crate std;

use dclutch_transition_vm::v2::{
    ProgramV2, RegisterInput as VmInput, RegisterOutput as VmOutput, execute_atomic as execute_vm,
};

use super::*;

const SCALARS: usize = DIRECT_PROGRAM_V2_SCALARS as usize;
const IDENTITIES: usize = DIRECT_PROGRAM_V2_IDENTITIES as usize;

fn example() -> ([u64; SCALARS], [[u8; 32]; IDENTITIES]) {
    let mut scalars = [0_u64; SCALARS];
    scalars[SCALAR_PHASE] = OPEN_PHASE_V2;
    scalars[SCALAR_SLOT] = 100;
    scalars[SCALAR_SELLER_FROM] = 90;
    scalars[SCALAR_SELLER_THROUGH] = 110;
    scalars[SCALAR_BUYER_FROM] = 95;
    scalars[SCALAR_BUYER_THROUGH] = 120;
    scalars[SCALAR_SELLER_SIDE] = SELL_SIDE_V2;
    scalars[SCALAR_BUYER_SIDE] = BUY_SIDE_V2;
    scalars[SCALAR_SELLER_GENERATION] = 3;
    scalars[SCALAR_BUYER_GENERATION] = 3;
    scalars[SCALAR_SELLER_OUTCOME] = 1;
    scalars[SCALAR_BUYER_OUTCOME] = 1;
    scalars[SCALAR_OUTCOME_COUNT] = 2;
    scalars[SCALAR_SELLER_LIFECYCLE] = 0;
    scalars[SCALAR_SELLER_MAXIMUM] = 2_000;
    scalars[SCALAR_BUYER_LIFECYCLE] = 0;
    scalars[SCALAR_BUYER_MAXIMUM] = 2_000;
    scalars[SCALAR_SELLER_NONCE] = 0;
    scalars[SCALAR_BUYER_NONCE] = 0;
    scalars[SCALAR_SELLER_NEXT_NONCE] = 0;
    scalars[SCALAR_BUYER_NEXT_NONCE] = 0;
    scalars[SCALAR_SELLER_LIMIT] = 400_000;
    scalars[SCALAR_EXECUTION_PRICE] = 500_000;
    scalars[SCALAR_BUYER_LIMIT] = 600_000;
    scalars[SCALAR_PRICE_SCALE] = 1_000_000;
    scalars[SCALAR_SELLER_FEE_BPS] = 25;
    scalars[SCALAR_BUYER_FEE_BPS] = 25;
    scalars[SCALAR_POLICY_FEE_BPS] = 25;
    scalars[SCALAR_FILL] = 2_000;
    scalars[SCALAR_SELLER_CLAIMS] = 5_000;
    scalars[SCALAR_BUYER_CLAIMS] = 200;
    scalars[SCALAR_BUYER_COLLATERAL] = 2_000;
    scalars[SCALAR_SELLER_COLLATERAL] = 100;
    scalars[SCALAR_VENUE_COLLATERAL] = 20;

    let mut identities = [[0_u8; 32]; IDENTITIES];
    identities[IDENTITY_SELLER_MARKET] = [101; 32];
    identities[IDENTITY_BUYER_MARKET] = [101; 32];
    identities[IDENTITY_SELLER_MAKER] = [11; 32];
    identities[IDENTITY_BUYER_MAKER] = [12; 32];
    (scalars, identities)
}

fn compare(scalars: &[u64; SCALARS], identities: &[[u8; 32]; IDENTITIES]) {
    let mut aot_scratch = [0xa5_u64; SCALARS];
    let mut aot_scratch_ids = [[0xa5_u8; 32]; IDENTITIES];
    let mut aot_output = [0x5a_u64; SCALARS];
    let mut aot_output_ids = [[0x5a_u8; 32]; IDENTITIES];
    let aot = execute_atomic(
        RegisterInput {
            scalars,
            identities,
        },
        RegisterOutput {
            scalars: &mut aot_scratch,
            identities: &mut aot_scratch_ids,
        },
        RegisterOutput {
            scalars: &mut aot_output,
            identities: &mut aot_output_ids,
        },
    );

    let program = ProgramV2::decode(&DIRECT_PROGRAM_V2).expect("generated descriptor");
    let mut vm_scratch = [0xa5_u64; SCALARS];
    let mut vm_scratch_ids = [[0xa5_u8; 32]; IDENTITIES];
    let mut vm_output = [0x5a_u64; SCALARS];
    let mut vm_output_ids = [[0x5a_u8; 32]; IDENTITIES];
    let vm = execute_vm(
        program,
        VmInput {
            scalars,
            identities,
        },
        VmOutput {
            scalars: &mut vm_scratch,
            identities: &mut vm_scratch_ids,
        },
        VmOutput {
            scalars: &mut vm_output,
            identities: &mut vm_output_ids,
        },
    );

    assert_eq!(aot.is_ok(), vm.is_ok(), "acceptance/refusal divergence");
    assert_eq!(aot_output, vm_output, "scalar candidate divergence");
    assert_eq!(aot_output_ids, vm_output_ids, "identity divergence");
}

#[test]
fn generated_descriptor_is_exact_runtime_width_v2() {
    let program = ProgramV2::decode(&DIRECT_PROGRAM_V2).expect("generated descriptor");
    assert_eq!(DIRECT_PROGRAM_V2.len(), 856);
    assert_eq!(program.instruction_count(), 35);
    assert_eq!(program.scalar_count(), 41);
    assert_eq!(program.identity_count(), 4);
}

#[test]
fn formal_example_matches_interpreter_and_exact_outputs() {
    let (scalars, identities) = example();
    compare(&scalars, &identities);

    let mut scratch = [0_u64; SCALARS];
    let mut scratch_ids = [[0_u8; 32]; IDENTITIES];
    let mut output = [0_u64; SCALARS];
    let mut output_ids = [[0_u8; 32]; IDENTITIES];
    execute_atomic(
        RegisterInput {
            scalars: &scalars,
            identities: &identities,
        },
        RegisterOutput {
            scalars: &mut scratch,
            identities: &mut scratch_ids,
        },
        RegisterOutput {
            scalars: &mut output,
            identities: &mut output_ids,
        },
    )
    .expect("accepted example");
    assert_eq!(output[SCALAR_SELLER_NONCE_OUTPUT], 1);
    assert_eq!(output[SCALAR_BUYER_NONCE_OUTPUT], 1);
    assert_eq!(output[SCALAR_GROSS_OUTPUT], 1_000);
    assert_eq!(output[SCALAR_FEE_OUTPUT], 2);
    assert_eq!(output_ids, identities);
}

#[test]
fn hostile_admission_substitutions_match_interpreter() {
    let (base, base_identities) = example();
    let hostile_scalars = [
        (SCALAR_PHASE, 2),
        (SCALAR_FILL, 0),
        (SCALAR_SELLER_FROM, 101),
        (SCALAR_BUYER_THROUGH, 99),
        (SCALAR_SELLER_SIDE, 1),
        (SCALAR_BUYER_SIDE, 0),
        (SCALAR_BUYER_GENERATION, 4),
        (SCALAR_BUYER_OUTCOME, 0),
        (SCALAR_OUTCOME_COUNT, 1),
        (SCALAR_SELLER_LIFECYCLE, 3),
        (SCALAR_BUYER_MAXIMUM, 1_999),
        (SCALAR_SELLER_NONCE, 1),
        (SCALAR_EXECUTION_PRICE, 399_999),
        (SCALAR_BUYER_LIMIT, 499_999),
        (SCALAR_SELLER_FEE_BPS, 24),
        (SCALAR_POLICY_FEE_BPS, 10_001),
        (SCALAR_PRICE_SCALE, 999_999),
        (SCALAR_SELLER_CLAIMS, 1_999),
        (SCALAR_BUYER_COLLATERAL, 1_001),
    ];
    for (index, value) in hostile_scalars {
        let mut scalars = base;
        scalars[index] = value;
        compare(&scalars, &base_identities);
    }

    let mut wrong_market = base_identities;
    wrong_market[IDENTITY_BUYER_MARKET] = [102; 32];
    compare(&base, &wrong_market);
    let mut alias_maker = base_identities;
    alias_maker[IDENTITY_BUYER_MAKER] = alias_maker[IDENTITY_SELLER_MAKER];
    compare(&base, &alias_maker);
}

#[test]
fn deterministic_valid_corpus_is_output_equivalent() {
    let (base, identities) = example();
    for seed in 1_u64..=128 {
        let mut scalars = base;
        let fill = seed * 2;
        let lifecycle = seed % 3;
        scalars[SCALAR_FILL] = fill;
        scalars[SCALAR_SELLER_LIFECYCLE] = lifecycle;
        scalars[SCALAR_BUYER_LIFECYCLE] = lifecycle;
        scalars[SCALAR_SELLER_MAXIMUM] = if lifecycle == 0 { fill } else { fill + seed };
        scalars[SCALAR_BUYER_MAXIMUM] = if lifecycle == 0 { fill } else { fill + seed };
        scalars[SCALAR_SELLER_NONCE] = seed;
        scalars[SCALAR_BUYER_NONCE] = seed + 1;
        scalars[SCALAR_SELLER_NEXT_NONCE] = seed;
        scalars[SCALAR_BUYER_NEXT_NONCE] = seed + 1;
        scalars[SCALAR_SELLER_CLAIMS] = fill;
        scalars[SCALAR_BUYER_COLLATERAL] = fill;
        scalars[SCALAR_OUTCOME_COUNT] = 2 + seed;
        compare(&scalars, &identities);
    }
}

#[test]
fn late_refusal_is_atomic_and_equivalent() {
    let (mut scalars, identities) = example();
    scalars[SCALAR_VENUE_COLLATERAL] = u64::MAX;
    let output_before = [0x5a_u64; SCALARS];
    let output_ids_before = [[0x5a_u8; 32]; IDENTITIES];
    let mut scratch = [0xa5_u64; SCALARS];
    let mut scratch_ids = [[0xa5_u8; 32]; IDENTITIES];
    let mut output = output_before;
    let mut output_ids = output_ids_before;
    assert_eq!(
        execute_atomic(
            RegisterInput {
                scalars: &scalars,
                identities: &identities,
            },
            RegisterOutput {
                scalars: &mut scratch,
                identities: &mut scratch_ids,
            },
            RegisterOutput {
                scalars: &mut output,
                identities: &mut output_ids,
            },
        ),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(output, output_before);
    assert_eq!(output_ids, output_ids_before);
    compare(&scalars, &identities);
}

#[test]
fn product_width_is_runtime_data_not_a_family_limit() {
    let (base, identities) = example();
    for outcome_count in [2, 16, 65_535, u64::MAX] {
        let mut scalars = base;
        scalars[SCALAR_OUTCOME_COUNT] = outcome_count;
        compare(&scalars, &identities);
    }
}

#[test]
fn wrong_physical_bank_width_refuses_without_output_mutation() {
    let (scalars, identities) = example();
    let mut scratch = [0_u64; SCALARS - 1];
    let mut scratch_ids = [[0_u8; 32]; IDENTITIES];
    let mut output = [9_u64; SCALARS];
    let mut output_ids = [[9_u8; 32]; IDENTITIES];
    let before = output;
    let before_ids = output_ids;
    assert_eq!(
        execute_atomic(
            RegisterInput {
                scalars: &scalars,
                identities: &identities,
            },
            RegisterOutput {
                scalars: &mut scratch,
                identities: &mut scratch_ids,
            },
            RegisterOutput {
                scalars: &mut output,
                identities: &mut output_ids,
            },
        ),
        Err(Error::RegisterWidthMismatch)
    );
    assert_eq!(output, before);
    assert_eq!(output_ids, before_ids);
}
