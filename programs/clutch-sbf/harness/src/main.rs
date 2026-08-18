//! Differential bring-up harness for the Dragon's Clutch SBF program.
//!
//! This binary builds one deterministic genesis and one transaction per
//! implemented instruction family, computes the expected post-state with the
//! **offline reference adapter** (`clutch_solana_reference::apply` and
//! `apply_with_evidence`) wherever that adapter models the family, and emits
//! everything a local `solana-test-validator` needs to execute the same
//! transitions inside a real SVM: genesis account dumps, serialized
//! transactions, expected and pre-state bytes, and a machine-readable plan.
//!
//! It never signs anything and never touches a keypair.  Every address it uses
//! is either a program-derived address of this program's frozen seed schema or
//! a program-derived address of the System program with a fixed literal seed,
//! so the fixture is reproducible and carries no key material.  Signature
//! verification is switched off at the RPC layer by `scripts/simulate.py`; see
//! `docs/implementation/SBF_BRINGUP.md` for exactly what that does and does not
//! establish.
//!
//! ## Which oracle covers which family
//!
//! | family | oracle |
//! | --- | --- |
//! | `Split`, `Merge`, `Materialize`, `Dematerialize` | `clutch_solana_reference::apply` |
//! | `Resolve`, `RedeemInternal` | `clutch_solana_reference::apply_with_evidence` |
//! | `CreateMarket` | the frozen `clutch_solana_layout` codecs re-encoded here, then accepted by `clutch_solana_reference::validate_market_init` |
//! | `FeedAdvance` | `clutch_accumulator`'s own fold plus the frozen `FeedAccount` codec |
//!
//! The last two are weaker than the first two and are labelled as such in the
//! emitted plan: the reference adapter has no `CreateMarket` transition and no
//! `FeedAdvance` at all, so there is no second implementation of those to
//! disagree with.  What is compared is still bytes against bytes, and the
//! `CreateMarket` expectation is additionally required to satisfy the
//! reference's own `validate_market_init`.
//!
//! ## Why there are several markets
//!
//! `simulateTransaction` never commits, so every transaction in the plan runs
//! against the *genesis* state.  A family whose precondition is another
//! family's post-state therefore needs its own genesis, and each such genesis
//! is produced by running the reference adapter forward from an empty market --
//! never by hand-writing the intermediate bytes.  The one place a real
//! sequence is executed is the `roundtrip` transaction, which carries `Split`
//! and `Merge` as two instructions of one transaction so that the bank itself
//! sequences them.
//!
//! Nothing here is evidence about mainnet, devnet, or any deployment.

use clutch_accumulator::{Grid, Observation, Summary, COVERAGE_POLICY_COMPLETE_REQUIRED};
use clutch_kernel::{PayoutSet, PayoutVector, MAX_PAYOUTS};
use clutch_sbf::instructions::observe_resolve::{
    BUFFER_VERSION, EVIDENCE_BUFFER_HEADER_BYTES, EVIDENCE_BUFFER_TAG, FEED_PAGE_HEADER_BYTES,
    FEED_PAGE_TAG,
};
use clutch_sbf::seeds;
use clutch_solana_layout::{
    account_len, canonical_epoch_id, canonical_market_id, canonical_order_id,
    canonical_order_set_id, canonical_outcome_id, canonical_profile_hash, canonical_realm_id,
    collateral, CandidateRecord, EpochAccount, FeedAccount, FeedId, FinalPotAccount, Hash32,
    HoardAccount, Intent, MarketAccount, OrderPageAccount, OrderRecord, OrderSlot,
    PayoutVectorBytes, PositionAccount, PriceGridAccount, ProfileAccount, RealmAccount,
    ResolutionAccount, SettlementReceiptAccount, SupplyLedgerAccount, TermsAccount,
    CANDIDATE_STATUS_SELECTED, EPOCH_PHASE_CLEARED, MAX_GRID_TICKS, MAX_INTENT_BYTES,
    MAX_ORDERS_PER_PAGE, MAX_OUTCOMES, PAYOUT_INDEX_UNRESOLVED, POT_PHASE_OPEN,
    PROFILE_FLAG_POLICY_FROZEN, PROFILE_PARENT_BYTES, RECEIPT_LEG_DIRECT, RELATION_VERSION,
};
use clutch_solana_reference::{
    apply, apply_with_evidence, validate_market_init, AccountMetadata, ActorMetadata,
    EvidenceBindings, EvidenceBytes, EvidenceMetadata, ExpectedBindings, ExternalAccount,
    KernelAccount, ReplayAccount, ResolutionEvidence, StateBytes, TransitionMetadata,
    TransitionOutput, EXTERNAL_ACCOUNT_LEN, FAIL_UNIFORM_REFUND_01, GEN_EXACT_01,
    KERNEL_ACCOUNT_LEN, REPLAY_ACCOUNT_LEN, V1_EVALUATOR_VERSION, V1_EXACT_GENERATION,
    V1_SOURCE_VERSION,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/* ------------------------------------------------------------------------ */
/* Fixture constants                                                         */
/* ------------------------------------------------------------------------ */

/// The parent-Profile preimage is exactly `PROFILE_PARENT_BYTES` long because
/// `canonical_profile_hash` refuses any other length; the contents are an
/// arbitrary fixed pattern, since this lane derives an identity and makes no
/// claim about which collateral policy the Profile commits to.
const PROFILE_PREIMAGE_FILL: u8 = 0x5b;
/// Mint identity of the fixture's collateral policy.
///
/// The Profile is *frozen* in this fixture to the digest of a **real**,
/// decodable 266-byte collateral policy (see `fixture_policy`): the offline
/// reference's `validate_market_init` now recomputes the child digest from
/// the policy bytes and compares, so an opaque digest fill would refuse.
/// Still an offline fixture: the mint named here is a fixed pattern, not a
/// chain fact.
const COLLATERAL_MINT_FILL: u8 = 0x9d;
const REALM_NONCE: u64 = 7;
const GENERATION: u64 = 2;
const CASH_ATOMS: u64 = 100;
const RESERVED_CASH_ATOMS: u64 = 7;
const COLLATERAL_CAP: u64 = 1_000;
const OUTCOME_COUNT: u8 = 2;
/// Comfortably above the rent-exempt minimum for every account in the fixture.
const ACCOUNT_LAMPORTS: u64 = 100_000_000;

/// Market nonces.  One market per distinct genesis pre-state.
const NONCE_SEAM: u64 = 9;
const NONCE_HELD: u64 = 10;
const NONCE_SHADOW: u64 = 11;
const NONCE_REDEEM: u64 = 13;
const NONCE_CREATE: u64 = 14;

/// Quantities.
const SPLIT_QUANTITY: u64 = 5;
const HELD_QUANTITY: u64 = 20;
const MERGE_QUANTITY: u64 = 5;
const MATERIALIZE_QUANTITY: u64 = 3;
const REDEEM_QUANTITY: u64 = 20;

/* Window-policy constants.  These are exactly the offline reference adapter's
 * own resolution fixture and the `observe_resolve` lifecycle vector, so that
 * the resolution differential compares two adapters over one scenario rather
 * than over two. */
const START_BUCKET: u64 = 100;
const END_BUCKET_EXCLUSIVE: u64 = 103;
const MATURITY_HORIZON: u64 = 4;
const FEED_CURSOR: u64 = 104;
const GRID_FAMILY: u32 = 7;
const GRID_VERSION: u16 = 1;
const BUCKET_SECONDS: u64 = 60;
const PRICE_SCALE: u64 = 10_000;
const WINNING_PAYOUT_INDEX: u8 = 1;
/// The declared window identity, recorded and never believed; the value is the
/// `observe_resolve` lifecycle vector's own `h(77)`.
const WINDOW_ID_FILL: u8 = 77;
/// The recorded feed-page digest of a `FeedAdvance`, likewise recorded only.
const FEED_EVIDENCE_FILL: u8 = 0x5e;

/* Batch-auction plane constants.  No adapter implements that family, so these
 * are the smallest shape the frozen codecs accept. */
const EPOCH_INDEX: u64 = 0;
const REMAINDER_SEED: u64 = 99;
const BOOK_ID_FILL: u8 = 0x6b;
const POLICY_ID_FILL: u8 = 0x70;
const SLICE_QUANTITY: u64 = 3;
const VIRTUAL_SPLIT: u64 = 5;

/* The reference request envelope and the window-evidence blob format.  Both
 * are private to `clutch_solana_reference`, so these copies are pinned by use
 * rather than by import: every blob this harness encodes is folded by
 * `apply_with_evidence` while the fixture is built, and every request it
 * encodes is decoded by `Request::decode` on the same path, so a drift in any
 * constant below is a build-time panic rather than a silent divergence. */
const REQUEST_TAG: u8 = 0xd1;
const REFERENCE_VERSION: u8 = 1;
const ACTION_LAYOUT: u8 = 0;
const ACTION_RESOLVE: u8 = 1;
const ACTION_REDEEM_INTERNAL: u8 = 2;
const WINDOW_EVIDENCE_TAG: u8 = 0x45;
const OBSERVATION_ACCEPTED: u8 = 1;

/// Refusal codes this plan expects, from `programs/clutch-sbf/program/src/error.rs`.
mod code {
    /// `ClutchError::MissingSignature`.
    pub const MISSING_SIGNATURE: u32 = 0x0002;
    /// `ClutchError::WrongPda`.
    pub const WRONG_PDA: u32 = 0x0009;
    /// `ClutchError::UnauthorizedActor`.
    pub const UNAUTHORIZED_ACTOR: u32 = 0x0011;
    /// `Error::MissingSignature` projected through `reference_code`.
    pub const REFERENCE_MISSING_SIGNATURE: u32 = 0x3009;
    /// `Error::UnauthorizedActor` projected through `reference_code`.
    pub const REFERENCE_UNAUTHORIZED_ACTOR: u32 = 0x300a;
    /// `Error::Replay` projected through `reference_code`.
    pub const REFERENCE_REPLAY: u32 = 0x3011;
    /// `Error::PayoutIndexMismatch` projected through the allocated
    /// `0x0050-0x005f` gate block (the lossy `0x3fff` collapse is gone; see
    /// `observe_resolve`'s `the_numeric_projection_of_the_gate_is_allocated`).
    pub const PAYOUT_INDEX_MISMATCH: u32 = 0x0057;
    /// `ClutchError::AlreadyInitialized`: the account-plane re-initialization
    /// refusal, allocated with the market-init appends.
    pub const ALREADY_INITIALIZED: u32 = 0x0040;
    /// `ClutchError::NotActive`.
    ///
    /// One of the two documented *refinements* in `split.rs`'s projection
    /// table rather than a rename: the offline reference adapter reports the
    /// generic `MismatchedState` for a lifecycle or close-state refusal, and
    /// this program distinguishes it.  Neither implementation accepts a
    /// request the other refuses, so the constant is written out here exactly
    /// as every other adapter-vocabulary refusal in this plan is.
    pub const NOT_ACTIVE: u32 = 0x0016;
}

const B58: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";
/// The runtime's compute-budget program.
///
/// Three of the eight families do not fit the 200 000-unit default: the
/// evidence gate decodes the immutable terms artifact four or five times, and
/// `CreateMarket` scans, writes, and then re-validates eight accounts.  A real
/// caller raises the limit the same way, so the plan raises it rather than
/// declaring those families undrivable -- and the raised number is itself the
/// measurement, recorded in `docs/implementation/SBF_BRINGUP.md`.
const COMPUTE_BUDGET_PROGRAM: &str = "ComputeBudget111111111111111111111111111111";
/// `SetComputeUnitLimit` discriminator.
const SET_COMPUTE_UNIT_LIMIT: u8 = 2;
/// The per-transaction ceiling the runtime will grant.
const COMPUTE_UNIT_CEILING: u32 = 1_400_000;

/* ------------------------------------------------------------------------ */
/* Encodings                                                                 */
/* ------------------------------------------------------------------------ */

fn b58_decode32(text: &str) -> [u8; 32] {
    let mut out = [0_u8; 32];
    for character in text.bytes() {
        let value = B58
            .iter()
            .position(|candidate| *candidate == character)
            .unwrap_or_else(|| panic!("not base58: {text}")) as u32;
        let mut carry = value;
        for byte in out.iter_mut().rev() {
            let wide = u32::from(*byte) * 58 + carry;
            *byte = (wide & 0xff) as u8;
            carry = wide >> 8;
        }
        assert_eq!(carry, 0, "base58 value wider than 32 bytes: {text}");
    }
    out
}

fn b64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map(u32::from).unwrap_or(0);
        let b2 = chunk.get(2).copied().map(u32::from).unwrap_or(0);
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[(triple >> 18) as usize & 0x3f] as char);
        out.push(B64[(triple >> 12) as usize & 0x3f] as char);
        out.push(if chunk.len() > 1 {
            B64[(triple >> 6) as usize & 0x3f] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[triple as usize & 0x3f] as char
        } else {
            '='
        });
    }
    out
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn solana_bin() -> String {
    std::env::var("SOLANA_BIN").unwrap_or_else(|_| "solana".to_string())
}

fn json_string_field(text: &str, key: &str) -> String {
    let needle = format!("\"{key}\":");
    let start = text
        .find(&needle)
        .unwrap_or_else(|| panic!("field {key} missing in: {text}"))
        + needle.len();
    let rest = &text[start..];
    let open = rest.find('"').expect("string field opening quote");
    let close = rest[open + 1..]
        .find('"')
        .expect("string field closing quote");
    rest[open + 1..open + 1 + close].to_string()
}

fn json_u64_field(text: &str, key: &str) -> u64 {
    let needle = format!("\"{key}\":");
    let start = text
        .find(&needle)
        .unwrap_or_else(|| panic!("field {key} missing in: {text}"))
        + needle.len();
    let digits: String = text[start..]
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().expect("numeric field")
}

/* ------------------------------------------------------------------------ */
/* Program-derived addresses                                                 */
/* ------------------------------------------------------------------------ */

/// One program-derived address, kept in both the forms the pipeline needs.
#[derive(Clone, Debug)]
struct Pda {
    address: String,
    bytes: [u8; 32],
    bump: u8,
}

/// Derive a program address with the pinned `solana` CLI.
///
/// Derivation deliberately does not happen in this process: the `curve25519`
/// backend needed for off-chain derivation is unavailable in this host's
/// offline crate cache.  Using the same CLI that ships with the pinned
/// validator keeps one implementation of the derivation on the host side.
fn derive(program_id: &str, seeds: &[Vec<u8>]) -> Pda {
    let mut args = vec![
        "find-program-derived-address".to_string(),
        program_id.to_string(),
    ];
    for seed in seeds {
        args.push(format!("hex:{}", hex_encode(seed)));
    }
    args.push("--output".to_string());
    args.push("json-compact".to_string());
    let output = Command::new(solana_bin())
        .args(&args)
        .output()
        .expect("failed to run the solana CLI");
    assert!(
        output.status.success(),
        "solana find-program-derived-address failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("CLI output is utf8");
    let address = json_string_field(&text, "address");
    let bump = json_u64_field(&text, "bumpSeed");
    assert!(bump <= u64::from(u8::MAX), "bump out of range");
    Pda {
        bytes: b58_decode32(&address),
        address,
        bump: bump as u8,
    }
}

/// A fixed, key-free 32-byte identity: a System-program PDA of a literal seed.
///
/// Using a derived address rather than a generated keypair keeps the fixture
/// reproducible and keeps every kind of secret out of this lane.  Seeds must
/// stay within the 32-byte single-seed limit.
fn fixed_address(label: &str) -> Pda {
    assert!(label.len() <= 32, "seed longer than 32 bytes: {label}");
    derive(SYSTEM_PROGRAM, &[label.as_bytes().to_vec()])
}

/* ------------------------------------------------------------------------ */
/* Requests and blobs                                                        */
/* ------------------------------------------------------------------------ */

/// Build the reference request envelope around one frozen layout intent.
fn layout_request(sequence: u64, intent: Intent) -> Vec<u8> {
    let mut intent_bytes = [0_u8; MAX_INTENT_BYTES];
    let len = intent.encode(&mut intent_bytes).expect("intent encodes");
    let mut out = Vec::with_capacity(13 + len);
    out.push(REQUEST_TAG);
    out.push(REFERENCE_VERSION);
    out.extend_from_slice(&sequence.to_le_bytes());
    out.push(ACTION_LAYOUT);
    out.extend_from_slice(&(len as u16).to_le_bytes());
    out.extend_from_slice(&intent_bytes[..len]);
    out
}

/// Build the reference request envelope for `Action::Resolve`.
fn resolve_request(sequence: u64, payout_index: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(12);
    out.push(REQUEST_TAG);
    out.push(REFERENCE_VERSION);
    out.extend_from_slice(&sequence.to_le_bytes());
    out.push(ACTION_RESOLVE);
    out.push(payout_index);
    out
}

/// Build the reference request envelope for `Action::RedeemInternal`.
fn redeem_request(sequence: u64, outcome: u8, quantity: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(20);
    out.push(REQUEST_TAG);
    out.push(REFERENCE_VERSION);
    out.extend_from_slice(&sequence.to_le_bytes());
    out.push(ACTION_REDEEM_INTERNAL);
    out.push(outcome);
    out.extend_from_slice(&quantity.to_le_bytes());
    out
}

/// One observation record in the reference's encoding.
type Record = (u8, u64, u128, u128);

fn encode_records(out: &mut Vec<u8>, records: &[Record]) {
    for (kind, bucket, low, high) in records {
        out.push(*kind);
        out.extend_from_slice(&bucket.to_le_bytes());
        out.extend_from_slice(&low.to_le_bytes());
        out.extend_from_slice(&high.to_le_bytes());
    }
}

/// The window this fixture's terms expect: buckets 100 and 101 land in cell 0,
/// bucket 102 terminates in cell 1, so the terminal statistic selects payout 1.
fn winning_records() -> [Record; 3] {
    [
        (OBSERVATION_ACCEPTED, 100, 0, 0),
        (OBSERVATION_ACCEPTED, 101, 0, 0),
        (OBSERVATION_ACCEPTED, 102, 1, 1),
    ]
}

/// Encode one window-evidence blob in the reference adapter's own format.
fn encode_window(feed: FeedId, records: &[Record]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(WINDOW_EVIDENCE_TAG);
    out.push(REFERENCE_VERSION);
    out.extend_from_slice(&feed.bytes());
    out.extend_from_slice(&feed.bytes());
    out.extend_from_slice(&V1_SOURCE_VERSION.to_le_bytes());
    out.extend_from_slice(&V1_EVALUATOR_VERSION.to_le_bytes());
    out.extend_from_slice(&GRID_FAMILY.to_le_bytes());
    out.extend_from_slice(&GRID_VERSION.to_le_bytes());
    out.extend_from_slice(&BUCKET_SECONDS.to_le_bytes());
    out.extend_from_slice(&START_BUCKET.to_le_bytes());
    out.extend_from_slice(&END_BUCKET_EXCLUSIVE.to_le_bytes());
    out.extend_from_slice(&(START_BUCKET + MATURITY_HORIZON).to_le_bytes());
    out.extend_from_slice(&V1_EXACT_GENERATION.to_le_bytes());
    out.extend_from_slice(&COVERAGE_POLICY_COMPLETE_REQUIRED.to_le_bytes());
    out.extend_from_slice(&0_u64.to_le_bytes());
    out.extend_from_slice(&(records.len() as u16).to_le_bytes());
    encode_records(&mut out, records);
    out
}

/// Wrap one window-evidence payload in the program's PROPOSED evidence buffer.
fn encode_evidence_buffer(window_id: Hash32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(EVIDENCE_BUFFER_TAG);
    out.push(BUFFER_VERSION);
    out.extend_from_slice(&window_id.bytes());
    out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    assert_eq!(out.len(), EVIDENCE_BUFFER_HEADER_BYTES);
    out.extend_from_slice(payload);
    out
}

/// Encode a PROPOSED feed observation page.
fn encode_feed_page(feed: Hash32, start: u64, end: u64, records: &[Record]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(FEED_PAGE_TAG);
    out.push(BUFFER_VERSION);
    out.extend_from_slice(&feed.bytes());
    out.extend_from_slice(&GRID_FAMILY.to_le_bytes());
    out.extend_from_slice(&GRID_VERSION.to_le_bytes());
    out.extend_from_slice(&BUCKET_SECONDS.to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&end.to_le_bytes());
    out.extend_from_slice(&(records.len() as u16).to_le_bytes());
    assert_eq!(out.len(), FEED_PAGE_HEADER_BYTES);
    encode_records(&mut out, records);
    out
}

/* ------------------------------------------------------------------------ */
/* Transaction assembly                                                      */
/* ------------------------------------------------------------------------ */

fn compact_u16(value: usize, out: &mut Vec<u8>) {
    let mut remaining = value;
    loop {
        let mut byte = (remaining & 0x7f) as u8;
        remaining >>= 7;
        if remaining != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if remaining == 0 {
            break;
        }
    }
}

/// One legacy message, in the account order the runtime requires.
///
/// The four groups are the message's own ordering rule -- writable signers,
/// read-only signers, writable non-signers, read-only non-signers -- and
/// [`Message::index`] is what turns a role's *key* into the index an
/// instruction account list needs.  Computing those indices by hand is how an
/// account list silently points at the wrong account, so it is not done here.
#[derive(Clone, Debug)]
struct Message {
    keys: Vec<[u8; 32]>,
    required_signatures: u8,
    readonly_signed: u8,
    readonly_unsigned: u8,
}

impl Message {
    fn new(
        writable_signers: &[[u8; 32]],
        readonly_signers: &[[u8; 32]],
        writable: &[[u8; 32]],
        readonly: &[[u8; 32]],
    ) -> Self {
        let mut keys = Vec::new();
        keys.extend_from_slice(writable_signers);
        keys.extend_from_slice(readonly_signers);
        keys.extend_from_slice(writable);
        keys.extend_from_slice(readonly);
        for (index, key) in keys.iter().enumerate() {
            assert!(
                !keys[index + 1..].contains(key),
                "duplicate key in message account list"
            );
        }
        Self {
            keys,
            required_signatures: (writable_signers.len() + readonly_signers.len()) as u8,
            readonly_signed: readonly_signers.len() as u8,
            readonly_unsigned: readonly.len() as u8,
        }
    }

    fn index(&self, key: &[u8; 32]) -> u8 {
        self.keys
            .iter()
            .position(|candidate| candidate == key)
            .expect("account is not in the message") as u8
    }

    fn indices(&self, keys: &[[u8; 32]]) -> Vec<u8> {
        keys.iter().map(|key| self.index(key)).collect()
    }
}

/// One instruction: a program index plus already-resolved account indices.
#[derive(Clone, Debug)]
struct Instruction {
    program_index: u8,
    accounts: Vec<u8>,
    data: Vec<u8>,
}

/// One `SetComputeUnitLimit` instruction, for a family the default cannot run.
fn budget_instruction(message: &Message, budget: &[u8; 32], units: u32) -> Instruction {
    let mut data = Vec::with_capacity(5);
    data.push(SET_COMPUTE_UNIT_LIMIT);
    data.extend_from_slice(&units.to_le_bytes());
    Instruction {
        program_index: message.index(budget),
        accounts: Vec::new(),
        data,
    }
}

/// Serialize one unsigned legacy transaction.
///
/// Signatures are zero-filled.  `scripts/simulate.py` sends this to
/// `simulateTransaction` with `sigVerify: false`, so the runtime executes the
/// instructions without authenticating the signature bytes.  The `is_signer`
/// bits the program sees still come from the message header, which is the fact
/// under test.
fn transaction(message: &Message, instructions: &[Instruction]) -> Vec<u8> {
    let mut out = Vec::new();
    compact_u16(usize::from(message.required_signatures), &mut out);
    for _ in 0..message.required_signatures {
        out.extend_from_slice(&[0_u8; 64]);
    }
    out.push(message.required_signatures);
    out.push(message.readonly_signed);
    out.push(message.readonly_unsigned);
    compact_u16(message.keys.len(), &mut out);
    for key in &message.keys {
        out.extend_from_slice(key);
    }
    out.extend_from_slice(&[0_u8; 32]);
    compact_u16(instructions.len(), &mut out);
    for instruction in instructions {
        out.push(instruction.program_index);
        compact_u16(instruction.accounts.len(), &mut out);
        out.extend_from_slice(&instruction.accounts);
        compact_u16(instruction.data.len(), &mut out);
        out.extend_from_slice(&instruction.data);
    }
    assert!(
        out.len() <= 1232,
        "transaction exceeds the legacy packet limit: {} bytes",
        out.len()
    );
    out
}

/* ------------------------------------------------------------------------ */
/* The shared plane                                                          */
/* ------------------------------------------------------------------------ */

fn payout_set() -> PayoutSet {
    let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
    let mut left = [0; MAX_OUTCOMES];
    left[0] = 1;
    vectors[0] = PayoutVector::new(1, left);
    let mut right = [0; MAX_OUTCOMES];
    right[1] = 1;
    vectors[1] = PayoutVector::new(1, right);
    PayoutSet::new(2, OUTCOME_COUNT, vectors)
}

fn payout_vector_bytes() -> [PayoutVectorBytes; MAX_PAYOUTS] {
    let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    let mut left = [0; MAX_OUTCOMES];
    left[0] = 1;
    let mut right = [0; MAX_OUTCOMES];
    right[1] = 1;
    payouts[0] = PayoutVectorBytes {
        denominator: 1,
        weights: left,
    };
    payouts[1] = PayoutVectorBytes {
        denominator: 1,
        weights: right,
    };
    payouts
}

/// The Realm-wide accounts every market in the plan shares.
///
/// One Realm, one Profile, one price grid, and one immutable terms artifact
/// serve every market here, because none of those four codecs binds a market
/// identity: `TermsAccount::binds_market` compares realm, profile, feed, and
/// outcome count, and this plan varies only the market nonce.
struct Shared {
    program: Pda,
    compute_budget: [u8; 32],
    payer: Pda,
    actor: Pda,
    stranger: Pda,
    imposter: Pda,
    realm_hash: Hash32,
    profile_hash: Hash32,
    feed: FeedId,
    advance_feed: FeedId,
    realm: Pda,
    profile: Pda,
    grid: Pda,
    terms: Pda,
    feed_head: Pda,
    advance_feed_head: Pda,
    terms_digest: Hash32,
    terms_account: TermsAccount,
    realm_bytes: [u8; account_len::REALM],
    profile_bytes: [u8; account_len::PROFILE],
    grid_bytes: [u8; account_len::PRICE_GRID],
    terms_bytes: [u8; account_len::TERMS],
    policy_bytes: [u8; collateral::COLLATERAL_POLICY_BYTES],
    feed_bytes: [u8; account_len::FEED],
    advance_feed_bytes: [u8; account_len::FEED],
}

/// The Realm's frozen collateral policy: a real, decodable 266-byte policy
/// whose recomputed child digest the fixture Profile freezes.
fn fixture_policy() -> collateral::CollateralPolicy {
    let backing = collateral::CurrencyRef::spl(
        collateral::TOKEN_2022_PROGRAM,
        [COLLATERAL_MINT_FILL; 32],
        9,
    );
    collateral::CollateralPolicy {
        schema_version: collateral::COLLATERAL_POLICY_SCHEMA,
        flags: collateral::COLLATERAL_POLICY_STRICT_FLAGS,
        collateral: backing,
        fee: backing,
        liveness: collateral::CurrencyRef::NATIVE_SOL,
        max_supply_atoms: 1_000_000_000,
        allowed_mint_extensions: 0,
        required_mint_extensions: 0,
        allowed_account_extensions: collateral::EXTENSION_IMMUTABLE_OWNER,
        required_account_extensions: 0,
    }
}

fn build_shared() -> Shared {
    let program = fixed_address("clutch-sbf/bringup/program/v1");
    let payer = fixed_address("clutch-sbf/bringup/payer/v1");
    let actor = fixed_address("clutch-sbf/bringup/actor/v1");
    let stranger = fixed_address("clutch-sbf/bringup/stranger/v1");
    let imposter = fixed_address("clutch-sbf/bringup/imposter/v1");
    let pid = program.address.clone();

    let profile_preimage = [PROFILE_PREIMAGE_FILL; PROFILE_PARENT_BYTES];
    let profile_hash = canonical_profile_hash(&profile_preimage)
        .expect("the fixture profile preimage must be a canonical profile hash");
    let realm_hash = canonical_realm_id(profile_hash, REALM_NONCE);
    let feed = FeedId::from_bytes([9; 32]);
    let advance_feed = FeedId::from_bytes([0x0b; 32]);

    let realm_seed = realm_hash.bytes().to_vec();
    let realm = derive(&pid, &[seeds::SEED_REALM.to_vec(), realm_seed.clone()]);
    let profile = derive(
        &pid,
        &[
            seeds::SEED_PROFILE.to_vec(),
            realm_seed.clone(),
            profile_hash.bytes().to_vec(),
        ],
    );

    let realm_account = RealmAccount {
        realm: realm_hash,
        profile: profile_hash,
        max_outcomes: MAX_OUTCOMES as u8,
        profile_version: 1,
        stored_bump: realm.bump,
        flags: 0,
    };
    let policy = fixture_policy();
    let policy_bytes = policy
        .canonical_bytes()
        .expect("the fixture collateral policy must encode");
    let profile_account = ProfileAccount {
        profile: profile_hash,
        realm: realm_hash,
        collateral_policy_digest: policy.digest().expect("the fixture policy must digest"),
        version: 1,
        flags: PROFILE_FLAG_POLICY_FROZEN,
    };

    // Frozen price grid: the exact tick domain every order limit lives on.
    let mut ticks = [0; MAX_GRID_TICKS];
    ticks[1] = 2_500;
    ticks[2] = 5_000;
    ticks[3] = 7_500;
    ticks[4] = PRICE_SCALE;
    let mut grid_account = PriceGridAccount {
        grid: Hash32::ZERO,
        realm: realm_hash,
        price_scale: PRICE_SCALE,
        tick_count: 5,
        ticks,
        stored_bump: 0,
        flags: 0,
    };
    grid_account.grid = grid_account
        .recomputed_grid_id()
        .expect("the fixture grid body must digest");
    let grid = derive(
        &pid,
        &[
            seeds::SEED_GRID.to_vec(),
            realm_seed.clone(),
            grid_account.grid.bytes().to_vec(),
        ],
    );
    grid_account.stored_bump = grid.bump;

    /* Immutable terms.  The window policy is exactly the offline reference
     * adapter's resolution fixture, so the resolution differential is a
     * disagreement between two adapters rather than between two scenarios. */
    let mut knots = [0u128; clutch_solana_layout::MAX_KNOTS];
    knots[0] = 1;
    let mut payout_map = [clutch_solana_layout::PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    payout_map[0] = 0;
    payout_map[1] = 1;
    let mut terms_account = TermsAccount {
        terms: Hash32::ZERO,
        realm: realm_hash,
        profile: profile_hash,
        feed,
        price_grid: grid_account.grid,
        outcome_count: OUTCOME_COUNT,
        payout_count: 2,
        payouts: payout_vector_bytes(),
        grid_family_id: GRID_FAMILY,
        grid_version: GRID_VERSION,
        bucket_seconds: BUCKET_SECONDS,
        expected_start_bucket: START_BUCKET,
        expected_end_bucket_exclusive: END_BUCKET_EXCLUSIVE,
        maturity_horizon_buckets: MATURITY_HORIZON,
        coverage_policy_id: u32::from(COVERAGE_POLICY_COMPLETE_REQUIRED),
        repair_policy_id: u32::from(GEN_EXACT_01),
        failure_policy_id: u32::from(FAIL_UNIFORM_REFUND_01),
        statistic_id: 1,
        ambiguity_policy_id: 1,
        edge_policy_id: 1,
        basis_degree: 0,
        knot_count: 1,
        uniform_log2_spacing: clutch_solana_layout::UNIFORM_SPACING_NONE,
        failure_payout_index: 0,
        coverage_policy_parameter: 0,
        repair_generation: 0,
        source_version: 1,
        evaluator_version: 1,
        source_adapter_id: feed,
        payout_map,
        knots,
        collateral_cap: COLLATERAL_CAP,
        stored_bump: 0,
        flags: 0,
    };
    terms_account.terms = terms_account
        .recomputed_terms_digest()
        .expect("the fixture terms body must digest");
    let terms = derive(
        &pid,
        &[
            seeds::SEED_TERMS.to_vec(),
            realm_seed.clone(),
            terms_account.terms.bytes().to_vec(),
        ],
    );
    terms_account.stored_bump = terms.bump;
    grid_account
        .binds_terms(&terms_account)
        .expect("grid binds terms");

    /* The feed head every market resolves against, already matured: its cursor
     * is past the window's maturity bound, which is the fact `Resolve` reads
     * and no caller may assert. */
    let feed_head = derive(&pid, &[seeds::SEED_FEED.to_vec(), feed.bytes().to_vec()]);
    let feed_account = FeedAccount {
        feed,
        realm: realm_hash,
        cursor: FEED_CURSOR,
        next_boundary: START_BUCKET,
        archive_pages: 1,
        /* Nonzero because the codec refuses a zero identity; this lane has no
         * accumulator summary to commit to and makes no claim about one. */
        summary: Hash32::from_bytes([0x5c; 32]),
        stored_bump: feed_head.bump,
        flags: 0,
    };

    /* A second feed identity exists only so that `FeedAdvance` has a *writable*
     * head to move.  The resolution head above must arrive read-only and
     * already matured; one account cannot be both. */
    let advance_feed_head = derive(
        &pid,
        &[seeds::SEED_FEED.to_vec(), advance_feed.bytes().to_vec()],
    );
    let advance_feed_account = FeedAccount {
        feed: advance_feed,
        realm: realm_hash,
        cursor: START_BUCKET,
        next_boundary: START_BUCKET,
        archive_pages: 0,
        summary: Hash32::from_bytes([0x5d; 32]),
        stored_bump: advance_feed_head.bump,
        flags: 0,
    };

    let mut realm_bytes = [0; account_len::REALM];
    let mut profile_bytes = [0; account_len::PROFILE];
    let mut grid_bytes = [0; account_len::PRICE_GRID];
    let mut terms_bytes = [0; account_len::TERMS];
    let mut feed_bytes = [0; account_len::FEED];
    let mut advance_feed_bytes = [0; account_len::FEED];
    realm_account.encode(&mut realm_bytes).expect("realm");
    profile_account.encode(&mut profile_bytes).expect("profile");
    grid_account.encode(&mut grid_bytes).expect("grid");
    terms_account.encode(&mut terms_bytes).expect("terms");
    feed_account.encode(&mut feed_bytes).expect("feed");
    advance_feed_account
        .encode(&mut advance_feed_bytes)
        .expect("advance feed");

    Shared {
        program,
        compute_budget: b58_decode32(COMPUTE_BUDGET_PROGRAM),
        payer,
        actor,
        stranger,
        imposter,
        realm_hash,
        profile_hash,
        feed,
        advance_feed,
        realm,
        profile,
        grid,
        terms,
        feed_head,
        advance_feed_head,
        terms_digest: terms_account.terms,
        terms_account,
        realm_bytes,
        profile_bytes,
        grid_bytes,
        terms_bytes,
        policy_bytes,
        feed_bytes,
        advance_feed_bytes,
    }
}

/* ------------------------------------------------------------------------ */
/* One market plane                                                          */
/* ------------------------------------------------------------------------ */

/// Every account of one market, plus the state bytes its genesis carries.
struct Plane {
    label: &'static str,
    market_id: Hash32,
    owner: Hash32,
    generation: u64,
    market: Pda,
    hoard: Pda,
    position: Pda,
    kernel: Pda,
    external: Pda,
    replay: Pda,
    supply: Pda,
    resolution: Pda,
    state: TransitionOutput,
    resolution_bytes: [u8; account_len::RESOLUTION],
}

fn state_bytes(state: &TransitionOutput) -> StateBytes<'_> {
    StateBytes {
        market: &state.market,
        hoard: &state.hoard,
        position: &state.position,
        kernel: &state.kernel,
        external: &state.external,
        replay: &state.replay,
        supply: &state.supply,
    }
}

impl Plane {
    /// Derive every address of one market and encode its opening state.
    ///
    /// `cash` is the founding position's cash balance; a plane built with the
    /// zero flag set carries all-zero bytes instead, which is the pre-state
    /// `CreateMarket` requires.
    fn build(shared: &Shared, label: &'static str, nonce: u64, generation: u64) -> Self {
        let pid = shared.program.address.clone();
        let market_id = canonical_market_id(shared.realm_hash, shared.profile_hash, nonce);
        let owner = Hash32::from_bytes(shared.actor.bytes);
        let realm_seed = shared.realm_hash.bytes().to_vec();
        let market_seed = market_id.bytes().to_vec();
        let owner_seed = owner.bytes().to_vec();
        let generation_seed = generation.to_le_bytes().to_vec();

        let market = derive(
            &pid,
            &[seeds::SEED_MARKET.to_vec(), realm_seed, market_seed.clone()],
        );
        let hoard = derive(&pid, &[seeds::SEED_HOARD.to_vec(), market_seed.clone()]);
        let position = derive(
            &pid,
            &[
                seeds::SEED_POSITION.to_vec(),
                market_seed.clone(),
                owner_seed.clone(),
            ],
        );
        let kernel = derive(&pid, &[seeds::SEED_KERNEL.to_vec(), market_seed.clone()]);
        let external = derive(
            &pid,
            &[
                seeds::SEED_EXTERNAL.to_vec(),
                market_seed.clone(),
                owner_seed.clone(),
                generation_seed.clone(),
            ],
        );
        let replay = derive(
            &pid,
            &[
                seeds::SEED_REPLAY.to_vec(),
                market_seed.clone(),
                owner_seed,
                generation_seed,
            ],
        );
        let supply = derive(&pid, &[seeds::SEED_SUPPLY.to_vec(), market_seed.clone()]);
        let resolution = derive(&pid, &[seeds::SEED_RESOLUTION.to_vec(), market_seed]);

        Self {
            label,
            market_id,
            owner,
            generation,
            market,
            hoard,
            position,
            kernel,
            external,
            replay,
            supply,
            resolution,
            state: zero_state(),
            resolution_bytes: [0; account_len::RESOLUTION],
        }
    }

    /// Encode the opening state of an active, empty market.
    fn open(&mut self, shared: &Shared, cash: u64) {
        let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
        outcomes[0] = canonical_outcome_id(self.market_id, 0);
        outcomes[1] = canonical_outcome_id(self.market_id, 1);
        let market_account = MarketAccount {
            market: self.market_id,
            realm: shared.realm_hash,
            profile: shared.profile_hash,
            terms: shared.terms_digest,
            outcome_count: OUTCOME_COUNT,
            lifecycle: 0,
            stored_bump: self.market.bump,
            hoard_bump: self.hoard.bump,
            outcomes,
            feed: shared.feed,
            collateral_cap: COLLATERAL_CAP,
            created_slot: 55,
            reserved: Hash32::ZERO,
        };
        let hoard_account = HoardAccount {
            market: self.market_id,
            realm: shared.realm_hash,
            authority: Hash32::from_bytes(self.hoard.bytes),
            collateral_atoms: 0,
            stored_bump: self.hoard.bump,
            flags: 0,
        };
        let position_account = PositionAccount {
            market: self.market_id,
            owner: self.owner,
            generation: self.generation,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: cash,
            reserved_cash_atoms: RESERVED_CASH_ATOMS,
            stored_bump: self.position.bump,
            close_state: 0,
        };
        let kernel_account = KernelAccount {
            market: self.market_id,
            phase: 0,
            resolved_payout: 0,
            payouts: payout_set(),
            total_supply: [0; MAX_OUTCOMES],
        };
        let external_account = ExternalAccount {
            market: self.market_id,
            owner: self.owner,
            position_generation: self.generation,
            balances: [0; MAX_OUTCOMES],
            stored_bump: self.external.bump,
            flags: 0,
        };
        let replay_account = ReplayAccount {
            market: self.market_id,
            owner: self.owner,
            position_generation: self.generation,
            sequence: 0,
            stored_bump: self.replay.bump,
            flags: 0,
        };
        let supply_account = SupplyLedgerAccount {
            market: self.market_id,
            realm: shared.realm_hash,
            generation: self.generation,
            outcome_count: OUTCOME_COUNT,
            internal_supply: [0; MAX_OUTCOMES],
            external_supply: [0; MAX_OUTCOMES],
            stored_bump: self.supply.bump,
            flags: 0,
        };
        let resolution_account = ResolutionAccount {
            market: self.market_id,
            terms: shared.terms_digest,
            feed: shared.feed,
            window: Hash32::ZERO,
            feed_cursor: 0,
            sealed_end_bucket_exclusive: 0,
            repair_generation: 0,
            resolved_slot: 0,
            payout_index: PAYOUT_INDEX_UNRESOLVED,
            stored_bump: self.resolution.bump,
            flags: 0,
        };

        shared
            .terms_account
            .binds_market(&market_account)
            .expect("terms bind the market");
        supply_account
            .binds_market(&market_account)
            .expect("supply ledger binds the market");
        resolution_account
            .binds_terms(&shared.terms_account)
            .expect("resolution binds the immutable terms");

        market_account
            .encode(&mut self.state.market)
            .expect("market");
        hoard_account.encode(&mut self.state.hoard).expect("hoard");
        position_account
            .encode(&mut self.state.position)
            .expect("position");
        kernel_account
            .encode(&mut self.state.kernel)
            .expect("kernel");
        external_account
            .encode(&mut self.state.external)
            .expect("external");
        replay_account
            .encode(&mut self.state.replay)
            .expect("replay");
        supply_account
            .encode(&mut self.state.supply)
            .expect("supply");
        resolution_account
            .encode(&mut self.resolution_bytes)
            .expect("resolution");
    }

    fn metadata(&self, shared: &Shared, actor: [u8; 32], signer: bool) -> TransitionMetadata {
        let program = Hash32::from_bytes(shared.program.bytes);
        let account = |pda: &Pda| AccountMetadata {
            key: Hash32::from_bytes(pda.bytes),
            owner_program: program,
            writable: true,
        };
        TransitionMetadata {
            market: account(&self.market),
            hoard: account(&self.hoard),
            position: account(&self.position),
            kernel: account(&self.kernel),
            external: account(&self.external),
            replay: account(&self.replay),
            supply: account(&self.supply),
            actor: ActorMetadata {
                key: Hash32::from_bytes(actor),
                signer,
            },
        }
    }

    fn bindings(&self, shared: &Shared) -> ExpectedBindings {
        ExpectedBindings {
            program_id: Hash32::from_bytes(shared.program.bytes),
            market: Hash32::from_bytes(self.market.bytes),
            hoard: Hash32::from_bytes(self.hoard.bytes),
            position: Hash32::from_bytes(self.position.bytes),
            kernel: Hash32::from_bytes(self.kernel.bytes),
            external: Hash32::from_bytes(self.external.bytes),
            replay: Hash32::from_bytes(self.replay.bytes),
            supply: Hash32::from_bytes(self.supply.bytes),
            market_bump: self.market.bump,
            hoard_bump: self.hoard.bump,
            position_bump: self.position.bump,
            external_bump: self.external.bump,
            replay_bump: self.replay.bump,
            supply_bump: self.supply.bump,
        }
    }

    fn evidence_metadata(&self, shared: &Shared, resolution_writable: bool) -> EvidenceMetadata {
        let program = Hash32::from_bytes(shared.program.bytes);
        EvidenceMetadata {
            terms: AccountMetadata {
                key: Hash32::from_bytes(shared.terms.bytes),
                owner_program: program,
                writable: false,
            },
            resolution: AccountMetadata {
                key: Hash32::from_bytes(self.resolution.bytes),
                owner_program: program,
                writable: resolution_writable,
            },
        }
    }

    fn evidence_bindings(&self, shared: &Shared) -> EvidenceBindings {
        EvidenceBindings {
            terms: Hash32::from_bytes(shared.terms.bytes),
            resolution: Hash32::from_bytes(self.resolution.bytes),
            terms_bump: shared.terms.bump,
            resolution_bump: self.resolution.bump,
            window_id: Hash32::from_bytes([WINDOW_ID_FILL; 32]),
        }
    }

    /// Run the offline reference adapter over this plane's current state.
    fn layout(
        &self,
        shared: &Shared,
        request: &[u8],
        actor: [u8; 32],
        signer: bool,
    ) -> Result<TransitionOutput, clutch_solana_reference::Error> {
        apply(
            request,
            state_bytes(&self.state),
            &self.metadata(shared, actor, signer),
            &self.bindings(shared),
        )
    }

    /// Run the offline reference adapter's evidence gate over this plane.
    fn gate(
        &self,
        shared: &Shared,
        request: &[u8],
        window: &[u8],
        resolution_writable: bool,
        actor: [u8; 32],
        signer: bool,
    ) -> Result<TransitionOutput, clutch_solana_reference::Error> {
        apply_with_evidence(
            request,
            state_bytes(&self.state),
            &ResolutionEvidence {
                bytes: EvidenceBytes {
                    terms: &shared.terms_bytes,
                    resolution: &self.resolution_bytes,
                    window,
                },
                metadata: self.evidence_metadata(shared, resolution_writable),
                bindings: self.evidence_bindings(shared),
                feed_cursor: FEED_CURSOR,
                /* Named gap: the program has no clock, so it records zero and
                 * the oracle must be told the same thing or the two would
                 * disagree about a field neither one can source. */
                resolved_slot: 0,
            },
            &self.metadata(shared, actor, signer),
            &self.bindings(shared),
        )
    }

    /// Advance this plane's genesis by one reference transition.
    fn advance(&mut self, output: TransitionOutput) {
        if let Some(record) = output.resolution {
            self.resolution_bytes = record;
        }
        self.state = output;
    }

    /// The seven state accounts, in the seam plane's account order.
    fn state_roles(&self) -> [(&'static str, &Pda); 7] {
        [
            ("market", &self.market),
            ("hoard", &self.hoard),
            ("position", &self.position),
            ("kernel", &self.kernel),
            ("external", &self.external),
            ("replay", &self.replay),
            ("supply", &self.supply),
        ]
    }

    fn state_slice(&self, role: &str) -> &[u8] {
        match role {
            "market" => &self.state.market,
            "hoard" => &self.state.hoard,
            "position" => &self.state.position,
            "kernel" => &self.state.kernel,
            "external" => &self.state.external,
            "replay" => &self.state.replay,
            "supply" => &self.state.supply,
            "resolution" => &self.resolution_bytes,
            other => panic!("unknown state role {other}"),
        }
    }
}

fn zero_state() -> TransitionOutput {
    TransitionOutput {
        market: [0; account_len::MARKET],
        hoard: [0; account_len::HOARD],
        position: [0; account_len::POSITION],
        kernel: [0; KERNEL_ACCOUNT_LEN],
        external: [0; EXTERNAL_ACCOUNT_LEN],
        replay: [0; REPLAY_ACCOUNT_LEN],
        supply: [0; account_len::SUPPLY_LEDGER],
        resolution: None,
        redemption_payout: 0,
    }
}

fn output_slice<'a>(output: &'a TransitionOutput, role: &str) -> &'a [u8] {
    match role {
        "market" => &output.market,
        "hoard" => &output.hoard,
        "position" => &output.position,
        "kernel" => &output.kernel,
        "external" => &output.external,
        "replay" => &output.replay,
        "supply" => &output.supply,
        other => panic!("unknown state role {other}"),
    }
}

/* ------------------------------------------------------------------------ */
/* The batch-auction plane                                                   */
/* ------------------------------------------------------------------------ */

/// The frozen-layout accounts no implemented instruction touches.
///
/// They are loaded at genesis so that a per-instruction lane inherits a real,
/// bound, canonically addressed plane instead of inventing one.  Every identity
/// binding the layout crate can decide is asserted while they are built; no
/// economic coherence is claimed and none should be read in.
struct Batch {
    epoch: Pda,
    page: Pda,
    candidate: Pda,
    pot: Pda,
    receipt: Pda,
    epoch_bytes: [u8; account_len::EPOCH],
    page_bytes: [u8; account_len::ORDER_PAGE],
    candidate_bytes: [u8; account_len::CANDIDATE],
    pot_bytes: [u8; account_len::FINAL_POT],
    receipt_bytes: [u8; account_len::SETTLEMENT_RECEIPT],
}

fn build_batch(shared: &Shared, plane: &Plane) -> Batch {
    let pid = shared.program.address.clone();
    let market_id = plane.market_id;
    let market_seed = market_id.bytes().to_vec();
    let epoch_id = canonical_epoch_id(market_id, EPOCH_INDEX);
    let epoch_seed = epoch_id.bytes().to_vec();
    let buyer = plane.owner;
    let seller = Hash32::from_bytes(shared.stranger.bytes);

    let epoch = derive(
        &pid,
        &[
            seeds::SEED_EPOCH.to_vec(),
            market_seed,
            EPOCH_INDEX.to_le_bytes().to_vec(),
        ],
    );
    let page = derive(
        &pid,
        &[
            seeds::SEED_PAGE.to_vec(),
            epoch_seed.clone(),
            0_u16.to_le_bytes().to_vec(),
        ],
    );

    /* Order ids are positional under OrderPage v4: slot `j` of page `p` is
     * rank `p * MAX_ORDERS_PER_PAGE + j + 1`, and nothing else decodes. */
    let buy_order_id = canonical_order_id(1);
    let sell_order_id = canonical_order_id(2);
    let mut orders = [OrderSlot::Empty; MAX_ORDERS_PER_PAGE];
    orders[0] = OrderSlot::Single(OrderRecord {
        owner: buyer,
        order_id: buy_order_id,
        outcome: 0,
        side: 0,
        quantity: 10,
        limit: 5_000,
        minimum_fill: 0,
        flags: 0,
        generation: GENERATION,
        /* No expiry horizon: this plane is a frozen shape the layout codecs
         * accept, not an economically live book. */
        expiry_epoch: u64::MAX,
    });
    orders[1] = OrderSlot::Single(OrderRecord {
        owner: seller,
        order_id: sell_order_id,
        outcome: 0,
        side: 1,
        quantity: 10,
        limit: 5_000,
        minimum_fill: 0,
        flags: 0,
        generation: GENERATION,
        /* No expiry horizon: this plane is a frozen shape the layout codecs
         * accept, not an economically live book. */
        expiry_epoch: u64::MAX,
    });
    let mut page_account = OrderPageAccount {
        market: market_id,
        epoch: epoch_id,
        order_set: Hash32::ZERO,
        page_digest: Hash32::ZERO,
        first_order_id: buy_order_id,
        last_order_id: sell_order_id,
        prev_page_last_order_id: Hash32::ZERO,
        page_index: 0,
        page_count: 1,
        set_order_count: 2,
        order_count: 2,
        tombstone_count: 0,
        frozen: 1,
        stored_bump: page.bump,
        orders,
    };
    page_account.page_digest = page_account
        .recomputed_page_digest()
        .expect("the fixture page must digest");
    let order_set = canonical_order_set_id(
        market_id,
        epoch_id,
        page_account.page_count,
        page_account.set_order_count,
        &[page_account.page_digest],
    );
    page_account.order_set = order_set;

    let epoch_account = EpochAccount {
        epoch: epoch_id,
        market: market_id,
        book: Hash32::from_bytes([BOOK_ID_FILL; 32]),
        terms: shared.terms_digest,
        price_grid: shared.terms_account.price_grid,
        policy: Hash32::from_bytes([POLICY_ID_FILL; 32]),
        order_set,
        first_order_id: buy_order_id,
        last_order_id: sell_order_id,
        epoch_index: EPOCH_INDEX,
        relation_version: RELATION_VERSION,
        price_scale: PRICE_SCALE,
        remainder_seed: REMAINDER_SEED,
        owner_count: 2,
        page_count: 1,
        order_count: 2,
        outcome_count: OUTCOME_COUNT,
        phase: EPOCH_PHASE_CLEARED,
        stored_bump: epoch.bump,
        flags: 0,
    };

    let mut prices = [0; MAX_OUTCOMES];
    prices[0] = 6_000;
    prices[1] = PRICE_SCALE - prices[0];
    let mut candidate_account = CandidateRecord {
        candidate: Hash32::ZERO,
        epoch: epoch_id,
        market: market_id,
        prices,
        virtual_split: VIRTUAL_SPLIT,
        virtual_merge: 0,
        honored_aon_mask: 0,
        weighted_direct_volume: 0,
        limit_surplus_price_units: 0,
        churn: VIRTUAL_SPLIT,
        submitted_slot: 55,
        distinct_owners: 2,
        order_len: 2,
        outcome_count: OUTCOME_COUNT,
        status: CANDIDATE_STATUS_SELECTED,
        stored_bump: 0,
        flags: 0,
    };
    candidate_account.candidate = candidate_account
        .recomputed_candidate_digest()
        .expect("the fixture candidate body must digest");
    let candidate = derive(
        &pid,
        &[
            seeds::SEED_CANDIDATE.to_vec(),
            epoch_seed.clone(),
            candidate_account.candidate.bytes().to_vec(),
        ],
    );
    candidate_account.stored_bump = candidate.bump;

    let pot = derive(&pid, &[seeds::SEED_POT.to_vec(), epoch_seed.clone()]);
    let mut pot_internal = [0; MAX_OUTCOMES];
    pot_internal[0] = VIRTUAL_SPLIT;
    pot_internal[1] = VIRTUAL_SPLIT;
    let pot_account = FinalPotAccount {
        epoch: epoch_id,
        market: market_id,
        candidate: candidate_account.candidate,
        pot_internal,
        pot_cash_price_units: 0,
        rounding_pot_price_units: 0,
        outcome_count: OUTCOME_COUNT,
        phase: POT_PHASE_OPEN,
        stored_bump: pot.bump,
        flags: 0,
    };

    let receipt = derive(
        &pid,
        &[
            seeds::SEED_RECEIPT.to_vec(),
            epoch_seed,
            candidate_account.candidate.bytes().to_vec(),
            0_u16.to_le_bytes().to_vec(),
        ],
    );
    let receipt_account = SettlementReceiptAccount {
        epoch: epoch_id,
        market: market_id,
        candidate: candidate_account.candidate,
        buy_order_id,
        sell_order_id,
        consideration_price_units: u128::from(SLICE_QUANTITY) * u128::from(prices[0]),
        quantity: SLICE_QUANTITY,
        settled_quantity: 0,
        price: prices[0],
        sequence: 1,
        slice_index: 0,
        outcome: 0,
        leg_kind: RECEIPT_LEG_DIRECT,
        consumed_flags: 0,
        stored_bump: receipt.bump,
        flags: 0,
    };

    epoch_account
        .binds_terms(&shared.terms_account, &grid_of(shared))
        .expect("epoch binds terms and grid");
    epoch_account
        .binds_page_set(&[page_account])
        .expect("epoch binds its frozen page set");
    candidate_account
        .binds_epoch(&epoch_account)
        .expect("candidate binds the frozen epoch simplex");
    pot_account
        .binds_candidate(&candidate_account)
        .expect("pot binds the selected candidate");
    receipt_account
        .binds_candidate(&candidate_account)
        .expect("receipt binds the selected candidate");

    let mut epoch_bytes = [0; account_len::EPOCH];
    let mut page_bytes = [0; account_len::ORDER_PAGE];
    let mut candidate_bytes = [0; account_len::CANDIDATE];
    let mut pot_bytes = [0; account_len::FINAL_POT];
    let mut receipt_bytes = [0; account_len::SETTLEMENT_RECEIPT];
    epoch_account.encode(&mut epoch_bytes).expect("epoch");
    page_account.encode(&mut page_bytes).expect("page");
    candidate_account
        .encode(&mut candidate_bytes)
        .expect("candidate");
    pot_account.encode(&mut pot_bytes).expect("pot");
    receipt_account.encode(&mut receipt_bytes).expect("receipt");

    Batch {
        epoch,
        page,
        candidate,
        pot,
        receipt,
        epoch_bytes,
        page_bytes,
        candidate_bytes,
        pot_bytes,
        receipt_bytes,
    }
}

fn grid_of(shared: &Shared) -> PriceGridAccount {
    PriceGridAccount::decode(&shared.grid_bytes).expect("the fixture grid decodes")
}

/* ------------------------------------------------------------------------ */
/* The emitted plan                                                          */
/* ------------------------------------------------------------------------ */

/// One genesis account dump.
struct Genesis {
    role: String,
    address: String,
    data: Vec<u8>,
}

/// One writable account the SVM must return byte-identical to the oracle.
struct Compare {
    role: String,
    address: String,
    expected: Vec<u8>,
    pre: Vec<u8>,
}

/// One transaction in the plan.
struct Case {
    name: String,
    family: &'static str,
    oracle: &'static str,
    note: String,
    tx: Vec<u8>,
    instruction_count: usize,
    /// `None` for a refusal case.
    compare: Option<Vec<Compare>>,
    /// Roles that must come back byte-identical to the genesis pre-state.
    identical_to_pre: Vec<String>,
    /// Expected `ProgramError::Custom` code, for a refusal case.
    expect_code: Option<u32>,
    /// The offline reference adapter's own refusal for the same situation.
    reference: String,
    /// Compute-unit limit this transaction asks the runtime for, when the
    /// 200 000-unit default is not enough.
    compute_limit: Option<u32>,
    /// This transaction exhausts the runtime's per-transaction compute ceiling
    /// before the program reaches a decision.
    ///
    /// It is kept in the plan, with its oracle expectation still written to
    /// disk, because "this instruction does not fit in a Solana transaction"
    /// is a measurement and not a reason to stop measuring.  The gate asserts
    /// the exhaustion, so a program that became cheap enough to finish turns
    /// this red and the evidence in `SBF_BRINGUP.md` has to be re-written
    /// rather than quietly left wrong.
    exhausted: bool,
}

impl Case {
    fn accept(
        name: &str,
        family: &'static str,
        oracle: &'static str,
        note: &str,
        tx: Vec<u8>,
        instruction_count: usize,
        compare: Vec<Compare>,
    ) -> Self {
        let identical_to_pre = compare
            .iter()
            .filter(|entry| entry.expected == entry.pre)
            .map(|entry| entry.role.clone())
            .collect();
        Self {
            name: name.to_string(),
            family,
            oracle,
            note: note.to_string(),
            tx,
            instruction_count,
            compare: Some(compare),
            identical_to_pre,
            expect_code: None,
            reference: String::new(),
            compute_limit: None,
            exhausted: false,
        }
    }

    fn refuse(
        name: &str,
        family: &'static str,
        note: &str,
        tx: Vec<u8>,
        expect_code: u32,
        reference: String,
    ) -> Self {
        Self {
            name: name.to_string(),
            family,
            oracle: "reference-refusal",
            note: note.to_string(),
            tx,
            instruction_count: 1,
            compare: None,
            identical_to_pre: Vec::new(),
            expect_code: Some(expect_code),
            reference,
            compute_limit: None,
            exhausted: false,
        }
    }
}

/// Everything the plan writer needs, accumulated as the fixture is built.
#[derive(Default)]
struct Plan {
    genesis: Vec<Genesis>,
    cases: Vec<Case>,
}

impl Plan {
    fn account(&mut self, role: &str, pda: &Pda, data: &[u8]) {
        assert!(
            !self
                .genesis
                .iter()
                .any(|entry| entry.address == pda.address),
            "two genesis accounts at one address: {role}"
        );
        self.genesis.push(Genesis {
            role: role.to_string(),
            address: pda.address.clone(),
            data: data.to_vec(),
        });
    }
}

fn compare_of(plane: &Plane, role: &str, expected: &[u8]) -> Compare {
    let address = match role {
        "market" => &plane.market,
        "hoard" => &plane.hoard,
        "position" => &plane.position,
        "kernel" => &plane.kernel,
        "external" => &plane.external,
        "replay" => &plane.replay,
        "supply" => &plane.supply,
        "resolution" => &plane.resolution,
        other => panic!("unknown role {other}"),
    };
    Compare {
        role: format!("{}.{}", plane.label, role),
        address: address.address.clone(),
        expected: expected.to_vec(),
        pre: plane.state_slice(role).to_vec(),
    }
}

/// Every writable state account of a seam or gate transition, compared.
fn state_compares(plane: &Plane, post: &TransitionOutput) -> Vec<Compare> {
    plane
        .state_roles()
        .iter()
        .map(|(role, _)| compare_of(plane, role, output_slice(post, role)))
        .collect()
}

/* ------------------------------------------------------------------------ */
/* Instruction account lists                                                 */
/* ------------------------------------------------------------------------ */

/// Build the seam plane's ten-account instruction against a message.
fn seam_instruction(
    message: &Message,
    shared: &Shared,
    plane: &Plane,
    actor: [u8; 32],
    replay_override: Option<[u8; 32]>,
    data: Vec<u8>,
) -> Instruction {
    let replay = replay_override.unwrap_or(plane.replay.bytes);
    Instruction {
        program_index: message.index(&shared.program.bytes),
        accounts: message.indices(&[
            actor,
            shared.realm.bytes,
            shared.profile.bytes,
            plane.market.bytes,
            plane.hoard.bytes,
            plane.position.bytes,
            plane.kernel.bytes,
            plane.external.bytes,
            replay,
            plane.supply.bytes,
        ]),
        data,
    }
}

/// The message every seam transaction uses.
fn seam_message(
    shared: &Shared,
    plane: &Plane,
    actor: [u8; 32],
    actor_signs: bool,
    replay_override: Option<[u8; 32]>,
) -> Message {
    let replay = replay_override.unwrap_or(plane.replay.bytes);
    let writable = [
        plane.market.bytes,
        plane.hoard.bytes,
        plane.position.bytes,
        plane.kernel.bytes,
        plane.external.bytes,
        replay,
        plane.supply.bytes,
    ];
    if actor_signs {
        Message::new(
            &[shared.payer.bytes],
            &[actor],
            &writable,
            &[
                shared.realm.bytes,
                shared.profile.bytes,
                shared.program.bytes,
            ],
        )
    } else {
        Message::new(
            &[shared.payer.bytes],
            &[],
            &writable,
            &[
                shared.realm.bytes,
                shared.profile.bytes,
                actor,
                shared.program.bytes,
            ],
        )
    }
}

/// The message and instruction of one evidence-gated transaction.
fn gate_transaction(
    shared: &Shared,
    plane: &Plane,
    buffer: &Pda,
    actor: [u8; 32],
    actor_signs: bool,
    resolution_writable: bool,
    data: Vec<u8>,
) -> Vec<u8> {
    let mut writable = vec![
        plane.market.bytes,
        plane.hoard.bytes,
        plane.position.bytes,
        plane.kernel.bytes,
        plane.external.bytes,
        plane.replay.bytes,
        plane.supply.bytes,
    ];
    let mut readonly = vec![
        shared.terms.bytes,
        shared.feed_head.bytes,
        buffer.bytes,
        shared.program.bytes,
        shared.compute_budget,
    ];
    if resolution_writable {
        writable.push(plane.resolution.bytes);
    } else {
        readonly.insert(1, plane.resolution.bytes);
    }
    let message = if actor_signs {
        Message::new(&[shared.payer.bytes], &[actor], &writable, &readonly)
    } else {
        let mut readonly_unsigned = readonly.clone();
        readonly_unsigned.insert(0, actor);
        Message::new(&[shared.payer.bytes], &[], &writable, &readonly_unsigned)
    };
    let instruction = Instruction {
        program_index: message.index(&shared.program.bytes),
        accounts: message.indices(&[
            actor,
            plane.market.bytes,
            plane.hoard.bytes,
            plane.position.bytes,
            plane.kernel.bytes,
            plane.external.bytes,
            plane.replay.bytes,
            plane.supply.bytes,
            shared.terms.bytes,
            plane.resolution.bytes,
            shared.feed_head.bytes,
            buffer.bytes,
        ]),
        data,
    };
    transaction(
        &message,
        &[
            budget_instruction(&message, &shared.compute_budget, COMPUTE_UNIT_CEILING),
            instruction,
        ],
    )
}

/// The message and instruction of one `FeedAdvance` transaction.
fn advance_transaction(
    shared: &Shared,
    buffer: &Pda,
    actor: [u8; 32],
    actor_signs: bool,
    data: Vec<u8>,
) -> Vec<u8> {
    let writable = [shared.advance_feed_head.bytes];
    let message = if actor_signs {
        Message::new(
            &[shared.payer.bytes],
            &[actor],
            &writable,
            &[buffer.bytes, shared.program.bytes],
        )
    } else {
        Message::new(
            &[shared.payer.bytes],
            &[],
            &writable,
            &[buffer.bytes, actor, shared.program.bytes],
        )
    };
    let instruction = Instruction {
        program_index: message.index(&shared.program.bytes),
        accounts: message.indices(&[actor, shared.advance_feed_head.bytes, buffer.bytes]),
        data,
    };
    transaction(&message, &[instruction])
}

/// The message and instruction of one `CreateMarket` transaction.
fn create_transaction(
    shared: &Shared,
    plane: &Plane,
    creator: [u8; 32],
    creator_signs: bool,
    data: Vec<u8>,
) -> Vec<u8> {
    let writable = [
        plane.market.bytes,
        plane.hoard.bytes,
        plane.position.bytes,
        plane.kernel.bytes,
        plane.external.bytes,
        plane.replay.bytes,
        plane.supply.bytes,
        plane.resolution.bytes,
    ];
    let readonly = [
        shared.realm.bytes,
        shared.profile.bytes,
        shared.terms.bytes,
        shared.program.bytes,
        shared.compute_budget,
    ];
    let message = if creator_signs {
        Message::new(&[shared.payer.bytes], &[creator], &writable, &readonly)
    } else {
        let mut readonly_unsigned = readonly.to_vec();
        readonly_unsigned.insert(0, creator);
        Message::new(&[shared.payer.bytes], &[], &writable, &readonly_unsigned)
    };
    let instruction = Instruction {
        program_index: message.index(&shared.program.bytes),
        accounts: message.indices(&[
            creator,
            shared.realm.bytes,
            shared.profile.bytes,
            shared.terms.bytes,
            plane.market.bytes,
            plane.hoard.bytes,
            plane.position.bytes,
            plane.kernel.bytes,
            plane.external.bytes,
            plane.replay.bytes,
            plane.supply.bytes,
            plane.resolution.bytes,
        ]),
        data,
    };
    transaction(
        &message,
        &[
            budget_instruction(&message, &shared.compute_budget, COMPUTE_UNIT_CEILING),
            instruction,
        ],
    )
}

/* ------------------------------------------------------------------------ */
/* Fixture assembly                                                          */
/* ------------------------------------------------------------------------ */

/// Everything the plan needs, built once.
struct Fixture {
    shared: Shared,
    seam: Plane,
    held: Plane,
    shadow: Plane,
    redeem: Plane,
    create: Plane,
    batch: Batch,
    resolve_buffer: Pda,
    resolve_buffer_bytes: Vec<u8>,
    redeem_buffer: Pda,
    redeem_buffer_bytes: Vec<u8>,
    page_buffer: Pda,
    page_buffer_bytes: Vec<u8>,
    /// The `FeedAdvance` post-state, folded by the accumulator here.
    advanced_feed_bytes: [u8; account_len::FEED],
    /// The eight founding account images `CreateMarket` must write.
    created: TransitionOutput,
    created_resolution: [u8; account_len::RESOLUTION],
    /// The lifecycle walk: one market, one ordered narrative, one gate.
    walk: Walk,
}

fn build_fixture() -> Fixture {
    let shared = build_shared();

    /* The seam market: an empty, active market at replay sequence zero. */
    let mut seam = Plane::build(&shared, "seam", NONCE_SEAM, GENERATION);
    seam.open(&shared, CASH_ATOMS);

    /* The held market: the same market after the *oracle itself* split twenty
     * complete sets out of it.  A pre-state a lane hand-wrote would be a state
     * neither implementation produced. */
    let mut held = Plane::build(&shared, "held", NONCE_HELD, GENERATION);
    held.open(&shared, CASH_ATOMS);
    let split_twenty = layout_request(
        0,
        Intent::Split {
            market: held.market_id,
            owner: held.owner,
            quantity: HELD_QUANTITY,
        },
    );
    let held_post = held
        .layout(&shared, &split_twenty, shared.actor.bytes, true)
        .expect("the oracle splits the held market open");
    held.advance(held_post);

    /* The shadow market: split, then materialized, so `Dematerialize` has an
     * external balance to move back. */
    let mut shadow = Plane::build(&shared, "shadow", NONCE_SHADOW, GENERATION);
    shadow.open(&shared, CASH_ATOMS);
    let shadow_split = layout_request(
        0,
        Intent::Split {
            market: shadow.market_id,
            owner: shadow.owner,
            quantity: HELD_QUANTITY,
        },
    );
    let shadow_post = shadow
        .layout(&shared, &shadow_split, shared.actor.bytes, true)
        .expect("the oracle splits the shadow market open");
    shadow.advance(shadow_post);
    let shadow_materialize = layout_request(
        1,
        Intent::Materialize {
            market: shadow.market_id,
            owner: shadow.owner,
            destination: Hash32::from_bytes(shadow.external.bytes),
            outcome: 0,
            quantity: MATERIALIZE_QUANTITY,
        },
    );
    let shadow_post = shadow
        .layout(&shared, &shadow_materialize, shared.actor.bytes, true)
        .expect("the oracle materializes the shadow market");
    shadow.advance(shadow_post);

    /* The redeem market: split, then resolved by the oracle's evidence gate, so
     * `RedeemInternal` starts from a state a resolve actually produced. */
    let mut redeem = Plane::build(&shared, "redeem", NONCE_REDEEM, GENERATION);
    redeem.open(&shared, CASH_ATOMS);
    let redeem_split = layout_request(
        0,
        Intent::Split {
            market: redeem.market_id,
            owner: redeem.owner,
            quantity: HELD_QUANTITY,
        },
    );
    let redeem_post = redeem
        .layout(&shared, &redeem_split, shared.actor.bytes, true)
        .expect("the oracle splits the redeem market open");
    redeem.advance(redeem_post);
    let window = encode_window(shared.feed, &winning_records());
    let redeem_resolve = resolve_request(1, WINNING_PAYOUT_INDEX);
    let redeem_post = redeem
        .gate(
            &shared,
            &redeem_resolve,
            &window,
            true,
            shared.actor.bytes,
            true,
        )
        .expect("the oracle resolves the redeem market");
    redeem.advance(redeem_post);

    /* The created market: eight zeroed accounts at their canonical addresses. */
    let create = Plane::build(&shared, "create", NONCE_CREATE, 0);

    let batch = build_batch(&shared, &seam);

    /* Caller-supplied buffers.  None of the three is address-bound: the buffer
     * is the one account in the program that is deliberately not, because its
     * bytes are the claim and not the state. */
    let resolve_buffer = fixed_address("clutch/bringup/buffer/resolve/v1");
    let resolve_buffer_bytes =
        encode_evidence_buffer(Hash32::from_bytes([WINDOW_ID_FILL; 32]), &window);
    let redeem_buffer = fixed_address("clutch/bringup/buffer/redeem/v1");
    let redeem_buffer_bytes = encode_evidence_buffer(Hash32::from_bytes([WINDOW_ID_FILL; 32]), &[]);
    let page_buffer = fixed_address("clutch/bringup/buffer/page/v1");
    let page_records: Vec<Record> = (START_BUCKET..END_BUCKET_EXCLUSIVE)
        .map(|bucket| (OBSERVATION_ACCEPTED, bucket, 40, 41))
        .collect();
    let page_buffer_bytes = encode_feed_page(
        Hash32::from_bytes(shared.advance_feed.bytes()),
        START_BUCKET,
        END_BUCKET_EXCLUSIVE,
        &page_records,
    );

    /* The `FeedAdvance` expectation is the accumulator's own fold, not a
     * restatement of what the program does: the page is folded here with
     * `Summary::append`, and the cursor the fold lands on is what the expected
     * post-state carries. */
    let advanced_feed_bytes = fold_feed_page(&shared, &page_records);

    let (created, created_resolution) = founding_plane(&shared, &create, NONCE_CREATE);

    let walk = build_walk(&shared);

    Fixture {
        shared,
        seam,
        held,
        shadow,
        redeem,
        create,
        batch,
        resolve_buffer,
        resolve_buffer_bytes,
        redeem_buffer,
        redeem_buffer_bytes,
        page_buffer,
        page_buffer_bytes,
        advanced_feed_bytes,
        created,
        created_resolution,
        walk,
    }
}

/// Fold the `FeedAdvance` page with the accumulator and encode the post-state.
fn fold_feed_page(shared: &Shared, records: &[Record]) -> [u8; account_len::FEED] {
    let grid = Grid::new(GRID_FAMILY, GRID_VERSION, BUCKET_SECONDS).expect("the fixture grid");
    let mut summary = Summary::empty(grid);
    for (kind, bucket, low, high) in records {
        assert_eq!(*kind, OBSERVATION_ACCEPTED, "the fixture page is complete");
        summary = summary
            .append(Observation::accepted(*bucket, *low, *high))
            .expect("the fixture page folds");
    }
    let cursor = summary
        .end_bucket_exclusive()
        .expect("a folded page has an end bucket");
    let mut feed = FeedAccount::decode(&shared.advance_feed_bytes).expect("the advance feed head");
    feed.cursor = cursor;
    feed.archive_pages += 1;
    feed.summary = Hash32::from_bytes([FEED_EVIDENCE_FILL; 32]);
    let mut bytes = [0; account_len::FEED];
    feed.encode(&mut bytes).expect("the advanced feed head");
    bytes
}

/// The eight account images a `CreateMarket` must produce.
///
/// This is an independent re-encode through the frozen `clutch_solana_layout`
/// and reference-only codecs, from the intent and the immutable terms alone,
/// following exactly the PROPOSED initial values `market_init.rs` documents:
/// the terms' collateral cap, a zero created slot, generation zero, the Hoard PDA
/// as its own authority, the terms artifact's payout set, and an unresolved
/// resolution record.  It is then required to satisfy the reference adapter's
/// own `validate_market_init`, which is what makes it an oracle rather than a
/// restatement.
fn founding_plane(
    shared: &Shared,
    plane: &Plane,
    nonce: u64,
) -> (TransitionOutput, [u8; account_len::RESOLUTION]) {
    assert_eq!(
        plane.market_id,
        canonical_market_id(shared.realm_hash, shared.profile_hash, nonce),
        "the founding plane must be the plane the intent names"
    );
    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    outcomes[0] = canonical_outcome_id(plane.market_id, 0);
    outcomes[1] = canonical_outcome_id(plane.market_id, 1);
    let market_account = MarketAccount {
        market: plane.market_id,
        realm: shared.realm_hash,
        profile: shared.profile_hash,
        terms: shared.terms_digest,
        outcome_count: OUTCOME_COUNT,
        lifecycle: 0,
        stored_bump: plane.market.bump,
        hoard_bump: plane.hoard.bump,
        outcomes,
        feed: shared.feed,
        collateral_cap: COLLATERAL_CAP,
        created_slot: 0,
        reserved: Hash32::ZERO,
    };
    let hoard_account = HoardAccount {
        market: plane.market_id,
        realm: shared.realm_hash,
        authority: Hash32::from_bytes(plane.hoard.bytes),
        collateral_atoms: 0,
        stored_bump: plane.hoard.bump,
        flags: 0,
    };
    let position_account = PositionAccount {
        market: plane.market_id,
        owner: plane.owner,
        generation: 0,
        internal: [0; MAX_OUTCOMES],
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        stored_bump: plane.position.bump,
        close_state: 0,
    };
    let kernel_account = KernelAccount {
        market: plane.market_id,
        phase: 0,
        resolved_payout: 0,
        payouts: payout_set(),
        total_supply: [0; MAX_OUTCOMES],
    };
    let external_account = ExternalAccount {
        market: plane.market_id,
        owner: plane.owner,
        position_generation: 0,
        balances: [0; MAX_OUTCOMES],
        stored_bump: plane.external.bump,
        flags: 0,
    };
    let replay_account = ReplayAccount {
        market: plane.market_id,
        owner: plane.owner,
        position_generation: 0,
        sequence: 0,
        stored_bump: plane.replay.bump,
        flags: 0,
    };
    let supply_account = SupplyLedgerAccount {
        market: plane.market_id,
        realm: shared.realm_hash,
        generation: 0,
        outcome_count: OUTCOME_COUNT,
        internal_supply: [0; MAX_OUTCOMES],
        external_supply: [0; MAX_OUTCOMES],
        stored_bump: plane.supply.bump,
        flags: 0,
    };
    let resolution_account = ResolutionAccount {
        market: plane.market_id,
        terms: shared.terms_digest,
        feed: shared.feed,
        window: Hash32::ZERO,
        feed_cursor: 0,
        sealed_end_bucket_exclusive: 0,
        repair_generation: 0,
        resolved_slot: 0,
        payout_index: PAYOUT_INDEX_UNRESOLVED,
        stored_bump: plane.resolution.bump,
        flags: 0,
    };

    let mut state = zero_state();
    market_account.encode(&mut state.market).expect("market");
    hoard_account.encode(&mut state.hoard).expect("hoard");
    position_account
        .encode(&mut state.position)
        .expect("position");
    kernel_account.encode(&mut state.kernel).expect("kernel");
    external_account
        .encode(&mut state.external)
        .expect("external");
    replay_account.encode(&mut state.replay).expect("replay");
    supply_account.encode(&mut state.supply).expect("supply");
    let mut resolution_bytes = [0; account_len::RESOLUTION];
    resolution_account
        .encode(&mut resolution_bytes)
        .expect("resolution");
    resolution_account
        .binds_terms(&shared.terms_account)
        .expect("the founding record binds the immutable terms");

    validate_market_init(
        &shared.realm_bytes,
        &shared.profile_bytes,
        &shared.policy_bytes,
        &shared.terms_bytes,
        state_bytes(&state),
        &create_intent_bytes(shared, nonce),
        &plane.metadata(shared, shared.actor.bytes, true),
        &plane.bindings(shared),
    )
    .expect("the offline reference adapter must accept the founding plane");

    (state, resolution_bytes)
}

/// The frozen `CreateMarket` intent bytes for one market nonce.
fn create_intent(shared: &Shared, nonce: u64) -> Intent {
    Intent::CreateMarket {
        realm: shared.realm_hash,
        profile: shared.profile_hash,
        market_nonce: nonce,
        outcome_count: OUTCOME_COUNT,
        terms: shared.terms_digest,
        feed: shared.feed,
    }
}

/// The bare frozen intent encoding `validate_market_init` reads.
fn create_intent_bytes(shared: &Shared, nonce: u64) -> Vec<u8> {
    let mut bytes = [0_u8; MAX_INTENT_BYTES];
    let len = create_intent(shared, nonce)
        .encode(&mut bytes)
        .expect("create intent encodes");
    bytes[..len].to_vec()
}

/* ------------------------------------------------------------------------ */
/* Cases                                                                     */
/* ------------------------------------------------------------------------ */

fn refusal_text(error: clutch_solana_reference::Error) -> String {
    format!("{error:?}")
}

/// The numeric code a refusal the program shares with the oracle projects to.
///
/// This is used only where both implementations raise the *same* class -- the
/// pure kernel's own refusals, which neither adapter re-vocabularizes -- so the
/// expected number is `error.rs`'s projection of the class the oracle actually
/// returned rather than a constant this harness typed out and could get wrong.
/// Every adapter-vocabulary refusal keeps an explicit constant, because there
/// the two implementations deliberately differ.
fn shared_class_code(error: clutch_solana_reference::Error) -> u32 {
    clutch_sbf::error::reference_code(error)
}

fn build_cases(f: &Fixture) -> Vec<Case> {
    let shared = &f.shared;
    let actor = shared.actor.bytes;
    let mut cases = Vec::new();

    /* ---------------------------------------------------------------- */
    /* Split, on the empty seam market                                   */
    /* ---------------------------------------------------------------- */
    let split = layout_request(
        0,
        Intent::Split {
            market: f.seam.market_id,
            owner: f.seam.owner,
            quantity: SPLIT_QUANTITY,
        },
    );
    let split_post = f
        .seam
        .layout(shared, &split, actor, true)
        .expect("the oracle accepts the bring-up Split");
    let message = seam_message(shared, &f.seam, actor, true, None);
    let instruction = seam_instruction(&message, shared, &f.seam, actor, None, split.clone());
    cases.push(Case::accept(
        "split",
        "Split",
        "reference::apply",
        "one Split of five complete sets on the ten-account seam plane",
        transaction(&message, std::slice::from_ref(&instruction)),
        1,
        state_compares(&f.seam, &split_post),
    ));

    /* Split then Merge, sequenced by the bank inside one transaction. */
    let merge_back = layout_request(
        1,
        Intent::Merge {
            market: f.seam.market_id,
            owner: f.seam.owner,
            quantity: SPLIT_QUANTITY,
        },
    );
    let after_split = Plane::clone_state(&f.seam, &split_post);
    let roundtrip_post = after_split
        .layout(shared, &merge_back, actor, true)
        .expect("the oracle merges the round trip closed");
    let merge_instruction =
        seam_instruction(&message, shared, &f.seam, actor, None, merge_back.clone());
    let roundtrip_compares = state_compares(&f.seam, &roundtrip_post);
    for entry in &roundtrip_compares {
        if entry.role.ends_with(".replay") {
            assert_ne!(
                entry.expected, entry.pre,
                "the round trip must consume two sequences"
            );
        } else {
            assert_eq!(
                entry.expected, entry.pre,
                "the round trip must restore {} exactly",
                entry.role
            );
        }
    }
    cases.push(Case::accept(
        "roundtrip",
        "Split+Merge",
        "reference::apply",
        "Split then Merge as two instructions of one transaction; every account except the replay sequence must return to its pre-state",
        transaction(&message, &[instruction, merge_instruction]),
        2,
        roundtrip_compares,
    ));

    /* Refusal: the position owner is present but never signed. */
    let unsigned_message = seam_message(shared, &f.seam, actor, false, None);
    let unsigned_instruction = seam_instruction(
        &unsigned_message,
        shared,
        &f.seam,
        actor,
        None,
        split.clone(),
    );
    cases.push(Case::refuse(
        "split-unsigned",
        "Split",
        "the position owner is present, read-only, and never signed",
        transaction(&unsigned_message, &[unsigned_instruction]),
        code::MISSING_SIGNATURE,
        refusal_text(
            f.seam
                .layout(shared, &split, actor, false)
                .expect_err("the oracle refuses an unsigned Split"),
        ),
    ));

    /* Refusal: an authenticated signer who is not the position owner. */
    let stranger_message = seam_message(shared, &f.seam, shared.stranger.bytes, true, None);
    let stranger_instruction = seam_instruction(
        &stranger_message,
        shared,
        &f.seam,
        shared.stranger.bytes,
        None,
        split.clone(),
    );
    cases.push(Case::refuse(
        "split-stranger",
        "Split",
        "a different authenticated signer presents the owner's position",
        transaction(&stranger_message, &[stranger_instruction]),
        code::UNAUTHORIZED_ACTOR,
        refusal_text(
            f.seam
                .layout(shared, &split, shared.stranger.bytes, true)
                .expect_err("the oracle refuses a stranger's Split"),
        ),
    ));

    /* Refusal: byte-identical replay state at a non-canonical address. */
    let imposter_message = seam_message(shared, &f.seam, actor, true, Some(shared.imposter.bytes));
    let imposter_instruction = seam_instruction(
        &imposter_message,
        shared,
        &f.seam,
        actor,
        Some(shared.imposter.bytes),
        split.clone(),
    );
    let mut imposter_metadata = f.seam.metadata(shared, actor, true);
    imposter_metadata.replay.key = Hash32::from_bytes(shared.imposter.bytes);
    cases.push(Case::refuse(
        "split-imposter",
        "Split",
        "byte-identical replay state at an address that is not the canonical replay PDA",
        transaction(&imposter_message, &[imposter_instruction]),
        code::WRONG_PDA,
        refusal_text(
            apply(
                &split,
                state_bytes(&f.seam.state),
                &imposter_metadata,
                &f.seam.bindings(shared),
            )
            .expect_err("the oracle refuses an imposter replay account"),
        ),
    ));

    /* ---------------------------------------------------------------- */
    /* Merge and Materialize, on the held market                         */
    /* ---------------------------------------------------------------- */
    let merge = layout_request(
        1,
        Intent::Merge {
            market: f.held.market_id,
            owner: f.held.owner,
            quantity: MERGE_QUANTITY,
        },
    );
    let merge_post = f
        .held
        .layout(shared, &merge, actor, true)
        .expect("the oracle accepts the Merge");
    let held_message = seam_message(shared, &f.held, actor, true, None);
    cases.push(Case::accept(
        "merge",
        "Merge",
        "reference::apply",
        "merge five complete sets back into cash on a market holding twenty",
        transaction(
            &held_message,
            &[seam_instruction(
                &held_message,
                shared,
                &f.held,
                actor,
                None,
                merge.clone(),
            )],
        ),
        1,
        state_compares(&f.held, &merge_post),
    ));

    let held_unsigned = seam_message(shared, &f.held, actor, false, None);
    cases.push(Case::refuse(
        "merge-unsigned",
        "Merge",
        "the position owner is present, read-only, and never signed",
        transaction(
            &held_unsigned,
            &[seam_instruction(
                &held_unsigned,
                shared,
                &f.held,
                actor,
                None,
                merge.clone(),
            )],
        ),
        code::MISSING_SIGNATURE,
        refusal_text(
            f.held
                .layout(shared, &merge, actor, false)
                .expect_err("the oracle refuses an unsigned Merge"),
        ),
    ));

    let overdraw = layout_request(
        1,
        Intent::Merge {
            market: f.held.market_id,
            owner: f.held.owner,
            quantity: HELD_QUANTITY + 1,
        },
    );
    let overdraw_refusal = f
        .held
        .layout(shared, &overdraw, actor, true)
        .expect_err("the oracle refuses an overdrawing Merge");
    cases.push(Case::refuse(
        "merge-overdraw",
        "Merge",
        "merge one more complete set than the position holds",
        transaction(
            &held_message,
            &[seam_instruction(
                &held_message,
                shared,
                &f.held,
                actor,
                None,
                overdraw.clone(),
            )],
        ),
        shared_class_code(overdraw_refusal),
        refusal_text(overdraw_refusal),
    ));

    let materialize = layout_request(
        1,
        Intent::Materialize {
            market: f.held.market_id,
            owner: f.held.owner,
            destination: Hash32::from_bytes(f.held.external.bytes),
            outcome: 0,
            quantity: MATERIALIZE_QUANTITY,
        },
    );
    let materialize_post = f
        .held
        .layout(shared, &materialize, actor, true)
        .expect("the oracle accepts the Materialize");
    cases.push(Case::accept(
        "materialize",
        "Materialize",
        "reference::apply",
        "move three atoms of outcome zero from the internal ledger to the external shadow",
        transaction(
            &held_message,
            &[seam_instruction(
                &held_message,
                shared,
                &f.held,
                actor,
                None,
                materialize.clone(),
            )],
        ),
        1,
        state_compares(&f.held, &materialize_post),
    ));

    cases.push(Case::refuse(
        "materialize-unsigned",
        "Materialize",
        "the position owner is present, read-only, and never signed",
        transaction(
            &held_unsigned,
            &[seam_instruction(
                &held_unsigned,
                shared,
                &f.held,
                actor,
                None,
                materialize.clone(),
            )],
        ),
        code::MISSING_SIGNATURE,
        refusal_text(
            f.held
                .layout(shared, &materialize, actor, false)
                .expect_err("the oracle refuses an unsigned Materialize"),
        ),
    ));

    let wrong_destination = layout_request(
        1,
        Intent::Materialize {
            market: f.held.market_id,
            owner: f.held.owner,
            destination: Hash32::from_bytes(shared.imposter.bytes),
            outcome: 0,
            quantity: MATERIALIZE_QUANTITY,
        },
    );
    cases.push(Case::refuse(
        "materialize-wrong-destination",
        "Materialize",
        "the caller names a destination that is not the derived external-shadow address",
        transaction(
            &held_message,
            &[seam_instruction(
                &held_message,
                shared,
                &f.held,
                actor,
                None,
                wrong_destination.clone(),
            )],
        ),
        code::WRONG_PDA,
        refusal_text(
            f.held
                .layout(shared, &wrong_destination, actor, true)
                .expect_err("the oracle refuses a mis-named destination"),
        ),
    ));

    /* ---------------------------------------------------------------- */
    /* Dematerialize, on the shadow market                               */
    /* ---------------------------------------------------------------- */
    let dematerialize = layout_request(
        2,
        Intent::Dematerialize {
            market: f.shadow.market_id,
            owner: f.shadow.owner,
            source: Hash32::from_bytes(f.shadow.external.bytes),
            outcome: 0,
            quantity: MATERIALIZE_QUANTITY,
        },
    );
    let dematerialize_post = f
        .shadow
        .layout(shared, &dematerialize, actor, true)
        .expect("the oracle accepts the Dematerialize");
    let shadow_message = seam_message(shared, &f.shadow, actor, true, None);
    cases.push(Case::accept(
        "dematerialize",
        "Dematerialize",
        "reference::apply",
        "move three atoms of outcome zero from the external shadow back to the internal ledger",
        transaction(
            &shadow_message,
            &[seam_instruction(
                &shadow_message,
                shared,
                &f.shadow,
                actor,
                None,
                dematerialize.clone(),
            )],
        ),
        1,
        state_compares(&f.shadow, &dematerialize_post),
    ));

    let shadow_unsigned = seam_message(shared, &f.shadow, actor, false, None);
    cases.push(Case::refuse(
        "dematerialize-unsigned",
        "Dematerialize",
        "the position owner is present, read-only, and never signed",
        transaction(
            &shadow_unsigned,
            &[seam_instruction(
                &shadow_unsigned,
                shared,
                &f.shadow,
                actor,
                None,
                dematerialize.clone(),
            )],
        ),
        code::MISSING_SIGNATURE,
        refusal_text(
            f.shadow
                .layout(shared, &dematerialize, actor, false)
                .expect_err("the oracle refuses an unsigned Dematerialize"),
        ),
    ));

    let demat_overdraw = layout_request(
        2,
        Intent::Dematerialize {
            market: f.shadow.market_id,
            owner: f.shadow.owner,
            source: Hash32::from_bytes(f.shadow.external.bytes),
            outcome: 0,
            quantity: MATERIALIZE_QUANTITY + 1,
        },
    );
    let demat_refusal = f
        .shadow
        .layout(shared, &demat_overdraw, actor, true)
        .expect_err("the oracle refuses an overdrawing Dematerialize");
    cases.push(Case::refuse(
        "dematerialize-overdraw",
        "Dematerialize",
        "dematerialize one more atom than the external shadow holds",
        transaction(
            &shadow_message,
            &[seam_instruction(
                &shadow_message,
                shared,
                &f.shadow,
                actor,
                None,
                demat_overdraw.clone(),
            )],
        ),
        shared_class_code(demat_refusal),
        refusal_text(demat_refusal),
    ));

    /* ---------------------------------------------------------------- */
    /* Resolve, on the held market                                       */
    /* ---------------------------------------------------------------- */
    let window = encode_window(shared.feed, &winning_records());
    let resolve = resolve_request(1, WINNING_PAYOUT_INDEX);
    let resolve_post = f
        .held
        .gate(shared, &resolve, &window, true, actor, true)
        .expect("the oracle accepts the evidence-gated Resolve");
    let mut resolve_compares = state_compares(&f.held, &resolve_post);
    resolve_compares.push(compare_of(
        &f.held,
        "resolution",
        &resolve_post
            .resolution
            .expect("a resolve writes a resolution record"),
    ));
    cases.push(Case::accept(
        "resolve",
        "Resolve",
        "reference::apply_with_evidence",
        "one evidence-gated Resolve: the 0x47 buffer carries the sealed window, the feed head carries the matured cursor",
        gate_transaction(
            shared,
            &f.held,
            &f.resolve_buffer,
            actor,
            true,
            true,
            resolve.clone(),
        ),
        1,
        resolve_compares,
    ));

    cases.push(Case::refuse(
        "resolve-unsigned",
        "Resolve",
        "no authenticated signer; the gate checks the signature at the reference's point in the order, not hoisted",
        gate_transaction(
            shared,
            &f.held,
            &f.resolve_buffer,
            actor,
            false,
            true,
            resolve.clone(),
        ),
        code::REFERENCE_MISSING_SIGNATURE,
        refusal_text(
            f.held
                .gate(shared, &resolve, &window, true, actor, false)
                .expect_err("the oracle refuses an unsigned Resolve"),
        ),
    ));

    let wrong_payout = resolve_request(1, 0);
    cases.push(Case::refuse(
        "resolve-wrong-payout",
        "Resolve",
        "the request names payout zero while the sealed window selects payout one",
        gate_transaction(
            shared,
            &f.held,
            &f.resolve_buffer,
            actor,
            true,
            true,
            wrong_payout.clone(),
        ),
        code::PAYOUT_INDEX_MISMATCH,
        refusal_text(
            f.held
                .gate(shared, &wrong_payout, &window, true, actor, true)
                .expect_err("the oracle refuses a mis-named payout index"),
        ),
    ));

    /* ---------------------------------------------------------------- */
    /* RedeemInternal, on the resolved market                            */
    /* ---------------------------------------------------------------- */
    let redeem = redeem_request(2, WINNING_PAYOUT_INDEX, REDEEM_QUANTITY);
    let redeem_post = f
        .redeem
        .gate(shared, &redeem, &[], false, actor, true)
        .expect("the oracle accepts the RedeemInternal");
    assert_eq!(
        redeem_post.redemption_payout, REDEEM_QUANTITY,
        "the winning outcome pays one atom per claim"
    );
    let mut redeem_compares = state_compares(&f.redeem, &redeem_post);
    redeem_compares.push(compare_of(
        &f.redeem,
        "resolution",
        &redeem_post
            .resolution
            .expect("a redemption returns the record unchanged"),
    ));
    cases.push(Case::accept(
        "redeem",
        "RedeemInternal",
        "reference::apply_with_evidence",
        "redeem twenty atoms of the winning outcome against the recorded resolution; the record is presented read-only and must come back unchanged",
        gate_transaction(
            shared,
            &f.redeem,
            &f.redeem_buffer,
            actor,
            true,
            false,
            redeem.clone(),
        ),
        1,
        redeem_compares,
    ));

    cases.push(Case::refuse(
        "redeem-unsigned",
        "RedeemInternal",
        "the position owner is present, read-only, and never signed",
        gate_transaction(
            shared,
            &f.redeem,
            &f.redeem_buffer,
            actor,
            false,
            false,
            redeem.clone(),
        ),
        code::REFERENCE_MISSING_SIGNATURE,
        refusal_text(
            f.redeem
                .gate(shared, &redeem, &[], false, actor, false)
                .expect_err("the oracle refuses an unsigned redemption"),
        ),
    ));

    cases.push(Case::refuse(
        "redeem-stranger",
        "RedeemInternal",
        "a different authenticated signer redeems the owner's claims",
        gate_transaction(
            shared,
            &f.redeem,
            &f.redeem_buffer,
            shared.stranger.bytes,
            true,
            false,
            redeem.clone(),
        ),
        code::REFERENCE_UNAUTHORIZED_ACTOR,
        refusal_text(
            f.redeem
                .gate(shared, &redeem, &[], false, shared.stranger.bytes, true)
                .expect_err("the oracle refuses a stranger's redemption"),
        ),
    ));

    /* ---------------------------------------------------------------- */
    /* FeedAdvance                                                       */
    /* ---------------------------------------------------------------- */
    let advance = layout_request(
        0,
        Intent::FeedAdvance {
            feed: shared.advance_feed,
            cursor: END_BUCKET_EXCLUSIVE,
            evidence: Hash32::from_bytes([FEED_EVIDENCE_FILL; 32]),
        },
    );
    cases.push(Case::accept(
        "feed-advance",
        "FeedAdvance",
        "accumulator fold + FeedAccount codec",
        "fold one 0x48 observation page and move the feed cursor exactly across it",
        advance_transaction(shared, &f.page_buffer, actor, true, advance.clone()),
        1,
        vec![Compare {
            role: "advance.feed".to_string(),
            address: shared.advance_feed_head.address.clone(),
            expected: f.advanced_feed_bytes.to_vec(),
            pre: shared.advance_feed_bytes.to_vec(),
        }],
    ));

    cases.push(Case::refuse(
        "feed-advance-unsigned",
        "FeedAdvance",
        "no authenticated signer",
        advance_transaction(shared, &f.page_buffer, actor, false, advance.clone()),
        code::MISSING_SIGNATURE,
        "n/a (the offline adapter has no FeedAdvance)".to_string(),
    ));

    let replayed = layout_request(
        1,
        Intent::FeedAdvance {
            feed: shared.advance_feed,
            cursor: END_BUCKET_EXCLUSIVE,
            evidence: Hash32::from_bytes([FEED_EVIDENCE_FILL; 32]),
        },
    );
    cases.push(Case::refuse(
        "feed-advance-replay",
        "FeedAdvance",
        "the envelope names a page index the feed head has not reached; the page index is the feed's replay guard",
        advance_transaction(shared, &f.page_buffer, actor, true, replayed),
        code::REFERENCE_REPLAY,
        "n/a (the offline adapter has no FeedAdvance)".to_string(),
    ));

    /* ---------------------------------------------------------------- */
    /* CreateMarket                                                      */
    /* ---------------------------------------------------------------- */
    let create = layout_request(0, create_intent(shared, NONCE_CREATE));
    let mut create_compares: Vec<Compare> = f
        .create
        .state_roles()
        .iter()
        .map(|(role, _)| compare_of(&f.create, role, output_slice(&f.created, role)))
        .collect();
    create_compares.push(compare_of(&f.create, "resolution", &f.created_resolution));
    cases.push(Case::accept(
        "create-market",
        "CreateMarket",
        "layout re-encode + reference::validate_market_init",
        "found one market over eight pre-created, all-zero, canonically addressed accounts",
        create_transaction(shared, &f.create, actor, true, create.clone()),
        1,
        create_compares,
    ));

    cases.push(Case::refuse(
        "create-unsigned",
        "CreateMarket",
        "the creator is present, read-only, and never signed",
        create_transaction(shared, &f.create, actor, false, create.clone()),
        code::MISSING_SIGNATURE,
        "n/a (validate_market_init models no signer)".to_string(),
    ));

    /* The seam market's live accounts sit at exactly the canonical addresses a
     * `CreateMarket` for nonce 9 derives, and they are not zero. */
    let recreate = layout_request(0, create_intent(shared, NONCE_SEAM));
    cases.push(Case::refuse(
        "create-already-initialized",
        "CreateMarket",
        "re-found an existing market: every target account is at its canonical address and is not all-zero",
        create_transaction(shared, &f.seam, actor, true, recreate),
        code::ALREADY_INITIALIZED,
        refusal_text(
            validate_market_init(
                &shared.realm_bytes,
                &shared.profile_bytes,
                &shared.policy_bytes,
                &shared.terms_bytes,
                state_bytes(&f.seam.state),
                &create_intent_bytes(shared, NONCE_SEAM),
                &f.seam.metadata(shared, actor, true),
                &f.seam.bindings(shared),
            )
            .expect_err("the oracle refuses a re-initialization"),
        ),
    ));

    /* Three families do not fit the runtime's 200 000-unit default and carry a
     * `SetComputeUnitLimit` instruction ahead of the program instruction.
     * `gate_transaction` and `create_transaction` are the two builders that
     * emit it, so the marking is by family rather than by hand per case. */
    for case in &mut cases {
        if matches!(case.family, "Resolve" | "RedeemInternal" | "CreateMarket") {
            case.compute_limit = Some(COMPUTE_UNIT_CEILING);
        }
    }

    /* `Resolve` used to be marked `exhausted` here, MEASURED: five full
     * terms decodes -- one SHA-256 over the terms body each -- consumed every
     * unit the 1 400 000-unit transaction ceiling grants and the runtime
     * aborted it with `ProgramFailedToComplete`.  The decode-once rework
     * landed with the TermsAccount v3 revision (`accounts::read_terms` pays
     * the digest once in the account plane; every later gate read is
     * `TermsAccount::decode_unchecked`), the exhaustion gate went red as it
     * was designed to, and `resolve`/`resolve-wrong-payout` are ordinary
     * driven cases again.  The re-measured numbers live in the
     * SBF_BRINGUP.md resource envelope, re-recorded from `run_bringup.sh`'s
     * differential log on every regeneration. */

    cases
}

/* ------------------------------------------------------------------------ */
/* The lifecycle walk                                                        */
/* ------------------------------------------------------------------------ */

/* One market, walked end to end, as ONE gate.
 *
 * PROJECT.md section 10 asks for one reproducible local walk rather than ten
 * separately green instruction families.  This section is that walk: a single
 * ordered narrative over one market, from `CreateMarket` to a drained terminal
 * state whose accounting identity is asserted rather than eyeballed.
 *
 * ## Why the walk still needs several market planes
 *
 * `simulateTransaction` never commits, so one address can carry exactly one
 * pre-state in one genesis.  A chained walk of N steps therefore needs N
 * pre-states, and the only honest way to produce them is the one the rest of
 * this plan already uses: run the **offline reference adapter forward** from
 * the founding state and take step k's genesis to be the adapter's post-state
 * after steps 1..k-1.  Each step's SVM post-state is then compared byte for
 * byte against the adapter's post-state after step k, so the SVM and the
 * adapter walk the same trajectory and any divergence at any step fails the
 * whole walk.
 *
 * What differs between the planes is the market *identity* (one nonce per
 * step, so the addresses differ); everything else is the previous step's
 * reference post-state.  That is stated in `docs/implementation/
 * LIFECYCLE_WALK.md` rather than hidden, because a reader is owed the
 * difference between "the bank committed ten transactions in order" -- which
 * this is not -- and "ten pre-states, each of which is the previous step's
 * output, each executed by a real bank".
 *
 * The one place the bank itself sequences a chain is step 6, where three
 * `FeedAdvance` instructions ride in one transaction and the third reads the
 * second's writes.
 */

/// Market nonces of the walk, one per distinct pre-state.
const NONCE_WALK_FOUND: u64 = 20;
const NONCE_WALK_OPEN: u64 = 21;
const NONCE_WALK_SPLIT: u64 = 22;
const NONCE_WALK_MATERIALIZED: u64 = 23;
const NONCE_WALK_DEMATERIALIZED: u64 = 24;
const NONCE_WALK_MERGED: u64 = 25;
const NONCE_WALK_RESOLVED: u64 = 26;
const NONCE_WALK_REDEEMED: u64 = 27;

/// The walk's founding generation.  `CreateMarket` writes generation zero, and
/// the walk starts from exactly what `CreateMarket` wrote.
const WALK_GENERATION: u64 = 0;

/// Opening cash of the walk's position.
///
/// NAMED GAP, and the one field of the walk's opening state that no instruction
/// produced: nothing in this instruction set moves collateral into a Position,
/// so the walk credits its opening cash in the fixture.  Everything else in the
/// opening state is `CreateMarket`'s own post-state, and the harness asserts
/// exactly that (see `walk_opening_state_is_the_founding_state_plus_cash`).
const WALK_CASH: u64 = 64;

/// The walk's quantities.  Every terminal number is derived from these.
const WALK_SPLIT: u64 = 20;
const WALK_MATERIALIZE: u64 = 8;
const WALK_DEMATERIALIZE: u64 = 5;
const WALK_MERGE: u64 = 4;

/// The outcome the sealed window selects, and the one it does not.
///
/// `WINNING_PAYOUT_INDEX` is payout one, whose vector is the unit vector on
/// outcome one, so outcome one pays and outcome zero pays nothing.
const WALK_OUTCOME_WIN: u8 = 1;
const WALK_OUTCOME_LOSE: u8 = 0;

/// Internal winning claims left to redeem after the walk's materializations
/// and its pre-resolution merge.
const fn walk_redeem_winning() -> u64 {
    WALK_SPLIT - WALK_MATERIALIZE + WALK_DEMATERIALIZE - WALK_MERGE
}

/// Internal losing claims left to redeem, which pay zero.
const fn walk_redeem_losing() -> u64 {
    WALK_SPLIT - WALK_MERGE
}

/// Materialized winning claims the walk never brings back and never redeems.
///
/// There is no `RedeemExternal` instruction, so these stay outstanding, and the
/// Hoard must end holding exactly enough collateral to pay them.  That is the
/// section-10 item-10 identity this walk closes.
const fn walk_unredeemed_external() -> u64 {
    WALK_MATERIALIZE - WALK_DEMATERIALIZE
}

/// The three observation pages the walk folds into the shared feed.
///
/// Contiguous, and the last one lands the cursor exactly on the window's
/// maturity bound, which is the fact `Resolve` reads and no caller may assert.
const WALK_PAGE_BOUNDS: [(u64, u64); 3] = [
    (START_BUCKET, START_BUCKET + 2),
    (START_BUCKET + 2, END_BUCKET_EXCLUSIVE),
    (END_BUCKET_EXCLUSIVE, START_BUCKET + MATURITY_HORIZON),
];

/// Recorded page-evidence digests, one per advance; recorded, never believed.
const WALK_PAGE_EVIDENCE: [u8; 3] = [0x71, 0x72, 0x73];

/// One step of the walk as it is reported.
struct WalkStep {
    ordinal: u32,
    case: String,
    title: String,
    project_item: &'static str,
    narrative: String,
}

/// One PROJECT.md section-10 item the walk cannot drive, and why.
struct WalkSkip {
    project_item: &'static str,
    title: &'static str,
    reason: String,
}

/// One scalar the terminal accounting identity reads out of on-chain bytes.
struct TerminalValue {
    label: &'static str,
    role: String,
    offset: usize,
    width: usize,
    expected: u64,
}

/// One term of one identity: either an observed scalar times a weight, or a
/// constant the walk's own arithmetic fixes.
enum TerminalTerm {
    Observed { label: &'static str, scale: u64 },
    Constant { name: &'static str, value: u64 },
}

/// One accounting identity the terminal state must close.
struct TerminalIdentity {
    name: &'static str,
    equation: String,
    left: Vec<TerminalTerm>,
    right: Vec<TerminalTerm>,
}

/// The whole walk, as it is emitted into `plan.json`.
struct Lifecycle {
    steps: Vec<WalkStep>,
    skips: Vec<WalkSkip>,
    notes: Vec<String>,
    cases: Vec<Case>,
    terminal_case: String,
    values: Vec<TerminalValue>,
    identities: Vec<TerminalIdentity>,
}

/// Every plane and buffer the walk needs.
struct Walk {
    found: Plane,
    open: Plane,
    split: Plane,
    materialized: Plane,
    dematerialized: Plane,
    merged: Plane,
    resolved: Plane,
    redeemed: Plane,
    /// `CreateMarket`'s own post-state for the founding plane.
    founded: TransitionOutput,
    founded_resolution: [u8; account_len::RESOLUTION],
    pages: Vec<Pda>,
    page_bytes: Vec<Vec<u8>>,
    /// The feed head after all three advances, folded by the accumulator.
    advanced_feed_bytes: [u8; account_len::FEED],
    /// The reference post-state after the last step of the walk.
    terminal: TransitionOutput,
}

impl Walk {
    fn planes(&self) -> [&Plane; 8] {
        [
            &self.found,
            &self.open,
            &self.split,
            &self.materialized,
            &self.dematerialized,
            &self.merged,
            &self.resolved,
            &self.redeemed,
        ]
    }
}

/// The layout request the walk issues at `step`, on `plane`'s identity.
///
/// The replay sequence is the step index, because the walk starts from a
/// founding state whose replay account is at sequence zero and every step
/// consumes exactly one sequence.
fn walk_layout_request(plane: &Plane, step: usize) -> Vec<u8> {
    let market = plane.market_id;
    let owner = plane.owner;
    let sequence = step as u64;
    match step {
        0 => layout_request(
            sequence,
            Intent::Split {
                market,
                owner,
                quantity: WALK_SPLIT,
            },
        ),
        1 => layout_request(
            sequence,
            Intent::Materialize {
                market,
                owner,
                destination: Hash32::from_bytes(plane.external.bytes),
                outcome: WALK_OUTCOME_WIN,
                quantity: WALK_MATERIALIZE,
            },
        ),
        2 => layout_request(
            sequence,
            Intent::Dematerialize {
                market,
                owner,
                source: Hash32::from_bytes(plane.external.bytes),
                outcome: WALK_OUTCOME_WIN,
                quantity: WALK_DEMATERIALIZE,
            },
        ),
        3 => layout_request(
            sequence,
            Intent::Merge {
                market,
                owner,
                quantity: WALK_MERGE,
            },
        ),
        other => panic!("the walk has no layout step {other}"),
    }
}

/// Run the offline reference adapter forward over the first `steps` steps.
fn walk_forward(shared: &Shared, plane: &mut Plane, steps: usize) {
    let actor = shared.actor.bytes;
    let window = encode_window(shared.feed, &winning_records());
    for step in 0..steps {
        let output = match step {
            0..=3 => {
                let request = walk_layout_request(plane, step);
                plane.layout(shared, &request, actor, true)
            }
            4 => plane.gate(
                shared,
                &resolve_request(4, WINNING_PAYOUT_INDEX),
                &window,
                true,
                actor,
                true,
            ),
            5 => plane.gate(
                shared,
                &redeem_request(5, WALK_OUTCOME_WIN, walk_redeem_winning()),
                &[],
                false,
                actor,
                true,
            ),
            other => panic!("the walk has no step {other}"),
        }
        .unwrap_or_else(|error| panic!("the walk's step {step} must apply: {error:?}"));
        plane.advance(output);
    }
}

/// Build one walk plane: `CreateMarket`'s post-state, funded, walked forward.
fn walk_plane(shared: &Shared, label: &'static str, nonce: u64, steps: usize) -> Plane {
    let mut plane = Plane::build(shared, label, nonce, WALK_GENERATION);
    let (state, resolution) = founding_plane(shared, &plane, nonce);
    plane.state = state;
    plane.resolution_bytes = resolution;
    credit_walk_cash(&mut plane);
    walk_forward(shared, &mut plane, steps);
    plane
}

/// Credit the walk's opening cash into a founding position.
///
/// This is the walk's single fixture-written field; see [`WALK_CASH`].
fn credit_walk_cash(plane: &mut Plane) {
    let mut position =
        PositionAccount::decode(&plane.state.position).expect("the founding position must decode");
    assert_eq!(
        position.cash_atoms, 0,
        "a founding position must open with no cash"
    );
    position.cash_atoms = WALK_CASH;
    position
        .encode(&mut plane.state.position)
        .expect("the funded opening position must encode");
}

/// Fold the walk's observation pages onto the shared advance-feed head.
fn fold_walk_pages(shared: &Shared) -> [u8; account_len::FEED] {
    let mut feed =
        FeedAccount::decode(&shared.advance_feed_bytes).expect("the advance feed head decodes");
    for (index, (start, end)) in WALK_PAGE_BOUNDS.iter().enumerate() {
        let grid = Grid::new(GRID_FAMILY, GRID_VERSION, BUCKET_SECONDS).expect("the fixture grid");
        let mut summary = Summary::empty(grid);
        for bucket in *start..*end {
            summary = summary
                .append(Observation::accepted(bucket, 40, 41))
                .expect("the walk's page folds");
        }
        let cursor = summary
            .end_bucket_exclusive()
            .expect("a folded page has an end bucket");
        assert_eq!(
            feed.cursor, *start,
            "the walk's pages must be contiguous with the head they advance"
        );
        feed.cursor = cursor;
        feed.archive_pages += 1;
        feed.summary = Hash32::from_bytes([WALK_PAGE_EVIDENCE[index]; 32]);
    }
    assert_eq!(
        feed.cursor, FEED_CURSOR,
        "the walk's three advances must land the cursor exactly on the window's maturity bound"
    );
    let mut bytes = [0; account_len::FEED];
    feed.encode(&mut bytes).expect("the advanced feed head");
    bytes
}

/// One `FeedAdvance` observation page of the walk.
fn walk_page_bytes(shared: &Shared, index: usize) -> Vec<u8> {
    let (start, end) = WALK_PAGE_BOUNDS[index];
    let records: Vec<Record> = (start..end)
        .map(|bucket| (OBSERVATION_ACCEPTED, bucket, 40, 41))
        .collect();
    encode_feed_page(
        Hash32::from_bytes(shared.advance_feed.bytes()),
        start,
        end,
        &records,
    )
}

/// The transaction that carries all three of the walk's `FeedAdvance` steps.
///
/// One transaction, three instructions, one writable feed head: the **bank**
/// sequences them, so the second page is read against the first page's writes
/// and the third against the second's.  A chain the harness sequenced would
/// prove nothing about ordering; this one does.
fn walk_advance_transaction(shared: &Shared, walk: &Walk, actor: [u8; 32]) -> Vec<u8> {
    let writable = [shared.advance_feed_head.bytes];
    let mut readonly: Vec<[u8; 32]> = walk.pages.iter().map(|page| page.bytes).collect();
    readonly.push(shared.program.bytes);
    let message = Message::new(&[shared.payer.bytes], &[actor], &writable, &readonly);
    let instructions: Vec<Instruction> = walk
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| Instruction {
            program_index: message.index(&shared.program.bytes),
            accounts: message.indices(&[actor, shared.advance_feed_head.bytes, page.bytes]),
            data: layout_request(
                index as u64,
                Intent::FeedAdvance {
                    feed: shared.advance_feed,
                    cursor: WALK_PAGE_BOUNDS[index].1,
                    evidence: Hash32::from_bytes([WALK_PAGE_EVIDENCE[index]; 32]),
                },
            ),
        })
        .collect();
    transaction(&message, &instructions)
}

fn build_walk(shared: &Shared) -> Walk {
    let found = Plane::build(shared, "walk-found", NONCE_WALK_FOUND, WALK_GENERATION);
    let (founded, founded_resolution) = founding_plane(shared, &found, NONCE_WALK_FOUND);

    let open = walk_plane(shared, "walk-open", NONCE_WALK_OPEN, 0);
    let split = walk_plane(shared, "walk-split", NONCE_WALK_SPLIT, 1);
    let materialized = walk_plane(shared, "walk-materialized", NONCE_WALK_MATERIALIZED, 2);
    let dematerialized = walk_plane(shared, "walk-dematerialized", NONCE_WALK_DEMATERIALIZED, 3);
    let merged = walk_plane(shared, "walk-merged", NONCE_WALK_MERGED, 4);
    let resolved = walk_plane(shared, "walk-resolved", NONCE_WALK_RESOLVED, 5);
    let redeemed = walk_plane(shared, "walk-redeemed", NONCE_WALK_REDEEMED, 6);

    /* The last step's post-state, from the same forward run.  It is the walk's
     * terminal state and the thing the accounting identity is asserted over. */
    let terminal = redeemed
        .gate(
            shared,
            &redeem_request(6, WALK_OUTCOME_LOSE, walk_redeem_losing()),
            &[],
            false,
            shared.actor.bytes,
            true,
        )
        .expect("the walk's losing redemption must apply");
    assert_eq!(
        terminal.redemption_payout, 0,
        "a losing claim must pay exactly zero"
    );

    let pages: Vec<Pda> = (0..WALK_PAGE_BOUNDS.len())
        .map(|index| fixed_address(&format!("clutch/walk/page/{index}/v1")))
        .collect();
    let page_bytes: Vec<Vec<u8>> = (0..WALK_PAGE_BOUNDS.len())
        .map(|index| walk_page_bytes(shared, index))
        .collect();
    let advanced_feed_bytes = fold_walk_pages(shared);

    let walk = Walk {
        found,
        open,
        split,
        materialized,
        dematerialized,
        merged,
        resolved,
        redeemed,
        founded,
        founded_resolution,
        pages,
        page_bytes,
        advanced_feed_bytes,
        terminal,
    };
    assert_walk_chains(shared, &walk);
    walk
}

/// Every claim the walk makes about its own construction, checked here.
///
/// A walk that quietly stopped being a chain -- a plane whose genesis is not
/// the previous step's post-state, an opening state that is not what
/// `CreateMarket` writes, a feed that does not reach maturity -- is a build
/// failure rather than a green differential over a fiction.
fn assert_walk_chains(shared: &Shared, walk: &Walk) {
    /* The opening state is the founding state with exactly one field credited. */
    let (mut founded, founded_resolution) = founding_plane(shared, &walk.open, NONCE_WALK_OPEN);
    assert_eq!(
        walk.open.resolution_bytes, founded_resolution,
        "the walk opens on the resolution record CreateMarket writes"
    );
    for (role, _) in walk.open.state_roles() {
        if role == "position" {
            continue;
        }
        assert_eq!(
            walk.open.state_slice(role),
            output_slice(&founded, role),
            "the walk's opening {role} must be exactly what CreateMarket writes"
        );
    }
    let mut position =
        PositionAccount::decode(&founded.position).expect("the founding position decodes");
    position.cash_atoms = WALK_CASH;
    position
        .encode(&mut founded.position)
        .expect("the funded position encodes");
    assert_eq!(
        walk.open.state_slice("position"),
        founded.position.as_slice(),
        "the walk's opening position differs from CreateMarket's only in its cash"
    );

    /* Each plane is the previous plane's step replayed on its own identity. */
    let stages: [(&Plane, usize); 7] = [
        (&walk.open, 0),
        (&walk.split, 1),
        (&walk.materialized, 2),
        (&walk.dematerialized, 3),
        (&walk.merged, 4),
        (&walk.resolved, 5),
        (&walk.redeemed, 6),
    ];
    for (plane, steps) in stages {
        let replay = ReplayAccount::decode(&plane.state.replay).expect("a replay account decodes");
        assert_eq!(
            replay.sequence, steps as u64,
            "{}'s genesis must sit at replay sequence {steps}",
            plane.label
        );
    }

    /* The kernel and the market agree about resolution from the resolve on. */
    let resolved_kernel =
        KernelAccount::decode(&walk.resolved.state.kernel).expect("the kernel decodes");
    assert_eq!(
        resolved_kernel.phase, 1,
        "the walk resolves before it redeems"
    );
    assert_eq!(
        resolved_kernel.resolved_payout, WINNING_PAYOUT_INDEX,
        "the walk resolves onto the payout the sealed window selects"
    );
}

/* ---- The terminal accounting identity --------------------------------- */

/// The sole byte index at which two encodings of one account differ.
fn sole_difference(base: &[u8], probe: &[u8], label: &str) -> usize {
    assert_eq!(
        base.len(),
        probe.len(),
        "{label}: the probe changed the length"
    );
    let mut found = None;
    for (index, (left, right)) in base.iter().zip(probe.iter()).enumerate() {
        if left != right {
            assert!(
                found.is_none(),
                "{label}: the probe moved more than one byte"
            );
            found = Some(index);
        }
    }
    found.unwrap_or_else(|| panic!("{label}: the probe moved no byte"))
}

/// Locate one little-endian `u64` field inside a frozen account encoding.
///
/// Nothing here hard-codes an offset.  The field is written twice -- once as
/// `1` and once as `256` -- and the offset is where the encoding moved: at
/// `offset` for the first and at `offset + 1` for the second, which is what
/// little-endian means and what the assertion below refuses to assume.
fn u64_field_offset<F>(encode: F, label: &str) -> usize
where
    F: Fn(u64) -> Vec<u8>,
{
    let zero = encode(0);
    let low = sole_difference(&zero, &encode(1), label);
    let high = sole_difference(&zero, &encode(256), label);
    assert_eq!(
        high,
        low + 1,
        "{label}: the field is not a little-endian u64 at {low}"
    );
    low
}

/// Build one terminal readout from an account whose codec is probed for the
/// field's offset.
fn terminal_value<F>(label: &'static str, role: String, expected: u64, encode: F) -> TerminalValue
where
    F: Fn(u64) -> Vec<u8>,
{
    TerminalValue {
        label,
        offset: u64_field_offset(&encode, label),
        role,
        width: 8,
        expected,
    }
}

fn observed(label: &'static str, scale: u64) -> TerminalTerm {
    TerminalTerm::Observed { label, scale }
}

fn constant(name: &'static str, value: u64) -> TerminalTerm {
    TerminalTerm::Constant { name, value }
}

/// Read the walk's terminal state out and state the identities it closes.
///
/// Every expected number below is *derived*: either from the walk's own
/// quantities (`WALK_SPLIT` and friends), or by decoding the offline reference
/// adapter's terminal post-state.  Nothing is transcribed from an observed run,
/// and the two derivations are required to agree here, at build time, before a
/// validator is ever started.
fn walk_terminal(walk: &Walk) -> (Vec<TerminalValue>, Vec<TerminalIdentity>) {
    let terminal = &walk.terminal;
    let label = walk.redeemed.label;
    let hoard = HoardAccount::decode(&terminal.hoard).expect("terminal hoard decodes");
    let position = PositionAccount::decode(&terminal.position).expect("terminal position decodes");
    let kernel = KernelAccount::decode(&terminal.kernel).expect("terminal kernel decodes");
    let external = ExternalAccount::decode(&terminal.external).expect("terminal external decodes");
    let supply =
        SupplyLedgerAccount::decode(&terminal.supply).expect("terminal supply ledger decodes");

    /* The walk's own arithmetic, independent of the adapter's answer. */
    let expected_hoard = walk_unredeemed_external();
    let expected_cash = WALK_CASH - WALK_SPLIT + WALK_MERGE + walk_redeem_winning();
    let expected_external_win = walk_unredeemed_external();

    assert_eq!(
        hoard.collateral_atoms, expected_hoard,
        "the terminal Hoard must hold exactly the unredeemed obligations"
    );
    assert_eq!(
        position.cash_atoms, expected_cash,
        "the terminal cash must be the walk's arithmetic"
    );
    assert_eq!(
        position.internal, [0; MAX_OUTCOMES],
        "every internal claim must be merged or redeemed away"
    );
    assert_eq!(
        external.balances[usize::from(WALK_OUTCOME_WIN)],
        expected_external_win,
        "the materialized winning claims are the only thing left outstanding"
    );
    assert_eq!(
        external.balances[usize::from(WALK_OUTCOME_LOSE)],
        0,
        "the walk materialized nothing on the losing outcome"
    );
    assert_eq!(
        kernel.total_supply[usize::from(WALK_OUTCOME_LOSE)],
        0,
        "the losing outcome's supply must be fully redeemed away"
    );
    assert_eq!(
        kernel.total_supply[usize::from(WALK_OUTCOME_WIN)],
        expected_external_win,
        "the winning outcome's remaining supply is exactly the external claims"
    );
    assert_eq!(
        supply.internal_supply, [0; MAX_OUTCOMES],
        "the internal term of the supply ledger must drain with the position"
    );
    assert_eq!(
        supply.external_supply[usize::from(WALK_OUTCOME_WIN)],
        expected_external_win,
        "the external term of the supply ledger must carry the outstanding claims"
    );

    /* The resolved payout vector, read out of the terminal kernel rather than
     * retyped: it is what turns a claim count into a collateral obligation. */
    let resolved = usize::from(kernel.resolved_payout);
    assert!(
        resolved < usize::from(kernel.payouts.count),
        "the terminal kernel resolved onto a payout index its own set does not hold"
    );
    let vector = kernel.payouts.vectors[resolved];
    assert_eq!(
        vector.denominator, 1,
        "this walk's payout vector is integral; a fractional one would need a divisor here"
    );

    let hoard_role = format!("{label}.hoard");
    let position_role = format!("{label}.position");
    let kernel_role = format!("{label}.kernel");
    let external_role = format!("{label}.external");
    let supply_role = format!("{label}.supply");

    let mut values = vec![
        terminal_value(
            "hoard_collateral",
            hoard_role,
            hoard.collateral_atoms,
            |v| {
                let mut probe = hoard;
                probe.collateral_atoms = v;
                let mut bytes = [0; account_len::HOARD];
                probe.encode(&mut bytes).expect("hoard probe encodes");
                bytes.to_vec()
            },
        ),
        terminal_value(
            "position_cash",
            position_role.clone(),
            position.cash_atoms,
            |v| {
                let mut probe = position;
                probe.cash_atoms = v;
                let mut bytes = [0; account_len::POSITION];
                probe.encode(&mut bytes).expect("position probe encodes");
                bytes.to_vec()
            },
        ),
    ];
    let outcomes = usize::from(OUTCOME_COUNT);
    const INTERNAL: [&str; 2] = ["position_internal_0", "position_internal_1"];
    const TOTAL: [&str; 2] = ["kernel_total_supply_0", "kernel_total_supply_1"];
    const EXTERNAL: [&str; 2] = ["external_balance_0", "external_balance_1"];
    const LEDGER_INTERNAL: [&str; 2] = ["ledger_internal_0", "ledger_internal_1"];
    const LEDGER_EXTERNAL: [&str; 2] = ["ledger_external_0", "ledger_external_1"];
    for index in 0..outcomes {
        values.push(terminal_value(
            INTERNAL[index],
            position_role.clone(),
            position.internal[index],
            |v| {
                let mut probe = position;
                probe.internal[index] = v;
                let mut bytes = [0; account_len::POSITION];
                probe.encode(&mut bytes).expect("position probe encodes");
                bytes.to_vec()
            },
        ));
        values.push(terminal_value(
            TOTAL[index],
            kernel_role.clone(),
            kernel.total_supply[index],
            |v| {
                let mut probe = kernel;
                probe.total_supply[index] = v;
                let mut bytes = [0; KERNEL_ACCOUNT_LEN];
                probe.encode(&mut bytes).expect("kernel probe encodes");
                bytes.to_vec()
            },
        ));
        values.push(terminal_value(
            EXTERNAL[index],
            external_role.clone(),
            external.balances[index],
            |v| {
                let mut probe = external;
                probe.balances[index] = v;
                let mut bytes = [0; EXTERNAL_ACCOUNT_LEN];
                probe.encode(&mut bytes).expect("external probe encodes");
                bytes.to_vec()
            },
        ));
        values.push(terminal_value(
            LEDGER_INTERNAL[index],
            supply_role.clone(),
            supply.internal_supply[index],
            |v| {
                let mut probe = supply;
                probe.internal_supply[index] = v;
                let mut bytes = [0; account_len::SUPPLY_LEDGER];
                probe.encode(&mut bytes).expect("ledger probe encodes");
                bytes.to_vec()
            },
        ));
        values.push(terminal_value(
            LEDGER_EXTERNAL[index],
            supply_role.clone(),
            supply.external_supply[index],
            |v| {
                let mut probe = supply;
                probe.external_supply[index] = v;
                let mut bytes = [0; account_len::SUPPLY_LEDGER];
                probe.encode(&mut bytes).expect("ledger probe encodes");
                bytes.to_vec()
            },
        ));
    }

    let mut identities = vec![
        TerminalIdentity {
            name: "collateral conservation",
            equation: format!(
                "opening_cash ({WALK_CASH}) == position_cash + hoard_collateral"
            ),
            left: vec![constant("opening_cash", WALK_CASH)],
            right: vec![observed("position_cash", 1), observed("hoard_collateral", 1)],
        },
        TerminalIdentity {
            name: "the Hoard covers exactly the unredeemed obligations",
            equation: format!(
                "hoard_collateral == sum_i payout_weight[{}][i] * kernel_total_supply_i (denominator 1)",
                kernel.resolved_payout
            ),
            left: vec![observed("hoard_collateral", 1)],
            right: (0..outcomes)
                .map(|index| observed(TOTAL[index], vector.weights[index]))
                .collect(),
        },
        TerminalIdentity {
            name: "the internal ledger drains to zero",
            equation: "0 == sum_i position_internal_i".to_string(),
            left: vec![constant("zero", 0)],
            right: (0..outcomes).map(|index| observed(INTERNAL[index], 1)).collect(),
        },
        TerminalIdentity {
            name: "the kernel supply is exactly the outstanding external claims",
            equation: "sum_i kernel_total_supply_i == sum_i external_balance_i".to_string(),
            left: (0..outcomes).map(|index| observed(TOTAL[index], 1)).collect(),
            right: (0..outcomes).map(|index| observed(EXTERNAL[index], 1)).collect(),
        },
    ];
    for index in 0..outcomes {
        identities.push(TerminalIdentity {
            name: "the supply ledger closes over outcome",
            equation: format!(
                "kernel_total_supply_{index} == ledger_internal_{index} + ledger_external_{index}"
            ),
            left: vec![observed(TOTAL[index], 1)],
            right: vec![
                observed(LEDGER_INTERNAL[index], 1),
                observed(LEDGER_EXTERNAL[index], 1),
            ],
        });
    }

    /* Every identity must already hold over the derived numbers.  A gate that
     * only checks these on-chain would let a wrong expectation ship. */
    for identity in &identities {
        let sum = |terms: &Vec<TerminalTerm>| -> u128 {
            terms
                .iter()
                .map(|term| match term {
                    TerminalTerm::Observed { label, scale } => {
                        let value = values
                            .iter()
                            .find(|entry| entry.label == *label)
                            .unwrap_or_else(|| panic!("no terminal value named {label}"));
                        u128::from(value.expected) * u128::from(*scale)
                    }
                    TerminalTerm::Constant { value, .. } => u128::from(*value),
                })
                .sum()
        };
        assert_eq!(
            sum(&identity.left),
            sum(&identity.right),
            "the walk's terminal state does not close `{}`",
            identity.equation
        );
    }

    (values, identities)
}

/* ---- The walk's transactions ------------------------------------------ */

fn walk_step(
    ordinal: u32,
    case: &str,
    title: &str,
    project_item: &'static str,
    narrative: &str,
) -> WalkStep {
    WalkStep {
        ordinal,
        case: case.to_string(),
        title: title.to_string(),
        project_item,
        narrative: narrative.to_string(),
    }
}

fn build_lifecycle(f: &Fixture) -> Lifecycle {
    let shared = &f.shared;
    let walk = &f.walk;
    let actor = shared.actor.bytes;
    let mut cases = Vec::new();
    let mut steps = Vec::new();

    /* 1. CreateMarket, over eight all-zero canonically addressed accounts. */
    let create = layout_request(0, create_intent(shared, NONCE_WALK_FOUND));
    let mut create_compares: Vec<Compare> = walk
        .found
        .state_roles()
        .iter()
        .map(|(role, _)| compare_of(&walk.found, role, output_slice(&walk.founded, role)))
        .collect();
    create_compares.push(compare_of(
        &walk.found,
        "resolution",
        &walk.founded_resolution,
    ));
    let mut case = Case::accept(
        "walk-01-create-market",
        "Lifecycle",
        "layout re-encode + reference::validate_market_init",
        "found the walk's market: the collateral cap, payout set, and feed all come from the immutable terms artifact and nothing else",
        create_transaction(shared, &walk.found, actor, true, create),
        1,
        create_compares,
    );
    case.compute_limit = Some(COMPUTE_UNIT_CEILING);
    cases.push(case);
    steps.push(walk_step(
        1,
        "walk-01-create-market",
        "create the market",
        "1",
        "Eight all-zero accounts at their canonical addresses become one active market. The collateral cap, the outcome basis, the payout set, and the feed identity are read out of the frozen terms artifact; the instruction chooses none of them.",
    ));

    /* 2. Split. */
    let split = walk_layout_request(&walk.open, 0);
    let split_post = walk
        .open
        .layout(shared, &split, actor, true)
        .expect("the walk splits");
    let message = seam_message(shared, &walk.open, actor, true, None);
    cases.push(Case::accept(
        "walk-02-split",
        "Lifecycle",
        "reference::apply",
        "split the funded position into complete sets",
        transaction(
            &message,
            &[seam_instruction(
                &message, shared, &walk.open, actor, None, split,
            )],
        ),
        1,
        state_compares(&walk.open, &split_post),
    ));
    steps.push(walk_step(
        2,
        "walk-02-split",
        "split internally",
        "4",
        "One collateral debit credits one unit of every Egg. No mint CPI, no token account: the complete set lives in the position's internal balances and the Hoard's collateral rises by exactly the quantity split.",
    ));

    /* 3. Materialize. */
    let materialize = walk_layout_request(&walk.split, 1);
    let materialize_post = walk
        .split
        .layout(shared, &materialize, actor, true)
        .expect("the walk materializes");
    let message = seam_message(shared, &walk.split, actor, true, None);
    cases.push(Case::accept(
        "walk-03-materialize",
        "Lifecycle",
        "reference::apply",
        "materialize part of the winning outcome into the external shadow",
        transaction(
            &message,
            &[seam_instruction(
                &message,
                shared,
                &walk.split,
                actor,
                None,
                materialize,
            )],
        ),
        1,
        state_compares(&walk.split, &materialize_post),
    ));
    steps.push(walk_step(
        3,
        "walk-03-materialize",
        "materialize one Egg",
        "5",
        "Part of one outcome leaves the internal ledger for the external shadow. `total_i` is preserved exactly: what the position loses internally the shadow gains, and the Hoard does not move.",
    ));

    /* 4. Dematerialize. */
    let dematerialize = walk_layout_request(&walk.materialized, 2);
    let dematerialize_post = walk
        .materialized
        .layout(shared, &dematerialize, actor, true)
        .expect("the walk dematerializes");
    let message = seam_message(shared, &walk.materialized, actor, true, None);
    cases.push(Case::accept(
        "walk-04-dematerialize",
        "Lifecycle",
        "reference::apply",
        "bring part of the materialized outcome back to the internal ledger",
        transaction(
            &message,
            &[seam_instruction(
                &message,
                shared,
                &walk.materialized,
                actor,
                None,
                dematerialize,
            )],
        ),
        1,
        state_compares(&walk.materialized, &dematerialize_post),
    ));
    steps.push(walk_step(
        4,
        "walk-04-dematerialize",
        "dematerialize part of it",
        "5",
        "The reverse boundary crossing, for part of what was materialized. The remainder stays outstanding on the external side for the rest of the walk and is what the terminal Hoard has to cover.",
    ));

    /* 5. Merge, while the market is still active. */
    let merge = walk_layout_request(&walk.dematerialized, 3);
    let merge_post = walk
        .dematerialized
        .layout(shared, &merge, actor, true)
        .expect("the walk merges");
    let message = seam_message(shared, &walk.dematerialized, actor, true, None);
    cases.push(Case::accept(
        "walk-05-merge",
        "Lifecycle",
        "reference::apply",
        "recombine complete sets into cash before resolution",
        transaction(
            &message,
            &[seam_instruction(
                &message,
                shared,
                &walk.dematerialized,
                actor,
                None,
                merge,
            )],
        ),
        1,
        state_compares(&walk.dematerialized, &merge_post),
    ));
    steps.push(walk_step(
        5,
        "walk-05-merge",
        "merge complete sets back",
        "4",
        "The promise of section 1 exercised: a complete set can always be recombined into its collateral **before** resolution. Cash rises and the Hoard falls by the same quantity. Step 8 records what happens to the same request after resolution.",
    ));

    /* 6. Three FeedAdvance instructions, sequenced by the bank. */
    cases.push(Case::accept(
        "walk-06-feed-advance",
        "Lifecycle",
        "accumulator fold + FeedAccount codec",
        "three contiguous observation pages in one transaction; the bank sequences them and the cursor lands exactly on the window's maturity bound",
        walk_advance_transaction(shared, walk, actor),
        WALK_PAGE_BOUNDS.len(),
        vec![Compare {
            role: "walk.feed".to_string(),
            address: shared.advance_feed_head.address.clone(),
            expected: walk.advanced_feed_bytes.to_vec(),
            pre: shared.advance_feed_bytes.to_vec(),
        }],
    ));
    steps.push(walk_step(
        6,
        "walk-06-feed-advance",
        "advance the shared feed three times",
        "6, 7",
        "Three observation pages fold into one feed head inside one transaction, so the bank -- not this harness -- sequences the chain and page three is read against page two's writes. The cursor moves 100 -> 102 -> 103 -> 104, and 104 is exactly the maturity bound the market's window needs before it can seal.",
    ));

    /* 7. Resolve. */
    let window = encode_window(shared.feed, &winning_records());
    let resolve = resolve_request(4, WINNING_PAYOUT_INDEX);
    let resolve_post = walk
        .merged
        .gate(shared, &resolve, &window, true, actor, true)
        .expect("the walk resolves");
    let mut resolve_compares = state_compares(&walk.merged, &resolve_post);
    resolve_compares.push(compare_of(
        &walk.merged,
        "resolution",
        &resolve_post
            .resolution
            .expect("a resolve writes a resolution record"),
    ));
    let mut case = Case::accept(
        "walk-07-resolve",
        "Lifecycle",
        "reference::apply_with_evidence",
        "seal the window from the observation records and resolve onto the cell they select",
        gate_transaction(
            shared,
            &walk.merged,
            &f.resolve_buffer,
            actor,
            true,
            true,
            resolve.clone(),
        ),
        1,
        resolve_compares,
    );
    case.compute_limit = Some(COMPUTE_UNIT_CEILING);
    cases.push(case);
    steps.push(walk_step(
        7,
        "walk-07-resolve",
        "seal the window and resolve",
        "7, 9",
        "No reporter chooses the cell. The buffer carries observation records; the gate folds them through the accumulator's Open -> Mature -> Sealed machine against the terms' own window domain, reads the matured cursor off the feed head, and the payout index the caller named must be the one the sealed window selects.",
    ));

    /* 8. Merge after resolution: refused, and that is the point. */
    let late_merge = layout_request(
        5,
        Intent::Merge {
            market: walk.resolved.market_id,
            owner: walk.resolved.owner,
            quantity: walk_redeem_winning(),
        },
    );
    let late_refusal = walk
        .resolved
        .layout(shared, &late_merge, actor, true)
        .expect_err("the oracle refuses a merge on a resolved market");
    /* The two implementations refuse the same request and name it differently,
     * and that difference is documented rather than papered over: this program
     * refines the reference's generic `MismatchedState` into `NotActive`.  The
     * assertion pins the reference half so a drift on either side is loud. */
    assert_eq!(
        shared_class_code(late_refusal),
        0x300e,
        "the offline reference must refuse a post-resolution merge as MismatchedState"
    );
    let message = seam_message(shared, &walk.resolved, actor, true, None);
    cases.push(Case::refuse(
        "walk-08-merge-after-resolve",
        "Lifecycle",
        "recombine a complete set after resolution: the boundary section 1 draws, driven",
        transaction(
            &message,
            &[seam_instruction(
                &message,
                shared,
                &walk.resolved,
                actor,
                None,
                late_merge,
            )],
        ),
        code::NOT_ACTIVE,
        refusal_text(late_refusal),
    ));
    steps.push(walk_step(
        8,
        "walk-08-merge-after-resolve",
        "merge after resolution is refused",
        "4, 10",
        "The same complete-set merge that step 5 accepted is refused once the market has resolved. This is the boundary the product model draws -- recombination is a pre-resolution right -- and after this point the only way out of a claim is redemption, which is what makes the terminal accounting a redemption identity rather than a merge identity.",
    ));

    /* 9. Redeem the winning internal claims. */
    let redeem_win = redeem_request(5, WALK_OUTCOME_WIN, walk_redeem_winning());
    let redeem_win_post = walk
        .resolved
        .gate(shared, &redeem_win, &[], false, actor, true)
        .expect("the walk redeems its winning claims");
    assert_eq!(
        redeem_win_post.redemption_payout,
        walk_redeem_winning(),
        "the winning outcome pays one atom per claim"
    );
    let mut redeem_win_compares = state_compares(&walk.resolved, &redeem_win_post);
    redeem_win_compares.push(compare_of(
        &walk.resolved,
        "resolution",
        &redeem_win_post
            .resolution
            .expect("a redemption returns the record unchanged"),
    ));
    let mut case = Case::accept(
        "walk-09-redeem-winning",
        "Lifecycle",
        "reference::apply_with_evidence",
        "redeem every internal claim on the winning outcome; the resolution record is read-only and must come back unchanged",
        gate_transaction(
            shared,
            &walk.resolved,
            &f.redeem_buffer,
            actor,
            true,
            false,
            redeem_win,
        ),
        1,
        redeem_win_compares,
    );
    case.compute_limit = Some(COMPUTE_UNIT_CEILING);
    cases.push(case);
    steps.push(walk_step(
        9,
        "walk-09-redeem-winning",
        "redeem the winning internal claims",
        "9",
        "The first payoff shape: the unit vector on the realized cell. Collateral leaves the Hoard for the position's cash, one atom per claim, and the resolution record the redemption reads is presented read-only so a redemption can never edit its own authority.",
    ));

    /* 10. Redeem the losing claims: they pay zero. */
    let redeem_lose = redeem_request(6, WALK_OUTCOME_LOSE, walk_redeem_losing());
    let mut redeem_lose_compares = state_compares(&walk.redeemed, &walk.terminal);
    redeem_lose_compares.push(compare_of(
        &walk.redeemed,
        "resolution",
        &walk
            .terminal
            .resolution
            .expect("a redemption returns the record unchanged"),
    ));
    let mut case = Case::accept(
        "walk-10-redeem-losing",
        "Lifecycle",
        "reference::apply_with_evidence",
        "redeem every internal claim on the losing outcome; the claims burn and the payout is exactly zero",
        gate_transaction(
            shared,
            &walk.redeemed,
            &f.redeem_buffer,
            actor,
            true,
            false,
            redeem_lose,
        ),
        1,
        redeem_lose_compares,
    );
    case.compute_limit = Some(COMPUTE_UNIT_CEILING);
    cases.push(case);
    steps.push(walk_step(
        10,
        "walk-10-redeem-losing",
        "redeem the losing claims for zero",
        "9, 10",
        "The second payoff shape: the zero vector on an unrealized cell. The claims are burned and the Hoard does not move by one atom, which is the half of the solvency promise that is easy to state and easy to get wrong. After this the walk's internal ledger is empty and the terminal identity can be read.",
    ));

    let (values, identities) = walk_terminal(walk);

    let skips = vec![
        WalkSkip {
            project_item: "1 (in part)",
            title: "initialize a Realm",
            reason: "SKIPPED: there is no Realm, Profile, price-grid, or terms initialization instruction in this program. The walk drives `CreateMarket` and nothing else of item 1; the Realm-wide plane is loaded at genesis as frozen bytes the frozen codecs accept."
                .to_string(),
        },
        WalkSkip {
            project_item: "2",
            title: "prepay all mandatory work",
            reason: "SKIPPED: no endowment, prepayment, or deposit instruction exists, and nothing in this instruction set moves collateral into a Position. The walk credits its opening cash in the fixture, which is the one field of the opening state `CreateMarket` did not write."
                .to_string(),
        },
        WalkSkip {
            project_item: "3",
            title: "compile and prove one exhaustive state partition",
            reason: "NOTED, not driven: the immutable terms artifact **is** the compiled partition -- outcome count, payout vectors, payout map, knots, statistic, edge and ambiguity policies -- frozen into one digest the market binds and the resolve gate re-reads. There is no on-chain compiler instruction and none is claimed; the walk's step 1 consumes the artifact rather than producing it."
                .to_string(),
        },
        WalkSkip {
            project_item: "8",
            title: "clear one coupled simplex batch with portfolio intents",
            reason: "SKIPPED for two independent reasons: `PlaceOrder` has no SVM oracle -- the offline reference adapter models no order family, so there is no second implementation for a differential to disagree with -- and settlement awaits the streaming verifier on-chain. The batch-auction plane is loaded at genesis and no implemented instruction transacts against it."
                .to_string(),
        },
        WalkSkip {
            project_item: "11",
            title: "reproduce in the Rocq model, the Verus-verified kernel, and the SBF harness",
            reason: "OUT OF SCOPE for this walk per the standing deprioritization. This walk is the SBF-harness leg alone; it makes no claim about the other two legs and is not a triple reproduction."
                .to_string(),
        },
    ];

    let notes = vec![
        "`simulateTransaction` never commits, so one address carries one pre-state per genesis. Each step of the walk therefore runs on its own market plane, whose genesis is the offline reference adapter's post-state after every earlier step. Only the market identity differs between planes; the state trajectory is one chain, and the harness asserts the chaining at build time."
            .to_string(),
        "Step 6 is the exception: three `FeedAdvance` instructions ride in one transaction against one writable feed head, so the bank sequences that chain itself."
            .to_string(),
        "The feed head step 6 advances and the feed head step 7 resolves against are two accounts of two feed identities, because the same address cannot hold both cursor 100 and cursor 104 in one genesis. The harness asserts that step 6's three advances land the cursor on exactly the value step 7's head carries, which is the only fact the resolve gate reads off a feed head."
            .to_string(),
        "No signature is verified anywhere in this walk: every transaction is simulated with `sigVerify: false`. The `is_signer` bits the program reads do come from the transaction message header, and that is the whole of what the authorization steps establish."
            .to_string(),
    ];

    Lifecycle {
        steps,
        skips,
        notes,
        cases,
        terminal_case: "walk-10-redeem-losing".to_string(),
        values,
        identities,
    }
}

impl Plane {
    /// A shallow copy of one plane at a different state, for chained oracles.
    fn clone_state(source: &Plane, state: &TransitionOutput) -> Plane {
        Plane {
            label: source.label,
            market_id: source.market_id,
            owner: source.owner,
            generation: source.generation,
            market: source.market.clone(),
            hoard: source.hoard.clone(),
            position: source.position.clone(),
            kernel: source.kernel.clone(),
            external: source.external.clone(),
            replay: source.replay.clone(),
            supply: source.supply.clone(),
            resolution: source.resolution.clone(),
            state: state.clone(),
            resolution_bytes: source.resolution_bytes,
        }
    }
}

/* ------------------------------------------------------------------------ */
/* Output                                                                    */
/* ------------------------------------------------------------------------ */

fn write(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

fn account_json(pubkey: &str, owner: &str, data: &[u8]) -> String {
    format!(
        "{{\n  \"pubkey\": \"{pubkey}\",\n  \"account\": {{\n    \"lamports\": {ACCOUNT_LAMPORTS},\n    \"data\": [\n      \"{}\",\n      \"base64\"\n    ],\n    \"owner\": \"{owner}\",\n    \"executable\": false,\n    \"rentEpoch\": 0,\n    \"space\": {}\n  }}\n}}\n",
        b64_encode(data),
        data.len()
    )
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn json_list(items: &[String]) -> String {
    format!("[{}]", items.join(", "))
}

/// Serialize the lifecycle walk into the plan.
///
/// The walk is emitted as its own block rather than mixed into `cases`, so the
/// existing per-family bring-up gate runs exactly what it ran before and the
/// walk is one separate, ordered, all-or-nothing gate.
fn lifecycle_json(walk: &Lifecycle, cases: &[String]) -> String {
    let steps: Vec<String> = walk
        .steps
        .iter()
        .map(|step| {
            format!(
                "      {{\"ordinal\": {}, \"case\": {}, \"title\": {}, \"project_item\": {}, \"narrative\": {}}}",
                step.ordinal,
                json_string(&step.case),
                json_string(&step.title),
                json_string(step.project_item),
                json_string(&step.narrative)
            )
        })
        .collect();
    let skips: Vec<String> = walk
        .skips
        .iter()
        .map(|skip| {
            format!(
                "      {{\"project_item\": {}, \"title\": {}, \"reason\": {}}}",
                json_string(skip.project_item),
                json_string(skip.title),
                json_string(&skip.reason)
            )
        })
        .collect();
    let notes: Vec<String> = walk
        .notes
        .iter()
        .map(|note| format!("      {}", json_string(note)))
        .collect();
    let values: Vec<String> = walk
        .values
        .iter()
        .map(|value| {
            format!(
                "        {{\"label\": {}, \"role\": {}, \"offset\": {}, \"width\": {}, \"expected\": {}}}",
                json_string(value.label),
                json_string(&value.role),
                value.offset,
                value.width,
                value.expected
            )
        })
        .collect();
    let term = |term: &TerminalTerm| match term {
        TerminalTerm::Observed { label, scale } => {
            format!("{{\"label\": {}, \"scale\": {scale}}}", json_string(label))
        }
        TerminalTerm::Constant { name, value } => {
            format!(
                "{{\"constant\": {}, \"value\": {value}}}",
                json_string(name)
            )
        }
    };
    let identities: Vec<String> = walk
        .identities
        .iter()
        .map(|identity| {
            format!(
                "        {{\"name\": {}, \"equation\": {}, \"left\": [{}], \"right\": [{}]}}",
                json_string(identity.name),
                json_string(&identity.equation),
                identity
                    .left
                    .iter()
                    .map(term)
                    .collect::<Vec<_>>()
                    .join(", "),
                identity
                    .right
                    .iter()
                    .map(term)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect();
    format!(
        "{{\n    \"steps\": [\n{}\n    ],\n    \"skipped\": [\n{}\n    ],\n    \"notes\": [\n{}\n    ],\n    \"terminal\": {{\n      \"case\": {},\n      \"values\": [\n{}\n      ],\n      \"identities\": [\n{}\n      ]\n    }},\n    \"cases\": [\n{}\n    ]\n  }}",
        steps.join(",\n"),
        skips.join(",\n"),
        notes.join(",\n"),
        json_string(&walk.terminal_case),
        values.join(",\n"),
        identities.join(",\n"),
        cases.join(",\n")
    )
}

/// Write every transaction and expectation of one case list, and return the
/// JSON object each case emits into the plan.
fn emit_cases(out_dir: &Path, cases: &[Case]) -> Vec<String> {
    let mut case_json = Vec::new();
    for case in cases {
        let tx_file = format!("tx/{}.b64", case.name);
        write(
            &out_dir.join(&tx_file),
            &format!("{}\n", b64_encode(&case.tx)),
        );
        let mut fields = vec![
            format!("\"name\": {}", json_string(&case.name)),
            format!("\"family\": {}", json_string(case.family)),
            format!("\"oracle\": {}", json_string(case.oracle)),
            format!("\"note\": {}", json_string(&case.note)),
            format!("\"tx\": {}", json_string(&tx_file)),
            format!("\"instructions\": {}", case.instruction_count),
            format!("\"bytes\": {}", case.tx.len()),
            match case.compute_limit {
                Some(limit) => format!("\"compute_limit\": {limit}"),
                None => "\"compute_limit\": null".to_string(),
            },
        ];
        let kind = if case.exhausted {
            "exhausted"
        } else {
            "accept"
        };
        match (&case.compare, case.expect_code) {
            (Some(compares), _) => {
                let mut entries = Vec::new();
                for compare in compares {
                    let expected_file = format!("expected/{}.{}.hex", case.name, compare.role);
                    let pre_file = format!("expected/{}.{}.pre.hex", case.name, compare.role);
                    write(
                        &out_dir.join(&expected_file),
                        &format!("{}\n", hex_encode(&compare.expected)),
                    );
                    write(
                        &out_dir.join(&pre_file),
                        &format!("{}\n", hex_encode(&compare.pre)),
                    );
                    entries.push(format!(
                        "{{\"role\": {}, \"address\": {}, \"expected\": {}, \"pre\": {}}}",
                        json_string(&compare.role),
                        json_string(&compare.address),
                        json_string(&expected_file),
                        json_string(&pre_file)
                    ));
                }
                fields.push(format!("\"kind\": {}", json_string(kind)));
                fields.push(format!("\"compare\": [{}]", entries.join(", ")));
                fields.push(format!(
                    "\"identical_to_pre\": {}",
                    json_list(
                        &case
                            .identical_to_pre
                            .iter()
                            .map(|role| json_string(role))
                            .collect::<Vec<_>>()
                    )
                ));
            }
            (None, Some(expect)) => {
                fields.push(format!(
                    "\"kind\": {}",
                    json_string(if case.exhausted {
                        "exhausted"
                    } else {
                        "refuse"
                    })
                ));
                fields.push(format!("\"expect_code\": {expect}"));
                fields.push(format!(
                    "\"expect_code_hex\": {}",
                    json_string(&format!("0x{expect:04x}"))
                ));
                fields.push(format!("\"reference\": {}", json_string(&case.reference)));
            }
            (None, None) => panic!("case {} is neither an accept nor a refusal", case.name),
        }
        case_json.push(format!("    {{{}}}", fields.join(", ")));
    }
    case_json
}

fn main() {
    let out_dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: clutch-sbf-harness <out-dir>"),
    );
    for sub in ["accounts", "expected", "tx"] {
        fs::create_dir_all(out_dir.join(sub)).expect("create plan directory");
    }

    let f = build_fixture();
    let shared = &f.shared;
    let mut plan = Plan::default();

    /* Realm-wide accounts. */
    plan.account("realm", &shared.realm, &shared.realm_bytes);
    plan.account("profile", &shared.profile, &shared.profile_bytes);
    plan.account("grid", &shared.grid, &shared.grid_bytes);
    plan.account("terms", &shared.terms, &shared.terms_bytes);
    plan.account("feed", &shared.feed_head, &shared.feed_bytes);
    plan.account(
        "advance-feed",
        &shared.advance_feed_head,
        &shared.advance_feed_bytes,
    );

    /* Every market plane. */
    for plane in [&f.seam, &f.held, &f.shadow, &f.redeem, &f.create] {
        for (role, pda) in plane.state_roles() {
            plan.account(
                &format!("{}.{role}", plane.label),
                pda,
                plane.state_slice(role),
            );
        }
        plan.account(
            &format!("{}.resolution", plane.label),
            &plane.resolution,
            &plane.resolution_bytes,
        );
    }

    /* The batch-auction plane, bound to the seam market. */
    plan.account("epoch", &f.batch.epoch, &f.batch.epoch_bytes);
    plan.account("page", &f.batch.page, &f.batch.page_bytes);
    plan.account("candidate", &f.batch.candidate, &f.batch.candidate_bytes);
    plan.account("pot", &f.batch.pot, &f.batch.pot_bytes);
    plan.account("receipt", &f.batch.receipt, &f.batch.receipt_bytes);

    /* Caller-supplied buffers, and the imposter replay account. */
    plan.account("resolve-buffer", &f.resolve_buffer, &f.resolve_buffer_bytes);
    plan.account("redeem-buffer", &f.redeem_buffer, &f.redeem_buffer_bytes);
    plan.account("page-buffer", &f.page_buffer, &f.page_buffer_bytes);
    plan.account("replay-imposter", &shared.imposter, &f.seam.state.replay);

    /* Every plane of the lifecycle walk, plus its three observation pages.
     * A walk plane's genesis is the offline reference adapter's post-state
     * after every earlier step of the walk; none of it is hand-written. */
    for plane in f.walk.planes() {
        for (role, pda) in plane.state_roles() {
            plan.account(
                &format!("{}.{role}", plane.label),
                pda,
                plane.state_slice(role),
            );
        }
        plan.account(
            &format!("{}.resolution", plane.label),
            &plane.resolution,
            &plane.resolution_bytes,
        );
    }
    for (index, page) in f.walk.pages.iter().enumerate() {
        plan.account(
            &format!("walk-page-{index}"),
            page,
            &f.walk.page_bytes[index],
        );
    }

    plan.cases = build_cases(&f);
    let lifecycle = build_lifecycle(&f);

    /* Files. */
    let program_address = shared.program.address.clone();
    let mut genesis_lines = String::new();
    for account in &plan.genesis {
        let file = format!("accounts/{}.json", account.role);
        write(
            &out_dir.join(&file),
            &account_json(&account.address, &program_address, &account.data),
        );
        genesis_lines.push_str(&format!("{} {} {}\n", account.role, account.address, file));
    }
    write(&out_dir.join("genesis.txt"), &genesis_lines);

    let case_json = emit_cases(&out_dir, &plan.cases);
    let walk_json = emit_cases(&out_dir, &lifecycle.cases);

    let genesis_json: Vec<String> = plan
        .genesis
        .iter()
        .map(|account| {
            format!(
                "    {{\"role\": {}, \"address\": {}, \"file\": {}}}",
                json_string(&account.role),
                json_string(&account.address),
                json_string(&format!("accounts/{}.json", account.role))
            )
        })
        .collect();

    let plan_json = format!(
        "{{\n  \"program_id\": {},\n  \"payer\": {},\n  \"actor\": {},\n  \"stranger\": {},\n  \"imposter\": {},\n  \"genesis\": [\n{}\n  ],\n  \"cases\": [\n{}\n  ],\n  \"lifecycle\": {}\n}}\n",
        json_string(&program_address),
        json_string(&shared.payer.address),
        json_string(&shared.actor.address),
        json_string(&shared.stranger.address),
        json_string(&shared.imposter.address),
        genesis_json.join(",\n"),
        case_json.join(",\n"),
        lifecycle_json(&lifecycle, &walk_json)
    );
    write(&out_dir.join("plan.json"), &plan_json);

    println!("bring-up plan written to {}", out_dir.display());
    println!("program_id   {program_address}");
    println!("payer        {}", shared.payer.address);
    println!("actor        {}", shared.actor.address);
    println!(
        "genesis      {} accounts, {} transactions",
        plan.genesis.len(),
        plan.cases.len()
    );
    for case in &plan.cases {
        if case.exhausted {
            println!(
                "  EXHAUSTED {:<27} does not fit the {COMPUTE_UNIT_CEILING}-unit transaction ceiling",
                case.name
            );
            continue;
        }
        match case.expect_code {
            None => println!(
                "  accept {:<30} {} instruction(s), {} account(s) compared, oracle {}",
                case.name,
                case.instruction_count,
                case.compare.as_ref().map(Vec::len).unwrap_or(0),
                case.oracle
            ),
            Some(expect) => println!(
                "  refuse {:<30} expect Custom(0x{expect:04x})  offline reference: {}",
                case.name, case.reference
            ),
        }
    }

    println!(
        "lifecycle    {} steps, {} skipped section-10 items, {} terminal readouts, {} identities",
        lifecycle.steps.len(),
        lifecycle.skips.len(),
        lifecycle.values.len(),
        lifecycle.identities.len()
    );
    for step in &lifecycle.steps {
        println!(
            "  step {:>2}  {:<26} {}",
            step.ordinal, step.case, step.title
        );
    }
    for skip in &lifecycle.skips {
        println!("  skip item {:<12} {}", skip.project_item, skip.title);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base58_decodes_the_all_zero_address() {
        assert_eq!(b58_decode32(SYSTEM_PROGRAM), [0_u8; 32]);
    }

    #[test]
    fn base58_decode_agrees_with_the_pinned_cli() {
        /* The CLI is the oracle: derive an address, decode it, and check that
         * re-deriving with the decoded bytes as a seed is stable.  A decoder
         * that dropped or reordered bytes could not keep this consistent. */
        let first = fixed_address("clutch-sbf/bringup/decoder/v1");
        let again = derive(SYSTEM_PROGRAM, &[first.bytes.to_vec()]);
        let expected = derive(SYSTEM_PROGRAM, &[b58_decode32(&first.address).to_vec()]);
        assert_eq!(again.address, expected.address);
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(b64_encode(b""), "");
        assert_eq!(b64_encode(b"f"), "Zg==");
        assert_eq!(b64_encode(b"fo"), "Zm8=");
        assert_eq!(b64_encode(b"foo"), "Zm9v");
        assert_eq!(b64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(b64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(b64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn compact_u16_matches_the_short_vec_encoding() {
        let mut out = Vec::new();
        compact_u16(0, &mut out);
        assert_eq!(out, vec![0]);
        out.clear();
        compact_u16(127, &mut out);
        assert_eq!(out, vec![127]);
        out.clear();
        compact_u16(128, &mut out);
        assert_eq!(out, vec![0x80, 0x01]);
        out.clear();
        compact_u16(16384, &mut out);
        assert_eq!(out, vec![0x80, 0x80, 0x01]);
    }

    #[test]
    fn the_whole_plan_builds_and_every_oracle_accepts_it() {
        /* Building the fixture runs the offline reference adapter over every
         * accepting transition, `validate_market_init` over the founding
         * plane, and the accumulator over the observation page; it asserts
         * every cross-account binding the frozen layout can decide.  A fixture
         * that drifted apart is a test failure here rather than a genesis
         * nobody checked.
         *
         * It is deliberately not an assertion about the SVM: that is the
         * differential in `scripts/simulate.py`. */
        let f = build_fixture();
        let cases = build_cases(&f);
        assert!(
            cases.iter().any(|case| case.name == "roundtrip"),
            "the plan must carry the Split/Merge round trip"
        );
        for family in [
            "Split",
            "Merge",
            "Materialize",
            "Dematerialize",
            "Resolve",
            "RedeemInternal",
            "FeedAdvance",
            "CreateMarket",
        ] {
            assert!(
                cases
                    .iter()
                    .any(|case| case.family == family && case.compare.is_some()),
                "no accepting transaction for {family}"
            );
            assert!(
                cases
                    .iter()
                    .filter(|case| case.family == family && case.expect_code.is_some())
                    .count()
                    >= 2,
                "fewer than two refusals for {family}"
            );
        }
    }

    #[test]
    fn the_round_trip_restores_every_account_but_the_replay_sequence() {
        let f = build_fixture();
        let cases = build_cases(&f);
        let roundtrip = cases
            .iter()
            .find(|case| case.name == "roundtrip")
            .expect("the round trip case");
        let compares = roundtrip.compare.as_ref().expect("an accepting case");
        for compare in compares {
            if compare.role.ends_with(".replay") {
                assert_ne!(compare.expected, compare.pre);
            } else {
                assert_eq!(compare.expected, compare.pre, "{} moved", compare.role);
            }
        }
    }

    #[test]
    fn every_transaction_fits_one_legacy_packet() {
        let f = build_fixture();
        let walk = build_lifecycle(&f);
        for case in build_cases(&f).iter().chain(walk.cases.iter()) {
            assert!(
                case.tx.len() <= 1232,
                "{} is {} bytes",
                case.name,
                case.tx.len()
            );
        }
    }

    #[test]
    fn the_lifecycle_walk_is_one_ordered_chain() {
        /* Building the walk runs the offline reference adapter forward over
         * every step, asserts that each plane's genesis is the previous step's
         * post-state at the right replay sequence, asserts that the opening
         * state is `CreateMarket`'s own post-state plus exactly one credited
         * field, asserts that the three feed advances land the cursor on the
         * window's maturity bound, and asserts every terminal accounting
         * identity over derived numbers.  A walk that stopped being a chain is
         * a panic in here rather than a green differential over a fiction. */
        let f = build_fixture();
        let walk = build_lifecycle(&f);
        assert_eq!(walk.steps.len(), 10, "the walk is ten steps");
        for (index, step) in walk.steps.iter().enumerate() {
            assert_eq!(step.ordinal as usize, index + 1, "the walk is ordered");
            let case = walk
                .cases
                .iter()
                .find(|case| case.name == step.case)
                .unwrap_or_else(|| panic!("step {} names no case", step.ordinal));
            assert!(
                !step.narrative.is_empty() && !step.title.is_empty(),
                "step {} records no narrative",
                step.ordinal
            );
            assert!(
                case.compare.is_some() || case.expect_code.is_some(),
                "step {} is neither an accept nor a refusal",
                step.ordinal
            );
        }
        assert_eq!(
            walk.cases.len(),
            walk.steps.len(),
            "every walk case belongs to exactly one step"
        );
        assert_eq!(
            walk.cases
                .iter()
                .filter(|case| case.expect_code.is_some())
                .count(),
            1,
            "the walk carries exactly one recorded refusal: the post-resolution merge"
        );
    }

    #[test]
    fn the_walk_records_every_section_ten_item_it_cannot_drive() {
        let f = build_fixture();
        let walk = build_lifecycle(&f);
        for item in ["1 (in part)", "2", "3", "8", "11"] {
            let skip = walk
                .skips
                .iter()
                .find(|skip| skip.project_item == item)
                .unwrap_or_else(|| panic!("PROJECT.md section 10 item {item} is silently absent"));
            assert!(
                skip.reason.len() > 40,
                "item {item} is skipped without a reason"
            );
        }
    }

    #[test]
    fn the_terminal_identity_is_read_out_of_the_walks_own_last_post_state() {
        let f = build_fixture();
        let walk = build_lifecycle(&f);
        let terminal = walk
            .cases
            .iter()
            .find(|case| case.name == walk.terminal_case)
            .expect("the terminal case");
        let compares = terminal.compare.as_ref().expect("an accepting case");
        for value in &walk.values {
            let compare = compares
                .iter()
                .find(|compare| compare.role == value.role)
                .unwrap_or_else(|| {
                    panic!("{} names a role the walk does not compare", value.label)
                });
            let window = &compare.expected[value.offset..value.offset + value.width];
            let mut bytes = [0_u8; 8];
            bytes.copy_from_slice(window);
            assert_eq!(
                u64::from_le_bytes(bytes),
                value.expected,
                "{} is not at the offset the probe found",
                value.label
            );
        }
    }

    #[test]
    fn the_walk_drains_every_internal_claim() {
        /* The economic content of the walk, stated once, in one place: the
         * position ends with nothing internal, the Hoard ends holding exactly
         * the claims that were materialized and never brought back, and the
         * cash it did not keep is that same number. */
        let f = build_fixture();
        let terminal = &f.walk.terminal;
        let hoard = HoardAccount::decode(&terminal.hoard).expect("hoard");
        let position = PositionAccount::decode(&terminal.position).expect("position");
        assert_eq!(position.internal, [0; MAX_OUTCOMES]);
        assert_eq!(hoard.collateral_atoms, walk_unredeemed_external());
        assert_eq!(position.cash_atoms + hoard.collateral_atoms, WALK_CASH);
    }
}
