//! Exact ordered account-role schemas independent of Solana SDK types.

use crate::{Error, Result, instruction::InstructionTag};

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
    /// Signer capitalizing account rent and resolution liveness.
    Sponsor,
    /// Signer that owns one Position and authorizes token movement.
    PositionOwner,
    /// Immutable reusable collateral Realm PDA.
    Realm,
    /// Mutable Market root and collateral-liability state.
    Market,
    /// Prepaid one-shot resolution Fund direct child.
    ResolutionFund,
    /// Immutable resolution-policy record committed by Market identity.
    ResolutionPolicy,
    /// Immutable capability manifest committed by Market identity.
    CapabilityManifest,
    /// Market's collateral Vault token account.
    CollateralVault,
    /// Program-owned collateral-custody root and rent-refund contract.
    CollateralCustody,
    /// Authenticated recipient of custody-compartment rent principal.
    RentRefund,
    /// Native claims owned by one Market participant.
    Position,
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
pub const FOUND_MARKET_AND_FUND_FRAME: [AccountRole; 8] = [
    sponsor(),
    state(Role::Market, true),
    state(Role::ResolutionFund, true),
    immutable(Role::Realm),
    immutable(Role::ResolutionPolicy),
    immutable(Role::CapabilityManifest),
    SYSTEM_PROGRAM,
    RENT_SYSVAR,
];

/// Exact collateral-Vault initialization and Market-open frame.
pub const OPEN_COLLATERAL_VAULT_FRAME: [AccountRole; 9] = [
    sponsor(),
    state(Role::Market, true),
    immutable(Role::Realm),
    state(Role::CollateralCustody, true),
    token_account(Role::CollateralVault),
    mint(),
    TOKEN_PROGRAM,
    SYSTEM_PROGRAM,
    RENT_SYSVAR,
];

/// Exact Position creation and first complete-set split frame.
pub const CREATE_POSITION_AND_SPLIT_FRAME: [AccountRole; 10] = [
    authority(Role::PositionOwner, true, true),
    state(Role::Market, true),
    immutable(Role::Realm),
    state(Role::Position, true),
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

/// Exact permissionless collateral-surplus sweep frame.
pub const SWEEP_SURPLUS_FRAME: [AccountRole; 6] = [
    state(Role::Market, true),
    immutable(Role::Realm),
    token_account(Role::CollateralVault),
    token_account(Role::CollateralDestination),
    mint(),
    TOKEN_PROGRAM,
];

/// Exact empty-Position retirement frame.
pub const CLOSE_EMPTY_POSITION_FRAME: [AccountRole; 4] = [
    authority(Role::PositionOwner, true, true),
    state(Role::Market, true),
    state(Role::Position, true),
    SYSTEM_PROGRAM,
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
        InstructionTag::SweepSurplus => &SWEEP_SURPLUS_FRAME,
        InstructionTag::CloseEmptyPosition => &CLOSE_EMPTY_POSITION_FRAME,
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

    fn exact_privileges(tag: InstructionTag) -> [AccountPrivilege; 10] {
        let mut output = [AccountPrivilege {
            is_signer: false,
            is_writable: false,
            is_executable: false,
        }; 10];
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
            InstructionTag::SweepSurplus,
            InstructionTag::CloseEmptyPosition,
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
    }

    #[test]
    fn missing_requirements_refuse_but_safe_privilege_escalation_composes() {
        let tag = InstructionTag::CreatePositionAndSplit;
        let frame = instruction_frame(tag);
        let privileges = exact_privileges(tag);
        assert_eq!(frame.roles().len(), privileges.len());
        let exact = &privileges;
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
            }; 10];
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

        let mut escalated = *exact;
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
}
