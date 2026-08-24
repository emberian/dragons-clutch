// SPDX-License-Identifier: AGPL-3.0-or-later

//! Single checked entry point for the current General settlement successor.
//!
//! The central dispatcher owns family/profile admission. This module owns the
//! exhaustive hand-off from each admitted settlement action to exactly one
//! account-role decoder and atomic writer. It deliberately excludes the
//! predecessor founder, placement, clearing, and SelectedCandidate surfaces.

use crate::accounts::Outcome;
use crate::error::{ClutchError, Refusal};
use clutch_solana_layout::registry::GeneralV2Action;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::{
    general_v2_account_receipt_v5, general_v2_direct_v5,
    general_v2_exact_index_retirement_v1, general_v2_finalize_owner_v5,
    general_v2_freeze_v5, general_v2_materialize_v5, general_v2_merge_payment_v5,
    general_v2_portfolio_retirement_v5, general_v2_portfolio_v5,
    general_v2_settlement_producer_v5, general_v2_settlement_retirement_v1,
    general_v2_unfilled_release_v1, general_v2_virtual_v5,
};

/// Route one already capability-admitted General settlement action.
///
/// Every callee repeats its exact action and capability check before reading
/// accounts. Keeping this match exhaustive over the promoted subset prevents
/// one action's account list or payload decoder from becoming a fallback for
/// another action at the shared `74/1` family coordinate.
#[inline(never)]
pub(crate) fn process<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    match action {
        GeneralV2Action::FreezeEntitlement => general_v2_materialize_v5::process(
            program_id, accounts, sequence, action, payload,
        ),
        GeneralV2Action::AccountReceiptEnd => general_v2_account_receipt_v5::process(
            program_id, accounts, sequence, action, payload,
        ),
        GeneralV2Action::ConsumeDirectReceiptEggs => general_v2_direct_v5::process(
            program_id, accounts, sequence, action, payload,
        ),
        GeneralV2Action::CloseReceipt
        | GeneralV2Action::CloseReservation
        | GeneralV2Action::ClosePot
        | GeneralV2Action::CloseOwnerSettlementRow
        | GeneralV2Action::CloseOwnerFeeFinalization
        | GeneralV2Action::BeginSettlementRetirement => {
            general_v2_settlement_retirement_v1::process(
                program_id, accounts, sequence, action, payload,
            )
        }
        GeneralV2Action::ConsumeVirtualSplitReceiptEggs
        | GeneralV2Action::ConsumeVirtualMergeReceiptEggs => general_v2_virtual_v5::process(
            program_id, accounts, sequence, action, payload,
        ),
        GeneralV2Action::FinalizeOwnerSettlement => general_v2_finalize_owner_v5::process(
            program_id, accounts, sequence, action, payload,
        ),
        GeneralV2Action::InitializeSettlementRoot => {
            general_v2_settlement_producer_v5::process(
                program_id, accounts, sequence, action, payload,
            )
        }
        GeneralV2Action::FinalizeMergeReceiptPayment => {
            general_v2_merge_payment_v5::process(
                program_id, accounts, sequence, action, payload,
            )
        }
        GeneralV2Action::ReleaseUnfilledReservation => {
            general_v2_unfilled_release_v1::process(
                program_id, accounts, sequence, action, payload,
            )
        }
        GeneralV2Action::ConsumePortfolioPairEggs => general_v2_portfolio_v5::process(
            program_id, accounts, sequence, action, payload,
        ),
        GeneralV2Action::FreezeEpochV5 => general_v2_freeze_v5::process(
            program_id, accounts, sequence, action, payload,
        ),
        GeneralV2Action::RetirePortfolioPairArchives => {
            general_v2_portfolio_retirement_v5::process(
                program_id, accounts, sequence, action, payload,
            )
        }
        GeneralV2Action::RetireExactIndexChildren
        | GeneralV2Action::RetireRetainedFeed
        | GeneralV2Action::CloseIndexedSettlementRoot => {
            general_v2_exact_index_retirement_v1::process(
                program_id, accounts, sequence, action, payload,
            )
        }
        GeneralV2Action::CreateMarket
        | GeneralV2Action::InitEpoch
        | GeneralV2Action::InitOrderPage
        | GeneralV2Action::PlaceOrder
        | GeneralV2Action::CancelOrder
        | GeneralV2Action::FreezeEpoch
        | GeneralV2Action::BeginCandidate
        | GeneralV2Action::WriteCandidateFeed
        | GeneralV2Action::SealCandidate
        | GeneralV2Action::InitClearWork
        | GeneralV2Action::GrowClearWork
        | GeneralV2Action::AdvanceClearOrders
        | GeneralV2Action::AdvanceClearSlices
        | GeneralV2Action::CompleteCandidateVerification
        | GeneralV2Action::FinalizeSelection
        | GeneralV2Action::ExpireCandidate
        | GeneralV2Action::MarkWorkClosed
        | GeneralV2Action::ClaimCandidateBond
        | GeneralV2Action::ClaimCandidateWork
        | GeneralV2Action::CleanupCandidate
        | GeneralV2Action::ClaimSolver
        | GeneralV2Action::CloseCandidateIndexPage
        | GeneralV2Action::ClaimEpochUnused
        | GeneralV2Action::ClosePage
        | GeneralV2Action::CloseCandidate
        | GeneralV2Action::CloseClearWork
        | GeneralV2Action::CloseEpoch
        | GeneralV2Action::ClosePosition
        | GeneralV2Action::TransferPositionAssets => {
            Err(Refusal::Adapter(ClutchError::UnsupportedInstruction))
        }
    }
}
