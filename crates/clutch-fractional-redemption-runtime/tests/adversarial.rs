use clutch_collateral_adapter_v2::{
    accept_fractional_bearer_claim_burn_v3, bind_claim_issuance_v1, bind_collateral_profile_v2,
    prepare_fractional_bearer_claim_burn_v3, AdapterBearerClaimObservationV3, AdapterCatalogV2,
    AdapterReleaseV2, ClaimIssuanceBindingV1, ClaimLedgerV3, ClaimRuntimeObservationV1,
    CollateralPolicyV2, FractionalBindingStateV1, HoardV2, Id, MarketCollateralBindingV2,
    MarketLiabilityLifecycleV1, ProfileCollateralBindingV2, RealmCollateralBindingV2,
    ResolutionFinalizationFactsV5, ResolutionPayoutUnitBoundaryV5, ResolutionV5,
    RuntimeReleaseObservationV2, CLAIM_FLAGS_V1, TOKEN_2022_PROGRAM,
};
use clutch_fractional_redemption_runtime::*;
use clutch_general_v2_contract::{
    project_general_position_replay_prestate_v1, GeneralPositionReplayPrestateV1,
    GeneralReplayExtensionV1, GeneralReplayTransitionKindV1, Id32, GENERAL_REPLAY_ACCOUNT_V1_BYTES,
    GENERAL_REPLAY_EXTENSION_SCHEMA_V1, GENERAL_REPLAY_EXTENSION_V1_BYTES,
};
use clutch_owner_settlement::AuthenticatedPositionV3;
use clutch_retirement::{
    DeletableRentOwnerV1, Identity32V1, PositionAccountV3, PositionLifecycleV3, PositionPurposeV3,
    PositionV3Fields, PositionV3Sha256Backend, RentSplitV2, ReplayV3Envelope,
    ReplayV3EnvelopeFields, ReplayV3EnvelopeHeader, ReplayV3ExtensionSchema, ReplayV3HashBackend,
};
use sha2::{Digest, Sha256};

const COLLATERAL_DEPLOYMENT: Id = Id::from_bytes([2; 32]);
const COLLATERAL_CODE: Id = Id::from_bytes([3; 32]);
const COLLATERAL_RELEASE: AdapterReleaseV2 =
    AdapterReleaseV2::legacy_spl(COLLATERAL_DEPLOYMENT, COLLATERAL_CODE);
static COLLATERAL_RELEASES: [AdapterReleaseV2; 1] = [COLLATERAL_RELEASE];

#[derive(Clone, Copy)]
struct TestSha256;

impl PositionV3Sha256Backend for TestSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update(body);
        hasher.finalize().into()
    }
}

impl ReplayV3HashBackend for TestSha256 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        for part in parts {
            hasher.update(part);
        }
        hasher.finalize().into()
    }
}

fn rid(byte: u8) -> Identity32V1 {
    Identity32V1::new([byte; 32]).unwrap()
}

fn cid(byte: u8) -> Id {
    Id::from_bytes([byte; 32])
}

fn payout() -> PayoutVectorV1 {
    let mut weights = [0; MAX_OUTCOMES];
    weights[0] = 1;
    weights[1] = 6;
    PayoutVectorV1 {
        outcome_count: 2,
        denominator: 7,
        weights,
    }
}

fn deletable_rent(payer: u8) -> DeletableRentOwnerV1 {
    DeletableRentOwnerV1::from_persisted(rid(payer), 100, 3).unwrap()
}

fn split_rent(payer: u8) -> RentSplitV2 {
    RentSplitV2 {
        payer: rid(payer),
        refundable_live_principal: 100,
        permanent_tombstone_principal: 40,
        donation_floor: 3,
    }
}

fn live_credit(
    context: BoundFractionalContextV1,
    claimant: Identity32V1,
    numerator: u64,
    stored_bump: u8,
) -> FractionalCreditV2 {
    let policy = context.policy();
    FractionalCreditV2 {
        policy_account: context.policy_account(),
        ledger_account: context.ledger_account(),
        market_instance: policy.market_instance,
        resolution_account: policy.resolution_account,
        resolution_data_id: policy.resolution_data_id,
        claimant,
        domain_generation: policy.domain_generation,
        account_generation: 1,
        next_sequence: 1,
        numerator,
        stored_bump,
        rent: split_rent(claimant.bytes()[0]),
    }
}

fn external_context(
    aggregate_credit: u128,
    active_credits: u64,
    internal_supply: [u64; MAX_OUTCOMES],
    materialized_supply: [u64; MAX_OUTCOMES],
    locked_claim_principal_atoms: u64,
) -> (
    BoundFractionalContextV1,
    FractionalPolicyV3,
    FractionalLedgerV1,
) {
    let catalog = AdapterCatalogV2::new(&COLLATERAL_RELEASES).unwrap();
    let collateral_policy = CollateralPolicyV2::for_release(
        COLLATERAL_RELEASE,
        cid(4),
        6,
        1_000_000,
        10_000,
        0,
        0,
        0,
        0,
    )
    .unwrap();
    let policy_id = collateral_policy.id().unwrap();
    let collateral = bind_collateral_profile_v2(
        MarketCollateralBindingV2 {
            market: cid(20),
            realm: cid(10),
            profile: cid(11),
            collateral_cap_atoms: 100,
            hoard_authority: cid(12),
            hoard_token_account: cid(13),
        },
        RealmCollateralBindingV2 {
            realm: cid(10),
            profile: cid(11),
        },
        ProfileCollateralBindingV2 {
            profile: cid(11),
            collateral_policy: policy_id,
            adapter_release: COLLATERAL_RELEASE.id().unwrap(),
        },
        collateral_policy,
        catalog,
        RuntimeReleaseObservationV2 {
            token_program: COLLATERAL_RELEASE.token_program,
            token_program_executable: true,
            token_program_writable: false,
            token_program_signer: false,
            token_program_deployment: COLLATERAL_DEPLOYMENT,
            parser_cpi_code: COLLATERAL_CODE,
        },
    )
    .unwrap();
    let claim_binding = ClaimIssuanceBindingV1 {
        flags: CLAIM_FLAGS_V1,
        adapter_release: cid(30),
        token_program: TOKEN_2022_PROGRAM,
        token_program_deployment: cid(31),
        parser_cpi_code: cid(32),
        decimals: 0,
        mint_extensions: 0,
        account_extensions: 0,
    };
    let claim_id = claim_binding.id().unwrap();
    let claims = bind_claim_issuance_v1(
        claim_id,
        claim_binding,
        ClaimRuntimeObservationV1 {
            token_program: TOKEN_2022_PROGRAM,
            token_program_executable: true,
            token_program_writable: false,
            token_program_signer: false,
            token_program_deployment: cid(31),
            parser_cpi_code: cid(32),
        },
        COLLATERAL_RELEASE,
    )
    .unwrap();
    let vector = payout();
    let resolution = ResolutionV5::finalized(
        ResolutionFinalizationFactsV5 {
            market_instance_id: cid(20),
            native_claim_basis_id: cid(33),
            finalization_evidence_id: cid(34),
            outcome_count: vector.outcome_count,
            payout_denominator: vector.denominator,
            payout_weights: vector.weights,
            generation: 7,
            payout_unit_boundary: ResolutionPayoutUnitBoundaryV5::ExactWholeCollateralAtoms,
        },
        8,
        deletable_rent(46),
    )
    .unwrap();
    let policy = FractionalPolicyV3 {
        market_instance: rid(20),
        resolution_account: rid(21),
        resolution_data_id: rid_from_collateral(resolution.data_id(cid(21)).unwrap()),
        realm: rid(10),
        collateral_policy: rid_from_collateral(policy_id),
        collateral_release: rid_from_collateral(COLLATERAL_RELEASE.id().unwrap()),
        claim_issuance_binding: rid_from_collateral(claim_id),
        domain_generation: 7,
        common_lot: vector.common_lot().unwrap(),
        outcome_count: 2,
        terminal_policy: TerminalRemainderPolicyV1::RetainUntilExactAggregation,
        stored_bump: 4,
        rent: deletable_rent(40),
    };
    let policy_account = rid(41);
    let ledger_account = rid(42);
    let claim_ledger_account = rid(44);
    let claim_ledger = ClaimLedgerV3 {
        market_instance_id: cid(20),
        realm_id: cid(10),
        native_claim_basis_id: cid(33),
        fractional_policy_id: Id::ZERO,
        fractional_ledger_account: Id::ZERO,
        resolution_account: cid(21),
        aggregate_internal_supply: internal_supply,
        aggregate_materialized_supply: materialized_supply,
        next_fractional_sequence: 0,
        last_fractional_transition_id: Id::ZERO,
        fractional_binding: FractionalBindingStateV1::OpenUnlatched,
        lifecycle: MarketLiabilityLifecycleV1::Resolved,
        outcome_count: 2,
        stored_bump: 6,
        rent: deletable_rent(44),
    };
    let hoard = HoardV2 {
        market_instance_id: cid(20),
        realm_id: cid(10),
        profile_id: cid(11),
        collateral_policy_id: policy_id,
        collateral_release_id: COLLATERAL_RELEASE.id().unwrap(),
        authority: cid(12),
        token_account: cid(13),
        collateral_cap_atoms: 100,
        cash_liability_atoms: 0,
        locked_claim_principal_atoms,
        lifecycle: MarketLiabilityLifecycleV1::Resolved,
        outcome_count: 2,
        stored_bump: 7,
        rent: deletable_rent(45),
    };
    let founding = initialize_fractional_ledger_v1(
        policy_account,
        policy,
        ledger_account,
        claim_ledger_account,
        claim_ledger,
        5,
        deletable_rent(43),
    )
    .unwrap();
    assert_eq!(
        founding
            .claim_ledger
            .claim_ledger_after()
            .fractional_binding,
        FractionalBindingStateV1::Latched
    );
    assert_eq!(
        founding.family_admission.market_instance(),
        policy.market_instance
    );
    assert_eq!(founding.family_admission.policy_account(), policy_account);
    assert_eq!(
        founding.family_admission.claim_issuance_binding(),
        policy.claim_issuance_binding
    );
    assert_eq!(founding.family_admission.ledger_account(), ledger_account);
    assert_eq!(
        founding.family_admission.claim_ledger_account(),
        claim_ledger_account
    );
    assert_ne!(founding.family_admission.receipt_id().bytes(), [0; 32]);
    let verified_admission = verify_fractional_family_admission_postwrite_v1(
        founding,
        policy_account,
        policy,
        ledger_account,
        founding.ledger_after,
        claim_ledger_account,
        founding.claim_ledger.claim_ledger_after(),
    )
    .unwrap();
    assert_eq!(
        verified_admission.family_admission(),
        founding.family_admission
    );
    assert_ne!(verified_admission.verification_id().bytes(), [0; 32]);
    let ledger = FractionalLedgerV1 {
        aggregate_credit_numerator: aggregate_credit,
        active_credit_accounts: active_credits,
        ..founding.ledger_after
    };
    let claim_ledger = founding.claim_ledger.claim_ledger_after();
    let context = bind_fractional_context_v1(
        policy_account,
        policy,
        ledger_account,
        ledger,
        claim_ledger_account,
        claim_ledger,
        hoard,
        resolution,
        collateral,
        claims,
    )
    .unwrap();
    (context, policy, ledger)
}

#[test]
fn fractional_family_admission_postwrite_refuses_substituted_physical_or_latch() {
    let mut supply = [0; MAX_OUTCOMES];
    supply[0] = 1;
    supply[1] = 1;
    let (context, policy, _ledger) =
        external_context(0, 0, supply, [0; MAX_OUTCOMES], 1);
    let open_claim_ledger = ClaimLedgerV3 {
        fractional_policy_id: Id::ZERO,
        fractional_ledger_account: Id::ZERO,
        next_fractional_sequence: 0,
        last_fractional_transition_id: Id::ZERO,
        fractional_binding: FractionalBindingStateV1::OpenUnlatched,
        ..context.claim_ledger()
    };
    let founding = initialize_fractional_ledger_v1(
        rid(41),
        policy,
        rid(42),
        rid(44),
        open_claim_ledger,
        5,
        deletable_rent(43),
    )
    .unwrap();
    assert_eq!(
        verify_fractional_family_admission_postwrite_v1(
            founding,
            rid(43),
            policy,
            rid(42),
            founding.ledger_after,
            rid(44),
            founding.claim_ledger.claim_ledger_after(),
        ),
        Err(Error::MismatchedBinding)
    );
    assert_eq!(
        verify_fractional_family_admission_postwrite_v1(
            founding,
            rid(41),
            policy,
            rid(42),
            founding.ledger_after,
            rid(44),
            open_claim_ledger,
        ),
        Err(Error::MismatchedBinding)
    );
}

fn rid_from_collateral(value: Id) -> Identity32V1 {
    Identity32V1::new(value.bytes()).unwrap()
}

fn canonical_internal_replay_fixture(
    context: BoundFractionalContextV1,
    claimant: Identity32V1,
    native_eggs: [u64; MAX_OUTCOMES],
    next_sequence: u64,
) -> (
    [u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES],
    AuthenticatedPositionV3,
    Id32,
    u8,
) {
    let position_account = rid(54);
    let replay_account = rid(55);
    let general_market_runtime = rid(56);
    let replay_bump = 9;
    let position = PositionAccountV3::new(PositionV3Fields {
        purpose: PositionPurposeV3::General,
        lifecycle: PositionLifecycleV3::Open,
        outcome_count: context.policy().outcome_count,
        stored_bump: 8,
        generation: 1,
        market_instance_id: context.policy().market_instance,
        realm_id: context.policy().realm,
        collateral_policy_id: context.policy().collateral_policy,
        collateral_release_id: context.policy().collateral_release,
        owner: claimant,
        controller: claimant,
        replay_account,
        purpose_binding_id: general_market_runtime,
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        native_eggs,
        outstanding_reservations: 0,
        rent: split_rent(50),
    })
    .unwrap();
    let position_semantic_id = position.semantic_id(&TestSha256).unwrap().bytes();
    let extension = GeneralReplayExtensionV1::initial(
        Id32::new(general_market_runtime.bytes()).unwrap(),
        Id32::new(position_semantic_id).unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap();
    let header = ReplayV3EnvelopeHeader::new_live(
        ReplayV3EnvelopeFields {
            position_account,
            replay_account,
            purpose: PositionPurposeV3::General,
            purpose_binding_id: general_market_runtime,
            position_generation: 1,
            next_sequence,
            stored_bump: replay_bump,
            rent: deletable_rent(50),
        },
        ReplayV3ExtensionSchema::new(GENERAL_REPLAY_EXTENSION_SCHEMA_V1).unwrap(),
        &extension,
        &TestSha256,
    )
    .unwrap();
    let envelope = ReplayV3Envelope::from_header(header, &extension, &TestSha256).unwrap();
    let mut body = [0; GENERAL_REPLAY_ACCOUNT_V1_BYTES];
    envelope.encode_into(&mut body, &TestSha256).unwrap();
    let authenticated = AuthenticatedPositionV3 {
        account: position_account.bytes(),
        general_market_runtime: general_market_runtime.bytes(),
        semantic: position,
        semantic_id: position_semantic_id,
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    };
    (
        body,
        authenticated,
        Id32::new(replay_account.bytes()).unwrap(),
        replay_bump,
    )
}

fn canonical_internal_source(
    context: BoundFractionalContextV1,
    claimant: Identity32V1,
    native_eggs: [u64; MAX_OUTCOMES],
    next_sequence: u64,
) -> InternalPositionV1 {
    let (body, position, replay_account, replay_bump) =
        canonical_internal_replay_fixture(context, claimant, native_eggs, next_sequence);
    let position_replay: GeneralPositionReplayPrestateV1 =
        project_general_position_replay_prestate_v1(
            replay_account,
            replay_bump,
            next_sequence,
            &body,
            position,
            &TestSha256,
        )
        .unwrap();
    InternalPositionV1 { position_replay }
}

fn dealer_vector_prestate(
    context: BoundFractionalContextV1,
    native_eggs: [u64; MAX_OUTCOMES],
    generation: u64,
    replay_ordinal: u64,
) -> Result<BoundDealerFacilityVectorPrestateV1> {
    let facility_id = rid(60);
    let state_account = rid(61);
    let position_account = rid(62);
    let replay_account = rid(63);
    let binding_id = rid(64);
    let position = PositionAccountV3::new(PositionV3Fields {
        purpose: PositionPurposeV3::DealerFacility,
        lifecycle: PositionLifecycleV3::Open,
        outcome_count: context.policy().outcome_count,
        stored_bump: 7,
        generation,
        market_instance_id: context.policy().market_instance,
        realm_id: context.policy().realm,
        collateral_policy_id: context.policy().collateral_policy,
        collateral_release_id: context.policy().collateral_release,
        owner: facility_id,
        controller: state_account,
        replay_account,
        purpose_binding_id: binding_id,
        cash_atoms: 4,
        reserved_cash_atoms: 0,
        native_eggs,
        outstanding_reservations: 0,
        rent: split_rent(60),
    })
    .unwrap();
    let position_id = position.semantic_id(&TestSha256).unwrap();
    bind_dealer_facility_vector_prestate_v1(
        facility_id,
        state_account,
        rid(65),
        position_account,
        position,
        position_id,
        binding_id,
        replay_account,
        rid(66),
        replay_ordinal,
        rid(67),
        rid(68),
        rid(69),
        rid(70),
        rid(71),
        rid(72),
        rid(73),
    )
}

#[test]
fn dealer_vector_divides_once_and_retains_the_exact_remainder() {
    let mut internal_supply = [0; MAX_OUTCOMES];
    internal_supply[0] = 7;
    internal_supply[1] = 7;
    let (full, policy, ledger) =
        external_context(0, 0, internal_supply, [0; MAX_OUTCOMES], 7);
    let context = bind_fractional_internal_context_v1(
        full.policy_account(),
        policy,
        full.ledger_account(),
        ledger,
        full.claim_ledger_account(),
        full.claim_ledger(),
        full.hoard(),
        full.resolution(),
        full.collateral(),
    )
    .unwrap();
    let prestate = dealer_vector_prestate(context, internal_supply, 9, 12).unwrap();
    let mut quantities = [0u64; MAX_OUTCOMES];
    quantities[0] = 1;
    quantities[1] = 1;
    let plan = prepare_dealer_facility_vector_transition_v1(
        context,
        DealerFacilityVectorRequestV1 {
            expected_ledger_sequence: 1,
            expected_credit_sequence: 1,
            expected_position_generation: 9,
            expected_replay_ordinal: 12,
            outcome_count: 2,
            quantities,
        },
        prestate,
        CreditCreationV1::Fresh {
            claimant: prestate.facility_id(),
            stored_bump: 11,
            rent: split_rent(60),
        },
    )
    .unwrap();
    assert_eq!(plan.payout_atoms(), 1);
    assert_eq!(plan.residue_numerator(), 0);
    assert_eq!(plan.credit_after().numerator, 0);
    assert_eq!(plan.ledger_after().aggregate_credit_numerator, 0);
    assert_eq!(plan.ledger_after().active_credit_accounts, 1);
    assert_eq!(plan.facility_position_after().cash_atoms(), 5);
    assert_eq!(plan.facility_position_after().native_eggs()[0], 6);
    assert_eq!(plan.facility_position_after().native_eggs()[1], 6);

    let mut remainder_quantities = [0u64; MAX_OUTCOMES];
    remainder_quantities[0] = 1;
    let remainder_plan = prepare_dealer_facility_vector_transition_v1(
        context,
        DealerFacilityVectorRequestV1 {
            expected_ledger_sequence: 1,
            expected_credit_sequence: 1,
            expected_position_generation: 9,
            expected_replay_ordinal: 12,
            outcome_count: 2,
            quantities: remainder_quantities,
        },
        prestate,
        CreditCreationV1::Fresh {
            claimant: prestate.facility_id(),
            stored_bump: 11,
            rent: split_rent(60),
        },
    )
    .unwrap();
    assert_eq!(remainder_plan.payout_atoms(), 0);
    assert_eq!(remainder_plan.residue_numerator(), 1);
    assert_eq!(remainder_plan.credit_after().numerator, 1);
    assert_eq!(remainder_plan.ledger_after().aggregate_credit_numerator, 1);
    assert_ne!(remainder_plan.vector_transition_id().bytes(), [0; 32]);
}

#[test]
fn dealer_vector_request_and_binding_refuse_hostile_tail_or_generation() {
    let mut quantities = [0u64; MAX_OUTCOMES];
    quantities[0] = 1;
    let request = DealerFacilityVectorRequestV1 {
        expected_ledger_sequence: 1,
        expected_credit_sequence: 1,
        expected_position_generation: 9,
        expected_replay_ordinal: 12,
        outcome_count: 2,
        quantities,
    };
    let encoded = request.encode().unwrap();
    assert_eq!(DealerFacilityVectorRequestV1::decode(&encoded), Ok(request));
    let mut hostile_padding = encoded;
    hostile_padding[33] = 1;
    assert_eq!(
        DealerFacilityVectorRequestV1::decode(&hostile_padding),
        Err(Error::NonCanonicalPadding)
    );
    let mut hostile_tail = encoded;
    hostile_tail[40 + 15 * 8..40 + 16 * 8].copy_from_slice(&1u64.to_le_bytes());
    assert_eq!(
        DealerFacilityVectorRequestV1::decode(&hostile_tail),
        Err(Error::NonCanonicalPadding)
    );
    let mut founding_replay = request;
    founding_replay.expected_replay_ordinal = 0;
    assert_eq!(founding_replay.encode(), Err(Error::MismatchedBinding));

    let mut internal_supply = [0; MAX_OUTCOMES];
    internal_supply[0] = 7;
    let (full, policy, ledger) =
        external_context(0, 0, internal_supply, [0; MAX_OUTCOMES], 1);
    let context = bind_fractional_internal_context_v1(
        full.policy_account(),
        policy,
        full.ledger_account(),
        ledger,
        full.claim_ledger_account(),
        full.claim_ledger(),
        full.hoard(),
        full.resolution(),
        full.collateral(),
    )
    .unwrap();
    assert_eq!(
        dealer_vector_prestate(context, internal_supply, 9, 0),
        Err(Error::MismatchedBinding)
    );
    let prestate = dealer_vector_prestate(context, internal_supply, 9, 12).unwrap();
    let mut wrong_generation = request;
    wrong_generation.expected_position_generation = 10;
    assert_eq!(
        prepare_dealer_facility_vector_transition_v1(
            context,
            wrong_generation,
            prestate,
            CreditCreationV1::Fresh {
                claimant: prestate.facility_id(),
                stored_bump: 11,
                rent: split_rent(60),
            },
        ),
        Err(Error::MismatchedBinding)
    );
}

#[test]
fn payout_lots_and_solvency_use_exact_integer_numerators() {
    let vector = payout();
    assert_eq!(vector.outcome_lot(0), Ok(7));
    assert_eq!(vector.outcome_lot(1), Ok(7));
    assert_eq!(vector.common_lot(), Ok(7));
    let mut supply = [0; MAX_OUTCOMES];
    supply[0] = 1;
    supply[1] = 1;
    assert_eq!(vector.validate_solvency(supply, 1, 0), Ok(()));
    assert_eq!(
        vector.validate_solvency(supply, 1, 1),
        Err(Error::Insolvent)
    );
}

#[test]
fn exact_internal_redemption_needs_no_bearer_claim_release() {
    let mut internal_supply = [0; MAX_OUTCOMES];
    internal_supply[0] = 7;
    internal_supply[1] = 1;
    let (full, policy, ledger) = external_context(0, 0, internal_supply, [0; MAX_OUTCOMES], 2);
    let internal = bind_fractional_internal_context_v1(
        full.policy_account(),
        policy,
        full.ledger_account(),
        ledger,
        full.claim_ledger_account(),
        full.claim_ledger(),
        full.hoard(),
        full.resolution(),
        full.collateral(),
    )
    .unwrap();
    assert_eq!(internal.claims(), None);
    let mut position_eggs = [0; MAX_OUTCOMES];
    position_eggs[0] = 7;
    let source = canonical_internal_source(internal, rid(50), position_eggs, 1);
    let plan = redeem_internal_exact_v1(internal, 1, 1, source, 0, 7).unwrap();
    assert_eq!(plan.paid_atoms, 1);
    assert_eq!(plan.claimant_numerator_after, 0);
    let RedemptionSourcePoststateV1::Internal(position) = plan.source_after else {
        panic!("wrong payout source");
    };
    assert_eq!(position.position_after.fields().cash_atoms, 1);
    assert_eq!(position.position_after.fields().native_eggs[0], 0);
    assert_eq!(
        position.replay.kind(),
        GeneralReplayTransitionKindV1::FractionalRedeemInternalExact
    );
}

#[test]
fn policy_ledger_credit_and_tombstone_codecs_refuse_hostile_bytes() {
    let mut supply = [0; MAX_OUTCOMES];
    supply[0] = 1;
    supply[1] = 1;
    let (_context, policy, ledger) = external_context(0, 0, supply, [0; MAX_OUTCOMES], 1);
    let policy_bytes = policy.encode().unwrap();
    assert_eq!(policy_bytes[1], 3);
    assert_eq!(FractionalPolicyV3::decode(&policy_bytes), Ok(policy));
    let mut withdrawn_policy = policy_bytes;
    withdrawn_policy[1] = 1;
    assert_eq!(
        FractionalPolicyV3::decode(&withdrawn_policy),
        Err(Error::WrongVersion)
    );
    withdrawn_policy[1] = 2;
    assert_eq!(
        FractionalPolicyV3::decode(&withdrawn_policy),
        Err(Error::WrongVersion)
    );
    let mut hostile = policy_bytes;
    hostile[6] = 1;
    assert_eq!(
        FractionalPolicyV3::decode(&hostile),
        Err(Error::NonCanonicalPadding)
    );
    let ledger_bytes = ledger.encode().unwrap();
    assert_eq!(ledger_bytes[1], 1);
    assert_eq!(FractionalLedgerV1::decode(&ledger_bytes), Ok(ledger));
    let mut hostile_ledger = ledger_bytes;
    hostile_ledger[112] = 1;
    assert_eq!(
        FractionalLedgerV1::decode(&hostile_ledger),
        Err(Error::NonCanonicalPadding)
    );

    let credit = FractionalCreditV2 {
        policy_account: rid(41),
        ledger_account: rid(42),
        market_instance: policy.market_instance,
        resolution_account: policy.resolution_account,
        resolution_data_id: policy.resolution_data_id,
        claimant: rid(50),
        domain_generation: 7,
        account_generation: 1,
        next_sequence: 9,
        numerator: 3,
        stored_bump: 7,
        rent: split_rent(50),
    };
    let bytes = credit.encode().unwrap();
    assert_eq!(bytes[1], 2);
    assert_eq!(FractionalCreditV2::decode(&bytes), Ok(credit));
    let mut withdrawn_credit = bytes;
    withdrawn_credit[1] = 1;
    assert_eq!(
        FractionalCreditV2::decode(&withdrawn_credit),
        Err(Error::WrongVersion)
    );
    let mut bad_padding = bytes;
    bad_padding[47] = 1;
    assert_eq!(
        FractionalCreditV2::decode(&bad_padding),
        Err(Error::NonCanonicalPadding)
    );
    let tombstone = FractionalCreditTombstoneV2 {
        policy_account: credit.policy_account,
        ledger_account: credit.ledger_account,
        market_instance: credit.market_instance,
        resolution_account: credit.resolution_account,
        resolution_data_id: credit.resolution_data_id,
        claimant: credit.claimant,
        domain_generation: credit.domain_generation,
        account_generation: credit.account_generation,
        closed_next_sequence: 10,
        stored_bump: credit.stored_bump,
        permanent_tombstone_principal: 40,
    };
    let tombstone_bytes = tombstone.encode().unwrap();
    assert_eq!(tombstone_bytes[1], 2);
    assert_eq!(
        FractionalCreditTombstoneV2::decode(&tombstone_bytes),
        Ok(tombstone)
    );
    let mut withdrawn_tombstone = tombstone_bytes;
    withdrawn_tombstone[1] = 1;
    assert_eq!(
        FractionalCreditTombstoneV2::decode(&withdrawn_tombstone),
        Err(Error::WrongVersion)
    );
    assert_eq!(
        policy.pda_seeds().prefix(),
        b"fractional-redemption-policy:v3"
    );
    let mut different_resolution_body = policy;
    different_resolution_body.resolution_data_id = rid(199);
    assert_eq!(
        different_resolution_body.pda_seeds(),
        policy.pda_seeds(),
        "Foundation address must not depend on the future Resolution body"
    );
    assert_ne!(
        different_resolution_body.state_id().unwrap(),
        policy.state_id().unwrap(),
        "the immutable body must still commit the exact Resolution data identity"
    );
    assert_eq!(
        credit.pda_seeds().prefix(),
        b"fractional-redemption-credit:v2"
    );
}

#[test]
fn exact_bearer_path_refuses_a_sub_lot_before_any_successor_exists() {
    let mut internal = [0; MAX_OUTCOMES];
    internal[1] = 1;
    let mut materialized = [0; MAX_OUTCOMES];
    materialized[0] = 1;
    let (context, _policy, ledger) = external_context(0, 0, internal, materialized, 1);
    let source = BearerClaimSourceV1 {
        claimant: rid(50),
        claim_token_account: rid(51),
        claim_mint: rid(52),
        collateral_destination: rid(53),
        claim_issuance_binding: context.policy().claim_issuance_binding,
        source_claim_atoms: 1,
        observed_materialized_supply: materialized,
        accepted_collateral: None,
    };
    assert_eq!(
        redeem_bearer_exact_v1(context, 1, source, 0, 1),
        Err(Error::NonIntegralLot)
    );
    assert_eq!(context.ledger(), ledger);
    assert_eq!(context.claim_ledger().aggregate_materialized_supply[0], 1);
    assert_eq!(context.hoard().locked_claim_principal_atoms, 1);
}

#[test]
fn exact_bearer_request_is_hidden_until_the_bound_claim_burn_is_accepted() {
    let mut internal = [0; MAX_OUTCOMES];
    internal[1] = 1;
    let mut stored_materialized = [0; MAX_OUTCOMES];
    stored_materialized[0] = 8;
    let (context, _policy, _ledger) = external_context(0, 0, internal, stored_materialized, 2);
    let mut materialized = stored_materialized;
    materialized[0] = 7;
    let source = BearerClaimPrestateV1 {
        claimant: rid(50),
        claim_token_account: rid(51),
        claim_mint: rid(52),
        collateral_destination: rid(53),
        claim_issuance_binding: context.policy().claim_issuance_binding,
        source_claim_atoms: 7,
        observed_materialized_supply: materialized,
    };
    let prepared = prepare_bearer_exact_v1(context, 1, source, 0, 7).unwrap();
    assert_eq!(
        prepared
            .fractional_claim_ledger()
            .claim_ledger_after()
            .aggregate_materialized_supply[0],
        0
    );
    let mut hostile_observed = materialized;
    hostile_observed[0] = 9;
    assert_eq!(
        prepare_bearer_exact_v1(
            context,
            1,
            BearerClaimPrestateV1 {
                observed_materialized_supply: hostile_observed,
                ..source
            },
            0,
            7,
        ),
        Err(Error::CollateralRefused)
    );
    let token_before = AdapterBearerClaimObservationV3 {
        mint: cid(52),
        mint_authority: cid(60),
        source_token_account: cid(51),
        source_owner: cid(50),
        mint_supply_atoms: 7,
        source_atoms: 7,
    };
    let prepared_burn = prepare_fractional_bearer_claim_burn_v3(
        context.claims().unwrap(),
        cid(60),
        cid(50),
        0,
        7,
        materialized,
        token_before,
        prepared.fractional_claim_ledger(),
    )
    .unwrap();
    let mut materialized_after = materialized;
    materialized_after[0] = 0;
    let mut wrong_outcome_after = materialized_after;
    wrong_outcome_after[1] = 1;
    assert!(accept_fractional_bearer_claim_burn_v3(
        prepared_burn,
        wrong_outcome_after,
        AdapterBearerClaimObservationV3 {
            mint_supply_atoms: 0,
            source_atoms: 0,
            ..token_before
        },
    )
    .is_err());
    let accepted_burn = accept_fractional_bearer_claim_burn_v3(
        prepared_burn,
        materialized_after,
        AdapterBearerClaimObservationV3 {
            mint_supply_atoms: 0,
            source_atoms: 0,
            ..token_before
        },
    )
    .unwrap();
    let burned = accept_bearer_exact_burn_v1(prepared, accepted_burn).unwrap();
    let request = burned.collateral_request();
    assert_eq!(
        request.claim_redemption_id,
        accepted_burn.fractional().transition_id()
    );
    assert_eq!(request.claim_semantic_owner, cid(50));
    assert_eq!(request.destination_token_account, cid(53));
    assert_eq!(request.payout_atoms, 1);
}

#[test]
fn credited_bearer_request_is_hidden_until_its_exact_burn_is_accepted() {
    let mut internal = [0; MAX_OUTCOMES];
    internal[1] = 1;
    let mut materialized = [0; MAX_OUTCOMES];
    materialized[0] = 1;
    let (context, _policy, _ledger) = external_context(0, 0, internal, materialized, 1);
    let source = BearerClaimPrestateV1 {
        claimant: rid(50),
        claim_token_account: rid(51),
        claim_mint: rid(52),
        collateral_destination: rid(53),
        claim_issuance_binding: context.policy().claim_issuance_binding,
        source_claim_atoms: 1,
        observed_materialized_supply: materialized,
    };
    let prepared = prepare_bearer_credit_v1(
        context,
        1,
        1,
        CreditPrestateV1::Create(CreditCreationV1::Fresh {
            claimant: rid(50),
            stored_bump: 9,
            rent: split_rent(50),
        }),
        source,
        0,
        1,
    )
    .unwrap();
    assert_eq!(prepared.claimant(), rid(50));
    assert_eq!(prepared.outcome(), 0);
    assert_eq!(prepared.quantity(), 1);
    assert_eq!(prepared.observed_materialized_supply(), materialized);

    let token_before = AdapterBearerClaimObservationV3 {
        mint: cid(52),
        mint_authority: cid(60),
        source_token_account: cid(51),
        source_owner: cid(50),
        mint_supply_atoms: 1,
        source_atoms: 1,
    };
    let prepared_burn = prepare_fractional_bearer_claim_burn_v3(
        context.claims().unwrap(),
        cid(60),
        cid(50),
        0,
        1,
        materialized,
        token_before,
        prepared.fractional_claim_ledger(),
    )
    .unwrap();
    let accepted_burn = accept_fractional_bearer_claim_burn_v3(
        prepared_burn,
        [0; MAX_OUTCOMES],
        AdapterBearerClaimObservationV3 {
            mint_supply_atoms: 0,
            source_atoms: 0,
            ..token_before
        },
    )
    .unwrap();
    let wrong_token_before = AdapterBearerClaimObservationV3 {
        source_owner: cid(61),
        ..token_before
    };
    let wrong_prepared_burn = prepare_fractional_bearer_claim_burn_v3(
        context.claims().unwrap(),
        cid(60),
        cid(61),
        0,
        1,
        materialized,
        wrong_token_before,
        prepared.fractional_claim_ledger(),
    )
    .unwrap();
    let wrong_burn = accept_fractional_bearer_claim_burn_v3(
        wrong_prepared_burn,
        [0; MAX_OUTCOMES],
        AdapterBearerClaimObservationV3 {
            mint_supply_atoms: 0,
            source_atoms: 0,
            ..wrong_token_before
        },
    )
    .unwrap();
    assert_eq!(
        accept_bearer_credit_burn_v1(prepared, wrong_burn),
        Err(Error::ClaimPlaneRefused)
    );
    let burned = accept_bearer_credit_burn_v1(prepared, accepted_burn).unwrap();
    let request = burned.collateral_request();
    assert_eq!(request.payout_atoms, 0);
    assert_eq!(request.claim_semantic_owner, cid(50));
    assert_eq!(request.destination_token_account, cid(53));
}

#[test]
fn arbitrary_bearer_burn_retains_the_exact_credit_and_conservation() {
    let mut internal = [0; MAX_OUTCOMES];
    internal[1] = 1;
    let mut materialized = [0; MAX_OUTCOMES];
    materialized[0] = 1;
    let (context, _policy, _ledger) = external_context(0, 0, internal, materialized, 1);
    assert!(context.resolution().payout_atoms(0, 1).is_err());
    let source = BearerClaimSourceV1 {
        claimant: rid(50),
        claim_token_account: rid(51),
        claim_mint: rid(52),
        collateral_destination: rid(53),
        claim_issuance_binding: context.policy().claim_issuance_binding,
        source_claim_atoms: 1,
        observed_materialized_supply: materialized,
        accepted_collateral: None,
    };
    let plan = redeem_bearer_to_credit_v1(
        context,
        1,
        1,
        CreditPrestateV1::Create(CreditCreationV1::Fresh {
            claimant: rid(50),
            stored_bump: 9,
            rent: split_rent(50),
        }),
        source,
        0,
        1,
    )
    .unwrap();
    assert_eq!(plan.paid_atoms, 0);
    assert_eq!(plan.claimant_numerator_after, 1);
    assert_eq!(plan.resolution_payout.resolution_account(), cid(21));
    assert_eq!(
        plan.resolution_payout.resolution_semantic_id().bytes(),
        context.resolution_semantic_id().bytes()
    );
    assert_eq!(
        plan.resolution_payout.resolution_data_id().bytes(),
        context.resolution_data_id().bytes()
    );
    assert_eq!(plan.resolution_payout.outcome(), 0);
    assert_eq!(plan.resolution_payout.quantity(), 1);
    assert_eq!(plan.resolution_payout.payout_weight(), 1);
    assert_eq!(plan.resolution_payout.denominator(), 7);
    assert_eq!(plan.resolution_payout.whole_atoms(), 0);
    assert_eq!(plan.resolution_payout.remainder_numerator(), 1);
    assert!(!plan.resolution_payout.is_exact());
    assert_eq!(plan.ledger_after.aggregate_credit_numerator, 1);
    assert_eq!(plan.ledger_after.active_credit_accounts, 1);
    assert_eq!(
        plan.custody_after
            .hoard_after()
            .locked_claim_principal_atoms,
        1
    );
    assert_eq!(
        plan.custody_after
            .fractional()
            .claim_ledger_after()
            .aggregate_materialized_supply[0],
        0
    );
    let mut after_supply = [0; MAX_OUTCOMES];
    after_supply[1] = 1;
    assert_eq!(payout().solvency_slack(after_supply, 1, 1), Ok(0));
    assert_ne!(
        plan.custody_after
            .fractional()
            .fractional_ledger_before_id(),
        plan.custody_after.fractional().fractional_ledger_after_id()
    );
}

#[test]
fn context_refuses_a_coherent_but_wrong_resolution_v5_body() {
    let mut supply = [0; MAX_OUTCOMES];
    supply[0] = 1;
    supply[1] = 1;
    let (context, policy, ledger) = external_context(0, 0, supply, [0; MAX_OUTCOMES], 1);
    let wrong_resolution = ResolutionV5 {
        facts: ResolutionFinalizationFactsV5 {
            finalization_evidence_id: cid(35),
            ..context.resolution().facts
        },
        ..context.resolution()
    };
    assert!(matches!(
        bind_fractional_context_v1(
            context.policy_account(),
            policy,
            context.ledger_account(),
            ledger,
            context.claim_ledger_account(),
            context.claim_ledger(),
            context.hoard(),
            wrong_resolution,
            context.collateral(),
            context.claims().unwrap(),
        ),
        Err(Error::MismatchedBinding)
    ));
}

#[test]
fn irreducible_terminal_credit_blocks_every_close_and_names_no_sweep_recipient() {
    let (context, _policy, _ledger) =
        external_context(1, 1, [0; MAX_OUTCOMES], [0; MAX_OUTCOMES], 1);
    let sealed = seal_claims_exhausted_v1(context, 1).unwrap();
    let terminal_context = context
        .with_ledgers(
            sealed.ledger_after,
            sealed.claim_ledger_after.claim_ledger_after(),
            context.hoard(),
        )
        .unwrap();
    let facts = terminal_facts_v1(terminal_context).unwrap();
    assert_eq!(facts.aggregatable_credit_atoms, 0);
    assert_eq!(facts.irreducible_credit_numerator, 1);
    assert!(!facts.exactly_closable);
    assert_eq!(
        close_empty_ledger_v1(terminal_context, 2, 103, 103, rid(60)),
        Err(Error::LiabilityOutstanding)
    );
}

#[test]
fn external_credit_payout_is_exposed_only_after_both_owner_credits_authenticate() {
    let (context, _policy, _ledger) =
        external_context(7, 2, [0; MAX_OUTCOMES], [0; MAX_OUTCOMES], 1);
    let source = live_credit(context, rid(50), 4, 9);
    let destination = live_credit(context, rid(51), 3, 10);
    assert_eq!(
        prepare_external_credit_transfer_v1(
            context,
            1,
            source,
            1,
            CreditPrestateV1::Live(destination),
            rid(52),
            1,
            4,
            rid(53),
        ),
        Err(Error::MismatchedBinding)
    );
    let prepared = prepare_external_credit_transfer_v1(
        context,
        1,
        source,
        1,
        CreditPrestateV1::Live(destination),
        rid(51),
        1,
        4,
        rid(53),
    )
    .unwrap();
    let request = prepared.collateral_request();
    assert_eq!(request.claim_semantic_owner, cid(51));
    assert_eq!(request.destination_token_account, cid(53));
    assert_eq!(request.payout_atoms, 1);
    assert_eq!(request.backing_before.locked_atoms, 1);

    let alternate = prepare_external_credit_transfer_v1(
        context,
        1,
        live_credit(context, rid(54), 5, 11),
        1,
        CreditPrestateV1::Live(live_credit(context, rid(51), 2, 10)),
        rid(51),
        1,
        5,
        rid(53),
    )
    .unwrap();
    let alternate_request = alternate.collateral_request();
    assert_eq!(alternate_request.destination_token_account, cid(53));
    assert_eq!(alternate_request.claim_semantic_owner, cid(51));
    assert_eq!(alternate_request.payout_atoms, 1);
    assert_eq!(alternate_request.backing_before, request.backing_before);
    assert_ne!(alternate.credit_transition_id(), prepared.credit_transition_id());
    assert_ne!(alternate_request.claim_redemption_id, request.claim_redemption_id);

    let wrong_destination = prepare_external_credit_transfer_v1(
        context,
        1,
        source,
        1,
        CreditPrestateV1::Live(destination),
        rid(51),
        1,
        4,
        rid(55),
    )
    .unwrap();
    assert_ne!(
        wrong_destination.credit_transition_id(),
        prepared.credit_transition_id()
    );
    assert_ne!(
        wrong_destination
            .collateral_request()
            .claim_redemption_id,
        request.claim_redemption_id
    );
}

#[test]
fn internal_credit_merge_reclassifies_principal_and_commits_the_gen1_kind() {
    let (context, _policy, _ledger) =
        external_context(7, 2, [0; MAX_OUTCOMES], [0; MAX_OUTCOMES], 1);
    let source = live_credit(context, rid(50), 4, 9);
    let destination = live_credit(context, rid(51), 3, 10);
    let position = canonical_internal_source(context, rid(51), [0; MAX_OUTCOMES], 1);
    let plan = merge_credit_v1(
        context,
        1,
        source,
        1,
        CreditPrestateV1::Live(destination),
        rid(51),
        1,
        CreditPayoutTargetV1::Internal {
            position,
            expected_replay_sequence: 1,
        },
    )
    .unwrap();
    assert_eq!(plan.paid_atoms, 1);
    assert_eq!(plan.source_after.numerator, 0);
    assert_eq!(plan.destination_after.numerator, 0);
    assert_eq!(plan.ledger_after.aggregate_credit_numerator, 0);
    assert_eq!(plan.custody_after.hoard_after().locked_claim_principal_atoms, 0);
    assert_eq!(plan.custody_after.hoard_after().cash_liability_atoms, 1);
    let CreditPayoutPoststateV1::Internal(internal) = plan.payout_after else {
        panic!("wrong payout disposition");
    };
    assert_eq!(internal.position_after.fields().cash_atoms, 1);
    assert_eq!(
        internal.replay.kind(),
        GeneralReplayTransitionKindV1::FractionalMergeCreditPayout
    );
}

#[test]
fn close_refuses_a_nonzero_credit_without_changing_the_aggregate_owner() {
    let (context, policy, ledger) = external_context(1, 1, [0; MAX_OUTCOMES], [0; MAX_OUTCOMES], 1);
    let credit = FractionalCreditV2 {
        policy_account: context.policy_account(),
        ledger_account: context.ledger_account(),
        market_instance: policy.market_instance,
        resolution_account: policy.resolution_account,
        resolution_data_id: policy.resolution_data_id,
        claimant: rid(50),
        domain_generation: 7,
        account_generation: 1,
        next_sequence: 1,
        numerator: 1,
        stored_bump: 9,
        rent: split_rent(50),
    };
    assert_eq!(
        close_zero_credit_v1(context, 1, credit, 1, 143, rid(60)),
        Err(Error::CreditOutstanding)
    );
    assert_eq!(context.ledger(), ledger);
}

#[test]
fn zero_credit_close_conserves_tombstone_refund_and_neutral_lamports() {
    let (context, _policy, _ledger) =
        external_context(0, 1, [0; MAX_OUTCOMES], [0; MAX_OUTCOMES], 0);
    let credit = live_credit(context, rid(50), 0, 9);
    let plan = close_zero_credit_v1(context, 1, credit, 1, 160, rid(60)).unwrap();
    assert_eq!(plan.ledger_after.active_credit_accounts, 0);
    assert_eq!(plan.ledger_after.aggregate_credit_numerator, 0);
    assert_eq!(plan.funding.payer, rid(50));
    assert_eq!(plan.funding.payer_refund_lamports, 100);
    assert_eq!(plan.funding.tombstone_lamports, 40);
    assert_eq!(plan.funding.neutral_lamports, 20);
    assert_eq!(
        plan.funding.payer_refund_lamports
            + plan.funding.tombstone_lamports
            + plan.funding.neutral_lamports,
        160
    );
    assert_eq!(plan.tombstone.closed_next_sequence, 2);
}

#[test]
fn credit_move_and_close_intents_refuse_noncanonical_wire_bytes() {
    let transfer = FractionalTransferIntentV1 {
        expected_ledger_sequence: 1,
        expected_source_sequence: 2,
        expected_destination_sequence: 3,
        expected_payout_replay_sequence: 4,
        numerator: 5,
        source_claimant: rid(50),
        destination_claimant: rid(51),
        source_credit: rid(52),
        destination_credit: rid(53),
        payout_target: rid(54),
        payout_kind: 1,
        destination_mode: 1,
    };
    let encoded = transfer.encode().unwrap();
    assert_eq!(FractionalTransferIntentV1::decode(&encoded), Ok(transfer));
    let mut bad_padding = encoded;
    bad_padding[202] = 1;
    assert_eq!(
        FractionalTransferIntentV1::decode(&bad_padding),
        Err(Error::NonCanonicalPadding)
    );
    let mut bad_kind = encoded;
    bad_kind[200] = 3;
    assert_eq!(
        FractionalTransferIntentV1::decode(&bad_kind),
        Err(Error::MismatchedBinding)
    );

    let close = FractionalCloseCreditIntentV1 {
        expected_ledger_sequence: 6,
        expected_credit_sequence: 7,
        claimant: rid(50),
        credit_account: rid(52),
    };
    let encoded = close.encode().unwrap();
    assert_eq!(FractionalCloseCreditIntentV1::decode(&encoded), Ok(close));
    assert_eq!(
        FractionalCloseCreditIntentV1::decode(&encoded[..79]),
        Err(Error::Truncated)
    );
}

#[test]
fn claim_ledger_and_fractional_sequences_cannot_advance_independently() {
    let mut supply = [0; MAX_OUTCOMES];
    supply[0] = 1;
    supply[1] = 1;
    let (context, _policy, _ledger) = external_context(0, 0, supply, [0; MAX_OUTCOMES], 1);
    let skewed_claim_ledger = ClaimLedgerV3 {
        next_fractional_sequence: context
            .claim_ledger()
            .next_fractional_sequence
            .checked_add(1)
            .unwrap(),
        ..context.claim_ledger()
    };
    assert!(matches!(
        context.with_ledgers(context.ledger(), skewed_claim_ledger, context.hoard()),
        Err(Error::MismatchedBinding)
    ));
}

#[test]
fn claims_exhausted_phase_cannot_hide_live_canonical_supply() {
    let mut supply = [0; MAX_OUTCOMES];
    supply[0] = 1;
    supply[1] = 1;
    let (context, _policy, _ledger) = external_context(0, 0, supply, [0; MAX_OUTCOMES], 1);
    let false_terminal = FractionalLedgerV1 {
        phase: FractionalLedgerPhaseV1::ClaimsExhausted,
        ..context.ledger()
    };
    assert_eq!(
        context.with_ledgers(false_terminal, context.claim_ledger(), context.hoard()),
        Err(Error::LiabilityOutstanding)
    );
}

#[test]
fn exact_terminal_retirement_splits_only_policy_and_ledger_rent() {
    let (context, _policy, _ledger) =
        external_context(0, 0, [0; MAX_OUTCOMES], [0; MAX_OUTCOMES], 0);
    let sealed = seal_claims_exhausted_v1(context, 1).unwrap();
    let terminal_context = context
        .with_ledgers(
            sealed.ledger_after,
            sealed.claim_ledger_after.claim_ledger_after(),
            context.hoard(),
        )
        .unwrap();
    let close = close_empty_ledger_v1(terminal_context, 2, 103, 103, rid(60)).unwrap();
    assert_eq!(
        close.claim_ledger_after().claim_ledger_after().lifecycle,
        MarketLiabilityLifecycleV1::Retiring
    );
    assert_eq!(close.policy_funding().account(), rid(41));
    assert_eq!(close.policy_funding().payer(), rid(40));
    assert_eq!(close.policy_funding().payer_refund_lamports(), 100);
    assert_eq!(close.policy_funding().neutral_sink(), rid(60));
    assert_eq!(close.policy_funding().neutral_lamports(), 3);
    assert_eq!(close.ledger_funding().account(), rid(42));
    assert_eq!(close.ledger_funding().payer(), rid(43));
    assert_eq!(close.ledger_funding().payer_refund_lamports(), 100);
    assert_eq!(close.ledger_funding().neutral_sink(), rid(60));
    assert_eq!(close.ledger_funding().neutral_lamports(), 3);
    assert_eq!(close.terminal_requirement().market_instance_id(), rid(20));
    assert_eq!(close.terminal_requirement().domain_generation(), 7);
    assert_eq!(close.terminal_requirement().resolution_account(), rid(21));
    assert_eq!(
        close.terminal_requirement().resolution_semantic_id(),
        terminal_context.resolution_semantic_id()
    );
    assert_eq!(
        close.terminal_requirement().resolution_data_id(),
        terminal_context.resolution_data_id()
    );
    assert_eq!(
        close.terminal_requirement().native_claim_basis_id().bytes(),
        terminal_context
            .claim_ledger()
            .native_claim_basis_id
            .bytes()
    );
    assert_eq!(close.terminal_requirement().policy_account(), rid(41));
    assert_eq!(close.terminal_requirement().ledger_account(), rid(42));
    assert_eq!(
        close.terminal_requirement().policy_terminal_state_id(),
        terminal_context.policy().state_id().unwrap()
    );
    assert_eq!(
        close
            .terminal_requirement()
            .ledger_before_state_id()
            .bytes(),
        close
            .claim_ledger_after()
            .fractional_ledger_before_id()
            .bytes()
    );
    assert_eq!(
        close
            .terminal_requirement()
            .ledger_terminal_state_id()
            .bytes(),
        close
            .claim_ledger_after()
            .fractional_ledger_retirement_id()
            .bytes()
    );
    assert_eq!(
        close
            .terminal_requirement()
            .claim_ledger_post_state_id()
            .bytes(),
        close.claim_ledger_after().claim_ledger_after_id().bytes()
    );
    assert_eq!(
        close
            .terminal_requirement()
            .claim_ledger_transition_id()
            .bytes(),
        close.claim_ledger_after().transition_id().bytes()
    );
    assert_ne!(
        close.claim_ledger_after().fractional_ledger_before_id(),
        close.claim_ledger_after().fractional_ledger_retirement_id()
    );
    let terminal = project_fractional_family_terminal_receipt_v1(close, rid(70)).unwrap();
    assert_eq!(terminal.market_instance_id(), rid(20));
    assert_eq!(terminal.domain_generation(), 7);
    assert_eq!(terminal.policy_account(), rid(41));
    assert_eq!(terminal.ledger_account(), rid(42));
    assert_eq!(terminal.claim_ledger_account(), rid(44));
    assert_eq!(terminal.fractional_release_id(), rid(70));
    assert_eq!(
        terminal.policy_terminal_state_id(),
        close.terminal_requirement().policy_terminal_state_id()
    );
    assert_eq!(
        terminal.ledger_terminal_state_id(),
        close.terminal_requirement().ledger_terminal_state_id()
    );
    assert_eq!(
        terminal.claim_ledger_post_state_id(),
        close.terminal_requirement().claim_ledger_post_state_id()
    );
    assert_eq!(
        terminal.claim_ledger_transition_id(),
        close.terminal_requirement().claim_ledger_transition_id()
    );
    assert_ne!(terminal.rent_disposition_id().bytes(), [0; 32]);
    assert_ne!(terminal.receipt_id().bytes(), [0; 32]);
    let verified_terminal = verify_fractional_family_terminal_postwrite_v1(
        close,
        terminal,
        rid(41),
        terminal_context.policy(),
        rid(42),
        terminal_context.ledger(),
        rid(44),
        close.claim_ledger_after().claim_ledger_after(),
        103,
        103,
    )
    .unwrap();
    assert_eq!(verified_terminal.family_terminal(), terminal);
    assert_ne!(verified_terminal.verification_id().bytes(), [0; 32]);
    assert_eq!(
        verify_fractional_family_terminal_postwrite_v1(
            close,
            terminal,
            rid(41),
            terminal_context.policy(),
            rid(42),
            terminal_context.ledger(),
            rid(44),
            close.claim_ledger_after().claim_ledger_after(),
            104,
            103,
        ),
        Err(Error::RentRefused)
    );
    assert_eq!(
        verify_fractional_family_terminal_postwrite_v1(
            close,
            terminal,
            rid(41),
            terminal_context.policy(),
            rid(42),
            terminal_context.ledger(),
            rid(44),
            terminal_context.claim_ledger(),
            103,
            103,
        ),
        Err(Error::MismatchedBinding)
    );
    assert_ne!(
        terminal.receipt_id(),
        project_fractional_family_terminal_receipt_v1(close, rid(71))
            .unwrap()
            .receipt_id()
    );
}

#[test]
fn terminal_close_admits_each_rent_owner_independently() {
    let (context, _policy, _ledger) =
        external_context(0, 0, [0; MAX_OUTCOMES], [0; MAX_OUTCOMES], 0);
    let sealed = seal_claims_exhausted_v1(context, 1).unwrap();
    let terminal_context = context
        .with_ledgers(
            sealed.ledger_after,
            sealed.claim_ledger_after.claim_ledger_after(),
            context.hoard(),
        )
        .unwrap();
    assert_eq!(
        close_empty_ledger_v1(terminal_context, 2, 102, 103, rid(60)),
        Err(Error::RentRefused)
    );
    assert_eq!(
        close_empty_ledger_v1(terminal_context, 2, 103, 102, rid(60)),
        Err(Error::RentRefused)
    );
    assert_eq!(
        close_empty_ledger_v1(terminal_context, 2, 103, 103, rid(40)),
        Err(Error::RentRefused)
    );
    assert_eq!(
        close_empty_ledger_v1(terminal_context, 2, 103, 103, rid(43)),
        Err(Error::RentRefused)
    );
}

#[test]
fn internal_credit_redemption_advances_the_canonical_gen1_replay() {
    let mut internal = [0; MAX_OUTCOMES];
    internal[0] = 1;
    internal[1] = 1;
    let (context, _policy, _ledger) = external_context(0, 0, internal, [0; MAX_OUTCOMES], 1);
    let mut position_eggs = [0; MAX_OUTCOMES];
    position_eggs[0] = 1;
    let source = canonical_internal_source(context, rid(50), position_eggs, 1);
    let plan = redeem_internal_to_credit_v1(
        context,
        1,
        1,
        1,
        CreditPrestateV1::Create(CreditCreationV1::Fresh {
            claimant: rid(50),
            stored_bump: 9,
            rent: split_rent(50),
        }),
        source,
        0,
        1,
    )
    .unwrap();
    let internal = match plan.source_after {
        RedemptionSourcePoststateV1::Internal(value) => value,
        RedemptionSourcePoststateV1::Bearer(_) => panic!("wrong payout source"),
    };
    assert_eq!(plan.paid_atoms, 0);
    assert_eq!(plan.claimant_numerator_after, 1);
    assert_eq!(internal.position_after.fields().native_eggs[0], 0);
    assert_eq!(internal.position_after.fields().cash_atoms, 0);
    assert_eq!(
        internal.replay.kind(),
        GeneralReplayTransitionKindV1::FractionalRedeemInternalCredit
    );
    assert_eq!(internal.replay.consumed_sequence(), 1);
    assert_eq!(internal.replay.next_sequence(), 2);
    assert_eq!(
        internal.replay.transition_id().bytes(),
        plan.custody_after.fractional().transition_id().bytes()
    );
    assert_eq!(
        internal.replay.transition_evidence_id().bytes(),
        plan.custody_after.receipt_id().bytes()
    );
}

#[test]
fn canonical_gen1_parser_refuses_unallocated_fractional_coordinates() {
    let general_runtime = Id32::new([1; 32]).unwrap();
    let position_id = Id32::new([2; 32]).unwrap();
    let base = GeneralReplayExtensionV1::initial(general_runtime, position_id)
        .unwrap()
        .encode()
        .unwrap();
    for (kind, action) in [
        (
            GeneralReplayTransitionKindV1::FractionalRedeemInternalExact,
            2,
        ),
        (
            GeneralReplayTransitionKindV1::FractionalRedeemInternalCredit,
            4,
        ),
        (
            GeneralReplayTransitionKindV1::FractionalTransferCreditPayout,
            6,
        ),
        (
            GeneralReplayTransitionKindV1::FractionalMergeCreditPayout,
            7,
        ),
    ] {
        let mut extension = base;
        extension[64..96].copy_from_slice(&[3; 32]);
        extension[96..128].copy_from_slice(&[4; 32]);
        extension[128] = action;
        extension[129] = 1;
        extension[130] = 4;
        extension[131] = 1;
        extension[132] = 1;
        let decoded = GeneralReplayExtensionV1::decode(&extension).unwrap();
        assert_eq!(decoded.last_kind(), Some(kind));

        extension[130] = 5;
        assert!(GeneralReplayExtensionV1::decode(&extension).is_err());
    }

    let mut hostile = base;
    assert_eq!(hostile.len(), GENERAL_REPLAY_EXTENSION_V1_BYTES);
    hostile[133] = 1;
    assert!(GeneralReplayExtensionV1::decode(&hostile).is_err());
}

#[test]
fn disabled_adapter_refuses_before_payload_or_account_inspection() {
    let malformed = [79, 1, 255, 0xff];
    let hostile_accounts = [SolanaAccountMetaProjectionV1 {
        key: [0; 32],
        writable: true,
        signer: true,
    }];
    assert_eq!(
        refuse_disabled_fractional_redemption_v1(&malformed, &hostile_accounts),
        Err(Error::CapabilityDisabled)
    );
}

#[test]
fn live_successor_account_contracts_name_dynamic_bearer_mints_and_terminal_writes() {
    let initialize = fractional_account_contract_v1(FractionalRedemptionActionV1::Initialize);
    assert_eq!(initialize.account_count, 31);
    assert_eq!(initialize.foundation_core_accounts, 14);
    assert_eq!(initialize.foundation_aux_accounts, 17);
    assert!(initialize.foundation_outcome_pair_suffix);
    assert_eq!(initialize.foundation_aux_writable_mask, 0);
    assert_eq!(initialize.writable_mask, 0x1811);
    assert_eq!(initialize.signer_mask, 0);

    let bearer = fractional_account_contract_v1(FractionalRedemptionActionV1::RedeemBearerExact);
    assert_eq!(bearer.account_count, 21);
    assert_eq!(bearer.writable_mask, 0x95300);
    assert_eq!(bearer.writable_mask & (1 << 18), 0);
    assert_ne!(bearer.writable_mask & (1 << 19), 0);
    assert_eq!(bearer.writable_mask & (1 << 20), 0);
    assert_eq!(bearer.signer_mask, 1);
    assert!(bearer.outcome_mint_suffix);
    assert_eq!(bearer.post_mint_accounts, 0);
    assert!(!bearer.credit_creation_suffix);
    assert_eq!(bearer.external_payout_extra_accounts, 0);

    let internal_credit =
        fractional_account_contract_v1(FractionalRedemptionActionV1::RedeemInternalCredit);
    assert_eq!(internal_credit.account_count, 19);
    assert_eq!(internal_credit.signer_mask, 1);
    assert!(!internal_credit.outcome_mint_suffix);
    assert_eq!(internal_credit.post_mint_accounts, 0);
    assert!(internal_credit.credit_creation_suffix);
    assert_eq!(internal_credit.external_payout_extra_accounts, 0);

    let bearer_credit =
        fractional_account_contract_v1(FractionalRedemptionActionV1::RedeemBearerCredit);
    assert_eq!(bearer_credit.account_count, 21);
    assert_eq!(bearer_credit.writable_mask & (1 << 18), 0);
    assert_ne!(bearer_credit.writable_mask & (1 << 19), 0);
    assert_eq!(bearer_credit.writable_mask & (1 << 20), 0);
    assert_eq!(bearer_credit.signer_mask, 1);
    assert!(bearer_credit.outcome_mint_suffix);
    assert_eq!(bearer_credit.post_mint_accounts, 4);
    assert!(bearer_credit.credit_creation_suffix);
    assert_eq!(bearer_credit.external_payout_extra_accounts, 0);

    let transfer =
        fractional_account_contract_v1(FractionalRedemptionActionV1::TransferCredit);
    assert_eq!(transfer.account_count, 21);
    assert_eq!(transfer.signer_mask, 0b11);
    assert!(transfer.credit_creation_suffix);
    assert_eq!(transfer.external_payout_extra_accounts, 3);
    assert_ne!(transfer.writable_mask, transfer.external_writable_mask);
    assert_eq!(transfer.external_writable_mask & (1 << 16), 0);
    assert_ne!(transfer.external_writable_mask & (1 << 19), 0);
    assert_eq!(transfer.external_writable_mask & (1 << 20), 0);

    let close =
        fractional_account_contract_v1(FractionalRedemptionActionV1::CloseZeroCredit);
    assert_eq!(close.account_count, 18);
    assert_eq!(close.signer_mask, 1);
    assert!(!close.credit_creation_suffix);
    assert_eq!(close.external_payout_extra_accounts, 0);
    assert_eq!(close.external_writable_mask, close.writable_mask);

    let seal = fractional_account_contract_v1(FractionalRedemptionActionV1::SealClaimsExhausted);
    assert_eq!(seal.account_count, 12);
    assert_eq!(seal.writable_mask, 0x900);
    assert_eq!(seal.signer_mask, 0);
    assert!(!seal.outcome_mint_suffix);
    assert_eq!(seal.post_mint_accounts, 0);
    assert!(!seal.credit_creation_suffix);
    assert_eq!(seal.external_payout_extra_accounts, 0);

    let terminal =
        fractional_account_contract_v1(FractionalRedemptionActionV1::CloseEmptyLedger);
    assert_eq!(terminal.account_count, 31);
    assert_eq!(terminal.foundation_core_accounts, 14);
    assert_eq!(terminal.foundation_aux_accounts, 17);
    assert!(terminal.foundation_outcome_pair_suffix);
    assert_eq!(terminal.foundation_aux_writable_mask, 0x18000);
    assert_eq!(terminal.writable_mask, 0x1811);
    assert_eq!(terminal.signer_mask, 0);
}
