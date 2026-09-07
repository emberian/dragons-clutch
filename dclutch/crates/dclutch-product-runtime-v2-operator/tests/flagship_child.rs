//! The flagship conditional market, compiled end to end from three flags.
//!
//! **"If feature `X` activates by slot `S`, does mainnet's slot time move?"** —
//! decision 0029's tenth item. `X`, `S` and the metric are ember's; this drives
//! the shape they fill in, from the two parents' facts through the child's
//! `ParentReferenceV1` bytes, its consecutive-cut `ResultDomainV2`, the
//! founder's branch bundle and the refunding categorical basis.
//!
//! Parent `A` is the relay's row-2 market — a mainnet Feature account's
//! `activated_at`, cut once at `S + 1` — and parent `B` is the row-3 market,
//! mainnet's own mean slot time since the epoch began, cut at the two
//! milliseconds the decoding rules emit. Both parents are ORDINARY markets of
//! the relayed family; what makes this the flagship is that a child reads them
//! as certificates and nothing else.
//!
//! What this does not do is run a program. The 49-account Found instruction
//! this compilation feeds is asserted meta by meta in `tests/found.rs`
//! (`a_child_founding_appends_twelve_parent_slots_at_the_privileges_the_frame_declares`);
//! the real-ELF walk is owed and is named in `MERGE_NOTES_PRODUCT-SHAPES.md`.

use dclutch_product::admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_product::{ContentId, portfolio_record_bytes, result_domain_record_bytes};
use dclutch_product_runtime_v2_operator::child_v1::{
    ChildIdentitiesV1, ChildQuestionV1, ParentFactsV1, child_basis_output_bytes_v1,
    compile_child_product_records_v1,
};
use dclutch_product_runtime_v2_operator::flagship_v1::{
    FLAGSHIP_DECISION_ACTIVATED_CELL_V1, FLAGSHIP_DECISION_ORDINARY_COUNT_V1, FlagshipInputV1,
    flagship_child_input_v1, flagship_decision_cut_v1, flagship_slot_time_metric_v1,
};
use dclutch_source::parent_reference_v1::{ChildShapeV1, ParentReferenceKindV1, ParentReferenceV1};
use solana_program::pubkey::Pubkey;

/// The feature ember has not chosen yet, standing in as a flag value.
const FEATURE_GATE: [u8; 32] = [0x9f; 32];
/// Slot `S`, likewise.
const ACTIVATION_SLOT: u64 = 402_000_000;

const PARENT_DEADLINE: i64 = 1_800_000_000;
const MARGIN_SECONDS: u32 = 7_200;
const FOUNDING_UNIX_SECONDS: i64 = 1_799_000_000;
const SETTLE_BY: u64 = 1_800_020_000;

fn id(tag: u8) -> ContentId {
    let mut bytes = [0_u8; 32];
    bytes[0] = tag;
    bytes[1] = 0x5c;
    ContentId::new(bytes).expect("nonzero identity")
}

fn identities() -> ChildIdentitiesV1 {
    ChildIdentitiesV1 {
        product_id: id(1),
        coordinate_domain_id: id(2),
        result_unit_id: id(3),
        claim_basis_id: id(4),
        liability_basis_id: id(5),
        representation_release_id: id(6),
        mapping_release_id: id(7),
        evaluator_release_id: id(8),
        portfolio_denominator: 1,
    }
}

fn parent(tag: u8, ordinary_count: u32) -> ParentFactsV1 {
    let mut market = [0_u8; 32];
    market[0] = tag;
    market[1] = 0xf1;
    let mut digest = [0_u8; 32];
    digest[0] = tag;
    digest[1] = 0xf2;
    ParentFactsV1 {
        market,
        generation: 3,
        product_record_digest: digest,
        ordinary_count,
        deadline_unix_seconds: PARENT_DEADLINE,
    }
}

fn flagship() -> FlagshipInputV1 {
    let metric = flagship_slot_time_metric_v1();
    let metric_width = metric.ordinary_count().expect("metric width");
    FlagshipInputV1 {
        feature_gate: FEATURE_GATE,
        activation_slot: ACTIVATION_SLOT,
        metric,
        decision_parent: parent(0xA, FLAGSHIP_DECISION_ORDINARY_COUNT_V1),
        metric_parent: parent(0xB, metric_width),
        settle_by: SETTLE_BY,
        margin_seconds: MARGIN_SECONDS,
        bundle_quantity: 4,
        identities: identities(),
    }
}

/// The flagship compiles: three flags in, four records out, and every width a
/// function of the two parents' own widths.
///
/// The three ordinary branch cells are the three slot-time intervals the cuts
/// `[390, 410]` carve; cell three is off-condition and pays the scale without
/// reading `B`; cell four is the child's failure coordinate. A `payoutScale`
/// of four over a width of five is the refunding categorical basis decision
/// 0025 admits.
#[test]
fn the_flagship_compiles_from_three_flags_and_two_parent_facts() {
    let input = flagship_child_input_v1(&flagship()).expect("flagship child input");
    assert_eq!(
        input.question,
        ChildQuestionV1::ConditionalOn {
            condition: FLAGSHIP_DECISION_ACTIVATED_CELL_V1
        }
    );

    let shape = ChildShapeV1::Conditional {
        rows: FLAGSHIP_DECISION_ORDINARY_COUNT_V1,
        condition: FLAGSHIP_DECISION_ACTIVATED_CELL_V1,
        branch: 3,
    };
    let ordinary = shape.ordinary_count().expect("ordinary count");
    let width = shape.width().expect("width");
    assert_eq!((ordinary, width), (4, 5));

    let mut product = vec![0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![
        0_u8;
        result_domain_record_bytes(
            usize::try_from(shape.cut_count().expect("cut count")).expect("fits")
        )
        .expect("domain bytes")
    ];
    let mut portfolio =
        vec![0_u8; portfolio_record_bytes(usize::try_from(width).expect("fits")).expect("bytes")];
    let mut basis = vec![0_u8; child_basis_output_bytes_v1(shape).expect("basis bytes")];

    let compiled = compile_child_product_records_v1(
        Pubkey::new_from_array([0x41; 32]),
        input,
        FOUNDING_UNIX_SECONDS,
        &mut product,
        &mut domain,
        &mut portfolio,
        &mut basis,
    )
    .expect("the flagship child compiles");

    assert_eq!(compiled.shape, shape);
    assert_eq!(compiled.ordinary_count, ordinary);
    assert_eq!(compiled.cuts, vec![1_i128, 2, 3]);
    // The founder buys the three branch cells and neither the off-condition
    // cell nor the failure coordinate.
    assert_eq!(compiled.coefficients, vec![4, 4, 4, 0, 0]);
    assert_eq!(compiled.product.outcome_count, width);
    assert_eq!(
        compiled.window.end_unix_seconds,
        i64::try_from(SETTLE_BY).expect("settle_by fits")
    );
    assert_eq!(compiled.window.max_age_seconds, MARGIN_SECONDS);
    assert_eq!(compiled.window.start_unix_seconds, FOUNDING_UNIX_SECONDS);
}

/// The bytes Core will authenticate are the bytes this compiled: the
/// reference decodes to the same record, names both parents at the
/// generations and Product digests the driver read, and admits to the same
/// shape Core's own `admit_founding` will reach
/// (`programs/dclutch-core-sbf/src/parents_v1.rs`).
#[test]
fn the_reference_the_child_carries_is_the_record_core_will_admit() {
    let input = flagship_child_input_v1(&flagship()).expect("flagship child input");
    let mut product = vec![0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![0_u8; result_domain_record_bytes(3).expect("domain bytes")];
    let mut portfolio = vec![0_u8; portfolio_record_bytes(5).expect("portfolio bytes")];
    let mut basis = vec![
        0_u8;
        child_basis_output_bytes_v1(ChildShapeV1::Conditional {
            rows: FLAGSHIP_DECISION_ORDINARY_COUNT_V1,
            condition: FLAGSHIP_DECISION_ACTIVATED_CELL_V1,
            branch: 3,
        })
        .expect("basis bytes")
    ];
    let compiled = compile_child_product_records_v1(
        Pubkey::new_from_array([0x41; 32]),
        input,
        FOUNDING_UNIX_SECONDS,
        &mut product,
        &mut domain,
        &mut portfolio,
        &mut basis,
    )
    .expect("the flagship child compiles");

    let decoded = ParentReferenceV1::decode(&compiled.reference_bytes).expect("reference decodes");
    assert_eq!(decoded, compiled.reference);
    assert_eq!(decoded.kind, ParentReferenceKindV1::Conditional);
    assert_eq!(decoded.condition, FLAGSHIP_DECISION_ACTIVATED_CELL_V1);
    assert_eq!(decoded.settle_by, SETTLE_BY);
    assert_eq!(decoded.parent_a.market, parent(0xA, 0).market);
    assert_eq!(decoded.parent_b.market, parent(0xB, 0).market);
    assert_eq!(
        decoded.parent_a.ordinary_count,
        FLAGSHIP_DECISION_ORDINARY_COUNT_V1
    );
    assert_eq!(decoded.parent_b.ordinary_count, 3);
    assert_eq!(decoded.admit_founding(), Ok(compiled.shape));
}

/// The decision parent's cut is where the chosen slot puts it, and it is the
/// only place the slot appears. `S + 1` in the atoms row 2 publishes: an
/// `activated_at` of exactly `S` is inside cell zero, and the not-activated
/// sentinel is above every cut a `u64` slot can name.
#[test]
fn the_chosen_slot_appears_once_as_the_decision_parents_cut() {
    assert_eq!(
        flagship_decision_cut_v1(ACTIVATION_SLOT),
        Ok(i128::from(ACTIVATION_SLOT) + 1)
    );
    assert!(
        i128::from(dclutch_source::relay::FEATURE_NOT_ACTIVATED_SENTINEL_V1)
            >= flagship_decision_cut_v1(ACTIVATION_SLOT).expect("cut"),
        "a dormant feature must land above the cut, in the not-activated cell"
    );
}
