//! The fee leg in its OWN transaction, executed rather than argued.
//!
//! # The claim under test
//!
//! `docs/design/FEE_SECOND_TRANSACTION_V1.md` §1 closes with: *"Custody accepts
//! the fee request in a later transaction, unchanged, today. There is no
//! atomicity check to relax and no new binding to invent."* The argument is
//! read from source -- the Custody caller authority's `context` seed is the
//! buyer's maker replay root and never the family digest, the instructions
//! sysvar is never read on that path, and the only sequencing is the replay
//! revision plus the delegated allowance -- and it had never been run.
//!
//! When this file was written it could not be run by the shipped route. The
//! Direct inline transition derived `SellerIntermediate` and `FeeContinuation`
//! from the SAME fee register, so a fee-bearing fill projected both legs into
//! one Hot execution -- and that one execution did not fit:
//! `direct_hot_fee_bearing_margin_gate.rs` measured a key-independent floor of
//! 1,493,027 CU against a 1,400,000 ceiling, so the fee-bearing route had never
//! COMPLETED and the second Custody CPI had never returned.
//!
//! **That is history now, and the change is why this file needs reading with a
//! date attached.** The band became protocol: the transition pins the fee
//! continuation's enable register to zero, `FeeSole` is retired, and the fee
//! settles in a transaction of its own through the shipped `DCLTDFS1` route.
//! `direct_hot_fee_pair.rs` executes that pair on the five REAL role ELFs and
//! is the file to read for what TRADING does. This one keeps a narrower
//! question that the pair cannot ask, below.
//!
//! # Why the caller here is a stand-in, and what that costs the result
//!
//! Custody admits a delegated transfer only from the program the activated
//! release set binds to the role the request names
//! (`authenticate_calling_release`: `receipt.program() != request.caller_program
//! -> Release`), and the caller authority is a PDA of that program, so it can
//! only be signed by it. No third program can present a Direct fee request
//! beside Trading. It has to be deployed AS the Trading role.
//!
//! So this file runs a probe release set whose Trading role is
//! `test-programs/custody-leg-caller` -- a program that decodes the projected
//! request, derives its caller authority from the request's own seeds, and
//! forwards those exact bytes to the real Custody ELF. Everything else in the
//! world is the real thing: real Custody, real Core state, real Registry
//! activation cache, the real Realm record, the real token program, and
//! byte-exact projected requests: the fixture's own for the seller leg, and
//! `project_direct_fee_request_v1` -- the one function the shipped route also
//! calls -- for the fee leg.
//!
//! What that buys, and it is the reason this file outlived the route it was
//! written against: the Custody admission for the fee leg is executed with
//! NOTHING ELSE IN THE TRANSACTION and nothing else in the instruction. The
//! pair's tx2 goes through Trading, so a Trading authentication, a config read
//! and a maker-replay write sit between the caller and Custody. Here they do
//! not, so what Custody admits and what it refuses is attributable to Custody.
//!
//! What it does NOT buy, stated so nobody reads more out of this file than is
//! in it:
//!
//! * it says nothing about whether Trading can BUILD a fee request in tx2 --
//!   `direct_hot_fee_pair.rs` is where that is answered, and this program is
//!   not a sketch of the route that answers it;
//! * its CU figures are Custody's leg plus a thin caller, not the design's tx2,
//!   which would additionally carry a Trading route's own authentication;
//! * the release set differs from the shipped one by the Trading ELF digest, so
//!   every PDA depth here is redrawn and no number in this file is comparable
//!   to a number measured on the shipped release set.
//!
//! # What is asserted
//!
//! The design's own S0/S1/S2 ledger table (§2.1) on the real accounts, plus
//! the two hostile orderings its §1.3 refusal table names. Every balance,
//! revision and allowance is read back off the bank.

use std::{env, fs, path::PathBuf};

use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CustodyReplayV1, DelegatedCustodyRequestV2, TRANSFER_ACCOUNT_COUNT_V1,
};
use dclutch_custody_sbf::CustodySbfError;
use dclutch_direct_codec::fee_settlement_v1::{
    DirectFeeProjectionV1, project_direct_fee_request_v1,
};
use dclutch_direct_codec::ordinary_geometry_v3::DirectOrdinaryGeometryV3;
use dclutch_direct_hot_program_test_support::fixture::{
    DirectCustodyLegV1, DirectTradeScenarioV1, direct_hot_custody_legs_v1,
};
use dclutch_direct_hot_program_test_support::waist::{
    CUSTODY_PROGRAM_ID, DirectCase, RefusedExecution, SuccessfulExecution, TRADING_PROGRAM_ID,
    add_lookup_table, add_release_waist, canonical_lookup_addresses, direct_case_v5,
    direct_chain_input_v5, elves, fixture_substrate, program_test_without_forced_budget,
    start_with_substrate, submit_v0_observed, with_fixture_seed,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_token_svm::{COption, TokenAccount};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_program_test::{BanksClientError, ProgramTestContext};
use solana_sdk::{signature::Signer, transaction::TransactionError};

/// The fixture draw this probe runs at.
///
/// One seed, not a sweep: this file asks whether an admission HAPPENS, which is
/// a property of the code, and the seed moves only PDA search depths. The CU
/// figures it prints are therefore one draw's and are labelled as such;
/// `direct_hot_fee_bearing_margin_gate.rs` owns the distribution question.
const PROBE_SEED: u64 = 0;

/// `CUSTODY_ROUTES_V3` slot of the leg tx1 carries.
const SELLER_INTERMEDIATE: usize = 1;
/// `CUSTODY_ROUTES_V3` slot of the leg tx2 carries.
const FEE_CONTINUATION: usize = 2;

/// Exact `CustodyFrameSpecV1::new(OperationV1::Transfer)` width, from its owner.
const TRANSFER_FRAME_ACCOUNTS: usize = TRANSFER_ACCOUNT_COUNT_V1 as usize;

/// The scenario's ledger, from `fixture::custody_registers` at `FEE_BEARING`.
///
/// Restated here as the numbers the design's §2.1 table predicts, so a reader
/// checking this file against that table is comparing two written-down ledgers
/// and not a table against an arithmetic expression.
mod ledger {
    /// The buyer's staged collateral balance.
    pub const BUYER_BEFORE: u64 = 1_000;
    /// The seller's staged collateral balance.
    pub const SELLER_BEFORE: u64 = 30;
    /// The fee recipient's staged collateral balance.
    pub const FEE_BEFORE: u64 = 40;
    /// `gross + fee`, the allowance the buyer stages.
    pub const BUYER_DEBIT: u64 = 201;
    /// `gross - fee`, moved by the seller leg.
    pub const SELLER_NET: u64 = 199;
    /// `2 * fee`, moved by the fee leg.
    pub const COMBINED_FEE: u64 = 2;
    /// The replay revision the fixture plants.
    pub const REVISION_BEFORE: u64 = 7;
}

/// Everything one probe bank needs, arranged but not yet submitted.
struct ProbeV1 {
    context: ProgramTestContext,
    direct: DirectCase,
    legs: [DirectCustodyLegV1; 4],
    /// The lookup table's address list, exactly as it is installed.
    ///
    /// Carried rather than recomputed per submission, and that is a
    /// correctness requirement rather than tidiness: `v0::Message::try_compile`
    /// turns an address into an INDEX into the list it is handed, so compiling
    /// against a shorter list than the bank holds addresses a different account
    /// and refuses somewhere with no relationship to what was tested.
    addresses: Vec<Pubkey>,
}

/// The Trading-role ELF this probe requires, read from the runner's build.
///
/// Named rather than sniffed: the whole world below is derived from whatever
/// `SBF_OUT_DIR/dclutch_trading_sbf.so` contains, so a run against the REAL
/// Trading ELF would build a coherent world in which no instruction this file
/// sends means anything. The equality below is what makes that impossible.
fn required_caller_elf() -> Vec<u8> {
    let path =
        PathBuf::from(env::var("DCLUTCH_CUSTODY_LEG_CALLER_ELF").expect(
            "DCLUTCH_CUSTODY_LEG_CALLER_ELF is required; run run-fee-second-transaction.sh",
        ));
    fs::read(path).expect("custody-leg-caller ELF")
}

/// Build one probe bank at `PROBE_SEED` on the fee-bearing scenario.
async fn arrange() -> ProbeV1 {
    let artifacts = elves();
    assert_eq!(
        artifacts.trading,
        required_caller_elf(),
        "the Trading role of this probe's release set is not the custody-leg caller; \
         SBF_OUT_DIR/dclutch_trading_sbf.so must BE that program",
    );
    let substrate = fixture_substrate();
    let (mut test, direct, legs) = with_fixture_seed(PROBE_SEED, || {
        let mut test = program_test_without_forced_budget(&artifacts);
        let releases = add_release_waist(&mut test, &artifacts);
        let direct = direct_case_v5(
            &mut test,
            releases,
            &artifacts,
            false,
            false,
            substrate,
            DirectOrdinaryGeometryV3::CANONICAL,
            DirectTradeScenarioV1::FEE_BEARING,
        );
        let input = direct_chain_input_v5(
            releases,
            &artifacts,
            substrate,
            DirectOrdinaryGeometryV3::CANONICAL,
            DirectTradeScenarioV1::FEE_BEARING,
        );
        let legs = direct_hot_custody_legs_v1(input).expect("projected Custody legs");
        (test, direct, legs)
    });
    // The legs the fixture derives independently must be the legs the fixture
    // INSTALLED, or this probe is addressing four authorities nothing planted.
    for (slot, leg) in legs.iter().enumerate() {
        let installed = direct
            .chain
            .custody_routes
            .get(slot)
            .expect("four declared Custody routes");
        assert_eq!(leg.authority, installed.authority, "leg {slot} authority");
        assert_eq!(
            leg.request_digest, installed.request_digest,
            "leg {slot} request digest",
        );
    }
    // WHY tx2 does not carry slot 2's bytes, pinned rather than left in prose,
    // because prose is what let this rot silently once. The transition retired
    // the fee continuation, so slot 2 is an inactive projection whose
    // `resulting_revision` does not advance and which therefore is not a
    // request. It is still hashed to seed the slot's caller-authority
    // coordinate, which is why the fixture still mirrors it and why deleting it
    // is not the fix. If this ever decodes again, the fee continuation is back
    // in the fill and `fee_leg` is answering a question nobody is asking.
    assert!(
        DelegatedCustodyRequestV2::decode(&legs.get(FEE_CONTINUATION).expect("fee slot").request,)
            .is_err(),
        "slot {FEE_CONTINUATION} decodes as a Custody request again: the fee \
         continuation is no longer retired, so what tx2 should carry is a \
         decision to remake rather than a projection to follow",
    );
    // Addresses only. The seller entry is the instruction tx1 submits; the fee
    // entry is here for its FRAME, which is the fourteen coordinates tx2 also
    // carries -- its request bytes are the retired slot-2 shape and `fee_leg`
    // rather than this is what tx2 sends. The caller authority tx2 derives
    // depends on the replay it is projected against, so it differs per arm and
    // rides as a static key, exactly as the sibling fee-pair settlement does.
    let instructions = [
        leg_instruction(&legs, SELLER_INTERMEDIATE),
        leg_instruction(&legs, FEE_CONTINUATION),
    ];
    let addresses = canonical_lookup_addresses(&instructions, direct.payer.pubkey());
    add_lookup_table(&mut test, &addresses);
    let context = start_with_substrate(test, substrate).await;
    ProbeV1 {
        context,
        direct,
        legs,
        addresses,
    }
}

/// One leg's transaction: the fourteen Transfer coordinates, then the callee.
///
/// The privileges are the ones `CustodyFrameSpecV1` declares and Custody
/// re-checks in `require_account_count`; coordinate zero's signer bit is the
/// one the caller program supplies, so it is FALSE here and true inside the CPI.
fn leg_instruction(legs: &[DirectCustodyLegV1; 4], slot: usize) -> Instruction {
    let leg = legs.get(slot).expect("declared Custody route");
    assert_eq!(
        leg.frame.len(),
        TRANSFER_FRAME_ACCOUNTS,
        "the Custody Transfer frame is fourteen accounts",
    );
    let mut accounts = leg
        .frame
        .iter()
        .enumerate()
        .map(|(coordinate, key)| {
            // Replay, transfer source and transfer destination; every other
            // coordinate of a Transfer frame is read-only.
            if matches!(coordinate, 8 | 10 | 11) {
                AccountMeta::new(*key, false)
            } else {
                AccountMeta::new_readonly(*key, false)
            }
        })
        .collect::<Vec<_>>();
    accounts.push(AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false));
    Instruction {
        program_id: TRADING_PROGRAM_ID,
        accounts,
        data: leg.request.clone(),
    }
}

/// The fee leg of the SECOND transaction, projected from the live replay.
///
/// # Why this is not `leg_instruction(&legs, FEE_CONTINUATION)`
///
/// It was, until the fee left the fill. `CUSTODY_ROUTES_V3` slot 2 used to
/// carry a real `DelegatedCustodyRequestV2`: the transition enabled
/// `FeeContinuation` and projected the fee transfer beside the seller's. Since
/// the band became protocol the fee settles in a transaction of its own, so the
/// transition PINS that enable register to zero and slot 2 is inactive
/// projection bytes -- `ordinary_route_projection_v3`'s own canonical test now
/// asserts that slots 0, 2 and 3 do not decode. Those bytes are still hashed to
/// seed the slot's caller-authority coordinate, which is why the fixture keeps
/// mirroring them, but they are not a request any more: their
/// `resulting_revision` no longer advances past `expected_revision`, so
/// `CustodyRequestV1::validate` refuses them `RevisionOverflow`. Forwarded
/// here, they died inside the caller program on `InvalidInstructionData`
/// without Custody ever being reached, which is a probe measuring nothing.
///
/// So the bytes come from `project_direct_fee_request_v1`. That is the ONE
/// function that projects this request, for the reason its own docstring gives:
/// the caller authority's sixth seed is the digest of these bytes, so a second
/// builder addresses a PDA nothing signs the moment the two drift. It is the
/// same function the shipped `DCLTDFS1` route calls, which is what keeps this
/// probe pointed at the request Custody actually receives in tx2 rather than at
/// a retired shape. Every value it reads is read off the bank.
async fn fee_leg(context: &mut ProgramTestContext, legs: &[DirectCustodyLegV1; 4]) -> Instruction {
    let frame = legs
        .get(FEE_CONTINUATION)
        .expect("declared Custody route")
        .frame
        .clone();
    let at = |index: usize| -> Pubkey {
        frame
            .get(index)
            .copied()
            .expect("a Transfer frame coordinate")
    };
    let live = replay(context, at(8)).await;
    let source = at(10);
    let destination = at(11);
    let source_owner = token(context, source).await.owner;
    let destination_owner = token(context, destination).await.owner;
    let request = project_direct_fee_request_v1(DirectFeeProjectionV1 {
        replay: live,
        // The obligation the design's §2.1 ledger predicts, and the same number
        // this file asserts standing as the residual delegation after tx1.
        fee_owed: ledger::COMBINED_FEE,
        source: source.to_bytes(),
        source_owner,
        destination: destination.to_bytes(),
        destination_owner,
        mint: at(9).to_bytes(),
        token_program: at(13).to_bytes(),
        custody_authority: at(12).to_bytes(),
    })
    .expect("the projected fee request");
    let data = request.encode().expect("encoded fee request").to_vec();
    // The caller program re-derives this from the bytes and refuses if it
    // disagrees, so coordinate zero is the one the digest names and not the
    // slot's own retired authority.
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(live.release_set).expect("release set"),
        live.market,
        ExecutionRoleV1::Trading,
        live.context,
        hash(&data).to_bytes(),
    )
    .expect("caller authority seeds");
    let (authority, _) = Pubkey::find_program_address(&seeds.as_slices(), &TRADING_PROGRAM_ID);
    let mut accounts = frame
        .iter()
        .enumerate()
        .map(|(coordinate, key)| {
            let key = if coordinate == 0 { authority } else { *key };
            if matches!(coordinate, 8 | 10 | 11) {
                AccountMeta::new(key, false)
            } else {
                AccountMeta::new_readonly(key, false)
            }
        })
        .collect::<Vec<_>>();
    accounts.push(AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false));
    Instruction {
        program_id: TRADING_PROGRAM_ID,
        accounts,
        data,
    }
}

/// Custody's own compute consumption, read out of the program log.
fn custody_units(logs: &[String]) -> Option<u64> {
    let prefix = format!("Program {CUSTODY_PROGRAM_ID} consumed ");
    logs.iter().find_map(|line| {
        let tail = line.strip_prefix(&prefix)?;
        tail.split_whitespace().next()?.parse::<u64>().ok()
    })
}

/// The custom program code a refusal carried, when it carried one.
fn refusal_code(error: &BanksClientError) -> Option<u32> {
    match error {
        BanksClientError::TransactionError(TransactionError::InstructionError(
            _,
            solana_program::instruction::InstructionError::Custom(code),
        ))
        | BanksClientError::SimulationError {
            err:
                TransactionError::InstructionError(
                    _,
                    solana_program::instruction::InstructionError::Custom(code),
                ),
            ..
        } => Some(*code),
        _ => None,
    }
}

/// Everything a reader needs from a refusal, as one line plus its log.
fn describe(refusal: &RefusedExecution) -> String {
    format!(
        "{:?} code {:?}\n{}",
        refusal.error,
        refusal_code(&refusal.error),
        refusal.logs.join("\n"),
    )
}

/// The refusal a hostile arm must have carried.
///
/// `Result::expect_err` cannot say this: a `SuccessfulExecution` is not `Debug`,
/// and the number worth reporting when a supposedly hostile arm EXECUTES is what
/// it cost, not its whole metadata.
fn refused(outcome: Result<SuccessfulExecution, RefusedExecution>, what: &str) -> RefusedExecution {
    match outcome {
        Err(refusal) => Ok(refusal),
        Ok(execution) => Err(format!(
            "{what}: the transaction EXECUTED, at {} CU",
            execution.compute_units_consumed,
        )),
    }
    .expect("a refusal")
}

/// The execution an arm must have had, with the refusal's log if it had none.
fn executed(
    outcome: Result<SuccessfulExecution, RefusedExecution>,
    what: &str,
) -> SuccessfulExecution {
    outcome
        .map_err(|refusal| {
            format!(
                "{what}: refused rather than executed -- {}",
                describe(&refusal)
            )
        })
        .expect("an execution")
}

async fn token(context: &mut ProgramTestContext, key: Pubkey) -> TokenAccount {
    let account = context
        .banks_client
        .get_account(key)
        .await
        .expect("bank read")
        .expect("staged token account");
    TokenAccount::parse(&account.data).expect("base token account")
}

async fn replay(context: &mut ProgramTestContext, key: Pubkey) -> CustodyReplayV1 {
    let account = context
        .banks_client
        .get_account(key)
        .await
        .expect("bank read")
        .expect("staged Custody replay");
    CustodyReplayV1::decode(&account.data).expect("Custody replay")
}

/// The three collateral accounts, in the fixture's own order.
fn collateral(direct: &DirectCase) -> (Pubkey, Pubkey, Pubkey) {
    let accounts = direct.chain.collateral_accounts;
    (accounts[0], accounts[1], accounts[2])
}

/// tx1 the seller leg, tx2 the fee leg, and the ledger after each.
#[tokio::test]
async fn the_fee_leg_executes_in_its_own_later_transaction() {
    let ProbeV1 {
        mut context,
        direct,
        legs,
        addresses,
    } = arrange().await;
    let (buyer, seller, fee) = collateral(&direct);
    let custody_replay = direct.chain.custody_replay;
    let custody_authority = legs
        .get(FEE_CONTINUATION)
        .and_then(|leg| leg.frame.get(12))
        .copied()
        .expect("the Transfer frame's Custody authority coordinate");

    // S0, the prestate the fixture stages.
    let before = token(&mut context, buyer).await;
    assert_eq!(before.amount, ledger::BUYER_BEFORE);
    assert_eq!(before.delegated_amount, ledger::BUYER_DEBIT);
    assert_eq!(before.delegate, COption::Some(custody_authority.to_bytes()));
    assert_eq!(
        token(&mut context, seller).await.amount,
        ledger::SELLER_BEFORE
    );
    assert_eq!(token(&mut context, fee).await.amount, ledger::FEE_BEFORE);
    assert_eq!(
        replay(&mut context, custody_replay).await.next_revision,
        ledger::REVISION_BEFORE,
    );

    // tx1: the seller leg alone.
    let first = leg_instruction(&legs, SELLER_INTERMEDIATE);
    let landed = executed(
        submit_v0_observed(
            &mut context,
            &[first],
            addresses.clone(),
            Some(&direct.payer),
            &[],
        )
        .await,
        "tx1 (seller-intermediate)",
    );
    println!(
        "FEE2TX\ttx1\tseller-intermediate\ttransaction {} CU\tcustody {:?} CU",
        landed.compute_units_consumed,
        custody_units(&landed.logs),
    );

    // S1: the fee is owed, and the obligation is the residual SPL delegation.
    let staged = token(&mut context, buyer).await;
    assert_eq!(staged.amount, ledger::BUYER_BEFORE - ledger::SELLER_NET);
    assert_eq!(staged.delegated_amount, ledger::COMBINED_FEE);
    assert_eq!(staged.delegate, COption::Some(custody_authority.to_bytes()));
    assert_eq!(
        token(&mut context, seller).await.amount,
        ledger::SELLER_BEFORE + ledger::SELLER_NET,
    );
    assert_eq!(
        token(&mut context, fee).await.amount,
        ledger::FEE_BEFORE,
        "the fee has not moved yet",
    );
    let staged_replay = replay(&mut context, custody_replay).await;
    assert_eq!(staged_replay.next_revision, ledger::REVISION_BEFORE + 1);
    assert_eq!(
        staged_replay.last_request_digest,
        legs.get(SELLER_INTERMEDIATE)
            .expect("seller leg")
            .request_digest,
        "the replay records the seller leg as the last request, which is the \
         parent digest the design's tx2 would read",
    );

    // A new block between the two, so "a later transaction" is also a later
    // SLOT and no same-block artefact can be carrying the result. Custody reads
    // no Clock on this path; this costs nothing and closes a reading.
    context
        .warp_to_slot(fixture_substrate().bank_slot() + 1)
        .expect("one block between the legs");

    // tx2: the fee leg, in a transaction of its own, projected from the replay
    // tx1 left behind and carrying no authority a keypair could sign.
    let second = fee_leg(&mut context, &legs).await;
    let landed = executed(
        submit_v0_observed(
            &mut context,
            std::slice::from_ref(&second),
            addresses,
            Some(&direct.payer),
            &[],
        )
        .await,
        "tx2 (fee-continuation)",
    );
    // The account frame, counted three ways, because the design's §4 estimate
    // ("an 18-21 account frame") is about a transaction and the contract's
    // number is about a CPI: the Custody Transfer frame, the instruction that
    // carries it plus the callee, and the whole transaction once the fee payer
    // and the duplicate caller-program key are resolved.
    let mut resolved = second
        .accounts
        .iter()
        .map(|meta| meta.pubkey)
        .chain([second.program_id, direct.payer.pubkey()])
        .collect::<Vec<_>>();
    resolved.sort_unstable_by_key(Pubkey::to_bytes);
    resolved.dedup();
    println!(
        "FEE2TX\ttx2\tfee-continuation\ttransaction {} CU\tcustody {:?} CU\t\
         custody frame {TRANSFER_FRAME_ACCOUNTS}\tinstruction {}\ttransaction {}",
        landed.compute_units_consumed,
        custody_units(&landed.logs),
        second.accounts.len(),
        resolved.len(),
    );

    // S2: the design's predicted terminal state.
    let settled = token(&mut context, buyer).await;
    assert_eq!(settled.amount, ledger::BUYER_BEFORE - ledger::BUYER_DEBIT);
    assert_eq!(settled.delegated_amount, 0);
    assert!(
        settled.delegate.is_none(),
        "a terminal delegated transfer must leave no residual delegation",
    );
    assert_eq!(
        token(&mut context, seller).await.amount,
        ledger::SELLER_BEFORE + ledger::SELLER_NET,
    );
    assert_eq!(
        token(&mut context, fee).await.amount,
        ledger::FEE_BEFORE + ledger::COMBINED_FEE,
    );
    let settled_replay = replay(&mut context, custody_replay).await;
    assert_eq!(settled_replay.next_revision, ledger::REVISION_BEFORE + 2);
    assert_eq!(
        settled_replay.last_request_digest,
        hash(&second.data).to_bytes(),
        "the replay records the fee request this transaction carried, which is \
         the digest that seeded the authority Custody just admitted",
    );
}

/// tx2 submitted first refuses, and moves nothing.
///
/// §1.3's refusal table sends this row to `CustodyReplayV1::advance` and
/// `ReplayRevisionMismatch`. It refuses earlier than that, and the code says so.
#[tokio::test]
async fn the_fee_leg_refuses_before_the_seller_leg_has_landed() {
    let ProbeV1 {
        mut context,
        direct,
        legs,
        addresses,
    } = arrange().await;
    let (buyer, _, fee) = collateral(&direct);
    // Projected against the replay as it stands BEFORE the fill, which is what
    // a caller who ran ahead of the seller leg would have to present: the
    // revision it names is admissible, and the allowance it declares is the
    // obligation rather than the whole staged debit.
    let second = fee_leg(&mut context, &legs).await;
    let refusal = refused(
        submit_v0_observed(&mut context, &[second], addresses, Some(&direct.payer), &[]).await,
        "a fee leg with no seller leg before it",
    );
    let code = refusal_code(&refusal.error);
    println!(
        "FEE2TX\tout-of-order\tREFUSED\tcode {code:?}\tallowance {} replay {}",
        CustodySbfError::TokenState as u32,
        CustodySbfError::Replay as u32,
    );
    assert_eq!(
        code,
        Some(CustodySbfError::TokenState as u32),
        "the live allowance is read before the replay advances, so an \
         out-of-order fee leg refuses at the delegation and not at the revision",
    );
    assert_eq!(
        token(&mut context, buyer).await.amount,
        ledger::BUYER_BEFORE
    );
    assert_eq!(token(&mut context, fee).await.amount, ledger::FEE_BEFORE);
    assert_eq!(
        replay(&mut context, direct.chain.custody_replay)
            .await
            .next_revision,
        ledger::REVISION_BEFORE,
    );
}

/// tx2 replayed after it landed refuses, and moves nothing.
#[tokio::test]
async fn the_fee_leg_refuses_when_it_is_replayed() {
    let ProbeV1 {
        mut context,
        direct,
        legs,
        addresses,
    } = arrange().await;
    let (buyer, _, fee) = collateral(&direct);
    executed(
        submit_v0_observed(
            &mut context,
            &[leg_instruction(&legs, SELLER_INTERMEDIATE)],
            addresses.clone(),
            Some(&direct.payer),
            &[],
        )
        .await,
        "the seller leg this arm's fee leg continues",
    );
    // Built once and submitted twice, because "replayed" means these exact
    // bytes: re-projecting after the revision advanced would build a DIFFERENT
    // request, and the refusal it earned would be about the new one.
    let second = fee_leg(&mut context, &legs).await;
    executed(
        submit_v0_observed(
            &mut context,
            std::slice::from_ref(&second),
            addresses.clone(),
            Some(&direct.payer),
            &[],
        )
        .await,
        "the fee leg this arm then replays",
    );
    let settled = token(&mut context, fee).await.amount;
    // A new block, so the replayed transaction is a new transaction rather than
    // a duplicate signature the bank rejects before it reaches any program.
    context
        .warp_to_slot(fixture_substrate().bank_slot() + 1)
        .expect("one block forward");
    let refusal = refused(
        submit_v0_observed(&mut context, &[second], addresses, Some(&direct.payer), &[]).await,
        "a replayed fee leg",
    );
    let code = refusal_code(&refusal.error);
    println!("FEE2TX\treplayed\tREFUSED\tcode {code:?}");
    assert_eq!(
        code,
        Some(CustodySbfError::TokenState as u32),
        "the allowance the first fee leg consumed is gone, so the replay refuses \
         at the delegation before the revision is ever compared",
    );
    assert_eq!(token(&mut context, fee).await.amount, settled);
    assert_eq!(
        token(&mut context, buyer).await.delegated_amount,
        0,
        "no residual allowance survives the terminal leg",
    );
    assert_eq!(
        replay(&mut context, direct.chain.custody_replay)
            .await
            .next_revision,
        ledger::REVISION_BEFORE + 2,
    );
}
