#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! SDK-free wire boundary for Product payoff evaluation certificates.
//!
//! This crate assigns no Product or release authority. The physical adapter
//! authenticates finalized records, their owner and PDAs, and the current
//! Loader deployment before constructing these values.

/// Exact request width.
pub const PAYOFF_REQUEST_BYTES_V1: usize = 96;
/// Exact certificate width.
pub const PAYOFF_CERTIFICATE_BYTES_V1: usize = 176;
/// Product-payoff finalized-record schema/release identity.
///
/// SHA-256 of `dclutch/schema/product-payoff-v1`.
pub const PRODUCT_PAYOFF_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x68, 0x72, 0x53, 0x11, 0x8a, 0x36, 0x9c, 0xd2, 0x7d, 0x3f, 0x61, 0xa5, 0x9e, 0x3d, 0x8f, 0xcb,
    0xd2, 0xaf, 0x36, 0x7f, 0x54, 0xa2, 0xb9, 0x80, 0xa0, 0x45, 0x30, 0xdc, 0xad, 0x95, 0x75, 0xf1,
];
/// Semantic release implemented by the Product payoff adapter.
///
/// SHA-256 of `dclutch/semantic/product-payoff-adapter-v1`.
pub const PRODUCT_PAYOFF_ADAPTER_RELEASE_ID_V1: [u8; 32] = [
    0x17, 0xea, 0x76, 0x7c, 0x64, 0x76, 0xcf, 0x14, 0xfe, 0xad, 0x33, 0x15, 0x41, 0xc5, 0xc1, 0xa0,
    0xc2, 0x9f, 0x68, 0x54, 0xab, 0x6e, 0xbf, 0xa2, 0x2b, 0x49, 0x95, 0x77, 0x49, 0x9a, 0x1b, 0x67,
];
/// PDA seed domain for one immutable payoff certificate.
pub const PAYOFF_CERTIFICATE_PDA_DOMAIN_V1: &[u8; 29] = b"dclutch:payoff-certificate:v1";
/// Request wire magic.
pub const PAYOFF_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLTPRQ1";
/// Certificate wire magic.
pub const PAYOFF_CERTIFICATE_MAGIC_V1: [u8; 8] = *b"DCLTPCF1";
/// Shared implemented wire version.
pub const PAYOFF_WIRE_VERSION_V1: u16 = 1;

const VERSION_OFFSET: usize = 8;
const KIND_OFFSET: usize = 10;
const FLAG_OFFSET: usize = 11;
const HEADER_RESERVED_OFFSET: usize = 12;
const PRODUCT_DIGEST_OFFSET: usize = 16;
const ARTIFACT_DIGEST_OFFSET: usize = 48;
const REQUEST_QUERY_OFFSET: usize = 80;
const REQUEST_RESERVED_OFFSET: usize = 88;
const REGISTRY_PROGRAM_OFFSET: usize = 16;
const CERTIFICATE_PRODUCT_DIGEST_OFFSET: usize = 48;
const CERTIFICATE_ARTIFACT_DIGEST_OFFSET: usize = 80;
const PRODUCT_ID_OFFSET: usize = 112;
const DOMAIN_ID_OFFSET: usize = 120;
const COORDINATE_UNIT_ID_OFFSET: usize = 128;
const PAYOUT_SCALE_OFFSET: usize = 136;
const CERTIFICATE_QUERY_OFFSET: usize = 144;
const PAYOUT_OFFSET: usize = 152;
const LIABILITY_OFFSET: usize = 160;
const CERTIFICATE_RESERVED_OFFSET: usize = 168;

/// Refusal from the exact Product-payoff wire boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input did not have its sole exact width.
    InvalidLength,
    /// Magic selected a different wire family.
    InvalidMagic,
    /// The encoded version is unsupported.
    UnsupportedVersion,
    /// Action or Boolean discriminants were invalid.
    InvalidDiscriminant,
    /// Reserved or action-inactive bytes were nonzero.
    NonCanonicalReserved,
    /// A required record or program identity was all zero.
    ZeroIdentity,
    /// A certificate contradicted its kind-specific invariants.
    InvalidCertificate,
}

/// Certificate operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CertificateKindV1 {
    /// Evaluate the exact payoff at one Product coordinate.
    Evaluation = 0,
    /// Check one available amount against the conservative liability bound.
    Liability = 1,
}

impl CertificateKindV1 {
    const fn byte(self) -> u8 {
        match self {
            Self::Evaluation => 0,
            Self::Liability => 1,
        }
    }

    fn decode(value: u8) -> Result<Self, Error> {
        match value {
            0 => Ok(Self::Evaluation),
            1 => Ok(Self::Liability),
            _ => Err(Error::InvalidDiscriminant),
        }
    }
}

/// One exact Product payoff request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayoffRequestV1 {
    kind: CertificateKindV1,
    product_record_digest: [u8; 32],
    artifact_release_digest: [u8; 32],
    query: u64,
}

impl PayoffRequestV1 {
    /// Construct a request for two nonzero finalized-record identities.
    pub fn new(
        kind: CertificateKindV1,
        product_record_digest: [u8; 32],
        artifact_release_digest: [u8; 32],
        query: u64,
    ) -> Result<Self, Error> {
        require_nonzero(product_record_digest)?;
        require_nonzero(artifact_release_digest)?;
        Ok(Self {
            kind,
            product_record_digest,
            artifact_release_digest,
            query,
        })
    }

    /// Hostile-decode one exact request.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        require_header(bytes, PAYOFF_REQUEST_BYTES_V1, &PAYOFF_REQUEST_MAGIC_V1)?;
        if read_byte(bytes, FLAG_OFFSET)? != 0
            || !zero_span(bytes, HEADER_RESERVED_OFFSET, 4)?
            || !zero_span(bytes, REQUEST_RESERVED_OFFSET, 8)?
        {
            return Err(Error::NonCanonicalReserved);
        }
        Self::new(
            CertificateKindV1::decode(read_byte(bytes, KIND_OFFSET)?)?,
            read_array(bytes, PRODUCT_DIGEST_OFFSET)?,
            read_array(bytes, ARTIFACT_DIGEST_OFFSET)?,
            read_u64(bytes, REQUEST_QUERY_OFFSET)?,
        )
    }

    /// Encode the one canonical request.
    pub fn to_bytes(self) -> [u8; PAYOFF_REQUEST_BYTES_V1] {
        let mut output = [0_u8; PAYOFF_REQUEST_BYTES_V1];
        put(&mut output, 0, &PAYOFF_REQUEST_MAGIC_V1);
        put(
            &mut output,
            VERSION_OFFSET,
            &PAYOFF_WIRE_VERSION_V1.to_le_bytes(),
        );
        set(&mut output, KIND_OFFSET, self.kind.byte());
        put(
            &mut output,
            PRODUCT_DIGEST_OFFSET,
            &self.product_record_digest,
        );
        put(
            &mut output,
            ARTIFACT_DIGEST_OFFSET,
            &self.artifact_release_digest,
        );
        put(&mut output, REQUEST_QUERY_OFFSET, &self.query.to_le_bytes());
        output
    }

    /// Return the operation kind.
    pub const fn kind(self) -> CertificateKindV1 {
        self.kind
    }

    /// Return the exact Product record digest.
    pub const fn product_record_digest(self) -> [u8; 32] {
        self.product_record_digest
    }

    /// Return the exact artifact-release record digest.
    pub const fn artifact_release_digest(self) -> [u8; 32] {
        self.artifact_release_digest
    }

    /// Return the coordinate or available collateral amount.
    pub const fn query(self) -> u64 {
        self.query
    }
}

/// Exact authenticated Product payoff certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayoffCertificateV1 {
    kind: CertificateKindV1,
    collateralized: bool,
    registry_program: [u8; 32],
    product_record_digest: [u8; 32],
    artifact_release_digest: [u8; 32],
    product_id: u64,
    domain_id: u64,
    coordinate_unit_id: u64,
    payout_scale: u64,
    query: u64,
    payout: u64,
    liability_bound: u64,
}

impl PayoffCertificateV1 {
    /// Construct an exact evaluation certificate.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluation(
        registry_program: [u8; 32],
        product_record_digest: [u8; 32],
        artifact_release_digest: [u8; 32],
        product_id: u64,
        domain_id: u64,
        coordinate_unit_id: u64,
        payout_scale: u64,
        coordinate: u64,
        payout: u64,
        liability_bound: u64,
    ) -> Result<Self, Error> {
        let value = Self {
            kind: CertificateKindV1::Evaluation,
            collateralized: false,
            registry_program,
            product_record_digest,
            artifact_release_digest,
            product_id,
            domain_id,
            coordinate_unit_id,
            payout_scale,
            query: coordinate,
            payout,
            liability_bound,
        };
        value.validate()?;
        Ok(value)
    }

    /// Construct an exact conservative-liability certificate.
    #[allow(clippy::too_many_arguments)]
    pub fn liability(
        registry_program: [u8; 32],
        product_record_digest: [u8; 32],
        artifact_release_digest: [u8; 32],
        product_id: u64,
        domain_id: u64,
        coordinate_unit_id: u64,
        payout_scale: u64,
        available: u64,
        liability_bound: u64,
    ) -> Result<Self, Error> {
        let value = Self {
            kind: CertificateKindV1::Liability,
            collateralized: liability_bound <= available,
            registry_program,
            product_record_digest,
            artifact_release_digest,
            product_id,
            domain_id,
            coordinate_unit_id,
            payout_scale,
            query: available,
            payout: 0,
            liability_bound,
        };
        value.validate()?;
        Ok(value)
    }

    /// Hostile-decode one exact certificate.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        require_header(
            bytes,
            PAYOFF_CERTIFICATE_BYTES_V1,
            &PAYOFF_CERTIFICATE_MAGIC_V1,
        )?;
        if !zero_span(bytes, HEADER_RESERVED_OFFSET, 4)?
            || !zero_span(bytes, CERTIFICATE_RESERVED_OFFSET, 8)?
        {
            return Err(Error::NonCanonicalReserved);
        }
        let collateralized = match read_byte(bytes, FLAG_OFFSET)? {
            0 => false,
            1 => true,
            _ => return Err(Error::InvalidDiscriminant),
        };
        let value = Self {
            kind: CertificateKindV1::decode(read_byte(bytes, KIND_OFFSET)?)?,
            collateralized,
            registry_program: read_array(bytes, REGISTRY_PROGRAM_OFFSET)?,
            product_record_digest: read_array(bytes, CERTIFICATE_PRODUCT_DIGEST_OFFSET)?,
            artifact_release_digest: read_array(bytes, CERTIFICATE_ARTIFACT_DIGEST_OFFSET)?,
            product_id: read_u64(bytes, PRODUCT_ID_OFFSET)?,
            domain_id: read_u64(bytes, DOMAIN_ID_OFFSET)?,
            coordinate_unit_id: read_u64(bytes, COORDINATE_UNIT_ID_OFFSET)?,
            payout_scale: read_u64(bytes, PAYOUT_SCALE_OFFSET)?,
            query: read_u64(bytes, CERTIFICATE_QUERY_OFFSET)?,
            payout: read_u64(bytes, PAYOUT_OFFSET)?,
            liability_bound: read_u64(bytes, LIABILITY_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode the sole canonical certificate.
    pub fn to_bytes(self) -> [u8; PAYOFF_CERTIFICATE_BYTES_V1] {
        let mut output = [0_u8; PAYOFF_CERTIFICATE_BYTES_V1];
        put(&mut output, 0, &PAYOFF_CERTIFICATE_MAGIC_V1);
        put(
            &mut output,
            VERSION_OFFSET,
            &PAYOFF_WIRE_VERSION_V1.to_le_bytes(),
        );
        set(&mut output, KIND_OFFSET, self.kind.byte());
        set(&mut output, FLAG_OFFSET, u8::from(self.collateralized));
        put(&mut output, REGISTRY_PROGRAM_OFFSET, &self.registry_program);
        put(
            &mut output,
            CERTIFICATE_PRODUCT_DIGEST_OFFSET,
            &self.product_record_digest,
        );
        put(
            &mut output,
            CERTIFICATE_ARTIFACT_DIGEST_OFFSET,
            &self.artifact_release_digest,
        );
        put(
            &mut output,
            PRODUCT_ID_OFFSET,
            &self.product_id.to_le_bytes(),
        );
        put(&mut output, DOMAIN_ID_OFFSET, &self.domain_id.to_le_bytes());
        put(
            &mut output,
            COORDINATE_UNIT_ID_OFFSET,
            &self.coordinate_unit_id.to_le_bytes(),
        );
        put(
            &mut output,
            PAYOUT_SCALE_OFFSET,
            &self.payout_scale.to_le_bytes(),
        );
        put(
            &mut output,
            CERTIFICATE_QUERY_OFFSET,
            &self.query.to_le_bytes(),
        );
        put(&mut output, PAYOUT_OFFSET, &self.payout.to_le_bytes());
        put(
            &mut output,
            LIABILITY_OFFSET,
            &self.liability_bound.to_le_bytes(),
        );
        output
    }

    fn validate(self) -> Result<(), Error> {
        require_nonzero(self.registry_program)?;
        require_nonzero(self.product_record_digest)?;
        require_nonzero(self.artifact_release_digest)?;
        if self.product_id == 0
            || self.domain_id == 0
            || self.coordinate_unit_id == 0
            || self.payout_scale == 0
            || self.liability_bound == 0
            || self.payout > self.liability_bound
        {
            return Err(Error::InvalidCertificate);
        }
        match self.kind {
            CertificateKindV1::Evaluation if !self.collateralized => Ok(()),
            CertificateKindV1::Liability
                if self.payout == 0
                    && self.collateralized == (self.liability_bound <= self.query) =>
            {
                Ok(())
            }
            _ => Err(Error::InvalidCertificate),
        }
    }

    /// Return the operation kind.
    pub const fn kind(self) -> CertificateKindV1 {
        self.kind
    }
    /// Return the conservative collateral decision.
    pub const fn collateralized(self) -> bool {
        self.collateralized
    }
    /// Return the finalized-record owner that supplied both authorities.
    pub const fn registry_program(self) -> [u8; 32] {
        self.registry_program
    }
    /// Return the exact Product record digest.
    pub const fn product_record_digest(self) -> [u8; 32] {
        self.product_record_digest
    }
    /// Return the exact artifact-release record digest.
    pub const fn artifact_release_digest(self) -> [u8; 32] {
        self.artifact_release_digest
    }
    /// Return the Product's semantic scalar identity.
    pub const fn product_id(self) -> u64 {
        self.product_id
    }
    /// Return the Product-owned result-domain identity.
    pub const fn domain_id(self) -> u64 {
        self.domain_id
    }
    /// Return the Product-owned coordinate-unit identity.
    pub const fn coordinate_unit_id(self) -> u64 {
        self.coordinate_unit_id
    }
    /// Return the exact payout denominator.
    pub const fn payout_scale(self) -> u64 {
        self.payout_scale
    }
    /// Return the coordinate or available amount supplied to the operation.
    pub const fn query(self) -> u64 {
        self.query
    }
    /// Return the evaluated payout, or zero for a liability certificate.
    pub const fn payout(self) -> u64 {
        self.payout
    }
    /// Return the conservative sum-of-amplitudes liability bound.
    pub const fn liability_bound(self) -> u64 {
        self.liability_bound
    }
}

fn require_header(bytes: &[u8], width: usize, magic: &[u8; 8]) -> Result<(), Error> {
    if bytes.len() != width {
        return Err(Error::InvalidLength);
    }
    if bytes.get(..8) != Some(magic.as_slice()) {
        return Err(Error::InvalidMagic);
    }
    if read_u16(bytes, VERSION_OFFSET)? != PAYOFF_WIRE_VERSION_V1 {
        return Err(Error::UnsupportedVersion);
    }
    Ok(())
}

fn require_nonzero(value: [u8; 32]) -> Result<(), Error> {
    if value.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn checked_end(offset: usize, width: usize) -> Result<usize, Error> {
    offset.checked_add(width).ok_or(Error::InvalidLength)
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8, Error> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(read_small_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(read_small_array(bytes, offset)?))
}

fn read_array(bytes: &[u8], offset: usize) -> Result<[u8; 32], Error> {
    read_small_array(bytes, offset)
}

fn read_small_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], Error> {
    let end = checked_end(offset, N)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn zero_span(bytes: &[u8], offset: usize, width: usize) -> Result<bool, Error> {
    let end = checked_end(offset, width)?;
    Ok(bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0))
}

fn put(output: &mut [u8], offset: usize, source: &[u8]) {
    let Some(end) = offset.checked_add(source.len()) else {
        return;
    };
    let Some(destination) = output.get_mut(offset..end) else {
        return;
    };
    destination.copy_from_slice(source);
}

fn set(output: &mut [u8], offset: usize, value: u8) {
    let Some(destination) = output.get_mut(offset) else {
        return;
    };
    *destination = value;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> PayoffRequestV1 {
        PayoffRequestV1::new(CertificateKindV1::Evaluation, [1; 32], [2; 32], 37).expect("request")
    }

    #[test]
    fn request_is_exact_and_hostile() {
        let bytes = request().to_bytes();
        assert_eq!(bytes.len(), PAYOFF_REQUEST_BYTES_V1);
        assert_eq!(PayoffRequestV1::decode(&bytes), Ok(request()));
        for width in 0..PAYOFF_REQUEST_BYTES_V1 {
            assert_eq!(
                PayoffRequestV1::decode(bytes.get(..width).expect("bounded width")),
                Err(Error::InvalidLength)
            );
        }
        let mut padded = bytes.to_vec();
        padded.push(0);
        assert_eq!(PayoffRequestV1::decode(&padded), Err(Error::InvalidLength));
        for offset in [0, 8, 10, 11, 12, 88] {
            let mut hostile = bytes;
            *hostile.get_mut(offset).expect("bounded hostile offset") ^= 0xff;
            assert!(
                PayoffRequestV1::decode(&hostile).is_err(),
                "offset {offset}"
            );
        }
    }

    #[test]
    fn certificates_are_canonical_and_kind_specific() {
        let evaluation = PayoffCertificateV1::evaluation(
            [9; 32], [1; 32], [2; 32], 8101, 7001, 9, 100, 37, 17, 37,
        )
        .expect("evaluation");
        let liability =
            PayoffCertificateV1::liability([9; 32], [1; 32], [2; 32], 8101, 7001, 9, 100, 36, 37)
                .expect("liability");
        for value in [evaluation, liability] {
            let bytes = value.to_bytes();
            assert_eq!(PayoffCertificateV1::decode(&bytes), Ok(value));
            assert_eq!(
                value,
                PayoffCertificateV1::decode(&value.to_bytes()).expect("roundtrip")
            );
        }
        assert!(!liability.collateralized());
        let mut false_evaluation = evaluation.to_bytes();
        false_evaluation[FLAG_OFFSET] = 1;
        assert_eq!(
            PayoffCertificateV1::decode(&false_evaluation),
            Err(Error::InvalidCertificate)
        );
        let mut false_liability = liability.to_bytes();
        false_liability[FLAG_OFFSET] = 1;
        assert_eq!(
            PayoffCertificateV1::decode(&false_liability),
            Err(Error::InvalidCertificate)
        );
        let mut nonzero_inactive = liability.to_bytes();
        nonzero_inactive[PAYOUT_OFFSET] = 1;
        assert_eq!(
            PayoffCertificateV1::decode(&nonzero_inactive),
            Err(Error::InvalidCertificate)
        );
    }
}
