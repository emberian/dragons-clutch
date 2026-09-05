//! The burn-authority hand-off, with a derived escrow and a real shard profile.
//!
//! `permissioned_burn_wall.rs` proved Token-2022's rule and named two gaps of
//! its own, both in `docs/evidence/FRACTIONAL_CLAIM_CHECK_2026_08_30.md`:
//!
//! > The escrow in the hand-off test is an ordinary keypair, not a PDA. What is
//! > under test is who Token-2022 will accept as an approver, not the
//! > derivation of the approver.
//!
//! > The burn wall is measured on a Mint this campaign built, not on a Mint the
//! > Fractional route produced. The Mint carries the base state plus one
//! > `PermissionedBurn` entry, which is the half of the shard profile the wall
//! > turns on […] What is proved is Token-2022's rule; that the Fractional
//! > family's Mints carry the extension is read from `behavior_profile_v2.rs`,
//! > not executed here.
//!
//! This campaign closes both, and adds a third thing neither had: it runs
//! `Token2022BehaviorProfileV2` over the bytes Token-2022 itself wrote, before
//! and after the hand-off, so the split-controller arm is joined to real chain
//! state rather than to a fixture this repository builds by hand.
//!
//! **The escrow is derived, not invented.** Every signature here comes from
//! `dclutch-claim-check-escrow-signer-test-sbf`, which derives the escrow with
//! `ClaimCheckEscrowSeedsV1` -- the exact recipe the shipped Claims escrow uses
//! -- and signs with `invoke_signed`. The one thing it cannot borrow is the
//! Claims program id, because `invoke_signed` signs only for the calling
//! program's own addresses. So what is proved is that this tree's escrow seed
//! recipe produces a signature Token-2022 accepts as a burn approver; what is
//! not proved is the Claims program producing it, because no Claims route does
//! yet.
//!
//! **The Mint carries the whole shard profile.** Base Mint with the root as its
//! mint authority, `MintCloseAuthority` naming the root, `PermissionedBurn`
//! naming the root, no freeze authority -- the exact shape
//! `Token2022BehaviorProfileV2::read_mint` requires and refuses anything else
//! for. The funding mint is signed by the root, which is why the signer program
//! needs a mint action at all: a Mint whose mint authority was a convenient
//! keypair would not be a shard Mint, and running the profile over it would
//! prove nothing.
//!
//! **Both sides of the hand-off are program-derived**, which is production's
//! shape: an authority that cannot sign for itself hands to one that can. The
//! authority it moves *from* is a stated stand-in rather than the real
//! Fractional capability root, because that root is a Trading-derived PDA and
//! this program is not Trading.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use std::{env, fs, path::PathBuf};

use dclutch_claim_check_escrow_signer_test_sbf::{
    ESCROW_SIGNER_ACCOUNT_COUNT, ESCROW_SIGNER_INSTRUCTION_BYTES, EscrowSignerActionV1,
    escrow_address, root_stand_in_address, stranger_address,
};
use dclutch_claims::claim_check_v1::ClaimCheckEscrowSeedsV1;
use dclutch_custody::token_svm::{
    ACCOUNT_BYTES, Error as TokenSvmError, MINT_BYTES, Mint, TOKEN_2022_PROGRAM_ID,
    Token2022BehaviorProfileV2, TokenAccount,
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
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

const DECIMALS: u8 = 0;
const MINTED: u64 = 5_000;
const BURNED: u64 = 1_000;

/// The Claims aggregate this escrow answers to; the escrow's sole seed.
const AGGREGATE: [u8; 32] = [0x2b; 32];

/// The escrow-signer program's address in this campaign.
const ESCROW_SIGNER_PROGRAM: Pubkey = Pubkey::new_from_array([0x5e; 32]);

/// The two rows of `fixtures/token-2022-v11.provenance` this campaign accepts.
const TOKEN_2022_V11_CANONICAL_SHA256: [u8; 32] = [
    0xe2, 0xac, 0xdf, 0xb7, 0x50, 0x88, 0x14, 0x62, 0xad, 0x61, 0x3a, 0x15, 0xcc, 0x9c, 0x54, 0xae,
    0x17, 0xce, 0x06, 0x65, 0x80, 0xe8, 0x67, 0xe1, 0xe6, 0x35, 0xfb, 0xdf, 0xe0, 0x1f, 0x56, 0x97,
];
const TOKEN_2022_V11_MACOS_AUDIT_SHA256: [u8; 32] = [
    0x44, 0x7c, 0xa3, 0xc6, 0x90, 0xec, 0x00, 0x1c, 0x88, 0xca, 0xdc, 0xa3, 0x41, 0x52, 0xa4, 0xab,
    0xb7, 0x80, 0x65, 0x85, 0x52, 0xe8, 0x72, 0xd5, 0x97, 0x75, 0xcc, 0x41, 0x7e, 0x6e, 0xc2, 0x5d,
];

fn token_program_id() -> Pubkey {
    Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID)
}

fn context_test() -> ProgramTest {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let token = directory.join("spl_token_2022.so");
    assert!(token.is_file(), "missing real ELF: {}", token.display());
    let digest = hash(&fs::read(&token).expect("read real ELF")).to_bytes();
    assert!(
        digest == TOKEN_2022_V11_CANONICAL_SHA256 || digest == TOKEN_2022_V11_MACOS_AUDIT_SHA256,
        "spl_token_2022.so is in neither row of fixtures/token-2022-v11.provenance"
    );
    let signer = directory.join("dclutch_claim_check_escrow_signer_test_sbf.so");
    assert!(signer.is_file(), "missing real ELF: {}", signer.display());
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("spl_token_2022", token_program_id(), None);
    test.add_program(
        "dclutch_claim_check_escrow_signer_test_sbf",
        ESCROW_SIGNER_PROGRAM,
        None,
    );
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

fn instruction_error(error: TransactionError) -> InstructionError {
    match error {
        TransactionError::InstructionError(_, inner) => inner,
        other => panic!("expected an instruction error, got {other:?}"),
    }
}

async fn mint_bytes(context: &mut ProgramTestContext, mint: Pubkey) -> Vec<u8> {
    context
        .banks_client
        .get_account(mint)
        .await
        .expect("fetch mint")
        .expect("mint exists")
        .data
}

async fn mint_supply(context: &mut ProgramTestContext, mint: Pubkey) -> u64 {
    let data = mint_bytes(context, mint).await;
    Mint::parse(&data[..MINT_BYTES]).expect("mint base").supply
}

async fn token_amount(context: &mut ProgramTestContext, key: Pubkey) -> u64 {
    let account = context
        .banks_client
        .get_account(key)
        .await
        .expect("fetch token account")
        .expect("token account exists")
        .data;
    TokenAccount::parse(&account[..ACCOUNT_BYTES])
        .expect("token account")
        .amount
}

/// One escrow-signer invocation over the fixed seven-account frame.
fn signer_instruction(action: EscrowSignerActionV1, fixture: &Fixture, amount: u64) -> Instruction {
    let mut data = Vec::with_capacity(ESCROW_SIGNER_INSTRUCTION_BYTES);
    data.push(action as u8);
    data.extend_from_slice(&AGGREGATE);
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(DECIMALS);
    let accounts = vec![
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new(fixture.mint, false),
        AccountMeta::new(fixture.holder_tokens, false),
        AccountMeta::new_readonly(fixture.escrow, false),
        AccountMeta::new_readonly(fixture.root, false),
        AccountMeta::new_readonly(fixture.holder.pubkey(), true),
        AccountMeta::new_readonly(fixture.stranger, false),
    ];
    assert_eq!(accounts.len(), ESCROW_SIGNER_ACCOUNT_COUNT);
    Instruction {
        program_id: ESCROW_SIGNER_PROGRAM,
        accounts,
        data,
    }
}

/// A shard Mint carrying the whole profile, plus a funded holder.
struct Fixture {
    mint: Pubkey,
    holder_tokens: Pubkey,
    holder: Keypair,
    escrow: Pubkey,
    root: Pubkey,
    stranger: Pubkey,
}

async fn build_fixture(context: &mut ProgramTestContext) -> Fixture {
    let token = token_program_id();
    let mint = Keypair::new();
    let holder = Keypair::new();
    let holder_tokens = Keypair::new();
    let (escrow, escrow_bump) =
        escrow_address(AGGREGATE, &ESCROW_SIGNER_PROGRAM).expect("escrow coordinates");
    let (root, _) = root_stand_in_address(AGGREGATE, &ESCROW_SIGNER_PROGRAM);
    let (stranger, _) = stranger_address(AGGREGATE, &ESCROW_SIGNER_PROGRAM);

    // The escrow address the campaign passes is the one the shipped seed recipe
    // names, re-derived here through `ClaimCheckEscrowSeedsV1` so that a change
    // to either the domain or the seed order fails this campaign rather than
    // silently testing some other address.
    //
    // Through the exported type, and NOT by spelling `[CLAIM_CHECK_ESCROW_SEED_V1,
    // &AGGREGATE]` by hand: the seed ORDER belongs to `dclutch-claims`, and a
    // hand-written tuple here would be a second author for it. The seam register
    // caught the hand-written version, correctly — a cross-check that restates
    // what it is checking is not a cross-check.
    let seeds = ClaimCheckEscrowSeedsV1::new(AGGREGATE).expect("escrow seeds");
    let (independently, independent_bump) =
        Pubkey::find_program_address(&seeds.as_slices(), &ESCROW_SIGNER_PROGRAM);
    assert_eq!(escrow, independently);
    assert_eq!(escrow_bump, independent_bump);
    // The bump the signer program will sign with reproduces the same address,
    // which is the half `find_program_address` alone does not state.
    assert_eq!(
        Pubkey::create_program_address(
            &seeds.with_bump(escrow_bump).as_slices(),
            &ESCROW_SIGNER_PROGRAM,
        )
        .expect("canonical escrow bump"),
        escrow
    );
    assert_ne!(escrow, root);
    assert_ne!(escrow, stranger);
    assert_ne!(root, stranger);

    // Exactly the layout `Token2022BehaviorProfileV2::read_mint` admits: base
    // Mint plus one MintCloseAuthority and one PermissionedBurn, both naming
    // the controller, no freeze authority. Not the burn half alone.
    let mint_space = ExtensionType::try_calculate_account_len::<PodMint>(&[
        ExtensionType::MintCloseAuthority,
        ExtensionType::PermissionedBurn,
    ])
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
            token_instruction::initialize_mint_close_authority(&token, &mint.pubkey(), Some(&root))
                .expect("initialize close authority"),
            permissioned_burn::initialize(&token, &mint.pubkey(), &root)
                .expect("initialize permissioned burn"),
            token_instruction::initialize_mint2(&token, &mint.pubkey(), &root, None, DECIMALS)
                .expect("initialize mint"),
        ],
        &[&mint],
    )
    .await
    .expect("create the full-profile shard mint");

    send(
        context,
        &[
            create_account(
                &context.payer.pubkey(),
                &holder_tokens.pubkey(),
                rent.minimum_balance(ACCOUNT_BYTES).max(1),
                ACCOUNT_BYTES as u64,
                &token,
            ),
            token_instruction::initialize_account3(
                &token,
                &holder_tokens.pubkey(),
                &mint.pubkey(),
                &holder.pubkey(),
            )
            .expect("initialize holder account"),
        ],
        &[&holder_tokens],
    )
    .await
    .expect("create the holder account");

    let fixture = Fixture {
        mint: mint.pubkey(),
        holder_tokens: holder_tokens.pubkey(),
        holder,
        escrow,
        root,
        stranger,
    };

    // Funded by the root itself, because the root is the Mint authority. This
    // is the leg that makes the fixture a shard Mint rather than a Mint shaped
    // like one.
    send(
        context,
        &[signer_instruction(
            EscrowSignerActionV1::MintToHolder,
            &fixture,
            MINTED,
        )],
        &[&fixture.holder],
    )
    .await
    .expect("the root mints shards to the holder");
    assert_eq!(mint_supply(context, fixture.mint).await, MINTED);
    assert_eq!(token_amount(context, fixture.holder_tokens).await, MINTED);
    fixture
}

/// The whole hand-off, with the profile run over real bytes at each stage.
#[tokio::test]
async fn a_derived_escrow_takes_the_burn_and_the_profile_follows_the_real_bytes() {
    let mut context = context_test().start_with_context().await;
    let fixture = build_fixture(&mut context).await;
    let mint_key = fixture.mint.to_bytes();
    let root_key = fixture.root.to_bytes();
    let escrow_key = fixture.escrow.to_bytes();

    // ---- Before the hand-off. The live arm admits; the compacted arm does not.
    let before = mint_bytes(&mut context, fixture.mint).await;
    let live =
        Token2022BehaviorProfileV2::read_mint(TOKEN_2022_PROGRAM_ID, mint_key, &before, root_key)
            .expect("the shard profile admits the Mint Token-2022 just wrote");
    assert_eq!(live.controller(), root_key);
    assert_eq!(live.base_supply(), MINTED);
    assert_eq!(
        Token2022BehaviorProfileV2::read_compacted_shard_mint(
            TOKEN_2022_PROGRAM_ID,
            mint_key,
            &before,
            root_key,
            escrow_key,
        ),
        Err(TokenSvmError::AuthorityMismatch),
        "a coordinate nobody has compacted is not a compacted coordinate"
    );

    // ---- A stranger cannot hand the burn over, and the stranger is a PDA.
    //      FRACCHECK proved this for a keypair; a program-derived stranger is
    //      the case that matters, because a program CAN produce a signature for
    //      an address it did not earn the authority of.
    let usurped = send(
        &mut context,
        &[signer_instruction(
            EscrowSignerActionV1::StrangerHandOver,
            &fixture,
            0,
        )],
        &[&fixture.holder],
    )
    .await
    .expect_err("only the current burn authority may hand it over");
    // What surfaces is Token-2022's OWN code, not the signer program's
    // `TokenCpi`. A failed CPI is not recoverable: the runtime propagates the
    // inner refusal and the caller's own return value never reaches the
    // transaction result. Worth pinning rather than working around, because it
    // is what a validator log will show for the real route too -- a Claims
    // compaction whose hand-off is refused reports `0x4`, not a Claims code.
    assert_eq!(
        instruction_error(usurped),
        InstructionError::Custom(TokenError::OwnerMismatch as u32),
        "a stranger re-pointing a Mint's burn authority is an owner mismatch"
    );
    assert_eq!(TokenError::OwnerMismatch as u32, 4);
    // Nothing moved, and the profile still reads the Mint the same way.
    let unchanged = mint_bytes(&mut context, fixture.mint).await;
    assert_eq!(unchanged, before);

    // ---- The hand-off, root PDA to escrow PDA.
    send(
        &mut context,
        &[signer_instruction(
            EscrowSignerActionV1::HandOverBurn,
            &fixture,
            0,
        )],
        &[&fixture.holder],
    )
    .await
    .expect("the current authority hands the burn to the derived escrow");

    // ---- After the hand-off. The two arms swap, on bytes Token-2022 wrote.
    //      This is the split-controller profile's whole claim, executed: the
    //      live arm's refusal here is what stops a compacted Mint reaching any
    //      route that requires root control of the burn.
    let after = mint_bytes(&mut context, fixture.mint).await;
    assert_ne!(after, before, "SetAuthority must have moved a byte");
    assert_eq!(
        Token2022BehaviorProfileV2::read_mint(TOKEN_2022_PROGRAM_ID, mint_key, &after, root_key,),
        Err(TokenSvmError::AuthorityMismatch),
        "the live arm must refuse a Mint whose burn the root gave away"
    );
    assert_eq!(
        Token2022BehaviorProfileV2::read_mint(TOKEN_2022_PROGRAM_ID, mint_key, &after, escrow_key,),
        Err(TokenSvmError::AuthorityMismatch),
        "nor under the escrow, which never held the Mint authority"
    );
    let compacted = Token2022BehaviorProfileV2::read_compacted_shard_mint(
        TOKEN_2022_PROGRAM_ID,
        mint_key,
        &after,
        root_key,
        escrow_key,
    )
    .expect("the compacted arm admits the shape SetAuthority left behind");
    assert_eq!(compacted.controller(), root_key);
    assert_eq!(compacted.burn_authority(), escrow_key);
    assert_eq!(compacted.base_supply(), MINTED);

    // ---- The old authority is as powerless as the market that held it.
    let stale = send(
        &mut context,
        &[signer_instruction(
            EscrowSignerActionV1::StaleRootBurn,
            &fixture,
            BURNED,
        )],
        &[&fixture.holder],
    )
    .await
    .expect_err("the root must lose the burn when it hands it over");
    assert_eq!(
        instruction_error(stale),
        InstructionError::InvalidAccountData,
        "an approver the Mint no longer names is invalid account data"
    );
    assert_eq!(mint_supply(&mut context, fixture.mint).await, MINTED);

    // ---- The redemption shape: the holder signs for their own shards and a
    //      DERIVED escrow signs as approver. This is the burn FRACR3's sentence
    //      promised, in the only form it can actually take.
    send(
        &mut context,
        &[signer_instruction(
            EscrowSignerActionV1::ApproveBurn,
            &fixture,
            BURNED,
        )],
        &[&fixture.holder],
    )
    .await
    .expect("a holder-signed, derived-escrow-approved burn is accepted");
    assert_eq!(
        mint_supply(&mut context, fixture.mint).await,
        MINTED - BURNED
    );
    assert_eq!(
        token_amount(&mut context, fixture.holder_tokens).await,
        MINTED - BURNED
    );

    // ---- And the compacted arm still reads it at the lower supply, which is
    //      the reason it reports the supply instead of pinning it: every
    //      redemption moves that number and none of them may invalidate the
    //      profile the retirement arm reads.
    let paid_down = mint_bytes(&mut context, fixture.mint).await;
    let still = Token2022BehaviorProfileV2::read_compacted_shard_mint(
        TOKEN_2022_PROGRAM_ID,
        mint_key,
        &paid_down,
        root_key,
        escrow_key,
    )
    .expect("a partly redeemed compacted Mint is still a compacted Mint");
    assert_eq!(still.base_supply(), MINTED - BURNED);
}

/// The holder alone still cannot burn, after the hand-off as before it.
///
/// The hand-off moves *which* second signature a burn needs, and nothing else.
/// Without this, the campaign above would be consistent with a hand-off that
/// had also relaxed the extension -- which is exactly the misreading the
/// original sizing made in the other direction.
#[tokio::test]
async fn the_hand_off_moves_the_second_signature_and_never_removes_it() {
    let mut context = context_test().start_with_context().await;
    let token = token_program_id();
    let fixture = build_fixture(&mut context).await;

    send(
        &mut context,
        &[signer_instruction(
            EscrowSignerActionV1::HandOverBurn,
            &fixture,
            0,
        )],
        &[&fixture.holder],
    )
    .await
    .expect("hand the burn over");

    // A standard burn is still refused outright: the extension is still there.
    let standard = token_instruction::burn_checked(
        &token,
        &fixture.holder_tokens,
        &fixture.mint,
        &fixture.holder.pubkey(),
        &[],
        BURNED,
        DECIMALS,
    )
    .expect("standard burn instruction");
    let refusal = send(&mut context, &[standard], &[&fixture.holder])
        .await
        .expect_err("a standard burn is refused while PermissionedBurn is present");
    assert_eq!(
        instruction_error(refusal),
        InstructionError::Custom(TokenError::InvalidInstruction as u32)
    );

    // And a permissioned burn naming the escrow but not carrying its signature
    // is refused too -- which is what makes the escrow's `invoke_signed` in the
    // campaign above load-bearing rather than decorative. Nobody outside the
    // signer program can produce it.
    let mut unsigned_escrow = permissioned_burn::burn_checked(
        &token,
        &fixture.holder_tokens,
        &fixture.mint,
        &fixture.escrow,
        &fixture.holder.pubkey(),
        &[],
        BURNED,
        DECIMALS,
    )
    .expect("permissioned burn instruction");
    unsigned_escrow.accounts[2].is_signer = false;
    let missing = send(&mut context, &[unsigned_escrow], &[&fixture.holder])
        .await
        .expect_err("the escrow's signature is not optional");
    assert_eq!(
        instruction_error(missing),
        InstructionError::MissingRequiredSignature
    );

    assert_eq!(mint_supply(&mut context, fixture.mint).await, MINTED);
}
