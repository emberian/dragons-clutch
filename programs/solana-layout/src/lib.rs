#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Fixed, hostile-byte-facing layouts for the transparent V1 adapter.
//!
//! This crate deliberately contains no Solana SDK, allocator, CPI, token
//! implementation, RPC client, signing code, or entrypoint.  It only defines
//! byte ownership and deterministic intent bytes.  The eventual adapter must
//! authenticate account metadata and then hand these checked values to the
//! semantic kernel.

pub mod collateral;
pub mod stream;

/// Highest account schema version this build understands.
///
/// Each account carries its **own** schema version byte (see
/// [`account_version`]); this constant is the largest of them, not a single
/// wire version shared by every account.  An account keeps the version its
/// current bytes were introduced at; an account whose bytes change moves to the
/// next version and refuses every earlier one explicitly with
/// [`CodecError::WrongVersion`], so the pair `(tag, version)` never names two
/// shapes.
pub const LAYOUT_VERSION: u8 = 3;
/// The initial prototype schema version.
pub const LAYOUT_VERSION_V1: u8 = 1;
/// The schema version of the first persisted-state revision.
///
/// The dense order page encoded this version while its slots were bare 99-byte
/// single-Egg records; it now encodes [`account_version::ORDER_PAGE`] and
/// refuses this one.
pub const LAYOUT_VERSION_V2: u8 = 2;
/// Schema version of every deterministic intent encoding.
pub const INTENT_VERSION: u8 = 1;
/// Number of bytes in every identity/hash field.
pub const HASH_BYTES: usize = 32;

/// Exact byte length of the canonical parent Profile preimage.
///
/// Frozen by `docs/implementation/RESOLUTION_EVIDENCE_PLAN.md` §3.2: an eight
/// byte magic, parent schema and flags, the collateral subfield tag and its
/// schema, the 32-byte collateral-policy digest, and 16 zero reserved bytes.
/// [`canonical_profile_hash`] owns the length requirement; the parent
/// encoder/decoder that produces these bytes is
/// [`collateral::ParentProfile`].
pub const PROFILE_PARENT_BYTES: usize = 64;
/// Maximum number of outcomes in a market.
pub const MAX_OUTCOMES: usize = 16;
/// Maximum number of payout vectors in one immutable terms set.
///
/// Mirrors `clutch_kernel::MAX_PAYOUTS`; this crate stays dependency-free, so
/// the bound is restated rather than imported, and a codec test pins it.
pub const MAX_PAYOUTS: usize = 8;
/// Maximum number of ticks in a frozen price grid.
///
/// Mirrors `clutch_batch::MAX_GRID_TICKS`.
pub const MAX_GRID_TICKS: usize = 64;
/// Maximum records in one order page.
pub const MAX_ORDERS_PER_PAGE: usize = 16;
/// Maximum pages in one frozen epoch book.
pub const MAX_ORDER_PAGES: usize = 4;
/// Maximum orders in one frozen epoch book.
///
/// Mirrors `clutch_batch::MAX_ORDERS`; the page geometry is chosen so a full
/// page set is exactly one relation book.
pub const MAX_EPOCH_ORDERS: usize = MAX_ORDERS_PER_PAGE * MAX_ORDER_PAGES;
/// Maximum portfolio orders in one frozen epoch book.
///
/// Mirrors `clutch_batch::relation_v1::MAX_PORTFOLIO_ORDERS`.  A page set that
/// carries more portfolio records than this could never be one relation book,
/// so [`verify_page_set`] refuses it for the same reason the geometry above
/// exists.
pub const MAX_PORTFOLIO_ORDERS: usize = 8;
/// Relation version projected by [`EpochAccount`].
///
/// Mirrors `clutch_batch::relation_v1::RELATION_VERSION_V1`.
pub const RELATION_VERSION: u32 = 1;
/// Largest admitted observation bucket duration, in seconds.
///
/// Mirrors `clutch_accumulator::MAX_BUCKET_SECONDS`.
pub const MAX_BUCKET_SECONDS: u64 = 86_400;
/// Largest admitted observation window span, in buckets.
///
/// Mirrors `clutch_accumulator::MAX_BUCKETS`.
pub const MAX_WINDOW_BUCKETS: u64 = 1_000_000;
/// Largest price scale whose simplex sum cannot overflow a `u64`.
///
/// Mirrors `clutch_batch::relation_v1::MAX_PRICE_SCALE`.
pub const MAX_PRICE_SCALE: u64 = u64::MAX / MAX_OUTCOMES as u64;
/// Sentinel payout index meaning "this market has not resolved".
pub const PAYOUT_INDEX_UNRESOLVED: u8 = u8::MAX;
/// Exact encoded length of one [`OrderRecord`] body, without its slot kind byte.
pub const ORDER_RECORD_BYTES: usize = 32 + 32 + 1 + 1 + 8 + 8 + 8 + 1 + 8;
/// Exact encoded length of one [`PortfolioRecord`] body, without its kind byte.
///
/// The coefficient vector is stored at full [`MAX_OUTCOMES`] width with
/// canonical zero padding beyond `active_len`, exactly like every other
/// outcome-indexed vector here, so this length does not depend on how many
/// outcomes one portfolio actually touches.
pub const PORTFOLIO_RECORD_BYTES: usize = 32 + 32 + 1 + 1 + 1 + (MAX_OUTCOMES * 8) + 8 + 8 + 8 + 8;
/// Exact encoded length of one order slot in a page.
///
/// A slot is a one-byte kind discriminator, that kind's exact body, and
/// canonical zero padding out to this common width.  Fixing the slot width is
/// what lets one page hold both admitted order families while keeping a single
/// exact account length, a single strictly increasing order-id chain, and a
/// single page-set fold.  The padding is not slack: every byte of it is
/// required to be zero, so it can never influence a digest.
pub const ORDER_SLOT_BYTES: usize = 1 + PORTFOLIO_RECORD_BYTES;
/// Order-slot kind: canonical padding.  The whole slot is zero.
pub const ORDER_KIND_EMPTY: u8 = 0;
/// Order-slot kind: one single-Egg [`OrderRecord`].
pub const ORDER_KIND_SINGLE: u8 = 1;
/// Order-slot kind: one [`PortfolioRecord`].
pub const ORDER_KIND_PORTFOLIO: u8 = 2;
/// Maximum encoded instruction length.
pub const MAX_INTENT_BYTES: usize = 256;

const _: () = assert!(MAX_EPOCH_ORDERS == 64);
const _: () = assert!(MAX_PRICE_SCALE > 0);
// The slot is exactly wide enough for the widest record family and no wider.
const _: () = assert!(PORTFOLIO_RECORD_BYTES > ORDER_RECORD_BYTES);
const _: () = assert!(ORDER_SLOT_BYTES == 1 + PORTFOLIO_RECORD_BYTES);
const _: () = assert!(MAX_PORTFOLIO_ORDERS <= MAX_EPOCH_ORDERS);

const REALM_TAG: u8 = 1;
const PROFILE_TAG: u8 = 2;
const MARKET_TAG: u8 = 3;
const HOARD_TAG: u8 = 5;
const POSITION_TAG: u8 = 6;
const FEED_TAG: u8 = 7;
const ORDER_PAGE_TAG: u8 = 8;
const SUPPLY_LEDGER_TAG: u8 = 9;
const TERMS_TAG: u8 = 10;
const EPOCH_TAG: u8 = 11;
const PRICE_GRID_TAG: u8 = 12;
const CANDIDATE_TAG: u8 = 13;
const FINAL_POT_TAG: u8 = 14;
const SETTLEMENT_RECEIPT_TAG: u8 = 15;
const RESOLUTION_TAG: u8 = 16;

/// A fixed 32-byte domain-separated identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Hash32(pub [u8; HASH_BYTES]);

impl Hash32 {
    /// The all-zero identity, reserved as an absent-value sentinel.
    pub const ZERO: Self = Self([0; HASH_BYTES]);

    /// Construct an identity from raw bytes, refusing the reserved zero value.
    pub const fn new(bytes: [u8; HASH_BYTES]) -> Result<Self> {
        if is_zero(&bytes) {
            Err(CodecError::ZeroIdentity)
        } else {
            Ok(Self(bytes))
        }
    }

    /// Construct a hash from raw bytes without semantic validation.
    pub const fn from_bytes(bytes: [u8; HASH_BYTES]) -> Self {
        Self(bytes)
    }

    /// Return the raw bytes.
    pub const fn bytes(self) -> [u8; HASH_BYTES] {
        self.0
    }
}

/// A canonical Realm namespace identity.
pub type RealmHash = Hash32;
/// A canonical collateral/profile identity.
pub type ProfileHash = Hash32;
/// A canonical market identity.
pub type MarketId = Hash32;
/// A canonical outcome identity.
pub type OutcomeId = Hash32;
/// A canonical feed identity.
pub type FeedId = Hash32;
/// A canonical epoch identity.
pub type EpochId = Hash32;
/// A canonical owner identity (opaque to this crate).
pub type OwnerId = Hash32;

/// Errors returned by every parser and constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecError {
    /// Input ended before the fixed layout ended.
    Truncated,
    /// Input had bytes after the fixed layout.
    TrailingBytes,
    /// Account, intent, or order-slot discriminator was not recognized.
    WrongTag,
    /// Layout version is not supported.
    WrongVersion,
    /// A count was outside its frozen bound.
    InvalidCount,
    /// A reserved enum value or flag was present.
    InvalidEnum,
    /// A value that must be nonzero was zero.
    ZeroValue,
    /// A supplied identity was zero or not derivable from its parent.
    ZeroIdentity,
    /// An identity was not the canonical domain-separated value.
    NonCanonicalIdentity,
    /// Noncanonical bytes were found in padding or reserved fields.
    NonCanonicalPadding,
    /// A price grid is empty, unsorted, over-length, or exceeds its scale.
    InvalidPriceGrid,
    /// An order limit is not an exact member of the frozen price grid.
    InvalidTick,
    /// Two accounts that must agree on an immutable field disagree.
    MismatchedBinding,
    /// Internal plus accounted-external supply does not close.
    AggregateClosureMismatch,
    /// Consideration is not exactly quantity times the frozen price.
    InvalidConsideration,
    /// Arithmetic would exceed the fixed representation.
    ArithmeticOverflow,
    /// The destination buffer is too short.
    OutputTooSmall,
}

impl CodecError {
    /// The stable taxonomy code of this refusal.
    ///
    /// Numbers come from the `VECTOR_SPINE_PROPOSAL.md` §2.3 registry, which is
    /// PROPOSED.  Per its rule TAX-3 the enum's own discriminants are never a
    /// taxonomy code; this function is the only sanctioned mapping, and it is
    /// stable across variant insertion.
    pub const fn code(self) -> u32 {
        match self {
            Self::Truncated => 2011,
            Self::TrailingBytes => 2012,
            Self::WrongTag => 2030,
            Self::WrongVersion => 2031,
            Self::InvalidCount => 2040,
            Self::InvalidEnum => 2021,
            Self::ZeroValue => 2046,
            Self::ZeroIdentity => 4009,
            Self::NonCanonicalIdentity => 4010,
            Self::NonCanonicalPadding => 2022,
            Self::InvalidPriceGrid => 2049,
            Self::InvalidTick => 2050,
            Self::MismatchedBinding => 4011,
            Self::AggregateClosureMismatch => 5011,
            Self::InvalidConsideration => 5015,
            Self::ArithmeticOverflow => 1001,
            Self::OutputTooSmall => 8004,
        }
    }
}

/// Result used by this crate.
pub type Result<T> = core::result::Result<T, CodecError>;

const fn is_zero(bytes: &[u8; HASH_BYTES]) -> bool {
    let mut i = 0;
    while i < HASH_BYTES {
        if bytes[i] != 0 {
            return false;
        }
        i += 1;
    }
    true
}

/* SHA-256 is included as a tiny dependency-free identity primitive.  It is
 * used only for canonical IDs; this crate makes no claim that a future
 * Solana deployment has selected this primitive until its profile says so. */
fn digest(domain: &[u8], parts: &[&[u8]]) -> Hash32 {
    let mut h = Sha256::new();
    h.update(domain);
    let mut i = 0;
    while i < parts.len() {
        h.update(parts[i]);
        i += 1;
    }
    Hash32(h.finish())
}

/// Derive a canonical Realm hash from a profile/configuration digest.
pub fn canonical_realm_id(profile: ProfileHash, realm_nonce: u64) -> RealmHash {
    digest(
        b"dragons-clutch/realm/v1",
        &[&profile.0, &realm_nonce.to_le_bytes()],
    )
}

/// Derive the parent Profile hash from its exact canonical preimage.
///
/// The domain string and the algorithm are unchanged and stay frozen; what is
/// frozen here is *which bytes they consume*.
/// `docs/implementation/RESOLUTION_EVIDENCE_PLAN.md` §3.2 fixes the parent
/// Profile preimage at exactly [`PROFILE_PARENT_BYTES`] bytes, and §3.4
/// obligation 1 names the one real prefix-freeness hazard on the Rust side:
/// hashing a *variable-length* payload under a fixed domain string is not
/// prefix-free, so two different profiles could share a preimage boundary.
/// The exact length is therefore a refusal condition rather than a convention.
/// A shorter input is [`CodecError::Truncated`] and a longer one is
/// [`CodecError::TrailingBytes`], matching every other fixed codec here.
///
/// This function still computes an identity only. It does not decode the
/// parent Profile, does not recompute the collateral-policy subfield digest of
/// §3.2, and therefore proves nothing about which collateral policy a Profile
/// commits to. That binding check is [`collateral::verify_collateral_binding`],
/// and hashing bytes is never a substitute for it.
pub fn canonical_profile_hash(profile_bytes: &[u8]) -> Result<ProfileHash> {
    if profile_bytes.len() < PROFILE_PARENT_BYTES {
        return Err(CodecError::Truncated);
    }
    if profile_bytes.len() > PROFILE_PARENT_BYTES {
        return Err(CodecError::TrailingBytes);
    }
    Ok(digest(b"dragons-clutch/profile/v1", &[profile_bytes]))
}

/// Derive a canonical market ID from immutable namespace inputs.
pub fn canonical_market_id(realm: RealmHash, profile: ProfileHash, market_nonce: u64) -> MarketId {
    digest(
        b"dragons-clutch/market/v1",
        &[&realm.0, &profile.0, &market_nonce.to_le_bytes()],
    )
}

/// Derive an outcome ID from its canonical parent market and index.
pub fn canonical_outcome_id(market: MarketId, outcome_index: u8) -> OutcomeId {
    digest(b"dragons-clutch/outcome/v1", &[&market.0, &[outcome_index]])
}

/// Derive a canonical feed ID from its immutable feed specification bytes.
pub fn canonical_feed_id(spec_bytes: &[u8]) -> FeedId {
    digest(b"dragons-clutch/feed/v1", &[spec_bytes])
}

/// Derive a canonical epoch ID from its parent market and epoch index.
pub fn canonical_epoch_id(market: MarketId, epoch_index: u64) -> EpochId {
    digest(
        b"dragons-clutch/epoch/v1",
        &[&market.0, &epoch_index.to_le_bytes()],
    )
}

/// Derive the immutable terms digest from the exact terms body bytes.
///
/// The body is everything an encoded [`TermsAccount`] holds after its own
/// identity field, so the digest commits to the payout set, the window policy,
/// and the failure policy together.  [`MarketAccount::terms`] stores this value.
pub fn canonical_terms_digest(body_bytes: &[u8]) -> Hash32 {
    digest(b"dragons-clutch/terms/v1", &[body_bytes])
}

/// Derive the frozen price-grid identity from its exact body bytes.
pub fn canonical_price_grid_id(body_bytes: &[u8]) -> Hash32 {
    digest(b"dragons-clutch/price-grid/v1", &[body_bytes])
}

/// Derive a candidate identity from its exact body bytes.
pub fn canonical_candidate_digest(body_bytes: &[u8]) -> Hash32 {
    digest(b"dragons-clutch/candidate/v1", &[body_bytes])
}

/// Domain separator of the order-page digest.
///
/// It moved to `v2` when the page's records became fixed-width tagged slots:
/// the preimage shape changed, so the old domain must not be reusable over the
/// new bytes.  The order-set fold below keeps its own `v1` domain because its
/// preimage shape — market, epoch, page count, order count, page digests — did
/// not change; only the leaves it folds did, and those already carry the new
/// domain.
const ORDER_PAGE_DOMAIN: &[u8] = b"dragons-clutch/order-page/v2";

/// Derive one order page's digest from its page position and slot bytes.
///
/// `record_bytes` is the exact concatenation of all [`MAX_ORDERS_PER_PAGE`]
/// encoded slots, that is [`ORDER_SLOT_BYTES`] each including canonical
/// padding.  [`OrderPageAccount::recomputed_page_digest`] streams the same
/// bytes without buffering them.
pub fn canonical_page_digest(
    market: MarketId,
    epoch: EpochId,
    page_index: u16,
    order_count: u8,
    record_bytes: &[u8],
) -> Hash32 {
    digest(
        ORDER_PAGE_DOMAIN,
        &[
            &market.0,
            &epoch.0,
            &page_index.to_le_bytes(),
            &[order_count],
            record_bytes,
        ],
    )
}

/// Fold every page digest, in page order, into the set-wide order-set identity.
///
/// This is the cross-page commitment: a page cannot be added, dropped,
/// reordered, or mutated without changing the value every page of the set
/// stores in [`OrderPageAccount::order_set`].
pub fn canonical_order_set_id(
    market: MarketId,
    epoch: EpochId,
    page_count: u16,
    set_order_count: u16,
    page_digests: &[Hash32],
) -> Hash32 {
    let mut h = Sha256::new();
    h.update(b"dragons-clutch/order-set/v1");
    h.update(&market.0);
    h.update(&epoch.0);
    h.update(&page_count.to_le_bytes());
    h.update(&set_order_count.to_le_bytes());
    let mut i = 0;
    while i < page_digests.len() {
        h.update(&page_digests[i].0);
        i += 1;
    }
    Hash32(h.finish())
}

/// Check a market/outcome identity pair.
pub fn validate_outcome_id(market: MarketId, index: u8, id: OutcomeId) -> Result<()> {
    if index >= MAX_OUTCOMES as u8 || id != canonical_outcome_id(market, index) {
        return Err(CodecError::NonCanonicalIdentity);
    }
    Ok(())
}

/// Per-account schema versions.
///
/// An account keeps version `1` exactly while its bytes are unchanged from the
/// first prototype.  `PROFILE` grew a field and encodes `2`; `ORDER_PAGE` grew
/// the page-set commitment fields at `2` and then replaced its bare records
/// with tagged fixed-width slots, so it encodes `3` and refuses `1` and `2`.
/// The pair `(tag, version)` therefore never names two shapes.
pub mod account_version {
    /// Realm account, unchanged since the first prototype.
    pub const REALM: u8 = 1;
    /// Profile account; version 1 lacked the collateral-policy digest.
    pub const PROFILE: u8 = 2;
    /// Market account, unchanged since the first prototype.
    pub const MARKET: u8 = 1;
    /// Hoard account, unchanged since the first prototype.
    pub const HOARD: u8 = 1;
    /// Position account, unchanged since the first prototype.
    pub const POSITION: u8 = 1;
    /// Feed head account, unchanged since the first prototype.
    pub const FEED: u8 = 1;
    /// Order page; version 1 lacked every page-set commitment field and version
    /// 2 held bare [`super::ORDER_RECORD_BYTES`] single-Egg records with no kind
    /// discriminator and no portfolio family.
    pub const ORDER_PAGE: u8 = 3;
    /// Supply ledger account.
    pub const SUPPLY_LEDGER: u8 = 2;
    /// Immutable terms account.
    pub const TERMS: u8 = 2;
    /// Epoch/book-domain account.
    pub const EPOCH: u8 = 2;
    /// Frozen price-grid account.
    pub const PRICE_GRID: u8 = 2;
    /// Candidate record account.
    pub const CANDIDATE: u8 = 2;
    /// Final-pot account.
    pub const FINAL_POT: u8 = 2;
    /// Settlement receipt account.
    pub const SETTLEMENT_RECEIPT: u8 = 2;
    /// Resolution account.
    pub const RESOLUTION: u8 = 2;
}

/// Account discriminator and exact fixed byte lengths.
pub mod account_len {
    use super::{MAX_GRID_TICKS, MAX_ORDERS_PER_PAGE, MAX_OUTCOMES, MAX_PAYOUTS, ORDER_SLOT_BYTES};

    /// Realm account bytes.
    pub const REALM: usize = 2 + 32 + 32 + 1 + 1 + 1 + 1;
    /// Profile account bytes.
    pub const PROFILE: usize = 2 + 32 + 32 + 32 + 1 + 1;
    /// Market account bytes.
    pub const MARKET: usize = 2 + 32 + 32 + 32 + 32 + 1 + 1 + 1 + 1 + 512 + 32 + 8 + 8 + 32;
    /// Hoard account bytes.
    pub const HOARD: usize = 2 + 32 + 32 + 32 + 8 + 1 + 1;
    /// Position account bytes.
    pub const POSITION: usize = 2 + 32 + 32 + 8 + 128 + 8 + 8 + 1 + 1;
    /// Feed head account bytes.
    pub const FEED: usize = 2 + 32 + 32 + 8 + 8 + 8 + 32 + 1 + 1;
    /// Dense order page account bytes.
    pub const ORDER_PAGE: usize =
        2 + (7 * 32) + 2 + 2 + 2 + 1 + 1 + 1 + (MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES);
    /// Supply ledger account bytes.
    pub const SUPPLY_LEDGER: usize = 2 + 32 + 32 + 8 + 1 + (2 * MAX_OUTCOMES * 8) + 1 + 1;
    /// Immutable terms account bytes.
    pub const TERMS: usize = 2
        + (5 * 32)
        + 1
        + 1
        + (MAX_PAYOUTS * (8 + MAX_OUTCOMES * 8))
        + 4
        + 2
        + 8
        + 8
        + 8
        + 8
        + 4
        + 4
        + 4
        + 1
        + 1;
    /// Frozen price-grid account bytes.
    pub const PRICE_GRID: usize = 2 + 32 + 32 + 8 + 1 + (MAX_GRID_TICKS * 8) + 1 + 1;
    /// Epoch/book-domain account bytes.
    pub const EPOCH: usize = 2 + (9 * 32) + 8 + 4 + 8 + 8 + 2 + 2 + 2 + 1 + 1 + 1 + 1;
    /// Candidate record account bytes.
    pub const CANDIDATE: usize =
        2 + (3 * 32) + (MAX_OUTCOMES * 8) + 8 + 8 + 8 + 16 + 16 + 8 + 8 + 2 + 1 + 1 + 1 + 1 + 1;
    /// Final-pot account bytes.
    pub const FINAL_POT: usize = 2 + (3 * 32) + (MAX_OUTCOMES * 8) + 16 + 16 + 1 + 1 + 1 + 1;
    /// Settlement receipt account bytes.
    pub const SETTLEMENT_RECEIPT: usize = 2 + (5 * 32) + 16 + 8 + 8 + 8 + 8 + 2 + 1 + 1 + 1 + 1 + 1;
    /// Resolution account bytes.
    pub const RESOLUTION: usize = 2 + (4 * 32) + 8 + 8 + 8 + 8 + 1 + 1 + 1;
}

/// Realm collateral/profile configuration, frozen by an external adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmAccount {
    /// Realm identity.
    pub realm: RealmHash,
    /// Immutable collateral/profile hash.
    pub profile: ProfileHash,
    /// Maximum outcomes admitted by the profile (V1 must be 16).
    pub max_outcomes: u8,
    /// Adapter-visible profile version.
    pub profile_version: u8,
    /// Stored PDA bump, opaque to this crate.
    pub stored_bump: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
}

/// Immutable profile bytes identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileAccount {
    /// Profile identity.
    pub profile: ProfileHash,
    /// Owning Realm identity.
    pub realm: RealmHash,
    /// Domain-separated collateral-policy digest, at byte offset 66.
    ///
    /// Zero until the policy is frozen, and nonzero exactly when
    /// [`PROFILE_FLAG_POLICY_FROZEN`] is set in [`ProfileAccount::flags`]; the
    /// decoder refuses every other combination.  This account codec owns those
    /// 32 bytes and that zero-until-frozen rule and nothing more: it cannot tell
    /// whether the digest is the *right* one.
    ///
    /// The digest *algorithm* — domain string, preimage, and the Python/Rust
    /// cross-language equality — is owned by
    /// `research/collateral-profiles/model.py` and ported byte for byte in
    /// [`collateral`].  Recompute it from an actual 266-byte policy with
    /// [`collateral::verify_collateral_binding`] before treating a frozen
    /// Profile as evidence of anything; a well-formed frozen Profile can commit
    /// to another Realm's collateral policy.
    pub collateral_policy_digest: Hash32,
    /// Profile schema version.
    pub version: u8,
    /// Flags; bit 0 is [`PROFILE_FLAG_POLICY_FROZEN`], all other bits reserved.
    pub flags: u8,
}

/// Flag bit meaning the collateral policy digest is frozen and nonzero.
pub const PROFILE_FLAG_POLICY_FROZEN: u8 = 1;

/// Market account. Economics are interpreted by the kernel, not this codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketAccount {
    /// Canonical market identity.
    pub market: MarketId,
    /// Realm namespace.
    pub realm: RealmHash,
    /// Immutable profile hash.
    pub profile: ProfileHash,
    /// Immutable terms digest.
    pub terms: Hash32,
    /// Active outcome count, in `2..=MAX_OUTCOMES`.
    pub outcome_count: u8,
    /// Lifecycle enum: 0 active, 1 resolved, 2 closed.
    pub lifecycle: u8,
    /// Market PDA bump.
    pub stored_bump: u8,
    /// Hoard PDA bump.
    pub hoard_bump: u8,
    /// Canonical outcome identities; zero only in padding.
    pub outcomes: [OutcomeId; MAX_OUTCOMES],
    /// Feed or terminal-adapter identity.
    pub feed: FeedId,
    /// Collateral cap in opaque atoms; no arithmetic is performed here.
    pub collateral_cap: u64,
    /// Creation slot as supplied by the adapter.
    pub created_slot: u64,
    /// Reserved bytes; currently zero.
    pub reserved: Hash32,
}

/// Market-local collateral accounting state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HoardAccount {
    /// Market identity.
    pub market: MarketId,
    /// Realm identity.
    pub realm: RealmHash,
    /// Hoard authority identity (opaque PDA bytes).
    pub authority: Hash32,
    /// Collateral atoms held; this is not a fee or liveness balance.
    pub collateral_atoms: u64,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
}

/// Owner/market fixed position state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionAccount {
    /// Market identity.
    pub market: MarketId,
    /// Position owner identity.
    pub owner: OwnerId,
    /// Generation for close/reopen replay separation.
    pub generation: u64,
    /// Fixed internal outcome balances.
    pub internal: [u64; MAX_OUTCOMES],
    /// Realm collateral trading cash, if retained by the venue.
    pub cash_atoms: u64,
    /// Reserved collateral cash.
    pub reserved_cash_atoms: u64,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Close-state: 0 open, 1 close requested, 2 closed.
    pub close_state: u8,
}

/// Shared feed cursor and summary digest. Summary economics live in accumulator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeedAccount {
    /// Feed identity.
    pub feed: FeedId,
    /// Associated Realm identity.
    pub realm: RealmHash,
    /// Accepted cursor (monotone policy is adapter-owned).
    pub cursor: u64,
    /// Next logical boundary.
    pub next_boundary: u64,
    /// Number of archive pages.
    pub archive_pages: u64,
    /// Digest of the checked accumulator summary.
    pub summary: Hash32,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
}

/// A dense, canonically ordered page of transparent order slots.
///
/// Beyond within-page ordering, a page carries the whole page-set commitment:
/// its own digest, its order-id range, the previous page's last order id, and
/// the set-wide order-set digest.  Those fields are what make cross-page range,
/// uniqueness, and closure checkable from the bytes; see
/// [`verify_page_set`].
///
/// A slot holds either admitted order family — a single-Egg [`OrderRecord`] or
/// a [`PortfolioRecord`] — behind a one-byte kind discriminator at a fixed
/// [`ORDER_SLOT_BYTES`] width.  Both families therefore share one page, one
/// strictly increasing order-id chain, and one fold, which is what keeps
/// cross-family order-id uniqueness a property of the same checks that already
/// close the set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderPageAccount {
    /// Market identity.
    pub market: MarketId,
    /// Epoch identity.
    pub epoch: EpochId,
    /// Set-wide order-set digest; zero until the page set is frozen.
    pub order_set: Hash32,
    /// Digest of this page's position and record bytes.
    pub page_digest: Hash32,
    /// This page's lowest order id; zero exactly when the page is empty.
    pub first_order_id: Hash32,
    /// This page's highest order id; zero exactly when the page is empty.
    pub last_order_id: Hash32,
    /// The previous page's `last_order_id`; zero exactly on page zero.
    pub prev_page_last_order_id: Hash32,
    /// Zero-based page index.
    pub page_index: u16,
    /// Total frozen page count.
    pub page_count: u16,
    /// Orders across the whole page set; zero until the set is frozen.
    pub set_order_count: u16,
    /// Number of populated records.
    pub order_count: u8,
    /// Freeze state: 0 open, 1 frozen.
    pub frozen: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Slots in strictly increasing canonical order ID.
    pub orders: [OrderSlot; MAX_ORDERS_PER_PAGE],
}

/// One fixed-size transparent order record. It carries no matching result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderRecord {
    /// Position owner identity.
    pub owner: OwnerId,
    /// Canonical order identity.
    pub order_id: Hash32,
    /// Outcome index.
    pub outcome: u8,
    /// Side: 0 buy, 1 sell.
    pub side: u8,
    /// Quantity in opaque atoms.
    pub quantity: u64,
    /// Limit on the frozen venue scale.
    pub limit: u64,
    /// Minimum fill quantity.
    pub minimum_fill: u64,
    /// Flags: bit 0 all-or-none; all other bits reserved.
    pub flags: u8,
    /// Replay generation.
    pub generation: u64,
}

/// One fixed-size transparent portfolio order record.
///
/// A portfolio order is `lots` copies of one nonnegative coefficient vector over
/// the market's outcomes, bounded by a per-lot cash limit.  Like every other
/// outcome-indexed vector in this crate the coefficients are stored at full
/// [`MAX_OUTCOMES`] width with canonical zero padding at and beyond
/// `active_len`, so the record has one exact length and its padding can never
/// influence a digest.
///
/// This record carries no matching result and no economics.  Its fields are
/// exactly the persisted half of `clutch_batch::relation_v1::PortfolioOrderV1`;
/// the relation owns what they mean, and the field-by-field mapping — including
/// the two fields this crate deliberately does not persist — is written down in
/// `docs/implementation/SOLANA_LAYOUT.md`.
///
/// ```
/// use clutch_solana_layout::*;
///
/// let market = Hash32::from_bytes([1; 32]);
/// let epoch = canonical_epoch_id(market, 4);
///
/// // Three Eggs of outcome 0 and one of outcome 1 per lot, five lots, at most
/// // 9,000 cash units of collateral per lot on the frozen venue scale.
/// let mut coefficients = [0u64; MAX_OUTCOMES];
/// coefficients[0] = 3;
/// coefficients[1] = 1;
/// let portfolio = PortfolioRecord {
///     owner: Hash32::from_bytes([20; 32]),
///     order_id: Hash32::from_bytes([7; 32]),
///     side: 0,
///     active_len: 2,
///     flags: 0,
///     coefficients,
///     lots: 5,
///     limit_collateral_per_lot: 9_000,
///     minimum_fill_lots: 2,
///     generation: 1,
/// };
///
/// let mut orders = [OrderSlot::Empty; MAX_ORDERS_PER_PAGE];
/// orders[0] = OrderSlot::Portfolio(portfolio);
/// let mut page = OrderPageAccount {
///     market,
///     epoch,
///     order_set: Hash32::ZERO,
///     page_digest: Hash32::ZERO,
///     first_order_id: portfolio.order_id,
///     last_order_id: portfolio.order_id,
///     prev_page_last_order_id: Hash32::ZERO,
///     page_index: 0,
///     page_count: 1,
///     set_order_count: 0,
///     order_count: 1,
///     frozen: 0,
///     stored_bump: 5,
///     orders,
/// };
/// page.page_digest = page.recomputed_page_digest().unwrap();
///
/// let mut bytes = [0u8; account_len::ORDER_PAGE];
/// assert_eq!(page.encode(&mut bytes), Ok(account_len::ORDER_PAGE));
/// let decoded = OrderPageAccount::decode(&bytes).unwrap();
/// let record = match decoded.orders[0] {
///     OrderSlot::Portfolio(p) => p,
///     _ => panic!("slot 0 is a portfolio record"),
/// };
///
/// // The mapping contract: the layout owns these bytes, the relation owns
/// // their meaning.  Each assertion below is one `PortfolioOrderV1` field.
/// assert_eq!(record.coefficients, coefficients); // -> coefficients
/// assert_eq!(record.active_len, 2); //              -> active_len
/// assert_eq!(record.lots, 5); //                    -> lots
/// assert_eq!(record.limit_collateral_per_lot, 9_000); // -> limit_collateral_per_lot
/// assert_eq!(record.minimum_fill_lots, 2); //       -> minimum_fill_lots
/// assert_eq!(record.side, 0); //                    -> side == Side::Buy
/// assert_eq!(record.flags & 1, 0); //               -> partial_policy == Allow
///
/// // `canonical_order_id` and `owner` are the set-rank and owner-tag images of
/// // these 32-byte identities; `expiry_epoch` is not persisted by any record.
/// assert_eq!(record.order_id, Hash32::from_bytes([7; 32]));
/// assert_eq!(record.owner, Hash32::from_bytes([20; 32]));
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioRecord {
    /// Position owner identity.
    pub owner: OwnerId,
    /// Canonical order identity.
    pub order_id: Hash32,
    /// Side: 0 buy, 1 sell.
    pub side: u8,
    /// Active coefficient width, `1 ..= MAX_OUTCOMES`.
    pub active_len: u8,
    /// Flags: bit 0 all-or-none; all other bits reserved.
    pub flags: u8,
    /// Exact nonnegative Egg atoms per lot; zero at and beyond `active_len`.
    pub coefficients: [u64; MAX_OUTCOMES],
    /// Lots, nonzero.
    pub lots: u64,
    /// Per-lot cash bound, in complete-set units on the frozen venue scale.
    pub limit_collateral_per_lot: u64,
    /// Minimum acceptable lot fill, at most `lots`.
    pub minimum_fill_lots: u64,
    /// Replay generation.
    pub generation: u64,
}

impl PortfolioRecord {
    /// Validate a portfolio record without a frozen price scale.
    ///
    /// This is the scale-free half: identities, side and flag ranges, the
    /// coefficient width against its canonical zero padding, a nonzero demand,
    /// the lot bounds, and representability of the per-order Egg demand.  It
    /// deliberately decides nothing about whether the order is economically
    /// admissible; that is `clutch_batch`'s question.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.owner)?;
        check_hash(self.order_id)?;
        if self.side > 1 || self.flags & !1 != 0 {
            return Err(CodecError::InvalidEnum);
        }
        // `active_len` is a count, and it is the count the padding rule below
        // is stated against, so a bad width is refused before the padding is
        // read rather than after.
        if self.active_len == 0 || self.active_len as usize > MAX_OUTCOMES {
            return Err(CodecError::InvalidCount);
        }
        check_padded_amounts(&self.coefficients, self.active_len as usize)?;
        if self.lots == 0 {
            return Err(CodecError::ZeroValue);
        }
        if self.minimum_fill_lots > self.lots {
            return Err(CodecError::InvalidEnum);
        }
        if self.flags & 1 != 0 && self.minimum_fill_lots != self.lots {
            return Err(CodecError::InvalidEnum);
        }
        let demand = self.active_demand();
        // An all-zero active vector asks for nothing at any price; no
        // recomputation could ever accept it.
        if demand == 0 {
            return Err(CodecError::ZeroValue);
        }
        (self.lots as u128)
            .checked_mul(demand)
            .ok_or(CodecError::ArithmeticOverflow)?;
        Ok(())
    }

    /// Validate a portfolio record against a frozen price scale.
    ///
    /// The relation values one lot at `sum(coefficients[i] * price[i])` and
    /// bounds it by `limit_collateral_per_lot * price_scale`; both products,
    /// times `lots`, must stay inside the `u128` ledger or no candidate could
    /// ever be classified against this order.  The scale is frozen in
    /// [`PriceGridAccount`], which is why this is separate from
    /// [`PortfolioRecord::validate`] and why
    /// [`OrderPageAccount::decode_on_grid`] is the decoder that applies it.
    ///
    /// A portfolio's cash bound is **not** looked up in the tick vector.  The
    /// grid's domain is a per-outcome limit price in `0 ..= price_scale`; a
    /// per-lot collateral bound is in complete-set units and can legitimately
    /// exceed the scale, so it has no tick and none is required.
    pub fn validate_on_scale(&self, price_scale: u64) -> Result<()> {
        self.validate()?;
        if price_scale == 0 || price_scale > MAX_PRICE_SCALE {
            return Err(CodecError::InvalidPriceGrid);
        }
        let scale = price_scale as u128;
        let per_lot_value = self
            .active_demand()
            .checked_mul(scale)
            .ok_or(CodecError::ArithmeticOverflow)?;
        (self.lots as u128)
            .checked_mul(per_lot_value)
            .ok_or(CodecError::ArithmeticOverflow)?;
        let per_lot_bound = (self.limit_collateral_per_lot as u128)
            .checked_mul(scale)
            .ok_or(CodecError::ArithmeticOverflow)?;
        (self.lots as u128)
            .checked_mul(per_lot_bound)
            .ok_or(CodecError::ArithmeticOverflow)?;
        Ok(())
    }

    /// Sum of the active coefficients.  At most `16 * (2^64 - 1)`, so the
    /// accumulation itself cannot overflow a `u128`.
    fn active_demand(&self) -> u128 {
        let mut sum: u128 = 0;
        let mut i = 0;
        while i < self.active_len as usize && i < MAX_OUTCOMES {
            sum += self.coefficients[i] as u128;
            i += 1;
        }
        sum
    }
}

/// One page slot: canonical padding, a single-Egg record, or a portfolio record.
///
/// Every slot occupies exactly [`ORDER_SLOT_BYTES`] bytes — a one-byte kind
/// discriminator, that kind's exact body, and canonical zero padding to the
/// common width — so a page keeps one exact account length no matter which
/// families it holds.  Canonical padding is the all-zero slot, which is also
/// [`ORDER_KIND_EMPTY`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderSlot {
    /// Canonical padding: [`ORDER_SLOT_BYTES`] zero bytes, no record.
    Empty,
    /// One single-Egg order on one outcome.
    Single(OrderRecord),
    /// One portfolio order over a coefficient vector.
    Portfolio(PortfolioRecord),
}

impl OrderSlot {
    /// This slot's kind discriminator byte.
    pub fn kind(&self) -> u8 {
        match self {
            Self::Empty => ORDER_KIND_EMPTY,
            Self::Single(_) => ORDER_KIND_SINGLE,
            Self::Portfolio(_) => ORDER_KIND_PORTFOLIO,
        }
    }
    /// The record's canonical order identity, or zero for padding.
    pub fn order_id(&self) -> Hash32 {
        match self {
            Self::Empty => Hash32::ZERO,
            Self::Single(o) => o.order_id,
            Self::Portfolio(p) => p.order_id,
        }
    }
    /// The record's owner identity, or zero for padding.
    pub fn owner(&self) -> OwnerId {
        match self {
            Self::Empty => Hash32::ZERO,
            Self::Single(o) => o.owner,
            Self::Portfolio(p) => p.owner,
        }
    }
    /// Whether this slot holds a portfolio record.
    pub fn is_portfolio(&self) -> bool {
        matches!(self, Self::Portfolio(_))
    }
    /// Validate a populated slot.
    ///
    /// Padding has no record to validate and is refused here; a page reaches
    /// this only for slots below its own `order_count`, where an empty slot is
    /// a missing order rather than padding.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Empty => Err(CodecError::ZeroIdentity),
            Self::Single(o) => o.validate(),
            Self::Portfolio(p) => p.validate(),
        }
    }
}

fn check_hash(hash: Hash32) -> Result<()> {
    if hash == Hash32::ZERO {
        Err(CodecError::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn check_count(count: u8) -> Result<()> {
    if count < 2 || count as usize > MAX_OUTCOMES {
        Err(CodecError::InvalidCount)
    } else {
        Ok(())
    }
}

fn check_header(input: &[u8], tag: u8, version: u8, len: usize) -> Result<()> {
    if input.len() < len {
        return Err(CodecError::Truncated);
    }
    if input.len() > len {
        return Err(CodecError::TrailingBytes);
    }
    if input[0] != tag {
        return Err(CodecError::WrongTag);
    }
    // Every version other than this account's own schema version is refused,
    // including the superseded prototype version 1 and any future version.
    if input[1] != version {
        return Err(CodecError::WrongVersion);
    }
    Ok(())
}

fn check_padded_amounts(values: &[u64; MAX_OUTCOMES], active: usize) -> Result<()> {
    let mut i = active;
    while i < MAX_OUTCOMES {
        if values[i] != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        i += 1;
    }
    Ok(())
}

struct Writer<'a> {
    out: &'a mut [u8],
    at: usize,
}
impl<'a> Writer<'a> {
    fn new(out: &'a mut [u8]) -> Self {
        Self { out, at: 0 }
    }
    fn bytes(&mut self, value: &[u8]) -> Result<()> {
        let end = self
            .at
            .checked_add(value.len())
            .ok_or(CodecError::OutputTooSmall)?;
        if end > self.out.len() {
            return Err(CodecError::OutputTooSmall);
        }
        self.out[self.at..end].copy_from_slice(value);
        self.at = end;
        Ok(())
    }
    fn u8(&mut self, value: u8) -> Result<()> {
        self.bytes(&[value])
    }
    fn u16(&mut self, value: u16) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }
    fn u32(&mut self, value: u32) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }
    fn u128(&mut self, value: u128) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }
    fn i128(&mut self, value: i128) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }
    fn hash(&mut self, value: Hash32) -> Result<()> {
        self.bytes(&value.0)
    }
    fn amounts(&mut self, values: &[u64; MAX_OUTCOMES]) -> Result<()> {
        let mut i = 0;
        while i < MAX_OUTCOMES {
            self.u64(values[i])?;
            i += 1;
        }
        Ok(())
    }
}

struct Reader<'a> {
    input: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn new(input: &'a [u8], tag: u8, version: u8, len: usize) -> Result<Self> {
        check_header(input, tag, version, len)?;
        Ok(Self { input, at: 2 })
    }
    /// Position a reader at a byte offset inside an already-checked buffer.
    ///
    /// The streaming page decoders use this to read one slot without walking
    /// the slots before it; the caller has already established the buffer's
    /// tag, version, and exact length.
    fn at(input: &'a [u8], at: usize) -> Self {
        Self { input, at }
    }
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.at.checked_add(N).ok_or(CodecError::Truncated)?;
        if end > self.input.len() {
            return Err(CodecError::Truncated);
        }
        let mut value = [0; N];
        value.copy_from_slice(&self.input[self.at..end]);
        self.at = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes::<2>()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes::<4>()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes::<8>()?))
    }
    fn u128(&mut self) -> Result<u128> {
        Ok(u128::from_le_bytes(self.bytes::<16>()?))
    }
    fn i128(&mut self) -> Result<i128> {
        Ok(i128::from_le_bytes(self.bytes::<16>()?))
    }
    fn hash(&mut self) -> Result<Hash32> {
        Ok(Hash32(self.bytes::<32>()?))
    }
    fn amounts(&mut self) -> Result<[u64; MAX_OUTCOMES]> {
        let mut values = [0; MAX_OUTCOMES];
        let mut i = 0;
        while i < MAX_OUTCOMES {
            values[i] = self.u64()?;
            i += 1;
        }
        Ok(values)
    }
    fn done(self) -> Result<()> {
        if self.at == self.input.len() {
            Ok(())
        } else {
            Err(CodecError::TrailingBytes)
        }
    }
}

fn put_header(w: &mut Writer<'_>, tag: u8, version: u8) -> Result<()> {
    w.u8(tag)?;
    w.u8(version)
}

impl RealmAccount {
    /// Validate semantic shape without external account metadata.
    ///
    /// V1 freezes `max_outcomes` at exactly [`MAX_OUTCOMES`]; a Realm claiming
    /// any smaller admitted width is refused rather than silently accepted.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.realm)?;
        check_hash(self.profile)?;
        if self.max_outcomes as usize != MAX_OUTCOMES {
            return Err(CodecError::InvalidCount);
        }
        if self.profile_version == 0 || self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        Ok(())
    }
    /// Encode exactly [`account_len::REALM`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::REALM {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, REALM_TAG, account_version::REALM)?;
        w.hash(self.realm)?;
        w.hash(self.profile)?;
        w.u8(self.max_outcomes)?;
        w.u8(self.profile_version)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::REALM`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(input, REALM_TAG, account_version::REALM, account_len::REALM)?;
        let v = Self {
            realm: r.hash()?,
            profile: r.hash()?,
            max_outcomes: r.u8()?,
            profile_version: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
}

impl ProfileAccount {
    /// Validate profile shape, parent identity, and policy-freeze consistency.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.profile)?;
        check_hash(self.realm)?;
        if self.version == 0 || self.flags & !PROFILE_FLAG_POLICY_FROZEN != 0 {
            return Err(CodecError::InvalidEnum);
        }
        let frozen = self.flags & PROFILE_FLAG_POLICY_FROZEN != 0;
        if frozen == (self.collateral_policy_digest == Hash32::ZERO) {
            return Err(if frozen {
                CodecError::ZeroIdentity
            } else {
                CodecError::NonCanonicalPadding
            });
        }
        Ok(())
    }
    /// Encode exactly [`account_len::PROFILE`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::PROFILE {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, PROFILE_TAG, account_version::PROFILE)?;
        w.hash(self.profile)?;
        w.hash(self.realm)?;
        w.hash(self.collateral_policy_digest)?;
        w.u8(self.version)?;
        w.u8(self.flags)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::PROFILE`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(
            input,
            PROFILE_TAG,
            account_version::PROFILE,
            account_len::PROFILE,
        )?;
        let v = Self {
            profile: r.hash()?,
            realm: r.hash()?,
            collateral_policy_digest: r.hash()?,
            version: r.u8()?,
            flags: r.u8()?,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
}

impl MarketAccount {
    /// Validate IDs, outcomes, lifecycle and canonical padding.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.market)?;
        check_hash(self.realm)?;
        check_hash(self.profile)?;
        check_hash(self.terms)?;
        check_count(self.outcome_count)?;
        if self.lifecycle > 2 {
            return Err(CodecError::InvalidEnum);
        };
        check_hash(self.feed)?;
        let mut i = 0;
        while i < MAX_OUTCOMES {
            if i < self.outcome_count as usize {
                validate_outcome_id(self.market, i as u8, self.outcomes[i])?
            } else if self.outcomes[i] != Hash32::ZERO {
                return Err(CodecError::NonCanonicalPadding);
            };
            i += 1;
        }
        if self.reserved != Hash32::ZERO {
            return Err(CodecError::NonCanonicalPadding);
        };
        Ok(())
    }
    /// Encode exactly [`account_len::MARKET`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::MARKET {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, MARKET_TAG, account_version::MARKET)?;
        w.hash(self.market)?;
        w.hash(self.realm)?;
        w.hash(self.profile)?;
        w.hash(self.terms)?;
        w.u8(self.outcome_count)?;
        w.u8(self.lifecycle)?;
        w.u8(self.stored_bump)?;
        w.u8(self.hoard_bump)?;
        let mut i = 0;
        while i < MAX_OUTCOMES {
            w.hash(self.outcomes[i])?;
            i += 1;
        }
        w.hash(self.feed)?;
        w.u64(self.collateral_cap)?;
        w.u64(self.created_slot)?;
        w.hash(self.reserved)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::MARKET`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(
            input,
            MARKET_TAG,
            account_version::MARKET,
            account_len::MARKET,
        )?;
        let market = r.hash()?;
        let realm = r.hash()?;
        let profile = r.hash()?;
        let terms = r.hash()?;
        let outcome_count = r.u8()?;
        let lifecycle = r.u8()?;
        let stored_bump = r.u8()?;
        let hoard_bump = r.u8()?;
        let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
        let mut i = 0;
        while i < MAX_OUTCOMES {
            outcomes[i] = r.hash()?;
            i += 1;
        }
        let v = Self {
            market,
            realm,
            profile,
            terms,
            outcome_count,
            lifecycle,
            stored_bump,
            hoard_bump,
            outcomes,
            feed: r.hash()?,
            collateral_cap: r.u64()?,
            created_slot: r.u64()?,
            reserved: r.hash()?,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
}

impl HoardAccount {
    /// Validate market-local accounting shape.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.market)?;
        check_hash(self.realm)?;
        check_hash(self.authority)?;
        if self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        };
        Ok(())
    }
    /// Encode exactly [`account_len::HOARD`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::HOARD {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, HOARD_TAG, account_version::HOARD)?;
        w.hash(self.market)?;
        w.hash(self.realm)?;
        w.hash(self.authority)?;
        w.u64(self.collateral_atoms)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::HOARD`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(input, HOARD_TAG, account_version::HOARD, account_len::HOARD)?;
        let v = Self {
            market: r.hash()?,
            realm: r.hash()?,
            authority: r.hash()?,
            collateral_atoms: r.u64()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
}

impl PositionAccount {
    /// Validate close state and the frozen cash decomposition.
    ///
    /// `cash_atoms` is the **total** Realm collateral cash held for this
    /// position and `reserved_cash_atoms` is the encumbered part of that total,
    /// so free cash is their difference and the reserved part can never exceed
    /// the total.  A byte pattern claiming otherwise is refused.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.market)?;
        check_hash(self.owner)?;
        if self.close_state > 2 {
            return Err(CodecError::InvalidEnum);
        };
        if self.reserved_cash_atoms > self.cash_atoms {
            return Err(CodecError::AggregateClosureMismatch);
        }
        Ok(())
    }
    /// Free (unencumbered) collateral cash, or a refusal if the split is invalid.
    pub const fn free_cash_atoms(&self) -> Result<u64> {
        match self.cash_atoms.checked_sub(self.reserved_cash_atoms) {
            Some(free) => Ok(free),
            None => Err(CodecError::AggregateClosureMismatch),
        }
    }
    /// Encode exactly [`account_len::POSITION`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::POSITION {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, POSITION_TAG, account_version::POSITION)?;
        w.hash(self.market)?;
        w.hash(self.owner)?;
        w.u64(self.generation)?;
        let mut i = 0;
        while i < MAX_OUTCOMES {
            w.u64(self.internal[i])?;
            i += 1;
        }
        w.u64(self.cash_atoms)?;
        w.u64(self.reserved_cash_atoms)?;
        w.u8(self.stored_bump)?;
        w.u8(self.close_state)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::POSITION`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(
            input,
            POSITION_TAG,
            account_version::POSITION,
            account_len::POSITION,
        )?;
        let market = r.hash()?;
        let owner = r.hash()?;
        let generation = r.u64()?;
        let mut internal = [0; MAX_OUTCOMES];
        let mut i = 0;
        while i < MAX_OUTCOMES {
            internal[i] = r.u64()?;
            i += 1;
        }
        let v = Self {
            market,
            owner,
            generation,
            internal,
            cash_atoms: r.u64()?,
            reserved_cash_atoms: r.u64()?,
            stored_bump: r.u8()?,
            close_state: r.u8()?,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
}

impl FeedAccount {
    /// Validate feed cursor state shape.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.feed)?;
        check_hash(self.realm)?;
        check_hash(self.summary)?;
        if self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        };
        Ok(())
    }
    /// Encode exactly [`account_len::FEED`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::FEED {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, FEED_TAG, account_version::FEED)?;
        w.hash(self.feed)?;
        w.hash(self.realm)?;
        w.u64(self.cursor)?;
        w.u64(self.next_boundary)?;
        w.u64(self.archive_pages)?;
        w.hash(self.summary)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::FEED`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(input, FEED_TAG, account_version::FEED, account_len::FEED)?;
        let v = Self {
            feed: r.hash()?,
            realm: r.hash()?,
            cursor: r.u64()?,
            next_boundary: r.u64()?,
            archive_pages: r.u64()?,
            summary: r.hash()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
}

impl OrderPageAccount {
    /// Recompute this page's digest from its position and slot bytes.
    ///
    /// The slots are streamed into the hash one at a time instead of being
    /// buffered, so recomputing a digest costs one [`ORDER_SLOT_BYTES`] scratch
    /// slot rather than a whole page of stack.  The value is identical to
    /// [`canonical_page_digest`] over the concatenated slot bytes and a test
    /// pins that equality.
    pub fn recomputed_page_digest(&self) -> Result<Hash32> {
        let mut h = Sha256::new();
        h.update(ORDER_PAGE_DOMAIN);
        h.update(&self.market.0);
        h.update(&self.epoch.0);
        h.update(&self.page_index.to_le_bytes());
        h.update(&[self.order_count]);
        let mut slot = [0; ORDER_SLOT_BYTES];
        let mut i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            let mut w = Writer::new(&mut slot);
            encode_slot(&mut w, self.orders[i])?;
            h.update(&slot);
            i += 1;
        }
        Ok(Hash32(h.finish()))
    }
    /// Validate dense ordering, page bounds, records, commitments, and padding.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.market)?;
        check_hash(self.epoch)?;
        if self.frozen > 1 {
            return Err(CodecError::InvalidEnum);
        }
        let frozen = self.frozen == 1;
        if self.page_count == 0
            || self.page_count as usize > MAX_ORDER_PAGES
            || self.page_index >= self.page_count
            || self.order_count as usize > MAX_ORDERS_PER_PAGE
            || (frozen && self.order_count == 0)
        {
            return Err(CodecError::InvalidCount);
        };
        let mut previous = Hash32::ZERO;
        let mut portfolios = 0usize;
        let mut i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            if i < self.order_count as usize {
                self.orders[i].validate()?;
                let id = self.orders[i].order_id();
                if id == Hash32::ZERO || (previous != Hash32::ZERO && id.0 <= previous.0) {
                    return Err(CodecError::NonCanonicalIdentity);
                };
                previous = id;
                if self.orders[i].is_portfolio() {
                    portfolios += 1;
                }
            } else if self.orders[i] != OrderSlot::Empty {
                return Err(CodecError::NonCanonicalPadding);
            };
            i += 1;
        }
        // One page cannot hold more portfolio records than the whole frozen
        // book admits; see [`MAX_PORTFOLIO_ORDERS`].  The set-wide sum is
        // checked in [`verify_page_set`].
        if portfolios > MAX_PORTFOLIO_ORDERS {
            return Err(CodecError::InvalidCount);
        }
        // The stored range must be exactly the records' range.
        let (expected_first, expected_last) = if self.order_count == 0 {
            (Hash32::ZERO, Hash32::ZERO)
        } else {
            (
                self.orders[0].order_id(),
                self.orders[self.order_count as usize - 1].order_id(),
            )
        };
        if self.first_order_id != expected_first || self.last_order_id != expected_last {
            return Err(CodecError::MismatchedBinding);
        }
        // Page zero opens the chain; every later page links to its predecessor
        // and must open strictly above it, which is what makes the cross-page
        // order-id sequence strictly increasing rather than merely per-page.
        if self.page_index == 0 {
            if self.prev_page_last_order_id != Hash32::ZERO {
                return Err(CodecError::NonCanonicalPadding);
            }
        } else {
            if self.prev_page_last_order_id == Hash32::ZERO {
                return Err(CodecError::ZeroIdentity);
            }
            if self.order_count > 0 && self.first_order_id.0 <= self.prev_page_last_order_id.0 {
                return Err(CodecError::NonCanonicalIdentity);
            }
        }
        // Set-wide commitments exist exactly while the set is frozen.
        if frozen {
            check_hash(self.order_set)?;
            if self.set_order_count < self.order_count as u16
                || self.set_order_count as usize > MAX_EPOCH_ORDERS
            {
                return Err(CodecError::InvalidCount);
            }
            let full_pages = self.page_count as usize - 1;
            let low = full_pages * MAX_ORDERS_PER_PAGE;
            if self.page_index as usize + 1 == self.page_count as usize {
                // The last page closes the count exactly.
                if self.set_order_count as usize != low + self.order_count as usize {
                    return Err(CodecError::MismatchedBinding);
                }
            } else {
                // Every non-final page of a frozen set is dense.
                if self.order_count as usize != MAX_ORDERS_PER_PAGE {
                    return Err(CodecError::InvalidCount);
                }
                if self.set_order_count as usize <= low
                    || self.set_order_count as usize > low + MAX_ORDERS_PER_PAGE
                {
                    return Err(CodecError::MismatchedBinding);
                }
            }
        } else if self.order_set != Hash32::ZERO || self.set_order_count != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        if self.page_digest != self.recomputed_page_digest()? {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }
    /// Encode exactly [`account_len::ORDER_PAGE`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::ORDER_PAGE {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, ORDER_PAGE_TAG, account_version::ORDER_PAGE)?;
        w.hash(self.market)?;
        w.hash(self.epoch)?;
        w.hash(self.order_set)?;
        w.hash(self.page_digest)?;
        w.hash(self.first_order_id)?;
        w.hash(self.last_order_id)?;
        w.hash(self.prev_page_last_order_id)?;
        w.u16(self.page_index)?;
        w.u16(self.page_count)?;
        w.u16(self.set_order_count)?;
        w.u8(self.order_count)?;
        w.u8(self.frozen)?;
        w.u8(self.stored_bump)?;
        let mut i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            encode_slot(&mut w, self.orders[i])?;
            i += 1;
        }
        Ok(w.at)
    }
    /// Parse exactly [`account_len::ORDER_PAGE`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(
            input,
            ORDER_PAGE_TAG,
            account_version::ORDER_PAGE,
            account_len::ORDER_PAGE,
        )?;
        let market = r.hash()?;
        let epoch = r.hash()?;
        let order_set = r.hash()?;
        let page_digest = r.hash()?;
        let first_order_id = r.hash()?;
        let last_order_id = r.hash()?;
        let prev_page_last_order_id = r.hash()?;
        let page_index = r.u16()?;
        let page_count = r.u16()?;
        let set_order_count = r.u16()?;
        let order_count = r.u8()?;
        let frozen = r.u8()?;
        let stored_bump = r.u8()?;
        let mut orders = [OrderSlot::Empty; MAX_ORDERS_PER_PAGE];
        let mut i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            orders[i] = decode_slot(&mut r)?;
            i += 1;
        }
        let v = Self {
            market,
            epoch,
            order_set,
            page_digest,
            first_order_id,
            last_order_id,
            prev_page_last_order_id,
            page_index,
            page_count,
            set_order_count,
            order_count,
            frozen,
            stored_bump,
            orders,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
    /// Parse a page and apply every check that needs the frozen price grid.
    ///
    /// A single-Egg record's `limit` must be an exact member of the tick vector
    /// or it has no tick, which is [`CodecError::InvalidTick`].  A portfolio
    /// record has no tick to look up — its bound is a per-lot collateral in
    /// complete-set units, not a per-outcome limit price — so what the grid
    /// contributes there is the frozen scale, against which
    /// [`PortfolioRecord::validate_on_scale`] refuses bounds that no candidate
    /// could ever be classified against.
    pub fn decode_on_grid(input: &[u8], grid: &PriceGridAccount) -> Result<Self> {
        let page = Self::decode(input)?;
        grid.validate()?;
        let mut i = 0;
        while i < page.order_count as usize {
            match page.orders[i] {
                OrderSlot::Single(o) => {
                    grid.tick_of(o.limit)?;
                }
                OrderSlot::Portfolio(p) => p.validate_on_scale(grid.price_scale)?,
                // Unreachable after `decode`, which refuses an empty slot below
                // `order_count`; stated rather than assumed.
                OrderSlot::Empty => return Err(CodecError::ZeroIdentity),
            }
            i += 1;
        }
        Ok(page)
    }
}

/// Verify cross-page order range, uniqueness, and closure over a frozen set.
///
/// The pages must be supplied in page-index order.  On success the recomputed
/// set-wide order-set digest is returned; it is the value every page stores and
/// the value [`EpochAccount::order_set`] must equal.  A dropped page, a
/// duplicated order id across a page boundary, a reordered page, or any
/// post-freeze byte mutation changes one of the checked commitments.
pub fn verify_page_set(pages: &[OrderPageAccount]) -> Result<Hash32> {
    if pages.is_empty() || pages.len() > MAX_ORDER_PAGES {
        return Err(CodecError::InvalidCount);
    }
    let head = &pages[0];
    if head.page_count as usize != pages.len() {
        return Err(CodecError::InvalidCount);
    }
    let mut digests = [Hash32::ZERO; MAX_ORDER_PAGES];
    let mut total: u16 = 0;
    let mut portfolios = 0usize;
    let mut i = 0;
    while i < pages.len() {
        let page = &pages[i];
        page.validate()?;
        if page.frozen != 1 {
            return Err(CodecError::MismatchedBinding);
        }
        if page.page_index as usize != i
            || page.page_count != head.page_count
            || page.market != head.market
            || page.epoch != head.epoch
            || page.order_set != head.order_set
            || page.set_order_count != head.set_order_count
        {
            return Err(CodecError::MismatchedBinding);
        }
        if i == 0 {
            if page.prev_page_last_order_id != Hash32::ZERO {
                return Err(CodecError::NonCanonicalPadding);
            }
        } else if page.prev_page_last_order_id != pages[i - 1].last_order_id {
            return Err(CodecError::NonCanonicalIdentity);
        }
        total = total
            .checked_add(page.order_count as u16)
            .ok_or(CodecError::ArithmeticOverflow)?;
        let mut j = 0;
        while j < page.order_count as usize {
            if page.orders[j].is_portfolio() {
                portfolios += 1;
            }
            j += 1;
        }
        digests[i] = page.page_digest;
        i += 1;
    }
    if total != head.set_order_count {
        return Err(CodecError::MismatchedBinding);
    }
    // A set carrying more portfolio records than the relation admits could
    // never be one book, exactly as a set of more than `MAX_ORDER_PAGES` pages
    // could not.  Both are restated bounds, not local policy.
    if portfolios > MAX_PORTFOLIO_ORDERS {
        return Err(CodecError::InvalidCount);
    }
    let order_set = canonical_order_set_id(
        head.market,
        head.epoch,
        head.page_count,
        head.set_order_count,
        &digests[..pages.len()],
    );
    if order_set != head.order_set {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(order_set)
}

impl OrderRecord {
    /// Validate an order without interpreting price/economic semantics.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.owner)?;
        check_hash(self.order_id)?;
        if self.outcome >= MAX_OUTCOMES as u8
            || self.side > 1
            || self.quantity == 0
            || self.minimum_fill > self.quantity
            || self.flags & !1 != 0
        {
            return Err(CodecError::InvalidEnum);
        };
        if self.flags & 1 != 0 && self.minimum_fill != self.quantity {
            return Err(CodecError::InvalidEnum);
        };
        Ok(())
    }
}
fn encode_order(w: &mut Writer<'_>, o: OrderRecord) -> Result<()> {
    w.hash(o.owner)?;
    w.hash(o.order_id)?;
    w.u8(o.outcome)?;
    w.u8(o.side)?;
    w.u64(o.quantity)?;
    w.u64(o.limit)?;
    w.u64(o.minimum_fill)?;
    w.u8(o.flags)?;
    w.u64(o.generation)
}
fn decode_order(r: &mut Reader<'_>) -> Result<OrderRecord> {
    Ok(OrderRecord {
        owner: r.hash()?,
        order_id: r.hash()?,
        outcome: r.u8()?,
        side: r.u8()?,
        quantity: r.u64()?,
        limit: r.u64()?,
        minimum_fill: r.u64()?,
        flags: r.u8()?,
        generation: r.u64()?,
    })
}
fn encode_portfolio(w: &mut Writer<'_>, p: PortfolioRecord) -> Result<()> {
    w.hash(p.owner)?;
    w.hash(p.order_id)?;
    w.u8(p.side)?;
    w.u8(p.active_len)?;
    w.u8(p.flags)?;
    w.amounts(&p.coefficients)?;
    w.u64(p.lots)?;
    w.u64(p.limit_collateral_per_lot)?;
    w.u64(p.minimum_fill_lots)?;
    w.u64(p.generation)
}
fn decode_portfolio(r: &mut Reader<'_>) -> Result<PortfolioRecord> {
    Ok(PortfolioRecord {
        owner: r.hash()?,
        order_id: r.hash()?,
        side: r.u8()?,
        active_len: r.u8()?,
        flags: r.u8()?,
        coefficients: r.amounts()?,
        lots: r.u64()?,
        limit_collateral_per_lot: r.u64()?,
        minimum_fill_lots: r.u64()?,
        generation: r.u64()?,
    })
}
/// Write exactly [`ORDER_SLOT_BYTES`]: a kind byte, that kind's body, and zero
/// padding out to the common width.  The padding is written explicitly rather
/// than assumed, because the destination buffer is caller-owned and may be
/// dirty.
fn encode_slot(w: &mut Writer<'_>, slot: OrderSlot) -> Result<()> {
    let start = w.at;
    w.u8(slot.kind())?;
    match slot {
        OrderSlot::Empty => {}
        OrderSlot::Single(o) => encode_order(w, o)?,
        OrderSlot::Portfolio(p) => encode_portfolio(w, p)?,
    }
    while w.at - start < ORDER_SLOT_BYTES {
        w.u8(0)?;
    }
    Ok(())
}
/// Read exactly [`ORDER_SLOT_BYTES`].  An unrecognized kind is
/// [`CodecError::WrongTag`]; any nonzero byte between the body and the common
/// width is [`CodecError::NonCanonicalPadding`], so a slot has exactly one
/// encoding.
fn decode_slot(r: &mut Reader<'_>) -> Result<OrderSlot> {
    let start = r.at;
    let slot = match r.u8()? {
        ORDER_KIND_EMPTY => OrderSlot::Empty,
        ORDER_KIND_SINGLE => OrderSlot::Single(decode_order(r)?),
        ORDER_KIND_PORTFOLIO => OrderSlot::Portfolio(decode_portfolio(r)?),
        _ => return Err(CodecError::WrongTag),
    };
    while r.at - start < ORDER_SLOT_BYTES {
        if r.u8()? != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
    }
    Ok(slot)
}

/* ---------------------------------------------------------------------------
 * Persisted protocol state.
 *
 * The accounts below exist so the kernel/protocol state an adapter needs can be
 * reconstructed from authenticated bytes instead of from a scan over positions.
 * Each one owns exactly one fact family (ARCHITECTURE.md section 3): supply
 * decomposition, immutable terms, the frozen tick domain, the epoch book
 * domain, one candidate, the settlement pot, one receipt, and the resolution.
 * They are an offline codec prototype, not a frozen deployment ABI.
 * ------------------------------------------------------------------------- */

const TERMS_BODY_BYTES: usize = account_len::TERMS - 2 - HASH_BYTES - 1 - 1;
const PRICE_GRID_BODY_BYTES: usize = account_len::PRICE_GRID - 2 - HASH_BYTES - 1 - 1;
const CANDIDATE_BODY_BYTES: usize = (2 * HASH_BYTES) + 1 + 1 + (MAX_OUTCOMES * 8) + 8 + 8 + 8;

/// Epoch phase: the book accepts placements and cancellations.
pub const EPOCH_PHASE_OPEN: u8 = 0;
/// Epoch phase: the page set is frozen; no placement or cancellation remains.
pub const EPOCH_PHASE_FROZEN: u8 = 1;
/// Epoch phase: one candidate has been selected and its slices frozen.
pub const EPOCH_PHASE_CLEARED: u8 = 2;
/// Epoch phase: every slice has settled and the pot is empty.
pub const EPOCH_PHASE_SETTLED: u8 = 3;
/// Epoch phase: the window closed with no valid candidate; reservations refund.
pub const EPOCH_PHASE_LAPSED: u8 = 4;

/// Candidate status: submitted, not yet recomputed.
pub const CANDIDATE_STATUS_SUBMITTED: u8 = 0;
/// Candidate status: every claimed aggregate was recomputed and matched.
pub const CANDIDATE_STATUS_VERIFIED: u8 = 1;
/// Candidate status: the best valid submitted candidate of its window.
pub const CANDIDATE_STATUS_SELECTED: u8 = 2;
/// Candidate status: refused by the relation.
pub const CANDIDATE_STATUS_REFUSED: u8 = 3;
/// Candidate status: superseded by a better valid submitted candidate.
pub const CANDIDATE_STATUS_SUPERSEDED: u8 = 4;

/// Pot phase: the pot is being funded from collected buyer consideration.
pub const POT_PHASE_FUNDING: u8 = 0;
/// Pot phase: the pot is open and settlement draws on it.
pub const POT_PHASE_OPEN: u8 = 1;
/// Pot phase: the pot is closed and every balance is zero.
pub const POT_PHASE_CLOSED: u8 = 2;

/// Receipt leg kind: a direct pair between two distinct real owners.
pub const RECEIPT_LEG_DIRECT: u8 = 0;
/// Receipt leg kind: a buy leg served from the virtual split.
pub const RECEIPT_LEG_SPLIT: u8 = 1;
/// Receipt leg kind: a sell leg absorbed by the virtual merge.
pub const RECEIPT_LEG_MERGE: u8 = 2;

/// Receipt flag: the buy leg's cumulative fill ceiling has been charged.
pub const RECEIPT_FLAG_BUY_CONSUMED: u8 = 1;
/// Receipt flag: the sell leg's cumulative fill ceiling has been charged.
pub const RECEIPT_FLAG_SELL_CONSUMED: u8 = 2;
/// Receipt flag: the named slice is exhausted.
pub const RECEIPT_FLAG_SLICE_EXHAUSTED: u8 = 4;

/// Market-wide supply, decomposed into its two accounted terms.
///
/// The reference adapter's closure invariant (CLO-DELTA-V1) is
/// `position internal + accounted external == aggregate supply`.  Summing
/// positions is not an onchain option, so the aggregate is persisted here as
/// the two terms whose sum it is: claims still credited internally, and claims
/// materialized outside the internal ledger and accounted for by the adapter.
/// This account is not authority over any single position's balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupplyLedgerAccount {
    /// Market identity this ledger is bound to.
    pub market: MarketId,
    /// Realm namespace identity.
    pub realm: RealmHash,
    /// Generation, separating a closed/reopened accounting era from its replays.
    pub generation: u64,
    /// Active outcome count, in `2..=MAX_OUTCOMES`.
    pub outcome_count: u8,
    /// Claims credited inside the internal ledger, per outcome.
    pub internal_supply: [u64; MAX_OUTCOMES],
    /// Claims materialized externally and accounted, per outcome.
    pub external_supply: [u64; MAX_OUTCOMES],
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
}

impl SupplyLedgerAccount {
    /// Total supply of one outcome, refusing a representation overflow.
    pub fn aggregate_supply(&self, outcome: u8) -> Result<u64> {
        let index = outcome as usize;
        if index >= self.outcome_count as usize {
            return Err(CodecError::InvalidCount);
        }
        self.internal_supply[index]
            .checked_add(self.external_supply[index])
            .ok_or(CodecError::ArithmeticOverflow)
    }
    /// Validate identities, bounds, padding, and representability of every sum.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.market)?;
        check_hash(self.realm)?;
        check_count(self.outcome_count)?;
        if self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        let active = self.outcome_count as usize;
        check_padded_amounts(&self.internal_supply, active)?;
        check_padded_amounts(&self.external_supply, active)?;
        let mut i = 0;
        while i < active {
            self.aggregate_supply(i as u8)?;
            i += 1;
        }
        Ok(())
    }
    /// Check that this ledger belongs to the supplied market bytes.
    pub fn binds_market(&self, market: &MarketAccount) -> Result<()> {
        self.validate()?;
        market.validate()?;
        if self.market != market.market
            || self.realm != market.realm
            || self.outcome_count != market.outcome_count
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }
    /// Check one position's balances against this ledger's internal term.
    ///
    /// This is a necessary condition only: a single position can never exceed
    /// the market-wide internal aggregate.  It is not the multi-position
    /// closure equality, which no single account can decide.
    pub fn check_position_bound(&self, position: &PositionAccount) -> Result<()> {
        self.validate()?;
        position.validate()?;
        if self.market != position.market {
            return Err(CodecError::MismatchedBinding);
        }
        let mut i = 0;
        while i < self.outcome_count as usize {
            if position.internal[i] > self.internal_supply[i] {
                return Err(CodecError::AggregateClosureMismatch);
            }
            i += 1;
        }
        check_padded_amounts(&position.internal, self.outcome_count as usize)?;
        Ok(())
    }
    /// Encode exactly [`account_len::SUPPLY_LEDGER`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::SUPPLY_LEDGER {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, SUPPLY_LEDGER_TAG, account_version::SUPPLY_LEDGER)?;
        w.hash(self.market)?;
        w.hash(self.realm)?;
        w.u64(self.generation)?;
        w.u8(self.outcome_count)?;
        w.amounts(&self.internal_supply)?;
        w.amounts(&self.external_supply)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::SUPPLY_LEDGER`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(
            input,
            SUPPLY_LEDGER_TAG,
            account_version::SUPPLY_LEDGER,
            account_len::SUPPLY_LEDGER,
        )?;
        let v = Self {
            market: r.hash()?,
            realm: r.hash()?,
            generation: r.u64()?,
            outcome_count: r.u8()?,
            internal_supply: r.amounts()?,
            external_supply: r.amounts()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
}

/// One payout vector: exact integer weights over a common denominator.
///
/// Mirrors `clutch_kernel::PayoutVector`; the kernel owns the redemption
/// semantics and this crate owns only the bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayoutVectorBytes {
    /// Common denominator; zero only in an inactive padding slot.
    pub denominator: u64,
    /// Weights, which must sum to the denominator over the active outcomes.
    pub weights: [u64; MAX_OUTCOMES],
}

impl PayoutVectorBytes {
    /// The all-zero padding vector.
    pub const ZERO: Self = Self {
        denominator: 0,
        weights: [0; MAX_OUTCOMES],
    };
    /// Validate one active vector against the set's common denominator.
    pub fn validate_active(&self, outcome_count: u8, denominator: u64) -> Result<()> {
        if self.denominator == 0 {
            return Err(CodecError::ZeroValue);
        }
        if self.denominator != denominator {
            return Err(CodecError::MismatchedBinding);
        }
        let active = outcome_count as usize;
        check_padded_amounts(&self.weights, active)?;
        let mut sum: u64 = 0;
        let mut i = 0;
        while i < active {
            if self.weights[i] > denominator {
                return Err(CodecError::InvalidCount);
            }
            sum = sum
                .checked_add(self.weights[i])
                .ok_or(CodecError::ArithmeticOverflow)?;
            i += 1;
        }
        if sum != denominator {
            return Err(CodecError::InvalidCount);
        }
        Ok(())
    }
}

/// Immutable market terms: the payout set, the window policy, and the digest.
///
/// [`MarketAccount::terms`] stores [`TermsAccount::terms`], and that value is
/// the domain-separated digest of every other field below.  A market therefore
/// cannot be pointed at a different payout set, feed, grid, expected range,
/// coverage/repair policy, maturity horizon, or failure policy without changing
/// the digest the market already committed to.  That binding is what closes the
/// "payouts are not cryptographically bound to terms" gap at the byte level.
///
/// The account-local `stored_bump` and `flags` are outside the digest: they are
/// address-derivation artifacts, and a PDA derived from the digest cannot also
/// be an input to it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TermsAccount {
    /// The terms digest; equals the canonical digest of every field below.
    pub terms: Hash32,
    /// Realm namespace these terms were authored under.
    pub realm: RealmHash,
    /// Immutable collateral/profile identity.
    pub profile: ProfileHash,
    /// Feed identity the window is evaluated against.
    pub feed: FeedId,
    /// Identity of the frozen price grid, binding the order limit tick domain.
    pub price_grid: Hash32,
    /// Active outcome count, in `2..=MAX_OUTCOMES`.
    pub outcome_count: u8,
    /// Active payout vectors, in `1..=MAX_PAYOUTS`.
    pub payout_count: u8,
    /// The payout-vector set; inactive slots are all zero.
    pub payouts: [PayoutVectorBytes; MAX_PAYOUTS],
    /// Observation grid family identity.
    pub grid_family_id: u32,
    /// Observation grid version.
    pub grid_version: u16,
    /// Observation bucket duration in seconds, in `1..=MAX_BUCKET_SECONDS`.
    pub bucket_seconds: u64,
    /// First bucket of the exact expected range.
    pub expected_start_bucket: u64,
    /// Exclusive end bucket of the exact expected range.
    pub expected_end_bucket_exclusive: u64,
    /// Buckets that must be offered before resolution may be attempted.
    pub maturity_horizon_buckets: u64,
    /// Registered coverage-policy identity; zero is refused.
    pub coverage_policy_id: u32,
    /// Registered repair-policy identity; zero is refused.
    pub repair_policy_id: u32,
    /// Registered failure-policy identity; zero is refused.
    pub failure_policy_id: u32,
    /// Stored PDA bump; outside the digest.
    pub stored_bump: u8,
    /// Reserved flags; currently zero, and outside the digest.
    pub flags: u8,
}

impl TermsAccount {
    fn body(&self, out: &mut [u8; TERMS_BODY_BYTES]) -> Result<()> {
        let mut w = Writer::new(out);
        w.hash(self.realm)?;
        w.hash(self.profile)?;
        w.hash(self.feed)?;
        w.hash(self.price_grid)?;
        w.u8(self.outcome_count)?;
        w.u8(self.payout_count)?;
        let mut i = 0;
        while i < MAX_PAYOUTS {
            w.u64(self.payouts[i].denominator)?;
            w.amounts(&self.payouts[i].weights)?;
            i += 1;
        }
        w.u32(self.grid_family_id)?;
        w.u16(self.grid_version)?;
        w.u64(self.bucket_seconds)?;
        w.u64(self.expected_start_bucket)?;
        w.u64(self.expected_end_bucket_exclusive)?;
        w.u64(self.maturity_horizon_buckets)?;
        w.u32(self.coverage_policy_id)?;
        w.u32(self.repair_policy_id)?;
        w.u32(self.failure_policy_id)?;
        if w.at != TERMS_BODY_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(())
    }
    /// Recompute the terms digest from the current field values.
    pub fn recomputed_terms_digest(&self) -> Result<Hash32> {
        let mut body = [0; TERMS_BODY_BYTES];
        self.body(&mut body)?;
        Ok(canonical_terms_digest(&body))
    }
    /// Number of buckets in the exact expected range.
    pub fn expected_span(&self) -> Result<u64> {
        self.expected_end_bucket_exclusive
            .checked_sub(self.expected_start_bucket)
            .ok_or(CodecError::InvalidCount)
    }
    /// Validate the payout set, the window policy, and the self-certifying digest.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.realm)?;
        check_hash(self.profile)?;
        check_hash(self.feed)?;
        check_hash(self.price_grid)?;
        check_count(self.outcome_count)?;
        if self.payout_count == 0 || self.payout_count as usize > MAX_PAYOUTS {
            return Err(CodecError::InvalidCount);
        }
        if self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        let denominator = self.payouts[0].denominator;
        let mut i = 0;
        while i < MAX_PAYOUTS {
            if i < self.payout_count as usize {
                self.payouts[i].validate_active(self.outcome_count, denominator)?;
            } else if self.payouts[i] != PayoutVectorBytes::ZERO {
                return Err(CodecError::NonCanonicalPadding);
            }
            i += 1;
        }
        if self.grid_family_id == 0 || self.grid_version == 0 {
            return Err(CodecError::ZeroValue);
        }
        if self.bucket_seconds == 0 || self.bucket_seconds > MAX_BUCKET_SECONDS {
            return Err(CodecError::InvalidCount);
        }
        let span = self.expected_span()?;
        if span == 0 || span > MAX_WINDOW_BUCKETS {
            return Err(CodecError::InvalidCount);
        }
        // A maturity horizon shorter than the expected range would let a
        // prefix resolve; it is refused rather than clamped.
        if self.maturity_horizon_buckets < span
            || self.maturity_horizon_buckets > MAX_WINDOW_BUCKETS
        {
            return Err(CodecError::InvalidCount);
        }
        if self.coverage_policy_id == 0 || self.repair_policy_id == 0 || self.failure_policy_id == 0
        {
            return Err(CodecError::ZeroValue);
        }
        if self.terms != self.recomputed_terms_digest()? {
            return Err(CodecError::NonCanonicalIdentity);
        }
        Ok(())
    }
    /// Check that a market's committed terms digest is exactly these terms.
    pub fn binds_market(&self, market: &MarketAccount) -> Result<()> {
        self.validate()?;
        market.validate()?;
        if market.terms != self.terms
            || market.realm != self.realm
            || market.profile != self.profile
            || market.feed != self.feed
            || market.outcome_count != self.outcome_count
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }
    /// Encode exactly [`account_len::TERMS`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::TERMS {
            return Err(CodecError::OutputTooSmall);
        }
        let mut body = [0; TERMS_BODY_BYTES];
        self.body(&mut body)?;
        let mut w = Writer::new(out);
        put_header(&mut w, TERMS_TAG, account_version::TERMS)?;
        w.hash(self.terms)?;
        w.bytes(&body)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::TERMS`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(input, TERMS_TAG, account_version::TERMS, account_len::TERMS)?;
        let terms = r.hash()?;
        let realm = r.hash()?;
        let profile = r.hash()?;
        let feed = r.hash()?;
        let price_grid = r.hash()?;
        let outcome_count = r.u8()?;
        let payout_count = r.u8()?;
        let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
        let mut i = 0;
        while i < MAX_PAYOUTS {
            payouts[i] = PayoutVectorBytes {
                denominator: r.u64()?,
                weights: r.amounts()?,
            };
            i += 1;
        }
        let v = Self {
            terms,
            realm,
            profile,
            feed,
            price_grid,
            outcome_count,
            payout_count,
            payouts,
            grid_family_id: r.u32()?,
            grid_version: r.u16()?,
            bucket_seconds: r.u64()?,
            expected_start_bucket: r.u64()?,
            expected_end_bucket_exclusive: r.u64()?,
            maturity_horizon_buckets: r.u64()?,
            coverage_policy_id: r.u32()?,
            repair_policy_id: r.u32()?,
            failure_policy_id: r.u32()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
}

/// The frozen price grid: the only admitted mapping from an order limit to a tick.
///
/// [`OrderRecord::limit`] is an opaque `u64` on the venue scale.  The relation
/// consumes a tick index, so the mapping must be frozen somewhere; it is frozen
/// here, as an exact membership test in a strictly increasing tick vector.  A
/// limit that is not exactly one of the ticks has no tick and is refused, which
/// is why [`OrderPageAccount::decode_on_grid`] exists beside the plain decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceGridAccount {
    /// Grid identity; equals the canonical digest of the body below.
    pub grid: Hash32,
    /// Realm namespace identity.
    pub realm: RealmHash,
    /// Exact integer price scale; a complete set values at exactly this much.
    pub price_scale: u64,
    /// Active ticks, in `2..=MAX_GRID_TICKS`.
    pub tick_count: u8,
    /// Strictly increasing ticks; inactive slots are zero.
    pub ticks: [u64; MAX_GRID_TICKS],
    /// Stored PDA bump; outside the digest.
    pub stored_bump: u8,
    /// Reserved flags; currently zero, and outside the digest.
    pub flags: u8,
}

impl PriceGridAccount {
    fn body(&self, out: &mut [u8; PRICE_GRID_BODY_BYTES]) -> Result<()> {
        let mut w = Writer::new(out);
        w.hash(self.realm)?;
        w.u64(self.price_scale)?;
        w.u8(self.tick_count)?;
        let mut i = 0;
        while i < MAX_GRID_TICKS {
            w.u64(self.ticks[i])?;
            i += 1;
        }
        if w.at != PRICE_GRID_BODY_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(())
    }
    /// Recompute the grid identity from the current field values.
    pub fn recomputed_grid_id(&self) -> Result<Hash32> {
        let mut body = [0; PRICE_GRID_BODY_BYTES];
        self.body(&mut body)?;
        Ok(canonical_price_grid_id(&body))
    }
    /// The tick index of an exact grid member, or [`CodecError::InvalidTick`].
    pub fn tick_of(&self, limit: u64) -> Result<u8> {
        let mut i = 0;
        while i < self.tick_count as usize {
            if self.ticks[i] == limit {
                return Ok(i as u8);
            }
            i += 1;
        }
        Err(CodecError::InvalidTick)
    }
    /// The exact limit value of a tick index.
    pub fn tick_value(&self, tick: u8) -> Result<u64> {
        if tick as usize >= self.tick_count as usize {
            return Err(CodecError::InvalidTick);
        }
        Ok(self.ticks[tick as usize])
    }
    /// Validate scale, ordering, bounds, padding, and the self-certifying identity.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.realm)?;
        if self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        if self.price_scale == 0 || self.price_scale > MAX_PRICE_SCALE {
            return Err(CodecError::InvalidPriceGrid);
        }
        if self.tick_count < 2 || self.tick_count as usize > MAX_GRID_TICKS {
            return Err(CodecError::InvalidPriceGrid);
        }
        let mut i = 0;
        while i < MAX_GRID_TICKS {
            if i < self.tick_count as usize {
                if self.ticks[i] > self.price_scale {
                    return Err(CodecError::InvalidPriceGrid);
                }
                if i > 0 && self.ticks[i] <= self.ticks[i - 1] {
                    return Err(CodecError::InvalidPriceGrid);
                }
            } else if self.ticks[i] != 0 {
                return Err(CodecError::NonCanonicalPadding);
            }
            i += 1;
        }
        if self.grid != self.recomputed_grid_id()? {
            return Err(CodecError::NonCanonicalIdentity);
        }
        Ok(())
    }
    /// Check that immutable terms name exactly this grid.
    pub fn binds_terms(&self, terms: &TermsAccount) -> Result<()> {
        self.validate()?;
        terms.validate()?;
        if terms.price_grid != self.grid || terms.realm != self.realm {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }
    /// Encode exactly [`account_len::PRICE_GRID`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::PRICE_GRID {
            return Err(CodecError::OutputTooSmall);
        }
        let mut body = [0; PRICE_GRID_BODY_BYTES];
        self.body(&mut body)?;
        let mut w = Writer::new(out);
        put_header(&mut w, PRICE_GRID_TAG, account_version::PRICE_GRID)?;
        w.hash(self.grid)?;
        w.bytes(&body)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::PRICE_GRID`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(
            input,
            PRICE_GRID_TAG,
            account_version::PRICE_GRID,
            account_len::PRICE_GRID,
        )?;
        let grid = r.hash()?;
        let realm = r.hash()?;
        let price_scale = r.u64()?;
        let tick_count = r.u8()?;
        let mut ticks = [0; MAX_GRID_TICKS];
        let mut i = 0;
        while i < MAX_GRID_TICKS {
            ticks[i] = r.u64()?;
            i += 1;
        }
        let v = Self {
            grid,
            realm,
            price_scale,
            tick_count,
            ticks,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
}

/// The persisted projection of the relation's frozen book domain.
///
/// Field for field this is `clutch_batch::relation_v1::RelationDomainV1` with
/// its host-model `u64` tags replaced by 32-byte identities: market, book,
/// epoch, policy, and order set, plus the shape and seed the relation reads.
/// The policy identity is stored as an opaque digest; this crate never names or
/// selects a policy variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochAccount {
    /// Canonical epoch identity, derived from market and epoch index.
    pub epoch: EpochId,
    /// Market identity.
    pub market: MarketId,
    /// Book identity within the market.
    pub book: Hash32,
    /// Immutable terms digest this epoch clears under.
    pub terms: Hash32,
    /// Frozen price-grid identity this epoch's limits live on.
    pub price_grid: Hash32,
    /// Opaque frozen-policy identity.
    pub policy: Hash32,
    /// Set-wide order-set digest; zero until the page set is frozen.
    pub order_set: Hash32,
    /// Lowest order id in the frozen set; zero until frozen.
    pub first_order_id: Hash32,
    /// Highest order id in the frozen set; zero until frozen.
    pub last_order_id: Hash32,
    /// Epoch index within the market.
    pub epoch_index: u64,
    /// Relation version; must equal [`RELATION_VERSION`].
    pub relation_version: u32,
    /// Exact integer price scale.
    pub price_scale: u64,
    /// Seed of the frozen largest-remainder permutation.
    pub remainder_seed: u64,
    /// Distinct bound owners admitted in this book.
    pub owner_count: u16,
    /// Frozen page count; zero until frozen.
    pub page_count: u16,
    /// Frozen order count; zero until frozen.
    pub order_count: u16,
    /// Active outcome count, in `2..=MAX_OUTCOMES`.
    pub outcome_count: u8,
    /// Lifecycle phase; see the `EPOCH_PHASE_*` constants.
    pub phase: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
}

impl EpochAccount {
    /// Validate identities, relation shape, and freeze-state consistency.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.market)?;
        check_hash(self.book)?;
        check_hash(self.terms)?;
        check_hash(self.price_grid)?;
        check_hash(self.policy)?;
        if self.epoch != canonical_epoch_id(self.market, self.epoch_index) {
            return Err(CodecError::NonCanonicalIdentity);
        }
        if self.relation_version != RELATION_VERSION {
            return Err(CodecError::WrongVersion);
        }
        check_count(self.outcome_count)?;
        if self.phase > EPOCH_PHASE_LAPSED || self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        if self.owner_count == 0 {
            return Err(CodecError::InvalidCount);
        }
        if self.price_scale == 0 || self.price_scale > MAX_PRICE_SCALE {
            return Err(CodecError::InvalidPriceGrid);
        }
        if self.phase == EPOCH_PHASE_OPEN {
            // Nothing is committed while placements can still arrive.
            if self.order_set != Hash32::ZERO
                || self.first_order_id != Hash32::ZERO
                || self.last_order_id != Hash32::ZERO
                || self.page_count != 0
                || self.order_count != 0
            {
                return Err(CodecError::NonCanonicalPadding);
            }
            return Ok(());
        }
        check_hash(self.order_set)?;
        check_hash(self.first_order_id)?;
        check_hash(self.last_order_id)?;
        if self.page_count == 0
            || self.page_count as usize > MAX_ORDER_PAGES
            || self.order_count == 0
            || self.order_count as usize > MAX_EPOCH_ORDERS
        {
            return Err(CodecError::InvalidCount);
        }
        let low = (self.page_count as usize - 1) * MAX_ORDERS_PER_PAGE;
        if self.order_count as usize <= low || self.order_count as usize > low + MAX_ORDERS_PER_PAGE
        {
            return Err(CodecError::MismatchedBinding);
        }
        if self.order_count == 1 {
            if self.first_order_id != self.last_order_id {
                return Err(CodecError::MismatchedBinding);
            }
        } else if self.first_order_id.0 >= self.last_order_id.0 {
            return Err(CodecError::NonCanonicalIdentity);
        }
        Ok(())
    }
    /// Check this epoch against a complete, in-order, frozen page set.
    pub fn binds_page_set(&self, pages: &[OrderPageAccount]) -> Result<()> {
        self.validate()?;
        if self.phase == EPOCH_PHASE_OPEN {
            return Err(CodecError::MismatchedBinding);
        }
        let order_set = verify_page_set(pages)?;
        let head = &pages[0];
        let tail = &pages[pages.len() - 1];
        if order_set != self.order_set
            || head.market != self.market
            || head.epoch != self.epoch
            || head.page_count != self.page_count
            || head.set_order_count != self.order_count
            || head.first_order_id != self.first_order_id
            || tail.last_order_id != self.last_order_id
        {
            return Err(CodecError::MismatchedBinding);
        }
        // A page alone can only bound an order's outcome width by
        // [`MAX_OUTCOMES`]; the epoch is the account that names this market's
        // actual width, so a record claiming an outcome or an active
        // coefficient width the market does not have contradicts a binding the
        // epoch already committed to.
        let mut i = 0;
        while i < pages.len() {
            let mut j = 0;
            while j < pages[i].order_count as usize {
                match pages[i].orders[j] {
                    OrderSlot::Single(o) => {
                        if o.outcome >= self.outcome_count {
                            return Err(CodecError::MismatchedBinding);
                        }
                    }
                    OrderSlot::Portfolio(p) => {
                        if p.active_len > self.outcome_count {
                            return Err(CodecError::MismatchedBinding);
                        }
                    }
                    OrderSlot::Empty => return Err(CodecError::ZeroIdentity),
                }
                j += 1;
            }
            i += 1;
        }
        Ok(())
    }
    /// Check this epoch against the immutable terms and grid it names.
    pub fn binds_terms(&self, terms: &TermsAccount, grid: &PriceGridAccount) -> Result<()> {
        self.validate()?;
        grid.binds_terms(terms)?;
        if self.terms != terms.terms
            || self.price_grid != terms.price_grid
            || self.outcome_count != terms.outcome_count
            || self.price_scale != grid.price_scale
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }
    /// Encode exactly [`account_len::EPOCH`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::EPOCH {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, EPOCH_TAG, account_version::EPOCH)?;
        w.hash(self.epoch)?;
        w.hash(self.market)?;
        w.hash(self.book)?;
        w.hash(self.terms)?;
        w.hash(self.price_grid)?;
        w.hash(self.policy)?;
        w.hash(self.order_set)?;
        w.hash(self.first_order_id)?;
        w.hash(self.last_order_id)?;
        w.u64(self.epoch_index)?;
        w.u32(self.relation_version)?;
        w.u64(self.price_scale)?;
        w.u64(self.remainder_seed)?;
        w.u16(self.owner_count)?;
        w.u16(self.page_count)?;
        w.u16(self.order_count)?;
        w.u8(self.outcome_count)?;
        w.u8(self.phase)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::EPOCH`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(input, EPOCH_TAG, account_version::EPOCH, account_len::EPOCH)?;
        let v = Self {
            epoch: r.hash()?,
            market: r.hash()?,
            book: r.hash()?,
            terms: r.hash()?,
            price_grid: r.hash()?,
            policy: r.hash()?,
            order_set: r.hash()?,
            first_order_id: r.hash()?,
            last_order_id: r.hash()?,
            epoch_index: r.u64()?,
            relation_version: r.u32()?,
            price_scale: r.u64()?,
            remainder_seed: r.u64()?,
            owner_count: r.u16()?,
            page_count: r.u16()?,
            order_count: r.u16()?,
            outcome_count: r.u8()?,
            phase: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
}

/// One submitted candidate: its free coordinates, identity, score, and status.
///
/// Only the price vector, the virtual split/merge pair, and the honored
/// all-or-none mask are free; fills are derived canonically from those plus the
/// frozen book, so they are deliberately **not** persisted here.  The stored
/// score is a claim; the relation recomputes it, and this codec only refuses
/// shapes that no recomputation could accept.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateRecord {
    /// Candidate identity; equals the canonical digest of its free coordinates.
    pub candidate: Hash32,
    /// Epoch identity.
    pub epoch: EpochId,
    /// Market identity.
    pub market: MarketId,
    /// Exact scaled prices on the simplex; inactive outcomes are zero.
    pub prices: [u64; MAX_OUTCOMES],
    /// `sigma`: complete sets created by the single global virtual split.
    pub virtual_split: u64,
    /// `mu`: complete sets destroyed by the single global virtual merge.
    pub virtual_merge: u64,
    /// Honored minimum-fill subset, one bit per order.
    pub honored_aon_mask: u64,
    /// Score component 1, net of the self-overlap term; may be negative.
    pub weighted_direct_volume: i128,
    /// Score component 3, in exact price units.
    pub limit_surplus_price_units: u128,
    /// Score component 5: `sigma + mu`.
    pub churn: u64,
    /// Slot the candidate was submitted in, as supplied by the adapter.
    pub submitted_slot: u64,
    /// Score component 4: distinct participating owners.
    pub distinct_owners: u16,
    /// Orders this candidate binds; must equal the frozen book length.
    pub order_len: u8,
    /// Active outcome count, in `2..=MAX_OUTCOMES`.
    pub outcome_count: u8,
    /// Status; see the `CANDIDATE_STATUS_*` constants.
    pub status: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
}

impl CandidateRecord {
    fn body(&self, out: &mut [u8; CANDIDATE_BODY_BYTES]) -> Result<()> {
        let mut w = Writer::new(out);
        w.hash(self.epoch)?;
        w.hash(self.market)?;
        w.u8(self.order_len)?;
        w.u8(self.outcome_count)?;
        w.amounts(&self.prices)?;
        w.u64(self.virtual_split)?;
        w.u64(self.virtual_merge)?;
        w.u64(self.honored_aon_mask)?;
        if w.at != CANDIDATE_BODY_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(())
    }
    /// Recompute the candidate identity from its free coordinates and domain.
    pub fn recomputed_candidate_digest(&self) -> Result<Hash32> {
        let mut body = [0; CANDIDATE_BODY_BYTES];
        self.body(&mut body)?;
        Ok(canonical_candidate_digest(&body))
    }
    /// Validate coordinates, mask width, canonical churn, status, and identity.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.epoch)?;
        check_hash(self.market)?;
        check_count(self.outcome_count)?;
        if self.order_len == 0 || self.order_len as usize > MAX_EPOCH_ORDERS {
            return Err(CodecError::InvalidCount);
        }
        if self.status > CANDIDATE_STATUS_SUPERSEDED || self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        check_padded_amounts(&self.prices, self.outcome_count as usize)?;
        // A mask bit above the book length is a claim about an order that does
        // not exist; it is a leak, not padding to be ignored.
        if self.order_len < 64 && self.honored_aon_mask >> self.order_len != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        // Canonical churn: a candidate never splits and merges at once.
        if self.virtual_split != 0 && self.virtual_merge != 0 {
            return Err(CodecError::InvalidEnum);
        }
        let churn = self
            .virtual_split
            .checked_add(self.virtual_merge)
            .ok_or(CodecError::ArithmeticOverflow)?;
        if self.churn != churn {
            return Err(CodecError::MismatchedBinding);
        }
        if self.distinct_owners as usize > MAX_EPOCH_ORDERS {
            return Err(CodecError::InvalidCount);
        }
        if self.candidate != self.recomputed_candidate_digest()? {
            return Err(CodecError::NonCanonicalIdentity);
        }
        Ok(())
    }
    /// Check this candidate against the frozen epoch domain it clears.
    pub fn binds_epoch(&self, epoch: &EpochAccount) -> Result<()> {
        self.validate()?;
        epoch.validate()?;
        if epoch.phase == EPOCH_PHASE_OPEN {
            return Err(CodecError::MismatchedBinding);
        }
        if self.epoch != epoch.epoch
            || self.market != epoch.market
            || self.outcome_count != epoch.outcome_count
            || self.order_len as u16 != epoch.order_count
        {
            return Err(CodecError::MismatchedBinding);
        }
        // The price vector lives on the scaled simplex of the frozen domain.
        let mut sum: u64 = 0;
        let mut i = 0;
        while i < self.outcome_count as usize {
            if self.prices[i] > epoch.price_scale {
                return Err(CodecError::InvalidPriceGrid);
            }
            sum = sum
                .checked_add(self.prices[i])
                .ok_or(CodecError::ArithmeticOverflow)?;
            i += 1;
        }
        if sum != epoch.price_scale {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }
    /// Encode exactly [`account_len::CANDIDATE`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::CANDIDATE {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, CANDIDATE_TAG, account_version::CANDIDATE)?;
        w.hash(self.candidate)?;
        w.hash(self.epoch)?;
        w.hash(self.market)?;
        w.amounts(&self.prices)?;
        w.u64(self.virtual_split)?;
        w.u64(self.virtual_merge)?;
        w.u64(self.honored_aon_mask)?;
        w.i128(self.weighted_direct_volume)?;
        w.u128(self.limit_surplus_price_units)?;
        w.u64(self.churn)?;
        w.u64(self.submitted_slot)?;
        w.u16(self.distinct_owners)?;
        w.u8(self.order_len)?;
        w.u8(self.outcome_count)?;
        w.u8(self.status)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::CANDIDATE`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(
            input,
            CANDIDATE_TAG,
            account_version::CANDIDATE,
            account_len::CANDIDATE,
        )?;
        let v = Self {
            candidate: r.hash()?,
            epoch: r.hash()?,
            market: r.hash()?,
            prices: r.amounts()?,
            virtual_split: r.u64()?,
            virtual_merge: r.u64()?,
            honored_aon_mask: r.u64()?,
            weighted_direct_volume: r.i128()?,
            limit_surplus_price_units: r.u128()?,
            churn: r.u64()?,
            submitted_slot: r.u64()?,
            distinct_owners: r.u16()?,
            order_len: r.u8()?,
            outcome_count: r.u8()?,
            status: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
}

/// The settlement pot of one selected candidate.
///
/// A byte cannot be both an order reservation and a settlement pot
/// (ARCHITECTURE.md section 3), so this account holds **only** pot-phase
/// balances: the claims the virtual split produced, the cash collected from
/// buyer consideration, and the named rounding remainder.  It carries no
/// reservation field, and Hoard principal never appears in it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalPotAccount {
    /// Epoch identity.
    pub epoch: EpochId,
    /// Market identity.
    pub market: MarketId,
    /// Selected candidate identity.
    pub candidate: Hash32,
    /// Claims held by the pot position, per outcome.
    pub pot_internal: [u64; MAX_OUTCOMES],
    /// Pot cash in exact price units.
    pub pot_cash_price_units: u128,
    /// Remainder atoms of the one named rounding boundary, in price units.
    pub rounding_pot_price_units: u128,
    /// Active outcome count, in `2..=MAX_OUTCOMES`.
    pub outcome_count: u8,
    /// Pot phase; see the `POT_PHASE_*` constants.
    pub phase: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
}

impl FinalPotAccount {
    /// Validate bounds, padding, and the terminal empty-pot condition.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.epoch)?;
        check_hash(self.market)?;
        check_hash(self.candidate)?;
        check_count(self.outcome_count)?;
        if self.phase > POT_PHASE_CLOSED || self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        check_padded_amounts(&self.pot_internal, self.outcome_count as usize)?;
        if self.phase == POT_PHASE_CLOSED {
            // Epoch-terminal condition: a closed pot is an empty pot.
            let mut i = 0;
            while i < self.outcome_count as usize {
                if self.pot_internal[i] != 0 {
                    return Err(CodecError::AggregateClosureMismatch);
                }
                i += 1;
            }
            if self.pot_cash_price_units != 0 || self.rounding_pot_price_units != 0 {
                return Err(CodecError::AggregateClosureMismatch);
            }
        }
        Ok(())
    }
    /// Check that this pot belongs to the supplied candidate.
    pub fn binds_candidate(&self, candidate: &CandidateRecord) -> Result<()> {
        self.validate()?;
        candidate.validate()?;
        if self.candidate != candidate.candidate
            || self.epoch != candidate.epoch
            || self.market != candidate.market
            || self.outcome_count != candidate.outcome_count
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }
    /// Encode exactly [`account_len::FINAL_POT`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::FINAL_POT {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, FINAL_POT_TAG, account_version::FINAL_POT)?;
        w.hash(self.epoch)?;
        w.hash(self.market)?;
        w.hash(self.candidate)?;
        w.amounts(&self.pot_internal)?;
        w.u128(self.pot_cash_price_units)?;
        w.u128(self.rounding_pot_price_units)?;
        w.u8(self.outcome_count)?;
        w.u8(self.phase)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::FINAL_POT`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(
            input,
            FINAL_POT_TAG,
            account_version::FINAL_POT,
            account_len::FINAL_POT,
        )?;
        let v = Self {
            epoch: r.hash()?,
            market: r.hash()?,
            candidate: r.hash()?,
            pot_internal: r.amounts()?,
            pot_cash_price_units: r.u128()?,
            rounding_pot_price_units: r.u128()?,
            outcome_count: r.u8()?,
            phase: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
}

/// One settlement receipt against one frozen slice of the selected candidate.
///
/// The receipt is the single sequential authority for "how much of this slice
/// has settled"; nothing reconstructs it by combining per-party or per-page
/// views.  Consideration is bound to the frozen price by exact multiplication,
/// so a receipt cannot quietly re-price a slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptAccount {
    /// Epoch identity.
    pub epoch: EpochId,
    /// Market identity.
    pub market: MarketId,
    /// Selected candidate identity.
    pub candidate: Hash32,
    /// Buy-side order id; zero exactly when the buy end is the virtual merge.
    pub buy_order_id: Hash32,
    /// Sell-side order id; zero exactly when the sell end is the virtual split.
    pub sell_order_id: Hash32,
    /// Exact consideration in price units: `quantity * price`.
    pub consideration_price_units: u128,
    /// Slice quantity in claim atoms.
    pub quantity: u64,
    /// Cumulative settled quantity, never above `quantity`.
    pub settled_quantity: u64,
    /// The outcome's scaled price, frozen at clear time.
    pub price: u64,
    /// Monotone settlement sequence for this candidate.
    pub sequence: u64,
    /// Index of the frozen slice this receipt settles.
    pub slice_index: u16,
    /// Bound outcome of both ends.
    pub outcome: u8,
    /// Leg kind; see the `RECEIPT_LEG_*` constants.
    pub leg_kind: u8,
    /// Consumption flags; see the `RECEIPT_FLAG_*` constants.
    pub consumed_flags: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
}

impl SettlementReceiptAccount {
    /// Validate leg shape, exact consideration, and consumption consistency.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.epoch)?;
        check_hash(self.market)?;
        check_hash(self.candidate)?;
        if self.leg_kind > RECEIPT_LEG_MERGE
            || self.flags != 0
            || self.consumed_flags
                & !(RECEIPT_FLAG_BUY_CONSUMED
                    | RECEIPT_FLAG_SELL_CONSUMED
                    | RECEIPT_FLAG_SLICE_EXHAUSTED)
                != 0
        {
            return Err(CodecError::InvalidEnum);
        }
        match self.leg_kind {
            RECEIPT_LEG_DIRECT => {
                check_hash(self.buy_order_id)?;
                check_hash(self.sell_order_id)?;
                // One order can never be both ends of an executable transfer.
                if self.buy_order_id == self.sell_order_id {
                    return Err(CodecError::NonCanonicalIdentity);
                }
            }
            RECEIPT_LEG_SPLIT => {
                check_hash(self.buy_order_id)?;
                if self.sell_order_id != Hash32::ZERO {
                    return Err(CodecError::NonCanonicalPadding);
                }
            }
            _ => {
                check_hash(self.sell_order_id)?;
                if self.buy_order_id != Hash32::ZERO {
                    return Err(CodecError::NonCanonicalPadding);
                }
            }
        }
        if self.outcome as usize >= MAX_OUTCOMES {
            return Err(CodecError::InvalidCount);
        }
        if self.quantity == 0 {
            return Err(CodecError::ZeroValue);
        }
        if self.settled_quantity > self.quantity {
            return Err(CodecError::InvalidCount);
        }
        if self.slice_index as usize >= MAX_EPOCH_ORDERS * 2 {
            return Err(CodecError::InvalidCount);
        }
        // Two `u64` factors always fit a `u128` product, so exactness is the
        // whole content of this check and no overflow case exists.
        let exact = u128::from(self.quantity) * u128::from(self.price);
        if self.consideration_price_units != exact {
            return Err(CodecError::InvalidConsideration);
        }
        let exhausted = self.consumed_flags & RECEIPT_FLAG_SLICE_EXHAUSTED != 0;
        if exhausted != (self.settled_quantity == self.quantity) {
            return Err(CodecError::InvalidEnum);
        }
        Ok(())
    }
    /// Check that this receipt settles against the supplied candidate.
    pub fn binds_candidate(&self, candidate: &CandidateRecord) -> Result<()> {
        self.validate()?;
        candidate.validate()?;
        if self.candidate != candidate.candidate
            || self.epoch != candidate.epoch
            || self.market != candidate.market
        {
            return Err(CodecError::MismatchedBinding);
        }
        if self.outcome >= candidate.outcome_count {
            return Err(CodecError::InvalidCount);
        }
        if self.price != candidate.prices[self.outcome as usize] {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }
    /// Encode exactly [`account_len::SETTLEMENT_RECEIPT`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::SETTLEMENT_RECEIPT {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(
            &mut w,
            SETTLEMENT_RECEIPT_TAG,
            account_version::SETTLEMENT_RECEIPT,
        )?;
        w.hash(self.epoch)?;
        w.hash(self.market)?;
        w.hash(self.candidate)?;
        w.hash(self.buy_order_id)?;
        w.hash(self.sell_order_id)?;
        w.u128(self.consideration_price_units)?;
        w.u64(self.quantity)?;
        w.u64(self.settled_quantity)?;
        w.u64(self.price)?;
        w.u64(self.sequence)?;
        w.u16(self.slice_index)?;
        w.u8(self.outcome)?;
        w.u8(self.leg_kind)?;
        w.u8(self.consumed_flags)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::SETTLEMENT_RECEIPT`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(
            input,
            SETTLEMENT_RECEIPT_TAG,
            account_version::SETTLEMENT_RECEIPT,
            account_len::SETTLEMENT_RECEIPT,
        )?;
        let v = Self {
            epoch: r.hash()?,
            market: r.hash()?,
            candidate: r.hash()?,
            buy_order_id: r.hash()?,
            sell_order_id: r.hash()?,
            consideration_price_units: r.u128()?,
            quantity: r.u64()?,
            settled_quantity: r.u64()?,
            price: r.u64()?,
            sequence: r.u64()?,
            slice_index: r.u16()?,
            outcome: r.u8()?,
            leg_kind: r.u8()?,
            consumed_flags: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
}

/// The immutable resolution fact of one market.
///
/// It names the payout vector by index into the immutable terms set, together
/// with the sealed window evidence that selected it.  Persisting the index
/// beside the terms digest is what lets a resolved `clutch_kernel::MarketState`
/// be reconstructed; `MarketAccount::lifecycle` alone cannot do it.  This
/// account is bytes only: it is not evidence that a window was in fact sealed,
/// and an adapter must still authenticate the window result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionAccount {
    /// Market identity.
    pub market: MarketId,
    /// Immutable terms digest whose payout set this index refers to.
    pub terms: Hash32,
    /// Feed identity the sealed window came from.
    pub feed: FeedId,
    /// Digest of the sealed window result.
    pub window: Hash32,
    /// Accepted feed cursor at seal.
    pub feed_cursor: u64,
    /// Exclusive end bucket of the sealed window.
    pub sealed_end_bucket_exclusive: u64,
    /// Repair generation of the sealed window.
    pub repair_generation: u64,
    /// Slot the resolution was recorded in, as supplied by the adapter.
    pub resolved_slot: u64,
    /// Selected payout index, or [`PAYOUT_INDEX_UNRESOLVED`].
    pub payout_index: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
}

impl ResolutionAccount {
    /// True when a payout vector has been selected.
    pub const fn is_resolved(&self) -> bool {
        self.payout_index != PAYOUT_INDEX_UNRESOLVED
    }
    /// Validate identities and the unresolved/resolved field discipline.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.market)?;
        check_hash(self.terms)?;
        check_hash(self.feed)?;
        if self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        if self.is_resolved() {
            if self.payout_index as usize >= MAX_PAYOUTS {
                return Err(CodecError::InvalidCount);
            }
            check_hash(self.window)?;
            if self.sealed_end_bucket_exclusive == 0 {
                return Err(CodecError::ZeroValue);
            }
        } else if self.window != Hash32::ZERO
            || self.feed_cursor != 0
            || self.sealed_end_bucket_exclusive != 0
            || self.repair_generation != 0
            || self.resolved_slot != 0
        {
            return Err(CodecError::NonCanonicalPadding);
        }
        Ok(())
    }
    /// Check the resolution against the immutable terms it selects from.
    pub fn binds_terms(&self, terms: &TermsAccount) -> Result<()> {
        self.validate()?;
        terms.validate()?;
        if self.terms != terms.terms || self.feed != terms.feed {
            return Err(CodecError::MismatchedBinding);
        }
        if self.is_resolved() {
            if self.payout_index >= terms.payout_count {
                return Err(CodecError::InvalidCount);
            }
            // Resolution may not precede the frozen expected range's end.
            if self.sealed_end_bucket_exclusive < terms.expected_end_bucket_exclusive {
                return Err(CodecError::MismatchedBinding);
            }
        }
        Ok(())
    }
    /// Encode exactly [`account_len::RESOLUTION`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::RESOLUTION {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, RESOLUTION_TAG, account_version::RESOLUTION)?;
        w.hash(self.market)?;
        w.hash(self.terms)?;
        w.hash(self.feed)?;
        w.hash(self.window)?;
        w.u64(self.feed_cursor)?;
        w.u64(self.sealed_end_bucket_exclusive)?;
        w.u64(self.repair_generation)?;
        w.u64(self.resolved_slot)?;
        w.u8(self.payout_index)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::RESOLUTION`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(
            input,
            RESOLUTION_TAG,
            account_version::RESOLUTION,
            account_len::RESOLUTION,
        )?;
        let v = Self {
            market: r.hash()?,
            terms: r.hash()?,
            feed: r.hash()?,
            window: r.hash()?,
            feed_cursor: r.u64()?,
            sealed_end_bucket_exclusive: r.u64()?,
            repair_generation: r.u64()?,
            resolved_slot: r.u64()?,
            payout_index: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
}

/// Deterministic instruction intent, with no account metadata or signatures.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Intent {
    /// Create a market namespace binding.
    CreateMarket {
        realm: RealmHash,
        profile: ProfileHash,
        market_nonce: u64,
        outcome_count: u8,
        terms: Hash32,
        feed: FeedId,
    },
    /// Add a complete internal set to the market hoard/position seam.
    Split {
        market: MarketId,
        owner: OwnerId,
        quantity: u64,
    },
    /// Remove a complete internal set from the market hoard/position seam.
    Merge {
        market: MarketId,
        owner: OwnerId,
        quantity: u64,
    },
    /// Request one outcome's internal-to-external materialization.
    Materialize {
        market: MarketId,
        owner: OwnerId,
        destination: Hash32,
        outcome: u8,
        quantity: u64,
    },
    /// Request one outcome's external-to-internal dematerialization.
    Dematerialize {
        market: MarketId,
        owner: OwnerId,
        source: Hash32,
        outcome: u8,
        quantity: u64,
    },
    /// Advance an authenticated feed cursor by one checked evidence digest.
    FeedAdvance {
        feed: FeedId,
        cursor: u64,
        evidence: Hash32,
    },
    /// Append one order to a frozen-shape order page.
    PlaceOrder {
        market: MarketId,
        epoch: EpochId,
        order: OrderRecord,
    },
    /// Cancel one existing order identity.
    CancelOrder {
        market: MarketId,
        epoch: EpochId,
        owner: OwnerId,
        order_id: Hash32,
    },
    /// Settle one already-verified page.
    SettlePage {
        market: MarketId,
        epoch: EpochId,
        page_index: u16,
    },
}

const CREATE_TAG: u8 = 1;
const SPLIT_TAG: u8 = 2;
const MERGE_TAG: u8 = 3;
const MATERIALIZE_TAG: u8 = 4;
const DEMATERIALIZE_TAG: u8 = 5;
const FEED_ADVANCE_TAG: u8 = 6;
const PLACE_TAG: u8 = 7;
const CANCEL_TAG: u8 = 8;
const SETTLE_TAG: u8 = 9;

impl Intent {
    /// Return the exact encoded byte length for this intent.
    pub const fn encoded_len(&self) -> usize {
        match self {
            Self::CreateMarket { .. } => 2 + 32 + 32 + 8 + 1 + 32 + 32,
            Self::Split { .. } | Self::Merge { .. } => 2 + 32 + 32 + 8,
            Self::Materialize { .. } | Self::Dematerialize { .. } => 2 + 32 + 32 + 32 + 1 + 8,
            Self::FeedAdvance { .. } => 2 + 32 + 8 + 32,
            Self::PlaceOrder { .. } => 2 + 32 + 32 + 99,
            Self::CancelOrder { .. } => 2 + 32 + 32 + 32 + 32,
            Self::SettlePage { .. } => 2 + 32 + 32 + 2,
        }
    }
    /// Validate and encode into a caller-provided buffer.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        if out.len() < self.encoded_len() {
            return Err(CodecError::OutputTooSmall);
        };
        let mut w = Writer::new(out);
        match self {
            Self::CreateMarket {
                realm,
                profile,
                market_nonce,
                outcome_count,
                terms,
                feed,
            } => {
                check_count(*outcome_count)?;
                check_hash(*realm)?;
                check_hash(*profile)?;
                check_hash(*terms)?;
                check_hash(*feed)?;
                put_header(&mut w, CREATE_TAG, INTENT_VERSION)?;
                w.hash(*realm)?;
                w.hash(*profile)?;
                w.u64(*market_nonce)?;
                w.u8(*outcome_count)?;
                w.hash(*terms)?;
                w.hash(*feed)?
            }
            Self::Split {
                market,
                owner,
                quantity,
            }
            | Self::Merge {
                market,
                owner,
                quantity,
            } => {
                check_hash(*market)?;
                check_hash(*owner)?;
                if *quantity == 0 {
                    return Err(CodecError::ZeroValue);
                };
                put_header(
                    &mut w,
                    if matches!(self, Self::Split { .. }) {
                        SPLIT_TAG
                    } else {
                        MERGE_TAG
                    },
                    INTENT_VERSION,
                )?;
                w.hash(*market)?;
                w.hash(*owner)?;
                w.u64(*quantity)?
            }
            Self::Materialize {
                market,
                owner,
                destination,
                outcome,
                quantity,
            }
            | Self::Dematerialize {
                market,
                owner,
                source: destination,
                outcome,
                quantity,
            } => {
                check_hash(*market)?;
                check_hash(*owner)?;
                check_hash(*destination)?;
                if *outcome >= MAX_OUTCOMES as u8 {
                    return Err(CodecError::InvalidCount);
                }
                if *quantity == 0 {
                    return Err(CodecError::ZeroValue);
                };
                put_header(
                    &mut w,
                    if matches!(self, Self::Materialize { .. }) {
                        MATERIALIZE_TAG
                    } else {
                        DEMATERIALIZE_TAG
                    },
                    INTENT_VERSION,
                )?;
                w.hash(*market)?;
                w.hash(*owner)?;
                w.hash(*destination)?;
                w.u8(*outcome)?;
                w.u64(*quantity)?
            }
            Self::FeedAdvance {
                feed,
                cursor,
                evidence,
            } => {
                check_hash(*feed)?;
                check_hash(*evidence)?;
                put_header(&mut w, FEED_ADVANCE_TAG, INTENT_VERSION)?;
                w.hash(*feed)?;
                w.u64(*cursor)?;
                w.hash(*evidence)?
            }
            Self::PlaceOrder {
                market,
                epoch,
                order,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                order.validate()?;
                put_header(&mut w, PLACE_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                encode_order(&mut w, *order)?
            }
            Self::CancelOrder {
                market,
                epoch,
                owner,
                order_id,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                check_hash(*owner)?;
                check_hash(*order_id)?;
                put_header(&mut w, CANCEL_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.hash(*owner)?;
                w.hash(*order_id)?
            }
            Self::SettlePage {
                market,
                epoch,
                page_index,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                put_header(&mut w, SETTLE_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.u16(*page_index)?
            }
        };
        Ok(w.at)
    }
    /// Decode an intent by its discriminator, requiring exact length and canonical fields.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() < 2 {
            return Err(CodecError::Truncated);
        };
        let tag = input[0];
        let mut r = Reader::new(input, tag, INTENT_VERSION, input.len())?;
        match tag {
            CREATE_TAG => {
                let v = Self::CreateMarket {
                    realm: r.hash()?,
                    profile: r.hash()?,
                    market_nonce: r.u64()?,
                    outcome_count: r.u8()?,
                    terms: r.hash()?,
                    feed: r.hash()?,
                };
                r.done()?;
                let mut b = [0u8; MAX_INTENT_BYTES];
                let n = v.encode(&mut b)?;
                if n != input.len() {
                    return Err(CodecError::TrailingBytes);
                };
                Ok(v)
            }
            SPLIT_TAG | MERGE_TAG => {
                let market = r.hash()?;
                let owner = r.hash()?;
                let quantity = r.u64()?;
                r.done()?;
                check_hash(market)?;
                check_hash(owner)?;
                if quantity == 0 {
                    return Err(CodecError::ZeroValue);
                };
                Ok(if tag == SPLIT_TAG {
                    Self::Split {
                        market,
                        owner,
                        quantity,
                    }
                } else {
                    Self::Merge {
                        market,
                        owner,
                        quantity,
                    }
                })
            }
            MATERIALIZE_TAG | DEMATERIALIZE_TAG => {
                let market = r.hash()?;
                let owner = r.hash()?;
                let destination = r.hash()?;
                let outcome = r.u8()?;
                let quantity = r.u64()?;
                r.done()?;
                check_hash(market)?;
                check_hash(owner)?;
                check_hash(destination)?;
                if outcome >= MAX_OUTCOMES as u8 || quantity == 0 {
                    return Err(CodecError::InvalidCount);
                };
                Ok(if tag == MATERIALIZE_TAG {
                    Self::Materialize {
                        market,
                        owner,
                        destination,
                        outcome,
                        quantity,
                    }
                } else {
                    Self::Dematerialize {
                        market,
                        owner,
                        source: destination,
                        outcome,
                        quantity,
                    }
                })
            }
            FEED_ADVANCE_TAG => {
                let feed = r.hash()?;
                let cursor = r.u64()?;
                let evidence = r.hash()?;
                check_hash(feed)?;
                check_hash(evidence)?;
                let v = Self::FeedAdvance {
                    feed,
                    cursor,
                    evidence,
                };
                r.done()?;
                Ok(v)
            }
            PLACE_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let order = decode_order(&mut r)?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                order.validate()?;
                Ok(Self::PlaceOrder {
                    market,
                    epoch,
                    order,
                })
            }
            CANCEL_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let owner = r.hash()?;
                let order_id = r.hash()?;
                check_hash(market)?;
                check_hash(epoch)?;
                check_hash(owner)?;
                check_hash(order_id)?;
                let v = Self::CancelOrder {
                    market,
                    epoch,
                    owner,
                    order_id,
                };
                r.done()?;
                Ok(v)
            }
            SETTLE_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let page_index = r.u16()?;
                check_hash(market)?;
                check_hash(epoch)?;
                let v = Self::SettlePage {
                    market,
                    epoch,
                    page_index,
                };
                r.done()?;
                Ok(v)
            }
            _ => Err(CodecError::WrongTag),
        }
    }
}

/* Minimal SHA-256 implementation, adapted as straightforward fixed-array
 * code so the crate remains dependency-free and allocator-free. */
struct Sha256 {
    state: [u32; 8],
    block: [u8; 64],
    len: u64,
    used: usize,
}
impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            block: [0; 64],
            len: 0,
            used: 0,
        }
    }
    fn update(&mut self, data: &[u8]) {
        self.len = self.len.wrapping_add(data.len() as u64);
        let mut i = 0;
        while i < data.len() {
            self.block[self.used] = data[i];
            self.used += 1;
            i += 1;
            if self.used == 64 {
                self.compress();
                self.used = 0;
            }
        }
    }
    fn compress(&mut self) {
        let k: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut w = [0u32; 64];
        let mut i = 0;
        while i < 16 {
            let j = i * 4;
            w[i] = u32::from_be_bytes([
                self.block[j],
                self.block[j + 1],
                self.block[j + 2],
                self.block[j + 3],
            ]);
            i += 1;
        }
        while i < 64 {
            let a = w[i - 15];
            let b = w[i - 2];
            let s0 = a.rotate_right(7) ^ a.rotate_right(18) ^ (a >> 3);
            let s1 = b.rotate_right(17) ^ b.rotate_right(19) ^ (b >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
            i += 1;
        }
        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];
        i = 0;
        while i < 64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(k[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
            i += 1;
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h)
    }
    fn finish(mut self) -> [u8; 32] {
        let bit_len = self.len.wrapping_mul(8);
        self.block[self.used] = 0x80;
        self.used += 1;
        if self.used > 56 {
            while self.used < 64 {
                self.block[self.used] = 0;
                self.used += 1;
            }
            self.compress();
            self.used = 0;
        }
        while self.used < 56 {
            self.block[self.used] = 0;
            self.used += 1;
        }
        self.block[56..64].copy_from_slice(&bit_len.to_be_bytes());
        self.compress();
        let mut out = [0; 32];
        let mut i = 0;
        while i < 8 {
            out[i * 4..i * 4 + 4].copy_from_slice(&self.state[i].to_be_bytes());
            i += 1;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(n: u8) -> Hash32 {
        Hash32::from_bytes([n; 32])
    }
    fn market() -> MarketAccount {
        let m = h(1);
        let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
        let mut i = 0;
        while i < MAX_OUTCOMES {
            outcomes[i] = canonical_outcome_id(m, i as u8);
            i += 1;
        }
        MarketAccount {
            market: m,
            realm: h(2),
            profile: h(3),
            terms: h(4),
            outcome_count: MAX_OUTCOMES as u8,
            lifecycle: 0,
            stored_bump: 7,
            hoard_bump: 8,
            outcomes,
            feed: h(5),
            collateral_cap: 10,
            created_slot: 11,
            reserved: Hash32::ZERO,
        }
    }
    fn grid() -> PriceGridAccount {
        let mut ticks = [0; MAX_GRID_TICKS];
        ticks[1] = 2_500;
        ticks[2] = 5_000;
        ticks[3] = 7_500;
        ticks[4] = 10_000;
        let mut g = PriceGridAccount {
            grid: Hash32::ZERO,
            realm: h(2),
            price_scale: 10_000,
            tick_count: 5,
            ticks,
            stored_bump: 3,
            flags: 0,
        };
        g.grid = g.recomputed_grid_id().unwrap();
        g
    }
    fn terms() -> TermsAccount {
        let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
        let mut first = [0; MAX_OUTCOMES];
        first[0] = 4;
        let mut second = [0; MAX_OUTCOMES];
        second[1] = 4;
        let mut third = [0; MAX_OUTCOMES];
        third[0] = 1;
        third[1] = 3;
        payouts[0] = PayoutVectorBytes {
            denominator: 4,
            weights: first,
        };
        payouts[1] = PayoutVectorBytes {
            denominator: 4,
            weights: second,
        };
        payouts[2] = PayoutVectorBytes {
            denominator: 4,
            weights: third,
        };
        let mut t = TermsAccount {
            terms: Hash32::ZERO,
            realm: h(2),
            profile: h(3),
            feed: h(5),
            price_grid: grid().grid,
            outcome_count: MAX_OUTCOMES as u8,
            payout_count: 3,
            payouts,
            grid_family_id: 7,
            grid_version: 1,
            bucket_seconds: 60,
            expected_start_bucket: 100,
            expected_end_bucket_exclusive: 130,
            maturity_horizon_buckets: 30,
            coverage_policy_id: 11,
            repair_policy_id: 12,
            failure_policy_id: 13,
            stored_bump: 9,
            flags: 0,
        };
        t.terms = t.recomputed_terms_digest().unwrap();
        t
    }
    fn bound_market() -> MarketAccount {
        let mut m = market();
        m.terms = terms().terms;
        m
    }
    fn epoch_account() -> EpochAccount {
        EpochAccount {
            epoch: canonical_epoch_id(h(1), 4),
            market: h(1),
            book: h(21),
            terms: terms().terms,
            price_grid: grid().grid,
            policy: h(22),
            order_set: Hash32::ZERO,
            first_order_id: Hash32::ZERO,
            last_order_id: Hash32::ZERO,
            epoch_index: 4,
            relation_version: RELATION_VERSION,
            price_scale: 10_000,
            remainder_seed: 99,
            owner_count: 3,
            page_count: 0,
            order_count: 0,
            outcome_count: MAX_OUTCOMES as u8,
            phase: EPOCH_PHASE_OPEN,
            stored_bump: 6,
            flags: 0,
        }
    }
    fn order(id: u8) -> OrderRecord {
        OrderRecord {
            owner: h(20),
            order_id: h(id),
            outcome: 0,
            side: 0,
            quantity: 10,
            limit: 2_500,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
        }
    }
    fn single(id: u8) -> OrderSlot {
        OrderSlot::Single(order(id))
    }
    /// A two-outcome portfolio: three Eggs of outcome 0 and one of outcome 1.
    fn portfolio(id: u8) -> PortfolioRecord {
        let mut coefficients = [0; MAX_OUTCOMES];
        coefficients[0] = 3;
        coefficients[1] = 1;
        PortfolioRecord {
            owner: h(21),
            order_id: h(id),
            side: 0,
            active_len: 2,
            flags: 0,
            coefficients,
            lots: 5,
            limit_collateral_per_lot: 9_000,
            minimum_fill_lots: 2,
            generation: 1,
        }
    }
    /// Rebuild the page's range and digest after a slot was replaced.
    fn reseal(page: &mut OrderPageAccount) {
        page.first_order_id = page.orders[0].order_id();
        page.last_order_id = page.orders[page.order_count as usize - 1].order_id();
        page.page_digest = page.recomputed_page_digest().unwrap();
    }
    /// Build one open page over the given order ids.
    fn build_page(index: u16, count: u16, ids: &[u8], prev: Hash32) -> OrderPageAccount {
        let mut orders = [OrderSlot::Empty; MAX_ORDERS_PER_PAGE];
        let mut i = 0;
        while i < ids.len() {
            orders[i] = single(ids[i]);
            i += 1;
        }
        let (first, last) = if ids.is_empty() {
            (Hash32::ZERO, Hash32::ZERO)
        } else {
            (h(ids[0]), h(ids[ids.len() - 1]))
        };
        let mut page = OrderPageAccount {
            market: h(1),
            epoch: canonical_epoch_id(h(1), 4),
            order_set: Hash32::ZERO,
            page_digest: Hash32::ZERO,
            first_order_id: first,
            last_order_id: last,
            prev_page_last_order_id: prev,
            page_index: index,
            page_count: count,
            set_order_count: 0,
            order_count: ids.len() as u8,
            frozen: 0,
            stored_bump: 5,
            orders,
        };
        page.page_digest = page.recomputed_page_digest().unwrap();
        page
    }
    /// Freeze a page vector in place, computing the set-wide commitment.
    fn freeze_set(pages: &mut [OrderPageAccount]) {
        let mut total: u16 = 0;
        let mut i = 0;
        while i < pages.len() {
            total += pages[i].order_count as u16;
            i += 1;
        }
        let mut digests = [Hash32::ZERO; MAX_ORDER_PAGES];
        i = 0;
        while i < pages.len() {
            pages[i].frozen = 1;
            pages[i].set_order_count = total;
            pages[i].page_digest = pages[i].recomputed_page_digest().unwrap();
            digests[i] = pages[i].page_digest;
            i += 1;
        }
        let order_set = canonical_order_set_id(
            pages[0].market,
            pages[0].epoch,
            pages[0].page_count,
            total,
            &digests[..pages.len()],
        );
        i = 0;
        while i < pages.len() {
            pages[i].order_set = order_set;
            i += 1;
        }
    }
    /// A frozen, dense two-page set: page 0 full, page 1 partially filled.
    fn frozen_pages() -> [OrderPageAccount; 2] {
        let ids: [u8; MAX_ORDERS_PER_PAGE] =
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let page0 = build_page(0, 2, &ids, Hash32::ZERO);
        let page1 = build_page(1, 2, &[17, 18, 19], page0.last_order_id);
        let mut pages = [page0, page1];
        freeze_set(&mut pages);
        pages
    }
    fn frozen_epoch() -> EpochAccount {
        let pages = frozen_pages();
        let mut e = epoch_account();
        e.phase = EPOCH_PHASE_FROZEN;
        e.order_set = pages[0].order_set;
        e.first_order_id = pages[0].first_order_id;
        e.last_order_id = pages[1].last_order_id;
        e.page_count = 2;
        e.order_count = pages[0].set_order_count;
        e
    }
    fn candidate() -> CandidateRecord {
        let e = frozen_epoch();
        let mut prices = [0; MAX_OUTCOMES];
        prices[0] = 10_000;
        let mut c = CandidateRecord {
            candidate: Hash32::ZERO,
            epoch: e.epoch,
            market: e.market,
            prices,
            virtual_split: 5,
            virtual_merge: 0,
            honored_aon_mask: 0,
            weighted_direct_volume: -3,
            limit_surplus_price_units: 7,
            churn: 5,
            submitted_slot: 42,
            distinct_owners: 3,
            order_len: e.order_count as u8,
            outcome_count: MAX_OUTCOMES as u8,
            status: CANDIDATE_STATUS_VERIFIED,
            stored_bump: 4,
            flags: 0,
        };
        c.candidate = c.recomputed_candidate_digest().unwrap();
        c
    }
    fn receipt() -> SettlementReceiptAccount {
        let c = candidate();
        SettlementReceiptAccount {
            epoch: c.epoch,
            market: c.market,
            candidate: c.candidate,
            buy_order_id: h(1),
            sell_order_id: h(2),
            consideration_price_units: 3 * 10_000,
            quantity: 3,
            settled_quantity: 0,
            price: 10_000,
            sequence: 1,
            slice_index: 0,
            outcome: 0,
            leg_kind: RECEIPT_LEG_DIRECT,
            consumed_flags: 0,
            stored_bump: 2,
            flags: 0,
        }
    }
    fn supply_ledger() -> SupplyLedgerAccount {
        let mut internal = [0; MAX_OUTCOMES];
        let mut external = [0; MAX_OUTCOMES];
        internal[0] = 40;
        internal[1] = 10;
        external[0] = 60;
        SupplyLedgerAccount {
            market: h(1),
            realm: h(2),
            generation: 3,
            outcome_count: MAX_OUTCOMES as u8,
            internal_supply: internal,
            external_supply: external,
            stored_bump: 8,
            flags: 0,
        }
    }
    fn final_pot() -> FinalPotAccount {
        let c = candidate();
        let mut pot_internal = [0; MAX_OUTCOMES];
        pot_internal[0] = 5;
        FinalPotAccount {
            epoch: c.epoch,
            market: c.market,
            candidate: c.candidate,
            pot_internal,
            pot_cash_price_units: 50_000,
            rounding_pot_price_units: 3,
            outcome_count: MAX_OUTCOMES as u8,
            phase: POT_PHASE_OPEN,
            stored_bump: 1,
            flags: 0,
        }
    }
    fn resolution() -> ResolutionAccount {
        ResolutionAccount {
            market: h(1),
            terms: terms().terms,
            feed: h(5),
            window: h(30),
            feed_cursor: 130,
            sealed_end_bucket_exclusive: 130,
            repair_generation: 2,
            resolved_slot: 900,
            payout_index: 1,
            stored_bump: 3,
            flags: 0,
        }
    }
    /// Every decoder must refuse a short, long, mistagged, or misversioned account.
    fn hostile_header<T: core::fmt::Debug + PartialEq>(
        bytes: &[u8],
        decode: fn(&[u8]) -> Result<T>,
    ) {
        let n = bytes.len();
        let mut buf = [0u8; 2048];
        buf[..n].copy_from_slice(bytes);
        assert_eq!(decode(&buf[..n - 1]), Err(CodecError::Truncated));
        assert_eq!(decode(&buf[..n + 1]), Err(CodecError::TrailingBytes));
        let mut wrong_tag = buf;
        wrong_tag[0] ^= 0x80;
        assert_eq!(decode(&wrong_tag[..n]), Err(CodecError::WrongTag));
        let mut wrong_version = buf;
        wrong_version[1] = 0;
        assert_eq!(decode(&wrong_version[..n]), Err(CodecError::WrongVersion));
    }

    #[test]
    fn account_golden_lengths() {
        assert_eq!(account_len::REALM, 70);
        assert_eq!(account_len::PROFILE, 100);
        assert_eq!(account_len::MARKET, 726);
        assert_eq!(account_len::HOARD, 108);
        assert_eq!(account_len::POSITION, 220);
        assert_eq!(account_len::FEED, 124);
        assert_eq!(account_len::ORDER_PAGE, 3883);
        assert_eq!(account_len::SUPPLY_LEDGER, 333);
        assert_eq!(account_len::TERMS, 1304);
        assert_eq!(account_len::PRICE_GRID, 589);
        assert_eq!(account_len::EPOCH, 328);
        assert_eq!(account_len::CANDIDATE, 305);
        assert_eq!(account_len::FINAL_POT, 262);
        assert_eq!(account_len::SETTLEMENT_RECEIPT, 217);
        assert_eq!(account_len::RESOLUTION, 165);
        assert_eq!(ORDER_RECORD_BYTES, 99);
        assert_eq!(PORTFOLIO_RECORD_BYTES, 227);
        assert_eq!(ORDER_SLOT_BYTES, 228);
        // The page is exactly its header plus sixteen common-width slots.
        assert_eq!(
            account_len::ORDER_PAGE,
            235 + MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES
        );
    }
    #[test]
    fn mirrored_bounds_match_their_owning_crates() {
        // These are restatements, not imports; a divergence is a real defect.
        assert_eq!(MAX_OUTCOMES, 16);
        assert_eq!(MAX_PAYOUTS, 8);
        assert_eq!(MAX_GRID_TICKS, 64);
        assert_eq!(MAX_EPOCH_ORDERS, 64);
        assert_eq!(MAX_PORTFOLIO_ORDERS, 8);
        assert_eq!(MAX_BUCKET_SECONDS, 86_400);
        assert_eq!(MAX_WINDOW_BUCKETS, 1_000_000);
        assert_eq!(RELATION_VERSION, 1);
    }
    #[test]
    fn market_round_trip_and_golden_prefix() {
        let v = market();
        let mut b = [0; account_len::MARKET];
        assert_eq!(v.encode(&mut b), Ok(account_len::MARKET));
        assert_eq!(&b[..2], [MARKET_TAG, account_version::MARKET]);
        assert_eq!(MarketAccount::decode(&b), Ok(v));
    }
    #[test]
    fn every_account_codec_round_trips() {
        let realm = RealmAccount {
            realm: h(1),
            profile: h(2),
            max_outcomes: 16,
            profile_version: 1,
            stored_bump: 3,
            flags: 0,
        };
        let mut realm_bytes = [0; account_len::REALM];
        realm.encode(&mut realm_bytes).unwrap();
        assert_eq!(RealmAccount::decode(&realm_bytes), Ok(realm));

        let profile = ProfileAccount {
            profile: h(2),
            realm: h(1),
            collateral_policy_digest: Hash32::ZERO,
            version: 1,
            flags: 0,
        };
        let mut profile_bytes = [0; account_len::PROFILE];
        profile.encode(&mut profile_bytes).unwrap();
        assert_eq!(ProfileAccount::decode(&profile_bytes), Ok(profile));

        let hoard = HoardAccount {
            market: h(3),
            realm: h(1),
            authority: h(4),
            collateral_atoms: 5,
            stored_bump: 6,
            flags: 0,
        };
        let mut hoard_bytes = [0; account_len::HOARD];
        hoard.encode(&mut hoard_bytes).unwrap();
        assert_eq!(HoardAccount::decode(&hoard_bytes), Ok(hoard));

        let position = PositionAccount {
            market: h(3),
            owner: h(7),
            generation: 8,
            internal: [9; MAX_OUTCOMES],
            cash_atoms: 11,
            reserved_cash_atoms: 10,
            stored_bump: 12,
            close_state: 0,
        };
        let mut position_bytes = [0; account_len::POSITION];
        position.encode(&mut position_bytes).unwrap();
        assert_eq!(PositionAccount::decode(&position_bytes), Ok(position));
        assert_eq!(position.free_cash_atoms(), Ok(1));

        let feed = FeedAccount {
            feed: h(13),
            realm: h(1),
            cursor: 14,
            next_boundary: 15,
            archive_pages: 16,
            summary: h(17),
            stored_bump: 18,
            flags: 0,
        };
        let mut feed_bytes = [0; account_len::FEED];
        feed.encode(&mut feed_bytes).unwrap();
        assert_eq!(FeedAccount::decode(&feed_bytes), Ok(feed));

        let page = build_page(0, 1, &[3, 9], Hash32::ZERO);
        let mut page_bytes = [0; account_len::ORDER_PAGE];
        page.encode(&mut page_bytes).unwrap();
        assert_eq!(OrderPageAccount::decode(&page_bytes), Ok(page));
    }
    #[test]
    fn every_persisted_state_account_round_trips() {
        let ledger = supply_ledger();
        let mut b = [0; account_len::SUPPLY_LEDGER];
        assert_eq!(ledger.encode(&mut b), Ok(account_len::SUPPLY_LEDGER));
        assert_eq!(SupplyLedgerAccount::decode(&b), Ok(ledger));
        hostile_header(&b, SupplyLedgerAccount::decode);

        let t = terms();
        let mut b = [0; account_len::TERMS];
        assert_eq!(t.encode(&mut b), Ok(account_len::TERMS));
        assert_eq!(TermsAccount::decode(&b), Ok(t));
        hostile_header(&b, TermsAccount::decode);

        let g = grid();
        let mut b = [0; account_len::PRICE_GRID];
        assert_eq!(g.encode(&mut b), Ok(account_len::PRICE_GRID));
        assert_eq!(PriceGridAccount::decode(&b), Ok(g));
        hostile_header(&b, PriceGridAccount::decode);

        let e = frozen_epoch();
        let mut b = [0; account_len::EPOCH];
        assert_eq!(e.encode(&mut b), Ok(account_len::EPOCH));
        assert_eq!(EpochAccount::decode(&b), Ok(e));
        hostile_header(&b, EpochAccount::decode);

        let c = candidate();
        let mut b = [0; account_len::CANDIDATE];
        assert_eq!(c.encode(&mut b), Ok(account_len::CANDIDATE));
        assert_eq!(CandidateRecord::decode(&b), Ok(c));
        hostile_header(&b, CandidateRecord::decode);

        let p = final_pot();
        let mut b = [0; account_len::FINAL_POT];
        assert_eq!(p.encode(&mut b), Ok(account_len::FINAL_POT));
        assert_eq!(FinalPotAccount::decode(&b), Ok(p));
        hostile_header(&b, FinalPotAccount::decode);

        let r = receipt();
        let mut b = [0; account_len::SETTLEMENT_RECEIPT];
        assert_eq!(r.encode(&mut b), Ok(account_len::SETTLEMENT_RECEIPT));
        assert_eq!(SettlementReceiptAccount::decode(&b), Ok(r));
        hostile_header(&b, SettlementReceiptAccount::decode);

        let res = resolution();
        let mut b = [0; account_len::RESOLUTION];
        assert_eq!(res.encode(&mut b), Ok(account_len::RESOLUTION));
        assert_eq!(ResolutionAccount::decode(&b), Ok(res));
        hostile_header(&b, ResolutionAccount::decode);
    }
    #[test]
    fn hostile_lengths_and_padding_refuse() {
        let v = market();
        let mut b = [0; account_len::MARKET];
        v.encode(&mut b).unwrap();
        assert_eq!(
            MarketAccount::decode(&b[..b.len() - 1]),
            Err(CodecError::Truncated)
        );
        let mut x = [0; account_len::MARKET + 1];
        x[..b.len()].copy_from_slice(&b);
        assert_eq!(MarketAccount::decode(&x), Err(CodecError::TrailingBytes));
        x[account_len::MARKET - 1] = 9;
        assert_eq!(
            MarketAccount::decode(&x[..b.len()]),
            Err(CodecError::NonCanonicalPadding)
        );
    }
    #[test]
    fn ids_are_domain_separated_and_stable() {
        assert_ne!(
            canonical_market_id(h(1), h(2), 3),
            canonical_market_id(h(1), h(2), 4)
        );
        assert_eq!(canonical_outcome_id(h(1), 2), canonical_outcome_id(h(1), 2));
        assert_ne!(canonical_outcome_id(h(1), 2), canonical_outcome_id(h(1), 3));
        assert_eq!(canonical_epoch_id(h(1), 2), canonical_epoch_id(h(1), 2));
        assert_ne!(canonical_epoch_id(h(1), 2), canonical_epoch_id(h(1), 3));
        assert_ne!(canonical_epoch_id(h(1), 2), canonical_outcome_id(h(1), 2));
        assert_ne!(canonical_terms_digest(b"x"), canonical_price_grid_id(b"x"));
        assert_ne!(
            canonical_candidate_digest(b"x"),
            canonical_terms_digest(b"x")
        );
    }
    #[test]
    fn error_codes_are_stable_and_never_collide_across_facts() {
        assert_eq!(CodecError::Truncated.code(), 2011);
        assert_eq!(CodecError::NonCanonicalPadding.code(), 2022);
        assert_eq!(CodecError::InvalidPriceGrid.code(), 2049);
        assert_eq!(CodecError::InvalidTick.code(), 2050);
        assert_eq!(CodecError::ZeroIdentity.code(), 4009);
        assert_eq!(CodecError::MismatchedBinding.code(), 4011);
        assert_eq!(CodecError::AggregateClosureMismatch.code(), 5011);
        assert_eq!(CodecError::InvalidConsideration.code(), 5015);
        assert_eq!(CodecError::OutputTooSmall.code(), 8004);
        let all = [
            CodecError::Truncated,
            CodecError::TrailingBytes,
            CodecError::WrongTag,
            CodecError::WrongVersion,
            CodecError::InvalidCount,
            CodecError::InvalidEnum,
            CodecError::ZeroValue,
            CodecError::ZeroIdentity,
            CodecError::NonCanonicalIdentity,
            CodecError::NonCanonicalPadding,
            CodecError::InvalidPriceGrid,
            CodecError::InvalidTick,
            CodecError::MismatchedBinding,
            CodecError::AggregateClosureMismatch,
            CodecError::InvalidConsideration,
            CodecError::ArithmeticOverflow,
            CodecError::OutputTooSmall,
        ];
        let mut i = 0;
        while i < all.len() {
            let mut j = i + 1;
            while j < all.len() {
                assert_ne!(all[i].code(), all[j].code());
                j += 1;
            }
            i += 1;
        }
    }
    #[test]
    fn changed_accounts_refuse_version_one_and_unchanged_accounts_refuse_version_two() {
        let profile = ProfileAccount {
            profile: h(2),
            realm: h(1),
            collateral_policy_digest: Hash32::ZERO,
            version: 1,
            flags: 0,
        };
        let mut b = [0; account_len::PROFILE];
        profile.encode(&mut b).unwrap();
        assert_eq!(b[1], account_version::PROFILE);
        assert_eq!(account_version::PROFILE, LAYOUT_VERSION_V2);
        b[1] = LAYOUT_VERSION_V1;
        assert_eq!(ProfileAccount::decode(&b), Err(CodecError::WrongVersion));

        // The page is on its third shape: bare records, then the page-set
        // commitment fields, then tagged fixed-width slots.  Both earlier
        // versions are refused explicitly.
        let page = build_page(0, 1, &[3], Hash32::ZERO);
        let mut b = [0; account_len::ORDER_PAGE];
        page.encode(&mut b).unwrap();
        assert_eq!(b[1], LAYOUT_VERSION);
        assert_eq!(account_version::ORDER_PAGE, 3);
        b[1] = LAYOUT_VERSION_V1;
        assert_eq!(OrderPageAccount::decode(&b), Err(CodecError::WrongVersion));
        b[1] = LAYOUT_VERSION_V2;
        assert_eq!(OrderPageAccount::decode(&b), Err(CodecError::WrongVersion));

        let mut b = [0; account_len::MARKET];
        market().encode(&mut b).unwrap();
        assert_eq!(b[1], LAYOUT_VERSION_V1);
        b[1] = LAYOUT_VERSION;
        assert_eq!(MarketAccount::decode(&b), Err(CodecError::WrongVersion));
    }
    #[test]
    fn profile_policy_digest_is_zero_until_frozen() {
        let mut profile = ProfileAccount {
            profile: h(2),
            realm: h(1),
            collateral_policy_digest: Hash32::ZERO,
            version: 1,
            flags: 0,
        };
        let mut b = [0; account_len::PROFILE];
        profile.encode(&mut b).unwrap();
        // The digest occupies bytes 66..98 of the profile account.
        assert_eq!(&b[66..98], &[0u8; 32]);

        profile.collateral_policy_digest = h(77);
        assert_eq!(profile.validate(), Err(CodecError::NonCanonicalPadding));

        profile.flags = PROFILE_FLAG_POLICY_FROZEN;
        profile.encode(&mut b).unwrap();
        assert_eq!(&b[66..98], &[77u8; 32]);
        assert_eq!(ProfileAccount::decode(&b), Ok(profile));

        profile.collateral_policy_digest = Hash32::ZERO;
        assert_eq!(profile.validate(), Err(CodecError::ZeroIdentity));

        profile.flags = 2;
        assert_eq!(profile.validate(), Err(CodecError::InvalidEnum));
    }
    #[test]
    fn realm_refuses_a_narrowed_outcome_width() {
        let mut realm = RealmAccount {
            realm: h(1),
            profile: h(2),
            max_outcomes: 16,
            profile_version: 1,
            stored_bump: 3,
            flags: 0,
        };
        assert_eq!(realm.validate(), Ok(()));
        realm.max_outcomes = 2;
        assert_eq!(realm.validate(), Err(CodecError::InvalidCount));
        realm.max_outcomes = 17;
        assert_eq!(realm.validate(), Err(CodecError::InvalidCount));
    }
    #[test]
    fn position_refuses_reserved_cash_above_total() {
        let mut position = PositionAccount {
            market: h(3),
            owner: h(7),
            generation: 8,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: 10,
            reserved_cash_atoms: 10,
            stored_bump: 12,
            close_state: 0,
        };
        assert_eq!(position.validate(), Ok(()));
        assert_eq!(position.free_cash_atoms(), Ok(0));
        position.reserved_cash_atoms = 11;
        assert_eq!(
            position.validate(),
            Err(CodecError::AggregateClosureMismatch)
        );
        let mut b = [0; account_len::POSITION];
        assert_eq!(
            position.encode(&mut b),
            Err(CodecError::AggregateClosureMismatch)
        );
    }
    #[test]
    fn supply_ledger_decomposes_aggregate_supply_and_refuses_overflow() {
        let ledger = supply_ledger();
        assert_eq!(ledger.aggregate_supply(0), Ok(100));
        assert_eq!(ledger.aggregate_supply(1), Ok(10));
        assert_eq!(
            ledger.aggregate_supply(MAX_OUTCOMES as u8),
            Err(CodecError::InvalidCount)
        );

        let mut overflow = ledger;
        overflow.internal_supply[0] = u64::MAX;
        overflow.external_supply[0] = 1;
        assert_eq!(overflow.validate(), Err(CodecError::ArithmeticOverflow));

        let mut narrow = ledger;
        narrow.outcome_count = 2;
        assert_eq!(narrow.validate(), Ok(()));
        narrow.internal_supply[3] = 1;
        assert_eq!(narrow.validate(), Err(CodecError::NonCanonicalPadding));

        let mut flagged = ledger;
        flagged.flags = 1;
        assert_eq!(flagged.validate(), Err(CodecError::InvalidEnum));

        let mut zero_market = ledger;
        zero_market.market = Hash32::ZERO;
        assert_eq!(zero_market.validate(), Err(CodecError::ZeroIdentity));
    }
    #[test]
    fn supply_ledger_binds_market_and_bounds_one_position() {
        let ledger = supply_ledger();
        assert_eq!(ledger.binds_market(&market()), Ok(()));
        let mut other = market();
        other.market = h(9);
        let mut i = 0;
        while i < MAX_OUTCOMES {
            other.outcomes[i] = canonical_outcome_id(other.market, i as u8);
            i += 1;
        }
        assert_eq!(
            ledger.binds_market(&other),
            Err(CodecError::MismatchedBinding)
        );

        let mut position = PositionAccount {
            market: h(1),
            owner: h(7),
            generation: 1,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: 0,
            reserved_cash_atoms: 0,
            stored_bump: 1,
            close_state: 0,
        };
        position.internal[0] = 40;
        assert_eq!(ledger.check_position_bound(&position), Ok(()));
        position.internal[0] = 41;
        assert_eq!(
            ledger.check_position_bound(&position),
            Err(CodecError::AggregateClosureMismatch)
        );
    }
    #[test]
    fn terms_digest_binds_the_payout_set_and_the_window_policy() {
        let t = terms();
        assert_eq!(t.validate(), Ok(()));
        assert_eq!(t.binds_market(&bound_market()), Ok(()));
        assert_eq!(
            t.binds_market(&market()),
            Err(CodecError::MismatchedBinding)
        );

        // Every field under the digest moves it.
        let mut moved = t;
        moved.payouts[0].weights[0] = 3;
        moved.payouts[0].weights[1] = 1;
        assert_ne!(moved.recomputed_terms_digest().unwrap(), t.terms);
        assert_eq!(moved.validate(), Err(CodecError::NonCanonicalIdentity));

        let mut window = t;
        window.expected_end_bucket_exclusive = 131;
        assert_ne!(window.recomputed_terms_digest().unwrap(), t.terms);

        let mut policy = t;
        policy.failure_policy_id = 14;
        assert_ne!(policy.recomputed_terms_digest().unwrap(), t.terms);

        // The account-local bump is deliberately outside the digest.
        let mut bumped = t;
        bumped.stored_bump = 200;
        assert_eq!(bumped.recomputed_terms_digest().unwrap(), t.terms);
        assert_eq!(bumped.validate(), Ok(()));
    }
    #[test]
    fn terms_refuse_a_malformed_payout_set() {
        let base = terms();
        let mut short = base;
        short.payout_count = 0;
        assert_eq!(short.validate(), Err(CodecError::InvalidCount));

        let mut long = base;
        long.payout_count = MAX_PAYOUTS as u8 + 1;
        assert_eq!(long.validate(), Err(CodecError::InvalidCount));

        let mut mixed = base;
        mixed.payouts[1].denominator = 8;
        mixed.terms = mixed.recomputed_terms_digest().unwrap();
        assert_eq!(mixed.validate(), Err(CodecError::MismatchedBinding));

        let mut unsummed = base;
        unsummed.payouts[2].weights[1] = 2;
        unsummed.terms = unsummed.recomputed_terms_digest().unwrap();
        assert_eq!(unsummed.validate(), Err(CodecError::InvalidCount));

        let mut padded = base;
        padded.payouts[3].denominator = 4;
        padded.terms = padded.recomputed_terms_digest().unwrap();
        assert_eq!(padded.validate(), Err(CodecError::NonCanonicalPadding));

        let mut zero = base;
        zero.payouts[0].denominator = 0;
        zero.payouts[1].denominator = 0;
        zero.payouts[2].denominator = 0;
        zero.terms = zero.recomputed_terms_digest().unwrap();
        assert_eq!(zero.validate(), Err(CodecError::ZeroValue));
    }
    #[test]
    fn terms_refuse_window_policy_holes() {
        let base = terms();
        let mut unnamed = base;
        unnamed.coverage_policy_id = 0;
        unnamed.terms = unnamed.recomputed_terms_digest().unwrap();
        assert_eq!(unnamed.validate(), Err(CodecError::ZeroValue));

        let mut empty = base;
        empty.expected_end_bucket_exclusive = empty.expected_start_bucket;
        empty.terms = empty.recomputed_terms_digest().unwrap();
        assert_eq!(empty.validate(), Err(CodecError::InvalidCount));

        let mut reversed = base;
        reversed.expected_end_bucket_exclusive = reversed.expected_start_bucket - 1;
        reversed.terms = reversed.recomputed_terms_digest().unwrap();
        assert_eq!(reversed.validate(), Err(CodecError::InvalidCount));

        let mut prefix = base;
        prefix.maturity_horizon_buckets = 29;
        prefix.terms = prefix.recomputed_terms_digest().unwrap();
        assert_eq!(prefix.validate(), Err(CodecError::InvalidCount));

        let mut coarse = base;
        coarse.bucket_seconds = MAX_BUCKET_SECONDS + 1;
        coarse.terms = coarse.recomputed_terms_digest().unwrap();
        assert_eq!(coarse.validate(), Err(CodecError::InvalidCount));

        let mut boundary = base;
        boundary.bucket_seconds = MAX_BUCKET_SECONDS;
        boundary.terms = boundary.recomputed_terms_digest().unwrap();
        assert_eq!(boundary.validate(), Ok(()));
    }
    #[test]
    fn price_grid_freezes_the_limit_to_tick_mapping() {
        let g = grid();
        assert_eq!(g.validate(), Ok(()));
        assert_eq!(g.tick_of(0), Ok(0));
        assert_eq!(g.tick_of(10_000), Ok(4));
        assert_eq!(g.tick_of(2_501), Err(CodecError::InvalidTick));
        assert_eq!(g.tick_value(4), Ok(10_000));
        assert_eq!(g.tick_value(5), Err(CodecError::InvalidTick));
        assert_eq!(g.binds_terms(&terms()), Ok(()));

        let mut other = terms();
        other.price_grid = h(99);
        other.terms = other.recomputed_terms_digest().unwrap();
        assert_eq!(g.binds_terms(&other), Err(CodecError::MismatchedBinding));
    }
    #[test]
    fn price_grid_refuses_unsorted_over_scale_and_padded_ticks() {
        let base = grid();
        let mut unsorted = base;
        unsorted.ticks[2] = 2_500;
        unsorted.grid = unsorted.recomputed_grid_id().unwrap();
        assert_eq!(unsorted.validate(), Err(CodecError::InvalidPriceGrid));

        let mut over = base;
        over.ticks[4] = 10_001;
        over.grid = over.recomputed_grid_id().unwrap();
        assert_eq!(over.validate(), Err(CodecError::InvalidPriceGrid));

        let mut padded = base;
        padded.ticks[5] = 1;
        padded.grid = padded.recomputed_grid_id().unwrap();
        assert_eq!(padded.validate(), Err(CodecError::NonCanonicalPadding));

        let mut degenerate = base;
        degenerate.tick_count = 1;
        degenerate.grid = degenerate.recomputed_grid_id().unwrap();
        assert_eq!(degenerate.validate(), Err(CodecError::InvalidPriceGrid));

        let mut scaleless = base;
        scaleless.price_scale = 0;
        scaleless.grid = scaleless.recomputed_grid_id().unwrap();
        assert_eq!(scaleless.validate(), Err(CodecError::InvalidPriceGrid));

        let mut huge = base;
        huge.price_scale = MAX_PRICE_SCALE + 1;
        huge.grid = huge.recomputed_grid_id().unwrap();
        assert_eq!(huge.validate(), Err(CodecError::InvalidPriceGrid));

        let mut forged = base;
        forged.price_scale = 20_000;
        assert_eq!(forged.validate(), Err(CodecError::NonCanonicalIdentity));
    }
    #[test]
    fn off_grid_order_limits_are_refused_at_decode_time() {
        let g = grid();
        let page = build_page(0, 1, &[3, 4], Hash32::ZERO);
        let mut b = [0; account_len::ORDER_PAGE];
        page.encode(&mut b).unwrap();
        assert_eq!(OrderPageAccount::decode_on_grid(&b, &g), Ok(page));

        let mut off = page;
        off.orders[1] = OrderSlot::Single(OrderRecord {
            limit: 2_501,
            ..order(4)
        });
        off.page_digest = off.recomputed_page_digest().unwrap();
        let mut b = [0; account_len::ORDER_PAGE];
        off.encode(&mut b).unwrap();
        assert_eq!(OrderPageAccount::decode(&b), Ok(off));
        assert_eq!(
            OrderPageAccount::decode_on_grid(&b, &g),
            Err(CodecError::InvalidTick)
        );
    }
    #[test]
    fn epoch_open_phase_commits_to_nothing_and_frozen_phase_commits_to_everything() {
        let open = epoch_account();
        assert_eq!(open.validate(), Ok(()));
        let mut leaky = open;
        leaky.order_set = h(40);
        assert_eq!(leaky.validate(), Err(CodecError::NonCanonicalPadding));

        let frozen = frozen_epoch();
        assert_eq!(frozen.validate(), Ok(()));
        let mut unsealed = frozen;
        unsealed.order_set = Hash32::ZERO;
        assert_eq!(unsealed.validate(), Err(CodecError::ZeroIdentity));

        let mut forged = frozen;
        forged.epoch_index = 5;
        assert_eq!(forged.validate(), Err(CodecError::NonCanonicalIdentity));

        let mut wrong_relation = frozen;
        wrong_relation.relation_version = 2;
        assert_eq!(wrong_relation.validate(), Err(CodecError::WrongVersion));

        let mut sparse = frozen;
        sparse.order_count = 16;
        assert_eq!(sparse.validate(), Err(CodecError::MismatchedBinding));

        let mut ownerless = frozen;
        ownerless.owner_count = 0;
        assert_eq!(ownerless.validate(), Err(CodecError::InvalidCount));

        let mut phased = frozen;
        phased.phase = EPOCH_PHASE_LAPSED + 1;
        assert_eq!(phased.validate(), Err(CodecError::InvalidEnum));
    }
    #[test]
    fn epoch_binds_terms_grid_and_its_frozen_page_set() {
        let e = frozen_epoch();
        assert_eq!(e.binds_terms(&terms(), &grid()), Ok(()));

        let mut scaled = e;
        scaled.price_scale = 9_999;
        assert_eq!(
            scaled.binds_terms(&terms(), &grid()),
            Err(CodecError::MismatchedBinding)
        );

        let pages = frozen_pages();
        assert_eq!(e.binds_page_set(&pages), Ok(()));
        assert_eq!(
            epoch_account().binds_page_set(&pages),
            Err(CodecError::MismatchedBinding)
        );

        let mut short = e;
        short.order_count = 18;
        assert_eq!(
            short.binds_page_set(&pages),
            Err(CodecError::MismatchedBinding)
        );
    }
    #[test]
    fn page_set_closure_accepts_one_dense_ordered_frozen_set() {
        let pages = frozen_pages();
        let order_set = verify_page_set(&pages).unwrap();
        assert_eq!(order_set, pages[0].order_set);
        assert_eq!(pages[0].set_order_count, 19);
        assert_eq!(pages[0].order_count, MAX_ORDERS_PER_PAGE as u8);
        assert_eq!(pages[1].order_count, 3);
        assert_eq!(verify_page_set(&[]), Err(CodecError::InvalidCount));
        assert_eq!(verify_page_set(&pages[..1]), Err(CodecError::InvalidCount));
    }
    #[test]
    fn page_set_refuses_gap_duplicate_reorder_and_post_freeze_mutation() {
        let pages = frozen_pages();

        // Gap: the middle page of a three-page set is dropped.
        let ids: [u8; MAX_ORDERS_PER_PAGE] =
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let ids2: [u8; MAX_ORDERS_PER_PAGE] = [
            17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
        ];
        let p0 = build_page(0, 3, &ids, Hash32::ZERO);
        let p1 = build_page(1, 3, &ids2, p0.last_order_id);
        let p2 = build_page(2, 3, &[33, 34], p1.last_order_id);
        let mut three = [p0, p1, p2];
        freeze_set(&mut three);
        assert_eq!(verify_page_set(&three), Ok(three[0].order_set));
        let gapped = [three[0], three[2]];
        assert_eq!(verify_page_set(&gapped), Err(CodecError::InvalidCount));

        // Duplicate order id across a page boundary.
        let mut dup = pages;
        dup[1].orders[0] = single(16);
        dup[1].first_order_id = h(16);
        dup[1].page_digest = dup[1].recomputed_page_digest().unwrap();
        assert_eq!(dup[1].validate(), Err(CodecError::NonCanonicalIdentity));
        assert_eq!(verify_page_set(&dup), Err(CodecError::NonCanonicalIdentity));

        // Reorder: the two pages are presented out of index order.
        let reordered = [pages[1], pages[0]];
        assert_eq!(
            verify_page_set(&reordered),
            Err(CodecError::MismatchedBinding)
        );

        // Post-freeze mutation of one order byte.
        let mut mutated = pages;
        mutated[0].orders[3] = OrderSlot::Single(OrderRecord {
            quantity: 11,
            ..order(4)
        });
        assert_eq!(mutated[0].validate(), Err(CodecError::MismatchedBinding));
        // Recomputing the page digest does not repair the set commitment.
        mutated[0].page_digest = mutated[0].recomputed_page_digest().unwrap();
        assert_eq!(
            verify_page_set(&mutated),
            Err(CodecError::MismatchedBinding)
        );

        // A broken predecessor link.
        let mut unlinked = pages;
        unlinked[1].prev_page_last_order_id = h(15);
        assert_eq!(
            verify_page_set(&unlinked),
            Err(CodecError::NonCanonicalIdentity)
        );

        // An unfrozen page cannot participate in a closed set.
        let mut thawed = pages;
        thawed[1].frozen = 0;
        assert_eq!(
            verify_page_set(&thawed),
            Err(CodecError::NonCanonicalPadding)
        );
    }
    #[test]
    fn order_page_refuses_stale_ranges_and_sparse_frozen_pages() {
        let mut page = build_page(0, 1, &[3, 9], Hash32::ZERO);
        assert_eq!(page.validate(), Ok(()));

        let mut stale = page;
        stale.last_order_id = h(8);
        assert_eq!(stale.validate(), Err(CodecError::MismatchedBinding));

        let mut chained = page;
        chained.prev_page_last_order_id = h(2);
        assert_eq!(chained.validate(), Err(CodecError::NonCanonicalPadding));

        // Page one must link, and must open strictly above its predecessor.
        let mut second = build_page(1, 2, &[9, 10], h(9));
        assert_eq!(second.validate(), Err(CodecError::NonCanonicalIdentity));
        second.prev_page_last_order_id = Hash32::ZERO;
        assert_eq!(second.validate(), Err(CodecError::ZeroIdentity));

        // A frozen set commits a count; an open page commits none.
        page.set_order_count = 2;
        assert_eq!(page.validate(), Err(CodecError::NonCanonicalPadding));

        let ids: [u8; MAX_ORDERS_PER_PAGE] =
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let full = build_page(0, 2, &ids, Hash32::ZERO);
        let tail = build_page(1, 2, &[17], full.last_order_id);
        let mut set = [full, tail];
        freeze_set(&mut set);
        let mut sparse = set;
        sparse[0].order_count = 15;
        sparse[0].last_order_id = h(15);
        sparse[0].orders[15] = OrderSlot::Empty;
        sparse[0].page_digest = sparse[0].recomputed_page_digest().unwrap();
        assert_eq!(sparse[0].validate(), Err(CodecError::InvalidCount));

        let mut empty_frozen = set;
        empty_frozen[1].order_count = 0;
        assert_eq!(empty_frozen[1].validate(), Err(CodecError::InvalidCount));

        let mut too_many = build_page(0, 5, &[3], Hash32::ZERO);
        too_many.page_digest = too_many.recomputed_page_digest().unwrap();
        assert_eq!(too_many.validate(), Err(CodecError::InvalidCount));
    }
    #[test]
    fn page_rejects_duplicate_or_unsorted_orders() {
        let mut page = build_page(0, 1, &[3], Hash32::ZERO);
        page.orders[1] = single(3);
        page.order_count = 2;
        page.last_order_id = h(3);
        page.page_digest = page.recomputed_page_digest().unwrap();
        assert_eq!(page.validate(), Err(CodecError::NonCanonicalIdentity));

        let mut unsorted = build_page(0, 1, &[3, 4], Hash32::ZERO);
        unsorted.orders[0] = single(4);
        unsorted.orders[1] = single(3);
        unsorted.first_order_id = h(4);
        unsorted.last_order_id = h(3);
        unsorted.page_digest = unsorted.recomputed_page_digest().unwrap();
        assert_eq!(unsorted.validate(), Err(CodecError::NonCanonicalIdentity));

        let mut padded = build_page(0, 1, &[3], Hash32::ZERO);
        padded.orders[5] = single(9);
        padded.page_digest = padded.recomputed_page_digest().unwrap();
        assert_eq!(padded.validate(), Err(CodecError::NonCanonicalPadding));
    }
    /// Byte offset of slot zero: everything before the slot array.
    const PAGE_HEADER_BYTES: usize =
        account_len::ORDER_PAGE - MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES;
    fn slot_at(bytes: &[u8], index: usize) -> &[u8] {
        let start = PAGE_HEADER_BYTES + index * ORDER_SLOT_BYTES;
        &bytes[start..start + ORDER_SLOT_BYTES]
    }
    /// A frozen two-page set whose tail page opens with a portfolio record.
    fn frozen_pages_with_portfolio() -> [OrderPageAccount; 2] {
        let ids: [u8; MAX_ORDERS_PER_PAGE] =
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let page0 = build_page(0, 2, &ids, Hash32::ZERO);
        let mut page1 = build_page(1, 2, &[17, 18, 19], page0.last_order_id);
        page1.orders[0] = OrderSlot::Portfolio(portfolio(17));
        reseal(&mut page1);
        let mut pages = [page0, page1];
        freeze_set(&mut pages);
        pages
    }
    #[test]
    fn portfolio_and_single_egg_records_share_one_page_and_one_order_chain() {
        let mut page = build_page(0, 1, &[3, 9], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(portfolio(9));
        reseal(&mut page);
        assert_eq!(page.validate(), Ok(()));

        let mut b = [0; account_len::ORDER_PAGE];
        assert_eq!(page.encode(&mut b), Ok(account_len::ORDER_PAGE));
        assert_eq!(OrderPageAccount::decode(&b), Ok(page));

        // Every slot is the same width and starts with its kind byte.
        assert_eq!(slot_at(&b, 0)[0], ORDER_KIND_SINGLE);
        assert_eq!(slot_at(&b, 1)[0], ORDER_KIND_PORTFOLIO);
        assert_eq!(slot_at(&b, 2)[0], ORDER_KIND_EMPTY);
        // A single-Egg slot pads its unused tail with canonical zeros, and a
        // padding slot is zero end to end.
        assert!(slot_at(&b, 0)[1 + ORDER_RECORD_BYTES..]
            .iter()
            .all(|x| *x == 0));
        assert!(slot_at(&b, 2).iter().all(|x| *x == 0));
        assert_eq!(
            slot_at(&b, 1).len(),
            1 + PORTFOLIO_RECORD_BYTES,
            "a portfolio body fills its slot exactly"
        );

        // The field offsets published in `docs/implementation/SOLANA_LAYOUT.md`
        // are pinned here, in slot-local coordinates, so the byte tables and the
        // codec cannot drift apart.
        let o = slot_at(&b, 0);
        let single = order(3);
        assert_eq!(&o[1..33], &single.owner.0);
        assert_eq!(&o[33..65], &single.order_id.0);
        assert_eq!(o[65], single.outcome);
        assert_eq!(o[66], single.side);
        assert_eq!(&o[67..75], &single.quantity.to_le_bytes());
        assert_eq!(&o[75..83], &single.limit.to_le_bytes());
        assert_eq!(&o[83..91], &single.minimum_fill.to_le_bytes());
        assert_eq!(o[91], single.flags);
        assert_eq!(&o[92..100], &single.generation.to_le_bytes());
        assert!(o[100..].iter().all(|x| *x == 0));

        let p = slot_at(&b, 1);
        let expected = portfolio(9);
        assert_eq!(&p[1..33], &expected.owner.0);
        assert_eq!(&p[33..65], &expected.order_id.0);
        assert_eq!(p[65], expected.side);
        assert_eq!(p[66], expected.active_len);
        assert_eq!(p[67], expected.flags);
        assert_eq!(&p[68..76], &expected.coefficients[0].to_le_bytes());
        assert_eq!(&p[76..84], &expected.coefficients[1].to_le_bytes());
        assert_eq!(
            &p[188..196],
            &expected.coefficients[MAX_OUTCOMES - 1].to_le_bytes()
        );
        assert_eq!(&p[196..204], &expected.lots.to_le_bytes());
        assert_eq!(
            &p[204..212],
            &expected.limit_collateral_per_lot.to_le_bytes()
        );
        assert_eq!(&p[212..220], &expected.minimum_fill_lots.to_le_bytes());
        assert_eq!(&p[220..228], &expected.generation.to_le_bytes());

        // The order-id chain is one chain across both families.
        assert_eq!(page.orders[0].order_id(), h(3));
        assert_eq!(page.orders[1].order_id(), h(9));
        assert!(page.orders[1].is_portfolio());
        assert!(!page.orders[0].is_portfolio());
        assert_eq!(page.orders[1].owner(), h(21));
        assert_eq!(page.orders[2].order_id(), Hash32::ZERO);

        let mut crossed = page;
        crossed.orders[1] = OrderSlot::Portfolio(portfolio(3));
        crossed.last_order_id = h(3);
        crossed.page_digest = crossed.recomputed_page_digest().unwrap();
        assert_eq!(crossed.validate(), Err(CodecError::NonCanonicalIdentity));
    }
    #[test]
    fn the_page_set_fold_covers_portfolio_record_bytes() {
        let pages = frozen_pages_with_portfolio();
        let order_set = verify_page_set(&pages).unwrap();
        assert_eq!(order_set, pages[0].order_set);
        assert_eq!(pages[0].set_order_count, 19);

        // One coefficient atom is one changed digest.
        let mut mutated = pages;
        let mut coefficients = portfolio(17).coefficients;
        coefficients[1] = 2;
        mutated[1].orders[0] = OrderSlot::Portfolio(PortfolioRecord {
            coefficients,
            ..portfolio(17)
        });
        assert_eq!(mutated[1].validate(), Err(CodecError::MismatchedBinding));
        // Repairing the page's own digest does not repair the set commitment.
        mutated[1].page_digest = mutated[1].recomputed_page_digest().unwrap();
        assert_eq!(mutated[1].validate(), Ok(()));
        assert_eq!(
            verify_page_set(&mutated),
            Err(CodecError::MismatchedBinding)
        );

        // So is one changed cash bound, and one changed lot count.
        let mut rebound = pages;
        rebound[1].orders[0] = OrderSlot::Portfolio(PortfolioRecord {
            limit_collateral_per_lot: 9_001,
            ..portfolio(17)
        });
        assert_eq!(rebound[1].validate(), Err(CodecError::MismatchedBinding));

        // Re-typing a slot from portfolio to single-Egg is a digest change too.
        let mut retyped = pages;
        retyped[1].orders[0] = single(17);
        assert_eq!(retyped[1].validate(), Err(CodecError::MismatchedBinding));

        let mut e = frozen_epoch();
        e.first_order_id = pages[0].first_order_id;
        e.last_order_id = pages[1].last_order_id;
        e.order_set = pages[0].order_set;
        assert_eq!(e.binds_page_set(&pages), Ok(()));

        // The epoch owns the market's outcome width; a page cannot.  A
        // portfolio whose active coefficient width exceeds it, and a single-Egg
        // record naming an outcome the market does not have, are both refused
        // by the binding rather than by the page.
        let mut narrow = e;
        narrow.outcome_count = 2;
        assert_eq!(narrow.binds_page_set(&pages), Ok(()));

        let mut wide = pages;
        let mut coefficients = [0; MAX_OUTCOMES];
        coefficients[0] = 3;
        coefficients[2] = 1;
        wide[1].orders[0] = OrderSlot::Portfolio(PortfolioRecord {
            active_len: 3,
            coefficients,
            ..portfolio(17)
        });
        wide[1].page_digest = wide[1].recomputed_page_digest().unwrap();
        freeze_set(&mut wide);
        let mut wide_epoch = e;
        wide_epoch.order_set = wide[0].order_set;
        assert_eq!(wide_epoch.binds_page_set(&wide), Ok(()));
        wide_epoch.outcome_count = 2;
        assert_eq!(
            wide_epoch.binds_page_set(&wide),
            Err(CodecError::MismatchedBinding)
        );

        // The same binding governs the single-Egg family it already held.
        let mut off_market = pages;
        off_market[1].orders[1] = OrderSlot::Single(OrderRecord {
            outcome: 5,
            ..order(18)
        });
        off_market[1].page_digest = off_market[1].recomputed_page_digest().unwrap();
        freeze_set(&mut off_market);
        let mut narrow_epoch = e;
        narrow_epoch.order_set = off_market[0].order_set;
        assert_eq!(narrow_epoch.binds_page_set(&off_market), Ok(()));
        narrow_epoch.outcome_count = 5;
        assert_eq!(
            narrow_epoch.binds_page_set(&off_market),
            Err(CodecError::MismatchedBinding)
        );
    }
    #[test]
    fn the_streamed_page_digest_equals_the_public_helper() {
        let mut page = build_page(0, 1, &[3, 9], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(portfolio(9));
        reseal(&mut page);
        let mut b = [0; account_len::ORDER_PAGE];
        page.encode(&mut b).unwrap();
        assert_eq!(PAGE_HEADER_BYTES, 235);
        assert_eq!(
            page.page_digest,
            canonical_page_digest(
                page.market,
                page.epoch,
                page.page_index,
                page.order_count,
                &b[PAGE_HEADER_BYTES..],
            )
        );
    }
    #[test]
    fn portfolio_records_refuse_bad_widths_coefficients_and_lot_bounds() {
        let base = portfolio(9);
        assert_eq!(base.validate(), Ok(()));

        let mut zero_width = base;
        zero_width.active_len = 0;
        assert_eq!(zero_width.validate(), Err(CodecError::InvalidCount));

        let mut over_width = base;
        over_width.active_len = MAX_OUTCOMES as u8 + 1;
        assert_eq!(over_width.validate(), Err(CodecError::InvalidCount));

        // A coefficient outside the declared width is padding, and padding is
        // zero.  This is the "coefficient count disagrees with the declared
        // outcome width" refusal.
        let mut leaked = base;
        leaked.coefficients[2] = 1;
        assert_eq!(leaked.validate(), Err(CodecError::NonCanonicalPadding));
        let mut widened = leaked;
        widened.active_len = 3;
        assert_eq!(widened.validate(), Ok(()));

        let mut empty_demand = base;
        empty_demand.coefficients = [0; MAX_OUTCOMES];
        assert_eq!(empty_demand.validate(), Err(CodecError::ZeroValue));

        let mut no_lots = base;
        no_lots.lots = 0;
        assert_eq!(no_lots.validate(), Err(CodecError::ZeroValue));

        let mut over_fill = base;
        over_fill.minimum_fill_lots = base.lots + 1;
        assert_eq!(over_fill.validate(), Err(CodecError::InvalidEnum));

        let mut partial_aon = base;
        partial_aon.flags = 1;
        assert_eq!(partial_aon.validate(), Err(CodecError::InvalidEnum));
        partial_aon.minimum_fill_lots = partial_aon.lots;
        assert_eq!(partial_aon.validate(), Ok(()));

        let mut reserved_flag = base;
        reserved_flag.flags = 2;
        assert_eq!(reserved_flag.validate(), Err(CodecError::InvalidEnum));

        let mut bad_side = base;
        bad_side.side = 2;
        assert_eq!(bad_side.validate(), Err(CodecError::InvalidEnum));

        let mut anonymous = base;
        anonymous.owner = Hash32::ZERO;
        assert_eq!(anonymous.validate(), Err(CodecError::ZeroIdentity));
        let mut unnamed = base;
        unnamed.order_id = Hash32::ZERO;
        assert_eq!(unnamed.validate(), Err(CodecError::ZeroIdentity));

        // Boundary values that must be admitted: the full outcome width, a
        // single active outcome, and the largest representable lot count.
        let mut widest = base;
        widest.active_len = MAX_OUTCOMES as u8;
        widest.coefficients = [1; MAX_OUTCOMES];
        assert_eq!(widest.validate(), Ok(()));
        let mut narrowest = base;
        narrowest.active_len = 1;
        narrowest.coefficients = [0; MAX_OUTCOMES];
        narrowest.coefficients[0] = 1;
        assert_eq!(narrowest.validate(), Ok(()));
        let mut many_lots = narrowest;
        many_lots.lots = u64::MAX;
        assert_eq!(many_lots.validate(), Ok(()));

        // A page carrying a refused record cannot be encoded at all.
        let mut page = build_page(0, 1, &[3, 9], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(no_lots);
        page.page_digest = page.recomputed_page_digest().unwrap();
        let mut b = [0; account_len::ORDER_PAGE];
        assert_eq!(page.encode(&mut b), Err(CodecError::ZeroValue));
    }
    #[test]
    fn portfolio_bounds_are_checked_against_the_frozen_price_scale() {
        let g = grid();
        let mut page = build_page(0, 1, &[3, 9], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(portfolio(9));
        reseal(&mut page);
        let mut b = [0; account_len::ORDER_PAGE];
        page.encode(&mut b).unwrap();

        // 9,000 is not a tick, and a portfolio cash bound is deliberately not
        // looked up in the tick vector: it is a per-lot collateral in
        // complete-set units, not a per-outcome limit price.
        assert_eq!(g.tick_of(9_000), Err(CodecError::InvalidTick));
        assert_eq!(OrderPageAccount::decode_on_grid(&b, &g), Ok(page));

        // What the grid does contribute is the frozen scale.  A per-lot value
        // that cannot be represented could never be classified.
        let mut huge_value = portfolio(9);
        huge_value.coefficients = [0; MAX_OUTCOMES];
        huge_value.coefficients[0] = u64::MAX;
        huge_value.active_len = 1;
        huge_value.lots = u64::MAX;
        huge_value.minimum_fill_lots = 0;
        assert_eq!(huge_value.validate(), Ok(()));
        assert_eq!(
            huge_value.validate_on_scale(g.price_scale),
            Err(CodecError::ArithmeticOverflow)
        );

        let mut huge_bound = portfolio(9);
        huge_bound.coefficients = [0; MAX_OUTCOMES];
        huge_bound.coefficients[0] = 1;
        huge_bound.active_len = 1;
        huge_bound.lots = u64::MAX;
        huge_bound.minimum_fill_lots = 0;
        huge_bound.limit_collateral_per_lot = u64::MAX;
        assert_eq!(huge_bound.validate(), Ok(()));
        assert_eq!(
            huge_bound.validate_on_scale(g.price_scale),
            Err(CodecError::ArithmeticOverflow)
        );

        let mut overflowing = page;
        overflowing.orders[1] = OrderSlot::Portfolio(huge_bound);
        overflowing.page_digest = overflowing.recomputed_page_digest().unwrap();
        let mut b = [0; account_len::ORDER_PAGE];
        overflowing.encode(&mut b).unwrap();
        assert_eq!(OrderPageAccount::decode(&b), Ok(overflowing));
        assert_eq!(
            OrderPageAccount::decode_on_grid(&b, &g),
            Err(CodecError::ArithmeticOverflow)
        );

        // A scale of zero or above the simplex bound is not a scale.
        assert_eq!(
            portfolio(9).validate_on_scale(0),
            Err(CodecError::InvalidPriceGrid)
        );
        assert_eq!(
            portfolio(9).validate_on_scale(MAX_PRICE_SCALE + 1),
            Err(CodecError::InvalidPriceGrid)
        );
    }
    #[test]
    fn hostile_order_slots_refuse_unknown_kinds_and_nonzero_slot_padding() {
        let mut page = build_page(0, 1, &[3, 9], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(portfolio(9));
        reseal(&mut page);
        let mut clean = [0; account_len::ORDER_PAGE];
        page.encode(&mut clean).unwrap();

        // An unrecognized slot kind is refused like any other discriminator.
        let mut unknown = clean;
        unknown[PAGE_HEADER_BYTES] = 3;
        assert_eq!(
            OrderPageAccount::decode(&unknown),
            Err(CodecError::WrongTag)
        );
        let mut unknown_pad = clean;
        unknown_pad[PAGE_HEADER_BYTES + 2 * ORDER_SLOT_BYTES] = u8::MAX;
        assert_eq!(
            OrderPageAccount::decode(&unknown_pad),
            Err(CodecError::WrongTag)
        );

        // The unused tail of a single-Egg slot is canonical zero.
        let mut stuffed = clean;
        stuffed[PAGE_HEADER_BYTES + 1 + ORDER_RECORD_BYTES] = 1;
        assert_eq!(
            OrderPageAccount::decode(&stuffed),
            Err(CodecError::NonCanonicalPadding)
        );
        let mut stuffed_end = clean;
        stuffed_end[PAGE_HEADER_BYTES + ORDER_SLOT_BYTES - 1] = 1;
        assert_eq!(
            OrderPageAccount::decode(&stuffed_end),
            Err(CodecError::NonCanonicalPadding)
        );

        // So is every byte of a padding slot.
        let mut dirty_pad = clean;
        dirty_pad[PAGE_HEADER_BYTES + 2 * ORDER_SLOT_BYTES + 5] = 1;
        assert_eq!(
            OrderPageAccount::decode(&dirty_pad),
            Err(CodecError::NonCanonicalPadding)
        );

        // An all-zero record smuggled into a padding slot under a real kind
        // byte is not padding either.
        let mut typed_pad = clean;
        typed_pad[PAGE_HEADER_BYTES + 2 * ORDER_SLOT_BYTES] = ORDER_KIND_SINGLE;
        assert_eq!(
            OrderPageAccount::decode(&typed_pad),
            Err(CodecError::NonCanonicalPadding)
        );
        let mut typed_portfolio_pad = clean;
        typed_portfolio_pad[PAGE_HEADER_BYTES + 2 * ORDER_SLOT_BYTES] = ORDER_KIND_PORTFOLIO;
        assert_eq!(
            OrderPageAccount::decode(&typed_portfolio_pad),
            Err(CodecError::NonCanonicalPadding)
        );

        // An empty slot below `order_count` is a missing order, not padding.
        let mut hollow = page;
        hollow.orders[1] = OrderSlot::Empty;
        hollow.last_order_id = Hash32::ZERO;
        hollow.page_digest = hollow.recomputed_page_digest().unwrap();
        assert_eq!(hollow.validate(), Err(CodecError::ZeroIdentity));

        // Exact length still governs: the slot array cannot be short or long.
        assert_eq!(
            OrderPageAccount::decode(&clean[..clean.len() - 1]),
            Err(CodecError::Truncated)
        );
        let mut long = [0; account_len::ORDER_PAGE + 1];
        long[..clean.len()].copy_from_slice(&clean);
        assert_eq!(
            OrderPageAccount::decode(&long),
            Err(CodecError::TrailingBytes)
        );
    }
    #[test]
    fn a_page_set_refuses_more_portfolios_than_the_relation_admits() {
        // `MAX_PORTFOLIO_ORDERS` portfolios on one page is admitted.
        let ids: [u8; MAX_ORDERS_PER_PAGE] =
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let mut full = build_page(0, 2, &ids, Hash32::ZERO);
        let mut i = 0;
        while i < MAX_PORTFOLIO_ORDERS {
            full.orders[i] = OrderSlot::Portfolio(portfolio(ids[i]));
            i += 1;
        }
        reseal(&mut full);
        assert_eq!(full.validate(), Ok(()));

        // One more on the same page is refused by the page itself.
        let mut over = full;
        over.orders[MAX_PORTFOLIO_ORDERS] =
            OrderSlot::Portfolio(portfolio(ids[MAX_PORTFOLIO_ORDERS]));
        over.page_digest = over.recomputed_page_digest().unwrap();
        assert_eq!(over.validate(), Err(CodecError::InvalidCount));

        // One more on the *next* page is refused only by the set, which is
        // exactly the cross-page bound `verify_page_set` exists to check.
        let mut tail = build_page(1, 2, &[17, 18, 19], full.last_order_id);
        tail.orders[0] = OrderSlot::Portfolio(portfolio(17));
        reseal(&mut tail);
        assert_eq!(tail.validate(), Ok(()));
        let mut set = [full, tail];
        freeze_set(&mut set);
        assert_eq!(set[0].validate(), Ok(()));
        assert_eq!(set[1].validate(), Ok(()));
        assert_eq!(verify_page_set(&set), Err(CodecError::InvalidCount));
    }
    #[test]
    fn candidate_identity_binds_only_the_free_coordinates() {
        let c = candidate();
        assert_eq!(c.validate(), Ok(()));

        let mut repriced = c;
        repriced.prices[0] = 9_999;
        repriced.prices[1] = 1;
        assert_ne!(repriced.recomputed_candidate_digest().unwrap(), c.candidate);

        let mut churned = c;
        churned.virtual_split = 6;
        churned.churn = 6;
        assert_ne!(churned.recomputed_candidate_digest().unwrap(), c.candidate);

        let mut masked = c;
        masked.honored_aon_mask = 1;
        assert_ne!(masked.recomputed_candidate_digest().unwrap(), c.candidate);

        // Score, status, slot, and bump are outside the identity: they are
        // claims and lifecycle, not coordinates.
        let mut rescored = c;
        rescored.weighted_direct_volume = i128::MIN;
        rescored.limit_surplus_price_units = u128::MAX;
        rescored.distinct_owners = 1;
        rescored.status = CANDIDATE_STATUS_SUPERSEDED;
        rescored.submitted_slot = 0;
        rescored.stored_bump = 255;
        assert_eq!(rescored.recomputed_candidate_digest().unwrap(), c.candidate);
        assert_eq!(rescored.validate(), Ok(()));
        let mut b = [0; account_len::CANDIDATE];
        rescored.encode(&mut b).unwrap();
        assert_eq!(CandidateRecord::decode(&b), Ok(rescored));
    }
    #[test]
    fn candidate_refuses_mask_leaks_double_churn_and_inconsistent_score() {
        let c = candidate();
        let mut leak = c;
        leak.honored_aon_mask = 1 << c.order_len;
        leak.candidate = leak.recomputed_candidate_digest().unwrap();
        assert_eq!(leak.validate(), Err(CodecError::NonCanonicalPadding));

        let mut both = c;
        both.virtual_merge = 1;
        both.churn = 6;
        both.candidate = both.recomputed_candidate_digest().unwrap();
        assert_eq!(both.validate(), Err(CodecError::InvalidEnum));

        let mut miscounted = c;
        miscounted.churn = 4;
        assert_eq!(miscounted.validate(), Err(CodecError::MismatchedBinding));

        let mut oversized = c;
        oversized.order_len = MAX_EPOCH_ORDERS as u8 + 1;
        oversized.candidate = oversized.recomputed_candidate_digest().unwrap();
        assert_eq!(oversized.validate(), Err(CodecError::InvalidCount));

        let mut statused = c;
        statused.status = CANDIDATE_STATUS_SUPERSEDED + 1;
        assert_eq!(statused.validate(), Err(CodecError::InvalidEnum));

        let mut forged = c;
        forged.prices[0] = 1;
        assert_eq!(forged.validate(), Err(CodecError::NonCanonicalIdentity));
    }
    #[test]
    fn candidate_binds_the_frozen_epoch_simplex() {
        let c = candidate();
        let e = frozen_epoch();
        assert_eq!(c.binds_epoch(&e), Ok(()));
        assert_eq!(
            c.binds_epoch(&epoch_account()),
            Err(CodecError::MismatchedBinding)
        );

        let mut off_simplex = c;
        off_simplex.prices[0] = 9_999;
        off_simplex.candidate = off_simplex.recomputed_candidate_digest().unwrap();
        assert_eq!(
            off_simplex.binds_epoch(&e),
            Err(CodecError::MismatchedBinding)
        );

        let mut over_scale = c;
        over_scale.prices[0] = 10_001;
        over_scale.candidate = over_scale.recomputed_candidate_digest().unwrap();
        assert_eq!(
            over_scale.binds_epoch(&e),
            Err(CodecError::InvalidPriceGrid)
        );

        let mut wrong_len = c;
        wrong_len.order_len = 18;
        wrong_len.candidate = wrong_len.recomputed_candidate_digest().unwrap();
        assert_eq!(
            wrong_len.binds_epoch(&e),
            Err(CodecError::MismatchedBinding)
        );
    }
    #[test]
    fn final_pot_is_pot_phase_only_and_a_closed_pot_is_empty() {
        let p = final_pot();
        assert_eq!(p.validate(), Ok(()));
        assert_eq!(p.binds_candidate(&candidate()), Ok(()));

        let mut wrong = p;
        wrong.candidate = h(60);
        assert_eq!(
            wrong.binds_candidate(&candidate()),
            Err(CodecError::MismatchedBinding)
        );

        let mut closed = p;
        closed.phase = POT_PHASE_CLOSED;
        assert_eq!(closed.validate(), Err(CodecError::AggregateClosureMismatch));
        closed.pot_internal[0] = 0;
        assert_eq!(closed.validate(), Err(CodecError::AggregateClosureMismatch));
        closed.pot_cash_price_units = 0;
        assert_eq!(closed.validate(), Err(CodecError::AggregateClosureMismatch));
        closed.rounding_pot_price_units = 0;
        assert_eq!(closed.validate(), Ok(()));

        let mut padded = p;
        padded.outcome_count = 2;
        assert_eq!(padded.validate(), Ok(()));
        padded.pot_internal[4] = 1;
        assert_eq!(padded.validate(), Err(CodecError::NonCanonicalPadding));

        let mut phased = p;
        phased.phase = POT_PHASE_CLOSED + 1;
        assert_eq!(phased.validate(), Err(CodecError::InvalidEnum));
    }
    #[test]
    fn settlement_receipt_leg_shapes_are_exclusive() {
        let direct = receipt();
        assert_eq!(direct.validate(), Ok(()));
        assert_eq!(direct.binds_candidate(&candidate()), Ok(()));

        let mut aliased = direct;
        aliased.sell_order_id = aliased.buy_order_id;
        assert_eq!(aliased.validate(), Err(CodecError::NonCanonicalIdentity));

        let mut split = direct;
        split.leg_kind = RECEIPT_LEG_SPLIT;
        assert_eq!(split.validate(), Err(CodecError::NonCanonicalPadding));
        split.sell_order_id = Hash32::ZERO;
        assert_eq!(split.validate(), Ok(()));

        let mut merge = direct;
        merge.leg_kind = RECEIPT_LEG_MERGE;
        assert_eq!(merge.validate(), Err(CodecError::NonCanonicalPadding));
        merge.buy_order_id = Hash32::ZERO;
        assert_eq!(merge.validate(), Ok(()));

        let mut unknown = direct;
        unknown.leg_kind = RECEIPT_LEG_MERGE + 1;
        assert_eq!(unknown.validate(), Err(CodecError::InvalidEnum));

        let mut legless = direct;
        legless.buy_order_id = Hash32::ZERO;
        assert_eq!(legless.validate(), Err(CodecError::ZeroIdentity));
    }
    #[test]
    fn settlement_receipt_refuses_inexact_consideration_and_bad_consumption() {
        let base = receipt();
        let mut repriced = base;
        repriced.consideration_price_units += 1;
        assert_eq!(repriced.validate(), Err(CodecError::InvalidConsideration));

        let mut zero = base;
        zero.quantity = 0;
        assert_eq!(zero.validate(), Err(CodecError::ZeroValue));

        let mut over = base;
        over.settled_quantity = 4;
        assert_eq!(over.validate(), Err(CodecError::InvalidCount));

        let mut exhausted = base;
        exhausted.settled_quantity = 3;
        assert_eq!(exhausted.validate(), Err(CodecError::InvalidEnum));
        exhausted.consumed_flags = RECEIPT_FLAG_SLICE_EXHAUSTED;
        assert_eq!(exhausted.validate(), Ok(()));
        exhausted.consumed_flags |= RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SELL_CONSUMED;
        assert_eq!(exhausted.validate(), Ok(()));
        exhausted.consumed_flags = 8;
        assert_eq!(exhausted.validate(), Err(CodecError::InvalidEnum));

        // Boundary: the widest possible exact product still validates.
        let mut widest = base;
        widest.quantity = u64::MAX;
        widest.price = u64::MAX;
        widest.consideration_price_units = u128::from(u64::MAX) * u128::from(u64::MAX);
        assert_eq!(widest.validate(), Ok(()));
        widest.consideration_price_units -= 1;
        assert_eq!(widest.validate(), Err(CodecError::InvalidConsideration));

        let mut mispriced = base;
        mispriced.price = 5_000;
        mispriced.consideration_price_units = 15_000;
        assert_eq!(
            mispriced.binds_candidate(&candidate()),
            Err(CodecError::MismatchedBinding)
        );

        let mut wrong_outcome = base;
        wrong_outcome.outcome = 1;
        wrong_outcome.price = 0;
        wrong_outcome.consideration_price_units = 0;
        assert_eq!(wrong_outcome.binds_candidate(&candidate()), Ok(()));
        wrong_outcome.outcome = MAX_OUTCOMES as u8;
        assert_eq!(wrong_outcome.validate(), Err(CodecError::InvalidCount));
    }
    #[test]
    fn resolution_keeps_unresolved_fields_zero_and_binds_the_payout_index() {
        let r = resolution();
        assert_eq!(r.validate(), Ok(()));
        assert!(r.is_resolved());
        assert_eq!(r.binds_terms(&terms()), Ok(()));

        let mut unresolved = r;
        unresolved.payout_index = PAYOUT_INDEX_UNRESOLVED;
        assert_eq!(unresolved.validate(), Err(CodecError::NonCanonicalPadding));
        unresolved.window = Hash32::ZERO;
        unresolved.feed_cursor = 0;
        unresolved.sealed_end_bucket_exclusive = 0;
        unresolved.repair_generation = 0;
        unresolved.resolved_slot = 0;
        assert_eq!(unresolved.validate(), Ok(()));
        assert!(!unresolved.is_resolved());
        assert_eq!(unresolved.binds_terms(&terms()), Ok(()));

        let mut out_of_set = r;
        out_of_set.payout_index = 3;
        assert_eq!(out_of_set.validate(), Ok(()));
        assert_eq!(
            out_of_set.binds_terms(&terms()),
            Err(CodecError::InvalidCount)
        );

        let mut early = r;
        early.sealed_end_bucket_exclusive = 129;
        assert_eq!(
            early.binds_terms(&terms()),
            Err(CodecError::MismatchedBinding)
        );

        let mut unsealed = r;
        unsealed.window = Hash32::ZERO;
        assert_eq!(unsealed.validate(), Err(CodecError::ZeroIdentity));

        let mut wrong_terms = r;
        wrong_terms.terms = h(61);
        assert_eq!(
            wrong_terms.binds_terms(&terms()),
            Err(CodecError::MismatchedBinding)
        );
    }
    #[test]
    fn intent_golden_and_round_trip() {
        let i = Intent::Split {
            market: h(1),
            owner: h(2),
            quantity: 9,
        };
        let mut b = [0; MAX_INTENT_BYTES];
        let n = i.encode(&mut b).unwrap();
        assert_eq!(n, 74);
        assert_eq!(&b[..2], [SPLIT_TAG, INTENT_VERSION]);
        assert_eq!(Intent::decode(&b[..n]), Ok(i));
        assert_eq!(Intent::decode(&b[..n - 1]), Err(CodecError::Truncated));
    }
    #[test]
    fn intent_zero_identity_refuses() {
        let i = Intent::Split {
            market: Hash32::ZERO,
            owner: h(2),
            quantity: 1,
        };
        let mut b = [0; MAX_INTENT_BYTES];
        assert_eq!(i.encode(&mut b), Err(CodecError::ZeroIdentity));
        let mut raw = [0; 74];
        raw[0] = SPLIT_TAG;
        raw[1] = INTENT_VERSION;
        raw[66..74].copy_from_slice(&1u64.to_le_bytes());
        assert_eq!(Intent::decode(&raw), Err(CodecError::ZeroIdentity));
    }
    #[test]
    fn zero_identity_is_reserved() {
        assert_eq!(Hash32::new([0; HASH_BYTES]), Err(CodecError::ZeroIdentity));
        assert_eq!(Hash32::new([1; HASH_BYTES]), Ok(h(1)));
    }
    #[test]
    fn canonical_profile_hash_requires_the_exact_parent_preimage_length() {
        // The parent Profile preimage of RESOLUTION_EVIDENCE_PLAN.md 3.2 is
        // exactly 64 bytes: magic, parent schema/flags, subfield tag/schema,
        // the 32-byte collateral digest, and 16 zero reserved bytes.
        let mut parent = [0u8; PROFILE_PARENT_BYTES];
        parent[..8].copy_from_slice(b"DCPROF1\0");
        parent[8..10].copy_from_slice(&1u16.to_le_bytes());
        parent[12..14].copy_from_slice(&1u16.to_le_bytes());
        parent[14..16].copy_from_slice(&1u16.to_le_bytes());
        parent[16..48].copy_from_slice(&[0xab; 32]);
        let exact = canonical_profile_hash(&parent).expect("exact parent preimage");
        assert_ne!(exact, Hash32::ZERO);

        // A variable-length input under one fixed domain string is not
        // prefix-free, so every other length is a refusal rather than a hash.
        assert_eq!(
            canonical_profile_hash(&parent[..PROFILE_PARENT_BYTES - 1]),
            Err(CodecError::Truncated)
        );
        assert_eq!(canonical_profile_hash(b""), Err(CodecError::Truncated));
        assert_eq!(
            canonical_profile_hash(b"fixture-profile"),
            Err(CodecError::Truncated)
        );
        let mut extended = [0u8; PROFILE_PARENT_BYTES + 1];
        extended[..PROFILE_PARENT_BYTES].copy_from_slice(&parent);
        assert_eq!(
            canonical_profile_hash(&extended),
            Err(CodecError::TrailingBytes)
        );

        // The frozen domain string and algorithm are unchanged: the accepted
        // length still hashes exactly SHA-256(domain || preimage).
        assert_eq!(exact, digest(b"dragons-clutch/profile/v1", &[&parent]));
    }
    #[test]
    fn sha256_known_vector() {
        let got = digest(b"", &[]);
        assert_eq!(
            got.0,
            [
                0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
                0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
                0x78, 0x52, 0xb8, 0x55
            ]
        );
    }

    /* --- streaming decoder equivalence -------------------------------------
     *
     * The buffered decoders above stay the golden reference.  Every fixture
     * below is decided twice — once through `OrderPageAccount::decode` and once
     * through `stream::verify_page` — and the two verdicts must be the same
     * value: the same page, or the identical `CodecError`.
     * ---------------------------------------------------------------------- */

    /// Encode a page **without** validating it.
    ///
    /// `OrderPageAccount::encode` validates first, so it cannot produce the
    /// bytes of a page the codec refuses.  A refusal fixture is exactly what an
    /// equivalence test needs, so this writes the same bytes with no verdict
    /// attached.
    fn encode_page_unchecked(page: &OrderPageAccount) -> [u8; account_len::ORDER_PAGE] {
        let mut out = [0; account_len::ORDER_PAGE];
        let mut w = Writer::new(&mut out);
        put_header(&mut w, ORDER_PAGE_TAG, account_version::ORDER_PAGE).unwrap();
        w.hash(page.market).unwrap();
        w.hash(page.epoch).unwrap();
        w.hash(page.order_set).unwrap();
        w.hash(page.page_digest).unwrap();
        w.hash(page.first_order_id).unwrap();
        w.hash(page.last_order_id).unwrap();
        w.hash(page.prev_page_last_order_id).unwrap();
        w.u16(page.page_index).unwrap();
        w.u16(page.page_count).unwrap();
        w.u16(page.set_order_count).unwrap();
        w.u8(page.order_count).unwrap();
        w.u8(page.frozen).unwrap();
        w.u8(page.stored_bump).unwrap();
        let mut i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            encode_slot(&mut w, page.orders[i]).unwrap();
            i += 1;
        }
        assert_eq!(w.at, account_len::ORDER_PAGE);
        out
    }
    /// The buffered verdict, projected onto the header the streaming decoder
    /// returns.
    fn buffered_page(bytes: &[u8]) -> Result<stream::OrderPageHeader> {
        OrderPageAccount::decode(bytes).map(|p| stream::OrderPageHeader::of_page(&p))
    }
    fn agrees(label: &str, bytes: &[u8]) {
        assert_eq!(
            stream::verify_page(bytes),
            buffered_page(bytes),
            "page verdict diverged: {label}"
        );
    }
    /// The buffered page-set verdict over raw page bytes: decode every page in
    /// index order, then close the set.  This is the composition an on-chain
    /// caller would have written if the buffered decoder fitted a call frame.
    fn buffered_set(bufs: &[&[u8]]) -> Result<Hash32> {
        if bufs.is_empty() {
            return verify_page_set(&[]);
        }
        assert!(bufs.len() <= MAX_ORDER_PAGES);
        let first = OrderPageAccount::decode(bufs[0])?;
        let mut pages = [first; MAX_ORDER_PAGES];
        let mut i = 1;
        while i < bufs.len() {
            pages[i] = OrderPageAccount::decode(bufs[i])?;
            i += 1;
        }
        verify_page_set(&pages[..bufs.len()])
    }
    fn set_agrees(label: &str, bufs: &[&[u8]]) {
        assert_eq!(
            stream::verify_page_set(bufs),
            buffered_set(bufs),
            "page-set verdict diverged: {label}"
        );
    }
    #[test]
    fn streaming_page_verdicts_match_the_buffered_decoder() {
        // Accepted shapes.
        let mixed = {
            let mut p = build_page(0, 1, &[3, 9], Hash32::ZERO);
            p.orders[1] = OrderSlot::Portfolio(portfolio(9));
            reseal(&mut p);
            p
        };
        let ids: [u8; MAX_ORDERS_PER_PAGE] =
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let full = build_page(0, 2, &ids, Hash32::ZERO);
        let empty = build_page(0, 1, &[], Hash32::ZERO);
        let frozen = frozen_pages();
        let with_portfolio = frozen_pages_with_portfolio();
        let accepted = [
            ("single-egg page", build_page(0, 1, &[3, 9], Hash32::ZERO)),
            ("mixed families", mixed),
            ("dense page", full),
            ("empty open page", empty),
            ("frozen head", frozen[0]),
            ("frozen tail", frozen[1]),
            ("frozen head, portfolio set", with_portfolio[0]),
            ("frozen tail, portfolio set", with_portfolio[1]),
        ];
        let mut i = 0;
        while i < accepted.len() {
            let (label, page) = accepted[i];
            let bytes = encode_page_unchecked(&page);
            assert_eq!(
                stream::verify_page(&bytes),
                Ok(stream::OrderPageHeader::of_page(&page))
            );
            agrees(label, &bytes);
            i += 1;
        }

        // Refused shapes, built as bytes so both decoders see the same input.
        let base = build_page(0, 1, &[3, 9], Hash32::ZERO);
        let refused: [(&str, OrderPageAccount); 16] = [
            (
                "zero market",
                OrderPageAccount {
                    market: Hash32::ZERO,
                    ..base
                },
            ),
            (
                "zero epoch",
                OrderPageAccount {
                    epoch: Hash32::ZERO,
                    ..base
                },
            ),
            (
                "freeze flag out of range",
                OrderPageAccount { frozen: 2, ..base },
            ),
            (
                "no pages",
                OrderPageAccount {
                    page_count: 0,
                    ..base
                },
            ),
            (
                "too many pages",
                OrderPageAccount {
                    page_count: (MAX_ORDER_PAGES + 1) as u16,
                    ..base
                },
            ),
            (
                "index past count",
                OrderPageAccount {
                    page_index: 1,
                    ..base
                },
            ),
            (
                "order count past width",
                OrderPageAccount {
                    order_count: (MAX_ORDERS_PER_PAGE + 1) as u8,
                    ..base
                },
            ),
            (
                "stale range",
                OrderPageAccount {
                    last_order_id: h(8),
                    ..base
                },
            ),
            (
                "page zero links to a predecessor",
                OrderPageAccount {
                    prev_page_last_order_id: h(2),
                    ..base
                },
            ),
            (
                "open page commits a count",
                OrderPageAccount {
                    set_order_count: 2,
                    ..base
                },
            ),
            (
                "open page commits a set digest",
                OrderPageAccount {
                    order_set: h(9),
                    ..base
                },
            ),
            ("record above the order count", {
                // A record smuggled above `order_count` is padding that is not.
                let mut p = base;
                p.orders[5] = single(11);
                p.page_digest = p.recomputed_page_digest().unwrap();
                p
            }),
            ("hole below the order count", {
                // An empty slot below `order_count` is a missing order.
                let mut p = base;
                p.orders[1] = OrderSlot::Empty;
                p.last_order_id = Hash32::ZERO;
                p.page_digest = p.recomputed_page_digest().unwrap();
                p
            }),
            ("duplicate order id", {
                let mut p = base;
                p.orders[1] = single(3);
                p.last_order_id = h(3);
                p.page_digest = p.recomputed_page_digest().unwrap();
                p
            }),
            ("descending order ids", {
                let mut p = build_page(0, 1, &[9, 3], Hash32::ZERO);
                p.page_digest = p.recomputed_page_digest().unwrap();
                p
            }),
            ("record mutated without repairing the page digest", {
                let mut p = base;
                p.orders[0] = OrderSlot::Single(OrderRecord {
                    quantity: 11,
                    ..order(3)
                });
                p
            }),
        ];
        let mut i = 0;
        while i < refused.len() {
            let (label, page) = refused[i];
            let bytes = encode_page_unchecked(&page);
            assert!(
                stream::verify_page(&bytes).is_err(),
                "fixture is supposed to refuse: {label}"
            );
            agrees(label, &bytes);
            i += 1;
        }

        // Nine portfolio records on one page: the per-page bound.
        let mut over = build_page(0, 2, &ids, Hash32::ZERO);
        let mut j = 0;
        while j < MAX_PORTFOLIO_ORDERS + 1 {
            over.orders[j] = OrderSlot::Portfolio(portfolio(ids[j]));
            j += 1;
        }
        reseal(&mut over);
        let over_bytes = encode_page_unchecked(&over);
        assert_eq!(
            stream::verify_page(&over_bytes),
            Err(CodecError::InvalidCount)
        );
        agrees("nine portfolios on one page", &over_bytes);

        // A frozen page set's density and closure rules.
        let sparse = {
            let mut set = [full, build_page(1, 2, &[17], full.last_order_id)];
            freeze_set(&mut set);
            let mut s = set[0];
            s.order_count = 15;
            s.last_order_id = h(15);
            s.orders[15] = OrderSlot::Empty;
            s.page_digest = s.recomputed_page_digest().unwrap();
            s
        };
        let sparse_bytes = encode_page_unchecked(&sparse);
        agrees("sparse non-final frozen page", &sparse_bytes);
        let mut hollow_frozen = frozen[1];
        hollow_frozen.order_count = 0;
        agrees(
            "frozen page with no records",
            &encode_page_unchecked(&hollow_frozen),
        );
        let mut unset = frozen[0];
        unset.order_set = Hash32::ZERO;
        agrees(
            "frozen page with no set digest",
            &encode_page_unchecked(&unset),
        );
        let mut miscounted = frozen[0];
        miscounted.set_order_count = MAX_EPOCH_ORDERS as u16 + 1;
        agrees(
            "set count past the book",
            &encode_page_unchecked(&miscounted),
        );
        let mut short_set = frozen[1];
        short_set.set_order_count = 18;
        agrees(
            "final page does not close the count",
            &encode_page_unchecked(&short_set),
        );
        let mut unlinked = frozen[1];
        unlinked.prev_page_last_order_id = Hash32::ZERO;
        agrees(
            "later page with no predecessor",
            &encode_page_unchecked(&unlinked),
        );
        let mut overlapping = frozen[1];
        overlapping.prev_page_last_order_id = h(30);
        agrees(
            "later page opening below its predecessor",
            &encode_page_unchecked(&overlapping),
        );
    }
    #[test]
    fn streaming_page_verdicts_match_the_buffered_decoder_on_hostile_bytes() {
        let mut page = build_page(0, 1, &[3, 9], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(portfolio(9));
        reseal(&mut page);
        let clean = encode_page_unchecked(&page);
        agrees("clean", &clean);

        // Header framing.
        agrees("truncated", &clean[..clean.len() - 1]);
        agrees("empty input", &[]);
        let mut long = [0; account_len::ORDER_PAGE + 1];
        long[..clean.len()].copy_from_slice(&clean);
        agrees("trailing byte", &long);
        let mut wrong_tag = clean;
        wrong_tag[0] = ORDER_PAGE_TAG + 1;
        agrees("wrong tag", &wrong_tag);
        let mut wrong_version = clean;
        wrong_version[1] = account_version::ORDER_PAGE - 1;
        agrees("wrong version", &wrong_version);

        // Slot framing: every byte-level fixture the buffered slot decoder has.
        let mut unknown = clean;
        unknown[PAGE_HEADER_BYTES] = 3;
        agrees("unknown slot kind", &unknown);
        let mut unknown_pad = clean;
        unknown_pad[PAGE_HEADER_BYTES + 2 * ORDER_SLOT_BYTES] = u8::MAX;
        agrees("unknown kind in a padding slot", &unknown_pad);
        let mut stuffed = clean;
        stuffed[PAGE_HEADER_BYTES + 1 + ORDER_RECORD_BYTES] = 1;
        agrees("nonzero single-egg tail", &stuffed);
        let mut stuffed_end = clean;
        stuffed_end[PAGE_HEADER_BYTES + ORDER_SLOT_BYTES - 1] = 1;
        agrees("nonzero slot end", &stuffed_end);
        let mut dirty_pad = clean;
        dirty_pad[PAGE_HEADER_BYTES + 2 * ORDER_SLOT_BYTES + 5] = 1;
        agrees("dirty padding slot", &dirty_pad);
        let mut typed_pad = clean;
        typed_pad[PAGE_HEADER_BYTES + 2 * ORDER_SLOT_BYTES] = ORDER_KIND_SINGLE;
        agrees("all-zero record in a padding slot", &typed_pad);
        let mut typed_portfolio_pad = clean;
        typed_portfolio_pad[PAGE_HEADER_BYTES + 2 * ORDER_SLOT_BYTES] = ORDER_KIND_PORTFOLIO;
        agrees("all-zero portfolio in a padding slot", &typed_portfolio_pad);
        let mut last_slot = clean;
        last_slot[account_len::ORDER_PAGE - 1] = 1;
        agrees("nonzero final byte", &last_slot);

        // A structurally broken slot is refused before any header fault the
        // same bytes also carry: the buffered decoder reads its whole slot
        // array before it validates anything, and so does the streamed one.
        let mut both = clean;
        both[2] = 0; // zero the first byte of `market`
        both[PAGE_HEADER_BYTES] = 3;
        assert_eq!(stream::verify_page(&both), Err(CodecError::WrongTag));
        agrees("bad slot and bad header at once", &both);

        // A page-sized buffer of zeros is not a page.
        agrees("all zero", &[0; account_len::ORDER_PAGE]);
    }
    #[test]
    fn the_streaming_header_reads_235_bytes_and_decides_only_header_facts() {
        let page = build_page(0, 1, &[3, 9], Hash32::ZERO);
        let bytes = encode_page_unchecked(&page);
        assert_eq!(
            stream::ORDER_PAGE_HEADER_BYTES,
            account_len::ORDER_PAGE - MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES
        );
        assert_eq!(stream::ORDER_PAGE_HEADER_BYTES, 235);
        let header = stream::OrderPageHeader::decode(&bytes).unwrap();
        assert_eq!(header, stream::OrderPageHeader::of_page(&page));
        assert_eq!(header.validate_shape(), Ok(()));

        // The header still owns the account's framing: tag, version, and the
        // exact page length.  A 235-byte prefix is not an account.
        assert_eq!(
            stream::OrderPageHeader::decode(&bytes[..stream::ORDER_PAGE_HEADER_BYTES]),
            Err(CodecError::Truncated)
        );

        // It decides header facts only.  A page whose slot array is junk still
        // has a well-formed header, and the page verdict is the one that sees
        // the difference.
        let mut junk = bytes;
        junk[PAGE_HEADER_BYTES] = 3;
        assert_eq!(
            stream::OrderPageHeader::decode(&junk)
                .unwrap()
                .validate_shape(),
            Ok(())
        );
        assert_eq!(stream::verify_page(&junk), Err(CodecError::WrongTag));

        // And it refuses every header-local fault on its own.
        let mut open_with_commitment = page;
        open_with_commitment.set_order_count = 2;
        assert_eq!(
            stream::OrderPageHeader::decode(&encode_page_unchecked(&open_with_commitment))
                .unwrap()
                .validate_shape(),
            Err(CodecError::NonCanonicalPadding)
        );
        let mut zero_market = page;
        zero_market.market = Hash32::ZERO;
        assert_eq!(
            stream::OrderPageHeader::decode(&encode_page_unchecked(&zero_market))
                .unwrap()
                .validate_shape(),
            Err(CodecError::ZeroIdentity)
        );
        let mut ranged = build_page(0, 1, &[], Hash32::ZERO);
        ranged.first_order_id = h(3);
        assert_eq!(
            stream::OrderPageHeader::decode(&encode_page_unchecked(&ranged))
                .unwrap()
                .validate_shape(),
            Err(CodecError::MismatchedBinding)
        );
    }
    #[test]
    fn the_slot_cursor_reads_one_slot_at_a_time_and_keeps_the_order_chain() {
        let mut page = build_page(0, 1, &[3, 9], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(portfolio(9));
        reseal(&mut page);
        let bytes = encode_page_unchecked(&page);

        let mut cursor = stream::OrderSlotCursor::new(&bytes).unwrap();
        assert_eq!(cursor.index(), 0);
        assert_eq!(cursor.remaining(), MAX_ORDERS_PER_PAGE);
        assert_eq!(cursor.next_slot(), Some(Ok(single(3))));
        assert_eq!(cursor.index(), 1);
        assert_eq!(
            cursor.next_slot(),
            Some(Ok(OrderSlot::Portfolio(portfolio(9))))
        );
        let mut seen = 2;
        while let Some(step) = cursor.next_slot() {
            assert_eq!(step, Ok(OrderSlot::Empty));
            seen += 1;
        }
        assert_eq!(seen, MAX_ORDERS_PER_PAGE);
        assert_eq!(cursor.remaining(), 0);

        // The whole array, as an iterator, is the same walk.
        let walked = stream::OrderSlotCursor::new(&bytes).unwrap();
        let mut records = 0;
        for step in walked {
            if step.unwrap() != OrderSlot::Empty {
                records += 1;
            }
        }
        assert_eq!(records, page.order_count as usize);

        // The chain is enforced across calls, not within one slot.
        let mut descending = build_page(0, 1, &[9, 3], Hash32::ZERO);
        descending.page_digest = descending.recomputed_page_digest().unwrap();
        let descending_bytes = encode_page_unchecked(&descending);
        let mut chain = stream::OrderSlotCursor::new(&descending_bytes).unwrap();
        assert_eq!(chain.next_slot(), Some(Ok(single(9))));
        assert_eq!(
            chain.next_slot(),
            Some(Err(CodecError::NonCanonicalIdentity))
        );
        // A refusal fuses the cursor.
        assert_eq!(chain.next_slot(), None);

        // Structural refusals are per slot: kind byte, exact width, padding.
        let mut unknown = bytes;
        unknown[PAGE_HEADER_BYTES + ORDER_SLOT_BYTES] = 7;
        let mut kinds = stream::OrderSlotCursor::new(&unknown).unwrap();
        assert_eq!(kinds.next_slot(), Some(Ok(single(3))));
        assert_eq!(kinds.next_slot(), Some(Err(CodecError::WrongTag)));
        let mut dirty = bytes;
        dirty[PAGE_HEADER_BYTES + 3 * ORDER_SLOT_BYTES + 1] = 1;
        let mut pad = stream::OrderSlotCursor::new(&dirty).unwrap();
        assert_eq!(pad.nth(3), Some(Err(CodecError::NonCanonicalPadding)));
    }
    #[test]
    fn the_streamed_page_digest_matches_the_buffered_recompute() {
        let mut page = build_page(0, 1, &[3, 9], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(portfolio(9));
        reseal(&mut page);
        let bytes = encode_page_unchecked(&page);
        assert_eq!(
            stream::streamed_page_digest(&bytes),
            Ok(page.recomputed_page_digest().unwrap())
        );
        assert_eq!(
            stream::streamed_page_digest(&bytes),
            Ok(canonical_page_digest(
                page.market,
                page.epoch,
                page.page_index,
                page.order_count,
                &bytes[PAGE_HEADER_BYTES..],
            ))
        );
        // One changed record atom is one changed digest.
        let mut moved = page;
        moved.orders[0] = OrderSlot::Single(OrderRecord {
            quantity: 11,
            ..order(3)
        });
        assert_ne!(
            stream::streamed_page_digest(&encode_page_unchecked(&moved)),
            stream::streamed_page_digest(&bytes)
        );
        // A page whose slot array is not canonical has no digest at all.
        let mut junk = bytes;
        junk[PAGE_HEADER_BYTES + 2 * ORDER_SLOT_BYTES] = 9;
        assert_eq!(
            stream::streamed_page_digest(&junk),
            Err(CodecError::WrongTag)
        );
    }
    #[test]
    fn streaming_page_set_closure_matches_the_buffered_closure() {
        let pages = frozen_pages();
        let b0 = encode_page_unchecked(&pages[0]);
        let b1 = encode_page_unchecked(&pages[1]);
        assert_eq!(stream::verify_page_set(&[&b0, &b1]), Ok(pages[0].order_set));
        set_agrees("dense frozen set", &[&b0, &b1]);
        set_agrees("no pages", &[]);
        set_agrees("head alone", &[&b0]);

        let mixed = frozen_pages_with_portfolio();
        let m0 = encode_page_unchecked(&mixed[0]);
        let m1 = encode_page_unchecked(&mixed[1]);
        set_agrees("frozen set with a portfolio record", &[&m0, &m1]);

        // A three-page set, and the same set with its middle page dropped.
        let ids: [u8; MAX_ORDERS_PER_PAGE] =
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let ids2: [u8; MAX_ORDERS_PER_PAGE] = [
            17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
        ];
        let p0 = build_page(0, 3, &ids, Hash32::ZERO);
        let p1 = build_page(1, 3, &ids2, p0.last_order_id);
        let p2 = build_page(2, 3, &[33, 34], p1.last_order_id);
        let mut three = [p0, p1, p2];
        freeze_set(&mut three);
        let t0 = encode_page_unchecked(&three[0]);
        let t1 = encode_page_unchecked(&three[1]);
        let t2 = encode_page_unchecked(&three[2]);
        set_agrees("three dense pages", &[&t0, &t1, &t2]);
        set_agrees("middle page dropped", &[&t0, &t2]);

        // Duplicate order id across a page boundary.
        let mut dup = pages;
        dup[1].orders[0] = single(16);
        dup[1].first_order_id = h(16);
        dup[1].page_digest = dup[1].recomputed_page_digest().unwrap();
        let d1 = encode_page_unchecked(&dup[1]);
        set_agrees("duplicate id across the boundary", &[&b0, &d1]);

        // Pages presented out of index order.
        set_agrees("reordered pages", &[&b1, &b0]);

        // A post-freeze mutation, including the case where the mutator also
        // repairs that page's own digest.
        let mut mutated = pages;
        mutated[0].orders[3] = OrderSlot::Single(OrderRecord {
            quantity: 11,
            ..order(4)
        });
        let raw = encode_page_unchecked(&mutated[0]);
        set_agrees("mutated record", &[&raw, &b1]);
        mutated[0].page_digest = mutated[0].recomputed_page_digest().unwrap();
        let repaired = encode_page_unchecked(&mutated[0]);
        set_agrees("mutated record, page digest repaired", &[&repaired, &b1]);

        // A broken predecessor link, and an unfrozen page in a closed set.
        let mut unlinked = pages;
        unlinked[1].prev_page_last_order_id = h(15);
        let u1 = encode_page_unchecked(&unlinked[1]);
        set_agrees("broken predecessor link", &[&b0, &u1]);
        let mut thawed = pages;
        thawed[1].frozen = 0;
        let w1 = encode_page_unchecked(&thawed[1]);
        set_agrees("unfrozen page in a closed set", &[&b0, &w1]);

        // A ninth portfolio order added on the next page: the cross-page bound
        // no single page can decide.
        let mut full = build_page(0, 2, &ids, Hash32::ZERO);
        let mut j = 0;
        while j < MAX_PORTFOLIO_ORDERS {
            full.orders[j] = OrderSlot::Portfolio(portfolio(ids[j]));
            j += 1;
        }
        reseal(&mut full);
        let mut tail = build_page(1, 2, &[17, 18, 19], full.last_order_id);
        tail.orders[0] = OrderSlot::Portfolio(portfolio(17));
        reseal(&mut tail);
        let mut nine = [full, tail];
        freeze_set(&mut nine);
        let n0 = encode_page_unchecked(&nine[0]);
        let n1 = encode_page_unchecked(&nine[1]);
        assert_eq!(
            stream::verify_page_set(&[&n0, &n1]),
            Err(CodecError::InvalidCount)
        );
        set_agrees("nine portfolios across two pages", &[&n0, &n1]);

        // A page that does not decode at all is refused in page order, exactly
        // as decoding each page before closing the set would refuse it.
        let mut junk = b1;
        junk[PAGE_HEADER_BYTES] = 3;
        set_agrees("undecodable tail page", &[&b0, &junk]);
        set_agrees("undecodable head page", &[&junk, &b1]);

        // More pages than a book can have is refused before any page is read.
        assert_eq!(
            stream::verify_page_set(&[&t0, &t1, &t2, &t0, &t1]),
            Err(CodecError::InvalidCount)
        );
    }
    #[test]
    fn streaming_grid_and_epoch_bindings_match_the_buffered_ones() {
        let grid = grid();
        let page = build_page(0, 1, &[3, 9], Hash32::ZERO);
        let bytes = encode_page_unchecked(&page);
        assert_eq!(
            stream::verify_page_on_grid(&bytes, &grid),
            OrderPageAccount::decode_on_grid(&bytes, &grid)
                .map(|p| stream::OrderPageHeader::of_page(&p))
        );

        // An off-grid limit has no tick, on either path.
        let mut off = page;
        off.orders[0] = OrderSlot::Single(OrderRecord {
            limit: 2_501,
            ..order(3)
        });
        off.page_digest = off.recomputed_page_digest().unwrap();
        let off_bytes = encode_page_unchecked(&off);
        assert_eq!(
            stream::verify_page_on_grid(&off_bytes, &grid),
            Err(CodecError::InvalidTick)
        );
        assert_eq!(
            stream::verify_page_on_grid(&off_bytes, &grid),
            OrderPageAccount::decode_on_grid(&off_bytes, &grid)
                .map(|p| stream::OrderPageHeader::of_page(&p))
        );

        // A portfolio bound that no candidate could ever be classified against.
        let mut wild = page;
        wild.orders[1] = OrderSlot::Portfolio(PortfolioRecord {
            lots: u64::MAX,
            limit_collateral_per_lot: u64::MAX,
            minimum_fill_lots: 0,
            ..portfolio(9)
        });
        wild.page_digest = wild.recomputed_page_digest().unwrap();
        let wild_bytes = encode_page_unchecked(&wild);
        assert_eq!(
            stream::verify_page_on_grid(&wild_bytes, &grid),
            Err(CodecError::ArithmeticOverflow)
        );
        assert_eq!(
            stream::verify_page_on_grid(&wild_bytes, &grid),
            OrderPageAccount::decode_on_grid(&wild_bytes, &grid)
                .map(|p| stream::OrderPageHeader::of_page(&p))
        );

        // A refused grid refuses the page on either path.
        let mut broken = grid;
        broken.tick_count = 0;
        assert_eq!(
            stream::verify_page_on_grid(&bytes, &broken),
            OrderPageAccount::decode_on_grid(&bytes, &broken)
                .map(|p| stream::OrderPageHeader::of_page(&p))
        );

        // The epoch binding.
        let pages = frozen_pages_with_portfolio();
        let b0 = encode_page_unchecked(&pages[0]);
        let b1 = encode_page_unchecked(&pages[1]);
        let bufs: [&[u8]; 2] = [&b0, &b1];
        let mut e = frozen_epoch();
        e.first_order_id = pages[0].first_order_id;
        e.last_order_id = pages[1].last_order_id;
        e.order_set = pages[0].order_set;
        assert_eq!(stream::epoch_binds_page_set(&e, &bufs), Ok(()));
        assert_eq!(
            stream::epoch_binds_page_set(&e, &bufs),
            e.binds_page_set(&pages)
        );

        // An open epoch commits to nothing, so it can bind no set.
        let mut open = e;
        open.phase = EPOCH_PHASE_OPEN;
        assert_eq!(
            stream::epoch_binds_page_set(&open, &bufs),
            open.binds_page_set(&pages)
        );

        // A set whose commitment is not this epoch's.
        let mut wrong = e;
        wrong.order_set = h(200);
        assert_eq!(
            stream::epoch_binds_page_set(&wrong, &bufs),
            wrong.binds_page_set(&pages)
        );

        // The epoch owns the market's outcome width; a page cannot.
        let mut narrow = e;
        narrow.outcome_count = 2;
        assert_eq!(stream::epoch_binds_page_set(&narrow, &bufs), Ok(()));
        let mut wide = pages;
        let mut coefficients = [0; MAX_OUTCOMES];
        coefficients[0] = 3;
        coefficients[2] = 1;
        wide[1].orders[0] = OrderSlot::Portfolio(PortfolioRecord {
            active_len: 3,
            coefficients,
            ..portfolio(17)
        });
        wide[1].page_digest = wide[1].recomputed_page_digest().unwrap();
        freeze_set(&mut wide);
        let w0 = encode_page_unchecked(&wide[0]);
        let w1 = encode_page_unchecked(&wide[1]);
        let wide_bufs: [&[u8]; 2] = [&w0, &w1];
        let mut wide_epoch = e;
        wide_epoch.order_set = wide[0].order_set;
        assert_eq!(
            stream::epoch_binds_page_set(&wide_epoch, &wide_bufs),
            Ok(())
        );
        wide_epoch.outcome_count = 2;
        assert_eq!(
            stream::epoch_binds_page_set(&wide_epoch, &wide_bufs),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            stream::epoch_binds_page_set(&wide_epoch, &wide_bufs),
            wide_epoch.binds_page_set(&wide)
        );

        // And the single-Egg family it already held.
        let mut off_market = pages;
        off_market[1].orders[1] = OrderSlot::Single(OrderRecord {
            outcome: 5,
            ..order(18)
        });
        off_market[1].page_digest = off_market[1].recomputed_page_digest().unwrap();
        freeze_set(&mut off_market);
        let o0 = encode_page_unchecked(&off_market[0]);
        let o1 = encode_page_unchecked(&off_market[1]);
        let off_bufs: [&[u8]; 2] = [&o0, &o1];
        let mut narrow_epoch = e;
        narrow_epoch.order_set = off_market[0].order_set;
        assert_eq!(
            stream::epoch_binds_page_set(&narrow_epoch, &off_bufs),
            Ok(())
        );
        narrow_epoch.outcome_count = 5;
        assert_eq!(
            stream::epoch_binds_page_set(&narrow_epoch, &off_bufs),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            stream::epoch_binds_page_set(&narrow_epoch, &off_bufs),
            narrow_epoch.binds_page_set(&off_market)
        );
    }
    #[test]
    fn the_streaming_decoders_never_hold_a_page_sized_value() {
        // What the streaming API puts on a frame, by construction: one header,
        // one slot, one cursor, and — only in the set closure — one header per
        // page of a book.  What the buffered decoder puts on a frame is the
        // page itself, twice over: a reader's copy and the returned value.
        use core::mem::size_of;
        assert!(size_of::<stream::OrderPageHeader>() <= 256);
        assert!(size_of::<OrderSlot>() <= ORDER_SLOT_BYTES + 16);
        assert!(size_of::<stream::OrderSlotCursor<'_>>() <= 96);
        assert!(size_of::<[stream::OrderPageHeader; MAX_ORDER_PAGES]>() <= 1024);
        assert!(size_of::<OrderPageAccount>() > 3 * 1024);

        // The mechanical half of the same claim: no page-sized array type may
        // appear anywhere in the streaming module.  A slot array or a
        // page-sized byte buffer is exactly the regression that would put the
        // 8,640-byte frame back.
        //
        // Doc comments are excluded: the module's examples legitimately build a
        // fixture page through the buffered encoder, which is host-side code in
        // a doctest and never in a frame the loader runs.
        for line in include_str!("stream.rs").lines() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            // An array of slots, of slot-width buffers, or of page-width bytes.
            assert!(
                !code.contains("; MAX_ORDERS_PER_PAGE]"),
                "slot array: {line}"
            );
            assert!(!code.contains("; ORDER_SLOT_BYTES]"), "slot buffer: {line}");
            assert!(
                !code.contains("; account_len::ORDER_PAGE]"),
                "page buffer: {line}"
            );
            assert!(!code.contains("[OrderSlot;"), "slot array: {line}");
            // And no call into the buffered page decoder.
            assert!(
                !code.contains("OrderPageAccount::decode"),
                "buffered call: {line}"
            );
        }
    }
}
