//! Opaque, release-authenticated action material for operator projections.
//!
//! Construction accepts typed semantic-owner state and account projections,
//! never browser JSON or caller-authored instruction bytes. The resulting
//! artifact remains unsigned and blockhash-free. It can make an operator
//! control inspectable, but it cannot sign, submit, or predict poststate.

use crate::rpc_index::{
    CanonicalIntentCoordinate, IndexedProgramRelease, ObservedRpcAccount,
    ObservedRpcAccountRemoval, RpcAccountRemovalKind, RpcCommitment,
};
use crate::operatord::KeeperActionSelection;
use crate::transaction_builder::{
    ExactEquation, IntegerUnit, ProtocolFlow, ProtocolTransactionBuilder, RuntimeAdmission,
    SemanticOwner, UnsignedProtocolTransaction,
};
use crate::workflow_graph::{
    plan_source_crank, CanonicalActionCoordinate, ExplicitOperatorReleaseManifest,
    PlannedWorkflowNode, ResumableWorkflowCursor,
    SourceCrankObservation, SourceWorkflowActionMaterial, WorkflowGraphError,
};
use clutch_batch_policy_identity::revenue_policy_v2::{
    canonical_revenue_policy_v2_bytes, decode_revenue_policy_v2, revenue_policy_record_v2_id,
    revenue_policy_v2_digest, treasury_position_derivation_policy_v2_id, RevenuePolicyV2,
    REVENUE_POLICY_V2_BYTES,
};
use clutch_collateral_adapter_v2::{CollateralPolicyV2, COLLATERAL_POLICY_V2_BYTES};
use clutch_general_v2_contract::{
    project_general_position_replay_prestate_v1, Id32, MarketBindingV4,
    MarketRuntimeV3AccountV1, MARKET_BINDING_ACCOUNT_BYTES_V4,
    MARKET_BINDING_SEED_DOMAIN_V1, MARKET_RUNTIME_ACCOUNT_BYTES, MARKET_RUNTIME_SEED_DOMAIN_V1,
    GENERAL_POSITION_FOUNDING_GENERATION_V1,
};
use clutch_owner_settlement::AuthenticatedPositionV3;
use clutch_retirement::{
    Identity32V1, PositionAccountV3, PositionPurposeV3, PositionV3Sha256Backend,
    ReplayV3Envelope, ReplayV3HashBackend, POSITION_V3_BYTES, PURPOSE_REPLAY_V3_PDA_PREFIX,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::registry::{
    AllocationStatus, ExtensionAction, GeneralV2Action, RealmRevenueV2Action,
    GENERAL_V2_FAMILY_TAG, GENERAL_V2_FAMILY_VERSION, REALM_REVENUE_V2_FAMILY_TAG,
    REALM_REVENUE_V2_FAMILY_VERSION, SOURCE_SERIES_FAMILY_TAG, SOURCE_SERIES_FAMILY_VERSION,
};
use clutch_solana_layout::revenue::{
    CloseRevenuePolicyRecordV2Payload, InitializeFeeBearingRealmV2Payload,
    RevenuePolicyRecordV2, TreasuryServiceLedgerV1, REVENUE_POLICY_RECORD_BYTES_V2,
    TREASURY_SERVICE_LEDGER_V1_BYTES,
};
use clutch_solana_layout::{
    account_len, canonical_profile_v2_id, canonical_realm_id, Hash32, ProfileAccountV2,
    RealmAccount, MAX_OUTCOMES, PROFILE_SCHEMA_V2,
};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::AccountMeta;
use std::collections::BTreeSet;

pub const CANONICAL_ACTION_MATERIAL_SCHEMA_V1: &str =
    "dragons-clutch/operator-canonical-action-material/v1";
/// Fixed release-owned Revenue founding-intent width.
pub const CHECKED_REVENUE_REALM_INTENT_BYTES_V1: usize = 260;
const CHECKED_REVENUE_REALM_INTENT_MAGIC_V1: [u8; 8] = *b"DCREVOP1";
const CHECKED_REVENUE_REALM_INTENT_SCHEMA_V1: u16 = 1;
const PRODUCT_ARTIFACT_SEED_V1: &[u8] = b"dc:product-artifact:v1";
const REALM_SEED_V1: &[u8] = b"dragons-clutch:realm:v1";
const PROFILE_SEED_V1: &[u8] = b"dragons-clutch:profile:v1";
const REVENUE_POLICY_SEED_V1: &[u8] = b"dragons-clutch:revenue-policy:v1";
const TREASURY_SERVICE_LEDGER_SEED_V1: &[u8] = b"treasury-service-v1";
const REVENUE_OPERATOR_OBSERVATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/operator/revenue-observation/v1\0";

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
    InvalidCheckedIntent,
    InvalidChainState,
    Arithmetic,
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
            Self::InvalidCheckedIntent => {
                "revenue action differs from its checked release intent"
            }
            Self::InvalidChainState => {
                "revenue action input failed hostile chain authentication"
            }
            Self::Arithmetic => "revenue action accounting overflowed",
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

/// Hostile-decoded immutable operator intent selected by one checked release.
///
/// There is intentionally no public field constructor or browser codec. The
/// release file owns all 260 bytes, including the treasury beneficiary and
/// exact 40/10 development calibration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedRevenueRealmIntentV1 {
    profile: Hash32,
    collateral_policy_id: Hash32,
    realm_nonce: u64,
    max_outcomes: u8,
    profile_version: u8,
    policy: RevenuePolicyV2,
}

impl CheckedRevenueRealmIntentV1 {
    /// Decode the exact fixed release artifact. Trailing bytes, foreign
    /// releases, and any non-development calibration refuse.
    pub fn decode(input: &[u8], release: &IndexedProgramRelease) -> Result<Self> {
        release
            .validate()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
        if input.len() != CHECKED_REVENUE_REALM_INTENT_BYTES_V1
            || input.get(..8) != Some(CHECKED_REVENUE_REALM_INTENT_MAGIC_V1.as_slice())
            || input.get(8..10)
                != Some(CHECKED_REVENUE_REALM_INTENT_SCHEMA_V1.to_le_bytes().as_slice())
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidCheckedIntent);
        }
        let release_manifest_sha256 = fixed::<32>(input, 10)?;
        let capability_profile_id = fixed::<32>(input, 42)?;
        let program_id = Address::new_from_array(fixed::<32>(input, 74)?);
        let profile = Hash32::from_bytes(fixed::<32>(input, 106)?);
        let collateral_policy_id = Hash32::from_bytes(fixed::<32>(input, 138)?);
        let realm_nonce = u64::from_le_bytes(fixed::<8>(input, 170)?);
        let max_outcomes = input[178];
        let profile_version = input[179];
        let policy = decode_revenue_policy_v2(&input[180..260])
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidCheckedIntent)?;
        if release_manifest_sha256 != release.release_manifest_sha256
            || capability_profile_id != release.capability_profile_id
            || program_id != release.program_id
            || profile == Hash32::ZERO
            || collateral_policy_id == Hash32::ZERO
            || usize::from(max_outcomes) != MAX_OUTCOMES
            || profile_version != PROFILE_SCHEMA_V2
            || !policy.is_successor_development_profile()
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidCheckedIntent);
        }
        Ok(Self {
            profile,
            collateral_policy_id,
            realm_nonce,
            max_outcomes,
            profile_version,
            policy,
        })
    }
}

/// Authenticated chain inputs needed to create a fee-bearing Realm. Writable
/// Realm/record addresses are derived from these facts and never supplied.
#[derive(Clone, Debug)]
pub struct AuthenticatedRevenueRealmInitializationV1 {
    intent: CheckedRevenueRealmIntentV1,
    collateral_policy_account: Address,
    rent_sysvar_slot: u64,
    policy_slot: u64,
    realm: Hash32,
    realm_account: Address,
    record_account: Address,
    maximum_rent_principal_lamports: u64,
    observed_state_sha256: [u8; 32],
}

/// Hostile-authenticate the checked intent, exact collateral-policy artifact,
/// and live Rent sysvar before any action material is constructed.
pub fn authenticate_revenue_realm_initialization_v1(
    release: &IndexedProgramRelease,
    checked_intent: &[u8],
    collateral_policy_account: &ObservedRpcAccount,
    rent_sysvar: &ObservedRpcAccount,
) -> Result<AuthenticatedRevenueRealmInitializationV1> {
    let intent = CheckedRevenueRealmIntentV1::decode(checked_intent, release)?;
    require_release_account(release, collateral_policy_account)?;
    if collateral_policy_account.data.len() != COLLATERAL_POLICY_V2_BYTES {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let collateral_policy = CollateralPolicyV2::decode(&collateral_policy_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let collateral_policy_id = collateral_policy
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let profile = canonical_profile_v2_id(
        Hash32::from_bytes(collateral_policy_id.bytes()),
        Hash32::from_bytes(collateral_policy.adapter_release.bytes()),
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let policy_kind = [ArtifactKind::CollateralPolicy.byte()];
    let expected_policy_account = Address::find_program_address(
        &[
            PRODUCT_ARTIFACT_SEED_V1,
            &policy_kind,
            &collateral_policy_id.bytes(),
        ],
        &release.program_id,
    )
    .0;
    if collateral_policy_account.address != expected_policy_account
        || Hash32::from_bytes(collateral_policy_id.bytes()) != intent.collateral_policy_id
        || profile != intent.profile
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let rent = decode_rent_observation(release, rent_sysvar)?;
    let realm = canonical_realm_id(intent.profile, intent.realm_nonce);
    let realm_account = Address::find_program_address(
        &[REALM_SEED_V1, &realm.bytes()],
        &release.program_id,
    )
    .0;
    let record_account = Address::find_program_address(
        &[REVENUE_POLICY_SEED_V1, &realm.bytes()],
        &release.program_id,
    )
    .0;
    let realm_rent = rent.minimum_balance(account_len::REALM)?;
    let record_rent = rent.minimum_balance(REVENUE_POLICY_RECORD_BYTES_V2)?;
    let maximum_rent_principal_lamports = realm_rent
        .checked_add(record_rent)
        .ok_or(CanonicalActionMaterialErrorV1::Arithmetic)?;
    let observed_state_sha256 = observed_accounts_digest(&[
        collateral_policy_account,
        rent_sysvar,
    ]);
    Ok(AuthenticatedRevenueRealmInitializationV1 {
        intent,
        collateral_policy_account: collateral_policy_account.address,
        rent_sysvar_slot: rent_sysvar.provenance.slot,
        policy_slot: collateral_policy_account.provenance.slot,
        realm: Hash32::from_bytes(realm.bytes()),
        realm_account,
        record_account,
        maximum_rent_principal_lamports,
        observed_state_sha256,
    })
}

/// Authenticated terminal record/Realm-absence observation. The record's
/// persisted payer and rent split determine every writable destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRevenueRecordCloseV1 {
    record: RevenuePolicyRecordV2,
    record_account: Address,
    realm_account: Address,
    observed_slot: u64,
    observed_record_lamports: u64,
    neutral_lamports: u64,
    observed_state_sha256: [u8; 32],
}

/// Hostile-authenticate one live V2 record plus a finalized closed-account
/// observation for its canonical Realm address.
pub fn authenticate_revenue_record_close_v1(
    release: &IndexedProgramRelease,
    record_account: &ObservedRpcAccount,
    realm_absence: &ObservedRpcAccountRemoval,
) -> Result<AuthenticatedRevenueRecordCloseV1> {
    require_release_account(release, record_account)?;
    if record_account.data.len() != REVENUE_POLICY_RECORD_BYTES_V2
        || realm_absence.kind != RpcAccountRemovalKind::Closed
        || realm_absence.observed_lamports != 0
        || realm_absence.observed_data_bytes != 0
        || realm_absence.observed_executable
        || realm_absence.provenance.commitment != RpcCommitment::Finalized
        || realm_absence.provenance.release_key != release.key()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let record = RevenuePolicyRecordV2::decode(&record_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let realm_account = Address::find_program_address(
        &[REALM_SEED_V1, &record.realm.bytes()],
        &release.program_id,
    )
    .0;
    let expected_record = Address::find_program_address(
        &[REVENUE_POLICY_SEED_V1, &record.realm.bytes()],
        &release.program_id,
    )
    .0;
    if realm_absence.address != realm_account || record_account.address != expected_record {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let neutral_lamports = record_account
        .lamports
        .checked_sub(record.terminal_payer_principal)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if neutral_lamports < record.terminal_donation_floor {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let observed_slot = record_account
        .provenance
        .slot
        .max(realm_absence.provenance.slot);
    let mut hash = Sha256::new();
    hash.update(REVENUE_OPERATOR_OBSERVATION_DOMAIN_V1);
    hash.update(observed_accounts_digest(&[record_account]));
    hash.update(realm_absence.address.to_bytes());
    hash.update(realm_absence.provenance.slot.to_le_bytes());
    let observed_state_sha256 = hash.finalize().into();
    Ok(AuthenticatedRevenueRecordCloseV1 {
        record,
        record_account: record_account.address,
        realm_account,
        observed_slot,
        observed_record_lamports: record_account.lamports,
        neutral_lamports,
        observed_state_sha256,
    })
}

/// Exact chain accounts behind one Market's fee-bearing treasury service.
///
/// These are RPC observations, not caller-selected instruction roles. The
/// authenticator below rederives every PDA and joins every semantic body
/// before retaining any address.
#[derive(Clone, Copy)]
pub struct TreasuryServiceLifecycleObservationsV1<'a> {
    pub realm: &'a ObservedRpcAccount,
    pub profile: &'a ObservedRpcAccount,
    pub revenue_record: &'a ObservedRpcAccount,
    pub market_binding: &'a ObservedRpcAccount,
    pub market_runtime: &'a ObservedRpcAccount,
    pub treasury_position: &'a ObservedRpcAccount,
    pub treasury_replay: &'a ObservedRpcAccount,
    pub service_ledger: &'a ObservedRpcAccount,
}

/// Opaque hostile-authenticated authority for the stable 0xbb lifecycle.
///
/// This deliberately does not construct General action39/action50 bytes: the
/// complete General frames own the per-Epoch admission/terminal evidence and
/// remain their sole payload owners. It supplies the release-bound physical
/// and semantic facts those composers must consume, without letting an
/// operator or browser shape a parallel DTO.
#[derive(Clone, Copy, Debug)]
pub struct AuthenticatedTreasuryServiceLifecycleV1 {
    release_manifest_sha256: [u8; 32],
    capability_profile_id: [u8; 32],
    program_id: Address,
    realm_account: Address,
    profile_account: Address,
    revenue_record_account: Address,
    market_binding_account: Address,
    market_runtime_account: Address,
    treasury_position_account: Address,
    treasury_replay_account: Address,
    service_ledger_account: Address,
    market_instance_v2_id: Hash32,
    revenue_policy_record_v2_id: Hash32,
    revenue_policy_v2_digest: Hash32,
    treasury_owner: Hash32,
    ledger: TreasuryServiceLedgerV1,
    observed_slot: u64,
    observed_state_sha256: [u8; 32],
}

impl AuthenticatedTreasuryServiceLifecycleV1 {
    #[must_use]
    pub const fn service_ledger_account(self) -> Address {
        self.service_ledger_account
    }

    #[must_use]
    pub const fn ledger(self) -> TreasuryServiceLedgerV1 {
        self.ledger
    }

    #[must_use]
    pub const fn observed_slot(self) -> u64 {
        self.observed_slot
    }

    #[must_use]
    pub const fn observed_state_sha256(self) -> [u8; 32] {
        self.observed_state_sha256
    }

    /// Stable hostile-authenticated roles that a complete General composer
    /// must exact-join. This is not an instruction account list: action39 and
    /// action50 own their larger atomic frames.
    #[must_use]
    pub fn authority_roles(self) -> [CanonicalAccountRoleV1; 8] {
        [
            canonical_role("realm", self.realm_account, false, false),
            canonical_role("profile", self.profile_account, false, false),
            canonical_role(
                "revenue-policy-record-v2",
                self.revenue_record_account,
                false,
                false,
            ),
            canonical_role(
                "market-binding-v4",
                self.market_binding_account,
                false,
                false,
            ),
            canonical_role(
                "market-runtime-v3",
                self.market_runtime_account,
                false,
                false,
            ),
            canonical_role(
                "treasury-position-v3",
                self.treasury_position_account,
                false,
                false,
            ),
            canonical_role(
                "treasury-replay-v3",
                self.treasury_replay_account,
                false,
                false,
            ),
            canonical_role(
                "treasury-service-ledger-v1",
                self.service_ledger_account,
                true,
                false,
            ),
        ]
    }

    /// Refuse reuse under another checked release or after any authoritative
    /// account changed.
    #[must_use]
    pub fn matches_release(
        self,
        release: &IndexedProgramRelease,
        observed_state_sha256: [u8; 32],
    ) -> bool {
        release.validate().is_ok()
            && self.release_manifest_sha256 == release.release_manifest_sha256
            && self.capability_profile_id == release.capability_profile_id
            && self.program_id == release.program_id
            && self.observed_state_sha256 == observed_state_sha256
    }

    /// Whether this exact authority may be consumed by the named General
    /// ledger transition in the checked release. This never constructs the
    /// surrounding General payload or account frame.
    #[must_use]
    pub fn admits_general_transition(
        self,
        release: &IndexedProgramRelease,
        action: GeneralV2Action,
    ) -> bool {
        matches!(
            action,
            GeneralV2Action::InitializeSettlementRoot
                | GeneralV2Action::AdvanceFeeRetirement
        ) && self.matches_release(release, self.observed_state_sha256)
            && release
                .enabled_intents
                .binary_search(&CanonicalIntentCoordinate {
                    family_tag: GENERAL_V2_FAMILY_TAG,
                    family_version: GENERAL_V2_FAMILY_VERSION,
                    local_action: action.tag(),
                })
                .is_ok()
    }

    #[must_use]
    pub const fn market_instance_v2_id(self) -> Hash32 {
        self.market_instance_v2_id
    }

    #[must_use]
    pub const fn revenue_policy_record_v2_id(self) -> Hash32 {
        self.revenue_policy_record_v2_id
    }

    #[must_use]
    pub const fn revenue_policy_v2_digest(self) -> Hash32 {
        self.revenue_policy_v2_digest
    }

    #[must_use]
    pub const fn treasury_owner(self) -> Hash32 {
        self.treasury_owner
    }
}

/// Authenticate the complete current V4/PositionV3/ReplayV3/0xbb authority
/// graph from finalized chain observations and one release-owned Realm intent.
pub fn authenticate_treasury_service_lifecycle_v1(
    release: &IndexedProgramRelease,
    checked_intent: &[u8],
    observations: TreasuryServiceLifecycleObservationsV1<'_>,
) -> Result<AuthenticatedTreasuryServiceLifecycleV1> {
    release
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    let intent = CheckedRevenueRealmIntentV1::decode(checked_intent, release)?;
    let observed = [
        observations.realm,
        observations.profile,
        observations.revenue_record,
        observations.market_binding,
        observations.market_runtime,
        observations.treasury_position,
        observations.treasury_replay,
        observations.service_ledger,
    ];
    for account in observed {
        require_release_account(release, account)?;
    }
    let distinct = observed
        .iter()
        .map(|account| account.address)
        .collect::<BTreeSet<_>>();
    if distinct.len() != observed.len()
        || observations.realm.data.len() != account_len::REALM
        || observations.profile.data.len() != account_len::PROFILE
        || observations.revenue_record.data.len() != REVENUE_POLICY_RECORD_BYTES_V2
        || observations.market_binding.data.len() != MARKET_BINDING_ACCOUNT_BYTES_V4
        || observations.market_runtime.data.len() != MARKET_RUNTIME_ACCOUNT_BYTES
        || observations.treasury_position.data.len() != POSITION_V3_BYTES
        || observations.service_ledger.data.len() != TREASURY_SERVICE_LEDGER_V1_BYTES
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    let realm = RealmAccount::decode(&observations.realm.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let profile = ProfileAccountV2::decode(&observations.profile.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let record = RevenuePolicyRecordV2::decode(&observations.revenue_record.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    record
        .binds_policy(&intent.policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let binding = MarketBindingV4::decode(&observations.market_binding.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let runtime = MarketRuntimeV3AccountV1::decode(&observations.market_runtime.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let position = PositionAccountV3::decode(&observations.treasury_position.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let replay = ReplayV3Envelope::decode(&observations.treasury_replay.data, &OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let ledger = TreasuryServiceLedgerV1::decode(&observations.service_ledger.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;

    let expected_realm = canonical_realm_id(intent.profile, intent.realm_nonce);
    let realm_pda = Address::find_program_address(
        &[REALM_SEED_V1, &expected_realm.bytes()],
        &release.program_id,
    );
    let profile_pda = Address::find_program_address(
        &[PROFILE_SEED_V1, &expected_realm.bytes(), &intent.profile.bytes()],
        &release.program_id,
    );
    let record_pda = Address::find_program_address(
        &[REVENUE_POLICY_SEED_V1, &expected_realm.bytes()],
        &release.program_id,
    );
    let base = binding.base().base();
    let market_instance = base.market_instance_v2_id;
    let binding_pda = Address::find_program_address(
        &[MARKET_BINDING_SEED_DOMAIN_V1, &market_instance.bytes()],
        &release.program_id,
    );
    let runtime_pda = Address::find_program_address(
        &[MARKET_RUNTIME_SEED_DOMAIN_V1, &binding_pda.0.to_bytes()],
        &release.program_id,
    );
    let authority = binding.authority();
    let treasury_owner = Hash32::from_bytes(intent.policy.treasury_owner);
    let purpose = [u8::from(PositionPurposeV3::General)];
    let position_pda = Address::find_program_address(
        &[
            clutch_retirement::POSITION_V3_PDA_PREFIX,
            &market_instance.bytes(),
            &treasury_owner.bytes(),
            &purpose,
            &runtime_pda.0.to_bytes(),
        ],
        &release.program_id,
    );
    let replay_pda = Address::find_program_address(
        &[
            PURPOSE_REPLAY_V3_PDA_PREFIX,
            &position_pda.0.to_bytes(),
            &purpose,
            &runtime_pda.0.to_bytes(),
        ],
        &release.program_id,
    );
    let ledger_pda = Address::find_program_address(
        &[
            TREASURY_SERVICE_LEDGER_SEED_V1,
            &market_instance.bytes(),
            &position_pda.0.to_bytes(),
        ],
        &release.program_id,
    );
    let record_semantic_id = revenue_policy_record_v2_id(record.realm.bytes(), &intent.policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let policy_digest = revenue_policy_v2_digest(&intent.policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let derivation_policy_id =
        treasury_position_derivation_policy_v2_id(intent.policy.treasury_position_derivation);

    if realm.realm != expected_realm
        || realm.profile != intent.profile
        || realm.stored_bump != realm_pda.1
        || observations.realm.address != realm_pda.0
        || profile.realm != realm.realm
        || profile.profile != realm.profile
        || profile.collateral_policy_id != intent.collateral_policy_id
        || observations.profile.address != profile_pda.0
        || record.realm != realm.realm
        || record.stored_bump != record_pda.1
        || observations.revenue_record.address != record_pda.0
        || observations.market_binding.address != binding_pda.0
        || base.market.bytes() != runtime_pda.0.to_bytes()
        || observations.market_runtime.address != runtime_pda.0
        || runtime.stored_bump != runtime_pda.1
        || runtime.market_binding.bytes() != binding_pda.0.to_bytes()
        || runtime.market_instance_v2_id != market_instance
        || authority.revenue_policy_record_account().bytes()
            != observations.revenue_record.address.to_bytes()
        || authority.revenue_policy_record_v2_id().bytes() != record_semantic_id.0
        || authority.revenue_policy_v2_digest().bytes() != policy_digest.0
        || authority.treasury_owner().bytes() != treasury_owner.bytes()
        || authority.treasury_position_derivation_policy_v2_id().bytes()
            != derivation_policy_id.0
        || authority.treasury_position_account().bytes() != position_pda.0.to_bytes()
        || authority.treasury_service_ledger_account().bytes() != ledger_pda.0.to_bytes()
        || observations.treasury_position.address != position_pda.0
        || position.stored_bump() != position_pda.1
        || position.purpose() != PositionPurposeV3::General
        || position.market_instance_id().bytes() != market_instance.bytes()
        || position.realm_id().bytes() != realm.realm.bytes()
        || position.collateral_policy_id().bytes() != profile.collateral_policy_id.bytes()
        || position.collateral_release_id().bytes() != profile.adapter_release_id.bytes()
        || position.owner().bytes() != treasury_owner.bytes()
        || position.controller().bytes() != treasury_owner.bytes()
        || position.replay_account().bytes() != replay_pda.0.to_bytes()
        || position.purpose_binding_id().bytes() != runtime_pda.0.to_bytes()
        || position.outcome_count() != base.outcome_count
        || observations.treasury_replay.address != replay_pda.0
        || replay.header().stored_bump() != replay_pda.1
        || observations.service_ledger.address != ledger_pda.0
        || ledger.stored_bump != ledger_pda.1
        || ledger.realm != realm.realm
        || ledger.revenue_policy_record_account
            != Hash32::from_bytes(observations.revenue_record.address.to_bytes())
        || ledger.revenue_policy_record_v2_id != Hash32::from_bytes(record_semantic_id.0)
        || ledger.market_instance_v2_id != Hash32::from_bytes(market_instance.bytes())
        || ledger.treasury_owner != treasury_owner
        || ledger.treasury_position_account != Hash32::from_bytes(position_pda.0.to_bytes())
        || ledger.treasury_position_founding_generation
            != GENERAL_POSITION_FOUNDING_GENERATION_V1
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }

    let position_semantic_id = position
        .semantic_id(&OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let authenticated_position = AuthenticatedPositionV3 {
        account: position_pda.0.to_bytes(),
        general_market_runtime: runtime_pda.0.to_bytes(),
        semantic: position,
        semantic_id: position_semantic_id.bytes(),
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    };
    project_general_position_replay_prestate_v1(
        Id32::new(replay_pda.0.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?,
        replay_pda.1,
        replay.header().next_sequence(),
        &observations.treasury_replay.data,
        authenticated_position,
        &OperatorSha256V1,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let binding_rent = binding.rent();
    require_accounted_lamports(
        observations.market_binding,
        binding_rent.refundable_principal,
        binding_rent.donation_floor,
    )?;
    require_accounted_lamports(
        observations.market_runtime,
        runtime.rent.refundable_principal,
        runtime.rent.donation_floor,
    )?;
    require_accounted_lamports(
        observations.revenue_record,
        record.terminal_payer_principal,
        record.terminal_donation_floor,
    )?;
    let position_rent = position.rent();
    let position_principal = position_rent
        .refundable_live_principal
        .checked_add(position_rent.permanent_tombstone_principal)
        .ok_or(CanonicalActionMaterialErrorV1::Arithmetic)?;
    require_accounted_lamports(
        observations.treasury_position,
        position_principal,
        position_rent.donation_floor,
    )?;
    let replay_rent = replay.header().rent();
    require_accounted_lamports(
        observations.treasury_replay,
        replay_rent.refundable_principal(),
        replay_rent.donation_floor(),
    )?;
    require_accounted_lamports(
        observations.service_ledger,
        ledger.refundable_rent_principal,
        ledger.donation_floor,
    )?;

    let observed_slot = observed
        .iter()
        .map(|account| account.provenance.slot)
        .max()
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    Ok(AuthenticatedTreasuryServiceLifecycleV1 {
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        program_id: release.program_id,
        realm_account: observations.realm.address,
        profile_account: observations.profile.address,
        revenue_record_account: observations.revenue_record.address,
        market_binding_account: observations.market_binding.address,
        market_runtime_account: observations.market_runtime.address,
        treasury_position_account: observations.treasury_position.address,
        treasury_replay_account: observations.treasury_replay.address,
        service_ledger_account: observations.service_ledger.address,
        market_instance_v2_id: Hash32::from_bytes(market_instance.bytes()),
        revenue_policy_record_v2_id: Hash32::from_bytes(record_semantic_id.0),
        revenue_policy_v2_digest: Hash32::from_bytes(policy_digest.0),
        treasury_owner,
        ledger,
        observed_slot,
        observed_state_sha256: observed_accounts_digest(&observed),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OperatorSha256V1;

impl PositionV3Sha256Backend for OperatorSha256V1 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(domain);
        hash.update(body);
        hash.finalize().into()
    }
}

impl ReplayV3HashBackend for OperatorSha256V1 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut hash = Sha256::new();
        for part in parts {
            hash.update(part);
        }
        hash.finalize().into()
    }
}

fn require_accounted_lamports(
    account: &ObservedRpcAccount,
    principal: u64,
    donation_floor: u64,
) -> Result<()> {
    let accounted = principal
        .checked_add(donation_floor)
        .ok_or(CanonicalActionMaterialErrorV1::Arithmetic)?;
    if account.lamports < accounted {
        Err(CanonicalActionMaterialErrorV1::InvalidChainState)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct ObservedRentV1 {
    lamports_per_byte: u64,
    exemption_threshold: [u8; 8],
    burn_percent: u8,
}

impl ObservedRentV1 {
    fn minimum_balance(self, bytes: usize) -> Result<u64> {
        #[allow(deprecated)]
        let rent = solana_rent::Rent {
            lamports_per_byte: self.lamports_per_byte,
            exemption_threshold: self.exemption_threshold,
            burn_percent: self.burn_percent,
        };
        rent.try_minimum_balance(bytes)
            .ok_or(CanonicalActionMaterialErrorV1::Arithmetic)
    }
}

fn decode_rent_observation(
    release: &IndexedProgramRelease,
    account: &ObservedRpcAccount,
) -> Result<ObservedRentV1> {
    if account.address != solana_sdk_ids::sysvar::rent::ID
        || account.owner != solana_sdk_ids::sysvar::ID
        || account.executable
        || account.data.len() != 17
        || account.provenance.commitment != RpcCommitment::Finalized
        || account.provenance.release_key != release.key()
        || account.provenance.slot == 0
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidChainState);
    }
    let value = ObservedRentV1 {
        lamports_per_byte: u64::from_le_bytes(fixed::<8>(&account.data, 0)?),
        exemption_threshold: fixed::<8>(&account.data, 8)?,
        burn_percent: account.data[16],
    };
    value.minimum_balance(0)?;
    Ok(value)
}

fn fixed<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset
        .checked_add(N)
        .ok_or(CanonicalActionMaterialErrorV1::Arithmetic)?;
    input
        .get(offset..end)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidCheckedIntent)?
        .try_into()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidCheckedIntent)
}

fn require_release_account(
    release: &IndexedProgramRelease,
    account: &ObservedRpcAccount,
) -> Result<()> {
    if account.owner != release.program_id
        || account.executable
        || account.provenance.release_key != release.key()
        || account.provenance.commitment != RpcCommitment::Finalized
        || account.provenance.slot == 0
    {
        Err(CanonicalActionMaterialErrorV1::InvalidChainState)
    } else {
        Ok(())
    }
}

fn observed_accounts_digest(accounts: &[&ObservedRpcAccount]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(REVENUE_OPERATOR_OBSERVATION_DOMAIN_V1);
    for account in accounts {
        hash.update(account.address.to_bytes());
        hash.update(account.owner.to_bytes());
        hash.update(account.lamports.to_le_bytes());
        hash.update(account.provenance.slot.to_le_bytes());
        hash.update(
            u64::try_from(account.data.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        hash.update(&account.data);
    }
    hash.finalize().into()
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

    /// Exact release/observation join for RealmRevenue material, whose driver
    /// is a chain account rather than a keeper-selected Source cursor.
    #[must_use]
    pub fn matches_revenue_observation(
        &self,
        release: &IndexedProgramRelease,
        driver_account: Address,
        driver_slot: u64,
        observed_state_sha256: [u8; 32],
    ) -> bool {
        self.release_key == release.key()
            && self.release_manifest_sha256 == release.release_manifest_sha256
            && self.capability_profile_id == release.capability_profile_id
            && self.coordinate.family_tag == REALM_REVENUE_V2_FAMILY_TAG
            && self.coordinate.family_version == REALM_REVENUE_V2_FAMILY_VERSION
            && self.driver_account == driver_account
            && self.driver_account_slot == driver_slot
            && self.cursor.observed_state_sha256 == observed_state_sha256
            && release.enabled_intents.binary_search(&self.coordinate).is_ok()
            && self.planned.reload_authoritative_accounts
            && !self.planned.unsigned_transaction.has_recent_blockhash
            && !self.planned.unsigned_transaction.signed
            && !self.planned.unsigned_transaction.submitted
    }
}

/// Construct the exact unsigned 81/v1/action1 request. The checked intent owns
/// the treasury and policy bytes; the hostile collateral/rent observations
/// own every derived target and rent equation.
pub fn construct_initialize_fee_bearing_realm_material_v1(
    release: &IndexedProgramRelease,
    builder: &ProtocolTransactionBuilder,
    authenticated: &AuthenticatedRevenueRealmInitializationV1,
    freshness: ActionFreshnessBoundaryV1,
) -> Result<CanonicalActionMaterialV1> {
    let action = RealmRevenueV2Action::InitializeFeeBearingRealmV2;
    require_revenue_release(release, builder, action, freshness)?;
    let observed_slot = authenticated
        .policy_slot
        .max(authenticated.rent_sysvar_slot);
    if freshness.observed_slot != observed_slot {
        return Err(CanonicalActionMaterialErrorV1::InvalidFreshness);
    }
    let payload = InitializeFeeBearingRealmV2Payload {
        profile: authenticated.intent.profile,
        realm_nonce: authenticated.intent.realm_nonce,
        max_outcomes: authenticated.intent.max_outcomes,
        profile_version: authenticated.intent.profile_version,
        policy: authenticated.intent.policy,
    };
    let mut payload_bytes = vec![0; clutch_solana_layout::revenue::INITIALIZE_FEE_BEARING_REALM_V2_PAYLOAD_BYTES];
    let exact = payload
        .encode(&mut payload_bytes)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidCheckedIntent)?;
    payload_bytes.truncate(exact);
    let roles = vec![
        role("founding-payer", builder.payer(), true, true),
        role("realm", authenticated.realm_account, true, false),
        role(
            "collateral-policy",
            authenticated.collateral_policy_account,
            false,
            false,
        ),
        role(
            "revenue-policy-record-v2",
            authenticated.record_account,
            true,
            false,
        ),
        role(
            "system-program",
            solana_sdk_ids::system_program::ID,
            false,
            false,
        ),
        role(
            "rent-sysvar",
            solana_sdk_ids::sysvar::rent::ID,
            false,
            false,
        ),
    ];
    let equations = vec![ExactEquation {
        name: "maximum-realm-and-revenue-record-rent-principal".into(),
        unit: IntegerUnit::Lamports,
        left: u128::from(authenticated.maximum_rent_principal_lamports),
        right: u128::from(authenticated.maximum_rent_principal_lamports),
    }];
    let draft = crate::transaction_builder::OwnedInstructionDraft::enabled_realm_revenue_request_v1(
        "initialize-fee-bearing-realm-v2",
        revenue_semantic_owner(release),
        release.program_id,
        roles.clone(),
        vec![builder.payer()],
        equations,
        action,
        &payload_bytes,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    finish_revenue_material(
        release,
        builder,
        action,
        authenticated.collateral_policy_account,
        observed_slot,
        authenticated.realm,
        authenticated.observed_state_sha256,
        freshness,
        &[
            "founding-payer",
            "realm",
            "collateral-policy",
            "revenue-policy-record-v2",
            "system-program",
            "rent-sysvar",
        ],
        roles,
        draft,
    )
}

/// Construct the exact unsigned 81/v1/action2 request. The hostile record
/// supplies its Realm, principal recipient, and donation/surplus disposition;
/// no signer or browser field can redirect them.
pub fn construct_close_revenue_policy_record_material_v1(
    release: &IndexedProgramRelease,
    builder: &ProtocolTransactionBuilder,
    authenticated: AuthenticatedRevenueRecordCloseV1,
    freshness: ActionFreshnessBoundaryV1,
) -> Result<CanonicalActionMaterialV1> {
    let action = RealmRevenueV2Action::CloseRevenuePolicyRecordV2;
    require_revenue_release(release, builder, action, freshness)?;
    if freshness.observed_slot != authenticated.observed_slot {
        return Err(CanonicalActionMaterialErrorV1::InvalidFreshness);
    }
    let payload = CloseRevenuePolicyRecordV2Payload {
        realm: authenticated.record.realm,
    };
    let mut payload_bytes = vec![0; clutch_solana_layout::revenue::CLOSE_REVENUE_POLICY_RECORD_V2_PAYLOAD_BYTES];
    let exact = payload
        .encode(&mut payload_bytes)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    payload_bytes.truncate(exact);
    let refund_owner = Address::new_from_array(authenticated.record.terminal_payer.bytes());
    let neutral_sink = solana_sdk_ids::incinerator::ID;
    let roles = vec![
        role("absent-realm", authenticated.realm_account, false, false),
        role(
            "revenue-policy-record-v2",
            authenticated.record_account,
            true,
            false,
        ),
        role("record-rent-refund-owner", refund_owner, true, false),
        role("neutral-sink", neutral_sink, true, false),
    ];
    let accounted = authenticated
        .record
        .terminal_payer_principal
        .checked_add(authenticated.neutral_lamports)
        .ok_or(CanonicalActionMaterialErrorV1::Arithmetic)?;
    let equations = vec![ExactEquation {
        name: "record-balance-equals-principal-plus-neutral-lamports".into(),
        unit: IntegerUnit::Lamports,
        left: u128::from(authenticated.observed_record_lamports),
        right: u128::from(accounted),
    }];
    let draft = crate::transaction_builder::OwnedInstructionDraft::enabled_realm_revenue_request_v1(
        "close-revenue-policy-record-v2",
        revenue_semantic_owner(release),
        release.program_id,
        roles.clone(),
        Vec::new(),
        equations,
        action,
        &payload_bytes,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    finish_revenue_material(
        release,
        builder,
        action,
        authenticated.record_account,
        authenticated.observed_slot,
        authenticated.record.realm,
        authenticated.observed_state_sha256,
        freshness,
        &[
            "absent-realm",
            "revenue-policy-record-v2",
            "record-rent-refund-owner",
            "neutral-sink",
        ],
        roles,
        draft,
    )
}

fn require_revenue_release(
    release: &IndexedProgramRelease,
    builder: &ProtocolTransactionBuilder,
    action: RealmRevenueV2Action,
    freshness: ActionFreshnessBoundaryV1,
) -> Result<()> {
    release
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    freshness.validate()?;
    let coordinate = CanonicalIntentCoordinate {
        family_tag: REALM_REVENUE_V2_FAMILY_TAG,
        family_version: REALM_REVENUE_V2_FAMILY_VERSION,
        local_action: action.tag(),
    };
    if release.enabled_intents.binary_search(&coordinate).is_err() {
        return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
    }
    if builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn finish_revenue_material(
    release: &IndexedProgramRelease,
    builder: &ProtocolTransactionBuilder,
    action: RealmRevenueV2Action,
    driver_account: Address,
    driver_account_slot: u64,
    workflow_binding: Hash32,
    observed_state_sha256: [u8; 32],
    freshness: ActionFreshnessBoundaryV1,
    labels: &[&'static str],
    metas: Vec<AccountMeta>,
    draft: crate::transaction_builder::OwnedInstructionDraft,
) -> Result<CanonicalActionMaterialV1> {
    if labels.len() != metas.len() || observed_state_sha256 == [0; 32] {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let account_roles = labels
        .iter()
        .zip(&metas)
        .map(|(label, meta)| CanonicalAccountRoleV1 {
            label,
            address: meta.pubkey,
            writable: meta.is_writable,
            signer: meta.is_signer,
        })
        .collect::<Vec<_>>();
    let unsigned_transaction = builder
        .build_atomic(core::slice::from_ref(&draft))
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let coordinate = CanonicalIntentCoordinate {
        family_tag: REALM_REVENUE_V2_FAMILY_TAG,
        family_version: REALM_REVENUE_V2_FAMILY_VERSION,
        local_action: action.tag(),
    };
    validate_unsigned_revenue_plan(
        release,
        coordinate,
        builder.payer(),
        &account_roles,
        &unsigned_transaction,
    )?;
    let cursor = ResumableWorkflowCursor {
        workflow_id: revenue_workflow_id(release, workflow_binding),
        lane: match action {
            RealmRevenueV2Action::InitializeFeeBearingRealmV2 => {
                crate::workflow_graph::WorkflowLane::Creation
            }
            RealmRevenueV2Action::CloseRevenuePolicyRecordV2 => {
                crate::workflow_graph::WorkflowLane::RecoveryRetirement
            }
        },
        generation: 1,
        position: crate::workflow_graph::WorkflowPosition {
            phase: u16::from(action.tag()),
            item: 0,
        },
        observed_state_sha256,
    };
    let planned = PlannedWorkflowNode {
        manifest_sha256: release.release_manifest_sha256,
        cursor,
        coordinate: CanonicalActionCoordinate::Revenue(action),
        unsigned_transaction,
        reload_authoritative_accounts: true,
    };
    let release_key = release.key();
    let draft_id = action_material_id(
        &release_key,
        release.release_manifest_sha256,
        release.capability_profile_id,
        coordinate,
        driver_account,
        driver_account_slot,
        cursor,
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
        driver_account,
        driver_account_slot,
        cursor,
        freshness,
        fee_payer: builder.payer(),
        account_roles,
        planned,
        draft_id,
    })
}

fn validate_unsigned_revenue_plan(
    release: &IndexedProgramRelease,
    coordinate: CanonicalIntentCoordinate,
    fee_payer: Address,
    roles: &[CanonicalAccountRoleV1],
    transaction: &UnsignedProtocolTransaction,
) -> Result<()> {
    let binding_matches = matches!(
        transaction.registry_bindings.as_slice(),
        [Some(binding)]
            if binding.family.tag() == coordinate.family_tag
                && binding.family.version() == coordinate.family_version
                && binding.local_action == coordinate.local_action
                && binding.family.allocation_status() == Some(binding.family_status)
                && matches!(
                    binding.central_action,
                    Some(ExtensionAction::RealmRevenueV2(action))
                        if action.tag() == coordinate.local_action
                )
    );
    let instruction_signers = roles
        .iter()
        .filter(|role| role.signer)
        .map(|role| role.address)
        .collect::<BTreeSet<_>>();
    let semantic_owner_matches = matches!(
        transaction.semantic_owners.as_slice(),
        [owner]
            if owner.package == "clutch-solana-layout"
                && owner.schema == "realm-revenue-v2"
                && owner.release_sha256 == release.elf_sha256
    );
    if transaction.flows != [ProtocolFlow::RealmRevenue]
        || transaction.actions.len() != 1
        || !semantic_owner_matches
        || !binding_matches
        || transaction.runtime_admissions != [RuntimeAdmission::ReleaseBoundEnabled]
        || transaction.required_signers != [fee_payer]
        || !instruction_signers
            .iter()
            .all(|signer| transaction.required_signers.contains(signer))
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

fn role(label: &'static str, address: Address, writable: bool, signer: bool) -> AccountMeta {
    if writable {
        AccountMeta::new(address, signer)
    } else {
        AccountMeta::new_readonly(address, signer)
    }
}

const fn canonical_role(
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

fn revenue_semantic_owner(release: &IndexedProgramRelease) -> SemanticOwner {
    SemanticOwner {
        package: "clutch-solana-layout".into(),
        schema: "realm-revenue-v2".into(),
        release_sha256: release.elf_sha256,
    }
}

fn revenue_workflow_id(release: &IndexedProgramRelease, binding: Hash32) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator/revenue-workflow/v1\0");
    hash.update(release.program_id.to_bytes());
    hash.update(release.release_manifest_sha256);
    hash.update(binding.bytes());
    hash.finalize().into()
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

    fn revenue_release(enabled: bool) -> IndexedProgramRelease {
        IndexedProgramRelease {
            program_id: address(2),
            program_data: address(3),
            elf_sha256: [4; 32],
            deployment_slot: 7,
            release_manifest_sha256: [5; 32],
            capability_profile_id: [6; 32],
            source_commit: "11".repeat(20),
            enabled_intents: if enabled {
                vec![CanonicalIntentCoordinate {
                    family_tag: REALM_REVENUE_V2_FAMILY_TAG,
                    family_version: REALM_REVENUE_V2_FAMILY_VERSION,
                    local_action: RealmRevenueV2Action::InitializeFeeBearingRealmV2.tag(),
                }]
            } else {
                Vec::new()
            },
            families: vec![crate::rpc_index::CanonicalFamily::Fees],
        }
    }

    fn checked_revenue_intent(release: &IndexedProgramRelease) -> Vec<u8> {
        let policy = RevenuePolicyV2::successor_development([17; 32]);
        let policy_bytes = canonical_revenue_policy_v2_bytes(&policy).unwrap();
        let mut bytes = vec![0; CHECKED_REVENUE_REALM_INTENT_BYTES_V1];
        bytes[..8].copy_from_slice(&CHECKED_REVENUE_REALM_INTENT_MAGIC_V1);
        bytes[8..10].copy_from_slice(&CHECKED_REVENUE_REALM_INTENT_SCHEMA_V1.to_le_bytes());
        bytes[10..42].copy_from_slice(&release.release_manifest_sha256);
        bytes[42..74].copy_from_slice(&release.capability_profile_id);
        bytes[74..106].copy_from_slice(&release.program_id.to_bytes());
        bytes[106..138].copy_from_slice(&[8; 32]);
        bytes[138..170].copy_from_slice(&[9; 32]);
        bytes[170..178].copy_from_slice(&11_u64.to_le_bytes());
        bytes[178] = u8::try_from(MAX_OUTCOMES).unwrap();
        bytes[179] = PROFILE_SCHEMA_V2;
        bytes[180..].copy_from_slice(&policy_bytes);
        bytes
    }

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

    #[test]
    fn checked_revenue_intent_refuses_release_and_policy_mutations() {
        let release = revenue_release(true);
        let bytes = checked_revenue_intent(&release);
        assert!(CheckedRevenueRealmIntentV1::decode(&bytes, &release).is_ok());

        let mut foreign_release = release.clone();
        foreign_release.release_manifest_sha256 = [18; 32];
        assert_eq!(
            CheckedRevenueRealmIntentV1::decode(&bytes, &foreign_release),
            Err(CanonicalActionMaterialErrorV1::InvalidCheckedIntent)
        );

        let mut changed_rate = bytes.clone();
        changed_rate[180 + 44] ^= 1;
        assert_eq!(
            CheckedRevenueRealmIntentV1::decode(&changed_rate, &release),
            Err(CanonicalActionMaterialErrorV1::InvalidCheckedIntent)
        );

        let mut trailing = bytes;
        trailing.push(0);
        assert_eq!(
            CheckedRevenueRealmIntentV1::decode(&trailing, &release),
            Err(CanonicalActionMaterialErrorV1::InvalidCheckedIntent)
        );
    }

    #[test]
    fn realm_revenue_construction_requires_enabled_checked_coordinate() {
        let release = revenue_release(false);
        let builder = ProtocolTransactionBuilder::new(
            address(19),
            release.program_id,
            release.elf_sha256,
            crate::transaction_builder::TransactionTransport::default(),
        )
        .unwrap();
        assert_eq!(
            require_revenue_release(
                &release,
                &builder,
                RealmRevenueV2Action::InitializeFeeBearingRealmV2,
                ActionFreshnessBoundaryV1 {
                    observed_slot: 20,
                    valid_before_slot: 21,
                    maximum_validity_slots: 2,
                },
            ),
            Err(CanonicalActionMaterialErrorV1::CoordinateDisabled)
        );
    }

    #[test]
    fn treasury_service_authority_refuses_release_and_action_substitution() {
        let mut release = revenue_release(false);
        release.enabled_intents = vec![
            CanonicalIntentCoordinate {
                family_tag: GENERAL_V2_FAMILY_TAG,
                family_version: GENERAL_V2_FAMILY_VERSION,
                local_action: GeneralV2Action::InitializeSettlementRoot.tag(),
            },
            CanonicalIntentCoordinate {
                family_tag: GENERAL_V2_FAMILY_TAG,
                family_version: GENERAL_V2_FAMILY_VERSION,
                local_action: GeneralV2Action::AdvanceFeeRetirement.tag(),
            },
        ];
        let ledger = TreasuryServiceLedgerV1 {
            realm: Hash32::from_bytes([31; 32]),
            revenue_policy_record_account: Hash32::from_bytes([32; 32]),
            revenue_policy_record_v2_id: Hash32::from_bytes([33; 32]),
            market_instance_v2_id: Hash32::from_bytes([34; 32]),
            treasury_owner: Hash32::from_bytes([35; 32]),
            treasury_position_account: Hash32::from_bytes([36; 32]),
            treasury_position_founding_generation: 1,
            admitted_epoch_count: 2,
            settled_epoch_count: 1,
            rent_payer: Hash32::from_bytes([37; 32]),
            refundable_rent_principal: 10,
            donation_floor: 3,
            stored_bump: 200,
            flags: 0,
        };
        let authority = AuthenticatedTreasuryServiceLifecycleV1 {
            release_manifest_sha256: release.release_manifest_sha256,
            capability_profile_id: release.capability_profile_id,
            program_id: release.program_id,
            realm_account: address(21),
            profile_account: address(22),
            revenue_record_account: address(23),
            market_binding_account: address(24),
            market_runtime_account: address(25),
            treasury_position_account: address(26),
            treasury_replay_account: address(27),
            service_ledger_account: address(28),
            market_instance_v2_id: ledger.market_instance_v2_id,
            revenue_policy_record_v2_id: ledger.revenue_policy_record_v2_id,
            revenue_policy_v2_digest: Hash32::from_bytes([38; 32]),
            treasury_owner: ledger.treasury_owner,
            ledger,
            observed_slot: 9,
            observed_state_sha256: [39; 32],
        };
        assert!(authority.admits_general_transition(
            &release,
            GeneralV2Action::InitializeSettlementRoot
        ));
        assert!(authority.admits_general_transition(
            &release,
            GeneralV2Action::AdvanceFeeRetirement
        ));
        assert!(!authority.admits_general_transition(
            &release,
            GeneralV2Action::CloseEpoch
        ));
        release.capability_profile_id[0] ^= 1;
        assert!(!authority.admits_general_transition(
            &release,
            GeneralV2Action::InitializeSettlementRoot
        ));
    }
}
