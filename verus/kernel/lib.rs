//! Verus-facing shadow obligations for `crates/clutch-kernel`.
//!
//! This file is deliberately a small mathematical shadow, not a second
//! executable implementation.  The erased Rust kernel remains the semantic
//! owner.  It uses only bounded values and names the obligations that must be
//! discharged against a pinned Verus toolchain before a verification claim is
//! made.

use vstd::prelude::*;

verus! {
    pub open spec fn ceil_div(numerator: nat, denominator: nat) -> nat
        recommends denominator > 0
    {
        let quotient = numerator / denominator;
        let remainder = numerator % denominator;
        if remainder == 0 { quotient } else { quotient + 1 }
    }

    pub open spec fn weighted_numerator(
        supply: Seq<nat>,
        weights: Seq<nat>,
    ) -> nat
        recommends supply.len() == weights.len()
        decreases supply.len()
    {
        if supply.len() == 0 {
            0
        } else {
            supply[0] * weights[0]
                + weighted_numerator(supply.subrange(1, supply.len()),
                                     weights.subrange(1, weights.len()))
        }
    }

    pub open spec fn solvent(
        collateral: nat,
        supply: Seq<nat>,
        weights: Seq<nat>,
        denominator: nat,
    ) -> bool
        recommends supply.len() == weights.len(), denominator > 0
    {
        collateral * denominator >= weighted_numerator(supply, weights)
    }

    /// Exact obligation for a complete split: collateral and every claim
    /// increase by the same quantity. The executable method additionally
    /// checks each u64 operation before mutation.
    pub open spec fn split_preserves_solvent(
        collateral: nat,
        supply: Seq<nat>,
        weights: Seq<nat>,
        denominator: nat,
        quantity: nat,
    ) -> bool {
        solvent(collateral, supply, weights, denominator) ==> solvent(
                collateral + quantity,
                supply.map(|_index: int, value: nat| value + quantity),
                weights,
                denominator,
            )
    }

    pub proof fn ceil_div_covers(
        numerator: nat,
        denominator: nat,
    )
        requires denominator > 0
        ensures ceil_div(numerator, denominator) * denominator >= numerator
    {
        let quotient = numerator / denominator;
        let remainder = numerator % denominator;
        if remainder == 0 {
            assert(numerator == quotient * denominator);
        } else {
            assert(numerator == quotient * denominator + remainder);
            assert(remainder < denominator);
            assert((quotient + 1) * denominator >= numerator);
        }
    }
}
