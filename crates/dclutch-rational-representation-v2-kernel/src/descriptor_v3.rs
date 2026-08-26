//! Checked atomic encoder for [`RepresentationDescriptorV2`].

use crate::{
    DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_DENOMINATOR_OFFSET, DESCRIPTOR_GRAPH_DIGEST_OFFSET,
    DESCRIPTOR_GRAPH_ID_OFFSET, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_OFFSET,
    DESCRIPTOR_MAGIC_V3, DESCRIPTOR_MARKET_ID_OFFSET, DESCRIPTOR_OUTCOME_COUNT_OFFSET,
    DESCRIPTOR_RECEIPT_MINT_OFFSET, DESCRIPTOR_RELEASE_SET_ID_OFFSET, DESCRIPTOR_ROOT_ID_OFFSET,
    DESCRIPTOR_SCHEMA_VERSION_V3, DESCRIPTOR_TOKEN_PROGRAM_OFFSET, DESCRIPTOR_VERSION_OFFSET,
    DescriptorAdmissionV2, Error, RepresentationDescriptorV2, Result,
};

/// Semantic fields of one immutable rational execution descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepresentationDescriptorInputV3<'a> {
    /// Finalized exposure-record content identity selected for execution.
    pub exposure_id: [u8; 32],
    /// SHA-256 of the exact finalized exposure bytes.
    pub exposure_digest: [u8; 32],
    /// Exact selected representation root identity.
    pub root_id: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Immutable execution release set.
    pub release_set: [u8; 32],
    /// Claims-derived closeable receipt Mint.
    pub receipt_mint: [u8; 32],
    /// Exact Token program selected by Realm/release policy.
    pub token_program: [u8; 32],
    /// Common exact coefficient denominator.
    pub denominator: u64,
    /// Ordered nonnegative representation coefficients; its length is `K`.
    pub coefficients: &'a [u64],
}

/// Exact descriptor byte width for representation width `K`.
pub fn representation_descriptor_bytes_v3(representation_width: usize) -> Result<usize> {
    if representation_width == 0 || u32::try_from(representation_width).is_err() {
        return Err(Error::InvalidWidth);
    }
    representation_width
        .checked_mul(DESCRIPTOR_COEFFICIENT_BYTES)
        .and_then(|tail| DESCRIPTOR_HEADER_BYTES.checked_add(tail))
        .ok_or(Error::InvalidLength)
}

/// Encode one descriptor atomically, preserving `output` on every refusal.
pub fn encode_representation_descriptor_v3_atomic(
    input: RepresentationDescriptorInputV3<'_>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let bytes = representation_descriptor_bytes_v3(input.coefficients.len())?;
    if scratch.len() != bytes || output.len() != bytes {
        return Err(Error::InvalidLength);
    }
    if input.denominator == 0 {
        return Err(Error::ZeroDenominator);
    }
    if input
        .coefficients
        .iter()
        .all(|coefficient| *coefficient == 0)
    {
        return Err(Error::EmptyRecipe);
    }
    if [
        input.exposure_id,
        input.exposure_digest,
        input.root_id,
        input.market,
        input.release_set,
        input.receipt_mint,
        input.token_program,
    ]
    .into_iter()
    .any(|identity| identity == [0; 32])
    {
        return Err(Error::ZeroIdentity);
    }
    scratch.fill(0);
    put(scratch, DESCRIPTOR_MAGIC_OFFSET, &DESCRIPTOR_MAGIC_V3)?;
    put(
        scratch,
        DESCRIPTOR_VERSION_OFFSET,
        &DESCRIPTOR_SCHEMA_VERSION_V3.to_le_bytes(),
    )?;
    for (offset, value) in [
        (DESCRIPTOR_GRAPH_ID_OFFSET, input.exposure_id),
        (DESCRIPTOR_GRAPH_DIGEST_OFFSET, input.exposure_digest),
        (DESCRIPTOR_ROOT_ID_OFFSET, input.root_id),
        (DESCRIPTOR_MARKET_ID_OFFSET, input.market),
        (DESCRIPTOR_RELEASE_SET_ID_OFFSET, input.release_set),
        (DESCRIPTOR_RECEIPT_MINT_OFFSET, input.receipt_mint),
        (DESCRIPTOR_TOKEN_PROGRAM_OFFSET, input.token_program),
    ] {
        put(scratch, offset, &value)?;
    }
    put(
        scratch,
        DESCRIPTOR_OUTCOME_COUNT_OFFSET,
        &u32::try_from(input.coefficients.len())
            .map_err(|_| Error::InvalidWidth)?
            .to_le_bytes(),
    )?;
    put(
        scratch,
        DESCRIPTOR_DENOMINATOR_OFFSET,
        &input.denominator.to_le_bytes(),
    )?;
    for (index, coefficient) in input.coefficients.iter().copied().enumerate() {
        let offset = index
            .checked_mul(DESCRIPTOR_COEFFICIENT_BYTES)
            .and_then(|tail| DESCRIPTOR_HEADER_BYTES.checked_add(tail))
            .ok_or(Error::InvalidLength)?;
        put(scratch, offset, &coefficient.to_le_bytes())?;
    }
    let descriptor = RepresentationDescriptorV2::decode(
        scratch,
        DescriptorAdmissionV2 {
            selected_descriptor_id: [1; 32],
            finalized_descriptor_id: [1; 32],
            recomputed_descriptor_digest: [1; 32],
            finalized_descriptor_digest: [1; 32],
            record_authenticated: true,
            derived_representation_authority: [2; 32],
            authority_derivation_authenticated: true,
        },
    )?;
    if descriptor.graph_id() != input.exposure_id
        || descriptor.graph_digest() != input.exposure_digest
        || descriptor.root_id() != input.root_id
        || descriptor.market_id() != input.market
        || descriptor.release_set_id() != input.release_set
        || descriptor.receipt_mint() != input.receipt_mint
        || descriptor.token_program() != input.token_program
        || descriptor.outcome_count()
            != u32::try_from(input.coefficients.len()).map_err(|_| Error::InvalidWidth)?
        || descriptor.denominator() != input.denominator
    {
        return Err(Error::DescriptorMismatch);
    }
    output.copy_from_slice(scratch);
    Ok(())
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

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    fn input<'a>(coefficients: &'a [u64]) -> RepresentationDescriptorInputV3<'a> {
        RepresentationDescriptorInputV3 {
            exposure_id: [1; 32],
            exposure_digest: [2; 32],
            root_id: [3; 32],
            market: [4; 32],
            release_set: [5; 32],
            receipt_mint: [6; 32],
            token_program: [7; 32],
            denominator: 11,
            coefficients,
        }
    }

    #[test]
    fn exact_k3_roundtrip_and_atomic_refusal() {
        let width = representation_descriptor_bytes_v3(3).expect("K3 width");
        let mut scratch = std::vec![0; width];
        let mut output = std::vec![0xa5; width];
        encode_representation_descriptor_v3_atomic(input(&[2, 0, 5]), &mut scratch, &mut output)
            .expect("K3 descriptor");
        let decoded = RepresentationDescriptorV2::decode(
            &output,
            DescriptorAdmissionV2 {
                selected_descriptor_id: [8; 32],
                finalized_descriptor_id: [8; 32],
                recomputed_descriptor_digest: [8; 32],
                finalized_descriptor_digest: [8; 32],
                record_authenticated: true,
                derived_representation_authority: [9; 32],
                authority_derivation_authenticated: true,
            },
        )
        .expect("hostile decode");
        assert_eq!(decoded.outcome_count(), 3);
        assert_eq!(decoded.coefficient(0), Ok(2));
        assert_eq!(decoded.coefficient(1), Ok(0));
        assert_eq!(decoded.coefficient(2), Ok(5));

        let before = output.clone();
        assert!(
            encode_representation_descriptor_v3_atomic(
                input(&[0, 0, 0]),
                &mut scratch,
                &mut output,
            )
            .is_err()
        );
        assert_eq!(output, before);
    }
}
