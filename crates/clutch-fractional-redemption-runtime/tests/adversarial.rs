use clutch_collateral_adapter_v2::{
    bind_claim_issuance_v1, bind_collateral_profile_v2, AdapterCatalogV2, AdapterReleaseV2,
    ClaimIssuanceBindingV1, ClaimRuntimeObservationV1, CollateralPolicyV2, Id,
    MarketCollateralBindingV2, ProfileCollateralBindingV2, RealmCollateralBindingV2,
    RuntimeReleaseObservationV2, CLAIM_FLAGS_V1, TOKEN_2022_PROGRAM,
};
use clutch_fractional_redemption_runtime::*;
use clutch_retirement::{DeletableRentOwnerV1, Identity32V1, RentSplitV2};

const COLLATERAL_DEPLOYMENT: Id = Id::from_bytes([2; 32]);
const COLLATERAL_CODE: Id = Id::from_bytes([3; 32]);
const COLLATERAL_RELEASE: AdapterReleaseV2 =
    AdapterReleaseV2::legacy_spl(COLLATERAL_DEPLOYMENT, COLLATERAL_CODE);
static COLLATERAL_RELEASES: [AdapterReleaseV2; 1] = [COLLATERAL_RELEASE];

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
    let policy = FractionalPolicyV1 {
        market_instance: rid(20),
        resolution_account: rid(21),
        payout_vector_id: vector.id().unwrap(),
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
    let ledger = FractionalLedgerV1 {
        aggregate_credit_numerator: aggregate_credit,
        active_credit_accounts: active_credits,
        ..initialize_fractional_ledger_v1(
            policy_account,
            policy,
            ledger_account,
            5,
            deletable_rent(43),
        )
        .unwrap()
    };
    let context = bind_fractional_context_v1(
        policy_account,
        policy,
        ledger_account,
        ledger,
        vector,
        collateral,
        claims,
    )
    .unwrap();
    (context, policy, ledger)
}

fn rid_from_collateral(value: Id) -> Identity32V1 {
    Identity32V1::new(value.bytes()).unwrap()
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
    let solvent = LiabilitySnapshotV1 {
        remaining_supply: supply,
        claim_backing_atoms: 1,
    };
    assert_eq!(solvent.validate(vector, 0), Ok(()));
    assert_eq!(solvent.validate(vector, 1), Err(Error::Insolvent));
}

#[test]
fn policy_ledger_credit_and_tombstone_codecs_refuse_hostile_bytes() {
    let (_context, policy, ledger) = external_context(0, 0);
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

    let credit = FractionalCreditV1 {
        policy_account: rid(41),
        ledger_account: rid(42),
        market_instance: policy.market_instance,
        resolution_account: policy.resolution_account,
        payout_vector_id: policy.payout_vector_id,
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
        payout_vector_id: credit.payout_vector_id,
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
fn arbitrary_bearer_burn_retains_the_exact_credit_and_conservation() {
    let (context, _policy, _ledger) = external_context(0, 0);
    let mut supply = [0; MAX_OUTCOMES];
    supply[0] = 1;
    supply[1] = 1;
    let before = LiabilitySnapshotV1 {
        remaining_supply: supply,
        claim_backing_atoms: 1,
    };
    let source = BearerClaimSourceV1 {
        claimant: rid(50),
        claim_token_account: rid(51),
        claim_mint: rid(52),
        collateral_destination: rid(53),
        claim_issuance_binding: context.policy().claim_issuance_binding,
        source_claim_atoms: 1,
    };
    let plan = redeem_bearer_to_credit_v1(
        context,
        before,
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
    assert_eq!(plan.ledger_after.aggregate_credit_numerator, 1);
    assert_eq!(plan.ledger_after.active_credit_accounts, 1);
    assert_eq!(plan.liability_after.claim_backing_atoms, 1);
    assert_eq!(plan.liability_after.remaining_supply[0], 0);
    assert_eq!(plan.liability_after.slack(payout(), 1), Ok(0));
}

#[test]
fn irreducible_terminal_credit_blocks_every_close_and_names_no_sweep_recipient() {
    let (context, _policy, _ledger) = external_context(1, 1);
    let empty_supply = LiabilitySnapshotV1 {
        remaining_supply: [0; MAX_OUTCOMES],
        claim_backing_atoms: 1,
    };
    let sealed = seal_claims_exhausted_v1(context, empty_supply, 1).unwrap();
    let terminal_context = context.with_ledger(sealed).unwrap();
    let facts = terminal_facts_v1(terminal_context, empty_supply).unwrap();
    assert_eq!(facts.aggregatable_credit_atoms, 0);
    assert_eq!(facts.irreducible_credit_numerator, 1);
    assert!(!facts.exactly_closable);
    assert_eq!(
        close_empty_ledger_v1(terminal_context, empty_supply, 2, 103, rid(60)),
        Err(Error::LiabilityOutstanding)
    );
}

#[test]
fn close_refuses_a_nonzero_credit_without_changing_the_aggregate_owner() {
    let (context, policy, ledger) = external_context(1, 1);
    let credit = FractionalCreditV1 {
        policy_account: context.policy_account(),
        ledger_account: context.ledger_account(),
        market_instance: policy.market_instance,
        resolution_account: policy.resolution_account,
        payout_vector_id: policy.payout_vector_id,
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
