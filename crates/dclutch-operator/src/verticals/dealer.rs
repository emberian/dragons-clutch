//! Chain-derived unsigned Dealer lifecycle construction.
//!
//! Existing LP positions are selected by their observed physical account key.
//! Only fresh activation/creation routes carry an irreducible LP seed choice.

use dclutch_core_contract::{ContentId, Phase};
use dclutch_dealer_contract::{
    LP_POSITION_BYTES, LiquidityAmounts, LpPosition, TradeRequest, TradeSide,
    activation::retire_pool_in_place,
    frame::{
        ConfigPdaSeedsV1, DEALER_CONFIG_SCHEMA_RELEASE_ID_V1, DealerAccountMetaV1,
        DealerCollateralCompartmentV1, DealerCollateralVaultPdaSeedsV1, DealerFrameV1,
        LpPositionPdaSeedsV1, PoolPdaSeedsV1, dealer_account_privileges, validate_market_phase,
    },
    instruction::{
        AddLiquidityV1, CloseLpPositionV1, CreateLpPositionV1, DealerActionV1, DealerInstructionV1,
        RemoveLiquidityV1,
    },
    runtime::{
        LiquidityConfigViewV1, LiquidityProfileV1, PoolViewV1, add_liquidity, close_position,
        execute, remove_liquidity,
    },
};
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, POSITION_PDA_DOMAIN, PositionV1, REALM_PDA_DOMAIN,
    RealmV1,
};
use dclutch_record_contract::STAGING_CURSOR_PDA_SEED_V1;
use dclutch_token_svm::{COption, PRODUCTION_ADAPTER_RELEASES, TokenAccount};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader, bpf_loader_upgradeable, system_program};

use crate::{Observation, ObservedAccount, authenticate_rent_credit, foundation};

use super::{
    VerticalError, authenticate_system_actor, authenticate_system_program, decode_owned,
    observation,
};

/// Finalized Market/Pool/config tuple shared by every existing-Pool route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerPoolState {
    /// Canonical Market account.
    pub market: ObservedAccount,
    /// Canonical Dealer Pool account.
    pub pool: ObservedAccount,
    /// Immutable finalized Dealer config raw record.
    pub config: ObservedAccount,
    /// Drained staging cursor proving finalization of `config`.
    pub config_staging: ObservedAccount,
}

/// Finalized inputs for creating a fresh empty LP position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerCreateLpState {
    /// System payer for the new LP account.
    pub payer: ObservedAccount,
    /// Immutable owner of the new LP position and its RentCredit.
    pub owner: ObservedAccount,
    /// Existing authenticated Pool tuple.
    pub pool: DealerPoolState,
    /// Vacant LP PDA selected by the fresh seed choice.
    pub lp_position: ObservedAccount,
    /// Permanent owner RentCredit.
    pub lp_rent_credit: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
}

/// Exact result of creating a fresh LP account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerCreateLpReport {
    /// Exact unsigned ten-account SBF instruction.
    pub instruction: Instruction,
    /// One finalized observation selecting every fact.
    pub observation: Observation,
    /// Required transaction signers in role order.
    pub required_signers: Vec<Pubkey>,
    /// Fresh LP PDA derived from the caller's irreducible seed choice.
    pub lp_position: Pubkey,
    /// Chain-derived Pool replay sequence.
    pub pool_sequence: u64,
    /// Exact new-account Rent debit paid by `payer`.
    pub rent_debit_lamports: u64,
}

/// Finalized accounts shared by Add and Remove liquidity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerLiquidityState {
    /// LP owner and sole required signer.
    pub owner: ObservedAccount,
    /// Immutable Realm selected by the Market.
    pub realm: ObservedAccount,
    /// Existing authenticated Pool tuple.
    pub pool: DealerPoolState,
    /// Existing program-owned LP account; its physical key is the position identity.
    pub lp_position: ObservedAccount,
    /// Owner's canonical native Position.
    pub owner_position: ObservedAccount,
    /// Pool's canonical native Position.
    pub pool_position: ObservedAccount,
    /// Owner-controlled collateral token account.
    pub owner_collateral: ObservedAccount,
    /// Pool principal-collateral Vault.
    pub principal_vault: ObservedAccount,
    /// Pool realized-fee Vault.
    pub fee_vault: ObservedAccount,
    /// Realm collateral Mint.
    pub collateral_mint: ObservedAccount,
    /// Realm-selected executable token program.
    pub token_program: ObservedAccount,
}

/// Minimal holder choices for one bounded LP share change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerLiquidityChoice {
    /// Shares to mint or burn.
    pub shares: u64,
    /// Principal collateral ceiling (Add) or floor (Remove).
    pub principal_limit: u64,
    /// Realized-fee collateral ceiling (Add) or floor (Remove).
    pub fee_limit: u64,
    /// Per-outcome claim ceilings (Add) or floors (Remove), in Market order.
    pub claim_limits: Vec<u64>,
}

/// Exact segregated movement for a successful liquidity preflight.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerLiquidityMovement {
    /// Principal atoms moved, never combined with fees.
    pub principal_collateral: u64,
    /// Realized-fee atoms moved, never combined with principal.
    pub realized_fee_collateral: u64,
    /// Native claims moved in exact Market outcome order.
    pub claims: Vec<u64>,
    /// LP shares minted or burned.
    pub shares: u64,
    /// `true` for owner-to-Pool movement, `false` for Pool-to-owner movement.
    pub into_pool: bool,
}

/// Exact Add/Remove instruction and its independently computed value movement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerLiquidityReport {
    /// Exact unsigned fourteen-account SBF instruction.
    pub instruction: Instruction,
    /// One finalized observation selecting every fact.
    pub observation: Observation,
    /// Sole required LP-owner signer.
    pub required_signer: Pubkey,
    /// Observed physical LP account used as the kernel position identity.
    pub lp_position: Pubkey,
    /// Chain-derived Pool replay sequence.
    pub pool_sequence: u64,
    /// Chain-derived LP-local replay sequence.
    pub position_sequence: u64,
    /// Exact compartmentalized movement.
    pub movement: DealerLiquidityMovement,
}

/// Finalized accounts for one immediate covered trade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerTradeState {
    /// Trader and sole signer.
    pub trader: ObservedAccount,
    /// Immutable Realm selected by the Market.
    pub realm: ObservedAccount,
    /// Existing authenticated Pool tuple.
    pub pool: DealerPoolState,
    /// Trader's canonical native Position.
    pub trader_position: ObservedAccount,
    /// Pool's canonical native Position.
    pub pool_position: ObservedAccount,
    /// Trader-controlled collateral account.
    pub trader_collateral: ObservedAccount,
    /// Pool principal Vault.
    pub principal_vault: ObservedAccount,
    /// Pool realized-fee Vault.
    pub fee_vault: ObservedAccount,
    /// Realm collateral Mint.
    pub collateral_mint: ObservedAccount,
    /// Realm-selected executable token program.
    pub token_program: ObservedAccount,
}

/// Irreducible immediate-trade choices; reset and sequence come from chain state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerTradeChoice {
    /// Direction of the immediate inventory exchange.
    pub side: TradeSide,
    /// Zero-based Market outcome.
    pub claim_index: u8,
    /// Raw native-claim atoms.
    pub quantity: u64,
    /// Maximum gross collateral debit for Buy, minimum principal credit for Sell.
    pub collateral_limit: u64,
}

/// Exact immediate-trade effects, keeping principal and fee flows disjoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerTradeMovement {
    /// Principal notional transferred.
    pub principal_collateral: u64,
    /// Trader-paid fee transferred separately to the fee Vault.
    pub fee_collateral: u64,
    /// Trader gross collateral debit.
    pub trader_collateral_debit: u64,
    /// Trader gross principal credit.
    pub trader_collateral_credit: u64,
    /// Trader native-claim debit.
    pub trader_claim_debit: u64,
    /// Trader native-claim credit.
    pub trader_claim_credit: u64,
}

/// Exact immediate Dealer trade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerTradeReport {
    /// Exact unsigned thirteen-account SBF instruction.
    pub instruction: Instruction,
    /// One finalized observation selecting every fact.
    pub observation: Observation,
    /// Sole required trader signer.
    pub required_signer: Pubkey,
    /// Chain-derived ladder reset number.
    pub reset_number: u64,
    /// Chain-derived Pool replay sequence.
    pub pool_sequence: u64,
    /// Exact kernel-computed custody movement.
    pub movement: DealerTradeMovement,
}

/// Finalized inputs for closing an existing empty LP account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerCloseLpState {
    /// LP owner and sole signer.
    pub owner: ObservedAccount,
    /// Existing authenticated Pool tuple.
    pub pool: DealerPoolState,
    /// Existing program-owned LP account selected by physical identity.
    pub lp_position: ObservedAccount,
    /// Owner's permanent RentCredit destination.
    pub lp_rent_credit: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
}

/// Exact empty-LP close and rent attribution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerCloseLpReport {
    /// Exact unsigned eight-account SBF instruction.
    pub instruction: Instruction,
    /// One finalized observation selecting every fact.
    pub observation: Observation,
    /// Sole LP-owner signer.
    pub required_signer: Pubkey,
    /// Existing LP physical identity, never reconstructed from an absent seed.
    pub lp_position: Pubkey,
    /// Chain-derived Pool replay sequence.
    pub pool_sequence: u64,
    /// Chain-derived LP replay sequence.
    pub position_sequence: u64,
    /// All observed LP lamports routed to its immutable RentCredit.
    pub rent_credit_lamports: u64,
}

/// Finalized accounts for permissionless quiescent Pool retirement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerRetirePoolState {
    /// Existing authenticated retiring Pool tuple.
    pub pool: DealerPoolState,
    /// Immutable Realm selected by the Market.
    pub realm: ObservedAccount,
    /// Pool's canonical native Position.
    pub pool_position: ObservedAccount,
    /// Pool principal Vault.
    pub principal_vault: ObservedAccount,
    /// Pool realized-fee Vault.
    pub fee_vault: ObservedAccount,
    /// Pool service-funding Vault.
    pub service_vault: ObservedAccount,
    /// Config-owner collateral refund account.
    pub service_refund_vault: ObservedAccount,
    /// Config-owner native Position receiving any physical claim gifts.
    pub refund_position: ObservedAccount,
    /// Pool-authority RentCredit receiving Pool Position lamports.
    pub pool_position_rent_credit: ObservedAccount,
    /// Immutable Pool RentCredit receiving Pool and vault lamports.
    pub pool_rent_credit: ObservedAccount,
    /// Realm collateral Mint.
    pub collateral_mint: ObservedAccount,
    /// Realm-selected executable token program.
    pub token_program: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
}

/// Exact physical retirement effects, including unsolicited account gifts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerRetirePoolReport {
    /// Exact unsigned sixteen-account SBF instruction.
    pub instruction: Instruction,
    /// One finalized observation selecting every fact.
    pub observation: Observation,
    /// Chain-derived Pool replay sequence.
    pub pool_sequence: u64,
    /// Chain-derived Market direct-child replay guard.
    pub market_child_count: u64,
    /// Physical principal-Vault amount refunded before close.
    pub principal_refund: u64,
    /// Physical fee-Vault amount refunded before close.
    pub fee_refund: u64,
    /// Physical service-Vault amount refunded before close.
    pub service_refund: u64,
    /// Physical Pool Position claim balances gifted to the refund Position.
    pub claim_refunds: Vec<u64>,
    /// Pool plus all three token-Vault lamports routed to Pool RentCredit.
    pub pool_rent_credit_lamports: u64,
    /// Pool Position lamports routed to the Pool-authority RentCredit.
    pub pool_position_rent_credit_lamports: u64,
}

/// Fresh activation choice retained for the public refusal boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerActivateChoice {
    /// Fresh nonzero LP PDA seed identity.
    pub initial_lp_id: [u8; 32],
    /// Complete-set claim quantity supplied by the immutable liquidity owner.
    pub initial_claim_quantity: u64,
    /// Initial LP shares.
    pub initial_shares: u64,
}

/// Refuse Dealer activation until its manifest-record derivation is public.
///
/// The SBF route authenticates a capability-manifest raw-record PDA with a
/// schema/release identity that is private to the adapter. Accepting an account
/// or copied constant here would create parallel semantic authority, so the
/// operator deliberately cannot make activation selectable yet.
pub fn build_dealer_activate_pool_v1(
    _program_id: Pubkey,
    _choice: DealerActivateChoice,
) -> Result<Instruction, VerticalError> {
    Err(VerticalError::AbiUnavailable)
}

/// Construct CreateLpPosition from a finalized Pool and one fresh seed choice.
pub fn build_dealer_create_lp_position_v1(
    program_id: Pubkey,
    state: &DealerCreateLpState,
    lp_id: [u8; 32],
) -> Result<DealerCreateLpReport, VerticalError> {
    let observation = observation(&[
        &state.payer,
        &state.owner,
        &state.pool.market,
        &state.pool.pool,
        &state.pool.config,
        &state.pool.config_staging,
        &state.lp_position,
        &state.lp_rent_credit,
        &state.system_program,
        &state.rent_sysvar,
    ])?;
    authenticate_system_actor(&state.payer)?;
    authenticate_system_actor(&state.owner)?;
    authenticate_system_program(&state.system_program)?;
    let rent =
        foundation::decode_rent(&state.rent_sysvar).map_err(|_| VerticalError::InvalidState)?;
    let facts = authenticate_pool(program_id, &state.pool, DealerActionV1::CreateLpPosition)?;
    require_vacant(&state.lp_position)?;
    let lp_seeds = LpPositionPdaSeedsV1::new(
        state.pool.market.key.to_bytes(),
        facts.generation,
        facts.config_id,
        lp_id,
    )
    .map_err(|_| VerticalError::PdaMismatch)?;
    let expected = Pubkey::find_program_address(&lp_seeds.seed_components(), &program_id).0;
    if state.lp_position.key != expected {
        return Err(VerticalError::PdaMismatch);
    }
    authenticate_rent_credit(program_id, &state.lp_rent_credit, state.owner.key)
        .map_err(|_| VerticalError::ContentMismatch)?;
    let rent_debit_lamports = rent.minimum_balance(LP_POSITION_BYTES);
    if state.payer.lamports < rent_debit_lamports {
        return Err(VerticalError::InvalidState);
    }
    let request = CreateLpPositionV1::new(facts.pool_sequence, lp_id)
        .map_err(|_| VerticalError::InvalidState)?;
    let wire = DealerInstructionV1::<2>::CreateLpPosition(request);
    let data = encode_instruction(wire)?;
    let accounts = account_frame::<2>(
        program_id,
        DealerActionV1::CreateLpPosition,
        &[
            &state.payer,
            &state.owner,
            &state.pool.market,
            &state.pool.pool,
            &state.pool.config,
            &state.pool.config_staging,
            &state.lp_position,
            &state.lp_rent_credit,
            &state.system_program,
            &state.rent_sysvar,
        ],
        data,
    )?;
    Ok(DealerCreateLpReport {
        instruction: accounts,
        observation,
        required_signers: if state.payer.key == state.owner.key {
            vec![state.payer.key]
        } else {
            vec![state.payer.key, state.owner.key]
        },
        lp_position: state.lp_position.key,
        pool_sequence: facts.pool_sequence,
        rent_debit_lamports,
    })
}

/// Construct exact AddLiquidity material from observed Pool and LP state.
pub fn build_dealer_add_liquidity_v1(
    program_id: Pubkey,
    state: &DealerLiquidityState,
    choice: &DealerLiquidityChoice,
) -> Result<DealerLiquidityReport, VerticalError> {
    build_liquidity(program_id, state, choice, true)
}

/// Construct exact RemoveLiquidity material from observed Pool and LP state.
pub fn build_dealer_remove_liquidity_v1(
    program_id: Pubkey,
    state: &DealerLiquidityState,
    choice: &DealerLiquidityChoice,
) -> Result<DealerLiquidityReport, VerticalError> {
    build_liquidity(program_id, state, choice, false)
}

fn build_liquidity(
    program_id: Pubkey,
    state: &DealerLiquidityState,
    choice: &DealerLiquidityChoice,
    add: bool,
) -> Result<DealerLiquidityReport, VerticalError> {
    let observation = observation(&[
        &state.owner,
        &state.realm,
        &state.pool.market,
        &state.pool.pool,
        &state.pool.config,
        &state.pool.config_staging,
        &state.lp_position,
        &state.owner_position,
        &state.pool_position,
        &state.owner_collateral,
        &state.principal_vault,
        &state.fee_vault,
        &state.collateral_mint,
        &state.token_program,
    ])?;
    authenticate_system_actor(&state.owner)?;
    let action = if add {
        DealerActionV1::AddLiquidity
    } else {
        DealerActionV1::RemoveLiquidity
    };
    let facts = authenticate_pool(program_id, &state.pool, action)?;
    if choice.claim_limits.len() != facts.outcomes {
        return Err(VerticalError::InvalidState);
    }
    match facts.outcomes {
        2 => liquidity_width::<2>(program_id, state, choice, observation, facts, add),
        3 => liquidity_width::<3>(program_id, state, choice, observation, facts, add),
        4 => liquidity_width::<4>(program_id, state, choice, observation, facts, add),
        5 => liquidity_width::<5>(program_id, state, choice, observation, facts, add),
        6 => liquidity_width::<6>(program_id, state, choice, observation, facts, add),
        7 => liquidity_width::<7>(program_id, state, choice, observation, facts, add),
        8 => liquidity_width::<8>(program_id, state, choice, observation, facts, add),
        9 => liquidity_width::<9>(program_id, state, choice, observation, facts, add),
        10 => liquidity_width::<10>(program_id, state, choice, observation, facts, add),
        11 => liquidity_width::<11>(program_id, state, choice, observation, facts, add),
        12 => liquidity_width::<12>(program_id, state, choice, observation, facts, add),
        13 => liquidity_width::<13>(program_id, state, choice, observation, facts, add),
        14 => liquidity_width::<14>(program_id, state, choice, observation, facts, add),
        15 => liquidity_width::<15>(program_id, state, choice, observation, facts, add),
        16 => liquidity_width::<16>(program_id, state, choice, observation, facts, add),
        _ => Err(VerticalError::AbiUnavailable),
    }
}

fn liquidity_width<const N: usize>(
    program_id: Pubkey,
    state: &DealerLiquidityState,
    choice: &DealerLiquidityChoice,
    observation: Observation,
    facts: PoolFacts,
    add: bool,
) -> Result<DealerLiquidityReport, VerticalError> {
    let realm = authenticate_realm(
        program_id,
        &state.realm,
        &state.collateral_mint,
        &state.token_program,
        facts.realm_id,
    )?;
    let mut lp = authenticate_existing_lp(
        program_id,
        &state.lp_position,
        state.pool.pool.key,
        state.owner.key,
        facts.generation,
    )?;
    let owner_position = authenticate_position::<N>(
        program_id,
        &state.owner_position,
        state.pool.market.key,
        state.owner.key,
        facts.generation,
    )?;
    let pool_position = authenticate_position::<N>(
        program_id,
        &state.pool_position,
        state.pool.market.key,
        state.pool.pool.key,
        facts.generation,
    )?;
    let config =
        LiquidityConfigViewV1::new(facts.config_id, facts.profile, &state.pool.config.data)
            .map_err(|_| VerticalError::InvalidState)?;
    let pool_view = PoolViewV1::new(
        facts.profile,
        &state.pool.pool.data,
        state.pool.pool.key.to_bytes(),
        config,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    require_claim_coverage(
        pool_view
            .liquidity::<N>()
            .map_err(|_| VerticalError::InvalidState)?,
        &pool_position,
    )?;
    let principal_token = authenticate_pool_vault(
        program_id,
        &state.principal_vault,
        state.pool.pool.key,
        &state.collateral_mint,
        &state.token_program,
        realm,
        DealerCollateralCompartmentV1::Principal,
        pool_view
            .principal_collateral()
            .map_err(|_| VerticalError::InvalidState)?,
    )?;
    let fee_token = authenticate_pool_vault(
        program_id,
        &state.fee_vault,
        state.pool.pool.key,
        &state.collateral_mint,
        &state.token_program,
        realm,
        DealerCollateralCompartmentV1::RealizedFees,
        pool_view
            .realized_fee_collateral()
            .map_err(|_| VerticalError::InvalidState)?,
    )?;
    let owner_token = authenticate_user_vault(
        &state.owner_collateral,
        &state.collateral_mint,
        &state.token_program,
        realm,
        state.owner.key,
    )?;
    let limits = amounts::<N>(choice)?;
    let position_sequence = lp.next_sequence();
    let mut pool_bytes = state.pool.pool.data.clone();
    let receipt = if add {
        add_liquidity(
            &mut pool_bytes,
            facts.profile,
            state.pool.pool.key.to_bytes(),
            config,
            state.lp_position.key.to_bytes(),
            &mut lp,
            dclutch_dealer_contract::AddLiquidityRequest::new(
                facts.pool_sequence,
                position_sequence,
                choice.shares,
                limits,
            )
            .map_err(|_| VerticalError::InvalidState)?,
        )
    } else {
        remove_liquidity(
            &mut pool_bytes,
            facts.profile,
            state.pool.pool.key.to_bytes(),
            config,
            state.lp_position.key.to_bytes(),
            &mut lp,
            dclutch_dealer_contract::RemoveLiquidityRequest::new(
                facts.pool_sequence,
                position_sequence,
                choice.shares,
                limits,
            )
            .map_err(|_| VerticalError::InvalidState)?,
        )
    }
    .map_err(|_| VerticalError::InvalidState)?;
    let moved = receipt.amounts_transferred();
    let principal = moved.principal_collateral();
    let fees = moved.realized_fee_collateral();
    if add {
        let collateral = principal
            .checked_add(fees)
            .ok_or(VerticalError::InvalidState)?;
        if owner_token.amount < collateral {
            return Err(VerticalError::InvalidState);
        }
        require_position_debits(&owner_position, &moved.claim_reserves())?;
    } else {
        principal_token
            .amount
            .checked_sub(principal)
            .ok_or(VerticalError::InvalidState)?;
        fee_token
            .amount
            .checked_sub(fees)
            .ok_or(VerticalError::InvalidState)?;
        owner_token
            .amount
            .checked_add(principal)
            .and_then(|v| v.checked_add(fees))
            .ok_or(VerticalError::InvalidState)?;
        require_position_debits(&pool_position, &moved.claim_reserves())?;
    }
    let instruction = if add {
        let request = AddLiquidityV1::new(
            facts.pool_sequence,
            position_sequence,
            choice.shares,
            limits,
        )
        .map_err(|_| VerticalError::InvalidState)?;
        encode_instruction(DealerInstructionV1::<N>::AddLiquidity(request))?
    } else {
        let request = RemoveLiquidityV1::new(
            facts.pool_sequence,
            position_sequence,
            choice.shares,
            limits,
        )
        .map_err(|_| VerticalError::InvalidState)?;
        encode_instruction(DealerInstructionV1::<N>::RemoveLiquidity(request))?
    };
    let action = if add {
        DealerActionV1::AddLiquidity
    } else {
        DealerActionV1::RemoveLiquidity
    };
    let instruction = account_frame::<N>(
        program_id,
        action,
        &[
            &state.owner,
            &state.realm,
            &state.pool.market,
            &state.pool.pool,
            &state.pool.config,
            &state.pool.config_staging,
            &state.lp_position,
            &state.owner_position,
            &state.pool_position,
            &state.owner_collateral,
            &state.principal_vault,
            &state.fee_vault,
            &state.collateral_mint,
            &state.token_program,
        ],
        instruction,
    )?;
    Ok(DealerLiquidityReport {
        instruction,
        observation,
        required_signer: state.owner.key,
        lp_position: state.lp_position.key,
        pool_sequence: facts.pool_sequence,
        position_sequence,
        movement: DealerLiquidityMovement {
            principal_collateral: principal,
            realized_fee_collateral: fees,
            claims: moved.claim_reserves().to_vec(),
            shares: choice.shares,
            into_pool: add,
        },
    })
}

/// Construct one immediate trade from exact current reset/sequence state.
pub fn build_dealer_trade_v1(
    program_id: Pubkey,
    state: &DealerTradeState,
    choice: DealerTradeChoice,
) -> Result<DealerTradeReport, VerticalError> {
    let observation = observation(&[
        &state.trader,
        &state.realm,
        &state.pool.market,
        &state.pool.pool,
        &state.pool.config,
        &state.pool.config_staging,
        &state.trader_position,
        &state.pool_position,
        &state.trader_collateral,
        &state.principal_vault,
        &state.fee_vault,
        &state.collateral_mint,
        &state.token_program,
    ])?;
    authenticate_system_actor(&state.trader)?;
    let facts = authenticate_pool(program_id, &state.pool, DealerActionV1::Trade)?;
    match facts.outcomes {
        2 => trade_width::<2>(program_id, state, choice, observation, facts),
        3 => trade_width::<3>(program_id, state, choice, observation, facts),
        4 => trade_width::<4>(program_id, state, choice, observation, facts),
        5 => trade_width::<5>(program_id, state, choice, observation, facts),
        6 => trade_width::<6>(program_id, state, choice, observation, facts),
        7 => trade_width::<7>(program_id, state, choice, observation, facts),
        8 => trade_width::<8>(program_id, state, choice, observation, facts),
        9 => trade_width::<9>(program_id, state, choice, observation, facts),
        10 => trade_width::<10>(program_id, state, choice, observation, facts),
        11 => trade_width::<11>(program_id, state, choice, observation, facts),
        12 => trade_width::<12>(program_id, state, choice, observation, facts),
        13 => trade_width::<13>(program_id, state, choice, observation, facts),
        14 => trade_width::<14>(program_id, state, choice, observation, facts),
        15 => trade_width::<15>(program_id, state, choice, observation, facts),
        16 => trade_width::<16>(program_id, state, choice, observation, facts),
        _ => Err(VerticalError::AbiUnavailable),
    }
}

fn trade_width<const N: usize>(
    program_id: Pubkey,
    state: &DealerTradeState,
    choice: DealerTradeChoice,
    observation: Observation,
    facts: PoolFacts,
) -> Result<DealerTradeReport, VerticalError> {
    let realm = authenticate_realm(
        program_id,
        &state.realm,
        &state.collateral_mint,
        &state.token_program,
        facts.realm_id,
    )?;
    let trader_position = authenticate_position::<N>(
        program_id,
        &state.trader_position,
        state.pool.market.key,
        state.trader.key,
        facts.generation,
    )?;
    let pool_position = authenticate_position::<N>(
        program_id,
        &state.pool_position,
        state.pool.market.key,
        state.pool.pool.key,
        facts.generation,
    )?;
    let config =
        LiquidityConfigViewV1::new(facts.config_id, facts.profile, &state.pool.config.data)
            .map_err(|_| VerticalError::InvalidState)?;
    let pool_view = PoolViewV1::new(
        facts.profile,
        &state.pool.pool.data,
        state.pool.pool.key.to_bytes(),
        config,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    require_claim_coverage(
        pool_view
            .liquidity::<N>()
            .map_err(|_| VerticalError::InvalidState)?,
        &pool_position,
    )?;
    let principal_token = authenticate_pool_vault(
        program_id,
        &state.principal_vault,
        state.pool.pool.key,
        &state.collateral_mint,
        &state.token_program,
        realm,
        DealerCollateralCompartmentV1::Principal,
        pool_view
            .principal_collateral()
            .map_err(|_| VerticalError::InvalidState)?,
    )?;
    let fee_token = authenticate_pool_vault(
        program_id,
        &state.fee_vault,
        state.pool.pool.key,
        &state.collateral_mint,
        &state.token_program,
        realm,
        DealerCollateralCompartmentV1::RealizedFees,
        pool_view
            .realized_fee_collateral()
            .map_err(|_| VerticalError::InvalidState)?,
    )?;
    let trader_token = authenticate_user_vault(
        &state.trader_collateral,
        &state.collateral_mint,
        &state.token_program,
        realm,
        state.trader.key,
    )?;
    let claim_index = usize::from(choice.claim_index);
    let request = TradeRequest::new(
        facts.reset_number,
        facts.pool_sequence,
        choice.side,
        claim_index,
        choice.quantity,
        choice.collateral_limit,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    let mut pool_bytes = state.pool.pool.data.clone();
    let receipt = execute(
        &mut pool_bytes,
        facts.profile,
        state.pool.pool.key.to_bytes(),
        config,
        request,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    match choice.side {
        TradeSide::BuyClaimFromPool => {
            if trader_token.amount
                < receipt
                    .notional_collateral()
                    .checked_add(receipt.trader_fee_collateral())
                    .ok_or(VerticalError::InvalidState)?
            {
                return Err(VerticalError::InvalidState);
            }
            require_position_debit(&pool_position, claim_index, choice.quantity)?;
        }
        TradeSide::SellClaimToPool => {
            if trader_token.amount < receipt.trader_fee_collateral()
                || principal_token.amount < receipt.notional_collateral()
            {
                return Err(VerticalError::InvalidState);
            }
            fee_token
                .amount
                .checked_add(receipt.trader_fee_collateral())
                .ok_or(VerticalError::InvalidState)?;
            require_position_debit(&trader_position, claim_index, choice.quantity)?;
        }
    }
    let data = encode_instruction(DealerInstructionV1::<N>::Trade(request))?;
    let instruction = account_frame::<N>(
        program_id,
        DealerActionV1::Trade,
        &[
            &state.trader,
            &state.realm,
            &state.pool.market,
            &state.pool.pool,
            &state.pool.config,
            &state.pool.config_staging,
            &state.trader_position,
            &state.pool_position,
            &state.trader_collateral,
            &state.principal_vault,
            &state.fee_vault,
            &state.collateral_mint,
            &state.token_program,
        ],
        data,
    )?;
    let movement = DealerTradeMovement {
        principal_collateral: receipt.notional_collateral(),
        fee_collateral: receipt.trader_fee_collateral(),
        trader_collateral_debit: match choice.side {
            TradeSide::BuyClaimFromPool => receipt
                .notional_collateral()
                .checked_add(receipt.trader_fee_collateral())
                .ok_or(VerticalError::InvalidState)?,
            TradeSide::SellClaimToPool => receipt.trader_fee_collateral(),
        },
        trader_collateral_credit: match choice.side {
            TradeSide::BuyClaimFromPool => 0,
            TradeSide::SellClaimToPool => receipt.notional_collateral(),
        },
        trader_claim_debit: if matches!(choice.side, TradeSide::SellClaimToPool) {
            choice.quantity
        } else {
            0
        },
        trader_claim_credit: if matches!(choice.side, TradeSide::BuyClaimFromPool) {
            choice.quantity
        } else {
            0
        },
    };
    Ok(DealerTradeReport {
        instruction,
        observation,
        required_signer: state.trader.key,
        reset_number: facts.reset_number,
        pool_sequence: facts.pool_sequence,
        movement,
    })
}

/// Construct an exact close for the observed physical empty LP account.
pub fn build_dealer_close_lp_position_v1(
    program_id: Pubkey,
    state: &DealerCloseLpState,
) -> Result<DealerCloseLpReport, VerticalError> {
    let observation = observation(&[
        &state.owner,
        &state.pool.market,
        &state.pool.pool,
        &state.pool.config,
        &state.pool.config_staging,
        &state.lp_position,
        &state.lp_rent_credit,
        &state.system_program,
    ])?;
    authenticate_system_actor(&state.owner)?;
    authenticate_system_program(&state.system_program)?;
    let facts = authenticate_pool(program_id, &state.pool, DealerActionV1::CloseLpPosition)?;
    let mut lp = authenticate_existing_lp(
        program_id,
        &state.lp_position,
        state.pool.pool.key,
        state.owner.key,
        facts.generation,
    )?;
    let position_sequence = lp.next_sequence();
    authenticate_rent_credit(program_id, &state.lp_rent_credit, state.owner.key)
        .map_err(|_| VerticalError::ContentMismatch)?;
    let config =
        LiquidityConfigViewV1::new(facts.config_id, facts.profile, &state.pool.config.data)
            .map_err(|_| VerticalError::InvalidState)?;
    let mut pool_bytes = state.pool.pool.data.clone();
    let receipt = close_position(
        &mut pool_bytes,
        facts.profile,
        state.pool.pool.key.to_bytes(),
        config,
        state.lp_position.key.to_bytes(),
        &mut lp,
        facts.pool_sequence,
        position_sequence,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    if receipt.rent_credit() != lp.rent_credit()
        || state.lp_position.lamports < lp.rent_credit().funded_rent_principal()
    {
        return Err(VerticalError::InvalidState);
    }
    let request = CloseLpPositionV1::new(facts.pool_sequence, position_sequence)
        .map_err(|_| VerticalError::InvalidState)?;
    let data = encode_instruction(DealerInstructionV1::<2>::CloseLpPosition(request))?;
    let instruction = account_frame::<2>(
        program_id,
        DealerActionV1::CloseLpPosition,
        &[
            &state.owner,
            &state.pool.market,
            &state.pool.pool,
            &state.pool.config,
            &state.pool.config_staging,
            &state.lp_position,
            &state.lp_rent_credit,
            &state.system_program,
        ],
        data,
    )?;
    Ok(DealerCloseLpReport {
        instruction,
        observation,
        required_signer: state.owner.key,
        lp_position: state.lp_position.key,
        pool_sequence: facts.pool_sequence,
        position_sequence,
        rent_credit_lamports: state.lp_position.lamports,
    })
}

/// Construct exact permissionless retirement for a quiescent Pool.
pub fn build_dealer_retire_pool_v1(
    program_id: Pubkey,
    state: &DealerRetirePoolState,
) -> Result<DealerRetirePoolReport, VerticalError> {
    let observation = observation(&[
        &state.pool.market,
        &state.realm,
        &state.pool.pool,
        &state.pool.config,
        &state.pool.config_staging,
        &state.pool_position,
        &state.principal_vault,
        &state.fee_vault,
        &state.service_vault,
        &state.service_refund_vault,
        &state.refund_position,
        &state.pool_position_rent_credit,
        &state.pool_rent_credit,
        &state.collateral_mint,
        &state.token_program,
        &state.system_program,
    ])?;
    authenticate_system_program(&state.system_program)?;
    let facts = authenticate_pool(program_id, &state.pool, DealerActionV1::RetirePool)?;
    match facts.outcomes {
        2 => retire_width::<2>(program_id, state, observation, facts),
        3 => retire_width::<3>(program_id, state, observation, facts),
        4 => retire_width::<4>(program_id, state, observation, facts),
        5 => retire_width::<5>(program_id, state, observation, facts),
        6 => retire_width::<6>(program_id, state, observation, facts),
        7 => retire_width::<7>(program_id, state, observation, facts),
        8 => retire_width::<8>(program_id, state, observation, facts),
        9 => retire_width::<9>(program_id, state, observation, facts),
        10 => retire_width::<10>(program_id, state, observation, facts),
        11 => retire_width::<11>(program_id, state, observation, facts),
        12 => retire_width::<12>(program_id, state, observation, facts),
        13 => retire_width::<13>(program_id, state, observation, facts),
        14 => retire_width::<14>(program_id, state, observation, facts),
        15 => retire_width::<15>(program_id, state, observation, facts),
        16 => retire_width::<16>(program_id, state, observation, facts),
        _ => Err(VerticalError::AbiUnavailable),
    }
}

fn retire_width<const N: usize>(
    program_id: Pubkey,
    state: &DealerRetirePoolState,
    observation: Observation,
    facts: PoolFacts,
) -> Result<DealerRetirePoolReport, VerticalError> {
    let realm = authenticate_realm(
        program_id,
        &state.realm,
        &state.collateral_mint,
        &state.token_program,
        facts.realm_id,
    )?;
    let pool_position = authenticate_position::<N>(
        program_id,
        &state.pool_position,
        state.pool.market.key,
        state.pool.pool.key,
        facts.generation,
    )?;
    let config =
        LiquidityConfigViewV1::new(facts.config_id, facts.profile, &state.pool.config.data)
            .map_err(|_| VerticalError::InvalidState)?;
    let pool = PoolViewV1::new(
        facts.profile,
        &state.pool.pool.data,
        state.pool.pool.key.to_bytes(),
        config,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    let beneficiary = Pubkey::new_from_array(
        pool.attachment()
            .map_err(|_| VerticalError::InvalidState)?
            .service_refund_beneficiary(),
    );
    let refund_position = authenticate_position::<N>(
        program_id,
        &state.refund_position,
        state.pool.market.key,
        beneficiary,
        facts.generation,
    )?;
    let principal = authenticate_pool_vault(
        program_id,
        &state.principal_vault,
        state.pool.pool.key,
        &state.collateral_mint,
        &state.token_program,
        realm,
        DealerCollateralCompartmentV1::Principal,
        pool.principal_collateral()
            .map_err(|_| VerticalError::InvalidState)?,
    )?;
    let fees = authenticate_pool_vault(
        program_id,
        &state.fee_vault,
        state.pool.pool.key,
        &state.collateral_mint,
        &state.token_program,
        realm,
        DealerCollateralCompartmentV1::RealizedFees,
        pool.realized_fee_collateral()
            .map_err(|_| VerticalError::InvalidState)?,
    )?;
    let service = authenticate_pool_vault(
        program_id,
        &state.service_vault,
        state.pool.pool.key,
        &state.collateral_mint,
        &state.token_program,
        realm,
        DealerCollateralCompartmentV1::Service,
        pool.service_funding()
            .map_err(|_| VerticalError::InvalidState)?,
    )?;
    let refund = authenticate_user_vault(
        &state.service_refund_vault,
        &state.collateral_mint,
        &state.token_program,
        realm,
        beneficiary,
    )?;
    refund
        .amount
        .checked_add(principal.amount)
        .and_then(|v| v.checked_add(fees.amount))
        .and_then(|v| v.checked_add(service.amount))
        .ok_or(VerticalError::InvalidState)?;
    for (before, gift) in refund_position
        .balances()
        .iter()
        .zip(pool_position.balances())
    {
        before
            .checked_add(*gift)
            .ok_or(VerticalError::InvalidState)?;
    }
    authenticate_rent_credit(
        program_id,
        &state.pool_position_rent_credit,
        state.pool.pool.key,
    )
    .map_err(|_| VerticalError::ContentMismatch)?;
    let pool_beneficiary = Pubkey::new_from_array(
        pool.rent_credit()
            .map_err(|_| VerticalError::InvalidState)?
            .beneficiary(),
    );
    authenticate_rent_credit(program_id, &state.pool_rent_credit, pool_beneficiary)
        .map_err(|_| VerticalError::ContentMismatch)?;
    let mut pool_bytes = state.pool.pool.data.clone();
    let plan = retire_pool_in_place(
        market_root::<N>(program_id, &state.pool.market)?,
        &mut pool_bytes,
        facts.profile,
        state.pool.pool.key.to_bytes(),
        config,
        facts.pool_sequence,
        facts.market_child_count,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    if plan.receipt().service_refund_collateral()
        != pool
            .service_funding()
            .map_err(|_| VerticalError::InvalidState)?
    {
        return Err(VerticalError::ContentMismatch);
    }
    let data = encode_instruction(DealerInstructionV1::<N>::RetirePool {
        expected_pool_sequence: facts.pool_sequence,
        expected_market_child_count: facts.market_child_count,
    })?;
    let instruction = account_frame::<N>(
        program_id,
        DealerActionV1::RetirePool,
        &[
            &state.pool.market,
            &state.realm,
            &state.pool.pool,
            &state.pool.config,
            &state.pool.config_staging,
            &state.pool_position,
            &state.principal_vault,
            &state.fee_vault,
            &state.service_vault,
            &state.service_refund_vault,
            &state.refund_position,
            &state.pool_position_rent_credit,
            &state.pool_rent_credit,
            &state.collateral_mint,
            &state.token_program,
            &state.system_program,
        ],
        data,
    )?;
    let pool_rent_credit_lamports = state
        .pool
        .pool
        .lamports
        .checked_add(state.principal_vault.lamports)
        .and_then(|v| v.checked_add(state.fee_vault.lamports))
        .and_then(|v| v.checked_add(state.service_vault.lamports))
        .ok_or(VerticalError::InvalidState)?;
    Ok(DealerRetirePoolReport {
        instruction,
        observation,
        pool_sequence: facts.pool_sequence,
        market_child_count: facts.market_child_count,
        principal_refund: principal.amount,
        fee_refund: fees.amount,
        service_refund: service.amount,
        claim_refunds: pool_position.balances().to_vec(),
        pool_rent_credit_lamports,
        pool_position_rent_credit_lamports: state.pool_position.lamports,
    })
}

#[derive(Clone, Copy)]
struct PoolFacts {
    outcomes: usize,
    profile: LiquidityProfileV1,
    config_id: ContentId,
    generation: u64,
    realm_id: [u8; 32],
    pool_sequence: u64,
    reset_number: u64,
    market_child_count: u64,
}

fn authenticate_pool(
    program_id: Pubkey,
    state: &DealerPoolState,
    action: DealerActionV1,
) -> Result<PoolFacts, VerticalError> {
    let market = market_facts(program_id, &state.market)?;
    validate_market_phase(action, market.phase).map_err(|_| VerticalError::InvalidPhase)?;
    if state.config.owner != program_id || state.config.executable {
        return Err(VerticalError::InvalidOwner);
    }
    let config_digest = hash(&state.config.data).to_bytes();
    let config_id = ContentId::new(config_digest).map_err(|_| VerticalError::InvalidState)?;
    let profile = LiquidityProfileV1::from_config_len(market.outcomes, state.config.data.len())
        .map_err(|_| VerticalError::InvalidState)?;
    let config = LiquidityConfigViewV1::new(config_id, profile, &state.config.data)
        .map_err(|_| VerticalError::InvalidState)?;
    let expected_config = Pubkey::find_program_address(
        &ConfigPdaSeedsV1::new(config_id).seed_components(),
        &program_id,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &DEALER_CONFIG_SCHEMA_RELEASE_ID_V1,
            &config_digest,
        ],
        &program_id,
    )
    .0;
    if state.config.key != expected_config
        || state.config_staging.key != expected_staging
        || state.config_staging.owner != system_program::ID
        || state.config_staging.executable
        || !state.config_staging.data.is_empty()
    {
        return Err(VerticalError::FinalizationMismatch);
    }
    let pool = PoolViewV1::new(profile, &state.pool.data, state.pool.key.to_bytes(), config)
        .map_err(|_| VerticalError::InvalidState)?;
    let attachment = pool.attachment().map_err(|_| VerticalError::InvalidState)?;
    if state.pool.owner != program_id
        || state.pool.executable
        || attachment.market() != market.identity
        || attachment.liquidity_config_id() != config_id
    {
        return Err(VerticalError::ContentMismatch);
    }
    let seeds = PoolPdaSeedsV1::new(state.market.key.to_bytes(), market.generation, config_id)
        .map_err(|_| VerticalError::PdaMismatch)?;
    if state.pool.key != Pubkey::find_program_address(&seeds.seed_components(), &program_id).0 {
        return Err(VerticalError::PdaMismatch);
    }
    Ok(PoolFacts {
        outcomes: market.outcomes,
        profile,
        config_id,
        generation: market.generation,
        realm_id: market.realm_id,
        pool_sequence: pool
            .next_sequence()
            .map_err(|_| VerticalError::InvalidState)?,
        reset_number: pool
            .reset_number()
            .map_err(|_| VerticalError::InvalidState)?,
        market_child_count: market.child_count,
    })
}

#[derive(Clone, Copy)]
struct MarketFacts {
    outcomes: usize,
    generation: u64,
    phase: Phase,
    child_count: u64,
    realm_id: [u8; 32],
    identity: dclutch_core_contract::MarketIdentity,
}

fn market_facts(
    program_id: Pubkey,
    account: &ObservedAccount,
) -> Result<MarketFacts, VerticalError> {
    match decode_market_outcome_count(&account.data).map_err(|_| VerticalError::InvalidState)? {
        2 => market_width::<2>(program_id, account),
        3 => market_width::<3>(program_id, account),
        4 => market_width::<4>(program_id, account),
        5 => market_width::<5>(program_id, account),
        6 => market_width::<6>(program_id, account),
        7 => market_width::<7>(program_id, account),
        8 => market_width::<8>(program_id, account),
        9 => market_width::<9>(program_id, account),
        10 => market_width::<10>(program_id, account),
        11 => market_width::<11>(program_id, account),
        12 => market_width::<12>(program_id, account),
        13 => market_width::<13>(program_id, account),
        14 => market_width::<14>(program_id, account),
        15 => market_width::<15>(program_id, account),
        16 => market_width::<16>(program_id, account),
        _ => Err(VerticalError::AbiUnavailable),
    }
}

fn market_width<const N: usize>(
    program_id: Pubkey,
    account: &ObservedAccount,
) -> Result<MarketFacts, VerticalError> {
    let market = decode_owned(account, program_id, CategoricalMarketV1::<N>::decode)?;
    let mut canonical =
        vec![0; CategoricalMarketV1::<N>::encoded_len().map_err(|_| VerticalError::InvalidState)?];
    market
        .encode(&mut canonical)
        .map_err(|_| VerticalError::InvalidState)?;
    let root = market.root();
    let identity = root.identity();
    let digest = hash(&identity.to_bytes()).to_bytes();
    let expected = Pubkey::find_program_address(&[crate::MARKET_SEED, &digest], &program_id).0;
    if canonical != account.data || account.key != expected {
        return Err(VerticalError::PdaMismatch);
    }
    Ok(MarketFacts {
        outcomes: N,
        generation: identity.generation(),
        phase: root.phase(),
        child_count: root.outstanding_children(),
        realm_id: identity.realm_id().to_bytes(),
        identity,
    })
}

fn market_root<const N: usize>(
    program_id: Pubkey,
    account: &ObservedAccount,
) -> Result<dclutch_core_contract::MarketRoot, VerticalError> {
    let market = decode_owned(account, program_id, CategoricalMarketV1::<N>::decode)?;
    Ok(market.root())
}

fn authenticate_existing_lp(
    program_id: Pubkey,
    account: &ObservedAccount,
    pool: Pubkey,
    owner: Pubkey,
    generation: u64,
) -> Result<LpPosition, VerticalError> {
    let lp = decode_owned(account, program_id, LpPosition::decode)?;
    if lp
        .to_bytes()
        .map_err(|_| VerticalError::InvalidState)?
        .as_slice()
        != account.data.as_slice()
        || lp.parent().address() != pool.to_bytes()
        || lp.parent().market_generation() != generation
        || lp.owner() != owner.to_bytes()
        || lp.rent_credit().beneficiary() != owner.to_bytes()
        || account.lamports < lp.rent_credit().funded_rent_principal()
    {
        return Err(VerticalError::ContentMismatch);
    }
    Ok(lp)
}

fn authenticate_position<const N: usize>(
    program_id: Pubkey,
    account: &ObservedAccount,
    market: Pubkey,
    owner: Pubkey,
    generation: u64,
) -> Result<PositionV1<N>, VerticalError> {
    let position = decode_owned(account, program_id, PositionV1::<N>::decode)?;
    let mut canonical =
        vec![0; PositionV1::<N>::encoded_len().map_err(|_| VerticalError::InvalidState)?];
    position
        .encode(&mut canonical)
        .map_err(|_| VerticalError::InvalidState)?;
    let expected = Pubkey::find_program_address(
        &[POSITION_PDA_DOMAIN, market.as_ref(), owner.as_ref()],
        &program_id,
    )
    .0;
    if account.key != expected
        || canonical != account.data
        || position.market() != market.as_ref()
        || position.owner() != owner.as_ref()
        || position.generation() != generation
    {
        return Err(VerticalError::PdaMismatch);
    }
    Ok(position)
}

#[derive(Clone, Copy)]
struct RealmFacts {
    release: dclutch_token_svm::CollateralAdapterReleaseV1,
}

fn authenticate_realm(
    program_id: Pubkey,
    account: &ObservedAccount,
    mint: &ObservedAccount,
    token_program: &ObservedAccount,
    selected_realm_id: [u8; 32],
) -> Result<RealmFacts, VerticalError> {
    if account.owner != program_id
        || account.executable
        || mint.owner != token_program.key
        || mint.executable
        || !token_program.executable
        || !matches!(token_program.owner, key if key == bpf_loader::ID || key == bpf_loader_upgradeable::ID)
    {
        return Err(VerticalError::InvalidOwner);
    }
    let realm = RealmV1::decode(&account.data).map_err(|_| VerticalError::InvalidState)?;
    let realm_id = hash(&account.data).to_bytes();
    let expected = Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_id], &program_id).0;
    if realm.to_bytes().as_slice() != account.data.as_slice()
        || account.key != expected
        || realm_id != selected_realm_id
        || realm.token_program() != token_program.key.as_ref()
        || realm.collateral_mint() != mint.key.as_ref()
    {
        return Err(VerticalError::ContentMismatch);
    }
    let release = PRODUCTION_ADAPTER_RELEASES
        .into_iter()
        .find(|release| {
            hash(&release.to_bytes()).to_bytes() == *realm.collateral_adapter_release_id()
        })
        .ok_or(VerticalError::ContentMismatch)?;
    let parsed_mint = release
        .profile()
        .check_mint(token_program.key.to_bytes(), &mint.data)
        .map_err(|_| VerticalError::InvalidState)?;
    if release.token_program() != token_program.key.to_bytes()
        || matches!(
            realm.mint_authority_policy(),
            MintAuthorityPolicy::RequireAbsent
        ) && !matches!(parsed_mint.mint_authority, COption::None)
        || matches!(
            realm.freeze_authority_policy(),
            FreezeAuthorityPolicy::RequireAbsent
        ) && !matches!(parsed_mint.freeze_authority, COption::None)
    {
        return Err(VerticalError::ContentMismatch);
    }
    Ok(RealmFacts { release })
}

fn authenticate_pool_vault(
    program_id: Pubkey,
    account: &ObservedAccount,
    pool: Pubkey,
    mint: &ObservedAccount,
    token_program: &ObservedAccount,
    realm: RealmFacts,
    compartment: DealerCollateralCompartmentV1,
    minimum: u64,
) -> Result<TokenAccount, VerticalError> {
    let seeds = DealerCollateralVaultPdaSeedsV1::new(pool.to_bytes(), compartment)
        .map_err(|_| VerticalError::PdaMismatch)?;
    let expected = Pubkey::find_program_address(&seeds.seed_components(), &program_id).0;
    if account.key != expected || account.owner != token_program.key || account.executable {
        return Err(VerticalError::PdaMismatch);
    }
    let token = realm
        .release
        .profile()
        .check_custody_account(
            token_program.key.to_bytes(),
            &account.data,
            mint.key.to_bytes(),
            pool.to_bytes(),
        )
        .map_err(|_| VerticalError::InvalidState)?;
    if token.amount < minimum {
        return Err(VerticalError::InvalidState);
    }
    Ok(token)
}

fn authenticate_user_vault(
    account: &ObservedAccount,
    mint: &ObservedAccount,
    token_program: &ObservedAccount,
    realm: RealmFacts,
    authority: Pubkey,
) -> Result<TokenAccount, VerticalError> {
    if account.owner != token_program.key || account.executable {
        return Err(VerticalError::InvalidOwner);
    }
    let token = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &account.data)
        .map_err(|_| VerticalError::InvalidState)?;
    if token.mint != mint.key.to_bytes() || token.owner != authority.to_bytes() {
        return Err(VerticalError::ContentMismatch);
    }
    Ok(token)
}

fn require_claim_coverage<const N: usize>(
    liquidity: LiquidityAmounts<N>,
    position: &PositionV1<N>,
) -> Result<(), VerticalError> {
    if position
        .balances()
        .iter()
        .zip(liquidity.claim_reserves())
        .any(|(physical, categorized)| *physical < categorized)
    {
        Err(VerticalError::InvalidState)
    } else {
        Ok(())
    }
}

fn require_position_debits<const N: usize>(
    position: &PositionV1<N>,
    amounts: &[u64; N],
) -> Result<(), VerticalError> {
    if position
        .balances()
        .iter()
        .zip(amounts)
        .any(|(balance, debit)| balance < debit)
    {
        Err(VerticalError::InvalidState)
    } else {
        Ok(())
    }
}

fn require_position_debit<const N: usize>(
    position: &PositionV1<N>,
    outcome: usize,
    amount: u64,
) -> Result<(), VerticalError> {
    if position
        .balances()
        .get(outcome)
        .copied()
        .ok_or(VerticalError::InvalidState)?
        < amount
    {
        Err(VerticalError::InvalidState)
    } else {
        Ok(())
    }
}

fn amounts<const N: usize>(
    choice: &DealerLiquidityChoice,
) -> Result<LiquidityAmounts<N>, VerticalError> {
    let claims: [u64; N] = choice
        .claim_limits
        .as_slice()
        .try_into()
        .map_err(|_| VerticalError::InvalidState)?;
    LiquidityAmounts::new(choice.principal_limit, choice.fee_limit, claims)
        .map_err(|_| VerticalError::InvalidState)
}

fn require_vacant(account: &ObservedAccount) -> Result<(), VerticalError> {
    if account.owner == system_program::ID
        && !account.executable
        && account.lamports == 0
        && account.data.is_empty()
    {
        Ok(())
    } else {
        Err(VerticalError::InvalidState)
    }
}

fn encode_instruction<const N: usize>(
    instruction: DealerInstructionV1<N>,
) -> Result<Vec<u8>, VerticalError> {
    let mut data = vec![
        0;
        instruction
            .encoded_len()
            .map_err(|_| VerticalError::InvalidState)?
    ];
    instruction
        .encode_into(&mut data)
        .map_err(|_| VerticalError::InvalidState)?;
    Ok(data)
}

fn account_frame<const N: usize>(
    program_id: Pubkey,
    action: DealerActionV1,
    accounts: &[&ObservedAccount],
    data: Vec<u8>,
) -> Result<Instruction, VerticalError> {
    let mut frame = Vec::with_capacity(accounts.len());
    let mut metas = Vec::with_capacity(accounts.len());
    for (index, account) in accounts.iter().enumerate() {
        let role = dclutch_dealer_contract::frame::dealer_account_role::<N>(action, index)
            .map_err(|_| VerticalError::InvalidState)?;
        let (signer, writable, executable) = dealer_account_privileges(action, role);
        frame.push(DealerAccountMetaV1 {
            key: account.key.to_bytes(),
            is_signer: signer,
            is_writable: writable,
            is_executable: account.executable,
        });
        if executable != account.executable {
            return Err(VerticalError::InvalidOwner);
        }
        metas.push(if writable {
            AccountMeta::new(account.key, signer)
        } else {
            AccountMeta::new_readonly(account.key, signer)
        });
    }
    DealerFrameV1::<N>::new(action, &frame).map_err(|_| VerticalError::InvalidState)?;
    Ok(Instruction {
        program_id,
        accounts: metas,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Finality;
    use dclutch_core_contract::MarketIdentity;
    use dclutch_dealer_contract::{
        LiquidityAttachment, LiquidityConfigV1, RentCreditTerms, runtime::initialize_pool,
    };

    fn observed(key: Pubkey, owner: Pubkey, lamports: u64, data: Vec<u8>) -> ObservedAccount {
        ObservedAccount {
            observation: Observation {
                slot: 41,
                unix_timestamp: 1_800_000_000,
                finality: Finality::Finalized,
            },
            key,
            owner,
            lamports,
            executable: false,
            data,
        }
    }

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero fixture identity")
    }

    fn existing_lp() -> (Pubkey, Pubkey, Pubkey, ObservedAccount) {
        let program_id = Pubkey::new_from_array([90; 32]);
        let pool = Pubkey::new_from_array([91; 32]);
        let owner = Pubkey::new_from_array([92; 32]);
        let physical_lp = Pubkey::new_from_array([93; 32]);
        let config = LiquidityConfigV1::<2, 2>::new(
            id(1),
            owner.to_bytes(),
            10_000,
            25,
            2_500,
            100,
            [[4_000, 3_900]; 2],
            [[6_000, 6_100]; 2],
            [[1_000, 1_000]; 2],
            [[1_000, 1_000]; 2],
        )
        .expect("canonical config");
        let mut config_bytes = vec![0; LiquidityConfigV1::<2, 2>::encoded_len().expect("width")];
        config
            .encode_into(&mut config_bytes)
            .expect("encode config");
        let profile = LiquidityProfileV1::new(2, 2).expect("supported profile");
        let config_view =
            LiquidityConfigViewV1::new(id(1), profile, &config_bytes).expect("runtime config view");
        let identity = MarketIdentity::new(id(2), id(3), id(4), id(5), id(6), 7);
        let attachment =
            LiquidityAttachment::new(identity, id(7), id(1), owner.to_bytes()).expect("attachment");
        let rent = RentCreditTerms::new(owner.to_bytes(), 500).expect("rent terms");
        let pool_rent = RentCreditTerms::new(owner.to_bytes(), 1_000).expect("pool rent");
        let mut pool_bytes = vec![0; profile.pool_len().expect("pool width")];
        let (lp, _) = initialize_pool(
            &mut pool_bytes,
            profile,
            attachment,
            pool.to_bytes(),
            config_view,
            pool_rent,
            41,
            LiquidityAmounts::new(100_000, 0, [10_000, 10_000]).expect("liquidity"),
            5_000,
            physical_lp.to_bytes(),
            owner.to_bytes(),
            rent,
            1_000,
        )
        .expect("initialize fixture Pool");
        (
            program_id,
            pool,
            owner,
            observed(
                physical_lp,
                program_id,
                500,
                lp.to_bytes().expect("encode LP").to_vec(),
            ),
        )
    }

    #[test]
    fn existing_lp_physical_key_is_identity_without_seed_reconstruction() {
        let (program_id, pool, owner, account) = existing_lp();
        let lp = authenticate_existing_lp(program_id, &account, pool, owner, 7)
            .expect("arbitrary observed LP key is accepted after hostile state binding");
        assert_eq!(lp.owner(), owner.to_bytes());
        assert_eq!(account.key, Pubkey::new_from_array([93; 32]));
    }

    #[test]
    fn existing_lp_refuses_owner_generation_and_rent_mismatches() {
        let (program_id, pool, owner, mut account) = existing_lp();
        assert_eq!(
            authenticate_existing_lp(
                program_id,
                &account,
                pool,
                Pubkey::new_from_array([94; 32]),
                7,
            ),
            Err(VerticalError::ContentMismatch)
        );
        assert_eq!(
            authenticate_existing_lp(program_id, &account, pool, owner, 8),
            Err(VerticalError::ContentMismatch)
        );
        account.lamports = 499;
        assert_eq!(
            authenticate_existing_lp(program_id, &account, pool, owner, 7),
            Err(VerticalError::ContentMismatch)
        );
    }

    #[test]
    fn liquidity_limits_refuse_parallel_or_truncated_outcome_shapes() {
        let choice = DealerLiquidityChoice {
            shares: 1,
            principal_limit: 1,
            fee_limit: 0,
            claim_limits: vec![1],
        };
        assert_eq!(amounts::<2>(&choice), Err(VerticalError::InvalidState));
    }

    #[test]
    fn activation_remains_unavailable_without_shared_manifest_authority() {
        assert_eq!(
            build_dealer_activate_pool_v1(
                Pubkey::new_from_array([90; 32]),
                DealerActivateChoice {
                    initial_lp_id: [1; 32],
                    initial_claim_quantity: 1,
                    initial_shares: 1,
                },
            ),
            Err(VerticalError::AbiUnavailable)
        );
    }
}
