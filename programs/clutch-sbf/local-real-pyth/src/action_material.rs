//! Opaque, release-authenticated action material for operator projections.
//!
//! Construction accepts typed semantic-owner state and account projections,
//! never browser JSON or caller-authored instruction bytes. The resulting
//! artifact remains unsigned and blockhash-free. It can make an operator
//! control inspectable, but it cannot sign, submit, or predict poststate.

use crate::rpc_index::{CanonicalIntentCoordinate, IndexedProgramRelease};
use crate::operatord::KeeperActionSelection;
use crate::transaction_builder::{
    ExactEquation, IntegerUnit, OwnedInstructionDraft, ProtocolFlow,
    ProtocolTransactionBuilder, RuntimeAdmission, SemanticOwner, UnsignedProtocolTransaction,
};
use crate::workflow_graph::{
    plan_source_crank, CanonicalActionCoordinate, ExplicitOperatorReleaseManifest,
    PlannedWorkflowNode, ResumableWorkflowCursor,
    SourceCrankObservation, SourceWorkflowActionMaterial, WorkflowGraphError,
};
use clutch_solana_layout::registry::{
    AllocationStatus, DirectMarketAction, ExtensionAction, DIRECT_MARKET_FAMILY_TAG,
    DIRECT_MARKET_FAMILY_VERSION, SOURCE_SERIES_FAMILY_TAG, SOURCE_SERIES_FAMILY_VERSION,
};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::AccountMeta;
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
        })
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

/// Closed operator vocabulary for current Direct account roles.
///
/// This is an untrusted projection of onchain state, not an authorization
/// token. It exists to prevent a launcher from inventing positional metas or
/// privileges outside the exact action-specific frame checked again by SBF.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectAccountRoleV1 {
    ProductRoot,
    ProductDirectGlobalLiveness,
    FounderSeriesLink,
    CompilerBundle,
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
                | Self::ProductDirectGlobalLiveness
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
    pub const fn role(self) -> DirectAccountRoleV1 { self.role }

    #[must_use]
    pub const fn address(self) -> Address { self.address }
}

/// Exact action-specific Direct account projection for actions 2 through 13.
///
/// Action 1 deliberately refuses until Product publishes the final callable
/// `0xba` allocation account frame and quote-owned work schedule. Adding it
/// requires extending this closed grammar rather than accepting a generic
/// `Vec<AccountMeta>`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectActionAccountsV1 {
    action: DirectMarketAction,
    accounts: Vec<DirectNamedAccountV1>,
}

impl DirectActionAccountsV1 {
    pub fn new(
        action: DirectMarketAction,
        accounts: Vec<DirectNamedAccountV1>,
    ) -> Result<Self> {
        validate_direct_account_roles_v1(action, &accounts)?;
        Ok(Self { action, accounts })
    }

    #[must_use]
    pub const fn action(&self) -> DirectMarketAction { self.action }

    #[must_use]
    pub fn accounts(&self) -> &[DirectNamedAccountV1] { &self.accounts }

    fn driver_account(&self) -> Result<Address> {
        self.accounts
            .iter()
            .find(|account| account.role == DirectAccountRoleV1::DirectRoot)
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
    release_manifest_sha256: [u8; 32],
    capability_profile_id: [u8; 32],
    coordinate: CanonicalIntentCoordinate,
    driver_account: Address,
    driver_account_slot: u64,
    cursor: ResumableWorkflowCursor,
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
            && selection.release_key == self.release_key
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
    let draft_id = action_material_id(
        &release_key,
        release.release_manifest_sha256,
        release.capability_profile_id,
        coordinate,
        selection.account,
        selection.account_slot,
        selection.cursor,
        freshness,
        builder.payer(),
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        driver_account: selection.account,
        driver_account_slot: selection.account_slot,
        cursor: selection.cursor,
        freshness,
        fee_payer: builder.payer(),
        account_roles,
        planned,
        draft_id,
    })
}

/// Construct one release-admitted Direct action from the closed `80/1`
/// payload codec and exact action-specific account grammar.
///
/// The result remains unsigned and cannot exist for a coordinate absent from
/// the checked release. Action 1 additionally remains structurally unavailable
/// until Product owns the exact `0xba` allocation and quote work schedule.
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
    if material.payload.action() != action
        || material.valid_before_slot != freshness.valid_before_slot
        || material.sequence == 0
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
    let draft_id = action_material_id(
        &release_key,
        release.release_manifest_sha256,
        release.capability_profile_id,
        coordinate,
        selection.account,
        selection.account_slot,
        selection.cursor,
        freshness,
        fee_payer,
        &account_roles,
        &planned.unsigned_transaction,
    );
    Ok(CanonicalActionMaterialV1 {
        release_key,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        coordinate,
        driver_account: selection.account,
        driver_account_slot: selection.account_slot,
        cursor: selection.cursor,
        freshness,
        fee_payer,
        account_roles,
        planned,
        draft_id,
    })
}

fn validate_direct_account_roles_v1(
    action: DirectMarketAction,
    accounts: &[DirectNamedAccountV1],
) -> Result<()> {
    use DirectAccountRoleV1 as Role;
    if accounts.len() > 30 {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    match action {
        DirectMarketAction::InitializeMarket => Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        DirectMarketAction::AdmitOrder => {
            let end = require_direct_roles_v1(accounts, 0, &[
                Role::DirectRoot, Role::DirectReplay, Role::FreshReservation,
                Role::ActorPayer, Role::Position, Role::PositionReplay, Role::Realm,
                Role::CollateralProfile, Role::CollateralPolicy, Role::TokenProgram,
                Role::GeneralMarketBinding, Role::GeneralMarketRuntime,
                Role::MarketInstance, Role::SystemProgram, Role::RentSysvar,
                Role::ClockSysvar, Role::CompilerBundle, Role::MarketGenesis,
                Role::PriceGrid,
            ])?;
            if accounts.len() == end {
                Ok(())
            } else if accounts.len() == end + 1
                && accounts[end].role == Role::ReadonlyReservation
            {
                Ok(())
            } else {
                Err(CanonicalActionMaterialErrorV1::InvalidPlan)
            }
        }
        DirectMarketAction::CancelOrder => require_direct_exact_roles_v1(accounts, &[
            Role::DirectRoot, Role::DirectReplay, Role::WritableReservation,
            Role::ActorPayer, Role::Position, Role::PositionReplay, Role::Realm,
            Role::CollateralProfile, Role::CollateralPolicy, Role::TokenProgram,
            Role::GeneralMarketBinding, Role::GeneralMarketRuntime,
            Role::MarketInstance, Role::MarketGenesis, Role::NeutralSink,
            Role::ClockSysvar,
        ]),
        DirectMarketAction::FreezeBook => {
            let mut index = require_direct_roles_v1(accounts, 0, &[
                Role::DirectRoot, Role::DirectReplay, Role::FreshSelection,
                Role::ActorPayer, Role::SystemProgram, Role::RentSysvar,
                Role::ClockSysvar, Role::CompilerBundle, Role::NativeClaimBasis,
                Role::PriceMeasurePolicy, Role::MarketGenesis, Role::PriceGrid,
            ])?;
            index = consume_direct_roles_v1(accounts, index, Role::ReadonlyReservation, 0, 2)?;
            require_direct_suffix_v1(accounts, index)
        }
        DirectMarketAction::SubmitCandidate => {
            let end = require_direct_roles_v1(accounts, 0, &[
                Role::DirectRoot, Role::DirectReplay, Role::Selection,
                Role::ClockSysvar, Role::ActorPayer, Role::SystemProgram,
            ])?;
            if accounts.len() == end
                || accounts.len() == end + 1
                    && accounts[end].role == Role::BondRefundOwner
            {
                Ok(())
            } else {
                Err(CanonicalActionMaterialErrorV1::InvalidPlan)
            }
        }
        DirectMarketAction::BeginVerification | DirectMarketAction::VerifyCandidate => {
            let index = require_direct_roles_v1(accounts, 0, &[
                Role::DirectRoot, Role::DirectReplay, Role::Selection, Role::ClockSysvar,
            ])?;
            require_direct_suffix_v1(accounts, index)
        }
        DirectMarketAction::FinalizeSelection => validate_direct_finalize_roles_v1(accounts),
        DirectMarketAction::SettlePair => validate_direct_economic_roles_v1(
            accounts,
            true,
            2,
            2,
        ),
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
            let mut index = require_direct_roles_v1(accounts, 0, &[
                Role::ProductRoot, Role::FounderSeriesLink, Role::CompilerBundle,
                Role::DirectRoot, Role::DirectReplay, Role::Selection,
                Role::DirectResolution, Role::ClockSysvar, Role::NeutralSink,
            ])?;
            index = consume_direct_roles_v1(
                accounts,
                index,
                Role::WritableReservation,
                0,
                2,
            )?;
            index = consume_direct_roles_v1(
                accounts,
                index,
                Role::RentRefundOwner,
                1,
                5,
            )?;
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
    let mut index = require_direct_roles_v1(accounts, 0, &[
        Role::DirectRoot, Role::DirectReplay, Role::Selection, Role::ClockSysvar,
    ])?;
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
    let mut index = require_direct_roles_v1(accounts, 0, &[
        Role::DirectRoot, Role::DirectReplay, Role::Selection, Role::Realm,
        Role::CollateralProfile, Role::CollateralPolicy, Role::TokenProgram,
        Role::GeneralMarketBinding, Role::GeneralMarketRuntime, Role::MarketInstance,
        Role::MarketGenesis, Role::ClockSysvar,
    ])?;
    let mut endpoints = 0usize;
    while accounts.get(index).map(|account| account.role) == Some(Role::WritableReservation) {
        index = require_direct_roles_v1(accounts, index, &[
            Role::WritableReservation, Role::Position, Role::PositionReplay,
        ])?;
        endpoints += 1;
        if endpoints > maximum_endpoints {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    if endpoints < minimum_endpoints {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    if fee_bearing {
        index = require_direct_roles_v1(accounts, index, &[
            Role::BatchPolicy, Role::RevenuePolicyRecord, Role::RevenuePolicy,
        ])?;
        if accounts.get(index).map(|account| account.role) == Some(Role::Position) {
            index = require_direct_roles_v1(accounts, index, &[
                Role::Position, Role::PositionReplay,
            ])?;
        }
    }
    index = consume_direct_roles_v1(accounts, index, Role::BondRefundOwner, 0, 3)?;
    require_direct_suffix_v1(accounts, index)
}

fn validate_direct_missed_freeze_roles_v1(accounts: &[DirectNamedAccountV1]) -> Result<()> {
    use DirectAccountRoleV1 as Role;
    let mut index = require_direct_roles_v1(accounts, 0, &[
        Role::DirectRoot, Role::DirectReplay, Role::FreshSelection, Role::ActorPayer,
        Role::SystemProgram, Role::RentSysvar, Role::ClockSysvar, Role::CompilerBundle,
        Role::NativeClaimBasis, Role::PriceMeasurePolicy, Role::MarketGenesis,
        Role::PriceGrid, Role::Realm, Role::CollateralProfile, Role::CollateralPolicy,
        Role::TokenProgram, Role::GeneralMarketBinding, Role::GeneralMarketRuntime,
        Role::MarketInstance,
    ])?;
    let mut endpoints = 0usize;
    while accounts.get(index).map(|account| account.role) == Some(Role::WritableReservation) {
        index = require_direct_roles_v1(accounts, index, &[
            Role::WritableReservation, Role::Position, Role::PositionReplay,
        ])?;
        endpoints += 1;
        if endpoints > 2 {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    require_direct_suffix_v1(accounts, index)
}

fn require_direct_suffix_v1(accounts: &[DirectNamedAccountV1], index: usize) -> Result<()> {
    use DirectAccountRoleV1 as Role;
    let end = require_direct_roles_v1(accounts, index, &[
        Role::LivenessPolicy, Role::Candidate, Role::Keeper, Role::CandidatePayer,
    ])?;
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
                && binding.family_status == AllocationStatus::Frozen
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

#[allow(clippy::too_many_arguments)]
fn action_material_id(
    release_key: &str,
    release_manifest_sha256: [u8; 32],
    capability_profile_id: [u8; 32],
    coordinate: CanonicalIntentCoordinate,
    driver_account: Address,
    driver_account_slot: u64,
    cursor: ResumableWorkflowCursor,
    freshness: ActionFreshnessBoundaryV1,
    fee_payer: Address,
    roles: &[CanonicalAccountRoleV1],
    transaction: &UnsignedProtocolTransaction,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(CANONICAL_ACTION_MATERIAL_SCHEMA_V1.as_bytes());
    hash_text(&mut hash, release_key);
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
        Role::ProductRoot => "product-root",
        Role::ProductDirectGlobalLiveness => "product-direct-global-liveness-v1",
        Role::FounderSeriesLink => "founder-series-link",
        Role::CompilerBundle => "compiler-bundle",
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
        Role::GeneralMarketBinding => "general-market-binding-v3",
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
            serialized_transaction: vec![1, 2, 3],
            has_recent_blockhash: false,
            signed: false,
            submitted: false,
        }
    }

    fn direct_accounts(
        roles: &[DirectAccountRoleV1],
    ) -> Vec<DirectNamedAccountV1> {
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
    fn direct_product_foundation_refuses_until_final_allocation_frame_lands() {
        assert_eq!(
            DirectActionAccountsV1::new(DirectMarketAction::InitializeMarket, vec![]),
            Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        );
    }

    #[test]
    fn direct_retirement_requires_global_liveness_before_candidate_suffix() {
        use DirectAccountRoleV1 as Role;
        let roles = [
            Role::ProductRoot,
            Role::FounderSeriesLink,
            Role::CompilerBundle,
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
        missing_global.remove(12);
        assert_eq!(
            DirectActionAccountsV1::new(
                DirectMarketAction::RetireTerminal,
                direct_accounts(&missing_global),
            ),
            Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        );
        let mut substituted = roles;
        substituted[12] = Role::ProductRoot;
        assert_eq!(
            DirectActionAccountsV1::new(
                DirectMarketAction::RetireTerminal,
                direct_accounts(&substituted),
            ),
            Err(CanonicalActionMaterialErrorV1::InvalidPlan),
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
            [1; 32],
            [2; 32],
            coordinate,
            address(3),
            10,
            cursor(),
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
            [1; 32],
            [2; 32],
            coordinate,
            address(3),
            10,
            cursor(),
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
