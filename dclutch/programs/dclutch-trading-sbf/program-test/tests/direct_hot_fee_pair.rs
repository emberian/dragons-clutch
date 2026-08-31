//! The fee-bearing Direct trade as a PAIR of transactions, both real, both measured.
//!
//! # What this file is the instrument for
//!
//! `docs/design/FEE_SECOND_TRANSACTION_V1.md` split the fee-bearing Direct
//! trade in two because the single-transaction form is over the compute ceiling
//! by more than the whole fee leg costs. Three lanes closed the pieces:
//! the transition pinned the fee continuation off (lane A), the maker replay
//! grew `fee_owed` and the admission gate that reads it (lane B), and
//! `docs/evidence/FEE_SECOND_TRANSACTION_FOUNDATION_2026_08_30.md` proved
//! Custody admits the fee leg in a later transaction — using a stand-in caller,
//! because the Trading route did not exist.
//!
//! It exists now, and this file is the first execution of the actual pair:
//! the real Direct Hot fill on the real Trading ELF, then the real
//! `DCLTDFS1` settlement route on the same ELF, against the same bank, with the
//! ledger read back after each.
//!
//! # What it asserts
//!
//! * **The fill lands and owes.** After tx1 the buyer's maker replay records
//!   `fee_owed = combined_fee`, the residual SPL delegation is standing at the
//!   same number, and the fee recipient has not been paid.
//! * **The settlement lands and clears.** After tx2 the fee recipient holds
//!   `combined_fee` more, the delegation is gone, and `fee_owed` is zero.
//! * **Collateral is conserved in all three states**, which is the design's
//!   Identity 1 (§2.1) and the property the intermediate state does not weaken.
//! * **The settlement identity is FALSE in S1 and true in S2** — stated as its
//!   own assertion rather than hidden inside the conservation one, because
//!   "conserved" and "settled" are different words (§2.1, Identity 3).
//! * **The hostile set**, each naming its code: nothing owed, already settled,
//!   the fee routed to a stranger, the fee taken out of somebody else's
//!   account, a foreign maker replay.
//! * **E5's vanished-recipient case**: a DIFFERENT token account of the same
//!   configured recipient is admitted, because the destination is pinned by
//!   owner and never by address.
//!
//! # What it prints
//!
//! Both transactions' compute, at one fixture draw, labelled as one draw.
//! `direct_hot_fee_bearing_margin_gate.rs` owns the distribution question for
//! tx1; tx2 has over a million CU of headroom and no key-dependent refusal is
//! reachable in it (design §4.3), so a sweep here would be measuring a constant.

use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{CustodyReplayV1, TRANSFER_ACCOUNT_COUNT_V1};
use dclutch_direct_codec::fee_settlement_v1::{
    DirectFeeProjectionV1, DirectFeeSettlementReceiptV1, DirectFeeSettlementRequestV1,
    project_direct_fee_request_v1,
};
use dclutch_direct_codec::ordinary_geometry_v3::DirectOrdinaryGeometryV3;
use dclutch_direct_codec::successor::MakerReplayRootV1;
use dclutch_direct_hot_program_test_support::fixture::{
    DirectTradeScenarioV1, direct_hot_config_record_v1, direct_hot_custody_legs_v1,
};
use dclutch_direct_hot_program_test_support::waist::{
    CUSTODY_PROGRAM_ID, DirectCase, RefusedExecution, SuccessfulExecution, TRADING_PROGRAM_ID,
    add_lookup_table, add_release_waist, canonical_lookup_addresses, direct_case_v5,
    direct_chain_input_v5, direct_top_level_instructions, elves, fixture_substrate,
    program_test_without_forced_budget, start_with_substrate, submit_v0_observed,
    with_fixture_seed,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_token_svm::{ACCOUNT_BYTES, COption, TokenAccount};
use dclutch_trading_sbf::TradingSbfError;
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTestContext};
use solana_sdk::{signature::Signer, transaction::TransactionError};

/// One fixture draw. The seed moves PDA search depths and nothing else.
const PAIR_SEED: u64 = 0;

/// `CUSTODY_ROUTES_V3` slot whose Transfer frame this settlement reuses.
///
/// Every Custody `Transfer` in this market names the same thirteen coordinates
/// after the caller authority, and the fee routes name the fee destination at
/// coordinate 11 — which is the frame the settlement needs. Coordinate 0 is
/// the one that differs, because it is a function of the request's own digest,
/// and the settlement derives its own.
const FEE_CONTINUATION: usize = 2;

const TRANSFER_FRAME: usize = TRANSFER_ACCOUNT_COUNT_V1 as usize;

/// The `FEE_BEARING` scenario's ledger, restated from the design's §2.1 table.
mod ledger {
    /// The buyer's staged collateral balance.
    pub const BUYER_BEFORE: u64 = 1_000;
    /// The seller's staged collateral balance.
    pub const SELLER_BEFORE: u64 = 30;
    /// The fee recipient's staged collateral balance.
    pub const FEE_BEFORE: u64 = 40;
    /// `gross + fee`, the allowance the buyer stages.
    pub const BUYER_DEBIT: u64 = 201;
    /// `gross - fee`, moved by the fill.
    pub const SELLER_NET: u64 = 199;
    /// `2 * fee`, moved by the settlement.
    pub const COMBINED_FEE: u64 = 2;
}

/// A token account of `owner` on the fixture's mint, staged with no balance.
///
/// Two of these exist so the destination rule can be tested in both directions:
/// one belongs to the configured fee recipient and must be ADMITTED even though
/// it is not the account the fill named, and one belongs to a stranger and must
/// be refused. `Pubkey::new_from_array` rather than a keypair because neither
/// account ever signs anything.
///
/// The tags avoid `0xa1..0xa4`, which the fixture's own `key()` spends on the
/// three collateral accounts and the mint. Staging over one of those does not
/// fail loudly — it replaces the account the trade is about, and every
/// subsequent refusal is then about the wrong world.
const SPARE_RECIPIENT_ACCOUNT: Pubkey = Pubkey::new_from_array([0xf1; 32]);
const STRANGER_ACCOUNT: Pubkey = Pubkey::new_from_array([0xf2; 32]);
const STRANGER_OWNER: Pubkey = Pubkey::new_from_array([0xf3; 32]);

struct PairV1 {
    context: ProgramTestContext,
    direct: DirectCase,
    addresses: Vec<Pubkey>,
    /// The fourteen Transfer coordinates; index 0 is replaced per settlement.
    transfer_frame: Vec<Pubkey>,
    config_raw: Pubkey,
    config_staging: Pubkey,
    fill: [Instruction; 4],
}

impl PairV1 {
    fn buyer_maker_replay(&self) -> Pubkey {
        *self
            .direct
            .chain
            .maker_replays
            .get(1)
            .expect("the buyer's maker replay")
    }

    fn collateral(&self) -> (Pubkey, Pubkey, Pubkey) {
        let accounts = self.direct.chain.collateral_accounts;
        (accounts[0], accounts[1], accounts[2])
    }
}

async fn arrange() -> PairV1 {
    let artifacts = elves();
    let substrate = fixture_substrate();
    let (mut test, direct, transfer_frame, config_raw, config_staging, mint, token_program) =
        with_fixture_seed(PAIR_SEED, || {
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
            let (config_raw, config_staging) =
                direct_hot_config_record_v1(input).expect("the immutable Direct config record");
            let frame = legs
                .get(FEE_CONTINUATION)
                .expect("the fee route's Transfer frame")
                .frame
                .clone();
            assert_eq!(frame.len(), TRANSFER_FRAME);
            let mint = frame[9];
            let token_program = frame[13];
            (
                test,
                direct,
                frame,
                config_raw,
                config_staging,
                mint,
                token_program,
            )
        });
    // The two spare destinations. Staged here rather than created on chain
    // because what is under test is which of them the ROUTE admits, and an
    // account the route never looks at the creation of is the cleanest way to
    // ask that.
    let rent = Rent::default().minimum_balance(ACCOUNT_BYTES);
    for (key, owner) in [
        (SPARE_RECIPIENT_ACCOUNT, direct.payer.pubkey()),
        (STRANGER_ACCOUNT, STRANGER_OWNER),
    ] {
        test.add_account(
            key,
            Account {
                lamports: rent,
                data: TokenAccount::initialized_base_bytes(mint.to_bytes(), owner.to_bytes())
                    .expect("staged token account")
                    .to_vec(),
                owner: token_program,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    let fill = direct_top_level_instructions(&direct);
    assert_eq!(
        fill[3].program_id, TRADING_PROGRAM_ID,
        "tx1 must be the bare top-level Direct Hot instruction",
    );
    let addresses = canonical_lookup_addresses(&fill, direct.payer.pubkey());
    add_lookup_table(&mut test, &addresses);
    let context = start_with_substrate(test, substrate).await;
    PairV1 {
        context,
        direct,
        addresses,
        transfer_frame,
        config_raw,
        config_staging,
        fill,
    }
}

/// How one settlement arm differs from the canonical one.
#[derive(Clone, Copy, Default)]
struct ArmV1 {
    /// Coordinate 11: whose account the fee is routed into.
    destination: Option<Pubkey>,
    /// Coordinate 10: whose account it is taken out of.
    source: Option<Pubkey>,
    /// Coordinate 15 ONLY: a maker replay substituted under the buyer's wire.
    maker_replay: Option<Pubkey>,
    /// Coordinate 15 AND the wire: settle against a different maker entirely.
    ///
    /// Distinct from `maker_replay` because the two hostiles are different: a
    /// substituted replay is a caller lying about which account is the
    /// debtor's, and a different obligation is a caller honestly naming a maker
    /// who owes nothing.
    obligation_replay: Option<Pubkey>,
}

/// Build the settlement instruction the way a public caller would.
///
/// Every economic value is read off the bank and projected through
/// `project_direct_fee_request_v1` — the SAME function the program calls, which
/// is what makes the caller-authority PDA derivable at all. A builder that
/// reproduced §1.4's field table on its own would be addressing an authority
/// nothing signs the moment the two drifted by a byte.
async fn settlement(pair: &mut PairV1, arm: ArmV1) -> Instruction {
    let buyer_replay_key = pair.buyer_maker_replay();
    let custody_replay_key = pair.direct.chain.custody_replay;
    let root_key = pair.direct.chain.root;
    let config_raw = pair.config_raw;
    let config_staging = pair.config_staging;
    let frame = pair.transfer_frame.clone();
    let obligation_replay = arm.obligation_replay.unwrap_or(buyer_replay_key);
    let maker_replay = arm.maker_replay.unwrap_or(obligation_replay);
    let (market, maker, generation, fee_owed) = {
        let data = account_data(&mut pair.context, obligation_replay).await;
        let root = MakerReplayRootV1::decode(&data).expect("a Direct maker replay");
        (
            root.market(),
            root.maker(),
            root.generation(),
            root.fee_owed(),
        )
    };
    let replay = {
        let data = account_data(&mut pair.context, custody_replay_key).await;
        CustodyReplayV1::decode(&data).expect("the Custody replay")
    };
    let source = arm.source.unwrap_or(frame[10]);
    let destination = arm.destination.unwrap_or(frame[11]);
    let source_owner = token(&mut pair.context, source).await.owner;
    let destination_owner = token(&mut pair.context, destination).await.owner;
    // A hostile arm may name an obligation that does not exist; the projection
    // refuses a zero settlement, so those arms carry a request that will be
    // refused by the route for the same reason and address whatever authority
    // one atom would have named. That is the point: the refusal must be the
    // ROUTE's, not an unaddressable account's.
    let projected = project_direct_fee_request_v1(DirectFeeProjectionV1 {
        replay,
        fee_owed: fee_owed.max(1),
        source: source.to_bytes(),
        source_owner,
        destination: destination.to_bytes(),
        destination_owner,
        mint: frame[9].to_bytes(),
        token_program: frame[13].to_bytes(),
        custody_authority: frame[12].to_bytes(),
    })
    .expect("the projected fee request");
    let bytes = projected.encode().expect("encoded fee request");
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(replay.release_set).expect("release set"),
        replay.market,
        ExecutionRoleV1::Trading,
        replay.context,
        hash(&bytes).to_bytes(),
    )
    .expect("caller authority seeds");
    let (authority, bump) = Pubkey::find_program_address(&seeds.as_slices(), &TRADING_PROGRAM_ID);

    let wire = DirectFeeSettlementRequestV1 {
        market,
        maker,
        generation,
        caller_authority_bump: bump,
        custody_replay_bump: 0,
        custody_transfer_bump: 0,
    }
    .to_bytes()
    .expect("settlement wire");

    let mut accounts = Vec::with_capacity(19);
    for (index, key) in frame.iter().enumerate() {
        let key = match index {
            0 => authority,
            10 => source,
            11 => destination,
            _ => *key,
        };
        accounts.push(if matches!(index, 8 | 10 | 11) {
            AccountMeta::new(key, false)
        } else {
            AccountMeta::new_readonly(key, false)
        });
    }
    accounts.push(AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false));
    accounts.push(AccountMeta::new(maker_replay, false));
    accounts.push(AccountMeta::new_readonly(root_key, false));
    accounts.push(AccountMeta::new_readonly(config_raw, false));
    accounts.push(AccountMeta::new_readonly(config_staging, false));
    assert_eq!(
        accounts.len(),
        dclutch_trading_sbf::direct_fee_settlement_v1::DIRECT_FEE_SETTLEMENT_ACCOUNT_COUNT_V1,
        "the settlement frame is the width the route declares",
    );
    Instruction {
        program_id: TRADING_PROGRAM_ID,
        accounts,
        data: wire.to_vec(),
    }
}

async fn account_data(context: &mut ProgramTestContext, key: Pubkey) -> Vec<u8> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("bank read")
        .expect("live account")
        .data
}

async fn token(context: &mut ProgramTestContext, key: Pubkey) -> TokenAccount {
    TokenAccount::parse(&account_data(context, key).await).expect("base token account")
}

async fn fee_owed(pair: &mut PairV1) -> u64 {
    let key = pair.buyer_maker_replay();
    MakerReplayRootV1::decode(&account_data(&mut pair.context, key).await)
        .expect("the buyer's maker replay")
        .fee_owed()
}

/// `B + S + F`, the quantity the design's Identity 1 says never moves.
async fn conserved(pair: &mut PairV1) -> u64 {
    let (buyer, seller, fee) = pair.collateral();
    let mut total = 0_u64;
    for key in [buyer, seller, fee, SPARE_RECIPIENT_ACCOUNT, STRANGER_ACCOUNT] {
        total = total
            .checked_add(token(&mut pair.context, key).await.amount)
            .expect("collateral total");
    }
    total
}

fn refusal_code(error: &BanksClientError) -> Option<u32> {
    let transaction = match error {
        BanksClientError::TransactionError(value) => value,
        BanksClientError::SimulationError { err, .. } => err,
        _ => return None,
    };
    match transaction {
        TransactionError::InstructionError(_, InstructionError::Custom(code)) => Some(*code),
        _ => None,
    }
}

fn executed(
    outcome: Result<SuccessfulExecution, RefusedExecution>,
    what: &str,
) -> SuccessfulExecution {
    outcome
        .map_err(|refusal| {
            format!(
                "{what}: refused rather than executed -- {:?} code {:?}\n{}",
                refusal.error,
                refusal_code(&refusal.error),
                refusal.logs.join("\n"),
            )
        })
        .expect("an execution")
}

fn refused(outcome: Result<SuccessfulExecution, RefusedExecution>, what: &str) -> u32 {
    let refusal = match outcome {
        Err(refusal) => Ok(refusal),
        Ok(execution) => Err(format!(
            "{what}: the transaction EXECUTED, at {} CU",
            execution.compute_units_consumed,
        )),
    }
    .expect("a refusal");
    refusal_code(&refusal.error).unwrap_or_else(|| {
        panic!(
            "{what}: refused without a custom code -- {:?}\n{}",
            refusal.error,
            refusal.logs.join("\n"),
        )
    })
}

async fn submit_fill(pair: &mut PairV1) -> SuccessfulExecution {
    let instructions = pair.fill.clone();
    let addresses = pair.addresses.clone();
    executed(
        submit_v0_observed(
            &mut pair.context,
            &instructions,
            addresses,
            Some(&pair.direct.payer),
            &[],
        )
        .await,
        "tx1 (the fee-bearing Direct fill)",
    )
}

async fn submit_one(
    pair: &mut PairV1,
    instruction: Instruction,
) -> Result<SuccessfulExecution, RefusedExecution> {
    let addresses = pair.addresses.clone();
    submit_v0_observed(
        &mut pair.context,
        &[instruction],
        addresses,
        Some(&pair.direct.payer),
        &[],
    )
    .await
}

async fn submit_settlement(
    pair: &mut PairV1,
    arm: ArmV1,
) -> Result<SuccessfulExecution, RefusedExecution> {
    let instruction = settlement(pair, arm).await;
    submit_one(pair, instruction).await
}

/// The pair: the fill owes, the settlement pays, and nothing is created or lost.
#[tokio::test]
async fn the_fee_bearing_trade_settles_across_two_transactions() {
    let mut pair = arrange().await;
    let (buyer, seller, fee) = pair.collateral();

    // S0.
    let s0 = conserved(&mut pair).await;
    assert_eq!(token(&mut pair.context, buyer).await.amount, ledger::BUYER_BEFORE);
    assert_eq!(
        token(&mut pair.context, buyer).await.delegated_amount,
        ledger::BUYER_DEBIT
    );
    assert_eq!(token(&mut pair.context, fee).await.amount, ledger::FEE_BEFORE);

    let landed = submit_fill(&mut pair).await;
    let tx1_units = landed.compute_units_consumed;
    println!("FEEPAIR\ttx1\tdirect-hot-fill\t{tx1_units} CU");

    // S1: conserved and UNSETTLED, and those are different words.
    let s1 = conserved(&mut pair).await;
    assert_eq!(s1, s0, "collateral conservation across the fill");
    assert_eq!(
        token(&mut pair.context, buyer).await.amount,
        ledger::BUYER_BEFORE - ledger::SELLER_NET,
    );
    assert_eq!(
        token(&mut pair.context, seller).await.amount,
        ledger::SELLER_BEFORE + ledger::SELLER_NET,
        "the seller is whole the instant the fill lands",
    );
    assert_eq!(
        token(&mut pair.context, fee).await.amount,
        ledger::FEE_BEFORE,
        "the settlement identity is FALSE in S1: the fee has not moved",
    );
    assert_eq!(
        token(&mut pair.context, buyer).await.delegated_amount,
        ledger::COMBINED_FEE,
        "the residual allowance the fill deliberately left standing",
    );
    assert_eq!(
        fee_owed(&mut pair).await,
        ledger::COMBINED_FEE,
        "the fill records the obligation where the gate reads it",
    );

    let settled = executed(
        submit_settlement(&mut pair, ArmV1::default()).await,
        "tx2 (the fee settlement)",
    );
    let tx2_units = settled.compute_units_consumed;
    println!(
        "FEEPAIR\ttx2\tdirect-fee-settlement\t{tx2_units} CU\tpair {} CU",
        tx1_units + tx2_units,
    );

    // The receipt says what it did, in the wire a caller can read back.
    let (producer, receipt_bytes) = settled.return_data.expect("a settlement receipt");
    assert_eq!(producer, TRADING_PROGRAM_ID);
    let receipt =
        DirectFeeSettlementReceiptV1::decode(&receipt_bytes).expect("the settlement receipt");
    assert_eq!(receipt.settled_amount, ledger::COMBINED_FEE);
    assert_eq!(receipt.market, pair.direct.chain.market.to_bytes());
    assert_eq!(receipt.fee_destination, fee.to_bytes());
    assert_eq!(receipt.fee_source, buyer.to_bytes());
    assert_eq!(receipt.maker_root, pair.buyer_maker_replay().to_bytes());
    assert_eq!(
        receipt.expected_revision + 1,
        receipt.resulting_revision,
        "one settlement is one revision",
    );

    // S2: the design's predicted terminal state, and the identity is true again.
    assert_eq!(
        conserved(&mut pair).await,
        s0,
        "collateral conservation across the settlement",
    );
    let closed = token(&mut pair.context, buyer).await;
    assert_eq!(closed.amount, ledger::BUYER_BEFORE - ledger::BUYER_DEBIT);
    assert_eq!(closed.delegated_amount, 0);
    assert_eq!(
        closed.delegate,
        COption::None,
        "a terminal delegated transfer leaves no residual delegation",
    );
    assert_eq!(
        token(&mut pair.context, fee).await.amount,
        ledger::FEE_BEFORE + ledger::COMBINED_FEE,
    );
    assert_eq!(
        token(&mut pair.context, seller).await.amount,
        ledger::SELLER_BEFORE + ledger::SELLER_NET,
        "the settlement moved nothing of the seller's",
    );
    assert_eq!(
        fee_owed(&mut pair).await,
        0,
        "the obligation is cleared and the maker is unblocked",
    );
    // The replay this settlement advanced, read back off the bank.
    let replay = CustodyReplayV1::decode(
        &account_data(&mut pair.context, pair.direct.chain.custody_replay).await,
    )
    .expect("the Custody replay");
    assert_eq!(replay.next_revision, receipt.resulting_revision);
    assert_eq!(replay.last_request_digest, receipt.custody_request_digest);
    assert_eq!(
        replay.last_poststate_commitment,
        receipt.custody_poststate,
        "the receipt reports the commitment the replay actually holds",
    );
}

/// A settlement with nothing to settle refuses, and zero has one meaning.
///
/// Zero is both "never owed" and "already settled" (`FEE_SECOND_TRANSACTION_V1`
/// §2.4 invariant 3), and they are the same state: nothing to move and nobody
/// blocked. The route says so with one code, and this pins that the code is
/// reached from BOTH directions rather than one of them being some other
/// refusal that happens to look alike.
///
/// "Never owed" is the SELLER's replay, deliberately. It is a real Trading PDA
/// of the same market at the same generation, created by the same fill, and it
/// owes nothing — because the seller's half of the fee was a reduced credit and
/// not a later debit (§2.2). A vacant pre-fill replay would not test this at
/// all: there is no account there to refuse about.
#[tokio::test]
async fn a_settlement_with_nothing_owed_refuses_for_a_maker_who_never_owed_and_for_one_who_paid() {
    let mut pair = arrange().await;
    let (_, _, fee) = pair.collateral();
    submit_fill(&mut pair).await;
    let seller_replay = *pair
        .direct
        .chain
        .maker_replays
        .first()
        .expect("the seller's maker replay");
    assert_eq!(
        refused(
            submit_settlement(
                &mut pair,
                ArmV1 {
                    obligation_replay: Some(seller_replay),
                    ..ArmV1::default()
                },
            )
            .await,
            "a settlement against a maker who never owed",
        ),
        TradingSbfError::FeeNotOwed as u32,
    );

    executed(
        submit_settlement(&mut pair, ArmV1::default()).await,
        "the settlement this arm then replays",
    );
    let paid = token(&mut pair.context, fee).await.amount;

    pair.context
        .warp_to_slot(fixture_substrate().bank_slot() + 1)
        .expect("one block forward, so the replay is a new transaction");
    assert_eq!(
        refused(
            submit_settlement(&mut pair, ArmV1::default()).await,
            "a replayed settlement",
        ),
        TradingSbfError::FeeNotOwed as u32,
        "the obligation is gone, so the route refuses before Custody is reached",
    );
    assert_eq!(
        token(&mut pair.context, fee).await.amount,
        paid,
        "a replayed settlement moves nothing",
    );
}

/// The destination is pinned by OWNER, and that is the whole of E5's
/// vanished-recipient condition.
///
/// A recipient token account closed between the fill and its settlement must
/// not strand the fee: any account of that owner will do, and an idempotent
/// associated-token-account creation is permissionless. So the arm that must
/// EXECUTE is the one routing into a different account of the same owner. The
/// arm that must refuse is the one routing into a stranger's account — because
/// "any account will do" must not mean "any account".
#[tokio::test]
async fn a_foreign_account_of_the_same_recipient_is_admitted_and_a_strangers_is_not() {
    let mut pair = arrange().await;
    let (_, _, fee) = pair.collateral();
    submit_fill(&mut pair).await;

    assert_eq!(
        refused(
            submit_settlement(
                &mut pair,
                ArmV1 {
                    destination: Some(STRANGER_ACCOUNT),
                    ..ArmV1::default()
                },
            )
            .await,
            "a settlement routed to a stranger",
        ),
        TradingSbfError::FeeDestination as u32,
    );
    assert_eq!(
        fee_owed(&mut pair).await,
        ledger::COMBINED_FEE,
        "a refused settlement clears nothing",
    );

    executed(
        submit_settlement(
            &mut pair,
            ArmV1 {
                destination: Some(SPARE_RECIPIENT_ACCOUNT),
                ..ArmV1::default()
            },
        )
        .await,
        "a settlement into another account of the configured recipient",
    );
    assert_eq!(
        token(&mut pair.context, SPARE_RECIPIENT_ACCOUNT).await.amount,
        ledger::COMBINED_FEE,
        "the recipient is paid into the account the settlement named",
    );
    assert_eq!(
        token(&mut pair.context, fee).await.amount,
        ledger::FEE_BEFORE,
        "and not into the one the fill would have used",
    );
    assert_eq!(fee_owed(&mut pair).await, 0);
}

/// The fee comes out of an account the DEBTOR owns, and this is the hostile the
/// design's refusal table does not enumerate.
///
/// Custody checks `source.key == request.source` and the source's mint, and
/// never `semantic.source_owner`. Without the route's own pin, a settlement
/// could name any account whose standing delegation matched the debt — paying
/// one maker's obligation out of another maker's collateral, clearing the first
/// for free and consuming the allowance the second needs to settle with.
///
/// The seller's collateral account stands in for "somebody else's": it is on
/// the same mint, in the same market, and is not the debtor's.
#[tokio::test]
async fn a_settlement_out_of_somebody_elses_account_refuses() {
    let mut pair = arrange().await;
    let (_, seller, _) = pair.collateral();
    submit_fill(&mut pair).await;
    let before = token(&mut pair.context, seller).await.amount;

    assert_eq!(
        refused(
            submit_settlement(
                &mut pair,
                ArmV1 {
                    source: Some(seller),
                    ..ArmV1::default()
                },
            )
            .await,
            "a settlement out of the seller's account",
        ),
        TradingSbfError::FeeSource as u32,
    );
    assert_eq!(token(&mut pair.context, seller).await.amount, before);
    assert_eq!(fee_owed(&mut pair).await, ledger::COMBINED_FEE);
}

/// A settlement pointed at a maker replay that is not the one the wire names.
///
/// The seller's replay is a real Trading-owned PDA of the same market at the
/// same generation, so this is the narrowest possible substitution: everything
/// about the account is right except whose it is.
#[tokio::test]
async fn a_settlement_against_a_foreign_maker_replay_refuses() {
    let mut pair = arrange().await;
    submit_fill(&mut pair).await;
    let seller_replay = *pair
        .direct
        .chain
        .maker_replays
        .first()
        .expect("the seller's maker replay");

    assert_eq!(
        refused(
            submit_settlement(
                &mut pair,
                ArmV1 {
                    maker_replay: Some(seller_replay),
                    ..ArmV1::default()
                },
            )
            .await,
            "a settlement against the seller's maker replay",
        ),
        TradingSbfError::Content as u32,
        "the replay is authenticated against the coordinate the wire names",
    );
    assert_eq!(fee_owed(&mut pair).await, ledger::COMBINED_FEE);
}

/// The pair is unbatchable-free: the route's writability pin is one-directional.
///
/// This is the property `80b78181` and `16351a13` were each a fix for. It is
/// asserted structurally rather than by submitting a batched transaction,
/// because the two acts cannot share a transaction for a compute reason
/// (tx1 alone is ~1.34M CU) and a compute refusal would prove nothing about the
/// pin. What can be shown is that a coordinate this route only READS is
/// admitted when the caller marks it writable — which is exactly what a
/// batching caller's other instruction would do.
#[tokio::test]
async fn a_read_only_coordinate_marked_writable_is_still_admitted() {
    let mut pair = arrange().await;
    submit_fill(&mut pair).await;
    let mut instruction = settlement(&mut pair, ArmV1::default()).await;
    // The Direct root: read by this route, and written by the fill that any
    // batching caller would put in front of it.
    let root = pair.direct.chain.root;
    for meta in instruction.accounts.iter_mut() {
        if meta.pubkey == root {
            assert!(!meta.is_writable, "the root is a read coordinate here");
            meta.is_writable = true;
        }
    }
    executed(
        submit_one(&mut pair, instruction).await,
        "a settlement whose read-only root arrived writable",
    );
    assert_eq!(fee_owed(&mut pair).await, 0);
}

/// The Direct root is authenticated, and a root of the wrong width is not one.
///
/// Narrow on purpose: the root is where `config_id` comes from, and `config_id`
/// is the only thing standing between a caller and a fee recipient of their
/// choosing. `TradingSbfError::Root` names the account rather than folding into
/// the route's generic content refusal.
#[tokio::test]
async fn a_settlement_against_something_that_is_not_the_direct_root_refuses() {
    let mut pair = arrange().await;
    submit_fill(&mut pair).await;
    let mut instruction = settlement(&mut pair, ArmV1::default()).await;
    let root = pair.direct.chain.root;
    // The Claims market: a real account, a real PDA, and not this.
    let substitute = pair.direct.chain.claims_market;
    for meta in instruction.accounts.iter_mut() {
        if meta.pubkey == root {
            meta.pubkey = substitute;
        }
    }
    assert_eq!(
        refused(
            submit_one(&mut pair, instruction).await,
            "a settlement whose root is another account",
        ),
        TradingSbfError::Root as u32,
    );
    assert_eq!(fee_owed(&mut pair).await, ledger::COMBINED_FEE);
}
