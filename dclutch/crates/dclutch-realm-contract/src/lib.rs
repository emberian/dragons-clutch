#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Canonical, SDK-free contracts for reusable collateral Realms and compact
//! native Positions. Product claim semantics live in `dclutch-product-contract`.
//!
//! This crate deliberately owns no hashing or SVM account policy. An adapter
//! computes a Realm content identity from [`RealmV1::to_bytes`], derives the
//! Realm account from [`REALM_PDA_DOMAIN`] plus that identity. Token programs,
//! mint accounts, transfers, rent, and account ownership remain adapter
//! concerns.
//!
//! The compact native `PositionV1` was banished with the DCLTCAT1 stratum; its
//! only consumers were `dclutch-direct-contract` and the browser fixture
//! generator, both deleted in that series. `generated_abi` still carries its
//! Lean-emitted coordinates, because the same Lean schema drives the browser,
//! where `POSITION_PDA_DOMAIN_V1` remains live for a DIFFERENT PDA family --
//! the Direct controller's `[domain, market, maker, outcome]` positions. Two
//! families share the domain string; only one of them has an account type
//! here.

use core::convert::TryInto;

/// Lean-emitted byte coordinates for `RealmV1` and the retired `PositionV1`.
///
/// This module is the crate's single authority for every Realm width, offset,
/// magic and seed domain; the constants below are projections of it. It also
/// emits the Position coordinates, which this crate no longer reads at all
/// after the DCLTCAT1 burial, because the same Lean schema drives the
/// browser's decoder and the Direct controller's Position seed domain.
#[allow(missing_docs, dead_code)]
mod generated_abi;
mod realm_layout;

pub use realm_layout::RealmLayoutV1;

/// Exact byte width of one immutable [`RealmV1`] record.
pub const REALM_BYTES: usize = generated_abi::REALM_BYTES_V1;
/// Canonical Realm account magic.
pub const REALM_MAGIC: [u8; 8] = generated_abi::REALM_MAGIC_V1;
/// Implemented Realm schema version.
pub const REALM_SCHEMA_VERSION: u16 = generated_abi::REALM_SCHEMA_VERSION_V1;
/// Canonical finalized-record schema label for [`RealmV1`].
pub const REALM_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] = b"dclutch/schema/realm-v1";
/// SHA-256 identity of [`REALM_SCHEMA_RELEASE_PREIMAGE_V1`].
pub const REALM_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x94, 0xfe, 0x1f, 0xd6, 0xd7, 0x25, 0x9f, 0x47, 0x50, 0x3d, 0x6a, 0xc5, 0x7e, 0xc7, 0xda, 0x78,
    0xdc, 0x38, 0x06, 0xa5, 0xed, 0x49, 0x8f, 0xea, 0xe4, 0x3e, 0xd3, 0x78, 0x5b, 0x5d, 0x0c, 0x69,
];
/// Domain seed preceding a Realm content identity in its SVM PDA derivation.
pub const REALM_PDA_DOMAIN: &[u8] = generated_abi::REALM_PDA_DOMAIN_V1;

const REALM_MINT_AUTHORITY_POLICY_OFFSET: usize =
    generated_abi::REALM_MINT_AUTHORITY_POLICY_OFFSET_V1;
const REALM_FREEZE_AUTHORITY_POLICY_OFFSET: usize =
    generated_abi::REALM_FREEZE_AUTHORITY_POLICY_OFFSET_V1;
const REALM_RESERVED_OFFSET: usize = generated_abi::REALM_RESERVED_OFFSET_V1;
const REALM_RESERVED_BYTES: usize = generated_abi::REALM_RESERVED_BYTES_V1;
const REALM_TOKEN_PROGRAM_OFFSET: usize = generated_abi::REALM_TOKEN_PROGRAM_OFFSET_V1;
const REALM_COLLATERAL_MINT_OFFSET: usize = generated_abi::REALM_COLLATERAL_MINT_OFFSET_V1;
const REALM_ADAPTER_RELEASE_ID_OFFSET: usize = generated_abi::REALM_ADAPTER_RELEASE_ID_OFFSET_V1;

/// Explicit refusal returned by a Realm contract.
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

fn require_nonzero(identifier: &[u8; 32]) -> Result<()> {
    if identifier.iter().all(|byte| *byte == 0) {
        return Err(Error::ZeroIdentifier);
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
    fn pda_seed_components_are_owned_canonical_facts() -> Result<()> {
        assert_eq!(REALM_PDA_DOMAIN, b"dclutch/realm/v1");
        Ok(())
    }
}
