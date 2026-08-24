// SPDX-License-Identifier: AGPL-3.0-or-later

//! Single checked router for callable current General V5 actions.
//!
//! The central dispatcher admits an exact `74/1/action` tuple before entering
//! this module. Each concrete owner repeats that capability check and decodes
//! its own frozen payload and account contract. Historical General bindings
//! and account-width fallbacks are deliberately absent from this router.

use crate::accounts::Outcome;
use crate::error::ClutchError;
use clutch_solana_layout::registry::GeneralV2Action;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::{
    general_v2_exact_index_retirement_v1, general_v2_fee_retirement_v2,
    general_v2_freeze_v5, general_v2_portfolio_retirement_v5,
    general_v2_settlement_producer_v5, general_v2_settlement_retirement_v1,
};

/// Enter exactly one callable current General action owner.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    match action {
        GeneralV2Action::InitializeSettlementRoot => {
            general_v2_settlement_producer_v5::process(
                program_id, accounts, sequence, action, payload,
            )
        }
        GeneralV2Action::FreezeEpochV5 => general_v2_freeze_v5::process(
            program_id, accounts, sequence, action, payload,
        ),
        GeneralV2Action::RetirePortfolioPairArchives => {
            general_v2_portfolio_retirement_v5::process(
                program_id, accounts, sequence, action, payload,
            )
        }
        GeneralV2Action::RetireExactIndexChildren
        | GeneralV2Action::RetireRetainedFeed => {
            general_v2_exact_index_retirement_v1::process(
                program_id, accounts, sequence, action, payload,
            )
        }
        GeneralV2Action::CloseOwnerSettlementRow
        | GeneralV2Action::CloseOwnerFeeFinalization
        | GeneralV2Action::BeginSettlementRetirement => {
            general_v2_settlement_retirement_v1::process(
                program_id, accounts, sequence, action, payload,
            )
        }
        GeneralV2Action::AdvanceFeeRetirement => general_v2_fee_retirement_v2::process(
            program_id, accounts, sequence, action, payload,
        ),
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}
