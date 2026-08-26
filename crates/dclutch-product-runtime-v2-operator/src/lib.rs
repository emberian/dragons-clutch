//! Chain-derived Product Runtime V2 compilation and unsigned admission plans.
//!
//! This host adapter compiles exact caller-buffer records, hashes their entire
//! canonical bodies, derives Registry raw/staging PDAs, reacquires finalized
//! observations, and constructs one unsigned instruction. It never signs,
//! submits, creates, funds, or mutates an account.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use dclutch_product_runtime_v2::{
    ContentId, PortfolioInputV2, ResultDomainInputV2, compile_portfolio_v2,
    compile_result_domain_v2,
};
use dclutch_product_runtime_v2_admission::{
    ADMISSION_RECEIPT_BYTES_V2, ADMISSION_REQUEST_BYTES_V2, AdmissionReceiptV2, AdmissionRequestV2,
    FinalizedRecordCoordinateV2, PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2,
    PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

/// Compiler or chain-derived instruction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A runtime Product compiler or decoder refused.
    RuntimeProduct,
    /// Product record or receipt composition refused.
    Admission,
    /// Domain and portfolio widths did not join exactly.
    WidthMismatch,
    /// A finalized observation was stale or at another slot.
    ObservationMismatch,
    /// Registry, raw, staging, rent, or receipt account authority differed.
    AccountAuthority,
    /// Raw bytes, exact digest, or derived coordinate differed.
    RecordMismatch,
    /// A caller buffer had the wrong exact size.
    OutputLength,
}

/// Operator result alias.
pub type Result<T> = core::result::Result<T, Error>;

/// Irreducible Product authoring inputs. Child content digests and PDAs are
/// derived by the compiler and are never caller authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductCompilationInputV2<'a> {
    /// Stable Product semantic identity.
    pub product_id: ContentId,
    /// Exact source coordinate domain.
    pub coordinate_domain_id: ContentId,
    /// Exact result unit.
    pub result_unit_id: ContentId,
    /// Exact native claim basis.
    pub claim_basis_id: ContentId,
    /// Product-selected liability basis.
    pub liability_basis_id: ContentId,
    /// Product-selected representation semantic release.
    pub representation_release_id: ContentId,
    /// Product-selected coordinate mapping semantic release.
    pub mapping_release_id: ContentId,
    /// Positive common cut denominator.
    pub cut_denominator: u64,
    /// Runtime strictly increasing cut numerators.
    pub cuts: &'a [i128],
    /// Positive common exact portfolio denominator.
    pub portfolio_denominator: u64,
    /// Runtime portfolio coefficient numerators.
    pub coefficients: &'a [u64],
}

/// Exact compiled and derived record coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledProductRecordsV2 {
    /// Product/domain/portfolio finalized-record references.
    pub receipt: AdmissionReceiptV2,
    /// Admission request selecting all three exact content digests.
    pub request: AdmissionRequestV2,
    /// Runtime native outcome count.
    pub outcome_count: u32,
}

/// Compile canonical Product/domain/portfolio bytes and derive their Registry
/// coordinates. All caller buffers are validated before any is modified.
pub fn compile_product_records_v2(
    registry_program: Pubkey,
    input: ProductCompilationInputV2<'_>,
    product_output: &mut [u8],
    domain_output: &mut [u8],
    portfolio_output: &mut [u8],
) -> Result<CompiledProductRecordsV2> {
    let expected_outcomes = input
        .cuts
        .len()
        .checked_add(2)
        .ok_or(Error::WidthMismatch)?;
    if input.coefficients.len() != expected_outcomes
        || product_output.len() != PRODUCT_RECORD_BYTES_V2
    {
        return Err(Error::OutputLength);
    }
    compile_result_domain_v2(
        ResultDomainInputV2 {
            product_id: input.product_id,
            coordinate_domain_id: input.coordinate_domain_id,
            result_unit_id: input.result_unit_id,
            liability_basis_id: input.liability_basis_id,
            representation_release_id: input.representation_release_id,
            mapping_release_id: input.mapping_release_id,
            cut_denominator: input.cut_denominator,
            cuts: input.cuts,
        },
        domain_output,
    )
    .map_err(|_| Error::RuntimeProduct)?;
    let domain_digest = digest(domain_output)?;
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id: input.product_id,
            result_domain_id: domain_digest,
            claim_basis_id: input.claim_basis_id,
            liability_basis_id: input.liability_basis_id,
            representation_release_id: input.representation_release_id,
            denominator: input.portfolio_denominator,
            coefficients: input.coefficients,
        },
        portfolio_output,
    )
    .map_err(|_| Error::RuntimeProduct)?;
    let portfolio_digest = digest(portfolio_output)?;
    ProductRecordV2::new(input.product_id, domain_digest, portfolio_digest)
        .encode_into(product_output)
        .map_err(|_| Error::Admission)?;
    let product_digest = digest(product_output)?;
    let product = coordinate(
        registry_program,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        product_digest,
    )?;
    let result_domain = coordinate(registry_program, RESULT_DOMAIN_SCHEMA_ID_V2, domain_digest)?;
    let portfolio = coordinate(registry_program, PORTFOLIO_SCHEMA_ID_V2, portfolio_digest)?;
    let outcome_count = u32::try_from(expected_outcomes).map_err(|_| Error::WidthMismatch)?;
    Ok(CompiledProductRecordsV2 {
        receipt: AdmissionReceiptV2 {
            product,
            result_domain,
            portfolio,
        },
        request: AdmissionRequestV2 {
            product_digest,
            result_domain_digest: domain_digest,
            portfolio_digest,
        },
        outcome_count,
    })
}

/// One finalized account observation supplied by a bounded RPC reader.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountObservationV2<'a> {
    /// Finalized slot attached to this exact read.
    pub slot: u64,
    /// Account identity.
    pub key: Pubkey,
    /// Account owner.
    pub owner: Pubkey,
    /// Observed lamports.
    pub lamports: u64,
    /// Executable bit.
    pub executable: bool,
    /// Exact observed account bytes.
    pub data: &'a [u8],
}

/// Reacquired raw/staging pair and the chain-derived raw Rent minimum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizedRecordObservationV2<'a> {
    /// Registry-owned exact raw record.
    pub raw: AccountObservationV2<'a>,
    /// System-owned vacant staging PDA. Nonzero dust is permitted.
    pub staging: AccountObservationV2<'a>,
    /// Same-snapshot minimum balance for the exact raw byte width.
    pub raw_rent_minimum: u64,
}

/// Same-finalized snapshot for building an admission transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionStateV2<'a> {
    /// Executable Registry program observation.
    pub registry: AccountObservationV2<'a>,
    /// Writable, preallocated admission-program receipt output.
    pub receipt_output: AccountObservationV2<'a>,
    /// Canonical Rent sysvar observation.
    pub rent: AccountObservationV2<'a>,
    /// Product raw/staging observation.
    pub product: FinalizedRecordObservationV2<'a>,
    /// Result-domain raw/staging observation.
    pub result_domain: FinalizedRecordObservationV2<'a>,
    /// Portfolio raw/staging observation.
    pub portfolio: FinalizedRecordObservationV2<'a>,
}

/// Exact unsigned admission instruction and reference-only receipt bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionInstructionPlanV2 {
    /// Exact unsigned instruction.
    pub instruction: Instruction,
    /// Receipt body expected after successful execution.
    pub receipt_bytes: [u8; ADMISSION_RECEIPT_BYTES_V2],
    /// Shared finalized observation slot.
    pub observation_slot: u64,
}

/// Recheck finalized state and build one exact unsigned admission instruction.
pub fn build_admission_instruction_v2(
    admission_program: Pubkey,
    compiled: CompiledProductRecordsV2,
    state: AdmissionStateV2<'_>,
) -> Result<AdmissionInstructionPlanV2> {
    let slot = state.registry.slot;
    let observations = [
        state.receipt_output,
        state.rent,
        state.product.raw,
        state.product.staging,
        state.result_domain.raw,
        state.result_domain.staging,
        state.portfolio.raw,
        state.portfolio.staging,
    ];
    if observations
        .iter()
        .any(|observation| observation.slot != slot)
    {
        return Err(Error::ObservationMismatch);
    }
    if !state.registry.executable
        || state.receipt_output.owner != admission_program
        || state.receipt_output.executable
        || state.receipt_output.data.len() != ADMISSION_RECEIPT_BYTES_V2
        || state.receipt_output.data.iter().any(|byte| *byte != 0)
        || state.rent.key != sysvar::rent::ID
        || state.rent.owner != sysvar::ID
        || state.rent.executable
    {
        return Err(Error::AccountAuthority);
    }
    validate_record(state.registry.key, compiled.receipt.product, state.product)?;
    validate_record(
        state.registry.key,
        compiled.receipt.result_domain,
        state.result_domain,
    )?;
    validate_record(
        state.registry.key,
        compiled.receipt.portfolio,
        state.portfolio,
    )?;
    let mut data = [0_u8; ADMISSION_REQUEST_BYTES_V2];
    compiled
        .request
        .encode_into(&mut data)
        .map_err(|_| Error::Admission)?;
    let mut receipt_bytes = [0_u8; ADMISSION_RECEIPT_BYTES_V2];
    compiled
        .receipt
        .encode_into(&mut receipt_bytes)
        .map_err(|_| Error::Admission)?;
    Ok(AdmissionInstructionPlanV2 {
        instruction: Instruction {
            program_id: admission_program,
            accounts: vec![
                AccountMeta::new(state.receipt_output.key, false),
                AccountMeta::new_readonly(state.registry.key, false),
                AccountMeta::new_readonly(state.product.raw.key, false),
                AccountMeta::new_readonly(state.product.staging.key, false),
                AccountMeta::new_readonly(state.result_domain.raw.key, false),
                AccountMeta::new_readonly(state.result_domain.staging.key, false),
                AccountMeta::new_readonly(state.portfolio.raw.key, false),
                AccountMeta::new_readonly(state.portfolio.staging.key, false),
                AccountMeta::new_readonly(state.rent.key, false),
            ],
            data: data.to_vec(),
        },
        receipt_bytes,
        observation_slot: slot,
    })
}

fn validate_record(
    registry: Pubkey,
    coordinate: FinalizedRecordCoordinateV2,
    observation: FinalizedRecordObservationV2<'_>,
) -> Result<()> {
    let raw_key = pubkey(coordinate.raw_account);
    let staging_key = pubkey(coordinate.staging_account);
    if observation.raw.key != raw_key
        || observation.raw.owner != registry
        || observation.raw.executable
        || observation.raw.lamports < observation.raw_rent_minimum
        || observation.staging.key != staging_key
        || observation.staging.owner != system_program::ID
        || observation.staging.executable
        || !observation.staging.data.is_empty()
        || digest(observation.raw.data)? != coordinate.content_digest
    {
        return Err(Error::RecordMismatch);
    }
    Ok(())
}

fn coordinate(
    registry_program: Pubkey,
    schema: [u8; 32],
    content_digest: ContentId,
) -> Result<FinalizedRecordCoordinateV2> {
    let (raw, _) = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &content_digest.to_bytes()],
        &registry_program,
    );
    let (staging, _) = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &schema,
            &content_digest.to_bytes(),
        ],
        &registry_program,
    );
    Ok(FinalizedRecordCoordinateV2 {
        schema_id: ContentId::new(schema).map_err(|_| Error::Admission)?,
        content_digest,
        raw_account: ContentId::new(raw.to_bytes()).map_err(|_| Error::Admission)?,
        staging_account: ContentId::new(staging.to_bytes()).map_err(|_| Error::Admission)?,
    })
}

fn digest(bytes: &[u8]) -> Result<ContentId> {
    ContentId::new(hash(bytes).to_bytes()).map_err(|_| Error::RecordMismatch)
}

fn pubkey(identity: ContentId) -> Pubkey {
    Pubkey::new_from_array(identity.to_bytes())
}
