#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! SDK-free request and certificate boundary for exact signed-rational Product
//! payoff evaluation. Authority and account authentication belong to the SBF
//! adapter; this crate owns only the canonical fixed layout and invariants.

use core::convert::TryInto;

/// Exact V2 request width.
pub const PAYOFF_REQUEST_BYTES_V2: usize = 112;
/// Exact V2 certificate width.
pub const PAYOFF_CERTIFICATE_BYTES_V2: usize = 232;
/// SHA-256 of `dclutch/schema/product-payoff-v2`.
pub const PRODUCT_PAYOFF_SCHEMA_RELEASE_ID_V2: [u8; 32] = [
    0xaf, 0x30, 0x89, 0x3d, 0x5e, 0xe7, 0x59, 0xbb, 0x2b, 0x74, 0x56, 0x0e, 0x0d, 0x6f, 0x6d, 0x42,
    0xa5, 0x80, 0x8b, 0xde, 0xed, 0x40, 0xda, 0x10, 0x5b, 0x65, 0x32, 0x3d, 0x80, 0xaa, 0xad, 0xe0,
];
/// SHA-256 of `dclutch/semantic/product-payoff-adapter-v2`.
pub const PRODUCT_PAYOFF_ADAPTER_RELEASE_ID_V2: [u8; 32] = [
    0xa9, 0xf6, 0x6d, 0x20, 0x5e, 0xfb, 0x98, 0x5d, 0x6c, 0x93, 0xf8, 0xd7, 0x12, 0xd5, 0x8c, 0x91,
    0x57, 0x5d, 0x84, 0x46, 0xec, 0x95, 0x63, 0xa3, 0xa2, 0xae, 0x85, 0x6e, 0xee, 0xa7, 0xcc, 0x29,
];
/// SHA-256 of the exact rational, clamped-tail, final-floor semantics.
pub const PRODUCT_PAYOFF_ROUNDING_RELEASE_ID_V2: [u8; 32] = [
    0x58, 0x2a, 0x80, 0xdc, 0xe7, 0xf8, 0xdb, 0x2a, 0xf4, 0x74, 0xaa, 0x1f, 0x0c, 0x81, 0xc8, 0x9f,
    0x21, 0x87, 0x05, 0x7d, 0x39, 0xe2, 0xff, 0x53, 0x8e, 0x1b, 0x65, 0xa3, 0x7a, 0x54, 0x49, 0x8a,
];
/// PDA domain for one immutable V2 payoff certificate.
pub const PAYOFF_CERTIFICATE_PDA_DOMAIN_V2: &[u8; 29] = b"dclutch:payoff-certificate:v2";
/// Request wire magic.
pub const PAYOFF_REQUEST_MAGIC_V2: [u8; 8] = *b"DCLTPRQ2";
/// Certificate wire magic.
pub const PAYOFF_CERTIFICATE_MAGIC_V2: [u8; 8] = *b"DCLTPCF2";
/// Shared V2 wire version.
pub const PAYOFF_WIRE_VERSION_V2: u16 = 2;

const KIND_OFFSET: usize = 10;
const FLAG_OFFSET: usize = 11;
const HEADER_RESERVED_OFFSET: usize = 12;
const REQUEST_PRODUCT_DIGEST_OFFSET: usize = 16;
const REQUEST_ARTIFACT_DIGEST_OFFSET: usize = 48;
const REQUEST_NUMERATOR_OFFSET: usize = 80;
const REQUEST_DENOMINATOR_OFFSET: usize = 96;
const REQUEST_AVAILABLE_OFFSET: usize = 104;
const CERT_REGISTRY_OFFSET: usize = 16;
const CERT_PRODUCT_DIGEST_OFFSET: usize = 48;
const CERT_ARTIFACT_DIGEST_OFFSET: usize = 80;
const CERT_ROUNDING_RELEASE_OFFSET: usize = 112;
const CERT_PRODUCT_ID_OFFSET: usize = 144;
const CERT_DOMAIN_ID_OFFSET: usize = 152;
const CERT_UNIT_ID_OFFSET: usize = 160;
const CERT_PAYOUT_SCALE_OFFSET: usize = 168;
const CERT_NUMERATOR_OFFSET: usize = 176;
const CERT_DENOMINATOR_OFFSET: usize = 192;
const CERT_AVAILABLE_OFFSET: usize = 200;
const CERT_PAYOUT_OFFSET: usize = 208;
const CERT_LIABILITY_OFFSET: usize = 216;
const CERT_TAIL_RESERVED_OFFSET: usize = 224;

/// Refusal from the exact V2 wire boundary.
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
    /// Reserved or kind-inactive bytes were nonzero.
    NonCanonicalReserved,
    /// A required finalized-record or program identity was all zero.
    ZeroIdentity,
    /// A rational coordinate had a zero denominator.
    ZeroDenominator,
    /// A certificate contradicted its kind-specific invariants.
    InvalidCertificate,
}

/// Certificate operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CertificateKindV2 {
    /// Evaluate at one exact signed-rational coordinate.
    Evaluation = 0,
    /// Check available collateral against the conservative liability bound.
    Liability = 1,
}

impl CertificateKindV2 {
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

/// Exact V2 evaluator request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayoffRequestV2 {
    kind: CertificateKindV2,
    product_record_digest: [u8; 32],
    artifact_release_digest: [u8; 32],
    result_numerator: i128,
    result_denominator: u64,
    available: u64,
}

impl PayoffRequestV2 {
    /// Construct one exact evaluation request.
    pub fn evaluation(
        product_record_digest: [u8; 32],
        artifact_release_digest: [u8; 32],
        result_numerator: i128,
        result_denominator: u64,
    ) -> Result<Self, Error> {
        let value = Self {
            kind: CertificateKindV2::Evaluation,
            product_record_digest,
            artifact_release_digest,
            result_numerator,
            result_denominator,
            available: 0,
        };
        value.validate()?;
        Ok(value)
    }

    /// Construct one conservative-liability request.
    pub fn liability(
        product_record_digest: [u8; 32],
        artifact_release_digest: [u8; 32],
        available: u64,
    ) -> Result<Self, Error> {
        let value = Self {
            kind: CertificateKindV2::Liability,
            product_record_digest,
            artifact_release_digest,
            result_numerator: 0,
            result_denominator: 0,
            available,
        };
        value.validate()?;
        Ok(value)
    }

    /// Hostile-decode one exact request.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        require_header(bytes, PAYOFF_REQUEST_BYTES_V2, &PAYOFF_REQUEST_MAGIC_V2)?;
        if read_byte(bytes, FLAG_OFFSET)? != 0 || !zero_span(bytes, HEADER_RESERVED_OFFSET, 4)? {
            return Err(Error::NonCanonicalReserved);
        }
        let value = Self {
            kind: CertificateKindV2::decode(read_byte(bytes, KIND_OFFSET)?)?,
            product_record_digest: read_array(bytes, REQUEST_PRODUCT_DIGEST_OFFSET)?,
            artifact_release_digest: read_array(bytes, REQUEST_ARTIFACT_DIGEST_OFFSET)?,
            result_numerator: read_i128(bytes, REQUEST_NUMERATOR_OFFSET)?,
            result_denominator: read_u64(bytes, REQUEST_DENOMINATOR_OFFSET)?,
            available: read_u64(bytes, REQUEST_AVAILABLE_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode the sole canonical request.
    pub fn to_bytes(self) -> [u8; PAYOFF_REQUEST_BYTES_V2] {
        let mut output = [0_u8; PAYOFF_REQUEST_BYTES_V2];
        put_header(
            &mut output,
            &PAYOFF_REQUEST_MAGIC_V2,
            self.kind.byte(),
            false,
        );
        put(
            &mut output,
            REQUEST_PRODUCT_DIGEST_OFFSET,
            &self.product_record_digest,
        );
        put(
            &mut output,
            REQUEST_ARTIFACT_DIGEST_OFFSET,
            &self.artifact_release_digest,
        );
        put(
            &mut output,
            REQUEST_NUMERATOR_OFFSET,
            &self.result_numerator.to_le_bytes(),
        );
        put(
            &mut output,
            REQUEST_DENOMINATOR_OFFSET,
            &self.result_denominator.to_le_bytes(),
        );
        put(
            &mut output,
            REQUEST_AVAILABLE_OFFSET,
            &self.available.to_le_bytes(),
        );
        output
    }

    fn validate(self) -> Result<(), Error> {
        require_nonzero(self.product_record_digest)?;
        require_nonzero(self.artifact_release_digest)?;
        match self.kind {
            CertificateKindV2::Evaluation
                if self.result_denominator != 0 && self.available == 0 =>
            {
                Ok(())
            }
            CertificateKindV2::Evaluation => Err(Error::ZeroDenominator),
            CertificateKindV2::Liability
                if self.result_numerator == 0 && self.result_denominator == 0 =>
            {
                Ok(())
            }
            CertificateKindV2::Liability => Err(Error::NonCanonicalReserved),
        }
    }

    /// Return the operation kind.
    pub const fn kind(self) -> CertificateKindV2 {
        self.kind
    }
    /// Return the exact payoff-record digest.
    pub const fn product_record_digest(self) -> [u8; 32] {
        self.product_record_digest
    }
    /// Return the exact adapter artifact-release digest.
    pub const fn artifact_release_digest(self) -> [u8; 32] {
        self.artifact_release_digest
    }
    /// Return the exact signed result numerator.
    pub const fn result_numerator(self) -> i128 {
        self.result_numerator
    }
    /// Return the result denominator, zero only for liability.
    pub const fn result_denominator(self) -> u64 {
        self.result_denominator
    }
    /// Return available collateral, zero for evaluation.
    pub const fn available(self) -> u64 {
        self.available
    }
}

/// Exact authenticated V2 Product payoff certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayoffCertificateV2 {
    kind: CertificateKindV2,
    collateralized: bool,
    registry_program: [u8; 32],
    product_record_digest: [u8; 32],
    artifact_release_digest: [u8; 32],
    rounding_release_id: [u8; 32],
    product_id: u64,
    domain_id: u64,
    coordinate_unit_id: u64,
    payout_scale: u64,
    result_numerator: i128,
    result_denominator: u64,
    available: u64,
    payout: u64,
    liability_bound: u64,
}

impl PayoffCertificateV2 {
    /// Construct one exact evaluation certificate.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluation(
        registry_program: [u8; 32],
        product_record_digest: [u8; 32],
        artifact_release_digest: [u8; 32],
        product_id: u64,
        domain_id: u64,
        coordinate_unit_id: u64,
        payout_scale: u64,
        result_numerator: i128,
        result_denominator: u64,
        payout: u64,
        liability_bound: u64,
    ) -> Result<Self, Error> {
        let value = Self {
            kind: CertificateKindV2::Evaluation,
            collateralized: false,
            registry_program,
            product_record_digest,
            artifact_release_digest,
            rounding_release_id: PRODUCT_PAYOFF_ROUNDING_RELEASE_ID_V2,
            product_id,
            domain_id,
            coordinate_unit_id,
            payout_scale,
            result_numerator,
            result_denominator,
            available: 0,
            payout,
            liability_bound,
        };
        value.validate()?;
        Ok(value)
    }

    /// Construct one exact liability certificate.
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
            kind: CertificateKindV2::Liability,
            collateralized: liability_bound <= available,
            registry_program,
            product_record_digest,
            artifact_release_digest,
            rounding_release_id: PRODUCT_PAYOFF_ROUNDING_RELEASE_ID_V2,
            product_id,
            domain_id,
            coordinate_unit_id,
            payout_scale,
            result_numerator: 0,
            result_denominator: 0,
            available,
            payout: 0,
            liability_bound,
        };
        value.validate()?;
        Ok(value)
    }

    /// Hostile-decode one exact V2 certificate.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        require_header(
            bytes,
            PAYOFF_CERTIFICATE_BYTES_V2,
            &PAYOFF_CERTIFICATE_MAGIC_V2,
        )?;
        if !zero_span(bytes, HEADER_RESERVED_OFFSET, 4)?
            || !zero_span(bytes, CERT_TAIL_RESERVED_OFFSET, 8)?
        {
            return Err(Error::NonCanonicalReserved);
        }
        let value = Self {
            kind: CertificateKindV2::decode(read_byte(bytes, KIND_OFFSET)?)?,
            collateralized: decode_bool(read_byte(bytes, FLAG_OFFSET)?)?,
            registry_program: read_array(bytes, CERT_REGISTRY_OFFSET)?,
            product_record_digest: read_array(bytes, CERT_PRODUCT_DIGEST_OFFSET)?,
            artifact_release_digest: read_array(bytes, CERT_ARTIFACT_DIGEST_OFFSET)?,
            rounding_release_id: read_array(bytes, CERT_ROUNDING_RELEASE_OFFSET)?,
            product_id: read_u64(bytes, CERT_PRODUCT_ID_OFFSET)?,
            domain_id: read_u64(bytes, CERT_DOMAIN_ID_OFFSET)?,
            coordinate_unit_id: read_u64(bytes, CERT_UNIT_ID_OFFSET)?,
            payout_scale: read_u64(bytes, CERT_PAYOUT_SCALE_OFFSET)?,
            result_numerator: read_i128(bytes, CERT_NUMERATOR_OFFSET)?,
            result_denominator: read_u64(bytes, CERT_DENOMINATOR_OFFSET)?,
            available: read_u64(bytes, CERT_AVAILABLE_OFFSET)?,
            payout: read_u64(bytes, CERT_PAYOUT_OFFSET)?,
            liability_bound: read_u64(bytes, CERT_LIABILITY_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode the sole canonical V2 certificate.
    pub fn to_bytes(self) -> [u8; PAYOFF_CERTIFICATE_BYTES_V2] {
        let mut output = [0_u8; PAYOFF_CERTIFICATE_BYTES_V2];
        put_header(
            &mut output,
            &PAYOFF_CERTIFICATE_MAGIC_V2,
            self.kind.byte(),
            self.collateralized,
        );
        for (offset, value) in [
            (CERT_REGISTRY_OFFSET, self.registry_program),
            (CERT_PRODUCT_DIGEST_OFFSET, self.product_record_digest),
            (CERT_ARTIFACT_DIGEST_OFFSET, self.artifact_release_digest),
            (CERT_ROUNDING_RELEASE_OFFSET, self.rounding_release_id),
        ] {
            put(&mut output, offset, &value);
        }
        for (offset, value) in [
            (CERT_PRODUCT_ID_OFFSET, self.product_id),
            (CERT_DOMAIN_ID_OFFSET, self.domain_id),
            (CERT_UNIT_ID_OFFSET, self.coordinate_unit_id),
            (CERT_PAYOUT_SCALE_OFFSET, self.payout_scale),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        put(
            &mut output,
            CERT_NUMERATOR_OFFSET,
            &self.result_numerator.to_le_bytes(),
        );
        for (offset, value) in [
            (CERT_DENOMINATOR_OFFSET, self.result_denominator),
            (CERT_AVAILABLE_OFFSET, self.available),
            (CERT_PAYOUT_OFFSET, self.payout),
            (CERT_LIABILITY_OFFSET, self.liability_bound),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        output
    }

    fn validate(self) -> Result<(), Error> {
        require_nonzero(self.registry_program)?;
        require_nonzero(self.product_record_digest)?;
        require_nonzero(self.artifact_release_digest)?;
        if self.rounding_release_id != PRODUCT_PAYOFF_ROUNDING_RELEASE_ID_V2
            || self.product_id == 0
            || self.domain_id == 0
            || self.coordinate_unit_id == 0
            || self.payout_scale == 0
            || self.liability_bound == 0
            || self.payout > self.liability_bound
        {
            return Err(Error::InvalidCertificate);
        }
        match self.kind {
            CertificateKindV2::Evaluation
                if !self.collateralized && self.result_denominator != 0 && self.available == 0 =>
            {
                Ok(())
            }
            CertificateKindV2::Liability
                if self.result_numerator == 0
                    && self.result_denominator == 0
                    && self.payout == 0
                    && self.collateralized == (self.liability_bound <= self.available) =>
            {
                Ok(())
            }
            _ => Err(Error::InvalidCertificate),
        }
    }

    /// Return the certificate role.
    pub const fn kind(self) -> CertificateKindV2 {
        self.kind
    }
    /// Return the conservative collateral decision.
    pub const fn collateralized(self) -> bool {
        self.collateralized
    }
    /// Return the finalized-record owner.
    pub const fn registry_program(self) -> [u8; 32] {
        self.registry_program
    }
    /// Return the exact payoff-record digest.
    pub const fn product_record_digest(self) -> [u8; 32] {
        self.product_record_digest
    }
    /// Return the exact adapter artifact-release digest.
    pub const fn artifact_release_digest(self) -> [u8; 32] {
        self.artifact_release_digest
    }
    /// Return the exact rounding semantic release identity.
    pub const fn rounding_release_id(self) -> [u8; 32] {
        self.rounding_release_id
    }
    /// Return the payoff Product scalar identity.
    pub const fn product_id(self) -> u64 {
        self.product_id
    }
    /// Return the payoff result-domain scalar identity.
    pub const fn domain_id(self) -> u64 {
        self.domain_id
    }
    /// Return the payoff coordinate-unit scalar identity.
    pub const fn coordinate_unit_id(self) -> u64 {
        self.coordinate_unit_id
    }
    /// Return the exact payout scale.
    pub const fn payout_scale(self) -> u64 {
        self.payout_scale
    }
    /// Return the exact signed resolution numerator.
    pub const fn result_numerator(self) -> i128 {
        self.result_numerator
    }
    /// Return the positive result denominator, or zero for liability.
    pub const fn result_denominator(self) -> u64 {
        self.result_denominator
    }
    /// Return the available collateral, or zero for evaluation.
    pub const fn available(self) -> u64 {
        self.available
    }
    /// Return the evaluated payout, or zero for liability.
    pub const fn payout(self) -> u64 {
        self.payout
    }
    /// Return the conservative liability bound.
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
    if read_u16(bytes, 8)? != PAYOFF_WIRE_VERSION_V2 {
        return Err(Error::UnsupportedVersion);
    }
    Ok(())
}

fn put_header(output: &mut [u8], magic: &[u8; 8], kind: u8, flag: bool) {
    put(output, 0, magic);
    put(output, 8, &PAYOFF_WIRE_VERSION_V2.to_le_bytes());
    put_byte(output, KIND_OFFSET, kind);
    put_byte(output, FLAG_OFFSET, u8::from(flag));
}

fn require_nonzero(value: [u8; 32]) -> Result<(), Error> {
    if value.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn decode_bool(value: u8) -> Result<bool, Error> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::InvalidDiscriminant),
    }
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

fn read_i128(bytes: &[u8], offset: usize) -> Result<i128, Error> {
    Ok(i128::from_le_bytes(read_small_array(bytes, offset)?))
}

fn read_array(bytes: &[u8], offset: usize) -> Result<[u8; 32], Error> {
    read_small_array(bytes, offset)
}

fn read_small_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], Error> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn zero_span(bytes: &[u8], offset: usize, width: usize) -> Result<bool, Error> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    Ok(bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0))
}

fn put(output: &mut [u8], offset: usize, source: &[u8]) {
    if let Some(end) = offset.checked_add(source.len())
        && let Some(destination) = output.get_mut(offset..end)
    {
        destination.copy_from_slice(source);
    }
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) {
    if let Some(destination) = output.get_mut(offset) {
        *destination = value;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    #[test]
    fn requests_are_exact_and_kind_canonical() {
        let evaluation = PayoffRequestV2::evaluation([1; 32], [2; 32], -75, 2).expect("eval");
        let liability = PayoffRequestV2::liability([1; 32], [2; 32], 37).expect("liability");
        for value in [evaluation, liability] {
            assert_eq!(PayoffRequestV2::decode(&value.to_bytes()), Ok(value));
        }
        let bytes = evaluation.to_bytes();
        for width in 0..PAYOFF_REQUEST_BYTES_V2 {
            assert_eq!(
                PayoffRequestV2::decode(bytes.get(..width).expect("bounded width")),
                Err(Error::InvalidLength)
            );
        }
        let mut padded = Vec::from(evaluation.to_bytes());
        padded.push(0);
        assert_eq!(PayoffRequestV2::decode(&padded), Err(Error::InvalidLength));
    }

    #[test]
    fn certificates_bind_exact_rational_and_rounding_release() {
        let evaluation = PayoffCertificateV2::evaluation(
            [9; 32], [1; 32], [2; 32], 81, 70, 9, 100, -75, 2, 10, 37,
        )
        .expect("evaluation");
        let liability =
            PayoffCertificateV2::liability([9; 32], [1; 32], [2; 32], 81, 70, 9, 100, 37, 37)
                .expect("liability");
        for value in [evaluation, liability] {
            assert_eq!(PayoffCertificateV2::decode(&value.to_bytes()), Ok(value));
        }
        let mut hostile = evaluation.to_bytes();
        *hostile
            .get_mut(CERT_ROUNDING_RELEASE_OFFSET)
            .expect("offset") ^= 1;
        assert_eq!(
            PayoffCertificateV2::decode(&hostile),
            Err(Error::InvalidCertificate)
        );
        let mut rounded = evaluation.to_bytes();
        put(&mut rounded, CERT_PAYOUT_OFFSET, &11_u64.to_le_bytes());
        // Structurally canonical but semantically false; the verifier/admission
        // adapter must recompute and refuse this substitution.
        assert!(PayoffCertificateV2::decode(&rounded).is_ok());
    }
}
