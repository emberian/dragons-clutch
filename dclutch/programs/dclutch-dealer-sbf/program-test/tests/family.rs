//! Real-ELF refusal campaign for the canonical Dealer family route.
//!
//! Every transaction below is submitted to `solana-program-test` against the
//! real `dclutch_dealer_sbf.so`, with the real `dclutch_registry_sbf.so`,
//! `dclutch_core_sbf.so` and `dclutch_custody_sbf.so` installed as genuine
//! executables. Nothing here is a fixture shell: the bytes that refuse are the
//! bytes the loader would run.
//!
//! The campaign drives `dealer/process_dealer_family_instruction` and names,
//! for each transaction, the exact `DealerSbfError` the chain must report. A
//! refusal that arrives for a different reason than the case is testing is a
//! failure of the case, not a pass, so every assertion checks the numeric code
//! the runtime logged and not merely that something went wrong.
//!
//! # What this campaign is evidence of, and what it is not
//!
//! It is real-ELF execution evidence for the Dealer family route's
//! authentication prefix: instruction shape, account frame, account identity,
//! actor identity, Clock binding, and the Registry reauthentication CPI. It is
//! NOT evidence that any Dealer action commits, because no accepted transition
//! is reachable without a complete activated release set, a Core Market in an
//! admitting phase, and Claims/Custody prestate. Those are recorded as blocked
//! rather than asserted here.
//!
//! `solana-program-test` submits no packet, so this campaign cannot observe a
//! frame that exceeds the legacy 1,232-byte transaction limit; the Dealer
//! common frame is 23 accounts and is nowhere near it, but the exemption is
//! stated rather than assumed (`tools/gauntlet/TIERS.md`, the ProgramTest
//! fast-lane bar).

#![expect(
    clippy::indexing_slicing,
    reason = "fixed-width fixture arrays with statically known extents"
)]

use std::{
    env, fs,
    sync::atomic::{AtomicUsize, Ordering},
    vec::Vec,
};

use dclutch_dealer_codec::{
    Action, CANDIDATE_BYTES, CandidateInput, CurveBand, CurveInput, MAX_OUTCOMES, Phase, Policy,
    REQUEST_BYTES, Request, Side, State, encode_candidate,
};
use dclutch_program_test_evidence::{TransactionEvidence, record};
use solana_account::Account;
use solana_program::{
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::{instruction::InstructionError, signature::Signer, transaction::TransactionError};
use solana_sdk_ids::{bpf_loader_upgradeable, sysvar};
use solana_transaction::Transaction;

/// Census program label for the Dealer family artifact.
const DEALER_LABEL: &str = "dealer";
/// Census program label for the Registry artifact this campaign CPIs into.
const REGISTRY_LABEL: &str = "registry";

/// The Dealer family handler owns the canonical Trading controller identity,
/// so the program under test is installed at the Trading program address and
/// is passed at the fixed Trading-program slot (`programs/dclutch-dealer-sbf/
/// src/lib.rs`, `validate_prefix`: `accounts[TRADING_PROGRAM].key != program_id`
/// is a refusal).
const DEALER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd0; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd1; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd2; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd3; 32]);

const CORE_MARKET: Pubkey = Pubkey::new_from_array([0x6d; 32]);
const ACTIVATION_CACHE: Pubkey = Pubkey::new_from_array([0xa0; 32]);
const REALM: Pubkey = Pubkey::new_from_array([0xa1; 32]);
const COLLATERAL_MINT: Pubkey = Pubkey::new_from_array([0xa2; 32]);
const CUSTODY_AUTHORITY: Pubkey = Pubkey::new_from_array([0xa3; 32]);
const DEALER_QUOTE: Pubkey = Pubkey::new_from_array([0xa4; 32]);
const FEE_VAULT: Pubkey = Pubkey::new_from_array([0xa5; 32]);
const LIVENESS_VAULT: Pubkey = Pubkey::new_from_array([0xa6; 32]);
const FOREIGN_STATE: Pubkey = Pubkey::new_from_array([0xa7; 32]);
const NON_SIGNING_ACTOR: Pubkey = Pubkey::new_from_array([0xa8; 32]);

/// SPL Token, whose executable genesis account `solana-program-test` seeds.
const LEGACY_TOKEN_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
]);

/// PDA domains, quoted from the protocol facts they are:
/// `programs/dclutch-dealer-sbf/src/lib.rs` `POLICY_PDA_DOMAIN_V1`,
/// `CANDIDATE_PDA_DOMAIN_V1`, `STATE_PDA_DOMAIN_V1`. They are stated here
/// rather than imported so this campaign derives the address a client would
/// derive, not the address the program under test believes in.
const POLICY_PDA_DOMAIN_V1: &[u8] = b"dclutch:dealer-policy:v1";
const CANDIDATE_PDA_DOMAIN_V1: &[u8] = b"dclutch:dealer-candidate:v1";
const STATE_PDA_DOMAIN_V1: &[u8] = b"dclutch:dealer-state:v1";

/// Exact common account count before any plan-derived Claims/Custody resource
/// (`COMMON_ACCOUNT_COUNT_V1`).
const COMMON_ACCOUNT_COUNT_V1: usize = 23;

/// Wire offsets inside one canonical Dealer request.
///
/// Lean-emitted: `crates/dclutch-dealer-codec/src/generated_dealer_liquidity.rs`
/// `REQUEST_ACTION_OFFSET` / `REQUEST_NOW_OFFSET`, which that crate's
/// `generator_fresh` test holds byte-identical to
/// `formal/dclutch-semantics/EmitDealerLiquidityAbiRust.lean`.
const REQUEST_ACTION_OFFSET: usize = 10;
const REQUEST_NOW_OFFSET: usize = 24;

/// Stable Dealer SBF refusal codes (`DealerSbfError`, `#[repr(u32)]`).
const REFUSAL_INSTRUCTION: u32 = 0;
const REFUSAL_ACCOUNT_FRAME: u32 = 1;
const REFUSAL_ACCOUNT_IDENTITY: u32 = 2;
const REFUSAL_SIGNATURE: u32 = 3;
const REFUSAL_CLOCK: u32 = 4;
const REFUSAL_RELEASE: u32 = 5;

/// `MAX_COMPUTE_UNIT_LIMIT`. A measurement may not raise it and a gate may not
/// lower it.
const COMPUTE_LIMIT: u64 = 1_400_000;

const OUTCOMES: usize = 3;
const CANDIDATE_ID: [u8; 32] = [0x63; 32];
const RELEASE_SET_ID: [u8; 32] = [0x72; 32];

/// Load one real first-party artifact from the directory the gauntlet fills.
fn elf(name: &str) -> Vec<u8> {
    let directory = env::var("SBF_OUT_DIR").expect(
        "SBF_OUT_DIR is required: this campaign is real-ELF evidence and refuses to run without \
         the artifacts under test",
    );
    fs::read(format!("{directory}/{name}.so")).expect("first-party SBF artifact")
}

/// Loader-v3 ProgramData for one immutable deployment.
///
/// `UpgradeableLoaderState::ProgramData` serializes as a 4-byte enum variant
/// (`3`), an 8-byte deployment slot, and a 33-byte `Option<Pubkey>` upgrade
/// authority whose zero discriminant is the encoding of an authority that has
/// been surrendered. That is 45 bytes, and the ELF begins at exactly 45 - the
/// tail is not appended after 13.
fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::from([0_u8; 45]);
    bytes
        .get_mut(..4)
        .expect("loader variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    bytes
        .get_mut(4..12)
        .expect("deployment slot")
        .copy_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(elf);
    bytes
}

fn programdata_address(program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn add_program(test: &mut ProgramTest, name: &'static str, program: Pubkey, artifact: &[u8]) {
    test.add_upgradeable_program_to_genesis(name, &program);
    let bytes = immutable_programdata(artifact);
    test.add_account(
        programdata_address(&program),
        Account {
            lamports: Rent::default().minimum_balance(bytes.len()).max(1),
            data: bytes,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn account(owner: Pubkey, data: Vec<u8>) -> Account {
    Account {
        lamports: Rent::default().minimum_balance(data.len()).max(1),
        data,
        owner,
        executable: false,
        rent_epoch: 0,
    }
}

// ---------------------------------------------------------------- the prestate

fn policy() -> Policy {
    Policy {
        market_id: CORE_MARKET.to_bytes(),
        release_set_id: RELEASE_SET_ID,
        dealer_id: [0x81; 32],
        fee_recipient_id: [0x82; 32],
        unwind_recipient_id: [0x83; 32],
        outcome_count: 3,
        quote_scale: 100,
        fee_numerator: 1,
        fee_denominator: 100,
        minimum_work_funding: 50,
        replacement_delay: 5,
    }
}

fn candidate_bytes(candidate_id: [u8; 32]) -> [u8; CANDIDATE_BYTES] {
    let bids = [CurveBand {
        capacity: 100,
        price_numerator: 40,
    }];
    let asks = [CurveBand {
        capacity: 100,
        price_numerator: 60,
    }];
    let curves = [CurveInput {
        bids: &bids,
        asks: &asks,
    }; OUTCOMES];
    let mut output = [0_u8; CANDIDATE_BYTES];
    encode_candidate(
        &mut output,
        CandidateInput {
            candidate_id,
            revision: 1,
            valid_from: 0,
            expires_at: 1_000_000,
            quote_reserve_floor: 100,
            work_funding: 100,
            work_reward: 2,
            minimum_inventory: &[0, 0, 0],
            maximum_inventory: &[100, 100, 100],
            curves: &curves,
        },
    )
    .expect("canonical Candidate");
    output
}

fn dealer_state() -> State {
    let mut inventory = [0; MAX_OUTCOMES];
    inventory[..OUTCOMES].copy_from_slice(&[50, 50, 50]);
    State {
        phase: Phase::Open,
        outcome_count: 3,
        winner: 0,
        active_candidate_id: CANDIDATE_ID,
        pending_candidate_id: [0; 32],
        release_set_id: RELEASE_SET_ID,
        active_revision: 1,
        pending_revision: 0,
        state_revision: 1,
        inventory,
        buy_used: [0; MAX_OUTCOMES],
        sell_used: [0; MAX_OUTCOMES],
        buy_quote_paid: [0; MAX_OUTCOMES],
        sell_quote_paid: [0; MAX_OUTCOMES],
        fee_base: 0,
        fee_paid: 0,
        quote_custody: 1_000,
        fee_custody: 0,
        liveness_custody: 100,
        active_work_remaining: 100,
        pending_work_funding: 0,
    }
}

struct Addresses {
    policy: Pubkey,
    candidate: Pubkey,
    misbound_candidate: Pubkey,
    state: Pubkey,
    substituted_policy: Pubkey,
}

fn addresses() -> Addresses {
    let market = CORE_MARKET.to_bytes();
    let other_market = [0x6e; 32];
    Addresses {
        policy: Pubkey::find_program_address(
            &[POLICY_PDA_DOMAIN_V1, market.as_slice()],
            &DEALER_PROGRAM_ID,
        )
        .0,
        candidate: Pubkey::find_program_address(
            &[CANDIDATE_PDA_DOMAIN_V1, CANDIDATE_ID.as_slice()],
            &DEALER_PROGRAM_ID,
        )
        .0,
        // The address a DIFFERENT Candidate identity would own. Installing the
        // canonical Candidate body here is the substitution the PDA join exists
        // to refuse.
        misbound_candidate: Pubkey::find_program_address(
            &[CANDIDATE_PDA_DOMAIN_V1, [0x64_u8; 32].as_slice()],
            &DEALER_PROGRAM_ID,
        )
        .0,
        state: Pubkey::find_program_address(
            &[STATE_PDA_DOMAIN_V1, market.as_slice()],
            &DEALER_PROGRAM_ID,
        )
        .0,
        substituted_policy: Pubkey::find_program_address(
            &[POLICY_PDA_DOMAIN_V1, other_market.as_slice()],
            &DEALER_PROGRAM_ID,
        )
        .0,
    }
}

fn program_test(keys: &Addresses) -> ProgramTest {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    // The protocol ceiling, not a budget this campaign may move. The heap is
    // the SBF default 32,768 and is likewise never lifted: a Dealer common
    // frame that only fits a diagnostic budget has not executed.
    test.set_compute_max_units(COMPUTE_LIMIT);
    add_program(
        &mut test,
        "dclutch_dealer_sbf",
        DEALER_PROGRAM_ID,
        &elf("dclutch_dealer_sbf"),
    );
    add_program(
        &mut test,
        "dclutch_registry_sbf",
        REGISTRY_PROGRAM_ID,
        &elf("dclutch_registry_sbf"),
    );
    add_program(
        &mut test,
        "dclutch_core_sbf",
        CORE_PROGRAM_ID,
        &elf("dclutch_core_sbf"),
    );
    add_program(
        &mut test,
        "dclutch_custody_sbf",
        CUSTODY_PROGRAM_ID,
        &elf("dclutch_custody_sbf"),
    );

    let policy_bytes = policy().to_bytes().expect("canonical Policy").to_vec();
    test.add_account(
        keys.policy,
        account(DEALER_PROGRAM_ID, policy_bytes.clone()),
    );
    // Same canonical body, same width, the WRONG owner.
    test.add_account(
        keys.substituted_policy,
        account(REGISTRY_PROGRAM_ID, policy_bytes),
    );
    test.add_account(
        keys.candidate,
        account(DEALER_PROGRAM_ID, candidate_bytes(CANDIDATE_ID).to_vec()),
    );
    test.add_account(
        keys.misbound_candidate,
        account(DEALER_PROGRAM_ID, candidate_bytes(CANDIDATE_ID).to_vec()),
    );
    test.add_account(
        keys.state,
        account(
            DEALER_PROGRAM_ID,
            dealer_state().to_bytes().expect("canonical State").to_vec(),
        ),
    );
    // A State-shaped, State-owned account that is not the State PDA.
    test.add_account(
        FOREIGN_STATE,
        account(
            DEALER_PROGRAM_ID,
            dealer_state().to_bytes().expect("canonical State").to_vec(),
        ),
    );

    for key in [
        ACTIVATION_CACHE,
        REALM,
        COLLATERAL_MINT,
        CUSTODY_AUTHORITY,
        DEALER_QUOTE,
        FEE_VAULT,
        LIVENESS_VAULT,
        CORE_MARKET,
        NON_SIGNING_ACTOR,
    ] {
        test.add_account(key, account(CORE_PROGRAM_ID, Vec::from([0_u8; 64])));
    }
    test
}

/// The canonical 23-account common frame, in descriptor order.
fn common_metas(actor: Pubkey, keys: &Addresses) -> Vec<AccountMeta> {
    Vec::from([
        AccountMeta::new(actor, true),
        AccountMeta::new_readonly(keys.policy, false),
        AccountMeta::new_readonly(keys.candidate, false),
        // Absence of an optional Candidate is encoded by aliasing the Trading
        // program key, which is why these two slots are executable.
        AccountMeta::new_readonly(DEALER_PROGRAM_ID, false),
        AccountMeta::new_readonly(DEALER_PROGRAM_ID, false),
        AccountMeta::new(keys.state, false),
        AccountMeta::new_readonly(ACTIVATION_CACHE, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(DEALER_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(&DEALER_PROGRAM_ID), false),
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(&CUSTODY_PROGRAM_ID), false),
        AccountMeta::new_readonly(CORE_MARKET, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(&CORE_PROGRAM_ID), false),
        AccountMeta::new_readonly(sysvar::clock::ID, false),
        AccountMeta::new_readonly(REALM, false),
        AccountMeta::new_readonly(COLLATERAL_MINT, false),
        AccountMeta::new_readonly(LEGACY_TOKEN_PROGRAM_ID, false),
        AccountMeta::new_readonly(CUSTODY_AUTHORITY, false),
        AccountMeta::new(DEALER_QUOTE, false),
        AccountMeta::new(FEE_VAULT, false),
        AccountMeta::new(LIVENESS_VAULT, false),
    ])
}

fn fill_request(now: u64) -> Request {
    Request {
        action: Action::Fill,
        side: Side::TakerBuys,
        outcome: 0,
        expected_state_revision: 1,
        now,
        quantity: 1,
        expected_candidate_id: CANDIDATE_ID,
        actor_id: [0; 32],
        replacement_candidate_id: [0; 32],
        expected_candidate_revision: 1,
    }
}

fn schedule_request(actor_id: [u8; 32], now: u64) -> Request {
    Request {
        action: Action::ScheduleReplacement,
        side: Side::TakerBuys,
        outcome: 0,
        expected_state_revision: 1,
        now,
        quantity: 0,
        expected_candidate_id: CANDIDATE_ID,
        actor_id,
        replacement_candidate_id: [0x64; 32],
        expected_candidate_revision: 1,
    }
}

fn liquidity_request(action: Action, actor_id: [u8; 32], now: u64) -> Request {
    Request {
        action,
        side: Side::TakerBuys,
        outcome: 3,
        expected_state_revision: 1,
        now,
        quantity: 10,
        expected_candidate_id: CANDIDATE_ID,
        actor_id,
        replacement_candidate_id: [0; 32],
        expected_candidate_revision: 1,
    }
}

fn activate_request(now: u64) -> Request {
    Request {
        action: Action::ActivateReplacement,
        side: Side::TakerBuys,
        outcome: 0,
        expected_state_revision: 1,
        now,
        quantity: 0,
        expected_candidate_id: CANDIDATE_ID,
        actor_id: [0; 32],
        replacement_candidate_id: [0x64; 32],
        expected_candidate_revision: 1,
    }
}

fn enter_terminal_request() -> Request {
    Request {
        action: Action::EnterTerminal,
        side: Side::TakerBuys,
        outcome: 0,
        expected_state_revision: 1,
        now: 0,
        quantity: 0,
        expected_candidate_id: CANDIDATE_ID,
        // Terminal entry is permissionless but identity-bound: the request must
        // name the canonical Core Market, which the phase join then checks.
        actor_id: CORE_MARKET.to_bytes(),
        replacement_candidate_id: [0; 32],
        expected_candidate_revision: 1,
    }
}

fn unwind_request() -> Request {
    Request {
        action: Action::Unwind,
        side: Side::TakerBuys,
        outcome: 0,
        expected_state_revision: 1,
        now: 0,
        quantity: 1,
        expected_candidate_id: CANDIDATE_ID,
        actor_id: [0; 32],
        replacement_candidate_id: [0; 32],
        expected_candidate_revision: 1,
    }
}

/// One of the five actions bound to `now == 0` on BOTH sides: its Lean command
/// carries no slot, so the request shape requires zero and `authenticate_clock`
/// derives the same expectation from `Action::now_discipline`.
fn retire_request() -> Request {
    Request {
        action: Action::Retire,
        side: Side::TakerBuys,
        outcome: 0,
        expected_state_revision: 1,
        now: 0,
        quantity: 0,
        expected_candidate_id: CANDIDATE_ID,
        actor_id: [0; 32],
        replacement_candidate_id: [0; 32],
        expected_candidate_revision: 1,
    }
}

fn instruction(metas: Vec<AccountMeta>, data: Vec<u8>) -> Instruction {
    Instruction {
        program_id: DEALER_PROGRAM_ID,
        accounts: metas,
        data,
    }
}

// ------------------------------------------------------------------ the driver

struct Observed {
    accepted: bool,
    code: Option<u32>,
    logs: Vec<String>,
    compute_units: u64,
}

impl Observed {
    /// Whether the chain's own log shows `program` was invoked.
    ///
    /// A refusal that never reached the program it names is not evidence about
    /// that program.
    fn invoked(&self, program: Pubkey) -> bool {
        let needle = format!("Program {program} invoke");
        self.logs.iter().any(|line| line.starts_with(&needle))
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    label: &str,
    instruction: Instruction,
) -> Result<Observed, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .copied()
        .expect("a signed transaction has a signature")
        .to_string();
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    // The runtime's own structured verdict, not a string scraped back out of a
    // log line the campaign also wrote the expectation for.
    let code = match &processed.result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(value))) => Some(*value),
        _ => None,
    };
    let failure = processed
        .result
        .clone()
        .err()
        .map(|error| format!("{error:?}"));
    let accepted = processed.result.is_ok();
    let (logs, compute_units) = processed.metadata.map_or_else(
        || (Vec::new(), None),
        |metadata| (metadata.log_messages, Some(metadata.compute_units_consumed)),
    );
    record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: compute_units,
        // This campaign does not measure its wire extent; `None` says so
        // rather than implying the frame fits Solana's packet maximum.
        wire_bytes: None,
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    SUBMITTED.fetch_add(1, Ordering::Relaxed);
    Ok(Observed {
        accepted,
        code,
        logs,
        compute_units: compute_units.unwrap_or_default(),
    })
}

/// Transactions this campaign actually put on the chain.
///
/// Counted rather than written down: the printed total is what the census reads
/// back, and a hand-maintained literal goes stale the first time a case is
/// added -- which is exactly what happened when the two liquidity actions moved
/// out of the unreachability witness and into the Registry loop.
static SUBMITTED: AtomicUsize = AtomicUsize::new(0);

/// Assert the chain reported exactly the named Dealer refusal, from inside the
/// Dealer program, and report the compute the ledger recorded for it.
fn refused(observed: &Observed, expected: u32, case: &str) -> u64 {
    assert!(
        !observed.accepted,
        "{case}: the chain accepted a transaction this case requires it to refuse"
    );
    assert!(
        observed.invoked(DEALER_PROGRAM_ID),
        "{case}: refused before the Dealer program was ever invoked; the case proves nothing \
         about Dealer"
    );
    assert_eq!(
        observed.code,
        Some(expected),
        "{case}: the chain refused for a different reason than this case tests"
    );
    observed.compute_units
}

#[tokio::test]
async fn real_dealer_elf_refuses_every_unauthenticated_family_request() {
    let keys = addresses();
    let mut context = program_test(&keys).start_with_context().await;
    let actor = context.payer.pubkey();
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    assert_ne!(
        slot, 0,
        "a submitted transaction never executes in the genesis slot; the liquidity cases below \
         depend on that being true"
    );

    let canonical = fill_request(slot).to_bytes().expect("canonical Fill");

    // ---- DealerSbfError::Instruction (0) -----------------------------------
    let observed = submit(
        &mut context,
        "dealer family refuses a request that is not REQUEST_BYTES wide",
        instruction(
            common_metas(actor, &keys),
            canonical.get(..REQUEST_BYTES - 1).expect("prefix").to_vec(),
        ),
    )
    .await
    .expect("ProgramTest processing");
    refused(&observed, REFUSAL_INSTRUCTION, "truncated request");

    let mut unknown_action = canonical.to_vec();
    unknown_action[REQUEST_ACTION_OFFSET] = 8;
    let observed = submit(
        &mut context,
        "dealer family refuses an action tag outside the eight canonical actions",
        instruction(common_metas(actor, &keys), unknown_action),
    )
    .await
    .expect("ProgramTest processing");
    refused(&observed, REFUSAL_INSTRUCTION, "unknown action tag");

    // ---- DealerSbfError::AccountFrame (1) ----------------------------------
    let mut short = common_metas(actor, &keys);
    short.truncate(COMMON_ACCOUNT_COUNT_V1 - 1);
    let observed = submit(
        &mut context,
        "dealer family refuses a common frame narrower than 23 accounts",
        instruction(short, canonical.to_vec()),
    )
    .await
    .expect("ProgramTest processing");
    refused(&observed, REFUSAL_ACCOUNT_FRAME, "narrow common frame");

    let mut unsigned = common_metas(actor, &keys);
    unsigned[0] = AccountMeta::new_readonly(NON_SIGNING_ACTOR, false);
    let observed = submit(
        &mut context,
        "dealer family refuses an actor that did not sign",
        instruction(unsigned, canonical.to_vec()),
    )
    .await
    .expect("ProgramTest processing");
    refused(&observed, REFUSAL_ACCOUNT_FRAME, "unsigned actor");

    let mut writable_policy = common_metas(actor, &keys);
    writable_policy[1] = AccountMeta::new(keys.policy, false);
    let observed = submit(
        &mut context,
        "dealer family refuses a writable immutable Policy",
        instruction(writable_policy, canonical.to_vec()),
    )
    .await
    .expect("ProgramTest processing");
    refused(&observed, REFUSAL_ACCOUNT_FRAME, "writable Policy");

    let mut readonly_state = common_metas(actor, &keys);
    readonly_state[5] = AccountMeta::new_readonly(keys.state, false);
    let observed = submit(
        &mut context,
        "dealer family refuses a read-only State it is required to commit",
        instruction(readonly_state, canonical.to_vec()),
    )
    .await
    .expect("ProgramTest processing");
    refused(&observed, REFUSAL_ACCOUNT_FRAME, "read-only State");

    // A proposed Candidate is admissible only under ScheduleReplacement; a Fill
    // that presents one is a frame refusal, not a semantic one.
    let mut proposed_on_fill = common_metas(actor, &keys);
    proposed_on_fill[4] = AccountMeta::new_readonly(keys.candidate, false);
    let observed = submit(
        &mut context,
        "dealer family refuses a proposed Candidate outside ScheduleReplacement",
        instruction(proposed_on_fill, canonical.to_vec()),
    )
    .await
    .expect("ProgramTest processing");
    refused(
        &observed,
        REFUSAL_ACCOUNT_FRAME,
        "proposed Candidate on a Fill",
    );

    // ---- DealerSbfError::AccountIdentity (2) -------------------------------
    let mut foreign_policy = common_metas(actor, &keys);
    foreign_policy[1] = AccountMeta::new_readonly(keys.substituted_policy, false);
    let observed = submit(
        &mut context,
        "dealer family refuses a Policy body owned by another program",
        instruction(foreign_policy, canonical.to_vec()),
    )
    .await
    .expect("ProgramTest processing");
    refused(
        &observed,
        REFUSAL_ACCOUNT_IDENTITY,
        "foreign-owned Policy body",
    );

    let mut misbound_candidate = common_metas(actor, &keys);
    misbound_candidate[2] = AccountMeta::new_readonly(keys.misbound_candidate, false);
    let observed = submit(
        &mut context,
        "dealer family refuses a Candidate body at another identity's address",
        instruction(misbound_candidate, canonical.to_vec()),
    )
    .await
    .expect("ProgramTest processing");
    refused(
        &observed,
        REFUSAL_ACCOUNT_IDENTITY,
        "Candidate body at a foreign PDA",
    );

    let mut substituted_state = common_metas(actor, &keys);
    substituted_state[5] = AccountMeta::new(FOREIGN_STATE, false);
    let observed = submit(
        &mut context,
        "dealer family refuses a State body that is not the Market's State PDA",
        instruction(substituted_state, canonical.to_vec()),
    )
    .await
    .expect("ProgramTest processing");
    refused(&observed, REFUSAL_ACCOUNT_IDENTITY, "substituted State PDA");

    // ---- DealerSbfError::Signature (3) -------------------------------------
    // ScheduleReplacement is the actor-bound action: its request must name the
    // signer. A well-formed request naming somebody else is a signature
    // refusal, and it is reachable only because every earlier stage passed.
    let observed = submit(
        &mut context,
        "dealer family refuses a scheduling request that names another actor",
        instruction(
            common_metas(actor, &keys),
            schedule_request([0x9a; 32], slot)
                .to_bytes()
                .expect("canonical ScheduleReplacement")
                .to_vec(),
        ),
    )
    .await
    .expect("ProgramTest processing");
    refused(&observed, REFUSAL_SIGNATURE, "substituted scheduling actor");

    // ---- DealerSbfError::Clock (4) -----------------------------------------
    let observed = submit(
        &mut context,
        "dealer family refuses a Fill bound to a slot that is not the current one",
        instruction(
            common_metas(actor, &keys),
            fill_request(slot.wrapping_add(1_000))
                .to_bytes()
                .expect("canonical Fill")
                .to_vec(),
        ),
    )
    .await
    .expect("ProgramTest processing");
    refused(&observed, REFUSAL_CLOCK, "stale slot binding");

    // ---- The padding slot, for every action whose transition reads none -----
    //
    // This block was the AddLiquidity / RemoveLiquidity UNREACHABILITY witness
    // and it is deliberately no longer that. The contradiction it recorded --
    // `Request::validate_shape` requiring `now == 0` for the two liquidity
    // actions while `authenticate_clock` required `now == clock.slot` for every
    // action but `Retire`, so no slot the chain can offer satisfied both -- was
    // closed by giving the rule one owner: `Action::now_discipline` in
    // `dclutch-dealer-codec`, which both sides now read. The five commands that
    // carry no slot in `DClutchSemantics.DealerLiquidity.Command`
    // (`enterTerminal`, `unwind`, `retire`, `addLiquidity`, `removeLiquidity`)
    // require canonical zero on BOTH sides; the three that do carry one are
    // bound to the executing slot on both. The reachable halves moved into the
    // Registry loop below. What stays here is the hostile direction: a client
    // that puts a slot in a field the transition never reads.
    for (label, mut wire) in [
        (
            "dealer family refuses RemoveLiquidity carrying a slot in its padding",
            liquidity_request(Action::RemoveLiquidity, actor.to_bytes(), 0)
                .to_bytes()
                .expect("shape-canonical RemoveLiquidity")
                .to_vec(),
        ),
        (
            "dealer family refuses EnterTerminal carrying a slot in its padding",
            enter_terminal_request()
                .to_bytes()
                .expect("shape-canonical EnterTerminal")
                .to_vec(),
        ),
        (
            "dealer family refuses Unwind carrying a slot in its padding",
            unwind_request()
                .to_bytes()
                .expect("shape-canonical Unwind")
                .to_vec(),
        ),
    ] {
        // `to_bytes` refuses to encode the slot-bearing form at all, so the
        // wire bytes are patched directly: this is exactly what a client that
        // believed the old slot rule would have to put on the wire.
        wire.get_mut(REQUEST_NOW_OFFSET..REQUEST_NOW_OFFSET + 8)
            .expect("request now field")
            .copy_from_slice(&slot.to_le_bytes());
        let observed = submit(
            &mut context,
            label,
            instruction(common_metas(actor, &keys), wire),
        )
        .await
        .expect("ProgramTest processing");
        refused(&observed, REFUSAL_INSTRUCTION, label);
    }

    // ---- DealerSbfError::Release (5), through the real Registry ELF ---------
    //
    // Each has already passed instruction shape, the 23-account frame,
    // Policy/Candidate/State identity, its own actor rule, and its own Clock
    // rule, so reaching the Registry CPI is the deepest an action can go
    // against a release set that was never activated. AddLiquidity and
    // RemoveLiquidity are here because the `now` rule now has one owner; before
    // that they refused on the Clock at every slot the chain can offer, and
    // this loop had five entries instead of seven.
    let mut deepest = 0_u64;
    for (label, request) in [
        (
            "dealer family drives ScheduleReplacement into the real Registry reauthentication",
            schedule_request(actor.to_bytes(), slot),
        ),
        (
            "dealer family drives Fill into the real Registry reauthentication",
            fill_request(slot),
        ),
        (
            "dealer family drives ActivateReplacement into the real Registry reauthentication",
            activate_request(slot),
        ),
        (
            "dealer family drives EnterTerminal into the real Registry reauthentication",
            enter_terminal_request(),
        ),
        (
            "dealer family drives Unwind into the real Registry reauthentication",
            unwind_request(),
        ),
        (
            "dealer family drives Retire into the real Registry reauthentication",
            retire_request(),
        ),
        (
            "dealer family drives AddLiquidity into the real Registry reauthentication",
            liquidity_request(Action::AddLiquidity, actor.to_bytes(), 0),
        ),
        (
            "dealer family drives RemoveLiquidity into the real Registry reauthentication",
            liquidity_request(Action::RemoveLiquidity, actor.to_bytes(), 0),
        ),
    ] {
        let observed = submit(
            &mut context,
            label,
            instruction(
                common_metas(actor, &keys),
                request.to_bytes().expect("canonical request").to_vec(),
            ),
        )
        .await
        .expect("ProgramTest processing");
        let units = refused(&observed, REFUSAL_RELEASE, label);
        assert!(
            observed.invoked(REGISTRY_PROGRAM_ID),
            "{label}: the release refusal must come from a real Registry CPI, not from Dealer's \
             own opinion of the activation cache"
        );
        deepest = deepest.max(units);
    }

    std::eprintln!(
        "dealer-family: census program map is {DEALER_LABEL}={DEALER_PROGRAM_ID} \
         {REGISTRY_LABEL}={REGISTRY_PROGRAM_ID}"
    );
    std::eprintln!(
        "dealer-family: {} real-ELF transactions; deepest case (Dealer -> Registry CPI) consumed \
         {deepest} compute units against the {COMPUTE_LIMIT} protocol ceiling",
        SUBMITTED.load(Ordering::Relaxed)
    );
}
