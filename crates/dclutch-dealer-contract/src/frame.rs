//! SDK-free canonical account frames and PDA preimages for Dealer V1.
//!
//! The adapter must additionally authenticate program ownership, decoded state,
//! immutable content hashes, PDA bumps, Realm-selected collateral/token facts,
//! token-account mints/authorities/balances, and the trusted Clock slot. This
//! module refuses reordered, over-privileged, aliased, or extra accounts.

use dclutch_core_contract::{ContentId, Phase};
use dclutch_realm_contract::POSITION_PDA_DOMAIN;

use crate::{MAX_NATIVE_CLAIMS, MIN_NATIVE_CLAIMS, instruction::DealerActionV1};

/// Exact width of an SVM public key.
pub const DEALER_PUBKEY_BYTES: usize = 32;
/// Canonical Pool PDA domain.
pub const DEALER_POOL_PDA_DOMAIN_V1: &[u8] = b"dclutch/dealer-pool/v1";
/// Canonical immutable-config PDA domain.
pub const DEALER_CONFIG_PDA_DOMAIN_V1: &[u8] = b"dclutch/dealer-config/v1";
/// Canonical LP-position PDA domain.
pub const DEALER_LP_PDA_DOMAIN_V1: &[u8] = b"dclutch/dealer-lp/v1";
/// Canonical segregated collateral-vault PDA domain.
pub const DEALER_COLLATERAL_VAULT_PDA_DOMAIN_V1: &[u8] = b"dclutch/dealer-vault/v1";
/// Canonical System Program key.
pub const DEALER_SYSTEM_PROGRAM_ID: [u8; DEALER_PUBKEY_BYTES] = [0; DEALER_PUBKEY_BYTES];
/// Canonical Rent sysvar key.
pub const DEALER_RENT_SYSVAR_ID: [u8; DEALER_PUBKEY_BYTES] = [
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155, 161,
    253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
];
/// Solana packet data width pinned for the V1 risk report.
pub const SOLANA_PACKET_DATA_SIZE_V1: usize = 1_232;
/// Current protocol account-lock ceiling used by the V1 risk report.
pub const SOLANA_ACCOUNT_LOCK_LIMIT_V1: usize = 128;

/// Refusal from a Dealer account frame or derivation preimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameError {
    /// Exact-N lies outside the labeled V1 profile.
    UnsupportedProfile,
    /// Account count or role index did not match the exact action contract.
    InvalidAccountFrame,
    /// An ordinary account or compact identity used the zero sentinel.
    ZeroIdentity,
    /// Signer, writable, or executable privilege differed from the exact role.
    InvalidPrivilege,
    /// A non-authorized pair of roles used the same physical key.
    UnsafeAlias,
    /// System Program key or executable privilege was not canonical.
    InvalidSystemProgram,
    /// Rent sysvar key or privileges were not canonical.
    InvalidRentSysvar,
    /// The Market phase does not admit this lifecycle action.
    InvalidMarketPhase,
    /// Checked account-width arithmetic overflowed.
    ArithmeticOverflow,
}

/// Result alias for Dealer frame validation.
pub type Result<T> = core::result::Result<T, FrameError>;

/// One hostile runtime account projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerAccountMetaV1 {
    /// Exact public-key bytes.
    pub key: [u8; DEALER_PUBKEY_BYTES],
    /// Runtime signer privilege.
    pub is_signer: bool,
    /// Runtime writable privilege.
    pub is_writable: bool,
    /// Runtime executable flag.
    pub is_executable: bool,
}

/// Semantic role at one exact ordered frame index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerAccountRoleV1 {
    /// Permissionless activation transaction payer.
    Activator,
    /// System payer for a new LP-position account.
    SystemPayer,
    /// Immutable initial liquidity owner or later LP owner.
    LpOwner,
    /// Permissionless trader controlling their custody accounts.
    Trader,
    /// Immutable Realm selecting collateral and token program.
    Realm,
    /// Canonical Market root.
    Market,
    /// Immutable capability manifest committed by Market identity.
    CapabilityManifest,
    /// Selected capability funding ledger under shared capability derivation.
    FundingState,
    /// Immutable Dealer ladder configuration.
    LiquidityConfig,
    /// Mutable Dealer Pool root.
    Pool,
    /// Mutable or vacant compact LP position.
    LpPosition,
    /// LP/trader native Market Position supplying or receiving claims.
    ParticipantPosition,
    /// Pool-owned native Market Position holding all categorized claim inventory.
    PoolPosition,
    /// Owner-bound collateral token account.
    CollateralVault,
    /// Pool principal-collateral custody.
    PoolPrincipalVault,
    /// Pool realized-fee custody.
    PoolFeeVault,
    /// Pool segregated service-funding custody.
    PoolServiceVault,
    /// Immutable service-refund collateral destination.
    ServiceRefundVault,
    /// Permanent RentCredit receiving all Pool close lamports.
    PoolRentCredit,
    /// Permanent RentCredit receiving all config close lamports.
    ConfigRentCredit,
    /// Permanent RentCredit receiving all LP-position close lamports.
    LpRentCredit,
    /// Permanent RentCredit of the Pool authority receiving Pool Position rent.
    PoolPositionRentCredit,
    /// Realm collateral mint.
    CollateralMint,
    /// Realm-selected executable token program.
    TokenProgram,
    /// Canonical executable System Program.
    SystemProgram,
    /// Canonical nonexecutable Rent sysvar.
    RentSysvar,
}

/// Validated borrowed exact frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFrameV1<'a, const N: usize> {
    action: DealerActionV1,
    accounts: &'a [DealerAccountMetaV1],
}

impl<'a, const N: usize> DealerFrameV1<'a, N> {
    /// Validate exact count, ordering-derived privileges, fixed identities, and aliases.
    pub fn new(action: DealerActionV1, accounts: &'a [DealerAccountMetaV1]) -> Result<Self> {
        validate_profile::<N>()?;
        if accounts.len() != dealer_account_count::<N>(action)? {
            return Err(FrameError::InvalidAccountFrame);
        }
        for (index, account) in accounts.iter().enumerate() {
            let role = dealer_account_role::<N>(action, index)?;
            validate_meta(action, role, *account)?;
            let prior_accounts = accounts
                .get(..index)
                .ok_or(FrameError::InvalidAccountFrame)?;
            for (prior_index, prior) in prior_accounts.iter().enumerate() {
                if prior.key == account.key {
                    let prior_role = dealer_account_role::<N>(action, prior_index)?;
                    if !safe_alias(action, prior_role, role) {
                        return Err(FrameError::UnsafeAlias);
                    }
                }
            }
        }
        Ok(Self { action, accounts })
    }

    /// Return exact action selected by this frame.
    pub const fn action(self) -> DealerActionV1 {
        self.action
    }

    /// Return exact validated ordered account projections.
    pub const fn accounts(self) -> &'a [DealerAccountMetaV1] {
        self.accounts
    }

    /// Return one account by semantic role occurrence.
    pub fn account(
        self,
        role: DealerAccountRoleV1,
        occurrence: usize,
    ) -> Result<DealerAccountMetaV1> {
        let mut seen = 0usize;
        for (index, account) in self.accounts.iter().enumerate() {
            if dealer_account_role::<N>(self.action, index)? == role {
                if seen == occurrence {
                    return Ok(*account);
                }
                seen = seen.checked_add(1).ok_or(FrameError::ArithmeticOverflow)?;
            }
        }
        Err(FrameError::InvalidAccountFrame)
    }
}

/// Return exact account count for an action and exact-N profile.
pub fn dealer_account_count<const N: usize>(action: DealerActionV1) -> Result<usize> {
    validate_profile::<N>()?;
    let base: usize = match action {
        DealerActionV1::ActivatePool => 23,
        DealerActionV1::CreateLpPosition => 9,
        DealerActionV1::AddLiquidity | DealerActionV1::RemoveLiquidity => 13,
        DealerActionV1::Trade => 12,
        DealerActionV1::ResetLadder => 3,
        DealerActionV1::CloseLpPosition => 7,
        DealerActionV1::RetirePool => 15,
    };
    Ok(base)
}

/// Return the exact semantic role for an action/index.
pub fn dealer_account_role<const N: usize>(
    action: DealerActionV1,
    index: usize,
) -> Result<DealerAccountRoleV1> {
    if index >= dealer_account_count::<N>(action)? {
        return Err(FrameError::InvalidAccountFrame);
    }
    match action {
        DealerActionV1::ActivatePool => role_at(
            index,
            &[
                DealerAccountRoleV1::Activator,
                DealerAccountRoleV1::LpOwner,
                DealerAccountRoleV1::Realm,
                DealerAccountRoleV1::Market,
                DealerAccountRoleV1::CapabilityManifest,
                DealerAccountRoleV1::FundingState,
                DealerAccountRoleV1::LiquidityConfig,
                DealerAccountRoleV1::Pool,
                DealerAccountRoleV1::LpPosition,
                DealerAccountRoleV1::ParticipantPosition,
                DealerAccountRoleV1::PoolPosition,
                DealerAccountRoleV1::CollateralVault,
                DealerAccountRoleV1::PoolPrincipalVault,
                DealerAccountRoleV1::PoolFeeVault,
                DealerAccountRoleV1::PoolServiceVault,
                DealerAccountRoleV1::PoolPositionRentCredit,
                DealerAccountRoleV1::PoolRentCredit,
                DealerAccountRoleV1::ConfigRentCredit,
                DealerAccountRoleV1::LpRentCredit,
                DealerAccountRoleV1::CollateralMint,
                DealerAccountRoleV1::TokenProgram,
                DealerAccountRoleV1::SystemProgram,
                DealerAccountRoleV1::RentSysvar,
            ],
        ),
        DealerActionV1::CreateLpPosition => role_at(
            index,
            &[
                DealerAccountRoleV1::SystemPayer,
                DealerAccountRoleV1::LpOwner,
                DealerAccountRoleV1::Market,
                DealerAccountRoleV1::Pool,
                DealerAccountRoleV1::LiquidityConfig,
                DealerAccountRoleV1::LpPosition,
                DealerAccountRoleV1::LpRentCredit,
                DealerAccountRoleV1::SystemProgram,
                DealerAccountRoleV1::RentSysvar,
            ],
        ),
        DealerActionV1::AddLiquidity | DealerActionV1::RemoveLiquidity => role_at(
            index,
            &[
                DealerAccountRoleV1::LpOwner,
                DealerAccountRoleV1::Realm,
                DealerAccountRoleV1::Market,
                DealerAccountRoleV1::Pool,
                DealerAccountRoleV1::LiquidityConfig,
                DealerAccountRoleV1::LpPosition,
                DealerAccountRoleV1::ParticipantPosition,
                DealerAccountRoleV1::PoolPosition,
                DealerAccountRoleV1::CollateralVault,
                DealerAccountRoleV1::PoolPrincipalVault,
                DealerAccountRoleV1::PoolFeeVault,
                DealerAccountRoleV1::CollateralMint,
                DealerAccountRoleV1::TokenProgram,
            ],
        ),
        DealerActionV1::Trade => role_at(
            index,
            &[
                DealerAccountRoleV1::Trader,
                DealerAccountRoleV1::Realm,
                DealerAccountRoleV1::Market,
                DealerAccountRoleV1::Pool,
                DealerAccountRoleV1::LiquidityConfig,
                DealerAccountRoleV1::ParticipantPosition,
                DealerAccountRoleV1::PoolPosition,
                DealerAccountRoleV1::CollateralVault,
                DealerAccountRoleV1::PoolPrincipalVault,
                DealerAccountRoleV1::PoolFeeVault,
                DealerAccountRoleV1::CollateralMint,
                DealerAccountRoleV1::TokenProgram,
            ],
        ),
        DealerActionV1::ResetLadder => role_at(
            index,
            &[
                DealerAccountRoleV1::Market,
                DealerAccountRoleV1::Pool,
                DealerAccountRoleV1::LiquidityConfig,
            ],
        ),
        DealerActionV1::CloseLpPosition => role_at(
            index,
            &[
                DealerAccountRoleV1::LpOwner,
                DealerAccountRoleV1::Market,
                DealerAccountRoleV1::Pool,
                DealerAccountRoleV1::LiquidityConfig,
                DealerAccountRoleV1::LpPosition,
                DealerAccountRoleV1::LpRentCredit,
                DealerAccountRoleV1::SystemProgram,
            ],
        ),
        DealerActionV1::RetirePool => role_at(
            index,
            &[
                DealerAccountRoleV1::Market,
                DealerAccountRoleV1::Realm,
                DealerAccountRoleV1::Pool,
                DealerAccountRoleV1::LiquidityConfig,
                DealerAccountRoleV1::PoolPosition,
                DealerAccountRoleV1::PoolPrincipalVault,
                DealerAccountRoleV1::PoolFeeVault,
                DealerAccountRoleV1::PoolServiceVault,
                DealerAccountRoleV1::ServiceRefundVault,
                DealerAccountRoleV1::PoolPositionRentCredit,
                DealerAccountRoleV1::PoolRentCredit,
                DealerAccountRoleV1::ConfigRentCredit,
                DealerAccountRoleV1::CollateralMint,
                DealerAccountRoleV1::TokenProgram,
                DealerAccountRoleV1::SystemProgram,
            ],
        ),
    }
}

/// Validate Market lifecycle admission independently of untrusted account bytes.
pub const fn validate_market_phase(action: DealerActionV1, phase: Phase) -> Result<()> {
    let admitted = match action {
        DealerActionV1::ActivatePool => matches!(phase, Phase::Founding | Phase::Open),
        DealerActionV1::CreateLpPosition
        | DealerActionV1::AddLiquidity
        | DealerActionV1::Trade
        | DealerActionV1::ResetLadder => matches!(phase, Phase::Open),
        DealerActionV1::RemoveLiquidity | DealerActionV1::CloseLpPosition => {
            matches!(phase, Phase::Open | Phase::Resolved | Phase::Retiring)
        }
        DealerActionV1::RetirePool => matches!(phase, Phase::Retiring),
    };
    if admitted {
        Ok(())
    } else {
        Err(FrameError::InvalidMarketPhase)
    }
}

/// Exact Pool PDA seed projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolPdaSeedsV1 {
    market: [u8; 32],
    generation_le: [u8; 8],
    config_id: [u8; 32],
}

impl PoolPdaSeedsV1 {
    /// Construct from authenticated Market occurrence and immutable config ID.
    pub fn new(market: [u8; 32], generation: u64, config_id: ContentId) -> Result<Self> {
        require_nonzero(market)?;
        Ok(Self {
            market,
            generation_le: generation.to_le_bytes(),
            config_id: config_id.to_bytes(),
        })
    }
    /// Return ordered domain-separated seed components.
    pub fn seed_components(&self) -> [&[u8]; 4] {
        [
            DEALER_POOL_PDA_DOMAIN_V1,
            self.market.as_slice(),
            self.generation_le.as_slice(),
            self.config_id.as_slice(),
        ]
    }
    /// Return authenticated Market key seed.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }
    /// Return immutable Market generation seed.
    pub const fn generation(self) -> u64 {
        u64::from_le_bytes(self.generation_le)
    }
    /// Return immutable config content seed.
    pub const fn config_id(self) -> [u8; 32] {
        self.config_id
    }
}

/// Exact immutable-config PDA seed projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfigPdaSeedsV1(PoolPdaSeedsV1);

impl ConfigPdaSeedsV1 {
    /// Construct from authenticated Market occurrence and immutable config ID.
    pub fn new(market: [u8; 32], generation: u64, config_id: ContentId) -> Result<Self> {
        Ok(Self(PoolPdaSeedsV1::new(market, generation, config_id)?))
    }
    /// Return ordered domain-separated seed components.
    pub fn seed_components(&self) -> [&[u8]; 4] {
        [
            DEALER_CONFIG_PDA_DOMAIN_V1,
            self.0.market.as_slice(),
            self.0.generation_le.as_slice(),
            self.0.config_id.as_slice(),
        ]
    }
}

/// Exact compact LP-position PDA seed projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LpPositionPdaSeedsV1 {
    market: [u8; 32],
    generation_le: [u8; 8],
    config_id: [u8; 32],
    lp_id: [u8; 32],
}

impl LpPositionPdaSeedsV1 {
    /// Construct from Market occurrence, immutable config, and compact LP ID.
    pub fn new(
        market: [u8; 32],
        generation: u64,
        config_id: ContentId,
        lp_id: [u8; 32],
    ) -> Result<Self> {
        require_nonzero(market)?;
        require_nonzero(lp_id)?;
        Ok(Self {
            market,
            generation_le: generation.to_le_bytes(),
            config_id: config_id.to_bytes(),
            lp_id,
        })
    }
    /// Return ordered domain-separated seed components.
    pub fn seed_components(&self) -> [&[u8]; 5] {
        [
            DEALER_LP_PDA_DOMAIN_V1,
            self.market.as_slice(),
            self.generation_le.as_slice(),
            self.config_id.as_slice(),
            self.lp_id.as_slice(),
        ]
    }
    /// Return compact LP-position identity seed.
    pub const fn lp_id(self) -> [u8; 32] {
        self.lp_id
    }
}

/// Shared native Position PDA seed projection for Pool-owned claim inventory.
///
/// This deliberately reuses the Realm contract's canonical Position domain and
/// its exact `[domain, Market, owner]` tuple, with the authenticated Pool PDA as
/// owner. Dealer does not invent a parallel native-claim custody address.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolPositionPdaSeedsV1 {
    market: [u8; 32],
    pool: [u8; 32],
}

impl PoolPositionPdaSeedsV1 {
    /// Construct from authenticated Market and Pool keys.
    pub fn new(market: [u8; 32], pool: [u8; 32]) -> Result<Self> {
        require_nonzero(market)?;
        require_nonzero(pool)?;
        if market == pool {
            return Err(FrameError::UnsafeAlias);
        }
        Ok(Self { market, pool })
    }

    /// Return the Realm-owned exact Position seed tuple.
    pub fn seed_components(&self) -> [&[u8]; 3] {
        [
            POSITION_PDA_DOMAIN,
            self.market.as_slice(),
            self.pool.as_slice(),
        ]
    }

    /// Return authenticated Pool authority used as Position owner.
    pub const fn pool(self) -> [u8; 32] {
        self.pool
    }
}

/// One physically segregated Pool collateral compartment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerCollateralCompartmentV1 {
    /// LP-owned principal collateral.
    Principal = 0,
    /// LP-owned realized trader-paid fees.
    RealizedFees = 1,
    /// Non-LP prepaid service funding.
    Service = 2,
}

impl DealerCollateralCompartmentV1 {
    const fn tag(self) -> [u8; 1] {
        [self as u8]
    }
}

/// Exact PDA seed projection for one segregated collateral token Vault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerCollateralVaultPdaSeedsV1 {
    pool: [u8; 32],
    compartment_tag: [u8; 1],
}

impl DealerCollateralVaultPdaSeedsV1 {
    /// Construct from authenticated Pool and one disjoint compartment tag.
    pub fn new(pool: [u8; 32], compartment: DealerCollateralCompartmentV1) -> Result<Self> {
        require_nonzero(pool)?;
        Ok(Self {
            pool,
            compartment_tag: compartment.tag(),
        })
    }

    /// Return ordered domain-separated seed components.
    pub fn seed_components(&self) -> [&[u8]; 3] {
        [
            DEALER_COLLATERAL_VAULT_PDA_DOMAIN_V1,
            self.pool.as_slice(),
            self.compartment_tag.as_slice(),
        ]
    }

    /// Return exact persisted-free compartment tag.
    pub const fn compartment_tag(self) -> u8 {
        u8::from_le_bytes(self.compartment_tag)
    }
}

fn validate_meta(
    action: DealerActionV1,
    role: DealerAccountRoleV1,
    account: DealerAccountMetaV1,
) -> Result<()> {
    if role == DealerAccountRoleV1::SystemProgram {
        if account.key != DEALER_SYSTEM_PROGRAM_ID
            || account.is_signer
            || account.is_writable
            || !account.is_executable
        {
            return Err(FrameError::InvalidSystemProgram);
        }
        return Ok(());
    }
    if role == DealerAccountRoleV1::RentSysvar {
        if account.key != DEALER_RENT_SYSVAR_ID
            || account.is_signer
            || account.is_writable
            || account.is_executable
        {
            return Err(FrameError::InvalidRentSysvar);
        }
        return Ok(());
    }
    require_nonzero(account.key)?;
    let expected = dealer_account_privileges(action, role);
    if (
        account.is_signer,
        account.is_writable,
        account.is_executable,
    ) != expected
    {
        return Err(FrameError::InvalidPrivilege);
    }
    Ok(())
}

/// Return exact signer, writable, and executable bits for one action role.
pub const fn dealer_account_privileges(
    action: DealerActionV1,
    role: DealerAccountRoleV1,
) -> (bool, bool, bool) {
    let writable = match role {
        DealerAccountRoleV1::Activator
        | DealerAccountRoleV1::SystemPayer
        | DealerAccountRoleV1::Trader
        | DealerAccountRoleV1::FundingState
        | DealerAccountRoleV1::Pool
        | DealerAccountRoleV1::LpPosition
        | DealerAccountRoleV1::ParticipantPosition
        | DealerAccountRoleV1::PoolPosition
        | DealerAccountRoleV1::CollateralVault
        | DealerAccountRoleV1::PoolPrincipalVault
        | DealerAccountRoleV1::PoolFeeVault
        | DealerAccountRoleV1::PoolServiceVault
        | DealerAccountRoleV1::ServiceRefundVault => true,
        DealerAccountRoleV1::Market => {
            matches!(
                action,
                DealerActionV1::ActivatePool | DealerActionV1::RetirePool
            )
        }
        DealerAccountRoleV1::LiquidityConfig => matches!(action, DealerActionV1::RetirePool),
        DealerAccountRoleV1::PoolRentCredit | DealerAccountRoleV1::ConfigRentCredit => {
            matches!(action, DealerActionV1::RetirePool)
        }
        DealerAccountRoleV1::LpRentCredit => {
            matches!(action, DealerActionV1::CloseLpPosition)
        }
        DealerAccountRoleV1::PoolPositionRentCredit => {
            matches!(action, DealerActionV1::RetirePool)
        }
        DealerAccountRoleV1::LpOwner => matches!(
            action,
            DealerActionV1::ActivatePool | DealerActionV1::CreateLpPosition
        ),
        DealerAccountRoleV1::Realm
        | DealerAccountRoleV1::CapabilityManifest
        | DealerAccountRoleV1::CollateralMint
        | DealerAccountRoleV1::TokenProgram
        | DealerAccountRoleV1::SystemProgram
        | DealerAccountRoleV1::RentSysvar => false,
    };
    let signer = matches!(
        role,
        DealerAccountRoleV1::Activator
            | DealerAccountRoleV1::SystemPayer
            | DealerAccountRoleV1::LpOwner
            | DealerAccountRoleV1::Trader
    );
    let executable = matches!(
        role,
        DealerAccountRoleV1::TokenProgram | DealerAccountRoleV1::SystemProgram
    );
    (signer, writable, executable)
}

const fn safe_alias(
    action: DealerActionV1,
    left: DealerAccountRoleV1,
    right: DealerAccountRoleV1,
) -> bool {
    let payer_owner = matches!(
        (left, right),
        (DealerAccountRoleV1::Activator, DealerAccountRoleV1::LpOwner)
            | (
                DealerAccountRoleV1::SystemPayer,
                DealerAccountRoleV1::LpOwner
            )
            | (DealerAccountRoleV1::LpOwner, DealerAccountRoleV1::Activator)
            | (
                DealerAccountRoleV1::LpOwner,
                DealerAccountRoleV1::SystemPayer
            )
    ) && matches!(
        action,
        DealerActionV1::ActivatePool | DealerActionV1::CreateLpPosition
    );
    let rent_credits = is_rent_credit(left) && is_rent_credit(right);
    payer_owner || rent_credits
}

const fn is_rent_credit(role: DealerAccountRoleV1) -> bool {
    matches!(
        role,
        DealerAccountRoleV1::PoolRentCredit
            | DealerAccountRoleV1::ConfigRentCredit
            | DealerAccountRoleV1::LpRentCredit
    )
}

fn role_at(index: usize, roles: &[DealerAccountRoleV1]) -> Result<DealerAccountRoleV1> {
    roles
        .get(index)
        .copied()
        .ok_or(FrameError::InvalidAccountFrame)
}

fn validate_profile<const N: usize>() -> Result<()> {
    if (MIN_NATIVE_CLAIMS..=MAX_NATIVE_CLAIMS).contains(&N) {
        Ok(())
    } else {
        Err(FrameError::UnsupportedProfile)
    }
}

fn require_nonzero(value: [u8; 32]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(FrameError::ZeroIdentity)
    } else {
        Ok(())
    }
}
