//! Exact-N rational portfolio recipes over elementary categorical claims.

use core::convert::TryFrom;

use crate::claim::CategoricalUnitV1;
use crate::{ContentId, Error, Result, array, byte, content_id, put, require_zero};

/// Mathematical lower bound inherited from a liability-bearing partition.
pub const MIN_PORTFOLIO_CLAIMS: usize = 2;
/// Current provisional artifact profile's maximum exact recipe width.
///
/// This is neither a mathematical nor permanent Product limit. Its lifting
/// path is a new exact-width or paged template profile that keeps native
/// categorical liabilities unchanged.
pub const MAX_PORTFOLIO_CLAIMS: usize = 16;
/// Canonical portfolio-template magic.
pub const PORTFOLIO_TEMPLATE_MAGIC: [u8; 8] = *b"DCLTPFT1";
/// Implemented portfolio-template schema version.
pub const PORTFOLIO_TEMPLATE_SCHEMA_VERSION: u16 = 1;
/// Canonical content-identity byte domain for a portfolio-template preimage.
///
/// Hash derivation is an adapter concern; this Product-owned value names the
/// semantic namespace that adapters must supply when identifying this record.
pub const PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1: &[u8] = b"dclutch.portfolio-template.v1";

const CLAIM_BASIS_ID_OFFSET: usize = 16;
const RESULT_DOMAIN_ID_OFFSET: usize = 48;
const DENOMINATOR_OFFSET: usize = 80;
const COEFFICIENTS_OFFSET: usize = 88;

/// Return the checked exact wire width for profile `N`.
pub fn portfolio_template_bytes<const N: usize>() -> Result<usize> {
    validate_width::<N>()?;
    N.checked_mul(core::mem::size_of::<u64>())
        .and_then(|bytes| bytes.checked_add(COEFFICIENTS_OFFSET))
        .ok_or(Error::ArithmeticOverflow)
}

/// A content-addressable, exact rational portfolio recipe over one claim basis.
///
/// `coefficient[i] / denominator` is the nonnegative amount of native claim
/// `i` per one user scale unit. The common representation is uniquely
/// gcd-normalized and contains at least one nonzero coefficient. This is not a
/// liability basis and duplicates no Product or Market state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioTemplateV1<const N: usize> {
    claim_basis_id: ContentId,
    result_domain_id: ContentId,
    denominator: u64,
    coefficients: [u64; N],
}

impl<const N: usize> PortfolioTemplateV1<N> {
    /// Construct and gcd-normalize one nonnegative rational recipe.
    pub fn new(
        claim_basis_id: ContentId,
        result_domain_id: ContentId,
        mut coefficients: [u64; N],
        mut denominator: u64,
    ) -> Result<Self> {
        validate_width::<N>()?;
        if denominator == 0 {
            return Err(Error::ZeroPortfolioDenominator);
        }
        if coefficients.iter().all(|coefficient| *coefficient == 0) {
            return Err(Error::EmptyPortfolioTemplate);
        }
        let divisor = common_divisor(denominator, &coefficients);
        if divisor > 1 {
            denominator = denominator
                .checked_div(divisor)
                .ok_or(Error::ArithmeticOverflow)?;
            for coefficient in &mut coefficients {
                *coefficient = coefficient
                    .checked_div(divisor)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
        }
        Ok(Self {
            claim_basis_id,
            result_domain_id,
            denominator,
            coefficients,
        })
    }

    /// Decode one exact-width canonical recipe, refusing reducible forms.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != portfolio_template_bytes::<N>()? {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != PORTFOLIO_TEMPLATE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != PORTFOLIO_TEMPLATE_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if usize::from(byte(bytes, 10)?) != N {
            return Err(Error::UnsupportedPortfolioWidth);
        }
        require_zero(bytes, 11, 5)?;
        let denominator = u64::from_le_bytes(array(bytes, DENOMINATOR_OFFSET)?);
        if denominator == 0 {
            return Err(Error::ZeroPortfolioDenominator);
        }
        let mut coefficients = [0u64; N];
        for (index, coefficient) in coefficients.iter_mut().enumerate() {
            let offset = index
                .checked_mul(core::mem::size_of::<u64>())
                .and_then(|value| value.checked_add(COEFFICIENTS_OFFSET))
                .ok_or(Error::ArithmeticOverflow)?;
            *coefficient = u64::from_le_bytes(array(bytes, offset)?);
        }
        if coefficients.iter().all(|coefficient| *coefficient == 0) {
            return Err(Error::EmptyPortfolioTemplate);
        }
        if common_divisor(denominator, &coefficients) != 1 {
            return Err(Error::NonCanonicalPortfolioTemplate);
        }
        Ok(Self {
            claim_basis_id: content_id(bytes, CLAIM_BASIS_ID_OFFSET)?,
            result_domain_id: content_id(bytes, RESULT_DOMAIN_ID_OFFSET)?,
            denominator,
            coefficients,
        })
    }

    /// Encode into the one exact caller-provided width without partial writes.
    pub fn encode(self, output: &mut [u8]) -> Result<()> {
        if output.len() != portfolio_template_bytes::<N>()? {
            return Err(Error::OutputLength);
        }
        let width = u8::try_from(N).map_err(|_| Error::UnsupportedPortfolioWidth)?;
        output.fill(0);
        put(output, 0, &PORTFOLIO_TEMPLATE_MAGIC);
        put(output, 8, &PORTFOLIO_TEMPLATE_SCHEMA_VERSION.to_le_bytes());
        put(output, 10, &[width]);
        put(
            output,
            CLAIM_BASIS_ID_OFFSET,
            self.claim_basis_id.as_bytes(),
        );
        put(
            output,
            RESULT_DOMAIN_ID_OFFSET,
            self.result_domain_id.as_bytes(),
        );
        put(output, DENOMINATOR_OFFSET, &self.denominator.to_le_bytes());
        for (index, coefficient) in self.coefficients.iter().enumerate() {
            let offset = index
                .checked_mul(core::mem::size_of::<u64>())
                .and_then(|value| value.checked_add(COEFFICIENTS_OFFSET))
                .ok_or(Error::ArithmeticOverflow)?;
            put(output, offset, &coefficient.to_le_bytes());
        }
        Ok(())
    }

    /// Return the checked exact wire width for this selected profile.
    pub fn encoded_len() -> Result<usize> {
        portfolio_template_bytes::<N>()
    }

    /// Return the bound categorical claim-basis content identity.
    pub const fn claim_basis_id(self) -> ContentId {
        self.claim_basis_id
    }

    /// Return the exact Product-owned finite result-domain identity.
    pub const fn result_domain_id(self) -> ContentId {
        self.result_domain_id
    }

    /// Return the positive normalized common denominator.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    /// Borrow all normalized coefficients in partition/native-claim order.
    pub const fn coefficients(&self) -> &[u64; N] {
        &self.coefficients
    }

    /// Return one normalized coefficient by native-claim index.
    pub fn coefficient(self, index: usize) -> Result<u64> {
        self.coefficients
            .get(index)
            .copied()
            .ok_or(Error::InvalidPortfolioIndex)
    }

    /// Validate the authenticated basis ID and exact native-claim width.
    pub fn validate_claim_basis(
        self,
        claim_basis_id: ContentId,
        result_domain_id: ContentId,
        claim_basis: CategoricalUnitV1,
    ) -> Result<()> {
        let width = u32::try_from(N).map_err(|_| Error::UnsupportedPortfolioWidth)?;
        if self.claim_basis_id != claim_basis_id
            || self.result_domain_id != result_domain_id
            || claim_basis.outcome_count() != width
        {
            return Err(Error::IdentityMismatch);
        }
        Ok(())
    }

    /// Materialize exact integer native-claim quantities for one user scale.
    ///
    /// Refusal leaves `output` unchanged. There is no rounding boundary: every
    /// scaled numerator must divide by the common denominator exactly and each
    /// quotient must fit the chain-derived `u64` token quantity.
    pub fn materialize(self, scale: u64, output: &mut [u64; N]) -> Result<()> {
        let mut materialized = [0u64; N];
        for (index, coefficient) in self.coefficients.iter().enumerate() {
            let numerator = u128::from(*coefficient)
                .checked_mul(u128::from(scale))
                .ok_or(Error::ArithmeticOverflow)?;
            let denominator = u128::from(self.denominator);
            if numerator % denominator != 0 {
                return Err(Error::NonIntegralPortfolioScale);
            }
            let quantity = numerator
                .checked_div(denominator)
                .ok_or(Error::ArithmeticOverflow)?;
            let destination = materialized
                .get_mut(index)
                .ok_or(Error::ArithmeticOverflow)?;
            *destination = u64::try_from(quantity).map_err(|_| Error::ArithmeticOverflow)?;
        }
        *output = materialized;
        Ok(())
    }
}

fn validate_width<const N: usize>() -> Result<()> {
    if !(MIN_PORTFOLIO_CLAIMS..=MAX_PORTFOLIO_CLAIMS).contains(&N) {
        return Err(Error::UnsupportedPortfolioWidth);
    }
    Ok(())
}

fn common_divisor<const N: usize>(denominator: u64, coefficients: &[u64; N]) -> u64 {
    let mut divisor = denominator;
    for coefficient in coefficients {
        divisor = gcd(divisor, *coefficient);
    }
    divisor
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use crate::capacity::{
        CapacityEnvelope, CapacityProfileId, CapacityProfileV1, CapacityProfileV1Input,
    };
    use crate::claim::CategoricalUnitV1Input;
    use crate::id;

    use super::*;

    fn basis(outcomes: u32) -> CategoricalUnitV1 {
        let profile = CapacityProfileV1::new(CapacityProfileV1Input {
            envelope: CapacityEnvelope::Provisional,
            verifier_release_id: id(1),
            envelope_basis_id: id(2),
            max_artifact_bytes: 320,
            page_payload_bytes: 96,
            max_pages: 4,
            max_partition_cells: 16,
        })
        .expect("capacity");
        CategoricalUnitV1::new(
            CategoricalUnitV1Input {
                capacity_profile_id: CapacityProfileId::new(id(3)),
                outcome_count: outcomes,
            },
            profile,
        )
        .expect("basis")
    }

    fn bytes<const N: usize>(value: PortfolioTemplateV1<N>) -> [u8; 216] {
        let mut output = [0u8; 216];
        let length = PortfolioTemplateV1::<N>::encoded_len().expect("supported width");
        value
            .encode(output.get_mut(..length).expect("supported slice"))
            .expect("encoding");
        output
    }

    #[test]
    fn constructor_normalizes_and_exact_widths_scale_with_n() {
        let value = PortfolioTemplateV1::<2>::new(id(9), id(8), [2, 4], 6)
            .expect("normalized");
        assert_eq!(value.coefficients(), &[1, 2]);
        assert_eq!(value.denominator(), 3);
        assert_eq!(PortfolioTemplateV1::<2>::encoded_len(), Ok(104));
        assert_eq!(PortfolioTemplateV1::<16>::encoded_len(), Ok(216));
        assert_eq!(
            portfolio_template_bytes::<1>(),
            Err(Error::UnsupportedPortfolioWidth)
        );
        assert_eq!(
            portfolio_template_bytes::<17>(),
            Err(Error::UnsupportedPortfolioWidth)
        );
        let encoded = bytes(value);
        assert_eq!(PortfolioTemplateV1::<2>::decode(&encoded[..104]), Ok(value));
    }

    #[test]
    fn hostile_lengths_headers_width_reserved_and_identity_refuse() {
        let value = PortfolioTemplateV1::<2>::new(id(9), id(8), [1, 2], 3)
            .expect("template");
        let encoded = bytes(value);
        for length in 0..104 {
            assert_eq!(
                PortfolioTemplateV1::<2>::decode(encoded.get(..length).expect("supported prefix"),),
                Err(Error::InvalidLength)
            );
        }
        assert_eq!(
            PortfolioTemplateV1::<2>::decode(&encoded[..105]),
            Err(Error::InvalidLength)
        );
        let mut changed = encoded;
        changed[0] = 0;
        assert_eq!(
            PortfolioTemplateV1::<2>::decode(&changed[..104]),
            Err(Error::InvalidMagic)
        );
        let mut changed = encoded;
        changed[8] = 2;
        assert_eq!(
            PortfolioTemplateV1::<2>::decode(&changed[..104]),
            Err(Error::UnsupportedSchema)
        );
        let mut changed = encoded;
        changed[10] = 3;
        assert_eq!(
            PortfolioTemplateV1::<2>::decode(&changed[..104]),
            Err(Error::UnsupportedPortfolioWidth)
        );
        let mut changed = encoded;
        changed[11] = 1;
        assert_eq!(
            PortfolioTemplateV1::<2>::decode(&changed[..104]),
            Err(Error::NonCanonicalReservedBytes)
        );
        let mut changed = encoded;
        changed[16..48].fill(0);
        assert_eq!(
            PortfolioTemplateV1::<2>::decode(&changed[..104]),
            Err(Error::ZeroIdentifier)
        );
    }

    #[test]
    fn empty_zero_denominator_and_reducible_wire_forms_refuse() {
        assert_eq!(
            PortfolioTemplateV1::<2>::new(id(9), id(8), [1, 2], 0),
            Err(Error::ZeroPortfolioDenominator)
        );
        assert_eq!(
            PortfolioTemplateV1::<2>::new(id(9), id(8), [0, 0], 3),
            Err(Error::EmptyPortfolioTemplate)
        );

        let value = PortfolioTemplateV1::<2>::new(id(9), id(8), [1, 2], 3)
            .expect("template");
        let mut changed = bytes(value);
        changed[80..88].copy_from_slice(&6u64.to_le_bytes());
        changed[88..96].copy_from_slice(&2u64.to_le_bytes());
        changed[96..104].copy_from_slice(&4u64.to_le_bytes());
        assert_eq!(
            PortfolioTemplateV1::<2>::decode(&changed[..104]),
            Err(Error::NonCanonicalPortfolioTemplate)
        );
        let mut changed = bytes(value);
        changed[80..88].fill(0);
        assert_eq!(
            PortfolioTemplateV1::<2>::decode(&changed[..104]),
            Err(Error::ZeroPortfolioDenominator)
        );
        let mut changed = bytes(value);
        changed[88..104].fill(0);
        assert_eq!(
            PortfolioTemplateV1::<2>::decode(&changed[..104]),
            Err(Error::EmptyPortfolioTemplate)
        );
    }

    #[test]
    fn binding_and_materialization_are_exact_and_atomic() {
        let value = PortfolioTemplateV1::<2>::new(id(9), id(8), [1, 2], 3)
            .expect("template");
        assert_eq!(value.validate_claim_basis(id(9), id(8), basis(2)), Ok(()));
        assert_eq!(
            value.validate_claim_basis(id(7), id(8), basis(2)),
            Err(Error::IdentityMismatch)
        );
        assert_eq!(
            value.validate_claim_basis(id(9), id(7), basis(2)),
            Err(Error::IdentityMismatch)
        );
        assert_eq!(
            value.validate_claim_basis(id(9), id(8), basis(3)),
            Err(Error::IdentityMismatch)
        );
        assert_eq!(value.coefficient(2), Err(Error::InvalidPortfolioIndex));

        let mut output = [7, 7];
        assert_eq!(
            value.materialize(1, &mut output),
            Err(Error::NonIntegralPortfolioScale)
        );
        assert_eq!(output, [7, 7]);
        assert_eq!(value.materialize(3, &mut output), Ok(()));
        assert_eq!(output, [1, 2]);
        assert_eq!(value.materialize(0, &mut output), Ok(()));
        assert_eq!(output, [0, 0]);

        let overflowing = PortfolioTemplateV1::<2>::new(id(9), id(8), [u64::MAX, 1], 1)
            .expect("template");
        output = [9, 9];
        assert_eq!(
            overflowing.materialize(2, &mut output),
            Err(Error::ArithmeticOverflow)
        );
        assert_eq!(output, [9, 9]);
    }

    #[test]
    fn refused_output_width_does_not_mutate() {
        let value = PortfolioTemplateV1::<2>::new(id(9), id(8), [1, 2], 3)
            .expect("template");
        let mut output = [0xa5; 103];
        assert_eq!(value.encode(&mut output), Err(Error::OutputLength));
        assert_eq!(output, [0xa5; 103]);
    }

    #[test]
    fn maximum_profile_round_trips() {
        let value = PortfolioTemplateV1::<16>::new(id(9), id(8), [1; 16], 3)
            .expect("template");
        let encoded = bytes(value);
        assert_eq!(PortfolioTemplateV1::<16>::decode(&encoded), Ok(value));
    }
}
