//! Family-neutral Product terminal settlement through SignedDelta and Custody.

// Every private phase is reachable only after `process` proves the one exact
// 35-account frame. Fixed indexing below therefore cannot observe a short
// untrusted slice and keeps the SBF frame materially smaller than a duplicated
// 35-reference view.
#![allow(clippy::indexing_slicing)]

extern crate alloc;

use alloc::{boxed::Box, vec, vec::Vec};

use dclutch_claims_svm::{
    liability_basis_state_v2::LiabilityBasisMarketViewV2,
    product_basis_terminal_v3::{
        ProductClaimsTerminalAdmissionV3, ProductClaimsTerminalInputV3,
        TERMINAL_COORDINATE_BYTES_V2, TERMINAL_COORDINATE_MAGIC_V2,
        TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2, encode_product_claims_terminal_signed_delta_v3,
    },
    signed_delta_v3::{SignedDeltaV3, plan_bytes},
    terminal_settlement_v3::{
        TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 as ACCOUNT_COUNT,
        TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3,
        TERMINAL_SETTLEMENT_COLLATERAL_MINT_ACCOUNT_V3 as COLLATERAL_MINT,
        TERMINAL_SETTLEMENT_COORDINATE_ACCOUNT_V3 as COORDINATE,
        TERMINAL_SETTLEMENT_COORDINATE_STAGING_ACCOUNT_V3 as COORDINATE_STAGING,
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
        TERMINAL_SETTLEMENT_TOKEN_POSTSTATE_DOMAIN_V3,
        TERMINAL_SETTLEMENT_TOKEN_PROGRAM_ACCOUNT_V3 as TOKEN_PROGRAM,
        TerminalSettlementReceiptInputV3, TerminalSettlementReceiptV3, TerminalSettlementRequestV3,
    },
};
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CUSTODY_REPLAY_PDA_DOMAIN_V1,
    CallerRoleV1, CompartmentV1, CustodyReplayV1, CustodyVaultSeedsV1,
};
use dclutch_market_core_codec::{
    CoreState, MarketCoreStateSeedsV2, Phase as CorePhase, STATE_BYTES,
};
use dclutch_product_payoff_v2_codec::runtime_v3::BasisKindV3;
use dclutch_product_runtime_v2::ContentId as ProductContentId;
use dclutch_product_runtime_v2_svm_reader::{
    FinalizedRecordFrameV2, ProductRuntimeFrameV3, authenticate_product_runtime_v3,
};
use dclutch_realm_contract::REALM_SCHEMA_RELEASE_ID_V1;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
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
    liability_basis_v2::LIABILITY_BASIS_MARKET_SEED_V2,
    rational_terminal_v3::{
        RationalTerminalFrameV3, TerminalCustodyInputV3, execute_terminal_custody_v3,
    },
    signed_delta_v3::{
        AuthenticatedSignedDeltaParentV3, SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
        authenticate_parent_releases, execute_parent_authenticated,
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
    execute(program_id, accounts, &request, request_digest, prepared)
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
    let scenario = terminal_scenario(
        runtime.basis_kind,
        runtime.runtime.outcome_count,
        core,
        &accounts[18],
        &accounts[10],
        &accounts[COORDINATE],
        &accounts[COORDINATE_STAGING],
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
) -> Result<(), ProgramError> {
    let input = (*request).input();
    let signed_accounts = Vec::from(&accounts[..SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 + 1]);
    authenticate_parent_releases(program_id, &signed_accounts, &prepared.packet)?;
    let signed_receipt = execute_parent_authenticated(
        program_id,
        &signed_accounts,
        &prepared.packet,
        AuthenticatedSignedDeltaParentV3 {
            caller_role: input.caller_role,
            release_set: input.release_set,
            market: input.market,
            parent_context: input.parent_context,
            parent_request_digest: request_digest,
        },
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
) -> Result<(), ProgramError> {
    let receipt = TerminalSettlementReceiptV3::new(request, *evidence)
        .map_err(|_| ClaimsSbfError::Receipt)?;
    set_return_data(&receipt.to_bytes());
    Ok(())
}

fn authenticate_extra_privileges(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    input: dclutch_claims_svm::terminal_settlement_v3::TerminalSettlementRequestInputV3,
) -> Result<(), ProgramError> {
    for index in [
        EXPOSURE_RAW,
        EXPOSURE_STAGING,
        COORDINATE,
        COORDINATE_STAGING,
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
    Ok(())
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
    if core.phase != CorePhase::Terminal
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

fn terminal_scenario(
    kind: BasisKindV3,
    outcome_count: u32,
    core: CoreState,
    core_program: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
    coordinate: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
) -> Result<dclutch_rational_representation_v2_kernel::product_v3::TerminalScenarioV3, ProgramError>
{
    use dclutch_rational_representation_v2_kernel::product_v3::TerminalScenarioV3;
    match kind {
        BasisKindV3::CategoricalQ1 => {
            placeholders(rent, coordinate, staging)?;
            Ok(TerminalScenarioV3::Categorical(core.terminal_winner))
        }
        BasisKindV3::GradedExactComplement => {
            let failure = outcome_count
                .checked_sub(1)
                .ok_or(ClaimsSbfError::Identity)?;
            if core.terminal_winner == failure {
                placeholders(rent, coordinate, staging)?;
                return Ok(TerminalScenarioV3::Failure);
            }
            let digest = core
                .terminal_receipt
                .ok_or(ClaimsSbfError::Identity)?
                .to_bytes();
            let rent_value = Rent::from_account_info(rent).map_err(|_| ClaimsSbfError::Accounts)?;
            authenticate_finalized_record(
                coordinate,
                staging,
                core_program.key,
                &rent_value,
                TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2,
                digest,
            )?;
            let bytes = coordinate
                .try_borrow_data()
                .map_err(|_| ClaimsSbfError::Accounts)?;
            if bytes.len() != TERMINAL_COORDINATE_BYTES_V2
                || array::<8>(&bytes, 0)? != TERMINAL_COORDINATE_MAGIC_V2
                || u16::from_le_bytes(array(&bytes, 8)?) != 2
                || bytes[10..16].iter().any(|byte| *byte != 0)
                || bytes[28..32].iter().any(|byte| *byte != 0)
            {
                return Err(ClaimsSbfError::Identity.into());
            }
            let numerator = i64::from_le_bytes(array(&bytes, 16)?);
            let denominator = u32::from_le_bytes(array(&bytes, 24)?);
            if denominator == 0 {
                return Err(ClaimsSbfError::Identity.into());
            }
            Ok(TerminalScenarioV3::Rational {
                numerator: i128::from(numerator),
                denominator: u64::from(denominator),
            })
        }
    }
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
    let expected_replay = Pubkey::find_program_address(
        &[
            CUSTODY_REPLAY_PDA_DOMAIN_V1,
            input.market.as_slice(),
            input.release_set.as_slice(),
            custody_context.as_slice(),
        ],
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
        coordinate: &accounts[COORDINATE],
        coordinate_staging: &accounts[COORDINATE_STAGING],
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
fn placeholders(
    rent: &AccountInfo<'_>,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    if raw.key != rent.key || staging.key != rent.key {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(())
}
fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], ProgramError> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(ClaimsSbfError::Identity)?)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| ClaimsSbfError::Identity.into())
}
