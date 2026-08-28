//! Typed Direct capability closure for one exact market.
//!
//! The source capacity record is the sole capacity truth. This module only
//! turns its authenticated digest, the market geometry, and exact deployed
//! ProgramData widths into the finalized Direct records selected by the
//! manifest. It does not invent a family-level capacity label.

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    CompartmentFundingV1, ContentId as CapabilityContentId, FundingAmountsV1, FundingQuoteV1,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_capability_program_contract::{CapabilityProgramV1, v4::CapabilityProgramV4};
use dclutch_custody_contract::CustodyReplayLayoutV1;
use dclutch_direct_codec::{
    begin_retiring_bundle_v1::DirectBeginRetiringBundleV1,
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
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_registry_contract::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
use dclutch_registry_svm::{
    LOADER_V3_PROGRAM_BYTES, LOADER_V3_PROGRAMDATA_METADATA_BYTES, ProgramDataV3View, ProgramV3View,
};
use dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;
use dclutch_representation_composition_v3_operator::native_categorical_v1::{
    NativeCategoricalCompositionInputV1, compile_native_categorical_composition_v1,
};
use sha2::{Digest as _, Sha256};
use solana_sdk::{pubkey::Pubkey, rent::Rent};
use solana_sdk_ids::{bpf_loader_upgradeable, sysvar};
use std::{io::Read as _, path::Path};

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
    dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectDevnetPolicyObservationV1 {
    finalized_slot: u64,
    root_rent_minimum_lamports: u64,
    deployment: DirectDeploymentWidthsV1,
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

fn read_exact_json_v1<T>(path: &Path, label: &str) -> Result<T>
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

fn authenticate_devnet_plan_v1(plan: &SuccessorPlan, registry: Pubkey) -> Result<()> {
    if plan.schema != "dclutch-local-successor-infrastructure-plan-v2"
        || plan.record_publication != "transaction"
        || crate::plan::pubkey(&plan.registry.program_id)? != registry
    {
        return Err(Error::new(
            "Direct planner requires the exact transaction-published successor plan and Registry",
        ));
    }
    if decode_hex(&plan.trading.semantic_release_id)?
        != dclutch_direct_codec::COMPILED_DIRECT_RELEASE_ID_V1
    {
        return Err(Error::new(
            "Direct compiler requires the Trading COMPILED_DIRECT_RELEASE_ID_V1 semantic owner",
        ));
    }
    let checked = plan.checked_upgrade_set.as_ref().ok_or_else(|| {
        Error::new("Direct devnet planning requires a checked mixed deployment-set plan")
    })?;
    if checked.devnet_genesis_hash != DEVNET_GENESIS_HASH {
        return Err(Error::new(
            "checked deployment-set plan does not name devnet's genesis hash",
        ));
    }
    crate::upgrade::reauthenticate_checked_deployment_set_pin(checked)?;
    let retained = crate::plan::pubkey(&checked.retained_upgrade_authority)?;
    for (role, pin, disposition) in checked_plan_roles_v1(plan) {
        let evidence = checked_role_v1(checked, role)?;
        if evidence.disposition != disposition
            || evidence.program_id != pin.program_id
            || evidence.programdata_id != pin.programdata_id
            || evidence.checked_candidate_elf_path != pin.checked_candidate_elf_path
            || evidence.checked_candidate_elf_sha256 != pin.checked_candidate_elf_sha256
            || evidence.live_elf_sha256 != pin.live_elf_sha256
            || evidence.deployment_slot != pin.deployment_slot
            || evidence.programdata_account_sha256 != pin.programdata_sha256
            || evidence.semantic_release_id != pin.semantic_release_id
            || pin.upgrade_authority.as_deref() != Some(retained.to_string().as_str())
            || pin.deployment_source != "observed-programdata-account"
        {
            return Err(Error::new(format!(
                "checked deployment-set role {role} differs from the exact Direct plan pin"
            )));
        }
    }
    crate::runtime::authenticate_checked_activation_projection(plan)?;
    Ok(())
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

fn observe_devnet_policy_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
) -> Result<DirectDevnetPolicyObservationV1> {
    let checked = plan.checked_upgrade_set.as_ref().ok_or_else(|| {
        Error::new("Direct devnet observation omitted its checked deployment set")
    })?;
    let floor = checked
        .roles
        .iter()
        .map(|role| role.deployment_slot)
        .chain(std::iter::once(
            checked.infrastructure_carry_forward.context_slot,
        ))
        .max()
        .ok_or_else(|| Error::new("checked deployment set omitted every observation slot"))?;
    let roles = checked_plan_roles_v1(plan);
    let mut addresses = Vec::with_capacity(1 + roles.len() * 2);
    addresses.push(sysvar::rent::ID);
    for (_, pin, _) in roles {
        addresses.push(crate::plan::pubkey(&pin.program_id)?);
        addresses.push(crate::plan::pubkey(&pin.programdata_id)?);
    }
    let (finalized_slot, accounts) = rpc.finalized_accounts(&addresses, floor)?;
    let rent_account = accounts
        .first()
        .and_then(Option::as_ref)
        .ok_or_else(|| Error::new("finalized snapshot omitted the Rent sysvar"))?;
    if rent_account.owner != sysvar::ID || rent_account.executable {
        return Err(Error::new(
            "finalized Rent account did not have canonical sysvar ownership",
        ));
    }
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

fn direct_root_rent_minimum_v1(rent_sysvar: &[u8]) -> Result<u64> {
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
    let minimum = rent.minimum_balance(DIRECT_CAPABILITY_ROOT_BYTES_V1);
    if minimum == 0 {
        return Err(Error::new("finalized 256-byte Direct root Rent was zero"));
    }
    Ok(minimum)
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
    activation_deadline_slot: u64,
    root_rent_minimum_lamports: u64,
    lifetime: (),
}

impl DirectMarketCompilerOwnedV1 {
    /// Retired unsafe constructor retained only until the shared CLI dispatch
    /// window can delete its old call site.
    ///
    /// A file cannot own Direct price/fee coordinates, and caller scalars cannot
    /// own Rent or a cluster-slot deadline. This function therefore always
    /// refuses instead of preserving a second production authority path.
    pub(crate) fn load(
        _plan_path: &Path,
        _execution_config_path: &Path,
        _registry: Pubkey,
        _activation_deadline_slot: u64,
        _root_rent_minimum_lamports: u64,
    ) -> Result<Self> {
        Err(Error::new(
            "caller-authored --direct-execution-config, --direct-activation-deadline-slot, and \
             --direct-root-rent-minimum-lamports are retired; use the acknowledged devnet Direct \
             planner with explicit --direct-fee-basis-points and --direct-fee-recipient",
        ))
    }

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
        let origin = ClusterOriginV1::parse(rpc_url, devnet_acknowledgment)?;
        if !matches!(origin, ClusterOriginV1::AcknowledgedDevnet { .. }) {
            return Err(Error::new(
                "the production Direct planner is devnet-only and refuses loopback; use the lab fixture compiler for a local validator",
            ));
        }
        let plan = read_exact_json_v1::<SuccessorPlan>(plan_path, "successor plan")?;
        authenticate_devnet_plan_v1(&plan, registry)?;
        let fee = DirectFeeSelectionV1::explicit(fee_basis_points, fee_recipient)?;
        let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;
        let observation = observe_devnet_policy_v1(&mut rpc, &plan)?;
        Ok(Self {
            deployment: observation.deployment,
            fee,
            activation_deadline_slot: activation_deadline_v1(observation.finalized_slot)?,
            root_rent_minimum_lamports: observation.root_rent_minimum_lamports,
            lifetime: (),
        })
    }

    pub(crate) fn compiler(&self) -> DirectMarketCompilerInputV1<'_> {
        DirectMarketCompilerInputV1 {
            deployment: self.deployment,
            fee: self.fee,
            activation_deadline_slot: self.activation_deadline_slot,
            root_rent_minimum_lamports: self.root_rent_minimum_lamports,
            _lifetime: &self.lifetime,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(registry: Pubkey, deployment: DirectDeploymentWidthsV1) -> Self {
        Self {
            deployment,
            fee: DirectFeeSelectionV1::explicit(Some(0), Some(registry))
                .expect("test Direct fee policy"),
            activation_deadline_slot: u64::MAX,
            root_rent_minimum_lamports: Rent::default()
                .minimum_balance(DIRECT_CAPABILITY_ROOT_BYTES_V1),
            lifetime: (),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test_plan(registry: Pubkey, plan: &SuccessorPlan) -> Result<Self> {
        Ok(Self::for_test(
            registry,
            DirectDeploymentWidthsV1::from_plan(plan)?,
        ))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DirectMarketCompilerInputV1<'a> {
    pub(crate) deployment: DirectDeploymentWidthsV1,
    fee: DirectFeeSelectionV1,
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
        dclutch_source_contract::SourceSpecV1::decode(&decode_hex(&input.source_spec_hex)?)
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
    let descriptor = CapabilityProgramV4::decode(&release.ordinary.descriptor)
        .map_err(|error| Error::new(format!("Direct CapabilityProgramV4: {error:?}")))?;
    let direct_entry = direct_manifest_entry_v1(
        &release,
        descriptor,
        config_id,
        compiler.activation_deadline_slot,
        compiler.root_rent_minimum_lamports,
    )?;

    let base_bytes = decode_hex(&input.capability_manifest_hex)?;
    let base = CapabilityManifestV1::decode(&base_bytes)
        .map_err(|error| Error::new(format!("Resolution capability manifest: {error:?}")))?;
    if base.entry_count() != 3 || base.as_bytes() != base_bytes {
        return Err(Error::new(
            "Direct compilation requires the canonical three-entry Resolution base",
        ));
    }
    let first_release = base
        .entry(0)
        .map_err(|error| Error::new(format!("Resolution capability entry 0: {error:?}")))?
        .release_id();
    let mut entries = Vec::with_capacity(4);
    for index in 0..base.entry_count() {
        let entry = base.entry(index).map_err(|error| {
            Error::new(format!("Resolution capability entry {index}: {error:?}"))
        })?;
        if entry.kind_id().to_bytes() == DIRECT_SUCCESSOR_KIND_ID_V3
            || entry.release_id() != first_release
        {
            return Err(Error::new(
                "Resolution base must contain three same-release non-Direct companions",
            ));
        }
        entries.push(entry);
    }
    entries.push(direct_entry);
    entries.sort_by_key(|entry| entry.kind_id().to_bytes());
    let selected_manifest_entry_index = entries
        .iter()
        .position(|entry| entry.kind_id().to_bytes() == DIRECT_SUCCESSOR_KIND_ID_V3)
        .and_then(|index| u16::try_from(index).ok())
        .ok_or_else(|| Error::new("canonical manifest omitted its Direct entry"))?;
    let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut manifest)
        .map_err(|error| Error::new(format!("Direct-capable manifest: {error:?}")))?;

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
        dclutch_source_contract::SourceSpecV1::decode(&decode_hex(&input.source_spec_hex)?)
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
        descriptor,
        config_id,
        payload.activation_deadline_slot,
        payload.root_rent_minimum_lamports,
    )?;
    let manifest_bytes = decode_hex(&input.capability_manifest_hex)?;
    let manifest = CapabilityManifestV1::decode(&manifest_bytes)
        .map_err(|error| Error::new(format!("Direct-capable manifest: {error:?}")))?;
    if manifest.entry_count() != 4 || manifest.as_bytes() != manifest_bytes {
        return Err(Error::new(
            "Direct-capable manifest must be canonical and contain exactly four entries",
        ));
    }
    let selected = manifest
        .entry(payload.selected_manifest_entry_index)
        .map_err(|error| Error::new(format!("selected Direct manifest entry: {error:?}")))?;
    if selected != expected_entry {
        return Err(Error::new(
            "selected Direct manifest entry did not equal the typed Direct closure",
        ));
    }
    let mut direct_count = 0_u16;
    for index in 0..manifest.entry_count() {
        if manifest
            .entry(index)
            .map_err(|error| Error::new(format!("capability entry {index}: {error:?}")))?
            .kind_id()
            .to_bytes()
            == DIRECT_SUCCESSOR_KIND_ID_V3
        {
            direct_count = direct_count
                .checked_add(1)
                .ok_or_else(|| Error::new("Direct manifest count overflow"))?;
        }
    }
    if direct_count != 1 {
        return Err(Error::new(
            "Direct-capable manifest did not contain exactly one Direct kind",
        ));
    }
    Ok(())
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
            dclutch_capability_program_contract::v4::SCHEMA_RELEASE_ID,
            &release.ordinary.descriptor,
        ),
        record(
            "direct_begin_retiring_account_profile_record",
            dclutch_direct_codec::begin_retiring_bundle_v1::direct_begin_retiring_account_profile_schema_v1(),
            &release.begin_retiring.account_profile,
        ),
        record(
            "direct_begin_retiring_effect_record",
            dclutch_direct_codec::begin_retiring_bundle_v1::direct_begin_retiring_effect_schema_v1(),
            &release.begin_retiring.effect,
        ),
        record(
            "direct_begin_retiring_descriptor_record",
            dclutch_direct_codec::begin_retiring_bundle_v1::direct_begin_retiring_descriptor_schema_v1(),
            &release.begin_retiring.descriptor,
        ),
        record(
            "direct_native_close_account_profile_record",
            dclutch_direct_codec::native_close_bundle_v1::direct_native_close_account_profile_schema_v1(),
            &release.native_close.account_profile,
        ),
        record(
            "direct_native_close_effect_record",
            dclutch_direct_codec::native_close_bundle_v1::direct_native_close_effect_schema_v1(),
            &release.native_close.effect,
        ),
        record(
            "direct_native_close_descriptor_record",
            dclutch_direct_codec::native_close_bundle_v1::direct_native_close_descriptor_schema_v1(),
            &release.native_close.descriptor,
        ),
        record(
            "direct_program_set_record",
            dclutch_capability_program_contract::set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
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

fn direct_manifest_entry_v1(
    release: &DirectInlineOrdinaryLifecycleProgramSetV1,
    descriptor: CapabilityProgramV4,
    config_id: [u8; 32],
    activation_deadline_slot: u64,
    root_rent_minimum_lamports: u64,
) -> Result<CapabilityEntryV1> {
    if activation_deadline_slot == 0 {
        return Err(Error::new(
            "Direct activation deadline slot must be positive",
        ));
    }
    if root_rent_minimum_lamports == 0 {
        return Err(Error::new("Direct root rent minimum must be positive"));
    }
    let none = CompartmentFundingV1::not_applicable();
    let amounts = FundingAmountsV1::new(
        // The funding ledger owns the complete exact Rent quote. Any lamports
        // already sitting on the vacant PDA are classified at activation by the
        // dust-safe root-creation semantic owner as displaced prepayment or
        // unsolicited surplus; they never reduce this immutable quote.
        CompartmentFundingV1::native_lamports(root_rent_minimum_lamports)
            .map_err(|error| Error::new(format!("Direct root rent quote: {error:?}")))?,
        none,
        none,
        none,
        none,
        none,
        none,
    )
    .map_err(|error| Error::new(format!("Direct funding amounts: {error:?}")))?;
    CapabilityEntryV1::new(
        capability_content(DIRECT_SUCCESSOR_KIND_ID_V3)?,
        capability_content(release.program_set_id)?,
        capability_content(config_id)?,
        capability_content(descriptor.capacity_profile().to_bytes())?,
        capability_content(descriptor.root_schema().to_bytes())?,
        capability_content(descriptor.derivation_policy().to_bytes())?,
        ActivationPolicy::PrepaidLazy,
        activation_deadline_slot,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(amounts, None)
            .map_err(|error| Error::new(format!("Direct funding quote: {error:?}")))?,
    )
    .map_err(|error| Error::new(format!("Direct manifest entry: {error:?}")))
}

fn decode_direct_release_v1(
    payload: &DirectMarketCapabilityV1,
    capacity_profile: [u8; 32],
) -> Result<DirectInlineOrdinaryLifecycleProgramSetV1> {
    let ordinary = DirectInlineOrdinaryHotBundleV4 {
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
    };
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
    let program_set = decode_hex(&payload.program_set_hex)?;
    let release = DirectInlineOrdinaryLifecycleProgramSetV1 {
        ordinary,
        begin_retiring,
        native_close,
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

fn capability_content(value: [u8; 32]) -> Result<CapabilityContentId> {
    CapabilityContentId::new(value)
        .map_err(|error| Error::new(format!("capability content: {error:?}")))
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
        dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(dclutch_direct_codec::successor::DIRECT_ROOT_STATE_BYTES_V1)
            .ok_or_else(|| Error::new("Direct root width overflow"))?,
    )?;
    put_width(
        &mut output,
        1,
        dclutch_direct_codec::successor::DIRECT_EXECUTION_CONFIG_BYTES_V1,
    )?;
    put_width(&mut output, 2, PRODUCT_RECORD_BYTES_V2)?;
    put_geometry_width(&mut output, 3, geometry.portfolio_record_bytes())?;
    put_width(
        &mut output,
        4,
        dclutch_product_payoff_v2_codec::runtime_v3::BASIS_HEADER_BYTES_V3,
    )?;
    for coordinate in [5_usize, 8] {
        put_width(
            &mut output,
            coordinate,
            dclutch_direct_codec::successor::DIRECT_MAKER_REPLAY_BYTES_V1,
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
    put_width(&mut output, 23, dclutch_market_core_codec::STATE_BYTES)?;
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
    put_width(&mut output, 40, dclutch_realm_contract::REALM_BYTES)?;
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
    use dclutch_capability_program_contract::set_v2::CapabilityProgramSetV2;
    use dclutch_direct_codec::{
        native_close_bundle_v1::DIRECT_NATIVE_CLOSE_SELECTOR_V1,
        retirement_v1::DIRECT_BEGIN_RETIRING_SELECTOR_V1,
    };

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
    fn retired_detached_authority_refuses_before_reading_any_file() {
        let missing = Path::new("/this/path/must/not/be/read");
        let error = match DirectMarketCompilerOwnedV1::load(
            missing,
            missing,
            Pubkey::new_from_array([0x53; 32]),
            1,
            1,
        ) {
            Ok(_) => panic!("retired detached authority was admitted"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("retired"));
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
        assert_eq!(set.entry_count(), 3);
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
            dclutch_direct_codec::COMPILED_DIRECT_RELEASE_ID_V1,
            dclutch_direct_codec::COMPILED_DIRECT_RELEASE_ID_V1
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
        assert_eq!(publication.len(), 19);
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
