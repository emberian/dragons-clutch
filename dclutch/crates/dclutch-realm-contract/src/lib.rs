#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Canonical, SDK-free contracts for reusable collateral Realms and compact
//! native Positions. Product claim semantics live in `dclutch-product-contract`.
//!
//! This crate deliberately owns no hashing or SVM account policy. An adapter
//! computes a Realm content identity from [`RealmV1::to_bytes`], derives the
//! Realm account from [`REALM_PDA_DOMAIN`] plus that identity, and derives a
//! Position account from [`POSITION_PDA_DOMAIN`] plus the exact Market and
//! owner seed components exposed by [`PositionV1`]. Token programs, mint
//! accounts, transfers, rent, and account ownership remain adapter concerns.

use core::convert::TryInto;

mod realm_layout;

pub use realm_layout::RealmLayoutV1;

/// Exact byte width of one immutable [`RealmV1`] record.
pub const REALM_BYTES: usize = 112;
/// Fixed Position bytes before its `N` eight-byte outcome balances.
pub const POSITION_BASE_BYTES: usize = 88;
/// Exact byte width of a two-outcome Position.
pub const BINARY_POSITION_BYTES: usize = 104;
/// Exact byte width of a sixteen-outcome Position.
pub const MAX_POSITION_BYTES: usize = 216;
/// Minimum categorical width represented by this measured profile.
pub const MIN_OUTCOMES: usize = 2;
/// Maximum categorical width in this provisional measured profile.
pub const MAX_OUTCOMES: usize = 16;

/// Canonical Realm account magic.
pub const REALM_MAGIC: [u8; 8] = *b"DCLTRLM1";
/// Implemented Realm schema version.
pub const REALM_SCHEMA_VERSION: u16 = 1;
/// Canonical finalized-record schema label for [`RealmV1`].
pub const REALM_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] = b"dclutch/schema/realm-v1";
/// SHA-256 identity of [`REALM_SCHEMA_RELEASE_PREIMAGE_V1`].
pub const REALM_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x94, 0xfe, 0x1f, 0xd6, 0xd7, 0x25, 0x9f, 0x47, 0x50, 0x3d, 0x6a, 0xc5, 0x7e, 0xc7, 0xda, 0x78,
    0xdc, 0x38, 0x06, 0xa5, 0xed, 0x49, 0x8f, 0xea, 0xe4, 0x3e, 0xd3, 0x78, 0x5b, 0x5d, 0x0c, 0x69,
];
/// Domain seed preceding a Realm content identity in its SVM PDA derivation.
pub const REALM_PDA_DOMAIN: &[u8] = b"dclutch/realm/v1";

/// Canonical native Position account magic.
pub const POSITION_MAGIC: [u8; 8] = *b"DCLTPOS1";
/// Implemented Position schema version.
pub const POSITION_SCHEMA_VERSION: u16 = 1;
/// Domain seed preceding Market and owner keys in a Position PDA derivation.
pub const POSITION_PDA_DOMAIN: &[u8] = b"dclutch/position/v1";

const REALM_MINT_AUTHORITY_POLICY_OFFSET: usize = 10;
const REALM_FREEZE_AUTHORITY_POLICY_OFFSET: usize = 11;
const REALM_RESERVED_OFFSET: usize = 12;
const REALM_RESERVED_BYTES: usize = 4;
const REALM_TOKEN_PROGRAM_OFFSET: usize = 16;
const REALM_COLLATERAL_MINT_OFFSET: usize = 48;
const REALM_ADAPTER_RELEASE_ID_OFFSET: usize = 80;

const POSITION_OUTCOME_COUNT_OFFSET: usize = 10;
const POSITION_RESERVED_OFFSET: usize = 11;
const POSITION_RESERVED_BYTES: usize = 5;
const POSITION_MARKET_OFFSET: usize = 16;
const POSITION_OWNER_OFFSET: usize = 48;
const POSITION_GENERATION_OFFSET: usize = 80;
const POSITION_BALANCES_OFFSET: usize = 88;

/// Explicit refusal returned by a Realm or Position contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// An input or account did not have its one exact canonical width.
    InvalidLength,
    /// An output slice did not have its one exact canonical width.
    OutputLength,
    /// A canonical record had the wrong eight-byte magic.
    InvalidMagic,
    /// A canonical record named an unsupported schema version.
    UnsupportedSchema,
    /// Reserved bytes were not all zero.
    NonCanonicalReservedBytes,
    /// An authority-policy byte was not a defined canonical value.
    UnknownAuthorityPolicy,
    /// A required program, mint, release, Market, or owner was zero.
    ZeroIdentifier,
    /// A categorical outcome width was outside the measured profile.
    InvalidOutcomeCount,
    /// A quantity that must move claims was zero.
    ZeroQuantity,
    /// An outcome index was outside the active Position width.
    InvalidOutcome,
    /// Crediting a Position would exceed the exact integer domain.
    ArithmeticOverflow,
    /// Debiting a Position would exceed an owned outcome balance.
    InsufficientBalance,
    /// An operation requiring an empty Position observed a nonzero balance.
    NonemptyPosition,
}

/// Result alias for this contract crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Whether a Realm requires an absent mint authority or admits issuer control.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MintAuthorityPolicy {
    /// The composing adapter must prove that the mint authority is absent.
    RequireAbsent = 0,
    /// The Realm explicitly admits a present or absent issuer mint authority.
    AdmitIssuerControl = 1,
}

impl MintAuthorityPolicy {
    const fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::RequireAbsent),
            1 => Ok(Self::AdmitIssuerControl),
            _ => Err(Error::UnknownAuthorityPolicy),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::RequireAbsent => 0,
            Self::AdmitIssuerControl => 1,
        }
    }
}

/// Whether a Realm requires an absent freeze authority or admits issuer control.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FreezeAuthorityPolicy {
    /// The composing adapter must prove that the freeze authority is absent.
    RequireAbsent = 0,
    /// The Realm explicitly admits a present or absent issuer freeze authority.
    AdmitIssuerControl = 1,
}

impl FreezeAuthorityPolicy {
    const fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::RequireAbsent),
            1 => Ok(Self::AdmitIssuerControl),
            _ => Err(Error::UnknownAuthorityPolicy),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::RequireAbsent => 0,
            Self::AdmitIssuerControl => 1,
        }
    }
}

/// All immutable facts required to construct a reusable collateral Realm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmV1Input {
    /// Nonzero token-program public-key bytes.
    pub token_program: [u8; 32],
    /// Nonzero collateral-mint public-key bytes.
    pub collateral_mint: [u8; 32],
    /// Nonzero content identity selecting exact adapter semantics.
    pub collateral_adapter_release_id: [u8; 32],
    /// Immutable mint-authority admission policy.
    pub mint_authority_policy: MintAuthorityPolicy,
    /// Immutable freeze-authority admission policy.
    pub freeze_authority_policy: FreezeAuthorityPolicy,
}

/// Immutable, reusable collateral Realm contract.
///
/// The exact token program and Mint select the raw collateral atom. The record
/// additionally binds adapter release and explicit issuer-authority risk. It
/// deliberately stores neither a duplicate collateral-semantic identifier nor
/// mint decimals: protocol amounts are raw mint atoms, and the Mint remains the
/// semantic owner of its display precision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmV1 {
    token_program: [u8; 32],
    collateral_mint: [u8; 32],
    collateral_adapter_release_id: [u8; 32],
    mint_authority_policy: MintAuthorityPolicy,
    freeze_authority_policy: FreezeAuthorityPolicy,
}

impl RealmV1 {
    /// Validate and construct one immutable Realm.
    pub fn new(input: RealmV1Input) -> Result<Self> {
        require_nonzero(&input.token_program)?;
        require_nonzero(&input.collateral_mint)?;
        require_nonzero(&input.collateral_adapter_release_id)?;
        Ok(Self {
            token_program: input.token_program,
            collateral_mint: input.collateral_mint,
            collateral_adapter_release_id: input.collateral_adapter_release_id,
            mint_authority_policy: input.mint_authority_policy,
            freeze_authority_policy: input.freeze_authority_policy,
        })
    }

    /// Decode one exact canonical Realm record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != REALM_BYTES {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != REALM_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(read_array(bytes, 8)?) != REALM_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, REALM_RESERVED_OFFSET, REALM_RESERVED_BYTES)?;
        Self::new(RealmV1Input {
            token_program: read_array(bytes, REALM_TOKEN_PROGRAM_OFFSET)?,
            collateral_mint: read_array(bytes, REALM_COLLATERAL_MINT_OFFSET)?,
            collateral_adapter_release_id: read_array(bytes, REALM_ADAPTER_RELEASE_ID_OFFSET)?,
            mint_authority_policy: MintAuthorityPolicy::decode(read_byte(
                bytes,
                REALM_MINT_AUTHORITY_POLICY_OFFSET,
            )?)?,
            freeze_authority_policy: FreezeAuthorityPolicy::decode(read_byte(
                bytes,
                REALM_FREEZE_AUTHORITY_POLICY_OFFSET,
            )?)?,
        })
    }

    /// Return the exact Realm identity preimage used by a composing hash policy.
    pub fn to_bytes(self) -> [u8; REALM_BYTES] {
        let mut output = [0; REALM_BYTES];
        put(&mut output, 0, &REALM_MAGIC);
        put(&mut output, 8, &REALM_SCHEMA_VERSION.to_le_bytes());
        put(
            &mut output,
            REALM_MINT_AUTHORITY_POLICY_OFFSET,
            &[self.mint_authority_policy.byte()],
        );
        put(
            &mut output,
            REALM_FREEZE_AUTHORITY_POLICY_OFFSET,
            &[self.freeze_authority_policy.byte()],
        );
        put(&mut output, REALM_TOKEN_PROGRAM_OFFSET, &self.token_program);
        put(
            &mut output,
            REALM_COLLATERAL_MINT_OFFSET,
            &self.collateral_mint,
        );
        put(
            &mut output,
            REALM_ADAPTER_RELEASE_ID_OFFSET,
            &self.collateral_adapter_release_id,
        );
        output
    }

    /// Encode into one exact-width caller-owned buffer.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != REALM_BYTES {
            return Err(Error::OutputLength);
        }
        output.copy_from_slice(&self.to_bytes());
        Ok(())
    }

    /// Return the exact token-program public-key bytes.
    pub const fn token_program(&self) -> &[u8; 32] {
        &self.token_program
    }

    /// Return the exact collateral-mint public-key bytes.
    pub const fn collateral_mint(&self) -> &[u8; 32] {
        &self.collateral_mint
    }

    /// Return the selected collateral-adapter release identity.
    pub const fn collateral_adapter_release_id(&self) -> &[u8; 32] {
        &self.collateral_adapter_release_id
    }

    /// Return the immutable mint-authority admission policy.
    pub const fn mint_authority_policy(&self) -> MintAuthorityPolicy {
        self.mint_authority_policy
    }

    /// Return the immutable freeze-authority admission policy.
    pub const fn freeze_authority_policy(&self) -> FreezeAuthorityPolicy {
        self.freeze_authority_policy
    }
}

/// Compact owned native-claim balances for one Market participant.
///
/// The SVM Position PDA seed tuple is exactly
/// `POSITION_PDA_DOMAIN`, [`Self::market`], and [`Self::owner`], in that order.
/// This type deliberately does not derive an SVM address or depend on Solana.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionV1<const N: usize> {
    market: [u8; 32],
    owner: [u8; 32],
    generation: u64,
    balances: [u64; N],
}

impl<const N: usize> PositionV1<N> {
    /// Construct one validated Position from exact owned balances.
    pub fn new(
        market: [u8; 32],
        owner: [u8; 32],
        generation: u64,
        balances: [u64; N],
    ) -> Result<Self> {
        validate_outcome_count(N)?;
        require_nonzero(&market)?;
        require_nonzero(&owner)?;
        Ok(Self {
            market,
            owner,
            generation,
            balances,
        })
    }

    /// Construct an empty Position for one Market generation and owner.
    pub fn empty(market: [u8; 32], owner: [u8; 32], generation: u64) -> Result<Self> {
        Self::new(market, owner, generation, [0; N])
    }

    /// Return the exact checked encoded length for this Position width.
    pub fn encoded_len() -> Result<usize> {
        validate_outcome_count(N)?;
        N.checked_mul(8)
            .and_then(|balances| POSITION_BASE_BYTES.checked_add(balances))
            .ok_or(Error::InvalidLength)
    }

    /// Decode one exact canonical Position account.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let expected = Self::encoded_len()?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != POSITION_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(read_array(bytes, 8)?) != POSITION_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if usize::from(read_byte(bytes, POSITION_OUTCOME_COUNT_OFFSET)?) != N {
            return Err(Error::InvalidOutcomeCount);
        }
        require_zero(bytes, POSITION_RESERVED_OFFSET, POSITION_RESERVED_BYTES)?;

        let mut balances = [0; N];
        let mut index = 0usize;
        while index < N {
            let offset = POSITION_BALANCES_OFFSET + index * 8;
            let destination = balances.get_mut(index).ok_or(Error::InvalidOutcome)?;
            *destination = u64::from_le_bytes(read_array(bytes, offset)?);
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Self::new(
            read_array(bytes, POSITION_MARKET_OFFSET)?,
            read_array(bytes, POSITION_OWNER_OFFSET)?,
            u64::from_le_bytes(read_array(bytes, POSITION_GENERATION_OFFSET)?),
            balances,
        )
    }

    /// Encode into this width's exact caller-owned account buffer.
    ///
    /// Every refusal occurs before mutation, leaving `output` unchanged.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        let expected = Self::encoded_len()?;
        if output.len() != expected {
            return Err(Error::OutputLength);
        }
        require_nonzero(&self.market)?;
        require_nonzero(&self.owner)?;
        let count = u8::try_from(N).map_err(|_| Error::InvalidOutcomeCount)?;

        output.fill(0);
        put(output, 0, &POSITION_MAGIC);
        put(output, 8, &POSITION_SCHEMA_VERSION.to_le_bytes());
        put(output, POSITION_OUTCOME_COUNT_OFFSET, &[count]);
        put(output, POSITION_MARKET_OFFSET, &self.market);
        put(output, POSITION_OWNER_OFFSET, &self.owner);
        put(
            output,
            POSITION_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        for (index, balance) in self.balances.iter().enumerate() {
            let offset = POSITION_BALANCES_OFFSET + index * 8;
            put(output, offset, &balance.to_le_bytes());
        }
        Ok(())
    }

    /// Credit every outcome by one nonzero complete-set quantity atomically.
    pub fn credit_complete_set(&mut self, quantity: u64) -> Result<()> {
        require_nonzero_quantity(quantity)?;
        let mut next = self.balances;
        for balance in &mut next {
            *balance = balance
                .checked_add(quantity)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        self.balances = next;
        Ok(())
    }

    /// Debit every outcome by one nonzero complete-set quantity atomically.
    pub fn debit_complete_set(&mut self, quantity: u64) -> Result<()> {
        require_nonzero_quantity(quantity)?;
        let mut next = self.balances;
        for balance in &mut next {
            *balance = balance
                .checked_sub(quantity)
                .ok_or(Error::InsufficientBalance)?;
        }
        self.balances = next;
        Ok(())
    }

    /// Credit one selected outcome by a nonzero quantity atomically.
    pub fn credit_outcome(&mut self, outcome: usize, quantity: u64) -> Result<()> {
        require_nonzero_quantity(quantity)?;
        let mut next = self.balances;
        let selected = next.get_mut(outcome).ok_or(Error::InvalidOutcome)?;
        *selected = selected
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        self.balances = next;
        Ok(())
    }

    /// Debit one selected outcome by a nonzero quantity atomically.
    pub fn debit_outcome(&mut self, outcome: usize, quantity: u64) -> Result<()> {
        require_nonzero_quantity(quantity)?;
        let mut next = self.balances;
        let selected = next.get_mut(outcome).ok_or(Error::InvalidOutcome)?;
        *selected = selected
            .checked_sub(quantity)
            .ok_or(Error::InsufficientBalance)?;
        self.balances = next;
        Ok(())
    }

    /// Require every owned outcome balance to be exactly zero.
    pub fn require_empty(&self) -> Result<()> {
        if self.is_empty() {
            Ok(())
        } else {
            Err(Error::NonemptyPosition)
        }
    }

    /// Return whether every owned outcome balance is exactly zero.
    pub fn is_empty(&self) -> bool {
        self.balances.iter().all(|balance| *balance == 0)
    }

    /// Return the exact Market public-key bytes and second Position PDA seed.
    pub const fn market(&self) -> &[u8; 32] {
        &self.market
    }

    /// Return the exact owner public-key bytes and third Position PDA seed.
    pub const fn owner(&self) -> &[u8; 32] {
        &self.owner
    }

    /// Return the immutable Market generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Borrow the exact ordered owned outcome balances.
    pub const fn balances(&self) -> &[u64; N] {
        &self.balances
    }

    /// Consume the Position and return its exact validated composition parts.
    pub const fn into_parts(self) -> ([u8; 32], [u8; 32], u64, [u64; N]) {
        (self.market, self.owner, self.generation, self.balances)
    }
}

fn validate_outcome_count(count: usize) -> Result<()> {
    if !(MIN_OUTCOMES..=MAX_OUTCOMES).contains(&count) {
        return Err(Error::InvalidOutcomeCount);
    }
    Ok(())
}

fn require_nonzero(identifier: &[u8; 32]) -> Result<()> {
    if identifier.iter().all(|byte| *byte == 0) {
        return Err(Error::ZeroIdentifier);
    }
    Ok(())
}

const fn require_nonzero_quantity(quantity: u64) -> Result<()> {
    if quantity == 0 {
        return Err(Error::ZeroQuantity);
    }
    Ok(())
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

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(input) {
        *destination = *source;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn realm() -> Result<RealmV1> {
        RealmV1::new(RealmV1Input {
            token_program: id(2),
            collateral_mint: id(3),
            collateral_adapter_release_id: id(4),
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::AdmitIssuerControl,
        })
    }

    fn position<const N: usize>(balances: [u64; N]) -> Result<PositionV1<N>> {
        PositionV1::new(id(5), id(6), 0x0807_0605_0403_0201, balances)
    }

    #[test]
    fn realm_exact_layout_and_round_trip() -> Result<()> {
        let value = realm()?;
        let bytes = value.to_bytes();
        assert_eq!(bytes.len(), REALM_BYTES);
        assert_eq!(bytes.get(0..8), Some(&REALM_MAGIC[..]));
        assert_eq!(bytes.get(8..10), Some(&1_u16.to_le_bytes()[..]));
        assert_eq!(bytes.get(10), Some(&0));
        assert_eq!(bytes.get(11), Some(&1));
        assert_eq!(bytes.get(12..16), Some(&[0; 4][..]));
        assert_eq!(bytes.get(16..48), Some(&id(2)[..]));
        assert_eq!(bytes.get(48..80), Some(&id(3)[..]));
        assert_eq!(bytes.get(80..112), Some(&id(4)[..]));
        assert_eq!(RealmV1::decode(&bytes), Ok(value));
        assert_eq!(value.token_program(), &id(2));
        assert_eq!(value.collateral_mint(), &id(3));
        assert_eq!(value.collateral_adapter_release_id(), &id(4));
        assert_eq!(
            value.mint_authority_policy(),
            MintAuthorityPolicy::RequireAbsent
        );
        assert_eq!(
            value.freeze_authority_policy(),
            FreezeAuthorityPolicy::AdmitIssuerControl
        );
        Ok(())
    }

    #[test]
    fn hostile_realm_lengths_headers_policies_and_identifiers_refuse() -> Result<()> {
        let canonical = realm()?.to_bytes();
        for length in 0..REALM_BYTES {
            let short = canonical.get(..length).ok_or(Error::InvalidLength)?;
            assert_eq!(RealmV1::decode(short), Err(Error::InvalidLength));
        }
        let mut long = [0; REALM_BYTES + 1];
        put(&mut long, 0, &canonical);
        assert_eq!(RealmV1::decode(&long), Err(Error::InvalidLength));

        let mut bad_magic = canonical;
        *bad_magic.get_mut(0).ok_or(Error::InvalidLength)? ^= 1;
        assert_eq!(RealmV1::decode(&bad_magic), Err(Error::InvalidMagic));
        let mut bad_schema = canonical;
        put(&mut bad_schema, 8, &2_u16.to_le_bytes());
        assert_eq!(RealmV1::decode(&bad_schema), Err(Error::UnsupportedSchema));
        let mut reserved = canonical;
        *reserved.get_mut(12).ok_or(Error::InvalidLength)? = 1;
        assert_eq!(
            RealmV1::decode(&reserved),
            Err(Error::NonCanonicalReservedBytes)
        );
        for offset in [10, 11] {
            let mut unknown = canonical;
            *unknown.get_mut(offset).ok_or(Error::InvalidLength)? = 2;
            assert_eq!(
                RealmV1::decode(&unknown),
                Err(Error::UnknownAuthorityPolicy)
            );
        }
        for offset in [16, 48, 80] {
            let mut zero = canonical;
            zero.get_mut(offset..offset + 32)
                .ok_or(Error::InvalidLength)?
                .fill(0);
            assert_eq!(RealmV1::decode(&zero), Err(Error::ZeroIdentifier));
        }
        Ok(())
    }

    #[test]
    fn realm_output_length_refusal_is_atomic() -> Result<()> {
        let value = realm()?;
        let before = [0x5a; REALM_BYTES - 1];
        let mut output = before;
        assert_eq!(value.encode(&mut output), Err(Error::OutputLength));
        assert_eq!(output, before);
        Ok(())
    }

    #[test]
    fn position_exact_binary_and_maximum_layouts_round_trip() -> Result<()> {
        assert_eq!(PositionV1::<2>::encoded_len(), Ok(BINARY_POSITION_BYTES));
        assert_eq!(PositionV1::<16>::encoded_len(), Ok(MAX_POSITION_BYTES));

        let binary = position([9, 8])?;
        let mut binary_bytes = [0; BINARY_POSITION_BYTES];
        binary.encode(&mut binary_bytes)?;
        assert_eq!(binary_bytes.get(0..8), Some(&POSITION_MAGIC[..]));
        assert_eq!(binary_bytes.get(8..10), Some(&1_u16.to_le_bytes()[..]));
        assert_eq!(binary_bytes.get(10), Some(&2));
        assert_eq!(binary_bytes.get(11..16), Some(&[0; 5][..]));
        assert_eq!(binary_bytes.get(16..48), Some(&id(5)[..]));
        assert_eq!(binary_bytes.get(48..80), Some(&id(6)[..]));
        assert_eq!(
            binary_bytes.get(80..88),
            Some(&0x0807_0605_0403_0201_u64.to_le_bytes()[..])
        );
        assert_eq!(binary_bytes.get(88..96), Some(&9_u64.to_le_bytes()[..]));
        assert_eq!(binary_bytes.get(96..104), Some(&8_u64.to_le_bytes()[..]));
        assert_eq!(PositionV1::<2>::decode(&binary_bytes), Ok(binary));

        let maximum = position([7; 16])?;
        let mut maximum_bytes = [0; MAX_POSITION_BYTES];
        maximum.encode(&mut maximum_bytes)?;
        assert_eq!(PositionV1::<16>::decode(&maximum_bytes), Ok(maximum));
        assert_eq!(maximum_bytes.get(208..216), Some(&7_u64.to_le_bytes()[..]));
        assert_eq!(maximum.market(), &id(5));
        assert_eq!(maximum.owner(), &id(6));
        Ok(())
    }

    #[test]
    fn hostile_position_lengths_headers_count_reserved_and_ids_refuse() -> Result<()> {
        let value = position([1, 2, 3])?;
        let mut canonical = [0; POSITION_BASE_BYTES + 3 * 8];
        value.encode(&mut canonical)?;
        for length in 0..canonical.len() {
            let short = canonical.get(..length).ok_or(Error::InvalidLength)?;
            assert_eq!(PositionV1::<3>::decode(short), Err(Error::InvalidLength));
        }
        let mut long = [0; POSITION_BASE_BYTES + 3 * 8 + 1];
        put(&mut long, 0, &canonical);
        assert_eq!(PositionV1::<3>::decode(&long), Err(Error::InvalidLength));

        let mut changed = canonical;
        *changed.get_mut(0).ok_or(Error::InvalidLength)? ^= 1;
        assert_eq!(PositionV1::<3>::decode(&changed), Err(Error::InvalidMagic));
        let mut changed = canonical;
        put(&mut changed, 8, &2_u16.to_le_bytes());
        assert_eq!(
            PositionV1::<3>::decode(&changed),
            Err(Error::UnsupportedSchema)
        );
        let mut changed = canonical;
        *changed.get_mut(10).ok_or(Error::InvalidLength)? = 2;
        assert_eq!(
            PositionV1::<3>::decode(&changed),
            Err(Error::InvalidOutcomeCount)
        );
        let mut changed = canonical;
        *changed.get_mut(11).ok_or(Error::InvalidLength)? = 1;
        assert_eq!(
            PositionV1::<3>::decode(&changed),
            Err(Error::NonCanonicalReservedBytes)
        );
        for offset in [16, 48] {
            let mut zero = canonical;
            zero.get_mut(offset..offset + 32)
                .ok_or(Error::InvalidLength)?
                .fill(0);
            assert_eq!(PositionV1::<3>::decode(&zero), Err(Error::ZeroIdentifier));
        }
        assert_eq!(
            PositionV1::<1>::encoded_len(),
            Err(Error::InvalidOutcomeCount)
        );
        assert_eq!(
            PositionV1::<17>::encoded_len(),
            Err(Error::InvalidOutcomeCount)
        );
        Ok(())
    }

    #[test]
    fn position_output_length_refusal_is_atomic() -> Result<()> {
        let value = position([1, 2])?;
        let before = [0x5a; BINARY_POSITION_BYTES - 1];
        let mut output = before;
        assert_eq!(value.encode(&mut output), Err(Error::OutputLength));
        assert_eq!(output, before);
        Ok(())
    }

    #[test]
    fn complete_set_and_outcome_transitions_are_exact() -> Result<()> {
        let mut value = PositionV1::<3>::empty(id(1), id(2), 9)?;
        assert!(value.is_empty());
        assert_eq!(value.require_empty(), Ok(()));
        value.credit_complete_set(10)?;
        assert_eq!(value.balances(), &[10, 10, 10]);
        value.credit_outcome(1, 3)?;
        assert_eq!(value.balances(), &[10, 13, 10]);
        value.debit_outcome(1, 3)?;
        value.debit_complete_set(10)?;
        assert!(value.is_empty());
        assert_eq!(value.into_parts(), (id(1), id(2), 9, [0, 0, 0]));
        Ok(())
    }

    #[test]
    fn position_zero_invalid_index_overflow_and_underflow_are_atomic() -> Result<()> {
        let mut value = position([4, 5, 6])?;
        for refusal in [
            value.credit_complete_set(0),
            value.debit_complete_set(0),
            value.credit_outcome(0, 0),
            value.debit_outcome(0, 0),
        ] {
            assert_eq!(refusal, Err(Error::ZeroQuantity));
        }
        assert_eq!(value.balances(), &[4, 5, 6]);

        let before = value;
        assert_eq!(value.credit_outcome(3, 1), Err(Error::InvalidOutcome));
        assert_eq!(value, before);
        assert_eq!(value.debit_outcome(3, 1), Err(Error::InvalidOutcome));
        assert_eq!(value, before);
        assert_eq!(value.debit_complete_set(5), Err(Error::InsufficientBalance));
        assert_eq!(value, before);
        assert_eq!(value.debit_outcome(0, 5), Err(Error::InsufficientBalance));
        assert_eq!(value, before);

        let mut complete_overflow = position([u64::MAX, 0])?;
        let before = complete_overflow;
        assert_eq!(
            complete_overflow.credit_complete_set(1),
            Err(Error::ArithmeticOverflow)
        );
        assert_eq!(complete_overflow, before);
        let mut outcome_overflow = position([0, u64::MAX])?;
        let before = outcome_overflow;
        assert_eq!(
            outcome_overflow.credit_outcome(1, 1),
            Err(Error::ArithmeticOverflow)
        );
        assert_eq!(outcome_overflow, before);
        assert_eq!(value.require_empty(), Err(Error::NonemptyPosition));
        Ok(())
    }

    #[test]
    fn pda_seed_components_are_owned_canonical_facts() -> Result<()> {
        let value = PositionV1::<2>::empty(id(7), id(8), 11)?;
        assert_eq!(POSITION_PDA_DOMAIN, b"dclutch/position/v1");
        assert_eq!(value.market(), &id(7));
        assert_eq!(value.owner(), &id(8));
        assert_eq!(REALM_PDA_DOMAIN, b"dclutch/realm/v1");
        Ok(())
    }
}
