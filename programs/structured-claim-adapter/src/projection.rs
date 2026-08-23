//! Authenticated base-account, Token-2022, and account-access projections.

use clutch_kernel::{BasisMode, MarketState, Phase};
use clutch_solana_layout::{
    HoardAccount, MarketAccount, PositionAccount, SupplyLedgerAccount, TermsAccount,
};

use crate::{is_zero, Action, Error, Key, Result, MAX_OUTCOMES};

/// The only admitted holder-account extension bit.
pub const TOKEN_ACCOUNT_EXTENSION_IMMUTABLE_OWNER: u64 = 1;

/// Authenticated extension-free Token-2022 mint state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintProjection {
    /// Mint account address.
    pub key: Key,
    /// Owning Token-2022 program.
    pub token_program: Key,
    /// Actual Token-2022 supply; the only wrapper-supply truth.
    pub supply: u64,
    /// Mint authority PDA.
    pub mint_authority: Key,
    /// Mint is initialized.
    pub initialized: bool,
    /// Mint decimals; V1 requires zero.
    pub decimals: u8,
    /// Whether a freeze authority is present.
    pub freeze_authority_present: bool,
    /// Bitset of initialized mint extensions; V1 requires zero.
    pub extension_mask: u64,
}

impl MintProjection {
    /// Validate the deliberately boring V1 wrapper-mint profile.
    pub fn validate(&self, mint: Key, token_program: Key, vault_owner: Key) -> Result<()> {
        if self.key != mint
            || self.token_program != token_program
            || self.mint_authority != vault_owner
            || !self.initialized
            || self.decimals != 0
            || self.freeze_authority_present
            || self.extension_mask != 0
            || is_zero(&self.key)
        {
            return Err(Error::InvalidTokenProjection);
        }
        Ok(())
    }
}

/// Authenticated Token-2022 holder-account state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenAccountProjection {
    /// Token account address.
    pub key: Key,
    /// Owning Token-2022 program.
    pub token_program: Key,
    /// Wrapper mint address.
    pub mint: Key,
    /// Token authority/beneficiary identity.
    pub authority: Key,
    /// Actual holder balance.
    pub amount: u64,
    /// Account is initialized.
    pub initialized: bool,
    /// Account is frozen.
    pub frozen: bool,
    /// Native-token marker is present.
    pub native: bool,
    /// Delegate is present.
    pub delegate_present: bool,
    /// Close authority is present.
    pub close_authority_present: bool,
    /// Initialized account-extension bits.
    pub extension_mask: u64,
}

impl TokenAccountProjection {
    /// Validate an ordinary holder account with at most `ImmutableOwner`.
    pub fn validate(&self, mint: Key, token_program: Key) -> Result<()> {
        if self.mint != mint
            || self.token_program != token_program
            || !self.initialized
            || self.frozen
            || self.native
            || self.delegate_present
            || self.close_authority_present
            || self.extension_mask & !TOKEN_ACCOUNT_EXTENSION_IMMUTABLE_OWNER != 0
            || is_zero(&self.key)
            || is_zero(&self.authority)
        {
            return Err(Error::InvalidTokenProjection);
        }
        Ok(())
    }
}

/// Authenticated base Market facts required by the structured-claim core.
///
/// `base` is reconstructed from the base kernel/Hoard/Resolution records. It
/// is an ephemeral semantic value, never another persisted market truth.
#[derive(Clone, Copy, Debug)]
pub struct AuthenticatedMarket<'a> {
    /// Fixed-layout Market account.
    pub market: &'a MarketAccount,
    /// Self-certifying immutable Terms account.
    pub terms: &'a TermsAccount,
    /// Fixed-layout Hoard account.
    pub hoard: &'a HoardAccount,
    /// Market-wide internal/external aggregate.
    pub supply: &'a SupplyLedgerAccount,
    /// Reconstructed Eggcrate state.
    pub base: &'a MarketState,
}

/// Check all cross-account market, Terms, Hoard, phase, payout, cap, and
/// SupplyLedger closure obligations.
pub fn check_market_closure(state: &AuthenticatedMarket<'_>) -> Result<()> {
    state
        .market
        .validate()
        .map_err(|_| Error::DescriptorBinding)?;
    state
        .terms
        .binds_market(state.market)
        .map_err(|_| Error::DescriptorBinding)?;
    state
        .hoard
        .validate()
        .map_err(|_| Error::DescriptorBinding)?;
    state
        .supply
        .binds_market(state.market)
        .map_err(|_| Error::SupplyClosureMismatch)?;
    state
        .base
        .check_invariants()
        .map_err(|_| Error::SupplyClosureMismatch)?;
    if state.market.lifecycle > 1
        || state.market.realm != state.hoard.realm
        || state.market.market != state.hoard.market
        || state.market.collateral_cap != state.terms.collateral_cap
        || state.hoard.collateral_atoms != state.base.collateral
        || state.market.outcome_count != state.base.outcomes
        || state.supply.outcome_count != state.base.outcomes
    {
        return Err(Error::SupplyClosureMismatch);
    }
    let phase = if state.market.lifecycle == 0 {
        Phase::Active
    } else {
        Phase::Resolved
    };
    if state.base.phase != phase || state.base.collateral > state.market.collateral_cap {
        return Err(Error::SupplyClosureMismatch);
    }
    check_payout_binding(state)?;
    let active = usize::from(state.market.outcome_count);
    let mut i = 0;
    while i < MAX_OUTCOMES {
        if i < active {
            let aggregate = state.supply.internal_supply[i]
                .checked_add(state.supply.external_supply[i])
                .ok_or(Error::Arithmetic)?;
            if aggregate != state.base.total_supply[i] {
                return Err(Error::SupplyClosureMismatch);
            }
        } else if state.base.total_supply[i] != 0 {
            return Err(Error::SupplyClosureMismatch);
        }
        i += 1;
    }
    Ok(())
}

fn check_payout_binding(state: &AuthenticatedMarket<'_>) -> Result<()> {
    if state.base.payouts.outcomes != state.terms.outcome_count
        || state.base.payouts.count != state.terms.payout_count
    {
        return Err(Error::DescriptorBinding);
    }
    if state.terms.basis_degree == 0 {
        if state.base.basis_mode != BasisMode::FinitePreset {
            return Err(Error::DescriptorBinding);
        }
    } else if state.base.basis_mode != BasisMode::DerivedBasis {
        return Err(Error::DescriptorBinding);
    }
    let mut payout = 0;
    while payout < usize::from(state.terms.payout_count) {
        let kernel = state.base.payouts.vectors[payout];
        let terms = state.terms.payouts[payout];
        if kernel.denominator != terms.denominator || kernel.weights != terms.weights {
            return Err(Error::DescriptorBinding);
        }
        payout += 1;
    }
    Ok(())
}

/// Semantic account role used by the nonalias/access gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AccountRole {
    /// Wrapper executable.
    WrapperProgram,
    /// Base Dragon's Clutch executable.
    BaseProgram,
    /// Token-2022 executable.
    TokenProgram,
    /// Wrapper descriptor.
    Descriptor,
    /// Wrapper mint.
    Mint,
    /// Wrapper vault Position.
    VaultPosition,
    /// Holder source or beneficiary Position.
    HolderPosition,
    /// Holder wrapper token account.
    HolderToken,
    /// Per-actor wrapper replay.
    WrapperReplay,
    /// Source/beneficiary base replay.
    SourceReplay,
    /// Vault base replay.
    VaultReplay,
    /// Base Market.
    Market,
    /// Immutable Terms.
    Terms,
    /// Base Hoard.
    Hoard,
    /// Market-wide SupplyLedger.
    SupplyLedger,
    /// Base kernel aggregate.
    Kernel,
    /// Transaction actor.
    Actor,
    /// Unused fixed-capacity slot.
    Unused,
}

/// Address and runtime privileges for one role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountAccess {
    /// Semantic role.
    pub role: AccountRole,
    /// Account address.
    pub key: Key,
    /// Runtime account owner.
    pub owner: Key,
    /// Transaction signer bit.
    pub signer: bool,
    /// Transaction writable bit.
    pub writable: bool,
    /// Runtime executable bit.
    pub executable: bool,
}

impl AccountAccess {
    /// Empty fixed-capacity sentinel.
    pub const EMPTY: Self = Self {
        role: AccountRole::Unused,
        key: [0; 32],
        owner: [0; 32],
        signer: false,
        writable: false,
        executable: false,
    };
}

/// Maximum accounts in one isolated wrapper route.
pub const MAX_ROUTE_ACCOUNTS: usize = 17;

/// Fixed-capacity role set with no address or role aliases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountSet {
    /// Active prefix length.
    pub count: u8,
    /// Active role prefix followed by [`AccountAccess::EMPTY`].
    pub accounts: [AccountAccess; MAX_ROUTE_ACCOUNTS],
}

impl AccountSet {
    /// Find one unique active role.
    pub fn get(&self, role: AccountRole) -> Result<&AccountAccess> {
        let mut i = 0;
        let mut found = None;
        while i < usize::from(self.count) {
            if self.accounts[i].role == role {
                if found.is_some() {
                    return Err(Error::InvalidAccountSet);
                }
                found = Some(&self.accounts[i]);
            }
            i += 1;
        }
        found.ok_or(Error::InvalidAccountSet)
    }

    /// Validate active prefix, exact unique roles, address nonaliasing, and
    /// action-specific role presence. Privilege/owner checks remain with the
    /// planner because writability varies with the staged CPI set.
    pub fn validate_for(&self, action: Action) -> Result<()> {
        let count = usize::from(self.count);
        if count == 0 || count > MAX_ROUTE_ACCOUNTS {
            return Err(Error::InvalidAccountSet);
        }
        let mut left = 0;
        while left < MAX_ROUTE_ACCOUNTS {
            let account = self.accounts[left];
            if left < count {
                if account.role == AccountRole::Unused || is_zero(&account.key) {
                    return Err(Error::InvalidAccountSet);
                }
                let mut right = left + 1;
                while right < count {
                    if account.role == self.accounts[right].role
                        || account.key == self.accounts[right].key
                    {
                        return Err(Error::InvalidAccountSet);
                    }
                    right += 1;
                }
            } else if account != AccountAccess::EMPTY {
                return Err(Error::NonCanonical);
            }
            left += 1;
        }
        let common = [
            AccountRole::WrapperProgram,
            AccountRole::BaseProgram,
            AccountRole::TokenProgram,
            AccountRole::Descriptor,
            AccountRole::Mint,
            AccountRole::VaultPosition,
            AccountRole::WrapperReplay,
            AccountRole::VaultReplay,
            AccountRole::Market,
            AccountRole::Terms,
            AccountRole::Hoard,
            AccountRole::SupplyLedger,
            AccountRole::Kernel,
            AccountRole::Actor,
        ];
        let mut i = 0;
        while i < common.len() {
            self.get(common[i])?;
            i += 1;
        }
        if !matches!(action, Action::CompactDonation | Action::Retire) {
            self.get(AccountRole::HolderPosition)?;
            self.get(AccountRole::HolderToken)?;
            self.get(AccountRole::SourceReplay)?;
        }
        Ok(())
    }
}

/// Validate a Position against one Market, owner, generation, open-state and
/// market-wide internal bound.
pub(crate) fn check_position(
    position: &PositionAccount,
    market: &MarketAccount,
    supply: &SupplyLedgerAccount,
    owner: Key,
    generation: u64,
) -> Result<()> {
    position.validate().map_err(|_| Error::InvalidPosition)?;
    supply
        .check_position_bound(position)
        .map_err(|_| Error::SupplyClosureMismatch)?;
    if position.market.bytes() != market.market.bytes()
        || position.owner.bytes() != owner
        || position.generation != generation
        || position.close_state != 0
    {
        return Err(Error::InvalidPosition);
    }
    Ok(())
}
