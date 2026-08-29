//! Exact bounded-frame geometry for the Fractional V3 child topology.
//!
//! Counts are maximum-distinct lock counts: semantic aliases may reuse a key,
//! but the census fixture makes every permitted role distinct so it cannot
//! understate Solana's account-lock ceiling. Address lookup tables reduce wire
//! bytes only; these counts remain the lock admission fact.

use dclutch_claims_svm::{
    frame_spec_v1::{
        PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V1, SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
    },
    terminal_settlement_v3::TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3,
};
use dclutch_fractional_claim_contract::{
    FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2, FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3,
};

/// One-byte Trading family envelope before the exact child request.
pub const FRACTIONAL_CHILD_ENVELOPE_BYTES_V3: usize = 1;
/// Token-2022 `TransferChecked` data width.
pub const TOKEN_2022_TRANSFER_CHECKED_BYTES: usize = 10;

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
        // SignedDelta frame: fixed 20 + two Positions; selected Token route adds
        // Token program, Mint, holder account, and root controller.
        FractionalFrameKindV3::WrapOrWholeUnwrap => FractionalFrameCensusV3 {
            unique_account_locks: SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 as usize + 2 + 4,
            instruction_data_bytes: request,
            required_signatures: 1,
        },
        FractionalFrameKindV3::DirectTransfer => FractionalFrameCensusV3 {
            unique_account_locks: 5,
            instruction_data_bytes: TOKEN_2022_TRANSFER_CHECKED_BYTES,
            required_signatures: 1,
        },
        // Claims TerminalSettlement owns its exact 36-account frame, including
        // Custody. Token program, Mint, source, and root are additional.
        FractionalFrameKindV3::TerminalRedeemOrZeroBurn => FractionalFrameCensusV3 {
            unique_account_locks: TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 + 4,
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
