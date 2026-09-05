//! Chain-derived terminal Direct close and Custody replay handoff.
//!
//! Both builders accept exact same-finalized account observations, authenticate
//! the persisted authority graph, and emit unsigned instructions plus complete
//! expected poststate. They perform no RPC, signing, or submission.

use dclutch_vm::account_profile::AccountProfileV1;
use dclutch_market::capability_manifest::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityFundingLedgerDerivationV2,
    CapabilityManifestV1, FUNDING_LEDGER_ACTIVE_ADMISSIBLE_STATES_V2, FundingLedgerCloseCustodyV2,
    FundingLedgerV2, capability_dependency_closure_mask_v1, funding::funded_rent_persists_v1,
    manifest_entry_for_ledger_row_v2, validate_funding_ledger_masks_v2,
};
use dclutch_market::capability_program::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1,
    CapabilityRootHeaderV1,
    set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
};
use dclutch_claims::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2,
};
use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CUSTODY_REPLAY_BYTES_V1, CompartmentV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyVaultSeedsV1, RetirementReplayHandoffAccountLayoutV1,
    RetirementReplayHandoffObservationV1, RetirementReplayHandoffPlanV1,
    RetirementReplayHandoffReceiptV1, RetirementReplayHandoffRequestV1,
};
use dclutch_trading::{
    native_close_bundle_v1::{
        DIRECT_NATIVE_CLOSE_SELECTOR_V1, direct_native_close_account_profile_schema_v1,
        direct_native_close_effect_schema_v1, direct_native_close_request_v1,
    },
    successor::{DirectRootPhaseV1, DirectRootStateV1},
};
use dclutch_vm::effect::v2::ProgramV2 as EffectProgramV2;
use dclutch_market::{
    Action, CapabilityFundingHeaderV2, CapabilityRouteLayoutV1, CoreEffectActionV1,
    CoreEffectEnvelopeV1, Request, Role,
};
use dclutch_market::{CoreState, MarketCoreStateSeedsV2, Phase};
use dclutch_market::realm::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1, DeploymentObservationV1,
};
use dclutch_registry::svm::{ProgramDataV3View, ProgramV3View};
use dclutch_registry::release_set::{
    CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1, ExecutionRoleV1,
};
use dclutch_market::rent::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_custody::token_svm::{AccountState, COption, TokenAccount};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program};

use crate::{
    Finality, Observation, ObservedAccount,
    observation::{FinalizedRecordProof, authenticate_finalized_record, decode_rent},
};

/// Coordinate-only finalized-record pair for a frozen terminal ALT union.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalRecordCoordinatesV1 {
    /// Canonical raw-record PDA.
    pub raw: Pubkey,
    /// Canonical vacant staging-cursor PDA.
    pub staging: Pubkey,
}

/// Coordinate-only Loader deployment pair for a frozen terminal ALT union.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalDeploymentCoordinatesV1 {
    /// Executable Program address.
    pub program: Pubkey,
    /// Current ProgramData address.
    pub programdata: Pubkey,
}

/// Owner-assigned lookup policy for one terminal instruction meta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalMetaClassV1 {
    /// Immutable state, record, deployment-data, or sysvar coordinate.
    LookupStable,
    /// Transaction signer that must remain inline.
    InlineSigner,
    /// Executable program address that must remain inline.
    InlineProgram,
    /// PDA derived from the immutable request commitment that must remain inline.
    InlineRequestBound,
}

/// Exact coordinate-only unsigned instruction closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalCoordinateClosureV1 {
    /// Program invoked by the future instruction.
    pub program_id: Pubkey,
    /// Exact ordered account metas, including admitted aliases.
    pub accounts: Vec<AccountMeta>,
    /// Exact semantic-owner-assigned class for each account meta.
    pub classes: Vec<TerminalMetaClassV1>,
}

/// Immutable coordinates and request commitment for production Direct close.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectNativeCloseCoordinateInputV1 {
    /// Selected execution release-set identity.
    pub release_set: [u8; 32],
    /// Exact role-request digest for the production close.
    pub role_request_digest: [u8; 32],
    /// Core Market.
    pub market: Pubkey,
    /// Realm raw/staging coordinates.
    pub realm: TerminalRecordCoordinatesV1,
    /// Capability-manifest raw/staging coordinates.
    pub manifest: TerminalRecordCoordinatesV1,
    /// Resolution-owned `0b0111` dependency ledger.
    pub resolution_funding: Pubkey,
    /// Trading-owned `0b1000` selected ledger.
    pub trading_funding: Pubkey,
    /// Direct root.
    pub root: Pubkey,
    /// Registry activation cache.
    pub activation_cache: Pubkey,
    /// Core deployment.
    pub core: TerminalDeploymentCoordinatesV1,
    /// Trading deployment.
    pub trading: TerminalDeploymentCoordinatesV1,
    /// Resolution deployment.
    pub resolution: TerminalDeploymentCoordinatesV1,
    /// Registry program.
    pub registry_program: Pubkey,
    /// Rent sysvar.
    pub rent_sysvar: Pubkey,
    /// Direct ProgramSet raw/staging coordinates.
    pub program_set: TerminalRecordCoordinatesV1,
    /// Direct config raw/staging coordinates.
    pub config: TerminalRecordCoordinatesV1,
    /// Native-close AccountProfile raw/staging coordinates.
    pub close_profile: TerminalRecordCoordinatesV1,
    /// Native-close Effect raw/staging coordinates.
    pub close_effect: TerminalRecordCoordinatesV1,
    /// System Program.
    pub system_program: Pubkey,
    /// Native-close descriptor raw/staging coordinates.
    pub close_descriptor: TerminalRecordCoordinatesV1,
    /// Rent program.
    pub rent_program: Pubkey,
    /// Market RentCredit.
    pub rent_credit: Pubkey,
}

/// Immutable coordinates and request commitment for replay handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetirementReplayHandoffCoordinateInputV1 {
    /// Selected execution release-set identity.
    pub release_set: [u8; 32],
    /// Exact Custody-context identity.
    pub context: [u8; 32],
    /// SHA-256 of the exact handoff request.
    pub request_digest: [u8; 32],
    /// Signing payer.
    pub payer: Pubkey,
    /// Core Market.
    pub market: Pubkey,
    /// Registry activation cache.
    pub activation_cache: Pubkey,
    /// Registry program.
    pub registry_program: Pubkey,
    /// Core deployment.
    pub core: TerminalDeploymentCoordinatesV1,
    /// Trading deployment.
    pub trading: TerminalDeploymentCoordinatesV1,
    /// Custody deployment.
    pub custody: TerminalDeploymentCoordinatesV1,
    /// Claims aggregate.
    pub claims_aggregate: Pubkey,
    /// Realm raw/staging coordinates.
    pub realm: TerminalRecordCoordinatesV1,
    /// Rent sysvar.
    pub rent_sysvar: Pubkey,
    /// Market RentCredit.
    pub rent_credit: Pubkey,
    /// Trading-role replay.
    pub trading_replay: Pubkey,
    /// Core-role replay.
    pub core_replay: Pubkey,
    /// Shared Hoard token account.
    pub hoard: Pubkey,
    /// System Program.
    pub system_program: Pubkey,
    /// Realm collateral mint.
    pub mint: Pubkey,
    /// Realm token program.
    pub token_program: Pubkey,
    /// Custody authority PDA.
    pub custody_authority: Pubkey,
}

/// One exact finalized account graph for the retirement replay handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetirementReplayHandoffSnapshotV1 {
    /// Externally signing payer for the vacant Core replay.
    pub payer: ObservedAccount,
    /// Retiring Core Market.
    pub market: ObservedAccount,
    /// Market-selected Registry activation cache.
    pub activation_cache: ObservedAccount,
    /// Current executable Registry program.
    pub registry_program: ObservedAccount,
    /// Current executable Core program.
    pub core_program: ObservedAccount,
    /// Current Core ProgramData.
    pub core_programdata: ObservedAccount,
    /// Current executable Trading program.
    pub trading_program: ObservedAccount,
    /// Current Trading ProgramData.
    pub trading_programdata: ObservedAccount,
    /// Current executable Custody program.
    pub custody_program: ObservedAccount,
    /// Current Custody ProgramData.
    pub custody_programdata: ObservedAccount,
    /// Vacant canonical Core caller-authority PDA, absent only during discovery preflight.
    pub caller_authority: Option<ObservedAccount>,
    /// Live Claims aggregate owning the Custody context.
    pub claims_aggregate: ObservedAccount,
    /// Finalized Realm record.
    pub realm: ObservedAccount,
    /// Vacant Realm staging cursor.
    pub realm_staging: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Market RentCredit receiving source replay rent.
    pub rent_credit: ObservedAccount,
    /// Live Trading-role replay with one open Hoard Vault.
    pub trading_replay: ObservedAccount,
    /// Vacant canonical Core-role replay.
    pub core_replay: ObservedAccount,
    /// Shared Hoard token account, unchanged by the handoff.
    pub hoard: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Realm collateral mint.
    pub mint: ObservedAccount,
    /// Realm-selected executable token program.
    pub token_program: ObservedAccount,
    /// Canonical Custody authority PDA.
    pub custody_authority: ObservedAccount,
}

/// Complete unsigned handoff instruction and exact finalized-resume facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetirementReplayHandoffReportV1 {
    /// Permissionless unsigned Core instruction.
    pub instruction: Instruction,
    /// Exact coordinate-only meta closure and owner-assigned lookup classes.
    pub meta_closure: TerminalCoordinateClosureV1,
    /// Finalized observation shared by every account.
    pub observation: Observation,
    /// Exact fixed request bytes.
    pub request_body: [u8; 208],
    /// SHA-256 of the exact request bytes.
    pub request_digest: [u8; 32],
    /// Canonical Core caller-authority PDA.
    pub caller_authority: Pubkey,
    /// Exact DCLCRHR1 return-data body.
    pub expected_receipt_body: [u8; 512],
    /// Typed expected receipt.
    pub expected_receipt: RetirementReplayHandoffReceiptV1,
    /// Closed Trading replay identity.
    pub trading_replay: Pubkey,
    /// Created Core replay identity.
    pub core_replay: Pubkey,
    /// Exact Core replay bytes after creation.
    pub expected_core_replay_data: Vec<u8>,
    /// Exact Core replay data digest.
    pub expected_core_replay_digest: [u8; 32],
    /// Exact Core replay owner after creation.
    pub expected_core_replay_owner: Pubkey,
    /// Exact Core replay lamports after creation.
    pub expected_core_replay_lamports: u64,
    /// Exact Core replay executable bit after creation.
    pub expected_core_replay_executable: bool,
    /// Exact closed Trading replay refund.
    pub trading_replay_refund_lamports: u64,
    /// Exact Trading replay owner after closure.
    pub expected_trading_replay_owner: Pubkey,
    /// Exact Trading replay bytes after closure.
    pub expected_trading_replay_data: Vec<u8>,
    /// Exact Trading replay lamports after closure.
    pub expected_trading_replay_lamports: u64,
    /// Exact payer owner, unchanged by the handoff.
    pub expected_payer_owner: Pubkey,
    /// Exact payer bytes, unchanged by the handoff.
    pub expected_payer_data: Vec<u8>,
    /// Exact payer lamports after the rent prepayment.
    pub expected_payer_lamports: u64,
    /// Exact RentCredit owner, unchanged by the handoff.
    pub expected_rent_credit_owner: Pubkey,
    /// Exact RentCredit bytes, unchanged by the handoff.
    pub expected_rent_credit_data: Vec<u8>,
    /// Exact RentCredit lamports after the source refund.
    pub expected_rent_credit_lamports: u64,
    /// Unchanged Hoard identity.
    pub hoard: Pubkey,
    /// Exact unchanged Hoard bytes.
    pub expected_hoard_data: Vec<u8>,
    /// Exact unchanged Hoard owner.
    pub expected_hoard_owner: Pubkey,
    /// Exact unchanged Hoard lamports.
    pub expected_hoard_lamports: u64,
}

/// One exact finalized account graph for a Direct native capability close.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectNativeCloseSnapshotV1 {
    /// Retiring Core Market with one outstanding capability.
    pub market: ObservedAccount,
    /// Finalized Realm record selected by Market.
    pub realm: ObservedAccount,
    /// Vacant Realm staging cursor.
    pub realm_staging: ObservedAccount,
    /// Finalized capability manifest selected by Market.
    pub manifest: ObservedAccount,
    /// Vacant manifest staging cursor.
    pub manifest_staging: ObservedAccount,
    /// Exact ordered physical dependency and selected FundingLedgerV2 accounts.
    pub funding_ledgers: Vec<ObservedAccount>,
    /// Retiring Direct composite root.
    pub root: ObservedAccount,
    /// Market-selected Registry activation cache.
    pub activation_cache: ObservedAccount,
    /// Current executable Core program.
    pub core_program: ObservedAccount,
    /// Current Core ProgramData.
    pub core_programdata: ObservedAccount,
    /// Current executable Trading program.
    pub trading_program: ObservedAccount,
    /// Current Trading ProgramData.
    pub trading_programdata: ObservedAccount,
    /// Current executable Resolution program.
    pub resolution_program: ObservedAccount,
    /// Current Resolution ProgramData.
    pub resolution_programdata: ObservedAccount,
    /// Current executable Registry program.
    pub registry_program: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Vacant canonical Core caller authority, absent only during discovery preflight.
    pub caller_authority: Option<ObservedAccount>,
    /// Finalized Direct ProgramSet record.
    pub program_set: ObservedAccount,
    /// Vacant ProgramSet staging cursor.
    pub program_set_staging: ObservedAccount,
    /// Finalized Direct config record.
    pub config: ObservedAccount,
    /// Vacant config staging cursor.
    pub config_staging: ObservedAccount,
    /// Finalized native-close AccountProfile record.
    pub close_profile: ObservedAccount,
    /// Vacant close-profile staging cursor.
    pub close_profile_staging: ObservedAccount,
    /// Finalized native-close Effect record.
    pub close_effect: ObservedAccount,
    /// Vacant close-effect staging cursor.
    pub close_effect_staging: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Finalized native-close descriptor record.
    pub close_descriptor: ObservedAccount,
    /// Vacant close-descriptor staging cursor.
    pub close_descriptor_staging: ObservedAccount,
    /// Executable Rent program owning the Market RentCredit.
    pub rent_program: ObservedAccount,
    /// Market RentCredit receiving root and ledger lamports.
    pub rent_credit: ObservedAccount,
}

/// Request-bound caller coordinate discovered before its vacant account is fetched.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalCallerPreflightV1 {
    /// Exact finalized observation shared by every preflight input.
    pub observation: Observation,
    /// SHA-256 request commitment used in the caller-authority seeds.
    pub request_digest: [u8; 32],
    /// Canonical request-bound Core caller-authority PDA to fetch.
    pub caller_authority: Pubkey,
}

/// Exact unsigned Direct close and its complete expected closure facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectNativeCloseReportV1 {
    /// Permissionless unsigned Core instruction.
    pub instruction: Instruction,
    /// Exact coordinate-only meta closure and owner-assigned lookup classes.
    pub meta_closure: TerminalCoordinateClosureV1,
    /// Finalized observation shared by every account.
    pub observation: Observation,
    /// Canonical Core caller-authority PDA.
    pub caller_authority: Pubkey,
    /// SHA-256 of selection, funding header, and 0xffff_ff01 request.
    pub role_request_digest: [u8; 32],
    /// Exact selected lifecycle selector.
    pub selector: u32,
    /// Exact Core Market bytes after outstanding count reaches zero.
    pub expected_market_data: Vec<u8>,
    /// SHA-256 of expected Core Market bytes.
    pub expected_market_digest: [u8; 32],
    /// Exact Core Market owner, unchanged by close.
    pub expected_market_owner: Pubkey,
    /// Exact Core Market lamports, unchanged by close.
    pub expected_market_lamports: u64,
    /// Expected outstanding capability count, always zero.
    pub expected_outstanding_capabilities: u64,
    /// Direct root closed by Trading.
    pub closed_root: Pubkey,
    /// Exact root lamports refunded.
    pub root_refund_lamports: u64,
    /// Exact root owner after closure.
    pub expected_root_owner: Pubkey,
    /// Exact root bytes after closure.
    pub expected_root_data: Vec<u8>,
    /// Exact root lamports after closure.
    pub expected_root_lamports: u64,
    /// Funding ledger closed by Trading.
    pub closed_funding_ledger: Pubkey,
    /// Exact ledger lamports refunded.
    pub funding_refund_lamports: u64,
    /// Exact FundingLedger owner after closure.
    pub expected_funding_owner: Pubkey,
    /// Exact FundingLedger bytes after closure.
    pub expected_funding_data: Vec<u8>,
    /// Exact FundingLedger lamports after closure.
    pub expected_funding_lamports: u64,
    /// Every readonly dependency ledger preserved byte/owner/lamport-exactly.
    pub preserved_dependency_ledgers: Vec<PreservedFundingLedgerV1>,
    /// RentCredit identity.
    pub rent_credit: Pubkey,
    /// RentCredit lamports before close.
    pub rent_credit_pre_lamports: u64,
    /// Root plus ledger refund delta.
    pub rent_credit_delta_lamports: u64,
    /// RentCredit lamports after close.
    pub expected_rent_credit_lamports: u64,
    /// Exact RentCredit owner, unchanged by close.
    pub expected_rent_credit_owner: Pubkey,
    /// Exact RentCredit bytes, unchanged by close.
    pub expected_rent_credit_data: Vec<u8>,
}

/// Exact unchanged poststate for one readonly dependency FundingLedgerV2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreservedFundingLedgerV1 {
    /// Canonical dependency-ledger identity.
    pub key: Pubkey,
    /// Exact unchanged owner.
    pub owner: Pubkey,
    /// Exact unchanged bytes.
    pub data: Vec<u8>,
    /// SHA-256 of the exact unchanged bytes.
    pub data_digest: [u8; 32],
    /// Exact unchanged lamports.
    pub lamports: u64,
    /// Exact unchanged executable bit.
    pub executable: bool,
}

/// Stable refusal from terminal retirement construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalRetirementErrorV1 {
    /// Inputs were not one exact finalized observation.
    Snapshot,
    /// Two roles aliased outside an explicitly admitted close pair.
    Alias,
    /// Market identity, owner, generation, or phase refused.
    Market,
    /// Registry activation or current Loader deployment refused.
    Release,
    /// Claims aggregate or finalized Realm refused.
    Record,
    /// Rent, payer, replay, RentCredit, Hoard, mint, or token facts refused.
    Custody,
    /// Request, receipt, poststate, or checked balance arithmetic refused.
    Projection,
    /// Exact account privilege/order geometry refused.
    Frame,
    /// `dclutch_registry::release_set` refused; the cause is its own.
    ReleaseSet(dclutch_registry::release_set::Error),
    /// `dclutch_market` refused; the cause is its own.
    MarketCore(dclutch_market::Error),
    /// `dclutch_market::capability_manifest` refused; the cause is its own.
    Capability(dclutch_market::capability_manifest::Error),
    /// `dclutch_market::capability_program` refused; the cause is its own.
    CapabilityProgram(dclutch_market::capability_program::Error),
    /// `dclutch_operator` refused; the cause is its own.
    Observation(crate::observation::ObservationError),
    /// `dclutch_registry::svm` refused; the cause is its own.
    RegistrySvm(dclutch_registry::svm::Error),
    /// `dclutch_registry` refused; the cause is its own.
    Registry(dclutch_registry::Error),
    /// `dclutch_trading` refused; the cause is its own.
    Successor(dclutch_trading::successor::SuccessorError),
    /// `dclutch_market::capability_program` refused; the cause is its own.
    ProgramSet(dclutch_market::capability_program::set_v2::ProgramSetErrorV2),
    /// `dclutch_vm::account_profile` refused; the cause is its own.
    AccountProfile(dclutch_vm::account_profile::Error),
    /// `dclutch_vm::effect` refused; the cause is its own.
    EffectV2(dclutch_vm::effect::v2::Error),
    /// `dclutch_market::rent` refused; the cause is its own.
    LifecycleRent(dclutch_market::rent::lifecycle_v2::LifecycleRentErrorV2),
    /// `dclutch_custody` refused; the cause is its own.
    RetirementReplayHandoff(dclutch_custody::RetirementReplayHandoffErrorV1),
    /// `dclutch_custody` refused; the cause is its own.
    CustodyContract(dclutch_custody::Error),
    /// `dclutch_claims` refused; the cause is its own.
    LiabilityBasisState(dclutch_claims::liability_basis_state_v2::LiabilityBasisStateErrorV2),
    /// `dclutch_market::realm` refused; the cause is its own.
    Realm(dclutch_market::realm::Error),
    /// `dclutch_custody::token_svm` refused; the cause is its own.
    Token(dclutch_custody::token_svm::Error),
}

/// Derive the exact production `F=2` Direct-close account-meta closure.
///
/// This projection consumes coordinates and the immutable role-request
/// commitment only. It does not accept observations, account bytes, lamports,
/// or a predicted poststate.
pub fn project_direct_native_close_coordinate_closure_v1(
    input: &DirectNativeCloseCoordinateInputV1,
) -> Result<TerminalCoordinateClosureV1, TerminalRetirementErrorV1> {
    let caller_seeds = CallerAuthoritySeedsV1::from_bytes(
        input.release_set,
        input.market.to_bytes(),
        ExecutionRoleV1::Core,
        input.root.to_bytes(),
        input.role_request_digest,
    )
    .map_err(TerminalRetirementErrorV1::ReleaseSet)?;
    let caller = Pubkey::find_program_address(&caller_seeds.as_slices(), &input.core.program).0;
    let accounts = vec![
        AccountMeta::new(input.market, false),
        AccountMeta::new_readonly(input.realm.raw, false),
        AccountMeta::new_readonly(input.realm.staging, false),
        AccountMeta::new_readonly(input.manifest.raw, false),
        AccountMeta::new_readonly(input.manifest.staging, false),
        AccountMeta::new_readonly(input.resolution_funding, false),
        AccountMeta::new(input.trading_funding, false),
        AccountMeta::new(input.root, false),
        AccountMeta::new_readonly(input.activation_cache, false),
        AccountMeta::new_readonly(input.core.program, false),
        AccountMeta::new_readonly(input.core.programdata, false),
        AccountMeta::new_readonly(input.trading.program, false),
        AccountMeta::new_readonly(input.trading.programdata, false),
        AccountMeta::new_readonly(input.resolution.program, false),
        AccountMeta::new_readonly(input.resolution.programdata, false),
        AccountMeta::new_readonly(input.registry_program, false),
        AccountMeta::new_readonly(input.rent_sysvar, false),
        AccountMeta::new_readonly(caller, false),
        AccountMeta::new_readonly(input.program_set.raw, false),
        AccountMeta::new_readonly(input.program_set.staging, false),
        AccountMeta::new_readonly(input.config.raw, false),
        AccountMeta::new_readonly(input.config.staging, false),
        AccountMeta::new_readonly(input.close_profile.raw, false),
        AccountMeta::new_readonly(input.close_profile.staging, false),
        AccountMeta::new_readonly(input.close_effect.raw, false),
        AccountMeta::new_readonly(input.close_effect.staging, false),
        AccountMeta::new_readonly(input.activation_cache, false),
        AccountMeta::new_readonly(input.core.program, false),
        AccountMeta::new_readonly(input.core.programdata, false),
        AccountMeta::new_readonly(input.trading.program, false),
        AccountMeta::new_readonly(input.trading.programdata, false),
        AccountMeta::new_readonly(input.registry_program, false),
        AccountMeta::new_readonly(input.rent_sysvar, false),
        AccountMeta::new_readonly(input.system_program, false),
        AccountMeta::new_readonly(input.close_descriptor.raw, false),
        AccountMeta::new_readonly(input.close_descriptor.staging, false),
        AccountMeta::new_readonly(input.rent_program, false),
        AccountMeta::new(input.rent_credit, false),
    ];
    let layout =
        CapabilityRouteLayoutV1::new(2, 20).map_err(TerminalRetirementErrorV1::MarketCore)?;
    if accounts.len() != layout.account_count() || !exact_close_aliases(&accounts, layout) {
        return Err(TerminalRetirementErrorV1::Frame);
    }
    let mut classes = vec![TerminalMetaClassV1::LookupStable; accounts.len()];
    for index in [9_usize, 11, 13, 15, 27, 29, 31, 33, 36] {
        *classes
            .get_mut(index)
            .ok_or(TerminalRetirementErrorV1::Frame)? = TerminalMetaClassV1::InlineProgram;
    }
    *classes
        .get_mut(17)
        .ok_or(TerminalRetirementErrorV1::Frame)? = TerminalMetaClassV1::InlineRequestBound;
    Ok(TerminalCoordinateClosureV1 {
        program_id: input.core.program,
        accounts,
        classes,
    })
}

/// Derive the exact 23-role retirement replay-handoff account-meta closure.
///
/// This projection consumes coordinates and immutable request commitments
/// only. It does not accept observations, account bytes, lamports, or a
/// predicted poststate.
pub fn project_retirement_replay_handoff_coordinate_closure_v1(
    input: &RetirementReplayHandoffCoordinateInputV1,
) -> Result<TerminalCoordinateClosureV1, TerminalRetirementErrorV1> {
    let caller_seeds = CallerAuthoritySeedsV1::from_bytes(
        input.release_set,
        input.market.to_bytes(),
        ExecutionRoleV1::Core,
        input.context,
        input.request_digest,
    )
    .map_err(TerminalRetirementErrorV1::ReleaseSet)?;
    let caller = Pubkey::find_program_address(&caller_seeds.as_slices(), &input.core.program).0;
    let coordinates = [
        input.payer,
        input.market,
        input.activation_cache,
        input.registry_program,
        input.core.program,
        input.core.programdata,
        input.trading.program,
        input.trading.programdata,
        input.custody.program,
        input.custody.programdata,
        caller,
        input.claims_aggregate,
        input.realm.raw,
        input.realm.staging,
        input.rent_sysvar,
        input.rent_credit,
        input.trading_replay,
        input.core_replay,
        input.hoard,
        input.system_program,
        input.mint,
        input.token_program,
        input.custody_authority,
    ];
    for (left, coordinate) in coordinates.iter().enumerate() {
        if coordinates
            .iter()
            .skip(left.saturating_add(1))
            .any(|other| coordinate == other)
        {
            return Err(TerminalRetirementErrorV1::Alias);
        }
    }
    let accounts = coordinates
        .into_iter()
        .enumerate()
        .map(|(index, key)| {
            let writable = matches!(index, 0 | 15 | 16 | 17);
            let signer = index == 0;
            if writable {
                AccountMeta::new(key, signer)
            } else {
                AccountMeta::new_readonly(key, signer)
            }
        })
        .collect::<Vec<_>>();
    if accounts.len() != RetirementReplayHandoffAccountLayoutV1::COUNT {
        return Err(TerminalRetirementErrorV1::Frame);
    }
    let mut classes = vec![TerminalMetaClassV1::LookupStable; accounts.len()];
    *classes.get_mut(0).ok_or(TerminalRetirementErrorV1::Frame)? =
        TerminalMetaClassV1::InlineSigner;
    for index in [3_usize, 4, 6, 8, 19, 21] {
        *classes
            .get_mut(index)
            .ok_or(TerminalRetirementErrorV1::Frame)? = TerminalMetaClassV1::InlineProgram;
    }
    *classes
        .get_mut(10)
        .ok_or(TerminalRetirementErrorV1::Frame)? = TerminalMetaClassV1::InlineRequestBound;
    Ok(TerminalCoordinateClosureV1 {
        program_id: input.core.program,
        accounts,
        classes,
    })
}

/// Discover the request-bound Direct-close caller before fetching its vacant account.
///
/// The input must omit `caller_authority`. The semantic owner derives a candidate
/// from the authenticated request inputs, runs the complete close builder against
/// that derived vacant shape, and returns only the address needed for the caller's
/// same-slot account fetch. The subsequent full build still authenticates the real
/// fetched account.
pub fn preflight_direct_native_close_caller_v1(
    snapshot: &DirectNativeCloseSnapshotV1,
) -> Result<TerminalCallerPreflightV1, TerminalRetirementErrorV1> {
    if snapshot.caller_authority.is_some() {
        return Err(TerminalRetirementErrorV1::Frame);
    }
    let observation = close_preflight_observation(snapshot)?;
    let market =
        CoreState::decode(&snapshot.market.data).map_err(TerminalRetirementErrorV1::MarketCore)?;
    let manifest = CapabilityManifestV1::decode(&snapshot.manifest.data)
        .map_err(TerminalRetirementErrorV1::Capability)?;
    let header_bytes = snapshot
        .root
        .data
        .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
        .ok_or(TerminalRetirementErrorV1::Record)?;
    let header = CapabilityRootHeaderV1::decode(header_bytes)
        .map_err(TerminalRetirementErrorV1::CapabilityProgram)?;
    let selection = header.selection();
    let entry_index = selection.entry_index();
    let required_union = capability_dependency_closure_mask_v1(manifest, entry_index)
        .map_err(TerminalRetirementErrorV1::Capability)?;
    let wire_selection = CapabilityExecutionSelectionV1::new(
        entry_index,
        selection.manifest(),
        selection.kind(),
        selection.capability_release(),
        selection.config(),
    )
    .map_err(TerminalRetirementErrorV1::ReleaseSet)?;
    let funding_header = CapabilityFundingHeaderV2::new(
        u8::try_from(snapshot.funding_ledgers.len())
            .map_err(|_| TerminalRetirementErrorV1::Projection)?,
        u8::try_from(required_union.count_ones())
            .map_err(|_| TerminalRetirementErrorV1::Projection)?,
        required_union,
    )
    .map_err(TerminalRetirementErrorV1::MarketCore)?;
    let mut role_request = wire_selection.to_bytes().to_vec();
    role_request.extend_from_slice(&funding_header.encode());
    role_request.extend_from_slice(&direct_native_close_request_v1());
    let request_digest = hash(&role_request).to_bytes();
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        market.identity.selected_release_set.to_bytes(),
        snapshot.market.key.to_bytes(),
        ExecutionRoleV1::Core,
        snapshot.root.key.to_bytes(),
        request_digest,
    )
    .map_err(TerminalRetirementErrorV1::ReleaseSet)?;
    let caller_authority =
        Pubkey::find_program_address(&seeds.as_slices(), &snapshot.core_program.key).0;
    let mut complete = snapshot.clone();
    complete.caller_authority = Some(vacant_caller(observation, caller_authority));
    let report = build_direct_native_close_v1(&complete)?;
    if report.observation != observation
        || report.role_request_digest != request_digest
        || report.caller_authority != caller_authority
    {
        return Err(TerminalRetirementErrorV1::Projection);
    }
    Ok(TerminalCallerPreflightV1 {
        observation,
        request_digest,
        caller_authority,
    })
}

/// Build Core `CloseCapability` selecting Direct's exact `0xffff_ff01` entry.
pub fn build_direct_native_close_v1(
    snapshot: &DirectNativeCloseSnapshotV1,
) -> Result<DirectNativeCloseReportV1, TerminalRetirementErrorV1> {
    let observation = close_observation(snapshot)?;
    let market = authenticate_close_market(snapshot)?;
    authenticate_close_releases(snapshot, market)?;
    // The Rent sysvar is still AUTHENTICATED here -- key, owner, executable bit,
    // exact width, canonical body -- even though nothing prices a floor against
    // it any more. Dropping the decode with the floor would silently stop
    // checking the coordinate, which is the debt `a4b2cbb17` named at
    // `authenticate_execution_strategy_v2` and this does not repeat.
    decode_rent(&snapshot.rent_sysvar).map_err(TerminalRetirementErrorV1::Observation)?;
    authenticate_close_system(snapshot)?;
    authenticate_finalized_record(
        snapshot.registry_program.key,
        &snapshot.realm,
        &FinalizedRecordProof {
            schema_release_id: REALM_SCHEMA_RELEASE_ID_V1,
            staging_cursor: snapshot.realm_staging.clone(),
        },
    )
    .map_err(TerminalRetirementErrorV1::Observation)?;
    authenticate_finalized_record(
        snapshot.registry_program.key,
        &snapshot.manifest,
        &FinalizedRecordProof {
            schema_release_id: CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            staging_cursor: snapshot.manifest_staging.clone(),
        },
    )
    .map_err(TerminalRetirementErrorV1::Observation)?;
    if hash(&snapshot.realm.data).to_bytes() != market.identity.realm_id.to_bytes()
        || hash(&snapshot.manifest.data).to_bytes()
            != market.identity.capability_manifest.to_bytes()
    {
        return Err(TerminalRetirementErrorV1::Record);
    }
    let manifest = CapabilityManifestV1::decode(&snapshot.manifest.data)
        .map_err(TerminalRetirementErrorV1::Capability)?;
    let (root_header, root_state) = authenticate_close_root(snapshot, market, manifest)?;
    authenticate_close_records(snapshot, root_header, manifest)?;
    let selection = root_header.selection();
    let entry_index = selection.entry_index();
    let entry = manifest
        .entry(entry_index)
        .map_err(TerminalRetirementErrorV1::Capability)?;
    let selected_bit = 1_u16
        .checked_shl(u32::from(entry_index))
        .ok_or(TerminalRetirementErrorV1::Projection)?;
    let required_union = capability_dependency_closure_mask_v1(manifest, entry_index)
        .map_err(TerminalRetirementErrorV1::Capability)?;
    if manifest.entry_count() != 4
        || entry_index != 3
        || required_union != 0b1111
        || snapshot.funding_ledgers.len() != 2
    {
        return Err(TerminalRetirementErrorV1::Projection);
    }
    let funding =
        authenticate_close_funding(snapshot, market, manifest, selected_bit, required_union)?;
    if !funded_rent_persists_v1(snapshot.root.lamports) {
        return Err(TerminalRetirementErrorV1::Projection);
    }
    authenticate_close_rent_credit(snapshot, market)?;
    let wire_selection = CapabilityExecutionSelectionV1::new(
        entry_index,
        selection.manifest(),
        selection.kind(),
        selection.capability_release(),
        selection.config(),
    )
    .map_err(TerminalRetirementErrorV1::ReleaseSet)?;
    let physical_count = u8::try_from(snapshot.funding_ledgers.len())
        .map_err(|_| TerminalRetirementErrorV1::Projection)?;
    let logical_count = u8::try_from(required_union.count_ones())
        .map_err(|_| TerminalRetirementErrorV1::Projection)?;
    let funding_header =
        CapabilityFundingHeaderV2::new(physical_count, logical_count, required_union)
            .map_err(TerminalRetirementErrorV1::MarketCore)?;
    let family_request = direct_native_close_request_v1();
    let mut role_request = wire_selection.to_bytes().to_vec();
    role_request.extend_from_slice(&funding_header.encode());
    role_request.extend_from_slice(&family_request);
    let role_request_digest = hash(&role_request).to_bytes();
    let context = snapshot.root.key.to_bytes();
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        market.identity.selected_release_set.to_bytes(),
        snapshot.market.key.to_bytes(),
        ExecutionRoleV1::Core,
        context,
        role_request_digest,
    )
    .map_err(TerminalRetirementErrorV1::ReleaseSet)?;
    let caller_authority =
        Pubkey::find_program_address(&seeds.as_slices(), &snapshot.core_program.key).0;
    let observed_caller = snapshot
        .caller_authority
        .as_ref()
        .ok_or(TerminalRetirementErrorV1::Frame)?;
    if observed_caller.key != caller_authority
        || observed_caller.owner != system_program::ID
        || observed_caller.lamports != 0
        || observed_caller.executable
        || !observed_caller.data.is_empty()
    {
        return Err(TerminalRetirementErrorV1::Frame);
    }
    let envelope = CoreEffectEnvelopeV1::new(
        CoreEffectActionV1::CloseCapability,
        Role::Trading,
        dclutch_market::Identity::new(snapshot.core_program.key.to_bytes())
            .map_err(TerminalRetirementErrorV1::MarketCore)?,
        dclutch_market::Identity::new(caller_authority.to_bytes())
            .map_err(TerminalRetirementErrorV1::MarketCore)?,
        market.identity.selected_release_set,
        market.identity.market_id,
        dclutch_market::Identity::new(context)
            .map_err(TerminalRetirementErrorV1::MarketCore)?,
        dclutch_market::Identity::new(hash(&snapshot.market.data).to_bytes())
            .map_err(TerminalRetirementErrorV1::MarketCore)?,
        dclutch_market::Identity::new(role_request_digest)
            .map_err(TerminalRetirementErrorV1::MarketCore)?,
        market.identity.generation,
        0,
        0,
        u32::try_from(role_request.len()).map_err(|_| TerminalRetirementErrorV1::Projection)?,
    )
    .map_err(TerminalRetirementErrorV1::MarketCore)?;
    let request = Request::administrative(
        Action::CloseCapability,
        market.identity.generation,
        market.identity.market_id,
    );
    let mut data = request
        .encode()
        .map_err(TerminalRetirementErrorV1::MarketCore)?
        .to_vec();
    data.extend_from_slice(
        &envelope
            .encode()
            .map_err(TerminalRetirementErrorV1::MarketCore)?,
    );
    data.extend_from_slice(&role_request);
    let resolution_funding = snapshot
        .funding_ledgers
        .first()
        .ok_or(TerminalRetirementErrorV1::Frame)?
        .key;
    let trading_funding = snapshot
        .funding_ledgers
        .get(1)
        .ok_or(TerminalRetirementErrorV1::Frame)?
        .key;
    let coordinate_closure =
        project_direct_native_close_coordinate_closure_v1(&DirectNativeCloseCoordinateInputV1 {
            release_set: market.identity.selected_release_set.to_bytes(),
            role_request_digest,
            market: snapshot.market.key,
            realm: TerminalRecordCoordinatesV1 {
                raw: snapshot.realm.key,
                staging: snapshot.realm_staging.key,
            },
            manifest: TerminalRecordCoordinatesV1 {
                raw: snapshot.manifest.key,
                staging: snapshot.manifest_staging.key,
            },
            resolution_funding,
            trading_funding,
            root: snapshot.root.key,
            activation_cache: snapshot.activation_cache.key,
            core: TerminalDeploymentCoordinatesV1 {
                program: snapshot.core_program.key,
                programdata: snapshot.core_programdata.key,
            },
            trading: TerminalDeploymentCoordinatesV1 {
                program: snapshot.trading_program.key,
                programdata: snapshot.trading_programdata.key,
            },
            resolution: TerminalDeploymentCoordinatesV1 {
                program: snapshot.resolution_program.key,
                programdata: snapshot.resolution_programdata.key,
            },
            registry_program: snapshot.registry_program.key,
            rent_sysvar: snapshot.rent_sysvar.key,
            program_set: TerminalRecordCoordinatesV1 {
                raw: snapshot.program_set.key,
                staging: snapshot.program_set_staging.key,
            },
            config: TerminalRecordCoordinatesV1 {
                raw: snapshot.config.key,
                staging: snapshot.config_staging.key,
            },
            close_profile: TerminalRecordCoordinatesV1 {
                raw: snapshot.close_profile.key,
                staging: snapshot.close_profile_staging.key,
            },
            close_effect: TerminalRecordCoordinatesV1 {
                raw: snapshot.close_effect.key,
                staging: snapshot.close_effect_staging.key,
            },
            system_program: snapshot.system_program.key,
            close_descriptor: TerminalRecordCoordinatesV1 {
                raw: snapshot.close_descriptor.key,
                staging: snapshot.close_descriptor_staging.key,
            },
            rent_program: snapshot.rent_program.key,
            rent_credit: snapshot.rent_credit.key,
        })?;
    if coordinate_closure.program_id != snapshot.core_program.key
        || coordinate_closure
            .accounts
            .get(
                CapabilityRouteLayoutV1::new(2, 20)
                    .map_err(TerminalRetirementErrorV1::MarketCore)?
                    .caller_authority(),
            )
            .is_none_or(|meta| meta.pubkey != caller_authority)
    {
        return Err(TerminalRetirementErrorV1::Frame);
    }
    let accounts = coordinate_closure.accounts.clone();
    let mut post_market = market;
    post_market.outstanding_capabilities = post_market
        .outstanding_capabilities
        .checked_sub(1)
        .ok_or(TerminalRetirementErrorV1::Projection)?;
    let expected_market_data = post_market
        .encode()
        .map_err(TerminalRetirementErrorV1::MarketCore)?
        .to_vec();
    let selected_funding = snapshot
        .funding_ledgers
        .get(funding.selected_index)
        .ok_or(TerminalRetirementErrorV1::Projection)?;
    let rent_credit_delta_lamports = snapshot
        .root
        .lamports
        .checked_add(selected_funding.lamports)
        .ok_or(TerminalRetirementErrorV1::Projection)?;
    let expected_rent_credit_lamports = snapshot
        .rent_credit
        .lamports
        .checked_add(rent_credit_delta_lamports)
        .ok_or(TerminalRetirementErrorV1::Projection)?;
    let _ = (entry, root_state);
    Ok(DirectNativeCloseReportV1 {
        instruction: Instruction {
            program_id: snapshot.core_program.key,
            accounts,
            data,
        },
        meta_closure: coordinate_closure,
        observation,
        caller_authority,
        role_request_digest,
        selector: DIRECT_NATIVE_CLOSE_SELECTOR_V1,
        expected_market_digest: hash(&expected_market_data).to_bytes(),
        expected_market_data,
        expected_market_owner: snapshot.market.owner,
        expected_market_lamports: snapshot.market.lamports,
        expected_outstanding_capabilities: 0,
        closed_root: snapshot.root.key,
        root_refund_lamports: snapshot.root.lamports,
        expected_root_owner: system_program::ID,
        expected_root_data: Vec::new(),
        expected_root_lamports: 0,
        closed_funding_ledger: selected_funding.key,
        funding_refund_lamports: selected_funding.lamports,
        expected_funding_owner: system_program::ID,
        expected_funding_data: Vec::new(),
        expected_funding_lamports: 0,
        preserved_dependency_ledgers: funding.preserved_dependencies,
        rent_credit: snapshot.rent_credit.key,
        rent_credit_pre_lamports: snapshot.rent_credit.lamports,
        rent_credit_delta_lamports,
        expected_rent_credit_lamports,
        expected_rent_credit_owner: snapshot.rent_credit.owner,
        expected_rent_credit_data: snapshot.rent_credit.data.clone(),
    })
}

fn authenticate_close_system(
    snapshot: &DirectNativeCloseSnapshotV1,
) -> Result<(), TerminalRetirementErrorV1> {
    if snapshot.system_program.key != system_program::ID
        || snapshot.system_program.owner != native_loader::ID
        || !snapshot.system_program.executable
    {
        return Err(TerminalRetirementErrorV1::Record);
    }
    Ok(())
}

fn authenticate_close_market(
    snapshot: &DirectNativeCloseSnapshotV1,
) -> Result<CoreState, TerminalRetirementErrorV1> {
    let state =
        CoreState::decode(&snapshot.market.data).map_err(TerminalRetirementErrorV1::MarketCore)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        &snapshot.core_program.key,
    )
    .0;
    if snapshot.market.key != expected
        || snapshot.market.owner != snapshot.core_program.key
        || snapshot.market.executable
        || snapshot.market.key.to_bytes() != state.identity.market_id.to_bytes()
        || state.identity.registry_program.to_bytes() != snapshot.registry_program.key.to_bytes()
        || state.phase != Phase::Retiring
        || state.outstanding_capabilities != 1
        || state.rent_beneficiary.to_bytes() != snapshot.rent_credit.key.to_bytes()
    {
        return Err(TerminalRetirementErrorV1::Market);
    }
    Ok(state)
}

fn authenticate_close_releases(
    snapshot: &DirectNativeCloseSnapshotV1,
    market: CoreState,
) -> Result<(), TerminalRetirementErrorV1> {
    if snapshot.activation_cache.owner != snapshot.registry_program.key
        || snapshot.activation_cache.executable
        || snapshot.activation_cache.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || snapshot.registry_program.owner != bpf_loader_upgradeable::ID
        || !snapshot.registry_program.executable
    {
        return Err(TerminalRetirementErrorV1::Release);
    }
    ProgramV3View::parse(&snapshot.registry_program.data)
        .map_err(TerminalRetirementErrorV1::RegistrySvm)?;
    let view = ActivatedExecutionReleaseSetViewV1::decode(&snapshot.activation_cache.data)
        .map_err(TerminalRetirementErrorV1::Registry)?;
    let release_set = view
        .execution_release_set_id()
        .map_err(TerminalRetirementErrorV1::Registry)?;
    let expected = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set.as_bytes()],
        &snapshot.registry_program.key,
    )
    .0;
    if snapshot.activation_cache.key != expected
        || release_set.to_bytes() != market.identity.selected_release_set.to_bytes()
    {
        return Err(TerminalRetirementErrorV1::Release);
    }
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Core,
            &snapshot.core_program,
            &snapshot.core_programdata,
        ),
        (
            ExecutionRoleV1::Trading,
            &snapshot.trading_program,
            &snapshot.trading_programdata,
        ),
        (
            ExecutionRoleV1::Resolution,
            &snapshot.resolution_program,
            &snapshot.resolution_programdata,
        ),
    ] {
        authenticate_deployment(view, role, program, programdata)?;
    }
    Ok(())
}

fn authenticate_close_root(
    snapshot: &DirectNativeCloseSnapshotV1,
    market: CoreState,
    manifest: CapabilityManifestV1<'_>,
) -> Result<(CapabilityRootHeaderV1, DirectRootStateV1), TerminalRetirementErrorV1> {
    let (header_bytes, state_bytes) = snapshot
        .root
        .data
        .split_at_checked(CAPABILITY_ROOT_HEADER_BYTES_V1)
        .ok_or(TerminalRetirementErrorV1::Record)?;
    let header = CapabilityRootHeaderV1::decode(header_bytes)
        .map_err(TerminalRetirementErrorV1::CapabilityProgram)?;
    let state =
        DirectRootStateV1::decode(state_bytes).map_err(TerminalRetirementErrorV1::Successor)?;
    state
        .require_closable()
        .map_err(TerminalRetirementErrorV1::Successor)?;
    let selection = header.selection();
    let entry = manifest
        .entry(selection.entry_index())
        .map_err(TerminalRetirementErrorV1::Capability)?;
    let expected =
        Pubkey::find_program_address(&header.seeds().as_slices(), &snapshot.trading_program.key).0;
    if snapshot.root.key != expected
        || snapshot.root.owner != snapshot.trading_program.key
        || snapshot.root.executable
        || header.release_set().to_bytes() != market.identity.selected_release_set.to_bytes()
        || header.market() != snapshot.market.key.to_bytes()
        || header.generation() != market.identity.generation
        || selection.manifest().to_bytes() != market.identity.capability_manifest.to_bytes()
        || selection.kind() != entry.kind_id()
        || selection.capability_release() != entry.release_id()
        || selection.config() != entry.config_id()
        || state.phase() != DirectRootPhaseV1::Retiring
        || state.open_maker_root_count() != 0
    {
        return Err(TerminalRetirementErrorV1::Record);
    }
    Ok((header, state))
}

fn authenticate_close_records(
    snapshot: &DirectNativeCloseSnapshotV1,
    header: CapabilityRootHeaderV1,
    manifest: CapabilityManifestV1<'_>,
) -> Result<(), TerminalRetirementErrorV1> {
    let selection = header.selection();
    authenticate_record(
        snapshot,
        &snapshot.program_set,
        &snapshot.program_set_staging,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        selection.capability_release().to_bytes(),
    )?;
    let set = CapabilityProgramSetV2::decode_selected(
        selection.capability_release().to_bytes(),
        hash(&snapshot.program_set.data).to_bytes(),
        &snapshot.program_set.data,
    )
    .map_err(TerminalRetirementErrorV1::ProgramSet)?;
    let selected = set
        .select_descriptor(&direct_native_close_request_v1())
        .map_err(TerminalRetirementErrorV1::ProgramSet)?;
    authenticate_record(
        snapshot,
        &snapshot.close_descriptor,
        &snapshot.close_descriptor_staging,
        CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
        selected.program().to_bytes(),
    )?;
    if selected.schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1 {
        return Err(TerminalRetirementErrorV1::Record);
    }
    let descriptor = CapabilityProgramV1::decode(&snapshot.close_descriptor.data)
        .map_err(TerminalRetirementErrorV1::CapabilityProgram)?;
    let entry = manifest
        .entry(selection.entry_index())
        .map_err(TerminalRetirementErrorV1::Capability)?;
    authenticate_record(
        snapshot,
        &snapshot.config,
        &snapshot.config_staging,
        descriptor.config_schema().to_bytes(),
        selection.config().to_bytes(),
    )?;
    authenticate_record(
        snapshot,
        &snapshot.close_profile,
        &snapshot.close_profile_staging,
        direct_native_close_account_profile_schema_v1(),
        descriptor.account_profile().to_bytes(),
    )?;
    authenticate_record(
        snapshot,
        &snapshot.close_effect,
        &snapshot.close_effect_staging,
        direct_native_close_effect_schema_v1(),
        descriptor.effect_schema().to_bytes(),
    )?;
    AccountProfileV1::decode_selected(
        descriptor.account_profile().to_bytes(),
        hash(&snapshot.close_profile.data).to_bytes(),
        &snapshot.close_profile.data,
    )
    .map_err(TerminalRetirementErrorV1::AccountProfile)?;
    EffectProgramV2::decode(&snapshot.close_effect.data)
        .map_err(TerminalRetirementErrorV1::EffectV2)?;
    if descriptor.kind() != entry.kind_id()
        || descriptor.config_schema().to_bytes() == [0; 32]
        || descriptor.request_schema().to_bytes()
            != dclutch_trading::native_close_bundle_v1::DIRECT_NATIVE_CLOSE_REQUEST_SCHEMA_ID_V1
        || descriptor.root_schema() != entry.child_schema_id()
        || descriptor.capacity_profile() != entry.capacity_profile_id()
        || descriptor.derivation_policy() != entry.child_derivation_id()
        || descriptor.root_state_bytes()
            != u32::try_from(snapshot.root.data.len() - CAPABILITY_ROOT_HEADER_BYTES_V1)
                .map_err(|_| TerminalRetirementErrorV1::Record)?
    {
        return Err(TerminalRetirementErrorV1::Record);
    }
    Ok(())
}

fn authenticate_record(
    snapshot: &DirectNativeCloseSnapshotV1,
    record: &ObservedAccount,
    staging: &ObservedAccount,
    schema: [u8; 32],
    expected_digest: [u8; 32],
) -> Result<(), TerminalRetirementErrorV1> {
    authenticate_finalized_record(
        snapshot.registry_program.key,
        record,
        &FinalizedRecordProof {
            schema_release_id: schema,
            staging_cursor: staging.clone(),
        },
    )
    .map_err(TerminalRetirementErrorV1::Observation)?;
    if hash(&record.data).to_bytes() != expected_digest {
        return Err(TerminalRetirementErrorV1::Record);
    }
    Ok(())
}

struct CloseFundingProjectionV1 {
    selected_index: usize,
    preserved_dependencies: Vec<PreservedFundingLedgerV1>,
}

#[allow(clippy::too_many_arguments)]
fn authenticate_close_funding(
    snapshot: &DirectNativeCloseSnapshotV1,
    market: CoreState,
    manifest: CapabilityManifestV1<'_>,
    selected_bit: u16,
    required_union: u16,
) -> Result<CloseFundingProjectionV1, TerminalRetirementErrorV1> {
    let manifest_id = content_id(market.identity.capability_manifest.to_bytes())?;
    let selected_entry_index = u16::try_from(selected_bit.trailing_zeros())
        .map_err(|_| TerminalRetirementErrorV1::Projection)?;
    let mut masks = Vec::with_capacity(snapshot.funding_ledgers.len());
    let mut selected_index = None;
    let mut preserved_dependencies = Vec::new();
    for (index, account) in snapshot.funding_ledgers.iter().enumerate() {
        let ledger = FundingLedgerV2::decode(&account.data)
            .map_err(TerminalRetirementErrorV1::Capability)?;
        let authenticated = ledger
            .authenticate(manifest_id, manifest)
            .map_err(TerminalRetirementErrorV1::Capability)?;
        let mask = ledger.selected_mask();
        let selected = mask & selected_bit != 0;
        let controller = if selected {
            if mask != selected_bit || selected_index.replace(index).is_some() {
                return Err(TerminalRetirementErrorV1::Projection);
            }
            snapshot.trading_program.key
        } else {
            snapshot.resolution_program.key
        };
        let derivation = CapabilityFundingLedgerDerivationV2::new(
            controller.to_bytes(),
            snapshot.market.key.to_bytes(),
            market.identity.generation,
            manifest_id,
            ledger,
        )
        .map_err(TerminalRetirementErrorV1::Capability)?;
        let expected = Pubkey::find_program_address(&derivation.seed_components(), &controller).0;
        let exact_rent = authenticated
            .funded_rent_minimum(account.data.len())
            .map_err(TerminalRetirementErrorV1::Capability)?;
        if account.key != expected
            || account.owner != controller
            || account.executable
            || account.lamports < exact_rent
        {
            return Err(TerminalRetirementErrorV1::Projection);
        }
        authenticated
            .validate_native_custody(account.lamports, exact_rent, selected)
            .map_err(TerminalRetirementErrorV1::Capability)?;
        let mut row_index = 0_u16;
        while row_index < ledger.slot_count() {
            let entry_index = manifest_entry_for_ledger_row_v2(mask, row_index)
                .map_err(TerminalRetirementErrorV1::Capability)?;
            let slot = authenticated
                .slot(entry_index)
                .map_err(TerminalRetirementErrorV1::Capability)?;
            if !FUNDING_LEDGER_ACTIVE_ADMISSIBLE_STATES_V2.admits(slot.status()) {
                return Err(TerminalRetirementErrorV1::Projection);
            }
            row_index = row_index
                .checked_add(1)
                .ok_or(TerminalRetirementErrorV1::Projection)?;
        }
        if selected {
            let mut projected = account.data.clone();
            let close = FundingLedgerV2::close_slot_in_place(
                &mut projected,
                manifest_id,
                manifest,
                selected_entry_index,
                FundingLedgerCloseCustodyV2::native_only(
                    account.lamports,
                    exact_rent,
                    snapshot.rent_credit.key.to_bytes(),
                )
                .map_err(TerminalRetirementErrorV1::Capability)?,
            )
            .map_err(TerminalRetirementErrorV1::Capability)?;
            if !close.ledger_can_close()
                || close.expected_post_ledger_lamports() != 0
                || close.remaining_realm_collateral() != 0
            {
                return Err(TerminalRetirementErrorV1::Projection);
            }
        } else {
            preserved_dependencies.push(PreservedFundingLedgerV1 {
                key: account.key,
                owner: account.owner,
                data: account.data.clone(),
                data_digest: hash(&account.data).to_bytes(),
                lamports: account.lamports,
                executable: account.executable,
            });
        }
        masks.push(mask);
    }
    validate_funding_ledger_masks_v2(manifest.entry_count(), required_union, &masks)
        .map_err(TerminalRetirementErrorV1::Capability)?;
    let selected_index = selected_index.ok_or(TerminalRetirementErrorV1::Projection)?;
    if masks.get(selected_index).copied() != Some(selected_bit) {
        return Err(TerminalRetirementErrorV1::Projection);
    }
    Ok(CloseFundingProjectionV1 {
        selected_index,
        preserved_dependencies,
    })
}

fn authenticate_close_rent_credit(
    snapshot: &DirectNativeCloseSnapshotV1,
    market: CoreState,
) -> Result<(), TerminalRetirementErrorV1> {
    let credit = LifecycleRentCreditV2::decode(&snapshot.rent_credit.data)
        .map_err(TerminalRetirementErrorV1::LifecycleRent)?;
    let seeds = credit.pda_seeds();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            seeds.market().to_bytes().as_slice(),
            seeds.generation().as_slice(),
            &bump,
        ],
        &snapshot.rent_program.key,
    )
    .map_err(|_| TerminalRetirementErrorV1::Projection)?;
    if snapshot.rent_program.owner != bpf_loader_upgradeable::ID
        || !snapshot.rent_program.executable
        || snapshot.rent_credit.owner != snapshot.rent_program.key
        || snapshot.rent_credit.key != expected
        || snapshot.rent_credit.key.to_bytes() != market.rent_beneficiary.to_bytes()
        || credit.market().to_bytes() != snapshot.market.key.to_bytes()
        || credit.release_set().to_bytes() != market.identity.selected_release_set.to_bytes()
        || credit.generation() != market.identity.generation
    {
        return Err(TerminalRetirementErrorV1::Projection);
    }
    Ok(())
}

fn close_observation(
    snapshot: &DirectNativeCloseSnapshotV1,
) -> Result<Observation, TerminalRetirementErrorV1> {
    if snapshot.caller_authority.is_none() {
        return Err(TerminalRetirementErrorV1::Frame);
    }
    let accounts = close_unique_accounts(snapshot);
    observation_and_distinct(&accounts)
}

fn close_preflight_observation(
    snapshot: &DirectNativeCloseSnapshotV1,
) -> Result<Observation, TerminalRetirementErrorV1> {
    let mut without_caller = snapshot.clone();
    without_caller.caller_authority = None;
    let accounts = close_unique_accounts(&without_caller);
    observation_and_distinct(&accounts)
}

fn observation_and_distinct(
    accounts: &[&ObservedAccount],
) -> Result<Observation, TerminalRetirementErrorV1> {
    let observation = accounts
        .first()
        .ok_or(TerminalRetirementErrorV1::Snapshot)?
        .observation;
    if observation.finality != Finality::Finalized
        || accounts
            .iter()
            .any(|account| account.observation != observation)
    {
        return Err(TerminalRetirementErrorV1::Snapshot);
    }
    for (left, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(left.saturating_add(1))
            .any(|other| account.key == other.key)
        {
            return Err(TerminalRetirementErrorV1::Alias);
        }
    }
    Ok(observation)
}

fn close_unique_accounts(snapshot: &DirectNativeCloseSnapshotV1) -> Vec<&ObservedAccount> {
    let mut accounts = vec![
        &snapshot.market,
        &snapshot.realm,
        &snapshot.realm_staging,
        &snapshot.manifest,
        &snapshot.manifest_staging,
    ];
    accounts.extend(snapshot.funding_ledgers.iter());
    accounts.extend([
        &snapshot.root,
        &snapshot.activation_cache,
        &snapshot.core_program,
        &snapshot.core_programdata,
        &snapshot.trading_program,
        &snapshot.trading_programdata,
        &snapshot.resolution_program,
        &snapshot.resolution_programdata,
        &snapshot.registry_program,
        &snapshot.rent_sysvar,
    ]);
    accounts.extend(snapshot.caller_authority.iter());
    accounts.extend([
        &snapshot.program_set,
        &snapshot.program_set_staging,
        &snapshot.config,
        &snapshot.config_staging,
        &snapshot.close_profile,
        &snapshot.close_profile_staging,
        &snapshot.close_effect,
        &snapshot.close_effect_staging,
        &snapshot.system_program,
        &snapshot.close_descriptor,
        &snapshot.close_descriptor_staging,
        &snapshot.rent_program,
        &snapshot.rent_credit,
    ]);
    accounts
}

fn exact_close_aliases(accounts: &[AccountMeta], layout: CapabilityRouteLayoutV1) -> bool {
    let pairs = layout.close_alias_pairs();
    pairs.iter().all(|(left, right)| {
        accounts
            .get(*left)
            .zip(accounts.get(*right))
            .is_some_and(|(left, right)| left.pubkey == right.pubkey)
    }) && accounts.iter().enumerate().all(|(left_index, left)| {
        accounts
            .iter()
            .enumerate()
            .skip(left_index.saturating_add(1))
            .all(|(right_index, right)| {
                left.pubkey != right.pubkey || pairs.contains(&(left_index, right_index))
            })
    })
}

fn content_id(bytes: [u8; 32]) -> Result<ContentId, TerminalRetirementErrorV1> {
    ContentId::new(bytes).map_err(|_| TerminalRetirementErrorV1::Projection)
}

/// Build the one atomic Trading-to-Core retirement replay handoff.
pub fn build_retirement_replay_handoff_v1(
    snapshot: &RetirementReplayHandoffSnapshotV1,
) -> Result<RetirementReplayHandoffReportV1, TerminalRetirementErrorV1> {
    let observation = handoff_observation(snapshot)?;
    require_handoff_distinct(snapshot)?;
    let market = authenticate_handoff_market(snapshot)?;
    let activation = authenticate_handoff_releases(snapshot, market)?;
    let claims_program = activation
        .role(ExecutionRoleV1::Claims)
        .map_err(TerminalRetirementErrorV1::Registry)?
        .release()
        .program()
        .to_bytes();
    let context = authenticate_handoff_records(snapshot, market, claims_program)?;
    let replay = authenticate_handoff_custody(snapshot, market, context)?;
    let rent =
        decode_rent(&snapshot.rent_sysvar).map_err(TerminalRetirementErrorV1::Observation)?;
    let core_rent = rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1);
    let request = RetirementReplayHandoffRequestV1::new(
        snapshot.market.key.to_bytes(),
        context,
        hash(&snapshot.trading_replay.data).to_bytes(),
        hash(&snapshot.hoard.data).to_bytes(),
        market.identity.generation,
        replay.next_revision,
        snapshot.trading_replay.lamports,
        core_rent,
        snapshot.hoard.lamports,
        snapshot.rent_credit.lamports,
        snapshot.payer.lamports,
    )
    .map_err(TerminalRetirementErrorV1::RetirementReplayHandoff)?;
    let request_body = request.to_bytes();
    let request_digest = hash(&request_body).to_bytes();
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        market.identity.selected_release_set.to_bytes(),
        snapshot.market.key.to_bytes(),
        ExecutionRoleV1::Core,
        context,
        request_digest,
    )
    .map_err(TerminalRetirementErrorV1::ReleaseSet)?;
    let caller_authority =
        Pubkey::find_program_address(&seeds.as_slices(), &snapshot.core_program.key).0;
    let observed_caller = snapshot
        .caller_authority
        .as_ref()
        .ok_or(TerminalRetirementErrorV1::Frame)?;
    if observed_caller.key != caller_authority
        || observed_caller.owner != system_program::ID
        || observed_caller.lamports != 0
        || observed_caller.executable
        || !observed_caller.data.is_empty()
    {
        return Err(TerminalRetirementErrorV1::Frame);
    }
    let projected = CustodyReplayV1 {
        caller_role: ExecutionRoleV1::Core,
        caller_program: snapshot.core_program.key.to_bytes(),
        ..replay
    };
    let expected_core_replay_data = projected
        .to_bytes()
        .map_err(TerminalRetirementErrorV1::CustodyContract)?
        .to_vec();
    let expected_core_replay_digest = hash(&expected_core_replay_data).to_bytes();
    let plan = RetirementReplayHandoffPlanV1::new(
        request,
        request_digest,
        RetirementReplayHandoffObservationV1 {
            core_program: snapshot.core_program.key.to_bytes(),
            trading_program: snapshot.trading_program.key.to_bytes(),
            trading_replay: snapshot.trading_replay.key.to_bytes(),
            core_replay: snapshot.core_replay.key.to_bytes(),
            hoard_vault: snapshot.hoard.key.to_bytes(),
            rent_credit: snapshot.rent_credit.key.to_bytes(),
            replay,
            trading_replay_digest: hash(&snapshot.trading_replay.data).to_bytes(),
            hoard_data_digest: hash(&snapshot.hoard.data).to_bytes(),
            trading_replay_lamports: snapshot.trading_replay.lamports,
            core_replay_lamports: snapshot.core_replay.lamports,
            hoard_lamports: snapshot.hoard.lamports,
            rent_credit_lamports: snapshot.rent_credit.lamports,
            payer_lamports: snapshot.payer.lamports,
        },
        expected_core_replay_digest,
    )
    .map_err(TerminalRetirementErrorV1::RetirementReplayHandoff)?;
    let expected_receipt = plan.receipt();
    let coordinate_closure = project_retirement_replay_handoff_coordinate_closure_v1(
        &RetirementReplayHandoffCoordinateInputV1 {
            release_set: market.identity.selected_release_set.to_bytes(),
            context,
            request_digest,
            payer: snapshot.payer.key,
            market: snapshot.market.key,
            activation_cache: snapshot.activation_cache.key,
            registry_program: snapshot.registry_program.key,
            core: TerminalDeploymentCoordinatesV1 {
                program: snapshot.core_program.key,
                programdata: snapshot.core_programdata.key,
            },
            trading: TerminalDeploymentCoordinatesV1 {
                program: snapshot.trading_program.key,
                programdata: snapshot.trading_programdata.key,
            },
            custody: TerminalDeploymentCoordinatesV1 {
                program: snapshot.custody_program.key,
                programdata: snapshot.custody_programdata.key,
            },
            claims_aggregate: snapshot.claims_aggregate.key,
            realm: TerminalRecordCoordinatesV1 {
                raw: snapshot.realm.key,
                staging: snapshot.realm_staging.key,
            },
            rent_sysvar: snapshot.rent_sysvar.key,
            rent_credit: snapshot.rent_credit.key,
            trading_replay: snapshot.trading_replay.key,
            core_replay: snapshot.core_replay.key,
            hoard: snapshot.hoard.key,
            system_program: snapshot.system_program.key,
            mint: snapshot.mint.key,
            token_program: snapshot.token_program.key,
            custody_authority: snapshot.custody_authority.key,
        },
    )?;
    if coordinate_closure.program_id != snapshot.core_program.key
        || coordinate_closure
            .accounts
            .get(RetirementReplayHandoffAccountLayoutV1::CALLER_AUTHORITY)
            .is_none_or(|meta| meta.pubkey != caller_authority)
    {
        return Err(TerminalRetirementErrorV1::Frame);
    }
    let instruction = Instruction {
        program_id: coordinate_closure.program_id,
        accounts: coordinate_closure.accounts.clone(),
        data: request_body.to_vec(),
    };
    Ok(RetirementReplayHandoffReportV1 {
        instruction,
        meta_closure: coordinate_closure,
        observation,
        request_body,
        request_digest,
        caller_authority,
        expected_receipt_body: expected_receipt.to_bytes(),
        expected_receipt,
        trading_replay: snapshot.trading_replay.key,
        core_replay: snapshot.core_replay.key,
        expected_core_replay_data,
        expected_core_replay_digest,
        expected_core_replay_owner: snapshot.custody_program.key,
        expected_core_replay_lamports: core_rent,
        expected_core_replay_executable: false,
        trading_replay_refund_lamports: snapshot.trading_replay.lamports,
        expected_trading_replay_owner: system_program::ID,
        expected_trading_replay_data: Vec::new(),
        expected_trading_replay_lamports: 0,
        expected_payer_owner: snapshot.payer.owner,
        expected_payer_data: snapshot.payer.data.clone(),
        expected_payer_lamports: expected_receipt.payer_post_lamports,
        expected_rent_credit_owner: snapshot.rent_credit.owner,
        expected_rent_credit_data: snapshot.rent_credit.data.clone(),
        expected_rent_credit_lamports: expected_receipt.rent_credit_post_lamports,
        hoard: snapshot.hoard.key,
        expected_hoard_data: snapshot.hoard.data.clone(),
        expected_hoard_owner: snapshot.hoard.owner,
        expected_hoard_lamports: snapshot.hoard.lamports,
    })
}

/// Discover the request-bound replay-handoff caller before fetching its vacant account.
///
/// The input must omit `caller_authority`. The complete handoff builder authenticates
/// every other observed fact against a semantic-owner-derived vacant caller shape;
/// the later full build still authenticates the real same-slot fetched account.
pub fn preflight_retirement_replay_handoff_caller_v1(
    snapshot: &RetirementReplayHandoffSnapshotV1,
) -> Result<TerminalCallerPreflightV1, TerminalRetirementErrorV1> {
    if snapshot.caller_authority.is_some() {
        return Err(TerminalRetirementErrorV1::Frame);
    }
    let observation = handoff_preflight_observation(snapshot)?;
    let market =
        CoreState::decode(&snapshot.market.data).map_err(TerminalRetirementErrorV1::MarketCore)?;
    let aggregate = LiabilityBasisMarketViewV2::decode(&snapshot.claims_aggregate.data)
        .map_err(TerminalRetirementErrorV1::LiabilityBasisState)?;
    let context = aggregate.custody_context;
    let replay = CustodyReplayV1::decode(&snapshot.trading_replay.data)
        .map_err(TerminalRetirementErrorV1::CustodyContract)?;
    let rent =
        decode_rent(&snapshot.rent_sysvar).map_err(TerminalRetirementErrorV1::Observation)?;
    let request = RetirementReplayHandoffRequestV1::new(
        snapshot.market.key.to_bytes(),
        context,
        hash(&snapshot.trading_replay.data).to_bytes(),
        hash(&snapshot.hoard.data).to_bytes(),
        market.identity.generation,
        replay.next_revision,
        snapshot.trading_replay.lamports,
        rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1),
        snapshot.hoard.lamports,
        snapshot.rent_credit.lamports,
        snapshot.payer.lamports,
    )
    .map_err(TerminalRetirementErrorV1::RetirementReplayHandoff)?;
    let request_digest = hash(&request.to_bytes()).to_bytes();
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        market.identity.selected_release_set.to_bytes(),
        snapshot.market.key.to_bytes(),
        ExecutionRoleV1::Core,
        context,
        request_digest,
    )
    .map_err(TerminalRetirementErrorV1::ReleaseSet)?;
    let caller_authority =
        Pubkey::find_program_address(&seeds.as_slices(), &snapshot.core_program.key).0;
    let mut complete = snapshot.clone();
    complete.caller_authority = Some(vacant_caller(observation, caller_authority));
    let report = build_retirement_replay_handoff_v1(&complete)?;
    if report.observation != observation
        || report.request_digest != request_digest
        || report.caller_authority != caller_authority
    {
        return Err(TerminalRetirementErrorV1::Projection);
    }
    Ok(TerminalCallerPreflightV1 {
        observation,
        request_digest,
        caller_authority,
    })
}

fn vacant_caller(observation: Observation, key: Pubkey) -> ObservedAccount {
    ObservedAccount {
        observation,
        key,
        owner: system_program::ID,
        lamports: 0,
        executable: false,
        data: Vec::new(),
    }
}

fn authenticate_handoff_market(
    snapshot: &RetirementReplayHandoffSnapshotV1,
) -> Result<CoreState, TerminalRetirementErrorV1> {
    let state =
        CoreState::decode(&snapshot.market.data).map_err(TerminalRetirementErrorV1::MarketCore)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        &snapshot.core_program.key,
    )
    .0;
    if snapshot.market.key != expected
        || snapshot.market.owner != snapshot.core_program.key
        || snapshot.market.executable
        || snapshot.market.key.to_bytes() != state.identity.market_id.to_bytes()
        || state.identity.registry_program.to_bytes() != snapshot.registry_program.key.to_bytes()
        || state.phase != Phase::Retiring
    {
        return Err(TerminalRetirementErrorV1::Market);
    }
    Ok(state)
}

fn authenticate_handoff_releases<'a>(
    snapshot: &'a RetirementReplayHandoffSnapshotV1,
    market: CoreState,
) -> Result<ActivatedExecutionReleaseSetViewV1<'a>, TerminalRetirementErrorV1> {
    if snapshot.activation_cache.owner != snapshot.registry_program.key
        || snapshot.activation_cache.executable
        || snapshot.activation_cache.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || snapshot.registry_program.owner != bpf_loader_upgradeable::ID
        || !snapshot.registry_program.executable
    {
        return Err(TerminalRetirementErrorV1::Release);
    }
    ProgramV3View::parse(&snapshot.registry_program.data)
        .map_err(TerminalRetirementErrorV1::RegistrySvm)?;
    let view = ActivatedExecutionReleaseSetViewV1::decode(&snapshot.activation_cache.data)
        .map_err(TerminalRetirementErrorV1::Registry)?;
    let release_set = view
        .execution_release_set_id()
        .map_err(TerminalRetirementErrorV1::Registry)?;
    let expected = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set.as_bytes()],
        &snapshot.registry_program.key,
    )
    .0;
    if expected != snapshot.activation_cache.key
        || release_set.to_bytes() != market.identity.selected_release_set.to_bytes()
    {
        return Err(TerminalRetirementErrorV1::Release);
    }
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Core,
            &snapshot.core_program,
            &snapshot.core_programdata,
        ),
        (
            ExecutionRoleV1::Trading,
            &snapshot.trading_program,
            &snapshot.trading_programdata,
        ),
        (
            ExecutionRoleV1::Custody,
            &snapshot.custody_program,
            &snapshot.custody_programdata,
        ),
    ] {
        authenticate_deployment(view, role, program, programdata)?;
    }
    Ok(view)
}

fn authenticate_handoff_records(
    snapshot: &RetirementReplayHandoffSnapshotV1,
    market: CoreState,
    claims_program: [u8; 32],
) -> Result<[u8; 32], TerminalRetirementErrorV1> {
    let claims_program = Pubkey::new_from_array(claims_program);
    let aggregate = LiabilityBasisMarketViewV2::decode(&snapshot.claims_aggregate.data)
        .map_err(TerminalRetirementErrorV1::LiabilityBasisState)?;
    let expected = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, snapshot.market.key.as_ref()],
        &claims_program,
    )
    .0;
    if snapshot.claims_aggregate.key != expected
        || snapshot.claims_aggregate.owner != claims_program
        || snapshot.claims_aggregate.executable
        || aggregate.logical_market != snapshot.market.key.to_bytes()
        || aggregate.release_set != market.identity.selected_release_set.to_bytes()
        || aggregate.registry_program != snapshot.registry_program.key.to_bytes()
        || aggregate.product_instance_id != market.identity.product_id.to_bytes()
        || aggregate.realm_id != market.identity.realm_id.to_bytes()
        || aggregate.generation != market.identity.generation
    {
        return Err(TerminalRetirementErrorV1::Record);
    }
    authenticate_finalized_record(
        snapshot.registry_program.key,
        &snapshot.realm,
        &FinalizedRecordProof {
            schema_release_id: REALM_SCHEMA_RELEASE_ID_V1,
            staging_cursor: snapshot.realm_staging.clone(),
        },
    )
    .map_err(TerminalRetirementErrorV1::Observation)?;
    if hash(&snapshot.realm.data).to_bytes() != market.identity.realm_id.to_bytes() {
        return Err(TerminalRetirementErrorV1::Record);
    }
    Ok(aggregate.custody_context)
}

fn authenticate_handoff_custody(
    snapshot: &RetirementReplayHandoffSnapshotV1,
    market: CoreState,
    context: [u8; 32],
) -> Result<CustodyReplayV1, TerminalRetirementErrorV1> {
    if snapshot.system_program.key != system_program::ID
        || snapshot.system_program.owner != native_loader::ID
        || !snapshot.system_program.executable
        || snapshot.core_replay.owner != system_program::ID
        || snapshot.core_replay.lamports != 0
        || snapshot.core_replay.executable
        || !snapshot.core_replay.data.is_empty()
        || snapshot.payer.owner != system_program::ID
        || snapshot.payer.executable
        || !snapshot.payer.data.is_empty()
        || snapshot.payer.lamports == 0
    {
        return Err(TerminalRetirementErrorV1::Custody);
    }
    let release = market.identity.selected_release_set.to_bytes();
    let trading_seeds = CustodyReplaySeedsV1::new(
        snapshot.market.key.to_bytes(),
        release,
        ExecutionRoleV1::Trading,
        context,
    );
    let core_seeds = CustodyReplaySeedsV1::new(
        snapshot.market.key.to_bytes(),
        release,
        ExecutionRoleV1::Core,
        context,
    );
    let hoard_seeds = CustodyVaultSeedsV1::new(
        snapshot.market.key.to_bytes(),
        release,
        context,
        CompartmentV1::HoardPrincipal,
    );
    let authority_seeds = CustodyAuthoritySeedsV1::new(snapshot.market.key.to_bytes(), release);
    if snapshot.trading_replay.key
        != Pubkey::find_program_address(&trading_seeds.as_slices(), &snapshot.custody_program.key).0
        || snapshot.trading_replay.owner != snapshot.custody_program.key
        || snapshot.trading_replay.executable
        || snapshot.core_replay.key
            != Pubkey::find_program_address(&core_seeds.as_slices(), &snapshot.custody_program.key)
                .0
        || snapshot.hoard.key
            != Pubkey::find_program_address(&hoard_seeds.as_slices(), &snapshot.custody_program.key)
                .0
        || snapshot.custody_authority.key
            != Pubkey::find_program_address(
                &authority_seeds.as_slices(),
                &snapshot.custody_program.key,
            )
            .0
    {
        return Err(TerminalRetirementErrorV1::Custody);
    }
    let replay = CustodyReplayV1::decode(&snapshot.trading_replay.data)
        .map_err(TerminalRetirementErrorV1::CustodyContract)?;
    if replay.caller_role != ExecutionRoleV1::Trading
        || replay.release_set != release
        || replay.market != snapshot.market.key.to_bytes()
        || replay.realm != market.identity.realm_id.to_bytes()
        || replay.context != context
        || replay.caller_program != snapshot.trading_program.key.to_bytes()
        || replay.rent_refund != snapshot.rent_credit.key.to_bytes()
        || replay.open_vault_count != 1
        || replay.generation != market.identity.generation
    {
        return Err(TerminalRetirementErrorV1::Custody);
    }
    let credit = LifecycleRentCreditV2::decode(&snapshot.rent_credit.data)
        .map_err(TerminalRetirementErrorV1::LifecycleRent)?;
    let credit_seeds = credit.pda_seeds();
    let bump = [credit_seeds.bump()];
    let expected_credit = Pubkey::create_program_address(
        &[
            credit_seeds.domain(),
            credit_seeds.market().to_bytes().as_slice(),
            credit_seeds.generation().as_slice(),
            &bump,
        ],
        &snapshot.rent_credit.owner,
    )
    .map_err(|_| TerminalRetirementErrorV1::Custody)?;
    if expected_credit != snapshot.rent_credit.key
        || snapshot.rent_credit.executable
        || snapshot.rent_credit.key.to_bytes() != market.rent_beneficiary.to_bytes()
        || credit.market().to_bytes() != snapshot.market.key.to_bytes()
        || credit.release_set().to_bytes() != release
        || credit.generation() != market.identity.generation
    {
        return Err(TerminalRetirementErrorV1::Custody);
    }
    let realm = RealmV1::decode(&snapshot.realm.data).map_err(TerminalRetirementErrorV1::Realm)?;
    let token =
        TokenAccount::parse(&snapshot.hoard.data).map_err(TerminalRetirementErrorV1::Token)?;
    if snapshot.token_program.key.to_bytes() != *realm.token_program()
        || !snapshot.token_program.executable
        || snapshot.mint.key.to_bytes() != *realm.collateral_mint()
        || snapshot.mint.owner != snapshot.token_program.key
        || snapshot.mint.executable
        || snapshot.custody_authority.executable
        || snapshot.hoard.owner != snapshot.token_program.key
        || snapshot.hoard.executable
        || token.mint != snapshot.mint.key.to_bytes()
        || token.owner != snapshot.custody_authority.key.to_bytes()
        || token.state != AccountState::Initialized
        || token.delegate != COption::None
        || token.native_reserve != COption::None
        || token.close_authority != COption::None
        || token.delegated_amount != 0
    {
        return Err(TerminalRetirementErrorV1::Custody);
    }
    Ok(replay)
}

fn authenticate_deployment(
    view: ActivatedExecutionReleaseSetViewV1<'_>,
    role: ExecutionRoleV1,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<(), TerminalRetirementErrorV1> {
    let activated = view
        .role(role)
        .map_err(TerminalRetirementErrorV1::Registry)?;
    let release = activated.release();
    let observation = deployment_observation(program, programdata, release)?;
    activated
        .authenticate_current_deployment(observation)
        .map_err(TerminalRetirementErrorV1::Registry)
}

fn deployment_observation(
    program: &ObservedAccount,
    programdata: &ObservedAccount,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1, TerminalRetirementErrorV1> {
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.program().to_bytes() != program.key.to_bytes()
        || release.programdata() != programdata.key.to_bytes()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(TerminalRetirementErrorV1::Release);
    }
    let program_view =
        ProgramV3View::parse(&program.data).map_err(TerminalRetirementErrorV1::RegistrySvm)?;
    let data = ProgramDataV3View::parse(&programdata.data)
        .map_err(TerminalRetirementErrorV1::RegistrySvm)?;
    let derived =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != programdata.key.to_bytes() || programdata.key != derived {
        return Err(TerminalRetirementErrorV1::Release);
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
    .map_err(TerminalRetirementErrorV1::Registry)
}

fn handoff_observation(
    snapshot: &RetirementReplayHandoffSnapshotV1,
) -> Result<Observation, TerminalRetirementErrorV1> {
    if snapshot.caller_authority.is_none() {
        return Err(TerminalRetirementErrorV1::Frame);
    }
    let accounts = handoff_accounts(snapshot);
    observation_and_distinct(&accounts)
}

fn handoff_preflight_observation(
    snapshot: &RetirementReplayHandoffSnapshotV1,
) -> Result<Observation, TerminalRetirementErrorV1> {
    let mut without_caller = snapshot.clone();
    without_caller.caller_authority = None;
    let accounts = handoff_accounts(&without_caller);
    observation_and_distinct(&accounts)
}

fn require_handoff_distinct(
    snapshot: &RetirementReplayHandoffSnapshotV1,
) -> Result<(), TerminalRetirementErrorV1> {
    let accounts = handoff_accounts(snapshot);
    for (left, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(left.saturating_add(1))
            .any(|other| account.key == other.key)
        {
            return Err(TerminalRetirementErrorV1::Alias);
        }
    }
    Ok(())
}

fn handoff_accounts(snapshot: &RetirementReplayHandoffSnapshotV1) -> Vec<&ObservedAccount> {
    let mut accounts = vec![
        &snapshot.payer,
        &snapshot.market,
        &snapshot.activation_cache,
        &snapshot.registry_program,
        &snapshot.core_program,
        &snapshot.core_programdata,
        &snapshot.trading_program,
        &snapshot.trading_programdata,
        &snapshot.custody_program,
        &snapshot.custody_programdata,
    ];
    accounts.extend(snapshot.caller_authority.iter());
    accounts.extend([
        &snapshot.claims_aggregate,
        &snapshot.realm,
        &snapshot.realm_staging,
        &snapshot.rent_sysvar,
        &snapshot.rent_credit,
        &snapshot.trading_replay,
        &snapshot.core_replay,
        &snapshot.hoard,
        &snapshot.system_program,
        &snapshot.mint,
        &snapshot.token_program,
        &snapshot.custody_authority,
    ]);
    accounts
}

#[cfg(test)]
mod tests {
    use super::*;

    const OBSERVATION: Observation = Observation {
        slot: 900,
        unix_timestamp: 1_788_000_000,
        finality: Finality::Finalized,
    };

    fn account(value: u8) -> ObservedAccount {
        ObservedAccount {
            observation: OBSERVATION,
            key: Pubkey::new_from_array([value; 32]),
            owner: Pubkey::new_from_array([value.wrapping_add(100); 32]),
            lamports: u64::from(value) + 1,
            executable: false,
            data: vec![value],
        }
    }

    fn close_snapshot() -> DirectNativeCloseSnapshotV1 {
        let mut system = account(26);
        system.key = system_program::ID;
        let mut snapshot = DirectNativeCloseSnapshotV1 {
            market: account(1),
            realm: account(2),
            realm_staging: account(3),
            manifest: account(4),
            manifest_staging: account(5),
            funding_ledgers: vec![account(6), account(31)],
            root: account(7),
            activation_cache: account(8),
            core_program: account(9),
            core_programdata: account(10),
            trading_program: account(11),
            trading_programdata: account(12),
            resolution_program: account(13),
            resolution_programdata: account(14),
            registry_program: account(15),
            rent_sysvar: account(16),
            caller_authority: Some(account(17)),
            program_set: account(18),
            program_set_staging: account(19),
            config: account(20),
            config_staging: account(21),
            close_profile: account(22),
            close_profile_staging: account(23),
            close_effect: account(24),
            close_effect_staging: account(25),
            system_program: system,
            close_descriptor: account(27),
            close_descriptor_staging: account(28),
            rent_program: account(29),
            rent_credit: account(30),
        };
        snapshot.funding_ledgers[0].owner = snapshot.resolution_program.key;
        snapshot.funding_ledgers[1].owner = snapshot.trading_program.key;
        snapshot
    }

    fn handoff_snapshot() -> RetirementReplayHandoffSnapshotV1 {
        let mut system = account(59);
        system.key = system_program::ID;
        RetirementReplayHandoffSnapshotV1 {
            payer: account(40),
            market: account(41),
            activation_cache: account(42),
            registry_program: account(43),
            core_program: account(44),
            core_programdata: account(45),
            trading_program: account(46),
            trading_programdata: account(47),
            custody_program: account(48),
            custody_programdata: account(49),
            caller_authority: Some(account(50)),
            claims_aggregate: account(51),
            realm: account(52),
            realm_staging: account(53),
            rent_sysvar: account(54),
            rent_credit: account(55),
            trading_replay: account(56),
            core_replay: account(57),
            hoard: account(58),
            system_program: system,
            mint: account(60),
            token_program: account(61),
            custody_authority: account(62),
        }
    }

    fn close_coordinates(
        snapshot: &DirectNativeCloseSnapshotV1,
    ) -> DirectNativeCloseCoordinateInputV1 {
        DirectNativeCloseCoordinateInputV1 {
            release_set: [70; 32],
            role_request_digest: [71; 32],
            market: snapshot.market.key,
            realm: TerminalRecordCoordinatesV1 {
                raw: snapshot.realm.key,
                staging: snapshot.realm_staging.key,
            },
            manifest: TerminalRecordCoordinatesV1 {
                raw: snapshot.manifest.key,
                staging: snapshot.manifest_staging.key,
            },
            resolution_funding: snapshot.funding_ledgers[0].key,
            trading_funding: snapshot.funding_ledgers[1].key,
            root: snapshot.root.key,
            activation_cache: snapshot.activation_cache.key,
            core: TerminalDeploymentCoordinatesV1 {
                program: snapshot.core_program.key,
                programdata: snapshot.core_programdata.key,
            },
            trading: TerminalDeploymentCoordinatesV1 {
                program: snapshot.trading_program.key,
                programdata: snapshot.trading_programdata.key,
            },
            resolution: TerminalDeploymentCoordinatesV1 {
                program: snapshot.resolution_program.key,
                programdata: snapshot.resolution_programdata.key,
            },
            registry_program: snapshot.registry_program.key,
            rent_sysvar: snapshot.rent_sysvar.key,
            program_set: TerminalRecordCoordinatesV1 {
                raw: snapshot.program_set.key,
                staging: snapshot.program_set_staging.key,
            },
            config: TerminalRecordCoordinatesV1 {
                raw: snapshot.config.key,
                staging: snapshot.config_staging.key,
            },
            close_profile: TerminalRecordCoordinatesV1 {
                raw: snapshot.close_profile.key,
                staging: snapshot.close_profile_staging.key,
            },
            close_effect: TerminalRecordCoordinatesV1 {
                raw: snapshot.close_effect.key,
                staging: snapshot.close_effect_staging.key,
            },
            system_program: snapshot.system_program.key,
            close_descriptor: TerminalRecordCoordinatesV1 {
                raw: snapshot.close_descriptor.key,
                staging: snapshot.close_descriptor_staging.key,
            },
            rent_program: snapshot.rent_program.key,
            rent_credit: snapshot.rent_credit.key,
        }
    }

    fn handoff_coordinates(
        snapshot: &RetirementReplayHandoffSnapshotV1,
    ) -> RetirementReplayHandoffCoordinateInputV1 {
        RetirementReplayHandoffCoordinateInputV1 {
            release_set: [72; 32],
            context: [73; 32],
            request_digest: [74; 32],
            payer: snapshot.payer.key,
            market: snapshot.market.key,
            activation_cache: snapshot.activation_cache.key,
            registry_program: snapshot.registry_program.key,
            core: TerminalDeploymentCoordinatesV1 {
                program: snapshot.core_program.key,
                programdata: snapshot.core_programdata.key,
            },
            trading: TerminalDeploymentCoordinatesV1 {
                program: snapshot.trading_program.key,
                programdata: snapshot.trading_programdata.key,
            },
            custody: TerminalDeploymentCoordinatesV1 {
                program: snapshot.custody_program.key,
                programdata: snapshot.custody_programdata.key,
            },
            claims_aggregate: snapshot.claims_aggregate.key,
            realm: TerminalRecordCoordinatesV1 {
                raw: snapshot.realm.key,
                staging: snapshot.realm_staging.key,
            },
            rent_sysvar: snapshot.rent_sysvar.key,
            rent_credit: snapshot.rent_credit.key,
            trading_replay: snapshot.trading_replay.key,
            core_replay: snapshot.core_replay.key,
            hoard: snapshot.hoard.key,
            system_program: snapshot.system_program.key,
            mint: snapshot.mint.key,
            token_program: snapshot.token_program.key,
            custody_authority: snapshot.custody_authority.key,
        }
    }

    #[test]
    fn close_frame_admits_only_the_contract_owned_seven_aliases() {
        let snapshot = close_snapshot();
        let layout = CapabilityRouteLayoutV1::new(2, 20).expect("F=2 close layout");
        let closure =
            project_direct_native_close_coordinate_closure_v1(&close_coordinates(&snapshot))
                .expect("coordinate closure");
        let canonical = closure.accounts;
        assert_eq!(canonical.len(), 38);
        assert_eq!(closure.classes.len(), canonical.len());
        assert_eq!(
            closure
                .classes
                .iter()
                .enumerate()
                .filter_map(
                    |(index, class)| (*class != TerminalMetaClassV1::LookupStable)
                        .then_some((index, *class))
                )
                .collect::<Vec<_>>(),
            vec![
                (9, TerminalMetaClassV1::InlineProgram),
                (11, TerminalMetaClassV1::InlineProgram),
                (13, TerminalMetaClassV1::InlineProgram),
                (15, TerminalMetaClassV1::InlineProgram),
                (17, TerminalMetaClassV1::InlineRequestBound),
                (27, TerminalMetaClassV1::InlineProgram),
                (29, TerminalMetaClassV1::InlineProgram),
                (31, TerminalMetaClassV1::InlineProgram),
                (33, TerminalMetaClassV1::InlineProgram),
                (36, TerminalMetaClassV1::InlineProgram),
            ]
        );
        assert!(exact_close_aliases(&canonical, layout));
        assert_eq!(
            canonical
                .iter()
                .enumerate()
                .filter_map(|(index, meta)| meta.is_writable.then_some(index))
                .collect::<Vec<_>>(),
            vec![0, 6, 7, 37]
        );
        assert!(canonical.iter().all(|meta| !meta.is_signer));
        assert_eq!(
            layout.close_alias_pairs(),
            [
                (8, 26),
                (9, 27),
                (10, 28),
                (11, 29),
                (12, 30),
                (15, 31),
                (16, 32),
            ]
        );

        let mut missing = canonical.clone();
        missing[26].pubkey = Pubkey::new_unique();
        assert!(!exact_close_aliases(&missing, layout));

        let mut third_alias = canonical.clone();
        third_alias[33].pubkey = third_alias[8].pubkey;
        assert!(!exact_close_aliases(&third_alias, layout));

        let mut cross_pair = canonical.clone();
        cross_pair[9].pubkey = cross_pair[8].pubkey;
        cross_pair[27].pubkey = cross_pair[8].pubkey;
        assert!(!exact_close_aliases(&cross_pair, layout));

        let mut shifted = canonical.clone();
        shifted[26].pubkey = shifted[9].pubkey;
        assert!(!exact_close_aliases(&shifted, layout));

        let mut extra = canonical.clone();
        extra[34].pubkey = extra[33].pubkey;
        assert!(!exact_close_aliases(&extra, layout));
    }

    #[test]
    fn close_snapshot_refuses_mixed_finality_and_semantic_aliases() {
        let mut snapshot = close_snapshot();
        assert_eq!(close_observation(&snapshot), Ok(OBSERVATION));
        snapshot.close_effect_staging.observation.slot += 1;
        assert_eq!(
            close_observation(&snapshot),
            Err(TerminalRetirementErrorV1::Snapshot)
        );
        snapshot.close_effect_staging.observation = OBSERVATION;
        snapshot.close_effect_staging.key = snapshot.close_effect.key;
        assert_eq!(
            close_observation(&snapshot),
            Err(TerminalRetirementErrorV1::Alias)
        );
    }

    #[test]
    fn handoff_frame_uses_all_23_owned_roles_and_exact_privileges() {
        let snapshot = handoff_snapshot();
        let closure = project_retirement_replay_handoff_coordinate_closure_v1(
            &handoff_coordinates(&snapshot),
        )
        .expect("coordinate closure");
        let metas = closure.accounts;
        assert_eq!(metas.len(), RetirementReplayHandoffAccountLayoutV1::COUNT);
        assert_eq!(closure.classes.len(), metas.len());
        assert_eq!(closure.classes[0], TerminalMetaClassV1::InlineSigner);
        assert_eq!(closure.classes[10], TerminalMetaClassV1::InlineRequestBound);
        for index in [3_usize, 4, 6, 8, 19, 21] {
            assert_eq!(closure.classes[index], TerminalMetaClassV1::InlineProgram);
        }
        for index in [1_usize, 2, 5, 7, 9, 11, 12, 13, 14, 15, 16, 17, 18, 20, 22] {
            assert_eq!(closure.classes[index], TerminalMetaClassV1::LookupStable);
        }
        for (index, account) in handoff_accounts(&snapshot).into_iter().enumerate() {
            if index != RetirementReplayHandoffAccountLayoutV1::CALLER_AUTHORITY {
                assert_eq!(metas[index].pubkey, account.key);
            }
            assert_eq!(metas[index].is_signer, index == 0);
            assert_eq!(metas[index].is_writable, matches!(index, 0 | 15 | 16 | 17));
        }
        assert_eq!(handoff_observation(&snapshot), Ok(OBSERVATION));
        assert_eq!(require_handoff_distinct(&snapshot), Ok(()));
    }

    #[test]
    fn request_replans_change_only_the_request_bound_coordinate() {
        let close_snapshot = close_snapshot();
        let close_input = close_coordinates(&close_snapshot);
        let close =
            project_direct_native_close_coordinate_closure_v1(&close_input).expect("close closure");
        let mut close_replan = close_input.clone();
        close_replan.role_request_digest[0] ^= 1;
        let close_replan =
            project_direct_native_close_coordinate_closure_v1(&close_replan).expect("close replan");
        assert_eq!(close.classes, close_replan.classes);
        for index in 0..close.accounts.len() {
            assert_eq!(
                close.accounts[index].pubkey != close_replan.accounts[index].pubkey,
                index == 17
            );
        }

        let handoff_snapshot = handoff_snapshot();
        let handoff_input = handoff_coordinates(&handoff_snapshot);
        let handoff = project_retirement_replay_handoff_coordinate_closure_v1(&handoff_input)
            .expect("handoff closure");
        let mut handoff_replan = handoff_input.clone();
        handoff_replan.request_digest[0] ^= 1;
        let handoff_replan =
            project_retirement_replay_handoff_coordinate_closure_v1(&handoff_replan)
                .expect("handoff replan");
        assert_eq!(handoff.classes, handoff_replan.classes);
        for index in 0..handoff.accounts.len() {
            assert_eq!(
                handoff.accounts[index].pubkey != handoff_replan.accounts[index].pubkey,
                index == 10
            );
        }
    }

    #[test]
    fn caller_discovery_requires_absence_and_full_build_requires_fetched_vacancy() {
        let mut close = close_snapshot();
        assert_eq!(
            preflight_direct_native_close_caller_v1(&close),
            Err(TerminalRetirementErrorV1::Frame)
        );
        close.caller_authority = None;
        assert_eq!(
            build_direct_native_close_v1(&close),
            Err(TerminalRetirementErrorV1::Frame)
        );
        assert_eq!(
            preflight_direct_native_close_caller_v1(&close),
            Err(TerminalRetirementErrorV1::MarketCore(
                dclutch_market::Error::InvalidLength
            ))
        );

        let mut handoff = handoff_snapshot();
        assert_eq!(
            preflight_retirement_replay_handoff_caller_v1(&handoff),
            Err(TerminalRetirementErrorV1::Frame)
        );
        handoff.caller_authority = None;
        assert_eq!(
            build_retirement_replay_handoff_v1(&handoff),
            Err(TerminalRetirementErrorV1::Frame)
        );
        assert_eq!(
            preflight_retirement_replay_handoff_caller_v1(&handoff),
            Err(TerminalRetirementErrorV1::MarketCore(
                dclutch_market::Error::InvalidLength
            ))
        );
    }

    #[test]
    fn handoff_snapshot_refuses_mixed_finality_and_every_role_alias() {
        let mut mixed = handoff_snapshot();
        mixed.token_program.observation.unix_timestamp += 1;
        assert_eq!(
            handoff_observation(&mixed),
            Err(TerminalRetirementErrorV1::Snapshot)
        );

        let baseline = handoff_snapshot();
        for index in 1..RetirementReplayHandoffAccountLayoutV1::COUNT {
            let mut hostile = baseline.clone();
            match index {
                1 => hostile.market.key = hostile.payer.key,
                2 => hostile.activation_cache.key = hostile.payer.key,
                3 => hostile.registry_program.key = hostile.payer.key,
                4 => hostile.core_program.key = hostile.payer.key,
                5 => hostile.core_programdata.key = hostile.payer.key,
                6 => hostile.trading_program.key = hostile.payer.key,
                7 => hostile.trading_programdata.key = hostile.payer.key,
                8 => hostile.custody_program.key = hostile.payer.key,
                9 => hostile.custody_programdata.key = hostile.payer.key,
                10 => hostile.caller_authority.as_mut().expect("caller").key = hostile.payer.key,
                11 => hostile.claims_aggregate.key = hostile.payer.key,
                12 => hostile.realm.key = hostile.payer.key,
                13 => hostile.realm_staging.key = hostile.payer.key,
                14 => hostile.rent_sysvar.key = hostile.payer.key,
                15 => hostile.rent_credit.key = hostile.payer.key,
                16 => hostile.trading_replay.key = hostile.payer.key,
                17 => hostile.core_replay.key = hostile.payer.key,
                18 => hostile.hoard.key = hostile.payer.key,
                19 => hostile.system_program.key = hostile.payer.key,
                20 => hostile.mint.key = hostile.payer.key,
                21 => hostile.token_program.key = hostile.payer.key,
                22 => hostile.custody_authority.key = hostile.payer.key,
                _ => unreachable!(),
            }
            assert_eq!(
                require_handoff_distinct(&hostile),
                Err(TerminalRetirementErrorV1::Alias),
                "role index {index} must not alias payer"
            );
        }
    }
}
