//! Total cross-representation transition plans.

use dclutch_capability_contract::{
    CapabilityEntryV1, CapabilityManifestV1, ContentId, FundingAssetClassV1,
    FundingCustodyObservationV1, FundingStateV1,
};
use dclutch_core_contract::Phase;
use dclutch_market_contract::market::CategoricalMarketV1;
use dclutch_realm_contract::{PositionV1, RealmV1};

use crate::state::{
    BEARER_CAPABILITY_KIND_ID, BEARER_CHILD_DERIVATION_ID, BEARER_CHILD_SCHEMA_ID,
    BEARER_SEMANTIC_RELEASE_ID, BearerCapabilityDerivationV1, BearerCapabilityV1, BearerConfigV1,
    BearerMintDerivationV1, MintObservationV1, TokenAccountObservationV1, validate_width,
};
use crate::{Error, Result, require_nonzero, require_quantity};

/// Direction of the exact collateral movement paired with a Market mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollateralDirectionV1 {
    /// Transfer collateral atoms into the Market Hoard vault.
    DepositToHoard,
    /// Transfer collateral atoms out of the Market Hoard vault.
    WithdrawFromHoard,
}

/// Exact raw-collateral movement plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollateralPlanV1 {
    token_program: [u8; 32],
    mint: [u8; 32],
    amount: u64,
    direction: CollateralDirectionV1,
}

impl CollateralPlanV1 {
    /// Return the Realm-selected collateral token program.
    pub const fn token_program(self) -> [u8; 32] {
        self.token_program
    }

    /// Return the Realm-selected collateral Mint.
    pub const fn mint(self) -> [u8; 32] {
        self.mint
    }

    /// Return exact raw collateral atoms.
    pub const fn amount(self) -> u64 {
        self.amount
    }

    /// Return movement direction relative to the Hoard vault.
    pub const fn direction(self) -> CollateralDirectionV1 {
        self.direction
    }
}

/// Authenticated immutable Realm projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmBindingV1 {
    /// Content identity recomputed from the exact Realm preimage.
    pub content_id: ContentId,
    /// Hostile-decoded immutable Realm.
    pub realm: RealmV1,
}

/// One exact Mint initialization required by activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintInitializationV1 {
    /// Canonical Mint account address authenticated by the adapter.
    pub mint: [u8; 32],
    /// Exact canonical derivation seed projection.
    pub derivation: BearerMintDerivationV1,
    /// Capability PDA which must own mint, freeze, and close authority.
    pub controller: [u8; 32],
}

/// Atomic physical activation plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationPlanV1<const N: usize> {
    /// Exact capability-root seed projection.
    pub capability_derivation: BearerCapabilityDerivationV1,
    /// Capability PDA authenticated by the adapter.
    pub controller: [u8; 32],
    /// Exact initializations for all outcomes in canonical order.
    pub mints: [MintInitializationV1; N],
    /// Exact Rent lamports released from capability funding.
    pub rent_lamports: u64,
    /// Exact creation lamports released from capability funding.
    pub creation_lamports: u64,
    /// Immutable recipient of recovered capability Rent principal.
    pub rent_refund: [u8; 32],
}

/// Claim-side token supply operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenSupplyOperationV1 {
    /// Mint claim atoms after debiting native representation or splitting.
    Mint,
    /// Burn claim atoms before crediting native representation, merging, or redeeming.
    Burn,
}

/// Exact Token-2022 supply and holder-balance delta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenSupplyPlanV1 {
    /// Supply operation.
    pub operation: TokenSupplyOperationV1,
    /// Canonical outcome Mint.
    pub mint: [u8; 32],
    /// Holder token Account.
    pub token_account: [u8; 32],
    /// Exact claim atoms.
    pub amount: u64,
    /// Authenticated Mint supply before CPI.
    pub mint_supply_before: u64,
    /// Required Mint supply after CPI.
    pub mint_supply_after: u64,
    /// Authenticated holder balance before CPI.
    pub account_balance_before: u64,
    /// Required holder balance after CPI.
    pub account_balance_after: u64,
}

/// Exact ordinary Token-2022 transfer plan between two claim Accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerTransferPlanV1 {
    /// Canonical outcome Mint.
    pub mint: [u8; 32],
    /// Initialized source Account to debit.
    pub source: [u8; 32],
    /// Initialized destination Account to credit.
    pub destination: [u8; 32],
    /// Holder authority which must sign.
    pub authority: [u8; 32],
    /// Exact transferred claim atoms.
    pub amount: u64,
    /// Expected source balance after transfer.
    pub source_balance_after: u64,
    /// Expected destination balance after transfer.
    pub destination_balance_after: u64,
    /// Mint supply before and after transfer; it must not change.
    pub unchanged_mint_supply: u64,
}

/// Exact result of one claim redemption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RedemptionPlanV1 {
    /// Exact categorical payout in raw collateral atoms.
    pub payout: CollateralPlanV1,
    /// Bearer burn, or `None` when a native Position was debited.
    pub bearer_burn: Option<TokenSupplyPlanV1>,
}

/// Exact atomic close plan for the bearer direct child and all its Mints.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementPlanV1<const N: usize> {
    /// Canonical zero-supply Mint accounts to close in outcome order.
    pub mints: [[u8; 32]; N],
    /// Recipient of every recovered Mint and capability-root rent atom.
    pub rent_refund: [u8; 32],
    /// Market child count before the one exact decrement.
    pub market_child_count_before: u64,
    /// Market child count after the one exact decrement.
    pub market_child_count_after: u64,
}

/// Activate one manifest-selected, exactly funded optional bearer capability.
///
/// The caller-supplied Mint and controller addresses are not trusted
/// derivations. The adapter must derive them from the returned seed projections
/// before physical creation. Mutation is atomic across Market and funding
/// candidates in this pure plan.
#[allow(clippy::too_many_arguments)]
pub fn activate<const N: usize>(
    market_key: [u8; 32],
    market: &mut CategoricalMarketV1<N>,
    manifest_content_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    config_content_id: ContentId,
    config: BearerConfigV1,
    funding: &mut FundingStateV1,
    funding_custody: FundingCustodyObservationV1,
    current_slot: u64,
    exact_rent_lamports: u64,
    exact_creation_lamports: u64,
    expected_prior_child_count: u64,
    controller: [u8; 32],
    mint_keys: [[u8; 32]; N],
) -> Result<(BearerCapabilityV1<N>, ActivationPlanV1<N>)> {
    validate_width::<N>()?;
    require_nonzero(&market_key)?;
    require_nonzero(&controller)?;
    require_unique_keys(&mint_keys)?;
    if !matches!(market.root().phase(), Phase::Founding | Phase::Open) {
        return Err(Error::InvalidMarketPhase);
    }
    if market.root().identity().capability_manifest_id() != manifest_content_id {
        return Err(Error::ManifestMismatch);
    }
    validate_entry(
        manifest
            .entry(funding.entry_index())
            .map_err(capability_error)?,
        config_content_id,
    )?;

    let generation = market.root().identity().generation();
    let mut next_market = *market;
    let mut next_funding = *funding;
    let debit = next_funding
        .activate(manifest_content_id, manifest, funding_custody, current_slot)
        .map_err(capability_error)?;
    if debit.rent_lamports() != exact_rent_lamports
        || debit.creation_lamports() != exact_creation_lamports
    {
        return Err(Error::ActivationFundingMismatch);
    }
    next_market
        .register_child(generation, expected_prior_child_count)
        .map_err(market_error)?;
    let state = BearerCapabilityV1::activated(market_key, generation, funding.entry_index())?;
    let capability_derivation = BearerCapabilityDerivationV1::new(market_key, generation)?;
    let mut mints = [MintInitializationV1 {
        mint: [0; 32],
        derivation: BearerMintDerivationV1::new::<N>(market_key, generation, 0)?,
        controller,
    }; N];
    let mut index = 0usize;
    while index < N {
        let mint = mint_keys.get(index).copied().ok_or(Error::InvalidOutcome)?;
        let destination = mints.get_mut(index).ok_or(Error::InvalidOutcome)?;
        *destination = MintInitializationV1 {
            mint,
            derivation: BearerMintDerivationV1::new::<N>(market_key, generation, index)?,
            controller,
        };
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    *market = next_market;
    *funding = next_funding;
    Ok((
        state,
        ActivationPlanV1 {
            capability_derivation,
            controller,
            mints,
            rent_lamports: exact_rent_lamports,
            creation_lamports: exact_creation_lamports,
            rent_refund: config.rent_refund(),
        },
    ))
}

/// Validate every canonical Mint against persistent accounted bearer supply.
pub fn audit_mints<const N: usize>(
    state: &BearerCapabilityV1<N>,
    market_key: [u8; 32],
    market: &CategoricalMarketV1<N>,
    controller: [u8; 32],
    expected_mint_keys: [[u8; 32]; N],
    observations: [MintObservationV1; N],
) -> Result<()> {
    state.validate_market(market_key, market)?;
    require_unique_keys(&expected_mint_keys)?;
    let mut index = 0usize;
    while index < N {
        let expected = expected_mint_keys
            .get(index)
            .copied()
            .ok_or(Error::InvalidOutcome)?;
        let observation = observations
            .get(index)
            .copied()
            .ok_or(Error::InvalidOutcome)?;
        observation.validate_profile(expected, controller)?;
        if observation.supply
            != state
                .accounted_supply()
                .get(index)
                .copied()
                .ok_or(Error::InvalidOutcome)?
        {
            return Err(Error::UnaccountedMintSupply);
        }
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

/// Deposit one complete set's backing and credit a native Position.
pub fn split_to_position<const N: usize>(
    market_key: [u8; 32],
    market: &mut CategoricalMarketV1<N>,
    position: &mut PositionV1<N>,
    owner: [u8; 32],
    realm: RealmBindingV1,
    quantity: u64,
) -> Result<CollateralPlanV1> {
    require_quantity(quantity)?;
    validate_realm(market, realm)?;
    validate_position(market_key, market, position, owner)?;
    let mut next_market = *market;
    let mut next_position = *position;
    next_market
        .split_complete_set(quantity)
        .map_err(market_error)?;
    next_position
        .credit_complete_set(quantity)
        .map_err(realm_error)?;
    *market = next_market;
    *position = next_position;
    Ok(collateral_plan(
        realm.realm,
        quantity,
        CollateralDirectionV1::DepositToHoard,
    ))
}

/// Debit a native complete set and release exact Hoard backing.
pub fn merge_from_position<const N: usize>(
    market_key: [u8; 32],
    market: &mut CategoricalMarketV1<N>,
    position: &mut PositionV1<N>,
    owner: [u8; 32],
    realm: RealmBindingV1,
    quantity: u64,
) -> Result<CollateralPlanV1> {
    require_quantity(quantity)?;
    validate_realm(market, realm)?;
    validate_position(market_key, market, position, owner)?;
    let mut next_market = *market;
    let mut next_position = *position;
    next_position
        .debit_complete_set(quantity)
        .map_err(realm_error)?;
    next_market
        .merge_complete_set(quantity)
        .map_err(market_error)?;
    *market = next_market;
    *position = next_position;
    Ok(collateral_plan(
        realm.realm,
        quantity,
        CollateralDirectionV1::WithdrawFromHoard,
    ))
}

/// Move native Position claims into one canonical bearer Mint.
#[allow(clippy::too_many_arguments)]
pub fn materialize<const N: usize>(
    market_key: [u8; 32],
    market: &CategoricalMarketV1<N>,
    state: &mut BearerCapabilityV1<N>,
    position: &mut PositionV1<N>,
    owner: [u8; 32],
    outcome: usize,
    quantity: u64,
    controller: [u8; 32],
    expected_mint_key: [u8; 32],
    mint: MintObservationV1,
    destination: TokenAccountObservationV1,
) -> Result<TokenSupplyPlanV1> {
    require_quantity(quantity)?;
    if market.root().phase() != Phase::Open {
        return Err(Error::InvalidMarketPhase);
    }
    state.validate_market(market_key, market)?;
    validate_position(market_key, market, position, owner)?;
    validate_one_mint(state, outcome, controller, expected_mint_key, mint)?;
    destination.validate_holder(mint.key, owner, 0)?;
    let mut next_state = *state;
    let mut next_position = *position;
    next_position
        .debit_outcome(outcome, quantity)
        .map_err(realm_error)?;
    next_state.credit(outcome, quantity)?;
    next_state.validate_market(market_key, market)?;
    let mint_supply_after = mint
        .supply
        .checked_add(quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    let account_balance_after = destination
        .amount
        .checked_add(quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    *state = next_state;
    *position = next_position;
    Ok(TokenSupplyPlanV1 {
        operation: TokenSupplyOperationV1::Mint,
        mint: mint.key,
        token_account: destination.key,
        amount: quantity,
        mint_supply_before: mint.supply,
        mint_supply_after,
        account_balance_before: destination.amount,
        account_balance_after,
    })
}

/// Burn bearer claims and credit the holder's native Position.
#[allow(clippy::too_many_arguments)]
pub fn dematerialize<const N: usize>(
    market_key: [u8; 32],
    market: &CategoricalMarketV1<N>,
    state: &mut BearerCapabilityV1<N>,
    position: &mut PositionV1<N>,
    owner: [u8; 32],
    outcome: usize,
    quantity: u64,
    controller: [u8; 32],
    expected_mint_key: [u8; 32],
    mint: MintObservationV1,
    source: TokenAccountObservationV1,
) -> Result<TokenSupplyPlanV1> {
    require_quantity(quantity)?;
    if !matches!(
        market.root().phase(),
        Phase::Open | Phase::Resolved | Phase::Retiring
    ) {
        return Err(Error::InvalidMarketPhase);
    }
    state.validate_market(market_key, market)?;
    validate_position(market_key, market, position, owner)?;
    validate_one_mint(state, outcome, controller, expected_mint_key, mint)?;
    source.validate_holder(mint.key, owner, quantity)?;
    let mut next_state = *state;
    let mut next_position = *position;
    next_state.debit(outcome, quantity)?;
    next_position
        .credit_outcome(outcome, quantity)
        .map_err(realm_error)?;
    let plan = burn_plan(mint, source, quantity)?;
    *state = next_state;
    *position = next_position;
    Ok(plan)
}

/// Transfer bearer ownership without changing Market or Mint supply.
#[allow(clippy::too_many_arguments)]
pub fn transfer<const N: usize>(
    market_key: [u8; 32],
    market: &CategoricalMarketV1<N>,
    state: &BearerCapabilityV1<N>,
    outcome: usize,
    quantity: u64,
    controller: [u8; 32],
    authority: [u8; 32],
    expected_mint_key: [u8; 32],
    mint: MintObservationV1,
    source: TokenAccountObservationV1,
    destination: TokenAccountObservationV1,
) -> Result<BearerTransferPlanV1> {
    require_quantity(quantity)?;
    if !matches!(
        market.root().phase(),
        Phase::Open | Phase::Resolved | Phase::Retiring
    ) {
        return Err(Error::InvalidMarketPhase);
    }
    state.validate_market(market_key, market)?;
    validate_one_mint(state, outcome, controller, expected_mint_key, mint)?;
    source.validate_holder(mint.key, authority, quantity)?;
    destination.validate_holder(mint.key, destination.authority, 0)?;
    if source.key == destination.key {
        return Err(Error::AccountAlias);
    }
    Ok(BearerTransferPlanV1 {
        mint: mint.key,
        source: source.key,
        destination: destination.key,
        authority,
        amount: quantity,
        source_balance_after: source
            .amount
            .checked_sub(quantity)
            .ok_or(Error::InsufficientTokenBalance)?,
        destination_balance_after: destination
            .amount
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?,
        unchanged_mint_supply: mint.supply,
    })
}

/// Split collateral directly into one bearer token per outcome atom.
#[allow(clippy::too_many_arguments)]
pub fn split_to_bearer<const N: usize>(
    market_key: [u8; 32],
    market: &mut CategoricalMarketV1<N>,
    state: &mut BearerCapabilityV1<N>,
    holder: [u8; 32],
    realm: RealmBindingV1,
    quantity: u64,
    controller: [u8; 32],
    expected_mint_keys: [[u8; 32]; N],
    mints: [MintObservationV1; N],
    destinations: [TokenAccountObservationV1; N],
) -> Result<(CollateralPlanV1, [TokenSupplyPlanV1; N])> {
    require_quantity(quantity)?;
    validate_realm(market, realm)?;
    audit_mints(
        state,
        market_key,
        market,
        controller,
        expected_mint_keys,
        mints,
    )?;
    let mut next_market = *market;
    let mut next_state = *state;
    next_market
        .split_complete_set(quantity)
        .map_err(market_error)?;
    let mut plans = [empty_supply_plan(); N];
    let mut index = 0usize;
    while index < N {
        let mint = mints.get(index).copied().ok_or(Error::InvalidOutcome)?;
        let destination = destinations
            .get(index)
            .copied()
            .ok_or(Error::InvalidOutcome)?;
        destination.validate_holder(mint.key, holder, 0)?;
        next_state.credit(index, quantity)?;
        *plans.get_mut(index).ok_or(Error::InvalidOutcome)? = TokenSupplyPlanV1 {
            operation: TokenSupplyOperationV1::Mint,
            mint: mint.key,
            token_account: destination.key,
            amount: quantity,
            mint_supply_before: mint.supply,
            mint_supply_after: mint
                .supply
                .checked_add(quantity)
                .ok_or(Error::ArithmeticOverflow)?,
            account_balance_before: destination.amount,
            account_balance_after: destination
                .amount
                .checked_add(quantity)
                .ok_or(Error::ArithmeticOverflow)?,
        };
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    next_state.validate_market(market_key, &next_market)?;
    *market = next_market;
    *state = next_state;
    Ok((
        collateral_plan(realm.realm, quantity, CollateralDirectionV1::DepositToHoard),
        plans,
    ))
}

/// Burn one complete bearer set and release exact Hoard backing.
#[allow(clippy::too_many_arguments)]
pub fn merge_from_bearer<const N: usize>(
    market_key: [u8; 32],
    market: &mut CategoricalMarketV1<N>,
    state: &mut BearerCapabilityV1<N>,
    holder: [u8; 32],
    realm: RealmBindingV1,
    quantity: u64,
    controller: [u8; 32],
    expected_mint_keys: [[u8; 32]; N],
    mints: [MintObservationV1; N],
    sources: [TokenAccountObservationV1; N],
) -> Result<(CollateralPlanV1, [TokenSupplyPlanV1; N])> {
    require_quantity(quantity)?;
    validate_realm(market, realm)?;
    audit_mints(
        state,
        market_key,
        market,
        controller,
        expected_mint_keys,
        mints,
    )?;
    let mut next_market = *market;
    let mut next_state = *state;
    let mut plans = [empty_supply_plan(); N];
    let mut index = 0usize;
    while index < N {
        let mint = mints.get(index).copied().ok_or(Error::InvalidOutcome)?;
        let source = sources.get(index).copied().ok_or(Error::InvalidOutcome)?;
        source.validate_holder(mint.key, holder, quantity)?;
        next_state.debit(index, quantity)?;
        *plans.get_mut(index).ok_or(Error::InvalidOutcome)? = burn_plan(mint, source, quantity)?;
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    next_market
        .merge_complete_set(quantity)
        .map_err(market_error)?;
    *market = next_market;
    *state = next_state;
    Ok((
        collateral_plan(
            realm.realm,
            quantity,
            CollateralDirectionV1::WithdrawFromHoard,
        ),
        plans,
    ))
}

/// Redeem native Position claims against terminal Market truth.
pub fn redeem_native<const N: usize>(
    market_key: [u8; 32],
    market: &mut CategoricalMarketV1<N>,
    position: &mut PositionV1<N>,
    owner: [u8; 32],
    realm: RealmBindingV1,
    outcome: usize,
    quantity: u64,
) -> Result<RedemptionPlanV1> {
    require_quantity(quantity)?;
    validate_realm(market, realm)?;
    validate_position(market_key, market, position, owner)?;
    let mut next_market = *market;
    let mut next_position = *position;
    next_position
        .debit_outcome(outcome, quantity)
        .map_err(realm_error)?;
    let payout = next_market
        .redeem_outcome(outcome, quantity)
        .map_err(market_error)?;
    *market = next_market;
    *position = next_position;
    Ok(RedemptionPlanV1 {
        payout: collateral_plan(
            realm.realm,
            payout,
            CollateralDirectionV1::WithdrawFromHoard,
        ),
        bearer_burn: None,
    })
}

/// Burn and redeem bearer claims against terminal Market truth.
#[allow(clippy::too_many_arguments)]
pub fn redeem_bearer<const N: usize>(
    market_key: [u8; 32],
    market: &mut CategoricalMarketV1<N>,
    state: &mut BearerCapabilityV1<N>,
    holder: [u8; 32],
    realm: RealmBindingV1,
    outcome: usize,
    quantity: u64,
    controller: [u8; 32],
    expected_mint_key: [u8; 32],
    mint: MintObservationV1,
    source: TokenAccountObservationV1,
) -> Result<RedemptionPlanV1> {
    require_quantity(quantity)?;
    validate_realm(market, realm)?;
    state.validate_market(market_key, market)?;
    validate_one_mint(state, outcome, controller, expected_mint_key, mint)?;
    source.validate_holder(mint.key, holder, quantity)?;
    let mut next_market = *market;
    let mut next_state = *state;
    next_state.debit(outcome, quantity)?;
    let payout = next_market
        .redeem_outcome(outcome, quantity)
        .map_err(market_error)?;
    let burn = burn_plan(mint, source, quantity)?;
    *market = next_market;
    *state = next_state;
    Ok(RedemptionPlanV1 {
        payout: collateral_plan(
            realm.realm,
            payout,
            CollateralDirectionV1::WithdrawFromHoard,
        ),
        bearer_burn: Some(burn),
    })
}

/// Close every zero-supply Mint and retire the one direct Market child.
#[allow(clippy::too_many_arguments)]
pub fn retire<const N: usize>(
    market_key: [u8; 32],
    market: &mut CategoricalMarketV1<N>,
    state: BearerCapabilityV1<N>,
    manifest_content_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    config_content_id: ContentId,
    config: BearerConfigV1,
    expected_prior_child_count: u64,
    controller: [u8; 32],
    expected_mint_keys: [[u8; 32]; N],
    observations: [MintObservationV1; N],
) -> Result<RetirementPlanV1<N>> {
    if market.root().phase() != Phase::Retiring {
        return Err(Error::InvalidMarketPhase);
    }
    if market.root().identity().capability_manifest_id() != manifest_content_id {
        return Err(Error::ManifestMismatch);
    }
    validate_entry(
        manifest
            .entry(state.manifest_entry_index())
            .map_err(capability_error)?,
        config_content_id,
    )?;
    audit_mints(
        &state,
        market_key,
        market,
        controller,
        expected_mint_keys,
        observations,
    )?;
    if state.accounted_supply().iter().any(|supply| *supply != 0) {
        return Err(Error::OutstandingBearerSupply);
    }
    let mut next_market = *market;
    next_market
        .retire_child(state.generation(), expected_prior_child_count)
        .map_err(market_error)?;
    let after = next_market.root().outstanding_children();
    *market = next_market;
    Ok(RetirementPlanV1 {
        mints: expected_mint_keys,
        rent_refund: config.rent_refund(),
        market_child_count_before: expected_prior_child_count,
        market_child_count_after: after,
    })
}

fn validate_entry(entry: CapabilityEntryV1, config_content_id: ContentId) -> Result<()> {
    if entry.kind_id().to_bytes() != BEARER_CAPABILITY_KIND_ID {
        return Err(Error::CapabilityKindMismatch);
    }
    if entry.release_id().to_bytes() != BEARER_SEMANTIC_RELEASE_ID {
        return Err(Error::CapabilityReleaseMismatch);
    }
    if entry.config_id() != config_content_id {
        return Err(Error::CapabilityConfigMismatch);
    }
    if entry.child_schema_id().to_bytes() != BEARER_CHILD_SCHEMA_ID {
        return Err(Error::ChildSchemaMismatch);
    }
    if entry.child_derivation_id().to_bytes() != BEARER_CHILD_DERIVATION_ID {
        return Err(Error::ChildDerivationMismatch);
    }
    let quote = entry.funding_quote();
    let amounts = quote.amounts();
    let rent = amounts.rent();
    let creation = amounts.creation();
    if quote.realm_collateral().is_some()
        || !matches!(
            rent.asset_class(),
            FundingAssetClassV1::NotApplicable | FundingAssetClassV1::NativeLamports
        )
        || !matches!(
            creation.asset_class(),
            FundingAssetClassV1::NotApplicable | FundingAssetClassV1::NativeLamports
        )
        || amounts.work().amount() != 0
        || amounts.provider().amount() != 0
        || amounts.bounty().amount() != 0
        || amounts.liquidity().amount() != 0
        || amounts.service().amount() != 0
    {
        return Err(Error::ActivationFundingMismatch);
    }
    Ok(())
}

fn validate_realm<const N: usize>(
    market: &CategoricalMarketV1<N>,
    realm: RealmBindingV1,
) -> Result<()> {
    if market.root().identity().realm_id() != realm.content_id {
        return Err(Error::RealmMismatch);
    }
    Ok(())
}

fn validate_position<const N: usize>(
    market_key: [u8; 32],
    market: &CategoricalMarketV1<N>,
    position: &PositionV1<N>,
    owner: [u8; 32],
) -> Result<()> {
    if position.market() != &market_key || position.owner() != &owner {
        return Err(Error::MarketMismatch);
    }
    if position.generation() != market.root().identity().generation() {
        return Err(Error::GenerationMismatch);
    }
    Ok(())
}

fn validate_one_mint<const N: usize>(
    state: &BearerCapabilityV1<N>,
    outcome: usize,
    controller: [u8; 32],
    expected_mint_key: [u8; 32],
    mint: MintObservationV1,
) -> Result<()> {
    let expected_supply = state
        .accounted_supply()
        .get(outcome)
        .copied()
        .ok_or(Error::InvalidOutcome)?;
    mint.validate_profile(expected_mint_key, controller)?;
    if mint.supply != expected_supply {
        return Err(Error::UnaccountedMintSupply);
    }
    Ok(())
}

fn collateral_plan(
    realm: RealmV1,
    amount: u64,
    direction: CollateralDirectionV1,
) -> CollateralPlanV1 {
    CollateralPlanV1 {
        token_program: *realm.token_program(),
        mint: *realm.collateral_mint(),
        amount,
        direction,
    }
}

fn burn_plan(
    mint: MintObservationV1,
    source: TokenAccountObservationV1,
    quantity: u64,
) -> Result<TokenSupplyPlanV1> {
    Ok(TokenSupplyPlanV1 {
        operation: TokenSupplyOperationV1::Burn,
        mint: mint.key,
        token_account: source.key,
        amount: quantity,
        mint_supply_before: mint.supply,
        mint_supply_after: mint
            .supply
            .checked_sub(quantity)
            .ok_or(Error::InsufficientTokenBalance)?,
        account_balance_before: source.amount,
        account_balance_after: source
            .amount
            .checked_sub(quantity)
            .ok_or(Error::InsufficientTokenBalance)?,
    })
}

const fn empty_supply_plan() -> TokenSupplyPlanV1 {
    TokenSupplyPlanV1 {
        operation: TokenSupplyOperationV1::Mint,
        mint: [0; 32],
        token_account: [0; 32],
        amount: 0,
        mint_supply_before: 0,
        mint_supply_after: 0,
        account_balance_before: 0,
        account_balance_after: 0,
    }
}

fn require_unique_keys<const N: usize>(keys: &[[u8; 32]; N]) -> Result<()> {
    for (index, key) in keys.iter().enumerate() {
        require_nonzero(key)?;
        for prior in keys.iter().take(index) {
            if prior == key {
                return Err(Error::AccountAlias);
            }
        }
    }
    Ok(())
}

fn capability_error(error: dclutch_capability_contract::Error) -> Error {
    Error::CapabilityContract { error }
}

fn market_error(error: dclutch_market_contract::Error) -> Error {
    Error::MarketContract { error }
}

fn realm_error(error: dclutch_realm_contract::Error) -> Error {
    Error::RealmContract { error }
}
