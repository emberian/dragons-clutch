#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Generated fixed-layout wire types for the successor Resolution controller.
//!
//! The request contains only optimistic concurrency coordinates. Product and
//! Source policy remain in their canonical records. The receipt is the exact
//! 312-byte certificate layout generated from Lean's Source specialization.

#[rustfmt::skip]
mod generated_source_resolution;

/// Bytes in one fixed primary-Pyth admission request.
pub const ACCEPT_PYTH_REQUEST_BYTES: usize = generated_source_resolution::REQUEST_BYTES_VALUE;
/// Bytes in one canonical Source Resolution certificate.
pub const RESOLUTION_CERTIFICATE_BYTES: usize =
    generated_source_resolution::CERTIFICATE_BYTES_VALUE;
/// PDA domain for one terminal certificate paired with a Source state.
pub const RESOLUTION_CERTIFICATE_PDA_DOMAIN_V1: &[u8] = b"dclutch/resolution-cert/v1";
/// Domain separating the content identity of an authenticated Pyth update.
pub const PYTH_EVIDENCE_CONTENT_DOMAIN_V1: &[u8] = b"dclutch/pyth-evidence/v1";
/// Closed semantic release preimage for this primary-Pyth controller profile.
pub const RESOLUTION_CONTROLLER_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/release/source-resolution-primary-pyth-controller-v1";
/// SHA-256 of [`RESOLUTION_CONTROLLER_RELEASE_PREIMAGE_V1`].
pub const RESOLUTION_CONTROLLER_RELEASE_ID_V1: [u8; 32] = [
    0x2e, 0x95, 0x88, 0x6a, 0xb8, 0xca, 0xa2, 0xb7, 0x76, 0xa0, 0x0b, 0xa0, 0x47, 0x46, 0x48, 0x8c,
    0xf7, 0x66, 0xef, 0x1d, 0xb5, 0x6d, 0x79, 0x53, 0x5e, 0xae, 0xd3, 0x5d, 0xde, 0x49, 0xc1, 0x7d,
];
/// Schema identity used only to finalize a canonical Pyth-release record.
pub const PYTH_RELEASE_RECORD_SCHEMA_ID_V1: [u8; 32] = [
    0xb3, 0xa9, 0x8b, 0x34, 0x26, 0x68, 0xb4, 0x63, 0x3a, 0xb2, 0xa8, 0x42, 0x73, 0x16, 0xcd, 0xe1,
    0xb8, 0xac, 0xeb, 0x01, 0xee, 0xda, 0xcc, 0x3c, 0x3e, 0x29, 0x81, 0xec, 0x3f, 0x91, 0xdb, 0xf9,
];

/// Stable refusal from a hostile fixed-layout decoder or encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The slice did not have its one exact generated width.
    InvalidLength,
    /// Magic bytes did not identify the requested wire type.
    InvalidMagic,
    /// The schema version was not the generated V1 version.
    UnsupportedVersion,
    /// The action byte did not name primary-Pyth acceptance.
    UnknownAction,
    /// A reserved byte was nonzero.
    NonCanonicalReserved,
    /// A required generation, identity, denominator, or timestamp was zero.
    ZeroCoordinate,
    /// A result selector did not fit the physical Product profile.
    InvalidSelector,
}

/// Result alias for Resolution codecs.
pub type Result<T> = core::result::Result<T, Error>;

/// Optimistic concurrency coordinates for one primary-Pyth admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptPythRequestV1 {
    /// Exact immutable Market generation expected by the submitter.
    pub expected_generation: u64,
    /// Product-owned domain-separated result-domain content identity.
    pub expected_result_domain_id: [u8; 32],
    /// Exact Pyth deployment-release content identity selected by Source.
    pub expected_provider_release_id: [u8; 32],
}

impl AcceptPythRequestV1 {
    /// Decode one exact canonical request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_width(input, ACCEPT_PYTH_REQUEST_BYTES)?;
        exact(
            input,
            generated_source_resolution::REQUEST_MAGIC_OFFSET,
            &generated_source_resolution::REQUEST_MAGIC_BYTES,
            Error::InvalidMagic,
        )?;
        if u16_at(input, generated_source_resolution::REQUEST_VERSION_OFFSET)?
            != generated_source_resolution::REQUEST_ABI_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        if byte_at(input, generated_source_resolution::REQUEST_ACTION_OFFSET)?
            != generated_source_resolution::REQUEST_ACCEPT_PYTH_ACTION
        {
            return Err(Error::UnknownAction);
        }
        require_zero(
            input,
            generated_source_resolution::REQUEST_RESERVED_OFFSET,
            5,
        )?;
        let value = Self {
            expected_generation: u64_at(
                input,
                generated_source_resolution::REQUEST_EXPECTED_GENERATION_OFFSET,
            )?,
            expected_result_domain_id: array_at(
                input,
                generated_source_resolution::REQUEST_EXPECTED_RESULT_DOMAIN_ID_OFFSET,
            )?,
            expected_provider_release_id: array_at(
                input,
                generated_source_resolution::REQUEST_EXPECTED_PROVIDER_RELEASE_ID_OFFSET,
            )?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical request.
    pub fn to_bytes(self) -> Result<[u8; ACCEPT_PYTH_REQUEST_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; ACCEPT_PYTH_REQUEST_BYTES];
        put(
            &mut output,
            generated_source_resolution::REQUEST_MAGIC_OFFSET,
            &generated_source_resolution::REQUEST_MAGIC_BYTES,
        )?;
        put(
            &mut output,
            generated_source_resolution::REQUEST_VERSION_OFFSET,
            &generated_source_resolution::REQUEST_ABI_VERSION.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::REQUEST_ACTION_OFFSET,
            &[generated_source_resolution::REQUEST_ACCEPT_PYTH_ACTION],
        )?;
        put(
            &mut output,
            generated_source_resolution::REQUEST_EXPECTED_GENERATION_OFFSET,
            &self.expected_generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::REQUEST_EXPECTED_RESULT_DOMAIN_ID_OFFSET,
            &self.expected_result_domain_id,
        )?;
        put(
            &mut output,
            generated_source_resolution::REQUEST_EXPECTED_PROVIDER_RELEASE_ID_OFFSET,
            &self.expected_provider_release_id,
        )?;
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        if self.expected_generation == 0
            || is_zero(&self.expected_result_domain_id)
            || is_zero(&self.expected_provider_release_id)
        {
            return Err(Error::ZeroCoordinate);
        }
        Ok(())
    }
}

/// Candidate binding for the Registry-owned Resolution execution role.
///
/// This value is deliberately not accepted as runtime authority by the
/// controller. The Registry activation binding must eventually authenticate
/// this exact pair before lending the controller authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionReleaseCandidateV1 {
    /// Candidate Resolution program account identity.
    pub program_id: [u8; 32],
    /// Candidate checked artifact-release content identity.
    pub artifact_release_id: [u8; 32],
}

impl ResolutionReleaseCandidateV1 {
    /// Construct a nonzero role-binding candidate.
    pub fn new(program_id: [u8; 32], artifact_release_id: [u8; 32]) -> Result<Self> {
        if is_zero(&program_id) || is_zero(&artifact_release_id) {
            return Err(Error::ZeroCoordinate);
        }
        Ok(Self {
            program_id,
            artifact_release_id,
        })
    }
}

/// Canonical physical projection of one successful Source Resolution certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionCertificateV1 {
    /// Canonical Market account identity.
    pub market: [u8; 32],
    /// Exact active Source-specification content identity.
    pub route: [u8; 32],
    /// Canonical Source-material content identity.
    pub source_material: [u8; 32],
    /// Canonical Product-instance content identity.
    pub product: [u8; 32],
    /// Domain-separated authenticated Pyth-evidence content identity.
    pub provider_evidence: [u8; 32],
    /// Optional funding-allocation identity; primary admission uses all zeroes.
    pub funding_allocation: [u8; 32],
    /// Canonical certificate account identity.
    pub receipt_account: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Ordered recovery index; primary admission uses zero.
    pub attempt_index: u32,
    /// Exact Source schedule index.
    pub schedule_index: u32,
    /// Product-owned result selector.
    pub selector: u32,
    /// Exact work paid by this transition; zero in the primary hot profile.
    pub work_paid: u64,
    /// Authenticated remaining work funding; zero in the primary hot profile.
    pub funding_remaining: u64,
    /// Exact signed normalized result numerator.
    pub result_numerator: i128,
    /// Positive exact result denominator.
    pub result_denominator: u64,
    /// Positive provider publication timestamp.
    pub observed_at: u64,
}

impl ResolutionCertificateV1 {
    /// Decode one exact canonical successful certificate.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_width(input, RESOLUTION_CERTIFICATE_BYTES)?;
        exact(
            input,
            generated_source_resolution::CERTIFICATE_MAGIC_OFFSET,
            &generated_source_resolution::CERTIFICATE_MAGIC_BYTES,
            Error::InvalidMagic,
        )?;
        if u16_at(
            input,
            generated_source_resolution::CERTIFICATE_VERSION_OFFSET,
        )? != generated_source_resolution::CERTIFICATE_ABI_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        if byte_at(input, generated_source_resolution::CERTIFICATE_KIND_OFFSET)?
            != generated_source_resolution::CERTIFICATE_RESOLUTION_SUCCESS_KIND
        {
            return Err(Error::UnknownAction);
        }
        require_zero(
            input,
            generated_source_resolution::CERTIFICATE_RESERVED_HEADER_OFFSET,
            5,
        )?;
        require_zero(
            input,
            generated_source_resolution::CERTIFICATE_RESERVED_BODY_OFFSET,
            4,
        )?;
        let value = Self {
            market: array_at(
                input,
                generated_source_resolution::CERTIFICATE_MARKET_OFFSET,
            )?,
            route: array_at(input, generated_source_resolution::CERTIFICATE_ROUTE_OFFSET)?,
            source_material: array_at(
                input,
                generated_source_resolution::CERTIFICATE_SOURCE_MATERIAL_OFFSET,
            )?,
            product: array_at(
                input,
                generated_source_resolution::CERTIFICATE_PRODUCT_OFFSET,
            )?,
            provider_evidence: array_at(
                input,
                generated_source_resolution::CERTIFICATE_PROVIDER_EVIDENCE_OFFSET,
            )?,
            funding_allocation: array_at(
                input,
                generated_source_resolution::CERTIFICATE_FUNDING_ALLOCATION_OFFSET,
            )?,
            receipt_account: array_at(
                input,
                generated_source_resolution::CERTIFICATE_RECEIPT_ACCOUNT_OFFSET,
            )?,
            generation: u64_at(
                input,
                generated_source_resolution::CERTIFICATE_GENERATION_OFFSET,
            )?,
            attempt_index: u32_at(
                input,
                generated_source_resolution::CERTIFICATE_ATTEMPT_INDEX_OFFSET,
            )?,
            schedule_index: u32_at(
                input,
                generated_source_resolution::CERTIFICATE_SCHEDULE_INDEX_OFFSET,
            )?,
            selector: u32_at(
                input,
                generated_source_resolution::CERTIFICATE_SELECTOR_OFFSET,
            )?,
            work_paid: u64_at(
                input,
                generated_source_resolution::CERTIFICATE_WORK_PAID_OFFSET,
            )?,
            funding_remaining: u64_at(
                input,
                generated_source_resolution::CERTIFICATE_FUNDING_REMAINING_OFFSET,
            )?,
            result_numerator: i128_at(
                input,
                generated_source_resolution::CERTIFICATE_RESULT_NUMERATOR_OFFSET,
            )?,
            result_denominator: u64_at(
                input,
                generated_source_resolution::CERTIFICATE_RESULT_DENOMINATOR_OFFSET,
            )?,
            observed_at: u64_at(
                input,
                generated_source_resolution::CERTIFICATE_OBSERVED_AT_OFFSET,
            )?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical successful certificate.
    pub fn to_bytes(self) -> Result<[u8; RESOLUTION_CERTIFICATE_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; RESOLUTION_CERTIFICATE_BYTES];
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_MAGIC_OFFSET,
            &generated_source_resolution::CERTIFICATE_MAGIC_BYTES,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_VERSION_OFFSET,
            &generated_source_resolution::CERTIFICATE_ABI_VERSION.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_KIND_OFFSET,
            &[generated_source_resolution::CERTIFICATE_RESOLUTION_SUCCESS_KIND],
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_MARKET_OFFSET,
            &self.market,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_ROUTE_OFFSET,
            &self.route,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_SOURCE_MATERIAL_OFFSET,
            &self.source_material,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_PRODUCT_OFFSET,
            &self.product,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_PROVIDER_EVIDENCE_OFFSET,
            &self.provider_evidence,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_FUNDING_ALLOCATION_OFFSET,
            &self.funding_allocation,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_RECEIPT_ACCOUNT_OFFSET,
            &self.receipt_account,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_ATTEMPT_INDEX_OFFSET,
            &self.attempt_index.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_SCHEDULE_INDEX_OFFSET,
            &self.schedule_index.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_SELECTOR_OFFSET,
            &self.selector.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_WORK_PAID_OFFSET,
            &self.work_paid.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_FUNDING_REMAINING_OFFSET,
            &self.funding_remaining.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_RESULT_NUMERATOR_OFFSET,
            &self.result_numerator.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_RESULT_DENOMINATOR_OFFSET,
            &self.result_denominator.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_OBSERVED_AT_OFFSET,
            &self.observed_at.to_le_bytes(),
        )?;
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        if is_zero(&self.market)
            || is_zero(&self.route)
            || is_zero(&self.source_material)
            || is_zero(&self.product)
            || is_zero(&self.provider_evidence)
            || is_zero(&self.receipt_account)
            || self.generation == 0
            || self.result_denominator == 0
            || self.observed_at == 0
        {
            return Err(Error::ZeroCoordinate);
        }
        if self.selector > u32::from(u8::MAX) {
            return Err(Error::InvalidSelector);
        }
        Ok(())
    }
}

fn exact_width(input: &[u8], expected: usize) -> Result<()> {
    if input.len() == expected {
        Ok(())
    } else {
        Err(Error::InvalidLength)
    }
}

fn exact(input: &[u8], offset: usize, expected: &[u8], error: Error) -> Result<()> {
    let end = offset
        .checked_add(expected.len())
        .ok_or(Error::InvalidLength)?;
    if input.get(offset..end) == Some(expected) {
        Ok(())
    } else {
        Err(error)
    }
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    if input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0)
    {
        Ok(())
    } else {
        Err(Error::NonCanonicalReserved)
    }
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn byte_at(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array_at<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array_at(input, offset)?))
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array_at(input, offset)?))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array_at(input, offset)?))
}

fn i128_at(input: &[u8], offset: usize) -> Result<i128> {
    Ok(i128::from_le_bytes(array_at(input, offset)?))
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> AcceptPythRequestV1 {
        AcceptPythRequestV1 {
            expected_generation: 7,
            expected_result_domain_id: {
                let mut value = [0_u8; 32];
                value[0] = 0x22;
                value[1] = 0x11;
                value
            },
            expected_provider_release_id: {
                let mut value = [0_u8; 32];
                value[0] = 0x44;
                value[1] = 0x33;
                value
            },
        }
    }

    fn certificate() -> ResolutionCertificateV1 {
        ResolutionCertificateV1 {
            market: [1; 32],
            route: [2; 32],
            source_material: [3; 32],
            product: [4; 32],
            provider_evidence: [5; 32],
            funding_allocation: [0; 32],
            receipt_account: [6; 32],
            generation: 7,
            attempt_index: 0,
            schedule_index: 0,
            selector: 2,
            work_paid: 0,
            funding_remaining: 0,
            result_numerator: -123_456_789,
            result_denominator: 1,
            observed_at: 1_700_000_000,
        }
    }

    #[test]
    fn generated_request_vector_and_round_trip_match() -> Result<()> {
        let encoded = request().to_bytes()?;
        assert_eq!(encoded, generated_source_resolution::REQUEST_EXAMPLE);
        assert_eq!(AcceptPythRequestV1::decode(&encoded), Ok(request()));
        Ok(())
    }

    #[test]
    fn every_request_truncation_and_hostile_header_refuses() -> Result<()> {
        let encoded = request().to_bytes()?;
        for length in 0..ACCEPT_PYTH_REQUEST_BYTES {
            assert_eq!(
                AcceptPythRequestV1::decode(encoded.get(..length).ok_or(Error::InvalidLength)?),
                Err(Error::InvalidLength)
            );
        }
        let mut long = [0_u8; ACCEPT_PYTH_REQUEST_BYTES + 1];
        long.get_mut(..ACCEPT_PYTH_REQUEST_BYTES)
            .ok_or(Error::InvalidLength)?
            .copy_from_slice(&encoded);
        assert_eq!(
            AcceptPythRequestV1::decode(&long),
            Err(Error::InvalidLength)
        );

        for (offset, error) in [
            (
                generated_source_resolution::REQUEST_MAGIC_OFFSET,
                Error::InvalidMagic,
            ),
            (
                generated_source_resolution::REQUEST_VERSION_OFFSET,
                Error::UnsupportedVersion,
            ),
            (
                generated_source_resolution::REQUEST_ACTION_OFFSET,
                Error::UnknownAction,
            ),
            (
                generated_source_resolution::REQUEST_RESERVED_OFFSET,
                Error::NonCanonicalReserved,
            ),
        ] {
            let mut hostile = encoded;
            *hostile.get_mut(offset).ok_or(Error::InvalidLength)? ^= 1;
            assert_eq!(AcceptPythRequestV1::decode(&hostile), Err(error));
        }
        Ok(())
    }

    #[test]
    fn certificate_round_trip_is_exact_and_reserved_bytes_are_hostile() -> Result<()> {
        let value = certificate();
        let encoded = value.to_bytes()?;
        assert_eq!(encoded.len(), RESOLUTION_CERTIFICATE_BYTES);
        assert_eq!(ResolutionCertificateV1::decode(&encoded), Ok(value));
        assert!(encoded[11..16].iter().all(|byte| *byte == 0));
        assert!(encoded[260..264].iter().all(|byte| *byte == 0));

        let mut hostile = encoded;
        hostile[generated_source_resolution::CERTIFICATE_RESERVED_BODY_OFFSET] = 1;
        assert_eq!(
            ResolutionCertificateV1::decode(&hostile),
            Err(Error::NonCanonicalReserved)
        );
        Ok(())
    }

    #[test]
    fn release_candidate_is_not_an_implicit_zero_authority() {
        assert_eq!(
            ResolutionReleaseCandidateV1::new([0; 32], [1; 32]),
            Err(Error::ZeroCoordinate)
        );
        assert_eq!(
            ResolutionReleaseCandidateV1::new([1; 32], [0; 32]),
            Err(Error::ZeroCoordinate)
        );
        assert!(ResolutionReleaseCandidateV1::new([1; 32], [2; 32]).is_ok());
    }
}
