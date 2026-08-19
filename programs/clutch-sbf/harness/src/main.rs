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
use clutch_sbf::token;
use clutch_solana_layout::{
    account_len, canonical_epoch_id, canonical_market_id, canonical_order_id,
    canonical_order_set_id, canonical_outcome_id, canonical_realm_id, collateral, CandidateRecord,
    EpochAccount, FeedAccount, FeedId, FinalPotAccount, Hash32, HoardAccount, Intent,
    MarketAccount, OrderPageAccount, OrderRecord, OrderSlot, PayoutVectorBytes, PositionAccount,
    PriceGridAccount, ProfileAccount, RealmAccount, ResolutionAccount, SettlementReceiptAccount,
    SupplyLedgerAccount, TermsAccount, CANDIDATE_STATUS_SELECTED, EPOCH_PHASE_CLEARED,
    MAX_GRID_TICKS, MAX_INTENT_BYTES, MAX_ORDERS_PER_PAGE, MAX_OUTCOMES, PAYOUT_INDEX_UNRESOLVED,
    POT_PHASE_OPEN, PROFILE_FLAG_POLICY_FROZEN, RECEIPT_LEG_DIRECT, RELATION_VERSION,
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

/// Decimals of the fixture collateral mint, as the Realm's policy names them.
///
/// The mint account this harness installs must carry exactly this exponent:
/// `token::MintPolicy::collateral` reads it out of the 266 policy bytes and
/// `check_mint` refuses any other, which `assert_admitted_mint` re-runs here at
/// build time.
const COLLATERAL_DECIMALS: u8 = 9;
/// Supply of the fixture collateral mint.
///
/// Nonzero because `COLLATERAL_POLICY_STRICT_FLAGS` sets
/// `FLAG_REQUIRE_NONZERO_SUPPLY`: a mint nobody has ever minted is not
/// collateral.  Below the policy's `max_supply_atoms` ceiling, which
/// `check_mint` also enforces.
const COLLATERAL_SUPPLY: u64 = 100_000_000;
/// Collateral atoms the fixture actor holds before any `Split`.
///
/// Comfortably above every quantity this plan splits, because one actor token
/// account serves every market plane and each plan transaction is simulated
/// against genesis independently.
const ACTOR_COLLATERAL_ATOMS: u64 = 1_000_000;
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
/// Atoms one `Endow` credits into a position's internal cash.
///
/// Below the fixture market's `collateral_cap`, which is the one ceiling
/// `genesis::apply_endow` enforces and the one this plan can drive over.
const ENDOW_AMOUNT: u64 = 40;
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
    /// `ClutchError::Replay` on adapter-owned instruction families.
    pub const REPLAY: u32 = 0x000d;
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
    /// `ClutchError::TokenAccountNotAdmitted`.
    ///
    /// New with the mandatory collateral leg and with no counterpart in the
    /// offline reference adapter, which models no token plane at all: it is
    /// what a caller earns for presenting a Token-2022 account the Realm's
    /// frozen policy or the instruction's own owner-authority rule refuses.
    pub const TOKEN_ACCOUNT_NOT_ADMITTED: u32 = 0x001b;
    /// `ClutchError::TokenDeltaMismatch` after a duplicate bearer exit finds
    /// the source already burned inside the same transaction.
    pub const TOKEN_DELTA_MISMATCH: u32 = 0x001c;
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

/// Base58-encode a 32-byte address.
///
/// The inverse of [`b58_decode32`], and pinned by it: every address this
/// harness encodes is decoded straight back and required to be the same
/// thirty-two bytes, so a carry bug here is a panic rather than a genesis
/// account installed at the wrong address.
fn base58_of(bytes: &[u8; 32]) -> String {
    let mut digits: Vec<u8> = Vec::with_capacity(45);
    for byte in bytes {
        let mut carry = u32::from(*byte);
        for digit in digits.iter_mut() {
            carry += u32::from(*digit) << 8;
            *digit = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    let mut out = String::new();
    for byte in bytes {
        if *byte != 0 {
            break;
        }
        out.push('1');
    }
    for digit in digits.iter().rev() {
        out.push(B58[usize::from(*digit)] as char);
    }
    assert_eq!(
        b58_decode32(&out),
        *bytes,
        "the base58 encoder must round-trip through the decoder"
    );
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

/// A real test signer supplied by the committed-bank runner, or the ordinary
/// deterministic key-free fixture identity when the variable is absent.
///
/// Only the public key crosses this boundary.  The committed runner retains
/// the ephemeral secret long enough to sign and never passes it to this plan
/// generator.  `bump` is meaningless for a wallet identity and is never read.
fn fixture_identity(variable: &str, fallback: &str) -> Pda {
    match std::env::var(variable) {
        Ok(address) => Pda {
            bytes: b58_decode32(&address),
            address,
            bump: 0,
        },
        Err(std::env::VarError::NotPresent) => fixed_address(fallback),
        Err(error) => panic!("cannot read {variable}: {error}"),
    }
}

/* ------------------------------------------------------------------------ */
/* Token-2022 account images                                                 */
/* ------------------------------------------------------------------------ */

/* Every mint and every token account this plan installs at genesis is written
 * here, byte by byte, rather than created by the token program -- a validator
 * loaded from a genesis dump cannot run an instruction before the first slot.
 * That is a real claim to defend, and it is defended twice: the *real*
 * Token-2022 program is what executes `MintTo`, `Burn` and `TransferChecked`
 * against these bytes inside the SVM and refuses anything it did not write,
 * and this harness re-runs the program's own `token::check_mint` /
 * `token::check_token_account` admission over every image before it is
 * emitted, so a byte this program would refuse is a build-time panic here and
 * not a mysterious on-chain refusal later. */

/// A Token-2022 mint exactly as the token program writes one, extension-free.
///
/// `authority` is the `COption<Pubkey>` mint authority: `Some` for an outcome
/// mint whose authority is the market PDA, `None` for a collateral mint, whose
/// absent authority is what `MintPolicy::collateral` demands.  The freeze
/// authority is always absent.
fn mint_bytes(authority: Option<[u8; 32]>, decimals: u8, supply: u64) -> Vec<u8> {
    let mut data = vec![0_u8; token::BASE_MINT_LEN];
    if let Some(key) = authority {
        data[0..4].copy_from_slice(&1_u32.to_le_bytes());
        data[4..36].copy_from_slice(&key);
    }
    data[36..44].copy_from_slice(&supply.to_le_bytes());
    data[44] = decimals;
    data[45] = 1; // is_initialized
    data
}

/// A base Token-2022 token account, exactly as the token program writes one.
fn token_account_bytes(mint: [u8; 32], owner: [u8; 32], amount: u64) -> Vec<u8> {
    let mut data = vec![0_u8; token::BASE_TOKEN_ACCOUNT_LEN];
    data[0..32].copy_from_slice(&mint);
    data[32..64].copy_from_slice(&owner);
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1; // AccountState::Initialized
    data
}

/// The same, carrying the one `ImmutableOwner` extension entry the Hoard's
/// token account is created with by `market_init::create_token_plane`.
fn immutable_owner_account_bytes(mint: [u8; 32], owner: [u8; 32], amount: u64) -> Vec<u8> {
    let mut data = vec![0_u8; token::IMMUTABLE_OWNER_ACCOUNT_LEN];
    data[..token::BASE_TOKEN_ACCOUNT_LEN]
        .copy_from_slice(&token_account_bytes(mint, owner, amount));
    data[token::BASE_TOKEN_ACCOUNT_LEN] = 2; // AccountType::Account
    data[166..168].copy_from_slice(&u16::from(token::EXT_IMMUTABLE_OWNER).to_le_bytes());
    data[168..170].copy_from_slice(&0_u16.to_le_bytes());
    data
}

/// Overwrite a token account image's `amount` field, keeping every other byte.
///
/// Used to build the *expected* post-state of a token account from a number
/// the offline reference adapter produced, rather than from a second
/// description of what a transfer does.
fn with_amount(image: &[u8], amount: u64) -> Vec<u8> {
    let mut data = image.to_vec();
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data
}

/// Overwrite a mint image's `supply` field, keeping every other byte.
fn with_supply(image: &[u8], supply: u64) -> Vec<u8> {
    let mut data = image.to_vec();
    data[36..44].copy_from_slice(&supply.to_le_bytes());
    data
}

/// Re-run the program's own mint admission over an image this harness wrote.
fn assert_admitted_mint(address: &[u8; 32], data: &[u8], policy: &token::MintPolicy, label: &str) {
    token::check_mint(
        &solana_pubkey_of(collateral::TOKEN_2022_PROGRAM),
        &solana_pubkey_of(*address),
        data,
        policy,
    )
    .unwrap_or_else(|fault| panic!("{label}: this program refuses the fixture mint: {fault:?}"));
}

/// Re-run the program's own token-account admission over an image this harness
/// wrote.
fn assert_admitted_token_account(data: &[u8], policy: &token::TokenAccountPolicy, label: &str) {
    token::check_token_account(
        &solana_pubkey_of(collateral::TOKEN_2022_PROGRAM),
        data,
        policy,
    )
    .unwrap_or_else(|fault| {
        panic!("{label}: this program refuses the fixture token account: {fault:?}")
    });
}

/// The `solana_pubkey::Pubkey` the program's admission functions take.
fn solana_pubkey_of(bytes: [u8; 32]) -> solana_pubkey::Pubkey {
    solana_pubkey::Pubkey::new_from_array(bytes)
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

/// Build the canonical sequence-zero bearer redemption envelope.
fn redeem_external_request(shared: &Shared, plane: &Plane, quantity: u64) -> Vec<u8> {
    layout_request(
        0,
        Intent::RedeemExternal {
            market: plane.market_id,
            claimant: Hash32::from_bytes(shared.holder.bytes),
            source: Hash32::from_bytes(shared.holder_outcome_token.bytes),
            destination: Hash32::from_bytes(shared.holder_collateral_token.bytes),
            outcome: WALK_OUTCOME_WIN,
            quantity,
        },
    )
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

/// Create and initialize an ordinary, signer-addressed Token-2022 account.
///
/// This is intentionally not a Clutch instruction and not validator-genesis
/// assistance. The fee payer and fresh account identity sign the System
/// `CreateAccount`; Token-2022 `InitializeAccount3` then binds the resulting
/// account to the requested mint and bearer authority in the same atomic
/// transaction.
fn create_holder_token_transaction(
    shared: &Shared,
    mint: &[u8; 32],
    holder: &[u8; 32],
    account: &[u8; 32],
) -> Vec<u8> {
    let message = Message::new(
        &[shared.payer.bytes, *account],
        &[],
        &[],
        &[shared.system_program, shared.token_program, *mint],
    );

    /* `SystemInstruction::CreateAccount` is bincode's enum variant 0,
     * followed by lamports, space, and owner. This is the same frozen 52-byte
     * encoding `token::create_account_signed` emits inside the program. */
    let mut create = vec![0_u8; 52];
    create[4..12].copy_from_slice(&ACCOUNT_LAMPORTS.to_le_bytes());
    create[12..20].copy_from_slice(&(token::BASE_TOKEN_ACCOUNT_LEN as u64).to_le_bytes());
    create[20..52].copy_from_slice(&shared.token_program);
    let create = Instruction {
        program_index: message.index(&shared.system_program),
        accounts: message.indices(&[shared.payer.bytes, *account]),
        data: create,
    };

    let mut initialize = vec![0_u8; 33];
    initialize[0] = 18; // TokenInstruction::InitializeAccount3
    initialize[1..33].copy_from_slice(holder);
    let initialize = Instruction {
        program_index: message.index(&shared.token_program),
        accounts: message.indices(&[*account, *mint]),
        data: initialize,
    };
    transaction(&message, &[create, initialize])
}

/// Transfer a materialized outcome token to an independent bearer.
///
/// The source authority signs directly. The destination authority does not
/// sign a Token-2022 transfer, which is why this transaction needs only the
/// payer and the original actor even though the next committed step is driven
/// by the new holder.
fn transfer_outcome_transaction(
    shared: &Shared,
    source: &[u8; 32],
    mint: &[u8; 32],
    destination: &[u8; 32],
    quantity: u64,
) -> Vec<u8> {
    let message = Message::new(
        &[shared.payer.bytes],
        &[shared.actor.bytes],
        &[*source, *destination],
        &[*mint, shared.token_program],
    );
    let mut data = vec![0_u8; 10];
    data[0] = 12; // TokenInstruction::TransferChecked
    data[1..9].copy_from_slice(&quantity.to_le_bytes());
    data[9] = 0; // outcome mints have indivisible atoms
    let transfer = Instruction {
        program_index: message.index(&shared.token_program),
        accounts: message.indices(&[*source, *mint, *destination, shared.actor.bytes]),
        data,
    };
    transaction(&message, &[transfer])
}

/// Fund the fee payer's freshly created collateral account from the founding
/// actor through an ordinary Token-2022 transfer.
fn transfer_second_owner_collateral_transaction(shared: &Shared, quantity: u64) -> Vec<u8> {
    let message = Message::new(
        &[shared.payer.bytes],
        &[shared.actor.bytes],
        &[
            shared.actor_token.bytes,
            shared.payer_collateral_token.bytes,
        ],
        &[shared.collateral_mint.bytes, shared.token_program],
    );
    let mut data = vec![0_u8; 10];
    data[0] = 12; // TokenInstruction::TransferChecked
    data[1..9].copy_from_slice(&quantity.to_le_bytes());
    data[9] = COLLATERAL_DECIMALS;
    let transfer = Instruction {
        program_index: message.index(&shared.token_program),
        accounts: message.indices(&[
            shared.actor_token.bytes,
            shared.collateral_mint.bytes,
            shared.payer_collateral_token.bytes,
            shared.actor.bytes,
        ]),
        data,
    };
    transaction(&message, &[transfer])
}

/// Burn a bearer Egg and pay its resolved collateral value directly to the
/// independent claimant, without a Position or Replay account.
fn redeem_external_transaction(
    shared: &Shared,
    plane: &Plane,
    outcome: u8,
    data: Vec<u8>,
) -> Vec<u8> {
    redeem_external_transaction_repeated(shared, plane, outcome, data, 1)
}

/// Repeat the same bearer exit inside one transaction. With `repetitions ==
/// 2`, the first instruction completes its burn and payout, the duplicate then
/// refuses on the now-empty source, and SVM atomicity must roll the first one
/// back too.
fn redeem_external_transaction_repeated(
    shared: &Shared,
    plane: &Plane,
    outcome: u8,
    data: Vec<u8>,
    repetitions: usize,
) -> Vec<u8> {
    assert!(repetitions > 0, "at least one bearer exit is required");
    let touched = usize::from(outcome);
    let writable = vec![
        plane.hoard.bytes,
        plane.kernel.bytes,
        plane.supply.bytes,
        shared.holder_collateral_token.bytes,
        plane.hoard_token.bytes,
        shared.holder_outcome_token.bytes,
        plane.outcome_mints[touched].bytes,
    ];
    let mut readonly = vec![
        shared.profile.bytes,
        plane.market.bytes,
        plane.resolution.bytes,
        shared.terms.bytes,
        shared.policy_account.bytes,
        shared.token_program,
        shared.collateral_mint.bytes,
        plane.hoard_authority.bytes,
        shared.program.bytes,
        shared.compute_budget,
    ];
    for (index, mint) in plane.outcome_mints.iter().enumerate() {
        if index != touched {
            readonly.push(mint.bytes);
        }
    }
    let message = Message::new(
        &[shared.payer.bytes],
        &[shared.holder.bytes],
        &writable,
        &readonly,
    );
    let mut keys = vec![
        shared.holder.bytes,
        shared.profile.bytes,
        plane.market.bytes,
        plane.hoard.bytes,
        plane.kernel.bytes,
        plane.supply.bytes,
        plane.resolution.bytes,
        shared.terms.bytes,
        shared.policy_account.bytes,
        shared.token_program,
        shared.collateral_mint.bytes,
        shared.holder_collateral_token.bytes,
        plane.hoard_authority.bytes,
        plane.hoard_token.bytes,
        shared.holder_outcome_token.bytes,
    ];
    keys.extend(plane.outcome_mints.iter().map(|mint| mint.bytes));
    assert_eq!(
        keys.len(),
        clutch_sbf::instructions::external_exit::IX_OUTCOME_MINTS + usize::from(OUTCOME_COUNT),
        "RedeemExternal must carry the complete canonical mint vector"
    );
    let instruction = Instruction {
        program_index: message.index(&shared.program.bytes),
        accounts: message.indices(&keys),
        data,
    };
    let mut instructions = vec![budget_instruction(
        &message,
        &shared.compute_budget,
        COMPUTE_UNIT_CEILING,
    )];
    instructions.extend(std::iter::repeat_n(instruction, repetitions));
    transaction(&message, &instructions)
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
    /// Fresh ordinary Token-2022 account identity for the actor's materialized
    /// winning Egg in the committed-bank walk.
    actor_outcome_token: Pda,
    /// Fresh ordinary collateral account identity for the fee payer acting as
    /// a second market participant in the committed-bank walk.
    payer_collateral_token: Pda,
    /// Independent bearer claimant used only by the committed-bank walk.
    holder: Pda,
    /// Fresh ordinary account identity used to construct that claimant's
    /// Token-2022 outcome account through public System + Token instructions.
    holder_outcome_token: Pda,
    /// Fresh ordinary account identity for that claim holder's collateral
    /// destination, created through the same public System + Token path.
    holder_collateral_token: Pda,
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
    /// The decoded Realm collateral policy every token leg reads.
    policy: collateral::CollateralPolicy,
    /// The account the 266 policy bytes are presented from.
    ///
    /// Deliberately **not** derived: the policy is content-authenticated by
    /// recomputed digest against the Profile, so binding its address would
    /// suggest the address is what makes it the Realm's policy.
    policy_account: Pda,
    /// The Token-2022 collateral mint the policy names.
    collateral_mint: Pda,
    collateral_mint_bytes: Vec<u8>,
    /// The actor's own Token-2022 collateral account, shared by every plane.
    actor_token: Pda,
    actor_token_bytes: Vec<u8>,
    /// The stranger's own collateral account.
    ///
    /// It exists so that the two "a different authenticated signer" refusals
    /// stay refusals *about authorization*.  Without it a stranger would have
    /// to present the actor's collateral account and be refused
    /// `TokenAccountNotAdmitted` for owning the wrong token account, which is
    /// a true refusal about a different question.  Both questions are asked
    /// here, one case each.
    stranger_token: Pda,
    stranger_token_bytes: Vec<u8>,
    /// The pinned Token-2022 program's address.
    token_program: [u8; 32],
    /// The System program's address.
    system_program: [u8; 32],
    /// The Rent sysvar's address.
    rent_sysvar: [u8; 32],
}

/// The Realm's frozen collateral policy: a real, decodable 266-byte policy
/// whose recomputed child digest the fixture Profile freezes, and whose
/// recomputed *parent* is the fixture Profile's identity.
fn fixture_policy(collateral_mint: [u8; 32]) -> collateral::CollateralPolicy {
    let backing = collateral::CurrencyRef::spl(
        collateral::TOKEN_2022_PROGRAM,
        collateral_mint,
        COLLATERAL_DECIMALS,
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
    let payer = fixture_identity("CLUTCH_COMMITTED_PAYER", "clutch-sbf/bringup/payer/v1");
    let actor = fixture_identity("CLUTCH_COMMITTED_ACTOR", "clutch-sbf/bringup/actor/v1");
    let actor_outcome_token = fixture_identity(
        "CLUTCH_COMMITTED_ACTOR_OUTCOME_TOKEN",
        "clutch-sbf/actor/outcome-token",
    );
    let payer_collateral_token = fixture_identity(
        "CLUTCH_COMMITTED_PAYER_COLLATERAL_TOKEN",
        "clutch-sbf/payer/collat-token",
    );
    let holder = fixture_identity("CLUTCH_COMMITTED_HOLDER", "clutch-sbf/bringup/holder/v1");
    let holder_outcome_token = fixture_identity(
        "CLUTCH_COMMITTED_HOLDER_OUTCOME_TOKEN",
        "clutch-sbf/holder/outcome-token",
    );
    let holder_collateral_token = fixture_identity(
        "CLUTCH_COMMITTED_HOLDER_COLLATERAL_TOKEN",
        "clutch-sbf/holder/collat-token",
    );
    assert_ne!(payer.bytes, actor.bytes, "payer and actor must be distinct");
    assert_ne!(
        actor_outcome_token.bytes, actor.bytes,
        "actor outcome account and authority must be distinct"
    );
    assert_ne!(
        payer_collateral_token.bytes, payer.bytes,
        "payer collateral account and authority must be distinct"
    );
    assert_ne!(
        holder.bytes, actor.bytes,
        "holder and actor must be distinct"
    );
    assert_ne!(
        holder.bytes, payer.bytes,
        "holder and payer must be distinct"
    );
    assert_ne!(
        holder_outcome_token.bytes, holder.bytes,
        "holder-token account and authority must be distinct"
    );
    assert_ne!(
        holder_collateral_token.bytes, holder.bytes,
        "holder-collateral account and authority must be distinct"
    );
    assert_ne!(
        holder_outcome_token.bytes, holder_collateral_token.bytes,
        "holder outcome and collateral token accounts must be distinct"
    );
    let stranger = fixed_address("clutch-sbf/bringup/stranger/v1");
    let imposter = fixed_address("clutch-sbf/bringup/imposter/v1");
    let pid = program.address.clone();

    /* The Realm's collateral policy names a mint, and the Profile identity is
     * the canonical parent hash over that policy's own digest -- recomputed,
     * never chosen.  `collateral::verify_profile_identity` refuses any other
     * pairing, so this is the only Profile ID a Realm backed by this mint can
     * have, and every address below descends from it. */
    let collateral_mint = fixed_address("clutch/collat/mint/v1");
    let policy_account = fixed_address("clutch/collat/policy/v1");
    let actor_token = fixed_address("clutch/collat/actor/v1");
    let stranger_token = fixed_address("clutch/collat/stranger/v1");
    let policy = fixture_policy(collateral_mint.bytes);
    let profile_hash = collateral::ParentProfile::from_policy(&policy)
        .expect("the fixture policy must compose a parent profile")
        .identity()
        .expect("the parent profile must derive an identity");
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

    /* The collateral leg's two shared Token-2022 accounts.  Both images are
     * put through this program's own admission before they leave this
     * function, so a fixture the program would refuse cannot reach a
     * validator. */
    let collateral_mint_bytes = mint_bytes(None, COLLATERAL_DECIMALS, COLLATERAL_SUPPLY);
    assert_admitted_mint(
        &collateral_mint.bytes,
        &collateral_mint_bytes,
        &token::MintPolicy::collateral(&policy),
        "the fixture collateral mint",
    );
    let actor_token_bytes =
        token_account_bytes(collateral_mint.bytes, actor.bytes, ACTOR_COLLATERAL_ATOMS);
    assert_admitted_token_account(
        &actor_token_bytes,
        &token::TokenAccountPolicy::collateral_holder(&policy, solana_pubkey_of(actor.bytes)),
        "the fixture actor collateral account",
    );
    let stranger_token_bytes = token_account_bytes(
        collateral_mint.bytes,
        stranger.bytes,
        ACTOR_COLLATERAL_ATOMS,
    );
    assert_admitted_token_account(
        &stranger_token_bytes,
        &token::TokenAccountPolicy::collateral_holder(&policy, solana_pubkey_of(stranger.bytes)),
        "the fixture stranger collateral account",
    );

    Shared {
        program,
        compute_budget: b58_decode32(COMPUTE_BUDGET_PROGRAM),
        payer,
        actor,
        actor_outcome_token,
        payer_collateral_token,
        holder,
        holder_outcome_token,
        holder_collateral_token,
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
        policy,
        policy_account,
        collateral_mint,
        collateral_mint_bytes,
        actor_token,
        actor_token_bytes,
        stranger_token,
        stranger_token_bytes,
        token_program: collateral::TOKEN_2022_PROGRAM,
        system_program: [0; 32],
        rent_sysvar: token::RENT_SYSVAR_ID.to_bytes(),
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
    /// The Hoard's signing authority; holds no data and is never written.
    hoard_authority: Pda,
    /// The Hoard's Token-2022 collateral account.
    hoard_token: Pda,
    /// One outcome mint per active outcome, in index order.
    outcome_mints: Vec<Pda>,
    /// The actor's own Token-2022 account for each outcome mint.
    ///
    /// Not derived and not required to be: `TokenAccountPolicy::holder`
    /// authenticates a holder account by mint and owner authority and
    /// deliberately does not require an associated token account.
    holder_tokens: Vec<Pda>,
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
        let resolution = derive(
            &pid,
            &[seeds::SEED_RESOLUTION.to_vec(), market_seed.clone()],
        );
        let hoard_authority = derive(
            &pid,
            &[seeds::SEED_HOARD_AUTHORITY.to_vec(), market_seed.clone()],
        );
        let hoard_token = derive(
            &pid,
            &[seeds::SEED_HOARD_TOKEN.to_vec(), market_seed.clone()],
        );
        let outcome_mints: Vec<Pda> = (0..OUTCOME_COUNT)
            .map(|outcome| {
                derive(
                    &pid,
                    &[
                        seeds::SEED_OUTCOME_MINT.to_vec(),
                        market_seed.clone(),
                        vec![outcome],
                    ],
                )
            })
            .collect();
        let mut holder_tokens: Vec<Pda> = (0..OUTCOME_COUNT)
            .map(|outcome| fixed_address(&format!("clutch/tok/{nonce}/{outcome}")))
            .collect();
        if nonce == NONCE_COMMITTED {
            holder_tokens[usize::from(WALK_OUTCOME_WIN)] = shared.actor_outcome_token.clone();
        }

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
            hoard_authority,
            hoard_token,
            outcome_mints,
            holder_tokens,
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
            authority: Hash32::from_bytes(self.hoard_authority.bytes),
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

    /// The six persisted state accounts shared by seam/gate transitions.
    ///
    /// The legacy ExternalAccount remains only as an offline-reference ghost;
    /// actual Token-2022 supply is the on-chain bearer truth. Resolution is a
    /// seventh founding target but is carried separately by the evidence gate.
    fn state_roles(&self) -> [(&'static str, &Pda); 6] {
        [
            ("market", &self.market),
            ("hoard", &self.hoard),
            ("position", &self.position),
            ("kernel", &self.kernel),
            ("replay", &self.replay),
            ("supply", &self.supply),
        ]
    }

    /// Whether this plane's market exists yet.
    ///
    /// A plane whose state bytes are all zero is a `CreateMarket` *target*: the
    /// instruction founds the market **and** creates its Hoard token account
    /// and its outcome mints, and `market_init::require_uncreated` demands that
    /// those addresses hold nothing at all.  So a founding plane installs no
    /// token accounts, and that absence is the precondition under test.
    fn founded(&self) -> bool {
        self.state.market.iter().any(|byte| *byte != 0)
    }

    /// The Token-2022 account images this plane's genesis installs.
    ///
    /// Each one is derived from the plane's own state bytes rather than
    /// chosen: the Hoard token account holds exactly
    /// `HoardAccount::collateral_atoms` (the mirror
    /// `token::require_hoard_mirror` re-checks over the pre-state before
    /// anything moves), and outcome mint *i* has exactly the supply-ledger's
    /// external term for outcome *i* (the reconciliation
    /// `require_shadow_reconciles` re-checks after every mint or burn).  A
    /// fixture that disagreed with its own state would be refused on chain,
    /// not silently accepted.
    fn token_accounts(&self, shared: &Shared) -> Vec<(String, Pda, Vec<u8>)> {
        if !self.founded() {
            return Vec::new();
        }
        let hoard = HoardAccount::decode(&self.state.hoard).expect("a founded Hoard decodes");
        let ledger =
            SupplyLedgerAccount::decode(&self.state.supply).expect("a founded ledger decodes");
        let mint = shared.collateral_mint.bytes;

        let position =
            PositionAccount::decode(&self.state.position).expect("a founded position decodes");
        let custody = hoard
            .collateral_atoms
            .checked_add(position.cash_atoms)
            .expect("fixture custody fits u64");
        let hoard_token_bytes =
            immutable_owner_account_bytes(mint, self.hoard_authority.bytes, custody);
        assert_admitted_token_account(
            &hoard_token_bytes,
            &token::TokenAccountPolicy::hoard(
                &shared.policy,
                solana_pubkey_of(self.hoard_authority.bytes),
            ),
            &format!("{}.hoard-token", self.label),
        );
        let mut out = vec![(
            format!("{}.hoard-token", self.label),
            self.hoard_token.clone(),
            hoard_token_bytes,
        )];

        for outcome in 0..usize::from(OUTCOME_COUNT) {
            let supply = ledger.external_supply[outcome];
            let mint_bytes_image = mint_bytes(Some(self.market.bytes), 0, supply);
            assert_admitted_mint(
                &self.outcome_mints[outcome].bytes,
                &mint_bytes_image,
                &token::MintPolicy::outcome(
                    solana_pubkey_of(self.outcome_mints[outcome].bytes),
                    solana_pubkey_of(self.market.bytes),
                ),
                &format!("{}.outcome-mint-{outcome}", self.label),
            );
            let holder_bytes = token_account_bytes(
                self.outcome_mints[outcome].bytes,
                shared.actor.bytes,
                supply,
            );
            assert_admitted_token_account(
                &holder_bytes,
                &token::TokenAccountPolicy::holder(
                    solana_pubkey_of(self.outcome_mints[outcome].bytes),
                    solana_pubkey_of(shared.actor.bytes),
                ),
                &format!("{}.holder-token-{outcome}", self.label),
            );
            out.push((
                format!("{}.outcome-mint-{outcome}", self.label),
                self.outcome_mints[outcome].clone(),
                mint_bytes_image,
            ));
            out.push((
                format!("{}.holder-token-{outcome}", self.label),
                self.holder_tokens[outcome].clone(),
                holder_bytes,
            ));
        }
        out
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
    /// Base58 address of the program that owns the account.
    ///
    /// Not a constant any more: this plan installs Token-2022 mints and token
    /// accounts, which the token program must own for
    /// `token::check_mint`/`check_token_account` to admit them, and one
    /// System-owned account holding the creator's lamports.
    owner: String,
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
    /// Base58 address of this program, the default genesis owner.
    program: String,
    /// Base58 address of the Token-2022 program.
    token_program: String,
}

impl Plan {
    /// A program-owned genesis account: every account of the frozen layout.
    fn account(&mut self, role: &str, pda: &Pda, data: &[u8]) {
        let owner = self.program.clone();
        self.owned(role, pda, &owner, data);
    }

    /// A genesis account owned by the Token-2022 program.
    fn token_account(&mut self, role: &str, pda: &Pda, data: &[u8]) {
        let owner = self.token_program.clone();
        self.owned(role, pda, &owner, data);
    }

    fn owned(&mut self, role: &str, pda: &Pda, owner: &str, data: &[u8]) {
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
            owner: owner.to_string(),
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

/// Every Token-2022 account one transition moves, compared.
///
/// The expectations are **not** a second description of what a transfer, a
/// mint or a burn does.  Each one is a number the offline reference adapter
/// produced, written into the field the token program owns:
///
/// - the Hoard token account must end holding exactly the adapter's
///   `HoardAccount::collateral_atoms`, which is the mirror
///   `token::require_hoard_mirror` re-checks on chain;
/// - the actor's collateral account must end holding exactly what it started
///   with, less whatever the adapter says the Hoard gained (or plus whatever
///   the adapter says it lost); and
/// - an outcome mint's supply and its holder's balance must end at the
///   adapter's supply-ledger external term for that outcome, which is the
///   reconciliation `require_shadow_reconciles` re-checks on chain.
///
/// So this is the same claim the program enforces, stated over bytes the bank
/// returned, against numbers a second implementation computed.
fn token_compares(
    shared: &Shared,
    plane: &Plane,
    leg: Leg,
    post: &TransitionOutput,
) -> Vec<Compare> {
    match leg {
        Leg::Collateral => {
            let pre_hoard = HoardAccount::decode(&plane.state.hoard)
                .expect("the pre-state Hoard decodes")
                .collateral_atoms;
            let post_hoard = HoardAccount::decode(&post.hoard)
                .expect("the post-state Hoard decodes")
                .collateral_atoms;
            let pre_cash = PositionAccount::decode(&plane.state.position)
                .expect("the pre-state position decodes")
                .cash_atoms;
            let post_cash = PositionAccount::decode(&post.position)
                .expect("the post-state position decodes")
                .cash_atoms;
            let custody = pre_hoard
                .checked_add(pre_cash)
                .expect("fixture custody fits u64");
            assert_eq!(
                post_hoard.checked_add(post_cash),
                Some(custody),
                "a reclassification conserves pooled custody"
            );
            let hoard_pre = immutable_owner_account_bytes(
                shared.collateral_mint.bytes,
                plane.hoard_authority.bytes,
                custody,
            );
            vec![
                Compare {
                    role: format!("{}.hoard-token", plane.label),
                    address: plane.hoard_token.address.clone(),
                    expected: hoard_pre.clone(),
                    pre: hoard_pre,
                },
                Compare {
                    role: format!("{}.actor-collateral", plane.label),
                    address: shared.actor_token.address.clone(),
                    expected: shared.actor_token_bytes.clone(),
                    pre: shared.actor_token_bytes.clone(),
                },
            ]
        }
        Leg::Outcome(outcome) => {
            let index = usize::from(outcome);
            let pre_external = SupplyLedgerAccount::decode(&plane.state.supply)
                .expect("the pre-state ledger decodes")
                .external_supply[index];
            let post_external = SupplyLedgerAccount::decode(&post.supply)
                .expect("the post-state ledger decodes")
                .external_supply[index];
            let mint_pre = mint_bytes(Some(plane.market.bytes), 0, pre_external);
            let holder_pre = token_account_bytes(
                plane.outcome_mints[index].bytes,
                shared.actor.bytes,
                pre_external,
            );
            vec![
                Compare {
                    role: format!("{}.outcome-mint-{outcome}", plane.label),
                    address: plane.outcome_mints[index].address.clone(),
                    expected: with_supply(&mint_pre, post_external),
                    pre: mint_pre,
                },
                Compare {
                    role: format!("{}.holder-token-{outcome}", plane.label),
                    address: plane.holder_tokens[index].address.clone(),
                    expected: with_amount(&holder_pre, post_external),
                    pre: holder_pre,
                },
            ]
        }
    }
}

/// The state accounts and the token accounts of one seam transition, together.
fn seam_compares(
    shared: &Shared,
    plane: &Plane,
    leg: Leg,
    post: &TransitionOutput,
) -> Vec<Compare> {
    let mut compares = state_compares(plane, post);
    compares.extend(token_compares(shared, plane, leg, post));
    compares
}

/* ------------------------------------------------------------------------ */
/* Instruction account lists                                                 */
/* ------------------------------------------------------------------------ */

/// Who signs one transaction, and which collateral account they present.
///
/// The two travel together because the program binds them together:
/// `TokenAccountPolicy::collateral_holder` requires the presented collateral
/// account's *owner authority* to be the authenticated actor, so "who signs"
/// and "whose collateral moves" is one decision, not two.  Separating them is
/// exactly what a hostile caller would try, and
/// [`Signer::presenting`] is how this plan drives that attempt.
#[derive(Clone, Copy, Debug)]
struct Signer<'a> {
    /// The address at the actor role.
    key: [u8; 32],
    /// Whether the message header authenticates it.
    signs: bool,
    /// The collateral token account this transaction presents.
    collateral: &'a Pda,
}

impl<'a> Signer<'a> {
    /// A signer presenting **its own** collateral account.
    fn own(shared: &'a Shared, key: [u8; 32], signs: bool) -> Self {
        let collateral = if key == shared.stranger.bytes {
            &shared.stranger_token
        } else {
            &shared.actor_token
        };
        Self {
            key,
            signs,
            collateral,
        }
    }

    /// A signer presenting **someone else's** collateral account.
    fn presenting(self, collateral: &'a Pda) -> Self {
        Self { collateral, ..self }
    }
}

/// Which mandatory token leg a seam intent carries.
///
/// **Not a choice this harness makes.**  `split::select_token_leg` is a total
/// function of the intent -- `Split` and `Merge` take the sixteen-account
/// collateral plane, `Materialize` and `Dematerialize` take the thirteen-account
/// outcome plane -- and presenting any other count is
/// `ClutchError::AccountCount`.  This enum exists so the emitter says out loud
/// which plane it is building, and the assertion in [`Leg::account_count`] pins
/// it to the program's own constants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Leg {
    /// `Split` and `Merge`: collateral moves between the actor and the Hoard.
    Collateral,
    /// `Materialize` and `Dematerialize`: one outcome token is minted or burned.
    Outcome(u8),
}

impl Leg {
    /// The exact account count the program requires for this leg.
    ///
    /// Read out of `clutch_sbf::instructions::split` rather than written here,
    /// so a future plane change is a compile-time re-read and not a silent
    /// drift.
    const fn account_count(self) -> usize {
        match self {
            Leg::Collateral => {
                clutch_sbf::instructions::split::ACCOUNT_PREFIX_COLLATERAL + OUTCOME_COUNT as usize
            }
            Leg::Outcome(_) => {
                clutch_sbf::instructions::split::ACCOUNT_PREFIX_OUTCOME + OUTCOME_COUNT as usize
            }
        }
    }
}

/// The token accounts one seam leg appends, in the program's list order.
fn seam_leg_accounts(
    shared: &Shared,
    plane: &Plane,
    signer: Signer<'_>,
    leg: Leg,
) -> Vec<[u8; 32]> {
    match leg {
        Leg::Collateral => {
            let mut accounts = vec![
                shared.token_program,
                shared.policy_account.bytes,
                shared.collateral_mint.bytes,
                signer.collateral.bytes,
                plane.hoard_authority.bytes,
                plane.hoard_token.bytes,
            ];
            accounts.extend(plane.outcome_mints.iter().map(|mint| mint.bytes));
            accounts
        }
        Leg::Outcome(outcome) => {
            let mut accounts = vec![
                shared.token_program,
                plane.holder_tokens[usize::from(outcome)].bytes,
            ];
            accounts.extend(plane.outcome_mints.iter().map(|mint| mint.bytes));
            accounts
        }
    }
}

/// Build the seam plane's instruction against a message.
fn seam_instruction(
    message: &Message,
    shared: &Shared,
    plane: &Plane,
    signer: Signer<'_>,
    replay_override: Option<[u8; 32]>,
    leg: Leg,
    data: Vec<u8>,
) -> Instruction {
    let replay = replay_override.unwrap_or(plane.replay.bytes);
    let mut keys = vec![
        signer.key,
        shared.realm.bytes,
        shared.profile.bytes,
        plane.market.bytes,
        plane.hoard.bytes,
        plane.position.bytes,
        plane.kernel.bytes,
        replay,
        plane.supply.bytes,
    ];
    keys.extend(seam_leg_accounts(shared, plane, signer, leg));
    assert_eq!(
        keys.len(),
        leg.account_count(),
        "the emitted seam plane must be exactly the plane the program requires"
    );
    Instruction {
        program_index: message.index(&shared.program.bytes),
        accounts: message.indices(&keys),
        data,
    }
}

/// The message every seam transaction uses.
///
/// The token leg decides two of the four groups: on the collateral plane the
/// actor's and the Hoard's token accounts are writable and the policy, the
/// mint, the Hoard authority and the token program are read-only; on the
/// outcome plane the mint and the holder account are writable and only the
/// token program is read-only.  The program checks every one of those bits
/// (`NotWritable`, `UnexpectedWritable`), so the message header is part of what
/// is under test rather than plumbing.
fn seam_message(
    shared: &Shared,
    plane: &Plane,
    signer: Signer<'_>,
    replay_override: Option<[u8; 32]>,
    leg: Leg,
) -> Message {
    let replay = replay_override.unwrap_or(plane.replay.bytes);
    let mut writable = vec![
        plane.market.bytes,
        plane.hoard.bytes,
        plane.position.bytes,
        plane.kernel.bytes,
        replay,
        plane.supply.bytes,
    ];
    let mut readonly = vec![
        shared.realm.bytes,
        shared.profile.bytes,
        shared.program.bytes,
        shared.compute_budget,
        shared.token_program,
    ];
    match leg {
        Leg::Collateral => {
            writable.push(signer.collateral.bytes);
            writable.push(plane.hoard_token.bytes);
            readonly.push(shared.policy_account.bytes);
            readonly.push(shared.collateral_mint.bytes);
            readonly.push(plane.hoard_authority.bytes);
            readonly.extend(plane.outcome_mints.iter().map(|mint| mint.bytes));
        }
        Leg::Outcome(outcome) => {
            writable.push(plane.outcome_mints[usize::from(outcome)].bytes);
            writable.push(plane.holder_tokens[usize::from(outcome)].bytes);
            readonly.extend(
                plane
                    .outcome_mints
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != usize::from(outcome))
                    .map(|(_, mint)| mint.bytes),
            );
        }
    }
    if signer.signs {
        Message::new(&[shared.payer.bytes], &[signer.key], &writable, &readonly)
    } else {
        readonly.insert(0, signer.key);
        Message::new(&[shared.payer.bytes], &[], &writable, &readonly)
    }
}

/// One whole seam transaction: the compute-budget raise, then the instruction.
///
/// The raise is no longer optional for any family.  MEASURED: a `Split` that
/// moves real collateral recomputes the 266-byte policy digest, recomputes the
/// parent Profile hash, admits a mint and two token accounts, and performs a
/// `TransferChecked` CPI, which does not fit the runtime's 200 000-unit
/// default.  A real caller raises the limit the same way; the raised number is
/// itself the measurement, recorded in `docs/implementation/SBF_BRINGUP.md`.
fn seam_transaction(
    shared: &Shared,
    plane: &Plane,
    signer: Signer<'_>,
    replay_override: Option<[u8; 32]>,
    leg: Leg,
    data: Vec<u8>,
) -> Vec<u8> {
    let message = seam_message(shared, plane, signer, replay_override, leg);
    let instruction = seam_instruction(&message, shared, plane, signer, replay_override, leg, data);
    transaction(
        &message,
        &[
            budget_instruction(&message, &shared.compute_budget, COMPUTE_UNIT_CEILING),
            instruction,
        ],
    )
}

/// The message and instruction of one evidence-gated transaction.
///
/// `redeems` selects the plane: `Resolve` moves no value and keeps the twelve
/// evidence accounts unchanged, while `RedeemInternal` pays collateral out and
/// takes nineteen -- the twelve, then the Profile the 266 policy bytes are
/// bound to, the token program, the policy, the collateral mint, the
/// redeemer's own collateral account, the Hoard's signing authority, and the
/// Hoard token account.
#[allow(clippy::too_many_arguments)]
fn gate_transaction(
    shared: &Shared,
    plane: &Plane,
    buffer: &Pda,
    signer: Signer<'_>,
    resolution_writable: bool,
    redeems: bool,
    data: Vec<u8>,
) -> Vec<u8> {
    let mut writable = vec![
        plane.market.bytes,
        plane.hoard.bytes,
        plane.position.bytes,
        plane.kernel.bytes,
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
    let mut keys = vec![
        signer.key,
        plane.market.bytes,
        plane.hoard.bytes,
        plane.position.bytes,
        plane.kernel.bytes,
        plane.replay.bytes,
        plane.supply.bytes,
        shared.terms.bytes,
        plane.resolution.bytes,
        shared.feed_head.bytes,
        buffer.bytes,
    ];
    if redeems {
        writable.push(signer.collateral.bytes);
        writable.push(plane.hoard_token.bytes);
        readonly.push(shared.profile.bytes);
        readonly.push(shared.token_program);
        readonly.push(shared.policy_account.bytes);
        readonly.push(shared.collateral_mint.bytes);
        readonly.push(plane.hoard_authority.bytes);
        keys.extend([
            shared.profile.bytes,
            shared.token_program,
            shared.policy_account.bytes,
            shared.collateral_mint.bytes,
            signer.collateral.bytes,
            plane.hoard_authority.bytes,
            plane.hoard_token.bytes,
        ]);
    }
    readonly.extend(plane.outcome_mints.iter().map(|mint| mint.bytes));
    keys.extend(plane.outcome_mints.iter().map(|mint| mint.bytes));
    assert_eq!(
        keys.len(),
        if redeems {
            clutch_sbf::instructions::observe_resolve::REDEEM_ACCOUNT_PREFIX
                + usize::from(OUTCOME_COUNT)
        } else {
            clutch_sbf::instructions::observe_resolve::EVIDENCE_ACCOUNT_PREFIX
                + usize::from(OUTCOME_COUNT)
        },
        "the emitted evidence plane must be exactly the plane the program requires"
    );
    let message = if signer.signs {
        Message::new(&[shared.payer.bytes], &[signer.key], &writable, &readonly)
    } else {
        let mut readonly_unsigned = readonly.clone();
        readonly_unsigned.insert(0, signer.key);
        Message::new(&[shared.payer.bytes], &[], &writable, &readonly_unsigned)
    };
    let instruction = Instruction {
        program_index: message.index(&shared.program.bytes),
        accounts: message.indices(&keys),
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

/// The message and instruction of one `Endow` transaction.
///
/// Thirteen accounts: a first endowment may create the owner's Position and
/// Replay through System CPI, then transfers admitted Token-2022 collateral
/// into pooled custody. The owner is a writable signer because it funds that
/// owner plane when absent.
fn endow_transaction(shared: &Shared, plane: &Plane, signer: Signer<'_>, data: Vec<u8>) -> Vec<u8> {
    endow_transaction_at(shared, plane, &plane.position, &plane.replay, signer, data)
}

fn endow_transaction_at(
    shared: &Shared,
    plane: &Plane,
    position: &Pda,
    replay: &Pda,
    signer: Signer<'_>,
    data: Vec<u8>,
) -> Vec<u8> {
    let writable = [
        position.bytes,
        replay.bytes,
        signer.collateral.bytes,
        plane.hoard_token.bytes,
    ];
    let mut readonly = vec![
        plane.market.bytes,
        plane.hoard.bytes,
        shared.profile.bytes,
        shared.policy_account.bytes,
        shared.token_program,
        shared.collateral_mint.bytes,
        shared.system_program,
        shared.rent_sysvar,
        shared.program.bytes,
        shared.compute_budget,
    ];
    let message = if signer.signs {
        let writable_signers = if signer.key == shared.payer.bytes {
            vec![shared.payer.bytes]
        } else {
            vec![shared.payer.bytes, signer.key]
        };
        Message::new(&writable_signers, &[], &writable, &readonly)
    } else {
        readonly.insert(0, signer.key);
        Message::new(&[shared.payer.bytes], &[], &writable, &readonly)
    };
    let keys = [
        signer.key,
        plane.market.bytes,
        plane.hoard.bytes,
        position.bytes,
        replay.bytes,
        shared.profile.bytes,
        shared.policy_account.bytes,
        shared.token_program,
        shared.collateral_mint.bytes,
        signer.collateral.bytes,
        plane.hoard_token.bytes,
        shared.system_program,
        shared.rent_sysvar,
    ];
    assert_eq!(
        keys.len(),
        clutch_sbf::instructions::genesis::ENDOW_ACCOUNT_COUNT,
        "the emitted endowment plane must be exactly the plane the program requires"
    );
    let instruction = Instruction {
        program_index: message.index(&shared.program.bytes),
        accounts: message.indices(&keys),
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

/// The `Endow` intent one credit carries.
fn endow_request(plane: &Plane, sequence: u64, amount: u64) -> Vec<u8> {
    endow_request_for(plane, plane.owner, sequence, amount)
}

fn endow_request_for(plane: &Plane, owner: Hash32, sequence: u64, amount: u64) -> Vec<u8> {
    layout_request(
        sequence,
        Intent::Endow {
            market: plane.market_id,
            owner,
            amount,
        },
    )
}

fn owner_plane(shared: &Shared, plane: &Plane, owner: [u8; 32]) -> (Pda, Pda) {
    let pid = shared.program.address.clone();
    let market = plane.market_id.bytes().to_vec();
    let owner = owner.to_vec();
    let position = derive(
        &pid,
        &[seeds::SEED_POSITION.to_vec(), market.clone(), owner.clone()],
    );
    let replay = derive(
        &pid,
        &[
            seeds::SEED_REPLAY.to_vec(),
            market,
            owner,
            0_u64.to_le_bytes().to_vec(),
        ],
    );
    (position, replay)
}

fn first_endow_owner_bytes(
    plane: &Plane,
    owner: [u8; 32],
    position: &Pda,
    replay: &Pda,
    amount: u64,
) -> (Vec<u8>, Vec<u8>) {
    let mut position_bytes = vec![0_u8; account_len::POSITION];
    PositionAccount {
        market: plane.market_id,
        owner: Hash32::from_bytes(owner),
        generation: 0,
        internal: [0; MAX_OUTCOMES],
        cash_atoms: amount,
        reserved_cash_atoms: 0,
        stored_bump: position.bump,
        close_state: 0,
    }
    .encode(&mut position_bytes)
    .expect("second owner position encodes");
    let mut replay_bytes = vec![0_u8; REPLAY_ACCOUNT_LEN];
    ReplayAccount {
        market: plane.market_id,
        owner: Hash32::from_bytes(owner),
        position_generation: 0,
        sequence: 1,
        stored_bump: replay.bump,
        flags: 0,
    }
    .encode(&mut replay_bytes)
    .expect("second owner replay encodes");
    (position_bytes, replay_bytes)
}

/// The position and replay bytes one `Endow` must produce.
///
/// **This is the weakest oracle in the plan and it is labelled as one.**  The
/// offline reference adapter refuses `Intent::Endow` with `UnsupportedIntent`
/// -- it models no endowment -- so there is no second implementation to
/// disagree with.  What is compared is still bytes against bytes: the two
/// fields the transition moves are written here through the *frozen*
/// `PositionAccount` and `ReplayAccount` codecs, and every other byte of both
/// accounts must come back unchanged, which is what would catch a transition
/// that touched something it should not have.
fn endow_post(plane: &Plane, amount: u64) -> (Vec<u8>, Vec<u8>) {
    let mut position =
        PositionAccount::decode(&plane.state.position).expect("the pre-state position decodes");
    let mut replay =
        ReplayAccount::decode(&plane.state.replay).expect("the pre-state replay decodes");
    position.cash_atoms = position
        .cash_atoms
        .checked_add(amount)
        .expect("the endowed cash must not overflow");
    replay.sequence += 1;
    let mut position_bytes = [0_u8; account_len::POSITION];
    let mut replay_bytes = [0_u8; REPLAY_ACCOUNT_LEN];
    position
        .encode(&mut position_bytes)
        .expect("the endowed position encodes");
    replay
        .encode(&mut replay_bytes)
        .expect("the endowed replay encodes");
    (position_bytes.to_vec(), replay_bytes.to_vec())
}

/// The two ledger accounts and two Token-2022 accounts one `Endow` writes.
fn endow_compares(shared: &Shared, plane: &Plane, amount: u64) -> Vec<Compare> {
    let (position, replay) = endow_post(plane, amount);
    let hoard = HoardAccount::decode(&plane.state.hoard).expect("the pre-state Hoard decodes");
    let pre_position =
        PositionAccount::decode(&plane.state.position).expect("the pre-state position decodes");
    let custody = hoard
        .collateral_atoms
        .checked_add(pre_position.cash_atoms)
        .expect("fixture custody fits u64");
    let hoard_pre = immutable_owner_account_bytes(
        shared.collateral_mint.bytes,
        plane.hoard_authority.bytes,
        custody,
    );
    vec![
        compare_of(plane, "position", &position),
        compare_of(plane, "replay", &replay),
        Compare {
            role: format!("{}.hoard-token", plane.label),
            address: plane.hoard_token.address.clone(),
            expected: with_amount(&hoard_pre, custody + amount),
            pre: hoard_pre,
        },
        Compare {
            role: "actor-collateral".to_string(),
            address: shared.actor_token.address.clone(),
            expected: with_amount(&shared.actor_token_bytes, ACTOR_COLLATERAL_ATOMS - amount),
            pre: shared.actor_token_bytes.clone(),
        },
    ]
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
///
/// Eighteen accounts plus one mint per outcome. Three things about this list
/// are load-bearing and none of them is decoration:
///
/// 1. **the creator is a writable signer.**  It is the rent payer for every
///    account this instruction founds, so the runtime has to have been told its
///    lamports may fall; `market_init::process` refuses a read-only creator
///    with `ClutchError::NotWritable` before it derives anything;
/// 2. **the Hoard token account and every outcome mint arrive uncreated.**
///    They are writable, hold nothing, and are not in the genesis dump at all,
///    which is exactly what `require_uncreated` demands -- the instruction
///    creates them by System-program CPI; and
/// 3. **the System program and the Rent sysvar are in the list**, because the
///    creation CPI needs the first and the rent-exempt minimum is read off the
///    second rather than pinned as a constant.
fn create_transaction(
    shared: &Shared,
    plane: &Plane,
    creator: [u8; 32],
    creator_signs: bool,
    data: Vec<u8>,
) -> Vec<u8> {
    let mut writable = vec![
        plane.market.bytes,
        plane.hoard.bytes,
        plane.position.bytes,
        plane.kernel.bytes,
        plane.replay.bytes,
        plane.supply.bytes,
        plane.resolution.bytes,
        plane.hoard_token.bytes,
    ];
    writable.extend(plane.outcome_mints.iter().map(|mint| mint.bytes));
    let readonly = [
        shared.realm.bytes,
        shared.profile.bytes,
        shared.terms.bytes,
        shared.program.bytes,
        shared.compute_budget,
        shared.policy_account.bytes,
        shared.token_program,
        shared.collateral_mint.bytes,
        shared.system_program,
        shared.rent_sysvar,
        plane.hoard_authority.bytes,
    ];
    let message = if creator_signs {
        /* The creator is a *writable* signer here and a read-only one nowhere:
         * it pays rent, so it is in the first group rather than the second. */
        Message::new(&[shared.payer.bytes, creator], &[], &writable, &readonly)
    } else {
        let mut readonly_unsigned = readonly.to_vec();
        readonly_unsigned.insert(0, creator);
        Message::new(&[shared.payer.bytes], &[], &writable, &readonly_unsigned)
    };
    let mut keys = vec![
        creator,
        shared.realm.bytes,
        shared.profile.bytes,
        shared.terms.bytes,
        plane.market.bytes,
        plane.hoard.bytes,
        plane.position.bytes,
        plane.kernel.bytes,
        plane.replay.bytes,
        plane.supply.bytes,
        plane.resolution.bytes,
        shared.policy_account.bytes,
        shared.token_program,
        shared.collateral_mint.bytes,
        shared.system_program,
        shared.rent_sysvar,
        plane.hoard_authority.bytes,
        plane.hoard_token.bytes,
    ];
    keys.extend(plane.outcome_mints.iter().map(|mint| mint.bytes));
    assert_eq!(
        keys.len(),
        clutch_sbf::instructions::market_init::account_count(OUTCOME_COUNT),
        "the emitted founding plane must be exactly the plane the program requires"
    );
    let instruction = Instruction {
        program_index: message.index(&shared.program.bytes),
        accounts: message.indices(&keys),
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
        authority: Hash32::from_bytes(plane.hoard_authority.bytes),
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
    let signer = Signer::own(shared, actor, true);
    let message = seam_message(shared, &f.seam, signer, None, Leg::Collateral);
    let instruction = seam_instruction(
        &message,
        shared,
        &f.seam,
        signer,
        None,
        Leg::Collateral,
        split.clone(),
    );
    cases.push(Case::accept(
        "split",
        "Split",
        "reference::apply",
        "one Split of five complete sets on the sixteen-account collateral plane: real Token-2022 collateral moves from the actor to the Hoard",
        transaction(
            &message,
            &[
                budget_instruction(&message, &shared.compute_budget, COMPUTE_UNIT_CEILING),
                instruction.clone(),
            ],
        ),
        1,
        seam_compares(shared, &f.seam, Leg::Collateral, &split_post),
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
    let merge_instruction = seam_instruction(
        &message,
        shared,
        &f.seam,
        signer,
        None,
        Leg::Collateral,
        merge_back.clone(),
    );
    let roundtrip_compares = seam_compares(shared, &f.seam, Leg::Collateral, &roundtrip_post);
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
        "Split then Merge as two instructions of one transaction; every account except the replay sequence must return to its pre-state, the two token accounts included",
        transaction(
            &message,
            &[
                budget_instruction(&message, &shared.compute_budget, COMPUTE_UNIT_CEILING),
                instruction,
                merge_instruction,
            ],
        ),
        2,
        roundtrip_compares,
    ));

    /* Refusal: the position owner is present but never signed. */
    cases.push(Case::refuse(
        "split-unsigned",
        "Split",
        "the position owner is present, read-only, and never signed",
        seam_transaction(
            shared,
            &f.seam,
            Signer::own(shared, actor, false),
            None,
            Leg::Collateral,
            split.clone(),
        ),
        code::MISSING_SIGNATURE,
        refusal_text(
            f.seam
                .layout(shared, &split, actor, false)
                .expect_err("the oracle refuses an unsigned Split"),
        ),
    ));

    /* Refusal: an authenticated signer who is not the position owner. */
    cases.push(Case::refuse(
        "split-stranger",
        "Split",
        "a different authenticated signer presents the owner's position",
        seam_transaction(
            shared,
            &f.seam,
            Signer::own(shared, shared.stranger.bytes, true),
            None,
            Leg::Collateral,
            split.clone(),
        ),
        code::UNAUTHORIZED_ACTOR,
        refusal_text(
            f.seam
                .layout(shared, &split, shared.stranger.bytes, true)
                .expect_err("the oracle refuses a stranger's Split"),
        ),
    ));

    /* Refusal: byte-identical replay state at a non-canonical address. */
    let imposter_transaction = seam_transaction(
        shared,
        &f.seam,
        Signer::own(shared, actor, true),
        Some(shared.imposter.bytes),
        Leg::Collateral,
        split.clone(),
    );
    let mut imposter_metadata = f.seam.metadata(shared, actor, true);
    imposter_metadata.replay.key = Hash32::from_bytes(shared.imposter.bytes);
    cases.push(Case::refuse(
        "split-imposter",
        "Split",
        "byte-identical replay state at an address that is not the canonical replay PDA",
        imposter_transaction,
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

    /* Refusal: the owner signs, but funds the complete sets from an account
     * it does not own.  NEW WITH THE MANDATORY COLLATERAL LEG, and the reason
     * the leg has to name the actor at all: without the owner-authority check
     * a `Split` could be paid for out of anyone's wallet.  The offline
     * reference adapter models no token plane, so this refusal has no oracle
     * on the other side and says so. */
    cases.push(Case::refuse(
        "split-foreign-collateral",
        "Split",
        "the position owner signs but presents a collateral account owned by someone else",
        seam_transaction(
            shared,
            &f.seam,
            Signer::own(shared, actor, true).presenting(&shared.stranger_token),
            None,
            Leg::Collateral,
            split.clone(),
        ),
        code::TOKEN_ACCOUNT_NOT_ADMITTED,
        "n/a (the offline adapter has no collateral leg)".to_string(),
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
    cases.push(Case::accept(
        "merge",
        "Merge",
        "reference::apply",
        "merge five complete sets back into cash on a market holding twenty; the Hoard signs its own collateral out",
        seam_transaction(
            shared,
            &f.held,
            Signer::own(shared, actor, true),
            None,
            Leg::Collateral,
            merge.clone(),
        ),
        1,
        seam_compares(shared, &f.held, Leg::Collateral, &merge_post),
    ));

    cases.push(Case::refuse(
        "merge-unsigned",
        "Merge",
        "the position owner is present, read-only, and never signed",
        seam_transaction(
            shared,
            &f.held,
            Signer::own(shared, actor, false),
            None,
            Leg::Collateral,
            merge.clone(),
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
        seam_transaction(
            shared,
            &f.held,
            Signer::own(shared, actor, true),
            None,
            Leg::Collateral,
            overdraw.clone(),
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
        "move three atoms of outcome zero from the internal ledger to the external shadow: the outcome mint really mints and the holder account really receives",
        seam_transaction(
            shared,
            &f.held,
            Signer::own(shared, actor, true),
            None,
            Leg::Outcome(0),
            materialize.clone(),
        ),
        1,
        seam_compares(shared, &f.held, Leg::Outcome(0), &materialize_post),
    ));

    cases.push(Case::refuse(
        "materialize-unsigned",
        "Materialize",
        "the position owner is present, read-only, and never signed",
        seam_transaction(
            shared,
            &f.held,
            Signer::own(shared, actor, false),
            None,
            Leg::Outcome(0),
            materialize.clone(),
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
        seam_transaction(
            shared,
            &f.held,
            Signer::own(shared, actor, true),
            None,
            Leg::Outcome(0),
            wrong_destination.clone(),
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
    cases.push(Case::accept(
        "dematerialize",
        "Dematerialize",
        "reference::apply",
        "move three atoms of outcome zero from the external shadow back to the internal ledger: the holder's tokens are really burned",
        seam_transaction(
            shared,
            &f.shadow,
            Signer::own(shared, actor, true),
            None,
            Leg::Outcome(0),
            dematerialize.clone(),
        ),
        1,
        seam_compares(shared, &f.shadow, Leg::Outcome(0), &dematerialize_post),
    ));

    cases.push(Case::refuse(
        "dematerialize-unsigned",
        "Dematerialize",
        "the position owner is present, read-only, and never signed",
        seam_transaction(
            shared,
            &f.shadow,
            Signer::own(shared, actor, false),
            None,
            Leg::Outcome(0),
            dematerialize.clone(),
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
        seam_transaction(
            shared,
            &f.shadow,
            Signer::own(shared, actor, true),
            None,
            Leg::Outcome(0),
            demat_overdraw.clone(),
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
            Signer::own(shared, actor, true),
            true,
            false,
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
            Signer::own(shared, actor, false),
            true,
            false,
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
            Signer::own(shared, actor, true),
            true,
            false,
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
    let mut redeem_compares = seam_compares(shared, &f.redeem, Leg::Collateral, &redeem_post);
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
            Signer::own(shared, actor, true),
            false,
            true,
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
            Signer::own(shared, actor, false),
            false,
            true,
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
            Signer::own(shared, shared.stranger.bytes, true),
            false,
            true,
            redeem.clone(),
        ),
        code::REFERENCE_UNAUTHORIZED_ACTOR,
        refusal_text(
            f.redeem
                .gate(shared, &redeem, &[], false, shared.stranger.bytes, true)
                .expect_err("the oracle refuses a stranger's redemption"),
        ),
    ));

    /* Refusal: the redeemer signs, but the payout is directed into an account
     * it does not own.  The mirror image of `split-foreign-collateral`, on the
     * one instruction that pays collateral *out*. */
    cases.push(Case::refuse(
        "redeem-foreign-collateral",
        "RedeemInternal",
        "the claim owner signs but directs the payout into a collateral account owned by someone else",
        gate_transaction(
            shared,
            &f.redeem,
            &f.redeem_buffer,
            Signer::own(shared, actor, true).presenting(&shared.stranger_token),
            false,
            true,
            redeem.clone(),
        ),
        code::TOKEN_ACCOUNT_NOT_ADMITTED,
        "n/a (the offline adapter has no collateral leg)".to_string(),
    ));

    /* ---------------------------------------------------------------- */
    /* Endow, on the empty seam market                                   */
    /* ---------------------------------------------------------------- */
    /* The genesis plane's non-creating deposit transition.  The offline
     * reference still has no Endow oracle, so the expected ledger bytes come
     * from the frozen codecs; the value leg is stronger evidence: the real
     * Token-2022 program must debit the owner and credit pooled Hoard custody
     * by the exact requested amount in the same atomic transaction. */
    let endow = endow_request(&f.seam, 0, ENDOW_AMOUNT);
    cases.push(Case::accept(
        "endow",
        "Endow",
        "layout re-encode (the offline reference refuses Endow: UnsupportedIntent)",
        "deposit forty admitted collateral atoms into pooled custody and credit exactly forty atoms of position cash",
        endow_transaction(shared, &f.seam, Signer::own(shared, actor, true), endow.clone()),
        1,
        endow_compares(shared, &f.seam, ENDOW_AMOUNT),
    ));

    cases.push(Case::refuse(
        "endow-unsigned",
        "Endow",
        "the position owner is present, read-only, and never signed",
        endow_transaction(
            shared,
            &f.seam,
            Signer::own(shared, actor, false),
            endow.clone(),
        ),
        code::MISSING_SIGNATURE,
        "n/a (the offline adapter has no Endow)".to_string(),
    ));

    cases.push(Case::refuse(
        "endow-stranger",
        "Endow",
        "a different authenticated signer credits the owner's position; an endowment is the one genesis transition that is not permissionless",
        endow_transaction(
            shared,
            &f.seam,
            Signer::own(shared, shared.stranger.bytes, true),
            endow.clone(),
        ),
        code::UNAUTHORIZED_ACTOR,
        "n/a (the offline adapter has no Endow)".to_string(),
    ));

    cases.push(Case::refuse(
        "endow-skipped-sequence",
        "Endow",
        "the signed deposit skips the position's next replay sequence",
        endow_transaction(
            shared,
            &f.seam,
            Signer::own(shared, actor, true),
            endow_request(&f.seam, 1, ENDOW_AMOUNT),
        ),
        code::REPLAY,
        "n/a (the offline adapter has no Endow)".to_string(),
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

    /* Every family except `FeedAdvance` now carries a `SetComputeUnitLimit`
     * instruction ahead of the program instruction.  That is a change the
     * token plane forced and it is a MEASUREMENT, not a workaround: a `Split`
     * that moves real collateral recomputes a 266-byte policy digest and a
     * parent Profile hash, admits a mint and two token accounts, and performs
     * a `TransferChecked` CPI, and a `CreateMarket` additionally creates one
     * mint per outcome plus the Hoard token account through seven CPIs.  None
     * of that fits the runtime's 200 000-unit default.  `FeedAdvance` still
     * does, and is deliberately left without a raise so that the difference
     * stays visible in the recorded numbers. */
    for case in &mut cases {
        if case.family != "FeedAdvance" {
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
/// The plane the walk's endowment has already landed on.
///
/// Appended rather than inserted: the market identity is a function of the
/// nonce, so renumbering the walk would move every address in it for no
/// reason.  The *narrative* order is the step list's, not the nonce's.
const NONCE_WALK_ENDOWED: u64 = 28;
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
/// NO LONGER A FIXTURE FIELD.  It used to be the one number in the walk's
/// opening state that no instruction produced -- the walk wrote it into the
/// position account before any transaction ran -- and step 2 now *drives* it,
/// through the `Endow` instruction the genesis plane added.  The gap that
/// remains is narrower and is still named: an endowment credits internal cash
/// that **no collateral backs**, because the value leg (a Token-2022 transfer
/// into the market's Hoard) is constructed in `token.rs` and wired by nothing.
/// So the walk's opening cash now has a signer, a replay sequence, a log line
/// and a ceiling, and still has no deposit behind it.
const WALK_CASH: u64 = 64;

/// The walk's quantities.  Every terminal number is derived from these.
const WALK_SPLIT: u64 = 20;
const WALK_MATERIALIZE: u64 = 8;
const WALK_DEMATERIALIZE: u64 = 5;
const WALK_MERGE: u64 = 4;
/// A second owner's first deposit, used to prove permissionless Position and
/// Replay creation on the same market without conflating that owner with the
/// positionless bearer claimant.
const SECOND_ENDOW_AMOUNT: u64 = 6;

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
    endowed: Plane,
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
    fn planes(&self) -> [&Plane; 9] {
        [
            &self.found,
            &self.open,
            &self.endowed,
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
/// founding state whose replay account is at sequence zero and every step --
/// the endowment at step zero included -- consumes exactly one sequence.
fn walk_layout_request(plane: &Plane, step: usize) -> Vec<u8> {
    let market = plane.market_id;
    let owner = plane.owner;
    let sequence = step as u64;
    match step {
        0 => endow_request(plane, sequence, WALK_CASH),
        1 => layout_request(
            sequence,
            Intent::Split {
                market,
                owner,
                quantity: WALK_SPLIT,
            },
        ),
        2 => layout_request(
            sequence,
            Intent::Materialize {
                market,
                owner,
                destination: Hash32::from_bytes(plane.external.bytes),
                outcome: WALK_OUTCOME_WIN,
                quantity: WALK_MATERIALIZE,
            },
        ),
        3 => layout_request(
            sequence,
            Intent::Dematerialize {
                market,
                owner,
                source: Hash32::from_bytes(plane.external.bytes),
                outcome: WALK_OUTCOME_WIN,
                quantity: WALK_DEMATERIALIZE,
            },
        ),
        4 => layout_request(
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

/// The production SBF binding for bearer token accounts.
///
/// The offline reference adapter still models its retired owner-local External
/// account, so [`walk_layout_request`] deliberately names that ghost when it
/// computes expected economic state. The SBF program instead authenticates the
/// actual Token-2022 holder account. Only the address binding differs.
fn walk_sbf_layout_request(plane: &Plane, step: usize) -> Vec<u8> {
    let market = plane.market_id;
    let owner = plane.owner;
    let token = Hash32::from_bytes(plane.holder_tokens[usize::from(WALK_OUTCOME_WIN)].bytes);
    match step {
        2 => layout_request(
            step as u64,
            Intent::Materialize {
                market,
                owner,
                destination: token,
                outcome: WALK_OUTCOME_WIN,
                quantity: WALK_MATERIALIZE,
            },
        ),
        3 => layout_request(
            step as u64,
            Intent::Dematerialize {
                market,
                owner,
                source: token,
                outcome: WALK_OUTCOME_WIN,
                quantity: WALK_DEMATERIALIZE,
            },
        ),
        _ => walk_layout_request(plane, step),
    }
}

/// Run the walk forward over its first `steps` steps.
///
/// Every step but the first is the **offline reference adapter's** own
/// transition.  Step zero is the endowment, and it is the one step the
/// reference cannot run: `apply` refuses `Intent::Endow` with
/// `UnsupportedIntent`, so the credit is applied here through the frozen
/// `PositionAccount` and `ReplayAccount` codecs by [`endow_post`] -- the same
/// weaker oracle the `endow` case in the per-family plan declares, applied to
/// the same two fields.
fn walk_forward(shared: &Shared, plane: &mut Plane, steps: usize) {
    let actor = shared.actor.bytes;
    let window = encode_window(shared.feed, &winning_records());
    for step in 0..steps {
        if step == 0 {
            let (position, replay) = endow_post(plane, WALK_CASH);
            plane.state.position.copy_from_slice(&position);
            plane.state.replay.copy_from_slice(&replay);
            continue;
        }
        let output = match step {
            1..=4 => {
                let request = walk_layout_request(plane, step);
                plane.layout(shared, &request, actor, true)
            }
            5 => plane.gate(
                shared,
                &resolve_request(5, WINNING_PAYOUT_INDEX),
                &window,
                true,
                actor,
                true,
            ),
            6 => plane.gate(
                shared,
                &redeem_request(6, WALK_OUTCOME_WIN, walk_redeem_winning()),
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

/// Build one walk plane: `CreateMarket`'s post-state, walked forward.
fn walk_plane(shared: &Shared, label: &'static str, nonce: u64, steps: usize) -> Plane {
    let mut plane = Plane::build(shared, label, nonce, WALK_GENERATION);
    let (state, resolution) = founding_plane(shared, &plane, nonce);
    plane.state = state;
    plane.resolution_bytes = resolution;
    walk_forward(shared, &mut plane, steps);
    plane
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
    let endowed = walk_plane(shared, "walk-endowed", NONCE_WALK_ENDOWED, 1);
    let split = walk_plane(shared, "walk-split", NONCE_WALK_SPLIT, 2);
    let materialized = walk_plane(shared, "walk-materialized", NONCE_WALK_MATERIALIZED, 3);
    let dematerialized = walk_plane(shared, "walk-dematerialized", NONCE_WALK_DEMATERIALIZED, 4);
    let merged = walk_plane(shared, "walk-merged", NONCE_WALK_MERGED, 5);
    let resolved = walk_plane(shared, "walk-resolved", NONCE_WALK_RESOLVED, 6);
    let redeemed = walk_plane(shared, "walk-redeemed", NONCE_WALK_REDEEMED, 7);

    /* The last step's post-state, from the same forward run.  It is the walk's
     * terminal state and the thing the accounting identity is asserted over. */
    let terminal = redeemed
        .gate(
            shared,
            &redeem_request(7, WALK_OUTCOME_LOSE, walk_redeem_losing()),
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
        endowed,
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
    /* The opening state is EXACTLY what `CreateMarket` writes -- no field is
     * credited into it any more.  The walk's cash arrives at step 2, through
     * an `Endow` a signer authorized and a replay sequence counted. */
    let (founded, founded_resolution) = founding_plane(shared, &walk.open, NONCE_WALK_OPEN);
    assert_eq!(
        walk.open.resolution_bytes, founded_resolution,
        "the walk opens on the resolution record CreateMarket writes"
    );
    for (role, _) in walk.open.state_roles() {
        assert_eq!(
            walk.open.state_slice(role),
            output_slice(&founded, role),
            "the walk's opening {role} must be exactly what CreateMarket writes"
        );
    }
    assert_eq!(
        PositionAccount::decode(&walk.open.state.position)
            .expect("the opening position decodes")
            .cash_atoms,
        0,
        "the walk must open with no cash: the endowment is a step, not a fixture"
    );

    /* And the endowed plane is its **own** founding state plus exactly two
     * moved fields: the credited cash and the consumed replay sequence.  The
     * comparison is against a founding plane rebuilt at the endowed plane's
     * own nonce rather than against `walk.open`, because every account in a
     * plane carries its market identity and two planes at two nonces differ in
     * bytes that have nothing to do with the transition. */
    let mut founding_endowed =
        Plane::build(shared, "walk-endowed", NONCE_WALK_ENDOWED, WALK_GENERATION);
    let (endowed_founding_state, _) = founding_plane(shared, &founding_endowed, NONCE_WALK_ENDOWED);
    founding_endowed.state = endowed_founding_state;
    let (endowed_position, endowed_replay) = endow_post(&founding_endowed, WALK_CASH);
    assert_eq!(
        walk.endowed.state_slice("position"),
        endowed_position.as_slice(),
        "the endowed plane's position is its founding position plus the credit"
    );
    assert_eq!(
        walk.endowed.state_slice("replay"),
        endowed_replay.as_slice(),
        "an endowment consumes exactly one replay sequence"
    );
    for (role, _) in walk.endowed.state_roles() {
        if role == "position" || role == "replay" {
            continue;
        }
        assert_eq!(
            walk.endowed.state_slice(role),
            founding_endowed.state_slice(role),
            "an endowment must not move {role}"
        );
    }

    /* Each plane is the previous plane's step replayed on its own identity. */
    let stages: [(&Plane, usize); 8] = [
        (&walk.open, 0),
        (&walk.endowed, 1),
        (&walk.split, 2),
        (&walk.materialized, 3),
        (&walk.dematerialized, 4),
        (&walk.merged, 5),
        (&walk.resolved, 6),
        (&walk.redeemed, 7),
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
            name: "the kernel supply is exactly the cached bearer supply",
            equation: "sum_i kernel_total_supply_i == sum_i ledger_external_i".to_string(),
            left: (0..outcomes).map(|index| observed(TOTAL[index], 1)).collect(),
            right: (0..outcomes)
                .map(|index| observed(LEDGER_EXTERNAL[index], 1))
                .collect(),
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

    /* 2. Endow: the walk's opening cash, driven rather than conjured. */
    let endow = walk_layout_request(&walk.open, 0);
    cases.push(Case::accept(
        "walk-02-endow",
        "Lifecycle",
        "layout re-encode (the offline reference refuses Endow: UnsupportedIntent)",
        "credit the founding position's opening cash through the one instruction that credits it",
        endow_transaction(shared, &walk.open, Signer::own(shared, actor, true), endow),
        1,
        endow_compares(shared, &walk.open, WALK_CASH),
    ));
    steps.push(walk_step(
        2,
        "walk-02-endow",
        "endow the founding position",
        "2 (in part)",
        "The walk's opening cash used to be a number this harness wrote into the genesis position account: no signer, no sequence, no ceiling. It is now an instruction. What it still is not is a deposit -- `Endow` moves the internal ledger and no collateral, because the value leg is a Token-2022 transfer into the Hoard that `token.rs` constructs and nothing wires. The credit therefore has an author and a replay sequence and remains unbacked, and the Hoard's untouched bytes in this step are that statement driven.",
    ));

    /* 3. Split. */
    let split = walk_layout_request(&walk.endowed, 1);
    let split_post = walk
        .endowed
        .layout(shared, &split, actor, true)
        .expect("the walk splits");
    cases.push(Case::accept(
        "walk-03-split",
        "Lifecycle",
        "reference::apply",
        "split the endowed position into complete sets, moving real Token-2022 collateral into the Hoard",
        seam_transaction(
            shared,
            &walk.endowed,
            Signer::own(shared, actor, true),
            None,
            Leg::Collateral,
            split,
        ),
        1,
        seam_compares(shared, &walk.endowed, Leg::Collateral, &split_post),
    ));
    steps.push(walk_step(
        3,
        "walk-03-split",
        "split internally",
        "4",
        "One collateral debit credits one unit of every Egg. The complete set lives in the position's internal balances, and the Hoard's accounting and its Token-2022 account both rise by exactly the quantity split -- the two collateral truths the program refuses to let disagree.",
    ));

    /* 4. Materialize. */
    let materialize = walk_layout_request(&walk.split, 2);
    let materialize_post = walk
        .split
        .layout(shared, &materialize, actor, true)
        .expect("the walk materializes");
    cases.push(Case::accept(
        "walk-04-materialize",
        "Lifecycle",
        "reference::apply",
        "materialize part of the winning outcome into the external shadow, minting the outcome token that represents it",
        seam_transaction(
            shared,
            &walk.split,
            Signer::own(shared, actor, true),
            None,
            Leg::Outcome(WALK_OUTCOME_WIN),
            materialize,
        ),
        1,
        seam_compares(
            shared,
            &walk.split,
            Leg::Outcome(WALK_OUTCOME_WIN),
            &materialize_post,
        ),
    ));
    steps.push(walk_step(
        4,
        "walk-04-materialize",
        "materialize one Egg",
        "5",
        "Part of one outcome leaves the internal ledger for the external shadow, and a real Token-2022 `MintTo` signed by the market PDA is what makes it external. `total_i` is preserved exactly: what the position loses internally the shadow gains, the mint's supply is the market-wide external term, and the Hoard does not move.",
    ));

    /* 5. Dematerialize. */
    let dematerialize = walk_layout_request(&walk.materialized, 3);
    let dematerialize_post = walk
        .materialized
        .layout(shared, &dematerialize, actor, true)
        .expect("the walk dematerializes");
    cases.push(Case::accept(
        "walk-05-dematerialize",
        "Lifecycle",
        "reference::apply",
        "bring part of the materialized outcome back to the internal ledger, burning the outcome token",
        seam_transaction(
            shared,
            &walk.materialized,
            Signer::own(shared, actor, true),
            None,
            Leg::Outcome(WALK_OUTCOME_WIN),
            dematerialize,
        ),
        1,
        seam_compares(
            shared,
            &walk.materialized,
            Leg::Outcome(WALK_OUTCOME_WIN),
            &dematerialize_post,
        ),
    ));
    steps.push(walk_step(
        5,
        "walk-05-dematerialize",
        "dematerialize part of it",
        "5",
        "The reverse boundary crossing, for part of what was materialized: the holder's tokens are burned under the owner's own signature. The remainder stays outstanding on the external side for the rest of the walk and is what the terminal Hoard has to cover.",
    ));

    /* 6. Merge, while the market is still active. */
    let merge = walk_layout_request(&walk.dematerialized, 4);
    let merge_post = walk
        .dematerialized
        .layout(shared, &merge, actor, true)
        .expect("the walk merges");
    cases.push(Case::accept(
        "walk-06-merge",
        "Lifecycle",
        "reference::apply",
        "recombine complete sets into cash before resolution, returning collateral from the Hoard",
        seam_transaction(
            shared,
            &walk.dematerialized,
            Signer::own(shared, actor, true),
            None,
            Leg::Collateral,
            merge,
        ),
        1,
        seam_compares(shared, &walk.dematerialized, Leg::Collateral, &merge_post),
    ));
    steps.push(walk_step(
        6,
        "walk-06-merge",
        "merge complete sets back",
        "4",
        "The promise of section 1 exercised: a complete set can always be recombined into its collateral **before** resolution. Cash rises and the Hoard falls by the same quantity, and this is the one direction that is impossible without the program signing for the Hoard authority itself. Step 9 records what happens to the same request after resolution.",
    ));

    /* 7. Three FeedAdvance instructions, sequenced by the bank. */
    cases.push(Case::accept(
        "walk-07-feed-advance",
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
        7,
        "walk-07-feed-advance",
        "advance the shared feed three times",
        "6, 7",
        "Three observation pages fold into one feed head inside one transaction, so the bank -- not this harness -- sequences the chain and page three is read against page two's writes. The cursor moves 100 -> 102 -> 103 -> 104, and 104 is exactly the maturity bound the market's window needs before it can seal.",
    ));

    /* 8. Resolve. */
    let window = encode_window(shared.feed, &winning_records());
    let resolve = resolve_request(5, WINNING_PAYOUT_INDEX);
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
        "walk-08-resolve",
        "Lifecycle",
        "reference::apply_with_evidence",
        "seal the window from the observation records and resolve onto the cell they select",
        gate_transaction(
            shared,
            &walk.merged,
            &f.resolve_buffer,
            Signer::own(shared, actor, true),
            true,
            false,
            resolve.clone(),
        ),
        1,
        resolve_compares,
    );
    case.compute_limit = Some(COMPUTE_UNIT_CEILING);
    cases.push(case);
    steps.push(walk_step(
        8,
        "walk-08-resolve",
        "seal the window and resolve",
        "7, 9",
        "No reporter chooses the cell. The buffer carries observation records; the gate folds them through the accumulator's Open -> Mature -> Sealed machine against the terms' own window domain, reads the matured cursor off the feed head, and the payout index the caller named must be the one the sealed window selects.",
    ));

    /* 9. Merge after resolution: refused, and that is the point. */
    let late_merge = layout_request(
        6,
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
    cases.push(Case::refuse(
        "walk-09-merge-after-resolve",
        "Lifecycle",
        "recombine a complete set after resolution: the boundary section 1 draws, driven",
        seam_transaction(
            shared,
            &walk.resolved,
            Signer::own(shared, actor, true),
            None,
            Leg::Collateral,
            late_merge,
        ),
        code::NOT_ACTIVE,
        refusal_text(late_refusal),
    ));
    steps.push(walk_step(
        9,
        "walk-09-merge-after-resolve",
        "merge after resolution is refused",
        "4, 10",
        "The same complete-set merge that step 5 accepted is refused once the market has resolved. This is the boundary the product model draws -- recombination is a pre-resolution right -- and after this point the only way out of a claim is redemption, which is what makes the terminal accounting a redemption identity rather than a merge identity.",
    ));

    /* 10. Redeem the winning internal claims. */
    let redeem_win = redeem_request(6, WALK_OUTCOME_WIN, walk_redeem_winning());
    let redeem_win_post = walk
        .resolved
        .gate(shared, &redeem_win, &[], false, actor, true)
        .expect("the walk redeems its winning claims");
    assert_eq!(
        redeem_win_post.redemption_payout,
        walk_redeem_winning(),
        "the winning outcome pays one atom per claim"
    );
    let mut redeem_win_compares =
        seam_compares(shared, &walk.resolved, Leg::Collateral, &redeem_win_post);
    redeem_win_compares.push(compare_of(
        &walk.resolved,
        "resolution",
        &redeem_win_post
            .resolution
            .expect("a redemption returns the record unchanged"),
    ));
    let mut case = Case::accept(
        "walk-10-redeem-winning",
        "Lifecycle",
        "reference::apply_with_evidence",
        "redeem every internal claim on the winning outcome; the resolution record is read-only and must come back unchanged",
        gate_transaction(
            shared,
            &walk.resolved,
            &f.redeem_buffer,
            Signer::own(shared, actor, true),
            false,
            true,
            redeem_win,
        ),
        1,
        redeem_win_compares,
    );
    case.compute_limit = Some(COMPUTE_UNIT_CEILING);
    cases.push(case);
    steps.push(walk_step(
        10,
        "walk-10-redeem-winning",
        "redeem the winning internal claims",
        "9",
        "The first payoff shape: the unit vector on the realized cell. Collateral leaves the Hoard for the position's cash, one atom per claim, and the resolution record the redemption reads is presented read-only so a redemption can never edit its own authority.",
    ));

    /* 11. Redeem the losing claims: they pay zero. */
    let redeem_lose = redeem_request(7, WALK_OUTCOME_LOSE, walk_redeem_losing());
    let mut redeem_lose_compares =
        seam_compares(shared, &walk.redeemed, Leg::Collateral, &walk.terminal);
    redeem_lose_compares.push(compare_of(
        &walk.redeemed,
        "resolution",
        &walk
            .terminal
            .resolution
            .expect("a redemption returns the record unchanged"),
    ));
    let mut case = Case::accept(
        "walk-11-redeem-losing",
        "Lifecycle",
        "reference::apply_with_evidence",
        "redeem every internal claim on the losing outcome; the claims burn and the payout is exactly zero",
        gate_transaction(
            shared,
            &walk.redeemed,
            &f.redeem_buffer,
            Signer::own(shared, actor, true),
            false,
            true,
            redeem_lose,
        ),
        1,
        redeem_lose_compares,
    );
    case.compute_limit = Some(COMPUTE_UNIT_CEILING);
    cases.push(case);
    steps.push(walk_step(
        11,
        "walk-11-redeem-losing",
        "redeem the losing claims for zero",
        "9, 10",
        "The second payoff shape: the zero vector on an unrealized cell. The claims are burned and the Hoard does not move by one atom, which is the half of the solvency promise that is easy to state and easy to get wrong. After this the walk's internal ledger is empty and the terminal identity can be read.",
    ));

    let (values, identities) = walk_terminal(walk);

    let skips = vec![
        WalkSkip {
            project_item: "1 (in part)",
            title: "initialize a Realm",
            reason: "NOT DRIVEN THIS ROUND, and no longer for want of an instruction. `instructions::genesis` now implements `InitRealm`, `InitProfile`, `InitPriceGrid`, `InitTerms` and `InitOrderPage`, each creating its account through a real System-program CPI. This walk does not drive them and the per-family plan does not either: each needs its own fresh identity plane (a Realm nonce, a policy, a grid body and a terms body whose digests are not the ones already installed at genesis) and none has a reference oracle -- `reference::apply` refuses all five with `UnsupportedIntent`, so a differential would compare this program against a re-encode of its own intent. Their CPI has therefore never run on a bank. The walk drives `CreateMarket` and `Endow` of item 1's family; the Realm-wide plane is still loaded at genesis as frozen bytes the frozen codecs accept."
                .to_string(),
        },
        WalkSkip {
            project_item: "2 (in part)",
            title: "prepay all mandatory work",
            reason: "PARTLY DRIVEN, at step 2, and the residue is exact. The walk's opening cash is no longer a number this harness wrote into a genesis account: `Endow` credits it under the position owner's own signature, against the market's immutable collateral cap, consuming a replay sequence. What no instruction does is *back* that credit. The value leg is a Token-2022 `TransferChecked` into the market's Hoard token account; `token.rs` constructs exactly that CPI and no instruction wires it, so the endowed cash is an internal-ledger entry with no deposit behind it. `genesis.rs` says so itself, and the sufficient solvency check -- the sum of every position's cash plus the escrowed collateral -- needs a market-wide cash aggregate no account in the frozen layout carries."
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
        "Step 7 is the exception: three `FeedAdvance` instructions ride in one transaction against one writable feed head, so the bank sequences that chain itself."
            .to_string(),
        "The feed head step 7 advances and the feed head step 8 resolves against are two accounts of two feed identities, because the same address cannot hold both cursor 100 and cursor 104 in one genesis. The harness asserts that step 7's three advances land the cursor on exactly the value step 8's head carries, which is the only fact the resolve gate reads off a feed head."
            .to_string(),
        "No signature is verified anywhere in this walk: every transaction is simulated with `sigVerify: false`. The `is_signer` bits the program reads do come from the transaction message header, and that is the whole of what the authorization steps establish."
            .to_string(),
    ];

    Lifecycle {
        steps,
        skips,
        notes,
        cases,
        terminal_case: "walk-11-redeem-losing".to_string(),
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
            hoard_authority: source.hoard_authority.clone(),
            hoard_token: source.hoard_token.clone(),
            outcome_mints: source.outcome_mints.clone(),
            holder_tokens: source.holder_tokens.clone(),
            state: state.clone(),
            resolution_bytes: source.resolution_bytes,
        }
    }
}

/* ------------------------------------------------------------------------ */
/* One-address committed-bank walk                                           */
/* ------------------------------------------------------------------------ */

/// A market identity reserved for the signed, committing local-bank lane.
const NONCE_COMMITTED: u64 = 29;

/// State plus the pooled-custody token facts for a collateral
/// reclassification after the one backed Endow has committed.
///
/// The ordinary differential fixtures put each prestate on a fresh genesis
/// plane, so their shared actor token image always starts at the fixture
/// amount.  This walk does not: after Endow the actor remains debited and the
/// Hoard remains credited for every later step.  Naming that persistent fact
/// here is what prevents a same-address gate from accidentally comparing
/// against the per-plane fiction.
fn committed_custody_compares(
    shared: &Shared,
    plane: &Plane,
    post: &TransitionOutput,
) -> Vec<Compare> {
    let mut compares = state_compares(plane, post);
    let hoard_token = immutable_owner_account_bytes(
        shared.collateral_mint.bytes,
        plane.hoard_authority.bytes,
        WALK_CASH + SECOND_ENDOW_AMOUNT,
    );
    let actor_token = with_amount(
        &shared.actor_token_bytes,
        ACTOR_COLLATERAL_ATOMS - WALK_CASH - SECOND_ENDOW_AMOUNT,
    );
    compares.extend([
        Compare {
            role: format!("{}.hoard-token", plane.label),
            address: plane.hoard_token.address.clone(),
            expected: hoard_token.clone(),
            pre: hoard_token,
        },
        Compare {
            role: "actor-collateral".to_string(),
            address: shared.actor_token.address.clone(),
            expected: actor_token.clone(),
            pre: actor_token,
        },
    ]);
    compares
}

/// Exact token-account post-state of one ordinary System + Token-2022
/// construction transaction. The account is absent before the transaction,
/// so the empty `pre` image is provenance rather than an encoded token state.
fn constructed_token_compare(
    role: &str,
    address: &Pda,
    mint: [u8; 32],
    owner: [u8; 32],
) -> Compare {
    Compare {
        role: role.to_string(),
        address: address.address.clone(),
        expected: token_account_bytes(mint, owner, 0),
        pre: Vec::new(),
    }
}

fn second_owner_funding_compares(shared: &Shared) -> Vec<Compare> {
    let actor_pre = ACTOR_COLLATERAL_ATOMS - WALK_CASH;
    vec![
        Compare {
            role: "actor-collateral".to_string(),
            address: shared.actor_token.address.clone(),
            expected: with_amount(&shared.actor_token_bytes, actor_pre - SECOND_ENDOW_AMOUNT),
            pre: with_amount(&shared.actor_token_bytes, actor_pre),
        },
        Compare {
            role: "payer-collateral".to_string(),
            address: shared.payer_collateral_token.address.clone(),
            expected: token_account_bytes(
                shared.collateral_mint.bytes,
                shared.payer.bytes,
                SECOND_ENDOW_AMOUNT,
            ),
            pre: token_account_bytes(shared.collateral_mint.bytes, shared.payer.bytes, 0),
        },
    ]
}

fn second_owner_endow_compares(
    shared: &Shared,
    plane: &Plane,
    position: &Pda,
    replay: &Pda,
) -> Vec<Compare> {
    let (position_bytes, replay_bytes) = first_endow_owner_bytes(
        plane,
        shared.payer.bytes,
        position,
        replay,
        SECOND_ENDOW_AMOUNT,
    );
    vec![
        Compare {
            role: "payer-position".to_string(),
            address: position.address.clone(),
            expected: position_bytes,
            pre: Vec::new(),
        },
        Compare {
            role: "payer-replay".to_string(),
            address: replay.address.clone(),
            expected: replay_bytes,
            pre: Vec::new(),
        },
        Compare {
            role: "payer-collateral".to_string(),
            address: shared.payer_collateral_token.address.clone(),
            expected: token_account_bytes(shared.collateral_mint.bytes, shared.payer.bytes, 0),
            pre: token_account_bytes(
                shared.collateral_mint.bytes,
                shared.payer.bytes,
                SECOND_ENDOW_AMOUNT,
            ),
        },
        Compare {
            role: format!("{}.hoard-token", plane.label),
            address: plane.hoard_token.address.clone(),
            expected: immutable_owner_account_bytes(
                shared.collateral_mint.bytes,
                plane.hoard_authority.bytes,
                WALK_CASH + SECOND_ENDOW_AMOUNT,
            ),
            pre: immutable_owner_account_bytes(
                shared.collateral_mint.bytes,
                plane.hoard_authority.bytes,
                WALK_CASH,
            ),
        },
    ]
}

/// Byte expectations for the ordinary bearer transfer after the actor has
/// materialized and partly dematerialized the winning Egg.
fn bearer_transfer_compares(shared: &Shared, plane: &Plane, quantity: u64) -> Vec<Compare> {
    let index = usize::from(WALK_OUTCOME_WIN);
    let mint = mint_bytes(Some(plane.market.bytes), 0, quantity);
    vec![
        Compare {
            role: "actor-winning-egg".to_string(),
            address: plane.holder_tokens[index].address.clone(),
            expected: token_account_bytes(plane.outcome_mints[index].bytes, shared.actor.bytes, 0),
            pre: token_account_bytes(
                plane.outcome_mints[index].bytes,
                shared.actor.bytes,
                quantity,
            ),
        },
        Compare {
            role: "holder-winning-egg".to_string(),
            address: shared.holder_outcome_token.address.clone(),
            expected: token_account_bytes(
                plane.outcome_mints[index].bytes,
                shared.holder.bytes,
                quantity,
            ),
            pre: token_account_bytes(plane.outcome_mints[index].bytes, shared.holder.bytes, 0),
        },
        Compare {
            role: format!("{}.outcome-mint-{WALK_OUTCOME_WIN}", plane.label),
            address: plane.outcome_mints[index].address.clone(),
            expected: mint.clone(),
            pre: mint,
        },
    ]
}

/// Apply the exact unit-vector bearer redemption to the offline state image.
///
/// `RedeemExternal` deliberately has no owner Position and the legacy
/// reference adapter has no action for it, so this is a codec-level expected
/// image rather than a second execution oracle. The real gate separately
/// checks the Token-2022 burn, transfer, supply, and Hoard deltas.
fn bearer_redeem_post(plane: &Plane, quantity: u64) -> TransitionOutput {
    let index = usize::from(WALK_OUTCOME_WIN);
    let mut post = plane.state.clone();

    let mut hoard = HoardAccount::decode(&post.hoard).expect("pre-exit Hoard decodes");
    hoard.collateral_atoms = hoard
        .collateral_atoms
        .checked_sub(quantity)
        .expect("bearer payout is fully collateralized");
    hoard
        .encode(&mut post.hoard)
        .expect("post-exit Hoard encodes");

    let mut kernel = KernelAccount::decode(&post.kernel).expect("pre-exit kernel decodes");
    kernel.total_supply[index] = kernel.total_supply[index]
        .checked_sub(quantity)
        .expect("bearer supply covers redemption");
    kernel
        .encode(&mut post.kernel)
        .expect("post-exit kernel encodes");

    let mut supply = SupplyLedgerAccount::decode(&post.supply).expect("pre-exit supply decodes");
    supply.external_supply[index] = supply.external_supply[index]
        .checked_sub(quantity)
        .expect("cached bearer supply covers redemption");
    supply
        .encode(&mut post.supply)
        .expect("post-exit supply encodes");

    /* Keep the offline adapter's ghost representation internally coherent.
     * It is never presented to the SBF program after bearer truth lands. */
    let mut ghost = ExternalAccount::decode(&post.external).expect("pre-exit ghost decodes");
    ghost.balances[index] = ghost.balances[index]
        .checked_sub(quantity)
        .expect("ghost bearer balance covers redemption");
    ghost
        .encode(&mut post.external)
        .expect("post-exit ghost encodes");
    post
}

fn bearer_redeem_compares(
    shared: &Shared,
    plane: &Plane,
    post: &TransitionOutput,
    quantity: u64,
) -> Vec<Compare> {
    let index = usize::from(WALK_OUTCOME_WIN);
    let mut compares = vec![
        compare_of(plane, "hoard", &post.hoard),
        compare_of(plane, "kernel", &post.kernel),
        compare_of(plane, "supply", &post.supply),
        Compare {
            role: "holder-winning-egg".to_string(),
            address: shared.holder_outcome_token.address.clone(),
            expected: token_account_bytes(plane.outcome_mints[index].bytes, shared.holder.bytes, 0),
            pre: token_account_bytes(
                plane.outcome_mints[index].bytes,
                shared.holder.bytes,
                quantity,
            ),
        },
        Compare {
            role: "holder-collateral".to_string(),
            address: shared.holder_collateral_token.address.clone(),
            expected: token_account_bytes(
                shared.collateral_mint.bytes,
                shared.holder.bytes,
                quantity,
            ),
            pre: token_account_bytes(shared.collateral_mint.bytes, shared.holder.bytes, 0),
        },
        Compare {
            role: format!("{}.hoard-token", plane.label),
            address: plane.hoard_token.address.clone(),
            expected: immutable_owner_account_bytes(
                shared.collateral_mint.bytes,
                plane.hoard_authority.bytes,
                WALK_CASH + SECOND_ENDOW_AMOUNT - quantity,
            ),
            pre: immutable_owner_account_bytes(
                shared.collateral_mint.bytes,
                plane.hoard_authority.bytes,
                WALK_CASH + SECOND_ENDOW_AMOUNT,
            ),
        },
        Compare {
            role: format!("{}.outcome-mint-{WALK_OUTCOME_WIN}", plane.label),
            address: plane.outcome_mints[index].address.clone(),
            expected: mint_bytes(Some(plane.market.bytes), 0, 0),
            pre: mint_bytes(Some(plane.market.bytes), 0, quantity),
        },
    ];
    let (payer_position, payer_replay) = owner_plane(shared, plane, shared.payer.bytes);
    let (position_bytes, replay_bytes) = first_endow_owner_bytes(
        plane,
        shared.payer.bytes,
        &payer_position,
        &payer_replay,
        SECOND_ENDOW_AMOUNT,
    );
    compares.extend([
        Compare {
            role: "payer-position".to_string(),
            address: payer_position.address,
            expected: position_bytes.clone(),
            pre: position_bytes,
        },
        Compare {
            role: "payer-replay".to_string(),
            address: payer_replay.address,
            expected: replay_bytes.clone(),
            pre: replay_bytes,
        },
    ]);
    compares
}

/// Build the signed lane over one actual market address.
///
/// Unlike [`build_lifecycle`], this mutates `plane` after every accepted
/// offline transition and emits the next transaction against the same keys.
/// The local runner supplies a fresh blockhash and real signatures, commits
/// each transaction, and reloads these expectations in order.
fn build_committed_cases(f: &Fixture, plane: &mut Plane) -> Vec<Case> {
    let shared = &f.shared;
    let actor = shared.actor.bytes;
    let mut cases = Vec::new();

    /* 1. Create the one market and all seven state PDAs from absent slots. */
    let create = layout_request(0, create_intent(shared, NONCE_COMMITTED));
    let (founded, founded_resolution) = founding_plane(shared, plane, NONCE_COMMITTED);
    let mut compares: Vec<Compare> = plane
        .state_roles()
        .iter()
        .map(|(role, _)| compare_of(plane, role, output_slice(&founded, role)))
        .collect();
    compares.push(compare_of(plane, "resolution", &founded_resolution));
    let mut founded_plane = Plane::clone_state(plane, &founded);
    founded_plane.resolution_bytes = founded_resolution;
    for (role, pda, data) in founded_plane.token_accounts(shared) {
        /* Holder accounts are user-side prerequisites, not outputs of
         * CreateMarket. Later committed steps create them through ordinary
         * signed System and Token-2022 instructions; the Hoard account and
         * outcome mints are the three token accounts created here. */
        if role.contains("holder-token") {
            continue;
        }
        compares.push(Compare {
            role,
            address: pda.address,
            expected: data,
            pre: Vec::new(),
        });
    }
    let mut case = Case::accept(
        "committed-01-create-market",
        "Committed",
        "layout re-encode + reference::validate_market_init",
        "create one market identity and its seven program state PDAs through signed System CPIs from genuinely absent addresses",
        create_transaction(shared, plane, actor, true, create),
        1,
        compares,
    );
    case.compute_limit = Some(COMPUTE_UNIT_CEILING);
    cases.push(case);
    plane.state = founded;
    plane.resolution_bytes = founded_resolution;

    /* 2-4. Create every ordinary holder account by public System and
     * Token-2022 instructions. None is installed by validator genesis. */
    let winning = usize::from(WALK_OUTCOME_WIN);
    assert_eq!(
        plane.holder_tokens[winning].bytes, shared.actor_outcome_token.bytes,
        "the committed actor Egg account must be the fresh signer identity"
    );
    cases.push(Case::accept(
        "committed-02-create-actor-egg-account",
        "Committed",
        "System CreateAccount + Token-2022 InitializeAccount3",
        "create the actor's ordinary winning-Egg account with a fresh test-only account signer",
        create_holder_token_transaction(
            shared,
            &plane.outcome_mints[winning].bytes,
            &actor,
            &shared.actor_outcome_token.bytes,
        ),
        2,
        vec![constructed_token_compare(
            "actor-winning-egg",
            &shared.actor_outcome_token,
            plane.outcome_mints[winning].bytes,
            actor,
        )],
    ));
    cases.push(Case::accept(
        "committed-03-create-holder-egg-account",
        "Committed",
        "System CreateAccount + Token-2022 InitializeAccount3",
        "create an independent bearer's ordinary winning-Egg account",
        create_holder_token_transaction(
            shared,
            &plane.outcome_mints[winning].bytes,
            &shared.holder.bytes,
            &shared.holder_outcome_token.bytes,
        ),
        2,
        vec![constructed_token_compare(
            "holder-winning-egg",
            &shared.holder_outcome_token,
            plane.outcome_mints[winning].bytes,
            shared.holder.bytes,
        )],
    ));
    cases.push(Case::accept(
        "committed-04-create-holder-collateral-account",
        "Committed",
        "System CreateAccount + Token-2022 InitializeAccount3",
        "create that bearer's ordinary collateral destination account",
        create_holder_token_transaction(
            shared,
            &shared.collateral_mint.bytes,
            &shared.holder.bytes,
            &shared.holder_collateral_token.bytes,
        ),
        2,
        vec![constructed_token_compare(
            "holder-collateral",
            &shared.holder_collateral_token,
            shared.collateral_mint.bytes,
            shared.holder.bytes,
        )],
    ));

    /* 5. The sole inbound value boundary: real Token-2022 deposit. */
    let endow = endow_request(plane, 0, WALK_CASH);
    cases.push(Case::accept(
        "committed-05-backed-endow",
        "Committed",
        "layout re-encode + exact Token-2022 deltas",
        "debit the actor, credit pooled Hoard custody, credit position cash, and advance replay in one committed transaction",
        endow_transaction(shared, plane, Signer::own(shared, actor, true), endow),
        1,
        endow_compares(shared, plane, WALK_CASH),
    ));
    let (position, replay) = endow_post(plane, WALK_CASH);
    plane.state.position.copy_from_slice(&position);
    plane.state.replay.copy_from_slice(&replay);

    /* 6-8. Publicly create and fund a second owner's collateral account, then
     * let that owner's first Endow atomically create its Position/Replay pair. */
    cases.push(Case::accept(
        "committed-06-create-payer-collateral-account",
        "Committed",
        "System CreateAccount + Token-2022 InitializeAccount3",
        "create the fee payer's ordinary collateral account for a second-owner first deposit",
        create_holder_token_transaction(
            shared,
            &shared.collateral_mint.bytes,
            &shared.payer.bytes,
            &shared.payer_collateral_token.bytes,
        ),
        2,
        vec![constructed_token_compare(
            "payer-collateral",
            &shared.payer_collateral_token,
            shared.collateral_mint.bytes,
            shared.payer.bytes,
        )],
    ));
    cases.push(Case::accept(
        "committed-07-fund-second-owner",
        "Committed",
        "Token-2022 TransferChecked",
        "transfer collateral from the founding actor to the second owner's ordinary account",
        transfer_second_owner_collateral_transaction(shared, SECOND_ENDOW_AMOUNT),
        1,
        second_owner_funding_compares(shared),
    ));
    let (payer_position, payer_replay) = owner_plane(shared, plane, shared.payer.bytes);
    let payer_endow = endow_request_for(
        plane,
        Hash32::from_bytes(shared.payer.bytes),
        0,
        SECOND_ENDOW_AMOUNT,
    );
    cases.push(Case::accept(
        "committed-08-create-second-owner-with-endow",
        "Committed",
        "codec expected-state + exact Token-2022 deltas",
        "the second owner's first backed Endow creates absent Position and Replay PDAs atomically",
        endow_transaction_at(
            shared,
            plane,
            &payer_position,
            &payer_replay,
            Signer::own(shared, shared.payer.bytes, true)
                .presenting(&shared.payer_collateral_token),
            payer_endow,
        ),
        1,
        second_owner_endow_compares(shared, plane, &payer_position, &payer_replay),
    ));

    /* 9. Lock pooled cash as complete-set collateral; no second debit. */
    let split = walk_layout_request(plane, 1);
    let split_post = plane
        .layout(shared, &split, actor, true)
        .expect("the committed walk splits");
    cases.push(Case::accept(
        "committed-09-split",
        "Committed",
        "reference::apply",
        "lock pooled position cash into complete sets while both custody token balances remain unchanged",
        seam_transaction(
            shared,
            plane,
            Signer::own(shared, actor, true),
            None,
            Leg::Collateral,
            split,
        ),
        1,
        committed_custody_compares(shared, plane, &split_post),
    ));
    plane.advance(split_post);

    /* 10-11. Cross the internal/external boundary and partly return. */
    let materialize = walk_layout_request(plane, 2);
    let materialize_sbf = walk_sbf_layout_request(plane, 2);
    let materialize_post = plane
        .layout(shared, &materialize, actor, true)
        .expect("the committed walk materializes");
    cases.push(Case::accept(
        "committed-10-materialize",
        "Committed",
        "reference::apply",
        "mint a real Token-2022 outcome balance against the same market state",
        seam_transaction(
            shared,
            plane,
            Signer::own(shared, actor, true),
            None,
            Leg::Outcome(WALK_OUTCOME_WIN),
            materialize_sbf,
        ),
        1,
        seam_compares(
            shared,
            plane,
            Leg::Outcome(WALK_OUTCOME_WIN),
            &materialize_post,
        ),
    ));
    plane.advance(materialize_post);

    let dematerialize = walk_layout_request(plane, 3);
    let dematerialize_sbf = walk_sbf_layout_request(plane, 3);
    let dematerialize_post = plane
        .layout(shared, &dematerialize, actor, true)
        .expect("the committed walk dematerializes");
    cases.push(Case::accept(
        "committed-11-dematerialize",
        "Committed",
        "reference::apply",
        "burn part of that outcome balance back into the same internal position",
        seam_transaction(
            shared,
            plane,
            Signer::own(shared, actor, true),
            None,
            Leg::Outcome(WALK_OUTCOME_WIN),
            dematerialize_sbf,
        ),
        1,
        seam_compares(
            shared,
            plane,
            Leg::Outcome(WALK_OUTCOME_WIN),
            &dematerialize_post,
        ),
    ));
    plane.advance(dematerialize_post);

    /* 12. Move the remaining materialized Egg to an independent bearer using
     * only the ordinary Token-2022 program. Clutch state does not move. */
    let bearer_quantity = walk_unredeemed_external();
    cases.push(Case::accept(
        "committed-12-transfer-egg-to-bearer",
        "Committed",
        "Token-2022 TransferChecked",
        "transfer the live winning Egg to a wallet with no Clutch Position or Replay account",
        transfer_outcome_transaction(
            shared,
            &plane.holder_tokens[winning].bytes,
            &plane.outcome_mints[winning].bytes,
            &shared.holder_outcome_token.bytes,
            bearer_quantity,
        ),
        1,
        bearer_transfer_compares(shared, plane, bearer_quantity),
    ));

    /* 13. Unlock part of a complete set; pooled tokens still do not move. */
    let merge = walk_layout_request(plane, 4);
    let merge_post = plane
        .layout(shared, &merge, actor, true)
        .expect("the committed walk merges");
    cases.push(Case::accept(
        "committed-13-merge",
        "Committed",
        "reference::apply",
        "unlock complete-set backing back into position cash without pretending that reclassification is a withdrawal",
        seam_transaction(
            shared,
            plane,
            Signer::own(shared, actor, true),
            None,
            Leg::Collateral,
            merge,
        ),
        1,
        committed_custody_compares(shared, plane, &merge_post),
    ));
    plane.advance(merge_post);

    /* 14. A separately identified feed head commits three contiguous pages.
     * The market's immutable Terms still name the already-matured feed head;
     * this step proves committing feed sequencing, not that the two identities
     * are one.  That remaining construction gap is reported in the evidence. */
    cases.push(Case::accept(
        "committed-14-feed-advance",
        "Committed",
        "accumulator fold + FeedAccount codec",
        "commit three contiguous pages against one writable feed identity in one signed transaction",
        walk_advance_transaction(shared, &f.walk, actor),
        WALK_PAGE_BOUNDS.len(),
        vec![Compare {
            role: "committed.advance-feed".to_string(),
            address: shared.advance_feed_head.address.clone(),
            expected: f.walk.advanced_feed_bytes.to_vec(),
            pre: shared.advance_feed_bytes.to_vec(),
        }],
    ));

    /* 15. Resolve the market on its immutable, matured feed. */
    let window = encode_window(shared.feed, &winning_records());
    let resolve = resolve_request(5, WINNING_PAYOUT_INDEX);
    let resolve_post = plane
        .gate(shared, &resolve, &window, true, actor, true)
        .expect("the committed walk resolves");
    let mut compares = state_compares(plane, &resolve_post);
    compares.push(compare_of(
        plane,
        "resolution",
        &resolve_post
            .resolution
            .expect("a committed resolve writes its record"),
    ));
    let mut case = Case::accept(
        "committed-15-resolve",
        "Committed",
        "reference::apply_with_evidence",
        "resolve the same market identity from immutable Terms and a sealed observation window",
        gate_transaction(
            shared,
            plane,
            &f.resolve_buffer,
            Signer::own(shared, actor, true),
            true,
            false,
            resolve,
        ),
        1,
        compares,
    );
    case.compute_limit = Some(COMPUTE_UNIT_CEILING);
    cases.push(case);
    plane.advance(resolve_post);

    /* 16. Commit the failed transaction too, and require no watched byte move. */
    let late_merge = layout_request(
        6,
        Intent::Merge {
            market: plane.market_id,
            owner: plane.owner,
            quantity: walk_redeem_winning(),
        },
    );
    cases.push(Case::refuse(
        "committed-16-late-merge-refused",
        "Committed",
        "merge after resolution must fail without consuming replay or changing state",
        seam_transaction(
            shared,
            plane,
            Signer::own(shared, actor, true),
            None,
            Leg::Collateral,
            late_merge,
        ),
        code::NOT_ACTIVE,
        "reference::apply: MismatchedState".to_string(),
    ));

    /* 17-18. Internal redemptions are reclassifications into stranded cash,
     * not physical payouts; there is no Withdraw instruction yet. */
    let redeem_win = redeem_request(6, WALK_OUTCOME_WIN, walk_redeem_winning());
    let redeem_win_post = plane
        .gate(shared, &redeem_win, &[], false, actor, true)
        .expect("the committed walk redeems winning internal claims");
    let mut compares = committed_custody_compares(shared, plane, &redeem_win_post);
    compares.push(compare_of(
        plane,
        "resolution",
        &redeem_win_post
            .resolution
            .expect("redemption returns the record unchanged"),
    ));
    let mut case = Case::accept(
        "committed-17-redeem-winning-internal",
        "Committed",
        "reference::apply_with_evidence",
        "burn winning internal claims into position cash; pooled custody remains inside the Hoard",
        gate_transaction(
            shared,
            plane,
            &f.redeem_buffer,
            Signer::own(shared, actor, true),
            false,
            true,
            redeem_win,
        ),
        1,
        compares,
    );
    case.compute_limit = Some(COMPUTE_UNIT_CEILING);
    cases.push(case);
    plane.advance(redeem_win_post);

    let redeem_lose = redeem_request(7, WALK_OUTCOME_LOSE, walk_redeem_losing());
    let redeem_lose_post = plane
        .gate(shared, &redeem_lose, &[], false, actor, true)
        .expect("the committed walk redeems losing internal claims");
    let mut compares = committed_custody_compares(shared, plane, &redeem_lose_post);
    compares.push(compare_of(
        plane,
        "resolution",
        &redeem_lose_post
            .resolution
            .expect("redemption returns the record unchanged"),
    ));
    let mut case = Case::accept(
        "committed-18-redeem-losing-internal",
        "Committed",
        "reference::apply_with_evidence",
        "burn losing internal claims for zero while leaving the same pooled custody untouched",
        gate_transaction(
            shared,
            plane,
            &f.redeem_buffer,
            Signer::own(shared, actor, true),
            false,
            true,
            redeem_lose,
        ),
        1,
        compares,
    );
    case.compute_limit = Some(COMPUTE_UNIT_CEILING);
    cases.push(case);
    plane.advance(redeem_lose_post);

    /* 19. First prove late-fault atomicity: one bearer exit completes, an
     * identical sibling then finds the source empty, and the whole transaction
     * must roll every state write and both Token-2022 CPIs back. */
    let external_request = redeem_external_request(shared, plane, bearer_quantity);
    let mut case = Case::refuse(
        "committed-19-external-exit-rollback",
        "Committed",
        "a successful bearer burn+payout followed by a duplicate exit must fail atomically",
        redeem_external_transaction_repeated(
            shared,
            plane,
            WALK_OUTCOME_WIN,
            external_request.clone(),
            2,
        ),
        code::TOKEN_DELTA_MISMATCH,
        "n/a (runtime atomicity over two production instructions)".to_string(),
    );
    case.instruction_count = 2;
    case.compute_limit = Some(COMPUTE_UNIT_CEILING);
    cases.push(case);

    /* 20. The independent bearer burns its real Token-2022 claim and receives
     * the exact unit-vector payout directly from pooled custody. */
    let external_post = bearer_redeem_post(plane, bearer_quantity);
    let mut case = Case::accept(
        "committed-20-redeem-external-bearer",
        "Committed",
        "codec expected-state + exact Token-2022 deltas",
        "burn the independent bearer's winning Egg and pay collateral without any claimant Position, External, or Replay account",
        redeem_external_transaction(
            shared,
            plane,
            WALK_OUTCOME_WIN,
            external_request,
        ),
        1,
        bearer_redeem_compares(shared, plane, &external_post, bearer_quantity),
    );
    case.compute_limit = Some(COMPUTE_UNIT_CEILING);
    cases.push(case);
    plane.advance(external_post);

    let hoard = HoardAccount::decode(&plane.state.hoard).expect("terminal Hoard decodes");
    let position =
        PositionAccount::decode(&plane.state.position).expect("terminal position decodes");
    assert_eq!(
        hoard.collateral_atoms + position.cash_atoms + SECOND_ENDOW_AMOUNT + bearer_quantity,
        WALK_CASH + SECOND_ENDOW_AMOUNT,
        "both owners' stranded internal cash plus the paid bearer exit must equal deposits"
    );
    assert_eq!(
        hoard.collateral_atoms, 0,
        "all terminal backing is discharged"
    );
    assert_eq!(
        position.internal, [0; MAX_OUTCOMES],
        "the committed terminal internal claims drain"
    );
    cases
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

/// Emit the minimal genesis and ordered cases for the real-signature lane.
fn emit_committed_plan(out_dir: &Path, f: &Fixture) {
    let shared = &f.shared;
    let mut plan = Plan {
        program: shared.program.address.clone(),
        token_program: base58_of(&shared.token_program),
        ..Plan::default()
    };

    /* Frozen Realm/evidence prerequisites.  They are honest genesis
     * assistance too: the current signed walk begins at market construction,
     * not at Realm construction. */
    plan.account("realm", &shared.realm, &shared.realm_bytes);
    plan.account("profile", &shared.profile, &shared.profile_bytes);
    plan.account("terms", &shared.terms, &shared.terms_bytes);
    plan.account("feed", &shared.feed_head, &shared.feed_bytes);
    plan.account(
        "advance-feed",
        &shared.advance_feed_head,
        &shared.advance_feed_bytes,
    );
    plan.account(
        "collateral-policy",
        &shared.policy_account,
        &shared.policy_bytes,
    );
    plan.token_account(
        "collateral-mint",
        &shared.collateral_mint,
        &shared.collateral_mint_bytes,
    );
    plan.token_account(
        "actor-collateral",
        &shared.actor_token,
        &shared.actor_token_bytes,
    );
    plan.owned("actor-lamports", &shared.actor, SYSTEM_PROGRAM, &[]);

    let mut market = Plane::build(shared, "committed-market", NONCE_COMMITTED, WALK_GENERATION);
    /* All seven program state targets, the Hoard token account, the outcome
     * mints, and every user-side token account are deliberately absent. The
     * committed cases create them through ordinary signed instructions. */

    plan.account("resolve-buffer", &f.resolve_buffer, &f.resolve_buffer_bytes);
    plan.account("redeem-buffer", &f.redeem_buffer, &f.redeem_buffer_bytes);
    for (index, page) in f.walk.pages.iter().enumerate() {
        plan.account(
            &format!("committed-page-{index}"),
            page,
            &f.walk.page_bytes[index],
        );
    }

    plan.cases = build_committed_cases(f, &mut market);

    let mut genesis_lines = String::new();
    for account in &plan.genesis {
        let file = format!("accounts/{}.json", account.role);
        write(
            &out_dir.join(&file),
            &account_json(&account.address, &account.owner, &account.data),
        );
        genesis_lines.push_str(&format!("{} {} {}\n", account.role, account.address, file));
    }
    write(&out_dir.join("genesis.txt"), &genesis_lines);
    let cases = emit_cases(out_dir, &plan.cases);
    let precreated: Vec<String> = plan
        .genesis
        .iter()
        .filter(|account| account.owner == plan.program)
        .map(|account| {
            format!(
                "    {}",
                json_string(&format!("{} {}", account.role, account.address))
            )
        })
        .collect();
    let committed = format!(
        "{{\n  \"program_id\": {},\n  \"payer\": {},\n  \"actor\": {},\n  \"holder\": {},\n  \"genesis_assisted\": true,\n  \"precreated_program_accounts\": [\n{}\n  ],\n  \"steps\": [\n{}\n  ]\n}}\n",
        json_string(&shared.program.address),
        json_string(&shared.payer.address),
        json_string(&shared.actor.address),
        json_string(&shared.holder.address),
        precreated.join(",\n"),
        cases.join(",\n")
    );
    write(&out_dir.join("committed.json"), &committed);

    println!("committed plan written to {}", out_dir.display());
    println!("program_id   {}", shared.program.address);
    println!("payer        {}", shared.payer.address);
    println!("actor        {}", shared.actor.address);
    println!(
        "scope        GENESIS-ASSISTED: {} program-owned prerequisites",
        precreated.len()
    );
    for case in &plan.cases {
        match case.expect_code {
            None => println!(
                "  accept {:<40} {} account reload(s)",
                case.name,
                case.compare.as_ref().map_or(0, Vec::len)
            ),
            Some(code) => println!("  refuse {:<40} Custom(0x{code:04x})", case.name),
        }
    }
    println!(
        "terminal     bearer exit drained; two owners retain free cash because WithdrawCash is absent"
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out_dir = PathBuf::from(
        args.next()
            .expect("usage: clutch-sbf-harness <out-dir> [--committed]"),
    );
    let mode = args.next();
    assert!(args.next().is_none(), "too many harness arguments");
    assert!(
        mode.is_none() || mode.as_deref() == Some("--committed"),
        "unknown harness mode"
    );
    for sub in ["accounts", "expected", "tx"] {
        fs::create_dir_all(out_dir.join(sub)).expect("create plan directory");
    }

    let f = build_fixture();
    if mode.as_deref() == Some("--committed") {
        emit_committed_plan(&out_dir, &f);
        return;
    }
    let shared = &f.shared;
    let mut plan = Plan {
        program: shared.program.address.clone(),
        token_program: base58_of(&shared.token_program),
        ..Plan::default()
    };

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

    /* The Realm's collateral plane.  The 266 policy bytes sit in an ordinary
     * program-owned account because their *address* is arbitrary by design:
     * `collateral::verify_profile_identity` recomputes the child digest and
     * the parent Profile identity from the bytes, so the account they arrive
     * from is not what makes them this Realm's policy. */
    plan.account(
        "collateral-policy",
        &shared.policy_account,
        &shared.policy_bytes,
    );
    plan.token_account(
        "collateral-mint",
        &shared.collateral_mint,
        &shared.collateral_mint_bytes,
    );
    plan.token_account(
        "actor-collateral",
        &shared.actor_token,
        &shared.actor_token_bytes,
    );
    plan.token_account(
        "stranger-collateral",
        &shared.stranger_token,
        &shared.stranger_token_bytes,
    );

    /* The creator's lamports.  `CreateMarket` now founds real accounts through
     * a System-program CPI and the creator is the rent payer, so the creator
     * must be a System-owned account that *has* lamports -- a signer with no
     * account cannot fund a creation. */
    plan.owned("actor-lamports", &shared.actor, SYSTEM_PROGRAM, &[]);

    /* Every market plane, and the Token-2022 accounts of the founded ones. */
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
        for (role, pda, data) in plane.token_accounts(shared) {
            plan.token_account(&role, &pda, &data);
        }
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
        for (role, pda, data) in plane.token_accounts(shared) {
            plan.token_account(&role, &pda, &data);
        }
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
            &account_json(&account.address, &account.owner, &account.data),
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
            "Endow",
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
    fn the_committed_plan_uses_one_market_and_only_declared_signer_slots() {
        let f = build_fixture();
        let mut market = Plane::build(
            &f.shared,
            "committed-market",
            NONCE_COMMITTED,
            WALK_GENERATION,
        );
        let addresses: Vec<String> = market
            .state_roles()
            .iter()
            .map(|(_, pda)| pda.address.clone())
            .chain(std::iter::once(market.resolution.address.clone()))
            .collect();
        let cases = build_committed_cases(&f, &mut market);
        assert_eq!(cases.len(), 20, "the committed lane has twenty steps");
        for case in &cases {
            assert!(
                case.tx.first().is_some_and(|count| (1..=2).contains(count)),
                "{} must reserve only its one or two declared signer slots",
                case.name,
            );
            assert!(case.tx.len() <= 1232, "{} exceeds one packet", case.name);
            for compare in case.compare.iter().flatten().filter(|entry| {
                matches!(
                    entry.role.rsplit('.').next(),
                    Some(
                        "market"
                            | "hoard"
                            | "position"
                            | "kernel"
                            | "replay"
                            | "supply"
                            | "resolution"
                    )
                )
            }) {
                assert!(
                    addresses.contains(&compare.address),
                    "{} compares another market plane at {}",
                    case.name,
                    compare.address
                );
            }
        }
        let replay = ReplayAccount::decode(&market.state.replay).expect("terminal replay decodes");
        assert_eq!(
            replay.sequence, 8,
            "one identity consumed eight accepted intents"
        );
        let (payer_position, payer_replay) = owner_plane(&f.shared, &market, f.shared.payer.bytes);
        let terminal = cases.last().expect("terminal bearer redemption");
        let compares = terminal.compare.as_ref().expect("terminal accepts");
        for address in [payer_position.address, payer_replay.address] {
            assert!(
                compares.iter().any(|compare| compare.address == address),
                "the terminal reload must retain the second owner's state"
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
        assert_eq!(walk.steps.len(), 11, "the walk is eleven steps");
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
        for item in ["1 (in part)", "2 (in part)", "3", "8", "11"] {
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
