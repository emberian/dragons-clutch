//! Atomic price and permissionless failure resolution transitions.

use dclutch_core_contract::Phase as RootPhase;
use dclutch_kernel::resolution::categorical_pyth_v1::PythV1Observation;
use dclutch_pyth_contract::{
    instruction::{
        ResolveCategoricalFailureV1, ResolveCategoricalInstructionV1, ResolveCategoricalPythV1,
    },
    market::MarketStateV1,
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
    let funding = authenticate_fund(
        program_id,
        frame.fund,
        frame.market,
        frame.sponsor,
        instruction.generation(),
    )?;
    let clock = Clock::get().map_err(|_| AdapterError::AccountData)?;
    let release = selected_release(market.release_id, clock.unix_timestamp)?;
    let provider_facts = authenticate_provider(&frame, release, instruction.body(), funding)?;
    let body_digest = hash(instruction.body()).to_bytes();
    let update = provider::post_and_load(
        &frame,
        provider_facts,
        instruction.body(),
        clock.slot,
        market.provider_feed_id,
    )?;

    dispatch_price_width(
        market.outcome_count,
        &frame,
        instruction,
        clock,
        update,
        body_digest,
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
    let funding = authenticate_fund(
        program_id,
        frame.fund,
        frame.market,
        frame.sponsor,
        instruction.generation(),
    )?;
    let clock = Clock::get().map_err(|_| AdapterError::AccountData)?;
    dispatch_failure_width(market.outcome_count, &frame, instruction, clock)?;
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
) -> Result<(), ProgramError> {
    match outcomes {
        2 => transition_price::<2>(frame, instruction, clock, update, body_digest),
        3 => transition_price::<3>(frame, instruction, clock, update, body_digest),
        4 => transition_price::<4>(frame, instruction, clock, update, body_digest),
        5 => transition_price::<5>(frame, instruction, clock, update, body_digest),
        6 => transition_price::<6>(frame, instruction, clock, update, body_digest),
        7 => transition_price::<7>(frame, instruction, clock, update, body_digest),
        8 => transition_price::<8>(frame, instruction, clock, update, body_digest),
        9 => transition_price::<9>(frame, instruction, clock, update, body_digest),
        10 => transition_price::<10>(frame, instruction, clock, update, body_digest),
        11 => transition_price::<11>(frame, instruction, clock, update, body_digest),
        12 => transition_price::<12>(frame, instruction, clock, update, body_digest),
        13 => transition_price::<13>(frame, instruction, clock, update, body_digest),
        14 => transition_price::<14>(frame, instruction, clock, update, body_digest),
        15 => transition_price::<15>(frame, instruction, clock, update, body_digest),
        16 => transition_price::<16>(frame, instruction, clock, update, body_digest),
        _ => Err(AdapterError::AccountData.into()),
    }
}

#[inline(never)]
fn dispatch_failure_width(
    outcomes: u8,
    frame: &FailureFrame<'_, '_>,
    instruction: ResolveCategoricalFailureV1,
    clock: Clock,
) -> Result<(), ProgramError> {
    match outcomes {
        2 => transition_failure::<2>(frame, instruction, clock),
        3 => transition_failure::<3>(frame, instruction, clock),
        4 => transition_failure::<4>(frame, instruction, clock),
        5 => transition_failure::<5>(frame, instruction, clock),
        6 => transition_failure::<6>(frame, instruction, clock),
        7 => transition_failure::<7>(frame, instruction, clock),
        8 => transition_failure::<8>(frame, instruction, clock),
        9 => transition_failure::<9>(frame, instruction, clock),
        10 => transition_failure::<10>(frame, instruction, clock),
        11 => transition_failure::<11>(frame, instruction, clock),
        12 => transition_failure::<12>(frame, instruction, clock),
        13 => transition_failure::<13>(frame, instruction, clock),
        14 => transition_failure::<14>(frame, instruction, clock),
        15 => transition_failure::<15>(frame, instruction, clock),
        16 => transition_failure::<16>(frame, instruction, clock),
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
) -> Result<(), ProgramError> {
    let state = decode_market::<N>(frame.market)?;
    let policy = state
        .policy()
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
    let next = resolved_state(
        state,
        instruction.generation(),
        instruction.child_count(),
        usize::from(winner),
        receipt,
    )?;

    // Reclaim succeeds before any dClutch account is persistently changed.
    // Any later refusal still rolls the CPI back atomically at runtime.
    provider::reclaim(frame)?;
    encode_market(frame.market, &next)
}

#[inline(never)]
fn transition_failure<const N: usize>(
    frame: &FailureFrame<'_, '_>,
    instruction: ResolveCategoricalFailureV1,
    clock: Clock,
) -> Result<(), ProgramError> {
    let state = decode_market::<N>(frame.market)?;
    let policy = state
        .policy()
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
    let next = resolved_state(
        state,
        instruction.generation(),
        instruction.child_count(),
        usize::from(winner),
        receipt,
    )?;
    encode_market(frame.market, &next)
}

#[inline(never)]
fn resolved_state<const N: usize>(
    state: MarketStateV1<N>,
    generation: u64,
    child_count: u64,
    winner: usize,
    receipt: ResolutionReceiptV1,
) -> Result<MarketStateV1<N>, ProgramError> {
    let mut root = state.root();
    root.transition_phase(generation, RootPhase::Resolved)
        .map_err(|_| AdapterError::MarketTransition)?;
    root.retire_child(generation, child_count)
        .map_err(|_| AdapterError::MarketTransition)?;

    let mut ledger = state
        .to_kernel_ledger()
        .map_err(|_| AdapterError::MarketTransition)?;
    ledger
        .resolve(winner)
        .map_err(|_| AdapterError::MarketTransition)?;
    let (hoard_atoms, supply, _) = ledger.into_parts();
    MarketStateV1::new(
        root,
        *state.policy(),
        *state.feed_profile(),
        hoard_atoms,
        supply,
        receipt,
    )
    .map_err(|_| AdapterError::MarketTransition.into())
}

fn decode_market<const N: usize>(
    account: &AccountInfo<'_>,
) -> Result<MarketStateV1<N>, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    MarketStateV1::decode(&data).map_err(|_| AdapterError::AccountData.into())
}

fn encode_market<const N: usize>(
    account: &AccountInfo<'_>,
    state: &MarketStateV1<N>,
) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::AccountData)?;
    state
        .encode(&mut data)
        .map_err(|_| AdapterError::MarketTransition.into())
}
