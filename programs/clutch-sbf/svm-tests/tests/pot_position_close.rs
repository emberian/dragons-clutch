//! Real-SBF evidence for `Intent::ClosePosition` (tag 69) — the owner-signed
//! end of the position lifecycle, and the byte host of the revenue plane's
//! B4b mid-epoch-close grief rider.
//!
//! The gates:
//! * **economic zero first** — a Position holding trading cash, encumbered
//!   cash, or any claim refuses, with byte-exact rollback;
//! * **the grief rider, executed** — a Position the Realm's revenue-policy
//!   record names as its treasury refuses close with
//!   `TreasuryServiceOutstanding`, decided by
//!   `clutch_liveness::TreasuryServiceLedger`; a record naming somebody else
//!   leaves the same Position closable, so the refusal is the rider and not a
//!   blanket "a record exists" gate;
//! * **absence is the zero-take state** — with no record at the Realm's
//!   canonical address the close proceeds, and a record presented at the
//!   *wrong* address refuses at `WrongPda` rather than being trusted;
//! * **the lamport disposition** — the Position family has no creation-side
//!   funding ledger, so no principal is recorded and the whole live balance,
//!   injected donation included, burns at the frozen neutral sink; the close
//!   is measured to the lamport;
//! * **authority** — a stranger's signature refuses, and the closed Position's
//!   own disappearance is the replay guard.
//!
//! The fixture is deliberately flat: a Market, some Positions, and optionally
//! a revenue record, all written with the frozen codecs at their canonical
//! addresses.  Nothing here needs an epoch, a book, or a candidate — a
//! Position outlives every epoch, which is exactly why its close carries no
//! epoch coordinate.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.  The reference adapter
//! refuses the tag with `UnsupportedIntent`; the oracle is the layout codec
//! plus lamport conservation on this real bank.

use {
    clutch_batch_policy_identity::revenue_policy_v1::REVENUE_TREASURY_UNSET_V1,
    clutch_sbf::{
        error::ClutchError,
        instructions::orders_batch::terminal_closure::{
            CLOSE_POSITION_ACCOUNT_COUNT, GENERAL_NEUTRAL_SINK_V1,
        },
        seeds,
    },
    clutch_solana_layout::{
        account_len, canonical_outcome_id,
        revenue::{RevenuePolicyRecordV1, REVENUE_POLICY_RECORD_BYTES},
        Hash32, MarketAccount, PositionAccount, MAX_OUTCOMES,
    },
    clutch_svm_fixture::{compute_unit_limit_data, layout_request, COMPUTE_BUDGET, PROGRAM_ID},
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_test::{tokio, ProgramTest, ProgramTestContext},
    solana_signer::Signer,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

const OUTCOMES: u8 = 4;
const CU_LIMIT: u32 = 400_000;
const WALLET: u64 = 5_000_000_000;
/// A public prefund on the Position, above its rent minimum.  Nothing records
/// a payer for it, so the close must burn it rather than invent an owner.
const DONATION: u64 = 77_777;

fn h(byte: u8) -> Hash32 {
    Hash32::from_bytes([byte; 32])
}

fn pda(prefix: &[u8], suffixes: &[&[u8]]) -> (Address, u8) {
    let mut all = Vec::with_capacity(1 + suffixes.len());
    all.push(prefix);
    all.extend_from_slice(suffixes);
    Address::find_program_address(&all, &PROGRAM_ID)
}

fn rent_exempt(len: usize) -> u64 {
    solana_rent::Rent::default().minimum_balance(len).max(1)
}

fn sink_address() -> Address {
    Address::new_from_array(GENERAL_NEUTRAL_SINK_V1.to_bytes())
}

fn program_account(data: Vec<u8>, extra: u64) -> Account {
    Account {
        lamports: rent_exempt(data.len()) + extra,
        data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn encode<F>(len: usize, writer: F) -> Vec<u8>
where
    F: FnOnce(&mut [u8]) -> Result<usize, clutch_solana_layout::CodecError>,
{
    let mut bytes = vec![0; len];
    assert_eq!(writer(&mut bytes).unwrap(), len);
    bytes
}

/// Which Realm revenue record, if any, the bank starts with.
#[derive(Clone, Copy)]
enum Record {
    /// No record at the Realm's canonical address: the zero-take state (D4).
    Absent,
    /// A record naming the closing owner as the Realm's treasury.
    NamesTheCloser,
    /// A record naming some other identity as the treasury.
    NamesAnother,
    /// A record carrying the structural UNSET sentinel — every record a V1
    /// Realm can actually hold.
    Unset,
}

struct Fixture {
    realm: Hash32,
    market: Hash32,
    market_account: Address,
    owner: Keypair,
    position: Address,
    stranger: Keypair,
    stranger_position: Address,
    record_account: Address,
}

impl Fixture {
    fn close(&self, position: Address, owner: Hash32, record: Address) -> Instruction {
        let metas = vec![
            AccountMeta::new(Address::new_from_array(owner.bytes()), true),
            AccountMeta::new_readonly(self.market_account, false),
            AccountMeta::new(position, false),
            AccountMeta::new_readonly(record, false),
            AccountMeta::new(sink_address(), false),
        ];
        assert_eq!(metas.len(), CLOSE_POSITION_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::ClosePosition {
                    market: self.market,
                    owner,
                },
            ),
            metas,
        )
    }

    fn close_own(&self) -> Instruction {
        self.close(
            self.position,
            Hash32::from_bytes(self.owner.pubkey().to_bytes()),
            self.record_account,
        )
    }
}

fn position_bytes(market: Hash32, owner: Hash32, bump: u8, cash: u64, reserved: u64) -> Vec<u8> {
    encode(account_len::POSITION, |out| {
        PositionAccount {
            market,
            owner,
            generation: 0,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: cash,
            reserved_cash_atoms: reserved,
            stored_bump: bump,
            close_state: 0,
        }
        .encode(out)
    })
}

async fn start(record: Record) -> (ProgramTestContext, Fixture) {
    let realm = h(0x61);
    let profile = h(0x62);
    let feed = h(0x63);
    let market = h(0x3c);
    let owner = Keypair::new();
    let stranger = Keypair::new();
    let owner_id = Hash32::from_bytes(owner.pubkey().to_bytes());
    let stranger_id = Hash32::from_bytes(stranger.pubkey().to_bytes());

    let (market_account, market_bump) = pda(seeds::SEED_MARKET, &[&realm.bytes(), &market.bytes()]);
    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < OUTCOMES as usize {
        outcomes[outcome] = canonical_outcome_id(market, outcome as u8);
        outcome += 1;
    }
    let market_state = MarketAccount {
        market,
        realm,
        profile,
        terms: h(0x51),
        outcome_count: OUTCOMES,
        lifecycle: 0,
        stored_bump: market_bump,
        hoard_bump: 0,
        outcomes,
        feed: clutch_solana_layout::FeedId::from_bytes(feed.bytes()),
        collateral_cap: u64::MAX,
        created_slot: 0,
        reserved: Hash32::ZERO,
    };

    let (position, position_bump) =
        pda(seeds::SEED_POSITION, &[&market.bytes(), &owner_id.bytes()]);
    let (stranger_position, stranger_bump) = pda(
        seeds::SEED_POSITION,
        &[&market.bytes(), &stranger_id.bytes()],
    );
    let (record_account, record_bump) = pda(seeds::SEED_REVENUE_POLICY, &[&realm.bytes()]);

    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("clutch_sbf", PROGRAM_ID, None);
    for keypair in [&owner, &stranger] {
        test.add_account(
            keypair.pubkey(),
            Account {
                lamports: WALLET,
                data: Vec::new(),
                owner: clutch_svm_fixture::SYSTEM_PROGRAM,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    test.add_account(
        market_account,
        program_account(
            encode(account_len::MARKET, |out| market_state.encode(out)),
            0,
        ),
    );
    test.add_account(
        position,
        program_account(
            position_bytes(market, owner_id, position_bump, 0, 0),
            DONATION,
        ),
    );
    test.add_account(
        stranger_position,
        program_account(position_bytes(market, stranger_id, stranger_bump, 0, 0), 0),
    );
    if let Some(treasury) = match record {
        Record::Absent => None,
        Record::NamesTheCloser => Some(owner_id),
        Record::NamesAnother => Some(stranger_id),
        Record::Unset => Some(Hash32::from_bytes(REVENUE_TREASURY_UNSET_V1)),
    } {
        let value = RevenuePolicyRecordV1 {
            realm,
            policy_digest: h(0x71),
            treasury,
            terminal_payer: h(0x72),
            terminal_payer_principal: 1_976_640,
            terminal_donation_floor: 0,
            terminal_generation: 1,
            stored_bump: record_bump,
            flags: 0,
        };
        test.add_account(
            record_account,
            program_account(
                encode(REVENUE_POLICY_RECORD_BYTES, |out| value.encode(out)),
                0,
            ),
        );
    }

    let context = test.start_with_context().await;
    (
        context,
        Fixture {
            realm,
            market,
            market_account,
            owner,
            position,
            stranger,
            stranger_position,
            record_account,
        },
    )
}

async fn send(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    signer: &Keypair,
    nonce: u32,
) -> Result<(), TransactionError> {
    let budget = Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &compute_unit_limit_data(CU_LIMIT - nonce),
        vec![],
    );
    let blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let transaction = Transaction::new_signed_with_payer(
        &[budget, instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer, signer],
        blockhash,
    );
    context
        .banks_client
        .process_transaction(transaction)
        .await
        .map_err(|error| match error {
            solana_program_test::BanksClientError::TransactionError(inner) => inner,
            other => panic!("unexpected bank error: {other:?}"),
        })
}

async fn account(context: &mut ProgramTestContext, address: Address) -> Option<Account> {
    context.banks_client.get_account(address).await.unwrap()
}

async fn lamports(context: &mut ProgramTestContext, address: Address) -> u64 {
    account(context, address).await.map_or(0, |a| a.lamports)
}

fn custom(result: Result<(), TransactionError>) -> u32 {
    match result.unwrap_err() {
        TransactionError::InstructionError(_, error) => match error {
            solana_instruction::error::InstructionError::Custom(code) => code,
            other => panic!("unexpected instruction error: {other:?}"),
        },
        other => panic!("unexpected transaction error: {other:?}"),
    }
}

/// The zero-take Realm: no record, an empty Position, and every lamport —
/// rent minimum and injected donation alike — burned at the frozen sink.
#[tokio::test]
async fn an_empty_position_closes_and_burns_its_whole_balance() {
    let (mut context, fixture) = start(Record::Absent).await;
    let before = lamports(&mut context, fixture.position).await;
    assert_eq!(before, rent_exempt(account_len::POSITION) + DONATION);
    let sink_before = lamports(&mut context, sink_address()).await;

    send(&mut context, fixture.close_own(), &fixture.owner, 1)
        .await
        .unwrap();

    // The account is gone and no lamport was invented or lost: the Position
    // family records no payer, so the whole balance is "every other live
    // lamport" and it burns.
    assert!(account(&mut context, fixture.position).await.is_none());
    assert_eq!(
        lamports(&mut context, sink_address()).await,
        sink_before + before
    );
    // Its own disappearance is the replay guard.
    let replay = send(&mut context, fixture.close_own(), &fixture.owner, 2).await;
    assert!(replay.is_err());
}

/// Economic zero first, in each of the three ways a Position can hold value.
#[tokio::test]
async fn a_position_holding_any_value_refuses_with_full_rollback() {
    for (label, cash, reserved, eggs) in [
        ("free cash", 5u64, 0u64, 0u64),
        ("encumbered cash", 5, 5, 0),
        ("claims", 0, 0, 3),
    ] {
        let (mut context, fixture) = start(Record::Absent).await;
        let owner_id = Hash32::from_bytes(fixture.owner.pubkey().to_bytes());
        let mut internal = [0u64; MAX_OUTCOMES];
        internal[0] = eggs;
        let held = encode(account_len::POSITION, |out| {
            PositionAccount {
                market: fixture.market,
                owner: owner_id,
                generation: 0,
                internal,
                cash_atoms: cash,
                reserved_cash_atoms: reserved,
                stored_bump: pda(
                    seeds::SEED_POSITION,
                    &[&fixture.market.bytes(), &owner_id.bytes()],
                )
                .1,
                close_state: 0,
            }
            .encode(out)
        });
        context.set_account(&fixture.position, &program_account(held.clone(), 0).into());
        let sink_before = lamports(&mut context, sink_address()).await;

        let refused = send(&mut context, fixture.close_own(), &fixture.owner, 3).await;
        assert_eq!(
            custom(refused),
            ClutchError::AggregateClosureMismatch as u32,
            "{label}"
        );
        assert_eq!(
            account(&mut context, fixture.position).await.unwrap().data,
            held,
            "{label}"
        );
        assert_eq!(
            lamports(&mut context, sink_address()).await,
            sink_before,
            "{label}"
        );
    }
}

/// The grief rider, executed: the treasury Position of a Realm whose
/// fee-bearing epochs it serves cannot be closed out from under them.
///
/// The kernel decides it.  `TreasuryServiceLedger::begin_service` counts the
/// Realm's standing election as one outstanding service and `close` refuses
/// while any is outstanding; `settle_service` has no caller because
/// fee-bearing admission refuses at `RevenueTreasuryUnset`, so the count
/// cannot come back down and this refusal is the boundary itself.
#[tokio::test]
async fn a_named_treasury_position_refuses_close_while_it_serves() {
    let (mut context, fixture) = start(Record::NamesTheCloser).await;
    let before = account(&mut context, fixture.position).await.unwrap();
    let sink_before = lamports(&mut context, sink_address()).await;

    let refused = send(&mut context, fixture.close_own(), &fixture.owner, 4).await;
    assert_eq!(
        custom(refused),
        ClutchError::TreasuryServiceOutstanding as u32
    );
    let after = account(&mut context, fixture.position).await.unwrap();
    assert_eq!(after.data, before.data);
    assert_eq!(after.lamports, before.lamports);
    assert_eq!(lamports(&mut context, sink_address()).await, sink_before);
}

/// ...and the rider is the rider, not a blanket "a record exists" gate: the
/// same Position closes when the record names somebody else, and when it
/// carries the structural UNSET sentinel every V1 Realm actually holds.
#[tokio::test]
async fn a_record_naming_another_treasury_leaves_the_position_closable() {
    for record in [Record::NamesAnother, Record::Unset] {
        let (mut context, fixture) = start(record).await;
        send(&mut context, fixture.close_own(), &fixture.owner, 5)
            .await
            .unwrap();
        assert!(account(&mut context, fixture.position).await.is_none());
        // The record itself is untouched: this close reads it, never writes it.
        assert!(account(&mut context, fixture.record_account)
            .await
            .is_some());
    }
}

/// A substituted record address refuses rather than being trusted, and a
/// stranger cannot close somebody else's Position.
#[tokio::test]
async fn a_substituted_record_and_a_stranger_signature_refuse() {
    let (mut context, fixture) = start(Record::NamesTheCloser).await;
    let owner_id = Hash32::from_bytes(fixture.owner.pubkey().to_bytes());

    // The rider cannot be dodged by presenting a *different* Realm's record
    // address: the address is re-derived from the Market's own realm.
    let elsewhere = pda(seeds::SEED_REVENUE_POLICY, &[&h(0x99).bytes()]).0;
    let refused = send(
        &mut context,
        fixture.close(fixture.position, owner_id, elsewhere),
        &fixture.owner,
        6,
    )
    .await;
    assert_eq!(custom(refused), ClutchError::WrongPda as u32);
    assert!(account(&mut context, fixture.position).await.is_some());

    // A stranger signing for the owner's Position refuses: the signer *is*
    // the owner identity, and the intent's owner must be that signer.
    let refused = send(
        &mut context,
        fixture.close(fixture.position, owner_id, fixture.record_account),
        &fixture.stranger,
        7,
    )
    .await;
    assert!(refused.is_err());

    // A stranger closing their *own* Position while naming the owner's
    // Position refuses at the address binding.
    let stranger_id = Hash32::from_bytes(fixture.stranger.pubkey().to_bytes());
    let refused = send(
        &mut context,
        fixture.close(fixture.position, stranger_id, fixture.record_account),
        &fixture.stranger,
        8,
    )
    .await;
    assert_eq!(custom(refused), ClutchError::WrongPda as u32);
    assert!(account(&mut context, fixture.stranger_position)
        .await
        .is_some());
    let _ = fixture.realm;
}
