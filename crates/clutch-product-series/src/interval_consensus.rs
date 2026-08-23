//! Resumable exhaustive consensus over a quantized source interval.
//!
//! This module deliberately proves only a finite integer-lattice statement:
//! every integer coordinate in one authenticated SourcePlane interval was
//! evaluated by the canonical Product-selected B-spline evaluator and produced
//! one identical exact payout vector. It never samples, chooses a midpoint, or
//! infers an integer result from a continuous model.
//!
//! The fixed work record is a structural, non-authorizing value. Program-owned
//! account state and its write history are an adapter responsibility. The
//! private [`VerifiedQuantizedIntervalPayoutV1`] is therefore emitted only by
//! an in-memory session whose history begins in this module. Restoring that
//! capability from persisted work remains disabled until an SBF adapter can
//! authenticate the work PDA, owner, lifecycle, and replay chain.

use clutch_bspline::{Error as BasisError, ValidatedBasisSpec, WeightVector};
use clutch_price_measure::QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1;
use clutch_source_plane_v3::{
    Error as SourceError, StatisticKeyV3, StatisticResultV3, SummaryProgramV3, WindowSealV3,
    WindowSpecV3,
};
use sha2::{Digest, Sha256};

use crate::codec::{Reader, Writer};
use crate::{
    content_id, CompiledSourceOccurrenceV3, ContentId, Error, FixedCodec, MarketGenesisProfileV2,
    MarketGenesisProfileV2Id, MarketInstancePreimageV2, MarketInstanceV2Id, NativeClaimBasisId,
    NativeClaimBasisV1, PriceMeasurePolicyV1, PriceMeasurePolicyV1Id, ProductTemplateId,
    ProductTemplateV4, QuantizedEdgePolicyV1, QuantizedIntervalConsensusCertificateV1Id,
    QuantizedIntervalConsensusProfileV1Id, Result, SourceOccurrenceV1Id, MAX_OUTCOMES,
};

const PROFILE_MAGIC_V1: [u8; 8] = *b"DCQICP1\0";
const WORK_MAGIC_V1: [u8; 8] = *b"DCQICW1\0";
const CERTIFICATE_MAGIC_V1: [u8; 8] = *b"DCQICC1\0";
const SCHEMA_V1: u16 = 1;
const WORK_STATUS_ACTIVE_V1: u8 = 1;
const WORK_STATUS_COMPLETE_V1: u8 = 2;
const TRANSCRIPT_INITIAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/quantized-interval-consensus-transcript-initial/v1";
const TRANSCRIPT_STEP_DOMAIN_V1: &[u8] =
    b"dragons-clutch/quantized-interval-consensus-transcript-step/v1";
const PROFILE_DOMAIN_V1: &[u8] = b"dragons-clutch/quantized-interval-consensus-profile/v1";
const CERTIFICATE_DOMAIN_V1: &[u8] = b"dragons-clutch/quantized-interval-consensus-certificate/v1";
const ROUNDING_POLICY_PREIMAGE_V1: &[u8] =
    b"WEIGHT-ROUND-01/floor-largest-remainders/lowest-outcome-index-ties/v1";

/// Frozen canonical B-spline evaluator semantic version.
pub const BASIS_EVALUATOR_VERSION_V1: u8 = 1;
/// The pure contract exists, but no SBF/account capability is active.
pub const QUANTIZED_INTERVAL_CONSENSUS_RUNTIME_CAPABILITY_ENABLED_V1: bool = false;
/// Exact canonical profile body width.
pub const QUANTIZED_INTERVAL_CONSENSUS_PROFILE_BYTES_V1: usize = 64;
/// Exact canonical active/complete work body width.
pub const QUANTIZED_INTERVAL_CONSENSUS_WORK_BYTES_V1: usize = 592;
/// Exact canonical exhaustive certificate body width.
pub const QUANTIZED_INTERVAL_CONSENSUS_CERTIFICATE_BYTES_V1: usize = 576;
/// Domain naming the one admitted quantized payout rounding policy.
pub const QUANTIZED_INTERVAL_ROUNDING_POLICY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/quantized-interval-rounding-policy/v1";

const _: () = assert!(MAX_OUTCOMES == clutch_bspline::MAX_OUTCOMES);
const _: () = assert!(QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1 == 1);

/// Exact identity of `WEIGHT-ROUND-01` used by every coordinate evaluation.
pub fn quantized_interval_rounding_policy_id_v1() -> ContentId {
    content_id(
        QUANTIZED_INTERVAL_ROUNDING_POLICY_DOMAIN_V1,
        ROUNDING_POLICY_PREIMAGE_V1,
    )
}

/// Fail closed until a live account/runtime profile is deliberately activated.
pub const fn require_quantized_interval_consensus_runtime_capability_v1() -> Result<()> {
    Err(Error::RuntimeCapabilityDisabled)
}

/// Profile-selected resource bounds without profile-selected payout semantics.
///
/// The evaluator, quantized semantic version, and rounding rule are global V1
/// constants. This profile may only narrow how wide an interval may be and how
/// many coordinates one transition may evaluate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedIntervalConsensusProfileV1 {
    /// Existing central capability profile that selected these bounds.
    pub capability_profile_id: ContentId,
    /// Largest admitted `high - low`; must leave room for the inclusive count.
    pub maximum_interval_width: u64,
    /// Largest coordinate count accepted by one bounded advance.
    pub maximum_coordinates_per_advance: u16,
}

impl QuantizedIntervalConsensusProfileV1 {
    /// Validate only resource bounds and the existing profile identity.
    pub fn validate(&self) -> Result<()> {
        self.capability_profile_id.validate()?;
        if self.maximum_interval_width == u64::MAX || self.maximum_coordinates_per_advance == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Typed identity of this exact limit profile.
    pub fn id(&self) -> Result<QuantizedIntervalConsensusProfileV1Id> {
        let mut bytes = [0; QUANTIZED_INTERVAL_CONSENSUS_PROFILE_BYTES_V1];
        self.encode_into(&mut bytes)?;
        Ok(QuantizedIntervalConsensusProfileV1Id::from_bytes(
            content_id(PROFILE_DOMAIN_V1, &bytes).bytes(),
        ))
    }
}

impl FixedCodec for QuantizedIntervalConsensusProfileV1 {
    const ENCODED_LEN: usize = QUANTIZED_INTERVAL_CONSENSUS_PROFILE_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&PROFILE_MAGIC_V1);
        writer.u16(SCHEMA_V1);
        writer.reserved(6);
        writer.id(self.capability_profile_id);
        writer.u64(self.maximum_interval_width);
        writer.u16(self.maximum_coordinates_per_advance);
        writer.reserved(6);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&PROFILE_MAGIC_V1)?;
        if reader.u16() != SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            capability_profile_id: reader.id(),
            maximum_interval_width: reader.u64(),
            maximum_coordinates_per_advance: reader.u16(),
        };
        reader.reserved(6)?;
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Complete canonical bodies required to start or advance one pure work item.
///
/// This is a borrowed join, not another persisted truth. Source and registry
/// authentication remain adapter responsibilities.
#[derive(Clone, Copy, Debug)]
pub struct QuantizedIntervalConsensusContextV1<'a> {
    /// Exact economic Market preimage.
    pub market: &'a MarketInstancePreimageV2,
    /// Reusable Product terms body named by the Market.
    pub product_template: &'a ProductTemplateV4,
    /// Sole Product owner of the native basis.
    pub native_claim_basis: &'a NativeClaimBasisV1,
    /// Exact quantized evaluator/checker policy.
    pub price_measure_policy: &'a PriceMeasurePolicyV1,
    /// Exact Realm/profile/domain semantics named by the Market.
    pub market_genesis: &'a MarketGenesisProfileV2,
    /// Registry-authenticated resolution of the Product edge selector.
    pub resolved_edge_policy: QuantizedEdgePolicyV1,
    /// Canonical Product-to-Source occurrence provenance.
    pub source_occurrence: &'a CompiledSourceOccurrenceV3,
    /// Exact immutable successful SourcePlane interval result.
    pub source_interval: &'a StatisticResultV3,
    /// Exact predictable source statistic key.
    pub statistic_key: &'a StatisticKeyV3,
    /// Exact reviewed source-neutral summary program.
    pub summary_program: &'a SummaryProgramV3,
    /// Exact immutable seal that owns the source evidence root.
    pub window_seal: &'a WindowSealV3,
    /// Exact semantic SourcePlane window.
    pub window: &'a WindowSpecV3,
    /// Existing capability profile's bounded work selection.
    pub work_profile: &'a QuantizedIntervalConsensusProfileV1,
}

#[derive(Clone, Copy, Debug)]
struct ValidatedContextV1 {
    market_instance_id: MarketInstanceV2Id,
    product_template_id: ProductTemplateId,
    market_genesis_profile_id: MarketGenesisProfileV2Id,
    native_claim_basis_id: NativeClaimBasisId,
    source_occurrence_id: SourceOccurrenceV1Id,
    source_interval_id: ContentId,
    price_measure_policy_id: PriceMeasurePolicyV1Id,
    capability_profile_id: ContentId,
    interval_profile_id: QuantizedIntervalConsensusProfileV1Id,
    evaluator_release_id: ContentId,
    rounding_policy_id: ContentId,
    low: u128,
    high: u128,
    total_coordinates: u64,
    maximum_interval_width: u64,
    maximum_coordinates_per_advance: u16,
    outcome_count: u8,
    denominator: u64,
    evaluator: ValidatedBasisSpec,
}

/// Fixed structural work record for one exhaustive integer-coordinate scan.
///
/// Decoding this type does not authenticate a persisted account or its history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedIntervalConsensusWorkV1 {
    market_instance_id: MarketInstanceV2Id,
    product_template_id: ProductTemplateId,
    market_genesis_profile_id: MarketGenesisProfileV2Id,
    native_claim_basis_id: NativeClaimBasisId,
    source_occurrence_id: SourceOccurrenceV1Id,
    source_interval_id: ContentId,
    price_measure_policy_id: PriceMeasurePolicyV1Id,
    capability_profile_id: ContentId,
    interval_profile_id: QuantizedIntervalConsensusProfileV1Id,
    evaluator_release_id: ContentId,
    rounding_policy_id: ContentId,
    transcript: ContentId,
    low: u128,
    high: u128,
    checked_coordinates: u64,
    maximum_interval_width: u64,
    maximum_coordinates_per_advance: u16,
    denominator: u64,
    weights: [u64; MAX_OUTCOMES],
    status: u8,
    outcome_count: u8,
}

impl QuantizedIntervalConsensusWorkV1 {
    /// Exact economic Market identity bound at Begin.
    pub const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact Product terms/template identity bound at Begin.
    pub const fn product_template_id(&self) -> ProductTemplateId {
        self.product_template_id
    }

    /// Exact Realm/profile/domain Genesis identity bound at Begin.
    pub const fn market_genesis_profile_id(&self) -> MarketGenesisProfileV2Id {
        self.market_genesis_profile_id
    }

    /// Sole native basis identity evaluated by this work.
    pub const fn native_claim_basis_id(&self) -> NativeClaimBasisId {
        self.native_claim_basis_id
    }

    /// Canonical Product-to-Source occurrence provenance identity.
    pub const fn source_occurrence_id(&self) -> SourceOccurrenceV1Id {
        self.source_occurrence_id
    }

    /// Immutable SourcePlane result identity owning the interval.
    pub const fn source_interval_id(&self) -> ContentId {
        self.source_interval_id
    }

    /// Exact quantized price/evaluator policy identity.
    pub const fn price_measure_policy_id(&self) -> PriceMeasurePolicyV1Id {
        self.price_measure_policy_id
    }

    /// Central capability-profile identity that selected the work bounds.
    pub const fn capability_profile_id(&self) -> ContentId {
        self.capability_profile_id
    }

    /// Canonical bounded interval-work profile identity.
    pub const fn interval_profile_id(&self) -> QuantizedIntervalConsensusProfileV1Id {
        self.interval_profile_id
    }

    /// Exact checked evaluator release bound by PriceMeasurePolicy V1.
    pub const fn evaluator_release_id(&self) -> ContentId {
        self.evaluator_release_id
    }

    /// Exact frozen `WEIGHT-ROUND-01` identity.
    pub const fn rounding_policy_id(&self) -> ContentId {
        self.rounding_policy_id
    }

    /// Inclusive source interval copied from the bound immutable result.
    pub const fn interval(&self) -> (u128, u128) {
        (self.low, self.high)
    }

    /// Number of exhaustively checked integer coordinates.
    pub const fn checked_coordinates(&self) -> u64 {
        self.checked_coordinates
    }

    /// Inclusive interval coordinate count.
    pub fn total_coordinates(&self) -> Result<u64> {
        inclusive_coordinate_count(self.low, self.high)
    }

    /// Whether every coordinate has been processed without disagreement.
    pub const fn is_complete(&self) -> bool {
        self.status == WORK_STATUS_COMPLETE_V1
    }

    /// Current canonical rolling transcript commitment.
    pub const fn transcript(&self) -> ContentId {
        self.transcript
    }

    /// Structurally latched payout, absent until at least one coordinate ran.
    pub fn latched_payout(&self) -> Option<WeightVector> {
        if self.checked_coordinates == 0 {
            None
        } else {
            Some(WeightVector {
                active_len: self.outcome_count,
                denominator: self.denominator,
                weights: self.weights,
            })
        }
    }

    fn new(context: ValidatedContextV1) -> Result<Self> {
        let mut value = Self {
            market_instance_id: context.market_instance_id,
            product_template_id: context.product_template_id,
            market_genesis_profile_id: context.market_genesis_profile_id,
            native_claim_basis_id: context.native_claim_basis_id,
            source_occurrence_id: context.source_occurrence_id,
            source_interval_id: context.source_interval_id,
            price_measure_policy_id: context.price_measure_policy_id,
            capability_profile_id: context.capability_profile_id,
            interval_profile_id: context.interval_profile_id,
            evaluator_release_id: context.evaluator_release_id,
            rounding_policy_id: context.rounding_policy_id,
            transcript: ContentId::ZERO,
            low: context.low,
            high: context.high,
            checked_coordinates: 0,
            maximum_interval_width: context.maximum_interval_width,
            maximum_coordinates_per_advance: context.maximum_coordinates_per_advance,
            denominator: context.denominator,
            weights: [0; MAX_OUTCOMES],
            status: WORK_STATUS_ACTIVE_V1,
            outcome_count: context.outcome_count,
        };
        value.transcript = initial_transcript(&value);
        value.validate_shape()?;
        Ok(value)
    }

    fn validate_shape(&self) -> Result<()> {
        self.market_instance_id.validate()?;
        self.product_template_id.validate()?;
        self.market_genesis_profile_id.validate()?;
        self.native_claim_basis_id.validate()?;
        self.source_occurrence_id.validate()?;
        self.source_interval_id.validate()?;
        self.price_measure_policy_id.validate()?;
        self.capability_profile_id.validate()?;
        self.interval_profile_id.validate()?;
        self.evaluator_release_id.validate()?;
        self.rounding_policy_id.validate()?;
        self.transcript.validate()?;
        if self.rounding_policy_id != quantized_interval_rounding_policy_id_v1()
            || !(2..=u8::try_from(MAX_OUTCOMES).map_err(|_| Error::InvalidParameter)?)
                .contains(&self.outcome_count)
            || self.denominator == 0
            || self.maximum_interval_width == u64::MAX
            || self.maximum_coordinates_per_advance == 0
        {
            return Err(Error::InvalidParameter);
        }
        let width = self
            .high
            .checked_sub(self.low)
            .ok_or(Error::InvalidParameter)?;
        if width > u128::from(self.maximum_interval_width) {
            return Err(Error::IntervalTooWide);
        }
        let total = inclusive_coordinate_count(self.low, self.high)?;
        if self.checked_coordinates > total {
            return Err(Error::WorkStateMismatch);
        }
        match self.status {
            WORK_STATUS_ACTIVE_V1 if self.checked_coordinates < total => {}
            WORK_STATUS_COMPLETE_V1 if self.checked_coordinates == total => {}
            WORK_STATUS_ACTIVE_V1 | WORK_STATUS_COMPLETE_V1 => {
                return Err(Error::WorkStateMismatch)
            }
            _ => return Err(Error::InvalidParameter),
        }
        if self.checked_coordinates == 0 {
            if self.weights.iter().any(|weight| *weight != 0) {
                return Err(Error::NonCanonicalPadding);
            }
        } else {
            WeightVector {
                active_len: self.outcome_count,
                denominator: self.denominator,
                weights: self.weights,
            }
            .validate()
            .map_err(map_basis_error)?;
        }
        Ok(())
    }

    fn validate_against(&self, context: ValidatedContextV1) -> Result<()> {
        self.validate_shape()?;
        if self.market_instance_id != context.market_instance_id
            || self.product_template_id != context.product_template_id
            || self.market_genesis_profile_id != context.market_genesis_profile_id
            || self.native_claim_basis_id != context.native_claim_basis_id
            || self.source_occurrence_id != context.source_occurrence_id
            || self.source_interval_id != context.source_interval_id
            || self.price_measure_policy_id != context.price_measure_policy_id
            || self.capability_profile_id != context.capability_profile_id
            || self.interval_profile_id != context.interval_profile_id
            || self.evaluator_release_id != context.evaluator_release_id
            || self.rounding_policy_id != context.rounding_policy_id
            || self.low != context.low
            || self.high != context.high
            || self.maximum_interval_width != context.maximum_interval_width
            || self.maximum_coordinates_per_advance != context.maximum_coordinates_per_advance
            || self.outcome_count != context.outcome_count
            || self.denominator != context.denominator
        {
            return Err(Error::WorkStateMismatch);
        }
        if self.checked_coordinates == 0 && self.transcript != initial_transcript(self) {
            return Err(Error::WorkStateMismatch);
        }
        Ok(())
    }
}

impl FixedCodec for QuantizedIntervalConsensusWorkV1 {
    const ENCODED_LEN: usize = QUANTIZED_INTERVAL_CONSENSUS_WORK_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&WORK_MAGIC_V1);
        writer.u16(SCHEMA_V1);
        writer.u8(self.status);
        writer.u8(self.outcome_count);
        writer.u8(QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1);
        writer.u8(BASIS_EVALUATOR_VERSION_V1);
        writer.reserved(2);
        write_work_ids(&mut writer, self);
        writer.id(self.transcript);
        writer.u128(self.low);
        writer.u128(self.high);
        writer.u64(self.checked_coordinates);
        writer.u64(self.maximum_interval_width);
        writer.u16(self.maximum_coordinates_per_advance);
        writer.reserved(6);
        writer.u64(self.denominator);
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            writer.u64(self.weights[index]);
            index += 1;
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&WORK_MAGIC_V1)?;
        if reader.u16() != SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        let status = reader.u8();
        let outcome_count = reader.u8();
        if reader.u8() != QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1
            || reader.u8() != BASIS_EVALUATOR_VERSION_V1
        {
            return Err(Error::BadVersion);
        }
        reader.reserved(2)?;
        let market_instance_id = MarketInstanceV2Id::from_bytes(reader.id().bytes());
        let product_template_id = ProductTemplateId::from_bytes(reader.id().bytes());
        let market_genesis_profile_id = MarketGenesisProfileV2Id::from_bytes(reader.id().bytes());
        let native_claim_basis_id = NativeClaimBasisId::from_bytes(reader.id().bytes());
        let source_occurrence_id = SourceOccurrenceV1Id::from_bytes(reader.id().bytes());
        let source_interval_id = reader.id();
        let price_measure_policy_id = PriceMeasurePolicyV1Id::from_bytes(reader.id().bytes());
        let capability_profile_id = reader.id();
        let interval_profile_id =
            QuantizedIntervalConsensusProfileV1Id::from_bytes(reader.id().bytes());
        let evaluator_release_id = reader.id();
        let rounding_policy_id = reader.id();
        let transcript = reader.id();
        let low = reader.u128();
        let high = reader.u128();
        let checked_coordinates = reader.u64();
        let maximum_interval_width = reader.u64();
        let maximum_coordinates_per_advance = reader.u16();
        reader.reserved(6)?;
        let denominator = reader.u64();
        let mut weights = [0; MAX_OUTCOMES];
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            weights[index] = reader.u64();
            index += 1;
        }
        reader.finish()?;
        let value = Self {
            market_instance_id,
            product_template_id,
            market_genesis_profile_id,
            native_claim_basis_id,
            source_occurrence_id,
            source_interval_id,
            price_measure_policy_id,
            capability_profile_id,
            interval_profile_id,
            evaluator_release_id,
            rounding_policy_id,
            transcript,
            low,
            high,
            checked_coordinates,
            maximum_interval_width,
            maximum_coordinates_per_advance,
            denominator,
            weights,
            status,
            outcome_count,
        };
        value.validate_shape()?;
        Ok(value)
    }
}

/// Public structural progress; this is not a verified payout capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedIntervalConsensusProgressV1 {
    /// Number of coordinates processed in this call.
    pub processed_coordinates: u16,
    /// Cumulative exact coordinate count.
    pub checked_coordinates: u64,
    /// Inclusive total coordinate count.
    pub total_coordinates: u64,
    /// Whether the structural cursor reached the upper bound.
    pub is_complete: bool,
    /// Exact rolling transcript after this call.
    pub transcript: ContentId,
}

/// Advance an untrusted structural work record by one bounded exact chunk.
///
/// This function is useful to the future account adapter, but its output alone
/// never mints [`VerifiedQuantizedIntervalPayoutV1`].
pub fn advance_quantized_interval_consensus_work_v1(
    work: &QuantizedIntervalConsensusWorkV1,
    context: QuantizedIntervalConsensusContextV1<'_>,
    requested_coordinates: u16,
) -> Result<(
    QuantizedIntervalConsensusWorkV1,
    QuantizedIntervalConsensusProgressV1,
)> {
    let validated = validate_context(context)?;
    advance_work(*work, validated, requested_coordinates)
}

/// In-memory checked history that cannot be reconstructed from caller bytes.
#[derive(Clone, Copy, Debug)]
pub struct QuantizedIntervalConsensusSessionV1 {
    work: QuantizedIntervalConsensusWorkV1,
}

/// Begin a private checked session from complete canonical bodies.
pub fn begin_quantized_interval_consensus_v1(
    context: QuantizedIntervalConsensusContextV1<'_>,
) -> Result<QuantizedIntervalConsensusSessionV1> {
    let validated = validate_context(context)?;
    Ok(QuantizedIntervalConsensusSessionV1 {
        work: QuantizedIntervalConsensusWorkV1::new(validated)?,
    })
}

impl QuantizedIntervalConsensusSessionV1 {
    /// Inspect the fixed structural work record without obtaining mutation or
    /// an account-authentication claim.
    pub const fn work(&self) -> &QuantizedIntervalConsensusWorkV1 {
        &self.work
    }

    /// Exhaustively process one bounded prefix of the remaining coordinates.
    pub fn advance(
        &mut self,
        context: QuantizedIntervalConsensusContextV1<'_>,
        requested_coordinates: u16,
    ) -> Result<QuantizedIntervalConsensusProgressV1> {
        let validated = validate_context(context)?;
        self.advance_validated(validated, requested_coordinates)
    }

    fn advance_validated(
        &mut self,
        validated: ValidatedContextV1,
        requested_coordinates: u16,
    ) -> Result<QuantizedIntervalConsensusProgressV1> {
        let (next, progress) = advance_work(self.work, validated, requested_coordinates)?;
        self.work = next;
        Ok(progress)
    }

    /// Mint the private algebraic capability only after exhaustive completion.
    pub fn verified_payout(
        &self,
        context: QuantizedIntervalConsensusContextV1<'_>,
    ) -> Result<VerifiedQuantizedIntervalPayoutV1> {
        let validated = validate_context(context)?;
        self.verified_payout_validated(validated)
    }

    fn verified_payout_validated(
        &self,
        validated: ValidatedContextV1,
    ) -> Result<VerifiedQuantizedIntervalPayoutV1> {
        self.work.validate_against(validated)?;
        if !self.work.is_complete() {
            return Err(Error::WorkIncomplete);
        }
        let certificate = QuantizedIntervalConsensusCertificateV1::from_complete_work(self.work)?;
        Ok(VerifiedQuantizedIntervalPayoutV1 { certificate })
    }
}

/// Canonical exhaustive result body. Decoding it does not create capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedIntervalConsensusCertificateV1 {
    market_instance_id: MarketInstanceV2Id,
    product_template_id: ProductTemplateId,
    market_genesis_profile_id: MarketGenesisProfileV2Id,
    native_claim_basis_id: NativeClaimBasisId,
    source_occurrence_id: SourceOccurrenceV1Id,
    source_interval_id: ContentId,
    price_measure_policy_id: PriceMeasurePolicyV1Id,
    capability_profile_id: ContentId,
    interval_profile_id: QuantizedIntervalConsensusProfileV1Id,
    evaluator_release_id: ContentId,
    rounding_policy_id: ContentId,
    transcript: ContentId,
    low: u128,
    high: u128,
    coordinate_count: u64,
    payout: WeightVector,
}

impl QuantizedIntervalConsensusCertificateV1 {
    fn from_complete_work(work: QuantizedIntervalConsensusWorkV1) -> Result<Self> {
        work.validate_shape()?;
        if !work.is_complete() {
            return Err(Error::WorkIncomplete);
        }
        let payout = work.latched_payout().ok_or(Error::WorkStateMismatch)?;
        let value = Self {
            market_instance_id: work.market_instance_id,
            product_template_id: work.product_template_id,
            market_genesis_profile_id: work.market_genesis_profile_id,
            native_claim_basis_id: work.native_claim_basis_id,
            source_occurrence_id: work.source_occurrence_id,
            source_interval_id: work.source_interval_id,
            price_measure_policy_id: work.price_measure_policy_id,
            capability_profile_id: work.capability_profile_id,
            interval_profile_id: work.interval_profile_id,
            evaluator_release_id: work.evaluator_release_id,
            rounding_policy_id: work.rounding_policy_id,
            transcript: work.transcript,
            low: work.low,
            high: work.high,
            coordinate_count: work.total_coordinates()?,
            payout,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate exact identities, interval width, count, and payout simplex.
    pub fn validate(&self) -> Result<()> {
        self.market_instance_id.validate()?;
        self.product_template_id.validate()?;
        self.market_genesis_profile_id.validate()?;
        self.native_claim_basis_id.validate()?;
        self.source_occurrence_id.validate()?;
        self.source_interval_id.validate()?;
        self.price_measure_policy_id.validate()?;
        self.capability_profile_id.validate()?;
        self.interval_profile_id.validate()?;
        self.evaluator_release_id.validate()?;
        self.rounding_policy_id.validate()?;
        self.transcript.validate()?;
        if self.rounding_policy_id != quantized_interval_rounding_policy_id_v1()
            || self.coordinate_count != inclusive_coordinate_count(self.low, self.high)?
        {
            return Err(Error::WorkStateMismatch);
        }
        self.payout.validate().map_err(map_basis_error)
    }

    /// Exact canonical payout proven constant over the integer interval.
    pub const fn payout(&self) -> WeightVector {
        self.payout
    }

    /// Exact economic Market identity certified by this transcript.
    pub const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact Product terms/template identity certified by this transcript.
    pub const fn product_template_id(&self) -> ProductTemplateId {
        self.product_template_id
    }

    /// Exact Realm/profile/domain Genesis identity certified by this transcript.
    pub const fn market_genesis_profile_id(&self) -> MarketGenesisProfileV2Id {
        self.market_genesis_profile_id
    }

    /// Sole Product-native basis identity used for all evaluations.
    pub const fn native_claim_basis_id(&self) -> NativeClaimBasisId {
        self.native_claim_basis_id
    }

    /// Product-to-Source occurrence provenance identity.
    pub const fn source_occurrence_id(&self) -> SourceOccurrenceV1Id {
        self.source_occurrence_id
    }

    /// Quantized price/evaluator policy identity.
    pub const fn price_measure_policy_id(&self) -> PriceMeasurePolicyV1Id {
        self.price_measure_policy_id
    }

    /// Existing capability-profile identity that selected resource bounds.
    pub const fn capability_profile_id(&self) -> ContentId {
        self.capability_profile_id
    }

    /// Exact bounded interval-work profile identity.
    pub const fn interval_profile_id(&self) -> QuantizedIntervalConsensusProfileV1Id {
        self.interval_profile_id
    }

    /// Checked evaluator release identity.
    pub const fn evaluator_release_id(&self) -> ContentId {
        self.evaluator_release_id
    }

    /// Canonical `WEIGHT-ROUND-01` identity.
    pub const fn rounding_policy_id(&self) -> ContentId {
        self.rounding_policy_id
    }

    /// Inclusive source interval proven by exhaustive integer evaluation.
    pub const fn interval(&self) -> (u128, u128) {
        (self.low, self.high)
    }

    /// Exact inclusive integer coordinate count evaluated.
    pub const fn coordinate_count(&self) -> u64 {
        self.coordinate_count
    }

    /// Exact source result identity owning the interval.
    pub const fn source_interval_id(&self) -> ContentId {
        self.source_interval_id
    }

    /// Canonical rolling transcript after the upper endpoint.
    pub const fn transcript(&self) -> ContentId {
        self.transcript
    }

    /// Typed content identity of these complete certificate bytes.
    pub fn id(&self) -> Result<QuantizedIntervalConsensusCertificateV1Id> {
        let mut bytes = [0; QUANTIZED_INTERVAL_CONSENSUS_CERTIFICATE_BYTES_V1];
        self.encode_into(&mut bytes)?;
        Ok(QuantizedIntervalConsensusCertificateV1Id::from_bytes(
            content_id(CERTIFICATE_DOMAIN_V1, &bytes).bytes(),
        ))
    }
}

impl FixedCodec for QuantizedIntervalConsensusCertificateV1 {
    const ENCODED_LEN: usize = QUANTIZED_INTERVAL_CONSENSUS_CERTIFICATE_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&CERTIFICATE_MAGIC_V1);
        writer.u16(SCHEMA_V1);
        writer.u8(self.payout.active_len);
        writer.u8(QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1);
        writer.u8(BASIS_EVALUATOR_VERSION_V1);
        writer.reserved(3);
        write_certificate_ids(&mut writer, self);
        writer.id(self.transcript);
        writer.u128(self.low);
        writer.u128(self.high);
        writer.u64(self.coordinate_count);
        writer.u64(self.payout.denominator);
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            writer.u64(self.payout.weights[index]);
            index += 1;
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&CERTIFICATE_MAGIC_V1)?;
        if reader.u16() != SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        let outcome_count = reader.u8();
        if reader.u8() != QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1
            || reader.u8() != BASIS_EVALUATOR_VERSION_V1
        {
            return Err(Error::BadVersion);
        }
        reader.reserved(3)?;
        let market_instance_id = MarketInstanceV2Id::from_bytes(reader.id().bytes());
        let product_template_id = ProductTemplateId::from_bytes(reader.id().bytes());
        let market_genesis_profile_id = MarketGenesisProfileV2Id::from_bytes(reader.id().bytes());
        let native_claim_basis_id = NativeClaimBasisId::from_bytes(reader.id().bytes());
        let source_occurrence_id = SourceOccurrenceV1Id::from_bytes(reader.id().bytes());
        let source_interval_id = reader.id();
        let price_measure_policy_id = PriceMeasurePolicyV1Id::from_bytes(reader.id().bytes());
        let capability_profile_id = reader.id();
        let interval_profile_id =
            QuantizedIntervalConsensusProfileV1Id::from_bytes(reader.id().bytes());
        let evaluator_release_id = reader.id();
        let rounding_policy_id = reader.id();
        let transcript = reader.id();
        let low = reader.u128();
        let high = reader.u128();
        let coordinate_count = reader.u64();
        let denominator = reader.u64();
        let mut weights = [0; MAX_OUTCOMES];
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            weights[index] = reader.u64();
            index += 1;
        }
        reader.finish()?;
        let value = Self {
            market_instance_id,
            product_template_id,
            market_genesis_profile_id,
            native_claim_basis_id,
            source_occurrence_id,
            source_interval_id,
            price_measure_policy_id,
            capability_profile_id,
            interval_profile_id,
            evaluator_release_id,
            rounding_policy_id,
            transcript,
            low,
            high,
            coordinate_count,
            payout: WeightVector {
                active_len: outcome_count,
                denominator,
                weights,
            },
        };
        value.validate()?;
        Ok(value)
    }
}

/// Private-field algebraic payout capability from one checked session history.
///
/// It proves exhaustive equality relative to the exact canonical bodies named
/// by the certificate. It does not authenticate those bodies or a Solana
/// account; the disabled adapter seam must do so before live resolution.
#[derive(Clone, Copy, Debug)]
pub struct VerifiedQuantizedIntervalPayoutV1 {
    certificate: QuantizedIntervalConsensusCertificateV1,
}

impl VerifiedQuantizedIntervalPayoutV1 {
    /// Exact exhaustive certificate carried by this private capability.
    pub const fn certificate(&self) -> QuantizedIntervalConsensusCertificateV1 {
        self.certificate
    }

    /// Exact payout vector common to every integer coordinate in the interval.
    pub const fn payout(&self) -> WeightVector {
        self.certificate.payout
    }
}

fn validate_context(
    context: QuantizedIntervalConsensusContextV1<'_>,
) -> Result<ValidatedContextV1> {
    context.work_profile.validate()?;
    context.market.validate_bindings(
        context.product_template,
        context.native_claim_basis,
        context.price_measure_policy,
        context.market_genesis,
    )?;
    if context.native_claim_basis.basis_degree == 0
        || context.work_profile.capability_profile_id
            != context.market_genesis.capability_profile_id
    {
        return Err(Error::UnsupportedCapability);
    }
    let projected = context.price_measure_policy.project_smooth_basis(
        context.native_claim_basis,
        context.market_genesis,
        context.resolved_edge_policy,
    )?;
    let evaluator = projected.validated().map_err(map_basis_error)?;

    context
        .source_interval
        .validate_against(
            context.statistic_key,
            context.summary_program,
            context.window_seal,
            context.window,
        )
        .map_err(map_source_error)?;
    let (low, high) = context
        .source_interval
        .terminal_interval()
        .map_err(map_source_error)?;
    if low < context.market_genesis.coordinate_domain_min
        || high > context.market_genesis.coordinate_domain_max
    {
        return Err(Error::MismatchedArtifact);
    }

    let market_instance_id = context.market.id()?;
    let product_template_id = context.product_template.id()?;
    let market_genesis_profile_id = context.market_genesis.id()?;
    let native_claim_basis_id = context.native_claim_basis.id()?;
    let source_occurrence_id = context.source_occurrence.id()?;
    let source_interval_id =
        local_source_id(context.source_interval.id().map_err(map_source_error)?);
    let price_measure_policy_id = context.price_measure_policy.id()?;
    let interval_profile_id = context.work_profile.id()?;
    let source_window_id = local_source_id(context.window.id().map_err(map_source_error)?);
    let statistic_key_id = local_source_id(context.statistic_key.id().map_err(map_source_error)?);
    let summary_program_id =
        local_source_id(context.summary_program.id().map_err(map_source_error)?);
    if context.source_occurrence.market_instance_id != market_instance_id
        || context.source_occurrence.source_window_id != source_window_id
        || context.source_occurrence.statistic_key_id != statistic_key_id
        || local_source_id(context.window.source_spec_id) != context.product_template.source_spec_id
        || local_source_id(context.window.source_plane_program_id)
            != context.product_template.source_plane_contract_id
        || summary_program_id != context.product_template.summary_program_id
    {
        return Err(Error::MismatchedArtifact);
    }

    let width = high.checked_sub(low).ok_or(Error::InvalidParameter)?;
    if width > u128::from(context.work_profile.maximum_interval_width) {
        return Err(Error::IntervalTooWide);
    }
    let total_coordinates = inclusive_coordinate_count(low, high)?;
    Ok(ValidatedContextV1 {
        market_instance_id,
        product_template_id,
        market_genesis_profile_id,
        native_claim_basis_id,
        source_occurrence_id,
        source_interval_id,
        price_measure_policy_id,
        capability_profile_id: context.market_genesis.capability_profile_id,
        interval_profile_id,
        evaluator_release_id: context.price_measure_policy.checker_release_id,
        rounding_policy_id: quantized_interval_rounding_policy_id_v1(),
        low,
        high,
        total_coordinates,
        maximum_interval_width: context.work_profile.maximum_interval_width,
        maximum_coordinates_per_advance: context.work_profile.maximum_coordinates_per_advance,
        outcome_count: context.native_claim_basis.outcome_count,
        denominator: context.native_claim_basis.denominator,
        evaluator,
    })
}

fn advance_work(
    work: QuantizedIntervalConsensusWorkV1,
    context: ValidatedContextV1,
    requested_coordinates: u16,
) -> Result<(
    QuantizedIntervalConsensusWorkV1,
    QuantizedIntervalConsensusProgressV1,
)> {
    work.validate_against(context)?;
    if work.is_complete() {
        return Err(Error::WorkAlreadyComplete);
    }
    if requested_coordinates == 0 || requested_coordinates > context.maximum_coordinates_per_advance
    {
        return Err(Error::WorkLimitExceeded);
    }
    let remaining = context
        .total_coordinates
        .checked_sub(work.checked_coordinates)
        .ok_or(Error::WorkStateMismatch)?;
    let step_count = core::cmp::min(u64::from(requested_coordinates), remaining);
    let mut next = work;
    let mut processed = 0_u64;
    while processed < step_count {
        let offset = next
            .checked_coordinates
            .checked_add(processed)
            .ok_or(Error::ArithmeticOverflow)?;
        let coordinate = next
            .low
            .checked_add(u128::from(offset))
            .ok_or(Error::ArithmeticOverflow)?;
        let payout = context
            .evaluator
            .evaluate_point(coordinate)
            .map_err(map_basis_error)?;
        if offset == 0 {
            next.weights = payout.weights;
        } else if payout.active_len != next.outcome_count
            || payout.denominator != next.denominator
            || payout.weights != next.weights
        {
            return Err(Error::IntervalPayoutDisagreement);
        }
        next.transcript = step_transcript(next.transcript, coordinate, payout);
        processed = processed.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    next.checked_coordinates = next
        .checked_coordinates
        .checked_add(processed)
        .ok_or(Error::ArithmeticOverflow)?;
    if next.checked_coordinates == context.total_coordinates {
        next.status = WORK_STATUS_COMPLETE_V1;
    }
    next.validate_shape()?;
    let processed_coordinates = u16::try_from(processed).map_err(|_| Error::ArithmeticOverflow)?;
    Ok((
        next,
        QuantizedIntervalConsensusProgressV1 {
            processed_coordinates,
            checked_coordinates: next.checked_coordinates,
            total_coordinates: context.total_coordinates,
            is_complete: next.is_complete(),
            transcript: next.transcript,
        },
    ))
}

fn inclusive_coordinate_count(low: u128, high: u128) -> Result<u64> {
    let width = high.checked_sub(low).ok_or(Error::InvalidParameter)?;
    let count = width.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    u64::try_from(count).map_err(|_| Error::IntervalTooWide)
}

fn initial_transcript(work: &QuantizedIntervalConsensusWorkV1) -> ContentId {
    let mut hasher = Sha256::new();
    hasher.update(TRANSCRIPT_INITIAL_DOMAIN_V1);
    for id in work_ids(work) {
        hasher.update(id.bytes());
    }
    hasher.update(work.low.to_le_bytes());
    hasher.update(work.high.to_le_bytes());
    hasher.update(work.maximum_interval_width.to_le_bytes());
    hasher.update(work.maximum_coordinates_per_advance.to_le_bytes());
    hasher.update([work.outcome_count]);
    hasher.update(work.denominator.to_le_bytes());
    hasher.update([QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1]);
    hasher.update([BASIS_EVALUATOR_VERSION_V1]);
    ContentId::from_bytes(hasher.finalize().into())
}

fn step_transcript(previous: ContentId, coordinate: u128, payout: WeightVector) -> ContentId {
    let mut hasher = Sha256::new();
    hasher.update(TRANSCRIPT_STEP_DOMAIN_V1);
    hasher.update(previous.bytes());
    hasher.update(coordinate.to_le_bytes());
    hasher.update([payout.active_len]);
    hasher.update(payout.denominator.to_le_bytes());
    let mut index = 0_usize;
    while index < MAX_OUTCOMES {
        hasher.update(payout.weights[index].to_le_bytes());
        index += 1;
    }
    ContentId::from_bytes(hasher.finalize().into())
}

fn work_ids(work: &QuantizedIntervalConsensusWorkV1) -> [ContentId; 11] {
    [
        work.market_instance_id.content_id(),
        work.product_template_id.content_id(),
        work.market_genesis_profile_id.content_id(),
        work.native_claim_basis_id.content_id(),
        work.source_occurrence_id.content_id(),
        work.source_interval_id,
        work.price_measure_policy_id.content_id(),
        work.capability_profile_id,
        work.interval_profile_id.content_id(),
        work.evaluator_release_id,
        work.rounding_policy_id,
    ]
}

fn write_work_ids(writer: &mut Writer<'_>, work: &QuantizedIntervalConsensusWorkV1) {
    for id in work_ids(work) {
        writer.id(id);
    }
}

fn write_certificate_ids(
    writer: &mut Writer<'_>,
    certificate: &QuantizedIntervalConsensusCertificateV1,
) {
    for id in [
        certificate.market_instance_id.content_id(),
        certificate.product_template_id.content_id(),
        certificate.market_genesis_profile_id.content_id(),
        certificate.native_claim_basis_id.content_id(),
        certificate.source_occurrence_id.content_id(),
        certificate.source_interval_id,
        certificate.price_measure_policy_id.content_id(),
        certificate.capability_profile_id,
        certificate.interval_profile_id.content_id(),
        certificate.evaluator_release_id,
        certificate.rounding_policy_id,
    ] {
        writer.id(id);
    }
}

fn local_source_id(value: clutch_source_plane_v3::ContentId) -> ContentId {
    ContentId::from_bytes(value.bytes())
}

fn map_basis_error(error: BasisError) -> Error {
    match error {
        BasisError::ArithmeticOverflow | BasisError::ArithmeticBound => Error::ArithmeticOverflow,
        BasisError::ValueOutOfRange => Error::MismatchedArtifact,
        BasisError::InvalidOutcomeCount
        | BasisError::InvalidDegree
        | BasisError::InvalidDenominator
        | BasisError::InvalidKnotCount
        | BasisError::InvalidKnot
        | BasisError::NonCanonicalPadding
        | BasisError::UniformSpacingMismatch
        | BasisError::UniformSpacingRequired
        | BasisError::InvalidEdgePolicy
        | BasisError::InvalidWeights => Error::UnsupportedCapability,
    }
}

fn map_source_error(error: SourceError) -> Error {
    match error {
        SourceError::Truncated => Error::Truncated,
        SourceError::TrailingBytes => Error::TrailingBytes,
        SourceError::BadMagic => Error::BadMagic,
        SourceError::BadVersion => Error::BadVersion,
        SourceError::NonCanonicalReserved => Error::NonCanonicalReserved,
        SourceError::ZeroIdentity => Error::ZeroIdentity,
        SourceError::InvalidParameter => Error::InvalidParameter,
        SourceError::NonCanonicalPadding => Error::NonCanonicalPadding,
        SourceError::ArithmeticOverflow => Error::ArithmeticOverflow,
        SourceError::MismatchedArtifact => Error::MismatchedArtifact,
        SourceError::UnsupportedStatistic
        | SourceError::UnsupportedPolicy
        | SourceError::FailurePayoutNotUniform => Error::UnsupportedCapability,
        SourceError::DiscontinuousPage
        | SourceError::IncompleteWindow
        | SourceError::WindowAlreadyMature
        | SourceError::WrongOrdinal
        | SourceError::NotEligible
        | SourceError::SeriesExhausted
        | SourceError::InsufficientPrepayment => Error::MismatchedArtifact,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_bspline::{BasisSpec, EdgePolicy};

    fn id(seed: u8) -> ContentId {
        ContentId::from_bytes([seed; 32])
    }

    fn validated_context(
        low: u128,
        high: u128,
        maximum_interval_width: u64,
        maximum_coordinates_per_advance: u16,
    ) -> ValidatedContextV1 {
        let mut knots = [0; MAX_OUTCOMES];
        knots[..2].copy_from_slice(&[0, 8]);
        let evaluator = BasisSpec {
            outcome_count: 2,
            degree: 1,
            knot_count: 2,
            uniform_log2_spacing: 3,
            denominator: 2,
            domain_max: 8,
            edge_policy: EdgePolicy::Clamp,
            knots,
        }
        .validated()
        .unwrap();
        ValidatedContextV1 {
            market_instance_id: MarketInstanceV2Id::from_bytes([1; 32]),
            product_template_id: ProductTemplateId::from_bytes([2; 32]),
            market_genesis_profile_id: MarketGenesisProfileV2Id::from_bytes([3; 32]),
            native_claim_basis_id: NativeClaimBasisId::from_bytes([4; 32]),
            source_occurrence_id: SourceOccurrenceV1Id::from_bytes([5; 32]),
            source_interval_id: id(6),
            price_measure_policy_id: PriceMeasurePolicyV1Id::from_bytes([7; 32]),
            capability_profile_id: id(8),
            interval_profile_id: QuantizedIntervalConsensusProfileV1Id::from_bytes([9; 32]),
            evaluator_release_id: id(10),
            rounding_policy_id: quantized_interval_rounding_policy_id_v1(),
            low,
            high,
            total_coordinates: inclusive_coordinate_count(low, high).unwrap(),
            maximum_interval_width,
            maximum_coordinates_per_advance,
            outcome_count: 2,
            denominator: 2,
            evaluator,
        }
    }

    #[test]
    fn exhaustive_chunks_latch_one_vector_and_only_then_mint_capability() {
        let context = validated_context(0, 2, 8, 2);
        let mut session = QuantizedIntervalConsensusSessionV1 {
            work: QuantizedIntervalConsensusWorkV1::new(context).unwrap(),
        };
        assert!(matches!(
            session.verified_payout_validated(context),
            Err(Error::WorkIncomplete)
        ));
        let first = session.advance_validated(context, 2).unwrap();
        assert_eq!(first.processed_coordinates, 2);
        assert!(!first.is_complete);
        assert_eq!(session.work.checked_coordinates(), 2);
        assert!(matches!(
            session.verified_payout_validated(context),
            Err(Error::WorkIncomplete)
        ));
        let second = session.advance_validated(context, 2).unwrap();
        assert_eq!(second.processed_coordinates, 1);
        assert!(second.is_complete);
        let verified = session.verified_payout_validated(context).unwrap();
        assert_eq!(verified.payout().weights[..2], [2, 0]);
        assert_eq!(verified.certificate().interval(), (0, 2));
        assert_ne!(verified.certificate().transcript(), id(0));
    }

    #[test]
    fn exhaustive_scan_refuses_first_differing_integer_coordinate() {
        let context = validated_context(0, 3, 8, 4);
        let work = QuantizedIntervalConsensusWorkV1::new(context).unwrap();
        assert_eq!(
            advance_work(work, context, 4),
            Err(Error::IntervalPayoutDisagreement)
        );
        assert_eq!(work.checked_coordinates(), 0);
        assert!(work.latched_payout().is_none());
    }

    #[test]
    fn work_limits_are_exact_and_do_not_truncate_the_interval() {
        let context = validated_context(0, 2, 8, 2);
        let work = QuantizedIntervalConsensusWorkV1::new(context).unwrap();
        assert_eq!(
            advance_work(work, context, 0),
            Err(Error::WorkLimitExceeded)
        );
        assert_eq!(
            advance_work(work, context, 3),
            Err(Error::WorkLimitExceeded)
        );
        let too_wide = validated_context(0, 9, 8, 2);
        assert_eq!(
            QuantizedIntervalConsensusWorkV1::new(too_wide),
            Err(Error::IntervalTooWide)
        );
    }

    #[test]
    fn work_codec_refuses_hostile_status_padding_cursor_and_weights() {
        let context = validated_context(0, 2, 8, 2);
        let work = QuantizedIntervalConsensusWorkV1::new(context).unwrap();
        let mut bytes = [0; QUANTIZED_INTERVAL_CONSENSUS_WORK_BYTES_V1];
        work.encode_into(&mut bytes).unwrap();
        assert_eq!(QuantizedIntervalConsensusWorkV1::decode(&bytes), Ok(work));

        let mut reserved = bytes;
        reserved[14] = 1;
        assert_eq!(
            QuantizedIntervalConsensusWorkV1::decode(&reserved),
            Err(Error::NonCanonicalReserved)
        );
        let mut premature_complete = bytes;
        premature_complete[10] = WORK_STATUS_COMPLETE_V1;
        assert_eq!(
            QuantizedIntervalConsensusWorkV1::decode(&premature_complete),
            Err(Error::WorkStateMismatch)
        );
        let mut forged_cursor = bytes;
        forged_cursor[432..440].copy_from_slice(&4_u64.to_le_bytes());
        assert_eq!(
            QuantizedIntervalConsensusWorkV1::decode(&forged_cursor),
            Err(Error::WorkStateMismatch)
        );
        let mut premature_weight = bytes;
        premature_weight[464..472].copy_from_slice(&2_u64.to_le_bytes());
        assert_eq!(
            QuantizedIntervalConsensusWorkV1::decode(&premature_weight),
            Err(Error::NonCanonicalPadding)
        );
    }

    #[test]
    fn certificate_codec_binds_versions_rounding_and_exact_count() {
        let context = validated_context(0, 2, 8, 3);
        let work = QuantizedIntervalConsensusWorkV1::new(context).unwrap();
        let (complete, _) = advance_work(work, context, 3).unwrap();
        let certificate =
            QuantizedIntervalConsensusCertificateV1::from_complete_work(complete).unwrap();
        let mut bytes = [0; QUANTIZED_INTERVAL_CONSENSUS_CERTIFICATE_BYTES_V1];
        certificate.encode_into(&mut bytes).unwrap();
        assert_eq!(
            QuantizedIntervalConsensusCertificateV1::decode(&bytes),
            Ok(certificate)
        );
        assert!(!certificate
            .id()
            .unwrap()
            .bytes()
            .iter()
            .all(|byte| *byte == 0));

        let mut wrong_version = bytes;
        wrong_version[12] = 2;
        assert_eq!(
            QuantizedIntervalConsensusCertificateV1::decode(&wrong_version),
            Err(Error::BadVersion)
        );
        let mut reserved = bytes;
        reserved[15] = 1;
        assert_eq!(
            QuantizedIntervalConsensusCertificateV1::decode(&reserved),
            Err(Error::NonCanonicalReserved)
        );
        let mut wrong_count = bytes;
        wrong_count[432..440].copy_from_slice(&2_u64.to_le_bytes());
        assert_eq!(
            QuantizedIntervalConsensusCertificateV1::decode(&wrong_count),
            Err(Error::WorkStateMismatch)
        );
    }

    #[test]
    fn profile_is_only_a_resource_bound_and_live_capability_stays_disabled() {
        let profile = QuantizedIntervalConsensusProfileV1 {
            capability_profile_id: id(1),
            maximum_interval_width: 32,
            maximum_coordinates_per_advance: 4,
        };
        let mut bytes = [0; QUANTIZED_INTERVAL_CONSENSUS_PROFILE_BYTES_V1];
        profile.encode_into(&mut bytes).unwrap();
        assert_eq!(
            QuantizedIntervalConsensusProfileV1::decode(&bytes),
            Ok(profile)
        );
        assert_eq!(
            require_quantized_interval_consensus_runtime_capability_v1(),
            Err(Error::RuntimeCapabilityDisabled)
        );
        assert_ne!(quantized_interval_rounding_policy_id_v1(), ContentId::ZERO);
    }
}
