//! Total native-custody and Token-2022 receipt transition plans.

use dclutch_bearer_contract::state::{
    BEARER_MINT_BYTES, MintObservationV1, TokenAccountObservationV1,
};
use dclutch_core_contract::Phase;
use dclutch_market_contract::market::CategoricalMarketV1;
use dclutch_realm_contract::PositionV1;

use crate::descriptor::{
    BackingRecipeV1, STRUCTURED_RECEIPT_DECIMALS_V1, StructuredChildDerivationV1,
    StructuredContextV1, StructuredDescriptorDerivationV1, StructuredDescriptorV1,
    custody_owner_derivation_v1, receipt_authority_derivation_v1, receipt_mint_derivation_v1,
};
use crate::{Error, ID_BYTES, Result, require_nonzero, require_quantity};

/// Token-2022 receipt supply operation required by a Structured transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptOperationV1 {
    /// Mint integral receipt atoms only after exact native backing is staged.
    Mint,
    /// Permissioned-burn integral receipt atoms before releasing backing.
    Burn,
}

/// Exact Token-2022 Mint and holder-account delta.
///
/// The observed Mint supply and token Account amount are the only supply and
/// holder truths. No Structured account mirrors either value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceiptSupplyPlanV1 {
    operation: ReceiptOperationV1,
    mint: [u8; ID_BYTES],
    receipt_controller: [u8; ID_BYTES],
    token_account: [u8; ID_BYTES],
    holder: [u8; ID_BYTES],
    amount: u64,
    mint_supply_before: u64,
    mint_supply_after: u64,
    account_balance_before: u64,
    account_balance_after: u64,
}

impl ReceiptSupplyPlanV1 {
    /// Return whether the adapter must mint or permissioned-burn.
    pub const fn operation(self) -> ReceiptOperationV1 {
        self.operation
    }

    /// Return the immutable descriptor-bound receipt Mint.
    pub const fn mint(self) -> [u8; ID_BYTES] {
        self.mint
    }

    /// Return the authority required for mint, permissioned burn, and close.
    pub const fn receipt_controller(self) -> [u8; ID_BYTES] {
        self.receipt_controller
    }

    /// Return the holder Token-2022 Account whose amount changes.
    pub const fn token_account(self) -> [u8; ID_BYTES] {
        self.token_account
    }

    /// Return the holder authority which must authorize the native debit or burn.
    pub const fn holder(self) -> [u8; ID_BYTES] {
        self.holder
    }

    /// Return exact integral receipt atoms minted or burned.
    pub const fn amount(self) -> u64 {
        self.amount
    }

    /// Return authenticated Mint supply before CPI.
    pub const fn mint_supply_before(self) -> u64 {
        self.mint_supply_before
    }

    /// Return required Mint supply after CPI.
    pub const fn mint_supply_after(self) -> u64 {
        self.mint_supply_after
    }

    /// Return authenticated holder Account amount before CPI.
    pub const fn account_balance_before(self) -> u64 {
        self.account_balance_before
    }

    /// Return required holder Account amount after CPI.
    pub const fn account_balance_after(self) -> u64 {
        self.account_balance_after
    }
}

/// Semantic transition represented by one exact plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredTransitionKindV1 {
    /// Native claims entered custody and receipt atoms were minted.
    Wrap,
    /// Receipt atoms were burned and native claims returned to a Position.
    Unwrap,
    /// Receipt atoms and every backed native claim redeemed at terminal truth.
    RedeemTerminal,
}

/// Exact atomic plan returned by wrap, unwrap, or terminal redemption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredTransitionPlanV1<const N: usize> {
    kind: StructuredTransitionKindV1,
    minimum_realization_lot: u64,
    backing: [u64; N],
    receipt: ReceiptSupplyPlanV1,
    collateral_payout_atoms: u64,
}

impl<const N: usize> StructuredTransitionPlanV1<N> {
    /// Return the semantic transition kind.
    pub const fn kind(self) -> StructuredTransitionKindV1 {
        self.kind
    }

    /// Return Product scale represented by one integral receipt atom.
    pub const fn minimum_realization_lot(self) -> u64 {
        self.minimum_realization_lot
    }

    /// Borrow exact native claims moved or redeemed in outcome order.
    pub const fn backing(&self) -> &[u64; N] {
        &self.backing
    }

    /// Return the exact receipt Mint/account CPI plan.
    pub const fn receipt(self) -> ReceiptSupplyPlanV1 {
        self.receipt
    }

    /// Return exact terminal winner collateral, zero for non-redemption plans.
    pub const fn collateral_payout_atoms(self) -> u64 {
        self.collateral_payout_atoms
    }
}

/// Exact initialization requirements for one transferable receipt Mint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceiptMintInitializationV1 {
    mint: [u8; ID_BYTES],
    controller: [u8; ID_BYTES],
    data_len: usize,
    decimals: u8,
    permissioned_burn_required: bool,
    close_authority_required: bool,
}

impl ReceiptMintInitializationV1 {
    /// Return the canonical Mint address.
    pub const fn mint(self) -> [u8; ID_BYTES] {
        self.mint
    }

    /// Return common mint/permissioned-burn/close authority.
    pub const fn controller(self) -> [u8; ID_BYTES] {
        self.controller
    }

    /// Return exact required Token-2022 Mint account width.
    pub const fn data_len(self) -> usize {
        self.data_len
    }

    /// Return immutable zero display decimals.
    pub const fn decimals(self) -> u8 {
        self.decimals
    }

    /// Return whether the PermissionedBurn extension is mandatory.
    pub const fn permissioned_burn_required(self) -> bool {
        self.permissioned_burn_required
    }

    /// Return whether the MintCloseAuthority extension is mandatory.
    pub const fn close_authority_required(self) -> bool {
        self.close_authority_required
    }
}

/// Atomic creation plan for descriptor, receipt Mint, and empty custody Position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredActivationPlanV1<const N: usize> {
    descriptor: StructuredDescriptorV1,
    descriptor_derivation: StructuredDescriptorDerivationV1,
    receipt_mint_derivation: StructuredChildDerivationV1,
    receipt_authority_derivation: StructuredChildDerivationV1,
    custody_owner_derivation: StructuredChildDerivationV1,
    receipt_mint: ReceiptMintInitializationV1,
    custody_position_key: [u8; ID_BYTES],
    custody_position: PositionV1<N>,
    rent_credit: [u8; ID_BYTES],
    market_child_count_before: u64,
    market_child_count_after: u64,
}

impl<const N: usize> StructuredActivationPlanV1<N> {
    /// Return the exact immutable descriptor to persist.
    pub const fn descriptor(self) -> StructuredDescriptorV1 {
        self.descriptor
    }

    /// Return canonical descriptor PDA seeds.
    pub const fn descriptor_derivation(self) -> StructuredDescriptorDerivationV1 {
        self.descriptor_derivation
    }

    /// Return canonical receipt-Mint PDA seeds.
    pub const fn receipt_mint_derivation(self) -> StructuredChildDerivationV1 {
        self.receipt_mint_derivation
    }

    /// Return canonical receipt-controller PDA seeds.
    pub const fn receipt_authority_derivation(self) -> StructuredChildDerivationV1 {
        self.receipt_authority_derivation
    }

    /// Return canonical custody-owner PDA seeds.
    pub const fn custody_owner_derivation(self) -> StructuredChildDerivationV1 {
        self.custody_owner_derivation
    }

    /// Return exact receipt-Mint initialization requirements.
    pub const fn receipt_mint(self) -> ReceiptMintInitializationV1 {
        self.receipt_mint
    }

    /// Return the descriptor-bound canonical custody Position key.
    pub const fn custody_position_key(self) -> [u8; ID_BYTES] {
        self.custody_position_key
    }

    /// Return the exact empty descriptor-owned native Position.
    pub const fn custody_position(self) -> PositionV1<N> {
        self.custody_position
    }

    /// Return the permanent RentCredit for descriptor/Mint/custody principal.
    pub const fn rent_credit(self) -> [u8; ID_BYTES] {
        self.rent_credit
    }

    /// Return the guarded child count before activation.
    pub const fn market_child_count_before(self) -> u64 {
        self.market_child_count_before
    }

    /// Return exact child count after activation.
    pub const fn market_child_count_after(self) -> u64 {
        self.market_child_count_after
    }
}

/// Exact close plan for an economically empty transferable receipt child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredRetirementPlanV1 {
    descriptor_key: [u8; ID_BYTES],
    receipt_mint: [u8; ID_BYTES],
    custody_position: [u8; ID_BYTES],
    rent_credit: [u8; ID_BYTES],
    market_child_count_before: u64,
    market_child_count_after: u64,
}

impl StructuredRetirementPlanV1 {
    /// Return the immutable descriptor account to close.
    pub const fn descriptor_key(self) -> [u8; ID_BYTES] {
        self.descriptor_key
    }

    /// Return the authenticated zero-supply receipt Mint to close.
    pub const fn receipt_mint(self) -> [u8; ID_BYTES] {
        self.receipt_mint
    }

    /// Return the authenticated empty custody Position to close.
    pub const fn custody_position(self) -> [u8; ID_BYTES] {
        self.custody_position
    }

    /// Return the only recipient of recovered rent.
    pub const fn rent_credit(self) -> [u8; ID_BYTES] {
        self.rent_credit
    }

    /// Return child count before the exact decrement.
    pub const fn market_child_count_before(self) -> u64 {
        self.market_child_count_before
    }

    /// Return child count after the exact decrement.
    pub const fn market_child_count_after(self) -> u64 {
        self.market_child_count_after
    }
}

/// Register one immutable descriptor and create its receipt/custody plan.
///
/// The adapter must validate the manifest entry, recompute content IDs/PDAs,
/// debit only typed prepaid capability funding, create the exact Mint and
/// Position, and commit this Market mutation atomically. No Hoard principal or
/// future revenue appears in the plan.
pub fn activate<const N: usize>(
    context: StructuredContextV1<N>,
    market_key: [u8; ID_BYTES],
    market: &mut CategoricalMarketV1<N>,
    expected_prior_child_count: u64,
) -> Result<StructuredActivationPlanV1<N>> {
    context.validate_market(market_key, market)?;
    if !matches!(market.root().phase(), Phase::Founding | Phase::Open) {
        return Err(Error::InvalidMarketPhase);
    }
    let descriptor = context.descriptor();
    let mut next_market = *market;
    next_market
        .register_child(descriptor.generation(), expected_prior_child_count)
        .map_err(market_error)?;
    let custody_position = PositionV1::empty(
        market_key,
        descriptor.custody_owner(),
        descriptor.generation(),
    )
    .map_err(realm_error)?;
    let plan = StructuredActivationPlanV1 {
        descriptor,
        descriptor_derivation: StructuredDescriptorDerivationV1::new(descriptor)?,
        receipt_mint_derivation: receipt_mint_derivation_v1(context.descriptor_key())?,
        receipt_authority_derivation: receipt_authority_derivation_v1(context.descriptor_key())?,
        custody_owner_derivation: custody_owner_derivation_v1(context.descriptor_key())?,
        receipt_mint: ReceiptMintInitializationV1 {
            mint: descriptor.receipt_mint(),
            controller: descriptor.receipt_authority(),
            data_len: BEARER_MINT_BYTES,
            decimals: STRUCTURED_RECEIPT_DECIMALS_V1,
            permissioned_burn_required: true,
            close_authority_required: true,
        },
        custody_position_key: descriptor.custody_position(),
        custody_position,
        rent_credit: descriptor.rent_credit(),
        market_child_count_before: expected_prior_child_count,
        market_child_count_after: next_market.root().outstanding_children(),
    };
    *market = next_market;
    Ok(plan)
}

/// Check the sole Mint supply against byte-exact native Position backing.
pub fn audit_backing<const N: usize>(
    context: StructuredContextV1<N>,
    market_key: [u8; ID_BYTES],
    market: &CategoricalMarketV1<N>,
    custody_position_key: [u8; ID_BYTES],
    custody: &PositionV1<N>,
    mint: MintObservationV1,
) -> Result<()> {
    context.validate_market(market_key, market)?;
    validate_mint(context, mint)?;
    validate_custody(context, market_key, custody_position_key, custody)?;
    validate_backing_amount(context, market, custody, mint.supply)
}

/// Move Product coefficients from one native Position into custody and return
/// one exact Token-2022 MintTo plan.
#[allow(clippy::too_many_arguments)]
pub fn wrap<const N: usize>(
    context: StructuredContextV1<N>,
    market_key: [u8; ID_BYTES],
    market: &CategoricalMarketV1<N>,
    owner: [u8; ID_BYTES],
    owner_position: &mut PositionV1<N>,
    custody_position_key: [u8; ID_BYTES],
    custody: &mut PositionV1<N>,
    mint: MintObservationV1,
    destination: TokenAccountObservationV1,
    units: u64,
) -> Result<StructuredTransitionPlanV1<N>> {
    require_quantity(units)?;
    if !matches!(market.root().phase(), Phase::Open | Phase::Resolved) {
        return Err(Error::InvalidMarketPhase);
    }
    validate_owner_snapshot(
        context,
        market_key,
        market,
        owner,
        owner_position,
        custody_position_key,
        custody,
        mint,
    )?;
    validate_token_account(context, owner, destination, 0)?;
    require_action_accounts_distinct(context, owner, destination.key)?;
    let backing = scaled_backing(context.recipe(), units)?;
    let supply_after = mint
        .supply
        .checked_add(units)
        .ok_or(Error::ArithmeticOverflow)?;
    let account_after = destination
        .amount
        .checked_add(units)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut next_owner = *owner_position;
    let mut next_custody = *custody;
    debit_vector(&mut next_owner, &backing)?;
    credit_vector(&mut next_custody, &backing)?;
    validate_visible_positions(market, &next_owner, &next_custody)?;
    validate_backing_amount(context, market, &next_custody, supply_after)?;
    *owner_position = next_owner;
    *custody = next_custody;
    Ok(transition_plan(
        StructuredTransitionKindV1::Wrap,
        context,
        backing,
        ReceiptSupplyPlanV1 {
            operation: ReceiptOperationV1::Mint,
            mint: mint.key,
            receipt_controller: context.descriptor().receipt_authority(),
            token_account: destination.key,
            holder: owner,
            amount: units,
            mint_supply_before: mint.supply,
            mint_supply_after: supply_after,
            account_balance_before: destination.amount,
            account_balance_after: account_after,
        },
        0,
    ))
}

/// Permissioned-burn receipt atoms and return their native coefficient vector.
#[allow(clippy::too_many_arguments)]
pub fn unwrap<const N: usize>(
    context: StructuredContextV1<N>,
    market_key: [u8; ID_BYTES],
    market: &CategoricalMarketV1<N>,
    owner: [u8; ID_BYTES],
    owner_position: &mut PositionV1<N>,
    custody_position_key: [u8; ID_BYTES],
    custody: &mut PositionV1<N>,
    mint: MintObservationV1,
    source: TokenAccountObservationV1,
    units: u64,
) -> Result<StructuredTransitionPlanV1<N>> {
    require_quantity(units)?;
    if !matches!(
        market.root().phase(),
        Phase::Open | Phase::Resolved | Phase::Retiring
    ) {
        return Err(Error::InvalidMarketPhase);
    }
    validate_owner_snapshot(
        context,
        market_key,
        market,
        owner,
        owner_position,
        custody_position_key,
        custody,
        mint,
    )?;
    validate_token_account(context, owner, source, units)?;
    require_action_accounts_distinct(context, owner, source.key)?;
    let backing = scaled_backing(context.recipe(), units)?;
    let supply_after = mint
        .supply
        .checked_sub(units)
        .ok_or(Error::BackingMismatch)?;
    let account_after = source
        .amount
        .checked_sub(units)
        .ok_or(Error::BackingMismatch)?;
    let mut next_owner = *owner_position;
    let mut next_custody = *custody;
    debit_vector(&mut next_custody, &backing)?;
    credit_vector(&mut next_owner, &backing)?;
    validate_visible_positions(market, &next_owner, &next_custody)?;
    validate_backing_amount(context, market, &next_custody, supply_after)?;
    *owner_position = next_owner;
    *custody = next_custody;
    Ok(transition_plan(
        StructuredTransitionKindV1::Unwrap,
        context,
        backing,
        ReceiptSupplyPlanV1 {
            operation: ReceiptOperationV1::Burn,
            mint: mint.key,
            receipt_controller: context.descriptor().receipt_authority(),
            token_account: source.key,
            holder: owner,
            amount: units,
            mint_supply_before: mint.supply,
            mint_supply_after: supply_after,
            account_balance_before: source.amount,
            account_balance_after: account_after,
        },
        0,
    ))
}

/// Permissioned-burn receipt atoms, consume every backed categorical claim,
/// and return only the canonical winner payout.
///
/// Every nonzero coefficient invokes [`CategoricalMarketV1::redeem_outcome`].
/// Losing supply retires together with winning supply rather than remaining in
/// custody. All candidates commit only after every outcome and payout check.
#[allow(clippy::too_many_arguments)]
pub fn redeem_terminal<const N: usize>(
    context: StructuredContextV1<N>,
    market_key: [u8; ID_BYTES],
    market: &mut CategoricalMarketV1<N>,
    owner: [u8; ID_BYTES],
    custody_position_key: [u8; ID_BYTES],
    custody: &mut PositionV1<N>,
    mint: MintObservationV1,
    source: TokenAccountObservationV1,
    units: u64,
) -> Result<StructuredTransitionPlanV1<N>> {
    require_quantity(units)?;
    if !matches!(market.root().phase(), Phase::Resolved | Phase::Retiring) {
        return Err(Error::InvalidMarketPhase);
    }
    audit_backing(
        context,
        market_key,
        market,
        custody_position_key,
        custody,
        mint,
    )?;
    validate_token_account(context, owner, source, units)?;
    require_action_accounts_distinct(context, owner, source.key)?;
    let resolution = market
        .settlement()
        .resolution()
        .ok_or(Error::InvalidMarketPhase)?;
    let winner = usize::from(resolution.winner());
    let backing = scaled_backing(context.recipe(), units)?;
    let supply_after = mint
        .supply
        .checked_sub(units)
        .ok_or(Error::BackingMismatch)?;
    let account_after = source
        .amount
        .checked_sub(units)
        .ok_or(Error::BackingMismatch)?;
    let mut next_market = *market;
    let mut next_custody = *custody;
    let mut collateral_payout_atoms = 0u64;
    for (outcome, amount) in backing.iter().copied().enumerate() {
        if amount == 0 {
            continue;
        }
        next_custody
            .debit_outcome(outcome, amount)
            .map_err(realm_error)?;
        let payout = next_market
            .redeem_outcome(outcome, amount)
            .map_err(market_error)?;
        let expected = if outcome == winner { amount } else { 0 };
        if payout != expected {
            return Err(Error::RedemptionPayoutMismatch);
        }
        if outcome == winner {
            collateral_payout_atoms = collateral_payout_atoms
                .checked_add(payout)
                .ok_or(Error::ArithmeticOverflow)?;
        }
    }
    validate_backing_amount(context, &next_market, &next_custody, supply_after)?;
    *market = next_market;
    *custody = next_custody;
    Ok(transition_plan(
        StructuredTransitionKindV1::RedeemTerminal,
        context,
        backing,
        ReceiptSupplyPlanV1 {
            operation: ReceiptOperationV1::Burn,
            mint: mint.key,
            receipt_controller: context.descriptor().receipt_authority(),
            token_account: source.key,
            holder: owner,
            amount: units,
            mint_supply_before: mint.supply,
            mint_supply_after: supply_after,
            account_balance_before: source.amount,
            account_balance_after: account_after,
        },
        collateral_payout_atoms,
    ))
}

/// Retire only an authenticated zero-supply Mint and byte-exact empty custody.
#[allow(clippy::too_many_arguments)]
pub fn retire<const N: usize>(
    context: StructuredContextV1<N>,
    market_key: [u8; ID_BYTES],
    market: &mut CategoricalMarketV1<N>,
    custody_position_key: [u8; ID_BYTES],
    custody: &PositionV1<N>,
    mint: MintObservationV1,
    expected_prior_child_count: u64,
) -> Result<StructuredRetirementPlanV1> {
    if market.root().phase() != Phase::Retiring {
        return Err(Error::InvalidMarketPhase);
    }
    audit_backing(
        context,
        market_key,
        market,
        custody_position_key,
        custody,
        mint,
    )?;
    if mint.supply != 0 || !custody.is_empty() {
        return Err(Error::OutstandingStructuredBacking);
    }
    let mut next_market = *market;
    next_market
        .retire_child(
            context.descriptor().generation(),
            expected_prior_child_count,
        )
        .map_err(market_error)?;
    let plan = StructuredRetirementPlanV1 {
        descriptor_key: context.descriptor_key(),
        receipt_mint: context.descriptor().receipt_mint(),
        custody_position: context.descriptor().custody_position(),
        rent_credit: context.descriptor().rent_credit(),
        market_child_count_before: expected_prior_child_count,
        market_child_count_after: next_market.root().outstanding_children(),
    };
    *market = next_market;
    Ok(plan)
}

fn transition_plan<const N: usize>(
    kind: StructuredTransitionKindV1,
    context: StructuredContextV1<N>,
    backing: [u64; N],
    receipt: ReceiptSupplyPlanV1,
    collateral_payout_atoms: u64,
) -> StructuredTransitionPlanV1<N> {
    StructuredTransitionPlanV1 {
        kind,
        minimum_realization_lot: context.recipe().minimum_realization_lot(),
        backing,
        receipt,
        collateral_payout_atoms,
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_owner_snapshot<const N: usize>(
    context: StructuredContextV1<N>,
    market_key: [u8; ID_BYTES],
    market: &CategoricalMarketV1<N>,
    owner: [u8; ID_BYTES],
    owner_position: &PositionV1<N>,
    custody_position_key: [u8; ID_BYTES],
    custody: &PositionV1<N>,
    mint: MintObservationV1,
) -> Result<()> {
    require_nonzero(&owner)?;
    if owner == context.descriptor().custody_owner() {
        return Err(Error::PositionAliasing);
    }
    audit_backing(
        context,
        market_key,
        market,
        custody_position_key,
        custody,
        mint,
    )?;
    validate_position_binding(
        market_key,
        context.descriptor().generation(),
        owner,
        owner_position,
    )?;
    validate_visible_positions(market, owner_position, custody)
}

fn validate_mint<const N: usize>(
    context: StructuredContextV1<N>,
    mint: MintObservationV1,
) -> Result<()> {
    mint.validate_profile(
        context.descriptor().receipt_mint(),
        context.descriptor().receipt_authority(),
    )
    .map_err(bearer_error)
}

fn validate_token_account<const N: usize>(
    context: StructuredContextV1<N>,
    holder: [u8; ID_BYTES],
    account: TokenAccountObservationV1,
    minimum_amount: u64,
) -> Result<()> {
    account
        .validate_holder(context.descriptor().receipt_mint(), holder, minimum_amount)
        .map_err(bearer_error)
}

fn validate_custody<const N: usize>(
    context: StructuredContextV1<N>,
    market_key: [u8; ID_BYTES],
    custody_position_key: [u8; ID_BYTES],
    custody: &PositionV1<N>,
) -> Result<()> {
    if custody_position_key != context.descriptor().custody_position() {
        return Err(Error::CustodyPositionMismatch);
    }
    validate_position_binding(
        market_key,
        context.descriptor().generation(),
        context.descriptor().custody_owner(),
        custody,
    )
}

fn validate_position_binding<const N: usize>(
    market_key: [u8; ID_BYTES],
    generation: u64,
    owner: [u8; ID_BYTES],
    position: &PositionV1<N>,
) -> Result<()> {
    if position.market() != &market_key {
        return Err(Error::MarketMismatch);
    }
    if position.generation() != generation {
        return Err(Error::GenerationMismatch);
    }
    if position.owner() != &owner {
        return Err(Error::PositionOwnerMismatch);
    }
    Ok(())
}

fn validate_backing_amount<const N: usize>(
    context: StructuredContextV1<N>,
    market: &CategoricalMarketV1<N>,
    custody: &PositionV1<N>,
    mint_supply: u64,
) -> Result<()> {
    let expected = scaled_backing(context.recipe(), mint_supply)?;
    if custody.balances() != &expected {
        return Err(Error::BackingMismatch);
    }
    for (custody_amount, aggregate) in custody.balances().iter().zip(market.supply()) {
        if custody_amount > aggregate {
            return Err(Error::MarketSupplyMismatch);
        }
    }
    Ok(())
}

fn validate_visible_positions<const N: usize>(
    market: &CategoricalMarketV1<N>,
    owner: &PositionV1<N>,
    custody: &PositionV1<N>,
) -> Result<()> {
    for ((owner_amount, custody_amount), aggregate) in owner
        .balances()
        .iter()
        .zip(custody.balances())
        .zip(market.supply())
    {
        let visible = owner_amount
            .checked_add(*custody_amount)
            .ok_or(Error::ArithmeticOverflow)?;
        if visible > *aggregate {
            return Err(Error::MarketSupplyMismatch);
        }
    }
    Ok(())
}

fn require_action_accounts_distinct<const N: usize>(
    context: StructuredContextV1<N>,
    owner: [u8; ID_BYTES],
    token_account: [u8; ID_BYTES],
) -> Result<()> {
    require_nonzero(&token_account)?;
    let physical = [
        context.descriptor_key(),
        context.descriptor().market(),
        context.descriptor().receipt_mint(),
        context.descriptor().receipt_authority(),
        context.descriptor().custody_position(),
        context.descriptor().custody_owner(),
        owner,
        token_account,
    ];
    for (index, value) in physical.iter().enumerate() {
        if physical.iter().take(index).any(|prior| prior == value) {
            return Err(Error::AccountAlias);
        }
    }
    Ok(())
}

fn scaled_backing<const N: usize>(recipe: BackingRecipeV1<N>, units: u64) -> Result<[u64; N]> {
    let mut output = [0; N];
    for (destination, coefficient) in output.iter_mut().zip(recipe.coefficients()) {
        *destination = coefficient
            .checked_mul(units)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(output)
}

fn debit_vector<const N: usize>(position: &mut PositionV1<N>, amounts: &[u64; N]) -> Result<()> {
    let mut next = *position;
    for (outcome, amount) in amounts.iter().copied().enumerate() {
        if amount != 0 {
            next.debit_outcome(outcome, amount).map_err(realm_error)?;
        }
    }
    *position = next;
    Ok(())
}

fn credit_vector<const N: usize>(position: &mut PositionV1<N>, amounts: &[u64; N]) -> Result<()> {
    let mut next = *position;
    for (outcome, amount) in amounts.iter().copied().enumerate() {
        if amount != 0 {
            next.credit_outcome(outcome, amount).map_err(realm_error)?;
        }
    }
    *position = next;
    Ok(())
}

fn market_error(error: dclutch_market_contract::Error) -> Error {
    Error::MarketContract { error }
}

fn realm_error(error: dclutch_realm_contract::Error) -> Error {
    Error::RealmContract { error }
}

fn bearer_error(error: dclutch_bearer_contract::Error) -> Error {
    Error::BearerContract { error }
}
