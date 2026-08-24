// SPDX-License-Identifier: AGPL-3.0-or-later
//! Physical terminal owners for the Market ClaimLedger and Hoard shared cores.
//!
//! Product does not authorize these closes with a caller projection. This
//! module hostile-reopens the exact writable Retiring RootV3, authenticates the
//! current RegistryV5/BundleV7 and GraphV4/ScheduleV4 owners, proves that both
//! liability aggregates and the collateral Hoard are empty, closes the
//! ClaimLedger first and the Hoard state/token vault second, and returns two
//! distinct move-only receipts. Product must consume those receipts into the
//! two consecutive RootV3 shared-core transitions in the same instruction.

use crate::accounts::{expect_pda, require, Outcome};
use crate::collateral_release::{
    authenticate_collateral_release_deployment_v2, authenticate_realm_collateral_v2,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{transfer_data, SYSTEM_PROGRAM_ID};
use crate::instructions::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, AuthenticatedMarketLifecycleRootV3,
};
use crate::instructions::product_series_current::AuthenticatedRegistryCapabilityV5;
use crate::instructions::product_source_current::AuthenticatedCompiledProductSeriesBundleV7;
use crate::seeds;
use clutch_collateral_adapter_v2::{
    admit_collateral_account_v2, refine_market_collateral_v2, ClaimLedgerV3, HoardV2, Id,
    MarketCollateralBindingV2, MarketLiabilityLifecycleV1, RuntimeAccountViewV2,
    TokenAccountRoleV2, CLAIM_LEDGER_V3_BYTES, HOARD_V2_BYTES,
};
use clutch_product_series::{
    ContentId, MarketFoundationAccountGraphV4, MarketFoundationScheduleV4,
    MarketFoundationSlotV4, MarketInstanceV2Id, MarketLifecycleBindingV3,
    MarketLifecyclePhaseV3, MarketSharedCoreV3,
};
use clutch_retirement::PositionV3Sha256Backend;
use clutch_solana_layout::product_series::MarketLifecycleRootAccountV3;
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const CLAIM_LEDGER_PHYSICAL_TERMINAL_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/claim-ledger-physical-terminal/v3\0";
const HOARD_PHYSICAL_TERMINAL_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/hoard-physical-terminal/v3\0";
const LIABILITY_ACCOUNT_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/market-liability-account-authentication/v3\0";
const LIABILITY_ACCOUNT_CLOSED_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/market-liability-account-closed/v3\0";
const HOARD_TOKEN_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/hoard-token-authentication/v3\0";
const HOARD_TOKEN_CLOSED_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/hoard-token-closed/v3\0";
const SHARED_CORE_RELEASE_AUTHORITY_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/shared-core-release-authority/v3\0";

#[derive(Clone, Copy, Debug)]
struct RuntimeSha256;

impl PositionV3Sha256Backend for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

/// Exact RootV3 preauthorization retained independently by both physical
/// terminal receipts. It is data, not authority: the unique borrowed hostile
/// Root authentication remains owned by the ordered pair below.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MarketSharedCoreRootPreauthorizationV3 {
    root_account: Pubkey,
    market_binding_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    root_data_id: ContentId,
    root_authentication_id: ContentId,
    root_semantic_id: ContentId,
    root_transition_sequence: u64,
}

impl MarketSharedCoreRootPreauthorizationV3 {
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn market_binding_id(&self) -> ContentId { self.market_binding_id }
    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    pub(crate) const fn generation(&self) -> u64 { self.generation }
    pub(crate) const fn root_data_id(&self) -> ContentId { self.root_data_id }
    pub(crate) const fn root_authentication_id(&self) -> ContentId {
        self.root_authentication_id
    }
    pub(crate) const fn root_semantic_id(&self) -> ContentId { self.root_semantic_id }
    pub(crate) const fn root_transition_sequence(&self) -> u64 {
        self.root_transition_sequence
    }
}

/// Move-only proof that the canonical ClaimLedger is terminal and physically
/// deleted with its exact rent principal and donation residue conserved.
#[derive(Debug)]
pub(crate) struct AuthenticatedClaimLedgerPhysicalTerminalV3 {
    id: ContentId,
    root: MarketSharedCoreRootPreauthorizationV3,
    owner_account_id: ContentId,
    owner_release_id: ContentId,
    release_authority_id: ContentId,
    root_transition_sequence: u64,
    account_data_before_id: ContentId,
    account_semantic_before_id: ContentId,
    account_authentication_before_id: ContentId,
    account_closed_state_id: ContentId,
    refund_owner: Pubkey,
    neutral_sink: Pubkey,
    principal_lamports: u64,
    donation_lamports: u64,
    refund_lamports_before: u64,
    refund_lamports_after: u64,
    sink_lamports_before: u64,
    sink_lamports_after: u64,
}

impl AuthenticatedClaimLedgerPhysicalTerminalV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn owner(&self) -> MarketSharedCoreV3 {
        MarketSharedCoreV3::ClaimLedger
    }
    pub(crate) const fn root(&self) -> MarketSharedCoreRootPreauthorizationV3 { self.root }
    pub(crate) const fn owner_account_id(&self) -> ContentId { self.owner_account_id }
    pub(crate) const fn owner_release_id(&self) -> ContentId { self.owner_release_id }
    pub(crate) const fn release_authority_id(&self) -> ContentId {
        self.release_authority_id
    }
    pub(crate) const fn root_transition_sequence(&self) -> u64 {
        self.root_transition_sequence
    }
    pub(crate) const fn account_data_before_id(&self) -> ContentId {
        self.account_data_before_id
    }
    pub(crate) const fn account_semantic_before_id(&self) -> ContentId {
        self.account_semantic_before_id
    }
    pub(crate) const fn account_authentication_before_id(&self) -> ContentId {
        self.account_authentication_before_id
    }
    pub(crate) const fn account_closed_state_id(&self) -> ContentId {
        self.account_closed_state_id
    }
    pub(crate) const fn refund_owner(&self) -> Pubkey { self.refund_owner }
    pub(crate) const fn neutral_sink(&self) -> Pubkey { self.neutral_sink }
    pub(crate) const fn principal_lamports(&self) -> u64 { self.principal_lamports }
    pub(crate) const fn donation_lamports(&self) -> u64 { self.donation_lamports }
    pub(crate) const fn refund_lamports_before(&self) -> u64 { self.refund_lamports_before }
    pub(crate) const fn refund_lamports_after(&self) -> u64 { self.refund_lamports_after }
    pub(crate) const fn sink_lamports_before(&self) -> u64 { self.sink_lamports_before }
    pub(crate) const fn sink_lamports_after(&self) -> u64 { self.sink_lamports_after }
}

/// Move-only proof that the canonical zero-liability Hoard state and empty
/// collateral token vault were both physically closed with exact conservation.
#[derive(Debug)]
pub(crate) struct AuthenticatedHoardPhysicalTerminalV3 {
    id: ContentId,
    root: MarketSharedCoreRootPreauthorizationV3,
    owner_account_id: ContentId,
    owner_release_id: ContentId,
    release_authority_id: ContentId,
    root_transition_sequence: u64,
    account_data_before_id: ContentId,
    account_semantic_before_id: ContentId,
    account_authentication_before_id: ContentId,
    account_closed_state_id: ContentId,
    token_account_id: ContentId,
    token_data_before_id: ContentId,
    token_authentication_before_id: ContentId,
    token_closed_state_id: ContentId,
    collateral_release_deployment_receipt_id: ContentId,
    foundation_vault: Pubkey,
    foundation_vault_lamports_before: u64,
    foundation_vault_lamports_after: u64,
    refund_owner: Pubkey,
    neutral_sink: Pubkey,
    state_principal_lamports: u64,
    state_donation_lamports: u64,
    token_principal_lamports: u64,
    token_donation_lamports: u64,
    refund_lamports_before: u64,
    refund_lamports_after: u64,
    sink_lamports_before: u64,
    sink_lamports_after: u64,
}

impl AuthenticatedHoardPhysicalTerminalV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn owner(&self) -> MarketSharedCoreV3 { MarketSharedCoreV3::Hoard }
    pub(crate) const fn root(&self) -> MarketSharedCoreRootPreauthorizationV3 { self.root }
    pub(crate) const fn owner_account_id(&self) -> ContentId { self.owner_account_id }
    pub(crate) const fn owner_release_id(&self) -> ContentId { self.owner_release_id }
    pub(crate) const fn release_authority_id(&self) -> ContentId {
        self.release_authority_id
    }
    pub(crate) const fn root_transition_sequence(&self) -> u64 {
        self.root_transition_sequence
    }
    pub(crate) const fn account_data_before_id(&self) -> ContentId {
        self.account_data_before_id
    }
    pub(crate) const fn account_semantic_before_id(&self) -> ContentId {
        self.account_semantic_before_id
    }
    pub(crate) const fn account_authentication_before_id(&self) -> ContentId {
        self.account_authentication_before_id
    }
    pub(crate) const fn account_closed_state_id(&self) -> ContentId {
        self.account_closed_state_id
    }
    pub(crate) const fn token_account_id(&self) -> ContentId { self.token_account_id }
    pub(crate) const fn token_data_before_id(&self) -> ContentId { self.token_data_before_id }
    pub(crate) const fn token_authentication_before_id(&self) -> ContentId {
        self.token_authentication_before_id
    }
    pub(crate) const fn token_closed_state_id(&self) -> ContentId {
        self.token_closed_state_id
    }
    pub(crate) const fn collateral_release_deployment_receipt_id(&self) -> ContentId {
        self.collateral_release_deployment_receipt_id
    }
    pub(crate) const fn foundation_vault(&self) -> Pubkey { self.foundation_vault }
    pub(crate) const fn foundation_vault_lamports_before(&self) -> u64 {
        self.foundation_vault_lamports_before
    }
    pub(crate) const fn foundation_vault_lamports_after(&self) -> u64 {
        self.foundation_vault_lamports_after
    }
    pub(crate) const fn refund_owner(&self) -> Pubkey { self.refund_owner }
    pub(crate) const fn neutral_sink(&self) -> Pubkey { self.neutral_sink }
    pub(crate) const fn state_principal_lamports(&self) -> u64 {
        self.state_principal_lamports
    }
    pub(crate) const fn state_donation_lamports(&self) -> u64 {
        self.state_donation_lamports
    }
    pub(crate) const fn token_principal_lamports(&self) -> u64 {
        self.token_principal_lamports
    }
    pub(crate) const fn token_donation_lamports(&self) -> u64 {
        self.token_donation_lamports
    }
    pub(crate) const fn refund_lamports_before(&self) -> u64 { self.refund_lamports_before }
    pub(crate) const fn refund_lamports_after(&self) -> u64 { self.refund_lamports_after }
    pub(crate) const fn sink_lamports_before(&self) -> u64 { self.sink_lamports_before }
    pub(crate) const fn sink_lamports_after(&self) -> u64 { self.sink_lamports_after }
}

/// Ordered move-only pair. The borrowed hostile RootV3 authentication is
/// returned with the two child owners so Product can perform the adjacent
/// ClaimLedger then Hoard Root transitions without reconstituting authority.
#[derive(Debug)]
pub(crate) struct AuthenticatedMarketLiabilityPhysicalTerminalsV3<'root> {
    root: AuthenticatedMarketLifecycleRootV3<'root>,
    claim_ledger: AuthenticatedClaimLedgerPhysicalTerminalV3,
    hoard: AuthenticatedHoardPhysicalTerminalV3,
}

impl<'root> AuthenticatedMarketLiabilityPhysicalTerminalsV3<'root> {
    pub(crate) fn into_product_parts(
        self,
    ) -> (
        AuthenticatedMarketLifecycleRootV3<'root>,
        AuthenticatedClaimLedgerPhysicalTerminalV3,
        AuthenticatedHoardPhysicalTerminalV3,
    ) {
        (self.root, self.claim_ledger, self.hoard)
    }
}

/// Physically close both mandatory collateral-liability cores before Product
/// writes either RootV3 shared-core receipt. Any later Product refusal rolls
/// every close and credit back atomically.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn close_market_liability_shared_cores_v3<'root, 'info>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'info>,
    claim_ledger_account: &AccountInfo<'info>,
    hoard_account: &AccountInfo<'info>,
    hoard_token_account: &AccountInfo<'info>,
    hoard_authority: &AccountInfo<'info>,
    foundation_vault: &AccountInfo<'info>,
    refund_owner: &AccountInfo<'info>,
    neutral_sink: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    realm_account: &AccountInfo<'info>,
    profile_account: &AccountInfo<'info>,
    collateral_policy_account: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    token_programdata: &AccountInfo<'info>,
    registry: &AuthenticatedRegistryCapabilityV5,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
    schedule: &MarketFoundationScheduleV4,
    graph: &MarketFoundationAccountGraphV4,
    root_decode: &'root mut MarketLifecycleRootAccountV3,
) -> Outcome<AuthenticatedMarketLiabilityPhysicalTerminalsV3<'root>> {
    require_distinct_roles(&[
        root_account,
        claim_ledger_account,
        hoard_account,
        hoard_token_account,
        hoard_authority,
        foundation_vault,
        refund_owner,
        neutral_sink,
        system_program,
        realm_account,
        profile_account,
        collateral_policy_account,
        token_program,
        token_programdata,
    ])?;
    require_external_authority_separation(
        &[
            root_account,
            claim_ledger_account,
            hoard_account,
            hoard_token_account,
            hoard_authority,
            foundation_vault,
            refund_owner,
            neutral_sink,
            system_program,
            realm_account,
            profile_account,
            collateral_policy_account,
            token_program,
            token_programdata,
        ],
        registry,
        bundle,
    )?;
    require(
        *system_program.key == SYSTEM_PROGRAM_ID
            && !system_program.is_signer
            && !system_program.is_writable
            && system_program.executable,
        ClutchError::MismatchedState,
    )?;
    require_system_recipient(refund_owner)?;
    require_system_recipient(neutral_sink)?;

    schedule
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    graph
        .validate(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        graph.market_instance_id,
        graph.generation,
        true,
        root_decode,
    )?;
    let state = root.state();
    let binding = state.binding_ref();
    let root_sequence = state.transition_sequence();
    let claim_sequence = root_sequence.checked_add(1).ok_or(ClutchError::Arithmetic)?;
    let hoard_sequence = claim_sequence.checked_add(1).ok_or(ClutchError::Arithmetic)?;
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        state.phase() == MarketLifecyclePhaseV3::Retiring
            && state.shared_core_terminal_receipt(MarketSharedCoreV3::ClaimLedger).is_zero()
            && state.shared_core_terminal_receipt(MarketSharedCoreV3::Hoard).is_zero()
            && !state.resolution_semantic_id().is_zero()
            && !state.resolution_data_id().is_zero()
            && !state.resolution_activation_receipt_id().is_zero()
            && binding.foundation_schedule_id == schedule_id
            && binding.foundation_account_graph_id == graph_id
            && binding.market_instance_id == graph.market_instance_id
            && binding.generation == graph.generation
            && binding.outcome_count == schedule.outcome_count,
        ClutchError::MismatchedState,
    )?;
    require_current_release_join(registry, bundle, binding)?;

    let market = binding.market_instance_id.bytes();
    let (expected_claim, claim_bump) = seeds::claim_ledger_v3_pda(program_id, &market);
    let (expected_hoard, hoard_bump) = seeds::hoard_v2_pda(program_id, &market);
    let (expected_hoard_authority, hoard_authority_bump) =
        seeds::hoard_authority_v2_pda(program_id, &market);
    let (expected_hoard_token, _) = seeds::hoard_token_v2_pda(program_id, &market);
    let (expected_foundation_vault, foundation_vault_bump) =
        seeds::product_market_foundation_vault_pda(program_id, &market, binding.generation);
    require(
        graph.account(MarketFoundationSlotV4::ClaimLedger)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == account_id(claim_ledger_account.key)
            && graph.account(MarketFoundationSlotV4::Hoard)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == account_id(hoard_account.key)
            && graph.account(MarketFoundationSlotV4::HoardCollateralVault)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == account_id(hoard_token_account.key)
            && binding.foundation_vault_id == account_id(foundation_vault.key)
            && expected_claim == *claim_ledger_account.key
            && expected_hoard == *hoard_account.key
            && expected_hoard_authority == *hoard_authority.key
            && expected_hoard_token == *hoard_token_account.key
            && expected_foundation_vault == *foundation_vault.key,
        ClutchError::WrongPda,
    )?;
    require(
        !hoard_authority.is_signer
            && !hoard_authority.is_writable
            && !hoard_authority.executable
            && hoard_authority.owner == &SYSTEM_PROGRAM_ID
            && hoard_authority.data_len() == 0
            && !foundation_vault.is_signer
            && foundation_vault.is_writable
            && !foundation_vault.executable
            && foundation_vault.owner == &SYSTEM_PROGRAM_ID
            && foundation_vault.data_len() == 0
            && hoard_token_account.is_writable
            && !hoard_token_account.is_signer
            && !hoard_token_account.executable,
        ClutchError::MismatchedState,
    )?;

    require_program_state(claim_ledger_account, program_id, CLAIM_LEDGER_V3_BYTES)?;
    require_program_state(hoard_account, program_id, HOARD_V2_BYTES)?;
    let claim_data = claim_ledger_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let claim = ClaimLedgerV3::decode(&claim_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let claim_data_id = hash_data(&claim_data);
    drop(claim_data);
    let hoard_data = hoard_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let hoard = HoardV2::decode(&hoard_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let hoard_data_id = hash_data(&hoard_data);
    drop(hoard_data);
    expect_pda(claim_ledger_account.key, (expected_claim, claim_bump), Some(claim.stored_bump))?;
    expect_pda(hoard_account.key, (expected_hoard, hoard_bump), Some(hoard.stored_bump))?;
    require_zero_liability_state(claim, hoard, binding)?;

    let claim_semantic_id = content_id(
        claim
            .semantic_id(&RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
    );
    let hoard_semantic_id = content_id(
        hoard
            .semantic_id(&RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
    );
    let claim_lamports = claim_ledger_account.lamports();
    let hoard_lamports = hoard_account.lamports();
    let claim_principal = schedule.slot_principal_lamports
        [MarketFoundationSlotV4::ClaimLedger.index().map_err(|_| ClutchError::Arithmetic)?];
    let hoard_principal = schedule.slot_principal_lamports
        [MarketFoundationSlotV4::Hoard.index().map_err(|_| ClutchError::Arithmetic)?];
    let token_principal = schedule.slot_principal_lamports
        [MarketFoundationSlotV4::HoardCollateralVault.index().map_err(|_| ClutchError::Arithmetic)?];
    require(
        claim.rent.refundable_principal() == claim_principal
            && hoard.rent.refundable_principal() == hoard_principal
            && claim.rent.payer().bytes() == refund_owner.key.to_bytes()
            && hoard.rent.payer().bytes() == refund_owner.key.to_bytes()
            && account_id(refund_owner.key) == state.capital().rent_refund_owner
            && account_id(neutral_sink.key) == state.capital().neutral_lamport_sink
            && claim_lamports >= claim_principal
            && hoard_lamports >= hoard_principal,
        ClutchError::MismatchedState,
    )?;
    let claim_donation = claim_lamports.checked_sub(claim_principal).ok_or(ClutchError::Arithmetic)?;
    let hoard_donation = hoard_lamports.checked_sub(hoard_principal).ok_or(ClutchError::Arithmetic)?;
    require(
        claim_donation >= claim.rent.donation_floor()
            && hoard_donation >= hoard.rent.donation_floor(),
        ClutchError::MismatchedState,
    )?;

    let bound_realm = authenticate_realm_collateral_v2(
        program_id,
        realm_account,
        profile_account,
        collateral_policy_account,
        token_program,
    )?;
    let realm = bound_realm.realm();
    let release = bound_realm.release();
    let release_id = release
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let bound = refine_market_collateral_v2(
        bound_realm,
        MarketCollateralBindingV2 {
            market: collateral_id(binding.market_instance_id.content_id()),
            realm: realm.realm,
            profile: realm.profile,
            collateral_cap_atoms: hoard.collateral_cap_atoms,
            hoard_authority: collateral_pubkey(hoard_authority.key),
            hoard_token_account: collateral_pubkey(hoard_token_account.key),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let deployment = authenticate_collateral_release_deployment_v2(
        release,
        token_program,
        token_programdata,
    )?;
    require(
        realm.realm.bytes() == binding.realm_id.bytes()
            && realm.profile.bytes() == binding.collateral_profile_id.bytes()
            && bound.policy_id().bytes() == binding.collateral_policy_id.bytes()
            && release_id.bytes() == binding.collateral_release_id.bytes()
            && hoard.realm_id == realm.realm
            && hoard.profile_id == realm.profile
            && hoard.collateral_policy_id == bound.policy_id()
            && hoard.collateral_release_id == release_id
            && hoard.authority == collateral_pubkey(hoard_authority.key)
            && hoard.token_account == collateral_pubkey(hoard_token_account.key),
        ClutchError::MismatchedState,
    )?;
    let token_data = hoard_token_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let token_observation = admit_collateral_account_v2(
        bound,
        RuntimeAccountViewV2 {
            key: collateral_pubkey(hoard_token_account.key),
            owner_program: collateral_pubkey(hoard_token_account.owner),
            data: &token_data,
            is_signer: hoard_token_account.is_signer,
            is_writable: hoard_token_account.is_writable,
            executable: hoard_token_account.executable,
        },
        TokenAccountRoleV2::Hoard,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(token_observation.amount_atoms == 0, ClutchError::MismatchedState)?;
    let token_data_id = hash_data(&token_data);
    drop(token_data);
    let token_lamports = hoard_token_account.lamports();
    require(token_lamports >= token_principal, ClutchError::MismatchedState)?;
    let token_donation = token_lamports.checked_sub(token_principal).ok_or(ClutchError::Arithmetic)?;

    let owner_release_id = registry.registry_release_id();
    let release_authority_id = hashv(&[
        SHARED_CORE_RELEASE_AUTHORITY_DOMAIN_V3,
        program_id.as_ref(),
        registry.series_registry_account().as_ref(),
        &registry.id().bytes(),
        &registry.series_registry_authentication_id().bytes(),
        bundle.artifact_account().as_ref(),
        &bundle.bundle_id().bytes(),
        &owner_release_id.bytes(),
        &registry.capability_profile_id().bytes(),
    ]);
    require_live(release_authority_id)?;
    let root_facts = MarketSharedCoreRootPreauthorizationV3 {
        root_account: root.account(),
        market_binding_id: root.binding_id(),
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        root_data_id: root.data_id(),
        root_authentication_id: root.authentication_id(),
        root_semantic_id: root.semantic_id(),
        root_transition_sequence: root_sequence,
    };
    let claim_authentication_id = liability_account_authentication_id(
        program_id,
        claim_ledger_account,
        claim_data_id,
        claim_semantic_id,
        claim_lamports,
    );
    let hoard_authentication_id = liability_account_authentication_id(
        program_id,
        hoard_account,
        hoard_data_id,
        hoard_semantic_id,
        hoard_lamports,
    );
    let token_authentication_id = hashv(&[
        HOARD_TOKEN_AUTHENTICATION_DOMAIN_V3,
        hoard_token_account.key.as_ref(),
        hoard_token_account.owner.as_ref(),
        &token_data_id.bytes(),
        &token_lamports.to_le_bytes(),
        &deployment.receipt_id().bytes(),
        &[u8::from(hoard_token_account.is_writable)],
    ]);
    require_live(claim_authentication_id)?;
    require_live(hoard_authentication_id)?;
    require_live(token_authentication_id)?;

    let refund_before = refund_owner.lamports();
    let sink_before = neutral_sink.lamports();
    let foundation_vault_before = foundation_vault.lamports();
    let claim_refund_after = refund_before.checked_add(claim_principal).ok_or(ClutchError::Arithmetic)?;
    let claim_sink_after = sink_before.checked_add(claim_donation).ok_or(ClutchError::Arithmetic)?;
    let final_refund_after = claim_refund_after
        .checked_add(hoard_principal)
        .and_then(|value| value.checked_add(token_principal))
        .ok_or(ClutchError::Arithmetic)?;
    let final_sink_after = claim_sink_after
        .checked_add(hoard_donation)
        .and_then(|value| value.checked_add(token_donation))
        .ok_or(ClutchError::Arithmetic)?;
    foundation_vault_before.checked_add(token_lamports).ok_or(ClutchError::Arithmetic)?;
    preflight_writes(&[
        claim_ledger_account,
        hoard_account,
        hoard_token_account,
        foundation_vault,
        refund_owner,
        neutral_sink,
    ])?;

    close_program_state_with_split(
        claim_ledger_account,
        refund_owner,
        neutral_sink,
        claim_principal,
        claim_donation,
        claim_refund_after,
        claim_sink_after,
    )?;
    let claim_closed_id = closed_program_account_id(claim_ledger_account);
    require_live(claim_closed_id)?;
    let claim_receipt_id = hashv(&[
        CLAIM_LEDGER_PHYSICAL_TERMINAL_DOMAIN_V3,
        program_id.as_ref(),
        &root_facts.market_binding_id.bytes(),
        root_account.key.as_ref(),
        &root_facts.root_data_id.bytes(),
        &root_facts.root_authentication_id.bytes(),
        &root_facts.root_semantic_id.bytes(),
        &root_sequence.to_le_bytes(),
        &claim_sequence.to_le_bytes(),
        claim_ledger_account.key.as_ref(),
        &owner_release_id.bytes(),
        &release_authority_id.bytes(),
        &claim_data_id.bytes(),
        &claim_semantic_id.bytes(),
        &claim_authentication_id.bytes(),
        &claim_closed_id.bytes(),
        refund_owner.key.as_ref(),
        neutral_sink.key.as_ref(),
        &claim_principal.to_le_bytes(),
        &claim_donation.to_le_bytes(),
        &refund_before.to_le_bytes(),
        &claim_refund_after.to_le_bytes(),
        &sink_before.to_le_bytes(),
        &claim_sink_after.to_le_bytes(),
    ]);
    require_live(claim_receipt_id)?;

    close_hoard_token_vault(
        token_program,
        hoard_token_account,
        foundation_vault,
        hoard_authority,
        hoard_authority_bump,
        binding.market_instance_id,
    )?;
    require(
        hoard_token_account.lamports() == 0
            && foundation_vault.lamports()
                == foundation_vault_before
                    .checked_add(token_lamports)
                    .ok_or(ClutchError::Arithmetic)?,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    require_closed_token_reopen(bound, hoard_token_account)?;
    transfer_foundation_vault_split(
        system_program,
        foundation_vault,
        refund_owner,
        neutral_sink,
        binding.market_instance_id,
        binding.generation,
        foundation_vault_bump,
        token_principal,
        token_donation,
    )?;
    require(
        foundation_vault.lamports() == foundation_vault_before,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let refund_after_token = claim_refund_after
        .checked_add(token_principal)
        .ok_or(ClutchError::Arithmetic)?;
    let sink_after_token = claim_sink_after
        .checked_add(token_donation)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        refund_owner.lamports() == refund_after_token
            && neutral_sink.lamports() == sink_after_token,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    close_program_state_with_split(
        hoard_account,
        refund_owner,
        neutral_sink,
        hoard_principal,
        hoard_donation,
        final_refund_after,
        final_sink_after,
    )?;
    let hoard_closed_id = closed_program_account_id(hoard_account);
    require_live(hoard_closed_id)?;
    let token_closed_id = closed_token_account_id(hoard_token_account)?;
    let hoard_receipt_id = hashv(&[
        HOARD_PHYSICAL_TERMINAL_DOMAIN_V3,
        program_id.as_ref(),
        &root_facts.market_binding_id.bytes(),
        root_account.key.as_ref(),
        &root_facts.root_data_id.bytes(),
        &root_facts.root_authentication_id.bytes(),
        &root_facts.root_semantic_id.bytes(),
        &root_sequence.to_le_bytes(),
        &hoard_sequence.to_le_bytes(),
        hoard_account.key.as_ref(),
        hoard_token_account.key.as_ref(),
        &owner_release_id.bytes(),
        &release_authority_id.bytes(),
        &hoard_data_id.bytes(),
        &hoard_semantic_id.bytes(),
        &hoard_authentication_id.bytes(),
        &hoard_closed_id.bytes(),
        &token_data_id.bytes(),
        &token_authentication_id.bytes(),
        &token_closed_id.bytes(),
        &deployment.receipt_id().bytes(),
        foundation_vault.key.as_ref(),
        &foundation_vault_before.to_le_bytes(),
        refund_owner.key.as_ref(),
        neutral_sink.key.as_ref(),
        &hoard_principal.to_le_bytes(),
        &hoard_donation.to_le_bytes(),
        &token_principal.to_le_bytes(),
        &token_donation.to_le_bytes(),
        &claim_refund_after.to_le_bytes(),
        &final_refund_after.to_le_bytes(),
        &claim_sink_after.to_le_bytes(),
        &final_sink_after.to_le_bytes(),
    ]);
    require_live(hoard_receipt_id)?;
    require(
        claim_receipt_id != hoard_receipt_id
            && account_id(claim_ledger_account.key) != owner_release_id
            && account_id(claim_ledger_account.key) != claim_receipt_id
            && owner_release_id != claim_receipt_id
            && account_id(hoard_account.key) != owner_release_id
            && account_id(hoard_account.key) != hoard_receipt_id
            && owner_release_id != hoard_receipt_id,
        ClutchError::MismatchedState,
    )?;

    Ok(AuthenticatedMarketLiabilityPhysicalTerminalsV3 {
        root,
        claim_ledger: AuthenticatedClaimLedgerPhysicalTerminalV3 {
            id: claim_receipt_id,
            root: root_facts,
            owner_account_id: account_id(claim_ledger_account.key),
            owner_release_id,
            release_authority_id,
            root_transition_sequence: claim_sequence,
            account_data_before_id: claim_data_id,
            account_semantic_before_id: claim_semantic_id,
            account_authentication_before_id: claim_authentication_id,
            account_closed_state_id: claim_closed_id,
            refund_owner: *refund_owner.key,
            neutral_sink: *neutral_sink.key,
            principal_lamports: claim_principal,
            donation_lamports: claim_donation,
            refund_lamports_before: refund_before,
            refund_lamports_after: claim_refund_after,
            sink_lamports_before: sink_before,
            sink_lamports_after: claim_sink_after,
        },
        hoard: AuthenticatedHoardPhysicalTerminalV3 {
            id: hoard_receipt_id,
            root: root_facts,
            owner_account_id: account_id(hoard_account.key),
            owner_release_id,
            release_authority_id,
            root_transition_sequence: hoard_sequence,
            account_data_before_id: hoard_data_id,
            account_semantic_before_id: hoard_semantic_id,
            account_authentication_before_id: hoard_authentication_id,
            account_closed_state_id: hoard_closed_id,
            token_account_id: account_id(hoard_token_account.key),
            token_data_before_id: token_data_id,
            token_authentication_before_id: token_authentication_id,
            token_closed_state_id: token_closed_id,
            collateral_release_deployment_receipt_id: content_id(
                deployment.receipt_id().bytes(),
            ),
            foundation_vault: *foundation_vault.key,
            foundation_vault_lamports_before: foundation_vault_before,
            foundation_vault_lamports_after: foundation_vault.lamports(),
            refund_owner: *refund_owner.key,
            neutral_sink: *neutral_sink.key,
            state_principal_lamports: hoard_principal,
            state_donation_lamports: hoard_donation,
            token_principal_lamports: token_principal,
            token_donation_lamports: token_donation,
            refund_lamports_before: claim_refund_after,
            refund_lamports_after: final_refund_after,
            sink_lamports_before: claim_sink_after,
            sink_lamports_after: final_sink_after,
        },
    })
}

fn require_current_release_join(
    registry: &AuthenticatedRegistryCapabilityV5,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
    binding: &MarketLifecycleBindingV3,
) -> Outcome<()> {
    let compiled = bundle.bundle();
    require(
        registry.activation_consumed()
            && registry.compiler_bundle_id() == bundle.bundle_id()
            && registry.registry_release_id() == compiled.registry_release_id
            && registry.capability_profile_id() == compiled.capability_profile_id.content_id()
            && registry.registry_release_id() == binding.registry_release_id
            && registry.capability_profile_id() == binding.capability_profile_id,
        ClutchError::MismatchedState,
    )
}

fn require_zero_liability_state(
    claim: ClaimLedgerV3,
    hoard: HoardV2,
    binding: &MarketLifecycleBindingV3,
) -> Outcome<()> {
    let claim_zero = claim.aggregate_internal_supply.iter().all(|amount| *amount == 0)
        && claim.aggregate_materialized_supply.iter().all(|amount| *amount == 0);
    require(
        claim.lifecycle == MarketLiabilityLifecycleV1::Retiring
            && claim_zero
            && hoard.lifecycle == MarketLiabilityLifecycleV1::Resolved
            && hoard.cash_liability_atoms == 0
            && hoard.locked_claim_principal_atoms == 0
            && claim.market_instance_id.bytes() == binding.market_instance_id.bytes()
            && claim.realm_id.bytes() == binding.realm_id.bytes()
            && claim.native_claim_basis_id.bytes() == binding.native_claim_basis_id.bytes()
            && claim.resolution_account.bytes() == binding.resolution_account_id.bytes()
            && hoard.market_instance_id.bytes() == binding.market_instance_id.bytes()
            && claim.outcome_count == binding.outcome_count
            && hoard.outcome_count == binding.outcome_count,
        ClutchError::MismatchedState,
    )
}

fn require_program_state(
    account: &AccountInfo<'_>,
    program_id: &Pubkey,
    expected_len: usize,
) -> Outcome<()> {
    require(
        account.owner == program_id
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && account.data_len() == expected_len,
        ClutchError::MismatchedState,
    )
}

fn require_system_recipient(account: &AccountInfo<'_>) -> Outcome<()> {
    require(
        account.owner == &SYSTEM_PROGRAM_ID
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && account.data_len() == 0,
        ClutchError::MismatchedState,
    )
}

fn require_distinct_roles(accounts: &[&AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            require(accounts[left].key != accounts[right].key, ClutchError::AccountAlias)?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn require_external_authority_separation(
    accounts: &[&AccountInfo<'_>],
    registry: &AuthenticatedRegistryCapabilityV5,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
) -> Outcome<()> {
    let authority_accounts = [
        registry.series_registry_account(),
        registry.program_account(),
        registry.programdata_account(),
        registry.release_artifact_account(),
        registry.profile_artifact_account(),
        bundle.artifact_account(),
    ];
    let mut index = 0usize;
    while index < authority_accounts.len() {
        let mut account_index = 0usize;
        while account_index < accounts.len() {
            require(
                authority_accounts[index] != *accounts[account_index].key,
                ClutchError::AccountAlias,
            )?;
            account_index += 1;
        }
        let mut prior = 0usize;
        while prior < index {
            require(
                authority_accounts[prior] != authority_accounts[index],
                ClutchError::AccountAlias,
            )?;
            prior += 1;
        }
        index += 1;
    }
    Ok(())
}

fn preflight_writes(accounts: &[&AccountInfo<'_>]) -> Outcome<()> {
    for account in accounts {
        let data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        drop(data);
        let lamports = account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        drop(lamports);
    }
    Ok(())
}

fn close_program_state_with_split(
    account: &AccountInfo<'_>,
    refund: &AccountInfo<'_>,
    sink: &AccountInfo<'_>,
    principal: u64,
    donation: u64,
    refund_after: u64,
    sink_after: u64,
) -> Outcome<()> {
    require(
        account.lamports() == principal.checked_add(donation).ok_or(ClutchError::Arithmetic)?,
        ClutchError::MismatchedState,
    )?;
    **account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = 0;
    **refund
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = refund_after;
    **sink
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = sink_after;
    account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    account.assign(&SYSTEM_PROGRAM_ID);
    require(
        account.lamports() == 0
            && account.data_len() == 0
            && account.owner == &SYSTEM_PROGRAM_ID
            && refund.lamports() == refund_after
            && sink.lamports() == sink_after,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn close_hoard_token_vault<'info>(
    token_program: &AccountInfo<'info>,
    token_account: &AccountInfo<'info>,
    foundation_vault: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    authority_bump: u8,
    market: MarketInstanceV2Id,
) -> Outcome<()> {
    let instruction = Instruction::new_with_bytes(
        *token_program.key,
        &[9u8],
        vec![
            AccountMeta::new(*token_account.key, false),
            AccountMeta::new(*foundation_vault.key, false),
            AccountMeta::new_readonly(*authority.key, true),
        ],
    );
    invoke_signed(
        &instruction,
        &[
            token_account.clone(),
            foundation_vault.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[&[
            seeds::SEED_HOARD_AUTHORITY_V2,
            &market.bytes(),
            &[authority_bump],
        ]],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))
}

#[allow(clippy::too_many_arguments)]
fn transfer_foundation_vault_split<'info>(
    system_program: &AccountInfo<'info>,
    foundation_vault: &AccountInfo<'info>,
    refund: &AccountInfo<'info>,
    sink: &AccountInfo<'info>,
    market: MarketInstanceV2Id,
    generation: u64,
    bump: u8,
    principal: u64,
    donation: u64,
) -> Outcome<()> {
    transfer_from_foundation_vault(
        system_program,
        foundation_vault,
        refund,
        market,
        generation,
        bump,
        principal,
    )?;
    if donation != 0 {
        transfer_from_foundation_vault(
            system_program,
            foundation_vault,
            sink,
            market,
            generation,
            bump,
            donation,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn transfer_from_foundation_vault<'info>(
    system_program: &AccountInfo<'info>,
    foundation_vault: &AccountInfo<'info>,
    recipient: &AccountInfo<'info>,
    market: MarketInstanceV2Id,
    generation: u64,
    bump: u8,
    lamports: u64,
) -> Outcome<()> {
    if lamports == 0 {
        return Ok(());
    }
    let instruction = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(lamports),
        vec![
            AccountMeta::new(*foundation_vault.key, true),
            AccountMeta::new(*recipient.key, false),
        ],
    );
    invoke_signed(
        &instruction,
        &[
            foundation_vault.clone(),
            recipient.clone(),
            system_program.clone(),
        ],
        &[&[
            seeds::SEED_PRODUCT_MARKET_FOUNDATION_VAULT,
            &market.bytes(),
            &generation.to_le_bytes(),
            &[bump],
        ]],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))
}

fn require_closed_token_reopen(
    bound: clutch_collateral_adapter_v2::BoundCollateralProfileV2,
    account: &AccountInfo<'_>,
) -> Outcome<()> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let reopened = admit_collateral_account_v2(
        bound,
        RuntimeAccountViewV2 {
            key: collateral_pubkey(account.key),
            owner_program: collateral_pubkey(account.owner),
            data: &data,
            is_signer: account.is_signer,
            is_writable: account.is_writable,
            executable: account.executable,
        },
        TokenAccountRoleV2::Hoard,
    );
    require(reopened.is_err() && account.lamports() == 0, ClutchError::MismatchedState)
}

fn liability_account_authentication_id(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    data_id: ContentId,
    semantic_id: ContentId,
    lamports: u64,
) -> ContentId {
    hashv(&[
        LIABILITY_ACCOUNT_AUTHENTICATION_DOMAIN_V3,
        account.key.as_ref(),
        program_id.as_ref(),
        &data_id.bytes(),
        &semantic_id.bytes(),
        &lamports.to_le_bytes(),
        &[u8::from(account.is_writable)],
    ])
}

fn closed_program_account_id(account: &AccountInfo<'_>) -> ContentId {
    hashv(&[
        LIABILITY_ACCOUNT_CLOSED_DOMAIN_V3,
        account.key.as_ref(),
        account.owner.as_ref(),
        &account.lamports().to_le_bytes(),
        &0u64.to_le_bytes(),
    ])
}

fn closed_token_account_id(account: &AccountInfo<'_>) -> Outcome<ContentId> {
    let data_id = account
        .try_borrow_data()
        .map(|data| hash_data(&data))
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let id = hashv(&[
        HOARD_TOKEN_CLOSED_DOMAIN_V3,
        account.key.as_ref(),
        account.owner.as_ref(),
        &account.lamports().to_le_bytes(),
        &data_id.bytes(),
    ]);
    require_live(id)?;
    Ok(id)
}

fn account_id(account: &Pubkey) -> ContentId { ContentId::from_bytes(account.to_bytes()) }

fn collateral_pubkey(account: &Pubkey) -> Id { Id::from_bytes(account.to_bytes()) }

fn collateral_id(id: ContentId) -> Id { Id::from_bytes(id.bytes()) }

fn content_id(bytes: [u8; 32]) -> ContentId { ContentId::from_bytes(bytes) }

fn hash_data(data: &[u8]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(&[data]).to_bytes())
}

fn hashv(parts: &[&[u8]]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(parts).to_bytes())
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}

#[cfg(test)]
mod tests {
    #[test]
    fn physical_terminal_receipts_are_move_only_and_ordered() {
        let source = include_str!("collateral_shared_core_terminal_v3.rs");
        let claim = source
            .split_once("pub(crate) struct AuthenticatedClaimLedgerPhysicalTerminalV3")
            .expect("ClaimLedger terminal owner")
            .0
            .rsplit_once("#[derive(")
            .expect("ClaimLedger derive")
            .1;
        let hoard = source
            .split_once("pub(crate) struct AuthenticatedHoardPhysicalTerminalV3")
            .expect("Hoard terminal owner")
            .0
            .rsplit_once("#[derive(")
            .expect("Hoard derive")
            .1;
        assert!(!claim.contains("Clone"));
        assert!(!claim.contains("Copy"));
        assert!(!hoard.contains("Clone"));
        assert!(!hoard.contains("Copy"));
        let close = source
            .split_once("pub(crate) fn close_market_liability_shared_cores_v3")
            .expect("sole close")
            .1;
        let claim_close = close.find("close_program_state_with_split(\n        claim_ledger_account").unwrap();
        let token_close = close.find("close_hoard_token_vault(").unwrap();
        let hoard_close = close.find("close_program_state_with_split(\n        hoard_account").unwrap();
        assert!(claim_close < token_close && token_close < hoard_close);
    }

    #[test]
    fn physical_terminal_boundary_refuses_splices_and_caller_ids() {
        let source = include_str!("collateral_shared_core_terminal_v3.rs");
        for required in [
            "require_distinct_roles",
            "MarketLifecyclePhaseV3::Retiring",
            "MarketSharedCoreV3::ClaimLedger).is_zero()",
            "MarketSharedCoreV3::Hoard).is_zero()",
            "registry.compiler_bundle_id() == bundle.bundle_id()",
            "binding.foundation_schedule_id == schedule_id",
            "binding.foundation_account_graph_id == graph_id",
            "MarketLiabilityLifecycleV1::Retiring",
            "MarketLiabilityLifecycleV1::Resolved",
            "token_observation.amount_atoms == 0",
            "foundation_vault.lamports() == foundation_vault_before",
            "claim.rent.payer().bytes() == refund_owner.key.to_bytes()",
            "hoard.rent.payer().bytes() == refund_owner.key.to_bytes()",
        ] {
            assert!(source.contains(required), "missing invariant: {required}");
        }
        let signature = source
            .split_once("pub(crate) fn close_market_liability_shared_cores_v3")
            .expect("close function")
            .1
            .split_once(") -> Outcome")
            .expect("signature")
            .0;
        assert!(!signature.contains("expected_id"));
        assert!(!signature.contains("terminal_receipt_id"));
        assert!(!source.contains("MarketSharedCoreTerminalProjectionV3::new"));
    }
}
