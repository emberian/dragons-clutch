//! Exact bounded-frame geometry for the Fractional V3 child topology.
//!
//! Counts are maximum-distinct lock counts: semantic aliases may reuse a key,
//! but the census fixture makes every permitted role distinct so it cannot
//! understate Solana's account-lock ceiling. Address lookup tables reduce wire
//! bytes only; these counts remain the lock admission fact.

use dclutch_claims_svm::frame_spec_v1::PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V1;
use dclutch_fractional_claim_contract::{
    FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3, FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2,
    FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3, FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3,
};

/// One-byte Trading family envelope before the exact child request.
pub const FRACTIONAL_CHILD_ENVELOPE_BYTES_V3: usize = 1;
/// Token-2022 `TransferChecked` data width.
pub const TOKEN_2022_TRANSFER_CHECKED_BYTES: usize = 10;
/// Current devnet transaction account-lock ceiling.
pub const FRACTIONAL_DEVNET_MAX_ACCOUNT_LOCKS_V3: usize = 64;

/// Exact bounded outer frame kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalFrameKindV3 {
    /// Two-position Claims SignedDelta plus one selected Mint/holder route.
    WrapOrWholeUnwrap,
    /// Holder-signed direct Token-2022 transfer.
    DirectTransfer,
    /// Claims terminal-settlement frame plus selected Mint/source Token route.
    TerminalRedeemOrZeroBurn,
    /// Permissionless terminal-state binding.
    Terminalize,
    /// Create the ordered retirement cursor.
    RetirementBegin,
    /// Close one exact next Mint and empty Claims reserve.
    RetirementCoordinate,
    /// Fixed-account close after `next_coordinate == K`.
    RetirementFinish,
}

/// Exact maximum-distinct packet geometry before compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalFrameCensusV3 {
    /// Unique account locks including the invoked outer program.
    pub unique_account_locks: usize,
    /// Exact instruction-data width.
    pub instruction_data_bytes: usize,
    /// Required transaction signatures; the permissionless payer is still one signer.
    pub required_signatures: usize,
}

/// Return the frozen V3 maximum-distinct frame census.
pub const fn fractional_frame_census_v3(kind: FractionalFrameKindV3) -> FractionalFrameCensusV3 {
    let request = FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2 + FRACTIONAL_CHILD_ENVELOPE_BYTES_V3;
    let retirement = FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3 + FRACTIONAL_CHILD_ENVELOPE_BYTES_V3;
    match kind {
        // Exact production Claims child: SignedDelta fixed 20 + two Positions,
        // terms raw/staging, TokenBehavior raw/staging, root, holder signer,
        // selected Mint, holder Token account, and Token-2022 program.
        FractionalFrameKindV3::WrapOrWholeUnwrap => FractionalFrameCensusV3 {
            unique_account_locks: FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3,
            instruction_data_bytes: request,
            required_signatures: 1,
        },
        FractionalFrameKindV3::DirectTransfer => FractionalFrameCensusV3 {
            unique_account_locks: 5,
            instruction_data_bytes: TOKEN_2022_TRANSFER_CHECKED_BYTES,
            required_signatures: 1,
        },
        // Exact production terminal child: Claims/Custody fixed 36 plus
        // terms raw/staging, TokenBehavior raw/staging, root, holder signer,
        // selected Mint, and source Token account.
        FractionalFrameKindV3::TerminalRedeemOrZeroBurn => FractionalFrameCensusV3 {
            unique_account_locks: FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3,
            instruction_data_bytes: request,
            required_signatures: 1,
        },
        FractionalFrameKindV3::Terminalize => FractionalFrameCensusV3 {
            unique_account_locks: 18,
            instruction_data_bytes: request,
            required_signatures: 1,
        },
        FractionalFrameKindV3::RetirementBegin => FractionalFrameCensusV3 {
            unique_account_locks: 8,
            instruction_data_bytes: retirement,
            required_signatures: 1,
        },
        // Claims ProtocolPositionClose owns 15 accounts; the outer route adds
        // cursor, terms, Token program, selected Mint, root, and RentCredit.
        FractionalFrameKindV3::RetirementCoordinate => FractionalFrameCensusV3 {
            unique_account_locks: PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V1 as usize + 6,
            instruction_data_bytes: retirement,
            required_signatures: 1,
        },
        FractionalFrameKindV3::RetirementFinish => FractionalFrameCensusV3 {
            unique_account_locks: 10,
            instruction_data_bytes: retirement,
            required_signatures: 1,
        },
    }
}

/// Refuse a frame that crosses the current devnet 64-lock admission wall.
pub const fn admit_fractional_devnet_locks_v3(unique_account_locks: usize) -> bool {
    unique_account_locks != 0 && unique_account_locks <= FRACTIONAL_DEVNET_MAX_ACCOUNT_LOCKS_V3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrected_atomic_frame_and_runtime_boundary_are_exact() {
        let atomic = fractional_frame_census_v3(FractionalFrameKindV3::WrapOrWholeUnwrap);
        assert_eq!(atomic.unique_account_locks, 31);
        assert!(admit_fractional_devnet_locks_v3(
            atomic.unique_account_locks
        ));
        let terminal = fractional_frame_census_v3(FractionalFrameKindV3::TerminalRedeemOrZeroBurn);
        assert_eq!(terminal.unique_account_locks, 44);
        assert!(admit_fractional_devnet_locks_v3(
            terminal.unique_account_locks
        ));
        assert!(admit_fractional_devnet_locks_v3(64));
        assert!(!admit_fractional_devnet_locks_v3(65));
        assert!(!admit_fractional_devnet_locks_v3(0));
    }
}
