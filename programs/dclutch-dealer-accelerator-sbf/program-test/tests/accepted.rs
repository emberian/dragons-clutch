//! Executed caller evidence for the Dealer scenario accepted transition.
//!
//! The canonical unsplit admitted Hot instruction for this scenario resolves
//! 121 account locks against a 64-lock runtime ceiling, so it can never be
//! submitted anywhere -- devnet, mainnet, or this harness. The lock-bounded
//! checkpoint routes are the submittable form of the same transition, and this
//! campaign is the first thing that actually submits them.
//!
//! What runs here is a real caller against the real Trading ELF: the transcript
//! `dclutch_operator::dealer_scenario_checkpoint_v1::build_dealer_accepted_transcript_v4`
//! emits is signed and processed transaction by transaction, and the durable
//! journal advances only on observed success. Every instruction's account order,
//! privileges and route data come from the operator; this file states no span,
//! no bitmap, and no account order of its own.
//!
//! This is a Dealer scenario accepted-transition campaign. It selects no price,
//! quotes nothing, and holds no inventory. It is not an AMM, an order book, or a
//! quote surface.

use std::vec::Vec;

use dclutch_capability_program_contract::set_v1::CapabilityProgramSetV1;
use dclutch_dealer_codec::{
    scenario::ClaimsInventoryObservation,
    scenario_checkpoint_v1::DEALER_SCENARIO_PREPARATION_PAGES_V1,
    scenario_membership_manifest_v1::{
        DEALER_SCENARIO_MEMBERSHIP_PAGES_V1, DealerScenarioMembershipManifestV1,
    },
};
use dclutch_operator::{
    dealer_scenario_checkpoint_v1::{
        DealerScenarioCheckpointJournalV1, DealerScenarioCheckpointRouteV1,
        build_dealer_scenario_checkpoint_create_v1, build_dealer_scenario_checkpoint_page_v1,
        dealer_scenario_checkpoint_address_v1, dealer_scenario_membership_manifest_address_v1,
        project_dealer_scenario_canonical_membership_pages_v1,
    },
    dealer_scenario_hot_v4::{DealerScenarioHotMetaStateV4, SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1},
    direct_inline_v3::ObservedAccountMetaV3,
};
use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};
use dclutch_trading_sbf::dealer::{
    v3_obligation::{
        DEALER_OBLIGATION_HEADER_BYTES_V3, DEALER_OBLIGATION_MAGIC_V3,
        DEALER_OBLIGATION_PDA_DOMAIN_V3, DEALER_OBLIGATION_VERSION_V3,
        DealerObligationProjectionV3,
    },
    v3_trade::{
        DEALER_SCENARIO_TRADE_ACTION_V3, DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3,
        ScenarioTradeChainProjectionV3, ScenarioTradeDirectionV3, ScenarioTradeIntentV3,
        build_scenario_trade_request_v3, scenario_trade_max_request_bytes_v3,
    },
};
use solana_account::{Account, AccountSharedData};
use solana_program::{
    hash::{Hash, hash},
    instruction::Instruction,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::TransactionError;
use solana_sdk_ids::{system_program, sysvar};
use solana_transaction::Transaction;

/// Release-selected Trading program the campaign installs the real ELF at.
const TRADING: Pubkey = Pubkey::new_from_array([0xd0; 32]);
/// Producer that owns the canonical membership manifest.
const MANIFEST_PRODUCER: Pubkey = Pubkey::new_from_array([0xd1; 32]);
/// Immutable rent beneficiary named at creation.
const BENEFICIARY: Pubkey = Pubkey::new_from_array([0xd2; 32]);
/// Counterparty Claims Position owner.
const COUNTERPARTY: Pubkey = Pubkey::new_from_array([0xd3; 32]);
/// Counterparty external collateral account.
const COUNTERPARTY_ACCOUNT: Pubkey = Pubkey::new_from_array([0xd4; 32]);
/// Immutable Trading child root.
const CHILD_ROOT: Pubkey = Pubkey::new_from_array([0xd5; 32]);
/// Logical Core Market.
const MARKET: Pubkey = Pubkey::new_from_array([0xd6; 32]);
/// Exact immutable Dealer request account.
const REQUEST: Pubkey = Pubkey::new_from_array([0xd7; 32]);

/// The refusal Trading raises when a route's account content is not canonical.
const TRADING_CONTENT: u32 = 0x4003;

/// Runtime Product outcome width this scenario transitions.
const WIDTH: u32 = 3;

fn observation() -> Observation {
    Observation {
        slot: 20,
        unix_timestamp: 12,
        finality: Finality::Finalized,
    }
}

/// One membership observation. Only its identity reaches the manifest.
fn membership_meta(key: Pubkey) -> ObservedAccountMetaV3 {
    ObservedAccountMetaV3 {
        account: ObservedAccount {
            observation: observation(),
            key,
            owner: Pubkey::new_from_array([200; 32]),
            lamports: 1,
            executable: false,
            data: Vec::new(),
        },
        is_signer: false,
        is_writable: false,
    }
}

fn obligation_bytes(
    market: [u8; 32],
    product: [u8; 32],
    basis: [u8; 32],
    owner: [u8; 32],
    child: [u8; 32],
    revision: u64,
    values: &[u64],
) -> Vec<u8> {
    let mut bytes = vec![0; DEALER_OBLIGATION_HEADER_BYTES_V3 + values.len() * 8];
    bytes[..8].copy_from_slice(&DEALER_OBLIGATION_MAGIC_V3);
    bytes[8..10].copy_from_slice(&DEALER_OBLIGATION_VERSION_V3.to_le_bytes());
    bytes[12..16].copy_from_slice(
        &u32::try_from(values.len())
            .expect("small obligation width")
            .to_le_bytes(),
    );
    bytes[16..24].copy_from_slice(&revision.to_le_bytes());
    for (offset, value) in [
        (24, market),
        (56, product),
        (88, basis),
        (120, owner),
        (152, child),
    ] {
        bytes[offset..offset + 32].copy_from_slice(&value);
    }
    for (index, value) in values.iter().enumerate() {
        let offset = DEALER_OBLIGATION_HEADER_BYTES_V3 + index * 8;
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn program_set_bytes() -> Vec<u8> {
    let mut bytes = vec![0; 72];
    bytes[..8].copy_from_slice(b"DCLTCPS1");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3.to_le_bytes());
    bytes[16] = 2;
    bytes[18..20].copy_from_slice(&1_u16.to_le_bytes());
    bytes[32..36].copy_from_slice(&u32::from(DEALER_SCENARIO_TRADE_ACTION_V3).to_le_bytes());
    bytes[36..68].copy_from_slice(&[42; 32]);
    bytes
}

fn data_account(owner: Pubkey, data: Vec<u8>) -> Account {
    Account {
        lamports: Rent::default().minimum_balance(data.len()).max(1),
        data,
        owner,
        executable: false,
        rent_epoch: 0,
    }
}

/// Everything the campaign installs and every derived fact it re-checks.
struct Scenario {
    dealer: Keypair,
    request_bytes: Vec<u8>,
    request_digest: [u8; 32],
    obligation: Pubkey,
    obligation_state: Vec<u8>,
    checkpoint: Pubkey,
    membership_manifest: Pubkey,
    manifest_bytes: Vec<u8>,
    pages: [Vec<Pubkey>; DEALER_SCENARIO_MEMBERSHIP_PAGES_V1],
    membership: Vec<Pubkey>,
}

/// Derive one complete scenario: request, checkpoint, canonical membership.
fn scenario() -> Scenario {
    let dealer = Keypair::new();
    let dealer_owner = dealer.pubkey().to_bytes();
    let market = MARKET.to_bytes();
    let product = [0xb1; 32];
    let basis = [0xb2; 32];
    let child = CHILD_ROOT.to_bytes();
    let obligation_state = obligation_bytes(market, product, basis, dealer_owner, child, 7, &[
        12, 20, 10,
    ]);
    let current_obligation =
        DealerObligationProjectionV3::decode(&obligation_state).expect("canonical obligation");
    let obligation = Pubkey::find_program_address(&[DEALER_OBLIGATION_PDA_DOMAIN_V3, &child], &TRADING).0;
    let dealer_inventory = [2, 10, 0];
    let counterparty_inventory = [20, 5, 9];
    let chain = ScenarioTradeChainProjectionV3 {
        trading_program: TRADING.to_bytes(),
        release_set: [0xb3; 32],
        market,
        child_root: child,
        obligation_address: obligation.to_bytes(),
        current_obligation,
        dealer_position: ClaimsInventoryObservation {
            market_id: market,
            product_id: product,
            liability_basis_id: basis,
            position_owner: dealer_owner,
            revision: 9,
            inventory: &dealer_inventory,
        },
        counterparty_position: ClaimsInventoryObservation {
            market_id: market,
            product_id: product,
            liability_basis_id: basis,
            position_owner: COUNTERPARTY.to_bytes(),
            revision: 11,
            inventory: &counterparty_inventory,
        },
        product_record_digest: [0xb4; 32],
        linked_basis_record_digest: [0xb5; 32],
        counterparty_account: COUNTERPARTY_ACCOUNT.to_bytes(),
        principal_balance: 100,
        locked_capital_floor: 0,
        claims_revision: 8,
        generation: 17,
        now: 20,
        expires_at: 25,
        terminal: false,
    };
    let intent = ScenarioTradeIntentV3 {
        direction: ScenarioTradeDirectionV3::CounterpartyPaysDealer,
        principal: 10,
        realized_fee: 1,
        acquired: &[3, 0, 4],
        delivered: &[0, 1, 0],
        candidate_obligations: &[10, 19, 13],
    };
    let set_bytes = program_set_bytes();
    let set = CapabilityProgramSetV1::decode(&set_bytes).expect("canonical program set");
    let mut request_bytes =
        vec![0; scenario_trade_max_request_bytes_v3(WIDTH).expect("request bound")];
    let built = build_scenario_trade_request_v3(chain, intent, set, &mut request_bytes)
        .expect("chain-derived request");
    request_bytes.truncate(built.request_bytes);
    let request_digest = hash(&request_bytes).to_bytes();
    let checkpoint = dealer_scenario_checkpoint_address_v1(TRADING, request_digest);
    let membership_manifest =
        dealer_scenario_membership_manifest_address_v1(MANIFEST_PRODUCER, checkpoint, request_digest);

    // The membership transcript is the complete physical Dealer frame for this
    // scenario after alias de-duplication. Its width is the reason the split
    // exists: one instruction naming all of it cannot be submitted.
    // The checkpoint, the clock and the manifest are fixed page accounts, so
    // they are never members of the transcript the pages carry.
    let mut membership = vec![
        CHILD_ROOT,
        obligation,
        REQUEST,
        MARKET,
        TRADING,
        COUNTERPARTY,
        COUNTERPARTY_ACCOUNT,
        BENEFICIARY,
    ];
    let mut filler = 0_u32;
    while membership.len() < 121 {
        let mut seed = [0_u8; 32];
        seed[..4].copy_from_slice(&filler.to_le_bytes());
        seed[31] = 0xef;
        let key = Pubkey::new_from_array(seed);
        if !membership.contains(&key)
            && key != checkpoint
            && key != membership_manifest
            && key != sysvar::clock::ID
        {
            membership.push(key);
        }
        filler += 1;
    }
    let metas = membership
        .iter()
        .copied()
        .map(membership_meta)
        .collect::<Vec<_>>();
    let canonical = project_dealer_scenario_canonical_membership_pages_v1(
        DealerScenarioHotMetaStateV4 {
            fixed_accounts: &metas,
            strategy_accounts: &[],
            runtime_suffix_accounts: &[],
        },
        MANIFEST_PRODUCER,
        checkpoint,
        request_digest,
    )
    .expect("canonical membership partition");
    let manifest_bytes = canonical.manifest.encode().expect("manifest encode").to_vec();
    Scenario {
        dealer,
        request_bytes,
        request_digest,
        obligation,
        obligation_state,
        checkpoint,
        membership_manifest,
        manifest_bytes,
        pages: canonical.pages,
        membership,
    }
}

/// Install the whole scenario and the real Trading ELF.
fn program_test(scenario: &Scenario) -> ProgramTest {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    test.add_program("dclutch_trading_sbf", TRADING, None);
    test.add_account(
        REQUEST,
        data_account(TRADING, scenario.request_bytes.clone()),
    );
    test.add_account(CHILD_ROOT, data_account(TRADING, vec![0xaa; 64]));
    test.add_account(
        scenario.obligation,
        data_account(TRADING, scenario.obligation_state.clone()),
    );
    test.add_account(
        scenario.membership_manifest,
        data_account(MANIFEST_PRODUCER, scenario.manifest_bytes.clone()),
    );
    test.add_account(BENEFICIARY, data_account(system_program::ID, Vec::new()));
    test.add_account(
        COUNTERPARTY,
        data_account(system_program::ID, Vec::new()),
    );
    test.add_account(
        COUNTERPARTY_ACCOUNT,
        data_account(system_program::ID, Vec::new()),
    );
    add_executable(&mut test, MANIFEST_PRODUCER);
    for key in &scenario.membership {
        if *key == TRADING
            || *key == REQUEST
            || *key == CHILD_ROOT
            || *key == scenario.obligation
            || *key == scenario.membership_manifest
            || *key == COUNTERPARTY
            || *key == COUNTERPARTY_ACCOUNT
            || *key == MARKET
            || *key == BENEFICIARY
        {
            continue;
        }
        test.add_account(*key, data_account(system_program::ID, vec![0x11]));
    }
    test.add_account(MARKET, data_account(system_program::ID, vec![0x22; 32]));
    test
}

fn add_executable(test: &mut ProgramTest, key: Pubkey) {
    test.add_account(key, Account {
        lamports: 1,
        data: Vec::new(),
        owner: solana_sdk_ids::bpf_loader_upgradeable::ID,
        executable: true,
        rent_epoch: 0,
    });
}

/// Sign and process one route.
///
/// The signer set is route-derived: creation carries a wallet authority beyond
/// the payer, and every other route carries none.
async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    extra_signers: &[&Keypair],
) -> Result<solana_program_test::BanksTransactionResultWithMetadata, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let payer = context.payer.insecure_clone();
    let mut signers: Vec<&Keypair> = vec![&payer];
    signers.extend_from_slice(extra_signers);
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &signers,
        blockhash,
    );
    context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
}

/// Build the creation route for one dealer authority.
fn create_instruction(
    scenario: &Scenario,
    payer: Pubkey,
    dealer_authority: Pubkey,
) -> (Instruction, usize) {
    let packet = build_dealer_scenario_checkpoint_create_v1(
        TRADING,
        payer,
        dealer_authority,
        BENEFICIARY,
        scenario.checkpoint,
        REQUEST,
        CHILD_ROOT,
        scenario.obligation,
        sysvar::clock::ID,
        sysvar::rent::ID,
        system_program::ID,
        MANIFEST_PRODUCER,
        scenario.membership_manifest,
        Hash::default(),
        &[],
    )
    .expect("create packet");
    assert_eq!(packet.route, DealerScenarioCheckpointRouteV1::Create);
    (
        packet.instruction,
        packet.lock_census.unique_account_lock_count,
    )
}

/// Build one membership page route over an exact observation set.
fn page_instruction(
    scenario: &Scenario,
    payer: Pubkey,
    page_index: u8,
    page: &[Pubkey],
) -> (Instruction, usize) {
    let packet = build_dealer_scenario_checkpoint_page_v1(
        TRADING,
        payer,
        scenario.checkpoint,
        sysvar::clock::ID,
        scenario.membership_manifest,
        page_index,
        page,
        Hash::default(),
        &[],
    )
    .expect("page packet");
    (
        packet.instruction,
        packet.lock_census.unique_account_lock_count,
    )
}

/// Read the exact current checkpoint body.
async fn checkpoint_body(context: &mut ProgramTestContext, scenario: &Scenario) -> Vec<u8> {
    context
        .banks_client
        .get_account(scenario.checkpoint)
        .await
        .expect("checkpoint query")
        .expect("checkpoint exists")
        .data
}

/// Execute creation, which every hostile page case starts from.
async fn create_checkpoint(context: &mut ProgramTestContext, scenario: &Scenario) {
    let payer = context.payer.pubkey();
    let (instruction, _) = create_instruction(scenario, payer, scenario.dealer.pubkey());
    let dealer = scenario.dealer.insecure_clone();
    let processed = submit(context, instruction, &[&dealer])
        .await
        .expect("ProgramTest processing");
    assert!(
        processed.result.is_ok(),
        "checkpoint creation must commit; observed {:?} logs {:?}",
        processed.result,
        processed.metadata.as_ref().map(|value| &value.log_messages)
    );
}

#[tokio::test]
async fn real_trading_elf_executes_the_accepted_transition_preparation() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let mut journal = DealerScenarioCheckpointJournalV1::planned(TRADING, scenario.request_digest)
        .expect("planned journal");
    assert_eq!(
        journal.checkpoint, scenario.checkpoint,
        "the durable journal and the campaign name one checkpoint"
    );
    assert_eq!(
        scenario.membership.len(),
        121,
        "the unsplit form of this scenario is the 121-account frame the split exists to carry"
    );

    let payer = context.payer.pubkey();
    let (instruction, create_locks) =
        create_instruction(&scenario, payer, scenario.dealer.pubkey());
    assert!(
        create_locks <= SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1,
        "creation must be lock-bounded"
    );
    let dealer = scenario.dealer.insecure_clone();
    let processed = submit(&mut context, instruction, &[&dealer])
        .await
        .expect("ProgramTest processing");
    assert!(
        processed.result.is_ok(),
        "checkpoint creation must commit; observed {:?} logs {:?}",
        processed.result,
        processed.metadata.as_ref().map(|value| &value.log_messages)
    );
    let created = context
        .banks_client
        .get_account(scenario.checkpoint)
        .await
        .expect("checkpoint query")
        .expect("checkpoint exists after creation");
    assert_eq!(created.owner, TRADING, "the checkpoint is Trading-owned");
    journal
        .record_created(hash(&created.data).to_bytes())
        .expect("journal records creation");

    let mut peak_locks = create_locks;
    let mut carried = 0_usize;
    for (page_index, page) in scenario.pages.iter().enumerate() {
        let ordinal = u8::try_from(page_index).expect("six pages");
        let (instruction, locks) = page_instruction(&scenario, payer, ordinal, page);
        assert!(
            locks <= SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1,
            "page {page_index} must be lock-bounded"
        );
        peak_locks = peak_locks.max(locks);
        let processed = submit(&mut context, instruction, &[])
            .await
            .expect("ProgramTest processing");
        assert!(
            processed.result.is_ok(),
            "page {page_index} must commit; observed {:?} logs {:?}",
            processed.result,
            processed.metadata.as_ref().map(|value| &value.log_messages)
        );
        let returned = processed
            .metadata
            .as_ref()
            .and_then(|value| value.return_data.as_ref())
            .map(|value| value.data.clone())
            .expect("every page returns its receipt digest");
        let digest = <[u8; 32]>::try_from(returned.as_slice()).expect("32-byte page receipt");
        let observed = checkpoint_body(&mut context, &scenario).await;
        journal
            .record_page(ordinal, digest, hash(&observed).to_bytes())
            .expect("journal records the page it observed");
        carried += page.len();
    }
    assert_eq!(
        usize::from(journal.next_page),
        DEALER_SCENARIO_PREPARATION_PAGES_V1,
        "the whole canonical membership transcript is on chain"
    );
    assert_eq!(
        carried,
        scenario.membership.len(),
        "every member of the frame was carried exactly once"
    );
    assert!(
        peak_locks <= SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1,
        "the executed transcript's peak lock count is {peak_locks}, which must stay inside the \
         64-lock ceiling the unsplit 121-account instruction cannot meet"
    );
}

#[tokio::test]
async fn a_substituted_membership_member_refuses_and_the_checkpoint_does_not_advance() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    create_checkpoint(&mut context, &scenario).await;
    let before = checkpoint_body(&mut context, &scenario).await;

    // Same page ordinal, same width, one substituted member. The manifest
    // committed this page's digest at creation, so the substitution cannot pass.
    let mut substituted = scenario.pages.first().expect("page zero").clone();
    let replacement = *scenario
        .pages
        .get(1)
        .and_then(|page| page.last())
        .expect("page one is not empty");
    *substituted.last_mut().expect("page zero is not empty") = replacement;
    let payer = context.payer.pubkey();
    let (instruction, _) = page_instruction(&scenario, payer, 0, &substituted);
    let processed = submit(&mut context, instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_CONTENT),
        "a substituted member must refuse on content; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        before,
        "a refused page must not advance the checkpoint"
    );
}

#[tokio::test]
async fn a_wrong_dealer_authority_cannot_create_the_checkpoint() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;

    // A real signature from the wrong wallet. The request names its dealer
    // owner, so signing is necessary and never sufficient.
    let impostor = Keypair::new();
    let payer = context.payer.pubkey();
    let (instruction, _) = create_instruction(&scenario, payer, impostor.pubkey());
    let processed = submit(&mut context, instruction, &[&impostor])
        .await
        .expect("ProgramTest processing");
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_CONTENT),
        "a foreign dealer authority must refuse on content; observed {:?}",
        processed.result
    );
    assert!(
        context
            .banks_client
            .get_account(scenario.checkpoint)
            .await
            .expect("checkpoint query")
            .is_none(),
        "a refused creation must leave the checkpoint address vacant"
    );
}

#[tokio::test]
async fn a_malformed_membership_manifest_refuses_every_page() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    create_checkpoint(&mut context, &scenario).await;
    let before = checkpoint_body(&mut context, &scenario).await;

    // The manifest keeps its PDA, its owner, its width and its structural
    // validity; only the order of two committed page digests changes. What
    // refuses is the body the checkpoint bound at creation, nothing shallower.
    let mut manifest = DealerScenarioMembershipManifestV1::decode(&scenario.manifest_bytes)
        .expect("canonical manifest decodes");
    manifest.page_membership_digests.swap(4, 5);
    let substituted = manifest.encode().expect("substituted manifest encodes");
    assert_ne!(
        substituted.as_slice(),
        scenario.manifest_bytes.as_slice(),
        "the substitution must actually change the body"
    );
    context.set_account(
        &scenario.membership_manifest,
        &AccountSharedData::from(data_account(MANIFEST_PRODUCER, substituted.to_vec())),
    );

    let payer = context.payer.pubkey();
    let (instruction, _) =
        page_instruction(&scenario, payer, 0, scenario.pages.first().expect("page zero"));
    let processed = submit(&mut context, instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_CONTENT),
        "a substituted manifest body must refuse on content; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        before,
        "a refused page must not advance the checkpoint"
    );
}

#[tokio::test]
async fn a_replayed_page_ordinal_refuses_after_it_already_committed() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    create_checkpoint(&mut context, &scenario).await;
    let payer = context.payer.pubkey();
    let page = scenario.pages.first().expect("page zero");

    let (instruction, _) = page_instruction(&scenario, payer, 0, page);
    submit(&mut context, instruction, &[])
        .await
        .expect("ProgramTest processing")
        .result
        .expect("the first page must commit");
    let after_first = checkpoint_body(&mut context, &scenario).await;

    // Byte-identical replay of a page the checkpoint already carries.
    let (instruction, _) = page_instruction(&scenario, payer, 0, page);
    let processed = submit(&mut context, instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert!(
        processed.result.is_err(),
        "a replayed page ordinal must fail closed; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        after_first,
        "a refused replay must not advance the checkpoint"
    );
}

/// Extract the exact program refusal code from a processed transaction.
fn custom_code(result: &Result<(), TransactionError>) -> Option<u32> {
    match result {
        Err(TransactionError::InstructionError(
            _,
            solana_program::instruction::InstructionError::Custom(code),
        )) => Some(*code),
        _ => None,
    }
}
