//! Minimal replay/rent root with no supply or remainder ledger.

use core::convert::TryInto;

use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};

/// Exact mutable root width.
pub const FRACTIONAL_ROOT_BYTES_V1: usize = 128;
/// Byte offset of the Fractional state inside one activated Trading root.
pub const FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4: usize = CAPABILITY_ROOT_HEADER_BYTES_V1;
/// Exact activated Trading root width for the Fractional family.
pub const FRACTIONAL_CAPABILITY_ROOT_BYTES_V4: usize =
    FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4 + FRACTIONAL_ROOT_BYTES_V1;
/// Root-state magic.
pub const FRACTIONAL_ROOT_MAGIC_V1: [u8; 8] = *b"DCLTFR01";
/// Finalized root schema label.
pub const FRACTIONAL_ROOT_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/fractional-root-v1";
/// SHA-256 identity of [`FRACTIONAL_ROOT_SCHEMA_PREIMAGE_V1`].
pub const FRACTIONAL_ROOT_SCHEMA_ID_V1: [u8; 32] = [
    0x0c, 0x30, 0xc4, 0xe8, 0xbb, 0x2a, 0xbc, 0x61, 0xf7, 0xd7, 0x6a, 0x86, 0x5a, 0x55, 0x60, 0xa1,
    0xe5, 0x67, 0x80, 0xca, 0x6a, 0xb5, 0x5b, 0xab, 0x07, 0x0f, 0x4a, 0x1b, 0x0f, 0xd6, 0xe6, 0x1a,
];
/// Current finalized root schema label.
///
/// V2 keeps the fixed 128-byte state allocation but replaces the V1
/// market-bound terms coordinate with the market-free selection config that a
/// manifest actually selects.  Execution still authenticates the separate
/// terms record and joins it to this identity; it must never treat terms as
/// the manifest config.
pub const FRACTIONAL_ROOT_SCHEMA_PREIMAGE_V2: &[u8] =
    b"dclutch/schema/fractional-root-v2|selection-config-at-16|market|beneficiary|revision|rent-principal";
/// SHA-256 of [`FRACTIONAL_ROOT_SCHEMA_PREIMAGE_V2`].
pub const FRACTIONAL_ROOT_SCHEMA_ID_V2: [u8; 32] = [
    0x6b, 0xa4, 0x9d, 0xbf, 0x17, 0x58, 0xab, 0xca, 0x80, 0xe3, 0x78, 0x1c, 0x4e, 0x3a, 0x3a, 0x14,
    0xc4, 0x54, 0x89, 0xcf, 0x34, 0xb6, 0x64, 0xa9, 0x94, 0x44, 0x2b, 0xbb, 0xed, 0x7d, 0x09, 0xa5,
];

const VERSION: u16 = 1;
const VERSION_V2: u16 = 2;
const TERMS_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const BENEFICIARY_OFFSET: usize = 80;
const REVISION_OFFSET: usize = 112;
const RENT_PRINCIPAL_OFFSET: usize = 120;

/// Persisted immutable terms identity offset.
pub const FRACTIONAL_ROOT_TERMS_OFFSET_V1: usize = TERMS_OFFSET;
/// Persisted immutable market-free selection-config identity offset in V2.
pub const FRACTIONAL_ROOT_SELECTION_CONFIG_OFFSET_V2: usize = TERMS_OFFSET;
/// Persisted logical Market identity offset.
pub const FRACTIONAL_ROOT_MARKET_OFFSET_V1: usize = MARKET_OFFSET;
/// Persisted permanent RentCredit beneficiary offset.
pub const FRACTIONAL_ROOT_RENT_BENEFICIARY_OFFSET_V1: usize = BENEFICIARY_OFFSET;
/// Persisted replay revision offset.
pub const FRACTIONAL_ROOT_REVISION_OFFSET_V1: usize = REVISION_OFFSET;
/// Persisted historical rent-principal offset.
pub const FRACTIONAL_ROOT_RENT_PRINCIPAL_OFFSET_V1: usize = RENT_PRINCIPAL_OFFSET;

/// Canonical root fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalRootInputV1 {
    /// Canonical PDA bump authenticated by the lifecycle program.
    pub bump: u8,
    /// Immutable exact terms identity.
    pub terms: [u8; 32],
    /// Immutable logical Market identity.
    pub market: [u8; 32],
    /// Permanent RentCredit beneficiary.
    pub rent_beneficiary: [u8; 32],
    /// Replay revision; supply and balances remain owned elsewhere.
    pub revision: u64,
    /// Historical root rent principal.
    pub historical_rent_principal: u64,
}

/// Hostile-decoded minimal root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalRootV1(FractionalRootInputV1);

impl FractionalRootV1 {
    /// Construct one root without a shadow supply, terminal, or remainder field.
    pub fn new(input: FractionalRootInputV1) -> Option<Self> {
        if input.terms == [0; 32]
            || input.market == [0; 32]
            || input.rent_beneficiary == [0; 32]
            || input.historical_rent_principal == 0
        {
            return None;
        }
        Some(Self(input))
    }

    /// Decode one exact root.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != FRACTIONAL_ROOT_BYTES_V1
            || bytes.get(..8) != Some(FRACTIONAL_ROOT_MAGIC_V1.as_slice())
            || read_u16(bytes, 8)? != VERSION
            || bytes.get(11..16)?.iter().any(|byte| *byte != 0)
        {
            return None;
        }
        Self::new(FractionalRootInputV1 {
            bump: *bytes.get(10)?,
            terms: array(bytes, TERMS_OFFSET)?,
            market: array(bytes, MARKET_OFFSET)?,
            rent_beneficiary: array(bytes, BENEFICIARY_OFFSET)?,
            revision: read_u64(bytes, REVISION_OFFSET)?,
            historical_rent_principal: read_u64(bytes, RENT_PRINCIPAL_OFFSET)?,
        })
    }

    /// Encode exact state bytes.
    pub fn to_bytes(self) -> [u8; FRACTIONAL_ROOT_BYTES_V1] {
        let mut output = [0; FRACTIONAL_ROOT_BYTES_V1];
        output[..8].copy_from_slice(&FRACTIONAL_ROOT_MAGIC_V1);
        output[8..10].copy_from_slice(&VERSION.to_le_bytes());
        output[10] = self.0.bump;
        output[TERMS_OFFSET..TERMS_OFFSET + 32].copy_from_slice(&self.0.terms);
        output[MARKET_OFFSET..MARKET_OFFSET + 32].copy_from_slice(&self.0.market);
        output[BENEFICIARY_OFFSET..BENEFICIARY_OFFSET + 32]
            .copy_from_slice(&self.0.rent_beneficiary);
        output[REVISION_OFFSET..REVISION_OFFSET + 8]
            .copy_from_slice(&self.0.revision.to_le_bytes());
        output[RENT_PRINCIPAL_OFFSET..RENT_PRINCIPAL_OFFSET + 8]
            .copy_from_slice(&self.0.historical_rent_principal.to_le_bytes());
        output
    }

    /// Exact fields.
    pub const fn input(self) -> FractionalRootInputV1 {
        self.0
    }
}

/// Current root fields.
///
/// The execution terms remain a separate Market-bound authenticated record.
/// They are joined against `selection_config` at execution; storing their
/// digest in the root would recreate the manifest fixed point V2 removes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalRootInputV2 {
    /// Canonical PDA bump supplied by the activation seam.
    pub bump: u8,
    /// Immutable market-free Fractional selection-config identity.
    pub selection_config: [u8; 32],
    /// Immutable logical Market identity.
    pub market: [u8; 32],
    /// Permanent context-selected RentCredit beneficiary.
    pub rent_beneficiary: [u8; 32],
    /// Replay revision; supply and balances remain owned elsewhere.
    pub revision: u64,
    /// Historical root rent principal.
    pub historical_rent_principal: u64,
}

/// Hostile-decoded current minimal root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalRootV2(FractionalRootInputV2);

impl FractionalRootV2 {
    /// Construct one current root without a shadow supply, terminal, or
    /// remainder field.
    pub fn new(input: FractionalRootInputV2) -> Option<Self> {
        if input.selection_config == [0; 32]
            || input.market == [0; 32]
            || input.rent_beneficiary == [0; 32]
            || input.historical_rent_principal == 0
        {
            return None;
        }
        Some(Self(input))
    }

    /// Decode one exact current root.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != FRACTIONAL_ROOT_BYTES_V1
            || bytes.get(..8) != Some(FRACTIONAL_ROOT_MAGIC_V1.as_slice())
            || read_u16(bytes, 8)? != VERSION_V2
            || bytes.get(11..16)?.iter().any(|byte| *byte != 0)
        {
            return None;
        }
        Self::new(FractionalRootInputV2 {
            bump: *bytes.get(10)?,
            selection_config: array(bytes, TERMS_OFFSET)?,
            market: array(bytes, MARKET_OFFSET)?,
            rent_beneficiary: array(bytes, BENEFICIARY_OFFSET)?,
            revision: read_u64(bytes, REVISION_OFFSET)?,
            historical_rent_principal: read_u64(bytes, RENT_PRINCIPAL_OFFSET)?,
        })
    }

    /// Encode exact current state bytes.
    pub fn to_bytes(self) -> [u8; FRACTIONAL_ROOT_BYTES_V1] {
        let mut output = [0; FRACTIONAL_ROOT_BYTES_V1];
        output[..8].copy_from_slice(&FRACTIONAL_ROOT_MAGIC_V1);
        output[8..10].copy_from_slice(&VERSION_V2.to_le_bytes());
        output[10] = self.0.bump;
        output[TERMS_OFFSET..TERMS_OFFSET + 32].copy_from_slice(&self.0.selection_config);
        output[MARKET_OFFSET..MARKET_OFFSET + 32].copy_from_slice(&self.0.market);
        output[BENEFICIARY_OFFSET..BENEFICIARY_OFFSET + 32]
            .copy_from_slice(&self.0.rent_beneficiary);
        output[REVISION_OFFSET..REVISION_OFFSET + 8]
            .copy_from_slice(&self.0.revision.to_le_bytes());
        output[RENT_PRINCIPAL_OFFSET..RENT_PRINCIPAL_OFFSET + 8]
            .copy_from_slice(&self.0.historical_rent_principal.to_le_bytes());
        output
    }

    /// Exact fields.
    pub const fn input(self) -> FractionalRootInputV2 {
        self.0
    }
}

/// Compose the canonical current state tail from the exact activation facts.
///
/// This is the family creation oracle for the V2 root. Activation compilers
/// call it with independent probes and blank the declared seam fields; they do
/// not restate the root layout beside their generic effect instructions.
pub fn fractional_root_creation_tail_v2(
    input: FractionalRootInputV2,
) -> Option<[u8; FRACTIONAL_ROOT_BYTES_V1]> {
    FractionalRootV2::new(input).map(FractionalRootV2::to_bytes)
}

/// One exact Fractional root state, preserving V1 as a distinct historical
/// wire meaning rather than reinterpreting its byte 16 coordinate as V2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalRootStateV2 {
    /// Historical market-bound terms root.
    V1(FractionalRootV1),
    /// Current market-free selection-config root.
    V2(FractionalRootV2),
}

impl FractionalRootStateV2 {
    /// Decode either exact historical or current state bytes.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if let Some(root) = FractionalRootV1::decode(bytes) {
            return Some(Self::V1(root));
        }
        FractionalRootV2::decode(bytes).map(Self::V2)
    }

    /// Exact immutable logical Market identity.
    pub const fn market(self) -> [u8; 32] {
        match self {
            Self::V1(root) => root.input().market,
            Self::V2(root) => root.input().market,
        }
    }

    /// Exact replay revision.
    pub const fn revision(self) -> u64 {
        match self {
            Self::V1(root) => root.input().revision,
            Self::V2(root) => root.input().revision,
        }
    }

    /// Exact permanent RentCredit beneficiary.
    pub const fn rent_beneficiary(self) -> [u8; 32] {
        match self {
            Self::V1(root) => root.input().rent_beneficiary,
            Self::V2(root) => root.input().rent_beneficiary,
        }
    }

    /// Canonical PDA bump.
    pub const fn bump(self) -> u8 {
        match self {
            Self::V1(root) => root.input().bump,
            Self::V2(root) => root.input().bump,
        }
    }

    /// Historical V1 market-bound terms identity, absent for a current root.
    pub const fn terms_v1(self) -> Option<[u8; 32]> {
        match self {
            Self::V1(root) => Some(root.input().terms),
            Self::V2(_) => None,
        }
    }

    /// Current V2 market-free selection-config identity, absent for a
    /// historical terms-root.
    pub const fn selection_config_v2(self) -> Option<[u8; 32]> {
        match self {
            Self::V1(_) => None,
            Self::V2(root) => Some(root.input().selection_config),
        }
    }

    /// Historical root rent principal, whose unit and offset are unchanged.
    pub const fn historical_rent_principal(self) -> u64 {
        match self {
            Self::V1(root) => root.input().historical_rent_principal,
            Self::V2(root) => root.input().historical_rent_principal,
        }
    }
}

/// Hostile-decoded activated Trading root and its sole Fractional state tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalCapabilityRootV4 {
    header: CapabilityRootHeaderV1,
    state: FractionalRootStateV2,
}

impl FractionalCapabilityRootV4 {
    /// Immutable generic activation header.
    pub const fn header(self) -> CapabilityRootHeaderV1 {
        self.header
    }

    /// Sole Fractional mutable state owner.
    pub const fn state(self) -> FractionalRootStateV2 {
        self.state
    }
}

/// Decode one exact activated Trading root and its Fractional state tail.
///
/// A bare 128-byte family state is not an onchain root account. Both the
/// immutable generic activation header and the family tail must be canonical.
pub fn decode_fractional_capability_root_v4(bytes: &[u8]) -> Option<FractionalCapabilityRootV4> {
    if bytes.len() != FRACTIONAL_CAPABILITY_ROOT_BYTES_V4 {
        return None;
    }
    let (header, state) = bytes.split_at(FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4);
    Some(FractionalCapabilityRootV4 {
        header: CapabilityRootHeaderV1::decode(header).ok()?,
        state: FractionalRootStateV2::decode(state)?,
    })
}

fn array<const N: usize>(input: &[u8], offset: usize) -> Option<[u8; N]> {
    input.get(offset..offset + N)?.try_into().ok()
}

fn read_u16(input: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(array(input, offset)?))
}
