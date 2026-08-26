//! Minimal Structured V2 root: replay and rent only.
//!
//! Receipt supply, holder balances, shard custody, backing coefficients, payout
//! vectors, and Market lifecycle each already have exactly one semantic owner.
//! None of them is mirrored here.

use core::convert::TryInto;

use dclutch_structured_v2_kernel::{
    STRUCTURED_ROOT_BUMP_OFFSET_V2, STRUCTURED_ROOT_BYTES_V2, STRUCTURED_ROOT_MAGIC_OFFSET_V2,
    STRUCTURED_ROOT_MAGIC_V2, STRUCTURED_ROOT_MARKET_OFFSET_V2,
    STRUCTURED_ROOT_RENT_BENEFICIARY_OFFSET_V2, STRUCTURED_ROOT_RENT_PRINCIPAL_OFFSET_V2,
    STRUCTURED_ROOT_RESERVED_HEADER_OFFSET_V2, STRUCTURED_ROOT_REVISION_OFFSET_V2,
    STRUCTURED_ROOT_TERMS_OFFSET_V2, STRUCTURED_ROOT_VERSION_OFFSET_V2,
    STRUCTURED_SCHEMA_VERSION_V2,
};

const RESERVED_HEADER_BYTES: usize = 5;

/// Canonical Structured V2 root fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredRootInputV2 {
    /// Canonical PDA bump authenticated by the lifecycle program.
    pub bump: u8,
    /// Immutable Structured terms identity.
    pub terms: [u8; 32],
    /// Immutable logical Market identity.
    pub market: [u8; 32],
    /// Permanent RentCredit beneficiary.
    pub rent_beneficiary: [u8; 32],
    /// Replay revision; supply, custody, and balances remain owned elsewhere.
    pub revision: u64,
    /// Historical root rent principal.
    pub historical_rent_principal: u64,
}

/// Hostile-decoded minimal Structured V2 root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredRootV2(StructuredRootInputV2);

impl StructuredRootV2 {
    /// Construct one root without a shadow supply, coefficient, or phase field.
    pub fn new(input: StructuredRootInputV2) -> Option<Self> {
        if input.terms == [0; 32]
            || input.market == [0; 32]
            || input.rent_beneficiary == [0; 32]
            || input.terms == input.market
            || input.historical_rent_principal == 0
        {
            return None;
        }
        Some(Self(input))
    }

    /// Hostile-decode exact root bytes.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != STRUCTURED_ROOT_BYTES_V2
            || bytes.get(
                STRUCTURED_ROOT_MAGIC_OFFSET_V2
                    ..STRUCTURED_ROOT_MAGIC_OFFSET_V2 + STRUCTURED_ROOT_MAGIC_V2.len(),
            ) != Some(STRUCTURED_ROOT_MAGIC_V2.as_slice())
            || read_u16(bytes, STRUCTURED_ROOT_VERSION_OFFSET_V2)? != STRUCTURED_SCHEMA_VERSION_V2
            || bytes
                .get(
                    STRUCTURED_ROOT_RESERVED_HEADER_OFFSET_V2
                        ..STRUCTURED_ROOT_RESERVED_HEADER_OFFSET_V2 + RESERVED_HEADER_BYTES,
                )?
                .iter()
                .any(|value| *value != 0)
        {
            return None;
        }
        Self::new(StructuredRootInputV2 {
            bump: *bytes.get(STRUCTURED_ROOT_BUMP_OFFSET_V2)?,
            terms: array(bytes, STRUCTURED_ROOT_TERMS_OFFSET_V2)?,
            market: array(bytes, STRUCTURED_ROOT_MARKET_OFFSET_V2)?,
            rent_beneficiary: array(bytes, STRUCTURED_ROOT_RENT_BENEFICIARY_OFFSET_V2)?,
            revision: read_u64(bytes, STRUCTURED_ROOT_REVISION_OFFSET_V2)?,
            historical_rent_principal: read_u64(bytes, STRUCTURED_ROOT_RENT_PRINCIPAL_OFFSET_V2)?,
        })
    }

    /// Encode exact state bytes.
    pub fn to_bytes(self) -> [u8; STRUCTURED_ROOT_BYTES_V2] {
        let mut output = [0; STRUCTURED_ROOT_BYTES_V2];
        write(
            &mut output,
            STRUCTURED_ROOT_MAGIC_OFFSET_V2,
            &STRUCTURED_ROOT_MAGIC_V2,
        );
        write(
            &mut output,
            STRUCTURED_ROOT_VERSION_OFFSET_V2,
            &STRUCTURED_SCHEMA_VERSION_V2.to_le_bytes(),
        );
        write(&mut output, STRUCTURED_ROOT_BUMP_OFFSET_V2, &[self.0.bump]);
        write(&mut output, STRUCTURED_ROOT_TERMS_OFFSET_V2, &self.0.terms);
        write(
            &mut output,
            STRUCTURED_ROOT_MARKET_OFFSET_V2,
            &self.0.market,
        );
        write(
            &mut output,
            STRUCTURED_ROOT_RENT_BENEFICIARY_OFFSET_V2,
            &self.0.rent_beneficiary,
        );
        write(
            &mut output,
            STRUCTURED_ROOT_REVISION_OFFSET_V2,
            &self.0.revision.to_le_bytes(),
        );
        write(
            &mut output,
            STRUCTURED_ROOT_RENT_PRINCIPAL_OFFSET_V2,
            &self.0.historical_rent_principal.to_le_bytes(),
        );
        output
    }

    /// Exact fields.
    pub const fn input(self) -> StructuredRootInputV2 {
        self.0
    }

    /// Advance the replay revision by exactly one.
    pub fn advanced(self) -> Option<Self> {
        Self::new(StructuredRootInputV2 {
            revision: self.0.revision.checked_add(1)?,
            ..self.0
        })
    }
}

fn write(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset + value.len()) {
        target.copy_from_slice(value);
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
