//! Exact ordered account-role schemas independent of Solana SDK types.

use crate::{Error, Result, instruction::InstructionTag};
use dclutch_core_contract::MarketRoot;
use dclutch_realm_contract::RealmV1;

/// Semantic identity class an SVM adapter must authenticate for one role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountClass {
    /// Caller or protocol authority whose exact identity is operation-specific.
    Authority,
    /// Ordinary system-owned account receiving or providing lamports.
    SystemAccount,
    /// Program-derived mutable protocol state.
    ProtocolState,
    /// Immutable protocol record authenticated by a content commitment.
    ImmutableProtocolRecord,
    /// Collateral Mint named by the immutable Realm.
    CollateralMint,
    /// Collateral token account checked by the selected adapter release.
    CollateralTokenAccount,
    /// The one exact System Program address and executable owner.
    SystemProgram,
    /// The exact executable token program named by the immutable Realm.
    RealmTokenProgram,
    /// The one exact Rent sysvar address and sysvar owner.
    RentSysvar,
}

/// Semantic name of one ordered account role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// Signer funding creation of a Position account; need not own it.
    PositionPayer,
    /// Signer capitalizing account rent and resolution liveness.
    Sponsor,
    /// Signer that owns one Position and authorizes token movement.
    PositionOwner,
    /// Immutable reusable collateral Realm PDA.
    Realm,
    /// Immutable occurrence-specific Product Instance committed by Market identity.
    ProductInstance,
    /// Immutable Product ClaimBasis committed by Market and Product Instance.
    ClaimBasis,
    /// Immutable Product CapacityProfile governing the admitted ClaimBasis.
    CapacityProfile,
    /// Mutable Market root and collateral-liability state.
    Market,
    /// Prepaid one-shot resolution Fund direct child.
    ResolutionFund,
    /// Mutable manifest-bound generic capability funding ledger.
    FundingState,
    /// Immutable resolution-policy record committed by Market identity.
    ResolutionPolicy,
    /// Immutable capability manifest committed by Market identity.
    CapabilityManifest,
    /// Transient program-owned canonical capability-opening readiness child.
    CapabilityReadiness,
    /// Market's collateral Vault token account.
    CollateralVault,
    /// Program-owned collateral-custody root and rent-refund contract.
    CollateralCustody,
    /// Authenticated recipient of custody-compartment rent principal.
    RentRefund,
    /// Native claims owned by one Market participant.
    Position,
    /// Existing Position receiving a liability-neutral claim transfer.
    DestinationPosition,
    /// Permanent native-rent credit bound to a Position owner.
    RentCredit,
    /// User token account supplying collateral for a split.
    CollateralSource,
    /// Compatible token account receiving released collateral or surplus.
    CollateralDestination,
    /// Collateral Mint named by the Realm.
    CollateralMint,
    /// Executable token program named by the Realm.
    TokenProgram,
    /// Executable System Program.
    SystemProgram,
    /// Canonical Rent sysvar.
    RentSysvar,
}

/// One exact role and the only privileges admitted for it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountRole {
    role: Role,
    class: AccountClass,
    signer: bool,
    writable: bool,
    executable: bool,
}

impl AccountRole {
    const fn new(
        role: Role,
        class: AccountClass,
        signer: bool,
        writable: bool,
        executable: bool,
    ) -> Self {
        Self {
            role,
            class,
            signer,
            writable,
            executable,
        }
    }

    /// Return the semantic role name.
    pub const fn role(self) -> Role {
        self.role
    }

    /// Return the identity class requiring adapter authentication.
    pub const fn class(self) -> AccountClass {
        self.class
    }

    /// Return whether the exact role must be a signer.
    pub const fn is_signer(self) -> bool {
        self.signer
    }

    /// Return whether the exact role must be writable.
    pub const fn is_writable(self) -> bool {
        self.writable
    }

    /// Return whether the exact role must be executable.
    pub const fn is_executable(self) -> bool {
        self.executable
    }
}

/// Runtime privileges observed for one ordered account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountPrivilege {
    /// Whether the runtime presents this account as a signer.
    pub is_signer: bool,
    /// Whether the runtime presents this account as writable.
    pub is_writable: bool,
    /// Whether the runtime presents this account as executable.
    pub is_executable: bool,
}

/// Authenticated decoded facts of a collateral token account used by a
/// permissionless surplus sweep.
///
/// The composing adapter obtains these facts from the token account selected
/// by the immutable Realm's token-program release. This SDK-free type does not
/// decode a token account itself and cannot substitute for authentication of
/// the account address, token-program owner, mint, or token owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SweepSurplusTokenAccountFactsV1 {
    address: [u8; 32],
    mint: [u8; 32],
    token_owner: [u8; 32],
}

impl SweepSurplusTokenAccountFactsV1 {
    /// Construct nonzero decoded token-account facts.
    pub fn new(address: [u8; 32], mint: [u8; 32], token_owner: [u8; 32]) -> Result<Self> {
        if is_zero_identifier(&address)
            || is_zero_identifier(&mint)
            || is_zero_identifier(&token_owner)
        {
            return Err(Error::ZeroIdentifier);
        }
        Ok(Self {
            address,
            mint,
            token_owner,
        })
    }

    /// Return the token-account address authenticated by the adapter.
    pub const fn address(self) -> [u8; 32] {
        self.address
    }

    /// Return the token Mint decoded and authenticated by the adapter.
    pub const fn mint(self) -> [u8; 32] {
        self.mint
    }

    /// Return the token-account owner decoded and authenticated by the adapter.
    pub const fn token_owner(self) -> [u8; 32] {
        self.token_owner
    }
}

/// Authorize the fixed surplus-sweep destination selected by immutable Market
/// and Realm state.
///
/// The composing adapter must first authenticate `market` as the exact Market
/// root, `realm` as the root-committed Realm, and both fact records as the
/// decoded token accounts in [`SWEEP_SURPLUS_FRAME`]. A permissionless sweep
/// may transfer only to a different account whose Mint is the Realm Mint and
/// whose token owner is the immutable `MarketRoot::rent_refund` identity.
/// There is no caller-selected destination, destination signer, persisted
/// treasury field, or fee authority in this contract.
///
/// After this check, the adapter must transfer exactly
/// `vault.amount - Market.hoard` and leave `Market.hoard` unchanged. This
/// helper intentionally owns destination authorization only; the authenticated
/// adapter owns token balances, checked subtraction, and CPI atomicity.
pub fn authorize_sweep_surplus_destination(
    market: MarketRoot,
    realm: RealmV1,
    vault: SweepSurplusTokenAccountFactsV1,
    destination: SweepSurplusTokenAccountFactsV1,
) -> Result<()> {
    if vault.address() == destination.address() {
        return Err(Error::SweepDestinationAliasesVault);
    }
    if destination.mint() != *realm.collateral_mint() {
        return Err(Error::SweepDestinationMintMismatch);
    }
    if destination.token_owner() != market.rent_refund() {
        return Err(Error::SweepDestinationOwnerMismatch);
    }
    Ok(())
}

fn is_zero_identifier(identifier: &[u8; 32]) -> bool {
    identifier.iter().all(|byte| *byte == 0)
}

/// Borrowed exact frame associated with one semantic instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstructionFrame {
    tag: InstructionTag,
    roles: &'static [AccountRole],
}

impl InstructionFrame {
    /// Return the instruction whose frame is described.
    pub const fn tag(self) -> InstructionTag {
        self.tag
    }

    /// Borrow all exact ordered account roles.
    pub const fn roles(self) -> &'static [AccountRole] {
        self.roles
    }
}

const fn authority(role: Role, signer: bool, writable: bool) -> AccountRole {
    AccountRole::new(role, AccountClass::Authority, signer, writable, false)
}

const fn sponsor() -> AccountRole {
    AccountRole::new(
        Role::Sponsor,
        AccountClass::SystemAccount,
        true,
        true,
        false,
    )
}

const fn state(role: Role, writable: bool) -> AccountRole {
    AccountRole::new(role, AccountClass::ProtocolState, false, writable, false)
}

const fn immutable(role: Role) -> AccountRole {
    AccountRole::new(
        role,
        AccountClass::ImmutableProtocolRecord,
        false,
        false,
        false,
    )
}

const fn mint() -> AccountRole {
    AccountRole::new(
        Role::CollateralMint,
        AccountClass::CollateralMint,
        false,
        false,
        false,
    )
}

const fn token_account(role: Role) -> AccountRole {
    AccountRole::new(
        role,
        AccountClass::CollateralTokenAccount,
        false,
        true,
        false,
    )
}

const TOKEN_PROGRAM: AccountRole = AccountRole::new(
    Role::TokenProgram,
    AccountClass::RealmTokenProgram,
    false,
    false,
    true,
);
const SYSTEM_PROGRAM: AccountRole = AccountRole::new(
    Role::SystemProgram,
    AccountClass::SystemProgram,
    false,
    false,
    true,
);
const RENT_SYSVAR: AccountRole = AccountRole::new(
    Role::RentSysvar,
    AccountClass::RentSysvar,
    false,
    false,
    false,
);

/// Exact immutable-Realm creation frame.
pub const CREATE_REALM_FRAME: [AccountRole; 6] = [
    sponsor(),
    AccountRole::new(
        Role::Realm,
        AccountClass::ImmutableProtocolRecord,
        false,
        true,
        false,
    ),
    mint(),
    TOKEN_PROGRAM,
    SYSTEM_PROGRAM,
    RENT_SYSVAR,
];

/// Exact atomic Market and resolution-Fund founding frame.
///
/// `CapabilityManifest` is the sole funding authority. The adapter selects the
/// unique founding-required entry whose config is the authenticated
/// `ResolutionPolicy`, validates its specialized Fund quote against `Rent`,
/// and derives all Fund amounts from that entry. No founding instruction field
/// or sponsor argument may override the immutable quote.
pub const FOUND_MARKET_AND_FUND_FRAME: [AccountRole; 12] = [
    sponsor(),
    state(Role::Market, true),
    state(Role::FundingState, true),
    state(Role::RentCredit, false),
    immutable(Role::Realm),
    immutable(Role::ProductInstance),
    immutable(Role::ClaimBasis),
    immutable(Role::CapacityProfile),
    immutable(Role::ResolutionPolicy),
    immutable(Role::CapabilityManifest),
    SYSTEM_PROGRAM,
    RENT_SYSVAR,
];

/// Exact collateral-Vault initialization and Market-open frame.
///
/// The adapter must authenticate `CapabilityManifest` against the Market root,
/// decode its canonical bytes, and prove `CapabilityReadiness` is Ready for
/// this exact Market, generation, and manifest. It then atomically consumes
/// readiness while creating custody and refunding its rent. Readiness is a
/// direct Market child, so this replacement keeps the Market child count
/// coherent.
pub const OPEN_COLLATERAL_VAULT_FRAME: [AccountRole; 12] = [
    sponsor(),
    state(Role::Market, true),
    state(Role::CapabilityReadiness, true),
    state(Role::RentCredit, true),
    immutable(Role::CapabilityManifest),
    immutable(Role::Realm),
    state(Role::CollateralCustody, true),
    token_account(Role::CollateralVault),
    mint(),
    TOKEN_PROGRAM,
    SYSTEM_PROGRAM,
    RENT_SYSVAR,
];

/// Exact Position creation and first complete-set split frame.
pub const CREATE_POSITION_AND_SPLIT_FRAME: [AccountRole; 12] = [
    AccountRole::new(
        Role::PositionPayer,
        AccountClass::SystemAccount,
        true,
        true,
        false,
    ),
    authority(Role::PositionOwner, true, false),
    state(Role::Market, true),
    immutable(Role::Realm),
    state(Role::Position, true),
    state(Role::RentCredit, false),
    token_account(Role::CollateralVault),
    token_account(Role::CollateralSource),
    mint(),
    TOKEN_PROGRAM,
    SYSTEM_PROGRAM,
    RENT_SYSVAR,
];

/// Exact existing-Position complete-set split frame.
pub const SPLIT_COMPLETE_SET_FRAME: [AccountRole; 8] = [
    authority(Role::PositionOwner, true, false),
    state(Role::Market, true),
    immutable(Role::Realm),
    state(Role::Position, true),
    token_account(Role::CollateralVault),
    token_account(Role::CollateralSource),
    mint(),
    TOKEN_PROGRAM,
];

/// Exact complete-set merge and collateral-release frame.
pub const MERGE_COMPLETE_SET_FRAME: [AccountRole; 8] = [
    authority(Role::PositionOwner, true, false),
    state(Role::Market, true),
    immutable(Role::Realm),
    state(Role::Position, true),
    token_account(Role::CollateralVault),
    token_account(Role::CollateralDestination),
    mint(),
    TOKEN_PROGRAM,
];

/// Exact resolved-outcome redemption frame.
pub const REDEEM_RESOLVED_OUTCOME_FRAME: [AccountRole; 8] = [
    authority(Role::PositionOwner, true, false),
    state(Role::Market, true),
    immutable(Role::Realm),
    state(Role::Position, true),
    token_account(Role::CollateralVault),
    token_account(Role::CollateralDestination),
    mint(),
    TOKEN_PROGRAM,
];

/// Exact liability-neutral native-claim transfer frame.
///
/// The source owner alone signs. Both Position accounts are existing mutable
/// children of the same Market and must be distinct; the SVM adapter verifies
/// each stored owner and PDA independently. This operation has no collateral,
/// Vault, rent, or Market-child side effect.
pub const TRANSFER_CLAIMS_FRAME: [AccountRole; 4] = [
    authority(Role::PositionOwner, true, false),
    state(Role::Market, false),
    state(Role::Position, true),
    state(Role::DestinationPosition, true),
];

/// Exact permissionless collateral-surplus sweep frame.
///
/// The adapter must authenticate the Market root, Realm, Vault, and
/// Destination then call [`authorize_sweep_surplus_destination`] with decoded
/// token-account facts. `CollateralDestination` must be a different
/// Realm-Mint token account owned by `MarketRoot::rent_refund`; it is not a
/// caller-selected recipient. No role is a signer. The only admitted transfer
/// is exactly `vault.amount - Market.hoard`, and the sweep must not mutate
/// Hoard.
pub const SWEEP_SURPLUS_FRAME: [AccountRole; 6] = [
    state(Role::Market, true),
    immutable(Role::Realm),
    token_account(Role::CollateralVault),
    token_account(Role::CollateralDestination),
    mint(),
    TOKEN_PROGRAM,
];

/// Exact empty-Position retirement frame.
pub const CLOSE_EMPTY_POSITION_FRAME: [AccountRole; 3] = [
    state(Role::Market, true),
    state(Role::Position, true),
    state(Role::RentCredit, true),
];

/// Exact permissionless terminal-Market compaction frame.
pub const COMPACT_TERMINAL_MARKET_FRAME: [AccountRole; 3] = [
    state(Role::Market, true),
    state(Role::RentCredit, true),
    RENT_SYSVAR,
];

/// Exact empty collateral-Vault retirement frame.
///
/// `RentRefund` must match the immutable custody record. Both token-Vault and
/// custody-root lamports return there. This frame exposes no caller-selected
/// recipient and requires no caller signer.
pub const RETIRE_EMPTY_VAULT_FRAME: [AccountRole; 8] = [
    state(Role::Market, true),
    immutable(Role::Realm),
    state(Role::CollateralCustody, true),
    token_account(Role::CollateralVault),
    AccountRole::new(
        Role::RentRefund,
        AccountClass::SystemAccount,
        false,
        true,
        false,
    ),
    mint(),
    TOKEN_PROGRAM,
    SYSTEM_PROGRAM,
];

/// Return the exact account frame for one semantic instruction tag.
pub const fn instruction_frame(tag: InstructionTag) -> InstructionFrame {
    let roles: &'static [AccountRole] = match tag {
        InstructionTag::CreateRealm => &CREATE_REALM_FRAME,
        InstructionTag::FoundMarketAndFund => &FOUND_MARKET_AND_FUND_FRAME,
        InstructionTag::OpenCollateralVault => &OPEN_COLLATERAL_VAULT_FRAME,
        InstructionTag::CreatePositionAndSplit => &CREATE_POSITION_AND_SPLIT_FRAME,
        InstructionTag::SplitCompleteSet => &SPLIT_COMPLETE_SET_FRAME,
        InstructionTag::MergeCompleteSet => &MERGE_COMPLETE_SET_FRAME,
        InstructionTag::RedeemResolvedOutcome => &REDEEM_RESOLVED_OUTCOME_FRAME,
        InstructionTag::TransferClaims => &TRANSFER_CLAIMS_FRAME,
        InstructionTag::SweepSurplus => &SWEEP_SURPLUS_FRAME,
        InstructionTag::CloseEmptyPosition => &CLOSE_EMPTY_POSITION_FRAME,
        InstructionTag::CompactTerminalMarket => &COMPACT_TERMINAL_MARKET_FRAME,
        InstructionTag::RetireEmptyVault => &RETIRE_EMPTY_VAULT_FRAME,
    };
    InstructionFrame { tag, roles }
}

/// Require an exact account count and all minimum privileges for every role.
///
/// Extra signer or writable privilege is admitted for transaction and CPI
/// composability; the composing adapter must simply never use privilege absent
/// from the semantic role. Executability remains exact. Key, owner, PDA,
/// program-ID, sysvar-ID, data, and prohibited-alias authentication remain SVM
/// adapter obligations described by [`AccountClass`].
pub fn validate_account_frame(tag: InstructionTag, accounts: &[AccountPrivilege]) -> Result<()> {
    let frame = instruction_frame(tag);
    if accounts.len() != frame.roles().len() {
        return Err(Error::AccountCountMismatch);
    }
    for (actual, required) in accounts.iter().zip(frame.roles()) {
        if (required.is_signer() && !actual.is_signer)
            || (required.is_writable() && !actual.is_writable)
            || actual.is_executable != required.is_executable()
        {
            return Err(Error::AccountPrivilegeMismatch);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_core_contract::{ContentId, MarketIdentity};
    use dclutch_realm_contract::{FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1Input};

    fn root() -> MarketRoot {
        let content = |value| ContentId::new([value; 32]).expect("nonzero test content");
        let identity = MarketIdentity::new(
            content(1),
            content(2),
            content(3),
            content(4),
            content(5),
            7,
        );
        MarketRoot::founding(identity, [9; 32]).expect("nonzero test refund")
    }

    fn realm() -> RealmV1 {
        RealmV1::new(RealmV1Input {
            token_program: [1; 32],
            collateral_mint: [2; 32],
            collateral_adapter_release_id: [3; 32],
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("nonzero test Realm")
    }

    fn facts(address: u8, mint: u8, token_owner: u8) -> SweepSurplusTokenAccountFactsV1 {
        SweepSurplusTokenAccountFactsV1::new([address; 32], [mint; 32], [token_owner; 32])
            .expect("nonzero test token facts")
    }

    fn exact_privileges(tag: InstructionTag) -> [AccountPrivilege; 12] {
        let mut output = [AccountPrivilege {
            is_signer: false,
            is_writable: false,
            is_executable: false,
        }; 12];
        for (destination, role) in output.iter_mut().zip(instruction_frame(tag).roles()) {
            *destination = AccountPrivilege {
                is_signer: role.is_signer(),
                is_writable: role.is_writable(),
                is_executable: role.is_executable(),
            };
        }
        output
    }

    #[test]
    fn all_frames_are_exact_and_program_kinds_are_distinct() {
        let tags = [
            InstructionTag::CreateRealm,
            InstructionTag::FoundMarketAndFund,
            InstructionTag::OpenCollateralVault,
            InstructionTag::CreatePositionAndSplit,
            InstructionTag::SplitCompleteSet,
            InstructionTag::MergeCompleteSet,
            InstructionTag::RedeemResolvedOutcome,
            InstructionTag::TransferClaims,
            InstructionTag::SweepSurplus,
            InstructionTag::CloseEmptyPosition,
            InstructionTag::CompactTerminalMarket,
            InstructionTag::RetireEmptyVault,
        ];
        for tag in tags {
            let frame = instruction_frame(tag);
            let privileges = exact_privileges(tag);
            let exact = privileges.get(..frame.roles().len());
            assert!(exact.is_some());
            if let Some(accounts) = exact {
                assert_eq!(validate_account_frame(tag, accounts), Ok(()));
            }
        }
        assert_eq!(
            CREATE_REALM_FRAME.get(3).map(|role| role.class()),
            Some(AccountClass::RealmTokenProgram)
        );
        assert_eq!(
            CREATE_REALM_FRAME.get(4).map(|role| role.class()),
            Some(AccountClass::SystemProgram)
        );
        assert_eq!(
            CREATE_REALM_FRAME.get(5).map(|role| role.class()),
            Some(AccountClass::RentSysvar)
        );
        assert_eq!(
            SWEEP_SURPLUS_FRAME.first().map(|role| role.is_signer()),
            Some(false)
        );
        assert_eq!(
            RETIRE_EMPTY_VAULT_FRAME.first().map(|role| role.role()),
            Some(Role::Market)
        );
        assert_eq!(
            OPEN_COLLATERAL_VAULT_FRAME.get(2).map(|role| role.role()),
            Some(Role::CapabilityReadiness)
        );
        assert_eq!(
            OPEN_COLLATERAL_VAULT_FRAME.get(4).map(|role| role.role()),
            Some(Role::CapabilityManifest)
        );
        assert_eq!(
            OPEN_COLLATERAL_VAULT_FRAME.get(4).map(|role| role.class()),
            Some(AccountClass::ImmutableProtocolRecord)
        );
        assert_eq!(OPEN_COLLATERAL_VAULT_FRAME.len(), 12);
    }

    #[test]
    fn missing_requirements_refuse_but_safe_privilege_escalation_composes() {
        let tag = InstructionTag::CreatePositionAndSplit;
        let frame = instruction_frame(tag);
        let privileges = exact_privileges(tag);
        let exact = privileges
            .get(..frame.roles().len())
            .expect("largest exact frame fits test buffer");
        assert_eq!(
            exact
                .get(..9)
                .map(|short| validate_account_frame(tag, short)),
            Some(Err(Error::AccountCountMismatch))
        );

        for (position, required) in frame.roles().iter().enumerate() {
            let mut changed = [AccountPrivilege {
                is_signer: false,
                is_writable: false,
                is_executable: false,
            }; 12];
            changed.copy_from_slice(exact);
            if required.is_signer() {
                if let Some(account) = changed.get_mut(position) {
                    account.is_signer = false;
                }
                assert_eq!(
                    validate_account_frame(tag, &changed),
                    Err(Error::AccountPrivilegeMismatch)
                );
            }
            changed.copy_from_slice(exact);
            if required.is_writable() {
                if let Some(account) = changed.get_mut(position) {
                    account.is_writable = false;
                }
                assert_eq!(
                    validate_account_frame(tag, &changed),
                    Err(Error::AccountPrivilegeMismatch)
                );
            }
            changed.copy_from_slice(exact);
            if let Some(account) = changed.get_mut(position) {
                account.is_executable = !account.is_executable;
            }
            assert_eq!(
                validate_account_frame(tag, &changed),
                Err(Error::AccountPrivilegeMismatch)
            );
        }

        let mut escalated: [AccountPrivilege; 12] = exact
            .try_into()
            .expect("CreatePositionAndSplit has twelve roles");
        for (actual, required) in escalated.iter_mut().zip(frame.roles()) {
            if !required.is_signer() {
                actual.is_signer = true;
            }
            if !required.is_writable() {
                actual.is_writable = true;
            }
        }
        assert_eq!(validate_account_frame(tag, &escalated), Ok(()));
    }

    #[test]
    fn position_creation_separates_payer_owner_and_permanent_credit() {
        assert_eq!(CREATE_POSITION_AND_SPLIT_FRAME.len(), 12);
        assert_eq!(
            CREATE_POSITION_AND_SPLIT_FRAME
                .first()
                .map(|role| role.role()),
            Some(Role::PositionPayer)
        );
        assert_eq!(
            CREATE_POSITION_AND_SPLIT_FRAME.get(1).map(|role| (
                role.role(),
                role.is_signer(),
                role.is_writable()
            )),
            Some((Role::PositionOwner, true, false))
        );
        assert_eq!(
            CREATE_POSITION_AND_SPLIT_FRAME
                .get(5)
                .map(|role| (role.role(), role.is_writable())),
            Some((Role::RentCredit, false))
        );
    }

    #[test]
    fn empty_position_close_is_permissionless_and_credits_owner_record() {
        assert_eq!(CLOSE_EMPTY_POSITION_FRAME.len(), 3);
        assert_eq!(
            CLOSE_EMPTY_POSITION_FRAME.map(|role| role.role()),
            [Role::Market, Role::Position, Role::RentCredit]
        );
        assert!(
            CLOSE_EMPTY_POSITION_FRAME
                .iter()
                .all(|role| !role.is_signer())
        );
        assert!(
            CLOSE_EMPTY_POSITION_FRAME
                .iter()
                .all(|role| role.is_writable())
        );
    }

    #[test]
    fn sweep_destination_is_fixed_to_market_refund_and_realm_mint() {
        assert_eq!(
            authorize_sweep_surplus_destination(root(), realm(), facts(4, 2, 5), facts(6, 2, 9)),
            Ok(())
        );
    }

    #[test]
    fn sweep_destination_wrong_owner_refuses() {
        assert_eq!(
            authorize_sweep_surplus_destination(root(), realm(), facts(4, 2, 5), facts(6, 2, 8)),
            Err(Error::SweepDestinationOwnerMismatch)
        );
    }

    #[test]
    fn sweep_destination_wrong_mint_refuses() {
        assert_eq!(
            authorize_sweep_surplus_destination(root(), realm(), facts(4, 2, 5), facts(6, 7, 9)),
            Err(Error::SweepDestinationMintMismatch)
        );
    }

    #[test]
    fn sweep_destination_vault_alias_refuses() {
        assert_eq!(
            authorize_sweep_surplus_destination(root(), realm(), facts(4, 2, 5), facts(4, 2, 9)),
            Err(Error::SweepDestinationAliasesVault)
        );
    }

    #[test]
    fn sweep_token_facts_reject_zero_identifiers() {
        assert_eq!(
            SweepSurplusTokenAccountFactsV1::new([0; 32], [2; 32], [9; 32]),
            Err(Error::ZeroIdentifier)
        );
        assert_eq!(
            SweepSurplusTokenAccountFactsV1::new([4; 32], [0; 32], [9; 32]),
            Err(Error::ZeroIdentifier)
        );
        assert_eq!(
            SweepSurplusTokenAccountFactsV1::new([4; 32], [2; 32], [0; 32]),
            Err(Error::ZeroIdentifier)
        );
    }
}
