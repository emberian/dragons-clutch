//! ProductRuntimeV3 join and exact representation-custody solvency.
//!
//! This module is the pure successor seam between Product-owned terminal
//! semantics and representation-owned recipes. It consumes the exact
//! projection returned by the authenticated ProductRuntimeV3 reader, decodes
//! the finalized ProductBasisV3, representation descriptor, and DAG, and
//! requires all runtime widths and identities to agree. It then joins the
//! immutable admission to a caller-owned Token/Claims custody projection.
//!
//! SHA-256, Registry raw/staging PDAs, rent, Token-2022 accounts, Claims state,
//! and program deployment authentication remain adapter boundaries. No boolean
//! or identity echo in this module substitutes for those checks.

use dclutch_product_payoff_v2_codec::runtime_v3::{
    BasisKindV3, Error as ProductBasisError, ProductBasisV3,
};
use dclutch_representation_composition_v3_kernel::{
    CompositionExposureBundleV3, CompositionExposureExecutionExpectedV3, RecordAdmissionV3,
};

use crate::{
    ContentAdmissionV2, DescriptorAdmissionV2, Error as RepresentationError,
    RepresentationDescriptorV2, StructuredProjectionV2,
    generated_product_v3::{
        ADMISSION_BASIS_KIND_OFFSET_V3, ADMISSION_BASIS_WIDTH_OFFSET_V3,
        ADMISSION_COORDINATE_DOMAIN_ID_OFFSET_V3, ADMISSION_DENOMINATOR_OFFSET_V3,
        ADMISSION_DESCRIPTOR_ID_OFFSET_V3, ADMISSION_EVALUATOR_RELEASE_ID_OFFSET_V3,
        ADMISSION_GRAPH_DIGEST_OFFSET_V3, ADMISSION_GRAPH_ID_OFFSET_V3,
        ADMISSION_GRAPH_SCALE_OFFSET_V3, ADMISSION_LINKED_BASIS_DIGEST_OFFSET_V3,
        ADMISSION_MAGIC_OFFSET_V3, ADMISSION_MARKET_ID_OFFSET_V3, ADMISSION_PAYOUT_SCALE_OFFSET_V3,
        ADMISSION_PRODUCT_ID_OFFSET_V3, ADMISSION_RECEIPT_MINT_OFFSET_V3,
        ADMISSION_RELEASE_SET_ID_OFFSET_V3, ADMISSION_REPRESENTATION_AUTHORITY_OFFSET_V3,
        ADMISSION_RESERVED_HEADER_OFFSET_V3, ADMISSION_RESERVED_SCALARS_OFFSET_V3,
        ADMISSION_RESULT_DOMAIN_ID_OFFSET_V3, ADMISSION_RESULT_UNIT_ID_OFFSET_V3,
        ADMISSION_SEMANTIC_BASIS_ID_OFFSET_V3, ADMISSION_TOKEN_PROGRAM_OFFSET_V3,
        ADMISSION_VERSION_OFFSET_V3,
    },
};

pub use crate::generated_product_v3::{
    PRODUCT_REPRESENTATION_ADMISSION_BYTES_V3, PRODUCT_REPRESENTATION_ADMISSION_MAGIC_V3,
    PRODUCT_REPRESENTATION_ADMISSION_VERSION_V3, PRODUCT_REPRESENTATION_CATEGORICAL_KIND_V3,
    PRODUCT_REPRESENTATION_GRADED_KIND_V3,
};

/// Stable refusal from Product V3 representation admission or solvency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The canonical ProductBasisV3 decoder refused the exact raw body.
    ProductBasis(ProductBasisError),
    /// The representation descriptor, graph, or custody kernel refused.
    Representation(RepresentationError),
    /// The authenticated Product-to-Claims composition exposure refused.
    Composition(dclutch_representation_composition_v3_kernel::Error),
    /// A Product, Claims, Market, release, descriptor, or Token identity differed.
    IdentityMismatch,
    /// Product, Claims, descriptor, graph, or custody runtime widths differed.
    WidthMismatch,
    /// A fixed admission receipt had another width, magic, or version.
    InvalidAdmission,
    /// Reserved bytes or the encoded basis-kind tag were noncanonical.
    NonCanonical,
    /// A payout vector did not have the exact admitted partition shape.
    InvalidPayout,
    /// Exact representation/custody arithmetic exceeded the `u128` profile.
    ArithmeticOverflow,
}

/// Result alias for Product V3 representation admission.
pub type Result<T> = core::result::Result<T, Error>;

/// Adapter-authenticated ProductRuntimeV3 projection.
///
/// The canonical SVM reader constructs these fields only after it has checked
/// every Product/domain/portfolio/ProductBasis raw and staging account. This
/// pure kernel repeats all joins visible in the decoded ProductBasis body but
/// deliberately cannot prove how the projection was obtained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductRuntimeProjectionV3 {
    /// Stable Product identity from the authenticated graph root.
    pub product_id: [u8; 32],
    /// Exact Product-owned result-domain raw-record digest.
    pub result_domain_id: [u8; 32],
    /// Product-owned coordinate-domain identity.
    pub coordinate_domain_id: [u8; 32],
    /// Product-owned result-unit identity.
    pub result_unit_id: [u8; 32],
    /// Product-owned semantic basis identity used by Claims.
    pub semantic_basis_id: [u8; 32],
    /// Exact finalized ProductBasisV3 raw-record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Immutable ProductBasisV3 evaluator release.
    pub evaluator_release_id: [u8; 32],
    /// Product terminal-result width `N`.
    pub basis_width: u32,
    /// Positive exact Product payout scale.
    pub payout_scale: u64,
}

/// Exact Market/Claims/Token context selected independently of representation bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepresentationContextV3 {
    /// Logical Core Market whose Claims back this representation.
    pub market_id: [u8; 32],
    /// Immutable execution release set.
    pub release_set_id: [u8; 32],
    /// Semantic basis identity persisted by the LBV2 Claims Market.
    pub claims_basis_id: [u8; 32],
    /// Runtime claim width persisted by the LBV2 Claims Market.
    pub claims_width: u32,
    /// Exact closeable Structured receipt Mint.
    pub receipt_mint: [u8; 32],
    /// Realm-selected Token program authenticated by the adapter.
    pub token_program: [u8; 32],
    /// Claims PDA derived from the finalized descriptor digest.
    pub representation_authority: [u8; 32],
}

/// Complete borrowed input for one immutable Product/representation join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductRepresentationInputV3<'a> {
    /// Exact finalized ProductBasisV3 raw body.
    pub product_basis_bytes: &'a [u8],
    /// Independently authenticated ProductRuntimeV3 projection.
    pub product: ProductRuntimeProjectionV3,
    /// Exact finalized rational representation descriptor body.
    pub descriptor_bytes: &'a [u8],
    /// Adapter-authenticated descriptor coordinate and Claims PDA derivation.
    pub descriptor_admission: DescriptorAdmissionV2,
    /// Exact finalized Product-to-Claims composition exposure bundle.
    pub graph_bytes: &'a [u8],
    /// Adapter-authenticated exposure-bundle coordinate selected by the descriptor.
    pub graph_admission: ContentAdmissionV2,
    /// Independently authenticated Market/Claims/Token facts.
    pub context: RepresentationContextV3,
}

/// Fixed-layout receipt from an exact Product/representation join.
///
/// This receipt is an ephemeral checked projection. It is not a persisted
/// source of Product, Registry, Claims, or Token authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepresentationAdmissionV3 {
    basis_kind: BasisKindV3,
    descriptor_id: [u8; 32],
    graph_id: [u8; 32],
    graph_digest: [u8; 32],
    product_id: [u8; 32],
    result_domain_id: [u8; 32],
    coordinate_domain_id: [u8; 32],
    result_unit_id: [u8; 32],
    semantic_basis_id: [u8; 32],
    linked_basis_record_digest: [u8; 32],
    market_id: [u8; 32],
    release_set_id: [u8; 32],
    receipt_mint: [u8; 32],
    token_program: [u8; 32],
    representation_authority: [u8; 32],
    evaluator_release_id: [u8; 32],
    basis_width: u32,
    payout_scale: u64,
    denominator: u64,
    graph_scale: u64,
}

impl RepresentationAdmissionV3 {
    /// Decode and hostile-validate one exact fixed-layout admission receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != PRODUCT_REPRESENTATION_ADMISSION_BYTES_V3
            || array::<8>(input, ADMISSION_MAGIC_OFFSET_V3)?
                != PRODUCT_REPRESENTATION_ADMISSION_MAGIC_V3
            || read_u16(input, ADMISSION_VERSION_OFFSET_V3)?
                != PRODUCT_REPRESENTATION_ADMISSION_VERSION_V3
        {
            return Err(Error::InvalidAdmission);
        }
        require_zero(input, ADMISSION_RESERVED_HEADER_OFFSET_V3, 5)?;
        require_zero(input, ADMISSION_RESERVED_SCALARS_OFFSET_V3, 4)?;
        let basis_kind = match byte(input, ADMISSION_BASIS_KIND_OFFSET_V3)? {
            PRODUCT_REPRESENTATION_CATEGORICAL_KIND_V3 => BasisKindV3::CategoricalQ1,
            PRODUCT_REPRESENTATION_GRADED_KIND_V3 => BasisKindV3::GradedExactComplement,
            _ => return Err(Error::NonCanonical),
        };
        let value = Self {
            basis_kind,
            descriptor_id: nonzero(input, ADMISSION_DESCRIPTOR_ID_OFFSET_V3)?,
            graph_id: nonzero(input, ADMISSION_GRAPH_ID_OFFSET_V3)?,
            graph_digest: nonzero(input, ADMISSION_GRAPH_DIGEST_OFFSET_V3)?,
            product_id: nonzero(input, ADMISSION_PRODUCT_ID_OFFSET_V3)?,
            result_domain_id: nonzero(input, ADMISSION_RESULT_DOMAIN_ID_OFFSET_V3)?,
            coordinate_domain_id: nonzero(input, ADMISSION_COORDINATE_DOMAIN_ID_OFFSET_V3)?,
            result_unit_id: nonzero(input, ADMISSION_RESULT_UNIT_ID_OFFSET_V3)?,
            semantic_basis_id: nonzero(input, ADMISSION_SEMANTIC_BASIS_ID_OFFSET_V3)?,
            linked_basis_record_digest: nonzero(input, ADMISSION_LINKED_BASIS_DIGEST_OFFSET_V3)?,
            market_id: nonzero(input, ADMISSION_MARKET_ID_OFFSET_V3)?,
            release_set_id: nonzero(input, ADMISSION_RELEASE_SET_ID_OFFSET_V3)?,
            receipt_mint: nonzero(input, ADMISSION_RECEIPT_MINT_OFFSET_V3)?,
            token_program: nonzero(input, ADMISSION_TOKEN_PROGRAM_OFFSET_V3)?,
            representation_authority: nonzero(input, ADMISSION_REPRESENTATION_AUTHORITY_OFFSET_V3)?,
            evaluator_release_id: nonzero(input, ADMISSION_EVALUATOR_RELEASE_ID_OFFSET_V3)?,
            basis_width: read_u32(input, ADMISSION_BASIS_WIDTH_OFFSET_V3)?,
            payout_scale: read_u64(input, ADMISSION_PAYOUT_SCALE_OFFSET_V3)?,
            denominator: read_u64(input, ADMISSION_DENOMINATOR_OFFSET_V3)?,
            graph_scale: read_u64(input, ADMISSION_GRAPH_SCALE_OFFSET_V3)?,
        };
        if value.basis_width == 0
            || value.payout_scale == 0
            || value.denominator == 0
            || value.graph_scale == 0
        {
            return Err(Error::NonCanonical);
        }
        Ok(value)
    }

    /// Encode this checked projection into its exact fixed-layout receipt.
    pub fn to_bytes(self) -> [u8; PRODUCT_REPRESENTATION_ADMISSION_BYTES_V3] {
        let mut output = [0_u8; PRODUCT_REPRESENTATION_ADMISSION_BYTES_V3];
        put(
            &mut output,
            ADMISSION_MAGIC_OFFSET_V3,
            &PRODUCT_REPRESENTATION_ADMISSION_MAGIC_V3,
        );
        put(
            &mut output,
            ADMISSION_VERSION_OFFSET_V3,
            &PRODUCT_REPRESENTATION_ADMISSION_VERSION_V3.to_le_bytes(),
        );
        output[ADMISSION_BASIS_KIND_OFFSET_V3] = match self.basis_kind {
            BasisKindV3::CategoricalQ1 => PRODUCT_REPRESENTATION_CATEGORICAL_KIND_V3,
            BasisKindV3::GradedExactComplement => PRODUCT_REPRESENTATION_GRADED_KIND_V3,
        };
        for (offset, value) in [
            (ADMISSION_DESCRIPTOR_ID_OFFSET_V3, self.descriptor_id),
            (ADMISSION_GRAPH_ID_OFFSET_V3, self.graph_id),
            (ADMISSION_GRAPH_DIGEST_OFFSET_V3, self.graph_digest),
            (ADMISSION_PRODUCT_ID_OFFSET_V3, self.product_id),
            (ADMISSION_RESULT_DOMAIN_ID_OFFSET_V3, self.result_domain_id),
            (
                ADMISSION_COORDINATE_DOMAIN_ID_OFFSET_V3,
                self.coordinate_domain_id,
            ),
            (ADMISSION_RESULT_UNIT_ID_OFFSET_V3, self.result_unit_id),
            (
                ADMISSION_SEMANTIC_BASIS_ID_OFFSET_V3,
                self.semantic_basis_id,
            ),
            (
                ADMISSION_LINKED_BASIS_DIGEST_OFFSET_V3,
                self.linked_basis_record_digest,
            ),
            (ADMISSION_MARKET_ID_OFFSET_V3, self.market_id),
            (ADMISSION_RELEASE_SET_ID_OFFSET_V3, self.release_set_id),
            (ADMISSION_RECEIPT_MINT_OFFSET_V3, self.receipt_mint),
            (ADMISSION_TOKEN_PROGRAM_OFFSET_V3, self.token_program),
            (
                ADMISSION_REPRESENTATION_AUTHORITY_OFFSET_V3,
                self.representation_authority,
            ),
            (
                ADMISSION_EVALUATOR_RELEASE_ID_OFFSET_V3,
                self.evaluator_release_id,
            ),
        ] {
            put(&mut output, offset, &value);
        }
        put(
            &mut output,
            ADMISSION_BASIS_WIDTH_OFFSET_V3,
            &self.basis_width.to_le_bytes(),
        );
        put(
            &mut output,
            ADMISSION_PAYOUT_SCALE_OFFSET_V3,
            &self.payout_scale.to_le_bytes(),
        );
        put(
            &mut output,
            ADMISSION_DENOMINATOR_OFFSET_V3,
            &self.denominator.to_le_bytes(),
        );
        put(
            &mut output,
            ADMISSION_GRAPH_SCALE_OFFSET_V3,
            &self.graph_scale.to_le_bytes(),
        );
        output
    }

    /// Canonical evaluator kind.
    pub const fn basis_kind(self) -> BasisKindV3 {
        self.basis_kind
    }
    /// Finalized representation descriptor digest.
    pub const fn descriptor_id(self) -> [u8; 32] {
        self.descriptor_id
    }
    /// Finalized representation graph identity.
    pub const fn graph_id(self) -> [u8; 32] {
        self.graph_id
    }
    /// Finalized representation graph raw-record digest.
    pub const fn graph_digest(self) -> [u8; 32] {
        self.graph_digest
    }
    /// Stable Product identity.
    pub const fn product_id(self) -> [u8; 32] {
        self.product_id
    }
    /// Exact Product result-domain raw-record digest.
    pub const fn result_domain_id(self) -> [u8; 32] {
        self.result_domain_id
    }
    /// Product coordinate-domain identity.
    pub const fn coordinate_domain_id(self) -> [u8; 32] {
        self.coordinate_domain_id
    }
    /// Product result-unit identity.
    pub const fn result_unit_id(self) -> [u8; 32] {
        self.result_unit_id
    }
    /// Finalized ProductBasisV3 raw-record digest.
    pub const fn linked_basis_record_digest(self) -> [u8; 32] {
        self.linked_basis_record_digest
    }
    /// Product-owned semantic basis identity persisted by Claims.
    pub const fn semantic_basis_id(self) -> [u8; 32] {
        self.semantic_basis_id
    }
    /// Runtime native Claims width.
    pub const fn basis_width(self) -> u32 {
        self.basis_width
    }
    /// Exact Product payout scale.
    pub const fn payout_scale(self) -> u64 {
        self.payout_scale
    }
    /// Shard atoms backing one native Claims atom.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }
    /// Structured receipt Mint.
    pub const fn receipt_mint(self) -> [u8; 32] {
        self.receipt_mint
    }
    /// Logical Market identity.
    pub const fn market_id(self) -> [u8; 32] {
        self.market_id
    }
    /// Immutable execution release set.
    pub const fn release_set_id(self) -> [u8; 32] {
        self.release_set_id
    }
    /// Realm-selected Token program.
    pub const fn token_program(self) -> [u8; 32] {
        self.token_program
    }
    /// Claims PDA controlling representation Token effects.
    pub const fn representation_authority(self) -> [u8; 32] {
        self.representation_authority
    }
    /// Immutable Product evaluator semantic release.
    pub const fn evaluator_release_id(self) -> [u8; 32] {
        self.evaluator_release_id
    }
    /// Exact representation DAG scale.
    pub const fn graph_scale(self) -> u64 {
        self.graph_scale
    }
}

/// Immutable, runtime-width Product/representation join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedProductRepresentationV3<'a> {
    basis: ProductBasisV3<'a>,
    descriptor: RepresentationDescriptorV2<'a>,
    exposure: CompositionExposureBundleV3<'a>,
    admission: RepresentationAdmissionV3,
}

/// Product terminal scenario admitted by ProductBasisV3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalScenarioV3 {
    /// Runtime categorical selector.
    Categorical(u32),
    /// Exact signed rational coordinate.
    Rational {
        /// Signed coordinate numerator.
        numerator: i128,
        /// Positive coordinate denominator.
        denominator: u64,
    },
    /// Product's explicit resolution-failure result.
    Failure,
}

/// Exact scenario-solvency equality in payout-times-shard numerator units.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioSolvencyV3 {
    /// Common shard denominator; no division or rounding occurred.
    pub denominator: u64,
    /// Product payout scale used by the evaluated partition.
    pub payout_scale: u64,
    /// Current Structured receipt supply.
    pub receipt_supply: u64,
    /// Total receipt liability numerator at the selected scenario.
    pub receipt_liability_numerator: u128,
    /// Exact value numerator of shards held by Structured custody.
    pub custody_value_numerator: u128,
}

/// Admitted immutable representation plus exact observed custody projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedRepresentationCustodyV3<'a> {
    representation: AdmittedProductRepresentationV3<'a>,
    custody: StructuredProjectionV2<'a>,
}

/// Join ProductBasisV3, the exact representation descriptor/DAG, LBV2 basis,
/// and immutable Market/Token context.
pub fn admit_product_representation_v3(
    input: ProductRepresentationInputV3<'_>,
) -> Result<AdmittedProductRepresentationV3<'_>> {
    validate_product_projection(input.product)?;
    validate_context(input.context)?;
    let basis = ProductBasisV3::decode(input.product_basis_bytes).map_err(Error::ProductBasis)?;
    if basis.product_id() != input.product.product_id
        || basis.result_domain_id() != input.product.result_domain_id
        || basis.coordinate_domain_id() != input.product.coordinate_domain_id
        || basis.result_unit_id() != input.product.result_unit_id
        || basis.evaluator_release_id() != input.product.evaluator_release_id
        || basis.payout_scale() != input.product.payout_scale
        || input.product.semantic_basis_id != input.context.claims_basis_id
    {
        return Err(Error::IdentityMismatch);
    }
    if basis.basis_width() != input.product.basis_width {
        return Err(Error::WidthMismatch);
    }
    let descriptor =
        RepresentationDescriptorV2::decode(input.descriptor_bytes, input.descriptor_admission)
            .map_err(Error::Representation)?;
    if descriptor.market_id() != input.context.market_id
        || descriptor.release_set_id() != input.context.release_set_id
        || descriptor.receipt_mint() != input.context.receipt_mint
        || descriptor.token_program() != input.context.token_program
        || descriptor.representation_authority() != input.context.representation_authority
    {
        return Err(Error::IdentityMismatch);
    }
    if descriptor.outcome_count() != input.context.claims_width {
        return Err(Error::WidthMismatch);
    }
    let exposure = CompositionExposureBundleV3::decode(
        input.graph_bytes,
        RecordAdmissionV3 {
            selected_id: input.graph_admission.selected_graph_id,
            finalized_id: input.graph_admission.finalized_graph_id,
            recomputed_digest: input.graph_admission.recomputed_graph_digest,
            finalized_digest: input.graph_admission.finalized_graph_digest,
            record_authenticated: input.graph_admission.record_authenticated,
        },
    )
    .map_err(Error::Composition)?
    .verify_execution_for(CompositionExposureExecutionExpectedV3 {
        market: input.context.market_id,
        result_domain: input.product.result_domain_id,
        release_set: input.context.release_set_id,
        product_basis: input.product.linked_basis_record_digest,
        representation_basis: input.context.claims_basis_id,
        product_width: basis.basis_width(),
        representation_width: input.context.claims_width,
    })
    .map_err(Error::Composition)?;
    if exposure.bundle_id() != descriptor.graph_id()
        || input.graph_admission.finalized_graph_digest != descriptor.graph_digest()
    {
        return Err(Error::IdentityMismatch);
    }
    let admission = RepresentationAdmissionV3 {
        basis_kind: basis.kind(),
        descriptor_id: descriptor.descriptor_id(),
        graph_id: descriptor.graph_id(),
        graph_digest: descriptor.graph_digest(),
        product_id: input.product.product_id,
        result_domain_id: input.product.result_domain_id,
        coordinate_domain_id: input.product.coordinate_domain_id,
        result_unit_id: input.product.result_unit_id,
        semantic_basis_id: input.product.semantic_basis_id,
        linked_basis_record_digest: input.product.linked_basis_record_digest,
        market_id: input.context.market_id,
        release_set_id: input.context.release_set_id,
        receipt_mint: input.context.receipt_mint,
        token_program: input.context.token_program,
        representation_authority: input.context.representation_authority,
        evaluator_release_id: input.product.evaluator_release_id,
        basis_width: input.context.claims_width,
        payout_scale: basis.payout_scale(),
        denominator: descriptor.denominator(),
        graph_scale: exposure.common_denominator().map_err(Error::Composition)?,
    };
    Ok(AdmittedProductRepresentationV3 {
        basis,
        descriptor,
        exposure,
        admission,
    })
}

impl<'a> AdmittedProductRepresentationV3<'a> {
    /// Return the exact fixed-layout immutable admission receipt.
    pub const fn admission(self) -> RepresentationAdmissionV3 {
        self.admission
    }

    /// Join exact Token/Claims observations to this immutable admission.
    pub fn admit_custody(
        self,
        custody_bytes: &'a [u8],
    ) -> Result<AdmittedRepresentationCustodyV3<'a>> {
        let custody =
            StructuredProjectionV2::decode(custody_bytes).map_err(Error::Representation)?;
        if custody.descriptor_id() != self.admission.descriptor_id
            || custody.market_id() != self.admission.market_id
            || custody.receipt_mint() != self.admission.receipt_mint
        {
            return Err(Error::IdentityMismatch);
        }
        if custody.outcome_count() != self.admission.basis_width
            || custody.denominator() != self.admission.denominator
        {
            return Err(Error::WidthMismatch);
        }
        let mut outcome = 0_u32;
        while outcome < self.admission.basis_width {
            if custody
                .coordinate(outcome)
                .map_err(Error::Representation)?
                .coefficient
                != self
                    .descriptor
                    .coefficient(outcome)
                    .map_err(Error::Representation)?
            {
                return Err(Error::IdentityMismatch);
            }
            outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(AdmittedRepresentationCustodyV3 {
            representation: self,
            custody,
        })
    }
}

impl AdmittedRepresentationCustodyV3<'_> {
    /// Evaluate an admitted Product scenario and prove exact receipt backing.
    ///
    /// `product_payout_scratch` has exact Product width `N`.
    /// `translation_scratch` and `claims_payouts` have exact Claims width `K`.
    /// `claims_payouts` changes only after the Product partition and every
    /// exact exposure-row division have succeeded.
    pub fn prove_scenario_solvency(
        self,
        scenario: TerminalScenarioV3,
        product_payout_scratch: &mut [u64],
        translation_scratch: &mut [u64],
        claims_payouts: &mut [u64],
    ) -> Result<ScenarioSolvencyV3> {
        let basis = self.representation.basis;
        match scenario {
            TerminalScenarioV3::Categorical(selector) => basis
                .evaluate_categorical(selector, product_payout_scratch)
                .map_err(Error::ProductBasis)?,
            TerminalScenarioV3::Rational {
                numerator,
                denominator,
            } => basis
                .evaluate_rational(numerator, denominator, product_payout_scratch)
                .map_err(Error::ProductBasis)?,
            TerminalScenarioV3::Failure => basis
                .evaluate_failure(product_payout_scratch)
                .map_err(Error::ProductBasis)?,
        }
        validate_partition(
            product_payout_scratch,
            self.representation.admission.payout_scale,
        )?;
        self.representation
            .exposure
            .translate_product_payouts(product_payout_scratch, translation_scratch, claims_payouts)
            .map_err(Error::Composition)?;
        let receipt_supply = self.custody.receipt_supply();
        let mut receipt_unit_numerator = 0_u128;
        let mut custody_value_numerator = 0_u128;
        let mut outcome = 0_u32;
        while outcome < self.representation.admission.basis_width {
            let index = usize::try_from(outcome).map_err(|_| Error::WidthMismatch)?;
            let payout = u128::from(*claims_payouts.get(index).ok_or(Error::WidthMismatch)?);
            let coordinate = self
                .custody
                .coordinate(outcome)
                .map_err(Error::Representation)?;
            receipt_unit_numerator = receipt_unit_numerator
                .checked_add(
                    u128::from(coordinate.coefficient)
                        .checked_mul(payout)
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            custody_value_numerator = custody_value_numerator
                .checked_add(
                    u128::from(coordinate.structured_custody)
                        .checked_mul(payout)
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        let receipt_liability_numerator = u128::from(receipt_supply)
            .checked_mul(receipt_unit_numerator)
            .ok_or(Error::ArithmeticOverflow)?;
        if receipt_liability_numerator != custody_value_numerator {
            return Err(Error::InvalidPayout);
        }
        Ok(ScenarioSolvencyV3 {
            denominator: self.representation.admission.denominator,
            payout_scale: self.representation.admission.payout_scale,
            receipt_supply,
            receipt_liability_numerator,
            custody_value_numerator,
        })
    }
}

fn validate_product_projection(value: ProductRuntimeProjectionV3) -> Result<()> {
    for identity in [
        value.product_id,
        value.result_domain_id,
        value.coordinate_domain_id,
        value.result_unit_id,
        value.semantic_basis_id,
        value.linked_basis_record_digest,
        value.evaluator_release_id,
    ] {
        if identity.iter().all(|byte| *byte == 0) {
            return Err(Error::IdentityMismatch);
        }
    }
    if value.basis_width == 0 || value.payout_scale == 0 {
        return Err(Error::WidthMismatch);
    }
    Ok(())
}

fn validate_context(value: RepresentationContextV3) -> Result<()> {
    for identity in [
        value.market_id,
        value.release_set_id,
        value.claims_basis_id,
        value.receipt_mint,
        value.token_program,
        value.representation_authority,
    ] {
        if identity.iter().all(|byte| *byte == 0) {
            return Err(Error::IdentityMismatch);
        }
    }
    if value.claims_width == 0 {
        return Err(Error::WidthMismatch);
    }
    Ok(())
}

fn validate_partition(payouts: &[u64], scale: u64) -> Result<()> {
    let mut total = 0_u64;
    for payout in payouts {
        if *payout > scale {
            return Err(Error::InvalidPayout);
        }
        total = total
            .checked_add(*payout)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if payouts.is_empty() || total != scale {
        return Err(Error::InvalidPayout);
    }
    Ok(())
}

fn array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    input
        .get(offset..offset.checked_add(N).ok_or(Error::InvalidAdmission)?)
        .ok_or(Error::InvalidAdmission)?
        .try_into()
        .map_err(|_| Error::InvalidAdmission)
}

fn nonzero(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    let value = array(input, offset)?;
    if value.iter().all(|byte| *byte == 0) {
        Err(Error::NonCanonical)
    } else {
        Ok(value)
    }
}

fn byte(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidAdmission)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(input, offset)?))
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    if input
        .get(offset..offset.checked_add(width).ok_or(Error::InvalidAdmission)?)
        .ok_or(Error::InvalidAdmission)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(Error::NonCanonical)
    } else {
        Ok(())
    }
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(end) = offset.checked_add(value.len())
        && let Some(destination) = output.get_mut(offset..end)
    {
        destination.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::{
        CoordinateObservation, SCALAR_BYTES, STRUCTURED_HEADER_BYTES, STRUCTURED_VECTOR_COUNT,
        StructuredProjectionHeaderV2,
        generated_descriptor::{
            DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_DENOMINATOR_OFFSET,
            DESCRIPTOR_GRAPH_DIGEST_OFFSET, DESCRIPTOR_GRAPH_ID_OFFSET, DESCRIPTOR_HEADER_BYTES,
            DESCRIPTOR_MAGIC_OFFSET, DESCRIPTOR_MAGIC_V3, DESCRIPTOR_MARKET_ID_OFFSET,
            DESCRIPTOR_OUTCOME_COUNT_OFFSET, DESCRIPTOR_RECEIPT_MINT_OFFSET,
            DESCRIPTOR_RELEASE_SET_ID_OFFSET, DESCRIPTOR_ROOT_ID_OFFSET,
            DESCRIPTOR_SCHEMA_VERSION_V3, DESCRIPTOR_TOKEN_PROGRAM_OFFSET,
            DESCRIPTOR_VERSION_OFFSET,
        },
    };
    use dclutch_product_payoff_v2_codec::runtime_v3::{
        BasisInputV3, BasisShapeV3, BasisTermV3, basis_record_bytes_v3, compile_basis_v3,
    };
    use dclutch_representation_composition_v3_kernel::{
        CompositionExposureInputV3, CompositionExposureRowInputV3, CompositionExposureTermV3,
        composition_exposure_bytes_v3, encode_composition_exposure_v3_atomic,
    };
    use std::{vec, vec::Vec};

    const WIDTH: u32 = 3;
    const SHARD_DENOMINATOR: u64 = 10;
    const COEFFICIENTS: [u64; 3] = [20, 30, 10];

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn fixture_put(output: &mut [u8], offset: usize, value: &[u8]) {
        output
            .get_mut(offset..offset + value.len())
            .expect("fixture offset")
            .copy_from_slice(value);
    }

    fn put_u32(output: &mut [u8], offset: usize, value: u32) {
        fixture_put(output, offset, &value.to_le_bytes());
    }

    fn put_u64(output: &mut [u8], offset: usize, value: u64) {
        fixture_put(output, offset, &value.to_le_bytes());
    }

    fn graded_basis() -> Vec<u8> {
        let knots = [0_i128, 10_i128];
        let terms = [
            BasisTermV3 {
                claim_index: 0,
                shape: BasisShapeV3::RampUp { left: 0, right: 1 },
                amplitude: 4,
            },
            BasisTermV3 {
                claim_index: 1,
                shape: BasisShapeV3::RampDown { left: 0, right: 1 },
                amplitude: 3,
            },
        ];
        let failures = [0_u64, 0, 10];
        basis(
            BasisKindV3::GradedExactComplement,
            10,
            &knots,
            &terms,
            &failures,
        )
    }

    fn categorical_basis() -> Vec<u8> {
        basis(BasisKindV3::CategoricalQ1, 1, &[], &[], &[])
    }

    fn basis(
        kind: BasisKindV3,
        payout_scale: u64,
        knots: &[i128],
        terms: &[BasisTermV3],
        failures: &[u64],
    ) -> Vec<u8> {
        let width = basis_record_bytes_v3(kind, WIDTH as usize, knots.len(), terms.len())
            .expect("basis width");
        let mut output = vec![0_u8; width];
        compile_basis_v3(
            BasisInputV3 {
                kind,
                product_id: id(40),
                result_domain_id: id(41),
                coordinate_domain_id: id(42),
                result_unit_id: id(43),
                evaluator_release_id: id(44),
                basis_width: WIDTH,
                payout_scale,
                knot_denominator: 1,
                knots,
                terms,
                failure_payouts: failures,
            },
            &mut output,
        )
        .expect("canonical ProductBasisV3");
        output
    }

    fn descriptor() -> Vec<u8> {
        let mut output =
            vec![0_u8; DESCRIPTOR_HEADER_BYTES + WIDTH as usize * DESCRIPTOR_COEFFICIENT_BYTES];
        fixture_put(&mut output, DESCRIPTOR_MAGIC_OFFSET, &DESCRIPTOR_MAGIC_V3);
        fixture_put(
            &mut output,
            DESCRIPTOR_VERSION_OFFSET,
            &DESCRIPTOR_SCHEMA_VERSION_V3.to_le_bytes(),
        );
        for (offset, value) in [
            (DESCRIPTOR_GRAPH_ID_OFFSET, id(21)),
            (DESCRIPTOR_GRAPH_DIGEST_OFFSET, id(91)),
            (DESCRIPTOR_ROOT_ID_OFFSET, id(20)),
            (DESCRIPTOR_MARKET_ID_OFFSET, id(30)),
            (DESCRIPTOR_RELEASE_SET_ID_OFFSET, id(31)),
            (DESCRIPTOR_RECEIPT_MINT_OFFSET, id(32)),
            (DESCRIPTOR_TOKEN_PROGRAM_OFFSET, id(33)),
        ] {
            fixture_put(&mut output, offset, &value);
        }
        put_u32(&mut output, DESCRIPTOR_OUTCOME_COUNT_OFFSET, WIDTH);
        put_u64(
            &mut output,
            DESCRIPTOR_DENOMINATOR_OFFSET,
            SHARD_DENOMINATOR,
        );
        for (index, coefficient) in COEFFICIENTS.iter().copied().enumerate() {
            put_u64(
                &mut output,
                DESCRIPTOR_HEADER_BYTES + index * DESCRIPTOR_COEFFICIENT_BYTES,
                coefficient,
            );
        }
        output
    }

    fn graph() -> Vec<u8> {
        let terms = [
            [CompositionExposureTermV3 {
                product_coordinate: 0,
                numerator: 1,
            }],
            [CompositionExposureTermV3 {
                product_coordinate: 1,
                numerator: 1,
            }],
            [CompositionExposureTermV3 {
                product_coordinate: 2,
                numerator: 1,
            }],
        ];
        let rows = [
            CompositionExposureRowInputV3 {
                node_id: id(10),
                denominator: 1,
                terms: &terms[0],
            },
            CompositionExposureRowInputV3 {
                node_id: id(11),
                denominator: 1,
                terms: &terms[1],
            },
            CompositionExposureRowInputV3 {
                node_id: id(12),
                denominator: 1,
                terms: &terms[2],
            },
        ];
        let length = composition_exposure_bytes_v3(WIDTH, WIDTH).expect("exposure width");
        let mut scratch = vec![0_u8; length];
        let mut output = vec![0_u8; length];
        encode_composition_exposure_v3_atomic(
            CompositionExposureInputV3 {
                market: id(30),
                result_domain: id(41),
                release_set: id(31),
                product_basis: id(45),
                representation_basis: id(50),
                graph_id: id(22),
                product_width: WIDTH,
                rows: &rows,
            },
            &mut scratch,
            &mut output,
        )
        .expect("canonical exposure");
        output
    }

    fn representation_input<'a>(
        basis: &'a [u8],
        descriptor: &'a [u8],
        graph: &'a [u8],
        payout_scale: u64,
    ) -> ProductRepresentationInputV3<'a> {
        ProductRepresentationInputV3 {
            product_basis_bytes: basis,
            product: ProductRuntimeProjectionV3 {
                product_id: id(40),
                result_domain_id: id(41),
                coordinate_domain_id: id(42),
                result_unit_id: id(43),
                semantic_basis_id: id(50),
                linked_basis_record_digest: id(45),
                evaluator_release_id: id(44),
                basis_width: WIDTH,
                payout_scale,
            },
            descriptor_bytes: descriptor,
            descriptor_admission: DescriptorAdmissionV2 {
                selected_descriptor_id: id(90),
                finalized_descriptor_id: id(90),
                recomputed_descriptor_digest: id(90),
                finalized_descriptor_digest: id(90),
                record_authenticated: true,
                derived_representation_authority: id(70),
                authority_derivation_authenticated: true,
            },
            graph_bytes: graph,
            graph_admission: ContentAdmissionV2 {
                selected_graph_id: id(21),
                finalized_graph_id: id(21),
                recomputed_graph_digest: id(91),
                finalized_graph_digest: id(91),
                record_authenticated: true,
            },
            context: RepresentationContextV3 {
                market_id: id(30),
                release_set_id: id(31),
                claims_basis_id: id(50),
                claims_width: WIDTH,
                receipt_mint: id(32),
                token_program: id(33),
                representation_authority: id(70),
            },
        }
    }

    fn custody() -> Vec<u8> {
        let mut output = vec![
            0_u8;
            STRUCTURED_HEADER_BYTES
                + WIDTH as usize * STRUCTURED_VECTOR_COUNT * SCALAR_BYTES
        ];
        StructuredProjectionV2::write_header(
            &mut output,
            StructuredProjectionHeaderV2 {
                descriptor_id: id(90),
                market_id: id(30),
                receipt_mint: id(32),
                outcome_count: WIDTH,
                denominator: SHARD_DENOMINATOR,
                receipt_supply: 2,
                revision: 7,
            },
        )
        .expect("projection header");
        for (outcome, observation) in [0_u32, 1, 2].into_iter().zip([
            CoordinateObservation {
                coefficient: 20,
                native_locked: 5,
                shard_supply: 50,
                structured_custody: 40,
                explicit_free_shards: 10,
            },
            CoordinateObservation {
                coefficient: 30,
                native_locked: 7,
                shard_supply: 70,
                structured_custody: 60,
                explicit_free_shards: 10,
            },
            CoordinateObservation {
                coefficient: 10,
                native_locked: 3,
                shard_supply: 30,
                structured_custody: 20,
                explicit_free_shards: 10,
            },
        ]) {
            StructuredProjectionV2::write_coordinate(&mut output, WIDTH, outcome, observation)
                .expect("projection coordinate");
        }
        output
    }

    #[test]
    fn graded_representation_is_runtime_joined_and_scenario_solvent() {
        let basis = graded_basis();
        let descriptor = descriptor();
        let graph = graph();
        let admitted =
            admit_product_representation_v3(representation_input(&basis, &descriptor, &graph, 10))
                .expect("Product V3 representation admission");
        let receipt = admitted.admission();
        assert_eq!(
            RepresentationAdmissionV3::decode(&receipt.to_bytes()),
            Ok(receipt)
        );
        assert_eq!(receipt.basis_width(), WIDTH);
        assert_eq!(receipt.semantic_basis_id(), id(50));
        let custody = custody();
        let joined = admitted.admit_custody(&custody).expect("exact custody");
        let mut product_payouts = [0_u64; 3];
        let mut translation_scratch = [0_u64; 3];
        let mut claims_payouts = [0_u64; 3];
        let rational = joined
            .prove_scenario_solvency(
                TerminalScenarioV3::Rational {
                    numerator: 5,
                    denominator: 1,
                },
                &mut product_payouts,
                &mut translation_scratch,
                &mut claims_payouts,
            )
            .expect("rational solvency");
        assert_eq!(product_payouts, [2, 1, 7]);
        assert_eq!(claims_payouts, [2, 1, 7]);
        assert_eq!(rational.receipt_liability_numerator, 280);
        assert_eq!(rational.custody_value_numerator, 280);
        let failure = joined
            .prove_scenario_solvency(
                TerminalScenarioV3::Failure,
                &mut product_payouts,
                &mut translation_scratch,
                &mut claims_payouts,
            )
            .expect("failure solvency");
        assert_eq!(product_payouts, [0, 0, 10]);
        assert_eq!(claims_payouts, [0, 0, 10]);
        assert_eq!(failure.receipt_liability_numerator, 200);
        assert_eq!(failure.custody_value_numerator, 200);
    }

    #[test]
    fn categorical_basis_uses_the_same_runtime_custody_semantics() {
        let basis = categorical_basis();
        let descriptor = descriptor();
        let graph = graph();
        let admitted =
            admit_product_representation_v3(representation_input(&basis, &descriptor, &graph, 1))
                .expect("categorical admission");
        let custody = custody();
        let joined = admitted.admit_custody(&custody).expect("exact custody");
        let mut product_payouts = [9_u64; 3];
        let mut translation_scratch = [9_u64; 3];
        let mut claims_payouts = [9_u64; 3];
        let proof = joined
            .prove_scenario_solvency(
                TerminalScenarioV3::Categorical(1),
                &mut product_payouts,
                &mut translation_scratch,
                &mut claims_payouts,
            )
            .expect("categorical solvency");
        assert_eq!(product_payouts, [0, 1, 0]);
        assert_eq!(claims_payouts, [0, 1, 0]);
        assert_eq!(proof.receipt_liability_numerator, 60);
        assert_eq!(proof.custody_value_numerator, 60);
    }

    #[test]
    fn substitutions_and_malformed_custody_refuse_before_proof() {
        let basis = graded_basis();
        let descriptor = descriptor();
        let graph = graph();
        let mut width_substitution = representation_input(&basis, &descriptor, &graph, 10);
        width_substitution.product.basis_width = 2;
        assert!(matches!(
            admit_product_representation_v3(width_substitution),
            Err(Error::WidthMismatch)
        ));

        let mut coefficient_substitution = descriptor.clone();
        put_u64(&mut coefficient_substitution, DESCRIPTOR_HEADER_BYTES, 21);
        let substituted = admit_product_representation_v3(representation_input(
            &basis,
            &coefficient_substitution,
            &graph,
            10,
        ))
        .expect("descriptor coefficient is joined at custody");
        let canonical_custody = custody();
        assert!(matches!(
            substituted.admit_custody(&canonical_custody),
            Err(Error::IdentityMismatch)
        ));

        let admitted =
            admit_product_representation_v3(representation_input(&basis, &descriptor, &graph, 10))
                .expect("admission");
        let mut malformed_custody = custody();
        let structured_vector = STRUCTURED_HEADER_BYTES + 3 * WIDTH as usize * SCALAR_BYTES;
        put_u64(&mut malformed_custody, structured_vector, 39);
        assert!(matches!(
            admitted.admit_custody(&malformed_custody),
            Err(Error::Representation(
                RepresentationError::StructuredCustodyMismatch
            ))
        ));
    }

    #[test]
    fn fixed_admission_hostile_decoding_is_canonical() {
        let basis = graded_basis();
        let descriptor = descriptor();
        let graph = graph();
        let admitted =
            admit_product_representation_v3(representation_input(&basis, &descriptor, &graph, 10))
                .expect("admission");
        let canonical = admitted.admission().to_bytes();
        for length in [0_usize, 527] {
            assert_eq!(
                RepresentationAdmissionV3::decode(canonical.get(..length).expect("hostile prefix")),
                Err(Error::InvalidAdmission)
            );
        }
        let mut magic = canonical;
        magic[0] ^= 1;
        assert_eq!(
            RepresentationAdmissionV3::decode(&magic),
            Err(Error::InvalidAdmission)
        );
        let mut reserved = canonical;
        reserved[ADMISSION_RESERVED_HEADER_OFFSET_V3] = 1;
        assert_eq!(
            RepresentationAdmissionV3::decode(&reserved),
            Err(Error::NonCanonical)
        );
        let mut zero_identity = canonical;
        zero_identity[ADMISSION_PRODUCT_ID_OFFSET_V3..ADMISSION_PRODUCT_ID_OFFSET_V3 + 32].fill(0);
        assert_eq!(
            RepresentationAdmissionV3::decode(&zero_identity),
            Err(Error::NonCanonical)
        );
        let mut zero_width = canonical;
        zero_width[ADMISSION_BASIS_WIDTH_OFFSET_V3..ADMISSION_BASIS_WIDTH_OFFSET_V3 + 4].fill(0);
        assert_eq!(
            RepresentationAdmissionV3::decode(&zero_width),
            Err(Error::NonCanonical)
        );
    }
}
