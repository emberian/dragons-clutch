//! Atomic pure plans joining Dealer lifecycle to Market and capability authority.
//!
//! The adapter must authenticate the manifest content hash committed by Market,
//! configuration content hash, all PDA bumps, physical capitalization, Realm
//! collateral custody, native Position claim balances, and actual rent minima
//! before persisting a returned plan. A plan is committed only with all named
//! transfers/account writes; it is not a caller attestation.

use dclutch_capability_contract::{
    ActivationDebitV1, CapabilityFundingAuthorityDerivationV1, CapabilityFundingDerivationV1,
    CapabilityFundingVaultDerivationV1, CapabilityManifestV1, FundingAssetClassV1,
    FundingCompartment, FundingCustodyObservationV1, FundingReleasePlanV1, FundingStateV1,
    RealmCollateralCustodyV1, RealmCollateralVaultObservationV1,
};
use dclutch_core_contract::{MarketRoot, Phase};

use crate::{
    Error as DealerError, LiquidityAmounts, LiquidityAttachment, LiquidityChangeReceipt,
    LiquidityConfigV1, LpPosition, PoolRetirementReceipt, PoolState, RentCreditTerms,
    frame::{
        ConfigPdaSeedsV1, FrameError, LpPositionPdaSeedsV1, PoolPdaSeedsV1, PoolPositionPdaSeedsV1,
    },
    instruction::ActivatePoolV1,
};

/// Refusal from an atomic Dealer activation or terminal Market plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationError {
    /// The authenticated Market is in the wrong lifecycle phase.
    InvalidMarketPhase,
    /// Market/config/release/beneficiary authority did not join exactly.
    AuthorityMismatch,
    /// Activation rent quote did not equal all new accounts' funded rent.
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
    liquidity: FundingReleasePlanV1,
    service: Option<FundingReleasePlanV1>,
}

impl DealerFundingDebitV1 {
    /// Return shared rent/creation activation debit.
    pub const fn activation(self) -> ActivationDebitV1 {
        self.activation
    }
    /// Return exact present collateral moved into LP principal custody.
    pub const fn liquidity(self) -> FundingReleasePlanV1 {
        self.liquidity
    }
    /// Return optional exact typed release into segregated service custody.
    pub const fn service(self) -> Option<FundingReleasePlanV1> {
        self.service
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
    capability_funding_authority_seeds: CapabilityFundingAuthorityDerivationV1,
    capability_funding_vault_seeds: CapabilityFundingVaultDerivationV1,
    pool_seeds: PoolPdaSeedsV1,
    config_seeds: ConfigPdaSeedsV1,
    lp_seeds: LpPositionPdaSeedsV1,
    pool_position_seeds: PoolPositionPdaSeedsV1,
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
    /// Return the shared capability funding token-authority PDA preimage.
    pub const fn capability_funding_authority_seeds(
        self,
    ) -> CapabilityFundingAuthorityDerivationV1 {
        self.capability_funding_authority_seeds
    }
    /// Return the shared capability Realm-collateral Vault PDA preimage.
    pub const fn capability_funding_vault_seeds(self) -> CapabilityFundingVaultDerivationV1 {
        self.capability_funding_vault_seeds
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
    /// Return shared native Position PDA seeds for Pool-owned claim inventory.
    pub const fn pool_position_seeds(self) -> PoolPositionPdaSeedsV1 {
        self.pool_position_seeds
    }
}

/// Successful quiescent Pool retirement joined to Market child replay.
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
    /// Return exact service refund and Pool RentCredit destination.
    pub const fn receipt(self) -> PoolRetirementReceipt {
        self.receipt
    }
}

/// Activate selected funding and open a fully covered Pool atomically.
///
/// `funding_custody` separately authenticates the FundingState lamports and its
/// optional Realm-collateral token vault; no cross-asset sum exists.
/// `authenticated_now_slot` is read from the adapter's trusted Clock
/// syscall/sysvar access; neither is instruction data. `pool_rent` and
/// `initial_position_rent` are constructed from observed Rent minima and the
/// immutable config liquidity owner. `pool_position_rent` is the Rent
/// minimum of the shared native Position owned by the Pool PDA.
#[allow(clippy::too_many_arguments)]
pub fn activate_pool<const N: usize, const B: usize>(
    market: MarketRoot,
    market_address: [u8; 32],
    manifest: CapabilityManifestV1<'_>,
    funding: FundingStateV1,
    funding_state_address: [u8; 32],
    funding_authority_address: [u8; 32],
    funding_custody: FundingCustodyObservationV1,
    attachment: LiquidityAttachment,
    config: &LiquidityConfigV1<N, B>,
    pool_address: [u8; 32],
    initial_position_address: [u8; 32],
    initial_owner: [u8; 32],
    pool_rent: RentCreditTerms,
    initial_position_rent: RentCreditTerms,
    pool_position_rent: RentCreditTerms,
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
    let funding_amounts = selected.funding_quote().amounts();
    if funding_amounts.creation().amount() != 0
        || funding_amounts.work().amount() != 0
        || funding_amounts.provider().amount() != 0
        || funding_amounts.bounty().amount() != 0
        || funding_amounts.liquidity().asset_class() != FundingAssetClassV1::RealmCollateral
        || !matches!(
            funding_amounts.service().asset_class(),
            FundingAssetClassV1::NotApplicable | FundingAssetClassV1::RealmCollateral
        )
    {
        return Err(ActivationError::AuthorityMismatch);
    }
    let collateral_binding = selected
        .funding_quote()
        .realm_collateral()
        .ok_or(ActivationError::AuthorityMismatch)?;
    let observed_collateral = funding_custody
        .realm_collateral()
        .ok_or(ActivationError::AuthorityMismatch)?;
    if observed_collateral.canonical_funding_authority() != funding_authority_address {
        return Err(ActivationError::AuthorityMismatch);
    }

    // V1 uses the content-bound config liquidity owner as the immutable
    // bootstrap LP and service-refund authority. This avoids any caller-chosen
    // owner for prepaid liquidity without adding a parallel authority record.
    let immutable_owner = config.liquidity_owner();
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
    let capability_funding_authority_seeds =
        CapabilityFundingAuthorityDerivationV1::new(funding_state_address)
            .map_err(ActivationError::Capability)?;
    let capability_funding_vault_seeds =
        CapabilityFundingVaultDerivationV1::new(funding_authority_address, collateral_binding)
            .map_err(ActivationError::Capability)?;
    let pool_seeds = PoolPdaSeedsV1::new(market_address, request.generation(), config.content_id())
        .map_err(ActivationError::Frame)?;
    let config_seeds = ConfigPdaSeedsV1::new(config.content_id());
    let lp_seeds = LpPositionPdaSeedsV1::new(
        market_address,
        request.generation(),
        config.content_id(),
        request.initial_lp_id(),
    )
    .map_err(ActivationError::Frame)?;
    let pool_position_seeds = PoolPositionPdaSeedsV1::new(market_address, pool_address)
        .map_err(ActivationError::Frame)?;

    let mut next_funding = funding;
    let activation = next_funding
        .activate(
            manifest_id,
            manifest,
            funding_custody,
            authenticated_now_slot,
        )
        .map_err(ActivationError::Capability)?;
    if pool_position_rent.beneficiary() != pool_address {
        return Err(ActivationError::AuthorityMismatch);
    }
    let expected_rent = pool_rent
        .funded_rent_principal()
        .checked_add(initial_position_rent.funded_rent_principal())
        .and_then(|value| value.checked_add(pool_position_rent.funded_rent_principal()))
        .ok_or(ActivationError::FundingArithmetic)?;
    if activation.rent_lamports() != expected_rent || activation.creation_lamports() != 0 {
        return Err(ActivationError::RentFundingMismatch);
    }
    let after_activation = custody_after(
        funding_custody,
        activation
            .rent_lamports()
            .checked_add(activation.creation_lamports())
            .ok_or(ActivationError::FundingArithmetic)?,
        0,
    )?;
    let liquidity_principal = next_funding.remaining().liquidity().amount();
    if liquidity_principal == 0 {
        return Err(ActivationError::Dealer(
            DealerError::IncompleteInitialLiquidity,
        ));
    }
    let liquidity = next_funding
        .release(
            manifest_id,
            manifest,
            after_activation,
            FundingCompartment::Liquidity,
            liquidity_principal,
        )
        .map_err(ActivationError::Capability)?;
    if liquidity.asset_class() != FundingAssetClassV1::RealmCollateral
        || liquidity.amount() != liquidity_principal
    {
        return Err(ActivationError::AuthorityMismatch);
    }
    let after_liquidity = custody_after(after_activation, 0, liquidity_principal)?;
    let service_principal = next_funding.remaining().service().amount();
    let service = if service_principal > 0 {
        let release = next_funding
            .release(
                manifest_id,
                manifest,
                after_liquidity,
                FundingCompartment::Service,
                service_principal,
            )
            .map_err(ActivationError::Capability)?;
        if release.asset_class() != FundingAssetClassV1::RealmCollateral
            || release.amount() != service_principal
        {
            return Err(ActivationError::AuthorityMismatch);
        }
        Some(release)
    } else {
        None
    };

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
            liquidity,
            service,
        },
        liquidity_receipt,
        capability_funding_seeds,
        capability_funding_authority_seeds,
        capability_funding_vault_seeds,
        pool_seeds,
        config_seeds,
        lp_seeds,
        pool_position_seeds,
    })
}

fn custody_after(
    before: FundingCustodyObservationV1,
    native_lamports_debit: u64,
    realm_collateral_debit: u64,
) -> Result<FundingCustodyObservationV1> {
    let state_account_lamports = before
        .state_account_lamports()
        .checked_sub(native_lamports_debit)
        .ok_or(ActivationError::FundingArithmetic)?;
    let state_rent = before.exact_state_rent_lamports();
    match before.realm_collateral() {
        None if realm_collateral_debit == 0 => {
            FundingCustodyObservationV1::native_only(state_account_lamports, state_rent)
                .map_err(ActivationError::Capability)
        }
        Some(custody) => {
            let observed = custody.observation();
            let token_amount = observed
                .token_amount()
                .checked_sub(realm_collateral_debit)
                .ok_or(ActivationError::FundingArithmetic)?;
            let observation = RealmCollateralVaultObservationV1::new(
                observed.vault(),
                observed.authority(),
                observed.token_program(),
                observed.mint(),
                token_amount,
                observed.account_lamports(),
                observed.exact_rent_lamports(),
            )
            .map_err(ActivationError::Capability)?;
            let custody = RealmCollateralCustodyV1::new(
                custody.realm_id(),
                custody.collateral_release_id(),
                custody.canonical_funding_authority(),
                custody.canonical_vault(),
                observation,
            )
            .map_err(ActivationError::Capability)?;
            FundingCustodyObservationV1::with_realm_collateral(
                state_account_lamports,
                state_rent,
                custody,
            )
            .map_err(ActivationError::Capability)
        }
        None => Err(ActivationError::AuthorityMismatch),
    }
}

/// Retire a quiescent Pool and decrement Market direct-child replay.
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
