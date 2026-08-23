use clutch_product_series::*;

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

fn attachment() -> SeriesAttachmentPlanV1 {
    SeriesAttachmentPlanV1 {
        funding_quote_id: id(40),
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
        BURN_REGISTRY_VALUE,
        0,
    )
    .unwrap();

    for changed_attachment in [
        SeriesAttachmentPlanV1 {
            funding_quote_id: id(60),
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
            BURN_REGISTRY_VALUE,
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
        BURN_REGISTRY_VALUE,
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
        BURN_REGISTRY_VALUE,
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
        BURN_REGISTRY_VALUE,
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
        BURN_REGISTRY_VALUE,
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
        BURN_REGISTRY_VALUE,
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
            BURN_REGISTRY_VALUE,
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
            BURN_REGISTRY_VALUE,
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
        BURN_REGISTRY_VALUE,
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
        .validate_bindings(&series(), &genesis(), id(53), id(54))
        .unwrap();
    assert_eq!(
        funding_terms().validate_bindings(&series(), &genesis(), id(99), id(54)),
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
            BURN_REGISTRY_VALUE,
            0,
        ),
        Err(Error::MismatchedArtifact)
    );
    assert_eq!(
        genesis().validate_bindings(&basis(), BURN_REGISTRY_VALUE + 1),
        Err(Error::MismatchedArtifact)
    );
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
    final_deadline_overflow.stride_buckets = 1;
    assert_eq!(
        final_deadline_overflow.validate_bindings(
            &template(),
            &basis(),
            &recovery(),
            &genesis(),
            &attachment(),
            BURN_REGISTRY_VALUE,
        ),
        Err(Error::ArithmeticOverflow)
    );
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

fn quote() -> AuthenticatedFundingQuoteV1 {
    AuthenticatedFundingQuoteV1 {
        funding_quote_id: attachment().funding_quote_id,
        market_core: ComponentDebitV1 {
            lamports: 10,
            collateral_atoms: 0,
        },
        recovery_reserve: ComponentDebitV1 {
            lamports: 20,
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
    }
}

#[test]
fn component_projection_debits_only_absent_exact_components() {
    let all_absent = FulfillmentStatusV1 {
        market_core: ComponentStatusV1::Absent,
        recovery_reserve: ComponentStatusV1::Absent,
        source_work: ComponentStatusV1::Absent,
        liquidity_facility: ComponentStatusV1::Absent,
        wrapper_set: ComponentStatusV1::Absent,
    };
    let projected = project_component_debits(
        &attachment(),
        &quote(),
        all_absent,
        FundingBalancesV1 {
            lamports: 200,
            collateral_atoms: 200,
        },
    )
    .unwrap();
    assert_eq!(projected.total.lamports, 150);
    assert_eq!(projected.total.collateral_atoms, 110);
    assert_eq!(projected.remaining.lamports, 50);
    assert_eq!(projected.remaining.collateral_atoms, 90);

    let reuse_core_and_wrapper = FulfillmentStatusV1 {
        market_core: ComponentStatusV1::PresentExact,
        recovery_reserve: ComponentStatusV1::PresentExact,
        source_work: ComponentStatusV1::Absent,
        liquidity_facility: ComponentStatusV1::Absent,
        wrapper_set: ComponentStatusV1::PresentExact,
    };
    let projected = project_component_debits(
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

    let incoherent = FulfillmentStatusV1 {
        recovery_reserve: ComponentStatusV1::Absent,
        ..reuse_core_and_wrapper
    };
    assert_eq!(
        project_component_debits(
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
    assert_eq!(
        project_component_debits(
            &attachment(),
            &hostile_quote,
            FulfillmentStatusV1 {
                market_core: ComponentStatusV1::Absent,
                recovery_reserve: ComponentStatusV1::Absent,
                source_work: ComponentStatusV1::Absent,
                liquidity_facility: ComponentStatusV1::Absent,
                wrapper_set: ComponentStatusV1::Absent,
            },
            FundingBalancesV1 {
                lamports: u64::MAX,
                collateral_atoms: u64::MAX,
            },
        ),
        Err(Error::ArithmeticOverflow)
    );
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
        BURN_REGISTRY_VALUE,
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
                202, 7, 24, 134, 16, 74, 151, 121, 31, 220, 71, 62, 129, 194, 139, 153, 188, 82,
                128, 222, 101, 62, 48, 197, 200, 85, 170, 187, 95, 238, 8, 7,
            ],
            [
                42, 68, 41, 225, 234, 59, 127, 93, 11, 116, 129, 239, 136, 23, 25, 71, 59, 223,
                178, 113, 93, 115, 209, 237, 236, 167, 27, 6, 83, 236, 227, 165,
            ],
            [
                128, 89, 63, 52, 126, 65, 203, 161, 25, 210, 68, 99, 252, 135, 38, 193, 136, 47,
                160, 54, 186, 151, 204, 173, 100, 189, 8, 132, 99, 80, 70, 0,
            ],
        ]
    );
}
