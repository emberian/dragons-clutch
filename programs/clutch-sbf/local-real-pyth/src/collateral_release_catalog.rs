//! Shared finalized-chain collateral executable catalog.
//!
//! A catalog row is admitted only from one finalized account frame containing
//! the immutable `RegistryProgramReleaseV2` artifact and the exact Upgradeable
//! Loader Program/ProgramData pair it names. The artifact owns the complete
//! ProgramData digest; `AdapterReleaseV2` owns the ELF-suffix digest and the
//! parser/CPI semantics compiled into Clutch. Neither an instruction payload
//! nor an operator UI can supply a substitute release coordinate.

use crate::rpc_index::{
    CanonicalFamily, IndexedProgramRelease, ObservedRpcAccount, RpcCommitment,
};
use clutch_collateral_adapter_v2::{
    AdapterReleaseV2, CollateralPolicyV2, MAX_ADAPTER_RELEASES,
};
use clutch_product_series::{
    FixedCodec, RegistryProgramReleaseV2, RegistryReleaseLocusV2,
};
use clutch_solana_layout::artifact::ArtifactKind;
use sha2::{Digest, Sha256};
use solana_address::Address;
use std::collections::BTreeSet;

const PRODUCT_ARTIFACT_SEED_V1: &[u8] = b"dc:product-artifact:v1";
const UPGRADEABLE_PROGRAM_METADATA_BYTES: usize = 36;
const UPGRADEABLE_PROGRAMDATA_METADATA_BYTES: usize = 45;
const CATALOG_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/operator/current-collateral-release-catalog/v1\0";
const SELECTION_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/operator/current-collateral-release-selection/v1\0";

pub type Result<T> = core::result::Result<T, CurrentCollateralReleaseCatalogErrorV1>;

/// Fail-closed catalog ingestion and selection errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentCollateralReleaseCatalogErrorV1 {
    /// A semantic release or checked operator-release row is malformed.
    InvalidRelease,
    /// One account is not part of the same exact finalized observation frame.
    InvalidFinalizedFrame,
    /// The content artifact, loader pair, complete body, or ELF does not match.
    ReleaseMismatch,
    /// The catalog contains an alias, duplicate semantic release, or duplicate executable.
    DuplicateRelease,
    /// The selected Realm policy has no unique authenticated catalog row.
    ReleaseUnavailable,
}

impl core::fmt::Display for CurrentCollateralReleaseCatalogErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRelease => "collateral catalog release is invalid",
            Self::InvalidFinalizedFrame => {
                "collateral catalog accounts are not one finalized observation frame"
            }
            Self::ReleaseMismatch => {
                "collateral catalog artifact, loader deployment, or ELF differs"
            }
            Self::DuplicateRelease => "collateral catalog release is duplicated or aliased",
            Self::ReleaseUnavailable => {
                "Realm-selected collateral release is absent from the authenticated catalog"
            }
        })
    }
}

impl std::error::Error for CurrentCollateralReleaseCatalogErrorV1 {}

/// Exact finalized account frame required to ingest one collateral executable.
#[derive(Clone, Copy, Debug)]
pub struct FinalizedCollateralReleaseFrameV1<'account> {
    /// Program-owned, content-addressed `RegistryProgramReleaseV2` body.
    pub release_artifact: &'account ObservedRpcAccount,
    /// Upgradeable Loader Program account named by the artifact.
    pub program: &'account ObservedRpcAccount,
    /// Upgradeable Loader ProgramData account linked by `program`.
    pub programdata: &'account ObservedRpcAccount,
}

/// One dynamically ingested current collateral executable.
#[derive(Clone, Copy, Debug)]
pub struct AuthenticatedCurrentCollateralReleaseV1<'release> {
    adapter: AdapterReleaseV2,
    adapter_id: [u8; 32],
    program: &'release IndexedProgramRelease,
    artifact: RegistryProgramReleaseV2,
    artifact_owner: Address,
    artifact_account: Address,
    observed_slot: u64,
    receipt_id: [u8; 32],
}

impl<'release> AuthenticatedCurrentCollateralReleaseV1<'release> {
    /// Ingest one row from hostile-decoded finalized accounts.
    ///
    /// `artifact_owner` is the Clutch program that owns the immutable artifact;
    /// `program` is the checked operator description of the external token
    /// executable. Both remain projections until this function re-derives the
    /// content PDA, loader link, full ProgramData digest, and ELF digest.
    pub fn authenticate(
        adapter: AdapterReleaseV2,
        program: &'release IndexedProgramRelease,
        artifact_owner: Address,
        frame: FinalizedCollateralReleaseFrameV1<'_>,
    ) -> Result<Self> {
        adapter
            .validate()
            .map_err(|_| CurrentCollateralReleaseCatalogErrorV1::InvalidRelease)?;
        program
            .validate()
            .map_err(|_| CurrentCollateralReleaseCatalogErrorV1::InvalidRelease)?;
        if artifact_owner == Address::default()
            || program
                .families
                .binary_search(&CanonicalFamily::Collateral)
                .is_err()
            || adapter.token_program.bytes() != program.program_id.to_bytes()
            || adapter.token_program_deployment.bytes() != program.elf_sha256
        {
            return Err(CurrentCollateralReleaseCatalogErrorV1::InvalidRelease);
        }
        authenticate_finalized_frame(program, artifact_owner, frame)?;

        let artifact = RegistryProgramReleaseV2::decode(&frame.release_artifact.data)
            .map_err(|_| CurrentCollateralReleaseCatalogErrorV1::InvalidRelease)?;
        let artifact_id = artifact
            .id()
            .map_err(|_| CurrentCollateralReleaseCatalogErrorV1::InvalidRelease)?
            .content_id();
        let kind = [ArtifactKind::RegistryProgramReleaseV2.byte()];
        let expected_artifact = Address::find_program_address(
            &[PRODUCT_ARTIFACT_SEED_V1, &kind, &artifact_id.bytes()],
            &artifact_owner,
        );
        if frame.release_artifact.address != expected_artifact.0
            || artifact.program.bytes() != program.program_id.to_bytes()
            || artifact.programdata.bytes() != program.program_data.to_bytes()
            || artifact.deployment_slot != program.deployment_slot
            || artifact.locus != RegistryReleaseLocusV2::ObservedPositive
            || artifact.capability_manifest_id.bytes() != program.release_manifest_sha256
        {
            return Err(CurrentCollateralReleaseCatalogErrorV1::ReleaseMismatch);
        }

        let program_view = CurrentCollateralExecutableAccountViewV1::from_finalized(frame.program)
            .ok_or(CurrentCollateralReleaseCatalogErrorV1::InvalidFinalizedFrame)?;
        let programdata_view =
            CurrentCollateralExecutableAccountViewV1::from_finalized(frame.programdata)
                .ok_or(CurrentCollateralReleaseCatalogErrorV1::InvalidFinalizedFrame)?;
        authenticate_loader_and_elf(adapter, program, artifact, program_view, programdata_view)?;
        let adapter_id = adapter
            .id()
            .map_err(|_| CurrentCollateralReleaseCatalogErrorV1::InvalidRelease)?;
        let mut receipt = Sha256::new();
        receipt.update(CATALOG_RECEIPT_DOMAIN_V1);
        receipt.update(adapter_id.bytes());
        receipt.update(artifact_id.bytes());
        receipt.update(artifact_owner.to_bytes());
        for account in [frame.release_artifact, frame.program, frame.programdata] {
            receipt.update(account.address.to_bytes());
            receipt.update(account.owner.to_bytes());
            receipt.update(account.lamports.to_le_bytes());
            receipt.update([u8::from(account.executable)]);
            receipt.update(account.provenance.slot.to_le_bytes());
            hash_text(&mut receipt, &account.provenance.cluster_key);
            receipt.update(Sha256::digest(&account.data));
        }
        let receipt_id = receipt.finalize().into();
        if receipt_id == [0; 32] {
            return Err(CurrentCollateralReleaseCatalogErrorV1::ReleaseMismatch);
        }
        Ok(Self {
            adapter,
            adapter_id: adapter_id.bytes(),
            program,
            artifact,
            artifact_owner,
            artifact_account: frame.release_artifact.address,
            observed_slot: frame.release_artifact.provenance.slot,
            receipt_id,
        })
    }

    /// Semantic adapter release selected by a Realm policy.
    #[must_use]
    pub const fn adapter(&self) -> AdapterReleaseV2 {
        self.adapter
    }

    /// Canonical semantic identity of the selected adapter release.
    #[must_use]
    pub const fn adapter_id(&self) -> [u8; 32] {
        self.adapter_id
    }

    /// Checked external program release whose live bytes were re-derived.
    #[must_use]
    pub const fn program(&self) -> &'release IndexedProgramRelease {
        self.program
    }

    /// Hostile-decoded immutable release artifact.
    #[must_use]
    pub const fn artifact(&self) -> RegistryProgramReleaseV2 {
        self.artifact
    }

    /// Program that owns the content-addressed release artifact.
    #[must_use]
    pub const fn artifact_owner(&self) -> Address {
        self.artifact_owner
    }

    /// Exact content-PDA account authenticated for this row.
    #[must_use]
    pub const fn artifact_account(&self) -> Address {
        self.artifact_account
    }

    /// Finalized slot shared by the artifact and loader observations.
    #[must_use]
    pub const fn observed_slot(&self) -> u64 {
        self.observed_slot
    }

    /// Digest of the complete authenticated observation frame.
    #[must_use]
    pub const fn receipt_id(&self) -> [u8; 32] {
        self.receipt_id
    }

    /// Reauthenticate a freshly reacquired Program/ProgramData pair against
    /// this row without accepting offsets or expected digests from a family.
    pub fn reauthenticate_executable(
        &self,
        program: CurrentCollateralExecutableAccountViewV1<'_>,
        programdata: CurrentCollateralExecutableAccountViewV1<'_>,
    ) -> Result<()> {
        authenticate_loader_and_elf(
            self.adapter,
            self.program,
            self.artifact,
            program,
            programdata,
        )
    }

    /// Bind this executable to one hostile-decoded Realm policy and the
    /// currently executing Clutch release/profile.
    pub fn select_for<'entry>(
        &'entry self,
        clutch_release: &IndexedProgramRelease,
        policy: CollateralPolicyV2,
    ) -> Result<SelectedCurrentCollateralReleaseV1<'entry, 'release>> {
        clutch_release
            .validate()
            .map_err(|_| CurrentCollateralReleaseCatalogErrorV1::InvalidRelease)?;
        if self.artifact_owner != clutch_release.program_id
            || self.program.capability_profile_id != clutch_release.capability_profile_id
            || policy.adapter_release.bytes() != self.adapter_id
            || policy.validate_for_release(&self.adapter).is_err()
        {
            return Err(CurrentCollateralReleaseCatalogErrorV1::ReleaseUnavailable);
        }
        let policy_id = policy
            .id()
            .map_err(|_| CurrentCollateralReleaseCatalogErrorV1::ReleaseUnavailable)?
            .bytes();
        let mut receipt = Sha256::new();
        receipt.update(SELECTION_RECEIPT_DOMAIN_V1);
        receipt.update(self.receipt_id);
        receipt.update(policy_id);
        receipt.update(clutch_release.program_id.to_bytes());
        receipt.update(clutch_release.release_manifest_sha256);
        receipt.update(clutch_release.capability_profile_id);
        let receipt_id = receipt.finalize().into();
        if receipt_id == [0; 32] {
            return Err(CurrentCollateralReleaseCatalogErrorV1::ReleaseUnavailable);
        }
        Ok(SelectedCurrentCollateralReleaseV1 {
            entry: self,
            policy_id,
            receipt_id,
        })
    }
}

/// Borrowed neutral account view for current executable reauthentication.
/// Fields and the raw constructor stay crate-private so family code cannot
/// acquire loader-parser authority by supplying expected values.
#[derive(Clone, Copy, Debug)]
pub struct CurrentCollateralExecutableAccountViewV1<'account> {
    address: Address,
    owner: Address,
    executable: bool,
    data: &'account [u8],
}

impl<'account> CurrentCollateralExecutableAccountViewV1<'account> {
    /// Adapt one hostile-decoded finalized RPC observation without copying it.
    #[must_use]
    pub fn from_finalized(account: &'account ObservedRpcAccount) -> Option<Self> {
        (account.provenance.commitment == RpcCommitment::Finalized).then_some(Self {
            address: account.address,
            owner: account.owner,
            executable: account.executable,
            data: &account.data,
        })
    }

    pub(crate) const fn from_parts(
        address: Address,
        owner: Address,
        executable: bool,
        data: &'account [u8],
    ) -> Self {
        Self {
            address,
            owner,
            executable,
            data,
        }
    }
}

/// Policy-bound selection from one authenticated current catalog.
#[derive(Clone, Copy, Debug)]
pub struct SelectedCurrentCollateralReleaseV1<'catalog, 'release> {
    entry: &'catalog AuthenticatedCurrentCollateralReleaseV1<'release>,
    policy_id: [u8; 32],
    receipt_id: [u8; 32],
}

impl<'catalog, 'release> SelectedCurrentCollateralReleaseV1<'catalog, 'release> {
    /// Sole authenticated executable row selected by the policy.
    #[must_use]
    pub const fn entry(&self) -> &'catalog AuthenticatedCurrentCollateralReleaseV1<'release> {
        self.entry
    }

    /// Exact hostile-decoded `CollateralPolicyV2` content identity.
    #[must_use]
    pub const fn policy_id(&self) -> [u8; 32] {
        self.policy_id
    }

    /// Receipt joining policy, catalog row, and Clutch release/profile.
    #[must_use]
    pub const fn receipt_id(&self) -> [u8; 32] {
        self.receipt_id
    }
}

/// Bounded, canonical catalog assembled only from authenticated chain rows.
#[derive(Clone, Debug)]
pub struct CurrentCollateralReleaseCatalogV1<'release> {
    rows: Vec<AuthenticatedCurrentCollateralReleaseV1<'release>>,
}

impl<'release> CurrentCollateralReleaseCatalogV1<'release> {
    /// Canonicalize dynamically ingested rows and reject every duplicate axis.
    pub fn from_authenticated(
        mut rows: Vec<AuthenticatedCurrentCollateralReleaseV1<'release>>,
    ) -> Result<Self> {
        if rows.len() > MAX_ADAPTER_RELEASES {
            return Err(CurrentCollateralReleaseCatalogErrorV1::InvalidRelease);
        }
        rows.sort_by_key(|row| {
            row.adapter
                .id()
                .map_or([0; 32], |identity| identity.bytes())
        });
        let mut prior_adapter = None;
        let mut programs = BTreeSet::new();
        let mut artifacts = BTreeSet::new();
        for row in &rows {
            let adapter_id = row
                .adapter
                .id()
                .map_err(|_| CurrentCollateralReleaseCatalogErrorV1::InvalidRelease)?
                .bytes();
            if prior_adapter.is_some_and(|prior| prior >= adapter_id)
                || !programs.insert(row.program.program_id)
                || !artifacts.insert(row.artifact_account)
            {
                return Err(CurrentCollateralReleaseCatalogErrorV1::DuplicateRelease);
            }
            prior_adapter = Some(adapter_id);
        }
        Ok(Self { rows })
    }

    /// Empty means no observed executable has been supplied; selection remains
    /// fail-closed without manufacturing a placeholder row.
    #[must_use]
    pub const fn empty() -> Self {
        Self { rows: Vec::new() }
    }

    /// Select the sole exact row named by a chain-derived collateral policy and
    /// the same checked manifest/profile as the executing Clutch release.
    pub fn select(
        &self,
        clutch_release: &IndexedProgramRelease,
        policy: CollateralPolicyV2,
    ) -> Result<SelectedCurrentCollateralReleaseV1<'_, 'release>> {
        clutch_release
            .validate()
            .map_err(|_| CurrentCollateralReleaseCatalogErrorV1::InvalidRelease)?;
        let mut matches = self.rows.iter().filter(|row| {
            row.adapter
                .id()
                .is_ok_and(|identity| identity == policy.adapter_release)
                && row.program.capability_profile_id == clutch_release.capability_profile_id
                && row.artifact_owner == clutch_release.program_id
        });
        let selected = matches
            .next()
            .ok_or(CurrentCollateralReleaseCatalogErrorV1::ReleaseUnavailable)?;
        if matches.next().is_some() {
            return Err(CurrentCollateralReleaseCatalogErrorV1::ReleaseUnavailable);
        }
        selected.select_for(clutch_release, policy)
    }

    /// Number of authenticated rows. Zero advertises no executable authority.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether this catalog has no observed current executable.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

fn authenticate_finalized_frame(
    release: &IndexedProgramRelease,
    artifact_owner: Address,
    frame: FinalizedCollateralReleaseFrameV1<'_>,
) -> Result<()> {
    let accounts = [frame.release_artifact, frame.program, frame.programdata];
    let slot = frame.release_artifact.provenance.slot;
    let cluster = &frame.release_artifact.provenance.cluster_key;
    if slot == 0
        || cluster.trim().is_empty()
        || frame.release_artifact.owner != artifact_owner
        || frame.release_artifact.lamports == 0
        || frame.release_artifact.executable
        || frame.program.address != release.program_id
        || frame.programdata.address != release.program_data
        || frame.program.address == frame.programdata.address
        || accounts.iter().any(|account| {
            account.provenance.commitment != RpcCommitment::Finalized
                || account.provenance.slot != slot
                || account.provenance.cluster_key != *cluster
        })
        || frame.program.provenance.release_key != release.key()
        || frame.programdata.provenance.release_key != release.key()
    {
        return Err(CurrentCollateralReleaseCatalogErrorV1::InvalidFinalizedFrame);
    }
    Ok(())
}

fn authenticate_loader_and_elf(
    adapter: AdapterReleaseV2,
    release: &IndexedProgramRelease,
    artifact: RegistryProgramReleaseV2,
    program: CurrentCollateralExecutableAccountViewV1<'_>,
    programdata: CurrentCollateralExecutableAccountViewV1<'_>,
) -> Result<()> {
    let program_body = program.data;
    let programdata_body = programdata.data;
    if program.address != release.program_id || programdata.address != release.program_data {
        return Err(CurrentCollateralReleaseCatalogErrorV1::ReleaseMismatch);
    }
    if program.owner != solana_sdk_ids::bpf_loader_upgradeable::ID
        || programdata.owner != solana_sdk_ids::bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
        || program_body.len() != UPGRADEABLE_PROGRAM_METADATA_BYTES
        || programdata_body.len() <= UPGRADEABLE_PROGRAMDATA_METADATA_BYTES
        || program_body.get(0..4) != Some(2_u32.to_le_bytes().as_slice())
        || programdata_body.get(0..4) != Some(3_u32.to_le_bytes().as_slice())
        || program_body.get(4..36) != Some(release.program_data.to_bytes().as_slice())
    {
        return Err(CurrentCollateralReleaseCatalogErrorV1::ReleaseMismatch);
    }
    let deployment_slot = read_u64_le(programdata_body, 4)?;
    match programdata_body[12] {
        0 if programdata_body[13..45].iter().all(|byte| *byte == 0) => {}
        1 if programdata_body[13..45].iter().any(|byte| *byte != 0) => {}
        _ => return Err(CurrentCollateralReleaseCatalogErrorV1::ReleaseMismatch),
    }
    let complete_programdata_sha256: [u8; 32] = Sha256::digest(programdata_body).into();
    let elf_sha256: [u8; 32] =
        Sha256::digest(&programdata_body[UPGRADEABLE_PROGRAMDATA_METADATA_BYTES..]).into();
    if deployment_slot == 0
        || deployment_slot != release.deployment_slot
        || artifact.deployment_slot != deployment_slot
        || artifact.programdata_sha256.bytes() != complete_programdata_sha256
        || release.elf_sha256 != elf_sha256
        || adapter.token_program_deployment.bytes() != elf_sha256
    {
        return Err(CurrentCollateralReleaseCatalogErrorV1::ReleaseMismatch);
    }
    Ok(())
}

fn read_u64_le(input: &[u8], at: usize) -> Result<u64> {
    let bytes: [u8; 8] = input
        .get(at..at + 8)
        .ok_or(CurrentCollateralReleaseCatalogErrorV1::ReleaseMismatch)?
        .try_into()
        .map_err(|_| CurrentCollateralReleaseCatalogErrorV1::ReleaseMismatch)?;
    Ok(u64::from_le_bytes(bytes))
}

fn hash_text(hash: &mut Sha256, value: &str) {
    hash.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
    hash.update(value.as_bytes());
}
