//! Hostile Solana account metadata and canonical base/Token projections.

use clutch_kernel::{BasisMode, MarketState, Phase};
use clutch_solana_layout::{
    HoardAccount, MarketAccount, PositionAccount, SupplyLedgerAccount, TermsAccount,
};
use clutch_solana_reference::ReplayAccount;
use clutch_structured_claim::MarketLedger;

use crate::runtime_contract::{
    DescriptorBasisV1, PositionProjectionV1, StructuredClaimActionV1, StructuredClaimDescriptorV1,
    WrapperMintProjectionV1, WrapperTokenProjectionV1,
};
use crate::{is_zero, BoundDescriptorV1, Error, Key, Result};

/// Maximum accounts accepted by any structured-claim route contract.
pub const MAX_ROUTE_ACCOUNTS: usize = 21;

/// One semantic role in the strict structured-claim account frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AccountRoleV1 {
    /// Structured-claim wrapper executable.
    WrapperProgram,
    /// Base Dragon's Clutch executable.
    BaseProgram,
    /// Token-2022 executable.
    Token2022Program,
    /// System executable for predictable-PDA construction.
    SystemProgram,
    /// Construction payer.
    Payer,
    /// Current transaction actor.
    Actor,
    /// Canonical 0x88/1 descriptor PDA.
    Descriptor,
    /// Extension-free wrapper mint.
    Mint,
    /// Wrapper mint-authority PDA.
    MintAuthority,
    /// Base Position owned semantically by the vault-owner PDA.
    VaultPosition,
    /// Current-generation base Replay for the vault Position.
    VaultReplay,
    /// User source or beneficiary base Position.
    UserPosition,
    /// Current-generation base Replay for the user Position.
    UserReplay,
    /// User wrapper-token account.
    HolderToken,
    /// Canonical base Market.
    Market,
    /// Immutable base Terms.
    Terms,
    /// Base Hoard.
    Hoard,
    /// Base internal/external supply aggregate.
    SupplyLedger,
    /// Base Eggcrate/kernel aggregate.
    Kernel,
    /// Opaque base construction or retirement capability receipt.
    BaseCapability,
    /// Permanent base Position tombstone for descriptor retirement.
    VaultTombstone,
}

/// Borrowed Solana account metadata and bytes.
///
/// An SBF dispatcher can construct this directly from `AccountInfo` without
/// allocating. No method trusts `role`; the exact action frame validates role
/// order, keys, owners, aliases, and privileges before codecs are entered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawAccountV1<'a> {
    /// Semantic role assigned by the route's exact account list.
    pub role: AccountRoleV1,
    /// Runtime account address.
    pub key: Key,
    /// Runtime account owner.
    pub owner: Key,
    /// Lamports observed before execution.
    pub lamports: u64,
    /// Borrowed exact account data.
    pub data: &'a [u8],
    /// Transaction signer bit.
    pub signer: bool,
    /// Transaction writable bit.
    pub writable: bool,
    /// Runtime executable bit.
    pub executable: bool,
}

/// Exact access requirement for one account role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountAccessV1 {
    /// Expected semantic role.
    pub role: AccountRoleV1,
    /// Signer bit required when true.
    pub signer: bool,
    /// Writable bit required when true.
    pub writable: bool,
    /// Executable bit required when true.
    pub executable: bool,
}

/// Executable identities selected by the immutable descriptor deployment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountProgramsV1 {
    /// Wrapper executable.
    pub wrapper: Key,
    /// Base Dragon's Clutch executable.
    pub base: Key,
    /// Token-2022 executable.
    pub token_2022: Key,
    /// System executable.
    pub system: Key,
}

/// Borrowed strict action-specific account frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountFrameV1<'a> {
    /// Exact ordered account slice; no optional trailing accounts exist.
    pub accounts: &'a [RawAccountV1<'a>],
}

impl AccountFrameV1<'_> {
    /// Validate exact role order, pairwise address nonaliasing, and privileges.
    pub fn validate_for(
        &self,
        action: StructuredClaimActionV1,
        programs: AccountProgramsV1,
    ) -> Result<()> {
        let requirements = requirements(action);
        if self.accounts.len() != requirements.len()
            || self.accounts.len() > MAX_ROUTE_ACCOUNTS
            || [
                programs.wrapper,
                programs.base,
                programs.token_2022,
                programs.system,
            ]
            .iter()
            .any(is_zero)
        {
            return Err(Error::InvalidAccounts);
        }
        let mut index = 0_usize;
        while index < self.accounts.len() {
            let account = self.accounts[index];
            let required = requirements[index];
            if account.role != required.role
                || is_zero(&account.key)
                || (required.signer && !account.signer)
                || (required.writable && !account.writable)
                || account.executable != required.executable
            {
                return Err(Error::InvalidAccounts);
            }
            let mut later = index + 1;
            while later < self.accounts.len() {
                if account.key == self.accounts[later].key {
                    return Err(Error::InvalidAccounts);
                }
                later += 1;
            }
            index += 1;
        }
        self.validate_program_roles(programs)?;
        self.validate_owned_roles(action, programs)
    }

    /// Return the uniquely ordered account for one role.
    pub fn get(&self, role: AccountRoleV1) -> Result<&RawAccountV1<'_>> {
        let mut found = None;
        let mut index = 0_usize;
        while index < self.accounts.len() {
            if self.accounts[index].role == role {
                if found.is_some() {
                    return Err(Error::InvalidAccounts);
                }
                found = Some(&self.accounts[index]);
            }
            index += 1;
        }
        found.ok_or(Error::InvalidAccounts)
    }

    fn validate_program_roles(&self, programs: AccountProgramsV1) -> Result<()> {
        for (role, key) in [
            (AccountRoleV1::WrapperProgram, programs.wrapper),
            (AccountRoleV1::BaseProgram, programs.base),
            (AccountRoleV1::Token2022Program, programs.token_2022),
        ] {
            let account = self.get(role)?;
            if account.key != key || !account.executable || account.writable {
                return Err(Error::InvalidAccounts);
            }
        }
        if let Ok(system) = self.get(AccountRoleV1::SystemProgram) {
            if system.key != programs.system || !system.executable || system.writable {
                return Err(Error::InvalidAccounts);
            }
        }
        Ok(())
    }

    fn validate_owned_roles(
        &self,
        action: StructuredClaimActionV1,
        programs: AccountProgramsV1,
    ) -> Result<()> {
        if action == StructuredClaimActionV1::CreateDescriptor {
            for role in [
                AccountRoleV1::Descriptor,
                AccountRoleV1::Mint,
                AccountRoleV1::VaultPosition,
                AccountRoleV1::VaultReplay,
            ] {
                let target = self.get(role)?;
                if target.owner != programs.system || !target.data.is_empty() || target.executable {
                    return Err(Error::InvalidAccounts);
                }
            }
        } else {
            if self.get(AccountRoleV1::Descriptor)?.owner != programs.wrapper
                || self.get(AccountRoleV1::Mint)?.owner != programs.token_2022
            {
                return Err(Error::InvalidAccounts);
            }
        }
        for role in [
            AccountRoleV1::Market,
            AccountRoleV1::Terms,
            AccountRoleV1::Hoard,
            AccountRoleV1::SupplyLedger,
            AccountRoleV1::Kernel,
        ] {
            if let Ok(account) = self.get(role) {
                if account.owner != programs.base {
                    return Err(Error::InvalidAccounts);
                }
            }
        }
        if action != StructuredClaimActionV1::CreateDescriptor {
            for role in [AccountRoleV1::VaultPosition, AccountRoleV1::VaultReplay] {
                if self.get(role)?.owner != programs.base {
                    return Err(Error::InvalidAccounts);
                }
            }
        }
        for role in [AccountRoleV1::UserPosition, AccountRoleV1::UserReplay] {
            if let Ok(account) = self.get(role) {
                if account.owner != programs.base {
                    return Err(Error::InvalidAccounts);
                }
            }
        }
        if let Ok(holder) = self.get(AccountRoleV1::HolderToken) {
            if holder.owner != programs.token_2022 {
                return Err(Error::InvalidAccounts);
            }
        }
        Ok(())
    }
}

/// Fully checked base Market projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedBaseMarketV1 {
    market: MarketAccount,
    terms: TermsAccount,
    hoard: HoardAccount,
    supply: SupplyLedgerAccount,
    kernel: MarketState,
    basis: DescriptorBasisV1,
}

impl AuthenticatedBaseMarketV1 {
    /// Authenticated basis used to reconstruct a descriptor identity.
    pub const fn descriptor_basis(&self) -> DescriptorBasisV1 {
        self.basis
    }

    /// Authoritative Market account.
    pub const fn market_account(&self) -> &MarketAccount {
        &self.market
    }

    /// Authoritative aggregate supply account.
    pub const fn supply_account(&self) -> &SupplyLedgerAccount {
        &self.supply
    }

    /// Reconstruct the canonical runtime Market ledger for a bound descriptor.
    pub fn ledger(&self, descriptor: &BoundDescriptorV1) -> Result<MarketLedger> {
        if descriptor.identity().claim.basis.market != self.market.market.bytes()
            || descriptor.identity().claim.basis.terms != self.terms.terms.bytes()
            || descriptor.identity().claim.basis.basis_degree != self.basis.basis_degree
            || descriptor.identity().claim.basis.denominator != self.basis.denominator
            || descriptor.identity().claim.basis.outcome_count != self.basis.outcome_count
        {
            return Err(Error::BaseClosureMismatch);
        }
        let ledger = MarketLedger {
            basis: descriptor.identity().claim.basis,
            base: self.kernel,
        };
        ledger.validate().map_err(|_| Error::BaseClosureMismatch)?;
        Ok(ledger)
    }
}

/// Decode and join hostile Market, Terms, Hoard, SupplyLedger, and kernel truth.
pub fn authenticate_base_market_v1(
    market_data: &[u8],
    terms_data: &[u8],
    hoard_data: &[u8],
    supply_data: &[u8],
    kernel: MarketState,
) -> Result<AuthenticatedBaseMarketV1> {
    let market = MarketAccount::decode(market_data).map_err(|_| Error::InvalidAccountData)?;
    let terms = TermsAccount::decode(terms_data).map_err(|_| Error::InvalidAccountData)?;
    let hoard = HoardAccount::decode(hoard_data).map_err(|_| Error::InvalidAccountData)?;
    let supply = SupplyLedgerAccount::decode(supply_data).map_err(|_| Error::InvalidAccountData)?;
    terms
        .binds_market(&market)
        .map_err(|_| Error::BaseClosureMismatch)?;
    hoard.validate().map_err(|_| Error::BaseClosureMismatch)?;
    supply
        .binds_market(&market)
        .map_err(|_| Error::BaseClosureMismatch)?;
    kernel
        .check_invariants()
        .map_err(|_| Error::BaseClosureMismatch)?;
    if market.lifecycle > 1
        || market.market != hoard.market
        || market.realm != hoard.realm
        || market.collateral_cap != terms.collateral_cap
        || hoard.collateral_atoms != kernel.collateral
        || market.outcome_count != kernel.outcomes
        || supply.outcome_count != kernel.outcomes
        || kernel.collateral > market.collateral_cap
        || kernel.phase
            != if market.lifecycle == 0 {
                Phase::Active
            } else {
                Phase::Resolved
            }
        || kernel.payouts.outcomes != terms.outcome_count
        || kernel.payouts.count != terms.payout_count
    {
        return Err(Error::BaseClosureMismatch);
    }
    if (terms.basis_degree == 0 && kernel.basis_mode != BasisMode::FinitePreset)
        || (terms.basis_degree != 0 && kernel.basis_mode != BasisMode::DerivedBasis)
    {
        return Err(Error::BaseClosureMismatch);
    }
    let mut payout = 0_usize;
    while payout < usize::from(terms.payout_count) {
        if kernel.payouts.vectors[payout].denominator != terms.payouts[payout].denominator
            || kernel.payouts.vectors[payout].weights != terms.payouts[payout].weights
        {
            return Err(Error::BaseClosureMismatch);
        }
        payout += 1;
    }
    let mut outcome = 0_usize;
    while outcome < crate::runtime_contract::MAX_OUTCOMES {
        if outcome < usize::from(market.outcome_count) {
            if supply.internal_supply[outcome]
                .checked_add(supply.external_supply[outcome])
                .ok_or(Error::Arithmetic)?
                != kernel.total_supply[outcome]
            {
                return Err(Error::BaseClosureMismatch);
            }
        } else if kernel.total_supply[outcome] != 0 {
            return Err(Error::BaseClosureMismatch);
        }
        outcome += 1;
    }
    let denominator = terms.payouts[0].denominator;
    Ok(AuthenticatedBaseMarketV1 {
        market,
        terms,
        hoard,
        supply,
        kernel,
        basis: DescriptorBasisV1 {
            market: market.market.bytes(),
            terms_digest: terms.terms.bytes(),
            basis_degree: terms.basis_degree,
            denominator,
            outcome_count: terms.outcome_count,
        },
    })
}

/// Decode the canonical 0x88/1 descriptor from a wrapper-owned account.
pub fn decode_owned_descriptor_v1(
    wrapper_program: Key,
    expected_address: Key,
    account: &RawAccountV1<'_>,
) -> Result<StructuredClaimDescriptorV1> {
    if account.role != AccountRoleV1::Descriptor
        || account.key != expected_address
        || account.owner != wrapper_program
        || account.executable
        || is_zero(&account.key)
    {
        return Err(Error::InvalidAccounts);
    }
    StructuredClaimDescriptorV1::decode(account.data).map_err(|_| Error::InvalidAccountData)
}

/// Base-owned Position/Replay PDA verifier.
pub trait BasePositionPdaVerifierV1 {
    /// Authenticate the canonical Position PDA from the base namespace.
    fn verify_position(
        &self,
        program: Key,
        address: Key,
        market: Key,
        owner: Key,
        generation: u64,
        stored_bump: u8,
    ) -> bool;

    /// Authenticate the canonical current-generation Replay PDA.
    fn verify_replay(
        &self,
        program: Key,
        address: Key,
        market: Key,
        owner: Key,
        generation: u64,
        stored_bump: u8,
    ) -> bool;
}

/// Authenticated base Position plus current-generation Replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedBasePositionV1 {
    position_address: Key,
    replay_address: Key,
    projection: PositionProjectionV1,
}

impl AuthenticatedBasePositionV1 {
    /// Canonical semantic projection consumed by the runtime contract.
    pub const fn projection(&self) -> PositionProjectionV1 {
        self.projection
    }

    /// Canonical Position account address.
    pub const fn position_address(&self) -> Key {
        self.position_address
    }

    /// Canonical current-generation Replay account address.
    pub const fn replay_address(&self) -> Key {
        self.replay_address
    }
}

/// Decode and authenticate one base Position/Replay pair.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_base_position_v1<P: BasePositionPdaVerifierV1>(
    base_program: Key,
    position_address: Key,
    position_data: &[u8],
    replay_address: Key,
    replay_data: &[u8],
    expected_market: Key,
    expected_owner: Key,
    expected_generation: u64,
    outcome_count: u8,
    supply: &SupplyLedgerAccount,
    verifier: &P,
) -> Result<AuthenticatedBasePositionV1> {
    let position = PositionAccount::decode(position_data).map_err(|_| Error::InvalidAccountData)?;
    let replay = ReplayAccount::decode(replay_data).map_err(|_| Error::InvalidAccountData)?;
    if position.market.bytes() != expected_market
        || position.owner.bytes() != expected_owner
        || position.generation != expected_generation
        || position.close_state != 0
        || replay.market.bytes() != expected_market
        || replay.owner.bytes() != expected_owner
        || replay.position_generation != expected_generation
        || replay.flags != 0
    {
        return Err(Error::BaseClosureMismatch);
    }
    supply
        .check_position_bound(&position)
        .map_err(|_| Error::BaseClosureMismatch)?;
    if !verifier.verify_position(
        base_program,
        position_address,
        expected_market,
        expected_owner,
        expected_generation,
        position.stored_bump,
    ) || !verifier.verify_replay(
        base_program,
        replay_address,
        expected_market,
        expected_owner,
        expected_generation,
        replay.stored_bump,
    ) {
        return Err(Error::PdaMismatch);
    }
    let projection = PositionProjectionV1 {
        market: expected_market,
        owner: expected_owner,
        generation: expected_generation,
        replay_sequence: replay.sequence,
        cash_atoms: position.cash_atoms,
        reserved_cash_atoms: position.reserved_cash_atoms,
        internal: position.internal,
        closed: false,
    };
    let width = usize::from(outcome_count);
    if !(2..=crate::runtime_contract::MAX_OUTCOMES).contains(&width) {
        return Err(Error::BaseClosureMismatch);
    }
    let mut padding = width;
    while padding < crate::runtime_contract::MAX_OUTCOMES {
        if projection.internal[padding] != 0 {
            return Err(Error::BaseClosureMismatch);
        }
        padding += 1;
    }
    Ok(AuthenticatedBasePositionV1 {
        position_address,
        replay_address,
        projection,
    })
}

/// Target-specific Token-2022 parser boundary.
///
/// The implementation must use the pinned Token-2022 byte layout. Mint decode
/// must reject every extension, nonzero decimals, freeze authority, wrong mint
/// authority, or uninitialized state. Token-account decode must reject frozen,
/// native, delegated, close-authority, wrong-mint, and unknown-extension state;
/// only `ImmutableOwner` may be admitted. These facts deliberately do not live
/// in a second persisted wrapper DTO.
pub trait Token2022DecoderV1 {
    /// Decode an extension-free wrapper mint.
    fn decode_mint(
        &self,
        address: Key,
        data: &[u8],
    ) -> core::result::Result<WrapperMintProjectionV1, ()>;

    /// Decode an ordinary wrapper holder account.
    fn decode_token(
        &self,
        address: Key,
        data: &[u8],
    ) -> core::result::Result<WrapperTokenProjectionV1, ()>;
}

/// Mint projection authenticated by a named Token-2022 parser boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedTokenMintV1(WrapperMintProjectionV1);

impl AuthenticatedTokenMintV1 {
    /// Canonical runtime-contract mint projection.
    pub const fn projection(&self) -> WrapperMintProjectionV1 {
        self.0
    }
}

/// Holder projection authenticated by a named Token-2022 parser boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedTokenV1(WrapperTokenProjectionV1);

impl AuthenticatedTokenV1 {
    /// Canonical runtime-contract holder projection.
    pub const fn projection(&self) -> WrapperTokenProjectionV1 {
        self.0
    }
}

/// Authenticate an extension-free wrapper mint without restating its codec.
pub fn authenticate_token_2022_mint_v1<D: Token2022DecoderV1>(
    token_program: Key,
    account: &RawAccountV1<'_>,
    decoder: &D,
) -> Result<AuthenticatedTokenMintV1> {
    if account.owner != token_program || account.executable || is_zero(&account.key) {
        return Err(Error::Token2022Boundary);
    }
    let projection = decoder
        .decode_mint(account.key, account.data)
        .map_err(|_| Error::Token2022Boundary)?;
    if projection.address != account.key {
        return Err(Error::Token2022Boundary);
    }
    Ok(AuthenticatedTokenMintV1(projection))
}

/// Authenticate an ordinary holder token account without restating its codec.
pub fn authenticate_token_2022_token_v1<D: Token2022DecoderV1>(
    token_program: Key,
    account: &RawAccountV1<'_>,
    decoder: &D,
) -> Result<AuthenticatedTokenV1> {
    if account.owner != token_program || account.executable || is_zero(&account.key) {
        return Err(Error::Token2022Boundary);
    }
    let projection = decoder
        .decode_token(account.key, account.data)
        .map_err(|_| Error::Token2022Boundary)?;
    if projection.address != account.key {
        return Err(Error::Token2022Boundary);
    }
    Ok(AuthenticatedTokenV1(projection))
}

const PROGRAM: AccountAccessV1 = AccountAccessV1 {
    role: AccountRoleV1::WrapperProgram,
    signer: false,
    writable: false,
    executable: true,
};

const CREATE_REQUIREMENTS: &[AccountAccessV1] = &[
    PROGRAM,
    AccountAccessV1 {
        role: AccountRoleV1::BaseProgram,
        ..PROGRAM
    },
    AccountAccessV1 {
        role: AccountRoleV1::Token2022Program,
        ..PROGRAM
    },
    AccountAccessV1 {
        role: AccountRoleV1::SystemProgram,
        ..PROGRAM
    },
    AccountAccessV1 {
        role: AccountRoleV1::Payer,
        signer: true,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Descriptor,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Mint,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::MintAuthority,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::VaultPosition,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::VaultReplay,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Market,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Terms,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::BaseCapability,
        signer: false,
        writable: false,
        executable: false,
    },
];

const QUANTITY_REQUIREMENTS: &[AccountAccessV1] = &[
    PROGRAM,
    AccountAccessV1 {
        role: AccountRoleV1::BaseProgram,
        ..PROGRAM
    },
    AccountAccessV1 {
        role: AccountRoleV1::Token2022Program,
        ..PROGRAM
    },
    AccountAccessV1 {
        role: AccountRoleV1::Actor,
        signer: true,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Descriptor,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Mint,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::MintAuthority,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::HolderToken,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::UserPosition,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::UserReplay,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::VaultPosition,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::VaultReplay,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Market,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Terms,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Hoard,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::SupplyLedger,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Kernel,
        signer: false,
        writable: true,
        executable: false,
    },
];

const CANONICAL_REQUIREMENTS: &[AccountAccessV1] = &[
    PROGRAM,
    AccountAccessV1 {
        role: AccountRoleV1::BaseProgram,
        ..PROGRAM
    },
    AccountAccessV1 {
        role: AccountRoleV1::Token2022Program,
        ..PROGRAM
    },
    AccountAccessV1 {
        role: AccountRoleV1::Actor,
        signer: true,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Descriptor,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Mint,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::MintAuthority,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::HolderToken,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::UserPosition,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::UserReplay,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::VaultPosition,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::VaultReplay,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Market,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Terms,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Hoard,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::SupplyLedger,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Kernel,
        signer: false,
        writable: false,
        executable: false,
    },
];

const COMPACT_REQUIREMENTS: &[AccountAccessV1] = &[
    PROGRAM,
    AccountAccessV1 {
        role: AccountRoleV1::BaseProgram,
        ..PROGRAM
    },
    AccountAccessV1 {
        role: AccountRoleV1::Token2022Program,
        ..PROGRAM
    },
    AccountAccessV1 {
        role: AccountRoleV1::Actor,
        signer: true,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Descriptor,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Mint,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::VaultPosition,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::VaultReplay,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Market,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Terms,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Hoard,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::SupplyLedger,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Kernel,
        signer: false,
        writable: true,
        executable: false,
    },
];

const RETIRE_REQUIREMENTS: &[AccountAccessV1] = &[
    PROGRAM,
    AccountAccessV1 {
        role: AccountRoleV1::BaseProgram,
        ..PROGRAM
    },
    AccountAccessV1 {
        role: AccountRoleV1::Token2022Program,
        ..PROGRAM
    },
    AccountAccessV1 {
        role: AccountRoleV1::Actor,
        signer: true,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Descriptor,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Mint,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::MintAuthority,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::VaultPosition,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::VaultReplay,
        signer: false,
        writable: true,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Market,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::Terms,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::BaseCapability,
        signer: false,
        writable: false,
        executable: false,
    },
    AccountAccessV1 {
        role: AccountRoleV1::VaultTombstone,
        signer: false,
        writable: true,
        executable: false,
    },
];

fn requirements(action: StructuredClaimActionV1) -> &'static [AccountAccessV1] {
    match action {
        StructuredClaimActionV1::CreateDescriptor => CREATE_REQUIREMENTS,
        StructuredClaimActionV1::CompactDonation => COMPACT_REQUIREMENTS,
        StructuredClaimActionV1::Retire => RETIRE_REQUIREMENTS,
        StructuredClaimActionV1::WrapCanonical | StructuredClaimActionV1::UnwrapCanonical => {
            CANONICAL_REQUIREMENTS
        }
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => QUANTITY_REQUIREMENTS,
    }
}
