//! Root-independent transport for one recurring-Series Claims founding.
//!
//! A Series release is finalized before its composite Trading root address is
//! known, while the canonical [`ClaimsFoundingRequestV5`] commits three values
//! which transitively depend on that address: the Core founding-intent digest,
//! the projected-Custody Lock request digest, and the Lock receipt digest.
//! This transport replaces exactly those three coordinates with the
//! cycle-free Core permit PDA identity. Claims reconstructs the unchanged V5
//! semantic request from the authenticated permit and producer receipt before
//! it mutates any state or emits the existing V5 receipt.
//!
//! This is an instruction transport only. It is never persisted and its
//! distinct magic cannot be decoded as a V5 semantic request.

use crate::founding_v5::{
    CLAIMS_FOUNDING_REQUEST_BYTES_V5, CLAIMS_FOUNDING_REQUEST_MAGIC_V5,
    CLAIMS_FOUNDING_WIRE_VERSION_V5, ClaimsFoundingRequestV5,
};

/// Exact transient transport width.
pub const SERIES_CLAIMS_FOUNDING_TRANSPORT_BYTES_V1: usize = CLAIMS_FOUNDING_REQUEST_BYTES_V5;
/// Distinct transient transport magic.
pub const SERIES_CLAIMS_FOUNDING_TRANSPORT_MAGIC_V1: [u8; 8] = *b"DCLSFTR1";
/// Implemented transient transport version.
pub const SERIES_CLAIMS_FOUNDING_TRANSPORT_VERSION_V1: u16 = 1;

const VERSION_OFFSET: usize = 8;

/// Typed wire coordinates owned by the transient Series transport.
pub struct SeriesClaimsFoundingTransportLayoutV1;

impl SeriesClaimsFoundingTransportLayoutV1 {
    /// Placeholder replaced by the authenticated Core permit's intent digest.
    pub const FOUNDING_INTENT_DIGEST_OFFSET: usize = 240;
    /// Placeholder replaced by the appended Lock receipt's request digest.
    pub const CUSTODY_REQUEST_DIGEST_OFFSET: usize = 592;
    /// Placeholder replaced by the digest of the exact appended Lock receipt.
    pub const CUSTODY_RECEIPT_DIGEST_OFFSET: usize = 624;
}

/// Stable refusal from Series founding transport construction or decode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesClaimsFoundingTransportErrorV1 {
    /// Exact width, magic, version, or V5 static body differed.
    Encoding,
    /// The permit identity was zero.
    Permit,
    /// The three transport placeholders were not the same permit identity.
    DynamicCoordinates,
    /// Reconstructed canonical V5 coordinates refused.
    CanonicalRequest,
}

/// Result returned by the Series founding transport.
pub type Result<T> = core::result::Result<T, SeriesClaimsFoundingTransportErrorV1>;

/// Hostile-decoded root-independent Series Claims founding transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesClaimsFoundingTransportV1 {
    template: ClaimsFoundingRequestV5,
    permit: [u8; 32],
}

impl SeriesClaimsFoundingTransportV1 {
    /// Normalize a canonical request into a root-independent Effect template.
    ///
    /// The release-set identity is only a nonzero compile-time placeholder.
    /// The Series AccountProfile projects the actual permit account key and
    /// the Effect writes that key over all three placeholder coordinates before
    /// Claims sees the transport. Claims then requires the resolved identity to
    /// equal the authenticated permit account.
    pub fn root_independent_template(request: ClaimsFoundingRequestV5) -> Result<Self> {
        Self::from_canonical_v5(request.release_set(), request)
    }

    /// Normalize one canonical V5 request under its cycle-free permit PDA.
    ///
    /// Every non-dynamic coordinate is copied from the already validated V5
    /// request. The three root-dependent identities are replaced by `permit`,
    /// which makes transport bytes invariant under parent-root substitution.
    pub fn from_canonical_v5(permit: [u8; 32], request: ClaimsFoundingRequestV5) -> Result<Self> {
        require_permit(permit)?;
        let mut input = request.input();
        input.founding_intent_digest = permit;
        input.custody_request_digest = permit;
        input.custody_receipt_digest = permit;
        let template = ClaimsFoundingRequestV5::new(input)
            .map_err(|_| SeriesClaimsFoundingTransportErrorV1::CanonicalRequest)?;
        Ok(Self { template, permit })
    }

    /// Hostile-decode one exact transient transport.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != SERIES_CLAIMS_FOUNDING_TRANSPORT_BYTES_V1
            || input.get(..SERIES_CLAIMS_FOUNDING_TRANSPORT_MAGIC_V1.len())
                != Some(SERIES_CLAIMS_FOUNDING_TRANSPORT_MAGIC_V1.as_slice())
        {
            return Err(SeriesClaimsFoundingTransportErrorV1::Encoding);
        }
        let version = input
            .get(VERSION_OFFSET..VERSION_OFFSET + 2)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u16::from_le_bytes)
            .ok_or(SeriesClaimsFoundingTransportErrorV1::Encoding)?;
        if version != SERIES_CLAIMS_FOUNDING_TRANSPORT_VERSION_V1 {
            return Err(SeriesClaimsFoundingTransportErrorV1::Encoding);
        }
        let mut canonical = [0_u8; SERIES_CLAIMS_FOUNDING_TRANSPORT_BYTES_V1];
        canonical.copy_from_slice(input);
        canonical
            .get_mut(..CLAIMS_FOUNDING_REQUEST_MAGIC_V5.len())
            .ok_or(SeriesClaimsFoundingTransportErrorV1::Encoding)?
            .copy_from_slice(&CLAIMS_FOUNDING_REQUEST_MAGIC_V5);
        canonical
            .get_mut(VERSION_OFFSET..VERSION_OFFSET + 2)
            .ok_or(SeriesClaimsFoundingTransportErrorV1::Encoding)?
            .copy_from_slice(&CLAIMS_FOUNDING_WIRE_VERSION_V5.to_le_bytes());
        let template = ClaimsFoundingRequestV5::decode(&canonical)
            .map_err(|_| SeriesClaimsFoundingTransportErrorV1::Encoding)?;
        let permit = template.founding_intent_digest();
        require_permit(permit)?;
        if template.custody_request_digest() != permit
            || template.custody_receipt_digest() != permit
        {
            return Err(SeriesClaimsFoundingTransportErrorV1::DynamicCoordinates);
        }
        Ok(Self { template, permit })
    }

    /// Encode exact transient bytes.
    #[must_use]
    pub fn to_bytes(self) -> [u8; SERIES_CLAIMS_FOUNDING_TRANSPORT_BYTES_V1] {
        let mut output = self.template.to_bytes();
        if let Some(magic) = output.get_mut(..SERIES_CLAIMS_FOUNDING_TRANSPORT_MAGIC_V1.len()) {
            magic.copy_from_slice(&SERIES_CLAIMS_FOUNDING_TRANSPORT_MAGIC_V1);
        }
        if let Some(version) = output.get_mut(VERSION_OFFSET..VERSION_OFFSET + 2) {
            version.copy_from_slice(&SERIES_CLAIMS_FOUNDING_TRANSPORT_VERSION_V1.to_le_bytes());
        }
        output
    }

    /// Reconstruct the exact canonical V5 semantic request.
    ///
    /// The adapter must source these identities from the authenticated Core
    /// permit and exact producer receipt. This function supplies no digest and
    /// performs no hashing itself.
    pub fn reconstruct_v5(
        self,
        founding_intent_digest: [u8; 32],
        custody_request_digest: [u8; 32],
        custody_receipt_digest: [u8; 32],
    ) -> Result<ClaimsFoundingRequestV5> {
        let mut input = self.template.input();
        input.founding_intent_digest = founding_intent_digest;
        input.custody_request_digest = custody_request_digest;
        input.custody_receipt_digest = custody_receipt_digest;
        ClaimsFoundingRequestV5::new(input)
            .map_err(|_| SeriesClaimsFoundingTransportErrorV1::CanonicalRequest)
    }

    /// Immutable release-set identity used by the transport caller PDA.
    #[must_use]
    pub const fn release_set(&self) -> [u8; 32] {
        self.template.release_set()
    }

    /// Distinct occurrence future-Market identity used by the transport caller PDA.
    #[must_use]
    pub const fn market(&self) -> [u8; 32] {
        self.template.market()
    }

    /// Cycle-free Core permit PDA identity used as caller context.
    #[must_use]
    pub const fn permit(&self) -> [u8; 32] {
        self.permit
    }

    /// Registry-selected Trading program signing the transient request.
    #[must_use]
    pub const fn trading_program(&self) -> [u8; 32] {
        self.template.trading_program()
    }

    /// Registry-selected Claims program executing the transient request.
    #[must_use]
    pub const fn claims_program(&self) -> [u8; 32] {
        self.template.claims_program()
    }
}

fn require_permit(permit: [u8; 32]) -> Result<()> {
    if permit == [0; 32] {
        Err(SeriesClaimsFoundingTransportErrorV1::Permit)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::founding_v5::{ClaimsFoundingErrorV5, ClaimsFoundingRequestInputV5};

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn canonical(dynamic: u8) -> ClaimsFoundingRequestV5 {
        ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
            release_set: id(1),
            market: id(2),
            product_record_digest: id(3),
            product_instance_id: id(4),
            linked_basis_record_digest: id(5),
            semantic_basis_id: id(6),
            founder: id(7),
            founding_intent_digest: id(dynamic),
            aggregate: id(9),
            position: id(10),
            admission: id(11),
            funding_source: id(12),
            hoard: id(13),
            custody_replay: id(14),
            rent_credit: id(15),
            rent_program: id(16),
            claims_program: id(17),
            trading_program: id(18),
            custody_request_digest: id(dynamic.wrapping_add(1)),
            custody_receipt_digest: id(dynamic.wrapping_add(2)),
            generation: 21,
            claim_count: 5,
            quantity: 7,
            basis_scale: 11,
            pre_source_amount: 77,
            post_source_amount: 0,
            pre_hoard_amount: 23,
            post_hoard_amount: 100,
            pre_custody_revision: 24,
            post_custody_revision: 25,
            aggregate_rent_principal: 30,
            position_rent_principal: 31,
            admission_rent_principal: 32,
            observed_aggregate_lamports: 33,
            observed_position_lamports: 34,
            observed_admission_lamports: 35,
            pre_aggregate_revision: 0,
            post_aggregate_revision: 1,
            pre_position_revision: 0,
            post_position_revision: 1,
        })
        .expect("canonical request")
    }

    #[test]
    fn two_root_dependent_requests_have_one_transport_and_reconstruct_exactly() {
        let permit = id(40);
        let first = canonical(50);
        let second = canonical(60);
        let first_transport =
            SeriesClaimsFoundingTransportV1::from_canonical_v5(permit, first).expect("first");
        let second_transport =
            SeriesClaimsFoundingTransportV1::from_canonical_v5(permit, second).expect("second");
        assert_eq!(first_transport.to_bytes(), second_transport.to_bytes());
        assert_eq!(
            SeriesClaimsFoundingTransportV1::decode(&first_transport.to_bytes()),
            Ok(first_transport)
        );
        assert_eq!(
            first_transport.reconstruct_v5(id(50), id(51), id(52)),
            Ok(first)
        );
        assert_eq!(
            second_transport.reconstruct_v5(id(60), id(61), id(62)),
            Ok(second)
        );
    }

    #[test]
    fn wrong_magic_version_placeholders_and_zero_permit_refuse() {
        assert_eq!(
            SeriesClaimsFoundingTransportV1::from_canonical_v5([0; 32], canonical(50)),
            Err(SeriesClaimsFoundingTransportErrorV1::Permit)
        );
        let bytes = SeriesClaimsFoundingTransportV1::from_canonical_v5(id(40), canonical(50))
            .expect("transport")
            .to_bytes();
        for offset in [0, VERSION_OFFSET, 592] {
            let mut hostile = bytes;
            *hostile.get_mut(offset).expect("hostile offset") ^= 1;
            assert!(SeriesClaimsFoundingTransportV1::decode(&hostile).is_err());
        }
        assert_eq!(
            ClaimsFoundingRequestV5::decode(&bytes),
            Err(ClaimsFoundingErrorV5::InvalidMagic)
        );
    }
}
