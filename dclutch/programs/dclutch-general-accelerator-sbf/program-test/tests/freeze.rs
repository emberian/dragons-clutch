//! Real-ELF N=1/N=258 Freeze execution and malformed-page rollback evidence.

use std::{vec, vec::Vec};

use dclutch_capability_program_contract::hot_v3::{
    DIRECT_HOT_HEAP_FRAME_BYTES_V1, HotExecutionEnvelopeV3,
};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::admitted_v3::{
    ADMITTED_INSTRUCTIONS_ACCOUNT_V3, ADMITTED_RUNTIME_ACCOUNTS_START_V3,
    ADMITTED_TRADING_PROGRAM_ACCOUNT_V3,
};
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2, ACCELERATOR_REQUEST_HEADER_BYTES_V2, AcceleratorAckV2,
    AcceleratorDispositionV2, AcceleratorRequestV2, AuthenticatedScratchPageV2, RequestTransportV2,
    SCRATCH_PAGE_HEADER_BYTES_V2, ScratchPageKindV2,
};
use dclutch_general_accelerator_sbf::GeneralAcceleratorSbfErrorV3;
use dclutch_general_accelerator_test_caller_sbf::GENERAL_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1;
use dclutch_general_adapter_contract::{
    account_rules_v3::general_account_profile_fixed_count_v3,
    collection_v1::{GeneralBatchOpeningV1, GeneralBatchV1},
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, general_hot_candidate_bank_len_v3,
        general_hot_scalar_count_v3, identity, scalar,
    },
    local_state_v3::{
        GeneralLocalStateHeaderV3, GeneralLocalStateKindV3, encode_general_local_state_v3_atomic,
        general_local_state_len_v3,
    },
    runtime_selection::{
        RUNTIME_SELECTION_CURSOR_BYTES_V2, RuntimeSelectionPhaseV2, consider_verified_candidate_v2,
    },
    runtime_width::{VerifiedCandidateHeaderV2, VerifiedCandidateV2, verified_candidate_len},
    state_artifacts_v3::general_readonly_evidence_start_v3,
};
use dclutch_general_codec::{
    Action, MAX_SELECTION_CRITERIA, SelectionCriterion, SelectionPolicyV1,
    successor_request_v2::ControllerRequestV2,
};
use dclutch_general_config_contract::{
    root::GeneralRootV2,
    v3::{GeneralConfigV3, GeneralConfigV3Input},
};
use dclutch_program_test_evidence::{TransactionEvidence, record};
use solana_account::Account;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{system_program, sysvar};
use solana_transaction::Transaction;

const ACCELERATOR: Pubkey = Pubkey::new_from_array([0xa1; 32]);
const CALLER: Pubkey = Pubkey::new_from_array([0xa2; 32]);
const REQUEST_ACCOUNT: Pubkey = Pubkey::new_from_array([0xa3; 32]);
const DUMMY: Pubkey = Pubkey::new_from_array([0xa4; 32]);
const SELECTION_STATE: Pubkey = Pubkey::new_from_array([0xa5; 32]);
const CONFIG_ACCOUNT: Pubkey = Pubkey::new_from_array([0xa6; 32]);
const PRODUCT_ACCOUNT: Pubkey = Pubkey::new_from_array([0xa7; 32]);
const BATCH_STATE: Pubkey = Pubkey::new_from_array([0xa8; 32]);
const PRODUCT_RECORD: [u8; 64] = [0xb1; 64];
const MARKET: [u8; 32] = [0xb2; 32];
const POLICY: [u8; 32] = [0xb3; 32];
const CANDIDATE: [u8; 32] = [0xb4; 32];
// The config's own market coordinates, named because the input bank has to
// declare exactly them: `require_market` compares the bank's generation and
// semantic basis against the config account's, and a literal repeated in two
// places is how those two stop agreeing.
const CONFIG_GENERATION: u64 = 9;
const CONFIG_CLAIM_BASIS_ID: [u8; 32] = [2; 32];
// The batch whose selection this freeze closes, and the deadline it fixes.
// `BATCH` used to be the literal `[0xb2; 32]` here, which was fine while
// nothing joined it to anything: the selection cursor carried it and no
// account had to agree. `Freeze` now names the closed Batch as readonly
// evidence so its transition can compare the clock against
// `collection_close + selectionSlots`, and the accelerator joins the
// presented batch's recomputed `batch_id` against the cursor's -- so the
// identity has to be the digest of an opening that was really formed. The
// deadline these produce is 1,000 + 10 = 1,010.
const COLLECTION_CLOSE_SLOT: u64 = 1_000;
const SETTLEMENT_CLOSE_SLOT: u64 = 2_000;
const ADMISSION_SLOT: u64 = 10;
const BATCH_MAX_ORDERS: u32 = 4;
const CONFIG_SELECTION_SLOTS: u64 = 10;
const FREEZE_SLOT: u64 = COLLECTION_CLOSE_SLOT + CONFIG_SELECTION_SLOTS;

struct Fixture {
    test: ProgramTest,
    instruction: Instruction,
    request_bytes: Vec<u8>,
    selection_before: Vec<u8>,
}

fn content(value: u8) -> ContentId {
    ContentId::new([value; 32]).expect("nonzero content")
}

fn product_id() -> [u8; 32] {
    hash(&PRODUCT_RECORD).to_bytes()
}

fn config() -> Vec<u8> {
    GeneralConfigV3::new(GeneralConfigV3Input {
        capacity_profile_id: [1; 32],
        claim_basis_id: CONFIG_CLAIM_BASIS_ID,
        program_set_id: [3; 32],
        generation: CONFIG_GENERATION,
        price_scale: 1,
        collection_slots: 10,
        selection_slots: CONFIG_SELECTION_SLOTS,
        settlement_slots: 10,
        max_orders_per_candidate: 4,
        max_pages_per_candidate: 4,
        continuation_reward_lamports: 1,
        selection_policy_id: POLICY,
        quote_surplus_beneficiary: [0xc2; 32],
    })
    .expect("General config")
    .to_bytes()
    .to_vec()
}

fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

/// One real opening, whose digest is the batch identity everything else names.
fn batch_opening(outcome_count: u32, sequence: u64) -> GeneralBatchOpeningV1 {
    GeneralBatchOpeningV1 {
        outcome_count,
        sequence,
        generation: CONFIG_GENERATION,
        market: MARKET,
        product_id: product_id(),
        config_id: hash(&config()).to_bytes(),
        price_scale: 1,
        collection_close_slot: COLLECTION_CLOSE_SLOT,
        settlement_close_slot: SETTLEMENT_CLOSE_SLOT,
        max_orders: BATCH_MAX_ORDERS,
    }
}

/// One batch opened against one real root, closed the way selection requires.
fn opened_batch(outcome_count: u32, sequence: u64) -> GeneralBatchV1 {
    let mut root = GeneralRootV2::active(MARKET, hash(&config()).to_bytes(), CONFIG_GENERATION)
        .expect("active General root");
    for _ in 0..sequence {
        let revision = root.revision();
        root.open_batch(revision, root.next_batch_sequence())
            .expect("advance the root to the sequence under test");
    }
    let revision = root.revision();
    GeneralBatchV1::open(
        &mut root,
        batch_opening(outcome_count, sequence),
        revision,
        ADMISSION_SLOT,
    )
    .expect("open batch")
}

fn batch_id(outcome_count: u32, sequence: u64) -> [u8; 32] {
    opened_batch(outcome_count, sequence).batch_id()
}

/// The Batch local state the freeze presents as its readonly evidence.
fn batch_account(outcome_count: u32, sequence: u64) -> Vec<u8> {
    let body = opened_batch(outcome_count, sequence).to_bytes();
    let state_len = general_local_state_len_v3(GeneralLocalStateKindV3::Batch, outcome_count)
        .expect("batch state width");
    let mut scratch = vec![0_u8; state_len];
    let mut state = vec![0_u8; state_len];
    encode_general_local_state_v3_atomic(
        GeneralLocalStateHeaderV3 {
            kind: GeneralLocalStateKindV3::Batch,
            bump: 1,
            rent_principal: 1,
            beneficiary: [0xc1; 32],
        },
        &body,
        &mut scratch,
        &mut state,
    )
    .expect("batch envelope");
    state
}

fn open_selection(outcome_count: u32) -> Vec<u8> {
    let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
    criteria[1] = SelectionCriterion::MinimizeQuoteSurplus;
    criteria[2] = SelectionCriterion::MinimizeCandidateId;
    let policy = SelectionPolicyV1 {
        policy_id: POLICY,
        criterion_count: 3,
        criteria,
    };
    let candidate_count = usize::try_from(outcome_count).expect("test outcome count");
    let mut verified = vec![0_u8; verified_candidate_len(outcome_count).expect("verified width")];
    VerifiedCandidateV2::encode_into(
        VerifiedCandidateHeaderV2 {
            outcome_count,
            page_count: 1,
            candidate_coordinate: 1,
            revision: 1,
            candidate_id: CANDIDATE,
            product_id: product_id(),
            batch_id: batch_id(outcome_count, 0),
            filled_lots: 7,
            quote_debit: 7,
            quote_credit: 0,
            price_scale: 1,
        },
        &vec![7; candidate_count],
        &vec![7; candidate_count],
        &mut verified,
    )
    .expect("verified candidate");
    let vacant = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let mut scratch = vacant;
    let mut open = vacant;
    consider_verified_candidate_v2(policy, &vacant, &verified, 0, &mut scratch, &mut open)
        .expect("open best-valid-submitted selection");
    let state_len = general_local_state_len_v3(GeneralLocalStateKindV3::Selection, outcome_count)
        .expect("selection state width");
    let mut state_scratch = vec![0_u8; state_len];
    let mut state = vec![0_u8; state_len];
    encode_general_local_state_v3_atomic(
        GeneralLocalStateHeaderV3 {
            kind: GeneralLocalStateKindV3::Selection,
            bump: 1,
            rent_principal: 1,
            beneficiary: [0xc1; 32],
        },
        &open,
        &mut state_scratch,
        &mut state,
    )
    .expect("selection envelope");
    state
}

/// Build the authenticated input bank the accelerator will reassemble.
///
/// The bank is what the accelerator authenticates the DOMAIN against: it
/// compares the config account's digest, the config's generation and its claim
/// basis, and the Product record's digest against the identities declared
/// here. This fixture used to declare only the outcome count, the settlement
/// flag and the Product digest -- so `general_config_id` was thirty-two zero
/// bytes, and every run refused domain authentication before any Freeze
/// transition ran. `lifecycle.rs` had always written these; this file simply
/// had not, and nothing could say so until the refusal learned to name itself.
fn input_bank(outcome_count: u32, current_slot: u64) -> Vec<u8> {
    let mut bank = vec![
        0_u8;
        general_hot_candidate_bank_len_v3(Action::Freeze, outcome_count)
            .expect("bank width")
    ];
    write_scalar(&mut bank, scalar::OUTCOME_COUNT, u64::from(outcome_count));
    write_scalar(&mut bank, scalar::SETTLEMENT_POSITION_PRESENT, 0);
    write_scalar(&mut bank, scalar::GENERATION, CONFIG_GENERATION);
    // THE SELECTION DEADLINE'S THREE REGISTERS, AND THE SAME LABEL THE
    // SEMANTIC BASIS CARRIES BELOW. On the real route the clock is the
    // AccountProfile's `TrustedEnvironmentV2::CurrentSlot`, the collection
    // close is projected out of the Batch evidence account, and the selection
    // window out of the config -- all three added to `Freeze`'s profile on
    // 2026-09-04 with the window conjunct. This fixture runs no profile, so
    // the harness IS the producer for them, exactly as it is for the basis.
    //
    // The transition VM that READS them runs in Trading, not here, so this
    // file cannot prove the window conjunct; it proves the accelerator half --
    // that the presented Batch is joined to the cursor it claims to close.
    write_scalar(&mut bank, scalar::CURRENT_SLOT, current_slot);
    write_scalar(
        &mut bank,
        scalar::BATCH_COLLECTION_CLOSE_SLOT,
        COLLECTION_CLOSE_SLOT,
    );
    write_scalar(
        &mut bank,
        scalar::CONFIG_SELECTION_SLOTS,
        CONFIG_SELECTION_SLOTS,
    );
    for (coordinate, value) in [
        (identity::PRODUCT_RECORD_DIGEST, product_id()),
        (identity::GENERAL_CONFIG_ID, hash(&config()).to_bytes()),
        // A STAND-IN, AND LABELLED AS ONE.
        //
        // On the real route this register is projected by the General
        // AccountProfile out of the authenticated Portfolio record's
        // `claim_basis_id` -- `account_rules_v3.rs`, the operation at
        // `general_semantic_basis_operation_index_v3`. This fixture runs no
        // profile: it hand-builds the bank, so here the harness IS the
        // producer, and the value has to be written.
        //
        // Which is exactly the act that hid the producer gap for as long as it
        // was unlabelled. Measured 2026-09-01: delete this line and the test
        // refuses `ConfigMarket` at both widths, because nothing else in this
        // file writes the register. It is kept, and it is named, so the next
        // reader does not mistake a hand-written register for a sourced one.
        (identity::SEMANTIC_BASIS_ID, CONFIG_CLAIM_BASIS_ID),
    ] {
        write_identity(&mut bank, outcome_count, coordinate, value);
    }
    bank
}

fn write_scalar(bank: &mut [u8], coordinate: u32, value: u64) {
    let start = usize::try_from(coordinate)
        .expect("scalar coordinate")
        .checked_mul(8)
        .expect("scalar byte offset");
    bank.get_mut(start..start + 8)
        .expect("scalar bank")
        .copy_from_slice(&value.to_le_bytes());
}

fn write_identity(bank: &mut [u8], width: u32, coordinate: u32, value: [u8; 32]) {
    let scalar_bytes =
        usize::try_from(general_hot_scalar_count_v3(Action::Freeze, width).expect("scalar count"))
            .expect("scalar count")
            .checked_mul(8)
            .expect("scalar bytes");
    let start = scalar_bytes
        .checked_add(
            usize::try_from(coordinate)
                .expect("identity coordinate")
                .checked_mul(32)
                .expect("identity byte offset"),
        )
        .expect("identity bank offset");
    bank.get_mut(start..start + 32)
        .expect("identity bank")
        .copy_from_slice(&value);
}

fn page_key(index: u32) -> Pubkey {
    let byte =
        u8::try_from(index.checked_add(1).expect("page key")).expect("bounded test page count");
    Pubkey::new_from_array([byte; 32])
}

fn fixture(outcome_count: u32, corrupt_page: bool, batch_sequence: u64) -> Fixture {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    // NO `set_compute_max_units` -- see `lifecycle.rs` for the mechanism. No
    // wire `set_compute_unit_limit` either, and that is a claim rather than an
    // omission: this file's heaviest transaction is 84,718 CU at width 258,
    // inside the 200,000 default, so it executes as a caller that asked for
    // nothing. Adding one would move this file's recorded packet bytes and CU
    // for no execution it enables.
    test.add_program("dclutch_general_accelerator_sbf", ACCELERATOR, None);
    test.add_program("dclutch_general_accelerator_test_caller_sbf", CALLER, None);
    let (authority, _) = Pubkey::find_program_address(
        &[GENERAL_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1],
        &CALLER,
    );
    add_account(&mut test, authority, system_program::ID, Vec::new());
    add_account(&mut test, DUMMY, system_program::ID, Vec::new());
    add_account(&mut test, CONFIG_ACCOUNT, CALLER, config());
    add_account(&mut test, PRODUCT_ACCOUNT, CALLER, PRODUCT_RECORD.to_vec());
    let selection_before = open_selection(outcome_count);
    add_account(&mut test, SELECTION_STATE, CALLER, selection_before.clone());
    // `batch_sequence` is the hostile's whole lever: sequence 0 is the batch
    // the cursor names, and any other sequence is a DIFFERENT real batch --
    // opened against a real root, correctly formed, and simply not this
    // selection's. That is the substitution the join exists to refuse, and it
    // is the one a malformed account could never stand in for.
    add_account(
        &mut test,
        BATCH_STATE,
        CALLER,
        batch_account(outcome_count, batch_sequence),
    );

    let family_request = ControllerRequestV2 {
        action: Action::Freeze,
        expected_revision: 1,
        candidate_id: None,
        page_index: 0,
        execution_index: 0,
        manifest_order_index: 0,
        state_bump: 1,
        terminal_record_bump: 0,
    }
    .to_bytes()
    .expect("Freeze request");
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(family_request.len()).expect("family width"),
        [0xd1; 32],
        [0xd2; 32],
        1,
        [0xd3; 32],
    )
    .expect("Hot envelope");
    let mut top_level_data = envelope.to_bytes().to_vec();
    top_level_data.extend_from_slice(&family_request);

    let bank = input_bank(outcome_count, FREEZE_SLOT);
    let bank_digest = ContentId::new(hash(&bank).to_bytes()).expect("bank digest");
    let scalar_count =
        general_hot_scalar_count_v3(Action::Freeze, outcome_count).expect("scalar count");
    let request = AcceleratorRequestV2::new(
        RequestTransportV2::ScratchPages,
        content(1),
        content(2),
        content(3),
        content(4),
        bank_digest,
        outcome_count,
        scalar_count,
        GENERAL_HOT_COMMON_IDENTITIES_V3,
        0,
        &[],
    )
    .expect("accelerator request");
    let mut request_bytes = vec![0_u8; ACCELERATOR_REQUEST_HEADER_BYTES_V2];
    request
        .encode_into(&mut request_bytes)
        .expect("request bytes");
    add_account(&mut test, REQUEST_ACCOUNT, CALLER, request_bytes.clone());

    let page_count = request.chunk_count();
    let mut page_keys = Vec::with_capacity(usize::try_from(page_count).expect("page count"));
    for page_index in 0..page_count {
        let page_request = AcceleratorRequestV2::new(
            RequestTransportV2::ScratchPages,
            content(1),
            content(2),
            content(3),
            content(4),
            bank_digest,
            outcome_count,
            scalar_count,
            GENERAL_HOT_COMMON_IDENTITIES_V3,
            page_index,
            &[],
        )
        .expect("page request");
        let start = usize::try_from(page_request.chunk_offset()).expect("page offset");
        let end = start
            .checked_add(ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2)
            .unwrap_or(bank.len())
            .min(bank.len());
        let page = AuthenticatedScratchPageV2::new(
            ScratchPageKindV2::Input,
            ContentId::new(CALLER.to_bytes()).expect("caller identity"),
            content(1),
            content(4),
            bank_digest,
            outcome_count,
            scalar_count,
            GENERAL_HOT_COMMON_IDENTITIES_V3,
            page_index,
            bank.get(start..end).expect("page payload"),
        )
        .expect("scratch page");
        let mut page_bytes = vec![0_u8; SCRATCH_PAGE_HEADER_BYTES_V2 + page.payload().len()];
        page.encode_into(&mut page_bytes).expect("page bytes");
        if corrupt_page && page_index == page_count - 1 {
            let last = page_bytes.last_mut().expect("page payload byte");
            *last ^= 1;
        }
        let key = page_key(page_index);
        add_account(&mut test, key, CALLER, page_bytes);
        page_keys.push(key);
    }

    let fixed_count = usize::from(
        general_account_profile_fixed_count_v3(Action::Freeze).expect("Freeze account geometry"),
    );
    // `18` was written out four times here, and it is
    // `ADMITTED_RUNTIME_ACCOUNTS_START_V3` -- the same constant the accelerator
    // derives its scratch-page window from, in `assemble_input_bank`. The two
    // agree today; a second author for a frame offset is how they stop
    // agreeing, and the pages sit at the far end of exactly this offset, where
    // a drift would surface as a bank-content refusal rather than as a missing
    // account. The two named coordinates below come from the same table.
    let runtime_start = ADMITTED_RUNTIME_ACCOUNTS_START_V3;
    let mut frame = vec![DUMMY; runtime_start + fixed_count];
    *frame.first_mut().expect("authority frame") = authority;
    *frame
        .get_mut(ADMITTED_INSTRUCTIONS_ACCOUNT_V3)
        .expect("instructions frame") = sysvar::instructions::ID;
    *frame
        .get_mut(ADMITTED_TRADING_PROGRAM_ACCOUNT_V3)
        .expect("Trading frame") = CALLER;
    *frame
        .get_mut(runtime_start + 1)
        .expect("config runtime frame") = CONFIG_ACCOUNT;
    *frame
        .get_mut(runtime_start + 2)
        .expect("Product runtime frame") = PRODUCT_ACCOUNT;
    *frame
        .get_mut(runtime_start + 5)
        .expect("selection runtime frame") = SELECTION_STATE;
    *frame
        .get_mut(runtime_start + usize::from(general_readonly_evidence_start_v3(Action::Freeze)))
        .expect("batch evidence frame") = BATCH_STATE;
    frame.extend(page_keys);
    let mut metas = Vec::with_capacity(frame.len() + 2);
    metas.push(AccountMeta::new_readonly(REQUEST_ACCOUNT, false));
    metas.push(AccountMeta::new_readonly(ACCELERATOR, false));
    metas.extend(
        frame
            .into_iter()
            .map(|key| AccountMeta::new_readonly(key, false)),
    );
    Fixture {
        test,
        instruction: Instruction {
            program_id: CALLER,
            accounts: metas,
            data: top_level_data,
        },
        request_bytes,
        selection_before,
    }
}

/// Submit one transaction, recording it for the census when the tier claims it.
///
/// `label` and `recorded` carry the same discipline as `lifecycle.rs`: this
/// tier's fast lane is claimed at N=1 only, because at N=258 the frame does not
/// fit a Solana packet and ProgramTest is not in a position to notice. The
/// extent is measured here rather than assumed, exactly so a witness can compare
/// it against the stated maximum.
async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    label: &str,
    recorded: bool,
) -> Result<solana_program_test::BanksTransactionResultWithMetadata, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    // The accelerator authenticates that the heap it runs in was actually
    // granted, so a transaction that never asks for one is refused with
    // `InvalidTopLevelInstruction` -- correctly. This file used to send the
    // Trading instruction alone and then assert the execution committed, which
    // is two contradictory claims; it grants the heap now, as every real caller
    // and `lifecycle.rs` already did.
    let transaction = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1),
            instruction,
        ],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let packet_bytes = 1_usize
        .checked_add(64)
        .and_then(|prefix| prefix.checked_add(transaction.message_data().len()))
        .expect("bounded transaction wire");
    let signature = transaction
        .signatures
        .first()
        .copied()
        .expect("a signed transaction has a signature")
        .to_string();
    let slot = context
        .banks_client
        .get_sysvar::<solana_program::clock::Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    if recorded {
        let failure = processed
            .result
            .clone()
            .err()
            .map(|error| format!("{error:?}"));
        let logs = processed
            .metadata
            .as_ref()
            .map_or_else(Vec::new, |metadata| metadata.log_messages.clone());
        let compute_units = processed
            .metadata
            .as_ref()
            .map(|metadata| metadata.compute_units_consumed);
        record(&TransactionEvidence {
            label,
            signature: &signature,
            slot,
            error: failure.as_deref(),
            logs: &logs,
            compute_units_consumed: compute_units,
            wire_bytes: Some(packet_bytes),
        })
        .expect("campaign evidence must be writable when the gauntlet asked for it");
    }
    Ok(processed)
}

fn read_payload_scalar(payload: &[u8], coordinate: u32) -> u64 {
    let start = usize::try_from(coordinate)
        .expect("scalar coordinate")
        .checked_mul(8)
        .expect("scalar offset");
    u64::from_le_bytes(
        payload
            .get(start..start + 8)
            .expect("returned scalar")
            .try_into()
            .expect("u64 bytes"),
    )
}

#[tokio::test]
async fn real_sbf_freeze_accepts_runtime_widths_one_and_258() {
    for outcome_count in [1_u32, 258] {
        let fixture = fixture(outcome_count, false, 0);
        let request = AcceleratorRequestV2::decode(&fixture.request_bytes).expect("request decode");
        let mut context = fixture.test.start_with_context().await;
        let processed = submit(
            &mut context,
            fixture.instruction,
            &format!("general accelerator Freeze at runtime width {outcome_count}"),
            outcome_count == 1,
        )
        .await
        .expect("ProgramTest processing");
        assert!(
            processed.result.is_ok(),
            "real accelerator Freeze must commit"
        );
        let metadata = processed.metadata.expect("transaction metadata");
        let returned = metadata.return_data.expect("typed accelerator ack");
        assert_eq!(returned.program_id, CALLER);
        let ack = AcceleratorAckV2::decode(&returned.data).expect("ack decode");
        assert_eq!(ack.disposition(), AcceleratorDispositionV2::Accepted);
        ack.validate_request(
            request,
            ContentId::new(hash(&fixture.request_bytes).to_bytes()).expect("request digest"),
        )
        .expect("ack/request binding");
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::SELECTION_PHASE),
            u64::from(RuntimeSelectionPhaseV2::Frozen.tag())
        );
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::SELECTION_REVISION),
            2
        );
    }
}

/// A FREEZE THAT PRESENTS SOMEONE ELSE'S BATCH REFUSES, AND SAYS SO.
///
/// `Freeze` names the closed Batch as readonly evidence so its transition can
/// compare the clock against `collection_close + selectionSlots`. That account
/// is caller-supplied, and a deadline read out of an account nobody bound is
/// not a deadline: present any batch whose window has long elapsed and the
/// conjunct passes on a stranger's clock. `GeneralBatchV1::batch_id` recomputes
/// the occurrence identity from the batch's own immutable opening, so the join
/// against the cursor's `batch_id` is the one place a substitution can be
/// caught.
///
/// The substituted account is a REAL batch at a different root sequence, not a
/// malformed one: a fixture that hands over garbage proves only that the
/// decoder refuses garbage. The refusal is read from THIS transaction's own
/// logs -- program logs from one test binary interleave, and the disposition
/// alone would not distinguish this cause from the four other `State` refusals
/// the same path can raise.
#[tokio::test]
async fn a_freeze_presenting_another_batch_refuses_and_leaves_the_selection() {
    let fixture = fixture(1, false, 1);
    let selection_before = fixture.selection_before.clone();
    let mut context = fixture.test.start_with_context().await;
    let processed = submit(
        &mut context,
        fixture.instruction,
        "general accelerator Freeze refuses a batch the cursor does not name",
        false,
    )
    .await
    .expect("ProgramTest processing");
    let metadata = processed.metadata.expect("transaction metadata");
    let returned = metadata.return_data.expect("typed accelerator ack");
    let ack = AcceleratorAckV2::decode(&returned.data).expect("ack decode");
    assert_eq!(ack.disposition(), AcceleratorDispositionV2::Refused);
    let logs = metadata.log_messages;
    assert!(
        logs.iter()
            .any(|line| line
                .contains("general: refused, freeze batch is not the one the cursor names")),
        "the refusal must name the cursor/batch join, not some earlier conjunct: {logs:?}",
    );
    let selection_after = context
        .banks_client
        .get_account(SELECTION_STATE)
        .await
        .expect("selection query")
        .expect("selection account");
    assert_eq!(selection_after.data, selection_before);
}

#[tokio::test]
async fn corrupted_scratch_page_refuses_without_mutating_selection() {
    let fixture = fixture(1, true, 0);
    let selection_key = SELECTION_STATE;
    let selection_before = fixture.selection_before;
    let mut context = fixture.test.start_with_context().await;
    let processed = submit(
        &mut context,
        fixture.instruction,
        "general accelerator Freeze refuses a corrupted scratch page at runtime width 1",
        true,
    )
    .await
    .expect("ProgramTest processing");
    // A bare `is_err()` here would pass on whatever the transaction refused
    // first -- ledger M-38 -- and until the scratch-bank causes were split
    // there was no code to name instead: one `InvalidScratchBank` covered the
    // page privileges, the decode, the request binding, the streamed order and
    // the reassembled bank alike. The fixture flips one byte of the LAST
    // page's payload, so the page still decodes and still binds to this
    // request; what fails is the bank those pages reassemble to.
    assert_eq!(
        format!("{:?}", processed.result),
        format!(
            // Index 1: this file grants the heap frame ahead of the Trading
            // instruction, so the Trading instruction is the second one.
            "Err(InstructionError(1, Custom({})))",
            GeneralAcceleratorSbfErrorV3::ScratchBankDigest as u32
        ),
        "a corrupted page must refuse as a bank-content fault, not as anything else",
    );
    let selection_after = context
        .banks_client
        .get_account(selection_key)
        .await
        .expect("selection query")
        .expect("selection account");
    assert_eq!(selection_after.data, selection_before);
}
