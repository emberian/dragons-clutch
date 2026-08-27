use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_pyth_svm::local_validator_release_v1;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V1, ProgramIdentityV1,
    ProtocolInfrastructureProfileV1,
};
use dclutch_resolution_codec::{
    PYTH_RELEASE_RECORD_SCHEMA_ID_V1, RESOLUTION_CONTROLLER_RELEASE_ID_V4,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use solana_program::rent::Rent;
use solana_sdk::pubkey::Pubkey;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};

use crate::{
    Error, Result,
    model::{
        CoreBootstrapPin, GenesisAccountPin, InfrastructureProfilePin, ProgramPin, RecordPair,
        SuccessorPlan,
    },
};

pub(crate) const FIXTURE_PUBLISH_TIME: i64 = 1_787_431_680;

#[derive(Debug)]
pub(crate) struct PrepareArgs {
    pub(crate) account_dir: PathBuf,
    pub(crate) plan_path: PathBuf,
    pub(crate) registry_program: Pubkey,
    pub(crate) registry_elf: PathBuf,
    pub(crate) registry_sha256: String,
    pub(crate) registry_semantic_release_id: String,
    pub(crate) core_program: Pubkey,
    pub(crate) core_elf: PathBuf,
    pub(crate) core_sha256: String,
    pub(crate) core_semantic_release_id: String,
    pub(crate) core_bootstrap_upgrade_authority: Pubkey,
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
    pub(crate) resolution_semantic_release_id: String,
    pub(crate) custody_program: Pubkey,
    pub(crate) custody_elf: PathBuf,
    pub(crate) custody_sha256: String,
    pub(crate) custody_semantic_release_id: String,
    pub(crate) record_publication: RecordPublicationV1,
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
        let mut file_bytes = serde_json::to_vec_pretty(&output)?;
        file_bytes.push(b'\n');
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| Error::new(format!("create {}: {error}", path.display())))?
            .write_all(&file_bytes)?;

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
                data_sha256: hex(&sha256_bytes(data)),
                account_sha256: hex(&account_hasher.finalize()),
                json_file_sha256: hex(&sha256_bytes(&file_bytes)),
            },
        );
        Ok(())
    }

    fn upgradeable_program(
        &mut self,
        label: &str,
        program: Pubkey,
        elf: &[u8],
        upgrade_authority: Option<Pubkey>,
    ) -> Result<()> {
        let programdata = programdata(program);
        let mut program_bytes = [0_u8; 36];
        program_bytes[..4].copy_from_slice(&2_u32.to_le_bytes());
        program_bytes[4..].copy_from_slice(programdata.as_ref());
        self.add(
            format!("loader.{label}.program"),
            program,
            bpf_loader_upgradeable::ID,
            Rent::default().minimum_balance(program_bytes.len()),
            &program_bytes,
            true,
        )?;
        let programdata_bytes = loader_programdata_bytes(elf, upgrade_authority);
        self.add(
            format!("loader.{label}.programdata"),
            programdata,
            bpf_loader_upgradeable::ID,
            Rent::default().minimum_balance(programdata_bytes.len()),
            &programdata_bytes,
            false,
        )
    }

    fn finalized_record(
        &mut self,
        label: &str,
        registry: Pubkey,
        schema: [u8; 32],
        content: &[u8],
        publication: RecordPublicationV1,
    ) -> Result<RecordPair> {
        let digest = sha256_bytes(content);
        let raw =
            Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
        let staging = Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
            &registry,
        )
        .0;
        // The address derivation is identical either way -- a record's
        // coordinate is a function of schema and content, never of how the
        // bytes arrived. Only the writer differs.
        if publication == RecordPublicationV1::Genesis {
            self.add(
                format!("record.{label}"),
                raw,
                registry,
                Rent::default().minimum_balance(content.len()),
                content,
                false,
            )?;
        }
        Ok(RecordPair {
            raw: raw.to_string(),
            staging: staging.to_string(),
            schema_id: hex(&schema),
            content_sha256: hex(&digest),
            body_hex: hex(content),
        })
    }
}

/// Who writes the nine infrastructure record bodies.
///
/// `Genesis` injects them as finalized raw-record accounts in the validator's
/// `--account-dir`. That is fast and is what every campaign to date has run,
/// but **no cluster has a genesis you can write into**, so it is not a shape a
/// devnet or mainnet deployment can ever take.
///
/// `Transaction` leaves them out of genesis entirely and makes the supervisor
/// publish each one through the Registry's permissionless
/// `Begin -> Append -> Finalize` path, paying real rent from a real wallet.
/// This is the deployable shape, and running it is what makes a local campaign
/// a rehearsal for a cluster rather than a demonstration on a substrate the
/// cluster does not have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecordPublicationV1 {
    Genesis,
    Transaction,
}

impl RecordPublicationV1 {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Genesis => "genesis",
            Self::Transaction => "transaction",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value {
            "genesis" => Ok(Self::Genesis),
            "transaction" => Ok(Self::Transaction),
            other => Err(Error::new(format!(
                "record publication must be genesis or transaction, not {other}"
            ))),
        }
    }
}

#[derive(Clone, Copy)]
struct ReleaseFacts {
    release: ArtifactReleaseV1,
    id: ArtifactReleaseIdV1,
}

impl ReleaseFacts {
    fn binding(self) -> ExecutionRoleBindingV1 {
        ExecutionRoleBindingV1::new(self.release.program(), self.id)
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn prepare(args: PrepareArgs) -> Result<SuccessorPlan> {
    validate_prepare(&args)?;
    if !args.plan_path.is_absolute() || args.plan_path.exists() {
        return Err(Error::new(
            "--output must be an absolute path to a nonexistent file",
        ));
    }

    let registry_elf = load_elf("Registry", &args.registry_elf, &args.registry_sha256)?;
    let core_elf = load_elf("Core", &args.core_elf, &args.core_sha256)?;
    let claims_elf = load_elf("Claims", &args.claims_elf, &args.claims_sha256)?;
    let trading_elf = load_elf("Trading", &args.trading_elf, &args.trading_sha256)?;
    let resolution_elf = load_elf("Resolution", &args.resolution_elf, &args.resolution_sha256)?;
    let custody_elf = load_elf("Custody", &args.custody_elf, &args.custody_sha256)?;
    let rent_elf = load_elf(
        "RentCredit",
        &args.rent_credit_elf,
        &args.rent_credit_sha256,
    )?;

    let registry = release_facts(
        args.registry_program,
        hex32(&args.registry_semantic_release_id)?,
        hex32(&args.registry_sha256)?,
    )?;
    let core = release_facts(
        args.core_program,
        hex32(&args.core_semantic_release_id)?,
        hex32(&args.core_sha256)?,
    )?;
    let claims = release_facts(
        args.claims_program,
        hex32(&args.claims_semantic_release_id)?,
        hex32(&args.claims_sha256)?,
    )?;
    let trading = release_facts(
        args.trading_program,
        hex32(&args.trading_semantic_release_id)?,
        hex32(&args.trading_sha256)?,
    )?;
    let resolution_semantic = hex32(&args.resolution_semantic_release_id)?;
    if resolution_semantic != RESOLUTION_CONTROLLER_RELEASE_ID_V4 {
        return Err(Error::new(
            "Resolution semantic release ID does not match the selected executable contract",
        ));
    }
    let resolution = release_facts(
        args.resolution_program,
        resolution_semantic,
        hex32(&args.resolution_sha256)?,
    )?;
    let custody = release_facts(
        args.custody_program,
        hex32(&args.custody_semantic_release_id)?,
        hex32(&args.custody_sha256)?,
    )?;
    let rent = release_facts(
        args.rent_credit_program,
        hex32(&args.rent_credit_semantic_release_id)?,
        hex32(&args.rent_credit_sha256)?,
    )?;

    let release_set = ExecutionReleaseSetV1::new(
        core.binding(),
        claims.binding(),
        trading.binding(),
        resolution.binding(),
        custody.binding(),
    )
    .map_err(debug_error("execution release set"))?;
    let release_set_bytes = release_set.to_bytes();
    let release_set_id = sha256_bytes(&release_set_bytes);
    let activation = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set_id],
        &args.registry_program,
    )
    .0;

    let infrastructure = ProtocolInfrastructureProfileV1::new(registry.binding(), rent.binding())
        .map_err(debug_error("protocol infrastructure profile"))?;
    let infrastructure_bytes = infrastructure.to_bytes();
    let infrastructure_address = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &args.core_program,
    )
    .0;

    let provider = local_validator_release_v1()
        .map_err(|error| Error::new(format!("local Pyth release projection: {error:?}")))?;
    let provider_release_bytes = provider.release().to_bytes();
    let provider_release_id = sha256_bytes(&provider_release_bytes);

    let mut writer = PlanWriter::new(args.account_dir.clone())?;
    for (label, program, elf) in [
        ("registry", args.registry_program, registry_elf.as_slice()),
        ("claims", args.claims_program, claims_elf.as_slice()),
        ("trading", args.trading_program, trading_elf.as_slice()),
        (
            "resolution",
            args.resolution_program,
            resolution_elf.as_slice(),
        ),
        ("custody", args.custody_program, custody_elf.as_slice()),
        ("rent-credit", args.rent_credit_program, rent_elf.as_slice()),
    ] {
        writer.upgradeable_program(label, program, elf, None)?;
    }
    writer.upgradeable_program(
        "core",
        args.core_program,
        &core_elf,
        Some(args.core_bootstrap_upgrade_authority),
    )?;

    let publication = args.record_publication;
    let mut records = BTreeMap::new();
    records.insert(
        "execution_release_set".into(),
        writer.finalized_record(
            "execution_release_set",
            args.registry_program,
            EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
            &release_set_bytes,
            publication,
        )?,
    );
    for (label, facts) in [
        ("registry_artifact_release", registry),
        ("core_artifact_release", core),
        ("claims_artifact_release", claims),
        ("trading_artifact_release", trading),
        ("resolution_artifact_release", resolution),
        ("custody_artifact_release", custody),
        ("rent_artifact_release", rent),
    ] {
        records.insert(
            label.into(),
            writer.finalized_record(
                label,
                args.registry_program,
                ARTIFACT_RELEASE_SCHEMA_ID_V1,
                &facts.release.to_bytes(),
                publication,
            )?,
        );
    }
    records.insert(
        "pyth_release".into(),
        writer.finalized_record(
            "pyth_release",
            args.registry_program,
            PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
            &provider_release_bytes,
            publication,
        )?,
    );

    let plan = SuccessorPlan {
        schema: "dclutch-local-successor-infrastructure-plan-v2".into(),
        genesis_boundary: match publication {
            RecordPublicationV1::Genesis => vec![
                "Genesis fixtures are six immutable Loader-v3 programs, one authority-bearing pre-init Core Loader-v3 program with the same exact ELF, and finalized Registry record bodies.".into(),
                "Registry activation, Core infrastructure initialization, RentCredit creation, Found, Source creation, funding, and resolution are not genesis-prepared.".into(),
            ],
            RecordPublicationV1::Transaction => vec![
                "Genesis fixtures are six immutable Loader-v3 programs and one authority-bearing pre-init Core Loader-v3 program with the same exact ELF. Nothing else. No protocol state exists at genesis.".into(),
                "Every infrastructure record body, Registry activation, Core infrastructure initialization, RentCredit creation, Found, Source creation, funding, and resolution is a real transaction. This is the shape a cluster can reach.".into(),
            ],
        },
        bootstrap_order: vec![
            "Authenticate immutable Registry/Rent and remaining role Loader facts; authenticate Core ELF under its ephemeral exact upgrade authority.".into(),
            "Use that in-memory Core upgrade-authority signer to initialize the sole 144-byte ProtocolInfrastructureProfile from exact Registry and Rent artifact records.".into(),
            "Revoke Core upgrade authority to None through Loader-v3 and verify the exact tag-None fixed-offset poststate, including Loader-retained inactive authority bytes, before release recognition.".into(),
            "Activate the five-role immutable ExecutionReleaseSet through Registry, then create RentCredit and execute canonical Found31.".into(),
            "Create and fund Source through Core effects before consuming the captured signed Pyth PriceUpdate through Resolution.".into(),
        ],
        execution_blocker: "Infrastructure activation is executable in one supervised process. LifecycleRentCreditV2 and Found31 remain behind an explicit market-specific input bundle: finalized Realm, ProductV3 basis/result-domain, portfolio, resolution, execution-manifest, and lifecycle-policy records plus exact generation, immutable refund wallet, initial Hoard principal, and lifecycle-rent funding.".into(),
        account_dir: args.account_dir.display().to_string(),
        registry: pin(&args, ProgramKind::Registry, registry),
        core: pin(&args, ProgramKind::Core, core),
        claims: pin(&args, ProgramKind::Claims, claims),
        trading: pin(&args, ProgramKind::Trading, trading),
        resolution: pin(&args, ProgramKind::Resolution, resolution),
        custody: pin(&args, ProgramKind::Custody, custody),
        rent_credit: pin(&args, ProgramKind::Rent, rent),
        activation: activation.to_string(),
        release_set_id: hex(&release_set_id),
        core_bootstrap: CoreBootstrapPin {
            upgrade_authority: args.core_bootstrap_upgrade_authority.to_string(),
            genesis_programdata_sha256: hex(&sha256_bytes(&loader_programdata_bytes(
                &core_elf,
                Some(args.core_bootstrap_upgrade_authority),
            ))),
            post_revoke_programdata_sha256: hex(&sha256_bytes(
                &loader_programdata_bytes_after_revoke(
                &core_elf,
                args.core_bootstrap_upgrade_authority,
            ))),
            release_recognition_requires_revoke: true,
        },
        infrastructure_profile: InfrastructureProfilePin {
            address: infrastructure_address.to_string(),
            schema_id: hex(&PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V1),
            body_sha256: hex(&sha256_bytes(&infrastructure_bytes)),
            body_hex: hex(&infrastructure_bytes),
            registry_artifact_release_id: hex(registry.id.as_bytes()),
            rent_artifact_release_id: hex(rent.id.as_bytes()),
        },
        records,
        record_publication: publication.as_str().into(),
        provider_release_id: hex(&provider_release_id),
        fixture_publish_time: FIXTURE_PUBLISH_TIME,
        genesis_accounts: writer.accounts,
    };
    let mut bytes = serde_json::to_vec_pretty(&plan)?;
    bytes.push(b'\n');
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.plan_path)?
        .write_all(&bytes)?;
    Ok(plan)
}

#[derive(Clone, Copy)]
enum ProgramKind {
    Registry,
    Core,
    Claims,
    Trading,
    Resolution,
    Custody,
    Rent,
}

fn pin(args: &PrepareArgs, kind: ProgramKind, facts: ReleaseFacts) -> ProgramPin {
    let (program, elf, elf_sha, semantic) = match kind {
        ProgramKind::Registry => (
            args.registry_program,
            &args.registry_elf,
            &args.registry_sha256,
            &args.registry_semantic_release_id,
        ),
        ProgramKind::Core => (
            args.core_program,
            &args.core_elf,
            &args.core_sha256,
            &args.core_semantic_release_id,
        ),
        ProgramKind::Claims => (
            args.claims_program,
            &args.claims_elf,
            &args.claims_sha256,
            &args.claims_semantic_release_id,
        ),
        ProgramKind::Trading => (
            args.trading_program,
            &args.trading_elf,
            &args.trading_sha256,
            &args.trading_semantic_release_id,
        ),
        ProgramKind::Resolution => (
            args.resolution_program,
            &args.resolution_elf,
            &args.resolution_sha256,
            &args.resolution_semantic_release_id,
        ),
        ProgramKind::Custody => (
            args.custody_program,
            &args.custody_elf,
            &args.custody_sha256,
            &args.custody_semantic_release_id,
        ),
        ProgramKind::Rent => (
            args.rent_credit_program,
            &args.rent_credit_elf,
            &args.rent_credit_sha256,
            &args.rent_credit_semantic_release_id,
        ),
    };
    ProgramPin {
        program_id: program.to_string(),
        programdata_id: programdata(program).to_string(),
        elf_path: elf.display().to_string(),
        elf_sha256: elf_sha.clone(),
        semantic_release_id: semantic.clone(),
        artifact_release_id: hex(facts.id.as_bytes()),
        upgrade_authority: None,
    }
}

fn release_facts(
    program: Pubkey,
    semantic_release: [u8; 32],
    elf_sha256: [u8; 32],
) -> Result<ReleaseFacts> {
    let release = ArtifactReleaseV1::new(
        program_identity(program)?,
        program_identity(bpf_loader_upgradeable::ID)?,
        programdata(program).to_bytes(),
        content_id(semantic_release)?,
        elf_sha256,
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .map_err(debug_error("artifact release"))?;
    let id = ArtifactReleaseIdV1::new(sha256_bytes(&release.to_bytes()))
        .map_err(debug_error("artifact release ID"))?;
    Ok(ReleaseFacts { release, id })
}

fn validate_prepare(args: &PrepareArgs) -> Result<()> {
    validate_program_ids(&[
        args.registry_program,
        args.core_program,
        args.claims_program,
        args.trading_program,
        args.resolution_program,
        args.custody_program,
        args.rent_credit_program,
    ])?;
    if args.core_bootstrap_upgrade_authority == Pubkey::default()
        || args.core_bootstrap_upgrade_authority == system_program::ID
        || args.core_bootstrap_upgrade_authority == bpf_loader_upgradeable::ID
        || [
            args.registry_program,
            args.core_program,
            args.claims_program,
            args.trading_program,
            args.resolution_program,
            args.custody_program,
            args.rent_credit_program,
        ]
        .contains(&args.core_bootstrap_upgrade_authority)
    {
        return Err(Error::new(
            "Core bootstrap upgrade authority must be a distinct non-native signer identity",
        ));
    }
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
        (
            "Registry semantic release ID",
            &args.registry_semantic_release_id,
        ),
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
            "Resolution semantic release ID",
            &args.resolution_semantic_release_id,
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
        hex32(value).map_err(|_| {
            Error::new(format!(
                "{label} must be 64 lowercase hexadecimal characters"
            ))
        })?;
    }
    Ok(())
}

pub(crate) fn validate_program_ids(programs: &[Pubkey]) -> Result<()> {
    if programs.contains(&system_program::ID)
        || programs.contains(&bpf_loader_upgradeable::ID)
        || programs.iter().enumerate().any(|(index, program)| {
            programs
                .iter()
                .skip(index.saturating_add(1))
                .any(|other| other == program)
        })
    {
        return Err(Error::new(
            "Registry, all five role programs, and RentCredit must be pairwise-distinct non-System/non-Loader IDs",
        ));
    }
    Ok(())
}

pub(crate) fn loader_programdata_bytes(elf: &[u8], upgrade_authority: Option<Pubkey>) -> Vec<u8> {
    let mut bytes = vec![0_u8; 45];
    bytes[..4].copy_from_slice(&3_u32.to_le_bytes());
    if let Some(authority) = upgrade_authority {
        bytes[12] = 1;
        bytes[13..45].copy_from_slice(authority.as_ref());
    }
    bytes.extend_from_slice(elf);
    bytes
}

/// Exact Loader-v3 ProgramData bytes after its real `SetAuthority(Some -> None)`
/// transition. Loader bincode overwrites the 13-byte serialized None state but
/// does not clear the former 32-byte authority region before the fixed ELF
/// offset. Those retained bytes are non-authoritative runtime residue.
pub(crate) fn loader_programdata_bytes_after_revoke(
    elf: &[u8],
    prior_authority: Pubkey,
) -> Vec<u8> {
    let mut bytes = loader_programdata_bytes(elf, Some(prior_authority));
    bytes[12] = 0;
    bytes
}

fn load_elf(label: &str, path: &Path, expected: &str) -> Result<Vec<u8>> {
    let bytes = fs::read(path)?;
    if bytes.get(..4) != Some(b"\x7fELF") {
        return Err(Error::new(format!(
            "{label} input is not an ELF: {}",
            path.display()
        )));
    }
    let observed = hex(&sha256_bytes(&bytes));
    if observed != expected {
        return Err(Error::new(format!(
            "{label} ELF SHA-256 mismatch: observed {observed}, expected {expected}"
        )));
    }
    Ok(bytes)
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn program_identity(program: Pubkey) -> Result<ProgramIdentityV1> {
    ProgramIdentityV1::new(program.to_bytes()).map_err(debug_error("program identity"))
}

fn content_id(bytes: [u8; 32]) -> Result<CoreContentId> {
    CoreContentId::new(bytes).map_err(debug_error("content ID"))
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

    fn write_test_elf(directory: &Path, name: &str, tag: u8) -> (PathBuf, String) {
        let path = directory.join(name);
        let bytes = [0x7f, b'E', b'L', b'F', tag];
        fs::write(&path, bytes).expect("write test ELF");
        (path, hex(&sha256_bytes(&bytes)))
    }

    #[test]
    fn immutable_loader_header_has_fixed_45_byte_none_padding() {
        let bytes = loader_programdata_bytes(b"\x7fELFbody", None);
        assert_eq!(bytes.len(), 53);
        assert_eq!(&bytes[..4], &3_u32.to_le_bytes());
        assert_eq!(&bytes[4..12], &0_u64.to_le_bytes());
        assert_eq!(bytes[12], 0);
        assert!(bytes[13..45].iter().all(|byte| *byte == 0));
        assert_eq!(&bytes[45..], b"\x7fELFbody");
    }

    #[test]
    fn pre_init_core_and_real_loader_revoke_differ_only_in_authority_tag() {
        let authority = Pubkey::new_unique();
        let initial = loader_programdata_bytes(b"\x7fELFbody", Some(authority));
        let revoked = loader_programdata_bytes_after_revoke(b"\x7fELFbody", authority);
        assert_eq!(initial.len(), revoked.len());
        assert_eq!(initial[12], 1);
        assert_eq!(&initial[13..45], authority.as_ref());
        assert_eq!(revoked[12], 0);
        assert_eq!(&revoked[13..45], authority.as_ref());
        assert_eq!(&initial[..12], &revoked[..12]);
        assert_eq!(&initial[45..], &revoked[45..]);
        assert_ne!(sha256_bytes(&initial), sha256_bytes(&revoked));
    }

    #[test]
    fn successor_and_infrastructure_programs_are_distinct_non_native_ids() {
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
        programs[6] = bpf_loader_upgradeable::ID;
        assert!(validate_program_ids(&programs).is_err());
        programs[6] = system_program::ID;
        assert!(validate_program_ids(&programs).is_err());
    }

    #[test]
    fn infrastructure_profile_binds_distinct_registry_and_rent_artifacts() {
        let registry =
            release_facts(Pubkey::new_unique(), [1; 32], [2; 32]).expect("Registry release");
        let rent = release_facts(Pubkey::new_unique(), [3; 32], [4; 32]).expect("Rent release");
        let profile = ProtocolInfrastructureProfileV1::new(registry.binding(), rent.binding())
            .expect("infrastructure profile");
        assert_eq!(profile.registry(), registry.binding());
        assert_eq!(profile.rent(), rent.binding());
        assert_eq!(profile.to_bytes().len(), 144);
        assert!(
            ProtocolInfrastructureProfileV1::new(registry.binding(), registry.binding()).is_err()
        );
    }

    #[test]
    fn lowercase_hex_parser_is_exact() {
        assert_eq!(hex32(&"ab".repeat(32)).expect("hex"), [0xab; 32]);
        assert!(hex32(&"AB".repeat(32)).is_err());
        assert!(hex32("00").is_err());
    }

    fn prepared_plan(publication: RecordPublicationV1) -> (SuccessorPlan, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("dclutch-successor-plan-{}", Pubkey::new_unique()));
        fs::create_dir(&root).expect("create test root");
        let (registry_elf, registry_sha256) = write_test_elf(&root, "dclutch_registry_sbf.so", 1);
        let (core_elf, core_sha256) = write_test_elf(&root, "dclutch_core_sbf.so", 2);
        let (claims_elf, claims_sha256) = write_test_elf(&root, "dclutch_claims_sbf.so", 3);
        let (trading_elf, trading_sha256) = write_test_elf(&root, "dclutch_trading_sbf.so", 4);
        let (resolution_elf, resolution_sha256) =
            write_test_elf(&root, "dclutch_resolution_proof_sbf.so", 5);
        let (custody_elf, custody_sha256) = write_test_elf(&root, "dclutch_custody_sbf.so", 6);
        let (rent_credit_elf, rent_credit_sha256) = write_test_elf(&root, "dclutch_rent_sbf.so", 7);
        let program = |tag| Pubkey::new_from_array([tag; 32]);
        let plan = prepare(PrepareArgs {
            account_dir: root.join("accounts"),
            plan_path: root.join("plan.json"),
            registry_program: program(1),
            registry_elf,
            registry_sha256,
            registry_semantic_release_id: hex(&[11; 32]),
            core_program: program(2),
            core_elf,
            core_sha256,
            core_semantic_release_id: hex(&[12; 32]),
            core_bootstrap_upgrade_authority: program(8),
            claims_program: program(3),
            claims_elf,
            claims_sha256,
            claims_semantic_release_id: hex(&[13; 32]),
            trading_program: program(4),
            trading_elf,
            trading_sha256,
            trading_semantic_release_id: hex(&[14; 32]),
            resolution_program: program(5),
            resolution_elf,
            resolution_sha256,
            resolution_semantic_release_id: hex(&RESOLUTION_CONTROLLER_RELEASE_ID_V4),
            custody_program: program(6),
            custody_elf,
            custody_sha256,
            custody_semantic_release_id: hex(&[16; 32]),
            rent_credit_program: program(7),
            rent_credit_elf,
            rent_credit_sha256,
            rent_credit_semantic_release_id: hex(&[17; 32]),
            record_publication: publication,
        })
        .expect("prepare infrastructure plan");
        (plan, root)
    }

    #[test]
    fn prepare_materializes_only_seven_loaders_and_nine_finalized_records() {
        let (plan, root) = prepared_plan(RecordPublicationV1::Genesis);
        assert_eq!(plan.record_publication, "genesis");
        assert_eq!(plan.genesis_accounts.len(), 23);
        assert_eq!(plan.records.len(), 9);
        assert!(
            !plan
                .genesis_accounts
                .keys()
                .any(|label| label.contains("market") || label.contains("source"))
        );
        assert_eq!(
            plan.infrastructure_profile.registry_artifact_release_id,
            plan.registry.artifact_release_id
        );
        assert_eq!(
            plan.infrastructure_profile.rent_artifact_release_id,
            plan.rent_credit.artifact_release_id
        );
        fs::remove_dir_all(&root).expect("remove scoped test root");
    }

    /// The deployable shape: genesis carries the seven programs and nothing
    /// else. Every record coordinate must be identical to the genesis-injected
    /// plan's, because a record's address is a function of schema and content
    /// and never of who wrote the bytes.
    #[test]
    fn transaction_publication_leaves_genesis_holding_only_the_seven_programs() {
        let (genesis, genesis_root) = prepared_plan(RecordPublicationV1::Genesis);
        let (transaction, transaction_root) = prepared_plan(RecordPublicationV1::Transaction);

        assert_eq!(transaction.record_publication, "transaction");
        assert_eq!(transaction.genesis_accounts.len(), 14);
        assert_eq!(transaction.records.len(), 9);
        assert!(
            transaction
                .genesis_accounts
                .keys()
                .all(|label| label.starts_with("loader."))
        );

        for (label, pair) in &transaction.records {
            let genesis_pair = genesis
                .records
                .get(label)
                .expect("both modes derive the same record set");
            assert_eq!(pair.raw, genesis_pair.raw, "raw record moved for {label}");
            assert_eq!(pair.staging, genesis_pair.staging);
            assert_eq!(pair.content_sha256, genesis_pair.content_sha256);
            assert_eq!(pair.body_hex, genesis_pair.body_hex);
            // The carried body must be the body the coordinate commits to.
            let body = hex32_bytes(&pair.body_hex).expect("record body hex");
            assert_eq!(hex(&sha256_bytes(&body)), pair.content_sha256);
            assert!(
                !body.is_empty(),
                "record {label} carried an empty body into transaction mode"
            );
        }

        fs::remove_dir_all(&genesis_root).expect("remove scoped test root");
        fs::remove_dir_all(&transaction_root).expect("remove scoped test root");
    }

    fn hex32_bytes(value: &str) -> Option<Vec<u8>> {
        if !value.len().is_multiple_of(2) {
            return None;
        }
        let bytes = value.as_bytes();
        let mut out = Vec::with_capacity(value.len() / 2);
        for pair in bytes.chunks_exact(2) {
            let high = nibble(pair[0])?;
            let low = nibble(pair[1])?;
            out.push((high << 4) | low);
        }
        Some(out)
    }

    fn nibble(value: u8) -> Option<u8> {
        match value {
            b'0'..=b'9' => Some(value - b'0'),
            b'a'..=b'f' => Some(value - b'a' + 10),
            _ => None,
        }
    }
}
