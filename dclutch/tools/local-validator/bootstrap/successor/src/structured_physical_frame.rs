//! Physical Claims frame for one finalized Structured `ActivateReceipt` child.

use dclutch_claims::rational_lifecycle::{LifecycleActionV2, LifecycleRequestV2};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::hash::hash;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{Error, Result};

/// Every non-derived account in the fixed 20-account ActivateReceipt child.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ActivateReceiptPhysicalInputsV1 {
    pub(crate) trading: Pubkey,
    pub(crate) trading_programdata: Pubkey,
    pub(crate) claims: Pubkey,
    pub(crate) claims_programdata: Pubkey,
    pub(crate) registry: Pubkey,
    pub(crate) activation_cache: Pubkey,
    pub(crate) descriptor_raw: Pubkey,
    pub(crate) descriptor_staging: Pubkey,
    pub(crate) representation_authority: Pubkey,
    pub(crate) receipt_mint: Pubkey,
    pub(crate) rent_credit: Pubkey,
    pub(crate) rent_program: Pubkey,
    pub(crate) claims_market: Pubkey,
    pub(crate) core_market: Pubkey,
    pub(crate) core: Pubkey,
    pub(crate) core_programdata: Pubkey,
}

/// Every account in the 34-account coordinate lifecycle child.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CoordinatePhysicalInputsV1 {
    pub(crate) common: ActivateReceiptPhysicalInputsV1,
    pub(crate) child_authority: Pubkey,
    pub(crate) position: Pubkey,
    pub(crate) admission: Pubkey,
    pub(crate) shard_mint: Pubkey,
    pub(crate) structured_custody: Pubkey,
    pub(crate) owner: Pubkey,
    pub(crate) basis_record: Pubkey,
    pub(crate) basis_staging: Pubkey,
    pub(crate) product_record: Pubkey,
    pub(crate) product_staging: Pubkey,
    pub(crate) result_record: Pubkey,
    pub(crate) result_staging: Pubkey,
    pub(crate) portfolio_record: Pubkey,
    pub(crate) portfolio_staging: Pubkey,
}

/// Construct exactly the Claims common frame for the finalized receipt action.
pub(crate) fn activate_receipt_claims_instruction_v1(
    input: ActivateReceiptPhysicalInputsV1,
    lifecycle_bytes: &[u8],
) -> Result<Instruction> {
    let request = LifecycleRequestV2::decode(lifecycle_bytes)
        .map_err(|error| Error::new(format!("Structured receipt lifecycle: {error:?}")))?;
    let header = request.header();
    if header.action != LifecycleActionV2::ActivateReceipt
        || header.coordinate_count != 0
        || header.representation_authority != input.representation_authority.to_bytes()
        || header.receipt_mint != input.receipt_mint.to_bytes()
        || header.rent_credit != input.rent_credit.to_bytes()
        || header.rent_program != input.rent_program.to_bytes()
        || header.market != input.core_market.to_bytes()
    {
        return Err(Error::new(
            "Structured receipt physical frame differs from its finalized lifecycle request",
        ));
    }
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        header.release_set,
        header.market,
        ExecutionRoleV1::Trading,
        header.parent_context,
        hash(lifecycle_bytes).to_bytes(),
    )
    .map_err(|error| Error::new(format!("Structured receipt caller authority: {error:?}")))?;
    let outer = Pubkey::find_program_address(&authority_seeds.as_slices(), &input.trading).0;
    let values = [
        AccountMeta::new_readonly(outer, false),
        AccountMeta::new_readonly(input.trading, false),
        AccountMeta::new_readonly(input.trading_programdata, false),
        AccountMeta::new_readonly(input.claims, false),
        AccountMeta::new_readonly(input.claims_programdata, false),
        AccountMeta::new_readonly(input.registry, false),
        AccountMeta::new_readonly(input.activation_cache, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(input.descriptor_raw, false),
        AccountMeta::new_readonly(input.descriptor_staging, false),
        AccountMeta::new_readonly(input.representation_authority, false),
        AccountMeta::new(input.receipt_mint, false),
        AccountMeta::new_readonly(header.token_program.into(), false),
        AccountMeta::new_readonly(input.rent_credit, false),
        AccountMeta::new_readonly(input.rent_program, false),
        AccountMeta::new_readonly(input.claims_market, false),
        AccountMeta::new_readonly(input.core_market, false),
        AccountMeta::new_readonly(input.core, false),
        AccountMeta::new_readonly(input.core_programdata, false),
    ];
    Ok(Instruction {
        program_id: input.claims,
        accounts: values.to_vec(),
        data: lifecycle_bytes.to_vec(),
    })
}

/// Construct exactly the Claims frame for one coordinate creation or retirement.
pub(crate) fn coordinate_claims_instruction_v1(
    input: CoordinatePhysicalInputsV1,
    lifecycle_bytes: &[u8],
) -> Result<Instruction> {
    let request = LifecycleRequestV2::decode(lifecycle_bytes)
        .map_err(|error| Error::new(format!("Structured coordinate lifecycle: {error:?}")))?;
    let header = request.header();
    let row = request
        .coordinates()
        .next()
        .ok_or_else(|| Error::new("Structured coordinate lifecycle omitted its row"))?
        .map_err(|error| Error::new(format!("Structured coordinate row: {error:?}")))?;
    if !matches!(
        header.action,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate
    ) || header.coordinate_count != 1
        || request.coordinates().count() != 1
        || header.representation_authority != input.common.representation_authority.to_bytes()
        || header.receipt_mint != input.common.receipt_mint.to_bytes()
        || header.rent_credit != input.common.rent_credit.to_bytes()
        || header.rent_program != input.common.rent_program.to_bytes()
        || header.market != input.common.core_market.to_bytes()
        || row.claims_custody_position != input.position.to_bytes()
        || row.position_admission != input.admission.to_bytes()
        || row.shard_mint != input.shard_mint.to_bytes()
        || row.structured_custody_account != input.structured_custody.to_bytes()
        || row.claims_custody_owner != input.owner.to_bytes()
    {
        return Err(Error::new(
            "Structured coordinate physical frame differs from its finalized lifecycle request",
        ));
    }
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        header.release_set,
        header.market,
        ExecutionRoleV1::Trading,
        header.parent_context,
        hash(lifecycle_bytes).to_bytes(),
    )
    .map_err(|error| Error::new(format!("Structured coordinate caller authority: {error:?}")))?;
    let outer = Pubkey::find_program_address(&authority_seeds.as_slices(), &input.common.trading).0;
    let mut values = vec![
        AccountMeta::new_readonly(outer, false),
        AccountMeta::new_readonly(input.common.trading, false),
        AccountMeta::new_readonly(input.common.trading_programdata, false),
        AccountMeta::new_readonly(input.common.claims, false),
        AccountMeta::new_readonly(input.common.claims_programdata, false),
        AccountMeta::new_readonly(input.common.registry, false),
        AccountMeta::new_readonly(input.common.activation_cache, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(input.common.descriptor_raw, false),
        AccountMeta::new_readonly(input.common.descriptor_staging, false),
        AccountMeta::new_readonly(input.common.representation_authority, false),
        AccountMeta::new_readonly(input.common.receipt_mint, false),
        AccountMeta::new_readonly(header.token_program.into(), false),
        if header.action == LifecycleActionV2::RetireCoordinate {
            AccountMeta::new(input.common.rent_credit, false)
        } else {
            AccountMeta::new_readonly(input.common.rent_credit, false)
        },
        AccountMeta::new_readonly(input.common.rent_program, false),
        AccountMeta::new_readonly(input.common.claims_market, false),
        AccountMeta::new_readonly(input.common.core_market, false),
        AccountMeta::new_readonly(input.common.core, false),
        AccountMeta::new_readonly(input.common.core_programdata, false),
        AccountMeta::new_readonly(input.child_authority, false),
        AccountMeta::new(input.position, false),
        AccountMeta::new(input.admission, false),
        AccountMeta::new(input.shard_mint, false),
        AccountMeta::new(input.structured_custody, false),
        AccountMeta::new_readonly(input.owner, false),
        AccountMeta::new_readonly(input.basis_record, false),
        AccountMeta::new_readonly(input.basis_staging, false),
        AccountMeta::new_readonly(input.product_record, false),
        AccountMeta::new_readonly(input.product_staging, false),
        AccountMeta::new_readonly(input.result_record, false),
        AccountMeta::new_readonly(input.result_staging, false),
        AccountMeta::new_readonly(input.portfolio_record, false),
        AccountMeta::new_readonly(input.portfolio_staging, false),
    ];
    values[0].is_signer = true;
    values[20].is_signer = true;
    Ok(Instruction {
        program_id: input.common.claims,
        accounts: values,
        data: lifecycle_bytes.to_vec(),
    })
}
