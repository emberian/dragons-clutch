use clutch_collateral_adapter_v2::{
    bind_claim_issuance_v1, bind_collateral_profile_v2, AdapterCatalogV2, AdapterReleaseV2,
    ClaimIssuanceBindingV1, ClaimLedgerV3, ClaimRuntimeObservationV1, CollateralPolicyV2, HoardV2,
    Id, MarketCollateralBindingV2, MarketLiabilityLifecycleV1, ProfileCollateralBindingV2,
    RealmCollateralBindingV2, ResolutionFinalizationFactsV5, ResolutionPayoutUnitBoundaryV5,
    ResolutionV5, RuntimeReleaseObservationV2, CLAIM_FLAGS_V1, TOKEN_2022_PROGRAM,
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

fn external_context(
    aggregate_credit: u128,
    active_credits: u64,
    internal_supply: [u64; MAX_OUTCOMES],
    materialized_supply: [u64; MAX_OUTCOMES],
    locked_claim_principal_atoms: u64,
) -> (
    BoundFractionalContextV1,
    FractionalPolicyV1,
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
    let policy = FractionalPolicyV1 {
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
        fractional_policy_id: cid(41),
        fractional_ledger_account: cid(42),
        resolution_account: cid(21),
        aggregate_internal_supply: internal_supply,
        aggregate_materialized_supply: materialized_supply,
        next_fractional_sequence: 0,
        last_fractional_transition_id: Id::ZERO,
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
fn policy_ledger_credit_and_tombstone_codecs_refuse_hostile_bytes() {
    let mut supply = [0; MAX_OUTCOMES];
    supply[0] = 1;
    supply[1] = 1;
    let (_context, policy, ledger) = external_context(0, 0, supply, [0; MAX_OUTCOMES], 1);
    let policy_bytes = policy.encode().unwrap();
    assert_eq!(FractionalPolicyV1::decode(&policy_bytes), Ok(policy));
    let mut hostile = policy_bytes;
    hostile[6] = 1;
    assert_eq!(
        FractionalPolicyV1::decode(&hostile),
        Err(Error::NonCanonicalPadding)
    );
    let ledger_bytes = ledger.encode().unwrap();
    assert_eq!(FractionalLedgerV1::decode(&ledger_bytes), Ok(ledger));
    let mut hostile_ledger = ledger_bytes;
    hostile_ledger[112] = 1;
    assert_eq!(
        FractionalLedgerV1::decode(&hostile_ledger),
        Err(Error::NonCanonicalPadding)
    );

    let credit = FractionalCreditV1 {
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
    assert_eq!(FractionalCreditV1::decode(&bytes), Ok(credit));
    let mut bad_padding = bytes;
    bad_padding[47] = 1;
    assert_eq!(
        FractionalCreditV1::decode(&bad_padding),
        Err(Error::NonCanonicalPadding)
    );
    let tombstone = FractionalCreditTombstoneV1 {
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
    assert_eq!(
        FractionalCreditTombstoneV1::decode(&tombstone.encode().unwrap()),
        Ok(tombstone)
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
            context.claims(),
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
fn close_refuses_a_nonzero_credit_without_changing_the_aggregate_owner() {
    let (context, policy, ledger) = external_context(1, 1, [0; MAX_OUTCOMES], [0; MAX_OUTCOMES], 1);
    let credit = FractionalCreditV1 {
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
