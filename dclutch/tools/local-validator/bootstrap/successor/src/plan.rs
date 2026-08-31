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
use dclutch_registry_svm::ProgramDataV3View;
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V1, ProgramIdentityV1,
    ProtocolInfrastructureProfileV1,
};
use dclutch_resolution_codec::{
    PYTH_RELEASE_RECORD_SCHEMA_ID_V1, RESOLUTION_CONTROLLER_RELEASE_ID_V7,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_program::rent::Rent;
use solana_sdk::pubkey::Pubkey;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};

use crate::{
    Error, Result,
    model::{
        CheckedDeploymentDispositionV1, CheckedUpgradeRolePinV1, CheckedUpgradeSetPinV1,
        CoreBootstrapPin, GenesisAccountPin, InfrastructureProfilePin, ProgramPin, RecordPair,
        SuccessorPlan,
    },
};

pub(crate) const FIXTURE_PUBLISH_TIME: i64 = 1_787_431_680;
pub(crate) const LOCAL_PYTH_RECEIVER_ELF: &[u8] =
    include_bytes!("../../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver.so");
pub(crate) const LOCAL_PYTH_ROUTER_ELF: &[u8] =
    include_bytes!("../../../../../fixtures/pyth/local-upgraded-2026-08-22/router.so");

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
    pub(crate) deployments: RoleDeploymentsV1,
    pub(crate) rent_credit_program: Pubkey,
    pub(crate) rent_credit_elf: PathBuf,
    pub(crate) rent_credit_sha256: String,
    pub(crate) rent_credit_semantic_release_id: String,
    pub(crate) checked_upgrade_set: Option<CheckedUpgradeSetPinV1>,
}

/// Where one role's deployment slot was read from.
///
/// The slot is **never** a number a caller hands the plan. It is hostile-decoded
/// out of a Loader V3 `ProgramData` account image by exactly the
/// [`ProgramDataV3View`] parse that `require_loader_linkage` runs on chain
/// before building the `DeploymentObservationV1` that
/// `ArtifactReleaseV1::authenticate_deployment` checks. One encoding, one
/// reader, on both sides of the refusal. What a caller chooses is *which
/// account image* that is, and this enum records which it chose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeploymentSourceV1 {
    /// The image this plan materializes into the validator's `--account-dir`.
    /// A genesis install has no deploy transaction, so its slot is a property
    /// of the fixture: zero, unless a rehearsal deliberately asked for one.
    GenesisInstall,
    /// A real `ProgramData` account read off a cluster. This is the only
    /// source a devnet or mainnet deployment can have, because the slot a
    /// deploy lands in is unknowable until it has landed — a local deploy was
    /// measured at slot 167 and its redeploy at 531.
    ObservedAccount,
}

impl DeploymentSourceV1 {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::GenesisInstall => "genesis-install",
            Self::ObservedAccount => "observed-programdata-account",
        }
    }
}

/// How one role's `ProgramData` account image is obtained.
#[derive(Clone, Debug, Default)]
pub(crate) struct RoleDeploymentInputV1 {
    /// A complete `ProgramData` account body observed on a cluster, including
    /// its Loader metadata and ELF tail. Present means the role's release is
    /// minted from an observation.
    pub(crate) observed_programdata: Option<PathBuf>,
    /// Checked-mode-only exact ProgramData body derived from authenticated
    /// CarryForward evidence. No public flag populates this field.
    pub(crate) observed_programdata_bytes: Option<Vec<u8>>,
    /// Declared SHA-256 of the complete live ELF tail inside the observed
    /// ProgramData account, including Loader allocation padding. Required
    /// exactly when `observed_programdata` is present.
    pub(crate) expected_live_elf_sha256: Option<String>,
    /// The slot written into the genesis install this plan materializes, for a
    /// local rehearsal that wants a nonzero slot exercised end to end. It is
    /// refused together with `observed_programdata`: an observation is not
    /// something a caller gets to overwrite.
    pub(crate) genesis_deployment_slot: u64,
    /// The upgrade authority the caller DECLARES this role's observed
    /// `ProgramData` carries, for the mutable substrate decision 0012 chose.
    ///
    /// It does not supply the authority the release binds — that is decoded
    /// out of the observation like the slot is. It is the declaration the
    /// observation is checked against, so a role that quietly became mutable,
    /// or mutable under a key nobody named, still refuses at plan time instead
    /// of minting a release that hands upgrade rights to a stranger.
    ///
    /// Absent means the caller declares `None`, which is what every invocation
    /// before 0012 meant and is why they all still behave identically.
    pub(crate) expected_upgrade_authority: Option<Pubkey>,
}

/// The seven roles' deployment sources.
#[derive(Clone, Debug, Default)]
pub(crate) struct RoleDeploymentsV1 {
    pub(crate) registry: RoleDeploymentInputV1,
    pub(crate) core: RoleDeploymentInputV1,
    pub(crate) claims: RoleDeploymentInputV1,
    pub(crate) trading: RoleDeploymentInputV1,
    pub(crate) resolution: RoleDeploymentInputV1,
    pub(crate) custody: RoleDeploymentInputV1,
    pub(crate) rent_credit: RoleDeploymentInputV1,
}

/// One role's exact `ProgramData` account image and the facts decoded from it.
#[derive(Clone, Debug)]
struct RoleDeployment {
    image: Vec<u8>,
    deployment_slot: u64,
    live_elf_sha256: [u8; 32],
    live_elf_padding_bytes: usize,
    /// The upgrade authority the image actually carries, hostile-decoded by
    /// the same reader the on-chain authenticator runs. Never a caller's
    /// number — a declaration only gets to be checked against this, and the
    /// release's policy is minted from it.
    upgrade_authority: Option<[u8; 32]>,
    source: DeploymentSourceV1,
}

/// Validate the relationship Loader V3 may create between a checked build
/// candidate and the live ProgramData ELF tail: the candidate is an exact
/// prefix and every remaining allocated byte is zero.
///
/// This deliberately does not claim the two byte strings have one digest. A
/// release authenticates the complete live tail, while a checked-release
/// manifest normally authenticates the raw build output. Callers that want to
/// bridge those identities must record both digests rather than silently
/// replacing either one.
pub(crate) fn checked_candidate_padding_v1(candidate: &[u8], deployed: &[u8]) -> Result<usize> {
    if candidate.get(..4) != Some(b"\x7fELF") {
        return Err(Error::new("checked candidate is not an ELF"));
    }
    let suffix = deployed
        .strip_prefix(candidate)
        .ok_or_else(|| Error::new("deployed ProgramData ELF does not begin with the candidate"))?;
    if suffix.iter().any(|byte| *byte != 0) {
        return Err(Error::new(
            "deployed ProgramData ELF has nonzero bytes after the candidate",
        ));
    }
    Ok(suffix.len())
}

/// Read one role's `ProgramData` image and hostile-decode its deployment facts.
///
/// The ELF and the upgrade authority are checked against what this plan is
/// going to authenticate with, so an observation that does not describe the
/// pinned artifact is refused here rather than by a `DeploymentSlotMismatch`
/// or `ElfDigestMismatch` on chain after the money is spent.
///
/// `protocol_upgrade_authority` is the authority the PROTOCOL fixes for this
/// role. It is `Some` only for Core, whose bootstrap authority the caller names
/// on the command line and whose campaign revokes it before activation. Every
/// other role has none fixed, so a caller observing a mutable deployment must
/// DECLARE its authority per role for the observation to be admitted at all.
/// The two are required to agree where both are present: a plan does not get to
/// authenticate against one key while claiming another.
fn role_deployment(
    label: &str,
    elf: &[u8],
    protocol_upgrade_authority: Option<Pubkey>,
    input: &RoleDeploymentInputV1,
) -> Result<RoleDeployment> {
    let expected_upgrade_authority = match (
        input.expected_upgrade_authority,
        protocol_upgrade_authority,
    ) {
        (Some(declared), Some(fixed)) if declared != fixed => {
            return Err(Error::new(format!(
                "{label} declares upgrade authority {declared}, but this plan authenticates against {fixed}"
            )));
        }
        (Some(declared), _) => Some(declared),
        (None, fixed) => fixed,
    };
    let (image, source) = match (
        input.observed_programdata.as_deref(),
        input.observed_programdata_bytes.as_deref(),
    ) {
        (Some(_), Some(_)) => {
            return Err(Error::new(format!(
                "{label} ProgramData has both a raw path and checked embedded evidence"
            )));
        }
        (Some(path), None) => {
            if input.genesis_deployment_slot != 0 {
                return Err(Error::new(format!(
                    "{label} may either observe a ProgramData account or fabricate a genesis deployment slot, not both"
                )));
            }
            (fs::read(path)?, DeploymentSourceV1::ObservedAccount)
        }
        (None, Some(bytes)) => {
            if input.genesis_deployment_slot != 0 {
                return Err(Error::new(format!(
                    "{label} checked ProgramData cannot also select a genesis slot"
                )));
            }
            (bytes.to_vec(), DeploymentSourceV1::ObservedAccount)
        }
        (None, None) => {
            // A declaration describes something already on a cluster. The
            // genesis install fabricates its own account image, so a
            // declaration here would be describing bytes this plan is about to
            // write — and a mutable genesis role is a substrate this campaign
            // has no revocation stage for. Refused rather than half-honored.
            if input.expected_upgrade_authority.is_some() {
                return Err(Error::new(format!(
                    "{label} declares an expected upgrade authority, which describes an observed account; the genesis install this plan materializes fabricates its own"
                )));
            }
            (
                loader_programdata_bytes(
                    elf,
                    input.genesis_deployment_slot,
                    expected_upgrade_authority,
                ),
                DeploymentSourceV1::GenesisInstall,
            )
        }
    };
    let view = ProgramDataV3View::parse(&image).map_err(|error| {
        Error::new(format!(
            "{label} ProgramData is not a Loader-v3 account: {error:?}"
        ))
    })?;
    let live_elf_padding_bytes =
        checked_candidate_padding_v1(elf, view.elf()).map_err(|error| {
            Error::new(format!(
                "{label} live ProgramData ELF is not the checked raw candidate plus zero padding: \
             {error}"
            ))
        })?;
    let live_elf_sha256 = sha256_bytes(view.elf());
    let expected_live = match source {
        DeploymentSourceV1::ObservedAccount => input
            .expected_live_elf_sha256
            .as_deref()
            .ok_or_else(|| {
                Error::new(format!(
                    "{label} observed ProgramData requires an explicit live ELF SHA-256; the raw \
                     candidate digest does not authenticate Loader allocation padding"
                ))
            })
            .and_then(hex32)?,
        DeploymentSourceV1::GenesisInstall => {
            if input.expected_live_elf_sha256.is_some() {
                return Err(Error::new(format!(
                    "{label} live ELF SHA-256 describes an observed ProgramData account and is \
                     refused for a genesis install"
                )));
            }
            sha256_bytes(elf)
        }
    };
    if live_elf_sha256 != expected_live {
        return Err(Error::new(format!(
            "{label} complete live ProgramData ELF SHA-256 is {}, expected {}",
            hex(&live_elf_sha256),
            hex(&expected_live)
        )));
    }
    if view.upgrade_authority() != expected_upgrade_authority.map(|value| value.to_bytes()) {
        return Err(Error::new(format!(
            "{label} ProgramData account upgrade authority is not the one this plan authenticates against"
        )));
    }
    let deployment_slot = view.deployment_slot();
    let upgrade_authority = view.upgrade_authority();
    Ok(RoleDeployment {
        image,
        deployment_slot,
        live_elf_sha256,
        live_elf_padding_bytes,
        upgrade_authority,
        source,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CliAccount {
    pubkey: String,
    account: CliAccountBody,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
struct CliAccountBody {
    lamports: u64,
    data: (String, String),
    owner: String,
    executable: bool,
    rent_epoch: u64,
    space: usize,
}

pub(crate) struct CheckedCliAccountV1 {
    pub(crate) pubkey: Pubkey,
    pub(crate) owner: Pubkey,
    pub(crate) lamports: u64,
    pub(crate) data: Vec<u8>,
    pub(crate) executable: bool,
    pub(crate) rent_epoch: u64,
}

const MAX_CLI_ACCOUNT_FILE_BYTES_V1: u64 = 16 * 1024 * 1024;

pub(crate) fn authenticate_cli_account_file_v1(
    path: &Path,
    pin: &GenesisAccountPin,
) -> Result<CheckedCliAccountV1> {
    if !path.is_absolute() {
        return Err(Error::new("genesis account JSON path must be absolute"));
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_file()
        || metadata.len() > MAX_CLI_ACCOUNT_FILE_BYTES_V1
    {
        return Err(Error::new(
            "genesis account JSON must be one bounded regular non-symlink file",
        ));
    }
    let bytes = fs::read(path)?;
    if hex(&sha256_bytes(&bytes)) != pin.json_file_sha256 {
        return Err(Error::new(
            "genesis account JSON bytes differ from their persisted file digest",
        ));
    }
    let value = crate::rpc::parse_json_without_duplicate_keys_v1(&bytes)?;
    let account: CliAccount = serde_json::from_value(value)?;
    let address = pubkey(&account.pubkey)?;
    let owner = pubkey(&account.account.owner)?;
    if account.pubkey != pin.address
        || account.account.owner != pin.owner
        || account.account.lamports != pin.lamports
        || account.account.rent_epoch != 0
        || account.account.data.1 != "base64"
    {
        return Err(Error::new(
            "genesis account JSON header differs from its persisted account pin",
        ));
    }
    let data = BASE64
        .decode(&account.account.data.0)
        .map_err(|error| Error::new(format!("genesis account data base64: {error}")))?;
    if BASE64.encode(&data) != account.account.data.0
        || account.account.space != data.len()
        || account.account.space != pin.data_len
        || hex(&sha256_bytes(&data)) != pin.data_sha256
        || account_sha256_v1(
            owner,
            account.account.lamports,
            account.account.executable,
            account.account.rent_epoch,
            &data,
        )? != pin.account_sha256
    {
        return Err(Error::new(
            "genesis account JSON data, space, or account digest differs from its persisted pin",
        ));
    }
    Ok(CheckedCliAccountV1 {
        pubkey: address,
        owner,
        lamports: account.account.lamports,
        data,
        executable: account.account.executable,
        rent_epoch: account.account.rent_epoch,
    })
}

pub(crate) fn account_sha256_v1(
    owner: Pubkey,
    lamports: u64,
    executable: bool,
    rent_epoch: u64,
    data: &[u8],
) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(owner.as_ref());
    hasher.update(lamports.to_le_bytes());
    hasher.update([u8::from(executable)]);
    hasher.update(rent_epoch.to_le_bytes());
    hasher.update(
        u64::try_from(data.len())
            .map_err(|_| Error::new("account width does not fit u64"))?
            .to_le_bytes(),
    );
    hasher.update(data);
    Ok(hex(&hasher.finalize()))
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

    fn sync(&self) -> Result<()> {
        fs::File::open(&self.directory)?.sync_all()?;
        if let Some(parent) = self.directory.parent() {
            fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
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
                data: (BASE64.encode(data), "base64".into()),
                owner: owner.to_string(),
                executable,
                rent_epoch: 0,
                space: data.len(),
            },
        };
        let path = self.directory.join(format!("{address}.json"));
        let mut file_bytes = serde_json::to_vec_pretty(&output)?;
        file_bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| Error::new(format!("create {}: {error}", path.display())))?;
        file.write_all(&file_bytes)?;
        file.sync_all()?;
        self.accounts.insert(
            label,
            GenesisAccountPin {
                address: address.to_string(),
                owner: owner.to_string(),
                lamports,
                data_len: data.len(),
                data_sha256: hex(&sha256_bytes(data)),
                account_sha256: account_sha256_v1(owner, lamports, executable, 0, data)?,
                json_file_sha256: hex(&sha256_bytes(&file_bytes)),
            },
        );
        Ok(())
    }

    /// Materialize one role's Loader V3 pair from the exact `ProgramData`
    /// image its release was decoded from, so the genesis account and the
    /// authenticated release can never disagree about the deployment slot.
    fn upgradeable_program(
        &mut self,
        label: &str,
        program: Pubkey,
        programdata_bytes: &[u8],
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
        self.add(
            format!("loader.{label}.programdata"),
            programdata,
            bpf_loader_upgradeable::ID,
            Rent::default().minimum_balance(programdata_bytes.len()),
            programdata_bytes,
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

fn checked_set_role<'a>(
    set: &'a CheckedUpgradeSetPinV1,
    ordinal: usize,
    role: &str,
) -> Result<&'a CheckedUpgradeRolePinV1> {
    let pin = set
        .roles
        .get(ordinal)
        .ok_or_else(|| Error::new("checked Upgrade set omitted a permanent role"))?;
    if pin.role != role {
        return Err(Error::new(format!(
            "checked Upgrade set role {ordinal} is {:?}; expected {role}",
            pin.role
        )));
    }
    Ok(pin)
}

fn validate_checked_upgrade_set(
    args: &PrepareArgs,
    set: &CheckedUpgradeSetPinV1,
    deployments: [(&str, Pubkey, &Path, &str, &str, &RoleDeployment); 7],
) -> Result<()> {
    if args.record_publication != RecordPublicationV1::Transaction {
        return Err(Error::new(
            "checked permanent-devnet prepare requires transaction record publication",
        ));
    }
    crate::upgrade::reauthenticate_checked_deployment_set_pin(set)?;
    if set.schema != crate::upgrade::CHECKED_SET_PREPARE_SCHEMA
        || set.semantic_derivation != crate::upgrade::SEMANTIC_DERIVATION_V1
        || set.roles.len() != 7
    {
        return Err(Error::new(
            "checked Upgrade-set schema, semantic derivation, or role closure is invalid",
        ));
    }
    let retained = Pubkey::from_str(&set.retained_upgrade_authority)
        .map_err(|_| Error::new("checked Upgrade-set retained authority is not a Pubkey"))?;
    if retained != args.core_bootstrap_upgrade_authority {
        return Err(Error::new(
            "Core bootstrap authority differs from the checked Upgrade-set retained authority",
        ));
    }
    // The set journal's canonical order differs from release-plan display
    // order. Name both coordinates explicitly so no zip can silently swap a
    // role while preserving internally consistent caller data.
    let expected = [
        checked_set_role(set, 0, "registry")?,
        checked_set_role(set, 1, "rent")?,
        checked_set_role(set, 2, "custody")?,
        checked_set_role(set, 3, "resolution")?,
        checked_set_role(set, 4, "claims")?,
        checked_set_role(set, 5, "trading")?,
        checked_set_role(set, 6, "core")?,
    ];
    for (index, pin) in expected.iter().enumerate() {
        let carry = index < 2;
        let exact_tag = if carry {
            pin.disposition == CheckedDeploymentDispositionV1::CarryForward
                && pin.baseline_path.is_none()
                && pin.baseline_sha256.is_none()
                && pin.receipt_path.is_none()
                && pin.receipt_sha256.is_none()
                && pin.artifact_release_body_hex.is_some()
                && pin.artifact_release_id.is_some()
                && pin.carried_programdata_base64.is_some()
        } else {
            // Two shapes are admitted for a cut's own roles. An Upgrade carries a
            // baseline AND a receipt. An AlreadyCurrent role carries the baseline
            // -- it fixes the width the equality was judged at -- and NO receipt,
            // because no Upgrade exists or can exist for a payload that is already
            // the candidate. Neither may carry the carry-forward transport fields.
            match pin.disposition {
                CheckedDeploymentDispositionV1::Upgrade => {
                    pin.baseline_path.is_some()
                        && pin.baseline_sha256.is_some()
                        && pin.carries_no_transport_fields()
                        && pin.receipt_path.is_some()
                        && pin.receipt_sha256.is_some()
                }
                // The baseline and the absent receipt are the shared rule; the
                // transport fields are this row kind's own conjunct.
                CheckedDeploymentDispositionV1::AlreadyCurrent => {
                    pin.already_current_closure().holds() && pin.carries_no_transport_fields()
                }
                CheckedDeploymentDispositionV1::CarryForward => false,
            }
        };
        if !exact_tag {
            return Err(Error::new(format!(
                "checked deployment-set role {} violates its exact Upgrade/CarryForward field closure",
                pin.role
            )));
        }
    }
    for ((label, program, elf, candidate_sha, semantic, deployment), pin) in
        deployments.into_iter().zip(expected)
    {
        let canonical_elf = fs::canonicalize(elf)?;
        if pin.role != label
            || pin.program_id != program.to_string()
            || pin.programdata_id != programdata(program).to_string()
            || pin.checked_candidate_elf_path != canonical_elf.display().to_string()
            || pin.checked_candidate_elf_sha256 != candidate_sha
            || pin.semantic_release_id != semantic
            || deployment.source != DeploymentSourceV1::ObservedAccount
            || pin.live_elf_sha256 != hex(&deployment.live_elf_sha256)
            || pin.deployment_slot != deployment.deployment_slot
            || pin.programdata_account_sha256 != hex(&sha256_bytes(&deployment.image))
            || deployment.upgrade_authority != Some(retained.to_bytes())
        {
            return Err(Error::new(format!(
                "{label} prepare facts differ from the authenticated Upgrade-set role; raw caller substitution is refused"
            )));
        }
        hex32(&pin.programdata_account_sha256).map_err(|_| {
            Error::new(format!(
                "{label} checked Upgrade receipt ProgramData account digest is invalid"
            ))
        })?;
    }
    Ok(())
}

fn validate_carried_infrastructure_projection(
    set: &CheckedUpgradeSetPinV1,
    registry: ReleaseFacts,
    rent: ReleaseFacts,
    profile_address: Pubkey,
    profile_bytes: &[u8],
) -> Result<()> {
    let registry_pin = checked_set_role(set, 0, "registry")?;
    let rent_pin = checked_set_role(set, 1, "rent")?;
    for (label, pin, facts) in [
        ("registry", registry_pin, registry),
        ("rent", rent_pin, rent),
    ] {
        let expected_body = pin
            .artifact_release_body_hex
            .as_deref()
            .ok_or_else(|| Error::new(format!("CarryForward {label} omitted artifact body")))?;
        let expected_id = pin
            .artifact_release_id
            .as_deref()
            .ok_or_else(|| Error::new(format!("CarryForward {label} omitted artifact ID")))?;
        if hex(&facts.release.to_bytes()) != expected_body
            || hex(facts.id.as_bytes()) != expected_id
        {
            return Err(Error::new(format!(
                "checked prepare would replace the carried {label} ArtifactRelease instead of reusing it byte-for-byte"
            )));
        }
    }
    let carry = &set.infrastructure_carry_forward;
    if profile_address.to_string() != carry.profile_address
        || hex(profile_bytes) != carry.profile_body_hex
        || hex(&sha256_bytes(profile_bytes)) != carry.profile_body_sha256
    {
        return Err(Error::new(
            "checked prepare would replace the carried singleton infrastructure profile instead of reusing it byte-for-byte",
        ));
    }
    Ok(())
}

impl ReleaseFacts {
    fn binding(self) -> ExecutionRoleBindingV1 {
        ExecutionRoleBindingV1::new(self.release.program(), self.id)
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn prepare(args: PrepareArgs) -> Result<SuccessorPlan> {
    prepare_inner(args, None)
}

pub(crate) fn prepare_checked_local_mutable(
    args: PrepareArgs,
    gate: &crate::local_mutable::CheckedLocalMutableGateInputV1,
) -> Result<SuccessorPlan> {
    prepare_inner(args, Some(gate))
}

fn prepare_inner(
    args: PrepareArgs,
    checked_local_mutable_gate: Option<&crate::local_mutable::CheckedLocalMutableGateInputV1>,
) -> Result<SuccessorPlan> {
    if checked_local_mutable_gate.is_some() && args.checked_upgrade_set.is_some() {
        return Err(Error::new(
            "a plan cannot mix checked localhost genesis evidence with permanent-devnet Upgrade evidence",
        ));
    }
    if checked_local_mutable_gate.is_some() {
        if args.record_publication != RecordPublicationV1::Transaction {
            return Err(Error::new(
                "checked localhost mutable preparation requires transaction record publication",
            ));
        }
        for (role, deployment) in [
            ("registry", &args.deployments.registry),
            ("rent", &args.deployments.rent_credit),
            ("custody", &args.deployments.custody),
            ("resolution", &args.deployments.resolution),
            ("claims", &args.deployments.claims),
            ("trading", &args.deployments.trading),
            ("core", &args.deployments.core),
        ] {
            if deployment.observed_programdata.is_none()
                || deployment.observed_programdata_bytes.is_some()
                || deployment.expected_upgrade_authority
                    != Some(args.core_bootstrap_upgrade_authority)
                || deployment.genesis_deployment_slot != 0
            {
                return Err(Error::new(format!(
                    "checked localhost {role} must come from one exact observed ProgramData file under the retained authority"
                )));
            }
        }
    }
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

    // Observe first, mint second. Every role's deployment slot is decoded out
    // of a ProgramData account image before a single release body exists,
    // because a record's coordinate, the release-set digest, the activation
    // PDA and the infrastructure profile are all downstream of that slot.
    // §3.0 of the deploy runbook states the same ordering as a protocol fact:
    // deploy -> revoke -> observe -> mint bodies -> publish.
    let registry_deployment =
        role_deployment("Registry", &registry_elf, None, &args.deployments.registry)?;
    let core_deployment = role_deployment(
        "Core",
        &core_elf,
        Some(args.core_bootstrap_upgrade_authority),
        &args.deployments.core,
    )?;
    let claims_deployment = role_deployment("Claims", &claims_elf, None, &args.deployments.claims)?;
    let trading_deployment =
        role_deployment("Trading", &trading_elf, None, &args.deployments.trading)?;
    let resolution_deployment = role_deployment(
        "Resolution",
        &resolution_elf,
        None,
        &args.deployments.resolution,
    )?;
    let custody_deployment =
        role_deployment("Custody", &custody_elf, None, &args.deployments.custody)?;
    let rent_deployment =
        role_deployment("RentCredit", &rent_elf, None, &args.deployments.rent_credit)?;

    if let Some(set) = &args.checked_upgrade_set {
        validate_checked_upgrade_set(
            &args,
            set,
            [
                (
                    "registry",
                    args.registry_program,
                    &args.registry_elf,
                    &args.registry_sha256,
                    &args.registry_semantic_release_id,
                    &registry_deployment,
                ),
                (
                    "rent",
                    args.rent_credit_program,
                    &args.rent_credit_elf,
                    &args.rent_credit_sha256,
                    &args.rent_credit_semantic_release_id,
                    &rent_deployment,
                ),
                (
                    "custody",
                    args.custody_program,
                    &args.custody_elf,
                    &args.custody_sha256,
                    &args.custody_semantic_release_id,
                    &custody_deployment,
                ),
                (
                    "resolution",
                    args.resolution_program,
                    &args.resolution_elf,
                    &args.resolution_sha256,
                    &args.resolution_semantic_release_id,
                    &resolution_deployment,
                ),
                (
                    "claims",
                    args.claims_program,
                    &args.claims_elf,
                    &args.claims_sha256,
                    &args.claims_semantic_release_id,
                    &claims_deployment,
                ),
                (
                    "trading",
                    args.trading_program,
                    &args.trading_elf,
                    &args.trading_sha256,
                    &args.trading_semantic_release_id,
                    &trading_deployment,
                ),
                (
                    "core",
                    args.core_program,
                    &args.core_elf,
                    &args.core_sha256,
                    &args.core_semantic_release_id,
                    &core_deployment,
                ),
            ],
        )?;
    }

    // What a release BINDS is the authority its ProgramData carries AT
    // ACTIVATION, which is not always the authority observed now. Nothing in
    // this campaign touches the six non-Core loaders, so for them the
    // observation already is the activation state and the policy follows it.
    //
    // Core is the one role whose answer depends on WHICH campaign will run
    // this plan, because only one of the two revokes it. The deployment source
    // is that discriminator, and it is not a proxy for it — it IS it:
    //
    // * `GenesisInstall` is the supervised `run` path and nothing else. That
    //   supervisor materializes Core's ProgramData itself under an authority it
    //   generated in memory this second, and revokes it before it activates
    //   anything. Core's release therefore binds None, exactly as it always
    //   has. A real cluster's Core could not carry that key even in principle:
    //   it is fresh per run and never leaves the process.
    // * `ObservedAccount` is a cluster this tool did not create, which is the
    //   external driver's substrate. That campaign has NO revoke stage —
    //   decision 0012 retired it, and the slot pin carries the soundness the
    //   revocation used to — so the release must bind what the account
    //   actually carries, like every other role.
    //
    // The plan then says which of the two it is out loud, below, in
    // `release_recognition_requires_revoke`, and the `run` spec validator
    // refuses a plan whose answer is `false`. A 0012 mutable-substrate plan is
    // not something the local supervisor may run, and it fails closed saying so.
    let core_activation_upgrade_authority = match core_deployment.source {
        DeploymentSourceV1::GenesisInstall => None,
        DeploymentSourceV1::ObservedAccount => core_deployment.upgrade_authority,
    };

    let registry = release_facts(
        args.registry_program,
        hex32(&args.registry_semantic_release_id)?,
        registry_deployment.live_elf_sha256,
        registry_deployment.deployment_slot,
        registry_deployment.upgrade_authority,
    )?;
    let core = release_facts(
        args.core_program,
        hex32(&args.core_semantic_release_id)?,
        core_deployment.live_elf_sha256,
        core_deployment.deployment_slot,
        core_activation_upgrade_authority,
    )?;
    let claims = release_facts(
        args.claims_program,
        hex32(&args.claims_semantic_release_id)?,
        claims_deployment.live_elf_sha256,
        claims_deployment.deployment_slot,
        claims_deployment.upgrade_authority,
    )?;
    let trading = release_facts(
        args.trading_program,
        hex32(&args.trading_semantic_release_id)?,
        trading_deployment.live_elf_sha256,
        trading_deployment.deployment_slot,
        trading_deployment.upgrade_authority,
    )?;
    let resolution_semantic = hex32(&args.resolution_semantic_release_id)?;
    if resolution_semantic != RESOLUTION_CONTROLLER_RELEASE_ID_V7 {
        return Err(Error::new(
            "Resolution semantic release ID does not match the selected executable contract",
        ));
    }
    let resolution = release_facts(
        args.resolution_program,
        resolution_semantic,
        resolution_deployment.live_elf_sha256,
        resolution_deployment.deployment_slot,
        resolution_deployment.upgrade_authority,
    )?;
    let custody = release_facts(
        args.custody_program,
        hex32(&args.custody_semantic_release_id)?,
        custody_deployment.live_elf_sha256,
        custody_deployment.deployment_slot,
        custody_deployment.upgrade_authority,
    )?;
    let rent = release_facts(
        args.rent_credit_program,
        hex32(&args.rent_credit_semantic_release_id)?,
        rent_deployment.live_elf_sha256,
        rent_deployment.deployment_slot,
        rent_deployment.upgrade_authority,
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
    if let Some(set) = &args.checked_upgrade_set {
        validate_carried_infrastructure_projection(
            set,
            registry,
            rent,
            infrastructure_address,
            &infrastructure_bytes,
        )?;
    }

    let provider = local_validator_release_v1()
        .map_err(|error| Error::new(format!("local Pyth release projection: {error:?}")))?;
    let provider_release_bytes = provider.release().to_bytes();
    let provider_release_id = sha256_bytes(&provider_release_bytes);
    let provider = provider.release();

    let mut writer = PlanWriter::new(args.account_dir.clone())?;
    for (label, program, deployment) in [
        ("registry", args.registry_program, &registry_deployment),
        ("claims", args.claims_program, &claims_deployment),
        ("trading", args.trading_program, &trading_deployment),
        (
            "resolution",
            args.resolution_program,
            &resolution_deployment,
        ),
        ("custody", args.custody_program, &custody_deployment),
        ("rent-credit", args.rent_credit_program, &rent_deployment),
        ("core", args.core_program, &core_deployment),
    ] {
        writer.upgradeable_program(label, program, &deployment.image)?;
    }
    for (label, program, expected_programdata, deployment_slot, expected_elf_sha256, elf) in [
        (
            "pyth-receiver",
            Pubkey::new_from_array(provider.receiver_program()),
            Pubkey::new_from_array(provider.receiver_programdata()),
            provider.receiver_deployment_slot(),
            provider.receiver_abi_id(),
            LOCAL_PYTH_RECEIVER_ELF,
        ),
        (
            "pyth-router",
            Pubkey::new_from_array(provider.router_program()),
            Pubkey::new_from_array(provider.router_programdata()),
            provider.router_deployment_slot(),
            provider.router_abi_id(),
            LOCAL_PYTH_ROUTER_ELF,
        ),
    ] {
        let derived_programdata = programdata(program);
        if deployment_slot != 0
            || expected_programdata != derived_programdata
            || sha256_bytes(elf) != expected_elf_sha256
        {
            return Err(Error::new(format!(
                "local {label} release does not own the exact slot-zero fixture Loader pair"
            )));
        }
        let image = loader_programdata_bytes(elf, deployment_slot, None);
        let view = ProgramDataV3View::parse(&image).map_err(|error| {
            Error::new(format!(
                "local {label} prepared ProgramData is not Loader-v3: {error:?}"
            ))
        })?;
        if view.deployment_slot() != 0
            || view.upgrade_authority().is_some()
            || sha256_bytes(view.elf()) != expected_elf_sha256
        {
            return Err(Error::new(format!(
                "local {label} prepared ProgramData is not exact slot-zero tag-None fixture evidence"
            )));
        }
        writer.upgradeable_program(label, program, &image)?;
    }

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

    let registry_pin = pin(&args, ProgramKind::Registry, registry, &registry_deployment);
    let core_pin = pin(&args, ProgramKind::Core, core, &core_deployment);
    let claims_pin = pin(&args, ProgramKind::Claims, claims, &claims_deployment);
    let trading_pin = pin(&args, ProgramKind::Trading, trading, &trading_deployment);
    let resolution_pin = pin(
        &args,
        ProgramKind::Resolution,
        resolution,
        &resolution_deployment,
    );
    let custody_pin = pin(&args, ProgramKind::Custody, custody, &custody_deployment);
    let rent_pin = pin(&args, ProgramKind::Rent, rent, &rent_deployment);

    let checked_local_mutable_set = checked_local_mutable_gate
        .map(|gate| {
            crate::local_mutable::build_checked_local_mutable_set_v1(
                gate,
                args.core_bootstrap_upgrade_authority,
                &hex(&release_set_id),
                [
                    ("registry", &registry_pin),
                    ("rent", &rent_pin),
                    ("custody", &custody_pin),
                    ("resolution", &resolution_pin),
                    ("claims", &claims_pin),
                    ("trading", &trading_pin),
                    ("core", &core_pin),
                ],
            )
        })
        .transpose()?;
    writer.sync()?;
    let plan = SuccessorPlan {
        schema: "dclutch-local-successor-infrastructure-plan-v2".into(),
        genesis_boundary: if checked_local_mutable_gate.is_some() {
            vec![
                "Genesis installs seven exact checked-release Loader-v3 Program/ProgramData pairs under one disposable retained authority at canonical local slots 1 through 7, plus the two exact immutable slot-zero Pyth provider Loader pairs. Nothing else.".into(),
                "Every infrastructure record body, activation, founding, participant, trade, resolution, payout, and retirement remains a real localhost transaction.".into(),
            ]
        } else { match publication {
            RecordPublicationV1::Genesis => vec![
                "Genesis fixtures are six immutable dClutch Loader-v3 programs, one authority-bearing pre-init Core Loader-v3 program with the same exact ELF, two immutable slot-zero Pyth provider Loader pairs, and finalized Registry record bodies.".into(),
                "Registry activation, Core infrastructure initialization, RentCredit creation, Found, Source creation, funding, and resolution are not genesis-prepared.".into(),
            ],
            RecordPublicationV1::Transaction => vec![
                "Genesis fixtures are six immutable dClutch Loader-v3 programs, one authority-bearing pre-init Core Loader-v3 program with the same exact ELF, and two immutable slot-zero Pyth provider Loader pairs. Nothing else. No protocol state exists at genesis.".into(),
                "Every infrastructure record body, Registry activation, Core infrastructure initialization, RentCredit creation, Found, Source creation, funding, and resolution is a real transaction. This is the shape a cluster can reach.".into(),
            ],
        }},
        bootstrap_order: if checked_local_mutable_gate.is_some() {
            vec![
                "Re-authenticate the exact checked-release gate and all seven mutable Loader pairs before any key read.".into(),
                "Publish the exact seven ArtifactRelease records and singleton infrastructure profile through Registry transactions.".into(),
                "Activate the five-role exact-authority ExecutionReleaseSet without revoking the disposable local Upgrade authority.".into(),
                "Execute DCLTGMF3 founding, participant admission, Direct, resolution, payout, and retirement through their accepted exterior callers.".into(),
            ]
        } else {
            vec![
                "Authenticate immutable Registry/Rent and remaining role Loader facts; authenticate Core ELF under its ephemeral exact upgrade authority.".into(),
                "Use that in-memory Core upgrade-authority signer to initialize the sole 144-byte ProtocolInfrastructureProfile from exact Registry and Rent artifact records.".into(),
                "Revoke Core upgrade authority to None through Loader-v3 and verify the exact tag-None fixed-offset poststate, including Loader-retained inactive authority bytes, before release recognition.".into(),
                "Activate the five-role immutable ExecutionReleaseSet through Registry, then create RentCredit and execute canonical Found31.".into(),
                "Create and fund Source through Core effects before consuming the captured signed Pyth PriceUpdate through Resolution.".into(),
            ]
        },
        execution_blocker: "Infrastructure activation is executable in one supervised process. LifecycleRentCreditV2 and Found31 remain behind an explicit market-specific input bundle: finalized Realm, ProductV3 basis/result-domain, portfolio, resolution, execution-manifest, and lifecycle-policy records plus exact generation, immutable refund wallet, initial Hoard principal, and lifecycle-rent funding.".into(),
        account_dir: args.account_dir.display().to_string(),
        registry: registry_pin,
        core: core_pin,
        claims: claims_pin,
        trading: trading_pin,
        resolution: resolution_pin,
        custody: custody_pin,
        rent_credit: rent_pin,
        activation: activation.to_string(),
        release_set_id: hex(&release_set_id),
        core_bootstrap: CoreBootstrapPin {
            upgrade_authority: args.core_bootstrap_upgrade_authority.to_string(),
            genesis_programdata_sha256: hex(&sha256_bytes(&core_deployment.image)),
            post_revoke_programdata_sha256: hex(&sha256_bytes(&programdata_bytes_after_revoke(
                &core_deployment.image,
            )?)),
            // True exactly when Core's release binds None while its ProgramData
            // still carries an authority — that is, when a revocation stands
            // between the substrate and the release being recognizable. The
            // supervised `run` path is that case and always has been. The 0012
            // observed substrate is not: nothing revokes it, and the slot pin
            // carries the soundness instead. `run` refuses a plan that says
            // false, which is how the local supervisor declines to drive a
            // mutable substrate it has no revoke stage for.
            release_recognition_requires_revoke: core_activation_upgrade_authority.is_none(),
        },
        checked_upgrade_set: args.checked_upgrade_set,
        checked_local_mutable_set,
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
    let mut plan_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.plan_path)?;
    plan_file.write_all(&bytes)?;
    plan_file.sync_all()?;
    if let Some(parent) = args.plan_path.parent() {
        fs::File::open(parent)?.sync_all()?;
    }
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

fn pin(
    args: &PrepareArgs,
    kind: ProgramKind,
    facts: ReleaseFacts,
    deployment: &RoleDeployment,
) -> ProgramPin {
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
        checked_candidate_elf_path: elf.display().to_string(),
        checked_candidate_elf_sha256: elf_sha.clone(),
        live_elf_sha256: hex(&deployment.live_elf_sha256),
        live_elf_padding_bytes: deployment.live_elf_padding_bytes,
        semantic_release_id: semantic.clone(),
        artifact_release_id: hex(facts.id.as_bytes()),
        // The authority the RELEASE binds, which for a revoked or genesis role
        // is None exactly as it has always been, and for decision 0012's
        // mutable substrate is the key an Upgrade must be signed by.
        upgrade_authority: facts
            .release
            .upgrade_authority()
            .map(|bytes| Pubkey::new_from_array(bytes).to_string()),
        deployment_slot: deployment.deployment_slot,
        deployment_source: deployment.source.as_str().into(),
        programdata_sha256: hex(&sha256_bytes(&deployment.image)),
    }
}

/// Mint one role's `ArtifactReleaseV1` from facts decoded off its deployment.
///
/// The upgrade policy is DERIVED from the authority that role's `ProgramData`
/// will carry at activation, never asserted: an authority present means
/// `ExactAuthority` naming exactly it, absent means `Immutable`. Decision 0012
/// is what makes the distinction load-bearing — a devnet substrate iterated by
/// `Upgrade` must publish releases that say so, and the slot pin (not the
/// revocation) is what keeps them sound. A revoked loader still mints
/// `Immutable` with no authority, byte-for-byte as before.
fn release_facts(
    program: Pubkey,
    semantic_release: [u8; 32],
    elf_sha256: [u8; 32],
    deployment_slot: u64,
    upgrade_authority: Option<[u8; 32]>,
) -> Result<ReleaseFacts> {
    let upgrade_policy = if upgrade_authority.is_some() {
        ArtifactUpgradePolicyV1::ExactAuthority
    } else {
        ArtifactUpgradePolicyV1::Immutable
    };
    let release = ArtifactReleaseV1::new(
        program_identity(program)?,
        program_identity(bpf_loader_upgradeable::ID)?,
        programdata(program).to_bytes(),
        content_id(semantic_release)?,
        elf_sha256,
        deployment_slot,
        upgrade_policy,
        upgrade_authority,
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
    if args.checked_upgrade_set.is_none()
        && crate::upgrade::is_permanent_devnet_program_set(&[
            args.registry_program,
            args.rent_credit_program,
            args.custody_program,
            args.resolution_program,
            args.claims_program,
            args.trading_program,
            args.core_program,
        ])
    {
        return Err(Error::new(
            "the permanent devnet program set requires --upgrade-set-journal; raw release facts cannot authorize activation",
        ));
    }
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
    for (label, input) in [
        ("Registry", &args.deployments.registry),
        ("Core", &args.deployments.core),
        ("Claims", &args.deployments.claims),
        ("Trading", &args.deployments.trading),
        ("Resolution", &args.deployments.resolution),
        ("Custody", &args.deployments.custody),
        ("RentCredit", &args.deployments.rent_credit),
    ] {
        if let Some(path) = input.observed_programdata.as_deref()
            && (!path.is_absolute() || !path.is_file())
        {
            return Err(Error::new(format!(
                "{label} observed ProgramData must be an existing absolute regular file"
            )));
        }
        match (
            input.observed_programdata.is_some() || input.observed_programdata_bytes.is_some(),
            input.expected_live_elf_sha256.as_deref(),
        ) {
            (true, Some(digest)) => {
                hex32(digest).map_err(|_| {
                    Error::new(format!(
                        "{label} live ELF SHA-256 must be 64 lowercase hexadecimal characters"
                    ))
                })?;
            }
            (true, None) => {
                return Err(Error::new(format!(
                    "{label} observed ProgramData requires its complete live ELF SHA-256"
                )));
            }
            (false, Some(_)) => {
                return Err(Error::new(format!(
                    "{label} live ELF SHA-256 is only valid with observed ProgramData"
                )));
            }
            (false, None) => {}
        }
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

pub(crate) fn loader_programdata_bytes(
    elf: &[u8],
    deployment_slot: u64,
    upgrade_authority: Option<Pubkey>,
) -> Vec<u8> {
    let mut bytes = vec![0_u8; 45];
    bytes[..4].copy_from_slice(&3_u32.to_le_bytes());
    bytes[4..12].copy_from_slice(&deployment_slot.to_le_bytes());
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
///
/// This works from the account image rather than from `(elf, slot, authority)`
/// precisely *because* of that residue: a real revoked account carries a former
/// authority no triple can regenerate, so an observed poststate has to be
/// derived from the observed prestate.
pub(crate) fn programdata_bytes_after_revoke(image: &[u8]) -> Result<Vec<u8>> {
    let mut bytes = image.to_vec();
    match bytes.get_mut(12) {
        Some(tag) => *tag = 0,
        None => return Err(Error::new("ProgramData image is shorter than its metadata")),
    }
    Ok(bytes)
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
        let bytes = loader_programdata_bytes(b"\x7fELFbody", 0, None);
        assert_eq!(bytes.len(), 53);
        assert_eq!(&bytes[..4], &3_u32.to_le_bytes());
        assert_eq!(&bytes[4..12], &0_u64.to_le_bytes());
        assert_eq!(bytes[12], 0);
        assert!(bytes[13..45].iter().all(|byte| *byte == 0));
        assert_eq!(&bytes[45..], b"\x7fELFbody");
    }

    #[test]
    fn provider_genesis_bodies_are_slot_zero_tag_none_not_cli_none() {
        for (elf, expected_immutable, expected_cli_none) in [
            (
                LOCAL_PYTH_RECEIVER_ELF,
                "63d003102c1bd48be1be24706734813094747eee82edbe066c2042474f64004e",
                "c9e4e286f11f86d95478bb5c89496f97d4fb4471a8b67c8dc372248ec0b45f82",
            ),
            (
                LOCAL_PYTH_ROUTER_ELF,
                "04c1327626a93e09c4c833aa43b316f472d697e2fff0e5c029aaf343b83252c4",
                "ee21b7f378604d72d9412926650b12c6d9281f7559ed3fe671ce31bf1aeef9e0",
            ),
        ] {
            let immutable = loader_programdata_bytes(elf, 0, None);
            let cli_none = loader_programdata_bytes(elf, 0, Some(Pubkey::default()));
            assert_eq!(hex(&sha256_bytes(&immutable)), expected_immutable);
            assert_eq!(hex(&sha256_bytes(&cli_none)), expected_cli_none);
            assert_eq!(immutable[12], 0);
            assert_eq!(cli_none[12], 1);
            assert!(immutable[13..45].iter().all(|byte| *byte == 0));
            assert!(cli_none[13..45].iter().all(|byte| *byte == 0));
            assert_ne!(immutable, cli_none);
        }
    }

    #[test]
    fn checked_candidate_padding_accepts_only_an_exact_prefix_and_zero_suffix() {
        let candidate = b"\x7fELFchecked";
        assert_eq!(
            checked_candidate_padding_v1(candidate, candidate).expect("exact live tail"),
            0
        );
        let mut padded = candidate.to_vec();
        padded.extend_from_slice(&[0; 19]);
        assert_eq!(
            checked_candidate_padding_v1(candidate, &padded).expect("zero-padded live tail"),
            19
        );

        let mut nonzero_suffix = padded.clone();
        *nonzero_suffix.last_mut().expect("suffix") = 1;
        assert!(checked_candidate_padding_v1(candidate, &nonzero_suffix).is_err());

        let mut substituted_prefix = padded;
        substituted_prefix[4] ^= 1;
        assert!(checked_candidate_padding_v1(candidate, &substituted_prefix).is_err());
        assert!(
            checked_candidate_padding_v1(candidate, &candidate[..candidate.len() - 1]).is_err()
        );
        let mut candidate_and_padding_swapped = candidate.to_vec();
        candidate_and_padding_swapped.extend_from_slice(&[0; 8]);
        assert!(
            checked_candidate_padding_v1(&candidate_and_padding_swapped, candidate).is_err(),
            "the padded live payload cannot masquerade as the raw candidate"
        );
        assert!(checked_candidate_padding_v1(&[], &[]).is_err());
    }

    #[test]
    fn observed_payload_keeps_both_digests_and_binds_the_complete_live_tail() {
        let observations =
            std::env::temp_dir().join(format!("dclutch-successor-pad-{}", Pubkey::new_unique()));
        let former = Pubkey::new_unique();
        let raw = test_elf(1);
        let mut live = raw.to_vec();
        live.extend_from_slice(&[0; 23]);
        let image = revoked_account_image(&live, 808, former);
        let mut deployments = RoleDeploymentsV1::default();
        observe(
            &mut deployments.registry,
            &observations,
            "registry-padded.bin",
            &image,
        );

        let (plan, root) = prepared_plan_with(RecordPublicationV1::Transaction, deployments);
        let plan = plan.expect("zero-padded observed payload prepares");
        assert_eq!(
            plan.schema,
            "dclutch-local-successor-infrastructure-plan-v2"
        );
        assert_eq!(
            plan.registry.checked_candidate_elf_sha256,
            hex(&sha256_bytes(&raw))
        );
        assert_eq!(plan.registry.live_elf_sha256, hex(&sha256_bytes(&live)));
        assert_eq!(plan.registry.live_elf_padding_bytes, 23);
        assert_ne!(
            plan.registry.checked_candidate_elf_sha256,
            plan.registry.live_elf_sha256
        );
        let release = ArtifactReleaseV1::decode(
            &hex32_bytes(&plan.records["registry_artifact_release"].body_hex)
                .expect("artifact body"),
        )
        .expect("artifact release");
        assert_eq!(release.elf_digest(), sha256_bytes(&live));
        let serialized: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("plan.json")).expect("serialized infrastructure plan"),
        )
        .expect("plan JSON");
        assert_eq!(
            serialized["registry"]["checked_candidate_elf_sha256"],
            plan.registry.checked_candidate_elf_sha256
        );
        assert_eq!(
            serialized["registry"]["live_elf_sha256"],
            plan.registry.live_elf_sha256
        );
        assert_eq!(serialized["registry"]["live_elf_padding_bytes"], 23);

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&observations);
    }

    #[test]
    fn observed_payload_refuses_single_digest_ambiguity_and_wrong_live_digest() {
        let observations = std::env::temp_dir().join(format!(
            "dclutch-successor-pad-refuse-{}",
            Pubkey::new_unique()
        ));
        let former = Pubkey::new_unique();
        let raw = test_elf(1);
        let mut live = raw.to_vec();
        live.extend_from_slice(&[0; 11]);
        let image = revoked_account_image(&live, 809, former);

        let mut ambiguous = RoleDeploymentsV1::default();
        ambiguous.registry.observed_programdata = Some(write_observed(
            &observations,
            "registry-ambiguous.bin",
            &image,
        ));
        let (result, root) = prepared_plan_with(RecordPublicationV1::Transaction, ambiguous);
        let refusal = result.expect_err("old single-digest observation must refuse");
        assert!(refusal.to_string().contains("live ELF SHA-256"));
        let _ = fs::remove_dir_all(&root);

        let mut raw_digest_reused = RoleDeploymentsV1::default();
        raw_digest_reused.registry.observed_programdata = Some(write_observed(
            &observations,
            "registry-wrong-live-digest.bin",
            &image,
        ));
        raw_digest_reused.registry.expected_live_elf_sha256 = Some(hex(&sha256_bytes(&raw)));
        let (result, root) =
            prepared_plan_with(RecordPublicationV1::Transaction, raw_digest_reused);
        let refusal = result.expect_err("raw digest cannot stand in for padded live digest");
        assert!(
            refusal
                .to_string()
                .contains("complete live ProgramData ELF")
        );
        let _ = fs::remove_dir_all(&root);

        let _ = fs::remove_dir_all(&observations);
    }

    #[test]
    fn pre_init_core_and_real_loader_revoke_differ_only_in_authority_tag() {
        let authority = Pubkey::new_unique();
        let initial = loader_programdata_bytes(b"\x7fELFbody", 0, Some(authority));
        let revoked = programdata_bytes_after_revoke(&initial).expect("revoke poststate");
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
        let registry = release_facts(Pubkey::new_unique(), [1; 32], [2; 32], 0, None)
            .expect("Registry release");
        let rent =
            release_facts(Pubkey::new_unique(), [3; 32], [4; 32], 0, None).expect("Rent release");
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
        let (plan, root) = prepared_plan_with(publication, RoleDeploymentsV1::default());
        (plan.expect("prepare infrastructure plan"), root)
    }

    fn prepared_plan_with(
        publication: RecordPublicationV1,
        deployments: RoleDeploymentsV1,
    ) -> (Result<SuccessorPlan>, PathBuf) {
        prepared_plan_with_resolution_semantic(
            publication,
            deployments,
            RESOLUTION_CONTROLLER_RELEASE_ID_V7,
        )
    }

    fn prepared_plan_with_resolution_semantic(
        publication: RecordPublicationV1,
        deployments: RoleDeploymentsV1,
        resolution_semantic_release_id: [u8; 32],
    ) -> (Result<SuccessorPlan>, PathBuf) {
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
            resolution_semantic_release_id: hex(&resolution_semantic_release_id),
            custody_program: program(6),
            custody_elf,
            custody_sha256,
            custody_semantic_release_id: hex(&[16; 32]),
            rent_credit_program: program(7),
            rent_credit_elf,
            rent_credit_sha256,
            rent_credit_semantic_release_id: hex(&[17; 32]),
            checked_upgrade_set: None,
            record_publication: publication,
            deployments,
        });
        (plan, root)
    }

    #[test]
    fn resolution_v5_is_refused_after_the_v6_prepared_funding_migration() {
        let (result, root) = prepared_plan_with_resolution_semantic(
            RecordPublicationV1::Transaction,
            RoleDeploymentsV1::default(),
            dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V5,
        );
        let error = result.expect_err("Resolution V5 must not author a V6 successor plan");
        assert!(
            error
                .to_string()
                .contains("Resolution semantic release ID does not match"),
            "unexpected refusal: {error}"
        );
        fs::remove_dir_all(root).expect("remove scoped test root");
    }

    /// A ProgramData account body in the shape a real cluster leaves behind
    /// after `set-upgrade-authority --final`: authority tag zero with the
    /// former authority still sitting in bytes 13..45, and a nonzero slot.
    fn revoked_account_image(elf: &[u8], slot: u64, former_authority: Pubkey) -> Vec<u8> {
        programdata_bytes_after_revoke(&loader_programdata_bytes(elf, slot, Some(former_authority)))
            .expect("revoked account image")
    }

    fn write_observed(root: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        fs::create_dir_all(root).expect("observation directory");
        let path = root.join(name);
        fs::write(&path, bytes).expect("write observed account");
        path
    }

    fn observe(input: &mut RoleDeploymentInputV1, root: &Path, name: &str, programdata: &[u8]) {
        input.observed_programdata = Some(write_observed(root, name, programdata));
        let live = programdata
            .get(dclutch_registry_svm::LOADER_V3_PROGRAMDATA_METADATA_BYTES..)
            .unwrap_or(&[]);
        input.expected_live_elf_sha256 = Some(hex(&sha256_bytes(live)));
    }

    fn test_elf(tag: u8) -> [u8; 5] {
        [0x7f, b'E', b'L', b'F', tag]
    }

    /// Blocker A: an observed ProgramData account mints the slot it actually
    /// carries, and the retained former-authority bytes a real revocation
    /// leaves behind do not make it un-mintable.
    #[test]
    fn an_observed_programdata_account_mints_its_own_deployment_slot() {
        let observations =
            std::env::temp_dir().join(format!("dclutch-successor-obs-{}", Pubkey::new_unique()));
        let former = Pubkey::new_unique();
        let mut deployments = RoleDeploymentsV1::default();
        observe(
            &mut deployments.registry,
            &observations,
            "registry.bin",
            &revoked_account_image(&test_elf(1), 488_712_345, former),
        );
        observe(
            &mut deployments.claims,
            &observations,
            "claims.bin",
            &revoked_account_image(&test_elf(3), 488_712_401, former),
        );
        let (plan, root) = prepared_plan_with(RecordPublicationV1::Transaction, deployments);
        let plan = plan.expect("observed deployments prepare");

        assert_eq!(plan.registry.deployment_slot, 488_712_345);
        assert_eq!(
            plan.registry.deployment_source,
            "observed-programdata-account"
        );
        assert_eq!(plan.claims.deployment_slot, 488_712_401);
        assert_eq!(plan.registry.live_elf_padding_bytes, 0);
        assert_eq!(
            plan.registry.checked_candidate_elf_sha256, plan.registry.live_elf_sha256,
            "an explicit no-padding observation keeps equal but separately named digests"
        );
        // Roles nobody observed keep the genesis-install shape, and say so.
        assert_eq!(plan.trading.deployment_slot, 0);
        assert_eq!(plan.trading.deployment_source, "genesis-install");

        // The slot is in the record body the chain will authenticate against,
        // at ArtifactReleaseV1's fixed offset 176.
        let body =
            hex32_bytes(&plan.records["registry_artifact_release"].body_hex).expect("record body");
        assert_eq!(
            u64::from_le_bytes(body[176..184].try_into().expect("slot bytes")),
            488_712_345
        );
        // The retained authority survives into the genesis image byte for byte
        // and the tag stays zero, which is the whole shape blocker B is about.
        let genesis = &plan.genesis_accounts["loader.registry.programdata"];
        assert_eq!(genesis.data_sha256, plan.registry.programdata_sha256);
        assert_eq!(
            genesis.data_sha256,
            hex(&sha256_bytes(&revoked_account_image(
                &test_elf(1),
                488_712_345,
                former
            )))
        );

        fs::remove_dir_all(&root).expect("remove scoped test root");
        fs::remove_dir_all(&observations).expect("remove observation root");
    }

    /// A deployment slot is load-bearing all the way down: moving one moves
    /// that role's record coordinate, the release-set digest, and the
    /// activation PDA. That is why §3.0 forces observe-then-mint.
    #[test]
    fn a_moved_deployment_slot_moves_every_coordinate_downstream_of_it() {
        let observations =
            std::env::temp_dir().join(format!("dclutch-successor-obs-{}", Pubkey::new_unique()));
        let mut deployments = RoleDeploymentsV1::default();
        deployments.core.genesis_deployment_slot = 531;
        let (moved, moved_root) = prepared_plan_with(RecordPublicationV1::Transaction, deployments);
        let moved = moved.expect("nonzero Core slot prepares");
        let (zero, zero_root) = prepared_plan(RecordPublicationV1::Transaction);

        assert_eq!(moved.core.deployment_slot, 531);
        assert_eq!(moved.core.deployment_source, "genesis-install");
        assert_ne!(
            moved.records["core_artifact_release"].raw,
            zero.records["core_artifact_release"].raw
        );
        assert_ne!(moved.release_set_id, zero.release_set_id);
        assert_ne!(moved.activation, zero.activation);
        // Registry and Rent were not moved, so the infrastructure profile is
        // untouched -- the blast radius is exactly the roles that moved.
        assert_eq!(
            moved.infrastructure_profile.body_sha256,
            zero.infrastructure_profile.body_sha256
        );

        fs::remove_dir_all(&moved_root).expect("remove scoped test root");
        fs::remove_dir_all(&zero_root).expect("remove scoped test root");
        let _ = fs::remove_dir_all(&observations);
    }

    /// Adversarial: an observation that does not describe the pinned artifact
    /// is refused at plan time, not by a chain refusal after the rent is gone.
    #[test]
    fn a_dishonest_observed_programdata_account_is_refused() {
        let observations =
            std::env::temp_dir().join(format!("dclutch-successor-obs-{}", Pubkey::new_unique()));
        let former = Pubkey::new_unique();

        // Still mutable: the release claims Immutable/None and activation
        // would refuse it on chain.
        let mut live_authority = RoleDeploymentsV1::default();
        observe(
            &mut live_authority.registry,
            &observations,
            "live.bin",
            &loader_programdata_bytes(&test_elf(1), 400, Some(former)),
        );
        let (result, root) = prepared_plan_with(RecordPublicationV1::Transaction, live_authority);
        assert!(result.is_err(), "a live upgrade authority must refuse");
        let _ = fs::remove_dir_all(&root);

        // A different program's ELF under this role's name.
        let mut wrong_elf = RoleDeploymentsV1::default();
        observe(
            &mut wrong_elf.registry,
            &observations,
            "wrong.bin",
            &revoked_account_image(&test_elf(9), 400, former),
        );
        let (result, root) = prepared_plan_with(RecordPublicationV1::Transaction, wrong_elf);
        assert!(result.is_err(), "a substituted ELF must refuse");
        let _ = fs::remove_dir_all(&root);

        // Not Loader-v3 shaped at all.
        let mut garbage = RoleDeploymentsV1::default();
        observe(
            &mut garbage.registry,
            &observations,
            "garbage.bin",
            &[0_u8; 64],
        );
        let (result, root) = prepared_plan_with(RecordPublicationV1::Transaction, garbage);
        assert!(result.is_err(), "a non-Loader account must refuse");
        let _ = fs::remove_dir_all(&root);

        // An observation is not something a caller gets to overwrite.
        let mut both = RoleDeploymentsV1::default();
        observe(
            &mut both.registry,
            &observations,
            "both.bin",
            &revoked_account_image(&test_elf(1), 400, former),
        );
        both.registry.genesis_deployment_slot = 7;
        let (result, root) = prepared_plan_with(RecordPublicationV1::Transaction, both);
        assert!(
            result.is_err(),
            "observing and fabricating a slot at once must refuse"
        );
        let _ = fs::remove_dir_all(&root);

        fs::remove_dir_all(&observations).expect("remove observation root");
    }

    /// Decision 0012, minting arm one: a role observed MUTABLE and DECLARED
    /// mutable mints `ExactAuthority` naming exactly the key the account
    /// carries, while a revoked sibling in the same plan keeps minting
    /// `Immutable`. The policy follows the account, not a flag: the declaration
    /// only buys the right to be checked against it.
    #[test]
    fn an_observed_mutable_programdata_mints_its_exact_upgrade_authority() {
        let observations =
            std::env::temp_dir().join(format!("dclutch-successor-obs-{}", Pubkey::new_unique()));
        let authority = Pubkey::new_unique();
        let former = Pubkey::new_unique();
        let mut deployments = RoleDeploymentsV1::default();
        observe(
            &mut deployments.registry,
            &observations,
            "registry-mutable.bin",
            &loader_programdata_bytes(&test_elf(1), 488_712_345, Some(authority)),
        );
        deployments.registry.expected_upgrade_authority = Some(authority);
        observe(
            &mut deployments.claims,
            &observations,
            "claims-revoked.bin",
            &revoked_account_image(&test_elf(3), 488_712_401, former),
        );
        let (plan, root) = prepared_plan_with(RecordPublicationV1::Transaction, deployments);
        let plan = plan.expect("a declared mutable observation prepares");

        assert_eq!(
            plan.registry.upgrade_authority,
            Some(authority.to_string()),
            "a mutable observation binds the authority it carries"
        );
        assert_eq!(plan.registry.deployment_slot, 488_712_345);
        assert_eq!(
            plan.claims.upgrade_authority, None,
            "a revoked observation still mints Immutable"
        );
        assert_eq!(
            plan.core.upgrade_authority, None,
            "Core binds its post-revocation state, whatever it carries now"
        );
        let _ = fs::remove_dir_all(&root);
        fs::remove_dir_all(&observations).expect("remove observation root");
    }

    /// Decision 0012, minting arm two: the declaration is a crosscheck, never a
    /// source. An undeclared mutable account refuses with the same text SMOKE-0
    /// got off real devnet bytes, a declaration that misses refuses the same
    /// way, a declaration against a fabricated genesis install refuses as a
    /// category error, and Core may not be declared away from the bootstrap
    /// authority the rest of the plan authenticates against.
    #[test]
    fn a_declaration_only_crosschecks_and_never_supplies_an_authority() {
        let observations =
            std::env::temp_dir().join(format!("dclutch-successor-obs-{}", Pubkey::new_unique()));
        let authority = Pubkey::new_unique();
        let other = Pubkey::new_unique();

        // Undeclared and mutable: exactly SMOKE-0's live devnet refusal.
        let mut undeclared = RoleDeploymentsV1::default();
        observe(
            &mut undeclared.trading,
            &observations,
            "trading-undeclared.bin",
            &loader_programdata_bytes(&test_elf(4), 400, Some(authority)),
        );
        let (result, root) = prepared_plan_with(RecordPublicationV1::Transaction, undeclared);
        let message = result
            .expect_err("an undeclared authority refuses")
            .to_string();
        assert!(
            message.contains("upgrade authority is not the one this plan authenticates against"),
            "refusal must still name the mismatch: {message}"
        );
        let _ = fs::remove_dir_all(&root);

        // Declared, but not the key the account actually carries.
        let mut mismatched = RoleDeploymentsV1::default();
        observe(
            &mut mismatched.trading,
            &observations,
            "trading-mismatched.bin",
            &loader_programdata_bytes(&test_elf(4), 400, Some(authority)),
        );
        mismatched.trading.expected_upgrade_authority = Some(other);
        let (result, root) = prepared_plan_with(RecordPublicationV1::Transaction, mismatched);
        let message = result.expect_err("a wrong declaration refuses").to_string();
        assert!(
            message.contains("upgrade authority is not the one this plan authenticates against"),
            "refusal must still name the mismatch: {message}"
        );
        let _ = fs::remove_dir_all(&root);

        // A declaration describes an observation, not a genesis fabrication.
        let mut fabricated = RoleDeploymentsV1::default();
        fabricated.trading.expected_upgrade_authority = Some(authority);
        let (result, root) = prepared_plan_with(RecordPublicationV1::Transaction, fabricated);
        let message = result
            .expect_err("declaring against a genesis install refuses")
            .to_string();
        assert!(
            message.contains("describes an observed account"),
            "refusal must name the category error: {message}"
        );
        let _ = fs::remove_dir_all(&root);

        // Core's authority is fixed by the rest of the plan.
        let mut core_contradiction = RoleDeploymentsV1::default();
        observe(
            &mut core_contradiction.core,
            &observations,
            "core-contradiction.bin",
            &loader_programdata_bytes(&test_elf(2), 400, Some(other)),
        );
        core_contradiction.core.expected_upgrade_authority = Some(other);
        let (result, root) =
            prepared_plan_with(RecordPublicationV1::Transaction, core_contradiction);
        let message = result
            .expect_err("Core may not be declared away from its bootstrap authority")
            .to_string();
        assert!(
            message.contains("but this plan authenticates against"),
            "refusal must name the contradiction: {message}"
        );
        let _ = fs::remove_dir_all(&root);

        fs::remove_dir_all(&observations).expect("remove observation root");
    }

    /// Decision 0012 and Core, whose answer depends on WHICH campaign runs the
    /// plan because only one of the two revokes. A genesis-installed Core binds
    /// None and says a revocation stands between substrate and release, exactly
    /// as it always has. An OBSERVED Core binds the authority it carries and
    /// says it does not -- and `run` refuses a plan whose answer is `false`,
    /// which is the local supervisor declining a substrate it cannot revoke.
    #[test]
    fn core_binds_a_revocation_only_when_the_campaign_performs_one() {
        let observations =
            std::env::temp_dir().join(format!("dclutch-successor-obs-{}", Pubkey::new_unique()));
        // Must be the bootstrap authority `prepared_plan_with` declares, or the
        // crosscheck refuses before any of this is reached.
        let bootstrap = Pubkey::new_from_array([8; 32]);

        let mut observed = RoleDeploymentsV1::default();
        observe(
            &mut observed.core,
            &observations,
            "core-mutable.bin",
            &loader_programdata_bytes(&test_elf(2), 900, Some(bootstrap)),
        );
        let (plan, root) = prepared_plan_with(RecordPublicationV1::Transaction, observed);
        let plan = plan.expect("an observed Core prepares");
        assert_eq!(
            plan.core.upgrade_authority,
            Some(bootstrap.to_string()),
            "an observed Core binds what it carries: nothing revokes it"
        );
        assert!(
            !plan.core_bootstrap.release_recognition_requires_revoke,
            "no revocation stands between this substrate and its release"
        );
        let _ = fs::remove_dir_all(&root);

        let (genesis, genesis_root) = prepared_plan(RecordPublicationV1::Transaction);
        assert_eq!(
            genesis.core.upgrade_authority, None,
            "a genesis Core still binds its post-revocation state"
        );
        assert!(
            genesis.core_bootstrap.release_recognition_requires_revoke,
            "the supervised path still revokes, and still says so"
        );
        let _ = fs::remove_dir_all(&genesis_root);

        fs::remove_dir_all(&observations).expect("remove observation root");
    }

    #[test]
    fn prepare_materializes_nine_loaders_and_nine_finalized_records() {
        let (plan, root) = prepared_plan(RecordPublicationV1::Genesis);
        assert_eq!(plan.record_publication, "genesis");
        assert_eq!(plan.genesis_accounts.len(), 27);
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

    /// The deployable shape: genesis carries the seven dClutch programs plus
    /// the two exact immutable provider programs and no protocol state. Every
    /// record coordinate must be identical to the genesis-injected plan's,
    /// because a record's address is a function of schema and content and
    /// never of who wrote the bytes.
    #[test]
    fn transaction_publication_leaves_genesis_holding_only_the_nine_programs() {
        let (genesis, genesis_root) = prepared_plan(RecordPublicationV1::Genesis);
        let (transaction, transaction_root) = prepared_plan(RecordPublicationV1::Transaction);

        assert_eq!(transaction.record_publication, "transaction");
        assert_eq!(transaction.genesis_accounts.len(), 18);
        assert_eq!(transaction.records.len(), 9);
        assert!(
            transaction
                .genesis_accounts
                .keys()
                .all(|label| label.starts_with("loader."))
        );

        for (role, program, programdata, expected_full_sha256, expected_elf_sha256) in [
            (
                "pyth-receiver",
                "rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp",
                "3UV7w2yTaqVcUAbWm1KUXdcE1Ziw8CfyyCpZvhKFkPfX",
                "63d003102c1bd48be1be24706734813094747eee82edbe066c2042474f64004e",
                "c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64",
            ),
            (
                "pyth-router",
                "HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL",
                "9hLWdeVhSG9ufuQFA5d6zUoZ6qXoMRWrS8i4HGFHnR1x",
                "04c1327626a93e09c4c833aa43b316f472d697e2fff0e5c029aaf343b83252c4",
                "f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb",
            ),
        ] {
            let program_pin = &transaction.genesis_accounts[&format!("loader.{role}.program")];
            let programdata_pin =
                &transaction.genesis_accounts[&format!("loader.{role}.programdata")];
            assert_eq!(program_pin.address, program);
            assert_eq!(programdata_pin.address, programdata);
            assert_eq!(programdata_pin.data_sha256, expected_full_sha256);
            let account = authenticate_cli_account_file_v1(
                &Path::new(&transaction.account_dir).join(format!("{programdata}.json")),
                programdata_pin,
            )
            .expect("provider ProgramData account authenticates");
            let view = ProgramDataV3View::parse(&account.data).expect("provider Loader-v3 body");
            assert_eq!(view.deployment_slot(), 0);
            assert_eq!(view.upgrade_authority(), None);
            assert_eq!(hex(&sha256_bytes(view.elf())), expected_elf_sha256);
        }

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
