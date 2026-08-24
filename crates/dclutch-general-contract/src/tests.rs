use super::*;
use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    CompartmentFundingV1, FundingAmountsV1, FundingCustodyObservationV1, FundingQuoteV1,
    FundingStateV1, FundingStatus, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    RealmCollateralBindingV1,
};
use dclutch_core_contract::{
    ContentId as CoreContentId, MarketIdentity, MarketRoot, Phase as MarketPhase,
};
use sha2::{Digest, Sha256};
use std::{vec, vec::Vec};

fn id(fill: u8) -> ContentId {
    ContentId::new([fill; CONTENT_ID_BYTES]).expect("nonzero test id")
}

fn owner(fill: u8) -> OwnerKeyV1 {
    OwnerKeyV1::new([fill; 32]).expect("nonzero test owner")
}

fn capability_id(fill: u8) -> dclutch_capability_contract::ContentId {
    dclutch_capability_contract::ContentId::new([fill; 32]).expect("capability ID")
}

fn capability_id_from(id: ContentId) -> dclutch_capability_contract::ContentId {
    dclutch_capability_contract::ContentId::new(id.to_bytes()).expect("capability ID")
}

fn core_id_from(id: ContentId) -> CoreContentId {
    CoreContentId::new(id.to_bytes()).expect("core ID")
}

fn capability_entry(config_id: ContentId, quote: FundingAmountsV1) -> CapabilityEntryV1 {
    CapabilityEntryV1::new(
        capability_id_from(GENERAL_CAPABILITY_KIND_ID_V1),
        capability_id_from(GENERAL_CAPABILITY_RELEASE_ID_V1),
        capability_id_from(config_id),
        capability_id_from(config().capacity_profile_id()),
        capability_id_from(GENERAL_CHILD_SCHEMA_ID_V1),
        capability_id_from(GENERAL_CHILD_DERIVATION_ID_V1),
        ActivationPolicy::PrepaidLazy,
        100,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(quote, None).expect("native-only quote"),
    )
    .expect("General capability entry")
}

fn native(amount: u64) -> CompartmentFundingV1 {
    if amount == 0 {
        CompartmentFundingV1::not_applicable()
    } else {
        CompartmentFundingV1::native_lamports(amount).expect("native allocation")
    }
}

fn native_amounts(
    rent: u64,
    creation: u64,
    work: u64,
    provider: u64,
    bounty: u64,
    liquidity: u64,
    service: u64,
) -> FundingAmountsV1 {
    FundingAmountsV1::new(
        native(rent),
        native(creation),
        native(work),
        native(provider),
        native(bounty),
        native(liquidity),
        native(service),
    )
    .expect("typed native amounts")
}

fn funding_custody(amounts: FundingAmountsV1, state_rent: u64) -> FundingCustodyObservationV1 {
    FundingCustodyObservationV1::native_only(
        state_rent
            .checked_add(amounts.native_lamports_total())
            .expect("custody balance"),
        state_rent,
    )
    .expect("native custody")
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn config() -> GeneralConfigV1 {
    GeneralConfigV1::new(GeneralConfigV1Input {
        capacity_profile_id: id(1),
        market_identity_id: id(2),
        claim_basis_id: id(3),
        capability_release_id: GENERAL_CAPABILITY_RELEASE_ID_V1,
        generation: 7,
        price_scale: 100,
        collection_slots: 10,
        selection_slots: 10,
        settlement_slots: 10,
        max_orders_per_candidate: 8,
        max_pages_per_candidate: 2,
        outcome_count: 2,
    })
    .expect("valid config")
}

fn order(
    order_fill: u8,
    owner_fill: u8,
    nonce: u64,
    coefficients: [i64; 2],
    limit: i128,
) -> PortfolioOrderV1<2> {
    PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(owner_fill),
        order_id: id(order_fill),
        generation: 7,
        batch_sequence: 0,
        nonce,
        valid_until_slot: 30,
        max_lots: 1,
        max_quote_debit_per_lot_numerator: limit,
        coefficients,
        outcome_count: 2,
    })
    .expect("valid order")
}

fn selecting_batch() -> BatchRootV1 {
    let mut batch = BatchRootV1::open(id(9), 0, 0, config()).expect("batch opens");
    batch.open_selection(10).expect("selection opens");
    batch
}

fn executions() -> [Option<ExecutionV1<2>>; MAX_EXECUTIONS_PER_PAGE_V1] {
    let first = order(10, 20, 0, [1, 0], 50);
    let second = order(11, 21, 0, [0, 1], 70);
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

fn valid_candidate(batch: BatchRootV1, candidate_fill: u8) -> CandidateStateV1<2> {
    let prices = [40u64, 60];
    let submission = CandidateSubmissionV1 {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        submitter: owner(30),
        initial_transcript_id: id(50),
        generation: 7,
        batch_sequence: 0,
        valid_until_slot: 20,
        claimed_execution_count: 2,
        claimed_score: 20,
        prices,
        outcome_count: 2,
    };
    let mut candidate =
        CandidateStateV1::submit(id(candidate_fill), submission, config(), batch, 10)
            .expect("candidate submits");
    candidate
        .verify_page(
            VerificationPageV1 {
                page_index: 0,
                prior_transcript_id: id(50),
                next_transcript_id: id(51),
                execution_count: 2,
                executions: executions(),
            },
            config(),
            batch,
            11,
        )
        .expect("page verifies");
    candidate
        .finish_verification(config())
        .expect("candidate conserves complete set");
    candidate
}

fn round_trip_general_root(root: GeneralRootV1) {
    let mut bytes = [0; GENERAL_ROOT_BYTES];
    root.encode(&mut bytes).expect("root encodes");
    assert_eq!(&bytes[..8], b"DCLTGRR1");
    assert_eq!(GeneralRootV1::decode(&bytes), Ok(root));
}

fn round_trip_batch_root(batch: BatchRootV1) {
    let mut bytes = [0; BATCH_ROOT_BYTES];
    batch.encode(&mut bytes).expect("batch encodes");
    assert_eq!(&bytes[..8], b"DCLTGBR1");
    assert_eq!(BatchRootV1::decode(&bytes), Ok(batch));
}

fn round_trip_order_state(state: OrderStateV1) {
    let mut bytes = [0; ORDER_STATE_BYTES];
    state.encode(&mut bytes).expect("order state encodes");
    assert_eq!(&bytes[..8], b"DCLTGOS1");
    assert_eq!(OrderStateV1::decode(&bytes), Ok(state));
}

fn round_trip_funding(funding: GeneralFundingV1) {
    let mut bytes = [0; GENERAL_FUNDING_BYTES];
    funding.encode(&mut bytes).expect("funding encodes");
    assert_eq!(&bytes[..8], b"DCLTGFN1");
    assert_eq!(GeneralFundingV1::decode(&bytes), Ok(funding));
}

fn round_trip_candidate(candidate: CandidateStateV1<2>) {
    let mut bytes = vec![0; CandidateStateV1::<2>::encoded_len().expect("candidate width")];
    candidate.encode(&mut bytes).expect("candidate encodes");
    assert_eq!(bytes.get(..8), Some(b"DCLTGCA1".as_slice()));
    assert_eq!(CandidateStateV1::<2>::decode(&bytes), Ok(candidate));
}

fn round_trip_settlement_cursor(cursor: SettlementCursorV1<2>) {
    let mut bytes =
        vec![0; SettlementCursorV1::<2>::encoded_len().expect("settlement cursor width")];
    cursor.encode(&mut bytes).expect("cursor encodes");
    assert_eq!(bytes.get(..8), Some(b"DCLTGSC1".as_slice()));
    assert_eq!(SettlementCursorV1::<2>::decode(&bytes), Ok(cursor));
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
        | Role::WritableSettlementCursor
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
    let length = general_frame_account_count(tag, count).expect("frame width");
    let mut accounts = Vec::with_capacity(length);
    for index in 0..length {
        let role = general_frame_role(tag, count, index).expect("ordered role");
        accounts.push(valid_frame_meta(
            role,
            u8::try_from(index + 1).expect("small frame"),
        ));
    }
    accounts
}

#[test]
fn golden_config_and_order_round_trip() {
    let config = config();
    let bytes = config.to_bytes();
    assert_eq!(bytes.len(), GENERAL_CONFIG_BYTES);
    assert_eq!(&bytes[..8], b"DCLTGEN1");
    assert_eq!(GeneralConfigV1::decode(&bytes), Ok(config));

    let order = order(10, 20, 9, [-2, 3], 77);
    let mut bytes = [0; 216];
    order.encode(&mut bytes).expect("order encodes");
    assert_eq!(
        bytes.len(),
        PortfolioOrderV1::<2>::encoded_len().expect("length")
    );
    assert_eq!(&bytes[..8], b"DCLTGOR1");
    assert_eq!(PortfolioOrderV1::<2>::decode(&bytes), Ok(order));
}

#[test]
fn general_config_is_one_noncircular_permanent_generic_record() {
    let canonical = config();
    let bytes = canonical.to_bytes();
    let config_id = digest(&bytes);
    assert_eq!(bytes.len(), GENERAL_CONFIG_BYTES);
    assert_eq!(GeneralConfigV1::decode(&bytes), Ok(canonical));
    assert_eq!(
        digest(GENERAL_CONFIG_SCHEMA_PREIMAGE_V1),
        GENERAL_CONFIG_SCHEMA_ID_V1.to_bytes()
    );

    let mut changed_bytes = bytes;
    changed_bytes[152..160].copy_from_slice(&(canonical.price_scale() + 1).to_le_bytes());
    let changed = GeneralConfigV1::decode(&changed_bytes).expect("changed config");
    assert_ne!(digest(&changed.to_bytes()), config_id);

    let instruction = GeneralInstructionV1::<2>::Activate(ActivateGeneralV1 {
        expected_market_child_count: 7,
    });
    assert_eq!(instruction.encoded_len(), Ok(24));
    let mut wire = [0u8; 24];
    instruction.encode(&mut wire).expect("activation wire");
    assert_eq!(GeneralInstructionV1::<2>::decode(&wire), Ok(instruction));
    assert!(
        !wire
            .windows(config_id.len())
            .any(|window| window == config_id)
    );
}

#[test]
fn signed_order_identity_uses_one_exact_noncircular_preimage() {
    let provisional = order(10, 20, 9, [-2, 3], 77);
    assert_eq!(PortfolioOrderV1::<2>::signing_preimage_len(), Ok(184));
    assert_eq!(PortfolioOrderV1::<16>::signing_preimage_len(), Ok(296));
    let mut preimage = vec![0; PortfolioOrderV1::<2>::signing_preimage_len().expect("length")];
    provisional
        .encode_signing_preimage(&mut preimage)
        .expect("canonical signing message");
    assert_eq!(preimage.get(..8), Some(b"DCLTGOM1".as_slice()));
    let order_id = ContentId::new(digest(&preimage)).expect("nonzero order ID");
    let committed = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id,
        generation: 7,
        batch_sequence: 0,
        nonce: 9,
        valid_until_slot: 30,
        max_lots: 1,
        max_quote_debit_per_lot_numerator: 77,
        coefficients: [-2, 3],
        outcome_count: 2,
    })
    .expect("committed order");
    let mut committed_preimage = vec![0; preimage.len()];
    committed
        .encode_signing_preimage(&mut committed_preimage)
        .expect("same message");
    assert_eq!(committed_preimage, preimage);
    assert_eq!(digest(&committed_preimage), committed.order_id().to_bytes());

    let substituted_id = order(11, 20, 9, [-2, 3], 77);
    let mut substituted_id_preimage = vec![0; preimage.len()];
    substituted_id
        .encode_signing_preimage(&mut substituted_id_preimage)
        .expect("message excludes self ID");
    assert_eq!(substituted_id_preimage, preimage);
    assert_ne!(
        digest(&substituted_id_preimage),
        substituted_id.order_id().to_bytes()
    );

    let substituted_body = order(10, 20, 9, [-3, 3], 77);
    let mut substituted_body_preimage = vec![0; preimage.len()];
    substituted_body
        .encode_signing_preimage(&mut substituted_body_preimage)
        .expect("substituted message");
    assert_ne!(digest(&substituted_body_preimage), order_id.to_bytes());
    let short_length = committed_preimage.len() - 1;
    assert_eq!(
        committed.encode_signing_preimage(
            committed_preimage
                .get_mut(..short_length)
                .expect("short output")
        ),
        Err(Error::InvalidLength)
    );
}

#[test]
fn hostile_decoders_reject_reserved_and_unused_words() {
    assert_eq!(GENERAL_CONFIG_BYTES, 200);
    let mut config_bytes = config().to_bytes();
    config_bytes[199] = 1;
    assert_eq!(
        GeneralConfigV1::decode(&config_bytes),
        Err(Error::NonCanonicalReservedBytes)
    );
    let mut substituted_release = config().to_bytes();
    substituted_release[112] ^= 1;
    assert_eq!(
        GeneralConfigV1::decode(&substituted_release),
        Err(Error::UnrecognizedCapability)
    );

    let mut encoded_order = [0; 216];
    order(10, 20, 9, [1, -1], 10)
        .encode(&mut encoded_order)
        .expect("order encodes");
    let mut order_bytes = [0; 224];
    order_bytes[..216].copy_from_slice(&encoded_order);
    order_bytes[216] = 1;
    assert_eq!(
        PortfolioOrderV1::<2>::decode(&order_bytes),
        Err(Error::InvalidLength)
    );

    let mut zero_owner = encoded_order;
    zero_owner[80..112].fill(0);
    assert_eq!(
        PortfolioOrderV1::<2>::decode(&zero_owner),
        Err(Error::ZeroIdentifier)
    );
}

#[test]
fn outcome_vector_codecs_scale_exactly_with_selected_width() {
    assert_eq!(PortfolioOrderV1::<2>::encoded_len(), Ok(216));
    assert_eq!(PortfolioOrderV1::<16>::encoded_len(), Ok(328));
    assert_eq!(SettlementReceiptV1::<2>::encoded_len(), Ok(192));
    assert_eq!(SettlementReceiptV1::<16>::encoded_len(), Ok(304));

    let order = order(10, 20, 9, [1, -1], 10);
    let mut short = [0; 216];
    order.encode(&mut short).expect("two-outcome order encodes");
    assert_eq!(
        PortfolioOrderV1::<16>::decode(&short),
        Err(Error::InvalidLength)
    );

    let bad_width = PortfolioOrderV1::<2>::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(10),
        generation: 7,
        batch_sequence: 0,
        nonce: 9,
        valid_until_slot: 30,
        max_lots: 1,
        max_quote_debit_per_lot_numerator: 10,
        coefficients: [1, -1],
        outcome_count: 16,
    });
    assert_eq!(bad_width, Err(Error::InvalidOutcomeCount));

    let receipt = SettlementReceiptV1::<2> {
        candidate_id: id(40),
        order_id: id(10),
        owner: owner(20),
        generation: 7,
        batch_sequence: 0,
        nonce: 9,
        fill_lots: 1,
        remaining_lots: 0,
        quote_delta_atoms: -1,
        carry_before: 0,
        carry_after: 0,
        outcome_deltas: [1, -1],
        outcome_count: 2,
    };
    let mut receipt_bytes = [0; 192];
    receipt
        .encode(&mut receipt_bytes)
        .expect("two-outcome receipt encodes");
    assert_eq!(
        SettlementReceiptV1::<16>::decode(&receipt_bytes),
        Err(Error::InvalidLength)
    );
    receipt_bytes[12..14].copy_from_slice(&16_u16.to_le_bytes());
    assert_eq!(
        SettlementReceiptV1::<2>::decode(&receipt_bytes),
        Err(Error::InvalidOutcomeCount)
    );
}

#[test]
fn simplex_is_exact_and_has_one_canonical_width() {
    let batch = selecting_batch();
    let mut prices = [40u64, 59];
    let submission = CandidateSubmissionV1 {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        submitter: owner(30),
        initial_transcript_id: id(50),
        generation: 7,
        batch_sequence: 0,
        valid_until_slot: 20,
        claimed_execution_count: 2,
        claimed_score: 20,
        prices,
        outcome_count: 2,
    };
    assert_eq!(
        CandidateStateV1::submit(id(40), submission, config(), batch, 10),
        Err(Error::InvalidSimplexPrice)
    );

    prices[1] = 60;
    assert!(
        CandidateStateV1::submit(
            id(40),
            CandidateSubmissionV1 {
                prices,
                ..submission
            },
            config(),
            batch,
            10
        )
        .is_ok()
    );
}

#[test]
fn paginated_failure_does_not_advance_cursor() {
    let batch = selecting_batch();
    let prices = [40u64, 60];
    let mut candidate = CandidateStateV1::submit(
        id(40),
        CandidateSubmissionV1 {
            market_identity_id: id(2),
            claim_basis_id: id(3),
            submitter: owner(30),
            initial_transcript_id: id(50),
            generation: 7,
            batch_sequence: 0,
            valid_until_slot: 20,
            claimed_execution_count: 2,
            claimed_score: 20,
            prices,
            outcome_count: 2,
        },
        config(),
        batch,
        10,
    )
    .expect("submits");
    let before = candidate;
    let mut bad = executions();
    let mut second = bad[1].expect("second");
    second.order.order_id = id(9);
    second.order_state = OrderStateV1::open(second.order);
    bad[1] = Some(second);
    assert_eq!(
        candidate.verify_page(
            VerificationPageV1 {
                page_index: 0,
                prior_transcript_id: id(50),
                next_transcript_id: id(51),
                execution_count: 2,
                executions: bad,
            },
            config(),
            batch,
            11,
        ),
        Err(Error::NonCanonicalOrder)
    );
    assert_eq!(candidate, before);
}

#[test]
fn imbalance_cannot_reach_valid_candidate() {
    let batch = selecting_batch();
    let prices = [40u64, 60];
    let first = order(10, 20, 0, [1, 0], 50);
    let mut candidate = CandidateStateV1::submit(
        id(40),
        CandidateSubmissionV1 {
            market_identity_id: id(2),
            claim_basis_id: id(3),
            submitter: owner(30),
            initial_transcript_id: id(50),
            generation: 7,
            batch_sequence: 0,
            valid_until_slot: 20,
            claimed_execution_count: 1,
            claimed_score: 10,
            prices,
            outcome_count: 2,
        },
        config(),
        batch,
        10,
    )
    .expect("submits");
    candidate
        .verify_page(
            VerificationPageV1 {
                page_index: 0,
                prior_transcript_id: id(50),
                next_transcript_id: id(51),
                execution_count: 1,
                executions: [
                    Some(ExecutionV1 {
                        order: first,
                        order_state: OrderStateV1::open(first),
                        fill_lots: 1,
                    }),
                    None,
                    None,
                    None,
                ],
            },
            config(),
            batch,
            11,
        )
        .expect("page arithmetic is locally valid");
    assert_eq!(
        candidate.finish_verification(config()),
        Err(Error::IncompleteSetImbalance)
    );
}

#[test]
fn deterministic_best_valid_submitted_candidate_tie_breaks_by_id() {
    let mut batch = selecting_batch();
    let mut high_id = valid_candidate(batch, 42);
    let mut low_id = valid_candidate(batch, 41);
    batch
        .consider_candidate(&mut high_id, 12)
        .expect("consider high");
    assert_eq!(
        batch.consider_candidate(&mut high_id, 12),
        Err(Error::CandidateClaimMismatch)
    );
    batch
        .consider_candidate(&mut low_id, 13)
        .expect("consider low");
    assert_eq!(batch.close_selection(20), Ok(Some(id(41))));
}

#[test]
fn settlement_prefix_carry_conserves_exact_complete_set() {
    let mut batch = selecting_batch();
    let mut candidate = valid_candidate(batch, 40);
    batch
        .consider_candidate(&mut candidate, 12)
        .expect("candidate considered");
    assert_eq!(batch.close_selection(20), Ok(Some(id(40))));
    let mut hoard = HoardLedgerV1::new(id(2), 0, 0).expect("empty hoard");
    let mut cursor = SettlementCursorV1::begin(candidate, &mut batch, &mut hoard, config(), 20)
        .expect("settlement begins");
    assert_eq!(batch.phase(), BatchPhase::Applying);
    assert_eq!(hoard.principal_atoms(), 1);
    let result = cursor
        .settle_page(
            VerificationPageV1 {
                page_index: 0,
                prior_transcript_id: id(50),
                next_transcript_id: id(51),
                execution_count: 2,
                executions: executions(),
            },
            candidate,
            config(),
            batch,
        )
        .expect("page settles");
    let first = result.receipts[0].expect("first receipt");
    let second = result.receipts[1].expect("second receipt");
    assert_eq!(
        (
            first.quote_delta_atoms,
            first.carry_before,
            first.carry_after
        ),
        (-1, 0, 60)
    );
    assert_eq!(
        (
            second.quote_delta_atoms,
            second.carry_before,
            second.carry_after
        ),
        (0, 60, 0)
    );
    assert_eq!(first.quote_delta_atoms + second.quote_delta_atoms, -1);
    assert_eq!(
        result.order_states[0].expect("first state").phase(),
        OrderPhase::Consumed
    );

    let mut bytes = [0; 192];
    first.encode(&mut bytes).expect("receipt encodes");
    assert_eq!(SettlementReceiptV1::<2>::decode(&bytes), Ok(first));
    cursor
        .finish(candidate, &mut batch)
        .expect("exact settlement finishes");
    assert_eq!(hoard.principal_atoms(), 1);
    assert_eq!(hoard.liability_units_per_outcome(), 1);
    assert_eq!(batch.phase(), BatchPhase::Quiescent);
}

#[test]
fn applying_batch_cannot_expire_after_hoard_conversion() {
    let mut batch = selecting_batch();
    let mut candidate = valid_candidate(batch, 40);
    batch
        .consider_candidate(&mut candidate, 12)
        .expect("candidate considered");
    batch.close_selection(20).expect("winner freezes");
    let mut hoard = HoardLedgerV1::new(id(2), 0, 0).expect("empty hoard");
    let _cursor = SettlementCursorV1::begin(candidate, &mut batch, &mut hoard, config(), 20)
        .expect("application commits");
    assert_eq!(batch.expire_unsettled(31), Err(Error::OutsideWindow));
    assert_eq!(batch.phase(), BatchPhase::Applying);
    assert_eq!(hoard.principal_atoms(), 1);
}

#[test]
fn failed_complete_set_burn_leaves_batch_and_hoard_unchanged() {
    let mut batch = selecting_batch();
    let mut candidate = valid_candidate(batch, 40);
    candidate.complete_set_delta = -1;
    candidate.total_quote_debit_numerator = -100;
    batch
        .consider_candidate(&mut candidate, 12)
        .expect("candidate considered");
    batch.close_selection(20).expect("winner freezes");
    let before_batch = batch;
    let mut hoard = HoardLedgerV1::new(id(2), 0, 0).expect("empty hoard");
    let before_hoard = hoard;
    assert_eq!(
        SettlementCursorV1::begin(candidate, &mut batch, &mut hoard, config(), 20),
        Err(Error::InsufficientHoardPrincipal)
    );
    assert_eq!(batch, before_batch);
    assert_eq!(hoard, before_hoard);
}

#[test]
fn cancellation_replay_and_expiry_are_refused() {
    let order = order(10, 20, 5, [1, -1], 0);
    let mut state = OrderStateV1::open(order);
    assert_eq!(state.cancel(owner(20), 9, 10), Ok(()));
    assert_eq!(
        state.validate_snapshot(order, 1),
        Err(Error::OrderUnavailable)
    );

    let mut locked = OrderStateV1::open(order);
    assert_eq!(locked.cancel(owner(20), 10, 10), Err(Error::OutsideWindow));
    assert_eq!(locked.phase(), OrderPhase::Open);

    let mut expired = order;
    expired.valid_until_slot = 29;
    let execution = ExecutionV1 {
        order: expired,
        order_state: OrderStateV1::open(expired),
        fill_lots: 1,
    };
    assert_eq!(
        validate_execution_binding(execution, config(), selecting_batch()),
        Err(Error::OrderExpired)
    );
}

#[test]
fn funding_is_prepaid_segregated_and_conserved() {
    let mut funding = GeneralFundingV1::founding(id(4), 10, 20, 30);
    assert_eq!(
        funding.debit(FundingCompartment::Work, 21, id(60)),
        Err(Error::InsufficientFunding)
    );
    let debit = funding
        .debit(FundingCompartment::Work, 7, id(60))
        .expect("prepaid work");
    assert_eq!(debit.amount, 7);
    funding.validate().expect("still conserved");
    assert_eq!(
        funding.refund_terminal(GeneralPhase::Quiescing),
        Err(Error::NotQuiescent)
    );
    assert_eq!(
        funding.refund_terminal(GeneralPhase::Terminal),
        Ok([10, 13, 30])
    );
    assert!(funding.is_discharged());
    funding.validate().expect("refund conserved");
}

#[test]
fn root_requires_real_quiescence_before_retirement() {
    let mut root = GeneralRootV1::founding([8; 32], id(9), 7, [90; 32]).expect("root");
    assert_eq!(root.open_batch(), Ok(0));
    root.request_quiescence().expect("quiescing");
    assert_eq!(root.enter_terminal(), Err(Error::NotQuiescent));
    root.close_batch().expect("last batch closes");
    root.enter_terminal().expect("terminal");
    assert_eq!(root.retire(false), Err(Error::NotQuiescent));
    root.retire(true).expect("funding discharged");
    assert_eq!(root.phase(), GeneralPhase::Retired);
}

#[test]
fn general_root_codec_round_trips_every_phase_and_counter_transition() {
    let mut root = GeneralRootV1::founding([8; 32], id(9), 7, [90; 32]).expect("root");
    round_trip_general_root(root);
    assert_eq!(root.open_batch(), Ok(0));
    assert_eq!(root.open_batch(), Ok(1));
    round_trip_general_root(root);

    root.request_quiescence().expect("quiescing");
    round_trip_general_root(root);
    root.close_batch().expect("one child retires");
    round_trip_general_root(root);
    root.close_batch().expect("last child retires");
    root.enter_terminal().expect("terminal");
    round_trip_general_root(root);
    root.retire(true).expect("retired");
    round_trip_general_root(root);
}

#[test]
fn batch_root_codec_round_trips_every_phase_and_winner_shape() {
    let mut empty = BatchRootV1::open(id(9), 1, 0, config()).expect("empty batch opens");
    empty.open_selection(10).expect("empty selection opens");
    assert_eq!(empty.close_selection(20), Ok(None));
    round_trip_batch_root(empty);
    empty.retire(true).expect("empty batch retires");
    round_trip_batch_root(empty);

    let mut batch = BatchRootV1::open(id(9), 0, 0, config()).expect("batch opens");
    round_trip_batch_root(batch);
    batch.open_selection(10).expect("selection opens");
    round_trip_batch_root(batch);

    let mut candidate = valid_candidate(batch, 40);
    batch
        .consider_candidate(&mut candidate, 12)
        .expect("candidate considered");
    round_trip_batch_root(batch);
    batch.close_selection(20).expect("winner freezes");
    round_trip_batch_root(batch);

    let mut hoard = HoardLedgerV1::new(id(2), 0, 0).expect("empty hoard");
    let mut cursor = SettlementCursorV1::begin(candidate, &mut batch, &mut hoard, config(), 20)
        .expect("applying begins");
    round_trip_batch_root(batch);
    cursor
        .settle_page(
            VerificationPageV1 {
                page_index: 0,
                prior_transcript_id: id(50),
                next_transcript_id: id(51),
                execution_count: 2,
                executions: executions(),
            },
            candidate,
            config(),
            batch,
        )
        .expect("page settles");
    cursor
        .finish(candidate, &mut batch)
        .expect("batch quiesces");
    round_trip_batch_root(batch);
    batch.retire(true).expect("batch retires");
    round_trip_batch_root(batch);
}

#[test]
fn order_state_codec_round_trips_open_cancelled_partial_and_consumed() {
    let signed = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(10),
        generation: 7,
        batch_sequence: 0,
        nonce: 55,
        valid_until_slot: 30,
        max_lots: 2,
        max_quote_debit_per_lot_numerator: 0,
        coefficients: [1, -1],
        outcome_count: 2,
    })
    .expect("valid order");
    let mut partial = OrderStateV1::open(signed);
    round_trip_order_state(partial);
    partial.consume(signed, 1).expect("one lot consumes");
    round_trip_order_state(partial);
    partial.consume(signed, 1).expect("last lot consumes");
    round_trip_order_state(partial);

    let mut cancelled = OrderStateV1::open(signed);
    cancelled.cancel(owner(20), 9, 10).expect("owner cancels");
    round_trip_order_state(cancelled);

    let mut batch = BatchRootV1::open(id(9), 0, 0, config()).expect("batch");
    batch.open_selection(10).expect("selecting");
    batch.close_selection(20).expect("empty quiescent");
    let mut released = OrderStateV1::open(signed);
    released
        .release_after_batch(signed, batch)
        .expect("remainder releases");
    round_trip_order_state(released);
}

#[test]
fn general_funding_codec_round_trips_debits_and_atomic_terminal_refund() {
    let mut funding = GeneralFundingV1::founding(id(4), 10, 20, 30);
    round_trip_funding(funding);
    funding
        .debit(FundingCompartment::Liveness, 3, id(60))
        .expect("liveness debit");
    funding
        .debit(FundingCompartment::Work, 7, id(61))
        .expect("work debit");
    round_trip_funding(funding);
    assert_eq!(
        funding.refund_terminal(GeneralPhase::Terminal),
        Ok([7, 13, 30])
    );
    round_trip_funding(funding);
}

#[test]
fn mutable_state_codecs_reject_wrong_width_type_headers_and_reserved_bytes() {
    let root = GeneralRootV1::founding([8; 32], id(9), 7, [90; 32]).expect("root");
    let mut root_bytes = [0; GENERAL_ROOT_BYTES];
    root.encode(&mut root_bytes).expect("root encodes");
    assert_eq!(
        GeneralRootV1::decode(&root_bytes[..GENERAL_ROOT_BYTES - 1]),
        Err(Error::InvalidLength)
    );
    let mut root_trailing = [0; GENERAL_ROOT_BYTES + 1];
    root_trailing[..GENERAL_ROOT_BYTES].copy_from_slice(&root_bytes);
    assert_eq!(
        GeneralRootV1::decode(&root_trailing),
        Err(Error::InvalidLength)
    );
    root_bytes[..8].copy_from_slice(b"DCLTGBR1");
    assert_eq!(GeneralRootV1::decode(&root_bytes), Err(Error::InvalidMagic));

    let mut funding_bytes = [0; GENERAL_FUNDING_BYTES];
    GeneralFundingV1::founding(id(4), 1, 2, 3)
        .encode(&mut funding_bytes)
        .expect("funding encodes");
    assert_eq!(
        GeneralFundingV1::decode(&funding_bytes[..GENERAL_FUNDING_BYTES - 1]),
        Err(Error::InvalidLength)
    );
    let mut funding_trailing = [0; GENERAL_FUNDING_BYTES + 1];
    funding_trailing[..GENERAL_FUNDING_BYTES].copy_from_slice(&funding_bytes);
    assert_eq!(
        GeneralFundingV1::decode(&funding_trailing),
        Err(Error::InvalidLength)
    );
    funding_bytes[12] = 1;
    assert_eq!(
        GeneralFundingV1::decode(&funding_bytes),
        Err(Error::NonCanonicalReservedBytes)
    );

    let mut batch_bytes = [0; BATCH_ROOT_BYTES];
    BatchRootV1::open(id(9), 0, 0, config())
        .expect("batch opens")
        .encode(&mut batch_bytes)
        .expect("batch encodes");
    assert_eq!(
        BatchRootV1::decode(&batch_bytes[..BATCH_ROOT_BYTES - 1]),
        Err(Error::InvalidLength)
    );
    let mut batch_trailing = [0; BATCH_ROOT_BYTES + 1];
    batch_trailing[..BATCH_ROOT_BYTES].copy_from_slice(&batch_bytes);
    assert_eq!(
        BatchRootV1::decode(&batch_trailing),
        Err(Error::InvalidLength)
    );
    batch_bytes[84] = 1;
    assert_eq!(
        BatchRootV1::decode(&batch_bytes),
        Err(Error::NonCanonicalReservedBytes)
    );

    let signed = order(10, 20, 9, [1, -1], 0);
    let mut order_state_bytes = [0; ORDER_STATE_BYTES];
    OrderStateV1::open(signed)
        .encode(&mut order_state_bytes)
        .expect("order state encodes");
    assert_eq!(
        OrderStateV1::decode(&order_state_bytes[..ORDER_STATE_BYTES - 1]),
        Err(Error::InvalidLength)
    );
    let mut order_state_trailing = [0; ORDER_STATE_BYTES + 1];
    order_state_trailing[..ORDER_STATE_BYTES].copy_from_slice(&order_state_bytes);
    assert_eq!(
        OrderStateV1::decode(&order_state_trailing),
        Err(Error::InvalidLength)
    );
    order_state_bytes[13] = 1;
    assert_eq!(
        OrderStateV1::decode(&order_state_bytes),
        Err(Error::NonCanonicalReservedBytes)
    );
}

#[test]
fn general_root_decoder_rejects_unreachable_counts_terminal_children_and_tags() {
    let mut root = GeneralRootV1::founding([8; 32], id(9), 7, [90; 32]).expect("root");
    root.open_batch().expect("batch reserves");
    let mut bytes = [0; GENERAL_ROOT_BYTES];
    root.encode(&mut bytes).expect("root encodes");

    bytes[128..132].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(GeneralRootV1::decode(&bytes), Err(Error::NonCanonicalState));
    bytes[128..132].copy_from_slice(&1_u32.to_le_bytes());
    bytes[12] = general_phase_tag(GeneralPhase::Terminal);
    assert_eq!(GeneralRootV1::decode(&bytes), Err(Error::NonCanonicalState));
    bytes[12] = u8::MAX;
    assert_eq!(GeneralRootV1::decode(&bytes), Err(Error::InvalidPhase));
    bytes[12] = general_phase_tag(GeneralPhase::Active);
    bytes[16..48].fill(0);
    assert_eq!(GeneralRootV1::decode(&bytes), Err(Error::ZeroIdentifier));
    bytes[16..48].copy_from_slice(id(9).as_bytes());
    bytes[48..80].fill(0);
    assert_eq!(GeneralRootV1::decode(&bytes), Err(Error::ZeroIdentifier));
    bytes[48..80].copy_from_slice(&[8; 32]);
    bytes[80..112].fill(0);
    assert_eq!(GeneralRootV1::decode(&bytes), Err(Error::ZeroIdentifier));
}

#[test]
fn batch_root_decoder_rejects_bad_option_shape_deadlines_and_phase_substitution() {
    let mut bytes = [0; BATCH_ROOT_BYTES];
    BatchRootV1::open(id(9), 0, 0, config())
        .expect("batch opens")
        .encode(&mut bytes)
        .expect("batch encodes");

    bytes[88] = 1;
    assert_eq!(
        BatchRootV1::decode(&bytes),
        Err(Error::NonCanonicalReservedBytes)
    );
    bytes[88] = 0;
    bytes[13] = 2;
    assert_eq!(BatchRootV1::decode(&bytes), Err(Error::NonCanonicalState));
    bytes[13] = 0;
    bytes[64..72].copy_from_slice(&10_u64.to_le_bytes());
    assert_eq!(BatchRootV1::decode(&bytes), Err(Error::NonCanonicalState));

    let mut selecting = selecting_batch();
    let mut candidate = valid_candidate(selecting, 40);
    selecting
        .consider_candidate(&mut candidate, 12)
        .expect("candidate considered");
    selecting
        .encode(&mut bytes)
        .expect("winner-bearing selection encodes");
    bytes[12] = batch_phase_tag(BatchPhase::Collecting);
    assert_eq!(BatchRootV1::decode(&bytes), Err(Error::NonCanonicalState));
}

#[test]
fn order_state_decoder_rejects_phase_balance_and_identity_substitution() {
    let signed = order(10, 20, 9, [1, -1], 0);
    let state = OrderStateV1::open(signed);
    let mut bytes = [0; ORDER_STATE_BYTES];
    state.encode(&mut bytes).expect("state encodes");

    bytes[88..96].copy_from_slice(&2_u64.to_le_bytes());
    let inflated = OrderStateV1::decode(&bytes).expect("internally shaped state");
    assert_eq!(
        inflated.authenticate(signed),
        Err(Error::OrderBindingMismatch)
    );
    bytes[88..96].copy_from_slice(&1_u64.to_le_bytes());
    bytes[16..48].copy_from_slice(id(11).as_bytes());
    let substituted = OrderStateV1::decode(&bytes).expect("internally shaped state");
    assert_eq!(
        substituted.authenticate(signed),
        Err(Error::OrderBindingMismatch)
    );
    bytes[16..48].copy_from_slice(id(10).as_bytes());

    bytes[88..96].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(OrderStateV1::decode(&bytes), Err(Error::NonCanonicalState));
    bytes[12] = order_phase_tag(OrderPhase::Consumed);
    bytes[88..96].copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(OrderStateV1::decode(&bytes), Err(Error::NonCanonicalState));
    bytes[12] = u8::MAX;
    assert_eq!(OrderStateV1::decode(&bytes), Err(Error::InvalidPhase));
    bytes[12] = order_phase_tag(OrderPhase::Open);
    bytes[16..48].fill(0);
    assert_eq!(OrderStateV1::decode(&bytes), Err(Error::ZeroIdentifier));
}

#[test]
fn funding_decoder_rejects_overflow_broken_conservation_and_partial_refund() {
    let mut bytes = [0; GENERAL_FUNDING_BYTES];
    GeneralFundingV1::founding(id(4), u64::MAX, 20, 30)
        .encode(&mut bytes)
        .expect("funding encodes");
    bytes[96..104].copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(
        GeneralFundingV1::decode(&bytes),
        Err(Error::ArithmeticOverflow)
    );

    GeneralFundingV1::founding(id(4), 10, 20, 30)
        .encode(&mut bytes)
        .expect("funding encodes");
    bytes[72..80].copy_from_slice(&9_u64.to_le_bytes());
    assert_eq!(
        GeneralFundingV1::decode(&bytes),
        Err(Error::FundingConservationMismatch)
    );
    bytes[120..128].copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(
        GeneralFundingV1::decode(&bytes),
        Err(Error::NonCanonicalState)
    );
}

#[test]
fn recognized_general_release_and_config_schema_ids_are_derived_and_pda_domains_are_distinct() {
    assert_eq!(
        digest(GENERAL_CAPABILITY_KIND_PREIMAGE_V1),
        GENERAL_CAPABILITY_KIND_ID_V1.to_bytes()
    );
    assert_eq!(
        digest(GENERAL_CAPABILITY_RELEASE_PREIMAGE_V1),
        GENERAL_CAPABILITY_RELEASE_ID_V1.to_bytes()
    );
    assert_eq!(
        digest(GENERAL_CHILD_SCHEMA_PREIMAGE_V1),
        GENERAL_CHILD_SCHEMA_ID_V1.to_bytes()
    );
    assert_eq!(
        digest(GENERAL_CHILD_DERIVATION_PREIMAGE_V1),
        GENERAL_CHILD_DERIVATION_ID_V1.to_bytes()
    );
    assert_eq!(
        digest(GENERAL_CONFIG_SCHEMA_PREIMAGE_V1),
        GENERAL_CONFIG_SCHEMA_ID_V1.to_bytes()
    );

    let domains = [
        GENERAL_FUNDING_PDA_DOMAIN_V1,
        GENERAL_ROOT_PDA_DOMAIN_V1,
        GENERAL_BATCH_PDA_DOMAIN_V1,
        GENERAL_ORDER_STATE_PDA_DOMAIN_V1,
        GENERAL_ORDER_CUSTODY_PDA_DOMAIN_V1,
        GENERAL_QUOTE_ESCROW_PDA_DOMAIN_V1,
        GENERAL_CANDIDATE_PDA_DOMAIN_V1,
        GENERAL_SETTLEMENT_CURSOR_PDA_DOMAIN_V1,
    ];
    for (index, domain) in domains.iter().enumerate() {
        assert!(!domain.is_empty());
        assert!(domain.len() <= 32);
        assert!(domains.iter().skip(index + 1).all(|other| other != domain));
    }
}

#[test]
fn capability_activation_maps_only_the_immutable_quote_into_general_funding() {
    let config_id = id(70);
    let manifest_id = id(71);
    let quote = native_amounts(11, 13, 17, 0, 19, 0, 23);
    let entry = capability_entry(config_id, quote);
    let mut manifest_bytes = [0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let manifest = CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes)
        .expect("canonical manifest");
    validate_general_capability_entry_v1(entry, config_id, config()).expect("recognized entry");
    let capability_funding = FundingStateV1::new(
        capability_id_from(manifest_id),
        manifest,
        0,
        funding_custody(quote, 100),
    )
    .expect("prepaid generic funding");

    let activation = GeneralFundingV1::activate_from_capability(
        [72; 32],
        config_id,
        config(),
        manifest_id,
        manifest,
        capability_funding,
        funding_custody(quote, 100),
        10,
    )
    .expect("exact General activation");
    assert_eq!(activation.rent_lamports(), 11);
    assert_eq!(activation.creation_lamports(), 13);
    assert_eq!(activation.general_lamports(), 59);
    let derivation = activation.capability_funding_derivation();
    assert_eq!(derivation.market(), [72; 32]);
    assert_eq!(derivation.generation(), 7);
    assert_eq!(derivation.entry_index(), 0);
    assert_eq!(derivation.config_id(), config_id.to_bytes());
    assert_eq!(
        derivation.release_id(),
        GENERAL_CAPABILITY_RELEASE_ID_V1.to_bytes()
    );
    assert_eq!(
        activation.capability_funding_after().status(),
        FundingStatus::Active
    );
    assert_eq!(
        activation
            .capability_funding_after()
            .remaining()
            .native_lamports_total(),
        0
    );
    assert_eq!(
        activation
            .capability_funding_after()
            .released()
            .native_lamports_total(),
        quote.native_lamports_total()
    );
    let general = activation.general_funding();
    assert_eq!(general.remaining(FundingCompartment::Liveness), Ok(23));
    assert_eq!(general.remaining(FundingCompartment::Work), Ok(17));
    assert_eq!(general.remaining(FundingCompartment::Bounty), Ok(19));
    assert_eq!(
        general.capability_release_id(),
        GENERAL_CAPABILITY_RELEASE_ID_V1
    );
}

#[test]
fn activation_plan_binds_market_frame_rent_and_every_content_preimage() {
    let config = config();
    let config_id = id(70);
    let manifest_id = id(71);
    let quote = native_amounts(11, 13, 17, 0, 19, 0, 23);
    let entry = capability_entry(config_id, quote);
    let mut manifest_bytes = [0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let manifest = CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes)
        .expect("canonical manifest");
    let funding = FundingStateV1::new(
        capability_id_from(manifest_id),
        manifest,
        0,
        funding_custody(quote, 100),
    )
    .expect("prepaid generic funding");
    let identity = MarketIdentity::new(
        core_id_from(id(80)),
        core_id_from(id(81)),
        core_id_from(config.claim_basis_id()),
        core_id_from(id(82)),
        core_id_from(manifest_id),
        config.generation(),
    );
    let mut market_root = MarketRoot::founding(identity, [90; 32]).expect("Market root");
    market_root
        .transition_phase(config.generation(), MarketPhase::Open)
        .expect("Market opens");
    let mut accounts = valid_frame_accounts(GeneralInstructionTagV1::Activate, 0);
    accounts.get_mut(15).expect("RentCredit role").key = [90; 32];
    let frame = GeneralAccountFrameV1::new(GeneralInstructionTagV1::Activate, 0, &accounts)
        .expect("activation frame");
    let instruction = ActivateGeneralV1 {
        expected_market_child_count: 0,
    };

    let plan = activate_general_v1(
        frame,
        instruction,
        market_root,
        config_id,
        config,
        manifest_id,
        manifest,
        funding,
        funding_custody(quote, 100),
        GeneralActivationCapitalizationV1::new(4, 7),
        10,
    )
    .expect("complete activation plan");
    assert_eq!(plan.market_root_after().outstanding_children(), 1);
    assert_eq!(plan.root().market(), [2; 32]);
    assert_eq!(plan.root().rent_beneficiary(), [90; 32]);
    assert_eq!(plan.creation_recipient(), [1; 32]);
    assert_eq!(plan.general_funding_account_balance(), Ok(66));
    assert_eq!(plan.root_seeds().market(), [2; 32]);
    assert_eq!(plan.funding_seeds().seed_components()[1], [2; 32]);
    assert_eq!(
        plan.funding().capability_funding_derivation().market(),
        [2; 32]
    );
    let commitments = plan.commitments();
    assert_eq!(commitments.config(), config);
    assert_eq!(commitments.config_id(), config_id);
    assert_eq!(commitments.market_identity(), identity);
    assert_eq!(
        commitments.market_identity_id(),
        config.market_identity_id()
    );
    assert_eq!(commitments.manifest(), manifest);
    assert_eq!(commitments.manifest_id(), manifest_id);

    assert_eq!(
        activate_general_v1(
            frame,
            instruction,
            market_root,
            config_id,
            config,
            manifest_id,
            manifest,
            funding,
            funding_custody(quote, 100),
            GeneralActivationCapitalizationV1::new(4, 8),
            10,
        ),
        Err(Error::CapabilityFundingMismatch)
    );

    let mut substituted_accounts = accounts.clone();
    substituted_accounts
        .get_mut(15)
        .expect("RentCredit role")
        .key = [91; 32];
    let substituted_frame =
        GeneralAccountFrameV1::new(GeneralInstructionTagV1::Activate, 0, &substituted_accounts)
            .expect("structurally valid substituted frame");
    assert_eq!(
        activate_general_v1(
            substituted_frame,
            instruction,
            market_root,
            config_id,
            config,
            manifest_id,
            manifest,
            funding,
            funding_custody(quote, 100),
            GeneralActivationCapitalizationV1::new(4, 7),
            10,
        ),
        Err(Error::AuthorityMismatch)
    );

    let founding_market = MarketRoot::founding(identity, [90; 32]).expect("founding Market");
    assert_eq!(
        activate_general_v1(
            frame,
            instruction,
            founding_market,
            config_id,
            config,
            manifest_id,
            manifest,
            funding,
            funding_custody(quote, 100),
            GeneralActivationCapitalizationV1::new(4, 7),
            10,
        ),
        Err(Error::AuthorityMismatch)
    );

    let other_accounts = valid_frame_accounts(GeneralInstructionTagV1::OpenBatch, 0);
    let wrong_action =
        GeneralAccountFrameV1::new(GeneralInstructionTagV1::OpenBatch, 0, &other_accounts)
            .expect("other valid frame");
    assert_eq!(
        activate_general_v1(
            wrong_action,
            instruction,
            market_root,
            config_id,
            config,
            manifest_id,
            manifest,
            funding,
            funding_custody(quote, 100),
            GeneralActivationCapitalizationV1::new(4, 7),
            10,
        ),
        Err(Error::InvalidInstruction)
    );
}

#[test]
fn capability_activation_rejects_extra_compartments_release_substitution_and_deadline() {
    let config_id = id(70);
    let manifest_id = id(71);
    let extra = native_amounts(1, 1, 2, 1, 3, 0, 4);
    let extra_entry = capability_entry(config_id, extra);
    let mut extra_bytes = [0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let extra_manifest =
        CapabilityManifestV1::encode_into(&[extra_entry], &mut extra_bytes).expect("manifest");
    let extra_funding = FundingStateV1::new(
        capability_id_from(manifest_id),
        extra_manifest,
        0,
        funding_custody(extra, 100),
    )
    .expect("funding");
    assert_eq!(
        GeneralFundingV1::activate_from_capability(
            [72; 32],
            config_id,
            config(),
            manifest_id,
            extra_manifest,
            extra_funding,
            funding_custody(extra, 100),
            10,
        ),
        Err(Error::ExtraneousCapabilityFunding)
    );

    let quote = native_amounts(1, 1, 2, 0, 3, 0, 4);
    let wrong_release = CapabilityEntryV1::new(
        capability_id_from(GENERAL_CAPABILITY_KIND_ID_V1),
        capability_id(88),
        capability_id_from(config_id),
        capability_id_from(config().capacity_profile_id()),
        capability_id_from(GENERAL_CHILD_SCHEMA_ID_V1),
        capability_id_from(GENERAL_CHILD_DERIVATION_ID_V1),
        ActivationPolicy::PrepaidLazy,
        100,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(quote, None).expect("native-only quote"),
    )
    .expect("entry");
    assert_eq!(
        validate_general_capability_entry_v1(wrong_release, config_id, config()),
        Err(Error::UnrecognizedCapability)
    );

    let entry = capability_entry(config_id, quote);
    let mut manifest_bytes = [0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let manifest =
        CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes).expect("manifest");
    let funding = FundingStateV1::new(
        capability_id_from(manifest_id),
        manifest,
        0,
        funding_custody(quote, 100),
    )
    .expect("funding");
    assert_eq!(
        GeneralFundingV1::activate_from_capability(
            [72; 32],
            config_id,
            config(),
            manifest_id,
            manifest,
            funding,
            funding_custody(quote, 100),
            101,
        ),
        Err(Error::CapabilityFundingMismatch)
    );
    assert_eq!(
        GeneralFundingV1::activate_from_capability(
            [72; 32],
            id(72),
            config(),
            manifest_id,
            manifest,
            funding,
            funding_custody(quote, 100),
            10,
        ),
        Err(Error::UnrecognizedCapability)
    );

    let realm_amounts = FundingAmountsV1::new(
        native(1),
        native(1),
        CompartmentFundingV1::realm_collateral(2).expect("Realm work"),
        CompartmentFundingV1::not_applicable(),
        native(3),
        CompartmentFundingV1::not_applicable(),
        native(4),
    )
    .expect("typed Realm substitution");
    let realm_binding = RealmCollateralBindingV1::new(
        capability_id(91),
        capability_id(92),
        [93; 32],
        [94; 32],
        [95; 32],
    )
    .expect("Realm binding");
    let realm_quote = FundingQuoteV1::new(realm_amounts, Some(realm_binding)).expect("Realm quote");
    assert_eq!(
        validate_general_funding_quote_v1(realm_quote),
        Err(Error::ExtraneousCapabilityFunding)
    );
    assert_eq!(
        validate_general_funding_quote_v1(
            FundingQuoteV1::new(native_amounts(1, 1, 0, 0, 3, 0, 4), None)
                .expect("missing-work quote")
        ),
        Err(Error::ExtraneousCapabilityFunding)
    );
}

#[test]
fn open_batch_consumes_exact_liveness_rent_and_rejects_physical_substitution() {
    let config = config();
    let config_id = id(70);
    let mut accounts = valid_frame_accounts(GeneralInstructionTagV1::OpenBatch, 0);
    let market = accounts.get(1).expect("Market").key;
    let root_key = accounts.get(3).expect("root").key;
    let rent_credit = accounts.get(6).expect("RentCredit").key;
    let root = GeneralRootV1::founding(market, config_id, config.generation(), rent_credit)
        .expect("General root");
    let funding = GeneralFundingV1::founding(GENERAL_CAPABILITY_RELEASE_ID_V1, 10, 20, 30);
    let frame = GeneralAccountFrameV1::new(GeneralInstructionTagV1::OpenBatch, 0, &accounts)
        .expect("open frame");
    let instruction = GeneralBatchReplayV1 {
        generation: config.generation(),
        batch_sequence: 0,
    };
    let custody = GeneralFundingCustodyObservationV1::new(160, 100).expect("custody");
    let plan = open_general_batch_v1(
        frame,
        instruction,
        config_id,
        config,
        root,
        funding,
        custody,
        4,
        5,
    )
    .expect("batch opens");
    assert_eq!(plan.root_after().next_batch_sequence(), 1);
    assert_eq!(plan.root_after().open_batches(), 1);
    assert_eq!(
        plan.funding_after().remaining(FundingCompartment::Liveness),
        Ok(6)
    );
    assert_eq!(plan.batch().sequence(), 0);
    assert_eq!(plan.batch().collection_close(), 15);
    assert_eq!(plan.batch_rent_lamports(), 4);
    assert_eq!(plan.funding_account_lamports_after(), 156);
    assert_eq!(plan.batch_seeds().seed_components()[1], root_key);
    round_trip_general_root(plan.root_after());
    round_trip_funding(plan.funding_after());
    round_trip_batch_root(plan.batch());

    assert_eq!(
        open_general_batch_v1(
            frame,
            instruction,
            config_id,
            config,
            root,
            funding,
            GeneralFundingCustodyObservationV1::new(161, 100).expect("donated custody"),
            4,
            5,
        ),
        Err(Error::GeneralFundingCustodyMismatch)
    );
    assert_eq!(
        open_general_batch_v1(
            frame,
            instruction,
            config_id,
            config,
            root,
            funding,
            custody,
            11,
            5,
        ),
        Err(Error::InsufficientFunding)
    );
    accounts.get_mut(6).expect("RentCredit").key = [99; 32];
    let substituted = GeneralAccountFrameV1::new(GeneralInstructionTagV1::OpenBatch, 0, &accounts)
        .expect("substituted structural frame");
    assert_eq!(
        open_general_batch_v1(
            substituted,
            instruction,
            config_id,
            config,
            root,
            funding,
            custody,
            4,
            5,
        ),
        Err(Error::AuthorityMismatch)
    );
}

#[test]
fn order_reserve_uses_one_checked_ceiling_and_exact_negative_coefficients() {
    let order = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(10),
        generation: 7,
        batch_sequence: 0,
        nonce: 9,
        valid_until_slot: 30,
        max_lots: 3,
        max_quote_debit_per_lot_numerator: 101,
        coefficients: [-2, 3],
        outcome_count: 2,
    })
    .expect("order");
    assert_eq!(order.max_lots(), 3);
    assert_eq!(order.max_quote_debit_per_lot_numerator(), 101);
    let reserve = order.worst_case_reserve(config()).expect("reserve");
    assert_eq!(reserve.quote_atoms(), 4);
    assert_eq!(reserve.claim_atoms(), &[6, 0]);

    let zero_quote = PortfolioOrderV1::new(PortfolioOrderV1Input {
        max_quote_debit_per_lot_numerator: -1,
        ..PortfolioOrderV1Input {
            market_identity_id: id(2),
            claim_basis_id: id(3),
            owner: owner(20),
            order_id: id(11),
            generation: 7,
            batch_sequence: 0,
            nonce: 10,
            valid_until_slot: 30,
            max_lots: 3,
            max_quote_debit_per_lot_numerator: 0,
            coefficients: [-2, 3],
            outcome_count: 2,
        }
    })
    .expect("order");
    assert_eq!(
        zero_quote
            .worst_case_reserve(config())
            .expect("reserve")
            .quote_atoms(),
        0
    );

    let claim_overflow = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(12),
        generation: 7,
        batch_sequence: 0,
        nonce: 11,
        valid_until_slot: 30,
        max_lots: 2,
        max_quote_debit_per_lot_numerator: 0,
        coefficients: [i64::MIN, 1],
        outcome_count: 2,
    })
    .expect("order");
    assert_eq!(
        claim_overflow.worst_case_reserve(config()),
        Err(Error::TokenAmountOutOfRange)
    );

    let quote_overflow = PortfolioOrderV1::new(PortfolioOrderV1Input {
        max_lots: u64::MAX,
        max_quote_debit_per_lot_numerator: i128::MAX,
        coefficients: [-1, 1],
        order_id: id(13),
        nonce: 12,
        ..PortfolioOrderV1Input {
            market_identity_id: id(2),
            claim_basis_id: id(3),
            owner: owner(20),
            order_id: id(13),
            generation: 7,
            batch_sequence: 0,
            nonce: 12,
            valid_until_slot: 30,
            max_lots: 1,
            max_quote_debit_per_lot_numerator: 0,
            coefficients: [-1, 1],
            outcome_count: 2,
        }
    })
    .expect("order");
    assert_eq!(
        quote_overflow.worst_case_reserve(config()),
        Err(Error::ArithmeticOverflow)
    );
}

#[test]
fn exact_n_order_custody_round_trips_and_rejects_hostile_geometry() {
    assert_eq!(GeneralOrderCustodyV1::<2>::encoded_len(), Ok(208));
    assert_eq!(GeneralOrderCustodyV1::<16>::encoded_len(), Ok(320));
    let order = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(10),
        generation: 7,
        batch_sequence: 0,
        nonce: 9,
        valid_until_slot: 30,
        max_lots: 3,
        max_quote_debit_per_lot_numerator: 101,
        coefficients: [-2, 3],
        outcome_count: 2,
    })
    .expect("order");
    let admission =
        GeneralOrderCustodyV1::admit(order, config(), [70; 32], [71; 32]).expect("admission");
    let mut bytes = [0; 208];
    admission
        .custody
        .encode(&mut bytes)
        .expect("custody encodes");
    assert_eq!(&bytes[..8], b"DCLTGOC1");
    assert_eq!(
        GeneralOrderCustodyV1::<2>::decode(&bytes),
        Ok(admission.custody)
    );
    assert_eq!(
        GeneralOrderCustodyV1::<16>::decode(&bytes),
        Err(Error::InvalidLength)
    );
    assert_eq!(
        GeneralOrderCustodyV1::<2>::decode(&bytes[..207]),
        Err(Error::InvalidLength)
    );
    let mut trailing = [0; 209];
    trailing[..208].copy_from_slice(&bytes);
    assert_eq!(
        GeneralOrderCustodyV1::<2>::decode(&trailing),
        Err(Error::InvalidLength)
    );
    let mut changed = bytes;
    changed[14] = 1;
    assert_eq!(
        GeneralOrderCustodyV1::<2>::decode(&changed),
        Err(Error::NonCanonicalReservedBytes)
    );
    let mut changed = bytes;
    changed[80..112].fill(0);
    assert_eq!(
        GeneralOrderCustodyV1::<2>::decode(&changed),
        Err(Error::ZeroIdentifier)
    );
    let mut changed = bytes;
    changed[12..14].copy_from_slice(&16_u16.to_le_bytes());
    assert_eq!(
        GeneralOrderCustodyV1::<2>::decode(&changed),
        Err(Error::InvalidOutcomeCount)
    );
}

#[test]
fn custody_admission_receipt_cancel_and_post_batch_close_are_atomic() {
    let order = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(10),
        generation: 7,
        batch_sequence: 0,
        nonce: 9,
        valid_until_slot: 30,
        max_lots: 3,
        max_quote_debit_per_lot_numerator: 101,
        coefficients: [-2, 3],
        outcome_count: 2,
    })
    .expect("order");
    let admission =
        GeneralOrderCustodyV1::admit(order, config(), [70; 32], [71; 32]).expect("admission");
    assert_eq!(admission.reserve.quote_atoms(), 4);
    assert_eq!(admission.reserve.claim_atoms(), &[6, 0]);
    assert_eq!(admission.order_state.phase(), OrderPhase::Open);

    let mut state = admission.order_state;
    let mut custody = admission.custody;
    let receipt = SettlementReceiptV1 {
        candidate_id: id(40),
        order_id: order.order_id(),
        owner: order.owner(),
        generation: order.generation(),
        batch_sequence: order.batch_sequence(),
        nonce: order.nonce(),
        fill_lots: 1,
        remaining_lots: 2,
        quote_delta_atoms: -1,
        carry_before: 0,
        carry_after: 0,
        outcome_deltas: [-2, 3],
        outcome_count: 2,
    };
    let effects = custody
        .apply_receipt(&mut state, order, receipt, config())
        .expect("receipt applies");
    assert_eq!(effects.quote_debit_from_escrow(), 1);
    assert_eq!(effects.quote_credit_to_owner(), 0);
    assert_eq!(effects.claim_debits_from_custody(), &[2, 0]);
    assert_eq!(effects.claim_credits_to_owner(), &[0, 3]);
    assert_eq!(state.remaining_lots(), 2);
    assert_eq!(custody.reserved_quote_atoms(), 3);
    assert_eq!(custody.reserved_claim_atoms(), &[4, 0]);

    let mut batch = BatchRootV1::open(id(9), 0, 0, config()).expect("batch");
    batch.open_selection(10).expect("selecting");
    batch.close_selection(20).expect("quiescent");
    let release = custody
        .close_after_batch(&mut state, order, batch, config())
        .expect("partial remainder closes");
    assert_eq!(state.phase(), OrderPhase::Released);
    assert_eq!(release.quote_atoms, 3);
    assert_eq!(release.claim_atoms, [4, 0]);
    assert_eq!(release.owner, owner(20));
    assert_eq!(release.rent_beneficiary, [70; 32]);
    assert_eq!(release.quote_escrow, [71; 32]);

    let cancelled_admission =
        GeneralOrderCustodyV1::admit(order, config(), [70; 32], [71; 32]).expect("admission");
    let mut cancelled_state = cancelled_admission.order_state;
    let before = cancelled_state;
    assert_eq!(
        cancelled_admission.custody.cancel_and_release(
            &mut cancelled_state,
            order,
            owner(21),
            9,
            10,
            config(),
        ),
        Err(Error::AuthorityMismatch)
    );
    assert_eq!(cancelled_state, before);
    let cancel_release = cancelled_admission
        .custody
        .cancel_and_release(&mut cancelled_state, order, owner(20), 9, 10, config())
        .expect("cancel releases");
    assert_eq!(cancelled_state.phase(), OrderPhase::Cancelled);
    assert_eq!(cancel_release.quote_atoms, 4);
    assert_eq!(cancel_release.claim_atoms, [6, 0]);

    let consumed_order = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(11),
        generation: 7,
        batch_sequence: 0,
        nonce: 10,
        valid_until_slot: 30,
        max_lots: 1,
        max_quote_debit_per_lot_numerator: 101,
        coefficients: [-2, 3],
        outcome_count: 2,
    })
    .expect("one-lot order");
    let consumed_admission =
        GeneralOrderCustodyV1::admit(consumed_order, config(), [70; 32], [71; 32])
            .expect("admission");
    let mut consumed_state = consumed_admission.order_state;
    let mut consumed_custody = consumed_admission.custody;
    let consumed_receipt = SettlementReceiptV1 {
        candidate_id: id(40),
        order_id: consumed_order.order_id(),
        owner: consumed_order.owner(),
        generation: consumed_order.generation(),
        batch_sequence: consumed_order.batch_sequence(),
        nonce: consumed_order.nonce(),
        fill_lots: 1,
        remaining_lots: 0,
        quote_delta_atoms: -1,
        carry_before: 0,
        carry_after: 0,
        outcome_deltas: [-2, 3],
        outcome_count: 2,
    };
    consumed_custody
        .apply_receipt(
            &mut consumed_state,
            consumed_order,
            consumed_receipt,
            config(),
        )
        .expect("full receipt applies");
    assert_eq!(consumed_state.phase(), OrderPhase::Consumed);
    let consumed_release = consumed_custody
        .close_after_batch(&mut consumed_state, consumed_order, batch, config())
        .expect("fully consumed custody closes");
    assert_eq!(consumed_state.phase(), OrderPhase::Consumed);
    assert_eq!(consumed_release.quote_atoms, 1);
    assert_eq!(consumed_release.claim_atoms, [0, 0]);
}

#[test]
fn hostile_receipt_and_underfunded_custody_leave_replay_and_reserves_unchanged() {
    let order = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(10),
        generation: 7,
        batch_sequence: 0,
        nonce: 9,
        valid_until_slot: 30,
        max_lots: 3,
        max_quote_debit_per_lot_numerator: 101,
        coefficients: [-2, 3],
        outcome_count: 2,
    })
    .expect("order");
    let admission =
        GeneralOrderCustodyV1::admit(order, config(), [70; 32], [71; 32]).expect("admission");
    let receipt = SettlementReceiptV1 {
        candidate_id: id(40),
        order_id: order.order_id(),
        owner: order.owner(),
        generation: order.generation(),
        batch_sequence: order.batch_sequence(),
        nonce: order.nonce(),
        fill_lots: 1,
        remaining_lots: 2,
        quote_delta_atoms: -5,
        carry_before: 0,
        carry_after: 0,
        outcome_deltas: [-2, 3],
        outcome_count: 2,
    };
    let mut state = admission.order_state;
    let mut custody = admission.custody;
    let before_state = state;
    let before_custody = custody;
    assert_eq!(
        custody.apply_receipt(&mut state, order, receipt, config()),
        Err(Error::InsufficientCustody)
    );
    assert_eq!(state, before_state);
    assert_eq!(custody, before_custody);

    let mut bytes = [0; 208];
    admission
        .custody
        .encode(&mut bytes)
        .expect("custody encodes");
    bytes[192..200].copy_from_slice(&1_u64.to_le_bytes());
    let mut underfunded = GeneralOrderCustodyV1::<2>::decode(&bytes).expect("shaped custody");
    let mut state = admission.order_state;
    let claim_receipt = SettlementReceiptV1 {
        quote_delta_atoms: -1,
        ..receipt
    };
    let before_state = state;
    let before_custody = underfunded;
    assert_eq!(
        underfunded.apply_receipt(&mut state, order, claim_receipt, config()),
        Err(Error::InsufficientCustody)
    );
    assert_eq!(state, before_state);
    assert_eq!(underfunded, before_custody);

    let mut substituted = receipt;
    substituted.owner = owner(21);
    let mut state = admission.order_state;
    let mut custody = admission.custody;
    assert_eq!(
        custody.apply_receipt(&mut state, order, substituted, config()),
        Err(Error::CustodyMismatch)
    );
    assert_eq!(state, admission.order_state);
    assert_eq!(custody, admission.custody);

    let mut wrong_width = receipt;
    wrong_width.outcome_count = 16;
    let mut state = admission.order_state;
    let mut custody = admission.custody;
    assert_eq!(
        custody.apply_receipt(&mut state, order, wrong_width, config()),
        Err(Error::CustodyMismatch)
    );
    assert_eq!(state, admission.order_state);
    assert_eq!(custody, admission.custody);
}

#[test]
fn derivation_seed_tuples_are_ordered_closed_and_substitution_resistant() {
    let config_id = id(70);
    let root = GeneralRootPdaSeedsV1::new([8; 32], 7, config_id).expect("root seeds");
    let generation = 7_u64.to_le_bytes();
    assert_eq!(
        root.seed_components(),
        [
            GENERAL_ROOT_PDA_DOMAIN_V1,
            &[8; 32],
            generation.as_slice(),
            config_id.as_bytes(),
        ]
    );
    assert_eq!(
        GeneralRootPdaSeedsV1::new([0; 32], 7, config_id),
        Err(Error::ZeroIdentifier)
    );

    let funding =
        GeneralFundingPdaSeedsV1::new([8; 32], 7, config_id, GENERAL_CAPABILITY_RELEASE_ID_V1)
            .expect("funding seeds");
    assert_eq!(funding.seed_components()[0], GENERAL_FUNDING_PDA_DOMAIN_V1);
    assert_eq!(funding.seed_components()[2], generation.as_slice());
    assert_eq!(
        GeneralFundingPdaSeedsV1::new([8; 32], 7, config_id, id(99)),
        Err(Error::UnrecognizedCapability)
    );

    let sequence = 3_u64.to_le_bytes();
    let batch = GeneralBatchPdaSeedsV1::new([9; 32], 3).expect("batch seeds");
    assert_eq!(
        batch.seed_components(),
        [GENERAL_BATCH_PDA_DOMAIN_V1, &[9; 32], sequence.as_slice(),]
    );
    let signed = order(10, 20, 9, [-1, 1], 10);
    let replay = GeneralOrderStatePdaSeedsV1::new([8; 32], signed).expect("replay seeds");
    assert_eq!(
        replay.seed_components()[0],
        GENERAL_ORDER_STATE_PDA_DOMAIN_V1
    );
    assert_eq!(replay.seed_components()[3], signed.owner().as_bytes());
    assert_eq!(replay.seed_components()[5], signed.order_id().as_bytes());
    assert_ne!(
        replay,
        GeneralOrderStatePdaSeedsV1::new([8; 32], order(11, 20, 9, [-1, 1], 10))
            .expect("substituted replay")
    );

    let custody = GeneralOrderCustodyPdaSeedsV1::new([11; 32]).expect("custody seeds");
    let escrow = GeneralQuoteEscrowPdaSeedsV1::new([12; 32]).expect("escrow seeds");
    let candidate = GeneralCandidatePdaSeedsV1::new([13; 32], id(14)).expect("candidate seeds");
    let settlement = GeneralSettlementCursorPdaSeedsV1::new([15; 32]).expect("settlement seeds");
    assert_eq!(
        custody.seed_components()[0],
        GENERAL_ORDER_CUSTODY_PDA_DOMAIN_V1
    );
    assert_eq!(
        escrow.seed_components()[0],
        GENERAL_QUOTE_ESCROW_PDA_DOMAIN_V1
    );
    assert_eq!(
        candidate.seed_components()[0],
        GENERAL_CANDIDATE_PDA_DOMAIN_V1
    );
    assert_eq!(
        settlement.seed_components()[0],
        GENERAL_SETTLEMENT_CURSOR_PDA_DOMAIN_V1
    );
}

#[test]
fn candidate_and_page_codecs_are_exact_n_and_hostile() {
    let submission = CandidateSubmissionV1 {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        submitter: owner(30),
        initial_transcript_id: id(50),
        generation: 7,
        batch_sequence: 0,
        valid_until_slot: 20,
        claimed_execution_count: 2,
        claimed_score: 20,
        prices: [40, 60],
        outcome_count: 2,
    };
    assert_eq!(CandidateSubmissionV1::<2>::encoded_len(), Ok(208));
    assert_eq!(CandidateSubmissionV1::<16>::encoded_len(), Ok(320));
    let mut submission_bytes = [0; 208];
    submission
        .encode(&mut submission_bytes)
        .expect("submission encodes");
    assert_eq!(
        CandidateSubmissionV1::<2>::decode(&submission_bytes),
        Ok(submission)
    );
    assert_eq!(
        CandidateSubmissionV1::<16>::decode(&submission_bytes),
        Err(Error::InvalidLength)
    );
    let mut dirty = submission_bytes;
    dirty[172] = 1;
    assert_eq!(
        CandidateSubmissionV1::<2>::decode(&dirty),
        Err(Error::NonCanonicalReservedBytes)
    );
    let mut zero_submitter = submission_bytes;
    zero_submitter[80..112].fill(0);
    assert_eq!(
        CandidateSubmissionV1::<2>::decode(&zero_submitter),
        Err(Error::ZeroIdentifier)
    );

    let page = VerificationPageV1 {
        page_index: 0,
        prior_transcript_id: id(50),
        next_transcript_id: id(51),
        execution_count: 2,
        executions: executions(),
    };
    assert_eq!(VerificationPageV1::<2>::encoded_len(2), Ok(728));
    assert_eq!(VerificationPageV1::<16>::encoded_len(2), Ok(952));
    let mut page_bytes = vec![0; VerificationPageV1::<2>::encoded_len(2).expect("page width")];
    page.encode(&mut page_bytes).expect("page encodes");
    assert_eq!(VerificationPageV1::<2>::decode(&page_bytes), Ok(page));
    assert_eq!(
        VerificationPageV1::<16>::decode(&page_bytes),
        Err(Error::InvalidOutcomeCount)
    );
    let mut trailing = page_bytes.clone();
    trailing.push(0);
    assert_eq!(
        VerificationPageV1::<2>::decode(&trailing),
        Err(Error::InvalidLength)
    );
    let mut dirty = page_bytes;
    *dirty.get_mut(15).expect("reserved byte") = 1;
    assert_eq!(
        VerificationPageV1::<2>::decode(&dirty),
        Err(Error::NonCanonicalReservedBytes)
    );
}

#[test]
fn candidate_and_settlement_state_round_trip_every_reachable_phase() {
    assert_eq!(CandidateStateV1::<2>::encoded_len(), Ok(424));
    assert_eq!(CandidateStateV1::<16>::encoded_len(), Ok(760));
    assert_eq!(SettlementCursorV1::<2>::encoded_len(), Ok(200));
    assert_eq!(SettlementCursorV1::<16>::encoded_len(), Ok(424));
    let batch = selecting_batch();
    let submission = CandidateSubmissionV1 {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        submitter: owner(30),
        initial_transcript_id: id(50),
        generation: 7,
        batch_sequence: 0,
        valid_until_slot: 20,
        claimed_execution_count: 2,
        claimed_score: 20,
        prices: [40, 60],
        outcome_count: 2,
    };
    let mut candidate =
        CandidateStateV1::submit(id(40), submission, config(), batch, 10).expect("submission");
    round_trip_candidate(candidate);
    let mut rejected = candidate;
    rejected.reject().expect("pristine reject");
    round_trip_candidate(rejected);
    let page = VerificationPageV1 {
        page_index: 0,
        prior_transcript_id: id(50),
        next_transcript_id: id(51),
        execution_count: 2,
        executions: executions(),
    };
    candidate
        .verify_page(page, config(), batch, 11)
        .expect("verification page");
    round_trip_candidate(candidate);
    let mut rejected = candidate;
    rejected.reject().expect("partial reject");
    round_trip_candidate(rejected);
    candidate.finish_verification(config()).expect("valid");
    round_trip_candidate(candidate);

    let mut selected_batch = batch;
    selected_batch
        .consider_candidate(&mut candidate, 12)
        .expect("considered");
    round_trip_candidate(candidate);
    selected_batch
        .close_selection(20)
        .expect("selection locked");
    let mut hoard = HoardLedgerV1::new(id(2), 0, 0).expect("hoard");
    let mut cursor =
        SettlementCursorV1::begin(candidate, &mut selected_batch, &mut hoard, config(), 20)
            .expect("settlement");
    round_trip_settlement_cursor(cursor);
    cursor
        .settle_page(page, candidate, config(), selected_batch)
        .expect("settled page");
    round_trip_settlement_cursor(cursor);

    let mut candidate_bytes =
        vec![0; CandidateStateV1::<2>::encoded_len().expect("candidate width")];
    candidate
        .encode(&mut candidate_bytes)
        .expect("candidate bytes");
    let submission_bytes = CandidateSubmissionV1::<2>::encoded_len().expect("submission width");
    *candidate_bytes
        .get_mut(48 + submission_bytes)
        .expect("candidate phase") = u8::MAX;
    assert_eq!(
        CandidateStateV1::<2>::decode(&candidate_bytes),
        Err(Error::InvalidPhase)
    );
    let mut cursor_bytes = vec![0; SettlementCursorV1::<2>::encoded_len().expect("cursor width")];
    cursor.encode(&mut cursor_bytes).expect("cursor bytes");
    *cursor_bytes.get_mut(88).expect("last-order tag") = 0;
    assert_eq!(
        SettlementCursorV1::<2>::decode(&cursor_bytes),
        Err(Error::NonCanonicalReservedBytes)
    );
}

#[test]
fn general_instruction_family_round_trips_and_refuses_width_or_tag_substitution() {
    let replay = GeneralBatchReplayV1 {
        generation: 7,
        batch_sequence: 0,
    };
    let signed = order(10, 20, 9, [-1, 1], 10);
    let submission = CandidateSubmissionV1 {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        submitter: owner(30),
        initial_transcript_id: id(50),
        generation: 7,
        batch_sequence: 0,
        valid_until_slot: 20,
        claimed_execution_count: 2,
        claimed_score: 20,
        prices: [40, 60],
        outcome_count: 2,
    };
    let page = VerificationPageV1 {
        page_index: 0,
        prior_transcript_id: id(50),
        next_transcript_id: id(51),
        execution_count: 2,
        executions: executions(),
    };
    let candidate_page = GeneralCandidatePageV1 {
        candidate_id: id(40),
        page,
    };
    let instructions = [
        GeneralInstructionV1::Activate(ActivateGeneralV1 {
            expected_market_child_count: 1,
        }),
        GeneralInstructionV1::OpenBatch(replay),
        GeneralInstructionV1::LockBatch(replay),
        GeneralInstructionV1::AdmitOrder(signed),
        GeneralInstructionV1::CancelOrder(signed),
        GeneralInstructionV1::CloseOrder(signed),
        GeneralInstructionV1::SubmitCandidate(SubmitGeneralCandidateV1 {
            candidate_id: id(40),
            submission,
        }),
        GeneralInstructionV1::VerifyCandidatePage(candidate_page),
        GeneralInstructionV1::FinishCandidate(id(40)),
        GeneralInstructionV1::ConsiderCandidate(id(40)),
        GeneralInstructionV1::LockSelection(replay),
        GeneralInstructionV1::BeginSettlement(id(40)),
        GeneralInstructionV1::SettlePage(candidate_page),
        GeneralInstructionV1::FinishSettlement(id(40)),
        GeneralInstructionV1::CloseBatch(replay),
        GeneralInstructionV1::Quiesce(7),
        GeneralInstructionV1::CloseGeneral(7),
        GeneralInstructionV1::CloseCandidate(id(40)),
        GeneralInstructionV1::CloseSettlement(id(40)),
    ];
    for instruction in instructions {
        let mut bytes = vec![0; instruction.encoded_len().expect("instruction width")];
        instruction.encode(&mut bytes).expect("instruction encodes");
        assert_eq!(GeneralInstructionV1::<2>::decode(&bytes), Ok(instruction));
    }

    let instruction = GeneralInstructionV1::AdmitOrder(signed);
    let mut bytes = vec![0; instruction.encoded_len().expect("instruction width")];
    instruction.encode(&mut bytes).expect("instruction encodes");
    assert_eq!(
        GeneralInstructionV1::<16>::decode(&bytes),
        Err(Error::InvalidOutcomeCount)
    );
    *bytes.get_mut(10).expect("instruction tag") = u8::MAX;
    assert_eq!(
        GeneralInstructionV1::<2>::decode(&bytes),
        Err(Error::UnknownAction)
    );
    *bytes.get_mut(10).expect("instruction tag") = GeneralInstructionTagV1::AdmitOrder as u8;
    *bytes.get_mut(12).expect("reserved byte") = 1;
    assert_eq!(
        GeneralInstructionV1::<2>::decode(&bytes),
        Err(Error::NonCanonicalReservedBytes)
    );
    *bytes.get_mut(12).expect("reserved byte") = 0;
    bytes.push(0);
    assert_eq!(
        GeneralInstructionV1::<2>::decode(&bytes),
        Err(Error::InvalidLength)
    );
}

#[test]
fn ordered_general_frames_reject_privilege_alias_count_and_page_substitution() {
    let tags = [
        (GeneralInstructionTagV1::Activate, 0),
        (GeneralInstructionTagV1::OpenBatch, 0),
        (GeneralInstructionTagV1::LockBatch, 0),
        (GeneralInstructionTagV1::AdmitOrder, 0),
        (GeneralInstructionTagV1::CancelOrder, 0),
        (GeneralInstructionTagV1::CloseOrder, 0),
        (GeneralInstructionTagV1::SubmitCandidate, 0),
        (GeneralInstructionTagV1::VerifyCandidatePage, 4),
        (GeneralInstructionTagV1::FinishCandidate, 0),
        (GeneralInstructionTagV1::ConsiderCandidate, 0),
        (GeneralInstructionTagV1::LockSelection, 0),
        (GeneralInstructionTagV1::BeginSettlement, 0),
        (GeneralInstructionTagV1::SettlePage, 4),
        (GeneralInstructionTagV1::FinishSettlement, 0),
        (GeneralInstructionTagV1::CloseBatch, 0),
        (GeneralInstructionTagV1::Quiesce, 0),
        (GeneralInstructionTagV1::CloseGeneral, 0),
        (GeneralInstructionTagV1::CloseCandidate, 0),
        (GeneralInstructionTagV1::CloseSettlement, 0),
    ];
    for (tag, count) in tags {
        let accounts = valid_frame_accounts(tag, count);
        let frame = GeneralAccountFrameV1::new(tag, count, &accounts).expect("valid frame");
        assert_eq!(frame.account_count(), accounts.len());
        assert_eq!(frame.role(0), general_frame_role(tag, count, 0));
    }
    let activation = valid_frame_accounts(GeneralInstructionTagV1::Activate, 0);
    let activation_frame =
        GeneralAccountFrameV1::new(GeneralInstructionTagV1::Activate, 0, &activation)
            .expect("activation frame");
    assert_eq!(activation.len(), 19);
    assert_eq!(activation_frame.role(2), Ok(GeneralAccountRoleV1::Realm));
    assert_eq!(
        activation_frame.role(3),
        Ok(GeneralAccountRoleV1::ClaimBasis)
    );
    assert_eq!(activation_frame.role(4), Ok(GeneralAccountRoleV1::Manifest));
    assert_eq!(
        activation_frame.role(5),
        Ok(GeneralAccountRoleV1::GeneralConfig)
    );
    for index in 6..=9 {
        assert_eq!(
            activation_frame.role(index),
            Ok(GeneralAccountRoleV1::StagingCursorVacancy)
        );
    }
    assert_eq!(
        valid_frame_accounts(GeneralInstructionTagV1::AdmitOrder, 0).len(),
        19
    );
    assert_eq!(
        valid_frame_accounts(GeneralInstructionTagV1::CancelOrder, 0).len(),
        17
    );
    assert_eq!(
        valid_frame_accounts(GeneralInstructionTagV1::CloseOrder, 0).len(),
        15
    );
    assert_eq!(
        valid_frame_accounts(GeneralInstructionTagV1::VerifyCandidatePage, 4).len(),
        10
    );
    let settlement = valid_frame_accounts(GeneralInstructionTagV1::SettlePage, 4);
    assert_eq!(settlement.len(), 30);
    let close_general = valid_frame_accounts(GeneralInstructionTagV1::CloseGeneral, 0);
    let close_general_frame =
        GeneralAccountFrameV1::new(GeneralInstructionTagV1::CloseGeneral, 0, &close_general)
            .expect("terminal frame");
    assert_eq!(close_general.len(), 7);
    assert_eq!(
        close_general_frame.role(1),
        Ok(GeneralAccountRoleV1::GeneralConfig)
    );
    assert_eq!(
        close_general_frame.role(5),
        Ok(GeneralAccountRoleV1::StagingCursorVacancy)
    );
    assert_eq!(
        close_general_frame.role(6),
        Ok(GeneralAccountRoleV1::RentSysvar)
    );

    let mut wrong_privilege = valid_frame_accounts(GeneralInstructionTagV1::Activate, 0);
    wrong_privilege.get_mut(0).expect("activator").is_signer = false;
    assert_eq!(
        GeneralAccountFrameV1::new(GeneralInstructionTagV1::Activate, 0, &wrong_privilege),
        Err(Error::InvalidAccountPrivilege)
    );
    let mut alias = valid_frame_accounts(GeneralInstructionTagV1::Activate, 0);
    let realm_key = alias.get(2).expect("realm").key;
    alias.get_mut(6).expect("realm staging vacancy").key = realm_key;
    assert_eq!(
        GeneralAccountFrameV1::new(GeneralInstructionTagV1::Activate, 0, &alias),
        Err(Error::AccountAlias)
    );
    let mut short = valid_frame_accounts(GeneralInstructionTagV1::AdmitOrder, 0);
    short.pop();
    assert_eq!(
        GeneralAccountFrameV1::new(GeneralInstructionTagV1::AdmitOrder, 0, &short),
        Err(Error::InvalidLength)
    );
    assert_eq!(
        GeneralAccountFrameV1::new(GeneralInstructionTagV1::VerifyCandidatePage, 0, &[],),
        Err(Error::InvalidPageCount)
    );
    assert_eq!(
        GeneralAccountFrameV1::new(GeneralInstructionTagV1::SettlePage, 5, &settlement,),
        Err(Error::InvalidPageCount)
    );
}
