//! Real-SBF lifecycle evidence for persisted Direct intents selected by the
//! market-independent V3 fee-policy record.
//!
//! The ordinary and unwind paths use ProgramTest's bundled canonical
//! Token-2022 ELF.  No dClutch processor is registered natively or mocked.

use std::{env, path::PathBuf};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    CompartmentFundingV1, FundingAmountsV1, FundingQuoteV1, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot, Phase};
use dclutch_direct_contract::{
    DIRECT_ADAPTER_RELEASE_ID_V2, DIRECT_CAPABILITY_KIND_ID_V2, DIRECT_CAPACITY_PROFILE_ID_V2,
    DIRECT_CHILD_DERIVATION_ID_V2, DIRECT_CHILD_SCHEMA_ID_V2, DIRECT_INTENT_ESCROW_PDA_DOMAIN_V2,
    DIRECT_INTENT_RECORD_PDA_DOMAIN_V2, DirectIntentInputV2, DirectIntentV2, IntentLifecycleV2,
    MAKER_REPLAY_ROOT_PDA_DOMAIN_V2, MakerReplayRootV2, PRICE_SCALE, Side,
    VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3, VenueFeePolicyV3,
    adapter::{
        AdapterAccountMetaV2, AdapterActionV2, account_count_v2, encode_ordinary_instruction_v2,
        encode_register_instruction_v2, validate_account_frame_v2,
    },
};
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, POSITION_PDA_DOMAIN, PositionV1, RealmV1,
    RealmV1Input,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_rent_contract::{
    CreateRentCreditV1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1,
};
use dclutch_token_svm::{
    ACCOUNT_BYTES, CollateralAdapterReleaseV1, MINT_BYTES, TOKEN_2022_PROGRAM_ID, transfer_checked,
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
use solana_sdk_ids::{ed25519_program, system_program, sysvar};
use solana_transaction::Transaction;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([71; 32]);
const GENERATION: u64 = 73;
const OUTCOME: u8 = 0;
const FILL: u64 = 10;
const FEE_BPS: u16 = 1_000;
const DONATION: u64 = 7;
const SPONSOR_OPENING: u64 = 100_000_000;
const MARKET_DUST: u64 = 3;
const ROOT_DUST: u64 = 5;
const RECORD_DUST: u64 = 7;
const ESCROW_DUST: u64 = 11;
const MINT_DECIMALS: u8 = 0;
const MANIFEST_SCHEMA_LABEL: &[u8] = b"dclutch/schema/capability-manifest-profile-1-v1";

#[derive(Clone, Debug, Eq, PartialEq)]
struct AccountSnapshot {
    market: Option<Account>,
    seller_root: Option<Account>,
    buyer_root: Option<Account>,
    seller_record: Option<Account>,
    buyer_record: Option<Account>,
    buyer_escrow: Option<Account>,
    seller_position: Option<Account>,
    buyer_position: Option<Account>,
    seller_collateral: Option<Account>,
    buyer_collateral: Option<Account>,
    fee_recipient: Option<Account>,
    credit: Option<Account>,
}

struct Fixture {
    test: Option<ProgramTest>,
    sponsor: Keypair,
    seller: Keypair,
    buyer: Keypair,
    market: Pubkey,
    realm: Pubkey,
    policy: Pubkey,
    policy_digest: [u8; 32],
    policy_cursor: Pubkey,
    manifest: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
    fee_recipient: Pubkey,
    seller_position: Pubkey,
    buyer_position: Pubkey,
    seller_collateral: Pubkey,
    buyer_collateral: Pubkey,
    credit: Pubkey,
    credit_state: RentCreditV1,
    seller_root: Pubkey,
    buyer_root: Pubkey,
    seller_record: Pubkey,
    buyer_record: Pubkey,
    buyer_escrow: Pubkey,
    reserve: u64,
}

fn require_sbf() {
    let directory = env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required for real ELF tests");
    assert!(PathBuf::from(directory).join("dclutch_sbf.so").is_file());
}

fn protocol_account(data: Vec<u8>) -> Account {
    Account {
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn content(bytes: [u8; 32]) -> ContentId {
    ContentId::new(bytes).expect("nonzero content ID")
}

fn token_mint(supply: u64) -> Vec<u8> {
    let mut data = vec![0; MINT_BYTES];
    data[36..44].copy_from_slice(&supply.to_le_bytes());
    data[44] = MINT_DECIMALS;
    data[45] = 1;
    data
}

fn token_account(
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    delegate: Option<(Pubkey, u64)>,
) -> Vec<u8> {
    let mut data = vec![0; ACCOUNT_BYTES];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    if let Some((delegate, allowance)) = delegate {
        data[72..76].copy_from_slice(&1_u32.to_le_bytes());
        data[76..108].copy_from_slice(delegate.as_ref());
        data[121..129].copy_from_slice(&allowance.to_le_bytes());
    }
    data[108] = 1;
    data
}

fn token_amount(account: &Account) -> u64 {
    u64::from_le_bytes(account.data[64..72].try_into().expect("token amount"))
}

fn record_addresses(schema: [u8; 32], bytes: &[u8]) -> (Pubkey, Pubkey) {
    let digest = hash(bytes).to_bytes();
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
        &PROGRAM_ID,
    )
    .0;
    let cursor = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema.as_slice(),
            digest.as_slice(),
        ],
        &PROGRAM_ID,
    )
    .0;
    (raw, cursor)
}

fn zero_quote() -> FundingQuoteV1 {
    FundingQuoteV1::new(
        FundingAmountsV1::new(
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("zero funding"),
        None,
    )
    .expect("native-free direct funding")
}

fn manifest(policy_digest: [u8; 32]) -> Vec<u8> {
    let entry = CapabilityEntryV1::new(
        content(DIRECT_CAPABILITY_KIND_ID_V2),
        content(DIRECT_ADAPTER_RELEASE_ID_V2),
        content(policy_digest),
        content(DIRECT_CAPACITY_PROFILE_ID_V2),
        content(DIRECT_CHILD_SCHEMA_ID_V2),
        content(DIRECT_CHILD_DERIVATION_ID_V2),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        zero_quote(),
    )
    .expect("Direct manifest entry");
    let mut bytes = vec![0; 16 + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut bytes)
        .expect("Direct manifest")
        .as_bytes()
        .to_vec()
}

fn encode_position(position: PositionV1<2>) -> Vec<u8> {
    let mut bytes = vec![0; PositionV1::<2>::encoded_len().expect("Position width")];
    position.encode(&mut bytes).expect("Position bytes");
    bytes
}

fn encode_market(root: MarketRoot) -> Vec<u8> {
    let market =
        CategoricalMarketV1::<2>::new(root, 0, [0; 2], CategoricalSettlementSummaryV1::empty())
            .expect("Open Market");
    let mut bytes = vec![0; CategoricalMarketV1::<2>::encoded_len().expect("Market width")];
    market.encode(&mut bytes).expect("Market bytes");
    bytes
}

struct IntentAccounts {
    position: Pubkey,
    collateral: Pubkey,
    fee_config: [u8; 32],
    valid_through_slot: u64,
}

fn direct_intent(
    market: Pubkey,
    maker: Pubkey,
    nonce: u64,
    side: Side,
    accounts: IntentAccounts,
) -> DirectIntentV2 {
    DirectIntentV2::new(DirectIntentInputV2 {
        market: market.to_bytes(),
        generation: GENERATION,
        maker: maker.to_bytes(),
        nonce,
        valid_from_slot: 0,
        valid_through_slot: accounts.valid_through_slot,
        side,
        lifecycle: IntentLifecycleV2::Registered,
        outcome: OUTCOME,
        max_fill: FILL,
        limit_price: PRICE_SCALE,
        fee_config: accounts.fee_config,
        fee_basis_points: FEE_BPS,
        position_account: accounts.position.to_bytes(),
        collateral_account: accounts.collateral.to_bytes(),
    })
    .expect("Direct intent")
}

fn fixture() -> Fixture {
    require_sbf();
    let sponsor = Keypair::new();
    let seller = Keypair::new();
    let buyer = Keypair::new();
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
    let mint = Pubkey::new_unique();
    let fee_recipient = Pubkey::new_unique();
    let release = CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer();
    let realm_value = RealmV1::new(RealmV1Input {
        token_program: token_program.to_bytes(),
        collateral_mint: mint.to_bytes(),
        collateral_adapter_release_id: hash(&release.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("Token-2022 Realm");
    let realm_bytes = realm_value.to_bytes().to_vec();
    let realm_id = hash(&realm_bytes).to_bytes();
    let realm = Pubkey::find_program_address(
        &[
            dclutch_realm_contract::REALM_PDA_DOMAIN,
            realm_id.as_slice(),
        ],
        &PROGRAM_ID,
    )
    .0;

    // V3 policy content is intentionally independent of the downstream
    // Market PDA. The immutable Market manifest selects this exact digest,
    // while every intent binds the resulting Market and generation.
    let policy_value =
        VenueFeePolicyV3::new(fee_recipient.to_bytes(), FEE_BPS).expect("Venue fee policy");
    let mut policy_bytes = vec![0; dclutch_direct_contract::VENUE_FEE_POLICY_BYTES_V3];
    policy_value
        .encode(&mut policy_bytes)
        .expect("policy bytes");
    let policy_digest = hash(&policy_bytes).to_bytes();
    let manifest_bytes = manifest(policy_digest);
    let manifest_id = hash(&manifest_bytes).to_bytes();
    let identity = MarketIdentity::new(
        content(realm_id),
        content([11; 32]),
        content([12; 32]),
        content([13; 32]),
        content(manifest_id),
        GENERATION,
    );
    let market_digest = hash(&identity.to_bytes()).to_bytes();
    let market = Pubkey::find_program_address(
        &[b"dclutch/market-root/v1", market_digest.as_slice()],
        &PROGRAM_ID,
    )
    .0;

    let (policy, policy_cursor) =
        record_addresses(VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3, &policy_bytes);
    let manifest_schema = hash(MANIFEST_SCHEMA_LABEL).to_bytes();
    let (manifest, _) = record_addresses(manifest_schema, &manifest_bytes);
    let (seller_position, _) = Pubkey::find_program_address(
        &[
            POSITION_PDA_DOMAIN,
            market.as_ref(),
            seller.pubkey().as_ref(),
        ],
        &PROGRAM_ID,
    );
    let (buyer_position, _) = Pubkey::find_program_address(
        &[
            POSITION_PDA_DOMAIN,
            market.as_ref(),
            buyer.pubkey().as_ref(),
        ],
        &PROGRAM_ID,
    );
    let seller_collateral = Pubkey::new_unique();
    let buyer_collateral = Pubkey::new_unique();
    let authority = RefundAuthority::new(sponsor.pubkey().to_bytes()).expect("sponsor authority");
    let (credit, credit_bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, authority.to_bytes().as_slice()],
        &PROGRAM_ID,
    );
    let credit_state = RentCreditV1::new(authority, credit_bump);
    let (seller_root, _) = Pubkey::find_program_address(
        &[
            MAKER_REPLAY_ROOT_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
            seller.pubkey().as_ref(),
        ],
        &PROGRAM_ID,
    );
    let (buyer_root, _) = Pubkey::find_program_address(
        &[
            MAKER_REPLAY_ROOT_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
            buyer.pubkey().as_ref(),
        ],
        &PROGRAM_ID,
    );
    let (seller_record, _) = Pubkey::find_program_address(
        &[
            DIRECT_INTENT_RECORD_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
            seller.pubkey().as_ref(),
            &0_u64.to_le_bytes(),
        ],
        &PROGRAM_ID,
    );
    let (buyer_record, _) = Pubkey::find_program_address(
        &[
            DIRECT_INTENT_RECORD_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
            buyer.pubkey().as_ref(),
            &0_u64.to_le_bytes(),
        ],
        &PROGRAM_ID,
    );
    let (buyer_escrow, _) = Pubkey::find_program_address(
        &[DIRECT_INTENT_ESCROW_PDA_DOMAIN_V2, buyer_record.as_ref()],
        &PROGRAM_ID,
    );
    let reserve = FILL + FILL * u64::from(FEE_BPS) / 10_000;

    let mut root =
        MarketRoot::founding(identity, sponsor.pubkey().to_bytes()).expect("Market root");
    root.register_child(GENERATION, 0).expect("Fund child");
    root.register_child(GENERATION, 1).expect("custody child");
    root.transition_phase(GENERATION, Phase::Open)
        .expect("Open Market");
    let mut test = ProgramTest::new("dclutch_sbf", PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    test.add_account(
        sponsor.pubkey(),
        Account::new(SPONSOR_OPENING, 0, &system_program::ID),
    );
    test.add_account(seller.pubkey(), Account::new(1, 0, &system_program::ID));
    test.add_account(buyer.pubkey(), Account::new(1, 0, &system_program::ID));
    test.add_account(
        market,
        Account {
            lamports: Rent::default()
                .minimum_balance(CategoricalMarketV1::<2>::encoded_len().expect("Market width"))
                + MARKET_DUST,
            ..protocol_account(encode_market(root))
        },
    );
    test.add_account(realm, protocol_account(realm_bytes));
    test.add_account(policy, protocol_account(policy_bytes));
    test.add_account(manifest, protocol_account(manifest_bytes));
    test.add_account(policy_cursor, Account::new(0, 0, &system_program::ID));
    test.add_account(
        mint,
        Account {
            lamports: Rent::default().minimum_balance(MINT_BYTES),
            data: token_mint(1_000),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    );
    for (key, owner, amount, delegate) in [
        (seller_collateral, seller.pubkey(), 0, None),
        (
            buyer_collateral,
            buyer.pubkey(),
            100,
            Some((buyer_root, reserve)),
        ),
        (fee_recipient, Pubkey::new_unique(), 0, None),
    ] {
        test.add_account(
            key,
            Account {
                lamports: Rent::default().minimum_balance(ACCOUNT_BYTES),
                data: token_account(mint, owner, amount, delegate),
                owner: token_program,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    test.add_account(
        seller_position,
        protocol_account(encode_position(
            PositionV1::new(
                market.to_bytes(),
                seller.pubkey().to_bytes(),
                GENERATION,
                [20, 0],
            )
            .expect("seller Position"),
        )),
    );
    test.add_account(
        buyer_position,
        protocol_account(encode_position(
            PositionV1::new(
                market.to_bytes(),
                buyer.pubkey().to_bytes(),
                GENERATION,
                [0, 0],
            )
            .expect("buyer Position"),
        )),
    );
    for (key, lamports) in [
        (seller_root, ROOT_DUST),
        (buyer_root, ROOT_DUST),
        (seller_record, RECORD_DUST),
        (buyer_record, RECORD_DUST),
        (buyer_escrow, ESCROW_DUST),
    ] {
        test.add_account(key, Account::new(lamports, 0, &system_program::ID));
    }

    Fixture {
        test: Some(test),
        sponsor,
        seller,
        buyer,
        market,
        realm,
        policy,
        policy_digest,
        policy_cursor,
        manifest,
        mint,
        token_program,
        fee_recipient,
        seller_position,
        buyer_position,
        seller_collateral,
        buyer_collateral,
        credit,
        credit_state,
        seller_root,
        buyer_root,
        seller_record,
        buyer_record,
        buyer_escrow,
        reserve,
    }
}

fn to_adapter_metas(accounts: &[AccountMeta]) -> Vec<AdapterAccountMetaV2> {
    accounts
        .iter()
        .map(|account| AdapterAccountMetaV2 {
            key: account.pubkey.to_bytes(),
            is_signer: account.is_signer,
            is_writable: account.is_writable,
        })
        .collect()
}

fn assert_frame(action: AdapterActionV2, accounts: &[AccountMeta]) {
    assert_eq!(
        accounts.len(),
        account_count_v2(
            action,
            match action {
                AdapterActionV2::Ordinary => 2,
                _ => 1,
            }
        )
        .expect("frame count")
    );
    validate_account_frame_v2(
        action,
        match action {
            AdapterActionV2::Ordinary => 2,
            _ => 1,
        },
        &to_adapter_metas(accounts),
    )
    .expect("canonical Direct frame");
}

fn registration_accounts(fixture: &Fixture, side: Side) -> Vec<AccountMeta> {
    match side {
        Side::Sell => vec![
            AccountMeta::new(fixture.sponsor.pubkey(), true),
            AccountMeta::new_readonly(fixture.credit, false),
            AccountMeta::new(fixture.market, false),
            AccountMeta::new_readonly(fixture.policy, false),
            AccountMeta::new_readonly(fixture.policy_cursor, false),
            AccountMeta::new_readonly(fixture.manifest, false),
            AccountMeta::new(fixture.seller_root, false),
            AccountMeta::new(fixture.seller_record, false),
            AccountMeta::new(fixture.seller_position, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(sysvar::instructions::ID, false),
        ],
        Side::Buy => vec![
            AccountMeta::new(fixture.sponsor.pubkey(), true),
            AccountMeta::new_readonly(fixture.credit, false),
            AccountMeta::new(fixture.market, false),
            AccountMeta::new_readonly(fixture.realm, false),
            AccountMeta::new_readonly(fixture.policy, false),
            AccountMeta::new_readonly(fixture.policy_cursor, false),
            AccountMeta::new_readonly(fixture.manifest, false),
            AccountMeta::new(fixture.buyer_root, false),
            AccountMeta::new(fixture.buyer_record, false),
            AccountMeta::new(fixture.buyer_escrow, false),
            AccountMeta::new_readonly(fixture.buyer_position, false),
            AccountMeta::new(fixture.buyer_collateral, false),
            AccountMeta::new_readonly(fixture.mint, false),
            AccountMeta::new_readonly(fixture.token_program, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(sysvar::instructions::ID, false),
        ],
    }
}

fn ordinary_accounts(fixture: &Fixture) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new_readonly(fixture.market, false),
        AccountMeta::new_readonly(fixture.realm, false),
        AccountMeta::new_readonly(fixture.policy, false),
        AccountMeta::new_readonly(fixture.policy_cursor, false),
        AccountMeta::new_readonly(fixture.manifest, false),
        AccountMeta::new(fixture.fee_recipient, false),
        AccountMeta::new_readonly(fixture.mint, false),
        AccountMeta::new_readonly(fixture.token_program, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new(fixture.seller_root, false),
        AccountMeta::new(fixture.seller_record, false),
        AccountMeta::new_readonly(fixture.seller_position, false),
        AccountMeta::new(fixture.seller_collateral, false),
        AccountMeta::new(fixture.credit, false),
        AccountMeta::new(fixture.buyer_root, false),
        AccountMeta::new(fixture.buyer_record, false),
        AccountMeta::new(fixture.buyer_escrow, false),
        AccountMeta::new(fixture.buyer_position, false),
        AccountMeta::new(fixture.buyer_collateral, false),
        AccountMeta::new(fixture.credit, false),
    ]
}

fn signed_ed25519_instruction(maker: &Keypair, direct_data: &[u8], message: &[u8]) -> Instruction {
    assert_eq!(message, &direct_data[16..16 + message.len()]);
    let payload = 2 + 14;
    let signature = maker.sign_message(message);
    let mut data = vec![0; payload + 96];
    data[0..2].copy_from_slice(&1_u16.to_le_bytes());
    data[2..4].copy_from_slice(
        &u16::try_from(payload + 32)
            .expect("signature offset")
            .to_le_bytes(),
    );
    data[4..6].copy_from_slice(&u16::MAX.to_le_bytes());
    data[6..8].copy_from_slice(&u16::try_from(payload).expect("pubkey offset").to_le_bytes());
    data[10..12].copy_from_slice(&16_u16.to_le_bytes());
    data[12..14].copy_from_slice(
        &u16::try_from(message.len())
            .expect("message length")
            .to_le_bytes(),
    );
    data[14..16].copy_from_slice(&1_u16.to_le_bytes());
    data[payload..payload + 32].copy_from_slice(maker.pubkey().as_ref());
    data[payload + 32..payload + 96].copy_from_slice(signature.as_ref());
    Instruction {
        program_id: ed25519_program::ID,
        accounts: vec![],
        data,
    }
}

fn registration_instruction(
    fixture: &Fixture,
    intent: DirectIntentV2,
) -> (Instruction, Instruction) {
    let data = encode_register_instruction_v2(intent).expect("register data");
    let action = match intent.side() {
        Side::Sell => AdapterActionV2::RegisterSell,
        Side::Buy => AdapterActionV2::RegisterBuy,
    };
    let accounts = registration_accounts(fixture, intent.side());
    assert_frame(action, &accounts);
    let maker = match intent.side() {
        Side::Sell => &fixture.seller,
        Side::Buy => &fixture.buyer,
    };
    (
        signed_ed25519_instruction(maker, &data, &intent.signed_preimage()),
        Instruction {
            program_id: PROGRAM_ID,
            accounts,
            data: data.to_vec(),
        },
    )
}

fn ordinary_instruction(fixture: &Fixture, fill: u64, execution_price: u64) -> Instruction {
    let accounts = ordinary_accounts(fixture);
    assert_frame(AdapterActionV2::Ordinary, &accounts);
    Instruction {
        program_id: PROGRAM_ID,
        accounts,
        data: encode_ordinary_instruction_v2(fill, execution_price).to_vec(),
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let mut all = vec![&context.payer];
    all.extend_from_slice(signers);
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &all,
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

async fn submit_with_cu(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Result<u64, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let mut all = vec![&context.payer];
    all.extend_from_slice(signers);
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &all,
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    processed.result?;
    processed
        .metadata
        .map(|metadata| metadata.compute_units_consumed)
        .ok_or(BanksClientError::ClientError(
            "missing ProgramTest transaction metadata",
        ))
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
}

async fn create_credit(context: &mut ProgramTestContext, fixture: &Fixture) -> Account {
    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.sponsor.pubkey(), true),
            AccountMeta::new(fixture.credit, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ],
        data: CreateRentCreditV1::new(
            fixture.credit_state.refund_authority(),
            fixture.credit_state.pda_bump(),
        )
        .to_bytes()
        .to_vec(),
    };
    submit(context, &[instruction], &[&fixture.sponsor])
        .await
        .expect("routed RentCredit creation");
    account(context, fixture.credit).await.expect("RentCredit")
}

async fn snapshot(context: &mut ProgramTestContext, fixture: &Fixture) -> AccountSnapshot {
    AccountSnapshot {
        market: account(context, fixture.market).await,
        seller_root: account(context, fixture.seller_root).await,
        buyer_root: account(context, fixture.buyer_root).await,
        seller_record: account(context, fixture.seller_record).await,
        buyer_record: account(context, fixture.buyer_record).await,
        buyer_escrow: account(context, fixture.buyer_escrow).await,
        seller_position: account(context, fixture.seller_position).await,
        buyer_position: account(context, fixture.buyer_position).await,
        seller_collateral: account(context, fixture.seller_collateral).await,
        buyer_collateral: account(context, fixture.buyer_collateral).await,
        fee_recipient: account(context, fixture.fee_recipient).await,
        credit: account(context, fixture.credit).await,
    }
}

fn token_transfer_instruction(
    source: Pubkey,
    mint: Pubkey,
    destination: Pubkey,
    owner: &Keypair,
    amount: u64,
) -> Instruction {
    let spec = transfer_checked(
        TOKEN_2022_PROGRAM_ID,
        source.to_bytes(),
        mint.to_bytes(),
        destination.to_bytes(),
        owner.pubkey().to_bytes(),
        amount,
        MINT_DECIMALS,
    )
    .expect("exact token transfer");
    Instruction {
        program_id: Pubkey::new_from_array(*spec.program_id()),
        accounts: spec
            .accounts()
            .iter()
            .map(|meta| {
                if meta.is_writable() {
                    AccountMeta::new(Pubkey::new_from_array(*meta.address()), meta.is_signer())
                } else {
                    AccountMeta::new_readonly(
                        Pubkey::new_from_array(*meta.address()),
                        meta.is_signer(),
                    )
                }
            })
            .collect(),
        data: spec.data().to_vec(),
    }
}

#[tokio::test]
async fn direct_registered_ordinary_moves_real_tokens_returns_donation_and_refuses_replay() {
    let mut fixture = fixture();
    let seller_intent = direct_intent(
        fixture.market,
        fixture.seller.pubkey(),
        0,
        Side::Sell,
        IntentAccounts {
            position: fixture.seller_position,
            collateral: fixture.seller_collateral,
            fee_config: fixture.policy_digest,
            valid_through_slot: 1_000,
        },
    );
    let buyer_intent = direct_intent(
        fixture.market,
        fixture.buyer.pubkey(),
        0,
        Side::Buy,
        IntentAccounts {
            position: fixture.buyer_position,
            collateral: fixture.buyer_collateral,
            fee_config: fixture.policy_digest,
            valid_through_slot: 1_000,
        },
    );
    let (seller_ed, seller_register) = registration_instruction(&fixture, seller_intent);
    let (buyer_ed, buyer_register) = registration_instruction(&fixture, buyer_intent);
    let ordinary = ordinary_instruction(&fixture, FILL, PRICE_SCALE);
    let mut context = fixture
        .test
        .take()
        .expect("unstarted Direct fixture")
        .start_with_context()
        .await;
    let credit_before = create_credit(&mut context, &fixture).await;
    // Each native signature descriptor names the immediately following
    // Direct instruction at transaction index one. Keeping them in separate
    // transactions makes that authenticated adjacency exact for both makers.
    submit(
        &mut context,
        &[seller_ed, seller_register],
        &[&fixture.sponsor],
    )
    .await
    .expect("real registered Direct sell");
    submit(
        &mut context,
        &[buyer_ed, buyer_register],
        &[&fixture.sponsor],
    )
    .await
    .expect("real registered Direct buy");
    let registered = snapshot(&mut context, &fixture).await;
    assert_eq!(
        token_amount(
            registered
                .buyer_collateral
                .as_ref()
                .expect("buyer collateral"),
        ),
        100 - fixture.reserve
    );
    assert_eq!(
        token_amount(registered.buyer_escrow.as_ref().expect("escrow")),
        fixture.reserve
    );
    assert_eq!(
        PositionV1::<2>::decode(
            &registered
                .seller_position
                .as_ref()
                .expect("seller Position")
                .data,
        )
        .expect("seller Position"),
        PositionV1::new(
            fixture.market.to_bytes(),
            fixture.seller.pubkey().to_bytes(),
            GENERATION,
            [10, 0],
        )
        .expect("reserved seller claims")
    );

    // The exact registered quantities are preflighted before either token CPI.
    // An oversized match must leave every ordered record, token balance, and
    // RentCredit byte-for-byte intact, including the dust on output accounts.
    let oversized = ordinary_instruction(&fixture, FILL + 1, PRICE_SCALE);
    let oversized_before = snapshot(&mut context, &fixture).await;
    assert!(submit(&mut context, &[oversized], &[]).await.is_err());
    assert_eq!(snapshot(&mut context, &fixture).await, oversized_before);

    submit(
        &mut context,
        &[token_transfer_instruction(
            fixture.buyer_collateral,
            fixture.mint,
            fixture.buyer_escrow,
            &fixture.buyer,
            DONATION,
        )],
        &[&fixture.buyer],
    )
    .await
    .expect("real third-party escrow donation");
    let ordinary_cu = submit_with_cu(&mut context, std::slice::from_ref(&ordinary), &[])
        .await
        .expect("real Direct ordinary match");
    assert!(
        ordinary_cu > 0,
        "ordinary match reported zero compute units"
    );
    eprintln!("direct registered ordinary CU: {ordinary_cu}");
    let after = snapshot(&mut context, &fixture).await;
    let seller_position = PositionV1::<2>::decode(
        &after
            .seller_position
            .as_ref()
            .expect("seller Position")
            .data,
    )
    .expect("seller Position");
    let buyer_position =
        PositionV1::<2>::decode(&after.buyer_position.as_ref().expect("buyer Position").data)
            .expect("buyer Position");
    assert_eq!(seller_position.balances(), &[10, 0]);
    assert_eq!(buyer_position.balances(), &[FILL, 0]);
    assert_eq!(
        token_amount(after.seller_collateral.as_ref().expect("seller collateral"),),
        FILL
    );
    assert_eq!(
        token_amount(after.fee_recipient.as_ref().expect("fee account")),
        1,
        "one named floor fee boundary"
    );
    assert_eq!(
        token_amount(after.buyer_collateral.as_ref().expect("buyer collateral"),),
        100 - fixture.reserve - DONATION + DONATION
    );
    let seller_root =
        MakerReplayRootV2::decode(&after.seller_root.as_ref().expect("seller root").data)
            .expect("seller replay root");
    let buyer_root =
        MakerReplayRootV2::decode(&after.buyer_root.as_ref().expect("buyer root").data)
            .expect("buyer replay root");
    assert_eq!(seller_root.live_intent_count(), 0);
    assert_eq!(buyer_root.live_intent_count(), 0);
    for account in [
        after.seller_record.as_ref().expect("seller record"),
        after.buyer_record.as_ref().expect("buyer record"),
        after.buyer_escrow.as_ref().expect("buyer escrow"),
    ] {
        assert_eq!(account.owner, system_program::ID);
        assert_eq!(account.lamports, 0);
        assert!(account.data.is_empty());
    }
    let credit = after.credit.as_ref().expect("RentCredit");
    assert!(credit.lamports > credit_before.lamports);
    assert_eq!(RentCreditV1::decode(&credit.data), Ok(fixture.credit_state));
    assert!(submit(&mut context, &[ordinary], &[]).await.is_err());
    assert_eq!(snapshot(&mut context, &fixture).await, after);
}
