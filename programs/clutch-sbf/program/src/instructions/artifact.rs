//! Permissionless, typed upload and sealing of immutable protocol artifacts.
//!
//! Terms are 1,656 bytes, so they cannot travel in one Solana transaction.
//! This family creates an uploader-keyed staging PDA at its exact final body
//! size, accepts only the next 192-byte chunk, and seals only after the whole
//! body passes its pre-existing hostile-byte codec and semantic digest check.
//! The final account contains the exact historical raw Policy, PriceGrid, or
//! Terms bytes—or, only in the explicit non-production laboratory, one frozen
//! Product/Series codec body—at its content-derived PDA. It never contains a
//! generic blob wrapper and consumers never read the staging account.
//!
//! The stage's funder is its sole writer and sealer.  It may abort at any
//! time.  After the frozen expiry slot any signer may reap the abandoned stage,
//! but every lamport still returns to the funder stored in the stage header.
//! Hoard principal, collateral, protocol fees, and the reaper are never rent
//! sources or refund destinations.

use crate::accounts::{expect_pda, require, require_count, require_signer, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
#[cfg(target_os = "solana")]
use clutch_product_series::{
    CompiledProductSeriesBundleV2, CompiledProductSeriesBundleV3, CompiledProductSeriesBundleV4,
    CompiledProductSeriesBundleV5, CompiledProductSeriesBundleV6,
    FixedCodec as RegistryFixedCodec, MarketFamilyCapabilityPolicyV1,
    RegistryCapabilityProfileV2,
    RegistryCapabilityProfileV3, RegistryCapabilityProfileV4, RegistryProgramReleaseV1,
    RegistryProgramReleaseV2, SeriesAttachmentPlanV2, SeriesAttachmentPlanV3,
    SeriesAttachmentPlanV4, SeriesAttachmentPlanV5, SeriesFundingQuoteV2,
    SeriesFundingQuoteV3, SeriesFundingQuoteV4, SeriesFundingQuoteV5,
    COMPILED_PRODUCT_SERIES_BUNDLE_V2_DOMAIN, COMPILED_PRODUCT_SERIES_BUNDLE_V3_DOMAIN,
    COMPILED_PRODUCT_SERIES_BUNDLE_V4_DOMAIN, COMPILED_PRODUCT_SERIES_BUNDLE_V5_DOMAIN,
    COMPILED_PRODUCT_SERIES_BUNDLE_V6_DOMAIN,
    MARKET_FAMILY_CAPABILITY_POLICY_DOMAIN_V1,
    REGISTRY_CAPABILITY_PROFILE_V2_DOMAIN, REGISTRY_CAPABILITY_PROFILE_V3_DOMAIN,
    REGISTRY_CAPABILITY_PROFILE_V4_DOMAIN, REGISTRY_PROGRAM_RELEASE_V1_DOMAIN,
    REGISTRY_PROGRAM_RELEASE_V2_DOMAIN, SERIES_ATTACHMENT_PLAN_V2_DOMAIN,
    SERIES_ATTACHMENT_PLAN_V3_DOMAIN, SERIES_ATTACHMENT_PLAN_V4_DOMAIN,
    SERIES_ATTACHMENT_PLAN_V5_DOMAIN, SERIES_FUNDING_QUOTE_V2_DOMAIN,
    SERIES_FUNDING_QUOTE_V3_DOMAIN, SERIES_FUNDING_QUOTE_V4_DOMAIN,
    SERIES_FUNDING_QUOTE_V5_DOMAIN,
};
use clutch_solana_layout::artifact::{
    self, ArtifactBinding, ArtifactKind, ArtifactStageHeader, ARTIFACT_CHUNK_BYTES,
};
#[cfg(target_os = "solana")]
use clutch_solana_layout::{CodecError, TermsAccount, HASH_BYTES};
use clutch_solana_layout::{Hash32, Intent};
#[cfg(target_os = "solana")]
use clutch_structured_claim_runtime_contract::{
    WrapperRecipeHashV1, WrapperRecipeSetV1,
};
use clutch_solana_reference::{Action, Request};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

#[cfg(all(target_os = "solana", feature = "non-production-product-series-lab"))]
use clutch_product_series::{
    CompiledProductSeriesBundleV1, EvidenceOnlyRecoveryPolicyV1, FixedCodec,
    MarketGenesisProfileV2, NativeClaimBasisV1, PriceMeasurePolicyV1, ProductTemplateV4,
    SeriesAttachmentPlanV1, SeriesFundingQuoteV1, SeriesFundingTermsV2, SeriesPlanV5,
    COMPILED_PRODUCT_SERIES_BUNDLE_V1_DOMAIN, MARKET_GENESIS_PROFILE_V2_DOMAIN,
    NATIVE_CLAIM_BASIS_DOMAIN, PRICE_MEASURE_POLICY_DOMAIN, PRODUCT_TEMPLATE_DOMAIN,
    RECOVERY_POLICY_DOMAIN, SERIES_ATTACHMENT_PLAN_DOMAIN, SERIES_FUNDING_QUOTE_DOMAIN,
    SERIES_FUNDING_TERMS_V2_DOMAIN, SERIES_PLAN_V5_DOMAIN,
};

use super::genesis::{
    read_rent, require_system_program, RentParameters, MAX_PERMITTED_DATA_INCREASE,
    SYSTEM_PROGRAM_ID,
};

/// Shortest stage lifetime admitted at creation.
pub const MIN_UPLOAD_LIFETIME_SLOTS: u64 = 8;
/// Longest stage lifetime admitted at creation.
///
/// This is a resource bound, not a wall-clock promise: Solana slots have no
/// exact duration guarantee.  An uploader that needs longer may abort and
/// restart at the same uploader-keyed PDA after the old stage is closed.
pub const MAX_UPLOAD_LIFETIME_SLOTS: u64 = 432_000;

// `SystemInstruction` is bincode-encoded.  These are the frozen enum variant
// indices and payload widths for Assign, Transfer, and Allocate in
// `solana-system-interface`.  Artifact creation deliberately uses the three
// instructions rather than CreateAccount: a third party can transfer lamports
// to any predictable PDA before it exists, and CreateAccount rejects that
// otherwise harmless prefund.
const SYSTEM_IX_ASSIGN: u32 = 1;
const SYSTEM_IX_TRANSFER: u32 = 2;
const SYSTEM_IX_ALLOCATE: u32 = 8;
const SYSTEM_TRANSFER_DATA_LEN: usize = 4 + 8;
const SYSTEM_ALLOCATE_DATA_LEN: usize = 4 + 8;
const SYSTEM_ASSIGN_DATA_LEN: usize = 4 + 32;

const _: () = {
    assert!(MIN_UPLOAD_LIFETIME_SLOTS > 0);
    assert!(MAX_UPLOAD_LIFETIME_SLOTS >= MIN_UPLOAD_LIFETIME_SLOTS);
};

/// Clock sysvar address, `SysvarC1ock11111111111111111111111111111111`.
pub const CLOCK_SYSVAR_ID: Pubkey = Pubkey::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);
/// Bincode length of the Solana Clock sysvar.
pub const CLOCK_SYSVAR_LEN: usize = 8 + 8 + 8 + 8 + 8;

/// Begin account count: funder, stage, System program, Rent, Clock.
pub const BEGIN_ACCOUNT_COUNT: usize = 5;
/// Write account count: funder, stage, Clock.
pub const WRITE_ACCOUNT_COUNT: usize = 3;
/// Seal account count: funder, stage, final, System program, Rent, Clock.
pub const SEAL_ACCOUNT_COUNT: usize = 6;
/// Abort account count: caller, stage, recorded funder, Clock.
pub const ABORT_ACCOUNT_COUNT: usize = 4;

/// Signer that funds and owns an upload lifecycle.
pub const IX_FUNDER: usize = 0;
/// Uploader-keyed staging PDA.
pub const IX_STAGE: usize = 1;
/// System program in Begin and Seal.
pub const IX_BEGIN_SYSTEM: usize = 2;
/// Rent sysvar in Begin and Seal.
pub const IX_BEGIN_RENT: usize = 3;
/// Clock sysvar in Begin.
pub const IX_BEGIN_CLOCK: usize = 4;
/// Clock sysvar in Write.
pub const IX_WRITE_CLOCK: usize = 2;
/// Final content-derived artifact PDA in Seal.
pub const IX_FINAL: usize = 2;
/// System program in Seal.
pub const IX_SEAL_SYSTEM: usize = 3;
/// Rent sysvar in Seal.
pub const IX_SEAL_RENT: usize = 4;
/// Clock sysvar in Seal.
pub const IX_SEAL_CLOCK: usize = 5;
/// Any signer may call Abort after expiry; before expiry it must be the funder.
pub const IX_ABORT_CALLER: usize = 0;
/// Recorded funder and sole rent-refund destination in Abort.
pub const IX_ABORT_REFUND: usize = 2;
/// Clock sysvar in Abort.
pub const IX_ABORT_CLOCK: usize = 3;

/// Decode the current slot from an authenticated Clock sysvar account.
pub fn read_clock_slot(account: &AccountInfo) -> Outcome<u64> {
    require(
        *account.key == CLOCK_SYSVAR_ID,
        ClutchError::WrongClockSysvar,
    )?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        account.data_len() == CLOCK_SYSVAR_LEN,
        ClutchError::WrongClockSysvar,
    )?;
    let data = account.data.borrow();
    let mut slot = [0; 8];
    slot.copy_from_slice(&data[..8]);
    Ok(u64::from_le_bytes(slot))
}

fn require_zero_sequence(sequence: u64) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)
}

fn require_artifact_creation_target(target: &AccountInfo) -> Outcome<()> {
    require(target.is_writable, ClutchError::NotWritable)?;
    require(!target.executable, ClutchError::ExecutableAccount)?;
    require(
        target.data_len() == 0 && *target.owner == SYSTEM_PROGRAM_ID,
        ClutchError::AlreadyInitialized,
    )
}

fn system_transfer_data(lamports: u64) -> [u8; SYSTEM_TRANSFER_DATA_LEN] {
    let mut data = [0_u8; SYSTEM_TRANSFER_DATA_LEN];
    data[..4].copy_from_slice(&SYSTEM_IX_TRANSFER.to_le_bytes());
    data[4..].copy_from_slice(&lamports.to_le_bytes());
    data
}

fn system_allocate_data(space: usize) -> [u8; SYSTEM_ALLOCATE_DATA_LEN] {
    let mut data = [0_u8; SYSTEM_ALLOCATE_DATA_LEN];
    data[..4].copy_from_slice(&SYSTEM_IX_ALLOCATE.to_le_bytes());
    data[4..].copy_from_slice(&(space as u64).to_le_bytes());
    data
}

fn system_assign_data(owner: &Pubkey) -> [u8; SYSTEM_ASSIGN_DATA_LEN] {
    let mut data = [0_u8; SYSTEM_ASSIGN_DATA_LEN];
    data[..4].copy_from_slice(&SYSTEM_IX_ASSIGN.to_le_bytes());
    data[4..].copy_from_slice(&owner.to_bytes());
    data
}

/// Allocate and assign an exact artifact PDA even if someone prefunded it.
///
/// Any lamports already present are an unsolicited donation, never identity,
/// authority, a fee credit, or a refund claim.  The payer supplies exactly the
/// rent shortfall.  A persistent final retains any excess; a transient stage
/// follows its one frozen close destination, so all of its lamports eventually
/// go to the recorded funder on seal, abort, or reap.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn create_artifact_pda<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_artifact_creation_target(target)?;
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require_system_program(system_program)?;
    require(
        space <= MAX_PERMITTED_DATA_INCREASE,
        ClutchError::AccountCreationFailed,
    )?;

    let initial_lamports = target.lamports();
    let minimum_balance = rent.minimum_balance(space)?;
    let shortfall = minimum_balance.saturating_sub(initial_lamports);
    let funded_balance = initial_lamports
        .checked_add(shortfall)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;

    if shortfall != 0 {
        let transfer = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &system_transfer_data(shortfall),
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
            target.lamports() == funded_balance
                && target.data_len() == 0
                && *target.owner == SYSTEM_PROGRAM_ID,
            ClutchError::AccountCreationFailed,
        )?;
    }

    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &system_allocate_data(space),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.lamports() == funded_balance
            && target.data_len() == space
            && *target.owner == SYSTEM_PROGRAM_ID,
        ClutchError::AccountCreationFailed,
    )?;

    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &system_assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.lamports() == funded_balance
            && target.data_len() == space
            && target.owner == program_id,
        ClutchError::AccountCreationFailed,
    )
}

fn require_stage_metadata(program_id: &Pubkey, stage: &AccountInfo) -> Outcome<()> {
    require(stage.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(stage.is_writable, ClutchError::NotWritable)?;
    require(!stage.executable, ClutchError::ExecutableAccount)
}

fn require_live(header: ArtifactStageHeader, slot: u64) -> Outcome<()> {
    require(slot <= header.expires_slot, ClutchError::ArtifactExpired)
}

fn require_funder(header: ArtifactStageHeader, funder: &AccountInfo) -> Outcome<()> {
    require_signer(funder)?;
    require(
        funder.key.to_bytes() == header.funder,
        ClutchError::UnauthorizedActor,
    )
}

fn require_stage_pda(
    program_id: &Pubkey,
    stage: &AccountInfo,
    header: ArtifactStageHeader,
) -> Outcome<()> {
    let kind = [header.binding.kind.byte()];
    expect_pda(
        stage.key,
        seeds::artifact_stage_pda(
            program_id,
            &header.funder,
            &kind,
            &header.binding.context.bytes(),
            &header.binding.digest.bytes(),
        ),
        Some(header.stored_bump),
    )
}

fn begin(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    binding: ArtifactBinding,
    expires_slot: u64,
) -> Outcome<()> {
    require_count(accounts, BEGIN_ACCOUNT_COUNT)?;
    require_zero_sequence(sequence)?;
    require_signer(&accounts[IX_FUNDER])?;
    require(accounts[IX_FUNDER].is_writable, ClutchError::NotWritable)?;
    require(
        accounts[IX_FUNDER].key != accounts[IX_STAGE].key,
        ClutchError::AccountAlias,
    )?;
    binding.validate_for_registration()?;
    require_system_program(&accounts[IX_BEGIN_SYSTEM])?;
    let rent = read_rent(&accounts[IX_BEGIN_RENT])?;
    let current_slot = read_clock_slot(&accounts[IX_BEGIN_CLOCK])?;
    let lifetime = expires_slot
        .checked_sub(current_slot)
        .ok_or(Refusal::Adapter(ClutchError::InvalidArtifactExpiry))?;
    require(
        (MIN_UPLOAD_LIFETIME_SLOTS..=MAX_UPLOAD_LIFETIME_SLOTS).contains(&lifetime),
        ClutchError::InvalidArtifactExpiry,
    )?;

    let funder = accounts[IX_FUNDER].key.to_bytes();
    let kind = [binding.kind.byte()];
    let context = binding.context.bytes();
    let digest = binding.digest.bytes();
    let (address, bump) = seeds::artifact_stage_pda(program_id, &funder, &kind, &context, &digest);
    expect_pda(accounts[IX_STAGE].key, (address, bump), None)?;
    let header = ArtifactStageHeader {
        binding,
        funder,
        cursor: 0,
        created_slot: current_slot,
        expires_slot,
        stored_bump: bump,
    };
    let space = header.account_len()?;
    create_artifact_pda(
        program_id,
        &accounts[IX_FUNDER],
        &accounts[IX_STAGE],
        &accounts[IX_BEGIN_SYSTEM],
        &rent,
        space,
        &[
            seeds::SEED_ARTIFACT_STAGE,
            &funder,
            &kind,
            &context,
            &digest,
            &[bump],
        ],
    )?;
    let mut data = accounts[IX_STAGE]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    artifact::initialize_stage(&mut data, &header)?;
    require(
        artifact::decode_stage(&data)? == header,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn write(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    binding: ArtifactBinding,
    cursor: u16,
    chunk_len: u16,
    chunk: &[u8; ARTIFACT_CHUNK_BYTES],
) -> Outcome<()> {
    require_count(accounts, WRITE_ACCOUNT_COUNT)?;
    require_zero_sequence(sequence)?;
    binding.validate_for_registration()?;
    require(
        accounts[IX_FUNDER].key != accounts[IX_STAGE].key,
        ClutchError::AccountAlias,
    )?;
    require_stage_metadata(program_id, &accounts[IX_STAGE])?;
    let current_slot = read_clock_slot(&accounts[IX_WRITE_CLOCK])?;
    let mut data = accounts[IX_STAGE]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let header = artifact::decode_stage(&data)?;
    require_funder(header, &accounts[IX_FUNDER])?;
    require_stage_pda(program_id, &accounts[IX_STAGE], header)?;
    require_live(header, current_slot)?;
    artifact::append_chunk(&mut data, binding, cursor, chunk_len, chunk)?;
    Ok(())
}

fn expected_final_pda(program_id: &Pubkey, binding: ArtifactBinding) -> (Pubkey, u8) {
    let context = binding.context.bytes();
    let digest = binding.digest.bytes();
    match binding.kind {
        ArtifactKind::CollateralPolicy => seeds::policy_pda(program_id, &context, &digest),
        ArtifactKind::PriceGrid => seeds::grid_pda(program_id, &context, &digest),
        ArtifactKind::Terms => seeds::terms_pda(program_id, &context, &digest),
        ArtifactKind::BatchPolicy => seeds::batch_policy_pda(program_id, &context, &digest),
        ArtifactKind::DirectBatchPolicyV3 => {
            seeds::direct_batch_policy_v3_pda(program_id, &context, &digest)
        }
        kind @ (ArtifactKind::NativeClaimBasisV1
        | ArtifactKind::EvidenceOnlyRecoveryPolicyV1
        | ArtifactKind::ProductTemplateV4
        | ArtifactKind::PriceMeasurePolicyV1
        | ArtifactKind::MarketGenesisProfileV2
        | ArtifactKind::SeriesFundingQuoteV1
        | ArtifactKind::SeriesAttachmentPlanV1
        | ArtifactKind::SeriesPlanV5
        | ArtifactKind::SeriesFundingTermsV2
        | ArtifactKind::RegistryProgramReleaseV1
        | ArtifactKind::CompiledProductSeriesBundleV1
        | ArtifactKind::RegistryCapabilityProfileV2
        | ArtifactKind::SourceReleaseManifestV1
        | ArtifactKind::SourceReleaseManifestV2
        | ArtifactKind::SourceWorkScheduleV1
        | ArtifactKind::MarketInstancePreimageV2) => {
            seeds::product_artifact_pda(program_id, kind.byte(), &digest)
        }
        kind @ (ArtifactKind::SeriesFundingQuoteV2
        | ArtifactKind::CompiledProductSeriesBundleV2
        | ArtifactKind::SeriesAttachmentPlanV2
        | ArtifactKind::RegistryCapabilityProfileV3
        | ArtifactKind::SeriesFundingQuoteV3
        | ArtifactKind::CompiledProductSeriesBundleV3
        | ArtifactKind::SeriesAttachmentPlanV3
        | ArtifactKind::SeriesFundingQuoteV4
        | ArtifactKind::CompiledProductSeriesBundleV4
        | ArtifactKind::SeriesAttachmentPlanV4) => {
            seeds::product_artifact_pda(program_id, kind.byte(), &digest)
        }
        kind @ (ArtifactKind::RegistryProgramReleaseV2
        | ArtifactKind::RegistryCapabilityProfileV4
        | ArtifactKind::CompiledProductSeriesBundleV5
        | ArtifactKind::SeriesFundingQuoteV5
        | ArtifactKind::SeriesAttachmentPlanV5
        | ArtifactKind::CompiledProductSeriesBundleV6
        | ArtifactKind::WrapperRecipeSetV1
        | ArtifactKind::MarketFamilyCapabilityPolicyV1
        | ArtifactKind::SeriesFundingQuoteV6
        | ArtifactKind::SeriesAttachmentPlanV6
        | ArtifactKind::CompiledProductSeriesBundleV7
        | ArtifactKind::RuntimeLivenessPolicyV1) => {
            seeds::product_artifact_pda(program_id, kind.byte(), &digest)
        }
    }
}

/// Validate a staged artifact without paying the layout crate's portable
/// software SHA-256 cost on the SBF target.
///
/// `clutch-solana-layout` deliberately stays dependency-free and implements
/// SHA-256 in fixed-array Rust.  That is the correct portable reference, but a
/// full Terms preimage consumes more than the default Solana
/// transaction budget when interpreted as SBF instructions.  The adapter can
/// use Solana's authenticated SHA-256 syscall for the *same exact preimage*.
/// Every hostile-byte and semantic check still comes from the owning Terms
/// codec through `decode_unchecked_into`; "unchecked" here means only that the
/// portable digest recomputation is replaced immediately below. The
/// non-production Product/Series catalog uses the same native primitive after
/// the owning core codec has accepted every byte.
#[inline(never)]
fn validate_for_runtime(binding: ArtifactBinding, body: &[u8]) -> Outcome<u8> {
    #[cfg(target_os = "solana")]
    if binding.kind == ArtifactKind::WrapperRecipeSetV1 {
        struct RuntimeRecipeSha256V1;

        impl WrapperRecipeHashV1 for RuntimeRecipeSha256V1 {
            fn hashv(&self, slices: &[&[u8]]) -> [u8; 32] {
                solana_sha256_hasher::hashv(slices).to_bytes()
            }
        }

        binding.validate()?;
        require(
            binding.context == Hash32::ZERO && body.len() == usize::from(binding.exact_len),
            ClutchError::EvidenceBufferMismatch,
        )?;
        let value = WrapperRecipeSetV1::decode(body, &RuntimeRecipeSha256V1)
            .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
        require(
            value
                .id(&RuntimeRecipeSha256V1)
                .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?
                == binding.digest.bytes(),
            ClutchError::EvidenceBufferMismatch,
        )?;
        return Ok(0);
    }

    #[cfg(target_os = "solana")]
    if matches!(
        binding.kind,
        ArtifactKind::RegistryProgramReleaseV1
            | ArtifactKind::RegistryCapabilityProfileV2
            | ArtifactKind::SeriesFundingQuoteV2
            | ArtifactKind::CompiledProductSeriesBundleV2
            | ArtifactKind::SeriesAttachmentPlanV2
            | ArtifactKind::RegistryCapabilityProfileV3
            | ArtifactKind::SeriesFundingQuoteV3
            | ArtifactKind::CompiledProductSeriesBundleV3
            | ArtifactKind::SeriesAttachmentPlanV3
            | ArtifactKind::SeriesFundingQuoteV4
            | ArtifactKind::CompiledProductSeriesBundleV4
            | ArtifactKind::SeriesAttachmentPlanV4
            | ArtifactKind::RegistryProgramReleaseV2
            | ArtifactKind::RegistryCapabilityProfileV4
            | ArtifactKind::CompiledProductSeriesBundleV5
            | ArtifactKind::SeriesFundingQuoteV5
            | ArtifactKind::SeriesAttachmentPlanV5
            | ArtifactKind::CompiledProductSeriesBundleV6
            | ArtifactKind::MarketFamilyCapabilityPolicyV1
    ) {
        binding.validate()?;
        require(
            binding.context == Hash32::ZERO && body.len() == usize::from(binding.exact_len),
            ClutchError::EvidenceBufferMismatch,
        )?;
        let domain = match binding.kind {
            ArtifactKind::RegistryProgramReleaseV1 => {
                RegistryProgramReleaseV1::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                REGISTRY_PROGRAM_RELEASE_V1_DOMAIN
            }
            ArtifactKind::RegistryCapabilityProfileV2 => {
                RegistryCapabilityProfileV2::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                REGISTRY_CAPABILITY_PROFILE_V2_DOMAIN
            }
            ArtifactKind::SeriesFundingQuoteV2 => {
                SeriesFundingQuoteV2::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                SERIES_FUNDING_QUOTE_V2_DOMAIN
            }
            ArtifactKind::CompiledProductSeriesBundleV2 => {
                CompiledProductSeriesBundleV2::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                COMPILED_PRODUCT_SERIES_BUNDLE_V2_DOMAIN
            }
            ArtifactKind::SeriesAttachmentPlanV2 => {
                SeriesAttachmentPlanV2::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                SERIES_ATTACHMENT_PLAN_V2_DOMAIN
            }
            ArtifactKind::RegistryCapabilityProfileV3 => {
                RegistryCapabilityProfileV3::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                REGISTRY_CAPABILITY_PROFILE_V3_DOMAIN
            }
            ArtifactKind::SeriesFundingQuoteV3 => {
                SeriesFundingQuoteV3::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                SERIES_FUNDING_QUOTE_V3_DOMAIN
            }
            ArtifactKind::CompiledProductSeriesBundleV3 => {
                CompiledProductSeriesBundleV3::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                COMPILED_PRODUCT_SERIES_BUNDLE_V3_DOMAIN
            }
            ArtifactKind::SeriesAttachmentPlanV3 => {
                SeriesAttachmentPlanV3::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                SERIES_ATTACHMENT_PLAN_V3_DOMAIN
            }
            ArtifactKind::SeriesFundingQuoteV4 => {
                SeriesFundingQuoteV4::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                SERIES_FUNDING_QUOTE_V4_DOMAIN
            }
            ArtifactKind::CompiledProductSeriesBundleV4 => {
                CompiledProductSeriesBundleV4::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                COMPILED_PRODUCT_SERIES_BUNDLE_V4_DOMAIN
            }
            ArtifactKind::SeriesAttachmentPlanV4 => {
                SeriesAttachmentPlanV4::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                SERIES_ATTACHMENT_PLAN_V4_DOMAIN
            }
            ArtifactKind::RegistryProgramReleaseV2 => {
                RegistryProgramReleaseV2::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                REGISTRY_PROGRAM_RELEASE_V2_DOMAIN
            }
            ArtifactKind::RegistryCapabilityProfileV4 => {
                RegistryCapabilityProfileV4::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                REGISTRY_CAPABILITY_PROFILE_V4_DOMAIN
            }
            ArtifactKind::CompiledProductSeriesBundleV5 => {
                CompiledProductSeriesBundleV5::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                COMPILED_PRODUCT_SERIES_BUNDLE_V5_DOMAIN
            }
            ArtifactKind::SeriesFundingQuoteV5 => {
                SeriesFundingQuoteV5::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                SERIES_FUNDING_QUOTE_V5_DOMAIN
            }
            ArtifactKind::SeriesAttachmentPlanV5 => {
                SeriesAttachmentPlanV5::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                SERIES_ATTACHMENT_PLAN_V5_DOMAIN
            }
            ArtifactKind::CompiledProductSeriesBundleV6 => {
                CompiledProductSeriesBundleV6::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                COMPILED_PRODUCT_SERIES_BUNDLE_V6_DOMAIN
            }
            ArtifactKind::MarketFamilyCapabilityPolicyV1 => {
                MarketFamilyCapabilityPolicyV1::decode(body)
                    .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
                MARKET_FAMILY_CAPABILITY_POLICY_DOMAIN_V1
            }
            _ => return Err(ClutchError::MismatchedState.into()),
        };
        let observed = solana_sha256_hasher::hashv(&[domain, body]);
        require(
            observed.to_bytes() == binding.digest.bytes(),
            ClutchError::EvidenceBufferMismatch,
        )?;
        return Ok(0);
    }

    #[cfg(all(target_os = "solana", feature = "non-production-product-series-lab"))]
    {
        if binding.kind == ArtifactKind::NativeClaimBasisV1 {
            binding.validate()?;
            require(
                binding.context == Hash32::ZERO && body.len() == usize::from(binding.exact_len),
                ClutchError::EvidenceBufferMismatch,
            )?;
            let mut basis = Box::new(NativeClaimBasisV1::ZEROED);
            NativeClaimBasisV1::decode_into(body, &mut basis)
                .map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
            let observed = solana_sha256_hasher::hashv(&[NATIVE_CLAIM_BASIS_DOMAIN, body]);
            require(
                observed.to_bytes() == binding.digest.bytes(),
                ClutchError::EvidenceBufferMismatch,
            )?;
            return Ok(0);
        }

        #[inline(never)]
        fn validate_product<T: FixedCodec>(
            binding: ArtifactBinding,
            body: &[u8],
            domain: &[u8],
        ) -> Outcome<u8> {
            binding.validate()?;
            require(
                binding.context == Hash32::ZERO && body.len() == usize::from(binding.exact_len),
                ClutchError::EvidenceBufferMismatch,
            )?;
            T::decode(body).map_err(|_| Refusal::Codec(CodecError::MismatchedBinding))?;
            let observed = solana_sha256_hasher::hashv(&[domain, body]);
            require(
                observed.to_bytes() == binding.digest.bytes(),
                ClutchError::EvidenceBufferMismatch,
            )?;
            Ok(0)
        }

        match binding.kind {
            ArtifactKind::NativeClaimBasisV1 => {
                return Err(ClutchError::MismatchedState.into());
            }
            ArtifactKind::EvidenceOnlyRecoveryPolicyV1 => {
                return validate_product::<EvidenceOnlyRecoveryPolicyV1>(
                    binding,
                    body,
                    RECOVERY_POLICY_DOMAIN,
                );
            }
            ArtifactKind::ProductTemplateV4 => {
                return validate_product::<ProductTemplateV4>(
                    binding,
                    body,
                    PRODUCT_TEMPLATE_DOMAIN,
                );
            }
            ArtifactKind::PriceMeasurePolicyV1 => {
                return validate_product::<PriceMeasurePolicyV1>(
                    binding,
                    body,
                    PRICE_MEASURE_POLICY_DOMAIN,
                );
            }
            ArtifactKind::MarketGenesisProfileV2 => {
                return validate_product::<MarketGenesisProfileV2>(
                    binding,
                    body,
                    MARKET_GENESIS_PROFILE_V2_DOMAIN,
                );
            }
            ArtifactKind::SeriesFundingQuoteV1 => {
                return validate_product::<SeriesFundingQuoteV1>(
                    binding,
                    body,
                    SERIES_FUNDING_QUOTE_DOMAIN,
                );
            }
            ArtifactKind::SeriesAttachmentPlanV1 => {
                return validate_product::<SeriesAttachmentPlanV1>(
                    binding,
                    body,
                    SERIES_ATTACHMENT_PLAN_DOMAIN,
                );
            }
            ArtifactKind::SeriesPlanV5 => {
                return validate_product::<SeriesPlanV5>(binding, body, SERIES_PLAN_V5_DOMAIN);
            }
            ArtifactKind::SeriesFundingTermsV2 => {
                return validate_product::<SeriesFundingTermsV2>(
                    binding,
                    body,
                    SERIES_FUNDING_TERMS_V2_DOMAIN,
                );
            }
            ArtifactKind::CompiledProductSeriesBundleV1 => {
                return validate_product::<CompiledProductSeriesBundleV1>(
                    binding,
                    body,
                    COMPILED_PRODUCT_SERIES_BUNDLE_V1_DOMAIN,
                );
            }
            _ => {}
        }
    }

    #[cfg(target_os = "solana")]
    if matches!(binding.kind, ArtifactKind::Terms) {
        const TERMS_DOMAIN: &[u8] = b"dragons-clutch/terms/v2";
        const TERMS_BODY_START: usize = 2 + HASH_BYTES;
        const TERMS_TRAILER_BYTES: usize = 2; // stored bump and flags

        binding.validate()?;
        require(
            body.len() == usize::from(binding.exact_len),
            ClutchError::WrongDataLength,
        )?;
        let mut terms = TermsAccount::ZEROED;
        TermsAccount::decode_unchecked_into(body, &mut terms)?;
        if terms.realm != binding.context || terms.terms != binding.digest {
            return Err(CodecError::MismatchedBinding.into());
        }
        let body_end = body
            .len()
            .checked_sub(TERMS_TRAILER_BYTES)
            .ok_or(Refusal::Codec(CodecError::Truncated))?;
        let preimage = body
            .get(TERMS_BODY_START..body_end)
            .ok_or(Refusal::Codec(CodecError::Truncated))?;
        let observed = solana_sha256_hasher::hashv(&[TERMS_DOMAIN, preimage]);
        if observed.to_bytes() != binding.digest.bytes() {
            return Err(CodecError::NonCanonicalIdentity.into());
        }
        return Ok(terms.stored_bump);
    }

    // Host tests remain an independent oracle over the portable implementation;
    // the other, much smaller artifact families also use it on SBF.
    artifact::validate_artifact(binding, body).map_err(Into::into)
}

#[allow(clippy::too_many_arguments)]
fn create_final<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    final_account: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    rent: &super::genesis::RentParameters,
    binding: ArtifactBinding,
    bump: u8,
) -> Outcome<()> {
    let context = binding.context.bytes();
    let digest = binding.digest.bytes();
    match binding.kind {
        ArtifactKind::CollateralPolicy => create_artifact_pda(
            program_id,
            payer,
            final_account,
            system,
            rent,
            usize::from(binding.exact_len),
            &[seeds::SEED_POLICY, &context, &digest, &[bump]],
        ),
        ArtifactKind::PriceGrid => create_artifact_pda(
            program_id,
            payer,
            final_account,
            system,
            rent,
            usize::from(binding.exact_len),
            &[seeds::SEED_GRID, &context, &digest, &[bump]],
        ),
        ArtifactKind::Terms => create_artifact_pda(
            program_id,
            payer,
            final_account,
            system,
            rent,
            usize::from(binding.exact_len),
            &[seeds::SEED_TERMS, &context, &digest, &[bump]],
        ),
        ArtifactKind::BatchPolicy => create_artifact_pda(
            program_id,
            payer,
            final_account,
            system,
            rent,
            usize::from(binding.exact_len),
            &[seeds::SEED_BATCH_POLICY, &context, &digest, &[bump]],
        ),
        ArtifactKind::DirectBatchPolicyV3 => create_artifact_pda(
            program_id,
            payer,
            final_account,
            system,
            rent,
            usize::from(binding.exact_len),
            &[
                seeds::SEED_DIRECT_BATCH_POLICY_V3,
                &context,
                &digest,
                &[bump],
            ],
        ),
        kind @ (ArtifactKind::NativeClaimBasisV1
        | ArtifactKind::EvidenceOnlyRecoveryPolicyV1
        | ArtifactKind::ProductTemplateV4
        | ArtifactKind::PriceMeasurePolicyV1
        | ArtifactKind::MarketGenesisProfileV2
        | ArtifactKind::SeriesFundingQuoteV1
        | ArtifactKind::SeriesAttachmentPlanV1
        | ArtifactKind::SeriesPlanV5
        | ArtifactKind::SeriesFundingTermsV2
        | ArtifactKind::RegistryProgramReleaseV1
        | ArtifactKind::CompiledProductSeriesBundleV1
        | ArtifactKind::RegistryCapabilityProfileV2
        | ArtifactKind::SourceReleaseManifestV1
        | ArtifactKind::SourceReleaseManifestV2
        | ArtifactKind::SourceWorkScheduleV1
        | ArtifactKind::MarketInstancePreimageV2) => {
            let kind_byte = [kind.byte()];
            create_artifact_pda(
                program_id,
                payer,
                final_account,
                system,
                rent,
                usize::from(binding.exact_len),
                &[
                    seeds::SEED_PRODUCT_ARTIFACT_V1,
                    &kind_byte,
                    &digest,
                    &[bump],
                ],
            )
        }
        kind @ (ArtifactKind::SeriesFundingQuoteV2
        | ArtifactKind::CompiledProductSeriesBundleV2
        | ArtifactKind::SeriesAttachmentPlanV2
        | ArtifactKind::RegistryCapabilityProfileV3
        | ArtifactKind::SeriesFundingQuoteV3
        | ArtifactKind::CompiledProductSeriesBundleV3
        | ArtifactKind::SeriesAttachmentPlanV3
        | ArtifactKind::SeriesFundingQuoteV4
        | ArtifactKind::CompiledProductSeriesBundleV4
        | ArtifactKind::SeriesAttachmentPlanV4) => {
            let kind_byte = [kind.byte()];
            create_artifact_pda(
                program_id,
                payer,
                final_account,
                system,
                rent,
                usize::from(binding.exact_len),
                &[
                    seeds::SEED_PRODUCT_ARTIFACT_V1,
                    &kind_byte,
                    &digest,
                    &[bump],
                ],
            )
        }
        kind @ (ArtifactKind::RegistryProgramReleaseV2
        | ArtifactKind::RegistryCapabilityProfileV4
        | ArtifactKind::CompiledProductSeriesBundleV5
        | ArtifactKind::SeriesFundingQuoteV5
        | ArtifactKind::SeriesAttachmentPlanV5
        | ArtifactKind::CompiledProductSeriesBundleV6
        | ArtifactKind::WrapperRecipeSetV1
        | ArtifactKind::MarketFamilyCapabilityPolicyV1
        | ArtifactKind::SeriesFundingQuoteV6
        | ArtifactKind::SeriesAttachmentPlanV6
        | ArtifactKind::CompiledProductSeriesBundleV7
        | ArtifactKind::RuntimeLivenessPolicyV1) => {
            let kind_byte = [kind.byte()];
            create_artifact_pda(
                program_id,
                payer,
                final_account,
                system,
                rent,
                usize::from(binding.exact_len),
                &[
                    seeds::SEED_PRODUCT_ARTIFACT_V1,
                    &kind_byte,
                    &digest,
                    &[bump],
                ],
            )
        }
    }
}

fn close_stage(stage: &AccountInfo, funder: &AccountInfo) -> Outcome<()> {
    require(stage.key != funder.key, ClutchError::AccountAlias)?;
    require(funder.is_writable, ClutchError::NotWritable)?;
    require(!funder.executable, ClutchError::ExecutableAccount)?;
    let refund = stage.lamports();
    let next = funder
        .lamports()
        .checked_add(refund)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    {
        let mut destination = funder
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **destination = next;
    }
    {
        let mut source = stage
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **source = 0;
    }
    let mut data = stage
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.fill(0);
    Ok(())
}

fn seal(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    binding: ArtifactBinding,
) -> Outcome<()> {
    require_count(accounts, SEAL_ACCOUNT_COUNT)?;
    require_zero_sequence(sequence)?;
    binding.validate_for_registration()?;
    require(
        accounts[IX_FUNDER].key != accounts[IX_STAGE].key
            && accounts[IX_FUNDER].key != accounts[IX_FINAL].key
            && accounts[IX_STAGE].key != accounts[IX_FINAL].key,
        ClutchError::AccountAlias,
    )?;
    require(accounts[IX_FUNDER].is_writable, ClutchError::NotWritable)?;
    require_stage_metadata(program_id, &accounts[IX_STAGE])?;
    require(accounts[IX_FINAL].is_writable, ClutchError::NotWritable)?;
    require(
        !accounts[IX_FINAL].executable,
        ClutchError::ExecutableAccount,
    )?;
    require_system_program(&accounts[IX_SEAL_SYSTEM])?;
    let rent = read_rent(&accounts[IX_SEAL_RENT])?;
    let current_slot = read_clock_slot(&accounts[IX_SEAL_CLOCK])?;

    let stage_data = accounts[IX_STAGE].data.borrow();
    let header = artifact::decode_stage(&stage_data)?;
    require_funder(header, &accounts[IX_FUNDER])?;
    require_stage_pda(program_id, &accounts[IX_STAGE], header)?;
    require_live(header, current_slot)?;
    require(
        header.binding == binding,
        ClutchError::EvidenceBufferMismatch,
    )?;
    require(header.is_complete(), ClutchError::ArtifactIncomplete)?;
    let body = artifact::stage_payload(&stage_data)?;
    let encoded_bump = validate_for_runtime(binding, body)?;
    let (final_address, final_bump) = expected_final_pda(program_id, binding);
    expect_pda(accounts[IX_FINAL].key, (final_address, final_bump), None)?;
    if !binding.kind.is_globally_content_addressed()
        && !matches!(
            binding.kind,
            ArtifactKind::CollateralPolicy
                | ArtifactKind::BatchPolicy
                | ArtifactKind::DirectBatchPolicyV3
        )
    {
        require(encoded_bump == final_bump, ClutchError::WrongBump)?;
    }

    if accounts[IX_FINAL].owner == program_id {
        require(
            accounts[IX_FINAL].data_len() == usize::from(binding.exact_len),
            ClutchError::WrongDataLength,
        )?;
        let existing = accounts[IX_FINAL].data.borrow();
        validate_for_runtime(binding, &existing)?;
        require(
            existing.as_ref() == body,
            ClutchError::EvidenceBufferMismatch,
        )?;
    } else {
        create_final(
            program_id,
            &accounts[IX_FUNDER],
            &accounts[IX_FINAL],
            &accounts[IX_SEAL_SYSTEM],
            &rent,
            binding,
            final_bump,
        )?;
        let mut target = accounts[IX_FINAL]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(target.len() == body.len(), ClutchError::WrongDataLength)?;
        target.copy_from_slice(body);
        /* `body` was fully validated above, the target length is exact, and
         * this is a byte-for-byte copy in the same atomic instruction.  A
         * second decode/hash would establish no new fact. */
    }
    drop(stage_data);
    close_stage(&accounts[IX_STAGE], &accounts[IX_FUNDER])
}

fn abort(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    kind: ArtifactKind,
    context: Hash32,
    digest: Hash32,
) -> Outcome<()> {
    require_count(accounts, ABORT_ACCOUNT_COUNT)?;
    require_zero_sequence(sequence)?;
    require_signer(&accounts[IX_ABORT_CALLER])?;
    require(
        accounts[IX_ABORT_CALLER].key != accounts[IX_STAGE].key
            && accounts[IX_ABORT_REFUND].key != accounts[IX_STAGE].key,
        ClutchError::AccountAlias,
    )?;
    require_stage_metadata(program_id, &accounts[IX_STAGE])?;
    let current_slot = read_clock_slot(&accounts[IX_ABORT_CLOCK])?;
    let data = accounts[IX_STAGE].data.borrow();
    let header = artifact::decode_stage(&data)?;
    require(
        header.binding.kind == kind
            && header.binding.context == context
            && header.binding.digest == digest,
        ClutchError::EvidenceBufferMismatch,
    )?;
    require_stage_pda(program_id, &accounts[IX_STAGE], header)?;
    require(
        accounts[IX_ABORT_REFUND].key.to_bytes() == header.funder,
        ClutchError::ArtifactRefundMismatch,
    )?;
    require(
        current_slot > header.expires_slot
            || accounts[IX_ABORT_CALLER].key.to_bytes() == header.funder,
        ClutchError::UnauthorizedActor,
    )?;
    drop(data);
    close_stage(&accounts[IX_STAGE], &accounts[IX_ABORT_REFUND])
}

/// Route one decoded artifact request.
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    match request.action {
        Action::Layout(Intent::BeginArtifact {
            kind,
            context,
            digest,
            exact_len,
            expires_slot,
        }) => begin(
            program_id,
            accounts,
            request.sequence,
            ArtifactBinding {
                kind,
                context,
                digest,
                exact_len,
            },
            expires_slot,
        ),
        Action::Layout(Intent::WriteArtifact {
            kind,
            context,
            digest,
            cursor,
            chunk_len,
            chunk,
        }) => write(
            program_id,
            accounts,
            request.sequence,
            ArtifactBinding {
                kind,
                context,
                digest,
                exact_len: kind.exact_len() as u16,
            },
            cursor,
            chunk_len,
            &chunk,
        ),
        Action::Layout(Intent::SealArtifact {
            kind,
            context,
            digest,
            exact_len,
        }) => seal(
            program_id,
            accounts,
            request.sequence,
            ArtifactBinding {
                kind,
                context,
                digest,
                exact_len,
            },
        ),
        Action::Layout(Intent::AbortArtifact {
            kind,
            context,
            digest,
        }) => abort(
            program_id,
            accounts,
            request.sequence,
            kind,
            context,
            digest,
        ),
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}
