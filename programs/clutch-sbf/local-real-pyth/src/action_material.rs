//! Opaque, release-authenticated action material for operator projections.
//!
//! Construction accepts typed semantic-owner state and account projections,
//! never browser JSON or caller-authored instruction bytes. The resulting
//! artifact remains unsigned and blockhash-free. It can make an operator
//! control inspectable, but it cannot sign, submit, or predict poststate.

use crate::rpc_index::{
    CanonicalIntentCoordinate, FinalizedAccountAbsence, IndexedProgramRelease,
    ObservedRpcAccount, RpcCommitment,
};
use crate::operatord::KeeperActionSelection;
use crate::transaction_builder::{
    ExactEquation, IntegerUnit, ProtocolFlow, ProtocolTransactionBuilder, RuntimeAdmission,
    TransactionMessageVersionV1, UnsignedProtocolTransaction,
};
use crate::workflow_graph::{
    plan_source_crank, CanonicalActionCoordinate, ExplicitOperatorReleaseManifest,
    PlannedWorkflowNode, ResumableWorkflowCursor,
    SourceCrankObservation, SourceWorkflowActionMaterial, WorkflowGraphError,
};
use clutch_solana_layout::registry::{
    AllocationStatus, ExtensionAction, ExtensionFamily, STRUCTURED_CLAIM_FAMILY_TAG,
    STRUCTURED_CLAIM_FAMILY_VERSION, SOURCE_SERIES_FAMILY_TAG, SOURCE_SERIES_FAMILY_VERSION,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::product_series::{SeriesMarketLinkAccountV2, SeriesRegistryAccountV3};
use clutch_product_series::{
    CompiledProductSeriesBundleV6, ContentId, FixedCodec, MarketInstancePreimageV2,
    NativeClaimBasisV1, RegistryCapabilityProfileV4, RegistryProgramReleaseV2,
    RegistryReleaseLocusV2,
    SeriesAttachmentPlanV5, SeriesFundingTermsV2, SeriesLinkObligationStatusV2, SeriesLinkObligationV2,
    SeriesMarketLinkPhaseV2,
};
use clutch_retirement::{
    PositionAccountV3, PositionPurposeV3, ReplayV3Envelope, ReplayV3HashBackend,
    POSITION_V3_PDA_PREFIX, PURPOSE_REPLAY_V3_PDA_PREFIX,
};
use clutch_structured_claim::DeploymentBinding;
use clutch_structured_claim_adapter::{
    canonical_native_claim_id_v1, canonical_series_scoped_wrapper_product_id_v2,
    current_structured_account_meta_v1, current_structured_action_contract_v1,
    decode_canonical_wrapper_token_v1, STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1,
    STRUCTURED_CURRENT_ACCOUNT_CONTRACT_LABEL_V1,
    STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1,
    STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1,
    DESCRIPTOR_SEED, MINT_AUTHORITY_SEED, MINT_SEED, VAULT_OWNER_SEED,
};
use clutch_structured_claim_runtime_contract::{
    CreateDescriptorPayloadV1, DescriptorBasisV1, DescriptorStateV1,
    reconstruct_descriptor_identity_v1, structured_owner_release_id_v2,
    StructuredClaimActionV1, StructuredClaimDescriptorV2, StructuredMarketRootBindingV1,
    StructuredMarketRootV1, VaultMutationPayloadV1, WrapperQuantityPayloadV1,
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

/// Opaque exact wrapper/base/Token release join for Structured construction.
#[derive(Clone, Copy, Debug)]
pub struct StructuredOperatorReleaseSetV1<'release> {
    wrapper: &'release IndexedProgramRelease,
    base: &'release IndexedProgramRelease,
    token_2022: &'release IndexedProgramRelease,
}

impl<'release> StructuredOperatorReleaseSetV1<'release> {
    /// Authenticate the three disjoint checked manifests and the complete
    /// current Structured intent set. Empty or partial release admission is
    /// refused rather than advertised as callable.
    pub fn authenticate(
        wrapper: &'release IndexedProgramRelease,
        base: &'release IndexedProgramRelease,
        token_2022: &'release IndexedProgramRelease,
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
        if address == Address::default() || absence.slot == 0 {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        Ok(Self {
            address,
            present: None,
            observed_slot: absence.slot,
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

/// Construct one callable Structured wrapper draft solely from the checked
/// three-release join, a finalized scheduler cursor, one finalized on-chain
/// lookup table, and exact chain account bodies/absences in the semantic
/// owner's current ABI. Recipe bodies, witnesses, quantities, generations,
/// sequences, Product bindings, transport keys, and the dynamic Link privilege
/// are all derived here; none is accepted as a DTO.
#[allow(clippy::too_many_arguments)]
pub fn construct_structured_action_material_v1(
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
        structured_authority_state_id(accounts, lookup_table.state_sha256);
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
    validate_structured_release_accounts(releases, accounts, action)?;
    if action == StructuredClaimActionV1::CreateDescriptor {
        derive_structured_create_v1(releases, selection, accounts)
    } else {
        derive_structured_current_mutation_v1(releases, selection, accounts, action)
    }
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

    let (payload, quantity, product_link_writable) = match action {
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => {
            let source = PositionAccountV3::decode(accounts[8].data()?)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let destination = PositionAccountV3::decode(accounts[10].data()?)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let source_replay = ReplayV3Envelope::decode(accounts[9].data()?, &OperatorSha256V1)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let destination_replay =
                ReplayV3Envelope::decode(accounts[11].data()?, &OperatorSha256V1)
                    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let (user, user_replay, vault, vault_replay) = if action
                == StructuredClaimActionV1::WrapFull
            {
                (source, source_replay, destination, destination_replay)
            } else {
                (destination, destination_replay, source, source_replay)
            };
            if user.purpose() != PositionPurposeV3::General
                || vault.purpose() != PositionPurposeV3::StructuredClaim
                || vault.purpose_binding_id().bytes() != product
                || vault.owner().bytes() != addresses.vault_owner.0.to_bytes()
                || selection.cursor.generation != vault.generation()
            {
                return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
            }
            let quantity = if action == StructuredClaimActionV1::WrapFull {
                maximum_full_vector_quantity(user, descriptor.primitive, basis.outcome_count)?
            } else {
                let holder = decode_canonical_wrapper_token_v1(
                    releases.token_2022.program_id.to_bytes(),
                    addresses.mint.0.to_bytes(),
                    accounts[25].address.to_bytes(),
                    accounts[12].address.to_bytes(),
                    accounts[25].data()?,
                )
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
                if holder.amount == 0 {
                    return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
                }
                holder.amount
            };
            let value = WrapperQuantityPayloadV1 {
                wrapper_product_id: product,
                quantity,
                user_generation: user.generation(),
                user_replay_sequence: user_replay.header().next_sequence(),
                vault_generation: vault.generation(),
                vault_replay_sequence: vault_replay.header().next_sequence(),
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
            let vault = PositionAccountV3::decode(accounts[8].data()?)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let replay = ReplayV3Envelope::decode(accounts[9].data()?, &OperatorSha256V1)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
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
            if root.binding.id(&OperatorSha256V1)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
                .bytes()
                != descriptor.structured_root_id
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
            validate_current_product_join(accounts, action, root)?;
            let family_terminal = action == StructuredClaimActionV1::RetireDescriptor
                && root.live_descriptor_count == 1;
            let value = VaultMutationPayloadV1 {
                wrapper_product_id: product,
                vault_generation: vault.generation(),
                vault_replay_sequence: replay.header().next_sequence(),
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
        if root.binding != root_binding || root.root_bump != root_pda.1 {
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
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator/structured-authority-state/v1\0");
    hash.update(structured_chain_state_id(accounts));
    hash.update(lookup_table_state_sha256);
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
    if link.state.phase() != SeriesMarketLinkPhaseV2::Active
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
        if bundle
            .id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
            != binding.compiler_bundle_id
            || attachment
                .id()
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
                != binding.attachment_plan_id
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
