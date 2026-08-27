//! Canonical bytes at the native B-spline compiler/client boundary.
//!
//! This module is host tooling, not an on-chain parser.  It deliberately emits
//! the exact `Intent` bytes already owned by `clutch-solana-layout` rather than
//! defining a second market-creation wire format.  The standalone basis and
//! shape-certificate codecs are versioned, domain-separated compiler
//! artifacts.  A degree-zero basis remains a native degree-zero market; this
//! module has no compatibility-lowering mode and never calls
//! `compare_categorical_lowering`.

use std::vec::Vec;

use clutch_accumulator::MAX_VALUE;
use clutch_bspline::{BasisSpec, EdgePolicy, MAX_KNOTS, MAX_OUTCOMES};
use clutch_solana_layout::{
    account_len,
    artifact::{ArtifactKind, ARTIFACT_CHUNK_BYTES},
    CodecError, Intent, TermsAccount, MAX_INTENT_BYTES,
};
use num_bigint::{BigInt, Sign};
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};
use sha2::{Digest, Sha256};

use crate::{
    compile, Compilation, Construction, Error as CompilerError, ErrorCertificate, Shape, SpanStatus,
};

/// `DCBASV01`: canonical native basis artifact, revision one.
pub const BASIS_MAGIC_V1: [u8; 8] = *b"DCBASV01";
/// `DCSHCV01`: canonical native shape certificate, revision one.
pub const CERTIFICATE_MAGIC_V1: [u8; 8] = *b"DCSHCV01";
/// First and only basis artifact schema understood here.
pub const BASIS_SCHEMA_VERSION_V1: u16 = 1;
/// First and only shape-certificate schema understood here.
pub const CERTIFICATE_SCHEMA_VERSION_V1: u16 = 1;
/// Compiler semantic version bound into every certificate.
pub const SHAPE_COMPILER_VERSION_V1: u16 = 1;
/// Frozen `clutch-bspline` evaluator semantic version.
pub const BASIS_EVALUATOR_VERSION_V1: u16 = 1;
/// Frozen largest-remainder basis quantization rule.
pub const WEIGHT_ROUND_VERSION_V1: u16 = 1;
/// Explicit semantic tag: native B-spline, including native degree zero.
pub const SEMANTIC_NATIVE_BSPLINE: u8 = 1;
/// Exact canonical basis artifact length.
pub const BASIS_SPEC_BYTES_V1: usize = 304;
/// Exact fixed shape-description length inside a certificate.
pub const SHAPE_BYTES_V1: usize = 64;
/// Fixed certificate bytes before exact-rational fields.
pub const CERTIFICATE_FIXED_BYTES_V1: usize =
    8 + 2 + 2 + 2 + 2 + 4 + 32 + 32 + BASIS_SPEC_BYTES_V1 + SHAPE_BYTES_V1 + 2 + 2;
/// Largest accepted magnitude of one rational integer, in bytes.
pub const MAX_RATIONAL_INTEGER_BYTES_V1: usize = 4_096;
/// Largest complete certificate accepted by this host codec.
pub const MAX_CERTIFICATE_BYTES_V1: usize = 256 * 1_024;
/// Current `CreateMarket` wire width.
pub const CREATE_MARKET_INTENT_BYTES: usize = 2 + 32 + 32 + 8 + 1 + 32 + 32;
/// Current `BeginArtifact` wire width.
pub const BEGIN_ARTIFACT_INTENT_BYTES: usize = 2 + 1 + 32 + 32 + 2 + 8;
/// Current fixed-width `WriteArtifact` wire width.
pub const WRITE_ARTIFACT_INTENT_BYTES: usize = 2 + 1 + 32 + 32 + 2 + 2 + ARTIFACT_CHUNK_BYTES;
/// Current `SealArtifact` wire width.
pub const SEAL_ARTIFACT_INTENT_BYTES: usize = 2 + 1 + 32 + 32 + 2;
/// Exact number of ordered Terms writes at 192 bytes per wire chunk.
pub const TERMS_ARTIFACT_WRITE_COUNT_V1: usize = account_len::TERMS.div_ceil(ARTIFACT_CHUNK_BYTES);
/// Runtime's shortest admitted staging lifetime, mirrored for offline refusal.
pub const MIN_UPLOAD_LIFETIME_SLOTS_V1: u64 = 8;
/// Runtime's longest admitted staging lifetime, mirrored for offline refusal.
pub const MAX_UPLOAD_LIFETIME_SLOTS_V1: u64 = 432_000;

const BASIS_DIGEST_DOMAIN_V1: &[u8] = b"dragons-clutch/basis-spec/v1";
const CERTIFICATE_DIGEST_DOMAIN_V1: &[u8] = b"dragons-clutch/shape-certificate/v1";

/// Canonical artifact refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtifactError {
    /// A fixed-width artifact was truncated.
    Truncated,
    /// Bytes remained after the one canonical value.
    TrailingBytes,
    /// A magic, version, semantic tag, or enum value was not recognized.
    InvalidDiscriminant,
    /// A reserved or inactive field was not canonical zero.
    NonCanonicalPadding,
    /// A length/count was impossible or exceeded the host bound.
    InvalidLength,
    /// A digest was zero or did not recompute.
    DigestMismatch,
    /// A rational used a non-minimal, negative, zero-denominator, or unreduced form.
    NonCanonicalRational,
    /// The decoded compiler output did not exactly recompile from basis and shape.
    CertificateMismatch,
    /// The Terms fields do not project to the named native basis.
    TermsBasisMismatch,
    /// A typed Terms upload plan is not ordered or bound to the named artifact.
    UploadPlanMismatch,
    /// The native shape compiler refused the basis or shape.
    Compiler(CompilerError),
    /// The frozen Solana layout codec refused an account or intent.
    Layout(CodecError),
}

impl From<CompilerError> for ArtifactError {
    fn from(value: CompilerError) -> Self {
        Self::Compiler(value)
    }
}

impl From<CodecError> for ArtifactError {
    fn from(value: CodecError) -> Self {
        Self::Layout(value)
    }
}

/// One recompile-verifiable native shape certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeShapeCertificateV1 {
    /// Digest of the complete canonical Terms body used by market creation.
    pub terms_digest: [u8; 32],
    /// Exact native basis, not a categorical compatibility projection.
    pub basis: BasisSpec,
    /// Exact bounded analytic source description.
    pub shape: Shape,
    /// Deterministic compiler output and exact-rational bounds.
    pub compilation: Compilation,
}

impl NativeShapeCertificateV1 {
    /// Compile and bind a native shape to one nonzero Terms digest.
    pub fn compile(
        terms_digest: [u8; 32],
        basis: BasisSpec,
        shape: Shape,
    ) -> Result<Self, ArtifactError> {
        if terms_digest == [0; 32] {
            return Err(ArtifactError::DigestMismatch);
        }
        let compilation = compile(&basis, shape)?;
        Ok(Self {
            terms_digest,
            basis,
            shape,
            compilation,
        })
    }

    /// Re-run the compiler and require byte-semantic equality with the claim.
    pub fn verify(&self) -> Result<(), ArtifactError> {
        if self.terms_digest == [0; 32] {
            return Err(ArtifactError::DigestMismatch);
        }
        if compile(&self.basis, self.shape)? != self.compilation {
            return Err(ArtifactError::CertificateMismatch);
        }
        Ok(())
    }

    /// Require that one fully canonical Terms account owns this exact basis and digest.
    pub fn verify_terms(&self, terms: &TermsAccount) -> Result<(), ArtifactError> {
        terms.validate()?;
        let basis = basis_spec_from_terms_v1(terms)?;
        if self.terms_digest != terms.terms.0 || self.basis != basis {
            return Err(ArtifactError::TermsBasisMismatch);
        }
        self.verify()
    }

    /// Encode the unique certificate byte string.
    pub fn encode(&self) -> Result<Vec<u8>, ArtifactError> {
        self.verify()?;
        let basis_bytes = encode_basis_spec_v1(&self.basis)?;
        let basis_digest = digest_basis_spec_v1(&basis_bytes);
        let shape_bytes = encode_shape_v1(self.shape);
        let rational_count = usize::from(self.basis.outcome_count)
            .checked_add(10)
            .ok_or(ArtifactError::InvalidLength)?;
        let rational_count_u16 =
            u16::try_from(rational_count).map_err(|_| ArtifactError::InvalidLength)?;
        let mut out = Vec::with_capacity(CERTIFICATE_FIXED_BYTES_V1 + rational_count * 16);
        out.extend_from_slice(&CERTIFICATE_MAGIC_V1);
        push_u16(&mut out, CERTIFICATE_SCHEMA_VERSION_V1);
        push_u16(&mut out, SHAPE_COMPILER_VERSION_V1);
        push_u16(&mut out, BASIS_EVALUATOR_VERSION_V1);
        push_u16(&mut out, WEIGHT_ROUND_VERSION_V1);
        out.push(SEMANTIC_NATIVE_BSPLINE);
        out.push(span_status_byte(self.compilation.status));
        out.push(construction_byte(self.compilation.construction));
        out.push(self.compilation.certificate.subdivision_depth);
        out.extend_from_slice(&self.terms_digest);
        out.extend_from_slice(&basis_digest);
        out.extend_from_slice(&basis_bytes);
        out.extend_from_slice(&shape_bytes);
        push_u16(&mut out, rational_count_u16);
        push_u16(&mut out, 0);
        for coefficient in &self.compilation.coefficients {
            encode_rational_v1(coefficient, &mut out)?;
        }
        encode_rational_v1(&self.compilation.height, &mut out)?;
        encode_rational_v1(&self.compilation.max_coefficient, &mut out)?;
        let certificate = &self.compilation.certificate;
        for value in [
            &certificate.spline_sup_lower,
            &certificate.spline_sup_upper,
            &certificate.spline_l1_lower,
            &certificate.spline_l1_upper,
            &certificate.consensus_quantization_sup_upper,
            &certificate.consensus_sup_upper,
            &certificate.consensus_l1_upper,
            &certificate.coefficient_sample_sup_upper,
        ] {
            encode_rational_v1(value, &mut out)?;
        }
        if out.len() > MAX_CERTIFICATE_BYTES_V1 {
            return Err(ArtifactError::InvalidLength);
        }
        Ok(out)
    }

    /// Decode, recompile, and reject every noncanonical representation.
    pub fn decode(bytes: &[u8]) -> Result<Self, ArtifactError> {
        if bytes.len() > MAX_CERTIFICATE_BYTES_V1 {
            return Err(ArtifactError::InvalidLength);
        }
        let mut reader = Reader::new(bytes);
        if reader.take(8)? != CERTIFICATE_MAGIC_V1 {
            return Err(ArtifactError::InvalidDiscriminant);
        }
        if reader.u16()? != CERTIFICATE_SCHEMA_VERSION_V1
            || reader.u16()? != SHAPE_COMPILER_VERSION_V1
            || reader.u16()? != BASIS_EVALUATOR_VERSION_V1
            || reader.u16()? != WEIGHT_ROUND_VERSION_V1
        {
            return Err(ArtifactError::InvalidDiscriminant);
        }
        if reader.u8()? != SEMANTIC_NATIVE_BSPLINE {
            return Err(ArtifactError::InvalidDiscriminant);
        }
        let status = span_status_from_byte(reader.u8()?)?;
        let construction = construction_from_byte(reader.u8()?)?;
        let subdivision_depth = reader.u8()?;
        let terms_digest = reader.array32()?;
        if terms_digest == [0; 32] {
            return Err(ArtifactError::DigestMismatch);
        }
        let stored_basis_digest = reader.array32()?;
        let basis_slice = reader.take(BASIS_SPEC_BYTES_V1)?;
        if digest_basis_spec_v1(basis_slice) != stored_basis_digest {
            return Err(ArtifactError::DigestMismatch);
        }
        let basis = decode_basis_spec_v1(basis_slice)?;
        let shape = decode_shape_v1(reader.take(SHAPE_BYTES_V1)?)?;
        let rational_count = usize::from(reader.u16()?);
        if reader.u16()? != 0 {
            return Err(ArtifactError::NonCanonicalPadding);
        }
        if rational_count != usize::from(basis.outcome_count) + 10 {
            return Err(ArtifactError::InvalidLength);
        }
        let mut rationals = Vec::with_capacity(rational_count);
        for _ in 0..rational_count {
            rationals.push(decode_rational_v1(&mut reader)?);
        }
        if !reader.done() {
            return Err(ArtifactError::TrailingBytes);
        }
        let active = usize::from(basis.outcome_count);
        let coefficients = rationals[..active].to_vec();
        let height = rationals[active].clone();
        let max_coefficient = rationals[active + 1].clone();
        let metric = &rationals[active + 2..];
        let compilation = Compilation {
            status,
            construction,
            coefficients,
            height,
            max_coefficient,
            certificate: ErrorCertificate {
                spline_sup_lower: metric[0].clone(),
                spline_sup_upper: metric[1].clone(),
                spline_l1_lower: metric[2].clone(),
                spline_l1_upper: metric[3].clone(),
                consensus_quantization_sup_upper: metric[4].clone(),
                consensus_sup_upper: metric[5].clone(),
                consensus_l1_upper: metric[6].clone(),
                coefficient_sample_sup_upper: metric[7].clone(),
                subdivision_depth,
            },
        };
        let value = Self {
            terms_digest,
            basis,
            shape,
            compilation,
        };
        value.verify()?;
        if value.encode()? != bytes {
            return Err(ArtifactError::CertificateMismatch);
        }
        Ok(value)
    }

    /// Domain-separated digest of the unique certificate bytes.
    pub fn digest(&self) -> Result<[u8; 32], ArtifactError> {
        Ok(domain_digest(CERTIFICATE_DIGEST_DOMAIN_V1, &self.encode()?))
    }
}

/// All immutable bytes an offline client needs to inspect before founding a market.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketCreationArtifactsV1 {
    /// Exact native basis bytes.
    pub basis_spec_bytes: [u8; BASIS_SPEC_BYTES_V1],
    /// Domain-separated native basis digest.
    pub basis_spec_digest: [u8; 32],
    /// Recompile-verifiable native shape certificate bytes.
    pub shape_certificate_bytes: Vec<u8>,
    /// Domain-separated certificate digest.
    pub shape_certificate_digest: [u8; 32],
    /// Exact self-certifying Terms account bytes uploaded before market creation.
    pub terms_account_bytes: Vec<u8>,
    /// Canonical Terms identity consumed by both intents.
    pub terms_digest: [u8; 32],
    /// Live typed Terms upload route: Begin, nine ordered Writes, then Seal.
    pub terms_upload: TermsArtifactUploadPlanV1,
    /// Frozen `Intent::CreateMarket` bytes.
    pub create_market_intent: Vec<u8>,
}

/// Exact live Terms artifact-upload instruction plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TermsArtifactUploadPlanV1 {
    /// Caller-observed slot used only to refuse an obviously invalid lifetime.
    pub observed_current_slot: u64,
    /// Expiry carried by `BeginArtifact`; runtime rechecks against Clock.
    pub expires_slot: u64,
    /// One exact `Intent::BeginArtifact` byte string.
    pub begin_intent: Vec<u8>,
    /// Nine exact, cursor-ordered `Intent::WriteArtifact` byte strings.
    pub write_intents: Vec<Vec<u8>>,
    /// One exact `Intent::SealArtifact` byte string.
    pub seal_intent: Vec<u8>,
}

impl TermsArtifactUploadPlanV1 {
    /// Decode the entire plan, enforce runtime ordering/bindings, and recover Terms bytes.
    pub fn verify_and_reconstruct(&self, terms: &TermsAccount) -> Result<Vec<u8>, ArtifactError> {
        terms.validate()?;
        let lifetime = self
            .expires_slot
            .checked_sub(self.observed_current_slot)
            .ok_or(ArtifactError::UploadPlanMismatch)?;
        if !(MIN_UPLOAD_LIFETIME_SLOTS_V1..=MAX_UPLOAD_LIFETIME_SLOTS_V1).contains(&lifetime) {
            return Err(ArtifactError::UploadPlanMismatch);
        }
        let expected_len =
            u16::try_from(account_len::TERMS).map_err(|_| ArtifactError::InvalidLength)?;
        if Intent::decode(&self.begin_intent)?
            != (Intent::BeginArtifact {
                kind: ArtifactKind::Terms,
                context: terms.realm,
                digest: terms.terms,
                exact_len: expected_len,
                expires_slot: self.expires_slot,
            })
        {
            return Err(ArtifactError::UploadPlanMismatch);
        }
        if self.write_intents.len() != TERMS_ARTIFACT_WRITE_COUNT_V1 {
            return Err(ArtifactError::UploadPlanMismatch);
        }
        let mut reconstructed = Vec::with_capacity(account_len::TERMS);
        for (index, encoded) in self.write_intents.iter().enumerate() {
            let Intent::WriteArtifact {
                kind,
                context,
                digest,
                cursor,
                chunk_len,
                chunk,
            } = Intent::decode(encoded)?
            else {
                return Err(ArtifactError::UploadPlanMismatch);
            };
            let expected_cursor = index
                .checked_mul(ARTIFACT_CHUNK_BYTES)
                .and_then(|value| u16::try_from(value).ok())
                .ok_or(ArtifactError::InvalidLength)?;
            let remaining = account_len::TERMS
                .checked_sub(index * ARTIFACT_CHUNK_BYTES)
                .ok_or(ArtifactError::UploadPlanMismatch)?;
            let expected_chunk_len = remaining.min(ARTIFACT_CHUNK_BYTES);
            if kind != ArtifactKind::Terms
                || context != terms.realm
                || digest != terms.terms
                || cursor != expected_cursor
                || usize::from(chunk_len) != expected_chunk_len
            {
                return Err(ArtifactError::UploadPlanMismatch);
            }
            reconstructed.extend_from_slice(&chunk[..expected_chunk_len]);
        }
        if reconstructed.len() != account_len::TERMS
            || TermsAccount::decode(&reconstructed)? != *terms
            || Intent::decode(&self.seal_intent)?
                != (Intent::SealArtifact {
                    kind: ArtifactKind::Terms,
                    context: terms.realm,
                    digest: terms.terms,
                    exact_len: expected_len,
                })
        {
            return Err(ArtifactError::UploadPlanMismatch);
        }
        Ok(reconstructed)
    }
}

/// Project canonical Terms into the exact evaluator input used by native resolution.
pub fn basis_spec_from_terms_v1(terms: &TermsAccount) -> Result<BasisSpec, ArtifactError> {
    terms.validate()?;
    let edge_policy = match terms.edge_policy_id {
        1 => EdgePolicy::Clamp,
        2 => EdgePolicy::Refuse,
        _ => return Err(ArtifactError::TermsBasisMismatch),
    };
    let basis = BasisSpec {
        outcome_count: terms.outcome_count,
        degree: terms.basis_degree,
        knot_count: terms.knot_count,
        uniform_log2_spacing: terms.uniform_log2_spacing,
        denominator: terms.payouts[0].denominator,
        domain_max: MAX_VALUE,
        edge_policy,
        knots: terms.knots,
    };
    basis
        .validate()
        .map_err(|_| ArtifactError::TermsBasisMismatch)?;
    Ok(basis)
}

/// Build exact Terms-upload and market-creation intent bytes from one native shape.
///
/// This is a codec bridge, not authorization or runtime admission.  The SBF
/// adapter still checks signers, PDAs, registered policy/statistic ids, account
/// absence, custody, rent, and Token-2022 state.
pub fn build_market_creation_artifacts_v1(
    terms: &TermsAccount,
    market_nonce: u64,
    observed_current_slot: u64,
    expires_slot: u64,
    shape: Shape,
) -> Result<MarketCreationArtifactsV1, ArtifactError> {
    terms.validate()?;
    let basis = basis_spec_from_terms_v1(terms)?;
    let basis_spec_bytes = encode_basis_spec_v1(&basis)?;
    let basis_spec_digest = digest_basis_spec_v1(&basis_spec_bytes);
    let terms_digest = terms.terms.0;
    let certificate = NativeShapeCertificateV1::compile(terms_digest, basis, shape)?;
    let shape_certificate_bytes = certificate.encode()?;
    let shape_certificate_digest =
        domain_digest(CERTIFICATE_DIGEST_DOMAIN_V1, &shape_certificate_bytes);

    let mut terms_account_bytes = vec![0; account_len::TERMS];
    if terms.encode(&mut terms_account_bytes)? != account_len::TERMS {
        return Err(ArtifactError::InvalidLength);
    }
    let terms_upload = build_terms_artifact_upload_plan_v1(
        terms,
        &terms_account_bytes,
        observed_current_slot,
        expires_slot,
    )?;
    let create_market = Intent::CreateMarket {
        realm: terms.realm,
        profile: terms.profile,
        market_nonce,
        outcome_count: terms.outcome_count,
        terms: terms.terms,
        feed: terms.feed,
    };
    let create_market_intent = encode_intent(&create_market, CREATE_MARKET_INTENT_BYTES)?;
    Ok(MarketCreationArtifactsV1 {
        basis_spec_bytes,
        basis_spec_digest,
        shape_certificate_bytes,
        shape_certificate_digest,
        terms_account_bytes,
        terms_digest,
        terms_upload,
        create_market_intent,
    })
}

/// Build the live typed artifact route for one canonical Terms account.
pub fn build_terms_artifact_upload_plan_v1(
    terms: &TermsAccount,
    terms_account_bytes: &[u8],
    observed_current_slot: u64,
    expires_slot: u64,
) -> Result<TermsArtifactUploadPlanV1, ArtifactError> {
    terms.validate()?;
    if terms_account_bytes.len() != account_len::TERMS {
        return Err(ArtifactError::InvalidLength);
    }
    let lifetime = expires_slot
        .checked_sub(observed_current_slot)
        .ok_or(ArtifactError::InvalidLength)?;
    if !(MIN_UPLOAD_LIFETIME_SLOTS_V1..=MAX_UPLOAD_LIFETIME_SLOTS_V1).contains(&lifetime) {
        return Err(ArtifactError::InvalidLength);
    }
    let begin = Intent::BeginArtifact {
        kind: ArtifactKind::Terms,
        context: terms.realm,
        digest: terms.terms,
        exact_len: u16::try_from(account_len::TERMS).map_err(|_| ArtifactError::InvalidLength)?,
        expires_slot,
    };
    let begin_intent = encode_intent(&begin, BEGIN_ARTIFACT_INTENT_BYTES)?;
    let mut write_intents = Vec::with_capacity(TERMS_ARTIFACT_WRITE_COUNT_V1);
    for (index, source) in terms_account_bytes.chunks(ARTIFACT_CHUNK_BYTES).enumerate() {
        let cursor = index
            .checked_mul(ARTIFACT_CHUNK_BYTES)
            .and_then(|value| u16::try_from(value).ok())
            .ok_or(ArtifactError::InvalidLength)?;
        let chunk_len = u16::try_from(source.len()).map_err(|_| ArtifactError::InvalidLength)?;
        let mut chunk = [0_u8; ARTIFACT_CHUNK_BYTES];
        chunk[..source.len()].copy_from_slice(source);
        let write = Intent::WriteArtifact {
            kind: ArtifactKind::Terms,
            context: terms.realm,
            digest: terms.terms,
            cursor,
            chunk_len,
            chunk,
        };
        write_intents.push(encode_intent(&write, WRITE_ARTIFACT_INTENT_BYTES)?);
    }
    if write_intents.len() != TERMS_ARTIFACT_WRITE_COUNT_V1 {
        return Err(ArtifactError::InvalidLength);
    }
    let seal = Intent::SealArtifact {
        kind: ArtifactKind::Terms,
        context: terms.realm,
        digest: terms.terms,
        exact_len: u16::try_from(account_len::TERMS).map_err(|_| ArtifactError::InvalidLength)?,
    };
    let seal_intent = encode_intent(&seal, SEAL_ARTIFACT_INTENT_BYTES)?;
    let plan = TermsArtifactUploadPlanV1 {
        observed_current_slot,
        expires_slot,
        begin_intent,
        write_intents,
        seal_intent,
    };
    if plan.verify_and_reconstruct(terms)? != terms_account_bytes {
        return Err(ArtifactError::UploadPlanMismatch);
    }
    Ok(plan)
}

/// Render the checked cross-language fixture without introducing a JSON dependency.
///
/// Every integer that could exceed JavaScript's exact range is a decimal
/// string and every binary value is lowercase hex.  Digests remain over the
/// binary codecs, never over this diagnostic JSON.
pub fn render_cross_language_fixture_v1(
    artifacts: &MarketCreationArtifactsV1,
    market_nonce: u64,
) -> String {
    let writes = artifacts
        .terms_upload
        .write_intents
        .iter()
        .map(|intent| format!("\"{}\"", lowercase_hex(intent)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        concat!(
            "{{\n",
            "  \"schema\": \"dragon-clutch.native-bspline-cross-language.v1\",\n",
            "  \"maxValue\": \"{}\",\n",
            "  \"marketNonce\": \"{}\",\n",
            "  \"termsDigest\": \"{}\",\n",
            "  \"basisSpecBytes\": \"{}\",\n",
            "  \"basisSpecDigest\": \"{}\",\n",
            "  \"shapeCertificateBytes\": \"{}\",\n",
            "  \"shapeCertificateDigest\": \"{}\",\n",
            "  \"termsAccountBytes\": \"{}\",\n",
            "  \"observedCurrentSlot\": \"{}\",\n",
            "  \"expiresSlot\": \"{}\",\n",
            "  \"beginTermsArtifactIntentBytes\": \"{}\",\n",
            "  \"writeTermsArtifactIntentBytes\": [{}],\n",
            "  \"sealTermsArtifactIntentBytes\": \"{}\",\n",
            "  \"createMarketIntentBytes\": \"{}\"\n",
            "}}"
        ),
        MAX_VALUE,
        market_nonce,
        lowercase_hex(&artifacts.terms_digest),
        lowercase_hex(&artifacts.basis_spec_bytes),
        lowercase_hex(&artifacts.basis_spec_digest),
        lowercase_hex(&artifacts.shape_certificate_bytes),
        lowercase_hex(&artifacts.shape_certificate_digest),
        lowercase_hex(&artifacts.terms_account_bytes),
        artifacts.terms_upload.observed_current_slot,
        artifacts.terms_upload.expires_slot,
        lowercase_hex(&artifacts.terms_upload.begin_intent),
        writes,
        lowercase_hex(&artifacts.terms_upload.seal_intent),
        lowercase_hex(&artifacts.create_market_intent),
    )
}

fn lowercase_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Encode an admitted `BasisSpec` into the exact revision-one byte layout.
pub fn encode_basis_spec_v1(spec: &BasisSpec) -> Result<[u8; BASIS_SPEC_BYTES_V1], ArtifactError> {
    spec.validate()
        .map_err(|_| ArtifactError::TermsBasisMismatch)?;
    let mut out = [0_u8; BASIS_SPEC_BYTES_V1];
    let mut at = 0_usize;
    write(&mut out, &mut at, &BASIS_MAGIC_V1);
    write(&mut out, &mut at, &BASIS_SCHEMA_VERSION_V1.to_le_bytes());
    write(&mut out, &mut at, &BASIS_EVALUATOR_VERSION_V1.to_le_bytes());
    out[at] = SEMANTIC_NATIVE_BSPLINE;
    at += 1;
    out[at] = spec.outcome_count;
    at += 1;
    out[at] = spec.degree;
    at += 1;
    out[at] = spec.knot_count;
    at += 1;
    out[at] = spec.uniform_log2_spacing;
    at += 1;
    out[at] = edge_policy_byte(spec.edge_policy);
    at += 1;
    at += 6; // canonical zero reserved bytes
    write(&mut out, &mut at, &spec.denominator.to_le_bytes());
    write(&mut out, &mut at, &spec.domain_max.to_le_bytes());
    for knot in spec.knots {
        write(&mut out, &mut at, &knot.to_le_bytes());
    }
    if at != BASIS_SPEC_BYTES_V1 {
        return Err(ArtifactError::InvalidLength);
    }
    Ok(out)
}

/// Decode and validate the exact revision-one native basis layout.
pub fn decode_basis_spec_v1(bytes: &[u8]) -> Result<BasisSpec, ArtifactError> {
    if bytes.len() < BASIS_SPEC_BYTES_V1 {
        return Err(ArtifactError::Truncated);
    }
    if bytes.len() > BASIS_SPEC_BYTES_V1 {
        return Err(ArtifactError::TrailingBytes);
    }
    let mut reader = Reader::new(bytes);
    if reader.take(8)? != BASIS_MAGIC_V1
        || reader.u16()? != BASIS_SCHEMA_VERSION_V1
        || reader.u16()? != BASIS_EVALUATOR_VERSION_V1
        || reader.u8()? != SEMANTIC_NATIVE_BSPLINE
    {
        return Err(ArtifactError::InvalidDiscriminant);
    }
    let outcome_count = reader.u8()?;
    let degree = reader.u8()?;
    let knot_count = reader.u8()?;
    let uniform_log2_spacing = reader.u8()?;
    let edge_policy = edge_policy_from_byte(reader.u8()?)?;
    if reader.take(6)?.iter().any(|byte| *byte != 0) {
        return Err(ArtifactError::NonCanonicalPadding);
    }
    let denominator = reader.u64()?;
    let domain_max = reader.u128()?;
    let mut knots = [0_u128; MAX_KNOTS];
    for knot in &mut knots {
        *knot = reader.u128()?;
    }
    if !reader.done() {
        return Err(ArtifactError::TrailingBytes);
    }
    let spec = BasisSpec {
        outcome_count,
        degree,
        knot_count,
        uniform_log2_spacing,
        denominator,
        domain_max,
        edge_policy,
        knots,
    };
    spec.validate()
        .map_err(|_| ArtifactError::TermsBasisMismatch)?;
    if encode_basis_spec_v1(&spec)?.as_slice() != bytes {
        return Err(ArtifactError::NonCanonicalPadding);
    }
    Ok(spec)
}

/// Digest already-canonical basis bytes under their own domain.
pub fn digest_basis_spec_v1(bytes: &[u8]) -> [u8; 32] {
    domain_digest(BASIS_DIGEST_DOMAIN_V1, bytes)
}

fn encode_intent(intent: &Intent, expected: usize) -> Result<Vec<u8>, ArtifactError> {
    let mut scratch = [0_u8; MAX_INTENT_BYTES];
    let written = intent.encode(&mut scratch)?;
    if written != expected || intent.encoded_len() != expected {
        return Err(ArtifactError::InvalidLength);
    }
    Ok(scratch[..written].to_vec())
}

pub(crate) fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    hasher.finalize().into()
}

fn edge_policy_byte(policy: EdgePolicy) -> u8 {
    match policy {
        EdgePolicy::Clamp => 1,
        EdgePolicy::Refuse => 2,
    }
}

fn edge_policy_from_byte(byte: u8) -> Result<EdgePolicy, ArtifactError> {
    match byte {
        1 => Ok(EdgePolicy::Clamp),
        2 => Ok(EdgePolicy::Refuse),
        _ => Err(ArtifactError::InvalidDiscriminant),
    }
}

fn span_status_byte(status: SpanStatus) -> u8 {
    match status {
        SpanStatus::ExactInSpan => 1,
        SpanStatus::CertifiedApproximation => 2,
    }
}

fn span_status_from_byte(byte: u8) -> Result<SpanStatus, ArtifactError> {
    match byte {
        1 => Ok(SpanStatus::ExactInSpan),
        2 => Ok(SpanStatus::CertifiedApproximation),
        _ => Err(ArtifactError::InvalidDiscriminant),
    }
}

fn construction_byte(construction: Construction) -> u8 {
    match construction {
        Construction::DegreeZeroCells => 1,
        Construction::DegreeOneInterpolation => 2,
        Construction::GrevilleAffineReproduction => 3,
        Construction::SchoenbergGrevilleQuasiInterpolant => 4,
        Construction::GaussianIntervalSamples => 5,
    }
}

fn construction_from_byte(byte: u8) -> Result<Construction, ArtifactError> {
    match byte {
        1 => Ok(Construction::DegreeZeroCells),
        2 => Ok(Construction::DegreeOneInterpolation),
        3 => Ok(Construction::GrevilleAffineReproduction),
        4 => Ok(Construction::SchoenbergGrevilleQuasiInterpolant),
        5 => Ok(Construction::GaussianIntervalSamples),
        _ => Err(ArtifactError::InvalidDiscriminant),
    }
}

fn encode_shape_v1(shape: Shape) -> [u8; SHAPE_BYTES_V1] {
    let (tag, a, b, c, height) = match shape {
        Shape::HardRange { low, high, height } => (1, low, high, 0, height),
        Shape::UpperTail { strike, height } => (2, strike, 0, 0, height),
        Shape::LowerTail { strike, height } => (3, strike, 0, 0, height),
        Shape::Triangle {
            left,
            peak,
            right,
            height,
        } => (4, left, peak, right, height),
        Shape::CappedCall { low, high, height } => (5, low, high, 0, height),
        Shape::CappedPut { low, high, height } => (6, low, high, 0, height),
        Shape::Gaussian {
            center,
            sigma,
            height,
        } => (7, center, sigma, 0, height),
    };
    let mut out = [0_u8; SHAPE_BYTES_V1];
    out[0] = tag;
    out[8..24].copy_from_slice(&a.to_le_bytes());
    out[24..40].copy_from_slice(&b.to_le_bytes());
    out[40..56].copy_from_slice(&c.to_le_bytes());
    out[56..64].copy_from_slice(&height.to_le_bytes());
    out
}

fn decode_shape_v1(bytes: &[u8]) -> Result<Shape, ArtifactError> {
    if bytes.len() != SHAPE_BYTES_V1 {
        return Err(ArtifactError::InvalidLength);
    }
    if bytes[1..8].iter().any(|byte| *byte != 0) {
        return Err(ArtifactError::NonCanonicalPadding);
    }
    let a = u128::from_le_bytes(
        bytes[8..24]
            .try_into()
            .map_err(|_| ArtifactError::Truncated)?,
    );
    let b = u128::from_le_bytes(
        bytes[24..40]
            .try_into()
            .map_err(|_| ArtifactError::Truncated)?,
    );
    let c = u128::from_le_bytes(
        bytes[40..56]
            .try_into()
            .map_err(|_| ArtifactError::Truncated)?,
    );
    let height = u64::from_le_bytes(
        bytes[56..64]
            .try_into()
            .map_err(|_| ArtifactError::Truncated)?,
    );
    let shape = match bytes[0] {
        1 if c == 0 => Shape::HardRange {
            low: a,
            high: b,
            height,
        },
        2 if b == 0 && c == 0 => Shape::UpperTail { strike: a, height },
        3 if b == 0 && c == 0 => Shape::LowerTail { strike: a, height },
        4 => Shape::Triangle {
            left: a,
            peak: b,
            right: c,
            height,
        },
        5 if c == 0 => Shape::CappedCall {
            low: a,
            high: b,
            height,
        },
        6 if c == 0 => Shape::CappedPut {
            low: a,
            high: b,
            height,
        },
        7 if c == 0 => Shape::Gaussian {
            center: a,
            sigma: b,
            height,
        },
        _ => return Err(ArtifactError::NonCanonicalPadding),
    };
    Ok(shape)
}

pub(crate) fn encode_rational_v1(
    value: &BigRational,
    out: &mut Vec<u8>,
) -> Result<(), ArtifactError> {
    if value.is_negative() || value.denom().is_negative() || value.denom().is_zero() {
        return Err(ArtifactError::NonCanonicalRational);
    }
    let numerator = unsigned_magnitude(value.numer())?;
    let denominator = unsigned_magnitude(value.denom())?;
    if numerator.len() > MAX_RATIONAL_INTEGER_BYTES_V1
        || denominator.is_empty()
        || denominator.len() > MAX_RATIONAL_INTEGER_BYTES_V1
    {
        return Err(ArtifactError::InvalidLength);
    }
    push_u16(
        out,
        u16::try_from(numerator.len()).map_err(|_| ArtifactError::InvalidLength)?,
    );
    push_u16(
        out,
        u16::try_from(denominator.len()).map_err(|_| ArtifactError::InvalidLength)?,
    );
    out.extend_from_slice(&numerator);
    out.extend_from_slice(&denominator);
    Ok(())
}

pub(crate) fn decode_rational_v1(reader: &mut Reader<'_>) -> Result<BigRational, ArtifactError> {
    let numerator_len = usize::from(reader.u16()?);
    let denominator_len = usize::from(reader.u16()?);
    if numerator_len > MAX_RATIONAL_INTEGER_BYTES_V1
        || denominator_len == 0
        || denominator_len > MAX_RATIONAL_INTEGER_BYTES_V1
    {
        return Err(ArtifactError::InvalidLength);
    }
    let numerator_bytes = reader.take(numerator_len)?;
    let denominator_bytes = reader.take(denominator_len)?;
    if (numerator_len > 0 && numerator_bytes[0] == 0) || denominator_bytes[0] == 0 {
        return Err(ArtifactError::NonCanonicalRational);
    }
    let numerator = if numerator_len == 0 {
        BigInt::zero()
    } else {
        BigInt::from_bytes_be(Sign::Plus, numerator_bytes)
    };
    let denominator = BigInt::from_bytes_be(Sign::Plus, denominator_bytes);
    if denominator.is_zero() {
        return Err(ArtifactError::NonCanonicalRational);
    }
    let rational = BigRational::new(numerator, denominator);
    if rational.is_zero() {
        if numerator_len != 0 || !rational.denom().is_one() || denominator_bytes != [1] {
            return Err(ArtifactError::NonCanonicalRational);
        }
    } else if unsigned_magnitude(rational.numer())? != numerator_bytes
        || unsigned_magnitude(rational.denom())? != denominator_bytes
    {
        return Err(ArtifactError::NonCanonicalRational);
    }
    Ok(rational)
}

fn unsigned_magnitude(value: &BigInt) -> Result<Vec<u8>, ArtifactError> {
    let (sign, bytes) = value.to_bytes_be();
    match sign {
        Sign::Minus => Err(ArtifactError::NonCanonicalRational),
        Sign::NoSign => Ok(Vec::new()),
        Sign::Plus => Ok(bytes),
    }
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write<const N: usize>(out: &mut [u8], at: &mut usize, bytes: &[u8; N]) {
    out[*at..*at + N].copy_from_slice(bytes);
    *at += N;
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub(crate) fn take(&mut self, len: usize) -> Result<&'a [u8], ArtifactError> {
        let end = self
            .at
            .checked_add(len)
            .ok_or(ArtifactError::InvalidLength)?;
        if end > self.bytes.len() {
            return Err(ArtifactError::Truncated);
        }
        let value = &self.bytes[self.at..end];
        self.at = end;
        Ok(value)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, ArtifactError> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, ArtifactError> {
        Ok(u16::from_le_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| ArtifactError::Truncated)?,
        ))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, ArtifactError> {
        Ok(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| ArtifactError::Truncated)?,
        ))
    }

    pub(crate) fn u128(&mut self) -> Result<u128, ArtifactError> {
        Ok(u128::from_le_bytes(
            self.take(16)?
                .try_into()
                .map_err(|_| ArtifactError::Truncated)?,
        ))
    }

    pub(crate) fn array32(&mut self) -> Result<[u8; 32], ArtifactError> {
        self.take(32)?
            .try_into()
            .map_err(|_| ArtifactError::Truncated)
    }

    pub(crate) fn done(&self) -> bool {
        self.at == self.bytes.len()
    }
}

const _: () = assert!(MAX_OUTCOMES == 16);

#[cfg(test)]
mod private_tests {
    use super::*;

    #[test]
    fn rational_encoder_has_no_negative_wire_form() {
        let negative = BigRational::from_integer(BigInt::from(-1));
        let mut bytes = Vec::new();
        assert_eq!(
            encode_rational_v1(&negative, &mut bytes),
            Err(ArtifactError::NonCanonicalRational)
        );
        assert!(bytes.is_empty());
    }
}
