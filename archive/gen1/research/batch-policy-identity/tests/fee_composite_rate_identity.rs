//! The composite fee base's rate-representability seam, through the public
//! artifact API only.
//!
//! Two things must hold together, and neither is allowed to buy the other:
//!
//! * **Representability.** Every rate pair in the exact-basis-points band has a
//!   canonical 64-byte artifact, a distinct SHA-256 identity, and an exact
//!   round trip. Refusing to encode a rate the relation can verify would leave
//!   a policy that runs but cannot be named.
//! * **Digest discipline.** Landing that representability moved no frozen
//!   artifact. The zero-rate composite shape's bytes and digest are pinned here
//!   against literals taken from before the rates existed, and the two frozen
//!   production profiles are pinned against each other's difference.
//!
//! Representable is not decided. No production rate is proposed anywhere in
//! this file; the pairs below are the laboratory calibration of
//! `docs/decisions/REPORT_fee-base-selection_2026-08-20.md` §3.1 and the band
//! endpoints.

use clutch_batch::relation_v1::{FeeBaseV1, FrozenPolicyV1, FEE_BPS_DENOMINATOR};
use clutch_batch_policy_identity::general_clearing_v1::{
    GENERAL_CLEARING_FEE_SHAPE_V1, GENERAL_CLEARING_POLICY_V1,
};
use clutch_batch_policy_identity::{
    batch_policy_digest, canonical_batch_policy_bytes, decode_batch_policy, pack_composite_rates,
    unpack_composite_rates, Identity32V1, PolicyIdentityErrorV1,
};

fn rated(dispersion_bps: u32, floor_range_bps: u32) -> FrozenPolicyV1 {
    FrozenPolicyV1 {
        fee_base: FeeBaseV1::CompositeDispersionFloor {
            dispersion_bps,
            floor_range_bps,
        },
        ..GENERAL_CLEARING_FEE_SHAPE_V1
    }
}

/// The bytes and the SHA-256 of the frozen zero-rate composite shape, restated
/// as literals from before this lane existed.  If landing the rate packing had
/// moved either, this is where it would show.
#[test]
fn fee_shape_artifact_and_digest_are_untouched_by_rate_representability() {
    let bytes = canonical_batch_policy_bytes(&GENERAL_CLEARING_FEE_SHAPE_V1).unwrap();
    let mut expected = canonical_batch_policy_bytes(&GENERAL_CLEARING_POLICY_V1).unwrap();
    expected[22] = 0x02;
    assert_eq!(
        bytes, expected,
        "the fee shape must differ from the frozen zero-fee profile in exactly the fee tag"
    );
    // Both rate halves are zero words, so the whole rate slot and every
    // reserved byte stay zero.
    assert_eq!(&bytes[24..28], &[0u8; 4], "the rate word moved off zero");
    assert!(
        bytes[28..64].iter().all(|byte| *byte == 0),
        "the packing must not have reached into the reserved region"
    );
    assert_eq!(
        batch_policy_digest(&GENERAL_CLEARING_FEE_SHAPE_V1)
            .unwrap()
            .0,
        [
            0xac, 0xf9, 0x74, 0x7c, 0xf8, 0x45, 0x52, 0xea, 0x3c, 0x17, 0x7a, 0x16, 0x71, 0x03,
            0x33, 0xc9, 0x2f, 0x01, 0x83, 0xd9, 0x46, 0xa1, 0x21, 0x96, 0xf0, 0xd8, 0x6f, 0xc3,
            0x71, 0x34, 0xeb, 0x06
        ],
        "the frozen fee-shape identity moved"
    );
}

#[test]
fn every_admissible_rate_pair_round_trips_to_a_distinct_identity() {
    // The whole band would be 10,001^2 pairs; walk a deterministic lattice
    // through it plus both endpoints, which is enough to catch a dropped half,
    // a swapped half, or a truncated word.
    let mut seen: Vec<Identity32V1> = Vec::new();
    let mut samples = 0u32;
    let steps = [0u32, 1, 10, 40, 137, 2_500, 9_999, 10_000];
    for dispersion_bps in steps {
        for floor_range_bps in steps {
            let policy = rated(dispersion_bps, floor_range_bps);
            assert_eq!(policy.validate(), Ok(()));
            let bytes = canonical_batch_policy_bytes(&policy).unwrap();
            assert_eq!(
                decode_batch_policy(&bytes),
                Ok(policy),
                "({dispersion_bps}, {floor_range_bps}) lost a rate in the round trip"
            );
            // Every rate lives inside the one rate word; nothing else moved.
            let mut without_rates = bytes;
            without_rates[24..28].copy_from_slice(&[0u8; 4]);
            assert_eq!(
                without_rates,
                canonical_batch_policy_bytes(&GENERAL_CLEARING_FEE_SHAPE_V1).unwrap(),
                "a rate escaped the rate word"
            );
            let digest = batch_policy_digest(&policy).unwrap();
            assert!(
                !seen.contains(&digest),
                "({dispersion_bps}, {floor_range_bps}) collided with an earlier identity"
            );
            seen.push(digest);
            samples += 1;
        }
    }
    assert_eq!(samples, 64);
    // Exactly one of those pairs is the frozen shape, and only that one shares
    // its identity.
    let pinned = batch_policy_digest(&GENERAL_CLEARING_FEE_SHAPE_V1).unwrap();
    assert_eq!(
        seen.iter().filter(|digest| **digest == pinned).count(),
        1,
        "the frozen identity belongs to the zero pair alone"
    );
}

#[test]
fn rates_outside_the_basis_point_band_have_no_artifact() {
    for (dispersion_bps, floor_range_bps) in [
        (10_001u32, 0u32),
        (0, 10_001),
        (10_001, 10_001),
        (0x1_0000, 0),
        (0, 0x1_0000),
        (u32::MAX, u32::MAX),
    ] {
        let policy = rated(dispersion_bps, floor_range_bps);
        assert!(
            policy.validate().is_err(),
            "({dispersion_bps}, {floor_range_bps}) must not validate"
        );
        assert_eq!(
            canonical_batch_policy_bytes(&policy),
            Err(PolicyIdentityErrorV1::InvalidEnum),
            "({dispersion_bps}, {floor_range_bps}) must have no canonical bytes"
        );
    }
}

#[test]
fn a_tampered_rate_word_is_refused_rather_than_truncated() {
    let bytes = canonical_batch_policy_bytes(&rated(40, 10)).unwrap();
    for half in [24usize, 26] {
        let mut over = bytes;
        // 10,001 — one past the denominator.
        over[half] = 0x11;
        over[half + 1] = 0x27;
        assert_eq!(
            decode_batch_policy(&over),
            Err(PolicyIdentityErrorV1::InvalidEnum),
            "an over-denominator half must refuse, not saturate"
        );
        let mut wide = bytes;
        wide[half] = 0xff;
        wide[half + 1] = 0xff;
        assert_eq!(
            decode_batch_policy(&wide),
            Err(PolicyIdentityErrorV1::InvalidEnum)
        );
    }
    // The reserved region is still reserved: nothing about the packing opened
    // it up.
    let mut reserved = bytes;
    reserved[28] = 1;
    assert_eq!(
        decode_batch_policy(&reserved),
        Err(PolicyIdentityErrorV1::NonCanonicalPadding)
    );
}

#[test]
fn the_packing_is_a_bijection_on_the_admissible_band() {
    let bound = FEE_BPS_DENOMINATOR as u32;
    for dispersion_bps in [0u32, 1, 40, 9_999, bound] {
        for floor_range_bps in [0u32, 1, 10, 9_999, bound] {
            let word = pack_composite_rates(dispersion_bps, floor_range_bps);
            assert_eq!(
                unpack_composite_rates(word),
                Some((dispersion_bps, floor_range_bps))
            );
        }
    }
    // The zero pair is the zero word — the reason the frozen artifact did not
    // move.
    assert_eq!(pack_composite_rates(0, 0), 0);
    assert_eq!(unpack_composite_rates(0), Some((0, 0)));
    // A dispersion-only rate is its own bare value, so the artifact of a
    // dispersion-only profile is byte-identical to what the pre-packing codec
    // would have written for it.
    assert_eq!(pack_composite_rates(40, 0), 40);
    // Outside the band there is no unpacking at all.
    assert_eq!(unpack_composite_rates(10_001), None);
    assert_eq!(unpack_composite_rates(10_001 << 16), None);
    assert_eq!(unpack_composite_rates(u32::MAX), None);
}

#[test]
fn the_frozen_production_profiles_still_pin_the_zero_rate_pair() {
    // The discipline, restated as an assertion: nothing in this crate names a
    // production rate.  The rate decision is ember's, strictly after.
    assert_eq!(GENERAL_CLEARING_POLICY_V1.fee_base, FeeBaseV1::None);
    assert_eq!(
        GENERAL_CLEARING_FEE_SHAPE_V1.fee_base,
        FeeBaseV1::CompositeDispersionFloor {
            dispersion_bps: 0,
            floor_range_bps: 0,
        }
    );
    assert_ne!(
        batch_policy_digest(&GENERAL_CLEARING_POLICY_V1).unwrap(),
        batch_policy_digest(&GENERAL_CLEARING_FEE_SHAPE_V1).unwrap()
    );
}
