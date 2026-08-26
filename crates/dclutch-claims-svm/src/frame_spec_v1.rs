//! Canonical physical account frames for Claims child requests.
//!
//! This no-alloc contract is the sole owner of account order and privilege
//! facts shared by the Claims SBF adapter and data-defined Trading profiles.
//! Roles are semantic alias identities: equal roles may reuse one physical
//! account, while distinct roles must remain distinct unless a higher-level
//! profile proves another explicit relation.

use crate::protocol_position_v2::ProtocolPositionActionV2;

/// Exact ProtocolPosition Admit frame width.
pub const PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V1: u16 = 26;
/// Exact ProtocolPosition Close frame width.
pub const PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V1: u16 = 15;
/// Exact affine frame width before its runtime Position table.
pub const AFFINE_FIXED_ACCOUNT_COUNT_V1: u16 = 20;
/// Exact sparse native-transfer frame width.
pub const SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1: u16 = 22;

/// Stable frame-spec refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameSpecErrorV1 {
    /// A zero/overflowing runtime Position count was supplied.
    InvalidPositionCount,
    /// An account coordinate exceeded the selected exact frame.
    InvalidCoordinate,
}

/// Exact SVM privilege tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FramePrivilegesV1 {
    signer: bool,
    writable: bool,
    executable: bool,
}

impl FramePrivilegesV1 {
    /// Construct one exact privilege tuple.
    #[must_use]
    pub const fn new(signer: bool, writable: bool, executable: bool) -> Self {
        Self {
            signer,
            writable,
            executable,
        }
    }

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

const READONLY: FramePrivilegesV1 = FramePrivilegesV1::new(false, false, false);
const SIGNER: FramePrivilegesV1 = FramePrivilegesV1::new(true, false, false);
const WRITABLE: FramePrivilegesV1 = FramePrivilegesV1::new(false, true, false);
const EXECUTABLE: FramePrivilegesV1 = FramePrivilegesV1::new(false, false, true);

/// Canonical logical alias identity for a Claims frame coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimsFrameRoleV1 {
    /// Release-pinned Trading caller authority.
    CallerAuthority,
    /// Claims aggregate Market state.
    ClaimsMarket,
    /// Protocol-owned settlement Position selected by its request.
    ProtocolPosition,
    /// Admission record paired with the protocol Position.
    ProtocolPositionAdmission,
    /// Finalized linked-basis record.
    BasisRecord,
    /// Vacant linked-basis staging cursor.
    BasisStaging,
    /// Finalized Product record.
    ProductRecord,
    /// Vacant Product staging cursor.
    ProductStaging,
    /// Finalized result-domain record.
    ResultDomainRecord,
    /// Vacant result-domain staging cursor.
    ResultDomainStaging,
    /// Finalized portfolio record.
    PortfolioRecord,
    /// Vacant portfolio staging cursor.
    PortfolioStaging,
    /// Rent sysvar.
    RentSysvar,
    /// System program.
    SystemProgram,
    /// Core Market state.
    CoreMarket,
    /// Registry activation cache.
    ActivationCache,
    /// Registry program.
    RegistryProgram,
    /// Trading program.
    TradingProgram,
    /// Trading ProgramData.
    TradingProgramData,
    /// Claims program.
    ClaimsProgram,
    /// Claims ProgramData.
    ClaimsProgramData,
    /// Core program.
    CoreProgram,
    /// Core ProgramData.
    CoreProgramData,
    /// Immutable protocol-position owner identity.
    PositionOwnerIdentity,
    /// Claims Position RentCredit.
    RentCredit,
    /// Rent program.
    RentProgram,
    /// Runtime sorted affine Position-table entry.
    AffinePosition(u16),
    /// Sparse-transfer source Position.
    SparseSourcePosition,
    /// Sparse-transfer destination Position.
    SparseDestinationPosition,
}

/// One canonical frame coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsFrameAccountV1 {
    role: ClaimsFrameRoleV1,
    privileges: FramePrivilegesV1,
}

impl ClaimsFrameAccountV1 {
    /// Canonical semantic alias identity.
    #[must_use]
    pub const fn role(self) -> ClaimsFrameRoleV1 {
        self.role
    }

    /// Exact SVM privileges.
    #[must_use]
    pub const fn privileges(self) -> FramePrivilegesV1 {
        self.privileges
    }
}

/// Action-selected Claims frame kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimsFrameKindV1 {
    /// Admit one protocol Position and admission record.
    ProtocolPositionAdmit,
    /// Close one empty protocol Position and admission record.
    ProtocolPositionClose,
    /// Mutate one runtime-width affine Position table.
    Affine {
        /// Exact nonzero runtime Position-table width.
        position_count: u16,
    },
}

/// Borrow-free exact Claims frame specification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsFrameSpecV1 {
    kind: ClaimsFrameKindV1,
}

/// Borrow-free exact sparse native-transfer frame specification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SparseNativeTransferFrameSpecV1;

impl SparseNativeTransferFrameSpecV1 {
    /// Exact account count.
    #[must_use]
    pub const fn account_count(self) -> u16 {
        SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1
    }

    /// Exact account role and privileges at `index`.
    pub fn account(self, index: u16) -> Result<ClaimsFrameAccountV1, FrameSpecErrorV1> {
        if index >= self.account_count() {
            return Err(FrameSpecErrorV1::InvalidCoordinate);
        }
        sparse_native_transfer(index)
    }
}

impl ClaimsFrameSpecV1 {
    /// Select the exact ProtocolPosition frame.
    #[must_use]
    pub const fn protocol_position(action: ProtocolPositionActionV2) -> Self {
        Self {
            kind: match action {
                ProtocolPositionActionV2::Admit => ClaimsFrameKindV1::ProtocolPositionAdmit,
                ProtocolPositionActionV2::Close => ClaimsFrameKindV1::ProtocolPositionClose,
            },
        }
    }

    /// Select an affine frame with one exact nonzero Position count.
    pub fn affine(position_count: u32) -> Result<Self, FrameSpecErrorV1> {
        let position_count =
            u16::try_from(position_count).map_err(|_| FrameSpecErrorV1::InvalidPositionCount)?;
        if position_count == 0 {
            return Err(FrameSpecErrorV1::InvalidPositionCount);
        }
        Ok(Self {
            kind: ClaimsFrameKindV1::Affine { position_count },
        })
    }

    /// Exact account count.
    pub fn account_count(self) -> Result<u16, FrameSpecErrorV1> {
        match self.kind {
            ClaimsFrameKindV1::ProtocolPositionAdmit => {
                Ok(PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V1)
            }
            ClaimsFrameKindV1::ProtocolPositionClose => {
                Ok(PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V1)
            }
            ClaimsFrameKindV1::Affine { position_count } => AFFINE_FIXED_ACCOUNT_COUNT_V1
                .checked_add(position_count)
                .ok_or(FrameSpecErrorV1::InvalidPositionCount),
        }
    }

    /// Exact account role and privileges at `index`.
    pub fn account(self, index: u16) -> Result<ClaimsFrameAccountV1, FrameSpecErrorV1> {
        if index >= self.account_count()? {
            return Err(FrameSpecErrorV1::InvalidCoordinate);
        }
        match self.kind {
            ClaimsFrameKindV1::ProtocolPositionAdmit => protocol_position_admit(index),
            ClaimsFrameKindV1::ProtocolPositionClose => protocol_position_close(index),
            ClaimsFrameKindV1::Affine { .. } => affine(index),
        }
    }
}

fn account(role: ClaimsFrameRoleV1, privileges: FramePrivilegesV1) -> ClaimsFrameAccountV1 {
    ClaimsFrameAccountV1 { role, privileges }
}

fn protocol_position_admit(index: u16) -> Result<ClaimsFrameAccountV1, FrameSpecErrorV1> {
    let role = protocol_position_common_role(index).or(match index {
        4 => Some(ClaimsFrameRoleV1::BasisRecord),
        5 => Some(ClaimsFrameRoleV1::BasisStaging),
        6 => Some(ClaimsFrameRoleV1::ProductRecord),
        7 => Some(ClaimsFrameRoleV1::ProductStaging),
        8 => Some(ClaimsFrameRoleV1::ResultDomainRecord),
        9 => Some(ClaimsFrameRoleV1::ResultDomainStaging),
        10 => Some(ClaimsFrameRoleV1::PortfolioRecord),
        11 => Some(ClaimsFrameRoleV1::PortfolioStaging),
        12 => Some(ClaimsFrameRoleV1::RentSysvar),
        13 => Some(ClaimsFrameRoleV1::SystemProgram),
        14 => Some(ClaimsFrameRoleV1::CoreMarket),
        15 => Some(ClaimsFrameRoleV1::ActivationCache),
        16 => Some(ClaimsFrameRoleV1::RegistryProgram),
        17 => Some(ClaimsFrameRoleV1::TradingProgram),
        18 => Some(ClaimsFrameRoleV1::TradingProgramData),
        19 => Some(ClaimsFrameRoleV1::ClaimsProgram),
        20 => Some(ClaimsFrameRoleV1::ClaimsProgramData),
        21 => Some(ClaimsFrameRoleV1::CoreProgram),
        22 => Some(ClaimsFrameRoleV1::CoreProgramData),
        23 => Some(ClaimsFrameRoleV1::PositionOwnerIdentity),
        24 => Some(ClaimsFrameRoleV1::RentCredit),
        25 => Some(ClaimsFrameRoleV1::RentProgram),
        _ => None,
    });
    let role = role.ok_or(FrameSpecErrorV1::InvalidCoordinate)?;
    let privileges = match index {
        0 => SIGNER,
        2 | 3 => WRITABLE,
        13 | 16 | 17 | 19 | 21 | 25 => EXECUTABLE,
        _ => READONLY,
    };
    Ok(account(role, privileges))
}

fn protocol_position_close(index: u16) -> Result<ClaimsFrameAccountV1, FrameSpecErrorV1> {
    let role = protocol_position_common_role(index).or(match index {
        4 => Some(ClaimsFrameRoleV1::RentSysvar),
        5 => Some(ClaimsFrameRoleV1::SystemProgram),
        6 => Some(ClaimsFrameRoleV1::ActivationCache),
        7 => Some(ClaimsFrameRoleV1::RegistryProgram),
        8 => Some(ClaimsFrameRoleV1::TradingProgram),
        9 => Some(ClaimsFrameRoleV1::TradingProgramData),
        10 => Some(ClaimsFrameRoleV1::ClaimsProgram),
        11 => Some(ClaimsFrameRoleV1::ClaimsProgramData),
        12 => Some(ClaimsFrameRoleV1::PositionOwnerIdentity),
        13 => Some(ClaimsFrameRoleV1::RentCredit),
        14 => Some(ClaimsFrameRoleV1::RentProgram),
        _ => None,
    });
    let role = role.ok_or(FrameSpecErrorV1::InvalidCoordinate)?;
    let privileges = match index {
        0 => SIGNER,
        2 | 3 | 13 => WRITABLE,
        5 | 7 | 8 | 10 | 14 => EXECUTABLE,
        _ => READONLY,
    };
    Ok(account(role, privileges))
}

const fn protocol_position_common_role(index: u16) -> Option<ClaimsFrameRoleV1> {
    match index {
        0 => Some(ClaimsFrameRoleV1::CallerAuthority),
        1 => Some(ClaimsFrameRoleV1::ClaimsMarket),
        2 => Some(ClaimsFrameRoleV1::ProtocolPosition),
        3 => Some(ClaimsFrameRoleV1::ProtocolPositionAdmission),
        _ => None,
    }
}

fn affine(index: u16) -> Result<ClaimsFrameAccountV1, FrameSpecErrorV1> {
    let role = match index {
        0 => ClaimsFrameRoleV1::CallerAuthority,
        1 => ClaimsFrameRoleV1::ClaimsMarket,
        2 => ClaimsFrameRoleV1::BasisRecord,
        3 => ClaimsFrameRoleV1::BasisStaging,
        4 => ClaimsFrameRoleV1::ProductRecord,
        5 => ClaimsFrameRoleV1::ProductStaging,
        6 => ClaimsFrameRoleV1::ResultDomainRecord,
        7 => ClaimsFrameRoleV1::ResultDomainStaging,
        8 => ClaimsFrameRoleV1::PortfolioRecord,
        9 => ClaimsFrameRoleV1::PortfolioStaging,
        10 => ClaimsFrameRoleV1::RentSysvar,
        11 => ClaimsFrameRoleV1::CoreMarket,
        12 => ClaimsFrameRoleV1::ActivationCache,
        13 => ClaimsFrameRoleV1::RegistryProgram,
        14 => ClaimsFrameRoleV1::TradingProgram,
        15 => ClaimsFrameRoleV1::TradingProgramData,
        16 => ClaimsFrameRoleV1::ClaimsProgram,
        17 => ClaimsFrameRoleV1::ClaimsProgramData,
        18 => ClaimsFrameRoleV1::CoreProgram,
        19 => ClaimsFrameRoleV1::CoreProgramData,
        position => ClaimsFrameRoleV1::AffinePosition(
            position
                .checked_sub(AFFINE_FIXED_ACCOUNT_COUNT_V1)
                .ok_or(FrameSpecErrorV1::InvalidCoordinate)?,
        ),
    };
    let privileges = match index {
        0 => SIGNER,
        1 => WRITABLE,
        13 | 14 | 16 | 18 => EXECUTABLE,
        position if position >= AFFINE_FIXED_ACCOUNT_COUNT_V1 => WRITABLE,
        _ => READONLY,
    };
    Ok(account(role, privileges))
}

fn sparse_native_transfer(index: u16) -> Result<ClaimsFrameAccountV1, FrameSpecErrorV1> {
    let role = match index {
        0 => ClaimsFrameRoleV1::CallerAuthority,
        1 => ClaimsFrameRoleV1::ClaimsMarket,
        2 => ClaimsFrameRoleV1::BasisRecord,
        3 => ClaimsFrameRoleV1::BasisStaging,
        4 => ClaimsFrameRoleV1::ProductRecord,
        5 => ClaimsFrameRoleV1::ProductStaging,
        6 => ClaimsFrameRoleV1::ResultDomainRecord,
        7 => ClaimsFrameRoleV1::ResultDomainStaging,
        8 => ClaimsFrameRoleV1::PortfolioRecord,
        9 => ClaimsFrameRoleV1::PortfolioStaging,
        10 => ClaimsFrameRoleV1::RentSysvar,
        11 => ClaimsFrameRoleV1::CoreMarket,
        12 => ClaimsFrameRoleV1::ActivationCache,
        13 => ClaimsFrameRoleV1::RegistryProgram,
        14 => ClaimsFrameRoleV1::TradingProgram,
        15 => ClaimsFrameRoleV1::TradingProgramData,
        16 => ClaimsFrameRoleV1::ClaimsProgram,
        17 => ClaimsFrameRoleV1::ClaimsProgramData,
        18 => ClaimsFrameRoleV1::CoreProgram,
        19 => ClaimsFrameRoleV1::CoreProgramData,
        20 => ClaimsFrameRoleV1::SparseSourcePosition,
        21 => ClaimsFrameRoleV1::SparseDestinationPosition,
        _ => return Err(FrameSpecErrorV1::InvalidCoordinate),
    };
    let privileges = match index {
        0 => SIGNER,
        1 | 20 | 21 => WRITABLE,
        13 | 14 | 16 | 18 => EXECUTABLE,
        _ => READONLY,
    };
    Ok(account(role, privileges))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_frames_are_exact_and_runtime_affine_is_unbounded_by_profile()
    -> Result<(), FrameSpecErrorV1> {
        let admit = ClaimsFrameSpecV1::protocol_position(ProtocolPositionActionV2::Admit);
        let close = ClaimsFrameSpecV1::protocol_position(ProtocolPositionActionV2::Close);
        assert_eq!(admit.account_count(), Ok(26));
        assert_eq!(close.account_count(), Ok(15));
        assert_eq!(ClaimsFrameSpecV1::affine(258)?.account_count(), Ok(278));
        Ok(())
    }

    #[test]
    fn every_coordinate_has_one_exact_role_and_privilege_tuple() {
        for spec in [
            ClaimsFrameSpecV1::protocol_position(ProtocolPositionActionV2::Admit),
            ClaimsFrameSpecV1::protocol_position(ProtocolPositionActionV2::Close),
            ClaimsFrameSpecV1::affine(2).expect("affine"),
        ] {
            let count = spec.account_count().expect("count");
            for index in 0..count {
                spec.account(index).expect("coordinate");
            }
            assert_eq!(
                spec.account(count),
                Err(FrameSpecErrorV1::InvalidCoordinate)
            );
        }
    }

    #[test]
    fn sparse_native_transfer_frame_is_exact_and_distinguishes_positions() {
        let spec = SparseNativeTransferFrameSpecV1;
        assert_eq!(spec.account_count(), 22);
        for index in 0..spec.account_count() {
            spec.account(index).expect("sparse coordinate");
        }
        assert_eq!(
            spec.account(spec.account_count()),
            Err(FrameSpecErrorV1::InvalidCoordinate)
        );
        assert_eq!(
            spec.account(20).expect("source").role(),
            ClaimsFrameRoleV1::SparseSourcePosition
        );
        assert_eq!(
            spec.account(21).expect("destination").role(),
            ClaimsFrameRoleV1::SparseDestinationPosition
        );
        assert_ne!(
            spec.account(20).expect("source"),
            spec.account(21).expect("destination")
        );
    }
}
