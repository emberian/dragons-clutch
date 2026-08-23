//! Real-SBF evidence for the revenue-policy boundary (ADOPTED 2026-08-20
//! items 6/8, `docs/design/REVENUE_POLICY_V1.md` §3/§5/§8).
//!
//! What this drives on a real bank, in dependency order:
//!
//! 1. **Fee-bearing admission refuses without the record** — the record's
//!    absence IS the zero-take state (D4), with its own refusal code.
//! 2. **A fee-bearing epoch cannot smuggle or drop the account tail** —
//!    both count shapes refuse.
//! 3. **The record rides Realm creation** — `InitRealm` with the trailing
//!    pair pins the frozen const's digest, the structural treasury-UNSET
//!    sentinel, the embedded TerminalIdentityV1 header, and the mandatory
//!    funding-ledger sibling, byte-for-byte as the layout encoders write
//!    them.
//! 4. **Fee-bearing admission then refuses at the treasury byte** — the B4a
//!    deferral as a live refusal (`RevenueTreasuryUnset`), which fires on
//!    every fee-bearing admission until ember binds a key in a new const.
//! 5. **The close refuses while the Realm stands** (the TerminalClosure DAG
//!    extension), leaving every byte and lamport untouched.
//! 6. **No silent redirect**: re-running the creating instruction refuses
//!    (`AlreadyInitialized`) and the record bytes are bit-identical after
//!    every hostile attempt — there is no mutating instruction to test,
//!    which is the property itself.
//! 7. **The zero-fee plane is untouched**: the same epoch then opens under
//!    the frozen zero-fee const exactly as before.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.  Nothing here charges an
//! atom; both rates of the fee shape are zero and every `max_fee_atoms == 0`
//! gate stands.

use {
    clutch_batch_policy_identity::{
        batch_policy_digest, canonical_batch_policy_bytes,
        general_clearing_v1::{GENERAL_CLEARING_FEE_SHAPE_V1, GENERAL_CLEARING_POLICY_V1},
        revenue_policy_v1::{
            revenue_policy_digest, REVENUE_NEUTRAL_SINK_BYTES_V1, REVENUE_POLICY_V1,
            REVENUE_TREASURY_UNSET_V1,
        },
    },
    clutch_sbf::{
        error::ClutchError,
        instructions::artifact::CLOCK_SYSVAR_ID,
        instructions::genesis::{
            CLOSE_REVENUE_RECORD_ACCOUNT_COUNT, INIT_REALM_ACCOUNT_COUNT,
            INIT_REALM_REVENUE_ACCOUNT_COUNT,
        },
        instructions::orders_batch::general_epoch::{
            INIT_EPOCH_ACCOUNT_COUNT, INIT_EPOCH_FEE_ACCOUNT_COUNT,
        },
        instructions::orders_batch::terminal_closure::GENERAL_NEUTRAL_SINK_V1,
        seeds,
    },
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_outcome_id, canonical_realm_id,
        clearing::{
            GeneralFundingLedgerV1, FUNDING_COVERS_REVENUE_RECORD, GENERAL_FUNDING_LEDGER_BYTES,
        },
        collateral::ParentProfile,
        revenue::{RevenuePolicyRecordV1, REVENUE_POLICY_RECORD_BYTES},
        Hash32, Intent, MarketAccount, PriceGridAccount, TermsAccount, MAX_GRID_TICKS,
        MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_MAP_UNUSED,
    },
    clutch_svm_fixture::{
        compute_unit_limit_data, fixture_policy, fixture_terms, layout_request, COMPUTE_BUDGET,
        PROGRAM_ID, RENT_SYSVAR, SYSTEM_PROGRAM,
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_program_test::{tokio, ProgramTest, ProgramTestContext},
    solana_signer::Signer,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

const PRICE_SCALE: u64 = 10_000;
const EPOCH_INDEX: u64 = 3;
const FREEZE_DEADLINE: u64 = 500;
const CU_LIMIT: u32 = 1_400_000;
const REALM_NONCE: u64 = 9;
const OUTCOMES: u8 = 4;

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

fn clock_address() -> Address {
    Address::new_from_array(CLOCK_SYSVAR_ID.to_bytes())
}

fn sink_address() -> Address {
    // The frozen program-wide neutral sink, from the research crate's raw
    // byte pin — asserted equal to the canonical incinerator below.
    Address::new_from_array(REVENUE_NEUTRAL_SINK_BYTES_V1)
}

fn encode<F>(len: usize, writer: F) -> Vec<u8>
where
    F: FnOnce(&mut [u8]) -> Result<usize, clutch_solana_layout::CodecError>,
{
    let mut bytes = vec![0; len];
    assert_eq!(writer(&mut bytes).unwrap(), len);
    bytes
}

fn add_state(test: &mut ProgramTest, address: Address, data: Vec<u8>) {
    test.add_account(
        address,
        Account {
            lamports: rent_exempt(data.len()),
            data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

/// The general-plane terms shape (four outcomes), as `general_epoch.rs` builds
/// it.
fn general_terms(realm: Hash32, profile: Hash32, feed: Hash32) -> TermsAccount {
    let mut terms = fixture_terms(realm, profile, feed);
    let mut payouts = [clutch_solana_layout::PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    let mut payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < OUTCOMES as usize {
        let mut weights = [0; MAX_OUTCOMES];
        weights[outcome] = 1;
        payouts[outcome] = clutch_solana_layout::PayoutVectorBytes {
            denominator: 1,
            weights,
        };
        payout_map[outcome] = outcome as u8;
        outcome += 1;
    }
    terms.outcome_count = OUTCOMES;
    terms.payout_count = OUTCOMES;
    terms.payouts = payouts;
    terms.payout_map = payout_map;
    let mut knots = [0u128; clutch_solana_layout::MAX_KNOTS];
    let mut knot = 0usize;
    while knot < OUTCOMES as usize - 1 {
        knots[knot] = knot as u128 + 1;
        knot += 1;
    }
    terms.knot_count = OUTCOMES - 1;
    terms.knots = knots;
    terms.terms = terms.recomputed_terms_digest().unwrap();
    terms
}

struct Fixture {
    realm_id: Hash32,
    profile_id: Hash32,
    market: Hash32,
    zero_digest: Hash32,
    fee_digest: Hash32,
    collateral_policy: Address,
    realm_account: Address,
    record_account: Address,
    ledger_account: Address,
    market_account: Address,
    terms_account: Address,
    grid_account: Address,
    zero_policy_account: Address,
    fee_policy_account: Address,
    epoch_account: Address,
    window_account: Address,
    treasury_position: Address,
}

impl Fixture {
    fn init_realm(&self, payer: Address, with_revenue: bool) -> Instruction {
        let mut metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(self.realm_account, false),
            AccountMeta::new_readonly(self.collateral_policy, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        if with_revenue {
            metas.push(AccountMeta::new(self.record_account, false));
            metas.push(AccountMeta::new(self.ledger_account, false));
            assert_eq!(metas.len(), INIT_REALM_REVENUE_ACCOUNT_COUNT);
        } else {
            assert_eq!(metas.len(), INIT_REALM_ACCOUNT_COUNT);
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::InitRealm {
                    profile: self.profile_id,
                    realm_nonce: REALM_NONCE,
                    max_outcomes: MAX_OUTCOMES as u8,
                    profile_version: 2,
                },
            ),
            metas,
        )
    }

    fn init_epoch(&self, payer: Address, policy: Hash32, fee_tail: bool) -> Instruction {
        let policy_account = if policy == self.fee_digest {
            self.fee_policy_account
        } else {
            self.zero_policy_account
        };
        let mut metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(self.market_account, false),
            AccountMeta::new_readonly(self.terms_account, false),
            AccountMeta::new_readonly(self.grid_account, false),
            AccountMeta::new_readonly(policy_account, false),
            AccountMeta::new(self.epoch_account, false),
            AccountMeta::new(self.window_account, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(clock_address(), false),
        ];
        assert_eq!(metas.len(), INIT_EPOCH_ACCOUNT_COUNT);
        if fee_tail {
            metas.push(AccountMeta::new_readonly(self.record_account, false));
            metas.push(AccountMeta::new_readonly(self.treasury_position, false));
            assert_eq!(metas.len(), INIT_EPOCH_FEE_ACCOUNT_COUNT);
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::InitEpoch {
                    market: self.market,
                    epoch_index: EPOCH_INDEX,
                    policy,
                    freeze_deadline_slot: FREEZE_DEADLINE,
                },
            ),
            metas,
        )
    }

    fn close_record(&self, recipient: Address) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(self.realm_account, false),
            AccountMeta::new(self.record_account, false),
            AccountMeta::new(self.ledger_account, false),
            AccountMeta::new(recipient, false),
            AccountMeta::new(sink_address(), false),
        ];
        assert_eq!(metas.len(), CLOSE_REVENUE_RECORD_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::CloseRevenuePolicyRecord {
                    realm: self.realm_id,
                },
            ),
            metas,
        )
    }
}

async fn start() -> (ProgramTestContext, Fixture) {
    let feed = h(0x63);
    let market = h(0x35);

    // The Realm identity is honest: recomputed from the actual collateral
    // policy exactly as `InitRealm` recomputes it.
    let policy_value = fixture_policy([0x42; 32]);
    let policy_digest = policy_value.digest().unwrap();
    let profile_id = ParentProfile::from_policy(&policy_value)
        .and_then(|parent| parent.identity())
        .unwrap();
    let policy_body = policy_value.canonical_bytes().unwrap();
    let (collateral_policy, _) = pda(
        seeds::SEED_POLICY,
        &[&profile_id.bytes(), &policy_digest.bytes()],
    );
    let realm_id = canonical_realm_id(profile_id, REALM_NONCE);
    let (realm_account, _) = pda(seeds::SEED_REALM, &[&realm_id.bytes()]);
    let (record_account, _) = pda(seeds::SEED_REVENUE_POLICY, &[&realm_id.bytes()]);
    let (ledger_account, _) = pda(seeds::SEED_GENERAL_FUNDING, &[&record_account.to_bytes()]);

    let mut ticks = [0; MAX_GRID_TICKS];
    let mut tick = 0usize;
    while tick <= 10 {
        ticks[tick] = tick as u64 * 1_000;
        tick += 1;
    }
    let mut grid = PriceGridAccount {
        grid: Hash32::ZERO,
        realm: realm_id,
        price_scale: PRICE_SCALE,
        tick_count: 11,
        ticks,
        stored_bump: 0,
        flags: 0,
    };
    grid.grid = grid.recomputed_grid_id().unwrap();
    let (grid_address, grid_bump) = pda(seeds::SEED_GRID, &[&realm_id.bytes(), &grid.grid.bytes()]);
    grid.stored_bump = grid_bump;

    let mut terms = general_terms(realm_id, profile_id, feed);
    terms.price_grid = grid.grid;
    terms.terms = terms.recomputed_terms_digest().unwrap();
    let (terms_address, terms_bump) = pda(
        seeds::SEED_TERMS,
        &[&realm_id.bytes(), &terms.terms.bytes()],
    );
    terms.stored_bump = terms_bump;

    let (market_address, market_bump) =
        pda(seeds::SEED_MARKET, &[&realm_id.bytes(), &market.bytes()]);
    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < OUTCOMES as usize {
        outcomes[outcome] = canonical_outcome_id(market, outcome as u8);
        outcome += 1;
    }
    let market_state = MarketAccount {
        market,
        realm: realm_id,
        profile: profile_id,
        terms: terms.terms,
        outcome_count: OUTCOMES,
        lifecycle: 0,
        stored_bump: market_bump,
        hoard_bump: 0,
        outcomes,
        feed,
        collateral_cap: terms.collateral_cap,
        created_slot: 0,
        reserved: Hash32::ZERO,
    };

    let epoch_id = canonical_epoch_id(market, EPOCH_INDEX);
    let zero_digest =
        Hash32::from_bytes(batch_policy_digest(&GENERAL_CLEARING_POLICY_V1).unwrap().0);
    let fee_digest = Hash32::from_bytes(
        batch_policy_digest(&GENERAL_CLEARING_FEE_SHAPE_V1)
            .unwrap()
            .0,
    );
    let (zero_policy_address, _) = pda(
        seeds::SEED_BATCH_POLICY,
        &[&epoch_id.bytes(), &zero_digest.bytes()],
    );
    let (fee_policy_address, _) = pda(
        seeds::SEED_BATCH_POLICY,
        &[&epoch_id.bytes(), &fee_digest.bytes()],
    );
    let (epoch_address, _) = pda(
        seeds::SEED_EPOCH,
        &[&market.bytes(), &EPOCH_INDEX.to_le_bytes()],
    );
    let (window_address, _) = pda(
        seeds::SEED_EPOCH_WINDOW,
        &[&market.bytes(), &EPOCH_INDEX.to_le_bytes()],
    );
    // The canonical treasury Position address for the deferred sentinel:
    // never created — the admission walk must refuse before consulting it.
    let (treasury_position, _) = pda(
        seeds::SEED_POSITION,
        &[&market.bytes(), &REVENUE_TREASURY_UNSET_V1],
    );

    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("clutch_sbf", PROGRAM_ID, None);
    add_state(&mut test, collateral_policy, policy_body.to_vec());
    add_state(
        &mut test,
        market_address,
        encode(account_len::MARKET, |out| market_state.encode(out)),
    );
    add_state(
        &mut test,
        terms_address,
        encode(account_len::TERMS, |out| terms.encode(out)),
    );
    add_state(
        &mut test,
        grid_address,
        encode(account_len::PRICE_GRID, |out| grid.encode(out)),
    );
    add_state(
        &mut test,
        zero_policy_address,
        canonical_batch_policy_bytes(&GENERAL_CLEARING_POLICY_V1)
            .unwrap()
            .to_vec(),
    );
    add_state(
        &mut test,
        fee_policy_address,
        canonical_batch_policy_bytes(&GENERAL_CLEARING_FEE_SHAPE_V1)
            .unwrap()
            .to_vec(),
    );

    let fixture = Fixture {
        realm_id,
        profile_id,
        market,
        zero_digest,
        fee_digest,
        collateral_policy,
        realm_account,
        record_account,
        ledger_account,
        market_account: market_address,
        terms_account: terms_address,
        grid_account: grid_address,
        zero_policy_account: zero_policy_address,
        fee_policy_account: fee_policy_address,
        epoch_account: epoch_address,
        window_account: window_address,
        treasury_position,
    };
    (test.start_with_context().await, fixture)
}

async fn send(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    nonce: u32,
) -> Result<(), TransactionError> {
    let blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let budget = Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &compute_unit_limit_data(CU_LIMIT - nonce),
        Vec::new(),
    );
    let mut all = vec![budget];
    all.extend_from_slice(instructions);
    let transaction = Transaction::new_signed_with_payer(
        &all,
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .unwrap()
        .result
}

async fn account(context: &mut ProgramTestContext, address: Address) -> Option<Account> {
    context.banks_client.get_account(address).await.unwrap()
}

fn custom(result: Result<(), TransactionError>) -> u32 {
    match result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => code,
        other => panic!("expected custom refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn the_revenue_boundary_pins_the_record_and_refuses_fee_bearing_admission() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();

    // The research crate's raw incinerator byte pin is the canonical one.
    assert_eq!(
        REVENUE_NEUTRAL_SINK_BYTES_V1,
        GENERAL_NEUTRAL_SINK_V1.to_bytes(),
        "the revenue crate's neutral-sink bytes drifted from the incinerator"
    );

    // (1) Fee-bearing admission with NO record: refused with its own code —
    // the absence of the record is the zero-take state, permanently.
    let result = send(
        &mut context,
        &[fixture.init_epoch(payer, fixture.fee_digest, true)],
        0,
    )
    .await;
    assert_eq!(
        custom(result),
        ClutchError::RevenuePolicyRecordMissing as u32,
        "record absence must refuse fee-bearing admission"
    );

    // (2) A fee-bearing epoch cannot drop its account tail.
    let result = send(
        &mut context,
        &[fixture.init_epoch(payer, fixture.fee_digest, false)],
        1,
    )
    .await;
    assert_eq!(custom(result), ClutchError::AccountCount as u32);

    // (3) The record rides Realm creation, byte-for-byte.
    send(&mut context, &[fixture.init_realm(payer, true)], 2)
        .await
        .unwrap();
    let record_bytes = account(&mut context, fixture.record_account)
        .await
        .expect("the record exists")
        .data;
    let record = RevenuePolicyRecordV1::decode(&record_bytes).unwrap();
    let expected_digest = revenue_policy_digest(&REVENUE_POLICY_V1).unwrap();
    assert_eq!(record.realm, fixture.realm_id);
    assert_eq!(record.policy_digest.bytes(), expected_digest.0);
    assert_eq!(record.treasury.bytes(), REVENUE_TREASURY_UNSET_V1);
    assert_eq!(record.terminal_payer.bytes(), payer.to_bytes());
    assert_eq!(
        record.terminal_payer_principal,
        rent_exempt(REVENUE_POLICY_RECORD_BYTES),
        "full-principal creation: the payer's exact recorded outlay"
    );
    assert_eq!(record.terminal_donation_floor, 0);
    assert_eq!(record.terminal_generation, 1);
    let ledger_bytes = account(&mut context, fixture.ledger_account)
        .await
        .expect("the funding ledger exists")
        .data;
    let ledger = GeneralFundingLedgerV1::decode(&ledger_bytes).unwrap();
    assert_eq!(
        ledger.target.bytes(),
        fixture.record_account.to_bytes(),
        "the ledger covers the record"
    );
    assert_eq!(ledger.payer.bytes(), payer.to_bytes());
    assert_eq!(ledger.covered, FUNDING_COVERS_REVENUE_RECORD);
    assert_eq!(
        ledger.payer_principal_lamports,
        rent_exempt(REVENUE_POLICY_RECORD_BYTES) + rent_exempt(GENERAL_FUNDING_LEDGER_BYTES),
        "the group principal covers the record and the ledger itself"
    );
    assert_eq!(ledger.donation_floor_lamports, 0);

    // (4) The B4a deferral, live: with the record present the fee-bearing
    // walk reaches the treasury byte and refuses there.  This refusal fires
    // on EVERY fee-bearing admission until a const names a real key.
    let result = send(
        &mut context,
        &[fixture.init_epoch(payer, fixture.fee_digest, true)],
        3,
    )
    .await;
    assert_eq!(
        custom(result),
        ClutchError::RevenueTreasuryUnset as u32,
        "the structural treasury deferral must refuse fee-bearing admission"
    );

    // (5) A zero-fee epoch cannot smuggle the fee account tail.
    let result = send(
        &mut context,
        &[fixture.init_epoch(payer, fixture.zero_digest, true)],
        4,
    )
    .await;
    assert_eq!(custom(result), ClutchError::AccountCount as u32);

    // (6) The close refuses while the Realm stands — the TerminalClosure
    // refusal-set extension — and moves nothing.
    let payer_before = account(&mut context, payer).await.unwrap().lamports;
    let result = send(&mut context, &[fixture.close_record(payer)], 5).await;
    assert_eq!(
        custom(result),
        ClutchError::MismatchedState as u32,
        "a standing Realm must hold its record's close"
    );
    let record_after = account(&mut context, fixture.record_account).await.unwrap();
    assert_eq!(
        record_after.data, record_bytes,
        "the refused close moved a byte"
    );
    assert_eq!(
        account(&mut context, fixture.ledger_account)
            .await
            .unwrap()
            .data,
        ledger_bytes
    );
    let payer_after = account(&mut context, payer).await.unwrap().lamports;
    // The refused transaction still pays its signature fee; nothing else.
    assert!(payer_before - payer_after <= 10_000);

    // (7) No silent redirect: the only instruction that can ever write a
    // record refuses on the existing Realm, and the bytes stand.
    let result = send(&mut context, &[fixture.init_realm(payer, true)], 6).await;
    assert_eq!(custom(result), ClutchError::AlreadyInitialized as u32);
    assert_eq!(
        account(&mut context, fixture.record_account)
            .await
            .unwrap()
            .data,
        record_bytes,
        "a hostile re-creation attempt touched the record"
    );

    // (8) Control: the zero-fee plane is untouched — the same epoch opens
    // under the frozen zero-fee const with the unchanged ten-account list.
    send(
        &mut context,
        &[fixture.init_epoch(payer, fixture.zero_digest, false)],
        7,
    )
    .await
    .unwrap();
    assert!(account(&mut context, fixture.epoch_account).await.is_some());
}
