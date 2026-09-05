//! The wall a fractional claim-check's redemption route has to get over.
//!
//! `docs/design/CLAIM_CHECK_COMPACTION_V1.md` §17.3 closes with a sentence the
//! whole fractional sizing rests on:
//!
//! > the claim-check answers to the instrument, and the holder redeems by
//! > burning, with their own signature, forever.
//!
//! That sentence is false, and the reason is in the Token program rather than
//! anywhere in this tree. Every shard Mint in the Fractional family carries
//! Token-2022's `PermissionedBurn` extension -- `Token2022BehaviorProfileV2::read_mint`
//! *requires* it and refuses any Mint without it -- pinned to the Mint's
//! controller, which for a Fractional coordinate is the capability root: a
//! **Trading**-derived PDA that Claims cannot sign and that does not outlive the
//! market.
//!
//! This campaign executes the wall rather than reading it, against the same
//! audited `spl-token-2022` v11 ELF every other Fractional campaign loads. Four
//! transactions on one Mint:
//!
//! 1. a standard `BurnChecked`, signed by the account's own owner -- refused;
//! 2. a permissioned `BurnChecked` with the authority present but not signing --
//!    refused;
//! 3. a permissioned `BurnChecked` naming a different authority -- refused;
//! 4. a permissioned `BurnChecked` with both signatures -- accepted, and the
//!    supply and the holder's balance each fall by exactly the burn.
//!
//! The fourth is the control: without it, the first three would be equally
//! consistent with a Mint that simply cannot be burned at all, and the campaign
//! would prove nothing about *which* signature is missing.
//!
//! What follows from this is the correction the fractional record carries: the
//! only way a shard holder ever burns with their own signature is if the second
//! signature belongs to something Claims can sign, so compaction must re-point
//! the Mint's burn authority to the escrow while the root is still alive to
//! authorize it. Transaction 5 executes that hand-off and then re-runs the burn
//! under the new authority, which is the fractional redemption's own shape with
//! the escrow PDA replaced by an ordinary key.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use std::{env, fs, path::PathBuf};

use dclutch_custody::token_svm::{ACCOUNT_BYTES, MINT_BYTES, Mint, TokenAccount};
use solana_program::{hash::hash, instruction::Instruction, pubkey::Pubkey, rent::Rent};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::{
    instruction::InstructionError, signature::Keypair, signer::Signer,
    transaction::TransactionError,
};
use solana_system_interface::instruction::create_account;
use solana_transaction::Transaction;
use spl_token_2022_interface::{
    error::TokenError,
    extension::{ExtensionType, permissioned_burn::instruction as permissioned_burn},
    instruction as token_instruction,
    pod::PodMint,
};

/// Token-2022, at the address the whole tree pins it to.
fn token_program_id() -> Pubkey {
    Pubkey::new_from_array(dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID)
}

const DECIMALS: u8 = 0;
const MINTED: u64 = 5_000;
const BURNED: u64 = 1_000;

/// The two rows of `fixtures/token-2022-v11.provenance` this campaign accepts.
///
/// Checked here rather than only in a shell wrapper, because the whole value of
/// this campaign is that it ran against the *audited* Token program. A run
/// pointed at some other build would answer a different question and say so in
/// no other way.
const TOKEN_2022_V11_CANONICAL_SHA256: [u8; 32] = [
    0xe2, 0xac, 0xdf, 0xb7, 0x50, 0x88, 0x14, 0x62, 0xad, 0x61, 0x3a, 0x15, 0xcc, 0x9c, 0x54, 0xae,
    0x17, 0xce, 0x06, 0x65, 0x80, 0xe8, 0x67, 0xe1, 0xe6, 0x35, 0xfb, 0xdf, 0xe0, 0x1f, 0x56, 0x97,
];
const TOKEN_2022_V11_MACOS_AUDIT_SHA256: [u8; 32] = [
    0x44, 0x7c, 0xa3, 0xc6, 0x90, 0xec, 0x00, 0x1c, 0x88, 0xca, 0xdc, 0xa3, 0x41, 0x52, 0xa4, 0xab,
    0xb7, 0x80, 0x65, 0x85, 0x52, 0xe8, 0x72, 0xd5, 0x97, 0x75, 0xcc, 0x41, 0x7e, 0x6e, 0xc2, 0x5d,
];

fn context_test() -> ProgramTest {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let path = directory.join("spl_token_2022.so");
    assert!(path.is_file(), "missing real ELF: {}", path.display());
    let digest = hash(&fs::read(&path).expect("read real ELF")).to_bytes();
    assert!(
        digest == TOKEN_2022_V11_CANONICAL_SHA256 || digest == TOKEN_2022_V11_MACOS_AUDIT_SHA256,
        "spl_token_2022.so is in neither row of fixtures/token-2022-v11.provenance"
    );
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("spl_token_2022", token_program_id(), None);
    test
}

async fn send(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Result<(), TransactionError> {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let payer = context.payer.insecure_clone();
    let mut all: Vec<&Keypair> = vec![&payer];
    all.extend_from_slice(signers);
    let transaction =
        Transaction::new_signed_with_payer(instructions, Some(&payer.pubkey()), &all, blockhash);
    context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("transaction processing")
        .result
}

/// The exact refusal this instruction produced, as the code a log line shows.
fn custom_code(error: TransactionError) -> u32 {
    match error {
        TransactionError::InstructionError(_, InstructionError::Custom(code)) => code,
        other => panic!("expected a custom refusal, got {other:?}"),
    }
}

fn instruction_error(error: TransactionError) -> InstructionError {
    match error {
        TransactionError::InstructionError(_, inner) => inner,
        other => panic!("expected an instruction error, got {other:?}"),
    }
}

async fn mint_supply(context: &mut ProgramTestContext, mint: Pubkey) -> u64 {
    let account = context
        .banks_client
        .get_account(mint)
        .await
        .expect("fetch mint")
        .expect("mint exists");
    Mint::parse(&account.data[..MINT_BYTES])
        .expect("mint base")
        .supply
}

async fn token_amount(context: &mut ProgramTestContext, key: Pubkey) -> u64 {
    let account = context
        .banks_client
        .get_account(key)
        .await
        .expect("fetch token account")
        .expect("token account exists");
    TokenAccount::parse(&account.data[..ACCOUNT_BYTES])
        .expect("token account")
        .amount
}

/// One Mint carrying the shard profile's burn extension, plus a funded holder.
struct Wall {
    mint: Pubkey,
    holder_tokens: Pubkey,
    holder: Keypair,
    burn_authority: Keypair,
}

async fn build_wall(context: &mut ProgramTestContext) -> Wall {
    let token = token_program_id();
    let mint = Keypair::new();
    let holder = Keypair::new();
    let holder_tokens = Keypair::new();
    let burn_authority = Keypair::new();
    let mint_authority = context.payer.pubkey();

    // Exactly the layout `Token2022BehaviorProfileV2::read_mint` admits for a
    // shard Mint's burn half: the base Mint plus one `PermissionedBurn` entry.
    let mint_space =
        ExtensionType::try_calculate_account_len::<PodMint>(&[ExtensionType::PermissionedBurn])
            .expect("mint length");
    let rent = Rent::default();

    send(
        context,
        &[
            create_account(
                &context.payer.pubkey(),
                &mint.pubkey(),
                rent.minimum_balance(mint_space).max(1),
                mint_space as u64,
                &token,
            ),
            permissioned_burn::initialize(&token, &mint.pubkey(), &burn_authority.pubkey())
                .expect("initialize permissioned burn"),
            token_instruction::initialize_mint2(
                &token,
                &mint.pubkey(),
                &mint_authority,
                None,
                DECIMALS,
            )
            .expect("initialize mint"),
        ],
        &[&mint],
    )
    .await
    .expect("create the shard-profile mint");

    let account_space = ACCOUNT_BYTES;
    send(
        context,
        &[
            create_account(
                &context.payer.pubkey(),
                &holder_tokens.pubkey(),
                rent.minimum_balance(account_space).max(1),
                account_space as u64,
                &token,
            ),
            token_instruction::initialize_account3(
                &token,
                &holder_tokens.pubkey(),
                &mint.pubkey(),
                &holder.pubkey(),
            )
            .expect("initialize holder account"),
            token_instruction::mint_to(
                &token,
                &mint.pubkey(),
                &holder_tokens.pubkey(),
                &mint_authority,
                &[],
                MINTED,
            )
            .expect("mint to holder"),
        ],
        &[&holder_tokens],
    )
    .await
    .expect("fund the holder");

    assert_eq!(mint_supply(context, mint.pubkey()).await, MINTED);
    assert_eq!(token_amount(context, holder_tokens.pubkey()).await, MINTED);

    Wall {
        mint: mint.pubkey(),
        holder_tokens: holder_tokens.pubkey(),
        holder,
        burn_authority,
    }
}

#[tokio::test]
async fn a_shard_holders_own_signature_can_never_burn_a_shard() {
    let mut context = context_test().start_with_context().await;
    let token = token_program_id();
    let wall = build_wall(&mut context).await;

    // 1. The design's sentence, executed. The holder owns the account, signs for
    //    it, and is refused -- not for want of funds, not for a frame error, but
    //    because a standard burn is not an instruction this Mint accepts at all.
    let standard = token_instruction::burn_checked(
        &token,
        &wall.holder_tokens,
        &wall.mint,
        &wall.holder.pubkey(),
        &[],
        BURNED,
        DECIMALS,
    )
    .expect("standard burn instruction");
    let refusal = send(&mut context, &[standard], &[&wall.holder])
        .await
        .expect_err("a standard burn must be refused while PermissionedBurn is present");
    assert_eq!(
        custom_code(refusal),
        TokenError::InvalidInstruction as u32,
        "the refusal must be TokenError::InvalidInstruction"
    );
    assert_eq!(
        TokenError::InvalidInstruction as u32,
        12,
        "the literal a validator log shows for this wall"
    );
    // Nothing moved.
    assert_eq!(mint_supply(&mut context, wall.mint).await, MINTED);
    assert_eq!(token_amount(&mut context, wall.holder_tokens).await, MINTED);

    // 2. The permissioned burn with the authority present but not signing. The
    //    holder cannot manufacture this signature, and neither can Claims when
    //    the authority is another program's PDA.
    let mut unsigned_authority = permissioned_burn::burn_checked(
        &token,
        &wall.holder_tokens,
        &wall.mint,
        &wall.burn_authority.pubkey(),
        &wall.holder.pubkey(),
        &[],
        BURNED,
        DECIMALS,
    )
    .expect("permissioned burn instruction");
    unsigned_authority.accounts[2].is_signer = false;
    let missing = send(&mut context, &[unsigned_authority], &[&wall.holder])
        .await
        .expect_err("the burn authority's signature is not optional");
    assert_eq!(
        instruction_error(missing),
        InstructionError::MissingRequiredSignature
    );

    // 3. A different authority, signing. Presenting *a* signature is not enough;
    //    it has to be the one the Mint names.
    let impostor = Keypair::new();
    let wrong_authority = permissioned_burn::burn_checked(
        &token,
        &wall.holder_tokens,
        &wall.mint,
        &impostor.pubkey(),
        &wall.holder.pubkey(),
        &[],
        BURNED,
        DECIMALS,
    )
    .expect("permissioned burn instruction");
    let mismatched = send(&mut context, &[wrong_authority], &[&wall.holder, &impostor])
        .await
        .expect_err("only the Mint's own burn authority may approve");
    assert_eq!(
        instruction_error(mismatched),
        InstructionError::InvalidAccountData
    );
    assert_eq!(mint_supply(&mut context, wall.mint).await, MINTED);

    // 4. The control. Both signatures, and the burn goes through -- so the three
    //    refusals above are about WHICH signature is missing, not about a Mint
    //    that cannot be burned.
    let permitted = permissioned_burn::burn_checked(
        &token,
        &wall.holder_tokens,
        &wall.mint,
        &wall.burn_authority.pubkey(),
        &wall.holder.pubkey(),
        &[],
        BURNED,
        DECIMALS,
    )
    .expect("permissioned burn instruction");
    send(
        &mut context,
        &[permitted],
        &[&wall.holder, &wall.burn_authority],
    )
    .await
    .expect("a burn carrying both signatures is accepted");
    assert_eq!(mint_supply(&mut context, wall.mint).await, MINTED - BURNED);
    assert_eq!(
        token_amount(&mut context, wall.holder_tokens).await,
        MINTED - BURNED
    );
}

#[tokio::test]
async fn handing_the_burn_authority_over_is_what_makes_a_holder_signed_redemption_possible() {
    // The correction, executed. A fractional compaction re-points the shard
    // Mint's burn authority to the escrow -- an address Claims can sign for --
    // while the Fractional root is still alive to authorize the move. After that
    // one instruction, a redemption needs the holder's signature and a signature
    // the Claims program produces for itself, and needs nothing the market took
    // with it.
    //
    // The escrow is an ordinary key here rather than a PDA, because what is
    // under test is Token-2022's rule about who may approve a burn, not the
    // derivation of the approver. `invoke_signed` for a program-owned address is
    // the same signature to the Token program.
    let mut context = context_test().start_with_context().await;
    let token = token_program_id();
    let wall = build_wall(&mut context).await;
    let escrow = Keypair::new();

    // Only the CURRENT authority can hand it over, which is why this leg has to
    // happen at compaction and cannot be deferred to the first redemption.
    let impostor = Keypair::new();
    let usurped = token_instruction::set_authority(
        &token,
        &wall.mint,
        Some(&escrow.pubkey()),
        token_instruction::AuthorityType::PermissionedBurn,
        &impostor.pubkey(),
        &[],
    )
    .expect("set authority instruction");
    let refused = send(&mut context, &[usurped], &[&impostor])
        .await
        .expect_err("only the current burn authority may hand it over");
    assert_eq!(
        custom_code(refused),
        TokenError::OwnerMismatch as u32,
        "a stranger re-pointing a Mint's burn authority is an owner mismatch"
    );

    let handover = token_instruction::set_authority(
        &token,
        &wall.mint,
        Some(&escrow.pubkey()),
        token_instruction::AuthorityType::PermissionedBurn,
        &wall.burn_authority.pubkey(),
        &[],
    )
    .expect("set authority instruction");
    send(&mut context, &[handover], &[&wall.burn_authority])
        .await
        .expect("the current authority hands the burn over");

    // The old authority is now exactly as powerless as the market that held it.
    let stale = permissioned_burn::burn_checked(
        &token,
        &wall.holder_tokens,
        &wall.mint,
        &wall.burn_authority.pubkey(),
        &wall.holder.pubkey(),
        &[],
        BURNED,
        DECIMALS,
    )
    .expect("permissioned burn instruction");
    let after_handover = send(
        &mut context,
        &[stale],
        &[&wall.holder, &wall.burn_authority],
    )
    .await
    .expect_err("the Fractional root must lose the burn when it hands it over");
    assert_eq!(
        instruction_error(after_handover),
        InstructionError::InvalidAccountData
    );

    // And the redemption shape works: the holder signs for their own shards, the
    // escrow signs as approver, and the supply falls by exactly the burn.
    let redemption = permissioned_burn::burn_checked(
        &token,
        &wall.holder_tokens,
        &wall.mint,
        &escrow.pubkey(),
        &wall.holder.pubkey(),
        &[],
        BURNED,
        DECIMALS,
    )
    .expect("permissioned burn instruction");
    send(&mut context, &[redemption], &[&wall.holder, &escrow])
        .await
        .expect("a holder-signed, escrow-approved burn is accepted");
    assert_eq!(mint_supply(&mut context, wall.mint).await, MINTED - BURNED);
    assert_eq!(
        token_amount(&mut context, wall.holder_tokens).await,
        MINTED - BURNED
    );
}
