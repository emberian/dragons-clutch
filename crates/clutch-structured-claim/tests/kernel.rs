use clutch_kernel::{BasisMode, MarketState, PayoutSet, PayoutVector, MAX_PAYOUTS};
use clutch_solana_layout::{portfolio_settlement::canonical_native_portfolio_claim_id, Hash32};
use clutch_structured_claim::{
    realize_rational_shape, BackingVault, ClaimVector, CompositionAccumulator,
    CompositionDisposition, DeploymentBinding, Error, HolderAssets, MarketLedger, MarketPhase,
    NativeBasisIdentity, NativeClaim, RationalCoefficient, RationalShape, ResolvedWeights,
    StructuredClaimMachine, WrapperState, MAX_OUTCOMES,
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

fn active_market() -> MarketLedger {
    let mut supply = [0; MAX_OUTCOMES];
    supply[..3].copy_from_slice(&[100, 100, 100]);
    let mut payout_vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
    let mut one_hot = [0; MAX_OUTCOMES];
    one_hot[0] = 6;
    payout_vectors[0] = PayoutVector::new(6, one_hot);
    let payouts = PayoutSet::new(1, 3, payout_vectors);
    let mut base = MarketState::new(3, BasisMode::DerivedBasis, payouts, 100).unwrap();
    base.total_supply = supply;
    base.check_invariants().unwrap();
    MarketLedger::from_base(basis(2), base).unwrap()
}

fn weights(active: [u64; 3]) -> ResolvedWeights {
    let mut values = [0; MAX_OUTCOMES];
    values[..3].copy_from_slice(&active);
    ResolvedWeights {
        denominator: 6,
        weights: values,
    }
}

fn resolve_market(market: &mut MarketLedger, active: [u64; 3]) {
    let resolved = weights(active);
    market
        .base
        .resolve_with_vector(PayoutVector::new(resolved.denominator, resolved.weights))
        .unwrap();
    market.validate().unwrap();
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

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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

    for index in 0..3 {
        let coefficient = rationals([(1, 2), (1, 1), (2, 1)]).coefficients[index];
        assert_eq!(
            u128::from(half.wrapper_atoms_per_display_lot)
                * u128::from(half.claim.coefficients[index])
                * u128::from(coefficient.denominator),
            u128::from(half.target_units_per_display_lot) * u128::from(coefficient.numerator)
        );
    }
}

#[test]
fn rational_admission_refuses_ambiguity_and_width_overflow() {
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
fn zero_single_egg_complete_set_and_nonprimitive_products_refuse() {
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
fn identity_preimages_match_frozen_live_vectors_and_bind_deployments() {
    let native = claim([1, 2, 4]).identity_preimage().unwrap();
    let native_id = digest(&native);
    assert_eq!(
        hex(native_id),
        "41885f4a143807479f1e3fa00752c1ce4e1ca7e56834fae084204e3e63831261"
    );
    let live_id = canonical_native_portfolio_claim_id(
        Hash32::new([2; 32]).unwrap(),
        Hash32::new([3; 32]).unwrap(),
        2,
        6,
        3,
        &vector([1, 2, 4]).coefficients,
    );
    assert_eq!(native_id, live_id.0);
    let product = digest(&deployment().product_preimage(native_id).unwrap());
    assert_eq!(
        hex(product),
        "6877d6eab291a37968a705de78217233cff212c58e3f1ce9872c51e669392dd6"
    );

    let product_for =
        |binding: DeploymentBinding| digest(&binding.product_preimage(native_id).unwrap());
    let mut changed = deployment();
    changed.wrapper_program[0] += 1;
    assert_ne!(product, product_for(changed));
    changed = deployment();
    changed.wrapper_program_data[0] += 1;
    assert_ne!(product, product_for(changed));
    changed = deployment();
    changed.wrapper_deployment_slot += 1;
    assert_ne!(product, product_for(changed));
    changed = deployment();
    changed.base_program[0] += 1;
    assert_ne!(product, product_for(changed));
    changed = deployment();
    changed.base_program_data[0] += 1;
    assert_ne!(product, product_for(changed));
    changed = deployment();
    changed.base_deployment_slot += 1;
    assert_ne!(product, product_for(changed));
    changed = deployment();
    changed.token_2022_program[0] += 1;
    assert_ne!(product, product_for(changed));
    changed = deployment();
    changed.token_2022_program_data[0] += 1;
    assert_ne!(product, product_for(changed));
    changed = deployment();
    changed.token_2022_deployment_slot += 1;
    assert_ne!(product, product_for(changed));
    let mut aliased = deployment();
    aliased.base_program_data = aliased.base_program;
    assert_eq!(
        aliased.product_preimage(native_id),
        Err(Error::InvalidIdentity)
    );

    let mut equal_bytes_in_distinct_domains = basis(2);
    equal_bytes_in_distinct_domains.terms = equal_bytes_in_distinct_domains.market;
    assert_eq!(equal_bytes_in_distinct_domains.validate(), Ok(()));
}

#[test]
fn flat_composition_is_exact_associative_and_exposes_complete_sets() {
    let a = claim([1, 2, 4]);
    let b = claim([2, 1, 3]);
    let mut direct = CompositionAccumulator::new(a.basis).unwrap();
    direct.push(&a, 2).unwrap();
    direct.push(&b, 3).unwrap();
    let flat = direct.finish().unwrap();
    assert_eq!(&flat.exact_eggs[..3], &[8, 7, 17]);
    assert_eq!(&flat.primitive[..3], &[8, 7, 17]);
    assert_eq!(flat.primitive_units, 1);
    assert_eq!(flat.input_cash_atoms, 5);
    assert_eq!(flat.additional_complete_sets_to_merge, 2);
    assert_eq!(flat.output_cash_atoms, 7);
    assert_eq!(&flat.output_residual_eggs[..3], &[1, 0, 10]);
    assert_eq!(
        flat.disposition,
        CompositionDisposition::TransferableWrapper
    );

    let mut left = CompositionAccumulator::new(a.basis).unwrap();
    left.push(&a, 2).unwrap();
    let mut right = CompositionAccumulator::new(a.basis).unwrap();
    right.push(&b, 3).unwrap();
    left.combine(&right).unwrap();
    assert_eq!(left.finish().unwrap(), flat);

    let intermediate = NativeClaim {
        basis: flat.basis,
        vector: ClaimVector {
            outcome_count: 3,
            coefficients: flat.primitive,
        },
    };
    let mut regrouped = CompositionAccumulator::new(a.basis).unwrap();
    regrouped.push(&intermediate, 5).unwrap();
    let mut scaled_direct = CompositionAccumulator::new(a.basis).unwrap();
    scaled_direct.push(&a, 10).unwrap();
    scaled_direct.push(&b, 15).unwrap();
    let regrouped = regrouped.finish().unwrap();
    let scaled_direct = scaled_direct.finish().unwrap();
    assert_eq!(regrouped.exact_eggs, scaled_direct.exact_eggs);
    assert_eq!(regrouped.primitive, scaled_direct.primitive);
    assert_eq!(regrouped.primitive_units, scaled_direct.primitive_units);
    assert_eq!(regrouped.output_cash_atoms, scaled_direct.output_cash_atoms);
    assert_eq!(
        regrouped.input_cash_atoms + regrouped.additional_complete_sets_to_merge,
        regrouped.output_cash_atoms
    );
    assert_eq!(
        scaled_direct.input_cash_atoms + scaled_direct.additional_complete_sets_to_merge,
        scaled_direct.output_cash_atoms
    );

    let mut normalizing = CompositionAccumulator::new(a.basis).unwrap();
    normalizing.push(&a, 2).unwrap();
    normalizing.push(&b, 2).unwrap();
    let normalized = normalizing.finish().unwrap();
    assert_eq!(&normalized.exact_eggs[..3], &[6, 6, 14]);
    assert_eq!(&normalized.primitive[..3], &[3, 3, 7]);
    assert_eq!(normalized.primitive_units, 2);
    let normalized_claim = NativeClaim {
        basis: normalized.basis,
        vector: ClaimVector {
            outcome_count: 3,
            coefficients: normalized.primitive,
        },
    };
    let mut normalized_regrouping = CompositionAccumulator::new(a.basis).unwrap();
    normalized_regrouping
        .push(&normalized_claim, normalized.primitive_units)
        .unwrap();
    let normalized_regrouping = normalized_regrouping.finish().unwrap();
    assert_eq!(normalized_regrouping.exact_eggs, normalized.exact_eggs);
    assert_eq!(normalized_regrouping.primitive, normalized.primitive);
    assert_eq!(
        normalized_regrouping.primitive_units,
        normalized.primitive_units
    );
}

#[test]
fn constant_composition_exits_as_cash_and_cross_basis_refuses_transactionally() {
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

    let before = accumulator;
    assert_eq!(
        accumulator.push(&a, u64::MAX),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(accumulator, before);
}

#[test]
fn canonical_wrap_and_unwind_are_exact_and_supply_neutral_to_base() {
    let market = active_market();
    let original_market = market;
    let mut machine = StructuredClaimMachine::new(claim([1, 2, 4])).unwrap();
    let mut holder = HolderAssets {
        cash_atoms: 10,
        internal: {
            let mut value = [0; MAX_OUTCOMES];
            value[..3].copy_from_slice(&[10, 20, 40]);
            value
        },
        wrapper_atoms: 0,
    };
    let original_holder = holder;
    machine.wrap_canonical(&market, &mut holder, 2).unwrap();
    assert_eq!(machine.wrapper.actual_supply, 2);
    assert_eq!(machine.vault.cash_atoms, 2);
    assert_eq!(&machine.vault.internal[..3], &[0, 2, 6]);
    assert_eq!(market, original_market);
    machine.unwind_canonical(&market, &mut holder, 2).unwrap();
    assert_eq!(holder, original_holder);
    assert_eq!(machine.vault, BackingVault::EMPTY);
    assert_eq!(machine.wrapper.actual_supply, 0);
}

#[test]
fn full_wrap_and_unwind_are_the_same_canonical_vault_state() {
    let mut market = active_market();
    let original_market = market;
    let mut machine = StructuredClaimMachine::new(claim([1, 2, 4])).unwrap();
    let mut holder = HolderAssets {
        cash_atoms: 0,
        internal: {
            let mut value = [0; MAX_OUTCOMES];
            value[..3].copy_from_slice(&[10, 20, 40]);
            value
        },
        wrapper_atoms: 0,
    };
    let original_holder = holder;
    machine.wrap_full(&mut market, &mut holder, 3).unwrap();
    assert_eq!(market.base.collateral, 97);
    assert_eq!(&market.base.total_supply[..3], &[97, 97, 97]);
    assert_eq!(machine.vault.cash_atoms, 3);
    assert_eq!(&machine.vault.internal[..3], &[0, 3, 9]);

    let mut canonical_machine = StructuredClaimMachine::new(claim([1, 2, 4])).unwrap();
    let mut canonical_holder = HolderAssets {
        cash_atoms: 3,
        internal: {
            let mut value = [0; MAX_OUTCOMES];
            value[..3].copy_from_slice(&[0, 3, 9]);
            value
        },
        wrapper_atoms: 0,
    };
    canonical_machine
        .wrap_canonical(&original_market, &mut canonical_holder, 3)
        .unwrap();
    assert_eq!(machine, canonical_machine);

    machine.unwind_full(&mut market, &mut holder, 3).unwrap();
    assert_eq!(market, original_market);
    assert_eq!(holder, original_holder);
    assert_eq!(machine.vault, BackingVault::EMPTY);
}

#[test]
fn resolution_race_refuses_full_unwind_but_not_canonical_ownership_exit() {
    let mut market = active_market();
    let mut machine = StructuredClaimMachine::new(claim([1, 2, 4])).unwrap();
    let mut holder = HolderAssets {
        cash_atoms: 0,
        internal: {
            let mut value = [0; MAX_OUTCOMES];
            value[..3].copy_from_slice(&[2, 4, 8]);
            value
        },
        wrapper_atoms: 0,
    };
    machine.wrap_full(&mut market, &mut holder, 2).unwrap();
    resolve_market(&mut market, [1, 2, 3]);
    let before = (market, machine, holder);
    assert_eq!(
        machine.unwind_full(&mut market, &mut holder, 1),
        Err(Error::NotActive)
    );
    assert_eq!((market, machine, holder), before);
    machine.unwind_canonical(&market, &mut holder, 1).unwrap();
    assert_eq!(holder.cash_atoms, 1);
    assert_eq!(&holder.internal[..3], &[0, 1, 3]);
}

#[test]
fn canonical_wrapping_remains_exact_after_resolution() {
    let mut market = active_market();
    resolve_market(&mut market, [1, 2, 3]);
    let mut machine = StructuredClaimMachine::new(claim([1, 2, 4])).unwrap();
    let mut holder = HolderAssets {
        cash_atoms: 1,
        internal: {
            let mut value = [0; MAX_OUTCOMES];
            value[..3].copy_from_slice(&[0, 1, 3]);
            value
        },
        wrapper_atoms: 0,
    };
    machine.wrap_canonical(&market, &mut holder, 1).unwrap();
    assert_eq!(machine.wrapper.actual_supply, 1);
    machine.unwind_canonical(&market, &mut holder, 1).unwrap();
    assert_eq!(holder.cash_atoms, 1);
    assert_eq!(&holder.internal[..3], &[0, 1, 3]);
}

#[test]
fn zero_floor_full_routes_need_no_active_split_or_merge() {
    let mut market = active_market();
    resolve_market(&mut market, [1, 2, 3]);
    let zero_floor_claim = claim([0, 1, 2]);
    let mut machine = StructuredClaimMachine::new(zero_floor_claim).unwrap();
    let mut holder = HolderAssets {
        cash_atoms: 0,
        internal: {
            let mut value = [0; MAX_OUTCOMES];
            value[..3].copy_from_slice(&[0, 2, 4]);
            value
        },
        wrapper_atoms: 0,
    };
    let market_before = market;
    machine.wrap_full(&mut market, &mut holder, 2).unwrap();
    assert_eq!(market, market_before);
    assert_eq!(machine.vault.cash_atoms, 0);
    assert_eq!(&machine.vault.internal[..3], &[0, 2, 4]);
    machine.unwind_full(&mut market, &mut holder, 2).unwrap();
    assert_eq!(market, market_before);
    assert_eq!(&holder.internal[..3], &[0, 2, 4]);
}

#[test]
fn direct_burn_creates_no_entitlement_and_compaction_pays_nobody() {
    let mut market = active_market();
    let mut machine = StructuredClaimMachine::new(claim([1, 2, 4])).unwrap();
    let mut holder = HolderAssets {
        cash_atoms: 4,
        internal: {
            let mut value = [0; MAX_OUTCOMES];
            value[..3].copy_from_slice(&[0, 4, 12]);
            value
        },
        wrapper_atoms: 0,
    };
    machine.wrap_canonical(&market, &mut holder, 4).unwrap();
    machine.direct_burn(&market, &mut holder, 1).unwrap();
    assert_eq!(machine.wrapper.actual_supply, 3);
    assert_eq!(machine.vault.cash_atoms, 4);
    assert_eq!(machine.retire(&market), Err(Error::RetirementBlocked));
    let holder_before = holder;
    let donation = machine.compact_donation(&mut market).unwrap();
    assert_eq!(donation.cash_to_hoard, 1);
    assert_eq!(&donation.eggs_destroyed[..3], &[0, 1, 3]);
    assert_eq!(holder, holder_before);
    assert_eq!(machine.vault.cash_atoms, 3);
    assert_eq!(&machine.vault.internal[..3], &[0, 3, 9]);

    machine.direct_burn(&market, &mut holder, 3).unwrap();
    machine.compact_donation(&mut market).unwrap();
    machine.retire(&market).unwrap();
    assert_eq!(machine.wrapper.actual_supply, 0);
    assert_eq!(machine.vault, BackingVault::EMPTY);
    assert_eq!(machine.compact_donation(&mut market), Err(Error::Retired));
}

#[test]
fn aggregate_terminal_redemption_has_one_named_exact_lot_and_no_rounding() {
    let mut market = active_market();
    let mut machine = StructuredClaimMachine::new(claim([1, 2, 4])).unwrap();
    let mut holder = HolderAssets {
        cash_atoms: 0,
        internal: {
            let mut value = [0; MAX_OUTCOMES];
            value[..3].copy_from_slice(&[6, 12, 24]);
            value
        },
        wrapper_atoms: 0,
    };
    machine.wrap_full(&mut market, &mut holder, 6).unwrap();
    resolve_market(&mut market, [1, 2, 3]);
    assert_eq!(machine.terminal_lot(&market).unwrap(), 6);
    let before = (market, machine, holder);
    assert_eq!(
        machine.redeem_terminal(&mut market, &mut holder, 1),
        Err(Error::InexactRedemption)
    );
    assert_eq!((market, machine, holder), before);
    assert_eq!(
        machine
            .redeem_terminal(&mut market, &mut holder, 6)
            .unwrap(),
        17
    );
    assert_eq!(holder.cash_atoms, 17);
    assert_eq!(machine.wrapper.actual_supply, 0);
    assert_eq!(machine.vault, BackingVault::EMPTY);
    assert_eq!(market.base.collateral, 83);
    assert_eq!(&market.base.total_supply[..3], &[94, 88, 76]);
}

#[test]
fn every_refusal_is_transactional_and_undercoverage_never_loads() {
    let mut market = active_market();
    let mut machine = StructuredClaimMachine::new(claim([1, 2, 4])).unwrap();
    let mut holder = HolderAssets {
        cash_atoms: u64::MAX,
        internal: {
            let mut value = [0; MAX_OUTCOMES];
            value[..3].copy_from_slice(&[u64::MAX, u64::MAX, u64::MAX]);
            value
        },
        wrapper_atoms: 0,
    };
    market.base.total_supply[..3].copy_from_slice(&[u64::MAX; 3]);
    market.base.collateral = u64::MAX;
    market.base.check_invariants().unwrap();
    let before = (market, machine, holder);
    assert_eq!(
        machine.wrap_canonical(&market, &mut holder, u64::MAX),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!((market, machine, holder), before);

    let under = StructuredClaimMachine::restore(
        claim([1, 2, 4]),
        WrapperState {
            actual_supply: 2,
            retired: false,
        },
        BackingVault {
            cash_atoms: 2,
            internal: [0; MAX_OUTCOMES],
        },
        &market,
    );
    assert_eq!(under, Err(Error::UnderCollateralized));

    let mut inconsistent_holder = HolderAssets {
        cash_atoms: 1,
        internal: {
            let mut value = [0; MAX_OUTCOMES];
            value[0] = u64::MAX;
            value
        },
        wrapper_atoms: 0,
    };
    let machine_before = machine;
    let holder_before = inconsistent_holder;
    let small_market = active_market();
    assert_eq!(
        machine.wrap_canonical(&small_market, &mut inconsistent_holder, 1),
        Err(Error::InvariantViolation)
    );
    assert_eq!(machine, machine_before);
    assert_eq!(inconsistent_holder, holder_before);

    let mut donation_market = active_market();
    donation_market.base.collateral = u64::MAX;
    donation_market.base.check_invariants().unwrap();
    let mut donation_machine = StructuredClaimMachine::restore(
        claim([1, 2, 4]),
        WrapperState {
            actual_supply: 0,
            retired: false,
        },
        BackingVault {
            cash_atoms: 1,
            internal: [0; MAX_OUTCOMES],
        },
        &donation_market,
    )
    .unwrap();
    let before = (donation_market, donation_machine);
    assert_eq!(
        donation_machine.compact_donation(&mut donation_market),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!((donation_market, donation_machine), before);
}

#[test]
fn complete_set_compression_preserves_every_small_simplex_payoff() {
    for a in 0_u64..=4 {
        for b in 0_u64..=4 {
            for c in 0_u64..=4 {
                let candidate = vector([a, b, c]);
                let Ok(backing) = candidate.backing_plan() else {
                    continue;
                };
                for w0 in 0_u64..=6 {
                    for w1 in 0_u64..=(6 - w0) {
                        let w2 = 6 - w0 - w1;
                        let original = a * w0 + b * w1 + c * w2;
                        let compressed = backing.cash_per_wrapper * 6
                            + backing.residual_eggs_per_wrapper[0] * w0
                            + backing.residual_eggs_per_wrapper[1] * w1
                            + backing.residual_eggs_per_wrapper[2] * w2;
                        assert_eq!(compressed, original);
                    }
                }
            }
        }
    }
}

#[test]
fn every_supported_width_has_canonical_padding_backing_and_composition() {
    for count in [2_u8, 4, 8, 16] {
        let mut coefficients = [0; MAX_OUTCOMES];
        let mut index = 0_usize;
        while index < usize::from(count) {
            coefficients[index] = 1;
            index += 1;
        }
        coefficients[usize::from(count) - 1] = 2;
        let vector = ClaimVector {
            outcome_count: count,
            coefficients,
        };
        let mut width_basis = basis(count);
        width_basis.outcome_count = count;
        let width_claim = NativeClaim {
            basis: width_basis,
            vector,
        };
        width_claim.validate().unwrap();
        let backing = vector.backing_plan().unwrap();
        assert_eq!(backing.cash_per_wrapper, 1);
        assert!(backing.residual_eggs_per_wrapper[usize::from(count)..]
            .iter()
            .all(|amount| *amount == 0));
        assert!(width_claim.identity_preimage().is_ok());

        let mut composition = CompositionAccumulator::new(width_basis).unwrap();
        composition.push(&width_claim, 3).unwrap();
        let output = composition.finish().unwrap();
        assert_eq!(output.primitive, coefficients);
        assert_eq!(output.primitive_units, 3);
        assert_eq!(
            output.input_cash_atoms + output.additional_complete_sets_to_merge,
            output.output_cash_atoms
        );
    }
}

#[test]
fn wrapper_transfer_changes_only_bearer_ownership() {
    let market = active_market();
    let machine = StructuredClaimMachine::restore(
        claim([1, 2, 4]),
        WrapperState {
            actual_supply: 5,
            retired: false,
        },
        BackingVault {
            cash_atoms: 5,
            internal: {
                let mut value = [0; MAX_OUTCOMES];
                value[..3].copy_from_slice(&[0, 5, 15]);
                value
            },
        },
        &market,
    )
    .unwrap();
    let mut from = HolderAssets {
        wrapper_atoms: 3,
        ..HolderAssets::EMPTY
    };
    let mut to = HolderAssets {
        wrapper_atoms: 2,
        ..HolderAssets::EMPTY
    };
    let machine_before = machine;
    machine
        .transfer_wrappers(&market, &mut from, &mut to, 2)
        .unwrap();
    assert_eq!((from.wrapper_atoms, to.wrapper_atoms), (1, 4));
    assert_eq!(machine, machine_before);
    assert_eq!(machine.wrapper.actual_supply, 5);
    assert_eq!(market.phase(), MarketPhase::Active);

    let mut corrupt = machine;
    corrupt.vault.cash_atoms = 0;
    let before = (from, to);
    assert_eq!(
        corrupt.transfer_wrappers(&market, &mut from, &mut to, 1),
        Err(Error::UnderCollateralized)
    );
    assert_eq!((from, to), before);
}
