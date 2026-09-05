//! qedsvm trace fixture for dClutch's exact-account Effect proof target.

use qedsvm::{ProgramResult, Svm};
use solana_account::{Account, AccountSharedData, ReadableAccount};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([83; 32]);
const AUTHORITY: Pubkey = Pubkey::new_from_array([84; 32]);
const STATE: Pubkey = Pubkey::new_from_array([85; 32]);

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .trim()
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("ASCII hex");
            u8::from_str_radix(pair, 16).expect("valid hex")
        })
        .collect()
}

fn projection() -> Vec<u8> {
    let mut data = vec![0_u8; 104];
    data[0..4].copy_from_slice(b"DCES");
    data[4] = 1;
    data[8..40].copy_from_slice(AUTHORITY.as_ref());
    data[40..44].copy_from_slice(&1_u32.to_le_bytes());
    for (index, value) in [0_u64, 0, 5_000, 200, 2_000, 100, 20]
        .into_iter()
        .enumerate()
    {
        let offset = 48 + index * 8;
        data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    data
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    let elf_path = arguments
        .next()
        .expect("usage: dclutch_effect ELF PLAN_HEX");
    let plan_path = arguments
        .next()
        .expect("usage: dclutch_effect ELF PLAN_HEX");
    let elf = std::fs::read(elf_path).expect("read exact ELF");
    let plan = decode_hex(&std::fs::read_to_string(plan_path).expect("read Lean vector"));
    let mut svm = Svm::default();
    svm.add_program(&PROGRAM_ID, &elf);

    let authority = AccountSharedData::from(Account {
        lamports: 1_000_000,
        data: vec![],
        owner: Pubkey::new_from_array([0; 32]),
        executable: false,
        rent_epoch: 0,
    });
    let state = AccountSharedData::from(Account {
        lamports: 1_000_000,
        data: projection(),
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    });
    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(AUTHORITY, true),
            AccountMeta::new(STATE, false),
        ],
        data: plan,
    };

    let result = svm
        .process_instruction(&instruction, &[(AUTHORITY, authority), (STATE, state)])
        .expect("qedsvm executes exact ELF");
    assert_eq!(result.program_result, ProgramResult::Success);
    let post = result.resulting_accounts[1].1.data();
    let field = |index: usize| {
        let offset = 48 + index * 8;
        u64::from_le_bytes(post[offset..offset + 8].try_into().expect("u64 field"))
    };
    assert_eq!(
        [
            field(0),
            field(1),
            field(2),
            field(3),
            field(4),
            field(5),
            field(6),
        ],
        [1, 1, 3_000, 2_200, 998, 1_100, 22]
    );
    println!(
        "qedsvm exact ELF success: {} CU",
        result.compute_units_consumed
    );
}
