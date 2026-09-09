//! Pure planning and instruction construction for Structured receipt lifecycle actions.
//!
//! RPC acquisition lives in [`discovery`]. This module consumes only typed,
//! caller-owned observations, derives the Claims caller authority from the
//! lifecycle request, and emits unsigned instructions. It never reads a
//! wallet, signs, submits, or depends on the local-validator bootstrap.

pub mod discovery;
pub mod types;

pub use types::*;

use dclutch_claims::rational_lifecycle::{LifecycleActionV2, LifecycleRequestV2};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

/// Stable refusal from pure Structured lifecycle instruction construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredLifecycleConstructionErrorV1 {
    /// Claims refused the lifecycle request bytes.
    Lifecycle(dclutch_claims::rational_lifecycle::Error),
    /// The lifecycle action did not match the requested physical frame.
    Action,
    /// A coordinate action omitted its single canonical coordinate row.
    Coordinate,
    /// A semantic account differed from the lifecycle request it must serve.
    PhysicalFrame,
    /// The selected release could not derive the Trading caller authority.
    CallerAuthority(dclutch_registry::release_set::Error),
}

/// Every non-derived account in the fixed receipt Claims frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredReceiptPhysicalAccountsV1 {
    /// Current Trading program.
    pub trading: Pubkey,
    /// Current Trading ProgramData.
    pub trading_programdata: Pubkey,
    /// Current Claims program.
    pub claims: Pubkey,
    /// Current Claims ProgramData.
    pub claims_programdata: Pubkey,
    /// Immutable Registry program.
    pub registry: Pubkey,
    /// Activated release-set cache.
    pub activation_cache: Pubkey,
    /// Finalized representation descriptor record.
    pub descriptor_raw: Pubkey,
    /// Vacant representation descriptor staging cursor.
    pub descriptor_staging: Pubkey,
    /// Claims-derived representation authority.
    pub representation_authority: Pubkey,
    /// Closeable receipt Mint.
    pub receipt_mint: Pubkey,
    /// Lifecycle-scoped RentCredit.
    pub rent_credit: Pubkey,
    /// dClutch Rent program.
    pub rent_program: Pubkey,
    /// Claims liability-basis Market.
    pub claims_market: Pubkey,
    /// Core Market.
    pub core_market: Pubkey,
    /// Current Core program.
    pub core: Pubkey,
    /// Current Core ProgramData.
    pub core_programdata: Pubkey,
}

/// The additional accounts in one coordinate lifecycle Claims frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredCoordinatePhysicalAccountsV1 {
    /// Fixed receipt/common accounts.
    pub common: StructuredReceiptPhysicalAccountsV1,
    /// Claims child-authority PDA derived for this coordinate request.
    pub child_authority: Pubkey,
    /// Claims liability-basis Position.
    pub position: Pubkey,
    /// Protocol Position admission record.
    pub admission: Pubkey,
    /// Closeable shard Mint.
    pub shard_mint: Pubkey,
    /// Claims-derived Structured custody token account.
    pub structured_custody: Pubkey,
    /// Claims custody-owner PDA.
    pub owner: Pubkey,
    /// Finalized ProductBasis record.
    pub basis_record: Pubkey,
    /// Vacant ProductBasis staging cursor.
    pub basis_staging: Pubkey,
    /// Finalized Product record.
    pub product_record: Pubkey,
    /// Vacant Product staging cursor.
    pub product_staging: Pubkey,
    /// Finalized ResultDomain record.
    pub result_record: Pubkey,
    /// Vacant ResultDomain staging cursor.
    pub result_staging: Pubkey,
    /// Finalized Portfolio record.
    pub portfolio_record: Pubkey,
    /// Vacant Portfolio staging cursor.
    pub portfolio_staging: Pubkey,
}

/// Construct the exact Claims child for receipt activation.
pub fn activate_receipt_claims_instruction_v1(
    accounts: StructuredReceiptPhysicalAccountsV1,
    lifecycle_bytes: &[u8],
) -> Result<Instruction, StructuredLifecycleConstructionErrorV1> {
    receipt_claims_instruction_v1(
        accounts,
        lifecycle_bytes,
        LifecycleActionV2::ActivateReceipt,
    )
}

/// Construct the exact Claims child for complete-support receipt retirement.
pub fn retire_receipt_claims_instruction_v1(
    accounts: StructuredReceiptPhysicalAccountsV1,
    lifecycle_bytes: &[u8],
) -> Result<Instruction, StructuredLifecycleConstructionErrorV1> {
    receipt_claims_instruction_v1(accounts, lifecycle_bytes, LifecycleActionV2::RetireReceipt)
}

fn receipt_claims_instruction_v1(
    accounts: StructuredReceiptPhysicalAccountsV1,
    lifecycle_bytes: &[u8],
    expected_action: LifecycleActionV2,
) -> Result<Instruction, StructuredLifecycleConstructionErrorV1> {
    let request = LifecycleRequestV2::decode(lifecycle_bytes)
        .map_err(StructuredLifecycleConstructionErrorV1::Lifecycle)?;
    let header = request.header();
    if header.action != expected_action
        || !matches!(
            header.action,
            LifecycleActionV2::ActivateReceipt | LifecycleActionV2::RetireReceipt
        )
        || (header.action == LifecycleActionV2::ActivateReceipt && header.coordinate_count != 0)
        || (header.action == LifecycleActionV2::RetireReceipt && header.coordinate_count == 0)
    {
        return Err(StructuredLifecycleConstructionErrorV1::Action);
    }
    if header.representation_authority != accounts.representation_authority.to_bytes()
        || header.receipt_mint != accounts.receipt_mint.to_bytes()
        || header.rent_credit != accounts.rent_credit.to_bytes()
        || header.rent_program != accounts.rent_program.to_bytes()
        || header.market != accounts.core_market.to_bytes()
    {
        return Err(StructuredLifecycleConstructionErrorV1::PhysicalFrame);
    }
    let outer = caller_authority(accounts.trading, lifecycle_bytes, request)?;
    let mut metas = vec![
        AccountMeta::new_readonly(outer, true),
        AccountMeta::new_readonly(accounts.trading, false),
        AccountMeta::new_readonly(accounts.trading_programdata, false),
        AccountMeta::new_readonly(accounts.claims, false),
        AccountMeta::new_readonly(accounts.claims_programdata, false),
        AccountMeta::new_readonly(accounts.registry, false),
        AccountMeta::new_readonly(accounts.activation_cache, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(accounts.descriptor_raw, false),
        AccountMeta::new_readonly(accounts.descriptor_staging, false),
        AccountMeta::new_readonly(accounts.representation_authority, false),
        AccountMeta::new(accounts.receipt_mint, false),
        AccountMeta::new_readonly(header.token_program.into(), false),
        if header.action == LifecycleActionV2::RetireReceipt {
            AccountMeta::new(accounts.rent_credit, false)
        } else {
            AccountMeta::new_readonly(accounts.rent_credit, false)
        },
        AccountMeta::new_readonly(accounts.rent_program, false),
        AccountMeta::new_readonly(accounts.claims_market, false),
        AccountMeta::new_readonly(accounts.core_market, false),
        AccountMeta::new_readonly(accounts.core, false),
        AccountMeta::new_readonly(accounts.core_programdata, false),
    ];
    if header.action == LifecycleActionV2::RetireReceipt {
        for row in request.coordinates() {
            let row = row.map_err(StructuredLifecycleConstructionErrorV1::Lifecycle)?;
            metas.extend(
                [
                    row.shard_mint,
                    row.structured_custody_account,
                    row.claims_custody_owner,
                    row.claims_custody_position,
                    row.position_admission,
                ]
                .into_iter()
                .map(|key| AccountMeta::new_readonly(Pubkey::new_from_array(key), false)),
            );
        }
    }
    Ok(Instruction {
        program_id: accounts.claims,
        accounts: metas,
        data: lifecycle_bytes.to_vec(),
    })
}

/// Construct the exact Claims child for one coordinate activation or retirement.
pub fn coordinate_claims_instruction_v1(
    accounts: StructuredCoordinatePhysicalAccountsV1,
    lifecycle_bytes: &[u8],
) -> Result<Instruction, StructuredLifecycleConstructionErrorV1> {
    let request = LifecycleRequestV2::decode(lifecycle_bytes)
        .map_err(StructuredLifecycleConstructionErrorV1::Lifecycle)?;
    let header = request.header();
    if !matches!(
        header.action,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate
    ) || header.coordinate_count != 1
    {
        return Err(StructuredLifecycleConstructionErrorV1::Action);
    }
    let mut coordinates = request.coordinates();
    let row = coordinates
        .next()
        .ok_or(StructuredLifecycleConstructionErrorV1::Coordinate)?
        .map_err(StructuredLifecycleConstructionErrorV1::Lifecycle)?;
    if coordinates.next().is_some() {
        return Err(StructuredLifecycleConstructionErrorV1::Coordinate);
    }
    if header.representation_authority != accounts.common.representation_authority.to_bytes()
        || header.receipt_mint != accounts.common.receipt_mint.to_bytes()
        || header.rent_credit != accounts.common.rent_credit.to_bytes()
        || header.rent_program != accounts.common.rent_program.to_bytes()
        || header.market != accounts.common.core_market.to_bytes()
        || row.claims_custody_position != accounts.position.to_bytes()
        || row.position_admission != accounts.admission.to_bytes()
        || row.shard_mint != accounts.shard_mint.to_bytes()
        || row.structured_custody_account != accounts.structured_custody.to_bytes()
        || row.claims_custody_owner != accounts.owner.to_bytes()
    {
        return Err(StructuredLifecycleConstructionErrorV1::PhysicalFrame);
    }
    let outer = caller_authority(accounts.common.trading, lifecycle_bytes, request)?;
    let mut metas = vec![
        AccountMeta::new_readonly(outer, true),
        AccountMeta::new_readonly(accounts.common.trading, false),
        AccountMeta::new_readonly(accounts.common.trading_programdata, false),
        AccountMeta::new_readonly(accounts.common.claims, false),
        AccountMeta::new_readonly(accounts.common.claims_programdata, false),
        AccountMeta::new_readonly(accounts.common.registry, false),
        AccountMeta::new_readonly(accounts.common.activation_cache, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(accounts.common.descriptor_raw, false),
        AccountMeta::new_readonly(accounts.common.descriptor_staging, false),
        AccountMeta::new_readonly(accounts.common.representation_authority, false),
        AccountMeta::new_readonly(accounts.common.receipt_mint, false),
        AccountMeta::new_readonly(header.token_program.into(), false),
        if header.action == LifecycleActionV2::RetireCoordinate {
            AccountMeta::new(accounts.common.rent_credit, false)
        } else {
            AccountMeta::new_readonly(accounts.common.rent_credit, false)
        },
        AccountMeta::new_readonly(accounts.common.rent_program, false),
        AccountMeta::new_readonly(accounts.common.claims_market, false),
        AccountMeta::new_readonly(accounts.common.core_market, false),
        AccountMeta::new_readonly(accounts.common.core, false),
        AccountMeta::new_readonly(accounts.common.core_programdata, false),
        AccountMeta::new_readonly(accounts.child_authority, true),
        AccountMeta::new(accounts.position, false),
        AccountMeta::new(accounts.admission, false),
        AccountMeta::new(accounts.shard_mint, false),
        AccountMeta::new(accounts.structured_custody, false),
        AccountMeta::new_readonly(accounts.owner, false),
        AccountMeta::new_readonly(accounts.basis_record, false),
        AccountMeta::new_readonly(accounts.basis_staging, false),
        AccountMeta::new_readonly(accounts.product_record, false),
        AccountMeta::new_readonly(accounts.product_staging, false),
        AccountMeta::new_readonly(accounts.result_record, false),
        AccountMeta::new_readonly(accounts.result_staging, false),
        AccountMeta::new_readonly(accounts.portfolio_record, false),
        AccountMeta::new_readonly(accounts.portfolio_staging, false),
    ];
    // Spell these privileges after construction so the two semantic CPI
    // authorities remain visible to reviewers and cannot drift independently.
    metas[0].is_signer = true;
    metas[20].is_signer = true;
    Ok(Instruction {
        program_id: accounts.common.claims,
        accounts: metas,
        data: lifecycle_bytes.to_vec(),
    })
}

fn caller_authority(
    trading: Pubkey,
    lifecycle_bytes: &[u8],
    request: LifecycleRequestV2<'_>,
) -> Result<Pubkey, StructuredLifecycleConstructionErrorV1> {
    let header = request.header();
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        header.release_set,
        header.market,
        ExecutionRoleV1::Trading,
        header.parent_context,
        hash(lifecycle_bytes).to_bytes(),
    )
    .map_err(StructuredLifecycleConstructionErrorV1::CallerAuthority)?;
    Ok(Pubkey::find_program_address(&seeds.as_slices(), &trading).0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_claims::rational_lifecycle::{
        LIFECYCLE_COORDINATE_BYTES_V2, LIFECYCLE_HEADER_BYTES_V2, LifecycleCoordinateV2,
        LifecycleHeaderV2,
    };
    use dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID;

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn common() -> StructuredReceiptPhysicalAccountsV1 {
        StructuredReceiptPhysicalAccountsV1 {
            trading: key(20),
            trading_programdata: key(21),
            claims: key(22),
            claims_programdata: key(23),
            registry: key(24),
            activation_cache: key(25),
            descriptor_raw: key(26),
            descriptor_staging: key(27),
            representation_authority: key(6),
            receipt_mint: key(7),
            rent_credit: key(8),
            rent_program: key(9),
            claims_market: key(28),
            core_market: key(2),
            core: key(29),
            core_programdata: key(30),
        }
    }

    fn header(action: LifecycleActionV2, coordinate_count: u32) -> LifecycleHeaderV2 {
        LifecycleHeaderV2 {
            action,
            release_set: key(1).to_bytes(),
            market: key(2).to_bytes(),
            graph_id: key(3).to_bytes(),
            descriptor_id: key(4).to_bytes(),
            parent_context: key(5).to_bytes(),
            representation_authority: key(6).to_bytes(),
            receipt_mint: key(7).to_bytes(),
            token_program: TOKEN_2022_PROGRAM_ID,
            rent_credit: key(8).to_bytes(),
            rent_program: key(9).to_bytes(),
            generation: 10,
            expected_claims_market_revision: 11,
            observed_receipt_lamports: 12,
            receipt_rent_principal: 12,
            expected_receipt_supply: 0,
            outcome_count: 3,
            coordinate_count,
            rent_credit_before: 13,
            rent_credit_after: 13,
        }
    }

    fn lifecycle_bytes(
        action: LifecycleActionV2,
        coordinate: Option<LifecycleCoordinateV2>,
    ) -> Vec<u8> {
        let mut rows = Vec::new();
        if let Some(coordinate) = coordinate {
            let mut encoded = [0; LIFECYCLE_COORDINATE_BYTES_V2];
            coordinate.encode_into(&mut encoded).expect("coordinate");
            rows.extend_from_slice(&encoded);
        }
        let request =
            LifecycleRequestV2::new(header(action, u32::from(coordinate.is_some())), &rows)
                .expect("request");
        let mut bytes = vec![0; LIFECYCLE_HEADER_BYTES_V2 + rows.len()];
        request.encode_into(&mut bytes).expect("encode request");
        bytes
    }

    fn coordinate() -> LifecycleCoordinateV2 {
        LifecycleCoordinateV2 {
            outcome: 1,
            coefficient: 2,
            shard_mint: key(40).to_bytes(),
            structured_custody_account: key(41).to_bytes(),
            claims_custody_owner: key(42).to_bytes(),
            claims_custody_position: key(43).to_bytes(),
            position_admission: key(44).to_bytes(),
            observed_shard_lamports: 0,
            observed_structured_lamports: 0,
            observed_position_lamports: 0,
            observed_admission_lamports: 0,
            shard_rent_principal: 14,
            structured_rent_principal: 15,
            position_rent_principal: 16,
            admission_rent_principal: 17,
            expected_shard_supply: 0,
            expected_structured_amount: 0,
            expected_position_revision: 0,
        }
    }

    #[test]
    fn receipt_builder_owns_exact_frame_and_action() {
        let accounts = common();
        let bytes = lifecycle_bytes(LifecycleActionV2::ActivateReceipt, None);
        let instruction =
            activate_receipt_claims_instruction_v1(accounts, &bytes).expect("instruction");
        assert_eq!(instruction.program_id, accounts.claims);
        assert_eq!(instruction.accounts.len(), 20);
        assert!(instruction.accounts[0].is_signer);
        assert!(instruction.accounts[12].is_writable);
        assert!(!instruction.accounts[14].is_writable);
        assert_eq!(instruction.data, bytes);

        let wrong = lifecycle_bytes(LifecycleActionV2::RetireReceipt, None);
        assert_eq!(
            activate_receipt_claims_instruction_v1(accounts, &wrong),
            Err(StructuredLifecycleConstructionErrorV1::Action)
        );
    }

    #[test]
    fn coordinate_builder_refuses_a_substituted_physical_coordinate() {
        let row = coordinate();
        let bytes = lifecycle_bytes(LifecycleActionV2::ActivateCoordinate, Some(row));
        let mut accounts = StructuredCoordinatePhysicalAccountsV1 {
            common: common(),
            child_authority: key(45),
            position: Pubkey::new_from_array(row.claims_custody_position),
            admission: Pubkey::new_from_array(row.position_admission),
            shard_mint: Pubkey::new_from_array(row.shard_mint),
            structured_custody: Pubkey::new_from_array(row.structured_custody_account),
            owner: Pubkey::new_from_array(row.claims_custody_owner),
            basis_record: key(46),
            basis_staging: key(47),
            product_record: key(48),
            product_staging: key(49),
            result_record: key(50),
            result_staging: key(51),
            portfolio_record: key(52),
            portfolio_staging: key(53),
        };
        let instruction = coordinate_claims_instruction_v1(accounts, &bytes).expect("instruction");
        assert_eq!(instruction.accounts.len(), 34);
        assert_eq!(
            instruction
                .accounts
                .iter()
                .filter(|meta| meta.is_signer)
                .count(),
            2
        );
        assert_eq!(instruction.accounts[20].pubkey, accounts.child_authority);

        accounts.position = key(54);
        assert_eq!(
            coordinate_claims_instruction_v1(accounts, &bytes),
            Err(StructuredLifecycleConstructionErrorV1::PhysicalFrame)
        );
    }
}
