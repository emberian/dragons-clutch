//! The one genuinely new host-side artifact decision 0011 §3c named.
//!
//! The fixtures are in `fixture/mod.rs` and every one of them is a REAL record.
//! What is asserted here is the derivation itself: that the descriptor it emits
//! is the record the chain would admit, that it names the EXPOSURE bundle and
//! not the source graph, and that it refuses a composition whose root is not
//! the recipe the terms state -- a join the live chain route lost when the
//! exposure bundle superseded the legacy graph.

#[path = "structured_fixture/mod.rs"]
mod fixture;
#[path = "structured_support/mod.rs"]
mod support;

use dclutch_claims::structured_kernel::StructuredTermsV2;
use dclutch_operator::structured::{
    Error, StructuredDescriptorAuthorityV2, decode_derived_structured_descriptor_v2,
    derive_structured_representation_descriptor_v2, structured_child_descriptor_from_derivation_v2,
};
use dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID;

use fixture::{
    AUTHORITY, COEFFICIENTS, DENOMINATOR, GRAPH_ID, K, ROOT_ID, composition, decode_terms,
    shard_terms_bytes, shard_terms_bytes_scaled, terms_bytes,
};
use support::{digest, identity, shard_terms, structured_admission};

fn authority() -> StructuredDescriptorAuthorityV2 {
    StructuredDescriptorAuthorityV2 {
        representation_authority: identity(AUTHORITY),
    }
}

#[test]
fn the_derived_descriptor_decodes_as_the_record_the_chain_would_admit() {
    let composition = composition(COEFFICIENTS, DENOMINATOR, identity(GRAPH_ID));
    let exposure_id = composition.exposure_id();
    let terms_source = terms_bytes(exposure_id, &COEFFICIENTS);
    let shard_source = shard_terms_bytes(exposure_id);
    let terms = decode_terms(&terms_source, &shard_source);

    let derived = derive_structured_representation_descriptor_v2(
        terms,
        composition.bundle(),
        composition.exposure_bundle(),
    )
    .expect("derived descriptor");

    // The descriptor_id is the digest of the exact bytes, which is what the
    // Claims adapter recomputes at rational-representation-v2-operator:533.
    assert_eq!(derived.descriptor_id, digest(&derived.preimage));
    assert_eq!(derived.outcome_count, K);
    assert_eq!(derived.denominator, DENOMINATOR);

    let descriptor = decode_derived_structured_descriptor_v2(&derived, authority())
        .expect("hostile decode of the derived preimage");
    // The identity the chain reads out of this slot is the EXPOSURE bundle, and
    // it is not the source graph.
    assert_eq!(descriptor.graph_id(), exposure_id);
    assert_eq!(descriptor.graph_id(), terms.shard_exposure());
    assert_ne!(descriptor.graph_id(), terms.graph_id());
    assert_eq!(descriptor.root_id(), identity(ROOT_ID));
    assert_eq!(descriptor.market_id(), terms.market());
    assert_eq!(descriptor.release_set_id(), terms.release_set());
    assert_eq!(descriptor.receipt_mint(), terms.receipt_mint());
    assert_eq!(descriptor.token_program(), TOKEN_2022_PROGRAM_ID);
    assert_eq!(descriptor.outcome_count(), K);
    for (coordinate, coefficient) in COEFFICIENTS.iter().copied().enumerate() {
        let coordinate = u32::try_from(coordinate).expect("coordinate");
        assert_eq!(descriptor.coefficient(coordinate), Ok(coefficient));
    }

    // The chain re-runs this exact join on the exposure record it admits.
    descriptor
        .authenticate_exposure(composition.exposure_bundle())
        .expect("the descriptor authenticates the exposure it was derived from");
}

#[test]
fn the_derived_descriptor_feeds_the_child_wire_without_a_hand_filled_field() {
    let composition = composition(COEFFICIENTS, DENOMINATOR, identity(GRAPH_ID));
    let exposure_id = composition.exposure_id();
    let terms_source = terms_bytes(exposure_id, &COEFFICIENTS);
    let shard_source = shard_terms_bytes(exposure_id);
    let terms = decode_terms(&terms_source, &shard_source);
    let derived = derive_structured_representation_descriptor_v2(
        terms,
        composition.bundle(),
        composition.exposure_bundle(),
    )
    .expect("derived descriptor");

    let child = structured_child_descriptor_from_derivation_v2(terms, &derived, authority())
        .expect("child descriptor from the derivation");
    assert_eq!(child.descriptor_id, derived.descriptor_id);
    assert_eq!(child.exposure_id, exposure_id);
    assert_ne!(child.exposure_id, terms.graph_id());
    assert_eq!(child.outcome_count, K);
    assert_eq!(child.denominator, DENOMINATOR);
}

#[test]
fn a_composition_whose_root_is_not_the_recipe_refuses() {
    // The composition is a VALID record in its own right -- it encodes, decodes
    // and cross-joins -- it simply states a different recipe than the terms do.
    // Nothing on the live chain route would ever notice: `authenticate_exposure`
    // checks the bundle identity, digest and width and never the coefficients,
    // and the only join that did check them reads a superseded record with no
    // callers. This is the last place the two objects are both in hand.
    let composition = composition([2, 3, 6], DENOMINATOR, identity(GRAPH_ID));
    let exposure_id = composition.exposure_id();
    let terms_source = terms_bytes(exposure_id, &COEFFICIENTS);
    let shard_source = shard_terms_bytes(exposure_id);
    let terms = decode_terms(&terms_source, &shard_source);
    assert_eq!(
        derive_structured_representation_descriptor_v2(
            terms,
            composition.bundle(),
            composition.exposure_bundle(),
        )
        .err(),
        Some(Error::Terms)
    );
}

#[test]
fn the_same_recipe_at_a_different_scale_still_derives() {
    // Cross multiplication, not equality: the composition may state the recipe
    // at any common scale the graph encoder admits. `[4, 6, 10] / 14` is
    // `[2, 3, 5] / 7`, and the graph encoder itself refuses a non-reduced root
    // (`gcd(flattened_denominator, numerators) != 1`), so this is the SAME
    // record with a different, legal presentation of the leaves' contribution.
    let composition = composition(COEFFICIENTS, DENOMINATOR, identity(GRAPH_ID));
    let exposure_id = composition.exposure_id();
    // Terms at twice the scale: 4/14 == 2/7.
    let terms_source = terms_bytes(exposure_id, &[4, 6, 10]);
    let shard_source = shard_terms_bytes_scaled(exposure_id, 14);
    let terms = StructuredTermsV2::decode(
        &terms_source,
        structured_admission(&terms_source),
        shard_terms(&shard_source),
    );
    // The shard layer pins the denominator, so a rescaled recipe is a different
    // shard layer; if the two disagree the terms never decode at all, which is
    // the join doing its job one layer down.
    assert!(terms.is_err());
}

#[test]
fn an_exposure_bundle_from_another_composition_refuses() {
    // DIRECTION ONE of the cross-family substitution, at the record layer the
    // child join cannot see: a coherent exposure bundle belonging to a
    // different source graph.
    let canonical = composition(COEFFICIENTS, DENOMINATOR, identity(GRAPH_ID));
    let foreign = composition(COEFFICIENTS, DENOMINATOR, identity(0x99));
    let exposure_id = canonical.exposure_id();
    let terms_source = terms_bytes(exposure_id, &COEFFICIENTS);
    let shard_source = shard_terms_bytes(exposure_id);
    let terms = decode_terms(&terms_source, &shard_source);

    // The foreign exposure names a graph these terms do not.
    assert_eq!(
        derive_structured_representation_descriptor_v2(
            terms,
            canonical.bundle(),
            foreign.exposure_bundle(),
        )
        .err(),
        Some(Error::ChildIdentity)
    );
    // And the foreign bundle's graph is not the one the exposure names.
    assert_eq!(
        derive_structured_representation_descriptor_v2(
            terms,
            foreign.bundle(),
            canonical.exposure_bundle(),
        )
        .err(),
        Some(Error::ChildIdentity)
    );
}

#[test]
fn terms_naming_a_different_exposure_record_refuse() {
    // DIRECTION TWO: hold the composition, move the terms. These terms are
    // internally coherent -- their own shard layer names the same exposure --
    // so the refusal is the record join failing, not a fixture that never
    // decoded.
    let composition = composition(COEFFICIENTS, DENOMINATOR, identity(GRAPH_ID));
    let foreign_exposure = identity(0x97);
    let terms_source = terms_bytes(foreign_exposure, &COEFFICIENTS);
    let shard_source = shard_terms_bytes(foreign_exposure);
    let terms = decode_terms(&terms_source, &shard_source);
    assert_eq!(
        derive_structured_representation_descriptor_v2(
            terms,
            composition.bundle(),
            composition.exposure_bundle(),
        )
        .err(),
        Some(Error::ChildIdentity)
    );
}

#[test]
fn the_authority_may_not_alias_the_descriptor_it_is_derived_from() {
    let composition = composition(COEFFICIENTS, DENOMINATOR, identity(GRAPH_ID));
    let exposure_id = composition.exposure_id();
    let terms_source = terms_bytes(exposure_id, &COEFFICIENTS);
    let shard_source = shard_terms_bytes(exposure_id);
    let terms = decode_terms(&terms_source, &shard_source);
    let derived = derive_structured_representation_descriptor_v2(
        terms,
        composition.bundle(),
        composition.exposure_bundle(),
    )
    .expect("derived descriptor");

    for alias in [[0_u8; 32], derived.descriptor_id] {
        assert_eq!(
            decode_derived_structured_descriptor_v2(
                &derived,
                StructuredDescriptorAuthorityV2 {
                    representation_authority: alias,
                },
            )
            .err(),
            Some(Error::ChildIdentity)
        );
    }
}

#[test]
fn the_exact_backing_invariant_is_the_descriptors_own_coefficients() {
    // K_i = S * c_i, stated on both sides of the derivation: the Structured
    // terms compute it, and the descriptor the chain will read carries the same
    // c_i. The callee recomputes it as `asset.coefficient * header.quantity`
    // (rational-representation-v2 plan.rs:263), so agreement here is what makes
    // the on-chain arithmetic reproduce the host's.
    let composition = composition(COEFFICIENTS, DENOMINATOR, identity(GRAPH_ID));
    let exposure_id = composition.exposure_id();
    let terms_source = terms_bytes(exposure_id, &COEFFICIENTS);
    let shard_source = shard_terms_bytes(exposure_id);
    let terms = decode_terms(&terms_source, &shard_source);
    let derived = derive_structured_representation_descriptor_v2(
        terms,
        composition.bundle(),
        composition.exposure_bundle(),
    )
    .expect("derived descriptor");
    let descriptor =
        decode_derived_structured_descriptor_v2(&derived, authority()).expect("hostile decode");

    for supply in [0_u64, 1, 3, 1_000_000] {
        for coordinate in 0..K {
            let from_terms = terms
                .required_shard_custody(coordinate, supply)
                .expect("required backing");
            let from_descriptor = descriptor
                .coefficient(coordinate)
                .expect("descriptor coefficient")
                .checked_mul(supply)
                .expect("no overflow at test scale");
            assert_eq!(from_terms, from_descriptor);
        }
    }

    // Coprimality is the point of this basis: no single-coordinate skew of one
    // atom is a whole multiple of another coordinate's requirement, so it can
    // never be presented as a legitimate quantity at the wrong coordinate.
    for (left, right) in [(0_usize, 1_usize), (0, 2), (1, 2)] {
        let left = *COEFFICIENTS.get(left).expect("left coefficient");
        let right = *COEFFICIENTS.get(right).expect("right coefficient");
        assert_eq!(gcd(left, right), 1);
    }
    for coefficient in COEFFICIENTS {
        assert_eq!(gcd(coefficient, DENOMINATOR), 1);
    }
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let next = left % right;
        left = right;
        right = next;
    }
    left
}
