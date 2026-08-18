//! The 266-byte Realm collateral policy, its digest, and the parent Profile
//! binding that a Profile account's 32 reserved bytes commit to.
//!
//! # What this module closes
//!
//! `docs/implementation/RESOLUTION_EVIDENCE_PLAN.md` §3.4 left four obligations
//! on the Rust side.  Obligation 1 (an exact 64-byte length requirement on
//! [`crate::canonical_profile_hash`]) landed earlier.  This module is
//! obligations 2, 3, and 4: the parent encoder/decoder, a binding rule that
//! **recomputes** the child digest rather than merely decoding, and the
//! cross-language golden vectors.
//!
//! The policy encoding, every refusal, and both digest rules are owned by
//! `research/collateral-profiles/model.py` and tabulated in
//! `docs/implementation/COLLATERAL_PROFILES.md`.  Nothing here re-decides any of
//! them; this is a byte-for-byte port whose adversarial tests are the Python
//! test suite's refusals, plus the frozen vectors of
//! `research/collateral-profiles/identity_vectors.json` transcribed as fixtures.
//!
//! # Decoding authenticates nothing — the load-bearing negative
//!
//! [`CollateralPolicy::decode`] accepts *any* well-formed policy, and
//! [`ParentProfile::decode`] accepts *any* well-formed parent, including a
//! parent that commits to a different Realm's collateral policy.  A parser is
//! not evidence.  Only [`verify_collateral_binding`] — which recomputes the
//! child digest from the actual 266 policy bytes and compares it against
//! [`crate::ProfileAccount::collateral_policy_digest`] — decides that a Profile
//! commits to *this* policy, and only [`verify_profile_identity`] additionally
//! decides that the account's Profile ID is the canonical parent hash over that
//! same digest.  The Rust tests carry the same three binding refusals the Python
//! corpus does, for exactly this reason.
//!
//! # What this module still does not establish
//!
//! An admitted policy is not an admitted mint.  Everything in
//! `COLLATERAL_PROFILES.md` "Authority, supply, and account policy" — the mint
//! and Hoard token-account snapshot checks, TLV extension parsing, account
//! owner/executable checks, program pinning, and PDA derivation — requires
//! authenticating real accounts, which no offline crate can do.  This module
//! decides policy *bytes* and *identity composition* only.

use super::{
    canonical_profile_hash, is_zero, CodecError, Hash32, ProfileAccount, ProfileHash, Reader,
    Result, Writer, HASH_BYTES, PROFILE_FLAG_POLICY_FROZEN, PROFILE_PARENT_BYTES,
};

/// Exact byte length of one canonical collateral policy.
pub const COLLATERAL_POLICY_BYTES: usize = 266;
/// Exact byte length of one canonical currency reference inside a policy.
pub const CURRENCY_REF_BYTES: usize = 66;
/// Zero reserved bytes at the tail of a canonical collateral policy.
pub const COLLATERAL_POLICY_RESERVED_BYTES: usize = 16;
/// Zero reserved bytes at the tail of a canonical parent Profile preimage.
pub const PARENT_PROFILE_RESERVED_BYTES: usize = 16;
/// Collateral-policy magic: ASCII `DCCOLP1` and one zero byte.
pub const COLLATERAL_POLICY_MAGIC: [u8; 8] = *b"DCCOLP1\0";
/// Parent Profile magic: ASCII `DCPROF1` and one zero byte.
pub const PARENT_PROFILE_MAGIC: [u8; 8] = *b"DCPROF1\0";
/// The only collateral-policy schema version this build understands.
pub const COLLATERAL_POLICY_SCHEMA: u16 = 1;
/// The only parent-Profile schema version this build understands.
pub const PARENT_PROFILE_SCHEMA: u16 = 1;
/// The only parent-Profile flag word V1 admits.
pub const PARENT_PROFILE_FLAGS: u16 = 0;
/// Parent subfield tag naming the collateral policy.
pub const SUBFIELD_COLLATERAL_POLICY: u16 = 1;

/// Child digest domain.
///
/// Note the deliberate asymmetry with [`PARENT_PROFILE_DOMAIN`]: the child
/// domain carries a trailing zero byte and the parent domain does not.  Both are
/// unambiguous only because each payload has a fixed length
/// ([`COLLATERAL_POLICY_BYTES`] and [`PROFILE_PARENT_BYTES`]) and a distinct
/// magic.  A future variable-length payload under either domain must re-derive
/// prefix-freeness rather than inherit this note.
pub const COLLATERAL_POLICY_DOMAIN: &[u8] = b"dragons-clutch/collateral-profile/v1\0";
/// Parent digest domain, unchanged and shared with [`crate::canonical_profile_hash`].
pub const PARENT_PROFILE_DOMAIN: &[u8] = b"dragons-clutch/profile/v1";

/// Policy flag: the collateral mint must have no mint authority.
pub const FLAG_REQUIRE_MINT_AUTHORITY_NONE: u16 = 1 << 0;
/// Policy flag: the collateral mint must have no freeze authority.
pub const FLAG_REQUIRE_FREEZE_AUTHORITY_NONE: u16 = 1 << 1;
/// Policy flag: the collateral mint supply must be positive.
pub const FLAG_REQUIRE_NONZERO_SUPPLY: u16 = 1 << 2;
/// Policy flag: the Hoard token account must have no delegate.
pub const FLAG_REQUIRE_ACCOUNT_DELEGATE_NONE: u16 = 1 << 3;
/// Policy flag: the Hoard token account must have no close authority.
pub const FLAG_REQUIRE_ACCOUNT_CLOSE_AUTHORITY_NONE: u16 = 1 << 4;
/// Every flag bit this schema recognizes.
pub const COLLATERAL_POLICY_KNOWN_FLAGS: u16 = FLAG_REQUIRE_MINT_AUTHORITY_NONE
    | FLAG_REQUIRE_FREEZE_AUTHORITY_NONE
    | FLAG_REQUIRE_NONZERO_SUPPLY
    | FLAG_REQUIRE_ACCOUNT_DELEGATE_NONE
    | FLAG_REQUIRE_ACCOUNT_CLOSE_AUTHORITY_NONE;
/// The flag word V1 requires: a Realm may not weaken the authority/state policy.
pub const COLLATERAL_POLICY_STRICT_FLAGS: u16 = COLLATERAL_POLICY_KNOWN_FLAGS;

/// Number of pinned Token-2022 extension discriminants (`0..=28`).
///
/// Pinned to token-2022 source commit `426400f`; see the matrix in
/// `docs/implementation/COLLATERAL_PROFILES.md`.
pub const EXTENSION_DISCRIMINANTS: u32 = 29;
/// Every extension bit position this schema recognizes.
///
/// A bit outside this mask is a future or invented discriminant and fails
/// closed, in every one of the four bitsets.
pub const EXTENSION_KNOWN_MASK: u64 = (1 << EXTENSION_DISCRIMINANTS) - 1;
/// Bit position of the `ImmutableOwner` account extension.
pub const EXTENSION_IMMUTABLE_OWNER: u64 = 1 << 7;
/// Protocol ceiling on mint extensions: V1 admits none.
pub const PROTOCOL_MINT_EXTENSION_CEILING: u64 = 0;
/// Protocol ceiling on account extensions: V1 admits `ImmutableOwner` only.
pub const PROTOCOL_ACCOUNT_EXTENSION_CEILING: u64 = EXTENSION_IMMUTABLE_OWNER;

/// The legacy SPL Token program, `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`.
pub const LEGACY_TOKEN_PROGRAM: [u8; HASH_BYTES] = [
    0x06, 0xdd, 0xf6, 0xe1, 0xd7, 0x65, 0xa1, 0x93, 0xd9, 0xcb, 0xe1, 0x46, 0xce, 0xeb, 0x79, 0xac,
    0x1c, 0xb4, 0x85, 0xed, 0x5f, 0x5b, 0x37, 0x91, 0x3a, 0x8c, 0xf5, 0x85, 0x7e, 0xff, 0x00, 0xa9,
];
/// The Token-2022 program, `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb`.
pub const TOKEN_2022_PROGRAM: [u8; HASH_BYTES] = [
    0x06, 0xdd, 0xf6, 0xe1, 0xee, 0x75, 0x8f, 0xde, 0x18, 0x42, 0x5d, 0xbc, 0xe4, 0x6c, 0xcd, 0xda,
    0xb6, 0x1a, 0xfc, 0x4d, 0x83, 0xb9, 0x0d, 0x27, 0xfe, 0xbd, 0xf9, 0x28, 0xd8, 0xa1, 0x8b, 0xfc,
];
/// Decimals every native-SOL currency reference must carry.
pub const NATIVE_SOL_DECIMALS: u8 = 9;

const _: () = assert!(
    COLLATERAL_POLICY_BYTES
        == 8 + 2 + 2 + (3 * CURRENCY_REF_BYTES) + (5 * 8) + COLLATERAL_POLICY_RESERVED_BYTES
);
const _: () =
    assert!(PROFILE_PARENT_BYTES == 8 + 2 + 2 + 2 + 2 + HASH_BYTES + PARENT_PROFILE_RESERVED_BYTES);
const _: () = assert!(CURRENCY_REF_BYTES == 1 + HASH_BYTES + HASH_BYTES + 1);
const _: () = assert!(PROTOCOL_MINT_EXTENSION_CEILING & !EXTENSION_KNOWN_MASK == 0);
const _: () = assert!(PROTOCOL_ACCOUNT_EXTENSION_CEILING & !EXTENSION_KNOWN_MASK == 0);

/// Which accounting identity a currency reference names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrencyKind {
    /// Native SOL: zero program, zero mint, nine decimals.
    NativeSol,
    /// An SPL token on one of the two admitted token programs.
    SplToken,
}

impl CurrencyKind {
    /// The encoded discriminant byte.
    pub const fn byte(self) -> u8 {
        match self {
            Self::NativeSol => 0,
            Self::SplToken => 1,
        }
    }
    /// Decode a discriminant byte, refusing every unknown value.
    pub const fn from_byte(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::NativeSol),
            1 => Ok(Self::SplToken),
            _ => Err(CodecError::InvalidEnum),
        }
    }
}

/// One atom-denominated currency identity, exactly [`CURRENCY_REF_BYTES`] bytes.
///
/// A policy names three of these in three *separate* accounting roles —
/// collateral, fee, and liveness.  Sharing a mint between roles never merges the
/// identities; see `COLLATERAL_PROFILES.md` "Three currencies, separate
/// accounting identities".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrencyRef {
    /// Native SOL or SPL token.
    pub kind: CurrencyKind,
    /// Owning token program; zero exactly when `kind` is native SOL.
    pub token_program: [u8; HASH_BYTES],
    /// Mint identity; zero exactly when `kind` is native SOL.
    pub mint: [u8; HASH_BYTES],
    /// Atom exponent; exactly [`NATIVE_SOL_DECIMALS`] for native SOL.
    pub decimals: u8,
}

impl CurrencyRef {
    /// The canonical native-SOL reference.
    pub const NATIVE_SOL: Self = Self {
        kind: CurrencyKind::NativeSol,
        token_program: [0; HASH_BYTES],
        mint: [0; HASH_BYTES],
        decimals: NATIVE_SOL_DECIMALS,
    };

    /// An SPL token reference on a named program and mint.
    pub const fn spl(
        token_program: [u8; HASH_BYTES],
        mint: [u8; HASH_BYTES],
        decimals: u8,
    ) -> Self {
        Self {
            kind: CurrencyKind::SplToken,
            token_program,
            mint,
            decimals,
        }
    }

    /// Refuse every reference this schema does not admit.
    ///
    /// Native SOL must carry zero program and mint slots and nine decimals; an
    /// SPL token must carry a nonzero mint on one of the two pinned programs.
    pub fn validate(&self) -> Result<()> {
        match self.kind {
            CurrencyKind::NativeSol => {
                if !is_zero(&self.token_program) || !is_zero(&self.mint) {
                    return Err(CodecError::NonCanonicalPadding);
                }
                if self.decimals != NATIVE_SOL_DECIMALS {
                    return Err(CodecError::InvalidCount);
                }
            }
            CurrencyKind::SplToken => {
                if is_zero(&self.token_program) || is_zero(&self.mint) {
                    return Err(CodecError::ZeroIdentity);
                }
                if !self.is_legacy_token() && !bytes_eq(&self.token_program, &TOKEN_2022_PROGRAM) {
                    return Err(CodecError::InvalidEnum);
                }
            }
        }
        Ok(())
    }

    /// Whether this reference names the legacy SPL Token program.
    pub const fn is_legacy_token(&self) -> bool {
        bytes_eq(&self.token_program, &LEGACY_TOKEN_PROGRAM)
    }

    fn encode(&self, w: &mut Writer<'_>) -> Result<()> {
        w.u8(self.kind.byte())?;
        w.bytes(&self.token_program)?;
        w.bytes(&self.mint)?;
        w.u8(self.decimals)
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self> {
        let value = Self {
            kind: CurrencyKind::from_byte(r.u8()?)?,
            token_program: r.bytes::<HASH_BYTES>()?,
            mint: r.bytes::<HASH_BYTES>()?,
            decimals: r.u8()?,
        };
        value.validate()?;
        Ok(value)
    }
}

const fn bytes_eq(left: &[u8; HASH_BYTES], right: &[u8; HASH_BYTES]) -> bool {
    let mut i = 0;
    while i < HASH_BYTES {
        if left[i] != right[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// The immutable, collateral-generic Realm policy: exactly
/// [`COLLATERAL_POLICY_BYTES`] bytes.
///
/// Field order and offsets are the table in `COLLATERAL_PROFILES.md`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollateralPolicy {
    /// Schema version; only [`COLLATERAL_POLICY_SCHEMA`] is understood.
    pub schema_version: u16,
    /// Strict authority/state flags; V1 requires [`COLLATERAL_POLICY_STRICT_FLAGS`].
    pub flags: u16,
    /// The collateral currency backing complete-set liabilities in the Hoard.
    pub collateral: CurrencyRef,
    /// The fee currency; V1 admits collateral or native SOL.
    pub fee: CurrencyRef,
    /// The liveness currency; V1 requires native SOL.
    pub liveness: CurrencyRef,
    /// Maximum accepted collateral **mint supply**, in atoms.
    ///
    /// This is an asset-quality admission ceiling on the mint, not a per-market
    /// solvency cap; see [`CollateralPolicy::market_cap_ceiling_atoms`].
    pub max_supply_atoms: u64,
    /// Mint extensions the Realm admits; must not exceed the protocol ceiling.
    pub allowed_mint_extensions: u64,
    /// Mint extensions the Realm requires; must also be allowed.
    pub required_mint_extensions: u64,
    /// Account extensions the Realm admits; must not exceed the protocol ceiling.
    pub allowed_account_extensions: u64,
    /// Account extensions the Realm requires; must also be allowed.
    pub required_account_extensions: u64,
}

impl CollateralPolicy {
    /// Refuse every policy this schema does not admit.
    ///
    /// The order mirrors `RealmCollateralProfile.__post_init__` so that a policy
    /// with several faults reports the same one in both languages.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != COLLATERAL_POLICY_SCHEMA {
            return Err(CodecError::WrongVersion);
        }
        self.collateral.validate()?;
        self.fee.validate()?;
        self.liveness.validate()?;
        // V1 collateral is an SPL token: native SOL has no mint to admit and no
        // Hoard token account to authenticate.
        if !matches!(self.collateral.kind, CurrencyKind::SplToken) {
            return Err(CodecError::InvalidEnum);
        }
        if self.max_supply_atoms == 0 {
            return Err(CodecError::ZeroValue);
        }
        if self.flags & !COLLATERAL_POLICY_KNOWN_FLAGS != 0 {
            return Err(CodecError::InvalidEnum);
        }
        // A Realm may narrow the extension ceiling but may never weaken the
        // authority/state policy, so the flag word is fixed rather than merely
        // known.
        if self.flags != COLLATERAL_POLICY_STRICT_FLAGS {
            return Err(CodecError::InvalidEnum);
        }
        if !currency_eq(&self.fee, &self.collateral)
            && !currency_eq(&self.fee, &CurrencyRef::NATIVE_SOL)
        {
            return Err(CodecError::MismatchedBinding);
        }
        if !currency_eq(&self.liveness, &CurrencyRef::NATIVE_SOL) {
            return Err(CodecError::MismatchedBinding);
        }
        if (self.allowed_mint_extensions
            | self.required_mint_extensions
            | self.allowed_account_extensions
            | self.required_account_extensions)
            & !EXTENSION_KNOWN_MASK
            != 0
        {
            return Err(CodecError::InvalidEnum);
        }
        if self.required_mint_extensions & !self.allowed_mint_extensions != 0
            || self.required_account_extensions & !self.allowed_account_extensions != 0
        {
            return Err(CodecError::InvalidEnum);
        }
        if self.allowed_mint_extensions & !PROTOCOL_MINT_EXTENSION_CEILING != 0
            || self.allowed_account_extensions & !PROTOCOL_ACCOUNT_EXTENSION_CEILING != 0
        {
            return Err(CodecError::InvalidEnum);
        }
        if self.collateral.is_legacy_token()
            && (self.allowed_mint_extensions
                | self.required_mint_extensions
                | self.allowed_account_extensions
                | self.required_account_extensions)
                != 0
        {
            return Err(CodecError::InvalidEnum);
        }
        Ok(())
    }

    /// Encode exactly [`COLLATERAL_POLICY_BYTES`] canonical bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < COLLATERAL_POLICY_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        w.bytes(&COLLATERAL_POLICY_MAGIC)?;
        w.u16(self.schema_version)?;
        w.u16(self.flags)?;
        self.collateral.encode(&mut w)?;
        self.fee.encode(&mut w)?;
        self.liveness.encode(&mut w)?;
        w.u64(self.max_supply_atoms)?;
        w.u64(self.allowed_mint_extensions)?;
        w.u64(self.required_mint_extensions)?;
        w.u64(self.allowed_account_extensions)?;
        w.u64(self.required_account_extensions)?;
        w.bytes(&[0; COLLATERAL_POLICY_RESERVED_BYTES])?;
        Ok(w.at)
    }

    /// The canonical byte image of this policy.
    pub fn canonical_bytes(&self) -> Result<[u8; COLLATERAL_POLICY_BYTES]> {
        let mut out = [0; COLLATERAL_POLICY_BYTES];
        self.encode(&mut out)?;
        Ok(out)
    }

    /// Parse exactly [`COLLATERAL_POLICY_BYTES`] hostile bytes.
    ///
    /// Every refusal of `RealmCollateralProfile.from_canonical_bytes` is here,
    /// in that function's order: exact length, magic, zero reserved bytes, the
    /// three currency references, every policy-level constraint, and finally a
    /// byte-for-byte re-encode.
    ///
    /// A successful decode says the bytes are a well-formed V1 policy.  It says
    /// nothing about which Realm committed to them; that is
    /// [`verify_collateral_binding`].
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() < COLLATERAL_POLICY_BYTES {
            return Err(CodecError::Truncated);
        }
        if input.len() > COLLATERAL_POLICY_BYTES {
            return Err(CodecError::TrailingBytes);
        }
        if input[..8] != COLLATERAL_POLICY_MAGIC {
            return Err(CodecError::WrongTag);
        }
        let mut i = COLLATERAL_POLICY_BYTES - COLLATERAL_POLICY_RESERVED_BYTES;
        while i < COLLATERAL_POLICY_BYTES {
            if input[i] != 0 {
                return Err(CodecError::NonCanonicalPadding);
            }
            i += 1;
        }
        let mut r = Reader::at(input, 8);
        let value = Self {
            schema_version: r.u16()?,
            flags: r.u16()?,
            collateral: CurrencyRef::decode(&mut r)?,
            fee: CurrencyRef::decode(&mut r)?,
            liveness: CurrencyRef::decode(&mut r)?,
            max_supply_atoms: r.u64()?,
            allowed_mint_extensions: r.u64()?,
            required_mint_extensions: r.u64()?,
            allowed_account_extensions: r.u64()?,
            required_account_extensions: r.u64()?,
        };
        value.validate()?;
        // Byte-for-byte canonicality.  Every field above is already fully
        // constrained, so this can only fire on an encoder/decoder divergence —
        // which is exactly the bug a cross-language port must not ship.
        if value.canonical_bytes()?[..] != *input {
            return Err(CodecError::NonCanonicalPadding);
        }
        Ok(value)
    }

    /// The domain-separated child digest `D_col` of this policy.
    ///
    /// `SHA-256("dragons-clutch/collateral-profile/v1" || 0x00 || bytes)`.
    pub fn digest(&self) -> Result<Hash32> {
        let bytes = self.canonical_bytes()?;
        Ok(super::digest(COLLATERAL_POLICY_DOMAIN, &[&bytes]))
    }

    /// The ceiling any honest per-market collateral cap must respect.
    ///
    /// # This is a bound, not the cap
    ///
    /// [`crate::MarketAccount::collateral_cap`] is a **per-market** limit on
    /// Hoard atoms; the reference adapter and the SBF program both refuse a
    /// split whose resulting collateral would exceed it.  This policy carries no
    /// such field.  [`CollateralPolicy::max_supply_atoms`] is a *Realm-wide
    /// admission constraint on the mint*: it says which mints are acceptable
    /// collateral at all, and `COLLATERAL_PROFILES.md` states in as many words
    /// that "the supply ceiling is not a solvency proof".
    ///
    /// Using it *as* the cap would silently grant every market in a Realm
    /// permission to absorb the entire admitted mint supply, which constrains
    /// nothing in aggregate and is not what the field means.  What it does give
    /// is a sound necessary condition — a market can never hold more atoms of a
    /// mint than that mint is admitted to have — so a cap above this value is
    /// refusable, while a cap at or below it is merely *not refuted*.
    ///
    /// The per-market cap value itself has no source in this policy, in the
    /// frozen `CreateMarket` intent, or in [`crate::TermsAccount`].  It needs a
    /// new immutable terms field or a new intent version; this function
    /// deliberately does not invent one.
    pub const fn market_cap_ceiling_atoms(&self) -> u64 {
        self.max_supply_atoms
    }

    /// Refuse a per-market collateral cap this policy could never back.
    ///
    /// A zero cap is admitted: it is the fail-closed "accepts no collateral"
    /// state a market is created in today, not a policy violation.
    pub const fn check_market_cap(&self, cap: u64) -> Result<()> {
        if cap > self.market_cap_ceiling_atoms() {
            Err(CodecError::InvalidCount)
        } else {
            Ok(())
        }
    }
}

const fn currency_eq(left: &CurrencyRef, right: &CurrencyRef) -> bool {
    left.kind.byte() == right.kind.byte()
        && bytes_eq(&left.token_program, &right.token_program)
        && bytes_eq(&left.mint, &right.mint)
        && left.decimals == right.decimals
}

/// The parent Realm Profile preimage: exactly [`PROFILE_PARENT_BYTES`] bytes.
///
/// The collateral-policy digest is **not** the Realm's Profile ID.  It is one
/// domain-separated subfield inside this parent, whose canonical bytes are
/// hashed by the already-frozen [`crate::canonical_profile_hash`] rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParentProfile {
    /// Parent schema version; only [`PARENT_PROFILE_SCHEMA`] is understood.
    pub schema_version: u16,
    /// Parent flags; V1 requires [`PARENT_PROFILE_FLAGS`].
    pub flags: u16,
    /// Subfield tag; only [`SUBFIELD_COLLATERAL_POLICY`] is understood.
    pub subfield_tag: u16,
    /// Subfield schema version, mirroring the child policy's schema.
    ///
    /// It lives inside the preimage on purpose: a future collateral schema moves
    /// the parent identity even if it happened to produce the same child digest.
    pub subfield_schema_version: u16,
    /// The child digest `D_col`.
    pub collateral_policy_digest: Hash32,
}

impl ParentProfile {
    /// Compose the parent over one decoded collateral policy.
    pub fn from_policy(policy: &CollateralPolicy) -> Result<Self> {
        Self::from_policy_digest(policy.digest()?, policy.schema_version)
    }

    /// Compose the parent over an already-computed child digest.
    pub fn from_policy_digest(
        collateral_policy_digest: Hash32,
        subfield_schema_version: u16,
    ) -> Result<Self> {
        let value = Self {
            schema_version: PARENT_PROFILE_SCHEMA,
            flags: PARENT_PROFILE_FLAGS,
            subfield_tag: SUBFIELD_COLLATERAL_POLICY,
            subfield_schema_version,
            collateral_policy_digest,
        };
        value.validate()?;
        Ok(value)
    }

    /// Refuse every parent this schema does not admit.
    pub const fn validate(&self) -> Result<()> {
        if self.schema_version != PARENT_PROFILE_SCHEMA {
            return Err(CodecError::WrongVersion);
        }
        if self.flags != PARENT_PROFILE_FLAGS {
            return Err(CodecError::InvalidEnum);
        }
        if self.subfield_tag != SUBFIELD_COLLATERAL_POLICY {
            return Err(CodecError::WrongTag);
        }
        if self.subfield_schema_version != COLLATERAL_POLICY_SCHEMA {
            return Err(CodecError::WrongVersion);
        }
        if is_zero(&self.collateral_policy_digest.0) {
            return Err(CodecError::ZeroIdentity);
        }
        Ok(())
    }

    /// Encode exactly [`PROFILE_PARENT_BYTES`] canonical bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < PROFILE_PARENT_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        w.bytes(&PARENT_PROFILE_MAGIC)?;
        w.u16(self.schema_version)?;
        w.u16(self.flags)?;
        w.u16(self.subfield_tag)?;
        w.u16(self.subfield_schema_version)?;
        w.hash(self.collateral_policy_digest)?;
        w.bytes(&[0; PARENT_PROFILE_RESERVED_BYTES])?;
        Ok(w.at)
    }

    /// The canonical byte image of this parent preimage.
    pub fn canonical_bytes(&self) -> Result<[u8; PROFILE_PARENT_BYTES]> {
        let mut out = [0; PROFILE_PARENT_BYTES];
        self.encode(&mut out)?;
        Ok(out)
    }

    /// Parse exactly [`PROFILE_PARENT_BYTES`] hostile bytes.
    ///
    /// A successful decode says the bytes are a well-formed V1 parent.  It says
    /// nothing about *which* collateral policy they commit to: a well-formed
    /// parent can carry another Realm's child digest and this function will
    /// accept it happily.  Use [`ParentProfile::binds`].
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() < PROFILE_PARENT_BYTES {
            return Err(CodecError::Truncated);
        }
        if input.len() > PROFILE_PARENT_BYTES {
            return Err(CodecError::TrailingBytes);
        }
        if input[..8] != PARENT_PROFILE_MAGIC {
            return Err(CodecError::WrongTag);
        }
        let mut i = PROFILE_PARENT_BYTES - PARENT_PROFILE_RESERVED_BYTES;
        while i < PROFILE_PARENT_BYTES {
            if input[i] != 0 {
                return Err(CodecError::NonCanonicalPadding);
            }
            i += 1;
        }
        let mut r = Reader::at(input, 8);
        let value = Self {
            schema_version: r.u16()?,
            flags: r.u16()?,
            subfield_tag: r.u16()?,
            subfield_schema_version: r.u16()?,
            collateral_policy_digest: r.hash()?,
        };
        value.validate()?;
        if value.canonical_bytes()?[..] != *input {
            return Err(CodecError::NonCanonicalPadding);
        }
        Ok(value)
    }

    /// The parent Profile ID: [`crate::canonical_profile_hash`] over these bytes.
    pub fn identity(&self) -> Result<ProfileHash> {
        canonical_profile_hash(&self.canonical_bytes()?)
    }

    /// Whether this parent commits to exactly `policy`.
    ///
    /// This is the check that decoding does not perform.
    pub fn binds(&self, policy: &CollateralPolicy) -> Result<bool> {
        Ok(self.subfield_tag == SUBFIELD_COLLATERAL_POLICY
            && self.subfield_schema_version == policy.schema_version
            && self.collateral_policy_digest == policy.digest()?)
    }
}

/// Decode policy bytes and return their child digest `D_col`.
///
/// Refuses exactly what [`CollateralPolicy::decode`] refuses; a digest is never
/// computed over bytes this build would not admit as a policy.
pub fn collateral_policy_digest(policy_bytes: &[u8]) -> Result<Hash32> {
    CollateralPolicy::decode(policy_bytes)?.digest()
}

/// Refuse unless `profile` is frozen to **exactly** this collateral policy.
///
/// This is §3.4 obligation 3, and the thing a decoder alone cannot do.  It:
///
/// 1. validates the Profile account, which already pins "frozen flag set exactly
///    when the digest is nonzero";
/// 2. refuses an *unfrozen* Profile outright, because a Realm that has not
///    committed to a collateral policy must not mint liabilities;
/// 3. decodes the 266 policy bytes with every refusal of the Python model; and
/// 4. **recomputes** `D_col` from those bytes and compares it against
///    [`crate::ProfileAccount::collateral_policy_digest`].
///
/// Step 4 is the load-bearing one.  Without it a well-formed policy and a
/// well-formed Profile can be paired freely, and an adapter that merely decoded
/// both would have checked nothing.
///
/// On success the decoded policy is returned so the caller reads checked values
/// rather than re-parsing.  A [`CodecError::MismatchedBinding`] means the Profile
/// commits to a *different* collateral policy — never a warning, always a refusal.
pub fn verify_collateral_binding(
    policy_bytes: &[u8],
    profile: &ProfileAccount,
) -> Result<CollateralPolicy> {
    profile.validate()?;
    if profile.flags & PROFILE_FLAG_POLICY_FROZEN == 0 {
        return Err(CodecError::ZeroIdentity);
    }
    let policy = CollateralPolicy::decode(policy_bytes)?;
    if policy.digest()? != profile.collateral_policy_digest {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(policy)
}

/// [`verify_collateral_binding`], and additionally that the account's Profile ID
/// is the canonical parent hash over that same digest.
///
/// [`crate::ProfileAccount::profile`], [`crate::RealmAccount::profile`], and
/// [`crate::MarketAccount::profile`] all hold the **parent** `ProfileHash`.  In
/// the V1 parent schema the preimage carries exactly one subfield, so the parent
/// identity is a total function of `D_col` and is fully recomputable here.  A
/// future parent schema that adds a second subfield breaks that determinism, and
/// this function must then move behind the new composition rather than be
/// relaxed — which is why the schema version is inside the preimage.
///
/// Returns [`CodecError::NonCanonicalIdentity`] when the stored Profile ID is not
/// the derivation of the policy the account is frozen to.
pub fn verify_profile_identity(
    policy_bytes: &[u8],
    profile: &ProfileAccount,
) -> Result<CollateralPolicy> {
    let policy = verify_collateral_binding(policy_bytes, profile)?;
    if ParentProfile::from_policy(&policy)?.identity()? != profile.profile {
        return Err(CodecError::NonCanonicalIdentity);
    }
    Ok(policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account_len;

    /* ---------------------------------------------------------------------
     * Cross-language fixtures.
     *
     * Every byte string and every digest below is transcribed verbatim from
     * `research/collateral-profiles/identity_vectors.json`, which the Python
     * tests recompute from `model.py` on every run.  They are not recomputed
     * here from Rust: a round trip through this crate's own encoder would
     * agree with itself even if the domain string or field order had drifted.
     * ------------------------------------------------------------------- */

    const GENERIC_POLICY_HEX: &str = concat!(
        "4443434f4c50310001001f000106ddf6e1ee758fde18425dbce46ccddab61afc",
        "4d83b90d27febdf928d8a18bfccdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
        "cdcdcdcdcdcdcdcdcdcdcdcdcd060106ddf6e1ee758fde18425dbce46ccddab6",
        "1afc4d83b90d27febdf928d8a18bfccdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
        "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcd0600000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000090080c6a47e8d0300000000000000",
        "0000000000000000000080000000000000000000000000000000000000000000",
        "00000000000000000000",
    );
    const GENERIC_CHILD_HEX: &str =
        "aafb22527b09935db83362d09eebb7cd875a7714fc9e3c3764a9e57c207c5c32";
    const GENERIC_PARENT_HEX: &str = concat!(
        "444350524f4631000100000001000100aafb22527b09935db83362d09eebb7cd",
        "875a7714fc9e3c3764a9e57c207c5c3200000000000000000000000000000000",
    );
    const GENERIC_IDENTITY_HEX: &str =
        "8180f42830d90ef060ec2e4d91c6c19145db9cd9e2dbfd759045770930831688";

    const DREGG_POLICY_HEX: &str = concat!(
        "4443434f4c50310001001f000106ddf6e1d765a193d9cbe146ceeb79ac1cb485",
        "ed5f5b37913a8cf5857eff00a907e0c65663f8a2651cd249df49342c8dd5ff9d",
        "946f1b212f3dc484980af7980f060106ddf6e1d765a193d9cbe146ceeb79ac1c",
        "b485ed5f5b37913a8cf5857eff00a907e0c65663f8a2651cd249df49342c8dd5",
        "ff9d946f1b212f3dc484980af7980f0600000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000090080c6a47e8d0300000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "00000000000000000000",
    );
    const DREGG_CHILD_HEX: &str =
        "ef63ccd0c5e1616c1570dd96a985ef9924f622d44c246f5aa88e1b9545f54343";
    const DREGG_PARENT_HEX: &str = concat!(
        "444350524f4631000100000001000100ef63ccd0c5e1616c1570dd96a985ef99",
        "24f622d44c246f5aa88e1b9545f5434300000000000000000000000000000000",
    );
    const DREGG_IDENTITY_HEX: &str =
        "31cd82668ac7846bbf6bf38d25107d0301bc468d40816bf9a565ac93766f93b3";

    const LEGACY_SOL_FEE_POLICY_HEX: &str = concat!(
        "4443434f4c50310001001f000106ddf6e1d765a193d9cbe146ceeb79ac1cb485",
        "ed5f5b37913a8cf5857eff00a9ababababababababababababababababababab",
        "ababababababababababababab09000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000900000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "000000000000000000000000000000000009ffffffffffffffff000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "00000000000000000000",
    );
    const LEGACY_SOL_FEE_CHILD_HEX: &str =
        "e7c9503bb8a5fe6db1a40b8f94868e1c6a55826232d897e2883cd641d0bb21e3";
    const LEGACY_SOL_FEE_PARENT_HEX: &str = concat!(
        "444350524f4631000100000001000100e7c9503bb8a5fe6db1a40b8f94868e1c",
        "6a55826232d897e2883cd641d0bb21e300000000000000000000000000000000",
    );
    const LEGACY_SOL_FEE_IDENTITY_HEX: &str =
        "f2ea9b4747076c06c1adb6b5ce3bb5fbecdeacd2b7f03d6c131cc10b0ce85db6";

    fn nibble(c: u8) -> u8 {
        match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            _ => panic!("fixture is not lowercase hex"),
        }
    }

    fn unhex<const N: usize>(text: &str) -> [u8; N] {
        let raw = text.as_bytes();
        assert_eq!(raw.len(), N * 2, "fixture length");
        let mut out = [0; N];
        let mut i = 0;
        while i < N {
            out[i] = (nibble(raw[2 * i]) << 4) | nibble(raw[2 * i + 1]);
            i += 1;
        }
        out
    }

    fn policy_bytes(hex: &str) -> [u8; COLLATERAL_POLICY_BYTES] {
        unhex::<COLLATERAL_POLICY_BYTES>(hex)
    }

    fn hash(hex: &str) -> Hash32 {
        Hash32::from_bytes(unhex::<HASH_BYTES>(hex))
    }

    fn generic() -> [u8; COLLATERAL_POLICY_BYTES] {
        policy_bytes(GENERIC_POLICY_HEX)
    }

    /// A Profile account frozen to one policy, with the derived parent ID.
    fn frozen_profile(policy: &[u8; COLLATERAL_POLICY_BYTES]) -> ProfileAccount {
        let decoded = CollateralPolicy::decode(policy).expect("golden policy decodes");
        let child = decoded.digest().expect("child digest");
        ProfileAccount {
            profile: ParentProfile::from_policy(&decoded)
                .expect("parent composes")
                .identity()
                .expect("parent identity"),
            realm: Hash32::from_bytes([9; HASH_BYTES]),
            collateral_policy_digest: child,
            version: crate::account_version::PROFILE,
            flags: PROFILE_FLAG_POLICY_FROZEN,
        }
    }

    /// Mutate one byte of the generic golden policy.
    fn mutated(index: usize, value: u8) -> [u8; COLLATERAL_POLICY_BYTES] {
        let mut raw = generic();
        raw[index] = value;
        raw
    }

    /// Overwrite one little-endian `u64` of the generic golden policy.
    fn with_u64(offset: usize, value: u64) -> [u8; COLLATERAL_POLICY_BYTES] {
        let mut raw = generic();
        raw[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        raw
    }

    const OFF_SCHEMA: usize = 8;
    const OFF_FLAGS: usize = 10;
    const OFF_COLLATERAL: usize = 12;
    const OFF_FEE: usize = 78;
    const OFF_LIVENESS: usize = 144;
    const OFF_MAX_SUPPLY: usize = 210;
    const OFF_ALLOWED_MINT: usize = 218;
    const OFF_REQUIRED_MINT: usize = 226;
    const OFF_ALLOWED_ACCOUNT: usize = 234;
    const OFF_REQUIRED_ACCOUNT: usize = 242;
    const OFF_RESERVED: usize = 250;

    #[test]
    fn the_three_golden_policies_decode_to_their_frozen_child_digests() {
        for (hex, child) in [
            (GENERIC_POLICY_HEX, GENERIC_CHILD_HEX),
            (DREGG_POLICY_HEX, DREGG_CHILD_HEX),
            (LEGACY_SOL_FEE_POLICY_HEX, LEGACY_SOL_FEE_CHILD_HEX),
        ] {
            let raw = policy_bytes(hex);
            let policy = CollateralPolicy::decode(&raw).expect("golden decodes");
            assert_eq!(policy.canonical_bytes(), Ok(raw), "re-encode is exact");
            assert_eq!(policy.digest(), Ok(hash(child)), "child digest agrees");
            assert_eq!(collateral_policy_digest(&raw), Ok(hash(child)));
        }
    }

    #[test]
    fn the_golden_policies_carry_the_field_values_the_offsets_table_says() {
        let generic = CollateralPolicy::decode(&generic()).unwrap();
        assert_eq!(generic.schema_version, COLLATERAL_POLICY_SCHEMA);
        assert_eq!(generic.flags, COLLATERAL_POLICY_STRICT_FLAGS);
        assert_eq!(generic.collateral.token_program, TOKEN_2022_PROGRAM);
        assert_eq!(generic.collateral.decimals, 6);
        // Fee and collateral share a mint; they remain separate accounting roles.
        assert_eq!(generic.fee, generic.collateral);
        assert_eq!(generic.liveness, CurrencyRef::NATIVE_SOL);
        assert_eq!(generic.max_supply_atoms, 1_000_000_000_000_000);
        assert_eq!(
            generic.allowed_account_extensions,
            EXTENSION_IMMUTABLE_OWNER
        );
        assert_eq!(generic.allowed_mint_extensions, 0);

        let legacy = CollateralPolicy::decode(&policy_bytes(LEGACY_SOL_FEE_POLICY_HEX)).unwrap();
        assert!(legacy.collateral.is_legacy_token());
        assert_eq!(legacy.fee, CurrencyRef::NATIVE_SOL);
        assert_eq!(legacy.max_supply_atoms, u64::MAX);
        // A legacy profile may declare no extension of any kind.
        assert_eq!(legacy.allowed_account_extensions, 0);

        let dregg = CollateralPolicy::decode(&policy_bytes(DREGG_POLICY_HEX)).unwrap();
        assert!(dregg.collateral.is_legacy_token());
        assert_eq!(dregg.fee, dregg.collateral);
    }

    #[test]
    fn policy_length_magic_and_reserved_bytes_fail_closed() {
        let raw = generic();
        assert_eq!(
            CollateralPolicy::decode(&raw[..COLLATERAL_POLICY_BYTES - 1]),
            Err(CodecError::Truncated)
        );
        assert_eq!(CollateralPolicy::decode(&[]), Err(CodecError::Truncated));
        let mut extended = [0; COLLATERAL_POLICY_BYTES + 1];
        extended[..COLLATERAL_POLICY_BYTES].copy_from_slice(&raw);
        assert_eq!(
            CollateralPolicy::decode(&extended),
            Err(CodecError::TrailingBytes)
        );
        for (index, byte) in raw.iter().enumerate().take(8) {
            assert_eq!(
                CollateralPolicy::decode(&mutated(index, byte ^ 0x01)),
                Err(CodecError::WrongTag),
                "magic byte {index}"
            );
        }
        for index in OFF_RESERVED..COLLATERAL_POLICY_BYTES {
            assert_eq!(
                CollateralPolicy::decode(&mutated(index, 1)),
                Err(CodecError::NonCanonicalPadding),
                "reserved byte {index}"
            );
        }
    }

    #[test]
    fn an_unknown_policy_schema_never_inherits_v1_semantics() {
        assert_eq!(
            CollateralPolicy::decode(&mutated(OFF_SCHEMA, 2)),
            Err(CodecError::WrongVersion)
        );
        assert_eq!(
            CollateralPolicy::decode(&mutated(OFF_SCHEMA, 0)),
            Err(CodecError::WrongVersion)
        );
    }

    #[test]
    fn a_realm_can_neither_invent_nor_weaken_the_strict_flag_word() {
        // Unknown bit positions fail closed.
        assert_eq!(
            CollateralPolicy::decode(&mutated(OFF_FLAGS, 0x3f)),
            Err(CodecError::InvalidEnum)
        );
        assert_eq!(
            CollateralPolicy::decode(&mutated(OFF_FLAGS + 1, 0x80)),
            Err(CodecError::InvalidEnum)
        );
        // Every known-but-weakened combination fails closed too.
        for dropped in [
            FLAG_REQUIRE_MINT_AUTHORITY_NONE,
            FLAG_REQUIRE_FREEZE_AUTHORITY_NONE,
            FLAG_REQUIRE_NONZERO_SUPPLY,
            FLAG_REQUIRE_ACCOUNT_DELEGATE_NONE,
            FLAG_REQUIRE_ACCOUNT_CLOSE_AUTHORITY_NONE,
        ] {
            let weakened = COLLATERAL_POLICY_STRICT_FLAGS & !dropped;
            assert_eq!(
                CollateralPolicy::decode(&mutated(OFF_FLAGS, weakened as u8)),
                Err(CodecError::InvalidEnum),
                "weakened flags {weakened:#x}"
            );
        }
        assert_eq!(
            CollateralPolicy::decode(&mutated(OFF_FLAGS, 0)),
            Err(CodecError::InvalidEnum)
        );
    }

    #[test]
    fn every_currency_role_refuses_unknown_kinds_and_noncanonical_slots() {
        for role in [OFF_COLLATERAL, OFF_FEE, OFF_LIVENESS] {
            assert_eq!(
                CollateralPolicy::decode(&mutated(role, 2)),
                Err(CodecError::InvalidEnum),
                "unknown kind at {role}"
            );
            assert_eq!(
                CollateralPolicy::decode(&mutated(role, u8::MAX)),
                Err(CodecError::InvalidEnum),
                "unknown kind at {role}"
            );
        }
        // Native SOL must carry zero program and mint slots and nine decimals.
        assert_eq!(
            CollateralPolicy::decode(&mutated(OFF_LIVENESS + 1, 1)),
            Err(CodecError::NonCanonicalPadding)
        );
        assert_eq!(
            CollateralPolicy::decode(&mutated(OFF_LIVENESS + 33, 1)),
            Err(CodecError::NonCanonicalPadding)
        );
        assert_eq!(
            CollateralPolicy::decode(&mutated(OFF_LIVENESS + 65, 6)),
            Err(CodecError::InvalidCount)
        );
    }

    #[test]
    fn an_spl_currency_needs_a_nonzero_mint_on_a_pinned_program() {
        let mut zero_program = generic();
        zero_program[OFF_COLLATERAL + 1..OFF_COLLATERAL + 33].copy_from_slice(&[0; HASH_BYTES]);
        assert_eq!(
            CollateralPolicy::decode(&zero_program),
            Err(CodecError::ZeroIdentity)
        );
        let mut zero_mint = generic();
        zero_mint[OFF_COLLATERAL + 33..OFF_COLLATERAL + 65].copy_from_slice(&[0; HASH_BYTES]);
        assert_eq!(
            CollateralPolicy::decode(&zero_mint),
            Err(CodecError::ZeroIdentity)
        );
        // A third program, however plausible, is not one of the two pinned ones.
        let mut foreign = generic();
        foreign[OFF_COLLATERAL + 1..OFF_COLLATERAL + 33].copy_from_slice(&[0x77; HASH_BYTES]);
        assert_eq!(
            CollateralPolicy::decode(&foreign),
            Err(CodecError::InvalidEnum)
        );
    }

    #[test]
    fn v1_collateral_must_be_an_spl_token_with_a_positive_ceiling() {
        // Native SOL collateral has no mint to admit and no Hoard account.
        let mut native_collateral = generic();
        native_collateral[OFF_COLLATERAL] = CurrencyKind::NativeSol.byte();
        native_collateral[OFF_COLLATERAL + 1..OFF_COLLATERAL + 65]
            .copy_from_slice(&[0; 2 * HASH_BYTES]);
        native_collateral[OFF_COLLATERAL + 65] = NATIVE_SOL_DECIMALS;
        // The fee role still mirrors the old SPL currency, so this is refused
        // for being native collateral before any fee-role check runs.
        assert_eq!(
            CollateralPolicy::decode(&native_collateral),
            Err(CodecError::InvalidEnum)
        );
        assert_eq!(
            CollateralPolicy::decode(&with_u64(OFF_MAX_SUPPLY, 0)),
            Err(CodecError::ZeroValue)
        );
    }

    #[test]
    fn the_fee_and_liveness_roles_are_not_free_choices() {
        // A separately tokenized fee asset needs its own admission policy.
        let mut foreign_fee = generic();
        foreign_fee[OFF_FEE + 33..OFF_FEE + 65].copy_from_slice(&[0xef; HASH_BYTES]);
        assert_eq!(
            CollateralPolicy::decode(&foreign_fee),
            Err(CodecError::MismatchedBinding)
        );
        // Even the same mint at different decimals is a different currency.
        assert_eq!(
            CollateralPolicy::decode(&mutated(OFF_FEE + 65, 7)),
            Err(CodecError::MismatchedBinding)
        );
        // Liveness must be native SOL: future fees never capitalize liveness.
        let mut token_liveness = generic();
        token_liveness[OFF_LIVENESS..OFF_LIVENESS + CURRENCY_REF_BYTES]
            .copy_from_slice(&generic()[OFF_COLLATERAL..OFF_COLLATERAL + CURRENCY_REF_BYTES]);
        assert_eq!(
            CollateralPolicy::decode(&token_liveness),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn extension_bitsets_fail_closed_on_unknown_bits_and_on_expansion() {
        for offset in [
            OFF_ALLOWED_MINT,
            OFF_REQUIRED_MINT,
            OFF_ALLOWED_ACCOUNT,
            OFF_REQUIRED_ACCOUNT,
        ] {
            // The first unpinned discriminant, and the top of the word.
            for bit in [EXTENSION_DISCRIMINANTS, 63] {
                assert_eq!(
                    CollateralPolicy::decode(&with_u64(offset, 1 << bit)),
                    Err(CodecError::InvalidEnum),
                    "unknown bit {bit} at {offset}"
                );
            }
        }
        // A Realm may narrow the ceiling but never expand it.
        assert_eq!(
            CollateralPolicy::decode(&with_u64(OFF_ALLOWED_MINT, EXTENSION_IMMUTABLE_OWNER)),
            Err(CodecError::InvalidEnum)
        );
        assert_eq!(
            CollateralPolicy::decode(&with_u64(OFF_ALLOWED_ACCOUNT, 1 << 11)),
            Err(CodecError::InvalidEnum)
        );
        // Narrowing is admitted: dropping ImmutableOwner is a legal policy.
        let narrowed = with_u64(OFF_ALLOWED_ACCOUNT, 0);
        assert!(CollateralPolicy::decode(&narrowed).is_ok());
        // Required must also be allowed.
        assert_eq!(
            CollateralPolicy::decode(&with_u64(OFF_REQUIRED_ACCOUNT, EXTENSION_IMMUTABLE_OWNER))
                .map(|p| p.required_account_extensions),
            Ok(EXTENSION_IMMUTABLE_OWNER)
        );
        let mut required_not_allowed = with_u64(OFF_ALLOWED_ACCOUNT, 0);
        required_not_allowed[OFF_REQUIRED_ACCOUNT..OFF_REQUIRED_ACCOUNT + 8]
            .copy_from_slice(&EXTENSION_IMMUTABLE_OWNER.to_le_bytes());
        assert_eq!(
            CollateralPolicy::decode(&required_not_allowed),
            Err(CodecError::InvalidEnum)
        );
        assert_eq!(
            CollateralPolicy::decode(&with_u64(OFF_REQUIRED_MINT, 1)),
            Err(CodecError::InvalidEnum)
        );
    }

    #[test]
    fn a_legacy_spl_profile_cannot_declare_token_2022_extensions() {
        let legacy = policy_bytes(DREGG_POLICY_HEX);
        for offset in [
            OFF_ALLOWED_MINT,
            OFF_REQUIRED_MINT,
            OFF_ALLOWED_ACCOUNT,
            OFF_REQUIRED_ACCOUNT,
        ] {
            let mut raw = legacy;
            raw[offset..offset + 8].copy_from_slice(&EXTENSION_IMMUTABLE_OWNER.to_le_bytes());
            assert_eq!(
                CollateralPolicy::decode(&raw),
                Err(CodecError::InvalidEnum),
                "legacy extension claim at {offset}"
            );
        }
    }

    #[test]
    fn the_parent_preimage_and_identity_match_the_python_goldens() {
        for (policy_hex, parent_hex, identity_hex) in [
            (GENERIC_POLICY_HEX, GENERIC_PARENT_HEX, GENERIC_IDENTITY_HEX),
            (DREGG_POLICY_HEX, DREGG_PARENT_HEX, DREGG_IDENTITY_HEX),
            (
                LEGACY_SOL_FEE_POLICY_HEX,
                LEGACY_SOL_FEE_PARENT_HEX,
                LEGACY_SOL_FEE_IDENTITY_HEX,
            ),
        ] {
            let policy = CollateralPolicy::decode(&policy_bytes(policy_hex)).unwrap();
            let parent = ParentProfile::from_policy(&policy).expect("parent composes");
            assert_eq!(
                parent.canonical_bytes(),
                Ok(unhex::<PROFILE_PARENT_BYTES>(parent_hex)),
                "parent preimage bytes"
            );
            assert_eq!(parent.identity(), Ok(hash(identity_hex)), "parent identity");
            assert_eq!(
                ParentProfile::decode(&parent.canonical_bytes().unwrap()),
                Ok(parent),
                "parent round trip"
            );
            assert!(parent.binds(&policy).unwrap());
        }
        // The child digest is not the Profile ID, in either direction.
        assert_ne!(hash(GENERIC_CHILD_HEX), hash(GENERIC_IDENTITY_HEX));
    }

    #[test]
    fn the_nine_parent_decode_refusal_vectors_all_fail_closed() {
        for (name, hex, expected) in [
            (
                "wrong-magic",
                "454350524f4631000100000001000100aafb22527b09935db83362d09eebb7cd875a7714fc9e3c3764a9e57c207c5c3200000000000000000000000000000000",
                CodecError::WrongTag,
            ),
            (
                "nonzero-reserved",
                "444350524f4631000100000001000100aafb22527b09935db83362d09eebb7cd875a7714fc9e3c3764a9e57c207c5c3200000000000000000000000000000001",
                CodecError::NonCanonicalPadding,
            ),
            (
                "swapped-flags-and-subfield-tag",
                "444350524f4631000100010000000100aafb22527b09935db83362d09eebb7cd875a7714fc9e3c3764a9e57c207c5c3200000000000000000000000000000000",
                CodecError::InvalidEnum,
            ),
            (
                "unknown-subfield-tag",
                "444350524f4631000100000002000100aafb22527b09935db83362d09eebb7cd875a7714fc9e3c3764a9e57c207c5c3200000000000000000000000000000000",
                CodecError::WrongTag,
            ),
            (
                "unsupported-parent-schema",
                "444350524f4631000200000001000100aafb22527b09935db83362d09eebb7cd875a7714fc9e3c3764a9e57c207c5c3200000000000000000000000000000000",
                CodecError::WrongVersion,
            ),
            (
                "unsupported-subfield-schema",
                "444350524f4631000100000001000200aafb22527b09935db83362d09eebb7cd875a7714fc9e3c3764a9e57c207c5c3200000000000000000000000000000000",
                CodecError::WrongVersion,
            ),
            (
                "zero-child-digest",
                "444350524f4631000100000001000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                CodecError::ZeroIdentity,
            ),
        ] {
            let raw = unhex::<PROFILE_PARENT_BYTES>(hex);
            assert_eq!(ParentProfile::decode(&raw), Err(expected), "{name}");
        }
        // truncated: the golden vector with its last byte removed.
        let full = unhex::<PROFILE_PARENT_BYTES>(GENERIC_PARENT_HEX);
        assert_eq!(
            ParentProfile::decode(&full[..PROFILE_PARENT_BYTES - 1]),
            Err(CodecError::Truncated)
        );
        // extended: one zero byte appended.
        let mut extended = [0; PROFILE_PARENT_BYTES + 1];
        extended[..PROFILE_PARENT_BYTES].copy_from_slice(&full);
        assert_eq!(
            ParentProfile::decode(&extended),
            Err(CodecError::TrailingBytes)
        );
    }

    #[test]
    fn the_three_binding_refusal_vectors_decode_but_do_not_bind() {
        let policy = CollateralPolicy::decode(&generic()).unwrap();
        for (name, hex) in [
            (
                "child-digest-bit-flipped",
                "444350524f4631000100000001000100abfb22527b09935db83362d09eebb7cd875a7714fc9e3c3764a9e57c207c5c3200000000000000000000000000000000",
            ),
            (
                "swapped-child-digest-halves",
                "444350524f4631000100000001000100875a7714fc9e3c3764a9e57c207c5c32aafb22527b09935db83362d09eebb7cd00000000000000000000000000000000",
            ),
            (
                "foreign-collateral-policy",
                "444350524f4631000100000001000100ef63ccd0c5e1616c1570dd96a985ef9924f622d44c246f5aa88e1b9545f5434300000000000000000000000000000000",
            ),
        ] {
            let raw = unhex::<PROFILE_PARENT_BYTES>(hex);
            let parent = ParentProfile::decode(&raw)
                .unwrap_or_else(|_| panic!("{name} is well-formed and must decode"));
            assert!(!parent.binds(&policy).unwrap(), "{name} must not bind");
        }
        // The foreign parent is a *real* Realm's parent, not a corrupted one:
        // it binds the DREGG policy exactly. Well-formedness is not evidence.
        let dregg = CollateralPolicy::decode(&policy_bytes(DREGG_POLICY_HEX)).unwrap();
        let foreign = ParentProfile::decode(&unhex::<PROFILE_PARENT_BYTES>(
            "444350524f4631000100000001000100ef63ccd0c5e1616c1570dd96a985ef9924f622d44c246f5aa88e1b9545f5434300000000000000000000000000000000",
        ))
        .unwrap();
        assert!(foreign.binds(&dregg).unwrap());
    }

    #[test]
    fn the_four_domain_separation_confusions_stay_distinct() {
        let policy = generic();
        let parent = unhex::<PROFILE_PARENT_BYTES>(GENERIC_PARENT_HEX);
        let child = hash(GENERIC_CHILD_HEX);
        let identity = hash(GENERIC_IDENTITY_HEX);

        let confusions = [
            (
                "child-domain-over-parent-bytes",
                crate::digest(COLLATERAL_POLICY_DOMAIN, &[&parent]),
                "75d471c2945e737f0100c3a29b21f4d6e138f60b1c609a3580f8497c080feaa6",
            ),
            (
                "parent-domain-over-child-bytes",
                crate::digest(PARENT_PROFILE_DOMAIN, &[&policy]),
                "34afe8672d0bdf856a51090df404ee9c8e0675847820382c4295b71c6e331a12",
            ),
            (
                "undomained-parent-bytes",
                crate::digest(b"", &[&parent]),
                "21756320957c8ed9da2951b286dfba5325b2fd03b8aad2eb5d8a4a31697d44b8",
            ),
            (
                "parent-domain-with-separator-byte",
                crate::digest(PARENT_PROFILE_DOMAIN, &[&[0], &parent]),
                "a955ea401c5b89e31dd740794dfd9b252298e7728f184fc3d338e7aa98542387",
            ),
        ];
        for (name, computed, expected) in confusions {
            assert_eq!(computed, hash(expected), "{name}");
            assert_ne!(computed, child, "{name} vs child digest");
            assert_ne!(computed, identity, "{name} vs parent identity");
        }
        let mut rest = &confusions[..];
        while let Some((first, tail)) = rest.split_first() {
            for other in tail {
                assert_ne!(first.1, other.1, "confusion digests collide");
            }
            rest = tail;
        }
    }

    #[test]
    fn verify_refuses_a_profile_frozen_to_someone_elses_policy() {
        let generic_bytes = generic();
        let dregg_bytes = policy_bytes(DREGG_POLICY_HEX);
        let profile = frozen_profile(&generic_bytes);

        assert!(verify_collateral_binding(&generic_bytes, &profile).is_ok());
        assert!(verify_profile_identity(&generic_bytes, &profile).is_ok());

        // The load-bearing negative: the *other* Realm's policy is perfectly
        // well-formed and decodes without complaint, and is still refused.
        assert!(CollateralPolicy::decode(&dregg_bytes).is_ok());
        assert_eq!(
            verify_collateral_binding(&dregg_bytes, &profile),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            verify_profile_identity(&dregg_bytes, &profile),
            Err(CodecError::MismatchedBinding)
        );

        // One flipped bit in the stored digest is refused the same way.
        let mut flipped = profile;
        let mut digest_bytes = profile.collateral_policy_digest.bytes();
        digest_bytes[0] ^= 1;
        flipped.collateral_policy_digest = Hash32::from_bytes(digest_bytes);
        assert_eq!(
            verify_collateral_binding(&generic_bytes, &flipped),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn verify_refuses_an_unfrozen_profile_and_a_wrong_profile_id() {
        let generic_bytes = generic();
        let mut unfrozen = frozen_profile(&generic_bytes);
        unfrozen.flags = 0;
        unfrozen.collateral_policy_digest = Hash32::ZERO;
        assert_eq!(
            verify_collateral_binding(&generic_bytes, &unfrozen),
            Err(CodecError::ZeroIdentity)
        );

        // The subfield digest can be right while the stored Profile ID is not
        // the parent derivation over it; only the identity check notices.
        let mut wrong_id = frozen_profile(&generic_bytes);
        wrong_id.profile = Hash32::from_bytes([0x5a; HASH_BYTES]);
        assert!(verify_collateral_binding(&generic_bytes, &wrong_id).is_ok());
        assert_eq!(
            verify_profile_identity(&generic_bytes, &wrong_id),
            Err(CodecError::NonCanonicalIdentity)
        );

        // A Profile ID derived from the *other* Realm's policy is refused too.
        let dregg = CollateralPolicy::decode(&policy_bytes(DREGG_POLICY_HEX)).unwrap();
        let mut foreign_id = frozen_profile(&generic_bytes);
        foreign_id.profile = ParentProfile::from_policy(&dregg)
            .unwrap()
            .identity()
            .unwrap();
        assert_eq!(
            verify_profile_identity(&generic_bytes, &foreign_id),
            Err(CodecError::NonCanonicalIdentity)
        );
    }

    #[test]
    fn verify_passes_hostile_policy_bytes_through_the_full_decoder() {
        let profile = frozen_profile(&generic());
        assert_eq!(
            verify_collateral_binding(&[], &profile),
            Err(CodecError::Truncated)
        );
        assert_eq!(
            verify_collateral_binding(&mutated(0, b'X'), &profile),
            Err(CodecError::WrongTag)
        );
        assert_eq!(
            verify_collateral_binding(&mutated(OFF_FLAGS, 0), &profile),
            Err(CodecError::InvalidEnum)
        );
    }

    #[test]
    fn a_frozen_profile_account_round_trips_with_its_derived_identity() {
        let profile = frozen_profile(&generic());
        let mut encoded = [0; account_len::PROFILE];
        assert_eq!(profile.encode(&mut encoded), Ok(account_len::PROFILE));
        assert_eq!(ProfileAccount::decode(&encoded), Ok(profile));
        // The 32 bytes at offset 66 are the child digest, not the identity.
        assert_eq!(encoded[66..98], hash(GENERIC_CHILD_HEX).bytes()[..]);
        assert_eq!(encoded[2..34], hash(GENERIC_IDENTITY_HEX).bytes()[..]);
    }

    #[test]
    fn the_supply_ceiling_bounds_a_market_cap_but_does_not_supply_one() {
        let policy = CollateralPolicy::decode(&generic()).unwrap();
        assert_eq!(policy.market_cap_ceiling_atoms(), policy.max_supply_atoms);
        assert_eq!(policy.check_market_cap(0), Ok(()));
        assert_eq!(policy.check_market_cap(policy.max_supply_atoms), Ok(()));
        assert_eq!(
            policy.check_market_cap(policy.max_supply_atoms + 1),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            policy.check_market_cap(u64::MAX),
            Err(CodecError::InvalidCount)
        );
        // A policy whose ceiling is the whole word refutes nothing at all,
        // which is precisely why the ceiling is not a cap.
        let unlimited = CollateralPolicy::decode(&policy_bytes(LEGACY_SOL_FEE_POLICY_HEX)).unwrap();
        assert_eq!(unlimited.check_market_cap(u64::MAX), Ok(()));
    }

    #[test]
    fn constructed_policies_and_parents_refuse_the_same_things_decoding_does() {
        let mut policy = CollateralPolicy::decode(&generic()).unwrap();
        policy.flags = 0;
        assert_eq!(policy.validate(), Err(CodecError::InvalidEnum));
        assert_eq!(
            policy.encode(&mut [0; COLLATERAL_POLICY_BYTES]),
            Err(CodecError::InvalidEnum)
        );
        assert_eq!(policy.digest(), Err(CodecError::InvalidEnum));

        let good = CollateralPolicy::decode(&generic()).unwrap();
        assert_eq!(
            good.encode(&mut [0; COLLATERAL_POLICY_BYTES - 1]),
            Err(CodecError::OutputTooSmall)
        );
        let parent = ParentProfile::from_policy(&good).unwrap();
        assert_eq!(
            parent.encode(&mut [0; PROFILE_PARENT_BYTES - 1]),
            Err(CodecError::OutputTooSmall)
        );
        assert_eq!(
            ParentProfile::from_policy_digest(Hash32::ZERO, COLLATERAL_POLICY_SCHEMA),
            Err(CodecError::ZeroIdentity)
        );
        assert_eq!(
            ParentProfile::from_policy_digest(hash(GENERIC_CHILD_HEX), 2),
            Err(CodecError::WrongVersion)
        );
    }
}
