use std::{env, path::PathBuf};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityFundingDerivationV1,
    CapabilityManifestV1, CompartmentFundingV1, FundingAmountsV1, FundingQuoteV1,
};
use dclutch_collateral_contract::{FOUND_MARKET_AND_FUND_BYTES, FoundMarketAndFundV1};
use dclutch_core_contract::{ContentId as CoreContentId, MarketIdentity, Phase};
use dclutch_kernel::resolution::categorical_pyth_v1::{
    CategoricalPythV1PolicyInput, MAX_PRICE_CELLS,
};
use dclutch_market_contract::market::CategoricalMarketV1;
use dclutch_product_contract::{
    ContentId as ProductContentId,
    capacity::{CapacityEnvelope, CapacityProfileId, CapacityProfileV1, CapacityProfileV1Input},
    claim::{CategoricalUnitV1, CategoricalUnitV1Input},
    product::{InstanceV1, InstanceV1Input},
};
use dclutch_pyth_contract::{
    feed_profile::PythFeedProfileV1,
    funding::{FUNDING_BYTES, FundingStateV1, construct_required_resolution_funding},
    policy::CategoricalPythPolicyRecordV1,
    resolution_material::CategoricalPythResolutionMaterialV1,
};
use dclutch_realm_contract::{FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1, RealmV1Input};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_rent_contract::{
    CreateRentCreditV1, RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority,
    RentCreditV1,
};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{system_program, sysvar};
use solana_transaction::Transaction;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([71; 32]);
const GENERATION: u64 = 41;
const PROVIDER_FEE: u64 = 71;
const SUCCESS_BOUNTY: u64 = 73;
const SPONSOR_CUSHION: u64 = 1_000_000;
const MARKET_SEED: &[u8] = b"dclutch/market-root/v1";
const REALM_SCHEMA_LABEL: &[u8] = b"dclutch/schema/realm-v1";
const INSTANCE_SCHEMA_LABEL: &[u8] = b"dclutch/schema/product-instance-v1";
const CLAIM_SCHEMA_LABEL: &[u8] = b"dclutch/schema/categorical-unit-claim-v1";
const CAPACITY_SCHEMA_LABEL: &[u8] = b"dclutch/schema/product-capacity-profile-v1";
const MATERIAL_SCHEMA_LABEL: &[u8] = b"dclutch/schema/categorical-pyth-resolution-material-v1";
const MANIFEST_SCHEMA_LABEL: &[u8] = b"dclutch/schema/capability-manifest-profile-1-v1";

#[derive(Clone, Copy)]
enum Fault {
    None,
    WrongProductClaimLink,
    WrongClaimCapacityLink,
    WrongInstanceCapacityLink,
    WrongMarketPda,
    UnderfundedSponsor,
    PreexistingMarket,
    AliasedDestinations,
    MissingResolutionManifest,
    AmbiguousResolutionManifest,
    WrongResolutionQuote,
}

struct Fixture {
    test: ProgramTest,
    sponsor: Keypair,
    sponsor_before: u64,
    instruction: FoundMarketAndFundV1,
    realm: Pubkey,
    instance: Pubkey,
    claim: Pubkey,
    capacity: Pubkey,
    material: Pubkey,
    manifest: Pubkey,
    realm_cursor: Pubkey,
    instance_cursor: Pubkey,
    claim_cursor: Pubkey,
    capacity_cursor: Pubkey,
    material_cursor: Pubkey,
    manifest_cursor: Pubkey,
    rent_credit: Pubkey,
    rent_credit_state: RentCreditV1,
    submitted_market: Pubkey,
    submitted_fund: Pubkey,
    canonical_market: Pubkey,
    canonical_fund: Pubkey,
    preexisting_market: Option<Account>,
}

fn require_sbf_out_dir() {
    let directory = env::var("SBF_OUT_DIR").expect(
        "SBF_OUT_DIR is required; build target/deploy/dclutch_sbf.so first, then run `SBF_OUT_DIR=../../target/deploy cargo test --test found_market_and_fund`",
    );
    let artifact = PathBuf::from(directory).join("dclutch_sbf.so");
    assert!(
        artifact.is_file(),
        "SBF_OUT_DIR must contain the exact compiled dclutch_sbf.so artifact: {}",
        artifact.display()
    );
}

fn product_id(bytes: [u8; 32]) -> ProductContentId {
    ProductContentId::new(bytes).expect("nonzero deterministic product identity")
}

fn core_id(bytes: [u8; 32]) -> CoreContentId {
    CoreContentId::new(bytes).expect("nonzero deterministic core identity")
}

fn account(data: Vec<u8>) -> Account {
    Account {
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn record_addresses(schema_label: &[u8], exact_content: &[u8]) -> (Pubkey, Pubkey) {
    let schema = hash(schema_label).to_bytes();
    let digest = hash(exact_content).to_bytes();
    let (raw, _) = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
        &PROGRAM_ID,
    );
    let (cursor, _) = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema.as_slice(),
            digest.as_slice(),
        ],
        &PROGRAM_ID,
    );
    (raw, cursor)
}

fn vacant_cursor() -> Account {
    Account::new(0, 0, &system_program::ID)
}

fn resolution_entry(kind: u8, policy_id: CoreContentId, quoted_rent: u64) -> CapabilityEntryV1 {
    CapabilityEntryV1::new(
        core_id([kind; 32]),
        core_id([14; 32]),
        policy_id,
        core_id([15; 32]),
        core_id([16; 32]),
        core_id([17; 32]),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; 16],
        FundingQuoteV1::new(
            FundingAmountsV1::new(
                CompartmentFundingV1::native_lamports(quoted_rent)
                    .expect("Fund state rent is native lamports"),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::native_lamports(PROVIDER_FEE)
                    .expect("provider fee is native lamports"),
                CompartmentFundingV1::native_lamports(SUCCESS_BOUNTY)
                    .expect("success bounty is native lamports"),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            )
            .expect("exact typed resolution funding"),
            None,
        )
        .expect("exact native-only resolution Fund quote"),
    )
    .expect("canonical required resolution capability")
}

fn manifest_bytes(entries: &[CapabilityEntryV1]) -> Vec<u8> {
    let length = 16usize
        .checked_add(
            entries
                .len()
                .checked_mul(CAPABILITY_ENTRY_BYTES)
                .expect("bounded manifest entry width"),
        )
        .expect("bounded manifest length");
    let mut bytes = vec![0; length];
    CapabilityManifestV1::encode_into(entries, &mut bytes).expect("canonical manifest encoding");
    bytes
}

fn fixture(fault: Fault) -> Fixture {
    require_sbf_out_dir();
    let sponsor = Keypair::new();
    let realm_value = RealmV1::new(RealmV1Input {
        token_program: [2; 32],
        collateral_mint: [3; 32],
        collateral_adapter_release_id: [4; 32],
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("canonical Realm");
    let realm_bytes = realm_value.to_bytes();
    let realm_digest = hash(&realm_bytes).to_bytes();
    let (realm, realm_cursor) = record_addresses(REALM_SCHEMA_LABEL, &realm_bytes);

    let capacity_value = CapacityProfileV1::new(CapacityProfileV1Input {
        envelope: CapacityEnvelope::Measured,
        verifier_release_id: product_id([5; 32]),
        envelope_basis_id: product_id([6; 32]),
        max_artifact_bytes: 16,
        page_payload_bytes: 16,
        max_pages: 1,
        max_partition_cells: 2,
    })
    .expect("canonical capacity profile");
    let capacity_bytes = capacity_value.to_bytes();
    let capacity_digest = hash(&capacity_bytes).to_bytes();
    let capacity_id = CapacityProfileId::new(product_id(capacity_digest));

    let alternate_capacity_id = CapacityProfileId::new(product_id([7; 32]));
    let claim_capacity_id = if matches!(fault, Fault::WrongClaimCapacityLink) {
        alternate_capacity_id
    } else {
        capacity_id
    };
    let claim_value = CategoricalUnitV1::new(
        CategoricalUnitV1Input {
            capacity_profile_id: claim_capacity_id,
            outcome_count: 2,
        },
        capacity_value,
    )
    .expect("canonical categorical claim basis");
    let claim_bytes = claim_value.to_bytes();
    let claim_digest = hash(&claim_bytes).to_bytes();

    let instance_claim_id = if matches!(fault, Fault::WrongProductClaimLink) {
        product_id([8; 32])
    } else {
        product_id(claim_digest)
    };
    let instance_capacity_id = if matches!(fault, Fault::WrongInstanceCapacityLink) {
        alternate_capacity_id
    } else {
        claim_capacity_id
    };
    let instance_value = InstanceV1::new(InstanceV1Input {
        terms_id: product_id([9; 32]),
        occurrence_id: product_id([10; 32]),
        claim_basis_id: instance_claim_id,
        capacity_profile_id: instance_capacity_id,
        partition_cell_count: 2,
    })
    .expect("canonical Product instance");
    let instance_bytes = instance_value.to_bytes();
    let instance_digest = hash(&instance_bytes).to_bytes();
    let (instance, instance_cursor) = record_addresses(INSTANCE_SCHEMA_LABEL, &instance_bytes);
    let (claim, claim_cursor) = record_addresses(CLAIM_SCHEMA_LABEL, &claim_bytes);
    let (capacity, capacity_cursor) = record_addresses(CAPACITY_SCHEMA_LABEL, &capacity_bytes);

    let feed_profile =
        PythFeedProfileV1::new([11; 32], [12; 32], [13; 32]).expect("canonical Pyth feed profile");
    let upper_edges = [0u128; MAX_PRICE_CELLS];
    let policy = CategoricalPythPolicyRecordV1::new(CategoricalPythV1PolicyInput {
        pyth_release_id: [14; 32],
        feed_profile_id: hash(&feed_profile.to_bytes()).to_bytes(),
        target_time: 1,
        grace: 0,
        window: 1,
        max_crossing_lag: 1,
        max_age: 1,
        max_future_skew: 1,
        confidence_multiplier: 1,
        max_confidence_bps: 1,
        max_normalized_confidence_atoms: 1,
        normalized_decimals: 0,
        price_cell_count: 1,
        upper_edges,
        failure_outcome_index: 1,
    })
    .expect("canonical categorical policy");
    let material_value = CategoricalPythResolutionMaterialV1::new(policy, feed_profile)
        .expect("canonical resolution material");
    let material_bytes = material_value.to_bytes();
    let (material, material_cursor) = record_addresses(MATERIAL_SCHEMA_LABEL, &material_bytes);
    let policy_digest = hash(&policy.to_bytes()).to_bytes();

    let fund_rent = Rent::default().minimum_balance(FUNDING_BYTES);
    let policy_id = core_id(policy_digest);
    let valid_manifest_bytes = manifest_bytes(&[resolution_entry(61, policy_id, fund_rent)]);
    let manifest_bytes = match fault {
        Fault::MissingResolutionManifest => manifest_bytes(&[]),
        Fault::AmbiguousResolutionManifest => manifest_bytes(&[
            resolution_entry(61, policy_id, fund_rent),
            resolution_entry(62, policy_id, fund_rent),
        ]),
        Fault::WrongResolutionQuote => manifest_bytes(&[resolution_entry(
            61,
            policy_id,
            fund_rent.checked_add(1).expect("wrong quote rent"),
        )]),
        _ => valid_manifest_bytes.clone(),
    };
    let manifest_digest = hash(&manifest_bytes).to_bytes();
    let (manifest, manifest_cursor) = record_addresses(MANIFEST_SCHEMA_LABEL, &manifest_bytes);

    let identity = MarketIdentity::new(
        core_id(realm_digest),
        core_id(instance_digest),
        core_id(claim_digest),
        policy_id,
        core_id(manifest_digest),
        GENERATION,
    );
    let instruction =
        FoundMarketAndFundV1::new(identity, 2).expect("canonical founding instruction");
    let identity_digest = hash(&identity.to_bytes()).to_bytes();
    let (canonical_market, _) =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], &PROGRAM_ID);
    let manifest_value =
        CapabilityManifestV1::decode(&valid_manifest_bytes).expect("canonical manifest");
    let selected = manifest_value
        .required_founding_entry_for_config(policy_id)
        .expect("selected founding resolution capability");
    let funding = construct_required_resolution_funding(
        core_id(manifest_digest),
        manifest_value,
        selected,
        fund_rent,
        0,
    )
    .expect("canonical funding state");
    let funding_derivation = CapabilityFundingDerivationV1::new(
        canonical_market.to_bytes(),
        GENERATION,
        core_id(manifest_digest),
        manifest_value,
        funding,
    )
    .expect("canonical funding derivation");
    let (canonical_fund, _) =
        Pubkey::find_program_address(&funding_derivation.seed_components(), &PROGRAM_ID);
    let authority = RefundAuthority::new(sponsor.pubkey().to_bytes()).expect("beneficiary");
    let authority_bytes = authority.to_bytes();
    let (rent_credit, rent_credit_bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, authority_bytes.as_slice()],
        &PROGRAM_ID,
    );
    let rent_credit_state = RentCreditV1::new(authority, rent_credit_bump);

    let submitted_market = if matches!(fault, Fault::WrongMarketPda) {
        Pubkey::new_from_array([91; 32])
    } else {
        canonical_market
    };
    let submitted_fund = if matches!(fault, Fault::AliasedDestinations) {
        submitted_market
    } else {
        canonical_fund
    };

    let rent = Rent::default();
    let market_rent = rent.minimum_balance(
        CategoricalMarketV1::<2>::encoded_len().expect("two-outcome Market width"),
    );
    let fund_rent = rent.minimum_balance(FUNDING_BYTES);
    let required = market_rent
        .checked_add(fund_rent)
        .and_then(|value| value.checked_add(PROVIDER_FEE))
        .and_then(|value| value.checked_add(SUCCESS_BOUNTY))
        .expect("deterministic founding debit");
    let sponsor_before = if matches!(fault, Fault::UnderfundedSponsor) {
        required.checked_sub(1).expect("positive required debit")
    } else {
        required
            .checked_add(SPONSOR_CUSHION)
            .expect("sponsor opening")
    };
    let preexisting_market = if matches!(fault, Fault::PreexistingMarket) {
        Some(Account::new(1, 0, &system_program::ID))
    } else {
        None
    };

    let mut test = ProgramTest::new("dclutch_sbf", PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_account(
        sponsor.pubkey(),
        Account::new(sponsor_before, 0, &system_program::ID),
    );
    test.add_account(realm, account(realm_bytes.to_vec()));
    test.add_account(realm_cursor, vacant_cursor());
    test.add_account(instance, account(instance_bytes.to_vec()));
    test.add_account(instance_cursor, vacant_cursor());
    test.add_account(claim, account(claim_bytes.to_vec()));
    test.add_account(claim_cursor, vacant_cursor());
    test.add_account(capacity, account(capacity_bytes.to_vec()));
    test.add_account(capacity_cursor, vacant_cursor());
    test.add_account(material, account(material_bytes.to_vec()));
    test.add_account(material_cursor, vacant_cursor());
    test.add_account(manifest, account(manifest_bytes));
    test.add_account(manifest_cursor, vacant_cursor());
    if let Some(existing) = preexisting_market.clone() {
        test.add_account(canonical_market, existing);
    }

    Fixture {
        test,
        sponsor,
        sponsor_before,
        instruction,
        realm,
        instance,
        claim,
        capacity,
        material,
        manifest,
        realm_cursor,
        instance_cursor,
        claim_cursor,
        capacity_cursor,
        material_cursor,
        manifest_cursor,
        rent_credit,
        rent_credit_state,
        submitted_market,
        submitted_fund,
        canonical_market,
        canonical_fund,
        preexisting_market,
    }
}

fn founding_instruction(fixture: &Fixture) -> Instruction {
    let mut data = [0; FOUND_MARKET_AND_FUND_BYTES];
    fixture
        .instruction
        .encode(&mut data)
        .expect("exact founding instruction encoding");
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.sponsor.pubkey(), true),
            AccountMeta::new(fixture.submitted_market, false),
            AccountMeta::new(fixture.submitted_fund, false),
            AccountMeta::new_readonly(fixture.rent_credit, false),
            AccountMeta::new_readonly(fixture.realm, false),
            AccountMeta::new_readonly(fixture.instance, false),
            AccountMeta::new_readonly(fixture.claim, false),
            AccountMeta::new_readonly(fixture.capacity, false),
            AccountMeta::new_readonly(fixture.material, false),
            AccountMeta::new_readonly(fixture.manifest, false),
            AccountMeta::new_readonly(fixture.realm_cursor, false),
            AccountMeta::new_readonly(fixture.instance_cursor, false),
            AccountMeta::new_readonly(fixture.claim_cursor, false),
            AccountMeta::new_readonly(fixture.capacity_cursor, false),
            AccountMeta::new_readonly(fixture.material_cursor, false),
            AccountMeta::new_readonly(fixture.manifest_cursor, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ],
        data: data.to_vec(),
    }
}

async fn create_rent_credit(
    context: &mut ProgramTestContext,
    rent_credit: Pubkey,
    rent_credit_state: RentCreditV1,
) {
    assert!(
        observed_account(context, rent_credit).await.is_none(),
        "RentCredit starts vacant"
    );
    let data = CreateRentCreditV1::new(
        rent_credit_state.refund_authority(),
        rent_credit_state.pda_bump(),
    )
    .to_bytes();
    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new(rent_credit, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ],
        data: data.to_vec(),
    };
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("create credit blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    context
        .banks_client
        .process_transaction(transaction)
        .await
        .expect("route exact RentCredit creation through the loaded ELF");
    let credit = observed_account(context, rent_credit)
        .await
        .expect("RentCredit exists");
    assert_eq!(credit.owner, PROGRAM_ID);
    assert_eq!(
        credit.lamports,
        Rent::default().minimum_balance(RENT_CREDIT_BYTES_V1)
    );
    assert_eq!(RentCreditV1::decode(&credit.data), Ok(rent_credit_state));
}

async fn submit(
    context: &mut ProgramTestContext,
    sponsor: &Keypair,
    instruction: Instruction,
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer, sponsor],
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

async fn observed_account(context: &mut ProgramTestContext, address: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(address)
        .await
        .expect("bank account query")
}

#[tokio::test]
async fn found_market_and_fund_uses_real_elf_system_cpis_and_persists_exact_state() {
    let fixture = fixture(Fault::None);
    let instruction = founding_instruction(&fixture);
    let rent_credit = fixture.rent_credit;
    let rent_credit_state = fixture.rent_credit_state;
    let mut context = fixture.test.start_with_context().await;
    create_rent_credit(&mut context, rent_credit, rent_credit_state).await;
    submit(&mut context, &fixture.sponsor, instruction)
        .await
        .expect("canonical founding succeeds through the loaded ELF");

    let market = observed_account(&mut context, fixture.canonical_market)
        .await
        .expect("Market exists");
    let fund = observed_account(&mut context, fixture.canonical_fund)
        .await
        .expect("Fund exists");
    let sponsor = observed_account(&mut context, fixture.sponsor.pubkey())
        .await
        .expect("sponsor exists");
    let market_state = CategoricalMarketV1::<2>::decode(&market.data)
        .expect("exact provider-neutral categorical Market");
    let rent = Rent::default();
    let market_rent =
        rent.minimum_balance(CategoricalMarketV1::<2>::encoded_len().expect("Market width"));
    let fund_rent = rent.minimum_balance(FUNDING_BYTES);
    let fund_lamports = fund_rent + PROVIDER_FEE + SUCCESS_BOUNTY;

    assert_eq!(market.owner, PROGRAM_ID);
    assert_eq!(market.lamports, market_rent);
    assert_eq!(
        market.data.len(),
        CategoricalMarketV1::<2>::encoded_len().expect("Market width")
    );
    assert_eq!(market_state.root().phase(), Phase::Founding);
    assert_eq!(market_state.root().outstanding_children(), 1);
    assert_eq!(market_state.hoard_atoms(), 0);
    assert_eq!(market_state.supply(), &[0, 0]);
    assert_eq!(fund.owner, PROGRAM_ID);
    assert_eq!(fund.lamports, fund_lamports);
    let funding = FundingStateV1::decode(&fund.data).expect("exact raw FundingState");
    assert_eq!(funding.remaining().provider().amount(), PROVIDER_FEE);
    assert_eq!(funding.remaining().bounty().amount(), SUCCESS_BOUNTY);
    assert_eq!(funding.released().rent().amount(), fund_rent);
    let credit = observed_account(&mut context, fixture.rent_credit)
        .await
        .expect("beneficiary RentCredit persists");
    assert_eq!(credit.lamports, rent.minimum_balance(RENT_CREDIT_BYTES_V1));
    assert_eq!(
        RentCreditV1::decode(&credit.data),
        Ok(fixture.rent_credit_state)
    );
    assert_eq!(
        sponsor.lamports,
        fixture.sponsor_before - market_rent - fund_lamports
    );
}

#[tokio::test]
async fn hostile_founding_inputs_roll_back_sponsor_and_never_create_canonical_children() {
    for fault in [
        Fault::WrongProductClaimLink,
        Fault::WrongClaimCapacityLink,
        Fault::WrongInstanceCapacityLink,
        Fault::WrongMarketPda,
        Fault::UnderfundedSponsor,
        Fault::PreexistingMarket,
        Fault::AliasedDestinations,
        Fault::MissingResolutionManifest,
        Fault::AmbiguousResolutionManifest,
        Fault::WrongResolutionQuote,
    ] {
        let fixture = fixture(fault);
        let instruction = founding_instruction(&fixture);
        let rent_credit = fixture.rent_credit;
        let rent_credit_state = fixture.rent_credit_state;
        let mut context = fixture.test.start_with_context().await;
        create_rent_credit(&mut context, rent_credit, rent_credit_state).await;
        assert!(
            submit(&mut context, &fixture.sponsor, instruction)
                .await
                .is_err(),
            "hostile founding must refuse atomically"
        );
        let sponsor = observed_account(&mut context, fixture.sponsor.pubkey())
            .await
            .expect("sponsor remains");
        assert_eq!(sponsor.lamports, fixture.sponsor_before);

        let market = observed_account(&mut context, fixture.canonical_market).await;
        let fund = observed_account(&mut context, fixture.canonical_fund).await;
        let credit = observed_account(&mut context, fixture.rent_credit)
            .await
            .expect("RentCredit remains");
        if let Some(expected) = fixture.preexisting_market {
            assert_eq!(
                market,
                Some(expected),
                "preexisting input remains untouched"
            );
        } else {
            assert!(
                market.is_none(),
                "no canonical Market remains after refusal"
            );
        }
        assert!(fund.is_none(), "no canonical Fund remains after refusal");
        assert_eq!(
            RentCreditV1::decode(&credit.data),
            Ok(fixture.rent_credit_state)
        );
        assert_eq!(
            credit.lamports,
            Rent::default().minimum_balance(RENT_CREDIT_BYTES_V1),
            "founding refusals cannot debit or redirect pre-existing RentCredit"
        );
    }
}
