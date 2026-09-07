//! The scoring Dealer's accelerator arm: the same kernel, over the witness.
//!
//! `DealerFill` links the kernel and is authoritative (BUILD_DEALER.md §4.7).
//! This arm exists for the caller that does NOT hold the accounts: a General
//! candidate verifier already runs through the accelerator, and a Dealer row
//! inside a batch has to be admitted by the same `admit_fill` that the direct
//! route runs, not by a second implementation of R0–R3 that can disagree
//! with it. So the arm is a PURE FUNCTION and says so:
//!
//! - its input is the request the row carries (`DCLSFLR1`, untrusted) and the
//!   witness the caller composed from accounts it authenticated (`DCLSFLW1`);
//! - it authenticates nothing about accounts, because it is handed none. The
//!   witness IS the caller's authentication, and a caller that composes a
//!   witness from unauthenticated bytes has lied to itself, not to this arm;
//! - its output is one scalar bank the caller compares byte for byte against
//!   what its own state says, exactly as the LP and equity arms' banks are
//!   compared (`dealer/lp_accelerator.rs`'s `encode_bank`).
//!
//! **Why a magic on the instruction data and not the admitted-AOT transport.**
//! The two admitted transports select their entry by a `u16` at byte 10 of the
//! family request (`CapabilityProgramSetV1::selector_offset`, the Dealer set's
//! is 10). Byte 10 of `DCLSFLR1` is `outcome_count`, so a scoring row read
//! through that transport would select entry `K ∈ 2..=16` -- the equity and LP
//! selectors. Giving the scoring family a selector coordinate is a change to a
//! Lean-emitted layout; until it has one, this arm is transported the way the
//! Series arm already is, by its own magic on the instruction data, which is
//! also the honest shape for a function that authenticates no account.

extern crate alloc;

use dclutch_trading::scoring_rule::generated::MAX_OUTCOMES;
use dclutch_trading::scoring_rule::requests_v1::{DealerFillRequestV1, DealerFillWitnessV1};
use dclutch_trading::scoring_rule::{AdmittedFill, ScoringRefusal, admit_fill, generated};

/// The scoring row's accelerator wire: the witness, then the request.
///
/// The witness leads so the wire's magic is `DCLSFLW1` -- the accelerator's
/// own object, distinct from the `DCLSFLR1` the Trading route is selected by,
/// so neither transport can be mistaken for the other.
pub const SCORING_ROW_REQUEST_BYTES_V1: usize =
    generated::FILL_WITNESS_BYTES + generated::FILL_REQUEST_BYTES;

/// The bank's fixed scalars, before the two per-outcome vectors.
pub const SCORING_ROW_FIXED_SCALARS_V1: usize = 6;

/// Exact bank width for one row of `K` outcomes.
#[must_use]
pub const fn scoring_row_bank_bytes_v1(outcome_count: u8) -> usize {
    (SCORING_ROW_FIXED_SCALARS_V1 + 2 * outcome_count as usize) * 8
}

/// The widest bank any admitted rule can ask for.
pub const SCORING_ROW_MAX_BANK_BYTES_V1: usize =
    (SCORING_ROW_FIXED_SCALARS_V1 + 2 * MAX_OUTCOMES) * 8;

/// Why the arm refused, one conjunct each.
///
/// The kernel's fourteen causes are carried through rather than folded: the
/// boundary that publishes `Rule` alone would hand a campaign a universal
/// donor, which is the defect `accepted_or_named_v4` was written to end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoringAcceleratorRefusalV1 {
    /// The wire was not `SCORING_ROW_REQUEST_BYTES_V1` of witness then request.
    Transport,
    /// The witness half did not decode.
    Witness,
    /// The request half did not decode.
    Request,
    /// The two halves do not describe one row: market, dealer, width or the
    /// fund revision the request expects.
    Join,
    /// The bank the caller provisioned is not the width this row needs.
    Bank,
    /// The rule refused, by its own name.
    Rule(ScoringRefusal),
}

impl ScoringAcceleratorRefusalV1 {
    /// The line the arm logs, so a test can assert a cause by name.
    #[must_use]
    pub const fn refusal_name(self) -> &'static str {
        match self {
            Self::Transport => "scoring:Transport",
            Self::Witness => "scoring:Witness",
            Self::Request => "scoring:Request",
            Self::Join => "scoring:Join",
            Self::Bank => "scoring:Bank",
            Self::Rule(cause) => match cause {
                ScoringRefusal::OutcomeCount => "scoring:OutcomeCount",
                ScoringRefusal::Liquidity => "scoring:Liquidity",
                ScoringRefusal::Scale => "scoring:Scale",
                ScoringRefusal::Tolerance => "scoring:Tolerance",
                ScoringRefusal::Subsidy => "scoring:Subsidy",
                ScoringRefusal::Width => "scoring:Width",
                ScoringRefusal::Deliverable => "scoring:Deliverable",
                ScoringRefusal::NonCanonical => "scoring:NonCanonical",
                ScoringRefusal::NotNormalized => "scoring:NotNormalized",
                ScoringRefusal::OffSchedule => "scoring:OffSchedule",
                ScoringRefusal::Uncovered => "scoring:Uncovered",
                ScoringRefusal::PricesNotSimplex => "scoring:PricesNotSimplex",
                ScoringRefusal::Overflow => "scoring:Overflow",
                ScoringRefusal::WithdrawBelowFloor => "scoring:WithdrawBelowFloor",
            },
        }
    }
}

/// Split one accelerator wire into the witness the caller authenticated and
/// the row it is asked to admit.
pub fn decode_scoring_row_v1(
    request_bytes: &[u8],
) -> Result<(DealerFillWitnessV1, DealerFillRequestV1), ScoringAcceleratorRefusalV1> {
    if request_bytes.len() != SCORING_ROW_REQUEST_BYTES_V1 {
        return Err(ScoringAcceleratorRefusalV1::Transport);
    }
    let (witness_bytes, request_half) = request_bytes.split_at(generated::FILL_WITNESS_BYTES);
    let witness = DealerFillWitnessV1::decode(witness_bytes)
        .map_err(|_| ScoringAcceleratorRefusalV1::Witness)?;
    let request = DealerFillRequestV1::decode(request_half)
        .map_err(|_| ScoringAcceleratorRefusalV1::Request)?;
    if witness.rule.market != request.market
        || witness.rule.dealer_id != request.dealer_id
        || witness.rule.parameters.outcome_count != request.outcome_count
        || witness.fund_revision != request.expected_fund_revision
    {
        return Err(ScoringAcceleratorRefusalV1::Join);
    }
    Ok((witness, request))
}

/// Evaluate one scoring Dealer row and write the caller's candidate bank.
///
/// Commit-last, as both sibling arms are: `candidate_bank` is byte-for-byte
/// unchanged on every refusal, so a caller that reads a refused bank reads
/// what it wrote there itself.
pub fn evaluate_scoring_dealer_row_v1(
    request_bytes: &[u8],
    candidate_bank: &mut [u8],
) -> Result<AdmittedFill, ScoringAcceleratorRefusalV1> {
    let (witness, request) = decode_scoring_row_v1(request_bytes)?;
    let width = usize::from(witness.rule.parameters.outcome_count);
    if candidate_bank.len() != scoring_row_bank_bytes_v1(witness.rule.parameters.outcome_count) {
        return Err(ScoringAcceleratorRefusalV1::Bank);
    }
    // The rule's own parameters, re-admitted here: the witness is the caller's
    // word for what the seal says, and a caller that got the seal wrong should
    // be refused by the same conjunct the founding refuses by.
    let rule = witness
        .rule
        .parameters
        .admit()
        .map_err(ScoringAcceleratorRefusalV1::Rule)?;
    let admitted = admit_fill(
        rule,
        &witness.inventory,
        &request.receive,
        &request.deliver,
        &request.prices,
    )
    .map_err(ScoringAcceleratorRefusalV1::Rule)?;
    let mut staged = [0_u64; SCORING_ROW_FIXED_SCALARS_V1 + 2 * MAX_OUTCOMES];
    staged[0] = admitted.dealer_pays;
    staged[1] = admitted.dealer_receives;
    staged[2] = admitted.potential_before.minimum;
    staged[3] = admitted.potential_before.cost;
    staged[4] = admitted.potential_after.minimum;
    staged[5] = admitted.potential_after.cost;
    for outcome in 0..width {
        staged[SCORING_ROW_FIXED_SCALARS_V1 + outcome] = admitted.next_inventory[outcome];
        staged[SCORING_ROW_FIXED_SCALARS_V1 + width + outcome] = admitted.schedule[outcome];
    }
    for (index, chunk) in candidate_bank.chunks_exact_mut(8).enumerate() {
        chunk.copy_from_slice(&staged[index].to_le_bytes());
    }
    Ok(admitted)
}

#[cfg(test)]
mod tests {
    use dclutch_trading::scoring_rule::records_v1::ScoringRuleRecordV1;
    use dclutch_trading::scoring_rule::{
        RuleParameters, ZERO_VECTOR, potential, prices_of, subsidy_of,
    };

    use super::*;

    fn parameters(outcome_count: u8) -> RuleParameters {
        RuleParameters {
            outcome_count,
            liquidity: 1 << 20,
            scale: 1 << 62,
            tolerance: 1,
        }
    }

    fn rule(outcome_count: u8) -> ScoringRuleRecordV1 {
        let parameters = parameters(outcome_count);
        ScoringRuleRecordV1 {
            parameters,
            market: [7; 32],
            dealer_id: [9; 32],
            subsidy: subsidy_of(parameters).expect("subsidy"),
        }
    }

    fn witness(outcome_count: u8, inventory: [u64; MAX_OUTCOMES]) -> DealerFillWitnessV1 {
        DealerFillWitnessV1 {
            rule: rule(outcome_count),
            inventory,
            fund_revision: 3,
            cash: 1 << 30,
        }
    }

    fn request(outcome_count: u8) -> DealerFillRequestV1 {
        DealerFillRequestV1 {
            outcome_count,
            market: [7; 32],
            dealer_id: [9; 32],
            taker: [5; 32],
            expected_fund_revision: 3,
            mint: 0,
            prices: prices_of(parameters(outcome_count), &ZERO_VECTOR).expect("prices"),
            receive: ZERO_VECTOR,
            deliver: ZERO_VECTOR,
        }
    }

    fn wire(witness: DealerFillWitnessV1, request: DealerFillRequestV1) -> alloc::vec::Vec<u8> {
        let mut bytes = witness.to_bytes().expect("witness").to_vec();
        bytes.extend_from_slice(&request.to_bytes().expect("request"));
        bytes
    }

    /// The null fill clears at the Dealer's own price, and the bank the arm
    /// writes is the kernel's verdict, scalar for scalar.
    #[test]
    fn the_null_fill_admits_and_the_bank_is_the_kernels_verdict() {
        let bytes = wire(witness(3, ZERO_VECTOR), request(3));
        let mut bank = [0_u8; scoring_row_bank_bytes_v1(3)];
        let admitted =
            evaluate_scoring_dealer_row_v1(&bytes, &mut bank).expect("the null fill is admitted");
        let expected = potential(parameters(3), &ZERO_VECTOR).expect("potential");
        assert_eq!(admitted.dealer_pays, 0);
        assert_eq!(admitted.dealer_receives, 0);
        let scalar = |index: usize| {
            u64::from_le_bytes(bank[index * 8..index * 8 + 8].try_into().expect("scalar"))
        };
        assert_eq!(scalar(0), 0);
        assert_eq!(scalar(1), 0);
        assert_eq!(scalar(2), expected.minimum);
        assert_eq!(scalar(3), expected.cost);
        assert_eq!(scalar(4), expected.minimum);
        assert_eq!(scalar(5), expected.cost);
        let schedule = prices_of(parameters(3), &ZERO_VECTOR).expect("prices");
        for outcome in 0..3 {
            assert_eq!(scalar(SCORING_ROW_FIXED_SCALARS_V1 + outcome), 0);
            assert_eq!(
                scalar(SCORING_ROW_FIXED_SCALARS_V1 + 3 + outcome),
                schedule[outcome]
            );
        }
    }

    /// The arm and the route are one implementation: whatever `admit_fill`
    /// says on the same inputs is what the bank carries.
    #[test]
    fn the_arm_agrees_with_the_kernel_the_route_links() {
        let mut inventory = ZERO_VECTOR;
        inventory[1] = 4_000;
        inventory[2] = 9_000;
        // R1 holds the minimum at zero, so the Dealer receives on a coordinate
        // that is not the minimum: index 0 stays empty and stays the minimum.
        let mut receive = ZERO_VECTOR;
        receive[1] = 2_500;
        let mut next = inventory;
        next[1] += 2_500;
        let mut row = request(3);
        row.receive = receive;
        row.prices = prices_of(parameters(3), &next).expect("prices");
        let bytes = wire(witness(3, inventory), row);
        let mut bank = [0_u8; scoring_row_bank_bytes_v1(3)];
        let admitted = evaluate_scoring_dealer_row_v1(&bytes, &mut bank).expect("admitted");
        let direct = admit_fill(
            parameters(3),
            &inventory,
            &receive,
            &ZERO_VECTOR,
            &row.prices,
        )
        .expect("the kernel admits");
        assert_eq!(admitted, direct);
    }

    /// A price vector that is a valid simplex but is not the Dealer's price
    /// refuses `OffSchedule`, and the bank is untouched.
    #[test]
    fn a_price_off_the_schedule_refuses_by_name_and_writes_nothing() {
        let mut row = request(2);
        let mut prices = prices_of(parameters(2), &ZERO_VECTOR).expect("prices");
        // Move one price unit from one coordinate to the other: still a
        // simplex at the scale, still every coordinate positive, and outside
        // the sealed tolerance of one on both coordinates.
        prices[0] += 4;
        prices[1] -= 4;
        row.prices = prices;
        let bytes = wire(witness(2, ZERO_VECTOR), row);
        let mut bank = [0xab_u8; scoring_row_bank_bytes_v1(2)];
        assert_eq!(
            evaluate_scoring_dealer_row_v1(&bytes, &mut bank),
            Err(ScoringAcceleratorRefusalV1::Rule(
                ScoringRefusal::OffSchedule
            ))
        );
        assert_eq!(
            ScoringAcceleratorRefusalV1::Rule(ScoringRefusal::OffSchedule).refusal_name(),
            "scoring:OffSchedule"
        );
        assert!(bank.iter().all(|byte| *byte == 0xab));
    }

    /// A fill that would leave the Dealer holding a complete set refuses R1.
    #[test]
    fn a_fill_that_leaves_a_complete_set_refuses_not_normalized() {
        let mut inventory = ZERO_VECTOR;
        inventory[1] = 1_000;
        let mut receive = ZERO_VECTOR;
        receive[0] = 500;
        let mut next = inventory;
        next[0] += 500;
        let mut row = request(2);
        row.receive = receive;
        row.prices = prices_of(parameters(2), &next).expect("prices");
        let bytes = wire(witness(2, inventory), row);
        let mut bank = [0_u8; scoring_row_bank_bytes_v1(2)];
        assert_eq!(
            evaluate_scoring_dealer_row_v1(&bytes, &mut bank),
            Err(ScoringAcceleratorRefusalV1::Rule(
                ScoringRefusal::NotNormalized
            ))
        );
    }

    /// The witness and the request must describe ONE row.
    #[test]
    fn two_halves_of_different_rows_refuse_join() {
        let mut row = request(3);
        row.expected_fund_revision = 4;
        let bytes = wire(witness(3, ZERO_VECTOR), row);
        let mut bank = [0_u8; scoring_row_bank_bytes_v1(3)];
        assert_eq!(
            evaluate_scoring_dealer_row_v1(&bytes, &mut bank),
            Err(ScoringAcceleratorRefusalV1::Join)
        );
        let mut row = request(3);
        row.dealer_id = [11; 32];
        let bytes = wire(witness(3, ZERO_VECTOR), row);
        assert_eq!(
            evaluate_scoring_dealer_row_v1(&bytes, &mut bank),
            Err(ScoringAcceleratorRefusalV1::Join)
        );
    }

    /// A wire of the wrong width is the transport's refusal, not the rule's.
    #[test]
    fn a_short_wire_refuses_transport_and_a_narrow_bank_refuses_bank() {
        let bytes = wire(witness(3, ZERO_VECTOR), request(3));
        let mut bank = [0_u8; scoring_row_bank_bytes_v1(3)];
        assert_eq!(
            evaluate_scoring_dealer_row_v1(&bytes[..bytes.len() - 1], &mut bank),
            Err(ScoringAcceleratorRefusalV1::Transport)
        );
        let mut narrow = [0_u8; scoring_row_bank_bytes_v1(2)];
        assert_eq!(
            evaluate_scoring_dealer_row_v1(&bytes, &mut narrow),
            Err(ScoringAcceleratorRefusalV1::Bank)
        );
    }
}
