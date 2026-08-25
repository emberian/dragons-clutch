//! Chain-derived unsigned Dealer lifecycle construction.
//!
//! Existing LP positions are selected by their observed physical account key.
//! Only fresh activation/creation routes carry an irreducible LP seed choice.

use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityFundingAuthorityDerivationV1,
    CapabilityFundingDerivationV1, CapabilityFundingVaultDerivationV1, CapabilityManifestV1,
    FUNDING_STATE_BYTES, FundingCustodyObservationV1, FundingStateV1, RealmCollateralCustodyV1,
    RealmCollateralVaultObservationV1,
};
use dclutch_core_contract::{ContentId, Phase};
use dclutch_dealer_contract::{
    DEALER_CAPABILITY_KIND_ID_V1, DEALER_CAPABILITY_RELEASE_ID_V1, LP_POSITION_BYTES,
    LiquidityAmounts, LiquidityAttachment, LpPosition, RentCreditTerms, TradeRequest, TradeSide,
    activation::{activate_pool_into, retire_pool_in_place},
    frame::{
        ConfigPdaSeedsV1, DEALER_CONFIG_SCHEMA_RELEASE_ID_V1, DealerAccountMetaV1,
        DealerCollateralCompartmentV1, DealerCollateralVaultPdaSeedsV1, DealerFrameV1,
        LpPositionPdaSeedsV1, PoolPdaSeedsV1, PoolPositionPdaSeedsV1, dealer_account_privileges,
        validate_market_phase,
    },
    instruction::{
        ActivatePoolV1, AddLiquidityV1, CloseLpPositionV1, CreateLpPositionV1, DealerActionV1,
        DealerInstructionV1, RemoveLiquidityV1,
    },
    runtime::{
        LiquidityConfigViewV1, LiquidityProfileV1, PoolViewV1, add_liquidity, close_position,
        create_position, execute, remove_liquidity,
    },
};
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, POSITION_PDA_DOMAIN, PositionV1, REALM_PDA_DOMAIN,
    RealmV1,
};
use dclutch_record_contract::STAGING_CURSOR_PDA_SEED_V1;
use dclutch_token_svm::{ACCOUNT_BYTES, COption, PRODUCTION_ADAPTER_RELEASES, TokenAccount};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader, bpf_loader_upgradeable, system_program};

use crate::{
    Observation, ObservedAccount, authenticate_rent_credit,
    foundation::{self, FinalizedRecordProof},
};

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

/// Finalized accounts for atomic Dealer capability activation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerActivateState {
    /// Permissionless transaction payer reimbursed from capability funding.
    pub activator: ObservedAccount,
    /// Immutable bootstrap liquidity owner selected by config.
    pub owner: ObservedAccount,
    /// Immutable Realm selected by Market.
    pub realm: ObservedAccount,
    /// Founding or Open Market.
    pub market: ObservedAccount,
    /// Finalized capability manifest raw record selected by Market identity.
    pub capability_manifest: ObservedAccount,
    /// Exact finalization proof for the manifest record.
    pub capability_manifest_finalization: FinalizedRecordProof,
    /// Pending segregated capability funding state.
    pub funding_state: ObservedAccount,
    /// Vacant canonical capability funding authority PDA.
    pub funding_authority: ObservedAccount,
    /// Realm-collateral funding Vault.
    pub funding_collateral_vault: ObservedAccount,
    /// Finalized Dealer config raw record.
    pub config: ObservedAccount,
    /// Drained staging cursor proving Dealer config finalization.
    pub config_staging: ObservedAccount,
    /// Vacant canonical Pool PDA.
    pub pool: ObservedAccount,
    /// Vacant canonical initial LP PDA.
    pub lp_position: ObservedAccount,
    /// Config owner's canonical native Position supplying a complete set.
    pub owner_position: ObservedAccount,
    /// Vacant canonical Pool native Position PDA.
    pub pool_position: ObservedAccount,
    /// Vacant canonical principal Vault PDA.
    pub principal_vault: ObservedAccount,
    /// Vacant canonical realized-fee Vault PDA.
    pub fee_vault: ObservedAccount,
    /// Vacant canonical service Vault PDA.
    pub service_vault: ObservedAccount,
    /// Pool-authority RentCredit for Pool Position rent.
    pub pool_position_rent_credit: ObservedAccount,
    /// Owner RentCredit for Pool bundle rent.
    pub pool_rent_credit: ObservedAccount,
    /// Owner RentCredit for initial LP rent; may alias `pool_rent_credit`.
    pub lp_rent_credit: ObservedAccount,
    /// Realm collateral Mint.
    pub collateral_mint: ObservedAccount,
    /// Realm-selected executable token program.
    pub token_program: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
}

/// Fresh irreducible activation choices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerActivateChoice {
    /// Fresh nonzero LP PDA seed identity.
    pub initial_lp_id: [u8; 32],
    /// Complete-set claim quantity supplied by the immutable liquidity owner.
    pub initial_claim_quantity: u64,
    /// Initial LP shares.
    pub initial_shares: u64,
}

/// Exact activation funding, creation-rent, and value effects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerActivateReport {
    /// Exact unsigned 24-account SBF instruction.
    pub instruction: Instruction,
    /// One finalized observation selecting every account and proof.
    pub observation: Observation,
    /// Required transaction signers in role order.
    pub required_signers: Vec<Pubkey>,
    /// Fresh canonical Pool PDA.
    pub pool: Pubkey,
    /// Fresh canonical initial LP PDA.
    pub lp_position: Pubkey,
    /// Exact capability-funding Rent debit.
    pub rent_debit_lamports: u64,
    /// Exact Realm collateral moved into principal custody.
    pub principal_collateral: u64,
    /// Exact segregated service funding moved into service custody.
    pub service_collateral: u64,
    /// Complete-set claims moved from owner Position to Pool Position.
    pub claim_reserves: Vec<u64>,
    /// Market direct-child count before registration.
    pub market_child_count_before: u64,
    /// Market direct-child count after registration.
    pub market_child_count_after: u64,
}

/// Construct exact Dealer activation from finalized shared capability authority.
pub fn build_dealer_activate_pool_v1(
    program_id: Pubkey,
    state: &DealerActivateState,
    choice: DealerActivateChoice,
) -> Result<DealerActivateReport, VerticalError> {
    let observation = observation(&[
        &state.activator,
        &state.owner,
        &state.realm,
        &state.market,
        &state.capability_manifest,
        &state.capability_manifest_finalization.staging_cursor,
        &state.funding_state,
        &state.funding_authority,
        &state.funding_collateral_vault,
        &state.config,
        &state.config_staging,
        &state.pool,
        &state.lp_position,
        &state.owner_position,
        &state.pool_position,
        &state.principal_vault,
        &state.fee_vault,
        &state.service_vault,
        &state.pool_position_rent_credit,
        &state.pool_rent_credit,
        &state.lp_rent_credit,
        &state.collateral_mint,
        &state.token_program,
        &state.system_program,
        &state.rent_sysvar,
    ])?;
    authenticate_system_actor(&state.activator)?;
    authenticate_system_actor(&state.owner)?;
    authenticate_system_program(&state.system_program)?;
    let rent =
        foundation::decode_rent(&state.rent_sysvar).map_err(|_| VerticalError::InvalidState)?;
    let market = market_facts(program_id, &state.market)?;
    validate_market_phase(DealerActionV1::ActivatePool, market.phase)
        .map_err(|_| VerticalError::InvalidPhase)?;
    match market.outcomes {
        2 => activate_width::<2>(program_id, state, choice, observation, market, &rent),
        3 => activate_width::<3>(program_id, state, choice, observation, market, &rent),
        4 => activate_width::<4>(program_id, state, choice, observation, market, &rent),
        5 => activate_width::<5>(program_id, state, choice, observation, market, &rent),
        6 => activate_width::<6>(program_id, state, choice, observation, market, &rent),
        7 => activate_width::<7>(program_id, state, choice, observation, market, &rent),
        8 => activate_width::<8>(program_id, state, choice, observation, market, &rent),
        9 => activate_width::<9>(program_id, state, choice, observation, market, &rent),
        10 => activate_width::<10>(program_id, state, choice, observation, market, &rent),
        11 => activate_width::<11>(program_id, state, choice, observation, market, &rent),
        12 => activate_width::<12>(program_id, state, choice, observation, market, &rent),
        13 => activate_width::<13>(program_id, state, choice, observation, market, &rent),
        14 => activate_width::<14>(program_id, state, choice, observation, market, &rent),
        15 => activate_width::<15>(program_id, state, choice, observation, market, &rent),
        16 => activate_width::<16>(program_id, state, choice, observation, market, &rent),
        _ => Err(VerticalError::AbiUnavailable),
    }
}

#[allow(clippy::too_many_arguments)]
fn activate_width<const N: usize>(
    program_id: Pubkey,
    state: &DealerActivateState,
    choice: DealerActivateChoice,
    observation: Observation,
    market: MarketFacts,
    rent: &solana_program::rent::Rent,
) -> Result<DealerActivateReport, VerticalError> {
    require_manifest_schema(state.capability_manifest_finalization.schema_release_id)?;
    foundation::authenticate_finalized_record(
        program_id,
        rent,
        &state.capability_manifest,
        &state.capability_manifest_finalization,
    )
    .map_err(|_| VerticalError::FinalizationMismatch)?;
    let manifest_id = ContentId::new(hash(&state.capability_manifest.data).to_bytes())
        .map_err(|_| VerticalError::ContentMismatch)?;
    if manifest_id != market.identity.capability_manifest_id() {
        return Err(VerticalError::ContentMismatch);
    }
    let manifest = CapabilityManifestV1::decode(&state.capability_manifest.data)
        .map_err(|_| VerticalError::InvalidState)?;
    let (profile, config_id, config) =
        authenticate_config(program_id, &state.config, &state.config_staging, N)?;
    let funding = decode_owned(&state.funding_state, program_id, FundingStateV1::decode)?;
    if funding.to_bytes().as_slice() != state.funding_state.data.as_slice() {
        return Err(VerticalError::ContentMismatch);
    }
    let selected = manifest
        .entry(funding.entry_index())
        .map_err(|_| VerticalError::ContentMismatch)?;
    require_dealer_selection(
        selected.kind_id().to_bytes(),
        selected.release_id().to_bytes(),
        selected.config_id(),
        config_id,
    )?;
    let realm = authenticate_realm(
        program_id,
        &state.realm,
        &state.collateral_mint,
        &state.token_program,
        market.realm_id,
    )?;
    let binding = selected
        .funding_quote()
        .realm_collateral()
        .ok_or(VerticalError::ContentMismatch)?;
    if binding.realm_id().to_bytes() != market.realm_id
        || binding.collateral_release_id().to_bytes() != hash(&realm.release.to_bytes()).to_bytes()
        || binding.token_program() != state.token_program.key.to_bytes()
        || binding.mint() != state.collateral_mint.key.to_bytes()
    {
        return Err(VerticalError::ContentMismatch);
    }
    let funding_seeds = CapabilityFundingDerivationV1::new(
        state.market.key.to_bytes(),
        market.generation,
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| VerticalError::PdaMismatch)?;
    require_pda(
        state.funding_state.key,
        Pubkey::find_program_address(&funding_seeds.seed_components(), &program_id).0,
    )?;
    let authority_seeds =
        CapabilityFundingAuthorityDerivationV1::new(state.funding_state.key.to_bytes())
            .map_err(|_| VerticalError::PdaMismatch)?;
    require_pda(
        state.funding_authority.key,
        Pubkey::find_program_address(&authority_seeds.seed_components(), &program_id).0,
    )?;
    require_vacant(&state.funding_authority)?;
    let vault_seeds =
        CapabilityFundingVaultDerivationV1::new(state.funding_authority.key.to_bytes(), binding)
            .map_err(|_| VerticalError::PdaMismatch)?;
    if state.funding_collateral_vault.key
        != Pubkey::find_program_address(&vault_seeds.seed_components(), &program_id).0
        || state.funding_collateral_vault.owner != state.token_program.key
        || state.funding_collateral_vault.executable
    {
        return Err(VerticalError::PdaMismatch);
    }
    let funding_token = realm
        .release
        .profile()
        .check_custody_account(
            state.token_program.key.to_bytes(),
            &state.funding_collateral_vault.data,
            state.collateral_mint.key.to_bytes(),
            state.funding_authority.key.to_bytes(),
        )
        .map_err(|_| VerticalError::InvalidState)?;
    let state_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
    let token_rent = rent.minimum_balance(ACCOUNT_BYTES);
    require_exact_funding_principal(
        funding.remaining().native_lamports_total(),
        funding.remaining().realm_collateral_total(),
        state.funding_state.lamports,
        state_rent,
        funding_token.amount,
    )?;
    let vault_observation = RealmCollateralVaultObservationV1::new(
        state.funding_collateral_vault.key.to_bytes(),
        state.funding_authority.key.to_bytes(),
        state.token_program.key.to_bytes(),
        state.collateral_mint.key.to_bytes(),
        funding_token.amount,
        state.funding_collateral_vault.lamports,
        token_rent,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    let collateral_custody = RealmCollateralCustodyV1::new(
        binding.realm_id(),
        binding.collateral_release_id(),
        state.funding_authority.key.to_bytes(),
        state.funding_collateral_vault.key.to_bytes(),
        vault_observation,
    )
    .map_err(|_| VerticalError::ContentMismatch)?;
    let custody = FundingCustodyObservationV1::with_realm_collateral(
        state.funding_state.lamports,
        state_rent,
        collateral_custody,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    funding
        .validate_against(manifest_id, manifest, custody)
        .map_err(|_| VerticalError::ContentMismatch)?;

    require_vacant(&state.pool)?;
    require_vacant(&state.lp_position)?;
    require_vacant(&state.pool_position)?;
    require_vacant(&state.principal_vault)?;
    require_vacant(&state.fee_vault)?;
    require_vacant(&state.service_vault)?;
    let pool_seeds = PoolPdaSeedsV1::new(state.market.key.to_bytes(), market.generation, config_id)
        .map_err(|_| VerticalError::PdaMismatch)?;
    if state.pool.key != Pubkey::find_program_address(&pool_seeds.seed_components(), &program_id).0
    {
        return Err(VerticalError::PdaMismatch);
    }
    let lp_seeds = LpPositionPdaSeedsV1::new(
        state.market.key.to_bytes(),
        market.generation,
        config_id,
        choice.initial_lp_id,
    )
    .map_err(|_| VerticalError::PdaMismatch)?;
    if state.lp_position.key
        != Pubkey::find_program_address(&lp_seeds.seed_components(), &program_id).0
    {
        return Err(VerticalError::PdaMismatch);
    }
    let position_seeds =
        PoolPositionPdaSeedsV1::new(state.market.key.to_bytes(), state.pool.key.to_bytes())
            .map_err(|_| VerticalError::PdaMismatch)?;
    if state.pool_position.key
        != Pubkey::find_program_address(&position_seeds.seed_components(), &program_id).0
    {
        return Err(VerticalError::PdaMismatch);
    }
    for (account, compartment) in [
        (
            &state.principal_vault,
            DealerCollateralCompartmentV1::Principal,
        ),
        (
            &state.fee_vault,
            DealerCollateralCompartmentV1::RealizedFees,
        ),
        (&state.service_vault, DealerCollateralCompartmentV1::Service),
    ] {
        let seeds = DealerCollateralVaultPdaSeedsV1::new(state.pool.key.to_bytes(), compartment)
            .map_err(|_| VerticalError::PdaMismatch)?;
        if account.key != Pubkey::find_program_address(&seeds.seed_components(), &program_id).0 {
            return Err(VerticalError::PdaMismatch);
        }
    }
    if config
        .liquidity_owner()
        .map_err(|_| VerticalError::InvalidState)?
        != state.owner.key.to_bytes()
    {
        return Err(VerticalError::ContentMismatch);
    }
    let owner_position = authenticate_position::<N>(
        program_id,
        &state.owner_position,
        state.market.key,
        state.owner.key,
        market.generation,
    )?;
    for balance in owner_position.balances() {
        if *balance < choice.initial_claim_quantity {
            return Err(VerticalError::InvalidState);
        }
    }
    authenticate_rent_credit(program_id, &state.pool_position_rent_credit, state.pool.key)
        .map_err(|_| VerticalError::ContentMismatch)?;
    authenticate_rent_credit(program_id, &state.pool_rent_credit, state.owner.key)
        .map_err(|_| VerticalError::ContentMismatch)?;
    authenticate_rent_credit(program_id, &state.lp_rent_credit, state.owner.key)
        .map_err(|_| VerticalError::ContentMismatch)?;

    let pool_account_rent = rent.minimum_balance(
        profile
            .pool_len()
            .map_err(|_| VerticalError::InvalidState)?,
    );
    let lp_rent = rent.minimum_balance(LP_POSITION_BYTES);
    let position_rent = rent
        .minimum_balance(PositionV1::<N>::encoded_len().map_err(|_| VerticalError::InvalidState)?);
    let pool_bundle_rent = pool_account_rent
        .checked_add(
            token_rent
                .checked_mul(3)
                .ok_or(VerticalError::InvalidState)?,
        )
        .ok_or(VerticalError::InvalidState)?;
    let total_rent = pool_bundle_rent
        .checked_add(lp_rent)
        .and_then(|value| value.checked_add(position_rent))
        .ok_or(VerticalError::InvalidState)?;
    if state.activator.lamports < total_rent {
        return Err(VerticalError::InvalidState);
    }
    let release_id = ContentId::new(DEALER_CAPABILITY_RELEASE_ID_V1)
        .map_err(|_| VerticalError::ContentMismatch)?;
    let attachment = LiquidityAttachment::new(
        market.identity,
        release_id,
        config_id,
        state.owner.key.to_bytes(),
    )
    .map_err(|_| VerticalError::ContentMismatch)?;
    let request = ActivatePoolV1::new(
        market.generation,
        market.child_count,
        choice.initial_lp_id,
        choice.initial_claim_quantity,
        choice.initial_shares,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    let mut pool_out = vec![
        0;
        profile
            .pool_len()
            .map_err(|_| VerticalError::InvalidState)?
    ];
    let plan = activate_pool_into::<N>(
        market_root::<N>(program_id, &state.market)?,
        state.market.key.to_bytes(),
        manifest,
        funding,
        state.funding_state.key.to_bytes(),
        state.funding_authority.key.to_bytes(),
        custody,
        attachment,
        config,
        profile,
        &mut pool_out,
        state.pool.key.to_bytes(),
        state.lp_position.key.to_bytes(),
        state.owner.key.to_bytes(),
        RentCreditTerms::new(state.owner.key.to_bytes(), pool_bundle_rent)
            .map_err(|_| VerticalError::InvalidState)?,
        RentCreditTerms::new(state.owner.key.to_bytes(), lp_rent)
            .map_err(|_| VerticalError::InvalidState)?,
        RentCreditTerms::new(state.pool.key.to_bytes(), position_rent)
            .map_err(|_| VerticalError::InvalidState)?,
        request,
        observation.slot,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    let debit = plan.funding_debit();
    let principal_collateral = debit.liquidity().amount();
    let service_collateral = debit.service().map_or(0, |release| release.amount());
    if funding_token.amount
        < principal_collateral
            .checked_add(service_collateral)
            .ok_or(VerticalError::InvalidState)?
    {
        return Err(VerticalError::InvalidState);
    }
    let data = encode_instruction(DealerInstructionV1::<N>::ActivatePool(request))?;
    let instruction = account_frame::<N>(
        program_id,
        DealerActionV1::ActivatePool,
        &[
            &state.activator,
            &state.owner,
            &state.realm,
            &state.market,
            &state.capability_manifest,
            &state.funding_state,
            &state.funding_authority,
            &state.funding_collateral_vault,
            &state.config,
            &state.config_staging,
            &state.pool,
            &state.lp_position,
            &state.owner_position,
            &state.pool_position,
            &state.principal_vault,
            &state.fee_vault,
            &state.service_vault,
            &state.pool_position_rent_credit,
            &state.pool_rent_credit,
            &state.lp_rent_credit,
            &state.collateral_mint,
            &state.token_program,
            &state.system_program,
            &state.rent_sysvar,
        ],
        data,
    )?;
    Ok(DealerActivateReport {
        instruction,
        observation,
        required_signers: if state.activator.key == state.owner.key {
            vec![state.activator.key]
        } else {
            vec![state.activator.key, state.owner.key]
        },
        pool: state.pool.key,
        lp_position: state.lp_position.key,
        rent_debit_lamports: debit.activation().rent_lamports(),
        principal_collateral,
        service_collateral,
        claim_reserves: vec![choice.initial_claim_quantity; N],
        market_child_count_before: market.child_count,
        market_child_count_after: market
            .child_count
            .checked_add(1)
            .ok_or(VerticalError::InvalidState)?,
    })
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
    let config =
        LiquidityConfigViewV1::new(facts.config_id, facts.profile, &state.pool.config.data)
            .map_err(|_| VerticalError::InvalidState)?;
    let mut pool_bytes = state.pool.pool.data.clone();
    create_position(
        &mut pool_bytes,
        facts.profile,
        state.pool.pool.key.to_bytes(),
        config,
        facts.pool_sequence,
        state.lp_position.key.to_bytes(),
        state.owner.key.to_bytes(),
        dclutch_dealer_contract::RentCreditTerms::new(
            state.owner.key.to_bytes(),
            rent_debit_lamports,
        )
        .map_err(|_| VerticalError::InvalidState)?,
    )
    .map_err(|_| VerticalError::InvalidState)?;
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
    let owner_token = authenticate_transfer_account(
        &state.owner_collateral,
        &state.collateral_mint,
        &state.token_program,
        realm,
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
        require_token_authority(owner_token, state.owner.key, collateral)?;
        principal_token
            .amount
            .checked_add(principal)
            .ok_or(VerticalError::InvalidState)?;
        fee_token
            .amount
            .checked_add(fees)
            .ok_or(VerticalError::InvalidState)?;
        require_position_debits(&owner_position, &moved.claim_reserves())?;
        require_position_credits(&pool_position, &moved.claim_reserves())?;
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
        require_position_credits(&owner_position, &moved.claim_reserves())?;
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
    let trader_token = authenticate_transfer_account(
        &state.trader_collateral,
        &state.collateral_mint,
        &state.token_program,
        realm,
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
            let debit = receipt
                .notional_collateral()
                .checked_add(receipt.trader_fee_collateral())
                .ok_or(VerticalError::InvalidState)?;
            require_token_authority(trader_token, state.trader.key, debit)?;
            principal_token
                .amount
                .checked_add(receipt.notional_collateral())
                .ok_or(VerticalError::InvalidState)?;
            fee_token
                .amount
                .checked_add(receipt.trader_fee_collateral())
                .ok_or(VerticalError::InvalidState)?;
            require_position_debit(&pool_position, claim_index, choice.quantity)?;
            require_position_credit(&trader_position, claim_index, choice.quantity)?;
        }
        TradeSide::SellClaimToPool => {
            require_token_authority(
                trader_token,
                state.trader.key,
                receipt.trader_fee_collateral(),
            )?;
            if principal_token.amount < receipt.notional_collateral() {
                return Err(VerticalError::InvalidState);
            }
            fee_token
                .amount
                .checked_add(receipt.trader_fee_collateral())
                .ok_or(VerticalError::InvalidState)?;
            trader_token
                .amount
                .checked_add(receipt.notional_collateral())
                .ok_or(VerticalError::InvalidState)?;
            require_position_debit(&trader_position, claim_index, choice.quantity)?;
            require_position_credit(&pool_position, claim_index, choice.quantity)?;
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
    let refund = authenticate_destination_custody(
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

fn authenticate_config<'a>(
    program_id: Pubkey,
    config_account: &'a ObservedAccount,
    staging: &ObservedAccount,
    outcomes: usize,
) -> Result<(LiquidityProfileV1, ContentId, LiquidityConfigViewV1<'a>), VerticalError> {
    if config_account.owner != program_id || config_account.executable {
        return Err(VerticalError::InvalidOwner);
    }
    let digest = hash(&config_account.data).to_bytes();
    let config_id = ContentId::new(digest).map_err(|_| VerticalError::InvalidState)?;
    let profile = LiquidityProfileV1::from_config_len(outcomes, config_account.data.len())
        .map_err(|_| VerticalError::InvalidState)?;
    let config = LiquidityConfigViewV1::new(config_id, profile, &config_account.data)
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
            &digest,
        ],
        &program_id,
    )
    .0;
    if config_account.key != expected_config
        || staging.key != expected_staging
        || staging.owner != system_program::ID
        || staging.executable
        || !staging.data.is_empty()
    {
        return Err(VerticalError::FinalizationMismatch);
    }
    Ok((profile, config_id, config))
}

fn authenticate_pool(
    program_id: Pubkey,
    state: &DealerPoolState,
    action: DealerActionV1,
) -> Result<PoolFacts, VerticalError> {
    let market = market_facts(program_id, &state.market)?;
    validate_market_phase(action, market.phase).map_err(|_| VerticalError::InvalidPhase)?;
    let (profile, config_id, config) = authenticate_config(
        program_id,
        &state.config,
        &state.config_staging,
        market.outcomes,
    )?;
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

#[allow(clippy::too_many_arguments)]
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

fn authenticate_transfer_account(
    account: &ObservedAccount,
    mint: &ObservedAccount,
    token_program: &ObservedAccount,
    realm: RealmFacts,
) -> Result<TokenAccount, VerticalError> {
    if account.owner != token_program.key || account.executable {
        return Err(VerticalError::InvalidOwner);
    }
    let token = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &account.data)
        .map_err(|_| VerticalError::InvalidState)?;
    if token.mint != mint.key.to_bytes() {
        return Err(VerticalError::ContentMismatch);
    }
    Ok(token)
}

fn authenticate_destination_custody(
    account: &ObservedAccount,
    mint: &ObservedAccount,
    token_program: &ObservedAccount,
    realm: RealmFacts,
    authority: Pubkey,
) -> Result<TokenAccount, VerticalError> {
    if account.owner != token_program.key || account.executable {
        return Err(VerticalError::InvalidOwner);
    }
    realm
        .release
        .profile()
        .check_custody_account(
            token_program.key.to_bytes(),
            &account.data,
            mint.key.to_bytes(),
            authority.to_bytes(),
        )
        .map_err(|_| VerticalError::InvalidState)
}

fn require_token_authority(
    token: TokenAccount,
    authority: Pubkey,
    amount: u64,
) -> Result<(), VerticalError> {
    if token.amount < amount {
        return Err(VerticalError::InvalidState);
    }
    if token.owner == authority.to_bytes() {
        return Ok(());
    }
    match token.delegate {
        COption::Some(delegate)
            if delegate == authority.to_bytes() && token.delegated_amount >= amount =>
        {
            Ok(())
        }
        COption::None | COption::Some(_) => Err(VerticalError::InvalidAuthority),
    }
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

fn require_position_credits<const N: usize>(
    position: &PositionV1<N>,
    amounts: &[u64; N],
) -> Result<(), VerticalError> {
    for (balance, credit) in position.balances().iter().zip(amounts) {
        balance
            .checked_add(*credit)
            .ok_or(VerticalError::InvalidState)?;
    }
    Ok(())
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

fn require_position_credit<const N: usize>(
    position: &PositionV1<N>,
    outcome: usize,
    amount: u64,
) -> Result<(), VerticalError> {
    position
        .balances()
        .get(outcome)
        .copied()
        .ok_or(VerticalError::InvalidState)?
        .checked_add(amount)
        .ok_or(VerticalError::InvalidState)?;
    Ok(())
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

fn require_manifest_schema(schema: [u8; 32]) -> Result<(), VerticalError> {
    if schema == CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1 {
        Ok(())
    } else {
        Err(VerticalError::FinalizationMismatch)
    }
}

fn require_dealer_selection(
    kind: [u8; 32],
    release: [u8; 32],
    selected_config: ContentId,
    config: ContentId,
) -> Result<(), VerticalError> {
    if kind == DEALER_CAPABILITY_KIND_ID_V1
        && release == DEALER_CAPABILITY_RELEASE_ID_V1
        && selected_config == config
    {
        Ok(())
    } else {
        Err(VerticalError::ContentMismatch)
    }
}

fn require_pda(actual: Pubkey, expected: Pubkey) -> Result<(), VerticalError> {
    if actual == expected {
        Ok(())
    } else {
        Err(VerticalError::PdaMismatch)
    }
}

fn require_exact_funding_principal(
    native_principal: u64,
    collateral_principal: u64,
    state_lamports: u64,
    state_rent: u64,
    collateral_tokens: u64,
) -> Result<(), VerticalError> {
    if state_lamports.checked_sub(state_rent) != Some(native_principal)
        || collateral_tokens != collateral_principal
    {
        Err(VerticalError::ContentMismatch)
    } else {
        Ok(())
    }
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
    fn activation_refuses_wrong_selected_kind_release_and_config() {
        let config = id(40);
        assert_eq!(
            require_dealer_selection(
                DEALER_CAPABILITY_KIND_ID_V1,
                DEALER_CAPABILITY_RELEASE_ID_V1,
                config,
                config,
            ),
            Ok(())
        );
        assert_eq!(
            require_dealer_selection([41; 32], DEALER_CAPABILITY_RELEASE_ID_V1, config, config,),
            Err(VerticalError::ContentMismatch)
        );
        assert_eq!(
            require_dealer_selection(DEALER_CAPABILITY_KIND_ID_V1, [42; 32], config, config,),
            Err(VerticalError::ContentMismatch)
        );
        assert_eq!(
            require_dealer_selection(
                DEALER_CAPABILITY_KIND_ID_V1,
                DEALER_CAPABILITY_RELEASE_ID_V1,
                id(43),
                config,
            ),
            Err(VerticalError::ContentMismatch)
        );
    }

    #[test]
    fn activation_refuses_wrong_manifest_schema_and_pda() {
        assert_eq!(
            require_manifest_schema(CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1),
            Ok(())
        );
        assert_eq!(
            require_manifest_schema([44; 32]),
            Err(VerticalError::FinalizationMismatch)
        );
        let canonical = Pubkey::new_from_array([45; 32]);
        assert_eq!(require_pda(canonical, canonical), Ok(()));
        assert_eq!(
            require_pda(canonical, Pubkey::new_from_array([46; 32])),
            Err(VerticalError::PdaMismatch)
        );
    }

    #[test]
    fn activation_refuses_inexact_funding_dimensions() {
        assert_eq!(
            require_exact_funding_principal(600, 105_000, 1_600, 1_000, 105_000),
            Ok(())
        );
        assert_eq!(
            require_exact_funding_principal(600, 105_000, 1_599, 1_000, 105_000),
            Err(VerticalError::ContentMismatch)
        );
        assert_eq!(
            require_exact_funding_principal(600, 105_000, 1_600, 1_000, 104_999),
            Err(VerticalError::ContentMismatch)
        );
    }

    #[test]
    fn activation_refuses_occupied_fresh_physical_state() {
        let mut vacant = observed(
            Pubkey::new_from_array([47; 32]),
            system_program::ID,
            0,
            Vec::new(),
        );
        assert_eq!(require_vacant(&vacant), Ok(()));
        vacant.lamports = 1;
        assert_eq!(require_vacant(&vacant), Err(VerticalError::InvalidState));
    }
}
