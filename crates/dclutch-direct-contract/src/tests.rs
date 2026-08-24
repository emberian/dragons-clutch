#![allow(clippy::indexing_slicing)]

use dclutch_realm_contract::PositionV1;

use super::*;
use crate::adapter::{
    AdapterActionV2, ED25519_PROGRAM_ID_3_0, Ed25519ExpectationV2, Ed25519InstructionViewV2,
    MEASURED_LOOKUP_TABLES_V2, MEASURED_TRANSACTION_SIGNATURES_V2, PacketAdmissionV2,
    SOLANA_PACKET_DATA_SIZE_3_0, admit_settlement_packet_v2, canonical_ed25519_test_instruction,
    canonical_ed25519_test_instruction_len, decode_inline_complementary_instruction_v2,
    decode_inline_ordinary_instruction_v2, decode_ordinary_instruction_v2,
    encode_cancel_instruction_v2, encode_inline_complementary_instruction_v2,
    encode_inline_ordinary_instruction_v2, encode_ordinary_instruction_v2,
    encode_register_instruction_v2, inline_complementary_instruction_bytes_v2,
    inspect_preceding_ed25519_batch_v2, inspect_preceding_ed25519_v2,
    measured_inline_complementary_reference_v2, measured_settlement_envelope_v2,
    stateless_shared_message_ed25519_minimum_v2,
};

fn key(value: u8) -> [u8; 32] {
    [value; 32]
}

fn intent(
    maker: u8,
    side: Side,
    outcome: u8,
    nonce: u64,
    limit: u64,
    max_fill: u64,
    bps: u16,
) -> Result<DirectIntentV2> {
    intent_lifecycle(
        IntentLifecycleV2::Registered,
        (maker, side, outcome, nonce, limit, max_fill, bps),
    )
}

fn intent_lifecycle(
    lifecycle: IntentLifecycleV2,
    values: (u8, Side, u8, u64, u64, u64, u16),
) -> Result<DirectIntentV2> {
    let (maker, side, outcome, nonce, limit, max_fill, bps) = values;
    DirectIntentV2::new(DirectIntentInputV2 {
        market: key(7),
        generation: 3,
        maker: key(maker),
        nonce,
        valid_from_slot: 10,
        valid_through_slot: 20,
        side,
        lifecycle,
        outcome,
        max_fill,
        limit_price: limit,
        fee_config: key(8),
        fee_basis_points: bps,
        position_account: key(maker + 40),
        collateral_account: key(maker + 80),
    })
}

fn inline_accounts(value: DirectIntentV2) -> InlineParticipantAccountsV2 {
    let maker = value.maker()[0];
    InlineParticipantAccountsV2 {
        replay_root: key(maker + 100),
        position: *value.position_account(),
        collateral: *value.collateral_account(),
    }
}

fn root(maker: u8) -> Result<MakerReplayRootV2> {
    MakerReplayRootV2::new(key(7), 3, key(maker), key(maker + 20), maker)
}

fn accounts(value: DirectIntentV2) -> ParticipantAccountsV2 {
    let maker = value.maker()[0];
    ParticipantAccountsV2 {
        replay_root: key(maker + 100),
        record: key(maker + 120),
        position: *value.position_account(),
        collateral: *value.collateral_account(),
        escrow: match value.side() {
            Side::Buy => key(maker + 140),
            Side::Sell => [0; 32],
        },
    }
}

fn position(owner: u8, balances: [u64; 2]) -> Result<PositionV1<2>> {
    PositionV1::new(key(7), key(owner), 3, balances).map_err(position_error)
}

fn policy(bps: u16) -> Result<VenueFeePolicyV2> {
    VenueFeePolicyV2::new(key(7), 3, key(8), key(99), bps)
}

fn authorization(
    signer: [u8; 32],
    message: &[u8],
    current_data: &[u8],
    message_offset: u16,
) -> Result<adapter::Ed25519AuthorizationV2> {
    let data = canonical_ed25519_test_instruction(
        [signer],
        [message_offset],
        [u16::try_from(message.len()).map_err(|_| Error::InvalidSignatureInstruction)?],
        5,
    );
    inspect_preceding_ed25519_v2(
        Ed25519InstructionViewV2 {
            program_id: ED25519_PROGRAM_ID_3_0,
            ed25519_data: data
                .get(..canonical_ed25519_test_instruction_len(1))
                .ok_or(Error::InvalidSignatureInstruction)?,
            preceding_index: 4,
            current_index: 5,
            current_data,
        },
        Ed25519ExpectationV2 {
            message_offset,
            signer,
            message,
        },
    )
}

fn inspect_test<'a>(
    program_id: [u8; 32],
    ed25519_data: &'a [u8],
    preceding_index: u16,
    current_data: &'a [u8],
    signer: [u8; 32],
    message: &'a [u8],
) -> Result<adapter::Ed25519AuthorizationV2> {
    inspect_preceding_ed25519_v2(
        Ed25519InstructionViewV2 {
            program_id,
            ed25519_data,
            preceding_index,
            current_index: 5,
            current_data,
        },
        Ed25519ExpectationV2 {
            message_offset: 16,
            signer,
            message,
        },
    )
}

fn register(
    value: DirectIntentV2,
    replay_root: MakerReplayRootV2,
    balances: [u64; 2],
) -> Result<RegistrationV2<2>> {
    let instruction = encode_register_instruction_v2(value)?;
    register_intent_v2(
        replay_root,
        value,
        authorization(*value.maker(), &value.signed_preimage(), &instruction, 16)?,
        accounts(value),
        9,
        key(230 + value.maker()[0]),
        position(value.maker()[0], balances)?,
    )
}

#[test]
fn fixed_layout_round_trips_one_semantic_intent_root_and_record() -> Result<()> {
    let value = intent(1, Side::Buy, 1, 0, 600_000, 10, 100)?;
    let intent_bytes = value.signed_preimage();
    assert_eq!(intent_bytes.len(), DIRECT_INTENT_BYTES_V2);
    assert_eq!(
        DirectIntentV2::decode_signed_preimage(&intent_bytes)?,
        value
    );
    let registration = register(value, root(1)?, [0, 0])?;
    let mut root_bytes = [0; MAKER_REPLAY_ROOT_BYTES_V2];
    registration.replay_root.encode(&mut root_bytes)?;
    assert_eq!(
        MakerReplayRootV2::decode(&root_bytes)?,
        registration.replay_root
    );
    let mut record_bytes = [0; DIRECT_INTENT_RECORD_BYTES_V2];
    registration.record.encode(&mut record_bytes)?;
    assert_eq!(
        DirectIntentRecordV2::decode(&record_bytes)?,
        registration.record
    );
    assert_eq!(registration.reserved_collateral_debit, 6);
    Ok(())
}

#[test]
fn signature_parser_refuses_forgery_wrong_signer_message_and_order() -> Result<()> {
    let value = intent(1, Side::Buy, 0, 0, PRICE_SCALE, 1, 0)?;
    let message = value.signed_preimage();
    let current = encode_register_instruction_v2(value)?;
    let mut data = canonical_ed25519_test_instruction([key(1)], [16], [232], 5);
    let data_len = canonical_ed25519_test_instruction_len(1);
    data.get_mut(48..112).ok_or(Error::InvalidLength)?.fill(0);
    assert_eq!(
        inspect_test(
            ED25519_PROGRAM_ID_3_0,
            data.get(..data_len)
                .ok_or(Error::InvalidSignatureInstruction)?,
            4,
            &current,
            key(1),
            &message,
        ),
        Err(Error::ForgedSignature)
    );
    data = canonical_ed25519_test_instruction([key(1)], [16], [232], 5);
    assert_eq!(
        inspect_test(
            ED25519_PROGRAM_ID_3_0,
            data.get(..data_len)
                .ok_or(Error::InvalidSignatureInstruction)?,
            4,
            &current,
            key(2),
            &message,
        ),
        Err(Error::SignatureSignerMismatch)
    );
    let mut other = message;
    other[120] ^= 1;
    assert_eq!(
        inspect_test(
            ED25519_PROGRAM_ID_3_0,
            data.get(..data_len)
                .ok_or(Error::InvalidSignatureInstruction)?,
            4,
            &current,
            key(1),
            &other,
        ),
        Err(Error::SignatureMessageMismatch)
    );
    assert_eq!(
        inspect_test(
            ED25519_PROGRAM_ID_3_0,
            data.get(..data_len)
                .ok_or(Error::InvalidSignatureInstruction)?,
            3,
            &current,
            key(1),
            &message,
        ),
        Err(Error::InvalidSignatureInstructionOrder)
    );
    assert_eq!(
        inspect_test(
            key(9),
            data.get(..data_len)
                .ok_or(Error::InvalidSignatureInstruction)?,
            4,
            &current,
            key(1),
            &message,
        ),
        Err(Error::InvalidSignatureProgram)
    );
    let mut hostile = data;
    hostile
        .get_mut(14..16)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(
        inspect_test(
            ED25519_PROGRAM_ID_3_0,
            hostile
                .get(..data_len)
                .ok_or(Error::InvalidSignatureInstruction)?,
            4,
            &current,
            key(1),
            &message,
        ),
        Err(Error::InvalidSignatureInstruction)
    );
    hostile = data;
    hostile
        .get_mut(10..12)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&17_u16.to_le_bytes());
    assert_eq!(
        inspect_test(
            ED25519_PROGRAM_ID_3_0,
            hostile
                .get(..data_len)
                .ok_or(Error::InvalidSignatureInstruction)?,
            4,
            &current,
            key(1),
            &message,
        ),
        Err(Error::InvalidSignatureInstruction)
    );
    assert_eq!(
        inspect_test(
            ED25519_PROGRAM_ID_3_0,
            data.get(..data_len + 1)
                .ok_or(Error::InvalidSignatureInstruction)?,
            4,
            &current,
            key(1),
            &message,
        ),
        Err(Error::InvalidSignatureInstruction)
    );
    let overlap = canonical_ed25519_test_instruction([key(1), key(1)], [16, 16], [232, 232], 5);
    assert_eq!(
        inspect_preceding_ed25519_batch_v2(
            Ed25519InstructionViewV2 {
                program_id: ED25519_PROGRAM_ID_3_0,
                ed25519_data: overlap
                    .get(..canonical_ed25519_test_instruction_len(2))
                    .ok_or(Error::InvalidSignatureInstruction)?,
                preceding_index: 4,
                current_index: 5,
                current_data: &current,
            },
            [
                Ed25519ExpectationV2 {
                    message_offset: 16,
                    signer: key(1),
                    message: &message
                },
                Ed25519ExpectationV2 {
                    message_offset: 16,
                    signer: key(1),
                    message: &message
                },
            ],
        ),
        Err(Error::InvalidSignatureInstruction)
    );
    Ok(())
}

#[test]
fn gap_free_nonce_refuses_gap_race_and_closed_replay() -> Result<()> {
    let initial = root(1)?;
    let gap = intent(1, Side::Buy, 0, 1, PRICE_SCALE, 1, 0)?;
    assert_eq!(register(gap, initial, [0, 0]), Err(Error::NonceMismatch));

    let first = intent(1, Side::Buy, 0, 0, PRICE_SCALE, 1, 0)?;
    let winner = register(first, initial, [0, 0])?;
    assert_eq!(winner.replay_root.next_registration_nonce(), 1);
    assert_eq!(winner.replay_root.live_intent_count(), 1);
    assert_eq!(
        register(first, winner.replay_root, [0, 0]),
        Err(Error::NonceMismatch)
    );

    let racing_variant = DirectIntentV2::new(DirectIntentInputV2 {
        collateral_account: key(200),
        ..DirectIntentInputV2 {
            market: key(7),
            generation: 3,
            maker: key(1),
            nonce: 0,
            valid_from_slot: 10,
            valid_through_slot: 20,
            side: Side::Buy,
            lifecycle: IntentLifecycleV2::Registered,
            outcome: 1,
            max_fill: 1,
            limit_price: PRICE_SCALE,
            fee_config: key(8),
            fee_basis_points: 0,
            position_account: key(41),
            collateral_account: key(81),
        }
    })?;
    assert!(register(racing_variant, initial, [0, 0]).is_ok());
    assert_eq!(
        register(racing_variant, winner.replay_root, [0, 0]),
        Err(Error::NonceMismatch)
    );
    Ok(())
}

#[test]
fn ordinary_partial_fills_use_only_persisted_custody_and_close_cleanly() -> Result<()> {
    let ask = intent(1, Side::Sell, 0, 0, 400_000, 10, 0)?;
    let bid = intent(2, Side::Buy, 0, 0, 600_000, 10, 0)?;
    let seller = register(ask, root(1)?, [10, 0])?;
    let buyer = register(bid, root(2)?, [0, 0])?;
    assert_eq!(seller.position.balances(), &[0, 0]);
    assert_eq!(seller.record.reserved_claims(), 10);
    assert_eq!(buyer.record.reserved_collateral(), 6);

    let first = settle_ordinary_v2(OrdinaryMatchV2 {
        slot: 12,
        seller_replay_root: seller.replay_root,
        buyer_replay_root: buyer.replay_root,
        seller_record: seller.record,
        buyer_record: buyer.record,
        seller_accounts: accounts(ask),
        buyer_accounts: accounts(bid),
        seller_position: seller.position,
        buyer_position: buyer.position,
        fill: 5,
        execution_price: 600_000,
        fee_policy: policy(0)?,
        fee_recipient_account: key(99),
    })?;
    assert_eq!(first.buyer_position.balances(), &[5, 0]);
    assert_eq!(first.seller_collateral_credit, 3);
    assert!(!first.seller_record.is_closed());
    let seller_record = first
        .seller_record
        .live_record
        .ok_or(Error::InvalidReservation)?;
    let buyer_record = first
        .buyer_record
        .live_record
        .ok_or(Error::InvalidReservation)?;
    let second = settle_ordinary_v2(OrdinaryMatchV2 {
        seller_replay_root: first.seller_replay_root,
        buyer_replay_root: first.buyer_replay_root,
        seller_record,
        buyer_record,
        seller_position: first.seller_position,
        buyer_position: first.buyer_position,
        ..OrdinaryMatchV2 {
            slot: 12,
            seller_replay_root: seller.replay_root,
            buyer_replay_root: buyer.replay_root,
            seller_record: seller.record,
            buyer_record: buyer.record,
            seller_accounts: accounts(ask),
            buyer_accounts: accounts(bid),
            seller_position: seller.position,
            buyer_position: buyer.position,
            fill: 5,
            execution_price: 600_000,
            fee_policy: policy(0)?,
            fee_recipient_account: key(99),
        }
    })?;
    assert!(second.seller_record.is_closed());
    assert!(second.buyer_record.is_closed());
    assert_eq!(second.seller_replay_root.live_intent_count(), 0);
    assert_eq!(second.buyer_replay_root.live_intent_count(), 0);
    assert_eq!(second.buyer_position.balances(), &[10, 0]);
    Ok(())
}

#[test]
fn cancellation_expiry_refunds_exact_payers_and_double_close_refuses() -> Result<()> {
    let sell = intent(1, Side::Sell, 0, 0, PRICE_SCALE, 10, 0)?;
    let registration = register(sell, root(1)?, [10, 0])?;
    let cancel_message = DirectCancelV2::for_record(registration.record).signed_preimage();
    let cancel_instruction =
        encode_cancel_instruction_v2(Side::Sell, DirectCancelV2::for_record(registration.record));
    let cancelled = cancel_intent_v2(
        registration.replay_root,
        registration.record,
        authorization(key(1), &cancel_message, &cancel_instruction, 16)?,
        accounts(sell),
        registration.position,
    )?;
    assert_eq!(cancelled.position.balances(), &[10, 0]);
    assert_eq!(cancelled.close.claim_refund, 10);
    assert_eq!(cancelled.close.rent_refund_payer, key(231));
    assert_eq!(cancelled.replay_root.live_intent_count(), 0);
    assert_eq!(
        cancel_intent_v2(
            cancelled.replay_root,
            registration.record,
            authorization(key(1), &cancel_message, &cancel_instruction, 16)?,
            accounts(sell),
            registration.position,
        ),
        Err(Error::LiveCountInvariant)
    );

    let buy = intent(2, Side::Buy, 0, 0, PRICE_SCALE, 1, 0)?;
    let buy_registration = register(buy, root(2)?, [0, 0])?;
    assert_eq!(
        expire_intent_v2(
            buy_registration.replay_root,
            buy_registration.record,
            20,
            accounts(buy),
            buy_registration.position,
        ),
        Err(Error::IntentNotExpired)
    );
    let expired = expire_intent_v2(
        buy_registration.replay_root,
        buy_registration.record,
        21,
        accounts(buy),
        buy_registration.position,
    )?;
    assert_eq!(expired.close.collateral_refund, 1);
    assert_eq!(expired.close.rent_refund_payer, key(232));
    Ok(())
}

#[test]
fn wrong_root_and_market_retirement_lifecycle_refuse() -> Result<()> {
    let value = intent(1, Side::Buy, 0, 0, PRICE_SCALE, 1, 0)?;
    assert_eq!(
        register(value, root(2)?, [0, 0]),
        Err(Error::ReplayRootMismatch)
    );
    let wrong_generation = MakerReplayRootV2::new(key(7), 4, key(1), key(21), 1)?;
    assert_eq!(
        register(value, wrong_generation, [0, 0]),
        Err(Error::ReplayRootMismatch)
    );
    let registration = register(value, root(1)?, [0, 0])?;
    assert_eq!(
        prepare_replay_root_close_v2(registration.replay_root),
        Err(Error::RegistrationStillOpen)
    );
    let closed_registration = close_replay_registration_v2(registration.replay_root)?;
    assert_eq!(
        register(
            intent(1, Side::Buy, 0, 1, PRICE_SCALE, 1, 0)?,
            closed_registration,
            [0, 0],
        ),
        Err(Error::RegistrationClosed)
    );
    assert_eq!(
        prepare_replay_root_close_v2(closed_registration),
        Err(Error::LiveIntentsRemain)
    );
    let expired = expire_intent_v2(
        closed_registration,
        registration.record,
        21,
        accounts(value),
        registration.position,
    )?;
    let root_close = prepare_replay_root_close_v2(expired.replay_root)?;
    assert_eq!(root_close.rent_refund_payer, key(21));
    assert_eq!(root_close.final_next_nonce, 1);
    Ok(())
}

#[test]
fn hostile_root_counts_and_nonce_overflow_refuse() -> Result<()> {
    let mut bytes = [0; MAKER_REPLAY_ROOT_BYTES_V2];
    root(1)?.encode(&mut bytes)?;
    bytes
        .get_mut(96..104)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(
        MakerReplayRootV2::decode(&bytes),
        Err(Error::LiveCountInvariant)
    );

    bytes
        .get_mut(96..104)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&u64::MAX.to_le_bytes());
    bytes
        .get_mut(88..96)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&u64::MAX.to_le_bytes());
    let exhausted = MakerReplayRootV2::decode(&bytes)?;
    let final_nonce = intent(1, Side::Buy, 0, u64::MAX, PRICE_SCALE, 1, 0)?;
    assert_eq!(
        register(final_nonce, exhausted, [0, 0]),
        Err(Error::LiveCountInvariant)
    );
    let inline_final = intent_lifecycle(
        IntentLifecycleV2::InlineFillOrKill,
        (1, Side::Buy, 0, u64::MAX, PRICE_SCALE, 1, 0),
    )?;
    assert_eq!(
        exhausted.consume_inline(inline_final, 1),
        Err(Error::NonceMismatch)
    );
    Ok(())
}

#[test]
fn complementary_paths_are_custodied_conservative_and_atomic_on_refusal() -> Result<()> {
    let buy0 = intent(1, Side::Buy, 0, 0, 500_000, 10, 0)?;
    let buy1 = intent(2, Side::Buy, 1, 0, 500_000, 10, 0)?;
    let first = register(buy0, root(1)?, [0, 0])?;
    let second = register(buy1, root(2)?, [0, 0])?;
    let split_input = ComplementaryBuyMatchV2 {
        slot: 12,
        buyer_replay_roots: [first.replay_root, second.replay_root],
        buyer_records: [first.record, second.record],
        buyer_accounts: [accounts(buy0), accounts(buy1)],
        buyer_positions: [first.position, second.position],
        fill: 10,
        execution_prices: [500_000, 500_000],
        fee_policy: policy(0)?,
        fee_recipient_account: key(99),
    };
    let split = settle_split_v2(split_input)?;
    assert_eq!(split.buyer_gross_collateral_debits, [5, 5]);
    assert_eq!(split.market_vault_collateral_credit, 10);
    assert_eq!(split.buyer_positions[0].balances(), &[10, 0]);
    assert_eq!(split.buyer_positions[1].balances(), &[0, 10]);

    let mut hostile = split_input;
    hostile.fill = 1;
    hostile.execution_prices = [400_000, 600_000];
    let before = hostile;
    assert_eq!(settle_split_v2(hostile), Err(Error::NonIntegralQuote));
    assert_eq!(hostile, before);

    let sell0 = intent(3, Side::Sell, 0, 0, 500_000, 10, 0)?;
    let sell1 = intent(4, Side::Sell, 1, 0, 500_000, 10, 0)?;
    let third = register(sell0, root(3)?, [10, 0])?;
    let fourth = register(sell1, root(4)?, [0, 10])?;
    let merge = settle_merge_v2(ComplementarySellMatchV2 {
        slot: 12,
        seller_replay_roots: [third.replay_root, fourth.replay_root],
        seller_records: [third.record, fourth.record],
        seller_accounts: [accounts(sell0), accounts(sell1)],
        seller_positions: [third.position, fourth.position],
        fill: 10,
        execution_prices: [500_000, 500_000],
        fee_policy: policy(0)?,
        fee_recipient_account: key(99),
    })?;
    assert_eq!(merge.seller_gross_collateral_credits, [5, 5]);
    assert_eq!(merge.market_vault_collateral_debit, 10);
    assert!(
        merge
            .seller_records
            .iter()
            .all(RecordAfterFillV2::is_closed)
    );
    Ok(())
}

#[test]
fn inline_fok_ioc_consumes_same_nonce_without_live_rent_and_cross_mode_replay_refuses() -> Result<()>
{
    let ask = intent_lifecycle(
        IntentLifecycleV2::InlineFillOrKill,
        (1, Side::Sell, 0, 0, 400_000, 5, 0),
    )?;
    let bid = intent_lifecycle(
        IntentLifecycleV2::InlineImmediateOrCancel,
        (2, Side::Buy, 0, 0, 600_000, 10, 0),
    )?;
    let instruction = encode_inline_ordinary_instruction_v2(5, 600_000, ask, bid)?;
    assert_eq!(instruction.len(), 498);
    assert_eq!(
        decode_inline_ordinary_instruction_v2(&instruction)?.buyer_intent,
        bid
    );
    let ed = canonical_ed25519_test_instruction([key(1), key(2)], [34, 266], [232, 232], 5);
    let authorizations = inspect_preceding_ed25519_batch_v2(
        Ed25519InstructionViewV2 {
            program_id: ED25519_PROGRAM_ID_3_0,
            ed25519_data: ed
                .get(..canonical_ed25519_test_instruction_len(2))
                .ok_or(Error::InvalidSignatureInstruction)?,
            preceding_index: 4,
            current_index: 5,
            current_data: &instruction,
        },
        [
            Ed25519ExpectationV2 {
                message_offset: 34,
                signer: key(1),
                message: &ask.signed_preimage(),
            },
            Ed25519ExpectationV2 {
                message_offset: 266,
                signer: key(2),
                message: &bid.signed_preimage(),
            },
        ],
    )?;
    let settled = settle_inline_ordinary_v2(InlineOrdinaryMatchV2 {
        slot: 12,
        seller_replay_root: root(1)?,
        buyer_replay_root: root(2)?,
        seller_intent: ask,
        buyer_intent: bid,
        seller_authorization: authorizations[0],
        buyer_authorization: authorizations[1],
        seller_accounts: inline_accounts(ask),
        buyer_accounts: inline_accounts(bid),
        seller_position: position(1, [5, 0])?,
        buyer_position: position(2, [0, 0])?,
        fill: 5,
        execution_price: 600_000,
        fee_policy: policy(0)?,
        fee_recipient_account: key(99),
    })?;
    assert_eq!(settled.seller_position.balances(), &[0, 0]);
    assert_eq!(settled.buyer_position.balances(), &[5, 0]);
    assert_eq!(settled.seller_replay_root.next_registration_nonce(), 1);
    assert_eq!(settled.seller_replay_root.live_intent_count(), 0);

    let resting_same_nonce = intent(1, Side::Sell, 0, 0, 400_000, 5, 0)?;
    assert_eq!(
        register(resting_same_nonce, settled.seller_replay_root, [5, 0]),
        Err(Error::NonceMismatch)
    );

    let registered = register(resting_same_nonce, root(1)?, [5, 0])?;
    assert_eq!(
        settle_inline_ordinary_v2(InlineOrdinaryMatchV2 {
            seller_replay_root: registered.replay_root,
            ..InlineOrdinaryMatchV2 {
                slot: 12,
                seller_replay_root: root(1)?,
                buyer_replay_root: root(2)?,
                seller_intent: ask,
                buyer_intent: bid,
                seller_authorization: authorizations[0],
                buyer_authorization: authorizations[1],
                seller_accounts: inline_accounts(ask),
                buyer_accounts: inline_accounts(bid),
                seller_position: position(1, [5, 0])?,
                buyer_position: position(2, [0, 0])?,
                fill: 5,
                execution_price: 600_000,
                fee_policy: policy(0)?,
                fee_recipient_account: key(99),
            }
        }),
        Err(Error::NonceMismatch)
    );
    Ok(())
}

#[test]
fn inline_complementary_n2_fits_and_n3_is_physically_refused() -> Result<()> {
    let a = intent_lifecycle(
        IntentLifecycleV2::InlineFillOrKill,
        (1, Side::Buy, 0, 0, 500_000, 10, 0),
    )?;
    let b = intent_lifecycle(
        IntentLifecycleV2::InlineFillOrKill,
        (2, Side::Buy, 1, 0, 500_000, 10, 0),
    )?;
    let mut instruction = [0; 506];
    encode_inline_complementary_instruction_v2(
        AdapterActionV2::InlineSplit,
        10,
        [500_000, 500_000],
        [a, b],
        &mut instruction,
    )?;
    let decoded = decode_inline_complementary_instruction_v2::<2>(
        &instruction,
        AdapterActionV2::InlineSplit,
    )?;
    assert_eq!(decoded.1, [a, b]);
    let ed = canonical_ed25519_test_instruction([key(1), key(2)], [42, 274], [232, 232], 5);
    let authorizations = inspect_preceding_ed25519_batch_v2(
        Ed25519InstructionViewV2 {
            program_id: ED25519_PROGRAM_ID_3_0,
            ed25519_data: ed
                .get(..canonical_ed25519_test_instruction_len(2))
                .ok_or(Error::InvalidSignatureInstruction)?,
            preceding_index: 4,
            current_index: 5,
            current_data: &instruction,
        },
        [
            Ed25519ExpectationV2 {
                message_offset: 42,
                signer: key(1),
                message: &a.signed_preimage(),
            },
            Ed25519ExpectationV2 {
                message_offset: 274,
                signer: key(2),
                message: &b.signed_preimage(),
            },
        ],
    )?;
    let settled = settle_inline_complementary_v2(InlineComplementaryMatchV2 {
        slot: 12,
        side: Side::Buy,
        replay_roots: [root(1)?, root(2)?],
        intents: [a, b],
        authorizations,
        accounts: [inline_accounts(a), inline_accounts(b)],
        positions: [position(1, [0, 0])?, position(2, [0, 0])?],
        fill: 10,
        execution_prices: [500_000, 500_000],
        fee_policy: policy(0)?,
        fee_recipient_account: key(99),
    })?;
    assert_eq!(settled.gross_collateral, [5, 5]);
    assert_eq!(settled.positions[0].balances(), &[10, 0]);
    assert_eq!(settled.positions[1].balances(), &[0, 10]);
    assert_eq!(
        inline_complementary_instruction_bytes_v2(3),
        Err(Error::InvalidInlineWidth)
    );
    assert_eq!(
        measured_settlement_envelope_v2(AdapterActionV2::InlineSplit, 3),
        Err(Error::InvalidInlineWidth)
    );
    assert_eq!(measured_inline_complementary_reference_v2(2)?, 993);
    assert_eq!(measured_inline_complementary_reference_v2(3)?, 1_350);
    assert_eq!(measured_inline_complementary_reference_v2(16)?, 5_991);
    Ok(())
}

#[test]
fn aliases_mixed_modes_and_packet_overflow_refuse() -> Result<()> {
    let ask = intent(1, Side::Sell, 0, 0, PRICE_SCALE, 1, 0)?;
    let bid = intent(2, Side::Buy, 0, 0, PRICE_SCALE, 1, 0)?;
    let seller = register(ask, root(1)?, [1, 0])?;
    let buyer = register(bid, root(2)?, [0, 0])?;
    let mut buyer_accounts = accounts(bid);
    buyer_accounts.record = accounts(ask).record;
    assert_eq!(
        settle_ordinary_v2(OrdinaryMatchV2 {
            slot: 12,
            seller_replay_root: seller.replay_root,
            buyer_replay_root: buyer.replay_root,
            seller_record: seller.record,
            buyer_record: buyer.record,
            seller_accounts: accounts(ask),
            buyer_accounts,
            seller_position: seller.position,
            buyer_position: buyer.position,
            fill: 1,
            execution_price: PRICE_SCALE,
            fee_policy: policy(0)?,
            fee_recipient_account: key(99),
        }),
        Err(Error::Alias)
    );

    let mut data = encode_ordinary_instruction_v2(1, PRICE_SCALE);
    data[33] = 1;
    assert_eq!(
        decode_ordinary_instruction_v2(&data),
        Err(Error::MixedAuthorizationModes)
    );
    data[32] = 1;
    assert_eq!(
        decode_ordinary_instruction_v2(&data),
        Err(Error::AuthorizationLifecycleMismatch)
    );

    let split16 = measured_settlement_envelope_v2(AdapterActionV2::Split, 16)?;
    assert_eq!(split16.instruction_accounts, 86);
    assert_eq!(split16.total_account_locks, 87);
    assert_eq!(split16.serialized_transaction_bytes, 545);
    assert!(split16.serialized_transaction_bytes < SOLANA_PACKET_DATA_SIZE_3_0);
    let packet = PacketAdmissionV2 {
        serialized_transaction_bytes: split16.serialized_transaction_bytes,
        instruction_accounts: split16.instruction_accounts,
        instruction_data_bytes: split16.instruction_data_bytes,
        total_account_locks: split16.total_account_locks,
        transaction_signatures: MEASURED_TRANSACTION_SIGNATURES_V2,
        address_lookup_tables: MEASURED_LOOKUP_TABLES_V2,
    };
    admit_settlement_packet_v2(AdapterActionV2::Split, 16, packet)?;
    assert_eq!(
        admit_settlement_packet_v2(
            AdapterActionV2::Split,
            16,
            PacketAdmissionV2 {
                serialized_transaction_bytes: SOLANA_PACKET_DATA_SIZE_3_0 + 1,
                ..packet
            },
        ),
        Err(Error::PacketEnvelopeExceeded)
    );
    assert_eq!(stateless_shared_message_ed25519_minimum_v2(16, 0)?, 1_762);
    Ok(())
}

#[test]
fn rent_principal_and_donation_are_not_conflated() -> Result<()> {
    let transition = terminal_rent_transition_v2(12, 9)?;
    assert_eq!(transition.payer_refund, 9);
    assert_eq!(transition.neutral_donation, 3);
    assert_eq!(
        terminal_rent_transition_v2(8, 9),
        Err(Error::InvalidRentTransition)
    );
    Ok(())
}
