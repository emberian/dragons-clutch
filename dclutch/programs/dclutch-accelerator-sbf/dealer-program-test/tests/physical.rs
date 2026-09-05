//! Real-ELF transport evidence for the Dealer admitted accelerator.
//!
//! Two campaigns live here. The refusal case pins that a truncated frame fails
//! closed without mutating an observed account. The frontier case drives a
//! geometry-complete frame all the way through the real caller ELF into the
//! real accelerator ELF, and pins which side of the CPI now owns the refusal.
//!
//! Neither is an acceptance test. `AcceleratorDispositionV2::Accepted` on the
//! admitted-AOT path requires a complete Dealer scenario chain -- activation
//! cache, Market, finalized records, Claims aggregate, Custody replay, Realm
//! and collateral -- staged in `crate::dealer_chain`. What these pin is the
//! depth the lane actually reaches, so a regression toward the two
//! always-refuses frame bugs cannot pass unnoticed again.
//!
//! The accepted transition itself is executed in `tests/accepted.rs`, over the
//! lock-bounded checkpoint routes rather than this unsplit frame. That is not a
//! second story: the unsplit admitted instruction resolves 121 account locks
//! against a 64-lock ceiling, so it is the split, not this frame, that a caller
//! can ever send.

use std::vec::Vec;

use dclutch_capability_program_contract::hot_v3::{
    HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
    HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
    HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
    HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3,
    HOT_TRADING_PROGRAM_ACCOUNT_V3, HotExecutionEnvelopeV3,
};
use dclutch_core_contract::ContentId;
use dclutch_dealer_accelerator_test_caller_sbf::{
    DealerAcceleratorTestCallerErrorV1, dealer_accelerator_test_caller_authority_v1,
};
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_REQUEST_HEADER_BYTES_V2, AcceleratorRequestV2, RequestTransportV2,
};
use dclutch_refusal_registry::ACCELERATOR_REFUSAL_BASE;
use dclutch_trading_sbf::admitted_composition_v3::ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4;
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;

const ACCELERATOR: Pubkey = Pubkey::new_from_array([0xd1; 32]);
const CALLER: Pubkey = Pubkey::new_from_array([0xd2; 32]);
const REQUEST_ACCOUNT: Pubkey = Pubkey::new_from_array([0xd3; 32]);
const OBSERVED: Pubkey = Pubkey::new_from_array([0xd4; 32]);
const DUMMY: Pubkey = Pubkey::new_from_array([0xd5; 32]);
const CORE_PROGRAM: Pubkey = Pubkey::new_from_array([0xd6; 32]);
const REGISTRY_PROGRAM: Pubkey = Pubkey::new_from_array([0xd7; 32]);

/// The exact refusal the accelerator emits when Trading declines the frame:
/// `DealerAcceleratorSbfErrorV4::InvalidInvocation`, the second code in the
/// Dealer arm's sub-band (`0xC100`) of the accelerator's band. Derived from
/// the REGISTRY base rather than written as a literal --
/// `assert!(text.contains("Custom(3)"))` also accepts `Custom(30)`, and this
/// file cannot take a dependency on the program whose ELF it loads.
const ACCELERATOR_INVALID_INVOCATION: u32 = ACCELERATOR_REFUSAL_BASE + 0x100 + 1;
/// The exact refusal the caller emits when the forwarded frame is malformed.
const CALLER_FRAME: u32 = DealerAcceleratorTestCallerErrorV1::Frame as u32;
/// The exact refusal the caller emits when the authority or privileges differ.
const CALLER_AUTHORITY: u32 = DealerAcceleratorTestCallerErrorV1::Authority as u32;

fn content(value: u8) -> ContentId {
    ContentId::new([value; 32]).expect("nonzero fixture content")
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

/// One non-invokable executable identity, for the slots Trading only checks
/// the `executable` flag and owner of.
fn add_executable(test: &mut ProgramTest, key: Pubkey) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(36).max(1),
            data: vec![0_u8; 36],
            owner: bpf_loader_upgradeable::ID,
            executable: true,
            rent_epoch: 0,
        },
    );
}

fn frame_key(index: usize) -> Pubkey {
    let mut bytes = [0x10_u8; 32];
    let index = u32::try_from(index).expect("frame index width");
    for (slot, byte) in bytes
        .get_mut(28..32)
        .expect("pubkey tail")
        .iter_mut()
        .zip(index.to_le_bytes())
    {
        *slot = byte;
    }
    Pubkey::new_from_array(bytes)
}

fn evidence_key(index: usize) -> Pubkey {
    let mut bytes = [0x20_u8; 32];
    let index = u32::try_from(index).expect("evidence index width");
    for (slot, byte) in bytes
        .get_mut(28..32)
        .expect("pubkey tail")
        .iter_mut()
        .zip(index.to_le_bytes())
    {
        *slot = byte;
    }
    Pubkey::new_from_array(bytes)
}

fn malformed_frame_fixture() -> (ProgramTest, Instruction, Vec<u8>) {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("dclutch_accelerator_sbf", ACCELERATOR, None);
    test.add_program("dclutch_dealer_accelerator_test_caller_sbf", CALLER, None);
    let observed = vec![0x5a; 96];
    add_account(&mut test, OBSERVED, CALLER, observed.clone());
    add_account(&mut test, DUMMY, system_program::ID, Vec::new());

    let bank = [0_u8; 8];
    let request = AcceleratorRequestV2::new(
        RequestTransportV2::Inline,
        content(1),
        content(2),
        content(3),
        content(4),
        content(5),
        1,
        1,
        0,
        0,
        &bank,
    )
    .expect("canonical request");
    let mut request_bytes = vec![0_u8; ACCELERATOR_REQUEST_HEADER_BYTES_V2 + bank.len()];
    request
        .encode_into(&mut request_bytes)
        .expect("request encoding");

    let family = [0_u8; 4];
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(family.len()).expect("family width"),
        [1; 32],
        [2; 32],
        1,
        hash(&observed).to_bytes(),
    )
    .expect("Hot envelope");
    let mut top_level_data = envelope.to_bytes().to_vec();
    top_level_data.extend_from_slice(&family);
    let (authority, _, _) = dealer_accelerator_test_caller_authority_v1(
        &CALLER,
        &top_level_data,
        &OBSERVED,
        &request_bytes,
    )
    .expect("canonical caller authority");
    add_account(&mut test, authority, system_program::ID, Vec::new());
    add_account(&mut test, REQUEST_ACCOUNT, CALLER, request_bytes);
    let instruction = Instruction {
        program_id: CALLER,
        accounts: vec![
            AccountMeta::new_readonly(authority, false),
            AccountMeta::new_readonly(DUMMY, false),
            AccountMeta::new_readonly(OBSERVED, false),
            AccountMeta::new_readonly(REQUEST_ACCOUNT, false),
            AccountMeta::new_readonly(ACCELERATOR, false),
        ],
        data: top_level_data,
    };
    (test, instruction, observed)
}

/// A geometry-complete admitted frame in the canonical top-level layout.
///
/// `fixed(39) ++ evidence(8) ++ authority(1) ++ runtime(5) ++ request ++
/// accelerator`, with the test caller installed at the Trading fixed slot so
/// the authority PDA it signs is the one Trading re-derives. Every account
/// body past the sysvars is a placeholder: this fixture buys frame geometry,
/// not a chain.
fn geometry_complete_fixture() -> (ProgramTest, Instruction) {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("dclutch_accelerator_sbf", ACCELERATOR, None);
    test.add_program("dclutch_dealer_accelerator_test_caller_sbf", CALLER, None);
    add_executable(&mut test, CORE_PROGRAM);
    add_executable(&mut test, REGISTRY_PROGRAM);

    let root_data = vec![0x5a_u8; 96];
    let market = frame_key(HOT_MARKET_ACCOUNT_V3);
    let root = frame_key(HOT_ROOT_ACCOUNT_V3);

    let mut fixed = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
        .map(frame_key)
        .collect::<Vec<_>>();
    set(&mut fixed, HOT_TRADING_PROGRAM_ACCOUNT_V3, CALLER);
    set(&mut fixed, HOT_CORE_PROGRAM_ACCOUNT_V3, CORE_PROGRAM);
    set(
        &mut fixed,
        HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
        REGISTRY_PROGRAM,
    );
    set(&mut fixed, HOT_RENT_SYSVAR_ACCOUNT_V3, sysvar::rent::ID);
    set(
        &mut fixed,
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
        sysvar::instructions::ID,
    );

    for (index, key) in fixed.iter().enumerate() {
        if index == HOT_TRADING_PROGRAM_ACCOUNT_V3
            || index == HOT_CORE_PROGRAM_ACCOUNT_V3
            || index == HOT_REGISTRY_PROGRAM_ACCOUNT_V3
            || index == HOT_RENT_SYSVAR_ACCOUNT_V3
            || index == HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3
        {
            continue;
        }
        let data = if index == HOT_ROOT_ACCOUNT_V3 {
            root_data.clone()
        } else {
            vec![u8::try_from(index % 251).expect("body byte"); 8]
        };
        add_account(&mut test, *key, REGISTRY_PROGRAM, data);
    }

    let evidence = (0..ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4)
        .map(|index| {
            if index == 6 {
                ACCELERATOR
            } else {
                let key = evidence_key(index);
                add_account(&mut test, key, REGISTRY_PROGRAM, vec![0x20; 8]);
                key
            }
        })
        .collect::<Vec<_>>();

    let family = [7_u8; 4];
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(family.len()).expect("family width"),
        [3; 32],
        market.to_bytes(),
        9,
        hash(&root_data).to_bytes(),
    )
    .expect("Hot envelope");
    let mut top_level_data = envelope.to_bytes().to_vec();
    top_level_data.extend_from_slice(&family);

    let bank = [0_u8; 8];
    let request = AcceleratorRequestV2::new(
        RequestTransportV2::Inline,
        content(1),
        content(2),
        content(3),
        content(4),
        ContentId::new(hash(&bank).to_bytes()).expect("input bank digest"),
        1,
        1,
        0,
        0,
        &bank,
    )
    .expect("canonical request");
    let mut request_bytes = vec![0_u8; ACCELERATOR_REQUEST_HEADER_BYTES_V2 + bank.len()];
    request
        .encode_into(&mut request_bytes)
        .expect("request encoding");

    let (authority, _, _) = dealer_accelerator_test_caller_authority_v1(
        &CALLER,
        &top_level_data,
        &root,
        &request_bytes,
    )
    .expect("canonical caller authority");
    add_account(&mut test, authority, system_program::ID, Vec::new());
    add_account(&mut test, REQUEST_ACCOUNT, REGISTRY_PROGRAM, request_bytes);

    let mut accounts = fixed
        .iter()
        .enumerate()
        .map(|(index, key)| AccountMeta {
            pubkey: *key,
            is_signer: false,
            // Only the root is writable at top level, exactly as
            // `authenticate_accelerator_top_level_v4` requires.
            is_writable: index == HOT_ROOT_ACCOUNT_V3,
        })
        .collect::<Vec<_>>();
    accounts.extend(
        evidence
            .iter()
            .map(|key| AccountMeta::new_readonly(*key, false)),
    );
    accounts.push(AccountMeta::new_readonly(authority, false));
    // The five fixed logical runtime coordinates carry no suffix here, so the
    // caller rejoins them itself and the top-level adds nothing between the
    // authority block and its own two trailing accounts.
    accounts.push(AccountMeta::new_readonly(REQUEST_ACCOUNT, false));
    accounts.push(AccountMeta::new_readonly(ACCELERATOR, false));

    let instruction = Instruction {
        program_id: CALLER,
        accounts,
        data: top_level_data,
    };
    (test, instruction)
}

fn set(keys: &mut [Pubkey], index: usize, value: Pubkey) {
    *keys.get_mut(index).expect("fixed slot") = value;
}

/// Every fixed logical runtime coordinate the caller rejoins from the frame.
fn runtime_coordinates() -> [usize; 5] {
    [
        HOT_ROOT_ACCOUNT_V3,
        HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
    ]
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

fn custom_code(result: &Result<(), solana_sdk::transaction::TransactionError>) -> Option<u32> {
    match result {
        Err(solana_sdk::transaction::TransactionError::InstructionError(
            _,
            solana_program::instruction::InstructionError::Custom(code),
        )) => Some(*code),
        _ => None,
    }
}

#[tokio::test]
async fn real_elf_rejects_a_truncated_hot_frame_without_mutation() {
    let (test, instruction, observed_before) = malformed_frame_fixture();
    let mut context = test.start_with_context().await;
    let result = submit(&mut context, instruction)
        .await
        .expect("ProgramTest processing");
    assert!(result.result.is_err(), "truncated frame must fail closed");
    let observed_after = context
        .banks_client
        .get_account(OBSERVED)
        .await
        .expect("observed account query")
        .expect("observed account");
    assert_eq!(observed_after.data, observed_before);
}

/// Pin which side of the CPI owns the refusal on a geometry-complete frame.
///
/// The load-bearing assertion is *which program* refuses. A canonical admitted
/// frame must be forwarded: the caller must not stop it on its own frame check
/// (0x108000) or authority check (0x108001), and the code that comes back must
/// be the accelerator's own `InvalidInvocation` (0xD001), raised at CPI depth
/// two after `AcceleratorRequestV2::decode` succeeded.
///
/// It is deliberately NOT a claim about which authentication stage refuses.
/// The accelerator maps every `TradingSbfError` to that one code, so no
/// ProgramTest can tell them apart; `tests/frontier.rs` owns stage attribution
/// and measures it in-process.
#[tokio::test]
async fn real_elf_forwards_a_geometry_complete_frame_into_accelerator_authentication() {
    let (test, instruction) = geometry_complete_fixture();
    assert_eq!(
        runtime_coordinates().len(),
        HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3,
        "the caller rejoins exactly the fixed logical runtime coordinates the \
         contract declares, so the top-level carries only the suffix"
    );
    let mut context = test.start_with_context().await;
    let processed = submit(&mut context, instruction)
        .await
        .expect("ProgramTest processing");
    let code = custom_code(&processed.result);
    assert_ne!(
        code,
        Some(CALLER_FRAME),
        "the caller must accept a canonical admitted frame layout"
    );
    assert_ne!(
        code,
        Some(CALLER_AUTHORITY),
        "the caller-authority PDA must join the one Trading re-derives"
    );
    assert_eq!(
        code,
        Some(ACCELERATOR_INVALID_INVOCATION),
        "a geometry-complete frame must reach the accelerator and refuse on \
         chain content; observed {:?} with logs {:?}",
        processed.result,
        processed.metadata.as_ref().map(|value| &value.log_messages)
    );
}
