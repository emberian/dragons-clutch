//! ProductBasisV3 terminal Claims and Custody execution.
//!
//! Product semantics are evaluated by the no-allocation Claims kernel into one
//! family-neutral SignedDeltaV3 packet. Claims executes that packet internally;
//! positive exact collateral payout then crosses the typed Custody boundary.
//! A later failure rolls back both effects at the SVM instruction boundary.

extern crate alloc;

use alloc::{boxed::Box, vec, vec::Vec};

use dclutch_claims_svm::{
    CallerRole,
    product_basis_terminal_v3::{
        ProductBasisTerminalInputV3, TERMINAL_CANDIDATE_DOMAIN_V3,
        encode_product_basis_terminal_signed_delta_v3,
    },
    protocol_position_v2::ProtocolPositionClaimsCapabilitySeedsV2,
    signed_delta_v3::{SignedDeltaReceiptV3, SignedDeltaV3, plan_bytes},
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_RECEIPT_BYTES_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1,
    CustodyAuthoritySeedsV1, CustodyReceiptV1, CustodyReplaySeedsV1, CustodyReplayV1,
    CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_rational_representation_v2_contract::{
    ABSENT_REVISION, CallerRoleV2, RepresentationRequestV2,
};
use dclutch_realm_contract::REALM_SCHEMA_RELEASE_ID_V1;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_representation_composition_v3_kernel::RecordAdmissionV3;
use dclutch_token_svm::TokenAccount;
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use super::{
    ClaimsSbfError,
    rational_product_v3::{AuthenticatedRationalProductV3, authenticate_terminal_scenario_v3},
    signed_delta_v3::{
        AuthenticatedSignedDeltaParentV3, SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
        execute_parent_authenticated as execute_parent_signed_delta_v3,
    },
};

#[derive(Clone, Copy)]
pub(crate) struct RationalTerminalFrameV3<'accounts, 'info> {
    pub(crate) caller_authority: &'accounts AccountInfo<'info>,
    pub(crate) aggregate: &'accounts AccountInfo<'info>,
    pub(crate) linked_basis_record: &'accounts AccountInfo<'info>,
    pub(crate) linked_basis_staging: &'accounts AccountInfo<'info>,
    pub(crate) product_record: &'accounts AccountInfo<'info>,
    pub(crate) product_staging: &'accounts AccountInfo<'info>,
    pub(crate) result_domain_record: &'accounts AccountInfo<'info>,
    pub(crate) result_domain_staging: &'accounts AccountInfo<'info>,
    pub(crate) portfolio_record: &'accounts AccountInfo<'info>,
    pub(crate) portfolio_staging: &'accounts AccountInfo<'info>,
    pub(crate) graph_record: &'accounts AccountInfo<'info>,
    pub(crate) rent: &'accounts AccountInfo<'info>,
    pub(crate) core_market: &'accounts AccountInfo<'info>,
    pub(crate) cache: &'accounts AccountInfo<'info>,
    pub(crate) registry: &'accounts AccountInfo<'info>,
    pub(crate) caller_program: &'accounts AccountInfo<'info>,
    pub(crate) caller_programdata: &'accounts AccountInfo<'info>,
    pub(crate) claims_program: &'accounts AccountInfo<'info>,
    pub(crate) claims_programdata: &'accounts AccountInfo<'info>,
    pub(crate) core_program: &'accounts AccountInfo<'info>,
    pub(crate) core_programdata: &'accounts AccountInfo<'info>,
    pub(crate) position: &'accounts AccountInfo<'info>,
    pub(crate) custody_caller_authority: &'accounts AccountInfo<'info>,
    pub(crate) custody_program: &'accounts AccountInfo<'info>,
    pub(crate) coordinate: &'accounts AccountInfo<'info>,
    pub(crate) coordinate_staging: &'accounts AccountInfo<'info>,
    pub(crate) realm: &'accounts AccountInfo<'info>,
    pub(crate) realm_staging: &'accounts AccountInfo<'info>,
    pub(crate) custody_replay: &'accounts AccountInfo<'info>,
    pub(crate) collateral_mint: &'accounts AccountInfo<'info>,
    pub(crate) hoard: &'accounts AccountInfo<'info>,
    pub(crate) recipient: &'accounts AccountInfo<'info>,
    pub(crate) custody_authority: &'accounts AccountInfo<'info>,
    pub(crate) token_program: &'accounts AccountInfo<'info>,
}

pub(crate) struct RationalTerminalCustodyEvidenceV3 {
    pub(crate) request: Box<CustodyRequestV1>,
    pub(crate) request_digest: [u8; 32],
    pub(crate) receipt: Box<CustodyReceiptV1>,
    pub(crate) receipt_digest: [u8; 32],
    pub(crate) replay_digest: [u8; 32],
}

#[derive(Clone, Copy)]
pub(crate) struct TerminalCustodyInputV3 {
    pub(crate) release_set: [u8; 32],
    pub(crate) market: [u8; 32],
    pub(crate) realm: [u8; 32],
    pub(crate) parent_request_digest: [u8; 32],
    pub(crate) recipient_owner: [u8; 32],
    pub(crate) generation: u64,
    pub(crate) order_nonce: u64,
    pub(crate) transfer_index: u16,
    pub(crate) expected_custody_revision: u64,
    pub(crate) payout: u64,
    pub(crate) candidate_digest: [u8; 32],
    pub(crate) custody_context: [u8; 32],
}

pub(crate) struct RationalTerminalExecutionV3 {
    pub(crate) packet: Vec<u8>,
    pub(crate) packet_digest: [u8; 32],
    pub(crate) receipt: SignedDeltaReceiptV3,
    pub(crate) custody: Option<Box<RationalTerminalCustodyEvidenceV3>>,
}

#[inline(never)]
pub(crate) fn execute_rational_terminal_v3<'accounts, 'info>(
    program_id: &Pubkey,
    frame: RationalTerminalFrameV3<'accounts, 'info>,
    request: RepresentationRequestV2<'_>,
    request_digest: [u8; 32],
    authenticated: Box<AuthenticatedRationalProductV3>,
) -> Result<Box<RationalTerminalExecutionV3>, ProgramError> {
    let header = request.header();
    let asset = request.asset(0).map_err(|_| ClaimsSbfError::Instruction)?;
    let owner_seeds =
        ProtocolPositionClaimsCapabilitySeedsV2::new(header.descriptor_id, header.selected_outcome)
            .map_err(|_| ClaimsSbfError::Identity)?;
    let owner = Pubkey::find_program_address(&owner_seeds.as_slices(), program_id)
        .0
        .to_bytes();
    if owner != asset.claims_custody_owner {
        return Err(ClaimsSbfError::Identity.into());
    }
    let scenario = authenticate_terminal_scenario_v3(
        authenticated.as_ref(),
        frame.core_program,
        frame.rent,
        frame.coordinate,
        frame.coordinate_staging,
    )?;
    let hoard_before = token_amount(
        frame.hoard,
        frame.token_program,
        frame.collateral_mint.key.to_bytes(),
        frame.custody_authority.key.to_bytes(),
    )?;
    let claims_width = usize::try_from(authenticated.admission.basis_width())
        .map_err(|_| ClaimsSbfError::Economic)?;
    let product_width = usize::try_from(authenticated.result_outcome_count)
        .map_err(|_| ClaimsSbfError::Economic)?;
    let neutral = SignedDeltaV3::new(
        dclutch_claims_svm::signed_delta_v3::DeltaDirectionV3::Neutral,
        0,
    )
    .map_err(|_| ClaimsSbfError::Economic)?;
    let mut product_payout_scratch = vec![0_u64; product_width];
    let mut translation_scratch = vec![0_u64; claims_width];
    let mut claims_payout_scratch = vec![0_u64; claims_width];
    let mut aggregate_scratch = vec![neutral; claims_width];
    let mut packet = vec![
        0_u8;
        plan_bytes(authenticated.admission.basis_width(), 1, 1)
            .map_err(|_| ClaimsSbfError::Economic)?
    ];
    let market = frame
        .aggregate
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let position = frame
        .position
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let basis = frame
        .linked_basis_record
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let exposure = frame
        .graph_record
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let payout = encode_product_basis_terminal_signed_delta_v3(
        ProductBasisTerminalInputV3 {
            product_basis_bytes: &basis,
            representation: authenticated.admission,
            composition_exposure_bytes: &exposure,
            composition_exposure_admission: RecordAdmissionV3 {
                selected_id: authenticated.admission.graph_id(),
                finalized_id: authenticated.admission.graph_id(),
                recomputed_digest: hash(&exposure).to_bytes(),
                finalized_digest: authenticated.admission.graph_digest(),
                record_authenticated: true,
            },
            product_record_digest: authenticated.product_record_digest,
            market_account: frame.aggregate.key.to_bytes(),
            market_bytes: &market,
            position_bytes: &position,
            owner,
            request_id: request_digest,
            caller_role: caller_role(header.caller_role),
            terminal: scenario,
            claim_index: header.selected_outcome,
            quantity: header.quantity,
            expected_generation: header.generation,
            expected_market_revision: header.expected_claims_market_revision,
            expected_position_revision: header.expected_custody_position_revision,
            hoard_before,
        },
        &mut product_payout_scratch,
        &mut translation_scratch,
        &mut claims_payout_scratch,
        &mut aggregate_scratch,
        &mut packet,
    )
    .map_err(|_| ClaimsSbfError::Economic)?;
    drop(exposure);
    drop(basis);
    drop(position);
    drop(market);
    let packet_digest = hash(&packet).to_bytes();
    let signed_accounts = signed_delta_accounts(&frame);
    if signed_accounts.len() != SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 + 1 {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let receipt = execute_parent_signed_delta_v3(
        program_id,
        &signed_accounts,
        &packet,
        AuthenticatedSignedDeltaParentV3 {
            caller_role: caller_role(header.caller_role),
            release_set: header.release_set,
            market: header.market,
            parent_context: header.parent_context,
            parent_request_digest: request_digest,
        },
    )?;
    let candidate_digest = hashv(&[
        TERMINAL_CANDIDATE_DOMAIN_V3,
        &packet_digest,
        &payout.to_le_bytes(),
        &authenticated.admission.to_bytes(),
        &authenticated.core.terminal_winner.to_le_bytes(),
    ])
    .to_bytes();
    let custody = execute_terminal_custody_v3(
        program_id,
        &frame,
        TerminalCustodyInputV3 {
            release_set: header.release_set,
            market: header.market,
            realm: header.realm,
            parent_request_digest: request_digest,
            recipient_owner: header.actor,
            generation: header.generation,
            order_nonce: header.expected_representation_revision,
            transfer_index: 0,
            expected_custody_revision: header.expected_custody_replay_revision,
            payout,
            candidate_digest,
            custody_context: authenticated.market.custody_context,
        },
    )?;
    Ok(Box::new(RationalTerminalExecutionV3 {
        packet,
        packet_digest,
        receipt,
        custody,
    }))
}

fn signed_delta_accounts<'accounts, 'info>(
    frame: &RationalTerminalFrameV3<'accounts, 'info>,
) -> Vec<AccountInfo<'info>> {
    Vec::from([
        frame.caller_authority.clone(),
        frame.aggregate.clone(),
        frame.linked_basis_record.clone(),
        frame.linked_basis_staging.clone(),
        frame.product_record.clone(),
        frame.product_staging.clone(),
        frame.result_domain_record.clone(),
        frame.result_domain_staging.clone(),
        frame.portfolio_record.clone(),
        frame.portfolio_staging.clone(),
        frame.rent.clone(),
        frame.core_market.clone(),
        frame.cache.clone(),
        frame.registry.clone(),
        frame.caller_program.clone(),
        frame.caller_programdata.clone(),
        frame.claims_program.clone(),
        frame.claims_programdata.clone(),
        frame.core_program.clone(),
        frame.core_programdata.clone(),
        frame.position.clone(),
    ])
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn execute_terminal_custody_v3(
    program_id: &Pubkey,
    frame: &RationalTerminalFrameV3<'_, '_>,
    input: TerminalCustodyInputV3,
) -> Result<Option<Box<RationalTerminalCustodyEvidenceV3>>, ProgramError> {
    if input.payout == 0 {
        return Ok(None);
    }
    if input.expected_custody_revision == ABSENT_REVISION {
        return Err(ClaimsSbfError::Economic.into());
    }
    let custody_request = Box::new(CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Claims,
        source_compartment: CompartmentV1::HoardPrincipal,
        destination_compartment: CompartmentV1::External,
        release_set: input.release_set,
        market: input.market,
        realm: input.realm,
        context: input.custody_context,
        caller_program: program_id.to_bytes(),
        semantic: ContextV1 {
            candidate: input.candidate_digest,
            source_owner: [0; 32],
            destination_owner: input.recipient_owner,
            order: [0; 32],
            parent_request_digest: input.parent_request_digest,
            order_nonce: input.order_nonce,
            generation: input.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: input.transfer_index,
        },
        source: frame.hoard.key.to_bytes(),
        destination: frame.recipient.key.to_bytes(),
        // The Hoard Vault lives in the Market's Custody namespace, which the
        // Claims aggregate persists and this route must not re-guess. The
        // replay coordinate above already reads the field; naming the Market
        // here was the half that did not, and it is what put the founded
        // Market's principal out of reach of every payout route.
        source_vault_context: input.custody_context,
        destination_vault_context: [0; 32],
        mint: frame.collateral_mint.key.to_bytes(),
        token_program: frame.token_program.key.to_bytes(),
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: input.expected_custody_revision,
        resulting_revision: input
            .expected_custody_revision
            .checked_add(1)
            .ok_or(ClaimsSbfError::Economic)?,
        amount: input.payout,
        rent_lamports: 0,
    });
    let request_bytes = Box::new(
        custody_request
            .to_bytes()
            .map_err(|_| ClaimsSbfError::Economic)?,
    );
    authenticate_custody_accounts(
        program_id,
        frame,
        custody_request.as_ref(),
        request_bytes.as_ref(),
    )?;
    let source_before = token_amount(
        frame.hoard,
        frame.token_program,
        custody_request.mint,
        frame.custody_authority.key.to_bytes(),
    )?;
    let recipient_before = token_amount(
        frame.recipient,
        frame.token_program,
        custody_request.mint,
        input.recipient_owner,
    )?;
    let invoked = invoke_custody(
        program_id,
        frame,
        custody_request.as_ref(),
        request_bytes.as_ref(),
    )?;
    let source_after = token_amount(
        frame.hoard,
        frame.token_program,
        custody_request.mint,
        frame.custody_authority.key.to_bytes(),
    )?;
    let recipient_after = token_amount(
        frame.recipient,
        frame.token_program,
        custody_request.mint,
        input.recipient_owner,
    )?;
    if source_before.checked_sub(input.payout) != Some(source_after)
        || recipient_before.checked_add(input.payout) != Some(recipient_after)
    {
        return Err(ClaimsSbfError::Receipt.into());
    }
    Ok(Some(Box::new(RationalTerminalCustodyEvidenceV3 {
        request: custody_request,
        request_digest: invoked.request_digest,
        receipt: invoked.receipt,
        receipt_digest: invoked.receipt_digest,
        replay_digest: invoked.replay_digest,
    })))
}

struct InvokedCustodyV3 {
    request_digest: [u8; 32],
    receipt: Box<CustodyReceiptV1>,
    receipt_digest: [u8; 32],
    replay_digest: [u8; 32],
}

#[inline(never)]
fn invoke_custody(
    program_id: &Pubkey,
    frame: &RationalTerminalFrameV3<'_, '_>,
    custody_request: &CustodyRequestV1,
    request_bytes: &[u8],
) -> Result<Box<InvokedCustodyV3>, ProgramError> {
    let instruction = Instruction {
        program_id: *frame.custody_program.key,
        accounts: Vec::from([
            AccountMeta::new_readonly(*frame.custody_caller_authority.key, true),
            AccountMeta::new_readonly(*frame.core_market.key, false),
            AccountMeta::new_readonly(*frame.cache.key, false),
            AccountMeta::new_readonly(*frame.registry.key, false),
            AccountMeta::new_readonly(*frame.claims_program.key, false),
            AccountMeta::new_readonly(*frame.claims_programdata.key, false),
            AccountMeta::new_readonly(*frame.realm.key, false),
            AccountMeta::new_readonly(*frame.realm_staging.key, false),
            AccountMeta::new(*frame.custody_replay.key, false),
            AccountMeta::new_readonly(*frame.collateral_mint.key, false),
            AccountMeta::new(*frame.hoard.key, false),
            AccountMeta::new(*frame.recipient.key, false),
            AccountMeta::new_readonly(*frame.custody_authority.key, false),
            AccountMeta::new_readonly(*frame.token_program.key, false),
        ]),
        data: request_bytes.to_vec(),
    };
    let request_digest = hash(request_bytes).to_bytes();
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(custody_request.release_set).map_err(|_| ClaimsSbfError::Economic)?,
        custody_request.market,
        ExecutionRoleV1::Claims,
        custody_request.context,
        request_digest,
    )
    .map_err(|_| ClaimsSbfError::Economic)?;
    let bump = Pubkey::find_program_address(&caller_seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = caller_seeds.as_slices();
    invoke_signed(
        &instruction,
        &[
            frame.custody_caller_authority.clone(),
            frame.core_market.clone(),
            frame.cache.clone(),
            frame.registry.clone(),
            frame.claims_program.clone(),
            frame.claims_programdata.clone(),
            frame.realm.clone(),
            frame.realm_staging.clone(),
            frame.custody_replay.clone(),
            frame.collateral_mint.clone(),
            frame.hoard.clone(),
            frame.recipient.clone(),
            frame.custody_authority.clone(),
            frame.token_program.clone(),
            frame.custody_program.clone(),
        ],
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(|_| ClaimsSbfError::Economic)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(ClaimsSbfError::Receipt)?;
    if producer != *frame.custody_program.key || receipt_bytes.len() != CUSTODY_RECEIPT_BYTES_V1 {
        return Err(ClaimsSbfError::Receipt.into());
    }
    let receipt =
        Box::new(CustodyReceiptV1::decode(&receipt_bytes).map_err(|_| ClaimsSbfError::Receipt)?);
    let replay = frame
        .custody_replay
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let replay_digest = hashv(&[&replay]).to_bytes();
    drop(replay);
    receipt
        .verify_for(*custody_request, request_digest, replay_digest)
        .map_err(|_| ClaimsSbfError::Receipt)?;
    Ok(Box::new(InvokedCustodyV3 {
        request_digest,
        receipt,
        receipt_digest: hash(&receipt_bytes).to_bytes(),
        replay_digest,
    }))
}

fn authenticate_custody_accounts(
    program_id: &Pubkey,
    frame: &RationalTerminalFrameV3<'_, '_>,
    request: &CustodyRequestV1,
    request_bytes: &[u8],
) -> Result<(), ProgramError> {
    let request_digest = hash(request_bytes).to_bytes();
    let caller = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).map_err(|_| ClaimsSbfError::Economic)?,
        request.market,
        ExecutionRoleV1::Claims,
        request.context,
        request_digest,
    )
    .map_err(|_| ClaimsSbfError::Economic)?;
    let replay = CustodyReplaySeedsV1::from_request(*request);
    let authority = CustodyAuthoritySeedsV1::from_request(*request);
    let vault = CustodyVaultSeedsV1::from_request(*request, true);
    let expected_realm = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            request.realm.as_slice(),
        ],
        frame.registry.key,
    )
    .0;
    let expected_realm_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            request.realm.as_slice(),
        ],
        frame.registry.key,
    )
    .0;
    if frame.custody_caller_authority.key
        != &Pubkey::find_program_address(&caller.as_slices(), program_id).0
        || frame.custody_replay.key
            != &Pubkey::find_program_address(&replay.as_slices(), frame.custody_program.key).0
        || frame.custody_authority.key
            != &Pubkey::find_program_address(&authority.as_slices(), frame.custody_program.key).0
        || frame.hoard.key
            != &Pubkey::find_program_address(&vault.as_slices(), frame.custody_program.key).0
        || frame.recipient.key.to_bytes() != request.destination
        || frame.realm.key != &expected_realm
        || frame.realm_staging.key != &expected_realm_staging
        || frame.token_program.key.to_bytes() != request.token_program
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let replay_bytes = frame
        .custody_replay
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if replay_bytes.len() != CUSTODY_REPLAY_BYTES_V1 {
        return Err(ClaimsSbfError::Identity.into());
    }
    let replay_state =
        CustodyReplayV1::decode(&replay_bytes).map_err(|_| ClaimsSbfError::Identity)?;
    if replay_state.caller_role != CallerRoleV1::Claims
        || replay_state.release_set != request.release_set
        || replay_state.market != request.market
        || replay_state.realm != request.realm
        || replay_state.context != request.context
        || replay_state.caller_program != request.caller_program
        || replay_state.next_revision != request.expected_revision
        || replay_state.generation != request.semantic.generation
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(())
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

const fn caller_role(role: CallerRoleV2) -> CallerRole {
    match role {
        CallerRoleV2::Core => CallerRole::Core,
        CallerRoleV2::Trading => CallerRole::Trading,
    }
}
