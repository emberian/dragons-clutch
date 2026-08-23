//! Pure offline transport for the canonical production payoff compiler.
//!
//! Both the CLI and HTTP adapter call the same function. The adapter decodes
//! exact rational decimal-string JSON, calls the Rust semantic owners, and
//! returns an untrusted proposal. It has no RPC client, persistence, wallet,
//! signer, transaction builder, or registration authority.

use crate::{bus::Bus, http, Result};
use clutch_bspline::EdgePolicy;
use clutch_bspline_shape_compiler::artifact::NativeShapeCertificateV1;
use clutch_bspline_shape_compiler::exact_market::{
    bind_exact_market_bundle_v5, compile_exact_market_v1, ExactMarketCompilerRequestV1,
    ExactMarketCoordinateCoverageV1, ExactMarketSearchOutcomeV1,
    COMPILED_PRODUCT_SERIES_BUNDLE_V5_ARTIFACT_KIND, EXACT_MARKET_BUNDLE_SIDECAR_BYTES_V1,
};
use clutch_bspline_shape_compiler::production::{
    compile_production_payoff_v1, AnalyticSmoothPayoffDefinitionV1,
    ExactCategoricalPayoffDefinitionV1, ExactSmoothPayoffDefinitionV1,
    ProductionPayoffDefinitionV1, ProductionPayoffEvidenceV1, SmoothNativeBasisDefinitionV1,
};
use clutch_bspline_shape_compiler::{Shape, SpanStatus};
use clutch_product_series::{
    assemble_compiled_product_series_bundle_v5, CompiledProductSeriesBundleV5, ContentId,
    EvidenceOnlyRecoveryPolicyV1, FixedCodec, MarketGenesisProfileV2, PriceMeasurePolicyV1,
    ProductSeriesBundleInputsV5, ProductTemplateV4, RegistryCapabilityProfileV4,
    RegistryProgramReleaseV2, SeriesAttachmentPlanV4, SeriesFundingQuoteV4,
    SeriesFundingTermsV2, SeriesPlanV5, COMPILED_PRODUCT_SERIES_BUNDLE_V5_BYTES,
};
use num_bigint::BigInt;
use num_rational::BigRational;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use solana_address::Address;

pub const ENDPOINT: &str = "/v1/compiler/product-exact-market";
pub const MAX_REQUEST_BYTES: usize = 320 * 1024;
const REQUEST_SCHEMA: &str = "dragons-clutch/compiler/product-exact-market-request/v1";
const DEFINITION_SCHEMA: &str = "dragons-clutch/compiler/production-payoff-definition/v1";
const PROPOSAL_SCHEMA: &str = "dragons-clutch/compiler/product-exact-market-proposal/v1";

type CompileResult<T> = std::result::Result<T, String>;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CompileRequest {
    schema: String,
    expected_compiler_release_sha256: String,
    program_id: String,
    definition: DefinitionWire,
    bundle_inputs: BundleInputsWire,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exact_market_search: Option<ExactMarketSearchWire>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DefinitionWire {
    schema: String,
    product_terms_id: String,
    kind: String,
    definition: Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BundleInputsWire {
    registry_program_release_v2_bytes_hex: String,
    registry_capability_profile_v4_bytes_hex: String,
    source_release_manifest_id: String,
    evidence_only_recovery_policy_v1_bytes_hex: String,
    product_template_v4_bytes_hex: String,
    price_measure_policy_v1_bytes_hex: String,
    market_genesis_profile_v2_bytes_hex: String,
    series_funding_quote_v4_bytes_hex: String,
    series_attachment_plan_v4_bytes_hex: String,
    series_plan_v5_bytes_hex: String,
    series_funding_terms_v2_bytes_hex: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ExactMarketSearchWire {
    market_id: String,
    price_id: String,
    prices: Vec<String>,
    coordinates: Vec<String>,
    maximum_subset_evaluations_per_support: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RationalWire {
    numerator: String,
    denominator: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CategoricalWire {
    coordinate_domain_min: String,
    coordinate_domain_max: String,
    knots: Vec<String>,
    cell_payouts: Vec<Vec<RationalWire>>,
    ambiguity_policy_registry_value: String,
    edge_policy_registry_value: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SmoothBasisWire {
    degree: String,
    coordinate_domain_min: String,
    coordinate_domain_max: String,
    payout_denominator: String,
    knots: Vec<String>,
    resolved_edge_policy: String,
    ambiguity_policy_registry_value: String,
    edge_policy_registry_value: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ExactSmoothWire {
    basis: SmoothBasisWire,
    control_values: Vec<RationalWire>,
    maximum_liability: RationalWire,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AnalyticSmoothWire {
    basis: SmoothBasisWire,
    shape: AnalyticShapeWire,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum AnalyticShapeWire {
    HardRange {
        low: String,
        high: String,
        height: String,
    },
    UpperTail {
        strike: String,
        height: String,
    },
    LowerTail {
        strike: String,
        height: String,
    },
    Triangle {
        left: String,
        peak: String,
        right: String,
        height: String,
    },
    CappedCall {
        low: String,
        high: String,
        height: String,
    },
    CappedPut {
        low: String,
        high: String,
        height: String,
    },
    Gaussian {
        center: String,
        sigma: String,
        height: String,
    },
}

fn canonical_unsigned(text: &str, name: &str) -> CompileResult<()> {
    if text.is_empty()
        || (text != "0" && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!(
            "{name} must be a canonical unsigned decimal string"
        ));
    }
    Ok(())
}

fn parse_unsigned<T>(text: &str, name: &str) -> CompileResult<T>
where
    T: std::str::FromStr,
{
    canonical_unsigned(text, name)?;
    text.parse::<T>()
        .map_err(|_| format!("{name} exceeds its exact integer width"))
}

fn parse_rational(value: RationalWire, name: &str) -> CompileResult<BigRational> {
    if value.numerator.len() > 4096 || value.denominator.len() > 4096 {
        return Err(format!("{name} exceeds the rational transport bound"));
    }
    let numerator_ok = value.numerator == "0"
        || value.numerator.strip_prefix('-').is_some_and(|digits| {
            !digits.is_empty()
                && !digits.starts_with('0')
                && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
        || (!value.numerator.starts_with('0')
            && value.numerator.bytes().all(|byte| byte.is_ascii_digit()));
    if !numerator_ok {
        return Err(format!("{name}.numerator is not canonical signed decimal"));
    }
    canonical_unsigned(&value.denominator, &format!("{name}.denominator"))?;
    let numerator = BigInt::parse_bytes(value.numerator.as_bytes(), 10)
        .ok_or_else(|| format!("{name}.numerator is invalid"))?;
    let denominator = BigInt::parse_bytes(value.denominator.as_bytes(), 10)
        .ok_or_else(|| format!("{name}.denominator is invalid"))?;
    if denominator <= BigInt::from(0_u8) {
        return Err(format!("{name}.denominator must be positive"));
    }
    let rational = BigRational::new(numerator, denominator);
    if rational.numer().to_string() != value.numerator
        || rational.denom().to_string() != value.denominator
    {
        return Err(format!("{name} must be reduced to canonical lowest terms"));
    }
    Ok(rational)
}

fn parse_smooth_basis(value: SmoothBasisWire) -> CompileResult<SmoothNativeBasisDefinitionV1> {
    let resolved_edge_policy = match value.resolved_edge_policy.as_str() {
        "clamp" => EdgePolicy::Clamp,
        "refuse" => EdgePolicy::Refuse,
        _ => return Err("basis.resolvedEdgePolicy must be clamp or refuse".to_string()),
    };
    let knots = value
        .knots
        .iter()
        .enumerate()
        .map(|(index, knot)| parse_unsigned(knot, &format!("basis.knots[{index}]")))
        .collect::<CompileResult<Vec<u128>>>()?;
    Ok(SmoothNativeBasisDefinitionV1 {
        degree: parse_unsigned(&value.degree, "basis.degree")?,
        coordinate_domain_min: parse_unsigned(
            &value.coordinate_domain_min,
            "basis.coordinateDomainMin",
        )?,
        coordinate_domain_max: parse_unsigned(
            &value.coordinate_domain_max,
            "basis.coordinateDomainMax",
        )?,
        payout_denominator: parse_unsigned(&value.payout_denominator, "basis.payoutDenominator")?,
        knots,
        resolved_edge_policy,
        ambiguity_policy_registry_value: parse_unsigned(
            &value.ambiguity_policy_registry_value,
            "basis.ambiguityPolicyRegistryValue",
        )?,
        edge_policy_registry_value: parse_unsigned(
            &value.edge_policy_registry_value,
            "basis.edgePolicyRegistryValue",
        )?,
    })
}

fn parse_shape(value: AnalyticShapeWire) -> CompileResult<Shape> {
    Ok(match value {
        AnalyticShapeWire::HardRange { low, high, height } => Shape::HardRange {
            low: parse_unsigned(&low, "shape.low")?,
            high: parse_unsigned(&high, "shape.high")?,
            height: parse_unsigned(&height, "shape.height")?,
        },
        AnalyticShapeWire::UpperTail { strike, height } => Shape::UpperTail {
            strike: parse_unsigned(&strike, "shape.strike")?,
            height: parse_unsigned(&height, "shape.height")?,
        },
        AnalyticShapeWire::LowerTail { strike, height } => Shape::LowerTail {
            strike: parse_unsigned(&strike, "shape.strike")?,
            height: parse_unsigned(&height, "shape.height")?,
        },
        AnalyticShapeWire::Triangle {
            left,
            peak,
            right,
            height,
        } => Shape::Triangle {
            left: parse_unsigned(&left, "shape.left")?,
            peak: parse_unsigned(&peak, "shape.peak")?,
            right: parse_unsigned(&right, "shape.right")?,
            height: parse_unsigned(&height, "shape.height")?,
        },
        AnalyticShapeWire::CappedCall { low, high, height } => Shape::CappedCall {
            low: parse_unsigned(&low, "shape.low")?,
            high: parse_unsigned(&high, "shape.high")?,
            height: parse_unsigned(&height, "shape.height")?,
        },
        AnalyticShapeWire::CappedPut { low, high, height } => Shape::CappedPut {
            low: parse_unsigned(&low, "shape.low")?,
            high: parse_unsigned(&high, "shape.high")?,
            height: parse_unsigned(&height, "shape.height")?,
        },
        AnalyticShapeWire::Gaussian {
            center,
            sigma,
            height,
        } => Shape::Gaussian {
            center: parse_unsigned(&center, "shape.center")?,
            sigma: parse_unsigned(&sigma, "shape.sigma")?,
            height: parse_unsigned(&height, "shape.height")?,
        },
    })
}

fn parse_definition(value: &DefinitionWire) -> CompileResult<ProductionPayoffDefinitionV1> {
    if value.schema != DEFINITION_SCHEMA {
        return Err("definition.schema is not production-payoff-definition/v1".to_string());
    }
    match value.kind.as_str() {
        "exact-categorical" => {
            let raw: CategoricalWire = serde_json::from_value(value.definition.clone())
                .map_err(|error| format!("invalid exact categorical definition: {error}"))?;
            let knots = raw
                .knots
                .iter()
                .enumerate()
                .map(|(index, knot)| parse_unsigned(knot, &format!("definition.knots[{index}]")))
                .collect::<CompileResult<Vec<u128>>>()?;
            let cell_payouts = raw
                .cell_payouts
                .into_iter()
                .enumerate()
                .map(|(row_index, row)| {
                    row.into_iter()
                        .enumerate()
                        .map(|(column_index, rational)| {
                            parse_rational(
                                rational,
                                &format!("definition.cellPayouts[{row_index}][{column_index}]"),
                            )
                        })
                        .collect::<CompileResult<Vec<BigRational>>>()
                })
                .collect::<CompileResult<Vec<Vec<BigRational>>>>()?;
            Ok(ProductionPayoffDefinitionV1::ExactCategorical(
                ExactCategoricalPayoffDefinitionV1 {
                    coordinate_domain_min: parse_unsigned(
                        &raw.coordinate_domain_min,
                        "definition.coordinateDomainMin",
                    )?,
                    coordinate_domain_max: parse_unsigned(
                        &raw.coordinate_domain_max,
                        "definition.coordinateDomainMax",
                    )?,
                    knots,
                    cell_payouts,
                    ambiguity_policy_registry_value: parse_unsigned(
                        &raw.ambiguity_policy_registry_value,
                        "definition.ambiguityPolicyRegistryValue",
                    )?,
                    edge_policy_registry_value: parse_unsigned(
                        &raw.edge_policy_registry_value,
                        "definition.edgePolicyRegistryValue",
                    )?,
                },
            ))
        }
        "exact-smooth" => {
            let raw: ExactSmoothWire = serde_json::from_value(value.definition.clone())
                .map_err(|error| format!("invalid exact smooth definition: {error}"))?;
            let controls = raw
                .control_values
                .into_iter()
                .enumerate()
                .map(|(index, rational)| {
                    parse_rational(rational, &format!("definition.controlValues[{index}]"))
                })
                .collect::<CompileResult<Vec<BigRational>>>()?;
            Ok(ProductionPayoffDefinitionV1::ExactSmooth(
                ExactSmoothPayoffDefinitionV1 {
                    basis: parse_smooth_basis(raw.basis)?,
                    control_values: controls,
                    maximum_liability: parse_rational(
                        raw.maximum_liability,
                        "definition.maximumLiability",
                    )?,
                },
            ))
        }
        "analytic-smooth" => {
            let raw: AnalyticSmoothWire = serde_json::from_value(value.definition.clone())
                .map_err(|error| format!("invalid analytic smooth definition: {error}"))?;
            Ok(ProductionPayoffDefinitionV1::AnalyticSmooth(
                AnalyticSmoothPayoffDefinitionV1 {
                    basis: parse_smooth_basis(raw.basis)?,
                    shape: parse_shape(raw.shape)?,
                },
            ))
        }
        _ => Err("definition.kind is not a production payoff variant".to_string()),
    }
}

fn decode_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn decode_hex(text: &str, expected_bytes: usize, name: &str) -> CompileResult<Vec<u8>> {
    if text.len() != expected_bytes.saturating_mul(2) {
        return Err(format!(
            "{name} must contain exactly {expected_bytes} bytes"
        ));
    }
    let mut output = Vec::with_capacity(expected_bytes);
    for pair in text.as_bytes().chunks_exact(2) {
        let high = decode_nibble(pair[0])
            .ok_or_else(|| format!("{name} must be lowercase hexadecimal"))?;
        let low = decode_nibble(pair[1])
            .ok_or_else(|| format!("{name} must be lowercase hexadecimal"))?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

fn parse_id(text: &str, name: &str) -> CompileResult<ContentId> {
    let bytes = decode_hex(text, 32, name)?;
    let array: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("{name} must contain 32 bytes"))?;
    let id = ContentId::from_bytes(array);
    if id.is_zero() {
        return Err(format!("{name} must be nonzero"));
    }
    Ok(id)
}

fn decode_body<T: FixedCodec>(text: &str, name: &str) -> CompileResult<T> {
    let bytes = decode_hex(text, T::ENCODED_LEN, name)?;
    T::decode(&bytes).map_err(|error| format!("{name} is not canonical: {error:?}"))
}

fn hex(bytes: &[u8]) -> String {
    clutch_sbf_harness::hex_encode(bytes)
}

fn id_hex(bytes: [u8; 32]) -> String {
    hex(&bytes)
}

fn rational_json(value: &BigRational) -> Value {
    json!({
        "numerator": value.numer().to_string(),
        "denominator": value.denom().to_string(),
    })
}

fn canonical_json(value: &Value, output: &mut String) -> CompileResult<()> {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(boolean) => output.push_str(if *boolean { "true" } else { "false" }),
        Value::String(text) => output.push_str(
            &serde_json::to_string(text)
                .map_err(|error| format!("cannot canonicalize JSON string: {error}"))?,
        ),
        Value::Array(values) => {
            output.push('[');
            for (index, item) in values.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                canonical_json(item, output)?;
            }
            output.push(']');
        }
        Value::Object(values) => {
            output.push('{');
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                output.push_str(
                    &serde_json::to_string(key)
                        .map_err(|error| format!("cannot canonicalize JSON key: {error}"))?,
                );
                output.push(':');
                canonical_json(&values[key], output)?;
            }
            output.push('}');
        }
        Value::Number(_) => {
            return Err("definition contains a JSON number; exact integers must be strings".into())
        }
    }
    Ok(())
}

fn bundle_json(
    bundle: &CompiledProductSeriesBundleV5,
    program_id: &Address,
) -> CompileResult<Value> {
    let mut bytes = [0_u8; COMPILED_PRODUCT_SERIES_BUNDLE_V5_BYTES];
    bundle
        .encode_into(&mut bytes)
        .map_err(|error| format!("cannot encode compiled Product/Series bundle: {error:?}"))?;
    let bundle_id = bundle
        .id()
        .map_err(|error| format!("cannot identify compiled Product/Series bundle: {error:?}"))?;
    let kind = [COMPILED_PRODUCT_SERIES_BUNDLE_V5_ARTIFACT_KIND];
    let digest = bundle_id.bytes();
    let (artifact_pda, artifact_bump) = Address::find_program_address(
        &[
            clutch_sbf::seeds::SEED_PRODUCT_ARTIFACT_V1,
            &kind,
            &digest,
        ],
        program_id,
    );
    Ok(json!({
        "id": id_hex(bundle_id.bytes()),
        "bytesHex": hex(&bytes),
        "artifact": {
            "kind": COMPILED_PRODUCT_SERIES_BUNDLE_V5_ARTIFACT_KIND.to_string(),
            "context": id_hex(ContentId::ZERO.bytes()),
            "exactBodyBytes": COMPILED_PRODUCT_SERIES_BUNDLE_V5_BYTES.to_string(),
            "programId": program_id.to_string(),
            "pda": artifact_pda.to_string(),
            "bump": artifact_bump.to_string(),
        },
        "identities": {
            "registryReleaseId": id_hex(bundle.registry_release_id.bytes()),
            "capabilityProfileId": id_hex(bundle.capability_profile_id.bytes()),
            "sourceReleaseManifestId": id_hex(bundle.source_release_manifest_id.bytes()),
            "sourcePlaneContractId": id_hex(bundle.source_plane_contract_id.bytes()),
            "sourceSpecId": id_hex(bundle.source_spec_id.bytes()),
            "summaryProgramId": id_hex(bundle.summary_program_id.bytes()),
            "productCompilerReleaseId": id_hex(bundle.product_compiler_release_id.bytes()),
            "nativeClaimBasisId": id_hex(bundle.native_claim_basis_id.bytes()),
            "evidenceOnlyRecoveryPolicyId": id_hex(bundle.evidence_only_recovery_policy_id.bytes()),
            "productTemplateId": id_hex(bundle.product_template_id.bytes()),
            "priceMeasurePolicyId": id_hex(bundle.price_measure_policy_id.bytes()),
            "marketGenesisProfileId": id_hex(bundle.market_genesis_profile_id.bytes()),
            "fundingQuoteId": id_hex(bundle.funding_quote_id.bytes()),
            "attachmentPlanId": id_hex(bundle.attachment_plan_id.bytes()),
            "seriesPlanId": id_hex(bundle.series_plan_id.bytes()),
            "fundingTermsId": id_hex(bundle.funding_terms_id.bytes()),
        }
    }))
}

fn parse_exact_market_search(
    value: &ExactMarketSearchWire,
    product_terms_id: ContentId,
) -> CompileResult<ExactMarketCompilerRequestV1> {
    let market_id = parse_id(&value.market_id, "exactMarketSearch.marketId")?;
    let price_id = parse_id(&value.price_id, "exactMarketSearch.priceId")?;
    let prices = value
        .prices
        .iter()
        .enumerate()
        .map(|(index, price)| {
            parse_unsigned(price, &format!("exactMarketSearch.prices[{index}]"))
        })
        .collect::<CompileResult<Vec<u64>>>()?;
    let coordinates = value
        .coordinates
        .iter()
        .enumerate()
        .map(|(index, coordinate)| {
            parse_unsigned(
                coordinate,
                &format!("exactMarketSearch.coordinates[{index}]"),
            )
        })
        .collect::<CompileResult<Vec<u128>>>()?;
    let budget = parse_unsigned(
        &value.maximum_subset_evaluations_per_support,
        "exactMarketSearch.maximumSubsetEvaluationsPerSupport",
    )?;
    ExactMarketCompilerRequestV1::new(
        market_id,
        product_terms_id,
        price_id,
        &prices,
        &coordinates,
        budget,
    )
    .map_err(|error| format!("exact market request refused: {error:?}"))
}

fn exact_market_json(
    compiled: &clutch_bspline_shape_compiler::production::CompiledProductionPayoffV1,
    bundle: &CompiledProductSeriesBundleV5,
    value: &ExactMarketSearchWire,
    product_terms_id: ContentId,
) -> CompileResult<Value> {
    let request = parse_exact_market_search(value, product_terms_id)?;
    let output = compile_exact_market_v1(compiled, request)
        .map_err(|error| format!("exact market compiler refused input: {error:?}"))?;
    let sidecar = bind_exact_market_bundle_v5(compiled, bundle, &output)
        .map_err(|error| format!("BundleV5 exact-market join refused: {error:?}"))?;
    let mut sidecar_bytes = [0_u8; EXACT_MARKET_BUNDLE_SIDECAR_BYTES_V1];
    sidecar
        .encode_into(&mut sidecar_bytes)
        .map_err(|error| format!("cannot encode exact-market BundleV5 sidecar: {error:?}"))?;
    let sidecar_id = sidecar
        .content_id()
        .map_err(|error| format!("cannot identify exact-market BundleV5 sidecar: {error:?}"))?;
    let outcome = match output.manifest.outcome() {
        ExactMarketSearchOutcomeV1::Solved => "solved",
        ExactMarketSearchOutcomeV1::Unsupported => "unsupported",
        ExactMarketSearchOutcomeV1::OutOfProfile => "out-of-profile",
        ExactMarketSearchOutcomeV1::WorkTruncated => "work-truncated",
    };
    let coverage = match output.manifest.coverage() {
        ExactMarketCoordinateCoverageV1::FullIntegerDomain => "full-integer-domain",
        ExactMarketCoordinateCoverageV1::DeclaredCoordinateSubset => {
            "declared-coordinate-subset"
        }
    };
    let mut support_work = Vec::with_capacity(usize::from(output.manifest.outcome_count()));
    let mut support = 1_u8;
    while support <= output.manifest.outcome_count() {
        support_work.push(json!({
            "support": support.to_string(),
            "evaluations": output.manifest.evaluations_for_support(support)
                .ok_or_else(|| "exact-market report omitted an active support".to_string())?
                .to_string(),
            "exactButUnrepresentable": output.manifest
                .exact_but_unrepresentable_for_support(support)
                .ok_or_else(|| "exact-market report omitted an active support".to_string())?
                .to_string(),
        }));
        support = support
            .checked_add(1)
            .ok_or_else(|| "exact-market support counter overflowed".to_string())?;
    }
    let certificate = output.certificate_bytes.as_ref().map_or(Value::Null, |bytes| {
        json!({
            "outputId": id_hex(output.manifest.certificate_output_id().bytes()),
            "bytesHex": hex(bytes),
        })
    });
    Ok(json!({
        "authority": "untrusted-compiler-sidecar",
        "registrationAuthority": false,
        "outcome": outcome,
        "coverage": coverage,
        "completeFullDomainNegative": output.manifest.is_complete_full_domain_negative(),
        "claims": {
            "uniquePrice": false,
            "fairValue": false,
            "optimalClearing": false,
        },
        "bindings": {
            "marketId": id_hex(output.manifest.market_id().bytes()),
            "productTermsId": id_hex(output.manifest.product_terms_id().bytes()),
            "nativeClaimBasisId": id_hex(output.manifest.native_claim_basis_id().bytes()),
            "priceId": id_hex(output.manifest.price_id().bytes()),
            "bundleV5Id": id_hex(sidecar.bundle_v5_id().bytes()),
        },
        "target": {
            "outcomeCount": output.manifest.outcome_count().to_string(),
            "payoutDenominator": output.manifest.payout_denominator().to_string(),
            "prices": output.manifest.prices()[..usize::from(output.manifest.outcome_count())]
                .iter().map(u64::to_string).collect::<Vec<_>>(),
        },
        "search": {
            "coordinateDomainMin": output.manifest.coordinate_domain_min().to_string(),
            "coordinateDomainMax": output.manifest.coordinate_domain_max().to_string(),
            "coordinates": output.manifest.coordinates()
                [..usize::from(output.manifest.coordinate_count())]
                .iter().map(u128::to_string).collect::<Vec<_>>(),
            "maximumSubsetEvaluationsPerSupport": output.manifest
                .maximum_subset_evaluations_per_support().to_string(),
            "exhaustedThroughSupport": output.manifest.exhausted_through_support().to_string(),
            "truncatedSupport": output.manifest.truncated_support().to_string(),
            "workBySupport": support_work,
        },
        "workManifest": {
            "id": id_hex(output.manifest_id.bytes()),
            "bytesHex": hex(&output.manifest_bytes),
        },
        "certificate": certificate,
        "bundleV5Sidecar": {
            "id": id_hex(sidecar_id.bytes()),
            "bytesHex": hex(&sidecar_bytes),
            "bundleArtifactKind": sidecar.bundle_artifact_kind().to_string(),
            "bundleArtifactContext": id_hex(sidecar.bundle_artifact_context().bytes()),
        },
    }))
}

#[derive(Debug)]
struct CompilerService {
    compiler_release_sha256: String,
}

impl CompilerService {
    fn new(compiler_release_sha256: String) -> CompileResult<Self> {
        let bytes = decode_hex(
            &compiler_release_sha256,
            32,
            "configured compilerReleaseSha256",
        )?;
        if bytes.iter().all(|byte| *byte == 0) {
            return Err("configured compilerReleaseSha256 must be nonzero".to_string());
        }
        Ok(Self {
            compiler_release_sha256,
        })
    }

    fn compile_request(&self, body: &[u8]) -> CompileResult<Value> {
        if body.is_empty() || body.len() > MAX_REQUEST_BYTES {
            return Err(format!(
                "request must contain 1..={MAX_REQUEST_BYTES} UTF-8 JSON bytes"
            ));
        }
        let request: CompileRequest = serde_json::from_slice(body)
            .map_err(|error| format!("invalid Product exact-market request JSON: {error}"))?;
        if request.schema != REQUEST_SCHEMA {
            return Err("request.schema is not product-exact-market-request/v1".to_string());
        }
        let program_id = Address::from_str(&request.program_id)
            .map_err(|_| "programId is not a canonical Solana address".to_string())?;
        let expected_compiler_release = decode_hex(
            &request.expected_compiler_release_sha256,
            32,
            "expectedCompilerReleaseSha256",
        )?;
        if expected_compiler_release.iter().all(|byte| *byte == 0) {
            return Err("expectedCompilerReleaseSha256 must be nonzero".to_string());
        }
        if request.expected_compiler_release_sha256 != self.compiler_release_sha256 {
            return Err("expectedCompilerReleaseSha256 differs from the configured compiler service release".to_string());
        }
        let request_value = serde_json::to_value(&request)
            .map_err(|error| format!("cannot encode validated compiler request: {error}"))?;
        let mut canonical_request = String::new();
        canonical_json(&request_value, &mut canonical_request)?;
        let request_sha256 = solana_sha256_hasher::hash(canonical_request.as_bytes()).to_bytes();
        let product_terms_id = parse_id(&request.definition.product_terms_id, "productTermsId")?;
        let definition = parse_definition(&request.definition)?;
        let definition_value = serde_json::to_value(&request.definition)
            .map_err(|error| format!("cannot encode validated definition: {error}"))?;
        let mut canonical = String::new();
        canonical_json(&definition_value, &mut canonical)?;
        if canonical.len() > 262_144 {
            return Err("canonical payoff definition exceeds 262144 UTF-8 bytes".to_string());
        }
        let input_sha256 = solana_sha256_hasher::hash(canonical.as_bytes()).to_bytes();

        let compiled = compile_production_payoff_v1(product_terms_id, definition)
            .map_err(|error| format!("production payoff compiler refused input: {error:?}"))?;

        let registry_release: RegistryProgramReleaseV2 = decode_body(
            &request
                .bundle_inputs
                .registry_program_release_v2_bytes_hex,
            "bundleInputs.registryProgramReleaseV2BytesHex",
        )?;
        let registry_profile: RegistryCapabilityProfileV4 = decode_body(
            &request
                .bundle_inputs
                .registry_capability_profile_v4_bytes_hex,
            "bundleInputs.registryCapabilityProfileV4BytesHex",
        )?;
        let registry_release_id = registry_release
            .id()
            .map_err(|error| format!("RegistryProgramReleaseV2 identity refused: {error:?}"))?;
        if registry_profile.registry_release_id() != registry_release_id {
            return Err(
                "RegistryCapabilityProfileV4 is not bound to RegistryProgramReleaseV2"
                    .to_string(),
            );
        }
        let registry = registry_profile
            .projection()
            .map_err(|error| format!("registry capability projection refused: {error:?}"))?;
        let source_release_manifest_id = parse_id(
            &request.bundle_inputs.source_release_manifest_id,
            "bundleInputs.sourceReleaseManifestId",
        )?;
        let recovery: EvidenceOnlyRecoveryPolicyV1 = decode_body(
            &request
                .bundle_inputs
                .evidence_only_recovery_policy_v1_bytes_hex,
            "bundleInputs.evidenceOnlyRecoveryPolicyV1BytesHex",
        )?;
        let template: ProductTemplateV4 = decode_body(
            &request.bundle_inputs.product_template_v4_bytes_hex,
            "bundleInputs.productTemplateV4BytesHex",
        )?;
        let price_policy: PriceMeasurePolicyV1 = decode_body(
            &request.bundle_inputs.price_measure_policy_v1_bytes_hex,
            "bundleInputs.priceMeasurePolicyV1BytesHex",
        )?;
        let genesis: MarketGenesisProfileV2 = decode_body(
            &request.bundle_inputs.market_genesis_profile_v2_bytes_hex,
            "bundleInputs.marketGenesisProfileV2BytesHex",
        )?;
        let funding_quote: SeriesFundingQuoteV4 = decode_body(
            &request.bundle_inputs.series_funding_quote_v4_bytes_hex,
            "bundleInputs.seriesFundingQuoteV4BytesHex",
        )?;
        let attachment: SeriesAttachmentPlanV4 = decode_body(
            &request.bundle_inputs.series_attachment_plan_v4_bytes_hex,
            "bundleInputs.seriesAttachmentPlanV4BytesHex",
        )?;
        let series: SeriesPlanV5 = decode_body(
            &request.bundle_inputs.series_plan_v5_bytes_hex,
            "bundleInputs.seriesPlanV5BytesHex",
        )?;
        let funding_terms: SeriesFundingTermsV2 = decode_body(
            &request.bundle_inputs.series_funding_terms_v2_bytes_hex,
            "bundleInputs.seriesFundingTermsV2BytesHex",
        )?;
        let bundle = assemble_compiled_product_series_bundle_v5(ProductSeriesBundleInputsV5 {
            registry: &registry,
            source_release_manifest_id,
            basis: &compiled.native_claim_basis,
            recovery: &recovery,
            template: &template,
            price_policy: &price_policy,
            genesis: &genesis,
            funding_quote: &funding_quote,
            attachment: &attachment,
            series: &series,
            funding_terms: &funding_terms,
        })
        .map_err(|error| format!("canonical Product/Series BundleV5 join refused: {error:?}"))?;

        let exact_market = request
            .exact_market_search
            .as_ref()
            .map(|search| exact_market_json(&compiled, &bundle, search, product_terms_id))
            .transpose()?;

        let (span_status, certificate, bounds, subdivision_depth) = match &compiled.evidence {
            ProductionPayoffEvidenceV1::ExactCategoricalBasis => {
                ("exact-in-span", Value::Null, Vec::new(), Value::Null)
            }
            ProductionPayoffEvidenceV1::ExactSmooth {
                certificate_bytes,
                certificate_id,
                ..
            } => (
                "exact-in-span",
                json!({
                    "id": id_hex(certificate_id.bytes()),
                    "bytesHex": hex(certificate_bytes),
                }),
                Vec::new(),
                Value::Null,
            ),
            ProductionPayoffEvidenceV1::AnalyticSmooth {
                status,
                certificate_bytes,
                certificate_id,
            } => {
                let certificate_value = NativeShapeCertificateV1::decode(certificate_bytes)
                    .map_err(|error| format!("analytic certificate decode refused: {error:?}"))?;
                let error = &certificate_value.compilation.certificate;
                let bounds = vec![
                    json!({"name":"spline-sup-lower", "value":rational_json(&error.spline_sup_lower)}),
                    json!({"name":"spline-sup-upper", "value":rational_json(&error.spline_sup_upper)}),
                    json!({"name":"spline-l1-lower", "value":rational_json(&error.spline_l1_lower)}),
                    json!({"name":"spline-l1-upper", "value":rational_json(&error.spline_l1_upper)}),
                    json!({"name":"consensus-quantization-sup-upper", "value":rational_json(&error.consensus_quantization_sup_upper)}),
                    json!({"name":"consensus-sup-upper", "value":rational_json(&error.consensus_sup_upper)}),
                    json!({"name":"consensus-l1-upper", "value":rational_json(&error.consensus_l1_upper)}),
                    json!({"name":"coefficient-sample-sup-upper", "value":rational_json(&error.coefficient_sample_sup_upper)}),
                ];
                (
                    match status {
                        SpanStatus::ExactInSpan => "exact-in-span",
                        SpanStatus::CertifiedApproximation => "certified-approximation",
                    },
                    json!({
                        "id": id_hex(certificate_id.bytes()),
                        "bytesHex": hex(certificate_bytes),
                    }),
                    bounds,
                    Value::String(error.subdivision_depth.to_string()),
                )
            }
        };

        Ok(json!({
            "schema": PROPOSAL_SCHEMA,
            "authority": "untrusted-compiler-proposal",
            "registrationAuthority": false,
            "compilerReleaseSha256": self.compiler_release_sha256.as_str(),
            "programId": request.program_id,
            "requestCanonicalSha256": id_hex(request_sha256),
            "inputCanonicalSha256": id_hex(input_sha256),
            "productTermsId": request.definition.product_terms_id,
            "classification": request.definition.kind,
            "spanStatus": span_status,
            "nativeClaimBasis": {
                "id": id_hex(compiled.native_claim_basis_id.bytes()),
                "bytesHex": hex(&compiled.native_claim_basis_bytes),
            },
            "certificate": certificate,
            "bounds": bounds,
            "subdivisionDepth": subdivision_depth,
            "compiledProductSeriesBundleV5": bundle_json(&bundle, &program_id)?,
            "exactMarket": exact_market,
        }))
    }
}

fn error_json(detail: String) -> Value {
    json!({
        "schema": "dragons-clutch/compiler/refusal/v1",
        "authority": "pure-offline-compiler",
        "registrationAuthority": false,
        "error": "compiler-refused",
        "detail": detail,
    })
}

/// Read one bounded request from stdin and write one proposal to stdout.
pub fn compile_cli(compiler_release_sha256: String) -> Result<()> {
    let service = CompilerService::new(compiler_release_sha256)
        .map_err(|error| format!("compiler configuration refused: {error}"))?;
    let mut body = Vec::new();
    std::io::stdin()
        .lock()
        .take(u64::try_from(MAX_REQUEST_BYTES)?.saturating_add(1))
        .read_to_end(&mut body)?;
    if body.len() > MAX_REQUEST_BYTES {
        return Err(format!("compiler request exceeds {MAX_REQUEST_BYTES} bytes").into());
    }
    let proposal = service
        .compile_request(&body)
        .map_err(|error| format!("compiler refused: {error}"))?;
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &proposal)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

/// Serve Glass plus the pure compiler endpoint on an exact loopback authority.
pub fn serve(port: u16, statics: PathBuf, compiler_release_sha256: String) -> Result<()> {
    let post_api = post_api(compiler_release_sha256)?;
    let server = http::Server::bind_pure(port, Bus::new(), statics, None, Some(post_api))?;
    println!(
        "Glass pure compiler listening on http://127.0.0.1:{} (no RPC, wallet, signing, submission, or persistence)",
        server.port()?
    );
    server.serve_forever();
    Ok(())
}

/// Build the serialized pure-compiler callback for a read-only chain server.
pub fn post_api(compiler_release_sha256: String) -> Result<http::PostApi> {
    let service = Arc::new(Mutex::new(
        CompilerService::new(compiler_release_sha256)
            .map_err(|error| format!("compiler configuration refused: {error}"))?,
    ));
    let post_api: http::PostApi = Arc::new(move |path, body| {
        if path != ENDPOINT {
            return None;
        }
        let service = service
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Some(match service.compile_request(body) {
            Ok(proposal) => http::JsonReadResponse {
                status: 200,
                body: proposal,
            },
            Err(error) => http::JsonReadResponse {
                status: 400,
                body: error_json(error),
            },
        })
    });
    Ok(post_api)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release() -> String {
        "11".repeat(32)
    }

    #[test]
    fn historical_payoff_endpoint_and_request_schema_are_fail_closed() {
        let api = post_api(release()).unwrap();
        assert!(api("/v1/compiler/production-payoff", b"{}").is_none());

        let service = CompilerService::new(release()).unwrap();
        let old = json!({
            "schema": "dragons-clutch/compiler/production-payoff-request/v1",
            "expectedCompilerReleaseSha256": release(),
            "definition": {},
            "bundleInputs": {},
        });
        assert!(service.compile_request(old.to_string().as_bytes()).is_err());
    }

    #[test]
    fn exact_market_wire_refuses_noncanonical_decimal_and_zero_identity() {
        let noncanonical = ExactMarketSearchWire {
            market_id: "01".repeat(32),
            price_id: "02".repeat(32),
            prices: vec!["07".to_string(), "25".to_string()],
            coordinates: vec!["0".to_string(), "1".to_string()],
            maximum_subset_evaluations_per_support: "1".to_string(),
        };
        assert!(parse_exact_market_search(&noncanonical, ContentId::from_bytes([3; 32]))
            .unwrap_err()
            .contains("canonical unsigned decimal"));

        let zero_market = ExactMarketSearchWire {
            market_id: "00".repeat(32),
            price_id: "02".repeat(32),
            prices: vec!["7".to_string(), "25".to_string()],
            coordinates: vec!["0".to_string(), "1".to_string()],
            maximum_subset_evaluations_per_support: "1".to_string(),
        };
        assert!(parse_exact_market_search(&zero_market, ContentId::from_bytes([3; 32]))
            .unwrap_err()
            .contains("must be nonzero"));
    }
}
