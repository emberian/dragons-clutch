use clutch_price_measure::{
    verify_quantized_price_measure_v3_degree_zero, verify_quantized_price_measure_v3_smooth,
    AdapterBindingsV3, BindingFieldV3, ErrorV3, PriceVectorV3, QuantizedAtomWitnessV3,
};
use clutch_product_series::*;
use sha2::{Digest, Sha256};

const BURN_REGISTRY_VALUE: u16 = 7;

fn id(byte: u8) -> ContentId {
    ContentId::from_bytes([byte; 32])
}

fn independent_digest(domain: &[u8], body: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(body);
    hasher.finalize().into()
}

macro_rules! assert_wrong_lengths {
    ($type:ty, $bytes:expr) => {{
        let bytes = &$bytes;
        assert_eq!(
            <$type>::decode(&bytes[..bytes.len() - 1]),
            Err(Error::Truncated)
        );
        let mut extended = bytes.to_vec();
        extended.push(0);
        assert_eq!(<$type>::decode(&extended), Err(Error::TrailingBytes));
    }};
}

fn basis() -> NativeClaimBasisV1 {
    let mut knots = [0; MAX_OUTCOMES];
    knots[..3].copy_from_slice(&[0, 8, 16]);
    NativeClaimBasisV1 {
        basis_degree: 2,
        outcome_count: 4,
        payout_count: 0,
        knot_count: 3,
        uniform_log2_spacing: 3,
        ambiguity_policy_registry_value: 1,
        edge_policy_registry_value: 1,
        denominator: 1_000,
        payout_weights: [[0; MAX_OUTCOMES]; MAX_PAYOUTS],
        payout_map: [PAYOUT_MAP_UNUSED; MAX_OUTCOMES],
        knots,
    }
}

fn categorical_basis() -> NativeClaimBasisV1 {
    let mut payout_weights = [[0; MAX_OUTCOMES]; MAX_PAYOUTS];
    let mut index = 0usize;
    while index < 4 {
        payout_weights[index][index] = 1_000;
        index += 1;
    }
    let mut payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    payout_map[..4].copy_from_slice(&[0, 1, 2, 3]);
    let mut knots = [0; MAX_OUTCOMES];
    knots[..3].copy_from_slice(&[100, 200, 300]);
    NativeClaimBasisV1 {
        basis_degree: 0,
        outcome_count: 4,
        payout_count: 4,
        knot_count: 3,
        uniform_log2_spacing: UNIFORM_SPACING_NONE,
        ambiguity_policy_registry_value: 1,
        edge_policy_registry_value: 1,
        denominator: 1_000,
        payout_weights,
        payout_map,
        knots,
    }
}

fn finite_payout_basis() -> NativeClaimBasisV1 {
    let mut payout_weights = [[0; MAX_OUTCOMES]; MAX_PAYOUTS];
    payout_weights[0][0] = 1_000;
    payout_weights[1][1] = 500;
    payout_weights[1][2] = 500;
    payout_weights[2][3] = 1_000;
    let mut payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    payout_map[..4].copy_from_slice(&[0, 1, 1, 2]);
    let mut knots = [0; MAX_OUTCOMES];
    knots[..3].copy_from_slice(&[100, 200, 300]);
    NativeClaimBasisV1 {
        basis_degree: 0,
        outcome_count: 4,
        payout_count: 3,
        knot_count: 3,
        uniform_log2_spacing: UNIFORM_SPACING_NONE,
        ambiguity_policy_registry_value: 1,
        edge_policy_registry_value: 1,
        denominator: 1_000,
        payout_weights,
        payout_map,
        knots,
    }
}

fn linear_basis() -> NativeClaimBasisV1 {
    let mut knots = [0; MAX_OUTCOMES];
    knots[..4].copy_from_slice(&[0, 8, 16, 24]);
    NativeClaimBasisV1 {
        basis_degree: 1,
        outcome_count: 4,
        payout_count: 0,
        knot_count: 4,
        uniform_log2_spacing: 3,
        ambiguity_policy_registry_value: 1,
        edge_policy_registry_value: 1,
        denominator: 1_000,
        payout_weights: [[0; MAX_OUTCOMES]; MAX_PAYOUTS],
        payout_map: [PAYOUT_MAP_UNUSED; MAX_OUTCOMES],
        knots,
    }
}

fn cubic_basis() -> NativeClaimBasisV1 {
    let mut knots = [0; MAX_OUTCOMES];
    knots[..2].copy_from_slice(&[0, 8]);
    NativeClaimBasisV1 {
        basis_degree: 3,
        outcome_count: 4,
        payout_count: 0,
        knot_count: 2,
        uniform_log2_spacing: 3,
        ambiguity_policy_registry_value: 1,
        edge_policy_registry_value: 1,
        denominator: 1_000,
        payout_weights: [[0; MAX_OUTCOMES]; MAX_PAYOUTS],
        payout_map: [PAYOUT_MAP_UNUSED; MAX_OUTCOMES],
        knots,
    }
}

fn recovery() -> EvidenceOnlyRecoveryPolicyV1 {
    let mut attempts = [RecoveryAttemptV1::ZERO; MAX_RECOVERY_ATTEMPTS];
    attempts[0] = RecoveryAttemptV1 {
        repair_generation_delta: 0,
        opens_after_primary_maturity_buckets: 0,
        closes_after_primary_maturity_buckets: 2,
    };
    attempts[1] = RecoveryAttemptV1 {
        repair_generation_delta: 1,
        opens_after_primary_maturity_buckets: 2,
        closes_after_primary_maturity_buckets: 5,
    };
    EvidenceOnlyRecoveryPolicyV1 {
        attempt_count: 2,
        attempts,
    }
}

fn template_for(basis: &NativeClaimBasisV1) -> ProductTemplateV4 {
    ProductTemplateV4 {
        source_plane_contract_id: id(1),
        source_spec_id: id(2),
        summary_program_id: id(3),
        native_claim_basis_id: basis.id().unwrap(),
        evidence_only_recovery_policy_id: recovery().id().unwrap(),
        compiler_release_id: id(4),
        statistic_registry_value: 11,
        coverage_policy_registry_value: 12,
        window_span_buckets: 4,
        primary_maturity_grace_buckets: 2,
        base_repair_generation: 10,
        coverage_policy_parameter: 0,
    }
}

fn template() -> ProductTemplateV4 {
    template_for(&basis())
}

fn price_policy() -> PriceMeasurePolicyV1 {
    PriceMeasurePolicyV1 {
        checker_release_id: id(30),
        checker_version: 3,
        quantized_semantics_version: 1,
        minimum_basis_degree: 0,
        maximum_basis_degree: 3,
        maximum_outcome_count: 16,
        maximum_atom_count: 16,
        maximum_payout_denominator: u64::MAX,
        maximum_witness_denominator: u64::MAX,
        maximum_price_scale: u64::MAX / 16,
    }
}

fn genesis() -> MarketGenesisProfileV2 {
    MarketGenesisProfileV2 {
        realm_id: id(20),
        profile_id: id(21),
        price_grid_id: id(22),
        price_measure_policy_id: price_policy().id().unwrap(),
        fee_policy_id: id(23),
        relation_policy_id: id(24),
        score_policy_id: id(25),
        candidate_lifecycle_policy_id: id(26),
        candidate_liveness_policy_id: id(27),
        retirement_policy_id: id(28),
        capability_profile_id: id(29),
        terminal_disposition_registry_value: BURN_REGISTRY_VALUE,
        native_bearer_lot: 1_000,
        coordinate_domain_min: 0,
        coordinate_domain_max: 400,
    }
}

fn quote() -> SeriesFundingQuoteV1 {
    let mut attempts = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
    attempts[0] = RecoveryAttemptFundingV1 {
        max_progress_units: 3,
        lamports_per_progress_unit: 5,
    };
    attempts[1] = RecoveryAttemptFundingV1 {
        max_progress_units: 2,
        lamports_per_progress_unit: 7,
    };
    SeriesFundingQuoteV1 {
        evidence_only_recovery_policy_id: recovery().id().unwrap(),
        market_core: ComponentDebitV1 {
            lamports: 10,
            collateral_atoms: 0,
        },
        failure_root_rent_principal_lamports: 3,
        failure_replay_tombstone_rent_principal_lamports: 2,
        recovery_reserve: ComponentDebitV1 {
            lamports: 40,
            collateral_atoms: 0,
        },
        source_work: ComponentDebitV1 {
            lamports: 30,
            collateral_atoms: 0,
        },
        liquidity_facility: ComponentDebitV1 {
            lamports: 40,
            collateral_atoms: 100,
        },
        wrapper_set: ComponentDebitV1 {
            lamports: 50,
            collateral_atoms: 10,
        },
        recovery_attempt_count: 2,
        recovery_attempt_funding: attempts,
        recovery_rent_principal_lamports: 11,
    }
}

fn attachment() -> SeriesAttachmentPlanV1 {
    SeriesAttachmentPlanV1 {
        funding_quote_id: quote().id().unwrap(),
        liquidity_facility_plan_id: id(41),
        wrapper_recipe_set_id: id(42),
    }
}

fn series_for(template: &ProductTemplateV4) -> SeriesPlanV5 {
    SeriesPlanV5 {
        product_template_id: template.id().unwrap(),
        market_genesis_profile_id: genesis().id().unwrap(),
        attachment_plan_id: attachment().id().unwrap(),
        first_start_bucket: 100,
        stride_buckets: 10,
        instance_count: 3,
        creation_lead_buckets: 5,
        market_collateral_cap: 1_000,
    }
}

fn series() -> SeriesPlanV5 {
    series_for(&template())
}

fn registry_for(
    template: &ProductTemplateV4,
    basis: &NativeClaimBasisV1,
) -> RegistryCapabilityProjectionV2 {
    let recovery = recovery();
    let genesis = genesis();
    RegistryCapabilityProjectionV2 {
        registry_release_id: id(70),
        capability_profile_id: genesis.capability_profile_id,
        statistic_registry_value: template.statistic_registry_value,
        coverage_policy_registry_value: template.coverage_policy_registry_value,
        ambiguity_policy_registry_value: basis.ambiguity_policy_registry_value,
        edge_policy_registry_value: basis.edge_policy_registry_value,
        burn_terminal_disposition_registry_value: BURN_REGISTRY_VALUE,
        resolved_edge_policy: QuantizedEdgePolicyV1::Clamp,
        supported_basis_degrees: [true, true, true, true],
        max_outcome_count: 16,
        max_degree_zero_payout_count: 16,
        max_recovery_attempt_count: 8,
        min_coverage_policy_parameter: 0,
        max_coverage_policy_parameter: u64::MAX,
        max_window_span_buckets: u64::MAX,
        max_series_instance_count: u32::MAX,
        maximum_interval_width: 1_000,
        maximum_coordinates_per_advance: 16,
        maximum_recovery_progress_units_per_call: 64,
        semantic_owners: CapabilitySemanticOwnersV2 {
            source_plane_contract_id: template.source_plane_contract_id,
            source_spec_id: template.source_spec_id,
            summary_program_id: template.summary_program_id,
            native_claim_basis_id: basis.id().unwrap(),
            evidence_only_recovery_policy_id: recovery.id().unwrap(),
            product_compiler_release_id: template.compiler_release_id,
            price_grid_id: genesis.price_grid_id,
            price_measure_policy_id: price_policy().id().unwrap(),
            fee_policy_id: genesis.fee_policy_id,
            relation_policy_id: genesis.relation_policy_id,
            score_policy_id: genesis.score_policy_id,
            candidate_lifecycle_policy_id: genesis.candidate_lifecycle_policy_id,
            candidate_liveness_policy_id: genesis.candidate_liveness_policy_id,
            retirement_policy_id: genesis.retirement_policy_id,
        },
        realm_collateral: RealmCollateralProjectionV1 {
            realm_id: genesis.realm_id,
            profile_id: genesis.profile_id,
            collateral_mint: id(53),
            token_program: id(54),
            neutral_incinerator: id(52),
            neutral_lamport_sink: id(55),
            market_collateral_cap_ceiling: 2_000,
        },
    }
}

fn registry() -> RegistryCapabilityProjectionV2 {
    registry_for(&template(), &basis())
}

fn legacy_genesis() -> MarketGenesisProfileV1 {
    MarketGenesisProfileV1 {
        realm_id: id(20),
        profile_id: id(21),
        price_grid_id: id(22),
        fee_policy_id: id(23),
        relation_policy_id: id(24),
        score_policy_id: id(25),
        candidate_lifecycle_policy_id: id(26),
        candidate_liveness_policy_id: id(27),
        retirement_policy_id: id(28),
        capability_profile_id: id(29),
        terminal_disposition_registry_value: BURN_REGISTRY_VALUE,
        native_bearer_lot: 1_000,
    }
}

fn funding_terms_v2() -> SeriesFundingTermsV2 {
    SeriesFundingTermsV2 {
        series_plan_id: series().id().unwrap(),
        lamport_principal_refund: id(50),
        collateral_principal_refund_token_account: id(51),
        neutral_collateral_disposition_token_account: id(52),
        neutral_lamport_sink: id(55),
        collateral_mint: id(53),
        token_program: id(54),
    }
}

#[test]
fn successor_funding_terms_separate_refunds_and_neutral_destinations() {
    let value = funding_terms_v2();
    value.validate_shape().unwrap();

    let mut aliases = value;
    aliases.neutral_collateral_disposition_token_account = aliases.lamport_principal_refund;
    assert_eq!(aliases.validate_shape(), Err(Error::InvalidParameter));

    aliases = value;
    aliases.neutral_collateral_disposition_token_account =
        aliases.collateral_principal_refund_token_account;
    assert_eq!(aliases.validate_shape(), Err(Error::InvalidParameter));

    aliases = value;
    aliases.neutral_lamport_sink = aliases.lamport_principal_refund;
    assert_eq!(aliases.validate_shape(), Err(Error::InvalidParameter));

    aliases = value;
    aliases.neutral_lamport_sink = aliases.collateral_principal_refund_token_account;
    assert_eq!(aliases.validate_shape(), Err(Error::InvalidParameter));

    aliases = value;
    aliases.neutral_lamport_sink = aliases.neutral_collateral_disposition_token_account;
    assert_eq!(aliases.validate_shape(), Err(Error::InvalidParameter));
}

fn legacy_funding_terms() -> SeriesFundingTermsV1 {
    SeriesFundingTermsV1 {
        series_plan_id: SeriesPlanId::from_bytes([49; 32]),
        lamport_principal_refund: id(50),
        collateral_principal_refund_token_account: id(51),
        neutral_sink: id(52),
        collateral_mint: id(53),
        token_program: id(54),
    }
}

#[test]
fn price_policy_freezes_only_quantized_production_semantics() {
    let policy = price_policy();
    policy.validate().unwrap();
    policy.validate_basis(&basis()).unwrap();
    let mut candidate = PriceVectorV3 {
        basis_degree: 2,
        native_outcome_count: 4,
        price_scale: 10_000,
        prices: [0; MAX_OUTCOMES],
    };
    candidate.prices[..4].copy_from_slice(&[2_500; 4]);
    policy
        .validate_candidate_price_contract(&basis(), &candidate, 10_000)
        .unwrap();
    let mut three_outcome_degree_two = basis();
    three_outcome_degree_two.outcome_count = 3;
    three_outcome_degree_two.knot_count = 2;
    three_outcome_degree_two.knots[2] = 0;
    let mut three_outcome_candidate = candidate;
    three_outcome_candidate.native_outcome_count = 3;
    three_outcome_candidate.prices[..4].copy_from_slice(&[2_500, 2_500, 5_000, 0]);
    policy
        .validate_candidate_price_contract(
            &three_outcome_degree_two,
            &three_outcome_candidate,
            10_000,
        )
        .unwrap();

    let mut old_checker = policy;
    old_checker.checker_version = 2;
    assert_eq!(old_checker.validate(), Err(Error::InvalidParameter));

    let mut alternate_semantics = policy;
    alternate_semantics.quantized_semantics_version = 2;
    assert_eq!(alternate_semantics.validate(), Err(Error::InvalidParameter));

    policy.validate_basis(&categorical_basis()).unwrap();
    policy.validate_basis(&finite_payout_basis()).unwrap();
    policy.validate_basis(&linear_basis()).unwrap();
    assert_eq!(
        policy.validate_candidate_price_contract(
            &basis(),
            &PriceVectorV3 {
                price_scale: policy.maximum_price_scale + 1,
                ..candidate
            },
            policy.maximum_price_scale + 1,
        ),
        Err(Error::UnsupportedCapability)
    );
    assert_eq!(
        policy.validate_candidate_price_contract(&basis(), &candidate, 9_999),
        Err(Error::UnsupportedCapability)
    );
    assert_eq!(
        policy.validate_candidate_price_contract(&three_outcome_degree_two, &candidate, 10_000,),
        Err(Error::UnsupportedCapability)
    );
    let mut bad_sum = candidate;
    bad_sum.prices[0] -= 1;
    assert_eq!(
        policy.validate_candidate_price_contract(&basis(), &bad_sum, 10_000),
        Err(Error::InvalidParameter)
    );
    let mut padded = candidate;
    padded.prices[4] = 1;
    assert_eq!(
        policy.validate_candidate_price_contract(&basis(), &padded, 10_000),
        Err(Error::NonCanonicalPadding)
    );
}

#[test]
fn every_degree_compiles_through_the_full_successor_join() {
    for native_basis in [
        categorical_basis(),
        finite_payout_basis(),
        linear_basis(),
        basis(),
        cubic_basis(),
    ] {
        let product_template = template_for(&native_basis);
        let series_plan = series_for(&product_template);
        let registry_projection = registry_for(&product_template, &native_basis);
        let compiled = compile_ordinal_v2(
            &series_plan,
            &product_template,
            &native_basis,
            &recovery(),
            &price_policy(),
            &genesis(),
            &attachment(),
            &registry_projection,
            0,
        )
        .unwrap();
        assert_eq!(compiled.ordinal, 0);
        assert_eq!(
            compiled.market.product_template_id,
            product_template.id().unwrap()
        );
        if native_basis == finite_payout_basis() {
            assert_ne!(native_basis.payout_count, native_basis.outcome_count);
            assert_eq!(native_basis.payout_map[..4], [0, 1, 1, 2]);
            assert_eq!(native_basis.payout_weights[1][1], 500);
            assert_eq!(native_basis.payout_weights[1][2], 500);
        }
    }
}

#[test]
fn every_smooth_degree_projects_exactly_and_checks_a_v3_measure() {
    for native_basis in [linear_basis(), basis(), cubic_basis()] {
        assert_eq!(
            price_policy().project_smooth_basis(
                &native_basis,
                &genesis(),
                QuantizedEdgePolicyV1::Refuse,
            ),
            Err(Error::UnsupportedCapability)
        );
        let exact_refusing_genesis = MarketGenesisProfileV2 {
            coordinate_domain_min: native_basis.knots[0],
            coordinate_domain_max: native_basis.knots[usize::from(native_basis.knot_count) - 1],
            ..genesis()
        };
        let projected = price_policy()
            .project_smooth_basis(
                &native_basis,
                &exact_refusing_genesis,
                QuantizedEdgePolicyV1::Refuse,
            )
            .unwrap();
        assert_eq!(projected.degree, native_basis.basis_degree);
        assert_eq!(projected.outcome_count, native_basis.outcome_count);
        assert_eq!(projected.knot_count, native_basis.knot_count);
        assert_eq!(projected.knots, native_basis.knots);
        assert_eq!(projected.denominator, native_basis.denominator);
        assert_eq!(
            projected.domain_max,
            exact_refusing_genesis.coordinate_domain_max
        );

        let coordinate = native_basis.knots[0];
        let evaluated = projected.evaluate(coordinate).unwrap();
        let price_vector = PriceVectorV3 {
            basis_degree: native_basis.basis_degree,
            native_outcome_count: native_basis.outcome_count,
            price_scale: evaluated.denominator,
            prices: evaluated.weights,
        };
        let bindings = AdapterBindingsV3 {
            candidate_feed: [1; 32],
            relation_domain_digest: [2; 32],
            basis_digest: native_basis.id().unwrap().bytes(),
            candidate_price_digest: [4; 32],
            observed_body_digest: [5; 32],
        };
        let mut atom_coordinates = [0; MAX_OUTCOMES];
        atom_coordinates[0] = coordinate;
        let mut atom_masses = [0; MAX_OUTCOMES];
        atom_masses[0] = 1;
        let witness = QuantizedAtomWitnessV3 {
            schema_version: 3,
            quantized_semantics_version: 1,
            candidate_feed: bindings.candidate_feed,
            relation_domain_digest: bindings.relation_domain_digest,
            basis_digest: bindings.basis_digest,
            candidate_price_digest: bindings.candidate_price_digest,
            body_digest: bindings.observed_body_digest,
            basis_degree: native_basis.basis_degree,
            native_outcome_count: native_basis.outcome_count,
            atom_count: 1,
            common_denominator: 1,
            atom_coordinates,
            atom_masses,
        };
        price_policy()
            .validate_witness_contract(
                &native_basis,
                &price_vector,
                &witness,
                evaluated.denominator,
            )
            .unwrap();
        let verified = verify_quantized_price_measure_v3_smooth(
            &bindings,
            &projected,
            &price_vector,
            &witness,
        )
        .unwrap();
        assert_eq!(verified.basis_degree(), native_basis.basis_degree);
        assert_eq!(verified.native_outcome_count(), native_basis.outcome_count);
    }
}

#[test]
fn refusing_smooth_domain_is_total_only_on_the_exact_knot_span() {
    let native_basis = linear_basis();
    let product_template = template_for(&native_basis);
    let broad_genesis = genesis();
    let broad_series = series_for(&product_template);
    let mut refusing_registry = registry_for(&product_template, &native_basis);
    refusing_registry.resolved_edge_policy = QuantizedEdgePolicyV1::Refuse;
    assert_eq!(
        refusing_registry.validate_complete_join(
            &broad_series,
            &product_template,
            &native_basis,
            &recovery(),
            &price_policy(),
            &broad_genesis,
        ),
        Err(Error::UnsupportedCapability)
    );

    let exact_genesis = MarketGenesisProfileV2 {
        coordinate_domain_min: native_basis.knots[0],
        coordinate_domain_max: native_basis.knots[usize::from(native_basis.knot_count) - 1],
        ..broad_genesis
    };
    let exact_series = SeriesPlanV5 {
        market_genesis_profile_id: exact_genesis.id().unwrap(),
        ..broad_series
    };
    refusing_registry
        .validate_complete_join(
            &exact_series,
            &product_template,
            &native_basis,
            &recovery(),
            &price_policy(),
            &exact_genesis,
        )
        .unwrap();
    compile_ordinal_v2(
        &exact_series,
        &product_template,
        &native_basis,
        &recovery(),
        &price_policy(),
        &exact_genesis,
        &attachment(),
        &refusing_registry,
        0,
    )
    .unwrap();

    let clamping_projection = price_policy()
        .project_smooth_basis(&native_basis, &broad_genesis, QuantizedEdgePolicyV1::Clamp)
        .unwrap();
    assert_eq!(
        clamping_projection.evaluate(broad_genesis.coordinate_domain_min),
        clamping_projection.evaluate(native_basis.knots[0])
    );
    assert_eq!(
        clamping_projection.evaluate(broad_genesis.coordinate_domain_max),
        clamping_projection.evaluate(native_basis.knots[usize::from(native_basis.knot_count) - 1])
    );
}

#[test]
fn finite_product_table_projects_losslessly_and_checks_a_v3_measure() {
    let native_basis = finite_payout_basis();
    let table = price_policy()
        .project_degree_zero_table(&native_basis, &genesis())
        .unwrap();
    assert_eq!(table.native_outcome_count, native_basis.outcome_count);
    assert_eq!(table.payout_count, native_basis.payout_count);
    assert_eq!(table.knot_count, native_basis.knot_count);
    assert_eq!(table.payout_denominator, native_basis.denominator);
    assert_eq!(table.payout_weights, native_basis.payout_weights);
    assert_eq!(table.payout_map, native_basis.payout_map);
    assert_eq!(table.knots, native_basis.knots);
    assert_eq!(
        table.evaluate(100).unwrap().weights,
        native_basis.payout_weights[1]
    );
    assert_eq!(
        table.evaluate(200).unwrap().weights,
        native_basis.payout_weights[1]
    );

    let bindings = AdapterBindingsV3 {
        candidate_feed: [1; 32],
        relation_domain_digest: [2; 32],
        basis_digest: native_basis.id().unwrap().bytes(),
        candidate_price_digest: [4; 32],
        observed_body_digest: [5; 32],
    };
    let mut prices = [0; MAX_OUTCOMES];
    prices[1] = 500;
    prices[2] = 500;
    let price_vector = PriceVectorV3 {
        basis_degree: 0,
        native_outcome_count: 4,
        price_scale: 1_000,
        prices,
    };
    let mut atom_coordinates = [0; MAX_OUTCOMES];
    atom_coordinates[0] = 100;
    let mut atom_masses = [0; MAX_OUTCOMES];
    atom_masses[0] = 1;
    let witness = QuantizedAtomWitnessV3 {
        schema_version: 3,
        quantized_semantics_version: 1,
        candidate_feed: bindings.candidate_feed,
        relation_domain_digest: bindings.relation_domain_digest,
        basis_digest: bindings.basis_digest,
        candidate_price_digest: bindings.candidate_price_digest,
        body_digest: bindings.observed_body_digest,
        basis_degree: 0,
        native_outcome_count: 4,
        atom_count: 1,
        common_denominator: 1,
        atom_coordinates,
        atom_masses,
    };
    price_policy()
        .validate_witness_contract(&native_basis, &price_vector, &witness, 1_000)
        .unwrap();
    let mut bounded_policy = price_policy();
    bounded_policy.maximum_witness_denominator = 1;
    let mut oversized_witness = witness;
    oversized_witness.common_denominator = 2;
    oversized_witness.atom_masses[0] = 2;
    assert_eq!(
        bounded_policy.validate_witness_contract(
            &native_basis,
            &price_vector,
            &oversized_witness,
            1_000,
        ),
        Err(Error::UnsupportedCapability)
    );
    let mut too_many_atoms = witness;
    too_many_atoms.atom_count = 5;
    assert_eq!(
        price_policy().validate_witness_contract(
            &native_basis,
            &price_vector,
            &too_many_atoms,
            1_000,
        ),
        Err(Error::UnsupportedCapability)
    );
    let verified =
        verify_quantized_price_measure_v3_degree_zero(&bindings, &table, &price_vector, &witness)
            .unwrap();
    assert_eq!(verified.native_outcome_count(), 4);
    assert_eq!(verified.basis_region_count(), 4);

    let mut substituted_basis = native_basis;
    substituted_basis.payout_weights[1][1] = 250;
    substituted_basis.payout_weights[1][2] = 750;
    substituted_basis.validate().unwrap();
    let substituted_basis_table = price_policy()
        .project_degree_zero_table(&substituted_basis, &genesis())
        .unwrap();
    assert_ne!(substituted_basis.id().unwrap(), native_basis.id().unwrap());
    assert_ne!(substituted_basis_table.payout_weights, table.payout_weights);
    assert_eq!(substituted_basis_table.domain_min, table.domain_min);
    assert_eq!(substituted_basis_table.domain_max, table.domain_max);
    let mut substituted_basis_witness = witness;
    substituted_basis_witness.basis_digest = substituted_basis.id().unwrap().bytes();
    assert_eq!(
        verify_quantized_price_measure_v3_degree_zero(
            &bindings,
            &substituted_basis_table,
            &price_vector,
            &substituted_basis_witness,
        ),
        Err(ErrorV3::BindingMismatch {
            field: BindingFieldV3::BasisDigest,
        })
    );

    let mut substituted_domain = genesis();
    substituted_domain.coordinate_domain_max = 500;
    let substituted_domain_table = price_policy()
        .project_degree_zero_table(&native_basis, &substituted_domain)
        .unwrap();
    assert_eq!(
        substituted_domain_table.payout_weights,
        table.payout_weights
    );
    assert_eq!(substituted_domain_table.payout_map, table.payout_map);
    assert_ne!(substituted_domain_table.domain_max, table.domain_max);
    let mut substituted_domain_witness = witness;
    substituted_domain_witness.relation_domain_digest = [9; 32];
    assert_eq!(
        verify_quantized_price_measure_v3_degree_zero(
            &bindings,
            &substituted_domain_table,
            &price_vector,
            &substituted_domain_witness,
        ),
        Err(ErrorV3::BindingMismatch {
            field: BindingFieldV3::RelationDomainDigest,
        })
    );

    assert_eq!(
        price_policy().project_degree_zero_table(
            &native_basis,
            &MarketGenesisProfileV2 {
                coordinate_domain_min: 100,
                ..genesis()
            },
        ),
        Err(Error::MismatchedArtifact)
    );
    assert_eq!(
        price_policy().project_degree_zero_table(
            &native_basis,
            &MarketGenesisProfileV2 {
                coordinate_domain_max: 299,
                ..genesis()
            },
        ),
        Err(Error::MismatchedArtifact)
    );
    assert_eq!(
        price_policy().project_degree_zero_table(&linear_basis(), &genesis()),
        Err(Error::UnsupportedCapability)
    );
}

#[test]
fn successor_codecs_round_trip_and_cross_version_bytes_refuse() {
    let policy = price_policy();
    let mut policy_bytes = [0; PRICE_MEASURE_POLICY_BYTES];
    policy.encode_into(&mut policy_bytes).unwrap();
    assert_eq!(PriceMeasurePolicyV1::decode(&policy_bytes).unwrap(), policy);
    assert_eq!(&policy_bytes[..8], b"DCPMPV1\0");
    assert_eq!(&policy_bytes[8..10], &1_u16.to_le_bytes());
    assert_eq!(
        policy.id().unwrap().bytes(),
        independent_digest(PRICE_MEASURE_POLICY_DOMAIN, &policy_bytes)
    );
    assert_eq!(
        policy.id().unwrap().bytes(),
        [
            0x40, 0x8a, 0xa9, 0x8e, 0xa8, 0xf8, 0x90, 0xcf, 0xf8, 0x0c, 0x84, 0xe2, 0xdd, 0x29,
            0x79, 0x21, 0xa9, 0xa5, 0x56, 0x0f, 0x01, 0xd1, 0xcc, 0x2f, 0x84, 0xcb, 0x72, 0x70,
            0x3b, 0xf1, 0x48, 0x96,
        ]
    );

    let genesis_v2 = genesis();
    let mut genesis_v2_bytes = [0; MARKET_GENESIS_PROFILE_V2_BYTES];
    genesis_v2.encode_into(&mut genesis_v2_bytes).unwrap();
    assert_eq!(
        &genesis_v2_bytes[376..392],
        &genesis_v2.coordinate_domain_min.to_le_bytes()
    );
    assert_eq!(
        &genesis_v2_bytes[392..408],
        &genesis_v2.coordinate_domain_max.to_le_bytes()
    );
    assert_eq!(&genesis_v2_bytes[408..416], &[0; 8]);
    assert_eq!(
        genesis_v2.id().unwrap().bytes(),
        [
            0x6a, 0x05, 0x3d, 0x18, 0x6f, 0x63, 0x76, 0x8c, 0x74, 0x57, 0xb1, 0xec, 0x2d, 0x55,
            0x4a, 0x13, 0x79, 0xb0, 0xba, 0x59, 0x53, 0xc0, 0xa8, 0x38, 0x23, 0x00, 0xc3, 0x9a,
            0xf5, 0xab, 0xad, 0x6f,
        ]
    );
    assert_eq!(
        genesis_v2.id().unwrap().bytes(),
        independent_digest(MARKET_GENESIS_PROFILE_V2_DOMAIN, &genesis_v2_bytes)
    );
    assert_eq!(
        MarketGenesisProfileV2::decode(&genesis_v2_bytes).unwrap(),
        genesis_v2
    );
    let genesis_v1 = legacy_genesis();
    let mut genesis_v1_bytes = [0; MARKET_GENESIS_PROFILE_BYTES];
    genesis_v1.encode_into(&mut genesis_v1_bytes).unwrap();
    assert_eq!(
        MarketGenesisProfileV1::decode(&genesis_v2_bytes),
        Err(Error::TrailingBytes)
    );
    assert_eq!(
        MarketGenesisProfileV2::decode(&genesis_v1_bytes),
        Err(Error::Truncated)
    );

    let market_v2 = MarketInstancePreimageV2 {
        product_template_id: template().id().unwrap(),
        market_genesis_profile_id: genesis_v2.id().unwrap(),
        start_bucket: 100,
        collateral_cap: 1_000,
    };
    let mut market_v2_bytes = [0; MARKET_INSTANCE_PREIMAGE_V2_BYTES];
    market_v2.encode_into(&mut market_v2_bytes).unwrap();
    assert_eq!(
        market_v2.id().unwrap().bytes(),
        [
            0xc8, 0x3d, 0x24, 0x1e, 0x59, 0x6b, 0xd9, 0xb7, 0x3c, 0xc3, 0xa1, 0x48, 0x7f, 0x8c,
            0x67, 0x6c, 0x1d, 0x35, 0x32, 0xa9, 0xc4, 0xed, 0xc2, 0x92, 0x8a, 0x4d, 0x0c, 0x10,
            0xc6, 0xef, 0x11, 0xa3,
        ]
    );
    assert_eq!(
        market_v2.id().unwrap().bytes(),
        independent_digest(MARKET_INSTANCE_V2_DOMAIN, &market_v2_bytes)
    );
    assert_eq!(&market_v2_bytes[..8], b"DCMKTIN2");
    assert_eq!(
        &market_v2_bytes[8..40],
        &market_v2.product_template_id.bytes()
    );
    assert_eq!(
        &market_v2_bytes[40..72],
        &market_v2.market_genesis_profile_id.bytes()
    );
    assert_eq!(
        &market_v2_bytes[72..80],
        &market_v2.start_bucket.to_le_bytes()
    );
    assert_eq!(
        &market_v2_bytes[80..88],
        &market_v2.collateral_cap.to_le_bytes()
    );
    assert_eq!(
        MarketInstancePreimageV2::decode(&market_v2_bytes).unwrap(),
        market_v2
    );
    let market_v1 = MarketInstancePreimageV1 {
        product_template_id: template().id().unwrap(),
        market_genesis_profile_id: genesis_v1.id().unwrap(),
        start_bucket: 100,
        collateral_cap: 1_000,
    };
    let mut market_v1_bytes = [0; MARKET_INSTANCE_PREIMAGE_BYTES];
    market_v1.encode_into(&mut market_v1_bytes).unwrap();
    assert_eq!(
        MarketInstancePreimageV1::decode(&market_v2_bytes),
        Err(Error::BadMagic)
    );
    assert_eq!(
        MarketInstancePreimageV2::decode(&market_v1_bytes),
        Err(Error::BadMagic)
    );

    let series_v2 = series();
    let mut series_v2_bytes = [0; SERIES_PLAN_V5_BYTES];
    series_v2.encode_into(&mut series_v2_bytes).unwrap();
    assert_eq!(
        series_v2.id().unwrap().bytes(),
        [
            0x99, 0x23, 0x9f, 0x5a, 0x9b, 0x7c, 0x19, 0xd2, 0x1b, 0x95, 0xe9, 0xa1, 0xab, 0x40,
            0x21, 0x97, 0x5b, 0x71, 0xdc, 0xd4, 0x07, 0x86, 0xe0, 0x42, 0x73, 0x65, 0x5d, 0xff,
            0xa2, 0xad, 0xdf, 0xf5,
        ]
    );
    assert_eq!(
        series_v2.id().unwrap().bytes(),
        independent_digest(SERIES_PLAN_V5_DOMAIN, &series_v2_bytes)
    );
    assert_eq!(SeriesPlanV5::decode(&series_v2_bytes).unwrap(), series_v2);
    let legacy_series = SeriesPlanV4 {
        product_template_id: template().id().unwrap(),
        market_genesis_profile_id: genesis_v1.id().unwrap(),
        attachment_plan_id: attachment().id().unwrap(),
        first_start_bucket: 100,
        stride_buckets: 10,
        instance_count: 3,
        creation_lead_buckets: 5,
        market_collateral_cap: 1_000,
    };
    let mut legacy_series_bytes = [0; SERIES_PLAN_BYTES];
    legacy_series.encode_into(&mut legacy_series_bytes).unwrap();
    assert_eq!(SeriesPlanV4::decode(&series_v2_bytes), Err(Error::BadMagic));
    assert_eq!(
        SeriesPlanV5::decode(&legacy_series_bytes),
        Err(Error::BadMagic)
    );

    let terms_v2 = funding_terms_v2();
    let mut terms_v2_bytes = [0; SERIES_FUNDING_TERMS_V2_BYTES];
    terms_v2.encode_into(&mut terms_v2_bytes).unwrap();
    assert_eq!(&terms_v2_bytes[112..144], &[52; 32]);
    assert_eq!(&terms_v2_bytes[144..176], &[55; 32]);
    assert_eq!(&terms_v2_bytes[176..208], &[53; 32]);
    assert_eq!(&terms_v2_bytes[208..240], &[54; 32]);
    assert_eq!(
        terms_v2.id().unwrap().bytes(),
        independent_digest(SERIES_FUNDING_TERMS_V2_DOMAIN, &terms_v2_bytes)
    );
    let legacy_terms = legacy_funding_terms();
    let mut legacy_terms_bytes = [0; SERIES_FUNDING_TERMS_BYTES];
    legacy_terms.encode_into(&mut legacy_terms_bytes).unwrap();
    assert_eq!(
        SeriesFundingTermsV1::decode(&terms_v2_bytes),
        Err(Error::BadMagic)
    );
    assert_eq!(
        SeriesFundingTermsV2::decode(&legacy_terms_bytes),
        Err(Error::BadMagic)
    );

    assert_wrong_lengths!(PriceMeasurePolicyV1, policy_bytes);
    assert_wrong_lengths!(MarketGenesisProfileV2, genesis_v2_bytes);
    assert_wrong_lengths!(MarketInstancePreimageV2, market_v2_bytes);
    assert_wrong_lengths!(SeriesPlanV5, series_v2_bytes);
    assert_wrong_lengths!(SeriesFundingTermsV2, terms_v2_bytes);
}

#[test]
fn price_policy_mutation_changes_every_transitive_economic_identity() {
    let first_policy = price_policy();
    let mut second_policy = first_policy;
    second_policy.maximum_price_scale -= 1;
    assert_ne!(first_policy.id().unwrap(), second_policy.id().unwrap());

    let first_genesis = genesis();
    let mut second_genesis = first_genesis;
    second_genesis.price_measure_policy_id = second_policy.id().unwrap();
    assert_ne!(first_genesis.id().unwrap(), second_genesis.id().unwrap());

    let first_market = MarketInstancePreimageV2 {
        product_template_id: template().id().unwrap(),
        market_genesis_profile_id: first_genesis.id().unwrap(),
        start_bucket: 100,
        collateral_cap: 1_000,
    };
    let mut second_market = first_market;
    second_market.market_genesis_profile_id = second_genesis.id().unwrap();
    assert_ne!(first_market.id().unwrap(), second_market.id().unwrap());

    let first_series = series();
    let mut second_series = first_series;
    second_series.market_genesis_profile_id = second_genesis.id().unwrap();
    assert_ne!(first_series.id().unwrap(), second_series.id().unwrap());

    let first_terms = funding_terms_v2();
    let mut second_terms = first_terms;
    second_terms.series_plan_id = second_series.id().unwrap();
    assert_ne!(first_terms.id().unwrap(), second_terms.id().unwrap());

    let mut wider_domain_genesis = first_genesis;
    wider_domain_genesis.coordinate_domain_max += 1;
    assert_ne!(
        first_genesis.id().unwrap(),
        wider_domain_genesis.id().unwrap()
    );
    let mut wider_domain_market = first_market;
    wider_domain_market.market_genesis_profile_id = wider_domain_genesis.id().unwrap();
    assert_ne!(
        first_market.id().unwrap(),
        wider_domain_market.id().unwrap()
    );

    for market in [
        MarketInstancePreimageV2 {
            start_bucket: first_market.start_bucket + 1,
            ..first_market
        },
        MarketInstancePreimageV2 {
            collateral_cap: first_market.collateral_cap + genesis().native_bearer_lot,
            ..first_market
        },
        MarketInstancePreimageV2 {
            product_template_id: ProductTemplateId::from_bytes([98; 32]),
            ..first_market
        },
    ] {
        assert_ne!(first_market.id().unwrap(), market.id().unwrap());
    }
}

#[test]
fn registry_join_refuses_any_price_policy_substitution() {
    let series = series();
    let template = template();
    let basis = basis();
    let recovery = recovery();
    let policy = price_policy();
    let genesis = genesis();
    let registry = registry();
    registry
        .validate_complete_join(&series, &template, &basis, &recovery, &policy, &genesis)
        .unwrap();

    let mut wrong_owner = registry;
    wrong_owner.semantic_owners.price_measure_policy_id =
        PriceMeasurePolicyV1Id::from_bytes([99; 32]);
    assert_eq!(
        wrong_owner
            .validate_complete_join(&series, &template, &basis, &recovery, &policy, &genesis),
        Err(Error::MismatchedArtifact)
    );

    let mut alternate = policy;
    alternate.maximum_witness_denominator -= 1;
    assert_eq!(
        registry
            .validate_complete_join(&series, &template, &basis, &recovery, &alternate, &genesis),
        Err(Error::MismatchedArtifact)
    );

    let mut alternate_domain = genesis;
    alternate_domain.coordinate_domain_max += 1;
    assert_eq!(
        registry.validate_complete_join(
            &series,
            &template,
            &basis,
            &recovery,
            &policy,
            &alternate_domain,
        ),
        Err(Error::MismatchedArtifact)
    );
}

#[test]
fn successor_compilation_funding_terms_and_debits_use_fresh_typed_ids() {
    let compiled = compile_ordinal_v2(
        &series(),
        &template(),
        &basis(),
        &recovery(),
        &price_policy(),
        &genesis(),
        &attachment(),
        &registry(),
        1,
    )
    .unwrap();
    assert_eq!(compiled.ordinal, 1);
    assert_eq!(compiled.market.start_bucket, 110);
    assert_eq!(compiled.schedule.end_bucket_exclusive, 114);
    assert_eq!(compiled.market_instance_id, compiled.market.id().unwrap());

    let terms = funding_terms_v2();
    terms
        .validate_bindings(
            &series(),
            &template(),
            &basis(),
            &recovery(),
            &price_policy(),
            &genesis(),
            &registry(),
        )
        .unwrap();
    let mut terms_bytes = [0; SERIES_FUNDING_TERMS_V2_BYTES];
    terms.encode_into(&mut terms_bytes).unwrap();
    assert_eq!(SeriesFundingTermsV2::decode(&terms_bytes).unwrap(), terms);
    assert_eq!(&terms_bytes[..8], b"DCFTERM2");

    let status = AdapterFulfillmentProjectionV2 {
        market_instance_id: compiled.market_instance_id,
        attachment_plan_id: attachment().id().unwrap(),
        funding_quote_id: quote().id().unwrap(),
        market_core: ProjectedComponentPresenceV2::Absent,
        recovery_reserve: ProjectedComponentPresenceV2::Absent,
        source_work: ProjectedComponentPresenceV2::Absent,
        liquidity_facility: ProjectedComponentPresenceV2::Absent,
        wrapper_set: ProjectedComponentPresenceV2::Absent,
    };
    let projection = project_component_debits_v2(
        compiled.market_instance_id,
        &recovery(),
        &attachment(),
        &quote(),
        status,
        FundingBalancesV1 {
            lamports: 200,
            collateral_atoms: 200,
        },
    )
    .unwrap();
    assert_eq!(projection.total.lamports, 170);
    assert_eq!(projection.total.collateral_atoms, 110);
    assert_eq!(projection.remaining.lamports, 30);
    assert_eq!(projection.remaining.collateral_atoms, 90);
}

#[test]
fn public_v2_fulfillment_projection_is_explicitly_untrusted_model_input() {
    let compiled = compile_ordinal_v2(
        &series(),
        &template(),
        &basis(),
        &recovery(),
        &price_policy(),
        &genesis(),
        &attachment(),
        &registry(),
        0,
    )
    .unwrap();
    let forgeable_projection = AdapterFulfillmentProjectionV2 {
        market_instance_id: compiled.market_instance_id,
        attachment_plan_id: attachment().id().unwrap(),
        funding_quote_id: quote().id().unwrap(),
        market_core: ProjectedComponentPresenceV2::ClaimedPresentExactAndCapitalized,
        recovery_reserve: ProjectedComponentPresenceV2::ClaimedPresentExactAndCapitalized,
        source_work: ProjectedComponentPresenceV2::ClaimedPresentExactAndCapitalized,
        liquidity_facility: ProjectedComponentPresenceV2::ClaimedPresentExactAndCapitalized,
        wrapper_set: ProjectedComponentPresenceV2::ClaimedPresentExactAndCapitalized,
    };
    let modeled = project_component_debits_v2(
        compiled.market_instance_id,
        &recovery(),
        &attachment(),
        &quote(),
        forgeable_projection,
        FundingBalancesV1 {
            lamports: 0,
            collateral_atoms: 0,
        },
    )
    .unwrap();
    assert_eq!(modeled.total, ComponentDebitV1::ZERO);

    let mut wrong_market = forgeable_projection;
    wrong_market.market_instance_id = MarketInstanceV2Id::from_bytes([99; 32]);
    assert_eq!(
        project_component_debits_v2(
            compiled.market_instance_id,
            &recovery(),
            &attachment(),
            &quote(),
            wrong_market,
            FundingBalancesV1 {
                lamports: 0,
                collateral_atoms: 0,
            },
        ),
        Err(Error::MismatchedArtifact)
    );
}

#[test]
fn hostile_successor_reserved_and_semantic_bytes_refuse() {
    let mut policy_bytes = [0; PRICE_MEASURE_POLICY_BYTES];
    price_policy().encode_into(&mut policy_bytes).unwrap();
    policy_bytes[49] = 2;
    assert_eq!(
        PriceMeasurePolicyV1::decode(&policy_bytes),
        Err(Error::InvalidParameter)
    );

    price_policy().encode_into(&mut policy_bytes).unwrap();
    policy_bytes[95] = 1;
    assert_eq!(
        PriceMeasurePolicyV1::decode(&policy_bytes),
        Err(Error::NonCanonicalReserved)
    );
    price_policy().encode_into(&mut policy_bytes).unwrap();
    policy_bytes[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        PriceMeasurePolicyV1::decode(&policy_bytes),
        Err(Error::BadVersion)
    );

    let mut genesis_bytes = [0; MARKET_GENESIS_PROFILE_V2_BYTES];
    genesis().encode_into(&mut genesis_bytes).unwrap();
    genesis_bytes[12] = 1;
    assert_eq!(
        MarketGenesisProfileV2::decode(&genesis_bytes),
        Err(Error::NonCanonicalReserved)
    );
    genesis().encode_into(&mut genesis_bytes).unwrap();
    genesis_bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        MarketGenesisProfileV2::decode(&genesis_bytes),
        Err(Error::BadVersion)
    );
    let mut bad_bounds = genesis();
    bad_bounds.coordinate_domain_min = bad_bounds.coordinate_domain_max;
    assert_eq!(bad_bounds.validate_shape(), Err(Error::InvalidParameter));

    let mut market_bytes = [0; MARKET_INSTANCE_PREIMAGE_V2_BYTES];
    MarketInstancePreimageV2 {
        product_template_id: template().id().unwrap(),
        market_genesis_profile_id: genesis().id().unwrap(),
        start_bucket: 100,
        collateral_cap: 1_000,
    }
    .encode_into(&mut market_bytes)
    .unwrap();
    market_bytes[8..40].fill(0);
    assert_eq!(
        MarketInstancePreimageV2::decode(&market_bytes),
        Err(Error::ZeroIdentity)
    );

    let mut series_bytes = [0; SERIES_PLAN_V5_BYTES];
    series().encode_into(&mut series_bytes).unwrap();
    series_bytes[10] = 1;
    assert_eq!(
        SeriesPlanV5::decode(&series_bytes),
        Err(Error::NonCanonicalReserved)
    );
    let mut overflowing_series = series();
    overflowing_series.first_start_bucket = u64::MAX;
    overflowing_series.stride_buckets = 1;
    assert_eq!(
        overflowing_series.validate_shape(),
        Err(Error::ArithmeticOverflow)
    );

    let mut terms_bytes = [0; SERIES_FUNDING_TERMS_V2_BYTES];
    funding_terms_v2().encode_into(&mut terms_bytes).unwrap();
    terms_bytes[10] = 1;
    assert_eq!(
        SeriesFundingTermsV2::decode(&terms_bytes),
        Err(Error::NonCanonicalReserved)
    );
    funding_terms_v2().encode_into(&mut terms_bytes).unwrap();
    terms_bytes[16..48].fill(0);
    assert_eq!(
        SeriesFundingTermsV2::decode(&terms_bytes),
        Err(Error::ZeroIdentity)
    );
}
