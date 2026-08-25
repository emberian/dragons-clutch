#![allow(clippy::indexing_slicing, clippy::unwrap_used)]

use super::*;
use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    CompartmentFundingV1, FundingAmountsV1, FundingCustodyObservationV1, FundingQuoteV1,
    FundingStateV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_core_contract::{ContentId as CoreContentId, MarketIdentity, MarketRoot, Phase};
use sha2::{Digest, Sha256};
use std::{vec, vec::Vec};

const STATE_RENT: u64 = 1_000;
const PAGE_RENT: u64 = 100;
const BATCH_RENT: u64 = 500;

fn id(fill: u8) -> ContentId {
    ContentId::new([fill; 32]).expect("nonzero ID")
}

fn owner(fill: u8) -> OwnerKeyV1 {
    OwnerKeyV1::new([fill; 32]).expect("nonzero owner")
}

fn core_id(identifier: ContentId) -> CoreContentId {
    CoreContentId::new(identifier.to_bytes()).expect("core ID")
}

fn capability_id(identifier: ContentId) -> dclutch_capability_contract::ContentId {
    dclutch_capability_contract::ContentId::new(identifier.to_bytes()).expect("capability ID")
}

fn config() -> GeneralConfigV1 {
    GeneralConfigV1::new(GeneralConfigV1Input {
        capacity_profile_id: id(1),
        claim_basis_id: id(3),
        capability_release_id: GENERAL_CAPABILITY_RELEASE_ID_V1,
        generation: 7,
        price_scale: 100,
        collection_slots: 10,
        selection_slots: 10,
        settlement_slots: 10,
        max_orders_per_candidate: 8,
        max_pages_per_candidate: 2,
        continuation_reward_lamports: 1,
        outcome_count: 2,
    })
    .expect("config")
}

fn root() -> GeneralRootV1 {
    GeneralRootV1::founding([8; 32], id(9), 7, [6; 32]).expect("root")
}

fn selecting_batch() -> BatchRootV1 {
    let mut batch = BatchRootV1::open(id(9), 0, 0, config()).expect("batch");
    batch
        .open_selection(config(), batch_capitalization(batch), 10)
        .expect("selection");
    batch
}

fn batch_capitalization(batch: BatchRootV1) -> BatchCapitalizationV1 {
    BatchCapitalizationV1 {
        account_lamports: BATCH_RENT + batch.work_remaining_lamports(),
        exact_state_rent_lamports: BATCH_RENT,
    }
}

fn order(
    order_fill: u8,
    owner_fill: u8,
    coefficients: [i64; 2],
    limit: i128,
) -> PortfolioOrderV1<2> {
    PortfolioOrderV1::new(PortfolioOrderV1Input {
        market: [8; 32],
        claim_basis_id: id(3),
        owner: owner(owner_fill),
        order_id: id(order_fill),
        generation: 7,
        batch_sequence: 0,
        nonce: u64::from(order_fill),
        valid_until_slot: 30,
        max_lots: 1,
        max_quote_debit_per_lot_numerator: limit,
        coefficients,
        outcome_count: 2,
    })
    .expect("order")
}

fn executions() -> [Option<ExecutionV1<2>>; MAX_EXECUTIONS_PER_PAGE_V1] {
    let first = order(10, 20, [1, 0], 50);
    let second = order(11, 21, [0, 1], 70);
    [
        Some(ExecutionV1 {
            order: first,
            order_state: OrderStateV1::open(first),
            fill_lots: 1,
        }),
        Some(ExecutionV1 {
            order: second,
            order_state: OrderStateV1::open(second),
            fill_lots: 1,
        }),
        None,
        None,
    ]
}

fn page() -> CandidatePageV1<2> {
    CandidatePageV1 {
        page_index: 0,
        next_page_id: None,
        execution_count: 2,
        executions: executions(),
    }
}

fn page_id<const N: usize>(page: CandidatePageV1<N>) -> ContentId {
    let mut bytes = vec![0; CandidatePageV1::<N>::encoded_len(page.execution_count).unwrap()];
    page.encode(&mut bytes).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(GENERAL_CANDIDATE_PAGE_CONTENT_DOMAIN_V1);
    hasher.update(&bytes);
    ContentId::new(hasher.finalize().into()).unwrap()
}

fn candidate_id<const N: usize>(submission: CandidateSubmissionV1<N>) -> ContentId {
    let mut bytes = vec![0; CandidateSubmissionV1::<N>::encoded_len().unwrap()];
    submission.encode(&mut bytes).unwrap();
    ContentId::new(Sha256::digest(bytes).into()).unwrap()
}

fn submission(first_page_id: ContentId) -> CandidateSubmissionV1<2> {
    CandidateSubmissionV1 {
        market: [8; 32],
        claim_basis_id: id(3),
        submitter: owner(30),
        generation: 7,
        batch_sequence: 0,
        valid_until_slot: 20,
        claimed_execution_count: 2,
        claimed_page_count: 1,
        claimed_score: 20,
        first_page_id,
        page_rent_reserve_lamports: PAGE_RENT,
        settlement_rent_reserve_lamports: 300,
        claimed_total_quote_debit_numerator: 100,
        prices: [40, 60],
        claimed_net_coefficients: [1, 1],
        outcome_count: 2,
    }
}

fn capitalization<const N: usize>(candidate: CandidateStateV1<N>) -> CandidateCapitalizationV1 {
    CandidateCapitalizationV1 {
        account_lamports: STATE_RENT
            + candidate.page_rent_reserve_remaining
            + candidate.settlement_rent_reserve_remaining
            + candidate.verification_work_remaining
            + candidate.settlement_work_remaining
            + candidate.cleanup_work_remaining,
        exact_state_rent_lamports: STATE_RENT,
    }
}

fn settlement_rent() -> SettlementRentObservationV1 {
    SettlementRentObservationV1 {
        exact_rent_lamports: [100; 3],
        precreation_lamports: [0; 3],
    }
}

fn admitted_candidate(
    batch: &mut BatchRootV1,
) -> (CandidateStateV1<2>, CandidatePageV1<2>, ContentId) {
    let stored_page = page();
    let stored_page_id = page_id(stored_page);
    let submission = submission(stored_page_id);
    let mut candidate = CandidateStateV1::submit(
        candidate_id(submission),
        submission,
        root(),
        config(),
        batch,
        10,
    )
    .expect("submit");
    candidate
        .create_page(
            stored_page,
            config(),
            PAGE_RENT,
            0,
            capitalization(candidate),
        )
        .expect("create page");
    (candidate, stored_page, stored_page_id)
}

fn valid_candidate(
    batch: &mut BatchRootV1,
) -> (CandidateStateV1<2>, CandidatePageV1<2>, ContentId) {
    let (mut candidate, stored_page, stored_page_id) = admitted_candidate(batch);
    candidate
        .verify_page(
            stored_page_id,
            stored_page,
            root(),
            config(),
            *batch,
            11,
            capitalization(candidate),
        )
        .expect("verify");
    candidate
        .finish_verification(config(), *batch, capitalization(candidate), 11)
        .expect("finish verification");
    (candidate, stored_page, stored_page_id)
}

fn round_trip_candidate<const N: usize>(candidate: CandidateStateV1<N>) {
    let mut bytes = vec![0; CandidateStateV1::<N>::encoded_len().unwrap()];
    candidate.encode(&mut bytes).unwrap();
    assert_eq!(CandidateStateV1::<N>::decode(&bytes), Ok(candidate));
}

fn round_trip_cursor<const N: usize>(cursor: SettlementCursorV1<N>) {
    let mut bytes = vec![0; SettlementCursorV1::<N>::encoded_len().unwrap()];
    cursor.encode(&mut bytes).unwrap();
    assert_eq!(SettlementCursorV1::<N>::decode(&bytes), Ok(cursor));
}

#[test]
fn exact_width_config_has_no_market_or_manifest_fixed_point() {
    let config = config();
    let config_bytes = config.to_bytes();
    let config_digest = ContentId::new(Sha256::digest(config_bytes).into()).unwrap();
    let quote = native_amounts();
    let entry = capability_entry(config_digest, quote);
    let mut manifest_bytes = [0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes).unwrap();
    let manifest_digest = ContentId::new(Sha256::digest(manifest_bytes).into()).unwrap();
    let identity = MarketIdentity::new(
        core_id(id(80)),
        core_id(id(81)),
        core_id(config.claim_basis_id()),
        core_id(id(82)),
        core_id(manifest_digest),
        config.generation(),
    );
    let identity_bytes = identity.to_bytes();
    let market_digest = ContentId::new(Sha256::digest(identity_bytes).into()).unwrap();
    assert_ne!(config_digest, manifest_digest);
    assert_ne!(manifest_digest, market_digest);
    assert_eq!(GeneralConfigV1::decode(&config_bytes), Ok(config));
}

#[test]
fn root_is_the_only_post_activation_market_binding() {
    let root = root();
    root.validate_authority([8; 32], id(3), 7, config())
        .unwrap();
    assert_eq!(
        root.validate_authority([9; 32], id(3), 7, config()),
        Err(Error::AuthorityMismatch)
    );
    let mut bytes = [0; GENERAL_ROOT_BYTES];
    root.encode(&mut bytes).unwrap();
    assert_eq!(GENERAL_ROOT_BYTES, 136);
    assert_eq!(GeneralRootV1::decode(&bytes), Ok(root));
}

#[test]
fn page_is_exact_width_noncircular_and_terminal_is_canonical() {
    let page = page();
    assert_eq!(CandidatePageV1::<2>::encoded_len(2), Ok(696));
    assert_eq!(CandidatePageV1::<16>::encoded_len(2), Ok(920));
    let mut bytes = vec![0; CandidatePageV1::<2>::encoded_len(2).unwrap()];
    page.encode(&mut bytes).unwrap();
    assert_eq!(CandidatePageV1::<2>::decode(&bytes), Ok(page));
    let original_id = page_id(page);
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    assert_ne!(
        page_id(CandidatePageV1::<2>::decode(&bytes).unwrap()),
        original_id
    );
    bytes.push(0);
    assert_eq!(
        CandidatePageV1::<2>::decode(&bytes),
        Err(Error::InvalidLength)
    );

    let mut dirty_terminal = vec![0; CandidatePageV1::<2>::encoded_len(2).unwrap()];
    page.encode(&mut dirty_terminal).unwrap();
    dirty_terminal[24] = 1;
    assert_eq!(
        CandidatePageV1::<2>::decode(&dirty_terminal),
        Err(Error::NonCanonicalReservedBytes)
    );
}

#[test]
fn linked_page_substitution_is_refused_atomically() {
    let mut batch = selecting_batch();
    let (mut candidate, stored_page, stored_page_id) = admitted_candidate(&mut batch);
    let before = candidate;
    assert_eq!(
        candidate.verify_page(
            id(99),
            stored_page,
            root(),
            config(),
            batch,
            11,
            capitalization(candidate),
        ),
        Err(Error::CursorMismatch)
    );
    assert_eq!(candidate, before);
    let mut substituted = stored_page;
    substituted.executions[0].as_mut().unwrap().fill_lots = 0;
    assert_eq!(
        candidate.verify_page(
            stored_page_id,
            substituted,
            root(),
            config(),
            batch,
            11,
            capitalization(candidate),
        ),
        Err(Error::InvalidFill)
    );
    assert_eq!(candidate, before);
}

#[test]
fn selected_page_body_cannot_be_withheld_or_replaced_by_wire_bytes() {
    let reference = GeneralCandidatePageV1 {
        candidate_id: id(40),
        page_id: id(50),
    };
    let instruction = GeneralInstructionV1::<2>::CollectSettlementPage(reference);
    let mut wire = [0; 80];
    instruction.encode(&mut wire).unwrap();
    assert_eq!(GeneralInstructionV1::<2>::decode(&wire), Ok(instruction));

    let accounts = valid_frame_accounts(GeneralInstructionTagV1::CollectSettlementPage, 2);
    let page_index = accounts
        .iter()
        .enumerate()
        .find_map(|(index, _)| {
            (general_frame_role(GeneralInstructionTagV1::CollectSettlementPage, 2, index)
                == Ok(GeneralAccountRoleV1::ReadonlyCandidatePage))
            .then_some(index)
        })
        .unwrap();
    let mut missing_page = accounts.clone();
    missing_page.remove(page_index);
    assert_eq!(
        GeneralAccountFrameV1::new(
            GeneralInstructionTagV1::CollectSettlementPage,
            2,
            &missing_page,
        ),
        Err(Error::InvalidLength)
    );
    let mut writable_substitution = accounts;
    writable_substitution[page_index].is_writable = true;
    assert_eq!(
        GeneralAccountFrameV1::new(
            GeneralInstructionTagV1::CollectSettlementPage,
            2,
            &writable_substitution,
        ),
        Err(Error::InvalidAccountPrivilege)
    );
}

#[test]
fn page_frames_allow_only_semantically_repeatable_owner_destinations() {
    let mut collect =
        valid_frame_accounts(GeneralInstructionTagV1::CollectSettlementPage, 2);
    collect[23].key = collect[19].key;
    assert!(GeneralAccountFrameV1::new(
        GeneralInstructionTagV1::CollectSettlementPage,
        2,
        &collect,
    )
    .is_ok());

    let mut distribute =
        valid_frame_accounts(GeneralInstructionTagV1::DistributeSettlementPage, 2);
    distribute[20].key = distribute[18].key;
    distribute[21].key = distribute[19].key;
    assert!(GeneralAccountFrameV1::new(
        GeneralInstructionTagV1::DistributeSettlementPage,
        2,
        &distribute,
    )
    .is_ok());

    distribute[19].key = distribute[18].key;
    assert_eq!(
        GeneralAccountFrameV1::new(
            GeneralInstructionTagV1::DistributeSettlementPage,
            2,
            &distribute,
        ),
        Err(Error::AccountAlias)
    );
}

#[test]
fn page_creation_is_dust_safe_with_exact_surplus_routing() {
    let stored_page = page();
    let submission = submission(page_id(stored_page));
    for (dust, top_up, candidate_refund, page_refund) in [(40, 60, 40, 0), (140, 0, 100, 40)] {
        let mut batch = selecting_batch();
        let mut candidate = CandidateStateV1::submit(
            candidate_id(submission),
            submission,
            root(),
            config(),
            &mut batch,
            10,
        )
        .unwrap();
        let plan = candidate
            .create_page(
                stored_page,
                config(),
                PAGE_RENT,
                dust,
                capitalization(candidate),
            )
            .unwrap();
        assert_eq!(plan.page_top_up_lamports(), top_up);
        assert_eq!(plan.candidate_refund_lamports(), candidate_refund);
        assert_eq!(plan.page_surplus_refund_lamports(), page_refund);
        assert_eq!(top_up + candidate_refund, PAGE_RENT);
    }
}

#[test]
fn candidate_capital_is_segregated_and_round_trips() {
    let mut batch = selecting_batch();
    let (mut candidate, stored_page, stored_page_id) = admitted_candidate(&mut batch);
    assert_eq!(candidate.verification_work_remaining(), 3);
    assert_eq!(candidate.settlement_work_remaining(), 6);
    assert_eq!(candidate.cleanup_work_remaining(), 2);
    round_trip_candidate(candidate);
    candidate
        .verify_page(
            stored_page_id,
            stored_page,
            root(),
            config(),
            batch,
            11,
            capitalization(candidate),
        )
        .unwrap();
    candidate
        .finish_verification(config(), batch, capitalization(candidate), 11)
        .unwrap();
    let cap = capitalization(candidate);
    batch
        .consider_candidate(&mut candidate, config(), cap, 12)
        .unwrap();
    assert_eq!(candidate.verification_work_remaining(), 0);
    round_trip_candidate(candidate);
}

#[test]
fn split_settlement_reuses_stored_page_and_closes_on_distribution() {
    let mut batch = selecting_batch();
    let (mut candidate, stored_page, stored_page_id) = valid_candidate(&mut batch);
    let cap = capitalization(candidate);
    batch
        .consider_candidate(&mut candidate, config(), cap, 12)
        .unwrap();
    batch
        .close_selection(config(), batch_capitalization(batch), 20)
        .unwrap();
    let cap = capitalization(candidate);
    let begin = SettlementCursorV1::begin(
        &mut candidate,
        &mut batch,
        root(),
        config(),
        cap,
        settlement_rent(),
        20,
    )
    .unwrap();
    assert_eq!(begin.reward_lamports(), 1);
    let mut cursor = begin.cursor();
    round_trip_cursor(cursor);
    let cap = capitalization(candidate);
    let collected = cursor
        .collect_page(
            stored_page_id,
            stored_page,
            &mut candidate,
            root(),
            config(),
            batch,
            [0, 0],
            0,
            cap,
        )
        .unwrap();
    assert_eq!(
        (
            collected.claim_inventory_after,
            collected.quote_inventory_after
        ),
        ([0, 0], 1)
    );
    let cap = capitalization(candidate);
    let materialized = cursor
        .materialize(&mut candidate, batch, root(), config(), [0, 0], 1, cap)
        .unwrap();
    assert_eq!(
        materialized.action(),
        SettlementMaterializationActionV1::Split(1)
    );
    assert_eq!(materialized.claim_inventory_after(), [1, 1]);
    let cap = capitalization(candidate);
    let distributed = cursor
        .distribute_page(
            stored_page_id,
            stored_page,
            &mut candidate,
            root(),
            config(),
            batch,
            [1, 1],
            0,
            PAGE_RENT,
            cap,
        )
        .unwrap();
    assert_eq!(distributed.claim_inventory_after, [0, 0]);
    assert_eq!(
        distributed.page_close,
        Some(CandidatePageCloseV1 {
            cleanup_reward_lamports: 1,
            rent_credit_lamports: PAGE_RENT,
            rent_beneficiary: owner(30),
        })
    );
    assert_eq!(candidate.open_page_children(), 0);
    let cap = capitalization(candidate);
    cursor
        .finish(&mut candidate, &mut batch, root(), config(), [0, 0], 0, cap)
        .unwrap();
    let cap = capitalization(candidate);
    let settlement_close = cursor
        .close(
            &mut candidate,
            &mut batch,
            root(),
            config(),
            [0, 0],
            0,
            SettlementCloseObservationV1 {
                account_lamports: [100; 3],
                exact_rent_lamports: [100; 3],
            },
            cap,
        )
        .unwrap();
    assert_eq!(settlement_close.continuation_reward_lamports, 1);
    assert_eq!(settlement_close.rent_credit_lamports, 300);
    assert_eq!(settlement_close.rent_beneficiary, owner(30));
    let close = batch
        .close_candidate_child(candidate, config(), capitalization(candidate))
        .unwrap();
    assert_eq!(close.cleanup_reward_lamports, 1);
    assert_eq!(close.rent_credit_lamports, STATE_RENT);
    assert_eq!(batch.open_candidate_children(), 0);
}

#[test]
fn physical_inventory_mismatch_rolls_back() {
    let mut batch = selecting_batch();
    let (mut candidate, stored_page, stored_page_id) = valid_candidate(&mut batch);
    let cap = capitalization(candidate);
    batch
        .consider_candidate(&mut candidate, config(), cap, 12)
        .unwrap();
    batch
        .close_selection(config(), batch_capitalization(batch), 20)
        .unwrap();
    let cap = capitalization(candidate);
    let begin = SettlementCursorV1::begin(
        &mut candidate,
        &mut batch,
        root(),
        config(),
        cap,
        settlement_rent(),
        20,
    )
    .unwrap();
    let mut cursor = begin.cursor();
    let before_cursor = cursor;
    let before_candidate = candidate;
    let cap = capitalization(candidate);
    assert_eq!(
        cursor.collect_page(
            stored_page_id,
            stored_page,
            &mut candidate,
            root(),
            config(),
            batch,
            [1, 0],
            0,
            cap,
        ),
        Err(Error::CustodyMismatch)
    );
    assert_eq!(cursor, before_cursor);
    assert_eq!(candidate, before_candidate);
}

#[test]
fn abandoned_partial_chain_is_cleanup_funded() {
    let mut batch = selecting_batch();
    let stored_page = page();
    let mut submitted = submission(page_id(stored_page));
    submitted.claimed_page_count = 2;
    submitted.page_rent_reserve_lamports = 2 * PAGE_RENT;
    let mut first = stored_page;
    first.next_page_id = Some(id(99));
    let mut candidate = CandidateStateV1::submit(
        candidate_id(submitted),
        submitted,
        root(),
        config(),
        &mut batch,
        10,
    )
    .unwrap();
    candidate
        .create_page(first, config(), PAGE_RENT, 0, capitalization(candidate))
        .unwrap();
    assert_eq!(candidate.cleanup_work_remaining(), 3);
    candidate
        .reject(config(), capitalization(candidate), 21)
        .unwrap();
    let close_page = candidate
        .close_page(batch, config(), capitalization(candidate), PAGE_RENT)
        .unwrap();
    assert_eq!(close_page.cleanup_reward_lamports, 1);
    assert_eq!(candidate.open_page_children(), 0);
    let close = batch
        .close_candidate_child(candidate, config(), capitalization(candidate))
        .unwrap();
    assert_eq!(close.cleanup_reward_lamports, 1);
    assert_eq!(close.rent_credit_lamports, STATE_RENT + PAGE_RENT + 312);
    assert_eq!(batch.open_candidate_children(), 0);
}

#[test]
fn candidate_close_refuses_live_pages_then_pays_each_cleanup_step() {
    let mut batch = selecting_batch();
    let (mut candidate, _, _) = admitted_candidate(&mut batch);
    candidate
        .reject(config(), capitalization(candidate), 21)
        .unwrap();
    assert_eq!(
        batch.close_candidate_child(candidate, config(), capitalization(candidate)),
        Err(Error::NotQuiescent)
    );
    candidate
        .close_page(batch, config(), capitalization(candidate), PAGE_RENT)
        .unwrap();
    let plan = batch
        .close_candidate_child(candidate, config(), capitalization(candidate))
        .unwrap();
    assert_eq!(plan.cleanup_reward_lamports, 1);
    assert_eq!(batch.open_candidate_children(), 0);
}

#[test]
fn fully_verified_loser_funds_page_and_candidate_cleanup() {
    let mut batch = selecting_batch();
    let (mut winner, _, _) = valid_candidate(&mut batch);
    let (mut loser, _, _) = valid_candidate(&mut batch);
    winner.candidate_id = id(40);
    loser.candidate_id = id(41);
    let winner_cap = capitalization(winner);
    batch
        .consider_candidate(&mut winner, config(), winner_cap, 12)
        .unwrap();
    let loser_cap = capitalization(loser);
    batch
        .consider_candidate(&mut loser, config(), loser_cap, 13)
        .unwrap();
    assert_eq!(
        batch.close_selection(config(), batch_capitalization(batch), 20),
        Ok((Some(id(40)), 1))
    );
    let page_plan = loser
        .close_page(batch, config(), capitalization(loser), PAGE_RENT)
        .unwrap();
    assert_eq!(page_plan.cleanup_reward_lamports, 1);
    let close_plan = batch
        .close_candidate_child(loser, config(), capitalization(loser))
        .unwrap();
    assert_eq!(close_plan.cleanup_reward_lamports, 1);
    assert_eq!(batch.open_candidate_children(), 1);
}

#[test]
fn selected_candidate_refuses_damaged_reserved_capital() {
    let mut batch = selecting_batch();
    let (mut candidate, _, _) = valid_candidate(&mut batch);
    let cap = capitalization(candidate);
    batch
        .consider_candidate(&mut candidate, config(), cap, 12)
        .unwrap();
    batch
        .close_selection(config(), batch_capitalization(batch), 20)
        .unwrap();
    candidate.settlement_work_remaining -= 1;
    let before_batch = batch;
    let cap = capitalization(candidate);
    assert_eq!(
        SettlementCursorV1::begin(
            &mut candidate,
            &mut batch,
            root(),
            config(),
            cap,
            settlement_rent(),
            20,
        ),
        Err(Error::InsufficientFunding)
    );
    assert_eq!(batch, before_batch);
}

#[test]
fn work_compartments_are_not_interchangeable_even_at_equal_total_capital() {
    let mut batch = selecting_batch();
    let (mut candidate, _, _) = valid_candidate(&mut batch);
    let cap = capitalization(candidate);
    batch
        .consider_candidate(&mut candidate, config(), cap, 12)
        .unwrap();
    batch
        .close_selection(config(), batch_capitalization(batch), 20)
        .unwrap();
    candidate.cleanup_work_remaining -= 1;
    candidate.settlement_work_remaining += 1;
    let cap = capitalization(candidate);
    assert_eq!(
        SettlementCursorV1::begin(
            &mut candidate,
            &mut batch,
            root(),
            config(),
            cap,
            settlement_rent(),
            20,
        ),
        Err(Error::InsufficientFunding)
    );
}

#[test]
fn market_key_substitution_is_refused_for_signed_children() {
    let mut wrong_order = order(10, 20, [1, -1], 0);
    wrong_order.market = [9; 32];
    assert_eq!(
        wrong_order.worst_case_reserve(root(), config()),
        Err(Error::AuthorityMismatch)
    );

    let stored_page = page();
    let mut submitted = submission(page_id(stored_page));
    submitted.market = [9; 32];
    let mut batch = selecting_batch();
    assert_eq!(
        CandidateStateV1::submit(
            candidate_id(submitted),
            submitted,
            root(),
            config(),
            &mut batch,
            10,
        ),
        Err(Error::AuthorityMismatch)
    );
}

#[test]
fn exact_state_and_wire_widths_have_no_max_padding() {
    assert_eq!(SettlementCursorV1::<2>::encoded_len(), Ok(384));
    assert_eq!(SettlementCursorV1::<16>::encoded_len(), Ok(944));
    assert_eq!(CandidateSubmissionV1::<2>::encoded_len(), Ok(272));
    assert_eq!(CandidateSubmissionV1::<16>::encoded_len(), Ok(608));
    assert_eq!(CandidateStateV1::<2>::encoded_len(), Ok(520));
    assert_eq!(CandidateStateV1::<16>::encoded_len(), Ok(1_080));

    let reference = GeneralCandidatePageV1 {
        candidate_id: id(40),
        page_id: id(50),
    };
    for instruction in [
        GeneralInstructionV1::<2>::VerifyCandidatePage(reference),
        GeneralInstructionV1::<2>::CollectSettlementPage(reference),
        GeneralInstructionV1::<2>::DistributeSettlementPage(reference),
        GeneralInstructionV1::<2>::CloseCandidatePage(reference),
    ] {
        assert_eq!(instruction.encoded_len(), Ok(80));
        let mut bytes = vec![0; 80];
        instruction.encode(&mut bytes).unwrap();
        assert_eq!(GeneralInstructionV1::<2>::decode(&bytes), Ok(instruction));
        bytes.push(0);
        assert_eq!(
            GeneralInstructionV1::<2>::decode(&bytes),
            Err(Error::InvalidLength)
        );
    }
}

#[test]
fn hostile_reserved_substitution_and_child_saturation_refuse() {
    let mut config_bytes = config().to_bytes();
    config_bytes[168] = 1;
    assert_eq!(
        GeneralConfigV1::decode(&config_bytes),
        Err(Error::NonCanonicalReservedBytes)
    );

    let mut batch = selecting_batch();
    batch.open_candidate_children = u32::MAX;
    let stored_page = page();
    let submitted = submission(page_id(stored_page));
    let before = batch;
    assert_eq!(
        CandidateStateV1::submit(
            candidate_id(submitted),
            submitted,
            root(),
            config(),
            &mut batch,
            10,
        ),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(batch, before);
}

#[test]
fn every_action_frame_has_one_exact_constructible_geometry() {
    let tags = [
        GeneralInstructionTagV1::Activate,
        GeneralInstructionTagV1::OpenBatch,
        GeneralInstructionTagV1::LockBatch,
        GeneralInstructionTagV1::AdmitOrder,
        GeneralInstructionTagV1::CancelOrder,
        GeneralInstructionTagV1::CloseOrder,
        GeneralInstructionTagV1::SubmitCandidate,
        GeneralInstructionTagV1::VerifyCandidatePage,
        GeneralInstructionTagV1::FinishCandidate,
        GeneralInstructionTagV1::ConsiderCandidate,
        GeneralInstructionTagV1::LockSelection,
        GeneralInstructionTagV1::BeginSettlement,
        GeneralInstructionTagV1::CollectSettlementPage,
        GeneralInstructionTagV1::FinishSettlement,
        GeneralInstructionTagV1::CloseBatch,
        GeneralInstructionTagV1::Quiesce,
        GeneralInstructionTagV1::CloseGeneral,
        GeneralInstructionTagV1::CloseCandidate,
        GeneralInstructionTagV1::CloseSettlement,
        GeneralInstructionTagV1::MaterializeSettlement,
        GeneralInstructionTagV1::DistributeSettlementPage,
        GeneralInstructionTagV1::CreateCandidatePage,
        GeneralInstructionTagV1::CloseCandidatePage,
        GeneralInstructionTagV1::RejectCandidate,
        GeneralInstructionTagV1::ExpireSettlement,
    ];
    for tag in tags {
        let execution_count = match tag {
            GeneralInstructionTagV1::VerifyCandidatePage
            | GeneralInstructionTagV1::CollectSettlementPage
            | GeneralInstructionTagV1::DistributeSettlementPage => 2,
            _ => 0,
        };
        let accounts = valid_frame_accounts(tag, execution_count);
        let frame = GeneralAccountFrameV1::new(tag, execution_count, &accounts).unwrap();
        assert_eq!(frame.account_count(), accounts.len());
        assert_eq!(frame.execution_count(), execution_count);
    }
}

#[test]
fn batch_creation_and_all_terminal_work_are_present_and_dust_safe() {
    let mut accounts = valid_frame_accounts(GeneralInstructionTagV1::OpenBatch, 0);
    accounts[1].key = [8; 32];
    accounts[4].key = [44; 32];
    accounts[5].key = [55; 32];
    accounts[6].key = [6; 32];
    let frame = GeneralAccountFrameV1::new(GeneralInstructionTagV1::OpenBatch, 0, &accounts)
        .expect("open frame");
    let plan = open_general_batch_v1(
        frame,
        GeneralBatchReplayV1 {
            generation: 7,
            batch_sequence: 0,
        },
        id(9),
        config(),
        root(),
        BatchRentObservationV1 {
            exact_batch_rent_lamports: BATCH_RENT,
            precreation_lamports: 100,
        },
        0,
    )
    .unwrap();
    assert_eq!(plan.batch_account_lamports(), BATCH_RENT + 3);
    assert_eq!(plan.payer_top_up_lamports(), BATCH_RENT - 97);
    assert_eq!(plan.rent_credit_surplus_lamports(), 0);
    assert_eq!(plan.rent_beneficiary(), [6; 32]);

    let mut batch = plan.batch();
    let mut bytes = [0; BATCH_ROOT_BYTES];
    batch.encode(&mut bytes).unwrap();
    assert_eq!(BatchRootV1::decode(&bytes), Ok(batch));
    assert_eq!(batch.work_remaining_lamports(), 3);
    assert_eq!(
        batch.open_selection(config(), batch_capitalization(batch), 10),
        Ok(1)
    );
    assert_eq!(batch.work_remaining_lamports(), 2);
    assert_eq!(
        batch.close_selection(config(), batch_capitalization(batch), 20),
        Ok((None, 1))
    );
    let mut root_after = plan.root_after();
    let close = batch
        .retire(&mut root_after, config(), batch_capitalization(batch))
        .unwrap();
    assert_eq!(close.continuation_reward_lamports, 1);
    assert_eq!(close.rent_credit_lamports, BATCH_RENT);
    assert_eq!(close.rent_beneficiary, [6; 32]);
    assert_eq!(root_after.open_batches(), 0);
    assert_eq!(batch.work_remaining_lamports(), 0);
    batch.encode(&mut bytes).unwrap();
    assert_eq!(BatchRootV1::decode(&bytes), Ok(batch));

    let frame = GeneralAccountFrameV1::new(GeneralInstructionTagV1::OpenBatch, 0, &accounts)
        .expect("open frame");
    let dusty = open_general_batch_v1(
        frame,
        GeneralBatchReplayV1 {
            generation: 7,
            batch_sequence: 0,
        },
        id(9),
        config(),
        root(),
        BatchRentObservationV1 {
            exact_batch_rent_lamports: BATCH_RENT,
            precreation_lamports: BATCH_RENT + 100,
        },
        0,
    )
    .unwrap();
    assert_eq!(dusty.payer_top_up_lamports(), 0);
    assert_eq!(dusty.rent_credit_surplus_lamports(), 97);
}

#[test]
fn activation_binds_config_manifest_and_market_once() {
    let config = config();
    let config_id = ContentId::new(Sha256::digest(config.to_bytes()).into()).unwrap();
    let amounts = native_amounts();
    let entry = capability_entry(config_id, amounts);
    let mut manifest_bytes = [0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes).unwrap();
    let manifest_id = ContentId::new(Sha256::digest(manifest_bytes).into()).unwrap();
    let manifest = CapabilityManifestV1::decode(&manifest_bytes).unwrap();
    let identity = MarketIdentity::new(
        core_id(id(80)),
        core_id(id(81)),
        core_id(config.claim_basis_id()),
        core_id(id(82)),
        core_id(manifest_id),
        7,
    );
    let identity_bytes = identity.to_bytes();
    let market_identity_id = ContentId::new(Sha256::digest(identity_bytes).into()).unwrap();
    let mut market = MarketRoot::founding(identity, [90; 32]).unwrap();
    market.transition_phase(7, Phase::Open).unwrap();
    let custody =
        FundingCustodyObservationV1::native_only(100 + amounts.native_lamports_total(), 100)
            .unwrap();
    let funding = FundingStateV1::new(capability_id(manifest_id), manifest, 0, custody).unwrap();
    let mut accounts = valid_frame_accounts(GeneralInstructionTagV1::Activate, 0);
    accounts[15].key = [90; 32];
    let frame =
        GeneralAccountFrameV1::new(GeneralInstructionTagV1::Activate, 0, &accounts).unwrap();
    let plan = activate_general_v1(
        frame,
        ActivateGeneralV1 {
            expected_market_child_count: 0,
        },
        market,
        config_id,
        config,
        market_identity_id,
        manifest_id,
        manifest,
        funding,
        custody,
        GeneralActivationCapitalizationV1::new(4, 7),
        10,
    )
    .unwrap();
    assert_eq!(plan.root().market(), [2; 32]);
    assert_eq!(plan.commitments().market_identity_id(), market_identity_id);
}

fn native_amounts() -> FundingAmountsV1 {
    FundingAmountsV1::new(
        CompartmentFundingV1::native_lamports(11).unwrap(),
        CompartmentFundingV1::native_lamports(13).unwrap(),
        CompartmentFundingV1::native_lamports(17).unwrap(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::native_lamports(19).unwrap(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::native_lamports(23).unwrap(),
    )
    .unwrap()
}

fn capability_entry(config_id: ContentId, amounts: FundingAmountsV1) -> CapabilityEntryV1 {
    CapabilityEntryV1::new(
        capability_id(GENERAL_CAPABILITY_KIND_ID_V1),
        capability_id(GENERAL_CAPABILITY_RELEASE_ID_V1),
        capability_id(config_id),
        capability_id(config().capacity_profile_id()),
        capability_id(GENERAL_CHILD_SCHEMA_ID_V1),
        capability_id(GENERAL_CHILD_DERIVATION_ID_V1),
        ActivationPolicy::PrepaidLazy,
        100,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(amounts, None).unwrap(),
    )
    .unwrap()
}

fn valid_frame_meta(role: GeneralAccountRoleV1, fill: u8) -> GeneralAccountMetaV1 {
    use GeneralAccountRoleV1 as Role;
    let (is_signer, is_writable, is_executable) = match role {
        Role::Activator | Role::WorkActor | Role::OrderOwnerPayer | Role::CandidateSubmitter => {
            (true, true, false)
        }
        Role::OrderOwner => (true, false, false),
        Role::TokenProgram | Role::SystemProgram => (false, false, true),
        Role::WritableMarket
        | Role::CapabilityFunding
        | Role::WritableRoot
        | Role::WritableGeneralFunding
        | Role::WritableBatch
        | Role::WritableOrderState
        | Role::WritableOrderCustody
        | Role::OwnerPosition
        | Role::QuoteSource
        | Role::QuoteEscrow
        | Role::QuoteDestination
        | Role::WritableRentCredit
        | Role::WritableCandidate
        | Role::WritableCandidatePage
        | Role::WritableSettlementCursor
        | Role::SettlementPosition
        | Role::SettlementQuoteEscrow
        | Role::CollateralVault => (false, true, false),
        _ => (false, false, false),
    };
    let key = match role {
        Role::SystemProgram => GENERAL_SYSTEM_PROGRAM_ID,
        Role::RentSysvar => GENERAL_RENT_SYSVAR_ID,
        Role::ClockSysvar => GENERAL_CLOCK_SYSVAR_ID,
        _ => [fill; 32],
    };
    GeneralAccountMetaV1 {
        key,
        is_signer,
        is_writable,
        is_executable,
    }
}

fn valid_frame_accounts(tag: GeneralInstructionTagV1, count: u8) -> Vec<GeneralAccountMetaV1> {
    let account_count = general_frame_account_count(tag, count).unwrap();
    (0..account_count)
        .map(|index| {
            valid_frame_meta(
                general_frame_role(tag, count, index).unwrap(),
                u8::try_from(index + 1).unwrap(),
            )
        })
        .collect()
}
