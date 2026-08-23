use clutch_product_series::*;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

const BURN_REGISTRY_VALUE: u16 = 7;

fn id(byte: u8) -> ContentId {
    ContentId::from_bytes([byte; 32])
}

fn basis() -> NativeClaimBasisV1 {
    let mut payout_weights = [[0; MAX_OUTCOMES]; MAX_PAYOUTS];
    let mut index = 0_usize;
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

fn smooth_basis(degree: u8) -> NativeClaimBasisV1 {
    let mut knots = [0; MAX_OUTCOMES];
    let knot_count = match degree {
        1 => 4,
        2 => 3,
        3 => 2,
        _ => panic!("test fixture degree"),
    };
    let mut index = 0_usize;
    while index < usize::from(knot_count) {
        knots[index] = u128::from(u64::try_from(index).unwrap()) * 8;
        index += 1;
    }
    NativeClaimBasisV1 {
        basis_degree: degree,
        outcome_count: 4,
        payout_count: 0,
        knot_count,
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

fn genesis() -> MarketGenesisProfileV1 {
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

fn registry_for(
    template: &ProductTemplateV4,
    basis: &NativeClaimBasisV1,
    recovery: &EvidenceOnlyRecoveryPolicyV1,
    genesis: &MarketGenesisProfileV1,
) -> RegistryCapabilityProjectionV1 {
    RegistryCapabilityProjectionV1 {
        registry_release_id: id(70),
        capability_profile_id: genesis.capability_profile_id,
        statistic_registry_value: template.statistic_registry_value,
        coverage_policy_registry_value: template.coverage_policy_registry_value,
        ambiguity_policy_registry_value: basis.ambiguity_policy_registry_value,
        edge_policy_registry_value: basis.edge_policy_registry_value,
        burn_terminal_disposition_registry_value: BURN_REGISTRY_VALUE,
        supported_basis_degrees: [true; 4],
        max_outcome_count: 16,
        max_degree_zero_payout_count: 16,
        max_recovery_attempt_count: 8,
        min_coverage_policy_parameter: 0,
        max_coverage_policy_parameter: u64::MAX,
        max_window_span_buckets: u64::MAX,
        max_series_instance_count: u32::MAX,
        semantic_owners: CapabilitySemanticOwnersV1 {
            source_plane_contract_id: template.source_plane_contract_id,
            source_spec_id: template.source_spec_id,
            summary_program_id: template.summary_program_id,
            native_claim_basis_id: basis.id().unwrap(),
            evidence_only_recovery_policy_id: recovery.id().unwrap(),
            product_compiler_release_id: template.compiler_release_id,
            price_grid_id: genesis.price_grid_id,
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

fn registry() -> RegistryCapabilityProjectionV1 {
    registry_for(&template(), &basis(), &recovery(), &genesis())
}

fn attachment() -> SeriesAttachmentPlanV1 {
    SeriesAttachmentPlanV1 {
        funding_quote_id: quote().id().unwrap(),
        liquidity_facility_plan_id: id(41),
        wrapper_recipe_set_id: id(42),
    }
}

fn series_for(
    template: &ProductTemplateV4,
    genesis: &MarketGenesisProfileV1,
    attachment: &SeriesAttachmentPlanV1,
) -> SeriesPlanV4 {
    SeriesPlanV4 {
        product_template_id: template.id().unwrap(),
        market_genesis_profile_id: genesis.id().unwrap(),
        attachment_plan_id: attachment.id().unwrap(),
        first_start_bucket: 100,
        stride_buckets: 10,
        instance_count: 3,
        creation_lead_buckets: 5,
        market_collateral_cap: 1_000,
    }
}

fn series() -> SeriesPlanV4 {
    series_for(&template(), &genesis(), &attachment())
}

fn funding_terms() -> SeriesFundingTermsV1 {
    SeriesFundingTermsV1 {
        series_plan_id: series().id().unwrap(),
        lamport_principal_refund: id(50),
        collateral_principal_refund_token_account: id(51),
        neutral_sink: id(52),
        collateral_mint: id(53),
        token_program: id(54),
    }
}

fn assert_codec<T>(value: &T)
where
    T: FixedCodec + core::fmt::Debug + Eq,
{
    let mut encoded = vec![0xa5; T::ENCODED_LEN];
    value.encode_into(&mut encoded).unwrap();
    assert_eq!(T::decode(&encoded).unwrap(), *value);

    let mut reencoded = vec![0xa5; T::ENCODED_LEN];
    T::decode(&encoded)
        .unwrap()
        .encode_into(&mut reencoded)
        .unwrap();
    assert_eq!(encoded, reencoded);
    assert_eq!(
        T::decode(&encoded[..encoded.len() - 1]),
        Err(Error::Truncated)
    );
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(T::decode(&trailing), Err(Error::TrailingBytes));
    let mut bad_magic = encoded;
    bad_magic[0] ^= 1;
    assert_eq!(T::decode(&bad_magic), Err(Error::BadMagic));
}

#[test]
fn every_codec_round_trips_and_refuses_wrong_length_or_magic() {
    assert_codec(&basis());
    assert_codec(&recovery());
    assert_codec(&template());
    assert_codec(&genesis());
    let market = MarketInstancePreimageV1 {
        product_template_id: template().id().unwrap(),
        market_genesis_profile_id: genesis().id().unwrap(),
        start_bucket: 100,
        collateral_cap: 1_000,
    };
    assert_codec(&market);
    assert_codec(&quote());
    assert_codec(&attachment());
    assert_codec(&series());
    assert_codec(&funding_terms());
}

#[test]
fn canonical_offsets_match_the_documented_wire() {
    let mut basis_bytes = [0; BASIS_BYTES];
    basis().encode_into(&mut basis_bytes).unwrap();
    assert_eq!(&basis_bytes[0..8], b"DCBASIS1");
    assert_eq!(&basis_bytes[18..24], &[0; 6]);
    assert_eq!(
        u64::from_le_bytes(basis_bytes[24..32].try_into().unwrap()),
        1_000
    );
    assert_eq!(&basis_bytes[2_080..2_096], &basis().payout_map);

    let mut template_bytes = [0; PRODUCT_TEMPLATE_BYTES];
    template().encode_into(&mut template_bytes).unwrap();
    assert_eq!(&template_bytes[0..8], b"DCTMPLV4");
    assert_eq!(&template_bytes[240..256], &[0; 16]);

    let mut genesis_bytes = [0; MARKET_GENESIS_PROFILE_BYTES];
    genesis().encode_into(&mut genesis_bytes).unwrap();
    assert_eq!(&genesis_bytes[12..16], &[0; 4]);
    assert_eq!(&genesis_bytes[344..352], &[0; 8]);

    let mut quote_bytes = [0; SERIES_FUNDING_QUOTE_BYTES];
    quote().encode_into(&mut quote_bytes).unwrap();
    assert_eq!(&quote_bytes[0..8], b"DCFQUOT1");
    assert_eq!(quote_bytes[10], 2);
    assert_eq!(&quote_bytes[11..16], &[0; 5]);
    assert_eq!(&quote_bytes[16..48], &recovery().id().unwrap().bytes());
    assert_eq!(
        u64::from_le_bytes(quote_bytes[64..72].try_into().unwrap()),
        40
    );
    assert_eq!(
        u64::from_le_bytes(quote_bytes[128..136].try_into().unwrap()),
        11
    );
    assert_eq!(&quote_bytes[168..264], &[0; 96]);
}

#[test]
fn market_identity_excludes_attachment_and_funding_choices() {
    let baseline = compile_ordinal(
        &series(),
        &template(),
        &basis(),
        &recovery(),
        &genesis(),
        &attachment(),
        &registry(),
        0,
    )
    .unwrap();

    for changed_attachment in [
        SeriesAttachmentPlanV1 {
            funding_quote_id: SeriesFundingQuoteId::from_bytes([60; 32]),
            ..attachment()
        },
        SeriesAttachmentPlanV1 {
            liquidity_facility_plan_id: id(61),
            ..attachment()
        },
        SeriesAttachmentPlanV1 {
            wrapper_recipe_set_id: id(62),
            ..attachment()
        },
    ] {
        let changed_series = series_for(&template(), &genesis(), &changed_attachment);
        let compiled = compile_ordinal(
            &changed_series,
            &template(),
            &basis(),
            &recovery(),
            &genesis(),
            &changed_attachment,
            &registry(),
            0,
        )
        .unwrap();
        assert_eq!(compiled.market_instance_id, baseline.market_instance_id);
        assert_ne!(compiled.attachment_plan_id, baseline.attachment_plan_id);
        assert_ne!(compiled.series_plan_id, baseline.series_plan_id);
    }

    let mut changed_refund = funding_terms();
    changed_refund.lamport_principal_refund = id(63);
    assert_ne!(changed_refund.id().unwrap(), funding_terms().id().unwrap());
    assert_eq!(baseline.market_instance_id, baseline.market.id().unwrap());
}

#[test]
fn market_identity_commits_template_profile_start_and_cap() {
    let baseline = compile_ordinal(
        &series(),
        &template(),
        &basis(),
        &recovery(),
        &genesis(),
        &attachment(),
        &registry(),
        0,
    )
    .unwrap();

    let mut changed_template = template();
    changed_template.statistic_registry_value += 1;
    let changed_template_series = series_for(&changed_template, &genesis(), &attachment());
    let compiled = compile_ordinal(
        &changed_template_series,
        &changed_template,
        &basis(),
        &recovery(),
        &genesis(),
        &attachment(),
        &registry_for(&changed_template, &basis(), &recovery(), &genesis()),
        0,
    )
    .unwrap();
    assert_ne!(compiled.market_instance_id, baseline.market_instance_id);

    let mut changed_genesis = genesis();
    changed_genesis.profile_id = id(64);
    let changed_genesis_series = series_for(&template(), &changed_genesis, &attachment());
    let compiled = compile_ordinal(
        &changed_genesis_series,
        &template(),
        &basis(),
        &recovery(),
        &changed_genesis,
        &attachment(),
        &registry_for(&template(), &basis(), &recovery(), &changed_genesis),
        0,
    )
    .unwrap();
    assert_ne!(compiled.market_instance_id, baseline.market_instance_id);

    let ordinal_one = compile_ordinal(
        &series(),
        &template(),
        &basis(),
        &recovery(),
        &genesis(),
        &attachment(),
        &registry(),
        1,
    )
    .unwrap();
    assert_ne!(ordinal_one.market_instance_id, baseline.market_instance_id);

    let mut changed_cap = series();
    changed_cap.market_collateral_cap = 2_000;
    let compiled = compile_ordinal(
        &changed_cap,
        &template(),
        &basis(),
        &recovery(),
        &genesis(),
        &attachment(),
        &registry(),
        0,
    )
    .unwrap();
    assert_ne!(compiled.market_instance_id, baseline.market_instance_id);
}

#[test]
fn market_cap_is_one_or_more_exact_native_lots() {
    let mut below_lot = series();
    below_lot.market_collateral_cap = genesis().native_bearer_lot - 1;
    assert_eq!(
        compile_ordinal(
            &below_lot,
            &template(),
            &basis(),
            &recovery(),
            &genesis(),
            &attachment(),
            &registry(),
            0,
        ),
        Err(Error::MismatchedArtifact)
    );

    let mut fractional_lot = series();
    fractional_lot.market_collateral_cap = genesis().native_bearer_lot + 1;
    assert_eq!(
        compile_ordinal(
            &fractional_lot,
            &template(),
            &basis(),
            &recovery(),
            &genesis(),
            &attachment(),
            &registry(),
            0,
        ),
        Err(Error::MismatchedArtifact)
    );

    let hostile_market = MarketInstancePreimageV1 {
        product_template_id: template().id().unwrap(),
        market_genesis_profile_id: genesis().id().unwrap(),
        start_bucket: 100,
        collateral_cap: 200,
    };
    assert_eq!(
        hostile_market.validate_bindings(&template(), &genesis()),
        Err(Error::MismatchedArtifact)
    );
}

#[test]
fn joins_and_schedule_are_exact_and_checked() {
    let compiled = compile_ordinal(
        &series(),
        &template(),
        &basis(),
        &recovery(),
        &genesis(),
        &attachment(),
        &registry(),
        0,
    )
    .unwrap();
    assert_eq!(compiled.schedule.start_bucket, 100);
    assert_eq!(compiled.schedule.end_bucket_exclusive, 104);
    assert_eq!(compiled.schedule.primary_maturity_bucket_exclusive, 106);
    assert_eq!(compiled.schedule.recovery_attempts[0].repair_generation, 10);
    assert_eq!(compiled.schedule.recovery_attempts[0].opens_at_bucket, 106);
    assert_eq!(compiled.schedule.recovery_attempts[1].closes_at_bucket, 111);
    assert!(series().is_creation_eligible(0, 95).unwrap());
    assert!(!series().is_creation_eligible(0, 100).unwrap());
    assert_eq!(series().start_bucket(3), Err(Error::WrongOrdinal));
    funding_terms()
        .validate_bindings(
            &series(),
            &template(),
            &basis(),
            &recovery(),
            &genesis(),
            &registry(),
        )
        .unwrap();
    let mut wrong_collateral = registry();
    wrong_collateral.realm_collateral.collateral_mint = id(99);
    assert_eq!(
        funding_terms().validate_bindings(
            &series(),
            &template(),
            &basis(),
            &recovery(),
            &genesis(),
            &wrong_collateral,
        ),
        Err(Error::MismatchedArtifact)
    );

    let mut wrong_series = series();
    wrong_series.product_template_id = ProductTemplateId::from_bytes([99; 32]);
    assert_eq!(
        compile_ordinal(
            &wrong_series,
            &template(),
            &basis(),
            &recovery(),
            &genesis(),
            &attachment(),
            &registry(),
            0,
        ),
        Err(Error::MismatchedArtifact)
    );
    let mut wrong_burn = registry();
    wrong_burn.burn_terminal_disposition_registry_value = BURN_REGISTRY_VALUE + 1;
    assert_eq!(
        wrong_burn.validate_complete_join(
            &series(),
            &template(),
            &basis(),
            &recovery(),
            &genesis(),
        ),
        Err(Error::MismatchedArtifact)
    );
}

#[test]
fn registry_profile_join_is_total_and_realm_bounded() {
    let validate = |projection: &RegistryCapabilityProjectionV1| {
        projection.validate_complete_join(&series(), &template(), &basis(), &recovery(), &genesis())
    };
    validate(&registry()).unwrap();

    let mut wrong_statistic = registry();
    wrong_statistic.statistic_registry_value += 1;
    assert_eq!(validate(&wrong_statistic), Err(Error::MismatchedArtifact));
    let mut wrong_coverage = registry();
    wrong_coverage.coverage_policy_registry_value += 1;
    assert_eq!(validate(&wrong_coverage), Err(Error::MismatchedArtifact));
    let mut wrong_ambiguity = registry();
    wrong_ambiguity.ambiguity_policy_registry_value += 1;
    assert_eq!(validate(&wrong_ambiguity), Err(Error::MismatchedArtifact));
    let mut wrong_edge = registry();
    wrong_edge.edge_policy_registry_value += 1;
    assert_eq!(validate(&wrong_edge), Err(Error::MismatchedArtifact));
    let mut wrong_burn = registry();
    wrong_burn.burn_terminal_disposition_registry_value += 1;
    assert_eq!(validate(&wrong_burn), Err(Error::MismatchedArtifact));
    let mut wrong_profile = registry();
    wrong_profile.capability_profile_id = id(99);
    assert_eq!(validate(&wrong_profile), Err(Error::MismatchedArtifact));
    let mut wrong_source_owner = registry();
    wrong_source_owner.semantic_owners.source_plane_contract_id = id(99);
    assert_eq!(
        validate(&wrong_source_owner),
        Err(Error::MismatchedArtifact)
    );
    let mut wrong_lifecycle_owner = registry();
    wrong_lifecycle_owner
        .semantic_owners
        .candidate_lifecycle_policy_id = id(99);
    assert_eq!(
        validate(&wrong_lifecycle_owner),
        Err(Error::MismatchedArtifact)
    );
    let mut wrong_realm = registry();
    wrong_realm.realm_collateral.realm_id = id(99);
    assert_eq!(validate(&wrong_realm), Err(Error::MismatchedArtifact));
    let mut wrong_realm_profile = registry();
    wrong_realm_profile.realm_collateral.profile_id = id(99);
    assert_eq!(
        validate(&wrong_realm_profile),
        Err(Error::MismatchedArtifact)
    );

    let mut unsupported_degree = registry();
    unsupported_degree.supported_basis_degrees[0] = false;
    assert_eq!(
        validate(&unsupported_degree),
        Err(Error::UnsupportedCapability)
    );
    let mut unsupported_outcomes = registry();
    unsupported_outcomes.max_outcome_count = 3;
    assert_eq!(
        validate(&unsupported_outcomes),
        Err(Error::UnsupportedCapability)
    );
    let mut unsupported_payouts = registry();
    unsupported_payouts.max_degree_zero_payout_count = 3;
    assert_eq!(
        validate(&unsupported_payouts),
        Err(Error::UnsupportedCapability)
    );
    let mut unsupported_recovery = registry();
    unsupported_recovery.max_recovery_attempt_count = 1;
    assert_eq!(
        validate(&unsupported_recovery),
        Err(Error::UnsupportedCapability)
    );
    let mut unsupported_parameter = registry();
    unsupported_parameter.min_coverage_policy_parameter = 1;
    assert_eq!(
        validate(&unsupported_parameter),
        Err(Error::UnsupportedCapability)
    );
    let mut unsupported_window = registry();
    unsupported_window.max_window_span_buckets = 3;
    assert_eq!(
        validate(&unsupported_window),
        Err(Error::UnsupportedCapability)
    );
    let mut unsupported_count = registry();
    unsupported_count.max_series_instance_count = 2;
    assert_eq!(
        validate(&unsupported_count),
        Err(Error::UnsupportedCapability)
    );
    let mut unsupported_cap = registry();
    unsupported_cap
        .realm_collateral
        .market_collateral_cap_ceiling = 999;
    assert_eq!(
        validate(&unsupported_cap),
        Err(Error::UnsupportedCapability)
    );

    let mut wrong_sink = funding_terms();
    wrong_sink.neutral_sink = id(99);
    assert_eq!(
        wrong_sink.validate_bindings(
            &series(),
            &template(),
            &basis(),
            &recovery(),
            &genesis(),
            &registry(),
        ),
        Err(Error::MismatchedArtifact)
    );

    let mut unrelated_template = template();
    unrelated_template.statistic_registry_value += 1;
    let unrelated_registry = registry_for(&unrelated_template, &basis(), &recovery(), &genesis());
    assert_eq!(
        funding_terms().validate_bindings(
            &series(),
            &unrelated_template,
            &basis(),
            &recovery(),
            &genesis(),
            &unrelated_registry,
        ),
        Err(Error::MismatchedArtifact)
    );
}

#[test]
fn compiled_schedule_validation_is_complete_and_canonical() {
    let compiled = compile_ordinal(
        &series(),
        &template(),
        &basis(),
        &recovery(),
        &genesis(),
        &attachment(),
        &registry(),
        0,
    )
    .unwrap();
    compiled.schedule.validate().unwrap();

    let mut no_attempts = compiled.schedule;
    no_attempts.recovery_attempt_count = 0;
    assert_eq!(no_attempts.validate(), Err(Error::InvalidSchedule));
    let mut empty_window = compiled.schedule;
    empty_window.end_bucket_exclusive = empty_window.start_bucket;
    assert_eq!(empty_window.validate(), Err(Error::InvalidSchedule));
    let mut maturity_before_end = compiled.schedule;
    maturity_before_end.primary_maturity_bucket_exclusive =
        maturity_before_end.end_bucket_exclusive - 1;
    assert_eq!(maturity_before_end.validate(), Err(Error::InvalidSchedule));
    let mut opens_before_maturity = compiled.schedule;
    opens_before_maturity.recovery_attempts[0].opens_at_bucket -= 1;
    assert_eq!(
        opens_before_maturity.validate(),
        Err(Error::InvalidSchedule)
    );
    let mut empty_attempt = compiled.schedule;
    empty_attempt.recovery_attempts[0].closes_at_bucket =
        empty_attempt.recovery_attempts[0].opens_at_bucket;
    assert_eq!(empty_attempt.validate(), Err(Error::InvalidSchedule));
    let mut overlap = compiled.schedule;
    overlap.recovery_attempts[1].opens_at_bucket =
        overlap.recovery_attempts[0].closes_at_bucket - 1;
    assert_eq!(overlap.validate(), Err(Error::InvalidSchedule));
    let mut equal_generation = compiled.schedule;
    equal_generation.recovery_attempts[1].repair_generation =
        equal_generation.recovery_attempts[0].repair_generation;
    assert_eq!(equal_generation.validate(), Err(Error::InvalidSchedule));
    let mut dirty_tail = compiled.schedule;
    dirty_tail.recovery_attempts[2].closes_at_bucket = 1;
    assert_eq!(dirty_tail.validate(), Err(Error::NonCanonicalPadding));
}

#[test]
fn padding_order_and_arithmetic_refuse() {
    let mut padded_basis = basis();
    padded_basis.payout_weights[4][0] = 1;
    assert_eq!(padded_basis.validate(), Err(Error::NonCanonicalPadding));
    let mut mapped_basis = basis();
    mapped_basis.payout_map[4] = 0;
    assert_eq!(mapped_basis.validate(), Err(Error::NonCanonicalPadding));
    let mut unordered_knots = basis();
    unordered_knots.knots[1] = unordered_knots.knots[0];
    assert_eq!(unordered_knots.validate(), Err(Error::InvalidParameter));

    let mut overlapping = recovery();
    overlapping.attempts[1].opens_after_primary_maturity_buckets = 1;
    assert_eq!(overlapping.validate(), Err(Error::InvalidSchedule));
    let mut padded_recovery = recovery();
    padded_recovery.attempts[2].closes_after_primary_maturity_buckets = 1;
    assert_eq!(padded_recovery.validate(), Err(Error::NonCanonicalPadding));

    let mut generation_overflow = template();
    generation_overflow.base_repair_generation = u64::MAX;
    assert_eq!(
        generation_overflow.validate_bindings(&basis(), &recovery()),
        Err(Error::ArithmeticOverflow)
    );

    let mut final_deadline_overflow = series();
    final_deadline_overflow.first_start_bucket = u64::MAX - 5;
    final_deadline_overflow.instance_count = 1;
    final_deadline_overflow.stride_buckets = 0;
    assert_eq!(
        final_deadline_overflow.validate_bindings(
            &template(),
            &basis(),
            &recovery(),
            &genesis(),
            &attachment(),
            &registry(),
        ),
        Err(Error::ArithmeticOverflow)
    );
}

#[test]
fn native_basis_union_has_one_canonical_representation() {
    let mut degree_zero_spacing = basis();
    degree_zero_spacing.uniform_log2_spacing = 0;
    assert_eq!(degree_zero_spacing.validate(), Err(Error::InvalidParameter));

    let mut unreachable_row = basis();
    unreachable_row.payout_map[3] = 2;
    assert_eq!(unreachable_row.validate(), Err(Error::InvalidParameter));

    let mut skipped_first_use = basis();
    skipped_first_use.payout_map[..4].copy_from_slice(&[0, 2, 1, 3]);
    assert_eq!(skipped_first_use.validate(), Err(Error::InvalidParameter));

    let mut duplicate_row = basis();
    duplicate_row.payout_weights[1] = duplicate_row.payout_weights[0];
    assert_eq!(duplicate_row.validate(), Err(Error::InvalidParameter));

    let mut permuted_rows = basis();
    permuted_rows.payout_weights.swap(0, 1);
    permuted_rows.payout_map[..4].copy_from_slice(&[1, 0, 2, 3]);
    assert_eq!(permuted_rows.validate(), Err(Error::InvalidParameter));

    for degree in [1, 2, 3] {
        smooth_basis(degree).validate().unwrap();
    }

    let mut smooth_preset = smooth_basis(1);
    smooth_preset.payout_count = 1;
    smooth_preset.payout_weights[0][0] = smooth_preset.denominator;
    assert_eq!(smooth_preset.validate(), Err(Error::InvalidParameter));

    let mut smooth_baggage = smooth_basis(2);
    smooth_baggage.payout_weights[0][0] = 1;
    assert_eq!(smooth_baggage.validate(), Err(Error::NonCanonicalPadding));

    let mut smooth_map = smooth_basis(3);
    smooth_map.payout_map[0] = 0;
    assert_eq!(smooth_map.validate(), Err(Error::NonCanonicalPadding));

    let mut degree_one_alias = smooth_basis(1);
    degree_one_alias.uniform_log2_spacing = UNIFORM_SPACING_NONE;
    assert_eq!(degree_one_alias.validate(), Err(Error::InvalidParameter));

    let mut nonuniform_degree_one = smooth_basis(1);
    nonuniform_degree_one.knots[..4].copy_from_slice(&[0, 8, 20, 32]);
    nonuniform_degree_one.uniform_log2_spacing = UNIFORM_SPACING_NONE;
    nonuniform_degree_one.validate().unwrap();
    nonuniform_degree_one.uniform_log2_spacing = 3;
    assert_eq!(
        nonuniform_degree_one.validate(),
        Err(Error::InvalidParameter)
    );
}

#[test]
fn recurrence_has_no_dead_stride_generation_or_policy_caps() {
    let mut equal_generation = recovery();
    equal_generation.attempts[1].repair_generation_delta =
        equal_generation.attempts[0].repair_generation_delta;
    assert_eq!(equal_generation.validate(), Err(Error::InvalidSchedule));

    let mut wide_generation = recovery();
    wide_generation.attempts[1].repair_generation_delta = u64::from(u32::MAX) + 1;
    assert_codec(&wide_generation);

    let mut singleton = series();
    singleton.instance_count = 1;
    singleton.stride_buckets = 0;
    singleton.validate_shape().unwrap();
    singleton.stride_buckets = 1;
    assert_eq!(singleton.validate_shape(), Err(Error::InvalidParameter));

    let mut multiple = series();
    multiple.stride_buckets = 0;
    assert_eq!(multiple.validate_shape(), Err(Error::InvalidParameter));

    let mut formerly_capped_window = template();
    formerly_capped_window.window_span_buckets = 1_000_001;
    formerly_capped_window.validate_shape().unwrap();

    let mut formerly_capped_count = series();
    formerly_capped_count.instance_count = u32::MAX;
    formerly_capped_count.stride_buckets = 1;
    formerly_capped_count.validate_shape().unwrap();
}

#[test]
fn hostile_reserved_bytes_and_legacy_fallback_refuse() {
    let mut bytes = [0; PRODUCT_TEMPLATE_BYTES];
    template().encode_into(&mut bytes).unwrap();
    bytes[14] = 1;
    assert_eq!(
        ProductTemplateV4::decode(&bytes),
        Err(Error::NonCanonicalReserved)
    );

    let mut basis_bytes = [0; BASIS_BYTES];
    basis().encode_into(&mut basis_bytes).unwrap();
    basis_bytes[20] = 1;
    assert_eq!(
        NativeClaimBasisV1::decode(&basis_bytes),
        Err(Error::NonCanonicalReserved)
    );

    let mut legacy_template = [0; 248];
    legacy_template[..8].copy_from_slice(b"DCTMPLV3");
    assert_eq!(
        ProductTemplateV4::decode(&legacy_template),
        Err(Error::LegacyNumericFallback)
    );
    let mut legacy_payout = [0; 1_104];
    legacy_payout[..8].copy_from_slice(b"DCPAYTV3");
    assert_eq!(
        NativeClaimBasisV1::decode(&legacy_payout),
        Err(Error::LegacyNumericFallback)
    );
}

fn quote() -> SeriesFundingQuoteV1 {
    let mut recovery_attempt_funding = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
    recovery_attempt_funding[0] = RecoveryAttemptFundingV1 {
        max_progress_units: 3,
        lamports_per_progress_unit: 5,
    };
    recovery_attempt_funding[1] = RecoveryAttemptFundingV1 {
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
        recovery_attempt_funding,
        recovery_rent_principal_lamports: 11,
    }
}

fn market_instance_id() -> MarketInstanceId {
    MarketInstancePreimageV1 {
        product_template_id: template().id().unwrap(),
        market_genesis_profile_id: genesis().id().unwrap(),
        start_bucket: 100,
        collateral_cap: 1_000,
    }
    .id()
    .unwrap()
}

fn authenticated_status(
    market_core: AdapterAuthenticatedComponentStatusV1,
    recovery_reserve: AdapterAuthenticatedComponentStatusV1,
    source_work: AdapterAuthenticatedComponentStatusV1,
    liquidity_facility: AdapterAuthenticatedComponentStatusV1,
    wrapper_set: AdapterAuthenticatedComponentStatusV1,
) -> AdapterAuthenticatedFulfillmentStatusV1 {
    AdapterAuthenticatedFulfillmentStatusV1 {
        market_instance_id: market_instance_id(),
        attachment_plan_id: attachment().id().unwrap(),
        funding_quote_id: quote().id().unwrap(),
        market_core,
        recovery_reserve,
        source_work,
        liquidity_facility,
        wrapper_set,
    }
}

#[test]
fn component_projection_debits_only_absent_exact_components() {
    let all_absent = authenticated_status(
        AdapterAuthenticatedComponentStatusV1::Absent,
        AdapterAuthenticatedComponentStatusV1::Absent,
        AdapterAuthenticatedComponentStatusV1::Absent,
        AdapterAuthenticatedComponentStatusV1::Absent,
        AdapterAuthenticatedComponentStatusV1::Absent,
    );
    let projected = project_component_debits(
        market_instance_id(),
        &recovery(),
        &attachment(),
        &quote(),
        all_absent,
        FundingBalancesV1 {
            lamports: 200,
            collateral_atoms: 200,
        },
    )
    .unwrap();
    assert_eq!(projected.total.lamports, 170);
    assert_eq!(projected.total.collateral_atoms, 110);
    assert_eq!(projected.remaining.lamports, 30);
    assert_eq!(projected.remaining.collateral_atoms, 90);

    let reuse_core_and_wrapper = authenticated_status(
        AdapterAuthenticatedComponentStatusV1::PresentExactAndCapitalized,
        AdapterAuthenticatedComponentStatusV1::PresentExactAndCapitalized,
        AdapterAuthenticatedComponentStatusV1::Absent,
        AdapterAuthenticatedComponentStatusV1::Absent,
        AdapterAuthenticatedComponentStatusV1::PresentExactAndCapitalized,
    );
    let projected = project_component_debits(
        market_instance_id(),
        &recovery(),
        &attachment(),
        &quote(),
        reuse_core_and_wrapper,
        FundingBalancesV1 {
            lamports: 70,
            collateral_atoms: 100,
        },
    )
    .unwrap();
    assert_eq!(projected.market_core, ComponentDebitV1::ZERO);
    assert_eq!(projected.recovery_reserve, ComponentDebitV1::ZERO);
    assert_eq!(projected.wrapper_set, ComponentDebitV1::ZERO);
    assert_eq!(projected.total.lamports, 70);
    assert_eq!(projected.total.collateral_atoms, 100);
    assert_eq!(projected.remaining.lamports, 0);
    assert_eq!(projected.remaining.collateral_atoms, 0);

    let incoherent = AdapterAuthenticatedFulfillmentStatusV1 {
        recovery_reserve: AdapterAuthenticatedComponentStatusV1::Absent,
        ..reuse_core_and_wrapper
    };
    assert_eq!(
        project_component_debits(
            market_instance_id(),
            &recovery(),
            &attachment(),
            &quote(),
            incoherent,
            FundingBalancesV1 {
                lamports: u64::MAX,
                collateral_atoms: u64::MAX,
            },
        ),
        Err(Error::InvalidComponentStatus)
    );
    assert_eq!(
        project_component_debits(
            market_instance_id(),
            &recovery(),
            &attachment(),
            &quote(),
            all_absent,
            FundingBalancesV1 {
                lamports: 149,
                collateral_atoms: 110,
            },
        ),
        Err(Error::InsufficientPrepayment)
    );
}

#[test]
fn component_sum_overflow_refuses() {
    let mut hostile_quote = quote();
    hostile_quote.market_core.lamports = u64::MAX;
    let hostile_attachment = SeriesAttachmentPlanV1 {
        funding_quote_id: hostile_quote.id().unwrap(),
        ..attachment()
    };
    let hostile_status = AdapterAuthenticatedFulfillmentStatusV1 {
        attachment_plan_id: hostile_attachment.id().unwrap(),
        funding_quote_id: hostile_quote.id().unwrap(),
        ..authenticated_status(
            AdapterAuthenticatedComponentStatusV1::Absent,
            AdapterAuthenticatedComponentStatusV1::Absent,
            AdapterAuthenticatedComponentStatusV1::Absent,
            AdapterAuthenticatedComponentStatusV1::Absent,
            AdapterAuthenticatedComponentStatusV1::Absent,
        )
    };
    assert_eq!(
        project_component_debits(
            market_instance_id(),
            &recovery(),
            &hostile_attachment,
            &hostile_quote,
            hostile_status,
            FundingBalancesV1 {
                lamports: u64::MAX,
                collateral_atoms: u64::MAX,
            },
        ),
        Err(Error::ArithmeticOverflow)
    );
}

#[test]
fn forged_quote_or_component_status_refuses() {
    let all_absent = authenticated_status(
        AdapterAuthenticatedComponentStatusV1::Absent,
        AdapterAuthenticatedComponentStatusV1::Absent,
        AdapterAuthenticatedComponentStatusV1::Absent,
        AdapterAuthenticatedComponentStatusV1::Absent,
        AdapterAuthenticatedComponentStatusV1::Absent,
    );
    let mut forged_quote_body = quote();
    forged_quote_body.market_core.lamports += 1;
    assert_eq!(
        project_component_debits(
            market_instance_id(),
            &recovery(),
            &attachment(),
            &forged_quote_body,
            all_absent,
            FundingBalancesV1 {
                lamports: u64::MAX,
                collateral_atoms: u64::MAX,
            },
        ),
        Err(Error::MismatchedArtifact)
    );

    let mut substituted_recovery_prices = quote();
    substituted_recovery_prices.recovery_attempt_funding[0] = RecoveryAttemptFundingV1 {
        max_progress_units: 1,
        lamports_per_progress_unit: 15,
    };
    assert_eq!(
        substituted_recovery_prices
            .recovery_work_principal_lamports()
            .unwrap(),
        quote().recovery_work_principal_lamports().unwrap()
    );
    assert_ne!(
        substituted_recovery_prices.id().unwrap(),
        quote().id().unwrap()
    );
    assert_eq!(
        project_component_debits(
            market_instance_id(),
            &recovery(),
            &attachment(),
            &substituted_recovery_prices,
            all_absent,
            FundingBalancesV1 {
                lamports: u64::MAX,
                collateral_atoms: u64::MAX,
            },
        ),
        Err(Error::MismatchedArtifact)
    );

    let mut wrong_policy = quote();
    wrong_policy.evidence_only_recovery_policy_id =
        EvidenceOnlyRecoveryPolicyId::from_bytes([99; 32]);
    assert_eq!(
        wrong_policy.validate_recovery_binding(&recovery()),
        Err(Error::MismatchedArtifact)
    );
    assert_eq!(
        project_component_debits(
            market_instance_id(),
            &recovery(),
            &attachment(),
            &wrong_policy,
            all_absent,
            FundingBalancesV1 {
                lamports: u64::MAX,
                collateral_atoms: u64::MAX,
            },
        ),
        Err(Error::MismatchedArtifact)
    );
    let mut wrong_attempt_count = quote();
    wrong_attempt_count.recovery_attempt_count = 1;
    assert_eq!(
        wrong_attempt_count.validate(),
        Err(Error::NonCanonicalPadding)
    );
    let mut wrong_recovery_total = quote();
    wrong_recovery_total.recovery_rent_principal_lamports += 1;
    assert_eq!(
        wrong_recovery_total.validate(),
        Err(Error::InvalidParameter)
    );
    let mut unmodeled_collateral = quote();
    unmodeled_collateral.recovery_reserve.collateral_atoms = 1;
    assert_eq!(
        unmodeled_collateral.validate(),
        Err(Error::InvalidParameter)
    );

    for forged_status in [
        AdapterAuthenticatedFulfillmentStatusV1 {
            market_instance_id: MarketInstanceId::from_bytes([99; 32]),
            ..all_absent
        },
        AdapterAuthenticatedFulfillmentStatusV1 {
            attachment_plan_id: SeriesAttachmentPlanId::from_bytes([99; 32]),
            ..all_absent
        },
        AdapterAuthenticatedFulfillmentStatusV1 {
            funding_quote_id: SeriesFundingQuoteId::from_bytes([99; 32]),
            ..all_absent
        },
    ] {
        assert_eq!(
            project_component_debits(
                market_instance_id(),
                &recovery(),
                &attachment(),
                &quote(),
                forged_status,
                FundingBalancesV1 {
                    lamports: u64::MAX,
                    collateral_atoms: u64::MAX,
                },
            ),
            Err(Error::MismatchedArtifact)
        );
    }
}

#[test]
fn deterministic_identity_golden_vectors() {
    let compiled = compile_ordinal(
        &series(),
        &template(),
        &basis(),
        &recovery(),
        &genesis(),
        &attachment(),
        &registry(),
        0,
    )
    .unwrap();
    assert_eq!(
        [
            basis().id().unwrap().bytes(),
            recovery().id().unwrap().bytes(),
            template().id().unwrap().bytes(),
            genesis().id().unwrap().bytes(),
            compiled.market_instance_id.bytes(),
            quote().id().unwrap().bytes(),
            attachment().id().unwrap().bytes(),
            series().id().unwrap().bytes(),
            funding_terms().id().unwrap().bytes(),
        ],
        [
            [
                205, 152, 162, 98, 175, 240, 251, 190, 21, 58, 70, 242, 215, 33, 11, 168, 177, 74,
                20, 132, 58, 204, 192, 161, 192, 67, 30, 206, 64, 172, 185, 202,
            ],
            [
                228, 96, 67, 130, 93, 47, 247, 220, 81, 170, 203, 116, 200, 94, 253, 201, 81, 225,
                70, 34, 47, 230, 13, 82, 121, 213, 7, 54, 234, 93, 197, 82,
            ],
            [
                67, 220, 150, 115, 38, 189, 223, 246, 157, 184, 107, 77, 75, 229, 139, 171, 103,
                165, 130, 177, 252, 85, 131, 38, 170, 156, 126, 76, 188, 21, 132, 25,
            ],
            [
                26, 24, 210, 101, 136, 51, 241, 47, 184, 234, 103, 169, 161, 255, 107, 183, 51, 28,
                230, 71, 254, 231, 47, 225, 27, 83, 109, 16, 95, 41, 47, 54,
            ],
            [
                118, 26, 171, 35, 40, 113, 45, 174, 247, 93, 34, 252, 227, 251, 125, 48, 222, 121,
                189, 24, 152, 105, 172, 23, 255, 111, 148, 28, 178, 174, 160, 249,
            ],
            [
                68, 241, 130, 5, 71, 20, 73, 69, 94, 233, 146, 236, 6, 19, 54, 47, 86, 81, 97, 60,
                44, 152, 25, 86, 133, 96, 238, 187, 214, 156, 252, 153,
            ],
            [
                53, 209, 231, 66, 117, 164, 238, 33, 209, 159, 43, 104, 104, 138, 222, 251, 177,
                78, 44, 254, 110, 164, 202, 238, 248, 61, 143, 155, 118, 141, 96, 47,
            ],
            [
                110, 126, 167, 24, 125, 177, 92, 30, 99, 9, 56, 235, 241, 43, 177, 120, 139, 159,
                0, 137, 224, 118, 156, 41, 164, 82, 112, 146, 44, 155, 229, 138,
            ],
            [
                87, 59, 220, 202, 220, 120, 126, 74, 75, 195, 251, 112, 163, 60, 71, 169, 22, 238,
                226, 124, 145, 74, 102, 24, 106, 176, 224, 159, 82, 119, 125, 83,
            ],
        ]
    );
}

fn id_hex(bytes: [u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

fn assert_manifest_vector(name: &str, body_bytes: usize, domain: &[u8], id_bytes: [u8; 32]) {
    let manifest = include_str!("../vectors/product-series-v1.json");
    let marker = format!("\"type\": \"{name}\"");
    let section = manifest
        .split(&marker)
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    assert!(section.contains(&format!("\"body_bytes\": {body_bytes}")));
    assert!(section.contains(&format!(
        "\"domain\": \"{}\"",
        core::str::from_utf8(domain).unwrap()
    )));
    assert!(section.contains(&format!("\"id_hex\": \"{}\"", id_hex(id_bytes))));
}

#[test]
fn checked_manifest_contains_the_self_contained_fixture_and_every_vector() {
    let manifest = include_str!("../vectors/product-series-v1.json");
    let manifest_digest: [u8; 32] = Sha256::digest(manifest.as_bytes()).into();
    assert_eq!(
        id_hex(manifest_digest),
        "a5e0d540289b7827da5ad65fd61d790ebc3bda457634ba24b538e97fecb71acb"
    );
    let mutated = manifest.replacen(
        "\"first_start_bucket\": 100",
        "\"first_start_bucket\": 101",
        1,
    );
    let mutated_digest: [u8; 32] = Sha256::digest(mutated.as_bytes()).into();
    assert_ne!(mutated_digest, manifest_digest);
    let duplicate_key = manifest.replacen(
        "\"fixture_inputs\": {",
        "\"fixture_inputs\": {\"hostile\": true},\n  \"fixture_inputs\": {",
        1,
    );
    let duplicate_key_digest: [u8; 32] = Sha256::digest(duplicate_key.as_bytes()).into();
    assert_ne!(duplicate_key_digest, manifest_digest);
    const FROZEN_FIXTURE_INPUTS: &str = r#"  "fixture_inputs": {
    "native_claim_basis": {
      "basis_degree": 0,
      "outcome_count": 4,
      "payout_count": 4,
      "knot_count": 3,
      "uniform_log2_spacing": 255,
      "ambiguity_policy_registry_value": 1,
      "edge_policy_registry_value": 1,
      "denominator": 1000,
      "active_payout_weights": [[1000, 0, 0, 0], [0, 1000, 0, 0], [0, 0, 1000, 0], [0, 0, 0, 1000]],
      "active_payout_map": [0, 1, 2, 3],
      "active_knots": [100, 200, 300],
      "padding": "all remaining payout weights and knots are zero; remaining payout-map bytes are 255"
    },
    "evidence_only_recovery": {
      "attempt_count": 2,
      "attempts": [
        {"repair_generation_delta": 0, "opens_after_primary_maturity_buckets": 0, "closes_after_primary_maturity_buckets": 2},
        {"repair_generation_delta": 1, "opens_after_primary_maturity_buckets": 2, "closes_after_primary_maturity_buckets": 5}
      ],
      "padding": "six zero attempts"
    },
    "product_template": {
      "source_plane_contract_id": "[1;32]",
      "source_spec_id": "[2;32]",
      "summary_program_id": "[3;32]",
      "native_claim_basis_id": "cd98a262aff0fbbe153a46f2d7210ba8b14a14843accc0a1c0431ece40acb9ca",
      "evidence_only_recovery_policy_id": "e46043825d2ff7dc51aacb74c85efdc951e146222fe60d5279d50736ea5dc552",
      "compiler_release_id": "[4;32]",
      "statistic_registry_value": 11,
      "coverage_policy_registry_value": 12,
      "window_span_buckets": 4,
      "primary_maturity_grace_buckets": 2,
      "base_repair_generation": 10,
      "coverage_policy_parameter": 0
    },
    "market_genesis_profile": {
      "realm_id": "[20;32]",
      "profile_id": "[21;32]",
      "price_grid_id": "[22;32]",
      "fee_policy_id": "[23;32]",
      "relation_policy_id": "[24;32]",
      "score_policy_id": "[25;32]",
      "candidate_lifecycle_policy_id": "[26;32]",
      "candidate_liveness_policy_id": "[27;32]",
      "retirement_policy_id": "[28;32]",
      "capability_profile_id": "[29;32]",
      "terminal_disposition_registry_value": 7,
      "native_bearer_lot": 1000
    },
    "series_funding_quote": {
      "evidence_only_recovery_policy_id": "e46043825d2ff7dc51aacb74c85efdc951e146222fe60d5279d50736ea5dc552",
      "market_core": {"lamports": 10, "collateral_atoms": 0},
      "recovery_reserve": {"lamports": 40, "collateral_atoms": 0},
      "source_work": {"lamports": 30, "collateral_atoms": 0},
      "liquidity_facility": {"lamports": 40, "collateral_atoms": 100},
      "wrapper_set": {"lamports": 50, "collateral_atoms": 10},
      "recovery_attempt_count": 2,
      "recovery_attempt_funding": [
        {"max_progress_units": 3, "lamports_per_progress_unit": 5},
        {"max_progress_units": 2, "lamports_per_progress_unit": 7}
      ],
      "recovery_rent_principal_lamports": 11,
      "padding": "six zero recovery-attempt funding rows"
    },
    "series_attachment_plan": {
      "funding_quote_id": "44f18205471449455ee992ec0613362f5651613c2c9819568560eebbd69cfc99",
      "liquidity_facility_plan_id": "[41;32]",
      "wrapper_recipe_set_id": "[42;32]"
    },
    "series_plan": {
      "product_template_id": "43dc967326bddff69db86b4d4be58bab67a582b1fc558326aa9c7e4cbc158419",
      "market_genesis_profile_id": "1a18d2658833f12fb8ea67a9a1ff6bb7331ce647fee72fe11b536d105f292f36",
      "attachment_plan_id": "35d1e74275a4ee21d19f2b68688adefbb14e2cfe6ea4caeef83d8f9b768d602f",
      "first_start_bucket": 100,
      "stride_buckets": 10,
      "instance_count": 3,
      "creation_lead_buckets": 5,
      "market_collateral_cap": 1000
    },
    "series_funding_terms": {
      "series_plan_id": "6e7ea7187db15c1e630938ebf12bb1788b9f0089e0769c29a45270922c9be58a",
      "lamport_principal_refund": "[50;32]",
      "collateral_principal_refund_token_account": "[51;32]",
      "neutral_sink": "[52;32]",
      "collateral_mint": "[53;32]",
      "token_program": "[54;32]"
    },
    "compiled_ordinal_zero": {
      "start_bucket": 100,
      "end_bucket_exclusive": 104,
      "primary_maturity_bucket_exclusive": 106,
      "absolute_recovery_attempts": [
        {"repair_generation": 10, "opens_at_bucket": 106, "closes_at_bucket": 108},
        {"repair_generation": 11, "opens_at_bucket": 108, "closes_at_bucket": 111}
      ],
      "market_collateral_cap": 1000
    }
  },"#;
    assert!(manifest.contains(FROZEN_FIXTURE_INPUTS));
    assert!(manifest.contains("\"schema\": \"dragons-clutch/product-series-golden/v1\""));
    assert!(manifest.contains("\"hash\": \"sha256(domain_ascii || exact_body)\""));
    assert!(manifest.contains("\"integer_encoding\": \"little-endian\""));
    assert!(manifest.contains("\"fixed_id_notation\": \"[N;32] means 32 repetitions of byte N\""));
    assert_eq!(manifest.matches("\"type\":").count(), 9);

    let compiled = compile_ordinal(
        &series(),
        &template(),
        &basis(),
        &recovery(),
        &genesis(),
        &attachment(),
        &registry(),
        0,
    )
    .unwrap();
    assert_manifest_vector(
        "NativeClaimBasisV1",
        BASIS_BYTES,
        NATIVE_CLAIM_BASIS_DOMAIN,
        basis().id().unwrap().bytes(),
    );
    assert_manifest_vector(
        "EvidenceOnlyRecoveryPolicyV1",
        EVIDENCE_ONLY_RECOVERY_POLICY_BYTES,
        RECOVERY_POLICY_DOMAIN,
        recovery().id().unwrap().bytes(),
    );
    assert_manifest_vector(
        "ProductTemplateV4",
        PRODUCT_TEMPLATE_BYTES,
        PRODUCT_TEMPLATE_DOMAIN,
        template().id().unwrap().bytes(),
    );
    assert_manifest_vector(
        "MarketGenesisProfileV1",
        MARKET_GENESIS_PROFILE_BYTES,
        MARKET_GENESIS_PROFILE_DOMAIN,
        genesis().id().unwrap().bytes(),
    );
    assert_manifest_vector(
        "MarketInstancePreimageV1",
        MARKET_INSTANCE_PREIMAGE_BYTES,
        MARKET_INSTANCE_DOMAIN,
        compiled.market_instance_id.bytes(),
    );
    assert_manifest_vector(
        "SeriesFundingQuoteV1",
        SERIES_FUNDING_QUOTE_BYTES,
        SERIES_FUNDING_QUOTE_DOMAIN,
        quote().id().unwrap().bytes(),
    );
    assert_manifest_vector(
        "SeriesAttachmentPlanV1",
        SERIES_ATTACHMENT_PLAN_BYTES,
        SERIES_ATTACHMENT_PLAN_DOMAIN,
        attachment().id().unwrap().bytes(),
    );
    assert_manifest_vector(
        "SeriesPlanV4",
        SERIES_PLAN_BYTES,
        SERIES_PLAN_DOMAIN,
        series().id().unwrap().bytes(),
    );
    assert_manifest_vector(
        "SeriesFundingTermsV1",
        SERIES_FUNDING_TERMS_BYTES,
        SERIES_FUNDING_TERMS_DOMAIN,
        funding_terms().id().unwrap().bytes(),
    );
}
