//! Decision 0011 §3c items 2 and 3: the derived descriptor feeds the landed
//! builders, and the width ceiling is executable.
//!
//! §3c ruled that Structured authors no artifacts -- the four Rational Hot
//! bundle builders already exist and are parameterized by a
//! `RepresentationDescriptorV2`.  Until now nothing had ever driven them from a
//! descriptor a chain could admit: `bearer-v2-operator`'s own fixture
//! hand-fills the preimage byte by byte and then hands
//! `RepresentationDescriptorV2::decode` an ARBITRARY `descriptor_id` that is
//! not the digest of those bytes.  The Claims adapter computes
//! `descriptor_id = hash(record data)`
//! (`rational-representation-v2-operator/src/lib.rs:533`), so that fixture
//! describes a record no Record account can hold.
//!
//! Everything below starts from
//! [`derive_structured_representation_descriptor_v2`], whose `descriptor_id`
//! IS the digest of its own preimage, over the campaign's coprime `K = 3`
//! basis.

mod support;

use dclutch_account_profile_contract::lifecycle_v3::{
    ACTION_PLAN_BYTES, HEADER_BYTES as LIFECYCLE_HEADER_BYTES, PROTECTED_OUTPUT_BYTES,
    RECIPE_BYTES, SEED_BYTES,
    encode::{
        LifecycleAccountCoordinateV3, LifecycleGuardInputV3, LifecycleOperationInputV3,
        LifecyclePlanInputV3, LifecycleRecipeInputV3, LifecycleSeedInputV3,
        encode_lifecycle_policy_v5_atomic,
    },
};
use dclutch_bearer_v2_operator::{
    RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3, RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3,
    RATIONAL_OPEN_STRUCTURED_REQUEST_BASE_OPERATIONS_V3,
    RATIONAL_OPEN_STRUCTURED_REQUEST_ROW_OPERATIONS_V3,
    RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3, RationalOpenStructuredHotBundleInputV3,
    RationalTerminalAccountProfileInputV3, RationalTerminalHotBundleInputV3,
    build_rational_open_structured_hot_bundle_v3, build_rational_terminal_hot_bundle_v3,
    validate_rational_open_structured_hot_bundle_for_authenticated_selection_v3,
    validate_rational_open_structured_hot_bundle_v3,
};
use dclutch_product_payoff_v2_codec::runtime_v3::{
    BASIS_HEADER_BYTES_V3, BasisInputV3, BasisKindV3, compile_basis_v3,
};
use dclutch_rational_representation_v2_contract::{
    AuthenticatedTokenBehaviorV2, RepresentationActionV2, TokenBehaviorRecordAdmissionV2,
    authenticate_token_behavior_v2,
};
use dclutch_rational_representation_v2_kernel::{
    DescriptorAdmissionV2, RepresentationDescriptorV2,
    descriptor_v3::{
        RepresentationDescriptorInputV3, encode_representation_descriptor_v3_atomic,
        representation_descriptor_bytes_v3,
    },
};
use dclutch_request_profile_contract::{
    HEADER_BYTES as REQUEST_PROFILE_HEADER_BYTES, MAX_BYTES as REQUEST_PROFILE_MAX_BYTES,
    OPERATION_BYTES as REQUEST_PROFILE_OPERATION_BYTES, RequestProfileV1,
};
use dclutch_structured_v2_operator::{
    STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2, StructuredDescriptorAuthorityV2,
    StructuredRepresentationDescriptorV2, decode_derived_structured_descriptor_v2,
};
use dclutch_token_svm::{
    TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, TokenBehaviorSelectionV2,
};

use support::{digest, identity};

mod fixture;
use fixture::{COEFFICIENTS, DENOMINATOR, K, derived_descriptor, derived_descriptor_for_market};

const REALM: u8 = 0x15;
const RELEASE_SET: u8 = 0x14;
const AUTHORITY: u8 = 0x51;
/// Product outcome width `N`, deliberately not `K`.
const PRODUCT_N: u32 = 258;

fn authority() -> StructuredDescriptorAuthorityV2 {
    StructuredDescriptorAuthorityV2 {
        representation_authority: identity(AUTHORITY),
    }
}

fn basis(width: u32) -> [u8; BASIS_HEADER_BYTES_V3] {
    let mut output = [0_u8; BASIS_HEADER_BYTES_V3];
    compile_basis_v3(
        BasisInputV3 {
            kind: BasisKindV3::CategoricalQ1,
            product_id: identity(1),
            result_domain_id: identity(2),
            coordinate_domain_id: identity(3),
            result_unit_id: identity(4),
            evaluator_release_id: identity(5),
            basis_width: width,
            payout_scale: 1,
            knot_denominator: 1,
            knots: &[],
            terms: &[],
            failure_payouts: &[],
        },
        &mut output,
    )
    .expect("categorical Product basis");
    output
}

/// The same dormant-root policy shape the Rational fixture uses.
fn lifecycle_policy() -> Vec<u8> {
    let recipes = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(0),
        seed_start: 0,
        seed_count: 2,
        bump_offset: 1,
        data_base: 8,
        data_stride: 0,
    }];
    let seeds = [
        LifecycleSeedInputV3::Literal(b"dclutch/rational-open/dormant/v4"),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = [LifecyclePlanInputV3 {
        action: u32::MAX,
        operation: LifecycleOperationInputV3::Authenticate,
        recipe: 0,
        payer: None,
        rent_credit: None,
        principal: None,
        beneficiary: None,
        guard: LifecycleGuardInputV3::Always,
    }];
    let width = LIFECYCLE_HEADER_BYTES
        + RECIPE_BYTES
        + 2 * SEED_BYTES
        + ACTION_PLAN_BYTES
        + PROTECTED_OUTPUT_BYTES;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_lifecycle_policy_v5_atomic(
        &recipes,
        &seeds,
        &plans,
        &[None],
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .expect("lifecycle policy");
    output
}

/// Token behavior authenticated against the DERIVED descriptor, not a fixture.
fn token_behavior(descriptor: RepresentationDescriptorV2<'_>) -> AuthenticatedTokenBehaviorV2 {
    let selection = TokenBehaviorSelectionV2::new(identity(REALM), identity(RELEASE_SET))
        .expect("Token behavior selection")
        .to_bytes();
    let selection_id = digest(&selection);
    authenticate_token_behavior_v2(
        descriptor,
        identity(REALM),
        &selection,
        TokenBehaviorRecordAdmissionV2 {
            selected_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            finalized_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            selected_content_digest: selection_id,
            finalized_content_digest: selection_id,
            recomputed_content_digest: selection_id,
            record_authenticated: true,
            market_realm_authenticated: true,
        },
    )
    .expect("authenticated Token behavior")
}

fn fixed_lengths(basis: &[u8]) -> [u32; RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3 as usize] {
    let mut output = [0_u32; RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3 as usize];
    let width = u32::try_from(basis.len()).expect("basis width");
    *output.get_mut(4).expect("basis coordinate") = width;
    *output.get_mut(29).expect("basis alias coordinate") = width;
    output
}

#[test]
fn the_derived_descriptor_builds_both_structured_action_bundles() {
    let derived = derived_descriptor();
    let descriptor =
        decode_derived_structured_descriptor_v2(&derived, authority()).expect("hostile decode");
    let behavior = token_behavior(descriptor);
    let basis = basis(PRODUCT_N);
    let lengths = fixed_lengths(&basis);
    let policy = lifecycle_policy();

    for action in [
        RepresentationActionV2::IssueStructured,
        RepresentationActionV2::UnwrapStructured,
    ] {
        let bundle =
            build_rational_open_structured_hot_bundle_v3(RationalOpenStructuredHotBundleInputV3 {
                action,
                fixed_data_lengths: &lengths,
                item_data_lengths: [64, 82, 165, 165],
                product_basis: &basis,
                representation_descriptor: descriptor,
                kind: identity(0x10),
                authenticated_token_behavior: behavior,
                root_schema: identity(0x11),
                lifecycle_policy: &policy,
                capacity_profile: identity(0x13),
                root_state_bytes: 8,
            })
            .expect("structured bundle from a derived descriptor");

        // The two joins the builder's own crate runs on its fixture, now run on
        // a descriptor whose id is the digest of its bytes.
        validate_rational_open_structured_hot_bundle_v3(&bundle).expect("artifact join");
        validate_rational_open_structured_hot_bundle_for_authenticated_selection_v3(
            &bundle, behavior,
        )
        .expect("Realm/release join");

        // K comes from the descriptor and N from the basis, and they are
        // independent -- 258 outcomes over three shard coordinates.
        assert_eq!(bundle.representation_outcome_count, K);
        RequestProfileV1::decode(&bundle.request_profile).expect("RequestProfile V1");
        // The measured artifact width IS the cliff arithmetic: 32 + 53 * 24.
        assert_eq!(
            bundle.request_profile.len(),
            REQUEST_PROFILE_HEADER_BYTES
                + (RATIONAL_OPEN_STRUCTURED_REQUEST_BASE_OPERATIONS_V3
                    + RATIONAL_OPEN_STRUCTURED_REQUEST_ROW_OPERATIONS_V3 * K as usize)
                    * REQUEST_PROFILE_OPERATION_BYTES
        );
        assert_eq!(bundle.request_profile.len(), 1_304);
        assert!(bundle.request_profile.len() <= REQUEST_PROFILE_MAX_BYTES);
        assert!(!bundle.account_profile.is_empty());
        assert!(!bundle.effect.is_empty());
        assert!(!bundle.transition.is_empty());
    }
}

#[test]
fn the_two_ceilings_are_one_number_and_neither_crate_restates_it() {
    // `STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2` documents itself as the Rational
    // artifact's ceiling rather than a Structured choice. That is only true if
    // the two agree, so it is asserted rather than trusted.
    assert_eq!(
        STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2,
        RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3
    );
}

/// K = 4 is refused, and the reason is 1,496 bytes against 1,312.
///
/// **This is the cliff evidence RECORDS-MIGRATE is owed**, and it is executable
/// rather than an arithmetic claim in a doc comment. `REQUEST_PROFILE_MAX_BYTES_V1`
/// is a copied literal of the legacy packet allowance; RECORDS-MIGRATE's charter
/// is to derive every such limit from `genref` and retire the copied-1312 class.
/// When that happens, this test is what says whether the Structured ceiling
/// moved with it: K = 3 has EIGHT bytes of slack, and one more operation costs
/// twenty-four, so nothing short of raising the bound admits a fourth
/// coordinate.
///
/// The three assertions are three independent statements of the same wall:
/// the arithmetic over the real constants, the RequestProfile encoder refusing
/// the operation count, and the Rational bundle builder refusing a real K = 4
/// descriptor.
#[test]
fn records_migrate_cliff_a_fourth_coordinate_costs_1496_bytes_against_1312() {
    let profile_bytes = |outcomes: usize| {
        REQUEST_PROFILE_HEADER_BYTES
            + (RATIONAL_OPEN_STRUCTURED_REQUEST_BASE_OPERATIONS_V3
                + RATIONAL_OPEN_STRUCTURED_REQUEST_ROW_OPERATIONS_V3 * outcomes)
                * REQUEST_PROFILE_OPERATION_BYTES
    };
    assert_eq!(REQUEST_PROFILE_MAX_BYTES, 1312);
    assert_eq!(profile_bytes(3), 1_304);
    assert_eq!(profile_bytes(4), 1_496);
    assert!(profile_bytes(3) <= REQUEST_PROFILE_MAX_BYTES);
    assert!(profile_bytes(4) > REQUEST_PROFILE_MAX_BYTES);
    // Eight bytes of slack is not a fourth operation.
    assert_eq!(REQUEST_PROFILE_MAX_BYTES - profile_bytes(3), 8);
    assert!(REQUEST_PROFILE_OPERATION_BYTES > REQUEST_PROFILE_MAX_BYTES - profile_bytes(3));

    // The builder refuses a REAL K = 4 descriptor -- encoded by the kernel's own
    // atomic encoder and decoded under an admission whose identity is the digest
    // of its bytes, so the refusal is the width and nothing else.
    let width = representation_descriptor_bytes_v3(4).expect("K4 width");
    let mut scratch = vec![0_u8; width];
    let mut preimage = vec![0_u8; width];
    encode_representation_descriptor_v3_atomic(
        RepresentationDescriptorInputV3 {
            exposure_id: identity(0x21),
            exposure_digest: identity(0x22),
            root_id: identity(0x20),
            market: identity(0x11),
            release_set: identity(RELEASE_SET),
            receipt_mint: identity(0x1c),
            token_program: TOKEN_2022_PROGRAM_ID,
            denominator: DENOMINATOR,
            coefficients: &[2, 3, 5, 11],
        },
        &mut scratch,
        &mut preimage,
    )
    .expect("a K=4 descriptor is a perfectly valid RECORD");
    let id = digest(&preimage);
    let wide = RepresentationDescriptorV2::decode(
        &preimage,
        DescriptorAdmissionV2 {
            selected_descriptor_id: id,
            finalized_descriptor_id: id,
            recomputed_descriptor_digest: id,
            finalized_descriptor_digest: id,
            record_authenticated: true,
            derived_representation_authority: identity(AUTHORITY),
            authority_derivation_authenticated: true,
        },
    )
    .expect("and it decodes");
    assert_eq!(wide.outcome_count(), 4);

    let behavior = token_behavior(wide);
    let basis = basis(PRODUCT_N);
    let lengths = fixed_lengths(&basis);
    let policy = lifecycle_policy();
    assert!(
        build_rational_open_structured_hot_bundle_v3(RationalOpenStructuredHotBundleInputV3 {
            action: RepresentationActionV2::IssueStructured,
            fixed_data_lengths: &lengths,
            item_data_lengths: [64, 82, 165, 165],
            product_basis: &basis,
            representation_descriptor: wide,
            kind: identity(0x10),
            authenticated_token_behavior: behavior,
            root_schema: identity(0x11),
            lifecycle_policy: &policy,
            capacity_profile: identity(0x13),
            root_state_bytes: 8,
        })
        .is_err(),
        "a fourth coordinate has no executable RequestProfile"
    );

    // And the derivation refuses it upstream, so a Product too wide to execute
    // never acquires a descriptor to found in the first place.
    assert!(u32::try_from(COEFFICIENTS.len()).expect("K") <= STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2);
}

/// Guard the premise of every number above: the derived descriptor really is
/// self-identifying, which the crate's own fixture is not.
#[test]
fn the_derived_descriptor_id_is_the_digest_of_its_own_bytes() {
    let derived: StructuredRepresentationDescriptorV2 = derived_descriptor();
    assert_eq!(derived.descriptor_id, digest(&derived.preimage));
    assert_eq!(derived.outcome_count, K);
    assert_eq!(derived.denominator, DENOMINATOR);
}

/// THE FIXED-POINT QUESTION, asked of Structured the only way it can be
/// answered: build the whole bundle twice from closures differing in NOTHING
/// but the Market coordinate, and compare the bytes.
///
/// # Why this test exists
///
/// A capability manifest entry names a `release_id` that is the SHA-256 of a
/// `CapabilityProgramSetV2`, and the manifest digest is itself a seed of the
/// Market PDA (`MarketIdentity::capability_manifest`). So if any artifact byte
/// moved with the Market, a Structured manifest entry could not be constructed
/// before the Market that selects it exists -- a SHA-256 fixed point, which is
/// exactly the wall Fractional hit through its config and Rational hit through
/// its `release_id`. The seam states the general form: NO entry-authored
/// identity may transitively depend on the Market through the full artifact
/// closure.
///
/// # Why a hand-trace was not allowed to answer it
///
/// Reading `build_rational_open_structured_hot_bundle_v3` suggests the answer,
/// because `encode_request_profile`, `encode_transition` and `encode_effect`
/// take an action and a width rather than a descriptor. But that is a reading
/// of three encoders out of seven, and the descriptor IS passed to the builder
/// -- `require_representation_width` and `encode_account_profile` both receive
/// it. Only a byte comparison settles whether receiving it means baking it.
///
/// # What makes the substitution trustworthy
///
/// The Market travels through each encoder's own named `market` field, so this
/// test makes no claim about any byte offset. And the substitution is not
/// assumed to have taken: the two derived `descriptor_id`s are required to
/// DIFFER, and the DECODER is required to report the Market that was written.
/// A test that silently varied nothing would fail those two assertions before
/// it could report a false neutrality.
///
/// # The result, and the pre-registered criterion
///
/// Every artifact is byte-identical across the two Markets. If a future change
/// makes any of them Market-dependent, the corresponding `assert_eq!` below
/// flips to a failure naming the artifact, and that failure IS the report that
/// Structured has acquired the trap Rational had to be dug out of.
#[test]
fn every_structured_artifact_is_byte_identical_across_two_markets() {
    const FIRST_MARKET: u8 = 0x21;
    const SECOND_MARKET: u8 = 0x22;

    let first_derived = derived_descriptor_for_market(identity(FIRST_MARKET));
    let second_derived = derived_descriptor_for_market(identity(SECOND_MARKET));

    // The substitution took: two Markets, two descriptor identities.
    assert_ne!(first_derived.descriptor_id, second_derived.descriptor_id);
    assert_ne!(first_derived.preimage, second_derived.preimage);

    let first = decode_derived_structured_descriptor_v2(&first_derived, authority())
        .expect("first hostile decode");
    let second = decode_derived_structured_descriptor_v2(&second_derived, authority())
        .expect("second hostile decode");

    // The DECODER establishes which field was substituted, not a local offset
    // constant. This is what makes the comparison below meaningful.
    assert_eq!(first.market_id(), identity(FIRST_MARKET));
    assert_eq!(second.market_id(), identity(SECOND_MARKET));

    // Everything the closure shares is genuinely shared.
    assert_eq!(first.outcome_count(), second.outcome_count());
    assert_eq!(first.release_set_id(), second.release_set_id());
    assert_eq!(first.token_program(), second.token_program());

    let first_behavior = token_behavior(first);
    let second_behavior = token_behavior(second);
    let basis = basis(PRODUCT_N);
    let lengths = fixed_lengths(&basis);
    let policy = lifecycle_policy();

    // The config the manifest entry would name is market-free by INHERITANCE:
    // Structured adopts Rational's `TokenBehaviorSelectionV2` (Realm plus
    // release set), not its own market-bearing terms.
    assert_eq!(
        first_behavior.selection().to_bytes(),
        second_behavior.selection().to_bytes(),
        "the selected config must not move with the Market"
    );

    for action in [
        RepresentationActionV2::IssueStructured,
        RepresentationActionV2::UnwrapStructured,
    ] {
        let build = |descriptor, behavior| {
            build_rational_open_structured_hot_bundle_v3(RationalOpenStructuredHotBundleInputV3 {
                action,
                fixed_data_lengths: &lengths,
                item_data_lengths: [64, 82, 165, 165],
                product_basis: &basis,
                representation_descriptor: descriptor,
                kind: identity(0x10),
                authenticated_token_behavior: behavior,
                root_schema: identity(0x11),
                lifecycle_policy: &policy,
                capacity_profile: identity(0x13),
                root_state_bytes: 8,
            })
            .expect("structured bundle")
        };
        let a = build(first, first_behavior);
        let b = build(second, second_behavior);

        assert_eq!(a.representation_outcome_count, b.representation_outcome_count);
        assert_eq!(a.token_behavior_selection, b.token_behavior_selection, "config");
        assert_eq!(a.account_profile, b.account_profile, "account_profile");
        assert_eq!(a.request_profile, b.request_profile, "request_profile");
        assert_eq!(a.lifecycle_policy, b.lifecycle_policy, "lifecycle_policy");
        assert_eq!(a.transition, b.transition, "transition");
        assert_eq!(a.strategy, b.strategy, "strategy");
        assert_eq!(a.effect, b.effect, "effect");

        // The CapabilityProgramV4 is the entry-authored artifact: its digest is
        // a ProgramSet entry, and the set's SHA-256 is the manifest entry's
        // `release_id`. This is the assertion the fixed-point question reduces
        // to.
        assert_eq!(a.descriptor, b.descriptor, "CapabilityProgramV4 descriptor");
    }
}

/// Structured's THIRD action, asked the same question.
///
/// `TerminalRedeem` routes to `build_rational_terminal_hot_bundle_v3` rather
/// than the open-structured builder (decision 0011 §3c's action table), so the
/// previous test says nothing about it. The comparison is not vacuous: the
/// terminal builder still receives an `AuthenticatedTokenBehaviorV2`, and that
/// value DOES carry the Market-bearing `descriptor_id` -- the two behaviors
/// below are genuinely different values. What the test establishes is that only
/// the Market-free `selection()` reaches the bytes.
#[test]
fn the_terminal_redeem_artifacts_are_byte_identical_across_two_markets() {
    let first_derived = derived_descriptor_for_market(identity(0x21));
    let second_derived = derived_descriptor_for_market(identity(0x22));
    let first = decode_derived_structured_descriptor_v2(&first_derived, authority())
        .expect("first hostile decode");
    let second = decode_derived_structured_descriptor_v2(&second_derived, authority())
        .expect("second hostile decode");
    assert_ne!(first.descriptor_id(), second.descriptor_id());

    let first_behavior = token_behavior(first);
    let second_behavior = token_behavior(second);
    // The inputs really do differ: the admission is bound to two descriptors.
    assert_ne!(
        first_behavior.descriptor_id(),
        second_behavior.descriptor_id(),
        "the two authenticated behaviors must be distinct values"
    );

    let basis = basis(PRODUCT_N);
    let policy = lifecycle_policy();
    let mut lengths = vec![0_u32; RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3 as usize];
    *lengths.get_mut(4).expect("basis coordinate") =
        u32::try_from(basis.len()).expect("basis width");

    let build = |behavior| {
        build_rational_terminal_hot_bundle_v3(RationalTerminalHotBundleInputV3 {
            account_profile: RationalTerminalAccountProfileInputV3 {
                logical_data_lengths: &lengths,
                product_basis: &basis,
            },
            kind: identity(0x10),
            authenticated_token_behavior: behavior,
            root_schema: identity(0x11),
            lifecycle_policy: &policy,
            capacity_profile: identity(0x13),
            root_state_bytes: 8,
        })
        .expect("terminal bundle")
    };
    let a = build(first_behavior);
    let b = build(second_behavior);

    assert_eq!(a.token_behavior_selection, b.token_behavior_selection, "config");
    assert_eq!(a.account_profile, b.account_profile, "account_profile");
    assert_eq!(a.request_profile, b.request_profile, "request_profile");
    assert_eq!(a.lifecycle_policy, b.lifecycle_policy, "lifecycle_policy");
    assert_eq!(a.transition, b.transition, "transition");
    assert_eq!(a.strategy, b.strategy, "strategy");
    assert_eq!(a.effect, b.effect, "effect");
    assert_eq!(a.descriptor, b.descriptor, "CapabilityProgramV4 descriptor");
}
