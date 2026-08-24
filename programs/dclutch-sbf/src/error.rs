//! Stable program-local refusals for the dClutch SBF adapter.

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
    /// The authenticated observation did not satisfy the immutable kernel policy.
    KernelResolution = 10,
    /// The canonical Market/root/ledger/receipt transition refused.
    MarketTransition = 11,
    /// The receiver `post_update` CPI refused.
    ProviderPostCpi = 12,
    /// Receiver state or exact lamport deltas after `post_update` were wrong.
    ProviderPostcondition = 13,
    /// The receiver `reclaim_rent` CPI refused.
    ProviderReclaimCpi = 14,
    /// The temporary update was not closed and refunded exactly.
    ProviderReclaimPostcondition = 15,
    /// Checked adapter arithmetic left the exact `u64` or allocation domain.
    Arithmetic = 16,
    /// The Fund could not be distributed and closed canonically.
    FundClose = 17,
    /// Realm account, mint, token program, rent, or release facts did not authenticate.
    RealmAuthentication = 18,
    /// The System Program refused exact Realm PDA creation.
    RealmCreateCpi = 19,
    /// Realm creation or persistence did not produce exact postconditions.
    RealmPostcondition = 20,
    /// Immutable founding records, identities, or prepaid balances did not authenticate.
    FoundingAuthentication = 21,
    /// The System Program refused exact Market PDA creation.
    MarketCreateCpi = 22,
    /// The System Program refused exact resolution-Fund PDA creation.
    FundCreateCpi = 23,
    /// Founding did not persist exact Market, Fund, owner, rent, and debit postconditions.
    FoundingPostcondition = 24,
}

impl From<AdapterError> for ProgramError {
    fn from(error: AdapterError) -> Self {
        Self::Custom(error as u32)
    }
}
