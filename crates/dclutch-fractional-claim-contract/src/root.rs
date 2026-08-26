//! Minimal replay/rent root with no supply or remainder ledger.

use core::convert::TryInto;

/// Exact mutable root width.
pub const FRACTIONAL_ROOT_BYTES_V1: usize = 128;
/// Root-state magic.
pub const FRACTIONAL_ROOT_MAGIC_V1: [u8; 8] = *b"DCLTFR01";
/// Finalized root schema label.
pub const FRACTIONAL_ROOT_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/fractional-root-v1";
/// SHA-256 identity of [`FRACTIONAL_ROOT_SCHEMA_PREIMAGE_V1`].
pub const FRACTIONAL_ROOT_SCHEMA_ID_V1: [u8; 32] = [
    0x0c, 0x30, 0xc4, 0xe8, 0xbb, 0x2a, 0xbc, 0x61, 0xf7, 0xd7, 0x6a, 0x86, 0x5a, 0x55, 0x60,
    0xa1, 0xe5, 0x67, 0x80, 0xca, 0x6a, 0xb5, 0x5b, 0xab, 0x07, 0x0f, 0x4a, 0x1b, 0x0f, 0xd6,
    0xe6, 0x1a,
];

const VERSION: u16 = 1;
const TERMS_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const BENEFICIARY_OFFSET: usize = 80;
const REVISION_OFFSET: usize = 112;
const RENT_PRINCIPAL_OFFSET: usize = 120;

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

fn array<const N: usize>(input: &[u8], offset: usize) -> Option<[u8; N]> {
    input.get(offset..offset + N)?.try_into().ok()
}

fn read_u16(input: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(array(input, offset)?))
}
