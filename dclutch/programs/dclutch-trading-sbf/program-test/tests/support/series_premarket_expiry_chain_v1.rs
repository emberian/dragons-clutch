//! Canonical current-source chain corpus for the pre-Market Series Expire campaign.
//!
//! The release compiler and the production Series operator remain the only
//! owners of action selection, artifact bytes, and logical-to-physical packing.
//! This module owns only concrete ProgramTest accounts around that selected
//! release: finalized records, replay prestates, child-program frames, and the
//! exact poststates expected after the real ELFs return successfully.

#![allow(dead_code)]

use dclutch_vm::account_profile::{v2::AccountPrestateV2, v3::AccountProfileV3};
use dclutch_market::capability_manifest::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityManifestV1, CompartmentFundingV1, EMPTY_MANIFEST_BYTES,
    FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    hot_v3::{HOT_FIXED_ACCOUNT_COUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3},
    set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    v4::CapabilityProgramV4,
};
use dclutch_chain_bundle_builder::{
    WaistFactsV1,
    artifacts::{ArtifactSetV1, DerivedRecordV1, derive_record},
    bundle::{BuiltBundleV1, BundleInputV1, FixedCorpusV1, ScenarioV1, build_bundle},
    frame::{
        BuiltAccountV1, data_account, external_with_view, pack_frame, program_with_deployed_view,
        program_with_view, rent_sysvar_bytes, system_program_builtin, vacant,
    },
};
use dclutch_claims::{
    founding_v5::{
        ClaimsFoundingAggregateSeedsV5, ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5,
    },
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionSeedsV2,
    },
};
use dclutch_custody::{
    CUSTODY_POSTSTATE_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CUSTODY_REQUEST_BYTES_V1, CallerRoleV1,
    CompartmentV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1, CustodyReplayV1,
    CustodyVaultSeedsV1, PROJECTED_CUSTODY_STATE_BYTES_V2, PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
    ProjectedCustodyCallerSeedsV1, ProjectedCustodyLockReceiptV1, ProjectedCustodyOperationV1,
    ProjectedCustodyPhaseV1, ProjectedCustodyReceiptV1, ProjectedCustodyRequestV1,
    ProjectedCustodyStateSeedsV2, ProjectedCustodyStateV2,
};
use dclutch_direct_hot_program_test_support::waist::{
    Elves, REGISTRY_PROGRAM_ID, Releases, fixture_substrate, programdata, programdata_v2,
    registry_hot_instruction, release_v2,
};
use dclutch_market::{
    Action as CoreAction, CoreState, FoundingIntentV5, Identity, MarketCoreStateSeedsV2,
    MarketIdentity, PRODUCT_GRAPH_BUMP_COUNT, Phase, ProductGraphBumpsV1, ProjectFoundReceiptV2,
    Readiness, Request as CoreRequest, SERIES_FOUNDING_PERMIT_BYTES_V1, SeriesCoreActionV1,
    SeriesCoreRequestV1, SeriesFoundingPermitSeedsV1, SeriesFoundingPermitV1,
    SeriesPermitExpiryRequestV1, SeriesUnallocatedPermitExpiryRequestV1, StateBumpsV1,
};
use dclutch_operator::{Finality, Observation, ObservedAccount};
use dclutch_operator::{
    direct_inline_v3::ObservedAccountMetaV3,
    series_hot_v3::{
        SeriesCurrentHotPlanV5, SeriesCurrentHotStateV5, SeriesSelectedHotReportV5,
        inspect_current_series_hot_v5,
    },
    series_lifecycle_v3::{SeriesCurrentOccurrenceV3, SeriesLifecycleSnapshotV3},
};
use dclutch_product::payoff::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{
        BasisInputV3, BasisKindV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3, basis_record_bytes_v3,
        compile_basis_v3, semantic_basis_preimage_v3,
    },
};
use dclutch_product::{
    ContentId as ProductContentId, PortfolioInputV2, ResultDomainInputV2, compile_portfolio_v2,
    compile_result_domain_v2, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product::admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1,
};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1,
    ExecutionRoleBindingV1, ExecutionRoleV1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2, ProtocolInfrastructureProfileV2,
};
use dclutch_market::rent::{
    RefundAuthority,
    lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleAccountIdV2, LifecycleRentCreditV2},
};
use dclutch_custody::token_svm::{
    ACCOUNT_BYTES, LEGACY_TOKEN_PROGRAM_ID, MINT_BYTES, Mint, PRODUCTION_ADAPTER_RELEASES,
    TokenAccount,
};
/// Occurrence count of the Template this chain stages: exactly one.
///
/// `series_proof_count_v3(1) == 0`, so the canonical family request for every
/// occurrence action here is the bare 128-byte header, the Expire Effect
/// declares no borrowed proof range, and the Expire RequestProfile pins 128.
/// This is release geometry, so it must be the SAME number the Template record
/// carries -- one author, read twice.
const SERIES_PREMARKET_TEMPLATE_OCCURRENCE_COUNT_V1: u32 = 1;

use dclutch_trading_sbf::series::account_profile_v4::SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4;
use dclutch_trading_sbf::series::state::{
    SeriesStateV3, TicketPhaseV3, TicketStateSeedsV3, TicketStateV3,
};
use dclutch_trading_sbf::series::{
    AuthenticatedProductProjectionV2, SERIES_OCCURRENCE_BYTES_V3, SERIES_TEMPLATE_BYTES_V3,
    SERIES_TICKET_BYTES_V3, admit_occurrence, admit_ticket,
    consume_artifacts_v4::SeriesConsumeChildRequestsV4,
    consume_series_escrow_v3,
    custody_v3::{
        SeriesCustodyPhysicalV3, project_prepare_custody_v3, project_terminal_custody_v3,
    },
    expire_funding_artifacts_v5::{
        SERIES_EXPIRE_CUSTODY_PROGRAM_COORDINATE_V5, SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5,
        SeriesExpireAccountProfileInputV5, SeriesExpireChildRequestsV5,
    },
    expire_series_escrow_v3, future_market_projection,
    instruction::{SeriesActionV3, encode_series_action_header_v3},
    occurrence_artifacts_v4::SeriesPrepareChildRequestsV4,
    occurrence_content_id, pre_founding_series_escrow,
    prepare_funding_artifacts_v5::{
        SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5, SeriesPrepareAccountProfileInputV5,
    },
    prepare_series_escrow_v3,
    projected_custody_v3::{
        SeriesProjectedCustodyPhysicalV3, project_abort_v3, project_consume_v3,
    },
    release_v5::{
        SeriesCurrentReleaseInputV5, SeriesOwnedReleaseSourceV5, SeriesReleaseV5,
        SeriesSelectedActionV5, authenticate_series_selected_action_v5, compile_series_release_v5,
        emit_current_series_release_source_v5,
    },
    template_content_id,
};
use solana_account::Account;
use solana_program::{
    clock::Clock,
    hash::{hash, hashv},
    instruction::Instruction,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};

use super::series_premarket_expiry_v1::{
    SeriesExpectedAccountTransitionV1, SeriesPremarketExpiryInstallAccountV1,
};

/// Release-waist and executable identities already selected by the parent test.
pub struct SeriesPremarketExpiryChainInputV1<'a> {
    /// Registry activation cache and release-set identity.
    pub releases: Releases,
    /// Exact real ELF bodies installed by the parent ProgramTest waist.
    pub elves: &'a Elves,
    /// Same-bank canonical Rent observation.
    pub rent: Rent,
    /// Registry program.
    pub registry_program: Pubkey,
    /// Trading program.
    pub trading_program: Pubkey,
    /// Core program.
    pub core_program: Pubkey,
    /// Claims program.
    pub claims_program: Pubkey,
    /// Custody program.
    pub custody_program: Pubkey,
    /// Lifecycle Rent program.
    pub rent_program: Pubkey,
}

/// Complete concrete input and expected output for one positive real-ELF run.
pub struct SeriesPremarketExpiryChainFixtureV1 {
    /// Canonical five-entry release compiled from current semantic owners.
    pub release: SeriesReleaseV5,
    /// Reauthenticated Expire selection from that exact release.
    pub selected: SeriesSelectedActionV5,
    /// Production operator report; the test compares it with physical evidence.
    pub operator_report: SeriesSelectedHotReportV5,
    /// Exact accounts owned by this fixture, including named external accounts.
    pub install_accounts: Vec<SeriesPremarketExpiryInstallAccountV1>,
    /// Release-waist, executable, ProgramData, sysvar, and System identities
    /// already installed by the parent harness.
    pub externally_installed: Vec<Pubkey>,
    /// Trading Hot instruction before Registry wrapping.
    pub hot_instruction: Instruction,
    /// Top-level TRANSPARENT Registry Hot continuation submitted to
    /// ProgramTest: the `hot_continuation_v2` seam, whose instruction data is
    /// the Trading Hot bytes with nothing in front of them.
    ///
    /// It used to be the legacy `continuation_v1` container, and that is a
    /// seam Trading refuses on purpose. `authenticate_hot_invocation_v3`
    /// requires the instructions-sysvar record of the top-level instruction to
    /// carry the SAME bytes Trading received (`hot_v3.rs`
    /// `observed.data() != instruction_data`), and the legacy seam forwards
    /// only the stripped continuation, so its header is observable at the
    /// child and the frame refuses `NativeSignature` before any Series
    /// semantics run. That is not a discovery: `registry_hot_continuation.rs`
    /// asserts exactly this outcome for exactly this reason in
    /// `a_legacy_headered_hot_container_takes_the_v1_seam_and_not_the_transparent_one`.
    pub top_level_instruction: Instruction,
    /// Packed runtime keys in selected physical-ordinal order.
    pub runtime_physical_accounts: Vec<Pubkey>,
    /// Material account keys in exact snapshot/transition order.
    pub material_snapshot_keys: Vec<Pubkey>,
    /// Byte-and-lamport exact successful transitions.
    pub success_transitions: Vec<SeriesExpectedAccountTransitionV1>,
    /// Parent Trading Series root.
    pub parent_root: Pubkey,
    /// Prepared Ticket replay PDA.
    pub ticket_state: Pubkey,
    /// Future Market, intentionally still vacant.
    pub future_market: Pubkey,
    /// Still-unallocated, prefunded Core permit PDA.
    pub permit_account: Pubkey,
    /// Lifecycle RentCredit receiving all closed-account refunds.
    pub rent_credit: Pubkey,
    /// Trading-derived readonly caller for Core's atomic precommit expiry.
    pub precommit_caller: Pubkey,
    /// Exact composite-root prestate.
    pub parent_root_prestate: Vec<u8>,
    /// Exact prepared Ticket prestate.
    pub ticket_prestate: Vec<u8>,
    /// Kernel-derived composite-root replacement.
    pub root_poststate: Vec<u8>,
    /// Kernel-derived expired Ticket replacement.
    pub ticket_poststate: Vec<u8>,
}

/// Stable construction refusal. Each variant names a fixture seam and never a
/// protocol refusal code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesPremarketExpiryChainErrorV1 {
    /// One required identity was zero or two semantic roles aliased.
    Identity,
    /// Product, Series, Realm, or capability record encoding refused.
    Record,
    /// Current-source five-action release emission or selection refused.
    Release,
    /// A child request differed from the Series semantic projection.
    ChildRequest,
    /// Fixed or runtime physical account construction differed from Profile13.
    Physical,
    /// Root/Ticket replay evaluation did not yield exact replacements.
    Replay,
    /// Production operator inspection did not return a ready Expire report.
    Operator,
}

/// Build one exact current-source positive Series Expire fixture.
///
/// This entrypoint is intentionally total over its input. It never installs an
/// account and never reaches a validator; the caller owns those two operations.
pub fn build_series_premarket_expiry_chain_v1(
    input: SeriesPremarketExpiryChainInputV1<'_>,
) -> Result<SeriesPremarketExpiryChainFixtureV1, SeriesPremarketExpiryChainErrorV1> {
    validate_input(&input)?;
    let product = build_product_record_corpus_v1()?;
    let realm = build_realm_record_v1(key(0x68))?;
    let founder = derived_fixture_key_v1(
        b"dclutch/test/series-expire/founder",
        input.registry_program,
        input.releases.release_set,
    );
    let refund_beneficiary = derived_fixture_key_v1(
        b"dclutch/test/series-expire/refund-beneficiary",
        input.registry_program,
        input.releases.release_set,
    );
    // ONE WALLET, NAMED BY THREE AUTHORITIES. The Template's refund owner, the
    // Ticket's refund owner and the lifecycle RentCredit's `refund_wallet` are
    // required to be the same key by the Series kernel
    // (`terminal.rs::requires_wallet`), by Core
    // (`series_permit_expiry.rs::authenticate_rent_credit_coordinates`) and by
    // Trading's pre-CPI mirror of it -- and the RentCredit is the ACCOUNT the
    // refunded lamports land in, never the beneficiary they are credited to.
    //
    // This fixture used to stage the Ticket's refund owner as the RentCredit's
    // own ADDRESS, and derived that address in a throwaway first pass over the
    // record corpus for no other purpose. All three now name
    // `refund_beneficiary`, so the corpus is built once and the RentCredit is
    // derived from it rather than the other way round.
    let records = build_series_record_corpus_v1(SeriesRecordInputV1 {
        realm: hash(&realm).to_bytes(),
        release_set: input.releases.release_set,
        product_record: hash(&product.product).to_bytes(),
        product_id: product.product_id,
        result_domain: product.result_domain_digest,
        capability_manifest: hash(&EMPTY_MANIFEST_BYTES).to_bytes(),
        registry_program: input.registry_program,
        core_program: input.core_program,
        founder,
        template_refund_owner: refund_beneficiary,
        ticket_refund_owner: refund_beneficiary,
        close_rent: input.rent.minimum_balance(32),
    })?;
    let substrate = build_root_independent_substrate_v1(
        &input.rent,
        input.registry_program,
        input.trading_program,
        input.custody_program,
        input.core_program,
        input.rent_program,
        input.releases.release_set,
        refund_beneficiary,
        &product,
        &realm,
        &records,
    )?;
    if substrate.future_market.key != records.future_market
        || substrate.replay.ticket_before
            != TicketStateV3::prepared(records.ticket_id).encode().to_vec()
    {
        return Err(SeriesPremarketExpiryChainErrorV1::Physical);
    }
    let infrastructure = build_core_infrastructure_corpus_v1(&input)?;
    let provisional_root = derived_fixture_key_v1(
        b"dclutch/test/series-expire/provisional-root",
        input.trading_program,
        records.template_id.to_bytes(),
    );
    let projected = build_projected_custody_corpus_v1(
        &input.rent,
        input.registry_program,
        input.trading_program,
        input.core_program,
        input.custody_program,
        input.rent_program,
        input.releases.release_set,
        provisional_root,
        &product,
        &realm,
        &records,
        &substrate.normal_custody,
        substrate.rent_credit.key,
    )?;
    let provisional_release_source = build_current_release_tranche(
        &input,
        &product,
        &records,
        &substrate,
        &projected,
        provisional_root,
        &expire_fixed_data_lengths_v1(&input, &substrate, &projected, &infrastructure)?,
    )?;
    let provisional_release = compile_series_release_v5(provisional_release_source.as_source())
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Release)?;
    let controller = build_controller_corpus_v1(
        &input.rent,
        input.registry_program,
        input.trading_program,
        input.core_program,
        input.rent_program,
        input.releases.release_set,
        refund_beneficiary,
        &records,
        &substrate.finalized,
        product.product_id,
        &substrate.replay,
        &provisional_release,
    )?;
    let projected = build_projected_custody_corpus_v1(
        &input.rent,
        input.registry_program,
        input.trading_program,
        input.core_program,
        input.custody_program,
        input.rent_program,
        input.releases.release_set,
        controller.root.key,
        &product,
        &realm,
        &records,
        &substrate.normal_custody,
        substrate.rent_credit.key,
    )?;
    let expire_lengths =
        expire_fixed_data_lengths_v1(&input, &substrate, &projected, &infrastructure)?;
    let release_source = build_current_release_tranche(
        &input,
        &product,
        &records,
        &substrate,
        &projected,
        controller.root.key,
        &expire_lengths,
    )?;
    let release = compile_series_release_v5(release_source.as_source())
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Release)?;
    let selected = authenticate_series_selected_action_v5(
        &release,
        release_source.as_source(),
        &substrate.normal_custody.family_request,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Release)?;
    if release.program_set != provisional_release.program_set
        || release.descriptors != provisional_release.descriptors
        || release.strategies != provisional_release.strategies
        || release.artifact_ids != provisional_release.artifact_ids
        || selected.action != SeriesActionV3::Expire
        || selected.geometry.logical_fixed_accounts != SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5
        || controller.market.key == records.future_market
    {
        return Err(SeriesPremarketExpiryChainErrorV1::Release);
    }
    let (bundle, precommit_caller) = build_expire_bundle_v1(
        &input,
        &records,
        &substrate,
        &projected,
        &infrastructure,
        &controller,
        &release,
        &selected,
    )?;
    audit_expire_profile_data_lengths_v1(&bundle, &selected)?;
    let (operator_report, runtime_physical_accounts) = build_operator_report_v1(
        &input,
        &product,
        &records,
        &substrate,
        &projected,
        &controller,
        &bundle,
        &expire_lengths,
    )?;
    if operator_report.selected != selected || operator_report.instruction != bundle.hot_instruction
    {
        std::eprintln!(
            "Series Expire operator report differs from the bundle: selected_eq={} \
             instruction_eq={}",
            operator_report.selected == selected,
            operator_report.instruction == bundle.hot_instruction,
        );
        return Err(SeriesPremarketExpiryChainErrorV1::Operator);
    }
    let success_transitions =
        build_success_transitions_v1(&controller, &substrate, &projected, &precommit_caller)?;
    let material_snapshot_keys = success_transitions
        .iter()
        .map(|transition| transition.key)
        .collect::<Vec<_>>();
    let root_poststate = success_transitions
        .iter()
        .find(|transition| transition.key == controller.root.key)
        .and_then(|transition| transition.after.as_ref())
        .map(|account| account.data.clone())
        .ok_or(SeriesPremarketExpiryChainErrorV1::Replay)?;
    let ticket_poststate = success_transitions
        .iter()
        .find(|transition| transition.key == controller.ticket_state.key)
        .and_then(|transition| transition.after.as_ref())
        .map(|account| account.data.clone())
        .ok_or(SeriesPremarketExpiryChainErrorV1::Replay)?;
    let (install_accounts, externally_installed) = build_install_accounts_v1(
        &input,
        &substrate,
        &projected,
        &infrastructure,
        &controller,
        &bundle,
    )?;
    let hot_instruction = bundle.hot_instruction;
    let top_level_instruction = registry_hot_instruction(input.releases, hot_instruction.clone()).0;
    let _current_source_owner = release_source;
    Ok(SeriesPremarketExpiryChainFixtureV1 {
        release,
        selected,
        operator_report,
        install_accounts,
        externally_installed,
        hot_instruction,
        top_level_instruction,
        runtime_physical_accounts,
        material_snapshot_keys,
        success_transitions,
        parent_root: controller.root.key,
        ticket_state: controller.ticket_state.key,
        future_market: substrate.future_market.key,
        permit_account: substrate.permit_account.key,
        rent_credit: substrate.rent_credit.key,
        precommit_caller: precommit_caller.key,
        parent_root_prestate: controller.root.account.data.clone(),
        ticket_prestate: controller.ticket_state.account.data.clone(),
        root_poststate,
        ticket_poststate,
    })
}

/// Emit all five current actions from their production semantic owners.
///
/// This dependency-ordered tranche is intentionally useful on its own: it
/// proves the Expire body lives inside one complete current successor set and
/// cannot be substituted by an Expire-only fixture release.  The concrete
/// graph replaces the deterministic semantic corpus below with its exact
/// record-derived child requests before submission.
fn build_current_release_tranche(
    input: &SeriesPremarketExpiryChainInputV1<'_>,
    product: &ProductRecordCorpusV1,
    records: &SeriesRecordCorpusV1,
    substrate: &RootIndependentSubstrateV1,
    projected: &ProjectedCustodyCorpusV1,
    parent_root: Pubkey,
    expire_lengths: &[u32; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize],
) -> Result<SeriesOwnedReleaseSourceV5, SeriesPremarketExpiryChainErrorV1> {
    with_current_release_input_v1(
        input,
        product,
        records,
        substrate,
        projected,
        parent_root,
        expire_lengths,
        |current| {
            emit_current_series_release_source_v5(current)
                .map_err(|_| SeriesPremarketExpiryChainErrorV1::Release)
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn with_current_release_input_v1<R>(
    input: &SeriesPremarketExpiryChainInputV1<'_>,
    product: &ProductRecordCorpusV1,
    records: &SeriesRecordCorpusV1,
    substrate: &RootIndependentSubstrateV1,
    projected: &ProjectedCustodyCorpusV1,
    parent_root: Pubkey,
    expire_lengths: &[u32; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize],
    consume: impl FnOnce(
        SeriesCurrentReleaseInputV5<'_>,
    ) -> Result<R, SeriesPremarketExpiryChainErrorV1>,
) -> Result<R, SeriesPremarketExpiryChainErrorV1> {
    let admitted = admit_occurrence(&records.template, &records.occurrence, &[])
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let admitted_ticket =
        admit_ticket(&records.ticket).map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let occurrence = admitted.occurrence();
    let template_record = admitted.template();
    let ticket = admitted_ticket.ticket();
    let funds = occurrence.funds();
    let ticket_state_key = Pubkey::find_program_address(
        &TicketStateSeedsV3::new(parent_root.to_bytes(), records.ticket_id).as_slices(),
        &input.trading_program,
    )
    .0;
    let core_request = |action| {
        SeriesCoreRequestV1::occurrence(
            action,
            identity(template_record.release_set().to_bytes())?,
            identity(records.template_id.to_bytes())?,
            identity(ticket_state_key.to_bytes())?,
            identity(occurrence.market().to_bytes())?,
            identity(template_record.realm().to_bytes())?,
            identity(occurrence.product_record().to_bytes())?,
            identity(ticket.refund_owner().to_bytes())?,
            identity(ticket.founder().to_bytes())?,
            occurrence.occurrence(),
            1,
            0,
            funds.market_rent(),
            funds.capability_native(),
            funds.founding_work(),
            funds.hoard_principal(),
        )
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)
    };
    let consume_core_request = core_request(SeriesCoreActionV1::Consume)?;
    let consume_core = consume_core_request
        .encode()
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let lock = ProjectedCustodyRequestV1::decode(&projected.consume_lock)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let realize = ProjectedCustodyRequestV1::decode(&projected.consume_realize)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    if lock.parent_capability_root != parent_root.to_bytes()
        || realize.parent_capability_root != parent_root.to_bytes()
        || lock.resulting_revision.checked_add(1) != Some(realize.resulting_revision)
    {
        return Err(SeriesPremarketExpiryChainErrorV1::ChildRequest);
    }
    let lock_receipt = ProjectedCustodyLockReceiptV1 {
        market: lock.market,
        release_set: lock.release_set,
        context_digest: lock.context_digest,
        source_vault: lock.funding_source_vault,
        source_replay: substrate.normal_custody.replay.to_bytes(),
        hoard_vault: lock.hoard_vault,
        rent_credit: lock.rent_credit,
        request_digest: hash(&projected.consume_lock).to_bytes(),
        amount: lock.amount,
        source_vault_rent_lamports: lock.funding_source_vault_rent_lamports,
        source_replay_rent_lamports: lock.funding_source_state_rent_lamports,
        resulting_revision: lock.resulting_revision,
    }
    .encode()
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let projected_receipt = ProjectedCustodyReceiptV1 {
        realized: true,
        aborted_open: false,
        market: realize.market,
        release_set: realize.release_set,
        parent_capability_root: parent_root.to_bytes(),
        context_digest: realize.context_digest,
        hoard_vault: realize.hoard_vault,
        amount: realize.amount,
        request_digest: hash(&projected.consume_realize).to_bytes(),
        market_state_digest: hash(&consume_core).to_bytes(),
        rent_credit: realize.rent_credit,
        resulting_revision: realize.resulting_revision,
    }
    .encode()
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let permit_seeds = SeriesFoundingPermitSeedsV1::new(
        identity(input.releases.release_set)?,
        identity(records.future_market.to_bytes())?,
        identity(records.ticket_id.to_bytes())?,
    );
    let (permit_key, permit_bump) =
        Pubkey::find_program_address(&permit_seeds.as_slices(), &input.core_program);
    if permit_key != substrate.permit_account.key {
        return Err(SeriesPremarketExpiryChainErrorV1::Identity);
    }
    // The normal Custody replay has revision three after its three Prepare
    // requests. Claims founding creates an independent Market-position replay
    // and its canonical first committed revision is one.
    const CLAIMS_FOUNDING_REPLAY_REVISION: u64 = 1;
    let intent = FoundingIntentV5::new(
        permit_bump,
        identity(input.releases.release_set)?,
        identity(records.future_market.to_bytes())?,
        identity(hash(&product.product).to_bytes())?,
        identity(occurrence.resolution_policy().to_bytes())?,
        identity(ticket.founder().to_bytes())?,
        identity(records.ticket_id.to_bytes())?,
        identity(parent_root.to_bytes())?,
        identity(projected.state.to_bytes())?,
        identity(substrate.normal_custody.escrow_vault.to_bytes())?,
        identity(projected.hoard_vault.to_bytes())?,
        identity(hash(&projected.consume_realize).to_bytes())?,
        identity(hash(&projected_receipt).to_bytes())?,
        identity(input.trading_program.to_bytes())?,
        identity(input.claims_program.to_bytes())?,
        identity(substrate.rent_credit.key.to_bytes())?,
        records.generation,
        funds.hoard_principal(),
        1,
        records.expiry_slot,
        realize.resulting_revision,
        CLAIMS_FOUNDING_REPLAY_REVISION,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let intent_digest = hash(
        &intent
            .encode()
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?,
    )
    .to_bytes();
    let aggregate = Pubkey::find_program_address(
        &ClaimsFoundingAggregateSeedsV5::new(records.future_market.to_bytes())
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?
            .as_slices(),
        &input.claims_program,
    )
    .0;
    let position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), ticket.founder().to_bytes())
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?
            .as_slices(),
        &input.claims_program,
    )
    .0;
    let admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), ticket.founder().to_bytes())
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?
            .as_slices(),
        &input.claims_program,
    )
    .0;
    const CLAIM_COUNT: u32 = 3;
    let aggregate_rent = input
        .rent
        .minimum_balance(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + CLAIM_COUNT as usize * 8);
    let position_rent = input
        .rent
        .minimum_balance(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + CLAIM_COUNT as usize * 8);
    let admission_rent = input
        .rent
        .minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2);
    let consume_claims = ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
        release_set: input.releases.release_set,
        market: records.future_market.to_bytes(),
        product_record_digest: hash(&product.product).to_bytes(),
        product_instance_id: product.product_id,
        linked_basis_record_digest: hash(&product.linked_basis).to_bytes(),
        semantic_basis_id: product.semantic_basis_id,
        founder: ticket.founder().to_bytes(),
        founding_intent_digest: intent_digest,
        aggregate: aggregate.to_bytes(),
        position: position.to_bytes(),
        admission: admission.to_bytes(),
        funding_source: substrate.normal_custody.escrow_vault.to_bytes(),
        hoard: projected.hoard_vault.to_bytes(),
        custody_replay: projected.state.to_bytes(),
        rent_credit: substrate.rent_credit.key.to_bytes(),
        rent_program: input.rent_program.to_bytes(),
        claims_program: input.claims_program.to_bytes(),
        trading_program: input.trading_program.to_bytes(),
        custody_request_digest: hash(&projected.consume_lock).to_bytes(),
        custody_receipt_digest: hash(&lock_receipt).to_bytes(),
        generation: records.generation,
        claim_count: CLAIM_COUNT,
        quantity: funds.hoard_principal(),
        basis_scale: 1,
        pre_source_amount: funds.hoard_principal(),
        post_source_amount: 0,
        pre_hoard_amount: 0,
        post_hoard_amount: funds.hoard_principal(),
        pre_custody_revision: CLAIMS_FOUNDING_REPLAY_REVISION - 1,
        post_custody_revision: CLAIMS_FOUNDING_REPLAY_REVISION,
        aggregate_rent_principal: aggregate_rent,
        position_rent_principal: position_rent,
        admission_rent_principal: admission_rent,
        observed_aggregate_lamports: aggregate_rent,
        observed_position_lamports: position_rent,
        observed_admission_lamports: admission_rent,
        pre_aggregate_revision: 0,
        post_aggregate_revision: 1,
        pre_position_revision: 0,
        post_position_revision: 1,
    })
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?
    .to_bytes();
    let permit = SeriesFoundingPermitV1::new(
        intent,
        identity(intent_digest)?,
        identity(hash(&consume_claims).to_bytes())?,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let expire_core = core_request(SeriesCoreActionV1::Expire)?;
    let prepare_lengths = [0_u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize];
    let consume_lengths = [0_u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4];
    consume(SeriesCurrentReleaseInputV5 {
        template: records.template_id,
        template_occurrence_count: SERIES_PREMARKET_TEMPLATE_OCCURRENCE_COUNT_V1,
        consume_shadow_certificate_program: dclutch_core_contract::ContentId::new(
            hash(input.elves.trading.as_slice()).to_bytes(),
        )
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Identity)?,
        prepare_profile: SeriesPrepareAccountProfileInputV5 {
            fixed_data_lengths: &prepare_lengths,
        },
        prepare_requests: SeriesPrepareChildRequestsV4 {
            projected_initialize: &projected.prepare_initialize,
            projected_open: &projected.prepare_open,
            replay_initialize: &substrate.normal_custody.prepare_initialize,
            escrow_open: &substrate.normal_custody.prepare_open,
            escrow_lock: &substrate.normal_custody.prepare_lock,
        },
        prepare_ticket_rent_lamports: input.rent.minimum_balance(64),
        consume_observed_data_lengths: &consume_lengths,
        consume_requests: SeriesConsumeChildRequestsV4 {
            lock: &projected.consume_lock,
            core: &consume_core,
            realize: &projected.consume_realize,
            claims: &consume_claims,
        },
        consume_funding_count: 1,
        expire_profile: SeriesExpireAccountProfileInputV5 {
            fixed_data_lengths: expire_lengths,
        },
        expire_requests: SeriesExpireChildRequestsV5 {
            refund: &substrate.normal_custody.expire_refund,
            close_vault: &substrate.normal_custody.expire_close_vault,
            close_replay: &substrate.normal_custody.expire_close_replay,
            projected_abort: &projected.expire_abort,
            permit_expiry: SeriesPermitExpiryRequestV1::new(permit),
            core_expire: expire_core,
        },
    })
}

#[derive(Clone)]
struct ControllerCorpusV1 {
    market: BuiltAccountV1,
    manifest: DerivedRecordV1,
    root: BuiltAccountV1,
    ticket_state: BuiltAccountV1,
    generation: u64,
}

/// Reconstruct the exact activation projection for the already-live Series
/// controller. The controller manifest selects the current ProgramSet, while
/// the occurrence record independently selects the canonical empty manifest
/// for its still-vacant child Market.
#[allow(clippy::too_many_arguments)]
fn build_controller_corpus_v1(
    rent: &Rent,
    registry_program: Pubkey,
    trading_program: Pubkey,
    core_program: Pubkey,
    rent_program: Pubkey,
    release_set: [u8; 32],
    refund_beneficiary: Pubkey,
    records: &SeriesRecordCorpusV1,
    finalized: &FinalizedRecordCorpusV1,
    product_id: [u8; 32],
    replay: &RootIndependentReplayCorpusV1,
    release: &SeriesReleaseV5,
) -> Result<ControllerCorpusV1, SeriesPremarketExpiryChainErrorV1> {
    const CONTROLLER_GENERATION: u64 = 9;
    let descriptor = CapabilityProgramV4::decode(
        release
            .descriptors
            .get(SeriesActionV3::Expire as usize)
            .ok_or(SeriesPremarketExpiryChainErrorV1::Release)?,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Release)?;
    // ONE AUTHOR. The root's config identity is the Registry RECORD DIGEST of
    // its config record, and the Series config record IS the Template record --
    // the action descriptor's `config_schema()` is
    // `SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3`, the same schema the Template is
    // installed under. So this digest names the account already installed at
    // `finalized.template.raw`, the manifest entry and the root selection
    // agree with it, and the bundle builder derives the config raw/staging
    // coordinates onto that same record. Staging `records.template_id` here --
    // the DOMAIN-SEPARATED Template content identity -- named a coordinate at
    // which no Registry record can ever exist, because a record's coordinate is
    // its own hash.
    let config_digest = hash(&records.template).to_bytes();
    let amounts = FundingAmountsV1::new(
        CompartmentFundingV1::native_lamports(1)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let entry = CapabilityEntryV1::new(
        capability_content(descriptor.kind().to_bytes())?,
        capability_content(release.program_set_id)?,
        capability_content(config_digest)?,
        capability_content(descriptor.capacity_profile().to_bytes())?,
        capability_content(descriptor.root_schema().to_bytes())?,
        capability_content(descriptor.derivation_policy().to_bytes())?,
        ActivationPolicy::PrepaidLazy,
        records
            .expiry_slot
            .checked_add(100)
            .ok_or(SeriesPremarketExpiryChainErrorV1::Record)?,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(amounts, None)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let mut manifest_bytes = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let manifest = derive_record(
        registry_program,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        &manifest_bytes,
    );
    if manifest.digest == hash(&EMPTY_MANIFEST_BYTES).to_bytes() {
        return Err(SeriesPremarketExpiryChainErrorV1::Record);
    }
    let program_set_bumps = record_bumps_v1(
        registry_program,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        release.program_set_id,
    );
    let config_bumps = record_bumps_v1(
        registry_program,
        descriptor.config_schema().to_bytes(),
        config_digest,
    );
    let selection = CapabilityExecutionSelectionV1::new(
        0,
        dclutch_core_contract::ContentId::new(manifest.digest)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
        dclutch_core_contract::ContentId::new(descriptor.kind().to_bytes())
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
        dclutch_core_contract::ContentId::new(release.program_set_id)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
        dclutch_core_contract::ContentId::new(config_digest)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?
    .with_capability_release_record_bumps(program_set_bumps.0, program_set_bumps.1);
    if selection.executor_role() != ExecutionRoleV1::Trading {
        return Err(SeriesPremarketExpiryChainErrorV1::Record);
    }
    let manifest_bumps = record_bumps_v1(
        registry_program,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest.digest,
    );
    // THE CONTROLLER IS A LIVE FOUNDED MARKET, NOT A VACANT KEY.
    //
    // It was `vacant(derived_fixture_key_v1(...))`, and `authenticate_market`'s
    // very first conjunct refuses that: the fixed Market must be Core-owned and
    // exactly `STATE_BYTES` wide. `process_hot_execution_v3` says why in its own
    // words -- "the fixed Market is always the live Series controller ... the
    // occurrence's distinct future Market is a route-local account, never a
    // substitute for the fixed controller coordinate." Pre-Market is a claim
    // about the FUTURE Market, which stays vacant; the controller that owns the
    // Series capability root has been founded and open for as long as the root
    // has existed.
    //
    // Built the way a founding writes it, with `direct-hot/src/fixture.rs`'s
    // `market_and_claims` as the worked example: a provisional identity fixes
    // the address, the final identity carries that address, and both derive the
    // same pair because `MarketCoreStateSeedsV2` projects the identity
    // EXCLUDING the derived key. The recorded bumps are not decoration --
    // `market_core_state_address_v2` takes the recorded bump over the caller's
    // hint, and every Market reader reproduces the address from it and refuses
    // a wrong one, so an UNRECORDED tail would stage a market no founding
    // produces and quietly move all three readers onto their search arm.
    const CONTROLLER_RESOLUTION_POLICY: [u8; 32] = [0x77; 32];
    let provisional_identity = MarketIdentity {
        market_id: identity([0x78; 32])?,
        realm_id: identity(finalized.realm.digest)?,
        product_record: identity(finalized.product.digest)?,
        product_id: identity(product_id)?,
        resolution_policy: identity(CONTROLLER_RESOLUTION_POLICY)?,
        capability_manifest: identity(manifest.digest)?,
        selected_release_set: identity(release_set)?,
        registry_program: identity(registry_program.to_bytes())?,
        generation: CONTROLLER_GENERATION,
    };
    let controller_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(provisional_identity).as_slices(),
        &core_program,
    )
    .0;
    if controller_market == records.future_market {
        return Err(SeriesPremarketExpiryChainErrorV1::Identity);
    }
    let controller_identity = MarketIdentity {
        market_id: identity(controller_market.to_bytes())?,
        ..provisional_identity
    };
    let (founding_market, market_bump) = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(controller_identity).as_slices(),
        &core_program,
    );
    if founding_market != controller_market {
        return Err(SeriesPremarketExpiryChainErrorV1::Identity);
    }
    let realm_record_bumps = record_bumps_v1(
        registry_program,
        REALM_SCHEMA_RELEASE_ID_V1,
        finalized.realm.digest,
    );
    let product_graph_bumps = {
        let mut bumps = [0_u8; PRODUCT_GRAPH_BUMP_COUNT];
        for (slot, record) in [
            &finalized.product,
            &finalized.result_domain,
            &finalized.portfolio,
            &finalized.linked_basis,
        ]
        .into_iter()
        .enumerate()
        {
            let (raw, staging) = record_bumps_v1(registry_program, record.schema, record.digest);
            *bumps
                .get_mut(slot * 2)
                .ok_or(SeriesPremarketExpiryChainErrorV1::Record)? = raw;
            *bumps
                .get_mut(slot * 2 + 1)
                .ok_or(SeriesPremarketExpiryChainErrorV1::Record)? = staging;
        }
        bumps
    };
    // One credit per Market lifecycle. This route never reads the controller's
    // own credit -- Expire refunds through the FUTURE Market's -- so the
    // account is not installed; the identity is derived rather than invented so
    // the state is one a founding could have written.
    let (controller_rent_credit, _) = build_lifecycle_rent_credit_v1(
        rent_program,
        controller_market,
        release_set,
        CONTROLLER_GENERATION,
        refund_beneficiary,
    )?;
    let controller_state = CoreState {
        phase: Phase::Open,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity: controller_identity,
        outstanding_capabilities: 1,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: identity(controller_rent_credit.to_bytes())?,
        terminal_receipt: None,
        bumps: StateBumpsV1 {
            market: StateBumpsV1::record(market_bump),
            realm_raw_record: StateBumpsV1::record(realm_record_bumps.0),
            realm_staging_record: StateBumpsV1::record(realm_record_bumps.1),
            product_graph: ProductGraphBumpsV1::record(product_graph_bumps),
        },
    };
    let controller_market_account = data_account(
        rent,
        controller_market,
        core_program,
        controller_state
            .encode()
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?
            .to_vec(),
    );
    let header = CapabilityRootHeaderV1::new(
        dclutch_core_contract::ContentId::new(release_set)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Identity)?,
        controller_market.to_bytes(),
        CONTROLLER_GENERATION,
        selection,
        SelectedRecordBumpsV1::new(
            manifest_bumps.0,
            manifest_bumps.1,
            config_bumps.0,
            config_bumps.1,
        ),
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let root_key = Pubkey::find_program_address(&header.seeds().as_slices(), &trading_program).0;
    let mut root_bytes =
        Vec::with_capacity(CAPABILITY_ROOT_HEADER_BYTES_V1 + replay.series_before.len());
    root_bytes.extend_from_slice(&header.to_bytes());
    root_bytes.extend_from_slice(&replay.series_before);
    let root = data_account(rent, root_key, trading_program, root_bytes);
    let ticket_state_key = Pubkey::find_program_address(
        &TicketStateSeedsV3::new(root_key.to_bytes(), records.ticket_id).as_slices(),
        &trading_program,
    )
    .0;
    let ticket_state = data_account(
        rent,
        ticket_state_key,
        trading_program,
        replay.ticket_before.clone(),
    );
    Ok(ControllerCorpusV1 {
        market: controller_market_account,
        manifest,
        root,
        ticket_state,
        generation: CONTROLLER_GENERATION,
    })
}

fn record_bumps_v1(registry: Pubkey, schema: [u8; 32], digest: [u8; 32]) -> (u8, u8) {
    (
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).1,
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).1,
    )
}

fn capability_content(
    bytes: [u8; 32],
) -> Result<dclutch_market::capability_manifest::ContentId, SeriesPremarketExpiryChainErrorV1> {
    dclutch_market::capability_manifest::ContentId::new(bytes)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Identity)
}

fn identity(bytes: [u8; 32]) -> Result<Identity, SeriesPremarketExpiryChainErrorV1> {
    Identity::new(bytes).map_err(|_| SeriesPremarketExpiryChainErrorV1::Identity)
}

fn id(byte: u8) -> dclutch_core_contract::ContentId {
    dclutch_core_contract::ContentId::new([byte; 32]).expect("nonzero fixture content identity")
}

const fn key(byte: u8) -> Pubkey {
    Pubkey::new_from_array([byte; 32])
}

#[derive(Clone)]
struct ProductRecordCorpusV1 {
    product: Vec<u8>,
    result_domain: Vec<u8>,
    portfolio: Vec<u8>,
    linked_basis: Vec<u8>,
    product_id: [u8; 32],
    result_domain_digest: [u8; 32],
    semantic_basis_id: [u8; 32],
}

/// Compile one three-outcome Product graph through the public runtime and
/// linked-basis encoders. Every foreign key is a digest of the exact body the
/// fixture later finalizes; no Product coordinate is stated independently.
fn build_product_record_corpus_v1()
-> Result<ProductRecordCorpusV1, SeriesPremarketExpiryChainErrorV1> {
    const OUTCOME_COUNT: usize = 3;
    const OUTCOME_COUNT_U32: u32 = 3;
    let stable_product = product_content([0x81; 32])?;
    let coordinate_domain = product_content([0x82; 32])?;
    let result_unit = product_content([0x83; 32])?;
    let provisional_input = BasisInputV3 {
        kind: BasisKindV3::CategoricalQ1,
        product_id: stable_product.to_bytes(),
        result_domain_id: [0x84; 32],
        coordinate_domain_id: coordinate_domain.to_bytes(),
        result_unit_id: result_unit.to_bytes(),
        evaluator_release_id: [0x85; 32],
        basis_width: OUTCOME_COUNT_U32,
        payout_scale: 1,
        knot_denominator: 1,
        knots: &[],
        terms: &[],
        failure_payouts: &[],
        price_gate_certificate_digest: [0; 32],
    };
    let basis_bytes = basis_record_bytes_v3(BasisKindV3::CategoricalQ1, OUTCOME_COUNT, 0, 0)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let mut provisional = vec![0_u8; basis_bytes];
    compile_basis_v3(provisional_input, &mut provisional)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let preimage = semantic_basis_preimage_v3(&provisional)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let semantic_basis = hashv(&[
        SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        preimage.prefix(),
        preimage.suffix(),
    ])
    .to_bytes();

    let cuts = [0_i128];
    let mut result_domain = vec![
        0_u8;
        result_domain_record_bytes(cuts.len())
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?
    ];
    compile_result_domain_v2(
        ResultDomainInputV2 {
            product_id: stable_product,
            coordinate_domain_id: coordinate_domain,
            result_unit_id: result_unit,
            liability_basis_id: product_content(semantic_basis)?,
            representation_release_id: product_content([0x86; 32])?,
            mapping_release_id: product_content([0x87; 32])?,
            cut_denominator: 1,
            cuts: &cuts,
        },
        &mut result_domain,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let result_domain_digest = hash(&result_domain).to_bytes();
    let coefficients = [1_u64; OUTCOME_COUNT];
    let mut portfolio = vec![
        0_u8;
        portfolio_record_bytes(coefficients.len())
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?
    ];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id: stable_product,
            result_domain_id: product_content(result_domain_digest)?,
            claim_basis_id: product_content([0x88; 32])?,
            liability_basis_id: product_content(semantic_basis)?,
            representation_release_id: product_content([0x86; 32])?,
            denominator: 1,
            coefficients: &coefficients,
        },
        &mut portfolio,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let portfolio_digest = hash(&portfolio).to_bytes();
    let mut product = vec![0_u8; PRODUCT_RECORD_BYTES_V2];
    ProductRecordV2::new(
        stable_product,
        product_content(result_domain_digest)?,
        product_content(portfolio_digest)?,
    )
    .encode_into(&mut product)
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    ProductRecordV2::decode(&product).map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;

    let mut linked_basis = vec![0_u8; basis_bytes];
    compile_basis_v3(
        BasisInputV3 {
            result_domain_id: result_domain_digest,
            ..provisional_input
        },
        &mut linked_basis,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let linked_preimage = semantic_basis_preimage_v3(&linked_basis)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let semantic_basis_id = hashv(&[
        SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        linked_preimage.prefix(),
        linked_preimage.suffix(),
    ])
    .to_bytes();
    if semantic_basis_id != semantic_basis {
        return Err(SeriesPremarketExpiryChainErrorV1::Record);
    }
    Ok(ProductRecordCorpusV1 {
        product,
        result_domain,
        portfolio,
        linked_basis,
        product_id: stable_product.to_bytes(),
        result_domain_digest,
        semantic_basis_id,
    })
}

fn product_content(bytes: [u8; 32]) -> Result<ProductContentId, SeriesPremarketExpiryChainErrorV1> {
    ProductContentId::new(bytes).map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)
}

fn build_realm_record_v1(mint: Pubkey) -> Result<Vec<u8>, SeriesPremarketExpiryChainErrorV1> {
    let adapter = PRODUCTION_ADAPTER_RELEASES
        .first()
        .ok_or(SeriesPremarketExpiryChainErrorV1::Record)?;
    RealmV1::new(RealmV1Input {
        token_program: LEGACY_TOKEN_PROGRAM_ID,
        collateral_mint: mint.to_bytes(),
        collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .map(|realm| realm.to_bytes().to_vec())
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)
}

#[derive(Clone)]
struct SeriesRecordCorpusV1 {
    template: Vec<u8>,
    occurrence: Vec<u8>,
    ticket: Vec<u8>,
    template_id: dclutch_core_contract::ContentId,
    occurrence_id: dclutch_core_contract::ContentId,
    ticket_id: dclutch_core_contract::ContentId,
    future_market: Pubkey,
    generation: u64,
    expiry_slot: u64,
}

#[derive(Clone)]
struct FinalizedRecordCorpusV1 {
    child_manifest: DerivedRecordV1,
    product: DerivedRecordV1,
    result_domain: DerivedRecordV1,
    portfolio: DerivedRecordV1,
    linked_basis: DerivedRecordV1,
    realm: DerivedRecordV1,
    template: DerivedRecordV1,
    occurrence: DerivedRecordV1,
    ticket: DerivedRecordV1,
}

impl FinalizedRecordCorpusV1 {
    fn records(&self) -> [&DerivedRecordV1; 9] {
        [
            &self.child_manifest,
            &self.product,
            &self.result_domain,
            &self.portfolio,
            &self.linked_basis,
            &self.realm,
            &self.template,
            &self.occurrence,
            &self.ticket,
        ]
    }
}

#[derive(Clone)]
struct RootIndependentReplayCorpusV1 {
    series_before: Vec<u8>,
    series_after: Vec<u8>,
    ticket_before: Vec<u8>,
    ticket_after: Vec<u8>,
}

#[derive(Clone)]
struct RootIndependentSubstrateV1 {
    finalized: FinalizedRecordCorpusV1,
    install_accounts: Vec<SeriesPremarketExpiryInstallAccountV1>,
    collateral_mint: BuiltAccountV1,
    future_market: BuiltAccountV1,
    permit_account: BuiltAccountV1,
    rent_credit: BuiltAccountV1,
    normal_custody: NormalCustodyCorpusV1,
    replay: RootIndependentReplayCorpusV1,
}

#[derive(Clone)]
struct NormalCustodyCorpusV1 {
    family_request: Vec<u8>,
    prepare_initialize: [u8; CUSTODY_REQUEST_BYTES_V1],
    prepare_open: [u8; CUSTODY_REQUEST_BYTES_V1],
    prepare_lock: [u8; CUSTODY_REQUEST_BYTES_V1],
    expire_refund: [u8; CUSTODY_REQUEST_BYTES_V1],
    expire_close_vault: [u8; CUSTODY_REQUEST_BYTES_V1],
    expire_close_replay: [u8; CUSTODY_REQUEST_BYTES_V1],
    replay: Pubkey,
    escrow_vault: Pubkey,
    refund_destination: Pubkey,
    custody_authority: Pubkey,
    rent_refund: Pubkey,
    rent_refund_lamports: u64,
    install_accounts: Vec<BuiltAccountV1>,
    success_transitions: Vec<SeriesExpectedAccountTransitionV1>,
}

#[derive(Clone)]
struct ProjectedCustodyCorpusV1 {
    prepare_initialize: [u8; dclutch_custody::PROJECTED_CUSTODY_REQUEST_BYTES_V1],
    prepare_open: [u8; dclutch_custody::PROJECTED_CUSTODY_REQUEST_BYTES_V1],
    consume_lock: [u8; dclutch_custody::PROJECTED_CUSTODY_REQUEST_BYTES_V1],
    consume_realize: [u8; dclutch_custody::PROJECTED_CUSTODY_REQUEST_BYTES_V1],
    expire_abort: [u8; dclutch_custody::PROJECTED_CUSTODY_REQUEST_BYTES_V1],
    state: Pubkey,
    hoard_vault: Pubkey,
    caller: Pubkey,
    rent_refund_lamports: u64,
    install_accounts: Vec<BuiltAccountV1>,
    success_transitions: Vec<SeriesExpectedAccountTransitionV1>,
}

#[derive(Clone)]
struct CoreInfrastructureCorpusV1 {
    profile: BuiltAccountV1,
    registry_release: DerivedRecordV1,
    registry_program: BuiltAccountV1,
    registry_programdata: BuiltAccountV1,
    rent_release: DerivedRecordV1,
    rent_program: BuiltAccountV1,
    rent_programdata: BuiltAccountV1,
    install_accounts: Vec<SeriesPremarketExpiryInstallAccountV1>,
}

/// Reproduce the already-landed Core infrastructure state from checked Loader
/// bodies. Registry's real deployment is used directly. The Rent identity is
/// read-only on this route and is never invoked; until the shared waist grows a
/// sixth ELF slot, its checked deployment body is the same real Registry ELF
/// under the distinct Rent program address. The artifact records and V2
/// profile authenticate that exact substrate byte-for-byte.
fn build_core_infrastructure_corpus_v1(
    input: &SeriesPremarketExpiryChainInputV1<'_>,
) -> Result<CoreInfrastructureCorpusV1, SeriesPremarketExpiryChainErrorV1> {
    let substrate = fixture_substrate();
    let registry_artifact = release_v2(
        input.registry_program,
        0x30,
        &input.elves.registry,
        substrate,
    );
    let rent_artifact = release_v2(input.rent_program, 0x35, &input.elves.registry, substrate);
    let artifact_id = |release: ArtifactReleaseV1| {
        ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes())
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)
    };
    let registry_id = artifact_id(registry_artifact)?;
    let rent_id = artifact_id(rent_artifact)?;
    let binding = |release: ArtifactReleaseV1, id: ArtifactReleaseIdV1| {
        ExecutionRoleBindingV1::new(release.program(), id)
    };
    let profile_value = ProtocolInfrastructureProfileV2::new(
        binding(registry_artifact, registry_id),
        binding(rent_artifact, rent_id),
        registry_id,
        rent_id,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let profile_key = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &input.core_program,
    )
    .0;
    let profile = data_account(
        &input.rent,
        profile_key,
        input.core_program,
        profile_value.to_bytes().to_vec(),
    );
    let registry_release = derive_record(
        input.registry_program,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        &registry_artifact.to_bytes(),
    );
    let rent_release = derive_record(
        input.registry_program,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        &rent_artifact.to_bytes(),
    );
    let registry_programdata_key = programdata(input.registry_program);
    let registry_programdata_bytes = programdata_v2(substrate, input.elves.registry.as_slice());
    let registry_program = program_with_view(input.registry_program, registry_programdata_key);
    let registry_programdata = external_with_view(
        registry_programdata_key,
        bpf_loader_upgradeable::ID,
        registry_programdata_bytes,
    );
    let rent_programdata_key = programdata(input.rent_program);
    let rent_programdata_bytes = programdata_v2(substrate, input.elves.registry.as_slice());
    // THE BANK DOES NOT DEPLOY THE RENT PROGRAM; THIS CAMPAIGN INSTALLS IT.
    // `program_with_view` models a program the BANK deploys: its installed
    // `account` is an empty stand-in and only its `observed` view carries the
    // 36-byte Loader-V3 `Program` record. `program_test_without_forced_budget`
    // deploys Registry, Trading, Core, Claims and Custody -- not the Rent
    // program -- so installing that stand-in left the chain holding zero bytes
    // at a coordinate whose rule is `Exact`, and the account projection refused
    // `DataLengthMismatch` for the entire eighty-one-account walk. The bytes
    // installed and the bytes observed are now one record.
    let rent_program = {
        let viewed = program_with_view(input.rent_program, rent_programdata_key);
        let mut installed = data_account(
            &input.rent,
            viewed.key,
            bpf_loader_upgradeable::ID,
            viewed.chain_view().data.clone(),
        );
        installed.account.executable = true;
        installed
    };
    let rent_programdata = data_account(
        &input.rent,
        rent_programdata_key,
        bpf_loader_upgradeable::ID,
        rent_programdata_bytes,
    );
    let install_accounts = vec![
        install_account(profile.clone(), true),
        install_account(
            data_account(
                &input.rent,
                registry_release.raw,
                input.registry_program,
                registry_release.bytes.clone(),
            ),
            true,
        ),
        install_account(vacant(registry_release.staging), true),
        install_account(
            data_account(
                &input.rent,
                rent_release.raw,
                input.registry_program,
                rent_release.bytes.clone(),
            ),
            true,
        ),
        install_account(vacant(rent_release.staging), true),
        install_account(rent_program.clone(), true),
        install_account(rent_programdata.clone(), true),
    ];
    if profile.account.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
        || registry_release.digest != registry_id.to_bytes()
        || rent_release.digest != rent_id.to_bytes()
    {
        return Err(SeriesPremarketExpiryChainErrorV1::Record);
    }
    Ok(CoreInfrastructureCorpusV1 {
        profile,
        registry_release,
        registry_program,
        registry_programdata,
        rent_release,
        rent_program,
        rent_programdata,
        install_accounts,
    })
}

/// Authenticated Product outcome count this campaign's frame is packed at.
///
/// It is the `tail_count` every affine width in the Expire profile is resolved
/// against, and it is the same literal `pack_frame` is called with below.
const EXPIRE_PROFILE_TAIL_COUNT_V1: usize = 3;

/// Name the coordinate whose observed width disagrees with the Expire profile.
///
/// `project_accounts_atomic` refuses `DataLengthMismatch` for the whole walk
/// and Trading maps every projection refusal to one `TradingSbfError::Content`,
/// so on chain this is a 346,000-CU refusal with no coordinate in it. The
/// profile and the packed frame are both in hand here, before a transaction is
/// built, and comparing them costs microseconds -- so the fixture answers the
/// question the wire cannot carry.
fn audit_expire_profile_data_lengths_v1(
    bundle: &BuiltBundleV1,
    selected: &SeriesSelectedActionV5,
) -> Result<(), SeriesPremarketExpiryChainErrorV1> {
    let profile = AccountProfileV3::decode(selected.artifacts.account_profile.as_slice())
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Operator)?
        .base();
    let mut mismatches = 0_usize;
    for coordinate in 0..profile.fixed_account_count() {
        let rule = profile
            .rule(false, coordinate)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Operator)?;
        if rule.prestate() != AccountPrestateV2::Exact {
            continue;
        }
        let observed = match bundle.logical.get(usize::from(coordinate)) {
            Some(account) => account.chain_view().data.len(),
            None => {
                std::eprintln!(
                    "Series Expire profile coordinate {coordinate}: declared Exact and UNBOUND"
                );
                mismatches = mismatches.saturating_add(1);
                continue;
            }
        };
        // `exact_rule_data_length` is `data_length + data_item_stride *
        // tail_count`, and the Product's Portfolio is the coordinate with a
        // nonzero stride: 208 header bytes plus eight per outcome. Comparing
        // the header alone reported it as a mismatch at the exact width it is
        // supposed to have.
        let expected = usize::try_from(rule.data_length())
            .ok()
            .and_then(|header| {
                usize::try_from(rule.data_item_stride())
                    .ok()
                    .and_then(|stride| stride.checked_mul(EXPIRE_PROFILE_TAIL_COUNT_V1))
                    .and_then(|tail| header.checked_add(tail))
            })
            .ok_or(SeriesPremarketExpiryChainErrorV1::Operator)?;
        if observed != expected {
            mismatches = mismatches.saturating_add(1);
            std::eprintln!(
                "Series Expire profile coordinate {coordinate}: declared {expected} bytes, \
                 packed {observed} ({})",
                bundle
                    .logical
                    .get(usize::from(coordinate))
                    .map_or_else(|| "unbound".to_string(), |account| account.key.to_string()),
            );
        }
    }
    if mismatches == 0 {
        Ok(())
    } else {
        Err(SeriesPremarketExpiryChainErrorV1::Physical)
    }
}

fn expire_fixed_data_lengths_v1(
    input: &SeriesPremarketExpiryChainInputV1<'_>,
    substrate: &RootIndependentSubstrateV1,
    projected: &ProjectedCustodyCorpusV1,
    infrastructure: &CoreInfrastructureCorpusV1,
) -> Result<[u32; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize], SeriesPremarketExpiryChainErrorV1>
{
    let mut lengths = [0_u32; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize];
    let mut set = |coordinate: usize, bytes: usize| {
        let value =
            u32::try_from(bytes).map_err(|_| SeriesPremarketExpiryChainErrorV1::Physical)?;
        *lengths
            .get_mut(coordinate)
            .ok_or(SeriesPremarketExpiryChainErrorV1::Physical)? = value;
        Ok::<(), SeriesPremarketExpiryChainErrorV1>(())
    };
    set(8, ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)?;
    set(9, infrastructure.registry_program.chain_view().data.len())?;
    set(10, 36)?;
    set(
        11,
        programdata_v2(fixture_substrate(), input.elves.trading.as_slice()).len(),
    )?;
    set(12, substrate.finalized.realm.bytes.len())?;
    set(14, CUSTODY_REPLAY_BYTES_V1)?;
    set(15, MINT_BYTES)?;
    set(16, ACCOUNT_BYTES)?;
    set(17, ACCOUNT_BYTES)?;
    // See `build_expire_bundle_v1`: the SPL Token program is a Loader-V3
    // program account on this bank, and this width is read off the very
    // constructor that builds it rather than written out again.
    set(
        19,
        program_with_deployed_view(Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID))
            .chain_view()
            .data
            .len(),
    )?;
    set(45, PROJECTED_CUSTODY_STATE_BYTES_V2)?;
    set(51, ACCOUNT_BYTES)?;
    set(57, infrastructure.rent_program.chain_view().data.len())?;
    set(58, infrastructure.profile.account.data.len())?;
    set(59, infrastructure.registry_release.bytes.len())?;
    set(
        62,
        infrastructure.registry_programdata.chain_view().data.len(),
    )?;
    set(63, infrastructure.rent_release.bytes.len())?;
    set(65, infrastructure.rent_programdata.chain_view().data.len())?;
    set(73, substrate.finalized.occurrence.bytes.len())?;
    set(75, substrate.finalized.ticket.bytes.len())?;
    set(77, Clock::size_of())?;
    set(78, rent_sysvar_bytes(&input.rent).len())?;
    // The System builtin's registered name; see `build_expire_bundle_v1`.
    set(79, system_program_builtin().chain_view().data.len())?;
    // Assert the two material mutable widths came from the exact constructed
    // bodies rather than merely agreeing with ABI constants by accident.
    if substrate
        .normal_custody
        .install_accounts
        .iter()
        .find(|account| account.key == substrate.normal_custody.replay)
        .is_none_or(|account| account.account.data.len() != CUSTODY_REPLAY_BYTES_V1)
        || projected
            .install_accounts
            .iter()
            .find(|account| account.key == projected.state)
            .is_none_or(|account| account.account.data.len() != PROJECTED_CUSTODY_STATE_BYTES_V2)
    {
        return Err(SeriesPremarketExpiryChainErrorV1::Physical);
    }
    Ok(lengths)
}

fn account_by_key_v1(
    accounts: &[BuiltAccountV1],
    key: Pubkey,
) -> Result<BuiltAccountV1, SeriesPremarketExpiryChainErrorV1> {
    accounts
        .iter()
        .find(|account| account.key == key)
        .cloned()
        .ok_or(SeriesPremarketExpiryChainErrorV1::Physical)
}

#[allow(clippy::too_many_arguments)]
fn build_expire_bundle_v1(
    input: &SeriesPremarketExpiryChainInputV1<'_>,
    records: &SeriesRecordCorpusV1,
    substrate: &RootIndependentSubstrateV1,
    projected: &ProjectedCustodyCorpusV1,
    infrastructure: &CoreInfrastructureCorpusV1,
    controller: &ControllerCorpusV1,
    release: &SeriesReleaseV5,
    selected: &SeriesSelectedActionV5,
) -> Result<(BuiltBundleV1, BuiltAccountV1), SeriesPremarketExpiryChainErrorV1> {
    let selected_artifacts = release
        .descriptors
        .get(SeriesActionV3::Expire as usize)
        .ok_or(SeriesPremarketExpiryChainErrorV1::Release)?;
    if selected_artifacts.as_slice() != selected.descriptor {
        return Err(SeriesPremarketExpiryChainErrorV1::Release);
    }
    let set = ArtifactSetV1 {
        descriptor: &selected.descriptor,
        account_profile: &selected.artifacts.account_profile,
        request_profile: &selected.artifacts.request_profile,
        transition: &selected.artifacts.transition,
        effect: &selected.artifacts.effect,
        lifecycle: &selected.artifacts.lifecycle,
        strategy: &selected.artifacts.strategy,
        program_set: &release.program_set,
        manifest: &controller.manifest.bytes,
        config: &records.template,
    };
    let waist = WaistFactsV1 {
        registry_program: input.registry_program,
        trading_program: input.trading_program,
        core_program: input.core_program,
        claims_program: input.claims_program,
        custody_program: input.custody_program,
        release_set: input.releases.release_set,
        trading_semantic_release: [0x33; 32],
        activation_cache: input.releases.activation,
    };
    let fixed = FixedCorpusV1 {
        market: controller.market.clone(),
        root: controller.root.clone(),
        product: substrate.finalized.product.clone(),
        result_domain: substrate.finalized.result_domain.clone(),
        portfolio: substrate.finalized.portfolio.clone(),
        linked_basis: substrate.finalized.linked_basis.clone(),
        core_programdata: input.releases.core_programdata,
        trading_programdata: input.releases.trading_programdata,
    };

    let normal = &substrate.normal_custody;
    let trading_programdata = external_with_view(
        input.releases.trading_programdata,
        bpf_loader_upgradeable::ID,
        programdata_v2(fixture_substrate(), input.elves.trading.as_slice()),
    );
    let trading_program =
        program_with_view(input.trading_program, input.releases.trading_programdata);
    let activation = external_with_view(
        input.releases.activation,
        input.registry_program,
        vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1],
    );
    let token_program_key = Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID);
    // THE BANK DEPLOYS SPL Token AS A LOADER-V3 PROGRAM, NOT A BUILTIN.
    // `ProgramTest::default()` with `prefer_bpf(true)` installs it the same way
    // it installs this campaign's own five ELFs, so its account is the 36-byte
    // `Program { programdata }` state at the canonical derived address -- not
    // the empty native-loader account modelled here before. The profile's rule
    // at this coordinate is `Exact`, so an empty stand-in declared a width the
    // chain does not have and the account projection refused
    // `DataLengthMismatch` for the whole eighty-one-account walk. One
    // constructor now owns both the account and the width
    // `expire_fixed_data_lengths_v1` declares for it.
    let token_program = program_with_deployed_view(token_program_key);
    let core_request = SeriesUnallocatedPermitExpiryRequestV1::new(1, 0).encode();
    let core_caller_seeds = CallerAuthoritySeedsV1::from_bytes(
        input.releases.release_set,
        controller.market.key.to_bytes(),
        ExecutionRoleV1::Trading,
        records.ticket_id.to_bytes(),
        hash(&core_request).to_bytes(),
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Identity)?;
    let precommit_caller_key =
        Pubkey::find_program_address(&core_caller_seeds.as_slices(), &input.trading_program).0;
    let mut precommit_caller = vacant(precommit_caller_key);
    precommit_caller.account.lamports = 1;
    let clock = external_with_view(sysvar::clock::ID, sysvar::ID, vec![0_u8; Clock::size_of()]);
    let rent_sysvar =
        external_with_view(sysvar::rent::ID, sysvar::ID, rent_sysvar_bytes(&input.rent));
    // THE BANK'S System BUILTIN IS NOT EMPTY. A native-loader builtin account
    // holds its REGISTERED NAME, twenty-one bytes of `solana_system_program`,
    // and `system_program_builtin` is this tree's one author for that fact --
    // `general-hot`'s `open_batch` asserts it against a live bank. Modelling it
    // empty declared a width the chain does not have at a coordinate whose rule
    // is `Exact`.
    let system = system_program_builtin();
    let bindings = vec![
        (5, controller.ticket_state.clone()),
        (7, substrate.future_market.clone()),
        (8, activation),
        (9, infrastructure.registry_program.clone()),
        (10, trading_program),
        (11, trading_programdata),
        (
            12,
            data_account(
                &input.rent,
                substrate.finalized.realm.raw,
                input.registry_program,
                substrate.finalized.realm.bytes.clone(),
            ),
        ),
        (13, vacant(substrate.finalized.realm.staging)),
        (
            14,
            account_by_key_v1(&normal.install_accounts, normal.replay)?,
        ),
        (15, substrate.collateral_mint.clone()),
        (
            16,
            account_by_key_v1(&normal.install_accounts, normal.escrow_vault)?,
        ),
        (
            17,
            account_by_key_v1(&normal.install_accounts, normal.refund_destination)?,
        ),
        (
            18,
            account_by_key_v1(&normal.install_accounts, normal.custody_authority)?,
        ),
        (19, token_program),
        (33, substrate.rent_credit.clone()),
        (
            45,
            account_by_key_v1(&projected.install_accounts, projected.state)?,
        ),
        (
            51,
            account_by_key_v1(&projected.install_accounts, projected.hoard_vault)?,
        ),
        (55, substrate.permit_account.clone()),
        (57, infrastructure.rent_program.clone()),
        (58, infrastructure.profile.clone()),
        (
            59,
            data_account(
                &input.rent,
                infrastructure.registry_release.raw,
                input.registry_program,
                infrastructure.registry_release.bytes.clone(),
            ),
        ),
        (60, vacant(infrastructure.registry_release.staging)),
        (62, infrastructure.registry_programdata.clone()),
        (
            63,
            data_account(
                &input.rent,
                infrastructure.rent_release.raw,
                input.registry_program,
                infrastructure.rent_release.bytes.clone(),
            ),
        ),
        (64, vacant(infrastructure.rent_release.staging)),
        (65, infrastructure.rent_programdata.clone()),
        (72, vacant(substrate.finalized.template.staging)),
        (
            73,
            data_account(
                &input.rent,
                substrate.finalized.occurrence.raw,
                input.registry_program,
                substrate.finalized.occurrence.bytes.clone(),
            ),
        ),
        (74, vacant(substrate.finalized.occurrence.staging)),
        (
            75,
            data_account(
                &input.rent,
                substrate.finalized.ticket.raw,
                input.registry_program,
                substrate.finalized.ticket.bytes.clone(),
            ),
        ),
        (76, vacant(substrate.finalized.ticket.staging)),
        (77, clock),
        (78, rent_sysvar),
        (79, system),
        (80, precommit_caller.clone()),
        // THE CUSTODY PROGRAM IS A BINDING, NOT A WAIST FACT. The waist names
        // it so the bundle can MINE Custody's two bumps and so the installer
        // leaves the bank's own deployment alone; neither of those puts it in
        // the frame. `hot_v3::resolve_role_carrier_v3` scans the downgraded
        // logical vector for the key the activation cache names for the role,
        // so an Expire Custody route is invocable only if a COORDINATE carries
        // it -- and until `SERIES_EXPIRE_CUSTODY_PROGRAM_COORDINATE_V5` existed
        // there was none to bind. ProgramTest deploys this campaign's five
        // ELFs with `prefer_bpf`, so the bank holds the 36-byte Loader-V3
        // `Program` record at the canonical derived ProgramData address; the
        // same constructor that models the SPL Token program models this one.
        (
            usize::from(SERIES_EXPIRE_CUSTODY_PROGRAM_COORDINATE_V5),
            program_with_deployed_view(input.custody_program),
        ),
    ];
    // The System builtin is the BANK'S, not this campaign's. Installing an
    // account at its address REPLACES the builtin, which is why it was modelled
    // empty here: an empty account passes the installer's Rent gate and a
    // twenty-one-byte one does not. Naming it externally installed is the
    // honest form -- the campaign states what the bank holds and installs
    // nothing over it.
    let external = [token_program_key, sysvar::clock::ID, system_program::ID];
    let scenario = ScenarioV1 {
        family_request: &normal.family_request,
        tail_count: 3,
        clock_slot: 2,
        generation: controller.generation,
        ed25519_evidence: None,
        native_message_instruction_index: 1,
        externally_installed_extra: &external,
        payer: normal.rent_refund,
    };
    let bundle = build_bundle(&BundleInputV1 {
        set,
        waist,
        scenario,
        fixed,
        bindings: &bindings,
        rent: &input.rent,
    })
    .map_err(|error| {
        std::eprintln!("Series Expire bundle refused: {error:?}");
        SeriesPremarketExpiryChainErrorV1::Physical
    })?;
    Ok((bundle, precommit_caller))
}

fn observed_account_v1(
    observation: Observation,
    key: Pubkey,
    account: &Account,
) -> ObservedAccount {
    ObservedAccount {
        observation,
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data.clone(),
    }
}

fn fixed_operator_accounts_v1(
    input: &SeriesPremarketExpiryChainInputV1<'_>,
    bundle: &BuiltBundleV1,
    observation: Observation,
) -> Result<Vec<ObservedAccountMetaV3>, SeriesPremarketExpiryChainErrorV1> {
    let fixed_metas = bundle
        .hot_instruction
        .accounts
        .get(..HOT_FIXED_ACCOUNT_COUNT_V3)
        .ok_or(SeriesPremarketExpiryChainErrorV1::Operator)?;
    fixed_metas
        .iter()
        .enumerate()
        .map(|(index, meta)| {
            let installed = bundle
                .accounts
                .iter()
                .find(|candidate| candidate.key == meta.pubkey)
                .ok_or(SeriesPremarketExpiryChainErrorV1::Operator)?;
            let mut account = installed.account.clone();
            if index == HOT_RENT_SYSVAR_ACCOUNT_V3 {
                account = Account {
                    lamports: 1,
                    data: rent_sysvar_bytes(&input.rent),
                    owner: sysvar::ID,
                    executable: false,
                    rent_epoch: 0,
                };
            } else if meta.pubkey == input.releases.trading_programdata {
                account = Account {
                    lamports: 1,
                    data: programdata_v2(fixture_substrate(), input.elves.trading.as_slice()),
                    owner: bpf_loader_upgradeable::ID,
                    executable: false,
                    rent_epoch: 0,
                };
            } else if meta.pubkey == input.releases.activation {
                // The bank holds the REAL activation cache; the bundle
                // builder's copy of this coordinate is a zero-length
                // stand-in. An operator handed the stand-in reads no Custody
                // deployment out of it, mines an ABSENT Custody
                // transfer-authority bump, and disagrees with the bundle in
                // exactly one byte of a 256-byte instruction. Both envelopes
                // are valid -- an absent hint means the route searches -- so
                // nothing refuses and only this exact cross-check sees it.
                account = Account {
                    lamports: 1,
                    data: input.releases.activation_data.to_vec(),
                    owner: REGISTRY_PROGRAM_ID,
                    executable: false,
                    rent_epoch: 0,
                };
            } else if meta.pubkey == input.releases.core_programdata {
                account = Account {
                    lamports: 1,
                    data: programdata_v2(fixture_substrate(), input.elves.core.as_slice()),
                    owner: bpf_loader_upgradeable::ID,
                    executable: false,
                    rent_epoch: 0,
                };
            }
            Ok(ObservedAccountMetaV3 {
                account: observed_account_v1(observation, meta.pubkey, &account),
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn build_operator_report_v1(
    input: &SeriesPremarketExpiryChainInputV1<'_>,
    product: &ProductRecordCorpusV1,
    records: &SeriesRecordCorpusV1,
    substrate: &RootIndependentSubstrateV1,
    projected: &ProjectedCustodyCorpusV1,
    controller: &ControllerCorpusV1,
    bundle: &BuiltBundleV1,
    expire_lengths: &[u32; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize],
) -> Result<(SeriesSelectedHotReportV5, Vec<Pubkey>), SeriesPremarketExpiryChainErrorV1> {
    let observation = Observation {
        slot: 2,
        unix_timestamp: 0,
        finality: Finality::Finalized,
    };
    let profile = AccountProfileV3::decode(bundle.artifacts.account_profile.bytes.as_slice())
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Operator)?
        .base();
    let packed = pack_frame(profile, 3, &bundle.span_counts, &bundle.logical)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Operator)?;
    let runtime_physical_accounts = packed
        .iter()
        .map(|packed| ObservedAccountMetaV3 {
            account: observed_account_v1(observation, packed.built.key, packed.built.chain_view()),
            is_signer: packed.meta.is_signer,
            is_writable: packed.meta.is_writable,
        })
        .collect::<Vec<_>>();
    let runtime_keys = runtime_physical_accounts
        .iter()
        .map(|meta| meta.account.key)
        .collect::<Vec<_>>();
    let permit = runtime_physical_accounts
        .iter()
        .find(|meta| meta.account.key == substrate.permit_account.key)
        .map(|meta| meta.account.clone())
        .ok_or(SeriesPremarketExpiryChainErrorV1::Operator)?;
    let fixed_accounts = fixed_operator_accounts_v1(input, bundle, observation)?;
    let series = SeriesStateV3::decode(&substrate.replay.series_before, 1)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Replay)?;
    let ticket_state = TicketStateV3::decode(&substrate.replay.ticket_before)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Replay)?;
    let lifecycle = SeriesLifecycleSnapshotV3 {
        template_bytes: &records.template,
        series,
        now_slot: 2,
        current: Some(SeriesCurrentOccurrenceV3 {
            occurrence_bytes: &records.occurrence,
            ticket_bytes: &records.ticket,
            siblings: &[],
            ticket_state: Some(ticket_state),
        }),
        terminal_ticket: None,
        observed_root_lamports: controller.root.account.lamports,
        exact_root_rent: controller.root.account.lamports,
        rent_sink: None,
    };
    let state = SeriesCurrentHotStateV5 {
        fixed_accounts,
        strategy_accounts: Vec::new(),
        runtime_physical_accounts,
        lifecycle,
        permit: Some(permit),
    };
    let report = with_current_release_input_v1(
        input,
        product,
        records,
        substrate,
        projected,
        controller.root.key,
        expire_lengths,
        // The plan the operator returns is the diagnostic. Collapsing every
        // non-Ready answer into one `Operator` made this refusal a search over
        // the whole inspector: `Acquire` names a missing account, `WaitUntil` a
        // deadline the fixture has not warped past, and the error arm carries
        // the inspector's own conjunct. The sibling at :1616 already prints its
        // builder error; this one printed nothing.
        |current| match inspect_current_series_hot_v5(&state, current) {
            Ok(SeriesCurrentHotPlanV5::Ready(report)) => Ok(report),
            Ok(other) => {
                std::eprintln!("Series Expire operator is not Ready: {other:?}");
                Err(SeriesPremarketExpiryChainErrorV1::Operator)
            }
            Err(error) => {
                std::eprintln!("Series Expire operator refused: {error:?}");
                Err(SeriesPremarketExpiryChainErrorV1::Operator)
            }
        },
    )?;
    if report.instruction != bundle.hot_instruction {
        // Name the first differing byte. This refusal used to be one word for
        // a 256-byte instruction, and the byte that differed was the eighth
        // bump hint -- a value both sides may legally omit, which is why
        // nothing else in the fixture could see it.
        let left = &report.instruction.data;
        let right = &bundle.hot_instruction.data;
        let first = left.iter().zip(right.iter()).position(|(a, b)| a != b);
        std::eprintln!(
            "Series Expire operator instruction differs from the bundle: \
             report_len={} bundle_len={} first_differing_byte={first:?}",
            left.len(),
            right.len(),
        );
        return Err(SeriesPremarketExpiryChainErrorV1::Operator);
    }
    Ok((report, runtime_keys))
}

fn merge_install_account_v1(
    output: &mut Vec<SeriesPremarketExpiryInstallAccountV1>,
    candidate: SeriesPremarketExpiryInstallAccountV1,
) -> Result<(), SeriesPremarketExpiryChainErrorV1> {
    if let Some(existing) = output.iter_mut().find(|value| value.key == candidate.key) {
        if existing.account != candidate.account {
            return Err(SeriesPremarketExpiryChainErrorV1::Physical);
        }
        existing.snapshot_for_rollback |= candidate.snapshot_for_rollback;
    } else {
        output.push(candidate);
    }
    Ok(())
}

fn build_success_transitions_v1(
    controller: &ControllerCorpusV1,
    substrate: &RootIndependentSubstrateV1,
    projected: &ProjectedCustodyCorpusV1,
    precommit_caller: &BuiltAccountV1,
) -> Result<Vec<SeriesExpectedAccountTransitionV1>, SeriesPremarketExpiryChainErrorV1> {
    let mut root_after = controller.root.account.clone();
    root_after.data.truncate(CAPABILITY_ROOT_HEADER_BYTES_V1);
    root_after
        .data
        .extend_from_slice(&substrate.replay.series_after);
    let mut ticket_after = controller.ticket_state.account.clone();
    ticket_after.data = substrate.replay.ticket_after.clone();
    let mut rent_credit_after = substrate.rent_credit.account.clone();
    rent_credit_after.lamports = rent_credit_after
        .lamports
        .checked_add(substrate.normal_custody.rent_refund_lamports)
        .and_then(|value| value.checked_add(projected.rent_refund_lamports))
        .and_then(|value| value.checked_add(substrate.permit_account.account.lamports))
        .ok_or(SeriesPremarketExpiryChainErrorV1::Physical)?;
    let mut transitions = vec![
        SeriesExpectedAccountTransitionV1 {
            key: controller.root.key,
            before: Some(controller.root.account.clone()),
            after: Some(root_after),
        },
        SeriesExpectedAccountTransitionV1 {
            key: controller.ticket_state.key,
            before: Some(controller.ticket_state.account.clone()),
            after: Some(ticket_after),
        },
    ];
    transitions.extend(substrate.normal_custody.success_transitions.clone());
    transitions.extend(projected.success_transitions.clone());
    transitions.extend([
        SeriesExpectedAccountTransitionV1 {
            key: substrate.permit_account.key,
            before: Some(substrate.permit_account.account.clone()),
            after: None,
        },
        SeriesExpectedAccountTransitionV1 {
            key: substrate.rent_credit.key,
            before: Some(substrate.rent_credit.account.clone()),
            after: Some(rent_credit_after),
        },
        SeriesExpectedAccountTransitionV1 {
            key: substrate.future_market.key,
            before: None,
            after: None,
        },
        SeriesExpectedAccountTransitionV1 {
            key: precommit_caller.key,
            before: Some(precommit_caller.account.clone()),
            after: Some(precommit_caller.account.clone()),
        },
    ]);
    let mut keys = Vec::with_capacity(transitions.len());
    if transitions.iter().any(|transition| {
        !keys.contains(&transition.key) && {
            keys.push(transition.key);
            false
        }
    }) {
        return Err(SeriesPremarketExpiryChainErrorV1::Physical);
    }
    if keys.len() != transitions.len() {
        return Err(SeriesPremarketExpiryChainErrorV1::Physical);
    }
    Ok(transitions)
}

#[allow(clippy::too_many_arguments)]
fn build_install_accounts_v1(
    input: &SeriesPremarketExpiryChainInputV1<'_>,
    substrate: &RootIndependentSubstrateV1,
    projected: &ProjectedCustodyCorpusV1,
    infrastructure: &CoreInfrastructureCorpusV1,
    controller: &ControllerCorpusV1,
    bundle: &BuiltBundleV1,
) -> Result<
    (Vec<SeriesPremarketExpiryInstallAccountV1>, Vec<Pubkey>),
    SeriesPremarketExpiryChainErrorV1,
> {
    let mut output = Vec::new();
    for candidate in &bundle.accounts {
        merge_install_account_v1(
            &mut output,
            SeriesPremarketExpiryInstallAccountV1 {
                key: candidate.key,
                account: candidate.account.clone(),
                snapshot_for_rollback: candidate.snapshot_for_rollback,
            },
        )?;
    }
    for source in [
        substrate.install_accounts.as_slice(),
        infrastructure.install_accounts.as_slice(),
    ] {
        for candidate in source {
            merge_install_account_v1(&mut output, candidate.clone())?;
        }
    }
    for candidate in &projected.install_accounts {
        merge_install_account_v1(&mut output, install_account(candidate.clone(), true))?;
    }
    for candidate in [
        controller.market.clone(),
        controller.root.clone(),
        controller.ticket_state.clone(),
        data_account(
            &input.rent,
            controller.manifest.raw,
            input.registry_program,
            controller.manifest.bytes.clone(),
        ),
        vacant(controller.manifest.staging),
    ] {
        merge_install_account_v1(&mut output, install_account(candidate, true))?;
    }
    let mut external = bundle.externally_installed_keys.clone();
    for key in [programdata(input.registry_program), sysvar::clock::ID] {
        if !external.contains(&key) {
            external.push(key);
        }
    }
    require_disjoint_install_accounts_v1(&output)?;
    Ok((output, external))
}

/// Reconstruct the exact `HoardOpen` projected-Custody prestate left by
/// Prepare. The production Series projector owns every request field; this
/// fixture only supplies authenticated program identities, PDA addresses and
/// Rent observations. The projection receipt is deterministically rebuilt
/// from the same future-Market facts that Custody persists.
#[allow(clippy::too_many_arguments)]
fn build_projected_custody_corpus_v1(
    rent: &Rent,
    registry_program: Pubkey,
    trading_program: Pubkey,
    core_program: Pubkey,
    custody_program: Pubkey,
    rent_program: Pubkey,
    release_set: [u8; 32],
    parent_root: Pubkey,
    product: &ProductRecordCorpusV1,
    realm_bytes: &[u8],
    series: &SeriesRecordCorpusV1,
    normal: &NormalCustodyCorpusV1,
    rent_credit: Pubkey,
) -> Result<ProjectedCustodyCorpusV1, SeriesPremarketExpiryChainErrorV1> {
    let admitted = admit_occurrence(&series.template, &series.occurrence, &[])
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let admitted_ticket =
        admit_ticket(&series.ticket).map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let projection = AuthenticatedProductProjectionV2::new(
        dclutch_core_contract::ContentId::new(hash(&product.product).to_bytes())
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
        dclutch_core_contract::ContentId::new(product.product_id)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
        dclutch_core_contract::ContentId::new(product.result_domain_digest)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
    );
    let escrow = pre_founding_series_escrow(
        admitted,
        admitted_ticket,
        projection,
        dclutch_trading_sbf::series::AccountKeyV3::new(registry_program.to_bytes())
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Identity)?,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let realm =
        RealmV1::decode(realm_bytes).map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let future = escrow.future_market().identity();
    let found = CoreRequest::administrative(
        CoreAction::Found,
        series.generation,
        identity(series.future_market.to_bytes())?,
    );
    let found_bytes = found
        .encode()
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let receipt = ProjectFoundReceiptV2::new(
        identity(series.future_market.to_bytes())?,
        series.generation,
        identity(future.realm_id.to_bytes())?,
        identity(*realm.collateral_mint())?,
        identity(*realm.token_program())?,
        identity(*realm.collateral_adapter_release_id())?,
        identity(future.product_record.to_bytes())?,
        identity(future.product_id.to_bytes())?,
        identity(future.resolution_policy.to_bytes())?,
        identity(release_set)?,
        identity(rent_program.to_bytes())?,
        u64::MAX,
        hash(&found_bytes).to_bytes(),
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let receipt_bytes = receipt
        .encode()
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let projection_receipt_digest = hash(&receipt_bytes).to_bytes();
    let context_digest = hashv(&[
        PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
        &series.ticket_id.to_bytes(),
    ])
    .to_bytes();
    let hoard_vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            series.future_market.to_bytes(),
            release_set,
            context_digest,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &custody_program,
    )
    .0;
    let payer = derived_fixture_key_v1(
        b"dclutch/test/series-expire/projected-payer",
        series.future_market,
        series.ticket_id.to_bytes(),
    );
    let state_rent_lamports = rent.minimum_balance(PROJECTED_CUSTODY_STATE_BYTES_V2);
    let vault_rent_lamports = rent.minimum_balance(ACCOUNT_BYTES);
    let physical = SeriesProjectedCustodyPhysicalV3 {
        caller_program: trading_program.to_bytes(),
        core_program: core_program.to_bytes(),
        rent_program: rent_program.to_bytes(),
        parent_capability_root: parent_root.to_bytes(),
        projection_receipt_digest,
        payer: payer.to_bytes(),
        rent_credit: rent_credit.to_bytes(),
        hoard_vault: hoard_vault.to_bytes(),
        escrow_vault: normal.escrow_vault.to_bytes(),
        mint: *realm.collateral_mint(),
        token_program: *realm.token_program(),
        collateral_release: *realm.collateral_adapter_release_id(),
        projected_state_rent_lamports: state_rent_lamports,
        hoard_vault_rent_lamports: vault_rent_lamports,
        escrow_replay_rent_lamports: rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1),
        escrow_vault_rent_lamports: rent.minimum_balance(ACCOUNT_BYTES),
    };
    let consume = project_consume_v3(
        consume_series_escrow_v3(escrow),
        series.expiry_slot,
        physical,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let abort = project_abort_v3(escrow, series.expiry_slot, physical)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let prepare_initialize = ProjectedCustodyRequestV1 {
        operation: ProjectedCustodyOperationV1::Initialize,
        expected_revision: 0,
        resulting_revision: 1,
        amount: 0,
        ..abort
    };
    let prepare_open = ProjectedCustodyRequestV1 {
        operation: ProjectedCustodyOperationV1::OpenHoard,
        expected_revision: 1,
        resulting_revision: 2,
        amount: 0,
        ..abort
    };
    let prepare_initialize = prepare_initialize
        .encode()
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let prepare_open = prepare_open
        .encode()
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let consume_lock = consume
        .lock_and_close_source
        .encode()
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let consume_realize = consume
        .realize_and_close
        .encode()
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let expire_abort = abort
        .encode()
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;

    let state_seeds = ProjectedCustodyStateSeedsV2::from_request(abort);
    let (state, bump) = Pubkey::find_program_address(&state_seeds.as_slices(), &custody_program);
    let projected_state = ProjectedCustodyStateV2 {
        phase: ProjectedCustodyPhaseV1::HoardOpen,
        request: ProjectedCustodyRequestV1::decode(&prepare_open)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Replay)?,
        next_revision: 2,
        locked_amount: 0,
        principal_cap_sets: u64::MAX,
        last_request_digest: hash(&prepare_open).to_bytes(),
        bump,
    };
    let state_account = data_account(
        rent,
        state,
        custody_program,
        projected_state
            .encode()
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Replay)?
            .to_vec(),
    );
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(series.future_market.to_bytes(), release_set).as_slices(),
        &custody_program,
    )
    .0;
    let hoard_bytes = TokenAccount::initialized_base_bytes(
        realm.collateral_mint().to_owned(),
        custody_authority.to_bytes(),
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let hoard_account = data_account(
        rent,
        hoard_vault,
        Pubkey::new_from_array(*realm.token_program()),
        hoard_bytes.to_vec(),
    );
    let caller_seeds = ProjectedCustodyCallerSeedsV1::new(abort, hash(&expire_abort).to_bytes());
    let caller = Pubkey::find_program_address(&caller_seeds.as_slices(), &trading_program).0;
    let install_accounts = vec![state_account.clone(), hoard_account.clone(), vacant(caller)];
    let success_transitions = vec![
        SeriesExpectedAccountTransitionV1 {
            key: state,
            before: Some(state_account.account.clone()),
            after: None,
        },
        SeriesExpectedAccountTransitionV1 {
            key: hoard_vault,
            before: Some(hoard_account.account.clone()),
            after: None,
        },
    ];
    Ok(ProjectedCustodyCorpusV1 {
        prepare_initialize,
        prepare_open,
        consume_lock,
        consume_realize,
        expire_abort,
        state,
        hoard_vault,
        caller,
        rent_refund_lamports: state_rent_lamports
            .checked_add(vault_rent_lamports)
            .ok_or(SeriesPremarketExpiryChainErrorV1::Physical)?,
        install_accounts,
        success_transitions,
    })
}

/// Materialize every immutable record and paired vacant staging cursor that
/// can be known before the parent-root repair. The occurrence-derived future
/// Market is a distinct System vacancy; the live controller Market is not a
/// field of this substrate and cannot accidentally alias it here.
fn build_root_independent_substrate_v1(
    rent: &Rent,
    registry_program: Pubkey,
    trading_program: Pubkey,
    custody_program: Pubkey,
    core_program: Pubkey,
    rent_program: Pubkey,
    release_set: [u8; 32],
    refund_owner: Pubkey,
    product: &ProductRecordCorpusV1,
    realm: &[u8],
    series: &SeriesRecordCorpusV1,
) -> Result<RootIndependentSubstrateV1, SeriesPremarketExpiryChainErrorV1> {
    let finalized = FinalizedRecordCorpusV1 {
        child_manifest: derive_record(
            registry_program,
            CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            &EMPTY_MANIFEST_BYTES,
        ),
        product: derive_record(
            registry_program,
            PRODUCT_RECORD_SCHEMA_ID_V2,
            &product.product,
        ),
        result_domain: derive_record(
            registry_program,
            RESULT_DOMAIN_SCHEMA_ID_V2,
            &product.result_domain,
        ),
        portfolio: derive_record(registry_program, PORTFOLIO_SCHEMA_ID_V2, &product.portfolio),
        linked_basis: derive_record(
            registry_program,
            GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            &product.linked_basis,
        ),
        realm: derive_record(registry_program, REALM_SCHEMA_RELEASE_ID_V1, realm),
        template: derive_record(
            registry_program,
            hash(b"dclutch/schema/series-template-v3").to_bytes(),
            &series.template,
        ),
        occurrence: derive_record(
            registry_program,
            hash(b"dclutch/schema/series-occurrence-v3").to_bytes(),
            &series.occurrence,
        ),
        ticket: derive_record(
            registry_program,
            hash(b"dclutch/schema/series-ticket-v3").to_bytes(),
            &series.ticket,
        ),
    };
    let admitted = admit_occurrence(&series.template, &series.occurrence, &[])
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    if admitted.occurrence().capability_manifest().to_bytes() != finalized.child_manifest.digest
        || finalized.child_manifest.digest != hash(&EMPTY_MANIFEST_BYTES).to_bytes()
        || finalized.product.digest != hash(&product.product).to_bytes()
        || finalized.result_domain.digest != product.result_domain_digest
        || finalized.realm.digest != hash(realm).to_bytes()
        || finalized.template.digest != hash(&series.template).to_bytes()
        || finalized.occurrence.digest != hash(&series.occurrence).to_bytes()
        || finalized.ticket.digest != hash(&series.ticket).to_bytes()
    {
        return Err(SeriesPremarketExpiryChainErrorV1::Record);
    }

    let mut install_accounts = Vec::with_capacity(finalized.records().len() * 2 + 4);
    for record in finalized.records() {
        install_accounts.push(install_account(
            data_account(rent, record.raw, registry_program, record.bytes.clone()),
            true,
        ));
        install_accounts.push(install_account(vacant(record.staging), true));
    }
    let realm = RealmV1::decode(realm).map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    if realm.token_program() != &LEGACY_TOKEN_PROGRAM_ID {
        return Err(SeriesPremarketExpiryChainErrorV1::Record);
    }
    let collateral_mint_key = Pubkey::new_from_array(*realm.collateral_mint());
    let collateral_mint = data_account(
        rent,
        collateral_mint_key,
        Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID),
        legacy_mint_bytes_v1(9)?.to_vec(),
    );
    install_accounts.push(install_account(collateral_mint.clone(), true));

    let future_market = vacant(series.future_market);
    install_accounts.push(install_account(future_market.clone(), true));
    let (rent_credit_key, rent_credit_state) = build_lifecycle_rent_credit_v1(
        rent_program,
        series.future_market,
        release_set,
        series.generation,
        refund_owner,
    )?;
    let rent_credit = data_account(
        rent,
        rent_credit_key,
        rent_program,
        rent_credit_state.to_bytes().to_vec(),
    );
    install_accounts.push(install_account(rent_credit.clone(), true));

    let permit_seeds = SeriesFoundingPermitSeedsV1::new(
        identity(release_set)?,
        identity(series.future_market.to_bytes())?,
        identity(series.ticket_id.to_bytes())?,
    );
    let permit_key = Pubkey::find_program_address(&permit_seeds.as_slices(), &core_program).0;
    let mut permit_account = vacant(permit_key);
    permit_account.account.lamports = rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1);
    install_accounts.push(install_account(permit_account.clone(), true));

    let normal_custody = build_normal_custody_corpus_v1(
        rent,
        registry_program,
        trading_program,
        custody_program,
        release_set,
        product,
        series,
        collateral_mint_key,
    )?;
    for account in &normal_custody.install_accounts {
        install_accounts.push(install_account(account.clone(), true));
    }
    require_disjoint_install_accounts_v1(&install_accounts)?;

    let close_rent = rent.minimum_balance(32);
    let series_before_state = SeriesStateV3::new(close_rent)
        .prepare_ticket(0)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Replay)?;
    let series_before = series_before_state
        .encode(1)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Replay)?;
    let series_after = series_before_state
        .settle_current(1, 1)
        .and_then(|state| state.encode(1))
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Replay)?;
    let ticket_before_state = TicketStateV3::prepared(series.ticket_id);
    let ticket_before = ticket_before_state.encode();
    let ticket_after = ticket_before_state
        .settle(0, TicketPhaseV3::Expired)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Replay)?
        .encode();
    let replay = RootIndependentReplayCorpusV1 {
        series_before: series_before.to_vec(),
        series_after: series_after.to_vec(),
        ticket_before: ticket_before.to_vec(),
        ticket_after: ticket_after.to_vec(),
    };
    if replay.series_before == replay.series_after
        || replay.ticket_before == replay.ticket_after
        || TicketStateV3::decode(&replay.ticket_after)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Replay)?
            .phase()
            != TicketPhaseV3::Expired
    {
        return Err(SeriesPremarketExpiryChainErrorV1::Replay);
    }

    Ok(RootIndependentSubstrateV1 {
        finalized,
        install_accounts,
        collateral_mint,
        future_market,
        permit_account,
        rent_credit,
        normal_custody,
        replay,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_normal_custody_corpus_v1(
    rent: &Rent,
    registry_program: Pubkey,
    trading_program: Pubkey,
    custody_program: Pubkey,
    release_set: [u8; 32],
    product: &ProductRecordCorpusV1,
    series: &SeriesRecordCorpusV1,
    collateral_mint: Pubkey,
) -> Result<NormalCustodyCorpusV1, SeriesPremarketExpiryChainErrorV1> {
    let family_request = encode_series_action_header_v3(
        SeriesActionV3::Expire,
        series.template_id,
        Some(series.occurrence_id),
        Some(series.ticket_id),
        1,
        0,
        0,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?
    .to_vec();
    let admitted = admit_occurrence(&series.template, &series.occurrence, &[])
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let admitted_ticket =
        admit_ticket(&series.ticket).map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let rent_refund = Pubkey::new_from_array(admitted_ticket.ticket().refund_owner().to_bytes());
    let projection = AuthenticatedProductProjectionV2::new(
        dclutch_core_contract::ContentId::new(hash(&product.product).to_bytes())
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
        dclutch_core_contract::ContentId::new(product.product_id)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
        dclutch_core_contract::ContentId::new(product.result_domain_digest)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
    );
    let escrow = pre_founding_series_escrow(
        admitted,
        admitted_ticket,
        projection,
        dclutch_trading_sbf::series::AccountKeyV3::new(registry_program.to_bytes())
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Identity)?,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    if escrow.market().to_bytes() != series.future_market.to_bytes()
        || escrow.ticket_id() != series.ticket_id
        || escrow.hoard_principal() != 9
    {
        return Err(SeriesPremarketExpiryChainErrorV1::ChildRequest);
    }

    let context = series.ticket_id.to_bytes();
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            series.future_market.to_bytes(),
            release_set,
            CallerRoleV1::Trading,
            context,
        )
        .as_slices(),
        &custody_program,
    )
    .0;
    let escrow_vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            series.future_market.to_bytes(),
            release_set,
            context,
            CompartmentV1::SeriesEscrow,
        )
        .as_slices(),
        &custody_program,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(series.future_market.to_bytes(), release_set).as_slices(),
        &custody_program,
    )
    .0;
    let founder_source = derived_fixture_key_v1(
        b"dclutch/test/series-expire/founder-source",
        series.future_market,
        series.ticket_id.to_bytes(),
    );
    let refund_destination = derived_fixture_key_v1(
        b"dclutch/test/series-expire/refund-destination",
        series.future_market,
        rent_refund.to_bytes(),
    );
    let payer = derived_fixture_key_v1(
        b"dclutch/test/series-expire/prepare-payer",
        series.future_market,
        series.template_id.to_bytes(),
    );
    let replay_rent_lamports = rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1);
    let vault_rent_lamports = rent.minimum_balance(ACCOUNT_BYTES);
    let physical = SeriesCustodyPhysicalV3 {
        caller_program: trading_program.to_bytes(),
        parent_request_digest: hash(&family_request).to_bytes(),
        payer: payer.to_bytes(),
        mint: collateral_mint.to_bytes(),
        token_program: LEGACY_TOKEN_PROGRAM_ID,
        founder_source: founder_source.to_bytes(),
        escrow_vault: escrow_vault.to_bytes(),
        hoard_vault: [0; 32],
        refund_destination: refund_destination.to_bytes(),
        replay_rent_lamports,
        vault_rent_lamports,
    };
    let prepare = project_prepare_custody_v3(prepare_series_escrow_v3(escrow), physical)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let expire = project_terminal_custody_v3(expire_series_escrow_v3(escrow), physical)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let prepare_initialize = prepare[0]
        .to_bytes()
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let prepare_open = prepare[1]
        .to_bytes()
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let prepare_lock = prepare[2]
        .to_bytes()
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let expire_refund = expire[0]
        .to_bytes()
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let expire_close_vault = expire[1]
        .to_bytes()
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;
    let expire_close_replay = expire[2]
        .to_bytes()
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::ChildRequest)?;

    let initialize_digest = hash(&prepare_initialize).to_bytes();
    let initialize_poststate = custody_poststate_commitment_v1(
        initialize_digest,
        replay,
        replay,
        0,
        0,
        0,
        0,
        replay_rent_lamports,
    );
    let replay_state =
        CustodyReplayV1::initialize(prepare[0], initialize_digest, initialize_poststate)
            .and_then(|state| {
                let digest = hash(&prepare_open).to_bytes();
                let poststate = custody_poststate_commitment_v1(
                    digest,
                    escrow_vault,
                    escrow_vault,
                    0,
                    0,
                    0,
                    0,
                    vault_rent_lamports,
                );
                state.advance(prepare[1], digest, poststate)
            })
            .and_then(|state| {
                let digest = hash(&prepare_lock).to_bytes();
                let poststate = custody_poststate_commitment_v1(
                    digest,
                    founder_source,
                    escrow_vault,
                    9,
                    0,
                    0,
                    9,
                    0,
                );
                state.advance(prepare[2], digest, poststate)
            })
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Replay)?;
    if replay_state.next_revision != 3 || replay_state.open_vault_count != 1 {
        return Err(SeriesPremarketExpiryChainErrorV1::Replay);
    }

    let initialized_vault = TokenAccount::initialized_base_bytes(
        collateral_mint.to_bytes(),
        custody_authority.to_bytes(),
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let escrow_vault_bytes = TokenAccount::project_amount_poststate(&initialized_vault, 9)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let refund_destination_bytes =
        TokenAccount::initialized_base_bytes(collateral_mint.to_bytes(), rent_refund.to_bytes())
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let replay_account = data_account(
        rent,
        replay,
        custody_program,
        replay_state
            .to_bytes()
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Replay)?
            .to_vec(),
    );
    let escrow_vault_account = data_account(
        rent,
        escrow_vault,
        Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID),
        escrow_vault_bytes.to_vec(),
    );
    let refund_destination_account = data_account(
        rent,
        refund_destination,
        Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID),
        refund_destination_bytes.to_vec(),
    );
    let refund_destination_after = Account {
        data: TokenAccount::project_amount_poststate(&refund_destination_bytes, 9)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?
            .to_vec(),
        ..refund_destination_account.account.clone()
    };
    let rent_refund_lamports = replay_rent_lamports
        .checked_add(vault_rent_lamports)
        .ok_or(SeriesPremarketExpiryChainErrorV1::Physical)?;
    let success_transitions = vec![
        SeriesExpectedAccountTransitionV1 {
            key: refund_destination,
            before: Some(refund_destination_account.account.clone()),
            after: Some(refund_destination_after),
        },
        SeriesExpectedAccountTransitionV1 {
            key: escrow_vault,
            before: Some(escrow_vault_account.account.clone()),
            after: None,
        },
        SeriesExpectedAccountTransitionV1 {
            key: replay,
            before: Some(replay_account.account.clone()),
            after: None,
        },
    ];
    let install_accounts = vec![
        replay_account,
        escrow_vault_account,
        refund_destination_account,
        vacant(custody_authority),
    ];
    Ok(NormalCustodyCorpusV1 {
        family_request,
        prepare_initialize,
        prepare_open,
        prepare_lock,
        expire_refund,
        expire_close_vault,
        expire_close_replay,
        replay,
        escrow_vault,
        refund_destination,
        custody_authority,
        rent_refund,
        rent_refund_lamports,
        install_accounts,
        success_transitions,
    })
}

#[allow(clippy::too_many_arguments)]
fn custody_poststate_commitment_v1(
    request_digest: [u8; 32],
    source: Pubkey,
    destination: Pubkey,
    source_before: u64,
    source_after: u64,
    destination_before: u64,
    destination_after: u64,
    rent_lamports: u64,
) -> [u8; 32] {
    hashv(&[
        CUSTODY_POSTSTATE_DOMAIN_V1,
        &request_digest,
        source.as_ref(),
        destination.as_ref(),
        &source_before.to_le_bytes(),
        &source_after.to_le_bytes(),
        &destination_before.to_le_bytes(),
        &destination_after.to_le_bytes(),
        &rent_lamports.to_le_bytes(),
    ])
    .to_bytes()
}

fn derived_fixture_key_v1(domain: &[u8], market: Pubkey, context: [u8; 32]) -> Pubkey {
    Pubkey::new_from_array(hashv(&[domain, market.as_ref(), &context]).to_bytes())
}

/// Exact legacy SPL Mint base state admitted by the production token profile.
/// The Realm requires both authorities absent, so their COption tags and bodies
/// remain canonical zero. Supply is the one occurrence's locked collateral.
fn legacy_mint_bytes_v1(
    supply: u64,
) -> Result<[u8; MINT_BYTES], SeriesPremarketExpiryChainErrorV1> {
    let mut bytes = [0_u8; MINT_BYTES];
    put(&mut bytes, 36, &supply.to_le_bytes())?;
    put(&mut bytes, 44, &[0])?;
    put(&mut bytes, 45, &[1])?;
    let parsed = Mint::parse(&bytes).map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    if parsed.supply != supply
        || parsed.decimals != 0
        || !parsed.is_initialized
        || !matches!(parsed.mint_authority, dclutch_custody::token_svm::COption::None)
        || !matches!(parsed.freeze_authority, dclutch_custody::token_svm::COption::None)
    {
        return Err(SeriesPremarketExpiryChainErrorV1::Record);
    }
    Ok(bytes)
}

fn build_lifecycle_rent_credit_v1(
    rent_program: Pubkey,
    market: Pubkey,
    release_set: [u8; 32],
    generation: u64,
    refund_owner: Pubkey,
) -> Result<(Pubkey, LifecycleRentCreditV2), SeriesPremarketExpiryChainErrorV1> {
    let refund = RefundAuthority::new(refund_owner.to_bytes())
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Identity)?;
    let market_id = LifecycleAccountIdV2::new(market.to_bytes())
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Identity)?;
    let release_id = LifecycleAccountIdV2::new(release_set)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Identity)?;
    let provisional = LifecycleRentCreditV2::new(refund, market_id, release_id, generation, 0)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let seeds = provisional.pda_seeds();
    let market_seed = seeds.market().to_bytes();
    let generation_seed = seeds.generation();
    let (address, bump) = Pubkey::find_program_address(
        &[seeds.domain(), &market_seed, &generation_seed],
        &rent_program,
    );
    let credit = LifecycleRentCreditV2::new(refund, market_id, release_id, generation, bump)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    if credit.to_bytes().len() != LIFECYCLE_RENT_CREDIT_BYTES_V2 {
        return Err(SeriesPremarketExpiryChainErrorV1::Record);
    }
    Ok((address, credit))
}

fn install_account(
    built: BuiltAccountV1,
    snapshot_for_rollback: bool,
) -> SeriesPremarketExpiryInstallAccountV1 {
    SeriesPremarketExpiryInstallAccountV1 {
        key: built.key,
        account: built.account,
        snapshot_for_rollback,
    }
}

/// Refuse a duplicated install coordinate, and refuse an UNSET one.
///
/// The second half cannot be spelled `key == Pubkey::default()`. The System
/// program's canonical address IS the all-zero pubkey, and this chain always
/// installs the System program, so that rejection is unsatisfiable by any list
/// this fixture can build -- it refused every one of them, and the three
/// on-chain tests in this file therefore never reached a bank. The System
/// entry and this check landed in the same commit (`4185b871`), so they were
/// contradictory from the moment they were written.
///
/// What was meant is "no account carries an unset key", and the account itself
/// separates the two cases: at the zero address, only the System program is an
/// executable owned by the native loader. An accidentally unset key belongs to
/// some data account, which is not, and is still refused here.
fn require_disjoint_install_accounts_v1(
    accounts: &[SeriesPremarketExpiryInstallAccountV1],
) -> Result<(), SeriesPremarketExpiryChainErrorV1> {
    let mut system_entries = 0_usize;
    for (index, candidate) in accounts.iter().enumerate() {
        if candidate.key == Pubkey::default() {
            if !candidate.account.executable || candidate.account.owner != native_loader::ID {
                return Err(SeriesPremarketExpiryChainErrorV1::Physical);
            }
            system_entries += 1;
        }
        if accounts
            .get(..index)
            .ok_or(SeriesPremarketExpiryChainErrorV1::Physical)?
            .iter()
            .any(|prior| prior.key == candidate.key)
        {
            return Err(SeriesPremarketExpiryChainErrorV1::Physical);
        }
    }
    // The exemption is for exactly one builtin, not for a class.
    if system_entries > 1 {
        return Err(SeriesPremarketExpiryChainErrorV1::Physical);
    }
    Ok(())
}

struct SeriesRecordInputV1 {
    realm: [u8; 32],
    release_set: [u8; 32],
    product_record: [u8; 32],
    product_id: [u8; 32],
    result_domain: [u8; 32],
    capability_manifest: [u8; 32],
    registry_program: Pubkey,
    core_program: Pubkey,
    founder: Pubkey,
    template_refund_owner: Pubkey,
    ticket_refund_owner: Pubkey,
    close_rent: u64,
}

/// Construct one disjoint one-occurrence record corpus, then hostile-admit it
/// through the kernel reexports before any record account is derived.
fn build_series_record_corpus_v1(
    input: SeriesRecordInputV1,
) -> Result<SeriesRecordCorpusV1, SeriesPremarketExpiryChainErrorV1> {
    // The Founding intent requires a nonzero retry deadline. ProgramTest is
    // warped to slot two before submission, strictly after this occurrence.
    const FIRST_SLOT: u64 = 1;
    const PERIOD_SLOTS: u64 = 2;
    const RETRY_WINDOW: u64 = 0;
    const HOARD_PRINCIPAL: u64 = 9;
    const MARKET_RENT: u64 = 11;
    const CAPABILITY_NATIVE: u64 = 13;
    const FOUNDING_WORK: u64 = 17;
    let generation = 1;
    let resolution_policy = [0x71; 32];
    let provisional = MarketIdentity {
        market_id: identity([0x72; 32])?,
        realm_id: identity(input.realm)?,
        product_record: identity(input.product_record)?,
        product_id: identity(input.product_id)?,
        resolution_policy: identity(resolution_policy)?,
        capability_manifest: identity(input.capability_manifest)?,
        selected_release_set: identity(input.release_set)?,
        registry_program: identity(input.registry_program.to_bytes())?,
        generation,
    };
    let future_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(provisional).as_slices(),
        &input.core_program,
    )
    .0;

    let mut occurrence = vec![0_u8; SERIES_OCCURRENCE_BYTES_V3];
    put(&mut occurrence, 0, b"DCLTSOV3")?;
    put(&mut occurrence, 8, &3_u16.to_le_bytes())?;
    put(&mut occurrence, 10, &1_u16.to_le_bytes())?;
    put(&mut occurrence, 12, &0_u32.to_le_bytes())?;
    put(&mut occurrence, 16, &FIRST_SLOT.to_le_bytes())?;
    put(&mut occurrence, 24, &input.product_record)?;
    put(&mut occurrence, 56, &resolution_policy)?;
    put(&mut occurrence, 88, &[0x73; 32])?;
    put(&mut occurrence, 120, &[0x74; 32])?;
    put(&mut occurrence, 152, &input.capability_manifest)?;
    put(&mut occurrence, 184, &[0x75; 32])?;
    put(&mut occurrence, 216, &future_market.to_bytes())?;
    put(&mut occurrence, 248, &HOARD_PRINCIPAL.to_le_bytes())?;
    put(&mut occurrence, 256, &MARKET_RENT.to_le_bytes())?;
    put(&mut occurrence, 264, &CAPABILITY_NATIVE.to_le_bytes())?;
    put(&mut occurrence, 272, &FOUNDING_WORK.to_le_bytes())?;
    let occurrence_id = occurrence_content_id(&occurrence)
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;

    let mut template = vec![0_u8; SERIES_TEMPLATE_BYTES_V3];
    put(&mut template, 0, b"DCLTSTV3")?;
    put(&mut template, 8, &3_u16.to_le_bytes())?;
    put(&mut template, 10, &1_u16.to_le_bytes())?;
    put(
        &mut template,
        12,
        &SERIES_PREMARKET_TEMPLATE_OCCURRENCE_COUNT_V1.to_le_bytes(),
    )?;
    put(&mut template, 16, &FIRST_SLOT.to_le_bytes())?;
    put(&mut template, 24, &PERIOD_SLOTS.to_le_bytes())?;
    put(&mut template, 32, &RETRY_WINDOW.to_le_bytes())?;
    put(&mut template, 40, &input.close_rent.to_le_bytes())?;
    put(&mut template, 48, &input.realm)?;
    put(&mut template, 80, &input.release_set)?;
    for (offset, byte) in [
        (112, 0x76),
        (144, 0x77),
        (176, 0x78),
        (208, 0x79),
        (240, 0x7a),
        (272, 0x7b),
        (304, 0x7c),
    ] {
        put(&mut template, offset, &[byte; 32])?;
    }
    put(&mut template, 336, &occurrence_id.to_bytes())?;
    put(&mut template, 368, &input.template_refund_owner.to_bytes())?;
    let template_id =
        template_content_id(&template).map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;

    let mut ticket = vec![0_u8; SERIES_TICKET_BYTES_V3];
    put(&mut ticket, 0, b"DCLTSKV3")?;
    put(&mut ticket, 8, &3_u16.to_le_bytes())?;
    put(&mut ticket, 10, &1_u16.to_le_bytes())?;
    put(&mut ticket, 12, &0_u32.to_le_bytes())?;
    put(&mut ticket, 16, &template_id.to_bytes())?;
    put(&mut ticket, 48, &occurrence_id.to_bytes())?;
    put(&mut ticket, 80, &future_market.to_bytes())?;
    put(&mut ticket, 112, &[0x75; 32])?;
    put(&mut ticket, 144, &input.founder.to_bytes())?;
    put(&mut ticket, 176, &input.ticket_refund_owner.to_bytes())?;
    put(&mut ticket, 208, &HOARD_PRINCIPAL.to_le_bytes())?;
    put(&mut ticket, 216, &MARKET_RENT.to_le_bytes())?;
    put(&mut ticket, 224, &CAPABILITY_NATIVE.to_le_bytes())?;
    put(&mut ticket, 232, &FOUNDING_WORK.to_le_bytes())?;

    let admitted = admit_occurrence(&template, &occurrence, &[])
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let admitted_ticket =
        admit_ticket(&ticket).map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    admitted
        .require_ticket(admitted_ticket.ticket())
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    let product = AuthenticatedProductProjectionV2::new(
        dclutch_core_contract::ContentId::new(input.product_record)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
        dclutch_core_contract::ContentId::new(input.product_id)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
        dclutch_core_contract::ContentId::new(input.result_domain)
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
    );
    let future = future_market_projection(
        admitted,
        product,
        dclutch_trading_sbf::series::AccountKeyV3::new(input.registry_program.to_bytes())
            .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
    )
    .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    future
        .require_address(
            dclutch_trading_sbf::series::AccountKeyV3::new(future_market.to_bytes())
                .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?,
        )
        .map_err(|_| SeriesPremarketExpiryChainErrorV1::Record)?;
    Ok(SeriesRecordCorpusV1 {
        template,
        occurrence,
        ticket,
        template_id,
        occurrence_id,
        ticket_id: admitted_ticket.content_id(),
        future_market,
        generation,
        expiry_slot: FIRST_SLOT + RETRY_WINDOW,
    })
}

fn put(
    output: &mut [u8],
    offset: usize,
    value: &[u8],
) -> Result<(), SeriesPremarketExpiryChainErrorV1> {
    output
        .get_mut(offset..offset.saturating_add(value.len()))
        .ok_or(SeriesPremarketExpiryChainErrorV1::Record)?
        .copy_from_slice(value);
    Ok(())
}

fn validate_input(
    input: &SeriesPremarketExpiryChainInputV1<'_>,
) -> Result<(), SeriesPremarketExpiryChainErrorV1> {
    let identities = [
        input.registry_program,
        input.trading_program,
        input.core_program,
        input.claims_program,
        input.custody_program,
        input.rent_program,
        input.releases.activation,
        input.releases.core_programdata,
        input.releases.trading_programdata,
        input.releases.claims_programdata,
    ];
    if input.releases.release_set == [0; 32]
        || identities.iter().any(|key| *key == Pubkey::default())
    {
        return Err(SeriesPremarketExpiryChainErrorV1::Identity);
    }
    for (index, left) in identities.iter().enumerate() {
        if identities.iter().skip(index + 1).any(|right| left == right) {
            return Err(SeriesPremarketExpiryChainErrorV1::Identity);
        }
    }
    if input.elves.registry.is_empty()
        || input.elves.trading.is_empty()
        || input.elves.core.is_empty()
        || input.elves.claims.is_empty()
        || input.elves.custody.is_empty()
    {
        return Err(SeriesPremarketExpiryChainErrorV1::Identity);
    }
    Ok(())
}

fn _abi_type_pins(account: Account, identity: Identity) -> (Account, Identity) {
    (account, identity)
}

#[cfg(test)]
mod native_tests {
    use dclutch_custody::{CustodyRequestV1, OperationV1};
    use dclutch_trading::series::SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3;
    use dclutch_trading_sbf::series::artifacts_v3::SeriesArtifactSelectionV3;

    use super::*;

    /// ONE AUTHOR FOR THE SERIES ROOT'S CONFIG IDENTITY, PROVED WITHOUT AN ELF.
    ///
    /// A Series root's `selection().config()` is the Registry RECORD DIGEST of
    /// the root's config record, exactly as every other family's is. Nothing
    /// about that was ever a choice: a Registry record's coordinate is
    /// `[RAW_RECORD_PDA_SEED_V1, schema, digest]` with `digest == hash(bytes)`,
    /// and `borrow_record_against` refuses unless `hash(&data) == digest`. So
    /// the DOMAIN-SEPARATED `template_content_id(t)` can never be the identity
    /// of a record whose bytes are `t` -- it names a coordinate at which no
    /// Registry record can exist. The Series family had a second author saying
    /// otherwise at six sites, and that is why nothing Series ever executed
    /// through the family-neutral Hot prelude.
    ///
    /// Both values still exist, and each now has exactly one author:
    ///
    /// - `hash(t)` -- the record digest. The root's config field, the manifest
    ///   entry's `config_id`, the config record's PDA coordinate, and what
    ///   Core's four Series routes compare the root against.
    /// - `template_content_id(t) = sha256("dclutch/series-template-v3" || 0x00
    ///   || t)` -- the Template's content identity. The family request's
    ///   `template()`, the occurrence proof's root, the Ticket derivation, and
    ///   what `SeriesArtifactSelectionV3::from_config_record` DERIVES from the
    ///   config record's bytes. It is no longer readable off a root.
    ///
    /// The Series config record IS the Template record: every Series action
    /// descriptor pins `config_schema() == SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3`,
    /// which is the schema this corpus installs the Template under. So the two
    /// authors are one record's bytes, read twice, by two named functions.
    #[test]
    fn the_series_root_config_identity_has_one_author() {
        let product = build_product_record_corpus_v1().expect("Product corpus");
        let realm = build_realm_record_v1(key(0xc7)).expect("Realm corpus");
        let refund = key(0xc6);
        let records = build_series_record_corpus_v1(SeriesRecordInputV1 {
            realm: hash(&realm).to_bytes(),
            release_set: [0xc8; 32],
            product_record: hash(&product.product).to_bytes(),
            product_id: product.product_id,
            result_domain: product.result_domain_digest,
            capability_manifest: hash(&EMPTY_MANIFEST_BYTES).to_bytes(),
            registry_program: key(0xc1),
            core_program: key(0xc4),
            founder: key(0xca),
            template_refund_owner: refund,
            ticket_refund_owner: refund,
            close_rent: Rent::default().minimum_balance(32),
        })
        .expect("Series corpus");

        // The record digest is the coordinate the Registry itself derives, and
        // it is the only value a root's config field can name.
        let registry = key(0xc1);
        // The schema this corpus installs the Template under IS the schema
        // every Series action descriptor names as its `config_schema()`. That
        // identity is what makes the config record and the Template record one
        // account rather than two.
        assert_eq!(
            hash(b"dclutch/schema/series-template-v3").to_bytes(),
            SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        );
        let record = derive_record(
            registry,
            SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
            &records.template,
        );
        assert_eq!(record.digest, hash(&records.template).to_bytes());
        assert_eq!(
            record.raw,
            Pubkey::find_program_address(
                &[
                    RAW_RECORD_PDA_SEED_V1,
                    &SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
                    &record.digest,
                ],
                &registry,
            )
            .0
        );

        // The Template's content identity is a different value over the same
        // bytes, and the artifact join now DERIVES it from those bytes. A
        // caller cannot hand the join a root's config field any more: the
        // struct has no public fields and one constructor.
        let selection = SeriesArtifactSelectionV3::from_config_record([7; 32], &records.template)
            .expect("config-record selection");
        assert_eq!(
            selection.template(),
            template_content_id(&records.template).expect("Template content id"),
        );
        assert_eq!(selection.template(), records.template_id);

        // They are distinct, which is exactly why one field could not be both.
        assert_ne!(record.digest, records.template_id.to_bytes());
    }

    #[test]
    fn root_independent_expiry_substrate_is_exact_and_disjoint() {
        let rent = Rent::default();
        let registry = key(0xa1);
        let trading = key(0xa2);
        let custody = key(0xa3);
        let core = key(0xa4);
        let rent_program = key(0xa5);
        let refund = key(0xa6);
        let product = build_product_record_corpus_v1().expect("Product corpus");
        let realm = build_realm_record_v1(key(0xa7)).expect("Realm corpus");
        let release_set = [0xa8; 32];
        let records = build_series_record_corpus_v1(SeriesRecordInputV1 {
            realm: hash(&realm).to_bytes(),
            release_set,
            product_record: hash(&product.product).to_bytes(),
            product_id: product.product_id,
            result_domain: product.result_domain_digest,
            capability_manifest: hash(&EMPTY_MANIFEST_BYTES).to_bytes(),
            registry_program: registry,
            core_program: core,
            founder: key(0xaa),
            template_refund_owner: refund,
            ticket_refund_owner: refund,
            close_rent: rent.minimum_balance(32),
        })
        .expect("Series corpus");
        let substrate = build_root_independent_substrate_v1(
            &rent,
            registry,
            trading,
            custody,
            core,
            rent_program,
            release_set,
            refund,
            &product,
            &realm,
            &records,
        )
        .expect("root-independent substrate");

        assert_eq!(substrate.install_accounts.len(), 26);
        assert_eq!(substrate.future_market.key, records.future_market);
        assert_ne!(substrate.future_market.key, substrate.rent_credit.key);
        assert_ne!(substrate.future_market.key, substrate.permit_account.key);
        assert!(substrate.future_market.account.data.is_empty());
        assert!(substrate.permit_account.account.data.is_empty());
        assert_eq!(
            substrate.permit_account.account.lamports,
            rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1)
        );
        let credit =
            LifecycleRentCreditV2::decode(&substrate.rent_credit.account.data).expect("RentCredit");
        assert_eq!(credit.market().to_bytes(), records.future_market.to_bytes());
        // THE THREE AUTHORITIES THAT MUST NAME ONE WALLET, staged so they do.
        // The Series kernel (`terminal.rs::requires_wallet`), Core
        // (`series_permit_expiry.rs::authenticate_rent_credit_coordinates`) and
        // Trading's pre-CPI mirror each require the RentCredit's `refund_wallet`
        // to be the Ticket's refund owner; the Expire transition additionally
        // requires the Template's. Until this commit the fixture staged the
        // Ticket's as the RentCredit's own ADDRESS and the first two refused.
        assert_eq!(credit.refund_wallet().to_bytes(), refund.to_bytes());
        assert_eq!(
            admit_ticket(&records.ticket)
                .expect("Ticket record")
                .ticket()
                .refund_owner()
                .to_bytes(),
            refund.to_bytes()
        );
        assert_ne!(substrate.rent_credit.key, refund);

        let normal = &substrate.normal_custody;
        assert_eq!(normal.family_request.len(), 128);
        assert_eq!(normal.success_transitions.len(), 3);
        // The Ticket's refund owner is a WALLET, and the Custody token refund
        // destination is an account it OWNS. The lifecycle RentCredit is a
        // separate account, reached by the Expire effect at its own fixed
        // coordinate `SERIES_EXPIRE_RENT_CREDIT_COORDINATE_V5`, and it is where
        // the closed replay's and vault's rent lands -- not where the escrow's
        // collateral refund goes.
        assert_eq!(normal.rent_refund, refund);
        assert_ne!(normal.rent_refund, substrate.rent_credit.key);
        assert_eq!(
            CustodyRequestV1::decode(&normal.prepare_initialize)
                .expect("Initialize")
                .operation,
            OperationV1::InitializeReplay
        );
        assert_eq!(
            CustodyRequestV1::decode(&normal.expire_refund)
                .expect("Refund")
                .operation,
            OperationV1::Transfer
        );
        assert_eq!(
            CustodyRequestV1::decode(&normal.expire_close_vault)
                .expect("Close Vault")
                .operation,
            OperationV1::CloseVault
        );
        assert_eq!(
            CustodyRequestV1::decode(&normal.expire_close_replay)
                .expect("Close replay")
                .operation,
            OperationV1::CloseReplay
        );
        let replay = substrate
            .install_accounts
            .iter()
            .find(|account| account.key == normal.replay)
            .expect("normal replay account");
        let replay = CustodyReplayV1::decode(&replay.account.data).expect("normal replay body");
        assert_eq!(replay.next_revision, 3);
        assert_eq!(replay.open_vault_count, 1);
        let vault = substrate
            .install_accounts
            .iter()
            .find(|account| account.key == normal.escrow_vault)
            .expect("SeriesEscrow Vault");
        assert_eq!(
            TokenAccount::parse(&vault.account.data)
                .expect("SeriesEscrow token state")
                .amount,
            9
        );
        let refund_token = substrate
            .install_accounts
            .iter()
            .find(|account| account.key == normal.refund_destination)
            .expect("refund token account");
        assert_eq!(
            TokenAccount::parse(&refund_token.account.data)
                .expect("refund token state")
                .amount,
            0
        );

        let parent_root = key(0xab);
        let projected = build_projected_custody_corpus_v1(
            &rent,
            registry,
            trading,
            core,
            custody,
            rent_program,
            release_set,
            parent_root,
            &product,
            &realm,
            &records,
            normal,
            substrate.rent_credit.key,
        )
        .expect("projected Custody substrate");
        assert_eq!(projected.install_accounts.len(), 3);
        assert_eq!(projected.success_transitions.len(), 2);
        assert_eq!(
            ProjectedCustodyRequestV1::decode(&projected.prepare_initialize)
                .expect("projected initialize")
                .operation,
            ProjectedCustodyOperationV1::Initialize,
        );
        assert_eq!(
            ProjectedCustodyRequestV1::decode(&projected.prepare_open)
                .expect("projected open")
                .operation,
            ProjectedCustodyOperationV1::OpenHoard,
        );
        assert_eq!(
            ProjectedCustodyRequestV1::decode(&projected.consume_lock)
                .expect("projected lock")
                .operation,
            ProjectedCustodyOperationV1::LockHoardAndCloseSource,
        );
        assert_eq!(
            ProjectedCustodyRequestV1::decode(&projected.consume_realize)
                .expect("projected realize")
                .operation,
            ProjectedCustodyOperationV1::RealizeAndClose,
        );
        let abort =
            ProjectedCustodyRequestV1::decode(&projected.expire_abort).expect("projected abort");
        assert_eq!(
            abort.operation,
            ProjectedCustodyOperationV1::AbortOpenAndClose
        );
        assert_eq!(abort.parent_capability_root, parent_root.to_bytes());
        assert_eq!(abort.market, records.future_market.to_bytes());
        assert_eq!(abort.rent_credit, substrate.rent_credit.key.to_bytes());
        let projected_state = projected
            .install_accounts
            .iter()
            .find(|account| account.key == projected.state)
            .expect("projected state");
        let projected_state = ProjectedCustodyStateV2::decode(&projected_state.account.data)
            .expect("projected state body");
        assert_eq!(projected_state.phase, ProjectedCustodyPhaseV1::HoardOpen);
        assert_eq!(projected_state.next_revision, 2);
        assert_eq!(projected_state.locked_amount, 0);
    }

    #[test]
    fn expiry_occurrence_refuses_controller_manifest_substitution() {
        let rent = Rent::default();
        let registry = key(0xb1);
        let trading = key(0xb2);
        let custody = key(0xb3);
        let core = key(0xb4);
        let rent_program = key(0xb5);
        let refund = key(0xb6);
        let product = build_product_record_corpus_v1().expect("Product corpus");
        let realm = build_realm_record_v1(key(0xb7)).expect("Realm corpus");
        let release_set = [0xb8; 32];
        let controller_manifest_digest = hash(b"controller-series-manifest").to_bytes();
        assert_ne!(
            controller_manifest_digest,
            hash(&EMPTY_MANIFEST_BYTES).to_bytes()
        );
        let records = build_series_record_corpus_v1(SeriesRecordInputV1 {
            realm: hash(&realm).to_bytes(),
            release_set,
            product_record: hash(&product.product).to_bytes(),
            product_id: product.product_id,
            result_domain: product.result_domain_digest,
            capability_manifest: controller_manifest_digest,
            registry_program: registry,
            core_program: core,
            founder: key(0xba),
            template_refund_owner: refund,
            ticket_refund_owner: refund,
            close_rent: rent.minimum_balance(32),
        })
        .expect("Series corpus with controller manifest");

        let refusal = match build_root_independent_substrate_v1(
            &rent,
            registry,
            trading,
            custody,
            core,
            rent_program,
            release_set,
            refund,
            &product,
            &realm,
            &records,
        ) {
            Ok(_) => panic!("controller manifest must not substitute for empty child manifest"),
            Err(refusal) => refusal,
        };
        assert_eq!(refusal, SeriesPremarketExpiryChainErrorV1::Record);
    }
}
