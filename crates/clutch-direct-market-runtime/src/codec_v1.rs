// SPDX-License-Identifier: AGPL-3.0-or-later

//! Canonical fixed-width semantic bodies for Direct `0xb1..=0xb4/v1`.
//!
//! These bodies exclude the four-byte Solana frame (`tag`, `version`, PDA
//! bump, reserved zero). `clutch-solana-layout` owns that physical frame; this
//! crate remains the sole interpreter of every economic field.

use clutch_batch::direct_pair_v1::{
    authenticate_compact_selected_direct_pair_v1, AuthenticatedDirectSelectionAuthorityV1,
    DirectEconomicBookV1, DirectEconomicCandidateV1, DirectPairErrorV1,
};
use clutch_batch::relation_v2::{
    EconomicDomainV2, EconomicOrderV2, PricePreconditionV2, VerifiedEconomicsV2,
    EMPTY_ECONOMIC_ORDER_V2,
};
use clutch_batch::{PartialPolicy, Side};

use crate::reservation_v1::{DirectReservationPhaseV1, DirectReservationV1};
use crate::selection_v1::{DirectSelectionPhaseV1, DirectSelectionV1};
use crate::liveness_v1::{
    DirectCandidateLivenessBindingV1, DirectCandidateWorkScheduleV1,
};
use crate::{
    DirectActionReplayV1, DirectMarketBindingV1, DirectMarketErrorV1, DirectMarketRootV1,
    DirectRentOwnerV1, DirectReplayPhaseV1, DirectRootPhaseV1, DirectScheduleV1,
    DirectTerminalReasonV1,
};

/// Exact semantic bytes inside the `0xb1/1` frame.
pub const DIRECT_MARKET_ROOT_BODY_BYTES_V1: usize = 1_722;
/// Exact semantic bytes inside the `0xb2/1` frame.
pub const DIRECT_SELECTION_BODY_BYTES_V1: usize = 1_625;
/// Exact semantic bytes inside the `0xb3/1` frame.
pub const DIRECT_ACTION_REPLAY_BODY_BYTES_V1: usize = 390;
/// Exact semantic bytes inside the `0xb4/1` frame.
pub const DIRECT_RESERVATION_BODY_BYTES_V1: usize = 469;

/// Encode the sole canonical Direct root body.
pub fn encode_direct_market_root_body_v1(
    value: DirectMarketRootV1,
) -> Result<[u8; DIRECT_MARKET_ROOT_BODY_BYTES_V1], DirectMarketErrorV1> {
    value.validate()?;
    let mut output = [0u8; DIRECT_MARKET_ROOT_BODY_BYTES_V1];
    let mut writer = BodyWriter::new(&mut output);
    write_binding(&mut writer, value.binding)?;
    write_schedule(&mut writer, value.schedule)?;
    write_rent(&mut writer, value.root_rent)?;
    writer.u8(root_phase_byte(value.phase))?;
    writer.u8(value.terminal_reason.map_or(0, terminal_reason_byte))?;
    writer.u8(value.admitted_reservations)?;
    writer.u8(value.live_reservations)?;
    writer.u8(value.retired_reservations)?;
    writer.id(value.reservation_accounts[0])?;
    writer.id(value.reservation_accounts[1])?;
    writer.id(value.reservation_semantic_ids[0])?;
    writer.id(value.reservation_semantic_ids[1])?;
    writer.id(value.selection_account)?;
    writer.finish()?;
    Ok(output)
}

/// Decode and validate one hostile Direct root semantic body.
pub fn decode_direct_market_root_body_v1(
    input: &[u8; DIRECT_MARKET_ROOT_BODY_BYTES_V1],
) -> Result<DirectMarketRootV1, DirectMarketErrorV1> {
    let mut reader = BodyReader::new(input);
    let value = DirectMarketRootV1 {
        binding: read_binding(&mut reader)?,
        schedule: read_schedule(&mut reader)?,
        root_rent: read_rent(&mut reader)?,
        phase: decode_root_phase(reader.u8()?)?,
        terminal_reason: decode_terminal_reason(reader.u8()?)?,
        admitted_reservations: reader.u8()?,
        live_reservations: reader.u8()?,
        retired_reservations: reader.u8()?,
        reservation_accounts: [reader.id()?, reader.id()?],
        reservation_semantic_ids: [reader.id()?, reader.id()?],
        selection_account: reader.id()?,
    };
    reader.finish()?;
    value.validate()?;
    Ok(value)
}

/// Encode the permanent `0xb3/1` replay body.
pub fn encode_direct_action_replay_body_v1(
    value: DirectActionReplayV1,
    root: DirectMarketRootV1,
) -> Result<[u8; DIRECT_ACTION_REPLAY_BODY_BYTES_V1], DirectMarketErrorV1> {
    let mut output = [0u8; DIRECT_ACTION_REPLAY_BODY_BYTES_V1];
    encode_direct_action_replay_body_into_v1(value, root, &mut output)?;
    Ok(output)
}

/// Encode the permanent replay directly into exact caller-owned storage.
pub fn encode_direct_action_replay_body_into_v1(
    value: DirectActionReplayV1,
    root: DirectMarketRootV1,
    output: &mut [u8],
) -> Result<(), DirectMarketErrorV1> {
    value.validate_against(root)?;
    if value.candidate_liveness_pending {
        return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
    }
    if output.len() != DIRECT_ACTION_REPLAY_BODY_BYTES_V1 {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let mut writer = BodyWriter::new(output);
    writer.id(value.market_instance_id)?;
    writer.u64(value.generation)?;
    writer.id(value.direct_epoch_semantics_id)?;
    writer.id(value.direct_root_account)?;
    writer.id(value.replay_account)?;
    write_rent(&mut writer, value.rent)?;
    writer.u8(replay_phase_byte(value.phase))?;
    writer.u64(value.next_action_sequence)?;
    writer.id(value.action_transcript_id)?;
    writer.id(value.foundation_receipt_id)?;
    writer.id(value.economic_terminal_receipt_id)?;
    writer.id(value.family_terminal_receipt_id)?;
    writer.u32(value.candidate_liveness_completed_calls)?;
    writer.id(value.candidate_liveness_last_receipt_id)?;
    writer.id(value.candidate_liveness_batch_receipt_id)?;
    writer.u8(if value.candidate_liveness_pending { 1 } else { 0 })?;
    writer.finish()
}

/// Decode and validate one hostile permanent replay body.
pub fn decode_direct_action_replay_body_v1(
    input: &[u8; DIRECT_ACTION_REPLAY_BODY_BYTES_V1],
    root: DirectMarketRootV1,
) -> Result<DirectActionReplayV1, DirectMarketErrorV1> {
    let mut reader = BodyReader::new(input);
    let value = DirectActionReplayV1 {
        market_instance_id: reader.id()?,
        generation: reader.u64()?,
        direct_epoch_semantics_id: reader.id()?,
        direct_root_account: reader.id()?,
        replay_account: reader.id()?,
        rent: read_rent(&mut reader)?,
        phase: decode_replay_phase(reader.u8()?)?,
        next_action_sequence: reader.u64()?,
        action_transcript_id: reader.id()?,
        foundation_receipt_id: reader.id()?,
        economic_terminal_receipt_id: reader.id()?,
        family_terminal_receipt_id: reader.id()?,
        candidate_liveness_completed_calls: reader.u32()?,
        candidate_liveness_last_receipt_id: reader.id()?,
        candidate_liveness_batch_receipt_id: reader.id()?,
        candidate_liveness_pending: match reader.u8()? {
            0 => false,
            1 => true,
            _ => return Err(DirectMarketErrorV1::InvalidCount),
        },
    };
    reader.finish()?;
    if value.candidate_liveness_pending {
        return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
    }
    value.validate_against(root)?;
    Ok(value)
}

/// Encode one exact funded or terminal Reservation body.
pub fn encode_direct_reservation_body_v1(
    value: DirectReservationV1,
    root: DirectMarketRootV1,
) -> Result<[u8; DIRECT_RESERVATION_BODY_BYTES_V1], DirectMarketErrorV1> {
    let mut output = [0u8; DIRECT_RESERVATION_BODY_BYTES_V1];
    encode_direct_reservation_body_into_v1(value, root, &mut output)?;
    Ok(output)
}

/// Encode one Reservation directly into exact caller-owned storage.
pub fn encode_direct_reservation_body_into_v1(
    value: DirectReservationV1,
    root: DirectMarketRootV1,
    output: &mut [u8],
) -> Result<(), DirectMarketErrorV1> {
    value.validate_against_root(root)?;
    if output.len() != DIRECT_RESERVATION_BODY_BYTES_V1 {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let mut writer = BodyWriter::new(output);
    writer.id(value.market_instance_id)?;
    writer.u64(value.generation)?;
    writer.id(value.direct_epoch_semantics_id)?;
    writer.id(value.direct_root_account)?;
    writer.id(value.reservation_account)?;
    writer.id(value.general_market_runtime)?;
    writer.id(value.owner)?;
    writer.id(value.position_account)?;
    writer.id(value.position_replay_account)?;
    writer.u64(value.position_generation)?;
    writer.id(value.order_id)?;
    writer.u8(side_byte(value.side))?;
    writer.u8(value.outcome)?;
    writer.u8(value.outcome_count)?;
    writer.u64(value.quantity)?;
    writer.u64(value.minimum_fill)?;
    writer.u8(partial_policy_byte(value.partial_policy))?;
    writer.u64(value.expiry_epoch)?;
    writer.u128(value.limit_price_units_per_egg)?;
    writer.u64(value.price_scale)?;
    writer.u64(value.reserved_cash_atoms)?;
    writer.u64(value.reserved_eggs)?;
    writer.u64(value.maximum_fee_atoms)?;
    writer.u64(value.charged_fee_atoms)?;
    write_rent(&mut writer, value.rent)?;
    writer.u8(reservation_phase_byte(value.phase))?;
    writer.id(value.terminal_receipt_id)?;
    writer.finish()
}

/// Decode and validate one hostile Reservation body.
pub fn decode_direct_reservation_body_v1(
    input: &[u8; DIRECT_RESERVATION_BODY_BYTES_V1],
    root: DirectMarketRootV1,
) -> Result<DirectReservationV1, DirectMarketErrorV1> {
    let mut reader = BodyReader::new(input);
    let value = DirectReservationV1 {
        market_instance_id: reader.id()?,
        generation: reader.u64()?,
        direct_epoch_semantics_id: reader.id()?,
        direct_root_account: reader.id()?,
        reservation_account: reader.id()?,
        general_market_runtime: reader.id()?,
        owner: reader.id()?,
        position_account: reader.id()?,
        position_replay_account: reader.id()?,
        position_generation: reader.u64()?,
        order_id: reader.id()?,
        side: decode_side(reader.u8()?)?,
        outcome: reader.u8()?,
        outcome_count: reader.u8()?,
        quantity: reader.u64()?,
        minimum_fill: reader.u64()?,
        partial_policy: decode_partial_policy(reader.u8()?)?,
        expiry_epoch: reader.u64()?,
        limit_price_units_per_egg: reader.u128()?,
        price_scale: reader.u64()?,
        reserved_cash_atoms: reader.u64()?,
        reserved_eggs: reader.u64()?,
        maximum_fee_atoms: reader.u64()?,
        charged_fee_atoms: reader.u64()?,
        rent: read_rent(&mut reader)?,
        phase: decode_reservation_phase(reader.u8()?)?,
        terminal_receipt_id: reader.id()?,
    };
    reader.finish()?;
    value.validate_against_root(root)?;
    Ok(value)
}

/// Encode the complete fixed-capacity Selection body.
pub fn encode_direct_selection_body_v1(
    value: DirectSelectionV1,
    root: DirectMarketRootV1,
) -> Result<[u8; DIRECT_SELECTION_BODY_BYTES_V1], DirectMarketErrorV1> {
    let mut output = [0u8; DIRECT_SELECTION_BODY_BYTES_V1];
    encode_direct_selection_body_into_v1(value, root, &mut output)?;
    Ok(output)
}

/// Encode one complete Selection directly into exact caller-owned storage.
pub fn encode_direct_selection_body_into_v1(
    value: DirectSelectionV1,
    root: DirectMarketRootV1,
    output: &mut [u8],
) -> Result<(), DirectMarketErrorV1> {
    value.validate_against(root)?;
    if output.len() != DIRECT_SELECTION_BODY_BYTES_V1 {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let mut writer = BodyWriter::new(output);
    writer.id(value.market_instance_id)?;
    writer.u64(value.generation)?;
    writer.id(value.direct_root_account)?;
    writer.id(value.selection_account)?;
    writer.id(value.reservation_accounts[0])?;
    writer.id(value.reservation_accounts[1])?;
    writer.id(value.reservation_semantic_ids[0])?;
    writer.id(value.reservation_semantic_ids[1])?;
    writer.u8(value.reservation_count)?;
    write_domain(&mut writer, &value.domain)?;
    write_order(&mut writer, value.book.orders[0])?;
    write_order(&mut writer, value.book.orders[1])?;
    writer.u8(value.book.len)?;
    write_price(&mut writer, &value.price)?;
    let mut index = 0usize;
    while index < 3 {
        write_candidate(&mut writer, value.candidates[index])?;
        index += 1;
    }
    index = 0;
    while index < 3 {
        writer.id(value.candidate_digests[index])?;
        index += 1;
    }
    index = 0;
    while index < 3 {
        writer.id(value.candidate_submitters[index])?;
        index += 1;
    }
    writer.u8(value.candidate_count)?;
    writer.u8(value.verification_cursor)?;
    writer.u8(value.verified_mask)?;
    writer.id(value.traversal_transcript_id)?;
    match (value.selected_candidate_index, value.selected_pair) {
        (None, None) => {
            writer.u8(0)?;
            writer.u8(0)?;
            writer.bytes(&[0; 253])?;
        }
        (Some(selected_index), Some(pair)) => {
            writer.u8(1)?;
            writer.u8(selected_index)?;
            writer.bytes(&pair.canonical_transcript())?;
        }
        _ => return Err(DirectMarketErrorV1::InvalidCount),
    }
    writer.id(value.terminal_receipt_id)?;
    writer.id(value.candidate_bond_refund_receipt_id)?;
    write_rent(&mut writer, value.rent)?;
    writer.u8(selection_phase_byte(value.phase))?;
    writer.finish()
}

/// Decode, reverify, and validate one hostile Selection body.
pub fn decode_direct_selection_body_v1(
    input: &[u8; DIRECT_SELECTION_BODY_BYTES_V1],
    root: DirectMarketRootV1,
) -> Result<DirectSelectionV1, DirectMarketErrorV1> {
    let mut reader = BodyReader::new(input);
    let market_instance_id = reader.id()?;
    let generation = reader.u64()?;
    let direct_root_account = reader.id()?;
    let selection_account = reader.id()?;
    let reservation_accounts = [reader.id()?, reader.id()?];
    let reservation_semantic_ids = [reader.id()?, reader.id()?];
    let reservation_count = reader.u8()?;
    let domain = read_domain(&mut reader)?;
    let book = DirectEconomicBookV1 {
        orders: [read_order(&mut reader)?, read_order(&mut reader)?],
        len: reader.u8()?,
    };
    let price = read_price(&mut reader)?;
    let candidates = [
        read_candidate(&mut reader)?,
        read_candidate(&mut reader)?,
        read_candidate(&mut reader)?,
    ];
    let candidate_digests = [reader.id()?, reader.id()?, reader.id()?];
    let candidate_submitters = [reader.id()?, reader.id()?, reader.id()?];
    let candidate_count = reader.u8()?;
    let verification_cursor = reader.u8()?;
    let verified_mask = reader.u8()?;
    let traversal_transcript_id = reader.id()?;
    let selected_present = reader.u8()?;
    let selected_index_byte = reader.u8()?;
    let mut selected_transcript = [0u8; 253];
    reader.fill(&mut selected_transcript)?;
    let (selected_candidate_index, selected_pair) = match selected_present {
        0 if selected_index_byte == 0 && selected_transcript == [0; 253] => (None, None),
        1 => {
            let at = usize::from(selected_index_byte);
            if at >= usize::from(candidate_count) || at >= 3 {
                return Err(DirectMarketErrorV1::InvalidCount);
            }
            let authority = CodecSelectionAuthorityV1 {
                traversal_transcript_id,
                candidate_digest: candidate_digests[at],
                price_digest: price.semantic_price_digest,
            };
            let pair = authenticate_compact_selected_direct_pair_v1(
                &authority,
                traversal_transcript_id,
                &domain,
                &book,
                &price,
                candidates[at],
            )?;
            if pair.canonical_transcript() != selected_transcript {
                return Err(DirectMarketErrorV1::MismatchedBinding);
            }
            (Some(selected_index_byte), Some(pair))
        }
        _ => return Err(DirectMarketErrorV1::InvalidCount),
    };
    let value = DirectSelectionV1 {
        market_instance_id,
        generation,
        direct_root_account,
        selection_account,
        reservation_accounts,
        reservation_semantic_ids,
        reservation_count,
        domain,
        book,
        price,
        candidates,
        candidate_digests,
        candidate_submitters,
        candidate_count,
        verification_cursor,
        verified_mask,
        traversal_transcript_id,
        selected_candidate_index,
        selected_pair,
        terminal_receipt_id: reader.id()?,
        candidate_bond_refund_receipt_id: reader.id()?,
        rent: read_rent(&mut reader)?,
        phase: decode_selection_phase(reader.u8()?)?,
    };
    reader.finish()?;
    value.validate_against(root)?;
    Ok(value)
}

#[derive(Clone, Copy, Debug)]
struct CodecSelectionAuthorityV1 {
    traversal_transcript_id: [u8; 32],
    candidate_digest: [u8; 32],
    price_digest: [u8; 32],
}

impl AuthenticatedDirectSelectionAuthorityV1 for CodecSelectionAuthorityV1 {
    fn authenticate_compact_selected_pair(
        &self,
        selection_transcript_id: [u8; 32],
        _domain: &EconomicDomainV2,
        _orders: &[EconomicOrderV2; 2],
        price: &PricePreconditionV2,
        _candidate: DirectEconomicCandidateV1,
        economics: &VerifiedEconomicsV2,
    ) -> Result<(), DirectPairErrorV1> {
        if selection_transcript_id == self.traversal_transcript_id
            && economics.economic_candidate_digest == self.candidate_digest
            && price.semantic_price_digest == self.price_digest
        {
            Ok(())
        } else {
            Err(DirectPairErrorV1::UnauthenticatedSelection)
        }
    }
}

fn write_binding(
    writer: &mut BodyWriter<'_>,
    value: DirectMarketBindingV1,
) -> Result<(), DirectMarketErrorV1> {
    writer.id(value.market_instance_id)?;
    writer.u64(value.generation)?;
    writer.u8(value.outcome_count)?;
    writer.id(value.realm_id)?;
    writer.id(value.collateral_profile_id)?;
    writer.id(value.collateral_policy_id)?;
    writer.id(value.collateral_release_id)?;
    writer.id(value.resolution_account)?;
    writer.id(value.direct_epoch_semantics_id)?;
    writer.id(value.revenue_policy_id)?;
    writer.id(value.batch_policy_id)?;
    writer.id(value.direct_fee_shape_id)?;
    writer.id(value.fee_treasury_owner)?;
    writer.u32(value.fee_dispersion_bps)?;
    writer.u32(value.fee_floor_range_bps)?;
    writer.u32(value.fee_maker_rebate_num)?;
    writer.u32(value.fee_treasury_num)?;
    writer.u32(value.fee_split_den)?;
    writer.id(value.candidate_lifecycle_policy_id)?;
    writer.id(value.candidate_liveness_policy_id)?;
    write_candidate_liveness(&mut *writer, value.candidate_liveness)?;
    writer.id(value.direct_schedule_policy_id)?;
    writer.id(value.product_root_account)?;
    writer.id(value.product_market_binding_id)?;
    writer.id(value.product_family_prestate_id)?;
    writer.id(value.general_product_preauthorization_id)?;
    writer.u32(value.family_admission_sequence)?;
    writer.id(value.founder_series_link_account)?;
    writer.id(value.founder_series_link_binding_id)?;
    writer.id(value.compiler_bundle_v5_id)?;
    writer.id(value.founder_series_plan_id)?;
    writer.u32(value.founder_series_ordinal)?;
    writer.id(value.direct_root_account)?;
    writer.id(value.action_replay_account)?;
    writer.id(value.general_market_binding)?;
    writer.id(value.general_market_runtime)?;
    writer.id(value.neutral_lamport_sink)?;
    writer.id(value.relation_policy_id)?;
    writer.id(value.price_policy_id)?;
    writer.u64(value.price_scale)
}

fn read_binding(reader: &mut BodyReader<'_>) -> Result<DirectMarketBindingV1, DirectMarketErrorV1> {
    Ok(DirectMarketBindingV1 {
        market_instance_id: reader.id()?,
        generation: reader.u64()?,
        outcome_count: reader.u8()?,
        realm_id: reader.id()?,
        collateral_profile_id: reader.id()?,
        collateral_policy_id: reader.id()?,
        collateral_release_id: reader.id()?,
        resolution_account: reader.id()?,
        direct_epoch_semantics_id: reader.id()?,
        revenue_policy_id: reader.id()?,
        batch_policy_id: reader.id()?,
        direct_fee_shape_id: reader.id()?,
        fee_treasury_owner: reader.id()?,
        fee_dispersion_bps: reader.u32()?,
        fee_floor_range_bps: reader.u32()?,
        fee_maker_rebate_num: reader.u32()?,
        fee_treasury_num: reader.u32()?,
        fee_split_den: reader.u32()?,
        candidate_lifecycle_policy_id: reader.id()?,
        candidate_liveness_policy_id: reader.id()?,
        candidate_liveness: read_candidate_liveness(&mut *reader)?,
        direct_schedule_policy_id: reader.id()?,
        product_root_account: reader.id()?,
        product_market_binding_id: reader.id()?,
        product_family_prestate_id: reader.id()?,
        general_product_preauthorization_id: reader.id()?,
        family_admission_sequence: reader.u32()?,
        founder_series_link_account: reader.id()?,
        founder_series_link_binding_id: reader.id()?,
        compiler_bundle_v5_id: reader.id()?,
        founder_series_plan_id: reader.id()?,
        founder_series_ordinal: reader.u32()?,
        direct_root_account: reader.id()?,
        action_replay_account: reader.id()?,
        general_market_binding: reader.id()?,
        general_market_runtime: reader.id()?,
        neutral_lamport_sink: reader.id()?,
        relation_policy_id: reader.id()?,
        price_policy_id: reader.id()?,
        price_scale: reader.u64()?,
    })
}

fn write_candidate_liveness(
    writer: &mut BodyWriter<'_>,
    value: DirectCandidateLivenessBindingV1,
) -> Result<(), DirectMarketErrorV1> {
    value.validate()?;
    writer.id(value.policy_account)?;
    writer.id(value.policy_data_id)?;
    writer.id(value.global_lifecycle_id)?;
    writer.id(value.global_bundle_binding_id)?;
    writer.id(value.global_capitalization_receipt_id)?;
    writer.id(value.global_bundle_commitment_id)?;
    writer.id(value.candidate_account)?;
    writer.id(value.candidate_data_id)?;
    writer.id(value.candidate_semantic_owner)?;
    writer.id(value.candidate_quote_schedule_id)?;
    writer.id(value.candidate_receipt_program_id)?;
    writer.u64(value.candidate_generation)?;
    writer.u32(value.first_call_ordinal)?;
    writer.u32(value.reserved_calls)?;
    writer.u64(value.reserved_work_lamports)?;
    writer.id(value.allocation_receipt_id)?;
    writer.u64(value.work_schedule.freeze_book_lamports)?;
    writer.u64(value.work_schedule.begin_verification_lamports)?;
    writer.u64(value.work_schedule.verify_candidate_lamports)?;
    writer.u64(value.work_schedule.finalize_selection_lamports)?;
    writer.u64(value.work_schedule.economic_terminal_lamports)?;
    writer.u64(value.work_schedule.retire_terminal_lamports)?;
    writer.u64(value.work_schedule.retained_candidate_bond_lamports)?;
    writer.id(value.work_schedule_id)
}

fn read_candidate_liveness(
    reader: &mut BodyReader<'_>,
) -> Result<DirectCandidateLivenessBindingV1, DirectMarketErrorV1> {
    let value = DirectCandidateLivenessBindingV1 {
        policy_account: reader.id()?,
        policy_data_id: reader.id()?,
        global_lifecycle_id: reader.id()?,
        global_bundle_binding_id: reader.id()?,
        global_capitalization_receipt_id: reader.id()?,
        global_bundle_commitment_id: reader.id()?,
        candidate_account: reader.id()?,
        candidate_data_id: reader.id()?,
        candidate_semantic_owner: reader.id()?,
        candidate_quote_schedule_id: reader.id()?,
        candidate_receipt_program_id: reader.id()?,
        candidate_generation: reader.u64()?,
        first_call_ordinal: reader.u32()?,
        reserved_calls: reader.u32()?,
        reserved_work_lamports: reader.u64()?,
        allocation_receipt_id: reader.id()?,
        work_schedule: DirectCandidateWorkScheduleV1 {
            freeze_book_lamports: reader.u64()?,
            begin_verification_lamports: reader.u64()?,
            verify_candidate_lamports: reader.u64()?,
            finalize_selection_lamports: reader.u64()?,
            economic_terminal_lamports: reader.u64()?,
            retire_terminal_lamports: reader.u64()?,
            retained_candidate_bond_lamports: reader.u64()?,
        },
        work_schedule_id: reader.id()?,
    };
    value.validate()?;
    Ok(value)
}

fn write_schedule(
    writer: &mut BodyWriter<'_>,
    value: DirectScheduleV1,
) -> Result<(), DirectMarketErrorV1> {
    writer.u64(value.admission_opens_slot)?;
    writer.u64(value.admission_closes_slot)?;
    writer.u64(value.submission_closes_slot)?;
    writer.u64(value.selection_deadline_slot)?;
    writer.u64(value.settlement_deadline_slot)
}

fn read_schedule(reader: &mut BodyReader<'_>) -> Result<DirectScheduleV1, DirectMarketErrorV1> {
    Ok(DirectScheduleV1 {
        admission_opens_slot: reader.u64()?,
        admission_closes_slot: reader.u64()?,
        submission_closes_slot: reader.u64()?,
        selection_deadline_slot: reader.u64()?,
        settlement_deadline_slot: reader.u64()?,
    })
}

fn write_rent(
    writer: &mut BodyWriter<'_>,
    value: DirectRentOwnerV1,
) -> Result<(), DirectMarketErrorV1> {
    writer.id(value.payer)?;
    writer.u64(value.principal_lamports)?;
    writer.u64(value.donation_floor_lamports)
}

fn read_rent(reader: &mut BodyReader<'_>) -> Result<DirectRentOwnerV1, DirectMarketErrorV1> {
    Ok(DirectRentOwnerV1 {
        payer: reader.id()?,
        principal_lamports: reader.u64()?,
        donation_floor_lamports: reader.u64()?,
    })
}

fn write_domain(
    writer: &mut BodyWriter<'_>,
    value: &EconomicDomainV2,
) -> Result<(), DirectMarketErrorV1> {
    writer.u32(value.relation_version)?;
    writer.id(value.market_semantics_digest)?;
    writer.id(value.epoch_semantics_digest)?;
    writer.id(value.relation_policy_digest)?;
    writer.id(value.price_policy_digest)?;
    writer.u64(value.epoch_index)?;
    writer.u8(value.outcome_count)?;
    writer.u64(value.price_scale)
}

fn read_domain(reader: &mut BodyReader<'_>) -> Result<EconomicDomainV2, DirectMarketErrorV1> {
    Ok(EconomicDomainV2 {
        relation_version: reader.u32()?,
        market_semantics_digest: reader.id()?,
        epoch_semantics_digest: reader.id()?,
        relation_policy_digest: reader.id()?,
        price_policy_digest: reader.id()?,
        epoch_index: reader.u64()?,
        outcome_count: reader.u8()?,
        price_scale: reader.u64()?,
    })
}

fn write_order(
    writer: &mut BodyWriter<'_>,
    value: EconomicOrderV2,
) -> Result<(), DirectMarketErrorV1> {
    writer.id(value.order_id)?;
    writer.u8(side_byte(value.side))?;
    let mut outcome = 0usize;
    while outcome < 16 {
        writer.u64(value.coefficients[outcome])?;
        outcome += 1;
    }
    writer.u64(value.quantity)?;
    writer.u64(value.minimum_fill)?;
    writer.u8(partial_policy_byte(value.partial_policy))?;
    writer.u64(value.expiry_epoch)?;
    writer.u128(value.limit_value_price_units_per_unit)
}

fn read_order(reader: &mut BodyReader<'_>) -> Result<EconomicOrderV2, DirectMarketErrorV1> {
    let order_id = reader.id()?;
    let side = decode_side(reader.u8()?)?;
    let mut coefficients = [0u64; 16];
    let mut outcome = 0usize;
    while outcome < 16 {
        coefficients[outcome] = reader.u64()?;
        outcome += 1;
    }
    Ok(EconomicOrderV2 {
        order_id,
        side,
        coefficients,
        quantity: reader.u64()?,
        minimum_fill: reader.u64()?,
        partial_policy: decode_partial_policy(reader.u8()?)?,
        expiry_epoch: reader.u64()?,
        limit_value_price_units_per_unit: reader.u128()?,
    })
}

fn write_price(
    writer: &mut BodyWriter<'_>,
    value: &PricePreconditionV2,
) -> Result<(), DirectMarketErrorV1> {
    writer.id(value.policy_digest)?;
    writer.id(value.semantic_price_digest)?;
    let mut outcome = 0usize;
    while outcome < 16 {
        writer.u64(value.prices[outcome])?;
        outcome += 1;
    }
    Ok(())
}

fn read_price(reader: &mut BodyReader<'_>) -> Result<PricePreconditionV2, DirectMarketErrorV1> {
    let policy_digest = reader.id()?;
    let semantic_price_digest = reader.id()?;
    let mut prices = [0u64; 16];
    let mut outcome = 0usize;
    while outcome < 16 {
        prices[outcome] = reader.u64()?;
        outcome += 1;
    }
    Ok(PricePreconditionV2 {
        policy_digest,
        semantic_price_digest,
        prices,
    })
}

fn write_candidate(
    writer: &mut BodyWriter<'_>,
    value: DirectEconomicCandidateV1,
) -> Result<(), DirectMarketErrorV1> {
    writer.u64(value.fills[0])?;
    writer.u64(value.fills[1])?;
    writer.u8(value.honored_aon_mask)
}

fn read_candidate(
    reader: &mut BodyReader<'_>,
) -> Result<DirectEconomicCandidateV1, DirectMarketErrorV1> {
    Ok(DirectEconomicCandidateV1 {
        fills: [reader.u64()?, reader.u64()?],
        honored_aon_mask: reader.u8()?,
    })
}

const fn root_phase_byte(value: DirectRootPhaseV1) -> u8 { value.byte() }
const fn replay_phase_byte(value: DirectReplayPhaseV1) -> u8 { value.byte() }
const fn terminal_reason_byte(value: DirectTerminalReasonV1) -> u8 { value.byte() }
const fn reservation_phase_byte(value: DirectReservationPhaseV1) -> u8 { value.byte() }
const fn selection_phase_byte(value: DirectSelectionPhaseV1) -> u8 { value.byte() }

const fn side_byte(value: Side) -> u8 {
    match value { Side::Buy => 1, Side::Sell => 2 }
}

const fn partial_policy_byte(value: PartialPolicy) -> u8 {
    match value { PartialPolicy::Allow => 1, PartialPolicy::AllOrNone => 2 }
}

fn decode_root_phase(value: u8) -> Result<DirectRootPhaseV1, DirectMarketErrorV1> {
    match value {
        1 => Ok(DirectRootPhaseV1::Open),
        2 => Ok(DirectRootPhaseV1::FrozenEmpty),
        3 => Ok(DirectRootPhaseV1::SubmissionOpen),
        4 => Ok(DirectRootPhaseV1::Verifying),
        5 => Ok(DirectRootPhaseV1::Selected),
        6 => Ok(DirectRootPhaseV1::Terminal),
        _ => Err(DirectMarketErrorV1::WrongPhase),
    }
}

fn decode_replay_phase(value: u8) -> Result<DirectReplayPhaseV1, DirectMarketErrorV1> {
    match value {
        1 => Ok(DirectReplayPhaseV1::Active),
        2 => Ok(DirectReplayPhaseV1::Terminal),
        _ => Err(DirectMarketErrorV1::WrongPhase),
    }
}

fn decode_terminal_reason(
    value: u8,
) -> Result<Option<DirectTerminalReasonV1>, DirectMarketErrorV1> {
    match value {
        0 => Ok(None),
        1 => Ok(Some(DirectTerminalReasonV1::EmptyLapse)),
        2 => Ok(Some(DirectTerminalReasonV1::UnselectedLapse)),
        3 => Ok(Some(DirectTerminalReasonV1::SelectedLapse)),
        4 => Ok(Some(DirectTerminalReasonV1::Settled)),
        5 => Ok(Some(DirectTerminalReasonV1::MissedFreezeLapse)),
        6 => Ok(Some(DirectTerminalReasonV1::NoCandidate)),
        _ => Err(DirectMarketErrorV1::WrongPhase),
    }
}

fn decode_reservation_phase(value: u8) -> Result<DirectReservationPhaseV1, DirectMarketErrorV1> {
    match value {
        1 => Ok(DirectReservationPhaseV1::Active),
        2 => Ok(DirectReservationPhaseV1::Cancelled),
        3 => Ok(DirectReservationPhaseV1::Settled),
        4 => Ok(DirectReservationPhaseV1::Lapsed),
        _ => Err(DirectMarketErrorV1::WrongPhase),
    }
}

fn decode_selection_phase(value: u8) -> Result<DirectSelectionPhaseV1, DirectMarketErrorV1> {
    match value {
        1 => Ok(DirectSelectionPhaseV1::FrozenEmpty),
        2 => Ok(DirectSelectionPhaseV1::SubmissionOpen),
        3 => Ok(DirectSelectionPhaseV1::Verifying),
        4 => Ok(DirectSelectionPhaseV1::Selected),
        5 => Ok(DirectSelectionPhaseV1::Terminal),
        _ => Err(DirectMarketErrorV1::WrongPhase),
    }
}

fn decode_side(value: u8) -> Result<Side, DirectMarketErrorV1> {
    match value {
        1 => Ok(Side::Buy),
        2 => Ok(Side::Sell),
        _ => Err(DirectMarketErrorV1::MismatchedBinding),
    }
}

fn decode_partial_policy(value: u8) -> Result<PartialPolicy, DirectMarketErrorV1> {
    match value {
        1 => Ok(PartialPolicy::Allow),
        2 => Ok(PartialPolicy::AllOrNone),
        _ => Err(DirectMarketErrorV1::MismatchedBinding),
    }
}

struct BodyWriter<'a> {
    output: &'a mut [u8],
    at: usize,
}

impl<'a> BodyWriter<'a> {
    fn new(output: &'a mut [u8]) -> Self { Self { output, at: 0 } }

    fn bytes(&mut self, value: &[u8]) -> Result<(), DirectMarketErrorV1> {
        let end = self.at.checked_add(value.len()).ok_or(DirectMarketErrorV1::Arithmetic)?;
        let destination = self.output.get_mut(self.at..end)
            .ok_or(DirectMarketErrorV1::InvalidCount)?;
        destination.copy_from_slice(value);
        self.at = end;
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), DirectMarketErrorV1> { self.bytes(&[value]) }
    fn u32(&mut self, value: u32) -> Result<(), DirectMarketErrorV1> {
        self.bytes(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result<(), DirectMarketErrorV1> {
        self.bytes(&value.to_le_bytes())
    }
    fn u128(&mut self, value: u128) -> Result<(), DirectMarketErrorV1> {
        self.bytes(&value.to_le_bytes())
    }
    fn id(&mut self, value: [u8; 32]) -> Result<(), DirectMarketErrorV1> { self.bytes(&value) }
    fn finish(self) -> Result<(), DirectMarketErrorV1> {
        if self.at == self.output.len() { Ok(()) } else { Err(DirectMarketErrorV1::InvalidCount) }
    }
}

struct BodyReader<'a> {
    input: &'a [u8],
    at: usize,
}

impl<'a> BodyReader<'a> {
    fn new(input: &'a [u8]) -> Self { Self { input, at: 0 } }

    fn fill(&mut self, output: &mut [u8]) -> Result<(), DirectMarketErrorV1> {
        let end = self.at.checked_add(output.len()).ok_or(DirectMarketErrorV1::Arithmetic)?;
        let source = self.input.get(self.at..end).ok_or(DirectMarketErrorV1::InvalidCount)?;
        output.copy_from_slice(source);
        self.at = end;
        Ok(())
    }

    fn u8(&mut self) -> Result<u8, DirectMarketErrorV1> {
        let value = *self.input.get(self.at).ok_or(DirectMarketErrorV1::InvalidCount)?;
        self.at = self.at.checked_add(1).ok_or(DirectMarketErrorV1::Arithmetic)?;
        Ok(value)
    }
    fn u32(&mut self) -> Result<u32, DirectMarketErrorV1> {
        let mut bytes = [0u8; 4];
        self.fill(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }
    fn u64(&mut self) -> Result<u64, DirectMarketErrorV1> {
        let mut bytes = [0u8; 8];
        self.fill(&mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }
    fn u128(&mut self) -> Result<u128, DirectMarketErrorV1> {
        let mut bytes = [0u8; 16];
        self.fill(&mut bytes)?;
        Ok(u128::from_le_bytes(bytes))
    }
    fn id(&mut self) -> Result<[u8; 32], DirectMarketErrorV1> {
        let mut value = [0u8; 32];
        self.fill(&mut value)?;
        Ok(value)
    }
    fn finish(self) -> Result<(), DirectMarketErrorV1> {
        if self.at == self.input.len() { Ok(()) } else { Err(DirectMarketErrorV1::InvalidCount) }
    }
}

const _: () = assert!(DIRECT_MARKET_ROOT_BODY_BYTES_V1 == 1_722);
const _: () = assert!(DIRECT_SELECTION_BODY_BYTES_V1 == 1_625);
const _: () = assert!(DIRECT_ACTION_REPLAY_BODY_BYTES_V1 == 390);
const _: () = assert!(DIRECT_RESERVATION_BODY_BYTES_V1 == 469);
const _: () = assert!(core::mem::size_of::<[u8; 253]>() == 253);
const _: EconomicOrderV2 = EMPTY_ECONOMIC_ORDER_V2;
