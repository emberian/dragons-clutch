//! Typed, resumable transport for immutable protocol artifacts.
//!
//! A transport account is not an artifact and can never be consumed as one.
//! It is an uploader-scoped, program-owned staging area whose header commits
//! to one [`ArtifactBinding`] and whose body is filled strictly from left to
//! right in [`ARTIFACT_CHUNK_BYTES`] chunks.  Only a complete stage can be
//! validated through [`validate_artifact`].  The Solana adapter is responsible
//! for creating the final content-derived PDA, copying the validated bytes,
//! and closing the stage back to its recorded funder atomically.
//!
//! The module deliberately has no generic blob kind.  Every admitted kind has
//! one existing hostile-byte codec and one exact length.  Adding a source
//! specification, archive page, or clearing artifact therefore requires
//! adding its owning codec here first; callers cannot make an untyped upload
//! become consensus truth by choosing a new discriminant.

#[cfg(any(
    feature = "profile-full",
    feature = "profile-direct-v3-source-v2-point"
))]
use super::direct_selection_v3::DirectBatchPolicyV3;
use super::direct_selection_v3::DIRECT_BATCH_POLICY_V3_BYTES;
use super::{
    account_len, canonical_profile_v2_id, is_zero, CodecError, Hash32, PriceGridAccount, Result,
    TermsAccount, HASH_BYTES,
};
use clutch_collateral_adapter_v2::{CollateralPolicyV2, COLLATERAL_POLICY_V2_BYTES};
#[cfg(all(
    feature = "non-production-product-series-lab",
    not(target_os = "solana")
))]
use clutch_product_series::{
    CompiledProductSeriesBundleV1, EvidenceOnlyRecoveryPolicyV1, MarketGenesisProfileV2,
    NativeClaimBasisV1, PriceMeasurePolicyV1, ProductTemplateV4, SeriesAttachmentPlanV1,
    SeriesFundingQuoteV1, SeriesFundingTermsV2, SeriesPlanV5,
};
use clutch_product_series::{
    CompiledProductSeriesBundleV2, FixedCodec, MarketInstancePreimageV2,
    RegistryCapabilityProfileV2, RegistryProgramReleaseV1, SeriesAttachmentPlanV2,
    SeriesFundingQuoteV2, COMPILED_PRODUCT_SERIES_BUNDLE_V2_BYTES,
    MARKET_INSTANCE_PREIMAGE_V2_BYTES, REGISTRY_CAPABILITY_PROFILE_V2_BYTES,
    REGISTRY_PROGRAM_RELEASE_V1_BYTES, SERIES_ATTACHMENT_PLAN_BYTES_V2,
    SERIES_FUNDING_QUOTE_BYTES_V2,
};
use clutch_source_plane_v3_runtime::{
    SourceReleaseManifestV1, SourceReleaseManifestV2, SourceWorkScheduleBindingV1,
    SOURCE_RELEASE_MANIFEST_BYTES, SOURCE_RELEASE_MANIFEST_V1_BYTES, SOURCE_WORK_SCHEDULE_BYTES,
};

const PRODUCT_BASIS_BYTES: usize = 2_352;
const PRODUCT_RECOVERY_BYTES: usize = 208;
const PRODUCT_TEMPLATE_V4_BYTES: usize = 256;
const PRODUCT_PRICE_MEASURE_POLICY_BYTES: usize = 96;
const PRODUCT_MARKET_GENESIS_V2_BYTES: usize = 416;
const PRODUCT_FUNDING_QUOTE_BYTES: usize = 280;
const PRODUCT_ATTACHMENT_PLAN_BYTES: usize = 112;
const PRODUCT_SERIES_PLAN_V5_BYTES: usize = 152;
const PRODUCT_FUNDING_TERMS_V2_BYTES: usize = 240;
const COMPILED_PRODUCT_SERIES_BUNDLE_V1_BYTES: usize = 528;

const _: () = {
    assert!(REGISTRY_PROGRAM_RELEASE_V1_BYTES == 160);
    assert!(REGISTRY_CAPABILITY_PROFILE_V2_BYTES == 800);
    assert!(SOURCE_RELEASE_MANIFEST_V1_BYTES == 1_008);
    assert!(SOURCE_RELEASE_MANIFEST_BYTES == 1_296);
    assert!(SERIES_FUNDING_QUOTE_BYTES_V2 == 648);
    assert!(COMPILED_PRODUCT_SERIES_BUNDLE_V2_BYTES == 528);
    assert!(SERIES_ATTACHMENT_PLAN_BYTES_V2 == 112);
};

#[cfg(feature = "non-production-product-series-lab")]
const _: () = {
    assert!(PRODUCT_BASIS_BYTES == clutch_product_series::BASIS_BYTES);
    assert!(PRODUCT_RECOVERY_BYTES == clutch_product_series::EVIDENCE_ONLY_RECOVERY_POLICY_BYTES);
    assert!(PRODUCT_TEMPLATE_V4_BYTES == clutch_product_series::PRODUCT_TEMPLATE_BYTES);
    assert!(
        PRODUCT_PRICE_MEASURE_POLICY_BYTES == clutch_product_series::PRICE_MEASURE_POLICY_BYTES
    );
    assert!(
        PRODUCT_MARKET_GENESIS_V2_BYTES == clutch_product_series::MARKET_GENESIS_PROFILE_V2_BYTES
    );
    assert!(PRODUCT_FUNDING_QUOTE_BYTES == clutch_product_series::SERIES_FUNDING_QUOTE_BYTES);
    assert!(PRODUCT_ATTACHMENT_PLAN_BYTES == clutch_product_series::SERIES_ATTACHMENT_PLAN_BYTES);
    assert!(PRODUCT_SERIES_PLAN_V5_BYTES == clutch_product_series::SERIES_PLAN_V5_BYTES);
    assert!(PRODUCT_FUNDING_TERMS_V2_BYTES == clutch_product_series::SERIES_FUNDING_TERMS_V2_BYTES);
    assert!(
        COMPILED_PRODUCT_SERIES_BUNDLE_V1_BYTES
            == clutch_product_series::COMPILED_PRODUCT_SERIES_BUNDLE_V1_BYTES
    );
};
#[cfg(feature = "profile-direct-v3-source-v2-point")]
use clutch_batch_policy_identity::BATCH_POLICY_BYTES;
#[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
use clutch_batch_policy_identity::{
    batch_policy_digest, decode_batch_policy, Identity32V1, BATCH_POLICY_BYTES,
};

/// Stage-account discriminator.
pub const ARTIFACT_STAGE_TAG: u8 = 0x21;
/// First and only stage-account schema understood by this build.
pub const ARTIFACT_STAGE_VERSION: u8 = 1;
/// Bytes carried by every non-final upload write.
///
/// A write intent also carries the complete type/context/digest binding and
/// stays below the protocol's existing 310-byte intent ceiling.
pub const ARTIFACT_CHUNK_BYTES: usize = 192;
/// Reserved zero bytes at the end of the stage header.
pub const ARTIFACT_STAGE_RESERVED_BYTES: usize = 16;
/// Exact fixed header length before the staged artifact body.
pub const ARTIFACT_STAGE_HEADER_BYTES: usize = 2
    + 1
    + 1
    + 2
    + 2
    + 8
    + 8
    + HASH_BYTES
    + HASH_BYTES
    + HASH_BYTES
    + ARTIFACT_STAGE_RESERVED_BYTES;
/// Largest artifact body admitted by this transport revision.
pub const MAX_ARTIFACT_BYTES: usize = PRODUCT_BASIS_BYTES;

/// A fixed artifact family with one owning hostile-byte codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ArtifactKind {
    /// The Realm collateral policy. Its context is the parent Profile id.
    CollateralPolicy = 1,
    /// A frozen price grid. Its context is the Realm id.
    PriceGrid = 2,
    /// Immutable market terms. Its context is the Realm id.
    Terms = 3,
    /// Immutable full-width batch-policy preimage. Its context is an Epoch id.
    BatchPolicy = 4,
    /// Direct-policy plus verifier release identity. Its context is an Epoch id.
    DirectBatchPolicyV3 = 5,
    /// Product-owned finite or smooth native claim basis V1.
    NativeClaimBasisV1 = 32,
    /// Product-owned evidence-only recovery policy V1.
    EvidenceOnlyRecoveryPolicyV1 = 33,
    /// Reusable relative Product template V4.
    ProductTemplateV4 = 34,
    /// Exact quantized price-measure policy V1.
    PriceMeasurePolicyV1 = 35,
    /// Realm/profile-bound Market genesis profile V2.
    MarketGenesisProfileV2 = 36,
    /// Exact per-occurrence component funding quote V1.
    SeriesFundingQuoteV1 = 37,
    /// Operational attachment plan V1.
    SeriesAttachmentPlanV1 = 38,
    /// Finite recurring Series plan V5.
    SeriesPlanV5 = 39,
    /// Successor Series funding ownership terms V2.
    SeriesFundingTermsV2 = 40,
    /// Shared immutable central-registry executable release V1.
    RegistryProgramReleaseV1 = 41,
    /// Exact typed artifact graph emitted by an untrusted Product compiler.
    CompiledProductSeriesBundleV1 = 42,
    /// Shared immutable central-registry capability profile V2.
    RegistryCapabilityProfileV2 = 43,
    /// Complete reviewed SourcePlane V3 release manifest.
    SourceReleaseManifestV1 = 44,
    /// Immutable Source-selected paid-work schedule.
    SourceWorkScheduleV1 = 45,
    /// Full-width economic MarketInstance V2 identity preimage.
    MarketInstancePreimageV2 = 46,
    /// Receiver-release-authenticated SourcePlane V3 release manifest.
    SourceReleaseManifestV2 = 47,
    /// Six-compartment recurring-Series funding quote V2.
    SeriesFundingQuoteV2 = 48,
    /// Exact successor compiler graph binding QuoteV2 and AttachmentV2.
    CompiledProductSeriesBundleV2 = 49,
    /// Operational attachment plan bound to one exact QuoteV2.
    SeriesAttachmentPlanV2 = 50,
}

impl ArtifactKind {
    /// Decode the stable wire discriminant.
    pub const fn from_byte(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::CollateralPolicy),
            2 => Ok(Self::PriceGrid),
            3 => Ok(Self::Terms),
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            4 => Ok(Self::BatchPolicy),
            #[cfg(any(
                feature = "profile-full",
                feature = "profile-direct-v3-source-v2-point"
            ))]
            5 => Ok(Self::DirectBatchPolicyV3),
            #[cfg(feature = "non-production-product-series-lab")]
            32 => Ok(Self::NativeClaimBasisV1),
            #[cfg(feature = "non-production-product-series-lab")]
            33 => Ok(Self::EvidenceOnlyRecoveryPolicyV1),
            #[cfg(feature = "non-production-product-series-lab")]
            34 => Ok(Self::ProductTemplateV4),
            #[cfg(feature = "non-production-product-series-lab")]
            35 => Ok(Self::PriceMeasurePolicyV1),
            #[cfg(feature = "non-production-product-series-lab")]
            36 => Ok(Self::MarketGenesisProfileV2),
            #[cfg(feature = "non-production-product-series-lab")]
            37 => Ok(Self::SeriesFundingQuoteV1),
            #[cfg(feature = "non-production-product-series-lab")]
            38 => Ok(Self::SeriesAttachmentPlanV1),
            #[cfg(feature = "non-production-product-series-lab")]
            39 => Ok(Self::SeriesPlanV5),
            #[cfg(feature = "non-production-product-series-lab")]
            40 => Ok(Self::SeriesFundingTermsV2),
            41 => Ok(Self::RegistryProgramReleaseV1),
            #[cfg(feature = "non-production-product-series-lab")]
            42 => Ok(Self::CompiledProductSeriesBundleV1),
            43 => Ok(Self::RegistryCapabilityProfileV2),
            44 => Ok(Self::SourceReleaseManifestV1),
            45 => Ok(Self::SourceWorkScheduleV1),
            46 => Ok(Self::MarketInstancePreimageV2),
            47 => Ok(Self::SourceReleaseManifestV2),
            48 => Ok(Self::SeriesFundingQuoteV2),
            49 => Ok(Self::CompiledProductSeriesBundleV2),
            50 => Ok(Self::SeriesAttachmentPlanV2),
            _ => Err(CodecError::InvalidEnum),
        }
    }

    /// Stable wire discriminant.
    pub const fn byte(self) -> u8 {
        self as u8
    }

    /// Exact canonical body length for this kind.
    pub const fn exact_len(self) -> usize {
        match self {
            Self::CollateralPolicy => COLLATERAL_POLICY_V2_BYTES,
            Self::PriceGrid => account_len::PRICE_GRID,
            Self::Terms => account_len::TERMS,
            Self::BatchPolicy => BATCH_POLICY_BYTES,
            Self::DirectBatchPolicyV3 => DIRECT_BATCH_POLICY_V3_BYTES,
            Self::NativeClaimBasisV1 => PRODUCT_BASIS_BYTES,
            Self::EvidenceOnlyRecoveryPolicyV1 => PRODUCT_RECOVERY_BYTES,
            Self::ProductTemplateV4 => PRODUCT_TEMPLATE_V4_BYTES,
            Self::PriceMeasurePolicyV1 => PRODUCT_PRICE_MEASURE_POLICY_BYTES,
            Self::MarketGenesisProfileV2 => PRODUCT_MARKET_GENESIS_V2_BYTES,
            Self::SeriesFundingQuoteV1 => PRODUCT_FUNDING_QUOTE_BYTES,
            Self::SeriesAttachmentPlanV1 => PRODUCT_ATTACHMENT_PLAN_BYTES,
            Self::SeriesPlanV5 => PRODUCT_SERIES_PLAN_V5_BYTES,
            Self::SeriesFundingTermsV2 => PRODUCT_FUNDING_TERMS_V2_BYTES,
            Self::RegistryProgramReleaseV1 => REGISTRY_PROGRAM_RELEASE_V1_BYTES,
            Self::CompiledProductSeriesBundleV1 => COMPILED_PRODUCT_SERIES_BUNDLE_V1_BYTES,
            Self::RegistryCapabilityProfileV2 => REGISTRY_CAPABILITY_PROFILE_V2_BYTES,
            Self::SourceReleaseManifestV1 => SOURCE_RELEASE_MANIFEST_V1_BYTES,
            Self::SourceWorkScheduleV1 => SOURCE_WORK_SCHEDULE_BYTES,
            Self::MarketInstancePreimageV2 => MARKET_INSTANCE_PREIMAGE_V2_BYTES,
            Self::SourceReleaseManifestV2 => SOURCE_RELEASE_MANIFEST_BYTES,
            Self::SeriesFundingQuoteV2 => SERIES_FUNDING_QUOTE_BYTES_V2,
            Self::CompiledProductSeriesBundleV2 => COMPILED_PRODUCT_SERIES_BUNDLE_V2_BYTES,
            Self::SeriesAttachmentPlanV2 => SERIES_ATTACHMENT_PLAN_BYTES_V2,
        }
    }

    /// Whether this kind is a globally content-addressed protocol body.
    ///
    /// These artifacts are reusable across Realms. Their upload context is
    /// therefore the exact zero sentinel; Realm binding is checked later from
    /// the Genesis, Series, and Failure-policy bodies, never smuggled into
    /// transport identity.
    pub const fn is_globally_content_addressed(self) -> bool {
        matches!(
            self,
            Self::NativeClaimBasisV1
                | Self::EvidenceOnlyRecoveryPolicyV1
                | Self::ProductTemplateV4
                | Self::PriceMeasurePolicyV1
                | Self::MarketGenesisProfileV2
                | Self::SeriesFundingQuoteV1
                | Self::SeriesAttachmentPlanV1
                | Self::SeriesPlanV5
                | Self::SeriesFundingTermsV2
                | Self::RegistryProgramReleaseV1
                | Self::CompiledProductSeriesBundleV1
                | Self::RegistryCapabilityProfileV2
                | Self::SourceReleaseManifestV1
                | Self::SourceWorkScheduleV1
                | Self::MarketInstancePreimageV2
                | Self::SourceReleaseManifestV2
                | Self::SeriesFundingQuoteV2
                | Self::CompiledProductSeriesBundleV2
                | Self::SeriesAttachmentPlanV2
        )
    }
}

/// The immutable identity of one upload and its eventual final artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactBinding {
    /// Codec family.
    pub kind: ArtifactKind,
    /// Profile id for a collateral policy; Realm id for grid and terms; the
    /// canonical zero sentinel for globally reusable successor bodies.
    pub context: Hash32,
    /// Canonical semantic digest owned by the artifact codec.
    pub digest: Hash32,
    /// Exact byte length, redundantly checked against [`ArtifactKind`].
    pub exact_len: u16,
}

impl ArtifactBinding {
    /// Refuse a zero digest, a noncanonical context, invented lengths, and
    /// bodies above the bound.
    pub fn validate(&self) -> Result<()> {
        if is_zero(&self.digest.0)
            || (self.kind.is_globally_content_addressed() && self.context != Hash32::ZERO)
            || (!self.kind.is_globally_content_addressed() && is_zero(&self.context.0))
        {
            return Err(CodecError::ZeroIdentity);
        }
        if self.exact_len as usize != self.kind.exact_len()
            || self.exact_len as usize > MAX_ARTIFACT_BYTES
        {
            return Err(CodecError::InvalidCount);
        }
        Ok(())
    }
}

/// Decoded header of one in-progress artifact upload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactStageHeader {
    /// Immutable artifact identity.
    pub binding: ArtifactBinding,
    /// Wallet that created, funds, writes, seals, and may abort the stage.
    pub funder: [u8; HASH_BYTES],
    /// First byte not yet written.
    pub cursor: u16,
    /// Slot at which the stage was created.
    pub created_slot: u64,
    /// Last slot at which a write or seal is admitted.
    pub expires_slot: u64,
    /// Canonical staging-PDA bump.
    pub stored_bump: u8,
}

impl ArtifactStageHeader {
    /// Total account size required for this stage.
    pub fn account_len(&self) -> Result<usize> {
        self.validate()?;
        ARTIFACT_STAGE_HEADER_BYTES
            .checked_add(self.binding.exact_len as usize)
            .ok_or(CodecError::ArithmeticOverflow)
    }

    /// Refuse impossible upload geometry or authority/time metadata.
    pub fn validate(&self) -> Result<()> {
        self.binding.validate()?;
        if is_zero(&self.funder) {
            return Err(CodecError::ZeroIdentity);
        }
        if self.created_slot >= self.expires_slot {
            return Err(CodecError::InvalidCount);
        }
        if self.cursor > self.binding.exact_len {
            return Err(CodecError::InvalidCount);
        }
        if self.cursor != self.binding.exact_len
            && usize::from(self.cursor) % ARTIFACT_CHUNK_BYTES != 0
        {
            return Err(CodecError::InvalidCount);
        }
        Ok(())
    }

    /// Whether every artifact byte has been admitted.
    pub const fn is_complete(&self) -> bool {
        self.cursor == self.binding.exact_len
    }
}

fn put_u16(out: &mut [u8], at: &mut usize, value: u16) {
    out[*at..*at + 2].copy_from_slice(&value.to_le_bytes());
    *at += 2;
}

fn put_u64(out: &mut [u8], at: &mut usize, value: u64) {
    out[*at..*at + 8].copy_from_slice(&value.to_le_bytes());
    *at += 8;
}

fn take_u16(input: &[u8], at: &mut usize) -> u16 {
    let value = u16::from_le_bytes([input[*at], input[*at + 1]]);
    *at += 2;
    value
}

fn take_u64(input: &[u8], at: &mut usize) -> u64 {
    let value = u64::from_le_bytes([
        input[*at],
        input[*at + 1],
        input[*at + 2],
        input[*at + 3],
        input[*at + 4],
        input[*at + 5],
        input[*at + 6],
        input[*at + 7],
    ]);
    *at += 8;
    value
}

fn copy_32(input: &[u8], at: &mut usize) -> [u8; HASH_BYTES] {
    let mut value = [0; HASH_BYTES];
    value.copy_from_slice(&input[*at..*at + HASH_BYTES]);
    *at += HASH_BYTES;
    value
}

fn encode_header_prefix(out: &mut [u8], header: &ArtifactStageHeader) -> Result<()> {
    header.validate()?;
    if out.len() < ARTIFACT_STAGE_HEADER_BYTES {
        return Err(CodecError::OutputTooSmall);
    }
    let mut at = 0;
    out[at] = ARTIFACT_STAGE_TAG;
    at += 1;
    out[at] = ARTIFACT_STAGE_VERSION;
    at += 1;
    out[at] = header.binding.kind.byte();
    at += 1;
    out[at] = header.stored_bump;
    at += 1;
    put_u16(out, &mut at, header.binding.exact_len);
    put_u16(out, &mut at, header.cursor);
    put_u64(out, &mut at, header.created_slot);
    put_u64(out, &mut at, header.expires_slot);
    out[at..at + HASH_BYTES].copy_from_slice(&header.funder);
    at += HASH_BYTES;
    out[at..at + HASH_BYTES].copy_from_slice(&header.binding.context.0);
    at += HASH_BYTES;
    out[at..at + HASH_BYTES].copy_from_slice(&header.binding.digest.0);
    at += HASH_BYTES;
    out[at..at + ARTIFACT_STAGE_RESERVED_BYTES].fill(0);
    at += ARTIFACT_STAGE_RESERVED_BYTES;
    if at != ARTIFACT_STAGE_HEADER_BYTES {
        return Err(CodecError::OutputTooSmall);
    }
    Ok(())
}

/// Initialize an exact-size staging account, including its canonical zero tail.
pub fn initialize_stage(out: &mut [u8], header: &ArtifactStageHeader) -> Result<()> {
    if out.len() != header.account_len()? {
        return Err(CodecError::OutputTooSmall);
    }
    out.fill(0);
    encode_header_prefix(out, header)
}

/// Decode and fully validate a staging account.
///
/// Bytes beyond the cursor must still be zero.  This is redundant for a
/// normally program-owned account but makes hostile genesis fixtures and
/// corrupted state fail closed rather than masquerade as unwritten space.
pub fn decode_stage(input: &[u8]) -> Result<ArtifactStageHeader> {
    if input.len() < ARTIFACT_STAGE_HEADER_BYTES {
        return Err(CodecError::Truncated);
    }
    if input[0] != ARTIFACT_STAGE_TAG {
        return Err(CodecError::WrongTag);
    }
    if input[1] != ARTIFACT_STAGE_VERSION {
        return Err(CodecError::WrongVersion);
    }
    let mut at = 2;
    let kind = ArtifactKind::from_byte(input[at])?;
    at += 1;
    let stored_bump = input[at];
    at += 1;
    let exact_len = take_u16(input, &mut at);
    let cursor = take_u16(input, &mut at);
    let created_slot = take_u64(input, &mut at);
    let expires_slot = take_u64(input, &mut at);
    let funder = copy_32(input, &mut at);
    let context = Hash32::from_bytes(copy_32(input, &mut at));
    let digest = Hash32::from_bytes(copy_32(input, &mut at));
    let reserved = &input[at..at + ARTIFACT_STAGE_RESERVED_BYTES];
    at += ARTIFACT_STAGE_RESERVED_BYTES;
    if reserved.iter().any(|byte| *byte != 0) {
        return Err(CodecError::NonCanonicalPadding);
    }
    if at != ARTIFACT_STAGE_HEADER_BYTES {
        return Err(CodecError::TrailingBytes);
    }
    let header = ArtifactStageHeader {
        binding: ArtifactBinding {
            kind,
            context,
            digest,
            exact_len,
        },
        funder,
        cursor,
        created_slot,
        expires_slot,
        stored_bump,
    };
    if input.len() != header.account_len()? {
        return Err(CodecError::TrailingBytes);
    }
    let unwritten = ARTIFACT_STAGE_HEADER_BYTES + usize::from(header.cursor);
    if input[unwritten..].iter().any(|byte| *byte != 0) {
        return Err(CodecError::NonCanonicalPadding);
    }
    Ok(header)
}

/// Return the complete or partial payload after validating the whole stage.
pub fn stage_payload(input: &[u8]) -> Result<&[u8]> {
    decode_stage(input)?;
    Ok(&input[ARTIFACT_STAGE_HEADER_BYTES..])
}

/// Append exactly the next fixed-size chunk, or the unique shorter final one.
///
/// Duplicate chunks, gaps, overlaps, mixed artifact bindings, nonzero wire
/// padding, and writes after completion all refuse before any byte changes.
pub fn append_chunk(
    stage: &mut [u8],
    binding: ArtifactBinding,
    expected_cursor: u16,
    chunk_len: u16,
    chunk: &[u8; ARTIFACT_CHUNK_BYTES],
) -> Result<ArtifactStageHeader> {
    let mut header = decode_stage(stage)?;
    binding.validate()?;
    if header.binding != binding || header.cursor != expected_cursor || header.is_complete() {
        return Err(CodecError::MismatchedBinding);
    }
    let remaining = usize::from(header.binding.exact_len - header.cursor);
    let required = if remaining < ARTIFACT_CHUNK_BYTES {
        remaining
    } else {
        ARTIFACT_CHUNK_BYTES
    };
    if usize::from(chunk_len) != required {
        return Err(CodecError::InvalidCount);
    }
    if chunk[required..].iter().any(|byte| *byte != 0) {
        return Err(CodecError::NonCanonicalPadding);
    }
    let start = ARTIFACT_STAGE_HEADER_BYTES + usize::from(header.cursor);
    let end = start + required;
    stage[start..end].copy_from_slice(&chunk[..required]);
    header.cursor = header
        .cursor
        .checked_add(chunk_len)
        .ok_or(CodecError::ArithmeticOverflow)?;
    encode_header_prefix(stage, &header)?;
    Ok(header)
}

/// Validate a complete staged body through the existing owning codec.
///
/// Returns the final account's stored bump for grid/terms and zero for the raw
/// collateral policy, whose encoding intentionally contains no PDA field.
pub fn validate_artifact(binding: ArtifactBinding, body: &[u8]) -> Result<u8> {
    binding.validate()?;
    if body.len() != usize::from(binding.exact_len) {
        return Err(CodecError::Truncated);
    }
    match binding.kind {
        ArtifactKind::CollateralPolicy => {
            let policy =
                CollateralPolicyV2::decode(body).map_err(|_| CodecError::MismatchedBinding)?;
            let policy_id = policy.id().map_err(|_| CodecError::MismatchedBinding)?;
            let policy_id = Hash32::from_bytes(policy_id.bytes());
            let release_id = Hash32::from_bytes(policy.adapter_release.bytes());
            if policy_id != binding.digest
                || canonical_profile_v2_id(policy_id, release_id)? != binding.context
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        ArtifactKind::PriceGrid => {
            let grid = PriceGridAccount::decode(body)?;
            if grid.realm != binding.context || grid.grid != binding.digest {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(grid.stored_bump)
        }
        ArtifactKind::Terms => {
            let terms = TermsAccount::decode(body)?;
            if terms.realm != binding.context || terms.terms != binding.digest {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(terms.stored_bump)
        }
        #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
        ArtifactKind::BatchPolicy => {
            let policy = decode_batch_policy(body).map_err(|_| CodecError::MismatchedBinding)?;
            let digest = batch_policy_digest(&policy).map_err(|_| CodecError::MismatchedBinding)?;
            if digest != Identity32V1(binding.digest.bytes()) {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        #[cfg(feature = "profile-direct-v3-source-v2-point")]
        ArtifactKind::BatchPolicy => Err(CodecError::InvalidEnum),
        #[cfg(any(
            feature = "profile-full",
            feature = "profile-direct-v3-source-v2-point"
        ))]
        ArtifactKind::DirectBatchPolicyV3 => {
            let policy = DirectBatchPolicyV3::decode(body)?;
            if policy.digest_for_epoch(binding.context)? != binding.digest {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        #[cfg(feature = "profile-general-source-v2-point")]
        ArtifactKind::DirectBatchPolicyV3 => Err(CodecError::InvalidEnum),
        #[cfg(all(
            feature = "non-production-product-series-lab",
            not(target_os = "solana")
        ))]
        ArtifactKind::NativeClaimBasisV1 => {
            let value =
                NativeClaimBasisV1::decode(body).map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        #[cfg(all(
            feature = "non-production-product-series-lab",
            not(target_os = "solana")
        ))]
        ArtifactKind::EvidenceOnlyRecoveryPolicyV1 => {
            let value = EvidenceOnlyRecoveryPolicyV1::decode(body)
                .map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        #[cfg(all(
            feature = "non-production-product-series-lab",
            not(target_os = "solana")
        ))]
        ArtifactKind::ProductTemplateV4 => {
            let value =
                ProductTemplateV4::decode(body).map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        #[cfg(all(
            feature = "non-production-product-series-lab",
            not(target_os = "solana")
        ))]
        ArtifactKind::PriceMeasurePolicyV1 => {
            let value =
                PriceMeasurePolicyV1::decode(body).map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        #[cfg(all(
            feature = "non-production-product-series-lab",
            not(target_os = "solana")
        ))]
        ArtifactKind::MarketGenesisProfileV2 => {
            let value =
                MarketGenesisProfileV2::decode(body).map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        #[cfg(all(
            feature = "non-production-product-series-lab",
            not(target_os = "solana")
        ))]
        ArtifactKind::SeriesFundingQuoteV1 => {
            let value =
                SeriesFundingQuoteV1::decode(body).map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        #[cfg(all(
            feature = "non-production-product-series-lab",
            not(target_os = "solana")
        ))]
        ArtifactKind::SeriesAttachmentPlanV1 => {
            let value =
                SeriesAttachmentPlanV1::decode(body).map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        #[cfg(all(
            feature = "non-production-product-series-lab",
            not(target_os = "solana")
        ))]
        ArtifactKind::SeriesPlanV5 => {
            let value = SeriesPlanV5::decode(body).map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        #[cfg(all(
            feature = "non-production-product-series-lab",
            not(target_os = "solana")
        ))]
        ArtifactKind::SeriesFundingTermsV2 => {
            let value =
                SeriesFundingTermsV2::decode(body).map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        ArtifactKind::RegistryProgramReleaseV1 => {
            let value = RegistryProgramReleaseV1::decode(body)
                .map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        ArtifactKind::RegistryCapabilityProfileV2 => {
            let value = RegistryCapabilityProfileV2::decode(body)
                .map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        ArtifactKind::SourceReleaseManifestV1 => {
            let value =
                SourceReleaseManifestV1::decode(body).map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        ArtifactKind::SourceReleaseManifestV2 => {
            let value =
                SourceReleaseManifestV2::decode(body).map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        ArtifactKind::SourceWorkScheduleV1 => {
            let value = SourceWorkScheduleBindingV1::decode(body)
                .map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        ArtifactKind::MarketInstancePreimageV2 => {
            let value = MarketInstancePreimageV2::decode(body)
                .map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        ArtifactKind::SeriesFundingQuoteV2 => {
            let value =
                SeriesFundingQuoteV2::decode(body).map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        ArtifactKind::CompiledProductSeriesBundleV2 => {
            let value = CompiledProductSeriesBundleV2::decode(body)
                .map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        ArtifactKind::SeriesAttachmentPlanV2 => {
            let value =
                SeriesAttachmentPlanV2::decode(body).map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        #[cfg(all(
            feature = "non-production-product-series-lab",
            not(target_os = "solana")
        ))]
        ArtifactKind::CompiledProductSeriesBundleV1 => {
            let value = CompiledProductSeriesBundleV1::decode(body)
                .map_err(|_| CodecError::MismatchedBinding)?;
            if Hash32::from_bytes(
                value
                    .id()
                    .map_err(|_| CodecError::MismatchedBinding)?
                    .bytes(),
            ) != binding.digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(0)
        }
        #[cfg(any(
            not(feature = "non-production-product-series-lab"),
            target_os = "solana"
        ))]
        ArtifactKind::NativeClaimBasisV1
        | ArtifactKind::EvidenceOnlyRecoveryPolicyV1
        | ArtifactKind::ProductTemplateV4
        | ArtifactKind::PriceMeasurePolicyV1
        | ArtifactKind::MarketGenesisProfileV2
        | ArtifactKind::SeriesFundingQuoteV1
        | ArtifactKind::SeriesAttachmentPlanV1
        | ArtifactKind::SeriesPlanV5
        | ArtifactKind::SeriesFundingTermsV2
        | ArtifactKind::CompiledProductSeriesBundleV1 => Err(CodecError::InvalidEnum),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    use clutch_batch_policy_identity::{
        batch_policy_digest, direct_window_v1::DIRECT_POLICY_V1, encode_batch_policy,
    };
    extern crate std;

    #[cfg(feature = "non-production-product-series-lab")]
    use clutch_product_series::{
        ComponentDebitV1, ContentId, RecoveryAttemptFundingV1, RecoveryAttemptV1, SeriesPlanV5Id,
        MAX_OUTCOMES as PRODUCT_MAX_OUTCOMES, MAX_PAYOUTS as PRODUCT_MAX_PAYOUTS,
        MAX_RECOVERY_ATTEMPTS, PAYOUT_MAP_UNUSED, UNIFORM_SPACING_NONE,
    };

    #[cfg(feature = "non-production-product-series-lab")]
    fn product_id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    #[cfg(feature = "non-production-product-series-lab")]
    fn product_basis() -> NativeClaimBasisV1 {
        let mut payout_weights = [[0; PRODUCT_MAX_OUTCOMES]; PRODUCT_MAX_PAYOUTS];
        let mut index = 0_usize;
        while index < 4 {
            payout_weights[index][index] = 1_000;
            index += 1;
        }
        let mut payout_map = [PAYOUT_MAP_UNUSED; PRODUCT_MAX_OUTCOMES];
        payout_map[..4].copy_from_slice(&[0, 1, 2, 3]);
        let mut knots = [0; PRODUCT_MAX_OUTCOMES];
        knots[..3].copy_from_slice(&[100, 200, 300]);
        NativeClaimBasisV1 {
            basis_degree: 0,
            outcome_count: 4,
            payout_count: 4,
            knot_count: 3,
            uniform_log2_spacing: UNIFORM_SPACING_NONE,
            ambiguity_policy_registry_value: 1,
            edge_policy_registry_value: 1,
            denominator: 1_000,
            payout_weights,
            payout_map,
            knots,
        }
    }

    #[cfg(feature = "non-production-product-series-lab")]
    fn product_recovery() -> EvidenceOnlyRecoveryPolicyV1 {
        let mut attempts = [RecoveryAttemptV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        attempts[0] = RecoveryAttemptV1 {
            repair_generation_delta: 0,
            opens_after_primary_maturity_buckets: 0,
            closes_after_primary_maturity_buckets: 2,
        };
        attempts[1] = RecoveryAttemptV1 {
            repair_generation_delta: 1,
            opens_after_primary_maturity_buckets: 2,
            closes_after_primary_maturity_buckets: 5,
        };
        EvidenceOnlyRecoveryPolicyV1 {
            attempt_count: 2,
            attempts,
        }
    }

    #[cfg(feature = "non-production-product-series-lab")]
    fn product_template() -> ProductTemplateV4 {
        ProductTemplateV4 {
            source_plane_contract_id: product_id(1),
            source_spec_id: product_id(2),
            summary_program_id: product_id(3),
            native_claim_basis_id: product_basis().id().unwrap(),
            evidence_only_recovery_policy_id: product_recovery().id().unwrap(),
            compiler_release_id: product_id(4),
            statistic_registry_value: 11,
            coverage_policy_registry_value: 12,
            window_span_buckets: 4,
            primary_maturity_grace_buckets: 2,
            base_repair_generation: 10,
            coverage_policy_parameter: 0,
        }
    }

    #[cfg(feature = "non-production-product-series-lab")]
    fn product_price_policy() -> PriceMeasurePolicyV1 {
        PriceMeasurePolicyV1 {
            checker_release_id: product_id(30),
            checker_version: 3,
            quantized_semantics_version: 1,
            minimum_basis_degree: 0,
            maximum_basis_degree: 3,
            maximum_outcome_count: 16,
            maximum_atom_count: 16,
            maximum_payout_denominator: u64::MAX,
            maximum_witness_denominator: u64::MAX,
            maximum_price_scale: u64::MAX / 16,
        }
    }

    #[cfg(feature = "non-production-product-series-lab")]
    fn product_genesis() -> MarketGenesisProfileV2 {
        MarketGenesisProfileV2 {
            realm_id: product_id(20),
            profile_id: product_id(21),
            price_grid_id: product_id(22),
            price_measure_policy_id: product_price_policy().id().unwrap(),
            fee_policy_id: product_id(23),
            relation_policy_id: product_id(24),
            score_policy_id: product_id(25),
            candidate_lifecycle_policy_id: product_id(26),
            candidate_liveness_policy_id: product_id(27),
            retirement_policy_id: product_id(28),
            capability_profile_id: product_id(29),
            terminal_disposition_registry_value: 7,
            native_bearer_lot: 1_000,
            coordinate_domain_min: 0,
            coordinate_domain_max: 400,
        }
    }

    #[cfg(feature = "non-production-product-series-lab")]
    fn product_quote() -> SeriesFundingQuoteV1 {
        let mut attempts = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        attempts[0] = RecoveryAttemptFundingV1 {
            max_progress_units: 3,
            lamports_per_progress_unit: 5,
        };
        attempts[1] = RecoveryAttemptFundingV1 {
            max_progress_units: 2,
            lamports_per_progress_unit: 7,
        };
        SeriesFundingQuoteV1 {
            evidence_only_recovery_policy_id: product_recovery().id().unwrap(),
            market_core: ComponentDebitV1 {
                lamports: 10,
                collateral_atoms: 0,
            },
            failure_root_rent_principal_lamports: 3,
            failure_replay_tombstone_rent_principal_lamports: 2,
            recovery_reserve: ComponentDebitV1 {
                lamports: 40,
                collateral_atoms: 0,
            },
            source_work: ComponentDebitV1 {
                lamports: 30,
                collateral_atoms: 0,
            },
            liquidity_facility: ComponentDebitV1 {
                lamports: 40,
                collateral_atoms: 100,
            },
            wrapper_set: ComponentDebitV1 {
                lamports: 50,
                collateral_atoms: 10,
            },
            recovery_attempt_count: 2,
            recovery_attempt_funding: attempts,
            recovery_rent_principal_lamports: 11,
        }
    }

    #[cfg(feature = "non-production-product-series-lab")]
    fn product_attachment() -> SeriesAttachmentPlanV1 {
        SeriesAttachmentPlanV1 {
            funding_quote_id: product_quote().id().unwrap(),
            liquidity_facility_plan_id: product_id(41),
            wrapper_recipe_set_id: product_id(42),
        }
    }

    #[cfg(feature = "non-production-product-series-lab")]
    fn product_series() -> SeriesPlanV5 {
        SeriesPlanV5 {
            product_template_id: product_template().id().unwrap(),
            market_genesis_profile_id: product_genesis().id().unwrap(),
            attachment_plan_id: product_attachment().id().unwrap(),
            first_start_bucket: 100,
            stride_buckets: 10,
            instance_count: 3,
            creation_lead_buckets: 5,
            market_collateral_cap: 1_000,
        }
    }

    #[cfg(feature = "non-production-product-series-lab")]
    fn product_funding_terms() -> SeriesFundingTermsV2 {
        SeriesFundingTermsV2 {
            series_plan_id: SeriesPlanV5Id::from_bytes(product_series().id().unwrap().bytes()),
            lamport_principal_refund: product_id(50),
            collateral_principal_refund_token_account: product_id(51),
            neutral_collateral_disposition_token_account: product_id(52),
            neutral_lamport_sink: product_id(55),
            collateral_mint: product_id(53),
            token_program: product_id(54),
        }
    }

    #[cfg(feature = "non-production-product-series-lab")]
    fn compiled_product_series_bundle() -> CompiledProductSeriesBundleV1 {
        let template = product_template();
        CompiledProductSeriesBundleV1 {
            registry_release_id: product_id(60),
            capability_profile_id: product_genesis().capability_profile_id,
            source_release_manifest_id: product_id(61),
            source_plane_contract_id: template.source_plane_contract_id,
            source_spec_id: template.source_spec_id,
            summary_program_id: template.summary_program_id,
            product_compiler_release_id: template.compiler_release_id,
            native_claim_basis_id: product_basis().id().unwrap(),
            evidence_only_recovery_policy_id: product_recovery().id().unwrap(),
            product_template_id: template.id().unwrap(),
            price_measure_policy_id: product_price_policy().id().unwrap(),
            market_genesis_profile_id: product_genesis().id().unwrap(),
            funding_quote_id: product_quote().id().unwrap(),
            attachment_plan_id: product_attachment().id().unwrap(),
            series_plan_id: product_series().id().unwrap(),
            funding_terms_id: product_funding_terms().id().unwrap(),
        }
    }

    #[cfg(feature = "non-production-product-series-lab")]
    fn assert_product_artifact(kind: ArtifactKind, body: &[u8], digest: [u8; 32]) {
        let binding = ArtifactBinding {
            kind,
            context: Hash32::ZERO,
            digest: Hash32::from_bytes(digest),
            exact_len: kind.exact_len() as u16,
        };
        assert_eq!(validate_artifact(binding, body), Ok(0), "{kind:?}");

        let mut wrong_digest = binding;
        let mut bytes = wrong_digest.digest.bytes();
        bytes[0] ^= 1;
        wrong_digest.digest = Hash32::from_bytes(bytes);
        assert_eq!(
            validate_artifact(wrong_digest, body),
            Err(CodecError::MismatchedBinding),
            "{kind:?} digest"
        );

        let mut contextualized = binding;
        contextualized.context = Hash32::from_bytes([1; 32]);
        assert_eq!(
            validate_artifact(contextualized, body),
            Err(CodecError::ZeroIdentity),
            "{kind:?} context"
        );
    }

    fn binding(kind: ArtifactKind) -> ArtifactBinding {
        ArtifactBinding {
            kind,
            context: if kind.is_globally_content_addressed() {
                Hash32::ZERO
            } else {
                Hash32::from_bytes([0x31; 32])
            },
            digest: Hash32::from_bytes([0x52; 32]),
            exact_len: kind.exact_len() as u16,
        }
    }

    #[cfg(feature = "non-production-product-series-lab")]
    #[test]
    fn every_product_series_kind_is_exactly_typed_and_globally_context_free() {
        macro_rules! check {
            ($kind:expr, $type:ty, $value:expr) => {{
                let value = $value;
                let mut body = std::vec![0_u8; <$type as FixedCodec>::ENCODED_LEN];
                value.encode_into(&mut body).unwrap();
                assert_product_artifact($kind, &body, value.id().unwrap().bytes());
            }};
        }

        check!(
            ArtifactKind::NativeClaimBasisV1,
            NativeClaimBasisV1,
            product_basis()
        );
        check!(
            ArtifactKind::EvidenceOnlyRecoveryPolicyV1,
            EvidenceOnlyRecoveryPolicyV1,
            product_recovery()
        );
        check!(
            ArtifactKind::ProductTemplateV4,
            ProductTemplateV4,
            product_template()
        );
        check!(
            ArtifactKind::PriceMeasurePolicyV1,
            PriceMeasurePolicyV1,
            product_price_policy()
        );
        check!(
            ArtifactKind::MarketGenesisProfileV2,
            MarketGenesisProfileV2,
            product_genesis()
        );
        check!(
            ArtifactKind::SeriesFundingQuoteV1,
            SeriesFundingQuoteV1,
            product_quote()
        );
        check!(
            ArtifactKind::SeriesAttachmentPlanV1,
            SeriesAttachmentPlanV1,
            product_attachment()
        );
        check!(ArtifactKind::SeriesPlanV5, SeriesPlanV5, product_series());
        check!(
            ArtifactKind::SeriesFundingTermsV2,
            SeriesFundingTermsV2,
            product_funding_terms()
        );
        check!(
            ArtifactKind::CompiledProductSeriesBundleV1,
            CompiledProductSeriesBundleV1,
            compiled_product_series_bundle()
        );

        for (tag, expected) in (u8::MIN..=u8::MAX).map(|tag| {
            let expected = if (32..=50).contains(&tag) {
                Ok(match tag {
                    32 => ArtifactKind::NativeClaimBasisV1,
                    33 => ArtifactKind::EvidenceOnlyRecoveryPolicyV1,
                    34 => ArtifactKind::ProductTemplateV4,
                    35 => ArtifactKind::PriceMeasurePolicyV1,
                    36 => ArtifactKind::MarketGenesisProfileV2,
                    37 => ArtifactKind::SeriesFundingQuoteV1,
                    38 => ArtifactKind::SeriesAttachmentPlanV1,
                    39 => ArtifactKind::SeriesPlanV5,
                    40 => ArtifactKind::SeriesFundingTermsV2,
                    41 => ArtifactKind::RegistryProgramReleaseV1,
                    42 => ArtifactKind::CompiledProductSeriesBundleV1,
                    43 => ArtifactKind::RegistryCapabilityProfileV2,
                    44 => ArtifactKind::SourceReleaseManifestV1,
                    45 => ArtifactKind::SourceWorkScheduleV1,
                    46 => ArtifactKind::MarketInstancePreimageV2,
                    47 => ArtifactKind::SourceReleaseManifestV2,
                    48 => ArtifactKind::SeriesFundingQuoteV2,
                    49 => ArtifactKind::CompiledProductSeriesBundleV2,
                    50 => ArtifactKind::SeriesAttachmentPlanV2,
                    _ => unreachable!(),
                })
            } else {
                ArtifactKind::from_byte(tag)
            };
            (tag, expected)
        }) {
            if (32..=50).contains(&tag) {
                assert_eq!(ArtifactKind::from_byte(tag), expected, "kind {tag}");
            }
        }
    }

    fn header(kind: ArtifactKind) -> ArtifactStageHeader {
        ArtifactStageHeader {
            binding: binding(kind),
            funder: [0x73; 32],
            cursor: 0,
            created_slot: 40,
            expires_slot: 400,
            stored_bump: 254,
        }
    }

    #[test]
    fn stage_lengths_and_round_trip_are_exact() {
        fn round_trip(kind: ArtifactKind) {
            let h = header(kind);
            let mut bytes = std::vec![0xa5; h.account_len().unwrap()];
            initialize_stage(&mut bytes, &h).unwrap();
            assert_eq!(decode_stage(&bytes), Ok(h));
            assert_eq!(
                stage_payload(&bytes).unwrap(),
                &bytes[ARTIFACT_STAGE_HEADER_BYTES..]
            );
            assert!(bytes[ARTIFACT_STAGE_HEADER_BYTES..]
                .iter()
                .all(|byte| *byte == 0));
        }
        for kind in [
            ArtifactKind::CollateralPolicy,
            ArtifactKind::PriceGrid,
            ArtifactKind::Terms,
        ] {
            round_trip(kind);
        }
        #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
        round_trip(ArtifactKind::BatchPolicy);
        #[cfg(any(
            feature = "profile-full",
            feature = "profile-direct-v3-source-v2-point"
        ))]
        round_trip(ArtifactKind::DirectBatchPolicyV3);
    }

    #[test]
    fn ordered_chunks_reject_every_ambiguity() {
        let h = header(ArtifactKind::CollateralPolicy);
        let mut bytes = std::vec![0; h.account_len().unwrap()];
        initialize_stage(&mut bytes, &h).unwrap();
        let mut first = [0; ARTIFACT_CHUNK_BYTES];
        first.fill(0x19);

        let before = bytes.clone();
        assert_eq!(
            append_chunk(&mut bytes, h.binding, 1, 192, &first),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(bytes, before);

        let post = append_chunk(&mut bytes, h.binding, 0, 192, &first).unwrap();
        assert_eq!(post.cursor, 192);
        let after_first = bytes.clone();
        assert_eq!(
            append_chunk(&mut bytes, h.binding, 0, 192, &first),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(bytes, after_first);

        let mut final_chunk = [0; ARTIFACT_CHUNK_BYTES];
        final_chunk[..74].fill(0x2a);
        assert_eq!(
            append_chunk(&mut bytes, h.binding, 192, 73, &final_chunk),
            Err(CodecError::InvalidCount)
        );
        final_chunk[100] = 1;
        assert_eq!(
            append_chunk(&mut bytes, h.binding, 192, 74, &final_chunk),
            Err(CodecError::NonCanonicalPadding)
        );
        final_chunk[100] = 0;
        assert!(append_chunk(&mut bytes, h.binding, 192, 74, &final_chunk)
            .unwrap()
            .is_complete());
        let complete = bytes.clone();
        assert_eq!(
            append_chunk(&mut bytes, h.binding, 266, 0, &[0; ARTIFACT_CHUNK_BYTES]),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(bytes, complete);
    }

    #[test]
    fn hostile_stage_bytes_fail_closed() {
        let h = header(ArtifactKind::Terms);
        let mut bytes = std::vec![0; h.account_len().unwrap()];
        initialize_stage(&mut bytes, &h).unwrap();

        let mut bad = bytes.clone();
        bad[0] ^= 1;
        assert_eq!(decode_stage(&bad), Err(CodecError::WrongTag));
        bad = bytes.clone();
        bad[1] = 9;
        assert_eq!(decode_stage(&bad), Err(CodecError::WrongVersion));
        bad = bytes.clone();
        bad[2] = 9;
        assert_eq!(decode_stage(&bad), Err(CodecError::InvalidEnum));
        bad = bytes.clone();
        bad[ARTIFACT_STAGE_HEADER_BYTES - 1] = 1;
        assert_eq!(decode_stage(&bad), Err(CodecError::NonCanonicalPadding));
        bad = bytes.clone();
        bad[ARTIFACT_STAGE_HEADER_BYTES + 1] = 1;
        assert_eq!(decode_stage(&bad), Err(CodecError::NonCanonicalPadding));
        assert_eq!(
            decode_stage(&bytes[..bytes.len() - 1]),
            Err(CodecError::TrailingBytes)
        );
    }

    #[test]
    fn invented_kinds_lengths_and_times_refuse() {
        assert_eq!(ArtifactKind::from_byte(0), Err(CodecError::InvalidEnum));
        let mut h = header(ArtifactKind::Terms);
        h.binding.exact_len -= 1;
        assert_eq!(h.validate(), Err(CodecError::InvalidCount));
        h = header(ArtifactKind::Terms);
        h.expires_slot = h.created_slot;
        assert_eq!(h.validate(), Err(CodecError::InvalidCount));
        h = header(ArtifactKind::Terms);
        h.cursor = 1;
        assert_eq!(h.validate(), Err(CodecError::InvalidCount));
    }

    #[cfg(not(feature = "non-production-product-series-lab"))]
    #[test]
    fn production_profiles_refuse_product_series_but_admit_central_registry_artifacts() {
        for kind in (32..=40).chain(core::iter::once(42)) {
            assert_eq!(ArtifactKind::from_byte(kind), Err(CodecError::InvalidEnum));
        }
        assert_eq!(
            ArtifactKind::from_byte(41),
            Ok(ArtifactKind::RegistryProgramReleaseV1)
        );
        assert_eq!(
            ArtifactKind::from_byte(43),
            Ok(ArtifactKind::RegistryCapabilityProfileV2)
        );
        assert_eq!(
            ArtifactKind::from_byte(44),
            Ok(ArtifactKind::SourceReleaseManifestV1)
        );
        assert_eq!(
            ArtifactKind::from_byte(45),
            Ok(ArtifactKind::SourceWorkScheduleV1)
        );
        assert_eq!(
            ArtifactKind::from_byte(47),
            Ok(ArtifactKind::SourceReleaseManifestV2)
        );
        assert_eq!(
            ArtifactKind::from_byte(48),
            Ok(ArtifactKind::SeriesFundingQuoteV2)
        );
        assert_eq!(
            ArtifactKind::from_byte(49),
            Ok(ArtifactKind::CompiledProductSeriesBundleV2)
        );
        assert_eq!(
            ArtifactKind::from_byte(50),
            Ok(ArtifactKind::SeriesAttachmentPlanV2)
        );
        let source = binding(ArtifactKind::SourceReleaseManifestV1);
        assert_eq!(source.exact_len, 1_008);
        assert_eq!(
            validate_artifact(source, &[0; SOURCE_RELEASE_MANIFEST_V1_BYTES]),
            Err(CodecError::MismatchedBinding)
        );
        let successor = binding(ArtifactKind::SourceReleaseManifestV2);
        assert_eq!(successor.exact_len, 1_296);
        assert_eq!(
            validate_artifact(successor, &[0; SOURCE_RELEASE_MANIFEST_BYTES]),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    fn batch_policy_artifact_uses_the_canonical_policy_codec() {
        let mut bytes = [0u8; BATCH_POLICY_BYTES];
        assert_eq!(
            encode_batch_policy(&DIRECT_POLICY_V1, &mut bytes),
            Ok(BATCH_POLICY_BYTES)
        );
        let digest = batch_policy_digest(&DIRECT_POLICY_V1).unwrap();
        let binding = ArtifactBinding {
            kind: ArtifactKind::BatchPolicy,
            context: Hash32::from_bytes([0x44; 32]),
            digest: Hash32::from_bytes(digest.0),
            exact_len: BATCH_POLICY_BYTES as u16,
        };
        assert_eq!(validate_artifact(binding, &bytes), Ok(0));
        let mut hostile = bytes;
        hostile[12] ^= 1;
        assert_eq!(
            validate_artifact(binding, &hostile),
            Err(CodecError::MismatchedBinding)
        );
        let substituted = ArtifactBinding {
            digest: Hash32::from_bytes([0x55; 32]),
            ..binding
        };
        assert_eq!(
            validate_artifact(substituted, &bytes),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    #[cfg(any(
        feature = "profile-full",
        feature = "profile-direct-v3-source-v2-point"
    ))]
    fn direct_batch_policy_artifact_binds_kind_context_release_and_all_bytes() {
        let context = Hash32::from_bytes([0x44; 32]);
        let value = DirectBatchPolicyV3::direct(Hash32::from_bytes([0x77; 32])).unwrap();
        let mut bytes = [0u8; DIRECT_BATCH_POLICY_V3_BYTES];
        value.encode(&mut bytes).unwrap();
        let binding = ArtifactBinding {
            kind: ArtifactKind::DirectBatchPolicyV3,
            context,
            digest: value.digest_for_epoch(context).unwrap(),
            exact_len: DIRECT_BATCH_POLICY_V3_BYTES as u16,
        };
        assert_eq!(validate_artifact(binding, &bytes), Ok(0));

        let old_kind = ArtifactBinding {
            kind: ArtifactKind::BatchPolicy,
            exact_len: BATCH_POLICY_BYTES as u16,
            ..binding
        };
        #[cfg(feature = "profile-full")]
        assert_eq!(
            validate_artifact(old_kind, &bytes[..BATCH_POLICY_BYTES]),
            Err(CodecError::MismatchedBinding)
        );
        #[cfg(feature = "profile-direct-v3-source-v2-point")]
        assert_eq!(
            validate_artifact(old_kind, &bytes[..BATCH_POLICY_BYTES]),
            Err(CodecError::InvalidEnum)
        );
        let substituted_context = ArtifactBinding {
            context: Hash32::from_bytes([0x45; 32]),
            ..binding
        };
        assert_eq!(
            validate_artifact(substituted_context, &bytes),
            Err(CodecError::MismatchedBinding)
        );
        let mut hostile = bytes;
        hostile[DIRECT_BATCH_POLICY_V3_BYTES - 1] ^= 1;
        assert_eq!(
            validate_artifact(binding, &hostile),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            validate_artifact(binding, &bytes[..DIRECT_BATCH_POLICY_V3_BYTES - 1]),
            Err(CodecError::Truncated)
        );
    }

    #[test]
    #[cfg(feature = "profile-direct-v3-source-v2-point")]
    fn direct_profile_refuses_general_artifact_kind() {
        assert_eq!(ArtifactKind::from_byte(4), Err(CodecError::InvalidEnum));
    }

    #[test]
    #[cfg(feature = "profile-general-source-v2-point")]
    fn general_profile_refuses_direct_artifact_kind() {
        assert_eq!(ArtifactKind::from_byte(5), Err(CodecError::InvalidEnum));
    }
}
