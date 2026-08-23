//! Non-production immutable Dealer-policy catalog transport.
//!
//! This is deliberately not a funded Dealer facility. It admits no market,
//! Position, Replay, fee budget, liveness budget, Lease, Pot, trade, or token
//! transfer. It only transports one exact
//! [`DealerPolicyV1`](clutch_dealer_runtime_contract::DealerPolicyV1) body through a
//! strict replay-cursor stage and materializes its content-addressed catalog
//! account. Every economic Dealer action remains disabled.
//!
//! Unlike the legacy artifact plane, this successor owns fresh account tags,
//! PDA domains, rent attribution, and capability membership. A hostile stage
//! prefund never discounts the uploader's refundable principal: the uploader
//! supplies the full rent minimum, receives exactly that principal on close,
//! and every prefund or later surplus goes to the policy's neutral sink. A
//! hostile final-PDA prefund is routed to that same sink before the uploader
//! supplies the final account's full permanently locked rent principal.

use crate::accounts::{expect_pda, require, require_count, require_signer, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_dealer_runtime_contract::{DealerPolicyV1, FixedCodec, DEALER_POLICY_BYTES_V1};
use clutch_solana_layout::registry::{
    DealerPolicyAction, DEALER_BEGIN_POLICY_PAYLOAD_BYTES, DEALER_POLICY_ACCOUNT_BYTES,
    DEALER_POLICY_ACCOUNT_HEADER_BYTES, DEALER_POLICY_ACCOUNT_TAG, DEALER_POLICY_ACCOUNT_VERSION,
    DEALER_POLICY_BODY_BYTES, DEALER_POLICY_CHUNK_BYTES, DEALER_POLICY_ID_PAYLOAD_BYTES,
    DEALER_POLICY_STAGE_ACCOUNT_BYTES, DEALER_POLICY_STAGE_ACCOUNT_TAG,
    DEALER_POLICY_STAGE_ACCOUNT_VERSION, DEALER_POLICY_STAGE_HEADER_BYTES,
    DEALER_WRITE_POLICY_PAYLOAD_BYTES,
};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::artifact::{read_clock_slot, MAX_UPLOAD_LIFETIME_SLOTS, MIN_UPLOAD_LIFETIME_SLOTS};
use super::genesis::{
    read_rent, require_creatable, require_system_program, RentParameters,
    MAX_PERMITTED_DATA_INCREASE, SYSTEM_PROGRAM_ID,
};

const SYSTEM_IX_ASSIGN: u32 = 1;
const SYSTEM_IX_TRANSFER: u32 = 2;
const SYSTEM_IX_ALLOCATE: u32 = 8;
const SYSTEM_TRANSFER_DATA_LEN: usize = 12;
const SYSTEM_ALLOCATE_DATA_LEN: usize = 12;
const SYSTEM_ASSIGN_DATA_LEN: usize = 36;

const BEGIN_ACCOUNT_COUNT: usize = 5;
const WRITE_ACCOUNT_COUNT: usize = 3;
const SEAL_ACCOUNT_COUNT: usize = 7;
const ABORT_ACCOUNT_COUNT: usize = 5;

const _: () = {
    assert!(DEALER_POLICY_BODY_BYTES == DEALER_POLICY_BYTES_V1);
    assert!(DEALER_POLICY_STAGE_HEADER_BYTES == 140);
    assert!(DEALER_POLICY_ACCOUNT_HEADER_BYTES == 56);
    assert!(DEALER_POLICY_STAGE_ACCOUNT_BYTES <= MAX_PERMITTED_DATA_INCREASE);
    assert!(DEALER_POLICY_ACCOUNT_BYTES <= MAX_PERMITTED_DATA_INCREASE);
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StageHeaderV1 {
    stored_bump: u8,
    policy_id: [u8; 32],
    funder: [u8; 32],
    neutral_sink: [u8; 32],
    cursor: u16,
    created_slot: u64,
    expires_slot: u64,
    refundable_principal: u64,
    donation_floor: u64,
}

fn dealer_fault<T>(_: T) -> Refusal {
    ClutchError::DealerPolicyFault.into()
}

fn transfer_data(lamports: u64) -> [u8; SYSTEM_TRANSFER_DATA_LEN] {
    let mut out = [0; SYSTEM_TRANSFER_DATA_LEN];
    out[..4].copy_from_slice(&SYSTEM_IX_TRANSFER.to_le_bytes());
    out[4..].copy_from_slice(&lamports.to_le_bytes());
    out
}

fn allocate_data(space: usize) -> [u8; SYSTEM_ALLOCATE_DATA_LEN] {
    let mut out = [0; SYSTEM_ALLOCATE_DATA_LEN];
    out[..4].copy_from_slice(&SYSTEM_IX_ALLOCATE.to_le_bytes());
    out[4..].copy_from_slice(&(space as u64).to_le_bytes());
    out
}

fn assign_data(owner: &Pubkey) -> [u8; SYSTEM_ASSIGN_DATA_LEN] {
    let mut out = [0; SYSTEM_ASSIGN_DATA_LEN];
    out[..4].copy_from_slice(&SYSTEM_IX_ASSIGN.to_le_bytes());
    out[4..].copy_from_slice(&owner.to_bytes());
    out
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Outcome<[u8; N]> {
    input
        .get(offset..offset + N)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| ClutchError::DealerPolicyUploadMismatch.into())
}

fn encode_stage_header(out: &mut [u8], header: StageHeaderV1) -> Outcome<()> {
    require(
        out.len() == DEALER_POLICY_STAGE_ACCOUNT_BYTES,
        ClutchError::WrongDataLength,
    )?;
    out.fill(0);
    out[0] = DEALER_POLICY_STAGE_ACCOUNT_TAG;
    out[1] = DEALER_POLICY_STAGE_ACCOUNT_VERSION;
    out[2] = header.stored_bump;
    out[8..40].copy_from_slice(&header.policy_id);
    out[40..72].copy_from_slice(&header.funder);
    out[72..104].copy_from_slice(&header.neutral_sink);
    out[104..106].copy_from_slice(&header.cursor.to_le_bytes());
    out[106..108].copy_from_slice(&(DEALER_POLICY_BODY_BYTES as u16).to_le_bytes());
    out[108..116].copy_from_slice(&header.created_slot.to_le_bytes());
    out[116..124].copy_from_slice(&header.expires_slot.to_le_bytes());
    out[124..132].copy_from_slice(&header.refundable_principal.to_le_bytes());
    out[132..140].copy_from_slice(&header.donation_floor.to_le_bytes());
    Ok(())
}

fn decode_stage_header(input: &[u8]) -> Outcome<StageHeaderV1> {
    require(
        input.len() == DEALER_POLICY_STAGE_ACCOUNT_BYTES,
        ClutchError::WrongDataLength,
    )?;
    require(
        input[0] == DEALER_POLICY_STAGE_ACCOUNT_TAG
            && input[1] == DEALER_POLICY_STAGE_ACCOUNT_VERSION,
        ClutchError::DealerPolicyUploadMismatch,
    )?;
    require(
        input[3..8].iter().all(|byte| *byte == 0),
        ClutchError::DealerPolicyUploadMismatch,
    )?;
    require(
        u16::from_le_bytes(read_array(input, 106)?) as usize == DEALER_POLICY_BODY_BYTES,
        ClutchError::DealerPolicyUploadMismatch,
    )?;
    let header = StageHeaderV1 {
        stored_bump: input[2],
        policy_id: read_array(input, 8)?,
        funder: read_array(input, 40)?,
        neutral_sink: read_array(input, 72)?,
        cursor: u16::from_le_bytes(read_array(input, 104)?),
        created_slot: u64::from_le_bytes(read_array(input, 108)?),
        expires_slot: u64::from_le_bytes(read_array(input, 116)?),
        refundable_principal: u64::from_le_bytes(read_array(input, 124)?),
        donation_floor: u64::from_le_bytes(read_array(input, 132)?),
    };
    require(
        header.policy_id != [0; 32]
            && header.funder != [0; 32]
            && header.neutral_sink != [0; 32]
            && header.funder != header.neutral_sink
            && header.cursor as usize <= DEALER_POLICY_BODY_BYTES
            && header.created_slot < header.expires_slot
            && header.refundable_principal != 0,
        ClutchError::DealerPolicyUploadMismatch,
    )?;
    Ok(header)
}

fn require_stage(
    program_id: &Pubkey,
    stage: &AccountInfo,
    policy_id: [u8; 32],
) -> Outcome<StageHeaderV1> {
    require(stage.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(stage.is_writable, ClutchError::NotWritable)?;
    require(!stage.executable, ClutchError::ExecutableAccount)?;
    let data = stage
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let header = decode_stage_header(&data)?;
    require(
        header.policy_id == policy_id,
        ClutchError::DealerPolicyUploadMismatch,
    )?;
    expect_pda(
        stage.key,
        seeds::dealer_policy_stage_pda(program_id, &header.funder, &header.policy_id),
        Some(header.stored_bump),
    )?;
    let minimum = header
        .refundable_principal
        .checked_add(header.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        stage.lamports() >= minimum,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    Ok(header)
}

fn create_full_principal_pda<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    signer_seeds: &[&[u8]],
) -> Outcome<(u64, u64)> {
    require_creatable(target)?;
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require_system_program(system_program)?;
    require(
        space <= MAX_PERMITTED_DATA_INCREASE,
        ClutchError::AccountCreationFailed,
    )?;
    let donation = target.lamports();
    let principal = rent.minimum_balance(space)?;
    require(principal != 0, ClutchError::DealerPolicyRentMismatch)?;
    let after = donation
        .checked_add(principal)
        .ok_or(ClutchError::Arithmetic)?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(principal),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*target.key, false),
        ],
    );
    invoke(
        &transfer,
        &[payer.clone(), target.clone(), system_program.clone()],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.lamports() == after,
        ClutchError::AccountCreationFailed,
    )?;

    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(space),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.lamports() == after && target.data_len() == space && target.owner == program_id,
        ClutchError::AccountCreationFailed,
    )?;
    Ok((principal, donation))
}

fn route_final_prefund<'a>(
    final_account: &AccountInfo<'a>,
    sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    signer_seeds: &[&[u8]],
) -> Outcome<u64> {
    let donation = final_account.lamports();
    if donation == 0 {
        return Ok(0);
    }
    let sink_after = sink
        .lamports()
        .checked_add(donation)
        .ok_or(ClutchError::Arithmetic)?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(donation),
        vec![
            AccountMeta::new(*final_account.key, true),
            AccountMeta::new(*sink.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[final_account.clone(), sink.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        final_account.lamports() == 0 && sink.lamports() == sink_after,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    Ok(donation)
}

fn close_stage(stage: &AccountInfo, funder: &AccountInfo, sink: &AccountInfo) -> Outcome<()> {
    require(stage.key != funder.key, ClutchError::AccountAlias)?;
    require(stage.key != sink.key, ClutchError::AccountAlias)?;
    require(funder.key != sink.key, ClutchError::AccountAlias)?;
    require(
        funder.is_writable && sink.is_writable,
        ClutchError::NotWritable,
    )?;
    require(
        !funder.executable && !sink.executable,
        ClutchError::ExecutableAccount,
    )?;
    let data = stage
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let header = decode_stage_header(&data)?;
    drop(data);
    require(
        funder.key.to_bytes() == header.funder && sink.key.to_bytes() == header.neutral_sink,
        ClutchError::DealerPolicyUploadMismatch,
    )?;
    let surplus = stage
        .lamports()
        .checked_sub(header.refundable_principal)
        .ok_or(ClutchError::DealerPolicyRentMismatch)?;
    require(
        surplus >= header.donation_floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    let funder_after = funder
        .lamports()
        .checked_add(header.refundable_principal)
        .ok_or(ClutchError::Arithmetic)?;
    let sink_after = sink
        .lamports()
        .checked_add(surplus)
        .ok_or(ClutchError::Arithmetic)?;
    **funder
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = funder_after;
    **sink
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = sink_after;
    **stage
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = 0;
    stage
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .fill(0);
    Ok(())
}

fn begin(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(
        payload.len() == DEALER_BEGIN_POLICY_PAYLOAD_BYTES,
        ClutchError::WrongDataLength,
    )?;
    require_count(accounts, BEGIN_ACCOUNT_COUNT)?;
    require(sequence == 0, ClutchError::Replay)?;
    let policy_id = read_array(payload, 0)?;
    let neutral_sink: [u8; 32] = read_array(payload, 32)?;
    let expires_slot = u64::from_le_bytes(read_array(payload, 64)?);
    require(
        policy_id != [0; 32] && neutral_sink != [0; 32],
        ClutchError::DealerPolicyUploadMismatch,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    require(
        accounts[0].key != accounts[1].key && accounts[0].key.to_bytes() != neutral_sink,
        ClutchError::AccountAlias,
    )?;
    let rent = read_rent(&accounts[3])?;
    let current_slot = read_clock_slot(&accounts[4])?;
    let lifetime = expires_slot
        .checked_sub(current_slot)
        .ok_or(ClutchError::InvalidArtifactExpiry)?;
    require(
        (MIN_UPLOAD_LIFETIME_SLOTS..=MAX_UPLOAD_LIFETIME_SLOTS).contains(&lifetime),
        ClutchError::InvalidArtifactExpiry,
    )?;
    let funder = accounts[0].key.to_bytes();
    let (stage_address, bump) = seeds::dealer_policy_stage_pda(program_id, &funder, &policy_id);
    expect_pda(accounts[1].key, (stage_address, bump), None)?;
    let (principal, donation) = create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[1],
        &accounts[2],
        &rent,
        DEALER_POLICY_STAGE_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_POLICY_STAGE,
            &funder,
            &policy_id,
            &[bump],
        ],
    )?;
    let header = StageHeaderV1 {
        stored_bump: bump,
        policy_id,
        funder,
        neutral_sink,
        cursor: 0,
        created_slot: current_slot,
        expires_slot,
        refundable_principal: principal,
        donation_floor: donation,
    };
    let mut data = accounts[1]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    encode_stage_header(&mut data, header)?;
    require(
        decode_stage_header(&data)? == header,
        ClutchError::MismatchedState,
    )
}

fn write(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(
        payload.len() == DEALER_WRITE_POLICY_PAYLOAD_BYTES,
        ClutchError::WrongDataLength,
    )?;
    require_count(accounts, WRITE_ACCOUNT_COUNT)?;
    let policy_id = read_array(payload, 0)?;
    let cursor = u16::from_le_bytes(read_array(payload, 32)?);
    let chunk_len = u16::from_le_bytes(read_array(payload, 34)?) as usize;
    require_signer(&accounts[0])?;
    require(
        accounts[0].key != accounts[1].key,
        ClutchError::AccountAlias,
    )?;
    let slot = read_clock_slot(&accounts[2])?;
    let header = require_stage(program_id, &accounts[1], policy_id)?;
    require(
        accounts[0].key.to_bytes() == header.funder,
        ClutchError::UnauthorizedActor,
    )?;
    require(slot <= header.expires_slot, ClutchError::ArtifactExpired)?;
    require(sequence == u64::from(header.cursor), ClutchError::Replay)?;
    require(cursor == header.cursor, ClutchError::Replay)?;
    require(
        chunk_len != 0 && chunk_len <= DEALER_POLICY_CHUNK_BYTES,
        ClutchError::DealerPolicyUploadMismatch,
    )?;
    let end = usize::from(cursor)
        .checked_add(chunk_len)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        end <= DEALER_POLICY_BODY_BYTES,
        ClutchError::DealerPolicyUploadMismatch,
    )?;
    require(
        payload[36 + chunk_len..36 + DEALER_POLICY_CHUNK_BYTES]
            .iter()
            .all(|byte| *byte == 0),
        ClutchError::DealerPolicyUploadMismatch,
    )?;
    let mut data = accounts[1]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data[DEALER_POLICY_STAGE_HEADER_BYTES + usize::from(cursor)
        ..DEALER_POLICY_STAGE_HEADER_BYTES + end]
        .copy_from_slice(&payload[36..36 + chunk_len]);
    data[104..106].copy_from_slice(&(end as u16).to_le_bytes());
    require(
        decode_stage_header(&data)?.cursor as usize == end,
        ClutchError::MismatchedState,
    )
}

fn seal(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(
        payload.len() == DEALER_POLICY_ID_PAYLOAD_BYTES,
        ClutchError::WrongDataLength,
    )?;
    require_count(accounts, SEAL_ACCOUNT_COUNT)?;
    let policy_id = read_array(payload, 0)?;
    require(
        sequence == DEALER_POLICY_BODY_BYTES as u64,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    let header = require_stage(program_id, &accounts[1], policy_id)?;
    require(
        accounts[0].key.to_bytes() == header.funder,
        ClutchError::UnauthorizedActor,
    )?;
    require(
        accounts[3].key.to_bytes() == header.neutral_sink,
        ClutchError::DealerPolicyUploadMismatch,
    )?;
    require(
        accounts[0].key != accounts[1].key
            && accounts[0].key != accounts[2].key
            && accounts[0].key != accounts[3].key
            && accounts[1].key != accounts[2].key
            && accounts[1].key != accounts[3].key
            && accounts[2].key != accounts[3].key,
        ClutchError::AccountAlias,
    )?;
    let slot = read_clock_slot(&accounts[6])?;
    require(slot <= header.expires_slot, ClutchError::ArtifactExpired)?;
    require(
        header.cursor as usize == DEALER_POLICY_BODY_BYTES,
        ClutchError::ArtifactIncomplete,
    )?;
    let rent = read_rent(&accounts[5])?;
    require_system_program(&accounts[4])?;
    let stage_data = accounts[1]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let body = &stage_data[DEALER_POLICY_STAGE_HEADER_BYTES..];
    let policy = DealerPolicyV1::decode(body).map_err(dealer_fault)?;
    require(
        policy.neutral_sink.bytes() == header.neutral_sink,
        ClutchError::DealerPolicyFault,
    )?;
    let observed = solana_sha256_hasher::hashv(&[
        clutch_dealer_runtime_contract::DEALER_POLICY_CONTENT_DOMAIN_V1,
        body,
    ])
    .to_bytes();
    require(observed == policy_id, ClutchError::DealerPolicyFault)?;
    #[cfg(not(target_os = "solana"))]
    require(
        policy.policy_id().map_err(dealer_fault)?.bytes() == observed,
        ClutchError::DealerPolicyFault,
    )?;
    drop(stage_data);

    let (final_address, bump) = seeds::dealer_policy_pda(program_id, &policy_id);
    expect_pda(accounts[2].key, (final_address, bump), None)?;
    require_creatable(&accounts[2])?;
    require(accounts[3].is_writable, ClutchError::NotWritable)?;
    require(!accounts[3].executable, ClutchError::ExecutableAccount)?;
    let donation = route_final_prefund(
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &[seeds::SEED_DEALER_POLICY, &policy_id, &[bump]],
    )?;
    let (principal, observed_after_drain) = create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[2],
        &accounts[4],
        &rent,
        DEALER_POLICY_ACCOUNT_BYTES,
        &[seeds::SEED_DEALER_POLICY, &policy_id, &[bump]],
    )?;
    require(
        observed_after_drain == 0,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    {
        let mut final_data = accounts[2]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        final_data.fill(0);
        final_data[0] = DEALER_POLICY_ACCOUNT_TAG;
        final_data[1] = DEALER_POLICY_ACCOUNT_VERSION;
        final_data[2] = bump;
        final_data[8..40].copy_from_slice(&header.funder);
        final_data[40..48].copy_from_slice(&principal.to_le_bytes());
        final_data[48..56].copy_from_slice(&donation.to_le_bytes());
        let stage_data = accounts[1]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        final_data[DEALER_POLICY_ACCOUNT_HEADER_BYTES..]
            .copy_from_slice(&stage_data[DEALER_POLICY_STAGE_HEADER_BYTES..]);
    }
    close_stage(&accounts[1], &accounts[0], &accounts[3])?;
    validate_catalog_account(
        program_id,
        &accounts[2],
        policy_id,
        header.funder,
        principal,
        donation,
    )
}

fn abort(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(
        payload.len() == DEALER_POLICY_ID_PAYLOAD_BYTES,
        ClutchError::WrongDataLength,
    )?;
    require_count(accounts, ABORT_ACCOUNT_COUNT)?;
    let policy_id = read_array(payload, 0)?;
    require_signer(&accounts[0])?;
    let header = require_stage(program_id, &accounts[1], policy_id)?;
    require(sequence == u64::from(header.cursor), ClutchError::Replay)?;
    require(
        accounts[2].key.to_bytes() == header.funder,
        ClutchError::ArtifactRefundMismatch,
    )?;
    require(
        accounts[3].key.to_bytes() == header.neutral_sink,
        ClutchError::DealerPolicyUploadMismatch,
    )?;
    require(
        accounts[1].key != accounts[2].key
            && accounts[1].key != accounts[3].key
            && accounts[2].key != accounts[3].key
            && accounts[0].key != accounts[1].key
            && accounts[0].key != accounts[3].key,
        ClutchError::AccountAlias,
    )?;
    let slot = read_clock_slot(&accounts[4])?;
    require(
        accounts[0].key.to_bytes() == header.funder || slot > header.expires_slot,
        ClutchError::UnauthorizedActor,
    )?;
    close_stage(&accounts[1], &accounts[2], &accounts[3])
}

/// Authenticate one immutable catalog wrapper and recompute its owning policy.
fn validate_catalog_account(
    program_id: &Pubkey,
    account: &AccountInfo,
    policy_id: [u8; 32],
    funder: [u8; 32],
    principal: u64,
    creation_donation: u64,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        account.data_len() == DEALER_POLICY_ACCOUNT_BYTES,
        ClutchError::WrongDataLength,
    )?;
    // Unsolicited post-creation lamports are a donation, never a way to make a
    // content-addressed catalog unusable. The constructor and SVM evidence pin
    // the exact initial balance; later readers enforce only its rent floor.
    require(
        account.lamports() >= principal,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        data[0] == DEALER_POLICY_ACCOUNT_TAG
            && data[1] == DEALER_POLICY_ACCOUNT_VERSION
            && data[3..8].iter().all(|byte| *byte == 0)
            && data[8..40] == funder
            && u64::from_le_bytes(read_array(&data, 40)?) == principal
            && u64::from_le_bytes(read_array(&data, 48)?) == creation_donation,
        ClutchError::DealerPolicyUploadMismatch,
    )?;
    expect_pda(
        account.key,
        seeds::dealer_policy_pda(program_id, &policy_id),
        Some(data[2]),
    )?;
    let body = &data[DEALER_POLICY_ACCOUNT_HEADER_BYTES..];
    DealerPolicyV1::decode(body).map_err(dealer_fault)?;
    let observed = solana_sha256_hasher::hashv(&[
        clutch_dealer_runtime_contract::DEALER_POLICY_CONTENT_DOMAIN_V1,
        body,
    ])
    .to_bytes();
    require(observed == policy_id, ClutchError::DealerPolicyFault)
}

/// Execute one strictly allocated Dealer-policy transport action.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    action: DealerPolicyAction,
    payload: &[u8],
) -> Outcome<()> {
    match action {
        DealerPolicyAction::BeginPolicy => begin(program_id, accounts, sequence, payload),
        DealerPolicyAction::WritePolicy => write(program_id, accounts, sequence, payload),
        DealerPolicyAction::SealPolicy => seal(program_id, accounts, sequence, payload),
        DealerPolicyAction::AbortPolicy => abort(program_id, accounts, sequence, payload),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> StageHeaderV1 {
        StageHeaderV1 {
            stored_bump: 7,
            policy_id: [1; 32],
            funder: [2; 32],
            neutral_sink: [3; 32],
            cursor: 192,
            created_slot: 10,
            expires_slot: 100,
            refundable_principal: 55,
            donation_floor: 1,
        }
    }

    #[test]
    fn stage_header_is_exact_and_hostile() {
        let mut bytes = [0; DEALER_POLICY_STAGE_ACCOUNT_BYTES];
        encode_stage_header(&mut bytes, header()).unwrap();
        assert_eq!(decode_stage_header(&bytes).unwrap(), header());
        for offset in [0, 1, 3, 106, 107] {
            let mut hostile = bytes;
            hostile[offset] ^= 0xff;
            assert!(decode_stage_header(&hostile).is_err(), "offset {offset}");
        }
        let mut zero_principal = bytes;
        zero_principal[124..132].fill(0);
        assert!(decode_stage_header(&zero_principal).is_err());
        assert!(decode_stage_header(&bytes[..bytes.len() - 1]).is_err());
    }

    #[test]
    fn system_payloads_are_frozen() {
        assert_eq!(&transfer_data(9)[..4], &2u32.to_le_bytes());
        assert_eq!(&allocate_data(17)[..4], &8u32.to_le_bytes());
        assert_eq!(
            &assign_data(&Pubkey::new_from_array([9; 32]))[..4],
            &1u32.to_le_bytes()
        );
    }
}
