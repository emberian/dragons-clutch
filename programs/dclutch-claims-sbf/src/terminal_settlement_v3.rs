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
    COMPOSITION_EXPOSURE_SCHEMA_ID_V3, RecordAdmissionV3,
};
use dclutch_token_svm::TokenAccount;
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    program::set_return_data,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::system_program;

use super::{
    ClaimsSbfError,
    affine_batch_v2::CorePhaseGateV3,
    liability_basis_v2::LIABILITY_BASIS_MARKET_SEED_V2,
    rational_terminal_v3::{
        RationalTerminalFrameV3, TerminalCustodyInputV3, execute_terminal_custody_v3,
    },
    signed_delta_v3::{
        AuthenticatedSignedDeltaParentV3, ParentAuthorityV3, SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
        authenticate_parent_releases, execute_parent_authenticated,
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
    let prepared = authenticate_and_prepare(program_id, accounts, &request, request_digest)?;
    let receipt = execute(
        program_id,
        accounts,
        &request,
        request_digest,
        prepared,
        parent_authority(request.input()),
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
    let prepared = authenticate_and_prepare(program_id, accounts, &request, request_digest)?;
    execute(
        program_id,
        accounts,
        &request,
        request_digest,
        prepared,
        ParentAuthorityV3::EnclosingClaimsRoute,
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
) -> Result<Box<PreparedTerminalSettlementV3>, ProgramError> {
    let input = (*request).input();
    authenticate_extra_privileges(program_id, accounts, input)?;
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
    let rent = Rent::from_account_info(&accounts[10]).map_err(|_| ClaimsSbfError::Accounts)?;
    let runtime = authenticate_product_runtime_v3(
        accounts[13].key,
        &rent,
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
        &rent,
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
            rent: &accounts[10],
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
    let payout = encode_product_claims_terminal_signed_delta_v3(
        ProductClaimsTerminalInputV3 {
            product_basis_bytes: &basis_bytes,
            admission,
            composition_exposure_bytes: &exposure_bytes,
            composition_exposure_admission: RecordAdmissionV3 {
                selected_id: input.exposure_id,
                finalized_id: input.exposure_id,
                recomputed_digest: hash(&exposure_bytes).to_bytes(),
                finalized_digest: input.exposure_digest,
                record_authenticated: true,
            },
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
    authenticate_parent_releases(program_id, &signed_accounts, &prepared.packet)?;
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
        CorePhaseGateV3::TerminalOrRetiring,
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
    if input.caller_role == CallerRole::Claims && accounts[0].key.to_bytes() != input.owner {
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
    if !CorePhaseGateV3::TerminalOrRetiring.admits(core.phase)
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

fn authenticate_finalized_record(
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    owner: &Pubkey,
    rent: &Rent,
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
        || !rent.is_exempt(raw.lamports(), bytes.len())
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
    let rent = Rent::from_account_info(&accounts[10]).map_err(|_| ClaimsSbfError::Accounts)?;
    authenticate_finalized_record(
        &accounts[REALM],
        &accounts[REALM_STAGING],
        accounts[13].key,
        &rent,
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

fn token_amount(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    mint: [u8; 32],
    owner: [u8; 32],
) -> Result<u64, ProgramError> {
    if account.owner != token_program.key {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let bytes = account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let token = TokenAccount::parse(&bytes).map_err(|_| ClaimsSbfError::Accounts)?;
    if token.mint != mint || token.owner != owner {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(token.amount)
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
