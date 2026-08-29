//! Real-ELF General successor lifecycle and adversarial ordering evidence.

use std::{collections::BTreeMap, vec, vec::Vec};

use dclutch_capability_program_contract::hot_v3::HotExecutionEnvelopeV3;
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::admitted_v3::ADMITTED_RUNTIME_ACCOUNTS_START_V3;
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2, ACCELERATOR_REQUEST_HEADER_BYTES_V2, AcceleratorAckV2,
    AcceleratorDispositionV2, AcceleratorRequestV2, AuthenticatedScratchPageV2, RequestTransportV2,
    SCRATCH_PAGE_HEADER_BYTES_V2, ScratchPageKindV2,
};
use dclutch_general_accelerator_test_caller_sbf::GENERAL_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1;
use dclutch_general_adapter_contract::{
    account_rules_v3::general_account_profile_fixed_count_v3,
    candidate_v1::{
        CandidateVerifyRowBuffersV1, CandidateVerifyRowViewV1, GeneralCandidateOpeningV1,
        GeneralCandidateV1, authenticate_candidate_identity_v1, general_candidate_identity_v1,
        verify_candidate_row_v1,
    },
    collection_v1::{
        EscrowDirectionV1, GeneralBatchOpeningV1, GeneralBatchV1, GeneralOrderHeaderV1,
        GeneralOrderPhaseV1, GeneralOrderStateV1, GeneralOrderV1, MakerFundingV1,
        authenticate_batch_candidate_v1, authenticate_order_execution_v1, general_order_len_v1,
    },
    escrow_v1::{
        OrderEscrowObservationV1, OrderEscrowPlanV1, WorkEscrowClosePlanV1, WorkEscrowDrawPlanV1,
        WorkEscrowFundingPlanV1, WorkEscrowObservationV1, authenticate_collect_from_escrow_v1,
        authenticate_order_escrow_claims_v1, authenticate_work_escrow_v1,
        work_escrow_required_lamports_v1,
    },
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, general_hot_candidate_bank_len_v3,
        general_hot_scalar_count_v3, identity, scalar,
    },
    local_state_v3::{
        GeneralLocalStateHeaderV3, GeneralLocalStateKindV3, encode_general_local_state_v3_atomic,
        general_local_state_len_v3,
    },
    runtime_manifest::{SettlementManifestV2, settlement_manifest_len_v2},
    runtime_selection::{
        RUNTIME_SELECTION_CURSOR_BYTES_V2, RuntimeSelectionCursorV2, RuntimeSelectionPhaseV2,
        consider_verified_candidate_v2, freeze_selection_v2,
    },
    runtime_settlement::{
        RuntimeSettlementActionV2, RuntimeSettlementBuffersV2, RuntimeSettlementEffectPlanV2,
        RuntimeSettlementViewV2, evaluate_runtime_settlement_v2, initialize_runtime_settlement_v2,
        runtime_settlement_effect_len_v2,
    },
    runtime_verify::{AuthenticatedOrderTermsV2, runtime_verifier_len_v2},
    runtime_width::{
        CandidateHeaderV2, CandidateV2, ExecutionHeaderV2, ExecutionV2, PageHeaderV2, PageV2,
        SettlementCursorV2, VerifiedCandidateHeaderV2, VerifiedCandidateV2, candidate_len,
        execution_len, page_len, settlement_cursor_len, verified_candidate_len,
    },
    state_artifacts_v3::{
        GENERAL_PRIMARY_STATE_ACCOUNT_V3, GeneralReadonlyEvidenceKindV3,
        general_readonly_evidence_v3,
    },
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
const MARKET: [u8; 32] = [0xb2; 32];
const CONFIG_IDENTITY: [u8; 32] = [0xb6; 32];
const GENERATION: u64 = 7;
const COLLECTION_CLOSE_SLOT: u64 = 1_000;
const SETTLEMENT_CLOSE_SLOT: u64 = 2_000;
const ADMISSION_SLOT: u64 = 10;
const BATCH_MAX_ORDERS: u32 = 8;
const POLICY: [u8; 32] = [0xb3; 32];
const FIRST_CANDIDATE: [u8; 32] = [0xb4; 32];
const BEST_CANDIDATE: [u8; 32] = [0xb5; 32];
const OWNER: [u8; 32] = [0xc1; 32];
const SOLVER: [u8; 32] = [0xc3; 32];
const SUBMISSION_SLOT: u64 = 1_100;
const CANDIDATE_PAGE_REVISION: u64 = 11;
const CRANK_REWARD_LAMPORTS: u64 = 5_000;
const BENEFICIARY: [u8; 32] = [0xc2; 32];

struct RealSbfFixture {
    test: ProgramTest,
    instruction: Instruction,
    request_bytes: Vec<u8>,
    observed_accounts: Vec<(Pubkey, Vec<u8>)>,
    /// Census label for this transaction, and whether it is recorded.
    ///
    /// `recorded` is false at every runtime width but one, and that is the
    /// fast-lane packet clause rather than thrift: six of the seven actions
    /// serialise to 1,273-1,329 legacy bytes at N=258 against Solana's 1,232
    /// maximum, and ProgramTest submits no packet, so a census row folded from
    /// an N=258 transaction would be a route recorded as executed on a frame no
    /// validator would accept. At N=1 every packet is 745-867 bytes and the
    /// clause holds, so that is the width this tier claims. The N=258
    /// transactions still run, and their measured extents are the evidence that
    /// the clause fails there.
    label: String,
    recorded: bool,
}

struct TerminalFixture {
    width: u32,
    verifier: Vec<u8>,
    verified: Vec<u8>,
    manifests: Vec<Vec<u8>>,
    /// The candidate's OWN digest, not a chosen literal.
    candidate_id: [u8; 32],
    /// The submission record the verification advanced and paid out of.
    submission: GeneralCandidateV1,
    /// The closed batch, so a settlement row can be checked against the escrow
    /// its order actually holds rather than against a declared reserve.
    batch: GeneralBatchV1,
    /// The order records this batch admitted, in the candidate's row order.
    orders: Vec<Vec<u8>>,
    /// Every balance the escrow moved, carried through the whole campaign.
    escrow: EscrowLedgerV1,
}

/// Rent-exempt minimum for one 224-byte submission record, at the current Rent.
///
/// It is a floor rather than a compartment: the work escrow may never be drawn
/// into it, or the record it lives in becomes collectable.
const SUBMISSION_RENT_FLOOR: u64 = 2_449_680;
/// Lamports the solver holds before funding one submission.
const SOLVER_LAMPORTS: u64 = 1_000_000_000;
/// Quote atoms each maker holds before their order is admitted.
const MAKER_QUOTE_ATOMS: u64 = 1_000_000;

/// Every balance General's escrow touches, tracked across the whole campaign.
///
/// **This is what "the escrow moves" means for a campaign that has no lamport
/// mover.** Decision 0010 §6 item 3 records the work escrow as accounted and not
/// moved, and the sharper half is that nothing tied the accounting to a balance
/// at all. Here every transition of the real lifecycle constructs its exact
/// movement plan against an observed balance, applies it, and the plan's own
/// postcondition check is what advances the ledger -- so a step whose record and
/// whose balance disagree cannot be applied at all.
///
/// What it is NOT: a claim that lamports moved on chain. The seven collection and
/// candidate actions have no artifact triple, so no on-chain route exists to
/// carry these movements yet; the settlement half below is what runs on the real
/// ELF, and the escrow it draws on is the one this ledger holds.
#[derive(Clone, Debug)]
struct EscrowLedgerV1 {
    /// Lamports in the submission record's account.
    work_escrow: u64,
    /// Lamports held by the solver who funded the submission.
    solver: u64,
    /// Lamports paid out to the actors who performed cranks.
    cranked: u64,
    /// Quote atoms in each order's own escrow vault, keyed by `order_id`.
    vaults: BTreeMap<[u8; 32], u64>,
    /// Quote atoms each maker holds outside the protocol, keyed by `order_id`.
    makers: BTreeMap<[u8; 32], u64>,
    /// Quote atoms the settlement inventory has collected out of the escrows.
    settlement: u64,
}

impl EscrowLedgerV1 {
    fn new() -> Self {
        Self {
            work_escrow: 0,
            solver: SOLVER_LAMPORTS,
            cranked: 0,
            vaults: BTreeMap::new(),
            makers: BTreeMap::new(),
            settlement: 0,
        }
    }

    /// The observation one work-escrow transition reads.
    fn work_observation(&self, beneficiary: u64) -> WorkEscrowObservationV1 {
        WorkEscrowObservationV1 {
            escrow_lamports: self.work_escrow,
            rent_floor: SUBMISSION_RENT_FLOOR,
            beneficiary_lamports: beneficiary,
        }
    }

    /// Every lamport this ledger has ever held, which never changes.
    fn total_lamports(&self) -> u64 {
        self.work_escrow + self.solver + self.cranked
    }

    /// Every quote atom this ledger has ever held, which never changes.
    fn total_atoms(&self) -> u64 {
        self.vaults.values().sum::<u64>() + self.makers.values().sum::<u64>() + self.settlement
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExecutionEvidence {
    action: Action,
    outcome_count: u32,
    compute_units: u64,
    instruction_accounts: usize,
    packet_bytes: usize,
    scratch_pages: u32,
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
            batch_id: batch_id(width),
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
    // The runtime coordinates start where the contract says they start. This
    // read `18` until 2026-08-29, which is the same defect class as the frame
    // count asserted in `assert_execution_evidence`: a restated constant is one
    // the code can stop agreeing with silently.
    let mut frame = vec![DUMMY; ADMITTED_RUNTIME_ACCOUNTS_START_V3 + fixed_count];
    *frame.first_mut().expect("authority frame") = authority;
    *frame.get_mut(4).expect("instructions frame") = sysvar::instructions::ID;
    *frame.get_mut(5).expect("Trading frame") = CALLER;
    let mut observed_accounts = Vec::with_capacity(runtime_data.len());
    for (coordinate, data) in runtime_data {
        let key = runtime_key(controller.action, coordinate);
        add_account(&mut test, key, CALLER, data.clone());
        *frame
            .get_mut(ADMITTED_RUNTIME_ACCOUNTS_START_V3 + usize::from(coordinate))
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
        // The label a census binding matches. It names the action and the
        // width and nothing else: which DISPOSITION the accelerator returned is
        // not a property the transaction has, because a semantic refusal comes
        // back as a typed ack on a SUCCEEDING transaction, and binding on a
        // disposition the census cannot see would be a label asserting
        // something its own evidence does not carry.
        label: format!(
            "general accelerator {:?} at runtime width {width}",
            controller.action
        ),
        recorded: width == 1,
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    label: &str,
    recorded: bool,
) -> Result<
    (
        solana_program_test::BanksTransactionResultWithMetadata,
        usize,
        usize,
    ),
    BanksClientError,
> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let instruction_accounts = instruction.accounts.len();
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    // One short-vector signature count byte, one signature, and the exact
    // canonical message bytes. ProgramTest uses the same legacy packet wire.
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
            // Measured, never assumed: ProgramTest submits no packet, so the
            // tier serialises the transaction itself and records the extent for
            // a witness to compare against the stated 1,232-byte maximum.
            wire_bytes: Some(packet_bytes),
        })
        .expect("campaign evidence must be writable when the gauntlet asked for it");
    }
    Ok((processed, instruction_accounts, packet_bytes))
}

async fn execute(
    fixture: RealSbfFixture,
) -> (
    AcceleratorAckV2<'static>,
    ProgramTestContext,
    ExecutionEvidence,
) {
    let request = AcceleratorRequestV2::decode(&fixture.request_bytes).expect("request decode");
    let (_, family_request) =
        HotExecutionEnvelopeV3::split_instruction(&fixture.instruction.data).expect("Hot request");
    let controller = ControllerRequestV2::decode(family_request).expect("controller request");
    let request_digest =
        ContentId::new(hash(&fixture.request_bytes).to_bytes()).expect("request digest");
    let observed = fixture.observed_accounts;
    let mut context = fixture.test.start_with_context().await;
    let (processed, instruction_accounts, packet_bytes) = submit(
        &mut context,
        fixture.instruction,
        &fixture.label,
        fixture.recorded,
    )
    .await
    .expect("ProgramTest processing");
    assert!(
        processed.result.is_ok(),
        "authenticated transport must execute: {:?}",
        processed.result
    );
    let metadata = processed.metadata.expect("transaction metadata");
    let compute_units = metadata.compute_units_consumed;
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
    let evidence = ExecutionEvidence {
        action: controller.action,
        outcome_count: request.tail_count(),
        compute_units,
        instruction_accounts,
        packet_bytes,
        scratch_pages: request.chunk_count(),
    };
    (ack, context, evidence)
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

/// The immutable opening of the one batch every candidate below names.
fn batch_opening(width: u32) -> GeneralBatchOpeningV1 {
    GeneralBatchOpeningV1 {
        outcome_count: width,
        sequence: 0,
        generation: GENERATION,
        market: MARKET,
        product_id: product_id(),
        config_id: CONFIG_IDENTITY,
        price_scale: u64::from(width),
        collection_close_slot: COLLECTION_CLOSE_SLOT,
        settlement_close_slot: SETTLEMENT_CLOSE_SLOT,
        max_orders: BATCH_MAX_ORDERS,
    }
}

/// Open one real batch against one real root.
///
/// This is the whole point of the collection half: `batch_id` is no longer a
/// literal. It is the digest of an opening that consumed the root's exact next
/// sequence, so a candidate naming it is naming a batch that was really opened.
fn opened_batch(width: u32) -> (GeneralRootV2, GeneralBatchV1) {
    let mut root =
        GeneralRootV2::active(MARKET, CONFIG_IDENTITY, GENERATION).expect("active General root");
    let revision = root.revision();
    let batch = GeneralBatchV1::open(&mut root, batch_opening(width), revision, ADMISSION_SLOT)
        .expect("open batch");
    (root, batch)
}

fn batch_id(width: u32) -> [u8; 32] {
    opened_batch(width).1.batch_id()
}

/// One maker's signed portfolio order, as it would be placed into the batch.
struct OrderSpec {
    nonce: u64,
    lots: u64,
    receive: Vec<u64>,
    deliver: Vec<u64>,
    debit_limit: u64,
}

fn order_record(width: u32, batch_id: [u8; 32], spec: &OrderSpec) -> Vec<u8> {
    let mut bytes = vec![0_u8; general_order_len_v1(width).expect("order width")];
    GeneralOrderV1::encode_into(
        GeneralOrderHeaderV1 {
            outcome_count: width,
            nonce: spec.nonce,
            owner_id: OWNER,
            market: MARKET,
            batch_id,
            generation: GENERATION,
            max_lots: 10,
            max_quote_debit_per_lot: spec.debit_limit,
            valid_until_slot: SETTLEMENT_CLOSE_SLOT,
        },
        &spec.receive,
        &spec.deliver,
        GeneralOrderStateV1 {
            phase: GeneralOrderPhaseV1::Placed,
            admitted_slot: ADMISSION_SLOT,
            released_slot: 0,
        },
        &mut bytes,
    )
    .expect("order record");
    bytes
}

/// Build one compact Execution row from the immutable order it names.
///
/// The row's terms are not asserted here; they are returned by
/// `authenticate_order_execution_v1`, which checks every field the row repeats
/// against the record and checks the record's own digest against the `order_id`
/// the row claims.
fn execution_row(
    width: u32,
    page_coordinate: u32,
    batch: GeneralBatchV1,
    order_bytes: &[u8],
    lots: u64,
) -> (Vec<u8>, AuthenticatedOrderTermsV2) {
    let order = GeneralOrderV1::decode(order_bytes).expect("order record");
    let header = order.header();
    let receive: Vec<u64> = (0..width)
        .map(|index| order.receive_per_lot(index).expect("receive"))
        .collect();
    let deliver: Vec<u64> = (0..width)
        .map(|index| order.deliver_per_lot(index).expect("deliver"))
        .collect();
    let execution = ExecutionHeaderV2 {
        outcome_count: width,
        page_coordinate,
        execution_coordinate: 1,
        nonce: header.nonce,
        order_id: order.order_id(),
        owner_id: header.owner_id,
        max_lots: header.max_lots,
        lots,
    };
    let mut bytes = vec![0_u8; execution_len(width).expect("execution width")];
    ExecutionV2::encode_into(execution, &receive, &deliver, &mut bytes).expect("execution row");
    // The row is authenticated as a whole record, tails included: the per-lot
    // vectors are part of what the order record binds, so a header-only
    // authentication could not see a substituted portfolio.
    let terms = authenticate_order_execution_v1(
        batch,
        order,
        ExecutionV2::decode(&bytes).expect("row decodes"),
    )
    .expect("execution row authenticates against its order record");
    (bytes, terms)
}

fn terminal_fixture(width: u32) -> TerminalFixture {
    let count = usize::try_from(width).expect("test width");
    let ones = vec![1; count];
    let zeros = vec![0; count];

    // The real collection half: open a batch, place three signed orders into
    // it, close it. The batch identity below is the digest of that opening and
    // the terms below are projections of those order records.
    let (mut root, mut batch) = opened_batch(width);
    let specs = [
        OrderSpec {
            nonce: 1,
            lots: 2,
            receive: ones.clone(),
            deliver: zeros.clone(),
            debit_limit: 2,
        },
        OrderSpec {
            nonce: 2,
            lots: 1,
            receive: zeros.clone(),
            deliver: ones.clone(),
            debit_limit: 0,
        },
        OrderSpec {
            nonce: 3,
            lots: 2,
            receive: ones.clone(),
            deliver: zeros.clone(),
            debit_limit: 2,
        },
    ];
    let identity = batch.batch_id();
    let claims = vec![u64::MAX / 4; count];
    let mut ledger = EscrowLedgerV1::new();
    let mut placed: Vec<(Vec<u8>, u64)> = Vec::new();
    for spec in &specs {
        let bytes = order_record(width, identity, spec);
        let order = GeneralOrderV1::decode(&bytes).expect("order record");
        let escrow = batch
            .admit(
                order,
                MakerFundingV1 {
                    owner_id: OWNER,
                    available_quote: u64::MAX / 4,
                    available_claims: &claims,
                },
                ADMISSION_SLOT,
            )
            .expect("admit order");
        // ADMISSION MOVES THE ATOMS. Before this the escrow was a value the
        // transition returned and nothing carried; here the movement is planned
        // against the vault's observed balance, and the plan's own
        // postcondition is what advances the ledger.
        let order_id = order.order_id();
        let plan = OrderEscrowPlanV1::new(
            batch,
            order,
            escrow,
            OrderEscrowObservationV1 {
                escrow_context: order_id,
                vault_quote_atoms: 0,
                maker_quote_atoms: MAKER_QUOTE_ATOMS,
            },
        )
        .expect("admission escrow plan");
        for outcome in 0..width {
            authenticate_order_escrow_claims_v1(order, EscrowDirectionV1::Deposit, outcome, 0)
                .expect("a fresh escrow Position holds nothing at this outcome");
        }
        plan.validate_post(plan.vault_after(), plan.maker_after())
            .expect("the admission movement is exactly the planned one");
        assert_eq!(plan.vault_after(), order.quote_reserve().expect("reserve"));
        ledger.vaults.insert(order_id, plan.vault_after());
        ledger.makers.insert(order_id, plan.maker_after());
        placed.push((bytes, spec.lots));
    }
    assert_eq!(batch.state().order_count, 3);
    // The batch's counter is no longer a sum of promises: it is the sum of the
    // balances the protocol is holding, and that is now checkable.
    assert_eq!(
        batch.state().committed_quote_reserve,
        ledger.vaults.values().sum::<u64>(),
    );
    let revision = root.revision();
    let closed_identity = batch.close(&mut root, revision).expect("close batch");
    assert_eq!(closed_identity, identity);
    assert_eq!(root.open_batches(), 0);

    // Candidate rows must be globally grouped by increasing order identity, and
    // a real `order_id` is a digest rather than a chosen small integer. So the
    // candidate builder sorts by identity; the candidate-wide aggregate this
    // fixture depends on is invariant under that permutation.
    //
    // The order is the verifier's own: `runtime_verify::le_numeric_id` reads a
    // 32-byte identity as a LITTLE-ENDIAN 256-bit integer, which is not the
    // lexicographic order of `[u8; 32]`. The previous fixture could not tell the
    // two apart -- its identities were `[low, 0, 0, ...]`, where every high byte
    // is zero and both orders agree. A real digest distinguishes them, and
    // sorting the wrong way refuses with `NonCanonicalOrder`.
    placed.sort_by(|left, right| {
        let left = GeneralOrderV1::decode(&left.0).expect("order").order_id();
        let right = GeneralOrderV1::decode(&right.0).expect("order").order_id();
        left.iter().rev().cmp(right.iter().rev())
    });

    // The candidate carries its OWN digest as its identity. `CandidateV2`
    // treats `candidate_id` as a declared field and checks nothing about it, so
    // a literal here -- which is what this fixture used -- is a candidate that
    // could have named any identity at all, including one already verified
    // under other prices. Encode once to fix every other byte, then re-encode
    // with the digest those bytes produce.
    let mut candidate = vec![0_u8; candidate_len(width).expect("candidate width")];
    let header = CandidateHeaderV2 {
        outcome_count: width,
        page_count: 3,
        candidate_coordinate: 2,
        price_scale: u64::from(width),
        candidate_id: BEST_CANDIDATE,
        product_id: product_id(),
        batch_id: identity,
    };
    CandidateV2::encode_into(header, &ones, &mut candidate).expect("draft candidate");
    let candidate_id = general_candidate_identity_v1(&candidate).expect("candidate identity");
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            candidate_id,
            ..header
        },
        &ones,
        &mut candidate,
    )
    .expect("addressed candidate");
    authenticate_candidate_identity_v1(CandidateV2::decode(&candidate).expect("candidate"))
        .expect("the candidate is its own digest");
    authenticate_batch_candidate_v1(
        batch,
        CandidateV2::decode(&candidate).expect("candidate").header(),
    )
    .expect("candidate authenticates against the closed batch");

    // The submission record: the account `Consider` reads and that nothing
    // wrote before this. It funds exactly the cranks its own life requires --
    // one per row, one for the consideration, one to close out.
    let opening_probe = GeneralCandidateOpeningV1 {
        outcome_count: width,
        page_count: 3,
        page_revision: CANDIDATE_PAGE_REVISION,
        submitted_slot: SUBMISSION_SLOT,
        candidate_id,
        batch_id: identity,
        solver_id: SOLVER,
        row_count: 3,
        reward_rate_lamports: CRANK_REWARD_LAMPORTS,
    };
    let mut submission = GeneralCandidateV1::submit(
        batch,
        CandidateV2::decode(&candidate).expect("candidate"),
        CANDIDATE_PAGE_REVISION,
        3,
        CRANK_REWARD_LAMPORTS,
        SOLVER,
        opening_probe.work_capacity().expect("work capacity"),
        SUBMISSION_SLOT,
    )
    .expect("submit the candidate against the closed batch");

    // THE WORK ESCROW IS FUNDED, not merely declared. The solver pays the
    // record's rent and its whole work capacity in one exact movement; over- and
    // under-funding are both refused, and the account's balance is from here on
    // the referent every capitalization check reads.
    let funding = WorkEscrowFundingPlanV1::new(
        opening_probe,
        SUBMISSION_RENT_FLOOR,
        ledger.solver,
        ledger.work_escrow,
    )
    .expect("submission funding plan");
    funding
        .validate_post(funding.solver_after(), funding.escrow_after())
        .expect("the funding movement is exactly the planned one");
    ledger.solver = funding.solver_after();
    ledger.work_escrow = funding.escrow_after();
    authenticate_work_escrow_v1(submission, 0, ledger.work_observation(0))
        .expect("a funded submission authenticates against its own balance");
    assert_eq!(ledger.total_lamports(), SOLVER_LAMPORTS);

    // Each page is deliberately unbalanced. The complete candidate alone has
    // the uniform relation required for a complete-set materialization.
    let rows: [(Vec<u8>, AuthenticatedOrderTermsV2); 3] = core::array::from_fn(|index| {
        let (bytes, lots) = placed.get(index).expect("placed order");
        execution_row(
            width,
            u32::try_from(index).expect("page coordinate") + 1,
            batch,
            bytes,
            *lots,
        )
    });
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
                revision: CANDIDATE_PAGE_REVISION,
                candidate_id,
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
        // THE CAMPAIGN'S CERTIFICATE IS NOW DERIVED, NOT FABRICATED. This used
        // to call the evaluator directly with terms and a page revision the
        // fixture chose. It now goes through the protocol's own verification
        // verb, which binds the page to this submission's candidate at the
        // revision the submission pinned, authenticates the row against the
        // ESCROWED order record it names, and pays one crank out of the
        // candidate's own work escrow.
        let summary = verify_candidate_row_v1(
            CandidateVerifyRowViewV1 {
                batch,
                submission,
                candidate: &candidate,
                page: &page,
                order: order_bytes_for(&placed, index),
                cursor_before: &cursor,
                verified_before: &zero_verified,
                expected_page_index: u32::try_from(index).expect("page index"),
                expected_row_index: 0,
                expected_revision: u64::try_from(index).expect("revision"),
            },
            CandidateVerifyRowBuffersV1 {
                cursor_scratch: &mut cursor_scratch,
                cursor_output: &mut cursor_output,
                verified_scratch: &mut verified_scratch,
                verified_output: &mut verified_output,
                manifest_scratch: &mut manifest_scratch,
                manifest_output: &mut manifest_output,
            },
        )
        .expect("verified row");
        assert_eq!(summary.complete, index == 2);
        assert_eq!(summary.reward.lamports, CRANK_REWARD_LAMPORTS);
        // EVERY CRANK IS PAID, and paid out of a balance the candidate already
        // holds. The draw plan is constructed from the escrow's observed
        // lamports AND the successor record together, so a row that advanced its
        // cursor without moving the reward -- or the reverse -- cannot be
        // applied here at all.
        let rows_verified_after = u32::try_from(index).expect("row cursor") + 1;
        let draw = WorkEscrowDrawPlanV1::new(
            ledger.work_observation(ledger.cranked),
            summary.submission,
            rows_verified_after,
            summary.reward,
        )
        .expect("verification crank draw");
        draw.validate_post(draw.escrow_after(), draw.beneficiary_after())
            .expect("the crank payment is exactly the planned one");
        ledger.work_escrow = draw.escrow_after();
        ledger.cranked = draw.beneficiary_after();
        assert_eq!(ledger.total_lamports(), SOLVER_LAMPORTS);
        // The terms the verb derived are the ones this fixture independently
        // projected from the same order record.
        assert_eq!(
            *terms,
            GeneralOrderV1::decode(order_bytes_for(&placed, index))
                .expect("order")
                .terms()
        );
        submission = summary.submission;
        cursor = cursor_output;
        if manifest_count != 0 {
            manifests.push(manifest_output);
        }
        if summary.complete {
            verified = verified_output;
        }
    }
    assert_eq!(manifests.len(), 2);
    // Two rows paid, and the remainder is what the consideration is for.
    submission
        .record_verified(batch, &verified)
        .expect("the certificate this verification produced");
    assert_eq!(
        submission.state().verification_remaining,
        CRANK_REWARD_LAMPORTS
    );
    // The escrow's remaining lamports are exactly the remaining cranks, proved
    // against the account rather than against the record's own arithmetic.
    authenticate_work_escrow_v1(submission, 3, ledger.work_observation(ledger.cranked))
        .expect("the verified submission still holds exactly what it owes");
    TerminalFixture {
        width,
        verifier: cursor,
        verified,
        manifests,
        candidate_id,
        submission,
        batch,
        orders: placed.iter().map(|(bytes, _)| bytes.clone()).collect(),
        escrow: ledger,
    }
}

/// The order record one candidate row names, in the candidate's own row order.
fn order_bytes_for(placed: &[(Vec<u8>, u64)], index: usize) -> &[u8] {
    &placed.get(index).expect("placed order").0
}

/// The admitted order record carrying one content identity.
///
/// Resolved by digest rather than by position: a settlement effect names an
/// order by identity, and looking it up by the loop's index would let a row
/// settle against a record it does not name.
fn order_for_id(fixture: &TerminalFixture, order_id: [u8; 32]) -> Vec<u8> {
    fixture
        .orders
        .iter()
        .find(|bytes| {
            GeneralOrderV1::decode(bytes)
                .expect("admitted order")
                .order_id()
                == order_id
        })
        .expect("the effect names an order this batch admitted")
        .clone()
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

/// Run one settlement transition natively, returning `(cursor, effect)`.
///
/// The effect is returned because it carries the exact `quote_quantity` and
/// `order_id` a row moves, and the escrow that funds it has to be checked
/// against those and not against a figure the test restates.
fn settle_native(
    fixture: &TerminalFixture,
    cursor: &[u8],
    action: RuntimeSettlementActionV2,
    manifest: Option<&[u8]>,
    manifest_order_index: u32,
) -> (Vec<u8>, Vec<u8>) {
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
    (cursor_output, effect_output)
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
    fixture: &TerminalFixture,
    action: Action,
    revision: u64,
    page_index: u32,
    execution_index: u8,
) -> ControllerRequestV2 {
    request_with_manifest_order(fixture, action, revision, page_index, execution_index, 0)
}

fn request_with_manifest_order(
    fixture: &TerminalFixture,
    action: Action,
    revision: u64,
    page_index: u32,
    execution_index: u8,
    manifest_order_index: u8,
) -> ControllerRequestV2 {
    ControllerRequestV2 {
        action,
        expected_revision: revision,
        candidate_id: Some(fixture.candidate_id),
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

async fn execute_initialize(
    fixture: &TerminalFixture,
) -> (AcceleratorDispositionV2, ExecutionEvidence) {
    let controller = request(fixture, Action::InitializeSettlement, 0, 0, 0);
    let (ack, _, evidence) = execute(real_sbf_fixture(
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
    (ack.disposition(), evidence)
}

async fn execute_settlement(
    fixture: &TerminalFixture,
    action: Action,
    cursor: &[u8],
    manifest: Option<&[u8]>,
    page_index: u32,
    execution_index: u8,
    manifest_order_index: u8,
) -> (AcceleratorAckV2<'static>, ExecutionEvidence) {
    let revision = SettlementCursorV2::decode(cursor)
        .expect("settlement cursor")
        .header()
        .revision;
    let controller = request_with_manifest_order(
        fixture,
        action,
        revision,
        page_index,
        execution_index,
        manifest_order_index,
    );
    let (ack, _, evidence) = execute(real_sbf_fixture(
        fixture.width,
        controller,
        bank_for_request(fixture.width, controller),
        runtime_for_settlement(fixture, action, cursor, manifest),
    ))
    .await;
    (ack, evidence)
}

fn assert_execution_evidence(evidence: ExecutionEvidence, action: Action, outcome_count: u32) {
    assert_eq!(evidence.action, action);
    assert_eq!(evidence.outcome_count, outcome_count);
    assert!(evidence.compute_units > 0);
    assert!(evidence.compute_units <= 1_400_000);
    // The frame width is DERIVED here, not restated, and the 2026-08-29
    // addendum to `docs/evidence/GENERAL_ACCELERATOR_CAMPAIGN_2026_08_27.md`
    // is why that matters. `instruction_accounts` is the number that document
    // records per action, and the only assertion that used to stand behind it
    // was `> 20`, which admits every wrong answer above twenty. The counts
    // reach the document through the `eprintln!` below, by hand. So when
    // `f581af6b` widened Custody `InitializeReplay` from 12 to 13 accounts,
    // this harness stayed green while the document stopped describing the
    // code, for a day; what caught it was a control in another crate that had
    // no business being the first line of defence.
    //
    // Every term below is already a fact this test holds, so nothing is
    // asserted twice from two places: two leading accounts, the fixed runtime
    // start owned by `dclutch-execution-strategy-contract`, the per-action
    // profile width owned by `dclutch-general-adapter-contract`, and the page
    // span the run actually measured. A frame that drifts by one now fails
    // here, at the ELF, naming both numbers -- which is the only place that can
    // notice before the evidence document goes stale again.
    let expected_accounts = 2_usize
        .saturating_add(ADMITTED_RUNTIME_ACCOUNTS_START_V3)
        .saturating_add(usize::from(
            general_account_profile_fixed_count_v3(action).expect("account geometry"),
        ))
        .saturating_add(usize::try_from(evidence.scratch_pages).expect("scratch page span"));
    assert_eq!(
        evidence.instruction_accounts, expected_accounts,
        "{action:?} at N={outcome_count} built a {}-account frame; the profile derives {expected_accounts} \
         (2 + start {ADMITTED_RUNTIME_ACCOUNTS_START_V3} + fixed {} + scratch pages {}). \
         If the frame legitimately moved, re-run the campaign and move the evidence document with it -- \
         do NOT edit this number to make the harness green.",
        evidence.instruction_accounts,
        general_account_profile_fixed_count_v3(action).expect("account geometry"),
        evidence.scratch_pages,
    );
    // This readonly CPI harness deliberately submits a legacy message so the
    // accelerator can see every scratch page directly. Some N=258 actions
    // exceed the network packet ceiling in this diagnostic transport; the
    // production operator separately proves the same account set packet-safe
    // through its exact ALT-backed v0 plan. That sentence stood without a
    // witness until 2026-08-27 and now has one:
    // `dclutch-operator::general_hot_v3::
    //  every_action_is_alt_packet_safe_at_the_canonical_runtime_width`
    // compiles all seven N=258 account sets through `compile_general_hot_v0`,
    // widest 922 of 1,232 bytes, and reproduces every account count measured
    // here. See `docs/evidence/GENERAL_ALT_PACKET_WITNESS_2026_08_27.md`.
    //
    // The widest was 918, then 920, and is 922 because `f581af6b` appended a
    // rent-refund account to Custody `InitializeReplay`; `InitializeSettlement`
    // is the only General action that embeds that operation. Restating a number
    // that lives in another crate's assertion is how it goes stale, which is
    // exactly what the count below does not do -- see the note there.
    assert!(evidence.packet_bytes > 0 && evidence.packet_bytes <= 2_000);
    assert!(evidence.scratch_pages > 0);
    eprintln!(
        "general-real-sbf action={action:?} N={outcome_count} cu={} accounts={} legacy_packet={} scratch_pages={}",
        evidence.compute_units,
        evidence.instruction_accounts,
        evidence.packet_bytes,
        evidence.scratch_pages,
    );
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
        let (ack, _, evidence) = execute(real_sbf_fixture(
            width,
            controller,
            input_bank(width, Action::Consider),
            runtime,
        ))
        .await;
        assert_eq!(ack.disposition(), AcceleratorDispositionV2::Accepted);
        assert_execution_evidence(evidence, Action::Consider, width);
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
        let (ack, _, evidence) = execute(real_sbf_fixture(
            width,
            controller,
            input_bank(width, Action::Freeze),
            runtime,
        ))
        .await;
        assert_eq!(ack.disposition(), AcceleratorDispositionV2::Accepted);
        assert_execution_evidence(evidence, Action::Freeze, width);
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
    // The escrow the collection half funded, carried into the settlement half.
    let mut escrow = fixture.escrow.clone();
    let atoms_before = escrow.total_atoms();
    let lamports_before = escrow.total_lamports();
    let (initialize, evidence) = execute_initialize(&fixture).await;
    assert_eq!(initialize, AcceleratorDispositionV2::Accepted);
    assert_execution_evidence(evidence, Action::InitializeSettlement, width);
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
    let (substituted_source, _) = execute_settlement(
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
    let (refused, _) = execute_settlement(
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
        let (ack, evidence) = execute_settlement(
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
        assert_execution_evidence(evidence, Action::Collect, width);
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::ORDER_COORDINATE),
            u64::try_from(expected_coordinate).expect("order coordinate") + 1
        );
        let (next, effect) = settle_native(
            &fixture,
            &cursor,
            RuntimeSettlementActionV2::Collect,
            Some(manifest),
            u32::from(*manifest_order),
        );
        cursor = next;
        // SETTLEMENT DRAWS ON THE ESCROW, and the amount is the one the
        // transition produced rather than one this test restates. Decision 0010
        // §2 made this the whole point of escrow-at-admission: the only balance
        // a settlement can be short of is one the protocol is already holding.
        let plan = RuntimeSettlementEffectPlanV2::decode(&effect).expect("collect effect");
        let header = plan.header();
        let order_bytes = order_for_id(&fixture, header.order_id);
        let order = GeneralOrderV1::decode(&order_bytes).expect("collected order");
        let held = *escrow
            .vaults
            .get(&header.order_id)
            .expect("the collected order has an escrow");
        authenticate_collect_from_escrow_v1(
            fixture.batch,
            order,
            OrderEscrowObservationV1 {
                escrow_context: header.order_id,
                vault_quote_atoms: held,
                maker_quote_atoms: *escrow.makers.get(&header.order_id).expect("maker"),
            },
            header.quote_quantity,
        )
        .expect("the order's own escrow covers this row's debit");
        escrow
            .vaults
            .insert(header.order_id, held - header.quote_quantity);
        escrow.settlement += header.quote_quantity;
        assert_eq!(escrow.total_atoms(), atoms_before);
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
    let (materialized, evidence) =
        execute_settlement(&fixture, Action::Materialize, &cursor, None, 0, 0, 0).await;
    assert_execution_evidence(evidence, Action::Materialize, width);
    assert_eq!(
        read_payload_scalar(materialized.payload(), scalar::CURSOR_PHASE),
        6
    );
    let (next, _) = settle_native(
        &fixture,
        &cursor,
        RuntimeSettlementActionV2::Materialize,
        None,
        0,
    );
    cursor = next;

    for (expected_coordinate, (manifest, manifest_order, page_index, execution_index)) in
        rows.iter().enumerate()
    {
        let (ack, evidence) = execute_settlement(
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
        assert_execution_evidence(evidence, Action::Distribute, width);
        assert_eq!(
            read_payload_scalar(ack.payload(), scalar::ORDER_COORDINATE),
            u64::try_from(expected_coordinate).expect("order coordinate") + 1
        );
        let (next, _) = settle_native(
            &fixture,
            &cursor,
            RuntimeSettlementActionV2::Distribute,
            Some(manifest),
            u32::from(*manifest_order),
        );
        cursor = next;
    }
    let ready = SettlementCursorV2::decode(&cursor).expect("ready cursor");
    assert_eq!(ready.header().quote_inventory, 0);
    assert!((0..fixture.width).all(|outcome| ready.inventory(outcome).expect("inventory") == 0));

    // A late child-precondition failure (nonzero Position table on terminal
    // close) refuses the entire candidate before any observed account changes.
    let close_request = request(&fixture, Action::Close, ready.header().revision, 0, 0);
    let mut hostile_bank = bank_for_request(fixture.width, close_request);
    write_scalar(&mut hostile_bank, scalar::POSITION_TABLE_COUNT, 1);
    let (refused, _, _) = execute(real_sbf_fixture(
        fixture.width,
        close_request,
        hostile_bank,
        runtime_for_settlement(&fixture, Action::Close, &cursor, None),
    ))
    .await;
    assert_eq!(refused.disposition(), AcceleratorDispositionV2::Refused);

    let (ack, evidence) = execute_settlement(&fixture, Action::Close, &cursor, None, 0, 0, 0).await;
    assert_execution_evidence(evidence, Action::Close, width);
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

    // ------------------------------------------------------------------
    // The escrow's own life closes, after the settlement it funded.
    // ------------------------------------------------------------------
    //
    // A post-window release quotes no amount: whatever the winning candidate
    // collected has already left, so what remains IS the refund. Every order
    // gets one, and the movement is planned against the vault's own balance.
    let batch = fixture.batch;
    for bytes in &fixture.orders {
        let order = GeneralOrderV1::decode(bytes).expect("admitted order");
        let order_id = order.order_id();
        let residual = batch
            .release(order, SETTLEMENT_CLOSE_SLOT)
            .expect("post-window release");
        assert_eq!(residual.quote_atoms, 0);
        let plan = OrderEscrowPlanV1::new(
            batch,
            order,
            residual,
            OrderEscrowObservationV1 {
                escrow_context: order_id,
                vault_quote_atoms: *escrow.vaults.get(&order_id).expect("vault"),
                maker_quote_atoms: *escrow.makers.get(&order_id).expect("maker"),
            },
        )
        .expect("residual release plan");
        plan.validate_post(plan.vault_after(), plan.maker_after())
            .expect("the release is exactly the planned one");
        // The property the address was said to give for free, now measured: no
        // maker leaves with more than the order escrowed.
        assert!(plan.quote_atoms() <= order.quote_reserve().expect("reserve"));
        escrow.vaults.insert(order_id, plan.vault_after());
        escrow.makers.insert(order_id, plan.maker_after());
    }
    // Nothing was created and nothing was destroyed: every atom is either back
    // with a maker or in the settlement inventory the candidate collected.
    assert_eq!(escrow.total_atoms(), atoms_before);
    assert!(escrow.vaults.values().all(|held| *held == 0));
    assert_eq!(
        escrow.settlement,
        atoms_before - escrow.makers.values().sum::<u64>(),
    );

    // The consideration is the last crank the verification compartment was sized
    // for. Gen-2 left it permissionless and UNPAID, which made a valid candidate
    // nobody cranked simply not compete; here performing it draws its reward out
    // of the candidate's own escrow.
    let mut submission = fixture.submission;
    let reward = submission
        .record_considered()
        .expect("the consideration is funded");
    let draw = WorkEscrowDrawPlanV1::new(
        escrow.work_observation(escrow.cranked),
        submission,
        3,
        reward,
    )
    .expect("consideration draw");
    draw.validate_post(draw.escrow_after(), draw.beneficiary_after())
        .expect("the consideration payment is exactly the planned one");
    escrow.work_escrow = draw.escrow_after();
    escrow.cranked = draw.beneficiary_after();

    // Close-out pays the cleanup crank and returns the residual AND the record's
    // rent to the solver who paid it -- decision 0010 §6 item 3's rent ownership,
    // routed rather than designed.
    let (cleanup, refund) = submission.close_out().expect("close the submission out");
    let close = WorkEscrowClosePlanV1::new(
        escrow.work_observation(escrow.cranked),
        cleanup,
        refund,
        escrow.solver,
    )
    .expect("close-out plan");
    close
        .validate_post(0, close.cranker_after(), close.solver_after())
        .expect("the close-out is exactly the planned one");
    escrow.work_escrow = 0;
    escrow.cranked = close.cranker_after();
    escrow.solver = close.solver_after();

    // Every lamport the solver funded is back with the solver or with a cranker,
    // and the escrow account holds nothing at all.
    assert_eq!(escrow.total_lamports(), lamports_before);
    assert_eq!(escrow.work_escrow, 0);
    assert_eq!(escrow.cranked, 5 * CRANK_REWARD_LAMPORTS);
    assert_eq!(escrow.solver, SOLVER_LAMPORTS - 5 * CRANK_REWARD_LAMPORTS);
    // And the accounting has no remaining referent to disagree with: the record
    // says both compartments are empty and so does the account.
    assert_eq!(
        work_escrow_required_lamports_v1(submission, SUBMISSION_RENT_FLOOR).expect("required"),
        SUBMISSION_RENT_FLOOR,
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
    let (initialize, evidence) = execute_initialize(&fixture).await;
    assert_eq!(initialize, AcceleratorDispositionV2::Accepted);
    assert_execution_evidence(evidence, Action::InitializeSettlement, 258);
    let cursor = initialized_cursor(&fixture);
    let final_manifest =
        SettlementManifestV2::decode(fixture.manifests.get(1).expect("final manifest bytes"))
            .expect("final manifest");
    let (out_of_order, _) = execute_settlement(
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
    let controller = request_with_manifest_order(&fixture, Action::Collect, 1, 2, 0, 1);
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
    let (ack, _, _) = execute(real_sbf_fixture(
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
        let controller = request(&fixture, Action::InitializeSettlement, 0, 0, 0);

        let mut substituted_product = runtime_for_initialize(&fixture);
        substituted_product.insert(2, vec![0xcc; PRODUCT_RECORD.len()]);
        let (ack, _, _) = execute(real_sbf_fixture(
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
        let (ack, _, _) = execute(real_sbf_fixture(
            width,
            controller,
            bank_for_request(width, controller),
            substituted_scale,
        ))
        .await;
        assert_eq!(ack.disposition(), AcceleratorDispositionV2::Refused);
    }
}
