//! Exact bounded source manifest and deterministic rebuild gate.

use core::convert::TryInto;

use dclutch_account_profile_contract::{
    lifecycle_v3::StateLifecyclePolicyV5,
    v2::{AccountProfileV2, SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V2},
};
use dclutch_capability_program_contract::v4::{CapabilityArtifactsV4, CapabilityProgramV4};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::v4::{ProgramV4 as EffectProgramV4, SCHEMA_RELEASE_ID_V4};
use dclutch_execution_strategy_contract::{
    shadow_v3::{SHADOW_ACK_SCHEMA_ID_V3, SHADOW_REQUEST_SCHEMA_ID_V3},
    v2::{
        EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyProgramV2, StrategyDispositionV2,
    },
};
use dclutch_request_profile_contract::{RequestProfileV1, SCHEMA_RELEASE_ID as REQUEST_SCHEMA_ID};
use dclutch_trading_sbf::series::{
    artifacts_v3::{
        SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3, SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
    },
    consume_artifacts_v4::{
        SERIES_CONSUME_EFFECT_BYTES_V4, SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4,
        SERIES_CONSUME_TRANSITION_BYTES_V4, SeriesConsumeChildRequestsV4,
    },
};

use super::{
    CompiledSeriesShadowBundleV4, Result, SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4,
    SeriesShadowBundleCompileErrorV4, SeriesShadowBundleSourceV4,
    SeriesShadowDescriptorSemanticsV4, SeriesShadowReleaseSourcesV4, bundle_digest,
    compile_series_shadow_bundle_v4, content, id, reference,
};
use dclutch_transition_vm::v3::{
    ProgramV3 as TransitionProgramV3, SCHEMA_RELEASE_ID as TRANSITION_SCHEMA_ID,
};

/// Exact source-manifest magic.
pub const SERIES_SHADOW_SOURCE_MANIFEST_MAGIC_V1: [u8; 8] = *b"DCLTSSM1";
/// Exact source-manifest wire version.
pub const SERIES_SHADOW_SOURCE_MANIFEST_VERSION_V1: u16 = 1;
/// Exact occurrence-specific Consume artifact profile.
pub const SERIES_SHADOW_SOURCE_MANIFEST_PROFILE_V1: u16 = 1;
/// Fixed bytes before the exact fixed-width rules and child requests.
pub const SERIES_SHADOW_SOURCE_MANIFEST_HEADER_BYTES_V1: usize = 416;
/// Exact fixed-width rules committed as little-endian `u32` values.
pub const SERIES_SHADOW_SOURCE_FIXED_RULE_BYTES_V1: usize =
    SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4 * 4;
/// Exact four canonical child request bytes before generated bundle sections.
pub const SERIES_SHADOW_SOURCE_CHILD_REQUEST_BYTES_V1: usize = 2
    * SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3
    + 2 * SERIES_CONSUME_CORE_REQUEST_BYTES_V3
    + SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3;
/// Maximum accepted manifest width.
pub const SERIES_SHADOW_SOURCE_MANIFEST_MAX_BYTES_V1: usize = 65_536;

const VERSION_OFFSET: usize = 8;
const PROFILE_OFFSET: usize = 10;
const TOTAL_BYTES_OFFSET: usize = 12;
const SOURCE_LIFECYCLE_BYTES_OFFSET: usize = 16;
const CAPABILITY_BYTES_OFFSET: usize = 20;
const ACCOUNT_BYTES_OFFSET: usize = 24;
const REQUEST_BYTES_OFFSET: usize = 28;
const BUNDLE_LIFECYCLE_BYTES_OFFSET: usize = 32;
const TRANSITION_BYTES_OFFSET: usize = 36;
const EFFECT_BYTES_OFFSET: usize = 40;
const STRATEGY_BYTES_OFFSET: usize = 44;
const FIXED_RULE_COUNT_OFFSET: usize = 48;
const HEADER_RESERVED_OFFSET: usize = 50;
const ROOT_STATE_BYTES_OFFSET: usize = 52;
const TAIL_RESERVED_OFFSET: usize = 56;
const TAIL_RESERVED_BYTES: usize = 8;
const IDENTITIES_OFFSET: usize = 64;
const IDENTITY_COUNT: usize = 11;
const FIXED_RULES_OFFSET: usize = SERIES_SHADOW_SOURCE_MANIFEST_HEADER_BYTES_V1;
const CHILD_REQUESTS_OFFSET: usize = FIXED_RULES_OFFSET + SERIES_SHADOW_SOURCE_FIXED_RULE_BYTES_V1;
const SECTIONS_OFFSET: usize = CHILD_REQUESTS_OFFSET + SERIES_SHADOW_SOURCE_CHILD_REQUEST_BYTES_V1;

const SEMANTIC_SOURCE_IDENTITY: usize = 0;
const COMPILER_SOURCE_IDENTITY: usize = 1;
const TOOLCHAIN_IDENTITY: usize = 2;
const CERTIFICATE_IDENTITY: usize = 3;
const BUNDLE_IDENTITY: usize = 4;
const KIND_IDENTITY: usize = 5;
const CONFIG_SCHEMA_IDENTITY: usize = 6;
const REQUEST_SCHEMA_IDENTITY: usize = 7;
const ROOT_SCHEMA_IDENTITY: usize = 8;
const DERIVATION_POLICY_IDENTITY: usize = 9;
const CAPACITY_PROFILE_IDENTITY: usize = 10;

/// Exact source bytes supplied again during deterministic rebuilding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesShadowRebuildSourcesV1<'a> {
    /// Exact `series/consume_artifacts_v4.rs` bytes.
    pub semantic_source: &'a [u8],
    /// Exact generator source bytes.
    pub compiler_source: &'a [u8],
    /// Exact pinned toolchain manifest bytes.
    pub toolchain_manifest: &'a [u8],
}

/// Hostile-decoded borrowed source manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesShadowSourceManifestV1<'a> {
    bytes: &'a [u8],
    source_lifecycle: &'a [u8],
    capability_program: &'a [u8],
    account_profile: &'a [u8],
    request_profile: &'a [u8],
    bundle_lifecycle: &'a [u8],
    transition: &'a [u8],
    effect: &'a [u8],
    strategy: &'a [u8],
    semantic_source: ContentId,
    compiler_source: ContentId,
    toolchain: ContentId,
    certificate: ContentId,
    bundle_digest: ContentId,
    kind: ContentId,
    config_schema: ContentId,
    request_schema: ContentId,
    root_schema: ContentId,
    derivation_policy: ContentId,
    capacity_profile: ContentId,
    root_state_bytes: u32,
}

/// Borrowed exact generated sections ready for a checked source emitter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesShadowGeneratedBundleV1<'a> {
    /// Exact CapabilityProgramV4 bytes.
    pub capability_program: &'a [u8],
    /// Exact Profile13 bytes.
    pub account_profile: &'a [u8],
    /// Exact RequestProfile bytes.
    pub request_profile: &'a [u8],
    /// Exact selected LifecycleV5 bytes.
    pub lifecycle: &'a [u8],
    /// Exact TransitionVM bytes.
    pub transition: &'a [u8],
    /// Exact DCE5 Effect bytes.
    pub effect: &'a [u8],
    /// Exact Shadow-AOT strategy bytes.
    pub strategy: &'a [u8],
    /// Exact translation-certificate content identity.
    pub certificate: ContentId,
}

impl<'a> SeriesShadowSourceManifestV1<'a> {
    /// Hostile-decode one exact bounded manifest with no trailing bytes.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() > SERIES_SHADOW_SOURCE_MANIFEST_MAX_BYTES_V1
            || bytes.len() < SECTIONS_OFFSET
            || bytes.get(..8) != Some(SERIES_SHADOW_SOURCE_MANIFEST_MAGIC_V1.as_slice())
            || read_u16(bytes, VERSION_OFFSET)? != SERIES_SHADOW_SOURCE_MANIFEST_VERSION_V1
            || read_u16(bytes, PROFILE_OFFSET)? != SERIES_SHADOW_SOURCE_MANIFEST_PROFILE_V1
            || usize::try_from(read_u32(bytes, TOTAL_BYTES_OFFSET)?)
                .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?
                != bytes.len()
            || usize::from(read_u16(bytes, FIXED_RULE_COUNT_OFFSET)?)
                != SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4
            || read_u16(bytes, HEADER_RESERVED_OFFSET)? != 0
            || bytes
                .get(TAIL_RESERVED_OFFSET..TAIL_RESERVED_OFFSET + TAIL_RESERVED_BYTES)
                .is_none_or(|reserved| reserved.iter().any(|byte| *byte != 0))
        {
            return Err(SeriesShadowBundleCompileErrorV4::Manifest);
        }
        let lengths = [
            read_len(bytes, SOURCE_LIFECYCLE_BYTES_OFFSET)?,
            read_len(bytes, CAPABILITY_BYTES_OFFSET)?,
            read_len(bytes, ACCOUNT_BYTES_OFFSET)?,
            read_len(bytes, REQUEST_BYTES_OFFSET)?,
            read_len(bytes, BUNDLE_LIFECYCLE_BYTES_OFFSET)?,
            read_len(bytes, TRANSITION_BYTES_OFFSET)?,
            read_len(bytes, EFFECT_BYTES_OFFSET)?,
            read_len(bytes, STRATEGY_BYTES_OFFSET)?,
        ];
        if lengths.first() != lengths.get(4)
            || lengths.get(1).copied()
                != Some(dclutch_capability_program_contract::v4::CAPABILITY_PROGRAM_V4_BYTES)
            || lengths.get(2).copied()
                != Some(
                    dclutch_trading_sbf::series::account_profile_v4::SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4,
                )
            || lengths.get(3).copied() != Some(SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4)
            || lengths.get(5).copied() != Some(SERIES_CONSUME_TRANSITION_BYTES_V4)
            || lengths.get(6).copied() != Some(SERIES_CONSUME_EFFECT_BYTES_V4)
            || lengths.get(7).copied()
                != Some(
                    dclutch_execution_strategy_contract::v2::EXECUTION_STRATEGY_PROGRAM_BYTES_V2,
                )
        {
            return Err(SeriesShadowBundleCompileErrorV4::Manifest);
        }
        let expected = lengths.iter().try_fold(SECTIONS_OFFSET, |total, len| {
            total
                .checked_add(*len)
                .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)
        })?;
        if expected != bytes.len() {
            return Err(SeriesShadowBundleCompileErrorV4::Manifest);
        }
        let mut identities = [[0_u8; 32]; IDENTITY_COUNT];
        for (index, output) in identities.iter_mut().enumerate() {
            let start = IDENTITIES_OFFSET
                .checked_add(
                    index
                        .checked_mul(32)
                        .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)?,
                )
                .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)?;
            *output = array_32(bytes, start)?;
        }
        child_requests(bytes)?;
        let mut cursor = SECTIONS_OFFSET;
        let source_lifecycle = take(bytes, &mut cursor, lengths[0])?;
        let capability_program = take(bytes, &mut cursor, lengths[1])?;
        let account_profile = take(bytes, &mut cursor, lengths[2])?;
        let request_profile = take(bytes, &mut cursor, lengths[3])?;
        let bundle_lifecycle = take(bytes, &mut cursor, lengths[4])?;
        let transition = take(bytes, &mut cursor, lengths[5])?;
        let effect = take(bytes, &mut cursor, lengths[6])?;
        let strategy = take(bytes, &mut cursor, lengths[7])?;
        if cursor != bytes.len() || source_lifecycle != bundle_lifecycle {
            return Err(SeriesShadowBundleCompileErrorV4::Manifest);
        }
        let semantic_source = decoded_identity(&identities, SEMANTIC_SOURCE_IDENTITY)?;
        let compiler_source = decoded_identity(&identities, COMPILER_SOURCE_IDENTITY)?;
        let toolchain = decoded_identity(&identities, TOOLCHAIN_IDENTITY)?;
        let certificate = decoded_identity(&identities, CERTIFICATE_IDENTITY)?;
        let expected_bundle_digest = decoded_identity(&identities, BUNDLE_IDENTITY)?;
        let kind = decoded_identity(&identities, KIND_IDENTITY)?;
        let config_schema = decoded_identity(&identities, CONFIG_SCHEMA_IDENTITY)?;
        let request_schema = decoded_identity(&identities, REQUEST_SCHEMA_IDENTITY)?;
        let root_schema = decoded_identity(&identities, ROOT_SCHEMA_IDENTITY)?;
        let derivation_policy = decoded_identity(&identities, DERIVATION_POLICY_IDENTITY)?;
        let capacity_profile = decoded_identity(&identities, CAPACITY_PROFILE_IDENTITY)?;
        let root_state_bytes = read_u32(bytes, ROOT_STATE_BYTES_OFFSET)?;
        authenticate_generated_sections(
            GeneratedSectionsV1 {
                capability_program,
                account_profile,
                request_profile,
                lifecycle: bundle_lifecycle,
                transition,
                effect,
                strategy,
            },
            SeriesShadowDescriptorSemanticsV4 {
                kind,
                config_schema,
                request_schema,
                root_schema,
                derivation_policy,
                capacity_profile,
                root_state_bytes,
            },
            certificate,
        )?;
        if bundle_digest(
            capability_program,
            account_profile,
            request_profile,
            bundle_lifecycle,
            transition,
            effect,
            strategy,
            [semantic_source, compiler_source, toolchain, certificate],
        )? != expected_bundle_digest
        {
            return Err(SeriesShadowBundleCompileErrorV4::Manifest);
        }
        Ok(Self {
            bytes,
            source_lifecycle,
            capability_program,
            account_profile,
            request_profile,
            bundle_lifecycle,
            transition,
            effect,
            strategy,
            semantic_source,
            compiler_source,
            toolchain,
            certificate,
            bundle_digest: expected_bundle_digest,
            kind,
            config_schema,
            request_schema,
            root_schema,
            derivation_policy,
            capacity_profile,
            root_state_bytes,
        })
    }

    /// Exact complete manifest bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Domain-separated generated bundle digest.
    pub const fn bundle_digest(self) -> ContentId {
        self.bundle_digest
    }

    /// Reviewed semantic source identity.
    pub const fn semantic_source(self) -> ContentId {
        self.semantic_source
    }

    /// Exact compiler source identity.
    pub const fn compiler_source(self) -> ContentId {
        self.compiler_source
    }

    /// Exact toolchain manifest identity.
    pub const fn toolchain(self) -> ContentId {
        self.toolchain
    }

    /// Exact generated sections authenticated by this manifest.
    pub const fn generated_bundle(self) -> SeriesShadowGeneratedBundleV1<'a> {
        SeriesShadowGeneratedBundleV1 {
            capability_program: self.capability_program,
            account_profile: self.account_profile,
            request_profile: self.request_profile,
            lifecycle: self.bundle_lifecycle,
            transition: self.transition,
            effect: self.effect,
            strategy: self.strategy,
            certificate: self.certificate,
        }
    }
}

/// Compile and encode one exact bounded source manifest.
pub fn compile_series_shadow_source_manifest_v1(
    source: SeriesShadowBundleSourceV4<'_>,
) -> Result<Vec<u8>> {
    let compiled = compile_series_shadow_bundle_v4(source)?;
    encode_manifest(source, &compiled)
}

/// Rebuild one manifest and require byte-for-byte identity.
pub fn require_deterministic_series_shadow_rebuild_v1(
    manifest_bytes: &[u8],
    sources: SeriesShadowRebuildSourcesV1<'_>,
) -> Result<()> {
    let manifest = SeriesShadowSourceManifestV1::decode(manifest_bytes)?;
    if content(sources.semantic_source)? != manifest.semantic_source()
        || content(sources.compiler_source)? != manifest.compiler_source()
        || content(sources.toolchain_manifest)? != manifest.toolchain()
    {
        return Err(SeriesShadowBundleCompileErrorV4::SourceIdentity);
    }
    let mut fixed_data_lengths = [0_u32; SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4];
    for (index, output) in fixed_data_lengths.iter_mut().enumerate() {
        let offset = FIXED_RULES_OFFSET
            .checked_add(
                index
                    .checked_mul(4)
                    .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)?,
            )
            .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)?;
        *output = read_u32(manifest.bytes, offset)?;
    }
    let child = child_requests(manifest.bytes)?;
    let source = SeriesShadowBundleSourceV4 {
        descriptor: SeriesShadowDescriptorSemanticsV4 {
            kind: manifest.kind,
            config_schema: manifest.config_schema,
            request_schema: manifest.request_schema,
            root_schema: manifest.root_schema,
            derivation_policy: manifest.derivation_policy,
            capacity_profile: manifest.capacity_profile,
            root_state_bytes: manifest.root_state_bytes,
        },
        release_sources: SeriesShadowReleaseSourcesV4 {
            semantic_source: sources.semantic_source,
            compiler_source: sources.compiler_source,
            toolchain_manifest: sources.toolchain_manifest,
            certificate: manifest.certificate,
        },
        lifecycle: manifest.source_lifecycle,
        fixed_data_lengths: &fixed_data_lengths,
        child_requests: child,
    };
    let rebuilt = compile_series_shadow_source_manifest_v1(source)?;
    if rebuilt != manifest_bytes {
        return Err(SeriesShadowBundleCompileErrorV4::RebuildMismatch);
    }
    Ok(())
}

fn encode_manifest(
    source: SeriesShadowBundleSourceV4<'_>,
    compiled: &CompiledSeriesShadowBundleV4,
) -> Result<Vec<u8>> {
    let section_lengths = [
        source.lifecycle.len(),
        compiled.capability_program.len(),
        compiled.account_profile.len(),
        compiled.request_profile.len(),
        compiled.lifecycle.len(),
        compiled.transition.len(),
        compiled.effect.len(),
        compiled.strategy.len(),
    ];
    let total = section_lengths
        .iter()
        .try_fold(SECTIONS_OFFSET, |total, len| {
            total
                .checked_add(*len)
                .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)
        })?;
    if total > SERIES_SHADOW_SOURCE_MANIFEST_MAX_BYTES_V1 {
        return Err(SeriesShadowBundleCompileErrorV4::Manifest);
    }
    let mut output = vec![0_u8; total];
    put(&mut output, 0, &SERIES_SHADOW_SOURCE_MANIFEST_MAGIC_V1)?;
    put_u16(
        &mut output,
        VERSION_OFFSET,
        SERIES_SHADOW_SOURCE_MANIFEST_VERSION_V1,
    )?;
    put_u16(
        &mut output,
        PROFILE_OFFSET,
        SERIES_SHADOW_SOURCE_MANIFEST_PROFILE_V1,
    )?;
    put_u32(
        &mut output,
        TOTAL_BYTES_OFFSET,
        u32::try_from(total).map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?,
    )?;
    for (offset, len) in [
        SOURCE_LIFECYCLE_BYTES_OFFSET,
        CAPABILITY_BYTES_OFFSET,
        ACCOUNT_BYTES_OFFSET,
        REQUEST_BYTES_OFFSET,
        BUNDLE_LIFECYCLE_BYTES_OFFSET,
        TRANSITION_BYTES_OFFSET,
        EFFECT_BYTES_OFFSET,
        STRATEGY_BYTES_OFFSET,
    ]
    .into_iter()
    .zip(section_lengths)
    {
        put_u32(
            &mut output,
            offset,
            u32::try_from(len).map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?,
        )?;
    }
    put_u16(
        &mut output,
        FIXED_RULE_COUNT_OFFSET,
        u16::try_from(SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4)
            .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?,
    )?;
    put_u32(
        &mut output,
        ROOT_STATE_BYTES_OFFSET,
        source.descriptor.root_state_bytes,
    )?;
    let identities = [
        compiled.semantic_source,
        compiled.compiler_source,
        compiled.toolchain,
        compiled.certificate,
        compiled.bundle_digest,
        source.descriptor.kind,
        source.descriptor.config_schema,
        source.descriptor.request_schema,
        source.descriptor.root_schema,
        source.descriptor.derivation_policy,
        source.descriptor.capacity_profile,
    ];
    for (index, identity) in identities.iter().enumerate() {
        put(
            &mut output,
            IDENTITIES_OFFSET
                .checked_add(
                    index
                        .checked_mul(32)
                        .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)?,
                )
                .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)?,
            &identity.to_bytes(),
        )?;
    }
    for (index, length) in source.fixed_data_lengths.iter().enumerate() {
        put_u32(
            &mut output,
            FIXED_RULES_OFFSET
                .checked_add(
                    index
                        .checked_mul(4)
                        .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)?,
                )
                .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)?,
            *length,
        )?;
    }
    let mut cursor = CHILD_REQUESTS_OFFSET;
    for bytes in [
        source.child_requests.lock.as_slice(),
        source.child_requests.core.as_slice(),
        source.child_requests.realize.as_slice(),
        source.child_requests.claims.as_slice(),
        source.child_requests.core.as_slice(),
        source.lifecycle,
        compiled.capability_program.as_slice(),
        compiled.account_profile.as_slice(),
        compiled.request_profile.as_slice(),
        compiled.lifecycle.as_slice(),
        compiled.transition.as_slice(),
        compiled.effect.as_slice(),
        compiled.strategy.as_slice(),
    ] {
        put(&mut output, cursor, bytes)?;
        cursor = cursor
            .checked_add(bytes.len())
            .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)?;
    }
    if cursor != output.len() {
        return Err(SeriesShadowBundleCompileErrorV4::Manifest);
    }
    SeriesShadowSourceManifestV1::decode(&output)?;
    Ok(output)
}

fn child_requests(bytes: &[u8]) -> Result<SeriesConsumeChildRequestsV4<'_>> {
    let mut cursor = CHILD_REQUESTS_OFFSET;
    let lock: &[u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3] = take(
        bytes,
        &mut cursor,
        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
    )?
    .try_into()
    .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?;
    let core: &[u8; SERIES_CONSUME_CORE_REQUEST_BYTES_V3] =
        take(bytes, &mut cursor, SERIES_CONSUME_CORE_REQUEST_BYTES_V3)?
            .try_into()
            .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?;
    let realize: &[u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3] = take(
        bytes,
        &mut cursor,
        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
    )?
    .try_into()
    .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?;
    let claims: &[u8; SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3] =
        take(bytes, &mut cursor, SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3)?
            .try_into()
            .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?;
    let repeated_core = take(bytes, &mut cursor, SERIES_CONSUME_CORE_REQUEST_BYTES_V3)?;
    if repeated_core != core.as_slice() || cursor != SECTIONS_OFFSET {
        return Err(SeriesShadowBundleCompileErrorV4::Manifest);
    }
    Ok(SeriesConsumeChildRequestsV4 {
        lock,
        core,
        realize,
        claims,
    })
}

#[derive(Clone, Copy)]
struct GeneratedSectionsV1<'a> {
    capability_program: &'a [u8],
    account_profile: &'a [u8],
    request_profile: &'a [u8],
    lifecycle: &'a [u8],
    transition: &'a [u8],
    effect: &'a [u8],
    strategy: &'a [u8],
}

fn authenticate_generated_sections(
    sections: GeneratedSectionsV1<'_>,
    semantics: SeriesShadowDescriptorSemanticsV4,
    certificate: ContentId,
) -> Result<()> {
    let descriptor = CapabilityProgramV4::decode(sections.capability_program)
        .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?;
    let account = AccountProfileV2::decode(sections.account_profile)
        .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?;
    RequestProfileV1::decode(sections.request_profile)
        .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?;
    let lifecycle_id = content(sections.lifecycle)?;
    StateLifecyclePolicyV5::decode_selected(
        lifecycle_id.to_bytes(),
        lifecycle_id.to_bytes(),
        sections.lifecycle,
    )
    .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?
    .validate_account_profile(account)
    .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?;
    TransitionProgramV3::decode(sections.transition)
        .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?;
    EffectProgramV4::decode(sections.effect)
        .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?;
    let strategy = ExecutionStrategyProgramV2::decode(sections.strategy)
        .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)?;
    if descriptor.kind() != semantics.kind
        || descriptor.config_schema() != semantics.config_schema
        || descriptor.request_schema() != semantics.request_schema
        || descriptor.root_schema() != semantics.root_schema
        || descriptor.derivation_policy() != semantics.derivation_policy
        || descriptor.capacity_profile() != semantics.capacity_profile
        || descriptor.root_state_bytes() != semantics.root_state_bytes
        || strategy.disposition() != StrategyDispositionV2::ShadowAot
        || strategy.certificate_program() != Some(certificate)
        || strategy.request_schema() != id(SHADOW_REQUEST_SCHEMA_ID_V3)?
        || strategy.ack_schema() != id(SHADOW_ACK_SCHEMA_ID_V3)?
    {
        return Err(SeriesShadowBundleCompileErrorV4::Manifest);
    }
    descriptor
        .validate_artifacts(CapabilityArtifactsV4 {
            account_profile: reference(
                ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V2,
                content(sections.account_profile)?,
            )?,
            request_profile: reference(REQUEST_SCHEMA_ID, content(sections.request_profile)?)?,
            lifecycle: reference(
                dclutch_account_profile_contract::lifecycle_v3::CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5,
                lifecycle_id,
            )?,
            strategy: reference(
                EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                content(sections.strategy)?,
            )?,
            transition: reference(TRANSITION_SCHEMA_ID, content(sections.transition)?)?,
            effect: reference(SCHEMA_RELEASE_ID_V4, content(sections.effect)?)?,
        })
        .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)
}

fn decoded_identity(identities: &[[u8; 32]; IDENTITY_COUNT], index: usize) -> Result<ContentId> {
    let bytes = identities
        .get(index)
        .copied()
        .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)?;
    ContentId::new(bytes).map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)
}

fn read_len(bytes: &[u8], offset: usize) -> Result<usize> {
    usize::try_from(read_u32(bytes, offset)?)
        .map_err(|_| SeriesShadowBundleCompileErrorV4::Manifest)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)
}

fn array_32(bytes: &[u8], offset: usize) -> Result<[u8; 32]> {
    bytes
        .get(offset..offset + 32)
        .and_then(|value| value.try_into().ok())
        .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(offset..offset + value.len())
        .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)?
        .copy_from_slice(value);
    Ok(())
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) -> Result<()> {
    put(output, offset, &value.to_le_bytes())
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) -> Result<()> {
    put(output, offset, &value.to_le_bytes())
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, len: usize) -> Result<&'a [u8]> {
    let end = cursor
        .checked_add(len)
        .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)?;
    let output = bytes
        .get(*cursor..end)
        .ok_or(SeriesShadowBundleCompileErrorV4::Manifest)?;
    *cursor = end;
    Ok(output)
}

#[cfg(test)]
mod tests;
