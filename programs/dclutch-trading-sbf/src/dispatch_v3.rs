//! Capability Program V3 content joins behind the common Trading authority.
//!
//! This module authenticates exact descriptor/config/manifest relationships.
//! It performs no family dispatch and accepts no V1 descriptor in the V3 path.

use dclutch_capability_contract::{CapabilityEntryV1, CapabilityManifestV1};
use dclutch_capability_program_contract::v3::CapabilityProgramV3;
use dclutch_release_set_contract::CapabilityExecutionSelectionV1;
use solana_program::hash::hash;

use crate::{TradingSbfError, dispatch::TradingFamilyContextV1};

/// Authenticate an activation manifest entry and exact fixed V3 descriptor.
pub fn authenticate_activation_program_v3(
    context: TradingFamilyContextV1,
    manifest_bytes: &[u8],
    descriptor_bytes: &[u8],
    config_bytes: &[u8],
) -> Result<CapabilityProgramV3, TradingSbfError> {
    let selection = context.selection();
    if hash(manifest_bytes).to_bytes() != selection.manifest().to_bytes() {
        return Err(TradingSbfError::Content);
    }
    let manifest =
        CapabilityManifestV1::decode(manifest_bytes).map_err(|_| TradingSbfError::Content)?;
    let entry = manifest
        .entry(selection.entry_index())
        .map_err(|_| TradingSbfError::Content)?;
    require_entry_identity(entry, selection)?;
    let descriptor = authenticate_common_content(context, descriptor_bytes, config_bytes)?;
    descriptor
        .validate_selection(selection, entry)
        .map_err(|_| TradingSbfError::Content)?;
    Ok(descriptor)
}

/// Authenticate persisted hot-action content without repeating the manifest.
pub fn authenticate_hot_program_v3(
    context: TradingFamilyContextV1,
    descriptor_bytes: &[u8],
    config_bytes: &[u8],
) -> Result<CapabilityProgramV3, TradingSbfError> {
    let descriptor = authenticate_common_content(context, descriptor_bytes, config_bytes)?;
    descriptor
        .validate_persisted_selection(context.selection())
        .map_err(|_| TradingSbfError::Content)?;
    Ok(descriptor)
}

fn authenticate_common_content(
    context: TradingFamilyContextV1,
    descriptor_bytes: &[u8],
    config_bytes: &[u8],
) -> Result<CapabilityProgramV3, TradingSbfError> {
    let selection = context.selection();
    if hash(descriptor_bytes).to_bytes() != selection.capability_release().to_bytes()
        || hash(config_bytes).to_bytes() != selection.config().to_bytes()
    {
        return Err(TradingSbfError::Content);
    }
    let descriptor =
        CapabilityProgramV3::decode(descriptor_bytes).map_err(|_| TradingSbfError::Content)?;
    if descriptor
        .root_account_bytes()
        .map_err(|_| TradingSbfError::Root)?
        != context.root_account_bytes()
    {
        return Err(TradingSbfError::Root);
    }
    Ok(descriptor)
}

fn require_entry_identity(
    entry: CapabilityEntryV1,
    selection: CapabilityExecutionSelectionV1,
) -> Result<(), TradingSbfError> {
    if entry.kind_id() != selection.kind()
        || entry.release_id() != selection.capability_release()
        || entry.config_id() != selection.config()
    {
        Err(TradingSbfError::Content)
    } else {
        Ok(())
    }
}
