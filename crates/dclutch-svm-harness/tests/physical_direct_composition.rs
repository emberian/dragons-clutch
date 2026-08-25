//! Real signed-intent, compiled-transition, claim, and custody SBF campaign.
//!
//! The native Ed25519 precompile plus exact controller, claim, custody, and
//! official SPL Token ELFs execute. No native processor or mock token is used.

use std::{
    env, fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityManifestV1, CompartmentFundingV1, FundingAmountsV1,
    FundingQuoteV1, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot, Phase};
use dclutch_direct_codec::{
    COMPACT_INTENT_BYTES, COMPILED_DIRECT_CAPACITY_ID_V1, COMPILED_DIRECT_CHILD_SCHEMA_ID_V1,
    COMPILED_DIRECT_DERIVATION_ID_V1, COMPILED_DIRECT_RELEASE_ID_V1, CompactIntentV1,
    ControllerInstructionV1, RegisteredCreateInstructionV1, RegisteredFillInstructionV1,
    RegisteredIntentStateV1, RegisteredTerminalAction, RegisteredTerminalInstructionV1,
};
use dclutch_direct_contract::{
    DIRECT_CAPABILITY_KIND_ID_V2, VENUE_FEE_POLICY_BYTES_V3, VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3,
    VenueFeePolicyV3,
};
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_PDA_DOMAIN, RealmV1, RealmV1Input,
};
use dclutch_record_contract::RAW_RECORD_PDA_SEED_V1;
use dclutch_token_svm::{LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES};
use solana_account::Account;
use solana_address_lookup_table_interface::{
    instruction::{
        close_lookup_table, create_lookup_table, deactivate_lookup_table, extend_lookup_table,
    },
    state::{AddressLookupTable as LookupTableState, estimate_last_valid_slot},
};
use solana_commitment_config::CommitmentConfig;
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_rpc_client::rpc_client::RpcClient;
use solana_sdk::signature::{Keypair, Signer, keypair_from_seed};
use solana_sdk_ids::{ed25519_program, system_program, sysvar};
use solana_transaction::{Transaction, versioned::VersionedTransaction};

const CONTROLLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([67_u8; 32]);
const CLAIM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([81_u8; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([75_u8; 32]);
const PROTOCOL_PROGRAM_ID: Pubkey = Pubkey::new_from_array([68_u8; 32]);
const CONTROLLER_SEED: &[u8] = b"dclutch-controller-v1";
const REPLAY_SEED: &[u8] = b"dclutch/direct-replay/v3";
const REGISTERED_SEED: &[u8] = b"dclutch/direct-registered/v1";
const POSITION_SEED: &[u8] = b"dclutch/position/v1";
const REPLAY_STATE_BYTES: usize = 48;
const POSITION_STATE_BYTES: usize = 56;
const JOURNAL_BYTES: usize = 16;
const TOKEN_ACCOUNT_BYTES: usize = 165;
const MINT_BYTES: usize = 82;
const GENERATION: u64 = 3;
const FILL: u64 = 2_000;
const PRICE: u64 = 500_000;
const FEE_BPS: u16 = 25;

struct TransactionResult {
    accepted: bool,
    compute_units: u64,
    wire_bytes: usize,
    v0_wire_bytes: usize,
    market_v0_wire_bytes: usize,
    logs: Vec<String>,
}

#[derive(Clone, Copy)]
struct TokenTriplet {
    source: Pubkey,
    seller: Pubkey,
    venue: Pubkey,
}

#[derive(Clone, Copy)]
struct MarketFixture {
    market: Pubkey,
    realm: Pubkey,
    fee_policy: Pubkey,
    capability_manifest: Pubkey,
    seller_replay: Pubkey,
    seller_bump: u8,
    buyer_replay: Pubkey,
    buyer_bump: u8,
    seller_position: Pubkey,
    seller_position_bump: u8,
    buyer_position: Pubkey,
    buyer_position_bump: u8,
    tokens: TokenTriplet,
}

fn token_program_id() -> Pubkey {
    Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID)
}

fn require_sbf() {
    let directory = env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required for real ELF tests");
    for artifact in [
        "dclutch_controller_proof_sbf.so",
        "dclutch_claims_proof_sbf.so",
        "dclutch_custody_proof_sbf.so",
        "spl_token.so",
    ] {
        assert!(
            PathBuf::from(&directory).join(artifact).is_file(),
            "SBF_OUT_DIR must contain {artifact}"
        );
    }
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64 field"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32 field"))
}

fn replay_state(authority: Pubkey, nonce: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; REPLAY_STATE_BYTES];
    bytes[0..8].copy_from_slice(&[b'D', b'C', b'R', b'P', 1, 0, 0, 0]);
    bytes[8..40].copy_from_slice(authority.as_ref());
    put_u64(&mut bytes, 40, nonce);
    bytes
}

fn position_state(authority: Pubkey, outcome: u64, claims: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; POSITION_STATE_BYTES];
    bytes[0..8].copy_from_slice(&[b'D', b'C', b'P', b'N', 1, 0, 0, 0]);
    bytes[8..40].copy_from_slice(authority.as_ref());
    put_u64(&mut bytes, 40, outcome);
    put_u64(&mut bytes, 48, claims);
    bytes
}

fn journal(counter: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; JOURNAL_BYTES];
    bytes[0..4].copy_from_slice(b"DCCJ");
    put_u64(&mut bytes, 8, counter);
    bytes
}

fn content(bytes: [u8; 32]) -> ContentId {
    ContentId::new(bytes).expect("nonzero content ID")
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
    .expect("compiled Direct has no capability principal")
}

fn realm_record(mint: Pubkey) -> (Pubkey, [u8; 32], Vec<u8>) {
    let adapter_release = hash(&PRODUCTION_ADAPTER_RELEASES[0].to_bytes()).to_bytes();
    let realm = RealmV1::new(RealmV1Input {
        token_program: token_program_id().to_bytes(),
        collateral_mint: mint.to_bytes(),
        collateral_adapter_release_id: adapter_release,
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("canonical Realm");
    let bytes = realm.to_bytes();
    let digest = hash(&bytes).to_bytes();
    let key = Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &digest], &PROTOCOL_PROGRAM_ID).0;
    (key, digest, bytes.to_vec())
}

fn raw_record(schema: [u8; 32], bytes: &[u8]) -> Pubkey {
    let digest = hash(bytes).to_bytes();
    Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        &PROTOCOL_PROGRAM_ID,
    )
    .0
}

fn authority_records(
    mint: Pubkey,
    venue: Pubkey,
) -> (MarketFixtureAuthority, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let (realm, realm_digest, realm_bytes) = realm_record(mint);
    let policy = VenueFeePolicyV3::new(venue.to_bytes(), FEE_BPS).expect("fee policy");
    let mut policy_bytes = vec![0_u8; VENUE_FEE_POLICY_BYTES_V3];
    policy.encode(&mut policy_bytes).expect("fee policy bytes");
    let policy_digest = hash(&policy_bytes).to_bytes();
    let fee_policy = raw_record(VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3, &policy_bytes);

    let entry = CapabilityEntryV1::new(
        content(DIRECT_CAPABILITY_KIND_ID_V2),
        content(COMPILED_DIRECT_RELEASE_ID_V1),
        content(policy_digest),
        content(COMPILED_DIRECT_CAPACITY_ID_V1),
        content(COMPILED_DIRECT_CHILD_SCHEMA_ID_V1),
        content(COMPILED_DIRECT_DERIVATION_ID_V1),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        zero_quote(),
    )
    .expect("compiled Direct manifest entry");
    let mut manifest_bytes = vec![0_u8; 16 + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes).expect("capability manifest");
    let manifest_digest = hash(&manifest_bytes).to_bytes();
    let capability_manifest = raw_record(CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, &manifest_bytes);

    let identity = MarketIdentity::new(
        content(realm_digest),
        content([21; 32]),
        content([22; 32]),
        content([23; 32]),
        content(manifest_digest),
        GENERATION,
    );
    let mut root = MarketRoot::founding(identity, [24; 32]).expect("founding root");
    root.transition_phase(GENERATION, Phase::Open)
        .expect("open root");
    let market =
        CategoricalMarketV1::<2>::new(root, 0, [0; 2], CategoricalSettlementSummaryV1::empty())
            .expect("open Market");
    let mut market_bytes =
        vec![0_u8; CategoricalMarketV1::<2>::encoded_len().expect("Market width")];
    market.encode(&mut market_bytes).expect("Market bytes");
    let identity_digest = hash(&identity.to_bytes()).to_bytes();
    let market_key = Pubkey::find_program_address(
        &[b"dclutch/market-root/v1", &identity_digest],
        &PROTOCOL_PROGRAM_ID,
    )
    .0;
    (
        MarketFixtureAuthority {
            market: market_key,
            realm,
            fee_policy,
            capability_manifest,
        },
        market_bytes,
        realm_bytes,
        policy_bytes,
        manifest_bytes,
    )
}

#[derive(Clone, Copy)]
struct MarketFixtureAuthority {
    market: Pubkey,
    realm: Pubkey,
    fee_policy: Pubkey,
    capability_manifest: Pubkey,
}

fn compact_intent(market: Pubkey, collateral: Pubkey, side: u8, nonce: u64) -> CompactIntentV1 {
    CompactIntentV1 {
        side,
        outcome: 1,
        lifecycle: 0,
        market: market.to_bytes(),
        generation: GENERATION,
        nonce,
        valid_from: 0,
        valid_through: u64::MAX,
        maximum_fill: FILL,
        limit_price: if side == 0 { 400_000 } else { 600_000 },
        fee_basis_points: FEE_BPS,
        collateral_account: collateral.to_bytes(),
    }
}

fn registered_intent(market: Pubkey, collateral: Pubkey, side: u8) -> CompactIntentV1 {
    CompactIntentV1 {
        lifecycle: 2,
        ..compact_intent(market, collateral, side, 0)
    }
}

fn registered_state(controller: Pubkey, maker: Pubkey, intent: CompactIntentV1) -> Vec<u8> {
    RegisteredIntentStateV1 {
        phase: 0,
        controller: controller.to_bytes(),
        maker: maker.to_bytes(),
        intent,
        remaining: intent.maximum_fill,
        sequence: 0,
    }
    .encode()
    .expect("canonical registered state")
    .to_vec()
}

fn controller_data(controller_bump: u8, fixture: MarketFixture, nonce: u64) -> Vec<u8> {
    ControllerInstructionV1 {
        controller_bump,
        seller_replay_bump: fixture.seller_bump,
        buyer_replay_bump: fixture.buyer_bump,
        seller_position_bump: fixture.seller_position_bump,
        buyer_position_bump: fixture.buyer_position_bump,
        fill: FILL,
        execution_price: PRICE,
        seller: compact_intent(fixture.market, fixture.tokens.seller, 0, nonce),
        buyer: compact_intent(fixture.market, fixture.tokens.source, 1, nonce),
    }
    .encode()
    .expect("fixed controller instruction")
    .to_vec()
}

fn registered_controller_data(controller_bump: u8, fixture: MarketFixture, fill: u64) -> Vec<u8> {
    RegisteredFillInstructionV1 {
        controller_bump,
        seller_registration_bump: fixture.seller_bump,
        buyer_registration_bump: fixture.buyer_bump,
        seller_position_bump: fixture.seller_position_bump,
        buyer_position_bump: fixture.buyer_position_bump,
        fill,
        execution_price: PRICE,
    }
    .encode()
    .expect("registered controller instruction")
    .to_vec()
}

fn registered_create_data(
    controller_bump: u8,
    replay_bump: u8,
    registration_bump: u8,
    intent: CompactIntentV1,
) -> Vec<u8> {
    RegisteredCreateInstructionV1 {
        controller_bump,
        replay_bump,
        registration_bump,
        intent,
    }
    .encode()
    .expect("registered creation instruction")
    .to_vec()
}

fn registered_terminal_data(
    action: RegisteredTerminalAction,
    controller_bump: u8,
    registration_bump: u8,
    expected_sequence: u64,
) -> Vec<u8> {
    RegisteredTerminalInstructionV1 {
        action,
        controller_bump,
        registration_bump,
        expected_sequence,
    }
    .encode()
    .expect("registered terminal instruction")
    .to_vec()
}

fn signed_ed25519_batch(seller: &Keypair, buyer: &Keypair, controller_data: &[u8]) -> Instruction {
    let payload = 2 + 2 * 14;
    let mut data = vec![0_u8; payload + 2 * 96];
    put_u16(&mut data, 0, 2);
    for (index, (maker, message_offset)) in [(seller, 32_usize), (buyer, 168_usize)]
        .into_iter()
        .enumerate()
    {
        let descriptor = 2 + index * 14;
        let public_key_offset = payload + index * 96;
        let signature_offset = public_key_offset + 32;
        put_u16(
            &mut data,
            descriptor,
            u16::try_from(signature_offset).expect("signature offset"),
        );
        put_u16(&mut data, descriptor + 2, u16::MAX);
        put_u16(
            &mut data,
            descriptor + 4,
            u16::try_from(public_key_offset).expect("public-key offset"),
        );
        put_u16(&mut data, descriptor + 6, u16::MAX);
        put_u16(
            &mut data,
            descriptor + 8,
            u16::try_from(message_offset).expect("message offset"),
        );
        put_u16(
            &mut data,
            descriptor + 10,
            u16::try_from(COMPACT_INTENT_BYTES).expect("message length"),
        );
        put_u16(&mut data, descriptor + 12, 1);
        data[public_key_offset..public_key_offset + 32].copy_from_slice(maker.pubkey().as_ref());
        let message = &controller_data[message_offset..message_offset + COMPACT_INTENT_BYTES];
        data[signature_offset..signature_offset + 64]
            .copy_from_slice(maker.sign_message(message).as_ref());
    }
    Instruction {
        program_id: ed25519_program::ID,
        accounts: vec![],
        data,
    }
}

#[allow(clippy::too_many_arguments)]
fn controller_instruction(
    controller: Pubkey,
    journal: Pubkey,
    fixture: MarketFixture,
    mint: Pubkey,
    data: Vec<u8>,
) -> Instruction {
    Instruction {
        program_id: CONTROLLER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(controller, false),
            AccountMeta::new(fixture.seller_replay, false),
            AccountMeta::new(fixture.buyer_replay, false),
            AccountMeta::new(journal, false),
            AccountMeta::new(fixture.seller_position, false),
            AccountMeta::new(fixture.buyer_position, false),
            AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
            AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.market, false),
            AccountMeta::new_readonly(fixture.realm, false),
            AccountMeta::new_readonly(fixture.fee_policy, false),
            AccountMeta::new_readonly(fixture.capability_manifest, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(fixture.tokens.source, false),
            AccountMeta::new(fixture.tokens.seller, false),
            AccountMeta::new(fixture.tokens.venue, false),
            AccountMeta::new_readonly(token_program_id(), false),
            AccountMeta::new_readonly(sysvar::instructions::ID, false),
        ],
        data,
    }
}

fn registered_controller_instruction(
    controller: Pubkey,
    journal: Pubkey,
    fixture: MarketFixture,
    mint: Pubkey,
    data: Vec<u8>,
) -> Instruction {
    Instruction {
        program_id: CONTROLLER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(controller, false),
            AccountMeta::new(fixture.seller_replay, false),
            AccountMeta::new(fixture.buyer_replay, false),
            AccountMeta::new(journal, false),
            AccountMeta::new(fixture.seller_position, false),
            AccountMeta::new(fixture.buyer_position, false),
            AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
            AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.market, false),
            AccountMeta::new_readonly(fixture.realm, false),
            AccountMeta::new_readonly(fixture.fee_policy, false),
            AccountMeta::new_readonly(fixture.capability_manifest, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(fixture.tokens.source, false),
            AccountMeta::new(fixture.tokens.seller, false),
            AccountMeta::new(fixture.tokens.venue, false),
            AccountMeta::new_readonly(token_program_id(), false),
        ],
        data,
    }
}

#[allow(clippy::too_many_arguments)]
fn registered_create_instruction(
    controller: Pubkey,
    maker: Pubkey,
    payer: Pubkey,
    replay: Pubkey,
    registration: Pubkey,
    authority: MarketFixtureAuthority,
    mint: Pubkey,
    collateral: Pubkey,
    venue: Pubkey,
    data: Vec<u8>,
) -> Instruction {
    Instruction {
        program_id: CONTROLLER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(controller, false),
            AccountMeta::new_readonly(maker, true),
            AccountMeta::new(payer, true),
            AccountMeta::new(replay, false),
            AccountMeta::new(registration, false),
            AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(authority.market, false),
            AccountMeta::new_readonly(authority.realm, false),
            AccountMeta::new_readonly(authority.fee_policy, false),
            AccountMeta::new_readonly(authority.capability_manifest, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(collateral, false),
            AccountMeta::new_readonly(venue, false),
            AccountMeta::new_readonly(token_program_id(), false),
        ],
        data,
    }
}

fn token_approve(source: Pubkey, delegate: Pubkey, owner: Pubkey, amount: u64) -> Instruction {
    let mut data = vec![4_u8];
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id: token_program_id(),
        accounts: vec![
            AccountMeta::new(source, false),
            AccountMeta::new_readonly(delegate, false),
            AccountMeta::new_readonly(owner, true),
        ],
        data,
    }
}

fn registered_terminal_instruction(
    controller: Pubkey,
    registration: Pubkey,
    maker: Option<Pubkey>,
    data: Vec<u8>,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new_readonly(controller, false),
        AccountMeta::new(registration, false),
    ];
    if let Some(maker) = maker {
        accounts.push(AccountMeta::new_readonly(maker, true));
    }
    accounts.push(AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false));
    Instruction {
        program_id: CONTROLLER_PROGRAM_ID,
        accounts,
        data,
    }
}

fn direct_claim_instruction(controller: Pubkey, fixture: MarketFixture) -> Instruction {
    let mut plan = vec![0_u8; 72];
    plan[0..8].copy_from_slice(&[b'D', b'C', b'E', b'F', 1, 4, 0, 0]);
    Instruction {
        program_id: CLAIM_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(controller, false),
            AccountMeta::new(fixture.seller_replay, false),
            AccountMeta::new(fixture.buyer_replay, false),
            AccountMeta::new(fixture.seller_position, false),
            AccountMeta::new(fixture.buyer_position, false),
        ],
        data: plan,
    }
}

fn reusable_market_lookup_addresses(instructions: &[Instruction]) -> Option<Vec<Pubkey>> {
    let instruction = instructions.iter().find(|instruction| {
        instruction.program_id == CONTROLLER_PROGRAM_ID && instruction.accounts.len() == 18
    })?;
    let mut addresses = Vec::with_capacity(12);
    for index in [0_usize, 3, 6, 7, 8, 9, 10, 11, 12, 15, 16, 17] {
        addresses.push(instruction.accounts.get(index)?.pubkey);
    }
    Some(addresses)
}

async fn submit(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
) -> TransactionResult {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let wire_bytes = 1_usize
        .checked_add(transaction.signatures.len().saturating_mul(64))
        .and_then(|prefix| prefix.checked_add(transaction.message_data().len()))
        .expect("legacy transaction wire size");
    let mut lookup_addresses = Vec::new();
    for instruction in instructions {
        for meta in &instruction.accounts {
            if meta.pubkey != context.payer.pubkey() && !lookup_addresses.contains(&meta.pubkey) {
                lookup_addresses.push(meta.pubkey);
            }
        }
    }
    let market_lookup_addresses =
        reusable_market_lookup_addresses(instructions).unwrap_or_else(|| lookup_addresses.clone());
    let v0_wire_bytes = versioned_wire_bytes(
        context.payer.pubkey(),
        instructions,
        blockhash,
        lookup_addresses,
        91,
    );
    let market_v0_wire_bytes = versioned_wire_bytes(
        context.payer.pubkey(),
        instructions,
        blockhash,
        market_lookup_addresses,
        92,
    );
    assert!(v0_wire_bytes <= 1_232, "v0 transaction packet overflow");
    assert!(
        market_v0_wire_bytes <= 1_232,
        "reusable Market-table v0 transaction packet overflow"
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("banks processing");
    let metadata = processed.metadata.expect("transaction metadata");
    TransactionResult {
        accepted: processed.result.is_ok(),
        compute_units: metadata.compute_units_consumed,
        wire_bytes,
        v0_wire_bytes,
        market_v0_wire_bytes,
        logs: metadata.log_messages,
    }
}

async fn submit_terminal(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    maker: Option<&Keypair>,
) -> (bool, u64) {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = match maker {
        Some(maker) => Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, maker],
            blockhash,
        ),
        None => Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            blockhash,
        ),
    };
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("terminal transaction processing");
    (
        processed.result.is_ok(),
        processed
            .metadata
            .expect("terminal transaction metadata")
            .compute_units_consumed,
    )
}

async fn submit_registered_create(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    maker: &Keypair,
) -> (bool, u64) {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &[&context.payer, maker],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("registration transaction processing");
    (
        processed.result.is_ok(),
        processed
            .metadata
            .expect("registration transaction metadata")
            .compute_units_consumed,
    )
}

async fn process_legacy(context: &mut ProgramTestContext, instructions: &[Instruction]) -> u64 {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("lookup-table lifecycle transaction");
    assert!(
        processed.result.is_ok(),
        "lookup-table lifecycle transaction must commit"
    );
    processed
        .metadata
        .expect("lookup-table lifecycle metadata")
        .compute_units_consumed
}

async fn create_reusable_market_lookup_table(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
) -> (Pubkey, Vec<Pubkey>, [u64; 2]) {
    let addresses = reusable_market_lookup_addresses(instructions)
        .expect("compiled Direct Market lookup projection");
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock sysvar");
    context
        .warp_to_slot(clock.slot + 1)
        .expect("make the creation coordinate recent in SlotHashes");
    let payer = context.payer.pubkey();
    let (create, lookup_table) = create_lookup_table(payer, payer, clock.slot);
    let create_cu = process_legacy(context, &[create]).await;
    let extend = extend_lookup_table(lookup_table, payer, Some(payer), addresses.clone());
    let extend_cu = process_legacy(context, &[extend]).await;
    let extension_clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("post-extension Clock sysvar");
    context
        .warp_to_slot(extension_clock.slot + 1)
        .expect("activate lookup-table additions in the next slot");
    assert!(
        context
            .banks_client
            .get_account(lookup_table)
            .await
            .expect("lookup-table query")
            .is_some(),
        "created lookup table must exist"
    );
    (lookup_table, addresses, [create_cu, extend_cu])
}

async fn submit_with_live_market_lookup_table(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    lookup_table: Pubkey,
    market_lookup_addresses: Vec<Pubkey>,
) -> TransactionResult {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let legacy = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let wire_bytes = 1_usize
        .checked_add(legacy.signatures.len().saturating_mul(64))
        .and_then(|prefix| prefix.checked_add(legacy.message_data().len()))
        .expect("legacy transaction wire size");
    let mut all_lookup_addresses = Vec::new();
    for instruction in instructions {
        for meta in &instruction.accounts {
            if meta.pubkey != context.payer.pubkey() && !all_lookup_addresses.contains(&meta.pubkey)
            {
                all_lookup_addresses.push(meta.pubkey);
            }
        }
    }
    let v0_wire_bytes = versioned_wire_bytes(
        context.payer.pubkey(),
        instructions,
        blockhash,
        all_lookup_addresses,
        91,
    );
    let lookup = AddressLookupTableAccount {
        key: lookup_table,
        addresses: market_lookup_addresses,
    };
    let v0_message =
        v0::Message::try_compile(&context.payer.pubkey(), instructions, &[lookup], blockhash)
            .expect("live-table v0 message");
    let message = VersionedMessage::V0(v0_message);
    let market_v0_wire_bytes = 1_usize
        .checked_add(64)
        .and_then(|prefix| prefix.checked_add(message.serialize().len()))
        .expect("live-table v0 wire size");
    assert!(market_v0_wire_bytes <= 1_232, "live v0 packet overflow");
    let transaction = VersionedTransaction {
        signatures: vec![context.payer.sign_message(&message.serialize())],
        message,
    };
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("banks processing");
    let metadata = processed.metadata.expect("transaction metadata");
    TransactionResult {
        accepted: processed.result.is_ok(),
        compute_units: metadata.compute_units_consumed,
        wire_bytes,
        v0_wire_bytes,
        market_v0_wire_bytes,
        logs: metadata.log_messages,
    }
}

async fn retire_reusable_market_lookup_table(
    context: &mut ProgramTestContext,
    lookup_table: Pubkey,
) -> [u64; 2] {
    let payer = context.payer.pubkey();
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock sysvar");
    let deactivate_cu =
        process_legacy(context, &[deactivate_lookup_table(lookup_table, payer)]).await;
    let close_slot = estimate_last_valid_slot(clock.slot) + 1;
    let mut next_slot = clock.slot + 1;
    while next_slot <= close_slot {
        context
            .warp_to_slot(next_slot)
            .expect("advance one lookup-table cooldown slot");
        next_slot += 1;
    }
    let close_cu = process_legacy(context, &[close_lookup_table(lookup_table, payer, payer)]).await;
    assert!(
        context
            .banks_client
            .get_account(lookup_table)
            .await
            .expect("closed lookup-table query")
            .is_none(),
        "closed lookup table must be reclaimed"
    );
    [deactivate_cu, close_cu]
}

fn versioned_wire_bytes(
    payer: Pubkey,
    instructions: &[Instruction],
    blockhash: solana_program::hash::Hash,
    lookup_addresses: Vec<Pubkey>,
    table_key_byte: u8,
) -> usize {
    let lookup = AddressLookupTableAccount {
        key: Pubkey::new_from_array([table_key_byte; 32]),
        addresses: lookup_addresses,
    };
    let v0_message = v0::Message::try_compile(&payer, instructions, &[lookup], blockhash)
        .expect("one-table v0 message");
    1_usize
        .checked_add(64)
        .and_then(|prefix| prefix.checked_add(VersionedMessage::V0(v0_message).serialize().len()))
        .expect("v0 transaction wire size")
}

async fn account(context: &mut ProgramTestContext, address: Pubkey) -> Account {
    context
        .banks_client
        .get_account(address)
        .await
        .expect("query")
        .expect("account")
}

async fn claim_state_accounts(
    context: &mut ProgramTestContext,
    fixture: MarketFixture,
) -> [Account; 4] {
    [
        account(context, fixture.seller_replay).await,
        account(context, fixture.buyer_replay).await,
        account(context, fixture.seller_position).await,
        account(context, fixture.buyer_position).await,
    ]
}

fn mint(supply: u64, decimals: u8) -> Vec<u8> {
    let mut bytes = vec![0_u8; MINT_BYTES];
    put_u64(&mut bytes, 36, supply);
    bytes[44] = decimals;
    bytes[45] = 1;
    bytes
}

fn token_account(
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    delegate: Option<(Pubkey, u64)>,
    frozen: bool,
) -> Vec<u8> {
    let mut bytes = vec![0_u8; TOKEN_ACCOUNT_BYTES];
    bytes[0..32].copy_from_slice(mint.as_ref());
    bytes[32..64].copy_from_slice(owner.as_ref());
    put_u64(&mut bytes, 64, amount);
    if let Some((delegate, allowance)) = delegate {
        put_u32(&mut bytes, 72, 1);
        bytes[76..108].copy_from_slice(delegate.as_ref());
        put_u64(&mut bytes, 121, allowance);
    }
    bytes[108] = if frozen { 2 } else { 1 };
    bytes
}

fn add_program_account(test: &mut ProgramTest, key: Pubkey) {
    test.add_account(key, Account::new(1_000_000, 0, &system_program::ID));
}

fn add_claim_state(test: &mut ProgramTest, key: Pubkey, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: CLAIM_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_token_account(test: &mut ProgramTest, key: Pubkey, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(TOKEN_ACCOUNT_BYTES),
            data,
            owner: token_program_id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_protocol_account(test: &mut ProgramTest, key: Pubkey, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: PROTOCOL_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn market_fixture(
    authority: MarketFixtureAuthority,
    seller: Pubkey,
    buyer: Pubkey,
    tokens: TokenTriplet,
) -> MarketFixture {
    let generation = GENERATION.to_le_bytes();
    let (seller_replay, seller_bump) = Pubkey::find_program_address(
        &[
            REPLAY_SEED,
            authority.market.as_ref(),
            &generation,
            seller.as_ref(),
        ],
        &CONTROLLER_PROGRAM_ID,
    );
    let (buyer_replay, buyer_bump) = Pubkey::find_program_address(
        &[
            REPLAY_SEED,
            authority.market.as_ref(),
            &generation,
            buyer.as_ref(),
        ],
        &CONTROLLER_PROGRAM_ID,
    );
    let outcome = [1_u8];
    let (seller_position, seller_position_bump) = Pubkey::find_program_address(
        &[
            POSITION_SEED,
            authority.market.as_ref(),
            seller.as_ref(),
            &outcome,
        ],
        &CONTROLLER_PROGRAM_ID,
    );
    let (buyer_position, buyer_position_bump) = Pubkey::find_program_address(
        &[
            POSITION_SEED,
            authority.market.as_ref(),
            buyer.as_ref(),
            &outcome,
        ],
        &CONTROLLER_PROGRAM_ID,
    );
    MarketFixture {
        market: authority.market,
        realm: authority.realm,
        fee_policy: authority.fee_policy,
        capability_manifest: authority.capability_manifest,
        seller_replay,
        seller_bump,
        buyer_replay,
        buyer_bump,
        seller_position,
        seller_position_bump,
        buyer_position,
        buyer_position_bump,
        tokens,
    }
}

fn registered_market_fixture(
    authority: MarketFixtureAuthority,
    seller: Pubkey,
    buyer: Pubkey,
    tokens: TokenTriplet,
) -> MarketFixture {
    let generation = GENERATION.to_le_bytes();
    let nonce = 0_u64.to_le_bytes();
    let (seller_registration, seller_bump) = Pubkey::find_program_address(
        &[
            REGISTERED_SEED,
            authority.market.as_ref(),
            &generation,
            seller.as_ref(),
            &nonce,
        ],
        &CONTROLLER_PROGRAM_ID,
    );
    let (buyer_registration, buyer_bump) = Pubkey::find_program_address(
        &[
            REGISTERED_SEED,
            authority.market.as_ref(),
            &generation,
            buyer.as_ref(),
            &nonce,
        ],
        &CONTROLLER_PROGRAM_ID,
    );
    let outcome = [1_u8];
    let (seller_position, seller_position_bump) = Pubkey::find_program_address(
        &[
            POSITION_SEED,
            authority.market.as_ref(),
            seller.as_ref(),
            &outcome,
        ],
        &CONTROLLER_PROGRAM_ID,
    );
    let (buyer_position, buyer_position_bump) = Pubkey::find_program_address(
        &[
            POSITION_SEED,
            authority.market.as_ref(),
            buyer.as_ref(),
            &outcome,
        ],
        &CONTROLLER_PROGRAM_ID,
    );
    MarketFixture {
        market: authority.market,
        realm: authority.realm,
        fee_policy: authority.fee_policy,
        capability_manifest: authority.capability_manifest,
        seller_replay: seller_registration,
        seller_bump,
        buyer_replay: buyer_registration,
        buyer_bump,
        seller_position,
        seller_position_bump,
        buyer_position,
        buyer_position_bump,
        tokens,
    }
}

fn write_validator_account(
    directory: &Path,
    address: Pubkey,
    owner: Pubkey,
    data: &[u8],
    lamports: u64,
) {
    let account = serde_json::json!({
        "pubkey": address.to_string(),
        "account": {
            "lamports": lamports,
            "data": [BASE64.encode(data), "base64"],
            "owner": owner.to_string(),
            "executable": false,
            "rentEpoch": 0,
            "space": data.len(),
        }
    });
    let path = directory.join(format!("{address}.json"));
    fs::write(
        path,
        serde_json::to_vec_pretty(&account).expect("serialize validator account"),
    )
    .expect("write validator account");
}

fn local_port_block() -> u16 {
    for base in (19_000_u16..29_000).step_by(128) {
        let probes = [base, base + 1, base + 2, base + 3];
        let listeners = probes
            .into_iter()
            .map(|port| TcpListener::bind(("127.0.0.1", port)))
            .collect::<Result<Vec<_>, _>>();
        if listeners.is_ok() {
            return base;
        }
    }
    panic!("no local validator port block is available");
}

struct ValidatorProcess {
    child: Child,
}

impl Drop for ValidatorProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wait_until(label: &str, timeout: Duration, mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if predicate() {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("timed out waiting for {label}");
}

fn wait_for_validator(
    validator: &mut ValidatorProcess,
    rpc: &RpcClient,
    log_path: &Path,
    timeout: Duration,
) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if rpc.get_health().is_ok() {
            return;
        }
        if let Some(status) = validator
            .child
            .try_wait()
            .expect("validator process status")
        {
            let log = fs::read_to_string(log_path).unwrap_or_else(|error| error.to_string());
            panic!("local validator exited with {status}:\n{log}");
        }
        thread::sleep(Duration::from_millis(50));
    }
    let log = fs::read_to_string(log_path).unwrap_or_else(|error| error.to_string());
    panic!("timed out waiting for local validator RPC:\n{log}");
}

fn send_legacy_rpc(rpc: &RpcClient, payer: &Keypair, instructions: &[Instruction]) {
    let blockhash = rpc.get_latest_blockhash().expect("RPC blockhash");
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&payer.pubkey()),
        &[payer],
        blockhash,
    );
    let signature = rpc
        .send_transaction(&transaction)
        .expect("submitted legacy RPC transaction");
    wait_until(
        "processed legacy RPC transaction",
        Duration::from_secs(5),
        || matches!(rpc.get_signature_status(&signature), Ok(Some(_))),
    );
    rpc.get_signature_status(&signature)
        .expect("legacy RPC status query")
        .expect("legacy RPC status")
        .expect("successful legacy RPC transaction");
}

fn newest_slot_hash(rpc: &RpcClient) -> Option<u64> {
    let data = rpc.get_account(&sysvar::slot_hashes::ID).ok()?.data;
    if data.len() < 16 || read_u64(&data, 0) == 0 {
        return None;
    }
    Some(read_u64(&data, 8))
}

#[test]
#[ignore = "spawns solana-test-validator; run as the explicit transport campaign"]
fn compiled_direct_crosses_the_local_validator_rpc_boundary() {
    require_sbf();
    let sbf_directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR"));
    let seller_maker = keypair_from_seed(&[41_u8; 32]).expect("deterministic seller fixture");
    let buyer_maker = keypair_from_seed(&[42_u8; 32]).expect("deterministic buyer fixture");
    let (controller, controller_bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &CONTROLLER_PROGRAM_ID);
    let journal_key = Pubkey::new_from_array([131_u8; 32]);
    let mint_key = Pubkey::new_from_array([132_u8; 32]);
    let tokens = TokenTriplet {
        source: Pubkey::new_from_array([133_u8; 32]),
        seller: Pubkey::new_from_array([134_u8; 32]),
        venue: Pubkey::new_from_array([135_u8; 32]),
    };
    let (authority, market_bytes, realm_bytes, policy_bytes, manifest_bytes) =
        authority_records(mint_key, tokens.venue);
    let fixture = market_fixture(
        authority,
        seller_maker.pubkey(),
        buyer_maker.pubkey(),
        tokens,
    );
    let controller_bytes = controller_data(controller_bump, fixture, 0);
    let direct_instructions = [
        signed_ed25519_batch(&seller_maker, &buyer_maker, &controller_bytes),
        controller_instruction(controller, journal_key, fixture, mint_key, controller_bytes),
    ];
    let lookup_addresses = reusable_market_lookup_addresses(&direct_instructions)
        .expect("reusable Market address projection");

    let payer = Keypair::new();
    let temporary = tempfile::tempdir().expect("validator temporary directory");
    let account_directory = temporary.path().join("accounts");
    fs::create_dir(&account_directory).expect("validator account directory");
    write_validator_account(
        &account_directory,
        controller,
        system_program::ID,
        &[],
        1_000_000,
    );
    write_validator_account(
        &account_directory,
        payer.pubkey(),
        system_program::ID,
        &[],
        10_000_000_000,
    );
    for (address, data) in [
        (fixture.seller_replay, replay_state(controller, 0)),
        (fixture.buyer_replay, replay_state(controller, 0)),
        (
            fixture.seller_position,
            position_state(controller, 1, 5_000),
        ),
        (fixture.buyer_position, position_state(controller, 1, 200)),
    ] {
        write_validator_account(
            &account_directory,
            address,
            CLAIM_PROGRAM_ID,
            &data,
            10_000_000,
        );
    }
    write_validator_account(
        &account_directory,
        journal_key,
        CONTROLLER_PROGRAM_ID,
        &journal(0),
        10_000_000,
    );
    for (address, data) in [
        (authority.market, market_bytes),
        (authority.realm, realm_bytes),
        (authority.fee_policy, policy_bytes),
        (authority.capability_manifest, manifest_bytes),
    ] {
        write_validator_account(
            &account_directory,
            address,
            PROTOCOL_PROGRAM_ID,
            &data,
            10_000_000,
        );
    }
    write_validator_account(
        &account_directory,
        mint_key,
        token_program_id(),
        &mint(40_000, 6),
        10_000_000,
    );
    for (address, data) in [
        (
            tokens.source,
            token_account(
                mint_key,
                buyer_maker.pubkey(),
                2_000,
                Some((fixture.buyer_replay, 1_002)),
                false,
            ),
        ),
        (
            tokens.seller,
            token_account(mint_key, seller_maker.pubkey(), 100, None, false),
        ),
        (
            tokens.venue,
            token_account(
                mint_key,
                Pubkey::new_from_array([136_u8; 32]),
                20,
                None,
                false,
            ),
        ),
    ] {
        write_validator_account(
            &account_directory,
            address,
            token_program_id(),
            &data,
            10_000_000,
        );
    }

    let base_port = local_port_block();
    let validator_binary =
        env::var_os("SOLANA_TEST_VALIDATOR").unwrap_or_else(|| "solana-test-validator".into());
    let log_path = temporary.path().join("validator.log");
    let log = fs::File::create(&log_path).expect("validator log");
    let child = Command::new(validator_binary)
        .arg("--ledger")
        .arg(temporary.path().join("ledger"))
        .args(["--reset", "--quiet"])
        .arg("--rpc-port")
        .arg(base_port.to_string())
        .arg("--faucet-port")
        .arg((base_port + 2).to_string())
        .arg("--gossip-port")
        .arg((base_port + 3).to_string())
        .arg("--dynamic-port-range")
        .arg(format!("{}-{}", base_port + 10, base_port + 110))
        .arg("--account-dir")
        .arg(&account_directory)
        .arg("--bpf-program")
        .arg(CONTROLLER_PROGRAM_ID.to_string())
        .arg(sbf_directory.join("dclutch_controller_proof_sbf.so"))
        .arg("--bpf-program")
        .arg(CLAIM_PROGRAM_ID.to_string())
        .arg(sbf_directory.join("dclutch_claims_proof_sbf.so"))
        .arg("--bpf-program")
        .arg(CUSTODY_PROGRAM_ID.to_string())
        .arg(sbf_directory.join("dclutch_custody_proof_sbf.so"))
        .stdout(Stdio::from(log.try_clone().expect("clone validator log")))
        .stderr(Stdio::from(log))
        .spawn()
        .expect("spawn solana-test-validator");
    let mut validator = ValidatorProcess { child };
    let rpc = RpcClient::new_with_commitment(
        format!("http://127.0.0.1:{base_port}"),
        CommitmentConfig::processed(),
    );
    wait_for_validator(&mut validator, &rpc, &log_path, Duration::from_secs(30));

    assert_eq!(
        rpc.get_balance(&payer.pubkey())
            .expect("genesis payer balance"),
        10_000_000_000
    );

    wait_until("first slot hash", Duration::from_secs(10), || {
        newest_slot_hash(&rpc).is_some()
    });
    let recent_slot = newest_slot_hash(&rpc).expect("recent SlotHashes entry");
    let (create, lookup_table) = create_lookup_table(payer.pubkey(), payer.pubkey(), recent_slot);
    send_legacy_rpc(&rpc, &payer, &[create]);
    send_legacy_rpc(
        &rpc,
        &payer,
        &[extend_lookup_table(
            lookup_table,
            payer.pubkey(),
            Some(payer.pubkey()),
            lookup_addresses.clone(),
        )],
    );
    let table_account = rpc
        .get_account(&lookup_table)
        .expect("created lookup-table account");
    let table =
        LookupTableState::deserialize(&table_account.data).expect("decode created lookup table");
    assert_eq!(table.addresses.as_ref(), lookup_addresses.as_slice());
    let extension_slot = table.meta.last_extended_slot;
    wait_until(
        "lookup-table activation slot",
        Duration::from_secs(10),
        || rpc.get_slot().unwrap_or_default() > extension_slot,
    );

    let blockhash = rpc.get_latest_blockhash().expect("Direct v0 blockhash");
    let lookup = AddressLookupTableAccount {
        key: lookup_table,
        addresses: lookup_addresses,
    };
    let message = VersionedMessage::V0(
        v0::Message::try_compile(&payer.pubkey(), &direct_instructions, &[lookup], blockhash)
            .expect("compile external-validator Direct v0 message"),
    );
    let wire_bytes = 1_usize + 64 + message.serialize().len();
    assert_eq!(wire_bytes, 990, "canonical Direct v0 wire size drift");
    let transaction = VersionedTransaction {
        signatures: vec![payer.sign_message(&message.serialize())],
        message,
    };
    let direct_signature = rpc
        .send_and_confirm_transaction(&transaction)
        .expect("confirmed external-validator Direct v0 fill");

    assert_eq!(read_u64(&rpc.get_account(&journal_key).unwrap().data, 8), 1);
    assert_eq!(
        read_u64(&rpc.get_account(&fixture.seller_replay).unwrap().data, 40),
        1
    );
    assert_eq!(
        read_u64(&rpc.get_account(&fixture.buyer_replay).unwrap().data, 40),
        1
    );
    assert_eq!(
        read_u64(&rpc.get_account(&fixture.seller_position).unwrap().data, 48),
        3_000
    );
    assert_eq!(
        read_u64(&rpc.get_account(&fixture.buyer_position).unwrap().data, 48),
        2_200
    );
    assert_eq!(
        read_u64(&rpc.get_account(&tokens.source).unwrap().data, 64),
        998
    );
    assert_eq!(
        read_u64(&rpc.get_account(&tokens.seller).unwrap().data, 64),
        1_100
    );
    assert_eq!(
        read_u64(&rpc.get_account(&tokens.venue).unwrap().data, 64),
        22
    );

    send_legacy_rpc(
        &rpc,
        &payer,
        &[deactivate_lookup_table(lookup_table, payer.pubkey())],
    );
    let table_account = rpc
        .get_account(&lookup_table)
        .expect("deactivating lookup-table account");
    let table = LookupTableState::deserialize(&table_account.data)
        .expect("decode deactivating lookup table");
    assert_ne!(table.meta.deactivation_slot, u64::MAX);
    eprintln!(
        "external validator Direct: signature={direct_signature}, wire={wire_bytes}, ALT={lookup_table}, deactivation_slot={} (ledger removed on process exit; full 512-slot close is covered by ProgramTest)",
        table.meta.deactivation_slot
    );
}

#[tokio::test]
async fn signed_intents_compile_to_claims_and_real_token_transfers_atomically() {
    require_sbf();
    let seller_maker = Keypair::new();
    let buyer_maker = Keypair::new();
    let (controller, controller_bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &CONTROLLER_PROGRAM_ID);
    let journal_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let success_tokens = TokenTriplet {
        source: Pubkey::new_unique(),
        seller: Pubkey::new_unique(),
        venue: Pubkey::new_unique(),
    };
    let refusal_tokens = TokenTriplet {
        source: Pubkey::new_unique(),
        seller: Pubkey::new_unique(),
        venue: Pubkey::new_unique(),
    };
    let (success_authority, success_market, realm_bytes, success_policy, success_manifest) =
        authority_records(mint_key, success_tokens.venue);
    let (refusal_authority, refusal_market, refusal_realm, refusal_policy, refusal_manifest) =
        authority_records(mint_key, refusal_tokens.venue);
    assert_eq!(success_authority.realm, refusal_authority.realm);
    assert_eq!(realm_bytes, refusal_realm);
    let success = market_fixture(
        success_authority,
        seller_maker.pubkey(),
        buyer_maker.pubkey(),
        success_tokens,
    );
    let refusal = market_fixture(
        refusal_authority,
        seller_maker.pubkey(),
        buyer_maker.pubkey(),
        refusal_tokens,
    );

    let mut test = ProgramTest::new("dclutch_controller_proof_sbf", CONTROLLER_PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_program("dclutch_claims_proof_sbf", CLAIM_PROGRAM_ID, None);
    test.add_program("dclutch_custody_proof_sbf", CUSTODY_PROGRAM_ID, None);
    test.add_program("spl_token", token_program_id(), None);
    add_program_account(&mut test, controller);
    for fixture in [success, refusal] {
        add_claim_state(
            &mut test,
            fixture.seller_replay,
            replay_state(controller, 0),
        );
        add_claim_state(&mut test, fixture.buyer_replay, replay_state(controller, 0));
        add_claim_state(
            &mut test,
            fixture.seller_position,
            position_state(controller, 1, 5_000),
        );
        add_claim_state(
            &mut test,
            fixture.buyer_position,
            position_state(controller, 1, 200),
        );
    }
    test.add_account(
        journal_key,
        Account {
            lamports: Rent::default().minimum_balance(JOURNAL_BYTES),
            data: journal(0),
            owner: CONTROLLER_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    add_protocol_account(&mut test, success_authority.realm, realm_bytes);
    for (authority, market, policy, manifest) in [
        (
            success_authority,
            success_market,
            success_policy,
            success_manifest,
        ),
        (
            refusal_authority,
            refusal_market,
            refusal_policy,
            refusal_manifest,
        ),
    ] {
        add_protocol_account(&mut test, authority.market, market);
        add_protocol_account(&mut test, authority.fee_policy, policy);
        add_protocol_account(&mut test, authority.capability_manifest, manifest);
    }
    test.add_account(
        mint_key,
        Account {
            lamports: Rent::default().minimum_balance(MINT_BYTES),
            data: mint(40_000, 6),
            owner: token_program_id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    for (fixture, frozen) in [(success, false), (refusal, true)] {
        add_token_account(
            &mut test,
            fixture.tokens.source,
            token_account(
                mint_key,
                buyer_maker.pubkey(),
                2_000,
                Some((fixture.buyer_replay, 1_002)),
                false,
            ),
        );
        add_token_account(
            &mut test,
            fixture.tokens.seller,
            token_account(mint_key, seller_maker.pubkey(), 100, None, false),
        );
        add_token_account(
            &mut test,
            fixture.tokens.venue,
            token_account(mint_key, Pubkey::new_unique(), 20, None, frozen),
        );
    }
    let mut context = test.start_with_context().await;

    let direct = submit(
        &mut context,
        &[direct_claim_instruction(controller, success)],
    )
    .await;
    assert!(
        !direct.accepted,
        "transaction caller cannot sign controller PDA"
    );

    let untouched_journal = account(&mut context, journal_key).await;
    let untouched_claims = claim_state_accounts(&mut context, success).await;
    let untouched_source = account(&mut context, success.tokens.source).await;
    let untouched_seller = account(&mut context, success.tokens.seller).await;
    let untouched_venue = account(&mut context, success.tokens.venue).await;

    let mut wrong_data = controller_data(controller_bump, success, 0);
    wrong_data[12] = success.buyer_bump.wrapping_add(1);
    let wrong = controller_instruction(
        controller,
        journal_key,
        success,
        mint_key,
        wrong_data.clone(),
    );
    let wrong_bump = submit(
        &mut context,
        &[
            signed_ed25519_batch(&seller_maker, &buyer_maker, &wrong_data),
            wrong,
        ],
    )
    .await;
    assert!(!wrong_bump.accepted, "wrong replay coordinate must refuse");
    assert_eq!(account(&mut context, journal_key).await, untouched_journal);
    assert_eq!(
        claim_state_accounts(&mut context, success).await,
        untouched_claims
    );
    assert_eq!(
        account(&mut context, success.tokens.source).await,
        untouched_source
    );
    assert_eq!(
        account(&mut context, success.tokens.seller).await,
        untouched_seller
    );
    assert_eq!(
        account(&mut context, success.tokens.venue).await,
        untouched_venue
    );

    let mut wrong_position_data = controller_data(controller_bump, success, 0);
    wrong_position_data[14] = success.buyer_position_bump.wrapping_add(1);
    let wrong_position = submit(
        &mut context,
        &[
            signed_ed25519_batch(&seller_maker, &buyer_maker, &wrong_position_data),
            controller_instruction(
                controller,
                journal_key,
                success,
                mint_key,
                wrong_position_data,
            ),
        ],
    )
    .await;
    assert!(
        !wrong_position.accepted,
        "wrong maker/outcome Position coordinate must refuse"
    );
    assert_eq!(
        claim_state_accounts(&mut context, success).await,
        untouched_claims
    );
    assert_eq!(account(&mut context, journal_key).await, untouched_journal);

    let substituted_authority = MarketFixture {
        capability_manifest: refusal.capability_manifest,
        ..success
    };
    let authority_data = controller_data(controller_bump, success, 0);
    let wrong_authority = submit(
        &mut context,
        &[
            signed_ed25519_batch(&seller_maker, &buyer_maker, &authority_data),
            controller_instruction(
                controller,
                journal_key,
                substituted_authority,
                mint_key,
                authority_data,
            ),
        ],
    )
    .await;
    assert!(
        !wrong_authority.accepted,
        "same-shape manifest from another Market must refuse"
    );
    assert_eq!(account(&mut context, journal_key).await, untouched_journal);
    assert_eq!(
        claim_state_accounts(&mut context, success).await,
        untouched_claims
    );

    let mut bad_price_data = controller_data(controller_bump, success, 0);
    put_u64(&mut bad_price_data, 24, 399_999);
    let bad_price = submit(
        &mut context,
        &[
            signed_ed25519_batch(&seller_maker, &buyer_maker, &bad_price_data),
            controller_instruction(controller, journal_key, success, mint_key, bad_price_data),
        ],
    )
    .await;
    assert!(
        !bad_price.accepted,
        "matcher price below the signed seller limit must refuse"
    );
    assert_eq!(account(&mut context, journal_key).await, untouched_journal);
    assert_eq!(
        claim_state_accounts(&mut context, success).await,
        untouched_claims
    );
    assert_eq!(
        account(&mut context, success.tokens.source).await,
        untouched_source
    );

    let signed_data = controller_data(controller_bump, success, 0);
    let signature_batch = signed_ed25519_batch(&seller_maker, &buyer_maker, &signed_data);
    let mut tampered_data = signed_data;
    tampered_data[32 + 96] ^= 1;
    let tampered = submit(
        &mut context,
        &[
            signature_batch,
            controller_instruction(controller, journal_key, success, mint_key, tampered_data),
        ],
    )
    .await;
    assert!(
        !tampered.accepted,
        "mutating a signed fee-rate byte must fail native Ed25519 verification"
    );
    assert_eq!(account(&mut context, journal_key).await, untouched_journal);
    assert_eq!(
        claim_state_accounts(&mut context, success).await,
        untouched_claims
    );
    assert_eq!(
        account(&mut context, success.tokens.source).await,
        untouched_source
    );

    let success_data = controller_data(controller_bump, success, 0);
    let success_instructions = [
        signed_ed25519_batch(&seller_maker, &buyer_maker, &success_data),
        controller_instruction(controller, journal_key, success, mint_key, success_data),
    ];
    let (lookup_table, market_lookup_addresses, lookup_creation_cu) =
        create_reusable_market_lookup_table(&mut context, &success_instructions).await;
    let success_result = submit_with_live_market_lookup_table(
        &mut context,
        &success_instructions,
        lookup_table,
        market_lookup_addresses,
    )
    .await;
    assert!(
        success_result.accepted,
        "compiled physical fill must commit"
    );
    assert_eq!(
        read_u64(&account(&mut context, journal_key).await.data, 8),
        1
    );
    let claims = claim_state_accounts(&mut context, success).await;
    assert_eq!(read_u64(&claims[0].data, 40), 1);
    assert_eq!(read_u64(&claims[1].data, 40), 1);
    assert_eq!(read_u64(&claims[2].data, 48), 3_000);
    assert_eq!(read_u64(&claims[3].data, 48), 2_200);
    let source = account(&mut context, success.tokens.source).await;
    assert_eq!(read_u64(&source.data, 64), 998);
    assert_eq!(read_u64(&source.data, 121), 0);
    assert_eq!(read_u32(&source.data, 72), 0);
    assert_eq!(
        read_u64(&account(&mut context, success.tokens.seller).await.data, 64),
        1_100
    );
    assert_eq!(
        read_u64(&account(&mut context, success.tokens.venue).await.data, 64),
        22
    );

    let journal_before = account(&mut context, journal_key).await;
    let claim_before = claim_state_accounts(&mut context, refusal).await;
    let source_before = account(&mut context, refusal.tokens.source).await;
    let seller_before = account(&mut context, refusal.tokens.seller).await;
    let venue_before = account(&mut context, refusal.tokens.venue).await;
    let refusal_data = controller_data(controller_bump, refusal, 0);
    let late_refusal = submit(
        &mut context,
        &[
            signed_ed25519_batch(&seller_maker, &buyer_maker, &refusal_data),
            controller_instruction(controller, journal_key, refusal, mint_key, refusal_data),
        ],
    )
    .await;
    assert!(
        !late_refusal.accepted,
        "frozen venue must refuse after gross CPI"
    );
    let token_success = format!("Program {} success", token_program_id());
    assert!(
        late_refusal.logs.iter().any(|line| line == &token_success),
        "logs must prove first official Token CPI completed before refusal"
    );
    assert_eq!(account(&mut context, journal_key).await, journal_before);
    assert_eq!(
        claim_state_accounts(&mut context, refusal).await,
        claim_before
    );
    assert_eq!(
        account(&mut context, refusal.tokens.source).await,
        source_before
    );
    assert_eq!(
        account(&mut context, refusal.tokens.seller).await,
        seller_before
    );
    assert_eq!(
        account(&mut context, refusal.tokens.venue).await,
        venue_before
    );
    let lookup_retirement_cu =
        retire_reusable_market_lookup_table(&mut context, lookup_table).await;

    eprintln!(
        "compiled signed Direct CU: impersonation={}, wrong replay={}, wrong position={}, wrong authority={}, bad price={}, tamper={}, success={}, late rollback={}; success wire: legacy={} bytes, all-address v0={} bytes, reusable-Market v0={} bytes; live ALT CU: create={}, extend={}, deactivate={}, close={}",
        direct.compute_units,
        wrong_bump.compute_units,
        wrong_position.compute_units,
        wrong_authority.compute_units,
        bad_price.compute_units,
        tampered.compute_units,
        success_result.compute_units,
        late_refusal.compute_units,
        success_result.wire_bytes,
        success_result.v0_wire_bytes,
        success_result.market_v0_wire_bytes,
        lookup_creation_cu[0],
        lookup_creation_cu[1],
        lookup_retirement_cu[0],
        lookup_retirement_cu[1],
    );
}

#[tokio::test]
async fn registered_residuals_reuse_authenticated_state_and_real_custody_atomically() {
    require_sbf();
    let seller_maker = Keypair::new();
    let buyer_maker = Keypair::new();
    let (controller, controller_bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &CONTROLLER_PROGRAM_ID);
    let journal_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let success_tokens = TokenTriplet {
        source: Pubkey::new_unique(),
        seller: Pubkey::new_unique(),
        venue: Pubkey::new_unique(),
    };
    let refusal_tokens = TokenTriplet {
        source: Pubkey::new_unique(),
        seller: Pubkey::new_unique(),
        venue: Pubkey::new_unique(),
    };
    let (success_authority, success_market, realm_bytes, success_policy, success_manifest) =
        authority_records(mint_key, success_tokens.venue);
    let (refusal_authority, refusal_market, refusal_realm, refusal_policy, refusal_manifest) =
        authority_records(mint_key, refusal_tokens.venue);
    assert_eq!(success_authority.realm, refusal_authority.realm);
    assert_eq!(realm_bytes, refusal_realm);
    let success = registered_market_fixture(
        success_authority,
        seller_maker.pubkey(),
        buyer_maker.pubkey(),
        success_tokens,
    );
    let refusal = registered_market_fixture(
        refusal_authority,
        seller_maker.pubkey(),
        buyer_maker.pubkey(),
        refusal_tokens,
    );

    let mut test = ProgramTest::new("dclutch_controller_proof_sbf", CONTROLLER_PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_program("dclutch_claims_proof_sbf", CLAIM_PROGRAM_ID, None);
    test.add_program("dclutch_custody_proof_sbf", CUSTODY_PROGRAM_ID, None);
    test.add_program("spl_token", token_program_id(), None);
    add_program_account(&mut test, controller);
    for fixture in [success, refusal] {
        add_claim_state(
            &mut test,
            fixture.seller_replay,
            registered_state(
                controller,
                seller_maker.pubkey(),
                registered_intent(fixture.market, fixture.tokens.seller, 0),
            ),
        );
        add_claim_state(
            &mut test,
            fixture.buyer_replay,
            registered_state(
                controller,
                buyer_maker.pubkey(),
                registered_intent(fixture.market, fixture.tokens.source, 1),
            ),
        );
        add_claim_state(
            &mut test,
            fixture.seller_position,
            position_state(controller, 1, 5_000),
        );
        add_claim_state(
            &mut test,
            fixture.buyer_position,
            position_state(controller, 1, 200),
        );
    }
    test.add_account(
        journal_key,
        Account {
            lamports: Rent::default().minimum_balance(JOURNAL_BYTES),
            data: journal(0),
            owner: CONTROLLER_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    add_protocol_account(&mut test, success_authority.realm, realm_bytes);
    for (authority, market, policy, manifest) in [
        (
            success_authority,
            success_market,
            success_policy,
            success_manifest,
        ),
        (
            refusal_authority,
            refusal_market,
            refusal_policy,
            refusal_manifest,
        ),
    ] {
        add_protocol_account(&mut test, authority.market, market);
        add_protocol_account(&mut test, authority.fee_policy, policy);
        add_protocol_account(&mut test, authority.capability_manifest, manifest);
    }
    test.add_account(
        mint_key,
        Account {
            lamports: Rent::default().minimum_balance(MINT_BYTES),
            data: mint(40_000, 6),
            owner: token_program_id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    for (fixture, frozen) in [(success, false), (refusal, true)] {
        add_token_account(
            &mut test,
            fixture.tokens.source,
            token_account(
                mint_key,
                buyer_maker.pubkey(),
                2_000,
                Some((fixture.buyer_replay, 1_002)),
                false,
            ),
        );
        add_token_account(
            &mut test,
            fixture.tokens.seller,
            token_account(mint_key, seller_maker.pubkey(), 100, None, false),
        );
        add_token_account(
            &mut test,
            fixture.tokens.venue,
            token_account(mint_key, Pubkey::new_unique(), 20, None, frozen),
        );
    }
    let mut context = test.start_with_context().await;

    let untouched_journal = account(&mut context, journal_key).await;
    let untouched_claims = claim_state_accounts(&mut context, success).await;
    let untouched_source = account(&mut context, success.tokens.source).await;
    let mut wrong_data = registered_controller_data(controller_bump, success, 1_000);
    wrong_data[12] = success.buyer_bump.wrapping_add(1);
    let wrong_coordinate = submit(
        &mut context,
        &[registered_controller_instruction(
            controller,
            journal_key,
            success,
            mint_key,
            wrong_data,
        )],
    )
    .await;
    assert!(
        !wrong_coordinate.accepted,
        "wrong registered-intent coordinate must refuse"
    );
    assert_eq!(account(&mut context, journal_key).await, untouched_journal);
    assert_eq!(
        claim_state_accounts(&mut context, success).await,
        untouched_claims
    );
    assert_eq!(
        account(&mut context, success.tokens.source).await,
        untouched_source
    );

    let fill_data = registered_controller_data(controller_bump, success, 1_000);
    let first = submit(
        &mut context,
        &[registered_controller_instruction(
            controller,
            journal_key,
            success,
            mint_key,
            fill_data.clone(),
        )],
    )
    .await;
    assert!(first.accepted, "first registered residual fill must commit");
    let claims = claim_state_accounts(&mut context, success).await;
    let seller = RegisteredIntentStateV1::decode(&claims[0].data).expect("seller registration");
    let buyer = RegisteredIntentStateV1::decode(&claims[1].data).expect("buyer registration");
    assert_eq!(
        (seller.remaining, seller.sequence, seller.phase),
        (1_000, 1, 0)
    );
    assert_eq!(
        (buyer.remaining, buyer.sequence, buyer.phase),
        (1_000, 1, 0)
    );
    assert_eq!(read_u64(&claims[2].data, 48), 4_000);
    assert_eq!(read_u64(&claims[3].data, 48), 1_200);
    assert_eq!(
        read_u64(&account(&mut context, success.tokens.source).await.data, 64),
        1_499
    );
    assert_eq!(
        read_u64(&account(&mut context, success.tokens.seller).await.data, 64),
        600
    );
    assert_eq!(
        read_u64(&account(&mut context, success.tokens.venue).await.data, 64),
        21
    );

    let second = submit(
        &mut context,
        &[registered_controller_instruction(
            controller,
            journal_key,
            success,
            mint_key,
            fill_data,
        )],
    )
    .await;
    assert!(
        second.accepted,
        "terminal registered residual fill must commit"
    );
    let claims = claim_state_accounts(&mut context, success).await;
    let seller = RegisteredIntentStateV1::decode(&claims[0].data).expect("seller registration");
    let buyer = RegisteredIntentStateV1::decode(&claims[1].data).expect("buyer registration");
    assert_eq!((seller.remaining, seller.sequence, seller.phase), (0, 2, 1));
    assert_eq!((buyer.remaining, buyer.sequence, buyer.phase), (0, 2, 1));
    assert_eq!(read_u64(&claims[2].data, 48), 3_000);
    assert_eq!(read_u64(&claims[3].data, 48), 2_200);
    assert_eq!(
        read_u64(&account(&mut context, journal_key).await.data, 8),
        2
    );
    assert_eq!(
        read_u64(&account(&mut context, success.tokens.source).await.data, 64),
        998
    );
    assert_eq!(
        read_u64(&account(&mut context, success.tokens.seller).await.data, 64),
        1_100
    );
    assert_eq!(
        read_u64(&account(&mut context, success.tokens.venue).await.data, 64),
        22
    );

    let journal_before = account(&mut context, journal_key).await;
    let claims_before = claim_state_accounts(&mut context, refusal).await;
    let source_before = account(&mut context, refusal.tokens.source).await;
    let seller_before = account(&mut context, refusal.tokens.seller).await;
    let venue_before = account(&mut context, refusal.tokens.venue).await;
    let late_refusal = submit(
        &mut context,
        &[registered_controller_instruction(
            controller,
            journal_key,
            refusal,
            mint_key,
            registered_controller_data(controller_bump, refusal, 1_000),
        )],
    )
    .await;
    assert!(
        !late_refusal.accepted,
        "frozen venue must refuse after the first real Token CPI"
    );
    let token_success = format!("Program {} success", token_program_id());
    assert!(
        late_refusal.logs.iter().any(|line| line == &token_success),
        "logs must prove first official Token CPI completed before refusal"
    );
    assert_eq!(account(&mut context, journal_key).await, journal_before);
    assert_eq!(
        claim_state_accounts(&mut context, refusal).await,
        claims_before
    );
    assert_eq!(
        account(&mut context, refusal.tokens.source).await,
        source_before
    );
    assert_eq!(
        account(&mut context, refusal.tokens.seller).await,
        seller_before
    );
    assert_eq!(
        account(&mut context, refusal.tokens.venue).await,
        venue_before
    );

    eprintln!(
        "registered Direct CU: wrong coordinate={}, first residual={}, terminal residual={}, late rollback={}; wire: legacy={} bytes, all-address v0={} bytes",
        wrong_coordinate.compute_units,
        first.compute_units,
        second.compute_units,
        late_refusal.compute_units,
        first.wire_bytes,
        first.v0_wire_bytes,
    );
}

#[tokio::test]
async fn registered_creation_funds_dust_and_hands_replay_to_local_state_atomically() {
    require_sbf();
    let maker = Keypair::new();
    let (controller, controller_bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &CONTROLLER_PROGRAM_ID);
    let mint_key = Pubkey::new_unique();
    let source = Pubkey::new_unique();
    let venue = Pubkey::new_unique();
    let (authority, market_bytes, realm_bytes, policy_bytes, manifest_bytes) =
        authority_records(mint_key, venue);
    let generation = GENERATION.to_le_bytes();
    let (replay, replay_bump) = Pubkey::find_program_address(
        &[
            REPLAY_SEED,
            authority.market.as_ref(),
            &generation,
            maker.pubkey().as_ref(),
        ],
        &CONTROLLER_PROGRAM_ID,
    );
    let registration_address = |nonce: u64| {
        Pubkey::find_program_address(
            &[
                REGISTERED_SEED,
                authority.market.as_ref(),
                &generation,
                maker.pubkey().as_ref(),
                &nonce.to_le_bytes(),
            ],
            &CONTROLLER_PROGRAM_ID,
        )
    };
    let (first_registration, first_bump) = registration_address(0);
    let (second_registration, second_bump) = registration_address(1);
    let (wrong_nonce_registration, wrong_nonce_bump) = registration_address(3);

    let mut test = ProgramTest::new("dclutch_controller_proof_sbf", CONTROLLER_PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_program("dclutch_claims_proof_sbf", CLAIM_PROGRAM_ID, None);
    test.add_program("spl_token", token_program_id(), None);
    add_program_account(&mut test, controller);
    add_program_account(&mut test, maker.pubkey());
    for (address, lamports) in [
        (replay, 11_u64),
        (first_registration, 13),
        (second_registration, 17),
        (wrong_nonce_registration, 19),
    ] {
        test.add_account(
            address,
            Account {
                lamports,
                data: vec![],
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    for (address, data) in [
        (authority.market, market_bytes),
        (authority.realm, realm_bytes),
        (authority.fee_policy, policy_bytes),
        (authority.capability_manifest, manifest_bytes),
    ] {
        add_protocol_account(&mut test, address, data);
    }
    test.add_account(
        mint_key,
        Account {
            lamports: Rent::default().minimum_balance(MINT_BYTES),
            data: mint(40_000, 6),
            owner: token_program_id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    add_token_account(
        &mut test,
        source,
        token_account(mint_key, maker.pubkey(), 10_000, None, false),
    );
    add_token_account(
        &mut test,
        venue,
        token_account(mint_key, Pubkey::new_unique(), 0, None, false),
    );
    let mut context = test.start_with_context().await;
    let payer = context.payer.pubkey();

    let first_intent = compact_intent(authority.market, source, 1, 0);
    let replay_before = account(&mut context, replay).await;
    let first_before = account(&mut context, first_registration).await;
    let unapproved = submit_registered_create(
        &mut context,
        &[registered_create_instruction(
            controller,
            maker.pubkey(),
            payer,
            replay,
            first_registration,
            authority,
            mint_key,
            source,
            venue,
            registered_create_data(controller_bump, replay_bump, first_bump, first_intent),
        )],
        &maker,
    )
    .await;
    assert!(
        !unapproved.0,
        "buyer registration without delegation must refuse"
    );
    assert_eq!(account(&mut context, replay).await, replay_before);
    assert_eq!(
        account(&mut context, first_registration).await,
        first_before
    );

    let first = submit_registered_create(
        &mut context,
        &[
            token_approve(source, first_registration, maker.pubkey(), FILL),
            registered_create_instruction(
                controller,
                maker.pubkey(),
                payer,
                replay,
                first_registration,
                authority,
                mint_key,
                source,
                venue,
                registered_create_data(controller_bump, replay_bump, first_bump, first_intent),
            ),
        ],
        &maker,
    )
    .await;
    assert!(first.0, "dust-tolerant first registration must commit");
    let replay_after_first = account(&mut context, replay).await;
    assert_eq!(replay_after_first.owner, CLAIM_PROGRAM_ID);
    assert_eq!(replay_after_first.data.len(), REPLAY_STATE_BYTES);
    assert_eq!(read_u64(&replay_after_first.data, 40), 1);
    let first_account = account(&mut context, first_registration).await;
    assert_eq!(first_account.owner, CLAIM_PROGRAM_ID);
    assert!(first_account.lamports >= Rent::default().minimum_balance(first_account.data.len()));
    let first_state =
        RegisteredIntentStateV1::decode(&first_account.data).expect("first registration state");
    assert_eq!(first_state.intent, first_intent);
    assert_eq!(first_state.maker, maker.pubkey().to_bytes());
    assert_eq!((first_state.remaining, first_state.sequence), (FILL, 0));

    let second_intent = compact_intent(authority.market, source, 1, 1);
    let second = submit_registered_create(
        &mut context,
        &[
            token_approve(source, second_registration, maker.pubkey(), FILL),
            registered_create_instruction(
                controller,
                maker.pubkey(),
                payer,
                replay,
                second_registration,
                authority,
                mint_key,
                source,
                venue,
                registered_create_data(controller_bump, replay_bump, second_bump, second_intent),
            ),
        ],
        &maker,
    )
    .await;
    assert!(
        second.0,
        "existing replay root must admit its exact next nonce"
    );
    let replay_after_second = account(&mut context, replay).await;
    assert_eq!(read_u64(&replay_after_second.data, 40), 2);
    let second_account = account(&mut context, second_registration).await;
    let second_state =
        RegisteredIntentStateV1::decode(&second_account.data).expect("second registration state");
    assert_eq!(second_state.intent, second_intent);

    let wrong_nonce_before = account(&mut context, wrong_nonce_registration).await;
    let source_before = account(&mut context, source).await;
    let wrong_nonce_intent = compact_intent(authority.market, source, 1, 3);
    let wrong_nonce = submit_registered_create(
        &mut context,
        &[
            token_approve(source, wrong_nonce_registration, maker.pubkey(), FILL),
            registered_create_instruction(
                controller,
                maker.pubkey(),
                payer,
                replay,
                wrong_nonce_registration,
                authority,
                mint_key,
                source,
                venue,
                registered_create_data(
                    controller_bump,
                    replay_bump,
                    wrong_nonce_bump,
                    wrong_nonce_intent,
                ),
            ),
        ],
        &maker,
    )
    .await;
    assert!(
        !wrong_nonce.0,
        "registration must not skip the global maker replay nonce"
    );
    assert_eq!(account(&mut context, replay).await, replay_after_second);
    assert_eq!(
        account(&mut context, wrong_nonce_registration).await,
        wrong_nonce_before
    );
    assert_eq!(account(&mut context, source).await, source_before);

    eprintln!(
        "registered creation Direct CU: unapproved={}, first={}, reused replay={}, wrong nonce rollback={}",
        unapproved.1, first.1, second.1, wrong_nonce.1
    );
}

#[tokio::test]
async fn registered_terminal_routes_enforce_maker_time_and_local_sequence() {
    require_sbf();
    let cancel_maker = Keypair::new();
    let wrong_maker = Keypair::new();
    let expiry_maker = Pubkey::new_unique();
    let (controller, controller_bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &CONTROLLER_PROGRAM_ID);
    let cancel_market = Pubkey::new_unique();
    let expiry_market = Pubkey::new_unique();
    let generation = GENERATION.to_le_bytes();
    let nonce = 0_u64.to_le_bytes();
    let (cancel_registration, cancel_bump) = Pubkey::find_program_address(
        &[
            REGISTERED_SEED,
            cancel_market.as_ref(),
            &generation,
            cancel_maker.pubkey().as_ref(),
            &nonce,
        ],
        &CONTROLLER_PROGRAM_ID,
    );
    let (expiry_registration, expiry_bump) = Pubkey::find_program_address(
        &[
            REGISTERED_SEED,
            expiry_market.as_ref(),
            &generation,
            expiry_maker.as_ref(),
            &nonce,
        ],
        &CONTROLLER_PROGRAM_ID,
    );

    let cancel_intent = registered_intent(cancel_market, Pubkey::new_unique(), 0);
    let mut expiry_intent = registered_intent(expiry_market, Pubkey::new_unique(), 1);
    expiry_intent.valid_through = 100;
    let cancel_state = registered_state(controller, cancel_maker.pubkey(), cancel_intent);
    let expiry_state = RegisteredIntentStateV1 {
        phase: 0,
        controller: controller.to_bytes(),
        maker: expiry_maker.to_bytes(),
        intent: expiry_intent,
        remaining: expiry_intent.maximum_fill,
        sequence: 3,
    }
    .encode()
    .expect("canonical expiry registration")
    .to_vec();

    let mut test = ProgramTest::new("dclutch_controller_proof_sbf", CONTROLLER_PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_program("dclutch_claims_proof_sbf", CLAIM_PROGRAM_ID, None);
    add_program_account(&mut test, controller);
    add_program_account(&mut test, cancel_maker.pubkey());
    add_program_account(&mut test, wrong_maker.pubkey());
    add_claim_state(&mut test, cancel_registration, cancel_state);
    add_claim_state(&mut test, expiry_registration, expiry_state);
    let mut context = test.start_with_context().await;

    let cancel_before = account(&mut context, cancel_registration).await;
    let stale_cancel = submit_terminal(
        &mut context,
        registered_terminal_instruction(
            controller,
            cancel_registration,
            Some(cancel_maker.pubkey()),
            registered_terminal_data(
                RegisteredTerminalAction::Cancel,
                controller_bump,
                cancel_bump,
                1,
            ),
        ),
        Some(&cancel_maker),
    )
    .await;
    assert!(!stale_cancel.0, "stale local sequence must refuse cancel");
    assert_eq!(
        account(&mut context, cancel_registration).await,
        cancel_before
    );

    let impersonated_cancel = submit_terminal(
        &mut context,
        registered_terminal_instruction(
            controller,
            cancel_registration,
            Some(wrong_maker.pubkey()),
            registered_terminal_data(
                RegisteredTerminalAction::Cancel,
                controller_bump,
                cancel_bump,
                0,
            ),
        ),
        Some(&wrong_maker),
    )
    .await;
    assert!(
        !impersonated_cancel.0,
        "a signer other than the persisted maker must refuse cancel"
    );
    assert_eq!(
        account(&mut context, cancel_registration).await,
        cancel_before
    );

    let cancel = submit_terminal(
        &mut context,
        registered_terminal_instruction(
            controller,
            cancel_registration,
            Some(cancel_maker.pubkey()),
            registered_terminal_data(
                RegisteredTerminalAction::Cancel,
                controller_bump,
                cancel_bump,
                0,
            ),
        ),
        Some(&cancel_maker),
    )
    .await;
    assert!(cancel.0, "the exact maker-authorized cancel must commit");
    let cancelled_account = account(&mut context, cancel_registration).await;
    let cancelled =
        RegisteredIntentStateV1::decode(&cancelled_account.data).expect("cancelled registration");
    assert_eq!(
        (cancelled.phase, cancelled.remaining, cancelled.sequence),
        (2, FILL, 1)
    );

    let repeated_cancel = submit_terminal(
        &mut context,
        registered_terminal_instruction(
            controller,
            cancel_registration,
            Some(cancel_maker.pubkey()),
            registered_terminal_data(
                RegisteredTerminalAction::Cancel,
                controller_bump,
                cancel_bump,
                1,
            ),
        ),
        Some(&cancel_maker),
    )
    .await;
    assert!(!repeated_cancel.0, "terminal cancel replay must refuse");
    assert_eq!(
        account(&mut context, cancel_registration).await,
        cancelled_account
    );

    let expiry_before = account(&mut context, expiry_registration).await;
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock sysvar");
    assert!(clock.slot <= expiry_intent.valid_through);
    let early_expiry = submit_terminal(
        &mut context,
        registered_terminal_instruction(
            controller,
            expiry_registration,
            None,
            registered_terminal_data(
                RegisteredTerminalAction::Expire,
                controller_bump,
                expiry_bump,
                3,
            ),
        ),
        None,
    )
    .await;
    assert!(
        !early_expiry.0,
        "expiry at or before valid-through must refuse"
    );
    assert_eq!(
        account(&mut context, expiry_registration).await,
        expiry_before
    );

    context
        .warp_to_slot(expiry_intent.valid_through + 1)
        .expect("advance beyond the signed validity window");
    let expiry = submit_terminal(
        &mut context,
        registered_terminal_instruction(
            controller,
            expiry_registration,
            None,
            registered_terminal_data(
                RegisteredTerminalAction::Expire,
                controller_bump,
                expiry_bump,
                3,
            ),
        ),
        None,
    )
    .await;
    assert!(expiry.0, "permissionless post-window expiry must commit");
    let expired_account = account(&mut context, expiry_registration).await;
    let expired =
        RegisteredIntentStateV1::decode(&expired_account.data).expect("expired registration");
    assert_eq!(
        (expired.phase, expired.remaining, expired.sequence),
        (3, FILL, 4)
    );

    let repeated_expiry = submit_terminal(
        &mut context,
        registered_terminal_instruction(
            controller,
            expiry_registration,
            None,
            registered_terminal_data(
                RegisteredTerminalAction::Expire,
                controller_bump,
                expiry_bump,
                4,
            ),
        ),
        None,
    )
    .await;
    assert!(!repeated_expiry.0, "terminal expiry replay must refuse");
    assert_eq!(
        account(&mut context, expiry_registration).await,
        expired_account
    );

    eprintln!(
        "registered terminal Direct CU: stale cancel={}, impersonated cancel={}, cancel={}, replayed cancel={}, early expiry={}, expiry={}, replayed expiry={}",
        stale_cancel.1,
        impersonated_cancel.1,
        cancel.1,
        repeated_cancel.1,
        early_expiry.1,
        expiry.1,
        repeated_expiry.1,
    );
}
