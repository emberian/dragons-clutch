//! Stranger-operable next-act planning for ordered Fractional retirement V3.
//!
//! A caller supplies one finalized account graph, a fee payer, and routing
//! liveness. The authenticated cursor state -- never a caller action or
//! coordinate -- selects exactly one of `Begin`, `RetireCoordinate`, or
//! `Finish`. The result is one unsigned, packet-safe v0 message and the exact
//! production instruction it contains.

use dclutch_claims::{
    liability_basis_state_v2::{
        LiabilityBasisMarketSeedsV2, LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
    },
    protocol_position_v2::{
        ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2,
        ProtocolPositionSeedsV2,
    },
};
use dclutch_claims::fractional::{
    FRACTIONAL_RETIREMENT_BEGIN_ACCOUNT_COUNT_V3,
    FRACTIONAL_RETIREMENT_COORDINATE_ACCOUNT_COUNT_V3, FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3,
    FRACTIONAL_RETIREMENT_CURSOR_PDA_SEED_V3, FRACTIONAL_RETIREMENT_FINISH_ACCOUNT_COUNT_V3,
    FractionalRetireCoordinateObservationV3, FractionalRetirementActionV3,
    FractionalRetirementCursorV3, FractionalRetirementRequestInputV3,
    FractionalRetirementRequestV3, NO_RETIREMENT_COORDINATE_V3,
    decode_fractional_capability_root_v4,
};
use dclutch_claims::fractional_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FRACTIONAL_SELECTION_CONFIG_BYTES_V1,
    FractionalExposureTermsAdmissionV2, FractionalExposureTermsV2,
    encode_fractional_selection_config_v1, fractional_selection_config_from_terms_v1,
};
use dclutch_market::{CoreState, MarketCoreStateSeedsV2, Phase};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1, DeploymentObservationV1,
};
use dclutch_registry::svm::{ProgramDataV3View, ProgramV3View};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_market::rent::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_custody::token_svm::{TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, Token2022BehaviorProfileV2};
use dclutch_versioned_message_operator::{
    Finality, Observation, ObservedAccount, VersionedMessagePlanV0,
    compile_v0_message_with_optional_tables,
};
use solana_hash::Hash;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};

use crate::{
    Error, FractionalTokenBehaviorRecordAdmissionV2, Result,
    authenticate_fractional_token_behavior_v2,
};

/// One exact executable plus the Loader-v3 ProgramData it currently names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalRetirementDeploymentV3 {
    /// Executable program account.
    pub program: ObservedAccount,
    /// Current Loader-v3 ProgramData account.
    pub programdata: ObservedAccount,
}

/// One finalized raw record and its canonical vacant staging cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalRetirementRecordV3 {
    /// Registry-owned finalized raw record.
    pub raw: ObservedAccount,
    /// Canonical vacant staging cursor.
    pub staging: ObservedAccount,
}

/// The O(1) state needed only when the cursor selects its next coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalRetirementCoordinateSnapshotV3 {
    /// Claims-native reserve Position owned by the Fractional root.
    pub position: ObservedAccount,
    /// Claims-owned immutable Position admission.
    pub admission: ObservedAccount,
    /// Terms-owned zero-supply Token-2022 Mint.
    pub shard_mint: ObservedAccount,
}

/// One exact finalized graph from which the next retirement act is selected.
///
/// Address-bearing fields are untrusted routing hints until this module
/// derives or rejoins them. `coordinate` must be absent for begin/finish and
/// present for a live cursor that still owes one coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalRetirementSnapshotV3 {
    /// Public transaction fee payer. No key material is accepted.
    pub payer: Pubkey,
    /// Core Market selected by the root.
    pub core_market: ObservedAccount,
    /// Claims aggregate derived from that Core Market.
    pub claims_market: ObservedAccount,
    /// Registry activation cache for the selected execution release set.
    pub activation_cache: ObservedAccount,
    /// Current executable Registry program.
    pub registry_program: ObservedAccount,
    /// Current Core deployment.
    pub core: FractionalRetirementDeploymentV3,
    /// Current Claims deployment.
    pub claims: FractionalRetirementDeploymentV3,
    /// Current Trading deployment.
    pub trading: FractionalRetirementDeploymentV3,
    /// Current Rent deployment selected by the Market's RentCredit.
    pub rent: FractionalRetirementDeploymentV3,
    /// Activated Fractional capability root.
    pub root: ObservedAccount,
    /// Root-bound lifecycle RentCredit.
    pub rent_credit: ObservedAccount,
    /// Canonical cursor address, either vacant or live Claims state.
    pub cursor: ObservedAccount,
    /// Finalized Fractional Exposure Terms V2 record.
    pub terms: FractionalRetirementRecordV3,
    /// Finalized TokenBehaviorSelectionV2 record.
    pub token_behavior: FractionalRetirementRecordV3,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Canonical executable System program.
    pub system_program: ObservedAccount,
    /// Terms-selected executable Token-2022 program.
    pub token_program: ObservedAccount,
    /// Exact current coordinate state, present only for a coordinate act.
    pub coordinate: Option<FractionalRetirementCoordinateSnapshotV3>,
}

/// State-selected addresses needed to complete a coordinate snapshot.
///
/// This is the first phase of a two-read operator flow. It authenticates the
/// same common graph and cursor as the full planner, but returns only addresses
/// derived from those facts. The caller must reacquire the entire graph plus
/// these accounts at one finalized observation before planning an instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalRetirementDiscoveryV3 {
    /// Exact finalized observation authenticated for the discovery read.
    pub observation: Observation,
    /// State-selected next action.
    pub action: FractionalRetirementActionV3,
    /// State-selected coordinate, absent for begin and finish.
    pub coordinate: Option<u32>,
    /// Canonical reserve Position needed for a coordinate act.
    pub position: Option<Pubkey>,
    /// Canonical immutable Position admission needed for a coordinate act.
    pub admission: Option<Pubkey>,
    /// Terms-selected shard Mint needed for a coordinate act.
    pub shard_mint: Option<Pubkey>,
    /// Root revision frozen before the ordered walk began.
    pub root_revision_anchor: u64,
    /// Cursor revision expected by the selected act.
    pub expected_revision: u64,
    /// Runtime representation width selected by finalized terms.
    pub representation_width: u32,
}

/// Complete one-act unsigned output chosen from authenticated current state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalRetirementInstructionPlanV3 {
    /// State-selected action.
    pub action: FractionalRetirementActionV3,
    /// State-selected coordinate, absent on begin and finish.
    pub coordinate: Option<u32>,
    /// Canonical fixed retirement request.
    pub request: FractionalRetirementRequestV3,
    /// SHA-256 of the exact request bytes.
    pub request_digest: [u8; 32],
    /// One exact outer production instruction.
    pub instruction: Instruction,
    /// Exact finalized observation shared by every semantic input.
    pub observation: Observation,
    /// Root revision frozen before the walk began.
    pub root_revision_anchor: u64,
    /// Cursor revision expected by this act.
    pub expected_revision: u64,
    /// Runtime representation width selected by the finalized terms.
    pub representation_width: u32,
    /// Full consequence stated without projecting execution success.
    pub consequence: &'static str,
    /// Operator remedy if this expiring unsigned act is not executed.
    pub remedy: &'static str,
}

/// Complete one-act unsigned wallet handoff chosen from authenticated state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalRetirementNextPlanV3 {
    /// Semantic instruction plan shared with physical real-ELF campaigns.
    pub instruction_plan: FractionalRetirementInstructionPlanV3,
    /// Packet-safe unsigned v0 message containing only the planned instruction.
    pub message: VersionedMessagePlanV0,
}

/// Authenticate a common graph and discover the exact accounts for its next act.
///
/// `snapshot.coordinate` must be absent. This function does not weaken the
/// full planner's single-observation rule: its result is routing information,
/// not authority. The caller must reacquire every account at one finalized
/// observation and pass the resulting complete snapshot to
/// [`plan_fractional_retirement_next_v3`].
pub fn discover_fractional_retirement_next_v3(
    snapshot: &FractionalRetirementSnapshotV3,
) -> Result<FractionalRetirementDiscoveryV3> {
    if snapshot.payer == Pubkey::default() || snapshot.coordinate.is_some() {
        return Err(Error::Message);
    }
    let observation = authenticate_observation(snapshot)?;
    let common = authenticate_common(snapshot)?;
    let selection = select_retirement_act_v3(
        authenticate_cursor(snapshot, common.root_key)?,
        common.root_revision,
    )?;
    let (position, admission, shard_mint) = if let Some(coordinate) = selection.coordinate {
        let position = Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(
                snapshot.claims_market.key.to_bytes(),
                common.root_key.to_bytes(),
            )
            .map_err(Error::ProtocolPosition)?
            .as_slices(),
            &snapshot.claims.program.key,
        )
        .0;
        let admission = Pubkey::find_program_address(
            &ProtocolPositionAdmissionSeedsV2::new(
                snapshot.claims_market.key.to_bytes(),
                common.root_key.to_bytes(),
            )
            .map_err(Error::ProtocolPosition)?
            .as_slices(),
            &snapshot.claims.program.key,
        )
        .0;
        let shard_mint = Pubkey::new_from_array(
            common
                .terms
                .shard_mint(coordinate)
                .map_err(Error::FractionalClaim)?,
        );
        (Some(position), Some(admission), Some(shard_mint))
    } else {
        (None, None, None)
    };
    Ok(FractionalRetirementDiscoveryV3 {
        observation,
        action: selection.action,
        coordinate: selection.coordinate,
        position,
        admission,
        shard_mint,
        root_revision_anchor: selection.root_revision_anchor,
        expected_revision: selection.expected_revision,
        representation_width: common.terms.representation_width(),
    })
}

/// Select and compile exactly the next ordered retirement act.
///
/// The caller cannot name an action or coordinate. A vacant canonical cursor
/// selects begin, a live cursor below width selects its exact next coordinate,
/// and a complete cursor selects finish.
pub fn plan_fractional_retirement_next_v3(
    snapshot: &FractionalRetirementSnapshotV3,
    recent_blockhash: Hash,
    lookup_tables: &[ObservedAccount],
) -> Result<FractionalRetirementNextPlanV3> {
    if snapshot.payer == Pubkey::default() || recent_blockhash == Hash::default() {
        return Err(Error::Message);
    }
    let instruction_plan = plan_fractional_retirement_instruction_v3(snapshot)?;
    let message = compile_v0_message_with_optional_tables(
        snapshot.payer,
        core::slice::from_ref(&instruction_plan.instruction),
        recent_blockhash,
        instruction_plan.observation,
        lookup_tables,
    )
    .map_err(Error::VersionedMessage)?;
    Ok(FractionalRetirementNextPlanV3 {
        instruction_plan,
        message,
    })
}

/// Select one exact production instruction without compiling a wallet message.
///
/// This is the semantic entrance used by real-ELF campaigns and other hosts
/// that own their own transaction transport. It performs the same complete
/// state authentication and action selection as
/// [`plan_fractional_retirement_next_v3`].
pub fn plan_fractional_retirement_instruction_v3(
    snapshot: &FractionalRetirementSnapshotV3,
) -> Result<FractionalRetirementInstructionPlanV3> {
    if snapshot.payer == Pubkey::default() {
        return Err(Error::Message);
    }
    let observation = authenticate_observation(snapshot)?;
    let common = authenticate_common(snapshot)?;
    let cursor_kind = authenticate_cursor(snapshot, common.root_key)?;
    let selection = select_retirement_act_v3(cursor_kind, common.root_revision)?;
    let action = selection.action;
    let coordinate = selection.coordinate;
    let expected_revision = selection.expected_revision;
    let root_revision_anchor = selection.root_revision_anchor;
    let request = FractionalRetirementRequestV3::new(
        action,
        FractionalRetirementRequestInputV3 {
            release_set: common.release_set,
            market: common.market,
            terms: common.terms_id,
            token_program: common.terms.token_program(),
            token_behavior: common.terms.token_behavior(),
            exposure: common.terms.exposure_id(),
            root: common.root_key.to_bytes(),
            rent_credit: snapshot.rent_credit.key.to_bytes(),
            expected_revision,
            representation_coordinate: coordinate.unwrap_or(NO_RETIREMENT_COORDINATE_V3),
        },
    )
    .and_then(|request| request.bind_terms(common.terms))
    .map_err(Error::FractionalRetirement)?;
    let instruction = match (action, cursor_kind) {
        (FractionalRetirementActionV3::Begin, CursorKindV3::Vacant) => {
            if snapshot.coordinate.is_some() {
                return Err(Error::AccountFrame);
            }
            build_begin(snapshot, request)?
        }
        (FractionalRetirementActionV3::RetireCoordinate, CursorKindV3::Live(cursor)) => {
            build_coordinate(snapshot, common, cursor, request)?
        }
        (FractionalRetirementActionV3::Finish, CursorKindV3::Live(cursor)) => {
            if snapshot.coordinate.is_some() || cursor.finish(common.terms, request).is_err() {
                return Err(Error::Rent);
            }
            build_finish(snapshot, request)?
        }
        _ => return Err(Error::Rent),
    };
    let request_bytes = request.to_bytes().map_err(Error::FractionalRetirement)?;
    if instruction.data.ends_with(&request_bytes) == false {
        return Err(Error::AccountFrame);
    }
    let (consequence, remedy) = consequence_v3(action);
    Ok(FractionalRetirementInstructionPlanV3 {
        action,
        coordinate,
        request,
        request_digest: hash(&request_bytes).to_bytes(),
        instruction,
        observation,
        root_revision_anchor,
        expected_revision,
        representation_width: common.terms.representation_width(),
        consequence,
        remedy,
    })
}

#[derive(Clone, Copy)]
struct CommonV3<'a> {
    root_key: Pubkey,
    root_revision: u64,
    release_set: [u8; 32],
    market: [u8; 32],
    terms_id: [u8; 32],
    terms: FractionalExposureTermsV2<'a>,
    claims_market: LiabilityBasisMarketViewV2,
}

#[derive(Clone, Copy)]
enum CursorKindV3 {
    Vacant,
    Live(FractionalRetirementCursorV3),
}

#[derive(Clone, Copy)]
struct RetirementSelectionV3 {
    action: FractionalRetirementActionV3,
    coordinate: Option<u32>,
    expected_revision: u64,
    root_revision_anchor: u64,
}

fn select_retirement_act_v3(
    cursor_kind: CursorKindV3,
    current_root_revision: u64,
) -> Result<RetirementSelectionV3> {
    let selection = match cursor_kind {
        CursorKindV3::Vacant => RetirementSelectionV3 {
            action: FractionalRetirementActionV3::Begin,
            coordinate: None,
            expected_revision: current_root_revision,
            root_revision_anchor: current_root_revision,
        },
        CursorKindV3::Live(cursor) if cursor.next_coordinate() < cursor.representation_width() => {
            RetirementSelectionV3 {
                action: FractionalRetirementActionV3::RetireCoordinate,
                coordinate: Some(cursor.next_coordinate()),
                expected_revision: cursor.revision(),
                root_revision_anchor: cursor
                    .root_revision_anchor()
                    .map_err(Error::FractionalRetirement)?,
            }
        }
        CursorKindV3::Live(cursor) if cursor.next_coordinate() == cursor.representation_width() => {
            RetirementSelectionV3 {
                action: FractionalRetirementActionV3::Finish,
                coordinate: None,
                expected_revision: cursor.revision(),
                root_revision_anchor: cursor
                    .root_revision_anchor()
                    .map_err(Error::FractionalRetirement)?,
            }
        }
        CursorKindV3::Live(_) => return Err(Error::Rent),
    };
    if selection.root_revision_anchor != current_root_revision {
        return Err(Error::Rent);
    }
    Ok(selection)
}

fn authenticate_observation(snapshot: &FractionalRetirementSnapshotV3) -> Result<Observation> {
    let mut accounts = vec![
        &snapshot.core_market,
        &snapshot.claims_market,
        &snapshot.activation_cache,
        &snapshot.registry_program,
        &snapshot.core.program,
        &snapshot.core.programdata,
        &snapshot.claims.program,
        &snapshot.claims.programdata,
        &snapshot.trading.program,
        &snapshot.trading.programdata,
        &snapshot.rent.program,
        &snapshot.rent.programdata,
        &snapshot.root,
        &snapshot.rent_credit,
        &snapshot.cursor,
        &snapshot.terms.raw,
        &snapshot.terms.staging,
        &snapshot.token_behavior.raw,
        &snapshot.token_behavior.staging,
        &snapshot.rent_sysvar,
        &snapshot.system_program,
        &snapshot.token_program,
    ];
    if let Some(coordinate) = &snapshot.coordinate {
        accounts.extend([
            &coordinate.position,
            &coordinate.admission,
            &coordinate.shard_mint,
        ]);
    }
    let observation = accounts
        .first()
        .map(|account| account.observation)
        .ok_or(Error::ChainArtifacts)?;
    if observation.finality != Finality::Finalized
        || accounts
            .iter()
            .any(|account| account.observation != observation)
    {
        return Err(Error::ChainArtifacts);
    }
    let mut keys = Vec::with_capacity(accounts.len());
    for account in accounts {
        if keys.contains(&account.key) {
            return Err(Error::AccountFrame);
        }
        keys.push(account.key);
    }
    Ok(observation)
}

fn authenticate_common(snapshot: &FractionalRetirementSnapshotV3) -> Result<CommonV3<'_>> {
    authenticate_native_accounts(snapshot)?;
    let root = decode_fractional_capability_root_v4(&snapshot.root.data).ok_or(Error::Rent)?;
    let header = root.header();
    let root_input = match root.state() {
        // Retirement V3 is the preserved historical terms-root route. A V2
        // root must use its current selection-config-aware planner rather than
        // silently interpreting byte 16 as a terms identity.
        dclutch_claims::fractional::FractionalRootStateV2::V1(root) => root.input(),
        dclutch_claims::fractional::FractionalRootStateV2::V2(_) => return Err(Error::Rent),
    };
    let (root_key, root_bump) =
        Pubkey::find_program_address(&header.seeds().as_slices(), &snapshot.trading.program.key);
    if snapshot.root.key != root_key
        || snapshot.root.owner != snapshot.trading.program.key
        || snapshot.root.executable
        || root_input.bump != root_bump
        || root_input.market != header.market()
        || root_input.terms == [0; 32]
    {
        return Err(Error::Rent);
    }

    let release_set = header.release_set().to_bytes();
    authenticate_release(snapshot, release_set)?;
    let claims_market = LiabilityBasisMarketViewV2::decode(&snapshot.claims_market.data)
        .map_err(Error::LiabilityBasisState)?;
    let expected_claims_market = Pubkey::find_program_address(
        &LiabilityBasisMarketSeedsV2::new(header.market())
            .map_err(Error::LiabilityBasisState)?
            .as_slices(),
        &snapshot.claims.program.key,
    )
    .0;
    if snapshot.claims_market.key != expected_claims_market
        || snapshot.claims_market.owner != snapshot.claims.program.key
        || snapshot.claims_market.executable
        || claims_market.logical_market != header.market()
        || claims_market.release_set != release_set
        || claims_market.registry_program != snapshot.registry_program.key.to_bytes()
    {
        return Err(Error::Claims);
    }
    let core = CoreState::decode(&snapshot.core_market.data).map_err(Error::MarketCore)?;
    let expected_core = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        &snapshot.core.program.key,
    )
    .0;
    if snapshot.core_market.key != expected_core
        || snapshot.core_market.key.to_bytes() != header.market()
        || snapshot.core_market.owner != snapshot.core.program.key
        || snapshot.core_market.executable
        || core.identity.market_id.to_bytes() != header.market()
        || core.identity.selected_release_set.to_bytes() != release_set
        || core.identity.registry_program.to_bytes() != snapshot.registry_program.key.to_bytes()
        || core.identity.generation != claims_market.generation
        || !matches!(core.phase, Phase::Terminal | Phase::Retiring)
        || core.terminal_receipt.is_none()
    {
        return Err(Error::Claims);
    }

    authenticate_record(
        &snapshot.terms,
        snapshot.registry_program.key,
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        root_input.terms,
    )?;
    let terms_id = hash(&snapshot.terms.raw.data).to_bytes();
    let terms = FractionalExposureTermsV2::decode(
        &snapshot.terms.raw.data,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: root_input.terms,
            finalized_terms_id: terms_id,
            recomputed_terms_digest: terms_id,
            finalized_terms_digest: terms_id,
            record_authenticated: true,
        },
    )
    .map_err(Error::FractionalClaim)?;
    if terms.market() != header.market()
        || terms.release_set() != release_set
        || terms.terms_id() != root_input.terms
    {
        return Err(Error::Projection);
    }
    let mut selection = [0_u8; FRACTIONAL_SELECTION_CONFIG_BYTES_V1];
    encode_fractional_selection_config_v1(
        fractional_selection_config_from_terms_v1(terms),
        &mut selection,
    )
    .map_err(Error::FractionalClaim)?;
    if hash(&selection).to_bytes() != header.selection().config().to_bytes() {
        return Err(Error::Projection);
    }
    authenticate_record(
        &snapshot.token_behavior,
        snapshot.registry_program.key,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        terms.token_behavior(),
    )?;
    authenticate_fractional_token_behavior_v2(
        terms,
        claims_market.realm_id,
        &snapshot.token_behavior.raw.data,
        FractionalTokenBehaviorRecordAdmissionV2 {
            selected_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            finalized_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            selected_content_digest: terms.token_behavior(),
            finalized_content_digest: terms.token_behavior(),
            recomputed_content_digest: hash(&snapshot.token_behavior.raw.data).to_bytes(),
            record_authenticated: true,
            market_realm_authenticated: true,
        },
    )?;
    let credit =
        LifecycleRentCreditV2::decode(&snapshot.rent_credit.data).map_err(Error::LifecycleRent)?;
    if snapshot.rent_credit.owner != snapshot.rent.program.key
        || snapshot.rent_credit.executable
        || credit.market().to_bytes() != header.market()
        || credit.release_set().to_bytes() != release_set
        || credit.generation() != claims_market.generation
        || root_input.rent_beneficiary != snapshot.rent_credit.key.to_bytes()
    {
        return Err(Error::Rent);
    }
    Ok(CommonV3 {
        root_key,
        root_revision: root_input.revision,
        release_set,
        market: header.market(),
        terms_id,
        terms,
        claims_market,
    })
}

fn authenticate_native_accounts(snapshot: &FractionalRetirementSnapshotV3) -> Result<()> {
    if snapshot.registry_program.owner != bpf_loader_upgradeable::ID
        || !snapshot.registry_program.executable
        || ProgramV3View::parse(&snapshot.registry_program.data).is_err()
        || snapshot.rent_sysvar.key != sysvar::rent::ID
        || snapshot.rent_sysvar.executable
        || snapshot.system_program.key != system_program::ID
        || snapshot.system_program.owner != native_loader::ID
        || !snapshot.system_program.executable
        || snapshot.token_program.key.to_bytes() != dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID
        || !snapshot.token_program.executable
    {
        return Err(Error::ChainArtifacts);
    }
    Ok(())
}

fn authenticate_release(
    snapshot: &FractionalRetirementSnapshotV3,
    release_set: [u8; 32],
) -> Result<()> {
    if snapshot.activation_cache.owner != snapshot.registry_program.key
        || snapshot.activation_cache.executable
        || snapshot.activation_cache.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
    {
        return Err(Error::ChainArtifacts);
    }
    let view = ActivatedExecutionReleaseSetViewV1::decode(&snapshot.activation_cache.data)
        .map_err(Error::Registry)?;
    let selected = view.execution_release_set_id().map_err(Error::Registry)?;
    let expected = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, selected.as_bytes()],
        &snapshot.registry_program.key,
    )
    .0;
    if selected.to_bytes() != release_set || snapshot.activation_cache.key != expected {
        return Err(Error::ChainArtifacts);
    }
    for (role, deployment) in [
        (ExecutionRoleV1::Core, &snapshot.core),
        (ExecutionRoleV1::Claims, &snapshot.claims),
        (ExecutionRoleV1::Trading, &snapshot.trading),
        (ExecutionRoleV1::Custody, &snapshot.rent),
    ] {
        authenticate_deployment(view, role, deployment)?;
    }
    Ok(())
}

fn authenticate_deployment(
    view: ActivatedExecutionReleaseSetViewV1<'_>,
    role: ExecutionRoleV1,
    deployment: &FractionalRetirementDeploymentV3,
) -> Result<()> {
    let activated = view.role(role).map_err(Error::Registry)?;
    let release = activated.release();
    let observation = deployment_observation(deployment, release)?;
    activated
        .authenticate_current_deployment(observation)
        .map_err(Error::Registry)
}

fn deployment_observation(
    deployment: &FractionalRetirementDeploymentV3,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1> {
    let program = &deployment.program;
    let programdata = &deployment.programdata;
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.program().to_bytes() != program.key.to_bytes()
        || release.programdata() != programdata.key.to_bytes()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(Error::ChainArtifacts);
    }
    let program_view = ProgramV3View::parse(&program.data).map_err(Error::RegistrySvm)?;
    let data = ProgramDataV3View::parse(&programdata.data).map_err(Error::RegistrySvm)?;
    let expected =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != programdata.key.to_bytes() || programdata.key != expected {
        return Err(Error::ChainArtifacts);
    }
    DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        data.deployment_slot(),
        hash(data.elf()).to_bytes(),
        data.upgrade_authority(),
    )
    .map_err(Error::Registry)
}

fn authenticate_record(
    record: &FractionalRetirementRecordV3,
    registry: Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<()> {
    let raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0;
    if record.raw.key != raw
        || record.raw.owner != registry
        || record.raw.executable
        || hash(&record.raw.data).to_bytes() != digest
        || record.staging.key != staging
        || record.staging.owner != system_program::ID
        || record.staging.executable
        || !record.staging.data.is_empty()
    {
        return Err(Error::ChainArtifacts);
    }
    Ok(())
}

fn authenticate_cursor(
    snapshot: &FractionalRetirementSnapshotV3,
    root: Pubkey,
) -> Result<CursorKindV3> {
    let (expected, _) = Pubkey::find_program_address(
        &[FRACTIONAL_RETIREMENT_CURSOR_PDA_SEED_V3, root.as_ref()],
        &snapshot.claims.program.key,
    );
    if snapshot.cursor.key != expected || snapshot.cursor.executable {
        return Err(Error::Rent);
    }
    if snapshot.cursor.owner == system_program::ID && snapshot.cursor.data.is_empty() {
        return Ok(CursorKindV3::Vacant);
    }
    if snapshot.cursor.owner != snapshot.claims.program.key
        || snapshot.cursor.data.len() != FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3
    {
        return Err(Error::Rent);
    }
    let cursor = FractionalRetirementCursorV3::decode(&snapshot.cursor.data)
        .map_err(Error::FractionalRetirement)?;
    let bump = [cursor.bump()];
    let recreated = Pubkey::create_program_address(
        &[
            FRACTIONAL_RETIREMENT_CURSOR_PDA_SEED_V3,
            root.as_ref(),
            &bump,
        ],
        &snapshot.claims.program.key,
    )
    .map_err(|_| Error::Rent)?;
    if recreated != expected || snapshot.cursor.lamports < cursor.historical_rent_principal() {
        return Err(Error::Rent);
    }
    Ok(CursorKindV3::Live(cursor))
}

fn build_begin(
    snapshot: &FractionalRetirementSnapshotV3,
    request: FractionalRetirementRequestV3,
) -> Result<Instruction> {
    let accounts = vec![
        AccountMeta::new(snapshot.payer, true),
        AccountMeta::new_readonly(snapshot.claims_market.key, false),
        AccountMeta::new_readonly(snapshot.core_market.key, false),
        AccountMeta::new_readonly(snapshot.core.program.key, false),
        AccountMeta::new_readonly(snapshot.registry_program.key, false),
        AccountMeta::new_readonly(snapshot.rent_sysvar.key, false),
        AccountMeta::new_readonly(snapshot.trading.program.key, false),
        AccountMeta::new_readonly(snapshot.claims.program.key, false),
        AccountMeta::new_readonly(snapshot.root.key, false),
        AccountMeta::new_readonly(snapshot.rent_credit.key, false),
        AccountMeta::new(snapshot.cursor.key, false),
        AccountMeta::new_readonly(snapshot.terms.raw.key, false),
        AccountMeta::new_readonly(snapshot.terms.staging.key, false),
        AccountMeta::new_readonly(snapshot.token_behavior.raw.key, false),
        AccountMeta::new_readonly(snapshot.token_behavior.staging.key, false),
        AccountMeta::new_readonly(snapshot.system_program.key, false),
    ];
    if accounts.len() != FRACTIONAL_RETIREMENT_BEGIN_ACCOUNT_COUNT_V3 {
        return Err(Error::AccountFrame);
    }
    Ok(Instruction {
        program_id: snapshot.claims.program.key,
        accounts,
        data: request
            .to_bytes()
            .map_err(Error::FractionalRetirement)?
            .to_vec(),
    })
}

fn build_coordinate(
    snapshot: &FractionalRetirementSnapshotV3,
    common: CommonV3<'_>,
    cursor: FractionalRetirementCursorV3,
    request: FractionalRetirementRequestV3,
) -> Result<Instruction> {
    let coordinate = snapshot.coordinate.as_ref().ok_or(Error::AccountFrame)?;
    let selected = request.input().representation_coordinate;
    let expected_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(
            snapshot.claims_market.key.to_bytes(),
            snapshot.root.key.to_bytes(),
        )
        .map_err(Error::ProtocolPosition)?
        .as_slices(),
        &snapshot.claims.program.key,
    )
    .0;
    let expected_admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(
            snapshot.claims_market.key.to_bytes(),
            snapshot.root.key.to_bytes(),
        )
        .map_err(Error::ProtocolPosition)?
        .as_slices(),
        &snapshot.claims.program.key,
    )
    .0;
    let position = LiabilityBasisPositionViewV2::decode(&coordinate.position.data)
        .map_err(Error::LiabilityBasisState)?;
    let reserve = position
        .balance(&coordinate.position.data, selected)
        .map_err(Error::LiabilityBasisState)?;
    let admission = ProtocolPositionAdmissionV2::decode(&coordinate.admission.data)
        .map_err(Error::ProtocolPosition)?;
    if coordinate.position.key != expected_position
        || coordinate.position.owner != snapshot.claims.program.key
        || coordinate.position.executable
        || position.market_account != snapshot.claims_market.key.to_bytes()
        || position.owner != snapshot.root.key.to_bytes()
        || position.claim_count != common.claims_market.claim_count
        || coordinate.admission.key != expected_admission
        || coordinate.admission.owner != snapshot.claims.program.key
        || coordinate.admission.executable
        || admission.owner_kind() != ProtocolPositionOwnerKindV2::TradingRecord
        || admission.position_owner() != snapshot.root.key.to_bytes()
        || admission.market() != common.market
        || admission.release_set() != common.release_set
        || admission.rent_credit() != snapshot.rent_credit.key.to_bytes()
        || admission.rent_program() != snapshot.rent.program.key.to_bytes()
        || admission.generation() != common.claims_market.generation
    {
        return Err(Error::Claims);
    }
    let mint = common
        .terms
        .shard_mint(selected)
        .map_err(Error::FractionalClaim)?;
    if coordinate.shard_mint.key.to_bytes() != mint
        || coordinate.shard_mint.owner != snapshot.token_program.key
        || coordinate.shard_mint.executable
    {
        return Err(Error::Token);
    }
    Token2022BehaviorProfileV2::check_mint(
        common.terms.token_program(),
        mint,
        &coordinate.shard_mint.data,
        snapshot.root.key.to_bytes(),
        0,
    )
    .map_err(Error::TokenSvm)?;
    cursor
        .advance(
            common.terms,
            request,
            FractionalRetireCoordinateObservationV3 {
                shard_mint: mint,
                shard_supply: 0,
                reserve_claims: reserve,
                mint_authenticated: true,
                reserve_authenticated: true,
            },
        )
        .map_err(Error::FractionalRetirement)?;
    let request_bytes = request.to_bytes().map_err(Error::FractionalRetirement)?;
    let caller = CallerAuthoritySeedsV1::from_bytes(
        common.release_set,
        common.market,
        ExecutionRoleV1::Trading,
        common.terms_id,
        hash(&request_bytes).to_bytes(),
    )
    .map_err(Error::ReleaseSet)?;
    let authority =
        Pubkey::find_program_address(&caller.as_slices(), &snapshot.trading.program.key).0;
    let forwarded = vec![
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(snapshot.claims_market.key, false),
        AccountMeta::new(coordinate.position.key, false),
        AccountMeta::new(coordinate.admission.key, false),
        AccountMeta::new_readonly(snapshot.rent_sysvar.key, false),
        AccountMeta::new_readonly(snapshot.system_program.key, false),
        AccountMeta::new_readonly(snapshot.activation_cache.key, false),
        AccountMeta::new_readonly(snapshot.registry_program.key, false),
        AccountMeta::new_readonly(snapshot.trading.program.key, false),
        AccountMeta::new_readonly(snapshot.trading.programdata.key, false),
        AccountMeta::new_readonly(snapshot.claims.program.key, false),
        AccountMeta::new_readonly(snapshot.claims.programdata.key, false),
        AccountMeta::new(snapshot.root.key, false),
        AccountMeta::new(snapshot.rent_credit.key, false),
        AccountMeta::new_readonly(snapshot.rent.program.key, false),
        AccountMeta::new(snapshot.cursor.key, false),
        AccountMeta::new_readonly(snapshot.terms.raw.key, false),
        AccountMeta::new_readonly(snapshot.terms.staging.key, false),
        AccountMeta::new_readonly(snapshot.token_behavior.raw.key, false),
        AccountMeta::new_readonly(snapshot.token_behavior.staging.key, false),
        AccountMeta::new(coordinate.shard_mint.key, false),
        AccountMeta::new_readonly(snapshot.token_program.key, false),
    ];
    if forwarded.len() != FRACTIONAL_RETIREMENT_COORDINATE_ACCOUNT_COUNT_V3 {
        return Err(Error::AccountFrame);
    }
    let mut accounts = vec![AccountMeta::new_readonly(
        snapshot.claims.program.key,
        false,
    )];
    accounts.extend(forwarded);
    let mut data = vec![0_u8];
    data.extend_from_slice(&request_bytes);
    Ok(Instruction {
        program_id: snapshot.trading.program.key,
        accounts,
        data,
    })
}

fn build_finish(
    snapshot: &FractionalRetirementSnapshotV3,
    request: FractionalRetirementRequestV3,
) -> Result<Instruction> {
    let accounts = vec![
        AccountMeta::new_readonly(snapshot.claims_market.key, false),
        AccountMeta::new_readonly(snapshot.registry_program.key, false),
        AccountMeta::new_readonly(snapshot.rent_sysvar.key, false),
        AccountMeta::new_readonly(snapshot.trading.program.key, false),
        AccountMeta::new_readonly(snapshot.claims.program.key, false),
        AccountMeta::new_readonly(snapshot.root.key, false),
        AccountMeta::new(snapshot.rent_credit.key, false),
        AccountMeta::new_readonly(snapshot.rent.program.key, false),
        AccountMeta::new(snapshot.cursor.key, false),
        AccountMeta::new_readonly(snapshot.terms.raw.key, false),
        AccountMeta::new_readonly(snapshot.terms.staging.key, false),
        AccountMeta::new_readonly(snapshot.token_behavior.raw.key, false),
        AccountMeta::new_readonly(snapshot.token_behavior.staging.key, false),
    ];
    if accounts.len() != FRACTIONAL_RETIREMENT_FINISH_ACCOUNT_COUNT_V3 {
        return Err(Error::AccountFrame);
    }
    Ok(Instruction {
        program_id: snapshot.claims.program.key,
        accounts,
        data: request
            .to_bytes()
            .map_err(Error::FractionalRetirement)?
            .to_vec(),
    })
}

const fn consequence_v3(action: FractionalRetirementActionV3) -> (&'static str, &'static str) {
    match action {
        FractionalRetirementActionV3::Begin => (
            "creates the ordered retirement cursor and freezes its exact terms-owned width",
            "reacquire finalized state and rebuild the next act if this handoff expires",
        ),
        FractionalRetirementActionV3::RetireCoordinate => (
            "closes the exact next empty reserve Position, admission, and zero-supply shard Mint",
            "reacquire finalized state; the cursor will select the same coordinate until it closes",
        ),
        FractionalRetirementActionV3::Finish => (
            "closes the completed cursor and settles its entire live lamport balance to RentCredit",
            "reacquire finalized state and rebuild finish if this handoff expires",
        ),
    }
}
