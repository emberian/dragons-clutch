//! Exact account-role and privilege frames for a future SBF adapter.

use crate::instruction::ActionV1;
use crate::state::validate_width;
use crate::{Error, Result, require_nonzero};

/// SDK-free account metadata projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountMetaV1 {
    /// Account public-key bytes.
    pub key: [u8; 32],
    /// Whether the outer instruction grants signer privilege.
    pub is_signer: bool,
    /// Whether the outer instruction grants writable privilege.
    pub is_writable: bool,
    /// Whether the runtime marks this account executable.
    pub is_executable: bool,
}

/// Canonical semantic role assigned solely by account-list position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountRoleV1 {
    /// Provider-neutral Market account.
    Market,
    /// Direct bearer capability root and controller PDA.
    BearerState,
    /// Immutable capability manifest.
    CapabilityManifest,
    /// Immutable bearer config content record.
    BearerConfig,
    /// Mutable segregated capability funding state.
    FundingState,
    /// Permanent RentCredit or equivalent refund account.
    RentRefund,
    /// Separate activation System payer.
    CreationPayer,
    /// Official SPL Token-2022 program.
    Token2022Program,
    /// Native System program.
    SystemProgram,
    /// Authenticated Rent sysvar.
    RentSysvar,
    /// One canonical outcome Mint.
    ClaimMint,
    /// One native claim Position.
    Position,
    /// Immutable Realm record.
    Realm,
    /// Market collateral-custody root.
    CollateralCustody,
    /// Token-program-owned collateral Vault.
    CollateralVault,
    /// Holder collateral source or destination.
    CollateralAccount,
    /// Holder or Position-owner transaction signer.
    Holder,
    /// Realm-selected collateral token program.
    CollateralTokenProgram,
    /// Realm-selected collateral Mint.
    CollateralMint,
    /// Holder claim-token source or destination.
    ClaimTokenAccount,
}

/// Return the exact account count for one action and categorical width.
pub fn expected_account_count<const N: usize>(action: ActionV1) -> Result<usize> {
    validate_width::<N>()?;
    let count = match action {
        ActionV1::Activate => 10usize.checked_add(N),
        ActionV1::Audit => 3usize.checked_add(N),
        ActionV1::SplitNative | ActionV1::MergeNative | ActionV1::RedeemNative => Some(9),
        ActionV1::Materialize | ActionV1::Dematerialize | ActionV1::Transfer => Some(7),
        ActionV1::SplitBearer | ActionV1::MergeBearer => N
            .checked_mul(2)
            .and_then(|claims| 10usize.checked_add(claims)),
        ActionV1::RedeemBearer => Some(12),
        ActionV1::Retire => 8usize.checked_add(N),
    };
    count.ok_or(Error::ArithmeticOverflow)
}

/// Return the exact role at one canonical account index.
pub fn account_role<const N: usize>(action: ActionV1, index: usize) -> Result<AccountRoleV1> {
    let count = expected_account_count::<N>(action)?;
    if index >= count {
        return Err(Error::InvalidAccountFrame);
    }
    let role = match action {
        ActionV1::Activate => match index {
            0 => AccountRoleV1::Market,
            1 => AccountRoleV1::BearerState,
            2 => AccountRoleV1::CapabilityManifest,
            3 => AccountRoleV1::BearerConfig,
            4 => AccountRoleV1::FundingState,
            5 => AccountRoleV1::RentRefund,
            6 => AccountRoleV1::CreationPayer,
            7 => AccountRoleV1::Token2022Program,
            8 => AccountRoleV1::SystemProgram,
            9 => AccountRoleV1::RentSysvar,
            _ => AccountRoleV1::ClaimMint,
        },
        ActionV1::Audit => match index {
            0 => AccountRoleV1::Market,
            1 => AccountRoleV1::BearerState,
            2 => AccountRoleV1::Token2022Program,
            _ => AccountRoleV1::ClaimMint,
        },
        ActionV1::SplitNative | ActionV1::MergeNative => native_value_role(index),
        ActionV1::RedeemNative => native_value_role(index),
        ActionV1::Materialize | ActionV1::Dematerialize => match index {
            0 => AccountRoleV1::Market,
            1 => AccountRoleV1::BearerState,
            2 => AccountRoleV1::Position,
            3 => AccountRoleV1::ClaimMint,
            4 => AccountRoleV1::ClaimTokenAccount,
            5 => AccountRoleV1::Holder,
            _ => AccountRoleV1::Token2022Program,
        },
        ActionV1::Transfer => match index {
            0 => AccountRoleV1::Market,
            1 => AccountRoleV1::BearerState,
            2 => AccountRoleV1::ClaimMint,
            3 | 4 => AccountRoleV1::ClaimTokenAccount,
            5 => AccountRoleV1::Holder,
            _ => AccountRoleV1::Token2022Program,
        },
        ActionV1::SplitBearer | ActionV1::MergeBearer => match index {
            0 => AccountRoleV1::Market,
            1 => AccountRoleV1::BearerState,
            2 => AccountRoleV1::Realm,
            3 => AccountRoleV1::CollateralCustody,
            4 => AccountRoleV1::CollateralVault,
            5 => AccountRoleV1::CollateralAccount,
            6 => AccountRoleV1::Holder,
            7 => AccountRoleV1::CollateralTokenProgram,
            8 => AccountRoleV1::CollateralMint,
            9 => AccountRoleV1::Token2022Program,
            _ if (index - 10).is_multiple_of(2) => AccountRoleV1::ClaimMint,
            _ => AccountRoleV1::ClaimTokenAccount,
        },
        ActionV1::RedeemBearer => match index {
            0 => AccountRoleV1::Market,
            1 => AccountRoleV1::BearerState,
            2 => AccountRoleV1::Realm,
            3 => AccountRoleV1::CollateralCustody,
            4 => AccountRoleV1::CollateralVault,
            5 => AccountRoleV1::CollateralAccount,
            6 => AccountRoleV1::ClaimMint,
            7 => AccountRoleV1::ClaimTokenAccount,
            8 => AccountRoleV1::Holder,
            9 => AccountRoleV1::CollateralTokenProgram,
            10 => AccountRoleV1::CollateralMint,
            _ => AccountRoleV1::Token2022Program,
        },
        ActionV1::Retire => match index {
            0 => AccountRoleV1::Market,
            1 => AccountRoleV1::BearerState,
            2 => AccountRoleV1::CapabilityManifest,
            3 => AccountRoleV1::BearerConfig,
            4 => AccountRoleV1::RentRefund,
            5 => AccountRoleV1::Token2022Program,
            6 => AccountRoleV1::SystemProgram,
            7 => AccountRoleV1::RentSysvar,
            _ => AccountRoleV1::ClaimMint,
        },
    };
    Ok(role)
}

/// Validate exact role privileges, count, nonzero keys, and alias policy.
///
/// Extra signer or writable privilege is refused. The only admitted alias is
/// between the claim Token-2022 program and the Realm collateral program when
/// that Realm also selected Token-2022.
pub fn validate_account_frame<const N: usize>(
    action: ActionV1,
    accounts: &[AccountMetaV1],
) -> Result<()> {
    if accounts.len() != expected_account_count::<N>(action)? {
        return Err(Error::InvalidAccountFrame);
    }
    for (index, account) in accounts.iter().enumerate() {
        require_nonzero(&account.key)?;
        let role = account_role::<N>(action, index)?;
        let (signer, writable, executable) = exact_privileges(action, role);
        if account.is_signer != signer
            || account.is_writable != writable
            || account.is_executable != executable
        {
            return Err(Error::InvalidAccountFrame);
        }
        for (prior_index, prior) in accounts.iter().take(index).enumerate() {
            if prior.key == account.key {
                let prior_role = account_role::<N>(action, prior_index)?;
                let safe_program_alias = matches!(
                    (prior_role, role),
                    (
                        AccountRoleV1::CollateralTokenProgram,
                        AccountRoleV1::Token2022Program
                    ) | (
                        AccountRoleV1::Token2022Program,
                        AccountRoleV1::CollateralTokenProgram
                    )
                );
                if !safe_program_alias {
                    return Err(Error::AccountAlias);
                }
            }
        }
    }
    Ok(())
}

fn native_value_role(index: usize) -> AccountRoleV1 {
    match index {
        0 => AccountRoleV1::Market,
        1 => AccountRoleV1::Position,
        2 => AccountRoleV1::Realm,
        3 => AccountRoleV1::CollateralCustody,
        4 => AccountRoleV1::CollateralVault,
        5 => AccountRoleV1::CollateralAccount,
        6 => AccountRoleV1::Holder,
        7 => AccountRoleV1::CollateralTokenProgram,
        _ => AccountRoleV1::CollateralMint,
    }
}

fn exact_privileges(action: ActionV1, role: AccountRoleV1) -> (bool, bool, bool) {
    let executable = matches!(
        role,
        AccountRoleV1::Token2022Program
            | AccountRoleV1::CollateralTokenProgram
            | AccountRoleV1::SystemProgram
    );
    let signer = matches!(role, AccountRoleV1::CreationPayer | AccountRoleV1::Holder);
    let writable = match role {
        AccountRoleV1::Market => !matches!(
            action,
            ActionV1::Audit | ActionV1::Materialize | ActionV1::Dematerialize | ActionV1::Transfer
        ),
        AccountRoleV1::BearerState => !matches!(action, ActionV1::Audit | ActionV1::Transfer),
        AccountRoleV1::FundingState
        | AccountRoleV1::CreationPayer
        | AccountRoleV1::Position
        | AccountRoleV1::CollateralVault
        | AccountRoleV1::CollateralAccount
        | AccountRoleV1::ClaimTokenAccount => true,
        AccountRoleV1::ClaimMint => !matches!(action, ActionV1::Audit | ActionV1::Transfer),
        AccountRoleV1::CapabilityManifest
        | AccountRoleV1::BearerConfig
        | AccountRoleV1::RentRefund
        | AccountRoleV1::Token2022Program
        | AccountRoleV1::SystemProgram
        | AccountRoleV1::RentSysvar
        | AccountRoleV1::Realm
        | AccountRoleV1::CollateralCustody
        | AccountRoleV1::Holder
        | AccountRoleV1::CollateralTokenProgram
        | AccountRoleV1::CollateralMint => false,
    };
    (signer, writable, executable)
}
