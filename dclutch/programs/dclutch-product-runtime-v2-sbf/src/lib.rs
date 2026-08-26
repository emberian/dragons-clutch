//! Executable Registry-authenticated admission for Product Runtime V2.
//!
//! The adapter persists only content-addressed finalized-record coordinates.
//! It authenticates Registry owner/PDA/hash/rent/staging evidence and then
//! invokes the safe, no-allocation Product kernel over borrowed account bytes.
//! Core and Claims remain responsible for repeating these checks and decoding
//! the referenced facts at their own trust boundaries.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

extern crate std;

use dclutch_product_runtime_v2_admission::{
    ADMISSION_RECEIPT_BYTES_V2, ADMISSION_RECEIPT_PDA_DOMAIN_V2, AdmissionReceiptV2,
    AdmissionRequestV2,
};
use dclutch_product_runtime_v2_svm_reader::{
    FinalizedRecordFrameV2, ProductRuntimeFrameV2, authenticate_product_runtime_v2,
};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::sysvar;

/// Exact executable account count.
pub const ADMISSION_ACCOUNT_COUNT_V2: usize = 9;

/// Stable executable-adapter refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionSbfErrorV2 {
    /// Account count, aliasing, privilege, or owner was invalid.
    AccountFrame = 0,
    /// Instruction bytes or request identity were invalid.
    Instruction = 1,
    /// Rent sysvar identity or bytes were invalid.
    Rent = 2,
    /// Receipt address, initial bytes, or physical funding was invalid.
    Receipt = 3,
    /// Product finalized-record evidence was invalid.
    ProductRecord = 4,
    /// Result-domain finalized-record evidence was invalid.
    ResultDomainRecord = 5,
    /// Portfolio finalized-record evidence was invalid.
    PortfolioRecord = 6,
    /// Exact Product/domain/portfolio semantic composition refused.
    Composition = 7,
    /// Account data could not be borrowed.
    Borrow = 8,
}

impl From<AdmissionSbfErrorV2> for ProgramError {
    fn from(value: AdmissionSbfErrorV2) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(process_instruction);

/// Authenticate and persist one exact Product Runtime V2 admission graph.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != ADMISSION_ACCOUNT_COUNT_V2 {
        return Err(AdmissionSbfErrorV2::AccountFrame.into());
    }
    require_distinct(accounts)?;
    let request = AdmissionRequestV2::decode(instruction_data)
        .map_err(|_| AdmissionSbfErrorV2::Instruction)?;
    let mut iter = accounts.iter();
    let receipt = next(&mut iter)?;
    let registry = next(&mut iter)?;
    let product_raw = next(&mut iter)?;
    let product_staging = next(&mut iter)?;
    let domain_raw = next(&mut iter)?;
    let domain_staging = next(&mut iter)?;
    let portfolio_raw = next(&mut iter)?;
    let portfolio_staging = next(&mut iter)?;
    let rent_account = next(&mut iter)?;

    validate_frame(
        program_id,
        receipt,
        registry,
        product_raw,
        product_staging,
        domain_raw,
        domain_staging,
        portfolio_raw,
        portfolio_staging,
        rent_account,
        request,
    )?;
    let rent = Rent::from_account_info(rent_account).map_err(|_| AdmissionSbfErrorV2::Rent)?;
    if !rent.is_exempt(receipt.lamports(), receipt.data_len()) {
        return Err(AdmissionSbfErrorV2::Receipt.into());
    }

    // Deliberately precedes record authentication. ProgramTest exercises a
    // later refusal and proves the runtime rolls this mutation back atomically.
    {
        let mut receipt_data = receipt
            .try_borrow_mut_data()
            .map_err(|_| AdmissionSbfErrorV2::Borrow)?;
        let marker = receipt_data
            .get_mut(0)
            .ok_or(AdmissionSbfErrorV2::Receipt)?;
        *marker = 0xee;
    }

    let authenticated = authenticate_product_runtime_v2(
        registry.key,
        &rent,
        request.product_digest,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: product_raw,
                staging: product_staging,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: domain_raw,
                staging: domain_staging,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: portfolio_raw,
                staging: portfolio_staging,
            },
        },
    )
    .map_err(|error| match error {
        dclutch_product_runtime_v2_svm_reader::Error::ProductRecord => {
            AdmissionSbfErrorV2::ProductRecord
        }
        dclutch_product_runtime_v2_svm_reader::Error::ResultDomainRecord => {
            AdmissionSbfErrorV2::ResultDomainRecord
        }
        dclutch_product_runtime_v2_svm_reader::Error::PortfolioRecord => {
            AdmissionSbfErrorV2::PortfolioRecord
        }
        dclutch_product_runtime_v2_svm_reader::Error::Borrow => AdmissionSbfErrorV2::Borrow,
        _ => AdmissionSbfErrorV2::Composition,
    })?;
    if authenticated.result_domain_record.content_digest != request.result_domain_digest
        || authenticated.portfolio_record.content_digest != request.portfolio_digest
    {
        return Err(AdmissionSbfErrorV2::Instruction.into());
    }
    let admitted = AdmissionReceiptV2 {
        product: authenticated
            .product_record
            .coordinate()
            .map_err(|_| AdmissionSbfErrorV2::Composition)?,
        result_domain: authenticated
            .result_domain_record
            .coordinate()
            .map_err(|_| AdmissionSbfErrorV2::Composition)?,
        portfolio: authenticated
            .portfolio_record
            .coordinate()
            .map_err(|_| AdmissionSbfErrorV2::Composition)?,
    };
    let mut encoded = [0_u8; ADMISSION_RECEIPT_BYTES_V2];
    admitted
        .encode_into(&mut encoded)
        .map_err(|_| AdmissionSbfErrorV2::Composition)?;
    receipt
        .try_borrow_mut_data()
        .map_err(|_| AdmissionSbfErrorV2::Borrow)?
        .copy_from_slice(&encoded);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_frame<'info>(
    program_id: &Pubkey,
    receipt: &AccountInfo<'info>,
    registry: &AccountInfo<'info>,
    product_raw: &AccountInfo<'info>,
    product_staging: &AccountInfo<'info>,
    domain_raw: &AccountInfo<'info>,
    domain_staging: &AccountInfo<'info>,
    portfolio_raw: &AccountInfo<'info>,
    portfolio_staging: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    request: AdmissionRequestV2,
) -> ProgramResult {
    let expected_receipt = Pubkey::find_program_address(
        &[
            ADMISSION_RECEIPT_PDA_DOMAIN_V2,
            &request.product_digest.to_bytes(),
            &request.result_domain_digest.to_bytes(),
            &request.portfolio_digest.to_bytes(),
        ],
        program_id,
    )
    .0;
    if receipt.key != &expected_receipt
        || receipt.owner != program_id
        || !receipt.is_writable
        || receipt.is_signer
        || receipt.executable
        || receipt.data_len() != ADMISSION_RECEIPT_BYTES_V2
        || registry.is_writable
        || registry.is_signer
        || !registry.executable
        || rent.key != &sysvar::rent::ID
        || rent.owner != &sysvar::ID
        || rent.is_writable
        || rent.is_signer
        || rent.executable
    {
        return Err(AdmissionSbfErrorV2::AccountFrame.into());
    }
    for account in [
        product_raw,
        product_staging,
        domain_raw,
        domain_staging,
        portfolio_raw,
        portfolio_staging,
    ] {
        if account.is_writable || account.is_signer || account.executable {
            return Err(AdmissionSbfErrorV2::AccountFrame.into());
        }
    }
    if receipt
        .try_borrow_data()
        .map_err(|_| AdmissionSbfErrorV2::Borrow)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(AdmissionSbfErrorV2::Receipt.into());
    }
    Ok(())
}

fn require_distinct(accounts: &[AccountInfo<'_>]) -> ProgramResult {
    for (left_index, left) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(left_index.saturating_add(1))
            .any(|right| left.key == right.key)
        {
            return Err(AdmissionSbfErrorV2::AccountFrame.into());
        }
    }
    Ok(())
}

fn next<'a, 'info>(
    iter: &mut core::slice::Iter<'a, AccountInfo<'info>>,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    next_account_info(iter).map_err(|_| AdmissionSbfErrorV2::AccountFrame.into())
}
