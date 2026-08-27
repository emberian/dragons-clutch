//! Chain-derived construction for the canonical aggregate Market retirement.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use dclutch_claims_svm::{
    liability_basis_state_v2::{LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2},
    market_closure_v1::{
        CLAIMS_MARKET_CLOSURE_POST_RESOURCE_DIGEST_DOMAIN_V1,
        CLAIMS_MARKET_CLOSURE_PRE_RESOURCE_DIGEST_DOMAIN_V1,
        CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1, ClaimsMarketClosureReceiptInputV1,
        ClaimsMarketClosureReceiptV1, ClaimsMarketClosureRequestInputV1,
        ClaimsMarketClosureRequestV1,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_POSTSTATE_DOMAIN_V1, CUSTODY_REQUEST_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1,
    CustodyAuthoritySeedsV1, CustodyReceiptV1, CustodyReplaySeedsV1, CustodyReplayV1,
    CustodyRequestV1, CustodyVaultSeedsV1, OperationV1, ReceiptEvidenceV1,
};
use dclutch_market_core_codec::{
    Action, CoreState, MarketCoreStateSeedsV2, Phase, REQUEST_BYTES, RETIREMENT_BUNDLE_BYTES_V1,
    RETIREMENT_CUSTODY_RECEIPT_COUNT_V1, RETIREMENT_POST_RESOURCE_DIGEST_DOMAIN_V1,
    RETIREMENT_ROLE_COUNT_V1, Request, RetirementBundleInputV1, RetirementBundleV1, STATE_BYTES,
};
use dclutch_realm_contract::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetViewV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1,
};
use dclutch_registry_svm::{
    ProgramDataV3View, ProgramV3View,
    continuation_v1::{
        REGISTRY_CONTINUATION_REQUEST_BYTES_V1, RegistryContinuationAdmissionSeedsV1,
        RegistryContinuationRequestV1,
    },
};
use dclutch_release_set_contract::{
    CallerAuthoritySeedsV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, ProtocolInfrastructureProfileV1,
};
use dclutch_rent_contract::lifecycle_v2::{
    LifecycleAccountIdV2, LifecycleRentCoreCloseAuthoritySeedsV2, LifecycleRentCreditV2,
};
use dclutch_resolution_codec::SourceClosureReceiptV2;
pub use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_program_pack::Pack;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use spl_token_interface::state::{Account as SplTokenAccount, AccountState};

/// Exact top-level Registry prefix before the nested Core retirement frame.
pub const REGISTRY_RETIREMENT_CONTINUATION_PREFIX_ACCOUNTS_V1: usize = 10;
/// Exact Core retirement frame before the invocation-scoped Registry admission.
pub const CORE_RETIREMENT_ACCOUNT_COUNT_V1: usize = 35;
/// Exact nested Core retirement data width.
pub const MARKET_RETIREMENT_CORE_INSTRUCTION_BYTES_V1: usize = REQUEST_BYTES
    + RETIREMENT_BUNDLE_BYTES_V1
    + CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1
    + 2 * CUSTODY_REQUEST_BYTES_V1;
/// Exact top-level Registry account count for one aggregate retirement.
pub const MARKET_RETIREMENT_ACCOUNT_COUNT_V1: usize =
    REGISTRY_RETIREMENT_CONTINUATION_PREFIX_ACCOUNTS_V1 + CORE_RETIREMENT_ACCOUNT_COUNT_V1 + 1;

const RETIREMENT_CANDIDATE_DOMAIN_V1: &[u8] = b"dclutch/market-retirement-candidate/v1";
const RETIREMENT_ORDER_DOMAIN_V1: &[u8] = b"dclutch/market-retirement-order/v1";

/// Same-finalized accounts required to derive one complete aggregate retirement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketRetirementSnapshotV1 {
    /// Retiring Core Market.
    pub market: ObservedAccount,
    /// Permanent lifecycle RentCredit.
    pub rent_credit: ObservedAccount,
    /// Current activated release cache.
    pub activation_cache: ObservedAccount,
    /// Current Registry program.
    pub registry_program: ObservedAccount,
    /// Current Core program.
    pub core_program: ObservedAccount,
    /// Current Core ProgramData.
    pub core_programdata: ObservedAccount,
    /// Current Claims program.
    pub claims_program: ObservedAccount,
    /// Current Claims ProgramData.
    pub claims_programdata: ObservedAccount,
    /// Current Resolution program.
    pub resolution_program: ObservedAccount,
    /// Current Resolution ProgramData.
    pub resolution_programdata: ObservedAccount,
    /// Current Custody program.
    pub custody_program: ObservedAccount,
    /// Current Custody ProgramData.
    pub custody_programdata: ObservedAccount,
    /// Infrastructure-selected Rent program.
    pub rent_program: ObservedAccount,
    /// Resolution-owned closure receipt.
    pub source_receipt: ObservedAccount,
    /// Claims-owned runtime-width aggregate.
    pub claims_aggregate: ObservedAccount,
    /// Custody replay cursor.
    pub custody_replay: ObservedAccount,
    /// Empty HoardPrincipal vault.
    pub hoard_vault: ObservedAccount,
    /// Custody token authority PDA.
    pub custody_authority: ObservedAccount,
    /// Realm-selected collateral Mint.
    pub collateral_mint: ObservedAccount,
    /// Realm-selected collateral token program.
    pub collateral_token_program: ObservedAccount,
    /// Finalized Realm raw record.
    pub realm_raw: ObservedAccount,
    /// Vacant Realm staging cursor.
    pub realm_staging: ObservedAccount,
    /// Immutable Core infrastructure profile.
    pub infrastructure_profile: ObservedAccount,
    /// Finalized Registry ArtifactRelease record.
    pub registry_artifact_raw: ObservedAccount,
    /// Vacant Registry ArtifactRelease staging cursor.
    pub registry_artifact_staging: ObservedAccount,
    /// Current Registry ProgramData.
    pub registry_programdata: ObservedAccount,
    /// Finalized Rent ArtifactRelease record.
    pub rent_artifact_raw: ObservedAccount,
    /// Vacant Rent ArtifactRelease staging cursor.
    pub rent_artifact_staging: ObservedAccount,
    /// Current Rent ProgramData.
    pub rent_programdata: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Immutable lifecycle refund wallet.
    pub refund_wallet: ObservedAccount,
}

/// Exact unsigned retirement transaction constructed solely from finalized state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketRetirementReportV1 {
    /// Top-level Registry continuation instruction.
    pub instruction: Instruction,
    /// Nested Core instruction before the Registry admission is appended.
    pub direct_instruction: Instruction,
    /// Shared finalized observation.
    pub observation: Observation,
    /// Invocation-scoped Registry continuation admission.
    pub registry_admission: Pubkey,
    /// Core-derived Claims close authority.
    pub claims_authority: Pubkey,
    /// Core-derived Custody CloseVault authority.
    pub close_vault_authority: Pubkey,
    /// Core-derived Custody CloseReplay authority.
    pub close_replay_authority: Pubkey,
    /// Core-derived RentV2 close authority.
    pub rent_close_authority: Pubkey,
    /// Exact lamports credited through Claims, Custody, Core, and RentV2.
    pub expected_refund_delta: u64,
    /// Runtime Claims width; never assumed equal to a compile-time `N`.
    pub claim_count: u32,
    /// Exact Registry continuation header.
    pub continuation: RegistryContinuationRequestV1,
}

/// Stable refusal from chain observation, semantic join, or instruction construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarketRetirementOperatorErrorV1 {
    /// Observations were not one finalized snapshot.
    Observation,
    /// A Registry cache or Loader deployment differed.
    Release,
    /// Core Market or RentV2 lifecycle facts differed.
    Market,
    /// Resolution closure facts differed.
    Resolution,
    /// Claims aggregate identity, width, revision, or liability differed.
    Claims,
    /// Custody replay, vault, Realm, token, or conservation facts differed.
    Custody,
    /// A deterministic address, account alias, or privilege profile differed.
    Frame,
    /// Fixed-width request or digest construction refused.
    Encoding,
    /// Checked lamport or revision arithmetic overflowed.
    Arithmetic,
}

#[derive(Clone, Copy)]
struct AuthenticatedRetirementV1 {
    observation: Observation,
    market: CoreState,
    source: SourceClosureReceiptV2,
    claims: LiabilityBasisMarketViewV2,
    replay: CustodyReplayV1,
}

/// Construct the sole canonical aggregate-retirement transaction from one
/// finalized chain snapshot. The caller contributes no release, revision,
/// width, custody, refund, or child-receipt truth.
pub fn build_market_retirement_v1(
    snapshot: &MarketRetirementSnapshotV1,
) -> Result<MarketRetirementReportV1, MarketRetirementOperatorErrorV1> {
    let authenticated = authenticate_snapshot(snapshot)?;
    let market = authenticated.market;
    let release_set = market.identity.selected_release_set.to_bytes();
    let market_key = snapshot.market.key;
    let core_request = Request::administrative(
        Action::Retire,
        market.identity.generation,
        market.identity.market_id,
    );
    let core_bytes = core_request
        .encode()
        .map_err(|_| MarketRetirementOperatorErrorV1::Encoding)?;
    let parent_digest = hash(&core_bytes).to_bytes();

    let claims_request = ClaimsMarketClosureRequestV1::new(ClaimsMarketClosureRequestInputV1 {
        release_set,
        market: market_key.to_bytes(),
        aggregate: snapshot.claims_aggregate.key.to_bytes(),
        rent_credit: snapshot.rent_credit.key.to_bytes(),
        parent_request_digest: parent_digest,
        core_program: snapshot.core_program.key.to_bytes(),
        generation: market.identity.generation,
        expected_revision: authenticated.claims.revision,
        resulting_revision: authenticated
            .claims
            .revision
            .checked_add(1)
            .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?,
        claim_count: authenticated.claims.claim_count,
    })
    .map_err(|_| MarketRetirementOperatorErrorV1::Claims)?;
    let claims_bytes = claims_request.to_bytes();

    let candidate = hashv(&[
        RETIREMENT_CANDIDATE_DOMAIN_V1,
        market_key.as_ref(),
        &market.identity.generation.to_le_bytes(),
        &parent_digest,
    ])
    .to_bytes();
    let order = hashv(&[
        RETIREMENT_ORDER_DOMAIN_V1,
        market_key.as_ref(),
        &authenticated.replay.next_revision.to_le_bytes(),
        &parent_digest,
    ])
    .to_bytes();
    let close_vault = custody_request(
        snapshot,
        authenticated,
        OperationV1::CloseVault,
        parent_digest,
        candidate,
        order,
        authenticated.replay.next_revision,
        0,
    )?;
    authenticate_custody_authority(snapshot, close_vault)?;
    let close_vault_bytes = close_vault
        .to_bytes()
        .map_err(|_| MarketRetirementOperatorErrorV1::Custody)?;
    let close_replay = custody_request(
        snapshot,
        authenticated,
        OperationV1::CloseReplay,
        parent_digest,
        candidate,
        order,
        authenticated
            .replay
            .next_revision
            .checked_add(1)
            .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?,
        1,
    )?;
    let close_replay_bytes = close_replay
        .to_bytes()
        .map_err(|_| MarketRetirementOperatorErrorV1::Custody)?;

    let claims_request_digest = hash(&claims_bytes).to_bytes();
    let claims_receipt = projected_claims_receipt(snapshot, authenticated, claims_request_digest)?;
    let claims_receipt_digest = hash(&claims_receipt.to_bytes()).to_bytes();
    let (close_vault_receipt_digest, close_replay_receipt_digest) = projected_custody_receipts(
        snapshot,
        authenticated,
        close_vault,
        &close_vault_bytes,
        close_replay,
        &close_replay_bytes,
    )?;

    let source_digest = hash(&snapshot.source_receipt.data).to_bytes();
    let custody_refund = snapshot
        .hoard_vault
        .lamports
        .checked_add(snapshot.custody_replay.lamports)
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    let expected_refund_delta = snapshot
        .rent_credit
        .lamports
        .checked_add(snapshot.claims_aggregate.lamports)
        .and_then(|value| value.checked_add(custody_refund))
        .and_then(|value| value.checked_add(snapshot.market.lamports))
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    let post_resource_digest = hashv(&[
        &RETIREMENT_POST_RESOURCE_DIGEST_DOMAIN_V1,
        &[RETIREMENT_ROLE_COUNT_V1],
        &[RETIREMENT_CUSTODY_RECEIPT_COUNT_V1],
        snapshot.rent_credit.key.as_ref(),
        &source_digest,
        &claims_receipt_digest,
        &close_vault_receipt_digest,
        &close_replay_receipt_digest,
        &snapshot.market.lamports.to_le_bytes(),
        &snapshot.claims_aggregate.lamports.to_le_bytes(),
        &custody_refund.to_le_bytes(),
        &expected_refund_delta.to_le_bytes(),
    ])
    .to_bytes();
    let rent_close_seeds = LifecycleRentCoreCloseAuthoritySeedsV2::new(
        LifecycleAccountIdV2::new(snapshot.rent_credit.key.to_bytes())
            .map_err(|_| MarketRetirementOperatorErrorV1::Market)?,
        post_resource_digest,
    )
    .map_err(|_| MarketRetirementOperatorErrorV1::Market)?;
    let rent_credit_seed = rent_close_seeds.credit().to_bytes();
    let rent_close_digest = rent_close_seeds.post_resource_digest();
    let rent_close_authority = Pubkey::find_program_address(
        &[
            rent_close_seeds.domain(),
            &rent_credit_seed,
            &rent_close_digest,
        ],
        &snapshot.core_program.key,
    )
    .0;

    let bundle = RetirementBundleV1::new(RetirementBundleInputV1 {
        market: market_key.to_bytes(),
        release_set,
        rent_credit: snapshot.rent_credit.key.to_bytes(),
        source_receipt_account: snapshot.source_receipt.key.to_bytes(),
        claims_aggregate: snapshot.claims_aggregate.key.to_bytes(),
        custody_replay: snapshot.custody_replay.key.to_bytes(),
        hoard_vault: snapshot.hoard_vault.key.to_bytes(),
        source_receipt_digest: source_digest,
        claims_request_digest,
        custody_close_vault_request_digest: hash(&close_vault_bytes).to_bytes(),
        custody_close_replay_request_digest: hash(&close_replay_bytes).to_bytes(),
        core_prestate_digest: hash(&snapshot.market.data).to_bytes(),
        generation: market.identity.generation,
        source_closure_revision: authenticated
            .source
            .terminal_sequence
            .checked_add(1)
            .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?,
        claims_pre_revision: authenticated.claims.revision,
        claims_post_revision: authenticated
            .claims
            .revision
            .checked_add(1)
            .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?,
        custody_pre_revision: authenticated.replay.next_revision,
        custody_middle_revision: authenticated
            .replay
            .next_revision
            .checked_add(1)
            .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?,
        custody_post_revision: authenticated
            .replay
            .next_revision
            .checked_add(2)
            .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?,
        expected_core_lamports: snapshot.market.lamports,
    })
    .map_err(|_| MarketRetirementOperatorErrorV1::Encoding)?;

    let claims_authority = caller_authority(
        release_set,
        market_key,
        parent_digest,
        &claims_bytes,
        snapshot.core_program.key,
    )?;
    let close_vault_authority = caller_authority(
        release_set,
        market_key,
        authenticated.replay.context,
        &close_vault_bytes,
        snapshot.core_program.key,
    )?;
    let close_replay_authority = caller_authority(
        release_set,
        market_key,
        authenticated.replay.context,
        &close_replay_bytes,
        snapshot.core_program.key,
    )?;

    let mut data = Vec::with_capacity(MARKET_RETIREMENT_CORE_INSTRUCTION_BYTES_V1);
    data.extend_from_slice(&core_bytes);
    data.extend_from_slice(&bundle.to_bytes());
    data.extend_from_slice(&claims_bytes);
    data.extend_from_slice(&close_vault_bytes);
    data.extend_from_slice(&close_replay_bytes);
    if data.len() != MARKET_RETIREMENT_CORE_INSTRUCTION_BYTES_V1 {
        return Err(MarketRetirementOperatorErrorV1::Encoding);
    }
    let direct_instruction = Instruction {
        program_id: snapshot.core_program.key,
        accounts: core_accounts(
            snapshot,
            claims_authority,
            close_vault_authority,
            close_replay_authority,
            rent_close_authority,
        ),
        data,
    };
    if direct_instruction.accounts.len() != CORE_RETIREMENT_ACCOUNT_COUNT_V1 {
        return Err(MarketRetirementOperatorErrorV1::Frame);
    }
    let (instruction, registry_admission, continuation) =
        wrap_registry_continuation(snapshot, &direct_instruction)?;
    Ok(MarketRetirementReportV1 {
        instruction,
        direct_instruction,
        observation: authenticated.observation,
        registry_admission,
        claims_authority,
        close_vault_authority,
        close_replay_authority,
        rent_close_authority,
        expected_refund_delta,
        claim_count: authenticated.claims.claim_count,
        continuation,
    })
}

fn authenticate_snapshot(
    snapshot: &MarketRetirementSnapshotV1,
) -> Result<AuthenticatedRetirementV1, MarketRetirementOperatorErrorV1> {
    let accounts = snapshot_accounts(snapshot);
    let observation = accounts
        .first()
        .map(|account| account.observation)
        .ok_or(MarketRetirementOperatorErrorV1::Observation)?;
    if accounts
        .iter()
        .any(|account| account.observation.finality != Finality::Finalized)
        || accounts
            .iter()
            .any(|account| account.observation != observation)
    {
        return Err(MarketRetirementOperatorErrorV1::Observation);
    }
    for (left_index, left) in accounts.iter().enumerate() {
        for right in accounts.iter().skip(left_index.saturating_add(1)) {
            if left.key == right.key {
                return Err(MarketRetirementOperatorErrorV1::Frame);
            }
        }
    }

    let market = CoreState::decode(&snapshot.market.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::Market)?;
    let expected_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market.identity).as_slices(),
        &snapshot.core_program.key,
    )
    .0;
    if snapshot.market.owner != snapshot.core_program.key
        || snapshot.market.executable
        || snapshot.market.data.len() != STATE_BYTES
        || snapshot.market.lamports == 0
        || snapshot.market.key != expected_market
        || market.identity.market_id.to_bytes() != snapshot.market.key.to_bytes()
        || market.identity.registry_program.to_bytes() != snapshot.registry_program.key.to_bytes()
        || market.phase != Phase::Retiring
        || market.outstanding_capabilities != 0
        || market.rent_beneficiary.to_bytes() != snapshot.rent_credit.key.to_bytes()
    {
        return Err(MarketRetirementOperatorErrorV1::Market);
    }

    authenticate_release_set(snapshot, market)?;
    authenticate_infrastructure(snapshot)?;
    authenticate_rent(snapshot, market)?;
    let source = authenticate_resolution(snapshot, market)?;
    let claims = authenticate_claims(snapshot, market)?;
    let (replay, _) = authenticate_custody(snapshot, market)?;
    Ok(AuthenticatedRetirementV1 {
        observation,
        market,
        source,
        claims,
        replay,
    })
}

fn snapshot_accounts(snapshot: &MarketRetirementSnapshotV1) -> [&ObservedAccount; 31] {
    [
        &snapshot.market,
        &snapshot.rent_credit,
        &snapshot.activation_cache,
        &snapshot.registry_program,
        &snapshot.core_program,
        &snapshot.core_programdata,
        &snapshot.claims_program,
        &snapshot.claims_programdata,
        &snapshot.resolution_program,
        &snapshot.resolution_programdata,
        &snapshot.custody_program,
        &snapshot.custody_programdata,
        &snapshot.rent_program,
        &snapshot.source_receipt,
        &snapshot.claims_aggregate,
        &snapshot.custody_replay,
        &snapshot.hoard_vault,
        &snapshot.custody_authority,
        &snapshot.collateral_mint,
        &snapshot.collateral_token_program,
        &snapshot.realm_raw,
        &snapshot.realm_staging,
        &snapshot.infrastructure_profile,
        &snapshot.registry_artifact_raw,
        &snapshot.registry_artifact_staging,
        &snapshot.registry_programdata,
        &snapshot.rent_artifact_raw,
        &snapshot.rent_artifact_staging,
        &snapshot.rent_programdata,
        &snapshot.rent_sysvar,
        &snapshot.refund_wallet,
    ]
}

fn authenticate_release_set(
    snapshot: &MarketRetirementSnapshotV1,
    market: CoreState,
) -> Result<(), MarketRetirementOperatorErrorV1> {
    if snapshot.registry_program.owner != bpf_loader_upgradeable::ID
        || !snapshot.registry_program.executable
        || ProgramV3View::parse(&snapshot.registry_program.data).is_err()
        || snapshot.activation_cache.owner != snapshot.registry_program.key
        || snapshot.activation_cache.executable
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&snapshot.activation_cache.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::Release)?;
    let release_set = activated
        .execution_release_set_id()
        .map_err(|_| MarketRetirementOperatorErrorV1::Release)?;
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set.as_bytes()],
        &snapshot.registry_program.key,
    )
    .0;
    if release_set.to_bytes() != market.identity.selected_release_set.to_bytes()
        || expected_cache != snapshot.activation_cache.key
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Core,
            &snapshot.core_program,
            &snapshot.core_programdata,
        ),
        (
            ExecutionRoleV1::Claims,
            &snapshot.claims_program,
            &snapshot.claims_programdata,
        ),
        (
            ExecutionRoleV1::Resolution,
            &snapshot.resolution_program,
            &snapshot.resolution_programdata,
        ),
        (
            ExecutionRoleV1::Custody,
            &snapshot.custody_program,
            &snapshot.custody_programdata,
        ),
    ] {
        authenticate_current_role(activated, role, program, programdata)?;
    }
    Ok(())
}

fn authenticate_current_role(
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
    role: ExecutionRoleV1,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<(), MarketRetirementOperatorErrorV1> {
    let selected = activated
        .role(role)
        .map_err(|_| MarketRetirementOperatorErrorV1::Release)?;
    let release = selected.release();
    if release.program().to_bytes() != program.key.to_bytes()
        || release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.programdata() != programdata.key.to_bytes()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    let deployment = deployment_observation(program, programdata)?;
    selected
        .authenticate_current_deployment(deployment)
        .map_err(|_| MarketRetirementOperatorErrorV1::Release)
}

fn authenticate_infrastructure(
    snapshot: &MarketRetirementSnapshotV1,
) -> Result<(), MarketRetirementOperatorErrorV1> {
    let expected_profile = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &snapshot.core_program.key,
    )
    .0;
    if snapshot.infrastructure_profile.key != expected_profile
        || snapshot.infrastructure_profile.owner != snapshot.core_program.key
        || snapshot.infrastructure_profile.executable
        || snapshot.infrastructure_profile.lamports == 0
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    let profile = ProtocolInfrastructureProfileV1::decode(&snapshot.infrastructure_profile.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::Release)?;
    authenticate_infrastructure_artifact(
        snapshot,
        profile.registry(),
        &snapshot.registry_artifact_raw,
        &snapshot.registry_artifact_staging,
        &snapshot.registry_program,
        &snapshot.registry_programdata,
    )?;
    authenticate_infrastructure_artifact(
        snapshot,
        profile.rent(),
        &snapshot.rent_artifact_raw,
        &snapshot.rent_artifact_staging,
        &snapshot.rent_program,
        &snapshot.rent_programdata,
    )?;
    if snapshot.rent_sysvar.key != sysvar::rent::ID
        || snapshot.rent_sysvar.owner != sysvar::ID
        || snapshot.rent_sysvar.executable
        || snapshot.rent_sysvar.data.is_empty()
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    Ok(())
}

fn authenticate_infrastructure_artifact(
    snapshot: &MarketRetirementSnapshotV1,
    selected: ExecutionRoleBindingV1,
    raw: &ObservedAccount,
    staging: &ObservedAccount,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<(), MarketRetirementOperatorErrorV1> {
    let digest = hash(&raw.data).to_bytes();
    let expected_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &digest,
        ],
        &snapshot.registry_program.key,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &digest,
        ],
        &snapshot.registry_program.key,
    )
    .0;
    if selected.program().to_bytes() != program.key.to_bytes()
        || selected.artifact_release().to_bytes() != digest
        || raw.key != expected_raw
        || raw.owner != snapshot.registry_program.key
        || raw.executable
        || raw.lamports == 0
        || staging.key != expected_staging
        || staging.owner != system_program::ID
        || staging.executable
        || !staging.data.is_empty()
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    let release = ArtifactReleaseV1::decode(&raw.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::Release)?;
    if release.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable
        || release.program().to_bytes() != program.key.to_bytes()
        || release.programdata() != programdata.key.to_bytes()
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    release
        .authenticate_deployment(deployment_observation(program, programdata)?)
        .map_err(|_| MarketRetirementOperatorErrorV1::Release)
}

fn deployment_observation(
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<DeploymentObservationV1, MarketRetirementOperatorErrorV1> {
    let program_view = ProgramV3View::parse(&program.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::Release)?;
    let expected_programdata =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != programdata.key.to_bytes()
        || programdata.key != expected_programdata
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    let programdata_view = ProgramDataV3View::parse(&programdata.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::Release)?;
    DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        programdata_view.deployment_slot(),
        hash(programdata_view.elf()).to_bytes(),
        programdata_view.upgrade_authority(),
    )
    .map_err(|_| MarketRetirementOperatorErrorV1::Release)
}

fn authenticate_rent(
    snapshot: &MarketRetirementSnapshotV1,
    market: CoreState,
) -> Result<LifecycleRentCreditV2, MarketRetirementOperatorErrorV1> {
    let credit = LifecycleRentCreditV2::decode(&snapshot.rent_credit.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::Market)?;
    let seeds = credit.pda_seeds();
    let bump = [seeds.bump()];
    let generation = seeds.generation();
    let market_seed = seeds.market().to_bytes();
    let expected = Pubkey::create_program_address(
        &[seeds.domain(), &market_seed, &generation, &bump],
        &snapshot.rent_program.key,
    )
    .map_err(|_| MarketRetirementOperatorErrorV1::Market)?;
    if snapshot.rent_credit.owner != snapshot.rent_program.key
        || snapshot.rent_credit.executable
        || snapshot.rent_credit.lamports == 0
        || snapshot.rent_credit.key != expected
        || credit.market().to_bytes() != snapshot.market.key.to_bytes()
        || credit.release_set().to_bytes() != market.identity.selected_release_set.to_bytes()
        || credit.generation() != market.identity.generation
        || credit.refund_wallet().to_bytes() != snapshot.refund_wallet.key.to_bytes()
        || snapshot.refund_wallet.owner != system_program::ID
        || snapshot.refund_wallet.executable
        || !snapshot.refund_wallet.data.is_empty()
    {
        return Err(MarketRetirementOperatorErrorV1::Market);
    }
    Ok(credit)
}

fn authenticate_resolution(
    snapshot: &MarketRetirementSnapshotV1,
    market: CoreState,
) -> Result<SourceClosureReceiptV2, MarketRetirementOperatorErrorV1> {
    let source = SourceClosureReceiptV2::decode(&snapshot.source_receipt.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::Resolution)?;
    if snapshot.source_receipt.owner != snapshot.resolution_program.key
        || snapshot.source_receipt.executable
        || source.receipt_account != snapshot.source_receipt.key.to_bytes()
        || source.market != snapshot.market.key.to_bytes()
        || source.generation != market.identity.generation
        || source.beneficiary != snapshot.rent_credit.key.to_bytes()
        || market.terminal_receipt.map(|value| value.to_bytes())
            != Some(source.terminal_certificate)
    {
        return Err(MarketRetirementOperatorErrorV1::Resolution);
    }
    Ok(source)
}

fn authenticate_claims(
    snapshot: &MarketRetirementSnapshotV1,
    market: CoreState,
) -> Result<LiabilityBasisMarketViewV2, MarketRetirementOperatorErrorV1> {
    let claims = LiabilityBasisMarketViewV2::decode(&snapshot.claims_aggregate.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::Claims)?;
    let expected = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, snapshot.market.key.as_ref()],
        &snapshot.claims_program.key,
    )
    .0;
    if snapshot.claims_aggregate.owner != snapshot.claims_program.key
        || snapshot.claims_aggregate.executable
        || snapshot.claims_aggregate.lamports == 0
        || snapshot.claims_aggregate.key != expected
        || claims.claim_count < 2
        || claims.logical_market != snapshot.market.key.to_bytes()
        || claims.release_set != market.identity.selected_release_set.to_bytes()
        || claims.registry_program != snapshot.registry_program.key.to_bytes()
        || claims.product_instance_id != market.identity.product_record.to_bytes()
        || claims.realm_id != market.identity.realm_id.to_bytes()
        || claims.generation != market.identity.generation
    {
        return Err(MarketRetirementOperatorErrorV1::Claims);
    }
    for claim in 0..claims.claim_count {
        if claims
            .supply(&snapshot.claims_aggregate.data, claim)
            .map_err(|_| MarketRetirementOperatorErrorV1::Claims)?
            != 0
        {
            return Err(MarketRetirementOperatorErrorV1::Claims);
        }
    }
    Ok(claims)
}

fn authenticate_custody(
    snapshot: &MarketRetirementSnapshotV1,
    market: CoreState,
) -> Result<(CustodyReplayV1, RealmV1), MarketRetirementOperatorErrorV1> {
    let replay = CustodyReplayV1::decode(&snapshot.custody_replay.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::Custody)?;
    let realm_digest = market.identity.realm_id.to_bytes();
    let expected_realm_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &realm_digest,
        ],
        &snapshot.registry_program.key,
    )
    .0;
    let expected_realm_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &realm_digest,
        ],
        &snapshot.registry_program.key,
    )
    .0;
    let realm = RealmV1::decode(&snapshot.realm_raw.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::Custody)?;
    if snapshot.realm_raw.owner != snapshot.registry_program.key
        || snapshot.realm_raw.executable
        || snapshot.realm_raw.key != expected_realm_raw
        || hash(&snapshot.realm_raw.data).to_bytes() != realm_digest
        || snapshot.realm_staging.key != expected_realm_staging
        || snapshot.realm_staging.owner != system_program::ID
        || snapshot.realm_staging.executable
        || !snapshot.realm_staging.data.is_empty()
        || replay.caller_role != CallerRoleV1::Core
        || replay.release_set != market.identity.selected_release_set.to_bytes()
        || replay.market != snapshot.market.key.to_bytes()
        || replay.realm != realm_digest
        || replay.caller_program != snapshot.core_program.key.to_bytes()
        || replay.rent_refund != snapshot.rent_credit.key.to_bytes()
        || replay.open_vault_count != 1
        || replay.next_revision == 0
        || replay.generation != market.identity.generation
        || replay.context != claims_context(snapshot)?
        || snapshot.custody_replay.owner != snapshot.custody_program.key
        || snapshot.custody_replay.executable
        || snapshot.custody_replay.lamports == 0
        || realm.collateral_mint() != &snapshot.collateral_mint.key.to_bytes()
        || realm.token_program() != &snapshot.collateral_token_program.key.to_bytes()
    {
        return Err(MarketRetirementOperatorErrorV1::Custody);
    }
    let replay_seeds = CustodyReplaySeedsV1::from_request(custody_request(
        snapshot,
        AuthenticatedRetirementV1 {
            observation: snapshot.market.observation,
            market,
            source: SourceClosureReceiptV2::decode(&snapshot.source_receipt.data)
                .map_err(|_| MarketRetirementOperatorErrorV1::Resolution)?,
            claims: LiabilityBasisMarketViewV2::decode(&snapshot.claims_aggregate.data)
                .map_err(|_| MarketRetirementOperatorErrorV1::Claims)?,
            replay,
        },
        OperationV1::CloseVault,
        [1; 32],
        [2; 32],
        [3; 32],
        replay.next_revision,
        0,
    )?);
    if Pubkey::find_program_address(&replay_seeds.as_slices(), &snapshot.custody_program.key).0
        != snapshot.custody_replay.key
    {
        return Err(MarketRetirementOperatorErrorV1::Custody);
    }
    authenticate_vault(snapshot, replay, realm)?;
    Ok((replay, realm))
}

fn claims_context(
    snapshot: &MarketRetirementSnapshotV1,
) -> Result<[u8; 32], MarketRetirementOperatorErrorV1> {
    LiabilityBasisMarketViewV2::decode(&snapshot.claims_aggregate.data)
        .map(|view| view.custody_context)
        .map_err(|_| MarketRetirementOperatorErrorV1::Claims)
}

fn authenticate_vault(
    snapshot: &MarketRetirementSnapshotV1,
    replay: CustodyReplayV1,
    realm: RealmV1,
) -> Result<(), MarketRetirementOperatorErrorV1> {
    let token = SplTokenAccount::unpack(&snapshot.hoard_vault.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::Custody)?;
    let expected_vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            replay.market,
            replay.release_set,
            replay.context,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &snapshot.custody_program.key,
    )
    .0;
    if snapshot.hoard_vault.key != expected_vault
        || snapshot.hoard_vault.owner != snapshot.collateral_token_program.key
        || snapshot.hoard_vault.executable
        || snapshot.hoard_vault.lamports == 0
        || token.mint.to_bytes() != *realm.collateral_mint()
        || token.owner != snapshot.custody_authority.key
        || token.amount != 0
        || token.state != AccountState::Initialized
        || token.delegate.is_some()
        || token.delegated_amount != 0
        || token.is_native.is_some()
        || token.close_authority.is_some()
        || !snapshot.collateral_token_program.executable
        || snapshot.collateral_mint.owner != snapshot.collateral_token_program.key
    {
        return Err(MarketRetirementOperatorErrorV1::Custody);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn custody_request(
    snapshot: &MarketRetirementSnapshotV1,
    authenticated: AuthenticatedRetirementV1,
    operation: OperationV1,
    parent_request_digest: [u8; 32],
    candidate: [u8; 32],
    order: [u8; 32],
    expected_revision: u64,
    transfer_index: u16,
) -> Result<CustodyRequestV1, MarketRetirementOperatorErrorV1> {
    let close_vault = operation == OperationV1::CloseVault;
    if !matches!(
        operation,
        OperationV1::CloseVault | OperationV1::CloseReplay
    ) {
        return Err(MarketRetirementOperatorErrorV1::Custody);
    }
    let resulting_revision = expected_revision
        .checked_add(1)
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    let request = CustodyRequestV1 {
        operation,
        caller_role: CallerRoleV1::Core,
        source_compartment: if close_vault {
            CompartmentV1::HoardPrincipal
        } else {
            CompartmentV1::None
        },
        destination_compartment: CompartmentV1::None,
        release_set: authenticated
            .market
            .identity
            .selected_release_set
            .to_bytes(),
        market: snapshot.market.key.to_bytes(),
        realm: authenticated.market.identity.realm_id.to_bytes(),
        context: authenticated.replay.context,
        caller_program: snapshot.core_program.key.to_bytes(),
        semantic: ContextV1 {
            candidate,
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order,
            parent_request_digest,
            order_nonce: authenticated.market.identity.generation,
            generation: authenticated.market.identity.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index,
        },
        source: if close_vault {
            snapshot.hoard_vault.key.to_bytes()
        } else {
            [0; 32]
        },
        destination: [0; 32],
        source_vault_context: if close_vault {
            authenticated.replay.context
        } else {
            [0; 32]
        },
        destination_vault_context: [0; 32],
        mint: if close_vault {
            snapshot.collateral_mint.key.to_bytes()
        } else {
            [0; 32]
        },
        token_program: if close_vault {
            snapshot.collateral_token_program.key.to_bytes()
        } else {
            [0; 32]
        },
        payer: [0; 32],
        rent_refund: snapshot.rent_credit.key.to_bytes(),
        expected_revision,
        resulting_revision,
        amount: 0,
        rent_lamports: if close_vault {
            snapshot.hoard_vault.lamports
        } else {
            snapshot.custody_replay.lamports
        },
    };
    request
        .to_bytes()
        .map_err(|_| MarketRetirementOperatorErrorV1::Custody)?;
    Ok(request)
}

fn authenticate_custody_authority(
    snapshot: &MarketRetirementSnapshotV1,
    close_vault: CustodyRequestV1,
) -> Result<(), MarketRetirementOperatorErrorV1> {
    let expected = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::from_request(close_vault).as_slices(),
        &snapshot.custody_program.key,
    )
    .0;
    if snapshot.custody_authority.key != expected
        || snapshot.custody_authority.owner != system_program::ID
        || snapshot.custody_authority.executable
        || !snapshot.custody_authority.data.is_empty()
    {
        return Err(MarketRetirementOperatorErrorV1::Custody);
    }
    Ok(())
}

fn projected_claims_receipt(
    snapshot: &MarketRetirementSnapshotV1,
    authenticated: AuthenticatedRetirementV1,
    request_digest: [u8; 32],
) -> Result<ClaimsMarketClosureReceiptV1, MarketRetirementOperatorErrorV1> {
    let post_revision = authenticated
        .claims
        .revision
        .checked_add(1)
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    let credit_after = snapshot
        .rent_credit
        .lamports
        .checked_add(snapshot.claims_aggregate.lamports)
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    let pre_resource_digest = hashv(&[
        &CLAIMS_MARKET_CLOSURE_PRE_RESOURCE_DIGEST_DOMAIN_V1,
        snapshot.claims_aggregate.key.as_ref(),
        &snapshot.claims_aggregate.data,
    ])
    .to_bytes();
    let post_resource_digest = hashv(&[
        &CLAIMS_MARKET_CLOSURE_POST_RESOURCE_DIGEST_DOMAIN_V1,
        snapshot.claims_aggregate.key.as_ref(),
        snapshot.rent_credit.key.as_ref(),
        &post_revision.to_le_bytes(),
        &snapshot.claims_aggregate.lamports.to_le_bytes(),
        &credit_after.to_le_bytes(),
    ])
    .to_bytes();
    ClaimsMarketClosureReceiptV1::new(ClaimsMarketClosureReceiptInputV1 {
        producer: snapshot.claims_program.key.to_bytes(),
        release_set: authenticated
            .market
            .identity
            .selected_release_set
            .to_bytes(),
        market: snapshot.market.key.to_bytes(),
        aggregate: snapshot.claims_aggregate.key.to_bytes(),
        rent_credit: snapshot.rent_credit.key.to_bytes(),
        request_digest,
        pre_resource_digest,
        post_resource_digest,
        generation: authenticated.market.identity.generation,
        pre_revision: authenticated.claims.revision,
        post_revision,
        liability_units: 0,
        refund_lamports: snapshot.claims_aggregate.lamports,
        claim_count: authenticated.claims.claim_count,
    })
    .map_err(|_| MarketRetirementOperatorErrorV1::Claims)
}

fn projected_custody_receipts(
    snapshot: &MarketRetirementSnapshotV1,
    authenticated: AuthenticatedRetirementV1,
    close_vault: CustodyRequestV1,
    close_vault_bytes: &[u8],
    close_replay: CustodyRequestV1,
    close_replay_bytes: &[u8],
) -> Result<([u8; 32], [u8; 32]), MarketRetirementOperatorErrorV1> {
    let close_vault_digest = hash(close_vault_bytes).to_bytes();
    let close_vault_poststate = custody_poststate(
        close_vault_digest,
        snapshot.hoard_vault.key,
        snapshot.rent_credit.key,
        snapshot.hoard_vault.lamports,
    );
    let replay_after_vault = authenticated
        .replay
        .advance(close_vault, close_vault_digest, close_vault_poststate)
        .map_err(|_| MarketRetirementOperatorErrorV1::Custody)?;
    let replay_after_vault_bytes = replay_after_vault
        .to_bytes()
        .map_err(|_| MarketRetirementOperatorErrorV1::Custody)?;
    let close_vault_receipt = CustodyReceiptV1::new(
        close_vault,
        close_vault_digest,
        ReceiptEvidenceV1 {
            source_before: 0,
            source_after: 0,
            destination_before: 0,
            destination_after: 0,
            poststate_commitment: close_vault_poststate,
            replay_state_digest: hash(&replay_after_vault_bytes).to_bytes(),
        },
    )
    .map_err(|_| MarketRetirementOperatorErrorV1::Custody)?;
    let close_vault_receipt_digest = hash(
        &close_vault_receipt
            .to_bytes()
            .map_err(|_| MarketRetirementOperatorErrorV1::Custody)?,
    )
    .to_bytes();

    let close_replay_digest = hash(close_replay_bytes).to_bytes();
    let close_replay_poststate = custody_poststate(
        close_replay_digest,
        snapshot.custody_replay.key,
        snapshot.rent_credit.key,
        snapshot.custody_replay.lamports,
    );
    replay_after_vault
        .advance(close_replay, close_replay_digest, close_replay_poststate)
        .map_err(|_| MarketRetirementOperatorErrorV1::Custody)?;
    let close_replay_receipt = CustodyReceiptV1::new(
        close_replay,
        close_replay_digest,
        ReceiptEvidenceV1 {
            source_before: 0,
            source_after: 0,
            destination_before: 0,
            destination_after: 0,
            poststate_commitment: close_replay_poststate,
            replay_state_digest: hash(&[]).to_bytes(),
        },
    )
    .map_err(|_| MarketRetirementOperatorErrorV1::Custody)?;
    let close_replay_receipt_digest = hash(
        &close_replay_receipt
            .to_bytes()
            .map_err(|_| MarketRetirementOperatorErrorV1::Custody)?,
    )
    .to_bytes();
    Ok((close_vault_receipt_digest, close_replay_receipt_digest))
}

fn custody_poststate(
    request_digest: [u8; 32],
    source: Pubkey,
    destination: Pubkey,
    rent_lamports: u64,
) -> [u8; 32] {
    hashv(&[
        CUSTODY_POSTSTATE_DOMAIN_V1,
        &request_digest,
        source.as_ref(),
        destination.as_ref(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &rent_lamports.to_le_bytes(),
    ])
    .to_bytes()
}

fn caller_authority(
    release_set: [u8; 32],
    market: Pubkey,
    context: [u8; 32],
    request_bytes: &[u8],
    core_program: Pubkey,
) -> Result<Pubkey, MarketRetirementOperatorErrorV1> {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        release_set,
        market.to_bytes(),
        ExecutionRoleV1::Core,
        context,
        hash(request_bytes).to_bytes(),
    )
    .map_err(|_| MarketRetirementOperatorErrorV1::Frame)?;
    Ok(Pubkey::find_program_address(&seeds.as_slices(), &core_program).0)
}

fn core_accounts(
    snapshot: &MarketRetirementSnapshotV1,
    claims_authority: Pubkey,
    close_vault_authority: Pubkey,
    close_replay_authority: Pubkey,
    rent_close_authority: Pubkey,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(snapshot.market.key, false),
        AccountMeta::new(snapshot.rent_credit.key, false),
        AccountMeta::new_readonly(snapshot.activation_cache.key, false),
        AccountMeta::new_readonly(snapshot.registry_program.key, false),
        AccountMeta::new_readonly(snapshot.core_program.key, false),
        AccountMeta::new_readonly(snapshot.core_programdata.key, false),
        AccountMeta::new_readonly(snapshot.claims_program.key, false),
        AccountMeta::new_readonly(snapshot.claims_programdata.key, false),
        AccountMeta::new_readonly(snapshot.resolution_program.key, false),
        AccountMeta::new_readonly(snapshot.resolution_programdata.key, false),
        AccountMeta::new_readonly(snapshot.custody_program.key, false),
        AccountMeta::new_readonly(snapshot.custody_programdata.key, false),
        AccountMeta::new_readonly(snapshot.rent_program.key, false),
        AccountMeta::new_readonly(snapshot.source_receipt.key, false),
        AccountMeta::new(snapshot.claims_aggregate.key, false),
        AccountMeta::new(snapshot.custody_replay.key, false),
        AccountMeta::new(snapshot.hoard_vault.key, false),
        AccountMeta::new_readonly(snapshot.custody_authority.key, false),
        AccountMeta::new_readonly(snapshot.collateral_mint.key, false),
        AccountMeta::new_readonly(snapshot.collateral_token_program.key, false),
        AccountMeta::new_readonly(snapshot.realm_raw.key, false),
        AccountMeta::new_readonly(snapshot.realm_staging.key, false),
        AccountMeta::new_readonly(claims_authority, false),
        AccountMeta::new_readonly(close_vault_authority, false),
        AccountMeta::new_readonly(close_replay_authority, false),
        AccountMeta::new_readonly(snapshot.infrastructure_profile.key, false),
        AccountMeta::new_readonly(snapshot.registry_artifact_raw.key, false),
        AccountMeta::new_readonly(snapshot.registry_artifact_staging.key, false),
        AccountMeta::new_readonly(snapshot.registry_programdata.key, false),
        AccountMeta::new_readonly(snapshot.rent_artifact_raw.key, false),
        AccountMeta::new_readonly(snapshot.rent_artifact_staging.key, false),
        AccountMeta::new_readonly(snapshot.rent_programdata.key, false),
        AccountMeta::new_readonly(snapshot.rent_sysvar.key, false),
        AccountMeta::new(snapshot.refund_wallet.key, false),
        AccountMeta::new_readonly(rent_close_authority, false),
    ]
}

fn wrap_registry_continuation(
    snapshot: &MarketRetirementSnapshotV1,
    direct: &Instruction,
) -> Result<(Instruction, Pubkey, RegistryContinuationRequestV1), MarketRetirementOperatorErrorV1> {
    let release_set = ContentId::new(
        CoreState::decode(&snapshot.market.data)
            .map_err(|_| MarketRetirementOperatorErrorV1::Market)?
            .identity
            .selected_release_set
            .to_bytes(),
    )
    .map_err(|_| MarketRetirementOperatorErrorV1::Encoding)?;
    let activation_digest = ContentId::new(hash(&snapshot.activation_cache.data).to_bytes())
        .map_err(|_| MarketRetirementOperatorErrorV1::Encoding)?;
    let instruction_digest = ContentId::new(hash(&direct.data).to_bytes())
        .map_err(|_| MarketRetirementOperatorErrorV1::Encoding)?;
    let instruction_len =
        u32::try_from(direct.data.len()).map_err(|_| MarketRetirementOperatorErrorV1::Encoding)?;
    let roles = [
        ExecutionRoleV1::Core,
        ExecutionRoleV1::Claims,
        ExecutionRoleV1::Resolution,
        ExecutionRoleV1::Custody,
    ];
    let continuation = RegistryContinuationRequestV1::new(
        release_set,
        activation_digest,
        instruction_digest,
        instruction_len,
        ExecutionRoleV1::Core,
        &roles,
    )
    .map_err(|_| MarketRetirementOperatorErrorV1::Encoding)?;
    let batch = continuation
        .role_batch_request()
        .map_err(|_| MarketRetirementOperatorErrorV1::Encoding)?;
    let batch_digest = ContentId::new(hash(&batch.to_bytes()).to_bytes())
        .map_err(|_| MarketRetirementOperatorErrorV1::Encoding)?;
    let seeds = RegistryContinuationAdmissionSeedsV1::new(
        continuation,
        snapshot.activation_cache.key.to_bytes(),
        batch_digest,
    )
    .map_err(|_| MarketRetirementOperatorErrorV1::Encoding)?;
    let release = seeds.release_set();
    let cache = seeds.activation_cache();
    let role_batch = seeds.batch_request_digest();
    let role_mask = seeds.role_mask();
    let role = seeds.continuation_role();
    let continuation_digest = seeds.continuation_digest();
    let admission = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache.as_slice(),
            role_batch.as_slice(),
            role_mask.as_slice(),
            role.as_slice(),
            continuation_digest.as_slice(),
        ],
        &snapshot.registry_program.key,
    )
    .0;
    if direct.accounts.iter().any(|meta| meta.pubkey == admission) {
        return Err(MarketRetirementOperatorErrorV1::Frame);
    }

    let mut child_accounts = direct.accounts.clone();
    child_accounts.push(AccountMeta::new_readonly(admission, false));
    let mut accounts = Vec::with_capacity(MARKET_RETIREMENT_ACCOUNT_COUNT_V1);
    accounts.extend([
        AccountMeta::new_readonly(snapshot.activation_cache.key, false),
        AccountMeta::new_readonly(snapshot.core_program.key, false),
        AccountMeta::new_readonly(snapshot.core_programdata.key, false),
        AccountMeta::new_readonly(snapshot.claims_program.key, false),
        AccountMeta::new_readonly(snapshot.claims_programdata.key, false),
        AccountMeta::new_readonly(snapshot.resolution_program.key, false),
        AccountMeta::new_readonly(snapshot.resolution_programdata.key, false),
        AccountMeta::new_readonly(snapshot.custody_program.key, false),
        AccountMeta::new_readonly(snapshot.custody_programdata.key, false),
        AccountMeta::new_readonly(admission, false),
    ]);
    accounts.extend(child_accounts);
    if accounts.len() != MARKET_RETIREMENT_ACCOUNT_COUNT_V1 {
        return Err(MarketRetirementOperatorErrorV1::Frame);
    }
    let mut data = Vec::with_capacity(
        REGISTRY_CONTINUATION_REQUEST_BYTES_V1 + MARKET_RETIREMENT_CORE_INSTRUCTION_BYTES_V1,
    );
    data.extend_from_slice(&continuation.to_bytes());
    data.extend_from_slice(&direct.data);
    Ok((
        Instruction {
            program_id: snapshot.registry_program.key,
            accounts,
            data,
        },
        admission,
        continuation,
    ))
}
