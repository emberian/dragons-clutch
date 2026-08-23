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
    RealmCollateralProjectionV1, RegistryCapabilityProfileV2Id, RegistryCapabilityProfileV3Id,
    RegistryCapabilityProfileV4Id, RegistryCapabilityProjectionV2, RegistryProgramReleaseV1Id,
    RegistryProgramReleaseV2Id, Result,
};

const PROFILE_MAGIC_V2: [u8; 8] = *b"DCRCAPV2";
const PROFILE_VERSION_V2: u16 = 2;
const PROFILE_MAGIC_V3: [u8; 8] = *b"DCRCAPV3";
const PROFILE_VERSION_V3: u16 = 3;
const PROFILE_MAGIC_V4: [u8; 8] = *b"DCRCAPV4";
const PROFILE_VERSION_V4: u16 = 4;
const RELEASE_MAGIC_V1: [u8; 8] = *b"DCRRELV1";
const RELEASE_VERSION_V1: u16 = 1;
const RELEASE_MAGIC_V2: [u8; 8] = *b"DCRRELV2";
const RELEASE_VERSION_V2: u16 = 2;

/// SHA-256 domain for [`RegistryCapabilityProfileV2`].
pub const REGISTRY_CAPABILITY_PROFILE_V2_DOMAIN: &[u8] =
    b"dragons-clutch/registry-capability-profile/v2";
/// Exact canonical width of [`RegistryCapabilityProfileV2`].
pub const REGISTRY_CAPABILITY_PROFILE_V2_BYTES: usize = 800;
/// SHA-256 domain for [`RegistryCapabilityProfileV3`].
pub const REGISTRY_CAPABILITY_PROFILE_V3_DOMAIN: &[u8] =
    b"dragons-clutch/registry-capability-profile/v3";
/// Exact canonical width of [`RegistryCapabilityProfileV3`].
pub const REGISTRY_CAPABILITY_PROFILE_V3_BYTES: usize = 816;
/// SHA-256 domain for [`RegistryCapabilityProfileV4`].
pub const REGISTRY_CAPABILITY_PROFILE_V4_DOMAIN: &[u8] =
    b"dragons-clutch/registry-capability-profile/v4";
/// Exact canonical width of [`RegistryCapabilityProfileV4`].
pub const REGISTRY_CAPABILITY_PROFILE_V4_BYTES: usize = 816;

/// SHA-256 domain for [`RegistryProgramReleaseV1`].
pub const REGISTRY_PROGRAM_RELEASE_V1_DOMAIN: &[u8] = b"dragons-clutch/registry-program-release/v1";
/// Exact canonical width of [`RegistryProgramReleaseV1`].
pub const REGISTRY_PROGRAM_RELEASE_V1_BYTES: usize = 160;
/// SHA-256 domain for [`RegistryProgramReleaseV2`].
pub const REGISTRY_PROGRAM_RELEASE_V2_DOMAIN: &[u8] = b"dragons-clutch/registry-program-release/v2";
/// Exact canonical width of [`RegistryProgramReleaseV2`].
pub const REGISTRY_PROGRAM_RELEASE_V2_BYTES: usize = 160;

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
        writer.bytes(&RELEASE_MAGIC_V1);
        writer.u16(RELEASE_VERSION_V1);
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
        reader.magic(&RELEASE_MAGIC_V1)?;
        if reader.u16() != RELEASE_VERSION_V1 {
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

/// Disjoint deployment locus for a V2 registry release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RegistryReleaseLocusV2 {
    /// Agave-synthesized ProgramData whose loader slot is exactly zero.
    SynthesizedGenesisZero = 1,
    /// Loader-observed ProgramData whose deployment slot is strictly positive.
    ObservedPositive = 2,
}

impl RegistryReleaseLocusV2 {
    const fn byte(self) -> u8 {
        match self {
            Self::SynthesizedGenesisZero => 1,
            Self::ObservedPositive => 2,
        }
    }

    fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::SynthesizedGenesisZero),
            2 => Ok(Self::ObservedPositive),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Versioned executable release with a disjoint loader-observed coordinate kind.
///
/// Solana programs cannot authenticate the cluster genesis hash, so that truth
/// is deliberately not stored here. The operator's release manifest separately
/// binds genesis/network/workflow identity to this artifact's exact digest.
/// Onchain, slot zero is admitted only under `SynthesizedGenesisZero`; an
/// observed deployment is admitted only under `ObservedPositive` with a
/// positive loader slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryProgramReleaseV2 {
    /// Executing SBF program address.
    pub program: ContentId,
    /// Upgradeable-loader ProgramData linked from the Program account.
    pub programdata: ContentId,
    /// SHA-256 of the complete ProgramData account bytes, including ELF.
    pub programdata_sha256: ContentId,
    /// Canonical compiled capability-manifest identity reviewed for this ELF.
    pub capability_manifest_id: ContentId,
    /// Deployment slot decoded from ProgramData metadata.
    pub deployment_slot: u64,
    /// Disjoint local-synthesized versus observed-public policy.
    pub locus: RegistryReleaseLocusV2,
}

impl RegistryProgramReleaseV2 {
    /// Construct and validate one locus-explicit loader release.
    pub fn new(
        program: ContentId,
        programdata: ContentId,
        programdata_sha256: ContentId,
        capability_manifest_id: ContentId,
        deployment_slot: u64,
        locus: RegistryReleaseLocusV2,
    ) -> Result<Self> {
        let value = Self {
            program,
            programdata,
            programdata_sha256,
            capability_manifest_id,
            deployment_slot,
            locus,
        };
        value.validate()?;
        Ok(value)
    }

    /// Exact release identity derived from loader facts and chain coordinates.
    pub fn id(self) -> Result<RegistryProgramReleaseV2Id> {
        let mut body = [0; REGISTRY_PROGRAM_RELEASE_V2_BYTES];
        self.encode_into(&mut body)?;
        Ok(RegistryProgramReleaseV2Id::from_bytes(
            content_id(REGISTRY_PROGRAM_RELEASE_V2_DOMAIN, &body).bytes(),
        ))
    }

    fn validate(self) -> Result<()> {
        let identities = [
            self.program,
            self.programdata,
            self.programdata_sha256,
            self.capability_manifest_id,
        ];
        for identity in identities {
            identity.validate()?;
        }
        let mut left = 0usize;
        while left < identities.len() {
            let mut right = left + 1;
            while right < identities.len() {
                if identities[left] == identities[right] {
                    return Err(Error::MismatchedArtifact);
                }
                right += 1;
            }
            left += 1;
        }
        match self.locus {
            RegistryReleaseLocusV2::SynthesizedGenesisZero if self.deployment_slot == 0 => Ok(()),
            RegistryReleaseLocusV2::ObservedPositive if self.deployment_slot != 0 => Ok(()),
            _ => Err(Error::InvalidParameter),
        }
    }
}

impl FixedCodec for RegistryProgramReleaseV2 {
    const ENCODED_LEN: usize = REGISTRY_PROGRAM_RELEASE_V2_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&RELEASE_MAGIC_V2);
        writer.u16(RELEASE_VERSION_V2);
        writer.reserved(6);
        for identity in [
            self.program,
            self.programdata,
            self.programdata_sha256,
            self.capability_manifest_id,
        ] {
            writer.id(identity);
        }
        writer.u64(self.deployment_slot);
        writer.u8(self.locus.byte());
        writer.reserved(7);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&RELEASE_MAGIC_V2)?;
        if reader.u16() != RELEASE_VERSION_V2 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            program: reader.id(),
            programdata: reader.id(),
            programdata_sha256: reader.id(),
            capability_manifest_id: reader.id(),
            deployment_slot: reader.u64(),
            locus: RegistryReleaseLocusV2::decode(reader.u8())?,
        };
        reader.reserved(7)?;
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Withdrawn historical 800-byte central-registry capability profile V2.
///
/// This codec remains available only for exact decoding and audit of kind 43.
/// It cannot represent the later interval-work and Recovery-call limits and
/// must never be projected into a new registration.
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
    /// Exact admitted semantic-owner identities.
    pub semantic_owners: CapabilitySemanticOwnersV2,
    /// Exact reviewed evaluator semantics named by `semantic_owners`.
    pub summary_program: SummaryProgramV3,
    /// Exact immutable Realm/Profile collateral projection.
    pub realm_collateral: RealmCollateralProjectionV1,
}

impl RegistryCapabilityProfileV2 {
    /// Historical domain-separated identity of this exact 800-byte body.
    pub fn id(&self) -> Result<RegistryCapabilityProfileV2Id> {
        let mut body = [0; REGISTRY_CAPABILITY_PROFILE_V2_BYTES];
        self.encode_into(&mut body)?;
        Ok(RegistryCapabilityProfileV2Id::from_bytes(
            content_id(REGISTRY_CAPABILITY_PROFILE_V2_DOMAIN, &body).bytes(),
        ))
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
        let projection = RegistryCapabilityProjectionV2 {
            registry_release_id: self.registry_release_id,
            capability_profile_id: ContentId::from_bytes([1; 32]),
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
            maximum_interval_width: 0,
            maximum_coordinates_per_advance: 1,
            maximum_recovery_progress_units_per_call: 1,
            semantic_owners: self.semantic_owners,
            realm_collateral: self.realm_collateral,
        };
        // Fixed dummy successor limits let the shared checker validate every
        // V2-owned field without inventing a V2 admission projection.
        projection.validate_shape()
    }
}

impl FixedCodec for RegistryCapabilityProfileV2 {
    const ENCODED_LEN: usize = REGISTRY_CAPABILITY_PROFILE_V2_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&PROFILE_MAGIC_V2);
        writer.u16(PROFILE_VERSION_V2);
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
        writer.reserved(4);
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
        reader.magic(&PROFILE_MAGIC_V2)?;
        if reader.u16() != PROFILE_VERSION_V2 {
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
        reader.reserved(4)?;
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
            semantic_owners,
            summary_program,
            realm_collateral,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Content-addressed immutable central-registry capability profile V3.
///
/// The body deliberately omits a stored `capability_profile_id`: that value is
/// its own domain-separated content identity and is inserted only by
/// [`Self::projection`]. The stored registry-release ID is authenticated from
/// its separate immutable [`RegistryProgramReleaseV1`] artifact, so neither
/// identity is caller-shaped or self-referential.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryCapabilityProfileV3 {
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
    /// Largest inclusive interval width admitted by Product work.
    pub maximum_interval_width: u64,
    /// Largest interval coordinate count admitted by one Product advance.
    pub maximum_coordinates_per_advance: u16,
    /// Largest Recovery progress delta admitted by one paid call.
    pub maximum_recovery_progress_units_per_call: u64,
    /// Exact admitted semantic-owner identities.
    pub semantic_owners: CapabilitySemanticOwnersV2,
    /// Exact reviewed evaluator semantics named by `semantic_owners`.
    pub summary_program: SummaryProgramV3,
    /// Exact immutable Realm/Profile collateral projection.
    pub realm_collateral: RealmCollateralProjectionV1,
}

impl RegistryCapabilityProfileV3 {
    /// Domain-separated semantic capability-profile identity.
    pub fn id(&self) -> Result<RegistryCapabilityProfileV3Id> {
        let mut body = [0; REGISTRY_CAPABILITY_PROFILE_V3_BYTES];
        self.encode_into(&mut body)?;
        Ok(RegistryCapabilityProfileV3Id::from_bytes(
            content_id(REGISTRY_CAPABILITY_PROFILE_V3_DOMAIN, &body).bytes(),
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
            maximum_interval_width: self.maximum_interval_width,
            maximum_coordinates_per_advance: self.maximum_coordinates_per_advance,
            maximum_recovery_progress_units_per_call: self.maximum_recovery_progress_units_per_call,
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
        self.projection_with_id(ContentId::from_bytes([1; 32]))
            .validate_shape()?;
        Ok(())
    }
}

impl FixedCodec for RegistryCapabilityProfileV3 {
    const ENCODED_LEN: usize = REGISTRY_CAPABILITY_PROFILE_V3_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&PROFILE_MAGIC_V3);
        writer.u16(PROFILE_VERSION_V3);
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
        writer.u16(self.maximum_coordinates_per_advance);
        writer.reserved(2);
        writer.u64(self.maximum_interval_width);
        writer.u64(self.maximum_recovery_progress_units_per_call);
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
        reader.magic(&PROFILE_MAGIC_V3)?;
        if reader.u16() != PROFILE_VERSION_V3 {
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
        let maximum_coordinates_per_advance = reader.u16();
        reader.reserved(2)?;
        let maximum_interval_width = reader.u64();
        let maximum_recovery_progress_units_per_call = reader.u64();
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
            maximum_recovery_progress_units_per_call,
            semantic_owners,
            summary_program,
            realm_collateral,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Current central-registry capability profile bound to ReleaseV2.
///
/// The capability rule bytes remain exactly the reviewed V3 rule set, but the
/// fresh header, domain, typed ID, and artifact coordinate prevent a historical
/// ProfileV3 from being reinterpreted. `rules` is in-memory reuse of the single
/// rule validator, not a second persisted body: this codec only accepts and
/// emits the V4 header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryCapabilityProfileV4 {
    /// Exact reviewed capability rules whose release reference is a ReleaseV2 ID.
    pub rules: RegistryCapabilityProfileV3,
}

impl RegistryCapabilityProfileV4 {
    /// Promote reviewed V3 rule semantics under one exact ReleaseV2 identity.
    ///
    /// This does not authenticate loader state; the SBF adapter performs that
    /// join before it mints a private capability receipt.
    pub fn new(
        rules: RegistryCapabilityProfileV3,
        registry_release_id: RegistryProgramReleaseV2Id,
    ) -> Result<Self> {
        registry_release_id.validate()?;
        if rules.registry_release_id != registry_release_id.content_id() {
            return Err(Error::MismatchedArtifact);
        }
        let value = Self { rules };
        value.validate()?;
        Ok(value)
    }

    /// Domain-separated V4 profile identity.
    pub fn id(&self) -> Result<RegistryCapabilityProfileV4Id> {
        let mut body = [0; REGISTRY_CAPABILITY_PROFILE_V4_BYTES];
        self.encode_into(&mut body)?;
        Ok(RegistryCapabilityProfileV4Id::from_bytes(
            content_id(REGISTRY_CAPABILITY_PROFILE_V4_DOMAIN, &body).bytes(),
        ))
    }

    /// Exact typed ReleaseV2 selected by this profile.
    pub const fn registry_release_id(&self) -> RegistryProgramReleaseV2Id {
        RegistryProgramReleaseV2Id::from_bytes(self.rules.registry_release_id.bytes())
    }

    /// Reconstruct the sole compiler/runtime projection under the V4 profile ID.
    pub fn projection(&self) -> Result<RegistryCapabilityProjectionV2> {
        self.validate()?;
        Ok(self.rules.projection_with_id(self.id()?.content_id()))
    }

    /// Derive interval-consensus bounds under the V4 profile identity.
    pub fn interval_consensus_profile(&self) -> Result<QuantizedIntervalConsensusProfileV1> {
        self.validate()?;
        let profile = QuantizedIntervalConsensusProfileV1 {
            capability_profile_id: self.id()?.content_id(),
            maximum_interval_width: self.rules.maximum_interval_width,
            maximum_coordinates_per_advance: self.rules.maximum_coordinates_per_advance,
        };
        profile.validate()?;
        Ok(profile)
    }

    fn validate(&self) -> Result<()> {
        self.rules.validate()?;
        self.registry_release_id().validate()
    }
}

impl FixedCodec for RegistryCapabilityProfileV4 {
    const ENCODED_LEN: usize = REGISTRY_CAPABILITY_PROFILE_V4_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        self.rules.encode_into(output)?;
        output[..8].copy_from_slice(&PROFILE_MAGIC_V4);
        output[8..10].copy_from_slice(&PROFILE_VERSION_V4.to_le_bytes());
        Ok(())
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&PROFILE_MAGIC_V4)?;
        if reader.u16() != PROFILE_VERSION_V4 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let mut rule_bytes = [0u8; REGISTRY_CAPABILITY_PROFILE_V3_BYTES];
        rule_bytes.copy_from_slice(input);
        rule_bytes[..8].copy_from_slice(&PROFILE_MAGIC_V3);
        rule_bytes[8..10].copy_from_slice(&PROFILE_VERSION_V3.to_le_bytes());
        let value = Self {
            rules: RegistryCapabilityProfileV3::decode(&rule_bytes)?,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn release_v2(slot: u64, locus: RegistryReleaseLocusV2) -> RegistryProgramReleaseV2 {
        RegistryProgramReleaseV2 {
            program: ContentId::from_bytes([1; 32]),
            programdata: ContentId::from_bytes([2; 32]),
            programdata_sha256: ContentId::from_bytes([3; 32]),
            capability_manifest_id: ContentId::from_bytes([4; 32]),
            deployment_slot: slot,
            locus,
        }
    }

    #[test]
    fn withdrawn_v2_and_current_v3_profile_widths_never_cross_decode() {
        assert_eq!(REGISTRY_CAPABILITY_PROFILE_V2_BYTES, 800);
        assert_eq!(REGISTRY_CAPABILITY_PROFILE_V3_BYTES, 816);
        assert_eq!(
            RegistryCapabilityProfileV3::decode(&[0; REGISTRY_CAPABILITY_PROFILE_V2_BYTES]),
            Err(Error::Truncated)
        );
        assert_eq!(
            RegistryCapabilityProfileV2::decode(&[0; REGISTRY_CAPABILITY_PROFILE_V3_BYTES]),
            Err(Error::TrailingBytes)
        );
        assert_ne!(PROFILE_MAGIC_V2, PROFILE_MAGIC_V3);
        assert_ne!(
            REGISTRY_CAPABILITY_PROFILE_V2_DOMAIN,
            REGISTRY_CAPABILITY_PROFILE_V3_DOMAIN
        );
    }

    #[test]
    fn release_v2_locus_is_disjoint_and_v1_never_cross_decodes() {
        let local = release_v2(0, RegistryReleaseLocusV2::SynthesizedGenesisZero);
        let mut local_bytes = [0u8; REGISTRY_PROGRAM_RELEASE_V2_BYTES];
        assert_eq!(local.encode_into(&mut local_bytes), Ok(()));
        assert_eq!(RegistryProgramReleaseV2::decode(&local_bytes), Ok(local));
        assert_eq!(
            RegistryProgramReleaseV1::decode(&local_bytes),
            Err(Error::BadMagic)
        );

        let observed = release_v2(1, RegistryReleaseLocusV2::ObservedPositive);
        let mut observed_bytes = [0u8; REGISTRY_PROGRAM_RELEASE_V2_BYTES];
        assert_eq!(observed.encode_into(&mut observed_bytes), Ok(()));
        assert_eq!(
            RegistryProgramReleaseV2::decode(&observed_bytes),
            Ok(observed)
        );

        assert_eq!(
            release_v2(1, RegistryReleaseLocusV2::SynthesizedGenesisZero)
                .encode_into(&mut local_bytes),
            Err(Error::InvalidParameter)
        );
        assert_eq!(
            release_v2(0, RegistryReleaseLocusV2::ObservedPositive).encode_into(&mut local_bytes),
            Err(Error::InvalidParameter)
        );
    }
}
