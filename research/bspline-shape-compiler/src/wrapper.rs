//! Exact bridge from an analytic shape certificate to a transferable claim.
//!
//! The batch/runtime already owns the canonical native coefficient identity in
//! [`NativePortfolioClaimV1`].  This module deliberately reuses that identity:
//! it does not let an offline compiler invent a second meaning for the same Egg
//! vector.  The analytic certificate and display ratio are provenance, not
//! fungibility inputs.
//!
//! A transferable wrapper remains backed directly by one Market's native Eggs.
//! Wrapper inputs may be *flattened* off chain, but no wrapper-of-wrapper edge
//! is persisted.  The canonical backing plan also extracts the common
//! complete-set floor into the existing base Position's `cash_atoms`:
//!
//! ```text
//! p_i = k + r_i,  k = min_i p_i,  min_i r_i = 0
//! 1 W_p <-> k cash atoms + r_i native Egg atoms for every i
//! ```
//!
//! This is exact because one cash atom and one complete set have the same
//! payoff under every admitted simplex vector.  It reduces base Egg supply and
//! keeps cash and Eggs under the base Position's existing semantic ownership.
//! This host plan is not an SBF transition, Token-2022 mint, or deployment
//! claim.

use clutch_solana_layout::{
    portfolio_settlement::{NativePortfolioClaimV1, PortfolioSettlementError},
    Hash32, MarketAccount, TermsAccount, MAX_OUTCOMES,
};
use clutch_structured_claim::{
    realize_rational_shape, ClaimVector as CoreClaimVector,
    DeploymentBinding as CoreDeploymentBinding, Error as CoreError, RationalCoefficient,
    RationalShape,
};
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive};
use sha2::{Digest, Sha256};

use crate::{
    artifact::{basis_spec_from_terms_v1, ArtifactError, NativeShapeCertificateV1},
    Shape,
};

/// Wrapper backing policy encoded by this module.
pub const COMPLETE_SET_COMPRESSED_BACKING_V1: u16 = 1;

/// Refusal while joining compiler evidence to a transferable claim plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WrapperCompilerError {
    /// Canonical shape artifact or Terms projection refused.
    Artifact(ArtifactError),
    /// The live canonical portfolio owner refused the vector or identity.
    Portfolio(PortfolioSettlementError),
    /// Market and Terms bytes do not identify the same live market.
    MarketTermsMismatch,
    /// Wrapper creation was requested after the live market stopped being active.
    MarketNotActive,
    /// A program, ProgramData, or Token-2022 identity was zero or ambiguous.
    InvalidDeployment,
    /// The vector is a native Egg, zero, or complete-set cash in disguise.
    NoWrapperProductValue,
    /// Exact rational integerization does not fit the live `u64` amount domain.
    IntegerRealizationOverflow,
    /// A composition leg has zero quantity or names another native basis.
    InvalidComposition,
    /// A checked composition product or sum overflowed `u64`.
    CompositionOverflow,
    /// A recomputed field did not equal the host plan being verified.
    PlanMismatch,
}

impl From<ArtifactError> for WrapperCompilerError {
    fn from(value: ArtifactError) -> Self {
        Self::Artifact(value)
    }
}

impl From<PortfolioSettlementError> for WrapperCompilerError {
    fn from(value: PortfolioSettlementError) -> Self {
        Self::Portfolio(value)
    }
}

/// Exact deployments whose code and token semantics one wrapper depends on.
///
/// Program ids alone are not deployment identities while an upgradeable loader
/// may replace code at the same address.  A runtime adapter must authenticate
/// all three ProgramData accounts and their deployment slots on every mutating
/// route, or require an immutable release profile before promotion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WrapperDeploymentBindingV1 {
    pub wrapper_program: [u8; 32],
    pub wrapper_program_data: [u8; 32],
    pub wrapper_deployment_slot: u64,
    pub base_program: [u8; 32],
    pub base_program_data: [u8; 32],
    pub base_deployment_slot: u64,
    pub token_2022_program: [u8; 32],
    pub token_2022_program_data: [u8; 32],
    pub token_2022_deployment_slot: u64,
}

impl WrapperDeploymentBindingV1 {
    /// Refuse absent keys and aliasing between roles with different authority.
    pub fn validate(&self) -> Result<(), WrapperCompilerError> {
        self.core()
            .validate()
            .map_err(|_| WrapperCompilerError::InvalidDeployment)
    }

    fn core(&self) -> CoreDeploymentBinding {
        CoreDeploymentBinding {
            wrapper_program: self.wrapper_program,
            wrapper_program_data: self.wrapper_program_data,
            wrapper_deployment_slot: self.wrapper_deployment_slot,
            base_program: self.base_program,
            base_program_data: self.base_program_data,
            base_deployment_slot: self.base_deployment_slot,
            token_2022_program: self.token_2022_program,
            token_2022_program_data: self.token_2022_program_data,
            token_2022_deployment_slot: self.token_2022_deployment_slot,
        }
    }
}

/// Minimal integral realization of exact rational compiler coefficients.
///
/// If the target coefficients are `c_i`, this value proves
///
/// ```text
/// wrapper_atoms_per_display_lot * primitive_i
///   = target_units_per_display_lot * c_i
/// ```
///
/// for every active `i`.  Neither side is silently rounded.  The primitive
/// vector is the only coefficient vector used for the live claim identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntegerPortfolioRealizationV1 {
    pub outcome_count: u8,
    pub primitive: [u64; MAX_OUTCOMES],
    pub wrapper_atoms_per_display_lot: u64,
    pub target_units_per_display_lot: u64,
    pub complete_set_cash_atoms_per_wrapper: u64,
    pub residual_eggs_per_wrapper: [u64; MAX_OUTCOMES],
}

impl IntegerPortfolioRealizationV1 {
    fn validate(&self) -> Result<(), WrapperCompilerError> {
        let count = usize::from(self.outcome_count);
        if !(2..=MAX_OUTCOMES).contains(&count)
            || self.wrapper_atoms_per_display_lot == 0
            || self.target_units_per_display_lot == 0
            || self.primitive[count..].iter().any(|value| *value != 0)
            || self.residual_eggs_per_wrapper[count..]
                .iter()
                .any(|value| *value != 0)
        {
            return Err(WrapperCompilerError::PlanMismatch);
        }
        validate_wrapper_product(&self.primitive, count)?;
        let floor = self.primitive[..count]
            .iter()
            .copied()
            .min()
            .ok_or(WrapperCompilerError::PlanMismatch)?;
        if floor != self.complete_set_cash_atoms_per_wrapper {
            return Err(WrapperCompilerError::PlanMismatch);
        }
        for index in 0..count {
            if self.residual_eggs_per_wrapper[index]
                != self.primitive[index] - self.complete_set_cash_atoms_per_wrapper
            {
                return Err(WrapperCompilerError::PlanMismatch);
            }
        }
        Ok(())
    }
}

/// Host plan joining one recompile-verifiable shape to one live claim identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferableShapePlanV1 {
    pub deployment: WrapperDeploymentBindingV1,
    /// Wrapper identity; certificate, label, and display scaling are absent.
    pub product: Hash32,
    /// The exact identity already used by live portfolio clearing.
    pub claim: NativePortfolioClaimV1,
    /// Exact coefficient and complete-set decomposition.
    pub realization: IntegerPortfolioRealizationV1,
    /// Offline provenance. It is deliberately not part of `product`.
    pub certificate: NativeShapeCertificateV1,
    pub certificate_digest: [u8; 32],
}

impl TransferableShapePlanV1 {
    /// Recompute every binding from authenticated Market and Terms values.
    pub fn verify(
        &self,
        market: &MarketAccount,
        terms: &TermsAccount,
    ) -> Result<(), WrapperCompilerError> {
        self.deployment.validate()?;
        terms
            .binds_market(market)
            .map_err(|_| WrapperCompilerError::MarketTermsMismatch)?;
        self.certificate.verify_terms(terms)?;
        self.claim.validate()?;
        self.realization.validate()?;
        if self.claim.market != market.market
            || self.claim.terms != terms.terms
            || self.claim.basis_degree != terms.basis_degree
            || self.claim.denominator != terms.payouts[0].denominator
            || self.claim.outcome_count != terms.outcome_count
            || self.claim.coefficients != self.realization.primitive
            || self.certificate_digest != self.certificate.digest()?
            || self.product != canonical_wrapper_product_id_v1(self.deployment, self.claim.claim)?
        {
            return Err(WrapperCompilerError::PlanMismatch);
        }
        Ok(())
    }
}

/// Compile one analytic shape into the live portfolio identity and wrapper plan.
pub fn compile_transferable_shape_v1(
    deployment: WrapperDeploymentBindingV1,
    market: &MarketAccount,
    terms: &TermsAccount,
    shape: Shape,
) -> Result<TransferableShapePlanV1, WrapperCompilerError> {
    deployment.validate()?;
    terms
        .binds_market(market)
        .map_err(|_| WrapperCompilerError::MarketTermsMismatch)?;
    if market.lifecycle != 0 {
        return Err(WrapperCompilerError::MarketNotActive);
    }
    let basis = basis_spec_from_terms_v1(terms)?;
    let certificate = NativeShapeCertificateV1::compile(terms.terms.0, basis, shape)?;
    let realization = realize_native_coefficients_v1(&certificate.compilation.coefficients)?;
    if realization.outcome_count != terms.outcome_count {
        return Err(WrapperCompilerError::PlanMismatch);
    }
    let (claim, removed_gcd) =
        NativePortfolioClaimV1::compile(market.market, terms, realization.primitive)?;
    if removed_gcd != 1 {
        return Err(WrapperCompilerError::PlanMismatch);
    }
    let product = canonical_wrapper_product_id_v1(deployment, claim.claim)?;
    let certificate_digest = certificate.digest()?;
    let plan = TransferableShapePlanV1 {
        deployment,
        product,
        claim,
        realization,
        certificate,
        certificate_digest,
    };
    plan.verify(market, terms)?;
    Ok(plan)
}

/// Integerize exact rational coefficients, primitive-normalize them, and name
/// the minimal exact display conversion and complete-set floor.
pub fn realize_native_coefficients_v1(
    coefficients: &[BigRational],
) -> Result<IntegerPortfolioRealizationV1, WrapperCompilerError> {
    if coefficients.len() < 2 || coefficients.len() > MAX_OUTCOMES {
        return Err(WrapperCompilerError::NoWrapperProductValue);
    }
    if coefficients.iter().any(|value| value.is_negative()) {
        return Err(WrapperCompilerError::NoWrapperProductValue);
    }

    let mut exact = [RationalCoefficient::ZERO; MAX_OUTCOMES];
    for (index, coefficient) in coefficients.iter().enumerate() {
        exact[index] = RationalCoefficient::new(
            coefficient
                .numer()
                .to_u64()
                .ok_or(WrapperCompilerError::IntegerRealizationOverflow)?,
            coefficient
                .denom()
                .to_u64()
                .ok_or(WrapperCompilerError::IntegerRealizationOverflow)?,
        );
    }
    let core = realize_rational_shape(&RationalShape {
        outcome_count: u8::try_from(coefficients.len())
            .map_err(|_| WrapperCompilerError::PlanMismatch)?,
        coefficients: exact,
    })
    .map_err(map_core_realization_error)?;
    let realization = IntegerPortfolioRealizationV1 {
        outcome_count: core.claim.outcome_count,
        primitive: core.claim.coefficients,
        wrapper_atoms_per_display_lot: core.wrapper_atoms_per_display_lot,
        target_units_per_display_lot: core.target_units_per_display_lot,
        complete_set_cash_atoms_per_wrapper: core.backing.cash_per_wrapper,
        residual_eggs_per_wrapper: core.backing.residual_eggs_per_wrapper,
    };
    realization.validate()?;
    Ok(realization)
}

/// One transient composition input. The referenced wrapper must first split to
/// this already-authenticated native claim; it is never stored as an underlying.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionLegV1 {
    pub claim: NativePortfolioClaimV1,
    pub wrapper_atoms: u64,
}

/// Economic route for one exact flattened output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositionDispositionV1 {
    /// The output remains a nontrivial transferable coefficient product.
    TransferableWrapper,
    /// The vector is constant and must merge to cash, not mint a wrapper.
    CompleteSetCash,
}

/// Flattened exact native vector for a proposed fused wrapper operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlattenedCompositionV1 {
    pub claim: NativePortfolioClaimV1,
    /// GCD removed by the live identity owner.
    pub primitive_units: u64,
    /// Exact native Eggs released by all input wrappers.
    pub exact_eggs: [u64; MAX_OUTCOMES],
    /// Complete-set cash already present in the input wrappers' backing.
    pub input_cash_atoms: u64,
    /// Further complete sets exposed after vectors combine and available to
    /// merge before minting the output wrapper.
    pub additional_complete_sets_to_merge: u64,
    /// Canonical complete-set cash backing for all output wrapper atoms.
    pub output_cash_atoms: u64,
    /// Canonical residual Egg backing for all output wrapper atoms.
    pub output_residual_eggs: [u64; MAX_OUTCOMES],
    /// Whether the exact output may mint a wrapper or must collapse to cash.
    pub disposition: CompositionDispositionV1,
}

/// Flatten wrapper inputs into one native vector; never create a recursive edge.
pub fn flatten_composition_v1(
    market: &MarketAccount,
    terms: &TermsAccount,
    legs: &[CompositionLegV1],
) -> Result<FlattenedCompositionV1, WrapperCompilerError> {
    terms
        .binds_market(market)
        .map_err(|_| WrapperCompilerError::MarketTermsMismatch)?;
    if legs.is_empty() {
        return Err(WrapperCompilerError::InvalidComposition);
    }
    let mut exact = [0_u64; MAX_OUTCOMES];
    let mut input_cash_atoms = 0_u64;
    for leg in legs {
        leg.claim.validate()?;
        if leg.wrapper_atoms == 0
            || leg.claim.market != market.market
            || leg.claim.terms != terms.terms
            || leg.claim.basis_degree != terms.basis_degree
            || leg.claim.denominator != terms.payouts[0].denominator
            || leg.claim.outcome_count != terms.outcome_count
        {
            return Err(WrapperCompilerError::InvalidComposition);
        }
        validate_wrapper_product(&leg.claim.coefficients, usize::from(terms.outcome_count))
            .map_err(|_| WrapperCompilerError::InvalidComposition)?;
        let input_floor = leg.claim.coefficients[..usize::from(terms.outcome_count)]
            .iter()
            .copied()
            .min()
            .ok_or(WrapperCompilerError::InvalidComposition)?;
        input_cash_atoms = input_cash_atoms
            .checked_add(
                leg.wrapper_atoms
                    .checked_mul(input_floor)
                    .ok_or(WrapperCompilerError::CompositionOverflow)?,
            )
            .ok_or(WrapperCompilerError::CompositionOverflow)?;
        for (destination, coefficient) in exact.iter_mut().zip(leg.claim.coefficients) {
            let amount = leg
                .wrapper_atoms
                .checked_mul(coefficient)
                .ok_or(WrapperCompilerError::CompositionOverflow)?;
            *destination = destination
                .checked_add(amount)
                .ok_or(WrapperCompilerError::CompositionOverflow)?;
        }
    }
    let (claim, primitive_units) = NativePortfolioClaimV1::compile(market.market, terms, exact)?;
    let output_cash_atoms = exact[..usize::from(terms.outcome_count)]
        .iter()
        .copied()
        .min()
        .ok_or(WrapperCompilerError::InvalidComposition)?;
    let additional_complete_sets_to_merge = output_cash_atoms
        .checked_sub(input_cash_atoms)
        .ok_or(WrapperCompilerError::PlanMismatch)?;
    let mut output_residual_eggs = [0_u64; MAX_OUTCOMES];
    for index in 0..usize::from(terms.outcome_count) {
        output_residual_eggs[index] = exact[index] - output_cash_atoms;
    }
    let disposition = if output_residual_eggs[..usize::from(terms.outcome_count)]
        .iter()
        .all(|value| *value == 0)
    {
        CompositionDispositionV1::CompleteSetCash
    } else {
        validate_wrapper_product(&claim.coefficients, usize::from(terms.outcome_count))?;
        CompositionDispositionV1::TransferableWrapper
    };
    Ok(FlattenedCompositionV1 {
        claim,
        primitive_units,
        exact_eggs: exact,
        input_cash_atoms,
        additional_complete_sets_to_merge,
        output_cash_atoms,
        output_residual_eggs,
        disposition,
    })
}

/// Derive the wrapper-specific product id around the live native claim id.
///
/// The analytic certificate, human label, and display ratio are deliberately
/// absent. Deployment identities and backing semantics are deliberately
/// present: changing executable trust or custody semantics is a new wrapper.
pub fn canonical_wrapper_product_id_v1(
    deployment: WrapperDeploymentBindingV1,
    native_claim: Hash32,
) -> Result<Hash32, WrapperCompilerError> {
    deployment.validate()?;
    if native_claim == Hash32::ZERO {
        return Err(WrapperCompilerError::InvalidComposition);
    }
    let preimage = deployment
        .core()
        .product_preimage(native_claim.0)
        .map_err(|_| WrapperCompilerError::InvalidDeployment)?;
    let mut hasher = Sha256::new();
    hasher.update(preimage);
    let bytes: [u8; 32] = hasher.finalize().into();
    Hash32::new(bytes).map_err(|_| WrapperCompilerError::PlanMismatch)
}

fn validate_wrapper_product(
    coefficients: &[u64; MAX_OUTCOMES],
    count: usize,
) -> Result<(), WrapperCompilerError> {
    CoreClaimVector {
        outcome_count: u8::try_from(count)
            .map_err(|_| WrapperCompilerError::NoWrapperProductValue)?,
        coefficients: *coefficients,
    }
    .validate()
    .map_err(|_| WrapperCompilerError::NoWrapperProductValue)
}

fn map_core_realization_error(error: CoreError) -> WrapperCompilerError {
    match error {
        CoreError::ArithmeticOverflow | CoreError::ArithmeticUnderflow => {
            WrapperCompilerError::IntegerRealizationOverflow
        }
        CoreError::ZeroClaim | CoreError::SingleEggClaim | CoreError::CompleteSetClaim => {
            WrapperCompilerError::NoWrapperProductValue
        }
        _ => WrapperCompilerError::PlanMismatch,
    }
}
