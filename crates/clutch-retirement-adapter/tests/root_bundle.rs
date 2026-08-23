mod common;

use clutch_retirement::{EpochChildKindV1, GeneralEpochPhaseV2, Identity32V1, RetirementErrorV2};
use clutch_retirement_adapter::{
    authenticate_general_epoch_v5_exact, authenticate_terminal_epoch_root_bundle_v1,
    AccountAccessV2, AccountViewV2, AuthenticatedEpochChildClassV1, CanonicalPdaV1,
    GeneralEpochAccountV5, RetirementAdapterErrorV2, EPOCH_CHILD_CLASS_CAPACITY_V1,
};
use clutch_solana_layout::{
    EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN, EPOCH_PHASE_LAPSED, EPOCH_PHASE_OPEN,
    EPOCH_PHASE_SETTLED,
};

const KINDS: [EpochChildKindV1; EPOCH_CHILD_CLASS_CAPACITY_V1] = [
    EpochChildKindV1::CandidateBundle,
    EpochChildKindV1::CandidateIndexPage,
    EpochChildKindV1::CandidateVerdict,
    EpochChildKindV1::CandidateEscrow,
    EpochChildKindV1::ClearWorkBundle,
    EpochChildKindV1::OrderPage,
    EpochChildKindV1::ReservationArchive,
    EpochChildKindV1::SettlementReceipt,
    EpochChildKindV1::FinalPot,
];

fn parent(epoch: GeneralEpochAccountV5) -> (Identity32V1, Identity32V1, u64) {
    (
        Identity32V1::new(epoch.base.market.bytes()).unwrap(),
        Identity32V1::new(epoch.base.epoch.bytes()).unwrap(),
        epoch.retirement.epoch_generation,
    )
}

fn classes(
    epoch: GeneralEpochAccountV5,
) -> [AuthenticatedEpochChildClassV1; EPOCH_CHILD_CLASS_CAPACITY_V1] {
    let (market, epoch_id, generation) = parent(epoch);
    KINDS.map(|kind| {
        AuthenticatedEpochChildClassV1::after_authoritative_terminal_empty_validation(
            kind, market, epoch_id, generation, 0,
        )
        .unwrap()
    })
}

fn with_phase(mut epoch: GeneralEpochAccountV5, phase: u8) -> GeneralEpochAccountV5 {
    epoch.base.phase = phase;
    if phase != EPOCH_PHASE_OPEN {
        epoch.base.order_set = common::h(0x20);
        epoch.base.first_order_id = common::h(0x21);
        epoch.base.last_order_id = common::h(0x22);
        epoch.base.page_count = 1;
        epoch.base.order_count = 2;
    }
    epoch
}

fn join(
    epoch: GeneralEpochAccountV5,
    classes: [AuthenticatedEpochChildClassV1; EPOCH_CHILD_CLASS_CAPACITY_V1],
) -> Result<
    clutch_retirement_adapter::AuthenticatedTerminalEpochRootBundleV1,
    RetirementAdapterErrorV2,
> {
    let bytes = epoch.encode().unwrap();
    let address = common::id(0xa0);
    let program_id = common::id(0xa1);
    let authenticated = authenticate_general_epoch_v5_exact(
        AccountViewV2 {
            address,
            owner: program_id,
            data: &bytes,
            is_writable: false,
            is_executable: false,
        },
        program_id,
        CanonicalPdaV1::after_derivation(address, epoch.base.stored_bump),
        AccountAccessV2::ReadOnly,
    )?;
    authenticate_terminal_epoch_root_bundle_v1(authenticated, classes)
}

#[test]
fn settled_and_lapsed_roots_join_exactly_all_nine_classes() {
    for (wire_phase, pure_phase) in [
        (EPOCH_PHASE_SETTLED, GeneralEpochPhaseV2::Settled),
        (EPOCH_PHASE_LAPSED, GeneralEpochPhaseV2::Lapsed),
    ] {
        let epoch = with_phase(common::epoch_v5(), wire_phase);
        let joined = join(epoch, classes(epoch)).unwrap();
        assert_eq!(joined.epoch_account(), common::id(0xa0));
        assert_eq!(joined.epoch().phase, pure_phase);
        assert_eq!(joined.epoch().market, parent(epoch).0);
        assert_eq!(joined.epoch().epoch, parent(epoch).1);
        assert_eq!(joined.epoch().retirement.epoch_generation, parent(epoch).2);
        assert_eq!(joined.authenticated_class_count(), 9);
    }
}

#[test]
fn every_nonterminal_epoch_phase_refuses() {
    for phase in [EPOCH_PHASE_OPEN, EPOCH_PHASE_FROZEN, EPOCH_PHASE_CLEARED] {
        let epoch = with_phase(common::epoch_v5(), phase);
        assert_eq!(
            join(epoch, classes(epoch)),
            Err(RetirementAdapterErrorV2::Retirement(
                RetirementErrorV2::WrongPhase
            ))
        );
    }
}

fn set_count(epoch: &mut GeneralEpochAccountV5, kind: EpochChildKindV1) {
    match kind {
        EpochChildKindV1::CandidateBundle => epoch.retirement.children.candidate_bundles = 1,
        EpochChildKindV1::CandidateIndexPage => epoch.retirement.children.candidate_index_pages = 1,
        EpochChildKindV1::CandidateVerdict => epoch.retirement.children.candidate_verdicts = 1,
        EpochChildKindV1::CandidateEscrow => epoch.retirement.children.candidate_escrows = 1,
        EpochChildKindV1::ClearWorkBundle => epoch.retirement.children.clear_work_bundles = 1,
        EpochChildKindV1::OrderPage => epoch.retirement.children.order_pages = 1,
        EpochChildKindV1::ReservationArchive => epoch.retirement.children.reservation_archives = 1,
        EpochChildKindV1::SettlementReceipt => epoch.retirement.children.settlement_receipts = 1,
        EpochChildKindV1::FinalPot => epoch.retirement.children.final_pots = 1,
    }
}

#[test]
fn each_authoritative_nonzero_class_count_blocks_the_join() {
    for kind in KINDS {
        let mut epoch = with_phase(common::epoch_v5(), EPOCH_PHASE_SETTLED);
        let evidence = classes(epoch);
        set_count(&mut epoch, kind);
        assert_eq!(
            join(epoch, evidence),
            Err(RetirementAdapterErrorV2::Retirement(
                RetirementErrorV2::ChildOutstanding
            )),
            "{kind:?} must independently block root evidence"
        );
    }
}

#[test]
fn duplicate_class_is_also_an_omission_and_refuses() {
    let epoch = with_phase(common::epoch_v5(), EPOCH_PHASE_SETTLED);
    let mut evidence = classes(epoch);
    evidence[8] = evidence[0];
    assert_eq!(
        join(epoch, evidence),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::WrongChildKind
        ))
    );
}

#[test]
fn every_class_witness_is_bound_to_the_exact_parent_triple() {
    let epoch = with_phase(common::epoch_v5(), EPOCH_PHASE_SETTLED);
    let (market, epoch_id, generation) = parent(epoch);

    for index in 0..EPOCH_CHILD_CLASS_CAPACITY_V1 {
        let mut wrong_market = classes(epoch);
        wrong_market[index] =
            AuthenticatedEpochChildClassV1::after_authoritative_terminal_empty_validation(
                KINDS[index],
                common::id(0xb0),
                epoch_id,
                generation,
                0,
            )
            .unwrap();
        assert_eq!(
            join(epoch, wrong_market),
            Err(RetirementAdapterErrorV2::Retirement(
                RetirementErrorV2::WrongParent
            ))
        );

        let mut wrong_epoch = classes(epoch);
        wrong_epoch[index] =
            AuthenticatedEpochChildClassV1::after_authoritative_terminal_empty_validation(
                KINDS[index],
                market,
                common::id(0xb1),
                generation,
                0,
            )
            .unwrap();
        assert_eq!(
            join(epoch, wrong_epoch),
            Err(RetirementAdapterErrorV2::Retirement(
                RetirementErrorV2::WrongParent
            ))
        );

        let mut wrong_generation = classes(epoch);
        wrong_generation[index] =
            AuthenticatedEpochChildClassV1::after_authoritative_terminal_empty_validation(
                KINDS[index],
                market,
                epoch_id,
                generation + 1,
                0,
            )
            .unwrap();
        assert_eq!(
            join(epoch, wrong_generation),
            Err(RetirementAdapterErrorV2::Retirement(
                RetirementErrorV2::WrongGeneration
            ))
        );
    }
}

#[test]
fn class_constructor_refuses_zero_generation_and_live_children() {
    let epoch = common::epoch_v5();
    let (market, epoch_id, generation) = parent(epoch);
    assert_eq!(
        AuthenticatedEpochChildClassV1::after_authoritative_terminal_empty_validation(
            EpochChildKindV1::OrderPage,
            market,
            epoch_id,
            0,
            0,
        ),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::WrongGeneration
        ))
    );
    assert_eq!(
        AuthenticatedEpochChildClassV1::after_authoritative_terminal_empty_validation(
            EpochChildKindV1::OrderPage,
            market,
            epoch_id,
            generation,
            1,
        ),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::ChildOutstanding
        ))
    );
}
