//! Opaque, release-authenticated action material for operator projections.
//!
//! Construction accepts typed semantic-owner state and account projections,
//! never browser JSON or caller-authored instruction bytes. The resulting
//! artifact remains unsigned and blockhash-free. It can make an operator
//! control inspectable, but it cannot sign, submit, or predict poststate.

use crate::account_index::{FinalizedAccountAbsence, IndexedBranch};
use crate::collateral_release_catalog::CurrentCollateralExecutableAccountViewV1;
pub use crate::collateral_release_catalog::AuthenticatedCurrentCollateralReleaseV1 as StructuredCollateralCatalogEntryV1;
use crate::failure_action11_material::{
    ChainDerivedFailureAction11MaterialV1, FAILURE_ACTION11_ROLE_LABELS_V1,
    FAILURE_ACTION11_VALIDITY_SLOTS_V1,
};
use crate::rpc_index::{
    CanonicalIntentCoordinate, CanonicalIntentVariantV1, IndexedProgramRelease,
    ObservedRpcAccount, RpcCommitment,
};
use crate::operatord::KeeperActionSelection;
use crate::transaction_builder::{
    ExactEquation, IntegerUnit, OwnedInstructionDraft, ProtocolFlow,
    ProtocolTransactionBuilder, RuntimeAdmission, SemanticOwner, TransactionMessageVersionV1,
    UnsignedProtocolTransaction,
};
use crate::workflow_graph::{
    plan_source_crank, CanonicalActionCoordinate, ExplicitOperatorReleaseManifest,
    PlannedWorkflowNode, ResumableWorkflowCursor, WorkflowLane, WorkflowPosition,
    SourceCrankObservation, SourceWorkflowActionMaterial, WorkflowGraphError,
};
use clutch_solana_layout::registry::{
    AllocationStatus, DirectMarketAction, ExtensionAction, ExtensionFamily,
    DIRECT_MARKET_FAMILY_TAG, DIRECT_MARKET_FAMILY_VERSION, STRUCTURED_CLAIM_FAMILY_TAG,
    STRUCTURED_CLAIM_FAMILY_VERSION, SOURCE_SERIES_FAMILY_TAG, SOURCE_SERIES_FAMILY_VERSION,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::product_series::{
    MarketLifecycleReplayAccountV2, MarketLifecycleRootAccountV3, SeriesMarketLinkAccountV2,
    SeriesMarketLinkAccountV3, SeriesRegistryAccountV3, SeriesRegistryAccountV4,
    SERIES_REGISTRY_PDA_PREFIX_V1,
};
use clutch_solana_layout::product_series::MarketLifecycleRootAccountV2;
use clutch_solana_layout::{ProfileAccount, RealmAccount};
use clutch_collateral_adapter_v2::{
    AdapterReleaseV2, ClaimIssuanceBindingV1, ClaimLedgerV3, CollateralPolicyV2, HoardV2,
    MarketLiabilityLifecycleV1, ResolutionStateV5, ResolutionV5,
    CLAIM_LEDGER_V3_PDA_SEED_V1, COLLATERAL_POLICY_PDA_SEED_V1,
    HOARD_AUTHORITY_V2_PDA_SEED_V1, HOARD_TOKEN_V2_PDA_SEED_V1,
    HOARD_V2_PDA_SEED_V1, PROFILE_PDA_SEED_V1, REALM_PDA_SEED_V1,
};
use clutch_dealer_runtime_contract::{
    dealer_runtime_liveness_policy_id_v1, DealerActionReceiptV1, DealerFacilityReplayV1,
    DealerFundedDependenciesV2,
    DealerFutureCreditFundingV1, DealerLivenessCompartmentV1, DealerLivenessScheduleV1,
    DealerPhaseV2, DealerPolicyV1, DealerRuntimeActionV1, DealerRuntimeLivenessBindingV1,
    DealerSeriesObligationBindingV3, DealerSeriesObligationPhaseV1, DealerStateV3,
    DeletableRentOwnerV1, FacilityPositionBindingV2, FixedCodec as DealerFixedCodec,
    Id as DealerId, DEALER_ACTION_RECEIPT_PDA_DOMAIN_V1,
    DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V2,
    DEALER_FUTURE_CREDIT_FUNDING_PDA_DOMAIN_V1, DEALER_LIVENESS_SCHEDULE_PDA_DOMAIN_V1,
    DEALER_POLICY_CONTENT_DOMAIN_V1, DEALER_POLICY_PDA_DOMAIN_V1,
    DEALER_SERIES_OBLIGATION_PDA_DOMAIN_V1, DEALER_STATE_PDA_DOMAIN_V2,
};
use clutch_fractional_redemption_runtime::{
    FractionalCreditV2, FractionalInitializeIntentV1, FractionalLedgerPhaseV1,
    FractionalLedgerV1, FractionalPolicyV3, FractionalRedemptionActionV1,
    FractionalTerminalIntentV1, PayoutVectorV1,
    FRACTIONAL_CREDIT_PDA_PREFIX, FRACTIONAL_LEDGER_PDA_PREFIX,
    FRACTIONAL_POLICY_PDA_PREFIX,
};
use clutch_general_v2_contract::{
    MarketBindingV2, MarketBindingV5, MarketRuntimeV3AccountV1, MARKET_BINDING_SEED_DOMAIN_V1,
    MARKET_RUNTIME_SEED_DOMAIN_V1,
};
use clutch_product_series::{
    CompiledProductSeriesBundleV6, CompiledProductSeriesBundleV7, ContentId, FixedCodec,
    MarketFamilyCapabilityPolicyV1, MarketFamilyStatusV1, MarketFamilyV1,
    MarketFoundationAccountGraphV4,
    MarketFoundationSlotV4, MarketInstancePreimageV2, MarketLifecyclePhaseV2,
    MarketLifecyclePhaseV3, NativeClaimBasisV1,
    RegistryCapabilityProfileV4, RegistryProgramReleaseV2, RegistryReleaseLocusV2,
    SeriesAttachmentPlanV5, SeriesAttachmentPlanV6, SeriesFundingQuoteV6,
    SeriesFundingTermsV2, SeriesLinkObligationStatusV2, SeriesLinkObligationStatusV3,
    SeriesLinkObligationV2, SeriesLinkObligationV3, SeriesMarketLinkPhaseV2,
    SeriesMarketLinkBindingV3, SeriesMarketLinkPhaseV3,
    MARKET_FOUNDATION_CORE_SLOT_COUNT_V4, MARKET_FOUNDATION_MAX_OUTCOMES_V4,
    MARKET_FOUNDATION_SLOT_COUNT_V4,
};
use clutch_liveness::{
    RuntimeCompartmentKindV1, RuntimeCompartmentPhaseV1, RuntimeCompartmentV1,
    RuntimeLivenessPolicyV1,
};
use clutch_retirement::{
    PositionAccountV3, PositionLifecycleV3, PositionPurposeV3, ReplayV3Envelope,
    PositionV3Sha256Backend, ReplayV3HashBackend, ReplayV3Lifecycle,
    POSITION_V3_PDA_PREFIX, PURPOSE_REPLAY_V3_PDA_PREFIX,
};
use clutch_structured_claim::{ClaimVector, DeploymentBinding};
use clutch_structured_claim_adapter::{
    canonical_native_claim_id_v1, canonical_series_scoped_wrapper_product_id_v2,
    current_structured_account_meta_v1, current_structured_action_contract_v1,
    current_structured_alias_allowed_v1,
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

const OUTCOME_CUSTODY_PDA_DOMAIN_V1: &[u8] = b"dc:outcome-custody:v1";
const TREASURY_SERVICE_LEDGER_PDA_DOMAIN_V1: &[u8] = b"treasury-service-v1";

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

/// Checked independent claim-program row used by bearer Fractional actions.
/// The binding bytes are not authority by themselves: construction later
/// requires their content ID to equal the immutable binding ID decoded from
/// the market's Fractional policy.
#[derive(Clone, Copy, Debug)]
pub struct FractionalClaimCatalogEntryV1<'release> {
    binding: ClaimIssuanceBindingV1,
    program: &'release IndexedProgramRelease,
    artifact: RegistryProgramReleaseV2,
    artifact_owner: Address,
}

impl<'release> FractionalClaimCatalogEntryV1<'release> {
    /// Authenticate a checked Token program deployment and its base-owned
    /// release artifact. Binding authority remains deferred to the on-chain
    /// policy content-ID join in the action constructor.
    pub fn authenticate(
        binding: ClaimIssuanceBindingV1,
        program: &'release IndexedProgramRelease,
        artifact_owner: Address,
        artifact_account: &ObservedRpcAccount,
    ) -> Result<Self> {
        binding
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
            || binding.token_program.bytes() != program.program_id.to_bytes()
            || binding.token_program_deployment.bytes() != program.elf_sha256
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
        Ok(Self {
            binding,
            program,
            artifact,
            artifact_owner,
        })
    }
}

/// Exact base/collateral/claim release join for externally represented Eggs.
#[derive(Clone, Copy, Debug)]
pub struct FractionalExternalReleaseSetV1<'release> {
    base: &'release IndexedProgramRelease,
    collateral: StructuredCollateralCatalogEntryV1<'release>,
    claim: FractionalClaimCatalogEntryV1<'release>,
}

impl<'release> FractionalExternalReleaseSetV1<'release> {
    pub fn authenticate(
        base: &'release IndexedProgramRelease,
        collateral: StructuredCollateralCatalogEntryV1<'release>,
        claim: FractionalClaimCatalogEntryV1<'release>,
    ) -> Result<Self> {
        base.validate()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
        claim
            .binding
            .require_separate_from_collateral(collateral.adapter())
            .map_err(|_| CanonicalActionMaterialErrorV1::ReleaseMismatch)?;
        if collateral.artifact_owner() != base.program_id
            || claim.artifact_owner != base.program_id
            || collateral.program().capability_profile_id != base.capability_profile_id
            || claim.program.capability_profile_id != base.capability_profile_id
            || base
                .families
                .binary_search(&crate::rpc_index::CanonicalFamily::Fractional)
                .is_err()
        {
            return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
        }
        Ok(Self { base, collateral, claim })
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
            || collateral.program().capability_profile_id != base.capability_profile_id
            || collateral.artifact_owner() != base.program_id
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
            if !structured_release_intents_joined_v1(
                [
                    wrapper.enabled_intents.as_slice(),
                    base.enabled_intents.as_slice(),
                    token_2022.enabled_intents.as_slice(),
                ],
                coordinate,
            ) {
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

fn structured_release_intents_joined_v1(
    releases: [&[CanonicalIntentCoordinate]; 3],
    coordinate: CanonicalIntentCoordinate,
) -> bool {
    releases
        .iter()
        .all(|intents| intents.binary_search(&coordinate).is_ok())
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

    /// Retain absence of one exact program-owned PDA from an exhaustive
    /// finalized owner scan. The semantic constructor must still derive and
    /// equality-check the address before the absence is usable.
    pub(crate) fn absent_from_snapshot(
        address: Address,
        snapshot: &'account crate::rpc_index::FinalizedAccountSnapshotV1,
    ) -> Result<Self> {
        if address == Address::default()
            || snapshot.receipt().slot() == 0
            || snapshot.receipt().release_key().trim().is_empty()
            || snapshot.accounts().iter().any(|account| account.address == address)
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        Ok(Self {
            address,
            present: None,
            observed_slot: snapshot.receipt().slot(),
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

    /// Exact decoded table for a sibling semantic-owner constructor which has
    /// independently bound the same finalized observation.
    pub(crate) fn table(&self) -> AddressLookupTableAccount {
        self.table.clone()
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

impl PositionV3Sha256Backend for OperatorSha256V1 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        self.hashv(&[domain, body])
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
    pub(crate) const fn new(
        label: &'static str,
        address: Address,
        writable: bool,
        signer: bool,
    ) -> Self {
        Self {
            label,
            address,
            writable,
            signer,
        }
    }

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

/// Closed operator vocabulary for current Direct account roles.
///
/// This is an untrusted projection of onchain state, not an authorization
/// token. It exists to prevent a launcher from inventing positional metas or
/// privileges outside the exact action-specific frame checked again by SBF.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectAccountRoleV1 {
    ProductRoot,
    ProductReplay,
    ProductDirectGlobalLiveness,
    FounderSeriesLink,
    WritableFounderSeriesLink,
    SeriesFunding,
    SeriesRegistry,
    RegistryProgram,
    RegistryProgramData,
    RegistryReleaseArtifact,
    CapabilityProfileArtifact,
    SourceRelease,
    CompilerBundle,
    SeriesPlan,
    FundingTerms,
    SourceTemplate,
    RecoveryPolicy,
    FundingQuote,
    AttachmentPlan,
    FamilyCapabilityPolicy,
    LivenessSource,
    LivenessCandidate,
    LivenessClearing,
    LivenessSettlement,
    LivenessResolution,
    LivenessRetirement,
    LivenessRecovery,
    DirectRoot,
    DirectReplay,
    FreshReservation,
    WritableReservation,
    ReadonlyReservation,
    FreshSelection,
    Selection,
    DirectResolution,
    ActorPayer,
    Position,
    PositionReplay,
    Realm,
    CollateralProfile,
    CollateralPolicy,
    TokenProgram,
    GeneralMarketBinding,
    GeneralMarketRuntime,
    MarketInstance,
    MarketGenesis,
    SystemProgram,
    RentSysvar,
    ClockSysvar,
    PriceGrid,
    NativeClaimBasis,
    PriceMeasurePolicy,
    BatchPolicy,
    RevenuePolicyRecord,
    RevenuePolicy,
    NeutralSink,
    BondRefundOwner,
    RentRefundOwner,
    LivenessPolicy,
    Candidate,
    Keeper,
    CandidatePayer,
}

impl DirectAccountRoleV1 {
    const fn writable(self) -> bool {
        matches!(
            self,
            Self::ProductRoot
                | Self::ProductReplay
                | Self::ProductDirectGlobalLiveness
                | Self::WritableFounderSeriesLink
                | Self::DirectRoot
                | Self::DirectReplay
                | Self::FreshReservation
                | Self::WritableReservation
                | Self::FreshSelection
                | Self::Selection
                | Self::ActorPayer
                | Self::Position
                | Self::PositionReplay
                | Self::NeutralSink
                | Self::BondRefundOwner
                | Self::RentRefundOwner
                | Self::Candidate
                | Self::Keeper
                | Self::CandidatePayer
        )
    }

    const fn signer(self) -> bool {
        matches!(self, Self::ActorPayer | Self::Keeper)
    }
}

/// One named Direct address. Writable/signer bits are derived from the role;
/// callers cannot independently set them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectNamedAccountV1 {
    role: DirectAccountRoleV1,
    address: Address,
}

impl DirectNamedAccountV1 {
    pub fn new(role: DirectAccountRoleV1, address: Address) -> Result<Self> {
        if address == Address::default() {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
        Ok(Self { role, address })
    }

    #[must_use]
    pub const fn role(self) -> DirectAccountRoleV1 {
        self.role
    }

    #[must_use]
    pub const fn address(self) -> Address {
        self.address
    }
}

/// Exact action-specific Direct account projection for actions 1 through 13.
/// Action 1 is the closed Product-owned 41-role V3 foundation frame; no
/// generic account-meta escape hatch can substitute for its ordered graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectActionAccountsV1 {
    action: DirectMarketAction,
    accounts: Vec<DirectNamedAccountV1>,
}

impl DirectActionAccountsV1 {
    pub fn new(action: DirectMarketAction, accounts: Vec<DirectNamedAccountV1>) -> Result<Self> {
        validate_direct_account_roles_v1(action, &accounts)?;
        Ok(Self { action, accounts })
    }

    #[must_use]
    pub const fn action(&self) -> DirectMarketAction {
        self.action
    }

    #[must_use]
    pub fn accounts(&self) -> &[DirectNamedAccountV1] {
        &self.accounts
    }

    fn driver_account(&self) -> Result<Address> {
        let driver_role = if self.action == DirectMarketAction::InitializeMarket {
            DirectAccountRoleV1::ProductRoot
        } else {
            DirectAccountRoleV1::DirectRoot
        };
        self.accounts
            .iter()
            .find(|account| account.role == driver_role)
            .map(|account| account.address)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)
    }

    fn fee_payer(&self) -> Result<Address> {
        let preferred = if matches!(
            self.action,
            DirectMarketAction::FreezeBook
                | DirectMarketAction::BeginVerification
                | DirectMarketAction::VerifyCandidate
                | DirectMarketAction::FinalizeSelection
                | DirectMarketAction::SettlePair
                | DirectMarketAction::LapseEmpty
                | DirectMarketAction::LapseUnselected
                | DirectMarketAction::LapseSelected
                | DirectMarketAction::RetireTerminal
        ) {
            DirectAccountRoleV1::Keeper
        } else {
            DirectAccountRoleV1::ActorPayer
        };
        self.accounts
            .iter()
            .find(|account| account.role == preferred)
            .map(|account| account.address)
            .ok_or(CanonicalActionMaterialErrorV1::FeePayerMismatch)
    }

    fn instruction_parts(&self) -> (Vec<AccountMeta>, Vec<Address>) {
        let metas = self
            .accounts
            .iter()
            .map(|account| AccountMeta {
                pubkey: account.address,
                is_signer: account.role.signer(),
                is_writable: account.role.writable(),
            })
            .collect::<Vec<_>>();
        let signers = self
            .accounts
            .iter()
            .filter(|account| account.role.signer())
            .map(|account| account.address)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        (metas, signers)
    }
}

/// Direct material whose wire payload is owned by the current client codec and
/// whose positional metas are owned by [`DirectActionAccountsV1`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectWorkflowActionMaterialV1 {
    pub action_name: String,
    pub semantic_owner: SemanticOwner,
    pub sequence: u64,
    pub accounts: DirectActionAccountsV1,
    pub payload: clutch_client_contract::direct_market::DirectMarketClientPayloadV1,
    pub exact_equations: Vec<ExactEquation>,
    pub valid_before_slot: u64,
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
    variant: Option<CanonicalIntentVariantV1>,
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
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_chain_derived_direct_v2(
        release: &IndexedProgramRelease,
        coordinate: CanonicalIntentCoordinate,
        driver_account: Address,
        driver_account_slot: u64,
        cursor: ResumableWorkflowCursor,
        freshness: ActionFreshnessBoundaryV1,
        fee_payer: Address,
        account_roles: Vec<CanonicalAccountRoleV1>,
        planned: PlannedWorkflowNode,
        symbolic_postcondition_contract_id: [u8; 32],
    ) -> Result<Self> {
        freshness.validate()?;
        if symbolic_postcondition_contract_id == [0; 32]
            || cursor.observed_state_sha256 == [0; 32]
            || planned.manifest_sha256 != release.release_manifest_sha256
            || planned.cursor != cursor
            || planned.coordinate
                != CanonicalActionCoordinate::Direct(DirectMarketAction::FinalizeSelection)
            || !planned.reload_authoritative_accounts
            || planned.unsigned_transaction.has_recent_blockhash
            || planned.unsigned_transaction.signed
            || planned.unsigned_transaction.submitted
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
        validate_unsigned_direct_plan(coordinate, fee_payer, &account_roles, &planned)?;
        let release_key = release.key();
        let authority_state_sha256 = cursor.observed_state_sha256;
        let base_id = action_material_id(
            &release_key,
            &release_key,
            release.release_manifest_sha256,
            release.capability_profile_id,
            coordinate,
            driver_account,
            driver_account_slot,
            cursor,
            authority_state_sha256,
            freshness,
            fee_payer,
            &account_roles,
            &planned.unsigned_transaction,
        );
        let draft_id = Sha256::new()
            .chain_update(b"dragons-clutch/operator/direct-action8-material/v2\0")
            .chain_update(base_id)
            .chain_update(symbolic_postcondition_contract_id)
            .finalize()
            .into();
        Ok(Self {
            release_key: release_key.clone(),
            driver_release_key: release_key,
            release_manifest_sha256: release.release_manifest_sha256,
            capability_profile_id: release.capability_profile_id,
            coordinate,
            variant: None,
            driver_account,
            driver_account_slot,
            cursor,
            authority_state_sha256,
            freshness,
            fee_payer,
            account_roles,
            planned,
            draft_id,
        })
    }

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
    pub const fn variant(&self) -> Option<CanonicalIntentVariantV1> {
        self.variant
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
            && self.variant.is_none()
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


    /// Exact payload-scoped release join. Unlike ordinary coordinates, these
    /// variants deliberately require the coarse tuple to remain absent.
    #[must_use]
    pub fn matches_variant(
        &self,
        release: &IndexedProgramRelease,
        variant: CanonicalIntentVariantV1,
    ) -> bool {
        self.release_key == release.key()
            && self.release_manifest_sha256 == release.release_manifest_sha256
            && self.capability_profile_id == release.capability_profile_id
            && self.coordinate == variant.coordinate()
            && self.variant == Some(variant)
            && release.enabled_intents.binary_search(&variant.coordinate()).is_err()
            && release.enabled_intent_variants.binary_search(&variant).is_ok()
            && self.planned.reload_authoritative_accounts
            && !self.planned.unsigned_transaction.has_recent_blockhash
            && !self.planned.unsigned_transaction.signed
            && !self.planned.unsigned_transaction.submitted
    }
}

/// Construct one release-admitted Direct action from the closed `80/1`
/// payload codec and exact action-specific account grammar.
///
/// The result remains unsigned and cannot exist for a coordinate absent from
/// the checked release. Action 1 uses sequence zero; every later action uses a
/// nonzero replay sequence.
#[allow(clippy::too_many_arguments)]
pub fn construct_direct_action_material_v1(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    selection: &KeeperActionSelection,
    freshness: ActionFreshnessBoundaryV1,
    material: DirectWorkflowActionMaterialV1,
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
    let action = material.accounts.action();
    if matches!(
        action,
        DirectMarketAction::InitializeMarket
            | DirectMarketAction::SubmitCandidate
            | DirectMarketAction::BeginVerification
            | DirectMarketAction::VerifyCandidate
            | DirectMarketAction::SettlePair
            | DirectMarketAction::LapseEmpty
            | DirectMarketAction::LapseUnselected
            | DirectMarketAction::LapseSelected
            | DirectMarketAction::RetireTerminal
    ) {
        // Action 1 consumes the exact ProductRoot snapshot and is v0-only;
        // later actions consume current b1/v3+b2+b3 and exact descendants.
        // Their identities, deadlines, policies, refund owners, and postimages
        // are derived only by action-specific hostile-chain constructors. This
        // caller-shaped account grammar is withdrawn for those coordinates.
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let sequence_is_exact = if action == DirectMarketAction::InitializeMarket {
        material.sequence == 0
    } else {
        material.sequence != 0
    };
    if material.payload.action() != action
        || material.valid_before_slot != freshness.valid_before_slot
        || !sequence_is_exact
        || material.action_name != direct_selection_action(action)
    {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    let coordinate = CanonicalIntentCoordinate {
        family_tag: DIRECT_MARKET_FAMILY_TAG,
        family_version: DIRECT_MARKET_FAMILY_VERSION,
        local_action: action.tag(),
    };
    if release.enabled_intents.binary_search(&coordinate).is_err() {
        return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
    }
    let driver_account = material.accounts.driver_account()?;
    if selection.release_key != release.key()
        || selection.effective_commitment != crate::rpc_index::RpcCommitment::Finalized
        || selection.action != material.action_name
        || selection.account != driver_account
        || freshness.observed_slot < selection.account_slot
    {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    manifest
        .admits_owner(&material.semantic_owner)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let fee_payer = material.accounts.fee_payer()?;
    if builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
        || builder.payer() != fee_payer
    {
        return Err(CanonicalActionMaterialErrorV1::FeePayerMismatch);
    }
    let account_roles = material
        .accounts
        .accounts()
        .iter()
        .map(|account| CanonicalAccountRoleV1 {
            label: direct_role_label_v1(account.role),
            address: account.address,
            writable: account.role.writable(),
            signer: account.role.signer(),
        })
        .collect::<Vec<_>>();
    let (accounts, required_signers) = material.accounts.instruction_parts();
    let draft = OwnedInstructionDraft::enabled_direct_market_request_v1(
        material.action_name,
        material.semantic_owner,
        manifest.clutch.program_id,
        accounts,
        required_signers,
        material.exact_equations,
        material.sequence,
        &material.payload,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let unsigned_transaction = builder
        .build_atomic(&[draft])
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let planned = PlannedWorkflowNode {
        manifest_sha256: manifest.manifest_sha256,
        cursor: selection.cursor,
        coordinate: CanonicalActionCoordinate::Direct(action),
        unsigned_transaction,
        reload_authoritative_accounts: true,
    };
    validate_unsigned_direct_plan(coordinate, fee_payer, &account_roles, &planned)?;
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
        fee_payer,
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key,
        driver_release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        variant: None,
        driver_account: selection.account,
        driver_account_slot: selection.account_slot,
        cursor: selection.cursor,
        authority_state_sha256,
        freshness,
        fee_payer,
        account_roles,
        planned,
        draft_id,
    })
}

/// Finish one current Direct action whose payload, accounts, equations, and
/// postimages were already derived from one hostile finalized chain snapshot.
///
/// This boundary is crate-private so no browser/API caller can replace the
/// dedicated Direct semantic owner with a generic account or payload DTO.
#[allow(clippy::too_many_arguments)]
pub(crate) fn finish_chain_derived_direct_material_v2(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    selection: &KeeperActionSelection,
    freshness: ActionFreshnessBoundaryV1,
    action: DirectMarketAction,
    sequence: u64,
    accounts: Vec<AccountMeta>,
    required_signers: Vec<Address>,
    account_roles: Vec<CanonicalAccountRoleV1>,
    equations: Vec<ExactEquation>,
    payload: clutch_client_contract::direct_market::DirectMarketClientPayloadV1,
) -> Result<CanonicalActionMaterialV1> {
    release
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    manifest
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    freshness.validate()?;
    if !matches!(
        action,
        DirectMarketAction::SubmitCandidate
            | DirectMarketAction::BeginVerification
            | DirectMarketAction::VerifyCandidate
            | DirectMarketAction::SettlePair
            | DirectMarketAction::LapseEmpty
            | DirectMarketAction::LapseUnselected
            | DirectMarketAction::LapseSelected
            | DirectMarketAction::RetireTerminal
    ) || payload.action() != action
        || sequence == 0
        || release.program_id != manifest.clutch.program_id
        || release.program_data != manifest.clutch.program_data
        || release.deployment_slot != manifest.clutch.deployment_slot
        || release.elf_sha256 != manifest.clutch.elf_sha256
        || release.release_manifest_sha256 != manifest.manifest_sha256
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    let coordinate = CanonicalIntentCoordinate {
        family_tag: DIRECT_MARKET_FAMILY_TAG,
        family_version: DIRECT_MARKET_FAMILY_VERSION,
        local_action: action.tag(),
    };
    if release.enabled_intents.binary_search(&coordinate).is_err() {
        return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
    }
    let action_name = direct_selection_action(action);
    let root = account_roles
        .iter()
        .find(|role| role.label == "direct-root")
        .map(|role| role.address)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if selection.release_key != release.key()
        || selection.observed_commitment != crate::rpc_index::RpcCommitment::Finalized
        || selection.effective_commitment != crate::rpc_index::RpcCommitment::Finalized
        || selection.action != action_name
        || selection.account != root
        || freshness.observed_slot != selection.account_slot
    {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    let semantic_owner = manifest
        .semantic_releases
        .iter()
        .find(|owner| {
            owner.package == "clutch-direct-market-runtime" && owner.schema == "current-v1"
        })
        .cloned()
        .ok_or(CanonicalActionMaterialErrorV1::InvalidRelease)?;
    manifest
        .admits_owner(&semantic_owner)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    let fee_payer = required_signers
        .first()
        .copied()
        .ok_or(CanonicalActionMaterialErrorV1::FeePayerMismatch)?;
    if builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
        || builder.payer() != fee_payer
    {
        return Err(CanonicalActionMaterialErrorV1::FeePayerMismatch);
    }
    let draft = OwnedInstructionDraft::enabled_direct_market_request_v1(
        action_name,
        semantic_owner,
        release.program_id,
        accounts,
        required_signers,
        equations,
        sequence,
        &payload,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let unsigned_transaction = builder
        .build_atomic(&[draft])
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let planned = PlannedWorkflowNode {
        manifest_sha256: manifest.manifest_sha256,
        cursor: selection.cursor,
        coordinate: CanonicalActionCoordinate::Direct(action),
        unsigned_transaction,
        reload_authoritative_accounts: true,
    };
    validate_unsigned_direct_plan(coordinate, fee_payer, &account_roles, &planned)?;
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
        fee_payer,
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key,
        driver_release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        variant: None,
        driver_account: selection.account,
        driver_account_slot: selection.account_slot,
        cursor: selection.cursor,
        authority_state_sha256,
        freshness,
        fee_payer,
        account_roles,
        planned,
        draft_id,
    })
}

/// Finish the sole current Direct action-1 artifact after the dedicated
/// Product-root snapshot owner has derived and authenticated every role.
/// Unlike the historical generic constructor, this boundary fixes sequence
/// and payload to empty action 1 and requires one finalized ALT-backed v0
/// message for the exact 41-account frame.
#[allow(clippy::too_many_arguments)]
pub(crate) fn finish_chain_derived_direct_action1_material_v2(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    workflow_id: [u8; 32],
    freshness: ActionFreshnessBoundaryV1,
    product_root: Address,
    product_root_slot: u64,
    generation: u64,
    authority_state_sha256: [u8; 32],
    accounts: Vec<AccountMeta>,
    account_roles: Vec<CanonicalAccountRoleV1>,
    equations: Vec<ExactEquation>,
    lookup_table: &StructuredAddressLookupTableV1,
) -> Result<CanonicalActionMaterialV1> {
    release.validate().map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    manifest.validate().map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    freshness.validate()?;
    let action = DirectMarketAction::InitializeMarket;
    let coordinate = CanonicalIntentCoordinate {
        family_tag: DIRECT_MARKET_FAMILY_TAG,
        family_version: DIRECT_MARKET_FAMILY_VERSION,
        local_action: action.tag(),
    };
    if workflow_id == [0; 32]
        || authority_state_sha256 == [0; 32]
        || product_root == Address::default()
        || product_root_slot == 0
        || generation == 0
        || accounts.len() != 41
        || account_roles.len() != 41
        || release.enabled_intents.binary_search(&coordinate).is_err()
        || release.program_id != manifest.clutch.program_id
        || release.program_data != manifest.clutch.program_data
        || release.deployment_slot != manifest.clutch.deployment_slot
        || release.elf_sha256 != manifest.clutch.elf_sha256
        || release.release_manifest_sha256 != manifest.manifest_sha256
        || builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
        || lookup_table.observed_slot() > freshness.observed_slot
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    let semantic_owner = manifest
        .semantic_releases
        .iter()
        .find(|owner| {
            owner.package == "clutch-direct-market-runtime"
                && owner.schema == "current-v1"
        })
        .cloned()
        .ok_or(CanonicalActionMaterialErrorV1::InvalidRelease)?;
    manifest
        .admits_owner(&semantic_owner)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    let equation_count = equations.len();
    let draft = OwnedInstructionDraft::enabled_direct_initialize_market_request_v2(
        semantic_owner,
        release.program_id,
        accounts,
        builder.payer(),
        equations,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let unsigned_transaction = builder
        .build_exact_v0(
            draft,
            lookup_table.table(),
            lookup_table.observed_slot(),
            lookup_table.state_sha256(),
        )
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if unsigned_transaction.message_version != TransactionMessageVersionV1::V0
        || unsigned_transaction.address_lookup_tables.len() != 1
        || unsigned_transaction.exact_equations.len() != equation_count
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let cursor = ResumableWorkflowCursor {
        workflow_id,
        lane: WorkflowLane::Creation,
        generation,
        position: WorkflowPosition {
            phase: u16::from(action.tag()),
            item: 0,
        },
        observed_state_sha256: authority_state_sha256,
    };
    let planned = PlannedWorkflowNode {
        manifest_sha256: manifest.manifest_sha256,
        cursor,
        coordinate: CanonicalActionCoordinate::Direct(action),
        unsigned_transaction,
        reload_authoritative_accounts: true,
    };
    validate_unsigned_direct_plan(coordinate, builder.payer(), &account_roles, &planned)?;
    let release_key = release.key();
    let draft_id = action_material_id(
        &release_key,
        &release_key,
        release.release_manifest_sha256,
        release.capability_profile_id,
        coordinate,
        product_root,
        product_root_slot,
        cursor,
        authority_state_sha256,
        freshness,
        builder.payer(),
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key: release_key.clone(),
        driver_release_key: release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        variant: None,
        driver_account: product_root,
        driver_account_slot: product_root_slot,
        cursor,
        authority_state_sha256,
        freshness,
        fee_payer: builder.payer(),
        account_roles,
        planned,
        draft_id,
    })
}

pub(crate) const fn chain_derived_direct_role_v2(
    label: &'static str,
    address: Address,
    writable: bool,
    signer: bool,
) -> CanonicalAccountRoleV1 {
    CanonicalAccountRoleV1 {
        label,
        address,
        writable,
        signer,
    }
}

fn validate_direct_account_roles_v1(
    action: DirectMarketAction,
    accounts: &[DirectNamedAccountV1],
) -> Result<()> {
    use DirectAccountRoleV1 as Role;
    if accounts.len() > 41 {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    match action {
        DirectMarketAction::InitializeMarket => require_direct_exact_roles_v1(
            accounts,
            &[
                Role::GeneralMarketBinding,
                Role::GeneralMarketRuntime,
                Role::ProductRoot,
                Role::WritableFounderSeriesLink,
                Role::SeriesFunding,
                Role::SeriesRegistry,
                Role::RegistryProgram,
                Role::RegistryProgramData,
                Role::RegistryReleaseArtifact,
                Role::CapabilityProfileArtifact,
                Role::SourceRelease,
                Role::CompilerBundle,
                Role::MarketInstance,
                Role::Realm,
                Role::RevenuePolicyRecord,
                Role::RevenuePolicy,
                Role::SeriesPlan,
                Role::FundingTerms,
                Role::SourceTemplate,
                Role::NativeClaimBasis,
                Role::RecoveryPolicy,
                Role::PriceMeasurePolicy,
                Role::MarketGenesis,
                Role::FundingQuote,
                Role::AttachmentPlan,
                Role::ProductReplay,
                Role::FamilyCapabilityPolicy,
                Role::ProductDirectGlobalLiveness,
                Role::LivenessSource,
                Role::LivenessCandidate,
                Role::LivenessClearing,
                Role::LivenessSettlement,
                Role::LivenessResolution,
                Role::LivenessRetirement,
                Role::LivenessRecovery,
                Role::DirectRoot,
                Role::DirectReplay,
                Role::ActorPayer,
                Role::SystemProgram,
                Role::RentSysvar,
                Role::ClockSysvar,
            ],
        ),
        DirectMarketAction::AdmitOrder => {
            let end = require_direct_roles_v1(
                accounts,
                0,
                &[
                    Role::DirectRoot,
                    Role::DirectReplay,
                    Role::FreshReservation,
                    Role::ActorPayer,
                    Role::Position,
                    Role::PositionReplay,
                    Role::Realm,
                    Role::CollateralProfile,
                    Role::CollateralPolicy,
                    Role::TokenProgram,
                    Role::GeneralMarketBinding,
                    Role::GeneralMarketRuntime,
                    Role::MarketInstance,
                    Role::SystemProgram,
                    Role::RentSysvar,
                    Role::ClockSysvar,
                    Role::CompilerBundle,
                    Role::MarketGenesis,
                    Role::PriceGrid,
                ],
            )?;
            if accounts.len() == end
                || accounts.len() == end + 1
                    && accounts[end].role == Role::ReadonlyReservation
            {
                Ok(())
            } else {
                Err(CanonicalActionMaterialErrorV1::InvalidPlan)
            }
        }
        DirectMarketAction::CancelOrder => require_direct_exact_roles_v1(
            accounts,
            &[
                Role::DirectRoot,
                Role::DirectReplay,
                Role::WritableReservation,
                Role::ActorPayer,
                Role::Position,
                Role::PositionReplay,
                Role::Realm,
                Role::CollateralProfile,
                Role::CollateralPolicy,
                Role::TokenProgram,
                Role::GeneralMarketBinding,
                Role::GeneralMarketRuntime,
                Role::MarketInstance,
                Role::MarketGenesis,
                Role::NeutralSink,
                Role::ClockSysvar,
            ],
        ),
        DirectMarketAction::FreezeBook => {
            let mut index = require_direct_roles_v1(
                accounts,
                0,
                &[
                    Role::DirectRoot,
                    Role::DirectReplay,
                    Role::FreshSelection,
                    Role::ActorPayer,
                    Role::SystemProgram,
                    Role::RentSysvar,
                    Role::ClockSysvar,
                    Role::CompilerBundle,
                    Role::NativeClaimBasis,
                    Role::PriceMeasurePolicy,
                    Role::MarketGenesis,
                    Role::PriceGrid,
                ],
            )?;
            index = consume_direct_roles_v1(accounts, index, Role::ReadonlyReservation, 0, 2)?;
            require_direct_suffix_v1(accounts, index)
        }
        DirectMarketAction::SubmitCandidate => {
            let end = require_direct_roles_v1(
                accounts,
                0,
                &[
                    Role::DirectRoot,
                    Role::DirectReplay,
                    Role::Selection,
                    Role::ClockSysvar,
                    Role::ActorPayer,
                    Role::SystemProgram,
                ],
            )?;
            if accounts.len() == end
                || accounts.len() == end + 1 && accounts[end].role == Role::BondRefundOwner
            {
                Ok(())
            } else {
                Err(CanonicalActionMaterialErrorV1::InvalidPlan)
            }
        }
        DirectMarketAction::BeginVerification | DirectMarketAction::VerifyCandidate => {
            let index = require_direct_roles_v1(
                accounts,
                0,
                &[
                    Role::DirectRoot,
                    Role::DirectReplay,
                    Role::Selection,
                    Role::ClockSysvar,
                ],
            )?;
            require_direct_suffix_v1(accounts, index)
        }
        DirectMarketAction::FinalizeSelection => validate_direct_finalize_roles_v1(accounts),
        DirectMarketAction::SettlePair => validate_direct_economic_roles_v1(accounts, true, 2, 2),
        DirectMarketAction::LapseEmpty => {
            if accounts.get(4).map(|account| account.role) == Some(Role::SystemProgram) {
                validate_direct_missed_freeze_roles_v1(accounts)
            } else {
                validate_direct_economic_roles_v1(accounts, false, 0, 2)
            }
        }
        DirectMarketAction::LapseUnselected | DirectMarketAction::LapseSelected => {
            validate_direct_economic_roles_v1(accounts, false, 0, 2)
        }
        DirectMarketAction::RetireTerminal => {
            let mut index = require_direct_roles_v1(
                accounts,
                0,
                &[
                    Role::ProductRoot,
                    Role::FounderSeriesLink,
                    Role::DirectRoot,
                    Role::DirectReplay,
                    Role::Selection,
                    Role::DirectResolution,
                    Role::ClockSysvar,
                    Role::NeutralSink,
                ],
            )?;
            index =
                consume_direct_roles_v1(accounts, index, Role::WritableReservation, 0, 2)?;
            index = consume_direct_roles_v1(accounts, index, Role::RentRefundOwner, 1, 5)?;
            index = require_direct_roles_v1(
                accounts,
                index,
                &[Role::ProductDirectGlobalLiveness],
            )?;
            require_direct_suffix_v1(accounts, index)
        }
    }
}

fn validate_direct_finalize_roles_v1(accounts: &[DirectNamedAccountV1]) -> Result<()> {
    use DirectAccountRoleV1 as Role;
    if accounts.get(3).map(|account| account.role) == Some(Role::Realm) {
        return validate_direct_economic_roles_v1(accounts, false, 0, 2);
    }
    let mut index = require_direct_roles_v1(
        accounts,
        0,
        &[
            Role::DirectRoot,
            Role::DirectReplay,
            Role::Selection,
            Role::ClockSysvar,
        ],
    )?;
    index = consume_direct_roles_v1(accounts, index, Role::BondRefundOwner, 0, 3)?;
    require_direct_suffix_v1(accounts, index)
}

fn validate_direct_economic_roles_v1(
    accounts: &[DirectNamedAccountV1],
    fee_bearing: bool,
    minimum_endpoints: usize,
    maximum_endpoints: usize,
) -> Result<()> {
    use DirectAccountRoleV1 as Role;
    let mut index = require_direct_roles_v1(
        accounts,
        0,
        &[
            Role::DirectRoot,
            Role::DirectReplay,
            Role::Selection,
            Role::Realm,
            Role::CollateralProfile,
            Role::CollateralPolicy,
            Role::TokenProgram,
            Role::GeneralMarketBinding,
            Role::GeneralMarketRuntime,
            Role::MarketInstance,
            Role::MarketGenesis,
            Role::ClockSysvar,
        ],
    )?;
    let mut endpoints = 0_usize;
    while accounts.get(index).map(|account| account.role) == Some(Role::WritableReservation) {
        index = require_direct_roles_v1(
            accounts,
            index,
            &[
                Role::WritableReservation,
                Role::Position,
                Role::PositionReplay,
            ],
        )?;
        endpoints += 1;
        if endpoints > maximum_endpoints {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    if endpoints < minimum_endpoints {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    if fee_bearing {
        index = require_direct_roles_v1(
            accounts,
            index,
            &[
                Role::BatchPolicy,
                Role::RevenuePolicyRecord,
                Role::RevenuePolicy,
            ],
        )?;
        if accounts.get(index).map(|account| account.role) == Some(Role::Position) {
            index = require_direct_roles_v1(
                accounts,
                index,
                &[Role::Position, Role::PositionReplay],
            )?;
        }
    }
    index = consume_direct_roles_v1(accounts, index, Role::BondRefundOwner, 0, 3)?;
    require_direct_suffix_v1(accounts, index)
}

fn validate_direct_missed_freeze_roles_v1(accounts: &[DirectNamedAccountV1]) -> Result<()> {
    use DirectAccountRoleV1 as Role;
    let mut index = require_direct_roles_v1(
        accounts,
        0,
        &[
            Role::DirectRoot,
            Role::DirectReplay,
            Role::FreshSelection,
            Role::ActorPayer,
            Role::SystemProgram,
            Role::RentSysvar,
            Role::ClockSysvar,
            Role::CompilerBundle,
            Role::NativeClaimBasis,
            Role::PriceMeasurePolicy,
            Role::MarketGenesis,
            Role::PriceGrid,
            Role::Realm,
            Role::CollateralProfile,
            Role::CollateralPolicy,
            Role::TokenProgram,
            Role::GeneralMarketBinding,
            Role::GeneralMarketRuntime,
            Role::MarketInstance,
        ],
    )?;
    let mut endpoints = 0_usize;
    while accounts.get(index).map(|account| account.role) == Some(Role::WritableReservation) {
        index = require_direct_roles_v1(
            accounts,
            index,
            &[
                Role::WritableReservation,
                Role::Position,
                Role::PositionReplay,
            ],
        )?;
        endpoints += 1;
        if endpoints > 2 {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    require_direct_suffix_v1(accounts, index)
}

fn require_direct_suffix_v1(accounts: &[DirectNamedAccountV1], index: usize) -> Result<()> {
    use DirectAccountRoleV1 as Role;
    let end = require_direct_roles_v1(
        accounts,
        index,
        &[
            Role::LivenessPolicy,
            Role::Candidate,
            Role::Keeper,
            Role::CandidatePayer,
        ],
    )?;
    if end == accounts.len() {
        Ok(())
    } else {
        Err(CanonicalActionMaterialErrorV1::InvalidPlan)
    }
}

fn consume_direct_roles_v1(
    accounts: &[DirectNamedAccountV1],
    mut index: usize,
    role: DirectAccountRoleV1,
    minimum: usize,
    maximum: usize,
) -> Result<usize> {
    let start = index;
    while accounts.get(index).map(|account| account.role) == Some(role) {
        index += 1;
        if index - start > maximum {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    if index - start < minimum {
        Err(CanonicalActionMaterialErrorV1::InvalidPlan)
    } else {
        Ok(index)
    }
}

fn require_direct_exact_roles_v1(
    accounts: &[DirectNamedAccountV1],
    expected: &[DirectAccountRoleV1],
) -> Result<()> {
    let end = require_direct_roles_v1(accounts, 0, expected)?;
    if end == accounts.len() {
        Ok(())
    } else {
        Err(CanonicalActionMaterialErrorV1::InvalidPlan)
    }
}

fn require_direct_roles_v1(
    accounts: &[DirectNamedAccountV1],
    start: usize,
    expected: &[DirectAccountRoleV1],
) -> Result<usize> {
    let end = start
        .checked_add(expected.len())
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if accounts.get(start..end).map(|values| {
        values
            .iter()
            .zip(expected.iter())
            .all(|(account, role)| account.role == *role)
    }) == Some(true)
    {
        Ok(end)
    } else {
        Err(CanonicalActionMaterialErrorV1::InvalidPlan)
    }
/// Join the current Failure semantic-owner material to the exact finalized
/// cell cursor selected by the daemon. This boundary adds only the fee payer
/// and opaque API envelope; all roles and wire bytes remain chain-derived.
pub fn construct_failure_action11_action_material_v1(
    release: &IndexedProgramRelease,
    builder: &ProtocolTransactionBuilder,
    selection: &KeeperActionSelection,
    material: &ChainDerivedFailureAction11MaterialV1,
) -> Result<CanonicalActionMaterialV1> {
    let coordinate = CanonicalIntentCoordinate {
        family_tag: clutch_solana_layout::registry::RECOVERY_FAMILY_TAG,
        family_version: clutch_solana_layout::registry::RECOVERY_FAMILY_VERSION,
        local_action: clutch_solana_layout::registry::RecoveryAction::AdvanceIntervalConsensus
            .tag(),
    };
    let freshness = ActionFreshnessBoundaryV1 {
        observed_slot: material.observed_slot(),
        valid_before_slot: material.valid_before_slot(),
        maximum_validity_slots: FAILURE_ACTION11_VALIDITY_SLOTS_V1,
    };
    freshness.validate()?;
    if selection.action != "advance-failure-interval-consensus"
        || selection.account != material.driver_account()
        || selection.account_slot != material.observed_slot()
        || selection.release_key != release.key()
        || selection.observed_commitment != RpcCommitment::Finalized
        || selection.effective_commitment != RpcCommitment::Finalized
        || selection.cursor.lane != WorkflowLane::FailureRecovery
        || selection.cursor.generation != material.generation()
        || selection.cursor.position
            != (WorkflowPosition {
                phase: 1,
                item: material.transition_nonce(),
            })
        || selection.cursor.observed_state_sha256 == [0; 32]
        || builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.release_manifest_sha256
    {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    let transaction = material
        .build_unsigned_transaction(release, builder)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if transaction.flows != [ProtocolFlow::FailureRecovery]
        || transaction.actions.len() != 1
        || transaction.actions[0] != "advance-failure-interval-consensus-v1"
        || transaction.runtime_admissions != [RuntimeAdmission::ReleaseBoundEnabled]
        || transaction.required_signers != [builder.payer()]
        || transaction.message_version != TransactionMessageVersionV1::V0
        || transaction.address_lookup_tables.len() != 1
        || transaction.has_recent_blockhash
        || transaction.signed
        || transaction.submitted
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let metas = material.account_metas();
    if metas.len() != FAILURE_ACTION11_ROLE_LABELS_V1.len() {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let account_roles = metas
        .iter()
        .zip(FAILURE_ACTION11_ROLE_LABELS_V1)
        .map(|(meta, label)| CanonicalAccountRoleV1 {
            label,
            address: meta.pubkey,
            writable: meta.is_writable,
            signer: meta.is_signer,
        })
        .collect::<Vec<_>>();
    let cursor = selection.cursor;
    let planned = PlannedWorkflowNode {
        manifest_sha256: release.release_manifest_sha256,
        cursor,
        coordinate: CanonicalActionCoordinate::Recovery(
            clutch_solana_layout::registry::RecoveryAction::AdvanceIntervalConsensus,
        ),
        unsigned_transaction: transaction,
        reload_authoritative_accounts: true,
    };
    let release_key = release.key();
    let authority_state_sha256 = material.state_sha256();
    let draft_id = action_material_id(
        &release_key,
        &release_key,
        release.release_manifest_sha256,
        release.capability_profile_id,
        coordinate,
        selection.account,
        selection.account_slot,
        cursor,
        authority_state_sha256,
        freshness,
        builder.payer(),
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key: release_key.clone(),
        driver_release_key: release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        variant: None,
        driver_account: selection.account,
        driver_account_slot: selection.account_slot,
        cursor,
        authority_state_sha256,
        freshness,
        fee_payer: builder.payer(),
        account_roles,
        planned,
        draft_id,
    })
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
        releases.collateral.receipt_id(),
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
                releases.collateral.receipt_id(),
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
        .build_exact_v0(
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
            releases.collateral.receipt_id(),
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
        variant: None,
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
        38 => StructuredClaimActionV1::CreateDescriptor,
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
            let redeem = accounts
                .get(13)
                .and_then(|account| account.data().ok())
                .is_some_and(|body| StructuredClaimDescriptorV2::decode(body).is_ok());
            if redeem {
                detect_full_vector_direction_v1(accounts, true)?
            } else {
                return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
            }
        }
        34 => StructuredClaimActionV1::RetireDescriptor,
        _ => return Err(CanonicalActionMaterialErrorV1::InvalidChainState),
    };
    let (driver_index, generation, item) = if action
        == StructuredClaimActionV1::CreateDescriptor
    {
        let link = SeriesMarketLinkAccountV3::decode(accounts[26].data()?)
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
    let (collateral_program, token_program) = match action {
        StructuredClaimActionV1::CreateDescriptor => (7, 19),
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => (4, 18),
        StructuredClaimActionV1::CompactDonation
        | StructuredClaimActionV1::RetireDescriptor => (4, 15),
    };
    let same_token_release =
        accounts[collateral_program].address == accounts[token_program].address;
    let mut left = 0_usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let permitted_release_alias = same_token_release
                && current_structured_alias_allowed_v1(action, left, right);
            if accounts[left].address == accounts[right].address && !permitted_release_alias {
                return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
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
                    releases.collateral.program().program_id,
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
    let link = SeriesMarketLinkAccountV3::decode(accounts[26].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let link_binding = link.state.binding();
    if accounts[26].owner()? != releases.base.program_id
        || link.state.phase() != SeriesMarketLinkPhaseV3::Active
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
    let product_root = validate_structured_product_root_v3(
        releases.base.program_id,
        accounts[35],
        &link_binding,
    )?;
    let product_replay = MarketLifecycleReplayAccountV2::decode(accounts[36].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let replay_binding = product_replay.state.binding();
    let family_policy = MarketFamilyCapabilityPolicyV1::decode(accounts[37].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let family_policy_id = family_policy
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    verify_product_artifact(
        releases.base.program_id,
        accounts[37],
        ArtifactKind::MarketFamilyCapabilityPolicyV1,
        family_policy_id.bytes(),
    )?;
    if accounts[36].owner()? != releases.base.program_id
        || accounts[36].executable()
        || replay_binding.replay_account_id.bytes() != accounts[36].address.to_bytes()
        || replay_binding.lifecycle_root_account_id.bytes() != accounts[35].address.to_bytes()
        || replay_binding.market_instance_id != link_binding.market_instance_id
        || replay_binding.generation != link_binding.generation
        || replay_binding.market_family_capability_policy_id
            != family_policy_id.content_id()
        || replay_binding.registry_release_id.content_id()
            != product_root.state.binding_ref().registry_release_id
        || replay_binding.capability_profile_id.content_id()
            != product_root.state.binding_ref().capability_profile_id
        || family_policy.registry_capability_profile_id.content_id()
            != link_binding.capability_profile_id
        || family_policy.realm_id != product_root.state.binding_ref().realm_id
        || family_policy.collateral_profile_id
            != product_root.state.binding_ref().collateral_profile_id
        || !family_policy.is_enabled(MarketFamilyV1::Structured)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let bundle = CompiledProductSeriesBundleV7::decode(accounts[27].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let bundle_id = bundle
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let attachment = SeriesAttachmentPlanV6::decode(accounts[28].data()?)
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
        ArtifactKind::CompiledProductSeriesBundleV7,
        bundle_id.bytes(),
    )?;
    verify_product_artifact(
        releases.base.program_id,
        accounts[28],
        ArtifactKind::SeriesAttachmentPlanV6,
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

    let registry = SeriesRegistryAccountV4::decode(accounts[30].data()?)
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
        || product_root.state.binding_ref().registry_release_id != bundle.registry_release_id
        || product_root.state.binding_ref().capability_profile_id
            != link_binding.capability_profile_id
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
    let structured_status = link
        .state
        .obligation_status(SeriesLinkObligationV3::Structured);
    let wrapper_status = link.state.obligation_status(SeriesLinkObligationV3::Wrapper);
    let product_family_status = product_root
        .state
        .product_families()
        .family(MarketFamilyV1::Structured)
        .status();
    if (product_link_writable
        && (structured_status != SeriesLinkObligationStatusV3::EnabledNeverFounded
            || wrapper_status != SeriesLinkObligationStatusV3::EnabledNeverFounded
            || product_family_status != MarketFamilyStatusV1::EnabledNeverFounded))
        || (!product_link_writable
            && (structured_status != SeriesLinkObligationStatusV3::Live
                || wrapper_status != SeriesLinkObligationStatusV3::Live
                || product_family_status != MarketFamilyStatusV1::Live))
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
                    .obligation_admission_receipt_id(SeriesLinkObligationV3::Wrapper)
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
    if accounts[collateral_program_index].address != releases.collateral.program().program_id
        || accounts[collateral_data_index].address != releases.collateral.program().program_data
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    let collateral_program = CurrentCollateralExecutableAccountViewV1::from_parts(
        accounts[collateral_program_index].address,
        accounts[collateral_program_index].owner()?,
        accounts[collateral_program_index].executable(),
        accounts[collateral_program_index].data()?,
    );
    let collateral_programdata = CurrentCollateralExecutableAccountViewV1::from_parts(
        accounts[collateral_data_index].address,
        accounts[collateral_data_index].owner()?,
        accounts[collateral_data_index].executable(),
        accounts[collateral_data_index].data()?,
    );
    releases
        .collateral
        .reauthenticate_executable(collateral_program, collateral_programdata)
        .map_err(|_| CanonicalActionMaterialErrorV1::ReleaseMismatch)?;

    let realm = RealmAccount::decode(accounts[realm_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let profile = ProfileAccount::decode(accounts[profile_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let policy = CollateralPolicyV2::decode(accounts[policy_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let policy_id = policy
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let selected_collateral = releases
        .collateral
        .select_for(releases.base, policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::ReleaseMismatch)?;
    let collateral_release_id = selected_collateral.entry().adapter_id();
    if profile.realm != realm.realm
        || profile.profile != realm.profile
        || profile.collateral_policy_id.bytes() != selected_collateral.policy_id()
        || policy_id.bytes() != selected_collateral.policy_id()
        || profile.adapter_release_id.bytes() != collateral_release_id
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
) -> Result<(u64, PositionAccountV3)> {
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

fn validate_structured_product_root_v3(
    base_program: Address,
    account: StructuredChainAccountV1<'_>,
    link_binding: &SeriesMarketLinkBindingV3,
) -> Result<MarketLifecycleRootAccountV3> {
    let root = MarketLifecycleRootAccountV3::decode(account.data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let binding = root.state.binding_ref();
    let binding_id = binding
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let root_pda = Address::find_program_address(
        &[
            PRODUCT_MARKET_LIFECYCLE_ROOT_PDA_DOMAIN_V1,
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
        ],
        &base_program,
    );
    if account.owner()? != base_program
        || account.executable()
        || account.address != root_pda.0
        || root.stored_bump != root_pda.1
        || observed_lamports(account) < root.rent_principal_lamports
        || !matches!(
            root.state.phase(),
            MarketLifecyclePhaseV3::Active | MarketLifecyclePhaseV3::Retiring
        )
        || account.address.to_bytes() != link_binding.market_root_account_id.bytes()
        || binding_id != link_binding.market_binding_id
        || binding.market_instance_id != link_binding.market_instance_id
        || binding.generation != link_binding.generation
        || binding.capability_profile_id != link_binding.capability_profile_id
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok(root)
}

fn validate_current_product_join(
    accounts: &[StructuredChainAccountV1<'_>],
    action: StructuredClaimActionV1,
    root: StructuredMarketRootV1,
) -> Result<()> {
    let (link_index, bundle_index, attachment_index, product_root_index) = match action {
        StructuredClaimActionV1::CompactDonation => (27, None, None, None),
        StructuredClaimActionV1::RetireDescriptor => (24, Some(25), Some(26), Some(33)),
        _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
    };
    let link = SeriesMarketLinkAccountV3::decode(accounts[link_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let binding = link.state.binding();
    let product_root = product_root_index
        .map(|index| validate_structured_product_root_v3(accounts[13].address, accounts[index], &binding))
        .transpose()?;
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
        || link.state.phase() != SeriesMarketLinkPhaseV3::Active
        || link.state.obligation_status(SeriesLinkObligationV3::Structured)
            != SeriesLinkObligationStatusV3::Live
        || link.state.obligation_status(SeriesLinkObligationV3::Wrapper)
            != SeriesLinkObligationStatusV3::Live
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
        let bundle = CompiledProductSeriesBundleV7::decode(accounts[bundle_index].data()?)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let attachment = SeriesAttachmentPlanV6::decode(accounts[attachment_index].data()?)
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
            ArtifactKind::CompiledProductSeriesBundleV7,
            bundle_id.bytes(),
        )?;
        verify_product_artifact(
            accounts[13].address,
            accounts[attachment_index],
            ArtifactKind::SeriesAttachmentPlanV6,
            attachment_id.bytes(),
        )?;
        let invalid_product_root = match product_root.as_ref() {
            Some(product_root) => {
                product_root.state.binding_ref().registry_release_id
                    != bundle.registry_release_id
                    || product_root
                        .state
                        .product_families()
                        .family(MarketFamilyV1::Structured)
                        .status()
                        != MarketFamilyStatusV1::Live
            }
            None => true,
        };
        if bundle_id != binding.compiler_bundle_id
            || attachment_id != binding.attachment_plan_id
            || bundle.series_plan_id != binding.series_plan_id
            || bundle.funding_terms_id != binding.funding_terms_id
            || bundle.funding_quote_id != binding.funding_quote_id
            || bundle.attachment_plan_id != binding.attachment_plan_id
            || bundle.capability_profile_id.content_id() != binding.capability_profile_id
            || attachment.funding_quote_id != binding.funding_quote_id
            || attachment.wrapper_recipe_set_id != root.binding.wrapper_recipe_set_id
            || invalid_product_root
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

/// Named finalized account frame for action `79/1/2`. The names are only
/// acquisition handles: the constructor hostile-decodes every semantic owner,
/// derives every PDA, and installs the frozen order and privilege bitmap.
#[derive(Clone, Copy, Debug)]
pub struct FractionalRedeemInternalExactFrameV1<'account> {
    pub realm: &'account ObservedRpcAccount,
    pub profile: &'account ObservedRpcAccount,
    pub collateral_policy: &'account ObservedRpcAccount,
    pub collateral_token_program: &'account ObservedRpcAccount,
    pub market_binding: &'account ObservedRpcAccount,
    pub market_runtime: &'account ObservedRpcAccount,
    pub market_instance: &'account ObservedRpcAccount,
    pub hoard: &'account ObservedRpcAccount,
    pub claim_ledger: &'account ObservedRpcAccount,
    pub resolution: &'account ObservedRpcAccount,
    pub fractional_policy: &'account ObservedRpcAccount,
    pub fractional_ledger: &'account ObservedRpcAccount,
    pub position: &'account ObservedRpcAccount,
    pub replay: &'account ObservedRpcAccount,
}

impl<'account> FractionalRedeemInternalExactFrameV1<'account> {
    fn ordered(self) -> [&'account ObservedRpcAccount; 14] {
        [
            self.realm,
            self.profile,
            self.collateral_policy,
            self.collateral_token_program,
            self.market_binding,
            self.market_runtime,
            self.market_instance,
            self.hoard,
            self.claim_ledger,
            self.resolution,
            self.fractional_policy,
            self.fractional_ledger,
            self.position,
            self.replay,
        ]
    }
}

/// Finalized semantic-owner core shared by holder-directed Fractional actions.
/// Programs and dynamic token accounts are supplied separately so the
/// constructor can preserve only the two frozen loader aliases.
#[derive(Clone, Copy, Debug)]
pub struct FractionalHolderCoreFrameV1<'account> {
    pub realm: &'account ObservedRpcAccount,
    pub profile: &'account ObservedRpcAccount,
    pub collateral_policy: &'account ObservedRpcAccount,
    pub collateral_token_program: &'account ObservedRpcAccount,
    pub market_binding: &'account ObservedRpcAccount,
    pub market_runtime: &'account ObservedRpcAccount,
    pub market_instance: &'account ObservedRpcAccount,
    pub hoard: &'account ObservedRpcAccount,
    pub claim_ledger: &'account ObservedRpcAccount,
    pub resolution: &'account ObservedRpcAccount,
    pub fractional_policy: &'account ObservedRpcAccount,
    pub fractional_ledger: &'account ObservedRpcAccount,
}

impl<'account> FractionalHolderCoreFrameV1<'account> {
    fn ordered(self) -> [&'account ObservedRpcAccount; 12] {
        [
            self.realm,
            self.profile,
            self.collateral_policy,
            self.collateral_token_program,
            self.market_binding,
            self.market_runtime,
            self.market_instance,
            self.hoard,
            self.claim_ledger,
            self.resolution,
            self.fractional_policy,
            self.fractional_ledger,
        ]
    }
}

/// Dynamic bearer account frame for actions 3 and 5. Outcome mints are in
/// canonical outcome order and hostile-decoded against the persisted claim
/// ledger; no caller account metas survive construction.
#[derive(Clone, Copy, Debug)]
pub struct FractionalBearerFrameV1<'account> {
    pub core: FractionalHolderCoreFrameV1<'account>,
    pub collateral_mint: &'account ObservedRpcAccount,
    pub collateral_destination: &'account ObservedRpcAccount,
    pub hoard_authority: &'account ObservedRpcAccount,
    pub hoard_token: &'account ObservedRpcAccount,
    pub outcome_token_program: &'account ObservedRpcAccount,
    pub outcome_token_programdata: &'account ObservedRpcAccount,
    pub bearer_source: &'account ObservedRpcAccount,
    pub collateral_token_programdata: &'account ObservedRpcAccount,
    pub outcome_mints: &'account [&'account ObservedRpcAccount],
}

/// Optional credit suffix for action 5. `credit` may be a finalized absence,
/// a live credit, or a permanent tombstone. Payer/System accounts are present
/// exactly for fresh/reopen modes.
#[derive(Clone, Copy, Debug)]
pub struct FractionalCreditAdmissionFrameV1<'account> {
    pub credit: StructuredChainAccountV1<'account>,
    pub market_root: &'account ObservedRpcAccount,
    pub neutral_sink: &'account ObservedRpcAccount,
    pub rent_sysvar: &'account ObservedRpcAccount,
    pub funding_payer: Option<&'account ObservedRpcAccount>,
    pub system_program: Option<&'account ObservedRpcAccount>,
}

/// Finalized internal Position/Replay plus owner-credit suffix for action 4.
#[derive(Clone, Copy, Debug)]
pub struct FractionalInternalCreditFrameV1<'account> {
    pub core: FractionalHolderCoreFrameV1<'account>,
    pub position: &'account ObservedRpcAccount,
    pub replay: &'account ObservedRpcAccount,
    pub credit: FractionalCreditAdmissionFrameV1<'account>,
}

/// Finalized payout-owner accounts selected by the destination holder for a
/// credit transfer/merge. External payout adds exactly the collateral loader
/// ProgramData role; internal payout instead adds one Position/Replay pair.
#[derive(Clone, Copy, Debug)]
pub enum FractionalCreditPayoutFrameV1<'account> {
    Internal {
        position: &'account ObservedRpcAccount,
        replay: &'account ObservedRpcAccount,
    },
    External {
        collateral_mint: &'account ObservedRpcAccount,
        destination: &'account ObservedRpcAccount,
        hoard_authority: &'account ObservedRpcAccount,
        hoard_token: &'account ObservedRpcAccount,
        collateral_token_programdata: &'account ObservedRpcAccount,
    },
}

/// Complete finalized acquisition frame for actions 6 and 7.
#[derive(Clone, Copy, Debug)]
pub struct FractionalCreditMoveFrameV1<'account> {
    pub core: FractionalHolderCoreFrameV1<'account>,
    pub source_credit: &'account ObservedRpcAccount,
    pub destination_credit: StructuredChainAccountV1<'account>,
    pub market_root: &'account ObservedRpcAccount,
    pub neutral_sink: &'account ObservedRpcAccount,
    pub rent_sysvar: &'account ObservedRpcAccount,
    pub funding_payer: Option<&'account ObservedRpcAccount>,
    pub system_program: Option<&'account ObservedRpcAccount>,
    pub payout: FractionalCreditPayoutFrameV1<'account>,
}

/// Additional finalized accounts needed to close one zero-numerator owner
/// credit. The claimant, sequences, payer, and neutral sink are decoded from
/// these owners rather than supplied independently.
#[derive(Clone, Copy, Debug)]
pub struct FractionalCloseZeroCreditFrameV1<'account> {
    pub base: FractionalRedeemInternalExactFrameV1<'account>,
    pub credit: &'account ObservedRpcAccount,
    pub payer: &'account ObservedRpcAccount,
    pub market_root: &'account ObservedRpcAccount,
    pub neutral_sink: &'account ObservedRpcAccount,
    pub rent_sysvar: &'account ObservedRpcAccount,
}

/// Holder-selected payout route for an owner-authorized Fractional action.
/// The address is a choice to be wallet-reviewed, never a claimed semantic
/// identity; the constructor must still authenticate the corresponding
/// Position or collateral token account from finalized bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalHolderPayoutV1 {
    InternalPosition { position: Address },
    ExternalCollateral { token_account: Address },
}

/// The irreducible holder choices for actions 3 through 7. Persisted IDs,
/// account metas, sequences, credit modes, PDAs, and poststates are
/// intentionally absent and remain chain-derived constructor output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalHolderIntentV1 {
    RedeemBearerExact {
        claimant: Address,
        bearer_source: Address,
        outcome: u8,
        quantity: u64,
        collateral_destination: Address,
    },
    RedeemInternalCredit {
        claimant: Address,
        position: Address,
        outcome: u8,
        quantity: u64,
        funding_payer: Option<Address>,
    },
    RedeemBearerCredit {
        claimant: Address,
        bearer_source: Address,
        outcome: u8,
        quantity: u64,
        collateral_destination: Address,
        funding_payer: Option<Address>,
    },
    TransferCredit {
        source_claimant: Address,
        source_credit: Address,
        destination_claimant: Address,
        numerator: u64,
        payout: FractionalHolderPayoutV1,
        funding_payer: Option<Address>,
    },
    MergeCredit {
        source_claimant: Address,
        source_credit: Address,
        destination_claimant: Address,
        payout: FractionalHolderPayoutV1,
        funding_payer: Option<Address>,
    },
}

impl FractionalHolderIntentV1 {
    /// Refuse zero/aliased choices before any account acquisition. This does
    /// not authenticate an address; finalized semantic-owner decoding does.
    pub fn validate(self) -> Result<()> {
        let live = |address: Address| address != Address::default();
        let valid_payer = |payer: Option<Address>| payer.is_none_or(live);
        let valid_payout = |payout: FractionalHolderPayoutV1| match payout {
            FractionalHolderPayoutV1::InternalPosition { position } => live(position),
            FractionalHolderPayoutV1::ExternalCollateral { token_account } => {
                live(token_account)
            }
        };
        let valid = match self {
            Self::RedeemBearerExact {
                claimant,
                bearer_source,
                quantity,
                collateral_destination,
                ..
            } => {
                live(claimant)
                    && live(bearer_source)
                    && live(collateral_destination)
                    && quantity != 0
            }
            Self::RedeemInternalCredit {
                claimant,
                position,
                quantity,
                funding_payer,
                ..
            } => live(claimant) && live(position) && quantity != 0 && valid_payer(funding_payer),
            Self::RedeemBearerCredit {
                claimant,
                bearer_source,
                quantity,
                collateral_destination,
                funding_payer,
                ..
            } => {
                live(claimant)
                    && live(bearer_source)
                    && live(collateral_destination)
                    && quantity != 0
                    && valid_payer(funding_payer)
            }
            Self::TransferCredit {
                source_claimant,
                source_credit,
                destination_claimant,
                numerator,
                payout,
                funding_payer,
            } => {
                live(source_claimant)
                    && live(source_credit)
                    && live(destination_claimant)
                    && source_claimant != destination_claimant
                    && numerator != 0
                    && valid_payout(payout)
                    && valid_payer(funding_payer)
            }
            Self::MergeCredit {
                source_claimant,
                source_credit,
                destination_claimant,
                payout,
                funding_payer,
            } => {
                live(source_claimant)
                    && live(source_credit)
                    && live(destination_claimant)
                    && source_claimant != destination_claimant
                    && valid_payout(payout)
                    && valid_payer(funding_payer)
            }
        };
        if valid {
            Ok(())
        } else {
            Err(CanonicalActionMaterialErrorV1::InvalidPlan)
        }
    }
}

/// Complete finalized Product-foundation frame for Fractional actions 1/10.
/// Accounts are retained in the SBF's canonical dynamic order; the constructor
/// derives the active width from the hostile-decoded Product root and rejects
/// every caller-shaped geometry, privilege, sequence, and payload choice.
#[derive(Clone, Copy, Debug)]
pub struct FractionalLifecycleFrameV1<'account> {
    pub accounts: &'account [&'account ObservedRpcAccount],
}

/// Construct permissionless action 1 or 10 material from a complete finalized
/// lifecycle frame and three independently authenticated release rows.
pub fn construct_fractional_lifecycle_material_v1(
    releases: FractionalExternalReleaseSetV1<'_>,
    builder: &ProtocolTransactionBuilder,
    workflow_id: [u8; 32],
    freshness: ActionFreshnessBoundaryV1,
    frame: FractionalLifecycleFrameV1<'_>,
) -> Result<CanonicalActionMaterialV1> {
    releases.base.validate().map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    freshness.validate()?;
    if workflow_id == [0; 32]
        || builder.clutch_program() != releases.base.program_id
        || builder.clutch_release_sha256() != releases.base.elf_sha256
        || frame.accounts.len() < 34
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    let program = releases.base.program_id;
    let root = MarketLifecycleRootAccountV3::decode(&frame.accounts[0].data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let binding = root.state.binding();
    let outcomes = usize::from(binding.outcome_count);
    if !(2..=MARKET_FOUNDATION_MAX_OUTCOMES_V4).contains(&outcomes)
        || frame.accounts.len() != 32 + outcomes
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let aux = MARKET_FOUNDATION_CORE_SLOT_COUNT_V4 + outcomes;
    let cluster = &frame.accounts[0].provenance.cluster_key;
    let release_key = releases.base.key();
    let mut addresses = BTreeSet::new();
    for (index, account) in frame.accounts.iter().copied().enumerate() {
        if account.provenance.commitment != RpcCommitment::Finalized
            || account.provenance.slot == 0
            || account.provenance.slot > freshness.observed_slot
            || account.provenance.cluster_key != *cluster
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        let loader_alias = (index == aux + 5
            && account.address == frame.accounts[aux + 3].address)
            || (index == aux + 6
                && account.address == frame.accounts[aux + 4].address);
        if !addresses.insert(account.address) && !loader_alias {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    if addresses.len() > 64
        || frame.accounts[aux + 3].address == frame.accounts[aux + 5].address
        != (frame.accounts[aux + 4].address == frame.accounts[aux + 6].address)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let root_pda = Address::find_program_address(
        &[b"dc:market-lifecycle-root:v1", &binding.market_instance_id.bytes(),
          &binding.generation.to_le_bytes()],
        &program,
    );
    if frame.accounts[0].owner != program
        || frame.accounts[0].executable
        || frame.accounts[0].address != root_pda.0
        || root.stored_bump != root_pda.1
        || frame.accounts[0].lamports < root.rent_principal_lamports
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let link = SeriesMarketLinkAccountV3::decode(&frame.accounts[aux + 8].data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let link_binding = link.state.binding();
    let quote = SeriesFundingQuoteV6::decode(&frame.accounts[aux + 9].data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let quote_id = quote.id().map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    verify_product_artifact(
        program,
        StructuredChainAccountV1::present(frame.accounts[aux + 9])?,
        ArtifactKind::SeriesFundingQuoteV6,
        quote_id.content_id().bytes(),
    )?;
    let market_binding = MarketBindingV5::decode(&frame.accounts[1].data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let mut graph_ids = [ContentId::ZERO; MARKET_FOUNDATION_SLOT_COUNT_V4];
    for index in 0..MARKET_FOUNDATION_CORE_SLOT_COUNT_V4 {
        graph_ids[index] = ContentId::from_bytes(frame.accounts[index].address.to_bytes());
    }
    for index in 0..outcomes {
        graph_ids[MARKET_FOUNDATION_CORE_SLOT_COUNT_V4 + index] =
            ContentId::from_bytes(frame.accounts[MARKET_FOUNDATION_CORE_SLOT_COUNT_V4 + index].address.to_bytes());
        let outcome_index = u8::try_from(index)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let generation = binding.generation.to_le_bytes();
        let custody = Address::find_program_address(
            &[
                OUTCOME_CUSTODY_PDA_DOMAIN_V1,
                &binding.market_instance_id.bytes(),
                &generation,
                &[outcome_index],
            ],
            &program,
        )
        .0;
        graph_ids[MARKET_FOUNDATION_CORE_SLOT_COUNT_V4 + MARKET_FOUNDATION_MAX_OUTCOMES_V4 + index] =
            ContentId::from_bytes(custody.to_bytes());
    }
    let revenue = market_binding.authority();
    let purpose = [u8::from(PositionPurposeV3::General)];
    let treasury_position = Address::find_program_address(
        &[
            POSITION_V3_PDA_PREFIX,
            &binding.market_instance_id.bytes(),
            &revenue.treasury_owner().bytes(),
            &purpose,
            &frame.accounts[2].address.to_bytes(),
        ],
        &program,
    )
    .0;
    let treasury_replay = Address::find_program_address(
        &[
            PURPOSE_REPLAY_V3_PDA_PREFIX,
            &treasury_position.to_bytes(),
            &purpose,
            &frame.accounts[2].address.to_bytes(),
        ],
        &program,
    )
    .0;
    let treasury_service = Address::find_program_address(
        &[
            TREASURY_SERVICE_LEDGER_PDA_DOMAIN_V1,
            &binding.market_instance_id.bytes(),
            &treasury_position.to_bytes(),
        ],
        &program,
    )
    .0;
    if revenue.treasury_position_account().bytes() != treasury_position.to_bytes()
        || revenue.treasury_service_ledger_account().bytes() != treasury_service.to_bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    graph_ids[MarketFoundationSlotV4::GeneralTreasuryPosition.index()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?] =
        ContentId::from_bytes(treasury_position.to_bytes());
    graph_ids[MarketFoundationSlotV4::GeneralTreasuryReplay.index()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?] =
        ContentId::from_bytes(treasury_replay.to_bytes());
    graph_ids[MarketFoundationSlotV4::TreasuryServiceLedger.index()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?] =
        ContentId::from_bytes(treasury_service.to_bytes());
    let graph = MarketFoundationAccountGraphV4 {
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        foundation_schedule_id: quote.foundation.id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?,
        account_ids: graph_ids,
    };
    let graph_id = graph.id(&quote.foundation)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let link_id = link.state.semantic_id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if quote.foundation.outcome_count != binding.outcome_count
        || graph_id != binding.foundation_account_graph_id
        || link_id != root.state.capital().founder_link_id.content_id()
        || link_binding.market_instance_id != binding.market_instance_id
        || link_binding.generation != binding.generation
        || link_binding.funding_quote_id != quote_id
        || link_binding.rent_refund_owner != root.state.capital().rent_refund_owner
        || link_binding.neutral_lamport_sink != root.state.capital().neutral_lamport_sink
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let registry = SeriesRegistryAccountV4::decode(&frame.accounts[aux + 10].data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let registry_pda = Address::find_program_address(
        &[SERIES_REGISTRY_PDA_PREFIX_V1, &registry.series_plan_id.bytes()], &program,
    );
    let base_release_id = decode_release_artifact(
        releases.base,
        program,
        StructuredChainAccountV1::present(frame.accounts[aux + 11])?,
        StructuredChainAccountV1::present(frame.accounts[aux + 12])?,
        StructuredChainAccountV1::present(frame.accounts[aux + 13])?,
        releases.base.release_manifest_sha256,
    )?;
    let capability = RegistryCapabilityProfileV4::decode(&frame.accounts[aux + 14].data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let capability_id = capability.id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    verify_product_artifact(
        program,
        StructuredChainAccountV1::present(frame.accounts[aux + 14])?,
        ArtifactKind::RegistryCapabilityProfileV4,
        capability_id.content_id().bytes(),
    )?;
    if frame.accounts[aux + 10].owner != program
        || frame.accounts[aux + 10].address != registry_pda.0
        || registry.stored_bump != registry_pda.1
        || !registry.activation_consumed
        || registry.series_plan_id != link_binding.series_plan_id
        || registry.funding_terms_id != link_binding.funding_terms_id
        || registry.compiler_bundle_id != link_binding.compiler_bundle_id
        || registry.registry_release_id != base_release_id
        || registry.capability_profile_id != capability_id.content_id()
        || binding.registry_release_id != base_release_id
        || binding.capability_profile_id != capability_id.content_id()
        || capability_id.bytes() != releases.base.capability_profile_id
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    authenticate_indexed_loader_release(
        releases.collateral.program(),
        releases.collateral.artifact(),
        StructuredChainAccountV1::present(frame.accounts[aux + 3])?,
        StructuredChainAccountV1::present(frame.accounts[aux + 4])?,
    )?;
    authenticate_indexed_loader_release(
        releases.claim.program,
        releases.claim.artifact,
        StructuredChainAccountV1::present(frame.accounts[aux + 5])?,
        StructuredChainAccountV1::present(frame.accounts[aux + 6])?,
    )?;

    let realm = RealmAccount::decode(&frame.accounts[aux].data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let profile = ProfileAccount::decode(&frame.accounts[aux + 1].data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let collateral_policy = CollateralPolicyV2::decode(&frame.accounts[aux + 2].data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_instance = MarketInstancePreimageV2::decode(&frame.accounts[aux + 7].data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_id = market_instance.id().map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_runtime = MarketRuntimeV3AccountV1::decode(&frame.accounts[2].data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let hoard = HoardV2::decode(&frame.accounts[3].data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let claim = ClaimLedgerV3::decode(&frame.accounts[4].data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let resolution = ResolutionV5::decode(&frame.accounts[10].data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let collateral_id = collateral_policy.id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let collateral_release_id = releases.collateral.adapter().id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    let claim_binding_id = releases.claim.binding.id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    let realm_pda = Address::find_program_address(
        &[REALM_PDA_SEED_V1, &realm.realm.bytes()], &program,
    );
    let profile_pda = Address::find_program_address(
        &[PROFILE_PDA_SEED_V1, &realm.realm.bytes(), &profile.profile.bytes()], &program,
    );
    let collateral_policy_pda = Address::find_program_address(
        &[COLLATERAL_POLICY_PDA_SEED_V1, &profile.profile.bytes(), &profile.collateral_policy_id.bytes()],
        &program,
    );
    let market_binding_pda = Address::find_program_address(
        &[MARKET_BINDING_SEED_DOMAIN_V1, &market_id.bytes()], &program,
    );
    let market_runtime_pda = Address::find_program_address(
        &[MARKET_RUNTIME_SEED_DOMAIN_V1, &frame.accounts[1].address.to_bytes()], &program,
    );
    let market_kind = [ArtifactKind::MarketInstancePreimageV2.byte()];
    let market_artifact_pda = Address::find_program_address(
        &[b"dc:product-artifact:v1", &market_kind, &market_id.bytes()], &program,
    );
    let hoard_pda = Address::find_program_address(
        &[HOARD_V2_PDA_SEED_V1, &market_id.bytes()], &program,
    );
    let claim_pda = Address::find_program_address(
        &[CLAIM_LEDGER_V3_PDA_SEED_V1, &market_id.bytes()], &program,
    );
    let resolution_pda = Address::find_program_address(
        &[b"dc:resolution:v5", &market_id.bytes()], &program,
    );
    if market_id != binding.market_instance_id
        || frame.accounts[aux].address != realm_pda.0
        || realm.stored_bump != realm_pda.1
        || frame.accounts[aux + 1].address != profile_pda.0
        || frame.accounts[aux + 2].address != collateral_policy_pda.0
        || frame.accounts[1].address != market_binding_pda.0
        || frame.accounts[2].address != market_runtime_pda.0
        || frame.accounts[aux + 7].address != market_artifact_pda.0
        || frame.accounts[3].address != hoard_pda.0
        || hoard.stored_bump != hoard_pda.1
        || frame.accounts[4].address != claim_pda.0
        || claim.stored_bump != claim_pda.1
        || frame.accounts[10].address != resolution_pda.0
        || resolution.stored_bump != resolution_pda.1
        || profile.realm != realm.realm
        || profile.profile != realm.profile
        || profile.collateral_policy_id != collateral_id
        || profile.adapter_release_id.bytes() != collateral_release_id.bytes()
        || binding.realm_id.bytes() != realm.realm.bytes()
        || binding.collateral_policy_id.bytes() != collateral_id.bytes()
        || binding.collateral_release_id.bytes() != collateral_release_id.bytes()
        || binding.claim_issuance_binding_id.bytes() != claim_binding_id.bytes()
        || market_binding.base().base().market_instance_v2_id.bytes() != market_id.bytes()
        || market_runtime.market_binding.bytes() != frame.accounts[1].address.to_bytes()
        || hoard.market_instance_id.bytes() != market_id.bytes()
        || claim.market_instance_id.bytes() != market_id.bytes()
        || resolution.facts.market_instance_id.bytes() != market_id.bytes()
        || resolution.facts.generation != binding.generation
        || resolution.facts.outcome_count != binding.outcome_count
        || resolution.state != ResolutionStateV5::Finalized
        || hoard.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || claim.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || claim.native_claim_basis_id.bytes() != binding.native_claim_basis_id.bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    for index in [0usize, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 13] {
        if frame.accounts[index].owner != program || frame.accounts[index].executable {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    for index in [aux, aux + 1, aux + 2, aux + 7, aux + 8, aux + 9, aux + 10, aux + 13, aux + 14] {
        if frame.accounts[index].owner != program || frame.accounts[index].executable {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    if frame.accounts[14].owner != frame.accounts[aux + 3].address
        || frame.accounts[14].executable
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    for index in 0..outcomes {
        let mint = frame.accounts[MARKET_FOUNDATION_CORE_SLOT_COUNT_V4 + index];
        if mint.owner != frame.accounts[aux + 5].address
            || mint.executable
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        let decoded = decode_canonical_wrapper_mint_v1(
            frame.accounts[aux + 5].address.to_bytes(),
            mint.address.to_bytes(),
            frame.accounts[2].address.to_bytes(),
            &mint.data,
        ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        if decoded.supply != claim.aggregate_materialized_supply[index] {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }

    let payout = PayoutVectorV1::from_resolution_v5(resolution)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let common_lot = payout.common_lot()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let policy_pda = Address::find_program_address(
        &[FRACTIONAL_POLICY_PDA_PREFIX, &market_id.bytes(), &frame.accounts[10].address.to_bytes()],
        &program,
    );
    let ledger_pda = Address::find_program_address(
        &[FRACTIONAL_LEDGER_PDA_PREFIX, &frame.accounts[11].address.to_bytes()], &program,
    );
    if frame.accounts[11].address != policy_pda.0 || frame.accounts[12].address != ledger_pda.0 {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let (action, sequence, payload, driver, driver_slot) = if frame.accounts[11].owner == Address::default()
        && frame.accounts[12].owner == Address::default()
        && frame.accounts[11].data.is_empty()
        && frame.accounts[12].data.is_empty()
    {
        if claim.next_fractional_sequence != 0
            || claim.fractional_policy_id != clutch_collateral_adapter_v2::Id::ZERO
            || claim.fractional_ledger_account != clutch_collateral_adapter_v2::Id::ZERO
            || frame.accounts[aux + 15].address != Address::default()
            || !frame.accounts[aux + 15].executable
            || frame.accounts[11].lamports < quote.foundation.slot_principal_lamports[11]
            || frame.accounts[12].lamports < quote.foundation.slot_principal_lamports[12]
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        let intent = FractionalInitializeIntentV1 {
            domain_generation: binding.generation,
            common_lot,
            policy_bump: policy_pda.1,
            ledger_bump: ledger_pda.1,
        }.encode().map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?.to_vec();
        (FractionalRedemptionActionV1::Initialize, 0, intent, frame.accounts[0].address, frame.accounts[0].provenance.slot)
    } else {
        let policy = FractionalPolicyV3::decode(&frame.accounts[11].data)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let ledger = FractionalLedgerV1::decode(&frame.accounts[12].data)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let all_zero = claim.aggregate_internal_supply.iter()
            .chain(claim.aggregate_materialized_supply.iter()).all(|value| *value == 0);
        if frame.accounts[11].owner != program
            || frame.accounts[12].owner != program
            || policy.stored_bump != policy_pda.1
            || ledger.stored_bump != ledger_pda.1
            || policy.market_instance.bytes() != market_id.bytes()
            || policy.resolution_account.bytes() != frame.accounts[10].address.to_bytes()
            || policy.domain_generation != binding.generation
            || policy.common_lot != common_lot
            || ledger.policy_account.bytes() != frame.accounts[11].address.to_bytes()
            || ledger.claim_ledger_account.bytes() != frame.accounts[4].address.to_bytes()
            || ledger.phase != FractionalLedgerPhaseV1::ClaimsExhausted
            || ledger.active_credit_accounts != 0
            || ledger.aggregate_credit_numerator != 0
            || hoard.locked_claim_principal_atoms != 0
            || !all_zero
            || policy.rent.payer() != ledger.rent.payer()
            || frame.accounts[aux + 15].address.to_bytes() != policy.rent.payer().bytes()
            || frame.accounts[aux + 16].address.to_bytes() != root.state.capital().neutral_lamport_sink.bytes()
            || frame.accounts[aux + 15].owner != Address::default()
            || frame.accounts[aux + 16].owner != Address::default()
            || frame.accounts[aux + 15].executable
            || frame.accounts[aux + 16].executable
            || !frame.accounts[aux + 15].data.is_empty()
            || !frame.accounts[aux + 16].data.is_empty()
            || policy.rent.refundable_principal().checked_add(policy.rent.donation_floor())
                .is_none_or(|floor| frame.accounts[11].lamports < floor)
            || ledger.rent.refundable_principal().checked_add(ledger.rent.donation_floor())
                .is_none_or(|floor| frame.accounts[12].lamports < floor)
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        let payload = FractionalTerminalIntentV1 { expected_ledger_sequence: ledger.next_sequence }
            .encode().map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?.to_vec();
        (FractionalRedemptionActionV1::CloseEmptyLedger, ledger.next_sequence, payload,
         frame.accounts[12].address, frame.accounts[12].provenance.slot)
    };
    let coordinate = CanonicalIntentCoordinate {
        family_tag: FRACTIONAL_REDEMPTION_FAMILY_TAG,
        family_version: FRACTIONAL_REDEMPTION_FAMILY_VERSION,
        local_action: action.tag(),
    };
    if releases.base.enabled_intents.binary_search(&coordinate).is_err() {
        return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
    }
    let contract = clutch_fractional_redemption_runtime::fractional_account_contract_v1(action);
    if !contract.foundation_outcome_mint_suffix || contract.post_mint_accounts != 0 {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let mut metas = Vec::with_capacity(frame.accounts.len());
    let mut roles = Vec::with_capacity(frame.accounts.len());
    for (index, account) in frame.accounts.iter().copied().enumerate() {
        let writable = if index < 15 {
            contract.writable_mask & (1_u32 << index) != 0
        } else if index < aux {
            false
        } else {
            contract.foundation_aux_writable_mask & (1_u32 << (index - aux)) != 0
        };
        metas.push(if writable { AccountMeta::new(account.address, false) }
            else { AccountMeta::new_readonly(account.address, false) });
        roles.push(CanonicalAccountRoleV1 {
            label: if index < 15 {
                "foundation-core"
            } else if index < 15 + outcomes {
                "foundation-outcome-mint"
            } else {
                "lifecycle-authority"
            },
            address: account.address,
            writable,
            signer: false,
        });
    }
    let equations = vec![ExactEquation {
        name: "chain-derived Product foundation outcome width".into(),
        unit: IntegerUnit::EggAtoms { market: market_id.bytes(), outcome: 0 },
        left: u128::from(binding.outcome_count),
        right: u128::from(binding.outcome_count),
    }];
    let draft = crate::transaction_builder::OwnedInstructionDraft::enabled_fractional_lifecycle_v1(
        crate::transaction_builder::SemanticOwner {
            package: "clutch-fractional-redemption-runtime".into(),
            schema: match action {
                FractionalRedemptionActionV1::Initialize => "fractional-redemption/79/1/1/initialize",
                _ => "fractional-redemption/79/1/10/close-empty-ledger",
            }.into(),
            release_sha256: releases.base.elf_sha256,
        },
        program, metas, equations, action, sequence, &payload,
    ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let unsigned = builder.build_atomic(&[draft])
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let authority = fractional_authority_state_id_v1(frame.accounts, releases.base);
    let cursor = ResumableWorkflowCursor {
        workflow_id,
        lane: WorkflowLane::FractionalRedemption,
        generation: binding.generation,
        position: WorkflowPosition { phase: u16::from(action.tag()), item: sequence },
        observed_state_sha256: authority,
    };
    let planned = PlannedWorkflowNode {
        manifest_sha256: releases.base.release_manifest_sha256,
        cursor,
        coordinate: CanonicalActionCoordinate::FractionalRedemption(action),
        unsigned_transaction: unsigned,
        reload_authoritative_accounts: true,
    };
    validate_unsigned_fractional_plan(coordinate, builder.payer(), &roles, &planned)?;
    let draft_id = action_material_id(
        &release_key, &release_key, releases.base.release_manifest_sha256,
        releases.base.capability_profile_id, coordinate, driver, driver_slot,
        cursor, authority, freshness, builder.payer(), &roles, &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key: release_key.clone(), driver_release_key: release_key,
        release_manifest_sha256: releases.base.release_manifest_sha256,
        capability_profile_id: releases.base.capability_profile_id, coordinate, variant: None,
        driver_account: driver, driver_account_slot: driver_slot, cursor,
        authority_state_sha256: authority, freshness, fee_payer: builder.payer(),
        account_roles: roles, planned, draft_id,
    })
}

#[derive(Clone, Copy, Debug)]
struct AuthenticatedFractionalHolderCoreV1 {
    realm: RealmAccount,
    profile: ProfileAccount,
    collateral_policy: CollateralPolicyV2,
    market_instance_id: ContentId,
    hoard: HoardV2,
    claim_ledger: ClaimLedgerV3,
    resolution: ResolutionV5,
    policy: FractionalPolicyV3,
    ledger: FractionalLedgerV1,
}

fn authenticate_fractional_holder_core_v1(
    release: &IndexedProgramRelease,
    collateral: StructuredCollateralCatalogEntryV1<'_>,
    freshness: ActionFreshnessBoundaryV1,
    frame: FractionalHolderCoreFrameV1<'_>,
    collateral_programdata: Option<&ObservedRpcAccount>,
) -> Result<AuthenticatedFractionalHolderCoreV1> {
    let ordered = frame.ordered();
    let release_key = release.key();
    let cluster = &frame.realm.provenance.cluster_key;
    let mut identities = BTreeSet::new();
    for (index, account) in ordered.iter().enumerate() {
        if account.address == Address::default()
            || account.provenance.commitment != RpcCommitment::Finalized
            || account.provenance.slot == 0
            || account.provenance.slot > freshness.observed_slot
            || account.provenance.cluster_key != *cluster
            || !identities.insert(account.address)
            || (index != 3
                && (account.owner != release.program_id
                    || account.executable
                    || account.provenance.release_key != release_key))
            || (index == 3 && !account.executable)
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    for account in collateral_programdata {
        if account.address == Address::default()
            || account.provenance.commitment != RpcCommitment::Finalized
            || account.provenance.slot == 0
            || account.provenance.slot > freshness.observed_slot
            || account.provenance.cluster_key != *cluster
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    if let Some(programdata) = collateral_programdata {
        authenticate_indexed_loader_release(
            collateral.program(),
            collateral.artifact(),
            StructuredChainAccountV1::present(frame.collateral_token_program)?,
            StructuredChainAccountV1::present(programdata)?,
        )?;
    }

    let realm = RealmAccount::decode(&frame.realm.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let profile = ProfileAccount::decode(&frame.profile.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let collateral_policy = CollateralPolicyV2::decode(&frame.collateral_policy.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_binding = MarketBindingV5::decode(&frame.market_binding.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_runtime = MarketRuntimeV3AccountV1::decode(&frame.market_runtime.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_instance = MarketInstancePreimageV2::decode(&frame.market_instance.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_instance_id = market_instance
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let hoard = HoardV2::decode(&frame.hoard.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let claim_ledger = ClaimLedgerV3::decode(&frame.claim_ledger.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let resolution = ResolutionV5::decode(&frame.resolution.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let policy = FractionalPolicyV3::decode(&frame.fractional_policy.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let ledger = FractionalLedgerV1::decode(&frame.fractional_ledger.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;

    let realm_pda = Address::find_program_address(
        &[REALM_PDA_SEED_V1, &realm.realm.bytes()],
        &release.program_id,
    );
    let profile_pda = Address::find_program_address(
        &[PROFILE_PDA_SEED_V1, &realm.realm.bytes(), &profile.profile.bytes()],
        &release.program_id,
    );
    let collateral_policy_pda = Address::find_program_address(
        &[
            COLLATERAL_POLICY_PDA_SEED_V1,
            &profile.profile.bytes(),
            &profile.collateral_policy_id.bytes(),
        ],
        &release.program_id,
    );
    let market_binding_pda = Address::find_program_address(
        &[MARKET_BINDING_SEED_DOMAIN_V1, &market_instance_id.bytes()],
        &release.program_id,
    );
    let market_runtime_pda = Address::find_program_address(
        &[MARKET_RUNTIME_SEED_DOMAIN_V1, &frame.market_binding.address.to_bytes()],
        &release.program_id,
    );
    let artifact_kind = [ArtifactKind::MarketInstancePreimageV2.byte()];
    let market_instance_pda = Address::find_program_address(
        &[b"dc:product-artifact:v1", &artifact_kind, &market_instance_id.bytes()],
        &release.program_id,
    );
    let hoard_pda = Address::find_program_address(
        &[HOARD_V2_PDA_SEED_V1, &market_instance_id.bytes()],
        &release.program_id,
    );
    let claim_ledger_pda = Address::find_program_address(
        &[CLAIM_LEDGER_V3_PDA_SEED_V1, &market_instance_id.bytes()],
        &release.program_id,
    );
    let resolution_pda = Address::find_program_address(
        &[b"dc:resolution:v5", &market_instance_id.bytes()],
        &release.program_id,
    );
    let fractional_policy_pda = Address::find_program_address(
        &[
            FRACTIONAL_POLICY_PDA_PREFIX,
            &market_instance_id.bytes(),
            &frame.resolution.address.to_bytes(),
        ],
        &release.program_id,
    );
    let fractional_ledger_pda = Address::find_program_address(
        &[FRACTIONAL_LEDGER_PDA_PREFIX, &frame.fractional_policy.address.to_bytes()],
        &release.program_id,
    );
    let collateral_release_id = collateral
        .adapter()
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    let resolution_data_id = resolution
        .data_id(clutch_collateral_adapter_v2::Id::from_bytes(
            frame.resolution.address.to_bytes(),
        ))
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if frame.realm.address != realm_pda.0
        || realm.stored_bump != realm_pda.1
        || frame.profile.address != profile_pda.0
        || frame.collateral_policy.address != collateral_policy_pda.0
        || profile.realm != realm.realm
        || profile.profile != realm.profile
        || profile.collateral_policy_id.bytes()
            != collateral_policy
                .id()
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
                .bytes()
        || profile.adapter_release_id.bytes() != collateral_release_id.bytes()
        || collateral_policy.adapter_release != collateral_release_id
        || collateral_policy.token_program.bytes()
            != frame.collateral_token_program.address.to_bytes()
        || frame.collateral_token_program.address != collateral.program().program_id
        || collateral_release_id.bytes() != policy.collateral_release.bytes()
        || frame.market_binding.address != market_binding_pda.0
        || frame.market_runtime.address != market_runtime_pda.0
        || frame.market_instance.address != market_instance_pda.0
        || market_binding.base().base().market.bytes() != frame.market_runtime.address.to_bytes()
        || market_binding.base().base().market_instance_v2_id.bytes() != market_instance_id.bytes()
        || market_binding.base().base().market_genesis_profile_v2_id.bytes()
            != market_instance.market_genesis_profile_id.bytes()
        || market_runtime.market_binding.bytes() != frame.market_binding.address.to_bytes()
        || market_runtime.market_instance_v2_id.bytes() != market_instance_id.bytes()
        || frame.hoard.address != hoard_pda.0
        || hoard.stored_bump != hoard_pda.1
        || frame.claim_ledger.address != claim_ledger_pda.0
        || claim_ledger.stored_bump != claim_ledger_pda.1
        || frame.resolution.address != resolution_pda.0
        || resolution.stored_bump != resolution_pda.1
        || frame.fractional_policy.address != fractional_policy_pda.0
        || policy.stored_bump != fractional_policy_pda.1
        || frame.fractional_ledger.address != fractional_ledger_pda.0
        || ledger.stored_bump != fractional_ledger_pda.1
        || hoard.market_instance_id.bytes() != market_instance_id.bytes()
        || claim_ledger.market_instance_id.bytes() != market_instance_id.bytes()
        || resolution.facts.market_instance_id.bytes() != market_instance_id.bytes()
        || policy.market_instance.bytes() != market_instance_id.bytes()
        || hoard.realm_id != realm.realm
        || claim_ledger.realm_id != realm.realm
        || policy.realm.bytes() != realm.realm.bytes()
        || policy.collateral_policy.bytes() != profile.collateral_policy_id.bytes()
        || policy.resolution_account.bytes() != frame.resolution.address.to_bytes()
        || policy.resolution_data_id.bytes() != resolution_data_id.bytes()
        || policy.domain_generation != resolution.facts.generation
        || policy.outcome_count != resolution.facts.outcome_count
        || policy.outcome_count != claim_ledger.outcome_count
        || policy.outcome_count != hoard.outcome_count
        || policy.common_lot == 0
        || hoard.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || claim_ledger.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || resolution.state != ResolutionStateV5::Finalized
        || ledger.phase != FractionalLedgerPhaseV1::Live
        || ledger.policy_account.bytes() != frame.fractional_policy.address.to_bytes()
        || ledger.claim_ledger_account.bytes() != frame.claim_ledger.address.to_bytes()
        || ledger.domain_generation != policy.domain_generation
        || claim_ledger.fractional_policy_id.bytes() != frame.fractional_policy.address.to_bytes()
        || claim_ledger.fractional_ledger_account.bytes()
            != frame.fractional_ledger.address.to_bytes()
        || claim_ledger.resolution_account.bytes() != frame.resolution.address.to_bytes()
        || claim_ledger.next_fractional_sequence != ledger.next_sequence
        || resolution.facts.native_claim_basis_id.bytes()
            != claim_ledger.native_claim_basis_id.bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok(AuthenticatedFractionalHolderCoreV1 {
        realm,
        profile,
        collateral_policy,
        market_instance_id,
        hoard,
        claim_ledger,
        resolution,
        policy,
        ledger,
    })
}

fn validate_fractional_bearer_frame_v1(
    releases: FractionalExternalReleaseSetV1<'_>,
    freshness: ActionFreshnessBoundaryV1,
    frame: FractionalBearerFrameV1<'_>,
    intent: FractionalHolderIntentV1,
) -> Result<(AuthenticatedFractionalHolderCoreV1, u128)> {
    let (claimant, source, outcome, quantity, destination) = match intent {
        FractionalHolderIntentV1::RedeemBearerExact {
            claimant,
            bearer_source,
            outcome,
            quantity,
            collateral_destination,
        }
        | FractionalHolderIntentV1::RedeemBearerCredit {
            claimant,
            bearer_source,
            outcome,
            quantity,
            collateral_destination,
            ..
        } => (
            claimant,
            bearer_source,
            outcome,
            quantity,
            collateral_destination,
        ),
        _ => return Err(CanonicalActionMaterialErrorV1::WrongSelection),
    };
    let core = authenticate_fractional_holder_core_v1(
        releases.base,
        releases.collateral,
        freshness,
        frame.core,
        Some(frame.collateral_token_programdata),
    )?;
    let claim_binding_id = releases
        .claim
        .binding
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    authenticate_indexed_loader_release(
        releases.claim.program,
        releases.claim.artifact,
        StructuredChainAccountV1::present(frame.outcome_token_program)?,
        StructuredChainAccountV1::present(frame.outcome_token_programdata)?,
    )?;
    if outcome >= core.policy.outcome_count
        || frame.outcome_mints.len() != usize::from(core.policy.outcome_count)
        || frame.bearer_source.address != source
        || frame.collateral_destination.address != destination
        || frame.collateral_mint.address.to_bytes() != core.collateral_policy.mint.bytes()
        || frame.hoard_authority.address.to_bytes() != core.hoard.authority.bytes()
        || frame.hoard_token.address.to_bytes() != core.hoard.token_account.bytes()
        || frame.outcome_token_program.address != releases.claim.program.program_id
        || frame.outcome_token_programdata.address != releases.claim.program.program_data
        || frame.collateral_token_programdata.address != releases.collateral.program().program_data
        || core.policy.claim_issuance_binding.bytes() != claim_binding_id.bytes()
        || (frame.core.collateral_token_program.address == frame.outcome_token_program.address)
            != (frame.collateral_token_programdata.address
                == frame.outcome_token_programdata.address)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let hoard_authority_pda = Address::find_program_address(
        &[HOARD_AUTHORITY_V2_PDA_SEED_V1, &core.market_instance_id.bytes()],
        &releases.base.program_id,
    );
    let hoard_token_pda = Address::find_program_address(
        &[HOARD_TOKEN_V2_PDA_SEED_V1, &core.market_instance_id.bytes()],
        &releases.base.program_id,
    );
    if frame.hoard_authority.address != hoard_authority_pda.0
        || frame.hoard_token.address != hoard_token_pda.0
        || frame.hoard_authority.executable
        || !frame.hoard_authority.data.is_empty()
        || frame.hoard_token.owner != frame.core.collateral_token_program.address
        || frame.collateral_mint.owner != frame.core.collateral_token_program.address
        || frame.collateral_destination.owner != frame.core.collateral_token_program.address
        || frame.bearer_source.owner != frame.outcome_token_program.address
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let cluster = &frame.core.realm.provenance.cluster_key;
    let fixed = [
        frame.collateral_mint,
        frame.collateral_destination,
        frame.hoard_authority,
        frame.hoard_token,
        frame.outcome_token_program,
        frame.outcome_token_programdata,
        frame.bearer_source,
        frame.collateral_token_programdata,
    ];
    for account in fixed.into_iter().chain(frame.outcome_mints.iter().copied()) {
        if account.address == Address::default()
            || account.provenance.commitment != RpcCommitment::Finalized
            || account.provenance.slot == 0
            || account.provenance.slot > freshness.observed_slot
            || account.provenance.cluster_key != *cluster
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    let mut active_mint_addresses = BTreeSet::new();
    for (index, mint_account) in frame.outcome_mints.iter().copied().enumerate() {
        let outcome_index = u8::try_from(index)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let mint_pda = Address::find_program_address(
            &[
                b"dc:outcome-mint:v2",
                &core.market_instance_id.bytes(),
                &[outcome_index],
            ],
            &releases.base.program_id,
        );
        if mint_account.address != mint_pda.0
            || mint_account.owner != frame.outcome_token_program.address
            || mint_account.executable
            || !active_mint_addresses.insert(mint_account.address)
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        let mint = decode_canonical_wrapper_mint_v1(
            frame.outcome_token_program.address.to_bytes(),
            mint_account.address.to_bytes(),
            frame.core.market_runtime.address.to_bytes(),
            &mint_account.data,
        )
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        if mint.supply != core.claim_ledger.aggregate_materialized_supply[index] {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    let selected_mint = frame.outcome_mints[usize::from(outcome)];
    let source_token = decode_canonical_wrapper_token_v1(
        frame.outcome_token_program.address.to_bytes(),
        selected_mint.address.to_bytes(),
        frame.bearer_source.address.to_bytes(),
        claimant.to_bytes(),
        &frame.bearer_source.data,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let _hoard_token = decode_canonical_wrapper_token_v1(
        frame.core.collateral_token_program.address.to_bytes(),
        frame.collateral_mint.address.to_bytes(),
        frame.hoard_token.address.to_bytes(),
        frame.hoard_authority.address.to_bytes(),
        &frame.hoard_token.data,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if source_token.amount < quantity
        || core.claim_ledger.aggregate_materialized_supply[usize::from(outcome)] < quantity
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let numerator = u128::from(quantity)
        .checked_mul(u128::from(
            core.resolution.facts.payout_weights[usize::from(outcome)],
        ))
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    Ok((core, numerator))
}

fn derive_fractional_credit_admission_v1(
    release: &IndexedProgramRelease,
    freshness: ActionFreshnessBoundaryV1,
    core: AuthenticatedFractionalHolderCoreV1,
    core_frame: FractionalHolderCoreFrameV1<'_>,
    claimant: Address,
    selected_payer: Option<Address>,
    frame: FractionalCreditAdmissionFrameV1<'_>,
) -> Result<(u8, u64, u64)> {
    let credit_pda = Address::find_program_address(
        &[
            FRACTIONAL_CREDIT_PDA_PREFIX,
            &core_frame.fractional_policy.address.to_bytes(),
            &claimant.to_bytes(),
        ],
        &release.program_id,
    );
    let root_pda = Address::find_program_address(
        &[
            b"dc:market-lifecycle-root:v1",
            &core.market_instance_id.bytes(),
            &core.policy.domain_generation.to_le_bytes(),
        ],
        &release.program_id,
    );
    let root = MarketLifecycleRootAccountV3::decode(&frame.market_root.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if frame.credit.address != credit_pda.0
        || frame.market_root.address != root_pda.0
        || frame.market_root.owner != release.program_id
        || frame.market_root.executable
        || root.stored_bump != root_pda.1
        || root.state.binding_ref().market_instance_id.bytes() != core.market_instance_id.bytes()
        || root.state.binding_ref().generation != core.policy.domain_generation
        || root.state.binding_ref().claim_issuance_binding_id.bytes()
            != core.policy.claim_issuance_binding.bytes()
        || root.state.phase() != MarketLifecyclePhaseV3::Active
        || frame.neutral_sink.address.to_bytes()
            != root.state.capital().neutral_lamport_sink.bytes()
        || frame.neutral_sink.owner != solana_sdk_ids::system_program::ID
        || frame.neutral_sink.executable
        || !frame.neutral_sink.data.is_empty()
        || frame.rent_sysvar.address != solana_sdk_ids::sysvar::rent::ID
        || frame.rent_sysvar.executable
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let cluster = &core_frame.realm.provenance.cluster_key;
    for account in [frame.market_root, frame.neutral_sink, frame.rent_sysvar] {
        if account.provenance.commitment != RpcCommitment::Finalized
            || account.provenance.slot == 0
            || account.provenance.slot > freshness.observed_slot
            || account.provenance.cluster_key != *cluster
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    let common_join = |policy_account: [u8; 32],
                       ledger_account: [u8; 32],
                       market: [u8; 32],
                       resolution: [u8; 32],
                       resolution_data: [u8; 32],
                       owner: [u8; 32],
                       generation: u64,
                       bump: u8| {
        policy_account == core_frame.fractional_policy.address.to_bytes()
            && ledger_account == core_frame.fractional_ledger.address.to_bytes()
            && market == core.market_instance_id.bytes()
            && resolution == core_frame.resolution.address.to_bytes()
            && resolution_data == core.policy.resolution_data_id.bytes()
            && owner == claimant.to_bytes()
            && generation == core.policy.domain_generation
            && bump == credit_pda.1
    };
    let (mode, sequence, numerator) = match frame.credit.present {
        None => (2, 1, 0),
        Some(account) => {
            if account.owner != release.program_id
                || account.executable
                || account.provenance.commitment != RpcCommitment::Finalized
                || account.provenance.slot == 0
                || account.provenance.slot > freshness.observed_slot
                || account.provenance.cluster_key != *cluster
            {
                return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
            }
            if let Ok(credit) = FractionalCreditV2::decode(&account.data) {
                if !common_join(
                    credit.policy_account.bytes(),
                    credit.ledger_account.bytes(),
                    credit.market_instance.bytes(),
                    credit.resolution_account.bytes(),
                    credit.resolution_data_id.bytes(),
                    credit.claimant.bytes(),
                    credit.domain_generation,
                    credit.stored_bump,
                ) || credit.numerator >= core.resolution.facts.payout_denominator
                {
                    return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
                }
                (1, credit.next_sequence, credit.numerator)
            } else {
                let tombstone = FractionalCreditTombstoneV2::decode(&account.data)
                    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
                if !common_join(
                    tombstone.policy_account.bytes(),
                    tombstone.ledger_account.bytes(),
                    tombstone.market_instance.bytes(),
                    tombstone.resolution_account.bytes(),
                    tombstone.resolution_data_id.bytes(),
                    tombstone.claimant.bytes(),
                    tombstone.domain_generation,
                    tombstone.stored_bump,
                ) {
                    return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
                }
                (3, tombstone.closed_next_sequence, 0)
            }
        }
    };
    if mode == 1 {
        if selected_payer.is_some() || frame.funding_payer.is_some() || frame.system_program.is_some()
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    } else {
        let payer = frame
            .funding_payer
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let system = frame
            .system_program
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        if selected_payer != Some(payer.address)
            || payer.owner != solana_sdk_ids::system_program::ID
            || payer.executable
            || system.address != solana_sdk_ids::system_program::ID
            || !system.executable
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        for account in [payer, system] {
            if account.provenance.commitment != RpcCommitment::Finalized
                || account.provenance.slot == 0
                || account.provenance.slot > freshness.observed_slot
                || account.provenance.cluster_key != *cluster
            {
                return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
            }
        }
    }
    Ok((mode, sequence, numerator))
}

/// Construct action 3 or 5 from finalized holder choices and hostile-decoded
/// chain state. The holder selects only consent-bearing addresses, outcome and
/// quantity; all semantic IDs, sequences, suffix geometry and privileges are
/// derived here and remain unsigned for wallet review.
#[allow(clippy::too_many_arguments)]
pub fn construct_fractional_bearer_material_v1(
    releases: FractionalExternalReleaseSetV1<'_>,
    builder: &ProtocolTransactionBuilder,
    workflow_id: [u8; 32],
    freshness: ActionFreshnessBoundaryV1,
    intent: FractionalHolderIntentV1,
    frame: FractionalBearerFrameV1<'_>,
    credit: Option<FractionalCreditAdmissionFrameV1<'_>>,
) -> Result<CanonicalActionMaterialV1> {
    use clutch_retirement::Identity32V1;

    freshness.validate()?;
    intent.validate()?;
    let release = releases.base;
    if workflow_id == [0; 32]
        || builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    let (action, claimant, source, outcome, quantity, destination, selected_payer) = match intent {
        FractionalHolderIntentV1::RedeemBearerExact {
            claimant,
            bearer_source,
            outcome,
            quantity,
            collateral_destination,
        } => (
            FractionalRedemptionActionV1::RedeemBearerExact,
            claimant,
            bearer_source,
            outcome,
            quantity,
            collateral_destination,
            None,
        ),
        FractionalHolderIntentV1::RedeemBearerCredit {
            claimant,
            bearer_source,
            outcome,
            quantity,
            collateral_destination,
            funding_payer,
        } => (
            FractionalRedemptionActionV1::RedeemBearerCredit,
            claimant,
            bearer_source,
            outcome,
            quantity,
            collateral_destination,
            funding_payer,
        ),
        _ => return Err(CanonicalActionMaterialErrorV1::WrongSelection),
    };
    let coordinate = CanonicalIntentCoordinate {
        family_tag: FRACTIONAL_REDEMPTION_FAMILY_TAG,
        family_version: FRACTIONAL_REDEMPTION_FAMILY_VERSION,
        local_action: action.tag(),
    };
    if release.enabled_intents.binary_search(&coordinate).is_err() {
        return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
    }
    let (core, claim_numerator) =
        validate_fractional_bearer_frame_v1(releases, freshness, frame, intent)?;
    let (credit_mode, credit_sequence, prior_credit_numerator, credit_frame) = if action
        == FractionalRedemptionActionV1::RedeemBearerCredit
    {
        let value = credit.ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let (mode, sequence, numerator) = derive_fractional_credit_admission_v1(
            release,
            freshness,
            core,
            frame.core,
            claimant,
            selected_payer,
            value,
        )?;
        (mode, sequence, numerator, Some(value))
    } else {
        if credit.is_some() || selected_payer.is_some() {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
        let numerator = u128::from(quantity)
            .checked_mul(u128::from(
                core.resolution.facts.payout_weights[usize::from(outcome)],
            ))
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        if numerator % u128::from(core.resolution.facts.payout_denominator) != 0 {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        (0, 0, 0, None)
    };
    let total_numerator = claim_numerator
        .checked_add(u128::from(prior_credit_numerator))
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let paid_atoms = u64::try_from(
        total_numerator / u128::from(core.resolution.facts.payout_denominator),
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let credit_address = credit_frame
        .map_or(frame.core.fractional_policy.address, |value| value.credit.address);
    let payload = FractionalRedeemIntentV1 {
        expected_ledger_sequence: core.ledger.next_sequence,
        expected_credit_sequence: credit_sequence,
        expected_position_replay_sequence: 0,
        quantity,
        claimant: Identity32V1::new(claimant.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        claim_source: Identity32V1::new(source.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        payout_target: Identity32V1::new(destination.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        credit_or_policy: Identity32V1::new(credit_address.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        outcome,
        credit_mode,
    }
    .encode()
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;

    let mut addresses = vec![
        claimant,
        frame.core.realm.address,
        frame.core.profile.address,
        frame.core.collateral_policy.address,
        frame.core.collateral_token_program.address,
        frame.core.market_binding.address,
        frame.core.market_runtime.address,
        frame.core.market_instance.address,
        frame.core.hoard.address,
        frame.core.claim_ledger.address,
        frame.core.resolution.address,
        frame.core.fractional_policy.address,
        frame.core.fractional_ledger.address,
        frame.collateral_mint.address,
        frame.collateral_destination.address,
        frame.hoard_authority.address,
        frame.hoard_token.address,
        frame.outcome_token_program.address,
        frame.outcome_token_programdata.address,
        frame.bearer_source.address,
        frame.collateral_token_programdata.address,
    ];
    addresses.extend(frame.outcome_mints.iter().map(|account| account.address));
    let credit_index = addresses.len();
    if let Some(value) = credit_frame {
        addresses.extend([
            value.credit.address,
            value.market_root.address,
            value.neutral_sink.address,
            value.rent_sysvar.address,
        ]);
        if credit_mode != 1 {
            addresses.extend([
                value
                    .funding_payer
                    .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?
                    .address,
                value
                    .system_program
                    .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?
                    .address,
            ]);
        }
    }
    let payer_index = credit_index + 4;
    let payer_alias = credit_mode > 1
        && addresses
            .get(payer_index)
            .is_some_and(|payer| *payer == claimant);
    let mut metas = Vec::with_capacity(addresses.len());
    let mut account_roles = Vec::with_capacity(addresses.len());
    for (index, address) in addresses.iter().copied().enumerate() {
        let writable = matches!(index, 8 | 9 | 12 | 14 | 16 | 19)
            || index == 21 + usize::from(outcome)
            || (credit_frame.is_some() && index == credit_index)
            || (credit_mode > 1 && index == payer_index)
            || (index == 0 && payer_alias);
        let signer = index == 0 || (credit_mode > 1 && index == payer_index);
        metas.push(if writable {
            AccountMeta::new(address, signer)
        } else {
            AccountMeta::new_readonly(address, signer)
        });
        let label = match index {
            0 => "claimant",
            1 => "realm",
            2 => "profile",
            3 => "collateral-policy",
            4 => "collateral-token-program",
            5 => "market-binding-v2",
            6 => "market-runtime-v3",
            7 => "market-instance-preimage-v2",
            8 => "hoard-v2",
            9 => "claim-ledger-v3",
            10 => "resolution-v5",
            11 => "fractional-policy-v3",
            12 => "fractional-ledger-v1",
            13 => "collateral-mint",
            14 => "collateral-destination",
            15 => "hoard-authority",
            16 => "hoard-token",
            17 => "outcome-token-program",
            18 => "outcome-token-programdata",
            19 => "bearer-source",
            20 => "collateral-token-programdata",
            value if value < credit_index => "outcome-mint",
            value if value == credit_index => "fractional-credit-v2",
            value if value == credit_index + 1 => "market-lifecycle-root-v2",
            value if value == credit_index + 2 => "neutral-lamport-sink",
            value if value == credit_index + 3 => "rent-sysvar",
            value if value == credit_index + 4 => "credit-funding-payer",
            _ => "system-program",
        };
        account_roles.push(CanonicalAccountRoleV1 {
            label,
            address,
            writable,
            signer,
        });
    }
    let mint = Address::new_from_array(core.collateral_policy.mint.bytes());
    let remainder = total_numerator % u128::from(core.resolution.facts.payout_denominator);
    let equations = vec![
        ExactEquation {
            name: "holder-approved bearer Eggs burned".into(),
            unit: IntegerUnit::EggAtoms {
                market: core.market_instance_id.bytes(),
                outcome,
            },
            left: u128::from(quantity),
            right: u128::from(quantity),
        },
        ExactEquation {
            name: "chain-derived whole collateral payout".into(),
            unit: IntegerUnit::CollateralAtoms { mint },
            left: u128::from(paid_atoms),
            right: u128::from(paid_atoms),
        },
        ExactEquation {
            name: "chain-derived retained payout numerator".into(),
            unit: IntegerUnit::PriceUnits {
                scale: core.resolution.facts.payout_denominator,
            },
            left: remainder,
            right: remainder,
        },
    ];
    let semantic_owner = crate::transaction_builder::SemanticOwner {
        package: "clutch-fractional-redemption-runtime".into(),
        schema: if action == FractionalRedemptionActionV1::RedeemBearerExact {
            "fractional-redemption/79/1/3/redeem-bearer-exact".into()
        } else {
            "fractional-redemption/79/1/5/redeem-bearer-credit".into()
        },
        release_sha256: release.elf_sha256,
    };
    let draft = if action == FractionalRedemptionActionV1::RedeemBearerExact {
        crate::transaction_builder::OwnedInstructionDraft::enabled_fractional_redeem_bearer_exact_v1(
            semantic_owner,
            release.program_id,
            metas,
            equations,
            core.ledger.next_sequence,
            core.policy.outcome_count,
            &payload,
        )
    } else {
        crate::transaction_builder::OwnedInstructionDraft::enabled_fractional_redeem_bearer_credit_v1(
            semantic_owner,
            release.program_id,
            metas,
            equations,
            core.ledger.next_sequence,
            core.policy.outcome_count,
            &payload,
        )
    }
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let unsigned_transaction = builder
        .build_atomic(&[draft])
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let mut authority = Sha256::new();
    authority.update(fractional_authority_state_id_v1(
        &frame.core.ordered(),
        release,
    ));
    for account in [
        frame.collateral_mint,
        frame.collateral_destination,
        frame.hoard_authority,
        frame.hoard_token,
        frame.outcome_token_program,
        frame.outcome_token_programdata,
        frame.bearer_source,
        frame.collateral_token_programdata,
    ]
    .into_iter()
    .chain(frame.outcome_mints.iter().copied())
    {
        authority.update(account.address.to_bytes());
        authority.update(account.lamports.to_le_bytes());
        authority.update(account.provenance.slot.to_le_bytes());
        authority.update(Sha256::digest(&account.data));
    }
    if let Some(value) = credit_frame {
        authority.update(value.credit.address.to_bytes());
        authority.update(value.credit.observed_slot.to_le_bytes());
        if let Some(account) = value.credit.present {
            authority.update(Sha256::digest(&account.data));
        }
        authority.update([credit_mode]);
        authority.update(credit_sequence.to_le_bytes());
    }
    let authority_state_sha256 = authority.finalize().into();
    let cursor = ResumableWorkflowCursor {
        workflow_id,
        lane: WorkflowLane::FractionalRedemption,
        generation: core.policy.domain_generation,
        position: WorkflowPosition {
            phase: u16::from(action.tag()),
            item: core.ledger.next_sequence,
        },
        observed_state_sha256: authority_state_sha256,
    };
    let planned = PlannedWorkflowNode {
        manifest_sha256: release.release_manifest_sha256,
        cursor,
        coordinate: CanonicalActionCoordinate::FractionalRedemption(action),
        unsigned_transaction,
        reload_authoritative_accounts: true,
    };
    validate_unsigned_fractional_plan(coordinate, builder.payer(), &account_roles, &planned)?;
    let release_key = release.key();
    let driver_account_slot = frame.core.fractional_ledger.provenance.slot;
    let draft_id = action_material_id(
        &release_key,
        &release_key,
        release.release_manifest_sha256,
        release.capability_profile_id,
        coordinate,
        frame.core.fractional_ledger.address,
        driver_account_slot,
        cursor,
        authority_state_sha256,
        freshness,
        builder.payer(),
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key: release_key.clone(),
        driver_release_key: release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        variant: None,
        driver_account: frame.core.fractional_ledger.address,
        driver_account_slot,
        cursor,
        authority_state_sha256,
        freshness,
        fee_payer: builder.payer(),
        account_roles,
        planned,
        draft_id,
    })
}

fn authenticate_fractional_payout_position_v1(
    release: &IndexedProgramRelease,
    freshness: ActionFreshnessBoundaryV1,
    core: AuthenticatedFractionalHolderCoreV1,
    core_frame: FractionalHolderCoreFrameV1<'_>,
    claimant: Address,
    position_account: &ObservedRpcAccount,
    replay_account: &ObservedRpcAccount,
) -> Result<u64> {
    for account in [position_account, replay_account] {
        if account.owner != release.program_id
            || account.executable
            || account.provenance.release_key != release.key()
            || account.provenance.commitment != RpcCommitment::Finalized
            || account.provenance.slot == 0
            || account.provenance.slot > freshness.observed_slot
            || account.provenance.cluster_key != core_frame.realm.provenance.cluster_key
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    let position = PositionAccountV3::decode(&position_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let replay = ReplayV3Envelope::decode(&replay_account.data, &OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let fields = position.fields();
    let header = replay.header();
    let purpose = [u8::from(PositionPurposeV3::General)];
    let position_pda = Address::find_program_address(
        &[
            POSITION_V3_PDA_PREFIX,
            &core.market_instance_id.bytes(),
            &claimant.to_bytes(),
            &purpose,
            &core_frame.market_runtime.address.to_bytes(),
        ],
        &release.program_id,
    );
    let replay_pda = Address::find_program_address(
        &[
            PURPOSE_REPLAY_V3_PDA_PREFIX,
            &position_account.address.to_bytes(),
            &purpose,
            &core_frame.market_runtime.address.to_bytes(),
        ],
        &release.program_id,
    );
    if position_account.address != position_pda.0
        || fields.stored_bump != position_pda.1
        || replay_account.address != replay_pda.0
        || header.stored_bump() != replay_pda.1
        || fields.lifecycle != PositionLifecycleV3::Open
        || fields.purpose != PositionPurposeV3::General
        || fields.owner.bytes() != claimant.to_bytes()
        || fields.controller.bytes() != claimant.to_bytes()
        || fields.market_instance_id.bytes() != core.market_instance_id.bytes()
        || fields.realm_id != core.realm.realm
        || fields.collateral_policy_id.bytes() != core.profile.collateral_policy_id.bytes()
        || fields.collateral_release_id.bytes() != core.policy.collateral_release.bytes()
        || fields.purpose_binding_id.bytes() != core_frame.market_runtime.address.to_bytes()
        || fields.replay_account.bytes() != replay_account.address.to_bytes()
        || header.lifecycle() != ReplayV3Lifecycle::Live
        || header.purpose() != PositionPurposeV3::General
        || header.position_account().bytes() != position_account.address.to_bytes()
        || header.replay_account().bytes() != replay_account.address.to_bytes()
        || header.position_generation() != fields.generation
        || header.purpose_binding_id() != fields.purpose_binding_id
        || header.next_sequence() == 0
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok((header.next_sequence(), position))
}

/// Construct action 4 from one holder's exact Position/outcome/quantity choice
/// and the chain-derived live, fresh, or reopened credit namespace.
#[allow(clippy::too_many_arguments)]
pub fn construct_fractional_internal_credit_material_v1(
    release: &IndexedProgramRelease,
    collateral: StructuredCollateralCatalogEntryV1<'_>,
    builder: &ProtocolTransactionBuilder,
    workflow_id: [u8; 32],
    freshness: ActionFreshnessBoundaryV1,
    intent: FractionalHolderIntentV1,
    frame: FractionalInternalCreditFrameV1<'_>,
) -> Result<CanonicalActionMaterialV1> {
    use clutch_retirement::Identity32V1;

    release
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    freshness.validate()?;
    intent.validate()?;
    let (claimant, position, outcome, quantity, payer) = match intent {
        FractionalHolderIntentV1::RedeemInternalCredit {
            claimant,
            position,
            outcome,
            quantity,
            funding_payer,
        } => (claimant, position, outcome, quantity, funding_payer),
        _ => return Err(CanonicalActionMaterialErrorV1::WrongSelection),
    };
    let action = FractionalRedemptionActionV1::RedeemInternalCredit;
    let coordinate = CanonicalIntentCoordinate {
        family_tag: FRACTIONAL_REDEMPTION_FAMILY_TAG,
        family_version: FRACTIONAL_REDEMPTION_FAMILY_VERSION,
        local_action: action.tag(),
    };
    if workflow_id == [0; 32]
        || builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
        || collateral.artifact_owner() != release.program_id
        || collateral.program().capability_profile_id != release.capability_profile_id
        || release.enabled_intents.binary_search(&coordinate).is_err()
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    let core = authenticate_fractional_holder_core_v1(
        release,
        collateral,
        freshness,
        frame.core,
        None,
    )?;
    if outcome >= core.policy.outcome_count || frame.position.address != position {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let (replay_sequence, position_state) = authenticate_fractional_payout_position_v1(
        release,
        freshness,
        core,
        frame.core,
        claimant,
        frame.position,
        frame.replay,
    )?;
    let position_fields = position_state.fields();
    if position_fields.outcome_count != core.policy.outcome_count
        || position_fields.native_eggs[usize::from(outcome)] < quantity
        || core.claim_ledger.aggregate_internal_supply[usize::from(outcome)] < quantity
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let (credit_mode, credit_sequence, prior_credit_numerator) =
        derive_fractional_credit_admission_v1(
            release,
            freshness,
            core,
            frame.core,
            claimant,
            payer,
            frame.credit,
        )?;
    let claim_numerator = u128::from(quantity)
        .checked_mul(u128::from(
            core.resolution.facts.payout_weights[usize::from(outcome)],
        ))
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let total_numerator = claim_numerator
        .checked_add(u128::from(prior_credit_numerator))
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let denominator = u128::from(core.resolution.facts.payout_denominator);
    let paid_atoms = u64::try_from(total_numerator / denominator)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let remainder = total_numerator % denominator;
    let payload = FractionalRedeemIntentV1 {
        expected_ledger_sequence: core.ledger.next_sequence,
        expected_credit_sequence: credit_sequence,
        expected_position_replay_sequence: replay_sequence,
        quantity,
        claimant: Identity32V1::new(claimant.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        claim_source: Identity32V1::new(frame.position.address.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        payout_target: Identity32V1::new(frame.position.address.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        credit_or_policy: Identity32V1::new(frame.credit.credit.address.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        outcome,
        credit_mode,
    }
    .encode()
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let mut addresses = vec![
        claimant,
        frame.core.realm.address,
        frame.core.profile.address,
        frame.core.collateral_policy.address,
        frame.core.collateral_token_program.address,
        frame.core.market_binding.address,
        frame.core.market_runtime.address,
        frame.core.market_instance.address,
        frame.core.hoard.address,
        frame.core.claim_ledger.address,
        frame.core.resolution.address,
        frame.core.fractional_policy.address,
        frame.core.fractional_ledger.address,
        frame.position.address,
        frame.replay.address,
        frame.credit.credit.address,
        frame.credit.market_root.address,
        frame.credit.neutral_sink.address,
        frame.credit.rent_sysvar.address,
    ];
    if credit_mode > 1 {
        addresses.extend([
            frame
                .credit
                .funding_payer
                .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?
                .address,
            frame
                .credit
                .system_program
                .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?
                .address,
        ]);
    }
    let payer_alias = credit_mode > 1 && addresses[19] == claimant;
    let mut metas = Vec::with_capacity(addresses.len());
    let mut account_roles = Vec::with_capacity(addresses.len());
    for (index, address) in addresses.iter().copied().enumerate() {
        let writable = matches!(index, 8 | 9 | 12 | 13 | 14 | 15)
            || (credit_mode > 1 && index == 19)
            || (index == 0 && payer_alias);
        let signer = index == 0 || (credit_mode > 1 && index == 19);
        metas.push(if writable {
            AccountMeta::new(address, signer)
        } else {
            AccountMeta::new_readonly(address, signer)
        });
        let label = match index {
            0 => "claimant",
            1 => "realm",
            2 => "profile",
            3 => "collateral-policy",
            4 => "collateral-token-program",
            5 => "market-binding-v2",
            6 => "market-runtime-v3",
            7 => "market-instance-preimage-v2",
            8 => "hoard-v2",
            9 => "claim-ledger-v3",
            10 => "resolution-v5",
            11 => "fractional-policy-v3",
            12 => "fractional-ledger-v1",
            13 => "position-v3",
            14 => "general-replay-v3",
            15 => "fractional-credit-v2",
            16 => "market-lifecycle-root-v2",
            17 => "neutral-lamport-sink",
            18 => "rent-sysvar",
            19 => "credit-funding-payer",
            _ => "system-program",
        };
        account_roles.push(CanonicalAccountRoleV1 {
            label,
            address,
            writable,
            signer,
        });
    }
    let equations = vec![
        ExactEquation {
            name: "holder-approved internal Eggs retired".into(),
            unit: IntegerUnit::EggAtoms {
                market: core.market_instance_id.bytes(),
                outcome,
            },
            left: u128::from(quantity),
            right: u128::from(quantity),
        },
        ExactEquation {
            name: "chain-derived whole collateral payout".into(),
            unit: IntegerUnit::CollateralAtoms {
                mint: Address::new_from_array(core.collateral_policy.mint.bytes()),
            },
            left: u128::from(paid_atoms),
            right: u128::from(paid_atoms),
        },
        ExactEquation {
            name: "chain-derived retained payout numerator".into(),
            unit: IntegerUnit::PriceUnits {
                scale: core.resolution.facts.payout_denominator,
            },
            left: remainder,
            right: remainder,
        },
    ];
    let draft = crate::transaction_builder::OwnedInstructionDraft::enabled_fractional_redeem_internal_credit_v1(
        crate::transaction_builder::SemanticOwner {
            package: "clutch-fractional-redemption-runtime".into(),
            schema: "fractional-redemption/79/1/4/redeem-internal-credit".into(),
            release_sha256: release.elf_sha256,
        },
        release.program_id,
        metas,
        equations,
        core.ledger.next_sequence,
        &payload,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let unsigned_transaction = builder
        .build_atomic(&[draft])
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let mut authority = Sha256::new();
    authority.update(fractional_authority_state_id_v1(
        &frame.core.ordered(),
        release,
    ));
    for account in [
        frame.position,
        frame.replay,
        frame.credit.market_root,
        frame.credit.neutral_sink,
        frame.credit.rent_sysvar,
    ] {
        authority.update(account.address.to_bytes());
        authority.update(account.provenance.slot.to_le_bytes());
        authority.update(Sha256::digest(&account.data));
    }
    authority.update(frame.credit.credit.address.to_bytes());
    authority.update(frame.credit.credit.observed_slot.to_le_bytes());
    if let Some(account) = frame.credit.credit.present {
        authority.update(Sha256::digest(&account.data));
    }
    let authority_state_sha256 = authority.finalize().into();
    let cursor = ResumableWorkflowCursor {
        workflow_id,
        lane: WorkflowLane::FractionalRedemption,
        generation: core.policy.domain_generation,
        position: WorkflowPosition {
            phase: u16::from(action.tag()),
            item: core.ledger.next_sequence,
        },
        observed_state_sha256: authority_state_sha256,
    };
    let planned = PlannedWorkflowNode {
        manifest_sha256: release.release_manifest_sha256,
        cursor,
        coordinate: CanonicalActionCoordinate::FractionalRedemption(action),
        unsigned_transaction,
        reload_authoritative_accounts: true,
    };
    validate_unsigned_fractional_plan(coordinate, builder.payer(), &account_roles, &planned)?;
    let release_key = release.key();
    let driver_account_slot = frame.core.fractional_ledger.provenance.slot;
    let draft_id = action_material_id(
        &release_key,
        &release_key,
        release.release_manifest_sha256,
        release.capability_profile_id,
        coordinate,
        frame.core.fractional_ledger.address,
        driver_account_slot,
        cursor,
        authority_state_sha256,
        freshness,
        builder.payer(),
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key: release_key.clone(),
        driver_release_key: release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        variant: None,
        driver_account: frame.core.fractional_ledger.address,
        driver_account_slot,
        cursor,
        authority_state_sha256,
        freshness,
        fee_payer: builder.payer(),
        account_roles,
        planned,
        draft_id,
    })
}

/// Construct action 6 or 7 from two holder consents and finalized credit
/// state. Transfer quantity is the sole arithmetic choice; merge always
/// consumes the complete source numerator decoded from chain state.
#[allow(clippy::too_many_arguments)]
pub fn construct_fractional_credit_move_material_v1(
    release: &IndexedProgramRelease,
    collateral: StructuredCollateralCatalogEntryV1<'_>,
    builder: &ProtocolTransactionBuilder,
    workflow_id: [u8; 32],
    freshness: ActionFreshnessBoundaryV1,
    intent: FractionalHolderIntentV1,
    frame: FractionalCreditMoveFrameV1<'_>,
) -> Result<CanonicalActionMaterialV1> {
    use clutch_fractional_redemption_runtime::FractionalTransferIntentV1;
    use clutch_retirement::Identity32V1;

    release
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    freshness.validate()?;
    intent.validate()?;
    if workflow_id == [0; 32]
        || builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
        || collateral.artifact_owner() != release.program_id
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    let (action, source_claimant, source_address, destination_claimant, requested, payout, payer) =
        match intent {
            FractionalHolderIntentV1::TransferCredit {
                source_claimant,
                source_credit,
                destination_claimant,
                numerator,
                payout,
                funding_payer,
            } => (
                FractionalRedemptionActionV1::TransferCredit,
                source_claimant,
                source_credit,
                destination_claimant,
                Some(numerator),
                payout,
                funding_payer,
            ),
            FractionalHolderIntentV1::MergeCredit {
                source_claimant,
                source_credit,
                destination_claimant,
                payout,
                funding_payer,
            } => (
                FractionalRedemptionActionV1::MergeCredit,
                source_claimant,
                source_credit,
                destination_claimant,
                None,
                payout,
                funding_payer,
            ),
            _ => return Err(CanonicalActionMaterialErrorV1::WrongSelection),
        };
    let coordinate = CanonicalIntentCoordinate {
        family_tag: FRACTIONAL_REDEMPTION_FAMILY_TAG,
        family_version: FRACTIONAL_REDEMPTION_FAMILY_VERSION,
        local_action: action.tag(),
    };
    if release.enabled_intents.binary_search(&coordinate).is_err() {
        return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
    }
    let external_programdata = match frame.payout {
        FractionalCreditPayoutFrameV1::Internal { .. } => None,
        FractionalCreditPayoutFrameV1::External {
            collateral_token_programdata,
            ..
        } => Some(collateral_token_programdata),
    };
    let core = authenticate_fractional_holder_core_v1(
        release,
        collateral,
        freshness,
        frame.core,
        external_programdata,
    )?;
    if frame.source_credit.address != source_address
        || frame.source_credit.owner != release.program_id
        || frame.source_credit.executable
        || frame.source_credit.provenance.release_key != release.key()
        || frame.source_credit.provenance.commitment != RpcCommitment::Finalized
        || frame.source_credit.provenance.slot == 0
        || frame.source_credit.provenance.slot > freshness.observed_slot
        || frame.source_credit.provenance.cluster_key != frame.core.realm.provenance.cluster_key
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let source_credit = FractionalCreditV2::decode(&frame.source_credit.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let source_pda = Address::find_program_address(
        &[
            FRACTIONAL_CREDIT_PDA_PREFIX,
            &frame.core.fractional_policy.address.to_bytes(),
            &source_claimant.to_bytes(),
        ],
        &release.program_id,
    );
    if frame.source_credit.address != source_pda.0
        || source_credit.stored_bump != source_pda.1
        || source_credit.policy_account.bytes() != frame.core.fractional_policy.address.to_bytes()
        || source_credit.ledger_account.bytes() != frame.core.fractional_ledger.address.to_bytes()
        || source_credit.market_instance.bytes() != core.market_instance_id.bytes()
        || source_credit.resolution_account != core.policy.resolution_account
        || source_credit.resolution_data_id != core.policy.resolution_data_id
        || source_credit.claimant.bytes() != source_claimant.to_bytes()
        || source_credit.domain_generation != core.policy.domain_generation
        || source_credit.numerator == 0
        || source_credit.numerator >= core.resolution.facts.payout_denominator
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let admission = FractionalCreditAdmissionFrameV1 {
        credit: frame.destination_credit,
        market_root: frame.market_root,
        neutral_sink: frame.neutral_sink,
        rent_sysvar: frame.rent_sysvar,
        funding_payer: frame.funding_payer,
        system_program: frame.system_program,
    };
    let (destination_mode, destination_sequence, destination_numerator) =
        derive_fractional_credit_admission_v1(
            release,
            freshness,
            core,
            frame.core,
            destination_claimant,
            payer,
            admission,
        )?;
    let moved = requested.unwrap_or(source_credit.numerator);
    if moved == 0 || moved > source_credit.numerator {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let payout_total = destination_numerator
        .checked_add(moved)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let paid_atoms = payout_total / core.resolution.facts.payout_denominator;
    let payout_replay_sequence = match (payout, frame.payout) {
        (
            FractionalHolderPayoutV1::InternalPosition { position },
            FractionalCreditPayoutFrameV1::Internal {
                position: observed,
                replay,
            },
        ) if position == observed.address => {
            authenticate_fractional_payout_position_v1(
                release,
                freshness,
                core,
                frame.core,
                destination_claimant,
                observed,
                replay,
            )?
            .0
        }
        (
            FractionalHolderPayoutV1::ExternalCollateral { token_account },
            FractionalCreditPayoutFrameV1::External {
                collateral_mint,
                destination,
                hoard_authority,
                hoard_token,
                collateral_token_programdata,
            },
        ) if token_account == destination.address => {
            let authority_pda = Address::find_program_address(
                &[HOARD_AUTHORITY_V2_PDA_SEED_V1, &core.market_instance_id.bytes()],
                &release.program_id,
            );
            let token_pda = Address::find_program_address(
                &[HOARD_TOKEN_V2_PDA_SEED_V1, &core.market_instance_id.bytes()],
                &release.program_id,
            );
            if collateral_mint.address.to_bytes() != core.collateral_policy.mint.bytes()
                || collateral_mint.owner != frame.core.collateral_token_program.address
                || destination.owner != frame.core.collateral_token_program.address
                || hoard_token.owner != frame.core.collateral_token_program.address
                || hoard_authority.address != authority_pda.0
                || hoard_token.address != token_pda.0
                || hoard_authority.address.to_bytes() != core.hoard.authority.bytes()
                || hoard_token.address.to_bytes() != core.hoard.token_account.bytes()
                || collateral_token_programdata.address != collateral.program().program_data
                || hoard_authority.executable
                || !hoard_authority.data.is_empty()
            {
                return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
            }
            let _hoard_token = decode_canonical_wrapper_token_v1(
                frame.core.collateral_token_program.address.to_bytes(),
                collateral_mint.address.to_bytes(),
                hoard_token.address.to_bytes(),
                hoard_authority.address.to_bytes(),
                &hoard_token.data,
            )
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            0
        }
        _ => return Err(CanonicalActionMaterialErrorV1::WrongSelection),
    };
    let (payout_kind, payout_target) = match payout {
        FractionalHolderPayoutV1::InternalPosition { position } => (1, position),
        FractionalHolderPayoutV1::ExternalCollateral { token_account } => (2, token_account),
    };
    let payload = FractionalTransferIntentV1 {
        expected_ledger_sequence: core.ledger.next_sequence,
        expected_source_sequence: source_credit.next_sequence,
        expected_destination_sequence: destination_sequence,
        expected_payout_replay_sequence: payout_replay_sequence,
        numerator: requested.unwrap_or(0),
        source_claimant: Identity32V1::new(source_claimant.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        destination_claimant: Identity32V1::new(destination_claimant.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        source_credit: Identity32V1::new(frame.source_credit.address.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        destination_credit: Identity32V1::new(frame.destination_credit.address.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        payout_target: Identity32V1::new(payout_target.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        payout_kind,
        destination_mode,
    }
    .encode()
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;

    let mut addresses = vec![
        source_claimant,
        destination_claimant,
        frame.core.realm.address,
        frame.core.profile.address,
        frame.core.collateral_policy.address,
        frame.core.collateral_token_program.address,
        frame.core.market_binding.address,
        frame.core.market_runtime.address,
        frame.core.market_instance.address,
        frame.core.hoard.address,
        frame.core.claim_ledger.address,
        frame.core.resolution.address,
        frame.core.fractional_policy.address,
        frame.core.fractional_ledger.address,
        frame.source_credit.address,
        frame.destination_credit.address,
    ];
    match frame.payout {
        FractionalCreditPayoutFrameV1::Internal { position, replay } => {
            addresses.extend([
                position.address,
                replay.address,
                frame.market_root.address,
                frame.neutral_sink.address,
                frame.rent_sysvar.address,
            ]);
        }
        FractionalCreditPayoutFrameV1::External {
            collateral_mint,
            destination,
            hoard_authority,
            hoard_token,
            collateral_token_programdata,
        } => {
            addresses.extend([
                collateral_mint.address,
                destination.address,
                hoard_authority.address,
                hoard_token.address,
                collateral_token_programdata.address,
                frame.market_root.address,
                frame.neutral_sink.address,
                frame.rent_sysvar.address,
            ]);
        }
    }
    let funding_index = addresses.len();
    if destination_mode > 1 {
        addresses.extend([
            frame
                .funding_payer
                .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?
                .address,
            frame
                .system_program
                .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?
                .address,
        ]);
    }
    let payer_alias_source = destination_mode > 1 && addresses[funding_index] == source_claimant;
    let payer_alias_destination =
        destination_mode > 1 && addresses[funding_index] == destination_claimant;
    let mut metas = Vec::with_capacity(addresses.len());
    let mut account_roles = Vec::with_capacity(addresses.len());
    for (index, address) in addresses.iter().copied().enumerate() {
        let payout_writable = if payout_kind == 1 {
            matches!(index, 16 | 17)
        } else {
            matches!(index, 17 | 19)
        };
        let writable = matches!(index, 9 | 10 | 13 | 14 | 15)
            || payout_writable
            || (destination_mode > 1 && index == funding_index)
            || (index == 0 && payer_alias_source)
            || (index == 1 && payer_alias_destination);
        let signer = matches!(index, 0 | 1)
            || (destination_mode > 1 && index == funding_index);
        metas.push(if writable {
            AccountMeta::new(address, signer)
        } else {
            AccountMeta::new_readonly(address, signer)
        });
        let label = match index {
            0 => "source-claimant",
            1 => "destination-claimant",
            2 => "realm",
            3 => "profile",
            4 => "collateral-policy",
            5 => "collateral-token-program",
            6 => "market-binding-v2",
            7 => "market-runtime-v3",
            8 => "market-instance-preimage-v2",
            9 => "hoard-v2",
            10 => "claim-ledger-v3",
            11 => "resolution-v5",
            12 => "fractional-policy-v3",
            13 => "fractional-ledger-v1",
            14 => "source-credit-v2",
            15 => "destination-credit-v2",
            value if value == funding_index => "credit-funding-payer",
            value if value == funding_index + 1 => "system-program",
            _ => "payout-and-lifecycle-role",
        };
        account_roles.push(CanonicalAccountRoleV1 {
            label,
            address,
            writable,
            signer,
        });
    }
    let equations = vec![
        ExactEquation {
            name: "holder-approved credit numerator moved".into(),
            unit: IntegerUnit::PriceUnits {
                scale: core.resolution.facts.payout_denominator,
            },
            left: u128::from(moved),
            right: u128::from(moved),
        },
        ExactEquation {
            name: "chain-derived whole credit payout".into(),
            unit: IntegerUnit::CollateralAtoms {
                mint: Address::new_from_array(core.collateral_policy.mint.bytes()),
            },
            left: u128::from(paid_atoms),
            right: u128::from(paid_atoms),
        },
    ];
    let draft = crate::transaction_builder::OwnedInstructionDraft::enabled_fractional_credit_move_v1(
        crate::transaction_builder::SemanticOwner {
            package: "clutch-fractional-redemption-runtime".into(),
            schema: if action == FractionalRedemptionActionV1::TransferCredit {
                "fractional-redemption/79/1/6/transfer-credit".into()
            } else {
                "fractional-redemption/79/1/7/merge-credit".into()
            },
            release_sha256: release.elf_sha256,
        },
        release.program_id,
        metas,
        equations,
        action,
        core.ledger.next_sequence,
        &payload,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let unsigned_transaction = builder
        .build_atomic(&[draft])
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let mut authority = Sha256::new();
    authority.update(fractional_authority_state_id_v1(
        &frame.core.ordered(),
        release,
    ));
    for account in [frame.source_credit, frame.market_root, frame.neutral_sink, frame.rent_sysvar] {
        authority.update(account.address.to_bytes());
        authority.update(account.provenance.slot.to_le_bytes());
        authority.update(Sha256::digest(&account.data));
    }
    authority.update(frame.destination_credit.address.to_bytes());
    authority.update(frame.destination_credit.observed_slot.to_le_bytes());
    if let Some(account) = frame.destination_credit.present {
        authority.update(Sha256::digest(&account.data));
    }
    let authority_state_sha256 = authority.finalize().into();
    let cursor = ResumableWorkflowCursor {
        workflow_id,
        lane: WorkflowLane::FractionalRedemption,
        generation: core.policy.domain_generation,
        position: WorkflowPosition {
            phase: u16::from(action.tag()),
            item: core.ledger.next_sequence,
        },
        observed_state_sha256: authority_state_sha256,
    };
    let planned = PlannedWorkflowNode {
        manifest_sha256: release.release_manifest_sha256,
        cursor,
        coordinate: CanonicalActionCoordinate::FractionalRedemption(action),
        unsigned_transaction,
        reload_authoritative_accounts: true,
    };
    validate_unsigned_fractional_plan(coordinate, builder.payer(), &account_roles, &planned)?;
    let release_key = release.key();
    let driver_account_slot = frame.core.fractional_ledger.provenance.slot;
    let draft_id = action_material_id(
        &release_key,
        &release_key,
        release.release_manifest_sha256,
        release.capability_profile_id,
        coordinate,
        frame.core.fractional_ledger.address,
        driver_account_slot,
        cursor,
        authority_state_sha256,
        freshness,
        builder.payer(),
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key: release_key.clone(),
        driver_release_key: release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        variant: None,
        driver_account: frame.core.fractional_ledger.address,
        driver_account_slot,
        cursor,
        authority_state_sha256,
        freshness,
        fee_payer: builder.payer(),
        account_roles,
        planned,
        draft_id,
    })
}

/// Derive the first current Fractional operator action entirely from one
/// finalized chain frame. The lowest outcome containing one exact common lot
/// is canonical; callers cannot choose an outcome, amount, payload, role
/// order, signer vector, or instruction bytes.
pub fn construct_fractional_redeem_internal_exact_material_v1(
    release: &IndexedProgramRelease,
    collateral_release: AdapterReleaseV2,
    builder: &ProtocolTransactionBuilder,
    workflow_id: [u8; 32],
    freshness: ActionFreshnessBoundaryV1,
    frame: FractionalRedeemInternalExactFrameV1<'_>,
) -> Result<CanonicalActionMaterialV1> {
    use clutch_retirement::Identity32V1;

    release
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    collateral_release
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    freshness.validate()?;
    let action = FractionalRedemptionActionV1::RedeemInternalExact;
    let coordinate = CanonicalIntentCoordinate {
        family_tag: FRACTIONAL_REDEMPTION_FAMILY_TAG,
        family_version: FRACTIONAL_REDEMPTION_FAMILY_VERSION,
        local_action: action.tag(),
    };
    for family in [
        crate::rpc_index::CanonicalFamily::Collateral,
        crate::rpc_index::CanonicalFamily::Fractional,
        crate::rpc_index::CanonicalFamily::General,
        crate::rpc_index::CanonicalFamily::PositionV3,
        crate::rpc_index::CanonicalFamily::ReplayV3,
    ] {
        if release.families.binary_search(&family).is_err() {
            return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
        }
    }
    if workflow_id == [0; 32]
        || builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }

    let ordered = frame.ordered();
    let release_key = release.key();
    let cluster = &frame.realm.provenance.cluster_key;
    let mut identities = BTreeSet::new();
    for (index, account) in ordered.iter().enumerate() {
        if account.address == Address::default()
            || account.provenance.commitment != RpcCommitment::Finalized
            || account.provenance.slot == 0
            || account.provenance.slot > freshness.observed_slot
            || account.provenance.cluster_key != *cluster
            || !identities.insert(account.address)
            || (index != 3
                && (account.owner != release.program_id
                    || account.executable
                    || account.provenance.release_key != release_key))
            || (index == 3 && !account.executable)
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }

    let realm = RealmAccount::decode(&frame.realm.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let profile = ProfileAccount::decode(&frame.profile.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let collateral_policy = CollateralPolicyV2::decode(&frame.collateral_policy.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_binding = MarketBindingV5::decode(&frame.market_binding.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_runtime = MarketRuntimeV3AccountV1::decode(&frame.market_runtime.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_instance = MarketInstancePreimageV2::decode(&frame.market_instance.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_instance_id = market_instance
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let hoard = HoardV2::decode(&frame.hoard.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let claim_ledger = ClaimLedgerV3::decode(&frame.claim_ledger.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let resolution = ResolutionV5::decode(&frame.resolution.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let policy = FractionalPolicyV3::decode(&frame.fractional_policy.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let ledger = FractionalLedgerV1::decode(&frame.fractional_ledger.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let position = PositionAccountV3::decode(&frame.position.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let replay = ReplayV3Envelope::decode(&frame.replay.data, &OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let fields = position.fields();
    let replay_header = replay.header();

    let realm_pda = Address::find_program_address(
        &[REALM_PDA_SEED_V1, &realm.realm.bytes()],
        &release.program_id,
    );
    let profile_pda = Address::find_program_address(
        &[PROFILE_PDA_SEED_V1, &realm.realm.bytes(), &profile.profile.bytes()],
        &release.program_id,
    );
    let collateral_policy_pda = Address::find_program_address(
        &[COLLATERAL_POLICY_PDA_SEED_V1, &profile.profile.bytes(), &profile.collateral_policy_id.bytes()],
        &release.program_id,
    );
    let market_binding_pda = Address::find_program_address(
        &[MARKET_BINDING_SEED_DOMAIN_V1, &market_instance_id.bytes()],
        &release.program_id,
    );
    let market_runtime_pda = Address::find_program_address(
        &[MARKET_RUNTIME_SEED_DOMAIN_V1, &frame.market_binding.address.to_bytes()],
        &release.program_id,
    );
    let artifact_kind = [ArtifactKind::MarketInstancePreimageV2.byte()];
    let market_instance_pda = Address::find_program_address(
        &[b"dc:product-artifact:v1", &artifact_kind, &market_instance_id.bytes()],
        &release.program_id,
    );
    let hoard_pda = Address::find_program_address(
        &[HOARD_V2_PDA_SEED_V1, &market_instance_id.bytes()],
        &release.program_id,
    );
    let claim_ledger_pda = Address::find_program_address(
        &[CLAIM_LEDGER_V3_PDA_SEED_V1, &market_instance_id.bytes()],
        &release.program_id,
    );
    let resolution_pda = Address::find_program_address(
        &[b"dc:resolution:v5", &market_instance_id.bytes()],
        &release.program_id,
    );
    let fractional_policy_pda = Address::find_program_address(
        &[FRACTIONAL_POLICY_PDA_PREFIX, &market_instance_id.bytes(), &frame.resolution.address.to_bytes()],
        &release.program_id,
    );
    let fractional_ledger_pda = Address::find_program_address(
        &[FRACTIONAL_LEDGER_PDA_PREFIX, &frame.fractional_policy.address.to_bytes()],
        &release.program_id,
    );
    let purpose = [u8::from(PositionPurposeV3::General)];
    let position_pda = Address::find_program_address(
        &[POSITION_V3_PDA_PREFIX, &market_instance_id.bytes(), &fields.owner.bytes(), &purpose, &frame.market_runtime.address.to_bytes()],
        &release.program_id,
    );
    let replay_pda = Address::find_program_address(
        &[PURPOSE_REPLAY_V3_PDA_PREFIX, &frame.position.address.to_bytes(), &purpose, &frame.market_runtime.address.to_bytes()],
        &release.program_id,
    );

    if frame.realm.address != realm_pda.0
        || realm.stored_bump != realm_pda.1
        || frame.profile.address != profile_pda.0
        || frame.collateral_policy.address != collateral_policy_pda.0
        || frame.collateral_token_program.address.to_bytes() != collateral_policy.token_program.bytes()
        || collateral_release
            .id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?
            .bytes()
            != collateral_policy.adapter_release.bytes()
        || collateral_release.token_program.bytes() != collateral_policy.token_program.bytes()
        || collateral_release.token_program_deployment.bytes()
            != collateral_policy.token_program_deployment.bytes()
        || profile.realm != realm.realm
        || profile.profile != realm.profile
        || profile.collateral_policy_id.bytes() != collateral_policy
            .id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
            .bytes()
        || profile.adapter_release_id.bytes() != collateral_policy.adapter_release.bytes()
        || frame.market_binding.address != market_binding_pda.0
        || frame.market_runtime.address != market_runtime_pda.0
        || frame.market_instance.address != market_instance_pda.0
        || market_binding.base().base().market.bytes() != frame.market_runtime.address.to_bytes()
        || market_binding.base().base().market_instance_v2_id.bytes() != market_instance_id.bytes()
        || market_binding.base().base().market_genesis_profile_v2_id.bytes()
            != market_instance.market_genesis_profile_id.bytes()
        || market_runtime.market_binding.bytes() != frame.market_binding.address.to_bytes()
        || market_runtime.market_instance_v2_id.bytes() != market_instance_id.bytes()
        || frame.hoard.address != hoard_pda.0
        || hoard.stored_bump != hoard_pda.1
        || frame.claim_ledger.address != claim_ledger_pda.0
        || claim_ledger.stored_bump != claim_ledger_pda.1
        || frame.resolution.address != resolution_pda.0
        || resolution.stored_bump != resolution_pda.1
        || frame.fractional_policy.address != fractional_policy_pda.0
        || policy.stored_bump != fractional_policy_pda.1
        || frame.fractional_ledger.address != fractional_ledger_pda.0
        || ledger.stored_bump != fractional_ledger_pda.1
        || frame.position.address != position_pda.0
        || fields.stored_bump != position_pda.1
        || frame.replay.address != replay_pda.0
        || replay_header.stored_bump() != replay_pda.1
        || hoard.market_instance_id.bytes() != market_instance_id.bytes()
        || claim_ledger.market_instance_id.bytes() != market_instance_id.bytes()
        || resolution.facts.market_instance_id.bytes() != market_instance_id.bytes()
        || policy.market_instance.bytes() != market_instance_id.bytes()
        || fields.market_instance_id.bytes() != market_instance_id.bytes()
        || hoard.realm_id.bytes() != realm.realm.bytes()
        || claim_ledger.realm_id.bytes() != realm.realm.bytes()
        || policy.realm.bytes() != realm.realm.bytes()
        || fields.realm_id.bytes() != realm.realm.bytes()
        || policy.collateral_policy.bytes() != profile.collateral_policy_id.bytes()
        || fields.collateral_policy_id.bytes() != profile.collateral_policy_id.bytes()
        || policy.collateral_release.bytes() != profile.adapter_release_id.bytes()
        || fields.collateral_release_id.bytes() != profile.adapter_release_id.bytes()
        || hoard.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || claim_ledger.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || resolution.state != ResolutionStateV5::Finalized
        || ledger.phase != FractionalLedgerPhaseV1::Live
        || ledger.policy_account.bytes() != frame.fractional_policy.address.to_bytes()
        || ledger.claim_ledger_account.bytes() != frame.claim_ledger.address.to_bytes()
        || ledger.domain_generation != policy.domain_generation
        || claim_ledger.fractional_policy_id.bytes() != frame.fractional_policy.address.to_bytes()
        || claim_ledger.fractional_ledger_account.bytes() != frame.fractional_ledger.address.to_bytes()
        || claim_ledger.resolution_account.bytes() != frame.resolution.address.to_bytes()
        || claim_ledger.next_fractional_sequence != ledger.next_sequence
        || policy.resolution_account.bytes() != frame.resolution.address.to_bytes()
        || policy.domain_generation != resolution.facts.generation
        || policy.outcome_count != resolution.facts.outcome_count
        || policy.outcome_count != claim_ledger.outcome_count
        || policy.outcome_count != hoard.outcome_count
        || policy.outcome_count != fields.outcome_count
        || resolution.facts.native_claim_basis_id.bytes() != claim_ledger.native_claim_basis_id.bytes()
        || fields.purpose != PositionPurposeV3::General
        || fields.lifecycle != PositionLifecycleV3::Open
        || fields.owner != fields.controller
        || fields.purpose_binding_id.bytes() != frame.market_runtime.address.to_bytes()
        || fields.replay_account.bytes() != frame.replay.address.to_bytes()
        || replay_header.lifecycle() != ReplayV3Lifecycle::Live
        || replay_header.purpose() != PositionPurposeV3::General
        || replay_header.position_account().bytes() != frame.position.address.to_bytes()
        || replay_header.replay_account().bytes() != frame.replay.address.to_bytes()
        || replay_header.position_generation() != fields.generation
        || replay_header.purpose_binding_id() != fields.purpose_binding_id
        || replay_header.next_sequence() == 0
        || builder.payer().to_bytes() == fields.owner.bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    let no_native_claims = claim_ledger
        .aggregate_internal_supply
        .iter()
        .chain(claim_ledger.aggregate_materialized_supply.iter())
        .all(|amount| *amount == 0);
    if no_native_claims {
        let backing_numerator = u128::from(hoard.locked_claim_principal_atoms)
            .checked_mul(u128::from(resolution.facts.payout_denominator))
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        if backing_numerator < ledger.aggregate_credit_numerator {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        return construct_fractional_seal_claims_exhausted_material_v1(
            release,
            builder,
            workflow_id,
            freshness,
            frame,
            &ordered,
            policy,
            ledger,
            market_instance_id.bytes(),
        );
    }
    if release.enabled_intents.binary_search(&coordinate).is_err() {
        return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
    }

    let outcome = (0..usize::from(policy.outcome_count))
        .find(|index| {
            fields.native_eggs[*index] >= policy.common_lot
                && claim_ledger.aggregate_internal_supply[*index] >= policy.common_lot
                && resolution.payout_atoms(*index as u8, policy.common_lot).is_ok()
        })
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let outcome = u8::try_from(outcome)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let paid_atoms = resolution
        .payout_atoms(outcome, policy.common_lot)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let actor = Address::new_from_array(fields.owner.bytes());
    let payload = FractionalRedeemIntentV1 {
        expected_ledger_sequence: ledger.next_sequence,
        expected_credit_sequence: 0,
        expected_position_replay_sequence: replay_header.next_sequence(),
        quantity: policy.common_lot,
        claimant: Identity32V1::new(actor.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?,
        claim_source: Identity32V1::new(frame.position.address.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?,
        payout_target: Identity32V1::new(frame.position.address.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?,
        credit_or_policy: Identity32V1::new(frame.fractional_policy.address.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?,
        outcome,
        credit_mode: 0,
    }
    .encode()
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;

    let addresses = [
        actor,
        frame.realm.address,
        frame.profile.address,
        frame.collateral_policy.address,
        frame.collateral_token_program.address,
        frame.market_binding.address,
        frame.market_runtime.address,
        frame.market_instance.address,
        frame.hoard.address,
        frame.claim_ledger.address,
        frame.resolution.address,
        frame.fractional_policy.address,
        frame.fractional_ledger.address,
        frame.position.address,
        frame.replay.address,
    ];
    let labels = [
        "claimant", "realm", "profile", "collateral-policy", "collateral-token-program",
        "market-binding-v2", "market-runtime-v3", "market-instance-preimage-v2", "hoard-v2",
        "claim-ledger-v3", "resolution-v5", "fractional-policy-v3", "fractional-ledger-v1",
        "position-v3", "general-replay-v3",
    ];
    let contract = clutch_fractional_redemption_runtime::fractional_account_contract_v1(action);
    let mut metas = Vec::with_capacity(addresses.len());
    let mut account_roles = Vec::with_capacity(addresses.len());
    for (index, address) in addresses.iter().copied().enumerate() {
        let mask = 1_u32 << index;
        let writable = contract.writable_mask & mask != 0;
        let signer = contract.signer_mask & mask != 0;
        metas.push(if writable {
            AccountMeta::new(address, signer)
        } else {
            AccountMeta::new_readonly(address, signer)
        });
        account_roles.push(CanonicalAccountRoleV1 {
            label: labels[index],
            address,
            writable,
            signer,
        });
    }
    let mint = Address::new_from_array(collateral_policy.mint.bytes());
    let equations = vec![
        ExactEquation {
            name: "chain-derived internal Eggs burned".into(),
            unit: IntegerUnit::EggAtoms { market: market_instance_id.bytes(), outcome },
            left: u128::from(policy.common_lot),
            right: u128::from(policy.common_lot),
        },
        ExactEquation {
            name: "chain-derived exact collateral payout".into(),
            unit: IntegerUnit::CollateralAtoms { mint },
            left: u128::from(paid_atoms),
            right: u128::from(paid_atoms),
        },
    ];
    let draft = crate::transaction_builder::OwnedInstructionDraft::enabled_fractional_redeem_internal_exact_v1(
        "redeem-fractional-internal-exact",
        crate::transaction_builder::SemanticOwner {
            package: "clutch-fractional-redemption-runtime".into(),
            schema: "fractional-redemption/79/1/2/redeem-internal-exact".into(),
            release_sha256: release.elf_sha256,
        },
        release.program_id,
        metas,
        equations,
        ledger.next_sequence,
        &payload,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let unsigned_transaction = builder
        .build_atomic(&[draft])
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let authority_state_sha256 = fractional_authority_state_id_v1(&ordered, release);
    let cursor = ResumableWorkflowCursor {
        workflow_id,
        lane: WorkflowLane::FractionalRedemption,
        generation: policy.domain_generation,
        position: WorkflowPosition {
            phase: u16::from(action.tag()),
            item: ledger.next_sequence,
        },
        observed_state_sha256: authority_state_sha256,
    };
    let planned = PlannedWorkflowNode {
        manifest_sha256: release.release_manifest_sha256,
        cursor,
        coordinate: CanonicalActionCoordinate::FractionalRedemption(action),
        unsigned_transaction,
        reload_authoritative_accounts: true,
    };
    validate_unsigned_fractional_plan(coordinate, builder.payer(), &account_roles, &planned)?;
    let driver_account_slot = frame.fractional_ledger.provenance.slot;
    let draft_id = action_material_id(
        &release_key,
        &release_key,
        release.release_manifest_sha256,
        release.capability_profile_id,
        coordinate,
        frame.fractional_ledger.address,
        driver_account_slot,
        cursor,
        authority_state_sha256,
        freshness,
        builder.payer(),
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key: release_key.clone(),
        driver_release_key: release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        variant: None,
        driver_account: frame.fractional_ledger.address,
        driver_account_slot,
        cursor,
        authority_state_sha256,
        freshness,
        fee_payer: builder.payer(),
        account_roles,
        planned,
        draft_id,
    })
}

#[allow(clippy::too_many_arguments)]
fn construct_fractional_seal_claims_exhausted_material_v1(
    release: &IndexedProgramRelease,
    builder: &ProtocolTransactionBuilder,
    workflow_id: [u8; 32],
    freshness: ActionFreshnessBoundaryV1,
    frame: FractionalRedeemInternalExactFrameV1<'_>,
    ordered: &[&ObservedRpcAccount; 14],
    policy: FractionalPolicyV3,
    ledger: FractionalLedgerV1,
    market_instance: [u8; 32],
) -> Result<CanonicalActionMaterialV1> {
    let action = FractionalRedemptionActionV1::SealClaimsExhausted;
    let coordinate = CanonicalIntentCoordinate {
        family_tag: FRACTIONAL_REDEMPTION_FAMILY_TAG,
        family_version: FRACTIONAL_REDEMPTION_FAMILY_VERSION,
        local_action: action.tag(),
    };
    if release.enabled_intents.binary_search(&coordinate).is_err() {
        return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
    }
    let payload = FractionalTerminalIntentV1 {
        expected_ledger_sequence: ledger.next_sequence,
    }
    .encode()
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let addresses = [
        frame.realm.address,
        frame.profile.address,
        frame.collateral_policy.address,
        frame.collateral_token_program.address,
        frame.market_binding.address,
        frame.market_runtime.address,
        frame.market_instance.address,
        frame.hoard.address,
        frame.claim_ledger.address,
        frame.resolution.address,
        frame.fractional_policy.address,
        frame.fractional_ledger.address,
    ];
    let labels = [
        "realm", "profile", "collateral-policy", "collateral-token-program",
        "market-binding-v2", "market-runtime-v3", "market-instance-preimage-v2", "hoard-v2",
        "claim-ledger-v3", "resolution-v5", "fractional-policy-v3", "fractional-ledger-v1",
    ];
    let contract = clutch_fractional_redemption_runtime::fractional_account_contract_v1(action);
    let mut metas = Vec::with_capacity(addresses.len());
    let mut account_roles = Vec::with_capacity(addresses.len());
    for (index, address) in addresses.iter().copied().enumerate() {
        let mask = 1_u32 << index;
        let writable = contract.writable_mask & mask != 0;
        let signer = contract.signer_mask & mask != 0;
        metas.push(if writable {
            AccountMeta::new(address, signer)
        } else {
            AccountMeta::new_readonly(address, signer)
        });
        account_roles.push(CanonicalAccountRoleV1 {
            label: labels[index],
            address,
            writable,
            signer,
        });
    }
    let equations = vec![ExactEquation {
        name: "chain-derived zero native claim supply".into(),
        unit: IntegerUnit::EggAtoms {
            market: market_instance,
            outcome: 0,
        },
        left: 0,
        right: 0,
    }];
    let draft = crate::transaction_builder::OwnedInstructionDraft::enabled_fractional_seal_claims_exhausted_v1(
        crate::transaction_builder::SemanticOwner {
            package: "clutch-fractional-redemption-runtime".into(),
            schema: "fractional-redemption/79/1/9/seal-claims-exhausted".into(),
            release_sha256: release.elf_sha256,
        },
        release.program_id,
        metas,
        equations,
        ledger.next_sequence,
        &payload,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let unsigned_transaction = builder
        .build_atomic(&[draft])
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let authority_state_sha256 = fractional_authority_state_id_v1(&ordered[..12], release);
    let cursor = ResumableWorkflowCursor {
        workflow_id,
        lane: WorkflowLane::FractionalRedemption,
        generation: policy.domain_generation,
        position: WorkflowPosition {
            phase: u16::from(action.tag()),
            item: ledger.next_sequence,
        },
        observed_state_sha256: authority_state_sha256,
    };
    let planned = PlannedWorkflowNode {
        manifest_sha256: release.release_manifest_sha256,
        cursor,
        coordinate: CanonicalActionCoordinate::FractionalRedemption(action),
        unsigned_transaction,
        reload_authoritative_accounts: true,
    };
    validate_unsigned_fractional_plan(coordinate, builder.payer(), &account_roles, &planned)?;
    let release_key = release.key();
    let driver_account_slot = frame.fractional_ledger.provenance.slot;
    let draft_id = action_material_id(
        &release_key,
        &release_key,
        release.release_manifest_sha256,
        release.capability_profile_id,
        coordinate,
        frame.fractional_ledger.address,
        driver_account_slot,
        cursor,
        authority_state_sha256,
        freshness,
        builder.payer(),
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key: release_key.clone(),
        driver_release_key: release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        variant: None,
        driver_account: frame.fractional_ledger.address,
        driver_account_slot,
        cursor,
        authority_state_sha256,
        freshness,
        fee_payer: builder.payer(),
        account_roles,
        planned,
        draft_id,
    })
}

/// Derive action `79/1/8` from one live zero credit and its persisted close
/// funding identities. This constructor requires the transaction payer to be
/// the stored rent payer so Solana's global payer writability cannot weaken a
/// read-only role.
pub fn construct_fractional_close_zero_credit_material_v1(
    release: &IndexedProgramRelease,
    collateral_release: AdapterReleaseV2,
    builder: &ProtocolTransactionBuilder,
    workflow_id: [u8; 32],
    freshness: ActionFreshnessBoundaryV1,
    frame: FractionalCloseZeroCreditFrameV1<'_>,
) -> Result<CanonicalActionMaterialV1> {
    use clutch_retirement::Identity32V1;

    release
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    collateral_release
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    freshness.validate()?;
    let action = FractionalRedemptionActionV1::CloseZeroCredit;
    let coordinate = CanonicalIntentCoordinate {
        family_tag: FRACTIONAL_REDEMPTION_FAMILY_TAG,
        family_version: FRACTIONAL_REDEMPTION_FAMILY_VERSION,
        local_action: action.tag(),
    };
    if workflow_id == [0; 32]
        || release.enabled_intents.binary_search(&coordinate).is_err()
        || builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
        || builder.payer() != frame.payer.address
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    let base = frame.base.ordered();
    let cluster = &frame.base.realm.provenance.cluster_key;
    let extras = [
        frame.credit,
        frame.payer,
        frame.market_root,
        frame.neutral_sink,
        frame.rent_sysvar,
    ];
    for (index, account) in base.iter().take(12).enumerate() {
        if account.address == Address::default()
            || account.provenance.commitment != RpcCommitment::Finalized
            || account.provenance.slot == 0
            || account.provenance.slot > freshness.observed_slot
            || account.provenance.cluster_key != *cluster
            || account.executable != (index == 3)
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    for account in extras {
        if account.address == Address::default()
            || account.provenance.commitment != RpcCommitment::Finalized
            || account.provenance.slot == 0
            || account.provenance.slot > freshness.observed_slot
            || account.provenance.cluster_key != *cluster
            || account.executable
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    for account in [
        frame.base.realm,
        frame.base.profile,
        frame.base.collateral_policy,
        frame.base.market_binding,
        frame.base.market_runtime,
        frame.base.market_instance,
        frame.base.hoard,
        frame.base.claim_ledger,
        frame.base.resolution,
        frame.base.fractional_policy,
        frame.base.fractional_ledger,
        frame.credit,
        frame.market_root,
    ] {
        if account.owner != release.program_id || account.provenance.release_key != release.key() {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    if !frame.base.collateral_token_program.executable
        || frame.payer.owner != solana_sdk_ids::system_program::ID
        || frame.neutral_sink.owner != solana_sdk_ids::system_program::ID
        || !frame.neutral_sink.data.is_empty()
        || frame.rent_sysvar.address != solana_sdk_ids::sysvar::rent::ID
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    let realm = RealmAccount::decode(&frame.base.realm.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let profile = ProfileAccount::decode(&frame.base.profile.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let collateral_policy = CollateralPolicyV2::decode(&frame.base.collateral_policy.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_binding = MarketBindingV5::decode(&frame.base.market_binding.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_runtime = MarketRuntimeV3AccountV1::decode(&frame.base.market_runtime.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_instance = MarketInstancePreimageV2::decode(&frame.base.market_instance.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_instance_id = market_instance
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let hoard = HoardV2::decode(&frame.base.hoard.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let claim_ledger = ClaimLedgerV3::decode(&frame.base.claim_ledger.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let resolution = ResolutionV5::decode(&frame.base.resolution.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let policy = FractionalPolicyV3::decode(&frame.base.fractional_policy.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let ledger = FractionalLedgerV1::decode(&frame.base.fractional_ledger.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let credit = FractionalCreditV2::decode(&frame.credit.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let root = MarketLifecycleRootAccountV3::decode(&frame.market_root.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let collateral_release_id = collateral_release
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    let credit_pda = Address::find_program_address(
        &[
            FRACTIONAL_CREDIT_PDA_PREFIX,
            &frame.base.fractional_policy.address.to_bytes(),
            &credit.claimant.bytes(),
        ],
        &release.program_id,
    );
    let root_pda = Address::find_program_address(
        &[
            b"dc:market-lifecycle-root:v1",
            &market_instance_id.bytes(),
            &policy.domain_generation.to_le_bytes(),
        ],
        &release.program_id,
    );
    let required_credit_lamports = credit
        .rent
        .refundable_live_principal
        .checked_add(credit.rent.permanent_tombstone_principal)
        .and_then(|value| value.checked_add(credit.rent.donation_floor))
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let root_binding = root.state.binding_ref();
    if realm.profile != profile.profile
        || realm.realm != profile.realm
        || profile.collateral_policy_id.bytes()
            != collateral_policy
                .id()
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
                .bytes()
        || collateral_release_id.bytes() != collateral_policy.adapter_release.bytes()
        || collateral_release.token_program.bytes() != collateral_policy.token_program.bytes()
        || frame.base.collateral_token_program.address.to_bytes()
            != collateral_policy.token_program.bytes()
        || market_binding.base().base().market.bytes() != frame.base.market_runtime.address.to_bytes()
        || market_binding.base().base().market_instance_v2_id.bytes() != market_instance_id.bytes()
        || market_runtime.market_binding.bytes() != frame.base.market_binding.address.to_bytes()
        || market_runtime.market_instance_v2_id.bytes() != market_instance_id.bytes()
        || hoard.market_instance_id.bytes() != market_instance_id.bytes()
        || claim_ledger.market_instance_id.bytes() != market_instance_id.bytes()
        || resolution.facts.market_instance_id.bytes() != market_instance_id.bytes()
        || policy.market_instance.bytes() != market_instance_id.bytes()
        || ledger.policy_account.bytes() != frame.base.fractional_policy.address.to_bytes()
        || ledger.claim_ledger_account.bytes() != frame.base.claim_ledger.address.to_bytes()
        || claim_ledger.fractional_policy_id.bytes()
            != frame.base.fractional_policy.address.to_bytes()
        || claim_ledger.fractional_ledger_account.bytes()
            != frame.base.fractional_ledger.address.to_bytes()
        || claim_ledger.next_fractional_sequence != ledger.next_sequence
        || credit.policy_account.bytes() != frame.base.fractional_policy.address.to_bytes()
        || credit.ledger_account.bytes() != frame.base.fractional_ledger.address.to_bytes()
        || credit.market_instance.bytes() != market_instance_id.bytes()
        || credit.resolution_account != policy.resolution_account
        || credit.resolution_data_id != policy.resolution_data_id
        || credit.domain_generation != policy.domain_generation
        || credit.numerator != 0
        || frame.credit.address != credit_pda.0
        || credit.stored_bump != credit_pda.1
        || frame.credit.lamports < required_credit_lamports
        || frame.payer.address.to_bytes() != credit.rent.payer.bytes()
        || frame.market_root.address != root_pda.0
        || root.stored_bump != root_pda.1
        || root_binding.market_instance_id.bytes() != market_instance_id.bytes()
        || root_binding.generation != policy.domain_generation
        || root.state.phase() != MarketLifecyclePhaseV3::Active
        || frame.neutral_sink.address.to_bytes()
            != root.state.capital().neutral_lamport_sink.bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let claimant = Address::new_from_array(credit.claimant.bytes());
    let payload = FractionalCloseCreditIntentV1 {
        expected_ledger_sequence: ledger.next_sequence,
        expected_credit_sequence: credit.next_sequence,
        claimant: Identity32V1::new(claimant.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?,
        credit_account: Identity32V1::new(frame.credit.address.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?,
    }
    .encode()
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let addresses = [
        claimant,
        frame.base.realm.address,
        frame.base.profile.address,
        frame.base.collateral_policy.address,
        frame.base.collateral_token_program.address,
        frame.base.market_binding.address,
        frame.base.market_runtime.address,
        frame.base.market_instance.address,
        frame.base.hoard.address,
        frame.base.claim_ledger.address,
        frame.base.resolution.address,
        frame.base.fractional_policy.address,
        frame.base.fractional_ledger.address,
        frame.credit.address,
        frame.payer.address,
        frame.market_root.address,
        frame.neutral_sink.address,
        frame.rent_sysvar.address,
    ];
    let labels = [
        "claimant", "realm", "profile", "collateral-policy", "collateral-token-program",
        "market-binding-v2", "market-runtime-v3", "market-instance-preimage-v2", "hoard-v2",
        "claim-ledger-v3", "resolution-v5", "fractional-policy-v3", "fractional-ledger-v1",
        "fractional-credit-v2", "credit-rent-payer", "market-lifecycle-root-v2", "neutral-sink",
        "rent-sysvar",
    ];
    let payer_alias = claimant == frame.payer.address;
    let mut metas = Vec::with_capacity(addresses.len());
    let mut account_roles = Vec::with_capacity(addresses.len());
    for (index, address) in addresses.iter().copied().enumerate() {
        let writable = matches!(index, 9 | 12 | 13 | 14 | 16) || (index == 0 && payer_alias);
        let signer = index == 0 || (index == 14 && payer_alias);
        metas.push(if writable {
            AccountMeta::new(address, signer)
        } else {
            AccountMeta::new_readonly(address, signer)
        });
        account_roles.push(CanonicalAccountRoleV1 {
            label: labels[index],
            address,
            writable,
            signer,
        });
    }
    let principal = credit
        .rent
        .refundable_live_principal
        .checked_add(credit.rent.permanent_tombstone_principal)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let equation = ExactEquation {
        name: "chain-derived zero-credit rent split".into(),
        unit: IntegerUnit::Lamports,
        left: u128::from(frame.credit.lamports),
        right: u128::from(principal)
            .checked_add(u128::from(frame.credit.lamports - principal))
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?,
    };
    let draft = crate::transaction_builder::OwnedInstructionDraft::enabled_fractional_close_zero_credit_v1(
        crate::transaction_builder::SemanticOwner {
            package: "clutch-fractional-redemption-runtime".into(),
            schema: "fractional-redemption/79/1/8/close-zero-credit".into(),
            release_sha256: release.elf_sha256,
        },
        release.program_id,
        metas,
        vec![equation],
        ledger.next_sequence,
        &payload,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let unsigned_transaction = builder
        .build_atomic(&[draft])
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let mut authority_accounts = base[..12].to_vec();
    authority_accounts.extend(extras);
    let authority_state_sha256 = fractional_authority_state_id_v1(&authority_accounts, release);
    let cursor = ResumableWorkflowCursor {
        workflow_id,
        lane: WorkflowLane::FractionalRedemption,
        generation: policy.domain_generation,
        position: WorkflowPosition {
            phase: u16::from(action.tag()),
            item: ledger.next_sequence,
        },
        observed_state_sha256: authority_state_sha256,
    };
    let planned = PlannedWorkflowNode {
        manifest_sha256: release.release_manifest_sha256,
        cursor,
        coordinate: CanonicalActionCoordinate::FractionalRedemption(action),
        unsigned_transaction,
        reload_authoritative_accounts: true,
    };
    validate_unsigned_fractional_plan(coordinate, builder.payer(), &account_roles, &planned)?;
    let release_key = release.key();
    let driver_account_slot = frame.credit.provenance.slot;
    let draft_id = action_material_id(
        &release_key,
        &release_key,
        release.release_manifest_sha256,
        release.capability_profile_id,
        coordinate,
        frame.credit.address,
        driver_account_slot,
        cursor,
        authority_state_sha256,
        freshness,
        builder.payer(),
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key: release_key.clone(),
        driver_release_key: release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        variant: None,
        driver_account: frame.credit.address,
        driver_account_slot,
        cursor,
        authority_state_sha256,
        freshness,
        fee_payer: builder.payer(),
        account_roles,
        planned,
        draft_id,
    })
}

fn fractional_authority_state_id_v1(
    accounts: &[&ObservedRpcAccount],
    release: &IndexedProgramRelease,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator/fractional-chain-frame/v1\0");
    hash.update(release.program_id.to_bytes());
    hash.update(release.elf_sha256);
    hash.update(release.release_manifest_sha256);
    for account in accounts {
        hash.update(account.address.to_bytes());
        hash.update(account.owner.to_bytes());
        hash.update(account.lamports.to_le_bytes());
        hash.update([u8::from(account.executable)]);
        hash.update(account.provenance.slot.to_le_bytes());
        hash.update(account.provenance.receive_sequence.to_le_bytes());
        hash.update(Sha256::digest(&account.data));
    }
    hash.finalize().into()
}

fn validate_unsigned_fractional_plan(
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
            if binding.family == ExtensionFamily::FractionalRedemption
                && binding.local_action == coordinate.local_action
                && matches!(
                    binding.central_action,
                    Some(ExtensionAction::FractionalRedemption(action))
                        if action.tag() == coordinate.local_action
                )
    );
    if transaction.flows != [ProtocolFlow::FractionalRedemption]
        || transaction.actions.len() != 1
        || transaction.required_signers != expected_signers
        || transaction.runtime_admissions != [RuntimeAdmission::ReleaseBoundEnabled]
        || !binding_matches
        || transaction.message_version != TransactionMessageVersionV1::Legacy
        || transaction.exact_equations.is_empty()
        || transaction.serialized_transaction.is_empty()
        || transaction.has_recent_blockhash
        || transaction.signed
        || transaction.submitted
        || !planned.reload_authoritative_accounts
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(())
}

const DEALER_RUNTIME_LIVENESS_POLICY_PDA_DOMAIN_V1: &[u8] =
    b"dc-dealer-runtime-liveness-policy-v1";
const DEALER_RUNTIME_LIVENESS_ACCOUNT_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-live-account-v1";
const PRODUCT_MARKET_LIFECYCLE_ROOT_PDA_DOMAIN_V1: &[u8] = b"dc:market-lifecycle-root:v1";
const PRODUCT_SERIES_MARKET_LINK_PDA_DOMAIN_V1: &[u8] = b"dc:series-market-link:v1";

#[derive(Clone, Copy, Debug)]
struct DerivedDealerTerminalV1 {
    variant: CanonicalIntentVariantV1,
    generation: u64,
    replay_ordinal: u64,
    liveness_call_ordinal: u32,
    keeper_payment_lamports: u64,
    collateral_selection_receipt_id: [u8; 32],
}

/// Construct one unsigned current Dealer action-25 terminal cut from a single
/// finalized hostile account frame. The canonical tail body selects target 8
/// (live facility a6/v2) or target 9 (unused 0xbc/v1); callers supply neither
/// that discriminator nor replay/liveness semantic fields.
#[allow(clippy::too_many_arguments)]
pub fn construct_dealer_terminal_action_material_v1(
    release: &IndexedProgramRelease,
    collateral: StructuredCollateralCatalogEntryV1<'_>,
    builder: &ProtocolTransactionBuilder,
    workflow_id: [u8; 32],
    freshness: ActionFreshnessBoundaryV1,
    accounts: &[StructuredChainAccountV1<'_>],
    lookup_table: &StructuredAddressLookupTableV1,
) -> Result<CanonicalActionMaterialV1> {
    release
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    collateral
        .program()
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    freshness.validate()?;
    if workflow_id == [0; 32]
        || builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
        || collateral.artifact_owner() != release.program_id
        || lookup_table.observed_slot > freshness.observed_slot
        || lookup_table.cluster_key.trim().is_empty()
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    let variant = match accounts.len() {
        41 => CanonicalIntentVariantV1::DealerRetireActiveFacilityCredit,
        38 => CanonicalIntentVariantV1::DealerRetireUnusedFutureCredit,
        _ => return Err(CanonicalActionMaterialErrorV1::InvalidChainState),
    };
    let coordinate = variant.coordinate();
    if release.enabled_intents.binary_search(&coordinate).is_ok()
        || release.enabled_intent_variants.binary_search(&variant).is_err()
    {
        return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
    }
    if accounts.iter().any(|account| {
        account.address == Address::default()
            || account.observed_slot == 0
            || account.observed_slot > freshness.observed_slot
            || account.present.is_some_and(|present| {
                present.provenance.commitment != RpcCommitment::Finalized
                    || present.provenance.cluster_key != lookup_table.cluster_key
            })
    }) || accounts[15].present.is_some()
        || accounts
            .iter()
            .enumerate()
            .any(|(index, account)| index != 15 && account.present.is_none())
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    if builder.payer() != accounts[0].address {
        return Err(CanonicalActionMaterialErrorV1::FeePayerMismatch);
    }

    let derived = derive_dealer_terminal_v1(release, collateral, accounts, variant)?;
    let mut payload = [0u8; 40];
    payload[0..8].copy_from_slice(&derived.generation.to_le_bytes());
    payload[8..16].copy_from_slice(&derived.replay_ordinal.to_le_bytes());
    payload[16] = derived.variant.payload_discriminator();
    payload[24..28].copy_from_slice(&derived.liveness_call_ordinal.to_le_bytes());
    payload[32..40].copy_from_slice(&derived.keeper_payment_lamports.to_le_bytes());

    let mut metas = Vec::with_capacity(accounts.len());
    let mut account_roles = Vec::with_capacity(accounts.len());
    let mut index = 0usize;
    while index < accounts.len() {
        let spec = crate::transaction_builder::dealer_terminal_account_spec_v1(variant, index)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        metas.push(if spec.writable {
            AccountMeta::new(accounts[index].address, spec.signer)
        } else {
            AccountMeta::new_readonly(accounts[index].address, spec.signer)
        });
        account_roles.push(CanonicalAccountRoleV1 {
            label: spec.label,
            address: accounts[index].address,
            writable: spec.writable,
            signer: spec.signer,
        });
        index += 1;
    }
    let draft = crate::transaction_builder::OwnedInstructionDraft::enabled_dealer_terminal_retire_v1(
        crate::transaction_builder::SemanticOwner {
            package: "clutch-dealer-runtime-contract".into(),
            schema: "dragons-clutch/dealer-terminal-retire/action25/targets8-9/v1".into(),
            release_sha256: release.elf_sha256,
        },
        release.program_id,
        metas,
        vec![ExactEquation {
            name: "chain-derived Dealer retirement keeper payment".into(),
            unit: IntegerUnit::Lamports,
            left: u128::from(derived.keeper_payment_lamports),
            right: u128::from(derived.keeper_payment_lamports),
        }],
        variant,
        derived.replay_ordinal,
        &payload,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let unsigned_transaction = builder
        .build_exact_v0(
            draft,
            lookup_table.table.clone(),
            lookup_table.observed_slot,
            lookup_table.state_sha256,
        )
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let authority_state_sha256 = dealer_terminal_authority_state_id_v1(
        accounts,
        lookup_table.state_sha256,
        derived.collateral_selection_receipt_id,
        variant,
    );
    let cursor = ResumableWorkflowCursor {
        workflow_id,
        lane: WorkflowLane::RecoveryRetirement,
        generation: derived.generation,
        position: WorkflowPosition {
            phase: u16::from(
                clutch_solana_layout::registry::DealerFacilityAction::Retire.tag(),
            ),
            item: u64::from(variant.payload_discriminator()),
        },
        observed_state_sha256: authority_state_sha256,
    };
    let planned = PlannedWorkflowNode {
        manifest_sha256: release.release_manifest_sha256,
        cursor,
        coordinate: CanonicalActionCoordinate::DealerFacility {
            action: clutch_solana_layout::registry::DealerFacilityAction::Retire,
            payload_discriminator: variant.payload_discriminator(),
        },
        unsigned_transaction,
        reload_authoritative_accounts: true,
    };
    validate_unsigned_dealer_terminal_plan_v1(
        coordinate,
        variant,
        builder.payer(),
        &account_roles,
        &planned,
    )?;
    let release_key = release.key();
    let draft_id = action_material_id(
        &release_key,
        &release_key,
        release.release_manifest_sha256,
        release.capability_profile_id,
        coordinate,
        accounts[2].address,
        accounts[2].observed_slot,
        cursor,
        authority_state_sha256,
        freshness,
        builder.payer(),
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key: release_key.clone(),
        driver_release_key: release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        variant: Some(variant),
        driver_account: accounts[2].address,
        driver_account_slot: accounts[2].observed_slot,
        cursor,
        authority_state_sha256,
        freshness,
        fee_payer: builder.payer(),
        account_roles,
        planned,
        draft_id,
    })
}

fn decode_current_dealer_material_body_v1<T: DealerFixedCodec>(
    program: Address,
    account: StructuredChainAccountV1<'_>,
    tag: u8,
    version: u8,
    account_bytes: usize,
) -> Result<(u8, T)> {
    let data = account.data()?;
    if account.owner()? != program
        || account.executable()
        || data.len() != account_bytes
        || account_bytes != 8usize.saturating_add(T::ENCODED_LEN)
        || data[0] != tag
        || data[1] != version
        || data[3..8].iter().any(|byte| *byte != 0)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let value = <T as DealerFixedCodec>::decode(&data[8..])
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    Ok((data[2], value))
}

fn require_pda_v1(
    actual: Address,
    program: Address,
    seeds: &[&[u8]],
    stored_bump: u8,
) -> Result<()> {
    let expected = Address::find_program_address(seeds, &program);
    if actual != expected.0 || stored_bump != expected.1 {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok(())
}

fn dealer_rent_is_covered_v1(
    rent: DeletableRentOwnerV1,
    account: Option<&ObservedRpcAccount>,
) -> bool {
    account.is_some_and(|account| {
        rent.refundable_principal
            .checked_add(rent.donation_floor)
            .is_some_and(|minimum| account.lamports >= minimum)
    })
}

fn require_dealer_terminal_system_roles_v1(
    accounts: &[StructuredChainAccountV1<'_>],
) -> Result<()> {
    if accounts[21].address != solana_sdk_ids::sysvar::clock::ID
        || accounts[22].address != solana_sdk_ids::sysvar::rent::ID
        || accounts[23].address != solana_sdk_ids::system_program::ID
        || !accounts[23].executable()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    for index in [0usize, 16, 17, 18, 19, 20] {
        if !system_identity_is_valid(accounts[index]) {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    if accounts[20].address == accounts[17].address
        || accounts[20].address == accounts[18].address
        || accounts[20].address == accounts[19].address
        || accounts[20].address == accounts[16].address
        || accounts[20].address == accounts[0].address
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok(())
}

fn dealer_terminal_authority_state_id_v1(
    accounts: &[StructuredChainAccountV1<'_>],
    lookup_table_state_sha256: [u8; 32],
    collateral_catalog_receipt_id: [u8; 32],
    variant: CanonicalIntentVariantV1,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator/dealer-terminal-authority/v1\0");
    hash.update([variant.payload_discriminator()]);
    hash.update(lookup_table_state_sha256);
    hash.update(collateral_catalog_receipt_id);
    hash.update(structured_chain_state_id(accounts));
    hash.finalize().into()
}

fn validate_unsigned_dealer_terminal_plan_v1(
    coordinate: CanonicalIntentCoordinate,
    variant: CanonicalIntentVariantV1,
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
            if binding.family == ExtensionFamily::Dealer
                && binding.local_action == coordinate.local_action
                && Some(binding.family_status) == ExtensionFamily::Dealer.allocation_status()
                && matches!(
                    binding.central_action,
                    Some(ExtensionAction::DealerFacility(action))
                        if action
                            == clutch_solana_layout::registry::DealerFacilityAction::Retire
                )
    );
    let coordinate_matches = matches!(
        planned.coordinate,
        CanonicalActionCoordinate::DealerFacility { action, payload_discriminator }
            if action == clutch_solana_layout::registry::DealerFacilityAction::Retire
                && payload_discriminator == variant.payload_discriminator()
    );
    if !coordinate_matches
        || transaction.flows != [ProtocolFlow::DealerFacilityTerminal]
        || transaction.actions.len() != 1
        || transaction.semantic_owners.len() != 1
        || !binding_matches
        || transaction.runtime_admissions
            != [RuntimeAdmission::PayloadVariantReleaseBoundEnabled]
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

#[derive(Clone, Copy, Debug)]
struct DealerTerminalPositionV1 {
    replay: DealerFacilityReplayV1,
    binding: FacilityPositionBindingV2,
}

#[allow(clippy::too_many_arguments)]
fn authenticate_dealer_terminal_position_replay_v1(
    program: Address,
    accounts: &[StructuredChainAccountV1<'_>],
    policy: &DealerPolicyV1,
    policy_id: DealerId,
    state: &DealerStateV3,
) -> Result<DealerTerminalPositionV1> {
    let base = state.base;
    let position = PositionAccountV3::decode(accounts[3].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let replay = <DealerFacilityReplayV1 as DealerFixedCodec>::decode(accounts[4].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let binding = FacilityPositionBindingV2 {
        facility_id: base.facility_id,
        policy_id,
        market_instance_v2_id: policy.market_instance_v2_id,
        collateral_policy_id: DealerId::from_bytes(position.collateral_policy_id().bytes()),
        collateral_release_id: DealerId::from_bytes(position.collateral_release_id().bytes()),
        dealer_state_account_id: DealerId::from_bytes(accounts[2].address.to_bytes()),
        initial_position_generation: 1,
    };
    let binding_id = binding
        .binding_id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let purpose = [u8::from(PositionPurposeV3::DealerFacility)];
    let position_pda = Address::find_program_address(
        &[
            POSITION_V3_PDA_PREFIX,
            &policy.market_instance_v2_id.bytes(),
            &base.facility_id.bytes(),
            &purpose,
            &binding_id.bytes(),
        ],
        &program,
    );
    let replay_pda = Address::find_program_address(
        &[
            PURPOSE_REPLAY_V3_PDA_PREFIX,
            &accounts[3].address.to_bytes(),
            &purpose,
            &binding_id.bytes(),
        ],
        &program,
    );
    let position_id = position
        .semantic_id(&OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let replay_rent = replay.rent();
    if accounts[3].owner()? != program
        || accounts[4].owner()? != program
        || accounts[3].executable()
        || accounts[4].executable()
        || position.lifecycle() != PositionLifecycleV3::CloseRequested
        || position.purpose() != PositionPurposeV3::DealerFacility
        || position.market_instance_id().bytes() != policy.market_instance_v2_id.bytes()
        || position.realm_id().bytes() != policy.realm_id.bytes()
        || position.owner().bytes() != base.facility_id.bytes()
        || position.controller().bytes() != accounts[2].address.to_bytes()
        || position.replay_account().bytes() != accounts[4].address.to_bytes()
        || position.purpose_binding_id().bytes() != binding_id.bytes()
        || position.outcome_count() != policy.outcome_count
        || position.generation() != base.generation
        || position.cash_atoms() != 0
        || position.reserved_cash_atoms() != 0
        || position.native_eggs() != [0; clutch_retirement::MAX_OUTCOMES]
        || position.outstanding_reservations() != 0
        || position_id.bytes() != base.facility_position_id.bytes()
        || accounts[3].address != position_pda.0
        || position.stored_bump() != position_pda.1
        || !position_rent_is_covered(position.rent(), accounts[3].present)
        || position.rent().payer.bytes() != accounts[17].address.to_bytes()
        || replay.lifecycle() != ReplayV3Lifecycle::Live
        || replay.facility_position_account_id().bytes() != accounts[3].address.to_bytes()
        || replay.replay_account_id().bytes() != accounts[4].address.to_bytes()
        || replay.facility_position_binding_id() != binding_id
        || replay.position_generation() != base.generation
        || replay.next_transition_ordinal() == 0
        || accounts[4].address != replay_pda.0
        || replay.pda_seeds().stored_bump() != replay_pda.1
        || !rent_is_covered(replay_rent, accounts[4].present)
        || replay_rent.payer().bytes() != accounts[18].address.to_bytes()
        || base.facility_position_account_id.bytes() != accounts[3].address.to_bytes()
        || base.facility_replay_account_id.bytes() != accounts[4].address.to_bytes()
        || base.facility_position_binding_id != binding_id
        || base.children.facility_positions != 1
        || base.children.facility_replays != 1
        || base.children.live_lp_positions != 0
        || base.children.unclaimed_lp_positions != 0
        || base.children.exit_tickets != 0
        || base.children.epoch_bindings != 0
        || base.children.leases != 0
        || base.children.settlement_pots != 0
        || base.children.terminal_allocations != 0
        || base.children.claim_work != 0
        || policy.neutral_sink.bytes() != accounts[20].address.to_bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok(DealerTerminalPositionV1 {
        replay,
        binding,
    })
}

fn authenticate_dealer_terminal_liveness_v1(
    program: Address,
    accounts: &[StructuredChainAccountV1<'_>],
    state: &DealerStateV3,
    policy: &DealerPolicyV1,
    dependency: &DealerFundedDependenciesV2,
    schedule: &DealerLivenessScheduleV1,
    binding: &FacilityPositionBindingV2,
) -> Result<(DealerRuntimeLivenessBindingV1, RuntimeCompartmentV1)> {
    if accounts[7].owner()? != program
        || accounts[7].executable()
        || accounts[7].data()?.len()
            != clutch_liveness::runtime_v1::RUNTIME_LIVENESS_POLICY_BYTES_V1
        || dependency.bindings.runtime_liveness_program_id.bytes() != program.to_bytes()
        || dependency
            .bindings
            .runtime_liveness_policy_account_id
            .bytes()
            != accounts[7].address.to_bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let runtime_policy = RuntimeLivenessPolicyV1::decode(accounts[7].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let runtime_policy_id = dealer_runtime_liveness_policy_id_v1(runtime_policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let policy_pda = Address::find_program_address(
        &[
            DEALER_RUNTIME_LIVENESS_POLICY_PDA_DOMAIN_V1,
            &runtime_policy_id.bytes(),
        ],
        &program,
    );
    if accounts[7].address != policy_pda.0
        || runtime_policy.policy_id.bytes()
            != dependency.bindings.runtime_liveness_policy_id.bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    let first = RuntimeCompartmentV1::decode(accounts[8].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let mut compartments = [first; 7];
    let mut index = 0usize;
    while index < compartments.len() {
        let account_index = 8usize
            .checked_add(index)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let account = accounts[account_index];
        let compartment = RuntimeCompartmentV1::decode(account.data()?)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        let kind_seed = [match compartment.kind {
            RuntimeCompartmentKindV1::Source => 0,
            RuntimeCompartmentKindV1::Candidate => 1,
            RuntimeCompartmentKindV1::Clearing => 2,
            RuntimeCompartmentKindV1::Settlement => 3,
            RuntimeCompartmentKindV1::Resolution => 4,
            RuntimeCompartmentKindV1::Retirement => 5,
            RuntimeCompartmentKindV1::Recovery => 6,
        }];
        let expected = Address::find_program_address(
            &[
                DEALER_RUNTIME_LIVENESS_ACCOUNT_PDA_DOMAIN_V1,
                &state.base.facility_id.bytes(),
                &kind_seed,
            ],
            &program,
        );
        let expected_balance = compartment
            .expected_account_balance_lamports()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
        if account.owner()? != program
            || account.executable()
            || account.data()?.len()
                != clutch_liveness::runtime_v1::RUNTIME_LIVENESS_ACCOUNT_BYTES_V1
            || compartment.kind.index() != index
            || compartment.identity.account_id.bytes() != account.address.to_bytes()
            || account.address != expected.0
            || account
                .present
                .is_none_or(|present| present.lamports < expected_balance)
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        compartments[index] = compartment;
        index += 1;
    }
    let runtime = DealerRuntimeLivenessBindingV1::from_canonical(&runtime_policy, &compartments)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    dependency
        .validate_live_bindings_v4(binding, policy, schedule, &runtime)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    index = 1;
    while index < compartments.len() {
        let compartment = match index {
            1 => DealerLivenessCompartmentV1::Candidate,
            2 => DealerLivenessCompartmentV1::Clearing,
            3 => DealerLivenessCompartmentV1::Settlement,
            4 => DealerLivenessCompartmentV1::Resolution,
            5 => DealerLivenessCompartmentV1::Retirement,
            6 => DealerLivenessCompartmentV1::Recovery,
            _ => return Err(CanonicalActionMaterialErrorV1::InvalidChainState),
        };
        if runtime.owner(compartment).bytes() != accounts[2].address.to_bytes()
            || runtime.receipt_program_id(compartment).bytes() != program.to_bytes()
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
        index += 1;
    }
    let retirement = compartments[DealerLivenessCompartmentV1::Retirement.index()];
    if retirement.identity.payer.bytes() != accounts[16].address.to_bytes()
        || retirement.identity.neutral_sink.bytes() != accounts[20].address.to_bytes()
        || runtime.neutral_sink().bytes() != accounts[20].address.to_bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok((runtime, retirement))
}

#[derive(Clone, Copy, Debug)]
struct DealerTerminalMarketV1 {
    market_instance_id: [u8; 32],
    realm_id: [u8; 32],
    collateral_policy_id: [u8; 32],
    collateral_release_id: [u8; 32],
    native_claim_basis_id: [u8; 32],
    resolution_account: [u8; 32],
    resolution_semantic_id: [u8; 32],
    resolution_data_id: [u8; 32],
    generation: u64,
    outcome_count: u8,
    collateral_selection_receipt_id: [u8; 32],
}

fn authenticate_dealer_terminal_product_and_value_v1(
    release: &IndexedProgramRelease,
    collateral: StructuredCollateralCatalogEntryV1<'_>,
    accounts: &[StructuredChainAccountV1<'_>],
    dealer_policy: &DealerPolicyV1,
    obligation: &DealerSeriesObligationBindingV3,
    variant: CanonicalIntentVariantV1,
) -> Result<DealerTerminalMarketV1> {
    let program = release.program_id;
    let root_index = accounts.len().checked_sub(2).ok_or(
        CanonicalActionMaterialErrorV1::InvalidChainState,
    )?;
    let link_index = accounts.len().checked_sub(1).ok_or(
        CanonicalActionMaterialErrorV1::InvalidChainState,
    )?;
    let root = MarketLifecycleRootAccountV3::decode(accounts[root_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let root_state = &root.state;
    let root_binding = root_state.binding();
    let root_binding_id = root_binding
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let root_pda = Address::find_program_address(
        &[
            PRODUCT_MARKET_LIFECYCLE_ROOT_PDA_DOMAIN_V1,
            &root_binding.market_instance_id.bytes(),
            &root_binding.generation.to_le_bytes(),
        ],
        &program,
    );
    let link = SeriesMarketLinkAccountV3::decode(accounts[link_index].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let link_state = link.state;
    let link_binding = link_state.binding();
    let link_pda = Address::find_program_address(
        &[
            PRODUCT_SERIES_MARKET_LINK_PDA_DOMAIN_V1,
            &link_binding.series_plan_id.bytes(),
            &link_binding.ordinal.to_le_bytes(),
        ],
        &program,
    );
    let link_floor = link_state
        .rent_principal_lamports()
        .checked_add(link_state.current_donation_lamports())
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if accounts[root_index].owner()? != program
        || accounts[link_index].owner()? != program
        || accounts[root_index].executable()
        || accounts[link_index].executable()
        || accounts[root_index].address.to_bytes()
            != obligation.key.product_market_root_account_id.bytes()
        || accounts[link_index].address.to_bytes()
            != obligation.key.series_market_link_account_id.bytes()
        || accounts[root_index].address != root_pda.0
        || root.stored_bump != root_pda.1
        || accounts[root_index]
            .present
            .is_none_or(|value| value.lamports < root.rent_principal_lamports)
        || root_state.phase() != MarketLifecyclePhaseV3::Active
        || root_state
            .product_families()
            .family(MarketFamilyV1::Dealer)
            .status()
            != MarketFamilyStatusV1::Live
        || root_state
            .product_families()
            .family(MarketFamilyV1::Dealer)
            .counts()
            .live
            == 0
        || root_binding_id.bytes() != obligation.key.product_market_binding_id.bytes()
        || root_binding.market_instance_id.bytes() != obligation.key.market_instance_v2_id.bytes()
        || root_binding.generation != obligation.key.product_generation
        || root_binding.outcome_count != dealer_policy.outcome_count
        || root_binding.native_claim_basis_id.bytes() != dealer_policy.claim_basis_id.bytes()
        || root_state.resolution_semantic_id() == ContentId::ZERO
        || root_state.resolution_data_id() == ContentId::ZERO
        || root_state.resolution_activation_receipt_id() == ContentId::ZERO
        || accounts[link_index].address != link_pda.0
        || link.stored_bump != link_pda.1
        || accounts[link_index]
            .present
            .is_none_or(|value| value.lamports < link_floor)
        || link_state.phase() != SeriesMarketLinkPhaseV3::Active
        || link_binding.market_instance_id != root_binding.market_instance_id
        || link_binding.market_root_account_id.bytes()
            != accounts[root_index].address.to_bytes()
        || link_binding.market_binding_id != root_binding_id
        || link_binding.generation != root_binding.generation
        || link_binding.series_plan_id.bytes() != obligation.key.series_plan_v5_id.bytes()
        || link_binding.ordinal != obligation.key.series_ordinal
        || link_binding.compiler_bundle_id.content_id().bytes()
            != obligation.key.compiler_bundle_v7_id.bytes()
        || link_binding.attachment_plan_id.content_id().bytes()
            != obligation.key.attachment_plan_v6_id.bytes()
        || link_binding.capability_profile_id != root_binding.capability_profile_id
        || link_binding.rent_refund_owner.bytes() != obligation.rent.payer.bytes()
        || link_binding.neutral_lamport_sink.bytes() != obligation.rent.neutral_sink.bytes()
        || link_binding.obligation_configuration_id.content_id().bytes()
            != obligation.key.obligation_configuration_v3_id.bytes()
        || link_state.obligation_status(SeriesLinkObligationV3::Dealer)
            != SeriesLinkObligationStatusV3::Live
        || link_state
            .obligation_admission_receipt_id(SeriesLinkObligationV3::Dealer)
            .bytes()
            != obligation.admission_projection_id.bytes()
        || link_state.transition_sequence() < obligation.admission_link_transition_sequence
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    for index in [25usize, 26, 27, 30, 31, 32, 33, 34] {
        if accounts[index].owner()? != program || accounts[index].executable() {
            return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
        }
    }
    let collateral_program = accounts[28]
        .present
        .and_then(CurrentCollateralExecutableAccountViewV1::from_finalized)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let collateral_programdata = accounts[29]
        .present
        .and_then(CurrentCollateralExecutableAccountViewV1::from_finalized)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    collateral
        .reauthenticate_executable(collateral_program, collateral_programdata)
        .map_err(|_| CanonicalActionMaterialErrorV1::ReleaseMismatch)?;
    let realm = RealmAccount::decode(accounts[25].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let collateral_profile = ProfileAccount::decode(accounts[26].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let collateral_policy = CollateralPolicyV2::decode(accounts[27].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let collateral_policy_id = collateral_policy
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let selected_collateral = collateral
        .select_for(release, collateral_policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::ReleaseMismatch)?;
    let collateral_release_id = collateral
        .adapter()
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    let realm_pda = Address::find_program_address(
        &[REALM_PDA_SEED_V1, &realm.realm.bytes()],
        &program,
    );
    let collateral_profile_pda = Address::find_program_address(
        &[
            PROFILE_PDA_SEED_V1,
            &realm.realm.bytes(),
            &collateral_profile.profile.bytes(),
        ],
        &program,
    );
    let collateral_policy_pda = Address::find_program_address(
        &[
            COLLATERAL_POLICY_PDA_SEED_V1,
            &collateral_profile.profile.bytes(),
            &collateral_policy_id.bytes(),
        ],
        &program,
    );
    if accounts[28].address != collateral.program().program_id
        || accounts[29].address != collateral.program().program_data
        || realm.profile != collateral_profile.profile
        || realm.realm != collateral_profile.realm
        || collateral_profile.collateral_policy_id.bytes() != collateral_policy_id.bytes()
        || collateral_profile.adapter_release_id.bytes() != collateral_release_id.bytes()
        || collateral_policy.token_program.bytes() != accounts[28].address.to_bytes()
        || accounts[25].address != realm_pda.0
        || realm.stored_bump != realm_pda.1
        || accounts[26].address != collateral_profile_pda.0
        || accounts[27].address != collateral_policy_pda.0
        || realm.realm.bytes() != dealer_policy.realm_id.bytes()
        || collateral_profile.profile.bytes() != dealer_policy.profile_id.bytes()
        || collateral_policy.mint.bytes() != dealer_policy.collateral_mint.bytes()
        || collateral_policy.token_program.bytes() != dealer_policy.token_program.bytes()
        || root_binding.realm_id.bytes() != realm.realm.bytes()
        || root_binding.collateral_profile_id.bytes() != collateral_profile.profile.bytes()
        || root_binding.collateral_policy_id.bytes() != collateral_policy_id.bytes()
        || root_binding.collateral_release_id.bytes() != collateral_release_id.bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    let market_binding = MarketBindingV2::decode(accounts[30].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_base = market_binding.base();
    let market_runtime = MarketRuntimeV3AccountV1::decode(accounts[31].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let binding_pda = Address::find_program_address(
        &[
            MARKET_BINDING_SEED_DOMAIN_V1,
            &market_base.market_instance_v2_id.bytes(),
        ],
        &program,
    );
    let runtime_pda = Address::find_program_address(
        &[MARKET_RUNTIME_SEED_DOMAIN_V1, &accounts[30].address.to_bytes()],
        &program,
    );
    let runtime_floor = market_runtime
        .rent
        .refundable_principal
        .checked_add(market_runtime.rent.donation_floor)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market = MarketInstancePreimageV2::decode(accounts[32].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let market_id = market
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    verify_product_artifact(
        program,
        accounts[32],
        ArtifactKind::MarketInstancePreimageV2,
        market_id.bytes(),
    )?;
    let hoard = HoardV2::decode(accounts[33].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let claim = ClaimLedgerV3::decode(accounts[34].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let hoard_pda = Address::find_program_address(
        &[HOARD_V2_PDA_SEED_V1, &market_id.bytes()],
        &program,
    );
    let claim_pda = Address::find_program_address(
        &[CLAIM_LEDGER_V3_PDA_SEED_V1, &market_id.bytes()],
        &program,
    );
    let hoard_authority = Address::find_program_address(
        &[HOARD_AUTHORITY_V2_PDA_SEED_V1, &market_id.bytes()],
        &program,
    );
    let hoard_token = Address::find_program_address(
        &[HOARD_TOKEN_V2_PDA_SEED_V1, &market_id.bytes()],
        &program,
    );
    if accounts[30].address != binding_pda.0
        || market_base.stored_bump != binding_pda.1
        || accounts[31].address != runtime_pda.0
        || market_runtime.stored_bump != runtime_pda.1
        || accounts[31]
            .present
            .is_none_or(|value| value.lamports < runtime_floor)
        || market_base.market.bytes() != accounts[31].address.to_bytes()
        || market_runtime.market_binding.bytes() != accounts[30].address.to_bytes()
        || market_runtime.market_instance_v2_id != market_base.market_instance_v2_id
        || market_id.bytes() != market_base.market_instance_v2_id.bytes()
        || market_id.bytes() != dealer_policy.market_instance_v2_id.bytes()
        || market_base.market_genesis_profile_v2_id.bytes()
            != market.market_genesis_profile_id.bytes()
        || market_base.market_instance_v2_id.bytes() != root_binding.market_instance_id.bytes()
        || market_base.series_plan_v5_id.bytes() != link_binding.series_plan_id.bytes()
        || market_base.series_funding_terms_v2_id.bytes() != link_binding.funding_terms_id.bytes()
        || market_base.native_claim_basis_id.bytes() != root_binding.native_claim_basis_id.bytes()
        || market_base.outcome_count != root_binding.outcome_count
        || accounts[33].address != hoard_pda.0
        || hoard.stored_bump != hoard_pda.1
        || accounts[34].address != claim_pda.0
        || claim.stored_bump != claim_pda.1
        || hoard.market_instance_id.bytes() != market_id.bytes()
        || hoard.realm_id.bytes() != realm.realm.bytes()
        || hoard.profile_id.bytes() != collateral_profile.profile.bytes()
        || hoard.collateral_policy_id.bytes() != collateral_policy_id.bytes()
        || hoard.collateral_release_id.bytes() != collateral_release_id.bytes()
        || hoard.authority.bytes() != hoard_authority.0.to_bytes()
        || hoard.token_account.bytes() != hoard_token.0.to_bytes()
        || hoard.collateral_cap_atoms != market.collateral_cap
        || hoard.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || claim.market_instance_id != hoard.market_instance_id
        || claim.realm_id != hoard.realm_id
        || claim.native_claim_basis_id.bytes() != root_binding.native_claim_basis_id.bytes()
        || claim.resolution_account.bytes() != root_binding.resolution_account_id.bytes()
        || claim.lifecycle != hoard.lifecycle
        || claim.outcome_count != hoard.outcome_count
        || claim.outcome_count != dealer_policy.outcome_count
        || !rent_is_covered(hoard.rent, accounts[33].present)
        || !rent_is_covered(claim.rent, accounts[34].present)
        || (variant == CanonicalIntentVariantV1::DealerRetireActiveFacilityCredit
            && claim.fractional_ledger_account.is_zero())
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok(DealerTerminalMarketV1 {
        market_instance_id: market_id.bytes(),
        realm_id: realm.realm.bytes(),
        collateral_policy_id: collateral_policy_id.bytes(),
        collateral_release_id: collateral_release_id.bytes(),
        native_claim_basis_id: root_binding.native_claim_basis_id.bytes(),
        resolution_account: root_binding.resolution_account_id.bytes(),
        resolution_semantic_id: root_state.resolution_semantic_id().bytes(),
        resolution_data_id: root_state.resolution_data_id().bytes(),
        generation: root_binding.generation,
        outcome_count: root_binding.outcome_count,
        collateral_selection_receipt_id: selected_collateral.receipt_id(),
    })
}

fn authenticate_dealer_terminal_credit_tail_v1(
    program: Address,
    accounts: &[StructuredChainAccountV1<'_>],
    variant: CanonicalIntentVariantV1,
    policy: &DealerPolicyV1,
    state: &DealerStateV3,
    obligation: &DealerSeriesObligationBindingV3,
    market: DealerTerminalMarketV1,
) -> Result<()> {
    let claim = ClaimLedgerV3::decode(accounts[34].data()?)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    match variant {
        CanonicalIntentVariantV1::DealerRetireActiveFacilityCredit => {
            let resolution = ResolutionV5::decode(accounts[35].data()?)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let resolution_pda = Address::find_program_address(
                &[b"dc:resolution:v5", &market.market_instance_id],
                &program,
            );
            let resolution_semantic_id = resolution
                .semantic_id(&OperatorSha256V1)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let resolution_data_id = resolution
                .data_id(clutch_collateral_adapter_v2::Id::from_bytes(
                    accounts[35].address.to_bytes(),
                ))
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let fractional_policy = FractionalPolicyV3::decode(accounts[36].data()?)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let fractional_ledger = FractionalLedgerV1::decode(accounts[37].data()?)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let facility_credit = FractionalCreditV2::decode(accounts[38].data()?)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let policy_pda = Address::find_program_address(
                &[
                    FRACTIONAL_POLICY_PDA_PREFIX,
                    &fractional_policy.market_instance.bytes(),
                    &fractional_policy.resolution_account.bytes(),
                ],
                &program,
            );
            let ledger_pda = Address::find_program_address(
                &[FRACTIONAL_LEDGER_PDA_PREFIX, &accounts[36].address.to_bytes()],
                &program,
            );
            let credit_pda = Address::find_program_address(
                &[
                    FRACTIONAL_CREDIT_PDA_PREFIX,
                    &accounts[36].address.to_bytes(),
                    &state.base.facility_id.bytes(),
                ],
                &program,
            );
            let payout = clutch_fractional_redemption_runtime::PayoutVectorV1::from_resolution_v5(
                resolution,
            )
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let fractional_policy_account = clutch_retirement::Identity32V1::new(
                accounts[36].address.to_bytes(),
            )
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            let fractional_ledger_account = clutch_retirement::Identity32V1::new(
                accounts[37].address.to_bytes(),
            )
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            facility_credit
                .validate_with(
                    fractional_policy_account,
                    fractional_policy,
                    fractional_ledger_account,
                    fractional_ledger,
                    payout,
                )
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            if accounts[35].owner()? != program
                || accounts[36].owner()? != program
                || accounts[37].owner()? != program
                || accounts[38].owner()? != program
                || accounts[35].executable()
                || accounts[36].executable()
                || accounts[37].executable()
                || accounts[38].executable()
                || accounts[35].address != resolution_pda.0
                || resolution.stored_bump != resolution_pda.1
                || !rent_is_covered(resolution.rent, accounts[35].present)
                || resolution.state != ResolutionStateV5::Finalized
                || resolution.facts.market_instance_id.bytes() != market.market_instance_id
                || resolution.facts.native_claim_basis_id.bytes()
                    != market.native_claim_basis_id
                || resolution.facts.generation != market.generation
                || resolution.facts.outcome_count != market.outcome_count
                || resolution_semantic_id.bytes() != market.resolution_semantic_id
                || resolution_data_id.bytes() != market.resolution_data_id
                || accounts[35].address.to_bytes() != market.resolution_account
                || accounts[36].address != policy_pda.0
                || fractional_policy.stored_bump != policy_pda.1
                || !rent_is_covered(fractional_policy.rent, accounts[36].present)
                || fractional_policy.market_instance.bytes() != market.market_instance_id
                || fractional_policy.resolution_account.bytes() != market.resolution_account
                || fractional_policy.resolution_data_id.bytes() != market.resolution_data_id
                || fractional_policy.realm.bytes() != market.realm_id
                || fractional_policy.collateral_policy.bytes() != market.collateral_policy_id
                || fractional_policy.collateral_release.bytes() != market.collateral_release_id
                || fractional_policy.domain_generation != market.generation
                || fractional_policy.outcome_count != market.outcome_count
                || accounts[37].address != ledger_pda.0
                || fractional_ledger.stored_bump != ledger_pda.1
                || !rent_is_covered(fractional_ledger.rent, accounts[37].present)
                || fractional_ledger.policy_account.bytes() != accounts[36].address.to_bytes()
                || fractional_ledger.claim_ledger_account.bytes()
                    != accounts[34].address.to_bytes()
                || fractional_ledger.domain_generation != market.generation
                || fractional_ledger.active_credit_accounts == 0
                || claim.fractional_policy_id.bytes() != accounts[36].address.to_bytes()
                || claim.fractional_ledger_account.bytes() != accounts[37].address.to_bytes()
                || claim.resolution_account.bytes() != accounts[35].address.to_bytes()
                || claim.next_fractional_sequence != fractional_ledger.next_sequence
                || accounts[38].address != credit_pda.0
                || facility_credit.stored_bump != credit_pda.1
                || !position_rent_is_covered(facility_credit.rent, accounts[38].present)
                || facility_credit.rent.payer.bytes() != accounts[19].address.to_bytes()
                || facility_credit.policy_account.bytes() != accounts[36].address.to_bytes()
                || facility_credit.ledger_account.bytes() != accounts[37].address.to_bytes()
                || facility_credit.market_instance.bytes() != market.market_instance_id
                || facility_credit.resolution_account.bytes() != market.resolution_account
                || facility_credit.resolution_data_id.bytes() != market.resolution_data_id
                || facility_credit.claimant.bytes() != state.base.facility_id.bytes()
                || facility_credit.domain_generation != market.generation
                || facility_credit.numerator != 0
            {
                return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
            }
        }
        CanonicalIntentVariantV1::DealerRetireUnusedFutureCredit => {
            let (bump, funding) =
                decode_current_dealer_material_body_v1::<DealerFutureCreditFundingV1>(
                    program,
                    accounts[35],
                    clutch_solana_layout::registry::DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_TAG,
                    clutch_solana_layout::registry::DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_VERSION,
                    clutch_solana_layout::registry::DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_BYTES,
                )?;
            require_pda_v1(
                accounts[35].address,
                program,
                &[
                    DEALER_FUTURE_CREDIT_FUNDING_PDA_DOMAIN_V1,
                    &state.base.facility_id.bytes(),
                ],
                bump,
            )?;
            let minimum = funding
                .minimum_balance_lamports()
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
            if funding.funding_account_id.bytes() != accounts[35].address.to_bytes()
                || accounts[35]
                    .present
                    .is_none_or(|account| account.lamports < minimum)
                || funding.policy_id != state.base.policy_id
                || funding.facility_id != state.base.facility_id
                || funding.market_instance_v2_id.bytes() != market.market_instance_id
                || funding.realm_id.bytes() != market.realm_id
                || funding.collateral_policy_id.bytes() != market.collateral_policy_id
                || funding.collateral_release_id.bytes() != market.collateral_release_id
                || funding.dealer_state_account_id.bytes() != accounts[2].address.to_bytes()
                || funding.facility_position_account_id.bytes()
                    != accounts[3].address.to_bytes()
                || funding.facility_position_binding_id
                    != state.base.facility_position_binding_id
                || funding.dealer_replay_account_id.bytes() != accounts[4].address.to_bytes()
                || funding.refund_owner.bytes() != accounts[19].address.to_bytes()
                || funding.neutral_sink.bytes() != accounts[20].address.to_bytes()
                || funding.neutral_sink != obligation.rent.neutral_sink
                || funding.founding_generation > state.base.generation
            {
                return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
            }
        }
    }
    if policy.market_instance_v2_id.bytes() != market.market_instance_id
        || policy.realm_id.bytes() != market.realm_id
        || policy.claim_basis_id.bytes() != market.native_claim_basis_id
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok(())
}

fn derive_dealer_terminal_v1(
    release: &IndexedProgramRelease,
    collateral: StructuredCollateralCatalogEntryV1<'_>,
    accounts: &[StructuredChainAccountV1<'_>],
    variant: CanonicalIntentVariantV1,
) -> Result<DerivedDealerTerminalV1> {
    let program = release.program_id;
    require_dealer_terminal_system_roles_v1(accounts)?;
    let policy_data = accounts[1].data()?;
    if accounts[1].owner()? != program
        || accounts[1].executable()
        || policy_data.len() != clutch_solana_layout::registry::DEALER_POLICY_ACCOUNT_BYTES
        || policy_data[0] != clutch_solana_layout::registry::DEALER_POLICY_ACCOUNT_TAG
        || policy_data[1] != clutch_solana_layout::registry::DEALER_POLICY_ACCOUNT_VERSION
        || policy_data[3..8].iter().any(|byte| *byte != 0)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let policy_principal = read_u64_le(policy_data, 40)?;
    if policy_principal == 0
        || accounts[1]
            .present
            .is_none_or(|account| account.lamports < policy_principal)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let policy_body = &policy_data
        [clutch_solana_layout::registry::DEALER_POLICY_ACCOUNT_HEADER_BYTES..];
    let policy = <DealerPolicyV1 as DealerFixedCodec>::decode(policy_body)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let mut policy_hash = Sha256::new();
    policy_hash.update(DEALER_POLICY_CONTENT_DOMAIN_V1);
    policy_hash.update(policy_body);
    let policy_id = DealerId::from_bytes(policy_hash.finalize().into());
    if policy
        .policy_id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
        != policy_id
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    require_pda_v1(
        accounts[1].address,
        program,
        &[DEALER_POLICY_PDA_DOMAIN_V1, &policy_id.bytes()],
        policy_data[2],
    )?;

    let (state_bump, state) = decode_current_dealer_material_body_v1::<DealerStateV3>(
        program,
        accounts[2],
        clutch_solana_layout::registry::DEALER_STATE_V3_ACCOUNT_TAG,
        clutch_solana_layout::registry::DEALER_STATE_V3_ACCOUNT_VERSION,
        clutch_solana_layout::registry::DEALER_STATE_V3_ACCOUNT_BYTES,
    )?;
    let state_base = state.base;
    require_pda_v1(
        accounts[2].address,
        program,
        &[DEALER_STATE_PDA_DOMAIN_V2, &state_base.facility_id.bytes()],
        state_bump,
    )?;
    let state_floor = state_base
        .rent
        .refundable_live_principal
        .checked_add(state_base.rent.permanent_tombstone_principal)
        .and_then(|value| value.checked_add(state_base.rent.donation_floor))
        .and_then(|value| value.checked_add(state.product_upgrade_rent.refundable_principal))
        .and_then(|value| value.checked_add(state.product_upgrade_rent.donation_floor))
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if state_base.phase != DealerPhaseV2::Retiring
        || state_base.policy_id != policy_id
        || state.series_obligation_children != 1
        || accounts[2]
            .present
            .is_none_or(|account| account.lamports < state_floor)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    let (obligation_bump, obligation) =
        decode_current_dealer_material_body_v1::<DealerSeriesObligationBindingV3>(
            program,
            accounts[24],
            clutch_solana_layout::registry::DEALER_SERIES_OBLIGATION_ACCOUNT_TAG,
            clutch_solana_layout::registry::DEALER_SERIES_OBLIGATION_ACCOUNT_VERSION_V3,
            clutch_solana_layout::registry::DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES_V3,
        )?;
    let obligation_id = obligation
        .binding_id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    require_pda_v1(
        accounts[24].address,
        program,
        &[
            DEALER_SERIES_OBLIGATION_PDA_DOMAIN_V1,
            &state_base.facility_id.bytes(),
        ],
        obligation_bump,
    )?;
    let obligation_floor = obligation
        .rent
        .refundable_principal
        .checked_add(obligation.rent.donation_floor)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if obligation.phase != DealerSeriesObligationPhaseV1::Live
        || obligation.key.binding_account_id.bytes() != accounts[24].address.to_bytes()
        || obligation.key.policy_id != policy_id
        || obligation.key.facility_id != state_base.facility_id
        || obligation.key.dealer_state_account_id.bytes() != accounts[2].address.to_bytes()
        || obligation.key.facility_position_binding_id != state_base.facility_position_binding_id
        || state.series_obligation_binding_account_id.bytes() != accounts[24].address.to_bytes()
        || state.series_obligation_binding_id != obligation_id
        || obligation.rent.payer.bytes() != accounts[19].address.to_bytes()
        || obligation.rent.neutral_sink.bytes() != accounts[20].address.to_bytes()
        || accounts[24]
            .present
            .is_none_or(|account| account.lamports < obligation_floor)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    let position = authenticate_dealer_terminal_position_replay_v1(
        program, accounts, &policy, policy_id, &state,
    )?;
    let (dependency_bump, dependency) =
        decode_current_dealer_material_body_v1::<DealerFundedDependenciesV2>(
            program,
            accounts[5],
            clutch_solana_layout::registry::DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG,
            clutch_solana_layout::registry::DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION,
            clutch_solana_layout::registry::DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES,
        )?;
    let (schedule_bump, schedule) =
        decode_current_dealer_material_body_v1::<DealerLivenessScheduleV1>(
            program,
            accounts[6],
            clutch_solana_layout::registry::DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG,
            clutch_solana_layout::registry::DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION,
            clutch_solana_layout::registry::DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES,
        )?;
    require_pda_v1(
        accounts[5].address,
        program,
        &[
            DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V2,
            &state_base.facility_id.bytes(),
        ],
        dependency_bump,
    )?;
    let schedule_id = schedule
        .schedule_id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
        .untyped();
    require_pda_v1(
        accounts[6].address,
        program,
        &[DEALER_LIVENESS_SCHEDULE_PDA_DOMAIN_V1, &schedule_id.bytes()],
        schedule_bump,
    )?;
    if dependency.bindings.facility_id != state_base.facility_id
        || dependency.bindings.policy_id != policy_id
        || dependency.facility_position_binding_id != state_base.facility_position_binding_id
        || dependency.bindings.liveness_schedule_id != schedule_id
        || schedule_id != policy.liveness_policy_id
        || dependency.rent.neutral_sink != policy.neutral_sink
        || !dealer_rent_is_covered_v1(dependency.rent, accounts[5].present)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    let (runtime_binding, retirement) = authenticate_dealer_terminal_liveness_v1(
        program,
        accounts,
        &state,
        &policy,
        &dependency,
        &schedule,
        &position.binding,
    )?;
    let call_ordinal = retirement
        .completed_calls
        .checked_add(1)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let action_index = DealerLivenessScheduleV1::action_index(DealerRuntimeActionV1::Retire);
    let payment = schedule.reward_lamports[action_index];
    if retirement.phase != RuntimeCompartmentPhaseV1::Active
        || retirement.remaining_calls == 0
        || payment == 0
        || payment > retirement.maximum_lamports_per_call
        || payment > retirement.remaining_work_lamports
        || u64::from(call_ordinal) > schedule.maximum_calls[action_index]
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    let market = authenticate_dealer_terminal_product_and_value_v1(
        release,
        collateral,
        accounts,
        &policy,
        &obligation,
        variant,
    )?;
    if position.binding.collateral_policy_id.bytes() != market.collateral_policy_id
        || position.binding.collateral_release_id.bytes() != market.collateral_release_id
        || position.binding.market_instance_v2_id.bytes() != market.market_instance_id
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    authenticate_dealer_terminal_credit_tail_v1(
        program,
        accounts,
        variant,
        &policy,
        &state,
        &obligation,
        market,
    )?;

    let receipt = DealerActionReceiptV1 {
        policy_id,
        facility_id: state_base.facility_id,
        dealer_state_account_id: DealerId::from_bytes(accounts[2].address.to_bytes()),
        liveness_schedule_id: schedule_id,
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding
            .account_id(DealerLivenessCompartmentV1::Retirement),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Retirement),
        quote_schedule_id: runtime_binding
            .quote_schedule_id(DealerLivenessCompartmentV1::Retirement),
        receipt_account_id: DealerId::from_bytes(accounts[15].address.to_bytes()),
        receipt_program_id: DealerId::from_bytes(program.to_bytes()),
        keeper: DealerId::from_bytes(accounts[0].address.to_bytes()),
        replay_account_id: DealerId::from_bytes(accounts[4].address.to_bytes()),
        action: DealerRuntimeActionV1::Retire,
        compartment: DealerLivenessCompartmentV1::Retirement,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Retirement),
        facility_generation: state_base.generation,
        call_ordinal,
        call_ceiling_lamports: payment,
        keeper_payment_lamports: payment,
        expected_replay_ordinal: position.replay.next_transition_ordinal(),
        // Rent amount is intentionally absent from the receipt-slot identity.
        // The onchain handler recomputes the exact live Rent principal before
        // allocating this already-derived absent PDA.
        rent: DeletableRentOwnerV1 {
            payer: DealerId::from_bytes(accounts[0].address.to_bytes()),
            neutral_sink: policy.neutral_sink,
            refundable_principal: 1,
            donation_floor: 0,
        },
    };
    receipt
        .authorization(&schedule, &runtime_binding, &retirement)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let slot = receipt
        .receipt_slot_id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let expected_receipt = Address::find_program_address(
        &[DEALER_ACTION_RECEIPT_PDA_DOMAIN_V1, &slot.bytes()],
        &program,
    );
    if accounts[15].address != expected_receipt.0 {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    Ok(DerivedDealerTerminalV1 {
        variant,
        generation: state_base.generation,
        replay_ordinal: position.replay.next_transition_ordinal(),
        liveness_call_ordinal: call_ordinal,
        keeper_payment_lamports: payment,
        collateral_selection_receipt_id: market.collateral_selection_receipt_id,
    })
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
        variant: None,
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

pub(crate) const fn direct_selection_action(action: DirectMarketAction) -> &'static str {
    match action {
        DirectMarketAction::InitializeMarket => "initialize-direct-market",
        DirectMarketAction::AdmitOrder => "admit-direct-order",
        DirectMarketAction::CancelOrder => "cancel-direct-order",
        DirectMarketAction::FreezeBook => "freeze-direct-book",
        DirectMarketAction::SubmitCandidate => "submit-direct-candidate",
        DirectMarketAction::BeginVerification => "begin-direct-verification",
        DirectMarketAction::VerifyCandidate => "verify-direct-candidate",
        DirectMarketAction::FinalizeSelection => "finalize-direct-selection",
        DirectMarketAction::SettlePair => "settle-direct-pair",
        DirectMarketAction::LapseEmpty => "lapse-empty-direct-market",
        DirectMarketAction::LapseUnselected => "lapse-unselected-direct-market",
        DirectMarketAction::LapseSelected => "lapse-selected-direct-market",
        DirectMarketAction::RetireTerminal => "retire-direct-terminal",
    }
}

pub(crate) fn direct_action_from_selection(selection: &str) -> Option<DirectMarketAction> {
    match selection {
        "initialize-direct-market" => Some(DirectMarketAction::InitializeMarket),
        "admit-direct-order" => Some(DirectMarketAction::AdmitOrder),
        "cancel-direct-order" => Some(DirectMarketAction::CancelOrder),
        "freeze-direct-book" => Some(DirectMarketAction::FreezeBook),
        "submit-direct-candidate" => Some(DirectMarketAction::SubmitCandidate),
        "begin-direct-verification" => Some(DirectMarketAction::BeginVerification),
        "verify-direct-candidate" => Some(DirectMarketAction::VerifyCandidate),
        "finalize-direct-selection" => Some(DirectMarketAction::FinalizeSelection),
        "settle-direct-pair" => Some(DirectMarketAction::SettlePair),
        "lapse-empty-direct-market" => Some(DirectMarketAction::LapseEmpty),
        "lapse-unselected-direct-market" => Some(DirectMarketAction::LapseUnselected),
        "lapse-selected-direct-market" => Some(DirectMarketAction::LapseSelected),
        "retire-direct-terminal" => Some(DirectMarketAction::RetireTerminal),
        _ => None,
    }
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

fn validate_unsigned_direct_plan(
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
                && binding.family_status == AllocationStatus::Frozen
                && matches!(
                    binding.central_action,
                    Some(ExtensionAction::DirectMarket(action))
                        if action.tag() == coordinate.local_action
                )
    );
    if !matches!(
        planned.coordinate,
        CanonicalActionCoordinate::Direct(action)
            if action.tag() == coordinate.local_action
    )
        || transaction.flows != [ProtocolFlow::DirectMarketV1]
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

pub(crate) const fn direct_role_label_v1(role: DirectAccountRoleV1) -> &'static str {
    use DirectAccountRoleV1 as Role;
    match role {
        Role::ProductRoot => "product-root-v3",
        Role::ProductReplay => "product-market-replay-v2",
        Role::ProductDirectGlobalLiveness => "product-direct-global-liveness-v2",
        Role::FounderSeriesLink => "series-market-link-v3",
        Role::WritableFounderSeriesLink => "writable-series-market-link-v3",
        Role::SeriesFunding => "series-funding-v5",
        Role::SeriesRegistry => "series-registry-v4",
        Role::RegistryProgram => "registry-program",
        Role::RegistryProgramData => "registry-programdata",
        Role::RegistryReleaseArtifact => "registry-release-artifact",
        Role::CapabilityProfileArtifact => "capability-profile-artifact",
        Role::SourceRelease => "source-release-v3",
        Role::CompilerBundle => "compiler-bundle-v7",
        Role::SeriesPlan => "series-plan-v5",
        Role::FundingTerms => "funding-terms-v2",
        Role::SourceTemplate => "source-template-v4",
        Role::RecoveryPolicy => "recovery-policy-v1",
        Role::FundingQuote => "funding-quote-v6",
        Role::AttachmentPlan => "attachment-plan-v6",
        Role::FamilyCapabilityPolicy => "market-family-capability-policy-v1",
        Role::LivenessSource => "direct-liveness-source",
        Role::LivenessCandidate => "direct-liveness-candidate",
        Role::LivenessClearing => "direct-liveness-clearing",
        Role::LivenessSettlement => "direct-liveness-settlement",
        Role::LivenessResolution => "direct-liveness-resolution",
        Role::LivenessRetirement => "direct-liveness-retirement",
        Role::LivenessRecovery => "direct-liveness-recovery",
        Role::DirectRoot => "direct-root",
        Role::DirectReplay => "direct-replay",
        Role::FreshReservation => "fresh-direct-reservation",
        Role::WritableReservation => "writable-direct-reservation",
        Role::ReadonlyReservation => "readonly-direct-reservation",
        Role::FreshSelection => "fresh-direct-selection",
        Role::Selection => "direct-selection",
        Role::DirectResolution => "direct-resolution-v5",
        Role::ActorPayer => "actor-payer",
        Role::Position => "position-v3",
        Role::PositionReplay => "position-replay-v3",
        Role::Realm => "realm",
        Role::CollateralProfile => "collateral-profile",
        Role::CollateralPolicy => "collateral-policy",
        Role::TokenProgram => "token-2022-program",
        Role::GeneralMarketBinding => "general-market-binding-v5",
        Role::GeneralMarketRuntime => "general-market-runtime-v3",
        Role::MarketInstance => "market-instance-v2",
        Role::MarketGenesis => "market-genesis-v2",
        Role::SystemProgram => "system-program",
        Role::RentSysvar => "rent-sysvar",
        Role::ClockSysvar => "clock-sysvar",
        Role::PriceGrid => "price-grid",
        Role::NativeClaimBasis => "native-claim-basis",
        Role::PriceMeasurePolicy => "price-measure-policy",
        Role::BatchPolicy => "batch-policy",
        Role::RevenuePolicyRecord => "revenue-policy-record",
        Role::RevenuePolicy => "revenue-policy",
        Role::NeutralSink => "neutral-sink",
        Role::BondRefundOwner => "candidate-bond-refund-owner",
        Role::RentRefundOwner => "rent-refund-owner",
        Role::LivenessPolicy => "candidate-liveness-policy",
        Role::Candidate => "candidate-liveness-compartment",
        Role::Keeper => "keeper",
        Role::CandidatePayer => "candidate-liveness-payer",
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
        crate::workflow_graph::WorkflowLane::FractionalRedemption => 6,
        crate::workflow_graph::WorkflowLane::FailureRecovery => 7,
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

    #[test]
    fn fractional_lifecycle_material_derives_omitted_graph_accounts() {
        let source = include_str!("action_material.rs");
        let start = source
            .find("pub fn construct_fractional_lifecycle_material_v1(")
            .unwrap();
        let end = source[start..]
            .find("pub fn construct_fractional_bearer_material_v1(")
            .unwrap()
            + start;
        let body = &source[start..end];
        assert!(body.contains("frame.accounts.len() != 32 + outcomes"));
        assert!(body.contains("OUTCOME_CUSTODY_PDA_DOMAIN_V1"));
        assert!(body.contains("revenue.treasury_position_account()"));
        assert!(body.contains("PURPOSE_REPLAY_V3_PDA_PREFIX"));
        assert!(!body.contains("foundation-treasury"));
    }

    #[test]
    fn structured_operator_material_uses_only_current_product_lineage() {
        let source = include_str!("action_material.rs");
        let create_start = source.find("fn derive_structured_create_v1(").unwrap();
        let create_end = source[create_start..]
            .find("fn structured_runtime_addresses(")
            .unwrap()
            + create_start;
        let create = &source[create_start..create_end];
        let current_join_start = source.find("fn validate_current_product_join(").unwrap();
        let current_join_end = source[current_join_start..]
            .find("fn decode_current_dealer_material_body_v1")
            .unwrap()
            + current_join_start;
        let current_join = &source[current_join_start..current_join_end];
        for body in [create, current_join] {
            assert!(body.contains("SeriesMarketLinkAccountV3"));
            assert!(body.contains("SeriesMarketLinkPhaseV3"));
            assert!(body.contains("validate_structured_product_root_v3("));
            assert!(!body.contains("SeriesMarketLinkAccountV2"));
            assert!(!body.contains("SeriesMarketLinkPhaseV2"));
        }
        for required in [
            "CompiledProductSeriesBundleV7",
            "SeriesAttachmentPlanV6",
            "ArtifactKind::CompiledProductSeriesBundleV7",
            "ArtifactKind::SeriesAttachmentPlanV6",
        ] {
            assert!(create.contains(required));
            assert!(current_join.contains(required));
        }
        for withdrawn in [
            "CompiledProductSeriesBundleV6",
            "SeriesAttachmentPlanV5",
            "ArtifactKind::CompiledProductSeriesBundleV6",
            "ArtifactKind::SeriesAttachmentPlanV5",
        ] {
            assert!(!create.contains(withdrawn));
            assert!(!current_join.contains(withdrawn));
        }
    }

    fn address(byte: u8) -> Address {
        Address::new_from_array([byte; 32])
    }

    fn direct_accounts(roles: &[DirectAccountRoleV1]) -> Vec<DirectNamedAccountV1> {
        roles
            .iter()
            .enumerate()
            .map(|(index, role)| {
                DirectNamedAccountV1::new(
                    *role,
                    address(u8::try_from(index + 1).unwrap()),
                )
                .unwrap()
            })
            .collect()
    }

    #[test]
    fn direct_verification_frame_is_exact_and_reordered_suffix_refuses() {
        use DirectAccountRoleV1 as Role;
        let roles = [
            Role::DirectRoot,
            Role::DirectReplay,
            Role::Selection,
            Role::ClockSysvar,
            Role::LivenessPolicy,
            Role::Candidate,
            Role::Keeper,
            Role::CandidatePayer,
        ];
        assert!(DirectActionAccountsV1::new(
            DirectMarketAction::BeginVerification,
            direct_accounts(&roles),
        )
        .is_ok());
        let mut reordered = roles;
        reordered.swap(4, 5);
        assert_eq!(
            DirectActionAccountsV1::new(
                DirectMarketAction::BeginVerification,
                direct_accounts(&reordered),
            ),
            Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        );
    }

    #[test]
    fn direct_settlement_frame_requires_two_endpoints_and_fee_owner_tuple() {
        use DirectAccountRoleV1 as Role;
        let mut roles = vec![
            Role::DirectRoot,
            Role::DirectReplay,
            Role::Selection,
            Role::Realm,
            Role::CollateralProfile,
            Role::CollateralPolicy,
            Role::TokenProgram,
            Role::GeneralMarketBinding,
            Role::GeneralMarketRuntime,
            Role::MarketInstance,
            Role::MarketGenesis,
            Role::ClockSysvar,
        ];
        roles.extend([
            Role::WritableReservation,
            Role::Position,
            Role::PositionReplay,
            Role::WritableReservation,
            Role::Position,
            Role::PositionReplay,
            Role::BatchPolicy,
            Role::RevenuePolicyRecord,
            Role::RevenuePolicy,
            Role::Position,
            Role::PositionReplay,
            Role::BondRefundOwner,
            Role::LivenessPolicy,
            Role::Candidate,
            Role::Keeper,
            Role::CandidatePayer,
        ]);
        assert!(DirectActionAccountsV1::new(
            DirectMarketAction::SettlePair,
            direct_accounts(&roles),
        )
        .is_ok());
        roles.remove(15);
        roles.remove(14);
        roles.remove(13);
        assert_eq!(
            DirectActionAccountsV1::new(
                DirectMarketAction::SettlePair,
                direct_accounts(&roles),
            ),
            Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        );
    }

    #[test]
    fn direct_product_foundation_requires_exact_current_frame() {
        use DirectAccountRoleV1 as Role;
        let roles = [
            Role::GeneralMarketBinding,
            Role::GeneralMarketRuntime,
            Role::ProductRoot,
            Role::WritableFounderSeriesLink,
            Role::SeriesFunding,
            Role::SeriesRegistry,
            Role::RegistryProgram,
            Role::RegistryProgramData,
            Role::RegistryReleaseArtifact,
            Role::CapabilityProfileArtifact,
            Role::SourceRelease,
            Role::CompilerBundle,
            Role::MarketInstance,
            Role::Realm,
            Role::RevenuePolicyRecord,
            Role::RevenuePolicy,
            Role::SeriesPlan,
            Role::FundingTerms,
            Role::SourceTemplate,
            Role::NativeClaimBasis,
            Role::RecoveryPolicy,
            Role::PriceMeasurePolicy,
            Role::MarketGenesis,
            Role::FundingQuote,
            Role::AttachmentPlan,
            Role::ProductReplay,
            Role::FamilyCapabilityPolicy,
            Role::ProductDirectGlobalLiveness,
            Role::LivenessSource,
            Role::LivenessCandidate,
            Role::LivenessClearing,
            Role::LivenessSettlement,
            Role::LivenessResolution,
            Role::LivenessRetirement,
            Role::LivenessRecovery,
            Role::DirectRoot,
            Role::DirectReplay,
            Role::ActorPayer,
            Role::SystemProgram,
            Role::RentSysvar,
            Role::ClockSysvar,
        ];
        assert!(DirectActionAccountsV1::new(
            DirectMarketAction::InitializeMarket,
            direct_accounts(&roles),
        )
        .is_ok());
        let mut reordered = roles;
        reordered.swap(28, 29);
        assert_eq!(
            DirectActionAccountsV1::new(
                DirectMarketAction::InitializeMarket,
                direct_accounts(&reordered),
            ),
            Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        );
    }

    #[test]
    fn direct_retirement_requires_global_liveness_before_candidate_suffix() {
        use DirectAccountRoleV1 as Role;
        let roles = [
            Role::ProductRoot,
            Role::FounderSeriesLink,
            Role::DirectRoot,
            Role::DirectReplay,
            Role::Selection,
            Role::DirectResolution,
            Role::ClockSysvar,
            Role::NeutralSink,
            Role::WritableReservation,
            Role::WritableReservation,
            Role::RentRefundOwner,
            Role::ProductDirectGlobalLiveness,
            Role::LivenessPolicy,
            Role::Candidate,
            Role::Keeper,
            Role::CandidatePayer,
        ];
        assert!(DirectActionAccountsV1::new(
            DirectMarketAction::RetireTerminal,
            direct_accounts(&roles),
        )
        .is_ok());
        let mut missing_global = roles.to_vec();
        missing_global.remove(11);
        assert_eq!(
            DirectActionAccountsV1::new(
                DirectMarketAction::RetireTerminal,
                direct_accounts(&missing_global),
            ),
            Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        );
        let mut substituted = roles;
        substituted[11] = Role::ProductRoot;
        assert_eq!(
            DirectActionAccountsV1::new(
                DirectMarketAction::RetireTerminal,
                direct_accounts(&substituted),
            ),
            Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        );
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
    fn structured_coordinate_requires_wrapper_base_and_token_release_admission() {
        let coordinate = CanonicalIntentCoordinate {
            family_tag: STRUCTURED_CLAIM_FAMILY_TAG,
            family_version: STRUCTURED_CLAIM_FAMILY_VERSION,
            local_action: StructuredClaimActionV1::WrapFull.tag(),
        };
        let present = [coordinate];
        let absent = [];
        assert!(structured_release_intents_joined_v1(
            [&present, &present, &present],
            coordinate,
        ));
        assert!(!structured_release_intents_joined_v1(
            [&present, &absent, &present],
            coordinate,
        ));
        assert!(!structured_release_intents_joined_v1(
            [&present, &present, &absent],
            coordinate,
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
