//! Chain-derived unsigned material for current Source action 1.
//!
//! The operator reads the sealed Product artifact and destination slot from
//! one finalized snapshot. It hostile-decodes the exact 1,296-byte manifest,
//! recomputes both content-addressed PDAs, and emits only the compact manifest
//! ID request. No manifest body, account owner, signer, or destination address
//! is accepted from a frontend projection.

use crate::rpc_index::{
    CanonicalFamily, CanonicalIntentCoordinate, IndexedProgramRelease, ObservedRpcAccount,
    ObservedRpcAccountRemoval, RpcAccountRemovalKind, RpcCommitment,
};
use crate::action_material::{ActionFreshnessBoundaryV1, CanonicalActionMaterialV1};
use crate::transaction_builder::{
    ExactEquation, IntegerUnit, OwnedInstructionDraft, ProtocolTransactionBuilder, SemanticOwner,
    TransactionTransport, UnsignedProtocolTransaction,
};
use crate::workflow_graph::{ResumableWorkflowCursor, WorkflowLane, WorkflowPosition};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::registry::{ExtensionFamily, SourceSeriesAction};
use clutch_solana_layout::source_series::{
    account_contract_v2, validate_account_metas_v2, ObservedSourceAccountMetaV2,
    RegisterReleaseIntentV2, REGISTER_RELEASE_PAYLOAD_BYTES_V2,
};
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_adapter::PdaRecipeV3;
use clutch_source_plane_v3_runtime::{SourceReleaseManifestV2, SOURCE_RELEASE_MANIFEST_BYTES};
use solana_address::Address;
use solana_instruction::AccountMeta;
use sha2::{Digest, Sha256};

pub const SOURCE_ACTION1_VALIDITY_SLOTS_V1: u64 = 32;

pub const SOURCE_ACTION1_FAMILY_V1: ExtensionFamily = ExtensionFamily::SourceSeries;
pub const SOURCE_ACTION1_LOCAL_ACTION_V1: u8 = 1;

const OWNER_SCHEMA_V1: &str = "dragons-clutch/operator/source-action1-material/v1";
const OWNER_PACKAGE_V1: &str = "clutch-source-plane-v3-runtime";
const PRODUCT_ARTIFACT_SEED_V1: &[u8] = b"dc:product-artifact:v1";
const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0; 32]);
const RENT_SYSVAR_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238,
    8, 155, 161, 253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
]);

pub type SourceAction1MaterialResult<T> = core::result::Result<T, SourceAction1MaterialError>;
type Result<T> = SourceAction1MaterialResult<T>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceAction1MaterialError {
    CheckedRelease,
    ChainSnapshot,
    ChainAuthority,
    AccountOccupancy,
    Construction,
}

impl core::fmt::Display for SourceAction1MaterialError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::CheckedRelease => "checked release does not admit Source action 1",
            Self::ChainSnapshot => "Source registration accounts are not one finalized snapshot",
            Self::ChainAuthority => "Source release artifact failed hostile authentication",
            Self::AccountOccupancy => "Source release destination has a noncanonical occupant",
            Self::Construction => "canonical Source action-1 construction refused",
        })
    }
}

impl std::error::Error for SourceAction1MaterialError {}

#[derive(Clone, Copy, Debug)]
pub enum ObservedSourceReleaseSlotV1<'a> {
    Present(&'a ObservedRpcAccount),
    Removed(&'a ObservedRpcAccountRemoval),
}

impl ObservedSourceReleaseSlotV1<'_> {
    fn address(self) -> Address {
        match self {
            Self::Present(value) => value.address,
            Self::Removed(value) => value.address,
        }
    }

    fn provenance(self) -> &crate::rpc_index::RpcObservationProvenance {
        match self {
            Self::Present(value) => &value.provenance,
            Self::Removed(value) => &value.provenance,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SourceAction1ChainSnapshotV1<'a> {
    pub source_release_artifact: &'a ObservedRpcAccount,
    pub source_release: ObservedSourceReleaseSlotV1<'a>,
    pub system_program: &'a ObservedRpcAccount,
    pub rent_sysvar: &'a ObservedRpcAccount,
}

#[derive(Clone, Debug)]
pub struct ChainDerivedSourceAction1MaterialV1 {
    checked_release_key: String,
    release_manifest_sha256: [u8; 32],
    capability_profile_id: [u8; 32],
    program_id: Address,
    program_data: Address,
    source_release_manifest_id: [u8; 32],
    release_payer: Address,
    driver_account: Address,
    observed_slot: u64,
    valid_before_slot: u64,
    authority_state_sha256: [u8; 32],
    ordered_accounts: Vec<AccountMeta>,
}

impl ChainDerivedSourceAction1MaterialV1 {
    #[must_use]
    pub const fn source_release_manifest_id(&self) -> [u8; 32] {
        self.source_release_manifest_id
    }

    #[must_use]
    pub const fn release_payer(&self) -> Address {
        self.release_payer
    }

    pub fn unsigned_instruction(
        &self,
        release: &IndexedProgramRelease,
    ) -> Result<OwnedInstructionDraft> {
        let coordinate = CanonicalIntentCoordinate {
            family_tag: SOURCE_ACTION1_FAMILY_V1.tag(),
            family_version: SOURCE_ACTION1_FAMILY_V1.version(),
            local_action: SOURCE_ACTION1_LOCAL_ACTION_V1,
        };
        if release.key() != self.checked_release_key
            || release.program_id != self.program_id
            || release.program_data != self.program_data
            || release.release_manifest_sha256 != self.release_manifest_sha256
            || release.capability_profile_id != self.capability_profile_id
            || release.enabled_intents.binary_search(&coordinate).is_err()
        {
            return Err(SourceAction1MaterialError::CheckedRelease);
        }
        let intent = RegisterReleaseIntentV2 {
            source_release_manifest_id: self.source_release_manifest_id,
        };
        let mut payload = [0_u8; REGISTER_RELEASE_PAYLOAD_BYTES_V2];
        intent
            .encode(&mut payload)
            .map_err(|_| SourceAction1MaterialError::Construction)?;
        OwnedInstructionDraft::checked_release_source_request_v2(
            release,
            "register-source-release-v2",
            SemanticOwner {
                package: OWNER_PACKAGE_V1.into(),
                schema: OWNER_SCHEMA_V1.into(),
                release_sha256: self.release_manifest_sha256,
            },
            self.ordered_accounts.clone(),
            vec![self.release_payer],
            vec![ExactEquation {
                name: "Source release registration preserves immutable manifest bytes".into(),
                unit: IntegerUnit::Lamports,
                left: 0,
                right: 0,
            }],
            SourceSeriesAction::RegisterRelease,
            0,
            &payload,
        )
        .map_err(|_| SourceAction1MaterialError::Construction)
    }

    /// Compile the exact blockhash-free transaction. The release payer is the
    /// frozen writable signer role, so message privilege union cannot widen
    /// the instruction account contract.
    pub fn unsigned_transaction(
        &self,
        release: &IndexedProgramRelease,
        transport: TransactionTransport,
    ) -> Result<UnsignedProtocolTransaction> {
        let draft = self.unsigned_instruction(release)?;
        ProtocolTransactionBuilder::new(
            self.release_payer,
            self.program_id,
            self.release_manifest_sha256,
            transport,
        )
        .and_then(|builder| builder.build_source_v0(draft))
        .map_err(|_| SourceAction1MaterialError::Construction)
    }

    /// Promote the finalized registration snapshot into the read-only
    /// operator material registry.
    pub fn canonical_material(
        &self,
        release: &IndexedProgramRelease,
        workflow_id: [u8; 32],
        transport: TransactionTransport,
    ) -> Result<CanonicalActionMaterialV1> {
        CanonicalActionMaterialV1::from_chain_derived_source_v2(
            release,
            SourceSeriesAction::RegisterRelease,
            self.driver_account,
            self.observed_slot,
            ResumableWorkflowCursor {
                workflow_id,
                lane: WorkflowLane::SourceCrank,
                generation: 1,
                position: WorkflowPosition {
                    phase: SOURCE_ACTION1_LOCAL_ACTION_V1,
                    item: 0,
                },
                observed_state_sha256: self.authority_state_sha256,
            },
            ActionFreshnessBoundaryV1 {
                observed_slot: self.observed_slot,
                valid_before_slot: self.valid_before_slot,
                maximum_validity_slots: SOURCE_ACTION1_VALIDITY_SLOTS_V1,
            },
            self.release_payer,
            &self.ordered_accounts,
            self.unsigned_transaction(release, transport)?,
        )
        .map_err(|_| SourceAction1MaterialError::Construction)
    }
}

pub fn derive_source_action1_material_v1(
    release: &IndexedProgramRelease,
    release_payer: Address,
    snapshot: SourceAction1ChainSnapshotV1<'_>,
) -> Result<ChainDerivedSourceAction1MaterialV1> {
    authenticate_checked_release(release)?;
    authenticate_snapshot(release, snapshot)?;
    let authority_state_sha256 = snapshot_digest(snapshot, release_payer);
    if release_payer == Address::default()
        || release_payer == snapshot.source_release_artifact.address
        || release_payer == snapshot.source_release.address()
    {
        return Err(SourceAction1MaterialError::ChainAuthority);
    }
    let artifact = snapshot.source_release_artifact;
    if artifact.owner != release.program_id
        || artifact.executable
        || artifact.data.len() != SOURCE_RELEASE_MANIFEST_BYTES
    {
        return Err(SourceAction1MaterialError::ChainAuthority);
    }
    let manifest = SourceReleaseManifestV2::decode(&artifact.data)
        .map_err(|_| SourceAction1MaterialError::ChainAuthority)?;
    let manifest_id = manifest
        .id()
        .map_err(|_| SourceAction1MaterialError::ChainAuthority)?;
    let artifact_kind = [ArtifactKind::SourceReleaseManifestV2.byte()];
    let (expected_artifact, _) = Address::find_program_address(
        &[PRODUCT_ARTIFACT_SEED_V1, &artifact_kind, &manifest_id.bytes()],
        &release.program_id,
    );
    if artifact.address != expected_artifact {
        return Err(SourceAction1MaterialError::ChainAuthority);
    }
    let recipe = PdaRecipeV3::source_release(manifest_id)
        .map_err(|_| SourceAction1MaterialError::ChainAuthority)?;
    let expected_release = derive_recipe_address(release.program_id, recipe)?;
    if snapshot.source_release.address() != expected_release {
        return Err(SourceAction1MaterialError::AccountOccupancy);
    }
    match snapshot.source_release {
        ObservedSourceReleaseSlotV1::Present(account)
            if account.owner == release.program_id
                && !account.executable
                && account.data == artifact.data => {}
        ObservedSourceReleaseSlotV1::Present(account)
            if account.owner == SYSTEM_PROGRAM_ID
                && !account.executable
                && account.data.is_empty() => {}
        ObservedSourceReleaseSlotV1::Removed(account)
            if account.kind == RpcAccountRemovalKind::Closed
                && account.observed_lamports == 0
                && account.observed_data_bytes == 0 => {}
        _ => return Err(SourceAction1MaterialError::AccountOccupancy),
    }
    let addresses = [
        artifact.address,
        expected_release,
        release_payer,
        SYSTEM_PROGRAM_ID,
        RENT_SYSVAR_ID,
    ];
    let contract = account_contract_v2(SourceSeriesAction::RegisterRelease);
    if contract.len() != addresses.len() {
        return Err(SourceAction1MaterialError::Construction);
    }
    let mut ordered_accounts = Vec::with_capacity(addresses.len());
    let mut observed = Vec::with_capacity(addresses.len());
    for (index, pubkey) in addresses.into_iter().enumerate() {
        let expected = contract
            .meta(index)
            .ok_or(SourceAction1MaterialError::Construction)?;
        ordered_accounts.push(AccountMeta {
            pubkey,
            is_signer: expected.signer,
            is_writable: expected.writable,
        });
        observed.push(ObservedSourceAccountMetaV2 {
            key: pubkey.to_bytes(),
            writable: expected.writable,
            signer: expected.signer,
        });
    }
    validate_account_metas_v2(SourceSeriesAction::RegisterRelease, &observed)
        .map_err(|_| SourceAction1MaterialError::Construction)?;
    let valid_before_slot = snapshot.source_release_artifact.provenance.slot
        .checked_add(SOURCE_ACTION1_VALIDITY_SLOTS_V1)
        .ok_or(SourceAction1MaterialError::Construction)?;
    Ok(ChainDerivedSourceAction1MaterialV1 {
        checked_release_key: release.key(),
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        program_id: release.program_id,
        program_data: release.program_data,
        source_release_manifest_id: manifest_id.bytes(),
        release_payer,
        driver_account: artifact.address,
        observed_slot: artifact.provenance.slot,
        valid_before_slot,
        authority_state_sha256,
        ordered_accounts,
    })
}

fn snapshot_digest(snapshot: SourceAction1ChainSnapshotV1<'_>, release_payer: Address) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator/source-action1-finalized-snapshot/v1\0");
    hash.update(snapshot.source_release_artifact.provenance.slot.to_le_bytes());
    hash.update(release_payer.to_bytes());
    for account in [snapshot.source_release_artifact, snapshot.system_program, snapshot.rent_sysvar] {
        hash.update(account.address.to_bytes());
        hash.update(account.owner.to_bytes());
        hash.update(account.lamports.to_le_bytes());
        hash.update([u8::from(account.executable)]);
        hash.update(&account.data);
    }
    match snapshot.source_release {
        ObservedSourceReleaseSlotV1::Present(account) => {
            hash.update([1]);
            hash.update(account.address.to_bytes());
            hash.update(account.owner.to_bytes());
            hash.update(account.lamports.to_le_bytes());
            hash.update(&account.data);
        }
        ObservedSourceReleaseSlotV1::Removed(account) => {
            hash.update([2]);
            hash.update(account.address.to_bytes());
            hash.update(account.observed_owner.to_bytes());
            hash.update(account.observed_lamports.to_le_bytes());
            hash.update((account.observed_data_bytes as u64).to_le_bytes());
        }
    }
    hash.finalize().into()
}

fn authenticate_checked_release(release: &IndexedProgramRelease) -> Result<()> {
    let coordinate = CanonicalIntentCoordinate {
        family_tag: SOURCE_ACTION1_FAMILY_V1.tag(),
        family_version: SOURCE_ACTION1_FAMILY_V1.version(),
        local_action: SOURCE_ACTION1_LOCAL_ACTION_V1,
    };
    release
        .validate()
        .map_err(|_| SourceAction1MaterialError::CheckedRelease)?;
    if !release.families.contains(&CanonicalFamily::Source)
        || release.enabled_intents.binary_search(&coordinate).is_err()
    {
        return Err(SourceAction1MaterialError::CheckedRelease);
    }
    Ok(())
}

fn authenticate_snapshot(
    release: &IndexedProgramRelease,
    snapshot: SourceAction1ChainSnapshotV1<'_>,
) -> Result<()> {
    let first = &snapshot.source_release_artifact.provenance;
    let release_key = release.key();
    for provenance in [
        first,
        snapshot.source_release.provenance(),
        &snapshot.system_program.provenance,
        &snapshot.rent_sysvar.provenance,
    ] {
        if provenance.commitment != RpcCommitment::Finalized
            || provenance.slot != first.slot
            || provenance.cluster_key != first.cluster_key
            || provenance.release_key != release_key
        {
            return Err(SourceAction1MaterialError::ChainSnapshot);
        }
    }
    if snapshot.system_program.address != SYSTEM_PROGRAM_ID
        || !snapshot.system_program.executable
        || snapshot.rent_sysvar.address != RENT_SYSVAR_ID
        || snapshot.rent_sysvar.executable
    {
        return Err(SourceAction1MaterialError::ChainAuthority);
    }
    Ok(())
}

fn derive_recipe_address(program_id: Address, recipe: PdaRecipeV3) -> Result<Address> {
    recipe
        .validate()
        .map_err(|_| SourceAction1MaterialError::ChainAuthority)?;
    let mut seeds = Vec::with_capacity(usize::from(recipe.seed_count()));
    let mut index = 0_usize;
    while index < usize::from(recipe.seed_count()) {
        seeds.push(
            recipe
                .seed(index)
                .map_err(|_| SourceAction1MaterialError::ChainAuthority)?,
        );
        index += 1;
    }
    Ok(Address::find_program_address(&seeds, &program_id).0)
}
