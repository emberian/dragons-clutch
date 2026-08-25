//! Real-ELF adversarial campaign for registered Direct claim execution.
//!
//! The tested SBF artifact owns both registration-local replay sequences and
//! Position balances.  It executes the Lean-owned lifecycle bytecode; no
//! native processor, effect-plan input, or mock adapter is registered.

use std::{env, path::PathBuf};

use dclutch_direct_codec::{CompactIntentV1, RegisteredIntentStateV1};
use solana_account::Account;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::system_program;
use solana_transaction::Transaction;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([81_u8; 32]);
const POSITION_BYTES: usize = 56;
const INSTRUCTION_BYTES: usize = 16;

#[derive(Clone)]
struct Fixture {
    seller_registration: Pubkey,
    buyer_registration: Pubkey,
    seller_position: Pubkey,
    buyer_position: Pubkey,
    seller_data: Vec<u8>,
    buyer_data: Vec<u8>,
    seller_position_data: Vec<u8>,
    buyer_position_data: Vec<u8>,
    seller_owner: Pubkey,
    buyer_owner: Pubkey,
}

struct RefusalCase {
    name: &'static str,
    fixture: Fixture,
    instruction: Vec<u8>,
    seller_writable: bool,
}

fn require_sbf() {
    let directory = env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required for real ELF tests");
    assert!(
        PathBuf::from(directory)
            .join("dclutch_claims_proof_sbf.so")
            .is_file(),
        "SBF_OUT_DIR must contain dclutch_claims_proof_sbf.so"
    );
}

fn registered_state(authority: Pubkey, side: u8, lifecycle: u8, maker: u8, market: u8) -> Vec<u8> {
    RegisteredIntentStateV1 {
        phase: 0,
        controller: authority.to_bytes(),
        maker: [maker; 32],
        intent: CompactIntentV1 {
            side,
            outcome: 1,
            lifecycle,
            market: [market; 32],
            generation: 3,
            nonce: 9,
            valid_from: 0,
            valid_through: u64::MAX,
            maximum_fill: 2_000,
            limit_price: if side == 0 { 400_000 } else { 600_000 },
            fee_basis_points: 25,
            collateral_account: [5_u8.wrapping_add(side); 32],
        },
        remaining: 2_000,
        sequence: 0,
    }
    .encode()
    .expect("canonical registered state")
    .to_vec()
}

fn position(authority: Pubkey, claims: u64) -> Vec<u8> {
    let mut data = vec![0_u8; POSITION_BYTES];
    data[..8].copy_from_slice(&[b'D', b'C', b'P', b'N', 1, 0, 0, 0]);
    data[8..40].copy_from_slice(authority.as_ref());
    data[40..48].copy_from_slice(&1_u64.to_le_bytes());
    data[48..56].copy_from_slice(&claims.to_le_bytes());
    data
}

fn fill_instruction(fill: u64) -> Vec<u8> {
    let mut data = vec![0_u8; INSTRUCTION_BYTES];
    data[..8].copy_from_slice(&[b'D', b'C', b'R', b'F', 1, 0, 0, 0]);
    data[8..].copy_from_slice(&fill.to_le_bytes());
    data
}

fn fixture(authority: Pubkey) -> Fixture {
    Fixture {
        seller_registration: Pubkey::new_unique(),
        buyer_registration: Pubkey::new_unique(),
        seller_position: Pubkey::new_unique(),
        buyer_position: Pubkey::new_unique(),
        seller_data: registered_state(authority, 0, 2, 8, 4),
        buyer_data: registered_state(authority, 1, 2, 9, 4),
        seller_position_data: position(authority, 5_000),
        buyer_position_data: position(authority, 200),
        seller_owner: PROGRAM_ID,
        buyer_owner: PROGRAM_ID,
    }
}

fn instruction(
    authority: Pubkey,
    fixture: &Fixture,
    data: Vec<u8>,
    seller_writable: bool,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(authority, true),
            if seller_writable {
                AccountMeta::new(fixture.seller_registration, false)
            } else {
                AccountMeta::new_readonly(fixture.seller_registration, false)
            },
            AccountMeta::new(fixture.buyer_registration, false),
            AccountMeta::new(fixture.seller_position, false),
            AccountMeta::new(fixture.buyer_position, false),
        ],
        data,
    }
}

fn add_fixture(test: &mut ProgramTest, fixture: &Fixture) {
    let accounts = [
        (
            fixture.seller_registration,
            fixture.seller_data.clone(),
            fixture.seller_owner,
        ),
        (
            fixture.buyer_registration,
            fixture.buyer_data.clone(),
            fixture.buyer_owner,
        ),
        (
            fixture.seller_position,
            fixture.seller_position_data.clone(),
            PROGRAM_ID,
        ),
        (
            fixture.buyer_position,
            fixture.buyer_position_data.clone(),
            PROGRAM_ID,
        ),
    ];
    for (address, data, owner) in accounts {
        test.add_account(
            address,
            Account {
                lamports: Rent::default().minimum_balance(data.len()),
                data,
                owner,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    authority: &Keypair,
) -> (bool, u64) {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer, authority],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("banks processing");
    (
        processed.result.is_ok(),
        processed
            .metadata
            .expect("transaction metadata")
            .compute_units_consumed,
    )
}

async fn snapshot(context: &mut ProgramTestContext, fixture: &Fixture) -> [Account; 4] {
    let mut accounts = Vec::new();
    for address in [
        fixture.seller_registration,
        fixture.buyer_registration,
        fixture.seller_position,
        fixture.buyer_position,
    ] {
        accounts.push(
            context
                .banks_client
                .get_account(address)
                .await
                .expect("account query")
                .expect("fixture account"),
        );
    }
    accounts.try_into().expect("four-account snapshot")
}

fn read_claims(account: &Account) -> u64 {
    u64::from_le_bytes(account.data[48..56].try_into().expect("claims field"))
}

#[tokio::test]
async fn registered_claim_elf_executes_residuals_and_refuses_hostile_space() {
    require_sbf();
    let authority = Keypair::new();
    let success = fixture(authority.pubkey());

    let mut cases = Vec::new();
    let mut push = |name, fixture, data, seller_writable| {
        cases.push(RefusalCase {
            name,
            fixture,
            instruction: data,
            seller_writable,
        });
    };

    let mut bad = fixture(authority.pubkey());
    bad.seller_data[11] = 1;
    push(
        "noncanonical registration padding",
        bad,
        fill_instruction(500),
        true,
    );

    let mut bad = fixture(authority.pubkey());
    bad.seller_data[10] = 2;
    push(
        "terminal registration replay",
        bad,
        fill_instruction(500),
        true,
    );

    let mut bad = fixture(authority.pubkey());
    bad.buyer_data[48..80].fill(8);
    push("same maker", bad, fill_instruction(500), true);

    let mut bad = fixture(authority.pubkey());
    bad.buyer_data[96..128].fill(5);
    push("different market", bad, fill_instruction(500), true);

    let mut bad = fixture(authority.pubkey());
    bad.seller_data = registered_state(authority.pubkey(), 0, 0, 8, 4);
    push("partial fill-or-kill", bad, fill_instruction(500), true);

    let mut bad = fixture(authority.pubkey());
    bad.seller_position_data[48..56].copy_from_slice(&499_u64.to_le_bytes());
    push(
        "insufficient seller claims",
        bad,
        fill_instruction(500),
        true,
    );

    let mut bad = fixture(authority.pubkey());
    bad.buyer_position_data[48..56].copy_from_slice(&u64::MAX.to_le_bytes());
    push("buyer claim overflow", bad, fill_instruction(500), true);

    let mut bad = fixture(authority.pubkey());
    bad.seller_owner = system_program::ID;
    push("wrong registration owner", bad, fill_instruction(500), true);

    push(
        "readonly registration",
        fixture(authority.pubkey()),
        fill_instruction(500),
        false,
    );
    push(
        "zero fill",
        fixture(authority.pubkey()),
        fill_instruction(0),
        true,
    );
    let mut bad_instruction = fill_instruction(500);
    bad_instruction[6] = 1;
    push(
        "noncanonical instruction",
        fixture(authority.pubkey()),
        bad_instruction,
        true,
    );

    let mut test = ProgramTest::new("dclutch_claims_proof_sbf", PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_account(
        authority.pubkey(),
        Account::new(1_000_000, 0, &system_program::ID),
    );
    add_fixture(&mut test, &success);
    for case in &cases {
        add_fixture(&mut test, &case.fixture);
    }
    let mut context = test.start_with_context().await;

    let (accepted, first_cu) = submit(
        &mut context,
        instruction(authority.pubkey(), &success, fill_instruction(500), true),
        &authority,
    )
    .await;
    assert!(accepted, "first registered residual fill");
    eprintln!("registered residual first-fill CU: {first_cu}");
    let after_first = snapshot(&mut context, &success).await;
    let seller = RegisteredIntentStateV1::decode(&after_first[0].data).expect("seller state");
    let buyer = RegisteredIntentStateV1::decode(&after_first[1].data).expect("buyer state");
    assert_eq!(
        (seller.remaining, seller.sequence, seller.phase),
        (1_500, 1, 0)
    );
    assert_eq!(
        (buyer.remaining, buyer.sequence, buyer.phase),
        (1_500, 1, 0)
    );
    assert_eq!(read_claims(&after_first[2]), 4_500);
    assert_eq!(read_claims(&after_first[3]), 700);

    let (accepted, final_cu) = submit(
        &mut context,
        instruction(authority.pubkey(), &success, fill_instruction(1_500), true),
        &authority,
    )
    .await;
    assert!(accepted, "second registered residual fill");
    eprintln!("registered residual terminal-fill CU: {final_cu}");
    let after_final = snapshot(&mut context, &success).await;
    let seller = RegisteredIntentStateV1::decode(&after_final[0].data).expect("seller state");
    let buyer = RegisteredIntentStateV1::decode(&after_final[1].data).expect("buyer state");
    assert_eq!((seller.remaining, seller.sequence, seller.phase), (0, 2, 1));
    assert_eq!((buyer.remaining, buyer.sequence, buyer.phase), (0, 2, 1));
    assert_eq!(read_claims(&after_final[2]), 3_000);
    assert_eq!(read_claims(&after_final[3]), 2_200);

    for case in cases {
        let before = snapshot(&mut context, &case.fixture).await;
        let (accepted, refusal_cu) = submit(
            &mut context,
            instruction(
                authority.pubkey(),
                &case.fixture,
                case.instruction,
                case.seller_writable,
            ),
            &authority,
        )
        .await;
        assert!(!accepted, "{} must refuse", case.name);
        eprintln!("{} refusal CU: {refusal_cu}", case.name);
        let after = snapshot(&mut context, &case.fixture).await;
        assert_eq!(
            after, before,
            "{} must roll back all four accounts",
            case.name
        );
    }
}
