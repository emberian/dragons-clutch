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
    VenueFeePolicyV2::new(key(7), 3, key(99), bps)
}

fn buy_reserve(value: DirectIntentV2) -> u64 {
    let gross = u64::try_from(
        u128::from(value.max_fill()) * u128::from(value.limit_price()) / u128::from(PRICE_SCALE),
    )
    .unwrap_or(0);
    gross
        + u64::try_from(
            u128::from(gross) * u128::from(value.fee_basis_points())
                / u128::from(FEE_BASIS_POINTS_DENOMINATOR),
        )
        .unwrap_or(0)
}

fn buy_authority(value: DirectIntentV2, exact_debit: u64) -> adapter::BuyDebitAuthorityV2 {
    adapter::BuyDebitAuthorityV2 {
        token_account: *value.collateral_account(),
        mint: key(6),
        owner: *value.maker(),
        delegate: inline_accounts(value).replay_root,
        delegated_amount: exact_debit,
    }
}

fn escrow_authority(value: DirectIntentV2) -> adapter::EscrowAuthorityV2 {
    adapter::EscrowAuthorityV2 {
        token_account: accounts(value).escrow,
        mint: key(6),
        authority: accounts(value).record,
    }
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
    let buy_debit = if value.side() == Side::Buy {
        Some(buy_authority(value, buy_reserve(value)))
    } else {
        None
    };
    register_intent_v2(RegistrationInputV2 {
        replay_root: ReplayRootStateV2::existing(replay_root),
        intent: value,
        authorization: authorization(*value.maker(), &value.signed_preimage(), &instruction, 16)?,
        phase: adapter::MarketPhaseV2::Open,
        slot: 12,
        accounts: accounts(value),
        system_payer: key(230 + value.maker()[0]),
        collateral_mint: if value.side() == Side::Buy {
            Some(key(6))
        } else {
            None
        },
        buy_debit_authority: buy_debit,
        record_bump: 9,
        fee_policy: policy(value.fee_basis_points())?,
        fee_config_digest: key(8),
        position: position(value.maker()[0], balances)?,
    })
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
    let venue_policy = policy(100)?;
    let mut policy_bytes = [0; VENUE_FEE_POLICY_BYTES_V2];
    venue_policy.encode(&mut policy_bytes)?;
    assert_eq!(VenueFeePolicyV2::decode(&policy_bytes)?, venue_policy);
    assert_eq!(VENUE_FEE_POLICY_BYTES_V2, 88);
    assert_eq!(
        VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V2[0..4],
        [0xd0, 0xcb, 0x14, 0x80]
    );
    validate_venue_policy_selection_v2(value, policy(100)?, key(8))?;
    assert_eq!(
        validate_venue_policy_selection_v2(value, policy(100)?, key(9)),
        Err(Error::VenueUnauthorized)
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
        phase: adapter::MarketPhaseV2::Open,
        slot: 12,
        seller_replay_root: seller.replay_root,
        buyer_replay_root: buyer.replay_root,
        seller_record: seller.record,
        buyer_record: buyer.record,
        seller_accounts: accounts(ask),
        buyer_accounts: accounts(bid),
        seller_position: seller.position,
        buyer_position: buyer.position,
        collateral_mint: key(6),
        buyer_escrow_authority: escrow_authority(bid),
        fill: 5,
        execution_price: 600_000,
        fee_policy: policy(0)?,
        fee_config_digest: key(8),
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
            phase: adapter::MarketPhaseV2::Open,
            slot: 12,
            seller_replay_root: seller.replay_root,
            buyer_replay_root: buyer.replay_root,
            seller_record: seller.record,
            buyer_record: buyer.record,
            seller_accounts: accounts(ask),
            buyer_accounts: accounts(bid),
            seller_position: seller.position,
            buyer_position: buyer.position,
            collateral_mint: key(6),
            buyer_escrow_authority: escrow_authority(bid),
            fill: 5,
            execution_price: 600_000,
            fee_policy: policy(0)?,
            fee_config_digest: key(8),
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
    let cancelled = cancel_intent_v2(CancellationInputV2 {
        replay_root: registration.replay_root,
        record: registration.record,
        authorization: authorization(key(1), &cancel_message, &cancel_instruction, 16)?,
        phase: adapter::MarketPhaseV2::Open,
        accounts: accounts(sell),
        collateral_mint: None,
        escrow_authority: None,
        position: registration.position,
    })?;
    assert_eq!(cancelled.position.balances(), &[10, 0]);
    assert_eq!(cancelled.close.claim_refund, 10);
    assert_eq!(cancelled.close.rent_refund_payer, key(231));
    assert_eq!(cancelled.replay_root.live_intent_count(), 0);
    assert_eq!(
        cancel_intent_v2(CancellationInputV2 {
            replay_root: cancelled.replay_root,
            record: registration.record,
            authorization: authorization(key(1), &cancel_message, &cancel_instruction, 16)?,
            phase: adapter::MarketPhaseV2::Resolved,
            accounts: accounts(sell),
            collateral_mint: None,
            escrow_authority: None,
            position: registration.position,
        }),
        Err(Error::LiveCountInvariant)
    );

    let buy = intent(2, Side::Buy, 0, 0, PRICE_SCALE, 1, 0)?;
    let buy_registration = register(buy, root(2)?, [0, 0])?;
    assert_eq!(
        expire_intent_v2(ExpirationInputV2 {
            replay_root: buy_registration.replay_root,
            record: buy_registration.record,
            phase: adapter::MarketPhaseV2::Open,
            slot: 20,
            accounts: accounts(buy),
            collateral_mint: Some(key(6)),
            escrow_authority: Some(escrow_authority(buy)),
            position: buy_registration.position,
        }),
        Err(Error::IntentNotExpired)
    );
    let expired = expire_intent_v2(ExpirationInputV2 {
        replay_root: buy_registration.replay_root,
        record: buy_registration.record,
        phase: adapter::MarketPhaseV2::Resolved,
        slot: 21,
        accounts: accounts(buy),
        collateral_mint: Some(key(6)),
        escrow_authority: Some(escrow_authority(buy)),
        position: buy_registration.position,
    })?;
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
        prepare_replay_root_close_v2(registration.replay_root, adapter::MarketPhaseV2::Retiring,),
        Err(Error::RegistrationStillOpen)
    );
    let closed_registration =
        close_replay_registration_v2(registration.replay_root, adapter::MarketPhaseV2::Retiring)?;
    assert_eq!(
        register(
            intent(1, Side::Buy, 0, 1, PRICE_SCALE, 1, 0)?,
            closed_registration,
            [0, 0],
        ),
        Err(Error::RegistrationClosed)
    );
    assert_eq!(
        prepare_replay_root_close_v2(closed_registration, adapter::MarketPhaseV2::Retiring),
        Err(Error::LiveIntentsRemain)
    );
    let expired = expire_intent_v2(ExpirationInputV2 {
        replay_root: closed_registration,
        record: registration.record,
        phase: adapter::MarketPhaseV2::Retiring,
        slot: 21,
        accounts: accounts(value),
        collateral_mint: Some(key(6)),
        escrow_authority: Some(escrow_authority(value)),
        position: registration.position,
    })?;
    let root_close =
        prepare_replay_root_close_v2(expired.replay_root, adapter::MarketPhaseV2::Retiring)?;
    assert_eq!(root_close.rent_refund_payer, key(21));
    assert_eq!(root_close.final_next_nonce, 1);
    assert_eq!(root_close.final_minimum_live_nonce, 0);
    Ok(())
}

#[test]
fn hostile_root_counts_and_nonce_overflow_refuse() -> Result<()> {
    let mut bytes = [0; MAKER_REPLAY_ROOT_BYTES_V2];
    root(1)?.encode(&mut bytes)?;
    bytes
        .get_mut(104..112)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(
        MakerReplayRootV2::decode(&bytes),
        Err(Error::InvalidCancelThrough)
    );
    bytes
        .get_mut(104..112)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&0_u64.to_le_bytes());
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
        phase: adapter::MarketPhaseV2::Open,
        slot: 12,
        buyer_replay_roots: [first.replay_root, second.replay_root],
        buyer_records: [first.record, second.record],
        buyer_accounts: [accounts(buy0), accounts(buy1)],
        buyer_positions: [first.position, second.position],
        collateral_mint: key(6),
        escrow_authorities: [escrow_authority(buy0), escrow_authority(buy1)],
        fill: 10,
        execution_prices: [500_000, 500_000],
        fee_policy: policy(0)?,
        fee_config_digest: key(8),
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
        phase: adapter::MarketPhaseV2::Open,
        slot: 12,
        seller_replay_roots: [third.replay_root, fourth.replay_root],
        seller_records: [third.record, fourth.record],
        seller_accounts: [accounts(sell0), accounts(sell1)],
        seller_positions: [third.position, fourth.position],
        fill: 10,
        execution_prices: [500_000, 500_000],
        fee_policy: policy(0)?,
        fee_config_digest: key(8),
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
        phase: adapter::MarketPhaseV2::Open,
        slot: 12,
        seller_replay_root: ReplayRootStateV2::absent(1),
        buyer_replay_root: ReplayRootStateV2::absent(2),
        root_creation_payer: key(200),
        seller_intent: ask,
        buyer_intent: bid,
        seller_authorization: authorizations[0],
        buyer_authorization: authorizations[1],
        seller_accounts: inline_accounts(ask),
        buyer_accounts: inline_accounts(bid),
        seller_position: position(1, [5, 0])?,
        buyer_position: position(2, [0, 0])?,
        collateral_mint: key(6),
        buyer_debit_authority: buy_authority(bid, 3),
        fill: 5,
        execution_price: 600_000,
        fee_policy: policy(0)?,
        fee_config_digest: key(8),
        fee_recipient_account: key(99),
    })?;
    assert_eq!(settled.seller_position.balances(), &[0, 0]);
    assert_eq!(settled.buyer_position.balances(), &[5, 0]);
    assert_eq!(settled.seller_replay_root.next_registration_nonce(), 1);
    assert_eq!(settled.seller_replay_root.live_intent_count(), 0);
    assert_eq!(settled.seller_replay_root.rent_payer(), &key(200));
    assert_eq!(settled.buyer_replay_root.rent_payer(), &key(200));

    let resting_same_nonce = intent(1, Side::Sell, 0, 0, 400_000, 5, 0)?;
    assert_eq!(
        register(resting_same_nonce, settled.seller_replay_root, [5, 0]),
        Err(Error::NonceMismatch)
    );

    let registered = register(resting_same_nonce, root(1)?, [5, 0])?;
    assert_eq!(
        settle_inline_ordinary_v2(InlineOrdinaryMatchV2 {
            seller_replay_root: ReplayRootStateV2::existing(registered.replay_root),
            ..InlineOrdinaryMatchV2 {
                phase: adapter::MarketPhaseV2::Open,
                slot: 12,
                seller_replay_root: ReplayRootStateV2::existing(root(1)?),
                buyer_replay_root: ReplayRootStateV2::existing(root(2)?),
                root_creation_payer: key(200),
                seller_intent: ask,
                buyer_intent: bid,
                seller_authorization: authorizations[0],
                buyer_authorization: authorizations[1],
                seller_accounts: inline_accounts(ask),
                buyer_accounts: inline_accounts(bid),
                seller_position: position(1, [5, 0])?,
                buyer_position: position(2, [0, 0])?,
                collateral_mint: key(6),
                buyer_debit_authority: buy_authority(bid, 3),
                fill: 5,
                execution_price: 600_000,
                fee_policy: policy(0)?,
                fee_config_digest: key(8),
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
        phase: adapter::MarketPhaseV2::Open,
        slot: 12,
        side: Side::Buy,
        replay_roots: [ReplayRootStateV2::absent(1), ReplayRootStateV2::absent(2)],
        root_creation_payer: key(200),
        intents: [a, b],
        authorizations,
        accounts: [inline_accounts(a), inline_accounts(b)],
        positions: [position(1, [0, 0])?, position(2, [0, 0])?],
        collateral_mint: key(6),
        buy_debit_authorities: [Some(buy_authority(a, 5)), Some(buy_authority(b, 5))],
        fill: 10,
        execution_prices: [500_000, 500_000],
        fee_policy: policy(0)?,
        fee_config_digest: key(8),
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
    assert_eq!(measured_inline_complementary_reference_v2(2)?, 1_011);
    assert_eq!(measured_inline_complementary_reference_v2(3)?, 1_368);
    assert_eq!(measured_inline_complementary_reference_v2(16)?, 6_009);
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
            phase: adapter::MarketPhaseV2::Open,
            slot: 12,
            seller_replay_root: seller.replay_root,
            buyer_replay_root: buyer.replay_root,
            seller_record: seller.record,
            buyer_record: buyer.record,
            seller_accounts: accounts(ask),
            buyer_accounts,
            seller_position: seller.position,
            buyer_position: buyer.position,
            collateral_mint: key(6),
            buyer_escrow_authority: adapter::EscrowAuthorityV2 {
                authority: buyer_accounts.record,
                ..escrow_authority(bid)
            },
            fill: 1,
            execution_price: PRICE_SCALE,
            fee_policy: policy(0)?,
            fee_config_digest: key(8),
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
    assert_eq!(split16.instruction_accounts, 108);
    assert_eq!(split16.total_account_locks, 110);
    assert_eq!(split16.serialized_transaction_bytes, 621);
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
fn rent_principal_and_donation_bind_to_exact_permanent_credit() -> Result<()> {
    let authority =
        dclutch_rent_contract::RefundAuthority::new(key(231)).expect("nonzero refund authority");
    let rent_credit = dclutch_rent_contract::RentCreditV1::new(authority, 17);
    let plan =
        terminal_rent_credit_close_plan_v1(key(231), &rent_credit.to_bytes(), 17, 12, 9, 100)?;
    let transition = plan.classification();
    assert_eq!(transition.rent_principal, 9);
    assert_eq!(transition.unclassified_donation, 3);
    assert_eq!(transition.rent_credit_total, 12);
    assert_eq!(
        plan.rent_credit().pda_seeds().domain(),
        dclutch_rent_contract::RENT_CREDIT_PDA_DOMAIN_V1
    );
    assert_eq!(
        plan.rent_credit().pda_seeds().refund_authority().to_bytes(),
        key(231)
    );
    assert_eq!(plan.rent_credit().pda_seeds().bump(), 17);
    assert_eq!(plan.source_close().source_before(), 12);
    assert_eq!(plan.source_close().credit_before(), 100);
    assert_eq!(plan.source_close().credited_lamports(), 12);
    plan.source_close()
        .validate_post(0, 112)
        .expect("exact source close");
    assert_eq!(
        terminal_rent_transition_v2(8, 9)?,
        TerminalRentTransitionV2 {
            rent_principal: 8,
            unclassified_donation: 0,
            rent_credit_total: 8,
        }
    );
    assert_eq!(
        terminal_rent_credit_close_plan_v1(key(230), &rent_credit.to_bytes(), 17, 12, 9, 100,),
        Err(Error::RentCreditContract(
            dclutch_rent_contract::Error::CreditBindingMismatch
        ))
    );
    assert_eq!(
        terminal_rent_credit_close_plan_v1(key(231), &rent_credit.to_bytes(), 16, 12, 9, 100,),
        Err(Error::RentCreditContract(
            dclutch_rent_contract::Error::CreditBindingMismatch
        ))
    );
    Ok(())
}

#[test]
fn hostile_action_routing_phase_and_slot_matrix_refuse() -> Result<()> {
    let close_registration = adapter::encode_close_replay_registration_instruction_v2();
    assert_eq!(
        adapter::decode_adapter_header_v2(&close_registration)?,
        adapter::AdapterHeaderV2 {
            action: AdapterActionV2::CloseReplayRegistration,
            participants: 1,
        }
    );
    adapter::decode_close_replay_registration_instruction_v2(&close_registration)?;
    assert_eq!(
        adapter::decode_close_replay_root_instruction_v2(&close_registration),
        Err(Error::UnknownAdapterAction)
    );
    let mut reserved = close_registration;
    reserved[12] = 1;
    assert_eq!(
        adapter::decode_adapter_header_v2(&reserved),
        Err(Error::NonCanonicalReservedBytes)
    );

    adapter::require_market_phase_v2(AdapterActionV2::RegisterBuy, adapter::MarketPhaseV2::Open)?;
    assert_eq!(
        adapter::require_market_phase_v2(
            AdapterActionV2::RegisterBuy,
            adapter::MarketPhaseV2::Resolved,
        ),
        Err(Error::MarketPhaseRefused)
    );
    for phase in [
        adapter::MarketPhaseV2::Open,
        adapter::MarketPhaseV2::Resolved,
        adapter::MarketPhaseV2::Retiring,
    ] {
        adapter::require_market_phase_v2(AdapterActionV2::ExpireBuy, phase)?;
    }
    assert_eq!(
        adapter::require_market_phase_v2(
            AdapterActionV2::CloseReplayRoot,
            adapter::MarketPhaseV2::Retired,
        ),
        Err(Error::MarketPhaseRefused)
    );

    let value = intent(1, Side::Buy, 0, 0, PRICE_SCALE, 1, 0)?;
    let instruction = encode_register_instruction_v2(value)?;
    assert_eq!(
        register_intent_v2(RegistrationInputV2 {
            replay_root: ReplayRootStateV2::absent(1),
            intent: value,
            authorization: authorization(
                *value.maker(),
                &value.signed_preimage(),
                &instruction,
                16,
            )?,
            phase: adapter::MarketPhaseV2::Open,
            slot: 9,
            accounts: accounts(value),
            system_payer: key(200),
            collateral_mint: Some(key(6)),
            buy_debit_authority: Some(buy_authority(value, 1)),
            record_bump: 9,
            fee_policy: policy(value.fee_basis_points())?,
            fee_config_digest: key(8),
            position: position(1, [0, 0])?,
        }),
        Err(Error::IntentExpired)
    );
    Ok(())
}

#[test]
fn token_delegate_and_live_record_escrow_authority_are_exact() -> Result<()> {
    let value = intent(1, Side::Buy, 0, 0, PRICE_SCALE, 2, 0)?;
    let authority = buy_authority(value, 2);
    adapter::validate_buy_debit_authority_v2(
        authority,
        value,
        accounts(value).replay_root,
        key(6),
        2,
    )?;
    assert_eq!(
        adapter::validate_buy_debit_authority_v2(
            adapter::BuyDebitAuthorityV2 {
                delegate: key(222),
                ..authority
            },
            value,
            accounts(value).replay_root,
            key(6),
            2,
        ),
        Err(Error::InvalidBuyDebitAuthority)
    );
    assert_eq!(
        adapter::validate_buy_debit_authority_v2(
            adapter::BuyDebitAuthorityV2 {
                delegated_amount: 3,
                ..authority
            },
            value,
            accounts(value).replay_root,
            key(6),
            2,
        ),
        Err(Error::InvalidBuyDebitAuthority)
    );
    let registered = register(value, root(1)?, [0, 0])?;
    let escrow = escrow_authority(value);
    adapter::validate_registered_escrow_authority_v2(
        escrow,
        registered.record,
        accounts(value).record,
        accounts(value).escrow,
        key(6),
    )?;
    assert_eq!(
        adapter::validate_registered_escrow_authority_v2(
            adapter::EscrowAuthorityV2 {
                authority: key(222),
                ..escrow
            },
            registered.record,
            accounts(value).record,
            accounts(value).escrow,
            key(6),
        ),
        Err(Error::InvalidEscrowAuthority)
    );
    Ok(())
}

#[test]
fn corrected_account_frames_are_action_specific_and_rent_credit_alias_safe() -> Result<()> {
    assert_eq!(
        adapter::account_count_v2(AdapterActionV2::RegisterBuy, 1)?,
        17
    );
    assert_eq!(
        adapter::account_count_v2(AdapterActionV2::RegisterSell, 1)?,
        12
    );
    assert_eq!(adapter::account_count_v2(AdapterActionV2::Ordinary, 2)?, 21);
    assert_eq!(adapter::account_count_v2(AdapterActionV2::Split, 16)?, 108);
    assert_eq!(adapter::account_count_v2(AdapterActionV2::Merge, 16)?, 92);
    assert_eq!(
        adapter::account_count_v2(AdapterActionV2::InlineOrdinary, 2)?,
        19
    );
    assert_eq!(
        adapter::account_count_v2(AdapterActionV2::CancelThrough, 1)?,
        3
    );
    assert_eq!(
        adapter::account_count_v2(AdapterActionV2::CloseInvalidatedBuy, 1)?,
        12
    );
    assert_eq!(
        adapter::account_role_v2(AdapterActionV2::InlineOrdinary, 2, 12)?,
        adapter::AccountRoleV2::InstructionsSysvar
    );
    assert_eq!(
        adapter::account_role_v2(AdapterActionV2::InlineSplit, 2, 8)?,
        adapter::AccountRoleV2::Custody
    );
    assert_eq!(
        adapter::account_role_v2(AdapterActionV2::RegisterBuy, 1, 5)?,
        adapter::AccountRoleV2::VenuePolicyStagingCursor
    );

    let mut split = [adapter::AdapterAccountMetaV2 {
        key: [0; 32],
        is_signer: false,
        is_writable: false,
    }; 24];
    for (index, meta) in split.iter_mut().enumerate() {
        meta.key = key(u8::try_from(index + 1).map_err(|_| Error::ArithmeticOverflow)?);
        meta.is_writable = matches!(index, 0 | 5 | 7 | 12..=23);
    }
    split[10].key = [0; 32];
    adapter::validate_account_frame_v2(AdapterActionV2::Split, 2, &split)?;
    split[23].key = split[17].key;
    adapter::validate_account_frame_v2(AdapterActionV2::Split, 2, &split)?;
    split[0].is_writable = false;
    assert_eq!(
        adapter::validate_account_frame_v2(AdapterActionV2::Split, 2, &split),
        Err(Error::InvalidAccountFrame)
    );

    let mut register = [adapter::AdapterAccountMetaV2 {
        key: [0; 32],
        is_signer: false,
        is_writable: false,
    }; 17];
    for (index, meta) in register.iter_mut().enumerate() {
        meta.key = key(u8::try_from(index + 40).map_err(|_| Error::ArithmeticOverflow)?);
        meta.is_signer = index == 0;
        meta.is_writable = matches!(index, 0 | 2 | 7..=9 | 11);
    }
    register[14].key = [0; 32];
    adapter::validate_account_frame_v2(AdapterActionV2::RegisterBuy, 1, &register)?;
    register[2].is_writable = false;
    assert_eq!(
        adapter::validate_account_frame_v2(AdapterActionV2::RegisterBuy, 1, &register),
        Err(Error::InvalidAccountFrame)
    );

    let mut inline = [adapter::AdapterAccountMetaV2 {
        key: [0; 32],
        is_signer: false,
        is_writable: false,
    }; 19];
    for (index, meta) in inline.iter_mut().enumerate() {
        meta.key = key(u8::try_from(index + 70).map_err(|_| Error::ArithmeticOverflow)?);
        meta.is_signer = index == 0;
        meta.is_writable = matches!(index, 0 | 2 | 7 | 13..=18);
    }
    inline[10].key = [0; 32];
    adapter::validate_account_frame_v2(AdapterActionV2::InlineOrdinary, 2, &inline)?;
    inline[2].is_writable = false;
    assert_eq!(
        adapter::validate_account_frame_v2(AdapterActionV2::InlineOrdinary, 2, &inline),
        Err(Error::InvalidAccountFrame)
    );

    let mut close_root = [adapter::AdapterAccountMetaV2 {
        key: [0; 32],
        is_signer: false,
        is_writable: false,
    }; 5];
    for (index, meta) in close_root.iter_mut().enumerate() {
        meta.key = key(u8::try_from(index + 100).map_err(|_| Error::ArithmeticOverflow)?);
        meta.is_writable = matches!(index, 0..=2);
    }
    close_root[3].key = [0; 32];
    adapter::validate_account_frame_v2(AdapterActionV2::CloseReplayRoot, 1, &close_root)?;
    close_root[0].is_writable = false;
    assert_eq!(
        adapter::validate_account_frame_v2(AdapterActionV2::CloseReplayRoot, 1, &close_root),
        Err(Error::InvalidAccountFrame)
    );

    let inline_envelope = adapter::measured_action_envelope_v2(AdapterActionV2::InlineOrdinary, 2)?;
    assert_eq!(inline_envelope.instruction_accounts, 19);
    assert_eq!(inline_envelope.total_account_locks, 21);
    assert_eq!(inline_envelope.serialized_transaction_bytes, 999);
    let split16 = adapter::measured_action_envelope_v2(AdapterActionV2::Split, 16)?;
    assert_eq!(split16.total_account_locks, 110);
    assert_eq!(split16.serialized_transaction_bytes, 621);
    let cancel_through = adapter::measured_action_envelope_v2(AdapterActionV2::CancelThrough, 1)?;
    assert_eq!(cancel_through.serialized_transaction_bytes, 501);
    assert_eq!(cancel_through.total_account_locks, 6);
    Ok(())
}

#[test]
fn maker_cancel_through_is_o1_and_permissionless_unwind_preserves_assets() -> Result<()> {
    let sell0 = intent(1, Side::Sell, 0, 0, PRICE_SCALE, 5, 0)?;
    let first = register(sell0, root(1)?, [5, 0])?;
    let sell1 = intent(1, Side::Sell, 1, 1, PRICE_SCALE, 5, 0)?;
    let second = register(sell1, first.replay_root, [0, 5])?;
    assert_eq!(second.replay_root.live_intent_count(), 2);

    assert_eq!(
        close_invalidated_intent_v1(InvalidatedCloseInputV1 {
            replay_root: second.replay_root,
            record: first.record,
            phase: adapter::MarketPhaseV2::Open,
            accounts: accounts(sell0),
            collateral_mint: None,
            escrow_authority: None,
            position: first.position,
        }),
        Err(Error::IntentNotInvalidated)
    );

    let message = CancelThroughV1::new(second.replay_root, 2)?;
    let instruction = adapter::encode_cancel_through_instruction_v1(message);
    assert_eq!(
        adapter::decode_cancel_through_instruction_v1(&instruction)?,
        message
    );
    let signed = message.signed_preimage();
    assert_eq!(
        cancel_through_v1(
            second.replay_root,
            message,
            authorization(key(2), &signed, &instruction, 16)?,
            adapter::MarketPhaseV2::Open,
        ),
        Err(Error::SignatureSignerMismatch)
    );
    let invalidated = cancel_through_v1(
        second.replay_root,
        message,
        authorization(key(1), &signed, &instruction, 16)?,
        adapter::MarketPhaseV2::Open,
    )?;
    assert_eq!(invalidated.minimum_live_nonce(), 2);
    assert_eq!(invalidated.live_intent_count(), 2);
    assert_eq!(
        cancel_through_v1(
            invalidated,
            message,
            authorization(key(1), &signed, &instruction, 16)?,
            adapter::MarketPhaseV2::Open,
        ),
        Err(Error::InvalidCancelThrough)
    );
    assert_eq!(
        CancelThroughV1::new(invalidated, 3),
        Err(Error::InvalidCancelThrough)
    );

    let bid = intent(2, Side::Buy, 0, 0, PRICE_SCALE, 5, 0)?;
    let buyer = register(bid, root(2)?, [0, 0])?;
    assert_eq!(
        settle_ordinary_v2(OrdinaryMatchV2 {
            phase: adapter::MarketPhaseV2::Open,
            slot: 12,
            seller_replay_root: invalidated,
            buyer_replay_root: buyer.replay_root,
            seller_record: first.record,
            buyer_record: buyer.record,
            seller_accounts: accounts(sell0),
            buyer_accounts: accounts(bid),
            seller_position: first.position,
            buyer_position: buyer.position,
            collateral_mint: key(6),
            buyer_escrow_authority: escrow_authority(bid),
            fill: 5,
            execution_price: PRICE_SCALE,
            fee_policy: policy(0)?,
            fee_config_digest: key(8),
            fee_recipient_account: key(99),
        }),
        Err(Error::IntentInvalidated)
    );

    let closed0 = close_invalidated_intent_v1(InvalidatedCloseInputV1 {
        replay_root: invalidated,
        record: first.record,
        phase: adapter::MarketPhaseV2::Resolved,
        accounts: accounts(sell0),
        collateral_mint: None,
        escrow_authority: None,
        position: first.position,
    })?;
    let close_sell_instruction = adapter::encode_close_invalidated_instruction_v1(Side::Sell);
    adapter::decode_close_invalidated_instruction_v1(&close_sell_instruction, Side::Sell)?;
    assert_eq!(
        adapter::decode_close_invalidated_instruction_v1(&close_sell_instruction, Side::Buy),
        Err(Error::UnknownAdapterAction)
    );
    assert_eq!(closed0.close.claim_refund, 5);
    assert_eq!(closed0.position.balances(), &[5, 0]);
    let closed1 = close_invalidated_intent_v1(InvalidatedCloseInputV1 {
        replay_root: closed0.replay_root,
        record: second.record,
        phase: adapter::MarketPhaseV2::Retiring,
        accounts: accounts(sell1),
        collateral_mint: None,
        escrow_authority: None,
        position: second.position,
    })?;
    assert_eq!(closed1.close.claim_refund, 5);
    assert_eq!(closed1.position.balances(), &[0, 5]);
    assert_eq!(closed1.replay_root.live_intent_count(), 0);

    let buy = intent(3, Side::Buy, 0, 0, PRICE_SCALE, 1, 0)?;
    let registered_buy = register(buy, root(3)?, [0, 0])?;
    let buy_message = CancelThroughV1::new(registered_buy.replay_root, 1)?;
    let buy_instruction = adapter::encode_cancel_through_instruction_v1(buy_message);
    let buy_signed = buy_message.signed_preimage();
    let buy_root = cancel_through_v1(
        registered_buy.replay_root,
        buy_message,
        authorization(key(3), &buy_signed, &buy_instruction, 16)?,
        adapter::MarketPhaseV2::Resolved,
    )?;
    let closed_buy = close_invalidated_intent_v1(InvalidatedCloseInputV1 {
        replay_root: buy_root,
        record: registered_buy.record,
        phase: adapter::MarketPhaseV2::Resolved,
        accounts: accounts(buy),
        collateral_mint: Some(key(6)),
        escrow_authority: Some(escrow_authority(buy)),
        position: registered_buy.position,
    })?;
    assert_eq!(closed_buy.close.collateral_refund, 1);
    assert_eq!(closed_buy.close.rent_refund_payer, key(233));
    Ok(())
}
