#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Authenticated physical adapter for the Lean-owned Product payoff ABI.
//!
//! This program authenticates one finalized Product-payoff record, one
//! finalized artifact-release record, and the artifact's current Loader V3
//! deployment. It emits an immutable result certificate. It does not compile
//! Products, select releases for Markets, custody collateral, mint claims, or
//! mutate Market economics.

extern crate std;

use core::convert::TryFrom;

use dclutch_product_payoff_codec::ProductPayoff;
use dclutch_product_payoff_svm::{
    CertificateKindV1, PAYOFF_CERTIFICATE_BYTES_V1, PAYOFF_CERTIFICATE_PDA_DOMAIN_V1,
    PRODUCT_PAYOFF_ADAPTER_RELEASE_ID_V1, PRODUCT_PAYOFF_SCHEMA_RELEASE_ID_V1, PayoffCertificateV1,
    PayoffRequestV1,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1, DeploymentObservationV1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    hash::hash,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};
use solana_system_interface::instruction::create_account;

/// Exact account count for both certificate routes.
pub const PAYOFF_ACCOUNT_COUNT_V1: usize = 10;

/// Stable physical-adapter refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductPayoffError {
    /// Account count, ordering, aliasing, or privileges were invalid.
    AccountFrame = 0,
    /// The exact request wire refused.
    Instruction = 1,
    /// Canonical Rent or System identities/bytes refused.
    Sysvar = 2,
    /// The Product finalized record or its canonical decoder refused.
    ProductRecord = 3,
    /// The artifact-release finalized record refused.
    ArtifactRelease = 4,
    /// Current Loader V3 program, ProgramData, or complete ELF differed.
    Deployment = 5,
    /// Product evaluation or conservative-liability construction refused.
    Evaluation = 6,
    /// Certificate PDA, owner, vacancy, rent, or prior bytes refused.
    Certificate = 7,
    /// Checked physical width conversion refused.
    Arithmetic = 8,
    /// System account creation CPI refused.
    CreateCpi = 9,
    /// An account data borrow refused.
    Borrow = 10,
}

impl From<ProductPayoffError> for ProgramError {
    fn from(value: ProductPayoffError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(process_instruction);

/// Authenticate exact semantic and deployment records, then create or verify
/// one immutable payoff certificate.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != PAYOFF_ACCOUNT_COUNT_V1 {
        return Err(ProductPayoffError::AccountFrame.into());
    }
    let request =
        PayoffRequestV1::decode(instruction_data).map_err(|_| ProductPayoffError::Instruction)?;
    let mut iterator = accounts.iter();
    let payer = next(&mut iterator)?;
    let certificate_account = next(&mut iterator)?;
    let product_record = next(&mut iterator)?;
    let product_staging = next(&mut iterator)?;
    let artifact_record = next(&mut iterator)?;
    let artifact_staging = next(&mut iterator)?;
    let deployed_program = next(&mut iterator)?;
    let deployed_programdata = next(&mut iterator)?;
    let rent_sysvar = next(&mut iterator)?;
    let system = next(&mut iterator)?;

    validate_frame(
        accounts,
        program_id,
        payer,
        certificate_account,
        product_record,
        product_staging,
        artifact_record,
        artifact_staging,
        deployed_program,
        deployed_programdata,
        rent_sysvar,
        system,
    )?;
    let rent = authenticate_rent_and_system(rent_sysvar, system)?;
    let registry_program = *product_record.owner;
    if registry_program == *program_id
        || registry_program == system_program::ID
        || registry_program == bpf_loader_upgradeable::ID
        || artifact_record.owner != &registry_program
    {
        return Err(ProductPayoffError::ProductRecord.into());
    }

    let product = {
        let bytes = product_record
            .try_borrow_data()
            .map_err(|_| ProductPayoffError::Borrow)?;
        authenticate_finalized_record(
            product_record,
            product_staging,
            &rent,
            registry_program,
            PRODUCT_PAYOFF_SCHEMA_RELEASE_ID_V1,
            request.product_record_digest(),
            &bytes,
        )
        .map_err(|_| ProductPayoffError::ProductRecord)?;
        ProductPayoff::decode(&bytes).map_err(|_| ProductPayoffError::ProductRecord)?
    };
    let release = {
        let bytes = artifact_record
            .try_borrow_data()
            .map_err(|_| ProductPayoffError::Borrow)?;
        authenticate_finalized_record(
            artifact_record,
            artifact_staging,
            &rent,
            registry_program,
            ARTIFACT_RELEASE_SCHEMA_ID_V1,
            request.artifact_release_digest(),
            &bytes,
        )
        .map_err(|_| ProductPayoffError::ArtifactRelease)?;
        ArtifactReleaseV1::decode(&bytes).map_err(|_| ProductPayoffError::ArtifactRelease)?
    };
    if release.program().to_bytes() != program_id.to_bytes()
        || release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.semantic_release_id().to_bytes() != PRODUCT_PAYOFF_ADAPTER_RELEASE_ID_V1
    {
        return Err(ProductPayoffError::ArtifactRelease.into());
    }
    authenticate_deployment(program_id, deployed_program, deployed_programdata, release)?;

    let certificate = make_certificate(registry_program, request, product)?;
    let certificate_bytes = certificate.to_bytes();
    let created = ensure_certificate_account(
        program_id,
        payer,
        certificate_account,
        system,
        &rent,
        registry_program,
        request,
        &certificate_bytes,
    )?;
    if created {
        let mut destination = certificate_account
            .try_borrow_mut_data()
            .map_err(|_| ProductPayoffError::Borrow)?;
        if destination.len() != PAYOFF_CERTIFICATE_BYTES_V1 {
            return Err(ProductPayoffError::Certificate.into());
        }
        destination.copy_from_slice(&certificate_bytes);
    }
    Ok(())
}

fn make_certificate(
    registry_program: Pubkey,
    request: PayoffRequestV1,
    product: ProductPayoff,
) -> Result<PayoffCertificateV1, ProgramError> {
    let common = (
        registry_program.to_bytes(),
        request.product_record_digest(),
        request.artifact_release_digest(),
        product.product_id(),
        product.domain_id(),
        product.coordinate_unit_id(),
        product.payout_scale(),
    );
    match request.kind() {
        CertificateKindV1::Evaluation => {
            let payout = product
                .evaluate(request.query())
                .map_err(|_| ProductPayoffError::Evaluation)?;
            PayoffCertificateV1::evaluation(
                common.0,
                common.1,
                common.2,
                common.3,
                common.4,
                common.5,
                common.6,
                request.query(),
                payout,
                product.liability_bound(),
            )
            .map_err(|_| ProductPayoffError::Evaluation.into())
        }
        CertificateKindV1::Liability => PayoffCertificateV1::liability(
            common.0,
            common.1,
            common.2,
            common.3,
            common.4,
            common.5,
            common.6,
            request.query(),
            product.liability_bound(),
        )
        .map_err(|_| ProductPayoffError::Evaluation.into()),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_frame(
    accounts: &[AccountInfo<'_>],
    program_id: &Pubkey,
    payer: &AccountInfo<'_>,
    certificate: &AccountInfo<'_>,
    product_record: &AccountInfo<'_>,
    product_staging: &AccountInfo<'_>,
    artifact_record: &AccountInfo<'_>,
    artifact_staging: &AccountInfo<'_>,
    deployed_program: &AccountInfo<'_>,
    deployed_programdata: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
    system: &AccountInfo<'_>,
) -> ProgramResult {
    for (index, left) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(index.saturating_add(1))
            .any(|right| left.key == right.key)
        {
            return Err(ProductPayoffError::AccountFrame.into());
        }
    }
    if !payer.is_signer
        || !payer.is_writable
        || payer.executable
        || certificate.is_signer
        || !certificate.is_writable
        || certificate.executable
        || product_record.is_signer
        || product_record.is_writable
        || product_record.executable
        || product_staging.is_signer
        || product_staging.is_writable
        || product_staging.executable
        || artifact_record.is_signer
        || artifact_record.is_writable
        || artifact_record.executable
        || artifact_staging.is_signer
        || artifact_staging.is_writable
        || artifact_staging.executable
        || deployed_program.is_signer
        || deployed_program.is_writable
        || !deployed_program.executable
        || deployed_program.key != program_id
        || deployed_programdata.is_signer
        || deployed_programdata.is_writable
        || deployed_programdata.executable
        || rent.is_signer
        || rent.is_writable
        || rent.executable
        || system.is_signer
        || system.is_writable
        || !system.executable
    {
        return Err(ProductPayoffError::AccountFrame.into());
    }
    Ok(())
}

fn authenticate_rent_and_system(
    rent_account: &AccountInfo<'_>,
    system: &AccountInfo<'_>,
) -> Result<Rent, ProgramError> {
    if rent_account.key != &sysvar::rent::ID
        || rent_account.owner != &sysvar::ID
        || system.key != &system_program::ID
        || system.owner != &native_loader::ID
    {
        return Err(ProductPayoffError::Sysvar.into());
    }
    Rent::from_account_info(rent_account).map_err(|_| ProductPayoffError::Sysvar.into())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_finalized_record(
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    rent: &Rent,
    registry_program: Pubkey,
    schema_id: [u8; 32],
    expected_digest: [u8; 32],
    bytes: &[u8],
) -> ProgramResult {
    if hash(bytes).to_bytes() != expected_digest
        || raw.owner != &registry_program
        || raw.executable
        || !rent.is_exempt(raw.lamports(), bytes.len())
    {
        return Err(ProductPayoffError::ProductRecord.into());
    }
    let expected_raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema_id, &expected_digest],
        &registry_program,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema_id, &expected_digest],
        &registry_program,
    )
    .0;
    if raw.key != &expected_raw
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.executable
        || staging.lamports() != 0
        || staging.data_len() != 0
    {
        return Err(ProductPayoffError::ProductRecord.into());
    }
    Ok(())
}

fn authenticate_deployment(
    program_id: &Pubkey,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    release: ArtifactReleaseV1,
) -> ProgramResult {
    if program.key != program_id
        || program.owner != &bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.owner != &bpf_loader_upgradeable::ID
        || programdata.executable
        || programdata.key.to_bytes() != release.programdata()
    {
        return Err(ProductPayoffError::Deployment.into());
    }
    let expected_programdata =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if programdata.key != &expected_programdata {
        return Err(ProductPayoffError::Deployment.into());
    }
    let program_bytes = program
        .try_borrow_data()
        .map_err(|_| ProductPayoffError::Borrow)?;
    let program_view =
        ProgramV3View::parse(&program_bytes).map_err(|_| ProductPayoffError::Deployment)?;
    if program_view.programdata() != programdata.key.to_bytes() {
        return Err(ProductPayoffError::Deployment.into());
    }
    let programdata_bytes = programdata
        .try_borrow_data()
        .map_err(|_| ProductPayoffError::Borrow)?;
    let programdata_view =
        ProgramDataV3View::parse(&programdata_bytes).map_err(|_| ProductPayoffError::Deployment)?;
    let observation = DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        programdata_view.deployment_slot(),
        hash(programdata_view.elf()).to_bytes(),
        programdata_view.upgrade_authority(),
    )
    .map_err(|_| ProductPayoffError::Deployment)?;
    release
        .authenticate_deployment(observation)
        .map_err(|_| ProductPayoffError::Deployment.into())
}

#[allow(clippy::too_many_arguments)]
fn ensure_certificate_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    certificate: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    rent: &Rent,
    registry_program: Pubkey,
    request: PayoffRequestV1,
    expected_bytes: &[u8; PAYOFF_CERTIFICATE_BYTES_V1],
) -> Result<bool, ProgramError> {
    let kind_seed = [match request.kind() {
        CertificateKindV1::Evaluation => 0,
        CertificateKindV1::Liability => 1,
    }];
    let query_seed = request.query().to_le_bytes();
    let product_digest = request.product_record_digest();
    let artifact_digest = request.artifact_release_digest();
    let registry_bytes = registry_program.to_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[
            PAYOFF_CERTIFICATE_PDA_DOMAIN_V1,
            &registry_bytes,
            &product_digest,
            &artifact_digest,
            &kind_seed,
            &query_seed,
        ],
        program_id,
    );
    if certificate.key != &expected {
        return Err(ProductPayoffError::Certificate.into());
    }
    if certificate.owner == program_id {
        if certificate.executable
            || certificate.data_len() != PAYOFF_CERTIFICATE_BYTES_V1
            || !rent.is_exempt(certificate.lamports(), certificate.data_len())
        {
            return Err(ProductPayoffError::Certificate.into());
        }
        let bytes = certificate
            .try_borrow_data()
            .map_err(|_| ProductPayoffError::Borrow)?;
        if bytes.as_ref() != expected_bytes {
            return Err(ProductPayoffError::Certificate.into());
        }
        return Ok(false);
    }
    if certificate.owner != &system_program::ID
        || certificate.executable
        || certificate.lamports() != 0
        || certificate.data_len() != 0
    {
        return Err(ProductPayoffError::Certificate.into());
    }
    let lamports = rent.minimum_balance(PAYOFF_CERTIFICATE_BYTES_V1);
    let space =
        u64::try_from(PAYOFF_CERTIFICATE_BYTES_V1).map_err(|_| ProductPayoffError::Arithmetic)?;
    let instruction = create_account(payer.key, certificate.key, lamports, space, program_id);
    let bump_seed = [bump];
    let signer: [&[u8]; 7] = [
        PAYOFF_CERTIFICATE_PDA_DOMAIN_V1,
        &registry_bytes,
        &product_digest,
        &artifact_digest,
        &kind_seed,
        &query_seed,
        &bump_seed,
    ];
    invoke_signed(
        &instruction,
        &[payer.clone(), certificate.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ProductPayoffError::CreateCpi)?;
    if certificate.owner != program_id
        || certificate.executable
        || certificate.data_len() != PAYOFF_CERTIFICATE_BYTES_V1
        || certificate.lamports() != lamports
    {
        return Err(ProductPayoffError::CreateCpi.into());
    }
    Ok(true)
}

fn next<'accounts, 'info, I>(
    iterator: &mut I,
) -> Result<&'accounts AccountInfo<'info>, ProgramError>
where
    I: Iterator<Item = &'accounts AccountInfo<'info>>,
{
    next_account_info(iterator).map_err(|_| ProductPayoffError::AccountFrame.into())
}
