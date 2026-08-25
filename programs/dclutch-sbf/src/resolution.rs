//! Atomic price and permissionless failure resolution transitions.

use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_product_contract::{ContentId, terminal::ResolutionKind};
use dclutch_pyth_contract::{
    instruction::{
        ResolveCategoricalFailureV1, ResolveCategoricalInstructionV1, ResolveCategoricalPythV1,
    },
    receipt::{Clock as ReceiptClock, PriceInput, ResolutionReceiptV1},
};
use dclutch_pyth_svm::FullPriceUpdateV2;
use solana_program::{
    account_info::AccountInfo, clock::Clock, hash::hash, program_error::ProgramError,
    pubkey::Pubkey, sysvar::Sysvar,
};

use crate::{
    AdapterError,
    authenticate::{
        FailureFrame, PriceFrame, authenticate_fund, authenticate_market, authenticate_provider,
        selected_release,
    },
    close_fund, provider,
};

#[inline(never)]
pub(crate) fn dispatch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: ResolveCategoricalInstructionV1<'_>,
) -> Result<(), ProgramError> {
    match instruction {
        ResolveCategoricalInstructionV1::Pyth(instruction) => {
            process_price(program_id, PriceFrame::parse(accounts)?, instruction)
        }
        ResolveCategoricalInstructionV1::Failure(instruction) => {
            process_failure(program_id, FailureFrame::parse(accounts)?, instruction)
        }
    }
}

#[inline(never)]
fn process_price(
    program_id: &Pubkey,
    frame: PriceFrame<'_, '_>,
    instruction: ResolveCategoricalPythV1<'_>,
) -> Result<(), ProgramError> {
    let market = authenticate_market(
        program_id,
        frame.market,
        instruction.generation(),
        instruction.child_count(),
    )?;
    let (funding, material) = authenticate_fund(
        program_id,
        frame.fund,
        frame.market,
        frame.material,
        frame.manifest,
        frame.material_staging_cursor,
        frame.manifest_staging_cursor,
        frame.rent_credit,
        frame.rent_sysvar,
        market,
    )?;
    let clock = Clock::get().map_err(|_| AdapterError::AccountData)?;
    let release = selected_release(
        material
            .obligation
            .provider_release()
            .provider_deployment_release_id()
            .to_bytes(),
        clock.unix_timestamp,
    )?;
    let provider_facts = authenticate_provider(&frame, release, instruction.body(), funding)?;
    let body_digest = hash(instruction.body()).to_bytes();
    let update = provider::post_and_load(
        &frame,
        provider_facts,
        instruction.body(),
        clock.slot,
        material.obligation.adapter_config().provider_feed_id(),
    )?;
    let body_id = dclutch_source_contract::ContentId::new(body_digest)
        .map_err(|_| AdapterError::ContentIdentity)?;
    let normalized = material
        .obligation
        .normalize_authenticated_update(
            body_id,
            material.window.schedule_id(),
            0,
            update.feed_id(),
            update.price(),
            update.confidence(),
            update.exponent(),
            update.publish_time(),
        )
        .map_err(|_| AdapterError::ProviderAuthentication)?;
    normalized
        .validate(
            material.source_id,
            material.source,
            material.provider_release_id,
            material.provider_release,
            material.window,
            0,
            clock.unix_timestamp,
        )
        .map_err(|_| AdapterError::ProviderAuthentication)?;
    let winner = material
        .result_domain
        .map(normalized.atoms(), 1)
        .map_err(|_| AdapterError::MarketTransition)?;

    dispatch_price_width(
        market.outcome_count,
        &frame,
        instruction,
        clock,
        update,
        body_digest,
        winner,
    )?;
    close_fund::close_price(&frame, funding)
}

#[inline(never)]
fn process_failure(
    program_id: &Pubkey,
    frame: FailureFrame<'_, '_>,
    instruction: ResolveCategoricalFailureV1,
) -> Result<(), ProgramError> {
    let market = authenticate_market(
        program_id,
        frame.market,
        instruction.generation(),
        instruction.child_count(),
    )?;
    let (funding, material) = authenticate_fund(
        program_id,
        frame.fund,
        frame.market,
        frame.material,
        frame.manifest,
        frame.material_staging_cursor,
        frame.manifest_staging_cursor,
        frame.rent_credit,
        frame.rent_sysvar,
        market,
    )?;
    let clock = Clock::get().map_err(|_| AdapterError::AccountData)?;
    if clock.unix_timestamp <= material.window.end_unix_seconds() {
        return Err(AdapterError::KernelResolution.into());
    }
    dispatch_failure_width(
        market.outcome_count,
        &frame,
        instruction,
        clock,
        material.result_domain.failure_selector(),
    )?;
    close_fund::close_failure(&frame, funding)
}

#[inline(never)]
fn dispatch_price_width(
    outcomes: u8,
    frame: &PriceFrame<'_, '_>,
    instruction: ResolveCategoricalPythV1<'_>,
    clock: Clock,
    update: FullPriceUpdateV2,
    body_digest: [u8; 32],
    winner: u8,
) -> Result<(), ProgramError> {
    match outcomes {
        2 => transition_price::<2>(frame, instruction, clock, update, body_digest, winner),
        3 => transition_price::<3>(frame, instruction, clock, update, body_digest, winner),
        4 => transition_price::<4>(frame, instruction, clock, update, body_digest, winner),
        5 => transition_price::<5>(frame, instruction, clock, update, body_digest, winner),
        6 => transition_price::<6>(frame, instruction, clock, update, body_digest, winner),
        7 => transition_price::<7>(frame, instruction, clock, update, body_digest, winner),
        8 => transition_price::<8>(frame, instruction, clock, update, body_digest, winner),
        9 => transition_price::<9>(frame, instruction, clock, update, body_digest, winner),
        10 => transition_price::<10>(frame, instruction, clock, update, body_digest, winner),
        11 => transition_price::<11>(frame, instruction, clock, update, body_digest, winner),
        12 => transition_price::<12>(frame, instruction, clock, update, body_digest, winner),
        13 => transition_price::<13>(frame, instruction, clock, update, body_digest, winner),
        14 => transition_price::<14>(frame, instruction, clock, update, body_digest, winner),
        15 => transition_price::<15>(frame, instruction, clock, update, body_digest, winner),
        16 => transition_price::<16>(frame, instruction, clock, update, body_digest, winner),
        _ => Err(AdapterError::AccountData.into()),
    }
}

#[inline(never)]
fn dispatch_failure_width(
    outcomes: u8,
    frame: &FailureFrame<'_, '_>,
    instruction: ResolveCategoricalFailureV1,
    clock: Clock,
    winner: u8,
) -> Result<(), ProgramError> {
    match outcomes {
        2 => transition_failure::<2>(frame, instruction, clock, winner),
        3 => transition_failure::<3>(frame, instruction, clock, winner),
        4 => transition_failure::<4>(frame, instruction, clock, winner),
        5 => transition_failure::<5>(frame, instruction, clock, winner),
        6 => transition_failure::<6>(frame, instruction, clock, winner),
        7 => transition_failure::<7>(frame, instruction, clock, winner),
        8 => transition_failure::<8>(frame, instruction, clock, winner),
        9 => transition_failure::<9>(frame, instruction, clock, winner),
        10 => transition_failure::<10>(frame, instruction, clock, winner),
        11 => transition_failure::<11>(frame, instruction, clock, winner),
        12 => transition_failure::<12>(frame, instruction, clock, winner),
        13 => transition_failure::<13>(frame, instruction, clock, winner),
        14 => transition_failure::<14>(frame, instruction, clock, winner),
        15 => transition_failure::<15>(frame, instruction, clock, winner),
        16 => transition_failure::<16>(frame, instruction, clock, winner),
        _ => Err(AdapterError::AccountData.into()),
    }
}

#[inline(never)]
fn transition_price<const N: usize>(
    frame: &PriceFrame<'_, '_>,
    instruction: ResolveCategoricalPythV1<'_>,
    clock: Clock,
    update: FullPriceUpdateV2,
    body_digest: [u8; 32],
    winner: u8,
) -> Result<(), ProgramError> {
    let mut state = decode_market::<N>(frame.market)?;
    let outcome_count = u8::try_from(N).map_err(|_| AdapterError::MarketTransition)?;
    let receipt = ResolutionReceiptV1::price(
        PriceInput {
            winner,
            posted_slot: update.posted_slot(),
            consumed_slot: clock.slot,
            consumed_unix_timestamp: clock.unix_timestamp,
            previous_publish_time: update.prev_publish_time(),
            publish_time: update.publish_time(),
            price: update.price(),
            confidence: update.confidence(),
            exponent: update.exponent(),
            post_params_body_digest: body_digest,
        },
        outcome_count,
    )
    .map_err(|_| AdapterError::MarketTransition)?;
    let settlement = settlement_summary::<N>(
        receipt,
        ResolutionKind::Occurrence,
        usize::from(winner),
        clock.slot,
    )?;
    resolve_state(
        &mut state,
        instruction.generation(),
        instruction.child_count(),
        settlement,
    )?;

    // Reclaim succeeds before any dClutch account is persistently changed.
    // Any later refusal still rolls the CPI back atomically at runtime.
    provider::reclaim(frame)?;
    encode_market(frame.market, &state)
}

#[inline(never)]
fn transition_failure<const N: usize>(
    frame: &FailureFrame<'_, '_>,
    instruction: ResolveCategoricalFailureV1,
    clock: Clock,
    winner: u8,
) -> Result<(), ProgramError> {
    let mut state = decode_market::<N>(frame.market)?;
    let outcome_count = u8::try_from(N).map_err(|_| AdapterError::MarketTransition)?;
    let receipt = ResolutionReceiptV1::failure(
        winner,
        outcome_count,
        ReceiptClock {
            slot: clock.slot,
            unix_timestamp: clock.unix_timestamp,
        },
    )
    .map_err(|_| AdapterError::MarketTransition)?;
    let settlement = settlement_summary::<N>(
        receipt,
        ResolutionKind::Failure,
        usize::from(winner),
        clock.slot,
    )?;
    resolve_state(
        &mut state,
        instruction.generation(),
        instruction.child_count(),
        settlement,
    )?;
    encode_market(frame.market, &state)
}

#[inline(never)]
fn resolve_state<const N: usize>(
    state: &mut CategoricalMarketV1<N>,
    generation: u64,
    child_count: u64,
    settlement: CategoricalSettlementSummaryV1,
) -> Result<(), ProgramError> {
    state
        .resolve_with_summary(generation, settlement)
        .map_err(|_| AdapterError::MarketTransition)?;
    state
        .retire_child(generation, child_count)
        .map_err(|_| AdapterError::MarketTransition)?;
    Ok(())
}

fn settlement_summary<const N: usize>(
    receipt: ResolutionReceiptV1,
    kind: ResolutionKind,
    winner: usize,
    observed_slot: u64,
) -> Result<CategoricalSettlementSummaryV1, ProgramError> {
    let evidence_id = ContentId::new(hash(&receipt.to_bytes()).to_bytes())
        .map_err(|_| AdapterError::MarketTransition)?;
    let terminal_sequence = observed_slot
        .checked_add(1)
        .ok_or(AdapterError::Arithmetic)?;
    CategoricalSettlementSummaryV1::resolved::<N>(evidence_id, kind, winner, terminal_sequence)
        .map_err(|_| AdapterError::MarketTransition.into())
}

fn decode_market<const N: usize>(
    account: &AccountInfo<'_>,
) -> Result<CategoricalMarketV1<N>, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    CategoricalMarketV1::decode(&data).map_err(|_| AdapterError::AccountData.into())
}

fn encode_market<const N: usize>(
    account: &AccountInfo<'_>,
    state: &CategoricalMarketV1<N>,
) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::AccountData)?;
    state
        .encode(&mut data)
        .map_err(|_| AdapterError::MarketTransition.into())
}
