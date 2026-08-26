//! Real-SBF success, hostile substitution, and transaction-rollback campaign.

use std::{env, path::Path};

use dclutch_rent_contract::{
    CreateRentCreditV1, RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority,
    RentCreditV1, WithdrawRentCreditV1,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x72; 32]);

fn require_real_elf() {
    let directory = env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR must name the real-SBF output");
    let elf = Path::new(&directory).join("dclutch_rent_sbf.so");
    assert!(elf.is_file(), "missing real RentCredit ELF at {elf:?}");
}

fn create_instruction(payer: Pubkey, authority: Pubkey) -> (Pubkey, Instruction) {
    let authority = RefundAuthority::new(authority.to_bytes()).expect("nonzero authority");
    let authority_bytes = authority.to_bytes();
    let (credit, bump) =
        Pubkey::find_program_address(&[RENT_CREDIT_PDA_DOMAIN_V1, &authority_bytes], &PROGRAM_ID);
    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(credit, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ],
        data: CreateRentCreditV1::new(authority, bump).to_bytes().to_vec(),
    };
    (credit, instruction)
}

fn withdraw_instruction(
    credit: Pubkey,
    authority: Pubkey,
    recipient: Pubkey,
    rent: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(credit, false),
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(recipient, false),
            AccountMeta::new_readonly(rent, false),
        ],
        data: WithdrawRentCreditV1::new(amount)
            .expect("nonzero withdrawal")
            .to_bytes()
            .to_vec(),
    }
}

async fn process(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Result<(), solana_program_test::BanksClientError> {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("latest blockhash");
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        signers,
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

async fn balance(context: &mut ProgramTestContext, key: Pubkey) -> u64 {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
        .expect("account exists")
        .lamports
}

#[tokio::test]
async fn real_sbf_create_withdraw_substitution_and_late_rollback() {
    require_real_elf();
    let mut context = ProgramTest::new("dclutch_rent_sbf", PROGRAM_ID, None)
        .start_with_context()
        .await;
    let fee_payer = context.payer.insecure_clone();
    let authority = Keypair::new();
    let impostor = Keypair::new();
    let recipient = Pubkey::new_unique();
    let counterfeit_rent = Pubkey::new_unique();

    let payer = context.payer.pubkey();
    let wallet_rent = Rent::default().minimum_balance(0);
    let setup = [
        transfer(&payer, &authority.pubkey(), wallet_rent),
        transfer(&payer, &impostor.pubkey(), wallet_rent),
        transfer(&payer, &recipient, wallet_rent),
        transfer(&payer, &counterfeit_rent, wallet_rent),
    ];
    process(&mut context, &setup, &[&fee_payer])
        .await
        .expect("fund ordinary System accounts");

    let (credit, create) = create_instruction(payer, authority.pubkey());
    let mut substituted_system = create.clone();
    *substituted_system
        .accounts
        .get_mut(2)
        .expect("System account meta") = AccountMeta::new_readonly(counterfeit_rent, false);
    assert!(
        process(&mut context, &[substituted_system], &[&fee_payer])
            .await
            .is_err()
    );
    assert!(
        context
            .banks_client
            .get_account(credit)
            .await
            .expect("credit query")
            .is_none()
    );

    let wrong_credit = Pubkey::new_unique();
    let mut substituted_pda = create.clone();
    *substituted_pda
        .accounts
        .get_mut(1)
        .expect("RentCredit account meta") = AccountMeta::new(wrong_credit, false);
    assert!(
        process(&mut context, &[substituted_pda], &[&fee_payer])
            .await
            .is_err()
    );
    assert!(
        context
            .banks_client
            .get_account(wrong_credit)
            .await
            .expect("wrong credit query")
            .is_none()
    );

    process(&mut context, &[create], &[&fee_payer])
        .await
        .expect("create permanent RentCredit through real SBF");
    let floor = Rent::default().minimum_balance(RENT_CREDIT_BYTES_V1);
    let created = context
        .banks_client
        .get_account(credit)
        .await
        .expect("credit query")
        .expect("created credit");
    assert_eq!(created.owner, PROGRAM_ID);
    assert_eq!(created.lamports, floor);
    let state = RentCreditV1::decode(&created.data).expect("canonical RentCredit state");
    assert_eq!(
        state.refund_authority().to_bytes(),
        authority.pubkey().to_bytes()
    );
    let immutable_state = created.data;

    process(
        &mut context,
        &[transfer(&payer, &credit, 23)],
        &[&fee_payer],
    )
    .await
    .expect("donate exact surplus");
    assert_eq!(balance(&mut context, credit).await, floor + 23);

    let recipient_before = balance(&mut context, recipient).await;
    let withdraw = withdraw_instruction(credit, authority.pubkey(), recipient, sysvar::rent::ID, 7);
    process(&mut context, &[withdraw], &[&fee_payer, &authority])
        .await
        .expect("withdraw exact surplus through real SBF");
    assert_eq!(balance(&mut context, credit).await, floor + 16);
    assert_eq!(balance(&mut context, recipient).await, recipient_before + 7);
    assert_eq!(
        context
            .banks_client
            .get_account(credit)
            .await
            .expect("credit query")
            .expect("credit")
            .data,
        immutable_state
    );

    let before_hostile_credit = balance(&mut context, credit).await;
    let before_hostile_recipient = balance(&mut context, recipient).await;
    let substituted_authority =
        withdraw_instruction(credit, impostor.pubkey(), recipient, sysvar::rent::ID, 1);
    assert!(
        process(
            &mut context,
            &[substituted_authority],
            &[&fee_payer, &impostor],
        )
        .await
        .is_err()
    );
    assert_eq!(balance(&mut context, credit).await, before_hostile_credit);
    assert_eq!(
        balance(&mut context, recipient).await,
        before_hostile_recipient
    );

    let substituted_rent =
        withdraw_instruction(credit, authority.pubkey(), recipient, counterfeit_rent, 1);
    assert!(
        process(&mut context, &[substituted_rent], &[&fee_payer, &authority],)
            .await
            .is_err()
    );
    assert_eq!(balance(&mut context, credit).await, before_hostile_credit);
    assert_eq!(
        balance(&mut context, recipient).await,
        before_hostile_recipient
    );

    // The first instruction succeeds physically, then the hostile second
    // instruction refuses. ProgramTest must roll the whole transaction back.
    let first = withdraw_instruction(credit, authority.pubkey(), recipient, sysvar::rent::ID, 5);
    let late_refusal =
        withdraw_instruction(credit, impostor.pubkey(), recipient, sysvar::rent::ID, 1);
    assert!(
        process(
            &mut context,
            &[first, late_refusal],
            &[&fee_payer, &authority, &impostor],
        )
        .await
        .is_err()
    );
    assert_eq!(balance(&mut context, credit).await, before_hostile_credit);
    assert_eq!(
        balance(&mut context, recipient).await,
        before_hostile_recipient
    );
}
