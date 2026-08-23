//! Current full-width Structured lifecycle over canonical V3 liability owners.
//!
//! This module is the replacement for the historical `MarketAccount`/`Terms`/
//! `SupplyLedger` projection used by the model-only handler.  Every transition
//! starts from the exact current owners: Position/Replay V3 projections,
//! Hoard V2, ClaimLedger V3, and, for terminal redemption, Resolution V5.
//! It stages complete postimages without moving an external collateral token;
//! an SBF composer must authenticate the accounts, write every returned owner
//! atomically, and reconcile the Token-2022 wrapper delta separately.

use clutch_collateral_adapter_v2::{
    prepare_complete_set_reclassification_v3, BoundCollateralProfileV2, ClaimLedgerV3,
    CompleteSetReclassificationKindV3, HoardV2, MarketLiabilityLifecycleV1,
    PositionV3Sha256Backend, ResolutionStateV5, ResolutionV5,
};
use clutch_structured_claim::BackingPlan;

use crate::runtime_contract::{
    DescriptorStateV1, PositionProjectionV1, StructuredClaimActionV1, WrapperMintProjectionV1,
    WrapperQuantityPayloadV1, WrapperTokenProjectionV1, MAX_OUTCOMES,
};
use crate::{is_zero, BoundDescriptorV1, Error, Key, Result};

/// Exact receipt domain for one current full-width Structured transition.
pub const CURRENT_STRUCTURED_TRANSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/current-transition/v1\0";
/// Exact projection domain used inside the transition receipt.
pub const CURRENT_STRUCTURED_POSITION_PROJECTION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/current-position-projection/v1\0";

const POSITION_PROJECTION_PREIMAGE_BYTES: usize = 232;
const TRANSITION_PREIMAGE_BYTES: usize = 808;

/// Current physical accounts bound by a quantity-changing Structured route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentStructuredQuantityAccountsV1 {
    /// Wrapper-owned descriptor account.
    pub descriptor: Key,
    /// Canonical series-scoped wrapper Product identity.
    pub wrapper_product_id: Key,
    /// General-purpose user Position V3 account.
    pub user_position: Key,
    /// General-purpose user Replay V3 account.
    pub user_replay: Key,
    /// Structured-purpose vault Position V3 account.
    pub vault_position: Key,
    /// Structured-purpose vault Replay V3 account.
    pub vault_replay: Key,
    /// Extension-free wrapper mint.
    pub mint: Key,
    /// Participating wrapper-token account.
    pub holder: Key,
    /// Transaction signer controlling the General Position and holder token.
    pub actor: Key,
}

/// Current physical accounts bound by a vault-only compaction route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentStructuredVaultAccountsV1 {
    /// Wrapper-owned descriptor account.
    pub descriptor: Key,
    /// Canonical series-scoped wrapper Product identity.
    pub wrapper_product_id: Key,
    /// Structured-purpose vault Position V3 account.
    pub vault_position: Key,
    /// Structured-purpose vault Replay V3 account.
    pub vault_replay: Key,
    /// Extension-free wrapper mint.
    pub mint: Key,
}

/// Exact current liability owners consumed by every non-canonical route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentStructuredLiabilitiesV1 {
    /// Canonical Hoard V2 prestate.
    pub hoard: HoardV2,
    /// Canonical ClaimLedger V3 prestate.
    pub claim_ledger: ClaimLedgerV3,
}

/// Complete postimages for one current full-width Structured transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentStructuredTransitionPlanV1 {
    /// Exact family-local route.
    pub action: StructuredClaimActionV1,
    /// User Position/Replay successor; absent only for compaction.
    pub user_after: Option<PositionProjectionV1>,
    /// Vault Position/Replay successor.
    pub vault_after: PositionProjectionV1,
    /// Complete Hoard V2 successor.
    pub hoard_after: HoardV2,
    /// Complete ClaimLedger V3 successor.
    pub claim_ledger_after: ClaimLedgerV3,
    /// Actual wrapper-mint supply before the wrapper CPI.
    pub mint_supply_before: u64,
    /// Required wrapper-mint supply after the wrapper CPI.
    pub mint_supply_after: u64,
    /// Actual holder balance before the wrapper CPI; zero for compaction.
    pub holder_before: u64,
    /// Required holder balance after the wrapper CPI; zero for compaction.
    pub holder_after: u64,
    /// Wrapper quantity minted or burned; zero only for compaction.
    pub wrapper_quantity: u64,
    /// Complete-set floor reclassified between locked principal and cash.
    pub complete_set_atoms: u64,
    /// Exact terminal cash payout; zero for non-redemption routes.
    pub payout_cash_atoms: u64,
    /// Beneficiary-free cash liability erased by compaction.
    pub donated_cash_atoms: u64,
    /// Beneficiary-free native liabilities erased by compaction.
    pub donated_internal: [u64; MAX_OUTCOMES],
    /// Hoard semantic identity before the transition.
    pub hoard_before_id: Key,
    /// Hoard semantic identity after the transition.
    pub hoard_after_id: Key,
    /// ClaimLedger semantic identity before the transition.
    pub claim_ledger_before_id: Key,
    /// ClaimLedger semantic identity after the transition.
    pub claim_ledger_after_id: Key,
    /// Resolution semantic identity used by redemption; zero otherwise.
    pub resolution_semantic_id: Key,
    /// Existing complete-set receipt incorporated by full wrap/unwind.
    pub liability_receipt_id: Key,
    /// Receipt committing every account, projection, owner, and integer above.
    pub transition_id: Key,
}

/// Prepare full-vector custody followed by current complete-set compression.
#[allow(clippy::too_many_arguments)]
pub fn prepare_current_wrap_full_v1<B: PositionV3Sha256Backend>(
    descriptor: &BoundDescriptorV1,
    collateral: BoundCollateralProfileV2,
    accounts: CurrentStructuredQuantityAccountsV1,
    liabilities: CurrentStructuredLiabilitiesV1,
    mint: WrapperMintProjectionV1,
    holder: WrapperTokenProjectionV1,
    user: PositionProjectionV1,
    vault: PositionProjectionV1,
    request: WrapperQuantityPayloadV1,
    backend: &B,
) -> Result<CurrentStructuredTransitionPlanV1> {
    let quantity = request.quantity;
    let backing = preflight_quantity(
        descriptor,
        collateral,
        accounts,
        liabilities,
        mint,
        holder,
        user,
        vault,
        request,
    )?;
    let width = usize::from(backing.outcome_count);
    let complete_set_atoms = quantity
        .checked_mul(backing.cash_per_wrapper)
        .ok_or(Error::Arithmetic)?;
    let full = scaled_vector(
        descriptor.identity().claim.vector.coefficients,
        quantity,
        width,
    )?;
    let residual = scaled_vector(backing.residual_eggs_per_wrapper, quantity, width)?;

    let mut user_after = user;
    let mut vault_after = vault;
    let mut index = 0usize;
    while index < width {
        user_after.internal[index] = user_after.internal[index]
            .checked_sub(full[index])
            .ok_or(Error::Runtime(
                crate::runtime_contract::Error::InsufficientFreeAssets,
            ))?;
        vault_after.internal[index] = vault_after.internal[index]
            .checked_add(residual[index])
            .ok_or(Error::Arithmetic)?;
        index += 1;
    }
    vault_after.cash_atoms = vault_after
        .cash_atoms
        .checked_add(complete_set_atoms)
        .ok_or(Error::Arithmetic)?;
    advance_pair(&mut user_after, &mut vault_after)?;

    let (hoard_after, claim_ledger_after, liability_receipt_id) =
        if complete_set_atoms == 0 {
            (liabilities.hoard, liabilities.claim_ledger, [0; 32])
        } else {
            let reclassification = prepare_complete_set_reclassification_v3(
                liabilities.hoard,
                liabilities.claim_ledger,
                CompleteSetReclassificationKindV3::Merge,
                complete_set_atoms,
                backend,
            )
            .map_err(|_| Error::BaseClosureMismatch)?;
            (
                reclassification.hoard_after,
                reclassification.claim_ledger_after,
                reclassification.receipt_id.bytes(),
            )
        };
    let mint_supply_after = mint
        .supply
        .checked_add(quantity)
        .ok_or(Error::Arithmetic)?;
    let holder_after = holder
        .amount
        .checked_add(quantity)
        .ok_or(Error::Arithmetic)?;
    validate_wrapper_coverage(backing, mint_supply_after, vault_after)?;
    finish_quantity_plan(
        descriptor,
        accounts,
        StructuredClaimActionV1::WrapFull,
        liabilities,
        user,
        user_after,
        vault,
        vault_after,
        hoard_after,
        claim_ledger_after,
        mint.supply,
        mint_supply_after,
        holder.amount,
        holder_after,
        quantity,
        complete_set_atoms,
        0,
        [0; MAX_OUTCOMES],
        [0; 32],
        liability_receipt_id,
        backend,
    )
}

/// Prepare wrapper burning, current complete-set expansion, and full return.
#[allow(clippy::too_many_arguments)]
pub fn prepare_current_unwrap_full_v1<B: PositionV3Sha256Backend>(
    descriptor: &BoundDescriptorV1,
    collateral: BoundCollateralProfileV2,
    accounts: CurrentStructuredQuantityAccountsV1,
    liabilities: CurrentStructuredLiabilitiesV1,
    mint: WrapperMintProjectionV1,
    holder: WrapperTokenProjectionV1,
    user: PositionProjectionV1,
    vault: PositionProjectionV1,
    request: WrapperQuantityPayloadV1,
    backend: &B,
) -> Result<CurrentStructuredTransitionPlanV1> {
    let quantity = request.quantity;
    let backing = preflight_quantity(
        descriptor,
        collateral,
        accounts,
        liabilities,
        mint,
        holder,
        user,
        vault,
        request,
    )?;
    let width = usize::from(backing.outcome_count);
    let complete_set_atoms = quantity
        .checked_mul(backing.cash_per_wrapper)
        .ok_or(Error::Arithmetic)?;
    let full = scaled_vector(
        descriptor.identity().claim.vector.coefficients,
        quantity,
        width,
    )?;
    let residual = scaled_vector(backing.residual_eggs_per_wrapper, quantity, width)?;

    let mut user_after = user;
    let mut vault_after = vault;
    vault_after.cash_atoms = vault_after
        .cash_atoms
        .checked_sub(complete_set_atoms)
        .ok_or(Error::Runtime(
            crate::runtime_contract::Error::InsufficientFreeAssets,
        ))?;
    let mut index = 0usize;
    while index < width {
        vault_after.internal[index] = vault_after.internal[index]
            .checked_sub(residual[index])
            .ok_or(Error::Runtime(
                crate::runtime_contract::Error::InsufficientFreeAssets,
            ))?;
        user_after.internal[index] = user_after.internal[index]
            .checked_add(full[index])
            .ok_or(Error::Arithmetic)?;
        index += 1;
    }
    advance_pair(&mut user_after, &mut vault_after)?;

    let (hoard_after, claim_ledger_after, liability_receipt_id) =
        if complete_set_atoms == 0 {
            (liabilities.hoard, liabilities.claim_ledger, [0; 32])
        } else {
            let reclassification = prepare_complete_set_reclassification_v3(
                liabilities.hoard,
                liabilities.claim_ledger,
                CompleteSetReclassificationKindV3::Split,
                complete_set_atoms,
                backend,
            )
            .map_err(|_| Error::BaseClosureMismatch)?;
            (
                reclassification.hoard_after,
                reclassification.claim_ledger_after,
                reclassification.receipt_id.bytes(),
            )
        };
    let mint_supply_after = mint
        .supply
        .checked_sub(quantity)
        .ok_or(Error::Token2022Boundary)?;
    let holder_after = holder
        .amount
        .checked_sub(quantity)
        .ok_or(Error::Token2022Boundary)?;
    validate_wrapper_coverage(backing, mint_supply_after, vault_after)?;
    finish_quantity_plan(
        descriptor,
        accounts,
        StructuredClaimActionV1::UnwrapFull,
        liabilities,
        user,
        user_after,
        vault,
        vault_after,
        hoard_after,
        claim_ledger_after,
        mint.supply,
        mint_supply_after,
        holder.amount,
        holder_after,
        quantity,
        complete_set_atoms,
        0,
        [0; MAX_OUTCOMES],
        [0; 32],
        liability_receipt_id,
        backend,
    )
}

/// Prepare beneficiary-free destruction of every vault surplus atom.
#[allow(clippy::too_many_arguments)]
pub fn prepare_current_compact_donation_v1<B: PositionV3Sha256Backend>(
    descriptor: &BoundDescriptorV1,
    collateral: BoundCollateralProfileV2,
    accounts: CurrentStructuredVaultAccountsV1,
    liabilities: CurrentStructuredLiabilitiesV1,
    mint: WrapperMintProjectionV1,
    vault: PositionProjectionV1,
    backend: &B,
) -> Result<CurrentStructuredTransitionPlanV1> {
    let backing = preflight_vault(
        descriptor,
        collateral,
        accounts,
        liabilities,
        mint,
        vault,
    )?;
    let width = usize::from(backing.outcome_count);
    let required_cash = mint
        .supply
        .checked_mul(backing.cash_per_wrapper)
        .ok_or(Error::Arithmetic)?;
    let donated_cash_atoms = vault
        .cash_atoms
        .checked_sub(required_cash)
        .ok_or(Error::BaseClosureMismatch)?;
    let required_internal =
        scaled_vector(backing.residual_eggs_per_wrapper, mint.supply, width)?;
    let mut donated_internal = [0_u64; MAX_OUTCOMES];
    let mut any = donated_cash_atoms != 0;
    let mut index = 0usize;
    while index < width {
        donated_internal[index] = vault.internal[index]
            .checked_sub(required_internal[index])
            .ok_or(Error::BaseClosureMismatch)?;
        any |= donated_internal[index] != 0;
        index += 1;
    }
    if !any {
        return Err(Error::Runtime(
            crate::runtime_contract::Error::ZeroQuantity,
        ));
    }

    let mut vault_after = vault;
    vault_after.cash_atoms = required_cash;
    vault_after.internal = required_internal;
    vault_after.replay_sequence = vault_after
        .replay_sequence
        .checked_add(1)
        .ok_or(Error::Arithmetic)?;
    let hoard_after = HoardV2 {
        cash_liability_atoms: liabilities
            .hoard
            .cash_liability_atoms
            .checked_sub(donated_cash_atoms)
            .ok_or(Error::BaseClosureMismatch)?,
        ..liabilities.hoard
    };
    let mut aggregate_internal_supply = liabilities.claim_ledger.aggregate_internal_supply;
    index = 0;
    while index < width {
        aggregate_internal_supply[index] = aggregate_internal_supply[index]
            .checked_sub(donated_internal[index])
            .ok_or(Error::BaseClosureMismatch)?;
        index += 1;
    }
    let claim_ledger_after = ClaimLedgerV3 {
        aggregate_internal_supply,
        ..liabilities.claim_ledger
    };
    validate_liability_successors(hoard_after, claim_ledger_after)?;

    let quantity_accounts = CurrentStructuredQuantityAccountsV1 {
        descriptor: accounts.descriptor,
        wrapper_product_id: accounts.wrapper_product_id,
        user_position: [0; 32],
        user_replay: [0; 32],
        vault_position: accounts.vault_position,
        vault_replay: accounts.vault_replay,
        mint: accounts.mint,
        holder: [0; 32],
        actor: [0; 32],
    };
    finish_plan(
        descriptor,
        quantity_accounts,
        StructuredClaimActionV1::CompactDonation,
        liabilities,
        None,
        None,
        vault,
        vault_after,
        hoard_after,
        claim_ledger_after,
        mint.supply,
        mint.supply,
        0,
        0,
        0,
        0,
        0,
        donated_cash_atoms,
        donated_internal,
        [0; 32],
        [0; 32],
        backend,
    )
}

/// Prepare exact aggregate terminal redemption against Resolution V5.
#[allow(clippy::too_many_arguments)]
pub fn prepare_current_redeem_terminal_v1<B: PositionV3Sha256Backend>(
    descriptor: &BoundDescriptorV1,
    collateral: BoundCollateralProfileV2,
    accounts: CurrentStructuredQuantityAccountsV1,
    liabilities: CurrentStructuredLiabilitiesV1,
    resolution_account: Key,
    resolution: ResolutionV5,
    mint: WrapperMintProjectionV1,
    holder: WrapperTokenProjectionV1,
    user: PositionProjectionV1,
    vault: PositionProjectionV1,
    request: WrapperQuantityPayloadV1,
    backend: &B,
) -> Result<CurrentStructuredTransitionPlanV1> {
    let quantity = request.quantity;
    let backing = preflight_quantity(
        descriptor,
        collateral,
        accounts,
        liabilities,
        mint,
        holder,
        user,
        vault,
        request,
    )?;
    resolution
        .validate()
        .map_err(|_| Error::BaseClosureMismatch)?;
    if liabilities.hoard.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || liabilities.claim_ledger.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || resolution.state != ResolutionStateV5::Finalized
        || is_zero(&resolution_account)
        || liabilities.claim_ledger.resolution_account.bytes() != resolution_account
        || resolution.facts.market_instance_id != liabilities.hoard.market_instance_id
        || resolution.facts.native_claim_basis_id != liabilities.claim_ledger.native_claim_basis_id
        || resolution.facts.outcome_count != backing.outcome_count
    {
        return Err(Error::BaseClosureMismatch);
    }
    let width = usize::from(backing.outcome_count);
    let complete_set_atoms = quantity
        .checked_mul(backing.cash_per_wrapper)
        .ok_or(Error::Arithmetic)?;
    let residual = scaled_vector(backing.residual_eggs_per_wrapper, quantity, width)?;
    let mut residual_numerator = 0_u128;
    let mut index = 0usize;
    while index < width {
        residual_numerator = residual_numerator
            .checked_add(
                u128::from(residual[index])
                    .checked_mul(u128::from(resolution.facts.payout_weights[index]))
                    .ok_or(Error::Arithmetic)?,
            )
            .ok_or(Error::Arithmetic)?;
        index += 1;
    }
    let denominator = u128::from(resolution.facts.payout_denominator);
    if residual_numerator % denominator != 0 {
        return Err(Error::Runtime(
            crate::runtime_contract::Error::EconomicTransitionRefused,
        ));
    }
    let residual_payout = u64::try_from(residual_numerator / denominator)
        .map_err(|_| Error::Arithmetic)?;
    let payout_cash_atoms = complete_set_atoms
        .checked_add(residual_payout)
        .ok_or(Error::Arithmetic)?;

    let mut user_after = user;
    let mut vault_after = vault;
    vault_after.cash_atoms = vault_after
        .cash_atoms
        .checked_sub(complete_set_atoms)
        .ok_or(Error::BaseClosureMismatch)?;
    user_after.cash_atoms = user_after
        .cash_atoms
        .checked_add(payout_cash_atoms)
        .ok_or(Error::Arithmetic)?;
    index = 0;
    while index < width {
        vault_after.internal[index] = vault_after.internal[index]
            .checked_sub(residual[index])
            .ok_or(Error::BaseClosureMismatch)?;
        index += 1;
    }
    advance_pair(&mut user_after, &mut vault_after)?;

    let hoard_after = HoardV2 {
        cash_liability_atoms: liabilities
            .hoard
            .cash_liability_atoms
            .checked_add(residual_payout)
            .ok_or(Error::Arithmetic)?,
        locked_claim_principal_atoms: liabilities
            .hoard
            .locked_claim_principal_atoms
            .checked_sub(residual_payout)
            .ok_or(Error::BaseClosureMismatch)?,
        ..liabilities.hoard
    };
    let mut aggregate_internal_supply = liabilities.claim_ledger.aggregate_internal_supply;
    index = 0;
    while index < width {
        aggregate_internal_supply[index] = aggregate_internal_supply[index]
            .checked_sub(residual[index])
            .ok_or(Error::BaseClosureMismatch)?;
        index += 1;
    }
    let claim_ledger_after = ClaimLedgerV3 {
        aggregate_internal_supply,
        ..liabilities.claim_ledger
    };
    validate_liability_successors(hoard_after, claim_ledger_after)?;
    let mint_supply_after = mint
        .supply
        .checked_sub(quantity)
        .ok_or(Error::Token2022Boundary)?;
    let holder_after = holder
        .amount
        .checked_sub(quantity)
        .ok_or(Error::Token2022Boundary)?;
    validate_wrapper_coverage(backing, mint_supply_after, vault_after)?;
    let resolution_semantic_id = resolution
        .semantic_id(backend)
        .map_err(|_| Error::BaseClosureMismatch)?
        .bytes();
    finish_quantity_plan(
        descriptor,
        accounts,
        StructuredClaimActionV1::RedeemTerminal,
        liabilities,
        user,
        user_after,
        vault,
        vault_after,
        hoard_after,
        claim_ledger_after,
        mint.supply,
        mint_supply_after,
        holder.amount,
        holder_after,
        quantity,
        complete_set_atoms,
        payout_cash_atoms,
        [0; MAX_OUTCOMES],
        resolution_semantic_id,
        [0; 32],
        backend,
    )
}

#[allow(clippy::too_many_arguments)]
fn preflight_quantity(
    descriptor: &BoundDescriptorV1,
    collateral: BoundCollateralProfileV2,
    accounts: CurrentStructuredQuantityAccountsV1,
    liabilities: CurrentStructuredLiabilitiesV1,
    mint: WrapperMintProjectionV1,
    holder: WrapperTokenProjectionV1,
    user: PositionProjectionV1,
    vault: PositionProjectionV1,
    request: WrapperQuantityPayloadV1,
) -> Result<BackingPlan> {
    let quantity = request.quantity;
    if quantity == 0 {
        return Err(Error::Runtime(
            crate::runtime_contract::Error::ZeroQuantity,
        ));
    }
    validate_quantity_accounts(descriptor, accounts)?;
    validate_descriptor_and_liabilities(descriptor, collateral, liabilities)?;
    validate_mint(descriptor, mint, accounts.mint)?;
    if !holder.initialized
        || holder.address != accounts.holder
        || holder.mint != accounts.mint
        || holder.owner != user.owner
        || holder.owner != accounts.actor
        || is_zero(&holder.owner)
    {
        return Err(Error::Token2022Boundary);
    }
    validate_positions(descriptor, user, vault)?;
    if user.owner == descriptor.addresses().vault_owner
        || request.wrapper_product_id != accounts.wrapper_product_id
        || request.user_generation != user.generation
        || request.user_replay_sequence != user.replay_sequence
        || request.vault_generation != vault.generation
        || request.vault_replay_sequence != vault.replay_sequence
    {
        return Err(Error::CustodyAuthorityMismatch);
    }
    let backing = descriptor
        .identity()
        .claim
        .vector
        .backing_plan()
        .map_err(|_| Error::ProductBoundary)?;
    validate_wrapper_coverage(backing, mint.supply, vault)?;
    Ok(backing)
}

fn preflight_vault(
    descriptor: &BoundDescriptorV1,
    collateral: BoundCollateralProfileV2,
    accounts: CurrentStructuredVaultAccountsV1,
    liabilities: CurrentStructuredLiabilitiesV1,
    mint: WrapperMintProjectionV1,
    vault: PositionProjectionV1,
) -> Result<BackingPlan> {
    let ids = [
        accounts.descriptor,
        accounts.wrapper_product_id,
        accounts.vault_position,
        accounts.vault_replay,
        accounts.mint,
    ];
    require_distinct_live(&ids)?;
    if accounts.descriptor != descriptor.addresses().descriptor
        || accounts.wrapper_product_id != descriptor.wrapper_product_id()
        || accounts.mint != descriptor.addresses().mint
    {
        return Err(Error::CustodyAuthorityMismatch);
    }
    validate_descriptor_and_liabilities(descriptor, collateral, liabilities)?;
    validate_mint(descriptor, mint, accounts.mint)?;
    validate_position(descriptor, vault)?;
    if vault.owner != descriptor.addresses().vault_owner || vault.reserved_cash_atoms != 0 {
        return Err(Error::CustodyAuthorityMismatch);
    }
    let backing = descriptor
        .identity()
        .claim
        .vector
        .backing_plan()
        .map_err(|_| Error::ProductBoundary)?;
    validate_wrapper_coverage(backing, mint.supply, vault)?;
    Ok(backing)
}

fn validate_descriptor_and_liabilities(
    descriptor: &BoundDescriptorV1,
    collateral: BoundCollateralProfileV2,
    liabilities: CurrentStructuredLiabilitiesV1,
) -> Result<()> {
    liabilities
        .hoard
        .validate()
        .map_err(|_| Error::BaseClosureMismatch)?;
    liabilities
        .claim_ledger
        .validate()
        .map_err(|_| Error::BaseClosureMismatch)?;
    let market = collateral.market();
    let release_id = collateral
        .release()
        .id()
        .map_err(|_| Error::BaseClosureMismatch)?;
    if descriptor.descriptor().state != DescriptorStateV1::Active
        || descriptor.identity().claim.basis.market != liabilities.hoard.market_instance_id.bytes()
        || descriptor.identity().claim.basis.terms
            != liabilities.claim_ledger.native_claim_basis_id.bytes()
        || liabilities.hoard.market_instance_id != liabilities.claim_ledger.market_instance_id
        || liabilities.hoard.realm_id != liabilities.claim_ledger.realm_id
        || liabilities.hoard.lifecycle != liabilities.claim_ledger.lifecycle
        || liabilities.hoard.outcome_count != liabilities.claim_ledger.outcome_count
        || liabilities.hoard.outcome_count != descriptor.identity().claim.basis.outcome_count
        || market.market != liabilities.hoard.market_instance_id
        || market.realm != liabilities.hoard.realm_id
        || market.profile != liabilities.hoard.profile_id
        || market.collateral_cap_atoms != liabilities.hoard.collateral_cap_atoms
        || market.hoard_authority != liabilities.hoard.authority
        || market.hoard_token_account != liabilities.hoard.token_account
        || collateral.policy_id() != liabilities.hoard.collateral_policy_id
        || release_id != liabilities.hoard.collateral_release_id
        || liabilities.hoard.lifecycle == MarketLiabilityLifecycleV1::Retiring
    {
        return Err(Error::BaseClosureMismatch);
    }
    Ok(())
}

fn validate_quantity_accounts(
    descriptor: &BoundDescriptorV1,
    accounts: CurrentStructuredQuantityAccountsV1,
) -> Result<()> {
    let ids = [
        accounts.descriptor,
        accounts.wrapper_product_id,
        accounts.user_position,
        accounts.user_replay,
        accounts.vault_position,
        accounts.vault_replay,
        accounts.mint,
        accounts.holder,
        accounts.actor,
    ];
    require_distinct_live(&ids)?;
    if accounts.descriptor != descriptor.addresses().descriptor
        || accounts.wrapper_product_id != descriptor.wrapper_product_id()
        || accounts.mint != descriptor.addresses().mint
    {
        return Err(Error::CustodyAuthorityMismatch);
    }
    Ok(())
}

fn validate_mint(
    descriptor: &BoundDescriptorV1,
    mint: WrapperMintProjectionV1,
    account: Key,
) -> Result<()> {
    let addresses = descriptor.addresses();
    if !mint.initialized
        || mint.address != account
        || mint.address != addresses.mint
        || mint.mint_authority != addresses.mint_authority
        || mint.decimals != 0
        || mint.freeze_authority != [0; 32]
        || mint.extension_mask != 0
    {
        return Err(Error::Token2022Boundary);
    }
    Ok(())
}

fn validate_positions(
    descriptor: &BoundDescriptorV1,
    user: PositionProjectionV1,
    vault: PositionProjectionV1,
) -> Result<()> {
    validate_position(descriptor, user)?;
    validate_position(descriptor, vault)?;
    if user.market != vault.market
        || user.owner == vault.owner
        || vault.owner != descriptor.addresses().vault_owner
        || vault.reserved_cash_atoms != 0
    {
        return Err(Error::CustodyAuthorityMismatch);
    }
    Ok(())
}

fn validate_position(
    descriptor: &BoundDescriptorV1,
    position: PositionProjectionV1,
) -> Result<()> {
    let width = usize::from(descriptor.identity().claim.basis.outcome_count);
    if !(2..=MAX_OUTCOMES).contains(&width)
        || position.closed
        || is_zero(&position.market)
        || is_zero(&position.owner)
        || position.market != descriptor.identity().claim.basis.market
        || position.reserved_cash_atoms > position.cash_atoms
    {
        return Err(Error::BaseClosureMismatch);
    }
    let mut index = width;
    while index < MAX_OUTCOMES {
        if position.internal[index] != 0 {
            return Err(Error::BaseClosureMismatch);
        }
        index += 1;
    }
    Ok(())
}

fn validate_liability_successors(hoard: HoardV2, claim_ledger: ClaimLedgerV3) -> Result<()> {
    hoard
        .validate()
        .map_err(|_| Error::BaseClosureMismatch)?;
    claim_ledger
        .validate()
        .map_err(|_| Error::BaseClosureMismatch)?;
    if hoard.market_instance_id != claim_ledger.market_instance_id
        || hoard.realm_id != claim_ledger.realm_id
        || hoard.lifecycle != claim_ledger.lifecycle
        || hoard.outcome_count != claim_ledger.outcome_count
    {
        return Err(Error::BaseClosureMismatch);
    }
    Ok(())
}

fn validate_wrapper_coverage(
    backing: BackingPlan,
    wrapper_supply: u64,
    vault: PositionProjectionV1,
) -> Result<()> {
    let required_cash = wrapper_supply
        .checked_mul(backing.cash_per_wrapper)
        .ok_or(Error::Arithmetic)?;
    if vault.reserved_cash_atoms != 0 || vault.cash_atoms < required_cash {
        return Err(Error::BaseClosureMismatch);
    }
    let width = usize::from(backing.outcome_count);
    let mut index = 0usize;
    while index < width {
        let required = wrapper_supply
            .checked_mul(backing.residual_eggs_per_wrapper[index])
            .ok_or(Error::Arithmetic)?;
        if vault.internal[index] < required {
            return Err(Error::BaseClosureMismatch);
        }
        index += 1;
    }
    Ok(())
}

fn scaled_vector(
    coefficients: [u64; MAX_OUTCOMES],
    quantity: u64,
    width: usize,
) -> Result<[u64; MAX_OUTCOMES]> {
    let mut output = [0_u64; MAX_OUTCOMES];
    let mut index = 0usize;
    while index < width {
        output[index] = coefficients[index]
            .checked_mul(quantity)
            .ok_or(Error::Arithmetic)?;
        index += 1;
    }
    Ok(output)
}

fn advance_pair(
    user: &mut PositionProjectionV1,
    vault: &mut PositionProjectionV1,
) -> Result<()> {
    user.replay_sequence = user
        .replay_sequence
        .checked_add(1)
        .ok_or(Error::Arithmetic)?;
    vault.replay_sequence = vault
        .replay_sequence
        .checked_add(1)
        .ok_or(Error::Arithmetic)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn finish_quantity_plan<B: PositionV3Sha256Backend>(
    descriptor: &BoundDescriptorV1,
    accounts: CurrentStructuredQuantityAccountsV1,
    action: StructuredClaimActionV1,
    liabilities: CurrentStructuredLiabilitiesV1,
    user_before: PositionProjectionV1,
    user_after: PositionProjectionV1,
    vault_before: PositionProjectionV1,
    vault_after: PositionProjectionV1,
    hoard_after: HoardV2,
    claim_ledger_after: ClaimLedgerV3,
    mint_supply_before: u64,
    mint_supply_after: u64,
    holder_before: u64,
    holder_after: u64,
    wrapper_quantity: u64,
    complete_set_atoms: u64,
    payout_cash_atoms: u64,
    donated_internal: [u64; MAX_OUTCOMES],
    resolution_semantic_id: Key,
    liability_receipt_id: Key,
    backend: &B,
) -> Result<CurrentStructuredTransitionPlanV1> {
    finish_plan(
        descriptor,
        accounts,
        action,
        liabilities,
        Some(user_before),
        Some(user_after),
        vault_before,
        vault_after,
        hoard_after,
        claim_ledger_after,
        mint_supply_before,
        mint_supply_after,
        holder_before,
        holder_after,
        wrapper_quantity,
        complete_set_atoms,
        payout_cash_atoms,
        0,
        donated_internal,
        resolution_semantic_id,
        liability_receipt_id,
        backend,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_plan<B: PositionV3Sha256Backend>(
    descriptor: &BoundDescriptorV1,
    accounts: CurrentStructuredQuantityAccountsV1,
    action: StructuredClaimActionV1,
    liabilities: CurrentStructuredLiabilitiesV1,
    user_before: Option<PositionProjectionV1>,
    user_after: Option<PositionProjectionV1>,
    vault_before: PositionProjectionV1,
    vault_after: PositionProjectionV1,
    hoard_after: HoardV2,
    claim_ledger_after: ClaimLedgerV3,
    mint_supply_before: u64,
    mint_supply_after: u64,
    holder_before: u64,
    holder_after: u64,
    wrapper_quantity: u64,
    complete_set_atoms: u64,
    payout_cash_atoms: u64,
    donated_cash_atoms: u64,
    donated_internal: [u64; MAX_OUTCOMES],
    resolution_semantic_id: Key,
    liability_receipt_id: Key,
    backend: &B,
) -> Result<CurrentStructuredTransitionPlanV1> {
    validate_liability_successors(hoard_after, claim_ledger_after)?;
    let hoard_before_id = liabilities
        .hoard
        .semantic_id(backend)
        .map_err(|_| Error::BaseClosureMismatch)?
        .bytes();
    let hoard_after_id = hoard_after
        .semantic_id(backend)
        .map_err(|_| Error::BaseClosureMismatch)?
        .bytes();
    let claim_ledger_before_id = liabilities
        .claim_ledger
        .semantic_id(backend)
        .map_err(|_| Error::BaseClosureMismatch)?
        .bytes();
    let claim_ledger_after_id = claim_ledger_after
        .semantic_id(backend)
        .map_err(|_| Error::BaseClosureMismatch)?
        .bytes();
    let zero_position = PositionProjectionV1 {
        market: [0; 32],
        owner: [0; 32],
        generation: 0,
        replay_sequence: 0,
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        internal: [0; MAX_OUTCOMES],
        closed: false,
    };
    let user_before_id = projection_id(user_before.unwrap_or(zero_position), backend)?;
    let user_after_id = projection_id(user_after.unwrap_or(zero_position), backend)?;
    let vault_before_id = projection_id(vault_before, backend)?;
    let vault_after_id = projection_id(vault_after, backend)?;
    let transition_id = transition_id(
        action,
        accounts,
        wrapper_quantity,
        mint_supply_before,
        mint_supply_after,
        holder_before,
        holder_after,
        complete_set_atoms,
        payout_cash_atoms,
        donated_cash_atoms,
        user_before_id,
        user_after_id,
        vault_before_id,
        vault_after_id,
        hoard_before_id,
        hoard_after_id,
        claim_ledger_before_id,
        claim_ledger_after_id,
        resolution_semantic_id,
        liability_receipt_id,
        donated_internal,
        backend,
    )?;
    Ok(CurrentStructuredTransitionPlanV1 {
        action,
        user_after,
        vault_after,
        hoard_after,
        claim_ledger_after,
        mint_supply_before,
        mint_supply_after,
        holder_before,
        holder_after,
        wrapper_quantity,
        complete_set_atoms,
        payout_cash_atoms,
        donated_cash_atoms,
        donated_internal,
        hoard_before_id,
        hoard_after_id,
        claim_ledger_before_id,
        claim_ledger_after_id,
        resolution_semantic_id,
        liability_receipt_id,
        transition_id,
    })
}

fn projection_id<B: PositionV3Sha256Backend>(
    projection: PositionProjectionV1,
    backend: &B,
) -> Result<Key> {
    let mut body = [0_u8; POSITION_PROJECTION_PREIMAGE_BYTES];
    let mut cursor = 0usize;
    put(&mut body, &mut cursor, &projection.market)?;
    put(&mut body, &mut cursor, &projection.owner)?;
    put(&mut body, &mut cursor, &projection.generation.to_le_bytes())?;
    put(
        &mut body,
        &mut cursor,
        &projection.replay_sequence.to_le_bytes(),
    )?;
    put(&mut body, &mut cursor, &projection.cash_atoms.to_le_bytes())?;
    put(
        &mut body,
        &mut cursor,
        &projection.reserved_cash_atoms.to_le_bytes(),
    )?;
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        put(
            &mut body,
            &mut cursor,
            &projection.internal[index].to_le_bytes(),
        )?;
        index += 1;
    }
    put(&mut body, &mut cursor, &[u8::from(projection.closed)])?;
    put(&mut body, &mut cursor, &[0; 7])?;
    if cursor != POSITION_PROJECTION_PREIMAGE_BYTES {
        return Err(Error::Arithmetic);
    }
    let id = backend.sha256(CURRENT_STRUCTURED_POSITION_PROJECTION_DOMAIN_V1, &body);
    if is_zero(&id) {
        return Err(Error::DigestMismatch);
    }
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
fn transition_id<B: PositionV3Sha256Backend>(
    action: StructuredClaimActionV1,
    accounts: CurrentStructuredQuantityAccountsV1,
    quantity: u64,
    mint_before: u64,
    mint_after: u64,
    holder_before: u64,
    holder_after: u64,
    complete_set_atoms: u64,
    payout_cash_atoms: u64,
    donated_cash_atoms: u64,
    user_before_id: Key,
    user_after_id: Key,
    vault_before_id: Key,
    vault_after_id: Key,
    hoard_before_id: Key,
    hoard_after_id: Key,
    ledger_before_id: Key,
    ledger_after_id: Key,
    resolution_id: Key,
    liability_receipt_id: Key,
    donated_internal: [u64; MAX_OUTCOMES],
    backend: &B,
) -> Result<Key> {
    let mut body = [0_u8; TRANSITION_PREIMAGE_BYTES];
    let mut cursor = 0usize;
    put(&mut body, &mut cursor, &[action.tag()])?;
    put(&mut body, &mut cursor, &[0; 7])?;
    for id in [
        accounts.descriptor,
        accounts.wrapper_product_id,
        accounts.user_position,
        accounts.user_replay,
        accounts.vault_position,
        accounts.vault_replay,
        accounts.mint,
        accounts.holder,
        accounts.actor,
        user_before_id,
        user_after_id,
        vault_before_id,
        vault_after_id,
        hoard_before_id,
        hoard_after_id,
        ledger_before_id,
        ledger_after_id,
        resolution_id,
        liability_receipt_id,
    ] {
        put(&mut body, &mut cursor, &id)?;
    }
    for value in [
        quantity,
        mint_before,
        mint_after,
        holder_before,
        holder_after,
        complete_set_atoms,
        payout_cash_atoms,
        donated_cash_atoms,
    ] {
        put(&mut body, &mut cursor, &value.to_le_bytes())?;
    }
    for value in donated_internal {
        put(&mut body, &mut cursor, &value.to_le_bytes())?;
    }
    if cursor != TRANSITION_PREIMAGE_BYTES {
        return Err(Error::Arithmetic);
    }
    let id = backend.sha256(CURRENT_STRUCTURED_TRANSITION_DOMAIN_V1, &body);
    if is_zero(&id) {
        return Err(Error::DigestMismatch);
    }
    Ok(id)
}

fn put<const N: usize>(output: &mut [u8; N], cursor: &mut usize, input: &[u8]) -> Result<()> {
    let end = cursor.checked_add(input.len()).ok_or(Error::Arithmetic)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::Arithmetic)?
        .copy_from_slice(input);
    *cursor = end;
    Ok(())
}

fn require_distinct_live<const N: usize>(ids: &[Key; N]) -> Result<()> {
    let mut left = 0usize;
    while left < N {
        if is_zero(&ids[left]) {
            return Err(Error::CustodyAuthorityMismatch);
        }
        let mut right = left + 1;
        while right < N {
            if ids[left] == ids[right] {
                return Err(Error::CustodyAuthorityMismatch);
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_collateral_adapter_v2::{
        FractionalBindingStateV1, Id,
    };
    use clutch_retirement::{DeletableRentOwnerV1, Identity32V1};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Hash;

    impl PositionV3Sha256Backend for Hash {
        fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
            let mut output = [0_u8; 32];
            let mut index = 0usize;
            for byte in domain.iter().chain(body.iter()) {
                output[index % 32] = output[index % 32].wrapping_add(*byte).wrapping_add(1);
                index += 1;
            }
            output[0] |= 1;
            output
        }
    }

    fn id(value: u8) -> Id {
        Id::from_bytes([value; 32])
    }

    fn rent(value: u8) -> DeletableRentOwnerV1 {
        DeletableRentOwnerV1::from_persisted(
            Identity32V1::new([value; 32]).expect("identity"),
            1,
            0,
        )
        .expect("rent")
    }

    fn liabilities(lifecycle: MarketLiabilityLifecycleV1) -> CurrentStructuredLiabilitiesV1 {
        CurrentStructuredLiabilitiesV1 {
            hoard: HoardV2 {
                market_instance_id: id(1),
                realm_id: id(2),
                profile_id: id(3),
                collateral_policy_id: id(4),
                collateral_release_id: id(5),
                authority: id(6),
                token_account: id(7),
                collateral_cap_atoms: 1_000,
                cash_liability_atoms: 100,
                locked_claim_principal_atoms: 100,
                lifecycle,
                outcome_count: 2,
                stored_bump: 1,
                rent: rent(8),
            },
            claim_ledger: ClaimLedgerV3 {
                market_instance_id: id(1),
                realm_id: id(2),
                native_claim_basis_id: id(9),
                fractional_policy_id: Id::ZERO,
                fractional_ledger_account: Id::ZERO,
                resolution_account: if lifecycle == MarketLiabilityLifecycleV1::Open {
                    Id::ZERO
                } else {
                    id(10)
                },
                aggregate_internal_supply: {
                    let mut value = [0; MAX_OUTCOMES];
                    value[0] = 100;
                    value[1] = 100;
                    value
                },
                aggregate_materialized_supply: [0; MAX_OUTCOMES],
                next_fractional_sequence: 0,
                last_fractional_transition_id: Id::ZERO,
                fractional_binding: FractionalBindingStateV1::OpenUnlatched,
                lifecycle,
                outcome_count: 2,
                stored_bump: 1,
                rent: rent(11),
            },
        }
    }

    #[test]
    fn compaction_cannot_create_a_beneficiary_and_preserves_locked_principal() {
        let before = liabilities(MarketLiabilityLifecycleV1::Open);
        let mut after = before;
        after.hoard.cash_liability_atoms = 93;
        after.claim_ledger.aggregate_internal_supply[0] = 97;
        after.claim_ledger.aggregate_internal_supply[1] = 95;
        assert_eq!(after.hoard.locked_claim_principal_atoms, before.hoard.locked_claim_principal_atoms);
        assert_eq!(after.hoard.token_account, before.hoard.token_account);
        assert!(after.hoard.validate().is_ok());
        assert!(after.claim_ledger.validate().is_ok());
    }

    #[test]
    fn resolved_split_is_refused_by_current_complete_set_owner() {
        let value = liabilities(MarketLiabilityLifecycleV1::Resolved);
        assert!(prepare_complete_set_reclassification_v3(
            value.hoard,
            value.claim_ledger,
            CompleteSetReclassificationKindV3::Split,
            1,
            &Hash,
        )
        .is_err());
    }

    #[test]
    fn substituted_realm_breaks_the_current_liability_join() {
        let mut value = liabilities(MarketLiabilityLifecycleV1::Open);
        value.claim_ledger.realm_id = id(44);
        assert!(validate_liability_successors(value.hoard, value.claim_ledger).is_err());
    }

    #[test]
    fn transition_receipt_commits_every_donated_outcome_atom() {
        let accounts = CurrentStructuredQuantityAccountsV1 {
            descriptor: [1; 32],
            wrapper_product_id: [2; 32],
            user_position: [3; 32],
            user_replay: [4; 32],
            vault_position: [5; 32],
            vault_replay: [6; 32],
            mint: [7; 32],
            holder: [8; 32],
            actor: [9; 32],
        };
        let mut donation = [0_u64; MAX_OUTCOMES];
        let first = transition_id(
            StructuredClaimActionV1::CompactDonation,
            accounts,
            0,
            10,
            10,
            0,
            0,
            0,
            3,
            4,
            [10; 32],
            [11; 32],
            [12; 32],
            [13; 32],
            [14; 32],
            [15; 32],
            [16; 32],
            [17; 32],
            [0; 32],
            [0; 32],
            donation,
            &Hash,
        )
        .expect("first receipt");
        donation[MAX_OUTCOMES - 1] = 1;
        let second = transition_id(
            StructuredClaimActionV1::CompactDonation,
            accounts,
            0,
            10,
            10,
            0,
            0,
            0,
            3,
            4,
            [10; 32],
            [11; 32],
            [12; 32],
            [13; 32],
            [14; 32],
            [15; 32],
            [16; 32],
            [17; 32],
            [0; 32],
            [0; 32],
            donation,
            &Hash,
        )
        .expect("second receipt");
        assert_ne!(first, second);
    }
}
