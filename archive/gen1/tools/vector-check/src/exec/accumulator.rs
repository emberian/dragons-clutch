//! `clutch-accumulator`: the summary algebra (S2) and the window state machine
//! (S8, a surface `VECTOR_SPINE_PROPOSAL.md` §2.4 does not map).

use clutch_accumulator::{
    CoveragePolicy, FeedIdentity, Grid, Observation, StatisticError, SummaryError,
    WindowAccumulator, WindowDomain, WindowError, WindowPhase, WindowResult,
};

use super::*;
use crate::json::Value;
use crate::taxonomy::{Observed, Refusal};

pub fn summary_code(error: SummaryError) -> Refusal {
    let (code, variant) = match error {
        SummaryError::InvalidGrid => (2048, "InvalidGrid"),
        SummaryError::InvalidObservation => (2047, "InvalidObservation"),
        SummaryError::BucketOverflow => (1003, "BucketOverflow"),
        SummaryError::SpanTooLarge => (8002, "SpanTooLarge"),
        SummaryError::ArithmeticOverflow => (1001, "ArithmeticOverflow"),
        SummaryError::MismatchedGrid => (4012, "MismatchedGrid"),
        SummaryError::NonAdjacent => (3010, "NonAdjacent"),
        SummaryError::MalformedSummary => (2054, "MalformedSummary"),
    };
    Refusal::new(code, "accumulator", variant)
}

pub fn statistic_code(error: StatisticError) -> Refusal {
    let (code, variant) = match error {
        StatisticError::NoAcceptedCoverage => (6002, "NoAcceptedCoverage"),
        StatisticError::UnsupportedPredicate => (9002, "UnsupportedPredicate"),
        StatisticError::AmbiguousDenominator => (1005, "AmbiguousDenominator"),
        StatisticError::ArithmeticOverflow => (1001, "ArithmeticOverflow"),
    };
    Refusal::new(code, "accumulator", variant)
}

pub fn window_code(error: WindowError) -> Refusal {
    let (code, variant) = match error {
        WindowError::ZeroIdentity => (4009, "ZeroIdentity"),
        WindowError::UnversionedIdentity => (2046, "UnversionedIdentity"),
        WindowError::InvalidRange => (2073, "InvalidRange"),
        WindowError::InvalidMaturity => (2074, "InvalidMaturity"),
        WindowError::UnknownCoveragePolicy => (2075, "UnknownCoveragePolicy"),
        WindowError::InvalidPolicyParameter => (2076, "InvalidPolicyParameter"),
        WindowError::MismatchedGrid => (4012, "MismatchedGrid"),
        WindowError::MismatchedFeed => (4019, "MismatchedFeed"),
        WindowError::MismatchedCoveragePolicy => (4020, "MismatchedCoveragePolicy"),
        WindowError::MismatchedGeneration => (4021, "MismatchedGeneration"),
        WindowError::MismatchedMaturity => (4022, "MismatchedMaturity"),
        WindowError::WrongWindow => (4023, "WrongWindow"),
        WindowError::NonContiguous => (3008, "NonContiguous"),
        WindowError::RangeOverflow => (3012, "RangeOverflow"),
        WindowError::NonMonotoneCursor => (3013, "NonMonotoneCursor"),
        WindowError::ObservationAfterSeal => (3007, "ObservationAfterSeal"),
        WindowError::IncompleteDomain => (6005, "IncompleteDomain"),
        WindowError::NotMature => (3004, "NotMature"),
        WindowError::AlreadySealed => (3006, "AlreadySealed"),
        WindowError::NotSealed => (3005, "NotSealed"),
        WindowError::CoverageRefused => (6006, "CoverageRefused"),
        WindowError::MalformedResult => (2054, "MalformedResult"),
        WindowError::Summary(inner) => return summary_code(inner),
    };
    Refusal::new(code, "accumulator", variant)
}

fn read_grid(value: &Value) -> Result<Grid, String> {
    let family_id = u32::try_from(small_field(value, "family_id")?)
        .map_err(|_| "family_id out of range".to_string())?;
    let version = u16::try_from(small_field(value, "version")?)
        .map_err(|_| "version out of range".to_string())?;
    Grid::new(family_id, version, u64_field(value, "bucket_seconds")?)
        .map_err(|error| format!("grid is not constructible: {error:?}"))
}

fn read_coverage(value: &Value) -> Result<CoveragePolicy, String> {
    match str_field(value, "policy")? {
        "complete-required" => Ok(CoveragePolicy::COMPLETE_REQUIRED),
        "bounded-gaps" => CoveragePolicy::bounded_gaps(u64_field(value, "max_missing_buckets")?)
            .map_err(|error| format!("coverage policy is not constructible: {error:?}")),
        other => Err(format!("ENUM-1: unknown coverage policy {other:?}")),
    }
}

fn read_domain(value: &Value) -> Result<WindowDomain, String> {
    let feed_value = field(value, "feed")?;
    let feed = FeedIdentity::new(
        read_hash32(field(feed_value, "source_adapter_id")?)?,
        read_hash32(field(feed_value, "feed_spec_id")?)?,
        u32::try_from(small_field(feed_value, "source_version")?)
            .map_err(|_| "source_version out of range".to_string())?,
        u32::try_from(small_field(feed_value, "evaluator_version")?)
            .map_err(|_| "evaluator_version out of range".to_string())?,
    )
    .map_err(|error| format!("feed identity is not constructible: {error:?}"))?;
    WindowDomain::new(
        feed,
        read_grid(field(value, "grid")?)?,
        u64_field(value, "start_bucket")?,
        u64_field(value, "end_bucket_exclusive")?,
        u64_field(value, "maturity_bucket_exclusive")?,
        u64_field(value, "generation")?,
        read_coverage(field(value, "coverage")?)?,
    )
    .map_err(|error| format!("window domain is not constructible: {error:?}"))
}

pub struct WindowExecutor {
    window: WindowAccumulator,
}

fn render_result(result: &WindowResult) -> Value {
    let coverage = result.coverage();
    let mut pairs = vec![
        ("sealed_cursor", dec(u128::from(result.sealed_cursor()))),
        ("total_buckets", dec(u128::from(coverage.total_buckets()))),
        (
            "accepted_buckets",
            dec(u128::from(coverage.accepted_buckets())),
        ),
        (
            "missing_buckets",
            dec(u128::from(coverage.missing_buckets())),
        ),
        (
            "coverage_state",
            Value::Str(
                match coverage.state() {
                    clutch_accumulator::CoverageState::Empty => "empty",
                    clutch_accumulator::CoverageState::Complete => "complete",
                    clutch_accumulator::CoverageState::Gapped => "gapped",
                }
                .to_string(),
            ),
        ),
    ];
    // INT-4: an interval ships as its exact endpoints, never pre-divided.
    match result.terminal() {
        Ok(interval) => pairs.push((
            "terminal",
            obj(vec![
                ("low", dec(interval.low())),
                ("high", dec(interval.high())),
            ]),
        )),
        Err(error) => {
            let refusal = statistic_code(error);
            pairs.push((
                "terminal_refusal",
                obj(vec![
                    ("code", small(u64::from(refusal.code))),
                    ("variant", Value::Str(refusal.variant)),
                ]),
            ));
        }
    }
    obj(pairs)
}

impl WindowExecutor {
    pub fn open(constructed_by: &str, value: &Value) -> Result<Self, String> {
        if constructed_by != "constructor" {
            return Err(format!(
                "accumulator.window/v1 has only one constructor; constructed_by {constructed_by:?} is not available"
            ));
        }
        Ok(Self {
            window: WindowAccumulator::open(read_domain(field(value, "domain")?)?),
        })
    }
}

impl Executor for WindowExecutor {
    fn apply(&mut self, op: &str, args: &Value) -> Result<Observed, String> {
        match op {
            "observe" => {
                let bucket = u64_field(args, "bucket")?;
                let observation = match str_field(args, "kind")? {
                    "accepted" => Observation::accepted(
                        bucket,
                        u128_field(args, "low")?,
                        u128_field(args, "high")?,
                    ),
                    "missing" => Observation::missing(bucket),
                    other => return Err(format!("ENUM-1: unknown observation kind {other:?}")),
                };
                Ok(match self.window.observe(observation) {
                    Ok(()) => Observed::Ok(Value::Null),
                    Err(error) => Observed::Error(window_code(error)),
                })
            }
            "witness_feed_cursor" => {
                let next = u64_field(args, "next_bucket")?;
                Ok(match self.window.witness_feed_cursor(next) {
                    Ok(()) => Observed::Ok(Value::Null),
                    Err(error) => Observed::Error(window_code(error)),
                })
            }
            "seal" => Ok(match self.window.seal() {
                Ok(()) => Observed::Ok(Value::Null),
                Err(error) => Observed::Error(window_code(error)),
            }),
            "result" => Ok(match self.window.result() {
                Ok(result) => Observed::Ok(render_result(&result)),
                Err(error) => Observed::Error(window_code(error)),
            }),
            "check_domain" => {
                let expected = read_domain(field(args, "domain")?)?;
                Ok(match self.window.result() {
                    Ok(result) => match result.check_domain(&expected) {
                        Ok(()) => Observed::Ok(Value::Null),
                        Err(error) => Observed::Error(window_code(error)),
                    },
                    Err(error) => Observed::Error(window_code(error)),
                })
            }
            other => Err(format!(
                "clutch-accumulator has no window operation {other:?}"
            )),
        }
    }

    fn render_state(&self) -> Value {
        obj(vec![
            ("cursor", dec(u128::from(self.window.cursor()))),
            ("feed_cursor", dec(u128::from(self.window.feed_cursor()))),
            (
                "phase",
                Value::Str(
                    match self.window.phase() {
                        WindowPhase::Open => "open",
                        WindowPhase::Mature => "mature",
                        WindowPhase::Sealed => "sealed",
                    }
                    .to_string(),
                ),
            ),
        ])
    }
}
