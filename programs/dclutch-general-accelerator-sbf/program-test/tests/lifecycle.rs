//! Real-ELF General successor lifecycle and adversarial ordering evidence.

use std::{collections::BTreeMap, vec, vec::Vec};

use dclutch_capability_program_contract::hot_v3::HotExecutionEnvelopeV3;
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    AcceleratorAckV2, AcceleratorDispositionV2, AcceleratorRequestV2, AuthenticatedScratchPageV2,
    RequestTransportV2, ScratchPageKindV2, ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2,
    ACCELERATOR_REQUEST_HEADER_BYTES_V2, SCRATCH_PAGE_HEADER_BYTES_V2,
};
use dclutch_general_accelerator_test_caller_sbf::GENERAL_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1;
use dclutch_general_adapter_contract::{
    account_rules_v3::general_account_profile_fixed_count_v3,
    hot_candidate_v3::{
        general_hot_candidate_bank_len_v3, general_hot_scalar_count_v3, identity, scalar,
        GENERAL_HOT_COMMON_IDENTITIES_V3,
    },
    local_state_v3::{
        encode_general_local_state_v3_atomic, general_local_state_len_v3,
        GeneralLocalStateHeaderV3, GeneralLocalStateKindV3,
    },
    runtime_manifest::{settlement_manifest_len_v2, SettlementManifestV2},
    runtime_selection::{
        consider_verified_candidate_v2, freeze_selection_v2, RuntimeSelectionCursorV2,
        RuntimeSelectionPhaseV2, RUNTIME_SELECTION_CURSOR_BYTES_V2,
    },
    runtime_settlement::{
        evaluate_runtime_settlement_v2, initialize_runtime_settlement_v2,
        runtime_settlement_effect_len_v2, RuntimeSettlementActionV2, RuntimeSettlementBuffersV2,
        RuntimeSettlementViewV2,
    },
    runtime_verify::{
        evaluate_runtime_consider_row_with_manifest_v2, runtime_verifier_len_v2,
        AuthenticatedOrderTermsV2, RuntimeConsiderRowBuffersV2, RuntimeConsiderRowViewV2,
        RuntimeManifestBuffersV2,
    },
    runtime_width::{
        candidate_len, execution_len, page_len, settlement_cursor_len, verified_candidate_len,
        CandidateHeaderV2, CandidateV2, ExecutionHeaderV2, ExecutionV2, PageHeaderV2, PageV2,
        SettlementCursorV2, VerifiedCandidateHeaderV2, VerifiedCandidateV2,
    },
    state_artifacts_v3::{
        general_readonly_evidence_v3, GeneralReadonlyEvidenceKindV3,
        GENERAL_PRIMARY_STATE_ACCOUNT_V3,
    },
};
use dclutch_general_codec::{
    successor_request_v2::ControllerRequestV2, Action, SelectionCriterion, SelectionPolicyV1,
    MAX_SELECTION_CRITERIA,
};
use dclutch_general_config_contract::v3::{GeneralConfigV3, GeneralConfigV3Input};
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
const PRODUCT_RECORD: [u8; 64] = [0xb1; 64];
const BATCH: [u8; 32] = [0xb2; 32];
const POLICY: [u8; 32] = [0xb3; 32];
const FIRST_CANDIDATE: [u8; 32] = [0xb4; 32];
const BEST_CANDIDATE: [u8; 32] = [0xb5; 32];
const OWNER: [u8; 32] = [0xc1; 32];
const BENEFICIARY: [u8; 32] = [0xc2; 32];

struct RealSbfFixture {
    test: ProgramTest,
    instruction: Instruction,
    request_bytes: Vec<u8>,
    observed_accounts: Vec<(Pubkey, Vec<u8>)>,
}

struct TerminalFixture {
    width: u32,
    verifier: Vec<u8>,
    verified: Vec<u8>,
    manifests: Vec<Vec<u8>>,
}

fn content(value: u8) -> ContentId {
    ContentId::new([value; 32]).expect("nonzero content")
}

fn product_id() -> [u8; 32] {
    hash(&PRODUCT_RECORD).to_bytes()
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

fn policy() -> SelectionPolicyV1 {
    let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
    criteria[1] = SelectionCriterion::MinimizeQuoteSurplus;
    criteria[2] = SelectionCriterion::MinimizeCandidateId;
    SelectionPolicyV1 {
        policy_id: POLICY,
        criterion_count: 3,
        criteria,
    }
}

fn config(width: u32) -> Vec<u8> {
    config_with_price_scale(u64::from(width))
}

fn config_with_price_scale(price_scale: u64) -> Vec<u8> {
    GeneralConfigV3::new(GeneralConfigV3Input {
        capacity_profile_id: [1; 32],
        claim_basis_id: [2; 32],
        program_set_id: [3; 32],
        generation: 9,
        price_scale,
        collection_slots: 10,
        selection_slots: 10,
        settlement_slots: 10,
        max_orders_per_candidate: 4,
        max_pages_per_candidate: 4,
        continuation_reward_lamports: 1,
        selection_policy_id: POLICY,
        quote_surplus_beneficiary: BENEFICIARY,
    })
    .expect("General config")
    .to_bytes()
    .to_vec()
}

fn verified_candidate(
    width: u32,
    candidate_id: [u8; 32],
    coordinate: u32,
    revision: u64,
    filled_lots: u64,
    quote_debit: u64,
    quote_credit: u64,
) -> Vec<u8> {
    let count = usize::try_from(width).expect("test width");
    let mut output = vec![0_u8; verified_candidate_len(width).expect("verified width")];
    VerifiedCandidateV2::encode_into(
        VerifiedCandidateHeaderV2 {
            outcome_count: width,
            page_count: 1,
            candidate_coordinate: coordinate,
            revision,
            candidate_id,
            product_id: product_id(),
            batch_id: BATCH,
            filled_lots,
            quote_debit,
            quote_credit,
            price_scale: u64::from(width),
        },
        &vec![1; count],
        &vec![1; count],
        &mut output,
    )
    .expect("verified candidate");
    output
}

fn selection_body(before: &[u8], verified: &[u8], expected_revision: u64) -> Vec<u8> {
    let mut scratch = vec![0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let mut output = vec![0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    consider_verified_candidate_v2(
        policy(),
        before,
        verified,
        expected_revision,
        &mut scratch,
        &mut output,
    )
    .expect("selection transition");
    output
}

fn local_state(kind: GeneralLocalStateKindV3, width: u32, body: &[u8]) -> Vec<u8> {
    let state_len = general_local_state_len_v3(kind, width).expect("local state width");
    let mut scratch = vec![0_u8; state_len];
    let mut output = vec![0_u8; state_len];
    encode_general_local_state_v3_atomic(
        GeneralLocalStateHeaderV3 {
            kind,
            bump: 1,
            rent_principal: 99,
            beneficiary: BENEFICIARY,
        },
        body,
        &mut scratch,
        &mut output,
    )
    .expect("local state");
    output
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
    let scalar_bytes = usize::try_from(general_hot_scalar_count_v3(width).expect("scalar count"))
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

fn input_bank(width: u32, action: Action) -> Vec<u8> {
    let mut bank = vec![0_u8; general_hot_candidate_bank_len_v3(width).expect("bank width")];
    write_scalar(&mut bank, scalar::OUTCOME_COUNT, u64::from(width));
    write_scalar(&mut bank, scalar::GENERATION, 9);
    write_scalar(&mut bank, scalar::CLAIMS_MARKET_REVISION, 7);
    write_scalar(&mut bank, scalar::OWNER_POSITION_REVISION, 3);
    write_scalar(
        &mut bank,
        scalar::SETTLEMENT_POSITION_REVISION,
        if action == Action::InitializeSettlement {
            0
        } else {
            5
        },
    );
    write_scalar(&mut bank, scalar::OBSERVED_POSITION_LAMPORTS, 200);
    write_scalar(&mut bank, scalar::OBSERVED_ADMISSION_LAMPORTS, 300);
    write_scalar(&mut bank, scalar::POSITION_RENT_PRINCIPAL, 101);
    write_scalar(&mut bank, scalar::ADMISSION_RENT_PRINCIPAL, 202);
    write_scalar(
        &mut bank,
        scalar::CUSTODY_EXPECTED_REVISION,
        if action == Action::InitializeSettlement {
            0
        } else {
            11
        },
    );
    write_scalar(&mut bank, scalar::TRANSFER_INDEX, 2);
    write_scalar(&mut bank, scalar::CUSTODY_REPLAY_RENT_LAMPORTS, 303);
    write_scalar(&mut bank, scalar::CUSTODY_VAULT_RENT_LAMPORTS, 404);
    let settlement = matches!(
        action,
        Action::Collect | Action::Materialize | Action::Distribute | Action::Close
    );
    write_scalar(
        &mut bank,
        scalar::SETTLEMENT_POSITION_PRESENT,
        u64::from(settlement),
    );
    write_scalar(
        &mut bank,
        scalar::POSITION_TABLE_COUNT,
        if action == Action::Close { 0 } else { 1 },
    );
    for (coordinate, value) in [
        (identity::PARENT_REQUEST_DIGEST, [1; 32]),
        (identity::RELEASE_SET, [2; 32]),
        (identity::MARKET, [3; 32]),
        (identity::PRODUCT_RECORD_DIGEST, product_id()),
        (identity::SEMANTIC_BASIS_ID, [5; 32]),
        (identity::LINKED_BASIS_RECORD_DIGEST, [6; 32]),
        (identity::REALM, [7; 32]),
        (identity::TRADING_PROGRAM, CALLER.to_bytes()),
        (identity::CUSTODY_SOURCE, [15; 32]),
        (identity::CUSTODY_DESTINATION, [16; 32]),
        (identity::MINT, [17; 32]),
        (identity::TOKEN_PROGRAM, [18; 32]),
        (identity::SETTLEMENT_POSITION_OWNER, [9; 32]),
        (identity::RENT_CREDIT, [10; 32]),
        (identity::RENT_PROGRAM, [11; 32]),
        (identity::GENERAL_ROOT, [12; 32]),
    ] {
        write_identity(&mut bank, width, coordinate, value);
    }
    let initialize = action == Action::InitializeSettlement;
    let close = action == Action::Close;
    write_identity(
        &mut bank,
        width,
        identity::PAYER,
        if initialize { [13; 32] } else { [0; 32] },
    );
    write_identity(
        &mut bank,
        width,
        identity::RENT_REFUND,
        if initialize || close {
            [14; 32]
        } else {
            [0; 32]
        },
    );
    let collect = action == Action::Collect;
    let distribute = action == Action::Distribute;
    write_identity(
        &mut bank,
        width,
        identity::CUSTODY_SOURCE_OWNER,
        if collect { [19; 32] } else { [0; 32] },
    );
    write_identity(
        &mut bank,
        width,
        identity::SOURCE_VAULT_CONTEXT,
        if collect { [0; 32] } else { [20; 32] },
    );
    write_identity(
        &mut bank,
        width,
        identity::CUSTODY_DESTINATION_OWNER,
        if distribute || close {
            [21; 32]
        } else {
            [0; 32]
        },
    );
    write_identity(
        &mut bank,
        width,
        identity::DESTINATION_VAULT_CONTEXT,
        if distribute || close {
            [0; 32]
        } else {
            [22; 32]
        },
    );
    bank
}

fn page_key(index: u32) -> Pubkey {
    let byte =
        u8::try_from(index.checked_add(1).expect("page key")).expect("bounded test page count");
    Pubkey::new_from_array([byte; 32])
}

fn runtime_key(action: Action, coordinate: u16) -> Pubkey {
    let mut bytes = [0_u8; 32];
    bytes[0] = 0x70;
    bytes[1] = action as u8;
    bytes[2..4].copy_from_slice(&coordinate.to_le_bytes());
    Pubkey::new_from_array(bytes)
}

fn real_sbf_fixture(
    width: u32,
    controller: ControllerRequestV2,
    bank: Vec<u8>,
    mut runtime_data: BTreeMap<u16, Vec<u8>>,
) -> RealSbfFixture {
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
    runtime_data.entry(1).or_insert_with(|| config(width));
    runtime_data
        .entry(2)
        .or_insert_with(|| PRODUCT_RECORD.to_vec());

    let family_request = controller.to_bytes().expect("controller request");
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

    let bank_digest = ContentId::new(hash(&bank).to_bytes()).expect("bank digest");
    let scalar_count = general_hot_scalar_count_v3(width).expect("scalar count");
    let request = AcceleratorRequestV2::new(
        RequestTransportV2::ScratchPages,
        content(1),
        content(2),
        content(3),
        content(4),
        bank_digest,
        width,
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
            width,
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
            width,
            scalar_count,
            GENERAL_HOT_COMMON_IDENTITIES_V3,
            page_index,
            bank.get(start..end).expect("page payload"),
        )
        .expect("scratch page");
        let mut page_bytes = vec![0_u8; SCRATCH_PAGE_HEADER_BYTES_V2 + page.payload().len()];
        page.encode_into(&mut page_bytes).expect("page bytes");
        let key = page_key(page_index);
        add_account(&mut test, key, CALLER, page_bytes);
        page_keys.push(key);
    }

    let fixed_count = usize::from(
        general_account_profile_fixed_count_v3(controller.action).expect("account geometry"),
    );
    let mut frame = vec![DUMMY; 18 + fixed_count];
    *frame.first_mut().expect("authority frame") = authority;
    *frame.get_mut(4).expect("instructions frame") = sysvar::instructions::ID;
    *frame.get_mut(5).expect("Trading frame") = CALLER;
    let mut observed_accounts = Vec::with_capacity(runtime_data.len());
    for (coordinate, data) in runtime_data {
        let key = runtime_key(controller.action, coordinate);
        add_account(&mut test, key, CALLER, data.clone());
        *frame
            .get_mut(18 + usize::from(coordinate))
            .expect("runtime coordinate") = key;
        observed_accounts.push((key, data));
    }
    frame.extend(page_keys);
    let mut metas = Vec::with_capacity(frame.len() + 2);
    metas.push(AccountMeta::new_readonly(REQUEST_ACCOUNT, false));
    metas.push(AccountMeta::new_readonly(ACCELERATOR, false));
    metas.extend(
        frame
            .into_iter()
            .map(|key| AccountMeta::new_readonly(key, false)),
    );
    RealSbfFixture {
        test,
        instruction: Instruction {
            program_id: CALLER,
            accounts: metas,
            data: top_level_data,
        },
        request_bytes,
        observed_accounts,
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

async fn execute(fixture: RealSbfFixture) -> (AcceleratorAckV2<'static>, ProgramTestContext) {
    let request = AcceleratorRequestV2::decode(&fixture.request_bytes).expect("request decode");
    let request_digest =
        ContentId::new(hash(&fixture.request_bytes).to_bytes()).expect("request digest");
    let observed = fixture.observed_accounts;
    let mut context = fixture.test.start_with_context().await;
    let processed = submit(&mut context, fixture.instruction)
        .await
        .expect("ProgramTest processing");
    assert!(
        processed.result.is_ok(),
        "authenticated transport must execute: {:?}",
        processed.result
    );
    let metadata = processed.metadata.expect("transaction metadata");
    let returned = metadata.return_data.expect("typed accelerator ack");
    assert_eq!(returned.program_id, CALLER);
    let leaked: &'static [u8] = Box::leak(returned.data.into_boxed_slice());
    let ack = AcceleratorAckV2::decode(leaked).expect("ack decode");
    ack.validate_request(request, request_digest)
        .expect("ack/request binding");
    for (key, before) in observed {
        let after = context
            .banks_client
            .get_account(key)
            .await
            .expect("runtime query")
            .expect("runtime account");
        assert_eq!(after.data, before, "accelerator must remain readonly");
    }
    (ack, context)
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

fn evidence_coordinate(action: Action, kind: GeneralReadonlyEvidenceKindV3) -> u16 {
    let mut index = 0_u16;
    loop {
        let evidence = general_readonly_evidence_v3(action, index).expect("evidence coordinate");
        if evidence.kind == kind {
            return evidence.coordinate;
        }
        index = index.checked_add(1).expect("bounded evidence");
    }
}

fn order_id(low: u8) -> [u8; 32] {
    let mut value = [0_u8; 32];
    value[0] = low;
    value
}

fn execution_row(
    width: u32,
    page_coordinate: u32,
    order_low: u8,
    lots: u64,
    receive: &[u64],
    deliver: &[u64],
    debit_limit: u64,
) -> (Vec<u8>, AuthenticatedOrderTermsV2) {
    let terms = AuthenticatedOrderTermsV2 {
        order_id: order_id(order_low),
        owner_id: OWNER,
        nonce: u64::from(order_low),
        max_lots: 10,
        max_quote_debit_per_lot: debit_limit,
    };
    let mut bytes = vec![0_u8; execution_len(width).expect("execution width")];
    ExecutionV2::encode_into(
        ExecutionHeaderV2 {
            outcome_count: width,
            page_coordinate,
            execution_coordinate: 1,
            nonce: terms.nonce,
            order_id: terms.order_id,
            owner_id: terms.owner_id,
            max_lots: terms.max_lots,
            lots,
        },
        receive,
        deliver,
        &mut bytes,
    )
    .expect("execution row");
    (bytes, terms)
}

fn terminal_fixture(width: u32) -> TerminalFixture {
    let count = usize::try_from(width).expect("test width");
    let ones = vec![1; count];
    let zeros = vec![0; count];
    let mut candidate = vec![0_u8; candidate_len(width).expect("candidate width")];
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            outcome_count: width,
            page_count: 3,
            candidate_coordinate: 2,
            price_scale: u64::from(width),
            candidate_id: BEST_CANDIDATE,
            product_id: product_id(),
            batch_id: BATCH,
        },
        &ones,
        &mut candidate,
    )
    .expect("candidate");

    // Each page is deliberately unbalanced. The complete candidate alone has
    // the uniform relation required for a complete-set materialization.
    let rows = [
        execution_row(width, 1, 1, 2, &ones, &zeros, 2),
        execution_row(width, 2, 2, 1, &zeros, &ones, 0),
        execution_row(width, 3, 3, 2, &ones, &zeros, 2),
    ];
    let manifest_counts = [0_u32, 1, 2];
    let cursor_len = runtime_verifier_len_v2(width).expect("verifier width");
    let verified_len = verified_candidate_len(width).expect("verified width");
    let zero_verified = vec![0_u8; verified_len];
    let mut cursor = vec![0_u8; cursor_len];
    let mut verified = zero_verified.clone();
    let mut manifests = Vec::new();
    for (index, (row, terms)) in rows.iter().enumerate() {
        let page_coordinate = u32::try_from(index).expect("page index") + 1;
        let mut page = vec![0_u8; page_len(width, 1).expect("page width")];
        PageV2::encode_into(
            PageHeaderV2 {
                outcome_count: width,
                page_coordinate,
                page_count: 3,
                revision: 11 + u64::try_from(index).expect("page revision"),
                candidate_id: BEST_CANDIDATE,
            },
            &[row],
            &mut page,
        )
        .expect("page");
        let mut cursor_scratch = vec![0_u8; cursor_len];
        let mut cursor_output = vec![0xa5; cursor_len];
        let mut verified_scratch = vec![0_u8; verified_len];
        let mut verified_output = zero_verified.clone();
        let manifest_count = *manifest_counts.get(index).expect("manifest count");
        let manifest_len =
            settlement_manifest_len_v2(width, manifest_count).expect("manifest width");
        let mut manifest_scratch = vec![0_u8; manifest_len];
        let mut manifest_output = vec![0xa5; manifest_len];
        let summary = evaluate_runtime_consider_row_with_manifest_v2(
            RuntimeConsiderRowViewV2 {
                candidate: &candidate,
                page: &page,
                cursor_before: &cursor,
                verified_before: &zero_verified,
                authenticated_order: *terms,
                expected_page_index: u32::try_from(index).expect("page index"),
                expected_row_index: 0,
                expected_page_revision: 11 + u64::try_from(index).expect("page revision"),
                expected_revision: u64::try_from(index).expect("revision"),
                max_orders: 3,
            },
            RuntimeConsiderRowBuffersV2 {
                cursor_scratch: &mut cursor_scratch,
                cursor_output: &mut cursor_output,
                verified_scratch: &mut verified_scratch,
                verified_output: &mut verified_output,
            },
            RuntimeManifestBuffersV2 {
                manifest_scratch: &mut manifest_scratch,
                manifest_output: &mut manifest_output,
            },
        )
        .expect("verified row");
        assert_eq!(summary.complete, index == 2);
        cursor = cursor_output;
        if manifest_count != 0 {
            manifests.push(manifest_output);
        }
        if summary.complete {
            verified = verified_output;
        }
    }
    assert_eq!(manifests.len(), 2);
    TerminalFixture {
        width,
        verifier: cursor,
        verified,
        manifests,
    }
}

fn initialized_cursor(fixture: &TerminalFixture) -> Vec<u8> {
    let cursor_len = settlement_cursor_len(fixture.width).expect("cursor width");
    let mut inventory = vec![0_u8; usize::try_from(fixture.width).expect("width") * 8];
    let mut scratch = vec![0_u8; cursor_len];
    let mut output = vec![0_u8; cursor_len];
    initialize_runtime_settlement_v2(
        &fixture.verifier,
        &fixture.verified,
        0,
        &mut inventory,
        &mut scratch,
        &mut output,
    )
    .expect("initialize settlement");
    output
}

fn settle_native(
    fixture: &TerminalFixture,
    cursor: &[u8],
    action: RuntimeSettlementActionV2,
    manifest: Option<&[u8]>,
    manifest_order_index: u32,
) -> Vec<u8> {
    let cursor_value = SettlementCursorV2::decode(cursor).expect("cursor");
    let cursor_len = cursor.len();
    let effect_len = runtime_settlement_effect_len_v2(fixture.width).expect("effect width");
    let mut cursor_scratch = vec![0_u8; cursor_len];
    let mut cursor_output = vec![0xa5; cursor_len];
    let mut inventory = vec![0_u8; usize::try_from(fixture.width).expect("width") * 8];
    let mut effect_scratch = vec![0_u8; effect_len];
    let mut effect_output = vec![0xa5; effect_len];
    evaluate_runtime_settlement_v2(
        RuntimeSettlementViewV2 {
            action,
            cursor_before: cursor,
            verified: &fixture.verified,
            manifest,
            manifest_order_index,
            expected_revision: cursor_value.header().revision,
            surplus_beneficiary: (action == RuntimeSettlementActionV2::Close)
                .then_some(BENEFICIARY),
        },
        RuntimeSettlementBuffersV2 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_output,
            inventory_scratch: &mut inventory,
            effect_scratch: &mut effect_scratch,
            effect_output: &mut effect_output,
        },
    )
    .expect("native settlement transition");
    cursor_output
}

fn frozen_selection_for_verified(verified: &[u8]) -> Vec<u8> {
    let vacant = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let open = selection_body(&vacant, verified, 0);
    let mut scratch = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let mut frozen = scratch;
    freeze_selection_v2(&open, 1, &mut scratch, &mut frozen).expect("frozen selection");
    frozen.to_vec()
}

fn runtime_for_initialize(fixture: &TerminalFixture) -> BTreeMap<u16, Vec<u8>> {
    let mut runtime = BTreeMap::new();
    runtime.insert(1, config(fixture.width));
    runtime.insert(
        evidence_coordinate(
            Action::InitializeSettlement,
            GeneralReadonlyEvidenceKindV3::FrozenSelection,
        ),
        frozen_selection_for_verified(&fixture.verified),
    );
    runtime.insert(
        evidence_coordinate(
            Action::InitializeSettlement,
            GeneralReadonlyEvidenceKindV3::RuntimeVerifier,
        ),
        fixture.verifier.clone(),
    );
    runtime.insert(
        evidence_coordinate(
            Action::InitializeSettlement,
            GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate,
        ),
        fixture.verified.clone(),
    );
    runtime
}

fn runtime_for_settlement(
    fixture: &TerminalFixture,
    action: Action,
    cursor: &[u8],
    manifest: Option<&[u8]>,
) -> BTreeMap<u16, Vec<u8>> {
    let mut runtime = BTreeMap::new();
    runtime.insert(1, config(fixture.width));
    runtime.insert(
        GENERAL_PRIMARY_STATE_ACCOUNT_V3,
        local_state(GeneralLocalStateKindV3::Settlement, fixture.width, cursor),
    );
    runtime.insert(
        evidence_coordinate(
            action,
            GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate,
        ),
        fixture.verified.clone(),
    );
    if let Some(bytes) = manifest {
        runtime.insert(
            evidence_coordinate(action, GeneralReadonlyEvidenceKindV3::SettlementManifest),
            bytes.to_vec(),
        );
    }
    runtime
}

fn request(
    action: Action,
    revision: u64,
    page_index: u32,
    execution_index: u8,
) -> ControllerRequestV2 {
    request_with_manifest_order(action, revision, page_index, execution_index, 0)
}

fn request_with_manifest_order(
    action: Action,
    revision: u64,
    page_index: u32,
    execution_index: u8,
    manifest_order_index: u8,
) -> ControllerRequestV2 {
    ControllerRequestV2 {
        action,
        expected_revision: revision,
        candidate_id: Some(BEST_CANDIDATE),
        page_index,
        execution_index,
        manifest_order_index,
        state_bump: 1,
        terminal_record_bump: if action == Action::Close { 2 } else { 0 },
    }
}

fn bank_for_request(width: u32, controller: ControllerRequestV2) -> Vec<u8> {
    let mut bank = input_bank(width, controller.action);
    write_scalar(
        &mut bank,
        scalar::PAGE_INDEX,
        u64::from(controller.page_index),
    );
    write_scalar(
        &mut bank,
        scalar::EXECUTION_INDEX,
        u64::from(controller.execution_index),
    );
    write_scalar(
        &mut bank,
        scalar::MANIFEST_ORDER_INDEX,
        u64::from(controller.manifest_order_index),
    );
    bank
}

async fn execute_initialize(fixture: &TerminalFixture) -> AcceleratorDispositionV2 {
    let controller = request(Action::InitializeSettlement, 0, 0, 0);
    let (ack, _) = execute(real_sbf_fixture(
        fixture.width,
        controller,
        bank_for_request(fixture.width, controller),
        runtime_for_initialize(fixture),
    ))
    .await;
    if ack.disposition() == AcceleratorDispositionV2::Accepted {
        let expected_bytes = initialized_cursor(fixture);
        let expected = SettlementCursorV2::decode(&expected_bytes).expect("initialized cursor");
        assert_eq!(read_payload_scalar(ack.payload(), scalar::CURSOR_PHASE), 4);
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::CURSOR_ORDER_COUNT),
            u64::from(expected.header().order_count)
        );
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::CURSOR_RESULTING_REVISION),
            1
        );
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::POSITION_RENT_PRINCIPAL),
            101
        );
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::ADMISSION_RENT_PRINCIPAL),
            202
        );
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::CUSTODY_REPLAY_RENT_LAMPORTS),
            303
        );
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::CUSTODY_VAULT_RENT_LAMPORTS),
            404
        );
    }
    ack.disposition()
}

async fn execute_settlement(
    fixture: &TerminalFixture,
    action: Action,
    cursor: &[u8],
    manifest: Option<&[u8]>,
    page_index: u32,
    execution_index: u8,
    manifest_order_index: u8,
) -> AcceleratorAckV2<'static> {
    let revision = SettlementCursorV2::decode(cursor)
        .expect("settlement cursor")
        .header()
        .revision;
    let controller = request_with_manifest_order(
        action,
        revision,
        page_index,
        execution_index,
        manifest_order_index,
    );
    let (ack, _) = execute(real_sbf_fixture(
        fixture.width,
        controller,
        bank_for_request(fixture.width, controller),
        runtime_for_settlement(fixture, action, cursor, manifest),
    ))
    .await;
    ack
}

#[tokio::test]
async fn real_sbf_consider_replaces_with_best_valid_submitted_candidate_then_freezes() {
    for width in [1_u32, 258] {
        let first = verified_candidate(width, FIRST_CANDIDATE, 1, 1, 7, 7, 0);
        let better = verified_candidate(width, BEST_CANDIDATE, 2, 2, 9, 8, 0);
        let vacant = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
        let before = selection_body(&vacant, &first, 0);
        let expected = selection_body(&before, &better, 1);
        let mut runtime = BTreeMap::new();
        runtime.insert(1, config(width));
        runtime.insert(
            GENERAL_PRIMARY_STATE_ACCOUNT_V3,
            local_state(GeneralLocalStateKindV3::Selection, width, &before),
        );
        runtime.insert(
            evidence_coordinate(
                Action::Consider,
                GeneralReadonlyEvidenceKindV3::SelectionPolicy,
            ),
            policy().to_bytes().expect("policy bytes").to_vec(),
        );
        runtime.insert(
            evidence_coordinate(
                Action::Consider,
                GeneralReadonlyEvidenceKindV3::SubmittedVerifiedCandidate,
            ),
            better.clone(),
        );
        let controller = ControllerRequestV2 {
            action: Action::Consider,
            expected_revision: 1,
            candidate_id: Some(BEST_CANDIDATE),
            page_index: 2,
            execution_index: 0,
            manifest_order_index: 0,
            state_bump: 1,
            terminal_record_bump: 0,
        };
        let (ack, _) = execute(real_sbf_fixture(
            width,
            controller,
            input_bank(width, Action::Consider),
            runtime,
        ))
        .await;
        assert_eq!(ack.disposition(), AcceleratorDispositionV2::Accepted);
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::SELECTION_SUBMITTED_COUNT),
            2
        );
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::SELECTION_BEST_CANDIDATE_COORDINATE),
            2
        );
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::SELECTION_BEST_FILLED_LOTS),
            9
        );
        let selected = RuntimeSelectionCursorV2::decode(&expected).expect("selected cursor");
        assert_eq!(selected.header().best_candidate_id, BEST_CANDIDATE);

        let mut frozen = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
        let mut scratch = frozen;
        freeze_selection_v2(&expected, 2, &mut scratch, &mut frozen).expect("freeze selection");
        let mut runtime = BTreeMap::new();
        runtime.insert(
            GENERAL_PRIMARY_STATE_ACCOUNT_V3,
            local_state(GeneralLocalStateKindV3::Selection, width, &expected),
        );
        let controller = ControllerRequestV2 {
            action: Action::Freeze,
            expected_revision: 2,
            candidate_id: None,
            page_index: 0,
            execution_index: 0,
            manifest_order_index: 0,
            state_bump: 1,
            terminal_record_bump: 0,
        };
        let (ack, _) = execute(real_sbf_fixture(
            width,
            controller,
            input_bank(width, Action::Freeze),
            runtime,
        ))
        .await;
        assert_eq!(ack.disposition(), AcceleratorDispositionV2::Accepted);
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::SELECTION_PHASE),
            u64::from(RuntimeSelectionPhaseV2::Frozen.tag())
        );
        assert_eq!(
            RuntimeSelectionCursorV2::decode(&frozen)
                .expect("frozen cursor")
                .header()
                .best_candidate_id,
            BEST_CANDIDATE
        );
    }
}

async fn run_full_settlement_lifecycle(width: u32) {
    let fixture = terminal_fixture(width);
    assert_eq!(
        execute_initialize(&fixture).await,
        AcceleratorDispositionV2::Accepted
    );
    let first_manifest =
        SettlementManifestV2::decode(fixture.manifests.first().expect("first manifest bytes"))
            .expect("first manifest");
    let final_manifest =
        SettlementManifestV2::decode(fixture.manifests.get(1).expect("final manifest bytes"))
            .expect("final manifest");
    let row_sources = [
        (first_manifest.as_bytes(), 0_u8),
        (final_manifest.as_bytes(), 0),
        (final_manifest.as_bytes(), 1),
    ];
    let rows = row_sources.map(|(manifest_bytes, manifest_order_index)| {
        let manifest = SettlementManifestV2::decode(manifest_bytes).expect("manifest");
        let selected = manifest
            .order(u32::from(manifest_order_index))
            .expect("selected manifest row");
        (
            manifest_bytes,
            manifest_order_index,
            selected.header().source_page_index,
            u8::try_from(selected.header().source_execution_index).expect("source execution"),
        )
    });
    assert_eq!(rows[2].1, 1);
    assert_eq!(rows[2].2, 2);
    assert_eq!(rows[2].3, 0);
    let mut cursor = initialized_cursor(&fixture);

    // The manifest ordinal and source coordinates are distinct authenticated
    // facts. Row zero originated on source page zero; substituting the old
    // one-based/page-derived value must refuse without any runtime write.
    let substituted_source = execute_settlement(
        &fixture,
        Action::Collect,
        &cursor,
        Some(first_manifest.as_bytes()),
        1,
        0,
        0,
    )
    .await;
    assert_eq!(
        substituted_source.disposition(),
        AcceleratorDispositionV2::Refused
    );

    // A caller cannot skip to order three. Semantic refusal returns a typed
    // refusal and the readonly cursor/manifest accounts remain byte-identical.
    let refused = execute_settlement(
        &fixture,
        Action::Collect,
        &cursor,
        Some(final_manifest.as_bytes()),
        2,
        0,
        1,
    )
    .await;
    assert_eq!(refused.disposition(), AcceleratorDispositionV2::Refused);

    for (expected_coordinate, (manifest, manifest_order, page_index, execution_index)) in
        rows.iter().enumerate()
    {
        let ack = execute_settlement(
            &fixture,
            Action::Collect,
            &cursor,
            Some(manifest),
            *page_index,
            *execution_index,
            *manifest_order,
        )
        .await;
        assert_eq!(ack.disposition(), AcceleratorDispositionV2::Accepted);
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::ORDER_COORDINATE),
            u64::try_from(expected_coordinate).expect("order coordinate") + 1
        );
        cursor = settle_native(
            &fixture,
            &cursor,
            RuntimeSettlementActionV2::Collect,
            Some(manifest),
            u32::from(*manifest_order),
        );
        let expected = SettlementCursorV2::decode(&cursor).expect("collected cursor");
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::CURSOR_NEXT_ORDER),
            u64::from(expected.header().next_order)
        );
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::CURSOR_QUOTE_INVENTORY),
            expected.header().quote_inventory
        );
    }
    assert_eq!(
        read_payload_scalar(
            execute_settlement(&fixture, Action::Materialize, &cursor, None, 0, 0, 0)
                .await
                .payload(),
            scalar::CURSOR_PHASE
        ),
        6
    );
    cursor = settle_native(
        &fixture,
        &cursor,
        RuntimeSettlementActionV2::Materialize,
        None,
        0,
    );

    for (expected_coordinate, (manifest, manifest_order, page_index, execution_index)) in
        rows.iter().enumerate()
    {
        let ack = execute_settlement(
            &fixture,
            Action::Distribute,
            &cursor,
            Some(manifest),
            *page_index,
            *execution_index,
            *manifest_order,
        )
        .await;
        assert_eq!(ack.disposition(), AcceleratorDispositionV2::Accepted);
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::ORDER_COORDINATE),
            u64::try_from(expected_coordinate).expect("order coordinate") + 1
        );
        cursor = settle_native(
            &fixture,
            &cursor,
            RuntimeSettlementActionV2::Distribute,
            Some(manifest),
            u32::from(*manifest_order),
        );
    }
    let ready = SettlementCursorV2::decode(&cursor).expect("ready cursor");
    assert_eq!(ready.header().quote_inventory, 0);
    assert!((0..fixture.width).all(|outcome| ready.inventory(outcome).expect("inventory") == 0));

    // A late child-precondition failure (nonzero Position table on terminal
    // close) refuses the entire candidate before any observed account changes.
    let close_request = request(Action::Close, ready.header().revision, 0, 0);
    let mut hostile_bank = bank_for_request(fixture.width, close_request);
    write_scalar(&mut hostile_bank, scalar::POSITION_TABLE_COUNT, 1);
    let (refused, _) = execute(real_sbf_fixture(
        fixture.width,
        close_request,
        hostile_bank,
        runtime_for_settlement(&fixture, Action::Close, &cursor, None),
    ))
    .await;
    assert_eq!(refused.disposition(), AcceleratorDispositionV2::Refused);

    let ack = execute_settlement(&fixture, Action::Close, &cursor, None, 0, 0, 0).await;
    assert_eq!(ack.disposition(), AcceleratorDispositionV2::Accepted);
    assert_eq!(read_payload_scalar(ack.payload(), scalar::TERMINAL), 1);
    assert_eq!(read_payload_scalar(ack.payload(), scalar::CURSOR_PHASE), 8);
    assert_eq!(
        read_payload_scalar(ack.payload(), scalar::CURSOR_QUOTE_INVENTORY),
        0
    );
    assert_eq!(
        read_payload_scalar(ack.payload(), scalar::POSITION_TABLE_COUNT),
        0
    );
    assert_eq!(
        read_payload_scalar(ack.payload(), scalar::CURSOR_TERMINAL_COORDINATE),
        ready.header().revision + 1
    );
    assert_eq!(
        read_payload_scalar(ack.payload(), scalar::POSITION_RENT_PRINCIPAL),
        101
    );
    assert_eq!(
        read_payload_scalar(ack.payload(), scalar::ADMISSION_RENT_PRINCIPAL),
        202
    );
    assert_eq!(
        read_payload_scalar(ack.payload(), scalar::CUSTODY_REPLAY_RENT_LAMPORTS),
        303
    );
    assert_eq!(
        read_payload_scalar(ack.payload(), scalar::CUSTODY_VAULT_RENT_LAMPORTS),
        404
    );
}

#[tokio::test]
async fn real_sbf_runs_full_settlement_at_runtime_widths_one_and_258() {
    for width in [1_u32, 258] {
        run_full_settlement_lifecycle(width).await;
    }
}

#[tokio::test]
async fn hostile_n258_initializes_and_refuses_candidate_substitution() {
    let fixture = terminal_fixture(258);
    assert_eq!(
        execute_initialize(&fixture).await,
        AcceleratorDispositionV2::Accepted
    );
    let cursor = initialized_cursor(&fixture);
    let final_manifest =
        SettlementManifestV2::decode(fixture.manifests.get(1).expect("final manifest bytes"))
            .expect("final manifest");
    let out_of_order = execute_settlement(
        &fixture,
        Action::Collect,
        &cursor,
        Some(final_manifest.as_bytes()),
        2,
        0,
        1,
    )
    .await;
    assert_eq!(
        out_of_order.disposition(),
        AcceleratorDispositionV2::Refused
    );
    let controller = request_with_manifest_order(Action::Collect, 1, 2, 0, 1);
    let mut runtime = runtime_for_settlement(
        &fixture,
        Action::Collect,
        &cursor,
        Some(final_manifest.as_bytes()),
    );
    runtime.insert(
        evidence_coordinate(
            Action::Collect,
            GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate,
        ),
        verified_candidate(258, FIRST_CANDIDATE, 1, 3, 9, 8, 0),
    );
    let (ack, _) = execute(real_sbf_fixture(
        fixture.width,
        controller,
        bank_for_request(fixture.width, controller),
        runtime,
    ))
    .await;
    assert_eq!(ack.disposition(), AcceleratorDispositionV2::Refused);
}

#[tokio::test]
async fn product_or_price_scale_substitution_refuses_without_runtime_writes() {
    for width in [1_u32, 258] {
        let fixture = terminal_fixture(width);
        let controller = request(Action::InitializeSettlement, 0, 0, 0);

        let mut substituted_product = runtime_for_initialize(&fixture);
        substituted_product.insert(2, vec![0xcc; PRODUCT_RECORD.len()]);
        let (ack, _) = execute(real_sbf_fixture(
            width,
            controller,
            bank_for_request(width, controller),
            substituted_product,
        ))
        .await;
        assert_eq!(ack.disposition(), AcceleratorDispositionV2::Refused);

        let mut substituted_scale = runtime_for_initialize(&fixture);
        substituted_scale.insert(
            1,
            config_with_price_scale(u64::from(width).checked_add(1).expect("scale")),
        );
        let (ack, _) = execute(real_sbf_fixture(
            width,
            controller,
            bank_for_request(width, controller),
            substituted_scale,
        ))
        .await;
        assert_eq!(ack.disposition(), AcceleratorDispositionV2::Refused);
    }
}
