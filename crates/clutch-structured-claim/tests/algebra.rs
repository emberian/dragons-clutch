use clutch_solana_layout::{portfolio_settlement::canonical_native_portfolio_claim_id, Hash32};
use clutch_structured_claim::{
    realize_rational_shape, ClaimVector, CompositionAccumulator, CompositionDisposition,
    DeploymentBinding, Error, NativeBasisIdentity, NativeClaim, RationalCoefficient,
    RationalShape, MAX_OUTCOMES,
};
use sha2::{Digest, Sha256};

fn basis(marker: u8) -> NativeBasisIdentity {
    NativeBasisIdentity {
        market: [marker; 32],
        terms: [marker + 1; 32],
        basis_degree: 2,
        denominator: 6,
        outcome_count: 3,
    }
}

fn vector(active: [u64; 3]) -> ClaimVector {
    let mut coefficients = [0; MAX_OUTCOMES];
    coefficients[..3].copy_from_slice(&active);
    ClaimVector {
        outcome_count: 3,
        coefficients,
    }
}

fn claim(active: [u64; 3]) -> NativeClaim {
    NativeClaim {
        basis: basis(2),
        vector: vector(active),
    }
}

fn rationals(active: [(u64, u64); 3]) -> RationalShape {
    let mut coefficients = [RationalCoefficient::ZERO; MAX_OUTCOMES];
    let mut index = 0_usize;
    while index < active.len() {
        coefficients[index] = RationalCoefficient::new(active[index].0, active[index].1);
        index += 1;
    }
    RationalShape {
        outcome_count: 3,
        coefficients,
    }
}

fn deployment() -> DeploymentBinding {
    DeploymentBinding {
        wrapper_program: [11; 32],
        wrapper_program_data: [12; 32],
        wrapper_deployment_slot: 1_000,
        base_program: [13; 32],
        base_program_data: [14; 32],
        base_deployment_slot: 2_000,
        token_2022_program: [15; 32],
        token_2022_program_data: [16; 32],
        token_2022_deployment_slot: 3_000,
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[test]
fn exact_rationals_have_one_minimal_integral_realization() {
    let half = realize_rational_shape(&rationals([(1, 2), (1, 1), (2, 1)])).unwrap();
    assert_eq!(&half.claim.coefficients[..3], &[1, 2, 4]);
    assert_eq!(half.wrapper_atoms_per_display_lot, 1);
    assert_eq!(half.target_units_per_display_lot, 2);
    assert_eq!(half.backing.cash_per_wrapper, 1);
    assert_eq!(&half.backing.residual_eggs_per_wrapper[..3], &[0, 1, 3]);

    let doubled = realize_rational_shape(&rationals([(2, 1), (4, 1), (6, 1)])).unwrap();
    assert_eq!(&doubled.claim.coefficients[..3], &[1, 2, 3]);
    assert_eq!(doubled.wrapper_atoms_per_display_lot, 2);
    assert_eq!(doubled.target_units_per_display_lot, 1);
}

#[test]
fn rational_admission_refuses_ambiguity_padding_and_width_overflow() {
    let mut non_reduced = rationals([(2, 4), (1, 1), (2, 1)]);
    assert_eq!(
        realize_rational_shape(&non_reduced),
        Err(Error::NonCanonicalRational)
    );
    non_reduced.coefficients[0] = RationalCoefficient::new(0, 7);
    assert_eq!(
        realize_rational_shape(&non_reduced),
        Err(Error::NonCanonicalRational)
    );
    let mut padded = rationals([(1, 2), (1, 1), (2, 1)]);
    padded.coefficients[3] = RationalCoefficient::new(1, 1);
    assert_eq!(
        realize_rational_shape(&padded),
        Err(Error::NonCanonicalPadding)
    );
    let overflow = rationals([(1, u64::MAX), (1, u64::MAX - 1), (1, 4_294_967_291)]);
    assert_eq!(
        realize_rational_shape(&overflow),
        Err(Error::ArithmeticOverflow)
    );
}

#[test]
fn degenerate_or_nonprimitive_products_refuse() {
    for (candidate, expected) in [
        ([0, 0, 0], Error::ZeroClaim),
        ([0, 1, 0], Error::SingleEggClaim),
        ([1, 1, 1], Error::CompleteSetClaim),
        ([2, 4, 6], Error::NonPrimitiveClaim),
    ] {
        assert_eq!(vector(candidate).validate(), Err(expected));
    }
}

#[test]
fn identity_preimages_match_frozen_vectors_and_bind_every_deployment_locus() {
    let native_id = digest(&claim([1, 2, 4]).identity_preimage().unwrap());
    assert_eq!(
        native_id,
        canonical_native_portfolio_claim_id(
            Hash32::new([2; 32]).unwrap(),
            Hash32::new([3; 32]).unwrap(),
            2,
            6,
            3,
            &vector([1, 2, 4]).coefficients,
        )
        .0
    );
    let product = digest(&deployment().product_preimage(native_id).unwrap());
    let product_for =
        |binding: DeploymentBinding| digest(&binding.product_preimage(native_id).unwrap());
    let mut changed = deployment();
    changed.wrapper_program_data[0] += 1;
    assert_ne!(product, product_for(changed));
    changed = deployment();
    changed.base_deployment_slot += 1;
    assert_ne!(product, product_for(changed));
    changed = deployment();
    changed.token_2022_program_data[0] += 1;
    assert_ne!(product, product_for(changed));
    let mut aliased = deployment();
    aliased.base_program_data = aliased.base_program;
    assert_eq!(
        aliased.product_preimage(native_id),
        Err(Error::InvalidIdentity)
    );
}

#[test]
fn flat_composition_is_associative_and_exposes_complete_sets() {
    let a = claim([1, 2, 4]);
    let b = claim([2, 1, 3]);
    let mut direct = CompositionAccumulator::new(a.basis).unwrap();
    direct.push(&a, 2).unwrap();
    direct.push(&b, 3).unwrap();
    let flat = direct.finish().unwrap();
    assert_eq!(&flat.exact_eggs[..3], &[8, 7, 17]);
    assert_eq!(flat.output_cash_atoms, 7);
    assert_eq!(&flat.output_residual_eggs[..3], &[1, 0, 10]);
    assert_eq!(flat.disposition, CompositionDisposition::TransferableWrapper);

    let mut left = CompositionAccumulator::new(a.basis).unwrap();
    left.push(&a, 2).unwrap();
    let mut right = CompositionAccumulator::new(a.basis).unwrap();
    right.push(&b, 3).unwrap();
    left.combine(&right).unwrap();
    assert_eq!(left.finish().unwrap(), flat);
}

#[test]
fn constant_composition_exits_as_cash_and_refusals_do_not_mutate() {
    let a = claim([1, 2, 3]);
    let b = claim([2, 1, 0]);
    let mut accumulator = CompositionAccumulator::new(a.basis).unwrap();
    accumulator.push(&a, 1).unwrap();
    accumulator.push(&b, 1).unwrap();
    let output = accumulator.finish().unwrap();
    assert_eq!(&output.exact_eggs[..3], &[3, 3, 3]);
    assert_eq!(output.output_cash_atoms, 3);
    assert_eq!(output.disposition, CompositionDisposition::CompleteSetCash);

    let before = accumulator;
    let mut foreign = b;
    foreign.basis = basis(9);
    assert_eq!(accumulator.push(&foreign, 1), Err(Error::DifferentBasis));
    assert_eq!(accumulator, before);
}
