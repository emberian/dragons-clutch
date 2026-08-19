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
use crate::error::ClutchError;
use crate::instructions::{
    artifact, cash_exit, direct_selection, external_exit, genesis, market_init, merge_materialize,
    observe_resolve, orders_batch, source_ingest, split,
};
use clutch_solana_layout::Intent;
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
    SourceIngest,
    DirectSelection,
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

fn route_hint(instruction_data: &[u8]) -> Route {
    match instruction_data.get(10).copied() {
        Some(ACTION_LAYOUT_HINT) => match instruction_data.get(13).copied() {
            Some(INTENT_SPLIT_HINT) => Route::Split,
            Some(INTENT_MERGE_HINT | INTENT_MATERIALIZE_HINT | INTENT_DEMATERIALIZE_HINT) => {
                Route::MergeMaterialize
            }
            Some(INTENT_CREATE_MARKET_HINT) => Route::MarketInit,
            Some(INTENT_FEED_ADVANCE_HINT) => Route::ObserveResolve,
            Some(INTENT_REDEEM_EXTERNAL_HINT) => Route::ExternalExit,
            Some(INTENT_WITHDRAW_CASH_HINT) => Route::CashExit,
            Some(
                INTENT_BEGIN_ARTIFACT_HINT
                | INTENT_WRITE_ARTIFACT_HINT
                | INTENT_SEAL_ARTIFACT_HINT
                | INTENT_ABORT_ARTIFACT_HINT,
            ) => Route::Artifact,
            Some(
                INTENT_PLACE_ORDER_HINT
                | INTENT_CANCEL_ORDER_HINT
                | INTENT_SETTLE_PAGE_HINT
                | INTENT_SUBMIT_DIRECT_PAGE_HINT,
            ) => Route::OrdersBatch,
            Some(
                INTENT_INIT_REALM_HINT
                | INTENT_INIT_PROFILE_HINT
                | INTENT_INIT_PRICE_GRID_HINT
                | INTENT_INIT_TERMS_HINT
                | INTENT_INIT_ORDER_PAGE_HINT
                | INTENT_ENDOW_HINT,
            ) => Route::Genesis,
            Some(
                INTENT_INIT_SOURCE_SPEC_HINT
                | INTENT_INIT_SOURCE_ARCHIVE_HINT
                | INTENT_APPEND_SOURCE_ARCHIVE_HINT
                | INTENT_SEAL_SOURCE_ARCHIVE_HINT,
            ) => Route::SourceIngest,
            Some(
                INTENT_INIT_DIRECT_EPOCH_V3_HINT
                | INTENT_FREEZE_DIRECT_EPOCH_V3_HINT
                | INTENT_SUBMIT_DIRECT_CANDIDATE_V2_HINT
                | INTENT_SELECT_DIRECT_WINDOW_V1_HINT
                | INTENT_SETTLE_DIRECT_V2_HINT,
            ) => Route::DirectSelection,
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
        Route::SourceIngest => process_source_ingest(program_id, accounts, instruction_data),
        Route::DirectSelection => process_direct_selection(program_id, accounts, instruction_data),
        Route::DecodeOnly => decode_only(instruction_data),
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
        Action::Layout(Intent::FeedAdvance { .. })
        | Action::Resolve { .. }
        | Action::RedeemInternal { .. } => observe_resolve::process(program_id, accounts, &request),
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
        Action::Layout(Intent::PlaceOrder { .. })
        | Action::Layout(Intent::CancelOrder { .. })
        | Action::Layout(Intent::SubmitDirectPage { .. })
        | Action::Layout(Intent::SettlePage { .. }) => {
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
        | Action::Layout(Intent::Endow { .. }) => genesis::process(program_id, accounts, &request),
        _ => unexpected_route(),
    }
}

#[inline(never)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Refusal;
    use clutch_solana_layout::{
        artifact::{ArtifactKind, ARTIFACT_CHUNK_BYTES},
        canonical_order_id, Hash32, Intent, OrderRecord, OrderSlot, MAX_INTENT_BYTES, MAX_OUTCOMES,
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
