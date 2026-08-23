use crate::codec::{reject_legacy_numeric_fallback, Reader, Writer};
use crate::{
    content_id, ContentId, Error, EvidenceOnlyRecoveryPolicyId, FixedCodec, MarketGenesisProfileId,
    MarketInstanceId, NativeClaimBasisId, ProductTemplateId, Result, SeriesAttachmentPlanId,
    SeriesFundingTermsId, SeriesPlanId,
};

const BASIS_MAGIC: [u8; 8] = *b"DCBASIS1";
const RECOVERY_MAGIC: [u8; 8] = *b"DCRECV1\0";
const TEMPLATE_MAGIC: [u8; 8] = *b"DCTMPLV4";
const GENESIS_MAGIC: [u8; 8] = *b"DCMGPV1\0";
const MARKET_MAGIC: [u8; 8] = *b"DCMKTIN1";
const ATTACHMENT_MAGIC: [u8; 8] = *b"DCATPLN1";
const SERIES_MAGIC: [u8; 8] = *b"DCSERIV4";
const FUNDING_TERMS_MAGIC: [u8; 8] = *b"DCFTERM1";

const SCHEMA_V1: u16 = 1;

/// SHA-256 domain for [`NativeClaimBasisV1`].
pub const NATIVE_CLAIM_BASIS_DOMAIN: &[u8] = b"dragons-clutch/native-claim-basis/v1";
/// SHA-256 domain for [`EvidenceOnlyRecoveryPolicyV1`].
pub const RECOVERY_POLICY_DOMAIN: &[u8] = b"dragons-clutch/evidence-only-recovery-policy/v1";
/// SHA-256 domain for [`ProductTemplateV4`].
pub const PRODUCT_TEMPLATE_DOMAIN: &[u8] = b"dragons-clutch/product-template/v4";
/// SHA-256 domain for [`MarketGenesisProfileV1`].
pub const MARKET_GENESIS_PROFILE_DOMAIN: &[u8] = b"dragons-clutch/market-genesis-profile/v1";
/// SHA-256 domain for [`MarketInstancePreimageV1`].
pub const MARKET_INSTANCE_DOMAIN: &[u8] = b"dragons-clutch/market-instance/v1";
/// SHA-256 domain for [`SeriesAttachmentPlanV1`].
pub const SERIES_ATTACHMENT_PLAN_DOMAIN: &[u8] = b"dragons-clutch/series-attachment-plan/v1";
/// SHA-256 domain for [`SeriesPlanV4`].
pub const SERIES_PLAN_DOMAIN: &[u8] = b"dragons-clutch/series-plan/v4";
/// SHA-256 domain for [`SeriesFundingTermsV1`].
pub const SERIES_FUNDING_TERMS_DOMAIN: &[u8] = b"dragons-clutch/series-funding-terms/v1";

/// Maximum number of native outcomes.
pub const MAX_OUTCOMES: usize = 16;
/// Maximum number of evidence-selected payout vectors.
pub const MAX_PAYOUTS: usize = 16;
/// Maximum B-spline degree frozen by this basis codec.
pub const MAX_BASIS_DEGREE: u8 = 3;
/// Sentinel declaring that active knots are not uniformly spaced.
pub const UNIFORM_SPACING_NONE: u8 = u8::MAX;
/// Sentinel for an inactive degree-zero payout-map entry.
pub const PAYOUT_MAP_UNUSED: u8 = u8::MAX;
/// Maximum finite recovery attempts.
pub const MAX_RECOVERY_ATTEMPTS: usize = 8;
/// Maximum number of instances in one finite Series.
pub const MAX_SERIES_INSTANCES: u32 = 65_536;
/// Maximum primary source window span in buckets.
pub const MAX_SERIES_WINDOW_BUCKETS: u64 = 1_000_000;

/// Exact canonical byte length of [`NativeClaimBasisV1`].
pub const BASIS_BYTES: usize = 2_352;
/// Exact canonical byte length of [`EvidenceOnlyRecoveryPolicyV1`].
pub const EVIDENCE_ONLY_RECOVERY_POLICY_BYTES: usize = 208;
/// Exact canonical byte length of [`ProductTemplateV4`].
pub const PRODUCT_TEMPLATE_BYTES: usize = 256;
/// Exact canonical byte length of [`MarketGenesisProfileV1`].
pub const MARKET_GENESIS_PROFILE_BYTES: usize = 352;
/// Exact canonical byte length of [`MarketInstancePreimageV1`].
pub const MARKET_INSTANCE_PREIMAGE_BYTES: usize = 88;
/// Exact canonical byte length of [`SeriesAttachmentPlanV1`].
pub const SERIES_ATTACHMENT_PLAN_BYTES: usize = 112;
/// Exact canonical byte length of [`SeriesPlanV4`].
pub const SERIES_PLAN_BYTES: usize = 152;
/// Exact canonical byte length of [`SeriesFundingTermsV1`].
pub const SERIES_FUNDING_TERMS_BYTES: usize = 208;

/// One exhaustive native claim basis and its evidence-selected payout family.
///
/// This is the only owner of partition/basis/payout bytes. It deliberately has
/// no failure-policy field, failure-payout index, or privileged payout row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeClaimBasisV1 {
    /// B-spline basis degree, in `0..=3`.
    pub basis_degree: u8,
    /// Active liability count, in `2..=16`.
    pub outcome_count: u8,
    /// Active evidence-selected payout-vector count, in `1..=16`.
    pub payout_count: u8,
    /// Active knot prefix length.
    pub knot_count: u8,
    /// Exact log2 uniform gap or [`UNIFORM_SPACING_NONE`].
    pub uniform_log2_spacing: u8,
    /// Nonzero registry-owned ambiguity semantics.
    pub ambiguity_policy_registry_value: u8,
    /// Nonzero registry-owned edge semantics.
    pub edge_policy_registry_value: u8,
    /// Common positive payout denominator.
    pub denominator: u64,
    /// Active payout rows and outcome columns followed by zero padding.
    pub payout_weights: [[u64; MAX_OUTCOMES]; MAX_PAYOUTS],
    /// Degree-zero cell mapping; unused entries are [`PAYOUT_MAP_UNUSED`].
    pub payout_map: [u8; MAX_OUTCOMES],
    /// Strictly increasing active knots followed by zero padding.
    pub knots: [u128; MAX_OUTCOMES],
}

impl NativeClaimBasisV1 {
    /// Validate simplex rows, basis shape, liveness mapping, and arithmetic bounds.
    pub fn validate(&self) -> Result<()> {
        let max_outcomes = u8::try_from(MAX_OUTCOMES).map_err(|_| Error::InvalidParameter)?;
        if self.basis_degree > MAX_BASIS_DEGREE
            || !(2..=max_outcomes).contains(&self.outcome_count)
            || self.payout_count == 0
            || usize::from(self.payout_count) > MAX_PAYOUTS
            || self.ambiguity_policy_registry_value == 0
            || self.edge_policy_registry_value == 0
            || self.denominator == 0
        {
            return Err(Error::InvalidParameter);
        }

        let outcomes = usize::from(self.outcome_count);
        let payouts = usize::from(self.payout_count);
        let mut row = 0_usize;
        while row < MAX_PAYOUTS {
            let mut sum = 0_u64;
            let mut column = 0_usize;
            while column < MAX_OUTCOMES {
                let weight = self.payout_weights[row][column];
                if row < payouts && column < outcomes {
                    if weight > self.denominator {
                        return Err(Error::InvalidParameter);
                    }
                    sum = sum.checked_add(weight).ok_or(Error::ArithmeticOverflow)?;
                } else if weight != 0 {
                    return Err(Error::NonCanonicalPadding);
                }
                column += 1;
            }
            if row < payouts && sum != self.denominator {
                return Err(Error::InvalidParameter);
            }
            row += 1;
        }

        let expected_knots = match self.basis_degree {
            0 => outcomes - 1,
            1 => outcomes,
            degree => outcomes
                .checked_add(1)
                .and_then(|value| value.checked_sub(usize::from(degree)))
                .ok_or(Error::InvalidParameter)?,
        };
        let knot_count = usize::from(self.knot_count);
        if knot_count != expected_knots
            || knot_count == 0
            || knot_count > MAX_OUTCOMES
            || (self.basis_degree >= 1 && knot_count < 2)
        {
            return Err(Error::InvalidParameter);
        }

        let mut previous = 0_u128;
        let mut largest_gap = 0_u128;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            let knot = self.knots[index];
            if index < knot_count {
                if index == 0 {
                    if self.basis_degree == 0 && knot == 0 {
                        return Err(Error::InvalidParameter);
                    }
                } else {
                    if knot <= previous {
                        return Err(Error::InvalidParameter);
                    }
                    largest_gap = largest_gap.max(knot - previous);
                }
                previous = knot;
            } else if knot != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }

        if self.uniform_log2_spacing == UNIFORM_SPACING_NONE {
            if self.basis_degree >= 2 {
                return Err(Error::InvalidParameter);
            }
        } else {
            if self.uniform_log2_spacing >= 128 {
                return Err(Error::InvalidParameter);
            }
            let gap = 1_u128 << self.uniform_log2_spacing;
            let mut knot = 1_usize;
            while knot < knot_count {
                if self.knots[knot] - self.knots[knot - 1] != gap {
                    return Err(Error::InvalidParameter);
                }
                knot += 1;
            }
        }

        let mut map_index = 0_usize;
        while map_index < MAX_OUTCOMES {
            let value = self.payout_map[map_index];
            if self.basis_degree == 0 && map_index < outcomes {
                if value >= self.payout_count {
                    return Err(Error::InvalidParameter);
                }
            } else if value != PAYOUT_MAP_UNUSED {
                return Err(Error::NonCanonicalPadding);
            }
            map_index += 1;
        }

        if self.basis_degree >= 1 {
            let operand = match self.basis_degree {
                1 => largest_gap.checked_sub(1).ok_or(Error::InvalidParameter)?,
                2 => {
                    let gap = 1_u128 << self.uniform_log2_spacing;
                    gap.checked_mul(gap)
                        .and_then(|value| value.checked_mul(2))
                        .ok_or(Error::ArithmeticOverflow)?
                }
                3 => {
                    let gap = 1_u128 << self.uniform_log2_spacing;
                    gap.checked_mul(gap)
                        .and_then(|value| value.checked_mul(gap))
                        .and_then(|value| value.checked_mul(6))
                        .ok_or(Error::ArithmeticOverflow)?
                }
                _ => return Err(Error::InvalidParameter),
            };
            let bound = u128::from(self.denominator)
                .checked_mul(operand)
                .ok_or(Error::ArithmeticOverflow)?;
            if bound >> 127 != 0 {
                return Err(Error::ArithmeticOverflow);
            }
        }
        Ok(())
    }

    /// Typed identity of these exact canonical bytes.
    pub fn id(&self) -> Result<NativeClaimBasisId> {
        let mut body = [0; BASIS_BYTES];
        self.encode_into(&mut body)?;
        Ok(NativeClaimBasisId::from_bytes(
            content_id(NATIVE_CLAIM_BASIS_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for NativeClaimBasisV1 {
    const ENCODED_LEN: usize = BASIS_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&BASIS_MAGIC);
        writer.u16(SCHEMA_V1);
        writer.u8(self.basis_degree);
        writer.u8(self.outcome_count);
        writer.u8(self.payout_count);
        writer.u8(self.knot_count);
        writer.u8(self.uniform_log2_spacing);
        writer.u8(self.ambiguity_policy_registry_value);
        writer.u8(self.edge_policy_registry_value);
        writer.u8(0);
        writer.reserved(6);
        writer.u64(self.denominator);
        for row in self.payout_weights {
            for weight in row {
                writer.u64(weight);
            }
        }
        writer.bytes(&self.payout_map);
        for knot in self.knots {
            writer.u128(knot);
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        reject_legacy_numeric_fallback(input)?;
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&BASIS_MAGIC)?;
        if reader.u16() != SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        let basis_degree = reader.u8();
        let outcome_count = reader.u8();
        let payout_count = reader.u8();
        let knot_count = reader.u8();
        let uniform_log2_spacing = reader.u8();
        let ambiguity_policy_registry_value = reader.u8();
        let edge_policy_registry_value = reader.u8();
        if reader.u8() != 0 {
            return Err(Error::NonCanonicalReserved);
        }
        reader.reserved(6)?;
        let denominator = reader.u64();
        let mut payout_weights = [[0; MAX_OUTCOMES]; MAX_PAYOUTS];
        let mut row = 0_usize;
        while row < MAX_PAYOUTS {
            let mut column = 0_usize;
            while column < MAX_OUTCOMES {
                payout_weights[row][column] = reader.u64();
                column += 1;
            }
            row += 1;
        }
        let payout_map = reader.bytes();
        let mut knots = [0; MAX_OUTCOMES];
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            knots[index] = reader.u128();
            index += 1;
        }
        reader.finish()?;
        let value = Self {
            basis_degree,
            outcome_count,
            payout_count,
            knot_count,
            uniform_log2_spacing,
            ambiguity_policy_registry_value,
            edge_policy_registry_value,
            denominator,
            payout_weights,
            payout_map,
            knots,
        };
        value.validate()?;
        Ok(value)
    }
}

/// One finite repair attempt relative to the primary evidence maturity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryAttemptV1 {
    /// Source repair-generation increment relative to the Template base.
    pub repair_generation_delta: u32,
    /// First eligible bucket after primary maturity.
    pub opens_after_primary_maturity_buckets: u64,
    /// Exclusive close bucket after primary maturity.
    pub closes_after_primary_maturity_buckets: u64,
}

impl RecoveryAttemptV1 {
    /// Canonical inactive array padding.
    pub const ZERO: Self = Self {
        repair_generation_delta: 0,
        opens_after_primary_maturity_buckets: 0,
        closes_after_primary_maturity_buckets: 0,
    };
}

/// Finite evidence-only repair schedule with no numeric failure payout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceOnlyRecoveryPolicyV1 {
    /// Active ordered attempt count, in `1..=8`.
    pub attempt_count: u8,
    /// Active attempts followed by exact zero padding.
    pub attempts: [RecoveryAttemptV1; MAX_RECOVERY_ATTEMPTS],
}

impl EvidenceOnlyRecoveryPolicyV1 {
    /// Validate a finite, ordered, non-overlapping relative schedule.
    pub fn validate(&self) -> Result<()> {
        let count = usize::from(self.attempt_count);
        if count == 0 || count > MAX_RECOVERY_ATTEMPTS {
            return Err(Error::InvalidSchedule);
        }
        let mut previous_close = 0_u64;
        let mut previous_generation = 0_u32;
        let mut index = 0_usize;
        while index < MAX_RECOVERY_ATTEMPTS {
            let attempt = self.attempts[index];
            if index < count {
                if attempt.opens_after_primary_maturity_buckets
                    >= attempt.closes_after_primary_maturity_buckets
                    || (index > 0
                        && (attempt.opens_after_primary_maturity_buckets < previous_close
                            || attempt.repair_generation_delta < previous_generation))
                {
                    return Err(Error::InvalidSchedule);
                }
                previous_close = attempt.closes_after_primary_maturity_buckets;
                previous_generation = attempt.repair_generation_delta;
            } else if attempt != RecoveryAttemptV1::ZERO {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        Ok(())
    }

    /// Typed identity of this exact evidence-only schedule.
    pub fn id(&self) -> Result<EvidenceOnlyRecoveryPolicyId> {
        let mut body = [0; EVIDENCE_ONLY_RECOVERY_POLICY_BYTES];
        self.encode_into(&mut body)?;
        Ok(EvidenceOnlyRecoveryPolicyId::from_bytes(
            content_id(RECOVERY_POLICY_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for EvidenceOnlyRecoveryPolicyV1 {
    const ENCODED_LEN: usize = EVIDENCE_ONLY_RECOVERY_POLICY_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&RECOVERY_MAGIC);
        writer.u16(SCHEMA_V1);
        writer.u8(self.attempt_count);
        writer.u8(0);
        writer.reserved(4);
        for attempt in self.attempts {
            writer.u32(attempt.repair_generation_delta);
            writer.reserved(4);
            writer.u64(attempt.opens_after_primary_maturity_buckets);
            writer.u64(attempt.closes_after_primary_maturity_buckets);
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&RECOVERY_MAGIC)?;
        if reader.u16() != SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        let attempt_count = reader.u8();
        if reader.u8() != 0 {
            return Err(Error::NonCanonicalReserved);
        }
        reader.reserved(4)?;
        let mut attempts = [RecoveryAttemptV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        let mut index = 0_usize;
        while index < MAX_RECOVERY_ATTEMPTS {
            let repair_generation_delta = reader.u32();
            reader.reserved(4)?;
            attempts[index] = RecoveryAttemptV1 {
                repair_generation_delta,
                opens_after_primary_maturity_buckets: reader.u64(),
                closes_after_primary_maturity_buckets: reader.u64(),
            };
            index += 1;
        }
        reader.finish()?;
        let value = Self {
            attempt_count,
            attempts,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Reusable relative product semantics without Realm, absolute time, or liability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductTemplateV4 {
    /// Exact reviewed recurring SourcePlane release/contract identity.
    pub source_plane_contract_id: ContentId,
    /// Canonical source-description identity.
    pub source_spec_id: ContentId,
    /// Exact source-neutral evaluator release identity.
    pub summary_program_id: ContentId,
    /// Sole native partition/basis/payout artifact identity.
    pub native_claim_basis_id: NativeClaimBasisId,
    /// Exact evidence-only recovery schedule identity.
    pub evidence_only_recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
    /// Exact compiler/relation release identity.
    pub compiler_release_id: ContentId,
    /// Nonzero registry-owned statistic value.
    pub statistic_registry_value: u16,
    /// Nonzero registry-owned coverage-policy value.
    pub coverage_policy_registry_value: u16,
    /// Primary raw observation span in buckets.
    pub window_span_buckets: u64,
    /// Additional buckets before primary evidence maturity.
    pub primary_maturity_grace_buckets: u64,
    /// Original source repair generation.
    pub base_repair_generation: u64,
    /// Exact coverage-policy parameter.
    pub coverage_policy_parameter: u64,
}

impl ProductTemplateV4 {
    /// Validate exact local shape without pretending referenced bytes were supplied.
    pub fn validate_shape(&self) -> Result<()> {
        self.source_plane_contract_id.validate()?;
        self.source_spec_id.validate()?;
        self.summary_program_id.validate()?;
        self.native_claim_basis_id.validate()?;
        self.evidence_only_recovery_policy_id.validate()?;
        self.compiler_release_id.validate()?;
        if self.statistic_registry_value == 0
            || self.coverage_policy_registry_value == 0
            || self.window_span_buckets == 0
            || self.window_span_buckets > MAX_SERIES_WINDOW_BUCKETS
        {
            return Err(Error::InvalidParameter);
        }
        self.window_span_buckets
            .checked_add(self.primary_maturity_grace_buckets)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    /// Join the exact basis and evidence-only policy referenced by this Template.
    pub fn validate_bindings(
        &self,
        basis: &NativeClaimBasisV1,
        recovery: &EvidenceOnlyRecoveryPolicyV1,
    ) -> Result<()> {
        self.validate_shape()?;
        basis.validate()?;
        recovery.validate()?;
        if self.native_claim_basis_id != basis.id()?
            || self.evidence_only_recovery_policy_id != recovery.id()?
        {
            return Err(Error::MismatchedArtifact);
        }
        let last = recovery.attempts[usize::from(recovery.attempt_count) - 1];
        self.base_repair_generation
            .checked_add(u64::from(last.repair_generation_delta))
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    /// Typed identity of these exact reusable semantics.
    pub fn id(&self) -> Result<ProductTemplateId> {
        let mut body = [0; PRODUCT_TEMPLATE_BYTES];
        self.encode_into(&mut body)?;
        Ok(ProductTemplateId::from_bytes(
            content_id(PRODUCT_TEMPLATE_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for ProductTemplateV4 {
    const ENCODED_LEN: usize = PRODUCT_TEMPLATE_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&TEMPLATE_MAGIC);
        writer.u16(SCHEMA_V1);
        writer.u16(self.statistic_registry_value);
        writer.u16(self.coverage_policy_registry_value);
        writer.u16(0);
        writer.id(self.source_plane_contract_id);
        writer.id(self.source_spec_id);
        writer.id(self.summary_program_id);
        writer.id(self.native_claim_basis_id.content_id());
        writer.id(self.evidence_only_recovery_policy_id.content_id());
        writer.id(self.compiler_release_id);
        writer.u64(self.window_span_buckets);
        writer.u64(self.primary_maturity_grace_buckets);
        writer.u64(self.base_repair_generation);
        writer.u64(self.coverage_policy_parameter);
        writer.reserved(16);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        reject_legacy_numeric_fallback(input)?;
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&TEMPLATE_MAGIC)?;
        if reader.u16() != SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        let statistic_registry_value = reader.u16();
        let coverage_policy_registry_value = reader.u16();
        if reader.u16() != 0 {
            return Err(Error::NonCanonicalReserved);
        }
        let value = Self {
            source_plane_contract_id: reader.id(),
            source_spec_id: reader.id(),
            summary_program_id: reader.id(),
            native_claim_basis_id: NativeClaimBasisId::from_bytes(reader.id().bytes()),
            evidence_only_recovery_policy_id: EvidenceOnlyRecoveryPolicyId::from_bytes(
                reader.id().bytes(),
            ),
            compiler_release_id: reader.id(),
            statistic_registry_value,
            coverage_policy_registry_value,
            window_span_buckets: reader.u64(),
            primary_maturity_grace_buckets: reader.u64(),
            base_repair_generation: reader.u64(),
            coverage_policy_parameter: reader.u64(),
        };
        reader.reserved(16)?;
        reader.finish()?;
        value.validate_shape()?;
        Ok(value)
    }
}

/// Immutable Realm/profile and venue semantics used by market identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketGenesisProfileV1 {
    /// Immutable Realm identity.
    pub realm_id: ContentId,
    /// Immutable Profile identity selected by the Realm.
    pub profile_id: ContentId,
    /// Exact order price-grid identity.
    pub price_grid_id: ContentId,
    /// Exact fee-policy identity.
    pub fee_policy_id: ContentId,
    /// Exact settlement/evidence relation identity.
    pub relation_policy_id: ContentId,
    /// Exact score semantics identity.
    pub score_policy_id: ContentId,
    /// Exact candidate lifecycle identity.
    pub candidate_lifecycle_policy_id: ContentId,
    /// Exact candidate liveness identity.
    pub candidate_liveness_policy_id: ContentId,
    /// Exact counted-retirement policy identity.
    pub retirement_policy_id: ContentId,
    /// Exact ordered capability-profile identity.
    pub capability_profile_id: ContentId,
    /// Registry-owned terminal disposition; the live join must equal BURN.
    pub terminal_disposition_registry_value: u16,
    /// Raw native claims represented by one bearer token atom.
    pub native_bearer_lot: u64,
}

impl MarketGenesisProfileV1 {
    /// Validate exact local shape.
    pub fn validate_shape(&self) -> Result<()> {
        for id in [
            self.realm_id,
            self.profile_id,
            self.price_grid_id,
            self.fee_policy_id,
            self.relation_policy_id,
            self.score_policy_id,
            self.candidate_lifecycle_policy_id,
            self.candidate_liveness_policy_id,
            self.retirement_policy_id,
            self.capability_profile_id,
        ] {
            id.validate()?;
        }
        if self.terminal_disposition_registry_value == 0 || self.native_bearer_lot == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Join the native denominator and the registry-supplied exact BURN value.
    pub fn validate_bindings(
        &self,
        basis: &NativeClaimBasisV1,
        burn_terminal_disposition_registry_value: u16,
    ) -> Result<()> {
        self.validate_shape()?;
        basis.validate()?;
        if burn_terminal_disposition_registry_value == 0
            || self.terminal_disposition_registry_value != burn_terminal_disposition_registry_value
            || !self.native_bearer_lot.is_multiple_of(basis.denominator)
        {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }

    /// Typed identity of these exact immutable market-genesis semantics.
    pub fn id(&self) -> Result<MarketGenesisProfileId> {
        let mut body = [0; MARKET_GENESIS_PROFILE_BYTES];
        self.encode_into(&mut body)?;
        Ok(MarketGenesisProfileId::from_bytes(
            content_id(MARKET_GENESIS_PROFILE_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for MarketGenesisProfileV1 {
    const ENCODED_LEN: usize = MARKET_GENESIS_PROFILE_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&GENESIS_MAGIC);
        writer.u16(SCHEMA_V1);
        writer.u16(self.terminal_disposition_registry_value);
        writer.reserved(4);
        for id in [
            self.realm_id,
            self.profile_id,
            self.price_grid_id,
            self.fee_policy_id,
            self.relation_policy_id,
            self.score_policy_id,
            self.candidate_lifecycle_policy_id,
            self.candidate_liveness_policy_id,
            self.retirement_policy_id,
            self.capability_profile_id,
        ] {
            writer.id(id);
        }
        writer.u64(self.native_bearer_lot);
        writer.reserved(8);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&GENESIS_MAGIC)?;
        if reader.u16() != SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        let terminal_disposition_registry_value = reader.u16();
        reader.reserved(4)?;
        let value = Self {
            realm_id: reader.id(),
            profile_id: reader.id(),
            price_grid_id: reader.id(),
            fee_policy_id: reader.id(),
            relation_policy_id: reader.id(),
            score_policy_id: reader.id(),
            candidate_lifecycle_policy_id: reader.id(),
            candidate_liveness_policy_id: reader.id(),
            retirement_policy_id: reader.id(),
            capability_profile_id: reader.id(),
            terminal_disposition_registry_value,
            native_bearer_lot: reader.u64(),
        };
        reader.reserved(8)?;
        reader.finish()?;
        value.validate_shape()?;
        Ok(value)
    }
}

/// Economic market identity preimage, excluding Series funding and attachments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketInstancePreimageV1 {
    /// Reusable product semantics.
    pub product_template_id: ProductTemplateId,
    /// Immutable Realm/profile/venue semantics.
    pub market_genesis_profile_id: MarketGenesisProfileId,
    /// Absolute first observation bucket.
    pub start_bucket: u64,
    /// Market-local liability cap in collateral atoms.
    pub collateral_cap: u64,
}

impl MarketInstancePreimageV1 {
    /// Validate exact local shape.
    pub fn validate(&self) -> Result<()> {
        self.product_template_id.validate()?;
        self.market_genesis_profile_id.validate()?;
        if self.collateral_cap == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Join this preimage to the exact Template and GenesisProfile bodies.
    pub fn validate_bindings(
        &self,
        template: &ProductTemplateV4,
        genesis: &MarketGenesisProfileV1,
    ) -> Result<()> {
        self.validate()?;
        template.validate_shape()?;
        genesis.validate_shape()?;
        if self.product_template_id != template.id()?
            || self.market_genesis_profile_id != genesis.id()?
            || self.collateral_cap < genesis.native_bearer_lot
            || !self
                .collateral_cap
                .is_multiple_of(genesis.native_bearer_lot)
        {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }

    /// Full-width economic market identity.
    pub fn id(&self) -> Result<MarketInstanceId> {
        let mut body = [0; MARKET_INSTANCE_PREIMAGE_BYTES];
        self.encode_into(&mut body)?;
        Ok(MarketInstanceId::from_bytes(
            content_id(MARKET_INSTANCE_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for MarketInstancePreimageV1 {
    const ENCODED_LEN: usize = MARKET_INSTANCE_PREIMAGE_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MARKET_MAGIC);
        writer.id(self.product_template_id.content_id());
        writer.id(self.market_genesis_profile_id.content_id());
        writer.u64(self.start_bucket);
        writer.u64(self.collateral_cap);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&MARKET_MAGIC)?;
        let value = Self {
            product_template_id: ProductTemplateId::from_bytes(reader.id().bytes()),
            market_genesis_profile_id: MarketGenesisProfileId::from_bytes(reader.id().bytes()),
            start_bucket: reader.u64(),
            collateral_cap: reader.u64(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Operational attachment choices that must not fragment market identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesAttachmentPlanV1 {
    /// Exact per-component funding quote identity.
    pub funding_quote_id: ContentId,
    /// Exact liquidity-facility plan identity.
    pub liquidity_facility_plan_id: ContentId,
    /// Exact canonical wrapper-recipe set identity.
    pub wrapper_recipe_set_id: ContentId,
}

impl SeriesAttachmentPlanV1 {
    /// Validate that every component is a typed nonzero artifact reference.
    pub fn validate(&self) -> Result<()> {
        self.funding_quote_id.validate()?;
        self.liquidity_facility_plan_id.validate()?;
        self.wrapper_recipe_set_id.validate()?;
        Ok(())
    }

    /// Typed identity of these operational attachment choices.
    pub fn id(&self) -> Result<SeriesAttachmentPlanId> {
        let mut body = [0; SERIES_ATTACHMENT_PLAN_BYTES];
        self.encode_into(&mut body)?;
        Ok(SeriesAttachmentPlanId::from_bytes(
            content_id(SERIES_ATTACHMENT_PLAN_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for SeriesAttachmentPlanV1 {
    const ENCODED_LEN: usize = SERIES_ATTACHMENT_PLAN_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&ATTACHMENT_MAGIC);
        writer.u16(SCHEMA_V1);
        writer.reserved(6);
        writer.id(self.funding_quote_id);
        writer.id(self.liquidity_facility_plan_id);
        writer.id(self.wrapper_recipe_set_id);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&ATTACHMENT_MAGIC)?;
        if reader.u16() != SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            funding_quote_id: reader.id(),
            liquidity_facility_plan_id: reader.id(),
            wrapper_recipe_set_id: reader.id(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Immutable finite recurring schedule and its operational attachment plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesPlanV4 {
    /// Reusable relative product semantics.
    pub product_template_id: ProductTemplateId,
    /// Immutable Realm/profile/venue semantics.
    pub market_genesis_profile_id: MarketGenesisProfileId,
    /// Work/liquidity/wrapper choices excluded from market identity.
    pub attachment_plan_id: SeriesAttachmentPlanId,
    /// First absolute observation bucket.
    pub first_start_bucket: u64,
    /// Positive bucket stride between consecutive ordinals.
    pub stride_buckets: u64,
    /// Finite nonzero occurrence count.
    pub instance_count: u32,
    /// Buckets before start during which creation is eligible.
    pub creation_lead_buckets: u64,
    /// Per-market liability cap in collateral atoms.
    pub market_collateral_cap: u64,
}

impl SeriesPlanV4 {
    /// Validate local references and finite recurrence shape.
    pub fn validate_shape(&self) -> Result<()> {
        self.product_template_id.validate()?;
        self.market_genesis_profile_id.validate()?;
        self.attachment_plan_id.validate()?;
        if self.stride_buckets == 0
            || self.instance_count == 0
            || self.instance_count > MAX_SERIES_INSTANCES
            || self.creation_lead_buckets == 0
            || self.first_start_bucket < self.creation_lead_buckets
            || self.market_collateral_cap == 0
        {
            return Err(Error::InvalidParameter);
        }
        self.start_bucket(self.instance_count - 1)?;
        Ok(())
    }

    /// Derive the exact start bucket for one ordinal.
    pub fn start_bucket(&self, ordinal: u32) -> Result<u64> {
        if ordinal >= self.instance_count {
            return Err(Error::WrongOrdinal);
        }
        self.first_start_bucket
            .checked_add(
                self.stride_buckets
                    .checked_mul(u64::from(ordinal))
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Derive the inclusive creation-eligibility opening bucket.
    pub fn creation_open_bucket(&self, ordinal: u32) -> Result<u64> {
        self.start_bucket(ordinal)?
            .checked_sub(self.creation_lead_buckets)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Whether a bucket lies in the exact `[start - lead, start)` interval.
    pub fn is_creation_eligible(&self, ordinal: u32, current_bucket: u64) -> Result<bool> {
        let start = self.start_bucket(ordinal)?;
        Ok(current_bucket >= self.creation_open_bucket(ordinal)? && current_bucket < start)
    }

    /// Validate exact referenced bodies and the final possible recovery deadline.
    pub fn validate_bindings(
        &self,
        template: &ProductTemplateV4,
        basis: &NativeClaimBasisV1,
        recovery: &EvidenceOnlyRecoveryPolicyV1,
        genesis: &MarketGenesisProfileV1,
        attachment: &SeriesAttachmentPlanV1,
        burn_terminal_disposition_registry_value: u16,
    ) -> Result<()> {
        self.validate_shape()?;
        template.validate_bindings(basis, recovery)?;
        genesis.validate_bindings(basis, burn_terminal_disposition_registry_value)?;
        attachment.validate()?;
        if self.product_template_id != template.id()?
            || self.market_genesis_profile_id != genesis.id()?
            || self.attachment_plan_id != attachment.id()?
            || self.market_collateral_cap < genesis.native_bearer_lot
            || !self
                .market_collateral_cap
                .is_multiple_of(genesis.native_bearer_lot)
        {
            return Err(Error::MismatchedArtifact);
        }
        let final_start = self.start_bucket(self.instance_count - 1)?;
        let primary_maturity = final_start
            .checked_add(template.window_span_buckets)
            .and_then(|value| value.checked_add(template.primary_maturity_grace_buckets))
            .ok_or(Error::ArithmeticOverflow)?;
        let last = recovery.attempts[usize::from(recovery.attempt_count) - 1];
        primary_maturity
            .checked_add(last.closes_after_primary_maturity_buckets)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    /// Typed identity of this finite schedule and attachment choice.
    pub fn id(&self) -> Result<SeriesPlanId> {
        let mut body = [0; SERIES_PLAN_BYTES];
        self.encode_into(&mut body)?;
        Ok(SeriesPlanId::from_bytes(
            content_id(SERIES_PLAN_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for SeriesPlanV4 {
    const ENCODED_LEN: usize = SERIES_PLAN_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&SERIES_MAGIC);
        writer.u16(SCHEMA_V1);
        writer.reserved(6);
        writer.id(self.product_template_id.content_id());
        writer.id(self.market_genesis_profile_id.content_id());
        writer.id(self.attachment_plan_id.content_id());
        writer.u64(self.first_start_bucket);
        writer.u64(self.stride_buckets);
        writer.u32(self.instance_count);
        writer.reserved(4);
        writer.u64(self.creation_lead_buckets);
        writer.u64(self.market_collateral_cap);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&SERIES_MAGIC)?;
        if reader.u16() != SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            product_template_id: ProductTemplateId::from_bytes(reader.id().bytes()),
            market_genesis_profile_id: MarketGenesisProfileId::from_bytes(reader.id().bytes()),
            attachment_plan_id: SeriesAttachmentPlanId::from_bytes(reader.id().bytes()),
            first_start_bucket: reader.u64(),
            stride_buckets: reader.u64(),
            instance_count: reader.u32(),
            creation_lead_buckets: {
                reader.reserved(4)?;
                reader.u64()
            },
            market_collateral_cap: reader.u64(),
        };
        reader.finish()?;
        value.validate_shape()?;
        Ok(value)
    }
}

/// Immutable funding ownership and refund identities for one Series activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingTermsV1 {
    /// Exact immutable Series being funded.
    pub series_plan_id: SeriesPlanId,
    /// Persisted owner of refundable lamport principal.
    pub lamport_principal_refund: ContentId,
    /// Persisted token account receiving refundable collateral principal.
    pub collateral_principal_refund_token_account: ContentId,
    /// Immutable neutral destination for unowned residue.
    pub neutral_sink: ContentId,
    /// Exact collateral mint selected through the immutable Realm/Profile.
    pub collateral_mint: ContentId,
    /// Exact admitted token-program identity.
    pub token_program: ContentId,
}

impl SeriesFundingTermsV1 {
    /// Validate exact local shape.
    pub fn validate_shape(&self) -> Result<()> {
        self.series_plan_id.validate()?;
        for id in [
            self.lamport_principal_refund,
            self.collateral_principal_refund_token_account,
            self.neutral_sink,
            self.collateral_mint,
            self.token_program,
        ] {
            id.validate()?;
        }
        Ok(())
    }

    /// Join funding ownership to Series, Genesis, and admitted collateral.
    ///
    /// The final two identities are projections from an adapter-authenticated
    /// immutable collateral policy. The policy remains their semantic owner;
    /// this codec stores only the exact funding-side references.
    pub fn validate_bindings(
        &self,
        series: &SeriesPlanV4,
        genesis: &MarketGenesisProfileV1,
        policy_admitted_collateral_mint: ContentId,
        drivable_token_program: ContentId,
    ) -> Result<()> {
        self.validate_shape()?;
        series.validate_shape()?;
        genesis.validate_shape()?;
        policy_admitted_collateral_mint.validate()?;
        drivable_token_program.validate()?;
        if self.series_plan_id != series.id()?
            || series.market_genesis_profile_id != genesis.id()?
            || self.collateral_mint != policy_admitted_collateral_mint
            || self.token_program != drivable_token_program
        {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }

    /// Typed identity of funding ownership, distinct from Series and market IDs.
    pub fn id(&self) -> Result<SeriesFundingTermsId> {
        let mut body = [0; SERIES_FUNDING_TERMS_BYTES];
        self.encode_into(&mut body)?;
        Ok(SeriesFundingTermsId::from_bytes(
            content_id(SERIES_FUNDING_TERMS_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for SeriesFundingTermsV1 {
    const ENCODED_LEN: usize = SERIES_FUNDING_TERMS_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&FUNDING_TERMS_MAGIC);
        writer.u16(SCHEMA_V1);
        writer.reserved(6);
        writer.id(self.series_plan_id.content_id());
        writer.id(self.lamport_principal_refund);
        writer.id(self.collateral_principal_refund_token_account);
        writer.id(self.neutral_sink);
        writer.id(self.collateral_mint);
        writer.id(self.token_program);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&FUNDING_TERMS_MAGIC)?;
        if reader.u16() != SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            series_plan_id: SeriesPlanId::from_bytes(reader.id().bytes()),
            lamport_principal_refund: reader.id(),
            collateral_principal_refund_token_account: reader.id(),
            neutral_sink: reader.id(),
            collateral_mint: reader.id(),
            token_program: reader.id(),
        };
        reader.finish()?;
        value.validate_shape()?;
        Ok(value)
    }
}
