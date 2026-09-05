//! Typed Direct capability closure for one exact market.
//!
//! The source capacity record is the sole capacity truth. This module only
//! turns its authenticated digest, the market geometry, and exact deployed
//! ProgramData widths into the finalized Direct records selected by the
//! manifest. It does not invent a family-level capacity label.

use dclutch_market::capability_manifest::CapabilityEntryV1;
#[cfg(test)]
use dclutch_market::capability_manifest::CapabilityManifestV1;
use dclutch_market::capability_program::{CapabilityProgramV1, v4::CapabilityProgramV4};
use dclutch_custody::CustodyReplayLayoutV1;
use dclutch_trading::{
    activation_bundle_v1::DirectActivationBundleV1,
    begin_retiring_bundle_v1::DirectBeginRetiringBundleV1,
    close_maker_bundle_v1::DirectCloseMakerBundleV1,
    execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3,
    native_close_bundle_v1::DirectNativeCloseBundleV1,
    ordinary_account_artifacts_v3::DirectInlineOrdinaryAccountProfileInputV3,
    ordinary_bundle_v4::{
        DirectInlineOrdinaryHotBundleInputV4, DirectInlineOrdinaryHotBundleV4,
        build_direct_inline_ordinary_hot_bundle_v4,
    },
    ordinary_effect_artifacts_v3::{
        DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3, DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3,
    },
    ordinary_geometry_v3::{DirectOrdinaryGeometryErrorV3, DirectOrdinaryGeometryV3},
    program_set_v4::{
        DirectInlineOrdinaryLifecycleProgramSetV1,
        build_direct_inline_ordinary_lifecycle_program_set_v1,
        validate_direct_inline_ordinary_lifecycle_program_set_v1,
    },
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_ROOT_SCHEMA_ID_V1, DIRECT_ROOT_STATE_BYTES_V1,
        DirectExecutionConfigV1,
    },
};
use dclutch_product::admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_registry::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
use dclutch_registry::svm::{
    LOADER_V3_PROGRAM_BYTES, LOADER_V3_PROGRAMDATA_METADATA_BYTES, ProgramDataV3View, ProgramV3View,
};
use dclutch_registry::release_set::{
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2,
    ProtocolInfrastructureProfileV1, ProtocolInfrastructureProfileV2,
};
use dclutch_market::rent::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;
use dclutch_operator::representation_composition::native_categorical_v1::{
    NativeCategoricalCompositionInputV1, compile_native_categorical_composition_v1,
};
use sha2::{Digest as _, Sha256};
use solana_sdk::{pubkey::Pubkey, rent::Rent};
use solana_sdk_ids::{bpf_loader_upgradeable, sysvar};
use std::{collections::BTreeSet, io::Read as _, path::Path};

use crate::{
    Error, Result,
    cluster::{ClusterOriginV1, DEVNET_GENESIS_HASH},
    model::{
        CheckedDeploymentDispositionV1, CheckedUpgradeRolePinV1, DirectMarketCapabilityV1,
        MarketRunInput, ProgramPin, SuccessorPlan,
    },
    plan::hex,
    rpc::{Rpc, RpcAccount, WritePolicyV1},
    runtime::decode_hex,
};

const TOKEN_MINT_BYTES: u32 = 82;
const TOKEN_ACCOUNT_BYTES: u32 = 165;

/// Exact complete Direct child-root width: common capability header plus the
/// Direct-owned tail. The devnet planner quotes Rent for this width, never for
/// a detached caller number.
pub(crate) const DIRECT_CAPABILITY_ROOT_BYTES_V1: usize =
    dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1
        + DIRECT_ROOT_STATE_BYTES_V1;

const _: () = assert!(DIRECT_CAPABILITY_ROOT_BYTES_V1 == 256);

/// Provisional devnet policy: lazy Direct activation remains available for
/// 216,000 slots after the planner's finalized observation.
///
/// At Solana's target 400 ms slot time this is approximately 24 hours, but the
/// protocol fact is the exact slot count, not a wall-clock promise. It is long
/// enough to found, publish, and activate the first public-market route while
/// remaining finite. Replace this provisional smoke policy with a recorded
/// market-class policy before any production-cluster use; callers cannot widen
/// or shorten it with a scalar flag.
pub(crate) const DEVNET_DIRECT_ACTIVATION_WINDOW_SLOTS_V1: u64 = 216_000;

/// Provisional host-input ceiling for the checked plan document. The current
/// mixed-set plan embeds two carried ProgramData accounts and remains well below
/// this bound. Lift only alongside an explicit plan-format size budget.
const MAX_DIRECT_DEVNET_PLAN_BYTES_V1: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectFeeSelectionV1 {
    basis_points: u16,
    recipient: Pubkey,
}

impl DirectFeeSelectionV1 {
    fn explicit(basis_points: Option<u16>, recipient: Option<Pubkey>) -> Result<Self> {
        let basis_points = basis_points.ok_or_else(|| {
            Error::new(
                "--direct-fee-basis-points is required; the first-market planner has no fee default",
            )
        })?;
        let recipient = recipient.ok_or_else(|| {
            Error::new(
                "--direct-fee-recipient is required; the first-market planner has no recipient default",
            )
        })?;
        DirectExecutionConfigV1::new(1, basis_points, recipient.to_bytes())
            .map_err(|error| Error::new(format!("explicit Direct fee policy: {error:?}")))?;
        Ok(Self {
            basis_points,
            recipient,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectDeploymentWidthsV1 {
    trading_programdata_bytes: u32,
    claims_programdata_bytes: u32,
    core_programdata_bytes: u32,
}

impl DirectDeploymentWidthsV1 {
    pub(crate) fn from_plan(plan: &SuccessorPlan) -> Result<Self> {
        Ok(Self {
            trading_programdata_bytes: programdata_bytes(&plan.trading)?,
            claims_programdata_bytes: programdata_bytes(&plan.claims)?,
            core_programdata_bytes: programdata_bytes(&plan.core)?,
        })
    }

    #[cfg(test)]
    pub(crate) fn new(trading: u32, claims: u32, core: u32) -> Result<Self> {
        if [trading, claims, core].contains(&0) {
            return Err(Error::new("Direct ProgramData widths must be positive"));
        }
        Ok(Self {
            trading_programdata_bytes: trading,
            claims_programdata_bytes: claims,
            core_programdata_bytes: core,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DirectDevnetPolicyObservationV1 {
    pub(crate) finalized_slot: u64,
    root_rent_minimum_lamports: u64,
    /// The canonical finalized Rent, so a family compiler can quote its own
    /// root width from the same snapshot Direct quoted 256 bytes from.
    pub(crate) rent: Rent,
    deployment: DirectDeploymentWidthsV1,
}

trait DirectPlanEvidenceV1 {
    fn reauthenticate_checked_set(
        &self,
        checked: &crate::model::CheckedUpgradeSetPinV1,
    ) -> Result<()>;

    fn authenticate_activation(&self, plan: &SuccessorPlan) -> Result<()>;
}

struct ProductionDirectPlanEvidenceV1;

impl DirectPlanEvidenceV1 for ProductionDirectPlanEvidenceV1 {
    fn reauthenticate_checked_set(
        &self,
        checked: &crate::model::CheckedUpgradeSetPinV1,
    ) -> Result<()> {
        crate::upgrade::reauthenticate_checked_deployment_set_pin(checked)
    }

    fn authenticate_activation(&self, plan: &SuccessorPlan) -> Result<()> {
        crate::runtime::authenticate_checked_activation_projection(plan)
    }
}

trait DirectFinalizedSnapshotV1 {
    fn finalized_accounts(
        &mut self,
        addresses: &[Pubkey],
        minimum_slot: u64,
    ) -> Result<(u64, Vec<Option<RpcAccount>>)>;
}

impl DirectFinalizedSnapshotV1 for Rpc {
    fn finalized_accounts(
        &mut self,
        addresses: &[Pubkey],
        minimum_slot: u64,
    ) -> Result<(u64, Vec<Option<RpcAccount>>)> {
        Rpc::finalized_accounts(self, addresses, minimum_slot)
    }
}

fn decode_exact_json_v1<T>(bytes: &[u8], label: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    let value = crate::rpc::parse_json_without_duplicate_keys_v1(bytes)
        .map_err(|error| Error::new(format!("{label} {error}")))?;
    let parsed: T = serde_json::from_value(value.clone())
        .map_err(|error| Error::new(format!("{label} shape: {error}")))?;
    let canonical = serde_json::to_value(&parsed)
        .map_err(|error| Error::new(format!("{label} projection: {error}")))?;
    if canonical != value {
        return Err(Error::new(format!(
            "{label} contains an unknown, defaulted, or noncanonical field"
        )));
    }
    Ok(parsed)
}

pub(crate) fn read_exact_json_v1<T>(path: &Path, label: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    let advertised_len = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    if !metadata.is_file()
        || advertised_len == 0
        || advertised_len > MAX_DIRECT_DEVNET_PLAN_BYTES_V1
    {
        return Err(Error::new(format!(
            "{label} must be a regular file containing 1 through {MAX_DIRECT_DEVNET_PLAN_BYTES_V1} bytes"
        )));
    }
    let mut bytes = Vec::with_capacity(advertised_len);
    file.take(u64::try_from(MAX_DIRECT_DEVNET_PLAN_BYTES_V1).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() != advertised_len {
        return Err(Error::new(format!(
            "{label} changed while it was read or exceeded {MAX_DIRECT_DEVNET_PLAN_BYTES_V1} bytes"
        )));
    }
    decode_exact_json_v1(&bytes, label)
}

fn authenticate_devnet_plan_v1<E: DirectPlanEvidenceV1>(
    plan: &SuccessorPlan,
    registry: Pubkey,
    evidence_authenticator: &E,
) -> Result<()> {
    if plan.schema != crate::model::SUCCESSOR_PLAN_SCHEMA_V3
        || plan.record_publication != "transaction"
        || crate::plan::pubkey(&plan.registry.program_id)? != registry
    {
        return Err(Error::new(
            "Direct planner requires the exact transaction-published successor plan and Registry",
        ));
    }
    if decode_hex(&plan.trading.semantic_release_id)?
        != dclutch_trading::COMPILED_DIRECT_RELEASE_ID_V1
    {
        return Err(Error::new(
            "Direct compiler requires the Trading COMPILED_DIRECT_RELEASE_ID_V1 semantic owner",
        ));
    }
    if plan.checked_local_mutable_set.is_some() {
        return Err(Error::new(
            "Direct devnet planning refuses a plan that also carries owned-loopback deployment evidence",
        ));
    }
    let Some(checked) = plan.checked_upgrade_set.as_ref() else {
        // A FOUNDED cohort has no deployment set, and that is not a missing
        // input. The checked set is a SECOND COPY of facts, and every
        // `evidence.* != pin.*` comparison below is a cross-copy consistency
        // check: it proves the set did not lie about the plan. A cohort that
        // succeeds nothing was never upgraded, so there are no Upgrade
        // receipts to form that second copy, and the pins are the only source
        // -- read directly off the ProgramData accounts.
        //
        // So this arm keeps every check that HAS one source and drops only the
        // ones that compare a copy against itself. The two arms are disjoint
        // by construction: this one requires the checked set to be absent and
        // every role to be observed, so a succession can never fall into it.
        return authenticate_devnet_genesis_plan_v1(plan, evidence_authenticator);
    };
    if checked.schema != crate::upgrade::CHECKED_SET_PREPARE_SCHEMA
        || checked.semantic_derivation != crate::upgrade::SEMANTIC_DERIVATION_V1
        || checked.roles.len() != 7
        || checked.devnet_genesis_hash != DEVNET_GENESIS_HASH
    {
        return Err(Error::new(
            "checked deployment-set schema, role closure, semantic derivation, or devnet genesis is invalid",
        ));
    }
    evidence_authenticator.reauthenticate_checked_set(checked)?;
    let retained = crate::plan::pubkey(&checked.retained_upgrade_authority)?;
    let retained_text = retained.to_string();
    let mut loader_coordinates = BTreeSet::new();
    for (role, pin, disposition) in checked_plan_roles_v1(plan) {
        let evidence = checked_role_v1(checked, role)?;
        let program = crate::plan::pubkey(&pin.program_id)?;
        let programdata = crate::plan::pubkey(&pin.programdata_id)?;
        // The table above says what KIND of row each role is: the two
        // carry-forward rows are exactly that and nothing else, while a role the
        // cut owns may be satisfied either by an Upgrade receipt or -- when this
        // cut did not change its bytes -- by proven equality with the checked
        // candidate. Both are legitimate here and neither is legitimate for
        // registry or rent, so the carry-forward arm stays an exact match.
        let disposition_matches = disposition.admits(evidence.disposition);
        if !disposition_matches
            || evidence.program_id != pin.program_id
            || evidence.programdata_id != pin.programdata_id
            || evidence.checked_candidate_elf_path != pin.checked_candidate_elf_path
            || evidence.checked_candidate_elf_sha256 != pin.checked_candidate_elf_sha256
            || evidence.live_elf_sha256 != pin.live_elf_sha256
            || evidence.deployment_slot != pin.deployment_slot
            || evidence.programdata_account_sha256 != pin.programdata_sha256
            || evidence.semantic_release_id != pin.semantic_release_id
            || pin.elf_path != pin.checked_candidate_elf_path
            || pin.elf_sha256 != pin.checked_candidate_elf_sha256
            || pin.upgrade_authority.as_deref() != Some(retained_text.as_str())
            || pin.deployment_source != "observed-programdata-account"
            || !loader_coordinates.insert(program)
            || !loader_coordinates.insert(programdata)
        {
            return Err(Error::new(format!(
                "checked deployment-set role {role} differs from the exact Direct plan pin"
            )));
        }
    }
    if loader_coordinates.len() != 14 {
        return Err(Error::new(
            "Direct deployment plan must contain 14 pairwise-distinct Loader coordinates",
        ));
    }
    evidence_authenticator.authenticate_activation(plan)?;
    Ok(())
}

/// Authenticate a devnet plan for a cohort that succeeds nothing.
///
/// Every role must be an OBSERVED ProgramData account carrying one shared
/// upgrade authority, the two ELF projections must agree, and the fourteen
/// Loader coordinates must be pairwise distinct -- the same closure the
/// deployment-set arm requires, proven from the observations themselves rather
/// than from a second copy that a founding never produces.
fn authenticate_devnet_genesis_plan_v1<E: DirectPlanEvidenceV1>(
    plan: &SuccessorPlan,
    evidence_authenticator: &E,
) -> Result<()> {
    let mut loader_coordinates = BTreeSet::new();
    let mut declared_authority: Option<String> = None;
    let mut roles = 0_usize;
    for (role, pin, _disposition) in checked_plan_roles_v1(plan) {
        roles += 1;
        let program = crate::plan::pubkey(&pin.program_id)?;
        let programdata = crate::plan::pubkey(&pin.programdata_id)?;
        let authority = pin.upgrade_authority.as_deref().ok_or_else(|| {
            Error::new(format!(
                "founded devnet role {role} has no observed upgrade authority; a founded cohort \
                 is admitted only as observed mutable accounts, never as a fabricated install"
            ))
        })?;
        // ONE authority across all seven. Seven roles under seven authorities
        // is not one cohort, and nothing downstream would notice.
        match &declared_authority {
            None => declared_authority = Some(authority.to_owned()),
            Some(first) if first != authority => {
                return Err(Error::new(format!(
                    "founded devnet role {role} carries a different upgrade authority than its \
                     cohort"
                )));
            }
            Some(_) => {}
        }
        if pin.elf_path != pin.checked_candidate_elf_path
            || pin.elf_sha256 != pin.checked_candidate_elf_sha256
            || pin.deployment_source != "observed-programdata-account"
            || pin.live_elf_sha256.is_empty()
            || pin.deployment_slot == 0
            || !loader_coordinates.insert(program)
            || !loader_coordinates.insert(programdata)
        {
            return Err(Error::new(format!(
                "founded devnet role {role} is not an exact observed deployment"
            )));
        }
    }
    if roles != 7 {
        return Err(Error::new(
            "a founded devnet plan must pin exactly seven roles",
        ));
    }
    if loader_coordinates.len() != 14 {
        return Err(Error::new(
            "Direct deployment plan must contain 14 pairwise-distinct Loader coordinates",
        ));
    }
    evidence_authenticator.authenticate_activation(plan)?;
    Ok(())
}

fn authenticate_local_plan_v1(plan: &SuccessorPlan, registry: Pubkey) -> Result<u64> {
    if plan.schema != crate::model::SUCCESSOR_PLAN_SCHEMA_V3
        || plan.record_publication != "transaction"
        || crate::plan::pubkey(&plan.registry.program_id)? != registry
    {
        return Err(Error::new(
            "Direct localhost planning requires the exact transaction-published successor plan and Registry",
        ));
    }
    if decode_hex(&plan.trading.semantic_release_id)?
        != dclutch_trading::COMPILED_DIRECT_RELEASE_ID_V1
    {
        return Err(Error::new(
            "Direct compiler requires the Trading COMPILED_DIRECT_RELEASE_ID_V1 semantic owner",
        ));
    }
    crate::local_mutable::authenticate_checked_local_mutable_plan_v1(plan)?;
    crate::runtime::authenticate_checked_activation_projection(plan)?;
    plan.checked_local_mutable_set
        .as_ref()
        .and_then(|checked| checked.roles.iter().map(|role| role.deployment_slot).max())
        .ok_or_else(|| Error::new("checked local mutable set omitted every deployment slot"))
}

fn require_complete_local_succession_v1(state: crate::campaign::StageStateV1) -> Result<()> {
    match state {
        crate::campaign::StageStateV1::Complete => Ok(()),
        crate::campaign::StageStateV1::Absent => Err(Error::new(
            "Direct localhost observation requires the planned Registry succession to exist",
        )),
        crate::campaign::StageStateV1::Partial(detail) => Err(Error::new(format!(
            "Direct localhost observation requires complete Registry succession: {detail}"
        ))),
        crate::campaign::StageStateV1::Conflict(detail) => Err(Error::new(format!(
            "Direct localhost observation refuses conflicting Registry succession: {detail}"
        ))),
    }
}

/// Select the live Registry pin only after `campaign::succession_state` has
/// authenticated the complete plan-owned three-write succession.  The
/// persisted plan remains the predecessor history; this projection changes
/// only the three facts necessarily created by the Loader write and successor
/// artifact record.
fn authenticated_successor_registry_pin_v1(
    plan: &SuccessorPlan,
    programdata: &RpcAccount,
    v2_profile: &RpcAccount,
) -> Result<ProgramPin> {
    let succession = plan.infrastructure_succession.as_ref().ok_or_else(|| {
        Error::new("Direct localhost successor selection omitted its succession plan pin")
    })?;
    let core = crate::plan::pubkey(&plan.core.program_id)?;
    let registry = crate::plan::pubkey(&plan.registry.program_id)?;
    let rent = crate::plan::pubkey(&plan.rent_credit.program_id)?;
    let expected_authority = plan
        .registry
        .upgrade_authority
        .as_deref()
        .map(crate::plan::pubkey)
        .transpose()?
        .map(|authority| authority.to_bytes());
    let programdata_view = ProgramDataV3View::parse(&programdata.data)
        .map_err(|error| Error::new(format!("successor Registry ProgramData: {error:?}")))?;
    if programdata.owner != bpf_loader_upgradeable::ID
        || programdata.executable
        || programdata_view.deployment_slot() <= plan.registry.deployment_slot
        || programdata_view.upgrade_authority() != expected_authority
        || hex(&Sha256::digest(programdata_view.elf())) != plan.registry.live_elf_sha256
        || succession.registry_candidate_elf_sha256 != plan.registry.checked_candidate_elf_sha256
    {
        return Err(Error::new(
            "successor Registry ProgramData changed its pinned slot direction, authority, or ELF",
        ));
    }

    let predecessor_bytes = decode_hex(&plan.infrastructure_profile.body_hex)?;
    let predecessor = ProtocolInfrastructureProfileV1::decode(&predecessor_bytes)
        .map_err(|error| Error::new(format!("predecessor infrastructure profile: {error:?}")))?;
    let successor = ProtocolInfrastructureProfileV2::decode(&v2_profile.data)
        .map_err(|error| Error::new(format!("successor infrastructure profile: {error:?}")))?;
    if v2_profile.owner != core
        || v2_profile.executable
        || v2_profile.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
        || predecessor.registry().program().to_bytes() != registry.to_bytes()
        || predecessor.rent().program().to_bytes() != rent.to_bytes()
        || succession.predecessor_registry_artifact_release_id
            != hex(predecessor.registry().artifact_release().as_bytes())
        || succession.predecessor_rent_artifact_release_id
            != hex(predecessor.rent().artifact_release().as_bytes())
        || successor.registry().program() != predecessor.registry().program()
        || successor.rent() != predecessor.rent()
        || successor.predecessor_registry_artifact() != predecessor.registry().artifact_release()
        || successor.predecessor_rent_artifact() != predecessor.rent().artifact_release()
        || successor.registry().artifact_release() == predecessor.registry().artifact_release()
    {
        return Err(Error::new(
            "successor infrastructure profile changed its program or predecessor lineage",
        ));
    }

    let mut selected = plan.registry.clone();
    selected.deployment_slot = programdata_view.deployment_slot();
    selected.programdata_sha256 = hex(&Sha256::digest(&programdata.data));
    selected.artifact_release_id = hex(successor.registry().artifact_release().as_bytes());
    Ok(selected)
}

pub(crate) fn authenticated_resolution_release_v1(plan: &SuccessorPlan) -> Result<[u8; 32]> {
    decode_hex(&plan.resolution.semantic_release_id)?
        .try_into()
        .map_err(|_| Error::new("Resolution semantic release ID was not exactly 32 bytes"))
}

fn checked_plan_roles_v1(
    plan: &SuccessorPlan,
) -> [(&'static str, &ProgramPin, CheckedDeploymentDispositionV1); 7] {
    [
        (
            "registry",
            &plan.registry,
            CheckedDeploymentDispositionV1::CarryForward,
        ),
        (
            "rent",
            &plan.rent_credit,
            CheckedDeploymentDispositionV1::CarryForward,
        ),
        (
            "custody",
            &plan.custody,
            CheckedDeploymentDispositionV1::Upgrade,
        ),
        (
            "resolution",
            &plan.resolution,
            CheckedDeploymentDispositionV1::Upgrade,
        ),
        (
            "claims",
            &plan.claims,
            CheckedDeploymentDispositionV1::Upgrade,
        ),
        (
            "trading",
            &plan.trading,
            CheckedDeploymentDispositionV1::Upgrade,
        ),
        ("core", &plan.core, CheckedDeploymentDispositionV1::Upgrade),
    ]
}

fn checked_role_v1<'a>(
    checked: &'a crate::model::CheckedUpgradeSetPinV1,
    role: &str,
) -> Result<&'a CheckedUpgradeRolePinV1> {
    let mut matches = checked
        .roles
        .iter()
        .filter(|candidate| candidate.role == role);
    let selected = matches
        .next()
        .ok_or_else(|| Error::new(format!("checked deployment set omitted role {role}")))?;
    if matches.next().is_some() {
        return Err(Error::new(format!(
            "checked deployment set repeated role {role}"
        )));
    }
    Ok(selected)
}

fn observe_devnet_policy_v1<R: DirectFinalizedSnapshotV1>(
    rpc: &mut R,
    plan: &SuccessorPlan,
) -> Result<DirectDevnetPolicyObservationV1> {
    // The set is used here for ONE thing: the slot floor that forces the
    // finalized read to be at least as recent as every observation the plan
    // rests on. A founded cohort carries the same slots in its own role pins,
    // read off the ProgramData accounts, so the floor is the max of those --
    // the identical guarantee from the identical facts, minus a second copy a
    // founding never produces.
    let floor = match plan.checked_upgrade_set.as_ref() {
        Some(checked) => checked
            .roles
            .iter()
            .map(|role| role.deployment_slot)
            .chain(std::iter::once(
                checked.infrastructure_carry_forward.context_slot,
            ))
            .max()
            .ok_or_else(|| Error::new("checked deployment set omitted every observation slot"))?,
        None => checked_plan_roles_v1(plan)
            .into_iter()
            .map(|(_, pin, _)| pin.deployment_slot)
            .max()
            .ok_or_else(|| Error::new("founded devnet plan omitted every observation slot"))?,
    };
    observe_policy_v1(rpc, plan, floor)
}

fn observe_policy_v1<R: DirectFinalizedSnapshotV1>(
    rpc: &mut R,
    plan: &SuccessorPlan,
    floor: u64,
) -> Result<DirectDevnetPolicyObservationV1> {
    let roles = checked_plan_roles_v1(plan);
    let mut addresses = Vec::with_capacity(1 + roles.len() * 2);
    addresses.push(sysvar::rent::ID);
    for (_, pin, _) in roles {
        addresses.push(crate::plan::pubkey(&pin.program_id)?);
        addresses.push(crate::plan::pubkey(&pin.programdata_id)?);
    }
    let (finalized_slot, accounts) = rpc.finalized_accounts(&addresses, floor)?;
    if finalized_slot < floor || accounts.len() != addresses.len() {
        return Err(Error::new(
            "finalized Direct snapshot was below its checked floor or changed its exact 15-account width",
        ));
    }
    let rent_account = accounts
        .first()
        .and_then(Option::as_ref)
        .ok_or_else(|| Error::new("finalized snapshot omitted the Rent sysvar"))?;
    if rent_account.owner != sysvar::ID || rent_account.executable {
        return Err(Error::new(
            "finalized Rent account did not have canonical sysvar ownership",
        ));
    }
    let rent = canonical_rent_v1(&rent_account.data)?;
    let root_rent_minimum_lamports = direct_root_rent_minimum_v1(&rent_account.data)?;
    let mut trading_programdata_bytes = None;
    let mut claims_programdata_bytes = None;
    let mut core_programdata_bytes = None;
    for (index, (role, pin, _)) in checked_plan_roles_v1(plan).into_iter().enumerate() {
        let program = accounts
            .get(1 + index * 2)
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::new(format!("finalized snapshot omitted {role} Program")))?;
        let programdata = accounts
            .get(2 + index * 2)
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::new(format!("finalized snapshot omitted {role} ProgramData")))?;
        let width = authenticate_live_role_v1(role, pin, program, programdata)?;
        match role {
            "trading" => trading_programdata_bytes = Some(width),
            "claims" => claims_programdata_bytes = Some(width),
            "core" => core_programdata_bytes = Some(width),
            _ => {}
        }
    }
    Ok(DirectDevnetPolicyObservationV1 {
        finalized_slot,
        root_rent_minimum_lamports,
        rent,
        deployment: DirectDeploymentWidthsV1 {
            trading_programdata_bytes: trading_programdata_bytes
                .ok_or_else(|| Error::new("live deployment snapshot omitted Trading width"))?,
            claims_programdata_bytes: claims_programdata_bytes
                .ok_or_else(|| Error::new("live deployment snapshot omitted Claims width"))?,
            core_programdata_bytes: core_programdata_bytes
                .ok_or_else(|| Error::new("live deployment snapshot omitted Core width"))?,
        },
    })
}

fn canonical_rent_v1(rent_sysvar: &[u8]) -> Result<Rent> {
    let rent: Rent = bincode::deserialize(rent_sysvar)
        .map_err(|error| Error::new(format!("finalized Rent sysvar: {error}")))?;
    if bincode::serialize(&rent)
        .map_err(|error| Error::new(format!("re-encode finalized Rent sysvar: {error}")))?
        != rent_sysvar
    {
        return Err(Error::new(
            "finalized Rent sysvar was not its exact canonical body",
        ));
    }
    Ok(rent)
}

fn direct_root_rent_minimum_v1(rent_sysvar: &[u8]) -> Result<u64> {
    let minimum = canonical_rent_v1(rent_sysvar)?.minimum_balance(DIRECT_CAPABILITY_ROOT_BYTES_V1);
    if minimum == 0 {
        return Err(Error::new("finalized 256-byte Direct root Rent was zero"));
    }
    Ok(minimum)
}

impl DirectDevnetPolicyObservationV1 {
    /// One offline observation, for compilers that must be tested without a
    /// cluster. `DirectMarketCompilerOwnedV1::for_test` is the same
    /// affordance one level up; this is the family-neutral one, and it exists
    /// because a family compiler's PURE half — everything after the two
    /// observations — is the part a test can actually pin.
    #[cfg(test)]
    pub(crate) fn for_test(finalized_slot: u64) -> Self {
        let rent = Rent::default();
        Self {
            finalized_slot,
            root_rent_minimum_lamports: rent.minimum_balance(DIRECT_CAPABILITY_ROOT_BYTES_V1),
            rent,
            deployment: DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037)
                .expect("test Direct deployment widths"),
        }
    }

    /// Exact Rent quote for one capability-root width from this snapshot.
    pub(crate) fn root_rent_minimum_for_width_v1(&self, root_bytes: usize) -> Result<u64> {
        let minimum = self.rent.minimum_balance(root_bytes);
        if minimum == 0 {
            return Err(Error::new(format!(
                "finalized {root_bytes}-byte capability-root Rent was zero"
            )));
        }
        Ok(minimum)
    }

    /// The finite prepaid-lazy activation deadline this snapshot anchors.
    pub(crate) fn activation_deadline_slot_v1(&self) -> Result<u64> {
        activation_deadline_v1(self.finalized_slot)
    }
}

/// The one author for "this URL is the acknowledged devnet and nothing else".
///
/// Two callers now: Direct's production planner and the family-neutral devnet
/// observation below. A second spelling of this refusal would be a second
/// place for loopback to become admissible by accident.
fn require_acknowledged_devnet_origin_v1(
    rpc_url: &str,
    devnet_acknowledgment: Option<&str>,
) -> Result<ClusterOriginV1> {
    let origin = ClusterOriginV1::parse(rpc_url, devnet_acknowledgment)?;
    if !matches!(origin, ClusterOriginV1::AcknowledgedDevnet { .. }) {
        return Err(Error::new(
            "the production Direct planner is devnet-only and refuses loopback; use the lab fixture compiler for a local validator",
        ));
    }
    Ok(origin)
}

/// Read-only DEVNET observation for ANY capability compiler: the authenticated
/// devnet plan, one open read-only connection, and one finalized policy
/// snapshot.
///
/// The exact counterpart of [`observe_local_market_policy_v1`], and the reason
/// it exists is the same: a family compiler that opened its own connection
/// would authenticate its own genesis hash, pick its own slot floor, and
/// quote Rent from its own snapshot — three chances to disagree with the
/// market graph it is compiling a capability for. The connection is handed
/// back still open so a family that must observe something FURTHER (General
/// observes its accelerator) does it against the same finalized origin rather
/// than a second one.
pub(crate) fn observe_devnet_market_policy_v1(
    plan_path: &Path,
    rpc_url: &str,
    devnet_acknowledgment: Option<&str>,
    registry: Pubkey,
) -> Result<(SuccessorPlan, Rpc, DirectDevnetPolicyObservationV1)> {
    let origin = require_acknowledged_devnet_origin_v1(rpc_url, devnet_acknowledgment)?;
    let plan = read_exact_json_v1::<SuccessorPlan>(plan_path, "successor plan")?;
    authenticate_devnet_plan_v1(&plan, registry, &ProductionDirectPlanEvidenceV1)?;
    let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;
    let observation = observe_devnet_policy_v1(&mut rpc, &plan)?;
    Ok((plan, rpc, observation))
}

/// Read-only loopback observation for ANY capability compiler: the
/// authenticated local plan plus one finalized policy snapshot. Direct's
/// `load_local` and every family-neutral market compiler share this single
/// author for the plan/floor/snapshot discipline.
pub(crate) fn observe_local_market_policy_v1(
    plan_path: &Path,
    rpc_url: &str,
    registry: Pubkey,
) -> Result<(SuccessorPlan, DirectDevnetPolicyObservationV1)> {
    let origin = ClusterOriginV1::parse(rpc_url, None)?;
    if !matches!(origin, ClusterOriginV1::Loopback { .. }) {
        return Err(Error::new(
            "the checked-mutable market planner is localhost-only",
        ));
    }
    let plan = read_exact_json_v1::<SuccessorPlan>(plan_path, "successor plan")?;
    let mut floor = authenticate_local_plan_v1(&plan, registry)?;
    let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;
    let mut observation_plan = plan.clone();
    if plan.infrastructure_succession.is_some() {
        require_complete_local_succession_v1(crate::campaign::succession_state(&mut rpc, &plan)?)?;
        let registry_programdata = crate::plan::pubkey(&plan.registry.programdata_id)?;
        let programdata = rpc.account(registry_programdata)?.ok_or_else(|| {
            Error::new("Registry ProgramData disappeared after authenticated succession")
        })?;
        let core = crate::plan::pubkey(&plan.core.program_id)?;
        let v2_address =
            Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2], &core).0;
        let v2_profile = rpc.account(v2_address)?.ok_or_else(|| {
            Error::new("V2 infrastructure profile disappeared after authenticated succession")
        })?;
        observation_plan.registry =
            authenticated_successor_registry_pin_v1(&plan, &programdata, &v2_profile)?;
        floor = floor.max(observation_plan.registry.deployment_slot);
    }
    let observation = observe_policy_v1(&mut rpc, &observation_plan, floor)?;
    Ok((plan, observation))
}

fn authenticate_live_role_v1(
    role: &str,
    pin: &ProgramPin,
    program: &RpcAccount,
    programdata: &RpcAccount,
) -> Result<u32> {
    let program_key = crate::plan::pubkey(&pin.program_id)?;
    let programdata_key = crate::plan::pubkey(&pin.programdata_id)?;
    let expected_programdata =
        Pubkey::find_program_address(&[program_key.as_ref()], &bpf_loader_upgradeable::ID).0;
    let program_view = ProgramV3View::parse(&program.data)
        .map_err(|error| Error::new(format!("{role} Program account: {error:?}")))?;
    let programdata_view = ProgramDataV3View::parse(&programdata.data)
        .map_err(|error| Error::new(format!("{role} ProgramData account: {error:?}")))?;
    let authority = pin
        .upgrade_authority
        .as_deref()
        .map(crate::plan::pubkey)
        .transpose()?
        .map(|key| key.to_bytes());
    let candidate = std::fs::read(&pin.checked_candidate_elf_path)
        .map_err(|error| Error::new(format!("read {role} checked candidate: {error}")))?;
    let live = programdata_view.elf();
    if program.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || program.data.len() != LOADER_V3_PROGRAM_BYTES
        || programdata_key != expected_programdata
        || program_view.programdata() != programdata_key.to_bytes()
        || programdata.owner != bpf_loader_upgradeable::ID
        || programdata.executable
        || hex(&Sha256::digest(&programdata.data)) != pin.programdata_sha256
        || programdata_view.deployment_slot() != pin.deployment_slot
        || programdata_view.upgrade_authority() != authority
        || hex(&Sha256::digest(live)) != pin.live_elf_sha256
        || hex(&Sha256::digest(&candidate)) != pin.checked_candidate_elf_sha256
        || live.get(..candidate.len()) != Some(candidate.as_slice())
        || live.get(candidate.len()..).is_none_or(|padding| {
            padding.len() != pin.live_elf_padding_bytes || padding.iter().any(|byte| *byte != 0)
        })
    {
        return Err(Error::new(format!(
            "live {role} Program/ProgramData/link/slot/authority/ELF differs from the authenticated Direct plan"
        )));
    }
    checked_nonzero_width(programdata.data.len())
}

fn activation_deadline_v1(finalized_slot: u64) -> Result<u64> {
    finalized_slot
        .checked_add(DEVNET_DIRECT_ACTIVATION_WINDOW_SLOTS_V1)
        .ok_or_else(|| Error::new("Direct devnet activation deadline overflowed u64"))
}

fn direct_execution_config_v1(
    market: &MarketRunInput,
    fee: DirectFeeSelectionV1,
) -> Result<DirectExecutionConfigV1> {
    let price_scale = 10_u64
        .checked_pow(u32::from(market.collateral_display_decimals))
        .ok_or_else(|| Error::new("Market collateral decimals overflow Direct price scale"))?;
    DirectExecutionConfigV1::new(price_scale, fee.basis_points, fee.recipient.to_bytes())
        .map_err(|error| Error::new(format!("Market-derived Direct execution config: {error:?}")))
}

pub(crate) struct DirectMarketCompilerOwnedV1 {
    deployment: DirectDeploymentWidthsV1,
    fee: DirectFeeSelectionV1,
    resolution_release: [u8; 32],
    activation_deadline_slot: u64,
    root_rent_minimum_lamports: u64,
    lifetime: (),
}

impl DirectMarketCompilerOwnedV1 {
    /// Produce one key-free, read-only Direct compiler against the exact current
    /// permanent devnet deployment.
    ///
    /// The caller chooses only the two Ember-owned fee facts and must state both.
    /// The checked mixed deployment set owns program coordinates and live widths;
    /// the Market body owns price scale; one finalized Rent-sysvar snapshot owns
    /// the 256-byte root quote and the base slot for the finite activation
    /// deadline. The RPC connection is enforced read-only and authenticates the
    /// endpoint's genesis hash before the first snapshot request.
    pub(crate) fn load_devnet(
        plan_path: &Path,
        rpc_url: &str,
        devnet_acknowledgment: Option<&str>,
        registry: Pubkey,
        fee_basis_points: Option<u16>,
        fee_recipient: Option<Pubkey>,
    ) -> Result<Self> {
        Self::load_devnet_with_v1(
            plan_path,
            rpc_url,
            devnet_acknowledgment,
            registry,
            fee_basis_points,
            fee_recipient,
            &ProductionDirectPlanEvidenceV1,
            |origin| Rpc::connect_cluster(origin, WritePolicyV1::ReadsOnly),
        )
    }

    /// Produce one key-free, read-only Direct compiler against a fresh,
    /// checked-mutable localhost deployment. The origin must be literal
    /// loopback; no acknowledgment or external RPC URL is admitted.
    pub(crate) fn load_local(
        plan_path: &Path,
        rpc_url: &str,
        registry: Pubkey,
        fee_basis_points: Option<u16>,
        fee_recipient: Option<Pubkey>,
    ) -> Result<Self> {
        let fee = DirectFeeSelectionV1::explicit(fee_basis_points, fee_recipient)?;
        let (plan, observation) = observe_local_market_policy_v1(plan_path, rpc_url, registry)?;
        Ok(Self {
            deployment: observation.deployment,
            fee,
            resolution_release: authenticated_resolution_release_v1(&plan)?,
            activation_deadline_slot: activation_deadline_v1(observation.finalized_slot)?,
            root_rent_minimum_lamports: observation.root_rent_minimum_lamports,
            lifetime: (),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn load_devnet_with_v1<E, R, C>(
        plan_path: &Path,
        rpc_url: &str,
        devnet_acknowledgment: Option<&str>,
        registry: Pubkey,
        fee_basis_points: Option<u16>,
        fee_recipient: Option<Pubkey>,
        evidence_authenticator: &E,
        connect: C,
    ) -> Result<Self>
    where
        E: DirectPlanEvidenceV1,
        R: DirectFinalizedSnapshotV1,
        C: FnOnce(&ClusterOriginV1) -> Result<R>,
    {
        let origin = require_acknowledged_devnet_origin_v1(rpc_url, devnet_acknowledgment)?;
        let plan = read_exact_json_v1::<SuccessorPlan>(plan_path, "successor plan")?;
        authenticate_devnet_plan_v1(&plan, registry, evidence_authenticator)?;
        let fee = DirectFeeSelectionV1::explicit(fee_basis_points, fee_recipient)?;
        let mut rpc = connect(&origin)?;
        let observation = observe_devnet_policy_v1(&mut rpc, &plan)?;
        Ok(Self {
            deployment: observation.deployment,
            fee,
            resolution_release: authenticated_resolution_release_v1(&plan)?,
            activation_deadline_slot: activation_deadline_v1(observation.finalized_slot)?,
            root_rent_minimum_lamports: observation.root_rent_minimum_lamports,
            lifetime: (),
        })
    }

    pub(crate) fn compiler(&self) -> DirectMarketCompilerInputV1<'_> {
        DirectMarketCompilerInputV1 {
            deployment: self.deployment,
            fee: self.fee,
            resolution_release: self.resolution_release,
            activation_deadline_slot: self.activation_deadline_slot,
            root_rent_minimum_lamports: self.root_rent_minimum_lamports,
            _lifetime: &self.lifetime,
        }
    }

    /// The Direct compiler for a LOOPBACK infrastructure-floor run, whose
    /// Market input is compiled from the run's own plan.
    ///
    /// Tier 1 has no external Market producer and cannot borrow one: its only
    /// producer was `demo-market`, retired because it cannot authenticate the
    /// permanent devnet Direct deployment, and the successor's loopback
    /// planner authenticates a checked-MUTABLE plan and refuses immutable-Core
    /// semantics -- which is what an infrastructure-floor run is. So the run
    /// compiles its own.
    ///
    /// Everything that can be read is read: the deployment widths and the
    /// Resolution release come out of the authenticated plan, and the
    /// activation deadline and the capability-root Rent minimum come out of
    /// the chain this run launched. ONE fact is a fixture and is named as one:
    /// the Direct fee policy is zero basis points paid to the Registry
    /// address, because an infrastructure-floor run has no fee recipient and
    /// inventing an economic one would be worse than declaring none. This is
    /// deliberately NOT `for_test`'s `u64::MAX` deadline and NOT its hardcoded
    /// widths; the only thing the two share is the fee choice.
    ///
    /// It cannot reach a real cluster: `runtime::rpc_origin` refuses every
    /// origin the supervisor does not launch itself, and this is called from
    /// nowhere else.
    pub(crate) fn for_loopback_plan_fixture(
        registry: Pubkey,
        plan: &SuccessorPlan,
        finalized_slot: u64,
        root_rent_minimum_lamports: u64,
    ) -> Result<Self> {
        if root_rent_minimum_lamports == 0 {
            return Err(Error::new(
                "chain-quoted Direct capability-root Rent minimum was zero",
            ));
        }
        Ok(Self {
            deployment: DirectDeploymentWidthsV1::from_plan(plan)?,
            fee: DirectFeeSelectionV1::explicit(Some(0), Some(registry))?,
            resolution_release: authenticated_resolution_release_v1(plan)?,
            activation_deadline_slot: activation_deadline_v1(finalized_slot)?,
            root_rent_minimum_lamports,
            lifetime: (),
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(registry: Pubkey, deployment: DirectDeploymentWidthsV1) -> Self {
        Self {
            deployment,
            fee: DirectFeeSelectionV1::explicit(Some(0), Some(registry))
                .expect("test Direct fee policy"),
            resolution_release: dclutch_source::resolution::RESOLUTION_CONTROLLER_RELEASE_ID_V7,
            activation_deadline_slot: u64::MAX,
            root_rent_minimum_lamports: Rent::default()
                .minimum_balance(DIRECT_CAPABILITY_ROOT_BYTES_V1),
            lifetime: (),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test_plan(registry: Pubkey, plan: &SuccessorPlan) -> Result<Self> {
        let mut compiler = Self::for_test(registry, DirectDeploymentWidthsV1::from_plan(plan)?);
        compiler.resolution_release = authenticated_resolution_release_v1(plan)?;
        Ok(compiler)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DirectMarketCompilerInputV1<'a> {
    pub(crate) deployment: DirectDeploymentWidthsV1,
    fee: DirectFeeSelectionV1,
    pub(crate) resolution_release: [u8; 32],
    pub(crate) activation_deadline_slot: u64,
    pub(crate) root_rent_minimum_lamports: u64,
    _lifetime: &'a (),
}

pub(crate) fn attach_direct_market_capability_v1(
    input: &mut MarketRunInput,
    compiler: DirectMarketCompilerInputV1<'_>,
) -> Result<()> {
    if input.direct_capability.is_some() {
        return Err(Error::new(
            "Direct market capability may be compiled only once",
        ));
    }
    let capacity_bytes = decode_hex(&input.source_capacity_profile_hex)?;
    let capacity_profile: [u8; 32] = Sha256::digest(&capacity_bytes).into();
    let source_spec =
        dclutch_source::SourceSpecV1::decode(&decode_hex(&input.source_spec_hex)?)
            .map_err(|error| Error::new(format!("SourceSpecV1: {error:?}")))?;
    if source_spec.capacity_profile_id().to_bytes() != capacity_profile {
        return Err(Error::new(
            "Direct descriptor capacity is not the exact SourceCapacityProfile body named by SourceSpecV1",
        ));
    }
    let execution_config = direct_execution_config_v1(input, compiler.fee)?;
    let execution_config_bytes = execution_config.encode();
    let config_id: [u8; 32] = Sha256::digest(execution_config_bytes).into();
    let config =
        DirectExecutionConfigV1::decode_selected(config_id, config_id, &execution_config_bytes)
            .map_err(|error| Error::new(format!("DirectExecutionConfigV1: {error:?}")))?;
    if config != execution_config || config.encode() != execution_config_bytes {
        return Err(Error::new(
            "Market-derived Direct execution config was not canonical",
        ));
    }
    let outcome_count = input
        .cuts
        .len()
        .checked_add(2)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| Error::new("Direct market outcome width overflow"))?;
    let geometry = DirectOrdinaryGeometryV3::from_outcome_count(outcome_count)
        .map_err(|error| Error::new(format!("Direct market geometry: {error:?}")))?;
    let logical_data_lengths = direct_logical_data_lengths_v1(compiler.deployment, geometry)?;
    let ordinary =
        build_direct_inline_ordinary_hot_bundle_v4(DirectInlineOrdinaryHotBundleInputV4 {
            account_profile: DirectInlineOrdinaryAccountProfileInputV3 {
                logical_data_lengths: &logical_data_lengths,
            },
            capacity_profile,
        })
        .map_err(|error| Error::new(format!("Direct ordinary bundle: {error:?}")))?;
    let release = build_direct_inline_ordinary_lifecycle_program_set_v1(ordinary, capacity_profile)
        .map_err(|error| Error::new(format!("Direct ordinary/native-close release: {error:?}")))?;
    let direct_entry = direct_manifest_entry_v1(
        &release,
        &execution_config_bytes,
        compiler.activation_deadline_slot,
        compiler.root_rent_minimum_lamports,
    )?;

    let base_bytes = decode_hex(&input.capability_manifest_hex)?;
    let (manifest, selected_manifest_entry_index) =
        crate::selected_capability::merge_selected_manifest_v1(&base_bytes, direct_entry)?;

    input.capability_manifest_hex = hex(&manifest);
    input.direct_capability = Some(DirectMarketCapabilityV1 {
        execution_config_hex: hex(&execution_config_bytes),
        ordinary_account_profile_hex: hex(&release.ordinary.account_profile),
        ordinary_lifecycle_policy_hex: hex(&release.ordinary.lifecycle_policy),
        ordinary_request_profile_hex: hex(&release.ordinary.request_profile),
        ordinary_transition_hex: hex(&release.ordinary.transition),
        ordinary_strategy_hex: hex(&release.ordinary.strategy),
        ordinary_effect_hex: hex(&release.ordinary.effect),
        ordinary_descriptor_hex: hex(&release.ordinary.descriptor),
        begin_retiring_account_profile_hex: hex(&release.begin_retiring.account_profile),
        begin_retiring_effect_hex: hex(&release.begin_retiring.effect),
        begin_retiring_descriptor_hex: hex(&release.begin_retiring.descriptor),
        native_close_account_profile_hex: hex(&release.native_close.account_profile),
        native_close_effect_hex: hex(&release.native_close.effect),
        native_close_descriptor_hex: hex(&release.native_close.descriptor),
        activation_account_profile_hex: hex(&release.activation.account_profile),
        activation_effect_hex: hex(&release.activation.effect),
        activation_descriptor_hex: hex(&release.activation.descriptor),
        close_maker_account_profile_hex: hex(&release.close_maker.account_profile),
        close_maker_effect_hex: hex(&release.close_maker.effect),
        close_maker_descriptor_hex: hex(&release.close_maker.descriptor),
        program_set_hex: hex(&release.program_set),
        activation_deadline_slot: compiler.activation_deadline_slot,
        root_rent_minimum_lamports: compiler.root_rent_minimum_lamports,
        selected_manifest_entry_index,
    });
    validate_direct_market_capability_v1(input)
}

pub(crate) fn validate_direct_market_capability_v1(input: &MarketRunInput) -> Result<()> {
    let payload = input
        .direct_capability
        .as_ref()
        .ok_or_else(|| Error::new("market input omitted its required Direct capability closure"))?;
    let capacity_bytes = decode_hex(&input.source_capacity_profile_hex)?;
    let capacity_profile: [u8; 32] = Sha256::digest(&capacity_bytes).into();
    let source_spec =
        dclutch_source::SourceSpecV1::decode(&decode_hex(&input.source_spec_hex)?)
            .map_err(|error| Error::new(format!("SourceSpecV1: {error:?}")))?;
    if source_spec.capacity_profile_id().to_bytes() != capacity_profile {
        return Err(Error::new(
            "Direct descriptor capacity is not the exact SourceCapacityProfile body named by SourceSpecV1",
        ));
    }
    let execution_config = decode_hex(&payload.execution_config_hex)?;
    let config_id: [u8; 32] = Sha256::digest(&execution_config).into();
    let config = DirectExecutionConfigV1::decode_selected(config_id, config_id, &execution_config)
        .map_err(|error| Error::new(format!("DirectExecutionConfigV1: {error:?}")))?;
    if config.encode().as_slice() != execution_config {
        return Err(Error::new("Direct execution config was not canonical"));
    }
    let release = decode_direct_release_v1(payload, capacity_profile)?;
    validate_direct_inline_ordinary_lifecycle_program_set_v1(&release, capacity_profile)
        .map_err(|error| Error::new(format!("Direct lifecycle release: {error:?}")))?;
    let descriptor = CapabilityProgramV4::decode(&release.ordinary.descriptor)
        .map_err(|error| Error::new(format!("Direct CapabilityProgramV4: {error:?}")))?;
    if descriptor.kind().to_bytes() != DIRECT_SUCCESSOR_KIND_ID_V3
        || descriptor.config_schema().to_bytes() != DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1
        || descriptor.capacity_profile().to_bytes() != capacity_profile
        || descriptor.root_schema().to_bytes() != DIRECT_ROOT_SCHEMA_ID_V1
    {
        return Err(Error::new(
            "Direct descriptor did not bind exact kind/config/capacity/root coordinates",
        ));
    }
    let expected_entry = direct_manifest_entry_v1(
        &release,
        &execution_config,
        payload.activation_deadline_slot,
        payload.root_rent_minimum_lamports,
    )?;
    let manifest_bytes = decode_hex(&input.capability_manifest_hex)?;
    crate::selected_capability::validate_selected_manifest_v1(
        &manifest_bytes,
        expected_entry,
        payload.selected_manifest_entry_index,
    )
}

pub(crate) struct DirectPublicationRecordV1 {
    pub(crate) label: &'static str,
    pub(crate) schema: [u8; 32],
    pub(crate) body: Vec<u8>,
}

/// Exact finalized Registry closure selected by the market's Direct entry.
///
/// The close Transition is embedded in its V1 descriptor and therefore is
/// not a parallel record. Every returned body is independently rejoined by
/// `validate_direct_market_capability_v1` before this function returns.
pub(crate) fn direct_publication_records_v1(
    input: &MarketRunInput,
    native_composition: NativeCategoricalCompositionInputV1<'_>,
) -> Result<Vec<DirectPublicationRecordV1>> {
    validate_direct_market_capability_v1(input)?;
    let payload = input
        .direct_capability
        .as_ref()
        .ok_or_else(|| Error::new("Direct publication omitted its typed payload"))?;
    let capacity_profile: [u8; 32] =
        Sha256::digest(decode_hex(&input.source_capacity_profile_hex)?).into();
    let release = decode_direct_release_v1(payload, capacity_profile)?;
    let descriptor = CapabilityProgramV4::decode(&release.ordinary.descriptor)
        .map_err(|error| Error::new(format!("Direct CapabilityProgramV4: {error:?}")))?;
    let record = |label, schema, body: &[u8]| DirectPublicationRecordV1 {
        label,
        schema,
        body: body.to_vec(),
    };
    let mut records = vec![
        record(
            "direct_execution_config_record",
            descriptor.config_schema().to_bytes(),
            &decode_hex(&payload.execution_config_hex)?,
        ),
        record(
            "direct_ordinary_account_profile_record",
            descriptor.account_profile().schema().to_bytes(),
            &release.ordinary.account_profile,
        ),
        record(
            "direct_ordinary_lifecycle_policy_record",
            descriptor.lifecycle().schema().to_bytes(),
            &release.ordinary.lifecycle_policy,
        ),
        record(
            "direct_ordinary_request_profile_record",
            descriptor.request_profile().schema().to_bytes(),
            &release.ordinary.request_profile,
        ),
        record(
            "direct_ordinary_transition_record",
            descriptor.transition().schema().to_bytes(),
            &release.ordinary.transition,
        ),
        record(
            "direct_ordinary_strategy_record",
            descriptor.strategy().schema().to_bytes(),
            &release.ordinary.strategy,
        ),
        record(
            "direct_ordinary_effect_record",
            descriptor.effect().schema().to_bytes(),
            &release.ordinary.effect,
        ),
        record(
            "direct_ordinary_descriptor_record",
            dclutch_market::capability_program::v4::SCHEMA_RELEASE_ID,
            &release.ordinary.descriptor,
        ),
        record(
            "direct_begin_retiring_account_profile_record",
            dclutch_trading::begin_retiring_bundle_v1::direct_begin_retiring_account_profile_schema_v1(),
            &release.begin_retiring.account_profile,
        ),
        record(
            "direct_begin_retiring_effect_record",
            dclutch_trading::begin_retiring_bundle_v1::direct_begin_retiring_effect_schema_v1(),
            &release.begin_retiring.effect,
        ),
        record(
            "direct_begin_retiring_descriptor_record",
            dclutch_trading::begin_retiring_bundle_v1::direct_begin_retiring_descriptor_schema_v1(),
            &release.begin_retiring.descriptor,
        ),
        record(
            "direct_native_close_account_profile_record",
            dclutch_trading::native_close_bundle_v1::direct_native_close_account_profile_schema_v1(),
            &release.native_close.account_profile,
        ),
        record(
            "direct_native_close_effect_record",
            dclutch_trading::native_close_bundle_v1::direct_native_close_effect_schema_v1(),
            &release.native_close.effect,
        ),
        record(
            "direct_native_close_descriptor_record",
            dclutch_trading::native_close_bundle_v1::direct_native_close_descriptor_schema_v1(),
            &release.native_close.descriptor,
        ),
        record(
            "direct_activation_account_profile_record",
            dclutch_trading::activation_bundle_v1::direct_activation_account_profile_schema_v1(),
            &release.activation.account_profile,
        ),
        record(
            "direct_activation_effect_record",
            dclutch_trading::activation_bundle_v1::direct_activation_effect_schema_v1(),
            &release.activation.effect,
        ),
        record(
            "direct_activation_descriptor_record",
            dclutch_trading::activation_bundle_v1::direct_activation_descriptor_schema_v1(),
            &release.activation.descriptor,
        ),
        record(
            "direct_close_maker_account_profile_record",
            dclutch_trading::close_maker_bundle_v1::direct_close_maker_account_profile_schema_v1(),
            &release.close_maker.account_profile,
        ),
        record(
            "direct_close_maker_effect_record",
            dclutch_trading::close_maker_bundle_v1::direct_close_maker_effect_schema_v1(),
            &release.close_maker.effect,
        ),
        record(
            "direct_close_maker_descriptor_record",
            dclutch_trading::close_maker_bundle_v1::direct_close_maker_descriptor_schema_v1(),
            &release.close_maker.descriptor,
        ),
        record(
            "direct_program_set_record",
            dclutch_market::capability_program::set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            &release.program_set,
        ),
    ];
    let native = compile_native_categorical_composition_v1(native_composition)
        .map_err(|error| Error::new(format!("native categorical composition: {error:?}")))?;
    for (label, target) in [
        "terminal_composition_descriptor_record",
        "terminal_composition_graph_record",
        "terminal_composition_translation_record",
        "terminal_composition_exposure_record",
    ]
    .into_iter()
    .zip(native.publication_targets())
    {
        records.push(record(label, target.schema_id, target.bytes));
    }
    Ok(records)
}

/// Direct's manifest entry, derived through the capability-neutral seam.
///
/// The ordinary descriptor is the single author of the kind, capacity
/// profile, root schema, and derivation policy; the program-set and config
/// bytes author their own identities. `validate_direct_market_capability_v1`
/// separately pins that the descriptor really is Direct's, so the seam's
/// derived kind cannot drift from `DIRECT_SUCCESSOR_KIND_ID_V3`.
fn direct_manifest_entry_v1(
    release: &DirectInlineOrdinaryLifecycleProgramSetV1,
    config: &[u8],
    activation_deadline_slot: u64,
    root_rent_minimum_lamports: u64,
) -> Result<CapabilityEntryV1> {
    crate::selected_capability::selected_manifest_entry_v1(
        crate::selected_capability::SelectedCapabilityClosureV1 {
            program_set: &release.program_set,
            selected_descriptor: &release.ordinary.descriptor,
            config,
            activation_deadline_slot,
            root_rent_minimum_lamports,
        },
    )
}

/// The canonical ordinary bundle one market plan witnesses.
///
/// Every consumer that regenerates a Direct release needs exactly these seven
/// bodies, and each one that spelled them out separately was a second place for
/// a field to be forgotten. The close driver reads it to hand the operator's
/// plan builder a witness rather than a list of record addresses.
pub(crate) fn direct_ordinary_bundle_v1(
    payload: &DirectMarketCapabilityV1,
) -> Result<DirectInlineOrdinaryHotBundleV4> {
    Ok(DirectInlineOrdinaryHotBundleV4 {
        account_profile: exact_array(&payload.ordinary_account_profile_hex, "ordinary profile")?,
        lifecycle_policy: exact_array(
            &payload.ordinary_lifecycle_policy_hex,
            "ordinary lifecycle",
        )?,
        request_profile: exact_array(
            &payload.ordinary_request_profile_hex,
            "ordinary request profile",
        )?,
        transition: exact_array(&payload.ordinary_transition_hex, "ordinary transition")?,
        strategy: exact_array(&payload.ordinary_strategy_hex, "ordinary strategy")?,
        effect: exact_array(&payload.ordinary_effect_hex, "ordinary effect")?,
        descriptor: exact_array(&payload.ordinary_descriptor_hex, "ordinary descriptor")?,
    })
}

fn decode_direct_release_v1(
    payload: &DirectMarketCapabilityV1,
    capacity_profile: [u8; 32],
) -> Result<DirectInlineOrdinaryLifecycleProgramSetV1> {
    let ordinary = direct_ordinary_bundle_v1(payload)?;
    let close_descriptor = decode_hex(&payload.native_close_descriptor_hex)?;
    let close = CapabilityProgramV1::decode(&close_descriptor)
        .map_err(|error| Error::new(format!("Direct native-close descriptor: {error:?}")))?;
    let account_profile = decode_hex(&payload.native_close_account_profile_hex)?;
    let effect = decode_hex(&payload.native_close_effect_hex)?;
    let native_close = DirectNativeCloseBundleV1 {
        account_profile_id: Sha256::digest(&account_profile).into(),
        effect_id: Sha256::digest(&effect).into(),
        descriptor_id: Sha256::digest(&close_descriptor).into(),
        account_profile,
        transition: close.transition_program().bytes().to_vec(),
        effect,
        descriptor: close_descriptor,
    };
    let begin_descriptor = decode_hex(&payload.begin_retiring_descriptor_hex)?;
    let begin = CapabilityProgramV1::decode(&begin_descriptor)
        .map_err(|error| Error::new(format!("Direct begin-retiring descriptor: {error:?}")))?;
    let begin_account_profile = decode_hex(&payload.begin_retiring_account_profile_hex)?;
    let begin_effect = decode_hex(&payload.begin_retiring_effect_hex)?;
    let begin_retiring = DirectBeginRetiringBundleV1 {
        account_profile_id: Sha256::digest(&begin_account_profile).into(),
        effect_id: Sha256::digest(&begin_effect).into(),
        descriptor_id: Sha256::digest(&begin_descriptor).into(),
        account_profile: begin_account_profile,
        transition: begin.transition_program().bytes().to_vec(),
        effect: begin_effect,
        descriptor: begin_descriptor,
    };
    let activation_descriptor = decode_hex(&payload.activation_descriptor_hex)?;
    let activation_program = CapabilityProgramV1::decode(&activation_descriptor)
        .map_err(|error| Error::new(format!("Direct activation descriptor: {error:?}")))?;
    let activation_account_profile = decode_hex(&payload.activation_account_profile_hex)?;
    let activation_effect = decode_hex(&payload.activation_effect_hex)?;
    let activation = DirectActivationBundleV1 {
        account_profile_id: Sha256::digest(&activation_account_profile).into(),
        effect_id: Sha256::digest(&activation_effect).into(),
        descriptor_id: Sha256::digest(&activation_descriptor).into(),
        account_profile: activation_account_profile,
        transition: activation_program.transition_program().bytes().to_vec(),
        effect: activation_effect,
        descriptor: activation_descriptor,
    };
    let close_maker_descriptor = decode_hex(&payload.close_maker_descriptor_hex)?;
    let close_maker_program = CapabilityProgramV1::decode(&close_maker_descriptor)
        .map_err(|error| Error::new(format!("Direct close-maker descriptor: {error:?}")))?;
    let close_maker_account_profile = decode_hex(&payload.close_maker_account_profile_hex)?;
    let close_maker_effect = decode_hex(&payload.close_maker_effect_hex)?;
    let close_maker = DirectCloseMakerBundleV1 {
        account_profile_id: Sha256::digest(&close_maker_account_profile).into(),
        effect_id: Sha256::digest(&close_maker_effect).into(),
        descriptor_id: Sha256::digest(&close_maker_descriptor).into(),
        account_profile: close_maker_account_profile,
        transition: close_maker_program.transition_program().bytes().to_vec(),
        effect: close_maker_effect,
        descriptor: close_maker_descriptor,
    };
    let program_set = decode_hex(&payload.program_set_hex)?;
    let release = DirectInlineOrdinaryLifecycleProgramSetV1 {
        ordinary,
        begin_retiring,
        native_close,
        activation,
        close_maker,
        program_set_id: Sha256::digest(&program_set).into(),
        program_set,
    };
    validate_direct_inline_ordinary_lifecycle_program_set_v1(&release, capacity_profile)
        .map_err(|error| Error::new(format!("Direct lifecycle release: {error:?}")))?;
    Ok(release)
}

fn exact_array<const N: usize>(value: &str, label: &str) -> Result<[u8; N]> {
    let bytes = decode_hex(value)?;
    bytes
        .try_into()
        .map_err(|_: Vec<u8>| Error::new(format!("{label} had another width than {N}")))
}

fn programdata_bytes(pin: &ProgramPin) -> Result<u32> {
    let candidate = std::fs::metadata(&pin.checked_candidate_elf_path)
        .map_err(|error| {
            Error::new(format!(
                "cannot read checked candidate {}: {error}",
                pin.checked_candidate_elf_path
            ))
        })?
        .len();
    let candidate = usize::try_from(candidate)
        .map_err(|_| Error::new("checked candidate ELF width exceeds host usize"))?;
    let width = LOADER_V3_PROGRAMDATA_METADATA_BYTES
        .checked_add(candidate)
        .and_then(|value| value.checked_add(pin.live_elf_padding_bytes))
        .ok_or_else(|| Error::new("ProgramData account width overflow"))?;
    checked_nonzero_width(width)
}

pub(crate) fn direct_logical_data_lengths_v1(
    deployment: DirectDeploymentWidthsV1,
    geometry: DirectOrdinaryGeometryV3,
) -> Result<Vec<u32>> {
    let mut output = vec![0_u32; usize::from(DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3)];
    put_width(
        &mut output,
        0,
        dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(dclutch_trading::successor::DIRECT_ROOT_STATE_BYTES_V1)
            .ok_or_else(|| Error::new("Direct root width overflow"))?,
    )?;
    put_width(
        &mut output,
        1,
        dclutch_trading::successor::DIRECT_EXECUTION_CONFIG_BYTES_V1,
    )?;
    put_width(&mut output, 2, PRODUCT_RECORD_BYTES_V2)?;
    put_geometry_width(&mut output, 3, geometry.portfolio_record_bytes())?;
    put_width(
        &mut output,
        4,
        dclutch_product::payoff::runtime_v3::BASIS_HEADER_BYTES_V3,
    )?;
    for coordinate in [5_usize, 8] {
        put_width(
            &mut output,
            coordinate,
            dclutch_trading::successor::DIRECT_MAKER_REPLAY_BYTES_V1,
        )?;
    }
    put_width(&mut output, 7, LIFECYCLE_RENT_CREDIT_BYTES_V2)?;
    put_width(&mut output, 10, LOADER_V3_PROGRAM_BYTES)?;
    put_geometry_width(&mut output, 13, geometry.claims_aggregate_record_bytes())?;
    alias_width(&mut output, 14, 4)?;
    put_width(&mut output, 16, PRODUCT_RECORD_BYTES_V2)?;
    put_geometry_width(&mut output, 18, geometry.result_domain_record_bytes())?;
    alias_width(&mut output, 20, 3)?;
    set_width(&mut output, 22, 17)?;
    put_width(&mut output, 23, dclutch_market::STATE_BYTES)?;
    put_width(&mut output, 24, ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)?;
    for coordinate in [25_usize, 26, 28, 30] {
        put_width(&mut output, coordinate, LOADER_V3_PROGRAM_BYTES)?;
    }
    set_width(&mut output, 27, deployment.trading_programdata_bytes)?;
    set_width(&mut output, 29, deployment.claims_programdata_bytes)?;
    set_width(&mut output, 31, deployment.core_programdata_bytes)?;
    for coordinate in [32_usize, 33] {
        put_geometry_width(
            &mut output,
            coordinate,
            geometry.claims_position_record_bytes(),
        )?;
    }
    alias_width(&mut output, 35, 23)?;
    alias_width(&mut output, 36, 24)?;
    alias_width(&mut output, 37, 25)?;
    alias_width(&mut output, 38, 26)?;
    alias_width(&mut output, 39, 27)?;
    put_width(&mut output, 40, dclutch_market::realm::REALM_BYTES)?;
    put_width(&mut output, 42, CustodyReplayLayoutV1::BYTES)?;
    set_width(&mut output, 43, TOKEN_MINT_BYTES)?;
    set_width(&mut output, 44, TOKEN_ACCOUNT_BYTES)?;
    set_width(&mut output, 45, TOKEN_ACCOUNT_BYTES)?;
    put_width(&mut output, 47, LOADER_V3_PROGRAM_BYTES)?;
    set_width(&mut output, 73, TOKEN_ACCOUNT_BYTES)?;
    for (account, representative) in [
        (49, 23),
        (50, 24),
        (51, 25),
        (52, 26),
        (53, 27),
        (54, 40),
        (55, 41),
        (56, 42),
        (57, 43),
        (58, 44),
        (59, 45),
        (60, 46),
        (61, 47),
        (63, 23),
        (64, 24),
        (65, 25),
        (66, 26),
        (67, 27),
        (68, 40),
        (69, 41),
        (70, 42),
        (71, 43),
        (72, 44),
        (74, 46),
        (75, 47),
        (77, 23),
        (78, 24),
        (79, 25),
        (80, 26),
        (81, 27),
        (82, 40),
        (83, 41),
        (84, 42),
        (85, 43),
        (86, 44),
        (87, 73),
        (88, 46),
        (89, 47),
    ] {
        alias_width(&mut output, account, representative)?;
    }
    put_width(
        &mut output,
        usize::from(DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3),
        LOADER_V3_PROGRAM_BYTES,
    )?;
    Ok(output)
}

fn checked_nonzero_width(value: usize) -> Result<u32> {
    let output = u32::try_from(value).map_err(|_| Error::new("Direct account width overflow"))?;
    if output == 0 {
        return Err(Error::new("Direct account width must be positive"));
    }
    Ok(output)
}

fn put_width(output: &mut [u32], coordinate: usize, value: usize) -> Result<()> {
    set_width(output, coordinate, checked_nonzero_width(value)?)
}

fn put_geometry_width(
    output: &mut [u32],
    coordinate: usize,
    value: core::result::Result<u32, DirectOrdinaryGeometryErrorV3>,
) -> Result<()> {
    set_width(
        output,
        coordinate,
        value.map_err(|error| Error::new(format!("Direct market geometry: {error:?}")))?,
    )
}

fn alias_width(output: &mut [u32], coordinate: usize, representative: usize) -> Result<()> {
    let value = *output
        .get(representative)
        .ok_or_else(|| Error::new("Direct account-profile alias is out of range"))?;
    set_width(output, coordinate, value)
}

fn set_width(output: &mut [u32], coordinate: usize, value: u32) -> Result<()> {
    *output
        .get_mut(coordinate)
        .ok_or_else(|| Error::new("Direct account-profile coordinate is out of range"))? = value;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, fs, path::PathBuf, rc::Rc};

    use dclutch_market::capability_program::set_v2::CapabilityProgramSetV2;
    use dclutch_trading::{
        activation_bundle_v1::DIRECT_ACTIVATION_SELECTOR_V1,
        close_maker_v1::DIRECT_CLOSE_MAKER_SELECTOR_V1,
        native_close_bundle_v1::DIRECT_NATIVE_CLOSE_SELECTOR_V1,
        retirement_v1::DIRECT_BEGIN_RETIRING_SELECTOR_V1,
    };
    use dclutch_registry::release_set::{ArtifactReleaseIdV1, ExecutionRoleBindingV1};

    fn test_market() -> MarketRunInput {
        let registry = Pubkey::new_from_array([0x41; 32]);
        let direct = DirectMarketCompilerOwnedV1::for_test(
            registry,
            DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037).expect("deployment widths"),
        );
        crate::market::demo_market_input(registry, direct.compiler()).expect("Direct demo market")
    }

    #[derive(Debug, serde::Deserialize, serde::Serialize)]
    struct ExactJsonFixtureV1 {
        nested: ExactJsonNestedV1,
    }

    #[derive(Debug, serde::Deserialize, serde::Serialize)]
    struct ExactJsonNestedV1 {
        value: u64,
    }

    struct FixtureRoleV1 {
        elf: PathBuf,
        sha256: String,
        deployment: crate::plan::RoleDeploymentInputV1,
    }

    struct DevnetPlannerFixtureV1 {
        root: PathBuf,
        plan_path: PathBuf,
        plan: SuccessorPlan,
        checked: crate::model::CheckedUpgradeSetPinV1,
        registry: Pubkey,
        retained_authority: Pubkey,
        addresses: Vec<Pubkey>,
        accounts: Vec<Option<RpcAccount>>,
        floor: u64,
        finalized_slot: u64,
    }

    impl Drop for DevnetPlannerFixtureV1 {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[derive(Clone)]
    struct FixturePlanEvidenceV1 {
        expected: crate::model::CheckedUpgradeSetPinV1,
        checked_calls: Rc<Cell<u32>>,
        activation_calls: Rc<Cell<u32>>,
    }

    impl DirectPlanEvidenceV1 for FixturePlanEvidenceV1 {
        fn reauthenticate_checked_set(
            &self,
            checked: &crate::model::CheckedUpgradeSetPinV1,
        ) -> Result<()> {
            self.checked_calls
                .set(self.checked_calls.get().saturating_add(1));
            if checked != &self.expected {
                return Err(Error::new(
                    "fixture checked deployment set differs from its authenticated journal",
                ));
            }
            Ok(())
        }

        fn authenticate_activation(&self, plan: &SuccessorPlan) -> Result<()> {
            self.activation_calls
                .set(self.activation_calls.get().saturating_add(1));
            crate::runtime::authenticate_checked_activation_projection(plan)
        }
    }

    struct PassThroughPlanEvidenceV1;

    impl DirectPlanEvidenceV1 for PassThroughPlanEvidenceV1 {
        fn reauthenticate_checked_set(
            &self,
            _checked: &crate::model::CheckedUpgradeSetPinV1,
        ) -> Result<()> {
            Ok(())
        }

        fn authenticate_activation(&self, plan: &SuccessorPlan) -> Result<()> {
            crate::runtime::authenticate_checked_activation_projection(plan)
        }
    }

    struct FixtureSnapshotRpcV1 {
        expected_addresses: Vec<Pubkey>,
        expected_floor: u64,
        returned_slot: u64,
        accounts: Vec<Option<RpcAccount>>,
        calls: Rc<Cell<u32>>,
    }

    impl DirectFinalizedSnapshotV1 for FixtureSnapshotRpcV1 {
        fn finalized_accounts(
            &mut self,
            addresses: &[Pubkey],
            minimum_slot: u64,
        ) -> Result<(u64, Vec<Option<RpcAccount>>)> {
            let next = self.calls.get().saturating_add(1);
            self.calls.set(next);
            if next != 1
                || addresses != self.expected_addresses
                || minimum_slot != self.expected_floor
            {
                return Err(Error::new(
                    "Direct planner changed its one-shot 15-account snapshot",
                ));
            }
            Ok((self.returned_slot, self.accounts.clone()))
        }
    }

    fn fixture_role_v1(
        root: &Path,
        label: &str,
        tag: u8,
        slot: u64,
        retained_authority: Pubkey,
    ) -> FixtureRoleV1 {
        let elf = vec![0x7f, b'E', b'L', b'F', tag];
        let elf_path = root.join(format!("{label}.so"));
        fs::write(&elf_path, &elf).expect("write fixture ELF");
        let programdata =
            crate::plan::loader_programdata_bytes(&elf, slot, Some(retained_authority));
        let observed = root.join(format!("{label}-programdata.bin"));
        fs::write(&observed, &programdata).expect("write fixture ProgramData");
        FixtureRoleV1 {
            elf: elf_path,
            sha256: hex(&Sha256::digest(&elf)),
            deployment: crate::plan::RoleDeploymentInputV1 {
                observed_programdata: Some(observed),
                observed_programdata_bytes: None,
                expected_live_elf_sha256: Some(hex(&Sha256::digest(&elf))),
                genesis_deployment_slot: 0,
                expected_upgrade_authority: Some(retained_authority),
            },
        }
    }

    /// One role's semantic release id, derived the way `prepare` re-derives it.
    fn fixture_semantic_v1(role: &str, artifact_sha256: &str) -> String {
        crate::upgrade::checked_semantic_release_id(role, artifact_sha256)
            .expect("fixture semantic release id")
    }

    fn devnet_planner_fixture_v1() -> DevnetPlannerFixtureV1 {
        // PROCESS-UNIQUE, because `Pubkey::new_unique()` is not.
        //
        // It is a monotonic counter seeded identically in every process, so two
        // runs of this binary produce the SAME sequence of names. The fixture's
        // `Drop` removes its directory, which is why this held for months -- but
        // a run that is killed rather than failed never drops, and the eight
        // planner tests then refuse `AlreadyExists` in every later run on the
        // machine, forever, naming a file-system error instead of a defect.
        // Measured 2026-09-02: forty stale directories under $TMPDIR reddened
        // the whole suite for every lane. `direct_ticket.rs:102` already spells
        // the answer; this is the same one.
        let root = std::env::temp_dir().join(format!(
            "dclutch-direct-devnet-planner-{}-{}",
            std::process::id(),
            Pubkey::new_unique()
        ));
        fs::create_dir(&root).expect("create Direct planner fixture");
        let retained_authority = Pubkey::new_from_array([0xa1; 32]);
        let registry = Pubkey::new_from_array([1; 32]);
        let registry_role = fixture_role_v1(&root, "registry", 1, 101, retained_authority);
        let core_role = fixture_role_v1(&root, "core", 2, 107, retained_authority);
        let claims_role = fixture_role_v1(&root, "claims", 3, 105, retained_authority);
        let trading_role = fixture_role_v1(&root, "trading", 4, 106, retained_authority);
        let resolution_role = fixture_role_v1(&root, "resolution", 5, 104, retained_authority);
        let custody_role = fixture_role_v1(&root, "custody", 6, 103, retained_authority);
        let rent_role = fixture_role_v1(&root, "rent", 7, 102, retained_authority);
        let registry_role_sha256 = registry_role.sha256.clone();
        let core_role_sha256 = core_role.sha256.clone();
        let claims_role_sha256 = claims_role.sha256.clone();
        let custody_role_sha256 = custody_role.sha256.clone();
        let rent_role_sha256 = rent_role.sha256.clone();
        let plan_path = root.join("plan.json");
        let mut plan = crate::plan::prepare(crate::plan::PrepareArgs {
            observed_upgrade_authority: None,
            account_dir: root.join("accounts"),
            plan_path: plan_path.clone(),
            registry_program: registry,
            registry_elf: registry_role.elf,
            registry_sha256: registry_role_sha256.clone(),
            // DERIVED, NOT INVENTED. `2da012cd` made `validate_prepare` re-derive
            // every artifact-owned semantic id from `(role label, shipped ELF
            // digest)` and refuse a mismatch, because founding under one
            // release-set identity and sealing under another is what stranded
            // cohort-12. This fixture kept its placeholder `0x11..0x17` bytes
            // and has refused ever since -- invisibly, because the refusal
            // happens INSIDE the fixture builder before its `Drop` guard
            // exists, so every run leaked its scratch directory and the next
            // run reported `AlreadyExists` instead. Two symptoms, one cause.
            registry_semantic_release_id: fixture_semantic_v1("registry", &registry_role_sha256),
            core_program: Pubkey::new_from_array([2; 32]),
            core_elf: core_role.elf,
            core_sha256: core_role_sha256.clone(),
            core_semantic_release_id: fixture_semantic_v1("core", &core_role_sha256),
            core_bootstrap_upgrade_authority: retained_authority,
            claims_program: Pubkey::new_from_array([3; 32]),
            claims_elf: claims_role.elf,
            claims_sha256: claims_role_sha256.clone(),
            claims_semantic_release_id: fixture_semantic_v1("claims", &claims_role_sha256),
            trading_program: Pubkey::new_from_array([4; 32]),
            trading_elf: trading_role.elf,
            trading_sha256: trading_role.sha256,
            trading_semantic_release_id: hex(&dclutch_trading::COMPILED_DIRECT_RELEASE_ID_V1),
            resolution_program: Pubkey::new_from_array([5; 32]),
            resolution_elf: resolution_role.elf,
            resolution_sha256: resolution_role.sha256,
            resolution_semantic_release_id: hex(
                &dclutch_source::resolution::RESOLUTION_CONTROLLER_RELEASE_ID_V7,
            ),
            custody_program: Pubkey::new_from_array([6; 32]),
            custody_elf: custody_role.elf,
            custody_sha256: custody_role_sha256.clone(),
            custody_semantic_release_id: fixture_semantic_v1("custody", &custody_role_sha256),
            record_publication: crate::plan::RecordPublicationV1::Transaction,
            deployments: crate::plan::RoleDeploymentsV1 {
                registry: registry_role.deployment,
                core: core_role.deployment,
                claims: claims_role.deployment,
                trading: trading_role.deployment,
                resolution: resolution_role.deployment,
                custody: custody_role.deployment,
                rent_credit: rent_role.deployment,
            },
            rent_credit_program: Pubkey::new_from_array([7; 32]),
            rent_credit_elf: rent_role.elf,
            rent_credit_sha256: rent_role_sha256.clone(),
            rent_credit_semantic_release_id: fixture_semantic_v1("rent", &rent_role_sha256),
            checked_upgrade_set: None,
            general_accelerator: None,
        })
        .expect("prepare Direct planner fixture");

        let roles = checked_plan_roles_v1(&plan)
            .into_iter()
            .map(|(role, pin, disposition)| {
                let carry = disposition == CheckedDeploymentDispositionV1::CarryForward;
                let record = match role {
                    "registry" => Some(&plan.records["registry_artifact_release"]),
                    "rent" => Some(&plan.records["rent_artifact_release"]),
                    _ => None,
                };
                CheckedUpgradeRolePinV1 {
                    role: role.into(),
                    disposition,
                    program_id: pin.program_id.clone(),
                    programdata_id: pin.programdata_id.clone(),
                    baseline_path: (!carry).then(|| pin.checked_candidate_elf_path.clone()),
                    baseline_sha256: (!carry).then(|| pin.checked_candidate_elf_sha256.clone()),
                    receipt_path: (!carry).then(|| {
                        root.join(format!("{role}-receipt.json"))
                            .display()
                            .to_string()
                    }),
                    receipt_sha256: (!carry).then(|| hex(&[0x31; 32])),
                    dump_path: pin.checked_candidate_elf_path.clone(),
                    dump_sha256: pin.live_elf_sha256.clone(),
                    checked_candidate_elf_path: pin.checked_candidate_elf_path.clone(),
                    checked_candidate_elf_sha256: pin.checked_candidate_elf_sha256.clone(),
                    live_elf_sha256: pin.live_elf_sha256.clone(),
                    deployment_slot: pin.deployment_slot,
                    programdata_account_sha256: pin.programdata_sha256.clone(),
                    semantic_release_id: pin.semantic_release_id.clone(),
                    artifact_release_body_hex: record.map(|pair| pair.body_hex.clone()),
                    artifact_release_id: record.map(|_| pin.artifact_release_id.clone()),
                    carried_programdata_base64: carry.then(|| "fixture-programdata".into()),
                }
            })
            .collect();
        let checked = crate::model::CheckedUpgradeSetPinV1 {
            schema: crate::upgrade::CHECKED_SET_PREPARE_SCHEMA.into(),
            journal_path: root.join("deployment-set.json").display().to_string(),
            journal_sha256: hex(&[0x21; 32]),
            final_set_sha256: hex(&[0x22; 32]),
            checked_release_gate_path: root.join("checked-release.json").display().to_string(),
            checked_release_gate_sha256: hex(&[0x23; 32]),
            source_revision: "fixture-source".into(),
            source_tree_sha256: hex(&[0x24; 32]),
            devnet_genesis_hash: DEVNET_GENESIS_HASH.into(),
            solana_cli_version: "fixture-solana".into(),
            retained_upgrade_authority: retained_authority.to_string(),
            fee_payer: Pubkey::new_from_array([0xa2; 32]).to_string(),
            semantic_derivation: crate::upgrade::SEMANTIC_DERIVATION_V1.into(),
            infrastructure_carry_forward: crate::model::CheckedInfrastructureCarryForwardPinV1 {
                snapshot_path: root.join("carry.json").display().to_string(),
                snapshot_sha256: hex(&[0x25; 32]),
                context_slot: 200,
                profile_address: plan.infrastructure_profile.address.clone(),
                profile_account_sha256: hex(&[0x26; 32]),
                profile_body_sha256: plan.infrastructure_profile.body_sha256.clone(),
                profile_body_hex: plan.infrastructure_profile.body_hex.clone(),
                registry_raw_address: plan.records["registry_artifact_release"].raw.clone(),
                registry_staging_address: plan.records["registry_artifact_release"].staging.clone(),
                registry_programdata_account_sha256: plan.registry.programdata_sha256.clone(),
                rent_raw_address: plan.records["rent_artifact_release"].raw.clone(),
                rent_staging_address: plan.records["rent_artifact_release"].staging.clone(),
                rent_programdata_account_sha256: plan.rent_credit.programdata_sha256.clone(),
            },
            roles,
        };
        plan.checked_upgrade_set = Some(checked.clone());
        fs::write(
            &plan_path,
            serde_json::to_vec_pretty(&plan).expect("encode Direct planner plan"),
        )
        .expect("replace Direct planner plan");

        let mut addresses = vec![sysvar::rent::ID];
        let mut accounts = vec![Some(RpcAccount {
            lamports: 1,
            owner: sysvar::ID,
            executable: false,
            rent_epoch: 0,
            data: bincode::serialize(&Rent::default()).expect("Rent sysvar"),
        })];
        for (_, pin, _) in checked_plan_roles_v1(&plan) {
            let program = crate::plan::pubkey(&pin.program_id).expect("fixture Program");
            let programdata =
                crate::plan::pubkey(&pin.programdata_id).expect("fixture ProgramData");
            let mut program_body = Vec::with_capacity(LOADER_V3_PROGRAM_BYTES);
            program_body.extend_from_slice(&2_u32.to_le_bytes());
            program_body.extend_from_slice(programdata.as_ref());
            let candidate = fs::read(&pin.checked_candidate_elf_path).expect("fixture candidate");
            let programdata_body = crate::plan::loader_programdata_bytes(
                &candidate,
                pin.deployment_slot,
                Some(retained_authority),
            );
            assert_eq!(
                hex(&Sha256::digest(&programdata_body)),
                pin.programdata_sha256
            );
            addresses.extend([program, programdata]);
            accounts.extend([
                Some(RpcAccount {
                    lamports: 1,
                    owner: bpf_loader_upgradeable::ID,
                    executable: true,
                    rent_epoch: 0,
                    data: program_body,
                }),
                Some(RpcAccount {
                    lamports: 1,
                    owner: bpf_loader_upgradeable::ID,
                    executable: false,
                    rent_epoch: 0,
                    data: programdata_body,
                }),
            ]);
        }
        let floor = 200;
        DevnetPlannerFixtureV1 {
            root,
            plan_path,
            plan,
            checked,
            registry,
            retained_authority,
            addresses,
            accounts,
            floor,
            finalized_slot: 240,
        }
    }

    fn write_fixture_plan_v1(fixture: &DevnetPlannerFixtureV1, plan: &SuccessorPlan) {
        fs::write(
            &fixture.plan_path,
            serde_json::to_vec_pretty(plan).expect("encode hostile plan"),
        )
        .expect("write hostile plan");
    }

    struct FixtureLoadProbeV1 {
        result: Result<DirectMarketCompilerOwnedV1>,
        checked_calls: Rc<Cell<u32>>,
        activation_calls: Rc<Cell<u32>>,
        rpc_calls: Rc<Cell<u32>>,
    }

    fn load_fixture_v1(
        fixture: &DevnetPlannerFixtureV1,
        plan: &SuccessorPlan,
        accounts: Vec<Option<RpcAccount>>,
        finalized_slot: u64,
        fee_basis_points: Option<u16>,
        fee_recipient: Option<Pubkey>,
    ) -> FixtureLoadProbeV1 {
        write_fixture_plan_v1(fixture, plan);
        let checked_calls = Rc::new(Cell::new(0));
        let activation_calls = Rc::new(Cell::new(0));
        let rpc_calls = Rc::new(Cell::new(0));
        let evidence = FixturePlanEvidenceV1 {
            expected: fixture.checked.clone(),
            checked_calls: Rc::clone(&checked_calls),
            activation_calls: Rc::clone(&activation_calls),
        };
        let rpc = FixtureSnapshotRpcV1 {
            expected_addresses: fixture.addresses.clone(),
            expected_floor: fixture.floor,
            returned_slot: finalized_slot,
            accounts,
            calls: Rc::clone(&rpc_calls),
        };
        let result = DirectMarketCompilerOwnedV1::load_devnet_with_v1(
            &fixture.plan_path,
            "https://api.devnet.solana.com",
            Some(DEVNET_GENESIS_HASH),
            fixture.registry,
            fee_basis_points,
            fee_recipient,
            &evidence,
            |origin| {
                if origin.url() != "https://api.devnet.solana.com/" {
                    return Err(Error::new("fixture received the wrong devnet origin"));
                }
                Ok(rpc)
            },
        );
        FixtureLoadProbeV1 {
            result,
            checked_calls,
            activation_calls,
            rpc_calls,
        }
    }

    fn successor_registry_fixture_v1(
        fixture: &DevnetPlannerFixtureV1,
        deployment_slot: u64,
    ) -> (SuccessorPlan, RpcAccount, RpcAccount) {
        let mut plan = fixture.plan.clone();
        let predecessor = ProtocolInfrastructureProfileV1::decode(
            &decode_hex(&plan.infrastructure_profile.body_hex).expect("V1 profile bytes"),
        )
        .expect("V1 profile");
        plan.infrastructure_succession = Some(crate::model::InfrastructureSuccessionPinV1 {
            schema: crate::plan::INFRASTRUCTURE_SUCCESSION_SCHEMA_V1.into(),
            registry_upgrade_buffer: Pubkey::new_from_array([0xd0; 32]).to_string(),
            registry_candidate_elf_sha256: plan.registry.checked_candidate_elf_sha256.clone(),
            predecessor_registry_artifact_release_id: hex(predecessor
                .registry()
                .artifact_release()
                .as_bytes()),
            predecessor_rent_artifact_release_id: hex(predecessor
                .rent()
                .artifact_release()
                .as_bytes()),
        });
        let candidate =
            fs::read(&plan.registry.checked_candidate_elf_path).expect("Registry candidate ELF");
        let programdata = RpcAccount {
            lamports: 1,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
            data: crate::plan::loader_programdata_bytes(
                &candidate,
                deployment_slot,
                Some(fixture.retained_authority),
            ),
        };
        let successor_artifact =
            ArtifactReleaseIdV1::new([0xd1; 32]).expect("successor artifact id");
        let profile = ProtocolInfrastructureProfileV2::new(
            ExecutionRoleBindingV1::new(predecessor.registry().program(), successor_artifact),
            predecessor.rent(),
            predecessor.registry().artifact_release(),
            predecessor.rent().artifact_release(),
        )
        .expect("V2 profile");
        let v2_profile = RpcAccount {
            lamports: 1,
            owner: crate::plan::pubkey(&plan.core.program_id).expect("Core Program"),
            executable: false,
            rent_epoch: 0,
            data: profile.to_bytes().to_vec(),
        };
        (plan, programdata, v2_profile)
    }

    #[test]
    fn successor_registry_selection_changes_only_successor_owned_facts() {
        let fixture = devnet_planner_fixture_v1();
        let successor_slot = fixture.plan.registry.deployment_slot + 19;
        let (plan, programdata, v2_profile) =
            successor_registry_fixture_v1(&fixture, successor_slot);
        let selected = authenticated_successor_registry_pin_v1(&plan, &programdata, &v2_profile)
            .expect("authenticated successor Registry pin");
        assert_eq!(selected.deployment_slot, successor_slot);
        assert_eq!(
            selected.programdata_sha256,
            hex(&Sha256::digest(&programdata.data))
        );
        let profile =
            ProtocolInfrastructureProfileV2::decode(&v2_profile.data).expect("V2 profile");
        assert_eq!(
            selected.artifact_release_id,
            hex(profile.registry().artifact_release().as_bytes())
        );

        let mut restored = selected.clone();
        restored.deployment_slot = plan.registry.deployment_slot;
        restored.programdata_sha256 = plan.registry.programdata_sha256.clone();
        restored.artifact_release_id = plan.registry.artifact_release_id.clone();
        assert_eq!(
            serde_json::to_value(restored).expect("selected Registry pin"),
            serde_json::to_value(&plan.registry).expect("predecessor Registry pin")
        );

        let programdata_key =
            crate::plan::pubkey(&selected.programdata_id).expect("Registry ProgramData");
        let mut program_body = Vec::with_capacity(LOADER_V3_PROGRAM_BYTES);
        program_body.extend_from_slice(&2_u32.to_le_bytes());
        program_body.extend_from_slice(programdata_key.as_ref());
        let program = RpcAccount {
            lamports: 1,
            owner: bpf_loader_upgradeable::ID,
            executable: true,
            rent_epoch: 0,
            data: program_body,
        };
        authenticate_live_role_v1("registry", &selected, &program, &programdata)
            .expect("successor Registry Loader pair");

        let mut wrong_programdata_coordinate = selected.clone();
        wrong_programdata_coordinate.programdata_id = Pubkey::new_unique().to_string();
        assert!(
            authenticate_live_role_v1(
                "registry",
                &wrong_programdata_coordinate,
                &program,
                &programdata,
            )
            .is_err()
        );
        let mut wrong_program_coordinate = selected;
        wrong_program_coordinate.program_id = Pubkey::new_unique().to_string();
        assert!(
            authenticate_live_role_v1(
                "registry",
                &wrong_program_coordinate,
                &program,
                &programdata,
            )
            .is_err()
        );
    }

    #[test]
    fn successor_registry_selection_refuses_each_lineage_or_programdata_substitution() {
        let fixture = devnet_planner_fixture_v1();
        let successor_slot = fixture.plan.registry.deployment_slot + 19;
        let (plan, programdata, v2_profile) =
            successor_registry_fixture_v1(&fixture, successor_slot);

        let mut hostile_programdata = programdata.clone();
        hostile_programdata.owner = Pubkey::new_unique();
        assert!(
            authenticated_successor_registry_pin_v1(&plan, &hostile_programdata, &v2_profile)
                .is_err()
        );
        let mut hostile_programdata = programdata.clone();
        hostile_programdata.executable = true;
        assert!(
            authenticated_successor_registry_pin_v1(&plan, &hostile_programdata, &v2_profile)
                .is_err()
        );
        for slot in [
            plan.registry.deployment_slot,
            plan.registry.deployment_slot.saturating_sub(1),
        ] {
            let candidate = fs::read(&plan.registry.checked_candidate_elf_path)
                .expect("Registry candidate ELF");
            let hostile_programdata = RpcAccount {
                data: crate::plan::loader_programdata_bytes(
                    &candidate,
                    slot,
                    Some(fixture.retained_authority),
                ),
                ..programdata.clone()
            };
            assert!(
                authenticated_successor_registry_pin_v1(&plan, &hostile_programdata, &v2_profile)
                    .is_err()
            );
        }
        for (elf, authority) in [
            (
                vec![0x7f, b'E', b'L', b'F', 0xfe],
                fixture.retained_authority,
            ),
            (
                fs::read(&plan.registry.checked_candidate_elf_path)
                    .expect("Registry candidate ELF"),
                Pubkey::new_unique(),
            ),
        ] {
            let hostile_programdata = RpcAccount {
                data: crate::plan::loader_programdata_bytes(&elf, successor_slot, Some(authority)),
                ..programdata.clone()
            };
            assert!(
                authenticated_successor_registry_pin_v1(&plan, &hostile_programdata, &v2_profile)
                    .is_err()
            );
        }

        let mut hostile_profile = v2_profile.clone();
        hostile_profile.owner = Pubkey::new_unique();
        assert!(
            authenticated_successor_registry_pin_v1(&plan, &programdata, &hostile_profile).is_err()
        );
        let mut hostile_profile = v2_profile.clone();
        hostile_profile.executable = true;
        assert!(
            authenticated_successor_registry_pin_v1(&plan, &programdata, &hostile_profile).is_err()
        );

        let predecessor = ProtocolInfrastructureProfileV1::decode(
            &decode_hex(&plan.infrastructure_profile.body_hex).expect("V1 profile bytes"),
        )
        .expect("V1 profile");
        let successor = ProtocolInfrastructureProfileV2::decode(&v2_profile.data)
            .expect("successor profile")
            .registry()
            .artifact_release();
        for profile in [
            ProtocolInfrastructureProfileV2::new(
                ExecutionRoleBindingV1::new(
                    dclutch_registry::release_set::ProgramIdentityV1::new([0xd2; 32])
                        .expect("different Registry program"),
                    successor,
                ),
                predecessor.rent(),
                predecessor.registry().artifact_release(),
                predecessor.rent().artifact_release(),
            )
            .expect("wrong Registry program V2"),
            ProtocolInfrastructureProfileV2::new(
                ExecutionRoleBindingV1::new(predecessor.registry().program(), successor),
                predecessor.rent(),
                ArtifactReleaseIdV1::new([0xd3; 32]).expect("wrong predecessor Registry"),
                predecessor.rent().artifact_release(),
            )
            .expect("wrong predecessor V2"),
        ] {
            let hostile_profile = RpcAccount {
                data: profile.to_bytes().to_vec(),
                ..v2_profile.clone()
            };
            assert!(
                authenticated_successor_registry_pin_v1(&plan, &programdata, &hostile_profile)
                    .is_err()
            );
        }

        let mut hostile_plan = plan.clone();
        hostile_plan
            .infrastructure_succession
            .as_mut()
            .expect("succession pin")
            .registry_candidate_elf_sha256 = hex(&[0xd4; 32]);
        assert!(
            authenticated_successor_registry_pin_v1(&hostile_plan, &programdata, &v2_profile,)
                .is_err()
        );
    }

    #[test]
    fn planned_local_succession_must_be_complete_before_direct_observation() {
        assert!(
            require_complete_local_succession_v1(crate::campaign::StageStateV1::Complete).is_ok()
        );
        for state in [
            crate::campaign::StageStateV1::Absent,
            crate::campaign::StageStateV1::Partial("successor record missing".into()),
            crate::campaign::StageStateV1::Conflict("substituted V2".into()),
        ] {
            assert!(require_complete_local_succession_v1(state).is_err());
        }
    }

    #[test]
    fn production_devnet_planner_closes_plan_rpc_and_market_authority() {
        let fixture = devnet_planner_fixture_v1();
        let recipient = Pubkey::new_from_array([0xb1; 32]);
        let probe = load_fixture_v1(
            &fixture,
            &fixture.plan,
            fixture.accounts.clone(),
            fixture.finalized_slot,
            Some(37),
            Some(recipient),
        );
        let loaded = probe
            .result
            .expect("authenticated production devnet planner");
        assert_eq!(probe.checked_calls.get(), 1);
        assert_eq!(probe.activation_calls.get(), 1);
        assert_eq!(probe.rpc_calls.get(), 1);
        assert_eq!(fixture.addresses.len(), 15);
        assert_eq!(
            fixture.retained_authority,
            Pubkey::new_from_array([0xa1; 32])
        );

        let compiler = loaded.compiler();
        assert_eq!(
            compiler.resolution_release,
            dclutch_source::resolution::RESOLUTION_CONTROLLER_RELEASE_ID_V7
        );
        assert_eq!(
            compiler.activation_deadline_slot,
            fixture.finalized_slot + DEVNET_DIRECT_ACTIVATION_WINDOW_SLOTS_V1
        );
        assert_eq!(
            compiler.root_rent_minimum_lamports,
            Rent::default().minimum_balance(DIRECT_CAPABILITY_ROOT_BYTES_V1)
        );
        let input = crate::market::demo_market_input(fixture.registry, compiler)
            .expect("market from authenticated Direct planner");
        assert_eq!(input.collateral_display_decimals, 6);
        let payload = input.direct_capability.as_ref().expect("Direct payload");
        assert_eq!(
            payload.activation_deadline_slot,
            fixture.finalized_slot + DEVNET_DIRECT_ACTIVATION_WINDOW_SLOTS_V1
        );
        assert_eq!(
            payload.root_rent_minimum_lamports,
            Rent::default().minimum_balance(DIRECT_CAPABILITY_ROOT_BYTES_V1)
        );
        let config_bytes = decode_hex(&payload.execution_config_hex).expect("execution config");
        let config_id: [u8; 32] = Sha256::digest(&config_bytes).into();
        let config = DirectExecutionConfigV1::decode_selected(config_id, config_id, &config_bytes)
            .expect("canonical execution config");
        assert_eq!(config.price_scale(), 1_000_000);
        assert_eq!(config.fee_basis_points(), 37);
        assert_eq!(config.fee_recipient(), recipient.to_bytes());
        let manifest_bytes =
            decode_hex(&input.capability_manifest_hex).expect("capability manifest");
        let manifest =
            CapabilityManifestV1::decode(&manifest_bytes).expect("canonical capability manifest");
        assert_eq!(
            manifest
                .entry(payload.selected_manifest_entry_index)
                .expect("Direct manifest entry")
                .funding_quote()
                .amounts()
                .rent()
                .amount(),
            payload.root_rent_minimum_lamports
        );
    }

    #[test]
    fn production_devnet_entry_refuses_loopback_before_file_or_rpc_access() {
        let missing = Path::new("/this/direct-plan-must-not-be-read.json");
        assert!(
            DirectMarketCompilerOwnedV1::load_devnet(
                missing,
                "http://127.0.0.1:8899",
                None,
                Pubkey::new_from_array([0xb2; 32]),
                Some(0),
                Some(Pubkey::new_from_array([0xb3; 32])),
            )
            .is_err()
        );
    }

    #[test]
    fn production_devnet_planner_refuses_missing_fee_before_rpc_connect() {
        let fixture = devnet_planner_fixture_v1();
        for (basis_points, recipient) in [
            (None, Some(Pubkey::new_from_array([0xb4; 32]))),
            (Some(0), None),
            (Some(10_001), Some(Pubkey::new_from_array([0xb4; 32]))),
            (Some(1), Some(Pubkey::default())),
        ] {
            let probe = load_fixture_v1(
                &fixture,
                &fixture.plan,
                fixture.accounts.clone(),
                fixture.finalized_slot,
                basis_points,
                recipient,
            );
            assert!(probe.result.is_err());
            assert_eq!(probe.checked_calls.get(), 1);
            assert_eq!(probe.activation_calls.get(), 1);
            assert_eq!(probe.rpc_calls.get(), 0);
        }
    }

    #[test]
    fn production_devnet_planner_refuses_every_snapshot_coordinate_substitution() {
        let fixture = devnet_planner_fixture_v1();
        for index in 0..fixture.accounts.len() {
            let mut accounts = fixture.accounts.clone();
            let account = accounts[index].as_mut().expect("fixture account");
            if index == 0 {
                account.data.push(0);
            } else if index % 2 == 1 {
                account.data[4] ^= 1;
            } else {
                *account.data.last_mut().expect("ProgramData ELF") ^= 1;
            }
            let probe = load_fixture_v1(
                &fixture,
                &fixture.plan,
                accounts,
                fixture.finalized_slot,
                Some(0),
                Some(Pubkey::new_from_array([0xb5; 32])),
            );
            assert!(
                probe.result.is_err(),
                "snapshot coordinate {index} was admitted"
            );
            assert_eq!(probe.checked_calls.get(), 1);
            assert_eq!(probe.activation_calls.get(), 1);
            assert_eq!(probe.rpc_calls.get(), 1);
        }
    }

    #[test]
    fn every_loader_role_refuses_each_independent_linkage_substitution() {
        let fixture = devnet_planner_fixture_v1();
        for (index, (role, original_pin, _)) in
            checked_plan_roles_v1(&fixture.plan).into_iter().enumerate()
        {
            let original_program = fixture.accounts[1 + index * 2]
                .as_ref()
                .expect("fixture Program")
                .clone();
            let original_programdata = fixture.accounts[2 + index * 2]
                .as_ref()
                .expect("fixture ProgramData")
                .clone();
            authenticate_live_role_v1(role, original_pin, &original_program, &original_programdata)
                .expect("exact Loader role");

            let mut program = original_program.clone();
            program.owner = Pubkey::new_from_array([0xc1; 32]);
            assert!(
                authenticate_live_role_v1(role, original_pin, &program, &original_programdata)
                    .is_err()
            );
            let mut program = original_program.clone();
            program.executable = false;
            assert!(
                authenticate_live_role_v1(role, original_pin, &program, &original_programdata)
                    .is_err()
            );
            let mut program = original_program.clone();
            program.data[4] ^= 1;
            assert!(
                authenticate_live_role_v1(role, original_pin, &program, &original_programdata)
                    .is_err()
            );

            let mut programdata = original_programdata.clone();
            programdata.owner = Pubkey::new_from_array([0xc2; 32]);
            assert!(
                authenticate_live_role_v1(role, original_pin, &original_program, &programdata)
                    .is_err()
            );
            let mut programdata = original_programdata.clone();
            programdata.executable = true;
            assert!(
                authenticate_live_role_v1(role, original_pin, &original_program, &programdata)
                    .is_err()
            );

            let mut pin = original_pin.clone();
            pin.programdata_sha256 = hex(&[0xc3; 32]);
            assert!(
                authenticate_live_role_v1(role, &pin, &original_program, &original_programdata)
                    .is_err()
            );
            let mut pin = original_pin.clone();
            pin.deployment_slot = pin.deployment_slot.saturating_add(1);
            assert!(
                authenticate_live_role_v1(role, &pin, &original_program, &original_programdata)
                    .is_err()
            );
            let mut pin = original_pin.clone();
            pin.upgrade_authority = Some(Pubkey::new_from_array([0xc4; 32]).to_string());
            assert!(
                authenticate_live_role_v1(role, &pin, &original_program, &original_programdata)
                    .is_err()
            );
            let mut pin = original_pin.clone();
            pin.live_elf_sha256 = hex(&[0xc5; 32]);
            assert!(
                authenticate_live_role_v1(role, &pin, &original_program, &original_programdata)
                    .is_err()
            );
            let mut pin = original_pin.clone();
            pin.checked_candidate_elf_sha256 = hex(&[0xc6; 32]);
            assert!(
                authenticate_live_role_v1(role, &pin, &original_program, &original_programdata)
                    .is_err()
            );
            let hostile_candidate = fixture.root.join(format!("{role}-hostile-candidate.so"));
            let hostile_candidate_bytes = [0x7f, b'E', b'L', b'F', 0xfe];
            fs::write(&hostile_candidate, hostile_candidate_bytes)
                .expect("write hostile candidate");
            let mut pin = original_pin.clone();
            pin.checked_candidate_elf_path = hostile_candidate.display().to_string();
            pin.checked_candidate_elf_sha256 = hex(&Sha256::digest(hostile_candidate_bytes));
            assert!(
                authenticate_live_role_v1(role, &pin, &original_program, &original_programdata)
                    .is_err()
            );
            let mut pin = original_pin.clone();
            pin.live_elf_padding_bytes = pin.live_elf_padding_bytes.saturating_add(1);
            assert!(
                authenticate_live_role_v1(role, &pin, &original_program, &original_programdata)
                    .is_err()
            );
        }
    }

    #[test]
    fn production_devnet_planner_refuses_snapshot_width_and_context_drift() {
        let fixture = devnet_planner_fixture_v1();
        let mut short = fixture.accounts.clone();
        short.pop();
        let short_probe = load_fixture_v1(
            &fixture,
            &fixture.plan,
            short,
            fixture.finalized_slot,
            Some(0),
            Some(Pubkey::new_from_array([0xb6; 32])),
        );
        assert!(short_probe.result.is_err());
        assert_eq!(short_probe.rpc_calls.get(), 1);

        let stale_probe = load_fixture_v1(
            &fixture,
            &fixture.plan,
            fixture.accounts.clone(),
            fixture.floor - 1,
            Some(0),
            Some(Pubkey::new_from_array([0xb7; 32])),
        );
        assert!(stale_probe.result.is_err());
        assert_eq!(stale_probe.rpc_calls.get(), 1);

        for mutate in [
            |account: &mut RpcAccount| account.owner = Pubkey::new_from_array([0xbc; 32]),
            |account: &mut RpcAccount| account.executable = true,
        ] {
            let mut accounts = fixture.accounts.clone();
            mutate(accounts[0].as_mut().expect("Rent account"));
            let rent_probe = load_fixture_v1(
                &fixture,
                &fixture.plan,
                accounts,
                fixture.finalized_slot,
                Some(0),
                Some(Pubkey::new_from_array([0xbd; 32])),
            );
            assert!(rent_probe.result.is_err());
            assert_eq!(rent_probe.rpc_calls.get(), 1);
        }
    }

    #[test]
    fn production_devnet_planner_refuses_mixed_set_activation_and_alias_substitutions() {
        let fixture = devnet_planner_fixture_v1();

        let mut mixed = fixture.plan.clone();
        mixed
            .checked_upgrade_set
            .as_mut()
            .expect("checked set")
            .roles[2]
            .disposition = CheckedDeploymentDispositionV1::CarryForward;
        let mixed_probe = load_fixture_v1(
            &fixture,
            &mixed,
            fixture.accounts.clone(),
            fixture.finalized_slot,
            Some(0),
            Some(Pubkey::new_from_array([0xb8; 32])),
        );
        assert!(mixed_probe.result.is_err());
        assert_eq!(mixed_probe.checked_calls.get(), 1);
        assert_eq!(mixed_probe.activation_calls.get(), 0);
        assert_eq!(mixed_probe.rpc_calls.get(), 0);

        let mut alias = fixture.plan.clone();
        alias.trading.elf_path = alias.claims.elf_path.clone();
        let alias_probe = load_fixture_v1(
            &fixture,
            &alias,
            fixture.accounts.clone(),
            fixture.finalized_slot,
            Some(0),
            Some(Pubkey::new_from_array([0xb9; 32])),
        );
        assert!(alias_probe.result.is_err());
        assert_eq!(alias_probe.checked_calls.get(), 1);
        assert_eq!(alias_probe.activation_calls.get(), 0);
        assert_eq!(alias_probe.rpc_calls.get(), 0);

        let mut activation = fixture.plan.clone();
        activation.activation = Pubkey::new_from_array([0xba; 32]).to_string();
        let activation_probe = load_fixture_v1(
            &fixture,
            &activation,
            fixture.accounts.clone(),
            fixture.finalized_slot,
            Some(0),
            Some(Pubkey::new_from_array([0xbb; 32])),
        );
        assert!(activation_probe.result.is_err());
        assert_eq!(activation_probe.checked_calls.get(), 1);
        assert_eq!(activation_probe.activation_calls.get(), 1);
        assert_eq!(activation_probe.rpc_calls.get(), 0);

        let mut activation_body = fixture.plan.clone();
        let release_set = activation_body
            .records
            .get_mut("execution_release_set")
            .expect("execution release set");
        let mut body = decode_hex(&release_set.body_hex).expect("release-set body");
        body[0] ^= 1;
        release_set.body_hex = hex(&body);
        let activation_probe = load_fixture_v1(
            &fixture,
            &activation_body,
            fixture.accounts.clone(),
            fixture.finalized_slot,
            Some(0),
            Some(Pubkey::new_from_array([0xbe; 32])),
        );
        assert!(activation_probe.result.is_err());
        assert_eq!(activation_probe.checked_calls.get(), 1);
        assert_eq!(activation_probe.activation_calls.get(), 1);
        assert_eq!(activation_probe.rpc_calls.get(), 0);

        let mut mixed_origin = fixture.plan.clone();
        mixed_origin.checked_local_mutable_set = Some(crate::model::CheckedLocalMutableSetPinV1 {
            schema: crate::local_mutable::CHECKED_LOCAL_MUTABLE_SET_SCHEMA_V1.into(),
            checked_release_gate_path: "/private/tmp/never-read-mixed-gate.json".into(),
            checked_release_gate_sha256: "11".repeat(32),
            source_revision: "22".repeat(20),
            source_tree_sha256: "33".repeat(32),
            solana_cli_version: "solana-cli 4.0.2".into(),
            retained_upgrade_authority: fixture.retained_authority.to_string(),
            execution_release_set: crate::model::CheckedLocalExecutionReleaseSetPinV1 {
                schema: crate::local_mutable::CHECKED_LOCAL_EXECUTION_RELEASE_SET_SCHEMA_V1.into(),
                checked_execution_release_set_id: "55".repeat(32),
                execution_release_set_id: mixed_origin.release_set_id.clone(),
                checked_execution_release_set_base64: String::new(),
                roles: Vec::new(),
            },
            set_sha256: "44".repeat(32),
            roles: Vec::new(),
        });
        let mixed_origin_probe = load_fixture_v1(
            &fixture,
            &mixed_origin,
            fixture.accounts.clone(),
            fixture.finalized_slot,
            Some(0),
            Some(Pubkey::new_from_array([0xbf; 32])),
        );
        assert!(mixed_origin_probe.result.is_err());
        assert_eq!(mixed_origin_probe.checked_calls.get(), 0);
        assert_eq!(mixed_origin_probe.activation_calls.get(), 0);
        assert_eq!(mixed_origin_probe.rpc_calls.get(), 0);

        let mut coordinate_alias = fixture.plan.clone();
        coordinate_alias.claims.program_id = coordinate_alias.trading.program_id.clone();
        let claims_evidence = coordinate_alias
            .checked_upgrade_set
            .as_mut()
            .expect("checked set")
            .roles
            .iter_mut()
            .find(|role| role.role == "claims")
            .expect("Claims evidence");
        claims_evidence.program_id = coordinate_alias.claims.program_id.clone();
        assert!(
            authenticate_devnet_plan_v1(
                &coordinate_alias,
                fixture.registry,
                &PassThroughPlanEvidenceV1,
            )
            .is_err()
        );
    }

    #[test]
    fn devnet_policy_requires_both_explicit_fee_facts() {
        let recipient = Pubkey::new_from_array([0x51; 32]);
        assert!(DirectFeeSelectionV1::explicit(None, Some(recipient)).is_err());
        assert!(DirectFeeSelectionV1::explicit(Some(0), None).is_err());
        assert!(DirectFeeSelectionV1::explicit(Some(10_001), Some(recipient)).is_err());
        assert!(DirectFeeSelectionV1::explicit(Some(25), Some(Pubkey::default())).is_err());
        assert_eq!(
            DirectFeeSelectionV1::explicit(Some(0), Some(recipient))
                .expect("explicit zero-fee policy"),
            DirectFeeSelectionV1 {
                basis_points: 0,
                recipient,
            }
        );
    }

    #[test]
    fn exact_plan_json_refuses_nested_duplicates_trailing_bytes_and_unknown_fields() {
        assert!(
            decode_exact_json_v1::<ExactJsonFixtureV1>(
                br#"{"nested":{"value":1,"value":2}}"#,
                "fixture",
            )
            .is_err()
        );
        assert!(
            decode_exact_json_v1::<ExactJsonFixtureV1>(
                br#"{"nested":{"value":1}} {"second":true}"#,
                "fixture",
            )
            .is_err()
        );
        assert!(
            decode_exact_json_v1::<ExactJsonFixtureV1>(
                br#"{"nested":{"value":1,"extra":2}}"#,
                "fixture",
            )
            .is_err()
        );
        let parsed =
            decode_exact_json_v1::<ExactJsonFixtureV1>(br#"{"nested":{"value":1}}"#, "fixture")
                .expect("exact JSON");
        assert_eq!(parsed.nested.value, 1);
    }

    #[test]
    fn market_owns_price_scale_and_manifest_quotes_complete_root_rent() {
        let input = test_market();
        let payload = input.direct_capability.as_ref().expect("Direct payload");
        let config_bytes = decode_hex(&payload.execution_config_hex).expect("execution config");
        let config_id: [u8; 32] = Sha256::digest(&config_bytes).into();
        let config = DirectExecutionConfigV1::decode_selected(config_id, config_id, &config_bytes)
            .expect("canonical execution config");
        assert_eq!(config.price_scale(), 1_000_000);
        assert_eq!(config.fee_basis_points(), 0);
        assert_eq!(config.fee_recipient(), [0x41; 32]);

        let manifest_bytes = decode_hex(&input.capability_manifest_hex).expect("manifest bytes");
        let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest");
        let entry = manifest
            .entry(payload.selected_manifest_entry_index)
            .expect("selected Direct entry");
        assert_eq!(
            entry.funding_quote().amounts().rent().amount(),
            payload.root_rent_minimum_lamports
        );
        assert_eq!(DIRECT_CAPABILITY_ROOT_BYTES_V1, 256);
    }

    #[test]
    fn market_price_scale_and_activation_deadline_overflow_refuse() {
        let mut input = test_market();
        input.collateral_display_decimals = 20;
        let fee = DirectFeeSelectionV1::explicit(Some(0), Some(Pubkey::new_from_array([0x52; 32])))
            .expect("explicit fee policy");
        assert!(direct_execution_config_v1(&input, fee).is_err());
        assert_eq!(
            activation_deadline_v1(90).expect("bounded deadline"),
            90 + DEVNET_DIRECT_ACTIVATION_WINDOW_SLOTS_V1
        );
        assert!(activation_deadline_v1(u64::MAX).is_err());
    }

    #[test]
    fn direct_root_rent_requires_the_exact_sysvar_body() {
        let body = bincode::serialize(&Rent::default()).expect("canonical Rent sysvar");
        assert_eq!(
            direct_root_rent_minimum_v1(&body).expect("exact root Rent"),
            Rent::default().minimum_balance(DIRECT_CAPABILITY_ROOT_BYTES_V1)
        );
        let mut trailing = body;
        trailing.push(0);
        assert!(direct_root_rent_minimum_v1(&trailing).is_err());
    }

    #[test]
    fn ordinary_profile_lengths_bind_geometry_and_exact_deployments() {
        let widths =
            DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037).expect("deployment widths");
        let output = direct_logical_data_lengths_v1(
            widths,
            DirectOrdinaryGeometryV3::from_outcome_count(4).expect("geometry"),
        )
        .expect("profile lengths");
        assert_eq!(
            output.len(),
            usize::from(DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3)
        );
        assert_eq!(output[27], 1_141_117);
        assert_eq!(output[29], 971_053);
        assert_eq!(output[31], 934_037);
        assert_eq!(output[32], output[33]);
        assert_eq!(output[39], output[27]);
        assert_eq!(output[87], TOKEN_ACCOUNT_BYTES);
    }

    #[test]
    fn market_specific_capacity_profiles_coexist_under_one_trading_release() {
        let widths =
            DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037).expect("deployment widths");
        let logical_data_lengths = direct_logical_data_lengths_v1(
            widths,
            DirectOrdinaryGeometryV3::from_outcome_count(4).expect("geometry"),
        )
        .expect("profile lengths");
        let release = |capacity_profile| {
            let ordinary =
                build_direct_inline_ordinary_hot_bundle_v4(DirectInlineOrdinaryHotBundleInputV4 {
                    account_profile: DirectInlineOrdinaryAccountProfileInputV3 {
                        logical_data_lengths: &logical_data_lengths,
                    },
                    capacity_profile,
                })
                .expect("ordinary bundle");
            build_direct_inline_ordinary_lifecycle_program_set_v1(ordinary, capacity_profile)
                .expect("ordinary lifecycle ProgramSet")
        };
        let first = release([0x51; 32]);
        let second = release([0x52; 32]);
        assert_ne!(first.program_set_id, second.program_set_id);
        let set = CapabilityProgramSetV2::decode(&first.program_set).expect("ProgramSetV2");
        // Five since cohort-9: the maker-replay close is a lifecycle action of
        // the same standing as begin-retiring, and a market founded without its
        // entry is permanently unretirable once filled. This assertion said
        // four until the fifth entry landed and nothing re-ran it.
        assert_eq!(set.entry_count(), 5);
        assert_eq!(set.entry(0).expect("ordinary selector").selector(), 1);
        assert_eq!(
            set.entry(1).expect("begin-retiring selector").selector(),
            DIRECT_BEGIN_RETIRING_SELECTOR_V1
        );
        assert_eq!(
            set.entry(2).expect("native-close selector").selector(),
            DIRECT_NATIVE_CLOSE_SELECTOR_V1
        );
        assert_eq!(
            set.entry(3).expect("activation selector").selector(),
            DIRECT_ACTIVATION_SELECTOR_V1
        );
        // Entry four is the maker close, and its index is load-bearing: the
        // operator's close plan builder selects the descriptor by this index.
        assert_eq!(
            set.entry(4).expect("close-maker selector").selector(),
            DIRECT_CLOSE_MAKER_SELECTOR_V1
        );
    }

    #[test]
    fn typed_direct_closure_refuses_every_independent_identity_substitution() {
        let input = test_market();
        validate_direct_market_capability_v1(&input).expect("canonical Direct closure");
        let registry = Pubkey::new_from_array([0x41; 32]);
        let (product, domain, portfolio, basis) =
            crate::market::native_composition_bodies_for_test(registry, &input)
                .expect("native composition bodies");
        let publication = direct_publication_records_v1(
            &input,
            NativeCategoricalCompositionInputV1 {
                market: [0x61; 32],
                release_set: [0x62; 32],
                product_record_bytes: &product,
                result_domain_bytes: &domain,
                portfolio_bytes: &portfolio,
                product_basis_bytes: &basis,
            },
        )
        .expect("publication closure");
        assert_eq!(publication.len(), 25);
        let labels = publication
            .iter()
            .map(|record| record.label)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(labels.len(), publication.len());
        for label in [
            "direct_begin_retiring_account_profile_record",
            "direct_begin_retiring_effect_record",
            "direct_begin_retiring_descriptor_record",
            "terminal_composition_descriptor_record",
            "terminal_composition_graph_record",
            "terminal_composition_translation_record",
            "terminal_composition_exposure_record",
        ] {
            assert!(labels.contains(label), "missing publication {label}");
        }
        for record in &publication {
            assert_ne!(record.schema, [0; 32]);
            assert!(!record.body.is_empty());
        }

        let mut wrong_capacity = input.clone();
        let mut capacity = decode_hex(&wrong_capacity.source_capacity_profile_hex)
            .expect("capacity profile bytes");
        capacity[0] ^= 1;
        wrong_capacity.source_capacity_profile_hex = hex(&capacity);
        assert!(validate_direct_market_capability_v1(&wrong_capacity).is_err());

        let mut wrong_config = input.clone();
        let payload = wrong_config
            .direct_capability
            .as_mut()
            .expect("Direct payload");
        let mut config = decode_hex(&payload.execution_config_hex).expect("config bytes");
        config[0] ^= 1;
        payload.execution_config_hex = hex(&config);
        assert!(validate_direct_market_capability_v1(&wrong_config).is_err());

        let mut wrong_set = input.clone();
        let payload = wrong_set
            .direct_capability
            .as_mut()
            .expect("Direct payload");
        let mut program_set = decode_hex(&payload.program_set_hex).expect("ProgramSet bytes");
        let last = program_set.len() - 1;
        program_set[last] ^= 1;
        payload.program_set_hex = hex(&program_set);
        assert!(validate_direct_market_capability_v1(&wrong_set).is_err());

        let mut wrong_index = input;
        let payload = wrong_index
            .direct_capability
            .as_mut()
            .expect("Direct payload");
        payload.selected_manifest_entry_index = (payload.selected_manifest_entry_index + 1) % 4;
        assert!(validate_direct_market_capability_v1(&wrong_index).is_err());
    }
}
