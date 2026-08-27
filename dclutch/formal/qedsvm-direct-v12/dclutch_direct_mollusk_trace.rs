//! Capture exact Direct controller traces with qedsvm's pinned Agave/Mollusk dependency.

use dclutch_direct_codec::{
    CompactIntentV1, RegisteredIntentStateV1, RegisteredTerminalAction,
    RegisteredTerminalInstructionV1,
};
use mollusk_svm::{result::ProgramResult, Mollusk};
use qedsvm::diff::qedsvm_to_mollusk;
use sha2::{Digest as _, Sha256};
use solana_account::{Account, AccountSharedData, ReadableAccount};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const CONTROLLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([67_u8; 32]);
const CLAIM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([81_u8; 32]);
const CONTROLLER_SEED: &[u8] = b"dclutch-controller-v1";
const REGISTERED_SEED: &[u8] = b"dclutch/direct-registered/v1";
const GENERATION: u64 = 3;
const FILL: u64 = 2_000;

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn account(owner: Pubkey, data: Vec<u8>, executable: bool) -> AccountSharedData {
    AccountSharedData::from(Account {
        lamports: 1_000_000,
        data,
        owner,
        executable,
        rent_epoch: 0,
    })
}

fn fixture(expected_sequence: u64) -> (Instruction, Vec<(Pubkey, AccountSharedData)>, Pubkey) {
    let maker = Pubkey::new_from_array([61_u8; 32]);
    let market = Pubkey::new_from_array([62_u8; 32]);
    let collateral = Pubkey::new_from_array([63_u8; 32]);
    let (controller, controller_bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &CONTROLLER_PROGRAM_ID);
    let generation = GENERATION.to_le_bytes();
    let nonce = 0_u64.to_le_bytes();
    let (registration, registration_bump) = Pubkey::find_program_address(
        &[
            REGISTERED_SEED,
            market.as_ref(),
            &generation,
            maker.as_ref(),
            &nonce,
        ],
        &CONTROLLER_PROGRAM_ID,
    );
    let intent = CompactIntentV1 {
        side: 0,
        outcome: 1,
        lifecycle: 2,
        market: market.to_bytes(),
        generation: GENERATION,
        nonce: 0,
        valid_from: 0,
        valid_through: u64::MAX,
        maximum_fill: FILL,
        limit_price: 400_000,
        fee_basis_points: 25,
        collateral_account: collateral.to_bytes(),
    };
    let state = RegisteredIntentStateV1 {
        phase: 0,
        controller: controller.to_bytes(),
        maker: maker.to_bytes(),
        intent,
        remaining: FILL,
        sequence: 0,
    }
    .encode()
    .expect("canonical registered state")
    .to_vec();
    let instruction = Instruction {
        program_id: CONTROLLER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(controller, false),
            AccountMeta::new(registration, false),
            AccountMeta::new_readonly(maker, true),
            AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
        ],
        data: RegisteredTerminalInstructionV1 {
            action: RegisteredTerminalAction::Cancel,
            controller_bump,
            registration_bump,
            expected_sequence,
        }
        .encode()
        .expect("canonical terminal instruction")
        .to_vec(),
    };
    let accounts = vec![
        (
            controller,
            account(solana_sdk_ids::system_program::ID, vec![], false),
        ),
        (registration, account(CLAIM_PROGRAM_ID, state, false)),
        (
            maker,
            account(solana_sdk_ids::system_program::ID, vec![], false),
        ),
        (
            CLAIM_PROGRAM_ID,
            account(solana_sdk_ids::bpf_loader_upgradeable::ID, vec![], true),
        ),
    ];
    (instruction, accounts, registration)
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    let deploy_dir = arguments.next().expect("deploy directory");
    let claim_elf = std::fs::read(arguments.next().expect("claim ELF path")).expect("claim ELF");
    let mode = arguments.next().expect("success or stale");
    let expected_sequence = match mode.as_str() {
        "success" => 0,
        "stale" => 1,
        _ => panic!("mode must be success or stale"),
    };
    let (instruction, accounts, registration) = fixture(expected_sequence);
    let registration_pre = accounts
        .iter()
        .find(|(key, _)| *key == registration)
        .expect("registration prestate")
        .1
        .clone();
    let registration_pre_data = registration_pre.data().to_vec();
    let registration_pre_lamports = registration_pre.lamports();
    let registration_pre_owner = registration_pre.owner().to_bytes();
    let registration_pre_executable = registration_pre.executable();
    let registration_pre_rent_epoch = registration_pre.rent_epoch();
    println!(
        "fixture: controller={} registration={} request_bytes={} request_sha256={} state_bytes={} state_sha256={}",
        instruction.accounts[0].pubkey,
        registration,
        instruction.data.len(),
        sha256_hex(&instruction.data),
        registration_pre_data.len(),
        sha256_hex(&registration_pre_data),
    );
    std::env::set_var("SBF_OUT_DIR", deploy_dir);
    let mut mollusk =
        Mollusk::new_debuggable(&CONTROLLER_PROGRAM_ID, "dclutch_controller_proof_sbf", true);
    mollusk.add_program_with_loader_and_elf(
        &CLAIM_PROGRAM_ID,
        &solana_sdk_ids::bpf_loader_upgradeable::ID,
        &claim_elf,
    );
    let mollusk_accounts = qedsvm_to_mollusk(&accounts);
    let result = mollusk.process_instruction(&instruction, &mollusk_accounts);
    match mode.as_str() {
        "success" => {
            assert_eq!(result.program_result, ProgramResult::Success);
            let (_, registration_post) = result
                .resulting_accounts
                .iter()
                .find(|(key, _)| *key == registration)
                .expect("registration poststate");
            let decoded = RegisteredIntentStateV1::decode(&registration_post.data)
                .expect("terminal registration state");
            assert_eq!(
                (decoded.phase, decoded.remaining, decoded.sequence),
                (2, FILL, 1)
            );
        }
        "stale" => {
            assert!(matches!(result.program_result, ProgramResult::Failure(_)));
            let (_, registration_post) = result
                .resulting_accounts
                .iter()
                .find(|(key, _)| *key == registration)
                .expect("registration rollback poststate");
            assert_eq!(
                registration_post.data, registration_pre_data,
                "data rollback"
            );
            assert_eq!(
                registration_post.lamports, registration_pre_lamports,
                "lamport rollback"
            );
            assert_eq!(
                registration_post.owner.to_bytes(),
                registration_pre_owner,
                "owner rollback"
            );
            assert_eq!(
                registration_post.executable, registration_pre_executable,
                "executable rollback"
            );
            assert_eq!(
                registration_post.rent_epoch, registration_pre_rent_epoch,
                "rent epoch rollback"
            );
        }
        _ => unreachable!(),
    }
    println!(
        "{mode}: {:?}, {} CU",
        result.program_result, result.compute_units_consumed
    );
}
