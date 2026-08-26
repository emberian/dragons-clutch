//! Real-ELF N=1/N=258 Freeze execution and malformed-page rollback evidence.

use std::{vec, vec::Vec};

use dclutch_capability_program_contract::hot_v3::HotExecutionEnvelopeV3;
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2, ACCELERATOR_REQUEST_HEADER_BYTES_V2, AcceleratorAckV2,
    AcceleratorDispositionV2, AcceleratorRequestV2, AuthenticatedScratchPageV2, RequestTransportV2,
    SCRATCH_PAGE_HEADER_BYTES_V2, ScratchPageKindV2,
};
use dclutch_general_accelerator_test_caller_sbf::GENERAL_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1;
use dclutch_general_adapter_contract::{
    account_rules_v3::general_account_profile_fixed_count_v3,
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, general_hot_candidate_bank_len_v3,
        general_hot_scalar_count_v3, scalar,
    },
    local_state_v3::{
        GeneralLocalStateHeaderV3, GeneralLocalStateKindV3, encode_general_local_state_v3_atomic,
        general_local_state_len_v3,
    },
    runtime_selection::{
        RUNTIME_SELECTION_CURSOR_BYTES_V2, RuntimeSelectionPhaseV2, consider_verified_candidate_v2,
    },
    runtime_width::{VerifiedCandidateHeaderV2, VerifiedCandidateV2, verified_candidate_len},
};
use dclutch_general_codec::{
    Action, MAX_SELECTION_CRITERIA, SelectionCriterion, SelectionPolicyV1,
    successor_request_v2::ControllerRequestV2,
};
use solana_account::Account;
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
const PRODUCT: [u8; 32] = [0xb1; 32];
const BATCH: [u8; 32] = [0xb2; 32];
const POLICY: [u8; 32] = [0xb3; 32];
const CANDIDATE: [u8; 32] = [0xb4; 32];

struct Fixture {
    test: ProgramTest,
    instruction: Instruction,
    request_bytes: Vec<u8>,
    selection_before: Vec<u8>,
}

fn content(value: u8) -> ContentId {
    ContentId::new([value; 32]).expect("nonzero content")
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
            product_id: PRODUCT,
            batch_id: BATCH,
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

fn input_bank(outcome_count: u32) -> Vec<u8> {
    let mut bank =
        vec![0_u8; general_hot_candidate_bank_len_v3(outcome_count).expect("bank width")];
    write_scalar(&mut bank, scalar::OUTCOME_COUNT, u64::from(outcome_count));
    write_scalar(&mut bank, scalar::SETTLEMENT_POSITION_PRESENT, 0);
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

fn page_key(index: u32) -> Pubkey {
    let byte =
        u8::try_from(index.checked_add(1).expect("page key")).expect("bounded test page count");
    Pubkey::new_from_array([byte; 32])
}

fn fixture(outcome_count: u32, corrupt_page: bool) -> Fixture {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    test.add_program("dclutch_general_accelerator_sbf", ACCELERATOR, None);
    test.add_program("dclutch_general_accelerator_test_caller_sbf", CALLER, None);
    let (authority, _) = Pubkey::find_program_address(
        &[GENERAL_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1],
        &CALLER,
    );
    add_account(&mut test, authority, system_program::ID, Vec::new());
    add_account(&mut test, DUMMY, system_program::ID, Vec::new());
    let selection_before = open_selection(outcome_count);
    add_account(&mut test, SELECTION_STATE, CALLER, selection_before.clone());

    let family_request = ControllerRequestV2 {
        action: Action::Freeze,
        expected_revision: 1,
        candidate_id: None,
        page_index: 0,
        execution_index: 0,
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

    let bank = input_bank(outcome_count);
    let bank_digest = ContentId::new(hash(&bank).to_bytes()).expect("bank digest");
    let scalar_count = general_hot_scalar_count_v3(outcome_count).expect("scalar count");
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
    let mut frame = vec![DUMMY; 18 + fixed_count];
    *frame.first_mut().expect("authority frame") = authority;
    *frame.get_mut(4).expect("instructions frame") = sysvar::instructions::ID;
    *frame.get_mut(5).expect("Trading frame") = CALLER;
    *frame.get_mut(18 + 5).expect("selection runtime frame") = SELECTION_STATE;
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

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
) -> Result<solana_program_test::BanksTransactionResultWithMetadata, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
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
        let fixture = fixture(outcome_count, false);
        let request = AcceleratorRequestV2::decode(&fixture.request_bytes).expect("request decode");
        let mut context = fixture.test.start_with_context().await;
        let processed = submit(&mut context, fixture.instruction)
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

#[tokio::test]
async fn corrupted_scratch_page_refuses_without_mutating_selection() {
    let fixture = fixture(1, true);
    let selection_key = SELECTION_STATE;
    let selection_before = fixture.selection_before;
    let mut context = fixture.test.start_with_context().await;
    let processed = submit(&mut context, fixture.instruction)
        .await
        .expect("ProgramTest processing");
    assert!(processed.result.is_err(), "corrupted page must refuse");
    let selection_after = context
        .banks_client
        .get_account(selection_key)
        .await
        .expect("selection query")
        .expect("selection account");
    assert_eq!(selection_after.data, selection_before);
}
