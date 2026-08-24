// SPDX-License-Identifier: AGPL-3.0-or-later

//! Sole current General action-47 owner.
//!
//! The account contract is one atomic Source→Product→General retirement
//! union. It authenticates the full current General V5 graph and the terminal
//! indexed root before the first write, moves the chain-derived Failure tuple
//! through Source and Product, physically retires the General treasury
//! Position between Product's two ordered stages, then consumes the same
//! indexed preauthorization to close the durable b9 pair and indexed root
//! last. No terminal ID, graph, Failure preimage, or caller projection appears
//! in the payload.

use std::boxed::Box;

use clutch_general_v2_contract::{
    decode_exact_index_lifecycle_payload_v1, ExactIndexLifecyclePayloadKindV1,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, SeriesMarketLinkAccountV3,
};
use clutch_solana_layout::registry::GeneralV2Action;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{require, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};

use super::general_market_current_v5::{
    authenticate_general_market_current_v5_for_terminal,
    authenticate_general_product_retirement_preflight_v5,
    GeneralMarketCurrentAccountFrameV5, GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5,
};
use super::general_treasury_position_terminal_v5::
    retire_current_general_treasury_position_into_product_v5;
use super::general_v2_exact_index_retirement_v1::{
    close_indexed_root_after_product_series_retirement_v5,
    preauthenticate_general_indexed_close_v5, AuthenticatedGeneralIndexedCloseV5,
};
use super::product_series_current::retirement_v5::{
    finalize_current_product_series_retirement_v5,
    stage_current_product_series_retirement_v5, ProductRetirementAccountFrameV5,
};
use super::product_source_retirement_outer_v5::{
    compose_product_source_series_retirement_from_chain_v5,
    ProductSourceRetirementDispositionV5,
};

/// Canonical current-General prefix occupies indices 0..25.
pub(crate) const ACTION47_CURRENT_PREFIX_END_V1: usize =
    GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5;

/// Product's exact physical FundingV5 retirement slice.
pub(crate) const ACTION47_PHYSICAL_START_V1: usize = ACTION47_CURRENT_PREFIX_END_V1;
pub(crate) const ACTION47_PHYSICAL_END_V1: usize = ACTION47_PHYSICAL_START_V1 + 24;
pub(crate) const ACTION47_IX_CLAIM_LEDGER_V1: usize = ACTION47_PHYSICAL_END_V1;
pub(crate) const ACTION47_IX_HOARD_V1: usize = 50;
pub(crate) const ACTION47_IX_HOARD_TOKEN_V1: usize = 51;
pub(crate) const ACTION47_IX_HOARD_AUTHORITY_V1: usize = 52;
pub(crate) const ACTION47_IX_FOUNDATION_VAULT_V1: usize = 53;

/// Source-only roles beyond the Product/current shared core.
pub(crate) const ACTION47_IX_LIFECYCLE_REPLAY_V1: usize = 54;
pub(crate) const ACTION47_IX_SOURCE_ADAPTER_PROGRAM_V1: usize = 55;
pub(crate) const ACTION47_IX_SOURCE_ADAPTER_PROGRAMDATA_V1: usize = 56;
pub(crate) const ACTION47_IX_SOURCE_PARSER_PROGRAM_V1: usize = 57;
pub(crate) const ACTION47_IX_SOURCE_PARSER_PROGRAMDATA_V1: usize = 58;
pub(crate) const ACTION47_IX_SOURCE_PARSER_CONFIG_V1: usize = 59;
pub(crate) const ACTION47_IX_SOURCE_SPEC_V1: usize = 60;
pub(crate) const ACTION47_IX_SOURCE_WORK_SCHEDULE_V1: usize = 61;
pub(crate) const ACTION47_IX_SOURCE_CUSTODY_V1: usize = 62;

/// Failure accounts. The first five are shared with Product's finalizer.
pub(crate) const ACTION47_IX_FAILURE_ADMISSION_V1: usize = 63;
pub(crate) const ACTION47_IX_FAILURE_RUNTIME_V1: usize = 64;
pub(crate) const ACTION47_IX_FAILURE_CELL_V1: usize = 65;
pub(crate) const ACTION47_IX_FAILURE_HISTORY_V1: usize = 66;
pub(crate) const ACTION47_IX_FAILURE_REPLAY_V1: usize = 67;
pub(crate) const ACTION47_IX_FAILURE_LIVENESS_POLICY_V1: usize = 68;

/// General indexed-root and durable b9 roles not already in the V5 prefix.
pub(crate) const ACTION47_IX_INDEXED_ROOT_V1: usize = 69;
pub(crate) const ACTION47_IX_EPOCH_V1: usize = 70;
pub(crate) const ACTION47_IX_WINDOW_V1: usize = 71;
pub(crate) const ACTION47_IX_FEE_MANIFEST_V1: usize = 72;
pub(crate) const ACTION47_IX_FEE_TERMINAL_V1: usize = 73;
pub(crate) const ACTION47_IX_INDEXED_ROOT_PAYER_V1: usize = 74;
pub(crate) const ACTION47_IX_MANIFEST_PAYER_V1: usize = 75;
pub(crate) const ACTION47_IX_TERMINAL_PAYER_V1: usize = 76;

/// General treasury Position roles not already in Product/current state.
pub(crate) const ACTION47_IX_TREASURY_SERVICE_LEDGER_V1: usize = 77;
pub(crate) const ACTION47_IX_TREASURY_POSITION_V1: usize = 78;
pub(crate) const ACTION47_IX_TREASURY_REPLAY_V1: usize = 79;
pub(crate) const ACTION47_IX_POSITION_REFUND_V1: usize = 80;
pub(crate) const ACTION47_IX_REPLAY_REFUND_V1: usize = 81;

/// Failed Source retirement appends the one immutable Source terminal owner.
pub(crate) const ACTION47_IX_FAILED_SOURCE_TERMINAL_V1: usize = 82;
pub(crate) const ACTION47_SUCCESSFUL_ACCOUNT_COUNT_V1: usize = 82;
pub(crate) const ACTION47_FAILED_ACCOUNT_COUNT_V1: usize = 83;

const CURRENT_IX_BINDING: usize = 0;
const CURRENT_IX_ROOT: usize = 2;
const CURRENT_IX_LINK: usize = 3;
const CURRENT_IX_FUNDING: usize = 4;
const CURRENT_IX_REGISTRY: usize = 5;
const CURRENT_IX_SOURCE_RELEASE: usize = 10;
const CURRENT_IX_REALM: usize = 13;
const CURRENT_IX_ARTIFACT_START: usize = 16;
const CURRENT_IX_ARTIFACT_END: usize = 25;

const PHYSICAL_IX_LAMPORT_REFUND: usize = ACTION47_PHYSICAL_START_V1 + 2;
const PHYSICAL_IX_NEUTRAL_LAMPORT: usize = ACTION47_PHYSICAL_START_V1 + 3;
const PHYSICAL_IX_REALM: usize = ACTION47_PHYSICAL_START_V1 + 5;
const PHYSICAL_IX_SYSTEM_PROGRAM: usize = ACTION47_PHYSICAL_START_V1 + 11;

fn current_frame<'frame, 'info>(
    accounts: &'frame [AccountInfo<'info>],
) -> GeneralMarketCurrentAccountFrameV5<'frame, 'info> {
    GeneralMarketCurrentAccountFrameV5 {
        market_binding: &accounts[0],
        market_runtime: &accounts[1],
        product_root: &accounts[2],
        series_link: &accounts[3],
        series_funding: &accounts[4],
        series_registry: &accounts[5],
        registry_program: &accounts[6],
        registry_programdata: &accounts[7],
        registry_release_artifact: &accounts[8],
        capability_profile_artifact: &accounts[9],
        source_release: &accounts[10],
        compiler_bundle: &accounts[11],
        market_instance: &accounts[12],
        realm: &accounts[13],
        revenue_record: &accounts[14],
        revenue_policy_preimage: &accounts[15],
        artifacts: &accounts[CURRENT_IX_ARTIFACT_START..CURRENT_IX_ARTIFACT_END],
    }
}

fn product_frame<'frame, 'info>(
    accounts: &'frame [AccountInfo<'info>],
) -> Outcome<ProductRetirementAccountFrameV5<'frame, 'info>> {
    ProductRetirementAccountFrameV5::new(
        &accounts[CURRENT_IX_ROOT],
        &accounts[CURRENT_IX_LINK],
        &accounts[CURRENT_IX_REGISTRY],
        &accounts[CURRENT_IX_FUNDING],
        &accounts[ACTION47_IX_LIFECYCLE_REPLAY_V1],
        &accounts[ACTION47_PHYSICAL_START_V1..ACTION47_PHYSICAL_END_V1],
        &accounts[ACTION47_IX_CLAIM_LEDGER_V1],
        &accounts[ACTION47_IX_HOARD_V1],
        &accounts[ACTION47_IX_HOARD_TOKEN_V1],
        &accounts[ACTION47_IX_HOARD_AUTHORITY_V1],
        &accounts[ACTION47_IX_FOUNDATION_VAULT_V1],
        &accounts[ACTION47_IX_FAILURE_ADMISSION_V1],
        &accounts[ACTION47_IX_FAILURE_RUNTIME_V1],
        &accounts[ACTION47_IX_FAILURE_CELL_V1],
        &accounts[ACTION47_IX_FAILURE_HISTORY_V1],
        &accounts[ACTION47_IX_FAILURE_REPLAY_V1],
    )
}

fn source_retirement<'info, 'account>(
    program_id: &Pubkey,
    accounts: &'account [AccountInfo<'info>],
    registry: &super::product_series_current::AuthenticatedRegistryCapabilityV5,
    disposition: ProductSourceRetirementDispositionV5<'account, 'info>,
) -> Outcome<super::product_source_retirement_outer_v5::AuthenticatedProductSourceRetirementOuterV5>
{
    let source_accounts = [
        accounts[CURRENT_IX_ROOT].clone(),
        accounts[CURRENT_IX_LINK].clone(),
        accounts[CURRENT_IX_FUNDING].clone(),
        accounts[ACTION47_IX_LIFECYCLE_REPLAY_V1].clone(),
        accounts[CURRENT_IX_SOURCE_RELEASE].clone(),
        accounts[ACTION47_IX_SOURCE_ADAPTER_PROGRAM_V1].clone(),
        accounts[ACTION47_IX_SOURCE_ADAPTER_PROGRAMDATA_V1].clone(),
        accounts[ACTION47_IX_SOURCE_PARSER_PROGRAM_V1].clone(),
        accounts[ACTION47_IX_SOURCE_PARSER_PROGRAMDATA_V1].clone(),
        accounts[ACTION47_IX_SOURCE_PARSER_CONFIG_V1].clone(),
        accounts[ACTION47_IX_SOURCE_SPEC_V1].clone(),
        accounts[ACTION47_IX_SOURCE_WORK_SCHEDULE_V1].clone(),
        accounts[ACTION47_IX_SOURCE_CUSTODY_V1].clone(),
        accounts[PHYSICAL_IX_LAMPORT_REFUND].clone(),
        accounts[PHYSICAL_IX_NEUTRAL_LAMPORT].clone(),
        accounts[PHYSICAL_IX_SYSTEM_PROGRAM].clone(),
    ];
    compose_product_source_series_retirement_from_chain_v5(
        program_id,
        &source_accounts,
        &accounts[ACTION47_IX_FAILURE_ADMISSION_V1
            ..=ACTION47_IX_FAILURE_LIVENESS_POLICY_V1],
        registry,
        disposition,
    )
}

fn indexed_accounts<'info>(accounts: &[AccountInfo<'info>]) -> [AccountInfo<'info>; 10] {
    [
        accounts[ACTION47_IX_INDEXED_ROOT_V1].clone(),
        accounts[ACTION47_IX_EPOCH_V1].clone(),
        accounts[ACTION47_IX_WINDOW_V1].clone(),
        accounts[CURRENT_IX_BINDING].clone(),
        accounts[ACTION47_IX_FEE_MANIFEST_V1].clone(),
        accounts[ACTION47_IX_FEE_TERMINAL_V1].clone(),
        accounts[ACTION47_IX_INDEXED_ROOT_PAYER_V1].clone(),
        accounts[ACTION47_IX_MANIFEST_PAYER_V1].clone(),
        accounts[ACTION47_IX_TERMINAL_PAYER_V1].clone(),
        accounts[PHYSICAL_IX_NEUTRAL_LAMPORT].clone(),
    ]
}

#[inline(never)]
fn compose_action47(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: clutch_general_v2_contract::CountedSettlementRootSelectorV1,
) -> Outcome<()> {
    require(
        accounts.len() == ACTION47_SUCCESSFUL_ACCOUNT_COUNT_V1
            || accounts.len() == ACTION47_FAILED_ACCOUNT_COUNT_V1,
        ClutchError::AccountCount,
    )?;
    require(
        accounts[CURRENT_IX_REALM].key == accounts[PHYSICAL_IX_REALM].key,
        ClutchError::MismatchedState,
    )?;

    let product = product_frame(accounts)?;
    let frame = current_frame(accounts);
    let mut root = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let mut link = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let preflight = authenticate_general_product_retirement_preflight_v5(
        program_id,
        &frame,
        &mut root,
        &mut link,
    )?;
    let (current, registry, funding, bundle, artifacts) = preflight.into_parts();
    let indexed = indexed_accounts(accounts);
    let indexed_close = preauthenticate_general_indexed_close_v5(
        program_id,
        &indexed,
        selector,
        &current,
        &accounts[ACTION47_IX_TREASURY_SERVICE_LEDGER_V1],
    )?;

    let disposition = if accounts.len() == ACTION47_SUCCESSFUL_ACCOUNT_COUNT_V1 {
        ProductSourceRetirementDispositionV5::Successful
    } else {
        ProductSourceRetirementDispositionV5::Failed {
            persisted_source_terminal_account:
                &accounts[ACTION47_IX_FAILED_SOURCE_TERMINAL_V1],
        }
    };
    let source = source_retirement(program_id, accounts, &registry, disposition)?;
    let (source, failure_inputs) = source.into_parts();
    let lifecycle = stage_current_product_series_retirement_v5(
        program_id,
        &product,
        source,
        registry,
        funding,
        bundle,
        artifacts,
    )?;
    drop(current);

    let mut terminal_root = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let mut terminal_link = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let current = authenticate_general_market_current_v5_for_terminal(
        program_id,
        &frame,
        &mut terminal_root,
        &mut terminal_link,
    )?;
    let position = retire_current_general_treasury_position_into_product_v5(
        program_id,
        current,
        indexed_close.fee_terminal(),
        &accounts[CURRENT_IX_ROOT],
        &accounts[ACTION47_IX_TREASURY_SERVICE_LEDGER_V1],
        &accounts[ACTION47_IX_TREASURY_POSITION_V1],
        &accounts[ACTION47_IX_TREASURY_REPLAY_V1],
        &accounts[ACTION47_IX_POSITION_REFUND_V1],
        &accounts[ACTION47_IX_REPLAY_REFUND_V1],
        &accounts[PHYSICAL_IX_NEUTRAL_LAMPORT],
    )?;
    let product_terminal = finalize_current_product_series_retirement_v5(
        program_id,
        &product,
        lifecycle,
        position,
        failure_inputs,
    )?;
    close_indexed_root_after_product_series_retirement_v5(
        &indexed,
        indexed_close,
        product_terminal,
    )
}

/// Dispatch-compatible current General action-47 entrypoint.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        action == GeneralV2Action::CloseIndexedSettlementRoot
            && capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    let selector = match decode_exact_index_lifecycle_payload_v1(action.tag(), payload)? {
        ExactIndexLifecyclePayloadKindV1::CloseIndexedRoot(selector) => selector,
        _ => return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
    };
    compose_action47(program_id, accounts, selector)
}
