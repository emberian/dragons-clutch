//! Exact composition-DAG witness for the existing Fractional/Product basis.

use dclutch_representation_composition_v3_kernel::{
    CanonicalTranslationInputV3, CompositionBundleV3, RecordAdmissionV3, SparseTermV3,
    composition_translation_bytes_v3, decode_composition_bundle_v3,
    encode_canonical_translation_v3_atomic,
};

use crate::{Error, FractionalPreparedChainArtifactsV1, Result};

/// Ephemeral proof that one admitted DAG flattens to the already-authenticated
/// Product portfolio used by the existing Fractional terms.
///
/// This witness owns no payout, balance, supply, resolution, or custody fact.
/// Its fields are private so release tooling cannot construct one without the
/// exact Product/terms/retranslation checks below.
#[derive(Clone, Copy)]
pub struct CheckedFractionalCompositionV1<'a> {
    bundle: CompositionBundleV3<'a>,
    product_record: [u8; 32],
    portfolio: [u8; 32],
}

impl<'a> CheckedFractionalCompositionV1<'a> {
    /// Completely admitted descriptor, bounded DAG, and canonical translation.
    pub const fn bundle(self) -> CompositionBundleV3<'a> {
        self.bundle
    }

    /// Finalized Product root digest supplying Product identity and domain.
    pub const fn product_record(self) -> [u8; 32] {
        self.product_record
    }

    /// Exact Product portfolio digest whose coefficients equal the translation.
    pub const fn portfolio(self) -> [u8; 32] {
        self.portfolio
    }
}

/// Decode admitted composition records, then bind them to one prepared
/// Fractional chain snapshot and byte-identically retranslate the root.
#[allow(clippy::too_many_arguments)]
pub fn decode_and_check_fractional_composition_v1<'a>(
    prepared: FractionalPreparedChainArtifactsV1<'_>,
    descriptor_bytes: &[u8],
    descriptor_admission: RecordAdmissionV3,
    graph_bytes: &'a [u8],
    graph_admission: RecordAdmissionV3,
    translation_bytes: &'a [u8],
    translation_admission: RecordAdmissionV3,
) -> Result<CheckedFractionalCompositionV1<'a>> {
    let bundle = decode_composition_bundle_v3(
        descriptor_bytes,
        descriptor_admission,
        graph_bytes,
        graph_admission,
        translation_bytes,
        translation_admission,
    )
    .map_err(|_| Error::Composition)?;
    check_fractional_composition_bundle_v1(prepared, bundle)
}

/// Bind one already admitted composition bundle to the exact existing
/// Product portfolio and Fractional terms, then independently re-emit the
/// translation and require every byte to agree.
pub fn check_fractional_composition_bundle_v1<'a>(
    prepared: FractionalPreparedChainArtifactsV1<'_>,
    bundle: CompositionBundleV3<'a>,
) -> Result<CheckedFractionalCompositionV1<'a>> {
    let descriptor = bundle.descriptor();
    let translation = bundle.translation();
    let terms = prepared.terms();
    let context = prepared.request_context();
    let product_record = prepared.product_record();
    let product_join = prepared.product_join();
    let portfolio = prepared.portfolio();

    if descriptor.market() != context.market
        || descriptor.market() != terms.market_id()
        || descriptor.result_domain() != context.result_domain
        || descriptor.result_domain() != terms.result_domain_id()
        || descriptor.result_domain() != product_join.result_domain_id.to_bytes()
        || descriptor.release_set() != context.release_set
        || descriptor.release_set() != terms.release_set_id()
        || descriptor.native_basis() != product_join.claim_basis_id.to_bytes()
        || descriptor.outcome_count() != terms.outcome_count()
        || descriptor.outcome_count() != product_join.outcome_count
        || product_record.product_id() != product_join.product_id
        || product_record.result_domain_digest() != product_join.result_domain_id
        || product_record.portfolio_digest() != product_join.representation_id
        || portfolio.product_id() != product_join.product_id
        || portfolio.result_domain_id() != product_join.result_domain_id
        || portfolio.claim_basis_id() != product_join.claim_basis_id
        || portfolio.coefficient_count() != product_join.outcome_count
        || portfolio.denominator() != translation.denominator()
    {
        return Err(Error::Composition);
    }

    require_exact_portfolio_coefficients(portfolio.coefficients(), translation)?;
    require_byte_identical_retranslation(bundle)?;
    Ok(CheckedFractionalCompositionV1 {
        bundle,
        product_record: context.product_record,
        portfolio: product_join.representation_id.to_bytes(),
    })
}

fn require_exact_portfolio_coefficients(
    coefficients: impl ExactSizeIterator<Item = u64>,
    translation: dclutch_representation_composition_v3_kernel::CanonicalTranslationV3<'_>,
) -> Result<()> {
    if coefficients.len()
        != usize::try_from(translation.outcome_count()).map_err(|_| Error::Composition)?
    {
        return Err(Error::Composition);
    }
    let mut term_index = 0_u32;
    for (outcome, coefficient) in coefficients.enumerate() {
        let outcome = u32::try_from(outcome).map_err(|_| Error::Composition)?;
        let term = if term_index < translation.term_count() {
            Some(
                translation
                    .term(term_index)
                    .map_err(|_| Error::Composition)?,
            )
        } else {
            None
        };
        let expected = if term.is_some_and(|value| value.outcome == outcome) {
            term_index = term_index.checked_add(1).ok_or(Error::Composition)?;
            term.ok_or(Error::Composition)?.numerator
        } else {
            0
        };
        if coefficient != expected {
            return Err(Error::Composition);
        }
    }
    if term_index != translation.term_count() {
        return Err(Error::Composition);
    }
    Ok(())
}

fn require_byte_identical_retranslation(bundle: CompositionBundleV3<'_>) -> Result<()> {
    let translation = bundle.translation();
    let term_count = usize::try_from(translation.term_count()).map_err(|_| Error::Composition)?;
    let mut terms = Vec::<SparseTermV3>::with_capacity(term_count);
    let mut index = 0_u32;
    while index < translation.term_count() {
        terms.push(translation.term(index).map_err(|_| Error::Composition)?);
        index = index.checked_add(1).ok_or(Error::Composition)?;
    }
    let width = composition_translation_bytes_v3(translation.term_count())
        .map_err(|_| Error::Composition)?;
    let mut scratch = vec![0; width];
    let mut output = vec![0; width];
    encode_canonical_translation_v3_atomic(
        CanonicalTranslationInputV3 {
            graph_id: bundle.graph().graph_id(),
            root_id: bundle.graph().root_id(),
            outcome_count: bundle.graph().outcome_count(),
            denominator: bundle
                .graph()
                .root_denominator()
                .map_err(|_| Error::Composition)?,
            terms: &terms,
        },
        &mut scratch,
        &mut output,
    )
    .map_err(|_| Error::Composition)?;
    if output.as_slice() != translation.as_bytes() {
        return Err(Error::Composition);
    }
    Ok(())
}
