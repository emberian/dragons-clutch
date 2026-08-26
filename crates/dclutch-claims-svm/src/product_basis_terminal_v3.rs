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

use crate::{
    CallerRole,
    liability_basis_state_v2::{LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2},
    signed_delta_v3::{
        DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SignedDeltaPlanInputV3,
        SignedDeltaPlanV3, SignedDeltaPositionV3, SignedDeltaV3, plan_bytes,
    },
};

/// Stable refusal from ProductBasisV3 terminal planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// ProductBasisV3 decoding or terminal evaluation refused.
    ProductBasis,
    /// An independently authenticated Product/representation identity differed.
    Representation,
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

/// Borrowed input for one exact terminal debit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductBasisTerminalInputV3<'a> {
    /// Exact authenticated ProductBasisV3 raw bytes.
    pub product_basis_bytes: &'a [u8],
    /// Exact Product/descriptor/DAG admission from the SVM reader.
    pub representation: RepresentationAdmissionV3,
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
/// `payout_scratch` and `aggregate_delta_scratch` must have the exact runtime
/// basis width. The returned scalar is the exact collateral payout in Product
/// payout atoms: `quantity * payout[claim_index]`. No division or additional
/// rounding occurs here. Categorical ProductBasisV3 uses its exact Q=1
/// boundary; graded ProductBasisV3 uses its canonical per-term final-floor and
/// exact-complement boundary before this function performs integer accounting.
pub fn encode_product_basis_terminal_signed_delta_v3(
    input: ProductBasisTerminalInputV3<'_>,
    payout_scratch: &mut [u64],
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
    let width = usize::try_from(basis.basis_width()).map_err(|_| Error::WidthMismatch)?;
    if payout_scratch.len() != width || aggregate_delta_scratch.len() != width {
        return Err(Error::WidthMismatch);
    }
    let expected_output = plan_bytes(basis.basis_width(), 1, 1).map_err(|_| Error::SignedDelta)?;
    if output.len() != expected_output {
        return Err(Error::SignedDelta);
    }
    evaluate(basis, input.terminal, payout_scratch)?;
    validate_partition(payout_scratch, basis.payout_scale())?;
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
        let payout = *payout_scratch.get(index).ok_or(Error::WidthMismatch)?;
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
    let payout_per_claim = *payout_scratch.get(selected).ok_or(Error::InvalidDebit)?;
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
        basis.basis_width(),
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
            linked_basis_record_digest: input.representation.linked_basis_record_digest(),
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

fn validate_nonzero(input: ProductBasisTerminalInputV3<'_>) -> Result<()> {
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
    input: ProductBasisTerminalInputV3<'_>,
    basis: ProductBasisV3<'_>,
    market: LiabilityBasisMarketViewV2,
    position: LiabilityBasisPositionViewV2,
) -> Result<()> {
    let admission = input.representation;
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
    if basis.basis_width() != admission.basis_width()
        || market.claim_count != admission.basis_width()
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
        DESCRIPTOR_MAGIC_V3, DescriptorAdmissionV2, GRAPH_HEADER_BYTES, GRAPH_MAGIC_V2,
        GRAPH_NODE_BYTES, SCALAR_BYTES, SCHEMA_VERSION_V2,
        product_v3::{
            ProductRepresentationInputV3, ProductRuntimeProjectionV3, RepresentationContextV3,
            admit_product_representation_v3,
        },
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

    fn graph(width: u32) -> Vec<u8> {
        let width = usize::try_from(width).expect("graph width");
        let mut output = vec![0_u8; GRAPH_HEADER_BYTES + GRAPH_NODE_BYTES + width * SCALAR_BYTES];
        put(&mut output, 0, &GRAPH_MAGIC_V2);
        put(&mut output, 8, &SCHEMA_VERSION_V2.to_le_bytes());
        put(&mut output, 16, &id(71));
        put(&mut output, 48, &id(72));
        put_u32(&mut output, 80, u32::try_from(width).expect("graph width"));
        put_u32(&mut output, 84, 1);
        put_u32(&mut output, 88, 0);
        put_u64(&mut output, 96, 1);
        put(&mut output, GRAPH_HEADER_BYTES, &id(72));
        *output.get_mut(GRAPH_HEADER_BYTES + 44).expect("node kind") = 0;
        put_u64(&mut output, GRAPH_HEADER_BYTES + 48, 0);
        put_u64(&mut output, GRAPH_HEADER_BYTES + GRAPH_NODE_BYTES, 1);
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

    fn admission(basis: &[u8], width: u32, scale: u64) -> RepresentationAdmissionV3 {
        let descriptor = descriptor(width);
        let graph = graph(width);
        admit_product_representation_v3(ProductRepresentationInputV3 {
            product_basis_bytes: basis,
            product: ProductRuntimeProjectionV3 {
                product_id: PRODUCT,
                result_domain_id: RESULT_DOMAIN,
                coordinate_domain_id: COORDINATE_DOMAIN,
                result_unit_id: RESULT_UNIT,
                semantic_basis_id: SEMANTIC_BASIS,
                linked_basis_record_digest: LINKED_BASIS,
                evaluator_release_id: EVALUATOR,
                basis_width: width,
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
                claims_width: width,
                receipt_mint: id(73),
                token_program: id(74),
                representation_authority: id(70),
            },
        })
        .expect("representation admission")
        .admission()
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
        basis: &'a [u8],
        representation: RepresentationAdmissionV3,
        state: &'a State,
        terminal: TerminalScenarioV3,
        claim_index: u32,
        quantity: u64,
        hoard_before: u64,
    ) -> ProductBasisTerminalInputV3<'a> {
        ProductBasisTerminalInputV3 {
            product_basis_bytes: basis,
            representation,
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
        let admission = admission(&basis, 4, 1);
        let state = state(&[5, 7, 3, 4], &[2, 4, 1, 0], SEMANTIC_BASIS);
        let mut payouts = [0_u64; 4];
        let mut deltas = neutral(4);
        let mut packet = vec![0_u8; plan_bytes(4, 1, 1).expect("packet bytes")];
        let collateral = encode_product_basis_terminal_signed_delta_v3(
            input(
                &basis,
                admission,
                &state,
                TerminalScenarioV3::Categorical(1),
                1,
                2,
                7,
            ),
            &mut payouts,
            &mut deltas,
            &mut packet,
        )
        .expect("categorical terminal");
        assert_eq!(collateral, 2);
        assert_eq!(payouts, [0, 1, 0, 0]);
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
        let admission = admission(&basis, 3, 10);
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
            let mut deltas = neutral(3);
            let mut packet = vec![0_u8; plan_bytes(3, 1, 1).expect("packet bytes")];
            assert_eq!(
                encode_product_basis_terminal_signed_delta_v3(
                    input(&basis, admission, &state, terminal, 2, 2, hoard),
                    &mut payouts,
                    &mut deltas,
                    &mut packet,
                ),
                Ok(collateral)
            );
            assert_eq!(payouts, expected_payouts);
            assert!(SignedDeltaPlanV3::decode(&packet).is_ok());
        }
    }

    #[test]
    fn terminal_refuses_substitution_width_balance_and_insolvency() {
        let basis = categorical_basis(4);
        let admission = admission(&basis, 4, 1);
        let valid = state(&[5, 7, 3, 4], &[2, 4, 1, 0], SEMANTIC_BASIS);
        let wrong_basis = state(&[5, 7, 3, 4], &[2, 4, 1, 0], id(99));
        let overdrawn = state(&[5, 2, 3, 4], &[2, 4, 1, 0], SEMANTIC_BASIS);
        let mut payouts = [0_u64; 4];
        let mut deltas = neutral(4);
        let mut packet = vec![0xa5; plan_bytes(4, 1, 1).expect("packet bytes")];
        assert_eq!(
            encode_product_basis_terminal_signed_delta_v3(
                input(
                    &basis,
                    admission,
                    &wrong_basis,
                    TerminalScenarioV3::Categorical(1),
                    1,
                    1,
                    7,
                ),
                &mut payouts,
                &mut deltas,
                &mut packet,
            ),
            Err(Error::IdentityMismatch)
        );
        assert!(packet.iter().all(|byte| *byte == 0xa5));
        assert_eq!(
            encode_product_basis_terminal_signed_delta_v3(
                input(
                    &basis,
                    admission,
                    &overdrawn,
                    TerminalScenarioV3::Categorical(1),
                    1,
                    1,
                    7,
                ),
                &mut payouts,
                &mut deltas,
                &mut packet,
            ),
            Err(Error::PositionExceedsSupply)
        );
        assert_eq!(
            encode_product_basis_terminal_signed_delta_v3(
                input(
                    &basis,
                    admission,
                    &valid,
                    TerminalScenarioV3::Categorical(1),
                    1,
                    1,
                    6,
                ),
                &mut payouts,
                &mut deltas,
                &mut packet,
            ),
            Err(Error::Insolvent)
        );
        let mut short = [SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("neutral"); 3];
        assert_eq!(
            encode_product_basis_terminal_signed_delta_v3(
                input(
                    &basis,
                    admission,
                    &valid,
                    TerminalScenarioV3::Categorical(1),
                    1,
                    1,
                    7,
                ),
                &mut payouts,
                &mut short,
                &mut packet,
            ),
            Err(Error::WidthMismatch)
        );
    }
}
