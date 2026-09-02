//! Per-kind and per-action witnesses for the adopted Rational child wire.
//!
//! Decision 0011 §3b took Option A, so these tests are the evidence that
//! Structured's kinds reach the chain through Rational's ABI and nothing else.
//! Three of them exist specifically because the adoption is NOT a rename: the
//! two closure kinds leave the Token-effect vocabulary entirely, `Issue`'s
//! effect order inverts, and `TerminalRedeem` stops being one request.

mod support;

use dclutch_fractional_claim_kernel::{
    FractionalExposureTermsInputV2, encode_fractional_exposure_terms_v2,
    fractional_exposure_terms_bytes_v2,
};
use dclutch_rational_representation_v2_contract::{
    CallerRoleV2, REQUEST_MAGIC_V2, RepresentationActionV2, RepresentationRequestV2,
    TokenEffectStyleV2,
};
use dclutch_rational_representation_v2_lifecycle_contract::LifecycleActionV2;
use dclutch_structured_v2_contract::{StructuredActionV2, StructuredHotTokenKindV2};
use dclutch_structured_v2_kernel::{
    ShardMovementV2, StructuredTermsInputV2, StructuredTermsV2, encode_structured_terms_v2,
    structured_terms_bytes_v2,
};
use dclutch_structured_v2_operator::{
    Error, STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2, StructuredChildActorV2,
    StructuredChildCoordinateV2, StructuredChildDescriptorV2, StructuredChildWireV2,
    bind_structured_child_descriptor_v2, encode_structured_child_representation_v2,
    structured_child_effect_order_v2, structured_child_lifecycle_action_v2,
    structured_child_request_bytes_v2, structured_child_token_style_v2, structured_child_wire_v2,
};
use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

use support::{digest, identity, shard_mints, shard_terms, structured_admission};

const WIDTH: usize = 3;
const DENOMINATOR: u64 = 4;
/// The third coordinate is deliberately INERT. The Rational wire demands one
/// asset row per Product outcome, so a zero-coefficient coordinate still needs
/// a row and still needs its accounts materialized.
const COEFFICIENTS: [u64; WIDTH] = [1, 3, 0];

const MARKET: u8 = 0x11;
const PRODUCT_RECORD: u8 = 0x12;
const RESULT_DOMAIN: u8 = 0x13;
const RELEASE_SET: u8 = 0x14;
const SHARD_EXPOSURE: u8 = 0x18;
const GRAPH_ID: u8 = 0x1b;
const RECEIPT_MINT: u8 = 0x1c;

/// Shard terms carrying a REAL Token program id.
///
/// The shared fixture uses a synthetic `identity(TOKEN_PROGRAM)`, which the
/// Rational wire refuses outright (`TokenProgram::parse` in
/// `RepresentationRequestV2::validate`). Anything crossing this wire has to
/// name a Token program that actually exists, and `bind_shard_terms` requires
/// the two layers to agree on it, so both builders are paired here.
fn shard_terms_bytes(market: [u8; 32], graph_id: [u8; 32], exposure_id: [u8; 32]) -> Vec<u8> {
    let mints = shard_mints(WIDTH);
    let size = fractional_exposure_terms_bytes_v2(WIDTH).expect("shard terms width");
    let mut scratch = vec![0_u8; size];
    let mut output = vec![0_u8; size];
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market,
            product_record: identity(PRODUCT_RECORD),
            result_domain: identity(RESULT_DOMAIN),
            release_set: identity(RELEASE_SET),
            token_program: TOKEN_2022_PROGRAM_ID,
            token_behavior: identity(0x16),
            exposure_id,
            product_basis: identity(0x19),
            representation_basis: identity(0x1a),
            graph_id,
            product_width: u32::try_from(WIDTH).expect("product width"),
            denominator: DENOMINATOR,
            shard_mints: &mints,
        },
        &mut scratch,
        &mut output,
    )
    .expect("encode shard terms");
    output
}

/// Structured terms over the paired shard layer.
///
/// `graph_id` and `shard_exposure` are BOTH parameters because they are two
/// different records, and the descriptor join reads the second one.  A fixture
/// that varied only `graph_id` could not exercise the join at all.
fn terms_bytes(
    market: [u8; 32],
    graph_id: [u8; 32],
    exposure_id: [u8; 32],
    receipt_mint: [u8; 32],
) -> Vec<u8> {
    let size = structured_terms_bytes_v2(WIDTH).expect("terms width");
    let mut scratch = vec![0_u8; size];
    let mut output = vec![0_u8; size];
    encode_structured_terms_v2(
        StructuredTermsInputV2 {
            market,
            product_record: identity(PRODUCT_RECORD),
            result_domain: identity(RESULT_DOMAIN),
            release_set: identity(RELEASE_SET),
            token_program: TOKEN_2022_PROGRAM_ID,
            token_behavior: identity(0x17),
            shard_terms: digest(&shard_terms_bytes(market, graph_id, exposure_id)),
            shard_exposure: exposure_id,
            receipt_mint,
            graph_id,
            denominator: DENOMINATOR,
            coefficients: &COEFFICIENTS,
        },
        &mut scratch,
        &mut output,
    )
    .expect("encode terms");
    output
}

fn canonical_terms_bytes() -> Vec<u8> {
    terms_bytes(
        identity(MARKET),
        identity(GRAPH_ID),
        identity(SHARD_EXPOSURE),
        identity(RECEIPT_MINT),
    )
}

fn canonical_shard_bytes() -> Vec<u8> {
    shard_terms_bytes(
        identity(MARKET),
        identity(GRAPH_ID),
        identity(SHARD_EXPOSURE),
    )
}

fn decode_terms<'a>(bytes: &'a [u8], shard_bytes: &'a [u8]) -> StructuredTermsV2<'a> {
    StructuredTermsV2::decode(bytes, structured_admission(bytes), shard_terms(shard_bytes))
        .expect("decode terms")
}

fn descriptor() -> StructuredChildDescriptorV2 {
    StructuredChildDescriptorV2 {
        descriptor_id: identity(0x50),
        // The composition EXPOSURE bundle, which is what the Rational wire's
        // `graph_id` field holds -- NOT `identity(GRAPH_ID)`, the source graph.
        exposure_id: identity(SHARD_EXPOSURE),
        representation_authority: identity(0x51),
        receipt_mint: identity(RECEIPT_MINT),
        market: identity(MARKET),
        release_set: identity(RELEASE_SET),
        token_program: TOKEN_2022_PROGRAM_ID,
        outcome_count: u32::try_from(WIDTH).expect("width"),
        denominator: DENOMINATOR,
    }
}

fn actor() -> StructuredChildActorV2 {
    StructuredChildActorV2 {
        actor: identity(0x60),
        receipt_account: identity(0x61),
        parent_context: identity(0x62),
        generation: 4,
        expected_representation_revision: 2,
        expected_receipt_supply: 100,
    }
}

fn coordinates() -> Vec<StructuredChildCoordinateV2> {
    let mints = shard_mints(WIDTH);
    (0..WIDTH)
        .map(|row| StructuredChildCoordinateV2 {
            shard_mint: *mints.get(row).expect("mint"),
            actor_shard_account: identity(0x70 + u8::try_from(row).expect("row")),
            structured_custody_account: identity(0x80 + u8::try_from(row).expect("row")),
            claims_custody_owner: identity(0x90 + u8::try_from(row).expect("row")),
            expected_shard_supply: 10_000,
            expected_actor_shards: 5_000,
        })
        .collect()
}

/// Full-width movements, including the inert third row at zero atoms.
fn movements(receipt_atoms: u64) -> Vec<ShardMovementV2> {
    let mints = shard_mints(WIDTH);
    (0..WIDTH)
        .map(|row| {
            let coefficient = *COEFFICIENTS.get(row).expect("coefficient");
            ShardMovementV2 {
                representation_coordinate: u32::try_from(row).expect("row"),
                shard_mint: *mints.get(row).expect("mint"),
                shard_atoms: coefficient * receipt_atoms,
                post_required_custody: coefficient * (receipt_atoms + 1),
                surplus_shard_custody: 0,
            }
        })
        .collect()
}

#[test]
fn every_structured_kind_lands_on_exactly_one_wire() {
    for (kind, style) in [
        (
            StructuredHotTokenKindV2::MintReceipts,
            TokenEffectStyleV2::MintReceipt,
        ),
        (
            StructuredHotTokenKindV2::BurnReceipts,
            TokenEffectStyleV2::BurnReceipt,
        ),
        (
            StructuredHotTokenKindV2::LockShards,
            TokenEffectStyleV2::TransferShardToStructured,
        ),
        (
            StructuredHotTokenKindV2::ReleaseShards,
            TokenEffectStyleV2::TransferShardFromStructured,
        ),
    ] {
        assert_eq!(structured_child_token_style_v2(kind), Ok(style));
        assert_eq!(
            structured_child_lifecycle_action_v2(kind),
            Err(Error::ChildWire)
        );
    }
    for (kind, action) in [
        (
            StructuredHotTokenKindV2::CloseCustody,
            LifecycleActionV2::RetireCoordinate,
        ),
        (
            StructuredHotTokenKindV2::CloseReceiptMint,
            LifecycleActionV2::RetireReceipt,
        ),
    ] {
        assert_eq!(structured_child_lifecycle_action_v2(kind), Ok(action));
        // The closure kinds are not TokenEffectStyleV2 members at all. Asking
        // for one must refuse rather than return a plausible wrong style.
        assert_eq!(structured_child_token_style_v2(kind), Err(Error::ChildWire));
    }
}

#[test]
fn the_two_shard_supply_styles_are_unreachable_from_structured() {
    // MintShard and BurnShard belong to Denominate/Reconstitute, which create
    // and destroy shards. Structured only ever moves shards that already
    // exist, so no Structured kind may lower onto either.
    for kind in [
        StructuredHotTokenKindV2::MintReceipts,
        StructuredHotTokenKindV2::BurnReceipts,
        StructuredHotTokenKindV2::LockShards,
        StructuredHotTokenKindV2::ReleaseShards,
        StructuredHotTokenKindV2::CloseCustody,
        StructuredHotTokenKindV2::CloseReceiptMint,
    ] {
        assert!(!matches!(
            structured_child_token_style_v2(kind),
            Ok(TokenEffectStyleV2::MintShard) | Ok(TokenEffectStyleV2::BurnShard)
        ));
    }
}

#[test]
fn issue_puts_the_receipt_last_and_unwrap_puts_it_first() {
    let width = u32::try_from(WIDTH).expect("width");
    let issue = structured_child_effect_order_v2(RepresentationActionV2::IssueStructured, width)
        .expect("issue");
    assert_eq!(issue.len(), WIDTH + 1);
    for row in 0..width {
        let slot = issue.get(usize::try_from(row).expect("row")).expect("slot");
        assert_eq!(slot.cursor, row);
        assert_eq!(slot.style, TokenEffectStyleV2::TransferShardToStructured);
        // The callee reads asset_index = cursor when issuing.
        assert_eq!(slot.asset_row, Some(row));
    }
    // THE INVERSION. This crate's own plan pushes the receipt effect FIRST for
    // both actions; the wire mints it LAST for Issue, and the order is not
    // negotiable because the callee indexes the asset row from the cursor.
    let last = issue.last().expect("last");
    assert_eq!(last.style, TokenEffectStyleV2::MintReceipt);
    assert_eq!(last.cursor, width);
    assert_eq!(last.asset_row, None);

    let unwrap = structured_child_effect_order_v2(RepresentationActionV2::UnwrapStructured, width)
        .expect("unwrap");
    assert_eq!(unwrap.len(), WIDTH + 1);
    let first = unwrap.first().expect("first");
    assert_eq!(first.style, TokenEffectStyleV2::BurnReceipt);
    assert_eq!(first.cursor, 0);
    assert_eq!(first.asset_row, None);
    for row in 0..width {
        let slot = unwrap
            .get(usize::try_from(row).expect("row") + 1)
            .expect("slot");
        assert_eq!(slot.cursor, row + 1);
        assert_eq!(slot.style, TokenEffectStyleV2::TransferShardFromStructured);
        // The callee reads asset_index = cursor - 1 when unwrapping.
        assert_eq!(slot.asset_row, Some(row));
    }
}

#[test]
fn the_shard_sweep_is_strictly_ascending_on_both_actions() {
    let width = u32::try_from(WIDTH).expect("width");
    for action in [
        RepresentationActionV2::IssueStructured,
        RepresentationActionV2::UnwrapStructured,
    ] {
        let slots = structured_child_effect_order_v2(action, width).expect("slots");
        let rows: Vec<u32> = slots.iter().filter_map(|slot| slot.asset_row).collect();
        assert_eq!(rows, vec![0, 1, 2]);
        let cursors: Vec<u32> = slots.iter().map(|slot| slot.cursor).collect();
        assert_eq!(cursors, vec![0, 1, 2, 3]);
    }
}

#[test]
fn the_three_non_structured_actions_have_no_effect_order_here() {
    for action in [
        RepresentationActionV2::Denominate,
        RepresentationActionV2::Reconstitute,
        RepresentationActionV2::RedeemTerminal,
    ] {
        assert_eq!(
            structured_child_effect_order_v2(action, 3),
            Err(Error::ChildWire)
        );
    }
}

#[test]
fn structured_terminal_redeem_is_not_one_child_request() {
    assert_eq!(
        structured_child_wire_v2(StructuredActionV2::Issue),
        StructuredChildWireV2::Representation(RepresentationActionV2::IssueStructured)
    );
    assert_eq!(
        structured_child_wire_v2(StructuredActionV2::Unwrap),
        StructuredChildWireV2::Representation(RepresentationActionV2::UnwrapStructured)
    );
    // Structured's TerminalRedeem builds Unwrap's Token effects and THEN
    // settles. Rational's RedeemTerminal burns ONE selected outcome's shards
    // and pays its collateral, and refuses unless asset_count == 1. They are
    // not the same action, and this must never silently map.
    assert_eq!(
        structured_child_wire_v2(StructuredActionV2::TerminalRedeem),
        StructuredChildWireV2::TerminalTwoPhase
    );
    assert_eq!(
        structured_child_wire_v2(StructuredActionV2::ZeroSupplyRetire),
        StructuredChildWireV2::Lifecycle
    );

    let bytes = canonical_terms_bytes();
    let shard_bytes = canonical_shard_bytes();
    let terms = decode_terms(&bytes, &shard_bytes);
    for action in [
        StructuredActionV2::TerminalRedeem,
        StructuredActionV2::ZeroSupplyRetire,
    ] {
        assert_eq!(
            encode_structured_child_representation_v2(
                action,
                terms,
                descriptor(),
                actor(),
                &coordinates(),
                &movements(4),
                4,
            )
            .err(),
            Some(Error::ChildWire)
        );
    }
}

#[test]
fn the_executable_ceiling_is_three_outcomes_and_a_fourth_refuses() {
    assert_eq!(STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2, 3);
    // Physical ABI v3: a 384-byte structured header over 64-byte asset rows,
    // where v2 was 488 over 160. K = 3 is 576 bytes where it was 968.
    assert_eq!(structured_child_request_bytes_v2(3), Ok(576));
    assert_eq!(structured_child_request_bytes_v2(1), Ok(448));
    for hostile in [0, 4, 257] {
        assert_eq!(
            structured_child_request_bytes_v2(hostile),
            Err(Error::ChildWidth)
        );
        assert_eq!(
            structured_child_effect_order_v2(RepresentationActionV2::IssueStructured, hostile),
            Err(Error::ChildWidth)
        );
    }
}

#[test]
fn an_issue_encodes_and_decodes_as_the_exact_rational_request() {
    let bytes = canonical_terms_bytes();
    let shard_bytes = canonical_shard_bytes();
    let terms = decode_terms(&bytes, &shard_bytes);
    let receipt_atoms = 4;
    let encoded = encode_structured_child_representation_v2(
        StructuredActionV2::Issue,
        terms,
        descriptor(),
        actor(),
        &coordinates(),
        &movements(receipt_atoms),
        receipt_atoms,
    )
    .expect("encode");
    // 384 structured header + 3 * 64 asset rows.
    assert_eq!(encoded.len(), 576);
    assert_eq!(encoded.get(..8), Some(REQUEST_MAGIC_V2.as_slice()));

    let request = RepresentationRequestV2::decode(&encoded).expect("decode");
    let header = request.header();
    assert_eq!(header.action, RepresentationActionV2::IssueStructured);
    assert_eq!(header.caller_role, CallerRoleV2::Trading);
    assert_eq!(header.descriptor_id, identity(0x50));
    // The ADOPTED authority signs, not the Structured root. This assertion is
    // the identity death of decision 0011 §3b, stated where it executes.
    assert_eq!(header.representation_authority, identity(0x51));
    assert_eq!(header.quantity, receipt_atoms);
    assert_eq!(header.denominator, DENOMINATOR);
    // Full Product width -- INCLUDING the inert third coordinate -- and no
    // selected outcome. The wire refuses asset_count != outcome_count.
    let width = u32::try_from(WIDTH).expect("width");
    assert_eq!(header.asset_count, width);
    assert_eq!(header.outcome_count, width);
    assert_eq!(header.selected_outcome, u32::MAX);
    // Realm and collateral recipient are terminal-only and must be absent.
    assert_eq!(header.realm, [0; 32]);
    assert_eq!(header.collateral_recipient, [0; 32]);

    for row in 0..width {
        let index = usize::try_from(row).expect("row");
        let asset = request.asset_row(row).expect("asset");
        assert_eq!(asset.coefficient, *COEFFICIENTS.get(index).expect("c"));
        // The shard Mint assertion that stood here is gone with the field:
        // physical ABI v3 derives the Mint from
        // `(program_id, descriptor_id, outcome)` and does not send it, so what
        // the encoder is still answerable for at this row is the ONE key a
        // caller chooses, which the fixture lays out at 0x70 + row.
        assert_eq!(
            asset.actor_shard_account,
            identity(0x70 + u8::try_from(row).expect("row"))
        );
        // The callee recomputes K_i = c_i * S itself (plan.rs). The encoder
        // must publish the coefficient it will agree with.
        assert_eq!(
            asset.coefficient * header.quantity,
            COEFFICIENTS.get(index).expect("c") * receipt_atoms
        );
    }
    // The inert coordinate rides the wire at coefficient zero and will execute
    // a zero-amount transfer rather than being skipped.
    assert_eq!(request.asset_row(2).expect("inert").coefficient, 0);
}

#[test]
fn an_unwrap_carries_the_pre_action_custody_the_release_draws_down() {
    let bytes = canonical_terms_bytes();
    let shard_bytes = canonical_shard_bytes();
    let terms = decode_terms(&bytes, &shard_bytes);
    let receipt_atoms = 4;
    let plan = movements(receipt_atoms);
    let encoded = encode_structured_child_representation_v2(
        StructuredActionV2::Unwrap,
        terms,
        descriptor(),
        actor(),
        &coordinates(),
        &plan,
        receipt_atoms,
    )
    .expect("encode");
    let request = RepresentationRequestV2::decode(&encoded).expect("decode");
    assert_eq!(
        request.header().action,
        RepresentationActionV2::UnwrapStructured
    );
    for row in 0..u32::try_from(WIDTH).expect("width") {
        let index = usize::try_from(row).expect("row");
        let asset = request.asset_row(row).expect("asset");
        let movement = plan.get(index).expect("movement");
        // Releasing draws custody DOWN, so pre-action custody is the required
        // post-action backing plus whatever this action moves out. The callee
        // refuses unless custody covers the release.
        assert_eq!(
            asset.expected_structured_shards,
            movement.post_required_custody + movement.shard_atoms
        );
        assert!(asset.expected_structured_shards >= asset.coefficient * receipt_atoms);
    }
}

#[test]
fn issue_and_unwrap_move_custody_in_opposite_directions() {
    let bytes = canonical_terms_bytes();
    let shard_bytes = canonical_shard_bytes();
    let terms = decode_terms(&bytes, &shard_bytes);
    let receipt_atoms = 4;
    let plan = movements(receipt_atoms);
    let issue_bytes = encode_structured_child_representation_v2(
        StructuredActionV2::Issue,
        terms,
        descriptor(),
        actor(),
        &coordinates(),
        &plan,
        receipt_atoms,
    )
    .expect("issue");
    let unwrap_bytes = encode_structured_child_representation_v2(
        StructuredActionV2::Unwrap,
        terms,
        descriptor(),
        actor(),
        &coordinates(),
        &plan,
        receipt_atoms,
    )
    .expect("unwrap");
    let issue = RepresentationRequestV2::decode(&issue_bytes).expect("decode issue");
    let unwrap = RepresentationRequestV2::decode(&unwrap_bytes).expect("decode unwrap");
    for row in 0..u32::try_from(WIDTH).expect("width") {
        let index = usize::try_from(row).expect("row");
        let moved = plan.get(index).expect("movement").shard_atoms;
        let issued = issue.asset_row(row).expect("asset").expected_structured_shards;
        let unwrapped = unwrap.asset_row(row).expect("asset").expected_structured_shards;
        // Same plan, opposite sign: locking starts below the post-action
        // backing by exactly what it locks, releasing starts above it by
        // exactly what it releases.
        assert_eq!(unwrapped - issued, 2 * moved);
    }
}

#[test]
fn a_rational_context_descriptor_refuses_to_drive_structured_terms() {
    let bytes = canonical_terms_bytes();
    let shard_bytes = canonical_shard_bytes();
    let terms = decode_terms(&bytes, &shard_bytes);
    // DIRECTION ONE. A descriptor belonging to another representation --
    // another Market, release, graph, receipt Mint, Token program, width or
    // denominator -- must not be encodable against these terms.
    for hostile in [
        StructuredChildDescriptorV2 {
            market: identity(0x99),
            ..descriptor()
        },
        StructuredChildDescriptorV2 {
            release_set: identity(0x99),
            ..descriptor()
        },
        StructuredChildDescriptorV2 {
            exposure_id: identity(0x99),
            ..descriptor()
        },
        // THE NEAR MISS, and the one this join was originally written the wrong
        // way round for: the SOURCE GRAPH identity in the exposure slot. Both
        // values are carried by these very terms, and the terms decoder proves
        // they differ, so this is a coherent-looking descriptor that names the
        // wrong record.
        StructuredChildDescriptorV2 {
            exposure_id: identity(GRAPH_ID),
            ..descriptor()
        },
        StructuredChildDescriptorV2 {
            receipt_mint: identity(0x99),
            ..descriptor()
        },
        StructuredChildDescriptorV2 {
            token_program: identity(0x99),
            ..descriptor()
        },
        StructuredChildDescriptorV2 {
            denominator: DENOMINATOR + 1,
            ..descriptor()
        },
        StructuredChildDescriptorV2 {
            outcome_count: u32::try_from(WIDTH).expect("width") + 1,
            ..descriptor()
        },
    ] {
        assert_eq!(
            bind_structured_child_descriptor_v2(terms, hostile),
            Err(Error::ChildIdentity)
        );
        assert!(
            encode_structured_child_representation_v2(
                StructuredActionV2::Issue,
                terms,
                hostile,
                actor(),
                &coordinates(),
                &movements(4),
                4,
            )
            .is_err()
        );
    }
}

#[test]
fn structured_terms_refuse_to_drive_another_representations_descriptor() {
    // DIRECTION TWO, the mirror: hold the descriptor fixed and move the terms.
    // A second Structured root over a different Market, graph or receipt Mint
    // must not authenticate against the descriptor the first one founded.
    let canonical = descriptor();
    // Each hostile root is internally COHERENT -- its own shard layer agrees
    // with it -- so the refusal below is the descriptor join failing, not a
    // fixture that never decoded.
    //
    // The SOURCE GRAPH is deliberately absent from this list and that is not an
    // omission: the Rational descriptor has no source-graph field. It names the
    // exposure bundle, and the exposure bundle names the graph, so a Structured
    // root over a different graph is refused where the exposure RECORD is read
    // -- `derive_structured_representation_descriptor_v2` in `descriptor.rs`,
    // which has its own witness for it. This join sees only what the descriptor
    // carries, and claiming otherwise is how it came to compare the wrong field.
    for (market, exposure_id, receipt_mint) in [
        (
            identity(0x98),
            identity(SHARD_EXPOSURE),
            identity(RECEIPT_MINT),
        ),
        (identity(MARKET), identity(0x97), identity(RECEIPT_MINT)),
        (identity(MARKET), identity(SHARD_EXPOSURE), identity(0x96)),
    ] {
        let hostile_bytes = terms_bytes(market, identity(GRAPH_ID), exposure_id, receipt_mint);
        let hostile_shard = shard_terms_bytes(market, identity(GRAPH_ID), exposure_id);
        let terms = decode_terms(&hostile_bytes, &hostile_shard);
        assert_eq!(
            bind_structured_child_descriptor_v2(terms, canonical),
            Err(Error::ChildIdentity)
        );
    }
}

#[test]
fn the_adopted_authority_may_not_alias_what_it_is_independent_of() {
    let bytes = canonical_terms_bytes();
    let shard_bytes = canonical_shard_bytes();
    let terms = decode_terms(&bytes, &shard_bytes);
    for alias in [
        identity(RECEIPT_MINT),
        identity(0x50),
        identity(MARKET),
        [0; 32],
    ] {
        assert_eq!(
            bind_structured_child_descriptor_v2(
                terms,
                StructuredChildDescriptorV2 {
                    representation_authority: alias,
                    ..descriptor()
                }
            ),
            Err(Error::ChildIdentity)
        );
    }
}

#[test]
fn a_movement_table_that_disagrees_with_the_observed_coordinate_refuses() {
    let bytes = canonical_terms_bytes();
    let shard_bytes = canonical_shard_bytes();
    let terms = decode_terms(&bytes, &shard_bytes);

    // A shard Mint substituted in the movement table but not in the observed
    // accounts is the one-atom-skew hostile at the wire layer: it must refuse
    // rather than encode an asset row naming two different Mints.
    let mut skewed = movements(4);
    skewed.get_mut(1).expect("row").shard_mint = identity(0x99);
    assert_eq!(
        encode_structured_child_representation_v2(
            StructuredActionV2::Issue,
            terms,
            descriptor(),
            actor(),
            &coordinates(),
            &skewed,
            4,
        )
        .err(),
        Some(Error::ChildIdentity)
    );

    // An out-of-order movement table refuses too. The callee indexes by
    // position, so an unsorted table would pair the wrong coefficient with the
    // wrong Mint -- silently, and only at execution.
    let mut unsorted = movements(4);
    unsorted.swap(0, 2);
    assert_eq!(
        encode_structured_child_representation_v2(
            StructuredActionV2::Issue,
            terms,
            descriptor(),
            actor(),
            &coordinates(),
            &unsorted,
            4,
        )
        .err(),
        Some(Error::ChildIdentity)
    );
}

#[test]
fn an_issue_whose_custody_would_underflow_refuses_rather_than_wrapping() {
    let bytes = canonical_terms_bytes();
    let shard_bytes = canonical_shard_bytes();
    let terms = decode_terms(&bytes, &shard_bytes);
    // Locking more than the post-action backing is incoherent: the pre-action
    // custody it implies is negative. That must refuse, not wrap.
    let mut impossible = movements(4);
    let row = impossible.get_mut(0).expect("row");
    row.shard_atoms = row.post_required_custody + 1;
    assert_eq!(
        encode_structured_child_representation_v2(
            StructuredActionV2::Issue,
            terms,
            descriptor(),
            actor(),
            &coordinates(),
            &impossible,
            4,
        )
        .err(),
        Some(Error::ChildIdentity)
    );
}

#[test]
fn a_short_or_long_table_refuses_because_the_wire_demands_full_width() {
    let bytes = canonical_terms_bytes();
    let shard_bytes = canonical_shard_bytes();
    let terms = decode_terms(&bytes, &shard_bytes);
    let full = coordinates();
    let plan = movements(4);
    assert_eq!(
        encode_structured_child_representation_v2(
            StructuredActionV2::Issue,
            terms,
            descriptor(),
            actor(),
            full.get(..2).expect("short"),
            &plan,
            4,
        )
        .err(),
        Some(Error::ChildWidth)
    );
    assert_eq!(
        encode_structured_child_representation_v2(
            StructuredActionV2::Issue,
            terms,
            descriptor(),
            actor(),
            &full,
            plan.get(..2).expect("short"),
            4,
        )
        .err(),
        Some(Error::ChildWidth)
    );
    // A zero receipt quantity is not an action; the wire refuses it too.
    assert_eq!(
        encode_structured_child_representation_v2(
            StructuredActionV2::Issue,
            terms,
            descriptor(),
            actor(),
            &full,
            &plan,
            0,
        )
        .err(),
        Some(Error::ChildWire)
    );
}
