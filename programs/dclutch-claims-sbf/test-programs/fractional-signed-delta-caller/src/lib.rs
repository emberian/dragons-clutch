#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only real-SBF caller for Fractional SignedDelta rollback evidence.
//!
//! This program owns no protocol state or production ABI. It hostile-decodes
//! the existing Fractional request, lowers its chain-derived native effect
//! through the production Fractional Claims kernel, signs the release-scoped
//! Trading caller-authority PDA, validates Claims' sole receipt commitment
//! against returned resource bytes, and can deliberately refuse afterward.

extern crate alloc;

use alloc::vec::Vec;
use dclutch_claims_svm::{
    liability_basis_state_v2::LiabilityBasisPositionViewV2,
    signed_delta_v3::{
        DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3,
        SIGNED_DELTA_POST_RESOURCE_DIGEST_DOMAIN_V3, SIGNED_DELTA_TABLE_DIGEST_DOMAIN_V3,
        SignedDeltaV3,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_fractional_claim_contract::{
    FRACTIONAL_FAMILY_REQUEST_BYTES_V1, FractionalActionV1, FractionalFamilyRequestV1,
};
use dclutch_fractional_claims_kernel::{
    FractionalSignedDeltaInputV1, fractional_signed_delta_shape_v1,
    prepare_fractional_signed_delta_v1, validate_prepared_fractional_signed_delta_postcondition_v1,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Exact test-only wrapper bytes around the existing Fractional request.
pub const FRACTIONAL_SIGNED_DELTA_TEST_WRAPPER_BYTES: usize = 513;

const REQUEST_OFFSET: usize = 1;
const PRODUCT_OFFSET: usize = REQUEST_OFFSET + FRACTIONAL_FAMILY_REQUEST_BYTES_V1;
const LINKED_BASIS_OFFSET: usize = PRODUCT_OFFSET + 32;
const RESERVE_OWNER_OFFSET: usize = LINKED_BASIS_OFFSET + 32;
const NATIVE_CLAIMS_OFFSET: usize = RESERVE_OWNER_OFFSET + 32;
const COLLATERAL_OFFSET: usize = NATIVE_CLAIMS_OFFSET + 8;
const POST_RESERVE_OFFSET: usize = COLLATERAL_OFFSET + 8;
const POST_REVISION_OFFSET: usize = POST_RESERVE_OFFSET + 8;

/// Stable test-only caller refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FractionalSignedDeltaTestCallerError {
    /// Wrapper or canonical SignedDelta bytes were malformed.
    Instruction = 0,
    /// Claims program or forwarded account frame was malformed.
    AccountFrame = 1,
    /// Claims refused or returned another receipt/post-resource commitment.
    ClaimsCpi = 2,
    /// Deliberate refusal after Claims returned and the receipt validated.
    DeliberateLateFailure = 3,
}

impl From<FractionalSignedDeltaTestCallerError> for ProgramError {
    fn from(value: FractionalSignedDeltaTestCallerError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Lower one exact Fractional wrap through Claims; wrapper byte one refuses late.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() != FRACTIONAL_SIGNED_DELTA_TEST_WRAPPER_BYTES {
        return Err(FractionalSignedDeltaTestCallerError::Instruction.into());
    }
    let fail_after = *instruction_data
        .first()
        .ok_or(FractionalSignedDeltaTestCallerError::Instruction)?;
    if fail_after > 1 {
        return Err(FractionalSignedDeltaTestCallerError::Instruction.into());
    }
    let request_end = REQUEST_OFFSET
        .checked_add(FRACTIONAL_FAMILY_REQUEST_BYTES_V1)
        .ok_or(FractionalSignedDeltaTestCallerError::Instruction)?;
    let family_request = FractionalFamilyRequestV1::decode(
        instruction_data
            .get(REQUEST_OFFSET..request_end)
            .ok_or(FractionalSignedDeltaTestCallerError::Instruction)?,
    )
    .map_err(|_| FractionalSignedDeltaTestCallerError::Instruction)?;
    if family_request.action() != FractionalActionV1::Wrap {
        return Err(FractionalSignedDeltaTestCallerError::Instruction.into());
    }
    let claims_program = accounts
        .first()
        .ok_or(FractionalSignedDeltaTestCallerError::AccountFrame)?;
    let forwarded = accounts
        .get(1..)
        .ok_or(FractionalSignedDeltaTestCallerError::AccountFrame)?;
    if !claims_program.executable || claims_program.is_signer || claims_program.is_writable {
        return Err(FractionalSignedDeltaTestCallerError::AccountFrame.into());
    }
    if forwarded.len() != 22 {
        return Err(FractionalSignedDeltaTestCallerError::AccountFrame.into());
    }
    let market_account = forwarded
        .get(1)
        .ok_or(FractionalSignedDeltaTestCallerError::AccountFrame)?;
    let first_position = forwarded
        .get(20)
        .ok_or(FractionalSignedDeltaTestCallerError::AccountFrame)?;
    let second_position = forwarded
        .get(21)
        .ok_or(FractionalSignedDeltaTestCallerError::AccountFrame)?;
    let market_data = market_account
        .try_borrow_data()
        .map_err(|_| FractionalSignedDeltaTestCallerError::AccountFrame)?;
    let first_data = first_position
        .try_borrow_data()
        .map_err(|_| FractionalSignedDeltaTestCallerError::AccountFrame)?;
    let second_data = second_position
        .try_borrow_data()
        .map_err(|_| FractionalSignedDeltaTestCallerError::AccountFrame)?;
    let first_view = LiabilityBasisPositionViewV2::decode(&first_data)
        .map_err(|_| FractionalSignedDeltaTestCallerError::AccountFrame)?;
    let second_view = LiabilityBasisPositionViewV2::decode(&second_data)
        .map_err(|_| FractionalSignedDeltaTestCallerError::AccountFrame)?;
    if first_view.owner >= second_view.owner {
        return Err(FractionalSignedDeltaTestCallerError::AccountFrame.into());
    }
    let reserve_owner = array(instruction_data, RESERVE_OWNER_OFFSET)?;
    let actor_owner = family_request.input().owner;
    let (reserve_bytes, actor_bytes) =
        if first_view.owner == reserve_owner && second_view.owner == actor_owner {
            (&*first_data, &*second_data)
        } else if first_view.owner == actor_owner && second_view.owner == reserve_owner {
            (&*second_data, &*first_data)
        } else {
            return Err(FractionalSignedDeltaTestCallerError::AccountFrame.into());
        };
    let lowering_input = FractionalSignedDeltaInputV1 {
        request: family_request,
        semantic_product_id: array(instruction_data, PRODUCT_OFFSET)?,
        market_account: market_account.key.to_bytes(),
        market_bytes: &market_data,
        linked_basis_record_digest: array(instruction_data, LINKED_BASIS_OFFSET)?,
        claims_program: claims_program.key.to_bytes(),
        reserve_owner,
        reserve_position_bytes: reserve_bytes,
        actor_position_bytes: Some(actor_bytes),
        native_claims: u64_at(instruction_data, NATIVE_CLAIMS_OFFSET)?,
        collateral_atoms: u64_at(instruction_data, COLLATERAL_OFFSET)?,
        expected_post_reserve_native_claims: Some(u64_at(instruction_data, POST_RESERVE_OFFSET)?),
        retirement_native_burns: &[],
        post_fractional_revision: u64_at(instruction_data, POST_REVISION_OFFSET)?,
    };
    let shape = fractional_signed_delta_shape_v1(lowering_input)
        .map_err(|_| FractionalSignedDeltaTestCallerError::Instruction)?;
    if shape.position_count() != 2 {
        return Err(FractionalSignedDeltaTestCallerError::Instruction.into());
    }
    let neutral = SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0)
        .map_err(|_| FractionalSignedDeltaTestCallerError::Instruction)?;
    let mut aggregates = alloc::vec![
        neutral;
        usize::try_from(shape.claim_count())
            .map_err(|_| FractionalSignedDeltaTestCallerError::Instruction)?
    ];
    let dummy = PositionDeltaV3::new(
        PositionDeltaInputV3 {
            position_index: 0,
            outcome: 0,
            delta: SignedDeltaV3::new(DeltaDirectionV3::Debit, 1)
                .map_err(|_| FractionalSignedDeltaTestCallerError::Instruction)?,
        },
        shape.position_count(),
        shape.claim_count(),
    )
    .map_err(|_| FractionalSignedDeltaTestCallerError::Instruction)?;
    let mut rows = alloc::vec![
        dummy;
        usize::try_from(shape.position_delta_count())
            .map_err(|_| FractionalSignedDeltaTestCallerError::Instruction)?
    ];
    let mut packet = alloc::vec![0; shape.packet_bytes()];
    let lowering =
        prepare_fractional_signed_delta_v1(lowering_input, &mut aggregates, &mut rows, &mut packet)
            .map_err(|_| FractionalSignedDeltaTestCallerError::Instruction)?;
    let (position_table, aggregate_table, row_table) = lowering
        .table_bytes(&packet)
        .map_err(|_| FractionalSignedDeltaTestCallerError::Instruction)?;
    let packet_digest = hash(&packet).to_bytes();
    let table_digest = hashv(&[
        SIGNED_DELTA_TABLE_DIGEST_DOMAIN_V3,
        position_table,
        aggregate_table,
        row_table,
    ])
    .to_bytes();
    drop(second_data);
    drop(first_data);
    drop(market_data);

    let mut metas = Vec::with_capacity(forwarded.len());
    for (index, account) in forwarded.iter().enumerate() {
        let signer = index == 0;
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: packet.clone(),
    };
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(lowering.release_set())
            .map_err(|_| FractionalSignedDeltaTestCallerError::Instruction)?,
        lowering.market(),
        ExecutionRoleV1::Trading,
        lowering.request_digest(),
        packet_digest,
    )
    .map_err(|_| FractionalSignedDeltaTestCallerError::Instruction)?;
    let bump = [Pubkey::find_program_address(&seeds.as_slices(), program_id).1];
    let [domain, release, market, role, context, digest] = seeds.as_slices();
    let mut infos = Vec::with_capacity(accounts.len());
    infos.extend_from_slice(forwarded);
    infos.push(claims_program.clone());
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump]],
    )
    .map_err(|_| FractionalSignedDeltaTestCallerError::ClaimsCpi)?;

    let (producer, receipt_bytes) =
        get_return_data().ok_or(FractionalSignedDeltaTestCallerError::ClaimsCpi)?;
    let market_data = forwarded
        .get(1)
        .ok_or(FractionalSignedDeltaTestCallerError::AccountFrame)?
        .try_borrow_data()
        .map_err(|_| FractionalSignedDeltaTestCallerError::ClaimsCpi)?;
    let first_post = first_position
        .try_borrow_data()
        .map_err(|_| FractionalSignedDeltaTestCallerError::ClaimsCpi)?;
    let second_post = second_position
        .try_borrow_data()
        .map_err(|_| FractionalSignedDeltaTestCallerError::ClaimsCpi)?;
    let post_resource_digest = hashv(&[
        SIGNED_DELTA_POST_RESOURCE_DIGEST_DOMAIN_V3,
        &market_data,
        &first_post,
        &second_post,
    ])
    .to_bytes();
    if producer != *claims_program.key
        || validate_prepared_fractional_signed_delta_postcondition_v1(
            lowering,
            packet_digest,
            table_digest,
            post_resource_digest,
            &receipt_bytes,
            &market_data,
            &[&first_post, &second_post],
        )
        .is_err()
    {
        return Err(FractionalSignedDeltaTestCallerError::ClaimsCpi.into());
    }
    drop(second_post);
    drop(first_post);
    drop(market_data);
    if fail_after == 1 {
        return Err(FractionalSignedDeltaTestCallerError::DeliberateLateFailure.into());
    }
    Ok(())
}

fn array(input: &[u8], offset: usize) -> Result<[u8; 32], ProgramError> {
    let end = offset
        .checked_add(32)
        .ok_or(FractionalSignedDeltaTestCallerError::Instruction)?;
    input
        .get(offset..end)
        .ok_or(FractionalSignedDeltaTestCallerError::Instruction)?
        .try_into()
        .map_err(|_| FractionalSignedDeltaTestCallerError::Instruction.into())
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64, ProgramError> {
    let end = offset
        .checked_add(8)
        .ok_or(FractionalSignedDeltaTestCallerError::Instruction)?;
    let bytes: [u8; 8] = input
        .get(offset..end)
        .ok_or(FractionalSignedDeltaTestCallerError::Instruction)?
        .try_into()
        .map_err(|_| FractionalSignedDeltaTestCallerError::Instruction)?;
    Ok(u64::from_le_bytes(bytes))
}
