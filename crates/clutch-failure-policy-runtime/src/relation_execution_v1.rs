// SPDX-License-Identifier: AGPL-3.0-or-later
//! Canonical Source/Product relation execution for Failure recovery.
//!
//! This module owns the deterministic classification between one Source-owned
//! successful evaluation and one immutable Product V2 partition. It does not
//! own source ingestion, Product artifacts, a payout, a recovery transition,
//! an SBF instruction, an account tag, or account rent. The executed relation
//! capability is intentionally ephemeral and non-decodable: an account adapter
//! must construct it from authenticated Source/Product inputs and Failure must
//! consume it in the same atomic instruction.
//!
//! Source V3 currently exposes only successful integer intervals. Therefore
//! `AmbiguousDenominator` and `NoAcceptedCoverage` remain canonical stable
//! refusal codes but are unreachable from a well-formed V3 success; those
//! source conditions arrive through Source's own refused-result path. A later
//! successful statistic schema may make them executable only under a new
//! relation semantic version.

use clutch_product_series::{
    CompiledOrdinalV2, ContentId as ProductContentId, MarketGenesisProfileV2, MarketInstanceV2Id,
    NativeClaimBasisV1, PriceMeasurePolicyV1, ProductTemplateV4, QuantizedEdgePolicyV1,
    RegistryCapabilityProjectionV2,
};
use clutch_source_plane_v3::{
    ContentId as SourceContentId, StatisticKeyV3, StatisticKindV3, StatisticResultStatusV3,
};
use clutch_source_plane_v3_runtime::{
    RuntimeKey, SourcePolicyHandoffJoinV1, SuccessfulEvaluationHandoffV1,
};
use sha2::{Digest, Sha256};

use crate::{FailurePolicyBindingId, FailurePolicyBindingV1, RelationRefusalV1};

const POLICY_MAGIC: [u8; 8] = *b"DCFRELP1";
const RECORD_MAGIC: [u8; 8] = *b"DCFRELR1";
const POLICY_SCHEMA_V1: u16 = 1;
const RELATION_SEMANTICS_V1: u16 = 1;
const POLICY_ID_DOMAIN: &[u8] = b"dragons-clutch/failure-relation-policy/v1";
const RECORD_ID_DOMAIN: &[u8] = b"dragons-clutch/failure-relation-record/v1";
const EXECUTED_RELATION_DOMAIN: &[u8] = b"dragons-clutch/executed-failure-relation/v1";

/// Exact fixed width of one immutable Failure relation policy body.
pub const FAILURE_RELATION_POLICY_V1_BYTES: usize = 128;
/// Exact fixed width of one canonical Failure relation execution record.
pub const FAILURE_RELATION_RECORD_V1_BYTES: usize = 384;

macro_rules! relation_id {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Construct from exact digest bytes without claiming authenticity.
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Return the exact digest bytes.
            pub const fn bytes(self) -> [u8; 32] {
                self.0
            }

            /// Return this identity through the Product content-ID boundary.
            pub const fn content_id(self) -> ProductContentId {
                ProductContentId::from_bytes(self.0)
            }
        }
    };
}

relation_id!(
    FailureRelationPolicyIdV1,
    "Typed content identity of one immutable Failure relation policy."
);
relation_id!(
    FailureRelationRecordIdV1,
    "Typed content identity of one canonical relation execution record."
);
relation_id!(
    ExecutedFailureRelationIdV1,
    "Typed identity of one atomically executed Source/Product relation."
);

/// Fail-closed refusal from policy decoding or semantic execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureRelationExecutionErrorV1 {
    /// A fixed-layout body did not have its one exact canonical width.
    WrongLength,
    /// A fixed-layout body used another discriminator.
    BadMagic,
    /// A fixed-layout body used another schema or semantic version.
    BadVersion,
    /// Reserved bytes were nonzero.
    NonCanonicalReserved,
    /// An enum or disposition code was outside its closed set.
    InvalidEnum,
    /// A required content, program, account, or receipt identity was zero.
    ZeroIdentity,
    /// Exact Source, Product, Failure, policy, or generation bindings disagreed.
    BindingMismatch,
    /// A Product semantic owner refused the supplied artifact graph.
    Product(clutch_product_series::Error),
    /// Source semantic content could not be re-identified.
    Source(clutch_source_plane_v3_runtime::Error),
}

impl From<clutch_product_series::Error> for FailureRelationExecutionErrorV1 {
    fn from(value: clutch_product_series::Error) -> Self {
        Self::Product(value)
    }
}

impl From<clutch_source_plane_v3_runtime::Error> for FailureRelationExecutionErrorV1 {
    fn from(value: clutch_source_plane_v3_runtime::Error) -> Self {
        Self::Source(value)
    }
}

/// Result alias for the canonical Failure relation owner.
pub type FailureRelationResultV1<T> = core::result::Result<T, FailureRelationExecutionErrorV1>;

/// Immutable reviewed release and complete selector semantics for relation V1.
///
/// The body identity is the exact `relation_policy_id` which Product Genesis,
/// the registry capability profile, and `FailurePolicyBindingV1` must all
/// name. The schema assigns refuse-on-ambiguity semantics to the stored
/// ambiguity selector; no caller can swap a midpoint or endpoint policy into
/// one execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureRelationPolicyV1 {
    executor_program_id: RuntimeKey,
    executor_release_id: ProductContentId,
    registry_release_id: ProductContentId,
    statistic_registry_value: u16,
    ambiguity_policy_registry_value: u8,
    edge_policy_registry_value: u8,
    resolved_edge_policy: QuantizedEdgePolicyV1,
}

impl FailureRelationPolicyV1 {
    /// Construct one immutable relation policy for an authenticated registry mapping.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        executor_program_id: RuntimeKey,
        executor_release_id: ProductContentId,
        registry_release_id: ProductContentId,
        statistic_registry_value: u16,
        ambiguity_policy_registry_value: u8,
        edge_policy_registry_value: u8,
        resolved_edge_policy: QuantizedEdgePolicyV1,
    ) -> FailureRelationResultV1<Self> {
        let value = Self {
            executor_program_id,
            executor_release_id,
            registry_release_id,
            statistic_registry_value,
            ambiguity_policy_registry_value,
            edge_policy_registry_value,
            resolved_edge_policy,
        };
        value.validate()?;
        Ok(value)
    }

    /// Runtime program that must execute the atomic relation adapter.
    pub const fn executor_program_id(self) -> RuntimeKey {
        self.executor_program_id
    }

    /// Exact reviewed executor deployment/release identity.
    pub const fn executor_release_id(self) -> ProductContentId {
        self.executor_release_id
    }

    /// Exact authenticated registry release defining selector meanings.
    pub const fn registry_release_id(self) -> ProductContentId {
        self.registry_release_id
    }

    /// Exact Source/Product statistic selector.
    pub const fn statistic_registry_value(self) -> u16 {
        self.statistic_registry_value
    }

    /// Registry selector whose V1 meaning is refuse-on-ambiguity.
    pub const fn ambiguity_policy_registry_value(self) -> u8 {
        self.ambiguity_policy_registry_value
    }

    /// Exact registry-owned edge selector.
    pub const fn edge_policy_registry_value(self) -> u8 {
        self.edge_policy_registry_value
    }

    /// Exact registry-resolved edge behavior.
    pub const fn resolved_edge_policy(self) -> QuantizedEdgePolicyV1 {
        self.resolved_edge_policy
    }

    /// Canonical fixed-width bytes of this policy.
    pub fn encode(self) -> FailureRelationResultV1<[u8; FAILURE_RELATION_POLICY_V1_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; FAILURE_RELATION_POLICY_V1_BYTES];
        output[..8].copy_from_slice(&POLICY_MAGIC);
        output[8..10].copy_from_slice(&POLICY_SCHEMA_V1.to_le_bytes());
        output[10..12].copy_from_slice(&RELATION_SEMANTICS_V1.to_le_bytes());
        output[12..44].copy_from_slice(&self.executor_program_id.bytes());
        output[44..76].copy_from_slice(&self.executor_release_id.bytes());
        output[76..108].copy_from_slice(&self.registry_release_id.bytes());
        output[108..110].copy_from_slice(&self.statistic_registry_value.to_le_bytes());
        output[110] = self.ambiguity_policy_registry_value;
        output[111] = self.edge_policy_registry_value;
        output[112] = edge_policy_code(self.resolved_edge_policy);
        Ok(output)
    }

    /// Decode and validate one hostile fixed-width policy body.
    pub fn decode(input: &[u8]) -> FailureRelationResultV1<Self> {
        if input.len() != FAILURE_RELATION_POLICY_V1_BYTES {
            return Err(FailureRelationExecutionErrorV1::WrongLength);
        }
        if input[..8] != POLICY_MAGIC {
            return Err(FailureRelationExecutionErrorV1::BadMagic);
        }
        if read_u16(input, 8) != POLICY_SCHEMA_V1 || read_u16(input, 10) != RELATION_SEMANTICS_V1 {
            return Err(FailureRelationExecutionErrorV1::BadVersion);
        }
        if input[113..].iter().any(|byte| *byte != 0) {
            return Err(FailureRelationExecutionErrorV1::NonCanonicalReserved);
        }
        let value = Self {
            executor_program_id: RuntimeKey::from_bytes(read_id(input, 12)),
            executor_release_id: ProductContentId::from_bytes(read_id(input, 44)),
            registry_release_id: ProductContentId::from_bytes(read_id(input, 76)),
            statistic_registry_value: read_u16(input, 108),
            ambiguity_policy_registry_value: input[110],
            edge_policy_registry_value: input[111],
            resolved_edge_policy: decode_edge_policy(input[112])?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Typed content identity of the complete canonical policy bytes.
    pub fn id(self) -> FailureRelationResultV1<FailureRelationPolicyIdV1> {
        Ok(FailureRelationPolicyIdV1::from_bytes(domain_hash(
            POLICY_ID_DOMAIN,
            &self.encode()?,
        )))
    }

    fn validate(self) -> FailureRelationResultV1<()> {
        require_live(self.executor_program_id.bytes())?;
        require_live(self.executor_release_id.bytes())?;
        require_live(self.registry_release_id.bytes())?;
        if !matches!(self.statistic_registry_value, 1 | 2)
            || self.ambiguity_policy_registry_value == 0
            || self.edge_policy_registry_value == 0
        {
            return Err(FailureRelationExecutionErrorV1::InvalidEnum);
        }
        Ok(())
    }
}

/// Exhaustive deterministic disposition of one successful Source evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureRelationDispositionV1 {
    /// The evidence selects one admissible Product value region.
    Accepted,
    /// The immutable relation selected no value for one exact closed reason.
    Refused(RelationRefusalV1),
}

impl FailureRelationDispositionV1 {
    /// Stable encoded disposition code: zero for accepted, one through five for refusal.
    pub const fn code(self) -> u8 {
        match self {
            Self::Accepted => 0,
            Self::Refused(RelationRefusalV1::AmbiguousInterval) => 1,
            Self::Refused(RelationRefusalV1::AmbiguousDenominator) => 2,
            Self::Refused(RelationRefusalV1::ValueOutOfRange) => 3,
            Self::Refused(RelationRefusalV1::NonPointEvidence) => 4,
            Self::Refused(RelationRefusalV1::NoAcceptedCoverage) => 5,
        }
    }

    /// Stable refusal code, or zero for an accepted classification.
    pub const fn refusal_code(self) -> u32 {
        match self {
            Self::Accepted => 0,
            Self::Refused(refusal) => refusal.code(),
        }
    }

    fn decode(value: u8) -> FailureRelationResultV1<Self> {
        Ok(match value {
            0 => Self::Accepted,
            1 => Self::Refused(RelationRefusalV1::AmbiguousInterval),
            2 => Self::Refused(RelationRefusalV1::AmbiguousDenominator),
            3 => Self::Refused(RelationRefusalV1::ValueOutOfRange),
            4 => Self::Refused(RelationRefusalV1::NonPointEvidence),
            5 => Self::Refused(RelationRefusalV1::NoAcceptedCoverage),
            _ => return Err(FailureRelationExecutionErrorV1::InvalidEnum),
        })
    }
}

/// Canonical auditable record emitted by the relation executor.
///
/// Fields are private. The only constructor executes the complete Source and
/// Product join, so callers cannot attach a chosen refusal code to otherwise
/// authenticated evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureRelationRecordV1 {
    binding_id: FailurePolicyBindingId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    relation_policy_id: FailureRelationPolicyIdV1,
    relation_executor_release_id: ProductContentId,
    source_policy_handoff_authentication_id: SourceContentId,
    source_success_handoff_id: SourceContentId,
    statistic_result_id: SourceContentId,
    statistic_result_authentication_id: SourceContentId,
    source_release_authentication_id: SourceContentId,
    source_work_receipt_authentication_id: SourceContentId,
    disposition: FailureRelationDispositionV1,
}

impl FailureRelationRecordV1 {
    /// Exact Failure policy binding.
    pub const fn binding_id(self) -> FailurePolicyBindingId {
        self.binding_id
    }

    /// Exact Product V2 market occurrence.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact shared Failure/Source liveness generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Immutable relation policy identity selected by Product and Failure.
    pub const fn relation_policy_id(self) -> FailureRelationPolicyIdV1 {
        self.relation_policy_id
    }

    /// Exact reviewed relation executor release.
    pub const fn relation_executor_release_id(self) -> ProductContentId {
        self.relation_executor_release_id
    }

    /// Complete Source release/account/result/work join authentication identity.
    pub const fn source_policy_handoff_authentication_id(self) -> SourceContentId {
        self.source_policy_handoff_authentication_id
    }

    /// Exact Source-owned successful semantic handoff.
    pub const fn source_success_handoff_id(self) -> SourceContentId {
        self.source_success_handoff_id
    }

    /// Content identity of the exact successful StatisticResult.
    pub const fn statistic_result_id(self) -> SourceContentId {
        self.statistic_result_id
    }

    /// Exact result-account owner/PDA/full-body authentication identity.
    pub const fn statistic_result_authentication_id(self) -> SourceContentId {
        self.statistic_result_authentication_id
    }

    /// Exact admitted Source release authentication identity.
    pub const fn source_release_authentication_id(self) -> SourceContentId {
        self.source_release_authentication_id
    }

    /// Exact persisted Source work-receipt authentication identity.
    pub const fn source_work_receipt_authentication_id(self) -> SourceContentId {
        self.source_work_receipt_authentication_id
    }

    /// Deterministic accepted/refused classification.
    pub const fn disposition(self) -> FailureRelationDispositionV1 {
        self.disposition
    }

    /// Stable closed refusal code, or zero when accepted.
    pub const fn refusal_code(self) -> u32 {
        self.disposition.refusal_code()
    }

    /// Canonical fixed-width execution bytes.
    pub fn encode(self) -> FailureRelationResultV1<[u8; FAILURE_RELATION_RECORD_V1_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; FAILURE_RELATION_RECORD_V1_BYTES];
        output[..8].copy_from_slice(&RECORD_MAGIC);
        output[8..10].copy_from_slice(&POLICY_SCHEMA_V1.to_le_bytes());
        output[10..12].copy_from_slice(&RELATION_SEMANTICS_V1.to_le_bytes());
        output[12] = self.disposition.code();
        output[16..48].copy_from_slice(&self.binding_id.bytes());
        output[48..80].copy_from_slice(&self.market_instance_id.bytes());
        output[80..88].copy_from_slice(&self.generation.to_le_bytes());
        output[88..120].copy_from_slice(&self.relation_policy_id.bytes());
        output[120..152].copy_from_slice(&self.relation_executor_release_id.bytes());
        output[152..184].copy_from_slice(&self.source_policy_handoff_authentication_id.bytes());
        output[184..216].copy_from_slice(&self.source_success_handoff_id.bytes());
        output[216..248].copy_from_slice(&self.statistic_result_id.bytes());
        output[248..280].copy_from_slice(&self.statistic_result_authentication_id.bytes());
        output[280..312].copy_from_slice(&self.source_release_authentication_id.bytes());
        output[312..344].copy_from_slice(&self.source_work_receipt_authentication_id.bytes());
        Ok(output)
    }

    /// Decode one hostile record body without claiming account authenticity.
    pub fn decode(input: &[u8]) -> FailureRelationResultV1<Self> {
        if input.len() != FAILURE_RELATION_RECORD_V1_BYTES {
            return Err(FailureRelationExecutionErrorV1::WrongLength);
        }
        if input[..8] != RECORD_MAGIC {
            return Err(FailureRelationExecutionErrorV1::BadMagic);
        }
        if read_u16(input, 8) != POLICY_SCHEMA_V1 || read_u16(input, 10) != RELATION_SEMANTICS_V1 {
            return Err(FailureRelationExecutionErrorV1::BadVersion);
        }
        if input[13..16].iter().any(|byte| *byte != 0) || input[344..].iter().any(|byte| *byte != 0)
        {
            return Err(FailureRelationExecutionErrorV1::NonCanonicalReserved);
        }
        let value = Self {
            binding_id: FailurePolicyBindingId::from_bytes(read_id(input, 16)),
            market_instance_id: MarketInstanceV2Id::from_bytes(read_id(input, 48)),
            generation: read_u64(input, 80),
            relation_policy_id: FailureRelationPolicyIdV1::from_bytes(read_id(input, 88)),
            relation_executor_release_id: ProductContentId::from_bytes(read_id(input, 120)),
            source_policy_handoff_authentication_id: SourceContentId::from_bytes(read_id(
                input, 152,
            )),
            source_success_handoff_id: SourceContentId::from_bytes(read_id(input, 184)),
            statistic_result_id: SourceContentId::from_bytes(read_id(input, 216)),
            statistic_result_authentication_id: SourceContentId::from_bytes(read_id(input, 248)),
            source_release_authentication_id: SourceContentId::from_bytes(read_id(input, 280)),
            source_work_receipt_authentication_id: SourceContentId::from_bytes(read_id(input, 312)),
            disposition: FailureRelationDispositionV1::decode(input[12])?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Typed content identity of the complete canonical relation record.
    pub fn id(self) -> FailureRelationResultV1<FailureRelationRecordIdV1> {
        Ok(FailureRelationRecordIdV1::from_bytes(domain_hash(
            RECORD_ID_DOMAIN,
            &self.encode()?,
        )))
    }

    fn validate(self) -> FailureRelationResultV1<()> {
        for id in [
            self.binding_id.bytes(),
            self.market_instance_id.bytes(),
            self.relation_policy_id.bytes(),
            self.relation_executor_release_id.bytes(),
            self.source_policy_handoff_authentication_id.bytes(),
            self.source_success_handoff_id.bytes(),
            self.statistic_result_id.bytes(),
            self.statistic_result_authentication_id.bytes(),
            self.source_release_authentication_id.bytes(),
            self.source_work_receipt_authentication_id.bytes(),
        ] {
            require_live(id)?;
        }
        if self.generation == 0 {
            return Err(FailureRelationExecutionErrorV1::ZeroIdentity);
        }
        Ok(())
    }
}

/// Non-decodable capability proving one canonical relation was executed.
///
/// A decoded [`FailureRelationRecordV1`] is an untrusted projection and cannot
/// construct this type. The concrete account adapter must authenticate every
/// input passed to [`execute_failure_relation_v1`] and consume this capability
/// in the same atomic Failure transition; no relation account or rent is
/// required.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutedFailureRelationV1 {
    record: FailureRelationRecordV1,
    record_id: FailureRelationRecordIdV1,
    execution_id: ExecutedFailureRelationIdV1,
}

impl ExecutedFailureRelationV1 {
    /// Complete atomic execution identity.
    pub const fn id(self) -> ExecutedFailureRelationIdV1 {
        self.execution_id
    }

    /// Canonical record identity committed by Failure.
    pub const fn record_id(self) -> FailureRelationRecordIdV1 {
        self.record_id
    }

    /// Exact Failure policy binding.
    pub const fn binding_id(self) -> FailurePolicyBindingId {
        self.record.binding_id
    }

    /// Exact Product V2 occurrence.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.record.market_instance_id
    }

    /// Exact shared Failure/Source generation.
    pub const fn generation(self) -> u64 {
        self.record.generation
    }

    /// Exact Source-owned successful handoff.
    pub const fn source_success_handoff_id(self) -> SourceContentId {
        self.record.source_success_handoff_id
    }

    /// Complete Source physical handoff authentication.
    pub const fn source_policy_handoff_authentication_id(self) -> SourceContentId {
        self.record.source_policy_handoff_authentication_id
    }

    /// Exact successful StatisticResult content identity.
    pub const fn statistic_result_id(self) -> SourceContentId {
        self.record.statistic_result_id
    }

    /// Exact result-account owner/PDA/full-body authentication identity.
    pub const fn statistic_result_authentication_id(self) -> SourceContentId {
        self.record.statistic_result_authentication_id
    }

    /// Exact admitted Source release authentication identity.
    pub const fn source_release_authentication_id(self) -> SourceContentId {
        self.record.source_release_authentication_id
    }

    /// Exact persisted Source work-receipt authentication identity.
    pub const fn source_work_receipt_authentication_id(self) -> SourceContentId {
        self.record.source_work_receipt_authentication_id
    }

    /// Immutable relation policy selected by Product and Failure.
    pub const fn relation_policy_id(self) -> FailureRelationPolicyIdV1 {
        self.record.relation_policy_id
    }

    /// Exact reviewed relation executor release.
    pub const fn relation_executor_release_id(self) -> ProductContentId {
        self.record.relation_executor_release_id
    }

    /// Deterministic accepted/refused classification.
    pub const fn disposition(self) -> FailureRelationDispositionV1 {
        self.record.disposition
    }

    /// Stable closed refusal code, or zero when accepted.
    pub const fn refusal_code(self) -> u32 {
        self.record.disposition.refusal_code()
    }
}

/// Execute the canonical relation over one account-authenticated Source success.
///
/// The Product adapter must supply bodies from its private authenticated
/// artifact join and a capability projection from the authenticated registry
/// release. This function re-identifies and cross-checks every body which can
/// change the answer before classifying the exact successful result.
#[allow(clippy::too_many_arguments)]
pub fn execute_failure_relation_v1(
    policy: &FailureRelationPolicyV1,
    binding: FailurePolicyBindingV1,
    source_join: SourcePolicyHandoffJoinV1,
    success: SuccessfulEvaluationHandoffV1,
    compiled: &CompiledOrdinalV2,
    template: &ProductTemplateV4,
    basis: &NativeClaimBasisV1,
    price_policy: &PriceMeasurePolicyV1,
    genesis: &MarketGenesisProfileV2,
    statistic_key: &StatisticKeyV3,
    registry: &RegistryCapabilityProjectionV2,
) -> FailureRelationResultV1<ExecutedFailureRelationV1> {
    let policy_id = policy.id()?;
    validate_source_join(binding, source_join, success, statistic_key)?;
    validate_product_join(
        policy,
        policy_id,
        binding,
        compiled,
        template,
        basis,
        price_policy,
        genesis,
        statistic_key,
        registry,
    )?;
    let result = success.result();
    if result.status() != StatisticResultStatusV3::Success || result.refusal_code() != 0 {
        return Err(FailureRelationExecutionErrorV1::BindingMismatch);
    }
    let (low, high) = match statistic_key.statistic {
        StatisticKindV3::TerminalInterval => result
            .terminal_interval()
            .map_err(|_| FailureRelationExecutionErrorV1::BindingMismatch)?,
        StatisticKindV3::MaximumDrawdownInterval => {
            let interval = result
                .drawdown_interval()
                .map_err(|_| FailureRelationExecutionErrorV1::BindingMismatch)?;
            (u128::from(interval.low_ppm), u128::from(interval.high_ppm))
        }
    };
    let disposition = classify_interval(
        basis,
        price_policy,
        genesis,
        policy.resolved_edge_policy,
        low,
        high,
    )?;
    let statistic_result_id = success.statistic_result_id()?;
    let record = FailureRelationRecordV1 {
        binding_id: binding.id(),
        market_instance_id: binding.market_instance_id(),
        generation: binding.generation(),
        relation_policy_id: policy_id,
        relation_executor_release_id: policy.executor_release_id,
        source_policy_handoff_authentication_id: source_join.id(),
        source_success_handoff_id: success.id(),
        statistic_result_id,
        statistic_result_authentication_id: source_join.source_fact_authentication_id(),
        source_release_authentication_id: source_join.release_authentication_id(),
        source_work_receipt_authentication_id: source_join.work_receipt_authentication_id(),
        disposition,
    };
    let record_id = record.id()?;
    let mut bytes = [0_u8; 64];
    bytes[..32].copy_from_slice(&record_id.bytes());
    bytes[32..].copy_from_slice(&source_join.id().bytes());
    Ok(ExecutedFailureRelationV1 {
        record,
        record_id,
        execution_id: ExecutedFailureRelationIdV1::from_bytes(domain_hash(
            EXECUTED_RELATION_DOMAIN,
            &bytes,
        )),
    })
}

fn validate_source_join(
    binding: FailurePolicyBindingV1,
    source: SourcePolicyHandoffJoinV1,
    success: SuccessfulEvaluationHandoffV1,
    statistic_key: &StatisticKeyV3,
) -> FailureRelationResultV1<()> {
    let binding_id = binding.id();
    let occurrence = success.occurrence();
    let result_id = success.statistic_result_id()?;
    let statistic_key_id = statistic_key
        .id()
        .map_err(|_| FailureRelationExecutionErrorV1::BindingMismatch)?;
    if source.id().is_zero()
        || source.release_authentication_id().is_zero()
        || source.source_fact_authentication_id().is_zero()
        || source.work_receipt_authentication_id().is_zero()
        || source.handoff_id() != success.id()
        || source.failure_policy_binding_id().bytes() != binding_id.bytes()
        || success.failure_policy_binding_id().bytes() != binding_id.bytes()
        || source.generation() != binding.generation()
        || source.source_spec_id() != binding.source_spec_id()
        || source.window_id() != success.window_id()
        || source.statistic_key_id() != statistic_key_id
        || source.clock_policy_id() != binding.clock_policy_id()
        || source.clock() != success.clock()
        || source.occurrence_account() != occurrence.occurrence_account()
        || source.source_fact_authentication_id() != success.result_account_authentication_id()
        || result_id
            != success
                .result()
                .id()
                .map_err(|_| FailureRelationExecutionErrorV1::BindingMismatch)?
        || occurrence.series_plan_id().bytes() != binding.series_plan_id().bytes()
        || occurrence.ordinal() != binding.ordinal()
        || occurrence.market_instance_id().bytes() != binding.market_instance_id().bytes()
        || occurrence.id() != binding.source_occurrence_receipt_id()
        || occurrence.source_plane_contract_id() != binding.source_plane_program_id()
        || occurrence.source_spec_id() != binding.source_spec_id()
        || occurrence.window_id() != success.window_id()
        || occurrence.statistic_key_id() != statistic_key_id
        || occurrence.clock_policy_id() != binding.clock_policy_id()
        || success.clock_policy_id() != binding.clock_policy_id()
        || success.result().statistic_key_id() != statistic_key_id
    {
        return Err(FailureRelationExecutionErrorV1::BindingMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_product_join(
    policy: &FailureRelationPolicyV1,
    policy_id: FailureRelationPolicyIdV1,
    binding: FailurePolicyBindingV1,
    compiled: &CompiledOrdinalV2,
    template: &ProductTemplateV4,
    basis: &NativeClaimBasisV1,
    price_policy: &PriceMeasurePolicyV1,
    genesis: &MarketGenesisProfileV2,
    statistic_key: &StatisticKeyV3,
    registry: &RegistryCapabilityProjectionV2,
) -> FailureRelationResultV1<()> {
    let market = &compiled.market;
    market.validate_bindings(template, basis, price_policy, genesis)?;
    genesis.validate_partition_bindings(basis, price_policy, policy.resolved_edge_policy)?;
    statistic_key
        .validate()
        .map_err(|_| FailureRelationExecutionErrorV1::BindingMismatch)?;
    let market_id = market.id()?;
    let template_id = template.id()?;
    let basis_id = basis.id()?;
    let price_policy_id = price_policy.id()?;
    let genesis_id = genesis.id()?;
    let relation_content_id = policy_id.content_id();
    let owners = registry.semantic_owners;
    if compiled.series_plan_id != binding.series_plan_id()
        || compiled.ordinal != binding.ordinal()
        || compiled.market_instance_id != binding.market_instance_id()
        || market_id != compiled.market_instance_id
        || template_id != binding.product_template_id()
        || market.product_template_id != template_id
        || market.market_genesis_profile_id != genesis_id
        || genesis.relation_policy_id != relation_content_id
        || binding.relation_policy_id() != policy_id.bytes()
        || template.native_claim_basis_id != basis_id
        || template.source_plane_contract_id.bytes() != binding.source_plane_program_id().bytes()
        || template.source_spec_id.bytes() != binding.source_spec_id().bytes()
        || template.summary_program_id.bytes() != binding.summary_program_id().bytes()
        || template.statistic_registry_value != policy.statistic_registry_value
        || statistic_key.summary_program_id.bytes() != binding.summary_program_id().bytes()
        || statistic_kind_code(statistic_key.statistic) != policy.statistic_registry_value
        || registry.registry_release_id != policy.registry_release_id
        || registry.capability_profile_id != genesis.capability_profile_id
        || registry.statistic_registry_value != policy.statistic_registry_value
        || registry.coverage_policy_registry_value != template.coverage_policy_registry_value
        || registry.ambiguity_policy_registry_value != policy.ambiguity_policy_registry_value
        || registry.ambiguity_policy_registry_value != basis.ambiguity_policy_registry_value
        || registry.edge_policy_registry_value != policy.edge_policy_registry_value
        || registry.edge_policy_registry_value != basis.edge_policy_registry_value
        || registry.resolved_edge_policy != policy.resolved_edge_policy
        || owners.source_plane_contract_id != template.source_plane_contract_id
        || owners.source_spec_id != template.source_spec_id
        || owners.summary_program_id != template.summary_program_id
        || owners.native_claim_basis_id != basis_id
        || owners.evidence_only_recovery_policy_id != binding.recovery_policy_id()
        || owners.product_compiler_release_id != template.compiler_release_id
        || owners.price_measure_policy_id != price_policy_id
        || owners.relation_policy_id != relation_content_id
        || genesis.price_measure_policy_id != price_policy_id
    {
        return Err(FailureRelationExecutionErrorV1::BindingMismatch);
    }
    Ok(())
}

fn classify_interval(
    basis: &NativeClaimBasisV1,
    price_policy: &PriceMeasurePolicyV1,
    genesis: &MarketGenesisProfileV2,
    edge_policy: QuantizedEdgePolicyV1,
    low: u128,
    high: u128,
) -> FailureRelationResultV1<FailureRelationDispositionV1> {
    if low > high {
        return Err(FailureRelationExecutionErrorV1::BindingMismatch);
    }
    if basis.basis_degree == 0 {
        let (low, high) = apply_domain_edge(
            low,
            high,
            genesis.coordinate_domain_min,
            genesis.coordinate_domain_max,
            edge_policy,
        );
        let (low, high) = match (low, high) {
            (Some(low), Some(high)) => (low, high),
            _ => {
                return Ok(FailureRelationDispositionV1::Refused(
                    RelationRefusalV1::ValueOutOfRange,
                ))
            }
        };
        if !degree_zero_interval_selects_one_payout(basis, low, high) {
            return Ok(FailureRelationDispositionV1::Refused(
                RelationRefusalV1::AmbiguousInterval,
            ));
        }
        return Ok(FailureRelationDispositionV1::Accepted);
    }

    let first = basis.knots[0];
    let last = basis.knots[usize::from(basis.knot_count) - 1];
    let (low, high) = apply_domain_edge(low, high, first, last, edge_policy);
    let (low, high) = match (low, high) {
        (Some(low), Some(high)) => (low, high),
        _ => {
            return Ok(FailureRelationDispositionV1::Refused(
                RelationRefusalV1::ValueOutOfRange,
            ))
        }
    };
    if basis.basis_degree >= 2 && low != high {
        return Ok(FailureRelationDispositionV1::Refused(
            RelationRefusalV1::NonPointEvidence,
        ));
    }
    let spec = price_policy.project_smooth_basis(basis, genesis, edge_policy)?;
    let low_weights = spec.evaluate(low).map_err(|_| {
        FailureRelationExecutionErrorV1::Product(
            clutch_product_series::Error::UnsupportedCapability,
        )
    })?;
    let high_weights = spec.evaluate(high).map_err(|_| {
        FailureRelationExecutionErrorV1::Product(
            clutch_product_series::Error::UnsupportedCapability,
        )
    })?;
    if low_weights.weights != high_weights.weights {
        return Ok(FailureRelationDispositionV1::Refused(
            RelationRefusalV1::AmbiguousInterval,
        ));
    }
    Ok(FailureRelationDispositionV1::Accepted)
}

fn degree_zero_cell(basis: &NativeClaimBasisV1, value: u128) -> usize {
    let mut index = 0_usize;
    let active = usize::from(basis.knot_count);
    while index < active {
        if value < basis.knots[index] {
            return index;
        }
        index += 1;
    }
    usize::from(basis.outcome_count - 1)
}

fn degree_zero_interval_selects_one_payout(
    basis: &NativeClaimBasisV1,
    low: u128,
    high: u128,
) -> bool {
    let first = degree_zero_cell(basis, low);
    let last = degree_zero_cell(basis, high);
    let expected = basis.payout_map[first];
    let mut cell = first;
    loop {
        if basis.payout_map[cell] != expected {
            return false;
        }
        if cell == last {
            break;
        }
        cell += 1;
    }
    true
}

fn apply_domain_edge(
    low: u128,
    high: u128,
    minimum: u128,
    maximum: u128,
    edge: QuantizedEdgePolicyV1,
) -> (Option<u128>, Option<u128>) {
    match edge {
        QuantizedEdgePolicyV1::Clamp => (
            Some(low.clamp(minimum, maximum)),
            Some(high.clamp(minimum, maximum)),
        ),
        QuantizedEdgePolicyV1::Refuse => {
            if low < minimum || high > maximum {
                (None, None)
            } else {
                (Some(low), Some(high))
            }
        }
    }
}

fn edge_policy_code(value: QuantizedEdgePolicyV1) -> u8 {
    match value {
        QuantizedEdgePolicyV1::Clamp => 1,
        QuantizedEdgePolicyV1::Refuse => 2,
    }
}

fn statistic_kind_code(value: StatisticKindV3) -> u16 {
    match value {
        StatisticKindV3::TerminalInterval => 1,
        StatisticKindV3::MaximumDrawdownInterval => 2,
    }
}

fn decode_edge_policy(value: u8) -> FailureRelationResultV1<QuantizedEdgePolicyV1> {
    match value {
        1 => Ok(QuantizedEdgePolicyV1::Clamp),
        2 => Ok(QuantizedEdgePolicyV1::Refuse),
        _ => Err(FailureRelationExecutionErrorV1::InvalidEnum),
    }
}

fn require_live(value: [u8; 32]) -> FailureRelationResultV1<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(FailureRelationExecutionErrorV1::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    hasher.finalize().into()
}

fn read_id(input: &[u8], offset: usize) -> [u8; 32] {
    let mut value = [0_u8; 32];
    value.copy_from_slice(&input[offset..offset + 32]);
    value
}

fn read_u16(input: &[u8], offset: usize) -> u16 {
    let mut value = [0_u8; 2];
    value.copy_from_slice(&input[offset..offset + 2]);
    u16::from_le_bytes(value)
}

fn read_u64(input: &[u8], offset: usize) -> u64 {
    let mut value = [0_u8; 8];
    value.copy_from_slice(&input[offset..offset + 8]);
    u64::from_le_bytes(value)
}
