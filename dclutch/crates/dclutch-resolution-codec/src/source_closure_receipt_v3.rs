//! Fixed-layout Source closure receipt with an exhaustive lamport decomposition.

use crate::{
    Error, Result, array_at, byte_at, exact, exact_width, is_zero, put, require_zero, u16_at,
    u32_at, u64_at,
};

/// Exact width of one Source closure receipt with ledger refund accounting.
pub const SOURCE_CLOSURE_RECEIPT_BYTES_V3: usize = 416;
/// Schema version of the exhaustive ledger closure receipt.
pub const SOURCE_CLOSURE_RECEIPT_VERSION_V3: u16 = 3;
/// Receipt-kind tag for a Source closure.
pub const SOURCE_CLOSURE_RECEIPT_KIND_V3: u8 = 1;
/// Magic identifying an exhaustive V3 Source closure receipt.
pub const SOURCE_CLOSURE_RECEIPT_MAGIC_V3: [u8; 8] = *b"DCSRCLS3";
/// PDA domain for a deterministic exhaustive V3 Source closure receipt.
pub const SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3: &[u8] = b"dclutch/source-close/v3";

const MAGIC_OFFSET: usize = 0;
const VERSION_OFFSET: usize = 8;
const KIND_OFFSET: usize = 10;
const RESERVED_HEADER_OFFSET: usize = 11;
const MARKET_OFFSET: usize = 16;
const SOURCE_STATE_OFFSET: usize = 48;
const SOURCE_MATERIAL_OFFSET: usize = 80;
const CAPABILITY_MANIFEST_OFFSET: usize = 112;
const TERMINAL_CERTIFICATE_OFFSET: usize = 144;
const RECEIPT_ACCOUNT_OFFSET: usize = 176;
const BENEFICIARY_OFFSET: usize = 208;
const SOURCE_STATE_DIGEST_OFFSET: usize = 240;
const TERMINAL_CERTIFICATE_DIGEST_OFFSET: usize = 272;
const FUNDING_SET_DIGEST_OFFSET: usize = 304;
const GENERATION_OFFSET: usize = 336;
const TERMINAL_SEQUENCE_OFFSET: usize = 344;
const SELECTOR_OFFSET: usize = 352;
const RESERVED_COORDINATES_OFFSET: usize = 356;
const SOURCE_REFUND_LAMPORTS_OFFSET: usize = 360;
const LEDGER_REMAINING_NATIVE_PRINCIPAL_OFFSET: usize = 368;
const LEDGER_RENT_LAMPORTS_OFFSET: usize = 376;
const LEDGER_LAMPORT_SURPLUS_OFFSET: usize = 384;
const REFUND_LAMPORTS_OFFSET: usize = 392;
const CLOSED_AT_OFFSET: usize = 400;
const RESERVED_BODY_OFFSET: usize = 408;

const _: () = assert!(SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3.len() <= 32);
const _: () = assert!(RESERVED_BODY_OFFSET + 8 == SOURCE_CLOSURE_RECEIPT_BYTES_V3);

/// Persisted receipt proving terminal Source state and exhaustively classifying
/// every native lamport discharged to one beneficiary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceClosureReceiptV3 {
    /// Canonical Market identity.
    pub market: [u8; 32],
    /// Canonical Runtime V2 Source state account that was closed.
    pub source_state: [u8; 32],
    /// Exact `SourceMaterialV3` content digest.
    pub source_material: [u8; 32],
    /// Finalized capability-manifest content identity.
    pub capability_manifest: [u8; 32],
    /// Authenticated Runtime V2 terminal Resolution certificate account.
    pub terminal_certificate: [u8; 32],
    /// This deterministic V3 closure receipt account.
    pub receipt_account: [u8; 32],
    /// Exact beneficiary receiving the discharged lamports.
    pub beneficiary: [u8; 32],
    /// Digest of the authenticated terminal Runtime V2 Source pre-state.
    pub source_state_digest: [u8; 32],
    /// Digest of the authenticated Runtime V2 terminal certificate bytes.
    pub terminal_certificate_digest: [u8; 32],
    /// Digest of the exact ordered subset-ledger funding pre-state.
    pub funding_set_digest: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact terminal certificate sequence.
    pub terminal_sequence: u64,
    /// Native Product Runtime V2 terminal selector. Zero is a valid selector.
    pub selector: u32,
    /// Lamports discharged from the Source state account itself.
    pub source_refund_lamports: u64,
    /// Native principal remaining in the subset ledger at closure.
    pub ledger_remaining_native_principal: u64,
    /// Rent reserve carried by the subset ledger at closure.
    pub ledger_rent_lamports: u64,
    /// Lamports above the ledger's remaining principal and rent reserve.
    pub ledger_lamport_surplus: u64,
    /// Exact total discharged to `beneficiary`.
    pub refund_lamports: u64,
    /// Clock timestamp at which the atomic discharge committed.
    pub closed_at: u64,
}

impl SourceClosureReceiptV3 {
    /// Hostile-decode one exact canonical V3 Source closure receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_width(input, SOURCE_CLOSURE_RECEIPT_BYTES_V3)?;
        exact(
            input,
            MAGIC_OFFSET,
            &SOURCE_CLOSURE_RECEIPT_MAGIC_V3,
            Error::InvalidMagic,
        )?;
        if u16_at(input, VERSION_OFFSET)? != SOURCE_CLOSURE_RECEIPT_VERSION_V3 {
            return Err(Error::UnsupportedVersion);
        }
        if byte_at(input, KIND_OFFSET)? != SOURCE_CLOSURE_RECEIPT_KIND_V3 {
            return Err(Error::InvalidReceiptShape);
        }
        require_zero(input, RESERVED_HEADER_OFFSET, 5)?;
        require_zero(input, RESERVED_COORDINATES_OFFSET, 4)?;
        require_zero(input, RESERVED_BODY_OFFSET, 8)?;

        let value = Self {
            market: array_at(input, MARKET_OFFSET)?,
            source_state: array_at(input, SOURCE_STATE_OFFSET)?,
            source_material: array_at(input, SOURCE_MATERIAL_OFFSET)?,
            capability_manifest: array_at(input, CAPABILITY_MANIFEST_OFFSET)?,
            terminal_certificate: array_at(input, TERMINAL_CERTIFICATE_OFFSET)?,
            receipt_account: array_at(input, RECEIPT_ACCOUNT_OFFSET)?,
            beneficiary: array_at(input, BENEFICIARY_OFFSET)?,
            source_state_digest: array_at(input, SOURCE_STATE_DIGEST_OFFSET)?,
            terminal_certificate_digest: array_at(input, TERMINAL_CERTIFICATE_DIGEST_OFFSET)?,
            funding_set_digest: array_at(input, FUNDING_SET_DIGEST_OFFSET)?,
            generation: u64_at(input, GENERATION_OFFSET)?,
            terminal_sequence: u64_at(input, TERMINAL_SEQUENCE_OFFSET)?,
            selector: u32_at(input, SELECTOR_OFFSET)?,
            source_refund_lamports: u64_at(input, SOURCE_REFUND_LAMPORTS_OFFSET)?,
            ledger_remaining_native_principal: u64_at(
                input,
                LEDGER_REMAINING_NATIVE_PRINCIPAL_OFFSET,
            )?,
            ledger_rent_lamports: u64_at(input, LEDGER_RENT_LAMPORTS_OFFSET)?,
            ledger_lamport_surplus: u64_at(input, LEDGER_LAMPORT_SURPLUS_OFFSET)?,
            refund_lamports: u64_at(input, REFUND_LAMPORTS_OFFSET)?,
            closed_at: u64_at(input, CLOSED_AT_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical V3 Source closure receipt.
    pub fn to_bytes(self) -> Result<[u8; SOURCE_CLOSURE_RECEIPT_BYTES_V3]> {
        self.validate()?;
        let mut output = [0_u8; SOURCE_CLOSURE_RECEIPT_BYTES_V3];
        put(&mut output, MAGIC_OFFSET, &SOURCE_CLOSURE_RECEIPT_MAGIC_V3)?;
        put(
            &mut output,
            VERSION_OFFSET,
            &SOURCE_CLOSURE_RECEIPT_VERSION_V3.to_le_bytes(),
        )?;
        put(&mut output, KIND_OFFSET, &[SOURCE_CLOSURE_RECEIPT_KIND_V3])?;
        for (offset, value) in [
            (MARKET_OFFSET, &self.market),
            (SOURCE_STATE_OFFSET, &self.source_state),
            (SOURCE_MATERIAL_OFFSET, &self.source_material),
            (CAPABILITY_MANIFEST_OFFSET, &self.capability_manifest),
            (TERMINAL_CERTIFICATE_OFFSET, &self.terminal_certificate),
            (RECEIPT_ACCOUNT_OFFSET, &self.receipt_account),
            (BENEFICIARY_OFFSET, &self.beneficiary),
            (SOURCE_STATE_DIGEST_OFFSET, &self.source_state_digest),
            (
                TERMINAL_CERTIFICATE_DIGEST_OFFSET,
                &self.terminal_certificate_digest,
            ),
            (FUNDING_SET_DIGEST_OFFSET, &self.funding_set_digest),
        ] {
            put(&mut output, offset, value)?;
        }
        for (offset, value) in [
            (GENERATION_OFFSET, self.generation),
            (TERMINAL_SEQUENCE_OFFSET, self.terminal_sequence),
            (SOURCE_REFUND_LAMPORTS_OFFSET, self.source_refund_lamports),
            (
                LEDGER_REMAINING_NATIVE_PRINCIPAL_OFFSET,
                self.ledger_remaining_native_principal,
            ),
            (LEDGER_RENT_LAMPORTS_OFFSET, self.ledger_rent_lamports),
            (LEDGER_LAMPORT_SURPLUS_OFFSET, self.ledger_lamport_surplus),
            (REFUND_LAMPORTS_OFFSET, self.refund_lamports),
            (CLOSED_AT_OFFSET, self.closed_at),
        ] {
            put(&mut output, offset, &value.to_le_bytes())?;
        }
        put(&mut output, SELECTOR_OFFSET, &self.selector.to_le_bytes())?;
        Ok(output)
    }

    /// Validate all coordinates and the exhaustive checked refund equation.
    pub fn validate(self) -> Result<()> {
        if [
            self.market,
            self.source_state,
            self.source_material,
            self.capability_manifest,
            self.terminal_certificate,
            self.receipt_account,
            self.beneficiary,
            self.source_state_digest,
            self.terminal_certificate_digest,
            self.funding_set_digest,
        ]
        .iter()
        .any(is_zero)
            || self.generation == 0
            || self.terminal_sequence == 0
            || self.refund_lamports == 0
            || self.closed_at == 0
        {
            return Err(Error::ZeroCoordinate);
        }

        let classified_total = self
            .source_refund_lamports
            .checked_add(self.ledger_remaining_native_principal)
            .and_then(|total| total.checked_add(self.ledger_rent_lamports))
            .and_then(|total| total.checked_add(self.ledger_lamport_surplus))
            .ok_or(Error::InvalidClosureRefund)?;
        if classified_total != self.refund_lamports {
            return Err(Error::InvalidClosureRefund);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    fn id(tag: u8) -> [u8; 32] {
        let mut value = [0_u8; 32];
        value[0] = tag;
        value
    }

    fn receipt() -> SourceClosureReceiptV3 {
        SourceClosureReceiptV3 {
            market: id(1),
            source_state: id(2),
            source_material: id(3),
            capability_manifest: id(4),
            terminal_certificate: id(5),
            receipt_account: id(6),
            beneficiary: id(7),
            source_state_digest: id(8),
            terminal_certificate_digest: id(9),
            funding_set_digest: id(10),
            generation: 11,
            terminal_sequence: 12,
            selector: 0,
            source_refund_lamports: 13,
            ledger_remaining_native_principal: 17,
            ledger_rent_lamports: 19,
            ledger_lamport_surplus: 23,
            refund_lamports: 72,
            closed_at: 1_700_000_000,
        }
    }

    #[test]
    fn roundtrip_preserves_every_coordinate_and_component() {
        let value = receipt();
        let bytes = value.to_bytes().expect("canonical receipt");
        assert_eq!(bytes.len(), SOURCE_CLOSURE_RECEIPT_BYTES_V3);
        assert_eq!(SourceClosureReceiptV3::decode(&bytes), Ok(value));
        assert_eq!(SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3.len(), 23);

        let mut zero_components = value;
        zero_components.ledger_remaining_native_principal = 0;
        zero_components.ledger_rent_lamports = 0;
        zero_components.ledger_lamport_surplus = 0;
        zero_components.refund_lamports = zero_components.source_refund_lamports;
        assert_eq!(
            SourceClosureReceiptV3::decode(
                &zero_components
                    .to_bytes()
                    .expect("coherent zero components")
            ),
            Ok(zero_components)
        );
    }

    #[test]
    fn substitutions_and_reserved_bytes_are_refused() {
        let encoded = receipt().to_bytes().expect("canonical receipt");

        let mut substituted_magic = encoded;
        substituted_magic[7] = b'2';
        assert_eq!(
            SourceClosureReceiptV3::decode(&substituted_magic),
            Err(Error::InvalidMagic)
        );

        let mut substituted_identity = encoded;
        substituted_identity[SOURCE_MATERIAL_OFFSET..SOURCE_MATERIAL_OFFSET + 32].fill(0);
        assert_eq!(
            SourceClosureReceiptV3::decode(&substituted_identity),
            Err(Error::ZeroCoordinate)
        );

        for offset in [
            RESERVED_HEADER_OFFSET,
            RESERVED_COORDINATES_OFFSET,
            RESERVED_BODY_OFFSET,
        ] {
            let mut hostile = encoded;
            hostile[offset] = 1;
            assert_eq!(
                SourceClosureReceiptV3::decode(&hostile),
                Err(Error::NonCanonicalReserved)
            );
        }
    }

    #[test]
    fn overflow_and_mismatched_total_are_refused() {
        let mut overflow = receipt();
        overflow.source_refund_lamports = u64::MAX;
        overflow.ledger_remaining_native_principal = 1;
        overflow.ledger_rent_lamports = 0;
        overflow.ledger_lamport_surplus = 0;
        overflow.refund_lamports = u64::MAX;
        assert_eq!(overflow.to_bytes(), Err(Error::InvalidClosureRefund));

        let mut mismatch = receipt();
        mismatch.refund_lamports -= 1;
        assert_eq!(mismatch.to_bytes(), Err(Error::InvalidClosureRefund));

        let mut hostile = receipt().to_bytes().expect("canonical receipt");
        hostile[REFUND_LAMPORTS_OFFSET..REFUND_LAMPORTS_OFFSET + 8]
            .copy_from_slice(&71_u64.to_le_bytes());
        assert_eq!(
            SourceClosureReceiptV3::decode(&hostile),
            Err(Error::InvalidClosureRefund)
        );
    }
}
