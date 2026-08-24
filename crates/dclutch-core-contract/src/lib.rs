#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Hostile-decodable persistent contract for the compact dClutch Market root.
//!
//! This crate owns only immutable Market identity, optional capability
//! admission, root lifecycle, direct-child accounting, and their canonical
//! byte encodings. It contains no Solana types, account derivation, hashing
//! policy, venue semantics, source adapter, mint, collateral, or funding
//! layout.

use core::convert::TryInto;

/// Byte width of one opaque content identity.
pub const CONTENT_ID_BYTES: usize = 32;

/// Canonical byte width of [`MarketIdentity`].
pub const MARKET_IDENTITY_BYTES: usize = 137;

/// Canonical byte width of [`MarketRoot`].
pub const MARKET_ROOT_BYTES: usize = 168;

/// Byte width of the versioned Market root header.
pub const MARKET_ROOT_HEADER_BYTES: usize = 16;

/// Magic bytes at the start of every canonical Market root.
pub const MARKET_ROOT_MAGIC: [u8; 8] = *b"DCLTROOT";

/// Current canonical Market root schema version.
pub const MARKET_ROOT_SCHEMA_VERSION: u16 = 1;

const REALM_OFFSET: usize = 0;
const TERMS_OFFSET: usize = 32;
const CLAIM_BASIS_OFFSET: usize = 64;
const RESOLUTION_POLICY_OFFSET: usize = 96;
const GENERATION_OFFSET: usize = 128;
const CAPABILITIES_OFFSET: usize = 136;

const ROOT_MAGIC_OFFSET: usize = 0;
const ROOT_SCHEMA_OFFSET: usize = 8;
const ROOT_HEADER_RESERVED_OFFSET: usize = 10;
const ROOT_HEADER_RESERVED_BYTES: usize = 6;
const ROOT_IDENTITY_OFFSET: usize = MARKET_ROOT_HEADER_BYTES;
const ROOT_PHASE_OFFSET: usize = 153;
const ROOT_BODY_RESERVED_OFFSET: usize = 154;
const ROOT_BODY_RESERVED_BYTES: usize = 6;
const ROOT_CHILD_COUNT_OFFSET: usize = 160;

const KNOWN_CAPABILITY_BITS: u8 = 0b0011_1111;

/// Refusal returned while decoding or changing the persistent root contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A byte slice did not have the one exact canonical width.
    InvalidLength,
    /// A content identity was the reserved all-zero value.
    ZeroContentId,
    /// A capability bit outside the defined optional set was present.
    UnknownCapabilityBits,
    /// Root magic did not identify this account contract.
    InvalidMagic,
    /// The root schema version is not implemented by this crate.
    UnsupportedSchema,
    /// Reserved bytes were not all zero.
    NonCanonicalReservedBytes,
    /// A phase byte was outside the defined canonical values.
    UnknownPhase,
    /// The requested lifecycle edge is not admitted.
    InvalidPhaseTransition,
    /// The supplied generation did not match this immutable Market identity.
    GenerationMismatch,
    /// The supplied prior child count did not match persistent state.
    ChildCountMismatch,
    /// Incrementing the exact child count would overflow `u64`.
    ChildCountOverflow,
    /// Decrementing the exact child count would underflow zero.
    ChildCountUnderflow,
    /// A terminal transition was attempted while direct children remain.
    OutstandingChildren,
    /// A new direct child was attempted after retirement began.
    ChildCreationClosed,
}

/// Result alias for the persistent contract.
pub type Result<T> = core::result::Result<T, Error>;

/// An opaque, nonzero content identity supplied by a higher semantic layer.
///
/// This type deliberately assigns no hashing, derivation, or authority policy
/// to its bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct ContentId([u8; CONTENT_ID_BYTES]);

impl ContentId {
    /// Validate and construct an opaque content identity.
    pub fn new(bytes: [u8; CONTENT_ID_BYTES]) -> Result<Self> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(Error::ZeroContentId);
        }
        Ok(Self(bytes))
    }

    /// Decode one exact-width opaque content identity.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CONTENT_ID_BYTES {
            return Err(Error::InvalidLength);
        }
        Self::new(read_array::<CONTENT_ID_BYTES>(bytes, 0)?)
    }

    /// Return the exact opaque bytes.
    pub const fn to_bytes(self) -> [u8; CONTENT_ID_BYTES] {
        self.0
    }

    /// Borrow the exact opaque bytes.
    pub const fn as_bytes(&self) -> &[u8; CONTENT_ID_BYTES] {
        &self.0
    }
}

/// One optional facility admitted immutably by a Market.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Capability {
    /// Direct signed-intent execution.
    Direct,
    /// General frequent-batch portfolio execution.
    General,
    /// Covered Dealer liquidity.
    Dealer,
    /// Bearer/native claim materialization.
    BearerMaterialization,
    /// Fractional wrappers.
    Fractional,
    /// Structured wrappers.
    Structured,
}

impl Capability {
    const fn bit(self) -> u8 {
        match self {
            Self::Direct => 1 << 0,
            Self::General => 1 << 1,
            Self::Dealer => 1 << 2,
            Self::BearerMaterialization => 1 << 3,
            Self::Fractional => 1 << 4,
            Self::Structured => 1 << 5,
        }
    }
}

/// Canonical immutable admission set for optional Market facilities.
///
/// Resolution is universal Market semantics named by
/// [`MarketIdentity::resolution_policy_id`], so it is intentionally not an
/// optional capability bit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct CapabilitySet(u8);

impl CapabilitySet {
    /// No optional facilities admitted.
    pub const NONE: Self = Self(0);

    /// Decode the one-byte canonical bitset, refusing unknown bits 6 and 7.
    pub const fn from_bits(bits: u8) -> Result<Self> {
        if bits & !KNOWN_CAPABILITY_BITS != 0 {
            return Err(Error::UnknownCapabilityBits);
        }
        Ok(Self(bits))
    }

    /// Return the canonical one-byte representation.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Return whether one optional facility is admitted.
    pub const fn contains(self, capability: Capability) -> bool {
        self.0 & capability.bit() != 0
    }

    /// Return a new immutable set with one optional facility admitted.
    pub const fn with(self, capability: Capability) -> Self {
        Self(self.0 | capability.bit())
    }
}

/// Immutable canonical identity preimage for one Market generation.
///
/// The four content IDs are opaque at this layer. The generation and
/// capability set are part of the identity itself; there is no separate
/// caller-provided derived Market ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketIdentity {
    realm_id: ContentId,
    terms_id: ContentId,
    claim_basis_id: ContentId,
    resolution_policy_id: ContentId,
    generation: u64,
    capabilities: CapabilitySet,
}

impl MarketIdentity {
    /// Construct one validated immutable Market identity preimage.
    pub const fn new(
        realm_id: ContentId,
        terms_id: ContentId,
        claim_basis_id: ContentId,
        resolution_policy_id: ContentId,
        generation: u64,
        capabilities: CapabilitySet,
    ) -> Self {
        Self {
            realm_id,
            terms_id,
            claim_basis_id,
            resolution_policy_id,
            generation,
            capabilities,
        }
    }

    /// Decode the exact 137-byte canonical identity preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != MARKET_IDENTITY_BYTES {
            return Err(Error::InvalidLength);
        }
        Ok(Self {
            realm_id: read_content_id(bytes, REALM_OFFSET)?,
            terms_id: read_content_id(bytes, TERMS_OFFSET)?,
            claim_basis_id: read_content_id(bytes, CLAIM_BASIS_OFFSET)?,
            resolution_policy_id: read_content_id(bytes, RESOLUTION_POLICY_OFFSET)?,
            generation: u64::from_le_bytes(read_array::<8>(bytes, GENERATION_OFFSET)?),
            capabilities: CapabilitySet::from_bits(read_byte(bytes, CAPABILITIES_OFFSET)?)?,
        })
    }

    /// Encode the exact 137-byte canonical identity preimage.
    pub fn to_bytes(self) -> [u8; MARKET_IDENTITY_BYTES] {
        let mut output = [0u8; MARKET_IDENTITY_BYTES];
        copy_at(&mut output, REALM_OFFSET, self.realm_id.as_bytes());
        copy_at(&mut output, TERMS_OFFSET, self.terms_id.as_bytes());
        copy_at(
            &mut output,
            CLAIM_BASIS_OFFSET,
            self.claim_basis_id.as_bytes(),
        );
        copy_at(
            &mut output,
            RESOLUTION_POLICY_OFFSET,
            self.resolution_policy_id.as_bytes(),
        );
        copy_at(
            &mut output,
            GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        copy_at(
            &mut output,
            CAPABILITIES_OFFSET,
            &[self.capabilities.bits()],
        );
        output
    }

    /// Return the Realm content identity.
    pub const fn realm_id(self) -> ContentId {
        self.realm_id
    }

    /// Return the terms content identity.
    pub const fn terms_id(self) -> ContentId {
        self.terms_id
    }

    /// Return the claim-basis content identity.
    pub const fn claim_basis_id(self) -> ContentId {
        self.claim_basis_id
    }

    /// Return the resolution-policy content identity.
    pub const fn resolution_policy_id(self) -> ContentId {
        self.resolution_policy_id
    }

    /// Return the immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the immutable optional capability admission set.
    pub const fn capabilities(self) -> CapabilitySet {
        self.capabilities
    }
}

/// Persistent lifecycle phase of a Market root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Phase {
    /// Immutable identity exists while founding obligations are assembled.
    Founding = 0,
    /// Liability transitions are admitted.
    Open = 1,
    /// One terminal Product result has been accepted.
    Resolved = 2,
    /// No new direct children may be created while obligations are retired.
    Retiring = 3,
    /// Terminal replay authority retained after all direct children close.
    Retired = 4,
}

impl Phase {
    const fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Founding),
            1 => Ok(Self::Open),
            2 => Ok(Self::Resolved),
            3 => Ok(Self::Retiring),
            4 => Ok(Self::Retired),
            _ => Err(Error::UnknownPhase),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Founding => 0,
            Self::Open => 1,
            Self::Resolved => 2,
            Self::Retiring => 3,
            Self::Retired => 4,
        }
    }
}

/// Compact persistent Market identity, lifecycle, and replay authority.
///
/// `outstanding_children` counts currently live direct physical child roots or
/// obligations. It is independent of capability population; nested capability
/// roots own their descendant counts. Economic emptiness is outside this
/// contract and must be checked by the composing adapter before retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketRoot {
    identity: MarketIdentity,
    phase: Phase,
    outstanding_children: u64,
}

impl MarketRoot {
    /// Create a root in `Founding` with exactly zero live direct children.
    pub const fn founding(identity: MarketIdentity) -> Self {
        Self {
            identity,
            phase: Phase::Founding,
            outstanding_children: 0,
        }
    }

    /// Decode and validate the exact 168-byte canonical Market root.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != MARKET_ROOT_BYTES {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, ROOT_MAGIC_OFFSET)? != MARKET_ROOT_MAGIC {
            return Err(Error::InvalidMagic);
        }
        let schema = u16::from_le_bytes(read_array::<2>(bytes, ROOT_SCHEMA_OFFSET)?);
        if schema != MARKET_ROOT_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(
            bytes,
            ROOT_HEADER_RESERVED_OFFSET,
            ROOT_HEADER_RESERVED_BYTES,
        )?;
        require_zero(bytes, ROOT_BODY_RESERVED_OFFSET, ROOT_BODY_RESERVED_BYTES)?;

        let identity_end = ROOT_IDENTITY_OFFSET
            .checked_add(MARKET_IDENTITY_BYTES)
            .ok_or(Error::InvalidLength)?;
        let identity_bytes = bytes
            .get(ROOT_IDENTITY_OFFSET..identity_end)
            .ok_or(Error::InvalidLength)?;
        let root = Self {
            identity: MarketIdentity::decode(identity_bytes)?,
            phase: Phase::decode(read_byte(bytes, ROOT_PHASE_OFFSET)?)?,
            outstanding_children: u64::from_le_bytes(read_array::<8>(
                bytes,
                ROOT_CHILD_COUNT_OFFSET,
            )?),
        };
        root.validate()?;
        Ok(root)
    }

    /// Encode the exact 168-byte canonical Market root.
    pub fn to_bytes(self) -> [u8; MARKET_ROOT_BYTES] {
        let mut output = [0u8; MARKET_ROOT_BYTES];
        copy_at(&mut output, ROOT_MAGIC_OFFSET, &MARKET_ROOT_MAGIC);
        copy_at(
            &mut output,
            ROOT_SCHEMA_OFFSET,
            &MARKET_ROOT_SCHEMA_VERSION.to_le_bytes(),
        );
        copy_at(&mut output, ROOT_IDENTITY_OFFSET, &self.identity.to_bytes());
        copy_at(&mut output, ROOT_PHASE_OFFSET, &[self.phase.byte()]);
        copy_at(
            &mut output,
            ROOT_CHILD_COUNT_OFFSET,
            &self.outstanding_children.to_le_bytes(),
        );
        output
    }

    /// Validate cross-field canonical constraints.
    pub const fn validate(&self) -> Result<()> {
        if matches!(self.phase, Phase::Retired) && self.outstanding_children != 0 {
            return Err(Error::OutstandingChildren);
        }
        Ok(())
    }

    /// Return the immutable Market identity preimage.
    pub const fn identity(self) -> MarketIdentity {
        self.identity
    }

    /// Return the current lifecycle phase.
    pub const fn phase(self) -> Phase {
        self.phase
    }

    /// Return the exact number of live direct physical children or obligations.
    pub const fn outstanding_children(self) -> u64 {
        self.outstanding_children
    }

    /// Advance along one admitted lifecycle edge after checking generation.
    ///
    /// Admitted edges are `Founding -> Open`, `Open -> Resolved`, each of
    /// `Founding`, `Open`, and `Resolved` to `Retiring`, and
    /// `Retiring -> Retired`. The final edge additionally requires zero live
    /// direct children. Economic emptiness is a composing-adapter check.
    pub fn transition_phase(&mut self, expected_generation: u64, next: Phase) -> Result<()> {
        self.require_generation(expected_generation)?;
        let admitted = matches!(
            (self.phase, next),
            (Phase::Founding, Phase::Open)
                | (Phase::Founding, Phase::Retiring)
                | (Phase::Open, Phase::Resolved)
                | (Phase::Open, Phase::Retiring)
                | (Phase::Resolved, Phase::Retiring)
                | (Phase::Retiring, Phase::Retired)
        );
        if !admitted {
            return Err(Error::InvalidPhaseTransition);
        }
        if matches!(next, Phase::Retired) && self.outstanding_children != 0 {
            return Err(Error::OutstandingChildren);
        }
        self.phase = next;
        Ok(())
    }

    /// Register one live direct child using generation and prior-count replay guards.
    pub fn register_child(
        &mut self,
        expected_generation: u64,
        expected_prior_count: u64,
    ) -> Result<()> {
        self.require_count_guards(expected_generation, expected_prior_count)?;
        if matches!(self.phase, Phase::Retiring | Phase::Retired) {
            return Err(Error::ChildCreationClosed);
        }
        let next = self
            .outstanding_children
            .checked_add(1)
            .ok_or(Error::ChildCountOverflow)?;
        self.outstanding_children = next;
        Ok(())
    }

    /// Retire one live direct child using generation and prior-count replay guards.
    pub fn retire_child(
        &mut self,
        expected_generation: u64,
        expected_prior_count: u64,
    ) -> Result<()> {
        self.require_count_guards(expected_generation, expected_prior_count)?;
        let next = self
            .outstanding_children
            .checked_sub(1)
            .ok_or(Error::ChildCountUnderflow)?;
        self.outstanding_children = next;
        Ok(())
    }

    fn require_generation(&self, expected_generation: u64) -> Result<()> {
        if expected_generation != self.identity.generation() {
            return Err(Error::GenerationMismatch);
        }
        Ok(())
    }

    fn require_count_guards(
        &self,
        expected_generation: u64,
        expected_prior_count: u64,
    ) -> Result<()> {
        self.require_generation(expected_generation)?;
        if expected_prior_count != self.outstanding_children {
            return Err(Error::ChildCountMismatch);
        }
        Ok(())
    }
}

fn read_content_id(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(read_array::<CONTENT_ID_BYTES>(bytes, offset)?)
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn copy_at<const N: usize>(output: &mut [u8; N], offset: usize, input: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(input) {
        *destination = *source;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(byte: u8) -> Result<ContentId> {
        ContentId::new([byte; CONTENT_ID_BYTES])
    }

    fn identity(generation: u64) -> Result<MarketIdentity> {
        let capabilities = CapabilitySet::NONE
            .with(Capability::Direct)
            .with(Capability::BearerMaterialization)
            .with(Capability::Structured);
        Ok(MarketIdentity::new(
            content(1)?,
            content(2)?,
            content(3)?,
            content(4)?,
            generation,
            capabilities,
        ))
    }

    #[test]
    fn exact_width_round_trip_preserves_identity_and_root() -> Result<()> {
        assert_eq!(MARKET_IDENTITY_BYTES, 137);
        assert_eq!(MARKET_ROOT_BYTES, 168);
        let identity = identity(0x0102_0304_0506_0708)?;
        assert_eq!(MarketIdentity::decode(&identity.to_bytes()), Ok(identity));

        let root = MarketRoot::founding(identity);
        let bytes = root.to_bytes();
        assert_eq!(
            read_array::<8>(&bytes, ROOT_MAGIC_OFFSET)?,
            MARKET_ROOT_MAGIC
        );
        assert_eq!(
            read_array::<8>(&bytes, GENERATION_OFFSET + ROOT_IDENTITY_OFFSET)?,
            0x0102_0304_0506_0708u64.to_le_bytes()
        );
        assert_eq!(MarketRoot::decode(&bytes), Ok(root));
        Ok(())
    }

    #[test]
    fn every_zero_content_identity_refuses() -> Result<()> {
        assert_eq!(
            ContentId::new([0; CONTENT_ID_BYTES]),
            Err(Error::ZeroContentId)
        );
        let canonical = identity(7)?.to_bytes();
        for offset in [
            REALM_OFFSET,
            TERMS_OFFSET,
            CLAIM_BASIS_OFFSET,
            RESOLUTION_POLICY_OFFSET,
        ] {
            let mut hostile = canonical;
            for byte in hostile.iter_mut().skip(offset).take(CONTENT_ID_BYTES) {
                *byte = 0;
            }
            assert_eq!(MarketIdentity::decode(&hostile), Err(Error::ZeroContentId));
        }
        Ok(())
    }

    #[test]
    fn capability_bits_are_distinct_and_unknown_bits_refuse() -> Result<()> {
        let capabilities = [
            Capability::Direct,
            Capability::General,
            Capability::Dealer,
            Capability::BearerMaterialization,
            Capability::Fractional,
            Capability::Structured,
        ];
        let mut set = CapabilitySet::NONE;
        for capability in capabilities {
            assert!(!set.contains(capability));
            set = set.with(capability);
            assert!(set.contains(capability));
        }
        assert_eq!(set.bits(), KNOWN_CAPABILITY_BITS);
        assert_eq!(
            CapabilitySet::from_bits(1 << 6),
            Err(Error::UnknownCapabilityBits)
        );
        assert_eq!(
            CapabilitySet::from_bits(1 << 7),
            Err(Error::UnknownCapabilityBits)
        );
        assert_eq!(
            CapabilitySet::from_bits(u8::MAX),
            Err(Error::UnknownCapabilityBits)
        );

        let mut hostile = identity(1)?.to_bytes();
        let capability_byte = hostile
            .get_mut(CAPABILITIES_OFFSET)
            .ok_or(Error::InvalidLength)?;
        *capability_byte |= 1 << 6;
        assert_eq!(
            MarketIdentity::decode(&hostile),
            Err(Error::UnknownCapabilityBits)
        );
        Ok(())
    }

    #[test]
    fn noncanonical_root_encodings_refuse() -> Result<()> {
        let canonical = MarketRoot::founding(identity(9)?).to_bytes();
        let short = canonical
            .get(..MARKET_ROOT_BYTES - 1)
            .ok_or(Error::InvalidLength)?;
        assert_eq!(MarketRoot::decode(short), Err(Error::InvalidLength));
        let mut long = [0u8; MARKET_ROOT_BYTES + 1];
        copy_at(&mut long, 0, &canonical);
        assert_eq!(MarketRoot::decode(&long), Err(Error::InvalidLength));

        let mut bad_magic = canonical;
        *bad_magic
            .get_mut(ROOT_MAGIC_OFFSET)
            .ok_or(Error::InvalidLength)? ^= 0xff;
        assert_eq!(MarketRoot::decode(&bad_magic), Err(Error::InvalidMagic));

        let mut bad_schema = canonical;
        copy_at(
            &mut bad_schema,
            ROOT_SCHEMA_OFFSET,
            &(MARKET_ROOT_SCHEMA_VERSION + 1).to_le_bytes(),
        );
        assert_eq!(
            MarketRoot::decode(&bad_schema),
            Err(Error::UnsupportedSchema)
        );

        for offset in [ROOT_HEADER_RESERVED_OFFSET, ROOT_BODY_RESERVED_OFFSET] {
            let mut nonzero_reserved = canonical;
            *nonzero_reserved
                .get_mut(offset)
                .ok_or(Error::InvalidLength)? = 1;
            assert_eq!(
                MarketRoot::decode(&nonzero_reserved),
                Err(Error::NonCanonicalReservedBytes)
            );
        }

        let mut unknown_phase = canonical;
        *unknown_phase
            .get_mut(ROOT_PHASE_OFFSET)
            .ok_or(Error::InvalidLength)? = 5;
        assert_eq!(MarketRoot::decode(&unknown_phase), Err(Error::UnknownPhase));

        let mut impossible_terminal = canonical;
        *impossible_terminal
            .get_mut(ROOT_PHASE_OFFSET)
            .ok_or(Error::InvalidLength)? = Phase::Retired.byte();
        copy_at(
            &mut impossible_terminal,
            ROOT_CHILD_COUNT_OFFSET,
            &1u64.to_le_bytes(),
        );
        assert_eq!(
            MarketRoot::decode(&impossible_terminal),
            Err(Error::OutstandingChildren)
        );
        Ok(())
    }

    #[test]
    fn phase_graph_is_ordered_and_retired_is_terminal() -> Result<()> {
        let mut root = MarketRoot::founding(identity(12)?);
        let before = root;
        assert_eq!(
            root.transition_phase(12, Phase::Resolved),
            Err(Error::InvalidPhaseTransition)
        );
        assert_eq!(root, before);
        root.transition_phase(12, Phase::Open)?;
        root.transition_phase(12, Phase::Resolved)?;
        root.transition_phase(12, Phase::Retiring)?;
        root.transition_phase(12, Phase::Retired)?;
        assert_eq!(root.phase(), Phase::Retired);
        assert_eq!(
            root.transition_phase(12, Phase::Open),
            Err(Error::InvalidPhaseTransition)
        );

        let mut founding_retirement = MarketRoot::founding(identity(13)?);
        founding_retirement.transition_phase(13, Phase::Retiring)?;
        let mut open_retirement = MarketRoot::founding(identity(14)?);
        open_retirement.transition_phase(14, Phase::Open)?;
        open_retirement.transition_phase(14, Phase::Retiring)?;
        Ok(())
    }

    #[test]
    fn child_count_guards_refuse_stale_generation_and_replay_atomically() -> Result<()> {
        let mut root = MarketRoot::founding(identity(21)?);
        let initial = root;
        assert_eq!(root.register_child(20, 0), Err(Error::GenerationMismatch));
        assert_eq!(root, initial);
        root.register_child(21, 0)?;
        assert_eq!(root.outstanding_children(), 1);

        let one_child = root;
        assert_eq!(root.register_child(21, 0), Err(Error::ChildCountMismatch));
        assert_eq!(root, one_child);
        assert_eq!(root.retire_child(21, 0), Err(Error::ChildCountMismatch));
        assert_eq!(root, one_child);
        root.retire_child(21, 1)?;
        assert_eq!(root.outstanding_children(), 0);
        assert_eq!(root.retire_child(21, 0), Err(Error::ChildCountUnderflow));

        root.transition_phase(21, Phase::Retiring)?;
        assert_eq!(root.register_child(21, 0), Err(Error::ChildCreationClosed));
        root.transition_phase(21, Phase::Retired)?;
        assert_eq!(root.register_child(21, 0), Err(Error::ChildCreationClosed));
        Ok(())
    }

    #[test]
    fn retirement_requires_all_direct_children_closed() -> Result<()> {
        let mut root = MarketRoot::founding(identity(34)?);
        root.register_child(34, 0)?;
        root.transition_phase(34, Phase::Retiring)?;
        let retiring = root;
        assert_eq!(
            root.transition_phase(34, Phase::Retired),
            Err(Error::OutstandingChildren)
        );
        assert_eq!(root, retiring);
        root.retire_child(34, 1)?;
        root.transition_phase(34, Phase::Retired)?;
        assert_eq!(MarketRoot::decode(&root.to_bytes()), Ok(root));
        Ok(())
    }

    #[test]
    fn child_count_overflow_refuses_without_mutation() -> Result<()> {
        let mut bytes = MarketRoot::founding(identity(55)?).to_bytes();
        copy_at(&mut bytes, ROOT_CHILD_COUNT_OFFSET, &u64::MAX.to_le_bytes());
        let mut root = MarketRoot::decode(&bytes)?;
        let before = root;
        assert_eq!(
            root.register_child(55, u64::MAX),
            Err(Error::ChildCountOverflow)
        );
        assert_eq!(root, before);
        Ok(())
    }
}
