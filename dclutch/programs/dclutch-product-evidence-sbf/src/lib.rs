#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Executable exact-rational Product evidence boundary.
//!
//! One ELF supports two separately released deployments. The evaluator route
//! authenticates a finalized V2 payoff and its current Loader V3 deployment,
//! then emits an immutable evidence certificate. The admission route consumes
//! such a certificate under an authenticated Market capability manifest and
//! emits an immutable receipt. Neither route mutates Market state, custodies
//! collateral, or owns minting, settlement, or redemption authority.

extern crate std;

use core::convert::TryFrom;

use dclutch_capability_contract::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1};
use dclutch_core_contract::MarketRoot;
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_product_admission_contract::{
    AdmissionFactsV1, AdmissionRoleV1, PAYOFF_ADMISSION_RECEIPT_BYTES_V1,
    PAYOFF_ADMISSION_RECEIPT_PDA_DOMAIN_V1, PAYOFF_ADMISSION_REQUEST_MAGIC_V1,
    PRODUCT_PAYOFF_ADMISSION_RELEASE_ID_V1, PRODUCT_PAYOFF_BINDING_SCHEMA_ID_V1,
    PayoffAdmissionRequestV1, PayoffBindingV1, admit,
};
use dclutch_product_contract::{
    product::InstanceV1,
    result_domain::{FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, FiniteResultDomainV1},
};
use dclutch_product_payoff_v2_codec::ProductPayoffV2;
use dclutch_product_payoff_v2_svm::{
    CertificateKindV2, PAYOFF_CERTIFICATE_BYTES_V2, PAYOFF_CERTIFICATE_PDA_DOMAIN_V2,
    PAYOFF_REQUEST_MAGIC_V2, PRODUCT_PAYOFF_ADAPTER_RELEASE_ID_V2,
    PRODUCT_PAYOFF_SCHEMA_RELEASE_ID_V2, PayoffCertificateV2, PayoffRequestV2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1, DeploymentObservationV1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_resolution_codec::{
    RESOLUTION_CERTIFICATE_BYTES, RESOLUTION_CONTROLLER_RELEASE_ID_V3, ResolutionCertificateV1,
};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};
use solana_system_interface::instruction::create_account;

/// Exact evaluator account count.
pub const EVALUATOR_ACCOUNT_COUNT_V2: usize = 10;
/// Liability admission account count.
pub const LIABILITY_ADMISSION_ACCOUNT_COUNT_V1: usize = 28;
/// Success evaluation admission account count.
pub const SUCCESS_ADMISSION_ACCOUNT_COUNT_V1: usize = 29;
/// Failure evaluation admission account count.
pub const FAILURE_ADMISSION_ACCOUNT_COUNT_V1: usize = 28;
/// Canonical Market PDA domain owned by the Registry/Core program.
pub const MARKET_PDA_DOMAIN_V1: &[u8] = b"dclutch/market-root/v1";
/// Finalized Product-instance schema ID.
pub const PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x96, 0x20, 0xbc, 0xd9, 0xf3, 0x1a, 0x01, 0xca, 0x6f, 0x42, 0x09, 0x1c, 0x84, 0x57, 0x9d, 0x9a,
    0xcc, 0x48, 0x41, 0x27, 0xc0, 0x8d, 0x86, 0xac, 0xc4, 0x0f, 0xdd, 0x5a, 0x4c, 0xab, 0x1f, 0x14,
];
/// Finalized finite result-domain schema ID.
pub const FINITE_RESULT_DOMAIN_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x37, 0x3d, 0x8d, 0xf3, 0x60, 0x73, 0xe8, 0x45, 0x54, 0xed, 0xa9, 0x89, 0x11, 0xb8, 0x3a, 0x9c,
    0x13, 0xcb, 0x07, 0x74, 0x54, 0x8f, 0x68, 0x0c, 0xba, 0x66, 0x29, 0x13, 0xdd, 0x66, 0x0e, 0x14,
];

/// Stable executable-adapter refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductEvidenceError {
    /// Account count, ordering, aliasing, or privileges were invalid.
    AccountFrame = 0,
    /// Instruction bytes refused exact decoding.
    Instruction = 1,
    /// Canonical Rent or System identities/bytes refused.
    Sysvar = 2,
    /// A finalized raw record, digest, PDA, staging proof, or decoder refused.
    FinalizedRecord = 3,
    /// Market owner, PDA, complete bytes, phase, or identity refused.
    Market = 4,
    /// An artifact release did not bind the exact program and semantic release.
    ArtifactRelease = 5,
    /// Current Loader V3 Program, ProgramData, or complete ELF differed.
    Deployment = 6,
    /// Exact-rational Product evaluation or certificate construction refused.
    Evaluation = 7,
    /// Payoff certificate owner, digest, PDA, or immutable bytes refused.
    PayoffCertificate = 8,
    /// Resolution certificate owner, digest, release, or bytes refused.
    ResolutionCertificate = 9,
    /// Pure Market/Product/capability admission refused.
    Admission = 10,
    /// Admission receipt PDA, owner, vacancy, rent, or replay bytes refused.
    Receipt = 11,
    /// Checked physical width conversion refused.
    Arithmetic = 12,
    /// System account-creation CPI refused.
    CreateCpi = 13,
    /// Account data borrowing refused.
    Borrow = 14,
}

impl From<ProductEvidenceError> for ProgramError {
    fn from(value: ProductEvidenceError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(process_instruction);

/// Dispatch the evaluator or admission fixed wire without caller-selected
/// authority routing.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data.get(..8) {
        Some(magic) if magic == PAYOFF_REQUEST_MAGIC_V2 => {
            process_evaluator(program_id, accounts, instruction_data)
        }
        Some(magic) if magic == PAYOFF_ADMISSION_REQUEST_MAGIC_V1 => {
            process_admission(program_id, accounts, instruction_data)
        }
        _ => Err(ProductEvidenceError::Instruction.into()),
    }
}

#[inline(never)]
fn process_evaluator(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != EVALUATOR_ACCOUNT_COUNT_V2 {
        return Err(ProductEvidenceError::AccountFrame.into());
    }
    require_distinct(accounts)?;
    let request =
        PayoffRequestV2::decode(instruction_data).map_err(|_| ProductEvidenceError::Instruction)?;
    let mut iter = accounts.iter();
    let payer = next(&mut iter)?;
    let certificate = next(&mut iter)?;
    let product_raw = next(&mut iter)?;
    let product_staging = next(&mut iter)?;
    let artifact_raw = next(&mut iter)?;
    let artifact_staging = next(&mut iter)?;
    let deployed_program = next(&mut iter)?;
    let deployed_programdata = next(&mut iter)?;
    let rent_account = next(&mut iter)?;
    let system = next(&mut iter)?;
    validate_evaluator_privileges(
        program_id,
        payer,
        certificate,
        product_raw,
        product_staging,
        artifact_raw,
        artifact_staging,
        deployed_program,
        deployed_programdata,
        rent_account,
        system,
    )?;
    let rent = authenticate_rent_system(rent_account, system)?;
    let registry = *product_raw.owner;
    let product = {
        let bytes = product_raw
            .try_borrow_data()
            .map_err(|_| ProductEvidenceError::Borrow)?;
        authenticate_record(
            product_raw,
            product_staging,
            &rent,
            registry,
            PRODUCT_PAYOFF_SCHEMA_RELEASE_ID_V2,
            request.product_record_digest(),
            &bytes,
        )?;
        ProductPayoffV2::decode(&bytes).map_err(|_| ProductEvidenceError::FinalizedRecord)?
    };
    authenticate_release_and_deployment(
        artifact_raw,
        artifact_staging,
        deployed_program,
        deployed_programdata,
        &rent,
        registry,
        request.artifact_release_digest(),
        *program_id,
        PRODUCT_PAYOFF_ADAPTER_RELEASE_ID_V2,
    )?;
    let expected = make_payoff_certificate(registry, request, product)?;
    ensure_payoff_certificate(
        program_id,
        payer,
        certificate,
        system,
        &rent,
        registry,
        request,
        &expected.to_bytes(),
    )?;
    Ok(())
}

#[inline(never)]
fn process_admission(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = PayoffAdmissionRequestV1::decode(instruction_data)
        .map_err(|_| ProductEvidenceError::Instruction)?;
    let expected_count = match request.role() {
        AdmissionRoleV1::Liability => LIABILITY_ADMISSION_ACCOUNT_COUNT_V1,
        AdmissionRoleV1::SuccessEvaluation => SUCCESS_ADMISSION_ACCOUNT_COUNT_V1,
        AdmissionRoleV1::FailureEvaluation => FAILURE_ADMISSION_ACCOUNT_COUNT_V1,
    };
    if accounts.len() != expected_count {
        return Err(ProductEvidenceError::AccountFrame.into());
    }
    require_distinct(accounts)?;
    let frame = AdmissionFrame::parse(accounts, request.role())?;
    frame.validate_privileges(program_id)?;
    let rent = authenticate_rent_system(frame.rent, frame.system)?;
    let registry = *frame.market.owner;
    if registry == *program_id
        || registry == system_program::ID
        || registry == bpf_loader_upgradeable::ID
    {
        return Err(ProductEvidenceError::Market.into());
    }

    let (market, market_identity_digest) =
        authenticate_market(frame.market, registry, request.expected_generation())?;
    let manifest_bytes = frame
        .manifest_raw
        .try_borrow_data()
        .map_err(|_| ProductEvidenceError::Borrow)?;
    authenticate_record(
        frame.manifest_raw,
        frame.manifest_staging,
        &rent,
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        market.identity().capability_manifest_id().to_bytes(),
        &manifest_bytes,
    )?;
    let manifest = CapabilityManifestV1::decode(&manifest_bytes)
        .map_err(|_| ProductEvidenceError::FinalizedRecord)?;
    let binding = {
        let bytes = frame
            .binding_raw
            .try_borrow_data()
            .map_err(|_| ProductEvidenceError::Borrow)?;
        authenticate_record(
            frame.binding_raw,
            frame.binding_staging,
            &rent,
            registry,
            PRODUCT_PAYOFF_BINDING_SCHEMA_ID_V1,
            request.binding_digest(),
            &bytes,
        )?;
        PayoffBindingV1::decode(&bytes).map_err(|_| ProductEvidenceError::FinalizedRecord)?
    };
    let instance = {
        let bytes = frame
            .instance_raw
            .try_borrow_data()
            .map_err(|_| ProductEvidenceError::Borrow)?;
        authenticate_record(
            frame.instance_raw,
            frame.instance_staging,
            &rent,
            registry,
            PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
            market.identity().product_instance_id().to_bytes(),
            &bytes,
        )?;
        InstanceV1::decode(&bytes).map_err(|_| ProductEvidenceError::FinalizedRecord)?
    };
    let (domain, domain_id) = {
        let bytes = frame
            .domain_raw
            .try_borrow_data()
            .map_err(|_| ProductEvidenceError::Borrow)?;
        let raw_digest = hash(&bytes).to_bytes();
        authenticate_record(
            frame.domain_raw,
            frame.domain_staging,
            &rent,
            registry,
            FINITE_RESULT_DOMAIN_SCHEMA_RELEASE_ID_V1,
            raw_digest,
            &bytes,
        )?;
        let value = FiniteResultDomainV1::decode(&bytes)
            .map_err(|_| ProductEvidenceError::FinalizedRecord)?;
        let semantic = hashv(&[FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, &[0], &bytes]).to_bytes();
        (value, semantic)
    };
    let payoff_bytes = frame
        .payoff_raw
        .try_borrow_data()
        .map_err(|_| ProductEvidenceError::Borrow)?;
    finish_authenticated_admission(
        program_id,
        &frame,
        &rent,
        request,
        registry,
        &market,
        market_identity_digest,
        manifest,
        &binding,
        &instance,
        domain_id,
        &domain,
        &payoff_bytes,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn finish_authenticated_admission(
    program_id: &Pubkey,
    frame: &AdmissionFrame<'_, '_>,
    rent: &Rent,
    request: PayoffAdmissionRequestV1,
    registry: Pubkey,
    market: &MarketRoot,
    market_identity_digest: [u8; 32],
    manifest: CapabilityManifestV1<'_>,
    binding: &PayoffBindingV1,
    instance: &InstanceV1,
    domain_id: [u8; 32],
    domain: &FiniteResultDomainV1,
    payoff_bytes: &[u8],
) -> ProgramResult {
    authenticate_record(
        frame.payoff_raw,
        frame.payoff_staging,
        rent,
        registry,
        PRODUCT_PAYOFF_SCHEMA_RELEASE_ID_V2,
        binding.payoff_record_digest(),
        payoff_bytes,
    )?;
    let payoff =
        ProductPayoffV2::decode(payoff_bytes).map_err(|_| ProductEvidenceError::FinalizedRecord)?;
    authenticate_release_and_deployment(
        frame.payoff_artifact_raw,
        frame.payoff_artifact_staging,
        frame.payoff_program,
        frame.payoff_programdata,
        rent,
        registry,
        binding.payoff_artifact_digest(),
        Pubkey::new_from_array(binding.payoff_program()),
        PRODUCT_PAYOFF_ADAPTER_RELEASE_ID_V2,
    )?;
    authenticate_release_and_deployment(
        frame.admission_artifact_raw,
        frame.admission_artifact_staging,
        frame.admission_program,
        frame.admission_programdata,
        rent,
        registry,
        binding.admission_artifact_digest(),
        *program_id,
        PRODUCT_PAYOFF_ADMISSION_RELEASE_ID_V1,
    )?;
    authenticate_release_and_deployment(
        frame.resolution_artifact_raw,
        frame.resolution_artifact_staging,
        frame.resolution_program,
        frame.resolution_programdata,
        rent,
        registry,
        binding.resolution_artifact_digest(),
        Pubkey::new_from_array(binding.resolution_program()),
        RESOLUTION_CONTROLLER_RELEASE_ID_V3,
    )?;

    let payoff_certificate = authenticate_payoff_certificate(
        frame.payoff_certificate,
        rent,
        request,
        *binding,
        registry,
    )?;
    let resolution_certificate =
        authenticate_resolution_certificate(frame.resolution_certificate, rent, request, *binding)?;
    admit_and_persist(
        program_id,
        frame,
        rent,
        request,
        AdmissionFactsV1 {
            admission_program: program_id.to_bytes(),
            registry_program: registry.to_bytes(),
            market_account: frame.market.key.to_bytes(),
            market_identity_digest,
            market,
            manifest_digest: market.identity().capability_manifest_id().to_bytes(),
            manifest,
            binding_digest: request.binding_digest(),
            binding,
            product_instance_id: market.identity().product_instance_id().to_bytes(),
            product_instance: instance,
            result_domain_id: domain_id,
            result_domain: domain,
            payoff: &payoff,
            payoff_certificate_account: frame
                .payoff_certificate
                .map_or([0; 32], |account| account.key.to_bytes()),
            payoff_certificate_digest: if payoff_certificate.is_some() {
                request.payoff_certificate_digest()
            } else {
                [0; 32]
            },
            payoff_certificate: payoff_certificate.as_ref(),
            resolution_certificate_account: frame
                .resolution_certificate
                .map_or([0; 32], |account| account.key.to_bytes()),
            resolution_certificate_digest: if resolution_certificate.is_some() {
                request.resolution_certificate_digest()
            } else {
                [0; 32]
            },
            resolution_certificate: resolution_certificate.as_ref(),
        },
    )
}

#[inline(never)]
fn admit_and_persist(
    program_id: &Pubkey,
    frame: &AdmissionFrame<'_, '_>,
    rent: &Rent,
    request: PayoffAdmissionRequestV1,
    facts: AdmissionFactsV1<'_>,
) -> ProgramResult {
    let market_account = frame.market.key.to_bytes();
    let receipt = admit(request, facts).map_err(|_| ProductEvidenceError::Admission)?;
    ensure_admission_receipt(
        program_id,
        frame.payer,
        frame.receipt,
        frame.system,
        rent,
        request,
        market_account,
        &receipt.to_bytes(),
    )?;
    Ok(())
}

struct AdmissionFrame<'a, 'info> {
    payer: &'a AccountInfo<'info>,
    receipt: &'a AccountInfo<'info>,
    payoff_certificate: Option<&'a AccountInfo<'info>>,
    resolution_certificate: Option<&'a AccountInfo<'info>>,
    market: &'a AccountInfo<'info>,
    manifest_raw: &'a AccountInfo<'info>,
    manifest_staging: &'a AccountInfo<'info>,
    binding_raw: &'a AccountInfo<'info>,
    binding_staging: &'a AccountInfo<'info>,
    instance_raw: &'a AccountInfo<'info>,
    instance_staging: &'a AccountInfo<'info>,
    domain_raw: &'a AccountInfo<'info>,
    domain_staging: &'a AccountInfo<'info>,
    payoff_raw: &'a AccountInfo<'info>,
    payoff_staging: &'a AccountInfo<'info>,
    payoff_artifact_raw: &'a AccountInfo<'info>,
    payoff_artifact_staging: &'a AccountInfo<'info>,
    payoff_program: &'a AccountInfo<'info>,
    payoff_programdata: &'a AccountInfo<'info>,
    admission_artifact_raw: &'a AccountInfo<'info>,
    admission_artifact_staging: &'a AccountInfo<'info>,
    admission_program: &'a AccountInfo<'info>,
    admission_programdata: &'a AccountInfo<'info>,
    resolution_artifact_raw: &'a AccountInfo<'info>,
    resolution_artifact_staging: &'a AccountInfo<'info>,
    resolution_program: &'a AccountInfo<'info>,
    resolution_programdata: &'a AccountInfo<'info>,
    rent: &'a AccountInfo<'info>,
    system: &'a AccountInfo<'info>,
}

impl<'a, 'info> AdmissionFrame<'a, 'info> {
    fn parse(
        accounts: &'a [AccountInfo<'info>],
        role: AdmissionRoleV1,
    ) -> Result<Self, ProgramError> {
        let mut iter = accounts.iter();
        let payer = next(&mut iter)?;
        let receipt = next(&mut iter)?;
        let payoff_certificate = if role != AdmissionRoleV1::FailureEvaluation {
            Some(next(&mut iter)?)
        } else {
            None
        };
        let resolution_certificate = if role != AdmissionRoleV1::Liability {
            Some(next(&mut iter)?)
        } else {
            None
        };
        Ok(Self {
            payer,
            receipt,
            payoff_certificate,
            resolution_certificate,
            market: next(&mut iter)?,
            manifest_raw: next(&mut iter)?,
            manifest_staging: next(&mut iter)?,
            binding_raw: next(&mut iter)?,
            binding_staging: next(&mut iter)?,
            instance_raw: next(&mut iter)?,
            instance_staging: next(&mut iter)?,
            domain_raw: next(&mut iter)?,
            domain_staging: next(&mut iter)?,
            payoff_raw: next(&mut iter)?,
            payoff_staging: next(&mut iter)?,
            payoff_artifact_raw: next(&mut iter)?,
            payoff_artifact_staging: next(&mut iter)?,
            payoff_program: next(&mut iter)?,
            payoff_programdata: next(&mut iter)?,
            admission_artifact_raw: next(&mut iter)?,
            admission_artifact_staging: next(&mut iter)?,
            admission_program: next(&mut iter)?,
            admission_programdata: next(&mut iter)?,
            resolution_artifact_raw: next(&mut iter)?,
            resolution_artifact_staging: next(&mut iter)?,
            resolution_program: next(&mut iter)?,
            resolution_programdata: next(&mut iter)?,
            rent: next(&mut iter)?,
            system: next(&mut iter)?,
        })
    }

    fn validate_privileges(&self, program_id: &Pubkey) -> ProgramResult {
        if !self.payer.is_signer
            || !self.payer.is_writable
            || self.payer.executable
            || self.receipt.is_signer
            || !self.receipt.is_writable
            || self.receipt.executable
            || self.admission_program.key != program_id
            || !self.admission_program.executable
            || !self.payoff_program.executable
            || !self.resolution_program.executable
            || self.rent.is_signer
            || self.rent.is_writable
            || self.rent.executable
            || self.system.is_signer
            || self.system.is_writable
            || !self.system.executable
        {
            return Err(ProductEvidenceError::AccountFrame.into());
        }
        for account in [
            self.payoff_certificate,
            self.resolution_certificate,
            Some(self.market),
            Some(self.manifest_raw),
            Some(self.manifest_staging),
            Some(self.binding_raw),
            Some(self.binding_staging),
            Some(self.instance_raw),
            Some(self.instance_staging),
            Some(self.domain_raw),
            Some(self.domain_staging),
            Some(self.payoff_raw),
            Some(self.payoff_staging),
            Some(self.payoff_artifact_raw),
            Some(self.payoff_artifact_staging),
            Some(self.payoff_programdata),
            Some(self.admission_artifact_raw),
            Some(self.admission_artifact_staging),
            Some(self.admission_programdata),
            Some(self.resolution_artifact_raw),
            Some(self.resolution_artifact_staging),
            Some(self.resolution_programdata),
        ]
        .into_iter()
        .flatten()
        {
            if account.is_signer || account.is_writable {
                return Err(ProductEvidenceError::AccountFrame.into());
            }
        }
        Ok(())
    }
}

fn authenticate_market(
    account: &AccountInfo<'_>,
    registry: Pubkey,
    expected_generation: u64,
) -> Result<(MarketRoot, [u8; 32]), ProgramError> {
    if account.owner != &registry || account.executable || account.is_writable {
        return Err(ProductEvidenceError::Market.into());
    }
    let bytes = account
        .try_borrow_data()
        .map_err(|_| ProductEvidenceError::Borrow)?;
    let root = decode_market_root(&bytes)?;
    if root.identity().generation() != expected_generation {
        return Err(ProductEvidenceError::Market.into());
    }
    let digest = hash(&root.identity().to_bytes()).to_bytes();
    let expected = Pubkey::find_program_address(&[MARKET_PDA_DOMAIN_V1, &digest], &registry).0;
    if account.key != &expected {
        return Err(ProductEvidenceError::Market.into());
    }
    Ok((root, digest))
}

fn decode_market_root(bytes: &[u8]) -> Result<MarketRoot, ProgramError> {
    let count = decode_market_outcome_count(bytes).map_err(|_| ProductEvidenceError::Market)?;
    macro_rules! decode {
        ($width:literal) => {
            CategoricalMarketV1::<$width>::decode(bytes)
                .map(|value| value.root())
                .map_err(|_| ProductEvidenceError::Market.into())
        };
    }
    match count {
        2 => decode!(2),
        3 => decode!(3),
        4 => decode!(4),
        5 => decode!(5),
        6 => decode!(6),
        7 => decode!(7),
        8 => decode!(8),
        9 => decode!(9),
        10 => decode!(10),
        11 => decode!(11),
        12 => decode!(12),
        13 => decode!(13),
        14 => decode!(14),
        15 => decode!(15),
        16 => decode!(16),
        _ => Err(ProductEvidenceError::Market.into()),
    }
}

fn make_payoff_certificate(
    registry: Pubkey,
    request: PayoffRequestV2,
    product: ProductPayoffV2,
) -> Result<PayoffCertificateV2, ProgramError> {
    match request.kind() {
        CertificateKindV2::Evaluation => {
            let payout = product
                .evaluate_rational(request.result_numerator(), request.result_denominator())
                .map_err(|_| ProductEvidenceError::Evaluation)?;
            PayoffCertificateV2::evaluation(
                registry.to_bytes(),
                request.product_record_digest(),
                request.artifact_release_digest(),
                product.product_id(),
                product.domain_id(),
                product.coordinate_unit_id(),
                product.payout_scale(),
                request.result_numerator(),
                request.result_denominator(),
                payout,
                product.liability_bound(),
            )
            .map_err(|_| ProductEvidenceError::Evaluation.into())
        }
        CertificateKindV2::Liability => PayoffCertificateV2::liability(
            registry.to_bytes(),
            request.product_record_digest(),
            request.artifact_release_digest(),
            product.product_id(),
            product.domain_id(),
            product.coordinate_unit_id(),
            product.payout_scale(),
            request.available(),
            product.liability_bound(),
        )
        .map_err(|_| ProductEvidenceError::Evaluation.into()),
    }
}

fn authenticate_payoff_certificate(
    account: Option<&AccountInfo<'_>>,
    rent: &Rent,
    request: PayoffAdmissionRequestV1,
    binding: PayoffBindingV1,
    registry: Pubkey,
) -> Result<Option<PayoffCertificateV2>, ProgramError> {
    let Some(account) = account else {
        return Ok(None);
    };
    if account.owner.to_bytes() != binding.payoff_program()
        || account.executable
        || !rent.is_exempt(account.lamports(), account.data_len())
    {
        return Err(ProductEvidenceError::PayoffCertificate.into());
    }
    let bytes = account
        .try_borrow_data()
        .map_err(|_| ProductEvidenceError::Borrow)?;
    if bytes.len() != PAYOFF_CERTIFICATE_BYTES_V2
        || hash(&bytes).to_bytes() != request.payoff_certificate_digest()
    {
        return Err(ProductEvidenceError::PayoffCertificate.into());
    }
    let certificate =
        PayoffCertificateV2::decode(&bytes).map_err(|_| ProductEvidenceError::PayoffCertificate)?;
    if certificate.registry_program() != registry.to_bytes()
        || certificate.product_record_digest() != binding.payoff_record_digest()
        || certificate.artifact_release_digest() != binding.payoff_artifact_digest()
    {
        return Err(ProductEvidenceError::PayoffCertificate.into());
    }
    let expected = payoff_certificate_address(
        Pubkey::new_from_array(binding.payoff_program()),
        registry,
        certificate,
    );
    if account.key != &expected {
        return Err(ProductEvidenceError::PayoffCertificate.into());
    }
    Ok(Some(certificate))
}

fn authenticate_resolution_certificate(
    account: Option<&AccountInfo<'_>>,
    rent: &Rent,
    request: PayoffAdmissionRequestV1,
    binding: PayoffBindingV1,
) -> Result<Option<ResolutionCertificateV1>, ProgramError> {
    let Some(account) = account else {
        return Ok(None);
    };
    if account.owner.to_bytes() != binding.resolution_program()
        || account.executable
        || account.data_len() != RESOLUTION_CERTIFICATE_BYTES
        || !rent.is_exempt(account.lamports(), account.data_len())
    {
        return Err(ProductEvidenceError::ResolutionCertificate.into());
    }
    let bytes = account
        .try_borrow_data()
        .map_err(|_| ProductEvidenceError::Borrow)?;
    if hash(&bytes).to_bytes() != request.resolution_certificate_digest() {
        return Err(ProductEvidenceError::ResolutionCertificate.into());
    }
    let certificate = ResolutionCertificateV1::decode(&bytes)
        .map_err(|_| ProductEvidenceError::ResolutionCertificate)?;
    if certificate.receipt_account != account.key.to_bytes() {
        return Err(ProductEvidenceError::ResolutionCertificate.into());
    }
    Ok(Some(certificate))
}

#[allow(clippy::too_many_arguments)]
fn authenticate_release_and_deployment(
    artifact_raw: &AccountInfo<'_>,
    artifact_staging: &AccountInfo<'_>,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    rent: &Rent,
    registry: Pubkey,
    artifact_digest: [u8; 32],
    expected_program: Pubkey,
    semantic_release: [u8; 32],
) -> ProgramResult {
    let release = {
        let bytes = artifact_raw
            .try_borrow_data()
            .map_err(|_| ProductEvidenceError::Borrow)?;
        authenticate_record(
            artifact_raw,
            artifact_staging,
            rent,
            registry,
            ARTIFACT_RELEASE_SCHEMA_ID_V1,
            artifact_digest,
            &bytes,
        )?;
        ArtifactReleaseV1::decode(&bytes).map_err(|_| ProductEvidenceError::ArtifactRelease)?
    };
    if release.program().to_bytes() != expected_program.to_bytes()
        || release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.semantic_release_id().to_bytes() != semantic_release
        || program.key != &expected_program
    {
        return Err(ProductEvidenceError::ArtifactRelease.into());
    }
    authenticate_deployment(program, programdata, release)
}

fn authenticate_deployment(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    release: ArtifactReleaseV1,
) -> ProgramResult {
    if program.owner != &bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.owner != &bpf_loader_upgradeable::ID
        || programdata.executable
        || programdata.key.to_bytes() != release.programdata()
    {
        return Err(ProductEvidenceError::Deployment.into());
    }
    let expected =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if programdata.key != &expected {
        return Err(ProductEvidenceError::Deployment.into());
    }
    let program_bytes = program
        .try_borrow_data()
        .map_err(|_| ProductEvidenceError::Borrow)?;
    let program_view =
        ProgramV3View::parse(&program_bytes).map_err(|_| ProductEvidenceError::Deployment)?;
    if program_view.programdata() != programdata.key.to_bytes() {
        return Err(ProductEvidenceError::Deployment.into());
    }
    let programdata_bytes = programdata
        .try_borrow_data()
        .map_err(|_| ProductEvidenceError::Borrow)?;
    let view = ProgramDataV3View::parse(&programdata_bytes)
        .map_err(|_| ProductEvidenceError::Deployment)?;
    let observation = DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        view.deployment_slot(),
        hash(view.elf()).to_bytes(),
        view.upgrade_authority(),
    )
    .map_err(|_| ProductEvidenceError::Deployment)?;
    release
        .authenticate_deployment(observation)
        .map_err(|_| ProductEvidenceError::Deployment.into())
}

fn authenticate_record(
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    rent: &Rent,
    registry: Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
    bytes: &[u8],
) -> ProgramResult {
    if raw.owner != &registry
        || raw.executable
        || hash(bytes).to_bytes() != digest
        || !rent.is_exempt(raw.lamports(), bytes.len())
    {
        return Err(ProductEvidenceError::FinalizedRecord.into());
    }
    let expected_raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
    let expected_staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0;
    if raw.key != &expected_raw
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.executable
        || staging.lamports() != 0
        || staging.data_len() != 0
    {
        return Err(ProductEvidenceError::FinalizedRecord.into());
    }
    Ok(())
}

fn payoff_query_digest(request: PayoffRequestV2) -> [u8; 32] {
    hashv(&[
        &request.result_numerator().to_le_bytes(),
        &request.result_denominator().to_le_bytes(),
        &request.available().to_le_bytes(),
    ])
    .to_bytes()
}

fn payoff_certificate_address(
    program: Pubkey,
    registry: Pubkey,
    certificate: PayoffCertificateV2,
) -> Pubkey {
    let role = [match certificate.kind() {
        CertificateKindV2::Evaluation => 0,
        CertificateKindV2::Liability => 1,
    }];
    let query = hashv(&[
        &certificate.result_numerator().to_le_bytes(),
        &certificate.result_denominator().to_le_bytes(),
        &certificate.available().to_le_bytes(),
    ])
    .to_bytes();
    Pubkey::find_program_address(
        &[
            PAYOFF_CERTIFICATE_PDA_DOMAIN_V2,
            registry.as_ref(),
            &certificate.product_record_digest(),
            &certificate.artifact_release_digest(),
            &role,
            &query,
        ],
        &program,
    )
    .0
}

#[allow(clippy::too_many_arguments)]
fn ensure_payoff_certificate<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    certificate: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    rent: &Rent,
    registry: Pubkey,
    request: PayoffRequestV2,
    expected: &[u8; PAYOFF_CERTIFICATE_BYTES_V2],
) -> ProgramResult {
    let role = [match request.kind() {
        CertificateKindV2::Evaluation => 0,
        CertificateKindV2::Liability => 1,
    }];
    let query = payoff_query_digest(request);
    let product = request.product_record_digest();
    let artifact = request.artifact_release_digest();
    let (address, bump) = Pubkey::find_program_address(
        &[
            PAYOFF_CERTIFICATE_PDA_DOMAIN_V2,
            registry.as_ref(),
            &product,
            &artifact,
            &role,
            &query,
        ],
        program_id,
    );
    if certificate.key != &address {
        return Err(ProductEvidenceError::PayoffCertificate.into());
    }
    let created = ensure_output_account(
        program_id,
        payer,
        certificate,
        system,
        rent,
        PAYOFF_CERTIFICATE_BYTES_V2,
        &[
            PAYOFF_CERTIFICATE_PDA_DOMAIN_V2,
            registry.as_ref(),
            &product,
            &artifact,
            &role,
            &query,
        ],
        bump,
        expected,
        ProductEvidenceError::PayoffCertificate,
    )?;
    if created {
        let mut data = certificate
            .try_borrow_mut_data()
            .map_err(|_| ProductEvidenceError::Borrow)?;
        data.copy_from_slice(expected);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn ensure_admission_receipt<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    receipt: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    rent: &Rent,
    request: PayoffAdmissionRequestV1,
    market: [u8; 32],
    expected: &[u8; PAYOFF_ADMISSION_RECEIPT_BYTES_V1],
) -> ProgramResult {
    let generation = request.expected_generation().to_le_bytes();
    let role = [request.role().byte()];
    let binding = request.binding_digest();
    let payoff = request.payoff_certificate_digest();
    let resolution = request.resolution_certificate_digest();
    let seeds: [&[u8]; 7] = [
        PAYOFF_ADMISSION_RECEIPT_PDA_DOMAIN_V1,
        &market,
        &generation,
        &role,
        &binding,
        &payoff,
        &resolution,
    ];
    let (address, bump) = Pubkey::find_program_address(&seeds, program_id);
    if receipt.key != &address {
        return Err(ProductEvidenceError::Receipt.into());
    }
    let created = ensure_output_account(
        program_id,
        payer,
        receipt,
        system,
        rent,
        PAYOFF_ADMISSION_RECEIPT_BYTES_V1,
        &seeds,
        bump,
        expected,
        ProductEvidenceError::Receipt,
    )?;
    if created {
        let mut data = receipt
            .try_borrow_mut_data()
            .map_err(|_| ProductEvidenceError::Borrow)?;
        data.copy_from_slice(expected);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn ensure_output_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    output: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    rent: &Rent,
    width: usize,
    seeds: &[&[u8]],
    bump: u8,
    expected: &[u8],
    error: ProductEvidenceError,
) -> Result<bool, ProgramError> {
    if output.owner == program_id {
        if output.executable
            || output.data_len() != width
            || !rent.is_exempt(output.lamports(), width)
        {
            return Err(error.into());
        }
        let bytes = output
            .try_borrow_data()
            .map_err(|_| ProductEvidenceError::Borrow)?;
        if bytes.as_ref() != expected {
            return Err(error.into());
        }
        return Ok(false);
    }
    if output.owner != &system_program::ID
        || output.executable
        || output.lamports() != 0
        || output.data_len() != 0
    {
        return Err(error.into());
    }
    let lamports = rent.minimum_balance(width);
    let space = u64::try_from(width).map_err(|_| ProductEvidenceError::Arithmetic)?;
    let instruction = create_account(payer.key, output.key, lamports, space, program_id);
    let bump_seed = [bump];
    let mut signer: [&[u8]; 8] = [&[]; 8];
    if seeds.len() >= signer.len() {
        return Err(ProductEvidenceError::Arithmetic.into());
    }
    let mut index = 0_usize;
    while index < seeds.len() {
        *signer
            .get_mut(index)
            .ok_or(ProductEvidenceError::Arithmetic)? =
            *seeds.get(index).ok_or(ProductEvidenceError::Arithmetic)?;
        index = index
            .checked_add(1)
            .ok_or(ProductEvidenceError::Arithmetic)?;
    }
    *signer
        .get_mut(index)
        .ok_or(ProductEvidenceError::Arithmetic)? = &bump_seed;
    let signer_slice = signer
        .get(..=index)
        .ok_or(ProductEvidenceError::Arithmetic)?;
    invoke_signed(
        &instruction,
        &[payer.clone(), output.clone(), system.clone()],
        &[signer_slice],
    )
    .map_err(|_| ProductEvidenceError::CreateCpi)?;
    if output.owner != program_id || output.data_len() != width || output.lamports() != lamports {
        return Err(ProductEvidenceError::CreateCpi.into());
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn validate_evaluator_privileges<'info>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    certificate: &AccountInfo<'info>,
    product_raw: &AccountInfo<'info>,
    product_staging: &AccountInfo<'info>,
    artifact_raw: &AccountInfo<'info>,
    artifact_staging: &AccountInfo<'info>,
    program: &AccountInfo<'info>,
    programdata: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
) -> ProgramResult {
    if !payer.is_signer
        || !payer.is_writable
        || payer.executable
        || certificate.is_signer
        || !certificate.is_writable
        || certificate.executable
        || program.key != program_id
        || !program.executable
        || !system.executable
        || system.is_writable
        || system.is_signer
        || rent.is_writable
        || rent.is_signer
        || rent.executable
    {
        return Err(ProductEvidenceError::AccountFrame.into());
    }
    for account in [
        product_raw,
        product_staging,
        artifact_raw,
        artifact_staging,
        programdata,
    ] {
        if account.is_signer || account.is_writable {
            return Err(ProductEvidenceError::AccountFrame.into());
        }
    }
    Ok(())
}

fn authenticate_rent_system(
    rent: &AccountInfo<'_>,
    system: &AccountInfo<'_>,
) -> Result<Rent, ProgramError> {
    if rent.key != &sysvar::rent::ID
        || rent.owner != &sysvar::ID
        || system.key != &system_program::ID
        || system.owner != &native_loader::ID
    {
        return Err(ProductEvidenceError::Sysvar.into());
    }
    Rent::from_account_info(rent).map_err(|_| ProductEvidenceError::Sysvar.into())
}

fn require_distinct(accounts: &[AccountInfo<'_>]) -> ProgramResult {
    for (index, left) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(index.saturating_add(1))
            .any(|right| left.key == right.key)
        {
            return Err(ProductEvidenceError::AccountFrame.into());
        }
    }
    Ok(())
}

fn next<'a, 'info>(
    iter: &mut core::slice::Iter<'a, AccountInfo<'info>>,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    next_account_info(iter).map_err(|_| ProductEvidenceError::AccountFrame.into())
}
