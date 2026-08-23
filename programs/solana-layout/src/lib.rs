#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Fixed, hostile-byte-facing layouts for the transparent V1 adapter.
//!
//! This crate contains no allocator, CPI, token implementation, RPC client,
//! signing code, or entrypoint.  It defines byte ownership and deterministic
//! intent bytes.  Off-chain builds use its first-party portable SHA-256; the
//! SBF build delegates only large order-page commitments to Solana's safe
//! native SHA-256 wrapper so a mandatory pre- and post-state commitment fits
//! the transaction compute ceiling.  The adapter must still authenticate
//! account metadata and hand these checked values to the semantic kernel.

#[cfg(not(any(
    feature = "profile-full",
    feature = "profile-direct-v3-source-v2-point",
    feature = "profile-general-source-v2-point"
)))]
compile_error!("select exactly one Dragon's Clutch capability profile");
#[cfg(any(
    all(
        feature = "profile-full",
        feature = "profile-direct-v3-source-v2-point"
    ),
    all(feature = "profile-full", feature = "profile-general-source-v2-point"),
    all(
        feature = "profile-direct-v3-source-v2-point",
        feature = "profile-general-source-v2-point"
    )
))]
compile_error!("Dragon's Clutch capability profiles are mutually exclusive");

pub mod artifact;
pub mod clearing;
pub mod collateral;
pub mod collateral_v3_accounts;
pub mod direct_selection;
pub mod direct_selection_v3;
pub mod failure_recovery;
pub mod failure_interval_consensus;
pub mod failure_market_interval_v2;
pub mod failure_market_replay_v2;
pub mod native_resolution;
pub mod occupation_resolution;
pub mod order_page_v5;
pub mod portfolio_settlement;
pub mod product_series;
pub mod projection;
pub mod registry;
pub mod reservation;
pub mod reservation_v9;
pub mod settlement_receipt_v3;
pub mod source_series;
pub mod settlement_receipt_v4;
pub mod settlement_receipt_v5;
pub mod resolution_work;
pub mod revenue;
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
pub const LAYOUT_VERSION: u8 = 5;
/// The initial prototype schema version.
pub const LAYOUT_VERSION_V1: u8 = 1;
/// The schema version of the first persisted-state revision.
///
/// The dense order page encoded this version while its slots were bare 99-byte
/// single-Egg records; it now encodes [`account_version::ORDER_PAGE`] and
/// refuses this one.
pub const LAYOUT_VERSION_V2: u8 = 2;
/// The schema version of the first tagged-slot order page.
///
/// The dense order page encoded this version while its order ids were
/// caller-chosen 32-byte hashes, a cancellation had no representation at all,
/// and no record persisted an expiry.  It now encodes
/// [`account_version::ORDER_PAGE`] and refuses this one.
pub const LAYOUT_VERSION_V3: u8 = 3;
/// Schema version of every deterministic intent encoding.
pub const INTENT_VERSION: u8 = 3;
/// Superseded intent encoding carrying order families but no fee cap.
///
/// Version 2 made portfolio placement expressible. It is refused because a
/// funded buy reservation cannot be computed without the owner's maximum fee
/// authorization.
pub const INTENT_VERSION_V2: u8 = 2;
/// The superseded intent encoding version.
///
/// Version 1 carried a placement as a bare [`OrderRecord`], so a portfolio
/// order was unrepresentable on the wire, and a cancellation carried no
/// generation.  Every intent decoder refuses it explicitly with
/// [`CodecError::WrongVersion`].
pub const INTENT_VERSION_V1: u8 = 1;
/// Number of bytes in every identity/hash field.
pub const HASH_BYTES: usize = 32;

/// Exact byte length of the superseded V1 parent Profile preimage.
///
/// Frozen by `docs/implementation/RESOLUTION_EVIDENCE_PLAN.md` §3.2: an eight
/// byte magic, parent schema and flags, the collateral subfield tag and its
/// schema, the 32-byte collateral-policy digest, and 16 zero reserved bytes.
/// [`canonical_profile_hash`] owns the length requirement; the parent
/// encoder/decoder that produces these bytes is
/// [`collateral::ParentProfile`].
pub const PROFILE_PARENT_BYTES: usize = 64;
/// Canonical Profile V2 schema selected by every live Realm.
pub const PROFILE_SCHEMA_V2: u8 = 2;
/// Domain for the canonical Profile V2 identity over policy and release IDs.
pub const PROFILE_V2_DOMAIN: &[u8] = b"dragons-clutch/profile/v2\0";
/// Maximum number of outcomes in a market.
pub const MAX_OUTCOMES: usize = 16;
/// Maximum number of payout vectors in one immutable terms set.
///
/// Mirrors `clutch_kernel::MAX_PAYOUTS`; this crate stays independent of the
/// semantic kernel, so the bound is restated rather than imported, and a codec
/// test pins it.
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
/// Maximum slices in one explicit pairing witness.
///
/// Mirrors `clutch_batch::relation_v1::MAX_SLICES` (`2 * MAX_LEGS + 2 *
/// MAX_OUTCOMES` at `MAX_LEGS = 192`).  Restated rather than imported, like
/// every other bound here; [`clearing::CandidateFeedAccount`] is the account
/// that carries that many slices and a codec test pins the number.
pub const MAX_SLICES: usize = 416;
/// Exact byte length of the streaming-checkpoint body.
///
/// Mirrors the pinned `clutch_batch::relation_v1_stream::ClearWorkV1::
/// ENCODED_BYTES` — the checkpoint codec's canonical serialized length
/// (Tier 2 join 5), pinned there by `clear_work_encoded_bytes_are_pinned`.
/// This is a **wire fact**, not a `size_of` measurement: the codec is an
/// explicit little-endian field walk (`encode_into`/`decode_into`, both by
/// reference), independent of any `repr(Rust)` layout accident.  Before the
/// codec landed, this constant pinned `size_of::<ClearWorkV1>()` = 48,592 and
/// the body was opaque bytes.
///
/// **This crate still owns the length and nothing inside it.**  The body
/// region of [`clearing::ClearWorkAccount`] is written and read only by the
/// checkpoint codec in `clutch-batch`; this crate's contract remains the
/// framing, the identity binding, and the streaming window accessors — never
/// an interpretation.  See `docs/implementation/SOLANA_LAYOUT.md`, "The
/// clearing plane".
pub const CLEAR_WORK_BODY_BYTES: usize = 47_846;
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
/// Largest number of knots a terms basis can freeze.
///
/// Bounds every degree of the B-spline basis family of
/// `docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md` §2.1: at degree 0 the
/// knots are the interior cell boundaries (`K = n − 1`), at degree 1 they are
/// the claim anchor sites (`K = n`), and `MAX_OUTCOMES = 16` keeps every case
/// inside this width.
pub const MAX_KNOTS: usize = 16;
/// Largest admitted B-spline basis degree.
pub const MAX_BASIS_DEGREE: u8 = 3;
/// [`TermsAccount::uniform_log2_spacing`] sentinel: knots are not uniform.
pub const UNIFORM_SPACING_NONE: u8 = 0xFF;
/// [`TermsAccount::payout_map`] entry meaning "this cell index is not live".
pub const PAYOUT_MAP_UNUSED: u8 = 0xFF;
/// Exact encoded length of one [`OrderRecord`] body, without its slot kind byte.
pub const ORDER_RECORD_BYTES: usize = 32 + 32 + 1 + 1 + 8 + 8 + 8 + 1 + 8 + 8;
/// Exact encoded length of one [`PortfolioRecord`] body, without its kind byte.
///
/// The coefficient vector is stored at full [`MAX_OUTCOMES`] width with
/// canonical zero padding beyond `active_len`, exactly like every other
/// outcome-indexed vector here, so this length does not depend on how many
/// outcomes one portfolio actually touches.
///
/// The five trailing `u64` fields — `lots`, `limit_collateral_per_lot`,
/// `minimum_fill_lots`, `generation`, `expiry_epoch` — are grouped rather than
pub const PORTFOLIO_RECORD_BYTES: usize = 32 + 32 + 1 + 1 + 1 + (MAX_OUTCOMES * 8) + (5 * 8);
/// Exact encoded length of one [`TombstoneRecord`] body, without its kind byte.
///
/// A retirement keeps the identity and the owner it retired and adds the two
/// generations that order the retirement against the placement.  It is by far
/// the narrowest body, so a cancellation never widens a slot.
pub const TOMBSTONE_RECORD_BYTES: usize = 32 + 32 + 8 + 8;
/// Exact encoded length of one order slot in a page.
///
/// A slot is a one-byte kind discriminator, that kind's exact body, and
/// canonical zero padding out to this common width.  Fixing the slot width is
/// what lets one page hold every admitted slot kind — both order families and
/// a retirement — while keeping a single exact account length, a single
/// positional order-id chain, and a single page-set fold.  The padding is not
/// slack: every byte of it is required to be zero, so it can never influence a
/// digest.
pub const ORDER_SLOT_BYTES: usize = 1 + PORTFOLIO_RECORD_BYTES;
/// Order-slot kind: canonical padding.  The whole slot is zero.
pub const ORDER_KIND_EMPTY: u8 = 0;
/// Order-slot kind: one single-Egg [`OrderRecord`].
pub const ORDER_KIND_SINGLE: u8 = 1;
/// Order-slot kind: one [`PortfolioRecord`].
pub const ORDER_KIND_PORTFOLIO: u8 = 2;
/// Order-slot kind: one [`TombstoneRecord`], a retired order id.
///
/// A tombstone is what a cancellation writes.  It occupies the slot and the
/// order id of the record it retired, so retiring an order never renumbers a
/// later one; see [`canonical_order_id`].
pub const ORDER_KIND_TOMBSTONE: u8 = 3;
/// Maximum encoded instruction length.
///
/// This is exactly the widest admitted intent — a v2 source-spec construction,
/// which carries the whole 368-byte canonical pull body behind its Terms
/// binding — not a round number with slack in it.  A test pins every variant
/// against it.
///
/// It was the portfolio placement (310 bytes) until the v2 source family
/// landed; the placement remains the widest *order* intent and the constant
/// still names one exact variant rather than a budget.
pub const MAX_INTENT_BYTES: usize = 2 + HASH_BYTES + SOURCE_SPEC_BODY_V2_BYTES;
/// Exact signed-wire width of one V1 source-spec body.
///
/// The source admission crate owns the meaning of these bytes.  The layout
/// owns only their fixed instruction width so a wallet can present the body
/// without a caller-owned evidence account becoming a competing semantic
/// owner.  `clutch-sbf` compile-time asserts this equals its reviewed
/// `SourceSpecV1` codec width before admitting the intent.
pub const SOURCE_SPEC_BODY_V1_BYTES: usize = 256;
/// Exact signed-wire width of one v2 (pull-profile) source-spec body.
///
/// Same division of labour as [`SOURCE_SPEC_BODY_V1_BYTES`]: the admission
/// crate owns the meaning, the layout owns the fixed width, and `clutch-sbf`
/// compile-time asserts this equals its reviewed `SourceSpecV2` codec width
/// before admitting [`Intent::InitSourceSpecV2`].
///
/// The body is wider than V1's because a pull spec pins a *deployment* rather
/// than a data account: the receiver program, its ProgramData account, its
/// governance `Config` key and that config's digest, the provider feed id, and
/// the ProgramData deployment slot all travel where V1 carried one immutable
/// price-account key.
pub const SOURCE_SPEC_BODY_V2_BYTES: usize = 368;

const _: () = assert!(MAX_EPOCH_ORDERS == 64);
const _: () = assert!(MAX_PRICE_SCALE > 0);
// The slot is exactly wide enough for the widest record family and no wider.
const _: () = assert!(PORTFOLIO_RECORD_BYTES > ORDER_RECORD_BYTES);
const _: () = assert!(ORDER_RECORD_BYTES > TOMBSTONE_RECORD_BYTES);
const _: () = assert!(ORDER_SLOT_BYTES == 1 + PORTFOLIO_RECORD_BYTES);
const _: () = assert!(MAX_PORTFOLIO_ORDERS <= MAX_EPOCH_ORDERS);
// Every canonical order id in an admitted book is a rank in `1 ..= 64`, so the
// eight-byte rank field can never be the thing that overflows.
const _: () = assert!(MAX_EPOCH_ORDERS <= u16::MAX as usize);

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
const CLEAR_WORK_TAG: u8 = 17;
const CANDIDATE_FEED_TAG: u8 = 18;

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

#[cfg(all(test, not(feature = "profile-full")))]
mod capability_profile_tests {
    use super::{CodecError, Intent, INTENT_VERSION};

    fn assert_disabled(tags: &[u8]) {
        for tag in tags {
            assert_eq!(
                Intent::decode(&[*tag, INTENT_VERSION]),
                Err(CodecError::WrongTag),
                "disabled tag {tag} reached a payload decoder"
            );
        }
    }

    fn assert_enabled(tags: &[u8]) {
        for tag in tags {
            assert_ne!(
                Intent::decode(&[*tag, INTENT_VERSION]),
                Err(CodecError::WrongTag),
                "enabled tag {tag} was removed from Intent::decode"
            );
        }
    }

    #[test]
    #[cfg(feature = "profile-direct-v3-source-v2-point")]
    fn direct_profile_intent_decoder_excludes_other_families() {
        assert_disabled(&[6, 8, 9, 22, 23, 27, 32, 47, 69]);
        assert_enabled(&[1, 7, 10, 18, 68, 70, 73]);
    }

    #[test]
    #[cfg(feature = "profile-general-source-v2-point")]
    fn general_profile_intent_decoder_excludes_other_families() {
        assert_disabled(&[6, 22, 23, 27, 32]);
        assert_enabled(&[1, 7, 8, 9, 10, 18, 47, 69, 70, 73]);
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

/* SHA-256 is included as a tiny portable identity primitive.  It is
 * used only for canonical IDs; this crate makes no claim that a future
 * Solana deployment has selected this primitive until its profile says so. */

/// Largest number of `parts` any [`digest`] caller in this crate passes.
///
/// The domain string occupies one further slice, so the native preimage array
/// is one longer.  The bound is checked at compile time inside [`digest`], so a
/// new call site that exceeds it is a build failure and never a truncated
/// preimage.
const MAX_DIGEST_PARTS: usize = 16;

/// Domain-separated SHA-256 over `domain` followed by every element of `parts`.
///
/// Host and research builds fold the portable first-party SHA-256.  SBF hands
/// the identical slice sequence to Solana's safe native-hasher wrapper: the
/// concatenation `hashv` commits to is byte-for-byte the sequence the portable
/// path streams, so the two produce the same value and only the compute cost
/// differs.  Every canonical identity in this crate goes through here, so the
/// portable path stays as the off-chain oracle the equivalence tests compare
/// against rather than being deleted.
#[cfg(not(target_os = "solana"))]
fn digest<const N: usize>(domain: &[u8], parts: &[&[u8]; N]) -> Hash32 {
    const { assert!(N <= MAX_DIGEST_PARTS) };
    let mut h = Sha256::new();
    h.update(domain);
    let mut i = 0;
    while i < N {
        h.update(parts[i]);
        i += 1;
    }
    Hash32(h.finish())
}

/// Assemble and hash the domain-separated preimage with Solana's native wrapper.
///
/// Built on SBF and in host unit tests only.  The test build uses the wrapper's
/// `sha2` implementation, which is what lets the assembled slice sequence be
/// checked byte-for-byte against the portable path; production SBF invokes
/// `sol_sha256`.
#[cfg(any(target_os = "solana", test))]
fn native_digest<const N: usize>(domain: &[u8], parts: &[&[u8]; N]) -> Hash32 {
    const { assert!(N <= MAX_DIGEST_PARTS) };
    let mut preimage: [&[u8]; MAX_DIGEST_PARTS + 1] = [&[]; MAX_DIGEST_PARTS + 1];
    preimage[0] = domain;
    let mut i = 0;
    while i < N {
        preimage[1 + i] = parts[i];
        i += 1;
    }
    Hash32(solana_sha256_hasher::hashv(&preimage[..N + 1]).to_bytes())
}

#[cfg(target_os = "solana")]
fn digest<const N: usize>(domain: &[u8], parts: &[&[u8]; N]) -> Hash32 {
    native_digest(domain, parts)
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

/// Derive the canonical Profile V2 identity from its two immutable children.
///
/// The collateral-policy content identity binds mint, decimals, ceilings, and
/// the selected release. The separately retained release identity makes the
/// parent join directly inspectable and refuses a policy/release substitution.
pub fn canonical_profile_v2_id(
    collateral_policy_id: Hash32,
    adapter_release_id: Hash32,
) -> Result<ProfileHash> {
    check_hash(collateral_policy_id)?;
    check_hash(adapter_release_id)?;
    Ok(digest(
        PROFILE_V2_DOMAIN,
        &[&collateral_policy_id.0, &adapter_release_id.0],
    ))
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
/// The preimage is the exact 1,620-byte terms **body**: every encoded
/// [`TermsAccount`] byte after the two-byte `(tag, version)` header and the
/// 32-byte stored `terms` digest itself, up to but excluding the trailing
/// `stored_bump` and `flags` bytes.  The digest therefore commits to the
/// payout set, the window policy, the failure policy, every v3 resolution
/// field — statistic, ambiguity/edge policies, basis degree, knot count and
/// vector, uniform-spacing declaration, failure payout index, coverage
/// parameter, repair generation, source identity/versions, payout map — and
/// the per-market `collateral_cap`, together.  The `stored_bump` and `flags`
/// stay outside on purpose: they are address-derivation artifacts, and a PDA
/// derived from the digest cannot also be an input to it.
/// [`MarketAccount::terms`] stores this value.
///
/// The domain moved to `v2` with `account_version::TERMS = 3`, when the body
/// grew its 352 resolution-basis and collateral-cap bytes: the preimage shape
/// changed, so the old domain must not be reusable over the new bytes — the
/// same rule that moved the order-page domain when its record shape changed.
pub fn canonical_terms_digest(body_bytes: &[u8]) -> Hash32 {
    digest(b"dragons-clutch/terms/v2", &[body_bytes])
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
/// It moved to `v2` when the page's records became fixed-width tagged slots
/// and to `v3` when the preimage gained the retirement count: the preimage
/// shape changed both times, so an old domain must not be reusable over the new
/// bytes.  The order-set fold below keeps its own `v1` domain because its
/// preimage shape — market, epoch, page count, order count, page digests — has
/// never changed; only the leaves it folds did, and those already carry the new
/// domain.
const ORDER_PAGE_DOMAIN: &[u8] = b"dragons-clutch/order-page/v3";

/// Bytes of the rank inside a canonical order identity; the rest is zero.
pub const ORDER_ID_RANK_BYTES: usize = 8;

/// Derive the canonical order identity of the `rank`-th slot of a page set.
///
/// An order id is **not** a caller's choice.  It is the big-endian encoding of
/// the order's one-based position in the frozen page set — page
/// `p`, slot `j` is always rank `p * MAX_ORDERS_PER_PAGE + j + 1` — left-padded
/// with zeros to [`HASH_BYTES`].  Three properties follow from the encoding
/// rather than from a check:
///
/// * **No griefing.**  A caller cannot pick an id, so it cannot burn the
///   remainder of a page by claiming a huge one; the id it may place at is
///   whatever [`stream::OrderPageHeader::next_order_id`] says.
/// * **Byte order is rank order.**  The big-endian encoding makes the page's
///   lexicographic id chain and the numeric rank chain the same chain, which is
///   why the cross-page closure did not have to change shape.
/// * **Position is recoverable.**  [`order_id_rank`] inverts this exactly, so a
///   cancellation naming an order id names a page and a slot with no search.
///
/// `rank` zero has no order: ranks are one-based so that the all-zero identity
/// stays reserved for "no order", exactly as every other identity in this crate
/// reserves it.
pub fn canonical_order_id(rank: u64) -> Hash32 {
    let mut bytes = [0u8; HASH_BYTES];
    let rank_bytes = rank.to_be_bytes();
    let mut i = 0;
    while i < ORDER_ID_RANK_BYTES {
        bytes[HASH_BYTES - ORDER_ID_RANK_BYTES + i] = rank_bytes[i];
        i += 1;
    }
    Hash32(bytes)
}

/// Recover the rank a canonical order identity encodes, or refuse.
///
/// The inverse of [`canonical_order_id`], and the only admitted reading of an
/// order id anywhere in this crate.  Every byte before the rank must be zero
/// ([`CodecError::NonCanonicalIdentity`]), the rank must be nonzero
/// ([`CodecError::ZeroIdentity`]), and it must be a rank some admitted page set
/// could actually hold ([`CodecError::InvalidCount`]) — a book is exactly
/// [`MAX_EPOCH_ORDERS`] slots wide, so an id above that names no slot in any
/// book and is refused before any page is consulted.
pub fn order_id_rank(id: Hash32) -> Result<u64> {
    let mut i = 0;
    while i < HASH_BYTES - ORDER_ID_RANK_BYTES {
        if id.0[i] != 0 {
            return Err(CodecError::NonCanonicalIdentity);
        }
        i += 1;
    }
    let mut rank_bytes = [0u8; ORDER_ID_RANK_BYTES];
    let mut j = 0;
    while j < ORDER_ID_RANK_BYTES {
        rank_bytes[j] = id.0[HASH_BYTES - ORDER_ID_RANK_BYTES + j];
        j += 1;
    }
    let rank = u64::from_be_bytes(rank_bytes);
    if rank == 0 {
        return Err(CodecError::ZeroIdentity);
    }
    if rank > MAX_EPOCH_ORDERS as u64 {
        return Err(CodecError::InvalidCount);
    }
    Ok(rank)
}

/// The rank of the last slot before page `page_index`, as a canonical id.
///
/// Zero for page zero, which opens the chain.  A page's stored
/// `prev_page_last_order_id` is exactly this value, and it is a fact about the
/// page **geometry**, not about how full the earlier pages happen to be: ranks
/// are positional, so the slots before page `p` are the `p * MAX_ORDERS_PER_PAGE`
/// slots those pages own whether or not they are populated yet.  That is what
/// makes a rank globally unique the moment it is written — a half-filled page
/// zero can never reach a rank page one has already used.
fn page_base_order_id(page_index: u16) -> Hash32 {
    if page_index == 0 {
        Hash32::ZERO
    } else {
        canonical_order_id(page_base_rank(page_index))
    }
}

/// The count of slots in every page before `page_index`.
const fn page_base_rank(page_index: u16) -> u64 {
    (page_index as u64) * (MAX_ORDERS_PER_PAGE as u64)
}

/// Derive one order page's digest from its page position and slot bytes.
///
/// `record_bytes` is the exact concatenation of all [`MAX_ORDERS_PER_PAGE`]
/// encoded slots, that is [`ORDER_SLOT_BYTES`] each including canonical
/// padding.  [`OrderPageAccount::recomputed_page_digest`] streams the same
/// bytes without buffering them.
///
/// `tombstone_count` is in the preimage even though it is a fold over the very
/// slots that follow it, for the same reason `order_count` is: both are header
/// bytes a writer stores, and a digest that did not cover them would let a page
/// disagree with its own header without disagreeing with its own digest.
pub fn canonical_page_digest(
    market: MarketId,
    epoch: EpochId,
    page_index: u16,
    order_count: u8,
    tombstone_count: u8,
    record_bytes: &[u8],
) -> Hash32 {
    digest(
        ORDER_PAGE_DOMAIN,
        &[
            &market.0,
            &epoch.0,
            &page_index.to_le_bytes(),
            &[order_count, tombstone_count],
            record_bytes,
        ],
    )
}

/// Domain string of the cross-page order-set commitment.
const ORDER_SET_DOMAIN: &[u8] = b"dragons-clutch/order-set/v1";

/// Fold every page digest, in page order, into the set-wide order-set identity.
///
/// This is the cross-page commitment: a page cannot be added, dropped,
/// reordered, or mutated without changing the value every page of the set
/// stores in [`OrderPageAccount::order_set`].
///
/// A set has at most [`MAX_ORDER_PAGES`] pages, which is what sizes the native
/// preimage array on SBF.  A longer `page_digests` is not a representable order
/// set; it aborts there rather than committing to a truncated preimage, so no
/// input produces a digest that disagrees with the portable path.
pub fn canonical_order_set_id(
    market: MarketId,
    epoch: EpochId,
    page_count: u16,
    set_order_count: u16,
    page_digests: &[Hash32],
) -> Hash32 {
    fold_order_set_id(market, epoch, page_count, set_order_count, page_digests)
}

/// Portable first-party fold, retained off-chain as the equivalence oracle.
#[cfg(not(target_os = "solana"))]
fn fold_order_set_id(
    market: MarketId,
    epoch: EpochId,
    page_count: u16,
    set_order_count: u16,
    page_digests: &[Hash32],
) -> Hash32 {
    let mut h = Sha256::new();
    h.update(ORDER_SET_DOMAIN);
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

/// Assemble the identical preimage for Solana's native hasher wrapper.
///
/// Built on SBF and in host unit tests only; the test build proves the slice
/// assembly equals the portable fold.
#[cfg(any(target_os = "solana", test))]
fn native_order_set_id(
    market: MarketId,
    epoch: EpochId,
    page_count: u16,
    set_order_count: u16,
    page_digests: &[Hash32],
) -> Hash32 {
    let pages = page_count.to_le_bytes();
    let orders = set_order_count.to_le_bytes();
    let mut preimage: [&[u8]; 5 + MAX_ORDER_PAGES] = [&[]; 5 + MAX_ORDER_PAGES];
    preimage[0] = ORDER_SET_DOMAIN;
    preimage[1] = &market.0;
    preimage[2] = &epoch.0;
    preimage[3] = &pages;
    preimage[4] = &orders;
    let mut i = 0;
    while i < page_digests.len() {
        preimage[5 + i] = &page_digests[i].0;
        i += 1;
    }
    Hash32(solana_sha256_hasher::hashv(&preimage[..5 + page_digests.len()]).to_bytes())
}

#[cfg(target_os = "solana")]
fn fold_order_set_id(
    market: MarketId,
    epoch: EpochId,
    page_count: u16,
    set_order_count: u16,
    page_digests: &[Hash32],
) -> Hash32 {
    native_order_set_id(market, epoch, page_count, set_order_count, page_digests)
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
/// first prototype. `PROFILE` is the greenfield policy+release body at `2`;
/// the earlier body at this coordinate has no live decoder. `ORDER_PAGE` grew
/// the page-set commitment fields at `2`, replaced its bare records with tagged
/// fixed-width slots at `3`, and then made order ids positional, added the
/// retirement slot kind and its header count, and gave every record a persisted
/// expiry, so it encodes `4` and refuses `1`, `2`, and `3`.
/// `SETTLEMENT_RECEIPT_V3` keeps version 2's width but gives its final
/// reserved-zero byte independent accounting-latch semantics. V4 preserves
/// that width under a fresh version and adds the canonical merge-payment
/// window; neither successor reinterprets an earlier version.
/// The pair `(tag, version)` therefore never names two shapes.
pub mod account_version {
    /// Realm account, unchanged since the first prototype.
    pub const REALM: u8 = 1;
    /// Canonical Profile V2; prior live shapes are withdrawn in greenfield.
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
    pub const ORDER_PAGE: u8 = 4;
    /// General OrderPage successor with one Position generation per slot.
    /// Version 4 bytes are deliberately not reinterpreted.
    pub const ORDER_PAGE_V5: u8 = 5;
    /// Supply ledger account.
    pub const SUPPLY_LEDGER: u8 = 2;
    /// Immutable terms account.
    ///
    /// Version 3 is the unified resolution-basis revision
    /// (`docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md` §6 plus the
    /// per-market `collateral_cap` of the §3.5 finding in
    /// `docs/implementation/RESOLUTION_EVIDENCE_PLAN.md`).  Version 2 lacked
    /// every basis field — statistic, ambiguity/edge policies, degree, knots,
    /// payout map, coverage parameter, repair generation, source identity and
    /// versions — and the collateral cap; its bytes are refused, exactly as
    /// the superseded prototype version 1's are.
    pub const TERMS: u8 = 3;
    /// Epoch/book-domain account.
    pub const EPOCH: u8 = 2;
    /// Frozen price-grid account.
    pub const PRICE_GRID: u8 = 2;
    /// Candidate record account.
    ///
    /// Version 3 is the verified-score revision (T2-6c): it appends the
    /// 32-byte `score_digest` — the full-width relation-candidate tie
    /// identity `CompleteClearWork` stamps at verification — to the claimed
    /// score components.  Version 2 records carried claims only and are
    /// refused, exactly as version 1's were.
    pub const CANDIDATE: u8 = 3;
    /// Final-pot account.
    pub const FINAL_POT: u8 = 2;
    /// Settlement receipt account.
    pub const SETTLEMENT_RECEIPT: u8 = 2;
    /// General settlement receipt successor with independent accounting and
    /// delivery latches. Version 2 bytes are deliberately not reinterpreted.
    pub const SETTLEMENT_RECEIPT_V3: u8 = 3;
    /// Same-width General receipt with a distinct merge-payment transition.
    /// V3 bytes are deliberately not reinterpreted.
    pub const SETTLEMENT_RECEIPT_V4: u8 = 4;
    /// Rent-owned receipt with a typed specialized-transition commitment.
    /// V4 bytes remain historical and are never reinterpreted.
    pub const SETTLEMENT_RECEIPT_V5: u8 = 5;
    /// Resolution account.
    pub const RESOLUTION: u8 = 2;
    /// Streaming-checkpoint account; introduced by the clearing plane.
    pub const CLEAR_WORK: u8 = 1;
    /// Candidate feed account; introduced by the clearing plane.
    ///
    /// Version 1 is its first shape.  It is a *different account* from
    /// [`CANDIDATE`], which is at version 2: the candidate **record** persists
    /// a proposal's free coordinates and deliberately does not persist fills,
    /// while the candidate **feed** is the solver-written artifact the
    /// streaming verifier consumes — the same coordinates plus the fill vector
    /// and the optional pairing witness.  Two accounts, two tags, two version
    /// ladders.
    pub const CANDIDATE_FEED: u8 = 1;
}

/// Account discriminator and exact fixed byte lengths.
pub mod account_len {
    use super::{
        CLEAR_WORK_BODY_BYTES, MAX_EPOCH_ORDERS, MAX_GRID_TICKS, MAX_KNOTS, MAX_ORDERS_PER_PAGE,
        MAX_OUTCOMES, MAX_PAYOUTS, MAX_SLICES, ORDER_SLOT_BYTES,
    };

    /// Realm account bytes.
    pub const REALM: usize = 2 + 32 + 32 + 1 + 1 + 1 + 1;
    /// Profile account bytes.
    pub const PROFILE: usize = 2 + 32 + 32 + 32 + 32 + 1 + 1;
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
        2 + (7 * 32) + 2 + 2 + 2 + 1 + 1 + 1 + 1 + (MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES);
    /// General OrderPage successor bytes: the exact V4 semantic prefix and
    /// slots followed by one little-endian Position generation per slot.
    pub const ORDER_PAGE_V5: usize = ORDER_PAGE + (MAX_ORDERS_PER_PAGE * 8);
    /// Supply ledger account bytes.
    pub const SUPPLY_LEDGER: usize = 2 + 32 + 32 + 8 + 1 + (2 * MAX_OUTCOMES * 8) + 1 + 1;
    /// Immutable terms account bytes.
    ///
    /// The v3 body appends, after the v2 window-policy fields: statistic id
    /// (2), ambiguity/edge policy ids (1 + 1), basis degree (1), knot count
    /// (1), uniform-spacing declaration (1), failure payout index (1), one
    /// reserved zero byte, coverage-policy parameter (8), repair generation
    /// (8), source and evaluator versions (4 + 4), source-adapter identity
    /// (32), the degree-0 payout map (16), the knot vector (16 × 16), the
    /// per-market collateral cap (8), and seven reserved zero bytes — 352 new
    /// bytes, taking the account from 1,304 to 1,656.
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
        + 2
        + 1
        + 1
        + 1
        + 1
        + 1
        + 1
        + 1
        + 8
        + 8
        + 4
        + 4
        + 32
        + MAX_OUTCOMES
        + (MAX_KNOTS * 16)
        + 8
        + 7
        + 1
        + 1;
    /// Frozen price-grid account bytes.
    pub const PRICE_GRID: usize = 2 + 32 + 32 + 8 + 1 + (MAX_GRID_TICKS * 8) + 1 + 1;
    /// Epoch/book-domain account bytes.
    pub const EPOCH: usize = 2 + (9 * 32) + 8 + 4 + 8 + 8 + 2 + 2 + 2 + 1 + 1 + 1 + 1 + 1;
    /// Candidate record account bytes.
    pub const CANDIDATE: usize = 2
        + (3 * 32)
        + (MAX_OUTCOMES * 8)
        + 8
        + 8
        + 8
        + 16
        + 16
        + 32
        + 8
        + 8
        + 2
        + 1
        + 1
        + 1
        + 1
        + 1;
    /// Final-pot account bytes.
    pub const FINAL_POT: usize = 2 + (3 * 32) + (MAX_OUTCOMES * 8) + 16 + 16 + 1 + 1 + 1 + 1;
    /// Settlement receipt account bytes.
    pub const SETTLEMENT_RECEIPT: usize = 2 + (5 * 32) + 16 + 8 + 8 + 8 + 8 + 2 + 1 + 1 + 1 + 1 + 1;
    /// General settlement receipt successor bytes. Version 3 reuses the one
    /// formerly-reserved final byte without changing the rent footprint.
    pub const SETTLEMENT_RECEIPT_V3: usize = SETTLEMENT_RECEIPT;
    /// General settlement receipt V4 bytes. V4 changes lifecycle semantics and
    /// transition domains without changing the rent footprint.
    pub const SETTLEMENT_RECEIPT_V4: usize = SETTLEMENT_RECEIPT;
    /// Rent-owned General receipt V5 bytes: V4 semantics, one typed 33-byte
    /// transition compartment, and one exact 48-byte deletable-rent owner.
    pub const SETTLEMENT_RECEIPT_V5: usize = SETTLEMENT_RECEIPT_V4 + 33 + 48;
    /// Resolution account bytes.
    pub const RESOLUTION: usize = 2 + (4 * 32) + 8 + 8 + 8 + 8 + 1 + 1 + 1;
    /// Streaming-checkpoint account bytes: the header, the layout-owned
    /// owner-interning region, then the opaque body.
    ///
    /// [`super::clearing::CLEAR_WORK_HEADER_BYTES`] of layout-owned framing,
    /// then [`super::clearing::CLEAR_WORK_INTERNER_BYTES`] persisting the
    /// projection's owner↔tag table across walk transactions, then exactly
    /// [`super::CLEAR_WORK_BODY_BYTES`] the layout never reads.  This is by
    /// far the largest account in the inventory and it is the one that does
    /// not fit a single system-program creation via CPI; the creation path is
    /// analyzed in `docs/implementation/SOLANA_LAYOUT.md`.
    pub const CLEAR_WORK: usize = 2
        + (4 * 32)
        + 16
        + 4
        + 2
        + 2
        + 1
        + 1
        + 1
        + 1
        + super::clearing::CLEAR_WORK_INTERNER_BYTES
        + CLEAR_WORK_BODY_BYTES;
    /// Candidate feed account bytes: header, fill vector, slice vector.
    pub const CANDIDATE_FEED: usize = 2
        + (4 * 32)
        + (MAX_OUTCOMES * 8)
        + 8
        + 8
        + 8
        + 16
        + 16
        + 16
        + 8
        + 2
        + 2
        + 1
        + 1
        + 1
        + 1
        + (MAX_EPOCH_ORDERS * 8)
        + (MAX_SLICES * super::clearing::PAIRING_SLICE_BYTES);
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

/// Immutable Profile V2 bytes identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileAccountV2 {
    /// Profile identity.
    pub profile: ProfileHash,
    /// Owning Realm identity.
    pub realm: RealmHash,
    /// Exact `CollateralPolicyV2` content identity.
    pub collateral_policy_id: Hash32,
    /// Exact compiled `AdapterReleaseV2` content identity.
    pub adapter_release_id: Hash32,
    /// Profile schema version; exactly [`PROFILE_SCHEMA_V2`].
    pub version: u8,
    /// Flags; exactly [`PROFILE_FLAG_POLICY_FROZEN`] in greenfield V2.
    pub flags: u8,
}

/// Canonical Profile account. This aliases V2 only; no V1 decoder remains.
pub type ProfileAccount = ProfileAccountV2;

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
    /// Number of populated slots: live records **and** retirements.
    ///
    /// A cancellation replaces a record with a [`TombstoneRecord`] in the same
    /// slot, so it never lowers this count and never renumbers a later order.
    pub order_count: u8,
    /// How many of those populated slots are retirements.
    ///
    /// `order_count - tombstone_count` is the page's live-order count, which is
    /// what the relation projection walks; a fold over headers alone can
    /// therefore size a book's live order feed without touching a slot.
    pub tombstone_count: u8,
    /// Freeze state: 0 open, 1 frozen.
    pub frozen: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Slots in positional canonical order-id order; see [`canonical_order_id`].
    pub orders: [OrderSlot; MAX_ORDERS_PER_PAGE],
}

/// One fixed-size transparent order record. It carries no matching result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderRecord {
    /// Position owner identity.
    pub owner: OwnerId,
    /// Canonical order identity: this record's slot rank; see
    /// [`canonical_order_id`].
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
    /// Last epoch index this order may be cleared in, inclusive.
    ///
    /// The relation admits an order while `expiry_epoch >= domain.epoch`.  No
    /// page knows its own epoch **index** — it stores the 32-byte epoch
    /// identity, which is not invertible — so the horizon is checked exactly
    /// where the index is authenticated: [`EpochAccount::binds_page_set`] and
    /// [`stream::epoch_binds_page_set`] refuse a frozen set holding a live
    /// record already past its expiry.  There is no page-local rule, and none
    /// is claimed.
    pub expiry_epoch: u64,
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
///     // Not a choice: slot 0 of page 0 is rank 1, and nothing else decodes.
///     order_id: canonical_order_id(1),
///     side: 0,
///     active_len: 2,
///     flags: 0,
///     coefficients,
///     lots: 5,
///     limit_collateral_per_lot: 9_000,
///     minimum_fill_lots: 2,
///     generation: 1,
///     expiry_epoch: 7,
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
///     tombstone_count: 0,
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
/// assert_eq!(record.expiry_epoch, 7); //            -> expiry_epoch
///
/// // `canonical_order_id` is now the persisted identity itself: the id decodes
/// // to the record's rank in the page set, which is the relation's coordinate
/// // once retirements are skipped.  `owner` is still the owner-tag preimage;
/// // the tag is interned during the projection walk, not stored here.
/// assert_eq!(order_id_rank(record.order_id), Ok(1));
/// assert_eq!(record.owner, Hash32::from_bytes([20; 32]));
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioRecord {
    /// Position owner identity.
    pub owner: OwnerId,
    /// Canonical order identity: this record's slot rank; see
    /// [`canonical_order_id`].
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
    /// Last epoch index this order may be cleared in, inclusive; see
    /// [`OrderRecord::expiry_epoch`], which this field mirrors exactly.
    pub expiry_epoch: u64,
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
        order_id_rank(self.order_id)?;
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

/// One retired order id: what a cancellation writes.
///
/// A cancellation cannot remove a record.  Removing one would either leave a
/// hole in the slot array — which the dense-page rules forbid — or renumber
/// every later order, and order ids are positional
/// ([`canonical_order_id`]), so renumbering would silently rewrite identities
/// that receipts, candidates, and clients already name.  A retirement instead
/// **replaces the record in place**, keeping its slot and its id, and the page
/// counts it in `order_count` and again in `tombstone_count`.
///
/// The page-set commitment covers a tombstone exactly as it covers a record:
/// the retirement's bytes are slot bytes, so they are in the page digest and
/// therefore in the order-set fold.  A retirement cannot be added, undone, or
/// moved after a freeze without changing `order_set`.
///
/// The relation projection **skips** a tombstone: a retired order has no
/// coordinates to feed and takes no rank among live orders.  See
/// `docs/implementation/SOLANA_LAYOUT.md` for how the skip is recorded in the
/// projection walk's fold, which is what keeps a resumed walk on the same
/// numbering as an unbroken one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TombstoneRecord {
    /// The retired order's canonical identity: the slot's own rank, unchanged.
    pub order_id: Hash32,
    /// The retired record's owner, copied from the record being retired.
    pub owner: OwnerId,
    /// The retired record's replay generation, copied from it.
    pub retired_generation: u64,
    /// The retirement's own replay generation, strictly above the retired one.
    pub generation: u64,
}

impl TombstoneRecord {
    /// Validate a retirement without consulting the page it sits in.
    ///
    /// Identities are nonzero and the order id is a canonical rank; the
    /// retirement strictly follows the placement it retires, which is the one
    /// ordering fact a tombstone can state on its own.  Everything else about a
    /// retirement — that its slot really held a live record by this owner — is
    /// a page fact, checked by [`stream::write_tombstone`] at the moment of
    /// writing and by the slot's own position afterwards.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.owner)?;
        order_id_rank(self.order_id)?;
        if self.generation <= self.retired_generation {
            return Err(CodecError::InvalidEnum);
        }
        Ok(())
    }
}

/// One page slot: canonical padding, a single-Egg record, a portfolio record,
/// or a retirement.
///
/// Every slot occupies exactly [`ORDER_SLOT_BYTES`] bytes — a one-byte kind
/// discriminator, that kind's exact body, and canonical zero padding to the
/// common width — so a page keeps one exact account length no matter which
/// kinds it holds.  Canonical padding is the all-zero slot, which is also
/// [`ORDER_KIND_EMPTY`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderSlot {
    /// Canonical padding: [`ORDER_SLOT_BYTES`] zero bytes, no record.
    Empty,
    /// One single-Egg order on one outcome.
    Single(OrderRecord),
    /// One portfolio order over a coefficient vector.
    Portfolio(PortfolioRecord),
    /// One retired order id; see [`TombstoneRecord`].
    Tombstone(TombstoneRecord),
}

impl OrderSlot {
    /// This slot's kind discriminator byte.
    pub fn kind(&self) -> u8 {
        match self {
            Self::Empty => ORDER_KIND_EMPTY,
            Self::Single(_) => ORDER_KIND_SINGLE,
            Self::Portfolio(_) => ORDER_KIND_PORTFOLIO,
            Self::Tombstone(_) => ORDER_KIND_TOMBSTONE,
        }
    }
    /// The slot's canonical order identity, or zero for padding.
    ///
    /// A retirement answers with the id it retired, which is the whole point of
    /// retiring in place: the id chain does not notice a cancellation.
    pub fn order_id(&self) -> Hash32 {
        match self {
            Self::Empty => Hash32::ZERO,
            Self::Single(o) => o.order_id,
            Self::Portfolio(p) => p.order_id,
            Self::Tombstone(t) => t.order_id,
        }
    }
    /// The slot's owner identity, or zero for padding.
    pub fn owner(&self) -> OwnerId {
        match self {
            Self::Empty => Hash32::ZERO,
            Self::Single(o) => o.owner,
            Self::Portfolio(p) => p.owner,
            Self::Tombstone(t) => t.owner,
        }
    }
    /// The slot's replay generation, or zero for padding.
    pub fn generation(&self) -> u64 {
        match self {
            Self::Empty => 0,
            Self::Single(o) => o.generation,
            Self::Portfolio(p) => p.generation,
            Self::Tombstone(t) => t.generation,
        }
    }
    /// Whether this slot holds a portfolio record.
    pub fn is_portfolio(&self) -> bool {
        matches!(self, Self::Portfolio(_))
    }
    /// Whether this slot holds a retirement.
    pub fn is_tombstone(&self) -> bool {
        matches!(self, Self::Tombstone(_))
    }
    /// Whether this slot holds an order the relation projection will feed.
    ///
    /// Exactly the two order families.  Padding is not a record and a
    /// retirement is not live, so both answer `false`; this is the predicate
    /// the live-order fold and every live count are stated against.
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Single(_) | Self::Portfolio(_))
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
            Self::Tombstone(t) => t.validate(),
        }
    }
}

// The General product reaches these validators from enough distinct hostile
// decoders that SBF's normal inlining duplicates more text than it removes.
// Keep the smaller products' established codegen while sharing one checked
// implementation in the General ELF.
#[cfg_attr(feature = "profile-general-source-v2-point", inline(never))]
fn check_hash(hash: Hash32) -> Result<()> {
    if hash == Hash32::ZERO {
        Err(CodecError::ZeroIdentity)
    } else {
        Ok(())
    }
}

#[cfg_attr(feature = "profile-general-source-v2-point", inline(never))]
fn check_count(count: u8) -> Result<()> {
    if count < 2 || count as usize > MAX_OUTCOMES {
        Err(CodecError::InvalidCount)
    } else {
        Ok(())
    }
}

fn check_create_market_fields(
    realm: Hash32,
    profile: Hash32,
    outcome_count: u8,
    terms: Hash32,
    feed: Hash32,
) -> Result<()> {
    check_count(outcome_count)?;
    check_hash(realm)?;
    check_hash(profile)?;
    check_hash(terms)?;
    check_hash(feed)
}

#[cfg_attr(feature = "profile-general-source-v2-point", inline(never))]
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

#[cfg_attr(feature = "profile-general-source-v2-point", inline(never))]
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
        if self.profile_version != PROFILE_SCHEMA_V2 || self.flags != 0 {
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

impl ProfileAccountV2 {
    /// Validate the exact Profile V2 shape and canonical parent identity.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.profile)?;
        check_hash(self.realm)?;
        check_hash(self.collateral_policy_id)?;
        check_hash(self.adapter_release_id)?;
        if self.version != PROFILE_SCHEMA_V2 || self.flags != PROFILE_FLAG_POLICY_FROZEN {
            return Err(CodecError::InvalidEnum);
        }
        if self.profile
            != canonical_profile_v2_id(self.collateral_policy_id, self.adapter_release_id)?
        {
            return Err(CodecError::NonCanonicalIdentity);
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
        w.hash(self.collateral_policy_id)?;
        w.hash(self.adapter_release_id)?;
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
            collateral_policy_id: r.hash()?,
            adapter_release_id: r.hash()?,
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
    /// Live records on this page: populated slots minus retirements.
    ///
    /// The typed twin of [`stream::OrderPageHeader::live_count`].  Meaningful
    /// after [`Self::validate`], which is where `tombstone_count <=
    /// order_count` is established; the saturation is shape, not a semantic
    /// claim.
    pub const fn live_count(&self) -> u8 {
        self.order_count.saturating_sub(self.tombstone_count)
    }
    /// Recompute this page's digest from its position and slot bytes.
    ///
    /// The slots are streamed into the hash one at a time instead of being
    /// buffered, so recomputing a digest costs one [`ORDER_SLOT_BYTES`] scratch
    /// slot rather than a whole page of stack.  The value is identical to
    /// [`canonical_page_digest`] over the concatenated slot bytes and a test
    /// pins that equality.
    ///
    /// This one keeps the portable fold on every target, deliberately.  The
    /// native `hashv` wrapper takes a slice list, so a syscall form would have
    /// to materialise all [`MAX_ORDERS_PER_PAGE`] re-encoded slots at once —
    /// `MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES` bytes of frame, which alone
    /// exceeds Solana's 4 KiB stack frame.  Streaming one slot at a time is the
    /// only shape that fits, and it is why the on-chain page commitment is
    /// [`stream::streamed_page_digest`] instead: that path *does* use the
    /// syscall, over the raw account bytes it never has to re-encode.  Nothing
    /// on the SBF reach graph calls this method, so the portable implementation
    /// it folds is link-time dead there.
    pub fn recomputed_page_digest(&self) -> Result<Hash32> {
        let mut h = Sha256::new();
        h.update(ORDER_PAGE_DOMAIN);
        h.update(&self.market.0);
        h.update(&self.epoch.0);
        h.update(&self.page_index.to_le_bytes());
        h.update(&[self.order_count, self.tombstone_count]);
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
            || self.tombstone_count > self.order_count
            || (frozen && self.order_count == 0)
        {
            return Err(CodecError::InvalidCount);
        };
        /* Order ids are positional, so a slot's id is never compared with its
         * predecessor's: it is compared with the single value this slot's own
         * position admits.  That is strictly stronger than "strictly
         * increasing" — it refuses a gap as well as a repeat — and it is what
         * removes the caller's choice of id entirely. */
        let base = page_base_rank(self.page_index);
        let mut portfolios = 0usize;
        let mut tombstones = 0u8;
        let mut i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            if i < self.order_count as usize {
                self.orders[i].validate()?;
                if order_id_rank(self.orders[i].order_id())? != base + i as u64 + 1 {
                    return Err(CodecError::NonCanonicalIdentity);
                };
                if self.orders[i].is_portfolio() {
                    portfolios += 1;
                }
                if self.orders[i].is_tombstone() {
                    tombstones += 1;
                }
            } else if self.orders[i] != OrderSlot::Empty {
                return Err(CodecError::NonCanonicalPadding);
            };
            i += 1;
        }
        // The stored retirement count is a fold over the very slots above.
        if tombstones != self.tombstone_count {
            return Err(CodecError::MismatchedBinding);
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
        // Page zero opens the chain; every later page states the rank its own
        // slots start above, which is a fact about its index alone.
        if self.page_index == 0 {
            if self.prev_page_last_order_id != Hash32::ZERO {
                return Err(CodecError::NonCanonicalPadding);
            }
        } else {
            if self.prev_page_last_order_id == Hash32::ZERO {
                return Err(CodecError::ZeroIdentity);
            }
            if self.prev_page_last_order_id != page_base_order_id(self.page_index) {
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
        w.u8(self.tombstone_count)?;
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
        let tombstone_count = r.u8()?;
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
            tombstone_count,
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
                /* A retirement has no limit and no lots; the grid has nothing
                 * to say about one, and the record it retired was checked
                 * against the grid while it was live. */
                OrderSlot::Tombstone(_) => {}
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
    let mut live: u16 = 0;
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
        live = live
            .checked_add((page.order_count - page.tombstone_count) as u16)
            .ok_or(CodecError::ArithmeticOverflow)?;
        digests[i] = page.page_digest;
        i += 1;
    }
    if total != head.set_order_count {
        return Err(CodecError::MismatchedBinding);
    }
    /* A frozen set every one of whose records has been retired is not a book
     * with nothing in it — it is a book with nothing to clear, and the relation
     * has no order feed to build from it.  One live order is the floor, exactly
     * as one populated slot is the floor for a frozen page. */
    if live == 0 {
        return Err(CodecError::InvalidCount);
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
        order_id_rank(self.order_id)?;
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
    w.u64(o.generation)?;
    w.u64(o.expiry_epoch)
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
        expiry_epoch: r.u64()?,
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
    w.u64(p.generation)?;
    w.u64(p.expiry_epoch)
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
        expiry_epoch: r.u64()?,
    })
}
fn encode_tombstone(w: &mut Writer<'_>, t: TombstoneRecord) -> Result<()> {
    w.hash(t.order_id)?;
    w.hash(t.owner)?;
    w.u64(t.retired_generation)?;
    w.u64(t.generation)
}
fn decode_tombstone(r: &mut Reader<'_>) -> Result<TombstoneRecord> {
    Ok(TombstoneRecord {
        order_id: r.hash()?,
        owner: r.hash()?,
        retired_generation: r.u64()?,
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
        OrderSlot::Tombstone(t) => encode_tombstone(w, t)?,
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
        ORDER_KIND_TOMBSTONE => OrderSlot::Tombstone(decode_tombstone(r)?),
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

/// Market-wide supply, decomposed into internal claims and observed bearer
/// supply.
///
/// The reference adapter's closure invariant (CLO-DELTA-V1) is
/// `position internal + accounted external == aggregate supply`.  Summing
/// positions is not an onchain option, so the aggregate is persisted here as
/// the two terms whose sum it is: claims still credited internally, and claims
/// materialized outside the internal ledger.  The external field is only the
/// last Token-2022 mint-supply vector observed atomically by the adapter; the
/// actual mint accounts are authoritative.  A lower actual supply is a direct
/// holder burn and a safe liability donation.  This account is not authority
/// over any holder or token-account balance.
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
    /// Last observed Token-2022 mint supply, per outcome.
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
    /// Registered statistic identity; zero is refused.
    ///
    /// The registry itself (which ids exist, and which are admissible per
    /// degree) is owned by the resolution derivation in
    /// `clutch-solana-reference`; this codec owns only "a statistic was
    /// named".
    pub statistic_id: u16,
    /// Registered ambiguity-policy identity; zero is refused.
    pub ambiguity_policy_id: u8,
    /// Registered edge-policy identity; zero is refused.
    pub edge_policy_id: u8,
    /// B-spline basis degree, in `0..=MAX_BASIS_DEGREE`.
    ///
    /// Degree 0 is the boundary-table market: the knots are interior cell
    /// boundaries and [`TermsAccount::payout_map`] is live.  Degree ≥ 1 is a
    /// derived-basis market: the knots are claim anchors and the payout map
    /// must be entirely [`PAYOUT_MAP_UNUSED`].
    pub basis_degree: u8,
    /// Active knot count `K`; per-degree count rule against `outcome_count`.
    ///
    /// `n = K + 1` at degree 0, `n = K` at degree 1, `n = K − 1 + d` at
    /// degrees 2 and 3 (`DISTRIBUTIONAL_CLAIMS_DESIGN.md` §2.1).
    pub knot_count: u8,
    /// Uniform-spacing declaration: `s` when every active knot gap is `2^s`,
    /// or [`UNIFORM_SPACING_NONE`].
    ///
    /// The knot array is always the single semantic owner; this field is a
    /// validated promise checked against the array, never a second truth.
    /// Degrees ≥ 2 require a uniform declaration.
    pub uniform_log2_spacing: u8,
    /// Preset index of the frozen failure-refund vector; `< payout_count`.
    pub failure_payout_index: u8,
    /// Registered coverage-policy parameter (bounded-gaps bound).
    ///
    /// The coverage registry in `clutch-accumulator` owns its meaning and its
    /// per-policy domain; `COMPLETE_REQUIRED` requires zero.
    pub coverage_policy_parameter: u64,
    /// Repair generation pinned under `GEN-EXACT-01`.
    pub repair_generation: u64,
    /// Source-adapter version the window identity must carry; zero is refused.
    pub source_version: u32,
    /// Statistic-evaluator version the window identity must carry; zero is
    /// refused.
    pub evaluator_version: u32,
    /// Source-adapter identity; replaces the v2 "feed doubles as both" pin.
    pub source_adapter_id: Hash32,
    /// Degree-0 cell-to-preset map; entries at `>= outcome_count` are
    /// [`PAYOUT_MAP_UNUSED`], and every entry is unused for degree ≥ 1.
    pub payout_map: [u8; MAX_OUTCOMES],
    /// Knot vector: strictly increasing active prefix, zero padding.
    pub knots: [u128; MAX_KNOTS],
    /// Per-market collateral cap, in collateral atoms; zero is refused.
    ///
    /// This is the immutable cap `MarketAccount::collateral_cap` is founded
    /// from and checked against — the terms field the collateral-cap finding
    /// of `RESOLUTION_EVIDENCE_PLAN.md` §3.5 demanded.  **Zero refuses at
    /// decode**, which makes "cap 0 refuses at market init" structural: a
    /// terms artifact that made no cap decision cannot exist, so no market
    /// can be founded unfundable-forever.  "Unlimited" must be said out loud
    /// as an explicit large cap, and `collateral::check_market_cap` still
    /// refutes any cap above the Realm's admitted mint ceiling.
    pub collateral_cap: u64,
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
        w.u16(self.statistic_id)?;
        w.u8(self.ambiguity_policy_id)?;
        w.u8(self.edge_policy_id)?;
        w.u8(self.basis_degree)?;
        w.u8(self.knot_count)?;
        w.u8(self.uniform_log2_spacing)?;
        w.u8(self.failure_payout_index)?;
        w.u8(0)?;
        w.u64(self.coverage_policy_parameter)?;
        w.u64(self.repair_generation)?;
        w.u32(self.source_version)?;
        w.u32(self.evaluator_version)?;
        w.hash(self.source_adapter_id)?;
        w.bytes(&self.payout_map)?;
        let mut i = 0;
        while i < MAX_KNOTS {
            w.u128(self.knots[i])?;
            i += 1;
        }
        w.u64(self.collateral_cap)?;
        w.bytes(&[0; 7])?;
        if w.at != TERMS_BODY_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(())
    }
    /// Recompute the terms digest from the current field values.
    ///
    /// `#[inline(never)]` so the body buffer lives in exactly one frame on
    /// the SBF target; see the frame notes in `clutch-sbf`'s modules.
    #[inline(never)]
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
        self.validate_prehashed()?;
        if self.terms != self.recomputed_terms_digest()? {
            return Err(CodecError::NonCanonicalIdentity);
        }
        Ok(())
    }
    /// Every [`TermsAccount::validate`] check except the digest recomputation.
    fn validate_prehashed(&self) -> Result<()> {
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
        /* The v3 resolution-basis fields.  This codec owns byte shape only:
         * strictly increasing knots, per-degree count and payout-map liveness
         * rules, a truthful uniform-spacing declaration, canonical padding,
         * and the freeze-time arithmetic bound.  Which policy/statistic ids
         * are registered — and that degrees 2 and 3 are unimplemented — is
         * the resolution derivation's charter in `clutch-solana-reference`. */
        if self.statistic_id == 0
            || self.ambiguity_policy_id == 0
            || self.edge_policy_id == 0
            || self.source_version == 0
            || self.evaluator_version == 0
        {
            return Err(CodecError::ZeroValue);
        }
        check_hash(self.source_adapter_id)?;
        if self.basis_degree > MAX_BASIS_DEGREE {
            return Err(CodecError::InvalidEnum);
        }
        let claims = self.outcome_count as usize;
        let knots = self.knot_count as usize;
        // §2.1 count rule: n = K + 1 (deg 0), n = K (deg 1), n = K − 1 + d.
        let expected_knots = match self.basis_degree {
            0 => claims - 1,
            1 => claims,
            degree => claims + 1 - degree as usize,
        };
        if knots != expected_knots || knots == 0 || knots > MAX_KNOTS {
            return Err(CodecError::InvalidCount);
        }
        // Degree ≥ 1 evaluation needs at least one pane.
        if self.basis_degree >= 1 && knots < 2 {
            return Err(CodecError::InvalidCount);
        }
        // Strictly increasing active prefix; a degree-0 first boundary must be
        // nonzero (an empty first cell would mint a liability that cannot
        // pay); zero canonical padding beyond the prefix.
        let mut largest_gap: u128 = 0;
        let mut previous: u128 = 0;
        let mut i = 0;
        while i < MAX_KNOTS {
            let knot = self.knots[i];
            if i < knots {
                if i == 0 {
                    if self.basis_degree == 0 && knot == 0 {
                        return Err(CodecError::ZeroValue);
                    }
                } else {
                    if knot <= previous {
                        return Err(CodecError::InvalidCount);
                    }
                    let gap = knot - previous;
                    if gap > largest_gap {
                        largest_gap = gap;
                    }
                }
                previous = knot;
            } else if knot != 0 {
                return Err(CodecError::NonCanonicalPadding);
            }
            i += 1;
        }
        // The uniform declaration is a validated promise about the array,
        // never a second truth; degrees ≥ 2 must declare uniform spacing.
        if self.uniform_log2_spacing == UNIFORM_SPACING_NONE {
            if self.basis_degree >= 2 {
                return Err(CodecError::InvalidEnum);
            }
        } else {
            if self.uniform_log2_spacing >= 128 {
                return Err(CodecError::InvalidEnum);
            }
            let gap: u128 = 1 << self.uniform_log2_spacing;
            let mut i = 1;
            while i < knots {
                if self.knots[i] - self.knots[i - 1] != gap {
                    return Err(CodecError::InvalidEnum);
                }
                i += 1;
            }
        }
        if self.failure_payout_index >= self.payout_count {
            return Err(CodecError::InvalidCount);
        }
        // Payout-map liveness per degree: live and bounded for degree 0,
        // entirely unused for derived-basis markets (they have no map).
        let mut i = 0;
        while i < MAX_OUTCOMES {
            let entry = self.payout_map[i];
            if self.basis_degree == 0 && i < claims {
                if entry >= self.payout_count {
                    return Err(CodecError::InvalidCount);
                }
            } else if entry != PAYOUT_MAP_UNUSED {
                return Err(CodecError::NonCanonicalPadding);
            }
            i += 1;
        }
        if self.collateral_cap == 0 {
            // The cap decision is mandatory: see the field's documentation.
            return Err(CodecError::ZeroValue);
        }
        // Freeze-time arithmetic bound (DISTRIBUTIONAL_CLAIMS_DESIGN.md §2.5):
        // every checked product the weight derivation can form must fit below
        // 2^127, proved here once so the runtime refusal is defense in depth.
        if self.basis_degree >= 1 {
            let d = u128::from(denominator);
            let operand = match self.basis_degree {
                1 => largest_gap - 1,
                degree => {
                    // Uniform spacing is mandatory here, so the gap is 2^s.
                    let h: u128 = 1 << self.uniform_log2_spacing;
                    let h_squared = h.checked_mul(h).ok_or(CodecError::ArithmeticOverflow)?;
                    if degree == 2 {
                        h_squared
                            .checked_mul(2)
                            .ok_or(CodecError::ArithmeticOverflow)?
                    } else {
                        h_squared
                            .checked_mul(h)
                            .ok_or(CodecError::ArithmeticOverflow)?
                            .checked_mul(6)
                            .ok_or(CodecError::ArithmeticOverflow)?
                    }
                }
            };
            let product = d
                .checked_mul(operand)
                .ok_or(CodecError::ArithmeticOverflow)?;
            if product >> 127 != 0 {
                return Err(CodecError::ArithmeticOverflow);
            }
        }
        Ok(())
    }
    /// The binding comparisons of [`TermsAccount::binds_market`] alone.
    ///
    /// For a caller that has already run both accounts' full validation once
    /// in the same atomic context (the terms artifact is presented read-only,
    /// so its bytes cannot move within a transaction) and must not pay the
    /// digest recomputation again.  The comparisons, and the refusal class,
    /// are exactly [`TermsAccount::binds_market`]'s.
    pub fn binds_market_fields(&self, market: &MarketAccount) -> Result<()> {
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
    /// Check that a market's committed terms digest is exactly these terms.
    pub fn binds_market(&self, market: &MarketAccount) -> Result<()> {
        self.validate()?;
        market.validate()?;
        self.binds_market_fields(market)
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
    /// The all-zero placeholder every `*_into` decode overwrites completely.
    ///
    /// Not a valid account (`validate` refuses it); it exists so a decode can
    /// initialize a caller slot without a second account-sized temporary.
    pub const ZEROED: Self = Self {
        terms: Hash32::ZERO,
        realm: Hash32::ZERO,
        profile: Hash32::ZERO,
        feed: Hash32::ZERO,
        price_grid: Hash32::ZERO,
        outcome_count: 0,
        payout_count: 0,
        payouts: [PayoutVectorBytes::ZERO; MAX_PAYOUTS],
        grid_family_id: 0,
        grid_version: 0,
        bucket_seconds: 0,
        expected_start_bucket: 0,
        expected_end_bucket_exclusive: 0,
        maturity_horizon_buckets: 0,
        coverage_policy_id: 0,
        repair_policy_id: 0,
        failure_policy_id: 0,
        statistic_id: 0,
        ambiguity_policy_id: 0,
        edge_policy_id: 0,
        basis_degree: 0,
        knot_count: 0,
        uniform_log2_spacing: 0,
        failure_payout_index: 0,
        coverage_policy_parameter: 0,
        repair_generation: 0,
        source_version: 0,
        evaluator_version: 0,
        source_adapter_id: Hash32::ZERO,
        payout_map: [0; MAX_OUTCOMES],
        knots: [0; MAX_KNOTS],
        collateral_cap: 0,
        stored_bump: 0,
        flags: 0,
    };
    /// Parse exactly [`account_len::TERMS`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut v = Self::ZEROED;
        Self::decode_into(input, &mut v)?;
        Ok(v)
    }
    /// [`TermsAccount::decode`] into a caller-owned slot.
    ///
    /// The account is over 1.6 KiB; on a 4 KiB-frame target a by-value decode
    /// costs the caller two account-sized copies, and this entry point costs
    /// none — the parse writes fields directly into `out`.  On error `out`
    /// holds an unspecified partial parse and must not be read.
    pub fn decode_into(input: &[u8], out: &mut Self) -> Result<()> {
        Self::parse_into(input, out)?;
        out.validate()
    }
    /// Parse exactly [`account_len::TERMS`] bytes **without recomputing the
    /// self-certifying digest**.
    ///
    /// Every check of [`TermsAccount::decode`] runs except the SHA-256 over
    /// the 1,620-byte body, which dominates the decode's cost.  Sound only
    /// over bytes that already passed [`TermsAccount::decode`] once in the
    /// same atomic context — for an on-chain gate, the same transaction, with
    /// the terms account presented read-only so the runtime forbids its bytes
    /// from moving between the two reads.  A caller that has not paid the
    /// full decode once holds unproven bytes: the stored `terms` field is
    /// then a claim, not an identity.
    pub fn decode_unchecked(input: &[u8]) -> Result<Self> {
        let mut v = Self::ZEROED;
        Self::decode_unchecked_into(input, &mut v)?;
        Ok(v)
    }
    /// [`TermsAccount::decode_unchecked`] into a caller-owned slot; the frame
    /// and error contracts of [`TermsAccount::decode_into`] apply.
    pub fn decode_unchecked_into(input: &[u8], out: &mut Self) -> Result<()> {
        Self::parse_into(input, out)?;
        out.validate_prehashed()
    }
    fn parse_into(input: &[u8], out: &mut Self) -> Result<()> {
        let mut r = Reader::new(input, TERMS_TAG, account_version::TERMS, account_len::TERMS)?;
        out.terms = r.hash()?;
        out.realm = r.hash()?;
        out.profile = r.hash()?;
        out.feed = r.hash()?;
        out.price_grid = r.hash()?;
        out.outcome_count = r.u8()?;
        out.payout_count = r.u8()?;
        let mut i = 0;
        while i < MAX_PAYOUTS {
            out.payouts[i] = PayoutVectorBytes {
                denominator: r.u64()?,
                weights: r.amounts()?,
            };
            i += 1;
        }
        out.grid_family_id = r.u32()?;
        out.grid_version = r.u16()?;
        out.bucket_seconds = r.u64()?;
        out.expected_start_bucket = r.u64()?;
        out.expected_end_bucket_exclusive = r.u64()?;
        out.maturity_horizon_buckets = r.u64()?;
        out.coverage_policy_id = r.u32()?;
        out.repair_policy_id = r.u32()?;
        out.failure_policy_id = r.u32()?;
        out.statistic_id = r.u16()?;
        out.ambiguity_policy_id = r.u8()?;
        out.edge_policy_id = r.u8()?;
        out.basis_degree = r.u8()?;
        out.knot_count = r.u8()?;
        out.uniform_log2_spacing = r.u8()?;
        out.failure_payout_index = r.u8()?;
        if r.u8()? != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        out.coverage_policy_parameter = r.u64()?;
        out.repair_generation = r.u64()?;
        out.source_version = r.u32()?;
        out.evaluator_version = r.u32()?;
        out.source_adapter_id = r.hash()?;
        out.payout_map = r.bytes::<{ MAX_OUTCOMES }>()?;
        let mut i = 0;
        while i < MAX_KNOTS {
            out.knots[i] = r.u128()?;
            i += 1;
        }
        out.collateral_cap = r.u64()?;
        let reserved = r.bytes::<7>()?;
        if reserved != [0; 7] {
            return Err(CodecError::NonCanonicalPadding);
        }
        out.stored_bump = r.u8()?;
        out.flags = r.u8()?;
        r.done()
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
    /// Frozen payout-basis degree, copied from [`TermsAccount::basis_degree`]
    /// at epoch open, in `0..=MAX_BASIS_DEGREE`.
    ///
    /// The price plane's moment-cone gate (`clutch_batch::relation_v1`'s V1b,
    /// `DUAL_IS_THE_MEASURE.md` §7.6) needs the basis geometry, and by Lemma
    /// 7.6.1 the cone depends only on `(degree, outcome_count)` — the knot
    /// positions of an admitted grid are an affine reparameterization, and
    /// measures push forward along one.  So the epoch binds one byte, not a
    /// knot vector, and the pair sits adjacent here for exactly that reason.
    ///
    /// Degrees zero and one make V1b the constant true (Corollary 7.6.7), so
    /// every verdict a degree-≤1 market ever reached is unchanged.
    pub basis_degree: u8,
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
        if self.phase > EPOCH_PHASE_LAPSED
            || self.basis_degree > MAX_BASIS_DEGREE
            || self.flags != 0
        {
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
        /* A page alone can bound neither an order's outcome width nor its
         * horizon: it knows `MAX_OUTCOMES` and it stores a 32-byte epoch
         * identity that no page can invert back into an epoch index.  The epoch
         * is the account that names this market's actual width and this book's
         * actual index, so both checks live here.  A retired record is skipped:
         * it will never be fed to the relation, so neither bound applies to it,
         * and refusing a whole frozen book because an order someone cancelled
         * had already expired would be a refusal with no meaning. */
        let mut i = 0;
        while i < pages.len() {
            let mut j = 0;
            while j < pages[i].order_count as usize {
                match pages[i].orders[j] {
                    OrderSlot::Single(o) => {
                        if o.outcome >= self.outcome_count || o.expiry_epoch < self.epoch_index {
                            return Err(CodecError::MismatchedBinding);
                        }
                    }
                    OrderSlot::Portfolio(p) => {
                        if p.active_len > self.outcome_count || p.expiry_epoch < self.epoch_index {
                            return Err(CodecError::MismatchedBinding);
                        }
                    }
                    OrderSlot::Tombstone(_) => {}
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
            || self.basis_degree != terms.basis_degree
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
        w.u8(self.basis_degree)?;
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
            basis_degree: r.u8()?,
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
    /// The verified full-width relation-candidate tie digest (`FullScoreV1`'s
    /// fifth component), stamped by `CompleteClearWork` when the streaming
    /// verifier accepts this candidate.
    ///
    /// Zero exactly while the score is a *claim*: a `SUBMITTED` or `REFUSED`
    /// (or v3-`SUPERSEDED`) record carries no verified identity, and a
    /// `VERIFIED` or `SELECTED` one must carry a nonzero digest — the value
    /// selection's `FullScoreV1::total_order` breaks exact component ties
    /// with.  Never the claimed u128 the feed carries; recomputed on-chain
    /// over the full-width domain, the candidate identity, the fills, and
    /// the declared witness.
    pub score_digest: Hash32,
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
        /* The verified tie digest exists exactly when a verification verdict
         * does: nonzero on VERIFIED and SELECTED, zero everywhere else.  (A
         * later selection lifecycle superseding a verified record zeroes it;
         * a superseded record competes for nothing.) */
        let verified = matches!(
            self.status,
            CANDIDATE_STATUS_VERIFIED | CANDIDATE_STATUS_SELECTED
        );
        if verified == (self.score_digest == Hash32::ZERO) {
            return Err(if verified {
                CodecError::ZeroIdentity
            } else {
                CodecError::NonCanonicalPadding
            });
        }
        if self.candidate != self.recomputed_candidate_digest()? {
            return Err(CodecError::NonCanonicalIdentity);
        }
        Ok(())
    }
    /// Check this candidate against the frozen epoch domain it clears.
    ///
    /// `live_order_count` is the exact number of live (non-retired) records in
    /// the epoch's frozen page set.  The epoch stores only the populated-slot
    /// count (`order_count`, retirements included), while a candidate's
    /// `order_len` names the live orders the relation projection feeds, so the
    /// two agree only on a book nobody cancelled in.  The caller's contract:
    /// recompute `live_order_count` from digest-verified page headers — after
    /// [`stream::epoch_binds_page_set`] (or [`EpochAccount::binds_page_set`])
    /// has bound the complete frozen set to this epoch — never from a
    /// candidate's or feed's own claim.  `tombstone_count` sits inside the
    /// page-digest preimage, so a header that lies about its retirements no
    /// longer matches its own digest and the set binding refuses before this
    /// check is reached.
    pub fn binds_epoch(&self, epoch: &EpochAccount, live_order_count: u16) -> Result<()> {
        self.validate()?;
        epoch.validate()?;
        if epoch.phase == EPOCH_PHASE_OPEN {
            return Err(CodecError::MismatchedBinding);
        }
        // A live count above the populated-slot count is not a value any
        // digest-verified header fold over this epoch's set can produce.
        if self.epoch != epoch.epoch
            || self.market != epoch.market
            || self.outcome_count != epoch.outcome_count
            || self.order_len as u16 != live_order_count
            || live_order_count > epoch.order_count
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
        w.hash(self.score_digest)?;
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
            score_digest: r.hash()?,
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

/// One legacy V2 settlement receipt against one frozen slice of the selected
/// candidate.
///
/// The receipt is the single sequential authority for "how much of this slice
/// has settled"; nothing reconstructs it by combining per-party or per-page
/// views.  Consideration is bound to the frozen price by exact multiplication,
/// so a receipt cannot quietly re-price a slice.
/// General settlement uses the separate hostile
/// [`settlement_receipt_v3::SettlementReceiptAccountV3`] decoder.
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
        // Candidate feeds, the relation, receipt PDAs, and entitlement all use
        // the same witness coordinate. Keep the receipt's admitted index set
        // aligned with that one semantic owner rather than the older
        // pre-portfolio `2 * orders` estimate.
        if self.slice_index as usize >= MAX_SLICES {
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
    /// The binding comparisons of [`ResolutionAccount::binds_terms`] alone.
    ///
    /// For a caller that has already fully validated both accounts once in
    /// the same atomic context and must not pay the terms digest
    /// recomputation again; the comparisons and the refusal classes are
    /// exactly [`ResolutionAccount::binds_terms`]'s.
    pub fn binds_terms_fields(&self, terms: &TermsAccount) -> Result<()> {
        if self.terms != terms.terms || self.feed != terms.feed {
            return Err(CodecError::MismatchedBinding);
        }
        if self.is_resolved() {
            if self.payout_index >= terms.payout_count {
                return Err(CodecError::InvalidCount);
            }
            if self.sealed_end_bucket_exclusive < terms.expected_end_bucket_exclusive {
                return Err(CodecError::MismatchedBinding);
            }
        }
        Ok(())
    }
    /// Check the resolution against the immutable terms it selects from.
    ///
    /// The comparisons — including the rule that a resolution may not precede
    /// the frozen expected range's end — live in
    /// [`ResolutionAccount::binds_terms_fields`], so the two entry points
    /// cannot drift.
    pub fn binds_terms(&self, terms: &TermsAccount) -> Result<()> {
        self.validate()?;
        terms.validate()?;
        self.binds_terms_fields(terms)
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
    /// Burn a bearer Egg after resolution and pay its current holder directly.
    ///
    /// Unlike [`Intent::Dematerialize`], this action is not Position-bound.  The
    /// signed claimant and both token-account addresses are explicit wire
    /// bindings; the Solana adapter must still decode current Token-2022 owner,
    /// mint, and balance facts rather than trusting these names.
    RedeemExternal {
        market: MarketId,
        claimant: OwnerId,
        source: Hash32,
        destination: Hash32,
        outcome: u8,
        quantity: u64,
    },
    /// Advance an authenticated feed cursor by one checked evidence digest.
    FeedAdvance {
        feed: FeedId,
        cursor: u64,
        evidence: Hash32,
    },
    /// Construct one immutable authenticated source spec and its feed head.
    ///
    /// `terms` names the canonical sealed Terms account that supplies Realm,
    /// feed, window and source-release bindings. `spec_body` is decoded by the
    /// registered source codec and must hash to that Terms feed identity.
    InitSourceSpec {
        terms: Hash32,
        spec_body: [u8; SOURCE_SPEC_BODY_V1_BYTES],
    },
    /// Construct the canonical archive for the exact Terms window.
    InitSourceArchive { terms: Hash32 },
    /// Append the uniquely admitted next source record to that archive.
    AppendSourceArchive { terms: Hash32 },
    /// Authenticate maturity, seal the archive and advance its feed head.
    SealSourceArchive { terms: Hash32 },
    /// Append one order to a frozen-shape order page.
    ///
    /// The payload is an [`OrderSlot`] rather than a bare [`OrderRecord`], so
    /// the two admitted order families are both placeable on the wire; the
    /// encoding is the slot's kind byte and that kind's **exact** body, with
    /// none of the slot padding a page carries.
    ///
    /// `slot.order_id()` is not a choice.  Order ids are positional
    /// ([`canonical_order_id`]), so the only id this intent may carry is the
    /// one the target page's state already fixes — the placement path refuses
    /// any other.  Carrying it anyway is what turns a lost race into a refusal:
    /// a caller states the rank it believes it is taking, and a placement that
    /// slipped behind another one is refused instead of quietly landing at a
    /// different rank.
    PlaceOrder {
        market: MarketId,
        epoch: EpochId,
        /// Maximum collateral fee atoms this owner authorizes.
        max_fee_atoms: u64,
        slot: OrderSlot,
    },
    /// Retire one existing order identity, writing a [`TombstoneRecord`].
    ///
    /// `order_id` is a canonical rank, so it names the page and the slot with
    /// no search; `owner` must be the record's own owner; `generation` is the
    /// retirement's replay generation and must be strictly above the retired
    /// record's.
    CancelOrder {
        market: MarketId,
        epoch: EpochId,
        owner: OwnerId,
        order_id: Hash32,
        generation: u64,
    },
    /// Settle one already-verified page.
    SettlePage {
        market: MarketId,
        epoch: EpochId,
        page_index: u16,
    },
    /// Submit the one deterministic direct candidate carried by a frozen page.
    ///
    /// Submission is not verification or selection.  The program creates a
    /// `SUBMITTED` Candidate and its exact fill/slice feed, while the Epoch
    /// remains frozen and no settlement receipt exists yet.
    SubmitDirectPage {
        market: MarketId,
        epoch: EpochId,
        page_index: u16,
    },
    /// Construct a version-three direct Epoch with immutable submission slots.
    InitDirectEpochV3 {
        market: MarketId,
        epoch_index: u64,
        policy: Hash32,
        submission_opens_slot: u64,
        submission_closes_slot: u64,
    },
    /// Freeze the exact one-page/two-order direct book before submission opens.
    FreezeDirectEpochV3 { market: MarketId, epoch: EpochId },
    /// Verify and admit one priced candidate into the streaming direct window.
    SubmitDirectCandidateV2 {
        market: MarketId,
        epoch: EpochId,
        outcome_price: u64,
    },
    /// Close one expired candidate window and freeze its exact best candidate.
    SelectDirectWindowV1 { market: MarketId, epoch: EpochId },
    /// Consume the selected direct entitlement exactly once.
    SettleDirectV2 { market: MarketId, epoch: EpochId },
    /// Begin one deterministic, prepaid occupation-resolution work item.
    BeginResolutionWork(resolution_work::BeginResolutionWorkV1),
    /// Fold the exact next bounded archive chunk into program-owned work.
    FoldResolutionWork(resolution_work::FoldResolutionWorkV1),
    /// Finalize complete work into the sole canonical v4 Resolution.
    FinalizeResolutionWork(resolution_work::FinalizeResolutionWorkV1),
    /// Close work only through one of the narrowly safe abort paths.
    AbortResolutionWork(resolution_work::AbortResolutionWorkV1),
    /// Bring one Realm namespace into existence.
    ///
    /// The Realm identity is not carried, because it is not a choice:
    /// [`canonical_realm_id`] derives it from exactly `(profile,
    /// realm_nonce)`.  `profile` is the **parent** Profile identity, which is
    /// itself a total function of the Realm's collateral policy through
    /// [`collateral::ParentProfile`] — so an adapter holding the 266 policy
    /// bytes can refuse a claimed `profile` that those bytes do not produce,
    /// and this intent is checkable rather than merely well-formed.
    InitRealm {
        profile: ProfileHash,
        realm_nonce: u64,
        max_outcomes: u8,
        profile_version: u8,
    },
    /// Freeze one Realm's canonical Profile V2 identity.
    ///
    /// The policy bytes remain in their one content-authenticated artifact.
    /// This intent carries only the exact policy and compiled-release content
    /// identities that the Profile persists and the adapter recomputes.
    InitProfileV2 {
        realm: RealmHash,
        collateral_policy_id: Hash32,
        adapter_release_id: Hash32,
        profile_version: u8,
    },
    /// Bring one frozen price grid into existence.
    ///
    /// The grid body — 64 ticks and a scale, 521 bytes — rides an
    /// evidence-buffer account for the same reason the terms body does; see
    /// [`Intent::InitTerms`].  A [`PriceGridAccount`] is self-certifying
    /// ([`PriceGridAccount::recomputed_grid_id`]), so the digest this intent
    /// carries is exactly the binding: a buffer whose bytes do not produce
    /// `grid` is refused before anything is created.
    InitPriceGrid { realm: RealmHash, grid: Hash32 },
    /// Bring one immutable terms artifact into existence.
    ///
    /// The terms body is 1,656 bytes and [`MAX_INTENT_BYTES`] is 310; that is
    /// **by design**, not a limitation being worked around.  The intent budget
    /// is the width of the widest *transition*, and an initialization that
    /// carried a whole account would make every instruction's wire format as
    /// wide as the largest artifact anyone might ever found.  So the body
    /// rides an evidence-buffer account and the intent carries the digest
    /// binding, exactly as [`Intent::InitPriceGrid`] does.  A
    /// [`TermsAccount`] is self-certifying too
    /// ([`TermsAccount::recomputed_terms_digest`]), so "the buffer is the
    /// terms this intent names" is a recomputation and not a trust.
    InitTerms { realm: RealmHash, terms: Hash32 },
    /// Bring one order page of a frozen-shape page set into existence.
    ///
    /// The page set's geometry is a decision made once for the whole epoch, so
    /// `page_count` travels with every page rather than being read off the
    /// epoch: an open epoch's own `page_count` is zero until the set freezes
    /// (see [`EpochAccount::page_count`]), which is precisely the window in
    /// which pages are created.  `page_index` is the page's position, and
    /// [`stream::init_page`] derives everything else — the base order id, the
    /// empty range, the digest over sixteen canonical padding slots.
    InitOrderPage {
        market: MarketId,
        epoch: EpochId,
        page_index: u16,
        page_count: u16,
    },
    /// Deposit collateral into pooled custody and credit one position's
    /// internal trading cash.
    ///
    /// The Solana adapter binds this intent to an exact owner-authorized
    /// Token-2022 `TransferChecked` into the market's Hoard token account.
    /// `cash_atoms` is credited only after the observed token deltas match the
    /// requested amount.
    Endow {
        market: MarketId,
        owner: OwnerId,
        amount: u64,
    },
    /// Withdraw one position's unreserved collateral cash to an owner-controlled
    /// Token-2022 account.
    ///
    /// `destination` is an explicit signed wire binding. The Solana adapter
    /// must still decode the account's current mint, owner authority, state,
    /// and extensions before moving any value. Reserved cash is never
    /// withdrawable through this action.
    WithdrawCash {
        market: MarketId,
        owner: OwnerId,
        destination: Hash32,
        amount: u64,
    },
    /// Create one uploader-keyed, exact-size typed artifact stage.
    BeginArtifact {
        kind: artifact::ArtifactKind,
        context: Hash32,
        digest: Hash32,
        exact_len: u16,
        expires_slot: u64,
    },
    /// Append the unique next chunk to a typed artifact stage.
    WriteArtifact {
        kind: artifact::ArtifactKind,
        context: Hash32,
        digest: Hash32,
        cursor: u16,
        chunk_len: u16,
        chunk: [u8; artifact::ARTIFACT_CHUNK_BYTES],
    },
    /// Validate a complete stage and create/admit its final artifact PDA.
    SealArtifact {
        kind: artifact::ArtifactKind,
        context: Hash32,
        digest: Hash32,
        exact_len: u16,
    },
    /// Close an upload, always refunding the funder persisted in its header.
    AbortArtifact {
        kind: artifact::ArtifactKind,
        context: Hash32,
        digest: Hash32,
    },
    /// Begin staged creation of one clearing checkpoint
    /// ([`clearing::ClearWorkAccount`]).
    ///
    /// The checkpoint is the one account in the inventory above the runtime's
    /// 10,240-byte per-instruction growth ceiling (see [`clearing`]), so its
    /// creation is a five-instruction sequence: this intent transfers the
    /// **full final rent principal**, allocates the first
    /// [`clearing::CLEAR_WORK_GROW_STEP`] bytes under the canonical
    /// `(epoch, candidate)` PDA, and writes the resumable grow-stage prefix
    /// ([`clearing::ClearWorkGrowStage`]).  A growing account refuses every
    /// checkpoint reader by exact length until the final
    /// [`Intent::GrowClearWork`] writes the real header and idle body.
    InitClearWork {
        market: MarketId,
        epoch: EpochId,
        candidate: Hash32,
    },
    /// Grow a staged checkpoint by one step; the final step finishes creation.
    ///
    /// Four of these follow one [`Intent::InitClearWork`]; the growth cap is
    /// per *instruction*, so all five may share one transaction but nothing
    /// requires it.  Rent never tops up — the full principal moved at init.
    GrowClearWork {
        market: MarketId,
        epoch: EpochId,
        candidate: Hash32,
    },
    /// Construct one general (portfolio-clearing) [`EpochAccount`] and its
    /// deadline-window companion ([`clearing::EpochWindowAccount`]).
    ///
    /// The general sibling of [`Intent::InitDirectEpochV3`], minus every
    /// `== 2` gate: outcome width and price scale come from the bound terms
    /// and grid, so neither travels on the wire.  `policy` is the immutable
    /// batch-policy identity the epoch clears under; the presented policy
    /// artifact must re-derive it exactly.  The epoch identity is not carried
    /// — [`canonical_epoch_id`] derives it from `(market, epoch_index)`.
    InitEpoch {
        market: MarketId,
        epoch_index: u64,
        policy: Hash32,
        freeze_deadline_slot: u64,
    },
    /// Freeze one general epoch's complete page set, at or after its deadline.
    ///
    /// Permissionless keeper work: the deadline in the epoch's window account
    /// is the authority, not a signer.  Every page of the set rides the same
    /// instruction; the freeze seals each page, stamps the set commitment
    /// into the epoch, and rewrites `owner_count` with the exact
    /// distinct-owner count of the frozen set.
    FreezeEpoch { market: MarketId, epoch: EpochId },
    /// Advance one clearing checkpoint's order pass by up to `max_orders`
    /// live orders from its monotone `(page_cursor, slot_cursor)` position.
    ///
    /// The on-chain streaming walk: each invocation names the one
    /// digest-verified page the cursor sits on, projects its live records
    /// through [`projection`], and feeds them — with their candidate fills
    /// from the bound [`clearing::CandidateFeedAccount`] — into the
    /// checkpoint codec.  On pass 1 every pushed order also presents its
    /// exact ACTIVE reservation, which is what makes pass-1 completion the
    /// reservation-set sweep.  Permissionless keeper work: every authority is
    /// account state.
    AdvanceClearWork {
        market: MarketId,
        epoch: EpochId,
        candidate: Hash32,
        /// Most live orders this invocation may push; at least one.
        max_orders: u16,
    },
    /// Advance one clearing checkpoint's slice pass by up to `max_slices`
    /// declared pairing-witness slices from the feed.
    ///
    /// Reachable only on a checkpoint pass 1 has already bound; the slices
    /// arrive by stored index from the bound feed's declared witness and the
    /// pass closes itself when the last declared slice is fed.
    AdvanceClearSlices {
        market: MarketId,
        epoch: EpochId,
        candidate: Hash32,
        /// Most witness slices this invocation may push; at least one.
        max_slices: u16,
    },
    /// Close one complete clearing checkpoint and persist its verdict.
    ///
    /// Acceptance persists `VERIFIED` plus the recomputed full-width score —
    /// components from the streamed summary, the tie digest recomputed over
    /// the full-width domain and the fed coordinates, never the claimed u128
    /// — onto the [`CandidateRecord`]; a relation refusal persists `REFUSED`.
    /// Either way the checkpoint completes and no pass may resume it.
    CompleteClearWork {
        market: MarketId,
        epoch: EpochId,
        candidate: Hash32,
    },
    /// Begin one general candidate submission: create the `SUBMITTED`
    /// [`CandidateRecord`] and the staging [`clearing::CandidateFeedStage`]
    /// at their canonical `(epoch, candidate)` PDAs.
    ///
    /// The candidate identity does not travel — the program recomputes it
    /// from the free coordinates carried here, exactly as both accounts'
    /// codecs recompute it, and `order_len` comes from the frozen window's
    /// exact live cardinality rather than from any claim.  The score
    /// components carried here are **claims**: they seed the record and the
    /// feed header, they order registry admission at seal time, and
    /// verification overwrites them; selection never reads an unverified one.
    ///
    /// The feed's content — up to 64 fills and 416 witness slices, 6,266
    /// bytes in all — cannot ride one transaction, which is what the wire
    /// demands the staged shape for: content arrives through
    /// [`Intent::WriteCandidateFeed`] chunks and [`Intent::SealCandidate`]
    /// is the one-way door into a consumable feed.
    SubmitCandidate {
        market: MarketId,
        epoch: EpochId,
        /// Exact scaled prices on the simplex; inactive outcomes are zero.
        prices: [u64; MAX_OUTCOMES],
        /// `sigma`: complete sets created by the single global virtual split.
        virtual_split: u64,
        /// `mu`: complete sets destroyed by the single global virtual merge.
        virtual_merge: u64,
        /// Honored minimum-fill subset, one bit per order.
        honored_aon_mask: u64,
        /// Declared explicit pairing-witness length, if one is declared.
        declared_slices: Option<u16>,
        /// Claimed score component 1, net of the self-overlap term.
        weighted_direct_volume: i128,
        /// Claimed score component 3, in exact price units.
        limit_surplus_price_units: u128,
        /// Claimed score component 4: distinct participating owners.
        distinct_owners: u16,
    },
    /// Append one content chunk to a staging candidate feed at its
    /// sequential cursor.
    ///
    /// Fills first, then witness slices — one cursor, one sequence, replay
    /// held by state: the envelope sequence must equal the stage's
    /// elements-written count.  Writes are only possible while the account
    /// carries the stage tag; a sealed feed refuses every one of these by
    /// tag, which is what keeps the streamed bytes and the digest-folded
    /// bytes identical.
    WriteCandidateFeed {
        market: MarketId,
        epoch: EpochId,
        candidate: Hash32,
        chunk: CandidateFeedChunk,
    },
    /// Seal one complete staging feed and admit the candidate into the
    /// window's bounded retained registry.
    ///
    /// The one-way door: the stage prefix becomes the real feed header, the
    /// whole account re-verifies, and the candidate takes a registry slot —
    /// displacing the worst retained candidate when the registry is full,
    /// which closes the displaced candidate's feed in this same transaction
    /// and marks its record `SUPERSEDED` with a zeroed verified digest, or
    /// refusing when the incoming claim cannot beat the worst.
    SealCandidate {
        market: MarketId,
        epoch: EpochId,
        candidate: Hash32,
    },
    /// Close one general epoch's candidate window and select the winner.
    ///
    /// Permissionless at or after the window's selection deadline: compares
    /// **only** `VERIFIED` retained candidates by `FullScoreV1::total_order`
    /// over their persisted verified components and re-derived tie digests —
    /// never a claimed score — stamping the winner `SELECTED` and the epoch
    /// `EPOCH_PHASE_CLEARED`.  With zero verified candidates the epoch
    /// lapses honestly to `EPOCH_PHASE_LAPSED` and nothing is selected.
    FinalizeSelection { market: MarketId, epoch: EpochId },
    /// Create the epoch's [`FinalPotAccount`] from the selected candidate's
    /// **verified** relation summary — the entitlement freeze's opening move.
    ///
    /// Reads the completed clearing checkpoint (its body holds the streamed
    /// verdict) and funds the pot from the summary's rounding/refund scalars.
    /// It now *records* both churn directions and a nonzero rounding pot
    /// rather than refusing them; the consumption seam realizes the residue by
    /// never crediting it, mints `sigma` complete sets after the buyers have
    /// paid, and burns `mu` after the sellers have delivered.  What still
    /// refuses is authority, not churn: a nonzero verified fee scalar.
    FreezeEntitlement {
        market: MarketId,
        epoch: EpochId,
        candidate: Hash32,
    },
    /// Create the [`SettlementReceiptAccount`] entitlements for one witness
    /// slice of the selected candidate — resumable, one slice per invocation.
    ///
    /// A single-Egg pair slice freezes one receipt; a portfolio pair slice
    /// freezes the receipts of **every** slice of that pair at once (the pair
    /// consumes atomically, so its entitlement freezes atomically).  Both
    /// referenced reservations transition `ACTIVE → ENTITLED` in the same
    /// instruction.  Non-full-fill shapes (PartialFillLedger), mixed
    /// single/portfolio pairs, and virtual legs refuse honestly.
    EntitleSlice {
        market: MarketId,
        epoch: EpochId,
        candidate: Hash32,
        /// Index of the frozen witness slice being entitled; for a portfolio
        /// pair this must be the pair's first slice index.
        slice_index: u16,
    },
    /// Release one still-ACTIVE reservation of a terminal (CLEARED or LAPSED)
    /// general epoch back into its owner's Position — TerminalClosure's
    /// economic half, owner-signed.
    ///
    /// On a LAPSED epoch every ACTIVE reservation is releasable.  On a
    /// CLEARED epoch only an order the selected candidate's verified feed
    /// gives a zero fill is releasable — a filled order's envelope is spoken
    /// for by entitlement and consumption, never by release.
    ReleaseTerminalReservation { market: MarketId, epoch: EpochId },
    /// Close one exhausted settlement receipt of a CLEARED general epoch and
    /// return its recorded principal to its recorded payer.
    ///
    /// Economic zero first: an unconsumed receipt refuses.  The recorded
    /// principal comes from the receipt's general funding ledger; surplus
    /// routes to the frozen neutral sink.
    CloseGeneralReceipt {
        market: MarketId,
        epoch: EpochId,
        candidate: Hash32,
        slice_index: u16,
    },
    /// Close one RELEASED or CONSUMED reservation archive of a terminal
    /// general epoch and return its rent to its recorded owner-payer.
    ///
    /// The reservation's own bytes record the payer (`owner` — the placement
    /// actor funds the reservation), so no sibling ledger exists for this
    /// family.  Requires the reservation's page already closed, which is what
    /// keeps the page's reservation sweep provable.
    CloseGeneralReservation { market: MarketId, epoch: EpochId },
    /// Close one frozen order page of a terminal general epoch after proving
    /// every live record's reservation is no longer ACTIVE or ENTITLED.
    CloseGeneralPage {
        market: MarketId,
        epoch: EpochId,
        page_index: u16,
    },
    /// Close the CLEARED epoch's provably empty final pot after every page is
    /// closed (page closure is the executable proof that no entitlement or
    /// consumption remains pending).
    CloseGeneralPot { market: MarketId, epoch: EpochId },
    /// Close one candidate record (and its feed, when still present) of a
    /// terminal general epoch.
    ///
    /// A non-selected candidate closes at terminality.  The SELECTED
    /// candidate's pair closes only after the pot and every page are closed:
    /// its feed carries the fill proofs post-clear releases consume.
    CloseGeneralCandidate {
        market: MarketId,
        epoch: EpochId,
        candidate: Hash32,
    },
    /// Close one clearing checkpoint (complete, refused, or half-grown) of a
    /// terminal general epoch.
    ///
    /// While the SELECTED candidate's record still stands, its checkpoint
    /// closes only once the pot exists (the entitlement freeze consumed the
    /// verdict) or every page is closed — never out from under a pending
    /// freeze.
    CloseGeneralClearWork {
        market: MarketId,
        epoch: EpochId,
        candidate: Hash32,
    },
    /// Close the terminal general epoch and its schedule window together —
    /// the root of the close DAG, admitted only after the pot and every page
    /// are absent and every retained candidate is closed or unclosable.
    CloseGeneralEpoch { market: MarketId, epoch: EpochId },
    /// Close one Realm's revenue-policy record
    /// ([`revenue::RevenuePolicyRecordV1`]) — a TerminalClosure-convention
    /// close (recorded principal to the exact recorded payer, surplus to the
    /// frozen sink), gated on the **Realm account's absence**.
    ///
    /// The Realm row has no close route, so this refusal stands for the
    /// Realm's whole life: the record is Realm-lifetime by construction, and
    /// this intent is what keeps its close *admissible* rather than
    /// unrepresentable (B4f).  There is deliberately no mutate or re-pin
    /// intent beside it — the no-silent-redirect rule (§10.7).
    CloseRevenuePolicyRecord { realm: RealmHash },
    /// Close one owner's [`PositionAccount`] — the owner-signed end of the
    /// position lifecycle, and the byte host of the revenue plane's
    /// mid-epoch-close grief rider.
    ///
    /// Economic zero first: a position holding cash, reserved cash, or any
    /// claim refuses.  Then the rider — a Position the Realm's revenue-policy
    /// record names as its **treasury** is serving fee-bearing epochs, and
    /// closing it mid-epoch would let the fee recipient halt other parties'
    /// settlement, so it refuses while any service is outstanding.  The
    /// Realm's record is presented at its canonical address and is either
    /// absent (the Realm is zero-take by construction, so no treasury exists)
    /// or decoded and compared.
    ///
    /// No principal is paid: the Position family has no creation-side funding
    /// ledger, so no payer is recorded and the ratified convention — exactly
    /// the recorded principal to the exact recorded payer, every other live
    /// lamport burned — pays nothing and burns the whole balance at the frozen
    /// neutral sink.
    ClosePosition { market: MarketId, owner: OwnerId },
    /// Construct one immutable **v2** (pull-profile) source spec and its feed
    /// head.
    ///
    /// The v2 twin of [`Intent::InitSourceSpec`], and deliberately its exact
    /// shape: `terms` names the canonical sealed Terms account that supplies
    /// the Realm, feed, window and source-release bindings, and `spec_body` is
    /// the canonical 368-byte pull body that must hash — under the *v2* feed
    /// domain, never V1's — to that Terms feed identity.
    ///
    /// A separate tag rather than a version field inside the body is what makes
    /// the generations fail closed against each other: a V1 body presented here
    /// does not decode, and a v2 body presented to
    /// [`Intent::InitSourceSpec`] does not fit.
    InitSourceSpecV2 {
        terms: Hash32,
        spec_body: [u8; SOURCE_SPEC_BODY_V2_BYTES],
    },
    /// Construct the canonical **v2** archive for the exact Terms window.
    InitSourceArchiveV2 { terms: Hash32 },
    /// Append the uniquely admitted next pull record to that v2 archive.
    ///
    /// Nothing about the record travels in this intent — not the price, not the
    /// confidence, not the bucket.  The bucket comes from the archive's own
    /// cursor and the value comes from an ephemeral price-update account that
    /// the *immediately preceding* instruction in this same transaction posted
    /// through the pinned receiver program.  That adjacency is read from the
    /// Instructions sysvar, so this intent's whole wire content is which Terms
    /// window is being extended.
    AppendSourceArchiveV2 { terms: Hash32 },
    /// Authenticate maturity, seal the v2 archive and advance its feed head.
    SealSourceArchiveV2 { terms: Hash32 },
}

/// Most fills one [`Intent::WriteCandidateFeed`] chunk may carry.
///
/// Sized so the widest chunked-write intent stays inside [`MAX_INTENT_BYTES`]:
/// a full 64-fill vector is three chunks.
pub const FEED_FILLS_PER_CHUNK: usize = 24;
/// Most witness slices one [`Intent::WriteCandidateFeed`] chunk may carry.
///
/// The full 416-slice witness is twenty-six chunks; the canonical witnesses
/// of small books are one.
pub const FEED_SLICES_PER_CHUNK: usize = 16;

/// One content chunk of a staged candidate-feed submission.
///
/// The array is fixed-width storage for a variable count: elements at and
/// beyond `count` are canonical zero padding in memory and do not travel on
/// the wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateFeedChunk {
    /// The next `count` fills, in live-rank order at the stage cursor.
    Fills {
        /// Live fills in this chunk; `1..=FEED_FILLS_PER_CHUNK`.
        count: u8,
        /// The fills; entries at and beyond `count` are canonical zero.
        fills: [u64; FEED_FILLS_PER_CHUNK],
    },
    /// The next `count` declared witness slices at the stage cursor.
    Slices {
        /// Live slices in this chunk; `1..=FEED_SLICES_PER_CHUNK`.
        count: u8,
        /// The slices; entries at and beyond `count` are canonical padding.
        slices: [clearing::PairingSlice; FEED_SLICES_PER_CHUNK],
    },
}

/// Wire discriminant of a fills chunk.
const FEED_CHUNK_FILLS: u8 = 0;
/// Wire discriminant of a slices chunk.
const FEED_CHUNK_SLICES: u8 = 1;

/// The chunk admissibility rule, shared by the encoder and the decoder.
///
/// What the wire can decide alone: a chunk moves at least one element and at
/// most its width, in-memory padding beyond the count is canonical zero, a
/// live slice is representationally a slice (virtual legs on their admitted
/// sides, a nonzero quantity).  Order-index bounds belong to the staged
/// account and are checked there.
fn check_feed_chunk(chunk: &CandidateFeedChunk) -> Result<()> {
    match chunk {
        CandidateFeedChunk::Fills { count, fills } => {
            if *count == 0 || *count as usize > FEED_FILLS_PER_CHUNK {
                return Err(CodecError::InvalidCount);
            }
            let mut i = *count as usize;
            while i < FEED_FILLS_PER_CHUNK {
                if fills[i] != 0 {
                    return Err(CodecError::NonCanonicalPadding);
                }
                i += 1;
            }
        }
        CandidateFeedChunk::Slices { count, slices } => {
            if *count == 0 || *count as usize > FEED_SLICES_PER_CHUNK {
                return Err(CodecError::InvalidCount);
            }
            let mut i = 0;
            while i < FEED_SLICES_PER_CHUNK {
                let slice = &slices[i];
                if i < *count as usize {
                    if matches!(slice.buy_ref, clearing::LegRef::Split)
                        || matches!(slice.sell_ref, clearing::LegRef::Merge)
                    {
                        return Err(CodecError::InvalidEnum);
                    }
                    if slice.quantity == 0 {
                        return Err(CodecError::ZeroValue);
                    }
                } else if *slice != clearing::PairingSlice::PADDING {
                    return Err(CodecError::NonCanonicalPadding);
                }
                i += 1;
            }
        }
    }
    Ok(())
}

/// The submission-coordinate admissibility rule, shared by both directions.
///
/// Exactly the coordinate rules a wire without the frozen domain can decide:
/// nonzero identities, canonical churn (never split and merge at once, sum in
/// range), a declared witness inside [`MAX_SLICES`], and a distinct-owner
/// claim inside the book bound.  Simplex membership and mask width need the
/// frozen epoch and are the account plane's.
fn check_submit_candidate_shape(
    market: MarketId,
    epoch: EpochId,
    virtual_split: u64,
    virtual_merge: u64,
    declared_slices: Option<u16>,
    distinct_owners: u16,
) -> Result<()> {
    check_hash(market)?;
    check_hash(epoch)?;
    if virtual_split != 0 && virtual_merge != 0 {
        return Err(CodecError::InvalidEnum);
    }
    virtual_split
        .checked_add(virtual_merge)
        .ok_or(CodecError::ArithmeticOverflow)?;
    if let Some(declared) = declared_slices {
        if declared as usize > MAX_SLICES {
            return Err(CodecError::InvalidCount);
        }
    }
    if distinct_owners as usize > MAX_EPOCH_ORDERS {
        return Err(CodecError::InvalidCount);
    }
    Ok(())
}

/// The placement admissibility rule, shared by the encoder and the decoder.
///
/// A placement carries an order, and only an order.  Padding is not an order,
/// and a retirement is what a cancellation writes rather than something a
/// caller may place, so both are [`CodecError::InvalidEnum`] — the kind byte is
/// recognized, it simply names something that is not a placement.
fn check_placement(slot: &OrderSlot) -> Result<()> {
    match slot {
        OrderSlot::Single(o) => o.validate(),
        OrderSlot::Portfolio(p) => p.validate(),
        OrderSlot::Empty | OrderSlot::Tombstone(_) => Err(CodecError::InvalidEnum),
    }
}

/// The Realm-initialization admissibility rule, shared by both directions.
///
/// V1 freezes a Realm's outcome width at exactly [`MAX_OUTCOMES`], so an
/// intent claiming any other width names a Realm [`RealmAccount::validate`]
/// would refuse — better to refuse it on the wire than to create an account
/// that cannot decode.  A zero `profile_version` is the same kind of fault in
/// the other field.
fn check_realm_shape(profile: ProfileHash, max_outcomes: u8, profile_version: u8) -> Result<()> {
    check_hash(profile)?;
    if max_outcomes as usize != MAX_OUTCOMES {
        return Err(CodecError::InvalidCount);
    }
    if profile_version != PROFILE_SCHEMA_V2 {
        return Err(CodecError::InvalidEnum);
    }
    Ok(())
}

/// The Profile-initialization admissibility rule, shared by both directions.
///
/// Both descendants are exact content identities and the version is canonical.
fn check_profile_shape(
    realm: RealmHash,
    collateral_policy_id: Hash32,
    adapter_release_id: Hash32,
    profile_version: u8,
) -> Result<()> {
    check_hash(realm)?;
    check_hash(collateral_policy_id)?;
    check_hash(adapter_release_id)?;
    if profile_version != PROFILE_SCHEMA_V2 {
        return Err(CodecError::InvalidEnum);
    }
    Ok(())
}

/// The page-creation geometry rule, shared by both directions.
///
/// A page set is at most [`MAX_ORDER_PAGES`] pages and at least one, and a
/// page's index is a position inside its own set.  Both are refusals on the
/// wire because both are facts about a geometry that is chosen once: a page
/// created outside its set's declared width could never be closed by
/// [`verify_page_set`].
fn check_page_geometry(page_index: u16, page_count: u16) -> Result<()> {
    if page_count == 0 || page_count as usize > MAX_ORDER_PAGES || page_index >= page_count {
        return Err(CodecError::InvalidCount);
    }
    Ok(())
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
const INIT_REALM_TAG: u8 = 10;
const INIT_PROFILE_TAG: u8 = 11;
const INIT_PRICE_GRID_TAG: u8 = 12;
const INIT_TERMS_TAG: u8 = 13;
const INIT_ORDER_PAGE_TAG: u8 = 14;
const ENDOW_TAG: u8 = 15;
const REDEEM_EXTERNAL_TAG: u8 = 16;
const WITHDRAW_CASH_TAG: u8 = 17;
const BEGIN_ARTIFACT_TAG: u8 = 18;
const WRITE_ARTIFACT_TAG: u8 = 19;
const SEAL_ARTIFACT_TAG: u8 = 20;
const ABORT_ARTIFACT_TAG: u8 = 21;
const SUBMIT_DIRECT_PAGE_TAG: u8 = 22;
const INIT_SOURCE_SPEC_TAG: u8 = 23;
const INIT_SOURCE_ARCHIVE_TAG: u8 = 24;
const APPEND_SOURCE_ARCHIVE_TAG: u8 = 25;
const SEAL_SOURCE_ARCHIVE_TAG: u8 = 26;
const INIT_DIRECT_EPOCH_V3_TAG: u8 = 27;
const FREEZE_DIRECT_EPOCH_V3_TAG: u8 = 28;
const SUBMIT_DIRECT_CANDIDATE_V2_TAG: u8 = 29;
const SELECT_DIRECT_WINDOW_V1_TAG: u8 = 30;
const SETTLE_DIRECT_V2_TAG: u8 = 31;
/* Tags 32-35 are the resolution-work family (its own codec, dispatched above
 * `Reader::new` in `Intent::decode`); tags 36-46 are the Direct V3 family,
 * decoded only by the dedicated `DirectV3Request` envelope and refused here
 * (`direct_selection_v3::LAST_DIRECT_V3_INTENT_TAG`).  The Tier 2 clearing
 * family begins immediately after it. */
const INIT_CLEAR_WORK_TAG: u8 = 47;
const GROW_CLEAR_WORK_TAG: u8 = 48;
const INIT_EPOCH_TAG: u8 = 49;
const FREEZE_EPOCH_TAG: u8 = 50;
const ADVANCE_CLEAR_WORK_TAG: u8 = 51;
const ADVANCE_CLEAR_SLICES_TAG: u8 = 52;
const COMPLETE_CLEAR_WORK_TAG: u8 = 53;
const SUBMIT_CANDIDATE_TAG: u8 = 54;
const WRITE_CANDIDATE_FEED_TAG: u8 = 55;
const SEAL_CANDIDATE_TAG: u8 = 56;
const FINALIZE_SELECTION_TAG: u8 = 57;
const FREEZE_ENTITLEMENT_TAG: u8 = 58;
const ENTITLE_SLICE_TAG: u8 = 59;
const _: () = assert!(INIT_CLEAR_WORK_TAG == direct_selection_v3::LAST_DIRECT_V3_INTENT_TAG + 1);
// The Tier 2 clearing family is tag-contiguous: each intent takes exactly the
// next tag, so a gap is a compile error rather than a squatting hazard.
const _: () = assert!(INIT_EPOCH_TAG == GROW_CLEAR_WORK_TAG + 1);
const _: () = assert!(FREEZE_EPOCH_TAG == INIT_EPOCH_TAG + 1);
const _: () = assert!(ADVANCE_CLEAR_WORK_TAG == FREEZE_EPOCH_TAG + 1);
const _: () = assert!(ADVANCE_CLEAR_SLICES_TAG == ADVANCE_CLEAR_WORK_TAG + 1);
const _: () = assert!(COMPLETE_CLEAR_WORK_TAG == ADVANCE_CLEAR_SLICES_TAG + 1);
const _: () = assert!(SUBMIT_CANDIDATE_TAG == COMPLETE_CLEAR_WORK_TAG + 1);
const _: () = assert!(WRITE_CANDIDATE_FEED_TAG == SUBMIT_CANDIDATE_TAG + 1);
const _: () = assert!(SEAL_CANDIDATE_TAG == WRITE_CANDIDATE_FEED_TAG + 1);
const _: () = assert!(FINALIZE_SELECTION_TAG == SEAL_CANDIDATE_TAG + 1);
const _: () = assert!(FREEZE_ENTITLEMENT_TAG == FINALIZE_SELECTION_TAG + 1);
const _: () = assert!(ENTITLE_SLICE_TAG == FREEZE_ENTITLEMENT_TAG + 1);
/* The TerminalClosure family (release + dependency-ordered rent closes for
 * the general clearing plane) continues the same contiguous ladder: each
 * intent takes exactly the next tag after the entitlement freeze pair. */
const RELEASE_TERMINAL_RESERVATION_TAG: u8 = 60;
const CLOSE_GENERAL_RECEIPT_TAG: u8 = 61;
const CLOSE_GENERAL_RESERVATION_TAG: u8 = 62;
const CLOSE_GENERAL_PAGE_TAG: u8 = 63;
const CLOSE_GENERAL_POT_TAG: u8 = 64;
const CLOSE_GENERAL_CANDIDATE_TAG: u8 = 65;
const CLOSE_GENERAL_CLEAR_WORK_TAG: u8 = 66;
const CLOSE_GENERAL_EPOCH_TAG: u8 = 67;
const _: () = assert!(RELEASE_TERMINAL_RESERVATION_TAG == ENTITLE_SLICE_TAG + 1);
const _: () = assert!(CLOSE_GENERAL_RECEIPT_TAG == RELEASE_TERMINAL_RESERVATION_TAG + 1);
const _: () = assert!(CLOSE_GENERAL_RESERVATION_TAG == CLOSE_GENERAL_RECEIPT_TAG + 1);
const _: () = assert!(CLOSE_GENERAL_PAGE_TAG == CLOSE_GENERAL_RESERVATION_TAG + 1);
const _: () = assert!(CLOSE_GENERAL_POT_TAG == CLOSE_GENERAL_PAGE_TAG + 1);
const _: () = assert!(CLOSE_GENERAL_CANDIDATE_TAG == CLOSE_GENERAL_POT_TAG + 1);
const _: () = assert!(CLOSE_GENERAL_CLEAR_WORK_TAG == CLOSE_GENERAL_CANDIDATE_TAG + 1);
const _: () = assert!(CLOSE_GENERAL_EPOCH_TAG == CLOSE_GENERAL_CLEAR_WORK_TAG + 1);
/* The revenue plane's one close continues the ladder. */
const CLOSE_REVENUE_POLICY_RECORD_TAG: u8 = 68;
const CLOSE_POSITION_TAG: u8 = 69;
/* The v2 authenticated-source family continues the ladder, in the same order
 * its V1 twin took at tags 23-26: construct the spec, construct the archive,
 * append, seal.  Four tags rather than one nested step byte, because that is
 * what makes each step's *account plane* a property of the wire tag — the
 * append's eleven-account pull plane and the construction's payer/System plane
 * are not the same instruction wearing a discriminant. */
const INIT_SOURCE_SPEC_V2_TAG: u8 = 70;
const INIT_SOURCE_ARCHIVE_V2_TAG: u8 = 71;
const APPEND_SOURCE_ARCHIVE_V2_TAG: u8 = 72;
const SEAL_SOURCE_ARCHIVE_V2_TAG: u8 = 73;
const _: () = assert!(INIT_SOURCE_SPEC_V2_TAG == CLOSE_POSITION_TAG + 1);
const _: () = assert!(INIT_SOURCE_ARCHIVE_V2_TAG == INIT_SOURCE_SPEC_V2_TAG + 1);
const _: () = assert!(APPEND_SOURCE_ARCHIVE_V2_TAG == INIT_SOURCE_ARCHIVE_V2_TAG + 1);
const _: () = assert!(SEAL_SOURCE_ARCHIVE_V2_TAG == APPEND_SOURCE_ARCHIVE_V2_TAG + 1);
/* The v2 spec construction is the widest admitted intent; the portfolio
 * placement it displaced is still checked against the bound rather than
 * assumed to fit under it. */
const _: () = assert!(2 + HASH_BYTES + SOURCE_SPEC_BODY_V2_BYTES == MAX_INTENT_BYTES);
const _: () = assert!(2 + (2 * HASH_BYTES) + 8 + 1 + PORTFOLIO_RECORD_BYTES <= MAX_INTENT_BYTES);
const _: () = assert!(2 + HASH_BYTES + SOURCE_SPEC_BODY_V1_BYTES <= MAX_INTENT_BYTES);
const _: () = assert!(CLOSE_REVENUE_POLICY_RECORD_TAG == CLOSE_GENERAL_EPOCH_TAG + 1);
// The widest chunked write stays inside the frozen intent bound; the slices
// shape is the widest at 308 of 310 bytes.
const _: () = assert!(
    2 + (3 * HASH_BYTES) + 1 + 1 + (FEED_SLICES_PER_CHUNK * clearing::PAIRING_SLICE_BYTES)
        <= MAX_INTENT_BYTES
);
const _: () =
    assert!(2 + (3 * HASH_BYTES) + 1 + 1 + (FEED_FILLS_PER_CHUNK * 8) <= MAX_INTENT_BYTES);

fn resolution_work_codec(error: resolution_work::ResolutionWorkCodecError) -> CodecError {
    use resolution_work::ResolutionWorkCodecError as WorkError;
    match error {
        WorkError::Truncated => CodecError::Truncated,
        WorkError::TrailingBytes => CodecError::TrailingBytes,
        WorkError::OutputTooSmall => CodecError::OutputTooSmall,
        WorkError::WrongTag => CodecError::WrongTag,
        WorkError::WrongVersion => CodecError::WrongVersion,
        WorkError::ZeroIdentity => CodecError::ZeroIdentity,
        WorkError::InvalidEnum => CodecError::InvalidEnum,
        WorkError::InvalidCount | WorkError::InvalidWindow => CodecError::InvalidCount,
        WorkError::MismatchedBinding => CodecError::MismatchedBinding,
        WorkError::NonCanonicalPadding => CodecError::NonCanonicalPadding,
        WorkError::ArithmeticOverflow => CodecError::ArithmeticOverflow,
        WorkError::Underfunded => CodecError::ZeroValue,
    }
}

impl Intent {
    /// Return the exact encoded byte length for this intent.
    pub const fn encoded_len(&self) -> usize {
        match self {
            Self::CreateMarket { .. } => 2 + 32 + 32 + 8 + 1 + 32 + 32,
            Self::Split { .. } | Self::Merge { .. } => 2 + 32 + 32 + 8,
            Self::Materialize { .. } | Self::Dematerialize { .. } => 2 + 32 + 32 + 32 + 1 + 8,
            Self::RedeemExternal { .. } => 2 + 32 + 32 + 32 + 32 + 1 + 8,
            Self::FeedAdvance { .. } => 2 + 32 + 8 + 32,
            Self::InitSourceSpec { .. } => 2 + 32 + SOURCE_SPEC_BODY_V1_BYTES,
            Self::InitSourceArchive { .. }
            | Self::AppendSourceArchive { .. }
            | Self::SealSourceArchive { .. } => 2 + 32,
            Self::PlaceOrder { slot, .. } => match slot {
                OrderSlot::Single(_) => 2 + 32 + 32 + 8 + 1 + ORDER_RECORD_BYTES,
                OrderSlot::Portfolio(_) => 2 + 32 + 32 + 8 + 1 + PORTFOLIO_RECORD_BYTES,
                /* Neither padding nor a retirement is a placement; `encode`
                 * refuses both before this length could be used to write. */
                OrderSlot::Empty | OrderSlot::Tombstone(_) => 2 + 32 + 32 + 8 + 1,
            },
            Self::CancelOrder { .. } => 2 + 32 + 32 + 32 + 32 + 8,
            Self::SettlePage { .. } => 2 + 32 + 32 + 2,
            Self::SubmitDirectPage { .. } => 2 + 32 + 32 + 2,
            Self::InitDirectEpochV3 { .. } => 2 + 32 + 8 + 32 + 8 + 8,
            Self::FreezeDirectEpochV3 { .. }
            | Self::SelectDirectWindowV1 { .. }
            | Self::SettleDirectV2 { .. } => 2 + 32 + 32,
            Self::SubmitDirectCandidateV2 { .. } => 2 + 32 + 32 + 8,
            Self::BeginResolutionWork(value) => value.encoded_len(),
            Self::FoldResolutionWork(value) => value.encoded_len(),
            Self::FinalizeResolutionWork(value) => value.encoded_len(),
            Self::AbortResolutionWork(value) => value.encoded_len(),
            Self::InitRealm { .. } => 2 + 32 + 8 + 1 + 1,
            Self::InitProfileV2 { .. } => 2 + 32 + 32 + 32 + 1,
            Self::InitPriceGrid { .. } | Self::InitTerms { .. } => 2 + 32 + 32,
            Self::InitOrderPage { .. } => 2 + 32 + 32 + 2 + 2,
            Self::Endow { .. } => 2 + 32 + 32 + 8,
            Self::WithdrawCash { .. } => 2 + 32 + 32 + 32 + 8,
            Self::BeginArtifact { .. } => 2 + 1 + 32 + 32 + 2 + 8,
            Self::WriteArtifact { .. } => 2 + 1 + 32 + 32 + 2 + 2 + artifact::ARTIFACT_CHUNK_BYTES,
            Self::SealArtifact { .. } => 2 + 1 + 32 + 32 + 2,
            Self::AbortArtifact { .. } => 2 + 1 + 32 + 32,
            Self::InitClearWork { .. } | Self::GrowClearWork { .. } => 2 + 32 + 32 + 32,
            Self::InitEpoch { .. } => 2 + 32 + 8 + 32 + 8,
            Self::FreezeEpoch { .. } => 2 + 32 + 32,
            Self::AdvanceClearWork { .. } | Self::AdvanceClearSlices { .. } => 2 + 32 + 32 + 32 + 2,
            Self::CompleteClearWork { .. } => 2 + 32 + 32 + 32,
            Self::SubmitCandidate { .. } => {
                2 + 32 + 32 + (MAX_OUTCOMES * 8) + 8 + 8 + 8 + 1 + 2 + 16 + 16 + 2
            }
            Self::WriteCandidateFeed { chunk, .. } => {
                2 + 32
                    + 32
                    + 32
                    + 1
                    + 1
                    + match chunk {
                        CandidateFeedChunk::Fills { count, .. } => *count as usize * 8,
                        CandidateFeedChunk::Slices { count, .. } => {
                            *count as usize * clearing::PAIRING_SLICE_BYTES
                        }
                    }
            }
            Self::SealCandidate { .. } => 2 + 32 + 32 + 32,
            Self::FinalizeSelection { .. } => 2 + 32 + 32,
            Self::FreezeEntitlement { .. } => 2 + 32 + 32 + 32,
            Self::EntitleSlice { .. } => 2 + 32 + 32 + 32 + 2,
            Self::ReleaseTerminalReservation { .. }
            | Self::CloseGeneralReservation { .. }
            | Self::CloseGeneralPot { .. }
            | Self::CloseGeneralEpoch { .. } => 2 + 32 + 32,
            Self::CloseGeneralPage { .. } => 2 + 32 + 32 + 2,
            Self::CloseGeneralCandidate { .. } | Self::CloseGeneralClearWork { .. } => {
                2 + 32 + 32 + 32
            }
            Self::CloseGeneralReceipt { .. } => 2 + 32 + 32 + 32 + 2,
            Self::CloseRevenuePolicyRecord { .. } => 2 + 32,
            Self::ClosePosition { .. } => 2 + 32 + 32,
            Self::InitSourceSpecV2 { .. } => 2 + 32 + SOURCE_SPEC_BODY_V2_BYTES,
            Self::InitSourceArchiveV2 { .. }
            | Self::AppendSourceArchiveV2 { .. }
            | Self::SealSourceArchiveV2 { .. } => 2 + 32,
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
                check_create_market_fields(*realm, *profile, *outcome_count, *terms, *feed)?;
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
            Self::RedeemExternal {
                market,
                claimant,
                source,
                destination,
                outcome,
                quantity,
            } => {
                check_hash(*market)?;
                check_hash(*claimant)?;
                check_hash(*source)?;
                check_hash(*destination)?;
                if *outcome >= MAX_OUTCOMES as u8 {
                    return Err(CodecError::InvalidCount);
                }
                if *quantity == 0 {
                    return Err(CodecError::ZeroValue);
                }
                put_header(&mut w, REDEEM_EXTERNAL_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*claimant)?;
                w.hash(*source)?;
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
            Self::InitSourceSpec { terms, spec_body } => {
                check_hash(*terms)?;
                put_header(&mut w, INIT_SOURCE_SPEC_TAG, INTENT_VERSION)?;
                w.hash(*terms)?;
                w.bytes(spec_body)?
            }
            Self::InitSourceArchive { terms }
            | Self::AppendSourceArchive { terms }
            | Self::SealSourceArchive { terms } => {
                check_hash(*terms)?;
                let tag = match self {
                    Self::InitSourceArchive { .. } => INIT_SOURCE_ARCHIVE_TAG,
                    Self::AppendSourceArchive { .. } => APPEND_SOURCE_ARCHIVE_TAG,
                    Self::SealSourceArchive { .. } => SEAL_SOURCE_ARCHIVE_TAG,
                    _ => return Err(CodecError::InvalidEnum),
                };
                put_header(&mut w, tag, INTENT_VERSION)?;
                w.hash(*terms)?
            }
            Self::PlaceOrder {
                market,
                epoch,
                max_fee_atoms,
                slot,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                check_placement(slot)?;
                put_header(&mut w, PLACE_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.u64(*max_fee_atoms)?;
                w.u8(slot.kind())?;
                match slot {
                    OrderSlot::Single(o) => encode_order(&mut w, *o)?,
                    OrderSlot::Portfolio(p) => encode_portfolio(&mut w, *p)?,
                    /* Unreachable: `check_placement` refused every other kind
                     * above.  Stated, not assumed. */
                    OrderSlot::Empty | OrderSlot::Tombstone(_) => {
                        return Err(CodecError::InvalidEnum)
                    }
                }
            }
            Self::CancelOrder {
                market,
                epoch,
                owner,
                order_id,
                generation,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                check_hash(*owner)?;
                order_id_rank(*order_id)?;
                put_header(&mut w, CANCEL_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.hash(*owner)?;
                w.hash(*order_id)?;
                w.u64(*generation)?
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
            Self::SubmitDirectPage {
                market,
                epoch,
                page_index,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                put_header(&mut w, SUBMIT_DIRECT_PAGE_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.u16(*page_index)?
            }
            Self::InitDirectEpochV3 {
                market,
                epoch_index,
                policy,
                submission_opens_slot,
                submission_closes_slot,
            } => {
                check_hash(*market)?;
                check_hash(*policy)?;
                if submission_opens_slot >= submission_closes_slot {
                    return Err(CodecError::InvalidCount);
                }
                put_header(&mut w, INIT_DIRECT_EPOCH_V3_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.u64(*epoch_index)?;
                w.hash(*policy)?;
                w.u64(*submission_opens_slot)?;
                w.u64(*submission_closes_slot)?
            }
            Self::FreezeDirectEpochV3 { market, epoch } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                put_header(&mut w, FREEZE_DIRECT_EPOCH_V3_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?
            }
            Self::SubmitDirectCandidateV2 {
                market,
                epoch,
                outcome_price,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                if *outcome_price == 0 {
                    return Err(CodecError::ZeroValue);
                }
                put_header(&mut w, SUBMIT_DIRECT_CANDIDATE_V2_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.u64(*outcome_price)?
            }
            Self::SelectDirectWindowV1 { market, epoch } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                put_header(&mut w, SELECT_DIRECT_WINDOW_V1_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?
            }
            Self::SettleDirectV2 { market, epoch } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                put_header(&mut w, SETTLE_DIRECT_V2_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?
            }
            Self::BeginResolutionWork(value) => {
                return value.encode(out).map_err(resolution_work_codec)
            }
            Self::FoldResolutionWork(value) => {
                return value.encode(out).map_err(resolution_work_codec)
            }
            Self::FinalizeResolutionWork(value) => {
                return value.encode(out).map_err(resolution_work_codec)
            }
            Self::AbortResolutionWork(value) => {
                return value.encode(out).map_err(resolution_work_codec)
            }
            Self::InitRealm {
                profile,
                realm_nonce,
                max_outcomes,
                profile_version,
            } => {
                check_realm_shape(*profile, *max_outcomes, *profile_version)?;
                put_header(&mut w, INIT_REALM_TAG, INTENT_VERSION)?;
                w.hash(*profile)?;
                w.u64(*realm_nonce)?;
                w.u8(*max_outcomes)?;
                w.u8(*profile_version)?
            }
            Self::InitProfileV2 {
                realm,
                collateral_policy_id,
                adapter_release_id,
                profile_version,
            } => {
                check_profile_shape(
                    *realm,
                    *collateral_policy_id,
                    *adapter_release_id,
                    *profile_version,
                )?;
                put_header(&mut w, INIT_PROFILE_TAG, INTENT_VERSION)?;
                w.hash(*realm)?;
                w.hash(*collateral_policy_id)?;
                w.hash(*adapter_release_id)?;
                w.u8(*profile_version)?
            }
            Self::InitPriceGrid {
                realm,
                grid: artifact,
            }
            | Self::InitTerms {
                realm,
                terms: artifact,
            } => {
                check_hash(*realm)?;
                check_hash(*artifact)?;
                put_header(
                    &mut w,
                    if matches!(self, Self::InitPriceGrid { .. }) {
                        INIT_PRICE_GRID_TAG
                    } else {
                        INIT_TERMS_TAG
                    },
                    INTENT_VERSION,
                )?;
                w.hash(*realm)?;
                w.hash(*artifact)?
            }
            Self::InitOrderPage {
                market,
                epoch,
                page_index,
                page_count,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                check_page_geometry(*page_index, *page_count)?;
                put_header(&mut w, INIT_ORDER_PAGE_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.u16(*page_index)?;
                w.u16(*page_count)?
            }
            Self::Endow {
                market,
                owner,
                amount,
            } => {
                check_hash(*market)?;
                check_hash(*owner)?;
                if *amount == 0 {
                    return Err(CodecError::ZeroValue);
                };
                put_header(&mut w, ENDOW_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*owner)?;
                w.u64(*amount)?
            }
            Self::WithdrawCash {
                market,
                owner,
                destination,
                amount,
            } => {
                check_hash(*market)?;
                check_hash(*owner)?;
                check_hash(*destination)?;
                if *amount == 0 {
                    return Err(CodecError::ZeroValue);
                }
                put_header(&mut w, WITHDRAW_CASH_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*owner)?;
                w.hash(*destination)?;
                w.u64(*amount)?
            }
            Self::BeginArtifact {
                kind,
                context,
                digest,
                exact_len,
                expires_slot,
            } => {
                artifact::ArtifactBinding {
                    kind: *kind,
                    context: *context,
                    digest: *digest,
                    exact_len: *exact_len,
                }
                .validate()?;
                put_header(&mut w, BEGIN_ARTIFACT_TAG, INTENT_VERSION)?;
                w.u8(kind.byte())?;
                w.hash(*context)?;
                w.hash(*digest)?;
                w.u16(*exact_len)?;
                w.u64(*expires_slot)?
            }
            Self::WriteArtifact {
                kind,
                context,
                digest,
                cursor,
                chunk_len,
                chunk,
            } => {
                artifact::ArtifactBinding {
                    kind: *kind,
                    context: *context,
                    digest: *digest,
                    exact_len: kind.exact_len() as u16,
                }
                .validate()?;
                if *chunk_len == 0
                    || usize::from(*chunk_len) > artifact::ARTIFACT_CHUNK_BYTES
                    || chunk[usize::from(*chunk_len)..]
                        .iter()
                        .any(|byte| *byte != 0)
                {
                    return Err(CodecError::NonCanonicalPadding);
                }
                put_header(&mut w, WRITE_ARTIFACT_TAG, INTENT_VERSION)?;
                w.u8(kind.byte())?;
                w.hash(*context)?;
                w.hash(*digest)?;
                w.u16(*cursor)?;
                w.u16(*chunk_len)?;
                w.bytes(chunk)?
            }
            Self::SealArtifact {
                kind,
                context,
                digest,
                exact_len,
            } => {
                artifact::ArtifactBinding {
                    kind: *kind,
                    context: *context,
                    digest: *digest,
                    exact_len: *exact_len,
                }
                .validate()?;
                put_header(&mut w, SEAL_ARTIFACT_TAG, INTENT_VERSION)?;
                w.u8(kind.byte())?;
                w.hash(*context)?;
                w.hash(*digest)?;
                w.u16(*exact_len)?
            }
            Self::AbortArtifact {
                kind,
                context,
                digest,
            } => {
                artifact::ArtifactBinding {
                    kind: *kind,
                    context: *context,
                    digest: *digest,
                    exact_len: kind.exact_len() as u16,
                }
                .validate()?;
                put_header(&mut w, ABORT_ARTIFACT_TAG, INTENT_VERSION)?;
                w.u8(kind.byte())?;
                w.hash(*context)?;
                w.hash(*digest)?
            }
            Self::InitClearWork {
                market,
                epoch,
                candidate,
            }
            | Self::GrowClearWork {
                market,
                epoch,
                candidate,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                check_hash(*candidate)?;
                put_header(
                    &mut w,
                    if matches!(self, Self::InitClearWork { .. }) {
                        INIT_CLEAR_WORK_TAG
                    } else {
                        GROW_CLEAR_WORK_TAG
                    },
                    INTENT_VERSION,
                )?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.hash(*candidate)?
            }
            Self::InitEpoch {
                market,
                epoch_index,
                policy,
                freeze_deadline_slot,
            } => {
                check_hash(*market)?;
                check_hash(*policy)?;
                // A window with no deadline is not a window; the account codec
                // refuses the same zero (`EpochWindowAccount::validate`).
                if *freeze_deadline_slot == 0 {
                    return Err(CodecError::ZeroValue);
                }
                put_header(&mut w, INIT_EPOCH_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.u64(*epoch_index)?;
                w.hash(*policy)?;
                w.u64(*freeze_deadline_slot)?
            }
            Self::FreezeEpoch { market, epoch } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                put_header(&mut w, FREEZE_EPOCH_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?
            }
            Self::AdvanceClearWork {
                market,
                epoch,
                candidate,
                max_orders,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                check_hash(*candidate)?;
                // A walk that may push nothing is not a walk, and a bound
                // above the book is a bound the book already imposes.
                if *max_orders == 0 || *max_orders as usize > MAX_EPOCH_ORDERS {
                    return Err(CodecError::InvalidCount);
                }
                put_header(&mut w, ADVANCE_CLEAR_WORK_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.hash(*candidate)?;
                w.u16(*max_orders)?
            }
            Self::AdvanceClearSlices {
                market,
                epoch,
                candidate,
                max_slices,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                check_hash(*candidate)?;
                // Same rule at the witness bound: at least one, at most the
                // feed's own capacity.
                if *max_slices == 0 || *max_slices as usize > MAX_SLICES {
                    return Err(CodecError::InvalidCount);
                }
                put_header(&mut w, ADVANCE_CLEAR_SLICES_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.hash(*candidate)?;
                w.u16(*max_slices)?
            }
            Self::CompleteClearWork {
                market,
                epoch,
                candidate,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                check_hash(*candidate)?;
                put_header(&mut w, COMPLETE_CLEAR_WORK_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.hash(*candidate)?
            }
            Self::SubmitCandidate {
                market,
                epoch,
                prices,
                virtual_split,
                virtual_merge,
                honored_aon_mask,
                declared_slices,
                weighted_direct_volume,
                limit_surplus_price_units,
                distinct_owners,
            } => {
                check_submit_candidate_shape(
                    *market,
                    *epoch,
                    *virtual_split,
                    *virtual_merge,
                    *declared_slices,
                    *distinct_owners,
                )?;
                put_header(&mut w, SUBMIT_CANDIDATE_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.amounts(prices)?;
                w.u64(*virtual_split)?;
                w.u64(*virtual_merge)?;
                w.u64(*honored_aon_mask)?;
                // The witness declaration mirrors the feed's flag-plus-count:
                // "no witness" and "a witness of zero slices" are different
                // candidates and both are representable.
                w.u8(u8::from(declared_slices.is_some()))?;
                w.u16(declared_slices.unwrap_or(0))?;
                w.i128(*weighted_direct_volume)?;
                w.u128(*limit_surplus_price_units)?;
                w.u16(*distinct_owners)?
            }
            Self::WriteCandidateFeed {
                market,
                epoch,
                candidate,
                chunk,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                check_hash(*candidate)?;
                check_feed_chunk(chunk)?;
                put_header(&mut w, WRITE_CANDIDATE_FEED_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.hash(*candidate)?;
                match chunk {
                    CandidateFeedChunk::Fills { count, fills } => {
                        w.u8(FEED_CHUNK_FILLS)?;
                        w.u8(*count)?;
                        let mut i = 0;
                        while i < *count as usize {
                            w.u64(fills[i])?;
                            i += 1;
                        }
                    }
                    CandidateFeedChunk::Slices { count, slices } => {
                        w.u8(FEED_CHUNK_SLICES)?;
                        w.u8(*count)?;
                        let mut i = 0;
                        while i < *count as usize {
                            let slice = &slices[i];
                            w.u8(slice.buy_ref.kind())?;
                            w.u8(slice.buy_ref.index())?;
                            w.u8(slice.sell_ref.kind())?;
                            w.u8(slice.sell_ref.index())?;
                            w.u8(slice.outcome)?;
                            w.u64(slice.quantity)?;
                            i += 1;
                        }
                    }
                }
            }
            Self::SealCandidate {
                market,
                epoch,
                candidate,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                check_hash(*candidate)?;
                put_header(&mut w, SEAL_CANDIDATE_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.hash(*candidate)?
            }
            Self::FinalizeSelection { market, epoch } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                put_header(&mut w, FINALIZE_SELECTION_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?
            }
            Self::FreezeEntitlement {
                market,
                epoch,
                candidate,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                check_hash(*candidate)?;
                put_header(&mut w, FREEZE_ENTITLEMENT_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.hash(*candidate)?
            }
            Self::EntitleSlice {
                market,
                epoch,
                candidate,
                slice_index,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                check_hash(*candidate)?;
                put_header(&mut w, ENTITLE_SLICE_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.hash(*candidate)?;
                w.u16(*slice_index)?
            }
            Self::ReleaseTerminalReservation { market, epoch }
            | Self::CloseGeneralReservation { market, epoch }
            | Self::CloseGeneralPot { market, epoch }
            | Self::CloseGeneralEpoch { market, epoch } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                let tag = match self {
                    Self::ReleaseTerminalReservation { .. } => RELEASE_TERMINAL_RESERVATION_TAG,
                    Self::CloseGeneralReservation { .. } => CLOSE_GENERAL_RESERVATION_TAG,
                    Self::CloseGeneralPot { .. } => CLOSE_GENERAL_POT_TAG,
                    _ => CLOSE_GENERAL_EPOCH_TAG,
                };
                put_header(&mut w, tag, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?
            }
            Self::CloseGeneralPage {
                market,
                epoch,
                page_index,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                put_header(&mut w, CLOSE_GENERAL_PAGE_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.u16(*page_index)?
            }
            Self::CloseGeneralCandidate {
                market,
                epoch,
                candidate,
            }
            | Self::CloseGeneralClearWork {
                market,
                epoch,
                candidate,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                check_hash(*candidate)?;
                let tag = if matches!(self, Self::CloseGeneralCandidate { .. }) {
                    CLOSE_GENERAL_CANDIDATE_TAG
                } else {
                    CLOSE_GENERAL_CLEAR_WORK_TAG
                };
                put_header(&mut w, tag, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.hash(*candidate)?
            }
            Self::CloseGeneralReceipt {
                market,
                epoch,
                candidate,
                slice_index,
            } => {
                check_hash(*market)?;
                check_hash(*epoch)?;
                check_hash(*candidate)?;
                put_header(&mut w, CLOSE_GENERAL_RECEIPT_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*epoch)?;
                w.hash(*candidate)?;
                w.u16(*slice_index)?
            }
            Self::CloseRevenuePolicyRecord { realm } => {
                check_hash(*realm)?;
                put_header(&mut w, CLOSE_REVENUE_POLICY_RECORD_TAG, INTENT_VERSION)?;
                w.hash(*realm)?
            }
            Self::ClosePosition { market, owner } => {
                check_hash(*market)?;
                check_hash(*owner)?;
                put_header(&mut w, CLOSE_POSITION_TAG, INTENT_VERSION)?;
                w.hash(*market)?;
                w.hash(*owner)?
            }
            Self::InitSourceSpecV2 { terms, spec_body } => {
                check_hash(*terms)?;
                put_header(&mut w, INIT_SOURCE_SPEC_V2_TAG, INTENT_VERSION)?;
                w.hash(*terms)?;
                w.bytes(spec_body)?
            }
            Self::InitSourceArchiveV2 { terms }
            | Self::AppendSourceArchiveV2 { terms }
            | Self::SealSourceArchiveV2 { terms } => {
                check_hash(*terms)?;
                let tag = match self {
                    Self::InitSourceArchiveV2 { .. } => INIT_SOURCE_ARCHIVE_V2_TAG,
                    Self::AppendSourceArchiveV2 { .. } => APPEND_SOURCE_ARCHIVE_V2_TAG,
                    Self::SealSourceArchiveV2 { .. } => SEAL_SOURCE_ARCHIVE_V2_TAG,
                    _ => return Err(CodecError::InvalidEnum),
                };
                put_header(&mut w, tag, INTENT_VERSION)?;
                w.hash(*terms)?
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
        match tag {
            #[cfg(feature = "profile-full")]
            resolution_work::BEGIN_RESOLUTION_WORK_TAG => {
                return resolution_work::BeginResolutionWorkV1::decode(input)
                    .map(Self::BeginResolutionWork)
                    .map_err(resolution_work_codec)
            }
            #[cfg(feature = "profile-full")]
            resolution_work::FOLD_RESOLUTION_WORK_TAG => {
                return resolution_work::FoldResolutionWorkV1::decode(input)
                    .map(Self::FoldResolutionWork)
                    .map_err(resolution_work_codec)
            }
            #[cfg(feature = "profile-full")]
            resolution_work::FINALIZE_RESOLUTION_WORK_TAG => {
                return resolution_work::FinalizeResolutionWorkV1::decode(input)
                    .map(Self::FinalizeResolutionWork)
                    .map_err(resolution_work_codec)
            }
            #[cfg(feature = "profile-full")]
            resolution_work::ABORT_RESOLUTION_WORK_TAG => {
                return resolution_work::AbortResolutionWorkV1::decode(input)
                    .map(Self::AbortResolutionWork)
                    .map_err(resolution_work_codec)
            }
            _ => {}
        }
        let mut r = Reader::new(input, tag, INTENT_VERSION, input.len())?;
        match tag {
            CREATE_TAG => {
                let realm = r.hash()?;
                let profile = r.hash()?;
                let market_nonce = r.u64()?;
                let outcome_count = r.u8()?;
                let terms = r.hash()?;
                let feed = r.hash()?;
                r.done()?;
                check_create_market_fields(realm, profile, outcome_count, terms, feed)?;
                Ok(Self::CreateMarket {
                    realm,
                    profile,
                    market_nonce,
                    outcome_count,
                    terms,
                    feed,
                })
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
            REDEEM_EXTERNAL_TAG => {
                let market = r.hash()?;
                let claimant = r.hash()?;
                let source = r.hash()?;
                let destination = r.hash()?;
                let outcome = r.u8()?;
                let quantity = r.u64()?;
                r.done()?;
                check_hash(market)?;
                check_hash(claimant)?;
                check_hash(source)?;
                check_hash(destination)?;
                if outcome >= MAX_OUTCOMES as u8 {
                    return Err(CodecError::InvalidCount);
                }
                if quantity == 0 {
                    return Err(CodecError::ZeroValue);
                }
                Ok(Self::RedeemExternal {
                    market,
                    claimant,
                    source,
                    destination,
                    outcome,
                    quantity,
                })
            }
            #[cfg(feature = "profile-full")]
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
            #[cfg(feature = "profile-full")]
            INIT_SOURCE_SPEC_TAG => {
                let terms = r.hash()?;
                let spec_body = r.bytes::<SOURCE_SPEC_BODY_V1_BYTES>()?;
                r.done()?;
                check_hash(terms)?;
                Ok(Self::InitSourceSpec { terms, spec_body })
            }
            #[cfg(feature = "profile-full")]
            INIT_SOURCE_ARCHIVE_TAG | APPEND_SOURCE_ARCHIVE_TAG | SEAL_SOURCE_ARCHIVE_TAG => {
                let terms = r.hash()?;
                r.done()?;
                check_hash(terms)?;
                Ok(match tag {
                    INIT_SOURCE_ARCHIVE_TAG => Self::InitSourceArchive { terms },
                    APPEND_SOURCE_ARCHIVE_TAG => Self::AppendSourceArchive { terms },
                    SEAL_SOURCE_ARCHIVE_TAG => Self::SealSourceArchive { terms },
                    _ => return Err(CodecError::InvalidEnum),
                })
            }
            INIT_SOURCE_SPEC_V2_TAG => {
                let terms = r.hash()?;
                let spec_body = r.bytes::<SOURCE_SPEC_BODY_V2_BYTES>()?;
                r.done()?;
                check_hash(terms)?;
                Ok(Self::InitSourceSpecV2 { terms, spec_body })
            }
            INIT_SOURCE_ARCHIVE_V2_TAG
            | APPEND_SOURCE_ARCHIVE_V2_TAG
            | SEAL_SOURCE_ARCHIVE_V2_TAG => {
                let terms = r.hash()?;
                r.done()?;
                check_hash(terms)?;
                Ok(match tag {
                    INIT_SOURCE_ARCHIVE_V2_TAG => Self::InitSourceArchiveV2 { terms },
                    APPEND_SOURCE_ARCHIVE_V2_TAG => Self::AppendSourceArchiveV2 { terms },
                    SEAL_SOURCE_ARCHIVE_V2_TAG => Self::SealSourceArchiveV2 { terms },
                    _ => return Err(CodecError::InvalidEnum),
                })
            }
            PLACE_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let max_fee_atoms = r.u64()?;
                let slot = match r.u8()? {
                    ORDER_KIND_SINGLE => OrderSlot::Single(decode_order(&mut r)?),
                    ORDER_KIND_PORTFOLIO => OrderSlot::Portfolio(decode_portfolio(&mut r)?),
                    /* Padding and a retirement are real slot kinds that are not
                     * placements, so they are refused as a reserved value in
                     * this position; any other byte is no slot kind at all. */
                    ORDER_KIND_EMPTY | ORDER_KIND_TOMBSTONE => return Err(CodecError::InvalidEnum),
                    _ => return Err(CodecError::WrongTag),
                };
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                check_placement(&slot)?;
                Ok(Self::PlaceOrder {
                    market,
                    epoch,
                    max_fee_atoms,
                    slot,
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            CANCEL_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let owner = r.hash()?;
                let order_id = r.hash()?;
                let generation = r.u64()?;
                check_hash(market)?;
                check_hash(epoch)?;
                check_hash(owner)?;
                order_id_rank(order_id)?;
                let v = Self::CancelOrder {
                    market,
                    epoch,
                    owner,
                    order_id,
                    generation,
                };
                r.done()?;
                Ok(v)
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
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
            #[cfg(feature = "profile-full")]
            SUBMIT_DIRECT_PAGE_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let page_index = r.u16()?;
                check_hash(market)?;
                check_hash(epoch)?;
                let v = Self::SubmitDirectPage {
                    market,
                    epoch,
                    page_index,
                };
                r.done()?;
                Ok(v)
            }
            #[cfg(feature = "profile-full")]
            INIT_DIRECT_EPOCH_V3_TAG => {
                let market = r.hash()?;
                let epoch_index = r.u64()?;
                let policy = r.hash()?;
                let submission_opens_slot = r.u64()?;
                let submission_closes_slot = r.u64()?;
                r.done()?;
                check_hash(market)?;
                check_hash(policy)?;
                if submission_opens_slot >= submission_closes_slot {
                    return Err(CodecError::InvalidCount);
                }
                Ok(Self::InitDirectEpochV3 {
                    market,
                    epoch_index,
                    policy,
                    submission_opens_slot,
                    submission_closes_slot,
                })
            }
            #[cfg(feature = "profile-full")]
            FREEZE_DIRECT_EPOCH_V3_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                Ok(Self::FreezeDirectEpochV3 { market, epoch })
            }
            #[cfg(feature = "profile-full")]
            SUBMIT_DIRECT_CANDIDATE_V2_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let outcome_price = r.u64()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                if outcome_price == 0 {
                    return Err(CodecError::ZeroValue);
                }
                Ok(Self::SubmitDirectCandidateV2 {
                    market,
                    epoch,
                    outcome_price,
                })
            }
            #[cfg(feature = "profile-full")]
            SELECT_DIRECT_WINDOW_V1_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                Ok(Self::SelectDirectWindowV1 { market, epoch })
            }
            #[cfg(feature = "profile-full")]
            SETTLE_DIRECT_V2_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                Ok(Self::SettleDirectV2 { market, epoch })
            }
            INIT_REALM_TAG => {
                let profile = r.hash()?;
                let realm_nonce = r.u64()?;
                let max_outcomes = r.u8()?;
                let profile_version = r.u8()?;
                r.done()?;
                check_realm_shape(profile, max_outcomes, profile_version)?;
                Ok(Self::InitRealm {
                    profile,
                    realm_nonce,
                    max_outcomes,
                    profile_version,
                })
            }
            INIT_PROFILE_TAG => {
                let realm = r.hash()?;
                let collateral_policy_id = r.hash()?;
                let adapter_release_id = r.hash()?;
                let profile_version = r.u8()?;
                r.done()?;
                check_profile_shape(
                    realm,
                    collateral_policy_id,
                    adapter_release_id,
                    profile_version,
                )?;
                Ok(Self::InitProfileV2 {
                    realm,
                    collateral_policy_id,
                    adapter_release_id,
                    profile_version,
                })
            }
            INIT_PRICE_GRID_TAG | INIT_TERMS_TAG => {
                let realm = r.hash()?;
                let artifact = r.hash()?;
                r.done()?;
                check_hash(realm)?;
                check_hash(artifact)?;
                Ok(if tag == INIT_PRICE_GRID_TAG {
                    Self::InitPriceGrid {
                        realm,
                        grid: artifact,
                    }
                } else {
                    Self::InitTerms {
                        realm,
                        terms: artifact,
                    }
                })
            }
            INIT_ORDER_PAGE_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let page_index = r.u16()?;
                let page_count = r.u16()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                check_page_geometry(page_index, page_count)?;
                Ok(Self::InitOrderPage {
                    market,
                    epoch,
                    page_index,
                    page_count,
                })
            }
            ENDOW_TAG => {
                let market = r.hash()?;
                let owner = r.hash()?;
                let amount = r.u64()?;
                r.done()?;
                check_hash(market)?;
                check_hash(owner)?;
                if amount == 0 {
                    return Err(CodecError::ZeroValue);
                };
                Ok(Self::Endow {
                    market,
                    owner,
                    amount,
                })
            }
            WITHDRAW_CASH_TAG => {
                let market = r.hash()?;
                let owner = r.hash()?;
                let destination = r.hash()?;
                let amount = r.u64()?;
                r.done()?;
                check_hash(market)?;
                check_hash(owner)?;
                check_hash(destination)?;
                if amount == 0 {
                    return Err(CodecError::ZeroValue);
                }
                Ok(Self::WithdrawCash {
                    market,
                    owner,
                    destination,
                    amount,
                })
            }
            BEGIN_ARTIFACT_TAG => {
                let kind = artifact::ArtifactKind::from_byte(r.u8()?)?;
                let context = r.hash()?;
                let digest = r.hash()?;
                let exact_len = r.u16()?;
                let expires_slot = r.u64()?;
                r.done()?;
                artifact::ArtifactBinding {
                    kind,
                    context,
                    digest,
                    exact_len,
                }
                .validate()?;
                Ok(Self::BeginArtifact {
                    kind,
                    context,
                    digest,
                    exact_len,
                    expires_slot,
                })
            }
            WRITE_ARTIFACT_TAG => {
                let kind = artifact::ArtifactKind::from_byte(r.u8()?)?;
                let context = r.hash()?;
                let digest = r.hash()?;
                let cursor = r.u16()?;
                let chunk_len = r.u16()?;
                let chunk = r.bytes::<{ artifact::ARTIFACT_CHUNK_BYTES }>()?;
                r.done()?;
                artifact::ArtifactBinding {
                    kind,
                    context,
                    digest,
                    exact_len: kind.exact_len() as u16,
                }
                .validate()?;
                if chunk_len == 0
                    || usize::from(chunk_len) > artifact::ARTIFACT_CHUNK_BYTES
                    || chunk[usize::from(chunk_len)..]
                        .iter()
                        .any(|byte| *byte != 0)
                {
                    return Err(CodecError::NonCanonicalPadding);
                }
                Ok(Self::WriteArtifact {
                    kind,
                    context,
                    digest,
                    cursor,
                    chunk_len,
                    chunk,
                })
            }
            SEAL_ARTIFACT_TAG => {
                let kind = artifact::ArtifactKind::from_byte(r.u8()?)?;
                let context = r.hash()?;
                let digest = r.hash()?;
                let exact_len = r.u16()?;
                r.done()?;
                artifact::ArtifactBinding {
                    kind,
                    context,
                    digest,
                    exact_len,
                }
                .validate()?;
                Ok(Self::SealArtifact {
                    kind,
                    context,
                    digest,
                    exact_len,
                })
            }
            ABORT_ARTIFACT_TAG => {
                let kind = artifact::ArtifactKind::from_byte(r.u8()?)?;
                let context = r.hash()?;
                let digest = r.hash()?;
                r.done()?;
                artifact::ArtifactBinding {
                    kind,
                    context,
                    digest,
                    exact_len: kind.exact_len() as u16,
                }
                .validate()?;
                Ok(Self::AbortArtifact {
                    kind,
                    context,
                    digest,
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            INIT_CLEAR_WORK_TAG | GROW_CLEAR_WORK_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let candidate = r.hash()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                check_hash(candidate)?;
                Ok(if tag == INIT_CLEAR_WORK_TAG {
                    Self::InitClearWork {
                        market,
                        epoch,
                        candidate,
                    }
                } else {
                    Self::GrowClearWork {
                        market,
                        epoch,
                        candidate,
                    }
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            INIT_EPOCH_TAG => {
                let market = r.hash()?;
                let epoch_index = r.u64()?;
                let policy = r.hash()?;
                let freeze_deadline_slot = r.u64()?;
                r.done()?;
                check_hash(market)?;
                check_hash(policy)?;
                if freeze_deadline_slot == 0 {
                    return Err(CodecError::ZeroValue);
                }
                Ok(Self::InitEpoch {
                    market,
                    epoch_index,
                    policy,
                    freeze_deadline_slot,
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            FREEZE_EPOCH_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                Ok(Self::FreezeEpoch { market, epoch })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            ADVANCE_CLEAR_WORK_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let candidate = r.hash()?;
                let max_orders = r.u16()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                check_hash(candidate)?;
                if max_orders == 0 || max_orders as usize > MAX_EPOCH_ORDERS {
                    return Err(CodecError::InvalidCount);
                }
                Ok(Self::AdvanceClearWork {
                    market,
                    epoch,
                    candidate,
                    max_orders,
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            ADVANCE_CLEAR_SLICES_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let candidate = r.hash()?;
                let max_slices = r.u16()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                check_hash(candidate)?;
                if max_slices == 0 || max_slices as usize > MAX_SLICES {
                    return Err(CodecError::InvalidCount);
                }
                Ok(Self::AdvanceClearSlices {
                    market,
                    epoch,
                    candidate,
                    max_slices,
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            COMPLETE_CLEAR_WORK_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let candidate = r.hash()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                check_hash(candidate)?;
                Ok(Self::CompleteClearWork {
                    market,
                    epoch,
                    candidate,
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            SUBMIT_CANDIDATE_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let prices = r.amounts()?;
                let virtual_split = r.u64()?;
                let virtual_merge = r.u64()?;
                let honored_aon_mask = r.u64()?;
                let witness_flag = r.u8()?;
                let declared = r.u16()?;
                let weighted_direct_volume = r.i128()?;
                let limit_surplus_price_units = r.u128()?;
                let distinct_owners = r.u16()?;
                r.done()?;
                let declared_slices = match witness_flag {
                    0 => {
                        // An undeclared witness has no length; the count byte
                        // pair is padding and padding is canonical zero.
                        if declared != 0 {
                            return Err(CodecError::NonCanonicalPadding);
                        }
                        None
                    }
                    1 => Some(declared),
                    _ => return Err(CodecError::InvalidEnum),
                };
                check_submit_candidate_shape(
                    market,
                    epoch,
                    virtual_split,
                    virtual_merge,
                    declared_slices,
                    distinct_owners,
                )?;
                Ok(Self::SubmitCandidate {
                    market,
                    epoch,
                    prices,
                    virtual_split,
                    virtual_merge,
                    honored_aon_mask,
                    declared_slices,
                    weighted_direct_volume,
                    limit_surplus_price_units,
                    distinct_owners,
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            WRITE_CANDIDATE_FEED_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let candidate = r.hash()?;
                let kind = r.u8()?;
                let count = r.u8()?;
                let chunk = match kind {
                    FEED_CHUNK_FILLS => {
                        if count == 0 || count as usize > FEED_FILLS_PER_CHUNK {
                            return Err(CodecError::InvalidCount);
                        }
                        let mut fills = [0u64; FEED_FILLS_PER_CHUNK];
                        let mut i = 0;
                        while i < count as usize {
                            fills[i] = r.u64()?;
                            i += 1;
                        }
                        CandidateFeedChunk::Fills { count, fills }
                    }
                    FEED_CHUNK_SLICES => {
                        if count == 0 || count as usize > FEED_SLICES_PER_CHUNK {
                            return Err(CodecError::InvalidCount);
                        }
                        let mut slices = [clearing::PairingSlice::PADDING; FEED_SLICES_PER_CHUNK];
                        let mut i = 0;
                        while i < count as usize {
                            let buy_kind = r.u8()?;
                            let buy_index = r.u8()?;
                            let sell_kind = r.u8()?;
                            let sell_index = r.u8()?;
                            let outcome = r.u8()?;
                            let quantity = r.u64()?;
                            slices[i] = clearing::PairingSlice {
                                buy_ref: clearing::decode_leg(buy_kind, buy_index)?,
                                sell_ref: clearing::decode_leg(sell_kind, sell_index)?,
                                outcome,
                                quantity,
                            };
                            i += 1;
                        }
                        CandidateFeedChunk::Slices { count, slices }
                    }
                    _ => return Err(CodecError::InvalidEnum),
                };
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                check_hash(candidate)?;
                check_feed_chunk(&chunk)?;
                Ok(Self::WriteCandidateFeed {
                    market,
                    epoch,
                    candidate,
                    chunk,
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            SEAL_CANDIDATE_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let candidate = r.hash()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                check_hash(candidate)?;
                Ok(Self::SealCandidate {
                    market,
                    epoch,
                    candidate,
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            FINALIZE_SELECTION_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                Ok(Self::FinalizeSelection { market, epoch })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            FREEZE_ENTITLEMENT_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let candidate = r.hash()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                check_hash(candidate)?;
                Ok(Self::FreezeEntitlement {
                    market,
                    epoch,
                    candidate,
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            ENTITLE_SLICE_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let candidate = r.hash()?;
                let slice_index = r.u16()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                check_hash(candidate)?;
                Ok(Self::EntitleSlice {
                    market,
                    epoch,
                    candidate,
                    slice_index,
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            RELEASE_TERMINAL_RESERVATION_TAG
            | CLOSE_GENERAL_RESERVATION_TAG
            | CLOSE_GENERAL_POT_TAG
            | CLOSE_GENERAL_EPOCH_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                Ok(match tag {
                    RELEASE_TERMINAL_RESERVATION_TAG => {
                        Self::ReleaseTerminalReservation { market, epoch }
                    }
                    CLOSE_GENERAL_RESERVATION_TAG => {
                        Self::CloseGeneralReservation { market, epoch }
                    }
                    CLOSE_GENERAL_POT_TAG => Self::CloseGeneralPot { market, epoch },
                    _ => Self::CloseGeneralEpoch { market, epoch },
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            CLOSE_GENERAL_PAGE_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let page_index = r.u16()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                Ok(Self::CloseGeneralPage {
                    market,
                    epoch,
                    page_index,
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            CLOSE_GENERAL_CANDIDATE_TAG | CLOSE_GENERAL_CLEAR_WORK_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let candidate = r.hash()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                check_hash(candidate)?;
                Ok(if tag == CLOSE_GENERAL_CANDIDATE_TAG {
                    Self::CloseGeneralCandidate {
                        market,
                        epoch,
                        candidate,
                    }
                } else {
                    Self::CloseGeneralClearWork {
                        market,
                        epoch,
                        candidate,
                    }
                })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            CLOSE_GENERAL_RECEIPT_TAG => {
                let market = r.hash()?;
                let epoch = r.hash()?;
                let candidate = r.hash()?;
                let slice_index = r.u16()?;
                r.done()?;
                check_hash(market)?;
                check_hash(epoch)?;
                check_hash(candidate)?;
                Ok(Self::CloseGeneralReceipt {
                    market,
                    epoch,
                    candidate,
                    slice_index,
                })
            }
            CLOSE_REVENUE_POLICY_RECORD_TAG => {
                let realm = r.hash()?;
                r.done()?;
                check_hash(realm)?;
                Ok(Self::CloseRevenuePolicyRecord { realm })
            }
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            CLOSE_POSITION_TAG => {
                let market = r.hash()?;
                let owner = r.hash()?;
                r.done()?;
                check_hash(market)?;
                check_hash(owner)?;
                Ok(Self::ClosePosition { market, owner })
            }
            _ => Err(CodecError::WrongTag),
        }
    }
}

/* Minimal SHA-256 implementation, adapted as straightforward fixed-array
 * code so host/research builds remain allocator-free and independent of a
 * platform hashing implementation. */
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
    /// The canonical order identity of slot rank `n`; see [`canonical_order_id`].
    fn oid(n: u8) -> Hash32 {
        canonical_order_id(n as u64)
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
        /* A 16-cell degree-0 boundary table: 15 strictly increasing interior
         * boundaries, deliberately non-uniform so the uniform declaration
         * stays the 0xFF sentinel, and a live payout map folding the 16 cells
         * onto the 3 payout vectors. */
        let mut knots = [0u128; MAX_KNOTS];
        let mut payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
        let mut i = 0;
        while i < MAX_OUTCOMES - 1 {
            knots[i] = (10 * (i as u128 + 1)) + i as u128;
            i += 1;
        }
        i = 0;
        while i < MAX_OUTCOMES {
            payout_map[i] = (i % 3) as u8;
            i += 1;
        }
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
            statistic_id: 1,
            ambiguity_policy_id: 1,
            edge_policy_id: 1,
            basis_degree: 0,
            knot_count: (MAX_OUTCOMES - 1) as u8,
            uniform_log2_spacing: UNIFORM_SPACING_NONE,
            failure_payout_index: 2,
            coverage_policy_parameter: 0,
            repair_generation: 5,
            source_version: 1,
            evaluator_version: 1,
            source_adapter_id: h(6),
            payout_map,
            knots,
            collateral_cap: 1_000_000,
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
            basis_degree: 0,
            phase: EPOCH_PHASE_OPEN,
            stored_bump: 6,
            flags: 0,
        }
    }
    fn order(id: u8) -> OrderRecord {
        OrderRecord {
            owner: h(20),
            order_id: oid(id),
            outcome: 0,
            side: 0,
            quantity: 10,
            limit: 2_500,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: u64::MAX,
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
            order_id: oid(id),
            side: 0,
            active_len: 2,
            flags: 0,
            coefficients,
            lots: 5,
            limit_collateral_per_lot: 9_000,
            minimum_fill_lots: 2,
            generation: 1,
            expiry_epoch: u64::MAX,
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
            (oid(ids[0]), oid(ids[ids.len() - 1]))
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
            tombstone_count: 0,
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
            // A VERIFIED record carries its verified tie digest.
            score_digest: Hash32([0x5c; HASH_BYTES]),
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
        assert_eq!(account_len::ORDER_PAGE, 4012);
        assert_eq!(account_len::SUPPLY_LEDGER, 333);
        assert_eq!(account_len::TERMS, 1656);
        assert_eq!(account_len::PRICE_GRID, 589);
        assert_eq!(account_len::EPOCH, 329);
        assert_eq!(account_len::CANDIDATE, 337);
        assert_eq!(account_len::FINAL_POT, 262);
        assert_eq!(account_len::SETTLEMENT_RECEIPT, 217);
        assert_eq!(account_len::SETTLEMENT_RECEIPT_V3, 217);
        assert_eq!(account_len::SETTLEMENT_RECEIPT_V4, 217);
        assert_eq!(account_len::SETTLEMENT_RECEIPT_V5, 298);
        assert_eq!(account_len::RESOLUTION, 165);
        assert_eq!(ORDER_RECORD_BYTES, 107);
        assert_eq!(PORTFOLIO_RECORD_BYTES, 235);
        assert_eq!(TOMBSTONE_RECORD_BYTES, 80);
        assert_eq!(ORDER_SLOT_BYTES, 236);
        // The page is exactly its header plus sixteen common-width slots.
        assert_eq!(
            account_len::ORDER_PAGE,
            236 + MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES
        );
        // The widest admitted intent is a v2 source-spec construction, exactly:
        // header, Terms binding, canonical pull body.  The portfolio placement
        // it displaced (310) is pinned beside it so a regression in either is
        // visible as a moved number rather than as slack.
        assert_eq!(MAX_INTENT_BYTES, 402);
        assert_eq!(2 + HASH_BYTES + SOURCE_SPEC_BODY_V2_BYTES, MAX_INTENT_BYTES);
        assert_eq!(2 + (2 * HASH_BYTES) + 8 + 1 + PORTFOLIO_RECORD_BYTES, 310);
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
        let policy_id = h(3);
        let release_id = h(4);
        let profile_id = canonical_profile_v2_id(policy_id, release_id).unwrap();
        let realm = RealmAccount {
            realm: h(1),
            profile: profile_id,
            max_outcomes: 16,
            profile_version: PROFILE_SCHEMA_V2,
            stored_bump: 3,
            flags: 0,
        };
        let mut realm_bytes = [0; account_len::REALM];
        realm.encode(&mut realm_bytes).unwrap();
        assert_eq!(RealmAccount::decode(&realm_bytes), Ok(realm));

        let profile = ProfileAccount {
            profile: profile_id,
            realm: h(1),
            collateral_policy_id: policy_id,
            adapter_release_id: release_id,
            version: PROFILE_SCHEMA_V2,
            flags: PROFILE_FLAG_POLICY_FROZEN,
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

        let page = build_page(0, 1, &[1, 2], Hash32::ZERO);
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
    fn receipt_slice_index_uses_the_candidate_witness_bound() {
        let mut last = receipt();
        last.slice_index = (MAX_SLICES - 1) as u16;
        let mut bytes = [0; account_len::SETTLEMENT_RECEIPT];
        assert_eq!(last.encode(&mut bytes), Ok(account_len::SETTLEMENT_RECEIPT));
        assert_eq!(SettlementReceiptAccount::decode(&bytes), Ok(last));

        let mut outside = last;
        outside.slice_index = MAX_SLICES as u16;
        assert_eq!(outside.validate(), Err(CodecError::InvalidCount));
        assert_eq!(outside.encode(&mut bytes), Err(CodecError::InvalidCount));
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
        let policy_id = h(3);
        let release_id = h(4);
        let profile = ProfileAccount {
            profile: canonical_profile_v2_id(policy_id, release_id).unwrap(),
            realm: h(1),
            collateral_policy_id: policy_id,
            adapter_release_id: release_id,
            version: PROFILE_SCHEMA_V2,
            flags: PROFILE_FLAG_POLICY_FROZEN,
        };
        let mut b = [0; account_len::PROFILE];
        profile.encode(&mut b).unwrap();
        assert_eq!(b[1], account_version::PROFILE);
        assert_eq!(account_version::PROFILE, LAYOUT_VERSION_V2);
        b[1] = LAYOUT_VERSION_V1;
        assert_eq!(ProfileAccount::decode(&b), Err(CodecError::WrongVersion));

        /* The page is on its fourth shape: bare records, then the page-set
         * commitment fields, then tagged fixed-width slots, then positional
         * order ids with a retirement kind and a persisted expiry.  All three
         * earlier versions are refused explicitly. */
        let page = build_page(0, 1, &[1], Hash32::ZERO);
        let mut b = [0; account_len::ORDER_PAGE];
        page.encode(&mut b).unwrap();
        assert_eq!(b[1], account_version::ORDER_PAGE);
        assert_eq!(account_version::ORDER_PAGE, 4);
        for superseded in [LAYOUT_VERSION_V1, LAYOUT_VERSION_V2, LAYOUT_VERSION_V3] {
            b[1] = superseded;
            assert_eq!(OrderPageAccount::decode(&b), Err(CodecError::WrongVersion));
            assert_eq!(
                stream::verify_page(&b),
                Err(CodecError::WrongVersion),
                "the streaming reader refuses every superseded page version too"
            );
        }

        let mut b = [0; account_len::MARKET];
        market().encode(&mut b).unwrap();
        assert_eq!(b[1], LAYOUT_VERSION_V1);
        b[1] = LAYOUT_VERSION;
        assert_eq!(MarketAccount::decode(&b), Err(CodecError::WrongVersion));
    }
    #[test]
    fn profile_v2_requires_both_exact_children_and_canonical_identity() {
        let policy_id = h(77);
        let release_id = h(78);
        let mut profile = ProfileAccount {
            profile: canonical_profile_v2_id(policy_id, release_id).unwrap(),
            realm: h(1),
            collateral_policy_id: policy_id,
            adapter_release_id: release_id,
            version: PROFILE_SCHEMA_V2,
            flags: PROFILE_FLAG_POLICY_FROZEN,
        };
        let mut b = [0; account_len::PROFILE];
        profile.encode(&mut b).unwrap();
        assert_eq!(&b[66..98], &[77u8; 32]);
        assert_eq!(&b[98..130], &[78u8; 32]);

        profile.collateral_policy_id = h(79);
        assert_eq!(profile.validate(), Err(CodecError::NonCanonicalIdentity));

        profile.collateral_policy_id = Hash32::ZERO;
        assert_eq!(profile.validate(), Err(CodecError::ZeroIdentity));

        profile.collateral_policy_id = policy_id;
        profile.adapter_release_id = Hash32::ZERO;
        assert_eq!(profile.validate(), Err(CodecError::ZeroIdentity));

        profile.adapter_release_id = release_id;
        profile.flags = 2;
        assert_eq!(profile.validate(), Err(CodecError::InvalidEnum));
    }
    #[test]
    fn realm_refuses_a_narrowed_outcome_width() {
        let mut realm = RealmAccount {
            realm: h(1),
            profile: h(2),
            max_outcomes: 16,
            profile_version: PROFILE_SCHEMA_V2,
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
    /// A valid degree-1 (hat-basis) terms fixture: two outcomes anchored on
    /// two knots with a power-of-two gap, no payout map, D from the set.
    fn derived_terms() -> TermsAccount {
        let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
        let mut left = [0; MAX_OUTCOMES];
        left[0] = 8;
        let mut right = [0; MAX_OUTCOMES];
        right[1] = 8;
        payouts[0] = PayoutVectorBytes {
            denominator: 8,
            weights: left,
        };
        payouts[1] = PayoutVectorBytes {
            denominator: 8,
            weights: right,
        };
        let mut knots = [0u128; MAX_KNOTS];
        knots[0] = 100;
        knots[1] = 100 + (1 << 4);
        let mut t = terms();
        t.outcome_count = 2;
        t.payout_count = 2;
        t.payouts = payouts;
        t.basis_degree = 1;
        t.knot_count = 2;
        t.uniform_log2_spacing = 4;
        t.failure_payout_index = 0;
        t.payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
        t.knots = knots;
        t.terms = t.recomputed_terms_digest().unwrap();
        t
    }
    #[test]
    fn terms_v2_bytes_and_wrong_versions_refuse() {
        /* The crate's version discipline: exactly account_version::TERMS is
         * admitted, so superseded v2 bytes (and the prototype v1's) refuse as
         * WrongVersion before any field is read. */
        let t = terms();
        let mut bytes = [0; account_len::TERMS];
        t.encode(&mut bytes).unwrap();
        for version in [1, 2, account_version::TERMS + 1] {
            let mut wrong = bytes;
            wrong[1] = version;
            assert_eq!(TermsAccount::decode(&wrong), Err(CodecError::WrongVersion));
            assert_eq!(
                TermsAccount::decode_unchecked(&wrong),
                Err(CodecError::WrongVersion)
            );
        }
        /* And a v2-length blob is refused by exact length, whatever it says. */
        assert_eq!(
            TermsAccount::decode(&bytes[..1304]),
            Err(CodecError::Truncated)
        );
    }
    #[test]
    fn decode_unchecked_skips_exactly_the_digest_and_nothing_else() {
        let t = terms();
        let mut bytes = [0; account_len::TERMS];
        t.encode(&mut bytes).unwrap();
        assert_eq!(TermsAccount::decode_unchecked(&bytes), Ok(t));

        /* A bit-flipped stored digest: the full decode refuses, the unchecked
         * parse admits — which is exactly why it is sound only over bytes
         * that already passed the full decode once in the same transaction. */
        let mut flipped = bytes;
        flipped[2] ^= 0x01;
        assert_eq!(
            TermsAccount::decode(&flipped),
            Err(CodecError::NonCanonicalIdentity)
        );
        let lying = TermsAccount::decode_unchecked(&flipped).unwrap();
        assert_ne!(lying.terms, t.terms);

        /* Every non-digest fault still refuses through the unchecked path:
         * one representative, patched at the byte level because encode()
         * validates and will not produce hostile bytes. */
        let mut degree_bytes = bytes;
        degree_bytes[terms_field_offset_basis_degree()] = MAX_BASIS_DEGREE + 1;
        assert_eq!(
            TermsAccount::decode_unchecked(&degree_bytes),
            Err(CodecError::InvalidEnum)
        );
    }
    /// Byte offset of `basis_degree` inside an encoded terms account.
    fn terms_field_offset_basis_degree() -> usize {
        /* header(2) + terms(32) + realm/profile/feed/price_grid(4*32) +
         * outcome_count(1) + payout_count(1) + payouts(8*136) + grid(4+2) +
         * buckets(8*4) + policies(4*3) + statistic(2) + ambiguity(1) +
         * edge(1) */
        2 + 32
            + (4 * 32)
            + 1
            + 1
            + (MAX_PAYOUTS * (8 + MAX_OUTCOMES * 8))
            + 4
            + 2
            + (8 * 4)
            + (4 * 3)
            + 2
            + 1
            + 1
    }
    #[test]
    fn terms_refuse_malformed_basis_shapes() {
        let base = terms();

        /* Degree out of range. */
        let mut degree = base;
        degree.basis_degree = MAX_BASIS_DEGREE + 1;
        degree.terms = degree.recomputed_terms_digest().unwrap();
        assert_eq!(degree.validate(), Err(CodecError::InvalidEnum));

        /* Count rule per degree: a degree-0 table needs exactly n − 1 knots,
         * a degree-1 basis exactly n. */
        let mut short = base;
        short.knot_count -= 1;
        short.knots[MAX_OUTCOMES - 2] = 0;
        short.terms = short.recomputed_terms_digest().unwrap();
        assert_eq!(short.validate(), Err(CodecError::InvalidCount));
        let mut miscounted = derived_terms();
        miscounted.knot_count = 3;
        miscounted.knots[2] = miscounted.knots[1] + (1 << 4);
        miscounted.terms = miscounted.recomputed_terms_digest().unwrap();
        assert_eq!(miscounted.validate(), Err(CodecError::InvalidCount));

        /* Non-monotone knots. */
        let mut flat = base;
        flat.knots[3] = flat.knots[2];
        flat.terms = flat.recomputed_terms_digest().unwrap();
        assert_eq!(flat.validate(), Err(CodecError::InvalidCount));
        let mut reversed = base;
        reversed.knots[3] = reversed.knots[2] - 1;
        reversed.terms = reversed.recomputed_terms_digest().unwrap();
        assert_eq!(reversed.validate(), Err(CodecError::InvalidCount));

        /* A zero first boundary at degree 0 mints an empty first cell. */
        let mut hollow = derived_terms();
        hollow.knots[0] = 0;
        hollow.knots[1] = 1 << 4;
        hollow.terms = hollow.recomputed_terms_digest().unwrap();
        /* ...but a zero first *anchor* at degree 1 is admitted: the domain
         * starts at zero. */
        assert_eq!(hollow.validate(), Ok(()));
        let mut empty_cell = base;
        let mut i = 0;
        while i < usize::from(empty_cell.knot_count) {
            empty_cell.knots[i] = i as u128;
            i += 1;
        }
        empty_cell.terms = empty_cell.recomputed_terms_digest().unwrap();
        assert_eq!(empty_cell.validate(), Err(CodecError::ZeroValue));

        /* Live knot padding refuses. */
        let mut padded = base;
        padded.knots[MAX_KNOTS - 1] = u128::MAX;
        padded.terms = padded.recomputed_terms_digest().unwrap();
        assert_eq!(padded.validate(), Err(CodecError::NonCanonicalPadding));
    }
    #[test]
    fn terms_refuse_a_lying_uniform_spacing_declaration() {
        /* The fixture's gaps are 11 apart — not a power of two — so any
         * numeric declaration is a lie the array refutes. */
        let mut lying = terms();
        lying.uniform_log2_spacing = 3;
        lying.terms = lying.recomputed_terms_digest().unwrap();
        assert_eq!(lying.validate(), Err(CodecError::InvalidEnum));

        let mut overshift = derived_terms();
        overshift.uniform_log2_spacing = 128;
        overshift.terms = overshift.recomputed_terms_digest().unwrap();
        assert_eq!(overshift.validate(), Err(CodecError::InvalidEnum));

        /* The truthful declaration on a truly uniform grid is admitted, and
         * the sentinel is admitted for degrees ≤ 1. */
        assert_eq!(derived_terms().validate(), Ok(()));
        let mut sentinel = derived_terms();
        sentinel.uniform_log2_spacing = UNIFORM_SPACING_NONE;
        sentinel.terms = sentinel.recomputed_terms_digest().unwrap();
        assert_eq!(sentinel.validate(), Ok(()));
    }
    #[test]
    fn terms_payout_map_liveness_is_per_degree() {
        /* Degree 0: a live entry must stay inside the payout set... */
        let mut out_of_set = terms();
        out_of_set.payout_map[4] = out_of_set.payout_count;
        out_of_set.terms = out_of_set.recomputed_terms_digest().unwrap();
        assert_eq!(out_of_set.validate(), Err(CodecError::InvalidCount));
        /* ...and entries beyond the active cells must be unused.  The
         * fixture's 16 cells leave no padding, so shrink to expose some. */
        let mut trailing = terms();
        trailing.outcome_count = 4;
        trailing.knot_count = 3;
        let mut i = 3;
        while i < MAX_KNOTS {
            trailing.knots[i] = 0;
            i += 1;
        }
        /* payout_map[4..] still live from the fixture. */
        trailing.terms = trailing.recomputed_terms_digest().unwrap();
        assert_eq!(trailing.validate(), Err(CodecError::NonCanonicalPadding));

        /* Degree ≥ 1: derived mode has no map at all. */
        let mut mapped = derived_terms();
        mapped.payout_map[0] = 0;
        mapped.terms = mapped.recomputed_terms_digest().unwrap();
        assert_eq!(mapped.validate(), Err(CodecError::NonCanonicalPadding));
    }
    #[test]
    fn terms_refuse_an_undecided_collateral_cap_and_unnamed_identities() {
        /* Cap zero is not "unlimited" and not "decide later": it refuses at
         * decode, which is what makes "cap 0 refuses at market init"
         * structural — an unfundable-forever market cannot be founded because
         * its terms cannot exist. */
        let mut undecided = terms();
        undecided.collateral_cap = 0;
        undecided.terms = undecided.recomputed_terms_digest().unwrap();
        assert_eq!(undecided.validate(), Err(CodecError::ZeroValue));

        let mut unnamed = terms();
        unnamed.statistic_id = 0;
        unnamed.terms = unnamed.recomputed_terms_digest().unwrap();
        assert_eq!(unnamed.validate(), Err(CodecError::ZeroValue));

        let mut no_ambiguity = terms();
        no_ambiguity.ambiguity_policy_id = 0;
        no_ambiguity.terms = no_ambiguity.recomputed_terms_digest().unwrap();
        assert_eq!(no_ambiguity.validate(), Err(CodecError::ZeroValue));

        let mut no_edge = terms();
        no_edge.edge_policy_id = 0;
        no_edge.terms = no_edge.recomputed_terms_digest().unwrap();
        assert_eq!(no_edge.validate(), Err(CodecError::ZeroValue));

        let mut unversioned = terms();
        unversioned.source_version = 0;
        unversioned.terms = unversioned.recomputed_terms_digest().unwrap();
        assert_eq!(unversioned.validate(), Err(CodecError::ZeroValue));

        let mut anonymous = terms();
        anonymous.source_adapter_id = Hash32::ZERO;
        anonymous.terms = anonymous.recomputed_terms_digest().unwrap();
        assert_eq!(anonymous.validate(), Err(CodecError::ZeroIdentity));

        let mut refundless = terms();
        refundless.failure_payout_index = refundless.payout_count;
        refundless.terms = refundless.recomputed_terms_digest().unwrap();
        assert_eq!(refundless.validate(), Err(CodecError::InvalidCount));
    }
    #[test]
    fn terms_freeze_time_bound_refuses_an_unprovable_weight_derivation() {
        /* D · (g_max − 1) must stay below 2^127 (design §2.5).  A gap wide
         * enough to breach it with an 8-atom denominator refuses at decode,
         * so the runtime overflow refusal is defense in depth. */
        let mut wide = derived_terms();
        wide.uniform_log2_spacing = 126;
        wide.knots[0] = 0;
        wide.knots[1] = 1 << 126;
        wide.terms = wide.recomputed_terms_digest().unwrap();
        assert_eq!(wide.validate(), Err(CodecError::ArithmeticOverflow));

        /* Just inside the bound is admitted: D = 8 = 2^3, gap = 2^123. */
        let mut inside = derived_terms();
        inside.uniform_log2_spacing = 123;
        inside.knots[0] = 0;
        inside.knots[1] = 1 << 123;
        inside.terms = inside.recomputed_terms_digest().unwrap();
        assert_eq!(inside.validate(), Ok(()));
    }
    /// A valid smooth (degree 2 or 3) terms fixture on a uniform power-of-two
    /// grid: five claims, `K = n + 1 − d` anchors spaced `2^spacing`, no payout
    /// map, and one payout vector carrying the weight denominator `D = 8`.
    fn smooth_terms(basis_degree: u8, spacing: u8) -> TermsAccount {
        let claims = 5u8;
        let knot_count = claims + 1 - basis_degree;
        let mut weights = [0u64; MAX_OUTCOMES];
        weights[0] = 8;
        let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
        payouts[0] = PayoutVectorBytes {
            denominator: 8,
            weights,
        };
        let mut knots = [0u128; MAX_KNOTS];
        let gap: u128 = 1 << spacing;
        let mut i = 0usize;
        while i < usize::from(knot_count) {
            knots[i] = (i as u128) * gap;
            i += 1;
        }
        let mut t = terms();
        t.outcome_count = claims;
        t.payout_count = 1;
        t.payouts = payouts;
        t.failure_payout_index = 0;
        t.payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
        t.basis_degree = basis_degree;
        t.knot_count = knot_count;
        t.uniform_log2_spacing = spacing;
        t.knots = knots;
        t.terms = t.recomputed_terms_digest().unwrap();
        t
    }
    #[test]
    fn terms_admit_valid_smooth_degree_two_and_three() {
        /* The smooth rungs are not refused at admission and never were in this
         * codec: `validate` implements the §2.1 count rule for every degree in
         * `0..=MAX_BASIS_DEGREE`, and `programs/solana-reference` plus
         * `crates/clutch-bspline` evaluate them.  This test pins that, and the
         * byte round trip, so a future "tighten the ladder" edit has to break a
         * named assertion rather than a comment. */
        for degree in [2u8, 3] {
            let t = smooth_terms(degree, 4);
            assert_eq!(t.validate(), Ok(()), "degree {degree} must be admitted");
            let mut bytes = [0; account_len::TERMS];
            assert_eq!(t.encode(&mut bytes), Ok(account_len::TERMS));
            assert_eq!(TermsAccount::decode(&bytes), Ok(t));
            /* The count rule is the one thing that differs between the two. */
            assert_eq!(usize::from(t.knot_count), 5 + 1 - usize::from(degree));
        }
        /* Distinct degrees over the same fields are distinct terms. */
        assert_ne!(smooth_terms(2, 4).terms, smooth_terms(3, 4).terms);
    }
    #[test]
    fn terms_refuse_malformed_smooth_degree_two_and_three() {
        /* Every refusal below is a *side condition of a Lean theorem* about the
         * construction this codec is admitting, not a taste judgement.  The
         * anchors are in `lean/DragonsClutch/BSpline.lean`:
         *
         *  (a) `degree ≤ 3` — `RustExpandedKnotLinkage`'s second conjunct and
         *      `uniform_rust_expanded_knot_linkage`'s `hdegreeHigh`.  Above it
         *      no construction exists at all.
         *  (b) `K = n + 1 − d` — the local block has arity `d + 1`
         *      (`clampedDegreeTwo_length`, `clampedDegreeThree_length`,
         *      `refineTwo/refineThree`'s three- and four-element numerator
         *      lists) and `pad_length` places it inside the `n`-vector.
         *  (c) `2 ≤ K` — `uniform_rust_expanded_knot_linkage`'s `hcount`, also
         *      the hypothesis of `expandOpenClamped_uniform` and
         *      `expandedKnotAt_uniform`.  One anchor is not a pane.
         *  (d) strictly increasing anchors — `RustExpandedKnotLinkage`'s fourth
         *      conjunct, discharged from `hgap : 0 < gap`.  A flat pair makes
         *      `BasisFunsCell.distinct` false and the recurrence divides by zero.
         *  (e) uniform power-of-two spacing, mandatory at `d ≥ 2` — every
         *      linkage theorem is stated over `uniformStoredKnots origin gap
         *      count`; there is no nonuniform counterpart at any degree above
         *      one, so a sentinel declaration has no model to refine.
         *  (f) a truthful declaration — the array is the single semantic owner;
         *      a declaration the array refutes would name a different Lean grid
         *      than the one stored.
         *  (g) canonical zero padding and an entirely unused payout map —
         *      derived-basis markets have no cell-to-preset map.
         *  (h) the freeze-time bound `D·2h² < 2^127` / `D·6h³ < 2^127`
         *      (RECURRENCE-BOUND-01, design §2.5).  In Lean the column
         *      denominators are the products `q·z₀·z₁` and `q·z₀·z₁·z₂`
         *      (`refineTwo`, `refineThree`) and `Exact.bounded` bounds every
         *      numerator by them, so this is the width those products need. */

        /* (a) */
        let mut too_high = smooth_terms(2, 4);
        too_high.basis_degree = MAX_BASIS_DEGREE + 1;
        too_high.terms = too_high.recomputed_terms_digest().unwrap();
        assert_eq!(too_high.validate(), Err(CodecError::InvalidEnum));

        for degree in [2u8, 3] {
            /* (b) the neighbouring degree's knot count is refused, which is
             * exactly the "degree flip alone cannot manufacture a basis" claim
             * the reference crate asserts from the other side. */
            let mut flipped = smooth_terms(degree, 4);
            flipped.basis_degree = if degree == 2 { 3 } else { 2 };
            flipped.terms = flipped.recomputed_terms_digest().unwrap();
            assert_eq!(flipped.validate(), Err(CodecError::InvalidCount));

            let mut miscounted = smooth_terms(degree, 4);
            miscounted.knot_count += 1;
            miscounted.knots[usize::from(miscounted.knot_count) - 1] =
                miscounted.knots[usize::from(miscounted.knot_count) - 2] + (1 << 4);
            miscounted.terms = miscounted.recomputed_terms_digest().unwrap();
            assert_eq!(miscounted.validate(), Err(CodecError::InvalidCount));

            /* (c) a one-anchor grid: reachable only at degree 3 without also
             * breaking (b), so it is checked where the count rule allows it. */
            if degree == 3 {
                let mut lone = smooth_terms(3, 4);
                lone.outcome_count = 3;
                lone.knot_count = 1;
                lone.knots[1] = 0;
                lone.knots[2] = 0;
                lone.terms = lone.recomputed_terms_digest().unwrap();
                assert_eq!(lone.validate(), Err(CodecError::InvalidCount));
            }

            /* (d) */
            let mut flat = smooth_terms(degree, 4);
            flat.knots[1] = flat.knots[0];
            flat.terms = flat.recomputed_terms_digest().unwrap();
            assert_eq!(flat.validate(), Err(CodecError::InvalidCount));

            let mut reversed = smooth_terms(degree, 4);
            reversed.knots[1] = reversed.knots[0];
            reversed.knots[0] = reversed.knots[2];
            reversed.terms = reversed.recomputed_terms_digest().unwrap();
            assert_eq!(reversed.validate(), Err(CodecError::InvalidCount));

            /* (e) the sentinel is admitted at degree ≤ 1 and refused here. */
            let mut nonuniform = smooth_terms(degree, 4);
            nonuniform.uniform_log2_spacing = UNIFORM_SPACING_NONE;
            nonuniform.terms = nonuniform.recomputed_terms_digest().unwrap();
            assert_eq!(nonuniform.validate(), Err(CodecError::InvalidEnum));

            /* (f) */
            let mut lying = smooth_terms(degree, 4);
            lying.uniform_log2_spacing = 5;
            lying.terms = lying.recomputed_terms_digest().unwrap();
            assert_eq!(lying.validate(), Err(CodecError::InvalidEnum));

            let mut overshift = smooth_terms(degree, 4);
            overshift.uniform_log2_spacing = 128;
            overshift.terms = overshift.recomputed_terms_digest().unwrap();
            assert_eq!(overshift.validate(), Err(CodecError::InvalidEnum));

            /* (g) */
            let mut padded = smooth_terms(degree, 4);
            padded.knots[MAX_KNOTS - 1] = u128::MAX;
            padded.terms = padded.recomputed_terms_digest().unwrap();
            assert_eq!(padded.validate(), Err(CodecError::NonCanonicalPadding));

            let mut mapped = smooth_terms(degree, 4);
            mapped.payout_map[0] = 0;
            mapped.terms = mapped.recomputed_terms_digest().unwrap();
            assert_eq!(mapped.validate(), Err(CodecError::NonCanonicalPadding));
        }

        /* (h) The two smooth bounds differ, so each degree gets its own pair.
         * Degree 2 with D = 2^3: 2^3 · 2 · 2^(2s) < 2^127 iff s ≤ 61. */
        assert_eq!(smooth_terms(2, 61).validate(), Ok(()));
        assert_eq!(
            smooth_terms(2, 62).validate(),
            Err(CodecError::ArithmeticOverflow)
        );
        /* Degree 3 with D = 2^3: 3 · 2^(4 + 3s) < 2^127 iff s ≤ 40. */
        assert_eq!(smooth_terms(3, 40).validate(), Ok(()));
        assert_eq!(
            smooth_terms(3, 41).validate(),
            Err(CodecError::ArithmeticOverflow)
        );
    }
    #[test]
    fn terms_reserved_bytes_must_be_zero() {
        let t = terms();
        let mut bytes = [0; account_len::TERMS];
        t.encode(&mut bytes).unwrap();
        /* The mid reserved byte sits right after failure_payout_index. */
        let mid = terms_field_offset_basis_degree() + 4;
        assert_eq!(bytes[mid], 0);
        let mut poked = bytes;
        poked[mid] = 1;
        assert_eq!(
            TermsAccount::decode(&poked),
            Err(CodecError::NonCanonicalPadding)
        );
        /* And the trailing seven, just before stored_bump and flags. */
        let tail = account_len::TERMS - 2 - 7;
        let mut tailed = bytes;
        tailed[tail] = 1;
        assert_eq!(
            TermsAccount::decode(&tailed),
            Err(CodecError::NonCanonicalPadding)
        );
    }
    #[test]
    fn every_new_terms_field_is_inside_the_digest() {
        let t = terms();
        let mut statistic = t;
        statistic.statistic_id = 2;
        assert_ne!(statistic.recomputed_terms_digest().unwrap(), t.terms);
        let mut cap = t;
        cap.collateral_cap = 999;
        assert_ne!(cap.recomputed_terms_digest().unwrap(), t.terms);
        let mut knot = t;
        knot.knots[0] += 1;
        assert_ne!(knot.recomputed_terms_digest().unwrap(), t.terms);
        let mut map = t;
        map.payout_map[0] = 1;
        assert_ne!(map.recomputed_terms_digest().unwrap(), t.terms);
        let mut adapter = t;
        adapter.source_adapter_id = h(0x66);
        assert_ne!(adapter.recomputed_terms_digest().unwrap(), t.terms);
        let mut generation = t;
        generation.repair_generation = 6;
        assert_ne!(generation.recomputed_terms_digest().unwrap(), t.terms);
        let mut parameter = t;
        parameter.coverage_policy_parameter = 3;
        assert_ne!(parameter.recomputed_terms_digest().unwrap(), t.terms);
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
        let page = build_page(0, 1, &[1, 2], Hash32::ZERO);
        let mut b = [0; account_len::ORDER_PAGE];
        page.encode(&mut b).unwrap();
        assert_eq!(OrderPageAccount::decode_on_grid(&b, &g), Ok(page));

        let mut off = page;
        off.orders[1] = OrderSlot::Single(OrderRecord {
            limit: 2_501,
            ..order(2)
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
        dup[1].first_order_id = oid(16);
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
        unlinked[1].prev_page_last_order_id = oid(15);
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
        let mut page = build_page(0, 1, &[1, 2], Hash32::ZERO);
        assert_eq!(page.validate(), Ok(()));

        let mut stale = page;
        stale.last_order_id = oid(8);
        assert_eq!(stale.validate(), Err(CodecError::MismatchedBinding));

        let mut chained = page;
        chained.prev_page_last_order_id = oid(2);
        assert_eq!(chained.validate(), Err(CodecError::NonCanonicalPadding));

        // Page one must link to exactly the rank its own index fixes.
        let mut second = build_page(1, 2, &[17, 18], h(9));
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
        sparse[0].last_order_id = oid(15);
        sparse[0].orders[15] = OrderSlot::Empty;
        sparse[0].page_digest = sparse[0].recomputed_page_digest().unwrap();
        assert_eq!(sparse[0].validate(), Err(CodecError::InvalidCount));

        let mut empty_frozen = set;
        empty_frozen[1].order_count = 0;
        assert_eq!(empty_frozen[1].validate(), Err(CodecError::InvalidCount));

        let mut too_many = build_page(0, 5, &[1], Hash32::ZERO);
        too_many.page_digest = too_many.recomputed_page_digest().unwrap();
        assert_eq!(too_many.validate(), Err(CodecError::InvalidCount));
    }
    #[test]
    fn page_rejects_duplicate_or_unsorted_orders() {
        let mut page = build_page(0, 1, &[1], Hash32::ZERO);
        page.orders[1] = single(1);
        page.order_count = 2;
        page.last_order_id = oid(1);
        page.page_digest = page.recomputed_page_digest().unwrap();
        assert_eq!(page.validate(), Err(CodecError::NonCanonicalIdentity));

        // The same two ids, transposed: each slot now holds the other's rank.
        let mut unsorted = build_page(0, 1, &[1, 2], Hash32::ZERO);
        unsorted.orders[0] = single(2);
        unsorted.orders[1] = single(1);
        unsorted.first_order_id = oid(2);
        unsorted.last_order_id = oid(1);
        unsorted.page_digest = unsorted.recomputed_page_digest().unwrap();
        assert_eq!(unsorted.validate(), Err(CodecError::NonCanonicalIdentity));

        let mut padded = build_page(0, 1, &[1], Hash32::ZERO);
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
        let mut page = build_page(0, 1, &[1, 2], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(portfolio(2));
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
        let single = order(1);
        assert_eq!(&o[1..33], &single.owner.0);
        assert_eq!(&o[33..65], &single.order_id.0);
        assert_eq!(o[65], single.outcome);
        assert_eq!(o[66], single.side);
        assert_eq!(&o[67..75], &single.quantity.to_le_bytes());
        assert_eq!(&o[75..83], &single.limit.to_le_bytes());
        assert_eq!(&o[83..91], &single.minimum_fill.to_le_bytes());
        assert_eq!(o[91], single.flags);
        assert_eq!(&o[92..100], &single.generation.to_le_bytes());
        assert_eq!(&o[100..108], &single.expiry_epoch.to_le_bytes());
        assert!(o[108..].iter().all(|x| *x == 0));

        let p = slot_at(&b, 1);
        let expected = portfolio(2);
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
        assert_eq!(&p[228..236], &expected.expiry_epoch.to_le_bytes());

        // The order-id chain is one chain across both families.
        assert_eq!(page.orders[0].order_id(), oid(1));
        assert_eq!(page.orders[1].order_id(), oid(2));
        assert!(page.orders[1].is_portfolio());
        assert!(!page.orders[0].is_portfolio());
        assert_eq!(page.orders[1].owner(), h(21));
        assert_eq!(page.orders[2].order_id(), Hash32::ZERO);

        let mut crossed = page;
        crossed.orders[1] = OrderSlot::Portfolio(portfolio(1));
        crossed.last_order_id = oid(1);
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
        let mut page = build_page(0, 1, &[1, 2], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(portfolio(2));
        reseal(&mut page);
        let mut b = [0; account_len::ORDER_PAGE];
        page.encode(&mut b).unwrap();
        assert_eq!(PAGE_HEADER_BYTES, 236);
        assert_eq!(
            page.page_digest,
            canonical_page_digest(
                page.market,
                page.epoch,
                page.page_index,
                page.order_count,
                page.tombstone_count,
                &b[PAGE_HEADER_BYTES..],
            )
        );
    }
    #[test]
    fn portfolio_records_refuse_bad_widths_coefficients_and_lot_bounds() {
        let base = portfolio(2);
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
        let mut page = build_page(0, 1, &[1, 2], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(no_lots);
        page.page_digest = page.recomputed_page_digest().unwrap();
        let mut b = [0; account_len::ORDER_PAGE];
        assert_eq!(page.encode(&mut b), Err(CodecError::ZeroValue));
    }
    #[test]
    fn portfolio_bounds_are_checked_against_the_frozen_price_scale() {
        let g = grid();
        let mut page = build_page(0, 1, &[1, 2], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(portfolio(2));
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
        let mut huge_value = portfolio(2);
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

        let mut huge_bound = portfolio(2);
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
            portfolio(2).validate_on_scale(0),
            Err(CodecError::InvalidPriceGrid)
        );
        assert_eq!(
            portfolio(2).validate_on_scale(MAX_PRICE_SCALE + 1),
            Err(CodecError::InvalidPriceGrid)
        );
    }
    #[test]
    fn hostile_order_slots_refuse_unknown_kinds_and_nonzero_slot_padding() {
        let mut page = build_page(0, 1, &[1, 2], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(portfolio(2));
        reseal(&mut page);
        let mut clean = [0; account_len::ORDER_PAGE];
        page.encode(&mut clean).unwrap();

        // An unrecognized slot kind is refused like any other discriminator.
        let mut unknown = clean;
        unknown[PAGE_HEADER_BYTES] = ORDER_KIND_TOMBSTONE + 1;
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
        // claims and lifecycle, not coordinates.  Superseding zeroes the
        // verified tie digest — a superseded record competes for nothing.
        let mut rescored = c;
        rescored.weighted_direct_volume = i128::MIN;
        rescored.limit_surplus_price_units = u128::MAX;
        rescored.distinct_owners = 1;
        rescored.status = CANDIDATE_STATUS_SUPERSEDED;
        rescored.score_digest = Hash32::ZERO;
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

        /* The verified tie digest and the status are one fact stated twice:
         * a VERIFIED record without a digest has no verified identity, and a
         * SUBMITTED record with one is the shape a forged verification would
         * like to have. */
        let mut hollow = c;
        hollow.score_digest = Hash32::ZERO;
        assert_eq!(hollow.validate(), Err(CodecError::ZeroIdentity));
        let mut premature = c;
        premature.status = CANDIDATE_STATUS_SUBMITTED;
        assert_eq!(premature.validate(), Err(CodecError::NonCanonicalPadding));
        let mut refused = c;
        refused.status = CANDIDATE_STATUS_REFUSED;
        assert_eq!(refused.validate(), Err(CodecError::NonCanonicalPadding));
        refused.score_digest = Hash32::ZERO;
        assert_eq!(refused.validate(), Ok(()));

        let mut forged = c;
        forged.prices[0] = 1;
        assert_eq!(forged.validate(), Err(CodecError::NonCanonicalIdentity));
    }
    #[test]
    fn candidate_binds_the_frozen_epoch_simplex() {
        let c = candidate();
        let e = frozen_epoch();
        // The fixture set has no retirements, so its live count is the slot
        // count.
        let live = e.order_count;
        assert_eq!(c.binds_epoch(&e, live), Ok(()));
        assert_eq!(
            c.binds_epoch(&epoch_account(), live),
            Err(CodecError::MismatchedBinding)
        );

        let mut off_simplex = c;
        off_simplex.prices[0] = 9_999;
        off_simplex.candidate = off_simplex.recomputed_candidate_digest().unwrap();
        assert_eq!(
            off_simplex.binds_epoch(&e, live),
            Err(CodecError::MismatchedBinding)
        );

        let mut over_scale = c;
        over_scale.prices[0] = 10_001;
        over_scale.candidate = over_scale.recomputed_candidate_digest().unwrap();
        assert_eq!(
            over_scale.binds_epoch(&e, live),
            Err(CodecError::InvalidPriceGrid)
        );

        let mut wrong_len = c;
        wrong_len.order_len = 18;
        wrong_len.candidate = wrong_len.recomputed_candidate_digest().unwrap();
        assert_eq!(
            wrong_len.binds_epoch(&e, live),
            Err(CodecError::MismatchedBinding)
        );
    }
    /// The frozen two-page fixture set with one record retired in place, and
    /// an epoch frozen over that set.  Nineteen populated slots, eighteen live.
    fn cancelled_set_and_epoch() -> ([OrderPageAccount; 2], EpochAccount) {
        let mut pages = frozen_pages();
        let owner = pages[0].orders[2].owner();
        pages[0].orders[2] = tombstone(3, owner, 1, 2);
        pages[0].tombstone_count = 1;
        freeze_set(&mut pages);
        let mut e = epoch_account();
        e.phase = EPOCH_PHASE_FROZEN;
        e.order_set = pages[0].order_set;
        e.first_order_id = pages[0].first_order_id;
        e.last_order_id = pages[1].last_order_id;
        e.page_count = 2;
        e.order_count = pages[0].set_order_count;
        (pages, e)
    }
    #[test]
    fn candidate_binds_the_live_cardinality_of_a_cancelled_book() {
        let (pages, e) = cancelled_set_and_epoch();
        assert_eq!(e.binds_page_set(&pages), Ok(()));
        assert_eq!(e.order_count, 19, "the retirement keeps its slot");

        // The caller contract: the live count is a fold over the headers the
        // set binding just digest-verified, never a candidate's own claim.
        let live = u16::from(pages[0].live_count()) + u16::from(pages[1].live_count());
        assert_eq!(live, 18);

        let mut c = candidate();
        c.order_len = live as u8;
        c.candidate = c.recomputed_candidate_digest().unwrap();
        assert_eq!(c.binds_epoch(&e, live), Ok(()));
    }
    #[test]
    fn candidate_claiming_the_slot_count_of_a_cancelled_book_refuses() {
        let (pages, e) = cancelled_set_and_epoch();
        assert_eq!(e.binds_page_set(&pages), Ok(()));
        let live = u16::from(pages[0].live_count()) + u16::from(pages[1].live_count());

        // A candidate claiming the populated-slot count on a cancelled book
        // claims one more order than the relation will ever be fed.
        let mut slot_claim = candidate();
        slot_claim.order_len = e.order_count as u8;
        slot_claim.candidate = slot_claim.recomputed_candidate_digest().unwrap();
        assert_eq!(
            slot_claim.binds_epoch(&e, live),
            Err(CodecError::MismatchedBinding)
        );

        // A live count above the slot count is impossible whatever the
        // candidate claims alongside it.
        let mut oversold = candidate();
        oversold.order_len = 20;
        oversold.candidate = oversold.recomputed_candidate_digest().unwrap();
        assert_eq!(
            oversold.binds_epoch(&e, 20),
            Err(CodecError::MismatchedBinding)
        );
    }
    #[test]
    fn a_mutated_tombstone_count_is_caught_by_the_page_digest_not_the_binding() {
        let (pages, e) = cancelled_set_and_epoch();

        // Restating the retirement count alone: the count sits inside the
        // page-digest preimage, so the stored digest no longer matches, and
        // the page refuses before any set- or candidate-level check runs.
        let mut restated = pages[0];
        restated.tombstone_count = 2;
        assert_ne!(
            restated.recomputed_page_digest().unwrap(),
            pages[0].page_digest
        );
        assert_eq!(restated.validate(), Err(CodecError::MismatchedBinding));
        assert_eq!(
            e.binds_page_set(&[restated, pages[1]]),
            Err(CodecError::MismatchedBinding)
        );

        // A self-consistent post-freeze retirement — extra tombstone, count,
        // and digest all resealed — validates as a page but changes the page
        // digest, so the frozen order_set fold refuses the set.
        let mut retired_late = pages;
        let owner = retired_late[0].orders[3].owner();
        retired_late[0].orders[3] = tombstone(4, owner, 1, 2);
        retired_late[0].tombstone_count = 2;
        reseal(&mut retired_late[0]);
        assert_eq!(retired_late[0].validate(), Ok(()));
        assert_eq!(
            e.binds_page_set(&retired_late),
            Err(CodecError::MismatchedBinding)
        );

        // The candidate binding itself never sees a page: given the forged
        // set's live count it would bind.  The layer that catches the
        // mutation is the digest chain that forces an honest caller-supplied
        // count, exactly as the contract states.
        let forged_live =
            u16::from(retired_late[0].live_count()) + u16::from(retired_late[1].live_count());
        assert_eq!(forged_live, 17);
        let mut c = candidate();
        c.order_len = forged_live as u8;
        c.candidate = c.recomputed_candidate_digest().unwrap();
        assert_eq!(c.binds_epoch(&e, forged_live), Ok(()));
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
    fn create_market_intent_wire_is_exact_and_hostile_complete() {
        let intent = Intent::CreateMarket {
            realm: h(1),
            profile: h(2),
            market_nonce: 0,
            outcome_count: 2,
            terms: h(3),
            feed: h(4),
        };
        let mut bytes = [0u8; MAX_INTENT_BYTES];
        let len = intent.encode(&mut bytes).unwrap();
        assert_eq!(len, 139);
        assert_eq!(len, intent.encoded_len());
        assert_eq!(Intent::decode(&bytes[..len]), Ok(intent));
        // A zero nonce is an admitted coordinate, not a zero identity.
        assert!(matches!(
            Intent::decode(&bytes[..len]),
            Ok(Intent::CreateMarket {
                market_nonce: 0,
                ..
            })
        ));
        assert_eq!(
            Intent::decode(&bytes[..len - 1]),
            Err(CodecError::Truncated)
        );

        let mut longer = [0u8; MAX_INTENT_BYTES + 1];
        longer[..len].copy_from_slice(&bytes[..len]);
        longer[len] = 0xa5;
        assert_eq!(
            Intent::decode(&longer[..len + 1]),
            Err(CodecError::TrailingBytes)
        );

        for (label, start, end) in [
            ("realm", 2usize, 34usize),
            ("profile", 34, 66),
            ("terms", 75, 107),
            ("feed", 107, 139),
        ] {
            let mut hostile = bytes;
            hostile[start..end].fill(0);
            assert_eq!(
                Intent::decode(&hostile[..len]),
                Err(CodecError::ZeroIdentity),
                "zero {label}"
            );
        }

        for outcome_count in [0, 1, MAX_OUTCOMES as u8 + 1] {
            let mut hostile = bytes;
            hostile[74] = outcome_count;
            assert_eq!(
                Intent::decode(&hostile[..len]),
                Err(CodecError::InvalidCount),
                "outcome count {outcome_count}"
            );
        }
        for outcome_count in [2, MAX_OUTCOMES as u8] {
            let mut admitted = bytes;
            admitted[74] = outcome_count;
            assert!(matches!(
                Intent::decode(&admitted[..len]),
                Ok(Intent::CreateMarket {
                    outcome_count: decoded,
                    ..
                }) if decoded == outcome_count
            ));
        }

        // Semantic checks retain their encoder order after exact wire shape.
        let mut two_semantic_errors = bytes;
        two_semantic_errors[2..34].fill(0);
        two_semantic_errors[74] = 1;
        assert_eq!(
            Intent::decode(&two_semantic_errors[..len]),
            Err(CodecError::InvalidCount)
        );
        let hostile_value = Intent::CreateMarket {
            realm: Hash32::ZERO,
            profile: h(2),
            market_nonce: 0,
            outcome_count: 1,
            terms: h(3),
            feed: h(4),
        };
        assert_eq!(
            hostile_value.encode(&mut bytes),
            Err(CodecError::InvalidCount)
        );

        // Exact exhaustion remains ahead of semantic validation in the decoder.
        longer[..len].copy_from_slice(&two_semantic_errors[..len]);
        assert_eq!(
            Intent::decode(&longer[..len + 1]),
            Err(CodecError::TrailingBytes)
        );
    }

    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    #[test]
    fn terminal_closure_intents_round_trip_and_stay_tag_contiguous() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 4);
        let candidate = h(3);
        let mut b = [0; MAX_INTENT_BYTES];
        let cases = [
            (
                Intent::ReleaseTerminalReservation { market, epoch },
                RELEASE_TERMINAL_RESERVATION_TAG,
                66usize,
            ),
            (
                Intent::CloseGeneralReceipt {
                    market,
                    epoch,
                    candidate,
                    slice_index: 7,
                },
                CLOSE_GENERAL_RECEIPT_TAG,
                100,
            ),
            (
                Intent::CloseGeneralReservation { market, epoch },
                CLOSE_GENERAL_RESERVATION_TAG,
                66,
            ),
            (
                Intent::CloseGeneralPage {
                    market,
                    epoch,
                    page_index: 2,
                },
                CLOSE_GENERAL_PAGE_TAG,
                68,
            ),
            (
                Intent::CloseGeneralPot { market, epoch },
                CLOSE_GENERAL_POT_TAG,
                66,
            ),
            (
                Intent::CloseGeneralCandidate {
                    market,
                    epoch,
                    candidate,
                },
                CLOSE_GENERAL_CANDIDATE_TAG,
                98,
            ),
            (
                Intent::CloseGeneralClearWork {
                    market,
                    epoch,
                    candidate,
                },
                CLOSE_GENERAL_CLEAR_WORK_TAG,
                98,
            ),
            (
                Intent::CloseGeneralEpoch { market, epoch },
                CLOSE_GENERAL_EPOCH_TAG,
                66,
            ),
        ];
        for (index, (intent, tag, len)) in cases.iter().enumerate() {
            let n = intent.encode(&mut b).unwrap();
            assert_eq!(n, intent.encoded_len(), "{intent:?}");
            assert_eq!(n, *len, "{intent:?}");
            assert_eq!(&b[..2], [*tag, INTENT_VERSION], "{intent:?}");
            assert_eq!(Intent::decode(&b[..n]), Ok(*intent));
            assert_eq!(Intent::decode(&b[..n - 1]), Err(CodecError::Truncated));
            // The family is tag-contiguous from the entitlement pair on.
            assert_eq!(*tag, ENTITLE_SLICE_TAG + 1 + index as u8);
        }

        // Zero identities refuse on both sides of the wire.
        let zero = Intent::CloseGeneralCandidate {
            market,
            epoch,
            candidate: Hash32::ZERO,
        };
        assert_eq!(zero.encode(&mut b), Err(CodecError::ZeroIdentity));
        let good = Intent::CloseGeneralEpoch { market, epoch };
        let n = good.encode(&mut b).unwrap();
        b[2..34].fill(0);
        assert_eq!(Intent::decode(&b[..n]), Err(CodecError::ZeroIdentity));
    }

    /// The Position close route's wire: owner-signed, two identities, and its
    /// own tag past the revenue record's.
    ///
    /// It is deliberately *not* part of the epoch-terminal contiguity above —
    /// a Position outlives every epoch — and it carries no epoch coordinate at
    /// all, so no caller can aim it at one epoch's state.
    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    #[test]
    fn the_position_close_intent_has_an_exact_unambiguous_wire() {
        let market = h(1);
        let owner = h(9);
        let mut b = [0; MAX_INTENT_BYTES];
        let intent = Intent::ClosePosition { market, owner };
        let n = intent.encode(&mut b).unwrap();
        assert_eq!(n, intent.encoded_len());
        assert_eq!(n, 66);
        assert_eq!(&b[..2], [CLOSE_POSITION_TAG, INTENT_VERSION]);
        assert_eq!(CLOSE_POSITION_TAG, CLOSE_REVENUE_POLICY_RECORD_TAG + 1);
        assert_eq!(Intent::decode(&b[..n]), Ok(intent));
        assert_eq!(Intent::decode(&b[..n - 1]), Err(CodecError::Truncated));
        let mut long = [0u8; MAX_INTENT_BYTES];
        long[..n].copy_from_slice(&b[..n]);
        assert_eq!(
            Intent::decode(&long[..n + 1]),
            Err(CodecError::TrailingBytes)
        );
        for zeroed in [
            Intent::ClosePosition {
                market: Hash32::ZERO,
                owner,
            },
            Intent::ClosePosition {
                market,
                owner: Hash32::ZERO,
            },
        ] {
            assert_eq!(zeroed.encode(&mut b), Err(CodecError::ZeroIdentity));
        }
        let n = intent.encode(&mut b).unwrap();
        b[34..66].fill(0);
        assert_eq!(Intent::decode(&b[..n]), Err(CodecError::ZeroIdentity));
    }

    /// The grief rider's one predicate, over real record bytes.
    #[test]
    fn a_revenue_record_names_only_its_own_treasury() {
        let record = revenue::RevenuePolicyRecordV1 {
            realm: h(1),
            policy_digest: h(2),
            treasury: h(3),
            terminal_payer: h(4),
            terminal_payer_principal: 1,
            terminal_donation_floor: 0,
            terminal_generation: 1,
            stored_bump: 254,
            flags: 0,
        };
        let mut bytes = [0u8; revenue::REVENUE_POLICY_RECORD_BYTES];
        record.encode(&mut bytes).unwrap();
        let decoded = revenue::RevenuePolicyRecordV1::decode(&bytes).unwrap();
        assert!(decoded.names_treasury(h(3)));
        assert!(!decoded.names_treasury(h(4)));
        assert!(!decoded.names_treasury(Hash32::ZERO));
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
        assert_eq!(
            native_digest(b"", &[]).0,
            got.0,
            "the native wrapper agrees with the portable path on the empty preimage"
        );
    }

    /* --- native-hasher digest equivalence -----------------------------------
     *
     * On SBF `digest` and `canonical_order_set_id` hand their preimage to
     * Solana's native SHA-256 wrapper instead of folding the portable
     * first-party implementation.  That is a compute change and must never be a
     * value change: a different digest would silently move every canonical id,
     * every PDA derived from one, and every page/order-set commitment.
     *
     * These tests build both paths in the same host process (the wrapper's
     * `sha2` backend is a dev-dependency for exactly this) and require
     * byte-identical output on every shape the crate actually hashes.
     * ---------------------------------------------------------------------- */

    /// Both digest paths agree on the domain-separated shape, at every part
    /// count from zero to the compile-time maximum.
    #[test]
    fn native_digest_matches_the_portable_digest_on_every_part_count() {
        // Empty domain, empty parts: the degenerate preimage.
        assert_eq!(digest(b"", &[]), native_digest(b"", &[]));
        // Domain only, no parts.
        let domain = b"dragons-clutch/equivalence/v1";
        assert_eq!(digest(domain, &[]), native_digest(domain, &[]));
        // Empty parts inside a non-empty list: `hashv` concatenates, so a
        // zero-length slice must contribute nothing on either path.
        assert_eq!(
            digest(domain, &[&[], &[]]),
            native_digest(domain, &[&[], &[]])
        );
        assert_eq!(
            digest(domain, &[&[7u8], &[], &[9u8]]),
            native_digest(domain, &[&[7u8], &[], &[9u8]])
        );
        // A single part, and a part that straddles the 64-byte block boundary
        // the portable implementation buffers against.
        let long = [0xa5u8; 200];
        assert_eq!(digest(domain, &[&long]), native_digest(domain, &[&long]));
        assert_eq!(
            digest(&long, &[&long, &long]),
            native_digest(&long, &[&long, &long])
        );
        // Ragged widths, so no part length is a multiple of any other.
        let a = [1u8; 1];
        let b = [2u8; 31];
        let c = [3u8; 32];
        let d = [4u8; 33];
        let e = [5u8; 64];
        let f = [6u8; 65];
        assert_eq!(
            digest(domain, &[&a, &b, &c, &d, &e, &f]),
            native_digest(domain, &[&a, &b, &c, &d, &e, &f])
        );
        // The widest shape any call site uses: `MAX_DIGEST_PARTS` parts.
        let parts: [&[u8]; MAX_DIGEST_PARTS] = [
            &a, &b, &c, &d, &e, &f, &a, &b, &c, &d, &e, &f, &a, &b, &c, &d,
        ];
        assert_eq!(digest(domain, &parts), native_digest(domain, &parts));
        // Concatenation is what both paths commit to, so a re-split of the same
        // bytes is the same digest and a re-ordering is not.
        assert_eq!(digest(domain, &[&c, &d]), native_digest(domain, &[&c, &d]));
        assert_ne!(digest(domain, &[&c, &d]), digest(domain, &[&d, &c]));
    }

    /// Every public identity constructor that routes through `digest` produces
    /// the same value on both paths, including the widest one (14 parts).
    #[test]
    fn native_digest_matches_the_portable_digest_at_every_call_site() {
        // Each pair below is the constructor's own preimage, spelled out again
        // through `native_digest` so a domain string or a part order that drifts
        // in one place and not the other is a failure here.
        let market = h(3);
        let epoch = h(4);
        let profile = h(5);
        let realm = h(6);

        assert_eq!(
            canonical_realm_id(profile, 7),
            native_digest(
                b"dragons-clutch/realm/v1",
                &[&profile.0, &7u64.to_le_bytes()]
            )
        );
        assert_eq!(
            canonical_market_id(realm, profile, 8),
            native_digest(
                b"dragons-clutch/market/v1",
                &[&realm.0, &profile.0, &8u64.to_le_bytes()]
            )
        );
        assert_eq!(
            canonical_outcome_id(market, 2),
            native_digest(b"dragons-clutch/outcome/v1", &[&market.0, &[2u8]])
        );
        assert_eq!(
            canonical_epoch_id(market, 9),
            native_digest(
                b"dragons-clutch/epoch/v1",
                &[&market.0, &9u64.to_le_bytes()]
            )
        );
        assert_eq!(
            canonical_feed_id(&[0xfe; 40]),
            native_digest(b"dragons-clutch/feed/v1", &[&[0xfeu8; 40]])
        );
        assert_eq!(
            canonical_terms_digest(&[0x11; 64]),
            native_digest(b"dragons-clutch/terms/v2", &[&[0x11u8; 64]])
        );
        assert_eq!(
            canonical_price_grid_id(&[0x22; 96]),
            native_digest(b"dragons-clutch/price-grid/v1", &[&[0x22u8; 96]])
        );
        assert_eq!(
            canonical_candidate_digest(&[0x33; 128]),
            native_digest(b"dragons-clutch/candidate/v1", &[&[0x33u8; 128]])
        );

        // The variable-length page preimage: header fields plus a record blob.
        let records = [0x44u8; 300];
        assert_eq!(
            canonical_page_digest(market, epoch, 1, 5, 2, &records),
            native_digest(
                ORDER_PAGE_DOMAIN,
                &[
                    &market.0,
                    &epoch.0,
                    &1u16.to_le_bytes(),
                    &[5u8, 2u8],
                    &records,
                ]
            )
        );
        // Same call, empty record blob: an empty part must vanish identically.
        assert_eq!(
            canonical_page_digest(market, epoch, 0, 0, 0, &[]),
            native_digest(
                ORDER_PAGE_DOMAIN,
                &[&market.0, &epoch.0, &0u16.to_le_bytes(), &[0u8, 0u8], &[],]
            )
        );

        // The 14-part shape, which is the widest preimage in the crate.
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = 1;
        prices[1] = 2;
        let mut packed = [0u8; MAX_OUTCOMES * 8];
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            packed[i * 8..(i + 1) * 8].copy_from_slice(&prices[i].to_le_bytes());
            i += 1;
        }
        let widest = portfolio_settlement::canonical_portfolio_entitlement_id(
            h(3),
            h(4),
            h(5),
            h(6),
            h(7),
            h(8),
            h(9),
            h(10),
            h(11),
            &prices,
            1_000,
            2,
            5,
            77,
        );
        assert_eq!(
            widest,
            native_digest(
                b"dragons-clutch/portfolio-entitlement/v1",
                &[
                    &h(3).0,
                    &h(4).0,
                    &h(5).0,
                    &h(6).0,
                    &h(7).0,
                    &h(8).0,
                    &h(9).0,
                    &h(10).0,
                    &h(11).0,
                    &packed,
                    &1_000u64.to_le_bytes(),
                    &[2u8],
                    &5u64.to_le_bytes(),
                    &77u128.to_le_bytes(),
                ]
            ),
            "the widest preimage in the crate diverges between the two paths"
        );
    }

    /// The cross-page order-set commitment agrees on both paths at every page
    /// count a set can have, including the empty set.
    #[test]
    fn native_order_set_id_matches_the_portable_fold() {
        let market = h(11);
        let epoch = h(12);
        let pages = [h(21), h(22), h(23), h(24)];
        let mut count = 0;
        while count <= MAX_ORDER_PAGES {
            let slice = &pages[..count.min(pages.len())];
            assert_eq!(
                canonical_order_set_id(market, epoch, count as u16, 7, slice),
                native_order_set_id(market, epoch, count as u16, 7, slice),
                "order-set fold diverges at {count} page digests"
            );
            count += 1;
        }
        // The header fields are inside the preimage, not just the page list.
        assert_ne!(
            canonical_order_set_id(market, epoch, 2, 7, &pages[..2]),
            canonical_order_set_id(market, epoch, 2, 8, &pages[..2])
        );
        assert_ne!(
            canonical_order_set_id(market, epoch, 2, 7, &pages[..2]),
            canonical_order_set_id(market, epoch, 3, 7, &pages[..2])
        );
        // Page order is committed.
        let swapped = [pages[1], pages[0]];
        assert_ne!(
            canonical_order_set_id(market, epoch, 2, 7, &pages[..2]),
            canonical_order_set_id(market, epoch, 2, 7, &swapped)
        );
    }

    /// The identity values themselves are frozen.
    ///
    /// These are the digests the deployed program, every PDA seed, and every
    /// stored commitment already use.  The expected bytes are not read back
    /// from either path: they are plain SHA-256 over the documented preimage,
    /// computed independently, so this pins the *value* and not merely the
    /// agreement of two implementations that could both have moved.
    #[test]
    fn canonical_identity_values_are_frozen() {
        let realm = canonical_realm_id(h(1), 1);
        assert_eq!(
            realm.0,
            [
                0xd8, 0xe1, 0x2b, 0x33, 0x24, 0x0e, 0x86, 0x6a, 0xdf, 0xac, 0xae, 0xf3, 0x93, 0x6c,
                0x94, 0x7d, 0xd1, 0x7c, 0x93, 0x4e, 0xaa, 0xc2, 0xe2, 0xb9, 0xb9, 0xe2, 0xe0, 0x01,
                0xc4, 0x26, 0x3a, 0x30
            ],
            "SHA-256(\"dragons-clutch/realm/v1\" || [0x01; 32] || 1u64le) moved"
        );

        let page = canonical_page_digest(h(3), h(4), 1, 5, 2, &[0x44; 300]);
        assert_eq!(
            page.0,
            [
                0xbf, 0x24, 0x44, 0x8c, 0xae, 0xbd, 0xc5, 0xb2, 0x13, 0xa8, 0x9d, 0x78, 0x49, 0x4b,
                0x9e, 0xe0, 0x98, 0xd5, 0xea, 0xb7, 0xf7, 0x41, 0xef, 0xb8, 0x07, 0x8d, 0x89, 0x09,
                0x7b, 0xee, 0x0e, 0x39
            ],
            "the order-page commitment moved"
        );

        let set = canonical_order_set_id(h(3), h(4), 2, 7, &[h(21), h(22)]);
        assert_eq!(
            set.0,
            [
                0x1f, 0x23, 0x1d, 0xd7, 0x5f, 0x5a, 0x06, 0x3f, 0xed, 0x43, 0x59, 0x5f, 0x11, 0x3b,
                0x8c, 0xc9, 0x78, 0x8a, 0x09, 0x3b, 0x30, 0x72, 0x3f, 0xae, 0x2e, 0x32, 0x9a, 0x8d,
                0x15, 0x04, 0x7d, 0x89
            ],
            "the cross-page order-set commitment moved"
        );

        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = 1;
        prices[1] = 2;
        let entitlement = portfolio_settlement::canonical_portfolio_entitlement_id(
            h(3),
            h(4),
            h(5),
            h(6),
            h(7),
            h(8),
            h(9),
            h(10),
            h(11),
            &prices,
            1_000,
            2,
            5,
            77,
        );
        assert_eq!(
            entitlement.0,
            [
                0x39, 0xbc, 0xc7, 0xae, 0x59, 0x88, 0xf7, 0x4e, 0x44, 0x71, 0x5f, 0xc4, 0xa3, 0x53,
                0x2f, 0x3d, 0x95, 0x00, 0x07, 0xd6, 0x4f, 0xf0, 0xb4, 0xf1, 0x1c, 0x3c, 0xbe, 0xa6,
                0xb5, 0x8f, 0x4a, 0x9e
            ],
            "the portfolio-entitlement identity moved"
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
        w.u8(page.tombstone_count).unwrap();
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
            let mut p = build_page(0, 1, &[1, 2], Hash32::ZERO);
            p.orders[1] = OrderSlot::Portfolio(portfolio(2));
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
            ("single-egg page", build_page(0, 1, &[1, 2], Hash32::ZERO)),
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
        let base = build_page(0, 1, &[1, 2], Hash32::ZERO);
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
                    last_order_id: oid(8),
                    ..base
                },
            ),
            (
                "page zero links to a predecessor",
                OrderPageAccount {
                    prev_page_last_order_id: oid(2),
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
                p.last_order_id = oid(3);
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
                    ..order(1)
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
            s.last_order_id = oid(15);
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
        overlapping.prev_page_last_order_id = oid(30);
        agrees(
            "later page opening below its predecessor",
            &encode_page_unchecked(&overlapping),
        );
    }
    #[test]
    fn streaming_page_verdicts_match_the_buffered_decoder_on_hostile_bytes() {
        let mut page = build_page(0, 1, &[1, 2], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(portfolio(2));
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
        unknown[PAGE_HEADER_BYTES] = ORDER_KIND_TOMBSTONE + 1;
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
        both[PAGE_HEADER_BYTES] = ORDER_KIND_TOMBSTONE + 1;
        assert_eq!(stream::verify_page(&both), Err(CodecError::WrongTag));
        agrees("bad slot and bad header at once", &both);

        // A page-sized buffer of zeros is not a page.
        agrees("all zero", &[0; account_len::ORDER_PAGE]);
    }
    #[test]
    fn the_streaming_header_reads_236_bytes_and_decides_only_header_facts() {
        let page = build_page(0, 1, &[1, 2], Hash32::ZERO);
        let bytes = encode_page_unchecked(&page);
        assert_eq!(
            stream::ORDER_PAGE_HEADER_BYTES,
            account_len::ORDER_PAGE - MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES
        );
        assert_eq!(stream::ORDER_PAGE_HEADER_BYTES, 236);
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
        junk[PAGE_HEADER_BYTES] = ORDER_KIND_TOMBSTONE + 1;
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
        ranged.first_order_id = oid(3);
        assert_eq!(
            stream::OrderPageHeader::decode(&encode_page_unchecked(&ranged))
                .unwrap()
                .validate_shape(),
            Err(CodecError::MismatchedBinding)
        );
    }
    #[test]
    fn the_slot_cursor_reads_one_slot_at_a_time_and_keeps_the_order_chain() {
        let mut page = build_page(0, 1, &[1, 2], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(portfolio(2));
        reseal(&mut page);
        let bytes = encode_page_unchecked(&page);

        let mut cursor = stream::OrderSlotCursor::new(&bytes).unwrap();
        assert_eq!(cursor.index(), 0);
        assert_eq!(cursor.remaining(), MAX_ORDERS_PER_PAGE);
        assert_eq!(cursor.next_slot(), Some(Ok(single(1))));
        assert_eq!(cursor.index(), 1);
        assert_eq!(
            cursor.next_slot(),
            Some(Ok(OrderSlot::Portfolio(portfolio(2))))
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

        /* An id is checked against its own slot's position, so a wrong id
         * refuses at the slot that holds it — the slots before it are still
         * good, and the ones after it are never reached. */
        let mut misranked = build_page(0, 1, &[1, 9], Hash32::ZERO);
        misranked.page_digest = misranked.recomputed_page_digest().unwrap();
        let descending_bytes = encode_page_unchecked(&misranked);
        let mut chain = stream::OrderSlotCursor::new(&descending_bytes).unwrap();
        assert_eq!(chain.next_slot(), Some(Ok(single(1))));
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
        assert_eq!(kinds.next_slot(), Some(Ok(single(1))));
        assert_eq!(kinds.next_slot(), Some(Err(CodecError::WrongTag)));
        let mut dirty = bytes;
        dirty[PAGE_HEADER_BYTES + 3 * ORDER_SLOT_BYTES + 1] = 1;
        let mut pad = stream::OrderSlotCursor::new(&dirty).unwrap();
        assert_eq!(pad.nth(3), Some(Err(CodecError::NonCanonicalPadding)));
    }
    #[test]
    fn the_streamed_page_digest_matches_the_buffered_recompute() {
        let mut page = build_page(0, 1, &[1, 2], Hash32::ZERO);
        page.orders[1] = OrderSlot::Portfolio(portfolio(2));
        reseal(&mut page);
        let bytes = encode_page_unchecked(&page);
        assert_eq!(
            stream::streamed_page_digest(&bytes),
            Ok(page.recomputed_page_digest().unwrap())
        );
        assert_eq!(
            stream::native_page_digest_for_test(&bytes),
            stream::streamed_page_digest(&bytes),
            "the SBF hashv slice assembly is the portable SHA-256 preimage"
        );
        assert_eq!(
            stream::streamed_page_digest(&bytes),
            Ok(canonical_page_digest(
                page.market,
                page.epoch,
                page.page_index,
                page.order_count,
                page.tombstone_count,
                &bytes[PAGE_HEADER_BYTES..],
            ))
        );
        // One changed record atom is one changed digest.
        let mut moved = page;
        moved.orders[0] = OrderSlot::Single(OrderRecord {
            quantity: 11,
            ..order(1)
        });
        assert_ne!(
            stream::streamed_page_digest(&encode_page_unchecked(&moved)),
            stream::streamed_page_digest(&bytes)
        );
        assert_eq!(
            stream::native_page_digest_for_test(&encode_page_unchecked(&moved)),
            stream::streamed_page_digest(&encode_page_unchecked(&moved))
        );
        // A page whose slot array is not canonical has no digest at all.
        let mut junk = bytes;
        junk[PAGE_HEADER_BYTES + 2 * ORDER_SLOT_BYTES] = 9;
        assert_eq!(
            stream::streamed_page_digest(&junk),
            Err(CodecError::WrongTag)
        );
        assert_eq!(
            stream::native_page_digest_for_test(&junk),
            Err(CodecError::WrongTag),
            "native hashing never commits structurally invalid slot bytes"
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
        dup[1].first_order_id = oid(16);
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
        unlinked[1].prev_page_last_order_id = oid(15);
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
        junk[PAGE_HEADER_BYTES] = ORDER_KIND_TOMBSTONE + 1;
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
        let page = build_page(0, 1, &[1, 2], Hash32::ZERO);
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
            ..order(1)
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
            ..portfolio(2)
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

    /* ------------------------------------------------------------------------
     * v4: positional order ids, retirements, per-order expiry, and the writer.
     * --------------------------------------------------------------------- */

    /// One retirement of the record at `rank`, as this page's writers make it.
    fn tombstone(rank: u8, owner: Hash32, retired: u64, generation: u64) -> OrderSlot {
        OrderSlot::Tombstone(TombstoneRecord {
            order_id: oid(rank),
            owner,
            retired_generation: retired,
            generation,
        })
    }

    #[test]
    fn a_canonical_order_id_is_a_rank_and_nothing_else_decodes_as_one() {
        // The encoding is a rank, big-endian, in the low eight bytes.
        let mut expected = [0u8; HASH_BYTES];
        expected[HASH_BYTES - 1] = 7;
        assert_eq!(canonical_order_id(7), Hash32::from_bytes(expected));
        // Round trip over every rank a book can hold, and one past it.
        let mut rank = 1u64;
        while rank <= MAX_EPOCH_ORDERS as u64 {
            assert_eq!(order_id_rank(canonical_order_id(rank)), Ok(rank));
            rank += 1;
        }
        assert_eq!(
            order_id_rank(canonical_order_id(MAX_EPOCH_ORDERS as u64 + 1)),
            Err(CodecError::InvalidCount)
        );

        // Byte order is rank order, which is what let the id chain keep its
        // shape: the encoding of a larger rank compares larger.
        assert!(canonical_order_id(2).0 > canonical_order_id(1).0);
        assert!(canonical_order_id(17).0 > canonical_order_id(16).0);

        // Nothing else is an order id.
        assert_eq!(order_id_rank(Hash32::ZERO), Err(CodecError::ZeroIdentity));
        assert_eq!(order_id_rank(h(3)), Err(CodecError::NonCanonicalIdentity));
        let mut smuggled = canonical_order_id(1);
        smuggled.0[0] = 1;
        assert_eq!(
            order_id_rank(smuggled),
            Err(CodecError::NonCanonicalIdentity)
        );
        let mut high = canonical_order_id(1);
        high.0[HASH_BYTES - ORDER_ID_RANK_BYTES - 1] = 1;
        assert_eq!(order_id_rank(high), Err(CodecError::NonCanonicalIdentity));
        // A rank that does not fit the eight-byte field is not a near miss.
        assert_eq!(
            order_id_rank(canonical_order_id(u64::MAX)),
            Err(CodecError::InvalidCount)
        );

        // A record carrying a non-rank id is refused by the record, not only
        // by the page that would hold it.
        let mut wild = order(1);
        wild.order_id = h(9);
        assert_eq!(wild.validate(), Err(CodecError::NonCanonicalIdentity));
        let mut wild_portfolio = portfolio(1);
        wild_portfolio.order_id = h(9);
        assert_eq!(
            wild_portfolio.validate(),
            Err(CodecError::NonCanonicalIdentity)
        );
    }

    #[test]
    fn a_slot_id_is_fixed_by_its_position_on_every_page_of_the_set() {
        // Page one's slots are ranks 17..; page two's are 33...
        let ids: [u8; MAX_ORDERS_PER_PAGE] =
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let p0 = build_page(0, 3, &ids, Hash32::ZERO);
        let p1 = build_page(1, 3, &[17, 18], oid(16));
        assert_eq!(p0.validate(), Ok(()));
        assert_eq!(p1.validate(), Ok(()));
        assert_eq!(p1.prev_page_last_order_id, oid(16));

        // A page that links to anything but the rank its index fixes is
        // refused, even when the link it claims is a perfectly good order id.
        let mut short_link = p1;
        short_link.prev_page_last_order_id = oid(15);
        assert_eq!(short_link.validate(), Err(CodecError::NonCanonicalIdentity));

        // A page-one slot holding a page-zero rank is refused, which is the
        // cross-page duplicate the old chain could only catch at closure time.
        let mut stolen = build_page(1, 3, &[3, 18], oid(16));
        stolen.first_order_id = oid(3);
        stolen.page_digest = stolen.recomputed_page_digest().unwrap();
        assert_eq!(stolen.validate(), Err(CodecError::NonCanonicalIdentity));

        // The last rank a book admits is exactly `MAX_EPOCH_ORDERS`, and it is
        // the last slot of the last page rather than a value anyone chose.
        let tail_ids: [u8; MAX_ORDERS_PER_PAGE] = [
            49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64,
        ];
        let last = build_page(3, 4, &tail_ids, oid(48));
        assert_eq!(last.validate(), Ok(()));
        assert_eq!(
            order_id_rank(last.orders[MAX_ORDERS_PER_PAGE - 1].order_id()),
            Ok(MAX_EPOCH_ORDERS as u64)
        );
    }

    #[test]
    fn a_retirement_keeps_the_slot_and_the_id_it_retired() {
        let mut page = build_page(0, 1, &[1, 2], Hash32::ZERO);
        let owner = page.orders[0].owner();
        page.orders[0] = tombstone(1, owner, 1, 2);
        page.tombstone_count = 1;
        page.page_digest = page.recomputed_page_digest().unwrap();
        assert_eq!(page.validate(), Ok(()));

        // The retirement occupies the count and the range exactly as the record
        // it replaced did: nothing after it is renumbered.
        assert_eq!(page.order_count, 2);
        assert_eq!(page.first_order_id, oid(1));
        assert_eq!(page.last_order_id, oid(2));
        assert_eq!(order_id_rank(page.orders[1].order_id()), Ok(2));
        assert!(page.orders[0].is_tombstone());
        assert!(!page.orders[0].is_live());
        assert!(page.orders[1].is_live());

        // It round-trips as bytes, and the streaming reader sees the same page.
        let mut b = [0; account_len::ORDER_PAGE];
        page.encode(&mut b).unwrap();
        assert_eq!(OrderPageAccount::decode(&b), Ok(page));
        let header = stream::verify_page(&b).unwrap();
        assert_eq!(header, stream::OrderPageHeader::of_page(&page));
        assert_eq!(header.tombstone_count, 1);
        assert_eq!(header.live_count(), 1);

        // Its body is exactly `TOMBSTONE_RECORD_BYTES` behind the kind byte,
        // and every byte to the common slot width is canonical padding.
        let t = slot_at(&b, 0);
        assert_eq!(t[0], ORDER_KIND_TOMBSTONE);
        assert_eq!(&t[1..33], &oid(1).0);
        assert_eq!(&t[33..65], &owner.0);
        assert_eq!(&t[65..73], &1u64.to_le_bytes());
        assert_eq!(&t[73..81], &2u64.to_le_bytes());
        assert_eq!(1 + TOMBSTONE_RECORD_BYTES, 81);
        assert!(t[81..].iter().all(|x| *x == 0));

        // The page-set commitment covers the retirement's bytes: changing the
        // retirement changes the digest, exactly as changing a record does.
        let mut regenerated = page;
        regenerated.orders[0] = tombstone(1, owner, 1, 3);
        assert_ne!(
            regenerated.recomputed_page_digest().unwrap(),
            page.page_digest
        );
        assert_eq!(regenerated.validate(), Err(CodecError::MismatchedBinding));
    }

    #[test]
    fn retirements_refuse_bad_generations_counts_and_positions() {
        let mut page = build_page(0, 1, &[1, 2], Hash32::ZERO);
        let owner = page.orders[0].owner();

        // A retirement must strictly follow the placement it retires.
        let mut stale = page;
        stale.orders[0] = tombstone(1, owner, 2, 2);
        stale.tombstone_count = 1;
        stale.page_digest = stale.recomputed_page_digest().unwrap();
        assert_eq!(stale.validate(), Err(CodecError::InvalidEnum));

        // A retirement has an owner.
        let mut unowned = page;
        unowned.orders[0] = OrderSlot::Tombstone(TombstoneRecord {
            order_id: oid(1),
            owner: Hash32::ZERO,
            retired_generation: 1,
            generation: 2,
        });
        unowned.tombstone_count = 1;
        unowned.page_digest = unowned.recomputed_page_digest().unwrap();
        assert_eq!(unowned.validate(), Err(CodecError::ZeroIdentity));

        // A retirement is still a positional id: it cannot move slots.
        let mut moved = page;
        moved.orders[0] = tombstone(2, owner, 1, 2);
        moved.tombstone_count = 1;
        moved.page_digest = moved.recomputed_page_digest().unwrap();
        assert_eq!(moved.validate(), Err(CodecError::NonCanonicalIdentity));

        // The stored retirement count is a fold, both ways.
        page.orders[0] = tombstone(1, owner, 1, 2);
        page.page_digest = page.recomputed_page_digest().unwrap();
        assert_eq!(page.validate(), Err(CodecError::MismatchedBinding));
        let mut overcounted = page;
        overcounted.tombstone_count = 2;
        overcounted.page_digest = overcounted.recomputed_page_digest().unwrap();
        assert_eq!(overcounted.validate(), Err(CodecError::MismatchedBinding));
        let mut impossible = build_page(0, 1, &[1, 2], Hash32::ZERO);
        impossible.tombstone_count = 3;
        impossible.page_digest = impossible.recomputed_page_digest().unwrap();
        assert_eq!(impossible.validate(), Err(CodecError::InvalidCount));

        // A retirement above `order_count` is padding that is not.
        let mut padded = build_page(0, 1, &[1], Hash32::ZERO);
        padded.orders[5] = tombstone(6, owner, 1, 2);
        padded.page_digest = padded.recomputed_page_digest().unwrap();
        assert_eq!(padded.validate(), Err(CodecError::NonCanonicalPadding));
    }

    #[test]
    fn a_frozen_set_needs_one_live_order_and_the_streaming_closure_agrees() {
        let mut pages = frozen_pages();
        assert_eq!(verify_page_set(&pages), Ok(pages[0].order_set));

        // Retire every record of both pages: the set still closes as bytes, and
        // is still refused as a book, because it has nothing to clear.
        let mut i = 0;
        while i < pages.len() {
            let mut j = 0;
            while j < pages[i].order_count as usize {
                let rank = order_id_rank(pages[i].orders[j].order_id()).unwrap() as u8;
                let owner = pages[i].orders[j].owner();
                pages[i].orders[j] = tombstone(rank, owner, 1, 2);
                j += 1;
            }
            pages[i].tombstone_count = pages[i].order_count;
            pages[i].frozen = 0;
            pages[i].set_order_count = 0;
            pages[i].order_set = Hash32::ZERO;
            pages[i].page_digest = pages[i].recomputed_page_digest().unwrap();
            assert_eq!(pages[i].validate(), Ok(()));
            i += 1;
        }
        freeze_set(&mut pages);
        assert_eq!(verify_page_set(&pages), Err(CodecError::InvalidCount));
        let b0 = encode_page_unchecked(&pages[0]);
        let b1 = encode_page_unchecked(&pages[1]);
        set_agrees("every record retired", &[&b0, &b1]);

        // One live order left is enough.
        let mut revived = pages;
        revived[1].orders[0] = single(17);
        revived[1].tombstone_count -= 1;
        revived[1].frozen = 0;
        revived[1].set_order_count = 0;
        revived[1].order_set = Hash32::ZERO;
        revived[1].page_digest = revived[1].recomputed_page_digest().unwrap();
        revived[0].frozen = 0;
        revived[0].set_order_count = 0;
        revived[0].order_set = Hash32::ZERO;
        freeze_set(&mut revived);
        assert_eq!(verify_page_set(&revived), Ok(revived[0].order_set));
        let r0 = encode_page_unchecked(&revived[0]);
        let r1 = encode_page_unchecked(&revived[1]);
        set_agrees("one live order left", &[&r0, &r1]);
    }

    #[test]
    fn the_epoch_binding_refuses_a_live_record_that_is_already_expired() {
        let pages = frozen_pages();
        let mut e = frozen_epoch();
        e.first_order_id = pages[0].first_order_id;
        e.last_order_id = pages[1].last_order_id;
        e.order_set = pages[0].order_set;
        assert_eq!(e.binds_page_set(&pages), Ok(()));

        // An expiry exactly at this epoch is still live; one below it is not.
        let mut edge = pages;
        edge[0].orders[0] = OrderSlot::Single(OrderRecord {
            expiry_epoch: e.epoch_index,
            ..order(1)
        });
        edge[0].frozen = 0;
        edge[0].set_order_count = 0;
        edge[0].order_set = Hash32::ZERO;
        edge[1].frozen = 0;
        edge[1].set_order_count = 0;
        edge[1].order_set = Hash32::ZERO;
        edge[0].page_digest = edge[0].recomputed_page_digest().unwrap();
        freeze_set(&mut edge);
        let mut edge_epoch = e;
        edge_epoch.order_set = edge[0].order_set;
        assert_eq!(edge_epoch.binds_page_set(&edge), Ok(()));

        let mut stale = edge;
        stale[0].orders[0] = OrderSlot::Single(OrderRecord {
            expiry_epoch: e.epoch_index - 1,
            ..order(1)
        });
        stale[0].frozen = 0;
        stale[0].set_order_count = 0;
        stale[0].order_set = Hash32::ZERO;
        stale[1].frozen = 0;
        stale[1].set_order_count = 0;
        stale[1].order_set = Hash32::ZERO;
        stale[0].page_digest = stale[0].recomputed_page_digest().unwrap();
        freeze_set(&mut stale);
        let mut stale_epoch = e;
        stale_epoch.order_set = stale[0].order_set;
        assert_eq!(
            stale_epoch.binds_page_set(&stale),
            Err(CodecError::MismatchedBinding)
        );
        // The streaming binding gives the identical verdict.
        let s0 = encode_page_unchecked(&stale[0]);
        let s1 = encode_page_unchecked(&stale[1]);
        assert_eq!(
            stream::epoch_binds_page_set(&stale_epoch, &[&s0, &s1]),
            Err(CodecError::MismatchedBinding)
        );

        // A retired record's expiry binds nothing: it is never fed.
        let mut retired = stale;
        let owner = retired[0].orders[0].owner();
        retired[0].orders[0] = tombstone(1, owner, 1, 2);
        retired[0].tombstone_count = 1;
        retired[0].frozen = 0;
        retired[0].set_order_count = 0;
        retired[0].order_set = Hash32::ZERO;
        retired[1].frozen = 0;
        retired[1].set_order_count = 0;
        retired[1].order_set = Hash32::ZERO;
        retired[0].page_digest = retired[0].recomputed_page_digest().unwrap();
        freeze_set(&mut retired);
        let mut retired_epoch = e;
        retired_epoch.order_set = retired[0].order_set;
        assert_eq!(retired_epoch.binds_page_set(&retired), Ok(()));
        let t0 = encode_page_unchecked(&retired[0]);
        let t1 = encode_page_unchecked(&retired[1]);
        assert_eq!(
            stream::epoch_binds_page_set(&retired_epoch, &[&t0, &t1]),
            Ok(())
        );
    }

    #[test]
    fn the_streaming_writer_produces_exactly_the_buffered_encoders_bytes() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 4);
        let mut written = [0; account_len::ORDER_PAGE];
        let header = stream::init_page(&mut written, market, epoch, 0, 1, 5).unwrap();

        // An empty open page is a real page with a real digest, not zeros.
        let empty = build_page(0, 1, &[], Hash32::ZERO);
        let mut encoded = [0; account_len::ORDER_PAGE];
        empty.encode(&mut encoded).unwrap();
        assert_eq!(written, encoded);
        assert_eq!(header, stream::OrderPageHeader::of_page(&empty));
        assert_ne!(header.page_digest, Hash32::ZERO);

        // Two appends: one of each order family, each at the id the page fixes.
        let post = stream::write_single_slot(&mut written, &order(1)).unwrap();
        assert_eq!(post.order_count, 1);
        assert_eq!(post.first_order_id, oid(1));
        assert_eq!(post.last_order_id, oid(1));
        let post = stream::append_slot(&mut written, OrderSlot::Portfolio(portfolio(2))).unwrap();
        assert_eq!(post.order_count, 2);
        assert_eq!(post.last_order_id, oid(2));

        let mut expected = build_page(0, 1, &[1, 2], Hash32::ZERO);
        expected.orders[1] = OrderSlot::Portfolio(portfolio(2));
        reseal(&mut expected);
        let mut encoded = [0; account_len::ORDER_PAGE];
        expected.encode(&mut encoded).unwrap();
        assert_eq!(written, encoded, "the writer's bytes are the encoder's");
        assert_eq!(post, stream::OrderPageHeader::of_page(&expected));
        assert_eq!(stream::verify_page(&written), Ok(post));

        // A retirement, written the same way.
        let owner = order(1).owner;
        let post = stream::write_tombstone(&mut written, oid(1), owner, 2).unwrap();
        assert_eq!(post.order_count, 2);
        assert_eq!(post.tombstone_count, 1);
        assert_eq!(post.live_count(), 1);
        assert_eq!(post.first_order_id, oid(1));
        let mut retired = expected;
        retired.orders[0] = tombstone(1, owner, order(1).generation, 2);
        retired.tombstone_count = 1;
        retired.page_digest = retired.recomputed_page_digest().unwrap();
        let mut encoded = [0; account_len::ORDER_PAGE];
        retired.encode(&mut encoded).unwrap();
        assert_eq!(written, encoded);
        assert_eq!(stream::verify_page(&written), Ok(post));
    }

    #[test]
    fn the_streaming_writer_refuses_every_placement_the_page_format_refuses() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 4);
        let mut page = [0; account_len::ORDER_PAGE];
        let header = stream::init_page(&mut page, market, epoch, 0, 1, 5).unwrap();

        // A caller cannot choose an id: only the one the page's state fixes.
        assert_eq!(header.next_order_id(), Ok(oid(1)));
        let mut chosen = order(1);
        chosen.order_id = oid(2);
        assert_eq!(
            stream::write_single_slot(&mut page, &chosen),
            Err(CodecError::NonCanonicalIdentity)
        );
        chosen.order_id = h(200);
        assert_eq!(
            stream::write_single_slot(&mut page, &chosen),
            Err(CodecError::NonCanonicalIdentity)
        );
        // ... and a refused placement wrote nothing.
        assert_eq!(stream::verify_page(&page), Ok(header));

        // Padding and retirements are not placements.
        assert_eq!(
            stream::append_slot(&mut page, OrderSlot::Empty),
            Err(CodecError::InvalidEnum)
        );
        assert_eq!(
            stream::append_slot(&mut page, tombstone(1, h(20), 1, 2)),
            Err(CodecError::InvalidEnum)
        );
        // Neither is a record the codec would not accept anywhere.
        let mut empty_quantity = order(1);
        empty_quantity.quantity = 0;
        assert_eq!(
            stream::write_single_slot(&mut page, &empty_quantity),
            Err(CodecError::InvalidEnum)
        );

        // A full page has no free slot, and says so as a count fault.
        let mut rank = 1u8;
        while rank <= MAX_ORDERS_PER_PAGE as u8 {
            stream::write_single_slot(&mut page, &order(rank)).unwrap();
            rank += 1;
        }
        let full = stream::verify_page(&page).unwrap();
        assert_eq!(full.order_count as usize, MAX_ORDERS_PER_PAGE);
        assert_eq!(full.next_order_id(), Err(CodecError::InvalidCount));
        assert_eq!(
            stream::write_single_slot(&mut page, &order(17)),
            Err(CodecError::InvalidCount)
        );

        // A page that does not decode is not a page to write to.
        let mut wrong_version = page;
        wrong_version[1] = LAYOUT_VERSION_V3;
        assert_eq!(
            stream::write_single_slot(&mut wrong_version, &order(1)),
            Err(CodecError::WrongVersion)
        );
        assert_eq!(
            stream::write_single_slot(&mut [0; account_len::ORDER_PAGE - 1], &order(1)),
            Err(CodecError::Truncated)
        );
    }

    #[test]
    fn the_streaming_writer_refuses_every_cancellation_the_page_format_refuses() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 4);
        let mut page = [0; account_len::ORDER_PAGE];
        stream::init_page(&mut page, market, epoch, 0, 1, 5).unwrap();
        stream::write_single_slot(&mut page, &order(1)).unwrap();
        stream::write_single_slot(&mut page, &order(2)).unwrap();
        let owner = order(1).owner;

        // An id this page does not hold names no slot here.
        assert_eq!(
            stream::write_tombstone(&mut page, oid(3), owner, 2),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            stream::write_tombstone(&mut page, h(9), owner, 2),
            Err(CodecError::NonCanonicalIdentity)
        );
        assert_eq!(
            stream::write_tombstone(&mut page, Hash32::ZERO, owner, 2),
            Err(CodecError::ZeroIdentity)
        );

        // Only the record's own owner may retire it.
        assert_eq!(
            stream::write_tombstone(&mut page, oid(1), h(21), 2),
            Err(CodecError::MismatchedBinding)
        );
        // The retirement must strictly follow the placement.
        assert_eq!(
            stream::write_tombstone(&mut page, oid(1), owner, 1),
            Err(CodecError::InvalidEnum)
        );
        // Nothing above wrote a byte.
        let before = stream::verify_page(&page).unwrap();
        assert_eq!(before.tombstone_count, 0);

        // The retirement lands, and a replay of it refuses on the slot kind.
        let post = stream::write_tombstone(&mut page, oid(1), owner, 2).unwrap();
        assert_eq!(post.tombstone_count, 1);
        assert_eq!(
            stream::write_tombstone(&mut page, oid(1), owner, 3),
            Err(CodecError::MismatchedBinding)
        );
        // The order after it still holds its own rank.
        assert_eq!(post.last_order_id, oid(2));
        assert_eq!(post.next_order_id(), Ok(oid(3)));
    }

    #[test]
    fn the_freeze_writers_close_a_set_the_closure_then_accepts() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 4);
        let mut page0 = [0; account_len::ORDER_PAGE];
        let mut page1 = [0; account_len::ORDER_PAGE];
        stream::init_page(&mut page0, market, epoch, 0, 2, 5).unwrap();
        stream::init_page(&mut page1, market, epoch, 1, 2, 5).unwrap();

        // A set whose non-final page is not dense cannot be frozen.
        stream::write_single_slot(&mut page0, &order(1)).unwrap();
        stream::write_single_slot(&mut page1, &order(17)).unwrap();
        assert_eq!(
            stream::frozen_set_commitment(&[&page0, &page1]),
            Err(CodecError::InvalidCount)
        );

        let mut rank = 2u8;
        while rank <= MAX_ORDERS_PER_PAGE as u8 {
            stream::write_single_slot(&mut page0, &order(rank)).unwrap();
            rank += 1;
        }
        stream::write_single_slot(&mut page1, &order(18)).unwrap();
        // One of the two retired before the freeze: it stays in the set.
        stream::write_tombstone(&mut page1, oid(18), order(18).owner, 2).unwrap();

        let (order_set, set_order_count) =
            stream::frozen_set_commitment(&[&page0, &page1]).unwrap();
        assert_eq!(set_order_count, MAX_ORDERS_PER_PAGE as u16 + 2);

        // The closure refuses a half-frozen set, and accepts a whole one.
        stream::seal_page(&mut page0, order_set, set_order_count).unwrap();
        assert_eq!(
            stream::verify_page_set(&[&page0, &page1]),
            Err(CodecError::MismatchedBinding)
        );
        let sealed = stream::seal_page(&mut page1, order_set, set_order_count).unwrap();
        assert_eq!(sealed.frozen, 1);
        assert_eq!(sealed.tombstone_count, 1);
        assert_eq!(stream::verify_page_set(&[&page0, &page1]), Ok(order_set));
        set_agrees("writer-built frozen set", &[&page0, &page1]);

        // A freeze is once, and a frozen page takes no more placements.
        assert_eq!(
            stream::seal_page(&mut page1, order_set, set_order_count),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            stream::write_single_slot(&mut page1, &order(19)),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            stream::write_tombstone(&mut page1, oid(17), order(17).owner, 3),
            Err(CodecError::MismatchedBinding)
        );

        // A commitment the pages do not justify is refused before it is stored.
        let mut page2 = [0; account_len::ORDER_PAGE];
        stream::init_page(&mut page2, market, epoch, 0, 1, 5).unwrap();
        stream::write_single_slot(&mut page2, &order(1)).unwrap();
        assert_eq!(
            stream::seal_page(&mut page2, order_set, 0),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            stream::seal_page(&mut page2, Hash32::ZERO, 1),
            Err(CodecError::ZeroIdentity)
        );
    }

    #[test]
    fn placement_intents_carry_either_order_family_and_only_an_order() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 4);
        let mut b = [0; MAX_INTENT_BYTES];

        let single_placement = Intent::PlaceOrder {
            market,
            epoch,
            max_fee_atoms: 7,
            slot: single(1),
        };
        let n = single_placement.encode(&mut b).unwrap();
        assert_eq!(n, 2 + 32 + 32 + 8 + 1 + ORDER_RECORD_BYTES);
        assert_eq!(n, 182);
        assert_eq!(&b[..2], [PLACE_TAG, INTENT_VERSION]);
        assert_eq!(&b[66..74], &7u64.to_le_bytes());
        assert_eq!(b[74], ORDER_KIND_SINGLE);
        assert_eq!(Intent::decode(&b[..n]), Ok(single_placement));

        let portfolio_placement = Intent::PlaceOrder {
            market,
            epoch,
            max_fee_atoms: 9,
            slot: OrderSlot::Portfolio(portfolio(1)),
        };
        let n = portfolio_placement.encode(&mut b).unwrap();
        assert_eq!(n, 2 + 32 + 32 + 8 + 1 + PORTFOLIO_RECORD_BYTES);
        assert_eq!(n, 310);
        assert!(
            n <= MAX_INTENT_BYTES,
            "a portfolio placement is the widest *order* intent"
        );
        /* It was the widest intent of any family until the v2 source-spec
         * construction landed at 402; the exact numbers are pinned side by
         * side in `account_golden_lengths` so neither can drift silently. */
        assert_eq!(&b[66..74], &9u64.to_le_bytes());
        assert_eq!(b[74], ORDER_KIND_PORTFOLIO);
        assert_eq!(Intent::decode(&b[..n]), Ok(portfolio_placement));
        /* The wire body is the kind's *exact* body, not a slot: the widest
         * placement is exactly one slot wide because the portfolio body is what
         * sets the slot width, and the single-Egg one is far narrower than the
         * common width a page would pad it to. */
        assert_eq!(n, 2 + 32 + 32 + 8 + ORDER_SLOT_BYTES);
        assert_eq!(
            single_placement.encoded_len(),
            2 + 32 + 32 + 8 + 1 + ORDER_RECORD_BYTES
        );
        assert!(single_placement.encoded_len() < 2 + 32 + 32 + 8 + ORDER_SLOT_BYTES);

        // Padding and retirements are recognized kinds that are not placements.
        for refused in [OrderSlot::Empty, tombstone(1, h(20), 1, 2)] {
            let i = Intent::PlaceOrder {
                market,
                epoch,
                max_fee_atoms: 0,
                slot: refused,
            };
            assert_eq!(i.encode(&mut b), Err(CodecError::InvalidEnum));
        }
        let n = single_placement.encode(&mut b).unwrap();
        let mut retired_kind = b;
        retired_kind[74] = ORDER_KIND_TOMBSTONE;
        assert_eq!(
            Intent::decode(&retired_kind[..n]),
            Err(CodecError::InvalidEnum)
        );
        let mut empty_kind = b;
        empty_kind[74] = ORDER_KIND_EMPTY;
        assert_eq!(
            Intent::decode(&empty_kind[..n]),
            Err(CodecError::InvalidEnum)
        );
        let mut no_kind = b;
        no_kind[74] = ORDER_KIND_TOMBSTONE + 1;
        assert_eq!(Intent::decode(&no_kind[..n]), Err(CodecError::WrongTag));

        // A placement id is still a rank on the wire.
        let mut wild = order(1);
        wild.order_id = h(9);
        assert_eq!(
            Intent::PlaceOrder {
                market,
                epoch,
                max_fee_atoms: 0,
                slot: OrderSlot::Single(wild),
            }
            .encode(&mut b),
            Err(CodecError::NonCanonicalIdentity)
        );

        // Trailing bytes and truncation are both refused exactly.
        let n = single_placement.encode(&mut b).unwrap();
        assert_eq!(Intent::decode(&b[..n - 1]), Err(CodecError::Truncated));
        assert_eq!(Intent::decode(&b[..n + 1]), Err(CodecError::TrailingBytes));
    }

    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    #[test]
    fn cancellation_intents_name_a_rank_an_owner_and_a_generation() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 4);
        let i = Intent::CancelOrder {
            market,
            epoch,
            owner: h(20),
            order_id: oid(3),
            generation: 7,
        };
        let mut b = [0; MAX_INTENT_BYTES];
        let n = i.encode(&mut b).unwrap();
        assert_eq!(n, 2 + 32 + 32 + 32 + 32 + 8);
        assert_eq!(n, 138);
        assert_eq!(&b[..2], [CANCEL_TAG, INTENT_VERSION]);
        assert_eq!(&b[130..138], &7u64.to_le_bytes());
        assert_eq!(Intent::decode(&b[..n]), Ok(i));

        // The target is an order id, which is a rank and nothing else.
        for wrong in [
            Hash32::ZERO,
            h(9),
            canonical_order_id(MAX_EPOCH_ORDERS as u64 + 1),
        ] {
            let mut raw = b;
            raw[98..130].copy_from_slice(&wrong.0);
            assert!(Intent::decode(&raw[..n]).is_err());
        }
        assert_eq!(
            Intent::CancelOrder {
                market,
                epoch,
                owner: h(20),
                order_id: h(9),
                generation: 7,
            }
            .encode(&mut b),
            Err(CodecError::NonCanonicalIdentity)
        );
    }

    #[cfg(feature = "profile-full")]
    #[test]
    fn direct_submission_intent_has_one_exact_page_wire() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 4);
        let intent = Intent::SubmitDirectPage {
            market,
            epoch,
            page_index: 7,
        };
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = intent.encode(&mut bytes).unwrap();
        assert_eq!(len, 68);
        assert_eq!(&bytes[..2], [SUBMIT_DIRECT_PAGE_TAG, INTENT_VERSION]);
        assert_eq!(&bytes[66..68], &7u16.to_le_bytes());
        assert_eq!(Intent::decode(&bytes[..len]), Ok(intent));

        bytes[2..34].fill(0);
        assert_eq!(Intent::decode(&bytes[..len]), Err(CodecError::ZeroIdentity));
    }

    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    #[test]
    fn clear_work_creation_intents_have_exact_unambiguous_wires() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 7);
        let candidate = h(9);
        let intents = [
            Intent::InitClearWork {
                market,
                epoch,
                candidate,
            },
            Intent::GrowClearWork {
                market,
                epoch,
                candidate,
            },
        ];
        // The Tier 2 clearing family starts exactly one past the Direct V3
        // family's last tag, which its own decoder refuses below.
        let expected_tags = [
            direct_selection_v3::LAST_DIRECT_V3_INTENT_TAG + 1,
            direct_selection_v3::LAST_DIRECT_V3_INTENT_TAG + 2,
        ];
        assert_eq!(expected_tags, [47, 48]);
        for (intent, tag) in intents.into_iter().zip(expected_tags) {
            let mut bytes = [0; MAX_INTENT_BYTES];
            let len = intent.encode(&mut bytes).unwrap();
            assert_eq!(len, 98);
            assert_eq!(intent.encoded_len(), len);
            assert_eq!(&bytes[..2], [tag, INTENT_VERSION]);
            assert_eq!(&bytes[2..34], &market.bytes());
            assert_eq!(&bytes[34..66], &epoch.bytes());
            assert_eq!(&bytes[66..98], &candidate.bytes());
            assert_eq!(Intent::decode(&bytes[..len]), Ok(intent));
            assert_eq!(
                Intent::decode(&bytes[..len - 1]),
                Err(CodecError::Truncated)
            );
            let mut long = [0; MAX_INTENT_BYTES];
            long[..len].copy_from_slice(&bytes[..len]);
            assert_eq!(
                Intent::decode(&long[..len + 1]),
                Err(CodecError::TrailingBytes)
            );
            // A zero identity anywhere in the triple, both directions.
            for at in [2, 34, 66] {
                let mut zeroed = bytes;
                zeroed[at..at + 32].fill(0);
                assert_eq!(
                    Intent::decode(&zeroed[..len]),
                    Err(CodecError::ZeroIdentity)
                );
            }
            let mut wrong_version = bytes;
            wrong_version[1] = INTENT_VERSION + 1;
            assert_eq!(
                Intent::decode(&wrong_version[..len]),
                Err(CodecError::WrongVersion)
            );
        }
        assert_eq!(
            Intent::InitClearWork {
                market: Hash32::ZERO,
                epoch,
                candidate,
            }
            .encode(&mut [0; MAX_INTENT_BYTES]),
            Err(CodecError::ZeroIdentity)
        );
    }

    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    #[test]
    fn general_epoch_intents_have_exact_unambiguous_wires() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 7);
        let policy = h(0x66);

        // Tag continuity: the lifecycle pair takes exactly the next two tags
        // after the staged-creation pair.
        let init = Intent::InitEpoch {
            market,
            epoch_index: 7,
            policy,
            freeze_deadline_slot: 900,
        };
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = init.encode(&mut bytes).unwrap();
        assert_eq!(len, 82);
        assert_eq!(init.encoded_len(), len);
        assert_eq!(&bytes[..2], [49, INTENT_VERSION]);
        assert_eq!(&bytes[2..34], &market.bytes());
        assert_eq!(&bytes[34..42], &7u64.to_le_bytes());
        assert_eq!(&bytes[42..74], &policy.bytes());
        assert_eq!(&bytes[74..82], &900u64.to_le_bytes());
        assert_eq!(Intent::decode(&bytes[..len]), Ok(init));
        assert_eq!(
            Intent::decode(&bytes[..len - 1]),
            Err(CodecError::Truncated)
        );
        let mut long = [0; MAX_INTENT_BYTES];
        long[..len].copy_from_slice(&bytes[..len]);
        assert_eq!(
            Intent::decode(&long[..len + 1]),
            Err(CodecError::TrailingBytes)
        );
        let mut zero_market = bytes;
        zero_market[2..34].fill(0);
        assert_eq!(
            Intent::decode(&zero_market[..len]),
            Err(CodecError::ZeroIdentity)
        );
        let mut zero_policy = bytes;
        zero_policy[42..74].fill(0);
        assert_eq!(
            Intent::decode(&zero_policy[..len]),
            Err(CodecError::ZeroIdentity)
        );
        // No deadline is not a window, in both directions.
        let mut zero_deadline = bytes;
        zero_deadline[74..82].fill(0);
        assert_eq!(
            Intent::decode(&zero_deadline[..len]),
            Err(CodecError::ZeroValue)
        );
        assert_eq!(
            Intent::InitEpoch {
                market,
                epoch_index: 7,
                policy,
                freeze_deadline_slot: 0,
            }
            .encode(&mut [0; MAX_INTENT_BYTES]),
            Err(CodecError::ZeroValue)
        );

        let freeze = Intent::FreezeEpoch { market, epoch };
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = freeze.encode(&mut bytes).unwrap();
        assert_eq!(len, 66);
        assert_eq!(freeze.encoded_len(), len);
        assert_eq!(&bytes[..2], [50, INTENT_VERSION]);
        assert_eq!(&bytes[2..34], &market.bytes());
        assert_eq!(&bytes[34..66], &epoch.bytes());
        assert_eq!(Intent::decode(&bytes[..len]), Ok(freeze));
        assert_eq!(
            Intent::decode(&bytes[..len - 1]),
            Err(CodecError::Truncated)
        );
        for at in [2, 34] {
            let mut zeroed = bytes;
            zeroed[at..at + 32].fill(0);
            assert_eq!(
                Intent::decode(&zeroed[..len]),
                Err(CodecError::ZeroIdentity)
            );
        }
        let mut wrong_version = bytes;
        wrong_version[1] = INTENT_VERSION + 1;
        assert_eq!(
            Intent::decode(&wrong_version[..len]),
            Err(CodecError::WrongVersion)
        );
    }

    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    #[test]
    fn the_walk_intent_has_an_exact_unambiguous_wire() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 7);
        let candidate = h(9);
        let walk = Intent::AdvanceClearWork {
            market,
            epoch,
            candidate,
            max_orders: 16,
        };
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = walk.encode(&mut bytes).unwrap();
        assert_eq!(len, 100);
        assert_eq!(walk.encoded_len(), len);
        assert_eq!(&bytes[..2], [51, INTENT_VERSION]);
        assert_eq!(&bytes[2..34], &market.bytes());
        assert_eq!(&bytes[34..66], &epoch.bytes());
        assert_eq!(&bytes[66..98], &candidate.bytes());
        assert_eq!(&bytes[98..100], &16u16.to_le_bytes());
        assert_eq!(Intent::decode(&bytes[..len]), Ok(walk));
        assert_eq!(
            Intent::decode(&bytes[..len - 1]),
            Err(CodecError::Truncated)
        );
        let mut long = [0; MAX_INTENT_BYTES];
        long[..len].copy_from_slice(&bytes[..len]);
        assert_eq!(
            Intent::decode(&long[..len + 1]),
            Err(CodecError::TrailingBytes)
        );
        for at in [2, 34, 66] {
            let mut zeroed = bytes;
            zeroed[at..at + 32].fill(0);
            assert_eq!(
                Intent::decode(&zeroed[..len]),
                Err(CodecError::ZeroIdentity)
            );
        }
        // A walk that may push nothing, and a bound past the book, both ways.
        let mut zero_batch = bytes;
        zero_batch[98..100].fill(0);
        assert_eq!(
            Intent::decode(&zero_batch[..len]),
            Err(CodecError::InvalidCount)
        );
        let mut over_batch = bytes;
        over_batch[98..100].copy_from_slice(&(MAX_EPOCH_ORDERS as u16 + 1).to_le_bytes());
        assert_eq!(
            Intent::decode(&over_batch[..len]),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            Intent::AdvanceClearWork {
                market,
                epoch,
                candidate,
                max_orders: 0,
            }
            .encode(&mut [0; MAX_INTENT_BYTES]),
            Err(CodecError::InvalidCount)
        );
    }

    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    #[test]
    fn the_slice_and_close_intents_have_exact_unambiguous_wires() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 7);
        let candidate = h(9);

        let slices = Intent::AdvanceClearSlices {
            market,
            epoch,
            candidate,
            max_slices: 100,
        };
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = slices.encode(&mut bytes).unwrap();
        assert_eq!(len, 100);
        assert_eq!(slices.encoded_len(), len);
        assert_eq!(&bytes[..2], [52, INTENT_VERSION]);
        assert_eq!(&bytes[98..100], &100u16.to_le_bytes());
        assert_eq!(Intent::decode(&bytes[..len]), Ok(slices));
        assert_eq!(
            Intent::decode(&bytes[..len - 1]),
            Err(CodecError::Truncated)
        );
        let mut zero_batch = bytes;
        zero_batch[98..100].fill(0);
        assert_eq!(
            Intent::decode(&zero_batch[..len]),
            Err(CodecError::InvalidCount)
        );
        let mut over_batch = bytes;
        over_batch[98..100].copy_from_slice(&(MAX_SLICES as u16 + 1).to_le_bytes());
        assert_eq!(
            Intent::decode(&over_batch[..len]),
            Err(CodecError::InvalidCount)
        );
        for at in [2, 34, 66] {
            let mut zeroed = bytes;
            zeroed[at..at + 32].fill(0);
            assert_eq!(
                Intent::decode(&zeroed[..len]),
                Err(CodecError::ZeroIdentity)
            );
        }

        let close = Intent::CompleteClearWork {
            market,
            epoch,
            candidate,
        };
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = close.encode(&mut bytes).unwrap();
        assert_eq!(len, 98);
        assert_eq!(close.encoded_len(), len);
        assert_eq!(&bytes[..2], [53, INTENT_VERSION]);
        assert_eq!(Intent::decode(&bytes[..len]), Ok(close));
        assert_eq!(
            Intent::decode(&bytes[..len - 1]),
            Err(CodecError::Truncated)
        );
        let mut long = [0; MAX_INTENT_BYTES];
        long[..len].copy_from_slice(&bytes[..len]);
        assert_eq!(
            Intent::decode(&long[..len + 1]),
            Err(CodecError::TrailingBytes)
        );
        for at in [2, 34, 66] {
            let mut zeroed = bytes;
            zeroed[at..at + 32].fill(0);
            assert_eq!(
                Intent::decode(&zeroed[..len]),
                Err(CodecError::ZeroIdentity)
            );
        }
        let mut wrong_version = bytes;
        wrong_version[1] = INTENT_VERSION + 1;
        assert_eq!(
            Intent::decode(&wrong_version[..len]),
            Err(CodecError::WrongVersion)
        );
    }

    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    fn submit_candidate_intent() -> Intent {
        let market = h(1);
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = 4_000;
        prices[1] = 6_000;
        Intent::SubmitCandidate {
            market,
            epoch: canonical_epoch_id(market, 7),
            prices,
            virtual_split: 5,
            virtual_merge: 0,
            honored_aon_mask: 0b11,
            declared_slices: Some(9),
            weighted_direct_volume: -3,
            limit_surplus_price_units: 11,
            distinct_owners: 4,
        }
    }

    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    #[test]
    fn the_submission_intent_has_an_exact_unambiguous_wire() {
        let submit = submit_candidate_intent();
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = submit.encode(&mut bytes).unwrap();
        assert_eq!(len, 255);
        assert_eq!(submit.encoded_len(), len);
        assert_eq!(&bytes[..2], [54, INTENT_VERSION]);
        assert_eq!(Intent::decode(&bytes[..len]), Ok(submit));
        assert_eq!(
            Intent::decode(&bytes[..len - 1]),
            Err(CodecError::Truncated)
        );
        let mut long = [0; MAX_INTENT_BYTES];
        long[..len].copy_from_slice(&bytes[..len]);
        assert_eq!(
            Intent::decode(&long[..len + 1]),
            Err(CodecError::TrailingBytes)
        );
        for at in [2, 34] {
            let mut zeroed = bytes;
            zeroed[at..at + 32].fill(0);
            assert_eq!(
                Intent::decode(&zeroed[..len]),
                Err(CodecError::ZeroIdentity)
            );
        }
        // The witness declaration: flag byte at 218, count at 219..221.
        assert_eq!(bytes[218], 1);
        assert_eq!(&bytes[219..221], &9u16.to_le_bytes());
        let mut bad_flag = bytes;
        bad_flag[218] = 2;
        assert_eq!(
            Intent::decode(&bad_flag[..len]),
            Err(CodecError::InvalidEnum)
        );
        // An undeclared witness carries no length: the count is padding.
        let mut undeclared = bytes;
        undeclared[218] = 0;
        assert_eq!(
            Intent::decode(&undeclared[..len]),
            Err(CodecError::NonCanonicalPadding)
        );
        undeclared[219..221].fill(0);
        assert!(matches!(
            Intent::decode(&undeclared[..len]),
            Ok(Intent::SubmitCandidate {
                declared_slices: None,
                ..
            })
        ));
        // An over-wide declared witness refuses, both directions.
        let mut wide = bytes;
        wide[219..221].copy_from_slice(&(MAX_SLICES as u16 + 1).to_le_bytes());
        assert_eq!(Intent::decode(&wide[..len]), Err(CodecError::InvalidCount));
        // Canonical churn: never split and merge at once (the fixture's
        // split is 5; forging a nonzero merge at 202..210 makes both live).
        let mut churned = bytes;
        churned[202..210].copy_from_slice(&7u64.to_le_bytes());
        assert_eq!(
            Intent::decode(&churned[..len]),
            Err(CodecError::InvalidEnum)
        );
        let mut over_owners = submit_candidate_intent();
        if let Intent::SubmitCandidate {
            distinct_owners, ..
        } = &mut over_owners
        {
            *distinct_owners = MAX_EPOCH_ORDERS as u16 + 1;
        }
        assert_eq!(
            over_owners.encode(&mut [0; MAX_INTENT_BYTES]),
            Err(CodecError::InvalidCount)
        );
    }

    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    #[test]
    fn the_chunked_write_intent_has_an_exact_unambiguous_wire() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 7);
        let candidate = h(9);

        let mut fills = [0u64; FEED_FILLS_PER_CHUNK];
        fills[0] = 12;
        fills[1] = 0;
        fills[2] = 7;
        let fills_chunk = Intent::WriteCandidateFeed {
            market,
            epoch,
            candidate,
            chunk: CandidateFeedChunk::Fills { count: 3, fills },
        };
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = fills_chunk.encode(&mut bytes).unwrap();
        assert_eq!(len, 2 + 96 + 1 + 1 + 24);
        assert_eq!(fills_chunk.encoded_len(), len);
        assert_eq!(&bytes[..2], [55, INTENT_VERSION]);
        assert_eq!(bytes[98], 0, "fills kind");
        assert_eq!(bytes[99], 3, "count");
        assert_eq!(Intent::decode(&bytes[..len]), Ok(fills_chunk));
        assert_eq!(
            Intent::decode(&bytes[..len - 1]),
            Err(CodecError::Truncated)
        );
        let mut long = [0; MAX_INTENT_BYTES];
        long[..len].copy_from_slice(&bytes[..len]);
        assert_eq!(
            Intent::decode(&long[..len + 1]),
            Err(CodecError::TrailingBytes)
        );
        // A chunk that moves nothing, an over-wide one, and an unknown kind.
        let mut empty = bytes;
        empty[99] = 0;
        assert_eq!(
            Intent::decode(&empty[..2 + 96 + 1 + 1]),
            Err(CodecError::InvalidCount)
        );
        let mut wrong_kind = bytes;
        wrong_kind[98] = 2;
        assert_eq!(
            Intent::decode(&wrong_kind[..len]),
            Err(CodecError::InvalidEnum)
        );
        // Dirty in-memory padding refuses at encode.
        let mut dirty = fills;
        dirty[3] = 1;
        assert_eq!(
            Intent::WriteCandidateFeed {
                market,
                epoch,
                candidate,
                chunk: CandidateFeedChunk::Fills {
                    count: 3,
                    fills: dirty
                },
            }
            .encode(&mut [0; MAX_INTENT_BYTES]),
            Err(CodecError::NonCanonicalPadding)
        );

        let mut slices = [clearing::PairingSlice::PADDING; FEED_SLICES_PER_CHUNK];
        slices[0] = clearing::PairingSlice {
            buy_ref: clearing::LegRef::Order(2),
            sell_ref: clearing::LegRef::Split,
            outcome: 1,
            quantity: 5,
        };
        slices[1] = clearing::PairingSlice {
            buy_ref: clearing::LegRef::Merge,
            sell_ref: clearing::LegRef::Order(0),
            outcome: 0,
            quantity: 2,
        };
        let slices_chunk = Intent::WriteCandidateFeed {
            market,
            epoch,
            candidate,
            chunk: CandidateFeedChunk::Slices { count: 2, slices },
        };
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = slices_chunk.encode(&mut bytes).unwrap();
        assert_eq!(len, 2 + 96 + 1 + 1 + 26);
        assert_eq!(slices_chunk.encoded_len(), len);
        assert_eq!(bytes[98], 1, "slices kind");
        assert_eq!(Intent::decode(&bytes[..len]), Ok(slices_chunk));
        // The widest slices chunk is the widest intent this family emits and
        // it stays inside the frozen bound.
        let full = Intent::WriteCandidateFeed {
            market,
            epoch,
            candidate,
            chunk: CandidateFeedChunk::Slices {
                count: FEED_SLICES_PER_CHUNK as u8,
                slices: [slices[0]; FEED_SLICES_PER_CHUNK],
            },
        };
        assert_eq!(full.encoded_len(), 308);
        assert!(full.encoded_len() <= MAX_INTENT_BYTES);
        let mut widest = [0; MAX_INTENT_BYTES];
        let len = full.encode(&mut widest).unwrap();
        assert_eq!(Intent::decode(&widest[..len]), Ok(full));
        // A virtual leg on its refused side and a zero-quantity slice refuse.
        let mut backwards = slices;
        backwards[0].buy_ref = clearing::LegRef::Split;
        assert_eq!(
            Intent::WriteCandidateFeed {
                market,
                epoch,
                candidate,
                chunk: CandidateFeedChunk::Slices {
                    count: 2,
                    slices: backwards
                },
            }
            .encode(&mut [0; MAX_INTENT_BYTES]),
            Err(CodecError::InvalidEnum)
        );
        let mut still = slices;
        still[1].quantity = 0;
        assert_eq!(
            Intent::WriteCandidateFeed {
                market,
                epoch,
                candidate,
                chunk: CandidateFeedChunk::Slices {
                    count: 2,
                    slices: still
                },
            }
            .encode(&mut [0; MAX_INTENT_BYTES]),
            Err(CodecError::ZeroValue)
        );
    }

    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    #[test]
    fn the_seal_and_selection_intents_have_exact_unambiguous_wires() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 7);
        let candidate = h(9);

        let seal = Intent::SealCandidate {
            market,
            epoch,
            candidate,
        };
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = seal.encode(&mut bytes).unwrap();
        assert_eq!(len, 98);
        assert_eq!(seal.encoded_len(), len);
        assert_eq!(&bytes[..2], [56, INTENT_VERSION]);
        assert_eq!(Intent::decode(&bytes[..len]), Ok(seal));
        for at in [2, 34, 66] {
            let mut zeroed = bytes;
            zeroed[at..at + 32].fill(0);
            assert_eq!(
                Intent::decode(&zeroed[..len]),
                Err(CodecError::ZeroIdentity)
            );
        }

        let finalize = Intent::FinalizeSelection { market, epoch };
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = finalize.encode(&mut bytes).unwrap();
        assert_eq!(len, 66);
        assert_eq!(finalize.encoded_len(), len);
        assert_eq!(&bytes[..2], [57, INTENT_VERSION]);
        assert_eq!(Intent::decode(&bytes[..len]), Ok(finalize));
        assert_eq!(
            Intent::decode(&bytes[..len - 1]),
            Err(CodecError::Truncated)
        );
        let mut long = [0; MAX_INTENT_BYTES];
        long[..len].copy_from_slice(&bytes[..len]);
        assert_eq!(
            Intent::decode(&long[..len + 1]),
            Err(CodecError::TrailingBytes)
        );
        for at in [2, 34] {
            let mut zeroed = bytes;
            zeroed[at..at + 32].fill(0);
            assert_eq!(
                Intent::decode(&zeroed[..len]),
                Err(CodecError::ZeroIdentity)
            );
        }
        let mut wrong_version = bytes;
        wrong_version[1] = INTENT_VERSION + 1;
        assert_eq!(
            Intent::decode(&wrong_version[..len]),
            Err(CodecError::WrongVersion)
        );
        // Tag continuity: the selection family takes exactly the next four
        // tags after the walk family's close.
        assert_eq!(
            [
                SUBMIT_CANDIDATE_TAG,
                WRITE_CANDIDATE_FEED_TAG,
                SEAL_CANDIDATE_TAG,
                FINALIZE_SELECTION_TAG
            ],
            [54, 55, 56, 57]
        );
    }

    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    #[test]
    fn the_entitlement_intents_have_exact_unambiguous_wires() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 7);
        let candidate = h(9);

        let freeze = Intent::FreezeEntitlement {
            market,
            epoch,
            candidate,
        };
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = freeze.encode(&mut bytes).unwrap();
        assert_eq!(len, 98);
        assert_eq!(freeze.encoded_len(), len);
        assert_eq!(&bytes[..2], [58, INTENT_VERSION]);
        assert_eq!(Intent::decode(&bytes[..len]), Ok(freeze));
        for at in [2, 34, 66] {
            let mut zeroed = bytes;
            zeroed[at..at + 32].fill(0);
            assert_eq!(
                Intent::decode(&zeroed[..len]),
                Err(CodecError::ZeroIdentity)
            );
        }
        assert_eq!(
            Intent::decode(&bytes[..len - 1]),
            Err(CodecError::Truncated)
        );

        let entitle = Intent::EntitleSlice {
            market,
            epoch,
            candidate,
            slice_index: 3,
        };
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = entitle.encode(&mut bytes).unwrap();
        assert_eq!(len, 100);
        assert_eq!(entitle.encoded_len(), len);
        assert_eq!(&bytes[..2], [59, INTENT_VERSION]);
        assert_eq!(Intent::decode(&bytes[..len]), Ok(entitle));
        let mut long = [0; MAX_INTENT_BYTES];
        long[..len].copy_from_slice(&bytes[..len]);
        assert_eq!(
            Intent::decode(&long[..len + 1]),
            Err(CodecError::TrailingBytes)
        );
        let mut wrong_version = bytes;
        wrong_version[1] = INTENT_VERSION + 1;
        assert_eq!(
            Intent::decode(&wrong_version[..len]),
            Err(CodecError::WrongVersion)
        );
        // Tag continuity: the entitlement pair takes exactly the next two
        // tags after the selection family's close.
        assert_eq!(
            [FREEZE_ENTITLEMENT_TAG, ENTITLE_SLICE_TAG],
            [FINALIZE_SELECTION_TAG + 1, FINALIZE_SELECTION_TAG + 2]
        );
        assert_eq!([FREEZE_ENTITLEMENT_TAG, ENTITLE_SLICE_TAG], [58, 59]);
    }

    #[cfg(feature = "profile-full")]
    #[test]
    fn direct_v3_authority_intents_have_exact_wires() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 7);
        let intents = [
            Intent::InitDirectEpochV3 {
                market,
                epoch_index: 7,
                policy: h(4),
                submission_opens_slot: 100,
                submission_closes_slot: 120,
            },
            Intent::FreezeDirectEpochV3 { market, epoch },
            Intent::SubmitDirectCandidateV2 {
                market,
                epoch,
                outcome_price: 2_500,
            },
            Intent::SelectDirectWindowV1 { market, epoch },
            Intent::SettleDirectV2 { market, epoch },
        ];
        let expected = [90usize, 66, 74, 66, 66];
        let expected_tags = [27u8, 28, 29, 30, 31];
        let mut index = 0usize;
        while index < intents.len() {
            let mut bytes = [0; MAX_INTENT_BYTES];
            let len = intents[index].encode(&mut bytes).unwrap();
            assert_eq!(len, expected[index]);
            assert_eq!(bytes[0], expected_tags[index]);
            assert_eq!(bytes[1], INTENT_VERSION);
            assert_eq!(Intent::decode(&bytes[..len]), Ok(intents[index]));
            assert_eq!(
                Intent::decode(&bytes[..len - 1]),
                Err(CodecError::Truncated)
            );
            index += 1;
        }
    }

    #[cfg(feature = "profile-full")]
    #[test]
    fn every_intent_refuses_the_superseded_encoding_version() {
        let market = h(1);
        let epoch = canonical_epoch_id(market, 4);
        let intents = [
            Intent::Split {
                market,
                owner: h(2),
                quantity: 9,
            },
            Intent::PlaceOrder {
                market,
                epoch,
                max_fee_atoms: 0,
                slot: single(1),
            },
            Intent::PlaceOrder {
                market,
                epoch,
                max_fee_atoms: 0,
                slot: OrderSlot::Portfolio(portfolio(1)),
            },
            Intent::CancelOrder {
                market,
                epoch,
                owner: h(20),
                order_id: oid(1),
                generation: 2,
            },
            Intent::SettlePage {
                market,
                epoch,
                page_index: 0,
            },
            Intent::SubmitDirectPage {
                market,
                epoch,
                page_index: 0,
            },
            Intent::InitRealm {
                profile: h(3),
                realm_nonce: 5,
                max_outcomes: MAX_OUTCOMES as u8,
                profile_version: 2,
            },
            Intent::InitProfileV2 {
                realm: h(4),
                collateral_policy_id: h(5),
                adapter_release_id: h(6),
                profile_version: 2,
            },
            Intent::InitPriceGrid {
                realm: h(4),
                grid: h(6),
            },
            Intent::InitTerms {
                realm: h(4),
                terms: h(7),
            },
            Intent::InitOrderPage {
                market,
                epoch,
                page_index: 1,
                page_count: 4,
            },
            Intent::Endow {
                market,
                owner: h(8),
                amount: 12,
            },
            Intent::FreezeEntitlement {
                market,
                epoch,
                candidate: h(9),
            },
            Intent::EntitleSlice {
                market,
                epoch,
                candidate: h(9),
                slice_index: 1,
            },
        ];
        assert_eq!(INTENT_VERSION, 3);
        assert_eq!(INTENT_VERSION_V2, 2);
        assert_eq!(INTENT_VERSION_V1, 1);
        let mut i = 0;
        while i < intents.len() {
            let mut b = [0; MAX_INTENT_BYTES];
            let n = intents[i].encode(&mut b).unwrap();
            assert_eq!(b[1], INTENT_VERSION);
            assert_eq!(n, intents[i].encoded_len());
            assert!(n <= MAX_INTENT_BYTES);
            assert_eq!(Intent::decode(&b[..n]), Ok(intents[i]));
            b[1] = INTENT_VERSION_V1;
            assert_eq!(Intent::decode(&b[..n]), Err(CodecError::WrongVersion));
            b[1] = INTENT_VERSION_V2;
            assert_eq!(Intent::decode(&b[..n]), Err(CodecError::WrongVersion));
            b[1] = INTENT_VERSION + 1;
            assert_eq!(Intent::decode(&b[..n]), Err(CodecError::WrongVersion));
            i += 1;
        }
    }

    #[test]
    fn streaming_and_buffered_verdicts_agree_on_hostile_retirements() {
        let owner = order(1).owner;
        let mut page = build_page(0, 1, &[1, 2], Hash32::ZERO);
        page.orders[0] = tombstone(1, owner, 1, 2);
        page.tombstone_count = 1;
        page.page_digest = page.recomputed_page_digest().unwrap();
        agrees("one retirement", &encode_page_unchecked(&page));

        let refused: [(&str, OrderPageAccount); 5] = [
            ("retirement count not folded", {
                let mut p = page;
                p.tombstone_count = 0;
                p.page_digest = p.recomputed_page_digest().unwrap();
                p
            }),
            ("more retirements than slots", {
                let mut p = page;
                p.tombstone_count = 9;
                p.page_digest = p.recomputed_page_digest().unwrap();
                p
            }),
            ("retirement out of position", {
                let mut p = page;
                p.orders[0] = tombstone(2, owner, 1, 2);
                p.page_digest = p.recomputed_page_digest().unwrap();
                p
            }),
            ("retirement before its placement", {
                let mut p = page;
                p.orders[0] = tombstone(1, owner, 5, 2);
                p.page_digest = p.recomputed_page_digest().unwrap();
                p
            }),
            ("retirement smuggled above the count", {
                let mut p = build_page(0, 1, &[1], Hash32::ZERO);
                p.orders[7] = tombstone(8, owner, 1, 2);
                p.page_digest = p.recomputed_page_digest().unwrap();
                p
            }),
        ];
        let mut i = 0;
        while i < refused.len() {
            let (label, p) = refused[i];
            let bytes = encode_page_unchecked(&p);
            assert!(
                stream::verify_page(&bytes).is_err(),
                "fixture is supposed to refuse: {label}"
            );
            agrees(label, &bytes);
            i += 1;
        }

        // Nonzero padding after a retirement body is still non-canonical.
        let clean = encode_page_unchecked(&page);
        let mut dirty = clean;
        dirty[PAGE_HEADER_BYTES + 1 + TOMBSTONE_RECORD_BYTES] = 1;
        assert_eq!(
            stream::verify_page(&dirty),
            Err(CodecError::NonCanonicalPadding)
        );
        agrees("dirty retirement padding", &dirty);
    }

    /* --------------------------------------------------------------------
     * The genesis intents.
     *
     * Six intents that bring accounts into existence, plus the one that
     * credits a position's opening cash.  Each is pinned at its exact width
     * and round-tripped, then every field that can carry a lie is made to
     * carry one.
     * ----------------------------------------------------------------- */

    /// Every genesis intent, at the values the width test pins.
    fn genesis_intents() -> [Intent; 6] {
        let market = h(1);
        [
            Intent::InitRealm {
                profile: h(3),
                realm_nonce: 5,
                max_outcomes: MAX_OUTCOMES as u8,
                profile_version: 2,
            },
            Intent::InitProfileV2 {
                realm: h(4),
                collateral_policy_id: h(5),
                adapter_release_id: h(6),
                profile_version: 2,
            },
            Intent::InitPriceGrid {
                realm: h(4),
                grid: h(6),
            },
            Intent::InitTerms {
                realm: h(4),
                terms: h(7),
            },
            Intent::InitOrderPage {
                market,
                epoch: canonical_epoch_id(market, 4),
                page_index: 1,
                page_count: 4,
            },
            Intent::Endow {
                market,
                owner: h(8),
                amount: 12,
            },
        ]
    }

    #[test]
    fn genesis_intents_round_trip_at_their_exact_widths() {
        let widths = [44_usize, 99, 66, 66, 70, 74];
        let tags = [
            INIT_REALM_TAG,
            INIT_PROFILE_TAG,
            INIT_PRICE_GRID_TAG,
            INIT_TERMS_TAG,
            INIT_ORDER_PAGE_TAG,
            ENDOW_TAG,
        ];
        let intents = genesis_intents();
        let mut i = 0;
        while i < intents.len() {
            let mut b = [0; MAX_INTENT_BYTES];
            let n = intents[i].encode(&mut b).unwrap();
            assert_eq!(n, widths[i], "width of intent {i}");
            assert_eq!(n, intents[i].encoded_len());
            assert_eq!(b[0], tags[i]);
            assert_eq!(b[1], INTENT_VERSION);
            assert_eq!(Intent::decode(&b[..n]), Ok(intents[i]));
            assert_eq!(Intent::decode(&b[..n - 1]), Err(CodecError::Truncated));
            let mut longer = [0; MAX_INTENT_BYTES + 1];
            longer[..n].copy_from_slice(&b[..n]);
            assert_eq!(
                Intent::decode(&longer[..n + 1]),
                Err(CodecError::TrailingBytes)
            );
            let mut small = [0; 8];
            assert_eq!(
                intents[i].encode(&mut small),
                Err(CodecError::OutputTooSmall)
            );
            i += 1;
        }
        /* Not one of them widens the wire: the budget is exactly the v2
         * source-spec construction, which is what makes `MAX_INTENT_BYTES` a
         * measurement rather than a reservation. */
        let mut widest = 0;
        let mut i = 0;
        while i < widths.len() {
            if widths[i] > widest {
                widest = widths[i];
            }
            i += 1;
        }
        assert!(widest < MAX_INTENT_BYTES);
        assert_eq!(MAX_INTENT_BYTES, 402);
    }

    #[test]
    fn genesis_intents_refuse_every_hostile_field() {
        let refused: [(&str, Intent, CodecError); 12] = [
            (
                "a Realm with no Profile",
                Intent::InitRealm {
                    profile: Hash32::ZERO,
                    realm_nonce: 0,
                    max_outcomes: MAX_OUTCOMES as u8,
                    profile_version: 2,
                },
                CodecError::ZeroIdentity,
            ),
            (
                "a Realm narrower than V1 admits",
                Intent::InitRealm {
                    profile: h(3),
                    realm_nonce: 0,
                    max_outcomes: 8,
                    profile_version: 2,
                },
                CodecError::InvalidCount,
            ),
            (
                "a Realm expecting Profile version zero",
                Intent::InitRealm {
                    profile: h(3),
                    realm_nonce: 0,
                    max_outcomes: MAX_OUTCOMES as u8,
                    profile_version: 0,
                },
                CodecError::InvalidEnum,
            ),
            (
                "a Profile freezing the unfrozen sentinel",
                Intent::InitProfileV2 {
                    realm: h(4),
                    collateral_policy_id: Hash32::ZERO,
                    adapter_release_id: h(6),
                    profile_version: 2,
                },
                CodecError::ZeroIdentity,
            ),
            (
                "a Profile naming no adapter release",
                Intent::InitProfileV2 {
                    realm: h(4),
                    collateral_policy_id: h(5),
                    adapter_release_id: Hash32::ZERO,
                    profile_version: 2,
                },
                CodecError::InvalidEnum,
            ),
            (
                "a grid under no Realm",
                Intent::InitPriceGrid {
                    realm: Hash32::ZERO,
                    grid: h(6),
                },
                CodecError::ZeroIdentity,
            ),
            (
                "a grid that is not a digest",
                Intent::InitPriceGrid {
                    realm: h(4),
                    grid: Hash32::ZERO,
                },
                CodecError::ZeroIdentity,
            ),
            (
                "terms that are not a digest",
                Intent::InitTerms {
                    realm: h(4),
                    terms: Hash32::ZERO,
                },
                CodecError::ZeroIdentity,
            ),
            (
                "a page set with no pages",
                Intent::InitOrderPage {
                    market: h(1),
                    epoch: canonical_epoch_id(h(1), 0),
                    page_index: 0,
                    page_count: 0,
                },
                CodecError::InvalidCount,
            ),
            (
                "a page set wider than one book",
                Intent::InitOrderPage {
                    market: h(1),
                    epoch: canonical_epoch_id(h(1), 0),
                    page_index: 0,
                    page_count: MAX_ORDER_PAGES as u16 + 1,
                },
                CodecError::InvalidCount,
            ),
            (
                "a page outside its own set",
                Intent::InitOrderPage {
                    market: h(1),
                    epoch: canonical_epoch_id(h(1), 0),
                    page_index: 4,
                    page_count: 4,
                },
                CodecError::InvalidCount,
            ),
            (
                "an endowment of nothing",
                Intent::Endow {
                    market: h(1),
                    owner: h(8),
                    amount: 0,
                },
                CodecError::ZeroValue,
            ),
        ];
        let mut i = 0;
        while i < refused.len() {
            let (label, intent, error) = refused[i];
            let mut b = [0; MAX_INTENT_BYTES];
            assert_eq!(intent.encode(&mut b), Err(error), "{label}");
            i += 1;
        }
    }

    /// The decoder is not the encoder run backwards: bytes that never came
    /// from `encode` must hit the same refusals.
    #[test]
    fn genesis_intent_bytes_refuse_hostile_wire_forms() {
        let intents = genesis_intents();
        // A zeroed identity smuggled straight into the bytes.
        let mut i = 0;
        while i < intents.len() {
            let mut b = [0; MAX_INTENT_BYTES];
            let n = intents[i].encode(&mut b).unwrap();
            b[2..2 + HASH_BYTES].fill(0);
            assert_eq!(Intent::decode(&b[..n]), Err(CodecError::ZeroIdentity));
            i += 1;
        }
        // A page index at its own count, written past the encoder.
        let mut b = [0; MAX_INTENT_BYTES];
        let page = Intent::InitOrderPage {
            market: h(1),
            epoch: canonical_epoch_id(h(1), 0),
            page_index: 0,
            page_count: 1,
        };
        let n = page.encode(&mut b).unwrap();
        b[66..68].copy_from_slice(&1_u16.to_le_bytes());
        assert_eq!(Intent::decode(&b[..n]), Err(CodecError::InvalidCount));
        // An endowment of zero, written past the encoder.
        let endow = Intent::Endow {
            market: h(1),
            owner: h(8),
            amount: 1,
        };
        let n = endow.encode(&mut b).unwrap();
        b[66..74].fill(0);
        assert_eq!(Intent::decode(&b[..n]), Err(CodecError::ZeroValue));
        // A tag past the last one this version defines.
        b[0] = resolution_work::ABORT_RESOLUTION_WORK_TAG + 1;
        assert_eq!(Intent::decode(&b[..n]), Err(CodecError::WrongTag));
    }

    #[test]
    fn external_redemption_binds_bearer_and_both_token_accounts() {
        let intent = Intent::RedeemExternal {
            market: h(1),
            claimant: h(2),
            source: h(3),
            destination: h(4),
            outcome: 1,
            quantity: 17,
        };
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = intent.encode(&mut bytes).unwrap();
        assert_eq!(len, 2 + (4 * HASH_BYTES) + 1 + 8);
        assert_eq!(bytes[0], REDEEM_EXTERNAL_TAG);
        assert_eq!(Intent::decode(&bytes[..len]), Ok(intent));
        assert_eq!(
            Intent::decode(&bytes[..len - 1]),
            Err(CodecError::Truncated)
        );

        for invalid in [
            Intent::RedeemExternal {
                market: h(1),
                claimant: Hash32::ZERO,
                source: h(3),
                destination: h(4),
                outcome: 1,
                quantity: 17,
            },
            Intent::RedeemExternal {
                market: h(1),
                claimant: h(2),
                source: Hash32::ZERO,
                destination: h(4),
                outcome: 1,
                quantity: 17,
            },
            Intent::RedeemExternal {
                market: h(1),
                claimant: h(2),
                source: h(3),
                destination: Hash32::ZERO,
                outcome: 1,
                quantity: 17,
            },
            Intent::RedeemExternal {
                market: h(1),
                claimant: h(2),
                source: h(3),
                destination: h(4),
                outcome: MAX_OUTCOMES as u8,
                quantity: 17,
            },
            Intent::RedeemExternal {
                market: h(1),
                claimant: h(2),
                source: h(3),
                destination: h(4),
                outcome: 1,
                quantity: 0,
            },
        ] {
            assert!(invalid.encode(&mut bytes).is_err());
        }
    }

    #[test]
    fn cash_withdrawal_binds_owner_destination_and_nonzero_amount() {
        let intent = Intent::WithdrawCash {
            market: h(1),
            owner: h(2),
            destination: h(3),
            amount: 17,
        };
        let mut bytes = [0; MAX_INTENT_BYTES];
        let len = intent.encode(&mut bytes).unwrap();
        assert_eq!(len, 2 + (3 * HASH_BYTES) + 8);
        assert_eq!(bytes[0], WITHDRAW_CASH_TAG);
        assert_eq!(Intent::decode(&bytes[..len]), Ok(intent));
        assert_eq!(
            Intent::decode(&bytes[..len - 1]),
            Err(CodecError::Truncated)
        );

        for invalid in [
            Intent::WithdrawCash {
                market: Hash32::ZERO,
                owner: h(2),
                destination: h(3),
                amount: 17,
            },
            Intent::WithdrawCash {
                market: h(1),
                owner: Hash32::ZERO,
                destination: h(3),
                amount: 17,
            },
            Intent::WithdrawCash {
                market: h(1),
                owner: h(2),
                destination: Hash32::ZERO,
                amount: 17,
            },
            Intent::WithdrawCash {
                market: h(1),
                owner: h(2),
                destination: h(3),
                amount: 0,
            },
        ] {
            assert!(invalid.encode(&mut bytes).is_err());
        }

        let mut encoded = [0; MAX_INTENT_BYTES];
        let n = intent.encode(&mut encoded).unwrap();
        encoded[66..98].fill(0);
        assert_eq!(Intent::decode(&encoded[..n]), Err(CodecError::ZeroIdentity));
        let n = intent.encode(&mut encoded).unwrap();
        encoded[98..106].fill(0);
        assert_eq!(Intent::decode(&encoded[..n]), Err(CodecError::ZeroValue));
    }

    #[test]
    fn artifact_transport_intents_have_exact_unambiguous_wires() {
        let kind = artifact::ArtifactKind::CollateralPolicy;
        let context = h(1);
        let digest = h(2);
        let exact_len = kind.exact_len() as u16;
        let mut chunk = [0_u8; artifact::ARTIFACT_CHUNK_BYTES];
        chunk[..3].copy_from_slice(&[7, 8, 9]);
        let intents = [
            Intent::BeginArtifact {
                kind,
                context,
                digest,
                exact_len,
                expires_slot: 51,
            },
            Intent::WriteArtifact {
                kind,
                context,
                digest,
                cursor: 0,
                chunk_len: 3,
                chunk,
            },
            Intent::SealArtifact {
                kind,
                context,
                digest,
                exact_len,
            },
            Intent::AbortArtifact {
                kind,
                context,
                digest,
            },
        ];
        let expected = [77, 263, 69, 67];
        let tags = [
            BEGIN_ARTIFACT_TAG,
            WRITE_ARTIFACT_TAG,
            SEAL_ARTIFACT_TAG,
            ABORT_ARTIFACT_TAG,
        ];
        let mut i = 0;
        while i < intents.len() {
            let mut encoded = [0_u8; MAX_INTENT_BYTES];
            let n = intents[i].encode(&mut encoded).unwrap();
            assert_eq!(n, expected[i]);
            assert_eq!(n, intents[i].encoded_len());
            assert_eq!(&encoded[..2], &[tags[i], INTENT_VERSION]);
            assert_eq!(Intent::decode(&encoded[..n]), Ok(intents[i]));
            assert_eq!(
                Intent::decode(&encoded[..n - 1]),
                Err(CodecError::Truncated)
            );
            assert_eq!(
                Intent::decode(&encoded[..n + 1]),
                Err(CodecError::TrailingBytes)
            );
            let mut unknown_kind = encoded;
            unknown_kind[2] = 0;
            assert_eq!(
                Intent::decode(&unknown_kind[..n]),
                Err(CodecError::InvalidEnum)
            );
            i += 1;
        }

        let mut encoded = [0_u8; MAX_INTENT_BYTES];
        assert_eq!(
            Intent::BeginArtifact {
                kind,
                context: Hash32::ZERO,
                digest,
                exact_len,
                expires_slot: 51,
            }
            .encode(&mut encoded),
            Err(CodecError::ZeroIdentity)
        );
        assert_eq!(
            Intent::SealArtifact {
                kind,
                context,
                digest,
                exact_len: exact_len - 1,
            }
            .encode(&mut encoded),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            Intent::WriteArtifact {
                kind,
                context,
                digest,
                cursor: 0,
                chunk_len: 0,
                chunk,
            }
            .encode(&mut encoded),
            Err(CodecError::NonCanonicalPadding)
        );

        let write = intents[1];
        let n = write.encode(&mut encoded).unwrap();
        encoded[74] = 1;
        assert_eq!(
            Intent::decode(&encoded[..n]),
            Err(CodecError::NonCanonicalPadding)
        );
    }

    #[cfg(feature = "profile-full")]
    #[test]
    fn authenticated_source_intents_have_exact_unambiguous_wires() {
        let mut body = [0_u8; SOURCE_SPEC_BODY_V1_BYTES];
        for (index, byte) in body.iter_mut().enumerate() {
            *byte = index as u8;
        }
        let intents = [
            Intent::InitSourceSpec {
                terms: h(1),
                spec_body: body,
            },
            Intent::InitSourceArchive { terms: h(1) },
            Intent::AppendSourceArchive { terms: h(1) },
            Intent::SealSourceArchive { terms: h(1) },
        ];
        let lengths = [290, 34, 34, 34];
        let tags = [
            INIT_SOURCE_SPEC_TAG,
            INIT_SOURCE_ARCHIVE_TAG,
            APPEND_SOURCE_ARCHIVE_TAG,
            SEAL_SOURCE_ARCHIVE_TAG,
        ];
        for ((intent, expected_len), expected_tag) in intents.into_iter().zip(lengths).zip(tags) {
            let mut encoded = [0_u8; MAX_INTENT_BYTES];
            let len = intent.encode(&mut encoded).unwrap();
            assert_eq!(len, expected_len);
            assert_eq!(len, intent.encoded_len());
            assert_eq!(&encoded[..2], &[expected_tag, INTENT_VERSION]);
            assert_eq!(Intent::decode(&encoded[..len]), Ok(intent));
            assert_eq!(
                Intent::decode(&encoded[..len - 1]),
                Err(CodecError::Truncated)
            );
            assert_eq!(
                Intent::decode(&encoded[..len + 1]),
                Err(CodecError::TrailingBytes)
            );
            encoded[2..34].fill(0);
            assert_eq!(
                Intent::decode(&encoded[..len]),
                Err(CodecError::ZeroIdentity)
            );
        }
        assert!(intents[0].encoded_len() <= MAX_INTENT_BYTES);
    }

    #[test]
    fn authenticated_source_v2_intents_have_exact_unambiguous_wires() {
        let mut body = [0_u8; SOURCE_SPEC_BODY_V2_BYTES];
        for (index, byte) in body.iter_mut().enumerate() {
            *byte = index as u8;
        }
        let intents = [
            Intent::InitSourceSpecV2 {
                terms: h(1),
                spec_body: body,
            },
            Intent::InitSourceArchiveV2 { terms: h(1) },
            Intent::AppendSourceArchiveV2 { terms: h(1) },
            Intent::SealSourceArchiveV2 { terms: h(1) },
        ];
        let lengths = [402, 34, 34, 34];
        let tags = [
            INIT_SOURCE_SPEC_V2_TAG,
            INIT_SOURCE_ARCHIVE_V2_TAG,
            APPEND_SOURCE_ARCHIVE_V2_TAG,
            SEAL_SOURCE_ARCHIVE_V2_TAG,
        ];
        assert_eq!(tags, [70_u8, 71, 72, 73]);
        for ((intent, expected_len), expected_tag) in intents.into_iter().zip(lengths).zip(tags) {
            let mut encoded = [0_u8; MAX_INTENT_BYTES];
            let len = intent.encode(&mut encoded).unwrap();
            assert_eq!(len, expected_len);
            assert_eq!(len, intent.encoded_len());
            assert_eq!(&encoded[..2], &[expected_tag, INTENT_VERSION]);
            assert_eq!(Intent::decode(&encoded[..len]), Ok(intent));
            assert_eq!(
                Intent::decode(&encoded[..len - 1]),
                Err(CodecError::Truncated)
            );
            let mut longer = [0_u8; MAX_INTENT_BYTES + 1];
            longer[..len].copy_from_slice(&encoded[..len]);
            assert_eq!(
                Intent::decode(&longer[..len + 1]),
                Err(CodecError::TrailingBytes)
            );
            encoded[2..34].fill(0);
            assert_eq!(
                Intent::decode(&encoded[..len]),
                Err(CodecError::ZeroIdentity)
            );
        }

        /* The two generations are disjoint on the wire, not merely different:
         * a V1 spec body carried under the v2 tag is short by 112 bytes and a
         * v2 body under the V1 tag is long by the same, so neither can be read
         * as the other even with an otherwise valid Terms binding. */
        let v1 = Intent::InitSourceSpec {
            terms: h(1),
            spec_body: [7_u8; SOURCE_SPEC_BODY_V1_BYTES],
        };
        let mut v1_bytes = [0_u8; MAX_INTENT_BYTES];
        let v1_len = v1.encode(&mut v1_bytes).unwrap();
        v1_bytes[0] = INIT_SOURCE_SPEC_V2_TAG;
        assert_eq!(
            Intent::decode(&v1_bytes[..v1_len]),
            Err(CodecError::Truncated)
        );
        let mut v2_bytes = [0_u8; MAX_INTENT_BYTES];
        let v2_len = intents[0].encode(&mut v2_bytes).unwrap();
        v2_bytes[0] = INIT_SOURCE_SPEC_TAG;
        let v1_tag_result = if cfg!(feature = "profile-full") {
            CodecError::TrailingBytes
        } else {
            CodecError::WrongTag
        };
        assert_eq!(Intent::decode(&v2_bytes[..v2_len]), Err(v1_tag_result));

        // The widest admitted intent is exactly this one.
        assert_eq!(intents[0].encoded_len(), MAX_INTENT_BYTES);
    }
}
