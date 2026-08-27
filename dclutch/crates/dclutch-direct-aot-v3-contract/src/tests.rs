//! Register-bank differential for the inline ordinary transition.
//!
//! Two Rust executors run the same Lean-emitted TransitionVMV3 program bytes:
//! the hand-written AOT translation in `lib.rs` and the generic interpreter in
//! `dclutch-transition-vm`. Every corpus case compares both the scratch and the
//! output banks of the two executors. These are case tests over a fixed corpus;
//! they say nothing about inputs outside it.

use dclutch_direct_codec::ordinary_v3::*;
use dclutch_transition_vm::v3::{
    ProgramV3, RegisterInput as VmInput, RegisterOutput as VmOutput, execute_fold_atomic,
};

use super::*;

const N: u32 = 3;
const SCALARS: usize =
    DIRECT_ORDINARY_COMMON_SCALARS_V3 + 3 * (DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3 as usize);

/// Output-bank fill distinct from zero and from every value either executor
/// computes, so an untouched refusal bank is distinguishable from a written one.
const SENTINEL_SCALAR: u64 = 0x55;
/// Identity-bank counterpart of `SENTINEL_SCALAR`.
const SENTINEL_IDENTITY: [u8; 32] = [0x55; 32];
/// Identity value that collides with no baseline identity register.
const FOREIGN: [u8; 32] = [9; 32];
/// Exact fee denominator the emitted program loads as a constant.
const FEE_DENOMINATOR: u64 = dclutch_direct_codec::successor::DIRECT_FEE_DENOMINATOR_V1 as u64;

type Scalars = [u64; SCALARS];
type Identities = [[u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];

/// One named corpus entry: a mutation applied to the accepted baseline banks.
type Case = (&'static str, fn(&mut Scalars, &mut Identities));

fn input() -> (Scalars, Identities) {
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

/// Run both executors over one input and compare every bank they touch.
///
/// Returns `true` when both admitted the input. On admission the scratch and
/// output banks must match exactly. On refusal both output banks must still hold
/// the pre-execution sentinel and must equal each other; scratch is deliberately
/// unconstrained because `execute_inline_ordinary_atomic` promises only that
/// refusal leaves the caller's output unchanged.
fn assert_equivalent(case: &str, scalars: &Scalars, identities: &Identities) -> bool {
    let transition_bytes = transition();
    let program = ProgramV3::decode(&transition_bytes).expect("program");

    let sentinel_scalars = [SENTINEL_SCALAR; SCALARS];
    let sentinel_identities = [SENTINEL_IDENTITY; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];

    let mut aot_scratch_scalars = *scalars;
    let mut aot_scratch_identities = *identities;
    let mut aot_scalars = sentinel_scalars;
    let mut aot_identities = sentinel_identities;
    let aot = execute_inline_ordinary_atomic(
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
    let mut vm_scalars = sentinel_scalars;
    let mut vm_identities = sentinel_identities;
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

    assert_eq!(
        aot.is_ok(),
        vm.is_ok(),
        "case {case}: AOT returned {aot:?} but the interpreter returned {vm:?}"
    );

    if aot.is_ok() {
        assert_eq!(
            aot_scalars, vm_scalars,
            "case {case}: accepted output scalar banks differ"
        );
        assert_eq!(
            aot_identities, vm_identities,
            "case {case}: accepted output identity banks differ"
        );
        assert_eq!(
            aot_scratch_scalars, vm_scratch_scalars,
            "case {case}: accepted scratch scalar banks differ"
        );
        assert_eq!(
            aot_scratch_identities, vm_scratch_identities,
            "case {case}: accepted scratch identity banks differ"
        );
        true
    } else {
        assert_eq!(
            aot_scalars, sentinel_scalars,
            "case {case}: refused AOT output scalar bank was written"
        );
        assert_eq!(
            aot_identities, sentinel_identities,
            "case {case}: refused AOT output identity bank was written"
        );
        assert_eq!(
            vm_scalars, sentinel_scalars,
            "case {case}: refused interpreter output scalar bank was written"
        );
        assert_eq!(
            vm_identities, sentinel_identities,
            "case {case}: refused interpreter output identity bank was written"
        );
        assert_eq!(
            aot_scalars, vm_scalars,
            "case {case}: refused output scalar banks differ"
        );
        assert_eq!(
            aot_identities, vm_identities,
            "case {case}: refused output identity banks differ"
        );
        false
    }
}

fn apply(case: Case) -> (Scalars, Identities) {
    let (mut scalars, mut identities) = input();
    (case.1)(&mut scalars, &mut identities);
    (scalars, identities)
}

/// Inputs that must be refused, one per distinct guard reachable from the
/// baseline. Each entry trips a `require`, a `lifecycle_accepts`, a `checked_*`,
/// or a `mul_div_*` in `execute_inline_candidate` and the matching instruction in
/// the emitted program.
const HOSTILE_CORPUS: &[Case] = &[
    ("root_phase_not_open", |s, _i| {
        s[SCALAR_ROOT_PHASE_V3] = 1;
    }),
    ("zero_fill", |s, _i| {
        s[SCALAR_FILL_V3] = 0;
    }),
    ("fill_above_seller_maximum", |s, _i| {
        s[SCALAR_FILL_V3] = 11;
    }),
    ("fill_above_buyer_maximum", |s, _i| {
        s[SCALAR_BUYER_MAXIMUM_V3] = 9;
    }),
    ("fill_u64_max", |s, _i| {
        s[SCALAR_FILL_V3] = u64::MAX;
    }),
    ("seller_valid_from_after_slot", |s, _i| {
        s[SCALAR_SELLER_VALID_FROM_V3] = 101;
    }),
    ("slot_after_seller_valid_through", |s, _i| {
        s[SCALAR_SELLER_VALID_THROUGH_V3] = 99;
    }),
    ("buyer_valid_from_after_slot", |s, _i| {
        s[SCALAR_BUYER_VALID_FROM_V3] = 101;
    }),
    ("slot_after_buyer_valid_through", |s, _i| {
        s[SCALAR_BUYER_VALID_THROUGH_V3] = 99;
    }),
    ("seller_side_not_ask", |s, _i| {
        s[SCALAR_SELLER_SIDE_V3] = 1;
    }),
    ("buyer_side_not_bid", |s, _i| {
        s[SCALAR_BUYER_SIDE_V3] = 0;
    }),
    ("seller_intent_market_mismatch", |_s, i| {
        i[IDENTITY_SELLER_INTENT_MARKET_V3] = FOREIGN;
    }),
    ("buyer_intent_market_mismatch", |_s, i| {
        i[IDENTITY_BUYER_INTENT_MARKET_V3] = FOREIGN;
    }),
    ("market_identity_rotated", |_s, i| {
        i[IDENTITY_MARKET_V3] = FOREIGN;
    }),
    ("seller_generation_skew", |s, _i| {
        s[SCALAR_SELLER_GENERATION_V3] = 8;
    }),
    ("buyer_generation_skew", |s, _i| {
        s[SCALAR_BUYER_GENERATION_V3] = 8;
    }),
    ("market_generation_skew", |s, _i| {
        s[SCALAR_MARKET_GENERATION_V3] = 8;
    }),
    ("outcome_mismatch", |s, _i| {
        s[SCALAR_BUYER_OUTCOME_V3] = 2;
    }),
    ("seller_signer_not_request_maker", |_s, i| {
        i[IDENTITY_SELLER_NATIVE_SIGNER_V3] = FOREIGN;
    }),
    ("buyer_signer_not_request_maker", |_s, i| {
        i[IDENTITY_BUYER_NATIVE_SIGNER_V3] = FOREIGN;
    }),
    ("self_dealing_request_makers", |_s, i| {
        i[IDENTITY_BUYER_NATIVE_SIGNER_V3] = [3; 32];
        i[IDENTITY_BUYER_REQUEST_MAKER_V3] = [3; 32];
    }),
    ("seller_collateral_request_mismatch", |_s, i| {
        i[IDENTITY_SELLER_TOKEN_ACCOUNT_V3] = FOREIGN;
    }),
    ("buyer_collateral_request_mismatch", |_s, i| {
        i[IDENTITY_BUYER_TOKEN_ACCOUNT_V3] = FOREIGN;
    }),
    ("outcome_at_outcome_count", |s, _i| {
        s[SCALAR_SELLER_OUTCOME_V3] = 3;
        s[SCALAR_BUYER_OUTCOME_V3] = 3;
    }),
    ("zero_outcome_count", |s, _i| {
        s[SCALAR_OUTCOME_COUNT_V3] = 0;
    }),
    ("zero_price_scale", |s, _i| {
        s[SCALAR_PRICE_SCALE_V3] = 0;
    }),
    ("unknown_seller_lifecycle", |s, _i| {
        s[SCALAR_SELLER_LIFECYCLE_V3] = 9;
    }),
    ("unknown_buyer_lifecycle", |s, _i| {
        s[SCALAR_BUYER_LIFECYCLE_V3] = 9;
    }),
    ("seller_fok_partial_fill", |s, _i| {
        s[SCALAR_SELLER_LIFECYCLE_V3] = 0;
        s[SCALAR_SELLER_MAXIMUM_V3] = 11;
    }),
    ("buyer_fok_partial_fill", |s, _i| {
        s[SCALAR_BUYER_LIFECYCLE_V3] = 0;
        s[SCALAR_BUYER_MAXIMUM_V3] = 11;
    }),
    ("seller_nonce_progression_skew", |s, _i| {
        s[SCALAR_SELLER_NONCE_V3] = 2;
    }),
    ("buyer_nonce_progression_skew", |s, _i| {
        s[SCALAR_BUYER_NONCE_V3] = 3;
    }),
    ("seller_next_nonce_saturated", |s, _i| {
        s[SCALAR_SELLER_NONCE_V3] = u64::MAX;
        s[SCALAR_SELLER_NEXT_NONCE_V3] = u64::MAX;
    }),
    ("buyer_next_nonce_saturated", |s, _i| {
        s[SCALAR_BUYER_NONCE_V3] = u64::MAX;
        s[SCALAR_BUYER_NEXT_NONCE_V3] = u64::MAX;
    }),
    ("seller_limit_above_execution_price", |s, _i| {
        s[SCALAR_SELLER_LIMIT_V3] = 51;
    }),
    ("execution_price_above_buyer_limit", |s, _i| {
        s[SCALAR_BUYER_LIMIT_V3] = 49;
    }),
    ("execution_price_above_price_scale", |s, _i| {
        s[SCALAR_PRICE_SCALE_V3] = 49;
    }),
    ("execution_price_u64_max", |s, _i| {
        s[SCALAR_EXECUTION_PRICE_V3] = u64::MAX;
    }),
    ("seller_fee_bps_off_policy", |s, _i| {
        s[SCALAR_SELLER_FEE_BPS_V3] = 101;
    }),
    ("buyer_fee_bps_off_policy", |s, _i| {
        s[SCALAR_BUYER_FEE_BPS_V3] = 101;
    }),
    ("inexact_gross_division", |s, _i| {
        s[SCALAR_EXECUTION_PRICE_V3] = 51;
    }),
    ("fee_bps_u64_max", |s, _i| {
        s[SCALAR_POLICY_FEE_BPS_V3] = u64::MAX;
        s[SCALAR_SELLER_FEE_BPS_V3] = u64::MAX;
        s[SCALAR_BUYER_FEE_BPS_V3] = u64::MAX;
    }),
    ("seller_created_above_one", |s, _i| {
        s[SCALAR_SELLER_CREATED_V3] = 2;
    }),
    ("buyer_created_above_one", |s, _i| {
        s[SCALAR_BUYER_CREATED_V3] = 2;
    }),
    ("root_open_count_saturated_by_seller", |s, _i| {
        s[SCALAR_ROOT_OPEN_COUNT_V3] = u64::MAX;
        s[SCALAR_SELLER_CREATED_V3] = 1;
    }),
    ("root_open_count_saturated_by_buyer", |s, _i| {
        s[SCALAR_ROOT_OPEN_COUNT_V3] = u64::MAX;
        s[SCALAR_BUYER_CREATED_V3] = 1;
    }),
    ("seller_state_owner_mismatch", |_s, i| {
        i[IDENTITY_SELLER_STATE_OWNER_V3] = FOREIGN;
    }),
    ("buyer_state_owner_mismatch", |_s, i| {
        i[IDENTITY_BUYER_STATE_OWNER_V3] = FOREIGN;
    }),
    ("custody_revision_saturated", |s, _i| {
        s[SCALAR_CUSTODY_REVISION_V3] = u64::MAX;
    }),
    ("custody_revision_second_increment_saturated", |s, _i| {
        s[SCALAR_POLICY_FEE_BPS_V3] = 2_000;
        s[SCALAR_SELLER_FEE_BPS_V3] = 2_000;
        s[SCALAR_BUYER_FEE_BPS_V3] = 2_000;
        s[SCALAR_CUSTODY_REVISION_V3] = u64::MAX - 1;
    }),
];

/// Inputs that sit exactly on an admissible boundary, plus the distinct settlement
/// routes the epilogue selects. Both executors must admit them and agree bank for
/// bank.
const BOUNDARY_CORPUS: &[Case] = &[
    ("baseline_fill_at_maximum", |_s, _i| {}),
    ("seller_fok_exact_fill", |s, _i| {
        s[SCALAR_SELLER_LIFECYCLE_V3] = 0;
    }),
    ("buyer_fok_exact_fill", |s, _i| {
        s[SCALAR_BUYER_LIFECYCLE_V3] = 0;
    }),
    ("gtc_partial_fill", |s, _i| {
        s[SCALAR_SELLER_LIFECYCLE_V3] = 2;
        s[SCALAR_BUYER_LIFECYCLE_V3] = 2;
        s[SCALAR_FILL_V3] = 4;
    }),
    ("execution_price_equals_price_scale", |s, _i| {
        s[SCALAR_EXECUTION_PRICE_V3] = 100;
        s[SCALAR_BUYER_LIMIT_V3] = 100;
    }),
    ("seller_limit_equals_execution_price", |s, _i| {
        s[SCALAR_SELLER_LIMIT_V3] = 50;
    }),
    ("execution_price_equals_buyer_limit", |s, _i| {
        s[SCALAR_BUYER_LIMIT_V3] = 50;
    }),
    ("zero_gross_fill", |s, _i| {
        s[SCALAR_SELLER_LIMIT_V3] = 0;
        s[SCALAR_EXECUTION_PRICE_V3] = 0;
    }),
    ("slot_equals_valid_from", |s, _i| {
        s[SCALAR_SELLER_VALID_FROM_V3] = 100;
        s[SCALAR_BUYER_VALID_FROM_V3] = 100;
    }),
    ("slot_equals_valid_through", |s, _i| {
        s[SCALAR_SELLER_VALID_THROUGH_V3] = 100;
        s[SCALAR_BUYER_VALID_THROUGH_V3] = 100;
    }),
    ("outcome_at_last_index", |s, _i| {
        s[SCALAR_SELLER_OUTCOME_V3] = 2;
        s[SCALAR_BUYER_OUTCOME_V3] = 2;
    }),
    ("outcome_at_first_index", |s, _i| {
        s[SCALAR_SELLER_OUTCOME_V3] = 0;
        s[SCALAR_BUYER_OUTCOME_V3] = 0;
    }),
    ("zero_fee_bps", |s, _i| {
        s[SCALAR_POLICY_FEE_BPS_V3] = 0;
        s[SCALAR_SELLER_FEE_BPS_V3] = 0;
        s[SCALAR_BUYER_FEE_BPS_V3] = 0;
    }),
    ("fee_bps_selects_intermediate_route", |s, _i| {
        s[SCALAR_POLICY_FEE_BPS_V3] = 2_000;
        s[SCALAR_SELLER_FEE_BPS_V3] = 2_000;
        s[SCALAR_BUYER_FEE_BPS_V3] = 2_000;
    }),
    ("fee_bps_equals_denominator", |s, _i| {
        s[SCALAR_POLICY_FEE_BPS_V3] = FEE_DENOMINATOR;
        s[SCALAR_SELLER_FEE_BPS_V3] = FEE_DENOMINATOR;
        s[SCALAR_BUYER_FEE_BPS_V3] = FEE_DENOMINATOR;
    }),
    // The inline ordinary program carries no `fee_bps <= denominator` guard, so
    // this input is admitted by both executors rather than refused.
    ("fee_bps_above_denominator", |s, _i| {
        s[SCALAR_POLICY_FEE_BPS_V3] = FEE_DENOMINATOR + 1;
        s[SCALAR_SELLER_FEE_BPS_V3] = FEE_DENOMINATOR + 1;
        s[SCALAR_BUYER_FEE_BPS_V3] = FEE_DENOMINATOR + 1;
    }),
    ("both_participants_created", |s, _i| {
        s[SCALAR_SELLER_CREATED_V3] = 1;
        s[SCALAR_BUYER_CREATED_V3] = 1;
    }),
    ("root_open_count_one_below_saturation", |s, _i| {
        s[SCALAR_ROOT_OPEN_COUNT_V3] = u64::MAX - 2;
        s[SCALAR_SELLER_CREATED_V3] = 1;
        s[SCALAR_BUYER_CREATED_V3] = 1;
    }),
    ("nonces_one_below_saturation", |s, _i| {
        s[SCALAR_SELLER_NONCE_V3] = u64::MAX - 1;
        s[SCALAR_SELLER_NEXT_NONCE_V3] = u64::MAX - 1;
        s[SCALAR_BUYER_NONCE_V3] = u64::MAX - 1;
        s[SCALAR_BUYER_NEXT_NONCE_V3] = u64::MAX - 1;
    }),
    ("custody_revision_one_below_saturation", |s, _i| {
        s[SCALAR_CUSTODY_REVISION_V3] = u64::MAX - 1;
    }),
];

#[test]
fn accepted_bank_is_byte_exact_with_transition_vm() {
    let (scalars, identities) = input();
    assert!(
        assert_equivalent("accepted_baseline", &scalars, &identities),
        "accepted_baseline: the baseline inline ordinary fill was refused"
    );
}

#[test]
fn hostile_corpus_refuses_in_both_and_preserves_outputs() {
    for case in HOSTILE_CORPUS {
        let (scalars, identities) = apply(*case);
        let accepted = assert_equivalent(case.0, &scalars, &identities);
        assert!(
            !accepted,
            "case {}: expected a refusal, both admitted",
            case.0
        );
    }
}

#[test]
fn boundary_corpus_matches_admission_and_banks() {
    for case in BOUNDARY_CORPUS {
        let (scalars, identities) = apply(*case);
        let accepted = assert_equivalent(case.0, &scalars, &identities);
        assert!(
            accepted,
            "case {}: expected admission, both refused",
            case.0
        );
    }
}

/// The hand-written AOT translation carries a `seller_outcome < tail_count` guard
/// that the emitted program does not: the emitted program only checks
/// `seller_outcome < SCALAR_OUTCOME_COUNT_V3`, and its item body writes a claim
/// quantity only where the item index matches. When the authenticated outcome
/// count exceeds the Product tail count, the two executors split.
///
/// Left failing on purpose. Ignored only so the rest of the corpus still runs.
#[test]
#[ignore = "AOT refuses with CheckFailed while the interpreter admits: \
            execute_inline_candidate adds an outcome < tail_count guard that the \
            emitted DIRECT_ORDINARY_PRELUDE_V3 does not have. With \
            SCALAR_OUTCOME_COUNT_V3 = 5, SCALAR_SELLER_OUTCOME_V3 = 4 and \
            tail_count = 3 the interpreter admits a fill whose item claim \
            quantities are all zero."]
fn outcome_between_tail_count_and_outcome_count() {
    let (mut scalars, identities) = input();
    scalars[SCALAR_OUTCOME_COUNT_V3] = 5;
    scalars[SCALAR_SELLER_OUTCOME_V3] = 4;
    scalars[SCALAR_BUYER_OUTCOME_V3] = 4;
    assert_equivalent(
        "outcome_between_tail_count_and_outcome_count",
        &scalars,
        &identities,
    );
}
