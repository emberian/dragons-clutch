//! Canonical ProductBasisV3 terminal planning through SignedDeltaV3.
//!
//! This module introduces no terminal-family wire or receipt. It joins one
//! adapter-authenticated Product/representation admission to the canonical
//! runtime-width LBV2 Market and Position bytes, evaluates the ProductBasisV3
//! terminal partition, proves exact pre/post hoard solvency, and emits the
//! existing family-neutral [`SignedDeltaPlanV3`] packet.
//!
//! SHA-256, Registry raw/staging authentication, Product graph authentication,
//! finalized terminal-coordinate authentication, account ownership, PDA
//! derivation, and custody execution remain SVM-adapter boundaries. In
//! particular, `product_basis_bytes` must be the exact raw slice returned by
//! the authenticated ProductRuntimeV3 reader that produced `representation`.

use dclutch_product_payoff_v2_codec::runtime_v3::{BasisKindV3, ProductBasisV3};
use dclutch_rational_representation_v2_kernel::product_v3::{
    RepresentationAdmissionV3, TerminalScenarioV3,
};
use dclutch_representation_composition_v3_kernel::{
    CompositionExposureBundleV3, CompositionExposureExecutionExpectedV3, RecordAdmissionV3,
};

use crate::{
    CallerRole,
    liability_basis_state_v2::{LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2},
    signed_delta_v3::{
        DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SignedDeltaPlanInputV3,
        SignedDeltaPlanV3, SignedDeltaPositionV3, SignedDeltaV3, plan_bytes,
    },
};

/// Exact rational terminal-coordinate record width shared by ProductV3
/// adapters and chain-derived operators.
pub const TERMINAL_COORDINATE_BYTES_V2: usize = 32;
/// Canonical rational terminal-coordinate magic.
pub const TERMINAL_COORDINATE_MAGIC_V2: [u8; 8] = *b"DCLTRC02";
/// Canonical Core-owned rational terminal-coordinate Registry schema.
pub const TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2: [u8; 32] = [
    0xa8, 0x66, 0x06, 0x2a, 0xe7, 0x6d, 0x3d, 0xc3, 0xa7, 0xc7, 0xce, 0xe5, 0x34, 0x0a, 0xc9, 0xe4,
    0x1f, 0x20, 0x22, 0x69, 0xcb, 0x23, 0xe9, 0xb7, 0x04, 0x61, 0xb0, 0x16, 0xf1, 0x8d, 0x5f, 0x61,
];
/// Domain separating the terminal Custody candidate from every other digest.
pub const TERMINAL_CANDIDATE_DOMAIN_V3: &[u8] = b"dclutch/rational-terminal-candidate/v3";

/// Stable refusal from ProductBasisV3 terminal planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// ProductBasisV3 decoding or terminal evaluation refused.
    ProductBasis,
    /// An independently authenticated Product/representation identity differed.
    Representation,
    /// The finalized Product-to-Claims exposure bundle refused.
    Composition,
    /// Canonical LBV2 Market bytes refused.
    MarketState,
    /// Canonical LBV2 Position bytes refused.
    PositionState,
    /// Market, Position, Product, basis, release, owner, or account links differed.
    IdentityMismatch,
    /// Product, representation, Market, Position, or scratch widths differed.
    WidthMismatch,
    /// Generation or optimistic revision facts differed or were terminal sentinels.
    RevisionMismatch,
    /// The selected terminal claim or quantity was invalid.
    InvalidDebit,
    /// A Position coordinate exceeded aggregate supply.
    PositionExceedsSupply,
    /// Aggregate or Position balance could not fund the terminal debit.
    InsufficientBalance,
    /// Exact pre- or post-redemption liability exceeded the hoard.
    Insolvent,
    /// Exact payout, liability, or address arithmetic exceeded its bounded type.
    ArithmeticOverflow,
    /// The supplied Product terminal result was not an exact payout partition.
    InvalidPartition,
    /// The canonical SignedDeltaV3 packet refused construction or decoding.
    SignedDelta,
}

/// Result alias for ProductBasisV3 terminal planning.
pub type Result<T> = core::result::Result<T, Error>;

/// Family-neutral authenticated Product-to-Claims terminal projection.
///
/// The SVM adapter constructs this value only after independently
/// authenticating ProductRuntimeV3, the finalized composition-exposure
/// record, the canonical LBV2 Market, and the selected release. It contains
/// no Rational, Fractional, Bearer, or Structured action tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductClaimsTerminalAdmissionV3 {
    exposure_id: [u8; 32],
    exposure_digest: [u8; 32],
    product_id: [u8; 32],
    result_domain_id: [u8; 32],
    coordinate_domain_id: [u8; 32],
    result_unit_id: [u8; 32],
    semantic_basis_id: [u8; 32],
    linked_basis_record_digest: [u8; 32],
    market_id: [u8; 32],
    release_set_id: [u8; 32],
    evaluator_release_id: [u8; 32],
    basis_width: u32,
    payout_scale: u64,
}

impl ProductClaimsTerminalAdmissionV3 {
    /// Construct one checked projection from independently authenticated facts.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        exposure_id: [u8; 32],
        exposure_digest: [u8; 32],
        product_id: [u8; 32],
        result_domain_id: [u8; 32],
        coordinate_domain_id: [u8; 32],
        result_unit_id: [u8; 32],
        semantic_basis_id: [u8; 32],
        linked_basis_record_digest: [u8; 32],
        market_id: [u8; 32],
        release_set_id: [u8; 32],
        evaluator_release_id: [u8; 32],
        basis_width: u32,
        payout_scale: u64,
    ) -> Result<Self> {
        for identity in [
            exposure_id,
            exposure_digest,
            product_id,
            result_domain_id,
            coordinate_domain_id,
            result_unit_id,
            semantic_basis_id,
            linked_basis_record_digest,
            market_id,
            release_set_id,
            evaluator_release_id,
        ] {
            if identity.iter().all(|byte| *byte == 0) {
                return Err(Error::IdentityMismatch);
            }
        }
        if basis_width == 0 || payout_scale == 0 {
            return Err(Error::WidthMismatch);
        }
        Ok(Self {
            exposure_id,
            exposure_digest,
            product_id,
            result_domain_id,
            coordinate_domain_id,
            result_unit_id,
            semantic_basis_id,
            linked_basis_record_digest,
            market_id,
            release_set_id,
            evaluator_release_id,
            basis_width,
            payout_scale,
        })
    }

    /// Finalized logical exposure identity.
    pub const fn exposure_id(self) -> [u8; 32] {
        self.exposure_id
    }
    /// SHA-256 of the exact finalized exposure bytes.
    pub const fn exposure_digest(self) -> [u8; 32] {
        self.exposure_digest
    }
    /// Stable semantic Product identity.
    pub const fn product_id(self) -> [u8; 32] {
        self.product_id
    }
    /// Product-owned result-domain identity.
    pub const fn result_domain_id(self) -> [u8; 32] {
        self.result_domain_id
    }
    /// Product-owned coordinate-domain identity.
    pub const fn coordinate_domain_id(self) -> [u8; 32] {
        self.coordinate_domain_id
    }
    /// Product-owned result-unit identity.
    pub const fn result_unit_id(self) -> [u8; 32] {
        self.result_unit_id
    }
    /// Claims semantic basis identity.
    pub const fn semantic_basis_id(self) -> [u8; 32] {
        self.semantic_basis_id
    }
    /// Finalized ProductBasisV3 record digest.
    pub const fn linked_basis_record_digest(self) -> [u8; 32] {
        self.linked_basis_record_digest
    }
    /// Logical Core Market identity.
    pub const fn market_id(self) -> [u8; 32] {
        self.market_id
    }
    /// Immutable selected release set.
    pub const fn release_set_id(self) -> [u8; 32] {
        self.release_set_id
    }
    /// Immutable Product evaluator release.
    pub const fn evaluator_release_id(self) -> [u8; 32] {
        self.evaluator_release_id
    }
    /// Runtime Claims width K.
    pub const fn basis_width(self) -> u32 {
        self.basis_width
    }
    /// Exact Product payout scale.
    pub const fn payout_scale(self) -> u64 {
        self.payout_scale
    }
}

/// Borrowed family-neutral input for one terminal Claims settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductClaimsTerminalInputV3<'a> {
    /// Exact authenticated ProductBasisV3 raw bytes.
    pub product_basis_bytes: &'a [u8],
    /// Independently authenticated Product/Claims/exposure projection.
    pub admission: ProductClaimsTerminalAdmissionV3,
    /// Exact finalized Product-to-Claims exposure bytes.
    pub composition_exposure_bytes: &'a [u8],
    /// Exact finalized exposure record evidence.
    pub composition_exposure_admission: RecordAdmissionV3,
    /// Finalized Product graph-root digest.
    pub product_record_digest: [u8; 32],
    /// Canonical Claims aggregate account.
    pub market_account: [u8; 32],
    /// Exact canonical LBV2 aggregate bytes.
    pub market_bytes: &'a [u8],
    /// Exact canonical LBV2 Position bytes.
    pub position_bytes: &'a [u8],
    /// Sole Position owner debited.
    pub owner: [u8; 32],
    /// Digest of the complete Claims terminal-settlement request.
    pub request_id: [u8; 32],
    /// Registry role of the enclosing caller.
    pub caller_role: CallerRole,
    /// Exact Product terminal scenario authenticated from Core state.
    pub terminal: TerminalScenarioV3,
    /// Claims coordinate debited after Product-to-Claims translation.
    pub claim_index: u32,
    /// Positive native Claims atoms debited.
    pub quantity: u64,
    /// Immutable Market generation.
    pub expected_generation: u64,
    /// Aggregate optimistic pre-revision.
    pub expected_market_revision: u64,
    /// Position optimistic pre-revision.
    pub expected_position_revision: u64,
    /// Current canonical hoard collateral atoms.
    pub hoard_before: u64,
}

/// Borrowed input for one exact terminal debit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductBasisTerminalInputV3<'a> {
    /// Exact authenticated ProductBasisV3 raw bytes.
    pub product_basis_bytes: &'a [u8],
    /// Exact Product/descriptor/DAG admission from the SVM reader.
    pub representation: RepresentationAdmissionV3,
    /// Exact finalized Product-to-Claims exposure-bundle bytes.
    pub composition_exposure_bytes: &'a [u8],
    /// Adapter-authenticated finalized exposure record evidence.
    pub composition_exposure_admission: RecordAdmissionV3,
    /// Exact finalized Product graph-root digest from that same reader.
    pub product_record_digest: [u8; 32],
    /// Canonical Claims aggregate account identity authenticated by the adapter.
    pub market_account: [u8; 32],
    /// Exact canonical LBV2 aggregate bytes.
    pub market_bytes: &'a [u8],
    /// Exact canonical LBV2 Position bytes.
    pub position_bytes: &'a [u8],
    /// Sole Position owner debited by this packet.
    pub owner: [u8; 32],
    /// Caller-owned request digest binding the authenticated terminal coordinate.
    pub request_id: [u8; 32],
    /// Registry role of the enclosing execution program.
    pub caller_role: CallerRole,
    /// Exact Product terminal scenario authenticated by the adapter.
    pub terminal: TerminalScenarioV3,
    /// Product-native claim coordinate to debit.
    pub claim_index: u32,
    /// Positive claim atoms to debit.
    pub quantity: u64,
    /// Expected immutable Market generation.
    pub expected_generation: u64,
    /// Optimistic aggregate pre-revision.
    pub expected_market_revision: u64,
    /// Optimistic Position pre-revision.
    pub expected_position_revision: u64,
    /// Current collateral atoms in the canonical hoard.
    pub hoard_before: u64,
}

/// Evaluate one authenticated ProductBasisV3 terminal result and encode its
/// sole Position debit through the canonical SignedDeltaV3 waist.
///
/// `product_payout_scratch` has exact Product width `N`. `translation_scratch`,
/// `claims_payout_scratch`, and `aggregate_delta_scratch` have exact Claims
/// width `K`. The returned scalar is the exact collateral payout in Product
/// payout atoms: `quantity * translated_payout[claim_index]`. The only
/// translation is the authenticated canonical exposure bundle; no caller
/// matrix or additional rounding boundary exists here.
pub fn encode_product_basis_terminal_signed_delta_v3(
    input: ProductBasisTerminalInputV3<'_>,
    product_payout_scratch: &mut [u64],
    translation_scratch: &mut [u64],
    claims_payout_scratch: &mut [u64],
    aggregate_delta_scratch: &mut [SignedDeltaV3],
    output: &mut [u8],
) -> Result<u64> {
    let representation = input.representation;
    let admission = ProductClaimsTerminalAdmissionV3::new(
        representation.graph_id(),
        representation.graph_digest(),
        representation.product_id(),
        representation.result_domain_id(),
        representation.coordinate_domain_id(),
        representation.result_unit_id(),
        representation.semantic_basis_id(),
        representation.linked_basis_record_digest(),
        representation.market_id(),
        representation.release_set_id(),
        representation.evaluator_release_id(),
        representation.basis_width(),
        representation.payout_scale(),
    )?;
    encode_product_claims_terminal_signed_delta_v3(
        ProductClaimsTerminalInputV3 {
            product_basis_bytes: input.product_basis_bytes,
            admission,
            composition_exposure_bytes: input.composition_exposure_bytes,
            composition_exposure_admission: input.composition_exposure_admission,
            product_record_digest: input.product_record_digest,
            market_account: input.market_account,
            market_bytes: input.market_bytes,
            position_bytes: input.position_bytes,
            owner: input.owner,
            request_id: input.request_id,
            caller_role: input.caller_role,
            terminal: input.terminal,
            claim_index: input.claim_index,
            quantity: input.quantity,
            expected_generation: input.expected_generation,
            expected_market_revision: input.expected_market_revision,
            expected_position_revision: input.expected_position_revision,
            hoard_before: input.hoard_before,
        },
        product_payout_scratch,
        translation_scratch,
        claims_payout_scratch,
        aggregate_delta_scratch,
        output,
    )
}

/// Evaluate a family-neutral authenticated Product/Claims terminal projection
/// and encode its canonical SignedDeltaV3 packet.
pub fn encode_product_claims_terminal_signed_delta_v3(
    input: ProductClaimsTerminalInputV3<'_>,
    product_payout_scratch: &mut [u64],
    translation_scratch: &mut [u64],
    claims_payout_scratch: &mut [u64],
    aggregate_delta_scratch: &mut [SignedDeltaV3],
    output: &mut [u8],
) -> Result<u64> {
    validate_nonzero(input)?;
    let basis =
        ProductBasisV3::decode(input.product_basis_bytes).map_err(|_| Error::ProductBasis)?;
    let market =
        LiabilityBasisMarketViewV2::decode(input.market_bytes).map_err(|_| Error::MarketState)?;
    let position = LiabilityBasisPositionViewV2::decode(input.position_bytes)
        .map_err(|_| Error::PositionState)?;
    validate_joins(input, basis, market, position)?;
    let exposure = decode_exposure(input, basis, market)?;
    let product_width = usize::try_from(basis.basis_width()).map_err(|_| Error::WidthMismatch)?;
    let claims_width = usize::try_from(market.claim_count).map_err(|_| Error::WidthMismatch)?;
    if product_payout_scratch.len() != product_width
        || translation_scratch.len() != claims_width
        || claims_payout_scratch.len() != claims_width
        || aggregate_delta_scratch.len() != claims_width
    {
        return Err(Error::WidthMismatch);
    }
    let expected_output = plan_bytes(market.claim_count, 1, 1).map_err(|_| Error::SignedDelta)?;
    if output.len() != expected_output {
        return Err(Error::SignedDelta);
    }
    evaluate(basis, input.terminal, product_payout_scratch)?;
    validate_partition(product_payout_scratch, basis.payout_scale())?;
    exposure
        .translate_product_payouts(
            product_payout_scratch,
            translation_scratch,
            claims_payout_scratch,
        )
        .map_err(|_| Error::Composition)?;
    let selected = usize::try_from(input.claim_index).map_err(|_| Error::InvalidDebit)?;
    let mut liability_before = 0_u128;
    let mut outcome = 0_u32;
    while outcome < market.claim_count {
        let supply = market
            .supply(input.market_bytes, outcome)
            .map_err(|_| Error::MarketState)?;
        let balance = position
            .balance(input.position_bytes, outcome)
            .map_err(|_| Error::PositionState)?;
        if balance > supply {
            return Err(Error::PositionExceedsSupply);
        }
        let index = usize::try_from(outcome).map_err(|_| Error::WidthMismatch)?;
        let payout = *claims_payout_scratch
            .get(index)
            .ok_or(Error::WidthMismatch)?;
        liability_before = liability_before
            .checked_add(
                u128::from(supply)
                    .checked_mul(u128::from(payout))
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)?;
        outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    let selected_supply = market
        .supply(input.market_bytes, input.claim_index)
        .map_err(|_| Error::InvalidDebit)?;
    let selected_balance = position
        .balance(input.position_bytes, input.claim_index)
        .map_err(|_| Error::InvalidDebit)?;
    if selected_supply < input.quantity || selected_balance < input.quantity {
        return Err(Error::InsufficientBalance);
    }
    let payout_per_claim = *claims_payout_scratch
        .get(selected)
        .ok_or(Error::InvalidDebit)?;
    let collateral_out_u128 = u128::from(input.quantity)
        .checked_mul(u128::from(payout_per_claim))
        .ok_or(Error::ArithmeticOverflow)?;
    let collateral_out =
        u64::try_from(collateral_out_u128).map_err(|_| Error::ArithmeticOverflow)?;
    if liability_before > u128::from(input.hoard_before) {
        return Err(Error::Insolvent);
    }
    let liability_after = liability_before
        .checked_sub(collateral_out_u128)
        .ok_or(Error::ArithmeticOverflow)?;
    let hoard_after = input
        .hoard_before
        .checked_sub(collateral_out)
        .ok_or(Error::Insolvent)?;
    if liability_after > u128::from(hoard_after) {
        return Err(Error::Insolvent);
    }
    let neutral =
        SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).map_err(|_| Error::SignedDelta)?;
    aggregate_delta_scratch.fill(neutral);
    let debit = SignedDeltaV3::new(DeltaDirectionV3::Debit, input.quantity)
        .map_err(|_| Error::SignedDelta)?;
    *aggregate_delta_scratch
        .get_mut(selected)
        .ok_or(Error::InvalidDebit)? = debit;
    let positions = [
        SignedDeltaPositionV3::new(input.owner, input.expected_position_revision)
            .map_err(|_| Error::SignedDelta)?,
    ];
    let position_deltas = [PositionDeltaV3::new(
        PositionDeltaInputV3 {
            position_index: 0,
            outcome: input.claim_index,
            delta: debit,
        },
        1,
        market.claim_count,
    )
    .map_err(|_| Error::SignedDelta)?];
    SignedDeltaPlanV3::encode_into(
        SignedDeltaPlanInputV3 {
            caller_role: input.caller_role,
            release_set: market.release_set,
            market: market.logical_market,
            request_id: input.request_id,
            product_record_digest: input.product_record_digest,
            semantic_basis_id: market.basis_id,
            linked_basis_record_digest: input.admission.linked_basis_record_digest(),
            expected_market_revision: input.expected_market_revision,
            claim_count: market.claim_count,
        },
        &positions,
        aggregate_delta_scratch,
        &position_deltas,
        output,
    )
    .map_err(|_| Error::SignedDelta)?;
    SignedDeltaPlanV3::decode(output).map_err(|_| Error::SignedDelta)?;
    Ok(collateral_out)
}

fn validate_nonzero(input: ProductClaimsTerminalInputV3<'_>) -> Result<()> {
    for identity in [
        input.product_record_digest,
        input.market_account,
        input.owner,
        input.request_id,
    ] {
        if identity.iter().all(|byte| *byte == 0) {
            return Err(Error::IdentityMismatch);
        }
    }
    if input.quantity == 0 {
        return Err(Error::InvalidDebit);
    }
    if input.expected_market_revision == u64::MAX || input.expected_position_revision == u64::MAX {
        return Err(Error::RevisionMismatch);
    }
    Ok(())
}

fn validate_joins(
    input: ProductClaimsTerminalInputV3<'_>,
    basis: ProductBasisV3<'_>,
    market: LiabilityBasisMarketViewV2,
    position: LiabilityBasisPositionViewV2,
) -> Result<()> {
    let admission = input.admission;
    if basis.product_id() != admission.product_id()
        || basis.result_domain_id() != admission.result_domain_id()
        || basis.coordinate_domain_id() != admission.coordinate_domain_id()
        || basis.result_unit_id() != admission.result_unit_id()
        || basis.evaluator_release_id() != admission.evaluator_release_id()
        || basis.payout_scale() != admission.payout_scale()
        || market.logical_market != admission.market_id()
        || market.release_set != admission.release_set_id()
        || market.product_instance_id != admission.product_id()
        || market.basis_id != admission.semantic_basis_id()
        || position.market_account != input.market_account
        || position.owner != input.owner
        || position.basis_id != market.basis_id
    {
        return Err(Error::IdentityMismatch);
    }
    if market.claim_count != admission.basis_width()
        || position.claim_count != admission.basis_width()
        || input.claim_index >= admission.basis_width()
    {
        return Err(Error::WidthMismatch);
    }
    if market.generation != input.expected_generation
        || market.revision != input.expected_market_revision
        || position.revision != input.expected_position_revision
    {
        return Err(Error::RevisionMismatch);
    }
    Ok(())
}

fn decode_exposure<'a>(
    input: ProductClaimsTerminalInputV3<'a>,
    basis: ProductBasisV3<'_>,
    market: LiabilityBasisMarketViewV2,
) -> Result<CompositionExposureBundleV3<'a>> {
    let exposure = CompositionExposureBundleV3::decode(
        input.composition_exposure_bytes,
        input.composition_exposure_admission,
    )
    .map_err(|_| Error::Composition)?
    .verify_execution_for(CompositionExposureExecutionExpectedV3 {
        market: market.logical_market,
        result_domain: input.admission.result_domain_id(),
        release_set: market.release_set,
        product_basis: input.admission.linked_basis_record_digest(),
        representation_basis: market.basis_id,
        product_width: basis.basis_width(),
        representation_width: market.claim_count,
    })
    .map_err(|_| Error::Composition)?;
    if exposure.bundle_id() != input.admission.exposure_id()
        || input.composition_exposure_admission.finalized_digest
            != input.admission.exposure_digest()
    {
        return Err(Error::Composition);
    }
    Ok(exposure)
}

fn evaluate(
    basis: ProductBasisV3<'_>,
    terminal: TerminalScenarioV3,
    output: &mut [u64],
) -> Result<()> {
    match (basis.kind(), terminal) {
        (BasisKindV3::CategoricalQ1, TerminalScenarioV3::Categorical(selector)) => basis
            .evaluate_categorical(selector, output)
            .map_err(|_| Error::ProductBasis),
        (
            BasisKindV3::GradedExactComplement,
            TerminalScenarioV3::Rational {
                numerator,
                denominator,
            },
        ) => basis
            .evaluate_rational(numerator, denominator, output)
            .map_err(|_| Error::ProductBasis),
        (BasisKindV3::GradedExactComplement, TerminalScenarioV3::Failure) => basis
            .evaluate_failure(output)
            .map_err(|_| Error::ProductBasis),
        _ => Err(Error::ProductBasis),
    }
}

fn validate_partition(payouts: &[u64], scale: u64) -> Result<()> {
    let mut total = 0_u64;
    for payout in payouts {
        total = total
            .checked_add(*payout)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if payouts.is_empty() || total != scale {
        return Err(Error::InvalidPartition);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::{
        liability_basis_state_v2::{
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            LiabilityBasisMarketInputV2, LiabilityBasisPositionInputV2,
            encode_liability_basis_market_into_v2, encode_liability_basis_position_into_v2,
            liability_basis_vector_width_v2,
        },
        signed_delta_v3::{DeltaDirectionV3, SignedDeltaPlanV3, SignedDeltaV3},
    };
    use dclutch_product_payoff_v2_codec::runtime_v3::{
        BasisInputV3, BasisShapeV3, BasisTermV3, basis_record_bytes_v3, compile_basis_v3,
    };
    use dclutch_rational_representation_v2_kernel::{
        ContentAdmissionV2, DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES,
        DESCRIPTOR_MAGIC_V3, DescriptorAdmissionV2,
        product_v3::{
            ProductRepresentationInputV3, ProductRuntimeProjectionV3, RepresentationContextV3,
            admit_product_representation_v3,
        },
    };
    use dclutch_representation_composition_v3_kernel::{
        CompositionExposureInputV3, CompositionExposureRowInputV3, CompositionExposureTermV3,
        composition_exposure_bytes_v3, encode_composition_exposure_v3_atomic,
    };
    use std::{vec, vec::Vec};

    const PRODUCT: [u8; 32] = [40; 32];
    const RESULT_DOMAIN: [u8; 32] = [41; 32];
    const COORDINATE_DOMAIN: [u8; 32] = [42; 32];
    const RESULT_UNIT: [u8; 32] = [43; 32];
    const EVALUATOR: [u8; 32] = [44; 32];
    const LINKED_BASIS: [u8; 32] = [45; 32];
    const SEMANTIC_BASIS: [u8; 32] = [50; 32];
    const MARKET: [u8; 32] = [60; 32];
    const MARKET_ACCOUNT: [u8; 32] = [61; 32];
    const RELEASE_SET: [u8; 32] = [62; 32];
    const OWNER: [u8; 32] = [63; 32];
    const REGISTRY: [u8; 32] = [64; 32];
    const REALM: [u8; 32] = [65; 32];
    const CUSTODY_CONTEXT: [u8; 32] = [66; 32];

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn put(output: &mut [u8], offset: usize, value: &[u8]) {
        output
            .get_mut(offset..offset + value.len())
            .expect("fixture offset")
            .copy_from_slice(value);
    }

    fn put_u32(output: &mut [u8], offset: usize, value: u32) {
        put(output, offset, &value.to_le_bytes());
    }

    fn put_u64(output: &mut [u8], offset: usize, value: u64) {
        put(output, offset, &value.to_le_bytes());
    }

    fn categorical_basis(width: u32) -> Vec<u8> {
        basis(BasisKindV3::CategoricalQ1, width, 1, &[], &[], &[])
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
        basis(
            BasisKindV3::GradedExactComplement,
            3,
            10,
            &knots,
            &terms,
            &[0, 0, 10],
        )
    }

    fn basis(
        kind: BasisKindV3,
        width: u32,
        scale: u64,
        knots: &[i128],
        terms: &[BasisTermV3],
        failures: &[u64],
    ) -> Vec<u8> {
        let bytes = basis_record_bytes_v3(
            kind,
            usize::try_from(width).expect("basis width"),
            knots.len(),
            terms.len(),
        )
        .expect("basis record bytes");
        let mut output = vec![0_u8; bytes];
        compile_basis_v3(
            BasisInputV3 {
                kind,
                product_id: PRODUCT,
                result_domain_id: RESULT_DOMAIN,
                coordinate_domain_id: COORDINATE_DOMAIN,
                result_unit_id: RESULT_UNIT,
                evaluator_release_id: EVALUATOR,
                basis_width: width,
                payout_scale: scale,
                knot_denominator: 1,
                knots,
                terms,
                failure_payouts: failures,
            },
            &mut output,
        )
        .expect("ProductBasisV3");
        output
    }

    fn graph(product_width: u32, claims_width: u32) -> Vec<u8> {
        let claims_width_usize = usize::try_from(claims_width).expect("graph width");
        let mut term_rows = Vec::with_capacity(claims_width_usize);
        for coordinate in 0..claims_width {
            let product_coordinate = if product_width == claims_width {
                coordinate
            } else if product_width == 1 || claims_width == 1 {
                0
            } else {
                coordinate
                    .checked_mul(product_width - 1)
                    .expect("fixture coordinate")
                    / (claims_width - 1)
            };
            term_rows.push([CompositionExposureTermV3 {
                product_coordinate,
                numerator: 1,
            }]);
        }
        let mut rows = Vec::with_capacity(claims_width_usize);
        for (coordinate, terms) in term_rows.iter().enumerate() {
            rows.push(CompositionExposureRowInputV3 {
                node_id: id(u8::try_from(coordinate + 100).expect("test node id")),
                denominator: 1,
                terms,
            });
        }
        let length =
            composition_exposure_bytes_v3(claims_width, claims_width).expect("exposure width");
        let mut scratch = vec![0_u8; length];
        let mut output = vec![0_u8; length];
        encode_composition_exposure_v3_atomic(
            CompositionExposureInputV3 {
                market: MARKET,
                result_domain: RESULT_DOMAIN,
                release_set: RELEASE_SET,
                product_basis: LINKED_BASIS,
                representation_basis: SEMANTIC_BASIS,
                graph_id: id(72),
                product_width,
                rows: &rows,
            },
            &mut scratch,
            &mut output,
        )
        .expect("exposure bundle");
        output
    }

    fn descriptor(width: u32) -> Vec<u8> {
        let width_usize = usize::try_from(width).expect("descriptor width");
        let mut output =
            vec![0_u8; DESCRIPTOR_HEADER_BYTES + width_usize * DESCRIPTOR_COEFFICIENT_BYTES];
        put(&mut output, 0, &DESCRIPTOR_MAGIC_V3);
        put(&mut output, 8, &3_u16.to_le_bytes());
        put(&mut output, 16, &id(71));
        put(&mut output, 48, &id(91));
        put(&mut output, 80, &id(72));
        put(&mut output, 112, &MARKET);
        put(&mut output, 144, &RELEASE_SET);
        put(&mut output, 176, &id(73));
        put(&mut output, 208, &id(74));
        put_u32(&mut output, 240, width);
        put_u64(&mut output, 248, 10);
        put_u64(&mut output, DESCRIPTOR_HEADER_BYTES, 10);
        output
    }

    fn admission(
        basis: &[u8],
        product_width: u32,
        claims_width: u32,
        scale: u64,
    ) -> (RepresentationAdmissionV3, Vec<u8>) {
        let descriptor = descriptor(claims_width);
        let graph = graph(product_width, claims_width);
        let admission = admit_product_representation_v3(ProductRepresentationInputV3 {
            product_basis_bytes: basis,
            product: ProductRuntimeProjectionV3 {
                product_id: PRODUCT,
                result_domain_id: RESULT_DOMAIN,
                coordinate_domain_id: COORDINATE_DOMAIN,
                result_unit_id: RESULT_UNIT,
                semantic_basis_id: SEMANTIC_BASIS,
                linked_basis_record_digest: LINKED_BASIS,
                evaluator_release_id: EVALUATOR,
                basis_width: product_width,
                payout_scale: scale,
            },
            descriptor_bytes: &descriptor,
            descriptor_admission: DescriptorAdmissionV2 {
                selected_descriptor_id: id(90),
                finalized_descriptor_id: id(90),
                recomputed_descriptor_digest: id(90),
                finalized_descriptor_digest: id(90),
                record_authenticated: true,
                derived_representation_authority: id(70),
                authority_derivation_authenticated: true,
            },
            graph_bytes: &graph,
            graph_admission: ContentAdmissionV2 {
                selected_graph_id: id(71),
                finalized_graph_id: id(71),
                recomputed_graph_digest: id(91),
                finalized_graph_digest: id(91),
                record_authenticated: true,
            },
            context: RepresentationContextV3 {
                market_id: MARKET,
                release_set_id: RELEASE_SET,
                claims_basis_id: SEMANTIC_BASIS,
                claims_width,
                receipt_mint: id(73),
                token_program: id(74),
                representation_authority: id(70),
            },
        })
        .expect("representation admission")
        .admission();
        (admission, graph)
    }

    struct State {
        market: Vec<u8>,
        position: Vec<u8>,
    }

    fn state(supplies: &[u64], balances: &[u64], basis_id: [u8; 32]) -> State {
        let width = u32::try_from(supplies.len()).expect("state width");
        let mut market =
            vec![
                0_u8;
                liability_basis_vector_width_v2(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, width)
                    .expect("market bytes")
            ];
        encode_liability_basis_market_into_v2(
            LiabilityBasisMarketInputV2 {
                revision: 7,
                logical_market: MARKET,
                release_set: RELEASE_SET,
                registry_program: REGISTRY,
                product_instance_id: PRODUCT,
                basis_id,
                realm_id: REALM,
                custody_context: CUSTODY_CONTEXT,
                generation: 3,
            },
            supplies,
            &mut market,
        )
        .expect("market state");
        let mut position =
            vec![
                0_u8;
                liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, width)
                    .expect("position bytes")
            ];
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision: 11,
                market_account: MARKET_ACCOUNT,
                owner: OWNER,
                basis_id,
            },
            balances,
            &mut position,
        )
        .expect("position state");
        State { market, position }
    }

    fn input<'a>(
        representation_admission: (&'a [u8], RepresentationAdmissionV3, &'a [u8]),
        state: &'a State,
        terminal: TerminalScenarioV3,
        claim_index: u32,
        quantity: u64,
        hoard_before: u64,
    ) -> ProductBasisTerminalInputV3<'a> {
        let (basis, representation, exposure) = representation_admission;
        ProductBasisTerminalInputV3 {
            product_basis_bytes: basis,
            representation,
            composition_exposure_bytes: exposure,
            composition_exposure_admission: RecordAdmissionV3 {
                selected_id: id(71),
                finalized_id: id(71),
                recomputed_digest: id(91),
                finalized_digest: id(91),
                record_authenticated: true,
            },
            product_record_digest: id(92),
            market_account: MARKET_ACCOUNT,
            market_bytes: &state.market,
            position_bytes: &state.position,
            owner: OWNER,
            request_id: id(93),
            caller_role: CallerRole::Trading,
            terminal,
            claim_index,
            quantity,
            expected_generation: 3,
            expected_market_revision: 7,
            expected_position_revision: 11,
            hoard_before,
        }
    }

    fn neutral(width: usize) -> Vec<SignedDeltaV3> {
        vec![SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("neutral"); width]
    }

    #[test]
    fn categorical_terminal_emits_one_exact_signed_debit_without_rounding() {
        let basis = categorical_basis(4);
        let (admission, exposure) = admission(&basis, 4, 4, 1);
        let state = state(&[5, 7, 3, 4], &[2, 4, 1, 0], SEMANTIC_BASIS);
        let mut payouts = [0_u64; 4];
        let mut translation = [0_u64; 4];
        let mut claims_payouts = [0_u64; 4];
        let mut deltas = neutral(4);
        let mut packet = vec![0_u8; plan_bytes(4, 1, 1).expect("packet bytes")];
        let collateral = encode_product_basis_terminal_signed_delta_v3(
            input(
                (&basis, admission, &exposure),
                &state,
                TerminalScenarioV3::Categorical(1),
                1,
                2,
                7,
            ),
            &mut payouts,
            &mut translation,
            &mut claims_payouts,
            &mut deltas,
            &mut packet,
        )
        .expect("categorical terminal");
        assert_eq!(collateral, 2);
        assert_eq!(payouts, [0, 1, 0, 0]);
        assert_eq!(claims_payouts, payouts);
        let plan = SignedDeltaPlanV3::decode(&packet).expect("signed delta");
        assert_eq!(plan.claim_count(), 4);
        assert_eq!(plan.semantic_basis_id(), SEMANTIC_BASIS);
        assert_eq!(
            plan.aggregate_delta(1)
                .expect("aggregate debit")
                .direction(),
            DeltaDirectionV3::Debit
        );
        assert_eq!(plan.aggregate_delta(1).expect("debit").magnitude(), 2);
        assert_eq!(plan.position_delta(0).expect("position debit").outcome(), 1);
    }

    #[test]
    fn graded_rational_and_failure_results_use_the_product_partition() {
        let basis = graded_basis();
        let (admission, exposure) = admission(&basis, 3, 3, 10);
        let state = state(&[5, 6, 7], &[2, 3, 4], SEMANTIC_BASIS);
        for (terminal, expected_payouts, collateral, hoard) in [
            (
                TerminalScenarioV3::Rational {
                    numerator: 5,
                    denominator: 1,
                },
                [2, 1, 7],
                14,
                65,
            ),
            (TerminalScenarioV3::Failure, [0, 0, 10], 20, 70),
        ] {
            let mut payouts = [0_u64; 3];
            let mut translation = [0_u64; 3];
            let mut claims_payouts = [0_u64; 3];
            let mut deltas = neutral(3);
            let mut packet = vec![0_u8; plan_bytes(3, 1, 1).expect("packet bytes")];
            assert_eq!(
                encode_product_basis_terminal_signed_delta_v3(
                    input(
                        (&basis, admission, &exposure),
                        &state,
                        terminal,
                        2,
                        2,
                        hoard
                    ),
                    &mut payouts,
                    &mut translation,
                    &mut claims_payouts,
                    &mut deltas,
                    &mut packet,
                ),
                Ok(collateral)
            );
            assert_eq!(payouts, expected_payouts);
            assert_eq!(claims_payouts, expected_payouts);
            assert!(SignedDeltaPlanV3::decode(&packet).is_ok());
        }
    }

    #[test]
    fn k3_claims_translate_n1_and_n258_product_partitions() {
        for (product_width, selector, expected_claims, hoard) in [
            (1_u32, 0_u32, [1_u64, 1, 1], 3_u64),
            (258_u32, 257_u32, [0_u64, 0, 1], 1_u64),
        ] {
            let basis = categorical_basis(product_width);
            let (admission, exposure) = admission(&basis, product_width, 3, 1);
            assert_eq!(admission.basis_width(), 3);
            let state = state(&[1, 1, 1], &[0, 0, 1], SEMANTIC_BASIS);
            let mut product_payouts =
                vec![0_u64; usize::try_from(product_width).expect("Product width")];
            let mut translation = [0_u64; 3];
            let mut claims_payouts = [0_u64; 3];
            let mut deltas = neutral(3);
            let mut packet = vec![0_u8; plan_bytes(3, 1, 1).expect("packet bytes")];
            assert_eq!(
                encode_product_basis_terminal_signed_delta_v3(
                    input(
                        (&basis, admission, &exposure),
                        &state,
                        TerminalScenarioV3::Categorical(selector),
                        2,
                        1,
                        hoard,
                    ),
                    &mut product_payouts,
                    &mut translation,
                    &mut claims_payouts,
                    &mut deltas,
                    &mut packet,
                ),
                Ok(1)
            );
            assert_eq!(claims_payouts, expected_claims);
            let plan = SignedDeltaPlanV3::decode(&packet).expect("K3 signed delta");
            assert_eq!(plan.claim_count(), 3);
            assert_eq!(plan.position_delta(0).expect("debit").outcome(), 2);
        }
    }

    #[test]
    fn terminal_refuses_substitution_width_balance_and_insolvency() {
        let basis = categorical_basis(4);
        let (admission, exposure) = admission(&basis, 4, 4, 1);
        let valid = state(&[5, 7, 3, 4], &[2, 4, 1, 0], SEMANTIC_BASIS);
        let wrong_basis = state(&[5, 7, 3, 4], &[2, 4, 1, 0], id(99));
        let overdrawn = state(&[5, 2, 3, 4], &[2, 4, 1, 0], SEMANTIC_BASIS);
        let mut payouts = [0_u64; 4];
        let mut translation = [0_u64; 4];
        let mut claims_payouts = [0_u64; 4];
        let mut deltas = neutral(4);
        let mut packet = vec![0xa5; plan_bytes(4, 1, 1).expect("packet bytes")];
        assert_eq!(
            encode_product_basis_terminal_signed_delta_v3(
                input(
                    (&basis, admission, &exposure),
                    &wrong_basis,
                    TerminalScenarioV3::Categorical(1),
                    1,
                    1,
                    7,
                ),
                &mut payouts,
                &mut translation,
                &mut claims_payouts,
                &mut deltas,
                &mut packet,
            ),
            Err(Error::IdentityMismatch)
        );
        assert!(packet.iter().all(|byte| *byte == 0xa5));
        assert_eq!(
            encode_product_basis_terminal_signed_delta_v3(
                input(
                    (&basis, admission, &exposure),
                    &overdrawn,
                    TerminalScenarioV3::Categorical(1),
                    1,
                    1,
                    7,
                ),
                &mut payouts,
                &mut translation,
                &mut claims_payouts,
                &mut deltas,
                &mut packet,
            ),
            Err(Error::PositionExceedsSupply)
        );
        assert_eq!(
            encode_product_basis_terminal_signed_delta_v3(
                input(
                    (&basis, admission, &exposure),
                    &valid,
                    TerminalScenarioV3::Categorical(1),
                    1,
                    1,
                    6,
                ),
                &mut payouts,
                &mut translation,
                &mut claims_payouts,
                &mut deltas,
                &mut packet,
            ),
            Err(Error::Insolvent)
        );
        let mut short = [SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("neutral"); 3];
        assert_eq!(
            encode_product_basis_terminal_signed_delta_v3(
                input(
                    (&basis, admission, &exposure),
                    &valid,
                    TerminalScenarioV3::Categorical(1),
                    1,
                    1,
                    7,
                ),
                &mut payouts,
                &mut translation,
                &mut claims_payouts,
                &mut short,
                &mut packet,
            ),
            Err(Error::WidthMismatch)
        );
    }
}
