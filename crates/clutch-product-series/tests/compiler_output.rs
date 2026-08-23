use clutch_product_series::*;

const BURN: u16 = 7;

fn id(byte: u8) -> ContentId {
    ContentId::from_bytes([byte; 32])
}

fn basis() -> NativeClaimBasisV1 {
    let mut knots = [0_u128; MAX_OUTCOMES];
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

fn template() -> ProductTemplateV4 {
    ProductTemplateV4 {
        source_plane_contract_id: id(1),
        source_spec_id: id(2),
        summary_program_id: id(3),
        native_claim_basis_id: basis().id().unwrap(),
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
        terminal_disposition_registry_value: BURN,
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

fn series() -> SeriesPlanV5 {
    SeriesPlanV5 {
        product_template_id: template().id().unwrap(),
        market_genesis_profile_id: genesis().id().unwrap(),
        attachment_plan_id: attachment().id().unwrap(),
        first_start_bucket: 100,
        stride_buckets: 10,
        instance_count: 3,
        creation_lead_buckets: 5,
        market_collateral_cap: 1_000,
    }
}

fn funding_terms() -> SeriesFundingTermsV2 {
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

fn registry() -> RegistryCapabilityProjectionV2 {
    let template = template();
    let basis = basis();
    let genesis = genesis();
    RegistryCapabilityProjectionV2 {
        registry_release_id: id(70),
        capability_profile_id: genesis.capability_profile_id,
        statistic_registry_value: template.statistic_registry_value,
        coverage_policy_registry_value: template.coverage_policy_registry_value,
        ambiguity_policy_registry_value: basis.ambiguity_policy_registry_value,
        edge_policy_registry_value: basis.edge_policy_registry_value,
        burn_terminal_disposition_registry_value: BURN,
        resolved_edge_policy: QuantizedEdgePolicyV1::Clamp,
        supported_basis_degrees: [true; 4],
        max_outcome_count: 16,
        max_degree_zero_payout_count: 16,
        max_recovery_attempt_count: 8,
        min_coverage_policy_parameter: 0,
        max_coverage_policy_parameter: u64::MAX,
        max_window_span_buckets: u64::MAX,
        max_series_instance_count: u32::MAX,
        semantic_owners: CapabilitySemanticOwnersV2 {
            source_plane_contract_id: template.source_plane_contract_id,
            source_spec_id: template.source_spec_id,
            summary_program_id: template.summary_program_id,
            native_claim_basis_id: basis.id().unwrap(),
            evidence_only_recovery_policy_id: recovery().id().unwrap(),
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

fn assemble_with<'a>(
    source_release_manifest_id: ContentId,
    registry: &'a RegistryCapabilityProjectionV2,
    basis: &'a NativeClaimBasisV1,
    recovery: &'a EvidenceOnlyRecoveryPolicyV1,
    template: &'a ProductTemplateV4,
    price_policy: &'a PriceMeasurePolicyV1,
    genesis: &'a MarketGenesisProfileV2,
    funding_quote: &'a SeriesFundingQuoteV1,
    attachment: &'a SeriesAttachmentPlanV1,
    series: &'a SeriesPlanV5,
    funding_terms: &'a SeriesFundingTermsV2,
) -> Result<CompiledProductSeriesBundleV1> {
    assemble_compiled_product_series_bundle_v1(ProductSeriesBundleInputsV1 {
        registry,
        source_release_manifest_id,
        basis,
        recovery,
        template,
        price_policy,
        genesis,
        funding_quote,
        attachment,
        series,
        funding_terms,
    })
}

#[test]
fn assembler_recomputes_every_bundle_identity_from_canonical_bodies() {
    let registry = registry();
    let basis = basis();
    let recovery = recovery();
    let template = template();
    let price_policy = price_policy();
    let genesis = genesis();
    let quote = quote();
    let attachment = attachment();
    let series = series();
    let funding_terms = funding_terms();
    let bundle = assemble_with(
        id(80),
        &registry,
        &basis,
        &recovery,
        &template,
        &price_policy,
        &genesis,
        &quote,
        &attachment,
        &series,
        &funding_terms,
    )
    .unwrap();
    assert_eq!(bundle.registry_release_id, registry.registry_release_id);
    assert_eq!(bundle.native_claim_basis_id, basis.id().unwrap());
    assert_eq!(bundle.product_template_id, template.id().unwrap());
    assert_eq!(bundle.price_measure_policy_id, price_policy.id().unwrap());
    assert_eq!(bundle.market_genesis_profile_id, genesis.id().unwrap());
    assert_eq!(bundle.series_plan_id, series.id().unwrap());
    assert_eq!(bundle.funding_terms_id, funding_terms.id().unwrap());
    let mut bytes = [0_u8; COMPILED_PRODUCT_SERIES_BUNDLE_V1_BYTES];
    bundle.encode_into(&mut bytes).unwrap();
    assert_eq!(
        CompiledProductSeriesBundleV1::decode(&bytes).unwrap(),
        bundle
    );
    assert!(!bundle.id().unwrap().bytes().iter().all(|byte| *byte == 0));
}

#[test]
fn assembler_refuses_foreign_bodies_and_absent_source_release() {
    let registry = registry();
    let basis = basis();
    let recovery = recovery();
    let template = template();
    let price_policy = price_policy();
    let genesis = genesis();
    let quote = quote();
    let attachment = attachment();
    let series = series();
    let funding_terms = funding_terms();
    assert_eq!(
        assemble_with(
            ContentId::ZERO,
            &registry,
            &basis,
            &recovery,
            &template,
            &price_policy,
            &genesis,
            &quote,
            &attachment,
            &series,
            &funding_terms,
        ),
        Err(Error::ZeroIdentity)
    );

    let mut foreign_basis = basis;
    foreign_basis.denominator += 1;
    assert_eq!(
        assemble_with(
            id(80),
            &registry,
            &foreign_basis,
            &recovery,
            &template,
            &price_policy,
            &genesis,
            &quote,
            &attachment,
            &series,
            &funding_terms,
        ),
        Err(Error::MismatchedArtifact)
    );

    let mut wrong_terms = funding_terms;
    wrong_terms.series_plan_id = SeriesPlanV5Id::from_bytes([90; 32]);
    assert_eq!(
        assemble_with(
            id(80),
            &registry,
            &basis,
            &recovery,
            &template,
            &price_policy,
            &genesis,
            &quote,
            &attachment,
            &series,
            &wrong_terms,
        ),
        Err(Error::MismatchedArtifact)
    );
}

#[test]
fn bundle_codec_refuses_identity_substitution_and_noncanonical_bytes() {
    let registry = registry();
    let basis = basis();
    let recovery = recovery();
    let template = template();
    let price_policy = price_policy();
    let genesis = genesis();
    let quote = quote();
    let attachment = attachment();
    let series = series();
    let funding_terms = funding_terms();
    let bundle = assemble_with(
        id(80),
        &registry,
        &basis,
        &recovery,
        &template,
        &price_policy,
        &genesis,
        &quote,
        &attachment,
        &series,
        &funding_terms,
    )
    .unwrap();
    let mut bytes = [0_u8; COMPILED_PRODUCT_SERIES_BUNDLE_V1_BYTES];
    bundle.encode_into(&mut bytes).unwrap();
    bytes[10] = 1;
    assert_eq!(
        CompiledProductSeriesBundleV1::decode(&bytes),
        Err(Error::NonCanonicalReserved)
    );

    let mut changed = bundle;
    changed.native_claim_basis_id = NativeClaimBasisId::from_bytes([99; 32]);
    assert_ne!(changed.id().unwrap(), bundle.id().unwrap());
}
