use clutch_price_measure::{
    DegreeZeroPayoutTableV3, PriceVectorV3, QuantizedAtomWitnessV3, PAYOUT_MAP_UNUSED_V3,
    PRICE_MEASURE_WITNESS_VERSION_V3, QUANTIZED_PRICE_MEASURE_MAX_DEGREE_V3,
    QUANTIZED_PRICE_MEASURE_MIN_DEGREE_V3, QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1,
};

use crate::codec::{Reader, Writer};
use crate::{
    content_id, AbsoluteRecoveryAttemptV1, CompiledScheduleV1, ComponentDebitV1, ContentId,
    DebitProjectionV1, Error, EvidenceOnlyRecoveryPolicyV1, FixedCodec, FundingBalancesV1,
    MarketGenesisProfileV2Id, MarketInstanceV2Id, NativeClaimBasisV1, PriceMeasurePolicyV1Id,
    ProductTemplateId, ProductTemplateV4, QuantizedBasisSpecV1, QuantizedEdgePolicyV1,
    RealmCollateralProjectionV1, Result, SeriesAttachmentPlanId, SeriesAttachmentPlanV1,
    SeriesFundingQuoteId, SeriesFundingQuoteV1, SeriesFundingTermsV2Id, SeriesPlanV5Id,
    MAX_BASIS_DEGREE, MAX_OUTCOMES, MAX_PAYOUTS, MAX_RECOVERY_ATTEMPTS,
};

const PRICE_MEASURE_POLICY_MAGIC: [u8; 8] = *b"DCPMPV1\0";
const MARKET_GENESIS_PROFILE_V2_MAGIC: [u8; 8] = *b"DCMGPV2\0";
const MARKET_INSTANCE_V2_MAGIC: [u8; 8] = *b"DCMKTIN2";
const SERIES_PLAN_V5_MAGIC: [u8; 8] = *b"DCSERIV5";
const SERIES_FUNDING_TERMS_V2_MAGIC: [u8; 8] = *b"DCFTERM2";
const SCHEMA_V1: u16 = 1;
const SCHEMA_V2: u16 = 2;

/// SHA-256 domain for [`PriceMeasurePolicyV1`].
pub const PRICE_MEASURE_POLICY_DOMAIN: &[u8] = b"dragons-clutch/price-measure-policy/v1";
/// Exact canonical byte length of [`PriceMeasurePolicyV1`].
pub const PRICE_MEASURE_POLICY_BYTES: usize = 96;
/// SHA-256 domain for [`MarketGenesisProfileV2`].
pub const MARKET_GENESIS_PROFILE_V2_DOMAIN: &[u8] = b"dragons-clutch/market-genesis-profile/v2";
/// Exact canonical byte length of [`MarketGenesisProfileV2`].
pub const MARKET_GENESIS_PROFILE_V2_BYTES: usize = 416;
/// SHA-256 domain for [`MarketInstancePreimageV2`].
pub const MARKET_INSTANCE_V2_DOMAIN: &[u8] = b"dragons-clutch/market-instance/v2";
/// Exact canonical byte length of [`MarketInstancePreimageV2`].
pub const MARKET_INSTANCE_PREIMAGE_V2_BYTES: usize = 88;
/// SHA-256 domain for [`SeriesPlanV5`].
pub const SERIES_PLAN_V5_DOMAIN: &[u8] = b"dragons-clutch/series-plan/v5";
/// Exact canonical byte length of [`SeriesPlanV5`].
pub const SERIES_PLAN_V5_BYTES: usize = 152;
/// SHA-256 domain for [`SeriesFundingTermsV2`].
pub const SERIES_FUNDING_TERMS_V2_DOMAIN: &[u8] = b"dragons-clutch/series-funding-terms/v2";
/// Exact canonical byte length of [`SeriesFundingTermsV2`].
pub const SERIES_FUNDING_TERMS_V2_BYTES: usize = 208;

const _: () = assert!(MAX_OUTCOMES == clutch_price_measure::MAX_OUTCOMES);
const _: () = assert!(MAX_PAYOUTS == clutch_price_measure::MAX_OUTCOMES);
const _: () = assert!(crate::PAYOUT_MAP_UNUSED == PAYOUT_MAP_UNUSED_V3);
const _: () = assert!(PRICE_MEASURE_POLICY_BYTES == 96);
const _: () = assert!(MARKET_GENESIS_PROFILE_V2_BYTES == 416);
const _: () = assert!(MARKET_INSTANCE_PREIMAGE_V2_BYTES == 88);
const _: () = assert!(SERIES_PLAN_V5_BYTES == 152);
const _: () = assert!(SERIES_FUNDING_TERMS_V2_BYTES == 208);

/// Immutable selection of the only price-measure semantics compatible with
/// today's quantized settlement payouts for basis degrees zero through three.
///
/// This artifact deliberately admits only the V3 quantized atom checker. A
/// future continuous-price profile needs a distinct policy schema and identity;
/// it cannot be represented by changing fields in this body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceMeasurePolicyV1 {
    /// Exact reviewed release that owns the price-measure checker.
    pub checker_release_id: ContentId,
    /// Exact checker/witness interface version; currently three.
    pub checker_version: u8,
    /// Exact shared quantized evaluator and reconstruction semantics version.
    pub quantized_semantics_version: u8,
    /// Smallest admitted B-spline degree; exactly zero in this schema.
    pub minimum_basis_degree: u8,
    /// Largest admitted B-spline degree; exactly three in this schema.
    pub maximum_basis_degree: u8,
    /// Maximum active outcome width, at most sixteen.
    pub maximum_outcome_count: u8,
    /// Maximum active atom count; exactly the selected outcome bound.
    pub maximum_atom_count: u8,
    /// Largest admitted immutable payout denominator.
    pub maximum_payout_denominator: u64,
    /// Largest admitted primitive atom-mass denominator.
    pub maximum_witness_denominator: u64,
    /// Largest admitted exact integer price-simplex scale.
    pub maximum_price_scale: u64,
}

impl PriceMeasurePolicyV1 {
    /// Validate the exact production quantized checker and its bounded domain.
    pub fn validate(&self) -> Result<()> {
        self.checker_release_id.validate()?;
        if self.checker_version != PRICE_MEASURE_WITNESS_VERSION_V3
            || self.quantized_semantics_version != QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1
            || self.minimum_basis_degree != QUANTIZED_PRICE_MEASURE_MIN_DEGREE_V3
            || self.maximum_basis_degree != QUANTIZED_PRICE_MEASURE_MAX_DEGREE_V3
            || self.maximum_outcome_count < 2
            || usize::from(self.maximum_outcome_count) > MAX_OUTCOMES
            || self.maximum_atom_count != self.maximum_outcome_count
            || self.maximum_payout_denominator == 0
            || self.maximum_witness_denominator == 0
            || self.maximum_price_scale == 0
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Check one exact Product basis against the frozen checker domain.
    pub fn validate_basis(&self, basis: &NativeClaimBasisV1) -> Result<()> {
        self.validate()?;
        basis.validate()?;
        if basis.basis_degree < self.minimum_basis_degree
            || basis.basis_degree > self.maximum_basis_degree
            || basis.outcome_count > self.maximum_outcome_count
            || basis.denominator > self.maximum_payout_denominator
        {
            return Err(Error::UnsupportedCapability);
        }
        Ok(())
    }

    /// Check one exact candidate price vector against Product width and an
    /// adapter-authenticated venue-grid scale.
    ///
    /// A PriceGrid owns a scalar tick lattice, not an outcome width. The venue
    /// adapter must separately prove that every active candidate component is
    /// an admitted grid tick and derive the candidate-price digest from the
    /// canonical exact-price transcript. This pure check owns only simplex
    /// shape and equality with the authenticated grid scale.
    pub fn validate_candidate_price_contract(
        &self,
        basis: &NativeClaimBasisV1,
        prices: &PriceVectorV3,
        authenticated_price_grid_scale: u64,
    ) -> Result<()> {
        self.validate_basis(basis)?;
        if prices.basis_degree != basis.basis_degree
            || prices.native_outcome_count != basis.outcome_count
            || prices.price_scale == 0
            || prices.price_scale != authenticated_price_grid_scale
            || prices.price_scale > self.maximum_price_scale
        {
            return Err(Error::UnsupportedCapability);
        }
        let active = usize::from(prices.native_outcome_count);
        let mut sum = 0_u128;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            let price = prices.prices[index];
            if index < active {
                if price > prices.price_scale {
                    return Err(Error::InvalidParameter);
                }
                sum = sum
                    .checked_add(u128::from(price))
                    .ok_or(Error::ArithmeticOverflow)?;
            } else if price != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        if sum != u128::from(prices.price_scale) {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Check the policy-owned atom and primitive-denominator bounds before the
    /// V3 arithmetic checker consumes a candidate witness.
    ///
    /// This does not replace full V3 witness verification. It makes every
    /// bound committed by this policy behaviorally effective at the adapter
    /// join instead of leaving an identity-changing dead field.
    pub fn validate_witness_contract(
        &self,
        basis: &NativeClaimBasisV1,
        prices: &PriceVectorV3,
        witness: &QuantizedAtomWitnessV3,
        authenticated_price_grid_scale: u64,
    ) -> Result<()> {
        self.validate_candidate_price_contract(basis, prices, authenticated_price_grid_scale)?;
        if witness.schema_version != self.checker_version
            || witness.quantized_semantics_version != self.quantized_semantics_version
            || witness.basis_degree != basis.basis_degree
            || witness.native_outcome_count != basis.outcome_count
            || witness.atom_count == 0
            || witness.atom_count > basis.outcome_count
            || witness.atom_count > self.maximum_atom_count
            || witness.common_denominator == 0
            || witness.common_denominator > self.maximum_witness_denominator
        {
            return Err(Error::UnsupportedCapability);
        }
        Ok(())
    }

    /// Project a canonical degree-zero Product basis into the V3 finite payout
    /// table using the coordinate bounds committed by an exact Genesis V2.
    ///
    /// This method validates and copies the Product-owned rows, map, knots, and
    /// denominator without reinterpretation. The resulting table is ephemeral:
    /// `NativeClaimBasisV1Id` owns its payout
    /// body and ambiguity/edge registry selectors, while Genesis V2 owns the
    /// coordinate bounds. A live adapter still must authenticate both bodies
    /// and the registry mapping selected by the edge selector.
    pub fn project_degree_zero_table(
        &self,
        basis: &NativeClaimBasisV1,
        genesis: &MarketGenesisProfileV2,
    ) -> Result<DegreeZeroPayoutTableV3> {
        genesis.validate_bindings(basis, self)?;
        if basis.basis_degree != 0 {
            return Err(Error::UnsupportedCapability);
        }
        let table = DegreeZeroPayoutTableV3 {
            native_outcome_count: basis.outcome_count,
            payout_count: basis.payout_count,
            knot_count: basis.knot_count,
            payout_denominator: basis.denominator,
            domain_min: genesis.coordinate_domain_min,
            domain_max: genesis.coordinate_domain_max,
            payout_weights: basis.payout_weights,
            payout_map: basis.payout_map,
            knots: basis.knots,
        };
        table.validate().map_err(|_| Error::UnsupportedCapability)?;
        Ok(table)
    }

    /// Project the exact Product-owned smooth basis into the V3 evaluator using
    /// Genesis-owned coordinate bounds and an adapter-authenticated resolution
    /// of the Product-owned edge-policy selector.
    ///
    /// The caller must derive `authenticated_edge_policy` from the immutable
    /// registry body named by `basis.edge_policy_registry_value`; this pure core
    /// cannot authenticate that registry account by itself.
    pub fn project_smooth_basis(
        &self,
        basis: &NativeClaimBasisV1,
        genesis: &MarketGenesisProfileV2,
        authenticated_edge_policy: QuantizedEdgePolicyV1,
    ) -> Result<QuantizedBasisSpecV1> {
        genesis.validate_partition_bindings(basis, self, authenticated_edge_policy)?;
        if basis.basis_degree == 0 {
            return Err(Error::UnsupportedCapability);
        }
        let projected = QuantizedBasisSpecV1 {
            outcome_count: basis.outcome_count,
            degree: basis.basis_degree,
            knot_count: basis.knot_count,
            uniform_log2_spacing: basis.uniform_log2_spacing,
            denominator: basis.denominator,
            domain_max: genesis.coordinate_domain_max,
            edge_policy: authenticated_edge_policy,
            knots: basis.knots,
        };
        projected
            .validate()
            .map_err(|_| Error::UnsupportedCapability)?;
        Ok(projected)
    }

    /// Typed identity of this exact quantized price-measure contract.
    pub fn id(&self) -> Result<PriceMeasurePolicyV1Id> {
        let mut body = [0; PRICE_MEASURE_POLICY_BYTES];
        self.encode_into(&mut body)?;
        Ok(PriceMeasurePolicyV1Id::from_bytes(
            content_id(PRICE_MEASURE_POLICY_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for PriceMeasurePolicyV1 {
    const ENCODED_LEN: usize = PRICE_MEASURE_POLICY_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&PRICE_MEASURE_POLICY_MAGIC);
        writer.u16(SCHEMA_V1);
        writer.reserved(6);
        writer.id(self.checker_release_id);
        writer.u8(self.checker_version);
        writer.u8(self.quantized_semantics_version);
        writer.u8(self.minimum_basis_degree);
        writer.u8(self.maximum_basis_degree);
        writer.u8(self.maximum_outcome_count);
        writer.u8(self.maximum_atom_count);
        writer.reserved(10);
        writer.u64(self.maximum_payout_denominator);
        writer.u64(self.maximum_witness_denominator);
        writer.u64(self.maximum_price_scale);
        writer.reserved(8);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&PRICE_MEASURE_POLICY_MAGIC)?;
        if reader.u16() != SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            checker_release_id: reader.id(),
            checker_version: reader.u8(),
            quantized_semantics_version: reader.u8(),
            minimum_basis_degree: reader.u8(),
            maximum_basis_degree: reader.u8(),
            maximum_outcome_count: reader.u8(),
            maximum_atom_count: reader.u8(),
            maximum_payout_denominator: {
                reader.reserved(10)?;
                reader.u64()
            },
            maximum_witness_denominator: reader.u64(),
            maximum_price_scale: reader.u64(),
        };
        reader.reserved(8)?;
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Immutable Realm/profile and venue semantics with one exact price-measure
/// policy owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketGenesisProfileV2 {
    /// Immutable Realm identity.
    pub realm_id: ContentId,
    /// Immutable Profile identity selected by the Realm.
    pub profile_id: ContentId,
    /// Exact order price-grid identity.
    pub price_grid_id: ContentId,
    /// Exact quantized price-measure policy identity.
    pub price_measure_policy_id: PriceMeasurePolicyV1Id,
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
    /// Inclusive lower coordinate bound for every resolution in this market.
    pub coordinate_domain_min: u128,
    /// Inclusive upper coordinate bound for every resolution in this market.
    pub coordinate_domain_max: u128,
}

impl MarketGenesisProfileV2 {
    /// Validate exact local shape without authenticating referenced bodies.
    pub fn validate_shape(&self) -> Result<()> {
        for id in [
            self.realm_id,
            self.profile_id,
            self.price_grid_id,
            self.price_measure_policy_id.content_id(),
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
        if self.terminal_disposition_registry_value == 0
            || self.native_bearer_lot == 0
            || self.coordinate_domain_min >= self.coordinate_domain_max
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Join the exact quantized price policy and native denominator to this
    /// Genesis body.
    pub fn validate_bindings(
        &self,
        basis: &NativeClaimBasisV1,
        price_policy: &PriceMeasurePolicyV1,
    ) -> Result<()> {
        self.validate_shape()?;
        price_policy.validate_basis(basis)?;
        let first_knot = basis.knots[0];
        let last_knot = basis.knots[usize::from(basis.knot_count) - 1];
        if self.price_measure_policy_id != price_policy.id()?
            || !self.native_bearer_lot.is_multiple_of(basis.denominator)
            || (basis.basis_degree == 0 && first_knot <= self.coordinate_domain_min)
            || (basis.basis_degree != 0 && first_knot < self.coordinate_domain_min)
            || last_knot > self.coordinate_domain_max
        {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }

    /// Join the basis, price policy, and registry-resolved edge behavior while
    /// proving that every coordinate in this market has a smooth payout.
    ///
    /// A refusing evaluator is total over the Genesis domain only when that
    /// domain is exactly the inclusive stored-knot span. Clamping may cover a
    /// wider Genesis domain because every exterior point maps to an endpoint.
    pub fn validate_partition_bindings(
        &self,
        basis: &NativeClaimBasisV1,
        price_policy: &PriceMeasurePolicyV1,
        resolved_edge_policy: QuantizedEdgePolicyV1,
    ) -> Result<()> {
        self.validate_bindings(basis, price_policy)?;
        if basis.basis_degree != 0
            && resolved_edge_policy == QuantizedEdgePolicyV1::Refuse
            && (self.coordinate_domain_min != basis.knots[0]
                || self.coordinate_domain_max != basis.knots[usize::from(basis.knot_count) - 1])
        {
            return Err(Error::UnsupportedCapability);
        }
        Ok(())
    }

    /// Typed identity of these exact V2 market-genesis semantics.
    pub fn id(&self) -> Result<MarketGenesisProfileV2Id> {
        let mut body = [0; MARKET_GENESIS_PROFILE_V2_BYTES];
        self.encode_into(&mut body)?;
        Ok(MarketGenesisProfileV2Id::from_bytes(
            content_id(MARKET_GENESIS_PROFILE_V2_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for MarketGenesisProfileV2 {
    const ENCODED_LEN: usize = MARKET_GENESIS_PROFILE_V2_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MARKET_GENESIS_PROFILE_V2_MAGIC);
        writer.u16(SCHEMA_V2);
        writer.u16(self.terminal_disposition_registry_value);
        writer.reserved(4);
        for id in [
            self.realm_id,
            self.profile_id,
            self.price_grid_id,
            self.price_measure_policy_id.content_id(),
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
        writer.u128(self.coordinate_domain_min);
        writer.u128(self.coordinate_domain_max);
        writer.reserved(8);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&MARKET_GENESIS_PROFILE_V2_MAGIC)?;
        if reader.u16() != SCHEMA_V2 {
            return Err(Error::BadVersion);
        }
        let terminal_disposition_registry_value = reader.u16();
        reader.reserved(4)?;
        let value = Self {
            realm_id: reader.id(),
            profile_id: reader.id(),
            price_grid_id: reader.id(),
            price_measure_policy_id: PriceMeasurePolicyV1Id::from_bytes(reader.id().bytes()),
            fee_policy_id: reader.id(),
            relation_policy_id: reader.id(),
            score_policy_id: reader.id(),
            candidate_lifecycle_policy_id: reader.id(),
            candidate_liveness_policy_id: reader.id(),
            retirement_policy_id: reader.id(),
            capability_profile_id: reader.id(),
            terminal_disposition_registry_value,
            native_bearer_lot: reader.u64(),
            coordinate_domain_min: reader.u128(),
            coordinate_domain_max: reader.u128(),
        };
        reader.reserved(8)?;
        reader.finish()?;
        value.validate_shape()?;
        Ok(value)
    }
}

/// V2 economic market identity preimage, excluding Series funding and
/// operational attachments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketInstancePreimageV2 {
    /// Reusable product semantics.
    pub product_template_id: ProductTemplateId,
    /// Immutable V2 Realm/profile/venue/price semantics.
    pub market_genesis_profile_id: MarketGenesisProfileV2Id,
    /// Absolute first observation bucket.
    pub start_bucket: u64,
    /// Market-local liability cap in collateral atoms.
    pub collateral_cap: u64,
}

impl MarketInstancePreimageV2 {
    /// Validate exact local shape.
    pub fn validate(&self) -> Result<()> {
        self.product_template_id.validate()?;
        self.market_genesis_profile_id.validate()?;
        if self.collateral_cap == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Join this preimage to the exact Template, Genesis, basis, and price
    /// policy bodies.
    pub fn validate_bindings(
        &self,
        template: &ProductTemplateV4,
        basis: &NativeClaimBasisV1,
        price_policy: &PriceMeasurePolicyV1,
        genesis: &MarketGenesisProfileV2,
    ) -> Result<()> {
        self.validate()?;
        genesis.validate_bindings(basis, price_policy)?;
        if self.product_template_id != template.id()?
            || template.native_claim_basis_id != basis.id()?
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

    /// Full-width V2 economic market identity.
    pub fn id(&self) -> Result<MarketInstanceV2Id> {
        let mut body = [0; MARKET_INSTANCE_PREIMAGE_V2_BYTES];
        self.encode_into(&mut body)?;
        Ok(MarketInstanceV2Id::from_bytes(
            content_id(MARKET_INSTANCE_V2_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for MarketInstancePreimageV2 {
    const ENCODED_LEN: usize = MARKET_INSTANCE_PREIMAGE_V2_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MARKET_INSTANCE_V2_MAGIC);
        writer.id(self.product_template_id.content_id());
        writer.id(self.market_genesis_profile_id.content_id());
        writer.u64(self.start_bucket);
        writer.u64(self.collateral_cap);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&MARKET_INSTANCE_V2_MAGIC)?;
        let value = Self {
            product_template_id: ProductTemplateId::from_bytes(reader.id().bytes()),
            market_genesis_profile_id: MarketGenesisProfileV2Id::from_bytes(reader.id().bytes()),
            start_bucket: reader.u64(),
            collateral_cap: reader.u64(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Immutable finite recurring schedule using the V2 price-owning Genesis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesPlanV5 {
    /// Reusable relative product semantics.
    pub product_template_id: ProductTemplateId,
    /// Immutable V2 Realm/profile/venue/price semantics.
    pub market_genesis_profile_id: MarketGenesisProfileV2Id,
    /// Operational attachment choices excluded from market identity.
    pub attachment_plan_id: SeriesAttachmentPlanId,
    /// First absolute observation bucket.
    pub first_start_bucket: u64,
    /// Zero for a singleton Series; positive between multiple ordinals.
    pub stride_buckets: u64,
    /// Finite nonzero occurrence count.
    pub instance_count: u32,
    /// Buckets before start during which creation is eligible.
    pub creation_lead_buckets: u64,
    /// Per-market liability cap in collateral atoms.
    pub market_collateral_cap: u64,
}

impl SeriesPlanV5 {
    /// Validate local references and finite recurrence shape.
    pub fn validate_shape(&self) -> Result<()> {
        self.product_template_id.validate()?;
        self.market_genesis_profile_id.validate()?;
        self.attachment_plan_id.validate()?;
        if self.instance_count == 0
            || (self.instance_count == 1 && self.stride_buckets != 0)
            || (self.instance_count > 1 && self.stride_buckets == 0)
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

    /// Validate exact successor market-core bodies, structural attachment, and
    /// the complete registry capability join.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_bindings(
        &self,
        template: &ProductTemplateV4,
        basis: &NativeClaimBasisV1,
        recovery: &EvidenceOnlyRecoveryPolicyV1,
        price_policy: &PriceMeasurePolicyV1,
        genesis: &MarketGenesisProfileV2,
        attachment: &SeriesAttachmentPlanV1,
        registry: &RegistryCapabilityProjectionV2,
    ) -> Result<()> {
        self.validate_shape()?;
        template.validate_bindings(basis, recovery)?;
        registry.validate_complete_join(self, template, basis, recovery, price_policy, genesis)?;
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

    /// Typed identity of this V5 finite schedule and attachment choice.
    pub fn id(&self) -> Result<SeriesPlanV5Id> {
        let mut body = [0; SERIES_PLAN_V5_BYTES];
        self.encode_into(&mut body)?;
        Ok(SeriesPlanV5Id::from_bytes(
            content_id(SERIES_PLAN_V5_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for SeriesPlanV5 {
    const ENCODED_LEN: usize = SERIES_PLAN_V5_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&SERIES_PLAN_V5_MAGIC);
        writer.u16(SCHEMA_V2);
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
        reader.magic(&SERIES_PLAN_V5_MAGIC)?;
        if reader.u16() != SCHEMA_V2 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            product_template_id: ProductTemplateId::from_bytes(reader.id().bytes()),
            market_genesis_profile_id: MarketGenesisProfileV2Id::from_bytes(reader.id().bytes()),
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

/// Exact semantic-owner identities admitted by one successor capability
/// profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilitySemanticOwnersV2 {
    /// Exact recurring source-plane contract or release.
    pub source_plane_contract_id: ContentId,
    /// Exact source description.
    pub source_spec_id: ContentId,
    /// Exact source-neutral summary program.
    pub summary_program_id: ContentId,
    /// Exact native claim basis.
    pub native_claim_basis_id: crate::NativeClaimBasisId,
    /// Exact evidence-only recovery policy.
    pub evidence_only_recovery_policy_id: crate::EvidenceOnlyRecoveryPolicyId,
    /// Exact product compiler release.
    pub product_compiler_release_id: ContentId,
    /// Exact order price-grid semantics.
    pub price_grid_id: ContentId,
    /// Exact quantized price-measure policy.
    pub price_measure_policy_id: PriceMeasurePolicyV1Id,
    /// Exact fee semantics.
    pub fee_policy_id: ContentId,
    /// Exact settlement/evidence relation semantics.
    pub relation_policy_id: ContentId,
    /// Exact score semantics.
    pub score_policy_id: ContentId,
    /// Exact candidate lifecycle semantics.
    pub candidate_lifecycle_policy_id: ContentId,
    /// Exact candidate liveness semantics.
    pub candidate_liveness_policy_id: ContentId,
    /// Exact counted-retirement semantics.
    pub retirement_policy_id: ContentId,
}

impl CapabilitySemanticOwnersV2 {
    fn validate(self) -> Result<()> {
        for id in [
            self.source_plane_contract_id,
            self.source_spec_id,
            self.summary_program_id,
            self.native_claim_basis_id.content_id(),
            self.evidence_only_recovery_policy_id.content_id(),
            self.product_compiler_release_id,
            self.price_grid_id,
            self.price_measure_policy_id.content_id(),
            self.fee_policy_id,
            self.relation_policy_id,
            self.score_policy_id,
            self.candidate_lifecycle_policy_id,
            self.candidate_liveness_policy_id,
            self.retirement_policy_id,
        ] {
            id.validate()?;
        }
        Ok(())
    }
}

/// Complete successor market-core projection of one registry capability
/// profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryCapabilityProjectionV2 {
    /// Exact central registry release authenticated by the adapter.
    pub registry_release_id: ContentId,
    /// Exact capability profile selected by the market GenesisProfile.
    pub capability_profile_id: ContentId,
    /// Exact admitted statistic registry value.
    pub statistic_registry_value: u16,
    /// Exact admitted coverage-policy registry value.
    pub coverage_policy_registry_value: u16,
    /// Exact admitted ambiguity-policy registry value.
    pub ambiguity_policy_registry_value: u8,
    /// Exact admitted edge-policy registry value.
    pub edge_policy_registry_value: u8,
    /// Exact registry-owned terminal BURN disposition value.
    pub burn_terminal_disposition_registry_value: u16,
    /// Registry-resolved behavior named by the basis edge selector.
    ///
    /// This public projection is forgeable; the live adapter must derive the
    /// enum from the authenticated registry mapping before using this join.
    pub resolved_edge_policy: QuantizedEdgePolicyV1,
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
    /// Exact admitted semantic-owner identities, including price measure.
    pub semantic_owners: CapabilitySemanticOwnersV2,
    /// Exact immutable Realm/Profile collateral projection.
    pub realm_collateral: RealmCollateralProjectionV1,
}

impl RegistryCapabilityProjectionV2 {
    fn validate_shape(self) -> Result<()> {
        self.registry_release_id.validate()?;
        self.capability_profile_id.validate()?;
        self.semantic_owners.validate()?;
        validate_realm_collateral(self.realm_collateral)?;
        let hard_max_outcomes = u8::try_from(MAX_OUTCOMES).map_err(|_| Error::InvalidParameter)?;
        let hard_max_payouts = u8::try_from(MAX_PAYOUTS).map_err(|_| Error::InvalidParameter)?;
        let hard_max_attempts =
            u8::try_from(MAX_RECOVERY_ATTEMPTS).map_err(|_| Error::InvalidParameter)?;
        if self.statistic_registry_value == 0
            || self.coverage_policy_registry_value == 0
            || self.ambiguity_policy_registry_value == 0
            || self.edge_policy_registry_value == 0
            || self.burn_terminal_disposition_registry_value == 0
            || self.max_outcome_count == 0
            || self.max_outcome_count > hard_max_outcomes
            || self.max_degree_zero_payout_count > hard_max_payouts
            || self.max_recovery_attempt_count == 0
            || self.max_recovery_attempt_count > hard_max_attempts
            || self.min_coverage_policy_parameter > self.max_coverage_policy_parameter
            || self.max_window_span_buckets == 0
            || self.max_series_instance_count == 0
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Validate the total market-core successor join for one Series.
    ///
    /// Success proves equality with this supplied projection; it does not prove
    /// that the projection came from an authentic registry account or release.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_complete_join(
        &self,
        series: &SeriesPlanV5,
        template: &ProductTemplateV4,
        basis: &NativeClaimBasisV1,
        recovery: &EvidenceOnlyRecoveryPolicyV1,
        price_policy: &PriceMeasurePolicyV1,
        genesis: &MarketGenesisProfileV2,
    ) -> Result<()> {
        self.validate_shape()?;
        series.validate_shape()?;
        template.validate_bindings(basis, recovery)?;
        genesis.validate_partition_bindings(basis, price_policy, self.resolved_edge_policy)?;

        let owners = self.semantic_owners;
        if series.product_template_id != template.id()?
            || series.market_genesis_profile_id != genesis.id()?
            || self.capability_profile_id != genesis.capability_profile_id
            || self.statistic_registry_value != template.statistic_registry_value
            || self.coverage_policy_registry_value != template.coverage_policy_registry_value
            || self.ambiguity_policy_registry_value != basis.ambiguity_policy_registry_value
            || self.edge_policy_registry_value != basis.edge_policy_registry_value
            || self.burn_terminal_disposition_registry_value
                != genesis.terminal_disposition_registry_value
            || owners.source_plane_contract_id != template.source_plane_contract_id
            || owners.source_spec_id != template.source_spec_id
            || owners.summary_program_id != template.summary_program_id
            || owners.native_claim_basis_id != template.native_claim_basis_id
            || owners.evidence_only_recovery_policy_id != template.evidence_only_recovery_policy_id
            || owners.product_compiler_release_id != template.compiler_release_id
            || owners.price_grid_id != genesis.price_grid_id
            || owners.price_measure_policy_id != genesis.price_measure_policy_id
            || owners.price_measure_policy_id != price_policy.id()?
            || owners.fee_policy_id != genesis.fee_policy_id
            || owners.relation_policy_id != genesis.relation_policy_id
            || owners.score_policy_id != genesis.score_policy_id
            || owners.candidate_lifecycle_policy_id != genesis.candidate_lifecycle_policy_id
            || owners.candidate_liveness_policy_id != genesis.candidate_liveness_policy_id
            || owners.retirement_policy_id != genesis.retirement_policy_id
            || self.realm_collateral.realm_id != genesis.realm_id
            || self.realm_collateral.profile_id != genesis.profile_id
        {
            return Err(Error::MismatchedArtifact);
        }

        let degree = usize::from(basis.basis_degree);
        if basis.basis_degree > MAX_BASIS_DEGREE
            || !self.supported_basis_degrees[degree]
            || basis.outcome_count > self.max_outcome_count
            || basis.payout_count > self.max_degree_zero_payout_count
            || recovery.attempt_count > self.max_recovery_attempt_count
            || template.coverage_policy_parameter < self.min_coverage_policy_parameter
            || template.coverage_policy_parameter > self.max_coverage_policy_parameter
            || template.window_span_buckets > self.max_window_span_buckets
            || series.instance_count > self.max_series_instance_count
            || series.market_collateral_cap > self.realm_collateral.market_collateral_cap_ceiling
        {
            return Err(Error::UnsupportedCapability);
        }
        Ok(())
    }
}

/// Deterministic lowering of one V5 Series ordinal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledOrdinalV2 {
    /// Series schedule that requested this occurrence.
    pub series_plan_id: SeriesPlanV5Id,
    /// Ordinal within that finite schedule.
    pub ordinal: u32,
    /// Economic market preimage excluding funding and attachments.
    pub market: MarketInstancePreimageV2,
    /// Full-width V2 economic market identity.
    pub market_instance_id: MarketInstanceV2Id,
    /// Operational attachment plan inherited from the Series.
    pub attachment_plan_id: SeriesAttachmentPlanId,
    /// Fully checked absolute source and recovery schedule.
    pub schedule: CompiledScheduleV1,
}

/// Compile one successor ordinal after joining every immutable artifact.
#[allow(clippy::too_many_arguments)]
pub fn compile_ordinal_v2(
    series: &SeriesPlanV5,
    template: &ProductTemplateV4,
    basis: &NativeClaimBasisV1,
    recovery: &EvidenceOnlyRecoveryPolicyV1,
    price_policy: &PriceMeasurePolicyV1,
    genesis: &MarketGenesisProfileV2,
    attachment: &SeriesAttachmentPlanV1,
    registry: &RegistryCapabilityProjectionV2,
    ordinal: u32,
) -> Result<CompiledOrdinalV2> {
    series.validate_bindings(
        template,
        basis,
        recovery,
        price_policy,
        genesis,
        attachment,
        registry,
    )?;
    let start_bucket = series.start_bucket(ordinal)?;
    let schedule = compile_schedule_v2(template, recovery, start_bucket)?;
    let market = MarketInstancePreimageV2 {
        product_template_id: template.id()?,
        market_genesis_profile_id: genesis.id()?,
        start_bucket,
        collateral_cap: series.market_collateral_cap,
    };
    market.validate_bindings(template, basis, price_policy, genesis)?;
    Ok(CompiledOrdinalV2 {
        series_plan_id: series.id()?,
        ordinal,
        market_instance_id: market.id()?,
        market,
        attachment_plan_id: attachment.id()?,
        schedule,
    })
}

/// Immutable funding ownership and refund identities for one V5 Series
/// activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingTermsV2 {
    /// Exact immutable successor Series being funded.
    pub series_plan_id: SeriesPlanV5Id,
    /// Persisted owner of refundable lamport principal.
    pub lamport_principal_refund: ContentId,
    /// Token account receiving refundable collateral principal.
    pub collateral_principal_refund_token_account: ContentId,
    /// Immutable neutral destination for unowned residue.
    pub neutral_sink: ContentId,
    /// Exact collateral mint selected through the Realm/Profile.
    pub collateral_mint: ContentId,
    /// Exact admitted token-program identity.
    pub token_program: ContentId,
}

impl SeriesFundingTermsV2 {
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

    /// Join funding ownership to the successor Series, Genesis, price policy,
    /// and complete Realm view.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_bindings(
        &self,
        series: &SeriesPlanV5,
        template: &ProductTemplateV4,
        basis: &NativeClaimBasisV1,
        recovery: &EvidenceOnlyRecoveryPolicyV1,
        price_policy: &PriceMeasurePolicyV1,
        genesis: &MarketGenesisProfileV2,
        registry: &RegistryCapabilityProjectionV2,
    ) -> Result<()> {
        self.validate_shape()?;
        registry.validate_complete_join(
            series,
            template,
            basis,
            recovery,
            price_policy,
            genesis,
        )?;
        if self.series_plan_id != series.id()?
            || series.market_genesis_profile_id != genesis.id()?
            || registry.realm_collateral.realm_id != genesis.realm_id
            || registry.realm_collateral.profile_id != genesis.profile_id
            || self.collateral_mint != registry.realm_collateral.collateral_mint
            || self.token_program != registry.realm_collateral.token_program
            || self.neutral_sink != registry.realm_collateral.neutral_incinerator
        {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }

    /// Typed identity of this successor funding ownership body.
    pub fn id(&self) -> Result<SeriesFundingTermsV2Id> {
        let mut body = [0; SERIES_FUNDING_TERMS_V2_BYTES];
        self.encode_into(&mut body)?;
        Ok(SeriesFundingTermsV2Id::from_bytes(
            content_id(SERIES_FUNDING_TERMS_V2_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for SeriesFundingTermsV2 {
    const ENCODED_LEN: usize = SERIES_FUNDING_TERMS_V2_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&SERIES_FUNDING_TERMS_V2_MAGIC);
        writer.u16(SCHEMA_V2);
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
        reader.magic(&SERIES_FUNDING_TERMS_V2_MAGIC)?;
        if reader.u16() != SCHEMA_V2 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            series_plan_id: SeriesPlanV5Id::from_bytes(reader.id().bytes()),
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

/// Untrusted claimed presence for one projected V2 component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectedComponentPresenceV2 {
    /// The projection claims the exact component is absent.
    Absent,
    /// The projection claims exact presence and full capitalization.
    ///
    /// This variant is freely constructible and carries no authentication.
    ClaimedPresentExactAndCapitalized,
}

/// Untrusted exact-existing versus absent component projection for one V2
/// market identity.
///
/// Every field is public and therefore forgeable. This pure DTO carries no
/// account-authentication authority; a live adapter must populate it only after
/// checking exact account identity, owner, body, and capitalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterFulfillmentProjectionV2 {
    /// Exact successor economic occurrence whose accounts were inspected.
    pub market_instance_id: MarketInstanceV2Id,
    /// Exact operational attachment whose accounts were inspected.
    pub attachment_plan_id: SeriesAttachmentPlanId,
    /// Exact quote whose component balances were inspected.
    pub funding_quote_id: SeriesFundingQuoteId,
    /// Economic market root and mandatory genesis plane.
    pub market_core: ProjectedComponentPresenceV2,
    /// Mandatory recovery state/reserve belonging to that market.
    pub recovery_reserve: ProjectedComponentPresenceV2,
    /// Shared source/window/evaluator work.
    pub source_work: ProjectedComponentPresenceV2,
    /// Liquidity-facility attachment.
    pub liquidity_facility: ProjectedComponentPresenceV2,
    /// Canonical structured-wrapper set.
    pub wrapper_set: ProjectedComponentPresenceV2,
}

impl AdapterFulfillmentProjectionV2 {
    fn validate(
        self,
        expected_market_instance_id: MarketInstanceV2Id,
        attachment: &SeriesAttachmentPlanV1,
        quote: &SeriesFundingQuoteV1,
    ) -> Result<()> {
        if self.market_instance_id != expected_market_instance_id
            || self.attachment_plan_id != attachment.id()?
            || self.funding_quote_id != quote.id()?
        {
            return Err(Error::MismatchedArtifact);
        }
        if self.market_core != self.recovery_reserve {
            return Err(Error::InvalidComponentStatus);
        }
        Ok(())
    }
}

/// Project exact successor per-component spending while retaining the V1 quote
/// and attachment amount owners.
///
/// This is a pure arithmetic projection, not authorization to omit a debit. A
/// live adapter must derive `projection` from exact authenticated accounts
/// before using the result to move lamports or collateral.
pub fn project_component_debits_v2(
    market_instance_id: MarketInstanceV2Id,
    recovery: &EvidenceOnlyRecoveryPolicyV1,
    attachment: &SeriesAttachmentPlanV1,
    quote: &SeriesFundingQuoteV1,
    projection: AdapterFulfillmentProjectionV2,
    available: FundingBalancesV1,
) -> Result<DebitProjectionV1> {
    quote.validate_recovery_binding(recovery)?;
    attachment.validate()?;
    if attachment.funding_quote_id != quote.id()? {
        return Err(Error::MismatchedArtifact);
    }
    projection.validate(market_instance_id, attachment, quote)?;
    let market_core = selected_component(quote.market_core, projection.market_core);
    let recovery_reserve = selected_component(quote.recovery_reserve, projection.recovery_reserve);
    let source_work = selected_component(quote.source_work, projection.source_work);
    let liquidity_facility =
        selected_component(quote.liquidity_facility, projection.liquidity_facility);
    let wrapper_set = selected_component(quote.wrapper_set, projection.wrapper_set);
    let total = checked_component_add(
        checked_component_add(
            checked_component_add(
                checked_component_add(market_core, recovery_reserve)?,
                source_work,
            )?,
            liquidity_facility,
        )?,
        wrapper_set,
    )?;
    let remaining = FundingBalancesV1 {
        lamports: available
            .lamports
            .checked_sub(total.lamports)
            .ok_or(Error::InsufficientPrepayment)?,
        collateral_atoms: available
            .collateral_atoms
            .checked_sub(total.collateral_atoms)
            .ok_or(Error::InsufficientPrepayment)?,
    };
    Ok(DebitProjectionV1 {
        market_core,
        recovery_reserve,
        source_work,
        liquidity_facility,
        wrapper_set,
        total,
        remaining,
    })
}

fn compile_schedule_v2(
    template: &ProductTemplateV4,
    recovery: &EvidenceOnlyRecoveryPolicyV1,
    start_bucket: u64,
) -> Result<CompiledScheduleV1> {
    let end_bucket_exclusive = start_bucket
        .checked_add(template.window_span_buckets)
        .ok_or(Error::ArithmeticOverflow)?;
    let primary_maturity_bucket_exclusive = end_bucket_exclusive
        .checked_add(template.primary_maturity_grace_buckets)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut recovery_attempts = [AbsoluteRecoveryAttemptV1::ZERO; MAX_RECOVERY_ATTEMPTS];
    let mut index = 0usize;
    while index < usize::from(recovery.attempt_count) {
        let relative = recovery.attempts[index];
        recovery_attempts[index] = AbsoluteRecoveryAttemptV1 {
            repair_generation: template
                .base_repair_generation
                .checked_add(relative.repair_generation_delta)
                .ok_or(Error::ArithmeticOverflow)?,
            opens_at_bucket: primary_maturity_bucket_exclusive
                .checked_add(relative.opens_after_primary_maturity_buckets)
                .ok_or(Error::ArithmeticOverflow)?,
            closes_at_bucket: primary_maturity_bucket_exclusive
                .checked_add(relative.closes_after_primary_maturity_buckets)
                .ok_or(Error::ArithmeticOverflow)?,
        };
        index += 1;
    }
    let schedule = CompiledScheduleV1 {
        start_bucket,
        end_bucket_exclusive,
        primary_maturity_bucket_exclusive,
        recovery_attempt_count: recovery.attempt_count,
        recovery_attempts,
    };
    schedule.validate()?;
    Ok(schedule)
}

fn validate_realm_collateral(value: RealmCollateralProjectionV1) -> Result<()> {
    for id in [
        value.realm_id,
        value.profile_id,
        value.collateral_mint,
        value.token_program,
        value.neutral_incinerator,
    ] {
        id.validate()?;
    }
    if value.market_collateral_cap_ceiling == 0 {
        return Err(Error::InvalidParameter);
    }
    Ok(())
}

fn selected_component(
    amount: ComponentDebitV1,
    status: ProjectedComponentPresenceV2,
) -> ComponentDebitV1 {
    match status {
        ProjectedComponentPresenceV2::Absent => amount,
        ProjectedComponentPresenceV2::ClaimedPresentExactAndCapitalized => ComponentDebitV1::ZERO,
    }
}

fn checked_component_add(
    left: ComponentDebitV1,
    right: ComponentDebitV1,
) -> Result<ComponentDebitV1> {
    Ok(ComponentDebitV1 {
        lamports: left
            .lamports
            .checked_add(right.lamports)
            .ok_or(Error::ArithmeticOverflow)?,
        collateral_atoms: left
            .collateral_atoms
            .checked_add(right.collateral_atoms)
            .ok_or(Error::ArithmeticOverflow)?,
    })
}
