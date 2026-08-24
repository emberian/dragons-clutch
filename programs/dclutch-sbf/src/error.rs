//! Stable program-local refusals for the SBF authentication boundary.

use solana_program::program_error::ProgramError;

/// Program-local failures. These codes deliberately avoid reusing external
/// parser errors, which are implementation details of untrusted byte views.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterError {
    /// Instruction bytes were not the owned categorical resolve wire format.
    InvalidInstruction = 0,
    /// The account list did not have the exact role count for its instruction.
    AccountFrameLength = 1,
    /// An account role had an unexpected signer, writable, or executable bit.
    AccountPrivilege = 2,
    /// A required fixed program, owner, or account key did not match.
    AccountIdentity = 3,
    /// An untrusted persisted account failed its canonical decoder.
    AccountData = 4,
    /// A content hash did not match the immutable identity that names it.
    ContentIdentity = 5,
    /// An instruction replay fact did not match immutable Market state.
    ReplayMismatch = 6,
    /// The immutable Fund account was below its exact required balance.
    FundUnderfunded = 7,
    /// The policy selected no authenticated release in this build.
    ReleaseUnavailable = 8,
    /// A release-bound provider account or ABI fact did not authenticate.
    ProviderAuthentication = 9,
    /// The request is authenticated but state mutation is not implemented yet.
    MutationNotImplemented = 10,
}

impl From<AdapterError> for ProgramError {
    fn from(error: AdapterError) -> Self {
        Self::Custom(error as u32)
    }
}
