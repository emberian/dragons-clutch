//! Opaque, release-authenticated action material for operator projections.
//!
//! Construction accepts typed semantic-owner state and account projections,
//! never browser JSON or caller-authored instruction bytes. The resulting
//! artifact remains unsigned and blockhash-free. It can make an operator
//! control inspectable, but it cannot sign, submit, or predict poststate.

use crate::account_index::{FinalizedAccountAbsence, IndexedBranch};
use crate::rpc_index::{
    CanonicalIntentCoordinate, IndexedProgramRelease, ObservedRpcAccount, RpcCommitment,
};
use crate::operatord::KeeperActionSelection;
use crate::transaction_builder::{
    ExactEquation, IntegerUnit, ProtocolFlow, ProtocolTransactionBuilder, RuntimeAdmission,
    TransactionMessageVersionV1, UnsignedProtocolTransaction,
};
use crate::workflow_graph::{
    plan_source_crank, CanonicalActionCoordinate, ExplicitOperatorReleaseManifest,
    PlannedWorkflowNode, ResumableWorkflowCursor, WorkflowLane, WorkflowPosition,
    SourceCrankObservation, SourceWorkflowActionMaterial, WorkflowGraphError,
};
use clutch_solana_layout::registry::{
    AllocationStatus, ExtensionAction, ExtensionFamily, STRUCTURED_CLAIM_FAMILY_TAG,
    STRUCTURED_CLAIM_FAMILY_VERSION, SOURCE_SERIES_FAMILY_TAG, SOURCE_SERIES_FAMILY_VERSION,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::product_series::{SeriesMarketLinkAccountV2, SeriesRegistryAccountV3};
use clutch_solana_layout::{ProfileAccount, RealmAccount};
use clutch_collateral_adapter_v2::{
    AdapterReleaseV2, ClaimLedgerV3, CollateralPolicyV2, HoardV2,
    MarketLiabilityLifecycleV1, ResolutionStateV5, ResolutionV5,
    CLAIM_LEDGER_V3_PDA_SEED_V1, COLLATERAL_POLICY_PDA_SEED_V1,
    HOARD_AUTHORITY_V2_PDA_SEED_V1, HOARD_TOKEN_V2_PDA_SEED_V1,
    HOARD_V2_PDA_SEED_V1, PROFILE_PDA_SEED_V1, REALM_PDA_SEED_V1,
};
use clutch_general_v2_contract::{
    MarketBindingV2, MarketRuntimeV3AccountV1, MARKET_BINDING_SEED_DOMAIN_V1,
    MARKET_RUNTIME_SEED_DOMAIN_V1,
};
use clutch_product_series::{
    CompiledProductSeriesBundleV6, ContentId, FixedCodec, MarketInstancePreimageV2,
    NativeClaimBasisV1, RegistryCapabilityProfileV4, RegistryProgramReleaseV2,
    RegistryReleaseLocusV2,
    SeriesAttachmentPlanV5, SeriesFundingTermsV2, SeriesLinkObligationStatusV2, SeriesLinkObligationV2,
    SeriesMarketLinkPhaseV2,
};
use clutch_retirement::{
    PositionAccountV3, PositionLifecycleV3, PositionPurposeV3, ReplayV3Envelope,
    ReplayV3HashBackend, ReplayV3Lifecycle,
    POSITION_V3_PDA_PREFIX, PURPOSE_REPLAY_V3_PDA_PREFIX,
};
use clutch_structured_claim::{ClaimVector, DeploymentBinding};
use clutch_structured_claim_adapter::{
    canonical_native_claim_id_v1, canonical_series_scoped_wrapper_product_id_v2,
    current_structured_account_meta_v1, current_structured_action_contract_v1,
    decode_canonical_wrapper_mint_v1, decode_canonical_wrapper_token_v1,
    STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1,
    STRUCTURED_CURRENT_ACCOUNT_CONTRACT_LABEL_V1,
    STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1,
    STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1,
    DESCRIPTOR_SEED, MINT_AUTHORITY_SEED, MINT_SEED, VAULT_OWNER_SEED,
};
use clutch_structured_claim_runtime_contract::{
    CreateDescriptorPayloadV1, DescriptorBasisV1, DescriptorStateV1,
    reconstruct_descriptor_identity_v1, structured_owner_release_id_v2,
    StructuredClaimActionV1, StructuredClaimDescriptorV2, StructuredMarketRootBindingV1,
    StructuredClaimReplayExtensionV1, StructuredMarketRootV1, VaultMutationPayloadV1,
    WrapperQuantityPayloadV1,
    WrapperRecipeHashV1, WrapperRecipeSetV1, DESCRIPTOR_ACCOUNT_TAG,
    DESCRIPTOR_ACCOUNT_VERSION,
};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::AccountMeta;
use solana_message::AddressLookupTableAccount;
use std::collections::BTreeSet;

pub const CANONICAL_ACTION_MATERIAL_SCHEMA_V1: &str =
    "dragons-clutch/operator-canonical-action-material/v1";

pub type Result<T> = core::result::Result<T, CanonicalActionMaterialErrorV1>;

/// Fail-closed construction errors. None grants execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalActionMaterialErrorV1 {
    InvalidRelease,
    ReleaseMismatch,
    CoordinateDisabled,
    WrongSelection,
    InvalidFreshness,
    FeePayerMismatch,
    InvalidPlan,
    InvalidChainState,
}

impl core::fmt::Display for CanonicalActionMaterialErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRelease => "canonical action material has an invalid checked release",
            Self::ReleaseMismatch => {
                "canonical action material differs from the checked program release"
            }
            Self::CoordinateDisabled => {
                "canonical action coordinate is not enabled by the checked release"
            }
            Self::WrongSelection => {
                "canonical action material differs from the selected finalized cursor"
            }
            Self::InvalidFreshness => "canonical action validity boundary is invalid",
            Self::FeePayerMismatch => {
                "transaction fee payer differs from the semantic account-role payer"
            }
            Self::InvalidPlan => "semantic-owner transaction construction was noncanonical",
            Self::InvalidChainState => {
                "hostile-decoded chain state cannot derive an exact current action"
            }
        })
    }
}

/// Opaque checked collateral-catalog row selected by Profile V2.
///
/// The semantic adapter release owns parser/CPI behavior. The indexed program
/// release and content-addressed RegistryProgramReleaseV2 own the exact loader
/// deployment. Live role construction later reauthenticates the Program and
/// ProgramData bodies from the same finalized account frame.
#[derive(Clone, Copy, Debug)]
pub struct StructuredCollateralCatalogEntryV1<'release> {
    adapter: AdapterReleaseV2,
    program: &'release IndexedProgramRelease,
    artifact: RegistryProgramReleaseV2,
    artifact_owner: Address,
    receipt_id: [u8; 32],
}

impl<'release> StructuredCollateralCatalogEntryV1<'release> {
    /// Authenticate one finalized, base-owned catalog artifact against the
    /// checked operator release. No instruction account or browser field can
    /// replace this chain artifact.
    pub fn authenticate(
        adapter: AdapterReleaseV2,
        program: &'release IndexedProgramRelease,
        artifact_owner: Address,
        artifact_account: &ObservedRpcAccount,
    ) -> Result<Self> {
        adapter
            .validate()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
        program
            .validate()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
        if artifact_owner == Address::default()
            || artifact_account.owner != artifact_owner
            || artifact_account.executable
            || artifact_account.provenance.commitment != RpcCommitment::Finalized
            || artifact_account.provenance.slot == 0
            || adapter.token_program.bytes() != program.program_id.to_bytes()
            || adapter.token_program_deployment.bytes() != program.elf_sha256
            || program
                .families
                .binary_search(&crate::rpc_index::CanonicalFamily::Collateral)
                .is_err()
        {
            return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
        }
        let artifact = RegistryProgramReleaseV2::decode(&artifact_account.data)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
        let artifact_id = artifact
            .id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?
            .content_id();
        let kind = [ArtifactKind::RegistryProgramReleaseV2.byte()];
        let expected_artifact = Address::find_program_address(
            &[b"dc:product-artifact:v1", &kind, &artifact_id.bytes()],
            &artifact_owner,
        );
        if artifact_account.address != expected_artifact.0
            || artifact.program.bytes() != program.program_id.to_bytes()
            || artifact.programdata.bytes() != program.program_data.to_bytes()
            || artifact.deployment_slot != program.deployment_slot
            || artifact.locus != RegistryReleaseLocusV2::ObservedPositive
            || artifact.capability_manifest_id.bytes() != program.release_manifest_sha256
        {
            return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
        }
        let adapter_id = adapter
            .id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
        let mut receipt = Sha256::new();
        receipt.update(b"dragons-clutch/operator/structured-collateral-catalog/v1\0");
        receipt.update(adapter_id.bytes());
        receipt.update(artifact_id.bytes());
        receipt.update(artifact_account.address.to_bytes());
        receipt.update(artifact_account.lamports.to_le_bytes());
        receipt.update(artifact_account.provenance.slot.to_le_bytes());
        receipt.update(Sha256::digest(&artifact_account.data));
        let receipt_id = receipt.finalize().into();
        Ok(Self {
            adapter,
            program,
            artifact,
            artifact_owner,
            receipt_id,
        })
    }
}

/// Opaque exact wrapper/base/Token/collateral release join for Structured construction.
#[derive(Clone, Copy, Debug)]
pub struct StructuredOperatorReleaseSetV1<'release> {
    wrapper: &'release IndexedProgramRelease,
    base: &'release IndexedProgramRelease,
    token_2022: &'release IndexedProgramRelease,
    collateral: StructuredCollateralCatalogEntryV1<'release>,
}

impl<'release> StructuredOperatorReleaseSetV1<'release> {
    /// Authenticate the disjoint checked manifests and the complete
    /// current Structured intent set. Empty or partial release admission is
    /// refused rather than advertised as callable.
    pub fn authenticate(
        wrapper: &'release IndexedProgramRelease,
        base: &'release IndexedProgramRelease,
        token_2022: &'release IndexedProgramRelease,
        collateral: StructuredCollateralCatalogEntryV1<'release>,
    ) -> Result<Self> {
        for release in [wrapper, base, token_2022] {
            release
                .validate()
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
        }
        if ExtensionFamily::StructuredClaim.allocation_status() != Some(AllocationStatus::Frozen) {
            return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
        }
        if wrapper.program_id == base.program_id
            || wrapper.program_id == token_2022.program_id
            || base.program_id == token_2022.program_id
            || wrapper.release_manifest_sha256
                != STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1
            || base.release_manifest_sha256 != STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1
            || token_2022.release_manifest_sha256
                != STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1
            || wrapper.capability_profile_id != base.capability_profile_id
            || token_2022.capability_profile_id != base.capability_profile_id
            || collateral.program.capability_profile_id != base.capability_profile_id
            || collateral.artifact_owner != base.program_id
            || wrapper
                .families
                .binary_search(&crate::rpc_index::CanonicalFamily::StructuredClaim)
                .is_err()
        {
            return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
        }
        for action in [
            StructuredClaimActionV1::CreateDescriptor,
            StructuredClaimActionV1::WrapFull,
            StructuredClaimActionV1::UnwrapFull,
            StructuredClaimActionV1::CompactDonation,
            StructuredClaimActionV1::RedeemTerminal,
            StructuredClaimActionV1::RetireDescriptor,
        ] {
            let coordinate = structured_coordinate(action);
            if wrapper.enabled_intents.binary_search(&coordinate).is_err() {
                return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
            }
        }
        Ok(Self {
            wrapper,
            base,
            token_2022,
            collateral,
        })
    }
}

/// One role account reacquired from finalized chain state. An absence can only
/// be used where the semantic constructor independently derives the exact PDA.
#[derive(Clone, Copy, Debug)]
pub struct StructuredChainAccountV1<'account> {
    address: Address,
    present: Option<&'account ObservedRpcAccount>,
    observed_slot: u64,
}

impl<'account> StructuredChainAccountV1<'account> {
    /// Retain one bounded finalized RPC account body without projecting flags.
    pub fn present(account: &'account ObservedRpcAccount) -> Result<Self> {
        if account.provenance.commitment != RpcCommitment::Finalized {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        Ok(Self {
            address: account.address,
            present: Some(account),
            observed_slot: account.provenance.slot,
        })
    }

    /// Retain an exact finalized account absence. Its address remains
    /// untrusted until the action constructor derives and equality-checks it.
    pub fn absent(address: Address, absence: &'account FinalizedAccountAbsence) -> Result<Self> {
        if address == Address::default()
            || absence.slot() == 0
            || absence.receive_sequence() == 0
            || absence.release_key().trim().is_empty()
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        Ok(Self {
            address,
            present: None,
            observed_slot: absence.slot(),
        })
    }

    fn data(self) -> Result<&'account [u8]> {
        self.present
            .map(|account| account.data.as_slice())
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)
    }

    fn owner(self) -> Result<Address> {
        self.present
            .map(|account| account.owner)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)
    }

    fn executable(self) -> bool {
        self.present.is_some_and(|account| account.executable)
    }
}

const ADDRESS_LOOKUP_TABLE_META_BYTES: usize = 56;
const ADDRESS_LOOKUP_TABLE_MAX_ADDRESSES: usize = 256;

/// Finalized, hostile-decoded address lookup table used only to compress the
/// exact Structured role set into a Solana v0 transaction. It never becomes
/// an instruction account, protocol authority, or caller-shaped account list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredAddressLookupTableV1 {
    table: AddressLookupTableAccount,
    observed_slot: u64,
    state_sha256: [u8; 32],
    cluster_key: String,
}

impl StructuredAddressLookupTableV1 {
    /// Authenticate an initialized, non-deactivating lookup table whose full
    /// address tail was finalized after its most recent extension.
    pub fn authenticate(account: &ObservedRpcAccount) -> Result<Self> {
        if account.address == Address::default()
            || account.owner != solana_sdk_ids::address_lookup_table::ID
            || account.lamports == 0
            || account.executable
            || account.provenance.commitment != RpcCommitment::Finalized
            || account.provenance.slot == 0
            || account.provenance.cluster_key.trim().is_empty()
            || account.data.len() < ADDRESS_LOOKUP_TABLE_META_BYTES
            || (account.data.len() - ADDRESS_LOOKUP_TABLE_META_BYTES) % 32 != 0
            || account.data.get(0..4) != Some([1, 0, 0, 0].as_slice())
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        let deactivation_slot = read_u64_le(&account.data, 4)?;
        let last_extended_slot = read_u64_le(&account.data, 12)?;
        if deactivation_slot != u64::MAX || last_extended_slot >= account.provenance.slot {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        let address_count = (account.data.len() - ADDRESS_LOOKUP_TABLE_META_BYTES) / 32;
        if address_count == 0 || address_count > ADDRESS_LOOKUP_TABLE_MAX_ADDRESSES {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        let last_extension_start = usize::from(account.data[20]);
        if last_extension_start >= address_count {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        match account.data[21] {
            0 if account.data[22..ADDRESS_LOOKUP_TABLE_META_BYTES]
                .iter()
                .all(|byte| *byte == 0) => {}
            1 if account.data[22..54].iter().any(|byte| *byte != 0)
                && account.data[54..ADDRESS_LOOKUP_TABLE_META_BYTES]
                    .iter()
                    .all(|byte| *byte == 0) => {}
            _ => return Err(CanonicalActionMaterialErrorV1::InvalidChainState),
        }
        let mut addresses = Vec::with_capacity(address_count);
        let mut unique = BTreeSet::new();
        for body in account.data[ADDRESS_LOOKUP_TABLE_META_BYTES..].chunks_exact(32) {
            let bytes: [u8; 32] = body
                .try_into()
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let address = Address::new_from_array(bytes);
            if address == Address::default() || !unique.insert(address) {
                return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
            }
            addresses.push(address);
        }
        let mut hash = Sha256::new();
        hash.update(b"dragons-clutch/operator/finalized-address-lookup-table/v1\0");
        hash.update(account.address.to_bytes());
        hash.update(account.owner.to_bytes());
        hash.update(account.lamports.to_le_bytes());
        hash.update(account.rent_epoch.to_le_bytes());
        hash.update(account.provenance.slot.to_le_bytes());
        hash_text(&mut hash, &account.provenance.cluster_key);
        hash_text(&mut hash, &account.provenance.release_key);
        hash.update(Sha256::digest(&account.data));
        let state_sha256 = hash.finalize().into();
        Ok(Self {
            table: AddressLookupTableAccount {
                key: account.address,
                addresses,
            },
            observed_slot: account.provenance.slot,
            state_sha256,
            cluster_key: account.provenance.cluster_key.clone(),
        })
    }

    /// Lookup-table account identity encoded into the v0 message.
    #[must_use]
    pub const fn account(&self) -> Address {
        self.table.key
    }

    /// Finalized slot of the complete decoded table body.
    #[must_use]
    pub const fn observed_slot(&self) -> u64 {
        self.observed_slot
    }

    /// Digest of the complete lookup-table observation and provenance.
    #[must_use]
    pub const fn state_sha256(&self) -> [u8; 32] {
        self.state_sha256
    }
}

#[derive(Clone, Copy, Debug)]
struct OperatorSha256V1;

impl WrapperRecipeHashV1 for OperatorSha256V1 {
    fn hashv(&self, slices: &[&[u8]]) -> [u8; 32] {
        let mut hash = Sha256::new();
        for slice in slices {
            hash.update(slice);
        }
        hash.finalize().into()
    }
}

impl ReplayV3HashBackend for OperatorSha256V1 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        self.hashv(parts)
    }
}

impl std::error::Error for CanonicalActionMaterialErrorV1 {}

/// Slot boundary derived from the same bounded finalized acquisition as the
/// action inputs. A future launcher must acquire a recent blockhash separately
/// and discard this material after `valid_before_slot`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionFreshnessBoundaryV1 {
    pub observed_slot: u64,
    pub valid_before_slot: u64,
    pub maximum_validity_slots: u64,
}

/// Exact ordered account role retained by an opaque typed constructor. The
/// label is selected inside that constructor from the semantic owner's enum;
/// no public caller can construct this role from a string.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalAccountRoleV1 {
    label: &'static str,
    address: Address,
    writable: bool,
    signer: bool,
}

impl CanonicalAccountRoleV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        self.label
    }

    #[must_use]
    pub const fn address(self) -> Address {
        self.address
    }

    #[must_use]
    pub const fn writable(self) -> bool {
        self.writable
    }

    #[must_use]
    pub const fn signer(self) -> bool {
        self.signer
    }
}

impl ActionFreshnessBoundaryV1 {
    fn validate(self) -> Result<()> {
        let lifetime = self
            .valid_before_slot
            .checked_sub(self.observed_slot)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidFreshness)?;
        if self.observed_slot == 0
            || lifetime == 0
            || self.maximum_validity_slots == 0
            || lifetime > self.maximum_validity_slots
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidFreshness);
        }
        Ok(())
    }
}

/// Server-owned action artifact. Fields are intentionally private so a caller
/// cannot combine a valid release verdict with independently shaped accounts,
/// cursor, signer set, transaction bytes, or freshness claims.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalActionMaterialV1 {
    release_key: String,
    driver_release_key: String,
    release_manifest_sha256: [u8; 32],
    capability_profile_id: [u8; 32],
    coordinate: CanonicalIntentCoordinate,
    driver_account: Address,
    driver_account_slot: u64,
    cursor: ResumableWorkflowCursor,
    authority_state_sha256: [u8; 32],
    freshness: ActionFreshnessBoundaryV1,
    fee_payer: Address,
    account_roles: Vec<CanonicalAccountRoleV1>,
    planned: PlannedWorkflowNode,
    draft_id: [u8; 32],
}

impl CanonicalActionMaterialV1 {
    #[must_use]
    pub fn release_key(&self) -> &str {
        &self.release_key
    }

    #[must_use]
    pub fn driver_release_key(&self) -> &str {
        &self.driver_release_key
    }

    #[must_use]
    pub const fn release_manifest_sha256(&self) -> [u8; 32] {
        self.release_manifest_sha256
    }

    #[must_use]
    pub const fn capability_profile_id(&self) -> [u8; 32] {
        self.capability_profile_id
    }

    #[must_use]
    pub const fn coordinate(&self) -> CanonicalIntentCoordinate {
        self.coordinate
    }

    #[must_use]
    pub const fn driver_account(&self) -> Address {
        self.driver_account
    }

    #[must_use]
    pub const fn driver_account_slot(&self) -> u64 {
        self.driver_account_slot
    }

    #[must_use]
    pub const fn cursor(&self) -> ResumableWorkflowCursor {
        self.cursor
    }

    #[must_use]
    pub const fn authority_state_sha256(&self) -> [u8; 32] {
        self.authority_state_sha256
    }

    #[must_use]
    pub const fn freshness(&self) -> ActionFreshnessBoundaryV1 {
        self.freshness
    }

    #[must_use]
    pub const fn fee_payer(&self) -> Address {
        self.fee_payer
    }

    #[must_use]
    pub fn account_roles(&self) -> &[CanonicalAccountRoleV1] {
        &self.account_roles
    }

    #[must_use]
    pub fn unsigned_transaction(&self) -> &UnsignedProtocolTransaction {
        &self.planned.unsigned_transaction
    }

    #[must_use]
    pub const fn draft_id(&self) -> [u8; 32] {
        self.draft_id
    }

    #[must_use]
    pub const fn reload_authoritative_accounts(&self) -> bool {
        self.planned.reload_authoritative_accounts
    }

    /// Exact release/cursor join required before exposing this material as a
    /// callable verdict. Any rescan that changes the cursor invalidates it.
    #[must_use]
    pub fn matches(
        &self,
        release: &IndexedProgramRelease,
        coordinate: CanonicalIntentCoordinate,
        selection: &KeeperActionSelection,
    ) -> bool {
        self.release_key == release.key()
            && self.release_manifest_sha256 == release.release_manifest_sha256
            && self.capability_profile_id == release.capability_profile_id
            && self.coordinate == coordinate
            && self.driver_account == selection.account
            && self.driver_account_slot == selection.account_slot
            && self.cursor == selection.cursor
            && selection.release_key == self.driver_release_key
            && selection.effective_commitment == crate::rpc_index::RpcCommitment::Finalized
            && self.planned.reload_authoritative_accounts
            && !self
                .planned
                .unsigned_transaction
                .has_recent_blockhash
            && !self.planned.unsigned_transaction.signed
            && !self.planned.unsigned_transaction.submitted
    }
}

/// Construct one callable Structured wrapper draft from one exact finalized
/// current-account frame. The action, driver, generation, cursor position, and
/// later action-1 recipe leaf are detected from hostile-decoded semantic-owner
/// state; no generic keeper hint or browser-authored action selection enters
/// this boundary.
#[allow(clippy::too_many_arguments)]
pub fn construct_structured_action_material_v1(
    releases: StructuredOperatorReleaseSetV1<'_>,
    builder: &ProtocolTransactionBuilder,
    workflow_id: [u8; 32],
    freshness: ActionFreshnessBoundaryV1,
    accounts: &[StructuredChainAccountV1<'_>],
    lookup_table: &StructuredAddressLookupTableV1,
) -> Result<CanonicalActionMaterialV1> {
    freshness.validate()?;
    if workflow_id == [0; 32] {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    let schedule = detect_structured_schedule_v1(releases, accounts)?;
    let driver = accounts
        .get(schedule.driver_index)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let driver_account = driver
        .present
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let observed_state_sha256 = structured_authority_state_id(
        accounts,
        lookup_table.state_sha256,
        releases.collateral.receipt_id,
    );
    let dependencies = accounts
        .iter()
        .map(|account| account.address)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let selection = KeeperActionSelection {
        account: driver.address,
        release_key: releases.base.key(),
        action: structured_selection_action(schedule.action),
        cursor: ResumableWorkflowCursor {
            workflow_id,
            lane: WorkflowLane::StructuredLifecycle,
            generation: schedule.generation,
            position: WorkflowPosition {
                phase: u16::from(schedule.action.tag()),
                item: schedule.item,
            },
            observed_state_sha256,
        },
        account_slot: driver_account.provenance.slot,
        observed_commitment: RpcCommitment::Finalized,
        effective_commitment: RpcCommitment::Finalized,
        branch: IndexedBranch::FinalizedScan,
        dependencies,
    };
    construct_detected_structured_action_material_v1(
        releases,
        builder,
        &selection,
        freshness,
        accounts,
        lookup_table,
    )
}

/// Lowering boundary for the internally detected Structured schedule. Kept
/// private so a caller-shaped [`KeeperActionSelection`] cannot become current
/// action or recipe authority.
#[allow(clippy::too_many_arguments)]
fn construct_detected_structured_action_material_v1(
    releases: StructuredOperatorReleaseSetV1<'_>,
    builder: &ProtocolTransactionBuilder,
    selection: &KeeperActionSelection,
    freshness: ActionFreshnessBoundaryV1,
    accounts: &[StructuredChainAccountV1<'_>],
    lookup_table: &StructuredAddressLookupTableV1,
) -> Result<CanonicalActionMaterialV1> {
    freshness.validate()?;
    let action = structured_action_from_selection(selection.action)
        .ok_or(CanonicalActionMaterialErrorV1::WrongSelection)?;
    let coordinate = structured_coordinate(action);
    if releases.wrapper.enabled_intents.binary_search(&coordinate).is_err() {
        return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
    }
    if selection.release_key != releases.base.key()
        || selection.effective_commitment != RpcCommitment::Finalized
        || selection.observed_commitment != RpcCommitment::Finalized
        || freshness.observed_slot < selection.account_slot
        || lookup_table.observed_slot > freshness.observed_slot
        || builder.clutch_program() != releases.base.program_id
        || builder.clutch_release_sha256() != releases.base.elf_sha256
        || selection.cursor.lane != WorkflowLane::StructuredLifecycle
        || selection.cursor.position.phase != u16::from(action.tag())
        || selection.cursor.observed_state_sha256
            != structured_authority_state_id(
                accounts,
                lookup_table.state_sha256,
                releases.collateral.receipt_id,
            )
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    let contract = current_structured_action_contract_v1(action)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if accounts.len() != usize::from(contract.account_count)
        || !accounts.iter().any(|account| account.address == selection.account)
        || accounts.iter().any(|account| {
            account.address == Address::default()
                || account.observed_slot == 0
                || account.observed_slot > freshness.observed_slot
        })
        || accounts.iter().filter_map(|account| account.present).any(|account| {
            account.provenance.cluster_key != lookup_table.cluster_key
        })
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let derived = derive_structured_action_v1(releases, selection, accounts, action)?;
    let mut metas = Vec::with_capacity(accounts.len());
    let mut account_roles = Vec::with_capacity(accounts.len());
    let mut index = 0_usize;
    while index < accounts.len() {
        let expected = current_structured_account_meta_v1(
            action,
            index,
            derived.product_link_writable,
        )
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        if accounts[index].executable() != expected.executable
            && accounts[index].present.is_some()
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        metas.push(if expected.writable {
            AccountMeta::new(accounts[index].address, expected.signer)
        } else {
            AccountMeta::new_readonly(accounts[index].address, expected.signer)
        });
        account_roles.push(CanonicalAccountRoleV1 {
            label: expected.label,
            address: accounts[index].address,
            writable: expected.writable,
            signer: expected.signer,
        });
        index += 1;
    }
    if action == StructuredClaimActionV1::CreateDescriptor
        && builder.payer() != accounts[1].address
    {
        return Err(CanonicalActionMaterialErrorV1::FeePayerMismatch);
    }
    let equation = ExactEquation {
        name: "chain-derived structured transition quantity".into(),
        unit: IntegerUnit::WrapperAtoms {
            mint: derived.wrapper_mint,
        },
        left: u128::from(derived.quantity),
        right: u128::from(derived.quantity),
    };
    let draft = crate::transaction_builder::OwnedInstructionDraft::enabled_structured_claim_v1(
        structured_selection_action(action),
        crate::transaction_builder::SemanticOwner {
            package: "clutch-structured-claim-adapter".into(),
            schema: STRUCTURED_CURRENT_ACCOUNT_CONTRACT_LABEL_V1.into(),
            release_sha256: releases.wrapper.elf_sha256,
        },
        releases.wrapper.program_id,
        metas,
        vec![equation],
        action,
        derived.product_link_writable,
        &derived.payload,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let unsigned_transaction = builder
        .build_structured_v0(
            draft,
            lookup_table.table.clone(),
            lookup_table.observed_slot,
            lookup_table.state_sha256,
        )
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let planned = PlannedWorkflowNode {
        manifest_sha256: releases.wrapper.release_manifest_sha256,
        cursor: selection.cursor,
        coordinate: CanonicalActionCoordinate::StructuredClaim(action),
        unsigned_transaction,
        reload_authoritative_accounts: true,
    };
    validate_unsigned_structured_plan(
        coordinate,
        builder.payer(),
        &account_roles,
        &planned,
    )?;
    let release_key = releases.wrapper.key();
    let driver_release_key = releases.base.key();
    let authority_state_sha256 =
        structured_authority_state_id(
            accounts,
            lookup_table.state_sha256,
            releases.collateral.receipt_id,
        );
    let draft_id = action_material_id(
        &release_key,
        &driver_release_key,
        releases.wrapper.release_manifest_sha256,
        releases.wrapper.capability_profile_id,
        coordinate,
        selection.account,
        selection.account_slot,
        selection.cursor,
        authority_state_sha256,
        freshness,
        builder.payer(),
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key,
        driver_release_key,
        release_manifest_sha256: releases.wrapper.release_manifest_sha256,
        capability_profile_id: releases.wrapper.capability_profile_id,
        coordinate,
        driver_account: selection.account,
        driver_account_slot: selection.account_slot,
        cursor: selection.cursor,
        authority_state_sha256,
        freshness,
        fee_payer: builder.payer(),
        account_roles,
        planned,
        draft_id,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DetectedStructuredScheduleV1 {
    action: StructuredClaimActionV1,
    driver_index: usize,
    generation: u64,
    item: u64,
}

/// Partition the six current actions by exact account geometry and hostile
/// state. The partition is exhaustive only for a complete current frame and
/// explicitly refuses ambiguity rather than selecting the first match.
fn detect_structured_schedule_v1(
    releases: StructuredOperatorReleaseSetV1<'_>,
    accounts: &[StructuredChainAccountV1<'_>],
) -> Result<DetectedStructuredScheduleV1> {
    let action = match accounts.len() {
        35 => StructuredClaimActionV1::CreateDescriptor,
        32 => {
            let compact = accounts
                .get(10)
                .and_then(|account| account.data().ok())
                .is_some_and(|body| StructuredClaimDescriptorV2::decode(body).is_ok());
            let full = accounts
                .get(13)
                .and_then(|account| account.data().ok())
                .is_some_and(|body| StructuredClaimDescriptorV2::decode(body).is_ok());
            match (compact, full) {
                (true, false) => StructuredClaimActionV1::CompactDonation,
                (false, true) => detect_full_vector_direction_v1(accounts, false)?,
                _ => return Err(CanonicalActionMaterialErrorV1::InvalidChainState),
            }
        }
        33 => {
            let retire = accounts
                .get(10)
                .and_then(|account| account.data().ok())
                .is_some_and(|body| StructuredClaimDescriptorV2::decode(body).is_ok());
            let redeem = accounts
                .get(13)
                .and_then(|account| account.data().ok())
                .is_some_and(|body| StructuredClaimDescriptorV2::decode(body).is_ok());
            match (retire, redeem) {
                (true, false) => StructuredClaimActionV1::RetireDescriptor,
                (false, true) => detect_full_vector_direction_v1(accounts, true)?,
                _ => return Err(CanonicalActionMaterialErrorV1::InvalidChainState),
            }
        }
        _ => return Err(CanonicalActionMaterialErrorV1::InvalidChainState),
    };
    let (driver_index, generation, item) = if action
        == StructuredClaimActionV1::CreateDescriptor
    {
        let link = SeriesMarketLinkAccountV2::decode(accounts[26].data()?)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let leaf = detect_unique_create_leaf_v1(
            releases,
            accounts,
            link.state.binding().generation,
        )?;
        (26, link.state.binding().generation, u64::from(leaf))
    } else {
        let (vault_index, replay_index) = if action == StructuredClaimActionV1::WrapFull {
            (10, 11)
        } else {
            (8, 9)
        };
        let vault = PositionAccountV3::decode(accounts[vault_index].data()?)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let replay = ReplayV3Envelope::decode(accounts[replay_index].data()?, &OperatorSha256V1)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
            .header();
        let driver_index = if matches!(
            action,
            StructuredClaimActionV1::CompactDonation | StructuredClaimActionV1::RetireDescriptor
        ) {
            10
        } else {
            13
        };
        (driver_index, vault.generation(), replay.next_sequence())
    };
    Ok(DetectedStructuredScheduleV1 {
        action,
        driver_index,
        generation,
        item,
    })
}

fn detect_full_vector_direction_v1(
    accounts: &[StructuredChainAccountV1<'_>],
    terminal: bool,
) -> Result<StructuredClaimActionV1> {
    let source = PositionAccountV3::decode(accounts[8].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let destination = PositionAccountV3::decode(accounts[10].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    classify_full_vector_direction_v1(terminal, source.purpose(), destination.purpose())
}

/// Exhaustive direction classifier for the hostile-decoded Position purposes.
/// Only General<->StructuredClaim is a current Structured transition; Dealer
/// and Series positions can never be silently selected as an equivalent role.
fn classify_full_vector_direction_v1(
    terminal: bool,
    source: PositionPurposeV3,
    destination: PositionPurposeV3,
) -> Result<StructuredClaimActionV1> {
    match (terminal, source, destination) {
        (false, PositionPurposeV3::General, PositionPurposeV3::StructuredClaim) => {
            Ok(StructuredClaimActionV1::WrapFull)
        }
        (false, PositionPurposeV3::StructuredClaim, PositionPurposeV3::General) => {
            Ok(StructuredClaimActionV1::UnwrapFull)
        }
        (true, PositionPurposeV3::StructuredClaim, PositionPurposeV3::General) => {
            Ok(StructuredClaimActionV1::RedeemTerminal)
        }
        _ => Err(CanonicalActionMaterialErrorV1::InvalidChainState),
    }
}

fn detect_unique_create_leaf_v1(
    releases: StructuredOperatorReleaseSetV1<'_>,
    accounts: &[StructuredChainAccountV1<'_>],
    generation: u64,
) -> Result<u16> {
    let recipe_set = WrapperRecipeSetV1::decode(accounts[29].data()?, &OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let mut matched = None;
    let mut leaf = 0_u16;
    while leaf < recipe_set.leaf_count {
        let probe = KeeperActionSelection {
            account: accounts[26].address,
            release_key: releases.base.key(),
            action: structured_selection_action(StructuredClaimActionV1::CreateDescriptor),
            cursor: ResumableWorkflowCursor {
                workflow_id: [1; 32],
                lane: WorkflowLane::StructuredLifecycle,
                generation,
                position: WorkflowPosition {
                    phase: u16::from(StructuredClaimActionV1::CreateDescriptor.tag()),
                    item: u64::from(leaf),
                },
                observed_state_sha256: [1; 32],
            },
            account_slot: accounts[26].observed_slot,
            observed_commitment: RpcCommitment::Finalized,
            effective_commitment: RpcCommitment::Finalized,
            branch: IndexedBranch::FinalizedScan,
            dependencies: Vec::new(),
        };
        if derive_structured_action_v1(
            releases,
            &probe,
            accounts,
            StructuredClaimActionV1::CreateDescriptor,
        )
        .is_ok()
        {
            record_unique_create_leaf_v1(&mut matched, leaf)?;
        }
        leaf = leaf
            .checked_add(1)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    }
    matched.ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)
}

fn record_unique_create_leaf_v1(matched: &mut Option<u16>, leaf: u16) -> Result<()> {
    if matched.replace(leaf).is_some() {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct DerivedStructuredActionV1 {
    payload: Vec<u8>,
    wrapper_mint: Address,
    quantity: u64,
    product_link_writable: bool,
}

fn derive_structured_action_v1(
    releases: StructuredOperatorReleaseSetV1<'_>,
    selection: &KeeperActionSelection,
    accounts: &[StructuredChainAccountV1<'_>],
    action: StructuredClaimActionV1,
) -> Result<DerivedStructuredActionV1> {
    validate_structured_account_aliases_v1(accounts, action)?;
    validate_structured_release_accounts(releases, accounts, action)?;
    validate_common_structured_market_chain_v1(releases, accounts, action)?;
    if action == StructuredClaimActionV1::CreateDescriptor {
        derive_structured_create_v1(releases, selection, accounts)
    } else {
        derive_structured_current_mutation_v1(releases, selection, accounts, action)
    }
}

fn validate_structured_account_aliases_v1(
    accounts: &[StructuredChainAccountV1<'_>],
    action: StructuredClaimActionV1,
) -> Result<()> {
    let (collateral_program, collateral_data, token_program, token_data) = match action {
        StructuredClaimActionV1::CreateDescriptor => (7, 8, 19, 20),
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => (4, 5, 18, 19),
        StructuredClaimActionV1::CompactDonation
        | StructuredClaimActionV1::RetireDescriptor => (4, 5, 15, 16),
    };
    let same_token_release =
        accounts[collateral_program].address == accounts[token_program].address;
    let mut left = 0_usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let permitted_release_alias = structured_release_alias_is_permitted_v1(
                left,
                right,
                collateral_program,
                collateral_data,
                token_program,
                token_data,
                same_token_release,
            );
            if accounts[left].address == accounts[right].address && !permitted_release_alias {
                return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
const fn structured_release_alias_is_permitted_v1(
    left: usize,
    right: usize,
    collateral_program: usize,
    collateral_data: usize,
    token_program: usize,
    token_data: usize,
    same_token_release: bool,
) -> bool {
    same_token_release
        && ((left == collateral_program && right == token_program)
            || (left == collateral_data && right == token_data))
}

fn derive_structured_current_mutation_v1(
    releases: StructuredOperatorReleaseSetV1<'_>,
    selection: &KeeperActionSelection,
    accounts: &[StructuredChainAccountV1<'_>],
    action: StructuredClaimActionV1,
) -> Result<DerivedStructuredActionV1> {
    let (descriptor_index, basis_index, market_index, mint_index) = match action {
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => (13, 20, 21, 24),
        StructuredClaimActionV1::CompactDonation => (10, 17, 18, 21),
        StructuredClaimActionV1::RetireDescriptor => (10, 17, 18, 21),
        StructuredClaimActionV1::CreateDescriptor => {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan)
        }
    };
    let descriptor = StructuredClaimDescriptorV2::decode(accounts[descriptor_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if descriptor.state != DescriptorStateV1::Active
        || accounts[descriptor_index].owner()? != releases.wrapper.program_id
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let (
        wrapper_program_index,
        wrapper_data_index,
        base_program_index,
        base_data_index,
        token_program_index,
        token_data_index,
        wrapper_release_index,
        base_release_index,
        token_release_index,
    ) = match action {
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => (14, 15, 16, 17, 18, 19, 29, 30, 31),
        StructuredClaimActionV1::CompactDonation
        | StructuredClaimActionV1::RetireDescriptor => (11, 12, 13, 14, 15, 16, 29, 30, 31),
        StructuredClaimActionV1::CreateDescriptor => {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan)
        }
    };
    decode_release_artifact(
        releases.wrapper,
        releases.base.program_id,
        accounts[wrapper_program_index],
        accounts[wrapper_data_index],
        accounts[wrapper_release_index],
        STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1,
    )?;
    decode_release_artifact(
        releases.base,
        releases.base.program_id,
        accounts[base_program_index],
        accounts[base_data_index],
        accounts[base_release_index],
        STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1,
    )?;
    decode_release_artifact(
        releases.token_2022,
        releases.base.program_id,
        accounts[token_program_index],
        accounts[token_data_index],
        accounts[token_release_index],
        STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1,
    )?;
    let basis = NativeClaimBasisV1::decode(accounts[basis_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market = MarketInstancePreimageV2::decode(accounts[market_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let deployment = deployment_binding(releases);
    if descriptor.base_program != releases.base.program_id.to_bytes()
        || descriptor.base_program_data != releases.base.program_data.to_bytes()
        || descriptor.base_deployment_slot != releases.base.deployment_slot
        || descriptor.wrapper_program_data != releases.wrapper.program_data.to_bytes()
        || descriptor.wrapper_deployment_slot != releases.wrapper.deployment_slot
        || descriptor.token_2022_program != releases.token_2022.program_id.to_bytes()
        || descriptor.token_2022_program_data != releases.token_2022.program_data.to_bytes()
        || descriptor.token_2022_deployment_slot != releases.token_2022.deployment_slot
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    let basis_projection = DescriptorBasisV1 {
        market: market
            .id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
            .bytes(),
        terms_digest: basis
            .id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
            .bytes(),
        basis_degree: basis.basis_degree,
        denominator: basis.denominator,
        outcome_count: basis.outcome_count,
    };
    let identity = reconstruct_descriptor_identity_v1(&descriptor, basis_projection, deployment)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let native_claim_id = canonical_native_claim_id_v1(&identity)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let product = canonical_series_scoped_wrapper_product_id_v2(
        &identity,
        native_claim_id,
        descriptor.structured_root_id,
        descriptor.wrapper_recipe_id,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let addresses = structured_runtime_addresses(releases.wrapper.program_id, product);
    if accounts[descriptor_index].address != addresses.descriptor.0
        || accounts[mint_index].address != addresses.mint.0
        || descriptor.descriptor_bump != addresses.descriptor.1
        || descriptor.mint_bump != addresses.mint.1
        || descriptor.mint_authority_bump != addresses.mint_authority.1
        || descriptor.vault_owner_bump != addresses.vault_owner.1
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    if accounts[mint_index].owner()? != releases.token_2022.program_id
        || accounts[mint_index].executable()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let mint = decode_canonical_wrapper_mint_v1(
        releases.token_2022.program_id.to_bytes(),
        accounts[mint_index].address.to_bytes(),
        addresses.mint_authority.0.to_bytes(),
        accounts[mint_index].data()?,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let (hoard_index, claim_index) = match action {
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => (22, 23),
        StructuredClaimActionV1::CompactDonation
        | StructuredClaimActionV1::RetireDescriptor => (19, 20),
        StructuredClaimActionV1::CreateDescriptor => {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan)
        }
    };
    let hoard = HoardV2::decode(accounts[hoard_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let claim = ClaimLedgerV3::decode(accounts[claim_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if hoard.lifecycle == MarketLiabilityLifecycleV1::Retiring
        || hoard.lifecycle != claim.lifecycle
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let mint_authority_index = match action {
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => Some(26),
        StructuredClaimActionV1::RetireDescriptor => Some(22),
        StructuredClaimActionV1::CompactDonation => None,
        StructuredClaimActionV1::CreateDescriptor => {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan)
        }
    };
    if mint_authority_index.is_some_and(|index| {
        accounts[index].address != addresses.mint_authority.0
            || !system_identity_is_valid(accounts[index])
    }) {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    let (payload, quantity, product_link_writable) = match action {
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => {
            let actor = accounts[12].address;
            let source_purpose = if action == StructuredClaimActionV1::WrapFull {
                PositionPurposeV3::General
            } else {
                PositionPurposeV3::StructuredClaim
            };
            let destination_purpose = if action == StructuredClaimActionV1::WrapFull {
                PositionPurposeV3::StructuredClaim
            } else {
                PositionPurposeV3::General
            };
            let source_owner = if source_purpose == PositionPurposeV3::StructuredClaim {
                addresses.vault_owner.0
            } else {
                actor
            };
            let destination_owner = if destination_purpose == PositionPurposeV3::StructuredClaim {
                addresses.vault_owner.0
            } else {
                actor
            };
            let source_structured = (source_purpose == PositionPurposeV3::StructuredClaim)
                .then_some((
                    accounts[descriptor_index].address.to_bytes(),
                    product,
                    addresses.vault_owner.0.to_bytes(),
                ));
            let destination_structured =
                (destination_purpose == PositionPurposeV3::StructuredClaim).then_some((
                    accounts[descriptor_index].address.to_bytes(),
                    product,
                    addresses.vault_owner.0.to_bytes(),
                ));
            let (source, source_replay) = decode_current_position_pair_v1(
                releases.base.program_id,
                accounts,
                8,
                9,
                source_purpose,
                source_owner,
                descriptor.market,
                hoard.realm_id.bytes(),
                hoard.collateral_policy_id.bytes(),
                hoard.collateral_release_id.bytes(),
                source_structured,
            )?;
            let (destination, destination_replay) = decode_current_position_pair_v1(
                releases.base.program_id,
                accounts,
                10,
                11,
                destination_purpose,
                destination_owner,
                descriptor.market,
                hoard.realm_id.bytes(),
                hoard.collateral_policy_id.bytes(),
                hoard.collateral_release_id.bytes(),
                destination_structured,
            )?;
            let (user, user_replay, vault, vault_replay) = if action
                == StructuredClaimActionV1::WrapFull
            {
                (source, source_replay, destination, destination_replay)
            } else {
                (destination, destination_replay, source, source_replay)
            };
            if user.purpose() != PositionPurposeV3::General
                || vault.purpose() != PositionPurposeV3::StructuredClaim
                || user.controller().bytes() != actor.to_bytes()
                || user.purpose_binding_id().bytes() != accounts[7].address.to_bytes()
                || vault.purpose_binding_id().bytes() != product
                || vault.owner().bytes() != addresses.vault_owner.0.to_bytes()
                || selection.cursor.generation != vault.generation()
            {
                return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
            }
            if accounts[25].owner()? != releases.token_2022.program_id
                || accounts[25].executable()
            {
                return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
            }
            let holder = decode_canonical_wrapper_token_v1(
                releases.token_2022.program_id.to_bytes(),
                addresses.mint.0.to_bytes(),
                accounts[25].address.to_bytes(),
                actor.to_bytes(),
                accounts[25].data()?,
            )
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let backing = ClaimVector {
                outcome_count: basis.outcome_count,
                coefficients: descriptor.primitive,
            }
            .backing_plan()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            validate_wrapper_backing_floor_v1(backing, mint.supply, vault)?;
            let quantity = if action == StructuredClaimActionV1::WrapFull {
                maximum_full_vector_quantity(user, descriptor.primitive, basis.outcome_count)?
            } else {
                if holder.amount == 0 {
                    return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
                }
                holder.amount
            };
            if action == StructuredClaimActionV1::WrapFull {
                mint.supply
                    .checked_add(quantity)
                    .and_then(|_| holder.amount.checked_add(quantity))
                    .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
            } else {
                mint.supply
                    .checked_sub(quantity)
                    .and_then(|_| holder.amount.checked_sub(quantity))
                    .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
            }
            let terminal_residual_payout = if action == StructuredClaimActionV1::RedeemTerminal {
                Some(validate_terminal_resolution_trigger_v1(
                    releases.base.program_id,
                    accounts[32],
                    hoard,
                    claim,
                    basis,
                    backing,
                    quantity,
                )?)
            } else {
                None
            };
            validate_full_vector_transition_arithmetic_v1(
                action,
                quantity,
                descriptor.primitive,
                backing,
                user,
                vault,
                hoard,
                claim,
                terminal_residual_payout,
            )?;
            let value = WrapperQuantityPayloadV1 {
                wrapper_product_id: product,
                quantity,
                user_generation: user.generation(),
                user_replay_sequence: user_replay.next_sequence(),
                vault_generation: vault.generation(),
                vault_replay_sequence: vault_replay.next_sequence(),
            };
            (
                value
                    .encode()
                    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
                    .to_vec(),
                quantity,
                false,
            )
        }
        StructuredClaimActionV1::CompactDonation
        | StructuredClaimActionV1::RetireDescriptor => {
            let (vault, replay) = decode_current_position_pair_v1(
                releases.base.program_id,
                accounts,
                8,
                9,
                PositionPurposeV3::StructuredClaim,
                addresses.vault_owner.0,
                descriptor.market,
                hoard.realm_id.bytes(),
                hoard.collateral_policy_id.bytes(),
                hoard.collateral_release_id.bytes(),
                Some((
                    accounts[descriptor_index].address.to_bytes(),
                    product,
                    addresses.vault_owner.0.to_bytes(),
                )),
            )?;
            if vault.purpose() != PositionPurposeV3::StructuredClaim
                || vault.purpose_binding_id().bytes() != product
                || vault.owner().bytes() != addresses.vault_owner.0.to_bytes()
                || selection.cursor.generation != vault.generation()
            {
                return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
            }
            let root_index = if action == StructuredClaimActionV1::CompactDonation {
                26
            } else {
                23
            };
            let root = StructuredMarketRootV1::decode(accounts[root_index].data()?)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let root_pda = Address::find_program_address(
                &[b"dc:structured-root:v1", &descriptor.structured_root_id],
                &releases.base.program_id,
            );
            if root.binding.id(&OperatorSha256V1)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
                .bytes()
                != descriptor.structured_root_id
                || accounts[root_index].owner()? != releases.base.program_id
                || accounts[root_index].executable()
                || accounts[root_index].address != root_pda.0
                || root.root_bump != root_pda.1
                || root.binding.market_instance_id.bytes() != descriptor.market
                || root.live_descriptor_count == 0
                || root.binding.link_account
                    != accounts[if action == StructuredClaimActionV1::CompactDonation {
                        27
                    } else {
                        24
                    }]
                    .address
                    .to_bytes()
            {
                return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
            }
            let root = root
                .observe_lamport_balance(
                    accounts[root_index]
                        .present
                        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?
                        .lamports,
                )
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            validate_current_product_join(accounts, action, root)?;
            let backing = ClaimVector {
                outcome_count: basis.outcome_count,
                coefficients: descriptor.primitive,
            }
            .backing_plan()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            validate_wrapper_backing_floor_v1(backing, mint.supply, vault)?;
            if action == StructuredClaimActionV1::CompactDonation {
                validate_compaction_trigger_v1(
                    accounts,
                    releases.collateral.program.program_id,
                    mint.supply,
                    vault,
                    backing,
                    hoard,
                    claim,
                )?;
            } else {
                validate_retirement_trigger_v1(accounts, mint.supply, vault, replay, root)?;
            }
            let family_terminal = action == StructuredClaimActionV1::RetireDescriptor
                && root.live_descriptor_count == 1;
            let value = VaultMutationPayloadV1 {
                wrapper_product_id: product,
                vault_generation: vault.generation(),
                vault_replay_sequence: replay.next_sequence(),
            };
            (
                value
                    .encode()
                    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
                    .to_vec(),
                0,
                family_terminal,
            )
        }
        StructuredClaimActionV1::CreateDescriptor => {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan)
        }
    };
    Ok(DerivedStructuredActionV1 {
        payload,
        wrapper_mint: addresses.mint.0,
        quantity,
        product_link_writable,
    })
}

fn derive_structured_create_v1(
    releases: StructuredOperatorReleaseSetV1<'_>,
    selection: &KeeperActionSelection,
    accounts: &[StructuredChainAccountV1<'_>],
) -> Result<DerivedStructuredActionV1> {
    let link = SeriesMarketLinkAccountV2::decode(accounts[26].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let link_binding = link.state.binding();
    if accounts[26].owner()? != releases.base.program_id
        || link.state.phase() != SeriesMarketLinkPhaseV2::Active
        || selection.cursor.generation != link_binding.generation
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let link_pda = Address::find_program_address(
        &[
            b"dc:series-market-link:v1",
            &link_binding.series_plan_id.bytes(),
            &link_binding.ordinal.to_le_bytes(),
        ],
        &releases.base.program_id,
    );
    if accounts[26].address != link_pda.0 || link.stored_bump != link_pda.1 {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let bundle = CompiledProductSeriesBundleV6::decode(accounts[27].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let bundle_id = bundle
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let attachment = SeriesAttachmentPlanV5::decode(accounts[28].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let attachment_id = attachment
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if bundle_id != link_binding.compiler_bundle_id
        || attachment_id != link_binding.attachment_plan_id
        || bundle.series_plan_id != link_binding.series_plan_id
        || bundle.funding_terms_id != link_binding.funding_terms_id
        || bundle.funding_quote_id != link_binding.funding_quote_id
        || bundle.attachment_plan_id != link_binding.attachment_plan_id
        || bundle.capability_profile_id.content_id() != link_binding.capability_profile_id
        || attachment.funding_quote_id != link_binding.funding_quote_id
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    verify_product_artifact(
        releases.base.program_id,
        accounts[27],
        ArtifactKind::CompiledProductSeriesBundleV6,
        bundle_id.bytes(),
    )?;
    verify_product_artifact(
        releases.base.program_id,
        accounts[28],
        ArtifactKind::SeriesAttachmentPlanV5,
        attachment_id.bytes(),
    )?;
    let recipe_set = WrapperRecipeSetV1::decode(accounts[29].data()?, &OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let recipe_set_id = recipe_set
        .id(&OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if recipe_set_id != attachment.wrapper_recipe_set_id.bytes() {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    verify_product_artifact(
        releases.base.program_id,
        accounts[29],
        ArtifactKind::WrapperRecipeSetV1,
        recipe_set_id,
    )?;
    let leaf_index = u16::try_from(selection.cursor.position.item)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let (recipe, recipe_id, membership) = recipe_set
        .member(leaf_index, &OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;

    let registry = SeriesRegistryAccountV3::decode(accounts[30].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let base_release_id = decode_release_artifact(
        releases.base,
        releases.base.program_id,
        accounts[17],
        accounts[18],
        accounts[31],
        STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1,
    )?;
    let profile = RegistryCapabilityProfileV4::decode(accounts[32].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let profile_id = profile
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    verify_product_artifact(
        releases.base.program_id,
        accounts[32],
        ArtifactKind::RegistryCapabilityProfileV4,
        profile_id.bytes(),
    )?;
    let wrapper_release_id = decode_release_artifact(
        releases.wrapper,
        releases.base.program_id,
        accounts[15],
        accounts[16],
        accounts[33],
        STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1,
    )?;
    let token_release_id = decode_release_artifact(
        releases.token_2022,
        releases.base.program_id,
        accounts[19],
        accounts[20],
        accounts[34],
        STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1,
    )?;
    if !registry.activation_consumed
        || registry.series_plan_id != link_binding.series_plan_id
        || registry.funding_terms_id != link_binding.funding_terms_id
        || registry.registry_release_id != base_release_id
        || registry.capability_profile_id != profile_id.content_id()
        || registry.compiler_bundle_id != bundle_id
        || profile.registry_release_id().content_id() != base_release_id
        || profile_id.content_id() != link_binding.capability_profile_id
        || bundle.registry_release_id != base_release_id
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let basis = NativeClaimBasisV1::decode(accounts[21].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let basis_id = basis
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market = MarketInstancePreimageV2::decode(accounts[22].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_id = market
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    verify_product_artifact(
        releases.base.program_id,
        accounts[21],
        ArtifactKind::NativeClaimBasisV1,
        basis_id.bytes(),
    )?;
    verify_product_artifact(
        releases.base.program_id,
        accounts[22],
        ArtifactKind::MarketInstancePreimageV2,
        market_id.bytes(),
    )?;
    if basis_id != bundle.native_claim_basis_id
        || market_id != link_binding.market_instance_id
        || market.product_template_id != bundle.product_template_id
        || market.market_genesis_profile_id != bundle.market_genesis_profile_id
        || recipe.outcome_count != basis.outcome_count
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let deployment = deployment_binding(releases);
    let owner_release_id = structured_owner_release_id_v2(
        deployment,
        ContentId::from_bytes(wrapper_release_id.bytes()),
        ContentId::from_bytes(base_release_id.bytes()),
        ContentId::from_bytes(token_release_id.bytes()),
        &OperatorSha256V1,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let root_binding = StructuredMarketRootBindingV1 {
        link_account: accounts[26].address.to_bytes(),
        series_plan_id: link_binding.series_plan_id,
        ordinal: link_binding.ordinal,
        market_instance_id: link_binding.market_instance_id,
        generation: link_binding.generation,
        attachment_plan_id: link_binding.attachment_plan_id,
        compiler_output_id: link_binding.compiler_bundle_id,
        compiler_release_id: bundle.product_compiler_release_id,
        registry_release_id: ContentId::from_bytes(base_release_id.bytes()),
        capability_profile_id: link_binding.capability_profile_id,
        wrapper_recipe_set_id: ContentId::from_bytes(recipe_set_id),
        owner_release_id,
        rent_refund_owner: link_binding.rent_refund_owner,
        neutral_lamport_sink: link_binding.neutral_lamport_sink,
    };
    let root_id = root_binding
        .id(&OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let mut descriptor = StructuredClaimDescriptorV2 {
        tag: DESCRIPTOR_ACCOUNT_TAG,
        version: DESCRIPTOR_ACCOUNT_VERSION,
        flags: 0,
        base_program: releases.base.program_id.to_bytes(),
        base_program_data: releases.base.program_data.to_bytes(),
        base_deployment_slot: releases.base.deployment_slot,
        wrapper_program_data: releases.wrapper.program_data.to_bytes(),
        wrapper_deployment_slot: releases.wrapper.deployment_slot,
        token_2022_program: releases.token_2022.program_id.to_bytes(),
        token_2022_program_data: releases.token_2022.program_data.to_bytes(),
        token_2022_deployment_slot: releases.token_2022.deployment_slot,
        market: market_id.bytes(),
        terms_digest: basis_id.bytes(),
        structured_root_id: root_id.bytes(),
        wrapper_recipe_id: recipe_id,
        primitive: recipe.primitive,
        state: DescriptorStateV1::Active,
        descriptor_bump: 1,
        mint_bump: 1,
        mint_authority_bump: 1,
        vault_owner_bump: 1,
    };
    let basis_projection = DescriptorBasisV1 {
        market: market_id.bytes(),
        terms_digest: basis_id.bytes(),
        basis_degree: basis.basis_degree,
        denominator: basis.denominator,
        outcome_count: basis.outcome_count,
    };
    let identity = reconstruct_descriptor_identity_v1(&descriptor, basis_projection, deployment)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let native_claim_id = canonical_native_claim_id_v1(&identity)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if native_claim_id != recipe.native_claim_id {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let product = canonical_series_scoped_wrapper_product_id_v2(
        &identity,
        native_claim_id,
        root_id.bytes(),
        recipe_id,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let addresses = structured_runtime_addresses(releases.wrapper.program_id, product);
    if !system_identity_is_valid(accounts[0]) {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    descriptor.descriptor_bump = addresses.descriptor.1;
    descriptor.mint_bump = addresses.mint.1;
    descriptor.mint_authority_bump = addresses.mint_authority.1;
    descriptor.vault_owner_bump = addresses.vault_owner.1;
    descriptor
        .encode()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let root_pda = Address::find_program_address(
        &[b"dc:structured-root:v1", &root_id.bytes()],
        &releases.base.program_id,
    );
    let purpose = [u8::from(PositionPurposeV3::StructuredClaim)];
    let position_pda = Address::find_program_address(
        &[
            POSITION_V3_PDA_PREFIX,
            &market_id.bytes(),
            &addresses.vault_owner.0.to_bytes(),
            &purpose,
            &product,
        ],
        &releases.base.program_id,
    );
    let replay_pda = Address::find_program_address(
        &[
            PURPOSE_REPLAY_V3_PDA_PREFIX,
            &position_pda.0.to_bytes(),
            &purpose,
            &product,
        ],
        &releases.base.program_id,
    );
    if accounts[0].address != addresses.vault_owner.0
        || accounts[11].address != position_pda.0
        || accounts[12].address != replay_pda.0
        || accounts[13].address != addresses.descriptor.0
        || accounts[14].address != addresses.mint.0
        || accounts[25].address != root_pda.0
        || accounts[13].present.is_some()
        || accounts[14].present.is_some()
        || accounts[11].present.is_some()
        || accounts[12].present.is_some()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let product_link_writable = accounts[25].present.is_none();
    let wrapper_status = link.state.obligation_status(SeriesLinkObligationV2::Wrapper);
    if (product_link_writable
        && wrapper_status != SeriesLinkObligationStatusV2::EnabledNeverFounded)
        || (!product_link_writable && wrapper_status != SeriesLinkObligationStatusV2::Live)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    if let Some(root_account) = accounts[25].present {
        let root = StructuredMarketRootV1::decode(&root_account.data)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let current_link_binding_id = link_binding
            .id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        if root_account.owner != releases.base.program_id
            || root_account.executable
            || root.binding != root_binding
            || root.root_bump != root_pda.1
            || root.live_descriptor_count == 0
            || root.product_lineage.link_binding_id != current_link_binding_id
            || root.product_lineage.wrapper_obligation_configuration_id
                != link_binding.obligation_configuration_id.content_id()
            || root.product_lineage.product_admission_receipt_id
                != link
                    .state
                    .obligation_admission_receipt_id(SeriesLinkObligationV2::Wrapper)
            || link.state.transition_sequence()
                < root.product_lineage.last_observed_link_transition_sequence
            || root
                .observe_lamport_balance(root_account.lamports)
                .is_err()
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    let payload = CreateDescriptorPayloadV1 {
        native_claim_id,
        wrapper_product_id: product,
        structured_root_id: root_id.bytes(),
        wrapper_recipe_id: recipe_id,
        primitive: recipe.primitive,
        recipe_membership: membership,
    }
    .encode()
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
    .to_vec();
    Ok(DerivedStructuredActionV1 {
        payload,
        wrapper_mint: addresses.mint.0,
        quantity: 0,
        product_link_writable,
    })
}

#[derive(Clone, Copy, Debug)]
struct StructuredRuntimeAddressesV1 {
    descriptor: (Address, u8),
    mint: (Address, u8),
    mint_authority: (Address, u8),
    vault_owner: (Address, u8),
}

fn structured_runtime_addresses(
    wrapper_program: Address,
    product: [u8; 32],
) -> StructuredRuntimeAddressesV1 {
    StructuredRuntimeAddressesV1 {
        descriptor: Address::find_program_address(&[DESCRIPTOR_SEED, &product], &wrapper_program),
        mint: Address::find_program_address(&[MINT_SEED, &product], &wrapper_program),
        mint_authority: Address::find_program_address(
            &[MINT_AUTHORITY_SEED, &product],
            &wrapper_program,
        ),
        vault_owner: Address::find_program_address(
            &[VAULT_OWNER_SEED, &product],
            &wrapper_program,
        ),
    }
}

const fn deployment_binding(releases: StructuredOperatorReleaseSetV1<'_>) -> DeploymentBinding {
    DeploymentBinding {
        wrapper_program: releases.wrapper.program_id.to_bytes(),
        wrapper_program_data: releases.wrapper.program_data.to_bytes(),
        wrapper_deployment_slot: releases.wrapper.deployment_slot,
        base_program: releases.base.program_id.to_bytes(),
        base_program_data: releases.base.program_data.to_bytes(),
        base_deployment_slot: releases.base.deployment_slot,
        token_2022_program: releases.token_2022.program_id.to_bytes(),
        token_2022_program_data: releases.token_2022.program_data.to_bytes(),
        token_2022_deployment_slot: releases.token_2022.deployment_slot,
    }
}

fn validate_structured_release_accounts(
    releases: StructuredOperatorReleaseSetV1<'_>,
    accounts: &[StructuredChainAccountV1<'_>],
    action: StructuredClaimActionV1,
) -> Result<()> {
    let (wrapper_program, wrapper_data, base_program, base_data, token_program, token_data) =
        match action {
            StructuredClaimActionV1::CreateDescriptor => (15, 16, 17, 18, 19, 20),
            StructuredClaimActionV1::WrapFull
            | StructuredClaimActionV1::UnwrapFull
            | StructuredClaimActionV1::RedeemTerminal => (14, 15, 16, 17, 18, 19),
            StructuredClaimActionV1::CompactDonation
            | StructuredClaimActionV1::RetireDescriptor => (11, 12, 13, 14, 15, 16),
        };
    if accounts[wrapper_program].address != releases.wrapper.program_id
        || accounts[wrapper_data].address != releases.wrapper.program_data
        || accounts[base_program].address != releases.base.program_id
        || accounts[base_data].address != releases.base.program_data
        || accounts[token_program].address != releases.token_2022.program_id
        || accounts[token_data].address != releases.token_2022.program_data
        || !accounts[wrapper_program].executable()
        || !accounts[base_program].executable()
        || !accounts[token_program].executable()
        || accounts[wrapper_data].executable()
        || accounts[base_data].executable()
        || accounts[token_data].executable()
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    Ok(())
}

/// Reconstruct the common Realm/Profile/collateral/General-market account
/// graph from semantic bodies and canonical PDA seeds. No ordered account
/// address in this graph is accepted merely because a caller placed it at the
/// right index.
fn validate_common_structured_market_chain_v1(
    releases: StructuredOperatorReleaseSetV1<'_>,
    accounts: &[StructuredChainAccountV1<'_>],
    action: StructuredClaimActionV1,
) -> Result<()> {
    let (
        realm_index,
        profile_index,
        policy_index,
        collateral_program_index,
        collateral_data_index,
        binding_index,
        runtime_index,
        basis_index,
        market_index,
        hoard_index,
        claim_index,
        collateral_mint_index,
        hoard_token_index,
        hoard_authority_index,
    ) = match action {
        StructuredClaimActionV1::CreateDescriptor => {
            (4, 5, 6, 7, 8, 9, 10, 21, 22, 23, 24, None, None, None)
        }
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => (
            1,
            2,
            3,
            4,
            5,
            6,
            7,
            20,
            21,
            22,
            23,
            Some(27),
            Some(28),
            None,
        ),
        StructuredClaimActionV1::CompactDonation => (
            1,
            2,
            3,
            4,
            5,
            6,
            7,
            17,
            18,
            19,
            20,
            Some(22),
            Some(23),
            Some(24),
        ),
        StructuredClaimActionV1::RetireDescriptor => {
            (1, 2, 3, 4, 5, 6, 7, 17, 18, 19, 20, None, None, None)
        }
    };
    let base_program = releases.base.program_id;
    for index in [
        realm_index,
        profile_index,
        policy_index,
        binding_index,
        runtime_index,
        basis_index,
        market_index,
        hoard_index,
        claim_index,
    ] {
        if accounts[index].owner()? != base_program || accounts[index].executable() {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    if accounts[collateral_program_index].address != releases.collateral.program.program_id
        || accounts[collateral_data_index].address != releases.collateral.program.program_data
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    authenticate_indexed_loader_release(
        releases.collateral.program,
        releases.collateral.artifact,
        accounts[collateral_program_index],
        accounts[collateral_data_index],
    )?;

    let realm = RealmAccount::decode(accounts[realm_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let profile = ProfileAccount::decode(accounts[profile_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let policy = CollateralPolicyV2::decode(accounts[policy_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let policy_id = policy
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let collateral_release_id = releases
        .collateral
        .adapter
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    policy
        .validate_for_release(&releases.collateral.adapter)
        .map_err(|_| CanonicalActionMaterialErrorV1::ReleaseMismatch)?;
    if profile.realm != realm.realm
        || profile.profile != realm.profile
        || profile.collateral_policy_id.bytes() != policy_id.bytes()
        || profile.adapter_release_id.bytes() != collateral_release_id.bytes()
        || policy.token_program.bytes() != accounts[collateral_program_index].address.to_bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let realm_pda = Address::find_program_address(
        &[REALM_PDA_SEED_V1, &realm.realm.bytes()],
        &base_program,
    );
    let profile_pda = Address::find_program_address(
        &[PROFILE_PDA_SEED_V1, &realm.realm.bytes(), &profile.profile.bytes()],
        &base_program,
    );
    let policy_pda = Address::find_program_address(
        &[
            COLLATERAL_POLICY_PDA_SEED_V1,
            &profile.profile.bytes(),
            &policy_id.bytes(),
        ],
        &base_program,
    );
    if accounts[realm_index].address != realm_pda.0
        || realm.stored_bump != realm_pda.1
        || accounts[profile_index].address != profile_pda.0
        || accounts[policy_index].address != policy_pda.0
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    let binding = MarketBindingV2::decode(accounts[binding_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let runtime = MarketRuntimeV3AccountV1::decode(accounts[runtime_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let binding_body = binding.base();
    let binding_pda = Address::find_program_address(
        &[
            MARKET_BINDING_SEED_DOMAIN_V1,
            &binding_body.market_instance_v2_id.bytes(),
        ],
        &base_program,
    );
    let runtime_pda = Address::find_program_address(
        &[
            MARKET_RUNTIME_SEED_DOMAIN_V1,
            &accounts[binding_index].address.to_bytes(),
        ],
        &base_program,
    );
    if accounts[binding_index].address != binding_pda.0
        || binding_body.stored_bump != binding_pda.1
        || accounts[runtime_index].address != runtime_pda.0
        || runtime.stored_bump != runtime_pda.1
        || binding_body.market.bytes() != accounts[runtime_index].address.to_bytes()
        || runtime.market_binding.bytes() != accounts[binding_index].address.to_bytes()
        || runtime.market_instance_v2_id != binding_body.market_instance_v2_id
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    let basis = NativeClaimBasisV1::decode(accounts[basis_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let basis_id = basis
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market = MarketInstancePreimageV2::decode(accounts[market_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_id = market
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    verify_product_artifact(
        base_program,
        accounts[basis_index],
        ArtifactKind::NativeClaimBasisV1,
        basis_id.bytes(),
    )?;
    verify_product_artifact(
        base_program,
        accounts[market_index],
        ArtifactKind::MarketInstancePreimageV2,
        market_id.bytes(),
    )?;
    if market_id.bytes() != binding_body.market_instance_v2_id.bytes()
        || basis_id.bytes() != binding_body.native_claim_basis_id.bytes()
        || market.market_genesis_profile_id.bytes()
            != binding_body.market_genesis_profile_v2_id.bytes()
        || basis.outcome_count != binding_body.outcome_count
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    let hoard = HoardV2::decode(accounts[hoard_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let claim = ClaimLedgerV3::decode(accounts[claim_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_bytes = market_id.bytes();
    let hoard_pda = Address::find_program_address(
        &[HOARD_V2_PDA_SEED_V1, &market_bytes],
        &base_program,
    );
    let claim_pda = Address::find_program_address(
        &[CLAIM_LEDGER_V3_PDA_SEED_V1, &market_bytes],
        &base_program,
    );
    let hoard_authority = Address::find_program_address(
        &[HOARD_AUTHORITY_V2_PDA_SEED_V1, &market_bytes],
        &base_program,
    );
    let hoard_token = Address::find_program_address(
        &[HOARD_TOKEN_V2_PDA_SEED_V1, &market_bytes],
        &base_program,
    );
    if accounts[hoard_index].address != hoard_pda.0
        || hoard.stored_bump != hoard_pda.1
        || accounts[claim_index].address != claim_pda.0
        || claim.stored_bump != claim_pda.1
        || hoard.market_instance_id.bytes() != market_bytes
        || hoard.realm_id.bytes() != realm.realm.bytes()
        || hoard.profile_id.bytes() != profile.profile.bytes()
        || hoard.collateral_policy_id.bytes() != policy_id.bytes()
        || hoard.collateral_release_id.bytes() != collateral_release_id.bytes()
        || hoard.authority.bytes() != hoard_authority.0.to_bytes()
        || hoard.token_account.bytes() != hoard_token.0.to_bytes()
        || hoard.collateral_cap_atoms != market.collateral_cap
        || claim.market_instance_id != hoard.market_instance_id
        || claim.realm_id != hoard.realm_id
        || claim.native_claim_basis_id.bytes() != basis_id.bytes()
        || claim.lifecycle != hoard.lifecycle
        || claim.outcome_count != hoard.outcome_count
        || claim.outcome_count != binding_body.outcome_count
        || !rent_is_covered(hoard.rent, accounts[hoard_index].present)
        || !rent_is_covered(claim.rent, accounts[claim_index].present)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    if let Some(index) = collateral_mint_index {
        if accounts[index].address.to_bytes() != policy.mint.bytes()
            || accounts[index].owner()? != accounts[collateral_program_index].address
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    if let Some(index) = hoard_token_index {
        if accounts[index].address != hoard_token.0
            || accounts[index].owner()? != accounts[collateral_program_index].address
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    if let Some(index) = hoard_authority_index {
        if accounts[index].address != hoard_authority.0
            || !system_identity_is_valid(accounts[index])
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    if (action == StructuredClaimActionV1::CreateDescriptor
        && (accounts[2].address != solana_sdk_ids::system_program::ID
            || accounts[3].address != solana_sdk_ids::sysvar::rent::ID))
        || (action == StructuredClaimActionV1::RetireDescriptor
            && accounts[32].address != solana_sdk_ids::system_program::ID)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok(())
}

fn rent_is_covered(
    rent: clutch_retirement::DeletableRentOwnerV1,
    account: Option<&ObservedRpcAccount>,
) -> bool {
    let Some(account) = account else {
        return false;
    };
    rent.refundable_principal()
        .checked_add(rent.donation_floor())
        .is_some_and(|required| account.lamports >= required)
}

fn position_rent_is_covered(
    rent: clutch_retirement::RentSplitV2,
    account: Option<&ObservedRpcAccount>,
) -> bool {
    let Some(account) = account else {
        return false;
    };
    rent.refundable_live_principal
        .checked_add(rent.permanent_tombstone_principal)
        .and_then(|principal| principal.checked_add(rent.donation_floor))
        .is_some_and(|required| account.lamports >= required)
}

fn system_identity_is_valid(account: StructuredChainAccountV1<'_>) -> bool {
    account.present.is_none_or(|value| {
        value.owner == solana_sdk_ids::system_program::ID
            && !value.executable
            && value.data.is_empty()
    })
}

#[allow(clippy::too_many_arguments)]
fn decode_current_position_pair_v1(
    base_program: Address,
    accounts: &[StructuredChainAccountV1<'_>],
    position_index: usize,
    replay_index: usize,
    purpose: PositionPurposeV3,
    expected_owner: Address,
    expected_market: [u8; 32],
    expected_realm: [u8; 32],
    expected_policy: [u8; 32],
    expected_release: [u8; 32],
    structured_extension: Option<([u8; 32], [u8; 32], [u8; 32])>,
) -> Result<(PositionAccountV3, clutch_retirement::ReplayV3EnvelopeHeader)> {
    let position = PositionAccountV3::decode(accounts[position_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let replay = ReplayV3Envelope::decode(accounts[replay_index].data()?, &OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let header = replay.header();
    let purpose_seed = [u8::from(purpose)];
    let position_pda = Address::find_program_address(
        &[
            POSITION_V3_PDA_PREFIX,
            &expected_market,
            &expected_owner.to_bytes(),
            &purpose_seed,
            &position.purpose_binding_id().bytes(),
        ],
        &base_program,
    );
    let replay_pda = Address::find_program_address(
        &[
            PURPOSE_REPLAY_V3_PDA_PREFIX,
            &accounts[position_index].address.to_bytes(),
            &purpose_seed,
            &position.purpose_binding_id().bytes(),
        ],
        &base_program,
    );
    if accounts[position_index].owner()? != base_program
        || accounts[replay_index].owner()? != base_program
        || accounts[position_index].executable()
        || accounts[replay_index].executable()
        || position.lifecycle() != PositionLifecycleV3::Open
        || position.purpose() != purpose
        || position.market_instance_id().bytes() != expected_market
        || position.realm_id().bytes() != expected_realm
        || position.collateral_policy_id().bytes() != expected_policy
        || position.collateral_release_id().bytes() != expected_release
        || position.owner().bytes() != expected_owner.to_bytes()
        || position.replay_account().bytes() != accounts[replay_index].address.to_bytes()
        || accounts[position_index].address != position_pda.0
        || position.stored_bump() != position_pda.1
        || accounts[replay_index].address != replay_pda.0
        || header.stored_bump() != replay_pda.1
        || header.lifecycle() != ReplayV3Lifecycle::Live
        || header.position_account().bytes() != accounts[position_index].address.to_bytes()
        || header.replay_account().bytes() != accounts[replay_index].address.to_bytes()
        || header.purpose() != purpose
        || header.purpose_binding_id() != position.purpose_binding_id()
        || header.position_generation() != position.generation()
        || !position_rent_is_covered(position.rent(), accounts[position_index].present)
        || !rent_is_covered(header.rent(), accounts[replay_index].present)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    if let Some((descriptor, product, vault_authority)) = structured_extension {
        let extension = StructuredClaimReplayExtensionV1::decode(replay.extension())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        if position.purpose_binding_id().bytes() != product
            || position.controller().bytes() != vault_authority
            || position.reserved_cash_atoms() != 0
            || extension.descriptor_account != descriptor
            || extension.wrapper_product_id != product
            || extension.vault_authority != vault_authority
            || extension.current_position_semantic_id
                != position
                    .semantic_id(&OperatorSha256V1)
                    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
                    .bytes()
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    Ok((position, header))
}

fn decode_release_artifact(
    release: &IndexedProgramRelease,
    artifact_owner: Address,
    program: StructuredChainAccountV1<'_>,
    program_data: StructuredChainAccountV1<'_>,
    account: StructuredChainAccountV1<'_>,
    expected_manifest: [u8; 32],
) -> Result<ContentId> {
    if account.owner()? != artifact_owner {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let value = RegistryProgramReleaseV2::decode(account.data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let release_id = value
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
        .content_id();
    if value.program.bytes() != release.program_id.to_bytes()
        || value.programdata.bytes() != release.program_data.to_bytes()
        || value.deployment_slot != release.deployment_slot
        || value.locus != RegistryReleaseLocusV2::ObservedPositive
        || value.capability_manifest_id.bytes() != expected_manifest
        || release.release_manifest_sha256 != expected_manifest
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    authenticate_indexed_loader_release(release, value, program, program_data)?;
    verify_product_artifact(
        artifact_owner,
        account,
        ArtifactKind::RegistryProgramReleaseV2,
        release_id.bytes(),
    )?;
    Ok(release_id)
}

const UPGRADEABLE_PROGRAM_METADATA_BYTES: usize = 36;
const UPGRADEABLE_PROGRAMDATA_METADATA_BYTES: usize = 45;

/// Hostile-decode the same exact upgradeable-loader facts consumed by the
/// on-chain wrapper. The checked operator release separately pins the ELF
/// suffix while RegistryProgramReleaseV2 pins the complete ProgramData body.
fn authenticate_indexed_loader_release(
    release: &IndexedProgramRelease,
    artifact: RegistryProgramReleaseV2,
    program: StructuredChainAccountV1<'_>,
    program_data: StructuredChainAccountV1<'_>,
) -> Result<()> {
    let program_body = program.data()?;
    let program_data_body = program_data.data()?;
    if program.address != release.program_id
        || program_data.address != release.program_data
        || program.owner()? != solana_sdk_ids::bpf_loader_upgradeable::ID
        || program_data.owner()? != solana_sdk_ids::bpf_loader_upgradeable::ID
        || !program.executable()
        || program_data.executable()
        || program_body.len() < UPGRADEABLE_PROGRAM_METADATA_BYTES
        || program_data_body.len() < UPGRADEABLE_PROGRAMDATA_METADATA_BYTES
        || program_body.get(0..4) != Some(2_u32.to_le_bytes().as_slice())
        || program_data_body.get(0..4) != Some(3_u32.to_le_bytes().as_slice())
        || program_body.get(4..36) != Some(release.program_data.to_bytes().as_slice())
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    let deployment_slot = read_u64_le(program_data_body, 4)?;
    match program_data_body[12] {
        0 => {}
        1 if program_data_body[13..45].iter().any(|byte| *byte != 0) => {}
        _ => return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch),
    }
    let complete_program_data_sha256: [u8; 32] = Sha256::digest(program_data_body).into();
    let elf_sha256: [u8; 32] =
        Sha256::digest(&program_data_body[UPGRADEABLE_PROGRAMDATA_METADATA_BYTES..]).into();
    if deployment_slot == 0
        || deployment_slot != release.deployment_slot
        || artifact.deployment_slot != deployment_slot
        || artifact.programdata_sha256.bytes() != complete_program_data_sha256
        || release.elf_sha256 != elf_sha256
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    Ok(())
}

fn verify_product_artifact(
    program_id: Address,
    account: StructuredChainAccountV1<'_>,
    kind: ArtifactKind,
    semantic_id: [u8; 32],
) -> Result<()> {
    let kind_seed = [kind.byte()];
    let expected = Address::find_program_address(
        &[b"dc:product-artifact:v1", &kind_seed, &semantic_id],
        &program_id,
    );
    if account.address != expected.0 || account.owner()? != program_id || account.executable() {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok(())
}

fn structured_chain_state_id(accounts: &[StructuredChainAccountV1<'_>]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator/structured-chain-state/v1\0");
    hash.update(u64::try_from(accounts.len()).unwrap_or(u64::MAX).to_le_bytes());
    for account in accounts {
        hash.update(account.address.to_bytes());
        hash.update(account.observed_slot.to_le_bytes());
        if let Some(present) = account.present {
            hash.update([1]);
            hash.update(present.owner.to_bytes());
            hash.update(present.lamports.to_le_bytes());
            hash.update([u8::from(present.executable)]);
            hash.update(present.rent_epoch.to_le_bytes());
            hash.update(Sha256::digest(&present.data));
            hash_text(&mut hash, &present.provenance.release_key);
        } else {
            hash.update([0]);
        }
    }
    hash.finalize().into()
}

fn structured_authority_state_id(
    accounts: &[StructuredChainAccountV1<'_>],
    lookup_table_state_sha256: [u8; 32],
    collateral_catalog_receipt_id: [u8; 32],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator/structured-authority-state/v1\0");
    hash.update(structured_chain_state_id(accounts));
    hash.update(lookup_table_state_sha256);
    hash.update(collateral_catalog_receipt_id);
    hash.finalize().into()
}

fn read_u64_le(data: &[u8], offset: usize) -> Result<u64> {
    let end = offset
        .checked_add(8)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let bytes: [u8; 8] = data
        .get(offset..end)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?
        .try_into()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    Ok(u64::from_le_bytes(bytes))
}

fn maximum_full_vector_quantity(
    position: PositionAccountV3,
    primitive: [u64; clutch_structured_claim::MAX_OUTCOMES],
    outcome_count: u8,
) -> Result<u64> {
    if position.outcome_count() != outcome_count {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let eggs = position.native_eggs();
    let mut maximum = u64::MAX;
    let mut constrained = false;
    let mut index = 0_usize;
    while index < usize::from(outcome_count) {
        if primitive[index] != 0 {
            constrained = true;
            maximum = core::cmp::min(maximum, eggs[index] / primitive[index]);
        }
        index += 1;
    }
    if !constrained || maximum == 0 || maximum == u64::MAX {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok(maximum)
}

fn validate_wrapper_backing_floor_v1(
    backing: clutch_structured_claim::BackingPlan,
    wrapper_supply: u64,
    vault: PositionAccountV3,
) -> Result<()> {
    let required_cash = wrapper_supply
        .checked_mul(backing.cash_per_wrapper)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if vault.reserved_cash_atoms() != 0 || vault.cash_atoms() < required_cash {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let internal = vault.native_eggs();
    let mut outcome = 0_usize;
    while outcome < usize::from(backing.outcome_count) {
        let required = wrapper_supply
            .checked_mul(backing.residual_eggs_per_wrapper[outcome])
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        if internal[outcome] < required {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        outcome += 1;
    }
    Ok(())
}

fn validate_terminal_resolution_trigger_v1(
    base_program: Address,
    account: StructuredChainAccountV1<'_>,
    hoard: HoardV2,
    claim: ClaimLedgerV3,
    basis: NativeClaimBasisV1,
    backing: clutch_structured_claim::BackingPlan,
    quantity: u64,
) -> Result<u64> {
    if account.owner()? != base_program || account.executable() {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let resolution = ResolutionV5::decode(account.data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let resolution_pda = Address::find_program_address(
        &[b"dc:resolution:v5", &hoard.market_instance_id.bytes()],
        &base_program,
    );
    if hoard.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || claim.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || resolution.state != ResolutionStateV5::Finalized
        || claim.resolution_account.bytes() != account.address.to_bytes()
        || account.address != resolution_pda.0
        || resolution.stored_bump != resolution_pda.1
        || resolution.facts.market_instance_id != hoard.market_instance_id
        || resolution.facts.native_claim_basis_id != claim.native_claim_basis_id
        || resolution.facts.native_claim_basis_id.bytes()
            != basis
                .id()
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
                .bytes()
        || resolution.facts.outcome_count != backing.outcome_count
        || !rent_is_covered(resolution.rent, account.present)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let mut numerator = 0_u128;
    let mut outcome = 0_usize;
    while outcome < usize::from(backing.outcome_count) {
        let residual = quantity
            .checked_mul(backing.residual_eggs_per_wrapper[outcome])
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        numerator = numerator
            .checked_add(
                u128::from(residual)
                    .checked_mul(u128::from(resolution.facts.payout_weights[outcome]))
                    .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?,
            )
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        outcome += 1;
    }
    if numerator % u128::from(resolution.facts.payout_denominator) != 0 {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    u64::try_from(numerator / u128::from(resolution.facts.payout_denominator))
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)
}

#[allow(clippy::too_many_arguments)]
fn validate_full_vector_transition_arithmetic_v1(
    action: StructuredClaimActionV1,
    quantity: u64,
    primitive: [u64; clutch_structured_claim::MAX_OUTCOMES],
    backing: clutch_structured_claim::BackingPlan,
    user: PositionAccountV3,
    vault: PositionAccountV3,
    hoard: HoardV2,
    claim: ClaimLedgerV3,
    terminal_residual_payout: Option<u64>,
) -> Result<()> {
    let complete_cash = quantity
        .checked_mul(backing.cash_per_wrapper)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let user_internal = user.native_eggs();
    let vault_internal = vault.native_eggs();
    let mut outcome = 0_usize;
    while outcome < usize::from(backing.outcome_count) {
        let full = quantity
            .checked_mul(primitive[outcome])
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let residual = quantity
            .checked_mul(backing.residual_eggs_per_wrapper[outcome])
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        match action {
            StructuredClaimActionV1::WrapFull => {
                user_internal[outcome]
                    .checked_sub(full)
                    .and_then(|_| vault_internal[outcome].checked_add(residual))
                    .and_then(|_| claim.aggregate_internal_supply[outcome].checked_sub(complete_cash))
                    .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
            }
            StructuredClaimActionV1::UnwrapFull => {
                vault_internal[outcome]
                    .checked_sub(residual)
                    .and_then(|_| user_internal[outcome].checked_add(full))
                    .and_then(|_| claim.aggregate_internal_supply[outcome].checked_add(complete_cash))
                    .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
            }
            StructuredClaimActionV1::RedeemTerminal => {
                vault_internal[outcome]
                    .checked_sub(residual)
                    .and_then(|_| claim.aggregate_internal_supply[outcome].checked_sub(residual))
                    .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
            }
            _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        }
        outcome += 1;
    }
    match action {
        StructuredClaimActionV1::WrapFull => {
            vault
                .cash_atoms()
                .checked_add(complete_cash)
                .and_then(|_| hoard.cash_liability_atoms.checked_add(complete_cash))
                .and_then(|_| hoard.locked_claim_principal_atoms.checked_sub(complete_cash))
                .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        }
        StructuredClaimActionV1::UnwrapFull => {
            vault
                .cash_atoms()
                .checked_sub(complete_cash)
                .and_then(|_| hoard.cash_liability_atoms.checked_sub(complete_cash))
                .and_then(|_| hoard.locked_claim_principal_atoms.checked_add(complete_cash))
                .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        }
        StructuredClaimActionV1::RedeemTerminal => {
            let residual_payout = terminal_residual_payout
                .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
            vault
                .cash_atoms()
                .checked_sub(complete_cash)
                .and_then(|_| complete_cash.checked_add(residual_payout))
                .and_then(|payout| user.cash_atoms().checked_add(payout))
                .and_then(|_| hoard.cash_liability_atoms.checked_add(residual_payout))
                .and_then(|_| hoard.locked_claim_principal_atoms.checked_sub(residual_payout))
                .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        }
        _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
    }
    Ok(())
}

fn validate_compaction_trigger_v1(
    accounts: &[StructuredChainAccountV1<'_>],
    collateral_program: Address,
    wrapper_supply: u64,
    vault: PositionAccountV3,
    backing: clutch_structured_claim::BackingPlan,
    hoard: HoardV2,
    claim: ClaimLedgerV3,
) -> Result<()> {
    if accounts[25].owner()? != collateral_program
        || accounts[25].executable()
        || accounts[25].data()?.is_empty()
        || !system_identity_is_valid(accounts[24])
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let required_cash = wrapper_supply
        .checked_mul(backing.cash_per_wrapper)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let donated_cash = vault
        .cash_atoms()
        .checked_sub(required_cash)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if hoard.cash_liability_atoms < donated_cash {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let internal = vault.native_eggs();
    let mut any = donated_cash != 0;
    let mut outcome = 0_usize;
    while outcome < usize::from(backing.outcome_count) {
        let required = wrapper_supply
            .checked_mul(backing.residual_eggs_per_wrapper[outcome])
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let donated = internal[outcome]
            .checked_sub(required)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        if claim.aggregate_internal_supply[outcome] < donated {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        any |= donated != 0;
        outcome += 1;
    }
    if !any {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok(())
}

fn validate_retirement_trigger_v1(
    accounts: &[StructuredChainAccountV1<'_>],
    wrapper_supply: u64,
    vault: PositionAccountV3,
    replay: clutch_retirement::ReplayV3EnvelopeHeader,
    root: StructuredMarketRootV1,
) -> Result<()> {
    if wrapper_supply != 0
        || vault.cash_atoms() != 0
        || vault.reserved_cash_atoms() != 0
        || vault.native_eggs() != [0; clutch_retirement::MAX_OUTCOMES]
        || vault.outstanding_reservations() != 0
        || accounts[27].address.to_bytes() != root.binding.rent_refund_owner.bytes()
        || accounts[28].address.to_bytes() != root.binding.neutral_lamport_sink.bytes()
        || accounts[27].address == accounts[28].address
        || vault.rent().payer.bytes() != accounts[27].address.to_bytes()
        || replay.rent().payer().bytes() != accounts[27].address.to_bytes()
        || !system_identity_is_valid(accounts[27])
        || !system_identity_is_valid(accounts[28])
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let position_principal = vault
        .rent()
        .refundable_live_principal
        .checked_add(vault.rent().permanent_tombstone_principal)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let position_donation = observed_lamports(accounts[8])
        .checked_sub(position_principal)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let replay_donation = observed_lamports(accounts[9])
        .checked_sub(replay.rent().refundable_principal())
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let mut refund_credit = vault
        .rent()
        .refundable_live_principal
        .checked_add(replay.rent().refundable_principal())
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let mut sink_credit = position_donation
        .checked_add(replay_donation)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if root.live_descriptor_count == 1 {
        refund_credit = refund_credit
            .checked_add(root.rent_principal_lamports)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        sink_credit = sink_credit
            .checked_add(root.current_donation_lamports)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    }
    observed_lamports(accounts[27])
        .checked_add(refund_credit)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    observed_lamports(accounts[28])
        .checked_add(sink_credit)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    Ok(())
}

fn observed_lamports(account: StructuredChainAccountV1<'_>) -> u64 {
    account.present.map_or(0, |value| value.lamports)
}

fn validate_current_product_join(
    accounts: &[StructuredChainAccountV1<'_>],
    action: StructuredClaimActionV1,
    root: StructuredMarketRootV1,
) -> Result<()> {
    let (link_index, bundle_index, attachment_index) = match action {
        StructuredClaimActionV1::CompactDonation => (27, None, None),
        StructuredClaimActionV1::RetireDescriptor => (24, Some(25), Some(26)),
        _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
    };
    let link = SeriesMarketLinkAccountV2::decode(accounts[link_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let binding = link.state.binding();
    let link_pda = Address::find_program_address(
        &[
            b"dc:series-market-link:v1",
            &binding.series_plan_id.bytes(),
            &binding.ordinal.to_le_bytes(),
        ],
        &accounts[13].address,
    );
    if accounts[link_index].owner()? != accounts[13].address
        || accounts[link_index].executable()
        || accounts[link_index].address != link_pda.0
        || link.stored_bump != link_pda.1
        || link.state.phase() != SeriesMarketLinkPhaseV2::Active
        || link.state.obligation_status(SeriesLinkObligationV2::Wrapper)
            != SeriesLinkObligationStatusV2::Live
        || binding.series_plan_id != root.binding.series_plan_id
        || binding.ordinal != root.binding.ordinal
        || binding.market_instance_id != root.binding.market_instance_id
        || binding.generation != root.binding.generation
        || binding.attachment_plan_id != root.binding.attachment_plan_id
        || binding.compiler_bundle_id != root.binding.compiler_output_id
        || binding.capability_profile_id != root.binding.capability_profile_id
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    if let (Some(bundle_index), Some(attachment_index)) = (bundle_index, attachment_index) {
        let bundle = CompiledProductSeriesBundleV6::decode(accounts[bundle_index].data()?)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let attachment = SeriesAttachmentPlanV5::decode(accounts[attachment_index].data()?)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let bundle_id = bundle
            .id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let attachment_id = attachment
            .id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        verify_product_artifact(
            accounts[13].address,
            accounts[bundle_index],
            ArtifactKind::CompiledProductSeriesBundleV6,
            bundle_id.bytes(),
        )?;
        verify_product_artifact(
            accounts[13].address,
            accounts[attachment_index],
            ArtifactKind::SeriesAttachmentPlanV5,
            attachment_id.bytes(),
        )?;
        if bundle_id != binding.compiler_bundle_id
            || attachment_id != binding.attachment_plan_id
            || bundle.series_plan_id != binding.series_plan_id
            || bundle.funding_terms_id != binding.funding_terms_id
            || bundle.funding_quote_id != binding.funding_quote_id
            || bundle.attachment_plan_id != binding.attachment_plan_id
            || bundle.capability_profile_id.content_id() != binding.capability_profile_id
            || attachment.funding_quote_id != binding.funding_quote_id
            || attachment.wrapper_recipe_set_id != root.binding.wrapper_recipe_set_id
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    } else {
        let funding = SeriesFundingTermsV2::decode(accounts[28].data()?)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let funding_id = funding
            .id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        verify_product_artifact(
            accounts[13].address,
            accounts[28],
            ArtifactKind::SeriesFundingTermsV2,
            funding_id.bytes(),
        )?;
        if funding_id != binding.funding_terms_id
            || funding.series_plan_id != binding.series_plan_id
            || funding.neutral_collateral_disposition_token_account.bytes()
                != accounts[25].address.to_bytes()
            || funding.token_program.bytes() != accounts[15].address.to_bytes()
            || funding.collateral_mint.bytes() != accounts[22].address.to_bytes()
            || accounts[25].owner()? != accounts[15].address
            || accounts[25].executable()
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    Ok(())
}

/// Construct one Source material artifact through the sole typed Source graph.
/// The caller supplies decoded semantic-owner values and physical identities;
/// it cannot supply instruction bytes, account metas, signer vectors, or the
/// final transaction.
#[allow(clippy::too_many_arguments)]
pub fn construct_source_action_material_v1(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    selection: &KeeperActionSelection,
    observation: SourceCrankObservation<'_>,
    freshness: ActionFreshnessBoundaryV1,
    material: SourceWorkflowActionMaterial,
) -> Result<CanonicalActionMaterialV1> {
    release
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    manifest
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    freshness.validate()?;
    if release.program_id != manifest.clutch.program_id
        || release.program_data != manifest.clutch.program_data
        || release.deployment_slot != manifest.clutch.deployment_slot
        || release.elf_sha256 != manifest.clutch.elf_sha256
        || release.release_manifest_sha256 != manifest.manifest_sha256
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    let coordinate = CanonicalIntentCoordinate {
        family_tag: SOURCE_SERIES_FAMILY_TAG,
        family_version: SOURCE_SERIES_FAMILY_VERSION,
        local_action: material.accounts.action().tag(),
    };
    if release.enabled_intents.binary_search(&coordinate).is_err() {
        return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
    }
    if selection.release_key != release.key()
        || selection.effective_commitment != crate::rpc_index::RpcCommitment::Finalized
        || selection.action != source_selection_action(material.accounts.action())
        || material.action_name != selection.action
        || selection.cursor != observation_cursor(observation, selection.cursor)?
        || freshness.observed_slot < selection.account_slot
        || material.valid_before_slot != freshness.valid_before_slot
    {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    if builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
        || builder.payer() != material.accounts.payer_address()
    {
        return Err(CanonicalActionMaterialErrorV1::FeePayerMismatch);
    }
    let source_account_roles = material
        .accounts
        .ordered_projection()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let account_roles = source_account_roles
        .iter()
        .map(|role| CanonicalAccountRoleV1 {
            label: source_role_label_v2(role.role),
            address: role.address,
            writable: role.writable,
            signer: role.signer,
        })
        .collect::<Vec<_>>();
    if !account_roles
        .iter()
        .any(|role| role.address == selection.account)
    {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    let planned = plan_source_crank(
        manifest,
        builder,
        observation,
        selection.cursor,
        material,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let planned_coordinate_matches = matches!(
        planned.coordinate,
        CanonicalActionCoordinate::SourceTransition { registry, .. }
            if registry.tag() == coordinate.local_action
    );
    if planned.manifest_sha256 != release.release_manifest_sha256
        || planned.cursor != selection.cursor
        || !planned_coordinate_matches
        || !planned.reload_authoritative_accounts
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    validate_unsigned_source_plan(coordinate, builder.payer(), &account_roles, &planned)?;
    let release_key = release.key();
    let driver_release_key = release_key.clone();
    let authority_state_sha256 = selection.cursor.observed_state_sha256;
    let draft_id = action_material_id(
        &release_key,
        &driver_release_key,
        release.release_manifest_sha256,
        release.capability_profile_id,
        coordinate,
        selection.account,
        selection.account_slot,
        selection.cursor,
        authority_state_sha256,
        freshness,
        builder.payer(),
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key,
        driver_release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        driver_account: selection.account,
        driver_account_slot: selection.account_slot,
        cursor: selection.cursor,
        authority_state_sha256,
        freshness,
        fee_payer: builder.payer(),
        account_roles,
        planned,
        draft_id,
    })
}

pub(crate) const fn source_selection_action(
    action: clutch_solana_layout::registry::SourceSeriesAction,
) -> &'static str {
    use clutch_solana_layout::registry::SourceSeriesAction as Action;
    match action {
        Action::RegisterRelease => "register-source-release",
        Action::InitializeHead => "initialize-source-head",
        Action::OpenRawPage => "open-raw-page",
        Action::IngestBoundaryBatch => "ingest-boundary",
        Action::SealRawPage => "seal-raw-page",
        Action::InitializeWindowWork => "initialize-window-work",
        Action::FoldWindowPages => "fold-window-pages",
        Action::SealWindow => "seal-window",
        Action::EvaluateStatistic => "evaluate-statistic",
        Action::EmitFailureHandoff => "emit-failure-handoff",
        Action::ReopenGeneration => "reopen-source-generation",
        Action::CloseGeneration => "close-source-generation",
    }
}

pub(crate) fn source_action_from_selection(
    selection: &str,
) -> Option<clutch_solana_layout::registry::SourceSeriesAction> {
    use clutch_solana_layout::registry::SourceSeriesAction as Action;
    match selection {
        "register-source-release" => Some(Action::RegisterRelease),
        "initialize-source-head" => Some(Action::InitializeHead),
        "open-raw-page" => Some(Action::OpenRawPage),
        "ingest-boundary" => Some(Action::IngestBoundaryBatch),
        "seal-raw-page" => Some(Action::SealRawPage),
        "initialize-window-work" => Some(Action::InitializeWindowWork),
        "fold-window-pages" => Some(Action::FoldWindowPages),
        "seal-window" => Some(Action::SealWindow),
        "evaluate-statistic" => Some(Action::EvaluateStatistic),
        "emit-failure-handoff" => Some(Action::EmitFailureHandoff),
        "reopen-source-generation" => Some(Action::ReopenGeneration),
        "close-source-generation" => Some(Action::CloseGeneration),
        _ => None,
    }
}

pub(crate) const fn structured_selection_action(action: StructuredClaimActionV1) -> &'static str {
    match action {
        StructuredClaimActionV1::CreateDescriptor => "create-structured-descriptor",
        StructuredClaimActionV1::WrapFull => "wrap-structured-full",
        StructuredClaimActionV1::UnwrapFull => "unwrap-structured-full",
        StructuredClaimActionV1::CompactDonation => "compact-structured-donation",
        StructuredClaimActionV1::RedeemTerminal => "redeem-structured-terminal",
        StructuredClaimActionV1::RetireDescriptor => "retire-structured-descriptor",
    }
}

pub(crate) const fn structured_action_from_selection(
    selection: &str,
) -> Option<StructuredClaimActionV1> {
    match selection {
        "create-structured-descriptor" => Some(StructuredClaimActionV1::CreateDescriptor),
        "wrap-structured-full" => Some(StructuredClaimActionV1::WrapFull),
        "unwrap-structured-full" => Some(StructuredClaimActionV1::UnwrapFull),
        "compact-structured-donation" => Some(StructuredClaimActionV1::CompactDonation),
        "redeem-structured-terminal" => Some(StructuredClaimActionV1::RedeemTerminal),
        "retire-structured-descriptor" => Some(StructuredClaimActionV1::RetireDescriptor),
        _ => None,
    }
}

const fn structured_coordinate(action: StructuredClaimActionV1) -> CanonicalIntentCoordinate {
    CanonicalIntentCoordinate {
        family_tag: STRUCTURED_CLAIM_FAMILY_TAG,
        family_version: STRUCTURED_CLAIM_FAMILY_VERSION,
        local_action: action.tag(),
    }
}

fn observation_cursor(
    observation: SourceCrankObservation<'_>,
    cursor: ResumableWorkflowCursor,
) -> Result<ResumableWorkflowCursor> {
    if cursor.generation != observation.generation
        || cursor.observed_state_sha256 != observation.observed_state_sha256
    {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    Ok(cursor)
}

fn validate_unsigned_source_plan(
    coordinate: CanonicalIntentCoordinate,
    fee_payer: Address,
    roles: &[CanonicalAccountRoleV1],
    planned: &PlannedWorkflowNode,
) -> Result<()> {
    let transaction = &planned.unsigned_transaction;
    let expected_signers = roles
        .iter()
        .filter(|role| role.signer)
        .map(|role| role.address)
        .chain(core::iter::once(fee_payer))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let binding_matches = matches!(
        transaction.registry_bindings.as_slice(),
        [Some(binding)]
            if binding.family.tag() == coordinate.family_tag
                && binding.family.version() == coordinate.family_version
                && binding.local_action == coordinate.local_action
                && Some(binding.family_status)
                    == clutch_solana_layout::registry::ExtensionFamily::StructuredClaim
                        .allocation_status()
                && matches!(
                    binding.central_action,
                    Some(ExtensionAction::SourceV3(action))
                        if action.tag() == coordinate.local_action
                )
    );
    if transaction.flows != [ProtocolFlow::SourcePlaneV3]
        || transaction.actions.len() != 1
        || transaction.semantic_owners.len() != 1
        || !binding_matches
        || transaction.runtime_admissions != [RuntimeAdmission::ReleaseBoundEnabled]
        || transaction.required_signers != expected_signers
        || transaction.exact_equations.is_empty()
        || transaction.serialized_transaction.is_empty()
        || transaction.has_recent_blockhash
        || transaction.signed
        || transaction.submitted
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(())
}

fn validate_unsigned_structured_plan(
    coordinate: CanonicalIntentCoordinate,
    fee_payer: Address,
    roles: &[CanonicalAccountRoleV1],
    planned: &PlannedWorkflowNode,
) -> Result<()> {
    let transaction = &planned.unsigned_transaction;
    let expected_signers = roles
        .iter()
        .filter(|role| role.signer)
        .map(|role| role.address)
        .chain(core::iter::once(fee_payer))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let binding_matches = matches!(
        transaction.registry_bindings.as_slice(),
        [Some(binding)]
            if binding.family.tag() == coordinate.family_tag
                && binding.family.version() == coordinate.family_version
                && binding.local_action == coordinate.local_action
                && Some(binding.family_status)
                    == ExtensionFamily::StructuredClaim.allocation_status()
                && binding.central_action.is_none()
    );
    if transaction.flows != [ProtocolFlow::StructuredClaim]
        || transaction.actions.len() != 1
        || transaction.semantic_owners.len() != 1
        || !binding_matches
        || transaction.runtime_admissions != [RuntimeAdmission::ReleaseBoundEnabled]
        || transaction.required_signers != expected_signers
        || transaction.message_version != TransactionMessageVersionV1::V0
        || transaction.address_lookup_tables.len() != 1
        || transaction.has_recent_blockhash
        || transaction.signed
        || transaction.submitted
        || !planned.reload_authoritative_accounts
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn action_material_id(
    release_key: &str,
    driver_release_key: &str,
    release_manifest_sha256: [u8; 32],
    capability_profile_id: [u8; 32],
    coordinate: CanonicalIntentCoordinate,
    driver_account: Address,
    driver_account_slot: u64,
    cursor: ResumableWorkflowCursor,
    authority_state_sha256: [u8; 32],
    freshness: ActionFreshnessBoundaryV1,
    fee_payer: Address,
    roles: &[CanonicalAccountRoleV1],
    transaction: &UnsignedProtocolTransaction,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(CANONICAL_ACTION_MATERIAL_SCHEMA_V1.as_bytes());
    hash_text(&mut hash, release_key);
    hash_text(&mut hash, driver_release_key);
    hash.update(release_manifest_sha256);
    hash.update(capability_profile_id);
    hash.update([
        coordinate.family_tag,
        coordinate.family_version,
        coordinate.local_action,
    ]);
    hash.update(driver_account.to_bytes());
    hash.update(driver_account_slot.to_le_bytes());
    hash.update(cursor.workflow_id);
    hash.update([workflow_lane_byte(cursor.lane)]);
    hash.update(cursor.generation.to_le_bytes());
    hash.update(cursor.position.phase.to_le_bytes());
    hash.update(cursor.position.item.to_le_bytes());
    hash.update(cursor.observed_state_sha256);
    hash.update(authority_state_sha256);
    hash.update(freshness.observed_slot.to_le_bytes());
    hash.update(freshness.valid_before_slot.to_le_bytes());
    hash.update(freshness.maximum_validity_slots.to_le_bytes());
    hash.update(fee_payer.to_bytes());
    hash.update(
        u64::try_from(roles.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (index, role) in roles.iter().enumerate() {
        // The release-enabled action plus canonical contract index owns the
        // role identity; no unstable Rust enum discriminant enters the hash.
        hash.update(
            u64::try_from(index)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        hash_text(&mut hash, role.label);
        hash.update(role.address.to_bytes());
        hash.update([u8::from(role.writable), u8::from(role.signer)]);
    }
    hash.update(
        u64::try_from(transaction.actions.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for action in &transaction.actions {
        hash_text(&mut hash, action);
    }
    for owner in &transaction.semantic_owners {
        hash_text(&mut hash, &owner.package);
        hash_text(&mut hash, &owner.schema);
        hash.update(owner.release_sha256);
    }
    for equation in &transaction.exact_equations {
        hash_text(&mut hash, &equation.name);
        hash_integer_unit(&mut hash, equation.unit);
        hash.update(equation.left.to_le_bytes());
        hash.update(equation.right.to_le_bytes());
    }
    hash.update(
        u64::try_from(transaction.serialized_transaction.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hash.update(&transaction.serialized_transaction);
    hash.finalize().into()
}

fn hash_text(hash: &mut Sha256, value: &str) {
    hash.update(
        u64::try_from(value.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hash.update(value.as_bytes());
}

fn hash_integer_unit(hash: &mut Sha256, unit: IntegerUnit) {
    match unit {
        IntegerUnit::Lamports => hash.update([0]),
        IntegerUnit::CollateralAtoms { mint } => {
            hash.update([1]);
            hash.update(mint.to_bytes());
        }
        IntegerUnit::PriceUnits { scale } => {
            hash.update([2]);
            hash.update(scale.to_le_bytes());
        }
        IntegerUnit::EggAtoms { market, outcome } => {
            hash.update([3]);
            hash.update(market);
            hash.update([outcome]);
        }
        IntegerUnit::FeeAtoms { mint } => {
            hash.update([4]);
            hash.update(mint.to_bytes());
        }
        IntegerUnit::WrapperAtoms { mint } => {
            hash.update([5]);
            hash.update(mint.to_bytes());
        }
    }
}

pub(crate) const fn source_role_label_v2(
    role: clutch_solana_layout::source_series::SourceAccountRoleV2,
) -> &'static str {
    use clutch_solana_layout::source_series::SourceAccountRoleV2 as Role;
    match role {
        Role::SourceReleaseArtifact => "source-release-artifact",
        Role::SourceRelease => "source-release",
        Role::AdapterProgram => "adapter-program",
        Role::AdapterProgramData => "adapter-program-data",
        Role::ParserProgram => "parser-program",
        Role::ParserProgramData => "parser-program-data",
        Role::ParserConfig => "parser-config",
        Role::SourceSpec => "source-spec",
        Role::SourceWorkSchedule => "source-work-schedule",
        Role::GenerationRequest => "generation-request",
        Role::ClockSysvar => "clock-sysvar",
        Role::Feed => "feed",
        Role::ReceiverProgram => "receiver-program",
        Role::ReceiverProgramData => "receiver-program-data",
        Role::ReceiverConfig => "receiver-config",
        Role::SourceHead => "source-head",
        Role::HeadLineage => "head-lineage",
        Role::OpenRawPage => "open-raw-page",
        Role::OpenPageLineage => "open-page-lineage",
        Role::RawPage => "raw-page",
        Role::SourceOccurrence => "source-occurrence",
        Role::WindowSpec => "window-spec",
        Role::WindowWork => "window-work",
        Role::WorkLineage => "work-lineage",
        Role::WindowSeal => "window-seal",
        Role::StatisticKey => "statistic-key",
        Role::SummaryProgram => "summary-program",
        Role::EvaluatorProgram => "evaluator-program",
        Role::EvaluatorProgramData => "evaluator-program-data",
        Role::StatisticResult => "statistic-result",
        Role::ResultLineage => "result-lineage",
        Role::SourceWorkReceipt => "source-work-receipt",
        Role::LivenessPolicy => "liveness-policy",
        Role::SourceCompartment => "source-compartment",
        Role::Keeper => "keeper",
        Role::Payer => "payer",
        Role::PrincipalRefund => "principal-refund",
        Role::NeutralSink => "neutral-sink",
        Role::FailurePolicy => "failure-policy",
        Role::HandoffReceipt => "handoff-receipt",
        Role::GenerationAuthority => "generation-authority",
        Role::GenerationTarget => "generation-target",
        Role::GenerationLineage => "generation-lineage",
        Role::SystemProgram => "system-program",
        Role::RentSysvar => "rent-sysvar",
    }
}

const fn workflow_lane_byte(lane: crate::workflow_graph::WorkflowLane) -> u8 {
    match lane {
        crate::workflow_graph::WorkflowLane::Creation => 0,
        crate::workflow_graph::WorkflowLane::SourceCrank => 1,
        crate::workflow_graph::WorkflowLane::Candidate => 2,
        crate::workflow_graph::WorkflowLane::KeeperReceipts => 3,
        crate::workflow_graph::WorkflowLane::RecoveryRetirement => 4,
        crate::workflow_graph::WorkflowLane::StructuredLifecycle => 5,
    }
}

impl From<WorkflowGraphError> for CanonicalActionMaterialErrorV1 {
    fn from(_: WorkflowGraphError) -> Self {
        Self::InvalidPlan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc_index::{RpcObservationProvenance, RpcObservationSource};
    use crate::transaction_builder::{ExactEquation, SemanticOwner, CONSTRUCTION_PLAN_SCHEMA};
    use crate::workflow_graph::{WorkflowLane, WorkflowPosition};
    use clutch_solana_layout::source_series::SourceAccountRoleV2;

    fn address(byte: u8) -> Address {
        Address::new_from_array([byte; 32])
    }

    fn cursor() -> ResumableWorkflowCursor {
        ResumableWorkflowCursor {
            workflow_id: [9; 32],
            lane: WorkflowLane::SourceCrank,
            generation: 3,
            position: WorkflowPosition { phase: 2, item: 4 },
            observed_state_sha256: [8; 32],
        }
    }

    fn transaction() -> UnsignedProtocolTransaction {
        UnsignedProtocolTransaction {
            schema: CONSTRUCTION_PLAN_SCHEMA,
            flows: vec![ProtocolFlow::SourcePlaneV3],
            actions: vec!["open-raw-page".into()],
            semantic_owners: vec![SemanticOwner {
                package: "clutch-source-plane-v3-adapter".into(),
                schema: "intent-preimage-v3".into(),
                release_sha256: [7; 32],
            }],
            registry_bindings: vec![None],
            runtime_admissions: vec![RuntimeAdmission::ReleaseBoundEnabled],
            required_signers: vec![address(6)],
            exact_equations: vec![ExactEquation {
                name: "exact ceiling".into(),
                unit: IntegerUnit::Lamports,
                left: 11,
                right: 11,
            }],
            message_version: TransactionMessageVersionV1::Legacy,
            address_lookup_tables: Vec::new(),
            serialized_transaction: vec![1, 2, 3],
            has_recent_blockhash: false,
            signed: false,
            submitted: false,
        }
    }

    fn lookup_table_account(
        slot: u64,
        last_extended_slot: u64,
        deactivation_slot: u64,
        addresses: &[[u8; 32]],
    ) -> ObservedRpcAccount {
        let mut data = vec![0_u8; ADDRESS_LOOKUP_TABLE_META_BYTES];
        data[0..4].copy_from_slice(&[1, 0, 0, 0]);
        data[4..12].copy_from_slice(&deactivation_slot.to_le_bytes());
        data[12..20].copy_from_slice(&last_extended_slot.to_le_bytes());
        data[20] = 0;
        data[21] = 0;
        for address in addresses {
            data.extend_from_slice(address);
        }
        ObservedRpcAccount {
            address: address(0xa0),
            owner: solana_sdk_ids::address_lookup_table::ID,
            lamports: 1,
            executable: false,
            rent_epoch: 0,
            data,
            provenance: RpcObservationProvenance {
                cluster_key: "cluster:genesis".into(),
                release_key: "address-lookup-table-program".into(),
                slot,
                commitment: RpcCommitment::Finalized,
                source: RpcObservationSource::FinalizedScan,
                receive_sequence: 1,
            },
        }
    }

    #[test]
    fn structured_lookup_table_requires_a_finalized_stable_unique_tail() {
        let first = [0x31; 32];
        let second = [0x32; 32];
        let valid = lookup_table_account(10, 9, u64::MAX, &[first, second]);
        let authenticated = StructuredAddressLookupTableV1::authenticate(&valid).unwrap();
        assert_eq!(authenticated.account(), address(0xa0));
        assert_eq!(authenticated.observed_slot(), 10);
        assert_ne!(authenticated.state_sha256(), [0; 32]);

        let same_slot_extension = lookup_table_account(10, 10, u64::MAX, &[first, second]);
        assert_eq!(
            StructuredAddressLookupTableV1::authenticate(&same_slot_extension),
            Err(CanonicalActionMaterialErrorV1::InvalidChainState)
        );
        let deactivating = lookup_table_account(10, 9, 11, &[first, second]);
        assert_eq!(
            StructuredAddressLookupTableV1::authenticate(&deactivating),
            Err(CanonicalActionMaterialErrorV1::InvalidChainState)
        );
        let duplicate = lookup_table_account(10, 9, u64::MAX, &[first, first]);
        assert_eq!(
            StructuredAddressLookupTableV1::authenticate(&duplicate),
            Err(CanonicalActionMaterialErrorV1::InvalidChainState)
        );
    }

    #[test]
    fn structured_full_vector_direction_is_exhaustive_and_disjoint() {
        let purposes = [
            PositionPurposeV3::General,
            PositionPurposeV3::DealerFacility,
            PositionPurposeV3::Series,
            PositionPurposeV3::StructuredClaim,
        ];
        let mut admitted = Vec::new();
        for terminal in [false, true] {
            for source in purposes {
                for destination in purposes {
                    if let Ok(action) = classify_full_vector_direction_v1(
                        terminal,
                        source,
                        destination,
                    ) {
                        admitted.push((terminal, source, destination, action));
                    }
                }
            }
        }
        assert_eq!(
            admitted,
            vec![
                (
                    false,
                    PositionPurposeV3::General,
                    PositionPurposeV3::StructuredClaim,
                    StructuredClaimActionV1::WrapFull,
                ),
                (
                    false,
                    PositionPurposeV3::StructuredClaim,
                    PositionPurposeV3::General,
                    StructuredClaimActionV1::UnwrapFull,
                ),
                (
                    true,
                    PositionPurposeV3::StructuredClaim,
                    PositionPurposeV3::General,
                    StructuredClaimActionV1::RedeemTerminal,
                ),
            ]
        );
    }

    #[test]
    fn structured_create_leaf_detector_refuses_a_second_match() {
        let mut matched = None;
        record_unique_create_leaf_v1(&mut matched, 3).unwrap();
        assert_eq!(matched, Some(3));
        assert_eq!(
            record_unique_create_leaf_v1(&mut matched, 9),
            Err(CanonicalActionMaterialErrorV1::InvalidChainState)
        );
    }

    #[test]
    fn structured_alias_boundary_admits_only_one_identical_token_release() {
        assert!(structured_release_alias_is_permitted_v1(
            4, 18, 4, 5, 18, 19, true,
        ));
        assert!(structured_release_alias_is_permitted_v1(
            5, 19, 4, 5, 18, 19, true,
        ));
        assert!(!structured_release_alias_is_permitted_v1(
            5, 19, 4, 5, 18, 19, false,
        ));
        assert!(!structured_release_alias_is_permitted_v1(
            4, 19, 4, 5, 18, 19, true,
        ));
    }

    #[test]
    fn validity_boundary_refuses_zero_or_unbounded_lifetime() {
        assert_eq!(
            ActionFreshnessBoundaryV1 {
                observed_slot: 10,
                valid_before_slot: 10,
                maximum_validity_slots: 4,
            }
            .validate(),
            Err(CanonicalActionMaterialErrorV1::InvalidFreshness)
        );
        assert_eq!(
            ActionFreshnessBoundaryV1 {
                observed_slot: 10,
                valid_before_slot: 15,
                maximum_validity_slots: 4,
            }
            .validate(),
            Err(CanonicalActionMaterialErrorV1::InvalidFreshness)
        );
    }

    #[test]
    fn material_identity_commits_freshness_and_exact_role_address() {
        let coordinate = CanonicalIntentCoordinate {
            family_tag: SOURCE_SERIES_FAMILY_TAG,
            family_version: SOURCE_SERIES_FAMILY_VERSION,
            local_action: 3,
        };
        let roles = [CanonicalAccountRoleV1 {
            label: source_role_label_v2(SourceAccountRoleV2::Payer),
            address: address(6),
            writable: true,
            signer: true,
        }];
        let first = action_material_id(
            "release",
            "driver-release",
            [1; 32],
            [2; 32],
            coordinate,
            address(3),
            10,
            cursor(),
            [17; 32],
            ActionFreshnessBoundaryV1 {
                observed_slot: 10,
                valid_before_slot: 12,
                maximum_validity_slots: 4,
            },
            address(6),
            &roles,
            &transaction(),
        );
        let mut rebound = roles;
        rebound[0].address = address(5);
        let second = action_material_id(
            "release",
            "driver-release",
            [1; 32],
            [2; 32],
            coordinate,
            address(3),
            10,
            cursor(),
            [17; 32],
            ActionFreshnessBoundaryV1 {
                observed_slot: 10,
                valid_before_slot: 13,
                maximum_validity_slots: 4,
            },
            address(6),
            &rebound,
            &transaction(),
        );
        assert_ne!(first, second);
    }
}
