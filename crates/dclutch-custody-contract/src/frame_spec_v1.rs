//! Canonical physical account frames for Custody V1 requests.
//!
//! This no-allocation contract is the sole owner of the exact account order,
//! privileges, and logical alias roles shared by the Custody SBF adapter and
//! data-defined Trading profiles.

use crate::OperationV1;

/// Exact common Custody prefix width.
pub const CUSTODY_COMMON_ACCOUNT_COUNT_V1: u16 = 9;
/// Exact `InitializeReplay` frame width.
pub const INITIALIZE_REPLAY_ACCOUNT_COUNT_V1: u16 = 12;
/// Exact `OpenVault` frame width.
pub const OPEN_VAULT_ACCOUNT_COUNT_V1: u16 = 16;
/// Exact `Transfer` frame width.
pub const TRANSFER_ACCOUNT_COUNT_V1: u16 = 14;
/// Exact `CloseVault` frame width.
pub const CLOSE_VAULT_ACCOUNT_COUNT_V1: u16 = 14;
/// Exact `CloseReplay` frame width.
pub const CLOSE_REPLAY_ACCOUNT_COUNT_V1: u16 = 10;

/// Stable physical-frame refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustodyFrameSpecErrorV1 {
    /// An account coordinate exceeded the operation-selected frame.
    InvalidCoordinate,
}

/// Exact SVM privilege tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyFramePrivilegesV1 {
    signer: bool,
    writable: bool,
    executable: bool,
}

impl CustodyFramePrivilegesV1 {
    /// Exact signer bit.
    #[must_use]
    pub const fn signer(self) -> bool {
        self.signer
    }

    /// Exact writable bit.
    #[must_use]
    pub const fn writable(self) -> bool {
        self.writable
    }

    /// Exact executable bit.
    #[must_use]
    pub const fn executable(self) -> bool {
        self.executable
    }
}

const READONLY: CustodyFramePrivilegesV1 = privileges(false, false, false);
const SIGNER: CustodyFramePrivilegesV1 = privileges(true, false, false);
const WRITABLE: CustodyFramePrivilegesV1 = privileges(false, true, false);
const SIGNER_WRITABLE: CustodyFramePrivilegesV1 = privileges(true, true, false);
const EXECUTABLE: CustodyFramePrivilegesV1 = privileges(false, false, true);

const fn privileges(signer: bool, writable: bool, executable: bool) -> CustodyFramePrivilegesV1 {
    CustodyFramePrivilegesV1 {
        signer,
        writable,
        executable,
    }
}

/// Canonical logical alias identity at one Custody frame coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustodyFrameRoleV1 {
    /// Release-pinned caller authority.
    CallerAuthority,
    /// Canonical Core Market.
    CoreMarket,
    /// Current release activation cache.
    ActivationCache,
    /// Registry program.
    RegistryProgram,
    /// Selected caller program.
    CallerProgram,
    /// Selected caller ProgramData.
    CallerProgramData,
    /// Finalized Realm record.
    RealmRecord,
    /// Vacant Realm staging cursor.
    RealmStaging,
    /// Per-context Custody replay state.
    Replay,
    /// Transaction payer.
    Payer,
    /// System program.
    SystemProgram,
    /// Rent sysvar.
    RentSysvar,
    /// Realm-selected collateral mint.
    Mint,
    /// Custody token vault.
    Vault,
    /// Custody transfer-authority PDA.
    CustodyAuthority,
    /// Realm-selected token program.
    TokenProgram,
    /// Transfer source token account.
    TransferSource,
    /// Transfer destination token account.
    TransferDestination,
    /// Immutable rent-refund beneficiary.
    RentRefund,
}

/// One canonical Custody frame coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyFrameAccountV1 {
    role: CustodyFrameRoleV1,
    privileges: CustodyFramePrivilegesV1,
}

impl CustodyFrameAccountV1 {
    /// Canonical semantic alias identity.
    #[must_use]
    pub const fn role(self) -> CustodyFrameRoleV1 {
        self.role
    }

    /// Exact SVM privileges.
    #[must_use]
    pub const fn privileges(self) -> CustodyFramePrivilegesV1 {
        self.privileges
    }
}

/// Borrow-free operation-selected Custody frame specification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyFrameSpecV1 {
    operation: OperationV1,
}

impl CustodyFrameSpecV1 {
    /// Select the exact frame for `operation`.
    #[must_use]
    pub const fn new(operation: OperationV1) -> Self {
        Self { operation }
    }

    /// Exact account count.
    #[must_use]
    pub const fn account_count(self) -> u16 {
        match self.operation {
            OperationV1::InitializeReplay => INITIALIZE_REPLAY_ACCOUNT_COUNT_V1,
            OperationV1::OpenVault => OPEN_VAULT_ACCOUNT_COUNT_V1,
            OperationV1::Transfer => TRANSFER_ACCOUNT_COUNT_V1,
            OperationV1::CloseVault => CLOSE_VAULT_ACCOUNT_COUNT_V1,
            OperationV1::CloseReplay => CLOSE_REPLAY_ACCOUNT_COUNT_V1,
        }
    }

    /// Exact account role and privileges at `index`.
    pub fn account(self, index: u16) -> Result<CustodyFrameAccountV1, CustodyFrameSpecErrorV1> {
        if index >= self.account_count() {
            return Err(CustodyFrameSpecErrorV1::InvalidCoordinate);
        }
        if index < CUSTODY_COMMON_ACCOUNT_COUNT_V1 {
            return common(index);
        }
        let account = match (self.operation, index) {
            (OperationV1::InitializeReplay, 9) => {
                account(CustodyFrameRoleV1::Payer, SIGNER_WRITABLE)
            }
            (OperationV1::InitializeReplay, 10) => {
                account(CustodyFrameRoleV1::SystemProgram, EXECUTABLE)
            }
            (OperationV1::InitializeReplay, 11) => {
                account(CustodyFrameRoleV1::RentSysvar, READONLY)
            }
            (OperationV1::OpenVault, 9) => account(CustodyFrameRoleV1::Mint, READONLY),
            (OperationV1::OpenVault, 10) => account(CustodyFrameRoleV1::Vault, WRITABLE),
            (OperationV1::OpenVault, 11) => account(CustodyFrameRoleV1::CustodyAuthority, READONLY),
            (OperationV1::OpenVault, 12) => account(CustodyFrameRoleV1::TokenProgram, EXECUTABLE),
            (OperationV1::OpenVault, 13) => account(CustodyFrameRoleV1::Payer, SIGNER_WRITABLE),
            (OperationV1::OpenVault, 14) => account(CustodyFrameRoleV1::SystemProgram, EXECUTABLE),
            (OperationV1::OpenVault, 15) => account(CustodyFrameRoleV1::RentSysvar, READONLY),
            (OperationV1::Transfer, 9) => account(CustodyFrameRoleV1::Mint, READONLY),
            (OperationV1::Transfer, 10) => account(CustodyFrameRoleV1::TransferSource, WRITABLE),
            (OperationV1::Transfer, 11) => {
                account(CustodyFrameRoleV1::TransferDestination, WRITABLE)
            }
            (OperationV1::Transfer, 12) => account(CustodyFrameRoleV1::CustodyAuthority, READONLY),
            (OperationV1::Transfer, 13) => account(CustodyFrameRoleV1::TokenProgram, EXECUTABLE),
            (OperationV1::CloseVault, 9) => account(CustodyFrameRoleV1::Mint, READONLY),
            (OperationV1::CloseVault, 10) => account(CustodyFrameRoleV1::Vault, WRITABLE),
            (OperationV1::CloseVault, 11) => {
                account(CustodyFrameRoleV1::CustodyAuthority, READONLY)
            }
            (OperationV1::CloseVault, 12) => account(CustodyFrameRoleV1::TokenProgram, EXECUTABLE),
            (OperationV1::CloseVault, 13) | (OperationV1::CloseReplay, 9) => {
                account(CustodyFrameRoleV1::RentRefund, WRITABLE)
            }
            _ => return Err(CustodyFrameSpecErrorV1::InvalidCoordinate),
        };
        Ok(account)
    }
}

fn common(index: u16) -> Result<CustodyFrameAccountV1, CustodyFrameSpecErrorV1> {
    let account = match index {
        0 => account(CustodyFrameRoleV1::CallerAuthority, SIGNER),
        1 => account(CustodyFrameRoleV1::CoreMarket, READONLY),
        2 => account(CustodyFrameRoleV1::ActivationCache, READONLY),
        3 => account(CustodyFrameRoleV1::RegistryProgram, EXECUTABLE),
        4 => account(CustodyFrameRoleV1::CallerProgram, EXECUTABLE),
        5 => account(CustodyFrameRoleV1::CallerProgramData, READONLY),
        6 => account(CustodyFrameRoleV1::RealmRecord, READONLY),
        7 => account(CustodyFrameRoleV1::RealmStaging, READONLY),
        8 => account(CustodyFrameRoleV1::Replay, WRITABLE),
        _ => return Err(CustodyFrameSpecErrorV1::InvalidCoordinate),
    };
    Ok(account)
}

const fn account(
    role: CustodyFrameRoleV1,
    privileges: CustodyFramePrivilegesV1,
) -> CustodyFrameAccountV1 {
    CustodyFrameAccountV1 { role, privileges }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_operation_has_one_exact_total_frame() {
        for (operation, count) in [
            (OperationV1::InitializeReplay, 12),
            (OperationV1::OpenVault, 16),
            (OperationV1::Transfer, 14),
            (OperationV1::CloseVault, 14),
            (OperationV1::CloseReplay, 10),
        ] {
            let spec = CustodyFrameSpecV1::new(operation);
            assert_eq!(spec.account_count(), count);
            for index in 0..count {
                spec.account(index).expect("coordinate");
            }
            assert_eq!(
                spec.account(count),
                Err(CustodyFrameSpecErrorV1::InvalidCoordinate)
            );
        }
    }

    #[test]
    fn operation_specific_privileges_cannot_be_substituted() {
        let initialize = CustodyFrameSpecV1::new(OperationV1::InitializeReplay);
        let transfer = CustodyFrameSpecV1::new(OperationV1::Transfer);
        assert_eq!(
            initialize.account(9).expect("payer").privileges(),
            SIGNER_WRITABLE
        );
        assert_eq!(transfer.account(9).expect("mint").privileges(), READONLY);
        assert_ne!(
            initialize.account(9).expect("payer"),
            transfer.account(9).expect("mint")
        );
    }
}
