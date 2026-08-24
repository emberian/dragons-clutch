//! Atomic pure plans joining Dealer lifecycle to Market and capability authority.
//!
//! The adapter must authenticate the manifest content hash committed by Market,
//! configuration content hash, all PDA bumps, physical capitalization, Realm
//! collateral custody, native Position claim balances, and actual rent minima
//! before persisting a returned plan. A plan is committed only with all named
//! transfers/account writes; it is not a caller attestation.

use dclutch_capability_contract::{
    ActivationDebitV1, CapabilityFundingDerivationV1, CapabilityManifestV1, FundingCompartment,
    FundingStateV1,
};
use dclutch_core_contract::{MarketRoot, Phase};

use crate::{
    Error as DealerError, LiquidityAmounts, LiquidityAttachment, LiquidityChangeReceipt,
    LiquidityConfigV1, LpPosition, PoolRetirementReceipt, PoolState, RentCreditTerms,
    frame::{ConfigPdaSeedsV1, FrameError, LpPositionPdaSeedsV1, PoolPdaSeedsV1},
    instruction::ActivatePoolV1,
};

/// Refusal from an atomic Dealer activation or terminal Market plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationError {
    /// The authenticated Market is in the wrong lifecycle phase.
    InvalidMarketPhase,
    /// Market/config/release/beneficiary authority did not join exactly.
    AuthorityMismatch,
    /// Activation rent quote did not equal the two new accounts' funded rent.
    RentFundingMismatch,
    /// Checked physical funding arithmetic overflowed or underflowed.
    FundingArithmetic,
    /// Market child replay/lifecycle authority refused the transition.
    Market(dclutch_core_contract::Error),
    /// Shared capability funding authority refused activation or release.
    Capability(dclutch_capability_contract::Error),
    /// Dealer state transition refused opening or retirement.
    Dealer(DealerError),
    /// Dealer PDA preimage construction refused a hostile identity.
    Frame(FrameError),
}

/// Result alias for Dealer lifecycle plans.
pub type Result<T> = core::result::Result<T, ActivationError>;

/// Exact FundingState debits derived during Pool activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFundingDebitV1 {
    activation: ActivationDebitV1,
    liquidity_principal: u64,
    service_principal: u64,
}

impl DealerFundingDebitV1 {
    /// Return shared rent/creation activation debit.
    pub const fn activation(self) -> ActivationDebitV1 {
        self.activation
    }
    /// Return exact present collateral moved into LP principal custody.
    pub const fn liquidity_principal(self) -> u64 {
        self.liquidity_principal
    }
    /// Return exact present collateral moved into segregated service custody.
    pub const fn service_principal(self) -> u64 {
        self.service_principal
    }
}

/// Successful atomic Activate/Open state and custody plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivatePoolPlanV1<const N: usize, const B: usize> {
    market: MarketRoot,
    funding: FundingStateV1,
    pool: PoolState<N, B>,
    initial_position: LpPosition,
    funding_debit: DealerFundingDebitV1,
    liquidity_receipt: LiquidityChangeReceipt<N>,
    capability_funding_seeds: CapabilityFundingDerivationV1,
    pool_seeds: PoolPdaSeedsV1,
    config_seeds: ConfigPdaSeedsV1,
    lp_seeds: LpPositionPdaSeedsV1,
}

impl<const N: usize, const B: usize> ActivatePoolPlanV1<N, B> {
    /// Return Market after registering the Pool direct child.
    pub const fn market(self) -> MarketRoot {
        self.market
    }
    /// Return FundingState after exact activation/liquidity/service releases.
    pub const fn funding(self) -> FundingStateV1 {
        self.funding
    }
    /// Return Pool state to persist at the canonical Pool PDA.
    pub const fn pool(self) -> PoolState<N, B> {
        self.pool
    }
    /// Return initial LP position to persist at the canonical compact-ID PDA.
    pub const fn initial_position(self) -> LpPosition {
        self.initial_position
    }
    /// Return exact physical funding debit plan.
    pub const fn funding_debit(self) -> DealerFundingDebitV1 {
        self.funding_debit
    }
    /// Return transient initial custody/share receipt.
    pub const fn liquidity_receipt(self) -> LiquidityChangeReceipt<N> {
        self.liquidity_receipt
    }
    /// Return reusable shared capability FundingState derivation authority.
    pub const fn capability_funding_seeds(self) -> CapabilityFundingDerivationV1 {
        self.capability_funding_seeds
    }
    /// Return Pool PDA seed preimage.
    pub const fn pool_seeds(self) -> PoolPdaSeedsV1 {
        self.pool_seeds
    }
    /// Return config PDA seed preimage.
    pub const fn config_seeds(self) -> ConfigPdaSeedsV1 {
        self.config_seeds
    }
    /// Return initial LP-position PDA seed preimage.
    pub const fn lp_seeds(self) -> LpPositionPdaSeedsV1 {
        self.lp_seeds
    }
}

/// Successful quiescent Pool/config retirement joined to Market child replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirePoolPlanV1<const N: usize, const B: usize> {
    market: MarketRoot,
    pool: PoolState<N, B>,
    receipt: PoolRetirementReceipt,
}

impl<const N: usize, const B: usize> RetirePoolPlanV1<N, B> {
    /// Return Market after decrementing its direct-child count.
    pub const fn market(self) -> MarketRoot {
        self.market
    }
    /// Return terminal Pool state before physical close.
    pub const fn pool(self) -> PoolState<N, B> {
        self.pool
    }
    /// Return exact service refund and Pool/config RentCredit destinations.
    pub const fn receipt(self) -> PoolRetirementReceipt {
        self.receipt
    }
}

/// Activate selected funding and open a fully covered Pool atomically.
///
/// `observed_present_principal` is derived from the physical FundingState
/// holding, and `authenticated_now_slot` is read from the adapter's trusted
/// Clock syscall/sysvar access; neither is instruction data. `pool_rent` and
/// `initial_position_rent` are constructed from observed Rent minima and the
/// immutable config RentCredit beneficiary.
#[allow(clippy::too_many_arguments)]
pub fn activate_pool<const N: usize, const B: usize>(
    market: MarketRoot,
    market_address: [u8; 32],
    manifest: CapabilityManifestV1<'_>,
    funding: FundingStateV1,
    observed_present_principal: u64,
    attachment: LiquidityAttachment,
    config: &LiquidityConfigV1<N, B>,
    pool_address: [u8; 32],
    initial_position_address: [u8; 32],
    initial_owner: [u8; 32],
    pool_rent: RentCreditTerms,
    initial_position_rent: RentCreditTerms,
    request: ActivatePoolV1,
    authenticated_now_slot: u64,
) -> Result<ActivatePoolPlanV1<N, B>> {
    if !matches!(market.phase(), Phase::Founding | Phase::Open) {
        return Err(ActivationError::InvalidMarketPhase);
    }
    let identity = market.identity();
    if identity != attachment.market()
        || identity.generation() != request.generation()
        || identity.capability_manifest_id() != funding.manifest_content_id()
    {
        return Err(ActivationError::AuthorityMismatch);
    }
    let manifest_id = identity.capability_manifest_id();
    let selected = manifest
        .entry(funding.entry_index())
        .map_err(ActivationError::Capability)?;
    if selected.config_id() != config.content_id()
        || selected.config_id() != attachment.liquidity_config_id()
        || selected.release_id() != attachment.capability_release_id()
    {
        return Err(ActivationError::AuthorityMismatch);
    }

    // V1 uses the content-bound config RentCredit beneficiary as the immutable
    // bootstrap LP and service-refund authority. This avoids any caller-chosen
    // owner for prepaid liquidity without adding a parallel authority record.
    let immutable_owner = config.rent_credit().beneficiary();
    if initial_owner != immutable_owner
        || attachment.service_refund_beneficiary() != immutable_owner
        || pool_rent.beneficiary() != immutable_owner
        || initial_position_rent.beneficiary() != immutable_owner
    {
        return Err(ActivationError::AuthorityMismatch);
    }

    let capability_funding_seeds = CapabilityFundingDerivationV1::new(
        market_address,
        request.generation(),
        manifest_id,
        manifest,
        funding,
    )
    .map_err(ActivationError::Capability)?;
    let pool_seeds = PoolPdaSeedsV1::new(market_address, request.generation(), config.content_id())
        .map_err(ActivationError::Frame)?;
    let config_seeds =
        ConfigPdaSeedsV1::new(market_address, request.generation(), config.content_id())
            .map_err(ActivationError::Frame)?;
    let lp_seeds = LpPositionPdaSeedsV1::new(
        market_address,
        request.generation(),
        config.content_id(),
        request.initial_lp_id(),
    )
    .map_err(ActivationError::Frame)?;

    let mut next_funding = funding;
    let activation = next_funding
        .activate(
            manifest_id,
            manifest,
            observed_present_principal,
            authenticated_now_slot,
        )
        .map_err(ActivationError::Capability)?;
    let expected_rent = pool_rent
        .funded_rent_principal()
        .checked_add(initial_position_rent.funded_rent_principal())
        .ok_or(ActivationError::FundingArithmetic)?;
    if activation.rent_principal() != expected_rent {
        return Err(ActivationError::RentFundingMismatch);
    }
    let after_activation = observed_present_principal
        .checked_sub(activation.rent_principal())
        .and_then(|value| value.checked_sub(activation.creation_principal()))
        .ok_or(ActivationError::FundingArithmetic)?;
    let liquidity_principal = next_funding.remaining().liquidity_principal();
    if liquidity_principal == 0 {
        return Err(ActivationError::Dealer(
            DealerError::IncompleteInitialLiquidity,
        ));
    }
    next_funding
        .release(
            manifest_id,
            manifest,
            after_activation,
            FundingCompartment::Liquidity,
            liquidity_principal,
        )
        .map_err(ActivationError::Capability)?;
    let after_liquidity = after_activation
        .checked_sub(liquidity_principal)
        .ok_or(ActivationError::FundingArithmetic)?;
    let service_principal = next_funding.remaining().service_principal();
    if service_principal > 0 {
        next_funding
            .release(
                manifest_id,
                manifest,
                after_liquidity,
                FundingCompartment::Service,
                service_principal,
            )
            .map_err(ActivationError::Capability)?;
    }

    let initial_liquidity = LiquidityAmounts::new(
        liquidity_principal,
        0,
        [request.initial_claim_quantity(); N],
    )
    .map_err(ActivationError::Dealer)?;
    let (pool, initial_position, liquidity_receipt) = PoolState::open(
        attachment,
        pool_address,
        config,
        pool_rent,
        authenticated_now_slot,
        initial_liquidity,
        service_principal,
        initial_position_address,
        initial_owner,
        initial_position_rent,
        request.initial_shares(),
    )
    .map_err(ActivationError::Dealer)?;
    let mut next_market = market;
    next_market
        .register_child(request.generation(), request.expected_market_child_count())
        .map_err(ActivationError::Market)?;
    Ok(ActivatePoolPlanV1 {
        market: next_market,
        funding: next_funding,
        pool,
        initial_position,
        funding_debit: DealerFundingDebitV1 {
            activation,
            liquidity_principal,
            service_principal,
        },
        liquidity_receipt,
        capability_funding_seeds,
        pool_seeds,
        config_seeds,
        lp_seeds,
    })
}

/// Retire a quiescent Pool/config and decrement Market direct-child replay.
pub fn retire_pool<const N: usize, const B: usize>(
    market: MarketRoot,
    pool: PoolState<N, B>,
    pool_address: [u8; 32],
    config: &LiquidityConfigV1<N, B>,
    expected_pool_sequence: u64,
    expected_market_child_count: u64,
) -> Result<RetirePoolPlanV1<N, B>> {
    if market.phase() != Phase::Retiring || market.identity() != pool.attachment().market() {
        return Err(ActivationError::InvalidMarketPhase);
    }
    let mut next_pool = pool;
    let receipt = next_pool
        .retire(pool_address, config, expected_pool_sequence)
        .map_err(ActivationError::Dealer)?;
    let mut next_market = market;
    next_market
        .retire_child(
            next_market.identity().generation(),
            expected_market_child_count,
        )
        .map_err(ActivationError::Market)?;
    Ok(RetirePoolPlanV1 {
        market: next_market,
        pool: next_pool,
        receipt,
    })
}
