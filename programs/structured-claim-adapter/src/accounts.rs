//! Hostile Solana account metadata and canonical base/Token projections.

use clutch_general_v2_contract::{GeneralReplayExtensionV1, GENERAL_REPLAY_EXTENSION_SCHEMA_V1};
use clutch_retirement::{
    project_general_position_v3, project_structured_claim_position_v3,
    AdapterPositionMarketBindingV3, AdapterPositionPurposeBindingV3, Identity32V1,
    PositionAccountV3, PositionLifecycleV3, PositionPurposeV3, PositionV3PdaSeeds,
    ReplayV3Envelope, ReplayV3Lifecycle,
};
use clutch_retirement_adapter::{
    authenticate_position_v3_exact, authenticate_purpose_replay_v3_exact,
    AccountAccessV2 as RetirementAccountAccessV2, AccountViewV2 as RetirementAccountViewV2,
    CanonicalPdaV1,
};
use clutch_solana_layout::SupplyLedgerAccount;

use crate::runtime_contract::{
    PositionProjectionV1, StructuredClaimActionV1, StructuredClaimDescriptorV2,
    StructuredClaimReplayExtensionV1, WrapperMintProjectionV1, WrapperTokenProjectionV1,
    STRUCTURED_CLAIM_REPLAY_EXTENSION_SCHEMA_V1,
};
use crate::{is_zero, AdapterSha256V1, BoundDescriptorV1, Error, Key, Result};

/// Maximum accounts accepted by any structured-claim route contract.
pub const MAX_ROUTE_ACCOUNTS: usize = 32;

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
    /// Wrapper vault-owner PDA used only as a typed CPI signer.
    VaultAuthority,
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
    /// Immutable General V2 MarketBinding PDA used by action 35.
    MarketBinding,
    /// Immutable Realm selecting collateral semantics.
    Realm,
    /// Immutable Profile V2 selected by the Realm.
    Profile,
    /// Exact sealed CollateralPolicy V2 artifact.
    CollateralPolicy,
    /// Collateral token executable selected by the immutable Profile.
    CollateralTokenProgram,
    /// Stable General V2 MarketRuntime selected by MarketBinding.
    MarketRuntime,
    /// Source full-width Position V3 for action 35.
    SourcePositionV3,
    /// Source purpose-owned Replay V3 for action 35.
    SourceReplayV3,
    /// Destination full-width Position V3 for action 35.
    DestinationPositionV3,
    /// Destination purpose-owned Replay V3 for action 35.
    DestinationReplayV3,
    /// Wrapper ProgramData account selected by the descriptor.
    WrapperProgramData,
    /// Base ProgramData account selected by the descriptor.
    BaseProgramData,
    /// Token-2022 ProgramData account selected by the descriptor.
    Token2022ProgramData,
    /// Exact NativeClaimBasisV1 Product artifact.
    NativeClaimBasisArtifact,
    /// Exact MarketInstanceV2 preimage artifact.
    MarketInstanceArtifact,
    /// Full-width Hoard V2 aggregate owner.
    HoardV2,
    /// Full-width ClaimLedger V3 aggregate owner.
    ClaimLedgerV3,
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
            || [programs.wrapper, programs.base, programs.token_2022]
                .iter()
                .any(is_zero)
            || (action == StructuredClaimActionV1::CreateDescriptor && is_zero(&programs.system))
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
        if let Ok(capability) = self.get(AccountRoleV1::BaseCapability) {
            if capability.owner != programs.base || capability.executable {
                return Err(Error::InvalidAccounts);
            }
        }
        Ok(())
    }
}

/// Decode the canonical live descriptor-v2 body from a wrapper-owned account.
pub fn decode_owned_descriptor_v1(
    wrapper_program: Key,
    expected_address: Key,
    account: &RawAccountV1<'_>,
) -> Result<StructuredClaimDescriptorV2> {
    if account.role != AccountRoleV1::Descriptor
        || account.key != expected_address
        || account.owner != wrapper_program
        || account.executable
        || is_zero(&account.key)
    {
        return Err(Error::InvalidAccounts);
    }
    StructuredClaimDescriptorV2::decode(account.data).map_err(|_| Error::InvalidAccountData)
}

/// Base-owned Position/Replay PDA verifier.
pub trait BasePositionPdaVerifierV1 {
    /// Verify the canonical full-width Position V3 PDA seed tuple.
    fn verify_position_v3(&self, program: Key, address: Key, seeds: PositionV3PdaSeeds) -> bool;

    /// Verify the exact stable purpose-owned Replay V3 PDA.
    fn verify_replay_v3(
        &self,
        program: Key,
        address: Key,
        position_account: Key,
        purpose: PositionPurposeV3,
        purpose_binding_id: Key,
        stored_bump: u8,
    ) -> bool;
}

/// Authenticated base Position plus current-generation Replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedBasePositionV3 {
    position_address: Key,
    replay_address: Key,
    projection: PositionProjectionV1,
}

impl AuthenticatedBasePositionV3 {
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

/// Authenticate a General-purpose Position V3 and its exact `GEN1` Replay V3.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_general_base_position_v3_v1<P: BasePositionPdaVerifierV1>(
    base_program: Key,
    position_account: &RawAccountV1<'_>,
    replay_account: &RawAccountV1<'_>,
    market: AdapterPositionMarketBindingV3,
    market_binding_account: Key,
    general_market_runtime: Key,
    expected_owner: Key,
    expected_controller: Key,
    supply: &SupplyLedgerAccount,
    verifier: &P,
) -> Result<AuthenticatedBasePositionV3> {
    authenticate_position_v3_pair(
        base_program,
        position_account,
        replay_account,
        AccountRoleV1::UserPosition,
        AccountRoleV1::UserReplay,
        market,
        PositionPurposeV3::General,
        AdapterPositionPurposeBindingV3 {
            owner: identity(expected_owner)?,
            controller: identity(expected_controller)?,
            purpose_binding_id: identity(market_binding_account)?,
        },
        PositionReplayExtensionExpectationV1::General {
            market_runtime: general_market_runtime,
        },
        supply,
        verifier,
    )
}

/// Authenticate the descriptor vault Position V3 and its exact `SCV1` Replay V3.
pub fn authenticate_structured_claim_base_position_v3_v1<P: BasePositionPdaVerifierV1>(
    base_program: Key,
    position_account: &RawAccountV1<'_>,
    replay_account: &RawAccountV1<'_>,
    market: AdapterPositionMarketBindingV3,
    descriptor: &BoundDescriptorV1,
    supply: &SupplyLedgerAccount,
    verifier: &P,
) -> Result<AuthenticatedBasePositionV3> {
    let addresses = descriptor.addresses();
    authenticate_position_v3_pair(
        base_program,
        position_account,
        replay_account,
        AccountRoleV1::VaultPosition,
        AccountRoleV1::VaultReplay,
        market,
        PositionPurposeV3::StructuredClaim,
        AdapterPositionPurposeBindingV3 {
            owner: identity(addresses.vault_owner)?,
            controller: identity(addresses.vault_owner)?,
            purpose_binding_id: identity(descriptor.wrapper_product_id())?,
        },
        PositionReplayExtensionExpectationV1::StructuredClaim {
            descriptor_account: addresses.descriptor,
            wrapper_product_id: descriptor.wrapper_product_id(),
            vault_authority: addresses.vault_owner,
        },
        supply,
        verifier,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PositionReplayExtensionExpectationV1 {
    General {
        market_runtime: Key,
    },
    StructuredClaim {
        descriptor_account: Key,
        wrapper_product_id: Key,
        vault_authority: Key,
    },
}

#[allow(clippy::too_many_arguments)]
fn authenticate_position_v3_pair<P: BasePositionPdaVerifierV1>(
    base_program: Key,
    position_account: &RawAccountV1<'_>,
    replay_account: &RawAccountV1<'_>,
    expected_position_role: AccountRoleV1,
    expected_replay_role: AccountRoleV1,
    market: AdapterPositionMarketBindingV3,
    purpose: PositionPurposeV3,
    binding: AdapterPositionPurposeBindingV3,
    extension_expectation: PositionReplayExtensionExpectationV1,
    supply: &SupplyLedgerAccount,
    verifier: &P,
) -> Result<AuthenticatedBasePositionV3> {
    if is_zero(&base_program)
        || position_account.role != expected_position_role
        || replay_account.role != expected_replay_role
        || position_account.key == replay_account.key
        || position_account.owner != base_program
        || replay_account.owner != base_program
        || position_account.signer
        || replay_account.signer
        || !position_account.writable
        || !replay_account.writable
        || position_account.executable
        || replay_account.executable
    {
        return Err(Error::InvalidAccounts);
    }
    let position =
        PositionAccountV3::decode(position_account.data).map_err(|_| Error::InvalidAccountData)?;
    if !verifier.verify_position_v3(base_program, position_account.key, position.pda_seeds()) {
        return Err(Error::PdaMismatch);
    }
    let _authenticated_position = authenticate_position_v3_exact(
        RetirementAccountViewV2 {
            address: identity(position_account.key)?,
            owner: identity(position_account.owner)?,
            data: position_account.data,
            is_writable: position_account.writable,
            is_executable: position_account.executable,
        },
        identity(base_program)?,
        CanonicalPdaV1::after_derivation(identity(position_account.key)?, position.stored_bump()),
        RetirementAccountAccessV2::Writable,
    )
    .map_err(|_| Error::InvalidAccounts)?;
    match purpose {
        PositionPurposeV3::General => {
            let _ = project_general_position_v3(position, market, binding)
                .map_err(|_| Error::BaseClosureMismatch)?;
        }
        PositionPurposeV3::StructuredClaim => {
            let _ = project_structured_claim_position_v3(position, market, binding)
                .map_err(|_| Error::BaseClosureMismatch)?;
        }
        _ => return Err(Error::BaseClosureMismatch),
    }
    if position.lifecycle() != PositionLifecycleV3::Open
        || position.replay_account().bytes() != replay_account.key
    {
        return Err(Error::BaseClosureMismatch);
    }

    let replay_bump = *replay_account
        .data
        .get(4)
        .ok_or(Error::InvalidAccountData)?;
    if !verifier.verify_replay_v3(
        base_program,
        replay_account.key,
        position_account.key,
        purpose,
        binding.purpose_binding_id.bytes(),
        replay_bump,
    ) {
        return Err(Error::PdaMismatch);
    }
    let authenticated_replay = authenticate_purpose_replay_v3_exact(
        RetirementAccountViewV2 {
            address: identity(replay_account.key)?,
            owner: identity(replay_account.owner)?,
            data: replay_account.data,
            is_writable: replay_account.writable,
            is_executable: replay_account.executable,
        },
        identity(base_program)?,
        CanonicalPdaV1::after_derivation(identity(replay_account.key)?, replay_bump),
        RetirementAccountAccessV2::Writable,
    )
    .map_err(|_| Error::InvalidAccounts)?;
    let sha = AdapterSha256V1;
    let replay = ReplayV3Envelope::decode(authenticated_replay.data(), &sha)
        .map_err(|_| Error::InvalidAccountData)?;
    let header = replay.header();
    let position_semantic_id = position
        .semantic_id(&sha)
        .map_err(|_| Error::InvalidAccountData)?
        .bytes();
    if header.lifecycle() != ReplayV3Lifecycle::Live
        || header.position_account().bytes() != position_account.key
        || header.replay_account().bytes() != replay_account.key
        || header.position_generation() != position.generation()
        || header.purpose() != purpose
        || header.purpose_binding_id().bytes() != binding.purpose_binding_id.bytes()
    {
        return Err(Error::BaseClosureMismatch);
    }
    match extension_expectation {
        PositionReplayExtensionExpectationV1::General { market_runtime } => {
            let extension = GeneralReplayExtensionV1::decode(replay.extension())
                .map_err(|_| Error::InvalidAccountData)?;
            if header.extension_schema().get() != GENERAL_REPLAY_EXTENSION_SCHEMA_V1
                || extension.general_market_runtime().bytes() != market_runtime
                || extension.current_position_semantic_id().bytes() != position_semantic_id
            {
                return Err(Error::BaseClosureMismatch);
            }
        }
        PositionReplayExtensionExpectationV1::StructuredClaim {
            descriptor_account,
            wrapper_product_id,
            vault_authority,
        } => {
            let extension = StructuredClaimReplayExtensionV1::decode(replay.extension())?;
            if header.extension_schema().get() != STRUCTURED_CLAIM_REPLAY_EXTENSION_SCHEMA_V1
                || extension.descriptor_account != descriptor_account
                || extension.wrapper_product_id != wrapper_product_id
                || extension.vault_authority != vault_authority
                || extension.current_position_semantic_id != position_semantic_id
            {
                return Err(Error::BaseClosureMismatch);
            }
        }
    }
    validate_position_supply_v3(position, market, supply)?;
    Ok(AuthenticatedBasePositionV3 {
        position_address: position_account.key,
        replay_address: replay_account.key,
        projection: PositionProjectionV1 {
            market: position.market_instance_id().bytes(),
            owner: position.owner().bytes(),
            generation: position.generation(),
            replay_sequence: header.next_sequence(),
            cash_atoms: position.cash_atoms(),
            reserved_cash_atoms: position.reserved_cash_atoms(),
            internal: position.native_eggs(),
            closed: false,
        },
    })
}

fn validate_position_supply_v3(
    position: PositionAccountV3,
    market: AdapterPositionMarketBindingV3,
    supply: &SupplyLedgerAccount,
) -> Result<()> {
    supply.validate().map_err(|_| Error::BaseClosureMismatch)?;
    if supply.market.bytes() != market.market_instance_id.bytes()
        || supply.realm.bytes() != market.realm_id.bytes()
        || supply.outcome_count != market.outcome_count
    {
        return Err(Error::BaseClosureMismatch);
    }
    let internal = position.native_eggs();
    let mut index = 0_usize;
    while index < usize::from(market.outcome_count) {
        if internal[index] > supply.internal_supply[index] {
            return Err(Error::BaseClosureMismatch);
        }
        index += 1;
    }
    Ok(())
}

fn identity(bytes: Key) -> Result<Identity32V1> {
    Identity32V1::new(bytes).map_err(|_| Error::InvalidAccounts)
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
        StructuredClaimActionV1::RetireDescriptor => RETIRE_REQUIREMENTS,
        StructuredClaimActionV1::WrapCanonical | StructuredClaimActionV1::UnwrapCanonical => {
            CANONICAL_REQUIREMENTS
        }
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => QUANTITY_REQUIREMENTS,
    }
}
