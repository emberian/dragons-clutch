//! Family-neutral Product terminal settlement through SignedDelta and Custody.
//!
//! # Who may settle, and what they prove
//!
//! The request's execution role selects the proof that must stand at coordinate
//! 0, which the frame spec pins as a signer in every mode:
//!
//! | role | coordinate 0 | the entitled party |
//! |---|---|---|
//! | `Core` | `CallerAuthoritySeedsV1` PDA under the Core program | a lifecycle orchestration |
//! | `Trading` | the same PDA under the activated Trading program | a venue transition |
//! | `Claims` | the Position owner's own signature | the wallet that holds the claims |
//!
//! `Core` and `Trading` are EXTERNAL callers: they reach this route by CPI, and
//! only the Registry-activated program of that role can sign its authority PDA.
//! `Claims` names the case with no external caller at all — this program running
//! top-level — so there is no caller program to derive a PDA under, and the
//! authority is the one the Position itself names.
//!
//! **A signature at coordinate 0 equal to `request.owner` is by itself the proof
//! that the Position is wallet-held.** A program-derived address has no private
//! key, so a Trading record owner and a Claims capability owner cannot produce
//! it; the route needs no owner-kind tag and no extra account to tell the
//! families apart. The chain closes downstream without a second guard:
//! `product_basis_terminal_v3::validate_joins` refuses unless the Position
//! header's `owner` equals `request.owner`, and `signed_delta_v3`'s
//! `build_candidates` refuses unless the Position account IS the canonical PDA
//! under `(aggregate, owner)`.
//!
//! Nothing else about the route changes with the role. One evaluator computes
//! the payout, one Custody transfer moves it, one receipt records it — which is
//! why this is a widened admission and not a second route. See
//! `docs/decisions/0008-custody-namespace-owner.md` §8.
//!
//! **LBV2 Positions carry no lien.** The state is a header plus a balance
//! vector with a zero-checked reserved word; there is no escrow, lock or
//! delegate field. So owner authorization at terminal cannot bypass an
//! encumbrance — there is none to bypass.

// Every private phase is reachable only after `process` proves the one exact
// 36-account frame. Fixed indexing below therefore cannot observe a short
// untrusted slice and keeps the SBF frame materially smaller than a duplicated
// 36-reference view.
#![allow(clippy::indexing_slicing)]

extern crate alloc;

use alloc::{boxed::Box, vec, vec::Vec};

use dclutch_capability_contract::funding::funded_rent_persists_v1;
use dclutch_claims_svm::{
    CallerRole,
    liability_basis_state_v2::LiabilityBasisMarketViewV2,
    product_basis_terminal_v3::{
        ProductClaimsTerminalAdmissionV3, ProductClaimsTerminalInputV3,
        encode_product_claims_terminal_signed_delta_v3,
    },
    signed_delta_v3::{SignedDeltaV3, plan_bytes},
    terminal_settlement_v3::{
        TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 as ACCOUNT_COUNT,
        TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3,
        TERMINAL_SETTLEMENT_CERTIFICATE_ACCOUNT_V3 as CERTIFICATE,
        TERMINAL_SETTLEMENT_COLLATERAL_MINT_ACCOUNT_V3 as COLLATERAL_MINT,
        TERMINAL_SETTLEMENT_CUSTODY_AUTHORITY_ACCOUNT_V3 as CUSTODY_AUTHORITY,
        TERMINAL_SETTLEMENT_CUSTODY_CALLER_ACCOUNT_V3 as CUSTODY_CALLER,
        TERMINAL_SETTLEMENT_CUSTODY_PROGRAM_ACCOUNT_V3 as CUSTODY_PROGRAM,
        TERMINAL_SETTLEMENT_CUSTODY_REPLAY_ACCOUNT_V3 as CUSTODY_REPLAY,
        TERMINAL_SETTLEMENT_EXPOSURE_RAW_ACCOUNT_V3 as EXPOSURE_RAW,
        TERMINAL_SETTLEMENT_EXPOSURE_STAGING_ACCOUNT_V3 as EXPOSURE_STAGING,
        TERMINAL_SETTLEMENT_HOARD_ACCOUNT_V3 as HOARD, TERMINAL_SETTLEMENT_POST_RESOURCE_DOMAIN_V3,
        TERMINAL_SETTLEMENT_REALM_ACCOUNT_V3 as REALM,
        TERMINAL_SETTLEMENT_REALM_STAGING_ACCOUNT_V3 as REALM_STAGING,
        TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3 as RECIPIENT,
        TERMINAL_SETTLEMENT_RESOLUTION_PROGRAM_ACCOUNT_V3 as RESOLUTION_PROGRAM,
        TERMINAL_SETTLEMENT_RESOLUTION_PROGRAMDATA_ACCOUNT_V3 as RESOLUTION_PROGRAMDATA,
        TERMINAL_SETTLEMENT_TOKEN_POSTSTATE_DOMAIN_V3,
        TERMINAL_SETTLEMENT_TOKEN_PROGRAM_ACCOUNT_V3 as TOKEN_PROGRAM,
        TerminalSettlementReceiptInputV3, TerminalSettlementReceiptV3, TerminalSettlementRequestV3,
    },
};
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1,
    CustodyReplaySeedsV1, CustodyReplayV1, CustodyVaultSeedsV1,
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, STATE_BYTES};
use dclutch_product_runtime_v2::ContentId as ProductContentId;
use dclutch_product_runtime_v2_svm_reader::{
    FinalizedRecordFrameV2, ProductRuntimeFrameV3, authenticate_product_runtime_v3,
};
use dclutch_realm_contract::REALM_SCHEMA_RELEASE_ID_V1;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_representation_composition_v3_kernel::{
    COMPOSITION_EXPOSURE_SCHEMA_ID_V3, CompositionExposureBundleV3, RecordAdmissionV3,
};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    program::set_return_data,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use super::{
    ClaimsSbfError,
    liability_basis_v2::LIABILITY_BASIS_MARKET_SEED_V2,
    market_admission_v1::CLAIMS_SETTLED_MARKET_ADMISSIBLE_PRESTATES_V1,
    rational_terminal_v3::{
        RationalTerminalFrameV3, TerminalCustodyInputV3, execute_terminal_custody_v3, token_amount,
    },
    signed_delta_v3::{
        AuthenticatedSignedDeltaParentV3, ParentAuthorityV3, SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
        execute_parent_authenticated,
    },
    terminal_certificate_v3::{
        TerminalCertificateFrameV3, authenticate_terminal_certificate_scenario_v3,
    },
};

pub(super) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if accounts.len() != ACCOUNT_COUNT {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let request = TerminalSettlementRequestV3::decode(instruction_data)
        .map_err(|_| ClaimsSbfError::Instruction)?;
    let request_digest = hash(instruction_data).to_bytes();
    let authority = parent_authority(request.input());
    let prepared =
        authenticate_and_prepare(program_id, accounts, &request, request_digest, authority)?;
    let receipt = execute(
        program_id,
        accounts,
        &request,
        request_digest,
        prepared,
        authority,
    )?;
    set_return_data(&receipt.to_bytes());
    Ok(())
}

/// Execute a chain-derived terminal request from an enclosing Claims family route.
///
/// The outer route authenticates its own exact family-request PDA here; the
/// derived terminal request remains separately hashed and evidenced in the
/// returned terminal receipt. No public terminal submission can select this
/// mode.
pub(crate) fn execute_enclosing_authenticated(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: TerminalSettlementRequestV3,
    outer_context: [u8; 32],
    outer_request_digest: [u8; 32],
) -> Result<TerminalSettlementReceiptV3, ProgramError> {
    if accounts.len() != ACCOUNT_COUNT || request.input().caller_role != CallerRole::Trading {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let input = request.input();
    let seeds = CallerAuthoritySeedsV1::new(
        dclutch_core_contract::ContentId::new(input.release_set)
            .map_err(|_| ClaimsSbfError::Authority)?,
        input.market,
        ExecutionRoleV1::Trading,
        outer_context,
        outer_request_digest,
    )
    .map_err(|_| ClaimsSbfError::Authority)?;
    if !accounts[0].is_signer
        || accounts[0].key != &Pubkey::find_program_address(&seeds.as_slices(), accounts[14].key).0
        || input.parent_context != outer_request_digest
    {
        return Err(ClaimsSbfError::Authority.into());
    }
    let request_bytes = request.to_bytes();
    let request_digest = hash(&request_bytes).to_bytes();
    let prepared = authenticate_and_prepare(
        program_id,
        accounts,
        &request,
        request_digest,
        ParentAuthorityV3::EnclosingClaimsRoute,
    )?;
    execute(
        program_id,
        accounts,
        &request,
        request_digest,
        prepared,
        ParentAuthorityV3::EnclosingClaimsRoute,
    )
}

/// Execute one terminal settlement on behalf of an absent holder.
///
/// The compaction crank's entry, and the whole of what it changes is the proof
/// carried at coordinate 0. Every other authentication in this module runs
/// unaltered: the same aggregate join, the same Core phase gate, the same
/// Custody replay cursor, the same payout derivation, the same receipt.
///
/// That is deliberate and it is the architecture. Compaction pays what the
/// holder's own redemption would have paid because it *is* the holder's own
/// redemption, executed by somebody else into an escrow only the holder can
/// open. A compaction that re-derived the payout could pay a different number
/// than redemption and pass its own tests; one that calls this cannot.
///
/// Not a public submission mode: the entry is crate-private and the caller must
/// have proved the deadline and derived the recipient first.
pub(crate) fn execute_claim_check_compaction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: TerminalSettlementRequestV3,
) -> Result<TerminalSettlementReceiptV3, ProgramError> {
    if accounts.len() != ACCOUNT_COUNT || request.input().caller_role != CallerRole::Claims {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let request_bytes = request.to_bytes();
    let request_digest = hash(&request_bytes).to_bytes();
    let prepared = authenticate_and_prepare(
        program_id,
        accounts,
        &request,
        request_digest,
        ParentAuthorityV3::ClaimCheckCrank,
    )?;
    execute(
        program_id,
        accounts,
        &request,
        request_digest,
        prepared,
        ParentAuthorityV3::ClaimCheckCrank,
    )
}

struct PreparedTerminalSettlementV3 {
    packet: Vec<u8>,
    payout: u64,
    market: LiabilityBasisMarketViewV2,
    terminal_digest: [u8; 32],
}

#[inline(never)]
fn authenticate_and_prepare(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &TerminalSettlementRequestV3,
    request_digest: [u8; 32],
    authority: ParentAuthorityV3,
) -> Result<Box<PreparedTerminalSettlementV3>, ProgramError> {
    let input = (*request).input();
    authenticate_extra_privileges(program_id, accounts, input, authority)?;
    let aggregate_bytes = accounts[1]
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let market = LiabilityBasisMarketViewV2::decode(&aggregate_bytes)
        .map_err(|_| ClaimsSbfError::Economic)?;
    if market.logical_market != input.market
        || market.release_set != input.release_set
        || market.realm_id != input.realm
        || market.basis_id != input.semantic_basis_id
        || market.generation != input.generation
        || market.revision != input.expected_market_revision
        || market.registry_program != accounts[13].key.to_bytes()
        || accounts[1].owner != program_id
        || accounts[1].key
            != &Pubkey::find_program_address(
                &[LIABILITY_BASIS_MARKET_SEED_V2, input.market.as_slice()],
                program_id,
            )
            .0
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    drop(aggregate_bytes);
    let core = authenticate_core(accounts, market)?;
    let terminal_digest = core
        .terminal_receipt
        .ok_or(ClaimsSbfError::Identity)?
        .to_bytes();
    if terminal_digest != input.terminal_record_digest {
        return Err(ClaimsSbfError::Identity.into());
    }
    let runtime = authenticate_product_runtime_v3(
        accounts[13].key,
        ProductContentId::new(input.product_record_digest).map_err(|_| ClaimsSbfError::Identity)?,
        ProductRuntimeFrameV3 {
            product: record(&accounts[4], &accounts[5]),
            result_domain: record(&accounts[6], &accounts[7]),
            portfolio: record(&accounts[8], &accounts[9]),
            linked_basis: record(&accounts[2], &accounts[3]),
        },
    )
    .map_err(|_| ClaimsSbfError::Identity)?;
    if runtime.runtime.product_record.content_digest.to_bytes() != input.product_record_digest
        || runtime.runtime.product_id.to_bytes() != market.product_instance_id
        || runtime.semantic_basis_id.to_bytes() != market.basis_id
        || runtime.linked_basis_record.content_digest.to_bytes() != input.linked_basis_record_digest
        || core.identity.product_record.to_bytes() != input.product_record_digest
        || core.identity.product_id.to_bytes() != market.product_instance_id
        || core.identity.selected_release_set.to_bytes() != input.release_set
        || core.identity.market_id.to_bytes() != input.market
        || core.identity.generation != input.generation
        || core.terminal_winner >= runtime.runtime.outcome_count
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    authenticate_finalized_record(
        &accounts[EXPOSURE_RAW],
        &accounts[EXPOSURE_STAGING],
        accounts[13].key,
        COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
        input.exposure_digest,
    )?;
    let scenario = authenticate_terminal_certificate_scenario_v3(
        TerminalCertificateFrameV3 {
            registry: &accounts[13],
            cache: &accounts[12],
            resolution_program: &accounts[RESOLUTION_PROGRAM],
            resolution_programdata: &accounts[RESOLUTION_PROGRAMDATA],
            certificate: &accounts[CERTIFICATE],
        },
        input.release_set,
        core,
        runtime.basis_kind,
        runtime.runtime.outcome_count,
    )?;
    let exposure_bytes = accounts[EXPOSURE_RAW]
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let basis_bytes = accounts[2]
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let market_bytes = accounts[1]
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let position_bytes = accounts[20]
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let admission = ProductClaimsTerminalAdmissionV3::new(
        input.exposure_id,
        input.exposure_digest,
        runtime.runtime.product_id.to_bytes(),
        runtime
            .runtime
            .result_domain_record
            .content_digest
            .to_bytes(),
        runtime.runtime.coordinate_domain_id.to_bytes(),
        runtime.runtime.result_unit_id.to_bytes(),
        runtime.semantic_basis_id.to_bytes(),
        runtime.linked_basis_record.content_digest.to_bytes(),
        input.market,
        input.release_set,
        runtime.evaluator_release_id.to_bytes(),
        runtime.basis_width,
        runtime.payout_scale,
    )
    .map_err(|_| ClaimsSbfError::Economic)?;
    let product_width =
        usize::try_from(runtime.runtime.outcome_count).map_err(|_| ClaimsSbfError::Economic)?;
    let claims_width = usize::try_from(market.claim_count).map_err(|_| ClaimsSbfError::Economic)?;
    let neutral = SignedDeltaV3::new(
        dclutch_claims_svm::signed_delta_v3::DeltaDirectionV3::Neutral,
        0,
    )
    .map_err(|_| ClaimsSbfError::Economic)?;
    let mut product_scratch = vec![0_u64; product_width];
    let mut translation_scratch = vec![0_u64; claims_width];
    let mut claims_scratch = vec![0_u64; claims_width];
    let mut aggregate_scratch = vec![neutral; claims_width];
    let mut packet =
        vec![0_u8; plan_bytes(market.claim_count, 1, 1).map_err(|_| ClaimsSbfError::Economic)?];
    let hoard_before = token_amount(
        &accounts[HOARD],
        &accounts[TOKEN_PROGRAM],
        input.collateral_mint,
        accounts[CUSTODY_AUTHORITY].key.to_bytes(),
    )?;
    let exposure_admission = RecordAdmissionV3 {
        selected_id: input.exposure_id,
        finalized_id: input.exposure_id,
        recomputed_digest: hash(&exposure_bytes).to_bytes(),
        finalized_digest: input.exposure_digest,
        record_authenticated: true,
    };
    // BEFORE any payout is derived and before any byte is written. See
    // `require_identity_exposure_v3`.
    require_identity_exposure_v3(&exposure_bytes, exposure_admission)?;
    let payout = encode_product_claims_terminal_signed_delta_v3(
        ProductClaimsTerminalInputV3 {
            product_basis_bytes: &basis_bytes,
            admission,
            composition_exposure_bytes: &exposure_bytes,
            composition_exposure_admission: exposure_admission,
            product_record_digest: input.product_record_digest,
            market_account: accounts[1].key.to_bytes(),
            market_bytes: &market_bytes,
            position_bytes: &position_bytes,
            owner: input.owner,
            request_id: request_digest,
            caller_role: input.caller_role,
            terminal: scenario,
            claim_index: input.claim_index,
            quantity: input.quantity,
            expected_generation: input.generation,
            expected_market_revision: input.expected_market_revision,
            expected_position_revision: input.expected_position_revision,
            hoard_before,
        },
        &mut product_scratch,
        &mut translation_scratch,
        &mut claims_scratch,
        &mut aggregate_scratch,
        &mut packet,
    )
    .map_err(|_| ClaimsSbfError::Economic)?;
    Ok(Box::new(PreparedTerminalSettlementV3 {
        packet,
        payout,
        market,
        terminal_digest,
    }))
}

#[inline(never)]
fn execute(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &TerminalSettlementRequestV3,
    request_digest: [u8; 32],
    prepared: Box<PreparedTerminalSettlementV3>,
    authority: ParentAuthorityV3,
) -> Result<TerminalSettlementReceiptV3, ProgramError> {
    let input = (*request).input();
    let signed_accounts = Vec::from(&accounts[..SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 + 1]);
    let signed_receipt = execute_parent_authenticated(
        program_id,
        &signed_accounts,
        &prepared.packet,
        AuthenticatedSignedDeltaParentV3 {
            caller_role: input.caller_role,
            authority,
            release_set: input.release_set,
            market: input.market,
            parent_context: input.parent_context,
            parent_request_digest: request_digest,
        },
        // A terminal settlement necessarily runs on a resolved Market, so the
        // enclosed signed delta must expect a settled phase. Expecting Open
        // here is unsatisfiable: CoreState only carries the terminal receipt
        // this route requires once the Market has left Open. It admits
        // Retiring as well as Terminal because `begin_retiring` is
        // permissionless, and a redemption that refused there would hand any
        // stranger the power to end this holder's claim.
        CLAIMS_SETTLED_MARKET_ADMISSIBLE_PRESTATES_V1,
    )?;
    let packet_digest = hash(&prepared.packet).to_bytes();
    if signed_receipt.packet_digest() != packet_digest
        || signed_receipt.request_id() != request_digest
        || signed_receipt.table_digest().iter().all(|byte| *byte == 0)
        || signed_receipt
            .post_resource_digest()
            .iter()
            .all(|byte| *byte == 0)
    {
        return Err(ClaimsSbfError::Receipt.into());
    }
    let candidate_digest = hashv(&[
        TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3,
        &request_digest,
        &packet_digest,
        &prepared.payout.to_le_bytes(),
        &input.exposure_digest,
        &prepared.terminal_digest,
    ])
    .to_bytes();
    let frame = Box::new(shared_frame(accounts));
    let custody = execute_terminal_custody_v3(
        program_id,
        frame.as_ref(),
        TerminalCustodyInputV3 {
            release_set: input.release_set,
            market: input.market,
            realm: input.realm,
            parent_request_digest: request_digest,
            recipient_owner: input.recipient_owner,
            generation: input.generation,
            order_nonce: input.expected_position_revision,
            transfer_index: input.transfer_index,
            expected_custody_revision: input.expected_custody_revision,
            payout: prepared.payout,
            candidate_digest,
            custody_context: prepared.market.custody_context,
        },
    )?;
    if prepared.payout == 0 {
        authenticate_zero_custody_accounts(accounts, input, prepared.market.custody_context)?;
    }
    let replay_digest =
        custody_replay_digest(accounts, input.expected_custody_revision, prepared.payout)?;
    let token_poststate_digest = token_poststate_digest(accounts)?;
    let (custody_request_digest, custody_receipt_digest, post_custody_revision) =
        if let Some(custody) = custody {
            if custody.replay_digest != replay_digest {
                return Err(ClaimsSbfError::Receipt.into());
            }
            (
                custody.request_digest,
                custody.receipt_digest,
                input
                    .expected_custody_revision
                    .checked_add(1)
                    .ok_or(ClaimsSbfError::Receipt)?,
            )
        } else {
            ([0; 32], [0; 32], input.expected_custody_revision)
        };
    let post_resource_digest = hashv(&[
        TERMINAL_SETTLEMENT_POST_RESOURCE_DOMAIN_V3,
        &request_digest,
        &signed_receipt.post_resource_digest(),
        &replay_digest,
        &token_poststate_digest,
        &custody_receipt_digest,
    ])
    .to_bytes();
    let evidence = Box::new(TerminalSettlementReceiptInputV3 {
        request_digest,
        signed_packet_digest: packet_digest,
        signed_table_digest: signed_receipt.table_digest(),
        signed_post_resource_digest: signed_receipt.post_resource_digest(),
        custody_request_digest,
        custody_receipt_digest,
        custody_replay_digest: replay_digest,
        custody_token_poststate_digest: token_poststate_digest,
        post_resource_digest,
        payout: prepared.payout,
        pre_market_revision: input.expected_market_revision,
        post_market_revision: signed_receipt.post_market_revision(),
        pre_position_revision: input.expected_position_revision,
        post_position_revision: input
            .expected_position_revision
            .checked_add(1)
            .ok_or(ClaimsSbfError::Receipt)?,
        pre_custody_revision: input.expected_custody_revision,
        post_custody_revision,
    });
    emit_receipt(*request, evidence)
}

#[inline(never)]
fn emit_receipt(
    request: TerminalSettlementRequestV3,
    evidence: Box<TerminalSettlementReceiptInputV3>,
) -> Result<TerminalSettlementReceiptV3, ProgramError> {
    let receipt = TerminalSettlementReceiptV3::new(request, *evidence)
        .map_err(|_| ClaimsSbfError::Receipt)?;
    Ok(receipt)
}

fn authenticate_extra_privileges(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    input: dclutch_claims_svm::terminal_settlement_v3::TerminalSettlementRequestInputV3,
    authority: ParentAuthorityV3,
) -> Result<(), ProgramError> {
    for index in [
        EXPOSURE_RAW,
        EXPOSURE_STAGING,
        CERTIFICATE,
        REALM,
        REALM_STAGING,
        COLLATERAL_MINT,
        CUSTODY_AUTHORITY,
    ] {
        let account = &accounts[index];
        if account.is_signer || account.is_writable || account.executable {
            return Err(ClaimsSbfError::Accounts.into());
        }
    }
    if accounts[CUSTODY_CALLER].is_signer
        || accounts[CUSTODY_CALLER].is_writable
        || !accounts[CUSTODY_PROGRAM].executable
        || accounts[CUSTODY_PROGRAM].is_writable
        || !accounts[RESOLUTION_PROGRAM].executable
        || accounts[RESOLUTION_PROGRAM].is_signer
        || accounts[RESOLUTION_PROGRAM].is_writable
        || accounts[RESOLUTION_PROGRAMDATA].executable
        || accounts[RESOLUTION_PROGRAMDATA].is_signer
        || accounts[RESOLUTION_PROGRAMDATA].is_writable
        || !accounts[CUSTODY_REPLAY].is_writable
        || !accounts[HOARD].is_writable
        || !accounts[RECIPIENT].is_writable
        || !accounts[TOKEN_PROGRAM].executable
        || accounts[16].key != program_id
        || accounts[16].key.to_bytes() != input.claims_program
        || accounts[CUSTODY_PROGRAM].key.to_bytes() != input.custody_program
        || accounts[RECIPIENT].key.to_bytes() != input.recipient_token_account
        || accounts[COLLATERAL_MINT].key.to_bytes() != input.collateral_mint
        || accounts[TOKEN_PROGRAM].key.to_bytes() != input.token_program
        || accounts[20].key.to_bytes() != input.position
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    // Coordinate 0 is a signer in every mode; WHOSE signature is what the role
    // selects. Under `Claims` there is no caller program to derive an authority
    // PDA under, and the entitled party is the Position's owner. Binding the
    // signer to `input.owner` closes the chain by itself: the evaluator refuses
    // unless the Position header's `owner` equals it
    // (`product_basis_terminal_v3::validate_joins`), and `build_candidates`
    // refuses unless the Position account IS the canonical PDA under
    // `(aggregate, owner)`. A Trading record owner and a Claims capability owner
    // are both program-derived addresses with no key, so neither can produce
    // this proof -- which is why the route needs no owner-kind tag to tell a
    // wallet-held Position from resting inventory or a capability shard.
    //
    // The one exception is the permissionless compaction crank, and it is an
    // exception to WHO signs rather than to whether anyone does. Coordinate 0
    // is then the cranker, who is anybody; what entitles the crank is the
    // elapsed deadline and the derived recipient, both proved by
    // `claim_check_compaction_v1` before this mode can be selected. The
    // wallet-held proof the owner's signature was also silently carrying is
    // replaced there by the persisted owner-kind tag, not dropped.
    if input.caller_role == CallerRole::Claims
        && !matches!(authority, ParentAuthorityV3::ClaimCheckCrank)
        && accounts[0].key.to_bytes() != input.owner
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    Ok(())
}

/// Which proof coordinate 0 must carry for this request's execution role.
const fn parent_authority(
    input: dclutch_claims_svm::terminal_settlement_v3::TerminalSettlementRequestInputV3,
) -> ParentAuthorityV3 {
    match input.caller_role {
        CallerRole::Claims => ParentAuthorityV3::PositionOwner(input.owner),
        CallerRole::Core | CallerRole::Trading => ParentAuthorityV3::CallerProgramPda,
    }
}

fn authenticate_core(
    accounts: &[AccountInfo<'_>],
    market: LiabilityBasisMarketViewV2,
) -> Result<CoreState, ProgramError> {
    if accounts[11].owner != accounts[18].key || accounts[11].data_len() != STATE_BYTES {
        return Err(ClaimsSbfError::Identity.into());
    }
    let bytes = accounts[11]
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let core = CoreState::decode(&bytes).map_err(|_| ClaimsSbfError::Identity)?;
    if !CLAIMS_SETTLED_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(core.phase)
        || accounts[11].key
            != &Pubkey::find_program_address(
                &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
                accounts[18].key,
            )
            .0
        || core.identity.market_id.to_bytes() != market.logical_market
        || core.identity.product_id.to_bytes() != market.product_instance_id
        || core.identity.selected_release_set.to_bytes() != market.release_set
        || core.identity.registry_program.to_bytes() != accounts[13].key.to_bytes()
        || core.identity.generation != market.generation
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(core)
}

/// Refuse any Product-to-Claims exposure that is not the identity embedding.
///
/// # The hole this closes
///
/// The exposure is the matrix that decides WHICH claim a terminal payout goes
/// to. Until this check, the redeemer chose it. `exposure_id` and
/// `exposure_digest` are ordinary instruction fields
/// (`crates/dclutch-claims-svm/src/terminal_settlement_v3.rs:151-154`);
/// [`authenticate_finalized_record`] proves only that some finalized record
/// hashes to that digest -- it reads no Market and no Product; and
/// `verify_execution_for`
/// (`crates/dclutch-representation-composition-v3-kernel/src/exposure.rs:383-401`)
/// joins five header fields the record's own author writes, plus two widths.
/// Registry publication is permissionless
/// (`programs/dclutch-registry-sbf/src/record_v1.rs:1`), and founding pins no
/// exposure identity at all -- `generic_founding_v1.rs` contains no occurrence
/// of exposure, graph, composition or descriptor. So nothing upstream ever
/// fixed a recipe for the redeemer's choice to be measured against.
///
/// The conjunct that looked like it would have caught a substitution never
/// could: `exposure.bundle_id() != admission.exposure_id()` compared
/// `composition_exposure_admission.selected_id` -- which is where `bundle_id`
/// was assigned from (`exposure.rs:274`) -- against a second copy of the same
/// caller-supplied value. **It was a comparison of a value with itself, on both
/// this route and the Rational one**, and it has been deleted
/// (`product_basis_terminal_v3.rs`, `decode_exposure`). It is not replaced: the
/// obvious repair, reading the record's own `graph_id()`, measured false
/// against four fixtures because that field and the descriptor's `graph_id` are
/// different identities in this tree -- decision 0011's RECORDS-MIGRATE row.
/// The Rational route is sound regardless, because it anchors the record's
/// BYTES to an authenticated descriptor's `graph_digest`
/// (`crates/dclutch-rational-representation-v2-kernel/src/product_v3.rs:530-533`);
/// that anchor is what does the work there, not the identity conjunct.
///
/// # Why the identity, and why that is not a new policy
///
/// The Claims coordinates ARE the Product's own outcome coordinates, and that
/// is established twice over — once when the aggregate is created and again
/// here, by two independent conjuncts:
///
/// - **At founding.** `founding_v5.rs:1032` is the sole creator of every LBV2
///   aggregate, and it runs `authenticate_runtime_product_basis_core_with_rent_v3`,
///   which refuses unless `product.semantic_basis_id == market.basis_id`,
///   `product.runtime.outcome_count == market.claim_count` and
///   `product.basis_width == market.claim_count`
///   (`affine_batch_v2.rs:697-706`). So the Market's representation basis IS
///   the Product's own liability basis, at equal width.
/// - **At settlement.** This route passes the Product's OWN `runtime.basis_width`
///   into the admission (`:366`), and `validate_joins` then refuses with
///   `WidthMismatch` unless `market.claim_count == admission.basis_width()`
///   (`crates/dclutch-claims-svm/src/product_basis_terminal_v3.rs:542-546`).
///   `N == K` is therefore re-forced at redemption without trusting founding.
///
/// A representation whose basis IS the Product's own liability basis, at equal
/// width, is the identity by definition; any other square matrix claims to be
/// that basis while paying a different coordinate.
///
/// Note this route does NOT itself run the three founding conjuncts: it enters
/// `signed_delta_v3` with `parent_authenticated = true`
/// (`:448`, and `signed_delta_v3.rs:401-403,412-421`), which takes the
/// digest-rejoin branch instead. An earlier draft of this comment cited that
/// check as if it
/// ran here. It does not, and the corrected chain above is what actually holds.
///
/// The canonical publisher already emits exactly this and says so: "Native
/// Claims positions already carry one coordinate per categorical Product
/// outcome. Their execution exposure is therefore the identity map `K = N`, at
/// exact denominator one"
/// (`crates/dclutch-representation-composition-v3-operator/src/native_categorical_v1.rs:1-8`,
/// built at `:523-558` with `denominator: 1` and one term
/// `{ product_coordinate: coordinate, numerator: 1 }`). Non-identity
/// compositions belong to the representation families, which redeem through
/// `rational_terminal_v3` and its descriptor pin. This function moves an
/// invariant the tree already states in prose into a place the chain enforces.
///
/// # What it does and does not buy
///
/// Two records that both pass this check compute the same translation, so the
/// redeemer's remaining freedom -- which record, of several with different
/// `graph_id` or node ids -- is economically inert. That is the sense in which
/// the payout matrix is no longer chosen by the party being paid.
///
/// It does NOT give this route an upstream anchor. Pinning an exposure digest
/// at founding, the way the descriptor pins it for Rational, would need a new
/// persisted field in the LBV2 aggregate: a wire change, and a release event.
/// Named, not attempted here.
fn require_identity_exposure_v3(
    exposure_bytes: &[u8],
    admission: RecordAdmissionV3,
) -> Result<(), ProgramError> {
    let exposure = CompositionExposureBundleV3::decode(exposure_bytes, admission)
        .map_err(|_| ClaimsSbfError::ExposureNotIdentity)?;
    let width = exposure.representation_width();
    if exposure.product_width() != width || exposure.term_count() != width {
        return Err(ClaimsSbfError::ExposureNotIdentity.into());
    }
    let mut index = 0_u32;
    while index < width {
        let row = exposure
            .row(index)
            .map_err(|_| ClaimsSbfError::ExposureNotIdentity)?;
        // `row.term_count() != 1` is IMPLIED and is kept as depth, not as a
        // live guard: the kernel's own `validate` refuses a row with zero terms
        // (`exposure.rs:556-562`), so `K` rows summing to the `term_count ==
        // width` checked above forces exactly one term each. Deleting it kills
        // no test, and that is recorded rather than hidden -- it is here so the
        // shape of the identity reads completely in one place, and so a future
        // relaxation of either neighbour cannot silently widen this.
        if row.representation_coordinate() != index
            || row.term_count() != 1
            || row.denominator() != 1
        {
            return Err(ClaimsSbfError::ExposureNotIdentity.into());
        }
        let term = exposure
            .row_term(row, 0)
            .map_err(|_| ClaimsSbfError::ExposureNotIdentity)?;
        if term.product_coordinate != index || term.numerator != 1 {
            return Err(ClaimsSbfError::ExposureNotIdentity.into());
        }
        index = index
            .checked_add(1)
            .ok_or(ClaimsSbfError::ExposureNotIdentity)?;
    }
    Ok(())
}

fn authenticate_finalized_record(
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    owner: &Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<(), ProgramError> {
    let expected_raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], owner).0;
    let expected_staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], owner).0;
    let bytes = raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if raw.key != &expected_raw
        || raw.owner != owner
        || raw.is_signer
        || raw.is_writable
        || raw.executable
        || hash(&bytes).to_bytes() != digest
        || !funded_rent_persists_v1(raw.lamports())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
        || staging.is_signer
        || staging.is_writable
        || staging.executable
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_zero_custody_accounts(
    accounts: &[AccountInfo<'_>],
    input: dclutch_claims_svm::terminal_settlement_v3::TerminalSettlementRequestInputV3,
    custody_context: [u8; 32],
) -> Result<(), ProgramError> {
    // The CLAIMS compartment of the Market's Custody namespace. The role is a
    // seed, so this address is a different account from the Trading-role replay
    // the founding realizes and the Core-role replay legacy Open creates — which
    // is what makes a payout's replay reachable at all.
    let expected_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            input.market,
            input.release_set,
            CallerRoleV1::Claims,
            custody_context,
        )
        .as_slices(),
        accounts[CUSTODY_PROGRAM].key,
    )
    .0;
    let expected_authority = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            input.market.as_slice(),
            input.release_set.as_slice(),
        ],
        accounts[CUSTODY_PROGRAM].key,
    )
    .0;
    // Both coordinates come from the persisted namespace. The replay above
    // already did; this one hardcoded the Market address, which is a coordinate
    // no founding ever used.
    let vault = CustodyVaultSeedsV1::new(
        input.market,
        input.release_set,
        custody_context,
        CompartmentV1::HoardPrincipal,
    );
    let expected_hoard =
        Pubkey::find_program_address(&vault.as_slices(), accounts[CUSTODY_PROGRAM].key).0;
    if accounts[CUSTODY_CALLER].key != accounts[16].key
        || accounts[CUSTODY_REPLAY].key != &expected_replay
        || accounts[CUSTODY_AUTHORITY].key != &expected_authority
        || accounts[HOARD].key != &expected_hoard
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    authenticate_finalized_record(
        &accounts[REALM],
        &accounts[REALM_STAGING],
        accounts[13].key,
        REALM_SCHEMA_RELEASE_ID_V1,
        input.realm,
    )?;
    let replay_bytes = accounts[CUSTODY_REPLAY]
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let replay = CustodyReplayV1::decode(&replay_bytes).map_err(|_| ClaimsSbfError::Identity)?;
    if replay.caller_role != CallerRoleV1::Claims
        || replay.release_set != input.release_set
        || replay.market != input.market
        || replay.realm != input.realm
        || replay.context != custody_context
        || replay.caller_program != input.claims_program
        || replay.next_revision != input.expected_custody_revision
        || replay.generation != input.generation
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    token_amount(
        &accounts[HOARD],
        &accounts[TOKEN_PROGRAM],
        input.collateral_mint,
        accounts[CUSTODY_AUTHORITY].key.to_bytes(),
    )?;
    token_amount(
        &accounts[RECIPIENT],
        &accounts[TOKEN_PROGRAM],
        input.collateral_mint,
        input.recipient_owner,
    )?;
    Ok(())
}

fn custody_replay_digest(
    accounts: &[AccountInfo<'_>],
    expected_revision: u64,
    payout: u64,
) -> Result<[u8; 32], ProgramError> {
    let bytes = accounts[CUSTODY_REPLAY]
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if bytes.len() != CUSTODY_REPLAY_BYTES_V1 {
        return Err(ClaimsSbfError::Identity.into());
    }
    let replay = CustodyReplayV1::decode(&bytes).map_err(|_| ClaimsSbfError::Identity)?;
    let expected = if payout == 0 {
        expected_revision
    } else {
        expected_revision
            .checked_add(1)
            .ok_or(ClaimsSbfError::Receipt)?
    };
    if replay.next_revision != expected || replay.caller_role != CallerRoleV1::Claims {
        return Err(ClaimsSbfError::Receipt.into());
    }
    Ok(hashv(&[&bytes]).to_bytes())
}

fn token_poststate_digest(accounts: &[AccountInfo<'_>]) -> Result<[u8; 32], ProgramError> {
    let hoard = accounts[HOARD]
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let recipient = accounts[RECIPIENT]
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    Ok(hashv(&[
        TERMINAL_SETTLEMENT_TOKEN_POSTSTATE_DOMAIN_V3,
        &hoard,
        &recipient,
    ])
    .to_bytes())
}

fn shared_frame<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
) -> RationalTerminalFrameV3<'accounts, 'info> {
    RationalTerminalFrameV3 {
        caller_authority: &accounts[0],
        aggregate: &accounts[1],
        linked_basis_record: &accounts[2],
        linked_basis_staging: &accounts[3],
        product_record: &accounts[4],
        product_staging: &accounts[5],
        result_domain_record: &accounts[6],
        result_domain_staging: &accounts[7],
        portfolio_record: &accounts[8],
        portfolio_staging: &accounts[9],
        graph_record: &accounts[EXPOSURE_RAW],
        rent: &accounts[10],
        core_market: &accounts[11],
        cache: &accounts[12],
        registry: &accounts[13],
        caller_program: &accounts[14],
        caller_programdata: &accounts[15],
        claims_program: &accounts[16],
        claims_programdata: &accounts[17],
        core_program: &accounts[18],
        core_programdata: &accounts[19],
        position: &accounts[20],
        custody_caller_authority: &accounts[CUSTODY_CALLER],
        custody_program: &accounts[CUSTODY_PROGRAM],
        terminal_certificate: &accounts[CERTIFICATE],
        resolution_program: &accounts[RESOLUTION_PROGRAM],
        resolution_programdata: &accounts[RESOLUTION_PROGRAMDATA],
        realm: &accounts[REALM],
        realm_staging: &accounts[REALM_STAGING],
        custody_replay: &accounts[CUSTODY_REPLAY],
        collateral_mint: &accounts[COLLATERAL_MINT],
        hoard: &accounts[HOARD],
        recipient: &accounts[RECIPIENT],
        custody_authority: &accounts[CUSTODY_AUTHORITY],
        token_program: &accounts[TOKEN_PROGRAM],
    }
}

const fn record<'accounts, 'info>(
    raw: &'accounts AccountInfo<'info>,
    staging: &'accounts AccountInfo<'info>,
) -> FinalizedRecordFrameV2<'accounts, 'info> {
    FinalizedRecordFrameV2 { raw, staging }
}

#[cfg(test)]
mod exposure_identity_tests {
    use super::*;

    use dclutch_representation_composition_v3_kernel::{
        CompositionExposureInputV3, CompositionExposureRowInputV3, CompositionExposureTermV3,
        composition_exposure_bytes_v3, encode_composition_exposure_v3_atomic,
    };

    const WIDTH: u32 = 3;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    /// Encode a real exposure record from real kernel bytes.
    ///
    /// Every hostile below is a CANONICAL record: it round-trips the kernel's
    /// own encoder and passes its own `validate`. That is the whole point --
    /// the attack was never a malformed record, it was a well-formed one
    /// stating a different recipe.
    fn encode(rows: &[CompositionExposureRowInputV3<'_>]) -> Vec<u8> {
        let terms: u32 = rows
            .iter()
            .map(|row| u32::try_from(row.terms.len()).expect("terms"))
            .sum();
        let width = composition_exposure_bytes_v3(u32::try_from(rows.len()).expect("rows"), terms)
            .expect("width");
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_composition_exposure_v3_atomic(
            CompositionExposureInputV3 {
                market: id(2),
                result_domain: id(3),
                release_set: id(4),
                product_basis: id(5),
                representation_basis: id(6),
                graph_id: id(7),
                product_width: WIDTH,
                rows,
            },
            &mut scratch,
            &mut output,
        )
        .expect("encode exposure");
        output
    }

    fn admission_for(bytes: &[u8]) -> RecordAdmissionV3 {
        let digest = hash(bytes).to_bytes();
        RecordAdmissionV3 {
            selected_id: id(7),
            finalized_id: id(7),
            recomputed_digest: digest,
            finalized_digest: digest,
            record_authenticated: true,
        }
    }

    fn check(rows: &[CompositionExposureRowInputV3<'_>]) -> Result<(), ProgramError> {
        let bytes = encode(rows);
        require_identity_exposure_v3(&bytes, admission_for(&bytes))
    }

    fn identity_terms() -> Vec<[CompositionExposureTermV3; 1]> {
        (0..WIDTH)
            .map(|coordinate| {
                [CompositionExposureTermV3 {
                    product_coordinate: coordinate,
                    numerator: 1,
                }]
            })
            .collect()
    }

    fn rows_from<'a>(
        terms: &'a [[CompositionExposureTermV3; 1]],
        denominator: u64,
    ) -> Vec<CompositionExposureRowInputV3<'a>> {
        terms
            .iter()
            .enumerate()
            .map(|(index, row)| CompositionExposureRowInputV3 {
                node_id: id(u8::try_from(index).expect("node") + 0x40),
                denominator,
                terms: row,
            })
            .collect()
    }

    #[test]
    fn the_canonical_identity_embedding_is_admitted() {
        let terms = identity_terms();
        assert_eq!(check(&rows_from(&terms, 1)), Ok(()));
    }

    /// The attack the solvency refusal cannot see.
    ///
    /// Swapping which Product coordinate each Claims row reads keeps the
    /// translated vector a permutation of the original, so its sum is still
    /// exactly `payout_scale` and `liability_before == hoard_before` holds on
    /// the nose. The holder of a LOSING coordinate redeems at full value.
    #[test]
    fn a_sum_preserving_permutation_is_refused() {
        let terms = [
            [CompositionExposureTermV3 {
                product_coordinate: 1,
                numerator: 1,
            }],
            [CompositionExposureTermV3 {
                product_coordinate: 0,
                numerator: 1,
            }],
            [CompositionExposureTermV3 {
                product_coordinate: 2,
                numerator: 1,
            }],
        ];
        assert_eq!(
            check(&rows_from(&terms, 1)),
            Err(ClaimsSbfError::ExposureNotIdentity.into())
        );
    }

    /// Every row reading one coordinate: the winner is robbed, losers paid.
    #[test]
    fn a_socializing_exposure_is_refused() {
        let terms: Vec<[CompositionExposureTermV3; 1]> = (0..WIDTH)
            .map(|_| {
                [CompositionExposureTermV3 {
                    product_coordinate: 0,
                    numerator: 1,
                }]
            })
            .collect();
        assert_eq!(
            check(&rows_from(&terms, 1)),
            Err(ClaimsSbfError::ExposureNotIdentity.into())
        );
    }

    /// Under-payment, which solvency admits because it only bounds from above.
    #[test]
    fn a_scaled_denominator_that_underpays_is_refused() {
        let terms = identity_terms();
        assert_eq!(
            check(&rows_from(&terms, 2)),
            Err(ClaimsSbfError::ExposureNotIdentity.into())
        );
    }

    #[test]
    fn an_inflated_numerator_is_refused() {
        let terms: Vec<[CompositionExposureTermV3; 1]> = (0..WIDTH)
            .map(|coordinate| {
                [CompositionExposureTermV3 {
                    product_coordinate: coordinate,
                    numerator: 2,
                }]
            })
            .collect();
        assert_eq!(
            check(&rows_from(&terms, 1)),
            Err(ClaimsSbfError::ExposureNotIdentity.into())
        );
    }

    /// A canonical row that reads two Product coordinates at once.
    ///
    /// The conjunct that OWNS this is the aggregate `term_count != width`, not
    /// the per-row `term_count() != 1`: with every row required to carry at
    /// least one term, a blend anywhere pushes the total above `K`. Named that
    /// way because a mutation deleting the per-row check leaves this test green,
    /// and a test whose stated owner cannot fail it is a test that teaches the
    /// next reader something false.
    #[test]
    fn a_row_that_blends_two_coordinates_is_refused() {
        let blended = [
            CompositionExposureTermV3 {
                product_coordinate: 0,
                numerator: 1,
            },
            CompositionExposureTermV3 {
                product_coordinate: 1,
                numerator: 1,
            },
        ];
        let single_one = [CompositionExposureTermV3 {
            product_coordinate: 1,
            numerator: 1,
        }];
        let single_two = [CompositionExposureTermV3 {
            product_coordinate: 2,
            numerator: 1,
        }];
        let rows = [
            CompositionExposureRowInputV3 {
                node_id: id(0x40),
                denominator: 1,
                terms: &blended,
            },
            CompositionExposureRowInputV3 {
                node_id: id(0x41),
                denominator: 1,
                terms: &single_one,
            },
            CompositionExposureRowInputV3 {
                node_id: id(0x42),
                denominator: 1,
                terms: &single_two,
            },
        ];
        assert_eq!(
            check(&rows),
            Err(ClaimsSbfError::ExposureNotIdentity.into())
        );
    }

    /// `K != N` never reaches the payout either.
    #[test]
    fn a_narrower_representation_than_the_product_is_refused() {
        let terms = [
            [CompositionExposureTermV3 {
                product_coordinate: 0,
                numerator: 1,
            }],
            [CompositionExposureTermV3 {
                product_coordinate: 1,
                numerator: 1,
            }],
        ];
        assert_eq!(
            check(&rows_from(&terms, 1)),
            Err(ClaimsSbfError::ExposureNotIdentity.into())
        );
    }

    /// The refusal is the one the registry allocated, not a neighbour.
    #[test]
    fn the_refusal_is_the_registered_claims_band_code() {
        assert_eq!(
            ClaimsSbfError::ExposureNotIdentity as u32,
            dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x00E
        );
        assert_ne!(
            ClaimsSbfError::ExposureNotIdentity as u32,
            ClaimsSbfError::Economic as u32
        );
    }
}
