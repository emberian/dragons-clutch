//! Key-free degree-2/3 Product authoring command.
//!
//! This is a filesystem adapter over the public Rust operator. It supplies no
//! ProductBasis or price-gate semantics of its own: after canonical JSON
//! parsing, `compile_spline_product_records_v3` is the only compiler it invokes.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use dclutch_product_payoff_v2_codec::price_gate_v1::PRICE_GATE_REQUEST_BYTES_V1;
use dclutch_product_runtime_v2::{ContentId, portfolio_record_bytes, result_domain_record_bytes};
use dclutch_product_runtime_v2_admission::{FinalizedRecordCoordinateV2, PRODUCT_RECORD_BYTES_V2};
use dclutch_product_runtime_v2_operator::spline_basis_v3::{
    CompiledSplineProductRecordsV3, SplineProductCompilationInputV3,
    compile_spline_product_records_v3, spline_basis_output_bytes_v3,
};
use dclutch_product_runtime_v2_operator::{
    FoundingBandV1, FoundingBeliefV1, MAX_CELL_EX_ANTE_SHARE_BPS_V1, PartitionQualityModelV1,
    PartitionQualityReportV1, require_interesting_partition_v1,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use solana_program::{hash::hash, pubkey::Pubkey};

use crate::{Error, Result, stdout_json_value_v1};

/// Stable key-free successor command.
pub(crate) const COMMAND_V1: &str = "product-spline-compile-v1";

const INPUT_SCHEMA_V1: &str = "dclutch/product-spline-authoring-input/v1";
const REPORT_SCHEMA_V1: &str = "dclutch/product-spline-authoring-report/v1";
const COMPLETION_SCHEMA_V1: &str = "dclutch/product-spline-authoring-completion/v1";
const MAX_INPUT_BYTES_V1: u64 = 65_536;
const MAX_PRODUCT_OUTCOMES_V1: usize = 64;
const MAX_SPLINE_WIDTH_V1: usize = 10;
const PRODUCT_FILE_V1: &str = "product.bin";
const DOMAIN_FILE_V1: &str = "result-domain.bin";
const PORTFOLIO_FILE_V1: &str = "portfolio.bin";
const BASIS_FILE_V1: &str = "product-basis.bin";
const GATE_FILE_V1: &str = "price-gate.bin";
const REPORT_FILE_V1: &str = "report.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InputV1 {
    schema: String,
    registry_program: String,
    product_id: String,
    coordinate_domain_id: String,
    result_unit_id: String,
    claim_basis_id: String,
    representation_release_id: String,
    mapping_release_id: String,
    cut_denominator: String,
    cuts: Vec<String>,
    portfolio_denominator: String,
    coefficients: Vec<String>,
    evaluator_release_id: String,
    degree: u8,
    interior_multiplicity: bool,
    payout_scale: String,
    knot_denominator: String,
    knots: Vec<String>,
    failure_payouts: Vec<String>,
    price_gate_certificate_hex: String,
    /// What the author believes about where this market's coordinate will go.
    ///
    /// REQUIRED, and deliberately without a default. How uncertain the author
    /// thinks the outcome is is part of the description C-02 compiles, not a
    /// constant this file is entitled to pick: a market founded on a silently
    /// defaulted volatility is exactly the failure the gate exists to catch,
    /// and it would be founded with nobody having said anything false.
    ///
    /// All three quantities are declarations rather than observations, because
    /// on THIS path none of them is available to derive — measured
    /// 2026-09-01, the other nineteen input fields are pure geometry and carry
    /// no spot, no window and no volatility. A declared band also makes the
    /// refusal legible: you said 200 bp over an hour, and these cuts do not
    /// describe that belief.
    founding_band: FoundingBandInputV1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FoundingBandInputV1 {
    /// Spot coordinate numerator at founding, over `cut_denominator`.
    anchor: String,
    /// Stated volatility in basis points of spot over the reference window.
    volatility_bps: u32,
    /// This market's own window from founding to deadline, in slots.
    window_slots: String,
    /// How many characteristic displacements the plausible band reaches.
    plausible_half_widths: u32,
    /// Ceiling on any one cell's share of ex-ante mass, in basis points.
    max_cell_share_bps: u32,
}

#[derive(Clone, Debug)]
struct ParsedInputV1 {
    registry_program: Pubkey,
    product_id: ContentId,
    coordinate_domain_id: ContentId,
    result_unit_id: ContentId,
    claim_basis_id: ContentId,
    representation_release_id: ContentId,
    mapping_release_id: ContentId,
    cut_denominator: u64,
    cuts: Vec<i128>,
    portfolio_denominator: u64,
    coefficients: Vec<u64>,
    evaluator_release_id: ContentId,
    degree: u8,
    interior_multiplicity: bool,
    payout_scale: u64,
    knot_denominator: u64,
    knots: Vec<i128>,
    failure_payouts: Vec<u64>,
    price_gate_certificate: [u8; PRICE_GATE_REQUEST_BYTES_V1],
    founding_band: ParsedFoundingBandV1,
}

#[derive(Clone, Debug)]
struct ParsedFoundingBandV1 {
    belief: FoundingBeliefV1,
    max_cell_share_bps: u32,
}

impl ParsedInputV1 {
    fn operator_input(&self) -> SplineProductCompilationInputV3<'_> {
        SplineProductCompilationInputV3 {
            product_id: self.product_id,
            coordinate_domain_id: self.coordinate_domain_id,
            result_unit_id: self.result_unit_id,
            claim_basis_id: self.claim_basis_id,
            representation_release_id: self.representation_release_id,
            mapping_release_id: self.mapping_release_id,
            cut_denominator: self.cut_denominator,
            cuts: &self.cuts,
            portfolio_denominator: self.portfolio_denominator,
            coefficients: &self.coefficients,
            evaluator_release_id: self.evaluator_release_id,
            degree: self.degree,
            interior_multiplicity: self.interior_multiplicity,
            payout_scale: self.payout_scale,
            knot_denominator: self.knot_denominator,
            knots: &self.knots,
            failure_payouts: &self.failure_payouts,
            price_gate_certificate: &self.price_gate_certificate,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct RecordReportV1 {
    file: &'static str,
    bytes: usize,
    schema_id: String,
    content_sha256: String,
    raw_account: String,
    staging_account: String,
}

#[derive(Clone, Debug, Serialize)]
struct RecordsReportV1 {
    product: RecordReportV1,
    result_domain: RecordReportV1,
    portfolio: RecordReportV1,
    product_basis: RecordReportV1,
    price_gate: RecordReportV1,
}

#[derive(Clone, Debug, Serialize)]
struct PriceGateReportV1 {
    scale: u32,
    mass: String,
    degree: u8,
    width: usize,
    atom_count: usize,
    prices: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct ReportV1 {
    schema: &'static str,
    command: &'static str,
    key_free: bool,
    signs: bool,
    submits: bool,
    input_sha256: String,
    registry_program: String,
    product_outcome_count: u32,
    basis_width: u32,
    degree: u8,
    interior_multiplicity: bool,
    payout_scale: String,
    rounding_boundary: &'static str,
    semantic_basis_id: String,
    records: RecordsReportV1,
    verified_price_gate: PriceGateReportV1,
    partition_quality: PartitionQualityReportOutV1,
}

/// How much of the ex-ante question each cell takes.
///
/// Every field is unconditional because the founding band is a required input.
/// There is no "not measured" state to render: an input that declines to say
/// what the author believes refuses at parse time rather than producing a
/// report with a hole in it.
///
/// This exists because SHAPE WAS NEVER THE MISSING PROPERTY. The convicted
/// SOL/USD market — cuts in USD cents against a source reporting price atoms —
/// passes every structural check with full marks: strictly increasing cuts,
/// regions exactly cuts + 1, a gcd-normalized non-zero portfolio. What was
/// missing is where the cuts sit relative to the coordinate that will actually
/// be observed, and that is what these numbers state.
#[derive(Clone, Debug, Serialize)]
struct PartitionQualityReportOutV1 {
    model: &'static str,
    anchor: String,
    volatility_bps: u32,
    window_slots: String,
    characteristic_displacement: String,
    plausible_half_width: String,
    dominant_cell: u32,
    dominant_share_bps: u32,
    max_cell_share_bps: u32,
    cell_share_bps: Vec<u32>,
}

impl PartitionQualityReportOutV1 {
    fn measured(parsed: &ParsedFoundingBandV1, report: &PartitionQualityReportV1) -> Self {
        // Exhaustive on purpose: a second model variant must name itself here
        // rather than be reported under this one's name. This tool compiles
        // SPLINE price products, whose belief is a spot band by construction --
        // `parse_founding_band` builds no other kind -- so a stated prior here
        // would be a defect rather than a shape to render.
        let (model, band) = match (&report.model, &parsed.belief) {
            (
                PartitionQualityModelV1::TriangularPlausibleBand,
                FoundingBeliefV1::SpotBand { band, .. },
            ) => ("triangular-plausible-band-v1", *band),
            _ => unreachable!("a spline price product's belief is a spot band"),
        };
        let render = |value: Option<i128>| {
            value.map_or_else(|| "none".to_string(), |measured| measured.to_string())
        };
        Self {
            model,
            anchor: band.anchor.to_string(),
            volatility_bps: band.volatility_bps,
            window_slots: band.window_slots.to_string(),
            characteristic_displacement: render(report.characteristic_displacement),
            plausible_half_width: render(report.plausible_half_width),
            dominant_cell: report.dominant_cell,
            dominant_share_bps: report.dominant_share_bps,
            max_cell_share_bps: parsed.max_cell_share_bps,
            cell_share_bps: report.cell_share_bps.clone(),
        }
    }
}

#[derive(Debug)]
struct CompiledFilesV1 {
    product: [u8; PRODUCT_RECORD_BYTES_V2],
    domain: Vec<u8>,
    portfolio: Vec<u8>,
    basis: Vec<u8>,
    gate: [u8; PRICE_GATE_REQUEST_BYTES_V1],
    compiled: CompiledSplineProductRecordsV3,
    quality: PartitionQualityReportV1,
}

#[derive(Clone, Debug)]
struct ArgumentsV1 {
    input: PathBuf,
    output_dir: PathBuf,
}

/// Compile one key-free spline Product and atomically persist its report.
pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments(arguments)?;
    validate_output_path(&arguments.output_dir)?;
    let input_path = canonical_input_path(&arguments.input)?;
    let input_bytes = read_bounded(&input_path)?;
    let wire: InputV1 = serde_json::from_slice(&input_bytes)
        .map_err(|error| Error::new(format!("input/json: {error}")))?;
    let parsed = parse_input(wire)?;
    let compiled = compile(&parsed)?;
    let report = report(&parsed, &input_bytes, &compiled)?;
    let report_bytes = canonical_json(&report)?;
    persist(&arguments.output_dir, &compiled, &report_bytes)?;
    stdout_json_value_v1(&json!({
        "schema": COMPLETION_SCHEMA_V1,
        "output_dir": arguments.output_dir,
        "report": arguments.output_dir.join(REPORT_FILE_V1),
        "report_sha256": hex(hash(&report_bytes).to_bytes()),
    }))
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let (mut input, mut output_dir) = (None, None);
    let mut iterator = arguments.into_iter();
    while let Some(flag) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("argument/missing-value: {flag}")))?;
        let destination = match flag.as_str() {
            "--input" => &mut input,
            "--output-dir" => &mut output_dir,
            _ => return Err(Error::new(format!("argument/unknown: {flag}"))),
        };
        if destination.replace(PathBuf::from(value)).is_some() {
            return Err(Error::new(format!("argument/duplicate: {flag}")));
        }
    }
    Ok(ArgumentsV1 {
        input: input.ok_or_else(|| Error::new("argument/required: --input"))?,
        output_dir: output_dir.ok_or_else(|| Error::new("argument/required: --output-dir"))?,
    })
}

fn canonical_input_path(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Error::new(format!(
            "input/path-relative: {}",
            path.display()
        )));
    }
    let canonical = fs::canonicalize(path)
        .map_err(|error| Error::new(format!("input/unreadable: {}: {error}", path.display())))?;
    if canonical != path {
        return Err(Error::new(format!(
            "input/path-noncanonical: expected {}",
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn validate_output_path(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(Error::new(format!(
            "output/path-relative: {}",
            path.display()
        )));
    }
    if path.file_name().is_none() {
        return Err(Error::new("output/path-root"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("output/path-parent"))?;
    let canonical_parent = fs::canonicalize(parent).map_err(|error| {
        Error::new(format!(
            "output/parent-unreadable: {}: {error}",
            parent.display()
        ))
    })?;
    if canonical_parent != parent {
        return Err(Error::new(format!(
            "output/path-noncanonical: parent is {}",
            canonical_parent.display()
        )));
    }
    if path
        .try_exists()
        .map_err(|error| Error::new(format!("output/check: {error}")))?
    {
        return Err(Error::new(format!("output/exists: {}", path.display())));
    }
    Ok(())
}

fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_INPUT_BYTES_V1 {
        return Err(Error::new(format!(
            "input/size: expected 1..={MAX_INPUT_BYTES_V1} bytes"
        )));
    }
    let capacity = usize::try_from(metadata.len()).map_err(|_| Error::new("input/size"))?;
    let mut bytes = Vec::with_capacity(capacity);
    File::open(path)?
        .take(MAX_INPUT_BYTES_V1 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() != capacity {
        return Err(Error::new("input/changed-during-read"));
    }
    Ok(bytes)
}

fn parse_input(input: InputV1) -> Result<ParsedInputV1> {
    if input.schema != INPUT_SCHEMA_V1 {
        return Err(Error::new(format!(
            "input/schema: expected {INPUT_SCHEMA_V1}"
        )));
    }
    let registry_program = input
        .registry_program
        .parse::<Pubkey>()
        .map_err(|_| Error::new("input/registry-program"))?;
    if registry_program.to_string() != input.registry_program {
        return Err(Error::new("input/registry-program-noncanonical"));
    }
    if input
        .cuts
        .len()
        .checked_add(2)
        .is_none_or(|count| count > MAX_PRODUCT_OUTCOMES_V1 || input.coefficients.len() != count)
    {
        return Err(Error::new(format!(
            "input/product-width: coefficients must equal cuts + 2 and outcomes must be <= {MAX_PRODUCT_OUTCOMES_V1}"
        )));
    }
    if input.failure_payouts.is_empty() || input.failure_payouts.len() > MAX_SPLINE_WIDTH_V1 {
        return Err(Error::new(format!(
            "input/basis-width: expected 1..={MAX_SPLINE_WIDTH_V1}"
        )));
    }
    let cuts = input
        .cuts
        .iter()
        .enumerate()
        .map(|(index, value)| signed_decimal(value, &format!("cuts[{index}]")))
        .collect::<Result<Vec<_>>>()?;
    let coefficients = input
        .coefficients
        .iter()
        .enumerate()
        .map(|(index, value)| unsigned_decimal(value, &format!("coefficients[{index}]")))
        .collect::<Result<Vec<_>>>()?;
    let knots = input
        .knots
        .iter()
        .enumerate()
        .map(|(index, value)| signed_decimal(value, &format!("knots[{index}]")))
        .collect::<Result<Vec<_>>>()?;
    let failure_payouts = input
        .failure_payouts
        .iter()
        .enumerate()
        .map(|(index, value)| unsigned_decimal(value, &format!("failure_payouts[{index}]")))
        .collect::<Result<Vec<_>>>()?;
    let cut_denominator = unsigned_decimal(&input.cut_denominator, "cut_denominator")?;
    let founding_band = parse_founding_band(&input.founding_band, cut_denominator)?;
    Ok(ParsedInputV1 {
        registry_program,
        product_id: content_id(&input.product_id, "product_id")?,
        coordinate_domain_id: content_id(&input.coordinate_domain_id, "coordinate_domain_id")?,
        result_unit_id: content_id(&input.result_unit_id, "result_unit_id")?,
        claim_basis_id: content_id(&input.claim_basis_id, "claim_basis_id")?,
        representation_release_id: content_id(
            &input.representation_release_id,
            "representation_release_id",
        )?,
        mapping_release_id: content_id(&input.mapping_release_id, "mapping_release_id")?,
        cut_denominator,
        cuts,
        portfolio_denominator: unsigned_decimal(
            &input.portfolio_denominator,
            "portfolio_denominator",
        )?,
        coefficients,
        evaluator_release_id: content_id(&input.evaluator_release_id, "evaluator_release_id")?,
        degree: input.degree,
        interior_multiplicity: input.interior_multiplicity,
        payout_scale: unsigned_decimal(&input.payout_scale, "payout_scale")?,
        knot_denominator: unsigned_decimal(&input.knot_denominator, "knot_denominator")?,
        knots,
        failure_payouts,
        price_gate_certificate: fixed_hex::<PRICE_GATE_REQUEST_BYTES_V1>(
            &input.price_gate_certificate_hex,
            "price_gate_certificate_hex",
        )?,
        founding_band,
    })
}

/// Parse one founding band, keeping it on the partition's own denominator.
///
/// A band quoted over another denominator would measure a different market
/// than the one being compiled, so it refuses rather than rescaling: rescaling
/// would silently answer a question nobody asked.
fn parse_founding_band(
    band: &FoundingBandInputV1,
    cut_denominator: u64,
) -> Result<ParsedFoundingBandV1> {
    let anchor = signed_decimal(&band.anchor, "founding_band/anchor")?;
    let window_slots = unsigned_decimal(&band.window_slots, "founding_band/window_slots")?;
    if band.plausible_half_widths == 0 {
        return Err(Error::new(
            "input/founding_band/plausible_half_widths: expected at least one",
        ));
    }
    // The upper bound is the compiler's own MAX_CELL_EX_ANTE_SHARE_BPS_V1, read
    // rather than restated: an author states the ceiling their product wants,
    // at or below the one this release will enforce. It was `10_000` here until
    // 2026-09-01, which let an author disable the gate by stating it.
    if band.max_cell_share_bps == 0 || band.max_cell_share_bps > MAX_CELL_EX_ANTE_SHARE_BPS_V1 {
        return Err(Error::new(format!(
            "input/founding_band/max_cell_share_bps: expected 1..={MAX_CELL_EX_ANTE_SHARE_BPS_V1}"
        )));
    }
    Ok(ParsedFoundingBandV1 {
        belief: FoundingBeliefV1::SpotBand {
            band: FoundingBandV1 {
                anchor,
                denominator: cut_denominator,
                volatility_bps: band.volatility_bps,
                window_slots,
            },
            plausible_half_widths: band.plausible_half_widths,
        },
        max_cell_share_bps: band.max_cell_share_bps,
    })
}

fn content_id(value: &str, field: &str) -> Result<ContentId> {
    ContentId::new(fixed_hex::<32>(value, field)?)
        .map_err(|_| Error::new(format!("input/{field}: reserved zero identity")))
}

fn fixed_hex<const N: usize>(value: &str, field: &str) -> Result<[u8; N]> {
    if value.len() != N * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::new(format!(
            "input/{field}: expected {} lowercase hexadecimal characters",
            N * 2
        )));
    }
    let mut output = [0_u8; N];
    for (slot, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = core::str::from_utf8(pair).map_err(|_| Error::new(format!("input/{field}")))?;
        output[slot] =
            u8::from_str_radix(text, 16).map_err(|_| Error::new(format!("input/{field}")))?;
    }
    Ok(output)
}

fn unsigned_decimal(value: &str, field: &str) -> Result<u64> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(Error::new(format!(
            "input/{field}: expected canonical decimal u64"
        )));
    }
    value
        .parse()
        .map_err(|_| Error::new(format!("input/{field}: outside u64")))
}

fn signed_decimal(value: &str, field: &str) -> Result<i128> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty()
        || value.starts_with('+')
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
        || value == "-0"
    {
        return Err(Error::new(format!(
            "input/{field}: expected canonical decimal i128"
        )));
    }
    value
        .parse()
        .map_err(|_| Error::new(format!("input/{field}: outside i128")))
}

fn compile(input: &ParsedInputV1) -> Result<CompiledFilesV1> {
    let operator = input.operator_input();
    // Before any record is built. A degenerate partition must not leave a
    // compiled graph on disk that somebody founds later without the report.
    let parsed_band = &input.founding_band;
    let quality = require_interesting_partition_v1(
        &input.cuts,
        &parsed_band.belief,
        parsed_band.max_cell_share_bps,
    )
    .map_err(|error| {
        Error::new(format!(
            "compile/partition-quality: {error:?} — the cuts are exhaustive, disjoint, \
             ordered and canonical, and one cell still takes the market"
        ))
    })?;
    let domain_bytes = result_domain_record_bytes(input.cuts.len())
        .map_err(|_| Error::new("compile/result-domain-size"))?;
    let portfolio_bytes = portfolio_record_bytes(input.coefficients.len())
        .map_err(|_| Error::new("compile/portfolio-size"))?;
    let basis_bytes = spline_basis_output_bytes_v3(operator)
        .map_err(|error| Error::new(format!("compile/basis-size: {error:?}")))?;
    let mut product = [0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![0_u8; domain_bytes];
    let mut portfolio = vec![0_u8; portfolio_bytes];
    let mut basis = vec![0_u8; basis_bytes];
    let compiled = compile_spline_product_records_v3(
        input.registry_program,
        operator,
        &mut product,
        &mut domain,
        &mut portfolio,
        &mut basis,
    )
    .map_err(|error| Error::new(format!("compile/operator: {error:?}")))?;
    Ok(CompiledFilesV1 {
        product,
        domain,
        portfolio,
        basis,
        gate: input.price_gate_certificate,
        compiled,
        quality,
    })
}

fn report(input: &ParsedInputV1, input_bytes: &[u8], files: &CompiledFilesV1) -> Result<ReportV1> {
    let compiled = files.compiled;
    let gate = compiled.verified_price_gate;
    Ok(ReportV1 {
        schema: REPORT_SCHEMA_V1,
        command: COMMAND_V1,
        key_free: true,
        signs: false,
        submits: false,
        input_sha256: hex(hash(input_bytes).to_bytes()),
        registry_program: input.registry_program.to_string(),
        product_outcome_count: compiled.product.outcome_count,
        basis_width: compiled.basis_width,
        degree: input.degree,
        interior_multiplicity: input.interior_multiplicity,
        payout_scale: input.payout_scale.to_string(),
        rounding_boundary: "cumulative-floor-v3",
        semantic_basis_id: hex(compiled.semantic_basis_id.to_bytes()),
        records: RecordsReportV1 {
            product: record_report(
                PRODUCT_FILE_V1,
                &files.product,
                compiled.product.receipt.product,
            ),
            result_domain: record_report(
                DOMAIN_FILE_V1,
                &files.domain,
                compiled.product.receipt.result_domain,
            ),
            portfolio: record_report(
                PORTFOLIO_FILE_V1,
                &files.portfolio,
                compiled.product.receipt.portfolio,
            ),
            product_basis: record_report(BASIS_FILE_V1, &files.basis, compiled.linked_basis),
            price_gate: record_report(GATE_FILE_V1, &files.gate, compiled.price_gate),
        },
        verified_price_gate: PriceGateReportV1 {
            scale: gate.scale(),
            mass: gate.mass().to_string(),
            degree: gate.degree(),
            width: gate.width(),
            atom_count: gate.atom_count(),
            prices: gate.active_prices().iter().map(u64::to_string).collect(),
        },
        partition_quality: PartitionQualityReportOutV1::measured(
            &input.founding_band,
            &files.quality,
        ),
    })
}

fn record_report(
    file: &'static str,
    bytes: &[u8],
    coordinate: FinalizedRecordCoordinateV2,
) -> RecordReportV1 {
    debug_assert_eq!(hash(bytes).to_bytes(), coordinate.content_digest.to_bytes());
    RecordReportV1 {
        file,
        bytes: bytes.len(),
        schema_id: hex(coordinate.schema_id.to_bytes()),
        content_sha256: hex(coordinate.content_digest.to_bytes()),
        raw_account: Pubkey::new_from_array(coordinate.raw_account.to_bytes()).to_string(),
        staging_account: Pubkey::new_from_array(coordinate.staging_account.to_bytes()).to_string(),
    }
}

fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut output = serde_json::to_vec_pretty(value)?;
    output.push(b'\n');
    Ok(output)
}

fn persist(output: &Path, files: &CompiledFilesV1, report: &[u8]) -> Result<()> {
    let parent = output
        .parent()
        .ok_or_else(|| Error::new("output/path-parent"))?;
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::new("output/path-filename"))?;
    let temporary = parent.join(format!(".{name}.tmp-{}", std::process::id()));
    if temporary
        .try_exists()
        .map_err(|error| Error::new(format!("output/temp-check: {error}")))?
    {
        return Err(Error::new(format!(
            "output/temp-exists: {}",
            temporary.display()
        )));
    }
    fs::create_dir(&temporary)?;
    let outcome = (|| -> Result<()> {
        for (name, bytes) in [
            (PRODUCT_FILE_V1, files.product.as_slice()),
            (DOMAIN_FILE_V1, files.domain.as_slice()),
            (PORTFOLIO_FILE_V1, files.portfolio.as_slice()),
            (BASIS_FILE_V1, files.basis.as_slice()),
            (GATE_FILE_V1, files.gate.as_slice()),
            (REPORT_FILE_V1, report),
        ] {
            write_new(&temporary.join(name), bytes)?;
        }
        File::open(&temporary)?.sync_all()?;
        fs::rename(&temporary, output)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if outcome.is_err() && temporary.exists() {
        let _ = fs::remove_dir_all(&temporary);
    }
    outcome
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn hex<const N: usize>(bytes: [u8; N]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_product_payoff_v2_codec::price_gate_v1::{
        PRICE_GATE_ATOM_COUNT_OFFSET_V1, PRICE_GATE_DEGREE_OFFSET_V1,
        PRICE_GATE_DENOMINATORS_OFFSET_V1, PRICE_GATE_MAGIC_OFFSET_V1, PRICE_GATE_MAGIC_V1,
        PRICE_GATE_MASS_OFFSET_V1, PRICE_GATE_NUMERATORS_OFFSET_V1, PRICE_GATE_PRICES_OFFSET_V1,
        PRICE_GATE_PROFILE_OFFSET_V1, PRICE_GATE_PROFILE_V1, PRICE_GATE_SCALE_OFFSET_V1,
        PRICE_GATE_SCHEMA_VERSION_V1, PRICE_GATE_VERSION_OFFSET_V1, PRICE_GATE_WEIGHTS_OFFSET_V1,
        PRICE_GATE_WIDTH_OFFSET_V1,
    };
    use serde_json::Value;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn certificate() -> [u8; PRICE_GATE_REQUEST_BYTES_V1] {
        let mut bytes = [0_u8; PRICE_GATE_REQUEST_BYTES_V1];
        bytes[PRICE_GATE_MAGIC_OFFSET_V1..PRICE_GATE_MAGIC_OFFSET_V1 + 8]
            .copy_from_slice(&PRICE_GATE_MAGIC_V1);
        bytes[PRICE_GATE_VERSION_OFFSET_V1..PRICE_GATE_VERSION_OFFSET_V1 + 2]
            .copy_from_slice(&PRICE_GATE_SCHEMA_VERSION_V1.to_le_bytes());
        bytes[PRICE_GATE_PROFILE_OFFSET_V1..PRICE_GATE_PROFILE_OFFSET_V1 + 2]
            .copy_from_slice(&PRICE_GATE_PROFILE_V1.to_le_bytes());
        bytes[PRICE_GATE_SCALE_OFFSET_V1..PRICE_GATE_SCALE_OFFSET_V1 + 4]
            .copy_from_slice(&7_u32.to_le_bytes());
        bytes[PRICE_GATE_MASS_OFFSET_V1..PRICE_GATE_MASS_OFFSET_V1 + 8]
            .copy_from_slice(&1_u64.to_le_bytes());
        bytes[PRICE_GATE_DEGREE_OFFSET_V1] = 2;
        bytes[PRICE_GATE_WIDTH_OFFSET_V1] = 3;
        bytes[PRICE_GATE_ATOM_COUNT_OFFSET_V1] = 1;
        for (claim, payout) in [1_u64, 4, 2].iter().enumerate() {
            let offset = PRICE_GATE_PRICES_OFFSET_V1 + claim * 8;
            bytes[offset..offset + 8].copy_from_slice(&payout.to_le_bytes());
        }
        bytes[PRICE_GATE_WEIGHTS_OFFSET_V1..PRICE_GATE_WEIGHTS_OFFSET_V1 + 8]
            .copy_from_slice(&1_u64.to_le_bytes());
        bytes[PRICE_GATE_NUMERATORS_OFFSET_V1..PRICE_GATE_NUMERATORS_OFFSET_V1 + 8]
            .copy_from_slice(&3_i64.to_le_bytes());
        bytes[PRICE_GATE_DENOMINATORS_OFFSET_V1..PRICE_GATE_DENOMINATORS_OFFSET_V1 + 4]
            .copy_from_slice(&2_u32.to_le_bytes());
        bytes
    }

    fn fixture() -> InputV1 {
        let id = |byte: u8| hex([byte; 32]);
        InputV1 {
            schema: INPUT_SCHEMA_V1.to_owned(),
            registry_program: Pubkey::new_from_array([0xa2; 32]).to_string(),
            product_id: id(1),
            coordinate_domain_id: id(2),
            result_unit_id: id(3),
            claim_basis_id: id(4),
            representation_release_id: id(5),
            mapping_release_id: id(6),
            cut_denominator: "1".to_owned(),
            cuts: vec!["1".to_owned()],
            portfolio_denominator: "1".to_owned(),
            coefficients: vec!["1".to_owned(), "1".to_owned(), "1".to_owned()],
            evaluator_release_id: id(7),
            degree: 2,
            interior_multiplicity: false,
            payout_scale: "7".to_owned(),
            knot_denominator: "1".to_owned(),
            knots: ["0", "0", "0", "3", "3", "3"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            failure_payouts: ["0", "0", "7"].into_iter().map(str::to_owned).collect(),
            price_gate_certificate_hex: hex(certificate()),
            // The band this example's single cut actually describes. It is a
            // declaration, not an observation, so a synthetic fixture can make
            // it honestly: the cut sits at 1 and the author says spot is there.
            founding_band: band("1", 9_000),
        }
    }

    /// A band around the coordinate this fixture's single cut actually sits on.
    fn band(anchor: &str, max_cell_share_bps: u32) -> FoundingBandInputV1 {
        FoundingBandInputV1 {
            anchor: anchor.to_owned(),
            volatility_bps: 2_000,
            window_slots: "10000".to_owned(),
            plausible_half_widths: 2,
            max_cell_share_bps,
        }
    }

    fn directory() -> PathBuf {
        std::env::temp_dir().join(format!(
            "dclutch-spline-product-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn writes_the_five_records_and_machine_report_as_one_directory() {
        let root = directory();
        fs::create_dir(&root).expect("test root");
        let output = root.join("output");
        let parsed = parse_input(fixture()).expect("fixture input");
        let compiled = compile(&parsed).expect("operator compilation");
        let report = report(&parsed, b"fixture", &compiled).expect("report");
        let report_bytes = canonical_json(&report).expect("report bytes");
        persist(&output, &compiled, &report_bytes).expect("atomic output");
        for name in [
            PRODUCT_FILE_V1,
            DOMAIN_FILE_V1,
            PORTFOLIO_FILE_V1,
            BASIS_FILE_V1,
            GATE_FILE_V1,
            REPORT_FILE_V1,
        ] {
            assert!(output.join(name).is_file(), "missing {name}");
        }
        let decoded: Value =
            serde_json::from_slice(&fs::read(output.join(REPORT_FILE_V1)).expect("stored report"))
                .expect("report JSON");
        assert_eq!(decoded["key_free"], true);
        assert_eq!(
            decoded["verified_price_gate"]["prices"],
            json!(["1", "4", "2"])
        );
        assert_eq!(decoded["basis_width"], 3);
        fs::remove_dir_all(&root).expect("cleanup exact test root");
    }

    #[test]
    fn an_input_that_declares_no_band_refuses_rather_than_assuming_one() {
        // The whole point of the ruling: an author who will not say how
        // uncertain they think the outcome is does not get a default. A market
        // founded on an assumed volatility is founded with nobody having said
        // anything false, which is the failure mode the gate exists to catch.
        let mut absent = serde_json::to_value(fixture()).expect("fixture JSON");
        absent
            .as_object_mut()
            .expect("object")
            .remove("founding_band");
        let error = serde_json::from_value::<InputV1>(absent)
            .expect_err("an undeclared band must refuse")
            .to_string();
        assert!(error.contains("founding_band"), "{error}");

        // And each of its parts is required too, so a partial band cannot
        // smuggle a default in through one field.
        for field in [
            "anchor",
            "volatility_bps",
            "window_slots",
            "plausible_half_widths",
            "max_cell_share_bps",
        ] {
            let mut partial = serde_json::to_value(fixture()).expect("fixture JSON");
            partial["founding_band"]
                .as_object_mut()
                .expect("band object")
                .remove(field);
            let error = serde_json::from_value::<InputV1>(partial)
                .expect_err("a partial band must refuse")
                .to_string();
            assert!(error.contains(field), "{field}: {error}");
        }
    }

    #[test]
    fn the_report_states_how_much_of_the_question_each_cell_takes() {
        // The number the browser renders, from the Rust that owns the model.
        let parsed = parse_input(fixture()).expect("centred input");
        let compiled = compile(&parsed).expect("a centred band compiles");
        let decoded: Value = serde_json::from_slice(
            &canonical_json(&report(&parsed, b"fixture", &compiled).expect("report"))
                .expect("report bytes"),
        )
        .expect("report JSON");
        assert_eq!(
            decoded["partition_quality"]["model"],
            json!("triangular-plausible-band-v1")
        );
        // One cut at the anchor splits a symmetric band exactly in half.
        assert_eq!(
            decoded["partition_quality"]["cell_share_bps"],
            json!([5_000, 5_000])
        );
        assert_eq!(decoded["partition_quality"]["dominant_share_bps"], 5_000);
        assert_eq!(decoded["partition_quality"]["max_cell_share_bps"], 9_000);
        // The author's own declaration is echoed back, so a reader can see the
        // belief the shares were measured against and not just the shares.
        assert_eq!(decoded["partition_quality"]["volatility_bps"], 2_000);
        assert_eq!(decoded["partition_quality"]["window_slots"], json!("10000"));
    }

    #[test]
    fn a_cut_outside_the_band_refuses_before_any_record_is_written() {
        // SHAPE IS NOT THE MISSING PROPERTY. This input passes every structural
        // check the tree had before: the cut is canonical, regions are exactly
        // cuts + 1, the portfolio is gcd-normalized and nonzero. What is wrong
        // is that spot sits a thousand units above the only cut, so the market
        // resolves into its top cell and stays there.
        let root = directory();
        fs::create_dir(&root).expect("test root");
        let output = root.join("output");
        let mut degenerate = fixture();
        degenerate.founding_band = band("1000", 9_000);
        let parsed = parse_input(degenerate).expect("degenerate input");
        let error = compile(&parsed)
            .expect_err("a market with one answer must refuse")
            .0;
        assert!(error.contains("compile/partition-quality"), "{error}");
        assert!(
            error.contains("DegenerateOutcomePartition"),
            "the refusal must name itself: {error}"
        );
        assert!(!output.exists(), "a refused compile wrote a record graph");

        // POSITIVE CONTROL in the same run: the same fixture, the same band
        // shape, spot on the cut instead of far above it. If the refusal above
        // came from the band machinery rather than from placement, this fails.
        let mut centred = fixture();
        centred.founding_band = band("1", 9_000);
        let parsed = parse_input(centred).expect("centred input");
        compile(&parsed).expect("placement is what was wrong, not the band");
        fs::remove_dir_all(&root).expect("cleanup exact test root");
    }

    #[test]
    fn unknown_and_duplicate_json_fields_refuse() {
        let mut unknown = serde_json::to_value(fixture()).expect("fixture JSON");
        unknown
            .as_object_mut()
            .expect("object")
            .insert("invented_basis_authority".to_owned(), json!(true));
        let error = serde_json::from_value::<InputV1>(unknown)
            .expect_err("unknown authority must refuse")
            .to_string();
        assert!(error.contains("unknown field"), "{error}");

        let canonical = serde_json::to_string(&fixture()).expect("fixture JSON");
        let duplicate = canonical.replacen(
            "\"schema\":",
            "\"schema\":\"dclutch/product-spline-authoring-input/v1\",\"schema\":",
            1,
        );
        let error = serde_json::from_str::<InputV1>(&duplicate)
            .expect_err("duplicate authority must refuse")
            .to_string();
        assert!(error.contains("duplicate field"), "{error}");
    }

    #[test]
    fn relative_or_duplicate_output_and_forged_gate_refuse_before_artifacts() {
        assert!(
            validate_output_path(Path::new("relative/output"))
                .expect_err("relative output")
                .0
                .contains("output/path-relative")
        );
        assert!(
            parse_arguments(vec![
                "--input".into(),
                "/tmp/a".into(),
                "--output-dir".into(),
                "/tmp/b".into(),
                "--output-dir".into(),
                "/tmp/c".into(),
            ])
            .expect_err("duplicate output")
            .0
            .contains("argument/duplicate")
        );
        let mut fixture = fixture();
        fixture.price_gate_certificate_hex.replace_range(
            PRICE_GATE_PRICES_OFFSET_V1 * 2..PRICE_GATE_PRICES_OFFSET_V1 * 2 + 2,
            "02",
        );
        let parsed = parse_input(fixture).expect("structural input");
        assert!(
            compile(&parsed)
                .expect_err("forged gate")
                .0
                .contains("PriceGate")
        );
    }
}
