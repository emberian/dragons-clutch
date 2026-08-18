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

/// Frozen wire version for this prototype.
pub const LAYOUT_VERSION: u8 = 1;
/// Number of bytes in every identity/hash field.
pub const HASH_BYTES: usize = 32;
/// Maximum number of outcomes in a market.
pub const MAX_OUTCOMES: usize = 16;
/// Maximum records in one order page.
pub const MAX_ORDERS_PER_PAGE: usize = 16;
/// Maximum encoded instruction length.
pub const MAX_INTENT_BYTES: usize = 256;

const REALM_TAG: u8 = 1;
const PROFILE_TAG: u8 = 2;
const MARKET_TAG: u8 = 3;
const HOARD_TAG: u8 = 5;
const POSITION_TAG: u8 = 6;
const FEED_TAG: u8 = 7;
const ORDER_PAGE_TAG: u8 = 8;

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
    /// Account or intent discriminator was not recognized.
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
    /// Arithmetic would exceed the fixed representation.
    ArithmeticOverflow,
    /// The destination buffer is too short.
    OutputTooSmall,
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

/// Derive the profile hash from an exact profile byte sequence.
pub fn canonical_profile_hash(profile_bytes: &[u8]) -> ProfileHash {
    digest(b"dragons-clutch/profile/v1", &[profile_bytes])
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

/// Check a market/outcome identity pair.
pub fn validate_outcome_id(market: MarketId, index: u8, id: OutcomeId) -> Result<()> {
    if index >= MAX_OUTCOMES as u8 || id != canonical_outcome_id(market, index) {
        return Err(CodecError::NonCanonicalIdentity);
    }
    Ok(())
}

/// Account discriminator and exact fixed byte lengths.
pub mod account_len {
    /// Realm account bytes.
    pub const REALM: usize = 2 + 32 + 32 + 1 + 1 + 1 + 1;
    /// Profile account bytes.
    pub const PROFILE: usize = 2 + 32 + 32 + 1 + 1;
    /// Market account bytes.
    pub const MARKET: usize = 2 + 32 + 32 + 32 + 32 + 1 + 1 + 1 + 1 + 512 + 32 + 8 + 8 + 32;
    /// Hoard account bytes.
    pub const HOARD: usize = 2 + 32 + 32 + 32 + 8 + 1 + 1;
    /// Position account bytes.
    pub const POSITION: usize = 2 + 32 + 32 + 8 + 128 + 8 + 8 + 1 + 1;
    /// Feed head account bytes.
    pub const FEED: usize = 2 + 32 + 32 + 8 + 8 + 8 + 32 + 1 + 1;
    /// Dense order page account bytes.
    pub const ORDER_PAGE: usize = 2 + 32 + 32 + 2 + 2 + 1 + 1 + (16 * 99);
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
    /// Profile schema version.
    pub version: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
}

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

/// A dense, canonically ordered page of transparent order records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderPageAccount {
    /// Market identity.
    pub market: MarketId,
    /// Epoch identity.
    pub epoch: EpochId,
    /// Zero-based page index.
    pub page_index: u16,
    /// Total frozen page count.
    pub page_count: u16,
    /// Number of populated records.
    pub order_count: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Orders in strictly increasing canonical order ID.
    pub orders: [OrderRecord; MAX_ORDERS_PER_PAGE],
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

impl OrderRecord {
    const ZERO: Self = Self {
        owner: Hash32::ZERO,
        order_id: Hash32::ZERO,
        outcome: 0,
        side: 0,
        quantity: 0,
        limit: 0,
        minimum_fill: 0,
        flags: 0,
        generation: 0,
    };
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

fn check_header(input: &[u8], tag: u8, len: usize) -> Result<()> {
    if input.len() < len {
        return Err(CodecError::Truncated);
    }
    if input.len() > len {
        return Err(CodecError::TrailingBytes);
    }
    if input[0] != tag {
        return Err(CodecError::WrongTag);
    }
    if input[1] != LAYOUT_VERSION {
        return Err(CodecError::WrongVersion);
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
    fn u64(&mut self, value: u64) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }
    fn hash(&mut self, value: Hash32) -> Result<()> {
        self.bytes(&value.0)
    }
}

struct Reader<'a> {
    input: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn new(input: &'a [u8], tag: u8, len: usize) -> Result<Self> {
        check_header(input, tag, len)?;
        Ok(Self { input, at: 2 })
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
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes::<8>()?))
    }
    fn hash(&mut self) -> Result<Hash32> {
        Ok(Hash32(self.bytes::<32>()?))
    }
    fn done(self) -> Result<()> {
        if self.at == self.input.len() {
            Ok(())
        } else {
            Err(CodecError::TrailingBytes)
        }
    }
}

fn put_header(w: &mut Writer<'_>, tag: u8) -> Result<()> {
    w.u8(tag)?;
    w.u8(LAYOUT_VERSION)
}

impl RealmAccount {
    /// Validate semantic shape without external account metadata.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.realm)?;
        check_hash(self.profile)?;
        check_count(self.max_outcomes)?;
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
        put_header(&mut w, REALM_TAG)?;
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
        let mut r = Reader::new(input, REALM_TAG, account_len::REALM)?;
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
    /// Validate profile shape and parent identity.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.profile)?;
        check_hash(self.realm)?;
        if self.version == 0 || self.flags != 0 {
            return Err(CodecError::InvalidEnum);
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
        put_header(&mut w, PROFILE_TAG)?;
        w.hash(self.profile)?;
        w.hash(self.realm)?;
        w.u8(self.version)?;
        w.u8(self.flags)?;
        Ok(w.at)
    }
    /// Parse exactly [`account_len::PROFILE`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(input, PROFILE_TAG, account_len::PROFILE)?;
        let v = Self {
            profile: r.hash()?,
            realm: r.hash()?,
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
        put_header(&mut w, MARKET_TAG)?;
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
        let mut r = Reader::new(input, MARKET_TAG, account_len::MARKET)?;
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
        put_header(&mut w, HOARD_TAG)?;
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
        let mut r = Reader::new(input, HOARD_TAG, account_len::HOARD)?;
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
    /// Validate close state and canonical zero padding.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.market)?;
        check_hash(self.owner)?;
        if self.close_state > 2 {
            return Err(CodecError::InvalidEnum);
        };
        Ok(())
    }
    /// Encode exactly [`account_len::POSITION`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::POSITION {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, POSITION_TAG)?;
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
        let mut r = Reader::new(input, POSITION_TAG, account_len::POSITION)?;
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
        put_header(&mut w, FEED_TAG)?;
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
        let mut r = Reader::new(input, FEED_TAG, account_len::FEED)?;
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
    /// Validate dense ordering, page bounds, records, and zero padding.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.market)?;
        check_hash(self.epoch)?;
        if self.page_count == 0
            || self.page_index >= self.page_count
            || self.order_count as usize > MAX_ORDERS_PER_PAGE
        {
            return Err(CodecError::InvalidCount);
        };
        let mut previous = Hash32::ZERO;
        let mut i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            if i < self.order_count as usize {
                self.orders[i].validate()?;
                if self.orders[i].order_id == Hash32::ZERO
                    || (previous != Hash32::ZERO && self.orders[i].order_id.0 <= previous.0)
                {
                    return Err(CodecError::NonCanonicalIdentity);
                };
                previous = self.orders[i].order_id;
            } else if self.orders[i] != OrderRecord::ZERO {
                return Err(CodecError::NonCanonicalPadding);
            };
            i += 1;
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
        put_header(&mut w, ORDER_PAGE_TAG)?;
        w.hash(self.market)?;
        w.hash(self.epoch)?;
        w.u16(self.page_index)?;
        w.u16(self.page_count)?;
        w.u8(self.order_count)?;
        w.u8(self.stored_bump)?;
        let mut i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            encode_order(&mut w, self.orders[i])?;
            i += 1;
        }
        Ok(w.at)
    }
    /// Parse exactly [`account_len::ORDER_PAGE`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(input, ORDER_PAGE_TAG, account_len::ORDER_PAGE)?;
        let market = r.hash()?;
        let epoch = r.hash()?;
        let page_index = r.u16()?;
        let page_count = r.u16()?;
        let order_count = r.u8()?;
        let stored_bump = r.u8()?;
        let mut orders = [OrderRecord::ZERO; MAX_ORDERS_PER_PAGE];
        let mut i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            orders[i] = decode_order(&mut r)?;
            i += 1;
        }
        let v = Self {
            market,
            epoch,
            page_index,
            page_count,
            order_count,
            stored_bump,
            orders,
        };
        r.done()?;
        v.validate()?;
        Ok(v)
    }
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
                put_header(&mut w, CREATE_TAG)?;
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
                put_header(&mut w, FEED_ADVANCE_TAG)?;
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
                put_header(&mut w, PLACE_TAG)?;
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
                put_header(&mut w, CANCEL_TAG)?;
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
                put_header(&mut w, SETTLE_TAG)?;
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
        let mut r = Reader::new(input, tag, input.len())?;
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
    #[test]
    fn account_golden_lengths() {
        assert_eq!(account_len::REALM, 70);
        assert_eq!(account_len::PROFILE, 68);
        assert_eq!(account_len::MARKET, 726);
        assert_eq!(account_len::HOARD, 108);
        assert_eq!(account_len::POSITION, 220);
        assert_eq!(account_len::FEED, 124);
        assert_eq!(account_len::ORDER_PAGE, 1656);
    }
    #[test]
    fn market_round_trip_and_golden_prefix() {
        let v = market();
        let mut b = [0; account_len::MARKET];
        assert_eq!(v.encode(&mut b), Ok(account_len::MARKET));
        assert_eq!(&b[..2], [MARKET_TAG, LAYOUT_VERSION]);
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
            cash_atoms: 10,
            reserved_cash_atoms: 11,
            stored_bump: 12,
            close_state: 0,
        };
        let mut position_bytes = [0; account_len::POSITION];
        position.encode(&mut position_bytes).unwrap();
        assert_eq!(PositionAccount::decode(&position_bytes), Ok(position));

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

        let order = OrderRecord {
            owner: h(20),
            order_id: h(21),
            outcome: 0,
            side: 1,
            quantity: 22,
            limit: 23,
            minimum_fill: 22,
            flags: 1,
            generation: 24,
        };
        let mut orders = [OrderRecord::ZERO; MAX_ORDERS_PER_PAGE];
        orders[0] = order;
        let page = OrderPageAccount {
            market: h(3),
            epoch: h(25),
            page_index: 0,
            page_count: 1,
            order_count: 1,
            stored_bump: 26,
            orders,
        };
        let mut page_bytes = [0; account_len::ORDER_PAGE];
        page.encode(&mut page_bytes).unwrap();
        assert_eq!(OrderPageAccount::decode(&page_bytes), Ok(page));
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
        assert_eq!(&b[..2], [SPLIT_TAG, LAYOUT_VERSION]);
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
        raw[1] = LAYOUT_VERSION;
        raw[66..74].copy_from_slice(&1u64.to_le_bytes());
        assert_eq!(Intent::decode(&raw), Err(CodecError::ZeroIdentity));
    }
    #[test]
    fn page_rejects_duplicate_or_unsorted_orders() {
        let o = OrderRecord {
            owner: h(1),
            order_id: h(3),
            outcome: 0,
            side: 0,
            quantity: 1,
            limit: 2,
            minimum_fill: 1,
            flags: 0,
            generation: 0,
        };
        let mut orders = [OrderRecord::ZERO; MAX_ORDERS_PER_PAGE];
        orders[0] = o;
        orders[1] = o;
        let p = OrderPageAccount {
            market: h(1),
            epoch: h(2),
            page_index: 0,
            page_count: 1,
            order_count: 2,
            stored_bump: 1,
            orders,
        };
        assert_eq!(p.validate(), Err(CodecError::NonCanonicalIdentity));
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
}
