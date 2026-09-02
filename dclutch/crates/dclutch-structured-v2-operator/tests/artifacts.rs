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
        LifecyclePlanInputV3, LifecycleRecipeInputV3, LifecycleRefundSourceInputV3,
        LifecycleSeedInputV3, encode_lifecycle_policy_v5_atomic,
    },
};
use dclutch_bearer_v2_operator::{
    Error as BearerOperatorError, RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3,
    RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3, RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3,
    RATIONAL_OPEN_STRUCTURED_REQUEST_BASE_OPERATIONS_V3,
    RATIONAL_OPEN_STRUCTURED_REQUEST_ROW_OPERATIONS_V3, RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3,
    RationalOpenCapabilityProgramSetInputV3, RationalOpenCapabilityProgramSetInputV6,
    RationalOpenSelectedBundleInputV6, RationalOpenSelectedHotBundleInputV3,
    RationalOpenStructuredHotBundleInputV3, RationalOpenStructuredSelectedBundleInputV6,
    RationalTerminalAccountProfileInputV3, RationalTerminalHotBundleInputV3,
    RationalTerminalSelectedBundleInputV6, build_rational_open_capability_program_set_v3,
    build_rational_open_capability_program_set_v6, build_rational_open_selected_bundle_v6,
    build_rational_open_selected_hot_bundle_v3, build_rational_open_structured_hot_bundle_v3,
    build_rational_open_structured_selected_bundle_v6, build_rational_terminal_hot_bundle_v3,
    build_rational_terminal_selected_bundle_v6,
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
use dclutch_structured_v2_kernel::{
    STRUCTURED_CAPABILITY_KIND_ID_V2, STRUCTURED_CAPACITY_PROFILE_ID_V2,
};
use dclutch_structured_v2_operator::{
    STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2, StructuredDescriptorAuthorityV2,
    StructuredRepresentationDescriptorV2, decode_derived_structured_descriptor_v2,
};
use dclutch_token_svm::{
    TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_BYTES_V2,
    TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, TokenBehaviorSelectionV2,
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
            // Exempt by proof: degree 0 and 1 need no price gate,
            // and a digest offered alongside one is refused.
            price_gate_certificate_digest: [0_u8; 32],
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
        refund_source: LifecycleRefundSourceInputV3::Credit,
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
        // The measured artifact width IS the cliff arithmetic: 32 + 37 * 24.
        assert_eq!(
            bundle.request_profile.len(),
            REQUEST_PROFILE_HEADER_BYTES
                + (RATIONAL_OPEN_STRUCTURED_REQUEST_BASE_OPERATIONS_V3
                    + RATIONAL_OPEN_STRUCTURED_REQUEST_ROW_OPERATIONS_V3 * K as usize)
                    * REQUEST_PROFILE_OPERATION_BYTES
        );
        assert_eq!(bundle.request_profile.len(), 920);
        assert!(bundle.request_profile.len() <= REQUEST_PROFILE_MAX_BYTES);
        assert!(!bundle.account_profile.is_empty());
        assert!(!bundle.effect.is_empty());
        assert!(!bundle.transition.is_empty());
    }
}

#[test]
fn the_structured_ceiling_is_provisionally_below_the_artifact_ceiling_it_cites() {
    // These two WERE one number, asserted equal so that
    // `STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2`'s claim to be the Rational
    // artifact's ceiling rather than a Structured choice could be checked
    // rather than trusted. Physical ABI v3 separated them: the artifact ceiling
    // is DERIVED and moved 3 -> 6 when the request header became
    // action-conditional and three per-coordinate keys left the wire, while the
    // Structured child ceiling was deliberately NOT moved with it.
    //
    // So the relationship is now an inequality, and the gap is PROVISIONAL
    // debt, not a bound. Lifting plan: a K = 4 and a K = 5 Claims-direct route
    // must be driven end to end by the campaign -- fitting the RequestProfile
    // is necessary and not sufficient, because a wider K also costs compute
    // units and transaction bytes that only a run can measure. When they
    // execute, this constant rises to the lower of six and what executed; until
    // then it stays at three and this test says why.
    assert!(
        STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2 <= RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3
    );
    assert_eq!(RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3, 6);
    assert_eq!(STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2, 3);
}

/// A seventh coordinate is refused, and the reason is 1,400 bytes against 1,312.
///
/// **This is the cliff evidence RECORDS-MIGRATE is owed**, and it is executable
/// rather than an arithmetic claim in a doc comment. `REQUEST_PROFILE_MAX_BYTES_V1`
/// is a copied literal of the legacy packet allowance; RECORDS-MIGRATE's charter
/// is to derive every such limit from `genref` and retire the copied-1312 class.
/// When that happens, this test is what says whether the Structured ceiling
/// moved with it.
///
/// The wall MOVED with physical ABI v3, which is the whole point of keeping it
/// executable. Under v2 a base of 34 operations plus six per coordinate put the
/// cliff between K = 3 (1,304 bytes) and K = 4 (1,496). v3's action-conditional
/// header and its three departed per-coordinate keys cut the base to 22 and the
/// row to five, so K = 3 now costs 920 and the cliff moved to between K = 6
/// (1,280) and K = 7 (1,400). A test that had restated 1,304 as a literal would
/// have gone red saying only that a number changed; these assertions say which
/// number and why.
///
/// The four assertions are four independent statements of the same wall: the
/// arithmetic over the real constants, the derived ceiling agreeing with it, the
/// bundle builder ADMITTING a real K = 6 descriptor, and the same builder
/// refusing a real K = 7 one. The positive control is not decoration -- without
/// it, a builder broken for every K would pass the refusal half.
#[test]
fn records_migrate_cliff_a_seventh_coordinate_costs_1400_bytes_against_1312() {
    let profile_bytes = |outcomes: usize| {
        REQUEST_PROFILE_HEADER_BYTES
            + (RATIONAL_OPEN_STRUCTURED_REQUEST_BASE_OPERATIONS_V3
                + RATIONAL_OPEN_STRUCTURED_REQUEST_ROW_OPERATIONS_V3 * outcomes)
                * REQUEST_PROFILE_OPERATION_BYTES
    };
    assert_eq!(REQUEST_PROFILE_MAX_BYTES, 1312);
    assert_eq!(profile_bytes(3), 920);
    assert_eq!(profile_bytes(6), 1_280);
    assert_eq!(profile_bytes(7), 1_400);
    assert!(profile_bytes(6) <= REQUEST_PROFILE_MAX_BYTES);
    assert!(profile_bytes(7) > REQUEST_PROFILE_MAX_BYTES);
    // Thirty-two bytes of slack is not five more operations.
    assert_eq!(REQUEST_PROFILE_MAX_BYTES - profile_bytes(6), 32);
    assert!(
        RATIONAL_OPEN_STRUCTURED_REQUEST_ROW_OPERATIONS_V3 * REQUEST_PROFILE_OPERATION_BYTES
            > REQUEST_PROFILE_MAX_BYTES - profile_bytes(6)
    );
    // The ceiling the crate publishes is this arithmetic and not a second copy.
    assert_eq!(
        RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3 as usize,
        (0..=16usize)
            .filter(|k| profile_bytes(*k) <= REQUEST_PROFILE_MAX_BYTES)
            .max()
            .expect("some K fits")
    );

    // Both halves build a REAL descriptor -- encoded by the kernel's own atomic
    // encoder and decoded under an admission whose identity is the digest of its
    // bytes, so a refusal is the width and nothing else.
    const WIDE_COEFFICIENTS: [u64; 7] = [2, 3, 5, 11, 13, 17, 19];
    let bundle_at = |outcomes: usize| {
        let width = representation_descriptor_bytes_v3(outcomes).expect("descriptor width");
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
                coefficients: &WIDE_COEFFICIENTS[..outcomes],
            },
            &mut scratch,
            &mut preimage,
        )
        .expect("any K is a perfectly valid RECORD");
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
        assert_eq!(wide.outcome_count(), outcomes as u32);

        let behavior = token_behavior(wide);
        let basis = basis(PRODUCT_N);
        let lengths = fixed_lengths(&basis);
        let policy = lifecycle_policy();
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
        .map(|bundle| bundle.request_profile.len())
    };

    // POSITIVE CONTROL: the sixth coordinate has an executable RequestProfile,
    // and it is exactly the width the arithmetic above predicts.
    assert_eq!(bundle_at(6), Ok(profile_bytes(6)));
    // And the seventh does not.
    // And the seventh does not -- refused by the CEILING, named as such. The
    // predecessor of this assertion was a bare `is_err()`, which would have
    // passed on any refusal the builder reached first; the ceiling conjunct
    // then sat inside a six-way `AccountProfileInput`, so even reading the code
    // did not say the cliff was what refused. Both halves are fixed here.
    assert_eq!(
        bundle_at(7),
        Err(BearerOperatorError::CoordinateCeiling {
            requested: 7,
            ceiling: RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3,
        })
    );

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

        assert_eq!(
            a.representation_outcome_count,
            b.representation_outcome_count
        );
        assert_eq!(
            a.token_behavior_selection, b.token_behavior_selection,
            "config"
        );
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

    assert_eq!(
        a.token_behavior_selection, b.token_behavior_selection,
        "config"
    );
    assert_eq!(a.account_profile, b.account_profile, "account_profile");
    assert_eq!(a.request_profile, b.request_profile, "request_profile");
    assert_eq!(a.lifecycle_policy, b.lifecycle_policy, "lifecycle_policy");
    assert_eq!(a.transition, b.transition, "transition");
    assert_eq!(a.strategy, b.strategy, "strategy");
    assert_eq!(a.effect, b.effect, "effect");
    assert_eq!(a.descriptor, b.descriptor, "CapabilityProgramV4 descriptor");
}

/// THE WHOLE RELEASE, COMPILED TWICE. This is the assertion the seam's
/// invariant actually reduces to for Structured.
///
/// The previous two tests compare artifacts. This one compares the
/// `CapabilityProgramSetV2` those artifacts are assembled into, and its
/// SHA-256 -- which is the `release_id` a founded Market's capability manifest
/// entry names, and whose manifest digest is a seed of the Market PDA. If this
/// identity is stable across Markets then a Structured capability can be
/// compiled, published and finalized BEFORE the Market that selects it exists,
/// and the fixed point the seam refuses to express is simply absent.
///
/// The set is the complete five-action open capability -- Denominate,
/// Reconstitute, IssueStructured, UnwrapStructured, RedeemTerminal -- because
/// those five are one wire at one selector offset, and because Denominate is
/// where the shards IssueStructured consumes are created. A narrowed
/// three-action Structured set encodes legally (the codec requires only
/// strictly ascending selectors, and 3/4/5 are ascending) but would select a
/// capability whose Issue consumes a resource its own market has no verb to
/// make.
///
/// Two of the five bundles are driven by the Structured-derived descriptor and
/// therefore move with the Market at their INPUT; the other three receive only
/// the authenticated Token behavior. All five contribute a descriptor digest to
/// the set. So this is not a restatement of the earlier tests: it is the join.
#[test]
fn the_whole_five_action_program_set_has_one_identity_across_two_markets() {
    let compile = |market: u8| {
        let derived = derived_descriptor_for_market(identity(market));
        let descriptor =
            decode_derived_structured_descriptor_v2(&derived, authority()).expect("hostile decode");
        let behavior = token_behavior(descriptor);
        let basis = basis(PRODUCT_N);
        let policy = lifecycle_policy();

        let mut selected_lengths = vec![0_u32; RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3 as usize];
        let width = u32::try_from(basis.len()).expect("basis width");
        selected_lengths[4] = width;
        selected_lengths[29] = width;
        let structured_lengths = fixed_lengths(&basis);
        let mut terminal_lengths = vec![0_u32; RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3 as usize];
        terminal_lengths[1] =
            u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("selection width");
        terminal_lengths[4] = width;
        terminal_lengths[29] = width;

        let selected = |action| {
            build_rational_open_selected_hot_bundle_v3(RationalOpenSelectedHotBundleInputV3 {
                action,
                logical_data_lengths: &selected_lengths,
                product_basis: &basis,
                kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
                authenticated_token_behavior: behavior,
                root_schema: identity(0x11),
                lifecycle_policy: &policy,
                capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
                root_state_bytes: 8,
            })
            .expect("selected bundle")
        };
        let structured = |action| {
            build_rational_open_structured_hot_bundle_v3(RationalOpenStructuredHotBundleInputV3 {
                action,
                fixed_data_lengths: &structured_lengths,
                item_data_lengths: [64, 82, 165, 165],
                product_basis: &basis,
                representation_descriptor: descriptor,
                kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
                authenticated_token_behavior: behavior,
                root_schema: identity(0x11),
                lifecycle_policy: &policy,
                capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
                root_state_bytes: 8,
            })
            .expect("structured bundle")
        };
        let denominate = selected(RepresentationActionV2::Denominate);
        let reconstitute = selected(RepresentationActionV2::Reconstitute);
        let issue = structured(RepresentationActionV2::IssueStructured);
        let unwrap = structured(RepresentationActionV2::UnwrapStructured);
        let redeem = build_rational_terminal_hot_bundle_v3(RationalTerminalHotBundleInputV3 {
            account_profile: RationalTerminalAccountProfileInputV3 {
                logical_data_lengths: &terminal_lengths,
                product_basis: &basis,
            },
            kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
            authenticated_token_behavior: behavior,
            root_schema: identity(0x11),
            lifecycle_policy: &policy,
            capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
            root_state_bytes: 8,
        })
        .expect("terminal bundle");

        let set = build_rational_open_capability_program_set_v3(
            RationalOpenCapabilityProgramSetInputV3 {
                authenticated_token_behavior: behavior,
                denominate: &denominate,
                reconstitute: &reconstitute,
                issue_structured: &issue,
                unwrap_structured: &unwrap,
                redeem_terminal: &redeem,
            },
        )
        .expect("five-action capability set");
        (derived.descriptor_id, set)
    };

    let (first_descriptor_id, first) = compile(0x21);
    let (second_descriptor_id, second) = compile(0x22);

    // The positive control: the two compilations really were driven from two
    // different Markets, and that difference really did reach a content
    // identity. Without this the equalities below could be reporting that
    // nothing varied.
    assert_ne!(first_descriptor_id, second_descriptor_id);

    // *** THE FIXED-POINT ANSWER. ***
    assert_eq!(
        first.program_set_id, second.program_set_id,
        "the release_id a Market manifest entry names must not move with the Market"
    );
    assert_eq!(first.program_set, second.program_set, "program set bytes");
    assert_eq!(
        first.token_behavior_selection, second.token_behavior_selection,
        "the config_id a Market manifest entry names must not move with the Market"
    );
    assert_eq!(
        first.token_behavior_selection_id,
        second.token_behavior_selection_id
    );

    // The set really is the five-action one. Its entry count is already pinned
    // inside the builder -- `validate_rational_open_capability_program_set_v3`
    // requires `entry_count() == 5` and proves each action routes to its own
    // descriptor through the table -- so this asserts the width the five
    // entries imply rather than restating a check the builder already ran.
    assert_eq!(first.program_set.len(), second.program_set.len());
    assert!(!first.program_set.is_empty());
}

/// THE POINT OF THE WHOLE EXERCISE: the release compiled with NO Market
/// anywhere in scope is the same release, byte for byte.
///
/// The preceding tests establish that the emitted identity does not MOVE with
/// the Market. That is necessary but not sufficient for founding, because the
/// V3 builders still cannot RUN before a Market exists: they take a
/// `RepresentationDescriptorV2` and an `AuthenticatedTokenBehaviorV2`, and both
/// require a finalized descriptor whose identity is the SHA-256 of a preimage
/// carrying the Core Market. That is a CONSTRUCTIBILITY wall, not a
/// byte-dependence one, and it is the last thing between Structured and a
/// selectable release.
///
/// The V6 builders remove it by taking the two things the V3 path actually
/// consulted -- the representation width and the immutable
/// `TokenBehaviorSelectionV2` -- and nothing else. This test is the acceptance
/// criterion for that narrowing, and it is the strongest one available: the
/// pre-founding path must reproduce the Market-bound path EXACTLY, including
/// the `program_set_id` a capability manifest entry names as `release_id`.
///
/// If the narrowing had dropped or altered any input that reaches an artifact,
/// this comparison is what would say so.
#[test]
fn the_market_free_path_compiles_the_identical_release() {
    let basis = basis(PRODUCT_N);
    let policy = lifecycle_policy();
    let selection =
        TokenBehaviorSelectionV2::new(identity(REALM), identity(RELEASE_SET)).expect("selection");

    let mut selected_lengths = vec![0_u32; RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3 as usize];
    let width = u32::try_from(basis.len()).expect("basis width");
    selected_lengths[4] = width;
    selected_lengths[29] = width;
    let structured_lengths = fixed_lengths(&basis);
    let mut terminal_lengths = vec![0_u32; RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3 as usize];
    terminal_lengths[1] =
        u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("selection width");
    terminal_lengths[4] = width;
    terminal_lengths[29] = width;

    // Not one value below is, or contains, a Market. There is no descriptor to
    // derive and no admission to authenticate, so there is nothing a Market
    // could enter through.
    let selected = |action| {
        build_rational_open_selected_bundle_v6(RationalOpenSelectedBundleInputV6 {
            action,
            logical_data_lengths: &selected_lengths,
            product_basis: &basis,
            kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
            token_behavior_selection: selection,
            root_schema: identity(0x11),
            lifecycle_policy: &policy,
            capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
            root_state_bytes: 8,
        })
        .expect("market-free selected bundle")
    };
    let structured = |action| {
        build_rational_open_structured_selected_bundle_v6(
            RationalOpenStructuredSelectedBundleInputV6 {
                action,
                fixed_data_lengths: &structured_lengths,
                item_data_lengths: [64, 82, 165, 165],
                product_basis: &basis,
                representation_outcome_count: K,
                token_behavior_selection: selection,
                kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
                root_schema: identity(0x11),
                lifecycle_policy: &policy,
                capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
                root_state_bytes: 8,
            },
        )
        .expect("market-free structured bundle")
    };
    let denominate = selected(RepresentationActionV2::Denominate);
    let reconstitute = selected(RepresentationActionV2::Reconstitute);
    let issue = structured(RepresentationActionV2::IssueStructured);
    let unwrap = structured(RepresentationActionV2::UnwrapStructured);
    let redeem =
        build_rational_terminal_selected_bundle_v6(RationalTerminalSelectedBundleInputV6 {
            account_profile: RationalTerminalAccountProfileInputV3 {
                logical_data_lengths: &terminal_lengths,
                product_basis: &basis,
            },
            kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
            token_behavior_selection: selection,
            root_schema: identity(0x11),
            lifecycle_policy: &policy,
            capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
            root_state_bytes: 8,
        })
        .expect("market-free terminal bundle");

    let market_free =
        build_rational_open_capability_program_set_v6(RationalOpenCapabilityProgramSetInputV6 {
            token_behavior_selection: selection,
            denominate: &denominate,
            reconstitute: &reconstitute,
            issue_structured: &issue,
            unwrap_structured: &unwrap,
            redeem_terminal: &redeem,
        })
        .expect("market-free five-action capability set");

    // Now the Market-bound path, at an arbitrary Market, for comparison.
    let derived = derived_descriptor_for_market(identity(0x21));
    let descriptor =
        decode_derived_structured_descriptor_v2(&derived, authority()).expect("hostile decode");
    let behavior = token_behavior(descriptor);
    let bound_selected = |action| {
        build_rational_open_selected_hot_bundle_v3(RationalOpenSelectedHotBundleInputV3 {
            action,
            logical_data_lengths: &selected_lengths,
            product_basis: &basis,
            kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
            authenticated_token_behavior: behavior,
            root_schema: identity(0x11),
            lifecycle_policy: &policy,
            capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
            root_state_bytes: 8,
        })
        .expect("descriptor-bound selected bundle")
    };
    let bound_structured = |action| {
        build_rational_open_structured_hot_bundle_v3(RationalOpenStructuredHotBundleInputV3 {
            action,
            fixed_data_lengths: &structured_lengths,
            item_data_lengths: [64, 82, 165, 165],
            product_basis: &basis,
            representation_descriptor: descriptor,
            kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
            authenticated_token_behavior: behavior,
            root_schema: identity(0x11),
            lifecycle_policy: &policy,
            capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
            root_state_bytes: 8,
        })
        .expect("descriptor-bound structured bundle")
    };
    let bound_redeem = build_rational_terminal_hot_bundle_v3(RationalTerminalHotBundleInputV3 {
        account_profile: RationalTerminalAccountProfileInputV3 {
            logical_data_lengths: &terminal_lengths,
            product_basis: &basis,
        },
        kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
        authenticated_token_behavior: behavior,
        root_schema: identity(0x11),
        lifecycle_policy: &policy,
        capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
        root_state_bytes: 8,
    })
    .expect("descriptor-bound terminal bundle");
    let bound_denominate = bound_selected(RepresentationActionV2::Denominate);
    let bound_reconstitute = bound_selected(RepresentationActionV2::Reconstitute);
    let bound_issue = bound_structured(RepresentationActionV2::IssueStructured);
    let bound_unwrap = bound_structured(RepresentationActionV2::UnwrapStructured);

    let market_bound =
        build_rational_open_capability_program_set_v3(RationalOpenCapabilityProgramSetInputV3 {
            authenticated_token_behavior: behavior,
            denominate: &bound_denominate,
            reconstitute: &bound_reconstitute,
            issue_structured: &bound_issue,
            unwrap_structured: &bound_unwrap,
            redeem_terminal: &bound_redeem,
        })
        .expect("descriptor-bound five-action capability set");

    // Per-bundle, so a failure names the bundle rather than only the set.
    assert_eq!(denominate, bound_denominate, "Denominate");
    assert_eq!(reconstitute, bound_reconstitute, "Reconstitute");
    assert_eq!(issue, bound_issue, "IssueStructured");
    assert_eq!(unwrap, bound_unwrap, "UnwrapStructured");
    assert_eq!(redeem, bound_redeem, "RedeemTerminal");

    // *** A COMPLETE STRUCTURED CAPABILITY RELEASE, COMPILED BEFORE ITS
    // MARKET, IS THE RELEASE THAT MARKET WOULD HAVE SELECTED. ***
    assert_eq!(
        market_free.program_set_id, market_bound.program_set_id,
        "the pre-founding release_id must be the descriptor-bound one"
    );
    assert_eq!(market_free.program_set, market_bound.program_set);
    assert_eq!(
        market_free.token_behavior_selection_id,
        market_bound.token_behavior_selection_id
    );
}
