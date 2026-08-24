//! Atomic price and permissionless failure resolution transitions.

use dclutch_kernel::resolution::categorical_pyth_v1::PythV1Observation;
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_product_contract::{ContentId, terminal::ResolutionKind};
use dclutch_pyth_contract::{
    instruction::{
        ResolveCategoricalFailureV1, ResolveCategoricalInstructionV1, ResolveCategoricalPythV1,
    },
    policy::CategoricalPythPolicyRecordV1,
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
        frame.sponsor,
        frame.rent_sysvar,
        market,
    )?;
    let clock = Clock::get().map_err(|_| AdapterError::AccountData)?;
    let release = selected_release(*material.policy().release_id(), clock.unix_timestamp)?;
    let provider_facts = authenticate_provider(&frame, release, instruction.body(), funding)?;
    let body_digest = hash(instruction.body()).to_bytes();
    let update = provider::post_and_load(
        &frame,
        provider_facts,
        instruction.body(),
        clock.slot,
        *material.feed_profile().provider_feed_id(),
    )?;

    dispatch_price_width(
        market.outcome_count,
        &frame,
        instruction,
        clock,
        update,
        body_digest,
        *material.policy(),
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
        frame.sponsor,
        frame.rent_sysvar,
        market,
    )?;
    let clock = Clock::get().map_err(|_| AdapterError::AccountData)?;
    dispatch_failure_width(
        market.outcome_count,
        &frame,
        instruction,
        clock,
        *material.policy(),
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
    policy: CategoricalPythPolicyRecordV1,
) -> Result<(), ProgramError> {
    match outcomes {
        2 => transition_price::<2>(frame, instruction, clock, update, body_digest, policy),
        3 => transition_price::<3>(frame, instruction, clock, update, body_digest, policy),
        4 => transition_price::<4>(frame, instruction, clock, update, body_digest, policy),
        5 => transition_price::<5>(frame, instruction, clock, update, body_digest, policy),
        6 => transition_price::<6>(frame, instruction, clock, update, body_digest, policy),
        7 => transition_price::<7>(frame, instruction, clock, update, body_digest, policy),
        8 => transition_price::<8>(frame, instruction, clock, update, body_digest, policy),
        9 => transition_price::<9>(frame, instruction, clock, update, body_digest, policy),
        10 => transition_price::<10>(frame, instruction, clock, update, body_digest, policy),
        11 => transition_price::<11>(frame, instruction, clock, update, body_digest, policy),
        12 => transition_price::<12>(frame, instruction, clock, update, body_digest, policy),
        13 => transition_price::<13>(frame, instruction, clock, update, body_digest, policy),
        14 => transition_price::<14>(frame, instruction, clock, update, body_digest, policy),
        15 => transition_price::<15>(frame, instruction, clock, update, body_digest, policy),
        16 => transition_price::<16>(frame, instruction, clock, update, body_digest, policy),
        _ => Err(AdapterError::AccountData.into()),
    }
}

#[inline(never)]
fn dispatch_failure_width(
    outcomes: u8,
    frame: &FailureFrame<'_, '_>,
    instruction: ResolveCategoricalFailureV1,
    clock: Clock,
    policy: CategoricalPythPolicyRecordV1,
) -> Result<(), ProgramError> {
    match outcomes {
        2 => transition_failure::<2>(frame, instruction, clock, policy),
        3 => transition_failure::<3>(frame, instruction, clock, policy),
        4 => transition_failure::<4>(frame, instruction, clock, policy),
        5 => transition_failure::<5>(frame, instruction, clock, policy),
        6 => transition_failure::<6>(frame, instruction, clock, policy),
        7 => transition_failure::<7>(frame, instruction, clock, policy),
        8 => transition_failure::<8>(frame, instruction, clock, policy),
        9 => transition_failure::<9>(frame, instruction, clock, policy),
        10 => transition_failure::<10>(frame, instruction, clock, policy),
        11 => transition_failure::<11>(frame, instruction, clock, policy),
        12 => transition_failure::<12>(frame, instruction, clock, policy),
        13 => transition_failure::<13>(frame, instruction, clock, policy),
        14 => transition_failure::<14>(frame, instruction, clock, policy),
        15 => transition_failure::<15>(frame, instruction, clock, policy),
        16 => transition_failure::<16>(frame, instruction, clock, policy),
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
    policy_record: CategoricalPythPolicyRecordV1,
) -> Result<(), ProgramError> {
    let mut state = decode_market::<N>(frame.market)?;
    let policy = policy_record
        .to_kernel_policy()
        .map_err(|_| AdapterError::MarketTransition)?;
    let resolution = policy
        .resolve_price(
            clock.unix_timestamp,
            PythV1Observation {
                prev_publish_time: update.prev_publish_time(),
                publish_time: update.publish_time(),
                price: update.price(),
                confidence: update.confidence(),
                exponent: update.exponent(),
            },
        )
        .map_err(|_| AdapterError::KernelResolution)?;
    let winner = u8::try_from(resolution.winner()).map_err(|_| AdapterError::MarketTransition)?;
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
    policy_record: CategoricalPythPolicyRecordV1,
) -> Result<(), ProgramError> {
    let mut state = decode_market::<N>(frame.market)?;
    let policy = policy_record
        .to_kernel_policy()
        .map_err(|_| AdapterError::MarketTransition)?;
    let resolution = policy
        .resolve_failure(clock.unix_timestamp)
        .map_err(|_| AdapterError::KernelResolution)?;
    let winner = u8::try_from(resolution.winner()).map_err(|_| AdapterError::MarketTransition)?;
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
