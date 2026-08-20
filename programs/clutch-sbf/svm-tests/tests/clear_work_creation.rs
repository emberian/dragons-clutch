//! Real-SBF evidence for the staged creation of the clearing checkpoint.
//!
//! T2-3 (Tier 2 join 6, creation half): `Intent::InitClearWork` (tag 47)
//! funds the **full final rent**, allocates the first 10,240 bytes at the
//! canonical `(epoch, candidate)` PDA and writes the resumable grow-stage
//! prefix; `Intent::GrowClearWork` (tag 48) reallocs one step at a time, and
//! the final grow writes the real header plus the `ClearWorkV1::NEW` idle
//! body in the same instruction.  Claim plane: SBF-EXECUTED (bank), no
//! promotion — nothing consumes the created account yet, and this file
//! proves the *refusals* that make a half-grown account inert as directly as
//! it proves the happy path.
//!
//! The oracle is the layout codec byte-for-byte (the reference adapter
//! refuses both intents as `UnsupportedIntent`, per the genesis precedent),
//! and the idle body is compared against the same
//! `ClearWorkV1::encode_idle_into` writer the program itself runs.

use {
    clutch_batch::relation_v1_stream::ClearWorkV1,
    clutch_sbf::{
        error::ClutchError,
        instructions::orders_batch::clear_work::{
            GROW_CLEAR_WORK_ACCOUNT_COUNT, INIT_CLEAR_WORK_ACCOUNT_COUNT,
        },
        seeds,
    },
    clutch_solana_layout::{
        account_len, canonical_epoch_id,
        clearing::{
            advance_walk, bind_order_set, clear_work_body, clear_work_body_mut,
            clear_work_grow_stage_len, clear_work_grown_len, complete_clear_work,
            verify_clear_work, ClearWorkGrowStage, CLEAR_WORK_GROW_PREFIX_BYTES,
            CLEAR_WORK_GROW_STEP, CLEAR_WORK_STATUS_OPEN,
        },
        CodecError, Hash32, Intent, CLEAR_WORK_BODY_BYTES,
    },
    clutch_svm_fixture::{layout_request, PROGRAM_ID, RENT_SYSVAR, SYSTEM_PROGRAM},
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_test::{tokio, BanksClient, ProgramTest},
    solana_signer::Signer,
    solana_system_interface::instruction as system_instruction,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

const EPOCH_INDEX: u64 = 7;

fn h(byte: u8) -> Hash32 {
    Hash32::from_bytes([byte; 32])
}

/// The identity triple every test binds: market, epoch, candidate.
fn ids(candidate_byte: u8) -> (Hash32, Hash32, Hash32) {
    let market = h(0x71);
    (market, canonical_epoch_id(market, EPOCH_INDEX), h(candidate_byte))
}

fn work_address(epoch: &Hash32, candidate: &Hash32) -> (Address, u8) {
    Address::find_program_address(
        &[seeds::SEED_CLEAR_WORK, &epoch.bytes(), &candidate.bytes()],
        &PROGRAM_ID,
    )
}

/// The exact rent principal init moves: full and final, at the first stage.
fn final_principal() -> u64 {
    let principal = solana_rent::Rent::default().minimum_balance(account_len::CLEAR_WORK);
    // The envelope pin: rent = (128 + bytes) x 6,960 at default parameters.
    assert_eq!(principal, (128 + account_len::CLEAR_WORK as u64) * 6_960);
    assert_eq!(principal, 334_998_720);
    principal
}

fn init_instruction(
    payer: Address,
    work: Address,
    market: Hash32,
    epoch: Hash32,
    candidate: Hash32,
) -> Instruction {
    let metas = vec![
        AccountMeta::new(payer, true),
        AccountMeta::new(work, false),
        AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
        AccountMeta::new_readonly(RENT_SYSVAR, false),
    ];
    assert_eq!(metas.len(), INIT_CLEAR_WORK_ACCOUNT_COUNT);
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::InitClearWork {
                market,
                epoch,
                candidate,
            },
        ),
        metas,
    )
}

/// One grow at one stage.  `sequence` is the staged-length counter the
/// program requires: `len / step`, so 1, 2, 3, 4 across a full creation.
fn grow_instruction(
    work: Address,
    market: Hash32,
    epoch: Hash32,
    candidate: Hash32,
    sequence: u64,
) -> Instruction {
    let metas = vec![AccountMeta::new(work, false)];
    assert_eq!(metas.len(), GROW_CLEAR_WORK_ACCOUNT_COUNT);
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            sequence,
            Intent::GrowClearWork {
                market,
                epoch,
                candidate,
            },
        ),
        metas,
    )
}

async fn start(extra: Vec<(Address, Account)>) -> (BanksClient, Keypair) {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("clutch_sbf", PROGRAM_ID, None);
    for (address, account) in extra {
        test.add_account(address, account);
    }
    let (banks, payer, _) = test.start().await;
    (banks, payer)
}

/// Send instructions in one transaction; return the result and consumed CU.
///
/// No compute-budget instruction: every creation instruction must fit the
/// per-instruction default, and the returned units are exactly what the sent
/// instructions consumed.
async fn send(
    banks: &mut BanksClient,
    payer: &Keypair,
    instructions: &[Instruction],
) -> (Result<(), TransactionError>, u64) {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let transaction =
        Transaction::new_signed_with_payer(instructions, Some(&payer.pubkey()), &[payer], blockhash);
    let outcome = banks
        .process_transaction_with_metadata(transaction)
        .await
        .unwrap();
    let units = outcome
        .metadata
        .map(|metadata| metadata.compute_units_consumed)
        .unwrap_or_default();
    (outcome.result, units)
}

async fn account(banks: &mut BanksClient, address: Address) -> Option<Account> {
    banks.get_account(address).await.unwrap()
}

fn custom(result: Result<(), TransactionError>) -> u32 {
    match result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => code,
        other => panic!("expected custom refusal, got {other:?}"),
    }
}

/// Every ClearWork-consuming read that exists today, refused on a half-grown
/// account.  These layout readers are the *only* consumers of checkpoint
/// bytes in the tree (the on-chain walk is T2-6b, and `SettlePage`'s
/// preflight reads through the same `verify_clear_work`), so driving each of
/// them is driving the complete consuming surface.
fn assert_half_grown_is_inert(data: &[u8]) {
    assert!(clear_work_grow_stage_len(data.len()).is_ok());
    assert_eq!(verify_clear_work(data), Err(CodecError::Truncated));
    assert_eq!(clear_work_body(data), Err(CodecError::Truncated));
    let mut copy = data.to_vec();
    assert_eq!(clear_work_body_mut(&mut copy), Err(CodecError::Truncated));
    assert_eq!(
        bind_order_set(&mut copy, h(0x5e), 1),
        Err(CodecError::Truncated)
    );
    assert_eq!(advance_walk(&mut copy, 0, 1, 1), Err(CodecError::Truncated));
    assert_eq!(complete_clear_work(&mut copy), Err(CodecError::Truncated));
}

/// The finished checkpoint, checked byte for byte: real header, idle body.
fn assert_finished_checkpoint(
    account: &Account,
    market: Hash32,
    epoch: Hash32,
    candidate: Hash32,
    bump: u8,
    expected_lamports: u64,
) {
    assert_eq!(account.owner, PROGRAM_ID);
    assert_eq!(account.data.len(), account_len::CLEAR_WORK);
    assert_eq!(account.lamports, expected_lamports);
    let header = verify_clear_work(&account.data).unwrap();
    assert_eq!(header.market, market);
    assert_eq!(header.epoch, epoch);
    assert_eq!(header.candidate, candidate);
    assert_eq!(header.order_set, Hash32::ZERO);
    assert_eq!(header.consumed_fold, 0);
    assert_eq!(header.body_len as usize, CLEAR_WORK_BODY_BYTES);
    assert_eq!(header.page_cursor, 0);
    assert_eq!(header.live_rank, 0);
    assert_eq!(header.slot_cursor, 0);
    assert_eq!(header.status, CLEAR_WORK_STATUS_OPEN);
    assert_eq!(header.stored_bump, bump);
    // The body is the idle checkpoint, written by the same writer host-side.
    let mut idle = vec![0u8; CLEAR_WORK_BODY_BYTES];
    ClearWorkV1::encode_idle_into(&mut idle).unwrap();
    assert_eq!(clear_work_body(&account.data).unwrap(), &idle[..]);
    // And a finished checkpoint is no longer a grow stage.
    assert_eq!(
        ClearWorkGrowStage::decode(&account.data),
        Err(CodecError::InvalidCount)
    );
}

#[tokio::test]
async fn five_instructions_in_one_transaction_create_the_exact_checkpoint() {
    let (market, epoch, candidate) = ids(0x72);
    let (work, bump) = work_address(&epoch, &candidate);
    let (mut banks, payer) = start(Vec::new()).await;

    let instructions = [
        init_instruction(payer.pubkey(), work, market, epoch, candidate),
        grow_instruction(work, market, epoch, candidate, 1),
        grow_instruction(work, market, epoch, candidate, 2),
        grow_instruction(work, market, epoch, candidate, 3),
        grow_instruction(work, market, epoch, candidate, 4),
    ];
    let (result, units) = send(&mut banks, &payer, &instructions).await;
    result.unwrap();
    eprintln!("ClearWork five-in-one-transaction total CU: {units}");

    let finished = account(&mut banks, work).await.expect("created");
    assert_finished_checkpoint(&finished, market, epoch, candidate, bump, final_principal());
}

#[tokio::test]
async fn five_transactions_create_the_checkpoint_with_exact_rent_at_every_stage() {
    let (market, epoch, candidate) = ids(0x73);
    let (work, bump) = work_address(&epoch, &candidate);
    let (mut banks, payer) = start(Vec::new()).await;
    let principal = final_principal();

    let (result, units) = send(
        &mut banks,
        &payer,
        &[init_instruction(payer.pubkey(), work, market, epoch, candidate)],
    )
    .await;
    result.unwrap();
    eprintln!("InitClearWork CU: {units}");

    let mut expected_len = CLEAR_WORK_GROW_STEP;
    let created = account(&mut banks, work).await.expect("created");
    assert_eq!(created.owner, PROGRAM_ID);
    assert_eq!(created.data.len(), expected_len);
    // Rent exactness: the FULL final principal at the FIRST stage.
    assert_eq!(created.lamports, principal);
    let stage = ClearWorkGrowStage::decode(&created.data).unwrap();
    assert_eq!(stage.market, market);
    assert_eq!(stage.epoch, epoch);
    assert_eq!(stage.candidate, candidate);
    assert_eq!(stage.target_len as usize, account_len::CLEAR_WORK);
    assert_eq!(stage.stored_bump, bump);
    assert!(created.data[CLEAR_WORK_GROW_PREFIX_BYTES..]
        .iter()
        .all(|byte| *byte == 0));
    assert_half_grown_is_inert(&created.data);

    for grow in 1..=4u64 {
        let (result, units) = send(
            &mut banks,
            &payer,
            &[grow_instruction(work, market, epoch, candidate, grow)],
        )
        .await;
        result.unwrap();
        eprintln!("GrowClearWork stage {grow} CU: {units}");
        expected_len = clear_work_grown_len(expected_len);
        let grown = account(&mut banks, work).await.expect("exists");
        assert_eq!(grown.data.len(), expected_len);
        // Rent exactness per stage: grows never move lamports.
        assert_eq!(grown.lamports, principal);
        if expected_len < account_len::CLEAR_WORK {
            // The prefix rides every realloc, and the half-grown account
            // still refuses every consuming read.
            assert_eq!(ClearWorkGrowStage::decode(&grown.data), Ok(stage));
            assert_half_grown_is_inert(&grown.data);
        }
    }
    assert_eq!(expected_len, account_len::CLEAR_WORK);
    let finished = account(&mut banks, work).await.expect("finished");
    assert_finished_checkpoint(&finished, market, epoch, candidate, bump, principal);
}

#[tokio::test]
async fn duplicate_wrong_pda_wrong_bump_and_finished_grows_refuse() {
    let (market, epoch, candidate) = ids(0x74);
    let (work, _bump) = work_address(&epoch, &candidate);

    // A hostile genesis fixture: a half-grown-shaped account at the canonical
    // PDA of a SECOND candidate, whose stored bump is wrong.  Only the
    // program can write a true prefix; this one is forged off-chain exactly
    // to prove the grower re-derives rather than trusts.
    let forged_candidate = h(0x75);
    let (forged_work, forged_bump) = work_address(&epoch, &forged_candidate);
    let wrong_bump = forged_bump.wrapping_sub(1);
    let mut forged = vec![0u8; CLEAR_WORK_GROW_STEP];
    let forged_stage = ClearWorkGrowStage {
        market,
        epoch,
        candidate: forged_candidate,
        target_len: account_len::CLEAR_WORK as u32,
        stored_bump: wrong_bump,
    };
    forged_stage.encode(&mut forged).unwrap();
    let (mut banks, payer) = start(vec![(
        forged_work,
        Account {
            lamports: final_principal(),
            data: forged,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )])
    .await;

    // Create the real checkpoint completely.
    let instructions = [
        init_instruction(payer.pubkey(), work, market, epoch, candidate),
        grow_instruction(work, market, epoch, candidate, 1),
        grow_instruction(work, market, epoch, candidate, 2),
        grow_instruction(work, market, epoch, candidate, 3),
        grow_instruction(work, market, epoch, candidate, 4),
    ];
    let (result, _) = send(&mut banks, &payer, &instructions).await;
    result.unwrap();
    let before = account(&mut banks, work).await.expect("finished");

    // Duplicate init refuses: the target exists.
    let (result, _) = send(
        &mut banks,
        &payer,
        &[init_instruction(payer.pubkey(), work, market, epoch, candidate)],
    )
    .await;
    assert_eq!(custom(result), ClutchError::AlreadyInitialized as u32);

    // Wrong PDA refuses: a fresh address that is not the intent's derivation.
    let (other, _) = work_address(&epoch, &h(0x76));
    let (result, _) = send(
        &mut banks,
        &payer,
        &[init_instruction(payer.pubkey(), other, market, epoch, candidate)],
    )
    .await;
    assert_eq!(custom(result), ClutchError::WrongPda as u32);

    // A finished checkpoint accepts no further grow: the staged-length rule
    // refuses its exact length before one prefix byte is read.
    let (result, _) = send(
        &mut banks,
        &payer,
        &[grow_instruction(work, market, epoch, candidate, 4)],
    )
    .await;
    assert_eq!(custom(result), 0x1000 + 4, "codec InvalidCount");

    // A grow whose intent names a different candidate than the stored prefix
    // refuses on the triple comparison.
    let (result, _) = send(
        &mut banks,
        &payer,
        &[grow_instruction(forged_work, market, epoch, candidate, 1)],
    )
    .await;
    assert_eq!(custom(result), ClutchError::MismatchedState as u32);

    // The forged prefix's wrong bump refuses even with a matching triple.
    let (result, _) = send(
        &mut banks,
        &payer,
        &[grow_instruction(
            forged_work,
            market,
            epoch,
            forged_candidate,
            1,
        )],
    )
    .await;
    assert_eq!(custom(result), ClutchError::WrongBump as u32);

    // Nothing above moved the finished checkpoint by one byte or lamport.
    let after = account(&mut banks, work).await.expect("finished");
    assert_eq!(after, before);
}

#[tokio::test]
async fn a_prefunded_pda_is_tolerated_and_never_discounts_the_principal() {
    let (market, epoch, candidate) = ids(0x77);
    let (work, bump) = work_address(&epoch, &candidate);
    let (mut banks, payer) = start(Vec::new()).await;

    // Anyone may donate to the predictable address before its constructor
    // runs; the repo's prefund-safe constructor rule says creation neither
    // blocks nor discounts.
    const PREFUND: u64 = 1_000_000;
    let (result, _) = send(
        &mut banks,
        &payer,
        &[system_instruction::transfer(&payer.pubkey(), &work, PREFUND)],
    )
    .await;
    result.unwrap();

    let instructions = [
        init_instruction(payer.pubkey(), work, market, epoch, candidate),
        grow_instruction(work, market, epoch, candidate, 1),
        grow_instruction(work, market, epoch, candidate, 2),
        grow_instruction(work, market, epoch, candidate, 3),
        grow_instruction(work, market, epoch, candidate, 4),
    ];
    let (result, _) = send(&mut banks, &payer, &instructions).await;
    result.unwrap();

    let finished = account(&mut banks, work).await.expect("created");
    assert_finished_checkpoint(
        &finished,
        market,
        epoch,
        candidate,
        bump,
        PREFUND + final_principal(),
    );
}

#[tokio::test]
async fn partial_creation_resumes_and_a_mid_sequence_failure_rolls_back_cleanly() {
    let (market, epoch, candidate) = ids(0x78);
    let (work, bump) = work_address(&epoch, &candidate);
    let (mut banks, payer) = start(Vec::new()).await;
    let principal = final_principal();

    // A creation whose transaction fails midway leaves NOTHING: the third
    // instruction claims the wrong stage, the whole transaction rolls back,
    // and the account never existed.
    let (result, _) = send(
        &mut banks,
        &payer,
        &[
            init_instruction(payer.pubkey(), work, market, epoch, candidate),
            grow_instruction(work, market, epoch, candidate, 1),
            grow_instruction(work, market, epoch, candidate, 3),
        ],
    )
    .await;
    assert_eq!(custom(result), ClutchError::Replay as u32);
    assert!(account(&mut banks, work).await.is_none());

    // Partial creation: init plus one grow, in one transaction.
    let (result, _) = send(
        &mut banks,
        &payer,
        &[
            init_instruction(payer.pubkey(), work, market, epoch, candidate),
            grow_instruction(work, market, epoch, candidate, 1),
        ],
    )
    .await;
    result.unwrap();
    let parked = account(&mut banks, work).await.expect("half-grown");
    assert_eq!(parked.data.len(), 2 * CLEAR_WORK_GROW_STEP);
    assert_eq!(parked.lamports, principal);
    assert_half_grown_is_inert(&parked.data);

    // Init on the half-grown account refuses; it is not creatable.
    let (result, _) = send(
        &mut banks,
        &payer,
        &[init_instruction(payer.pubkey(), work, market, epoch, candidate)],
    )
    .await;
    assert_eq!(custom(result), ClutchError::AlreadyInitialized as u32);

    // A later failing resume also rolls back to the parked state exactly:
    // two good grows followed by a wrong-stage grow.
    let (result, _) = send(
        &mut banks,
        &payer,
        &[
            grow_instruction(work, market, epoch, candidate, 2),
            grow_instruction(work, market, epoch, candidate, 3),
            grow_instruction(work, market, epoch, candidate, 5),
        ],
    )
    .await;
    assert_eq!(custom(result), ClutchError::Replay as u32);
    let still_parked = account(&mut banks, work).await.expect("half-grown");
    assert_eq!(still_parked, parked);

    // And the honest resume finishes creation across transactions.
    for grow in 2..=4u64 {
        let (result, _) = send(
            &mut banks,
            &payer,
            &[grow_instruction(work, market, epoch, candidate, grow)],
        )
        .await;
        result.unwrap();
    }
    let finished = account(&mut banks, work).await.expect("finished");
    assert_finished_checkpoint(&finished, market, epoch, candidate, bump, principal);
}
