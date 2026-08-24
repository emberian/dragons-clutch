//! Canonical fixed-width semantic body for current Direct `0xb1/2`.
//!
//! The four-byte Solana frame remains owned by `clutch-solana-layout`.  This
//! codec writes into caller storage so the adapter never needs a second 2.5KiB
//! root array on its stack.

use crate::current_v2::{
    DirectCurrentGeneralAuthorityV2, DirectCurrentProductAuthorityV2,
    DirectMarketBindingV2, DirectMarketRootV2,
};
use crate::liveness_v1::{DirectCandidateLivenessBindingV1, DirectCandidateWorkScheduleV1};
use crate::reservation_v1::DirectReservationV1;
use crate::selection_v1::DirectSelectionV1;
use crate::{
    DirectActionReplayV1, DirectMarketErrorV1, DirectRentOwnerV1,
    DirectRootPhaseV1, DirectScheduleV1, DirectTerminalReasonV1,
};

/// Exact current b1/v2 semantic-body width.
pub const DIRECT_MARKET_ROOT_BODY_BYTES_V2: usize = 2_498;

/// Stream the canonical fresh action-1 root directly into caller-owned
/// storage. This is the live-SBF construction boundary: no `DirectMarketRootV2`
/// or second 2.5KiB body is returned by value.
pub fn encode_direct_market_foundation_body_v2(
    binding: &DirectMarketBindingV2,
    schedule: DirectScheduleV1,
    root_rent: DirectRentOwnerV1,
    output: &mut [u8],
) -> Result<(), DirectMarketErrorV1> {
    binding.validate()?;
    schedule.validate()?;
    root_rent.validate()?;
    if output.len() != DIRECT_MARKET_ROOT_BODY_BYTES_V2 {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let mut writer = BodyWriter::new(output);
    write_binding(&mut writer, binding)?;
    write_schedule(&mut writer, schedule)?;
    write_rent(&mut writer, root_rent)?;
    writer.u8(root_phase_byte(DirectRootPhaseV1::Open))?;
    writer.u8(0)?;
    writer.u8(0)?;
    writer.u8(0)?;
    writer.u8(0)?;
    writer.id([0; 32])?;
    writer.id([0; 32])?;
    writer.id([0; 32])?;
    writer.id([0; 32])?;
    writer.id([0; 32])?;
    writer.finish()
}

/// Encode the current root directly into exact caller-provided storage.
pub fn encode_direct_market_root_body_v2(
    value: &DirectMarketRootV2,
    output: &mut [u8],
) -> Result<(), DirectMarketErrorV1> {
    value.validate()?;
    if output.len() != DIRECT_MARKET_ROOT_BODY_BYTES_V2 {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let mut writer = BodyWriter::new(output);
    write_binding(&mut writer, value.binding())?;
    write_schedule(&mut writer, value.schedule())?;
    write_rent(&mut writer, value.root_rent())?;
    writer.u8(root_phase_byte(value.phase()))?;
    writer.u8(value.terminal_reason().map_or(0, terminal_reason_byte))?;
    writer.u8(value.admitted_reservations())?;
    writer.u8(value.live_reservations())?;
    writer.u8(value.retired_reservations())?;
    let mut index = 0u8;
    while index < 2 {
        if index < value.live_reservations() {
            writer.id(value.reservation_account(index)?)?;
        } else {
            writer.id([0; 32])?;
        }
        index += 1;
    }
    index = 0;
    while index < 2 {
        if index < value.live_reservations() {
            writer.id(value.reservation_semantic_id(index)?)?;
        } else {
            writer.id([0; 32])?;
        }
        index += 1;
    }
    writer.id(value.selection_account())?;
    writer.finish()
}

/// Decode and validate one hostile current root semantic body.
pub fn decode_direct_market_root_body_v2(
    input: &[u8],
) -> Result<DirectMarketRootV2, DirectMarketErrorV1> {
    if input.len() != DIRECT_MARKET_ROOT_BODY_BYTES_V2 {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let mut reader = BodyReader::new(input);
    let value = DirectMarketRootV2 {
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

/// Encode the unchanged permanent b3/v1 replay against current b1/v2.
pub fn encode_direct_action_replay_body_for_root_v2<B: crate::DirectHashBackendV1>(
    value: DirectActionReplayV1,
    root: &DirectMarketRootV2,
    backend: &B,
) -> Result<[u8; crate::codec_v1::DIRECT_ACTION_REPLAY_BODY_BYTES_V1], DirectMarketErrorV1> {
    let projection = root.transition_projection(backend)?;
    crate::codec_v1::encode_direct_action_replay_body_v1(value, projection)
}

/// Hostile-decode the unchanged permanent b3/v1 replay against current b1/v2.
pub fn decode_direct_action_replay_body_for_root_v2<B: crate::DirectHashBackendV1>(
    input: &[u8; crate::codec_v1::DIRECT_ACTION_REPLAY_BODY_BYTES_V1],
    root: &DirectMarketRootV2,
    backend: &B,
) -> Result<DirectActionReplayV1, DirectMarketErrorV1> {
    let projection = root.transition_projection(backend)?;
    crate::codec_v1::decode_direct_action_replay_body_v1(input, projection)
}

/// Encode the unchanged b4/v1 Reservation against current b1/v2.
pub fn encode_direct_reservation_body_for_root_v2<B: crate::DirectHashBackendV1>(
    value: DirectReservationV1,
    root: &DirectMarketRootV2,
    backend: &B,
) -> Result<[u8; crate::codec_v1::DIRECT_RESERVATION_BODY_BYTES_V1], DirectMarketErrorV1> {
    let projection = root.transition_projection(backend)?;
    crate::codec_v1::encode_direct_reservation_body_v1(value, projection)
}

/// Hostile-decode the unchanged b4/v1 Reservation against current b1/v2.
pub fn decode_direct_reservation_body_for_root_v2<B: crate::DirectHashBackendV1>(
    input: &[u8; crate::codec_v1::DIRECT_RESERVATION_BODY_BYTES_V1],
    root: &DirectMarketRootV2,
    backend: &B,
) -> Result<DirectReservationV1, DirectMarketErrorV1> {
    let projection = root.transition_projection(backend)?;
    crate::codec_v1::decode_direct_reservation_body_v1(input, projection)
}

/// Encode the unchanged b2/v1 Selection against current b1/v2.
pub fn encode_direct_selection_body_for_root_v2<B: crate::DirectHashBackendV1>(
    value: DirectSelectionV1,
    root: &DirectMarketRootV2,
    backend: &B,
) -> Result<[u8; crate::codec_v1::DIRECT_SELECTION_BODY_BYTES_V1], DirectMarketErrorV1> {
    let projection = root.transition_projection(backend)?;
    crate::codec_v1::encode_direct_selection_body_v1(value, projection)
}

/// Hostile-decode the unchanged b2/v1 Selection against current b1/v2.
pub fn decode_direct_selection_body_for_root_v2<B: crate::DirectHashBackendV1>(
    input: &[u8; crate::codec_v1::DIRECT_SELECTION_BODY_BYTES_V1],
    root: &DirectMarketRootV2,
    backend: &B,
) -> Result<DirectSelectionV1, DirectMarketErrorV1> {
    let projection = root.transition_projection(backend)?;
    crate::codec_v1::decode_direct_selection_body_v1(input, projection)
}


fn write_binding(
    writer: &mut BodyWriter<'_>,
    value: &DirectMarketBindingV2,
) -> Result<(), DirectMarketErrorV1> {
    writer.id(value.market_instance_id)?;
    writer.u64(value.generation)?;
    writer.u8(value.outcome_count)?;
    for id in [
        value.realm_id,
        value.collateral_profile_id,
        value.collateral_policy_id,
        value.collateral_release_id,
        value.resolution_account,
        value.direct_epoch_semantics_id,
        value.revenue_policy_id,
        value.batch_policy_id,
        value.direct_fee_shape_id,
        value.fee_treasury_owner,
    ] {
        writer.id(id)?;
    }
    writer.u32(value.fee_dispersion_bps)?;
    writer.u32(value.fee_floor_range_bps)?;
    writer.u32(value.fee_maker_rebate_num)?;
    writer.u32(value.fee_treasury_num)?;
    writer.u32(value.fee_split_den)?;
    writer.id(value.candidate_lifecycle_policy_id)?;
    writer.id(value.candidate_liveness_policy_id)?;
    write_candidate_liveness(writer, value.candidate_liveness)?;
    writer.id(value.direct_schedule_policy_id)?;
    write_product(writer, &value.product)?;
    write_general(writer, &value.general)?;
    for id in [
        value.direct_root_account,
        value.action_replay_account,
        value.neutral_lamport_sink,
        value.relation_policy_id,
        value.price_policy_id,
    ] {
        writer.id(id)?;
    }
    writer.u64(value.price_scale)
}

fn read_binding(reader: &mut BodyReader<'_>) -> Result<DirectMarketBindingV2, DirectMarketErrorV1> {
    Ok(DirectMarketBindingV2 {
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
        candidate_liveness: read_candidate_liveness(reader)?,
        direct_schedule_policy_id: reader.id()?,
        product: read_product(reader)?,
        general: read_general(reader)?,
        direct_root_account: reader.id()?,
        action_replay_account: reader.id()?,
        neutral_lamport_sink: reader.id()?,
        relation_policy_id: reader.id()?,
        price_policy_id: reader.id()?,
        price_scale: reader.u64()?,
    })
}

fn write_product(
    writer: &mut BodyWriter<'_>,
    value: &DirectCurrentProductAuthorityV2,
) -> Result<(), DirectMarketErrorV1> {
    writer.id(value.product_root_account)?;
    writer.id(value.product_market_binding_id)?;
    writer.u64(value.product_generation)?;
    writer.id(value.product_family_prestate_id)?;
    writer.id(value.product_family_poststate_id)?;
    writer.id(value.product_family_admission_receipt_id)?;
    writer.u32(value.family_admission_sequence)?;
    writer.id(value.series_link_account)?;
    writer.id(value.series_link_v2_id)?;
    writer.u32(value.series_ordinal)?;
    for id in [
        value.compiler_bundle_v6_id,
        value.funding_quote_v5_id,
        value.attachment_plan_v5_id,
        value.foundation_schedule_v3_id,
        value.foundation_graph_v3_id,
        value.market_liability_founding_id,
        value.claim_mint_founding_plan_id,
        value.claim_issuance_binding_id,
        value.general_founding_policy_id,
        value.product_preauthorization_id,
        value.product_direct_global_liveness_account,
        value.product_direct_global_liveness_binding_id,
        value.product_direct_global_liveness_activation_id,
        value.activated_product_market_binding_id,
        value.direct_work_quote_id,
    ] {
        writer.id(id)?;
    }
    Ok(())
}

fn read_product(
    reader: &mut BodyReader<'_>,
) -> Result<DirectCurrentProductAuthorityV2, DirectMarketErrorV1> {
    Ok(DirectCurrentProductAuthorityV2 {
        product_root_account: reader.id()?,
        product_market_binding_id: reader.id()?,
        product_generation: reader.u64()?,
        product_family_prestate_id: reader.id()?,
        product_family_poststate_id: reader.id()?,
        product_family_admission_receipt_id: reader.id()?,
        family_admission_sequence: reader.u32()?,
        series_link_account: reader.id()?,
        series_link_v2_id: reader.id()?,
        series_ordinal: reader.u32()?,
        compiler_bundle_v6_id: reader.id()?,
        funding_quote_v5_id: reader.id()?,
        attachment_plan_v5_id: reader.id()?,
        foundation_schedule_v3_id: reader.id()?,
        foundation_graph_v3_id: reader.id()?,
        market_liability_founding_id: reader.id()?,
        claim_mint_founding_plan_id: reader.id()?,
        claim_issuance_binding_id: reader.id()?,
        general_founding_policy_id: reader.id()?,
        product_preauthorization_id: reader.id()?,
        product_direct_global_liveness_account: reader.id()?,
        product_direct_global_liveness_binding_id: reader.id()?,
        product_direct_global_liveness_activation_id: reader.id()?,
        activated_product_market_binding_id: reader.id()?,
        direct_work_quote_id: reader.id()?,
    })
}

fn write_general(
    writer: &mut BodyWriter<'_>,
    value: &DirectCurrentGeneralAuthorityV2,
) -> Result<(), DirectMarketErrorV1> {
    for id in value.ids() {
        writer.id(id)?;
    }
    Ok(())
}

fn read_general(
    reader: &mut BodyReader<'_>,
) -> Result<DirectCurrentGeneralAuthorityV2, DirectMarketErrorV1> {
    Ok(DirectCurrentGeneralAuthorityV2 {
        general_market_binding_account: reader.id()?,
        general_market_binding_v4_data_id: reader.id()?,
        general_market_runtime_account: reader.id()?,
        general_market_runtime_data_id: reader.id()?,
        revenue_policy_record_account: reader.id()?,
        revenue_policy_record_v2_id: reader.id()?,
        revenue_policy_v2_digest: reader.id()?,
        treasury_owner: reader.id()?,
        treasury_position_derivation_policy_v2_id: reader.id()?,
        treasury_position_account: reader.id()?,
        treasury_replay_account: reader.id()?,
        treasury_service_ledger_account: reader.id()?,
    })
}

fn write_candidate_liveness(
    writer: &mut BodyWriter<'_>,
    value: DirectCandidateLivenessBindingV1,
) -> Result<(), DirectMarketErrorV1> {
    value.validate()?;
    for id in [
        value.policy_account,
        value.policy_data_id,
        value.global_lifecycle_id,
        value.global_bundle_binding_id,
        value.global_capitalization_receipt_id,
        value.global_bundle_commitment_id,
        value.candidate_account,
        value.candidate_data_id,
        value.candidate_semantic_owner,
        value.candidate_quote_schedule_id,
        value.candidate_receipt_program_id,
    ] {
        writer.id(id)?;
    }
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

fn root_phase_byte(value: DirectRootPhaseV1) -> u8 { value.byte() }

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

fn terminal_reason_byte(value: DirectTerminalReasonV1) -> u8 { value.byte() }

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
    fn id(&mut self, value: [u8; 32]) -> Result<(), DirectMarketErrorV1> {
        self.bytes(&value)
    }
    fn finish(self) -> Result<(), DirectMarketErrorV1> {
        if self.at == self.output.len() {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::InvalidCount)
        }
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
        let mut value = [0; 4];
        self.fill(&mut value)?;
        Ok(u32::from_le_bytes(value))
    }
    fn u64(&mut self) -> Result<u64, DirectMarketErrorV1> {
        let mut value = [0; 8];
        self.fill(&mut value)?;
        Ok(u64::from_le_bytes(value))
    }
    fn id(&mut self) -> Result<[u8; 32], DirectMarketErrorV1> {
        let mut value = [0; 32];
        self.fill(&mut value)?;
        Ok(value)
    }
    fn finish(self) -> Result<(), DirectMarketErrorV1> {
        if self.at == self.input.len() {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::InvalidCount)
        }
    }
}

const _: () = assert!(DIRECT_MARKET_ROOT_BODY_BYTES_V2 == 2_498);
