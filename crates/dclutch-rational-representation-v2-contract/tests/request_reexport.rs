//! Compile-time evidence that the historical public request path is an exact
//! re-export of the claims-independent semantic owner.

use dclutch_rational_representation_v2_contract::RepresentationRequestV2 as ReexportedRequestV2;
use dclutch_rational_representation_v2_request_contract::RepresentationRequestV2 as CanonicalRequestV2;

fn canonical_to_reexported(value: CanonicalRequestV2<'static>) -> ReexportedRequestV2<'static> {
    value
}

fn reexported_to_canonical(value: ReexportedRequestV2<'static>) -> CanonicalRequestV2<'static> {
    value
}

#[test]
fn both_public_paths_name_the_identical_rust_type() {
    let _: fn(CanonicalRequestV2<'static>) -> ReexportedRequestV2<'static> =
        canonical_to_reexported;
    let _: fn(ReexportedRequestV2<'static>) -> CanonicalRequestV2<'static> =
        reexported_to_canonical;
}
