//! Stable wrapper refusal surface.

use solana_program_error::ProgramError;

/// Deterministic refusal from the separately deployed wrapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum WrapperError {
    /// Instruction family, version, action, or exact payload was refused.
    Instruction = 1,
    /// Exact account count, order, alias, owner, or privilege was refused.
    Accounts = 2,
    /// Upgradeable-loader deployment evidence did not close.
    Deployment = 3,
    /// Descriptor, basis, Product identity, or PDA did not close.
    Identity = 4,
    /// Current rent or prefund-safe System construction failed.
    Construction = 5,
    /// Base custody CPI failed or its exact successor delta was absent.
    BaseCustody = 6,
    /// Token-2022 parser, CPI, or exact post-delta reconciliation failed.
    Token2022 = 7,
    /// Checked integer arithmetic refused.
    Arithmetic = 8,
    /// Account data could not be borrowed without conflict.
    Borrow = 9,
}

impl From<WrapperError> for ProgramError {
    fn from(value: WrapperError) -> Self {
        Self::Custom(value.code())
    }
}

impl WrapperError {
    const fn code(self) -> u32 {
        match self {
            Self::Instruction => 1,
            Self::Accounts => 2,
            Self::Deployment => 3,
            Self::Identity => 4,
            Self::Construction => 5,
            Self::BaseCustody => 6,
            Self::Token2022 => 7,
            Self::Arithmetic => 8,
            Self::Borrow => 9,
        }
    }
}

/// Wrapper result.
pub type Result<T> = core::result::Result<T, WrapperError>;
