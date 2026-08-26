use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityFundingDerivationV1, CapabilityManifestV1, CompartmentFundingV1,
    FUNDING_STATE_BYTES, FundingAmountsV1, FundingCustodyObservationV1, FundingQuoteV1,
    FundingStateV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_core_contract::{ContentId as CoreContentId, MarketIdentity, MarketRoot, Phase};
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_product_contract::{
    ContentId as ProductContentId,
    capacity::CapacityProfileId,
    product::{InstanceV1, InstanceV1Input, PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1},
    result_domain::{
        FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, FINITE_RESULT_DOMAIN_RELEASE_ID_V1,
        FiniteResultDomainV1,
    },
};
use dclutch_pyth_svm::local_validator_release_v1;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, EXECUTION_AUTHORITY_MANIFEST_SCHEMA_ID_V1,
    ExecutionAuthorityManifestV1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ProgramIdentityV1,
};
use dclutch_resolution_codec::{
    PRIMARY_CERTIFICATE_SEQUENCE_V3, PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
    RESOLUTION_CERTIFICATE_BYTES, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    RESOLUTION_CONTROLLER_RELEASE_ID_V4,
};
use dclutch_source_contract::{
    CapacityEnvelope, ContentId as SourceContentId, PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1,
    ProviderReleaseV1, PythAdapterConfigV1, RecoveryAttemptV1, RecoveryMaterialSlotV1,
    RecoveryPolicyV1, ResolutionPolicyV1, RoundingBoundary, SOURCE_MATERIAL_BYTES,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1,
    SourceAccessProfile, SourceCapacityProfileV1, SourceMaterialInputV1,
    SourceRecoveryMaterialInputV1, SourceResolutionStateV1, SourceSpecV1, StatisticKind,
    StatisticSpecV1, WindowKind, WindowSpecV1, encode_source_material_into_v1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use solana_program::{hash::hashv, rent::Rent};
use solana_sdk::pubkey::Pubkey;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};

use crate::{
    Error, Result,
    model::{GenesisAccountPin, ProgramPin, RecordPair, SourceCase, SuccessorPlan},
};

pub(crate) const GENERATION: u64 = 1;
pub(crate) const FIXTURE_PUBLISH_TIME: i64 = 1_787_431_680;
pub(crate) const BOUNTY_LAMPORTS: u64 = 100_000;
const CERTIFICATE_DUST_LAMPORTS: u64 = 3;
pub(crate) const FEED_ID: [u8; 32] = [0x2a; 32];

#[derive(Debug)]
pub(crate) struct PrepareArgs {
    pub(crate) account_dir: PathBuf,
    pub(crate) plan_path: PathBuf,
    pub(crate) registry_program: Pubkey,
    pub(crate) registry_elf: PathBuf,
    pub(crate) registry_sha256: String,
    pub(crate) core_program: Pubkey,
    pub(crate) core_elf: PathBuf,
    pub(crate) core_sha256: String,
    pub(crate) core_semantic_release_id: String,
    pub(crate) claims_program: Pubkey,
    pub(crate) claims_elf: PathBuf,
    pub(crate) claims_sha256: String,
    pub(crate) claims_semantic_release_id: String,
    pub(crate) trading_program: Pubkey,
    pub(crate) trading_elf: PathBuf,
    pub(crate) trading_sha256: String,
    pub(crate) trading_semantic_release_id: String,
    pub(crate) resolution_program: Pubkey,
    pub(crate) resolution_elf: PathBuf,
    pub(crate) resolution_sha256: String,
    pub(crate) custody_program: Pubkey,
    pub(crate) custody_elf: PathBuf,
    pub(crate) custody_sha256: String,
    pub(crate) custody_semantic_release_id: String,
    pub(crate) rent_credit_program: Pubkey,
    pub(crate) rent_credit_elf: PathBuf,
    pub(crate) rent_credit_sha256: String,
    pub(crate) rent_credit_semantic_release_id: String,
}

#[derive(Serialize)]
struct CliAccount {
    pubkey: String,
    account: CliAccountBody,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CliAccountBody {
    lamports: u64,
    data: (String, &'static str),
    owner: String,
    executable: bool,
    rent_epoch: u64,
    space: usize,
}

struct PlanWriter {
    directory: PathBuf,
    accounts: BTreeMap<String, GenesisAccountPin>,
}

impl PlanWriter {
    fn new(directory: PathBuf) -> Result<Self> {
        if !directory.is_absolute() {
            return Err(Error::new("--account-dir must be absolute"));
        }
        fs::create_dir(&directory).map_err(|error| {
            Error::new(format!(
                "create fresh account directory {}: {error}",
                directory.display()
            ))
        })?;
        Ok(Self {
            directory,
            accounts: BTreeMap::new(),
        })
    }

    fn add(
        &mut self,
        label: impl Into<String>,
        address: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: &[u8],
    ) -> Result<()> {
        self.add_with_executable(label, address, owner, lamports, data, false)
    }

    fn add_with_executable(
        &mut self,
        label: impl Into<String>,
        address: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: &[u8],
        executable: bool,
    ) -> Result<()> {
        let label = label.into();
        if self
            .accounts
            .values()
            .any(|value| value.address == address.to_string())
        {
            return Err(Error::new(format!("duplicate genesis account {address}")));
        }
        let output = CliAccount {
            pubkey: address.to_string(),
            account: CliAccountBody {
                lamports,
                data: (BASE64.encode(data), "base64"),
                owner: owner.to_string(),
                executable,
                rent_epoch: 0,
                space: data.len(),
            },
        };
        let path = self.directory.join(format!("{address}.json"));
        let bytes = serde_json::to_vec_pretty(&output)?;
        let mut file_bytes = bytes.clone();
        file_bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| Error::new(format!("create {}: {error}", path.display())))?;
        file.write_all(&file_bytes)?;
        let data_sha256 = sha256_bytes(data);
        let mut account_hasher = Sha256::new();
        account_hasher.update(owner.as_ref());
        account_hasher.update(lamports.to_le_bytes());
        account_hasher.update([u8::from(executable)]);
        account_hasher.update(0_u64.to_le_bytes());
        account_hasher.update(
            u64::try_from(data.len())
                .map_err(|_| Error::new("account width does not fit u64"))?
                .to_le_bytes(),
        );
        account_hasher.update(data);
        self.accounts.insert(
            label,
            GenesisAccountPin {
                address: address.to_string(),
                owner: owner.to_string(),
                lamports,
                data_len: data.len(),
                data_sha256: hex(&data_sha256),
                account_sha256: hex(&account_hasher.finalize()),
                json_file_sha256: hex(&sha256_bytes(&file_bytes)),
            },
        );
        Ok(())
    }

    fn immutable_upgradeable_program(
        &mut self,
        label: &str,
        program: Pubkey,
        programdata: Pubkey,
        elf: &[u8],
    ) -> Result<()> {
        let mut program_bytes = [0_u8; 36];
        program_bytes[..4].copy_from_slice(&2_u32.to_le_bytes());
        program_bytes[4..].copy_from_slice(programdata.as_ref());
        self.add_with_executable(
            format!("loader.{label}.program"),
            program,
            bpf_loader_upgradeable::ID,
            Rent::default().minimum_balance(program_bytes.len()),
            &program_bytes,
            true,
        )?;
        let mut programdata_bytes = vec![0_u8; 45];
        programdata_bytes[..4].copy_from_slice(&3_u32.to_le_bytes());
        programdata_bytes.extend_from_slice(elf);
        self.add(
            format!("loader.{label}.programdata"),
            programdata,
            bpf_loader_upgradeable::ID,
            Rent::default().minimum_balance(programdata_bytes.len()),
            &programdata_bytes,
        )
    }

    fn protocol(
        &mut self,
        label: impl Into<String>,
        address: Pubkey,
        owner: Pubkey,
        data: &[u8],
    ) -> Result<()> {
        self.add(
            label,
            address,
            owner,
            Rent::default().minimum_balance(data.len()),
            data,
        )
    }

    fn prepaid_certificate(&mut self, label: impl Into<String>, address: Pubkey) -> Result<()> {
        let lamports = Rent::default()
            .minimum_balance(RESOLUTION_CERTIFICATE_BYTES)
            .checked_add(CERTIFICATE_DUST_LAMPORTS)
            .ok_or_else(|| Error::new("certificate prepayment overflow"))?;
        self.add(label, address, system_program::ID, lamports, &[])
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn prepare(args: PrepareArgs) -> Result<SuccessorPlan> {
    validate_prepare(&args)?;
    if !args.plan_path.is_absolute() {
        return Err(Error::new("--plan must be absolute"));
    }
    if args.plan_path.exists() {
        return Err(Error::new(format!(
            "plan output already exists: {}",
            args.plan_path.display()
        )));
    }
    let registry_elf = fs::read(&args.registry_elf)?;
    let core_elf = fs::read(&args.core_elf)?;
    let claims_elf = fs::read(&args.claims_elf)?;
    let trading_elf = fs::read(&args.trading_elf)?;
    let resolution_elf = fs::read(&args.resolution_elf)?;
    let custody_elf = fs::read(&args.custody_elf)?;
    let rent_credit_elf = fs::read(&args.rent_credit_elf)?;
    require_elf(
        "Registry",
        &registry_elf,
        &args.registry_sha256,
        &args.registry_elf,
    )?;
    require_elf("Core", &core_elf, &args.core_sha256, &args.core_elf)?;
    require_elf("Claims", &claims_elf, &args.claims_sha256, &args.claims_elf)?;
    require_elf(
        "Trading",
        &trading_elf,
        &args.trading_sha256,
        &args.trading_elf,
    )?;
    require_elf(
        "Resolution",
        &resolution_elf,
        &args.resolution_sha256,
        &args.resolution_elf,
    )?;
    require_elf(
        "Custody",
        &custody_elf,
        &args.custody_sha256,
        &args.custody_elf,
    )?;
    require_elf(
        "RentCredit",
        &rent_credit_elf,
        &args.rent_credit_sha256,
        &args.rent_credit_elf,
    )?;

    let registry_programdata = programdata(args.registry_program);
    let core_programdata = programdata(args.core_program);
    let claims_programdata = programdata(args.claims_program);
    let trading_programdata = programdata(args.trading_program);
    let resolution_programdata = programdata(args.resolution_program);
    let custody_programdata = programdata(args.custody_program);
    let rent_credit_programdata = programdata(args.rent_credit_program);
    let registry_semantic = sha256_bytes(b"dclutch/local-validator/registry-successor-v1");
    let registry_release = artifact(
        args.registry_program,
        registry_programdata,
        registry_semantic,
        hex32(&args.registry_sha256)?,
    )?;
    let core_semantic = hex32(&args.core_semantic_release_id)?;
    let core_release = artifact(
        args.core_program,
        core_programdata,
        core_semantic,
        hex32(&args.core_sha256)?,
    )?;
    let claims_semantic = hex32(&args.claims_semantic_release_id)?;
    let claims_release = artifact(
        args.claims_program,
        claims_programdata,
        claims_semantic,
        hex32(&args.claims_sha256)?,
    )?;
    let trading_semantic = hex32(&args.trading_semantic_release_id)?;
    let trading_release = artifact(
        args.trading_program,
        trading_programdata,
        trading_semantic,
        hex32(&args.trading_sha256)?,
    )?;
    let resolution_release = artifact(
        args.resolution_program,
        resolution_programdata,
        RESOLUTION_CONTROLLER_RELEASE_ID_V4,
        hex32(&args.resolution_sha256)?,
    )?;
    let custody_semantic = hex32(&args.custody_semantic_release_id)?;
    let custody_release = artifact(
        args.custody_program,
        custody_programdata,
        custody_semantic,
        hex32(&args.custody_sha256)?,
    )?;
    let rent_credit_semantic = hex32(&args.rent_credit_semantic_release_id)?;
    let core_artifact_id = artifact_id(core_release)?;
    let claims_artifact_id = artifact_id(claims_release)?;
    let trading_artifact_id = artifact_id(trading_release)?;
    let resolution_artifact_id = artifact_id(resolution_release)?;
    let custody_artifact_id = artifact_id(custody_release)?;
    let core_binding =
        ExecutionRoleBindingV1::new(program_identity(args.core_program)?, core_artifact_id);
    let claims_binding =
        ExecutionRoleBindingV1::new(program_identity(args.claims_program)?, claims_artifact_id);
    let trading_binding =
        ExecutionRoleBindingV1::new(program_identity(args.trading_program)?, trading_artifact_id);
    let resolution_binding = ExecutionRoleBindingV1::new(
        program_identity(args.resolution_program)?,
        resolution_artifact_id,
    );
    let custody_binding =
        ExecutionRoleBindingV1::new(program_identity(args.custody_program)?, custody_artifact_id);
    let release_set = ExecutionReleaseSetV1::new(
        core_binding,
        claims_binding,
        trading_binding,
        resolution_binding,
        custody_binding,
    )
    .map_err(debug_error("execution release set"))?;
    let release_set_bytes = release_set.to_bytes();
    let release_set_id = sha256_bytes(&release_set_bytes);
    let activation = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set_id],
        &args.registry_program,
    )
    .0;

    let provider = local_validator_release_v1()
        .map_err(|error| Error::new(format!("local Pyth release projection: {error:?}")))?;
    let provider_release_bytes = provider.release().to_bytes();
    let provider_release_id = sha256_bytes(&provider_release_bytes);
    let domain =
        FiniteResultDomainV1::new(product_id([0xa1; 32])?, product_id([0xa2; 32])?, 1, &[0])
            .map_err(debug_error("finite result domain"))?;
    let domain_bytes = domain.to_bytes();
    let result_domain_id =
        hashv(&[FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, &[0], &domain_bytes]).to_bytes();
    let product_instance = InstanceV1::new(InstanceV1Input {
        terms_id: product_id([0xa3; 32])?,
        occurrence_id: product_id([0xa4; 32])?,
        claim_basis_id: product_id([0xa5; 32])?,
        result_domain_id: product_id(result_domain_id)?,
        capacity_profile_id: CapacityProfileId::new(product_id([0xa6; 32])?),
        partition_cell_count: u32::from(domain.outcome_count()),
    })
    .map_err(debug_error("Product instance"))?;
    let product_instance_id = sha256_bytes(&product_instance.to_bytes());

    let capacity = SourceCapacityProfileV1::new(
        CapacityEnvelope::Measured,
        1,
        1,
        source_id([0xb1; 32])?,
        source_id([0xb2; 32])?,
        256,
        0,
    )
    .map_err(debug_error("Source capacity"))?;
    let capacity_id = source_id(sha256_bytes(&capacity.to_bytes()))?;
    let provider_semantic = ProviderReleaseV1::new(
        source_id([0xb3; 32])?,
        source_id(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1)?,
        source_id(provider_release_id)?,
        source_id(provider.release().price_update_codec_id())?,
        source_id(provider.release().adapter_id())?,
    );
    let provider_semantic_id = source_id(sha256_bytes(&provider_semantic.to_bytes()))?;
    let adapter =
        PythAdapterConfigV1::new(FEED_ID, -8, 100).map_err(debug_error("Pyth adapter config"))?;
    let adapter_id = source_id(sha256_bytes(&adapter.to_bytes()))?;
    let source = SourceSpecV1::new(
        source_id([0xa1; 32])?,
        source_id([0xa2; 32])?,
        provider_semantic_id,
        SourceAccessProfile::PythTerminalOneTransaction,
        adapter_id,
        capacity_id,
    );
    let source_spec_id = source_id(sha256_bytes(&source.to_bytes()))?;
    let max_age = local_max_age()?;
    let window = WindowSpecV1::new(
        source_spec_id,
        WindowKind::Terminal,
        FIXTURE_PUBLISH_TIME,
        FIXTURE_PUBLISH_TIME,
        max_age,
        60,
        source_id([0xb4; 32])?,
    )
    .map_err(debug_error("Source window"))?;
    let window_id = source_id(sha256_bytes(&window.to_bytes()))?;
    let funded_window = WindowSpecV1::new(
        source_spec_id,
        WindowKind::Terminal,
        FIXTURE_PUBLISH_TIME,
        FIXTURE_PUBLISH_TIME,
        1,
        60,
        source_id([0xb6; 32])?,
    )
    .map_err(debug_error("funded Source window"))?;
    let funded_window_id = source_id(sha256_bytes(&funded_window.to_bytes()))?;
    let statistic = StatisticSpecV1::new(
        source_id([0xa2; 32])?,
        source_id([0xa2; 32])?,
        StatisticKind::TerminalSample,
        RoundingBoundary::ExactRational,
        1,
        0,
        capacity_id,
        source_id([0xb5; 32])?,
        capacity,
    )
    .map_err(debug_error("Source statistic"))?;
    let statistic_id = source_id(sha256_bytes(&statistic.to_bytes()))?;
    let source_product_id = source_id(product_instance_id)?;
    let recovery_allocation_id = [0xd2; 32];
    let recovery_policy = RecoveryPolicyV1::new(
        capacity_id,
        source_product_id,
        [
            Some(RecoveryAttemptV1::new(
                source_spec_id,
                provider_semantic_id,
                FIXTURE_PUBLISH_TIME + 20,
                source_id(recovery_allocation_id)?,
            )),
            None,
            None,
            None,
        ],
        1,
        capacity,
    )
    .map_err(debug_error("recovery policy"))?;
    let recovery_policy_id = source_id(sha256_bytes(&recovery_policy.to_bytes()))?;
    let recovery_slot = RecoveryMaterialSlotV1::new(
        source_spec_id,
        source,
        provider_semantic_id,
        provider_semantic,
        adapter,
    )
    .map_err(debug_error("recovery material slot"))?;
    let policy = ResolutionPolicyV1::new(
        capacity_id,
        source_product_id,
        source_spec_id,
        window_id,
        statistic_id,
        source_id(result_domain_id)?,
        Some(recovery_policy_id),
    );
    let mut material_bytes = [0_u8; SOURCE_MATERIAL_BYTES];
    encode_source_material_into_v1(
        &mut material_bytes,
        SourceMaterialInputV1 {
            policy: &policy,
            capacity_profile_id: capacity_id,
            capacity_profile: &capacity,
            primary_source_id: source_spec_id,
            primary_source: &source,
            primary_provider_release_id: provider_semantic_id,
            primary_provider_release: &provider_semantic,
            primary_adapter_config: &adapter,
            window_id,
            window: &window,
            statistic_id,
            statistic: &statistic,
            product_instance_id: source_product_id,
            product_instance: &product_instance,
            result_domain: &domain,
            recovery: Some(SourceRecoveryMaterialInputV1 {
                recovery_policy_id,
                recovery_policy: &recovery_policy,
                slots: &[recovery_slot],
            }),
        },
    )
    .map_err(debug_error("Source material"))?;
    let material_id = sha256_bytes(&material_bytes);
    let funded_policy = ResolutionPolicyV1::new(
        capacity_id,
        source_product_id,
        source_spec_id,
        funded_window_id,
        statistic_id,
        source_id(result_domain_id)?,
        Some(recovery_policy_id),
    );
    let mut funded_material_bytes = [0_u8; SOURCE_MATERIAL_BYTES];
    encode_source_material_into_v1(
        &mut funded_material_bytes,
        SourceMaterialInputV1 {
            policy: &funded_policy,
            capacity_profile_id: capacity_id,
            capacity_profile: &capacity,
            primary_source_id: source_spec_id,
            primary_source: &source,
            primary_provider_release_id: provider_semantic_id,
            primary_provider_release: &provider_semantic,
            primary_adapter_config: &adapter,
            window_id: funded_window_id,
            window: &funded_window,
            statistic_id,
            statistic: &statistic,
            product_instance_id: source_product_id,
            product_instance: &product_instance,
            result_domain: &domain,
            recovery: Some(SourceRecoveryMaterialInputV1 {
                recovery_policy_id,
                recovery_policy: &recovery_policy,
                slots: &[recovery_slot],
            }),
        },
    )
    .map_err(debug_error("funded Source material"))?;
    let funded_material_id = sha256_bytes(&funded_material_bytes);

    let funding_rent = Rent::default().minimum_balance(FUNDING_STATE_BYTES);
    let funding_quote = FundingQuoteV1::new(
        FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(funding_rent)
                .map_err(debug_error("funding rent"))?,
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::native_lamports(BOUNTY_LAMPORTS)
                .map_err(debug_error("funding bounty"))?,
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .map_err(debug_error("funding amounts"))?,
        None,
    )
    .map_err(debug_error("funding quote"))?;
    let entries = [
        capability_entry([0xd3; 32], recovery_allocation_id, &funding_quote)?,
        capability_entry([0xd4; 32], recovery_policy_id.to_bytes(), &funding_quote)?,
        capability_entry([0xd8; 32], funded_material_id, &funding_quote)?,
    ];
    let mut manifest_bytes = vec![0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut manifest_bytes)
        .map_err(debug_error("capability manifest"))?;
    let manifest_id = sha256_bytes(&manifest_bytes);
    let authority =
        ExecutionAuthorityManifestV1::new(core_id(manifest_id)?, core_id(release_set_id)?)
            .map_err(debug_error("execution authority manifest"))?;
    let authority_bytes = authority.to_bytes();
    let authority_id = sha256_bytes(&authority_bytes);

    let mut writer = PlanWriter::new(args.account_dir.clone())?;
    writer.immutable_upgradeable_program(
        "registry",
        args.registry_program,
        registry_programdata,
        &registry_elf,
    )?;
    writer.immutable_upgradeable_program("core", args.core_program, core_programdata, &core_elf)?;
    writer.immutable_upgradeable_program(
        "claims",
        args.claims_program,
        claims_programdata,
        &claims_elf,
    )?;
    writer.immutable_upgradeable_program(
        "trading",
        args.trading_program,
        trading_programdata,
        &trading_elf,
    )?;
    writer.immutable_upgradeable_program(
        "resolution",
        args.resolution_program,
        resolution_programdata,
        &resolution_elf,
    )?;
    writer.immutable_upgradeable_program(
        "custody",
        args.custody_program,
        custody_programdata,
        &custody_elf,
    )?;
    writer.immutable_upgradeable_program(
        "rent-credit",
        args.rent_credit_program,
        rent_credit_programdata,
        &rent_credit_elf,
    )?;
    let mut records = BTreeMap::new();
    add_record(
        &mut writer,
        &mut records,
        "execution_release_set",
        args.registry_program,
        EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
        &release_set_bytes,
    )?;
    add_record(
        &mut writer,
        &mut records,
        "core_artifact_release",
        args.registry_program,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        &core_release.to_bytes(),
    )?;
    add_record(
        &mut writer,
        &mut records,
        "claims_artifact_release",
        args.registry_program,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        &claims_release.to_bytes(),
    )?;
    add_record(
        &mut writer,
        &mut records,
        "trading_artifact_release",
        args.registry_program,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        &trading_release.to_bytes(),
    )?;
    add_record(
        &mut writer,
        &mut records,
        "registry_artifact_release",
        args.registry_program,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        &registry_release.to_bytes(),
    )?;
    add_record(
        &mut writer,
        &mut records,
        "resolution_artifact_release",
        args.registry_program,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        &resolution_release.to_bytes(),
    )?;
    add_record(
        &mut writer,
        &mut records,
        "custody_artifact_release",
        args.registry_program,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        &custody_release.to_bytes(),
    )?;
    add_record(
        &mut writer,
        &mut records,
        "execution_authority_manifest",
        args.registry_program,
        EXECUTION_AUTHORITY_MANIFEST_SCHEMA_ID_V1,
        &authority_bytes,
    )?;
    add_record(
        &mut writer,
        &mut records,
        "source_material",
        args.registry_program,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        &material_bytes,
    )?;
    add_record(
        &mut writer,
        &mut records,
        "funded_source_material",
        args.registry_program,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        &funded_material_bytes,
    )?;
    add_record(
        &mut writer,
        &mut records,
        "result_domain",
        args.registry_program,
        FINITE_RESULT_DOMAIN_RELEASE_ID_V1,
        &domain_bytes,
    )?;
    add_record(
        &mut writer,
        &mut records,
        "product_instance",
        args.registry_program,
        PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        &product_instance.to_bytes(),
    )?;
    add_record(
        &mut writer,
        &mut records,
        "pyth_release",
        args.registry_program,
        PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        &provider_release_bytes,
    )?;
    add_record(
        &mut writer,
        &mut records,
        "capability_manifest",
        args.registry_program,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        &manifest_bytes,
    )?;

    let manifest = CapabilityManifestV1::decode(&manifest_bytes)
        .map_err(debug_error("decoded capability manifest"))?;
    let primary = add_primary_case(
        &mut writer,
        args.registry_program,
        args.resolution_program,
        authority_id,
        product_instance_id,
        material_id,
    )?;
    let lifecycle = add_funded_case(
        &mut writer,
        args.registry_program,
        args.resolution_program,
        authority_id,
        product_instance_id,
        funded_material_id,
        core_id(manifest_id)?,
        manifest,
        0xe2,
        false,
    )?;
    let rollback = add_funded_case(
        &mut writer,
        args.registry_program,
        args.resolution_program,
        authority_id,
        product_instance_id,
        funded_material_id,
        core_id(manifest_id)?,
        manifest,
        0xe3,
        true,
    )?;

    let plan = SuccessorPlan {
        schema: "dclutch-local-successor-genesis-plan-v1".into(),
        genesis_boundary: vec![
            "Finalized semantic records are genesis-prepared because Registry activation consumes but does not publish them.".into(),
            "Market roots are genesis-prepared because the Registry successor does not yet own Market creation.".into(),
            "Source resolution states and capability funding are genesis-prepared because their effect-packet creation owners are not yet executable in this split.".into(),
            "Certificate PDAs alone are only prepaid as system-owned zero-data accounts; Resolution allocates, assigns, and writes them in the final atomic output gate.".into(),
        ],
        account_dir: args.account_dir.display().to_string(),
        registry: pin(
            args.registry_program,
            registry_programdata,
            &args.registry_elf,
            args.registry_sha256,
            registry_semantic,
        ),
        core: pin(
            args.core_program,
            core_programdata,
            &args.core_elf,
            args.core_sha256,
            core_semantic,
        ),
        claims: pin(
            args.claims_program,
            claims_programdata,
            &args.claims_elf,
            args.claims_sha256,
            claims_semantic,
        ),
        trading: pin(
            args.trading_program,
            trading_programdata,
            &args.trading_elf,
            args.trading_sha256,
            trading_semantic,
        ),
        resolution: pin(
            args.resolution_program,
            resolution_programdata,
            &args.resolution_elf,
            args.resolution_sha256,
            RESOLUTION_CONTROLLER_RELEASE_ID_V4,
        ),
        custody: pin(
            args.custody_program,
            custody_programdata,
            &args.custody_elf,
            args.custody_sha256,
            custody_semantic,
        ),
        rent_credit: pin(
            args.rent_credit_program,
            rent_credit_programdata,
            &args.rent_credit_elf,
            args.rent_credit_sha256,
            rent_credit_semantic,
        ),
        activation: activation.to_string(),
        release_set_id: hex(&release_set_id),
        records,
        result_domain_id: hex(&result_domain_id),
        source_material_id: hex(&material_id),
        funded_source_material_id: hex(&funded_material_id),
        capability_manifest_id: hex(&manifest_id),
        provider_release_id: hex(&provider_release_id),
        recovery_allocation_id: hex(&recovery_allocation_id),
        exhaustion_allocation_id: hex(recovery_policy_id.as_bytes()),
        fixture_publish_time: FIXTURE_PUBLISH_TIME,
        configured_max_age_seconds: max_age,
        funded_max_age_seconds: 1,
        generation: GENERATION,
        primary,
        lifecycle,
        rollback,
        genesis_accounts: writer.accounts,
    };
    let output = serde_json::to_vec_pretty(&plan)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.plan_path)?;
    file.write_all(&output)?;
    file.write_all(b"\n")?;
    Ok(plan)
}

fn validate_prepare(args: &PrepareArgs) -> Result<()> {
    let programs = [
        args.registry_program,
        args.core_program,
        args.claims_program,
        args.trading_program,
        args.resolution_program,
        args.custody_program,
        args.rent_credit_program,
    ];
    validate_program_ids(&programs)?;
    for (label, path) in [
        ("Registry ELF", &args.registry_elf),
        ("Core ELF", &args.core_elf),
        ("Claims ELF", &args.claims_elf),
        ("Trading ELF", &args.trading_elf),
        ("Resolution ELF", &args.resolution_elf),
        ("Custody ELF", &args.custody_elf),
        ("RentCredit ELF", &args.rent_credit_elf),
    ] {
        if !path.is_absolute() || !path.is_file() {
            return Err(Error::new(format!(
                "{label} must be an existing absolute regular file"
            )));
        }
    }
    for (label, value) in [
        ("Registry SHA-256", &args.registry_sha256),
        ("Core SHA-256", &args.core_sha256),
        ("Claims SHA-256", &args.claims_sha256),
        ("Trading SHA-256", &args.trading_sha256),
        ("Resolution SHA-256", &args.resolution_sha256),
        ("Custody SHA-256", &args.custody_sha256),
        ("RentCredit SHA-256", &args.rent_credit_sha256),
        ("Core semantic release ID", &args.core_semantic_release_id),
        (
            "Claims semantic release ID",
            &args.claims_semantic_release_id,
        ),
        (
            "Trading semantic release ID",
            &args.trading_semantic_release_id,
        ),
        (
            "Custody semantic release ID",
            &args.custody_semantic_release_id,
        ),
        (
            "RentCredit semantic release ID",
            &args.rent_credit_semantic_release_id,
        ),
    ] {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Error::new(format!(
                "{label} must be 64 lowercase hex characters"
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_program_ids(programs: &[Pubkey]) -> Result<()> {
    if programs.contains(&system_program::ID)
        || programs.iter().enumerate().any(|(index, program)| {
            programs
                .iter()
                .skip(index.saturating_add(1))
                .any(|other| other == program)
        })
    {
        return Err(Error::new(
            "Registry, all five role programs, and RentCredit must be pairwise-distinct non-System IDs",
        ));
    }
    Ok(())
}

fn add_record(
    writer: &mut PlanWriter,
    records: &mut BTreeMap<String, RecordPair>,
    label: &str,
    registry: Pubkey,
    schema: [u8; 32],
    content: &[u8],
) -> Result<()> {
    let digest = sha256_bytes(content);
    let raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0;
    writer.protocol(format!("record.{label}"), raw, registry, content)?;
    records.insert(
        label.into(),
        RecordPair {
            raw: raw.to_string(),
            staging: staging.to_string(),
            schema_id: hex(&schema),
            content_sha256: hex(&digest),
        },
    );
    Ok(())
}

fn add_primary_case(
    writer: &mut PlanWriter,
    registry: Pubkey,
    resolution: Pubkey,
    authority_id: [u8; 32],
    product_instance_id: [u8; 32],
    material_id: [u8; 32],
) -> Result<SourceCase> {
    let (market, market_bytes) = market(
        registry,
        authority_id,
        product_instance_id,
        material_id,
        0xe1,
    )?;
    let (state, state_bytes) = state(resolution, market, material_id)?;
    let certificate = certificate(resolution, state, 1, PRIMARY_CERTIFICATE_SEQUENCE_V3);
    writer.protocol("primary.market", market, registry, &market_bytes)?;
    writer.protocol("primary.state", state, resolution, &state_bytes)?;
    writer.prepaid_certificate("primary.certificate.success", certificate)?;
    Ok(SourceCase {
        market: market.to_string(),
        state: state.to_string(),
        certificates: BTreeMap::from([("success".into(), certificate.to_string())]),
        funding: BTreeMap::new(),
        hostile_certificate_preoccupied: false,
    })
}

#[allow(clippy::too_many_arguments)]
fn add_funded_case(
    writer: &mut PlanWriter,
    registry: Pubkey,
    resolution: Pubkey,
    authority_id: [u8; 32],
    product_instance_id: [u8; 32],
    material_id: [u8; 32],
    manifest_id: CoreContentId,
    manifest: CapabilityManifestV1<'_>,
    tag: u8,
    occupied_failure: bool,
) -> Result<SourceCase> {
    let prefix = if occupied_failure {
        "rollback"
    } else {
        "lifecycle"
    };
    let (market, market_bytes) = market(
        registry,
        authority_id,
        product_instance_id,
        material_id,
        tag,
    )?;
    let (state, state_bytes) = state(resolution, market, material_id)?;
    writer.protocol(format!("{prefix}.market"), market, registry, &market_bytes)?;
    writer.protocol(format!("{prefix}.state"), state, resolution, &state_bytes)?;
    let certificates = BTreeMap::from([
        ("recovery".into(), certificate(resolution, state, 2, 1)),
        ("exhaustion".into(), certificate(resolution, state, 3, 2)),
        ("failure".into(), certificate(resolution, state, 4, 3)),
    ]);
    for (name, address) in &certificates {
        if occupied_failure && name == "failure" {
            writer.protocol(
                format!("{prefix}.certificate.{name}.occupied"),
                *address,
                resolution,
                &vec![0xa5; RESOLUTION_CERTIFICATE_BYTES],
            )?;
        } else {
            writer.prepaid_certificate(format!("{prefix}.certificate.{name}"), *address)?;
        }
    }
    let mut funding = BTreeMap::new();
    for (name, index) in [("recovery", 0_u16), ("exhaustion", 1), ("failure", 2)] {
        let value = funding_state(manifest_id, manifest, index)?;
        let derivation = CapabilityFundingDerivationV1::new(
            market.to_bytes(),
            GENERATION,
            manifest_id,
            manifest,
            value,
        )
        .map_err(debug_error("funding derivation"))?;
        let address = Pubkey::find_program_address(&derivation.seed_components(), &resolution).0;
        let lamports = Rent::default()
            .minimum_balance(FUNDING_STATE_BYTES)
            .checked_add(BOUNTY_LAMPORTS)
            .ok_or_else(|| Error::new("funding balance overflow"))?;
        writer.add(
            format!("{prefix}.funding.{name}"),
            address,
            resolution,
            lamports,
            &value.to_bytes(),
        )?;
        funding.insert(name.into(), address.to_string());
    }
    Ok(SourceCase {
        market: market.to_string(),
        state: state.to_string(),
        certificates: certificates
            .into_iter()
            .map(|(name, key)| (name, key.to_string()))
            .collect(),
        funding,
        hostile_certificate_preoccupied: occupied_failure,
    })
}

fn funding_state(
    manifest_id: CoreContentId,
    manifest: CapabilityManifestV1<'_>,
    index: u16,
) -> Result<FundingStateV1> {
    let exact_rent = Rent::default().minimum_balance(FUNDING_STATE_BYTES);
    let custody = FundingCustodyObservationV1::native_only(
        exact_rent
            .checked_mul(2)
            .and_then(|value| value.checked_add(BOUNTY_LAMPORTS))
            .ok_or_else(|| Error::new("funding custody overflow"))?,
        exact_rent,
    )
    .map_err(debug_error("funding custody"))?;
    let mut state = FundingStateV1::new(manifest_id, manifest, index, custody)
        .map_err(debug_error("pending funding"))?;
    state
        .activate(manifest_id, manifest, custody, GENERATION)
        .map_err(debug_error("active funding"))?;
    Ok(state)
}

fn market(
    registry: Pubkey,
    authority_id: [u8; 32],
    product_instance_id: [u8; 32],
    material_id: [u8; 32],
    tag: u8,
) -> Result<(Pubkey, Vec<u8>)> {
    let identity = MarketIdentity::new(
        core_id([tag; 32])?,
        core_id(product_instance_id)?,
        core_id([0xc2; 32])?,
        core_id(material_id)?,
        core_id(authority_id)?,
        GENERATION,
    );
    let digest = sha256_bytes(&identity.to_bytes());
    let address = Pubkey::find_program_address(&[b"dclutch/market-root/v1", &digest], &registry).0;
    let mut root =
        MarketRoot::founding(identity, [0xc3; 32]).map_err(debug_error("Market root"))?;
    root.transition_phase(GENERATION, Phase::Open)
        .map_err(debug_error("open Market"))?;
    let value =
        CategoricalMarketV1::<3>::new(root, 0, [0; 3], CategoricalSettlementSummaryV1::empty())
            .map_err(debug_error("categorical Market"))?;
    let mut bytes =
        vec![0_u8; CategoricalMarketV1::<3>::encoded_len().map_err(debug_error("Market width"))?];
    value
        .encode(&mut bytes)
        .map_err(debug_error("Market bytes"))?;
    Ok((address, bytes))
}

fn state(resolution: Pubkey, market: Pubkey, material_id: [u8; 32]) -> Result<(Pubkey, Vec<u8>)> {
    let (address, bump) = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &resolution,
    );
    let value = SourceResolutionStateV1::fresh(
        market.to_bytes(),
        GENERATION,
        source_id(material_id)?,
        [0xd1; 32],
        bump,
        0,
        0,
    )
    .map_err(debug_error("fresh Source state"))?
    .state();
    Ok((address, value.to_bytes().to_vec()))
}

fn certificate(resolution: Pubkey, state: Pubkey, kind: u8, sequence: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            state.as_ref(),
            &[kind],
            &sequence.to_le_bytes(),
        ],
        &resolution,
    )
    .0
}

fn capability_entry(
    capability: [u8; 32],
    allocation: [u8; 32],
    quote: &FundingQuoteV1,
) -> Result<CapabilityEntryV1> {
    CapabilityEntryV1::new(
        core_id(capability)?,
        core_id(RESOLUTION_CONTROLLER_RELEASE_ID_V4)?,
        core_id(allocation)?,
        core_id([0xd5; 32])?,
        core_id([0xd6; 32])?,
        core_id([0xd7; 32])?,
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        *quote,
    )
    .map_err(debug_error("capability entry"))
}

fn artifact(
    program: Pubkey,
    programdata: Pubkey,
    semantic_release: [u8; 32],
    elf_sha256: [u8; 32],
) -> Result<ArtifactReleaseV1> {
    ArtifactReleaseV1::new(
        program_identity(program)?,
        program_identity(bpf_loader_upgradeable::ID)?,
        programdata.to_bytes(),
        core_id(semantic_release)?,
        elf_sha256,
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .map_err(debug_error("artifact release"))
}

fn artifact_id(release: ArtifactReleaseV1) -> Result<ArtifactReleaseIdV1> {
    ArtifactReleaseIdV1::new(sha256_bytes(&release.to_bytes()))
        .map_err(debug_error("artifact release ID"))
}

fn pin(
    program: Pubkey,
    programdata: Pubkey,
    elf_path: &Path,
    elf_sha256: String,
    semantic_release: [u8; 32],
) -> ProgramPin {
    ProgramPin {
        program_id: program.to_string(),
        programdata_id: programdata.to_string(),
        elf_path: elf_path.display().to_string(),
        elf_sha256,
        semantic_release_id: hex(&semantic_release),
        upgrade_authority: None,
    }
}

fn local_max_age() -> Result<u32> {
    let now = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
        .map_err(|_| Error::new("current Unix timestamp exceeds i64"))?;
    let age = now
        .checked_sub(FIXTURE_PUBLISH_TIME)
        .ok_or_else(|| Error::new("captured fixture publish time is in the future"))?;
    u32::try_from(
        age.checked_add(900)
            .ok_or_else(|| Error::new("local max-age margin overflow"))?,
    )
    .map_err(|_| Error::new("local fixture age does not fit the Source u32 boundary"))
}

fn require_elf(label: &str, bytes: &[u8], expected: &str, path: &Path) -> Result<()> {
    if bytes.get(..4) != Some(b"\x7fELF") {
        return Err(Error::new(format!(
            "{label} input is not an ELF: {}",
            path.display()
        )));
    }
    let observed = hex(&sha256_bytes(bytes));
    if observed != expected {
        return Err(Error::new(format!(
            "{label} ELF SHA-256 mismatch: observed {observed}, expected {expected}"
        )));
    }
    Ok(())
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn program_identity(program: Pubkey) -> Result<ProgramIdentityV1> {
    ProgramIdentityV1::new(program.to_bytes()).map_err(debug_error("program identity"))
}

fn core_id(bytes: [u8; 32]) -> Result<CoreContentId> {
    CoreContentId::new(bytes).map_err(debug_error("Core content ID"))
}

fn product_id(bytes: [u8; 32]) -> Result<ProductContentId> {
    ProductContentId::new(bytes).map_err(debug_error("Product content ID"))
}

fn source_id(bytes: [u8; 32]) -> Result<SourceContentId> {
    SourceContentId::new(bytes).map_err(debug_error("Source content ID"))
}

fn debug_error<E: core::fmt::Debug>(label: &'static str) -> impl FnOnce(E) -> Error {
    move |error| Error::new(format!("{label}: {error:?}"))
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use core::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

pub(crate) fn hex32(value: &str) -> Result<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::new("expected 64 lowercase hex characters"));
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = core::str::from_utf8(pair).map_err(|_| Error::new("non-UTF8 hex"))?;
        output[index] = u8::from_str_radix(text, 16)
            .map_err(|_| Error::new("invalid lowercase hexadecimal"))?;
    }
    Ok(output)
}

pub(crate) fn pubkey(value: &str) -> Result<Pubkey> {
    Pubkey::from_str(value).map_err(|error| Error::new(format!("invalid pubkey {value}: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certificate_addresses_partition_kind_and_sequence() {
        let program = Pubkey::new_unique();
        let state = Pubkey::new_unique();
        let keys = [
            certificate(program, state, 1, 1),
            certificate(program, state, 2, 1),
            certificate(program, state, 3, 2),
            certificate(program, state, 4, 3),
        ];
        for (index, key) in keys.iter().enumerate() {
            assert!(!keys.iter().skip(index + 1).any(|other| other == key));
        }
    }

    #[test]
    fn hex_parser_is_exact() {
        let input = "ab".repeat(32);
        assert_eq!(hex32(&input).expect("hex"), [0xab; 32]);
        assert!(hex32(&"AB".repeat(32)).is_err());
        assert!(hex32("00").is_err());
    }

    #[test]
    fn successor_and_rent_credit_programs_are_distinct_non_system_ids() {
        let mut programs = [
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        ];
        assert!(validate_program_ids(&programs).is_ok());

        programs[6] = programs[0];
        assert!(validate_program_ids(&programs).is_err());

        programs[6] = system_program::ID;
        assert!(validate_program_ids(&programs).is_err());
    }
}
