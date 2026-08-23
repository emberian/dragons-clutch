//! Immutable central-registry capability profile selected by the shared artifact graph.
//!
//! This body is the single semantic owner of the otherwise-ephemeral
//! [`RegistryCapabilityProjectionV2`]. Its content identity is the registry
//! capability-profile identity projected into compilation. The enclosing
//! artifact account supplies program ownership and content-addressed PDA
//! authentication; the SBF adapter additionally authenticates the exact
//! executable/ProgramData release frozen here.

use clutch_bspline::EdgePolicy;
use clutch_source_plane_v3::{
    FixedCodec as SourceFixedCodec, StatisticKindV3, SummaryProgramV3, SUMMARY_PROGRAM_BYTES,
};

use crate::codec::{Reader, Writer};
use crate::{
    content_id, CapabilitySemanticOwnersV2, ContentId, Error, EvidenceOnlyRecoveryPolicyId,
    FixedCodec, NativeClaimBasisId, PriceMeasurePolicyV1Id, QuantizedIntervalConsensusProfileV1,
    RealmCollateralProjectionV1, RegistryCapabilityProfileV2Id, RegistryCapabilityProjectionV2,
    RegistryProgramReleaseV1Id, Result,
};

const PROFILE_MAGIC: [u8; 8] = *b"DCRCAPV2";
const PROFILE_VERSION: u16 = 2;
const RELEASE_MAGIC: [u8; 8] = *b"DCRRELV1";
const RELEASE_VERSION: u16 = 1;

/// SHA-256 domain for [`RegistryCapabilityProfileV2`].
pub const REGISTRY_CAPABILITY_PROFILE_V2_DOMAIN: &[u8] =
    b"dragons-clutch/registry-capability-profile/v2";
/// Exact canonical width of [`RegistryCapabilityProfileV2`].
pub const REGISTRY_CAPABILITY_PROFILE_V2_BYTES: usize = 808;

/// SHA-256 domain for [`RegistryProgramReleaseV1`].
pub const REGISTRY_PROGRAM_RELEASE_V1_DOMAIN: &[u8] = b"dragons-clutch/registry-program-release/v1";
/// Exact canonical width of [`RegistryProgramReleaseV1`].
pub const REGISTRY_PROGRAM_RELEASE_V1_BYTES: usize = 160;

/// Exact executable release associated with one central-registry profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryProgramReleaseV1 {
    /// Executing SBF program address.
    pub program: ContentId,
    /// Upgradeable-loader ProgramData address linked from the program account.
    pub programdata: ContentId,
    /// SHA-256 of the complete ProgramData account bytes, including ELF.
    pub programdata_sha256: ContentId,
    /// Deployment slot decoded from the ProgramData metadata.
    pub deployment_slot: u64,
    /// Canonical linked capability-manifest identity reviewed for this ELF.
    pub capability_manifest_id: ContentId,
}

impl RegistryProgramReleaseV1 {
    /// Exact release identity derived from the complete executable binding.
    pub fn id(self) -> Result<RegistryProgramReleaseV1Id> {
        let mut body = [0; REGISTRY_PROGRAM_RELEASE_V1_BYTES];
        self.encode_into(&mut body)?;
        Ok(RegistryProgramReleaseV1Id::from_bytes(
            content_id(REGISTRY_PROGRAM_RELEASE_V1_DOMAIN, &body).bytes(),
        ))
    }

    fn validate(self) -> Result<()> {
        self.program.validate()?;
        self.programdata.validate()?;
        self.programdata_sha256.validate()?;
        self.capability_manifest_id.validate()?;
        if self.program == self.programdata || self.deployment_slot == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
}

impl FixedCodec for RegistryProgramReleaseV1 {
    const ENCODED_LEN: usize = REGISTRY_PROGRAM_RELEASE_V1_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&RELEASE_MAGIC);
        writer.u16(RELEASE_VERSION);
        writer.reserved(6);
        writer.id(self.program);
        writer.id(self.programdata);
        writer.id(self.programdata_sha256);
        writer.u64(self.deployment_slot);
        writer.id(self.capability_manifest_id);
        writer.reserved(8);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&RELEASE_MAGIC)?;
        if reader.u16() != RELEASE_VERSION {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            program: reader.id(),
            programdata: reader.id(),
            programdata_sha256: reader.id(),
            deployment_slot: reader.u64(),
            capability_manifest_id: reader.id(),
        };
        reader.reserved(8)?;
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Content-addressed immutable central-registry capability profile V2.
///
/// The body deliberately omits a stored `capability_profile_id`: that value is
/// its own domain-separated content identity and is inserted only by
/// [`Self::projection`]. The stored registry-release ID is authenticated from
/// its separate immutable [`RegistryProgramReleaseV1`] artifact, so neither
/// identity is caller-shaped or self-referential.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryCapabilityProfileV2 {
    /// Exact executable central-registry release selected by this profile.
    pub registry_release_id: ContentId,
    /// Exact admitted statistic registry value.
    pub statistic_registry_value: u16,
    /// Registry-resolved Source statistic semantics.
    pub resolved_statistic: StatisticKindV3,
    /// Exact admitted coverage-policy registry value.
    pub coverage_policy_registry_value: u16,
    /// Registry-resolved Source coverage semantics.
    pub resolved_coverage_policy_value: u16,
    /// Exact admitted ambiguity-policy registry value.
    pub ambiguity_policy_registry_value: u8,
    /// Exact admitted edge-policy registry value.
    pub edge_policy_registry_value: u8,
    /// Exact registry-owned terminal BURN disposition value.
    pub burn_terminal_disposition_registry_value: u16,
    /// Registry-resolved edge behavior.
    pub resolved_edge_policy: EdgePolicy,
    /// Whether basis degrees zero through three are executable.
    pub supported_basis_degrees: [bool; 4],
    /// Maximum executable native outcome count.
    pub max_outcome_count: u8,
    /// Maximum executable degree-zero finite payout count.
    pub max_degree_zero_payout_count: u8,
    /// Maximum executable evidence-only recovery attempt count.
    pub max_recovery_attempt_count: u8,
    /// Inclusive minimum coverage-policy parameter.
    pub min_coverage_policy_parameter: u64,
    /// Inclusive maximum coverage-policy parameter.
    pub max_coverage_policy_parameter: u64,
    /// Maximum executable raw observation span.
    pub max_window_span_buckets: u64,
    /// Maximum executable finite Series occurrence count.
    pub max_series_instance_count: u32,
    /// Largest admitted interval width for exact Product consensus work.
    pub maximum_interval_width: u64,
    /// Largest coordinate count evaluated by one paid advance.
    pub maximum_coordinates_per_advance: u16,
    /// Capability switches for `[Dealer, Fractional, Structured]` occurrence families.
    pub enabled_optional_occurrence_families: [bool; 3],
    /// Exact admitted semantic-owner identities.
    pub semantic_owners: CapabilitySemanticOwnersV2,
    /// Exact reviewed evaluator semantics named by `semantic_owners`.
    pub summary_program: SummaryProgramV3,
    /// Exact immutable Realm/Profile collateral projection.
    pub realm_collateral: RealmCollateralProjectionV1,
}

impl RegistryCapabilityProfileV2 {
    /// Domain-separated semantic capability-profile identity.
    pub fn id(&self) -> Result<RegistryCapabilityProfileV2Id> {
        let mut body = [0; REGISTRY_CAPABILITY_PROFILE_V2_BYTES];
        self.encode_into(&mut body)?;
        Ok(RegistryCapabilityProfileV2Id::from_bytes(
            content_id(REGISTRY_CAPABILITY_PROFILE_V2_DOMAIN, &body).bytes(),
        ))
    }

    /// Reconstruct the sole compiler projection owned by this artifact.
    pub fn projection(&self) -> Result<RegistryCapabilityProjectionV2> {
        self.validate()?;
        Ok(self.projection_with_id(self.id()?.content_id()))
    }

    /// Derive the sole interval-consensus work profile from authenticated bounds.
    pub fn interval_consensus_profile(&self) -> Result<QuantizedIntervalConsensusProfileV1> {
        self.validate()?;
        let profile = QuantizedIntervalConsensusProfileV1 {
            capability_profile_id: self.id()?.content_id(),
            maximum_interval_width: self.maximum_interval_width,
            maximum_coordinates_per_advance: self.maximum_coordinates_per_advance,
        };
        profile.validate()?;
        Ok(profile)
    }

    fn projection_with_id(
        &self,
        capability_profile_id: ContentId,
    ) -> RegistryCapabilityProjectionV2 {
        RegistryCapabilityProjectionV2 {
            registry_release_id: self.registry_release_id,
            capability_profile_id,
            statistic_registry_value: self.statistic_registry_value,
            coverage_policy_registry_value: self.coverage_policy_registry_value,
            ambiguity_policy_registry_value: self.ambiguity_policy_registry_value,
            edge_policy_registry_value: self.edge_policy_registry_value,
            burn_terminal_disposition_registry_value: self.burn_terminal_disposition_registry_value,
            resolved_edge_policy: self.resolved_edge_policy,
            supported_basis_degrees: self.supported_basis_degrees,
            max_outcome_count: self.max_outcome_count,
            max_degree_zero_payout_count: self.max_degree_zero_payout_count,
            max_recovery_attempt_count: self.max_recovery_attempt_count,
            min_coverage_policy_parameter: self.min_coverage_policy_parameter,
            max_coverage_policy_parameter: self.max_coverage_policy_parameter,
            max_window_span_buckets: self.max_window_span_buckets,
            max_series_instance_count: self.max_series_instance_count,
            semantic_owners: self.semantic_owners,
            realm_collateral: self.realm_collateral,
        }
    }

    fn validate(&self) -> Result<()> {
        self.registry_release_id.validate()?;
        self.summary_program
            .validate()
            .map_err(|_| Error::MismatchedArtifact)?;
        if self.resolved_coverage_policy_value == 0
            || self.resolved_coverage_policy_value != self.coverage_policy_registry_value
            || !self.summary_program.supports(self.resolved_statistic)
            || self
                .summary_program
                .id()
                .map_err(|_| Error::MismatchedArtifact)?
                .bytes()
                != self.semantic_owners.summary_program_id.bytes()
        {
            return Err(Error::MismatchedArtifact);
        }
        if self.maximum_interval_width == u64::MAX || self.maximum_coordinates_per_advance == 0 {
            return Err(Error::InvalidParameter);
        }
        self.projection_with_id(ContentId::from_bytes([1; 32]))
            .validate_shape()?;
        Ok(())
    }
}

impl FixedCodec for RegistryCapabilityProfileV2 {
    const ENCODED_LEN: usize = REGISTRY_CAPABILITY_PROFILE_V2_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&PROFILE_MAGIC);
        writer.u16(PROFILE_VERSION);
        writer.reserved(6);
        writer.id(self.registry_release_id);
        writer.u16(self.statistic_registry_value);
        writer.u16(self.coverage_policy_registry_value);
        writer.u8(self.ambiguity_policy_registry_value);
        writer.u8(self.edge_policy_registry_value);
        writer.u16(self.burn_terminal_disposition_registry_value);
        writer.u8(match self.resolved_edge_policy {
            EdgePolicy::Clamp => 0,
            EdgePolicy::Refuse => 1,
        });
        let mut degree_bitmap = 0u8;
        let mut degree = 0usize;
        while degree < self.supported_basis_degrees.len() {
            if self.supported_basis_degrees[degree] {
                degree_bitmap |= 1u8 << degree;
            }
            degree += 1;
        }
        writer.u8(degree_bitmap);
        writer.u8(self.max_outcome_count);
        writer.u8(self.max_degree_zero_payout_count);
        writer.u8(self.max_recovery_attempt_count);
        writer.u16(match self.resolved_statistic {
            StatisticKindV3::TerminalInterval => 1,
            StatisticKindV3::MaximumDrawdownInterval => 2,
        });
        writer.u8(0);
        writer.u64(self.min_coverage_policy_parameter);
        writer.u64(self.max_coverage_policy_parameter);
        writer.u64(self.max_window_span_buckets);
        writer.u32(self.max_series_instance_count);
        writer.u64(self.maximum_interval_width);
        writer.u16(self.maximum_coordinates_per_advance);
        let mut optional_bitmap = 0u8;
        let mut optional = 0usize;
        while optional < self.enabled_optional_occurrence_families.len() {
            if self.enabled_optional_occurrence_families[optional] {
                optional_bitmap |= 1u8 << optional;
            }
            optional += 1;
        }
        writer.u8(optional_bitmap);
        writer.reserved(1);
        encode_semantic_owners(&mut writer, self.semantic_owners);
        encode_realm_collateral(&mut writer, self.realm_collateral);
        let mut summary = [0; SUMMARY_PROGRAM_BYTES];
        SourceFixedCodec::encode_into(&self.summary_program, &mut summary)
            .map_err(|_| Error::MismatchedArtifact)?;
        writer.bytes(&summary);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&PROFILE_MAGIC)?;
        if reader.u16() != PROFILE_VERSION {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let registry_release_id = reader.id();
        let statistic_registry_value = reader.u16();
        let coverage_policy_registry_value = reader.u16();
        let ambiguity_policy_registry_value = reader.u8();
        let edge_policy_registry_value = reader.u8();
        let burn_terminal_disposition_registry_value = reader.u16();
        let resolved_edge_policy = match reader.u8() {
            0 => EdgePolicy::Clamp,
            1 => EdgePolicy::Refuse,
            _ => return Err(Error::InvalidParameter),
        };
        let degree_bitmap = reader.u8();
        if degree_bitmap & !0x0f != 0 {
            return Err(Error::NonCanonicalReserved);
        }
        let supported_basis_degrees = [
            degree_bitmap & 1 != 0,
            degree_bitmap & 2 != 0,
            degree_bitmap & 4 != 0,
            degree_bitmap & 8 != 0,
        ];
        let max_outcome_count = reader.u8();
        let max_degree_zero_payout_count = reader.u8();
        let max_recovery_attempt_count = reader.u8();
        let resolved_statistic = match reader.u16() {
            1 => StatisticKindV3::TerminalInterval,
            2 => StatisticKindV3::MaximumDrawdownInterval,
            _ => return Err(Error::UnsupportedCapability),
        };
        reader.reserved(1)?;
        let min_coverage_policy_parameter = reader.u64();
        let max_coverage_policy_parameter = reader.u64();
        let max_window_span_buckets = reader.u64();
        let max_series_instance_count = reader.u32();
        let maximum_interval_width = reader.u64();
        let maximum_coordinates_per_advance = reader.u16();
        let optional_bitmap = reader.u8();
        if optional_bitmap & !0x07 != 0 {
            return Err(Error::NonCanonicalReserved);
        }
        let enabled_optional_occurrence_families = [
            optional_bitmap & 1 != 0,
            optional_bitmap & 2 != 0,
            optional_bitmap & 4 != 0,
        ];
        reader.reserved(1)?;
        let semantic_owners = decode_semantic_owners(&mut reader);
        let realm_collateral = decode_realm_collateral(&mut reader);
        let summary_program = SourceFixedCodec::decode(&reader.bytes::<SUMMARY_PROGRAM_BYTES>())
            .map_err(|_| Error::MismatchedArtifact)?;
        reader.finish()?;
        let value = Self {
            registry_release_id,
            statistic_registry_value,
            resolved_statistic,
            coverage_policy_registry_value,
            resolved_coverage_policy_value: coverage_policy_registry_value,
            ambiguity_policy_registry_value,
            edge_policy_registry_value,
            burn_terminal_disposition_registry_value,
            resolved_edge_policy,
            supported_basis_degrees,
            max_outcome_count,
            max_degree_zero_payout_count,
            max_recovery_attempt_count,
            min_coverage_policy_parameter,
            max_coverage_policy_parameter,
            max_window_span_buckets,
            max_series_instance_count,
            maximum_interval_width,
            maximum_coordinates_per_advance,
            enabled_optional_occurrence_families,
            semantic_owners,
            summary_program,
            realm_collateral,
        };
        value.validate()?;
        Ok(value)
    }
}

fn encode_semantic_owners(writer: &mut Writer<'_>, owners: CapabilitySemanticOwnersV2) {
    for id in [
        owners.source_plane_contract_id,
        owners.source_spec_id,
        owners.summary_program_id,
        owners.native_claim_basis_id.content_id(),
        owners.evidence_only_recovery_policy_id.content_id(),
        owners.product_compiler_release_id,
        owners.price_grid_id,
        owners.price_measure_policy_id.content_id(),
        owners.fee_policy_id,
        owners.relation_policy_id,
        owners.score_policy_id,
        owners.candidate_lifecycle_policy_id,
        owners.candidate_liveness_policy_id,
        owners.retirement_policy_id,
    ] {
        writer.id(id);
    }
}

fn decode_semantic_owners(reader: &mut Reader<'_>) -> CapabilitySemanticOwnersV2 {
    CapabilitySemanticOwnersV2 {
        source_plane_contract_id: reader.id(),
        source_spec_id: reader.id(),
        summary_program_id: reader.id(),
        native_claim_basis_id: NativeClaimBasisId::from_bytes(reader.id().bytes()),
        evidence_only_recovery_policy_id: EvidenceOnlyRecoveryPolicyId::from_bytes(
            reader.id().bytes(),
        ),
        product_compiler_release_id: reader.id(),
        price_grid_id: reader.id(),
        price_measure_policy_id: PriceMeasurePolicyV1Id::from_bytes(reader.id().bytes()),
        fee_policy_id: reader.id(),
        relation_policy_id: reader.id(),
        score_policy_id: reader.id(),
        candidate_lifecycle_policy_id: reader.id(),
        candidate_liveness_policy_id: reader.id(),
        retirement_policy_id: reader.id(),
    }
}

fn encode_realm_collateral(writer: &mut Writer<'_>, realm: RealmCollateralProjectionV1) {
    for id in [
        realm.realm_id,
        realm.profile_id,
        realm.collateral_mint,
        realm.token_program,
        realm.neutral_incinerator,
        realm.neutral_lamport_sink,
    ] {
        writer.id(id);
    }
    writer.u64(realm.market_collateral_cap_ceiling);
}

fn decode_realm_collateral(reader: &mut Reader<'_>) -> RealmCollateralProjectionV1 {
    RealmCollateralProjectionV1 {
        realm_id: reader.id(),
        profile_id: reader.id(),
        collateral_mint: reader.id(),
        token_program: reader.id(),
        neutral_incinerator: reader.id(),
        neutral_lamport_sink: reader.id(),
        market_collateral_cap_ceiling: reader.u64(),
    }
}
