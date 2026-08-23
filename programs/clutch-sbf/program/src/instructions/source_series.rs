//! SourceSeries 77/v2 SBF entry seam.
//!
//! This module first enforces the frozen payload and account-role contract.
//! Individual runtime actions remain centrally capability-gated; adding this
//! decoder alone never makes a tuple executable.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::source_plane_v3_actions::register_release_from_artifact;
use clutch_solana_layout::registry::SourceSeriesAction;
use clutch_solana_layout::source_series::{
    decode_payload_v2, validate_account_metas_v2, ObservedSourceAccountMetaV2,
    SourceSeriesPayloadV2,
};
use clutch_source_plane_v3::ContentId;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;
use std::vec::Vec;

/// Decode one exact SourceSeries action and enter its bounded implementation.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: SourceSeriesAction,
    payload: &[u8],
) -> Outcome<()> {
    let mut observed = Vec::with_capacity(accounts.len());
    for account in accounts {
        observed.push(ObservedSourceAccountMetaV2 {
            key: account.key.to_bytes(),
            writable: account.is_writable,
            signer: account.is_signer,
        });
    }
    validate_account_metas_v2(action, &observed)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let decoded = decode_payload_v2(action, payload)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    match (action, decoded) {
        (
            SourceSeriesAction::RegisterRelease,
            SourceSeriesPayloadV2::RegisterRelease(intent),
        ) => {
            require(sequence == 0, ClutchError::Replay)?;
            register_release_from_artifact(
                program_id,
                ContentId::from_bytes(intent.source_release_manifest_id),
                &accounts[0],
                &accounts[2],
                &accounts[1],
                &accounts[3],
                &accounts[4],
            )?;
            Ok(())
        }
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}
