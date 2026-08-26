//! Real Token-2022 processor execution for nonzero display decimals and exact
//! base-unit transfer behavior.

use dclutch_token_svm::{ACCOUNT_BYTES, TOKEN_2022_PROGRAM_ID, Token2022BehaviorProfileV2};
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_program_test::{ProgramTest, ProgramTestContext, processor};
use solana_sdk::signature::{Keypair, Signer};
use solana_system_interface::instruction::create_account;
use solana_transaction::Transaction;
use spl_token_2022_interface::{
    extension::{ExtensionType, permissioned_burn},
    instruction::{
        initialize_account, initialize_mint, initialize_mint_close_authority, mint_to_checked,
        transfer_checked,
    },
    state::Mint,
};

const DECIMALS: u8 = u8::MAX;
const INITIAL_BASE_UNITS: u64 = 17;
const TRANSFER_BASE_UNITS: u64 = 5;

async fn submit(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let mut all_signers = std::vec![&context.payer];
    all_signers.extend_from_slice(signers);
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &all_signers,
        blockhash,
    );
    context
        .banks_client
        .process_transaction(transaction)
        .await
        .expect("Token-2022 transaction");
}

async fn create_token_account(
    context: &mut ProgramTestContext,
    account: &Keypair,
    mint: Pubkey,
    owner: Pubkey,
) {
    let rent = context.banks_client.get_rent().await.expect("Rent");
    let create = create_account(
        &context.payer.pubkey(),
        &account.pubkey(),
        rent.minimum_balance(ACCOUNT_BYTES),
        u64::try_from(ACCOUNT_BYTES).expect("Account width"),
        &spl_token_2022::id(),
    );
    let initialize = initialize_account(&spl_token_2022::id(), &account.pubkey(), &mint, &owner)
        .expect("InitializeAccount");
    submit(context, &[create, initialize], &[account]).await;
}

#[tokio::test]
async fn real_token_2022_keeps_max_decimals_display_only_and_transfers_exact_base_units() {
    let program_test = ProgramTest::new(
        "spl_token_2022",
        spl_token_2022::id(),
        processor!(spl_token_2022::processor::Processor::process),
    );
    let mut context = program_test.start_with_context().await;
    let mint = Keypair::new();
    let controller = Keypair::new();
    let holder = Keypair::new();
    let recipient = Keypair::new();
    let holder_account = Keypair::new();
    let recipient_account = Keypair::new();

    let mint_bytes = ExtensionType::try_calculate_account_len::<Mint>(&[
        ExtensionType::MintCloseAuthority,
        ExtensionType::PermissionedBurn,
    ])
    .expect("Mint extension width");
    let rent = context.banks_client.get_rent().await.expect("Rent");
    let create_mint = create_account(
        &context.payer.pubkey(),
        &mint.pubkey(),
        rent.minimum_balance(mint_bytes),
        u64::try_from(mint_bytes).expect("Mint width"),
        &spl_token_2022::id(),
    );
    let close = initialize_mint_close_authority(
        &spl_token_2022::id(),
        &mint.pubkey(),
        Some(&controller.pubkey()),
    )
    .expect("MintCloseAuthority");
    let burn = permissioned_burn::instruction::initialize(
        &spl_token_2022::id(),
        &mint.pubkey(),
        &controller.pubkey(),
    )
    .expect("PermissionedBurn");
    let initialize = initialize_mint(
        &spl_token_2022::id(),
        &mint.pubkey(),
        &controller.pubkey(),
        None,
        DECIMALS,
    )
    .expect("InitializeMint2");
    submit(
        &mut context,
        &[create_mint, close, burn, initialize],
        &[&mint],
    )
    .await;

    create_token_account(
        &mut context,
        &holder_account,
        mint.pubkey(),
        holder.pubkey(),
    )
    .await;
    create_token_account(
        &mut context,
        &recipient_account,
        mint.pubkey(),
        recipient.pubkey(),
    )
    .await;
    let mint_units = mint_to_checked(
        &spl_token_2022::id(),
        &mint.pubkey(),
        &holder_account.pubkey(),
        &controller.pubkey(),
        &[],
        INITIAL_BASE_UNITS,
        DECIMALS,
    )
    .expect("MintToChecked");
    submit(&mut context, &[mint_units], &[&controller]).await;
    let transfer = transfer_checked(
        &spl_token_2022::id(),
        &holder_account.pubkey(),
        &mint.pubkey(),
        &recipient_account.pubkey(),
        &holder.pubkey(),
        &[],
        TRANSFER_BASE_UNITS,
        DECIMALS,
    )
    .expect("TransferChecked");
    submit(&mut context, &[transfer], &[&holder]).await;

    let mint_state = context
        .banks_client
        .get_account(mint.pubkey())
        .await
        .expect("Mint fetch")
        .expect("Mint account");
    let holder_state = context
        .banks_client
        .get_account(holder_account.pubkey())
        .await
        .expect("holder fetch")
        .expect("holder account");
    let recipient_state = context
        .banks_client
        .get_account(recipient_account.pubkey())
        .await
        .expect("recipient fetch")
        .expect("recipient account");

    let mint_facts = Token2022BehaviorProfileV2::check_mint(
        TOKEN_2022_PROGRAM_ID,
        mint.pubkey().to_bytes(),
        &mint_state.data,
        controller.pubkey().to_bytes(),
        INITIAL_BASE_UNITS,
    )
    .expect("behavior Mint");
    let holder_facts = Token2022BehaviorProfileV2::check_account(
        TOKEN_2022_PROGRAM_ID,
        &holder_state.data,
        mint.pubkey().to_bytes(),
        holder.pubkey().to_bytes(),
        INITIAL_BASE_UNITS - TRANSFER_BASE_UNITS,
    )
    .expect("holder behavior Account");
    let recipient_facts = Token2022BehaviorProfileV2::check_account(
        TOKEN_2022_PROGRAM_ID,
        &recipient_state.data,
        mint.pubkey().to_bytes(),
        recipient.pubkey().to_bytes(),
        TRANSFER_BASE_UNITS,
    )
    .expect("recipient behavior Account");
    assert_eq!(mint_facts.display_decimals(), DECIMALS);
    assert_eq!(mint_facts.base_supply(), INITIAL_BASE_UNITS);
    assert_eq!(
        holder_facts.base_amount(),
        INITIAL_BASE_UNITS - TRANSFER_BASE_UNITS
    );
    assert_eq!(recipient_facts.base_amount(), TRANSFER_BASE_UNITS);
}
