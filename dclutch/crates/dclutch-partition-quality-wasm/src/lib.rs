//! Thin browser ABI over the authoritative partition-quality gate.
//!
//! WHY THIS EXISTS. `apps/dclutch-web` had **zero** occurrences of
//! `max_cell_share_bps`, `founding_band`, or volatility-as-input, so a market
//! founded through the create wizard was never measured by the gate that
//! refuses degenerate partitions. What the wizard ran instead was a strictly
//! weaker unit-sanity check with a provisional constant of its own, and that
//! file said so in its own words, including the lifting plan: *"delete it, and
//! call `require_interesting_partition_v1` with the market's own founding band
//! once `dclutch-product-compiler` reaches the browser."* This is the compiler
//! reaching the browser.
//!
//! The refusal to reimplement the triangular displacement in TypeScript was
//! right, and it is why this crate exists rather than a second model: a mirror
//! is not fixed by another mirror.
//!
//! This crate owns no measurement. It carries a belief and a partition's cuts
//! into `require_interesting_partition_v1` and carries that gate's own report,
//! or its own refusal by name, back out. **Both members of the belief family
//! are carried**, because a belief and its model are one decision and a
//! boundary that could only express the spot-shaped one would quietly make the
//! propositional markets unauthorable from the browser.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use dclutch_product_compiler::partition_quality::{
    BASIS_POINTS_PER_UNIT_V1, FoundingBandV1, FoundingBeliefV1, MAX_BAND_VOLATILITY_BPS_V1,
    MAX_CELL_EX_ANTE_SHARE_BPS_V1, PartitionQualityModelV1, StatedPropositionV1,
    require_interesting_partition_v1,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Exact JSON schema this boundary accepts. Another one is refused, not guessed.
pub const REQUEST_FORMAT_V1: &str = "dclutch-partition-quality-request-v1";
/// Exact JSON schema this boundary returns.
pub const REPORT_FORMAT_V1: &str = "dclutch-partition-quality-report-v1";

/// Most interior cuts one request may carry.
///
/// **Provisional**, and labelled: the compiler bounds a partition by its own
/// rules and this is a transport courtesy, so a malformed request is refused
/// before it becomes a large allocation rather than after.
pub const MAX_CUTS_V1: usize = 1_024;

/// THE CANARY.
///
/// The browser must never write the ceiling on the author's ceiling, the
/// basis-point unit, or the volatility bound down. It reads all three from
/// here, BY CONSTANT NAME, so a change in the compiler fails this BUILD rather
/// than leaving a wizard that offers a ceiling the gate refuses — which is the
/// exact shape of the defect this crate is closing.
const _: () = assert!(MAX_CELL_EX_ANTE_SHARE_BPS_V1 == 9_000);
const _: () = assert!(BASIS_POINTS_PER_UNIT_V1 == 10_000);
const _: () = assert!(MAX_BAND_VOLATILITY_BPS_V1 == 100_000);
const _: () = assert!(MAX_CELL_EX_ANTE_SHARE_BPS_V1 as u64 <= BASIS_POINTS_PER_UNIT_V1);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestV1 {
    format: String,
    /// Strictly increasing interior cuts, as decimal text over the belief's
    /// own denominator.
    cuts: Vec<String>,
    /// The author's ceiling, in basis points.
    ceiling_bps: u32,
    belief: BeliefWireV1,
}

/// The belief family on the wire, tagged by KIND rather than by shape.
///
/// Tagged on purpose: an untagged union would let a spot band with a stray
/// probability vector decode as either member, and which model measures a
/// market is the one thing this wire must not leave ambiguous.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", rename_all_fields = "camelCase", tag = "kind", deny_unknown_fields)]
enum BeliefWireV1 {
    #[serde(rename = "spot-band")]
    SpotBand {
        anchor: String,
        denominator: String,
        volatility_bps: u32,
        window_slots: String,
        plausible_half_widths: u32,
    },
    #[serde(rename = "stated-proposition")]
    StatedProposition {
        denominator: String,
        cell_probability_bps: Vec<u32>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportV1 {
    format: &'static str,
    model: &'static str,
    ceiling_bps: u32,
    maximum_ceiling_bps: u32,
    characteristic_displacement: Option<String>,
    plausible_half_width: Option<String>,
    dominant_cell: u32,
    dominant_share_bps: u32,
    cell_share_bps: Vec<u32>,
    unresolved_share_bps: u32,
    degenerate: bool,
}

fn decimal_i128(value: &str, field: &str) -> Result<i128, String> {
    value
        .parse::<i128>()
        .map_err(|_| format!("{field} is not exact i128 decimal text"))
}

fn decimal_u64(value: &str, field: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("{field} is not exact u64 decimal text"))
}

const fn model_name(model: PartitionQualityModelV1) -> &'static str {
    match model {
        PartitionQualityModelV1::TriangularPlausibleBand => "triangular-plausible-band-v1",
        PartitionQualityModelV1::StatedCategoricalPrior => "stated-categorical-prior-v1",
    }
}

fn assess(request_json: &str) -> Result<String, String> {
    let request: RequestV1 = serde_json::from_str(request_json).map_err(|error| {
        format!("partition quality request is not the exact accepted shape: {error}")
    })?;
    if request.format != REQUEST_FORMAT_V1 {
        return Err("partition quality request is not the exact accepted format".to_owned());
    }
    if request.cuts.len() > MAX_CUTS_V1 {
        return Err(format!(
            "partition quality request carries {} cuts, above the {MAX_CUTS_V1} this boundary accepts",
            request.cuts.len()
        ));
    }
    let mut cuts = Vec::with_capacity(request.cuts.len());
    for (index, cut) in request.cuts.iter().enumerate() {
        cuts.push(decimal_i128(cut, &format!("cut {index}"))?);
    }
    let belief = match request.belief {
        BeliefWireV1::SpotBand {
            anchor,
            denominator,
            volatility_bps,
            window_slots,
            plausible_half_widths,
        } => FoundingBeliefV1::SpotBand {
            band: FoundingBandV1 {
                anchor: decimal_i128(&anchor, "band anchor")?,
                denominator: decimal_u64(&denominator, "band denominator")?,
                volatility_bps,
                window_slots: decimal_u64(&window_slots, "band window slots")?,
            },
            plausible_half_widths,
        },
        BeliefWireV1::StatedProposition {
            denominator,
            cell_probability_bps,
        } => FoundingBeliefV1::StatedProposition(StatedPropositionV1 {
            denominator: decimal_u64(&denominator, "proposition denominator")?,
            cell_probability_bps,
        }),
    };

    // The GATE, not a measurement: `require_interesting_partition_v1` refuses
    // `DegenerateOutcomePartition` and `CellShareCeilingAboveMaximum` itself,
    // and those refusals reach the reader by the compiler's own name for them.
    let report = require_interesting_partition_v1(&cuts, &belief, request.ceiling_bps)
        .map_err(|error| format!("{error:?}"))?;

    let answer = ReportV1 {
        format: REPORT_FORMAT_V1,
        model: model_name(report.model),
        ceiling_bps: request.ceiling_bps,
        maximum_ceiling_bps: MAX_CELL_EX_ANTE_SHARE_BPS_V1,
        characteristic_displacement: report.characteristic_displacement.map(|v| v.to_string()),
        plausible_half_width: report.plausible_half_width.map(|v| v.to_string()),
        dominant_cell: report.dominant_cell,
        dominant_share_bps: report.dominant_share_bps,
        cell_share_bps: report.cell_share_bps.clone(),
        unresolved_share_bps: report.unresolved_share_bps,
        degenerate: report.is_degenerate(request.ceiling_bps),
    };
    serde_json::to_string(&answer)
        .map_err(|error| format!("partition quality report could not be serialized: {error}"))
}

/// Measure one partition against its own founding belief, through the gate.
///
/// Returns the compiler's report, or `{"error": "<the compiler's own refusal>"}`.
/// A refusal is never softened into a warning here: `DegenerateOutcomePartition`
/// reaches the browser as that word.
#[wasm_bindgen]
#[must_use]
pub fn require_interesting_partition_v1_wasm(request_json: &str) -> String {
    match assess(request_json) {
        Ok(report) => report,
        Err(reason) => serde_json::json!({ "error": reason }).to_string(),
    }
}

/// The ceiling on an author's ceiling, for the loader's post-load re-check.
#[wasm_bindgen]
#[must_use]
pub fn partition_quality_maximum_ceiling_bps_v1() -> u32 {
    MAX_CELL_EX_ANTE_SHARE_BPS_V1
}

/// One basis-point unit, for the loader's post-load re-check.
#[wasm_bindgen]
#[must_use]
pub fn partition_quality_basis_points_per_unit_v1() -> u64 {
    BASIS_POINTS_PER_UNIT_V1
}

/// The largest volatility a band may state, for the wizard's own input bound.
#[wasm_bindgen]
#[must_use]
pub fn partition_quality_maximum_volatility_bps_v1() -> u32 {
    MAX_BAND_VOLATILITY_BPS_V1
}
