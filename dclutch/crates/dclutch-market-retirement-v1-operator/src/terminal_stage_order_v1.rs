//! The six protocol mutations of one Market's terminal sequence, and their sole
//! admissible order, declared exactly once.
//!
//! # Why this is not in the driver
//!
//! It was, and the order was wrong there for the life of three cohorts. The
//! declaration lived in
//! `tools/local-validator/bootstrap/successor/src/terminal_sequence.rs` beside
//! the walk that consumed it, so nothing that could refute it ever read it: the
//! `dclutch-svm-harness` retirement campaign drives the LAST stage only -- four
//! checkpoint packets against a Market seeded with `outstanding_capabilities =
//! 0` and a pre-seeded `SourceClosureReceiptV3` -- and therefore agreed with any
//! order at all. The pair whose order was wrong was executed for the first time
//! on devnet, in cohort-17, on a market that cannot be repaired.
//!
//! This module is the one author. It lives in the operator crate BOTH readers
//! link -- the devnet/loopback driver
//! (`tools/local-validator/bootstrap/successor`) and the ProgramTest harness
//! (`crates/dclutch-svm-harness`) -- and it is in no SBF link's path-dependency
//! closure, so moving it moves no ELF.
//!
//! # The ruling this encodes
//!
//! `DirectCloseCapability` runs BEFORE `ResolutionCloseFund`.
//!
//! Core `CloseCapability` on the Direct entry decodes BOTH physical funding
//! ledgers of the market -- the selected row's own, which it closes, and the one
//! foreign controller's, which it PRESERVES byte for byte and re-states as a
//! `PreservedFundingLedgerV1` poststate. The foreign one is the Resolution
//! dependency ledger, and `ResolutionCloseFund` is what closes it. A sequence
//! that closes the dependency first destroys an input of the stage that
//! preserves it, and the close then refuses on a zero-byte account before it can
//! build a frame.
//!
//! Stated as the invariant rather than as the order: **the stage that preserves
//! a dependency runs before the stage that owns and closes it.** The order below
//! is the only ordering of the six that satisfies it.
//!
//! No program enforces this. Core's kernel admits `CloseCapability` in `Open`,
//! `Terminal` and `Retiring` alike (`dclutch_market::close_capability_child`),
//! Resolution admits `CloseFund` in exactly `(Retiring, Consumed)`
//! (`RESOLUTION_CLOSE_FUND_ADMISSIBLE_PRESTATES_V1`), and the only ordering fact
//! either chain-side state machine can see is `dclutch_market::retire`'s
//! `outstanding_capabilities == 0` -- which orders `CloseCapability` before
//! `AggregateRetirement` and says nothing about `CloseFund`. The order is a
//! host obligation, and this is where the host states it.

use serde::{Deserialize, Serialize};

/// One protocol mutation of the terminal sequence.
///
/// Variant order here is the RUN order; [`TerminalStageV1::ORDERED`] is the same
/// order written out, and [`TerminalStageV1::ordinal`] is a stage's index in it.
/// Nothing else in the tree may state a run order over these six.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub enum TerminalStageV1 {
    /// Core moves the Market from `Terminal` to `Retiring`.
    CoreBeginRetiring,
    /// Trading moves the Direct capability root into its own retiring phase.
    DirectBeginRetiring,
    /// Core `CloseCapability` closes the Direct root, closes the selected
    /// funding ledger, PRESERVES the Resolution dependency ledger, and takes
    /// `outstanding_capabilities` to zero.
    DirectCloseCapability,
    /// Resolution closes the Source subtree and its own dependency funding
    /// ledger -- the one the stage above preserved.
    ResolutionCloseFund,
    /// Trading hands the Custody replay cursor to Core.
    RetirementReplayHandoff,
    /// Core's four checkpoint packets close the aggregate and the Market.
    AggregateRetirement,
}

impl TerminalStageV1 {
    /// The six mutations in their sole admissible order.
    pub const ORDERED: [Self; 6] = [
        Self::CoreBeginRetiring,
        Self::DirectBeginRetiring,
        Self::DirectCloseCapability,
        Self::ResolutionCloseFund,
        Self::RetirementReplayHandoff,
        Self::AggregateRetirement,
    ];

    /// This stage's index in [`TerminalStageV1::ORDERED`].
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::CoreBeginRetiring => 0,
            Self::DirectBeginRetiring => 1,
            Self::DirectCloseCapability => 2,
            Self::ResolutionCloseFund => 3,
            Self::RetirementReplayHandoff => 4,
            Self::AggregateRetirement => 5,
        }
    }

    /// The stage's kebab-case name, identical to its serialized form.
    #[must_use]
    pub const fn kebab(self) -> &'static str {
        match self {
            Self::CoreBeginRetiring => "core-begin-retiring",
            Self::DirectBeginRetiring => "direct-begin-retiring",
            Self::DirectCloseCapability => "direct-close-capability",
            Self::ResolutionCloseFund => "resolution-close-fund",
            Self::RetirementReplayHandoff => "retirement-replay-handoff",
            Self::AggregateRetirement => "aggregate-retirement",
        }
    }
}

/// `ordinal()` is an index into `ORDERED` and the compiler holds it to that.
///
/// The index is bounded by the loop condition and the walk is evaluated at
/// compile time, so an out-of-bounds read here is a build failure and never a
/// runtime panic -- which is the whole reason the check is a `const` block
/// rather than a test.
#[allow(clippy::indexing_slicing)]
const _: () = {
    let mut index = 0;
    while index < TerminalStageV1::ORDERED.len() {
        assert!(TerminalStageV1::ORDERED[index].ordinal() as usize == index);
        index += 1;
    }
};

/// THE RULING, as a compile-time fact rather than a comment: the stage that
/// preserves the Resolution dependency funding ledger precedes the stage that
/// closes it.
const _: () = assert!(
    TerminalStageV1::DirectCloseCapability.ordinal()
        < TerminalStageV1::ResolutionCloseFund.ordinal(),
    "Core CloseCapability preserves the Resolution dependency funding ledger; \
     Resolution CloseFund closes it. The preserver runs first."
);

/// Why a sequence of terminal stages is not admissible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalStageOrderErrorV1 {
    /// The sequence is not the exact ordered six.
    NotTheOrderedSix,
    /// `ResolutionCloseFund` is present or planned without
    /// `DirectCloseCapability` in front of it. The Direct close decodes the
    /// Resolution dependency funding ledger to build its
    /// `CapabilityFundingHeaderV2` and to preserve it; once `CloseFund` has run,
    /// that account is gone and the Direct close can never be built for this
    /// market again.
    ResolutionCloseFundBeforeDirectClose,
    /// Some other stage appeared after a hole in the ordered prefix.
    LaterStageAfterMissingPrefix,
}

impl TerminalStageOrderErrorV1 {
    /// The refusal an operator reads, naming the cause rather than the symptom.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::NotTheOrderedSix => {
                "terminal stage sequence is not the exact ordered six-stage sequence"
            }
            Self::ResolutionCloseFundBeforeDirectClose => {
                "Resolution CloseFund would run before Core CloseCapability closes the Direct \
                 capability. CloseFund closes the Resolution dependency funding ledger that the \
                 Direct close decodes to build its CapabilityFundingHeaderV2 and preserves \
                 unchanged, so this order destroys the Direct close's own input and the \
                 capability can never be closed on this market again"
            }
            Self::LaterStageAfterMissingPrefix => {
                "terminal stages contained a later stage after a missing ordered prefix"
            }
        }
    }
}

/// Authenticate that a stage list is exactly [`TerminalStageV1::ORDERED`].
///
/// # Errors
///
/// [`TerminalStageOrderErrorV1::ResolutionCloseFundBeforeDirectClose`] when the
/// two stages the ruling orders appear the wrong way round, and
/// [`TerminalStageOrderErrorV1::NotTheOrderedSix`] for every other disagreement.
/// The specific cause is checked first so an operator reads the reason and not
/// the shape.
pub fn authenticate_terminal_stage_order_v1(
    stages: &[TerminalStageV1],
) -> Result<(), TerminalStageOrderErrorV1> {
    let close_fund = stages
        .iter()
        .position(|stage| *stage == TerminalStageV1::ResolutionCloseFund);
    let direct_close = stages
        .iter()
        .position(|stage| *stage == TerminalStageV1::DirectCloseCapability);
    match (close_fund, direct_close) {
        (Some(_), None) => {
            return Err(TerminalStageOrderErrorV1::ResolutionCloseFundBeforeDirectClose);
        }
        (Some(fund), Some(direct)) if fund < direct => {
            return Err(TerminalStageOrderErrorV1::ResolutionCloseFundBeforeDirectClose);
        }
        _ => {}
    }
    if stages != TerminalStageV1::ORDERED {
        return Err(TerminalStageOrderErrorV1::NotTheOrderedSix);
    }
    Ok(())
}

/// Authenticate a DURABLE PREFIX of the sequence: the stages a run has already
/// planned or finalized, in the order it planned them.
///
/// A resumable driver writes one journal per stage and refuses to open a later
/// stage over a hole. That refusal is generic; this one is not. A prefix that
/// carries `ResolutionCloseFund` without `DirectCloseCapability` in front of it
/// is the exact fault cohort-17 met on devnet, and it is named.
///
/// # Errors
///
/// [`TerminalStageOrderErrorV1::ResolutionCloseFundBeforeDirectClose`] when the
/// dependency's owner ran first, and
/// [`TerminalStageOrderErrorV1::LaterStageAfterMissingPrefix`] for any other
/// hole.
pub fn authenticate_terminal_stage_prefix_v1(
    present: &[TerminalStageV1],
) -> Result<(), TerminalStageOrderErrorV1> {
    let mut missing = None;
    for stage in TerminalStageV1::ORDERED {
        let held = present.contains(&stage);
        match (held, missing) {
            (true, Some(hole)) => {
                if stage == TerminalStageV1::ResolutionCloseFund
                    && hole == TerminalStageV1::DirectCloseCapability
                {
                    return Err(TerminalStageOrderErrorV1::ResolutionCloseFundBeforeDirectClose);
                }
                return Err(TerminalStageOrderErrorV1::LaterStageAfterMissingPrefix);
            }
            (false, None) => missing = Some(stage),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    /// The ruled adjacency, read off the declaration rather than retyped.
    #[test]
    fn the_direct_close_precedes_the_resolution_close() {
        assert_eq!(TerminalStageV1::DirectCloseCapability.ordinal(), 2);
        assert_eq!(TerminalStageV1::ResolutionCloseFund.ordinal(), 3);
        assert!(
            TerminalStageV1::DirectCloseCapability.ordinal()
                < TerminalStageV1::ResolutionCloseFund.ordinal(),
            "the stage that preserves the dependency ledger runs before its owner closes it"
        );
    }

    /// The order is a permutation: six distinct stages, ordinals 0..6.
    #[test]
    fn the_ordered_six_are_distinct_and_dense() {
        let mut seen = [false; 6];
        for stage in TerminalStageV1::ORDERED {
            let slot = usize::from(stage.ordinal());
            assert!(!seen[slot], "two stages claim ordinal {slot}");
            seen[slot] = true;
        }
        assert!(seen.into_iter().all(|held| held));
    }

    /// The hostile the ruling exists for: the dependency's owner first.
    #[test]
    fn close_fund_before_the_direct_close_refuses_by_name() {
        let hostile = [
            TerminalStageV1::CoreBeginRetiring,
            TerminalStageV1::DirectBeginRetiring,
            TerminalStageV1::ResolutionCloseFund,
            TerminalStageV1::DirectCloseCapability,
            TerminalStageV1::RetirementReplayHandoff,
            TerminalStageV1::AggregateRetirement,
        ];
        assert_eq!(
            authenticate_terminal_stage_order_v1(&hostile),
            Err(TerminalStageOrderErrorV1::ResolutionCloseFundBeforeDirectClose)
        );
        assert!(
            TerminalStageOrderErrorV1::ResolutionCloseFundBeforeDirectClose
                .message()
                .contains("destroys the Direct close's own input")
        );
        authenticate_terminal_stage_order_v1(&TerminalStageV1::ORDERED).expect("the ruled order");
    }

    /// The same accusation off a durable prefix, which is what a resumed run
    /// actually holds: cohort-17's journal directory in one line.
    #[test]
    fn a_durable_prefix_that_closed_the_fund_first_refuses_by_name() {
        let cohort_seventeen = [
            TerminalStageV1::CoreBeginRetiring,
            TerminalStageV1::DirectBeginRetiring,
            TerminalStageV1::ResolutionCloseFund,
        ];
        assert_eq!(
            authenticate_terminal_stage_prefix_v1(&cohort_seventeen),
            Err(TerminalStageOrderErrorV1::ResolutionCloseFundBeforeDirectClose)
        );
        authenticate_terminal_stage_prefix_v1(&[
            TerminalStageV1::CoreBeginRetiring,
            TerminalStageV1::DirectBeginRetiring,
            TerminalStageV1::DirectCloseCapability,
        ])
        .expect("the ruled prefix");
        assert_eq!(
            authenticate_terminal_stage_prefix_v1(&[
                TerminalStageV1::CoreBeginRetiring,
                TerminalStageV1::AggregateRetirement,
            ]),
            Err(TerminalStageOrderErrorV1::LaterStageAfterMissingPrefix)
        );
    }

    /// A shape disagreement that is NOT the ruled pair reads as a shape
    /// disagreement, so the named cause cannot be reached by accident.
    #[test]
    fn an_unrelated_reorder_is_not_the_named_cause() {
        let hostile = [
            TerminalStageV1::DirectBeginRetiring,
            TerminalStageV1::CoreBeginRetiring,
            TerminalStageV1::DirectCloseCapability,
            TerminalStageV1::ResolutionCloseFund,
            TerminalStageV1::RetirementReplayHandoff,
            TerminalStageV1::AggregateRetirement,
        ];
        assert_eq!(
            authenticate_terminal_stage_order_v1(&hostile),
            Err(TerminalStageOrderErrorV1::NotTheOrderedSix)
        );
    }

    /// The serialized form is the kebab name, in both directions.
    #[test]
    fn the_wire_name_and_the_kebab_name_are_one_string() {
        for stage in TerminalStageV1::ORDERED {
            let wire = serde_json::to_string(&stage).expect("stage serializes");
            assert_eq!(wire, format!("\"{}\"", stage.kebab()));
            let back: TerminalStageV1 = serde_json::from_str(&wire).expect("stage round-trips");
            assert_eq!(back, stage);
        }
    }
}
