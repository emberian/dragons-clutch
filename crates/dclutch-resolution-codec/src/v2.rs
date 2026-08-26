//! Runtime-width direct request and terminal certificate codecs.

use super::{
    Error, Result, array_at, byte_at, exact, exact_width, generated_v2 as generated, i128_at,
    is_zero, put, require_zero, u16_at, u32_at, u64_at,
};

/// Optimistic concurrency coordinates for one Runtime V2 primary-Pyth admission.
///
/// Product authority is intentionally absent. The controller derives the exact
/// Product-record root from authenticated `SourceMaterialV2` and authenticates
/// its selected Runtime V2 graph independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptPythRequestV2 {
    /// Exact immutable Market generation expected by the submitter.
    pub expected_generation: u64,
    /// Exact Pyth deployment-release content identity selected by Source.
    pub expected_provider_release_id: [u8; 32],
}

impl AcceptPythRequestV2 {
    /// Hostile-decode one exact canonical request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_width(input, generated::ACCEPT_PYTH_REQUEST_BYTES_V2)?;
        exact(
            input,
            generated::ACCEPT_PYTH_V2_MAGIC_OFFSET,
            &generated::ACCEPT_PYTH_V2_MAGIC,
            Error::InvalidMagic,
        )?;
        if u16_at(input, generated::ACCEPT_PYTH_V2_VERSION_OFFSET)?
            != generated::ACCEPT_PYTH_V2_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        if byte_at(input, generated::ACCEPT_PYTH_V2_ACTION_OFFSET)?
            != generated::ACCEPT_PYTH_V2_ACTION
        {
            return Err(Error::UnknownAction);
        }
        require_zero(input, generated::ACCEPT_PYTH_V2_RESERVED_OFFSET, 5)?;
        let value = Self {
            expected_generation: u64_at(
                input,
                generated::ACCEPT_PYTH_V2_EXPECTED_GENERATION_OFFSET,
            )?,
            expected_provider_release_id: array_at(
                input,
                generated::ACCEPT_PYTH_V2_EXPECTED_PROVIDER_RELEASE_OFFSET,
            )?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical request.
    pub fn to_bytes(self) -> Result<[u8; generated::ACCEPT_PYTH_REQUEST_BYTES_V2]> {
        self.validate()?;
        let mut output = [0_u8; generated::ACCEPT_PYTH_REQUEST_BYTES_V2];
        put(
            &mut output,
            generated::ACCEPT_PYTH_V2_MAGIC_OFFSET,
            &generated::ACCEPT_PYTH_V2_MAGIC,
        )?;
        put(
            &mut output,
            generated::ACCEPT_PYTH_V2_VERSION_OFFSET,
            &generated::ACCEPT_PYTH_V2_VERSION.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::ACCEPT_PYTH_V2_ACTION_OFFSET,
            &[generated::ACCEPT_PYTH_V2_ACTION],
        )?;
        put(
            &mut output,
            generated::ACCEPT_PYTH_V2_EXPECTED_GENERATION_OFFSET,
            &self.expected_generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::ACCEPT_PYTH_V2_EXPECTED_PROVIDER_RELEASE_OFFSET,
            &self.expected_provider_release_id,
        )?;
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        if self.expected_generation == 0 || is_zero(&self.expected_provider_release_id) {
            return Err(Error::ZeroCoordinate);
        }
        Ok(())
    }
}

/// Lean-owned kind of one Runtime V2 Source certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionCertificateKindV2 {
    /// Provider evidence resolved one ordinary Product cell.
    ResolutionSuccess,
    /// Funding was consumed and the next recovery became active.
    RecoveryAdvanced,
    /// Funding was consumed and the finite recovery sequence exhausted.
    Exhausted,
    /// Funding was consumed and Product's explicit failure cell was committed.
    ResolutionFailure,
}

impl ResolutionCertificateKindV2 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            generated::RESOLUTION_CERTIFICATE_SUCCESS_KIND_V2 => Ok(Self::ResolutionSuccess),
            generated::RESOLUTION_CERTIFICATE_RECOVERY_ADVANCED_KIND_V2 => {
                Ok(Self::RecoveryAdvanced)
            }
            generated::RESOLUTION_CERTIFICATE_EXHAUSTED_KIND_V2 => Ok(Self::Exhausted),
            generated::RESOLUTION_CERTIFICATE_FAILURE_KIND_V2 => Ok(Self::ResolutionFailure),
            _ => Err(Error::UnknownAction),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::ResolutionSuccess => generated::RESOLUTION_CERTIFICATE_SUCCESS_KIND_V2,
            Self::RecoveryAdvanced => generated::RESOLUTION_CERTIFICATE_RECOVERY_ADVANCED_KIND_V2,
            Self::Exhausted => generated::RESOLUTION_CERTIFICATE_EXHAUSTED_KIND_V2,
            Self::ResolutionFailure => generated::RESOLUTION_CERTIFICATE_FAILURE_KIND_V2,
        }
    }
}

/// Canonical Runtime V2 Source certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionCertificateV2 {
    /// Exact Lean-owned certificate kind.
    pub kind: ResolutionCertificateKindV2,
    /// Canonical Market account identity.
    pub market: [u8; 32],
    /// Active provider/recovery route; explicit failure uses all zeroes.
    pub route: [u8; 32],
    /// Exact `SourceMaterialV2` content digest.
    pub source_material: [u8; 32],
    /// Exact Product Runtime V2 Product-record content digest.
    pub product_record_digest: [u8; 32],
    /// Authenticated provider evidence; liveness transitions use all zeroes.
    pub provider_evidence: [u8; 32],
    /// Exact funding-allocation identity; unfunded primary uses all zeroes.
    pub funding_allocation: [u8; 32],
    /// Canonical certificate account identity.
    pub receipt_account: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Ordered recovery index; primary admission uses zero.
    pub attempt_index: u32,
    /// Exact Source schedule index.
    pub schedule_index: u32,
    /// Native Product Runtime V2 selector without truncation.
    pub selector: u32,
    /// Exact work paid by this transition.
    pub work_paid: u64,
    /// Authenticated remaining work funding.
    pub funding_remaining: u64,
    /// Exact signed normalized result numerator.
    pub result_numerator: i128,
    /// Positive denominator for success; liveness transitions use zero.
    pub result_denominator: u64,
    /// Provider publication or recovery-transition time; failure uses zero.
    pub observed_at: u64,
}

impl ResolutionCertificateV2 {
    /// Hostile-decode one exact canonical certificate.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_width(input, generated::RESOLUTION_CERTIFICATE_BYTES_V2)?;
        exact(
            input,
            generated::CERTIFICATE_V2_MAGIC_OFFSET,
            &generated::RESOLUTION_CERTIFICATE_MAGIC_V2,
            Error::InvalidMagic,
        )?;
        if u16_at(input, generated::CERTIFICATE_V2_VERSION_OFFSET)?
            != generated::RESOLUTION_CERTIFICATE_VERSION_V2
        {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, generated::CERTIFICATE_V2_RESERVED_HEADER_OFFSET, 5)?;
        require_zero(input, generated::CERTIFICATE_V2_RESERVED_BODY_OFFSET, 4)?;
        let value = Self {
            kind: ResolutionCertificateKindV2::decode(byte_at(
                input,
                generated::CERTIFICATE_V2_KIND_OFFSET,
            )?)?,
            market: array_at(input, generated::CERTIFICATE_V2_MARKET_OFFSET)?,
            route: array_at(input, generated::CERTIFICATE_V2_ROUTE_OFFSET)?,
            source_material: array_at(input, generated::CERTIFICATE_V2_SOURCE_MATERIAL_OFFSET)?,
            product_record_digest: array_at(
                input,
                generated::CERTIFICATE_V2_PRODUCT_RECORD_OFFSET,
            )?,
            provider_evidence: array_at(input, generated::CERTIFICATE_V2_PROVIDER_EVIDENCE_OFFSET)?,
            funding_allocation: array_at(
                input,
                generated::CERTIFICATE_V2_FUNDING_ALLOCATION_OFFSET,
            )?,
            receipt_account: array_at(input, generated::CERTIFICATE_V2_RECEIPT_ACCOUNT_OFFSET)?,
            generation: u64_at(input, generated::CERTIFICATE_V2_GENERATION_OFFSET)?,
            attempt_index: u32_at(input, generated::CERTIFICATE_V2_ATTEMPT_INDEX_OFFSET)?,
            schedule_index: u32_at(input, generated::CERTIFICATE_V2_SCHEDULE_INDEX_OFFSET)?,
            selector: u32_at(input, generated::CERTIFICATE_V2_SELECTOR_OFFSET)?,
            work_paid: u64_at(input, generated::CERTIFICATE_V2_WORK_PAID_OFFSET)?,
            funding_remaining: u64_at(input, generated::CERTIFICATE_V2_FUNDING_REMAINING_OFFSET)?,
            result_numerator: i128_at(input, generated::CERTIFICATE_V2_RESULT_NUMERATOR_OFFSET)?,
            result_denominator: u64_at(input, generated::CERTIFICATE_V2_RESULT_DENOMINATOR_OFFSET)?,
            observed_at: u64_at(input, generated::CERTIFICATE_V2_OBSERVED_AT_OFFSET)?,
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Encode one exact canonical certificate.
    pub fn to_bytes(self) -> Result<[u8; generated::RESOLUTION_CERTIFICATE_BYTES_V2]> {
        self.validate_shape()?;
        let mut output = [0_u8; generated::RESOLUTION_CERTIFICATE_BYTES_V2];
        put(
            &mut output,
            generated::CERTIFICATE_V2_MAGIC_OFFSET,
            &generated::RESOLUTION_CERTIFICATE_MAGIC_V2,
        )?;
        put(
            &mut output,
            generated::CERTIFICATE_V2_VERSION_OFFSET,
            &generated::RESOLUTION_CERTIFICATE_VERSION_V2.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CERTIFICATE_V2_KIND_OFFSET,
            &[self.kind.byte()],
        )?;
        for (offset, bytes) in [
            (generated::CERTIFICATE_V2_MARKET_OFFSET, &self.market),
            (generated::CERTIFICATE_V2_ROUTE_OFFSET, &self.route),
            (
                generated::CERTIFICATE_V2_SOURCE_MATERIAL_OFFSET,
                &self.source_material,
            ),
            (
                generated::CERTIFICATE_V2_PRODUCT_RECORD_OFFSET,
                &self.product_record_digest,
            ),
            (
                generated::CERTIFICATE_V2_PROVIDER_EVIDENCE_OFFSET,
                &self.provider_evidence,
            ),
            (
                generated::CERTIFICATE_V2_FUNDING_ALLOCATION_OFFSET,
                &self.funding_allocation,
            ),
            (
                generated::CERTIFICATE_V2_RECEIPT_ACCOUNT_OFFSET,
                &self.receipt_account,
            ),
        ] {
            put(&mut output, offset, bytes)?;
        }
        put(
            &mut output,
            generated::CERTIFICATE_V2_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CERTIFICATE_V2_ATTEMPT_INDEX_OFFSET,
            &self.attempt_index.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CERTIFICATE_V2_SCHEDULE_INDEX_OFFSET,
            &self.schedule_index.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CERTIFICATE_V2_SELECTOR_OFFSET,
            &self.selector.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CERTIFICATE_V2_WORK_PAID_OFFSET,
            &self.work_paid.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CERTIFICATE_V2_FUNDING_REMAINING_OFFSET,
            &self.funding_remaining.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CERTIFICATE_V2_RESULT_NUMERATOR_OFFSET,
            &self.result_numerator.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CERTIFICATE_V2_RESULT_DENOMINATOR_OFFSET,
            &self.result_denominator.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CERTIFICATE_V2_OBSERVED_AT_OFFSET,
            &self.observed_at.to_le_bytes(),
        )?;
        Ok(output)
    }

    /// Join a terminal certificate to the independently authenticated Product
    /// graph root and outcome count. Ordinary success may not select the final
    /// failure cell; explicit failure must select exactly that final cell.
    pub fn validate_terminal_product(
        self,
        authenticated_product_record_digest: [u8; 32],
        authenticated_outcome_count: u32,
    ) -> Result<()> {
        if self.product_record_digest != authenticated_product_record_digest {
            return Err(Error::ProductAuthorityMismatch);
        }
        let failure_selector = authenticated_outcome_count
            .checked_sub(1)
            .filter(|_| authenticated_outcome_count >= 2)
            .ok_or(Error::InvalidSelector)?;
        match self.kind {
            ResolutionCertificateKindV2::ResolutionSuccess if self.selector < failure_selector => {
                Ok(())
            }
            ResolutionCertificateKindV2::ResolutionFailure if self.selector == failure_selector => {
                Ok(())
            }
            ResolutionCertificateKindV2::ResolutionSuccess
            | ResolutionCertificateKindV2::ResolutionFailure => Err(Error::InvalidSelector),
            ResolutionCertificateKindV2::RecoveryAdvanced
            | ResolutionCertificateKindV2::Exhausted => Err(Error::InvalidReceiptShape),
        }
    }

    /// Exact Product Runtime V2 Product-record content digest.
    pub const fn product_record_digest(self) -> [u8; 32] {
        self.product_record_digest
    }

    /// Native runtime-width Product selector.
    pub const fn selector(self) -> u32 {
        self.selector
    }

    fn validate_shape(self) -> Result<()> {
        if is_zero(&self.market)
            || is_zero(&self.source_material)
            || is_zero(&self.product_record_digest)
            || is_zero(&self.receipt_account)
            || self.generation == 0
        {
            return Err(Error::ZeroCoordinate);
        }
        match self.kind {
            ResolutionCertificateKindV2::ResolutionSuccess => {
                if is_zero(&self.route)
                    || is_zero(&self.provider_evidence)
                    || self.result_denominator == 0
                    || self.observed_at == 0
                {
                    return Err(Error::ZeroCoordinate);
                }
            }
            ResolutionCertificateKindV2::RecoveryAdvanced
            | ResolutionCertificateKindV2::Exhausted => {
                if is_zero(&self.route)
                    || is_zero(&self.funding_allocation)
                    || !is_zero(&self.provider_evidence)
                    || self.selector != 0
                    || self.work_paid == 0
                    || self.result_numerator != 0
                    || self.result_denominator != 0
                    || self.observed_at == 0
                {
                    return Err(Error::ZeroCoordinate);
                }
            }
            ResolutionCertificateKindV2::ResolutionFailure => {
                if !is_zero(&self.route)
                    || is_zero(&self.funding_allocation)
                    || !is_zero(&self.provider_evidence)
                    || self.work_paid == 0
                    || self.schedule_index != 0
                    || self.result_numerator != 0
                    || self.result_denominator != 0
                    || self.observed_at != 0
                {
                    return Err(Error::ZeroCoordinate);
                }
            }
        }
        Ok(())
    }
}

/// Persisted Runtime V2 receipt proving terminal Source state and the exact
/// three funding compartments were atomically discharged.
///
/// The layout remains 384 bytes, but V2 has distinct magic/version semantics
/// and preserves the native `u32` Product selector without a legacy `u8` cap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceClosureReceiptV2 {
    /// Canonical Market identity.
    pub market: [u8; 32],
    /// Canonical Runtime V2 Source state account that was closed.
    pub source_state: [u8; 32],
    /// Exact `SourceMaterialV2` content digest.
    pub source_material: [u8; 32],
    /// Finalized capability-manifest content identity.
    pub capability_manifest: [u8; 32],
    /// Authenticated Runtime V2 terminal Resolution certificate account.
    pub terminal_certificate: [u8; 32],
    /// This deterministic V2 closure receipt account.
    pub receipt_account: [u8; 32],
    /// Exact beneficiary receiving all discharged lamports.
    pub beneficiary: [u8; 32],
    /// Digest of the authenticated terminal Runtime V2 Source pre-state.
    pub source_state_digest: [u8; 32],
    /// Digest of the authenticated Runtime V2 terminal certificate bytes.
    pub terminal_certificate_digest: [u8; 32],
    /// Digest of the exact ordered three-compartment funding pre-state.
    pub funding_set_digest: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact terminal certificate sequence.
    pub terminal_sequence: u64,
    /// Native Product Runtime V2 terminal selector.
    pub selector: u32,
    /// Exact Source and funding lamports discharged to `beneficiary`.
    pub refund_lamports: u64,
    /// Clock timestamp at which the atomic discharge committed.
    pub closed_at: u64,
}

impl SourceClosureReceiptV2 {
    /// Hostile-decode one exact canonical Runtime V2 Source closure receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_width(input, generated::SOURCE_CLOSURE_RECEIPT_BYTES_V2)?;
        exact(
            input,
            generated::CLOSURE_V2_MAGIC_OFFSET,
            &generated::SOURCE_CLOSURE_RECEIPT_MAGIC_V2,
            Error::InvalidMagic,
        )?;
        if u16_at(input, generated::CLOSURE_V2_VERSION_OFFSET)?
            != generated::SOURCE_CLOSURE_RECEIPT_VERSION_V2
        {
            return Err(Error::UnsupportedVersion);
        }
        if byte_at(input, generated::CLOSURE_V2_KIND_OFFSET)?
            != generated::SOURCE_CLOSURE_RECEIPT_KIND_V2
        {
            return Err(Error::InvalidReceiptShape);
        }
        require_zero(input, generated::CLOSURE_V2_RESERVED_HEADER_OFFSET, 5)?;
        if u32_at(input, generated::CLOSURE_V2_FUNDING_COUNT_OFFSET)?
            != generated::SOURCE_CLOSURE_FUNDING_COUNT_V2
        {
            return Err(Error::InvalidFundingCount);
        }
        require_zero(input, generated::CLOSURE_V2_RESERVED_BODY_OFFSET, 8)?;
        let value = Self {
            market: array_at(input, generated::CLOSURE_V2_MARKET_OFFSET)?,
            source_state: array_at(input, generated::CLOSURE_V2_SOURCE_STATE_OFFSET)?,
            source_material: array_at(input, generated::CLOSURE_V2_SOURCE_MATERIAL_OFFSET)?,
            capability_manifest: array_at(input, generated::CLOSURE_V2_CAPABILITY_MANIFEST_OFFSET)?,
            terminal_certificate: array_at(
                input,
                generated::CLOSURE_V2_TERMINAL_CERTIFICATE_OFFSET,
            )?,
            receipt_account: array_at(input, generated::CLOSURE_V2_RECEIPT_ACCOUNT_OFFSET)?,
            beneficiary: array_at(input, generated::CLOSURE_V2_BENEFICIARY_OFFSET)?,
            source_state_digest: array_at(input, generated::CLOSURE_V2_SOURCE_STATE_DIGEST_OFFSET)?,
            terminal_certificate_digest: array_at(
                input,
                generated::CLOSURE_V2_TERMINAL_CERTIFICATE_DIGEST_OFFSET,
            )?,
            funding_set_digest: array_at(input, generated::CLOSURE_V2_FUNDING_SET_DIGEST_OFFSET)?,
            generation: u64_at(input, generated::CLOSURE_V2_GENERATION_OFFSET)?,
            terminal_sequence: u64_at(input, generated::CLOSURE_V2_TERMINAL_SEQUENCE_OFFSET)?,
            selector: u32_at(input, generated::CLOSURE_V2_SELECTOR_OFFSET)?,
            refund_lamports: u64_at(input, generated::CLOSURE_V2_REFUND_LAMPORTS_OFFSET)?,
            closed_at: u64_at(input, generated::CLOSURE_V2_CLOSED_AT_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical Runtime V2 Source closure receipt.
    pub fn to_bytes(self) -> Result<[u8; generated::SOURCE_CLOSURE_RECEIPT_BYTES_V2]> {
        self.validate()?;
        let mut output = [0_u8; generated::SOURCE_CLOSURE_RECEIPT_BYTES_V2];
        put(
            &mut output,
            generated::CLOSURE_V2_MAGIC_OFFSET,
            &generated::SOURCE_CLOSURE_RECEIPT_MAGIC_V2,
        )?;
        put(
            &mut output,
            generated::CLOSURE_V2_VERSION_OFFSET,
            &generated::SOURCE_CLOSURE_RECEIPT_VERSION_V2.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CLOSURE_V2_KIND_OFFSET,
            &[generated::SOURCE_CLOSURE_RECEIPT_KIND_V2],
        )?;
        for (offset, value) in [
            (generated::CLOSURE_V2_MARKET_OFFSET, &self.market),
            (
                generated::CLOSURE_V2_SOURCE_STATE_OFFSET,
                &self.source_state,
            ),
            (
                generated::CLOSURE_V2_SOURCE_MATERIAL_OFFSET,
                &self.source_material,
            ),
            (
                generated::CLOSURE_V2_CAPABILITY_MANIFEST_OFFSET,
                &self.capability_manifest,
            ),
            (
                generated::CLOSURE_V2_TERMINAL_CERTIFICATE_OFFSET,
                &self.terminal_certificate,
            ),
            (
                generated::CLOSURE_V2_RECEIPT_ACCOUNT_OFFSET,
                &self.receipt_account,
            ),
            (generated::CLOSURE_V2_BENEFICIARY_OFFSET, &self.beneficiary),
            (
                generated::CLOSURE_V2_SOURCE_STATE_DIGEST_OFFSET,
                &self.source_state_digest,
            ),
            (
                generated::CLOSURE_V2_TERMINAL_CERTIFICATE_DIGEST_OFFSET,
                &self.terminal_certificate_digest,
            ),
            (
                generated::CLOSURE_V2_FUNDING_SET_DIGEST_OFFSET,
                &self.funding_set_digest,
            ),
        ] {
            put(&mut output, offset, value)?;
        }
        put(
            &mut output,
            generated::CLOSURE_V2_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CLOSURE_V2_TERMINAL_SEQUENCE_OFFSET,
            &self.terminal_sequence.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CLOSURE_V2_FUNDING_COUNT_OFFSET,
            &generated::SOURCE_CLOSURE_FUNDING_COUNT_V2.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CLOSURE_V2_SELECTOR_OFFSET,
            &self.selector.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CLOSURE_V2_REFUND_LAMPORTS_OFFSET,
            &self.refund_lamports.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CLOSURE_V2_CLOSED_AT_OFFSET,
            &self.closed_at.to_le_bytes(),
        )?;
        Ok(output)
    }

    /// Native runtime-width Product selector without truncation.
    pub const fn selector(self) -> u32 {
        self.selector
    }

    fn validate(self) -> Result<()> {
        if is_zero(&self.market)
            || is_zero(&self.source_state)
            || is_zero(&self.source_material)
            || is_zero(&self.capability_manifest)
            || is_zero(&self.terminal_certificate)
            || is_zero(&self.receipt_account)
            || is_zero(&self.beneficiary)
            || is_zero(&self.source_state_digest)
            || is_zero(&self.terminal_certificate_digest)
            || is_zero(&self.funding_set_digest)
            || self.generation == 0
            || self.terminal_sequence == 0
            || self.refund_lamports == 0
            || self.closed_at == 0
        {
            return Err(Error::ZeroCoordinate);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_v2::{
        ACCEPT_PYTH_REQUEST_V2_EXAMPLE, ACCEPT_PYTH_REQUEST_V2_REFUSAL_CORPUS,
        ACCEPT_PYTH_REQUEST_V2_REFUSAL_COUNT, RESOLUTION_CERTIFICATE_V2_REFUSAL_CORPUS,
        RESOLUTION_CERTIFICATE_V2_REFUSAL_COUNT, RESOLUTION_CERTIFICATE_V2_WIDE_FAILURE_EXAMPLE,
        RESOLUTION_CERTIFICATE_V2_WIDE_SUCCESS_EXAMPLE, SOURCE_CLOSURE_RECEIPT_V2_REFUSAL_CORPUS,
        SOURCE_CLOSURE_RECEIPT_V2_REFUSAL_COUNT, SOURCE_CLOSURE_RECEIPT_V2_WIDE_EXAMPLE,
    };

    fn id(tag: u8) -> [u8; 32] {
        let mut output = [0_u8; 32];
        output[0] = tag;
        output
    }

    #[test]
    fn generated_request_and_refusals_agree() {
        let expected = AcceptPythRequestV2 {
            expected_generation: 9,
            expected_provider_release_id: id(1),
        };
        assert_eq!(expected.to_bytes(), Ok(ACCEPT_PYTH_REQUEST_V2_EXAMPLE));
        assert_eq!(
            AcceptPythRequestV2::decode(&ACCEPT_PYTH_REQUEST_V2_EXAMPLE),
            Ok(expected)
        );
        assert_eq!(
            ACCEPT_PYTH_REQUEST_V2_REFUSAL_CORPUS.len(),
            ACCEPT_PYTH_REQUEST_V2_REFUSAL_COUNT
        );
        for hostile in ACCEPT_PYTH_REQUEST_V2_REFUSAL_CORPUS {
            assert!(AcceptPythRequestV2::decode(&hostile).is_err());
        }
    }

    #[test]
    fn generated_wide_certificates_agree_without_u8_ceiling() {
        let success =
            ResolutionCertificateV2::decode(&RESOLUTION_CERTIFICATE_V2_WIDE_SUCCESS_EXAMPLE)
                .expect("success");
        assert_eq!(success.selector(), 257);
        assert_eq!(
            success.to_bytes(),
            Ok(RESOLUTION_CERTIFICATE_V2_WIDE_SUCCESS_EXAMPLE)
        );
        assert_eq!(success.validate_terminal_product(id(4), 259), Ok(()));
        assert_eq!(
            success.validate_terminal_product(id(4), 258),
            Err(Error::InvalidSelector)
        );
        assert_eq!(
            success.validate_terminal_product(id(9), 259),
            Err(Error::ProductAuthorityMismatch)
        );

        let failure =
            ResolutionCertificateV2::decode(&RESOLUTION_CERTIFICATE_V2_WIDE_FAILURE_EXAMPLE)
                .expect("failure");
        assert_eq!(failure.selector(), 257);
        assert_eq!(failure.validate_terminal_product(id(4), 258), Ok(()));
        assert_eq!(
            failure.validate_terminal_product(id(4), 259),
            Err(Error::InvalidSelector)
        );
    }

    #[test]
    fn generated_certificate_refusal_corpus_fails_closed() {
        assert_eq!(
            RESOLUTION_CERTIFICATE_V2_REFUSAL_CORPUS.len(),
            RESOLUTION_CERTIFICATE_V2_REFUSAL_COUNT
        );
        for hostile in RESOLUTION_CERTIFICATE_V2_REFUSAL_CORPUS {
            assert!(ResolutionCertificateV2::decode(&hostile).is_err());
        }
    }

    #[test]
    fn liveness_certificate_cannot_smuggle_a_selector() {
        let mut bytes = RESOLUTION_CERTIFICATE_V2_WIDE_SUCCESS_EXAMPLE;
        bytes[generated::CERTIFICATE_V2_KIND_OFFSET] =
            generated::RESOLUTION_CERTIFICATE_RECOVERY_ADVANCED_KIND_V2;
        bytes[generated::CERTIFICATE_V2_PROVIDER_EVIDENCE_OFFSET..]
            .iter_mut()
            .take(32)
            .for_each(|byte| *byte = 0);
        bytes[generated::CERTIFICATE_V2_FUNDING_ALLOCATION_OFFSET] = 7;
        bytes[generated::CERTIFICATE_V2_WORK_PAID_OFFSET] = 1;
        assert_eq!(
            ResolutionCertificateV2::decode(&bytes),
            Err(Error::ZeroCoordinate)
        );
    }

    #[test]
    fn generated_wide_closure_preserves_native_u32_selector() {
        let closure = SourceClosureReceiptV2::decode(&SOURCE_CLOSURE_RECEIPT_V2_WIDE_EXAMPLE)
            .expect("closure");
        assert_eq!(closure.selector(), 257);
        assert_eq!(
            closure.to_bytes(),
            Ok(SOURCE_CLOSURE_RECEIPT_V2_WIDE_EXAMPLE)
        );
    }

    #[test]
    fn generated_closure_refusal_corpus_fails_closed() {
        assert_eq!(
            SOURCE_CLOSURE_RECEIPT_V2_REFUSAL_CORPUS.len(),
            SOURCE_CLOSURE_RECEIPT_V2_REFUSAL_COUNT
        );
        for hostile in SOURCE_CLOSURE_RECEIPT_V2_REFUSAL_CORPUS {
            assert!(SourceClosureReceiptV2::decode(&hostile).is_err());
        }
    }
}
