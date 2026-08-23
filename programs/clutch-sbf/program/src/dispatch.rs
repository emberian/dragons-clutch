//! Instruction routing.
//!
//! This module is a router and nothing else.  It decodes the reference request
//! envelope, matches the action tag, and hands the already-routed request to
//! exactly one instruction-family module.  It performs no account validation,
//! reads no account data, and writes nothing: every check lives either in
//! [`crate::accounts`] or in the family module that owns the instruction.
//!
//! ## Why the request is decoded first
//!
//! The bring-up program had one instruction, so it could authenticate its fixed
//! nine-account list before looking at the instruction data at all.  A program
//! with an instruction *set* cannot: how many accounts an instruction takes,
//! which are writable, and what each one must be is a function of which
//! instruction it is, and that fact lives in the data.  So the envelope is
//! decoded here, before any account is touched.
//!
//! The consequence is named rather than hidden.  For a request that decodes,
//! nothing changed: the family module runs the same checks in the same order
//! and produces the same refusal codes.  For a request that does *not* decode
//! and is also presented with bad accounts, the codec refusal now wins where
//! the account refusal used to.  Both are refusals, no state is read or written
//! in either case, and the SVM differential is unaffected — but it is an
//! ordering change, so it is written down here and in
//! `docs/implementation/SBF_BRINGUP.md` instead of being discovered later.
//!
//! ## Refusal discipline for families that are not written yet
//!
//! A family module refuses with [`ClutchError::NotYetImplemented`] unless the
//! offline reference adapter refuses the same action for a *stronger,
//! structural* reason, in which case this program mirrors that reason exactly.
//! Historically that was `CreateMarket` refusing
//! [`ClutchError::AuthorizationUnavailable`] before an authority model existed,
//! and `Resolve`/`RedeemInternal` refusing
//! [`ClutchError::ResolutionEvidenceUnavailable`] before the evidence plane
//! landed. `SettlePage` now admits only the narrow preselected, pre-entitled
//! direct full-slice subset recorded in
//! [`crate::instructions::orders_batch`]; every broader settlement shape
//! refuses. Nothing anywhere returns success it did not earn.

use crate::accounts::Outcome;
use crate::capabilities;
use crate::error::ClutchError;
#[cfg(any(
    feature = "profile-full",
    feature = "profile-direct-v3-source-v2-point"
))]
use crate::instructions::direct_selection_v3;
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
use crate::instructions::general_v2_identity;
use crate::instructions::{
    artifact, cash_exit, external_exit, genesis, market_init, merge_materialize, observe_resolve,
    orders_batch, source_ingest_v2, split,
};
#[cfg(feature = "profile-full")]
use crate::instructions::{direct_selection, resolution_work, source_ingest};
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
use clutch_solana_layout::registry::ExtensionAction;
use clutch_solana_layout::Intent;
#[cfg(any(
    feature = "profile-full",
    feature = "profile-direct-v3-source-v2-point"
))]
use clutch_solana_reference::DirectV3Request;
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
use clutch_solana_reference::ExtensionRequest;
use clutch_solana_reference::{Action, Request};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// A non-authoritative hint selecting one bounded decode-and-call frame.
///
/// The wire bytes are inspected only to keep the widest [`Request`] and the
/// calls reachable from one instruction family out of a single SBF frame.
/// Every selected function still runs [`Request::decode`] before it reads an
/// account or calls a family module.  Consequently a bad request tag, version,
/// length, action, intent, or field earns the codec's exact refusal even when
/// its untrusted discriminator happened to select a particular hint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Route {
    Split,
    MergeMaterialize,
    MarketInit,
    ObserveResolve,
    ExternalExit,
    CashExit,
    Artifact,
    OrdersBatch,
    Genesis,
    #[cfg(feature = "profile-full")]
    SourceIngest,
    SourceIngestV2,
    #[cfg(feature = "profile-full")]
    DirectSelection,
    #[cfg(any(
        feature = "profile-full",
        feature = "profile-direct-v3-source-v2-point"
    ))]
    DirectSelectionV3,
    #[cfg(feature = "profile-full")]
    ResolutionWork,
    #[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
    GeneralV2,
    DecodeOnly,
}

// These are routing hints, not a second decoder.  The values are the frozen
// `Request` action byte and `Intent` tag byte.  The encoded-request test pins
// every currently admitted mapping to the authoritative encoders and decoders,
// which remain the only functions allowed to accept or interpret the request.
const ACTION_LAYOUT_HINT: u8 = 0;
const ACTION_RESOLVE_HINT: u8 = 1;
const ACTION_REDEEM_INTERNAL_HINT: u8 = 2;
const INTENT_CREATE_MARKET_HINT: u8 = 1;
const INTENT_SPLIT_HINT: u8 = 2;
const INTENT_MERGE_HINT: u8 = 3;
const INTENT_MATERIALIZE_HINT: u8 = 4;
const INTENT_DEMATERIALIZE_HINT: u8 = 5;
const INTENT_FEED_ADVANCE_HINT: u8 = 6;
const INTENT_PLACE_ORDER_HINT: u8 = 7;
const INTENT_CANCEL_ORDER_HINT: u8 = 8;
const INTENT_SETTLE_PAGE_HINT: u8 = 9;
const INTENT_INIT_REALM_HINT: u8 = 10;
const INTENT_INIT_PROFILE_HINT: u8 = 11;
const INTENT_INIT_PRICE_GRID_HINT: u8 = 12;
const INTENT_INIT_TERMS_HINT: u8 = 13;
const INTENT_INIT_ORDER_PAGE_HINT: u8 = 14;
const INTENT_ENDOW_HINT: u8 = 15;
const INTENT_REDEEM_EXTERNAL_HINT: u8 = 16;
const INTENT_WITHDRAW_CASH_HINT: u8 = 17;
const INTENT_BEGIN_ARTIFACT_HINT: u8 = 18;
const INTENT_WRITE_ARTIFACT_HINT: u8 = 19;
const INTENT_SEAL_ARTIFACT_HINT: u8 = 20;
const INTENT_ABORT_ARTIFACT_HINT: u8 = 21;
const INTENT_SUBMIT_DIRECT_PAGE_HINT: u8 = 22;
const INTENT_INIT_SOURCE_SPEC_HINT: u8 = 23;
const INTENT_INIT_SOURCE_ARCHIVE_HINT: u8 = 24;
const INTENT_APPEND_SOURCE_ARCHIVE_HINT: u8 = 25;
const INTENT_SEAL_SOURCE_ARCHIVE_HINT: u8 = 26;
const INTENT_INIT_DIRECT_EPOCH_V3_HINT: u8 = 27;
const INTENT_FREEZE_DIRECT_EPOCH_V3_HINT: u8 = 28;
const INTENT_SUBMIT_DIRECT_CANDIDATE_V2_HINT: u8 = 29;
const INTENT_SELECT_DIRECT_WINDOW_V1_HINT: u8 = 30;
const INTENT_SETTLE_DIRECT_V2_HINT: u8 = 31;
const INTENT_BEGIN_RESOLUTION_WORK_HINT: u8 = 32;
const INTENT_FOLD_RESOLUTION_WORK_HINT: u8 = 33;
const INTENT_FINALIZE_RESOLUTION_WORK_HINT: u8 = 34;
const INTENT_ABORT_RESOLUTION_WORK_HINT: u8 = 35;
const INTENT_INIT_DIRECT_EPOCH_V4_HINT: u8 = 36;
const INTENT_LAPSE_SELECTED_DIRECT_V3_HINT: u8 = 46;
const INTENT_INIT_CLEAR_WORK_HINT: u8 = 47;
const INTENT_GROW_CLEAR_WORK_HINT: u8 = 48;
const INTENT_INIT_EPOCH_HINT: u8 = 49;
const INTENT_FREEZE_EPOCH_HINT: u8 = 50;
const INTENT_ADVANCE_CLEAR_WORK_HINT: u8 = 51;
const INTENT_ADVANCE_CLEAR_SLICES_HINT: u8 = 52;
const INTENT_COMPLETE_CLEAR_WORK_HINT: u8 = 53;
const INTENT_SUBMIT_CANDIDATE_HINT: u8 = 54;
const INTENT_WRITE_CANDIDATE_FEED_HINT: u8 = 55;
const INTENT_SEAL_CANDIDATE_HINT: u8 = 56;
const INTENT_FINALIZE_SELECTION_HINT: u8 = 57;
const INTENT_FREEZE_ENTITLEMENT_HINT: u8 = 58;
const INTENT_ENTITLE_SLICE_HINT: u8 = 59;
const INTENT_RELEASE_TERMINAL_RESERVATION_HINT: u8 = 60;
const INTENT_CLOSE_GENERAL_RECEIPT_HINT: u8 = 61;
const INTENT_CLOSE_GENERAL_RESERVATION_HINT: u8 = 62;
const INTENT_CLOSE_GENERAL_PAGE_HINT: u8 = 63;
const INTENT_CLOSE_GENERAL_POT_HINT: u8 = 64;
const INTENT_CLOSE_GENERAL_CANDIDATE_HINT: u8 = 65;
const INTENT_CLOSE_GENERAL_CLEAR_WORK_HINT: u8 = 66;
const INTENT_CLOSE_GENERAL_EPOCH_HINT: u8 = 67;
const INTENT_CLOSE_REVENUE_POLICY_RECORD_HINT: u8 = 68;
const INTENT_CLOSE_POSITION_HINT: u8 = 69;
const INTENT_INIT_SOURCE_SPEC_V2_HINT: u8 = 70;
const INTENT_INIT_SOURCE_ARCHIVE_V2_HINT: u8 = 71;
const INTENT_APPEND_SOURCE_ARCHIVE_V2_HINT: u8 = 72;
const INTENT_SEAL_SOURCE_ARCHIVE_V2_HINT: u8 = 73;

fn route_hint(instruction_data: &[u8]) -> Route {
    match instruction_data.get(10).copied() {
        Some(ACTION_LAYOUT_HINT) => match instruction_data.get(13).copied() {
            #[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
            Some(clutch_solana_layout::registry::GENERAL_V2_FAMILY_TAG)
                if instruction_data.get(14).copied()
                    == Some(clutch_solana_layout::registry::GENERAL_V2_FAMILY_VERSION)
                    && instruction_data.get(15).copied().is_some_and(|action| {
                        capabilities::extension_intent_action_enabled(
                            clutch_solana_layout::registry::GENERAL_V2_FAMILY_TAG,
                            clutch_solana_layout::registry::GENERAL_V2_FAMILY_VERSION,
                            action,
                        )
                    }) =>
            {
                Route::GeneralV2
            }
            Some(INTENT_SPLIT_HINT) => Route::Split,
            Some(INTENT_MERGE_HINT | INTENT_MATERIALIZE_HINT | INTENT_DEMATERIALIZE_HINT) => {
                Route::MergeMaterialize
            }
            Some(INTENT_CREATE_MARKET_HINT) => Route::MarketInit,
            #[cfg(feature = "profile-full")]
            Some(INTENT_FEED_ADVANCE_HINT) => Route::ObserveResolve,
            Some(INTENT_REDEEM_EXTERNAL_HINT) => Route::ExternalExit,
            Some(INTENT_WITHDRAW_CASH_HINT) => Route::CashExit,
            Some(
                INTENT_BEGIN_ARTIFACT_HINT
                | INTENT_WRITE_ARTIFACT_HINT
                | INTENT_SEAL_ARTIFACT_HINT
                | INTENT_ABORT_ARTIFACT_HINT,
            ) => Route::Artifact,
            Some(INTENT_PLACE_ORDER_HINT) => Route::OrdersBatch,
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            Some(INTENT_CANCEL_ORDER_HINT) => Route::OrdersBatch,
            #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
            Some(
                INTENT_SETTLE_PAGE_HINT
                | INTENT_INIT_CLEAR_WORK_HINT
                | INTENT_GROW_CLEAR_WORK_HINT
                | INTENT_INIT_EPOCH_HINT
                | INTENT_FREEZE_EPOCH_HINT
                | INTENT_ADVANCE_CLEAR_WORK_HINT
                | INTENT_ADVANCE_CLEAR_SLICES_HINT
                | INTENT_COMPLETE_CLEAR_WORK_HINT
                | INTENT_SUBMIT_CANDIDATE_HINT
                | INTENT_WRITE_CANDIDATE_FEED_HINT
                | INTENT_SEAL_CANDIDATE_HINT
                | INTENT_FINALIZE_SELECTION_HINT
                | INTENT_FREEZE_ENTITLEMENT_HINT
                | INTENT_ENTITLE_SLICE_HINT
                | INTENT_RELEASE_TERMINAL_RESERVATION_HINT
                | INTENT_CLOSE_GENERAL_RECEIPT_HINT
                | INTENT_CLOSE_GENERAL_RESERVATION_HINT
                | INTENT_CLOSE_GENERAL_PAGE_HINT
                | INTENT_CLOSE_GENERAL_POT_HINT
                | INTENT_CLOSE_GENERAL_CANDIDATE_HINT
                | INTENT_CLOSE_GENERAL_CLEAR_WORK_HINT
                | INTENT_CLOSE_GENERAL_EPOCH_HINT
                | INTENT_CLOSE_POSITION_HINT,
            ) => Route::OrdersBatch,
            #[cfg(feature = "profile-full")]
            Some(INTENT_SUBMIT_DIRECT_PAGE_HINT) => Route::OrdersBatch,
            Some(
                INTENT_INIT_REALM_HINT
                | INTENT_INIT_PROFILE_HINT
                | INTENT_INIT_PRICE_GRID_HINT
                | INTENT_INIT_TERMS_HINT
                | INTENT_INIT_ORDER_PAGE_HINT
                | INTENT_ENDOW_HINT
                | INTENT_CLOSE_REVENUE_POLICY_RECORD_HINT,
            ) => Route::Genesis,
            #[cfg(feature = "profile-full")]
            Some(
                INTENT_INIT_SOURCE_SPEC_HINT
                | INTENT_INIT_SOURCE_ARCHIVE_HINT
                | INTENT_APPEND_SOURCE_ARCHIVE_HINT
                | INTENT_SEAL_SOURCE_ARCHIVE_HINT,
            ) => Route::SourceIngest,
            /* The v2 family gets its own hint arm rather than joining V1's.
             * The two never share a frame: V1's append holds three provider
             * account views and v2's holds six, and the pull authentication
             * join below it is the deepest call in either family. */
            Some(
                INTENT_INIT_SOURCE_SPEC_V2_HINT
                | INTENT_INIT_SOURCE_ARCHIVE_V2_HINT
                | INTENT_APPEND_SOURCE_ARCHIVE_V2_HINT
                | INTENT_SEAL_SOURCE_ARCHIVE_V2_HINT,
            ) => Route::SourceIngestV2,
            #[cfg(feature = "profile-full")]
            Some(
                INTENT_INIT_DIRECT_EPOCH_V3_HINT
                | INTENT_FREEZE_DIRECT_EPOCH_V3_HINT
                | INTENT_SUBMIT_DIRECT_CANDIDATE_V2_HINT
                | INTENT_SELECT_DIRECT_WINDOW_V1_HINT
                | INTENT_SETTLE_DIRECT_V2_HINT,
            ) => Route::DirectSelection,
            #[cfg(feature = "profile-full")]
            Some(
                INTENT_BEGIN_RESOLUTION_WORK_HINT
                | INTENT_FOLD_RESOLUTION_WORK_HINT
                | INTENT_FINALIZE_RESOLUTION_WORK_HINT
                | INTENT_ABORT_RESOLUTION_WORK_HINT,
            ) => Route::ResolutionWork,
            // Tags 36 through 46 are one all-or-nothing family: the dedicated
            // Direct V3 request decoder is the only decoder that accepts them,
            // and its handler match is exhaustive with no unimplemented arm.
            #[cfg(any(
                feature = "profile-full",
                feature = "profile-direct-v3-source-v2-point"
            ))]
            Some(INTENT_INIT_DIRECT_EPOCH_V4_HINT..=INTENT_LAPSE_SELECTED_DIRECT_V3_HINT) => {
                Route::DirectSelectionV3
            }
            _ => Route::DecodeOnly,
        },
        Some(ACTION_RESOLVE_HINT | ACTION_REDEEM_INTERNAL_HINT) => Route::ObserveResolve,
        _ => Route::DecodeOnly,
    }
}

/// Decode one request and route it to the instruction family that owns it.
///
/// Each arm enters a distinct, non-inlined function so the SBF backend never
/// has to reserve one frame for the widest request plus the calls of every
/// family.  The hint itself has no authority: each arm decodes and checks the
/// action again before touching accounts.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    if let Some(action) = disabled_source_v3_action(instruction_data) {
        return crate::source_plane_v3::process_reserved_disabled(action);
    }
    if disabled_canonical_tag(instruction_data) {
        return Err(ClutchError::UnsupportedInstruction.into());
    }
    match route_hint(instruction_data) {
        Route::Split => process_split(program_id, accounts, instruction_data),
        Route::MergeMaterialize => {
            process_merge_materialize(program_id, accounts, instruction_data)
        }
        Route::MarketInit => process_market_init(program_id, accounts, instruction_data),
        Route::ObserveResolve => process_observe_resolve(program_id, accounts, instruction_data),
        Route::ExternalExit => process_external_exit(program_id, accounts, instruction_data),
        Route::CashExit => process_cash_exit(program_id, accounts, instruction_data),
        Route::Artifact => process_artifact(program_id, accounts, instruction_data),
        Route::OrdersBatch => process_orders_batch(program_id, accounts, instruction_data),
        Route::Genesis => process_genesis(program_id, accounts, instruction_data),
        #[cfg(feature = "profile-full")]
        Route::SourceIngest => process_source_ingest(program_id, accounts, instruction_data),
        Route::SourceIngestV2 => process_source_ingest_v2(program_id, accounts, instruction_data),
        #[cfg(feature = "profile-full")]
        Route::DirectSelection => process_direct_selection(program_id, accounts, instruction_data),
        #[cfg(any(
            feature = "profile-full",
            feature = "profile-direct-v3-source-v2-point"
        ))]
        Route::DirectSelectionV3 => {
            process_direct_selection_v3(program_id, accounts, instruction_data)
        }
        #[cfg(feature = "profile-full")]
        Route::ResolutionWork => process_resolution_work(program_id, accounts, instruction_data),
        #[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
        Route::GeneralV2 => process_general_v2(program_id, accounts, instruction_data),
        Route::DecodeOnly => decode_only(instruction_data),
    }
}

/// Identify one exact allocated-but-disabled SourcePlane V3 action without
/// decoding or inspecting its payload accounts.
fn disabled_source_v3_action(
    instruction_data: &[u8],
) -> Option<clutch_solana_layout::registry::SourceSeriesAction> {
    if !disabled_canonical_tag(instruction_data) {
        return None;
    }
    match clutch_solana_layout::registry::decode_extension_action(
        instruction_data[13],
        instruction_data[14],
        instruction_data[15],
    ) {
        Ok(clutch_solana_layout::registry::ExtensionAction::SourceV3(action)) => Some(action),
        _ => None,
    }
}

/// Decode the strict successor envelope and enter only the General V2 lab.
#[inline(never)]
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
fn process_general_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request =
        ExtensionRequest::decode(instruction_data).map_err(|_| ClutchError::NonCanonical)?;
    match request.envelope.action {
        ExtensionAction::GeneralV2(action) => general_v2_identity::process(
            program_id,
            accounts,
            request.sequence,
            action,
            request.envelope.payload,
        ),
        ExtensionAction::StructuredClaim(_)
        | ExtensionAction::SourceV3(_)
        | ExtensionAction::RecurringSeries(_)
        | ExtensionAction::Recovery(_) => unexpected_route(),
    }
}

/// Recognize only a structurally identified canonical layout tag and decide
/// whether the compiled product omitted it. This happens before decoding and
/// therefore before any account is inspected.
fn disabled_canonical_tag(instruction_data: &[u8]) -> bool {
    if instruction_data.len() < 15
        || instruction_data[0] != 0xd1
        || instruction_data[1] != 1
        || instruction_data[10] != ACTION_LAYOUT_HINT
    {
        return false;
    }
    let Ok(length_bytes) = instruction_data[11..13].try_into() else {
        return false;
    };
    let inner_len = usize::from(u16::from_le_bytes(length_bytes));
    let Some(exact_len) = 13usize.checked_add(inner_len) else {
        return false;
    };
    if inner_len > clutch_solana_layout::MAX_INTENT_BYTES
        || instruction_data.len() != exact_len
        || inner_len < 2
    {
        return false;
    }
    let tag = instruction_data[13];
    let version = instruction_data[14];
    match clutch_solana_layout::registry::classify_intent(tag, version) {
        Some(clutch_solana_layout::registry::IntentAllocation::LegacyV3) => {
            !capabilities::legacy_intent_enabled(tag, version)
                && !capabilities::direct_v3_intent_enabled(tag, version)
        }
        Some(clutch_solana_layout::registry::IntentAllocation::Extension(_)) => {
            inner_len >= clutch_solana_layout::registry::EXTENSION_ENVELOPE_BYTES
                && capabilities::extension_intent_action_allocated(
                    tag,
                    version,
                    instruction_data[15],
                )
                && !capabilities::extension_intent_action_enabled(
                    tag,
                    version,
                    instruction_data[15],
                )
        }
        None => false,
    }
}

#[inline(never)]
fn process_split(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = Request::decode(instruction_data)?;
    match request.action {
        Action::Layout(Intent::Split {
            market,
            owner,
            quantity,
        }) => split::process(
            program_id,
            accounts,
            &split::SplitRequest {
                sequence: request.sequence,
                market,
                owner,
                quantity,
            },
        ),
        _ => unexpected_route(),
    }
}

#[inline(never)]
fn process_merge_materialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = Request::decode(instruction_data)?;
    match request.action {
        Action::Layout(Intent::Merge { .. })
        | Action::Layout(Intent::Materialize { .. })
        | Action::Layout(Intent::Dematerialize { .. }) => {
            merge_materialize::process(program_id, accounts, &request)
        }
        _ => unexpected_route(),
    }
}

#[inline(never)]
fn process_market_init(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = Request::decode(instruction_data)?;
    match request.action {
        Action::Layout(Intent::CreateMarket { .. }) => {
            market_init::process(program_id, accounts, &request)
        }
        _ => unexpected_route(),
    }
}

#[inline(never)]
fn process_observe_resolve(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = Request::decode(instruction_data)?;
    match request.action {
        #[cfg(feature = "profile-full")]
        Action::Layout(Intent::FeedAdvance { .. }) => {
            observe_resolve::process(program_id, accounts, &request)
        }
        Action::Resolve { .. } | Action::RedeemInternal { .. } => {
            observe_resolve::process(program_id, accounts, &request)
        }
        _ => unexpected_route(),
    }
}

#[inline(never)]
fn process_external_exit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = Request::decode(instruction_data)?;
    match request.action {
        Action::Layout(Intent::RedeemExternal { .. }) => {
            external_exit::process(program_id, accounts, &request)
        }
        _ => unexpected_route(),
    }
}

#[inline(never)]
fn process_cash_exit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = Request::decode(instruction_data)?;
    match request.action {
        Action::Layout(Intent::WithdrawCash { .. }) => {
            cash_exit::process(program_id, accounts, &request)
        }
        _ => unexpected_route(),
    }
}

#[inline(never)]
fn process_artifact(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = Request::decode(instruction_data)?;
    match request.action {
        Action::Layout(Intent::BeginArtifact { .. })
        | Action::Layout(Intent::WriteArtifact { .. })
        | Action::Layout(Intent::SealArtifact { .. })
        | Action::Layout(Intent::AbortArtifact { .. }) => {
            artifact::process(program_id, accounts, &request)
        }
        _ => unexpected_route(),
    }
}

#[inline(never)]
fn process_orders_batch(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = Request::decode(instruction_data)?;
    match request.action {
        Action::Layout(Intent::PlaceOrder { .. }) => {
            orders_batch::process(program_id, accounts, &request)
        }
        #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
        Action::Layout(Intent::CancelOrder { .. }) => {
            orders_batch::process(program_id, accounts, &request)
        }
        #[cfg(feature = "profile-full")]
        Action::Layout(Intent::SubmitDirectPage { .. }) => {
            orders_batch::process(program_id, accounts, &request)
        }
        #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
        Action::Layout(Intent::SettlePage { .. })
        | Action::Layout(Intent::InitClearWork { .. })
        | Action::Layout(Intent::GrowClearWork { .. })
        | Action::Layout(Intent::InitEpoch { .. })
        | Action::Layout(Intent::FreezeEpoch { .. })
        | Action::Layout(Intent::AdvanceClearWork { .. })
        | Action::Layout(Intent::AdvanceClearSlices { .. })
        | Action::Layout(Intent::CompleteClearWork { .. })
        | Action::Layout(Intent::SubmitCandidate { .. })
        | Action::Layout(Intent::WriteCandidateFeed { .. })
        | Action::Layout(Intent::SealCandidate { .. })
        | Action::Layout(Intent::FinalizeSelection { .. })
        | Action::Layout(Intent::FreezeEntitlement { .. })
        | Action::Layout(Intent::EntitleSlice { .. })
        | Action::Layout(Intent::ReleaseTerminalReservation { .. })
        | Action::Layout(Intent::CloseGeneralReceipt { .. })
        | Action::Layout(Intent::CloseGeneralReservation { .. })
        | Action::Layout(Intent::CloseGeneralPage { .. })
        | Action::Layout(Intent::CloseGeneralPot { .. })
        | Action::Layout(Intent::CloseGeneralCandidate { .. })
        | Action::Layout(Intent::CloseGeneralClearWork { .. })
        | Action::Layout(Intent::CloseGeneralEpoch { .. })
        | Action::Layout(Intent::ClosePosition { .. }) => {
            orders_batch::process(program_id, accounts, &request)
        }
        _ => unexpected_route(),
    }
}

#[inline(never)]
fn process_genesis(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = Request::decode(instruction_data)?;
    match request.action {
        Action::Layout(Intent::InitRealm { .. })
        | Action::Layout(Intent::InitProfile { .. })
        | Action::Layout(Intent::InitPriceGrid { .. })
        | Action::Layout(Intent::InitTerms { .. })
        | Action::Layout(Intent::InitOrderPage { .. })
        | Action::Layout(Intent::Endow { .. })
        | Action::Layout(Intent::CloseRevenuePolicyRecord { .. }) => {
            genesis::process(program_id, accounts, &request)
        }
        _ => unexpected_route(),
    }
}

#[inline(never)]
#[cfg(feature = "profile-full")]
fn process_source_ingest(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = Request::decode(instruction_data)?;
    match request.action {
        Action::Layout(Intent::InitSourceSpec { .. })
        | Action::Layout(Intent::InitSourceArchive { .. })
        | Action::Layout(Intent::AppendSourceArchive { .. })
        | Action::Layout(Intent::SealSourceArchive { .. }) => {
            source_ingest::process(program_id, accounts, &request)
        }
        _ => unexpected_route(),
    }
}

#[inline(never)]
fn process_source_ingest_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = Request::decode(instruction_data)?;
    match request.action {
        Action::Layout(Intent::InitSourceSpecV2 { .. })
        | Action::Layout(Intent::InitSourceArchiveV2 { .. })
        | Action::Layout(Intent::AppendSourceArchiveV2 { .. })
        | Action::Layout(Intent::SealSourceArchiveV2 { .. }) => {
            source_ingest_v2::process(program_id, accounts, &request)
        }
        _ => unexpected_route(),
    }
}

#[inline(never)]
#[cfg(feature = "profile-full")]
fn process_direct_selection(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = Request::decode(instruction_data)?;
    match request.action {
        Action::Layout(Intent::InitDirectEpochV3 { .. })
        | Action::Layout(Intent::FreezeDirectEpochV3 { .. })
        | Action::Layout(Intent::SubmitDirectCandidateV2 { .. })
        | Action::Layout(Intent::SelectDirectWindowV1 { .. })
        | Action::Layout(Intent::SettleDirectV2 { .. }) => {
            direct_selection::process(program_id, accounts, &request)
        }
        _ => unexpected_route(),
    }
}

/// The Direct V3 family decodes through its dedicated strict envelope.
///
/// The legacy [`Request`] decoder still refuses every tag in `36..=46`, and
/// [`DirectV3Request::decode`] refuses every legacy tag, so a partially added
/// tag can never fall into a handler with different account versions.
#[inline(never)]
#[cfg(any(
    feature = "profile-full",
    feature = "profile-direct-v3-source-v2-point"
))]
fn process_direct_selection_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = DirectV3Request::decode(instruction_data)?;
    direct_selection_v3::process(program_id, accounts, &request)
}

#[inline(never)]
#[cfg(feature = "profile-full")]
fn process_resolution_work(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = Request::decode(instruction_data)?;
    match request.action {
        Action::Layout(Intent::BeginResolutionWork(_))
        | Action::Layout(Intent::FoldResolutionWork(_))
        | Action::Layout(Intent::FinalizeResolutionWork(_))
        | Action::Layout(Intent::AbortResolutionWork(_)) => {
            resolution_work::process(program_id, accounts, &request)
        }
        _ => unexpected_route(),
    }
}

#[inline(never)]
fn decode_only(instruction_data: &[u8]) -> Outcome<()> {
    match Request::decode(instruction_data) {
        Err(error) => Err(error.into()),
        // A newly added canonical action remains fail-closed until this router
        // assigns it to a reviewed family and extends the exhaustiveness test.
        Ok(_) => unexpected_route(),
    }
}

fn unexpected_route() -> Outcome<()> {
    Err(ClutchError::UnsupportedInstruction.into())
}

/// The refusal a family module returns when its transition is not written yet.
///
/// Kept here so that every stub returns one identical, greppable thing and a
/// lane replacing a stub deletes a call to this function rather than editing a
/// bespoke error expression.
pub fn not_yet_implemented() -> Outcome<()> {
    Err(ClutchError::NotYetImplemented.into())
}

#[cfg(all(test, feature = "profile-full"))]
mod tests {
    use super::*;
    use crate::error::Refusal;
    use clutch_solana_layout::{
        artifact::{ArtifactKind, ARTIFACT_CHUNK_BYTES},
        canonical_order_id,
        resolution_work::{
            AbortResolutionWorkV1, BeginResolutionWorkV1, FinalizeResolutionWorkV1,
            FoldResolutionWorkV1, FINALIZATION_EXACT_ONLY,
        },
        Hash32, Intent, OrderRecord, OrderSlot, MAX_INTENT_BYTES, MAX_OUTCOMES,
    };
    use clutch_solana_reference::Error as ReferenceError;
    use solana_program_error::ProgramError;

    const REQUEST_TAG: u8 = 0xd1;
    const REQUEST_VERSION: u8 = 1;

    fn hash(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; 32])
    }

    fn layout_request(sequence: u64, intent: Intent) -> Vec<u8> {
        let mut intent_bytes = [0; MAX_INTENT_BYTES];
        let intent_len = intent.encode(&mut intent_bytes).unwrap();
        let mut request = vec![0; 13 + intent_len];
        request[0] = REQUEST_TAG;
        request[1] = REQUEST_VERSION;
        request[2..10].copy_from_slice(&sequence.to_le_bytes());
        request[10] = ACTION_LAYOUT_HINT;
        request[11..13].copy_from_slice(&(intent_len as u16).to_le_bytes());
        request[13..].copy_from_slice(&intent_bytes[..intent_len]);
        request
    }

    fn split_request(sequence: u64, quantity: u64) -> Vec<u8> {
        layout_request(
            sequence,
            Intent::Split {
                market: hash(1),
                owner: hash(2),
                quantity,
            },
        )
    }

    fn intent_cases() -> Vec<(Intent, Route)> {
        let order = OrderRecord {
            owner: hash(9),
            order_id: canonical_order_id(1),
            outcome: 0,
            side: 0,
            quantity: 5,
            limit: 7,
            minimum_fill: 1,
            flags: 0,
            generation: 1,
            expiry_epoch: 4,
        };
        let kind = ArtifactKind::CollateralPolicy;
        vec![
            (
                Intent::CreateMarket {
                    realm: hash(1),
                    profile: hash(2),
                    market_nonce: 3,
                    outcome_count: 2,
                    terms: hash(4),
                    feed: hash(5),
                },
                Route::MarketInit,
            ),
            (
                Intent::Split {
                    market: hash(1),
                    owner: hash(2),
                    quantity: 3,
                },
                Route::Split,
            ),
            (
                Intent::Merge {
                    market: hash(1),
                    owner: hash(2),
                    quantity: 3,
                },
                Route::MergeMaterialize,
            ),
            (
                Intent::Materialize {
                    market: hash(1),
                    owner: hash(2),
                    destination: hash(3),
                    outcome: 0,
                    quantity: 4,
                },
                Route::MergeMaterialize,
            ),
            (
                Intent::Dematerialize {
                    market: hash(1),
                    owner: hash(2),
                    source: hash(3),
                    outcome: 0,
                    quantity: 4,
                },
                Route::MergeMaterialize,
            ),
            (
                Intent::FeedAdvance {
                    feed: hash(1),
                    cursor: 2,
                    evidence: hash(3),
                },
                Route::ObserveResolve,
            ),
            (
                Intent::PlaceOrder {
                    market: hash(1),
                    epoch: hash(2),
                    max_fee_atoms: 3,
                    slot: OrderSlot::Single(order),
                },
                Route::OrdersBatch,
            ),
            (
                Intent::CancelOrder {
                    market: hash(1),
                    epoch: hash(2),
                    owner: hash(3),
                    order_id: canonical_order_id(1),
                    generation: 4,
                },
                Route::OrdersBatch,
            ),
            (
                Intent::SettlePage {
                    market: hash(1),
                    epoch: hash(2),
                    page_index: 0,
                },
                Route::OrdersBatch,
            ),
            (
                Intent::InitRealm {
                    profile: hash(1),
                    realm_nonce: 2,
                    max_outcomes: MAX_OUTCOMES as u8,
                    profile_version: 1,
                },
                Route::Genesis,
            ),
            (
                Intent::InitProfile {
                    realm: hash(1),
                    collateral_policy_digest: hash(2),
                    subfield_schema_version: 1,
                    profile_version: 1,
                },
                Route::Genesis,
            ),
            (
                Intent::InitPriceGrid {
                    realm: hash(1),
                    grid: hash(2),
                },
                Route::Genesis,
            ),
            (
                Intent::InitTerms {
                    realm: hash(1),
                    terms: hash(2),
                },
                Route::Genesis,
            ),
            (
                Intent::InitOrderPage {
                    market: hash(1),
                    epoch: hash(2),
                    page_index: 0,
                    page_count: 1,
                },
                Route::Genesis,
            ),
            (
                Intent::Endow {
                    market: hash(1),
                    owner: hash(2),
                    amount: 3,
                },
                Route::Genesis,
            ),
            (
                Intent::RedeemExternal {
                    market: hash(1),
                    claimant: hash(2),
                    source: hash(3),
                    destination: hash(4),
                    outcome: 0,
                    quantity: 5,
                },
                Route::ExternalExit,
            ),
            (
                Intent::WithdrawCash {
                    market: hash(1),
                    owner: hash(2),
                    destination: hash(3),
                    amount: 4,
                },
                Route::CashExit,
            ),
            (
                Intent::BeginArtifact {
                    kind,
                    context: hash(1),
                    digest: hash(2),
                    exact_len: kind.exact_len() as u16,
                    expires_slot: 5,
                },
                Route::Artifact,
            ),
            (
                Intent::WriteArtifact {
                    kind,
                    context: hash(1),
                    digest: hash(2),
                    cursor: 0,
                    chunk_len: 1,
                    chunk: [0; ARTIFACT_CHUNK_BYTES],
                },
                Route::Artifact,
            ),
            (
                Intent::SealArtifact {
                    kind,
                    context: hash(1),
                    digest: hash(2),
                    exact_len: kind.exact_len() as u16,
                },
                Route::Artifact,
            ),
            (
                Intent::AbortArtifact {
                    kind,
                    context: hash(1),
                    digest: hash(2),
                },
                Route::Artifact,
            ),
            (
                Intent::SubmitDirectPage {
                    market: hash(1),
                    epoch: hash(2),
                    page_index: 0,
                },
                Route::OrdersBatch,
            ),
            (
                Intent::InitClearWork {
                    market: hash(1),
                    epoch: hash(2),
                    candidate: hash(3),
                },
                Route::OrdersBatch,
            ),
            (
                Intent::GrowClearWork {
                    market: hash(1),
                    epoch: hash(2),
                    candidate: hash(3),
                },
                Route::OrdersBatch,
            ),
            (
                Intent::InitEpoch {
                    market: hash(1),
                    epoch_index: 7,
                    policy: hash(2),
                    freeze_deadline_slot: 900,
                },
                Route::OrdersBatch,
            ),
            (
                Intent::FreezeEpoch {
                    market: hash(1),
                    epoch: hash(2),
                },
                Route::OrdersBatch,
            ),
            (
                Intent::AdvanceClearWork {
                    market: hash(1),
                    epoch: hash(2),
                    candidate: hash(3),
                    max_orders: 16,
                },
                Route::OrdersBatch,
            ),
            (
                Intent::AdvanceClearSlices {
                    market: hash(1),
                    epoch: hash(2),
                    candidate: hash(3),
                    max_slices: 16,
                },
                Route::OrdersBatch,
            ),
            (
                Intent::CompleteClearWork {
                    market: hash(1),
                    epoch: hash(2),
                    candidate: hash(3),
                },
                Route::OrdersBatch,
            ),
            (
                Intent::SubmitCandidate {
                    market: hash(1),
                    epoch: hash(2),
                    prices: {
                        let mut prices = [0u64; MAX_OUTCOMES];
                        prices[0] = 4_000;
                        prices[1] = 6_000;
                        prices
                    },
                    virtual_split: 0,
                    virtual_merge: 0,
                    honored_aon_mask: 0,
                    declared_slices: Some(3),
                    weighted_direct_volume: 0,
                    limit_surplus_price_units: 0,
                    distinct_owners: 2,
                },
                Route::OrdersBatch,
            ),
            (
                Intent::WriteCandidateFeed {
                    market: hash(1),
                    epoch: hash(2),
                    candidate: hash(3),
                    chunk: clutch_solana_layout::CandidateFeedChunk::Fills {
                        count: 1,
                        fills: {
                            let mut fills = [0u64; clutch_solana_layout::FEED_FILLS_PER_CHUNK];
                            fills[0] = 9;
                            fills
                        },
                    },
                },
                Route::OrdersBatch,
            ),
            (
                Intent::SealCandidate {
                    market: hash(1),
                    epoch: hash(2),
                    candidate: hash(3),
                },
                Route::OrdersBatch,
            ),
            (
                Intent::FinalizeSelection {
                    market: hash(1),
                    epoch: hash(2),
                },
                Route::OrdersBatch,
            ),
            (
                Intent::FreezeEntitlement {
                    market: hash(1),
                    epoch: hash(2),
                    candidate: hash(3),
                },
                Route::OrdersBatch,
            ),
            (
                Intent::EntitleSlice {
                    market: hash(1),
                    epoch: hash(2),
                    candidate: hash(3),
                    slice_index: 4,
                },
                Route::OrdersBatch,
            ),
            (
                Intent::ReleaseTerminalReservation {
                    market: hash(1),
                    epoch: hash(2),
                },
                Route::OrdersBatch,
            ),
            (
                Intent::CloseGeneralReceipt {
                    market: hash(1),
                    epoch: hash(2),
                    candidate: hash(3),
                    slice_index: 4,
                },
                Route::OrdersBatch,
            ),
            (
                Intent::CloseGeneralReservation {
                    market: hash(1),
                    epoch: hash(2),
                },
                Route::OrdersBatch,
            ),
            (
                Intent::CloseGeneralPage {
                    market: hash(1),
                    epoch: hash(2),
                    page_index: 0,
                },
                Route::OrdersBatch,
            ),
            (
                Intent::CloseGeneralPot {
                    market: hash(1),
                    epoch: hash(2),
                },
                Route::OrdersBatch,
            ),
            (
                Intent::CloseGeneralCandidate {
                    market: hash(1),
                    epoch: hash(2),
                    candidate: hash(3),
                },
                Route::OrdersBatch,
            ),
            (
                Intent::CloseGeneralClearWork {
                    market: hash(1),
                    epoch: hash(2),
                    candidate: hash(3),
                },
                Route::OrdersBatch,
            ),
            (
                Intent::CloseGeneralEpoch {
                    market: hash(1),
                    epoch: hash(2),
                },
                Route::OrdersBatch,
            ),
            (
                Intent::InitSourceSpec {
                    terms: hash(1),
                    spec_body: [7; clutch_solana_layout::SOURCE_SPEC_BODY_V1_BYTES],
                },
                Route::SourceIngest,
            ),
            (
                Intent::InitSourceArchive { terms: hash(1) },
                Route::SourceIngest,
            ),
            (
                Intent::AppendSourceArchive { terms: hash(1) },
                Route::SourceIngest,
            ),
            (
                Intent::SealSourceArchive { terms: hash(1) },
                Route::SourceIngest,
            ),
            (
                Intent::InitSourceSpecV2 {
                    terms: hash(1),
                    spec_body: [4; clutch_solana_layout::SOURCE_SPEC_BODY_V2_BYTES],
                },
                Route::SourceIngestV2,
            ),
            (
                Intent::InitSourceArchiveV2 { terms: hash(1) },
                Route::SourceIngestV2,
            ),
            (
                Intent::AppendSourceArchiveV2 { terms: hash(1) },
                Route::SourceIngestV2,
            ),
            (
                Intent::SealSourceArchiveV2 { terms: hash(1) },
                Route::SourceIngestV2,
            ),
            (
                Intent::InitDirectEpochV3 {
                    market: hash(1),
                    epoch_index: 7,
                    policy: hash(2),
                    submission_opens_slot: 100,
                    submission_closes_slot: 120,
                },
                Route::DirectSelection,
            ),
            (
                Intent::FreezeDirectEpochV3 {
                    market: hash(1),
                    epoch: hash(2),
                },
                Route::DirectSelection,
            ),
            (
                Intent::SubmitDirectCandidateV2 {
                    market: hash(1),
                    epoch: hash(2),
                    outcome_price: 5_000,
                },
                Route::DirectSelection,
            ),
            (
                Intent::SelectDirectWindowV1 {
                    market: hash(1),
                    epoch: hash(2),
                },
                Route::DirectSelection,
            ),
            (
                Intent::SettleDirectV2 {
                    market: hash(1),
                    epoch: hash(2),
                },
                Route::DirectSelection,
            ),
            (
                Intent::BeginResolutionWork(BeginResolutionWorkV1 {
                    work_nonce: [8; 32],
                    finalization_mode: FINALIZATION_EXACT_ONLY,
                    expires_slot: 200,
                    declared_deposit: 1,
                    cost_schedule_digest: [9; 32],
                }),
                Route::ResolutionWork,
            ),
            (
                Intent::FoldResolutionWork(FoldResolutionWorkV1 {
                    work_commitment: [1; 32],
                    archive_account: [2; 32],
                    archive_commitment: [3; 32],
                    expected_cursor: 4,
                    record_count: 1,
                }),
                Route::ResolutionWork,
            ),
            (
                Intent::FinalizeResolutionWork(FinalizeResolutionWorkV1 {
                    work_commitment: [1; 32],
                    expected_cursor: 4,
                    expected_archive_commitment: [3; 32],
                }),
                Route::ResolutionWork,
            ),
            (
                Intent::AbortResolutionWork(AbortResolutionWorkV1 {
                    work_commitment: [1; 32],
                    expected_cursor: 4,
                    expected_archive_commitment: [3; 32],
                }),
                Route::ResolutionWork,
            ),
        ]
    }

    fn non_layout_request(action: u8) -> Vec<u8> {
        let len = if action == ACTION_RESOLVE_HINT {
            12
        } else {
            20
        };
        let mut request = vec![0; len];
        request[0] = REQUEST_TAG;
        request[1] = REQUEST_VERSION;
        request[2..10].copy_from_slice(&7_u64.to_le_bytes());
        request[10] = action;
        if action == ACTION_REDEEM_INTERNAL_HINT {
            request[12..20].copy_from_slice(&5_u64.to_le_bytes());
        }
        request
    }

    fn process_without_accounts(instruction_data: &[u8]) -> ProgramError {
        process(&Pubkey::new_from_array([9; 32]), &[], instruction_data)
            .unwrap_err()
            .into()
    }

    #[test]
    fn encoded_requests_pin_every_route_hint_to_the_canonical_decoders() {
        for (intent, expected) in intent_cases() {
            let request = layout_request(7, intent);
            assert_eq!(
                Request::decode(&request),
                Ok(Request {
                    sequence: 7,
                    action: Action::Layout(intent)
                })
            );
            assert_eq!(route_hint(&request), expected, "{intent:?}");
        }

        let resolve = non_layout_request(ACTION_RESOLVE_HINT);
        assert_eq!(
            Request::decode(&resolve),
            Ok(Request {
                sequence: 7,
                action: Action::Resolve { payout_index: 0 },
            })
        );
        assert_eq!(route_hint(&resolve), Route::ObserveResolve);

        let redeem = non_layout_request(ACTION_REDEEM_INTERNAL_HINT);
        assert_eq!(
            Request::decode(&redeem),
            Ok(Request {
                sequence: 7,
                action: Action::RedeemInternal {
                    outcome: 0,
                    quantity: 5,
                },
            })
        );
        assert_eq!(route_hint(&redeem), Route::ObserveResolve);
    }

    #[test]
    fn malformed_mutations_keep_the_canonical_decoder_refusal_across_routes() {
        let mut cases = Vec::new();
        for (intent, _) in intent_cases() {
            let valid = layout_request(7, intent);

            let mut wrong_tag = valid.clone();
            wrong_tag[0] ^= 1;
            cases.push(wrong_tag);

            let mut wrong_version = valid.clone();
            wrong_version[1] = REQUEST_VERSION + 1;
            cases.push(wrong_version);

            let mut wrong_declared_len = valid.clone();
            wrong_declared_len[11] = wrong_declared_len[11].wrapping_add(1);
            cases.push(wrong_declared_len);

            let mut wrong_intent_version = valid;
            wrong_intent_version[14] = wrong_intent_version[14].wrapping_add(1);
            cases.push(wrong_intent_version);
        }

        for action in [ACTION_RESOLVE_HINT, ACTION_REDEEM_INTERNAL_HINT] {
            let valid = non_layout_request(action);
            cases.push(valid[..10].to_vec());
            let mut wrong_tag = valid.clone();
            wrong_tag[0] ^= 1;
            cases.push(wrong_tag);
            let mut wrong_version = valid;
            wrong_version[1] = REQUEST_VERSION + 1;
            cases.push(wrong_version);
        }

        let valid = split_request(7, 5);
        let mut zero_quantity = valid.clone();
        let quantity_at = zero_quantity.len() - 8;
        zero_quantity[quantity_at..].fill(0);
        cases.push(zero_quantity);

        let mut unknown_action = valid.clone();
        unknown_action[10] = 3;
        cases.push(unknown_action);

        for mutation in cases {
            let expected: ProgramError =
                Refusal::from(Request::decode(&mutation).unwrap_err()).into();
            assert_eq!(process_without_accounts(&mutation), expected);
        }
    }

    fn direct_v3_intents() -> [clutch_solana_layout::direct_selection_v3::DirectV3Intent; 11] {
        use clutch_solana_layout::direct_selection_v3::{DirectKeeperRewardsV3, DirectV3Intent};
        let rewards = DirectKeeperRewardsV3 {
            begin_verification: 1,
            verify_candidate: 2,
            finalize_selection: 3,
            settle: 4,
            lapse: 5,
        };
        [
            DirectV3Intent::InitEpoch {
                market: hash(1),
                epoch_index: 7,
                policy: hash(2),
                submission_opens_slot: 100,
                submission_closes_slot: 110,
                selection_deadline_slot: 120,
                settlement_deadline_slot: 140,
                neutral_lamport_sink: hash(90),
            },
            DirectV3Intent::FreezeEpoch {
                market: hash(1),
                epoch: hash(2),
                reward_deposit: rewards.worst_case().unwrap(),
                rewards,
            },
            DirectV3Intent::AbortUnfrozen {
                market: hash(1),
                epoch: hash(2),
            },
            DirectV3Intent::SubmitCandidate {
                market: hash(1),
                epoch: hash(2),
                outcome_price: 2_500,
            },
            DirectV3Intent::BeginVerification {
                market: hash(1),
                epoch: hash(2),
            },
            DirectV3Intent::VerifyCandidate {
                market: hash(1),
                epoch: hash(2),
                retained_index: 0,
            },
            DirectV3Intent::FinalizeSelection {
                market: hash(1),
                epoch: hash(2),
            },
            DirectV3Intent::Settle {
                market: hash(1),
                epoch: hash(2),
            },
            DirectV3Intent::LapseEmpty {
                market: hash(1),
                epoch: hash(2),
            },
            DirectV3Intent::LapseUnselected {
                market: hash(1),
                epoch: hash(2),
            },
            DirectV3Intent::LapseSelected {
                market: hash(1),
                epoch: hash(2),
            },
        ]
    }

    fn direct_v3_request(sequence: u64, index: usize) -> Vec<u8> {
        let request = clutch_solana_reference::DirectV3Request {
            sequence,
            intent: direct_v3_intents()[index],
        };
        let mut bytes = vec![0; 13 + clutch_solana_layout::MAX_INTENT_BYTES];
        let written = request.encode(&mut bytes).unwrap();
        bytes.truncate(written);
        bytes
    }

    /// Tags 36 through 46 are one family: every encoded V3 request selects
    /// the dedicated route, decodes only through the strict V3 envelope, and
    /// the legacy decoder still refuses every one of those tags, so a V3
    /// request can never fall into a legacy direct handler.
    #[test]
    fn direct_v3_family_routes_all_or_nothing() {
        for index in 0..direct_v3_intents().len() {
            let bytes = direct_v3_request(0, index);
            assert_eq!(route_hint(&bytes), Route::DirectSelectionV3, "{index}");
            assert!(clutch_solana_reference::DirectV3Request::decode(&bytes).is_ok());
            assert!(Request::decode(&bytes).is_err(), "{index}");
            // Fail-closed with no accounts: shape refusal, never a stub.
            let refusal = process_without_accounts(&bytes);
            assert_eq!(
                refusal,
                ProgramError::Custom(ClutchError::AccountCount as u32),
                "{index}"
            );

            // Hostile envelope mutations earn the canonical V3 decode refusal.
            for mutate in [0usize, 1, 10] {
                let mut hostile = direct_v3_request(0, index);
                hostile[mutate] ^= 1;
                let expected: ProgramError = Refusal::from(
                    clutch_solana_reference::DirectV3Request::decode(&hostile).unwrap_err(),
                )
                .into();
                // A mutated action byte no longer routes to the V3 family;
                // whatever route wins must still refuse by decode.
                assert!(
                    clutch_solana_reference::DirectV3Request::decode(&hostile).is_err(),
                    "{index}/{mutate}"
                );
                if route_hint(&hostile) == Route::DirectSelectionV3 {
                    assert_eq!(process_without_accounts(&hostile), expected);
                } else {
                    assert!(
                        process(&Pubkey::new_from_array([9; 32]), &[], &hostile).is_err(),
                        "{index}/{mutate}"
                    );
                }
            }
            let mut truncated = direct_v3_request(0, index);
            truncated.pop();
            assert!(
                process(&Pubkey::new_from_array([9; 32]), &[], &truncated).is_err(),
                "{index}"
            );
        }
    }

    #[test]
    fn decode_precedes_account_checks_on_a_routed_request() {
        let valid = split_request(7, 5);
        assert_eq!(
            process_without_accounts(&valid),
            ProgramError::Custom(ClutchError::AccountCount as u32)
        );

        let mut invalid_market = valid;
        invalid_market[15..47].fill(0);
        assert_eq!(
            process_without_accounts(&invalid_market),
            ProgramError::from(Refusal::from(ReferenceError::Layout(
                clutch_solana_layout::CodecError::ZeroIdentity
            )))
        );
    }
}

#[cfg(all(test, not(feature = "profile-full")))]
mod profile_tests {
    use super::*;
    use solana_program_error::ProgramError;

    fn canonical_tag_request(tag: u8, version: u8) -> [u8; 15] {
        let mut bytes = [0u8; 15];
        bytes[0] = 0xd1;
        bytes[1] = 1;
        bytes[10] = ACTION_LAYOUT_HINT;
        bytes[11..13].copy_from_slice(&2u16.to_le_bytes());
        bytes[13] = tag;
        bytes[14] = version;
        bytes
    }

    fn canonical_extension_request(tag: u8, version: u8, action: u8) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[0] = 0xd1;
        bytes[1] = 1;
        bytes[10] = ACTION_LAYOUT_HINT;
        bytes[11..13].copy_from_slice(&3u16.to_le_bytes());
        bytes[13] = tag;
        bytes[14] = version;
        bytes[15] = action;
        bytes
    }

    #[test]
    fn every_disabled_canonical_coordinate_refuses_before_accounts() {
        let mut disabled = 0usize;
        for tag in u8::MIN..=u8::MAX {
            for version in u8::MIN..=u8::MAX {
                let Some(allocation) =
                    clutch_solana_layout::registry::classify_intent(tag, version)
                else {
                    continue;
                };
                match allocation {
                    clutch_solana_layout::registry::IntentAllocation::LegacyV3 => {
                        let expected = !capabilities::legacy_intent_enabled(tag, version)
                            && !capabilities::direct_v3_intent_enabled(tag, version);
                        let bytes = canonical_tag_request(tag, version);
                        assert_eq!(
                            disabled_canonical_tag(&bytes),
                            expected,
                            "legacy {tag}/{version}"
                        );
                        if expected {
                            disabled += 1;
                            let actual = process(&Pubkey::new_from_array([9; 32]), &[], &bytes)
                                .map_err(ProgramError::from);
                            assert_eq!(
                                actual,
                                Err(ProgramError::Custom(
                                    ClutchError::UnsupportedInstruction as u32
                                )),
                                "legacy {tag}/{version}"
                            );
                        }
                    }
                    clutch_solana_layout::registry::IntentAllocation::Extension(_) => {
                        for action in u8::MIN..=u8::MAX {
                            let allocated = capabilities::extension_intent_action_allocated(
                                tag, version, action,
                            );
                            let expected = allocated
                                && !capabilities::extension_intent_action_enabled(
                                    tag, version, action,
                                );
                            let bytes = canonical_extension_request(tag, version, action);
                            assert_eq!(
                                disabled_canonical_tag(&bytes),
                                expected,
                                "extension {tag}/{version}/{action}"
                            );
                            if expected {
                                disabled += 1;
                                let actual = process(&Pubkey::new_from_array([9; 32]), &[], &bytes)
                                    .map_err(ProgramError::from);
                                assert_eq!(
                                    actual,
                                    Err(ProgramError::Custom(
                                        ClutchError::UnsupportedInstruction as u32
                                    )),
                                    "extension {tag}/{version}/{action}"
                                );
                            }
                        }
                    }
                }
            }
        }
        assert!(disabled > 0);
    }

    #[test]
    fn malformed_envelopes_are_not_misclassified_as_disabled_tags() {
        let mut bytes = canonical_tag_request(23, clutch_solana_layout::INTENT_VERSION);
        bytes[0] ^= 1;
        assert!(!disabled_canonical_tag(&bytes));
        bytes[0] = 0xd1;
        bytes[1] ^= 1;
        assert!(!disabled_canonical_tag(&bytes));
        assert!(!disabled_canonical_tag(&bytes[..13]));
    }
}

#[cfg(test)]
mod extension_registry_tests {
    extern crate std;

    use super::*;
    use solana_program_error::ProgramError;
    use std::vec::Vec;

    fn extension_request(family_tag: u8, family_version: u8, local_action: u8) -> Vec<u8> {
        let mut bytes = vec![0_u8; 16];
        bytes[0] = 0xd1;
        bytes[1] = 1;
        bytes[10] = ACTION_LAYOUT_HINT;
        bytes[11..13].copy_from_slice(&3_u16.to_le_bytes());
        bytes[13] = family_tag;
        bytes[14] = family_version;
        bytes[15] = local_action;
        bytes
    }

    #[test]
    fn every_allocated_extension_action_refuses_before_accounts() {
        for local_action in clutch_solana_layout::registry::GeneralV2Action::FIRST_TAG
            ..=clutch_solana_layout::registry::GeneralV2Action::LAST_TAG
        {
            let bytes = extension_request(
                clutch_solana_layout::registry::GENERAL_V2_FAMILY_TAG,
                clutch_solana_layout::registry::GENERAL_V2_FAMILY_VERSION,
                local_action,
            );
            let enabled = capabilities::extension_intent_action_enabled(
                clutch_solana_layout::registry::GENERAL_V2_FAMILY_TAG,
                clutch_solana_layout::registry::GENERAL_V2_FAMILY_VERSION,
                local_action,
            );
            assert_eq!(
                disabled_canonical_tag(&bytes),
                !enabled,
                "action {local_action}"
            );
            let actual =
                process(&Pubkey::new_from_array([9; 32]), &[], &bytes).map_err(ProgramError::from);
            if enabled {
                assert_ne!(
                    actual,
                    Err(ProgramError::from(ClutchError::UnsupportedInstruction)),
                    "enabled action {local_action} must reach its strict payload decoder"
                );
            } else {
                assert_eq!(
                    actual,
                    Err(ProgramError::from(ClutchError::UnsupportedInstruction)),
                    "disabled action {local_action}"
                );
            }
        }
        for local_action in clutch_solana_layout::registry::SourceSeriesAction::FIRST_TAG
            ..=clutch_solana_layout::registry::SourceSeriesAction::LAST_TAG
        {
            let bytes = extension_request(
                clutch_solana_layout::registry::SOURCE_SERIES_FAMILY_TAG,
                clutch_solana_layout::registry::SOURCE_SERIES_FAMILY_VERSION,
                local_action,
            );
            assert!(
                disabled_canonical_tag(&bytes),
                "source action {local_action}"
            );
            assert_eq!(
                process(&Pubkey::new_from_array([9; 32]), &[], &bytes).map_err(ProgramError::from),
                Err(ProgramError::from(ClutchError::UnsupportedInstruction)),
                "source action {local_action}"
            );
        }
        for local_action in clutch_solana_layout::registry::RecurringSeriesAction::FIRST_TAG
            ..=clutch_solana_layout::registry::RecurringSeriesAction::LAST_TAG
        {
            let bytes = extension_request(
                clutch_solana_layout::registry::SOURCE_SERIES_FAMILY_TAG,
                clutch_solana_layout::registry::SOURCE_SERIES_FAMILY_VERSION,
                local_action,
            );
            assert!(
                disabled_canonical_tag(&bytes),
                "series action {local_action}"
            );
            assert_eq!(
                process(&Pubkey::new_from_array([9; 32]), &[], &bytes).map_err(ProgramError::from),
                Err(ProgramError::from(ClutchError::UnsupportedInstruction)),
                "series action {local_action}"
            );
        }
        for local_action in clutch_solana_layout::registry::StructuredClaimAction::FIRST_TAG
            ..=clutch_solana_layout::registry::StructuredClaimAction::LAST_TAG
        {
            let bytes = extension_request(
                clutch_solana_layout::registry::STRUCTURED_CLAIM_FAMILY_TAG,
                clutch_solana_layout::registry::STRUCTURED_CLAIM_FAMILY_VERSION,
                local_action,
            );
            assert!(
                disabled_canonical_tag(&bytes),
                "structured-claim action {local_action}"
            );
            assert_eq!(
                process(&Pubkey::new_from_array([9; 32]), &[], &bytes).map_err(ProgramError::from),
                Err(ProgramError::from(ClutchError::UnsupportedInstruction)),
                "structured-claim action {local_action}"
            );
        }
        for local_action in clutch_solana_layout::registry::RecoveryAction::FIRST_TAG
            ..=clutch_solana_layout::registry::RecoveryAction::LAST_TAG
        {
            let bytes = extension_request(
                clutch_solana_layout::registry::RECOVERY_FAMILY_TAG,
                clutch_solana_layout::registry::RECOVERY_FAMILY_VERSION,
                local_action,
            );
            assert!(
                disabled_canonical_tag(&bytes),
                "recovery action {local_action}"
            );
            assert_eq!(
                process(&Pubkey::new_from_array([9; 32]), &[], &bytes).map_err(ProgramError::from),
                Err(ProgramError::from(ClutchError::UnsupportedInstruction)),
                "recovery action {local_action}"
            );
        }
    }

    #[test]
    fn unknown_family_version_and_local_action_do_not_gain_capability() {
        for (family_tag, family_version, local_action) in [
            (74, 2, 1),
            (74, 1, 0),
            (74, 1, 39),
            (75, 1, 0),
            (75, 1, 9),
            (77, 2, 0),
            (77, 2, 19),
            (78, 1, 0),
            (78, 1, 10),
            (79, 1, 1),
        ] {
            let bytes = extension_request(family_tag, family_version, local_action);
            assert!(!disabled_canonical_tag(&bytes));
            assert!(process(&Pubkey::new_from_array([9; 32]), &[], &bytes).is_err());
        }
    }

    #[test]
    fn malformed_outer_lengths_are_never_classified_as_disabled() {
        let mut bytes = extension_request(74, 1, 1);
        bytes[11..13].copy_from_slice(&4_u16.to_le_bytes());
        assert!(!disabled_canonical_tag(&bytes));
        bytes[11..13].copy_from_slice(&3_u16.to_le_bytes());
        bytes.push(0);
        assert!(!disabled_canonical_tag(&bytes));
        assert!(!disabled_canonical_tag(&bytes[..15]));
    }
}
