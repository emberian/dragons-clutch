//! Authenticated SourcePlane V3 binding and liability-free Series occurrence planning.

use clutch_source_plane_v3::{
    Error as SourcePlaneError, SourcePlaneProgramV3, StatisticKeyV3, StatisticKindV3,
    SummaryProgramV3, WindowSpecV3,
};

use crate::codec::{Reader, Writer};
use crate::{
    compile_ordinal_v2, content_id, ContentId, Error, EvidenceOnlyRecoveryPolicyV1, FixedCodec,
    MarketGenesisProfileV2, MarketInstanceV2Id, NativeClaimBasisV1, PriceMeasurePolicyV1,
    ProductTemplateV4, RegistryCapabilityProjectionV2, Result, SeriesAttachmentPlanId,
    SeriesAttachmentPlanV1, SeriesPlanV5, SeriesPlanV5Id, SourceOccurrenceV1Id,
};

const SOURCE_OCCURRENCE_RECORD_MAGIC: [u8; 8] = *b"DCSOCCV1";
const SCHEMA_V1: u16 = 1;

/// SHA-256 domain for [`CompiledSourceOccurrenceV3`].
pub const SOURCE_OCCURRENCE_RECORD_DOMAIN: &[u8] = b"dragons-clutch/source-occurrence-record/v1";
/// Exact canonical width of [`CompiledSourceOccurrenceV3`].
pub const SOURCE_OCCURRENCE_RECORD_BYTES: usize = 184;

fn local_id(id: clutch_source_plane_v3::ContentId) -> ContentId {
    ContentId::from_bytes(id.bytes())
}

fn source_error(error: SourcePlaneError) -> Error {
    match error {
        SourcePlaneError::Truncated => Error::Truncated,
        SourcePlaneError::TrailingBytes => Error::TrailingBytes,
        SourcePlaneError::BadMagic => Error::BadMagic,
        SourcePlaneError::BadVersion => Error::BadVersion,
        SourcePlaneError::NonCanonicalReserved => Error::NonCanonicalReserved,
        SourcePlaneError::ZeroIdentity => Error::ZeroIdentity,
        SourcePlaneError::NonCanonicalPadding => Error::NonCanonicalPadding,
        SourcePlaneError::ArithmeticOverflow => Error::ArithmeticOverflow,
        SourcePlaneError::MismatchedArtifact => Error::MismatchedArtifact,
        SourcePlaneError::WrongOrdinal => Error::WrongOrdinal,
        SourcePlaneError::InsufficientPrepayment => Error::InsufficientPrepayment,
        SourcePlaneError::InvalidParameter => Error::InvalidParameter,
        SourcePlaneError::DiscontinuousPage
        | SourcePlaneError::IncompleteWindow
        | SourcePlaneError::WindowAlreadyMature
        | SourcePlaneError::NotEligible
        | SourcePlaneError::SeriesExhausted => Error::InvalidParameter,
        SourcePlaneError::UnsupportedStatistic
        | SourcePlaneError::UnsupportedPolicy
        | SourcePlaneError::FailurePayoutNotUniform => Error::UnsupportedCapability,
    }
}

/// Adapter-owned authentication boundary for SourcePlane and registry facts.
///
/// Every method defaults to [`Error::UnauthenticatedAuthority`]. A live adapter
/// must implement this trait on a type that it can construct only after checking
/// account identity, owner, release/body, and the central registry mapping. The
/// pure core deliberately supplies no raw projection constructor and no
/// `bool is_authenticated` escape hatch.
///
/// Rust trait implementation is not cryptographic authentication. The concrete
/// adapter implementing this trait remains an explicitly unverified boundary.
pub trait AuthenticatedSourceSeriesAuthorityV3 {
    /// Authenticate the complete otherwise-forgeable registry projection.
    fn authenticate_registry_projection(
        &self,
        _projection: &RegistryCapabilityProjectionV2,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Return the exact authenticated SourcePlane program contract.
    fn authenticated_source_plane(
        &self,
        _expected_contract_id: ContentId,
    ) -> Result<SourcePlaneProgramV3> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate the existing SourceSpec semantic owner.
    fn authenticate_source_spec(&self, _expected_source_spec_id: ContentId) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Return the exact authenticated source-neutral summary program.
    fn authenticated_summary_program(
        &self,
        _expected_summary_program_id: ContentId,
    ) -> Result<SummaryProgramV3> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Resolve one authenticated registry statistic selector to V3 semantics.
    fn resolve_statistic(
        &self,
        _registry_release_id: ContentId,
        _capability_profile_id: ContentId,
        _statistic_registry_value: u16,
    ) -> Result<StatisticKindV3> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Resolve one authenticated registry coverage selector to the exact
    /// SourcePlane V3 coverage value.
    fn resolve_coverage_policy(
        &self,
        _registry_release_id: ContentId,
        _capability_profile_id: ContentId,
        _coverage_policy_registry_value: u16,
    ) -> Result<u16> {
        Err(Error::UnauthenticatedAuthority)
    }
}

/// Canonical provenance record for one V5 ordinal compiled against SourcePlane V3.
///
/// `market_instance_id` remains the economic identity and excludes Series and
/// ordinal. This record separately binds the requesting Series/ordinal to the
/// canonical SourcePlane WindowKey and StatisticKey. It is a liability-free
/// plan: publication does not create a Market, spend funding, or prove any
/// account exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledSourceOccurrenceV3 {
    /// Immutable finite Series that requested this occurrence.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact ordinal in that Series.
    pub ordinal: u32,
    /// Full-width V2 economic market identity.
    pub market_instance_id: MarketInstanceV2Id,
    /// Operational attachment inherited from the Series.
    pub attachment_plan_id: SeriesAttachmentPlanId,
    /// Canonical SourcePlane V3 WindowKey.
    pub source_window_id: ContentId,
    /// Canonical SourcePlane V3 predictable StatisticKey.
    pub statistic_key_id: ContentId,
}

impl CompiledSourceOccurrenceV3 {
    /// Validate required identities. Semantic derivation is checked by
    /// [`compile_source_occurrence_v3`], not by accepting this decoded body as
    /// authority.
    pub fn validate_shape(&self) -> Result<()> {
        self.series_plan_id.validate()?;
        self.market_instance_id.validate()?;
        self.attachment_plan_id.validate()?;
        self.source_window_id.validate()?;
        self.statistic_key_id.validate()?;
        Ok(())
    }

    /// Typed provenance identity of the exact canonical record.
    pub fn id(&self) -> Result<SourceOccurrenceV1Id> {
        let mut body = [0; SOURCE_OCCURRENCE_RECORD_BYTES];
        self.encode_into(&mut body)?;
        Ok(SourceOccurrenceV1Id::from_bytes(
            content_id(SOURCE_OCCURRENCE_RECORD_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for CompiledSourceOccurrenceV3 {
    const ENCODED_LEN: usize = SOURCE_OCCURRENCE_RECORD_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&SOURCE_OCCURRENCE_RECORD_MAGIC);
        writer.u16(SCHEMA_V1);
        writer.reserved(6);
        writer.id(self.series_plan_id.content_id());
        writer.u32(self.ordinal);
        writer.reserved(4);
        writer.id(self.market_instance_id.content_id());
        writer.id(self.attachment_plan_id.content_id());
        writer.id(self.source_window_id);
        writer.id(self.statistic_key_id);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&SOURCE_OCCURRENCE_RECORD_MAGIC)?;
        if reader.u16() != SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            series_plan_id: SeriesPlanV5Id::from_bytes(reader.id().bytes()),
            ordinal: reader.u32(),
            market_instance_id: {
                reader.reserved(4)?;
                MarketInstanceV2Id::from_bytes(reader.id().bytes())
            },
            attachment_plan_id: SeriesAttachmentPlanId::from_bytes(reader.id().bytes()),
            source_window_id: reader.id(),
            statistic_key_id: reader.id(),
        };
        reader.finish()?;
        value.validate_shape()?;
        Ok(value)
    }
}

/// Compile one V5 ordinal only after the adapter authenticates all source and
/// registry facts required to construct SourcePlane V3 identities.
#[allow(clippy::too_many_arguments)]
pub fn compile_source_occurrence_v3<A: AuthenticatedSourceSeriesAuthorityV3 + ?Sized>(
    authority: &A,
    series: &SeriesPlanV5,
    template: &ProductTemplateV4,
    basis: &NativeClaimBasisV1,
    recovery: &EvidenceOnlyRecoveryPolicyV1,
    price_policy: &PriceMeasurePolicyV1,
    genesis: &MarketGenesisProfileV2,
    attachment: &SeriesAttachmentPlanV1,
    registry: &RegistryCapabilityProjectionV2,
    ordinal: u32,
) -> Result<CompiledSourceOccurrenceV3> {
    authority.authenticate_registry_projection(registry)?;
    authority.authenticate_source_spec(template.source_spec_id)?;

    let source_plane = authority.authenticated_source_plane(template.source_plane_contract_id)?;
    source_plane.validate().map_err(source_error)?;
    let source_plane_id = local_id(source_plane.id().map_err(source_error)?);
    if source_plane_id != template.source_plane_contract_id {
        return Err(Error::MismatchedArtifact);
    }

    let summary = authority.authenticated_summary_program(template.summary_program_id)?;
    summary.validate().map_err(source_error)?;
    if local_id(summary.id().map_err(source_error)?) != template.summary_program_id {
        return Err(Error::MismatchedArtifact);
    }

    let statistic = authority.resolve_statistic(
        registry.registry_release_id,
        registry.capability_profile_id,
        template.statistic_registry_value,
    )?;
    if !summary.supports(statistic) {
        return Err(Error::UnsupportedCapability);
    }
    let coverage_policy_id = authority.resolve_coverage_policy(
        registry.registry_release_id,
        registry.capability_profile_id,
        template.coverage_policy_registry_value,
    )?;

    let compiled = compile_ordinal_v2(
        series,
        template,
        basis,
        recovery,
        price_policy,
        genesis,
        attachment,
        registry,
        ordinal,
    )?;
    let window = WindowSpecV3 {
        source_spec_id: clutch_source_plane_v3::ContentId::from_bytes(
            template.source_spec_id.bytes(),
        ),
        source_plane_program_id: source_plane.id().map_err(source_error)?,
        start_bucket: compiled.schedule.start_bucket,
        end_bucket_exclusive: compiled.schedule.end_bucket_exclusive,
        maturity_bucket_exclusive: compiled.schedule.primary_maturity_bucket_exclusive,
        repair_generation: template.base_repair_generation,
        coverage_policy_id,
        coverage_policy_parameter: template.coverage_policy_parameter,
    };
    window.validate().map_err(source_error)?;
    let statistic_key = StatisticKeyV3 {
        window_id: window.id().map_err(source_error)?,
        summary_program_id: summary.id().map_err(source_error)?,
        statistic,
    };
    statistic_key.validate().map_err(source_error)?;
    let value = CompiledSourceOccurrenceV3 {
        series_plan_id: compiled.series_plan_id,
        ordinal,
        market_instance_id: compiled.market_instance_id,
        attachment_plan_id: compiled.attachment_plan_id,
        source_window_id: local_id(window.id().map_err(source_error)?),
        statistic_key_id: local_id(statistic_key.id().map_err(source_error)?),
    };
    value.validate_shape()?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(seed: u8) -> ContentId {
        ContentId::from_bytes([seed; 32])
    }

    #[test]
    fn occurrence_record_codec_is_exact_and_hostile() {
        let value = CompiledSourceOccurrenceV3 {
            series_plan_id: SeriesPlanV5Id::from_bytes([1; 32]),
            ordinal: 7,
            market_instance_id: MarketInstanceV2Id::from_bytes([2; 32]),
            attachment_plan_id: SeriesAttachmentPlanId::from_bytes([3; 32]),
            source_window_id: id(4),
            statistic_key_id: id(5),
        };
        let mut bytes = [0; SOURCE_OCCURRENCE_RECORD_BYTES];
        value.encode_into(&mut bytes).unwrap();
        assert_eq!(CompiledSourceOccurrenceV3::decode(&bytes), Ok(value));

        let mut bad = bytes;
        bad[10] = 1;
        assert_eq!(
            CompiledSourceOccurrenceV3::decode(&bad),
            Err(Error::NonCanonicalReserved)
        );
        assert_eq!(
            CompiledSourceOccurrenceV3::decode(&bytes[..bytes.len() - 1]),
            Err(Error::Truncated)
        );
        let mut trailing = [0; SOURCE_OCCURRENCE_RECORD_BYTES + 1];
        trailing[..SOURCE_OCCURRENCE_RECORD_BYTES].copy_from_slice(&bytes);
        assert_eq!(
            CompiledSourceOccurrenceV3::decode(&trailing),
            Err(Error::TrailingBytes)
        );
    }

    #[test]
    fn authentication_boundary_defaults_to_refusal() {
        struct NoAuthority;
        impl AuthenticatedSourceSeriesAuthorityV3 for NoAuthority {}

        assert_eq!(
            NoAuthority.authenticate_source_spec(id(9)),
            Err(Error::UnauthenticatedAuthority)
        );
        assert_eq!(
            NoAuthority.resolve_coverage_policy(id(1), id(2), 3),
            Err(Error::UnauthenticatedAuthority)
        );
    }
}
