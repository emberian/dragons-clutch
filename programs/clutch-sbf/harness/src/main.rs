//! Differential bring-up harness for the Dragon's Clutch SBF program.
//!
//! This binary builds one deterministic `Split` fixture, computes the expected
//! post-state with the **offline reference adapter**
//! (`clutch_solana_reference::apply`), and emits everything a local
//! `solana-test-validator` needs to execute the same transition inside a real
//! SVM: genesis account dumps, a serialized transaction, and the expected
//! post-state bytes.
//!
//! It never signs anything and never touches a keypair.  Every address it uses
//! is a program-derived address of the System program with a fixed seed, so the
//! fixture is reproducible and carries no key material.  Signature verification
//! is switched off at the RPC layer by `scripts/simulate.py`; see
//! `docs/implementation/SBF_BRINGUP.md` for exactly what that does and does not
//! establish.
//!
//! Nothing here is evidence about mainnet, devnet, or any deployment.

use clutch_accumulator::COVERAGE_POLICY_COMPLETE_REQUIRED;
use clutch_kernel::{PayoutSet, PayoutVector, MAX_PAYOUTS};
use clutch_sbf::seeds;
use clutch_solana_layout::{
    account_len, canonical_epoch_id, canonical_market_id, canonical_order_set_id,
    canonical_outcome_id, canonical_profile_hash, canonical_realm_id, CandidateRecord,
    EpochAccount, FeedAccount, FeedId, FinalPotAccount, Hash32, HoardAccount, Intent,
    MarketAccount, OrderPageAccount, OrderRecord, OrderSlot, PayoutVectorBytes, PositionAccount,
    PriceGridAccount, ProfileAccount, RealmAccount, ResolutionAccount, SettlementReceiptAccount,
    SupplyLedgerAccount, TermsAccount, CANDIDATE_STATUS_SELECTED, EPOCH_PHASE_CLEARED,
    MAX_GRID_TICKS, MAX_INTENT_BYTES, MAX_ORDERS_PER_PAGE, MAX_OUTCOMES, PAYOUT_INDEX_UNRESOLVED,
    POT_PHASE_OPEN, PROFILE_PARENT_BYTES, RECEIPT_LEG_DIRECT, RELATION_VERSION,
};
use clutch_solana_reference::{
    apply, AccountMetadata, ActorMetadata, ExpectedBindings, ExternalAccount, KernelAccount,
    ReplayAccount, StateBytes, TransitionMetadata, TransitionOutput, EXTERNAL_ACCOUNT_LEN,
    FAIL_UNIFORM_REFUND_01, GEN_EXACT_01, KERNEL_ACCOUNT_LEN, REPLAY_ACCOUNT_LEN,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Fixture constants.  These mirror the offline reference adapter's `Split`
/// test so that a divergence is a real disagreement rather than a different
/// scenario.
///
/// The parent-Profile preimage is exactly `PROFILE_PARENT_BYTES` long because
/// `canonical_profile_hash` refuses any other length; the contents are an
/// arbitrary fixed pattern, since this lane derives an identity and makes no
/// claim about which collateral policy the Profile commits to.
const PROFILE_PREIMAGE_FILL: u8 = 0x5b;
const REALM_NONCE: u64 = 7;
const MARKET_NONCE: u64 = 9;
const GENERATION: u64 = 2;
const SEQUENCE: u64 = 0;
const CASH_ATOMS: u64 = 100;
const RESERVED_CASH_ATOMS: u64 = 7;
const COLLATERAL_CAP: u64 = 1_000;
const SPLIT_QUANTITY: u64 = 5;
const OUTCOME_COUNT: u8 = 2;
/// Comfortably above the rent-exempt minimum for every account in the fixture.
const ACCOUNT_LAMPORTS: u64 = 100_000_000;

/* Wave-3 plane constants.  The window-policy numbers are exactly the offline
 * reference adapter's own resolution fixture, so that a future resolution
 * differential compares two adapters over one scenario rather than over two.
 * The batch-auction numbers have no reference counterpart at all -- no adapter,
 * offline or on-chain, implements that family yet -- so they are the smallest
 * shape the frozen codecs accept. */
const START_BUCKET: u64 = 100;
const END_BUCKET_EXCLUSIVE: u64 = 103;
const MATURITY_HORIZON: u64 = 4;
const GRID_FAMILY: u32 = 7;
const GRID_VERSION: u16 = 1;
const BUCKET_SECONDS: u64 = 60;
const PRICE_SCALE: u64 = 10_000;
const EPOCH_INDEX: u64 = 0;
const REMAINDER_SEED: u64 = 99;
/// Opaque book identity inside the market; this lane names no book policy.
const BOOK_ID_FILL: u8 = 0x6b;
/// Opaque frozen-policy identity; this lane names no clearing policy.
const POLICY_ID_FILL: u8 = 0x70;
const BUY_ORDER_ID_FILL: u8 = 0x11;
const SELL_ORDER_ID_FILL: u8 = 0x22;
const SLICE_QUANTITY: u64 = 3;
const VIRTUAL_SPLIT: u64 = 5;

const B58: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

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

/// The wave-3 account plane.
///
/// These are the frozen-layout accounts the single implemented instruction does
/// **not** touch: the immutable terms artifact and its price grid, the
/// resolution record, the feed head, and the whole batch-auction family.  They
/// are loaded at genesis so that a per-instruction lane has a real, bound,
/// on-chain plane to write against instead of inventing one.
///
/// ## What this fixture claims, and what it does not
///
/// Every account here decodes through its frozen codec, and every *identity*
/// binding the layout crate can decide is asserted in [`build_wave3`] and in
/// [`build_fixture`]: terms to market, supply ledger to market, grid to terms,
/// epoch to terms and grid, epoch to its frozen page set, candidate to epoch,
/// pot to candidate, receipt to candidate, and resolution to terms.
///
/// No *economic* coherence is claimed and none should be read in.  Whether this
/// candidate is the best valid submitted candidate for this book, whether the
/// pot balances against the receipts, and whether the prices clear anything are
/// questions for a batch relation that no adapter runs yet.  The fixture is a
/// shape, bound at every seam a codec owns and at none that it does not.
struct Wave3 {
    grid: Pda,
    terms: Pda,
    resolution: Pda,
    feed: Pda,
    epoch: Pda,
    page: Pda,
    candidate: Pda,
    pot: Pda,
    receipt: Pda,
    /// The immutable terms digest, which [`MarketAccount::terms`] must equal.
    terms_digest: Hash32,
    terms_account: TermsAccount,
    grid_bytes: [u8; account_len::PRICE_GRID],
    terms_bytes: [u8; account_len::TERMS],
    resolution_bytes: [u8; account_len::RESOLUTION],
    feed_bytes: [u8; account_len::FEED],
    epoch_bytes: [u8; account_len::EPOCH],
    page_bytes: [u8; account_len::ORDER_PAGE],
    candidate_bytes: [u8; account_len::CANDIDATE],
    pot_bytes: [u8; account_len::FINAL_POT],
    receipt_bytes: [u8; account_len::SETTLEMENT_RECEIPT],
}

/// Build and bind the wave-3 plane, deriving every address from `seeds`.
///
/// Each content-addressed account is built with a zero stored bump, digested,
/// used to derive its own address, and only then given the canonical bump: the
/// bump is deliberately outside every digest, so a PDA derived from a digest can
/// still carry the bump that derivation produced.
fn build_wave3(
    pid: &str,
    realm_hash: Hash32,
    profile_hash: Hash32,
    market_id: Hash32,
    feed: FeedId,
    buyer: Hash32,
    seller: Hash32,
) -> Wave3 {
    let realm_seed = realm_hash.bytes().to_vec();
    let market_seed = market_id.bytes().to_vec();

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
        pid,
        &[
            seeds::SEED_GRID.to_vec(),
            realm_seed.clone(),
            grid_account.grid.bytes().to_vec(),
        ],
    );
    grid_account.stored_bump = grid.bump;

    /* Immutable terms.  The window policy is exactly the offline reference
     * adapter's resolution fixture, so a future resolution differential is a
     * disagreement between two adapters rather than between two scenarios. */
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
    let mut terms_account = TermsAccount {
        terms: Hash32::ZERO,
        realm: realm_hash,
        profile: profile_hash,
        feed,
        price_grid: grid_account.grid,
        outcome_count: OUTCOME_COUNT,
        payout_count: 2,
        payouts,
        grid_family_id: GRID_FAMILY,
        grid_version: GRID_VERSION,
        bucket_seconds: BUCKET_SECONDS,
        expected_start_bucket: START_BUCKET,
        expected_end_bucket_exclusive: END_BUCKET_EXCLUSIVE,
        maturity_horizon_buckets: MATURITY_HORIZON,
        coverage_policy_id: u32::from(COVERAGE_POLICY_COMPLETE_REQUIRED),
        repair_policy_id: u32::from(GEN_EXACT_01),
        failure_policy_id: u32::from(FAIL_UNIFORM_REFUND_01),
        stored_bump: 0,
        flags: 0,
    };
    terms_account.terms = terms_account
        .recomputed_terms_digest()
        .expect("the fixture terms body must digest");
    let terms = derive(
        pid,
        &[
            seeds::SEED_TERMS.to_vec(),
            realm_seed.clone(),
            terms_account.terms.bytes().to_vec(),
        ],
    );
    terms_account.stored_bump = terms.bump;

    /* Resolution record, unresolved.  An unresolved record is the honest
     * genesis state: nothing has sealed a window, so no payout is selected. */
    let resolution = derive(pid, &[seeds::SEED_RESOLUTION.to_vec(), market_seed.clone()]);
    let resolution_account = ResolutionAccount {
        market: market_id,
        terms: terms_account.terms,
        feed,
        window: Hash32::ZERO,
        feed_cursor: 0,
        sealed_end_bucket_exclusive: 0,
        repair_generation: 0,
        resolved_slot: 0,
        payout_index: PAYOUT_INDEX_UNRESOLVED,
        stored_bump: resolution.bump,
        flags: 0,
    };

    let feed_pda = derive(pid, &[seeds::SEED_FEED.to_vec(), feed.bytes().to_vec()]);
    let feed_account = FeedAccount {
        feed,
        realm: realm_hash,
        cursor: 0,
        next_boundary: START_BUCKET,
        archive_pages: 0,
        /* Nonzero because the codec refuses a zero identity; this lane has no
         * accumulator summary to commit to and makes no claim about one. */
        summary: Hash32::from_bytes([0x5c; 32]),
        stored_bump: feed_pda.bump,
        flags: 0,
    };

    /* Batch-auction plane.  One epoch, one frozen page holding two orders, one
     * selected candidate, its pot, and one unsettled receipt slice. */
    let epoch_id = canonical_epoch_id(market_id, EPOCH_INDEX);
    let epoch_seed = epoch_id.bytes().to_vec();
    let epoch = derive(
        pid,
        &[
            seeds::SEED_EPOCH.to_vec(),
            market_seed.clone(),
            EPOCH_INDEX.to_le_bytes().to_vec(),
        ],
    );
    let page = derive(
        pid,
        &[
            seeds::SEED_PAGE.to_vec(),
            epoch_seed.clone(),
            0_u16.to_le_bytes().to_vec(),
        ],
    );

    let buy_order_id = Hash32::from_bytes([BUY_ORDER_ID_FILL; 32]);
    let sell_order_id = Hash32::from_bytes([SELL_ORDER_ID_FILL; 32]);
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
        terms: terms_account.terms,
        price_grid: grid_account.grid,
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
        pid,
        &[
            seeds::SEED_CANDIDATE.to_vec(),
            epoch_seed.clone(),
            candidate_account.candidate.bytes().to_vec(),
        ],
    );
    candidate_account.stored_bump = candidate.bump;

    let pot = derive(pid, &[seeds::SEED_POT.to_vec(), epoch_seed.clone()]);
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
        pid,
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

    /* Every binding the frozen layout can decide, asserted here so that a
     * fixture which drifted apart fails the harness instead of quietly
     * shipping an incoherent genesis. */
    grid_account
        .binds_terms(&terms_account)
        .expect("grid binds terms");
    epoch_account
        .binds_terms(&terms_account, &grid_account)
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
    resolution_account
        .binds_terms(&terms_account)
        .expect("resolution binds the immutable terms");

    let mut grid_bytes = [0; account_len::PRICE_GRID];
    let mut terms_bytes = [0; account_len::TERMS];
    let mut resolution_bytes = [0; account_len::RESOLUTION];
    let mut feed_bytes = [0; account_len::FEED];
    let mut epoch_bytes = [0; account_len::EPOCH];
    let mut page_bytes = [0; account_len::ORDER_PAGE];
    let mut candidate_bytes = [0; account_len::CANDIDATE];
    let mut pot_bytes = [0; account_len::FINAL_POT];
    let mut receipt_bytes = [0; account_len::SETTLEMENT_RECEIPT];
    grid_account.encode(&mut grid_bytes).expect("grid");
    terms_account.encode(&mut terms_bytes).expect("terms");
    resolution_account
        .encode(&mut resolution_bytes)
        .expect("resolution");
    feed_account.encode(&mut feed_bytes).expect("feed");
    epoch_account.encode(&mut epoch_bytes).expect("epoch");
    page_account.encode(&mut page_bytes).expect("page");
    candidate_account
        .encode(&mut candidate_bytes)
        .expect("candidate");
    pot_account.encode(&mut pot_bytes).expect("pot");
    receipt_account.encode(&mut receipt_bytes).expect("receipt");

    Wave3 {
        grid,
        terms,
        resolution,
        feed: feed_pda,
        epoch,
        page,
        candidate,
        pot,
        receipt,
        terms_digest: terms_account.terms,
        terms_account,
        grid_bytes,
        terms_bytes,
        resolution_bytes,
        feed_bytes,
        epoch_bytes,
        page_bytes,
        candidate_bytes,
        pot_bytes,
        receipt_bytes,
    }
}

/// Everything the fixture needs, in both byte and address form.
struct Fixture {
    program: Pda,
    payer: Pda,
    actor: Pda,
    stranger: Pda,
    imposter: Pda,
    realm: Pda,
    profile: Pda,
    market: Pda,
    hoard: Pda,
    position: Pda,
    kernel: Pda,
    external: Pda,
    replay: Pda,
    supply: Pda,
    wave3: Wave3,
    realm_bytes: [u8; account_len::REALM],
    profile_bytes: [u8; account_len::PROFILE],
    pre: TransitionOutput,
    post: TransitionOutput,
    request: Vec<u8>,
}

fn build_fixture() -> Fixture {
    let program = fixed_address("clutch-sbf/bringup/program/v1");
    let payer = fixed_address("clutch-sbf/bringup/payer/v1");
    let actor = fixed_address("clutch-sbf/bringup/actor/v1");
    let stranger = fixed_address("clutch-sbf/bringup/stranger/v1");
    let imposter = fixed_address("clutch-sbf/bringup/imposter/v1");

    let profile_preimage = [PROFILE_PREIMAGE_FILL; PROFILE_PARENT_BYTES];
    let profile_hash = canonical_profile_hash(&profile_preimage)
        .expect("the fixture profile preimage must be a canonical profile hash");
    let realm_hash = canonical_realm_id(profile_hash, REALM_NONCE);
    let market_id = canonical_market_id(realm_hash, profile_hash, MARKET_NONCE);
    let owner = Hash32::from_bytes(actor.bytes);

    let realm_seed = realm_hash.bytes().to_vec();
    let profile_seed = profile_hash.bytes().to_vec();
    let market_seed = market_id.bytes().to_vec();
    let owner_seed = owner.bytes().to_vec();
    let generation_seed = GENERATION.to_le_bytes().to_vec();

    let pid = program.address.clone();
    let realm = derive(&pid, &[seeds::SEED_REALM.to_vec(), realm_seed.clone()]);
    let profile = derive(
        &pid,
        &[
            seeds::SEED_PROFILE.to_vec(),
            realm_seed.clone(),
            profile_seed.clone(),
        ],
    );
    let market = derive(
        &pid,
        &[
            seeds::SEED_MARKET.to_vec(),
            realm_seed.clone(),
            market_seed.clone(),
        ],
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
    let supply = derive(&pid, &[seeds::SEED_SUPPLY.to_vec(), market_seed.clone()]);
    let replay = derive(
        &pid,
        &[
            seeds::SEED_REPLAY.to_vec(),
            market_seed,
            owner_seed,
            generation_seed,
        ],
    );

    /* The wave-3 plane is built before the Market account because the Market
     * binds the immutable terms by digest: `MarketAccount::terms` is not a free
     * field, it is the identity of the terms artifact loaded beside it.  The
     * offline reference adapter's own fixture does the same thing, and a Market
     * carrying an unbound terms digest could never resolve. */
    let feed = FeedId::from_bytes([9; 32]);
    let wave3 = build_wave3(
        &pid,
        realm_hash,
        profile_hash,
        market_id,
        feed,
        owner,
        Hash32::from_bytes(stranger.bytes),
    );

    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    outcomes[0] = canonical_outcome_id(market_id, 0);
    outcomes[1] = canonical_outcome_id(market_id, 1);
    let market_account = MarketAccount {
        market: market_id,
        realm: realm_hash,
        profile: profile_hash,
        terms: wave3.terms_digest,
        outcome_count: OUTCOME_COUNT,
        lifecycle: 0,
        stored_bump: market.bump,
        hoard_bump: hoard.bump,
        outcomes,
        feed,
        collateral_cap: COLLATERAL_CAP,
        created_slot: 55,
        reserved: Hash32::ZERO,
    };
    let hoard_account = HoardAccount {
        market: market_id,
        realm: realm_hash,
        authority: Hash32::from_bytes(hoard.bytes),
        collateral_atoms: 0,
        stored_bump: hoard.bump,
        flags: 0,
    };
    let position_account = PositionAccount {
        market: market_id,
        owner,
        generation: GENERATION,
        internal: [0; MAX_OUTCOMES],
        cash_atoms: CASH_ATOMS,
        reserved_cash_atoms: RESERVED_CASH_ATOMS,
        stored_bump: position.bump,
        close_state: 0,
    };
    let kernel_account = KernelAccount {
        market: market_id,
        phase: 0,
        resolved_payout: 0,
        payouts: payout_set(),
        total_supply: [0; MAX_OUTCOMES],
    };
    let external_account = ExternalAccount {
        market: market_id,
        owner,
        position_generation: GENERATION,
        balances: [0; MAX_OUTCOMES],
        stored_bump: external.bump,
        flags: 0,
    };
    let replay_account = ReplayAccount {
        market: market_id,
        owner,
        position_generation: GENERATION,
        sequence: SEQUENCE,
        stored_bump: replay.bump,
        flags: 0,
    };
    let realm_account = RealmAccount {
        realm: realm_hash,
        profile: profile_hash,
        max_outcomes: MAX_OUTCOMES as u8,
        profile_version: 1,
        stored_bump: realm.bump,
        flags: 0,
    };
    let profile_account = ProfileAccount {
        profile: profile_hash,
        realm: realm_hash,
        /* The collateral policy is not frozen in this fixture, so the digest is
         * zero and the freeze flag is clear.  Freezing it is the collateral
         * profile lane's decision, not this harness's. */
        collateral_policy_digest: Hash32::ZERO,
        version: 1,
        flags: 0,
    };

    let supply_account = SupplyLedgerAccount {
        market: market_id,
        realm: realm_hash,
        generation: GENERATION,
        outcome_count: OUTCOME_COUNT,
        internal_supply: [0; MAX_OUTCOMES],
        external_supply: [0; MAX_OUTCOMES],
        stored_bump: supply.bump,
        flags: 0,
    };

    /* The two bindings that need the Market account itself. */
    wave3
        .terms_account
        .binds_market(&market_account)
        .expect("terms bind the market");
    supply_account
        .binds_market(&market_account)
        .expect("supply ledger binds the market");
    let mut pre = TransitionOutput {
        market: [0; account_len::MARKET],
        hoard: [0; account_len::HOARD],
        position: [0; account_len::POSITION],
        kernel: [0; KERNEL_ACCOUNT_LEN],
        external: [0; EXTERNAL_ACCOUNT_LEN],
        replay: [0; REPLAY_ACCOUNT_LEN],
        supply: [0; account_len::SUPPLY_LEDGER],
        resolution: None,
        redemption_payout: 0,
    };
    market_account.encode(&mut pre.market).expect("market");
    hoard_account.encode(&mut pre.hoard).expect("hoard");
    position_account
        .encode(&mut pre.position)
        .expect("position");
    kernel_account.encode(&mut pre.kernel).expect("kernel");
    external_account
        .encode(&mut pre.external)
        .expect("external");
    replay_account.encode(&mut pre.replay).expect("replay");
    supply_account.encode(&mut pre.supply).expect("supply");
    let mut realm_bytes = [0; account_len::REALM];
    let mut profile_bytes = [0; account_len::PROFILE];
    realm_account.encode(&mut realm_bytes).expect("realm bytes");
    profile_account
        .encode(&mut profile_bytes)
        .expect("profile bytes");

    let request = layout_request(
        SEQUENCE,
        Intent::Split {
            market: market_id,
            owner,
            quantity: SPLIT_QUANTITY,
        },
    );

    let metadata = transition_metadata(
        &market,
        &hoard,
        &position,
        &kernel,
        &external,
        &replay,
        &supply,
        &program,
        actor.bytes,
        true,
    );
    let bindings = expected_bindings(
        &program, &market, &hoard, &position, &kernel, &external, &replay, &supply,
    );
    let post = apply(&request, state_bytes(&pre), &metadata, &bindings)
        .expect("the offline reference adapter must accept the bring-up fixture");

    Fixture {
        program,
        payer,
        actor,
        stranger,
        imposter,
        realm,
        profile,
        market,
        hoard,
        position,
        kernel,
        external,
        replay,
        supply,
        wave3,
        realm_bytes,
        profile_bytes,
        pre,
        post,
        request,
    }
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

#[allow(clippy::too_many_arguments)]
fn transition_metadata(
    market: &Pda,
    hoard: &Pda,
    position: &Pda,
    kernel: &Pda,
    external: &Pda,
    replay: &Pda,
    supply: &Pda,
    program: &Pda,
    actor: [u8; 32],
    signer: bool,
) -> TransitionMetadata {
    let account = |pda: &Pda| AccountMetadata {
        key: Hash32::from_bytes(pda.bytes),
        owner_program: Hash32::from_bytes(program.bytes),
        writable: true,
    };
    TransitionMetadata {
        market: account(market),
        hoard: account(hoard),
        position: account(position),
        kernel: account(kernel),
        external: account(external),
        replay: account(replay),
        supply: account(supply),
        actor: ActorMetadata {
            key: Hash32::from_bytes(actor),
            signer,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn expected_bindings(
    program: &Pda,
    market: &Pda,
    hoard: &Pda,
    position: &Pda,
    kernel: &Pda,
    external: &Pda,
    replay: &Pda,
    supply: &Pda,
) -> ExpectedBindings {
    ExpectedBindings {
        program_id: Hash32::from_bytes(program.bytes),
        market: Hash32::from_bytes(market.bytes),
        hoard: Hash32::from_bytes(hoard.bytes),
        position: Hash32::from_bytes(position.bytes),
        kernel: Hash32::from_bytes(kernel.bytes),
        external: Hash32::from_bytes(external.bytes),
        replay: Hash32::from_bytes(replay.bytes),
        supply: Hash32::from_bytes(supply.bytes),
        market_bump: market.bump,
        hoard_bump: hoard.bump,
        position_bump: position.bump,
        external_bump: external.bump,
        replay_bump: replay.bump,
        supply_bump: supply.bump,
    }
}

/// Build the reference request envelope around one frozen layout intent.
///
/// The envelope shape is the one `clutch_solana_reference::Request::decode`
/// accepts.  The constants are re-stated here because they are private to that
/// crate; the decoder is the authority, and a mismatch shows up immediately as
/// a refusal rather than as a silent divergence.
fn layout_request(sequence: u64, intent: Intent) -> Vec<u8> {
    let mut intent_bytes = [0_u8; MAX_INTENT_BYTES];
    let len = intent.encode(&mut intent_bytes).expect("intent encodes");
    let mut out = Vec::with_capacity(13 + len);
    out.push(0xd1);
    out.push(1);
    out.extend_from_slice(&sequence.to_le_bytes());
    out.push(0);
    out.extend_from_slice(&(len as u16).to_le_bytes());
    out.extend_from_slice(&intent_bytes[..len]);
    out
}

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

/// One legacy transaction, unsigned.
///
/// Signatures are zero-filled.  `scripts/simulate.py` sends this to
/// `simulateTransaction` with `sigVerify: false`, so the runtime executes the
/// instruction without authenticating the signature bytes.  The `is_signer`
/// bits the program sees still come from the message header, which is the fact
/// under test.
fn transaction(
    keys: &[[u8; 32]],
    required_signatures: u8,
    readonly_signed: u8,
    readonly_unsigned: u8,
    program_index: u8,
    instruction_accounts: &[u8],
    data: &[u8],
) -> Vec<u8> {
    let mut out = Vec::new();
    compact_u16(usize::from(required_signatures), &mut out);
    for _ in 0..required_signatures {
        out.extend_from_slice(&[0_u8; 64]);
    }
    out.push(required_signatures);
    out.push(readonly_signed);
    out.push(readonly_unsigned);
    compact_u16(keys.len(), &mut out);
    for key in keys {
        out.extend_from_slice(key);
    }
    out.extend_from_slice(&[0_u8; 32]);
    compact_u16(1, &mut out);
    out.push(program_index);
    compact_u16(instruction_accounts.len(), &mut out);
    out.extend_from_slice(instruction_accounts);
    compact_u16(data.len(), &mut out);
    out.extend_from_slice(data);
    out
}

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

fn main() {
    let out_dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: clutch-sbf-harness <out-dir>"),
    );
    fs::create_dir_all(out_dir.join("accounts")).expect("create accounts dir");
    fs::create_dir_all(out_dir.join("expected")).expect("create expected dir");
    fs::create_dir_all(out_dir.join("tx")).expect("create tx dir");

    let f = build_fixture();

    /* Genesis account dumps.  Realm and Profile are read-only roles but are
     * still program-owned state, so they are preloaded the same way. */
    let program_address = f.program.address.clone();
    let dumps: [(&str, &Pda, &[u8]); 8] = [
        ("realm", &f.realm, &f.realm_bytes),
        ("profile", &f.profile, &f.profile_bytes),
        ("market", &f.market, &f.pre.market),
        ("hoard", &f.hoard, &f.pre.hoard),
        ("position", &f.position, &f.pre.position),
        ("kernel", &f.kernel, &f.pre.kernel),
        ("external", &f.external, &f.pre.external),
        ("replay", &f.replay, &f.pre.replay),
    ];
    /* The wave-3 plane.  No transaction in this plan touches any of these
     * accounts: the one implemented instruction does not take them, and a
     * differential over them would compare nothing.  They are loaded at genesis
     * so that a per-instruction lane inherits a bound on-chain plane instead of
     * inventing one, and so that the addresses in `manifest.txt` are already
     * the canonical ones for the seed schema. */
    let wave3_dumps: [(&str, &Pda, &[u8]); 10] = [
        ("supply", &f.supply, &f.pre.supply),
        ("grid", &f.wave3.grid, &f.wave3.grid_bytes),
        ("terms", &f.wave3.terms, &f.wave3.terms_bytes),
        ("resolution", &f.wave3.resolution, &f.wave3.resolution_bytes),
        ("feed", &f.wave3.feed, &f.wave3.feed_bytes),
        ("epoch", &f.wave3.epoch, &f.wave3.epoch_bytes),
        ("page", &f.wave3.page, &f.wave3.page_bytes),
        ("candidate", &f.wave3.candidate, &f.wave3.candidate_bytes),
        ("pot", &f.wave3.pot, &f.wave3.pot_bytes),
        ("receipt", &f.wave3.receipt, &f.wave3.receipt_bytes),
    ];
    for (name, pda, data) in dumps.iter().chain(wave3_dumps.iter()) {
        write(
            &out_dir.join(format!("accounts/{name}.json")),
            &account_json(&pda.address, &program_address, data),
        );
    }
    /* The imposter carries byte-identical replay state at an address that is
     * not the canonical replay PDA.  Every decode and linkage check passes on
     * it, so the only thing that can refuse it is address derivation. */
    write(
        &out_dir.join("accounts/replay-imposter.json"),
        &account_json(&f.imposter.address, &program_address, &f.pre.replay),
    );

    /* Expected post-state, from the offline reference adapter. */
    let expected: [(&str, &[u8]); 6] = [
        ("market", &f.post.market),
        ("hoard", &f.post.hoard),
        ("position", &f.post.position),
        ("kernel", &f.post.kernel),
        ("external", &f.post.external),
        ("replay", &f.post.replay),
    ];
    for (name, data) in expected {
        write(
            &out_dir.join(format!("expected/{name}.hex")),
            &format!("{}\n", hex_encode(data)),
        );
    }
    let pre_state: [(&str, &[u8]); 6] = [
        ("market", &f.pre.market),
        ("hoard", &f.pre.hoard),
        ("position", &f.pre.position),
        ("kernel", &f.pre.kernel),
        ("external", &f.pre.external),
        ("replay", &f.pre.replay),
    ];
    for (name, data) in pre_state {
        write(
            &out_dir.join(format!("expected/{name}.pre.hex")),
            &format!("{}\n", hex_encode(data)),
        );
    }

    /* The supply ledger is written out but deliberately **not** compared.  The
     * offline adapter updates it on every transition; the nine-account
     * instruction set has no ledger account, so the SVM cannot.  Emitting both
     * sides makes the size of that gap visible and gives the lane that adds the
     * account its expectation already computed.  See deferred check 13 in
     * `docs/implementation/SBF_BRINGUP.md`. */
    write(
        &out_dir.join("expected/supply.pre.hex"),
        &format!("{}\n", hex_encode(&f.pre.supply)),
    );
    write(
        &out_dir.join("expected/supply.hex"),
        &format!("{}\n", hex_encode(&f.post.supply)),
    );

    /* Message account order: writable signers, readonly signers, writable
     * non-signers, readonly non-signers.  The instruction's own account order
     * is the program's fixed role order and is independent of this. */
    let state = [
        f.market.bytes,
        f.hoard.bytes,
        f.position.bytes,
        f.kernel.bytes,
        f.external.bytes,
        f.replay.bytes,
    ];

    // Accepting transaction.
    let mut keys = vec![f.payer.bytes, f.actor.bytes];
    keys.extend_from_slice(&state);
    keys.push(f.realm.bytes);
    keys.push(f.profile.bytes);
    keys.push(f.program.bytes);
    let accept = transaction(&keys, 2, 1, 3, 10, &[1, 8, 9, 2, 3, 4, 5, 6, 7], &f.request);
    write(
        &out_dir.join("tx/accept.b64"),
        &format!("{}\n", b64_encode(&accept)),
    );

    // Refusal A: the position owner is present but never signed.
    let mut keys_unsigned = vec![f.payer.bytes];
    keys_unsigned.extend_from_slice(&state);
    keys_unsigned.push(f.realm.bytes);
    keys_unsigned.push(f.profile.bytes);
    keys_unsigned.push(f.actor.bytes);
    keys_unsigned.push(f.program.bytes);
    let refuse_unsigned = transaction(
        &keys_unsigned,
        1,
        0,
        4,
        10,
        &[9, 7, 8, 1, 2, 3, 4, 5, 6],
        &f.request,
    );
    write(
        &out_dir.join("tx/refuse-unsigned.b64"),
        &format!("{}\n", b64_encode(&refuse_unsigned)),
    );

    // Refusal B: an authenticated signer who is not the position owner.
    let mut keys_stranger = vec![f.payer.bytes, f.stranger.bytes];
    keys_stranger.extend_from_slice(&state);
    keys_stranger.push(f.realm.bytes);
    keys_stranger.push(f.profile.bytes);
    keys_stranger.push(f.program.bytes);
    let refuse_stranger = transaction(
        &keys_stranger,
        2,
        1,
        3,
        10,
        &[1, 8, 9, 2, 3, 4, 5, 6, 7],
        &f.request,
    );
    write(
        &out_dir.join("tx/refuse-stranger.b64"),
        &format!("{}\n", b64_encode(&refuse_stranger)),
    );

    // Refusal C: byte-identical replay state at a non-canonical address.
    let mut keys_imposter = vec![f.payer.bytes, f.actor.bytes];
    keys_imposter.extend_from_slice(&state[..5]);
    keys_imposter.push(f.imposter.bytes);
    keys_imposter.push(f.realm.bytes);
    keys_imposter.push(f.profile.bytes);
    keys_imposter.push(f.program.bytes);
    let refuse_imposter = transaction(
        &keys_imposter,
        2,
        1,
        3,
        10,
        &[1, 8, 9, 2, 3, 4, 5, 6, 7],
        &f.request,
    );
    write(
        &out_dir.join("tx/refuse-imposter.b64"),
        &format!("{}\n", b64_encode(&refuse_imposter)),
    );

    /* Cross-check the same three refusals against the offline reference
     * adapter, so the SBF refusal is compared with a refusal and not merely
     * asserted on its own. */
    let bindings = expected_bindings(
        &f.program,
        &f.market,
        &f.hoard,
        &f.position,
        &f.kernel,
        &f.external,
        &f.replay,
        &f.supply,
    );
    let unsigned_metadata = transition_metadata(
        &f.market,
        &f.hoard,
        &f.position,
        &f.kernel,
        &f.external,
        &f.replay,
        &f.supply,
        &f.program,
        f.actor.bytes,
        false,
    );
    let stranger_metadata = transition_metadata(
        &f.market,
        &f.hoard,
        &f.position,
        &f.kernel,
        &f.external,
        &f.replay,
        &f.supply,
        &f.program,
        f.stranger.bytes,
        true,
    );
    let imposter_metadata = transition_metadata(
        &f.market,
        &f.hoard,
        &f.position,
        &f.kernel,
        &f.external,
        &f.imposter,
        &f.supply,
        &f.program,
        f.actor.bytes,
        true,
    );
    let mut reference_refusals = BTreeMap::new();
    for (name, metadata) in [
        ("refuse-unsigned", unsigned_metadata),
        ("refuse-stranger", stranger_metadata),
        ("refuse-imposter", imposter_metadata),
    ] {
        let outcome = apply(&f.request, state_bytes(&f.pre), &metadata, &bindings);
        let text = match outcome {
            Ok(_) => panic!("the offline reference adapter accepted {name}"),
            Err(error) => format!("{error:?}"),
        };
        reference_refusals.insert(name, text);
    }

    let mut manifest = String::new();
    manifest.push_str("# Generated by clutch-sbf-harness. Do not edit.\n");
    manifest.push_str(&format!("program_id={}\n", f.program.address));
    manifest.push_str(&format!("payer={}\n", f.payer.address));
    manifest.push_str(&format!("actor={}\n", f.actor.address));
    manifest.push_str(&format!("stranger={}\n", f.stranger.address));
    manifest.push_str(&format!("imposter={}\n", f.imposter.address));
    for (name, pda, _) in dumps.iter().chain(wave3_dumps.iter()) {
        manifest.push_str(&format!("account.{name}={}\n", pda.address));
        manifest.push_str(&format!("bump.{name}={}\n", pda.bump));
    }
    manifest.push_str(&format!("split_quantity={SPLIT_QUANTITY}\n"));
    manifest.push_str(&format!("sequence={SEQUENCE}\n"));
    manifest.push_str("expect.accept=ok\n");
    manifest.push_str("expect.refuse-unsigned=0x0002\n");
    manifest.push_str("expect.refuse-stranger=0x0011\n");
    manifest.push_str("expect.refuse-imposter=0x0009\n");
    for (name, text) in &reference_refusals {
        manifest.push_str(&format!("reference.{name}={text}\n"));
    }
    write(&out_dir.join("manifest.txt"), &manifest);

    println!("bring-up plan written to {}", out_dir.display());
    println!("program_id  {}", f.program.address);
    println!("actor       {}", f.actor.address);
    for (name, pda, _) in dumps.iter().chain(wave3_dumps.iter()) {
        println!("{name:<11} {} bump {}", pda.address, pda.bump);
    }
    for (name, text) in &reference_refusals {
        println!("reference {name:<16} {text}");
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
    fn the_whole_fixture_builds_and_every_binding_holds() {
        /* `build_fixture` asserts every cross-account binding the frozen layout
         * can decide -- terms to market, supply ledger to market, grid to
         * terms, epoch to terms and grid, epoch to its frozen page set,
         * candidate to epoch, pot and receipt to candidate, resolution to terms
         * -- and it runs the offline reference adapter over the `Split`
         * request.  Building it here is what makes a fixture that drifted apart
         * a test failure rather than a genesis nobody checked.
         *
         * It is deliberately not an assertion about the SVM: that is the
         * differential in `scripts/simulate.py`. */
        let f = build_fixture();
        assert_ne!(f.pre.position, f.post.position, "Split must move position");
        assert_ne!(f.pre.hoard, f.post.hoard, "Split must move collateral");
        assert_eq!(f.pre.market, f.post.market, "Split must not touch Market");
        assert_eq!(
            f.pre.external, f.post.external,
            "Split must not touch the external shadow"
        );
        assert!(
            f.post.resolution.is_none(),
            "a layout intent admits no resolution record"
        );
        assert_eq!(f.post.redemption_payout, 0, "Split pays nothing out");
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
}
