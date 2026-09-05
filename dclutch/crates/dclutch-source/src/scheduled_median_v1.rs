//! The tolerated `OddScheduledMedian` schedule, and the exact median itself.
//!
//! `docs/research/CHAIN_STATE_SOURCES_2026_08.md` §6.4 chose the odd scheduled
//! median as the family-general mechanism for a chain-state price Source and, in
//! the same paragraph, recorded the implementation hazard: the statistic
//! required **strict equal cadence**, so a submitter that missed its schedule
//! second under congestion broke the window and the statistic refused. That was
//! a provisional judgement with no measurement, which `AGENTS.md` admits only
//! with a lifting plan.
//!
//! This module is the lift. `WindowSpecV1` gained one coordinate,
//! `cadence_tolerance_seconds`, and the schedule widens each nominal slot into
//! an admission interval of that half-width. The bound `2 · τ < cadence` is
//! what preserves every property the median was chosen for, and
//! `formal/dclutch-semantics/DClutchSemantics/SourceScheduledMedianV1.lean`
//! proves it: admission windows are pairwise disjoint, so one observation
//! cannot answer two scheduled positions; admitted times strictly increase, so
//! the evaluator's ordering check is implied rather than assumed; and
//! consecutive admitted samples stay at least `cadence − 2τ` apart, which is
//! the real, stated cost of the lift against §5.1's atomicity bound.
//!
//! The cadence is derived from the **window**, never from the samples. If the
//! samples determined the cadence, an attacker who chose when to submit would
//! be choosing the schedule they are supposed to be answering.
//!
//! A zero tolerance admits exactly the strict cadence, so every window written
//! before this coordinate existed means precisely what it always meant.

use super::{Error, Result, WindowKind, WindowSpecV1};

pub use super::generated_scheduled_median_v1::{
    CADENCE_TOLERANCE_LIFTING_PLAN_ID_V1, CADENCE_TOLERANCE_LIFTING_PLAN_PREIMAGE_V1,
    MEDIAN_CASES_V1, SCHEDULE_CASES_V1, SCHEDULED_MEDIAN_CORPUS_MAX_SAMPLES_V1,
    WINDOW_SPEC_CADENCE_TOLERANCE_OFFSET_V1, WINDOW_SPEC_CADENCE_TOLERANCE_TAIL_RESERVED_BYTES_V1,
    WINDOW_SPEC_CADENCE_TOLERANCE_TAIL_RESERVED_OFFSET_V1,
};

/// Minimum sample count admitted by an odd scheduled median (§6.4).
///
/// Mathematical: two samples have no strict majority, so an attacker holding
/// one of them already moves the answer.
pub const MINIMUM_MEDIAN_SAMPLES_V1: usize = 3;

/// One Lean-emitted tolerated-schedule case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScheduleCaseV1 {
    /// Closed lower window bound and first nominal slot.
    pub start_unix_seconds: i64,
    /// Exact derived cadence between nominal slots.
    pub cadence_seconds: i64,
    /// Committed slot count.
    pub count: usize,
    /// Admission half-width around each nominal slot.
    pub tolerance_seconds: i64,
    /// How many observation times the case actually offers.
    pub offered: usize,
    /// Offered observation times, zero-padded to the corpus width.
    pub times: [i64; SCHEDULED_MEDIAN_CORPUS_MAX_SAMPLES_V1],
    /// Whether the schedule admits every offered time at its own position.
    pub admitted: bool,
}

/// One Lean-emitted exact-median case over admitted atoms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MedianCaseV1 {
    /// How many atoms the case actually offers.
    pub count: usize,
    /// Offered atoms, zero-padded to the corpus width.
    pub atoms: [i128; SCHEDULED_MEDIAN_CORPUS_MAX_SAMPLES_V1],
    /// The one value the rank selection admits.
    pub expected: i128,
}

/// The committed schedule an odd scheduled median is answered against.
///
/// Every field is derived from the window and the committed sample count. No
/// observation contributes to the schedule it is checked against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScheduledMedianScheduleV1 {
    start_unix_seconds: i64,
    cadence_seconds: i64,
    count: usize,
    tolerance_seconds: i64,
}

impl ScheduledMedianScheduleV1 {
    /// Derive the schedule a window commits to for exactly `count` samples.
    ///
    /// Refuses a window that is not a scheduled interval, a count below the
    /// mathematical minimum, a span the count does not divide exactly, and a
    /// tolerance that is not strictly below half the derived cadence.
    pub fn derive(window: WindowSpecV1, count: usize) -> Result<Self> {
        if window.kind() != WindowKind::ScheduledInterval || count < MINIMUM_MEDIAN_SAMPLES_V1 {
            return Err(Error::NonCanonicalStatistic);
        }
        let intervals = i64::try_from(count.saturating_sub(1))
            .map_err(|_| Error::InvalidObservationSchedule)?;
        let span = window
            .end_unix_seconds()
            .checked_sub(window.start_unix_seconds())
            .ok_or(Error::ArithmeticOverflow)?;
        if intervals == 0 || span.rem_euclid(intervals) != 0 {
            return Err(Error::InvalidObservationSchedule);
        }
        let cadence_seconds = span.div_euclid(intervals);
        if cadence_seconds <= 0 {
            return Err(Error::InvalidObservationSchedule);
        }
        let tolerance_seconds = i64::from(window.cadence_tolerance_seconds());
        let doubled = tolerance_seconds
            .checked_mul(2)
            .ok_or(Error::ArithmeticOverflow)?;
        if doubled >= cadence_seconds {
            return Err(Error::CadenceToleranceExceedsSchedule);
        }
        Ok(Self {
            start_unix_seconds: window.start_unix_seconds(),
            cadence_seconds,
            count,
            tolerance_seconds,
        })
    }

    /// Nominal time of one scheduled position.
    pub fn slot_unix_seconds(self, index: usize) -> Result<i64> {
        if index >= self.count {
            return Err(Error::InvalidObservationSchedule);
        }
        let position = i64::try_from(index).map_err(|_| Error::InvalidObservationSchedule)?;
        self.cadence_seconds
            .checked_mul(position)
            .and_then(|offset| self.start_unix_seconds.checked_add(offset))
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Admit one observation time at one scheduled position.
    pub fn admit(self, index: usize, unix_seconds: i64) -> Result<()> {
        let slot = self.slot_unix_seconds(index)?;
        let earliest = slot
            .checked_sub(self.tolerance_seconds)
            .ok_or(Error::ArithmeticOverflow)?;
        let latest = slot
            .checked_add(self.tolerance_seconds)
            .ok_or(Error::ArithmeticOverflow)?;
        if unix_seconds < earliest || unix_seconds > latest {
            return Err(Error::InvalidObservationSchedule);
        }
        Ok(())
    }

    /// Exact derived cadence.
    pub const fn cadence_seconds(self) -> i64 {
        self.cadence_seconds
    }

    /// Admission half-width around each nominal slot.
    pub const fn tolerance_seconds(self) -> i64 {
        self.tolerance_seconds
    }

    /// Guaranteed minimum separation between consecutive admitted samples.
    ///
    /// This is the quantity §5.1's atomicity argument now runs against, and it
    /// is positive by construction.
    pub fn minimum_separation_seconds(self) -> Result<i64> {
        self.tolerance_seconds
            .checked_mul(2)
            .and_then(|doubled| self.cadence_seconds.checked_sub(doubled))
            .ok_or(Error::ArithmeticOverflow)
    }
}

/// The exact rank selection, over any accessor of the admitted atoms.
///
/// This is the one median implementation in the crate; both the borrowed
/// observation path and the normalized provider-evidence path call it, so they
/// cannot drift apart. It performs no division, no allocation and no sort: it
/// tests every candidate against the two counts the Lean `Selects` predicate
/// names, and `selects_unique` proves at most one candidate can pass.
pub(crate) fn exact_median_by(count: usize, atom_at: impl Fn(usize) -> i128) -> Result<i128> {
    let rank = count / 2;
    let mut index = 0usize;
    while index < count {
        let candidate = atom_at(index);
        let mut below = 0usize;
        let mut equal = 0usize;
        let mut other = 0usize;
        while other < count {
            let value = atom_at(other);
            if value < candidate {
                below = below.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            } else if value == candidate {
                equal = equal.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
            other = other.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        let after_equal = below.checked_add(equal).ok_or(Error::ArithmeticOverflow)?;
        if below <= rank && rank < after_equal {
            return Ok(candidate);
        }
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Err(Error::InvalidObservationSchedule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContentId, WINDOW_SPEC_BYTES};

    fn window(start: i64, end: i64, tolerance: u32) -> WindowSpecV1 {
        WindowSpecV1::new(
            ContentId::new([1u8; 32]).expect("source"),
            WindowKind::ScheduledInterval,
            start,
            end,
            600,
            30,
            ContentId::new([2u8; 32]).expect("schedule"),
        )
        .expect("window")
        .tolerating_cadence(tolerance)
        .expect("tolerance")
    }

    #[test]
    fn the_lean_schedule_corpus_decides_every_case_the_same_way() {
        for case in SCHEDULE_CASES_V1 {
            let tolerance =
                u32::try_from(case.tolerance_seconds).expect("Lean tolerance fits the wire");
            let span = case
                .cadence_seconds
                .checked_mul(i64::try_from(case.count - 1).expect("count"))
                .expect("span");
            let spec = window(
                case.start_unix_seconds,
                case.start_unix_seconds + span,
                tolerance,
            );
            let observed =
                ScheduledMedianScheduleV1::derive(spec, case.offered).and_then(|schedule| {
                    for index in 0..case.offered {
                        let offered = case
                            .times
                            .get(index)
                            .copied()
                            .ok_or(Error::InvalidObservationSchedule)?;
                        schedule.admit(index, offered)?;
                    }
                    Ok(())
                });
            assert_eq!(
                observed.is_ok(),
                case.admitted,
                "Lean and Rust disagreed on {case:?}"
            );
        }
    }

    #[test]
    fn the_lean_median_corpus_selects_the_same_value() {
        for case in MEDIAN_CASES_V1 {
            let atom_at = |index: usize| case.atoms.get(index).copied().unwrap_or_default();
            assert_eq!(
                exact_median_by(case.count, atom_at),
                Ok(case.expected),
                "median disagreed on {case:?}"
            );
            let mut reversed = case.atoms;
            reversed
                .get_mut(..case.count)
                .expect("offered prefix")
                .reverse();
            let reversed_at = |index: usize| reversed.get(index).copied().unwrap_or_default();
            assert_eq!(
                exact_median_by(case.count, reversed_at),
                Ok(case.expected),
                "the median depended on submission order for {case:?}"
            );
        }
    }

    #[test]
    fn a_zero_tolerance_is_exactly_the_strict_cadence() {
        let strict = ScheduledMedianScheduleV1::derive(window(1000, 1120, 0), 3).expect("schedule");
        assert_eq!(strict.tolerance_seconds(), 0);
        for offset in [-1i64, 1] {
            assert_eq!(
                strict.admit(1, 1060 + offset),
                Err(Error::InvalidObservationSchedule)
            );
        }
        assert_eq!(strict.admit(1, 1060), Ok(()));
        assert_eq!(strict.minimum_separation_seconds(), Ok(60));
    }

    #[test]
    fn the_tolerance_bound_is_enforced_against_the_derived_cadence() {
        // Cadence 60: half is 30, so 29 is the widest admissible tolerance.
        assert!(ScheduledMedianScheduleV1::derive(window(1000, 1120, 29), 3).is_ok());
        assert_eq!(
            ScheduledMedianScheduleV1::derive(window(1000, 1120, 30), 3),
            Err(Error::CadenceToleranceExceedsSchedule)
        );
        // The same window with more samples has a shorter cadence, and the same
        // tolerance stops being admissible. The bound is against the *schedule*,
        // which is why it cannot be checked by the window alone.
        assert_eq!(
            ScheduledMedianScheduleV1::derive(window(1000, 1120, 29), 5),
            Err(Error::CadenceToleranceExceedsSchedule)
        );
    }

    #[test]
    fn no_instant_answers_two_scheduled_positions() {
        let schedule =
            ScheduledMedianScheduleV1::derive(window(1000, 1120, 29), 3).expect("schedule");
        for instant in 971..=1149 {
            let admitted = (0..3)
                .filter(|index| schedule.admit(*index, instant).is_ok())
                .count();
            assert!(
                admitted <= 1,
                "instant {instant} answered {admitted} scheduled positions"
            );
        }
        assert_eq!(schedule.minimum_separation_seconds(), Ok(2));
    }

    #[test]
    fn a_terminal_window_refuses_a_cadence_tolerance_outright() {
        let terminal = WindowSpecV1::new(
            ContentId::new([1u8; 32]).expect("source"),
            WindowKind::Terminal,
            1000,
            1300,
            600,
            30,
            ContentId::new([2u8; 32]).expect("schedule"),
        )
        .expect("window");
        assert_eq!(terminal.tolerating_cadence(5), Err(Error::InvalidWindow));
        assert_eq!(terminal.cadence_tolerance_seconds(), 0);
    }

    #[test]
    fn the_tolerance_survives_the_wire_and_its_reserved_neighbours_do_not() {
        let tolerated = window(1000, 1120, 17);
        let bytes = tolerated.to_bytes();
        assert_eq!(
            bytes.get(
                WINDOW_SPEC_CADENCE_TOLERANCE_OFFSET_V1
                    ..WINDOW_SPEC_CADENCE_TOLERANCE_OFFSET_V1 + 4
            ),
            Some(17u32.to_le_bytes().as_slice())
        );
        assert_eq!(WindowSpecV1::decode(&bytes), Ok(tolerated));
        for offset in WINDOW_SPEC_CADENCE_TOLERANCE_TAIL_RESERVED_OFFSET_V1
            ..WINDOW_SPEC_CADENCE_TOLERANCE_TAIL_RESERVED_OFFSET_V1
                + WINDOW_SPEC_CADENCE_TOLERANCE_TAIL_RESERVED_BYTES_V1
        {
            let mut hostile = bytes;
            *hostile.get_mut(offset).expect("reserved byte") = 1;
            assert_eq!(
                WindowSpecV1::decode(&hostile),
                Err(Error::NonCanonicalReservedBytes)
            );
        }
        assert_eq!(bytes.len(), WINDOW_SPEC_BYTES);
    }

    #[test]
    fn a_median_with_no_selecting_candidate_refuses_rather_than_guessing() {
        assert_eq!(
            exact_median_by(0, |_| 0),
            Err(Error::InvalidObservationSchedule)
        );
    }
}
