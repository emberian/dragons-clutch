//! Register-bank differential for the registered ordinary fill.
//!
//! Two Rust executors run the same Lean-emitted TransitionVMV3 program bytes:
//! the hand-written AOT translation in `registered.rs` and the generic
//! interpreter in `dclutch-transition-vm`. Every corpus case compares both the
//! scratch and the output banks of the two executors. These are case tests over
//! a fixed corpus; they say nothing about inputs outside it.

use dclutch_direct_codec::registered_fill_artifacts_v4::*;
use dclutch_transition_vm::v3::{
    ProgramV3, RegisterInput as VmInput, RegisterOutput as VmOutput, execute_fold_atomic,
};

use super::*;

const N: u32 = 3;

/// Output-bank fill distinct from zero and from every value either executor
/// computes, so an untouched refusal bank is distinguishable from a written one.
const SENTINEL_SCALAR: u64 = 0x55;
/// Identity-bank counterpart of `SENTINEL_SCALAR`.
const SENTINEL_IDENTITY: [u8; 32] = [0x55; 32];

type Scalars = [u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4];
type Identities = [[u8; 32]; DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4];

/// One named corpus entry: a mutation applied to the accepted baseline banks.
type Case = (&'static str, fn(&mut Scalars, &mut Identities));

/// Identity value that collides with no baseline identity register.
const FOREIGN: [u8; 32] = [9; 32];
/// Exact fee denominator the emitted program loads as a constant.
const FEE_DENOMINATOR: u64 = dclutch_direct_codec::successor::DIRECT_FEE_DENOMINATOR_V1 as u64;

fn valid_scalars() -> Scalars {
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

fn valid_identities() -> Identities {
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

/// Run both executors over one input and compare every bank they touch.
///
/// Returns `true` when both admitted the input. On admission the scratch and
/// output banks must match exactly. On refusal both output banks must still hold
/// the pre-execution sentinel and must equal each other; scratch is deliberately
/// unconstrained because `execute_registered_ordinary_fill_atomic` promises only
/// that refusal leaves the caller's output unchanged.
fn assert_equivalent(case: &str, scalars: &Scalars, identities: &Identities) -> bool {
    let transition_bytes = transition();
    let program = ProgramV3::decode(&transition_bytes).expect("decode transition");

    let sentinel_scalars = [SENTINEL_SCALAR; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4];
    let sentinel_identities = [SENTINEL_IDENTITY; DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4];

    let mut aot_scratch_scalars = *scalars;
    let mut aot_scratch_identities = *identities;
    let mut aot_scalars = sentinel_scalars;
    let mut aot_identities = sentinel_identities;
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
    let mut scalars = valid_scalars();
    let mut identities = valid_identities();
    (case.1)(&mut scalars, &mut identities);
    (scalars, identities)
}

/// Inputs that must be refused, one per distinct guard reachable from the
/// baseline. Each entry trips a `require`, a `checked_*`, or a `mul_div_*` in
/// `execute_candidate` and the matching instruction in the emitted program.
const HOSTILE_CORPUS: &[Case] = &[
    ("root_phase_not_open", |s, _i| {
        s[FILL_SCALAR_ROOT_PHASE_V4] = 1;
    }),
    ("zero_open_outcome_count", |s, _i| {
        s[FILL_SCALAR_ROOT_OPEN_COUNT_V4] = 0;
    }),
    ("zero_quantity", |s, _i| {
        s[FILL_SCALAR_QUANTITY_V4] = 0;
    }),
    ("quantity_u64_max", |s, _i| {
        s[FILL_SCALAR_QUANTITY_V4] = u64::MAX;
    }),
    ("quantity_above_seller_maximum", |s, _i| {
        s[FILL_SCALAR_QUANTITY_V4] = 21;
    }),
    ("quantity_overflows_seller_filled", |s, _i| {
        s[FILL_SCALAR_SELLER_FILLED_V4] = 1;
        s[FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4] = 19;
        s[FILL_SCALAR_QUANTITY_V4] = u64::MAX;
    }),
    ("inexact_gross_division", |s, _i| {
        s[FILL_SCALAR_EXECUTION_PRICE_V4] = 51;
    }),
    ("market_identity_rotated", |_s, i| {
        i[FILL_IDENTITY_MARKET_V4] = FOREIGN;
    }),
    ("seller_intent_market_mismatch", |_s, i| {
        i[FILL_IDENTITY_SELLER_INTENT_MARKET_V4] = FOREIGN;
    }),
    ("buyer_intent_market_mismatch", |_s, i| {
        i[FILL_IDENTITY_BUYER_INTENT_MARKET_V4] = FOREIGN;
    }),
    ("seller_maker_market_mismatch", |_s, i| {
        i[FILL_IDENTITY_SELLER_MAKER_MARKET_V4] = FOREIGN;
    }),
    ("buyer_maker_market_mismatch", |_s, i| {
        i[FILL_IDENTITY_BUYER_MAKER_MARKET_V4] = FOREIGN;
    }),
    ("seller_maker_equals_buyer_maker", |_s, i| {
        i[FILL_IDENTITY_BUYER_MAKER_V4] = [3; 32];
        i[FILL_IDENTITY_BUYER_MAKER_REPLAY_OWNER_V4] = [3; 32];
    }),
    ("seller_replay_owner_mismatch", |_s, i| {
        i[FILL_IDENTITY_SELLER_MAKER_REPLAY_OWNER_V4] = FOREIGN;
    }),
    ("buyer_replay_owner_mismatch", |_s, i| {
        i[FILL_IDENTITY_BUYER_MAKER_REPLAY_OWNER_V4] = FOREIGN;
    }),
    ("market_generation_skew", |s, _i| {
        s[FILL_SCALAR_MARKET_GENERATION_V4] = 8;
    }),
    ("seller_generation_skew", |s, _i| {
        s[FILL_SCALAR_SELLER_GENERATION_V4] = 8;
    }),
    ("buyer_generation_skew", |s, _i| {
        s[FILL_SCALAR_BUYER_GENERATION_V4] = 8;
    }),
    ("seller_maker_generation_skew", |s, _i| {
        s[FILL_SCALAR_SELLER_MAKER_GENERATION_V4] = 8;
    }),
    ("buyer_maker_generation_skew", |s, _i| {
        s[FILL_SCALAR_BUYER_MAKER_GENERATION_V4] = 8;
    }),
    ("seller_side_not_ask", |s, _i| {
        s[FILL_SCALAR_SELLER_SIDE_V4] = 1;
    }),
    ("buyer_side_not_bid", |s, _i| {
        s[FILL_SCALAR_BUYER_SIDE_V4] = 0;
    }),
    ("seller_lifecycle_not_gtc", |s, _i| {
        s[FILL_SCALAR_SELLER_LIFECYCLE_V4] = 1;
    }),
    ("buyer_lifecycle_not_gtc", |s, _i| {
        s[FILL_SCALAR_BUYER_LIFECYCLE_V4] = 1;
    }),
    ("outcome_mismatch", |s, _i| {
        s[FILL_SCALAR_BUYER_OUTCOME_V4] = 2;
    }),
    ("outcome_at_outcome_count", |s, _i| {
        s[FILL_SCALAR_SELLER_OUTCOME_V4] = 3;
        s[FILL_SCALAR_BUYER_OUTCOME_V4] = 3;
    }),
    ("zero_outcome_count", |s, _i| {
        s[FILL_SCALAR_OUTCOME_COUNT_V4] = 0;
    }),
    ("seller_fee_bps_off_policy", |s, _i| {
        s[FILL_SCALAR_SELLER_FEE_BPS_V4] = 101;
    }),
    ("buyer_fee_bps_off_policy", |s, _i| {
        s[FILL_SCALAR_BUYER_FEE_BPS_V4] = 101;
    }),
    ("fee_bps_above_denominator", |s, _i| {
        s[FILL_SCALAR_POLICY_FEE_BPS_V4] = FEE_DENOMINATOR + 1;
        s[FILL_SCALAR_SELLER_FEE_BPS_V4] = FEE_DENOMINATOR + 1;
        s[FILL_SCALAR_BUYER_FEE_BPS_V4] = FEE_DENOMINATOR + 1;
    }),
    ("fee_bps_u64_max", |s, _i| {
        s[FILL_SCALAR_POLICY_FEE_BPS_V4] = u64::MAX;
        s[FILL_SCALAR_SELLER_FEE_BPS_V4] = u64::MAX;
        s[FILL_SCALAR_BUYER_FEE_BPS_V4] = u64::MAX;
    }),
    ("seller_valid_from_after_slot", |s, _i| {
        s[FILL_SCALAR_SELLER_VALID_FROM_V4] = 101;
    }),
    ("slot_after_seller_valid_through", |s, _i| {
        s[FILL_SCALAR_SELLER_VALID_THROUGH_V4] = 99;
    }),
    ("buyer_valid_from_after_slot", |s, _i| {
        s[FILL_SCALAR_BUYER_VALID_FROM_V4] = 101;
    }),
    ("slot_after_buyer_valid_through", |s, _i| {
        s[FILL_SCALAR_BUYER_VALID_THROUGH_V4] = 99;
    }),
    ("seller_limit_above_execution_price", |s, _i| {
        s[FILL_SCALAR_SELLER_LIMIT_V4] = 51;
    }),
    ("seller_limit_u64_max", |s, _i| {
        s[FILL_SCALAR_SELLER_LIMIT_V4] = u64::MAX;
    }),
    ("execution_price_above_buyer_limit", |s, _i| {
        s[FILL_SCALAR_BUYER_LIMIT_V4] = 49;
    }),
    ("execution_price_above_price_scale", |s, _i| {
        s[FILL_SCALAR_EXECUTION_PRICE_V4] = 101;
    }),
    ("execution_price_u64_max", |s, _i| {
        s[FILL_SCALAR_EXECUTION_PRICE_V4] = u64::MAX;
    }),
    ("buyer_limit_above_price_scale", |s, _i| {
        s[FILL_SCALAR_BUYER_LIMIT_V4] = 101;
    }),
    ("buyer_limit_u64_max", |s, _i| {
        s[FILL_SCALAR_BUYER_LIMIT_V4] = u64::MAX;
    }),
    ("zero_price_scale", |s, _i| {
        s[FILL_SCALAR_PRICE_SCALE_V4] = 0;
    }),
    ("zero_price_scale_and_zero_prices", |s, _i| {
        s[FILL_SCALAR_PRICE_SCALE_V4] = 0;
        s[FILL_SCALAR_EXECUTION_PRICE_V4] = 0;
        s[FILL_SCALAR_SELLER_LIMIT_V4] = 0;
        s[FILL_SCALAR_BUYER_LIMIT_V4] = 0;
    }),
    ("price_scale_u64_max", |s, _i| {
        s[FILL_SCALAR_PRICE_SCALE_V4] = u64::MAX;
    }),
    ("seller_nonce_not_below_next", |s, _i| {
        s[FILL_SCALAR_SELLER_NONCE_V4] = 1;
    }),
    ("buyer_nonce_not_below_next", |s, _i| {
        s[FILL_SCALAR_BUYER_NONCE_V4] = 1;
    }),
    ("seller_minimum_nonce_above_nonce", |s, _i| {
        s[FILL_SCALAR_SELLER_MINIMUM_NONCE_V4] = 1;
    }),
    ("buyer_minimum_nonce_above_nonce", |s, _i| {
        s[FILL_SCALAR_BUYER_MINIMUM_NONCE_V4] = 1;
    }),
    ("zero_seller_live_count", |s, _i| {
        s[FILL_SCALAR_SELLER_LIVE_COUNT_V4] = 0;
    }),
    ("zero_buyer_live_count", |s, _i| {
        s[FILL_SCALAR_BUYER_LIVE_COUNT_V4] = 0;
    }),
    ("seller_live_count_above_next_nonce", |s, _i| {
        s[FILL_SCALAR_SELLER_LIVE_COUNT_V4] = 2;
    }),
    ("buyer_live_count_above_next_nonce", |s, _i| {
        s[FILL_SCALAR_BUYER_LIVE_COUNT_V4] = 2;
    }),
    ("zero_seller_maker_rent_principal", |s, _i| {
        s[FILL_SCALAR_SELLER_MAKER_RENT_PRINCIPAL_V4] = 0;
    }),
    ("zero_seller_record_rent_principal", |s, _i| {
        s[FILL_SCALAR_SELLER_RECORD_RENT_PRINCIPAL_V4] = 0;
    }),
    ("zero_buyer_maker_rent_principal", |s, _i| {
        s[FILL_SCALAR_BUYER_MAKER_RENT_PRINCIPAL_V4] = 0;
    }),
    ("zero_buyer_record_rent_principal", |s, _i| {
        s[FILL_SCALAR_BUYER_RECORD_RENT_PRINCIPAL_V4] = 0;
    }),
    ("seller_filled_at_maximum", |s, _i| {
        s[FILL_SCALAR_SELLER_FILLED_V4] = 20;
    }),
    ("buyer_filled_at_maximum", |s, _i| {
        s[FILL_SCALAR_BUYER_FILLED_V4] = 20;
    }),
    ("zero_seller_maximum", |s, _i| {
        s[FILL_SCALAR_SELLER_MAXIMUM_V4] = 0;
    }),
    ("zero_buyer_maximum", |s, _i| {
        s[FILL_SCALAR_BUYER_MAXIMUM_V4] = 0;
    }),
    ("seller_maximum_u64_max", |s, _i| {
        s[FILL_SCALAR_SELLER_MAXIMUM_V4] = u64::MAX;
    }),
    ("buyer_maximum_u64_max", |s, _i| {
        s[FILL_SCALAR_BUYER_MAXIMUM_V4] = u64::MAX;
    }),
    ("seller_cumulative_gross_above_filled", |s, _i| {
        s[FILL_SCALAR_SELLER_CUMULATIVE_GROSS_V4] = 1;
    }),
    ("buyer_cumulative_gross_above_filled", |s, _i| {
        s[FILL_SCALAR_BUYER_CUMULATIVE_GROSS_V4] = 1;
    }),
    ("seller_cumulative_fee_mismatch", |s, _i| {
        s[FILL_SCALAR_SELLER_CUMULATIVE_FEE_V4] = 1;
    }),
    ("buyer_cumulative_fee_mismatch", |s, _i| {
        s[FILL_SCALAR_BUYER_CUMULATIVE_FEE_V4] = 1;
    }),
    ("seller_reserved_claims_short", |s, _i| {
        s[FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4] = 19;
    }),
    ("seller_reserved_claims_u64_max", |s, _i| {
        s[FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4] = u64::MAX;
    }),
    ("seller_reserved_collateral_nonzero", |s, _i| {
        s[FILL_SCALAR_SELLER_RESERVED_COLLATERAL_V4] = 1;
    }),
    ("buyer_reserved_collateral_short", |s, _i| {
        s[FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4] = 11;
    }),
    ("buyer_reserved_collateral_u64_max", |s, _i| {
        s[FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4] = u64::MAX;
    }),
    ("buyer_reserved_claims_nonzero", |s, _i| {
        s[FILL_SCALAR_BUYER_RESERVED_CLAIMS_V4] = 1;
    }),
    ("buyer_initial_reserve_overflows", |s, _i| {
        s[FILL_SCALAR_PRICE_SCALE_V4] = 1;
        s[FILL_SCALAR_EXECUTION_PRICE_V4] = 1;
        s[FILL_SCALAR_SELLER_LIMIT_V4] = 0;
        s[FILL_SCALAR_BUYER_LIMIT_V4] = 1;
        s[FILL_SCALAR_BUYER_MAXIMUM_V4] = u64::MAX;
    }),
    ("claim_source_revision_saturated", |s, _i| {
        s[FILL_SCALAR_CLAIM_SOURCE_REVISION_V4] = u64::MAX;
    }),
    ("claim_destination_revision_saturated", |s, _i| {
        s[FILL_SCALAR_CLAIM_DESTINATION_REVISION_V4] = u64::MAX;
    }),
    ("custody_revision_saturated", |s, _i| {
        s[FILL_SCALAR_CUSTODY_REVISION_V4] = u64::MAX;
    }),
    ("custody_revision_second_increment_saturated", |s, _i| {
        s[FILL_SCALAR_CUSTODY_REVISION_V4] = u64::MAX - 1;
    }),
];

/// Inputs that sit exactly on an admissible boundary: the just-inside twins of
/// the refusals above. Both executors must admit them and agree bank for bank.
const BOUNDARY_CORPUS: &[Case] = &[
    ("baseline_partial_fill", |_s, _i| {}),
    ("quantity_equals_both_maximums", |s, _i| {
        s[FILL_SCALAR_QUANTITY_V4] = 20;
    }),
    ("execution_price_equals_price_scale", |s, _i| {
        s[FILL_SCALAR_EXECUTION_PRICE_V4] = 100;
        s[FILL_SCALAR_BUYER_LIMIT_V4] = 100;
        s[FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4] = 20;
    }),
    ("buyer_limit_equals_price_scale", |s, _i| {
        s[FILL_SCALAR_BUYER_LIMIT_V4] = 100;
        s[FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4] = 20;
    }),
    ("seller_limit_equals_execution_price", |s, _i| {
        s[FILL_SCALAR_SELLER_LIMIT_V4] = 50;
    }),
    ("slot_equals_valid_from", |s, _i| {
        s[FILL_SCALAR_SELLER_VALID_FROM_V4] = 100;
        s[FILL_SCALAR_BUYER_VALID_FROM_V4] = 100;
    }),
    ("slot_equals_valid_through", |s, _i| {
        s[FILL_SCALAR_SELLER_VALID_THROUGH_V4] = 100;
        s[FILL_SCALAR_BUYER_VALID_THROUGH_V4] = 100;
    }),
    ("minimum_nonce_equals_nonce", |s, _i| {
        s[FILL_SCALAR_SELLER_NONCE_V4] = 5;
        s[FILL_SCALAR_SELLER_NEXT_NONCE_V4] = 6;
        s[FILL_SCALAR_SELLER_MINIMUM_NONCE_V4] = 5;
        s[FILL_SCALAR_BUYER_NONCE_V4] = 5;
        s[FILL_SCALAR_BUYER_NEXT_NONCE_V4] = 6;
        s[FILL_SCALAR_BUYER_MINIMUM_NONCE_V4] = 5;
    }),
    ("live_count_equals_next_nonce", |s, _i| {
        s[FILL_SCALAR_SELLER_NONCE_V4] = 2;
        s[FILL_SCALAR_SELLER_NEXT_NONCE_V4] = 3;
        s[FILL_SCALAR_SELLER_LIVE_COUNT_V4] = 3;
        s[FILL_SCALAR_BUYER_NONCE_V4] = 2;
        s[FILL_SCALAR_BUYER_NEXT_NONCE_V4] = 3;
        s[FILL_SCALAR_BUYER_LIVE_COUNT_V4] = 3;
    }),
    ("filled_one_step_below_maximum", |s, _i| {
        s[FILL_SCALAR_SELLER_FILLED_V4] = 18;
        s[FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4] = 2;
        s[FILL_SCALAR_BUYER_FILLED_V4] = 18;
        s[FILL_SCALAR_QUANTITY_V4] = 2;
    }),
    ("buyer_reserve_exactly_exhausted", |s, _i| {
        s[FILL_SCALAR_EXECUTION_PRICE_V4] = 60;
        s[FILL_SCALAR_QUANTITY_V4] = 20;
    }),
    ("rent_principals_saturated", |s, _i| {
        s[FILL_SCALAR_SELLER_MAKER_RENT_PRINCIPAL_V4] = u64::MAX;
        s[FILL_SCALAR_SELLER_RECORD_RENT_PRINCIPAL_V4] = u64::MAX;
        s[FILL_SCALAR_BUYER_MAKER_RENT_PRINCIPAL_V4] = u64::MAX;
        s[FILL_SCALAR_BUYER_RECORD_RENT_PRINCIPAL_V4] = u64::MAX;
    }),
    ("root_open_count_saturated", |s, _i| {
        s[FILL_SCALAR_ROOT_OPEN_COUNT_V4] = u64::MAX;
    }),
    ("zero_fee_bps", |s, _i| {
        s[FILL_SCALAR_POLICY_FEE_BPS_V4] = 0;
        s[FILL_SCALAR_SELLER_FEE_BPS_V4] = 0;
        s[FILL_SCALAR_BUYER_FEE_BPS_V4] = 0;
    }),
    ("fee_bps_equals_denominator", |s, _i| {
        s[FILL_SCALAR_POLICY_FEE_BPS_V4] = FEE_DENOMINATOR;
        s[FILL_SCALAR_SELLER_FEE_BPS_V4] = FEE_DENOMINATOR;
        s[FILL_SCALAR_BUYER_FEE_BPS_V4] = FEE_DENOMINATOR;
        s[FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4] = 24;
    }),
    ("outcome_at_last_index", |s, _i| {
        s[FILL_SCALAR_SELLER_OUTCOME_V4] = 2;
        s[FILL_SCALAR_BUYER_OUTCOME_V4] = 2;
    }),
    ("outcome_at_first_index", |s, _i| {
        s[FILL_SCALAR_SELLER_OUTCOME_V4] = 0;
        s[FILL_SCALAR_BUYER_OUTCOME_V4] = 0;
    }),
    ("custody_revision_one_below_saturation", |s, _i| {
        s[FILL_SCALAR_CUSTODY_REVISION_V4] = u64::MAX - 2;
    }),
    ("claim_revisions_one_below_saturation", |s, _i| {
        s[FILL_SCALAR_CLAIM_SOURCE_REVISION_V4] = u64::MAX - 1;
        s[FILL_SCALAR_CLAIM_DESTINATION_REVISION_V4] = u64::MAX - 1;
    }),
];

#[test]
fn accepted_partial_and_terminal_banks_are_exact() {
    let identities = valid_identities();
    let partial = valid_scalars();
    assert!(
        assert_equivalent("accepted_partial", &partial, &identities),
        "accepted_partial: the baseline registered fill was refused"
    );

    let mut terminal = valid_scalars();
    terminal[FILL_SCALAR_QUANTITY_V4] = 20;
    assert!(
        assert_equivalent("accepted_terminal", &terminal, &identities),
        "accepted_terminal: the terminal registered fill was refused"
    );
}

#[test]
fn hostile_corpus_matches_refusal_and_preserves_output() {
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
