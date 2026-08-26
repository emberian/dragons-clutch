//! Exact Registry and Product authentication for one rational representation.
//!
//! The descriptor digest is the external representation selector. The
//! descriptor itself selects the graph identity and exact graph digest; a
//! caller cannot independently substitute either child. ProductRuntimeV3 owns
//! the result ontology and liability-basis semantics. The finalized descriptor
//! owns its runtime-width receipt recipe. Claims and Token account state remain
//! a later physical adapter boundary.

use dclutch_product_runtime_v2::ContentId;
use dclutch_rational_representation_v2_kernel::{
    ContentAdmissionV2, DescriptorAdmissionV2, RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
    REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3, REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2,
    RepresentationDescriptorV2,
    product_v3::{
        ProductRepresentationInputV3, ProductRuntimeProjectionV3, RepresentationAdmissionV3,
        RepresentationContextV3, admit_product_representation_v3,
    },
};
use solana_program::{account_info::AccountInfo, pubkey::Pubkey, rent::Rent};

use super::{
    AuthenticatedProductRuntimeV3, Error, FinalizedRecordFrameV2, ProductRuntimeFrameV3, Result,
    authenticate_product_runtime_v3, authenticate_record, content,
};

/// Product graph plus immutable descriptor and descriptor-selected graph.
#[derive(Clone, Copy)]
pub struct RepresentationRuntimeFrameV3<'accounts, 'info> {
    /// Exact Product/domain/portfolio/ProductBasisV3 evidence.
    pub product: ProductRuntimeFrameV3<'accounts, 'info>,
    /// Finalized representation descriptor.
    pub descriptor: FinalizedRecordFrameV2<'accounts, 'info>,
    /// Finalized representation graph selected by the descriptor.
    pub graph: FinalizedRecordFrameV2<'accounts, 'info>,
}

/// Independently authenticated Market, Claims, and Token identities.
///
/// These values must come from canonical Market/Claims/Realm account state.
/// The representation descriptor may only agree with them; it cannot create
/// their authority. The representation PDA is absent because this adapter
/// derives it under `claims_program` from the finalized descriptor digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepresentationRuntimeContextV3 {
    /// Deployed Claims program owning the representation authority PDA.
    pub claims_program: Pubkey,
    /// Logical Core Market whose Claims back the representation.
    pub market: Pubkey,
    /// Immutable release set selected by the Market.
    pub release_set: Pubkey,
    /// Semantic basis persisted by the authenticated LBV2 Claims Market.
    pub claims_basis_id: ContentId,
    /// Runtime width persisted by the authenticated LBV2 Claims Market.
    pub claims_width: u32,
    /// Exact closeable Structured receipt Mint authenticated by Token-2022.
    pub receipt_mint: Pubkey,
    /// Realm-selected Token program authenticated by the physical adapter.
    pub token_program: Pubkey,
}

/// Complete ephemeral authentication result for a Product representation.
///
/// The fixed admission is a checked cache for downstream execution. It never
/// replaces reauthentication of any raw/staging pair at an effect boundary.
#[derive(Clone, Copy)]
pub struct AuthenticatedRepresentationRuntimeV3<'accounts, 'info> {
    /// Exact authenticated Product graph-root digest.
    pub product_record_digest: ContentId,
    /// Runtime Product result selector count, including explicit failure.
    pub result_outcome_count: u32,
    /// Marker retaining the raw-frame lifetime in this authenticated view.
    pub frame_lifetime: core::marker::PhantomData<&'accounts AccountInfo<'info>>,
    /// Exact immutable Product/Claims/representation join.
    pub admission: RepresentationAdmissionV3,
}

/// Authenticate one ProductRuntimeV3 graph, one selected representation
/// descriptor, and the graph selected inside that descriptor.
///
/// `expected_descriptor_digest` is the sole representation record selector.
/// The graph identity and digest come from the authenticated descriptor body.
/// All twelve Registry accounts must be distinct and read-only.
pub fn authenticate_product_representation_v3<'accounts, 'info>(
    registry_program: &Pubkey,
    rent: &Rent,
    expected_product_digest: ContentId,
    expected_descriptor_digest: ContentId,
    context: RepresentationRuntimeContextV3,
    frame: RepresentationRuntimeFrameV3<'accounts, 'info>,
) -> Result<AuthenticatedRepresentationRuntimeV3<'accounts, 'info>> {
    require_distinct(frame)?;
    let product = authenticate_product_runtime_v3(
        registry_program,
        rent,
        expected_product_digest,
        frame.product,
    )?;
    let descriptor_record = authenticate_record(
        registry_program,
        rent,
        frame.descriptor,
        REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
        expected_descriptor_digest,
        Error::RepresentationDescriptorRecord,
    )?;
    let representation_authority = Pubkey::find_program_address(
        &[
            RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
            expected_descriptor_digest.to_bytes().as_slice(),
        ],
        &context.claims_program,
    )
    .0;
    let descriptor_admission = DescriptorAdmissionV2 {
        selected_descriptor_id: expected_descriptor_digest.to_bytes(),
        finalized_descriptor_id: descriptor_record.content_digest.to_bytes(),
        recomputed_descriptor_digest: descriptor_record.content_digest.to_bytes(),
        finalized_descriptor_digest: descriptor_record.content_digest.to_bytes(),
        record_authenticated: true,
        derived_representation_authority: representation_authority.to_bytes(),
        authority_derivation_authenticated: true,
    };
    let (graph_id, graph_digest) = {
        let descriptor_data = frame
            .descriptor
            .raw
            .try_borrow_data()
            .map_err(|_| Error::Borrow)?;
        let descriptor = RepresentationDescriptorV2::decode(&descriptor_data, descriptor_admission)
            .map_err(|_| Error::RepresentationComposition)?;
        (descriptor.graph_id(), descriptor.graph_digest())
    };
    let graph_digest = content(graph_digest).map_err(|_| Error::RepresentationComposition)?;
    let graph_record = authenticate_record(
        registry_program,
        rent,
        frame.graph,
        REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2,
        graph_digest,
        Error::RepresentationGraphRecord,
    )?;
    let graph_admission = ContentAdmissionV2 {
        selected_graph_id: graph_id,
        finalized_graph_id: graph_id,
        recomputed_graph_digest: graph_record.content_digest.to_bytes(),
        finalized_graph_digest: graph_record.content_digest.to_bytes(),
        record_authenticated: true,
    };
    let product_record_digest = product.runtime.product_record.content_digest;
    let result_outcome_count = product.runtime.outcome_count;
    let admission = admit_authenticated_representation_v3(
        product,
        context,
        frame,
        descriptor_admission,
        graph_admission,
    )?;
    Ok(AuthenticatedRepresentationRuntimeV3 {
        product_record_digest,
        result_outcome_count,
        frame_lifetime: core::marker::PhantomData,
        admission,
    })
}

#[inline(never)]
fn admit_authenticated_representation_v3(
    product: AuthenticatedProductRuntimeV3<'_, '_>,
    context: RepresentationRuntimeContextV3,
    frame: RepresentationRuntimeFrameV3<'_, '_>,
    descriptor_admission: DescriptorAdmissionV2,
    graph_admission: ContentAdmissionV2,
) -> Result<RepresentationAdmissionV3> {
    let basis_data = frame
        .product
        .linked_basis
        .raw
        .try_borrow_data()
        .map_err(|_| Error::Borrow)?;
    let descriptor_data = frame
        .descriptor
        .raw
        .try_borrow_data()
        .map_err(|_| Error::Borrow)?;
    let graph_data = frame
        .graph
        .raw
        .try_borrow_data()
        .map_err(|_| Error::Borrow)?;
    admit_product_representation_v3(ProductRepresentationInputV3 {
        product_basis_bytes: &basis_data,
        product: ProductRuntimeProjectionV3 {
            product_id: product.runtime.product_id.to_bytes(),
            result_domain_id: product
                .runtime
                .result_domain_record
                .content_digest
                .to_bytes(),
            coordinate_domain_id: product.runtime.coordinate_domain_id.to_bytes(),
            result_unit_id: product.runtime.result_unit_id.to_bytes(),
            semantic_basis_id: product.semantic_basis_id.to_bytes(),
            linked_basis_record_digest: product.linked_basis_record.content_digest.to_bytes(),
            evaluator_release_id: product.evaluator_release_id.to_bytes(),
            basis_width: product.basis_width,
            payout_scale: product.payout_scale,
        },
        descriptor_bytes: &descriptor_data,
        descriptor_admission,
        graph_bytes: &graph_data,
        graph_admission,
        context: RepresentationContextV3 {
            market_id: context.market.to_bytes(),
            release_set_id: context.release_set.to_bytes(),
            claims_basis_id: context.claims_basis_id.to_bytes(),
            claims_width: context.claims_width,
            receipt_mint: context.receipt_mint.to_bytes(),
            token_program: context.token_program.to_bytes(),
            representation_authority: descriptor_admission.derived_representation_authority,
        },
    })
    .map(|admitted| admitted.admission())
    .map_err(|_| Error::RepresentationComposition)
}

fn require_distinct(frame: RepresentationRuntimeFrameV3<'_, '_>) -> Result<()> {
    let accounts: [&AccountInfo<'_>; 12] = [
        frame.product.product.raw,
        frame.product.product.staging,
        frame.product.result_domain.raw,
        frame.product.result_domain.staging,
        frame.product.portfolio.raw,
        frame.product.portfolio.staging,
        frame.product.linked_basis.raw,
        frame.product.linked_basis.staging,
        frame.descriptor.raw,
        frame.descriptor.staging,
        frame.graph.raw,
        frame.graph.staging,
    ];
    for (left_index, left) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(left_index.saturating_add(1))
            .any(|right| left.key == right.key)
        {
            return Err(Error::AccountFrame);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authority_seed_is_not_request_selectable() {
        let claims_program = Pubkey::new_from_array([9; 32]);
        let descriptor = ContentId::new([7; 32]).expect("descriptor");
        let expected = Pubkey::find_program_address(
            &[
                RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
                descriptor.to_bytes().as_slice(),
            ],
            &claims_program,
        )
        .0;
        assert_ne!(expected, Pubkey::default());
        assert_ne!(
            expected,
            Pubkey::find_program_address(
                &[b"caller-selected", descriptor.to_bytes().as_slice()],
                &claims_program,
            )
            .0
        );
    }
}
