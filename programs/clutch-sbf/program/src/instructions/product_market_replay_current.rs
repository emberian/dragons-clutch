//! Current persistent ProductReplayAnchor physical owner.
//!
//! This module is the only current constructor of a Market generation. It
//! creates the market-only `0xb0/v2` account before RootV3, derives the exact
//! nonzero generation from authenticated immutable owners, and later repays
//! the temporary bootstrap principal from canonical foundation slot 13. The
//! replay account itself remains live through Market activation and terminal
//! replay; the historical closure-only `0xb0/v1` authority is never imported.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, transfer_data, SYSTEM_PROGRAM_ID,
};
use crate::instructions::product_market_family_capability_current::
    AuthenticatedMarketFamilyCapabilityPolicyArtifactV1;
use crate::instructions::product_market_lifecycle_v3_current::
    AuthenticatedProductMarketFoundationStepPostwriteV4;
use crate::instructions::product_market_foundation_graph_v4_current::
    AuthenticatedCurrentMarketFoundationGraphV4;
use crate::instructions::product_series::physical_v5::AuthenticatedSeriesPhysicalFounderV5;
use crate::instructions::product_series_current::AuthenticatedRegistryCapabilityV5;
use crate::seeds;
use clutch_product_series::{
    derive_initial_market_generation_v2, AuthenticatedMarketLifecycleGenerationAuthorityV2,
    AuthenticatedMarketLifecycleReplayActivationAuthorityV2,
    AuthenticatedMarketLifecycleReplayFoundationAuthorityV2, ContentId,
    MarketFoundationScheduleV4, MarketFoundationSlotV4, MarketInstanceV2Id,
    MarketLifecycleBootstrapLineageV2, MarketLifecycleGenerationBindingV2,
    MarketLifecycleReplayPhaseV2,
    MarketLifecycleReplayV2, RegistryCapabilityProfileV4Id, RegistryProgramReleaseV2Id,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleReplayAccountV2, MARKET_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V2,
};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::product_market_activation_v3_current::
    AuthenticatedCurrentProductMarketActivationV3;

const PRODUCT_MARKET_REPLAY_ACCOUNT_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-market-replay-account-authentication/v2\0";
const PRODUCT_MARKET_GENERATION_BOOTSTRAP_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-market-generation-bootstrap/v2\0";
const PRODUCT_MARKET_GENERATION_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-market-generation-authentication/v2\0";
const PRODUCT_MARKET_REPLAY_FOUNDATION_SETTLEMENT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-market-replay-foundation-settlement/v2\0";
const PRODUCT_MARKET_REPLAY_FOUNDATION_POSTWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-market-replay-foundation-postwrite/v2\0";

/// Move-only owner of the exact post-Source action14 transcript persisted in
/// ProductReplayAnchor. The default refuses so a decoded lineage body or an
/// operator projection cannot authorize replay creation.
pub(crate) trait AuthenticatedCurrentProductMarketBootstrapLineageV2 {
    fn bootstrap_lineage(&self) -> MarketLifecycleBootstrapLineageV2;

    fn authenticate_current_product_market_bootstrap_lineage_v2(
        &self,
        _program_id: &Pubkey,
        _physical: &AuthenticatedSeriesPhysicalFounderV5,
        _registry: &AuthenticatedRegistryCapabilityV5,
        _family_policy: &AuthenticatedMarketFamilyCapabilityPolicyArtifactV1,
        _graph: &AuthenticatedCurrentMarketFoundationGraphV4,
        _lineage: MarketLifecycleBootstrapLineageV2,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Move-only hostile authentication of the current persistent replay account.
#[derive(Debug)]
pub(crate) struct AuthenticatedMarketLifecycleReplayV2 {
    account: Pubkey,
    value: Box<MarketLifecycleReplayAccountV2>,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedMarketLifecycleReplayV2 {
    pub(crate) const fn account(&self) -> Pubkey { self.account }
    pub(crate) const fn value(&self) -> &MarketLifecycleReplayAccountV2 { &self.value }
    pub(crate) const fn state(&self) -> &MarketLifecycleReplayV2 { &self.value.state }
    pub(crate) const fn generation(&self) -> u64 { self.value.state.generation() }
    pub(crate) const fn observed_lamports(&self) -> u64 { self.observed_lamports }
    pub(crate) const fn is_writable(&self) -> bool { self.writable }
    pub(crate) const fn data_id(&self) -> ContentId { self.data_id }
    pub(crate) const fn authentication_id(&self) -> ContentId { self.authentication_id }
}

/// Instruction-local move-only bootstrap output. Cross-transaction authority
/// is reconstructed only by hostile-reopening the persisted replay account.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductMarketGenerationV2 {
    id: ContentId,
    replay: AuthenticatedMarketLifecycleReplayV2,
}

impl AuthenticatedProductMarketGenerationV2 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn generation(&self) -> u64 { self.replay.generation() }
    pub(crate) const fn replay(&self) -> &AuthenticatedMarketLifecycleReplayV2 {
        &self.replay
    }
}

struct ExactGenerationInitializationV2 {
    binding: MarketLifecycleGenerationBindingV2,
    bootstrap_authority_id: ContentId,
    bootstrap_receipt_id: ContentId,
}

impl AuthenticatedMarketLifecycleGenerationAuthorityV2 for ExactGenerationInitializationV2 {
    fn authenticate_market_lifecycle_generation_v2(
        &self,
        binding: MarketLifecycleGenerationBindingV2,
        bootstrap_authority_id: ContentId,
        bootstrap_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if binding != self.binding
            || bootstrap_authority_id != self.bootstrap_authority_id
            || bootstrap_receipt_id != self.bootstrap_receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

struct ExactFoundationSettlementV2 {
    state_id: ContentId,
    settlement_receipt_id: ContentId,
}

struct ExactMarketActivationV2 {
    replay_semantic_before_id: ContentId,
    root_semantic_id: ContentId,
    root_activation_receipt_id: ContentId,
}

impl AuthenticatedMarketLifecycleReplayActivationAuthorityV2 for ExactMarketActivationV2 {
    fn authenticate_market_lifecycle_replay_activation_v2(
        &self,
        state: &MarketLifecycleReplayV2,
        root: &clutch_product_series::MarketLifecycleRootV3,
        root_activation_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if state.id()?.content_id() != self.replay_semantic_before_id
            || root.semantic_id()? != self.root_semantic_id
            || root_activation_receipt_id != self.root_activation_receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

/// Final instruction-local lineage after RootV3, LinkV3, and the permanent
/// market-only replay all persist the same activation.
#[derive(Debug)]
pub(crate) struct AuthenticatedCurrentProductMarketReplayActivationV3<'root, 'link> {
    id: ContentId,
    market_activation: AuthenticatedCurrentProductMarketActivationV3<'root, 'link>,
    replay_after: AuthenticatedMarketLifecycleReplayV2,
}

impl<'root, 'link> AuthenticatedCurrentProductMarketReplayActivationV3<'root, 'link> {
    pub(crate) const fn id(&self) -> ContentId { self.id }
}

impl AuthenticatedMarketLifecycleReplayFoundationAuthorityV2 for ExactFoundationSettlementV2 {
    fn authenticate_market_lifecycle_replay_foundation_v2(
        &self,
        state: &MarketLifecycleReplayV2,
        foundation_settlement_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if state.id()?.content_id() != self.state_id
            || foundation_settlement_receipt_id != self.settlement_receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

/// Move-only physical postwrite for canonical foundation slot 13.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductMarketReplayFoundationPostwriteV2 {
    id: ContentId,
    bootstrap_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    replay_account: Pubkey,
    replay_observed_lamports: u64,
    replay_authentication_before_id: ContentId,
    replay_authentication_after_id: ContentId,
    replay_semantic_before_id: ContentId,
    replay_semantic_after_id: ContentId,
    foundation_vault: Pubkey,
    bootstrap_payer: Pubkey,
    neutral_lamport_sink: Pubkey,
    principal_lamports: u64,
    vault_lamports_before: u64,
    vault_lamports_after: u64,
    payer_lamports_before: u64,
    payer_lamports_after: u64,
    settlement_receipt_id: ContentId,
    physical_capitalization_receipt_id: ContentId,
    family_policy_artifact_authentication_id: ContentId,
    foundation_schedule_id: ContentId,
    foundation_graph_id: ContentId,
}

impl AuthenticatedProductMarketFoundationStepPostwriteV4
    for AuthenticatedProductMarketReplayFoundationPostwriteV2
{
    #[allow(clippy::too_many_arguments)]
    fn consume_product_market_foundation_step_postwrite_v4(
        self,
        debit_id: ContentId,
        founder_creation_receipt_id: ContentId,
        founder_preauthorization_id: ContentId,
        foundation_steps_id: ContentId,
        market_binding_id: ContentId,
        foundation_schedule_id: ContentId,
        foundation_graph_id: ContentId,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        slot: MarketFoundationSlotV4,
        root_transition_sequence_after: u64,
        account_id: ContentId,
        principal_lamports: u64,
        principal_before_lamports: u64,
        principal_after_lamports: u64,
        destination_donation_floor_lamports: u64,
        destination_balance_after_lamports: u64,
        vault_donation_before_lamports: u64,
        vault_donation_after_lamports: u64,
        foundation_vault_account: Pubkey,
        rent_refund_owner: Pubkey,
        neutral_lamport_sink: Pubkey,
    ) -> Outcome<(ContentId, u64)> {
        let observed_vault_donation_lamports = self
            .vault_lamports_before
            .checked_sub(principal_before_lamports)
            .ok_or(ClutchError::Arithmetic)?;
        let expected_vault_after = principal_after_lamports
            .checked_add(observed_vault_donation_lamports)
            .ok_or(ClutchError::Arithmetic)?;
        require(
            !debit_id.is_zero()
                && !founder_creation_receipt_id.is_zero()
                && !founder_preauthorization_id.is_zero()
                && !foundation_steps_id.is_zero()
                && !market_binding_id.is_zero()
                && market_instance_id == self.market_instance_id
                && generation == self.generation
                && root_transition_sequence_after != 0
                && slot == MarketFoundationSlotV4::ProductReplayAnchor
                && account_id.bytes() == self.replay_account.to_bytes()
                && principal_lamports == self.principal_lamports
                && self.foundation_schedule_id == foundation_schedule_id
                && self.foundation_graph_id == foundation_graph_id
                && self.foundation_vault == foundation_vault_account
                && self.bootstrap_payer == rent_refund_owner
                && self.neutral_lamport_sink == neutral_lamport_sink
                && self.vault_lamports_after == expected_vault_after
                && observed_vault_donation_lamports == vault_donation_before_lamports
                && observed_vault_donation_lamports == vault_donation_after_lamports
                && destination_donation_floor_lamports == self.replay_observed_lamports
                && destination_balance_after_lamports == self.replay_observed_lamports
                && self.payer_lamports_after
                    == self.payer_lamports_before
                        .checked_add(self.principal_lamports)
                        .ok_or(ClutchError::Arithmetic)?,
            ClutchError::MismatchedState,
        )?;
        let accepted = hashv(&[
            PRODUCT_MARKET_REPLAY_FOUNDATION_POSTWRITE_DOMAIN_V2,
            &self.id.bytes(),
            &debit_id.bytes(),
            &self.bootstrap_id.bytes(),
            &founder_creation_receipt_id.bytes(),
            &founder_preauthorization_id.bytes(),
            &foundation_steps_id.bytes(),
            &market_binding_id.bytes(),
            &foundation_schedule_id.bytes(),
            &foundation_graph_id.bytes(),
            &market_instance_id.bytes(),
            &generation.to_le_bytes(),
            &[13],
            &root_transition_sequence_after.to_le_bytes(),
            self.replay_account.as_ref(),
            &self.replay_authentication_before_id.bytes(),
            &self.replay_authentication_after_id.bytes(),
            &self.replay_semantic_before_id.bytes(),
            &self.replay_semantic_after_id.bytes(),
            &self.settlement_receipt_id.bytes(),
            &self.physical_capitalization_receipt_id.bytes(),
            &self.family_policy_artifact_authentication_id.bytes(),
            &principal_lamports.to_le_bytes(),
            &principal_before_lamports.to_le_bytes(),
            &principal_after_lamports.to_le_bytes(),
            &observed_vault_donation_lamports.to_le_bytes(),
        ]);
        require_live(accepted)?;
        Ok((accepted, observed_vault_donation_lamports))
    }
}

/// Hostile-authenticate the exact current persistent ProductReplayAnchor.
#[inline(never)]
pub(crate) fn authenticate_market_lifecycle_replay_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_market_instance_id: MarketInstanceV2Id,
    require_writable: bool,
) -> Outcome<AuthenticatedMarketLifecycleReplayV2> {
    require(
        !account.is_signer
            && !account.executable
            && account.is_writable == require_writable
            && account.owner == program_id
            && account.data_len() == MARKET_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V2,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = Box::new(MarketLifecycleReplayAccountV2::decode(&data)?);
    let binding = value.state.binding();
    let (expected, bump) = seeds::product_market_lifecycle_replay_v2_pda(
        program_id,
        &expected_market_instance_id.bytes(),
    );
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let observed_lamports = account.lamports();
    require(
        binding.market_instance_id == expected_market_instance_id
            && binding.replay_account_id.bytes() == account.key.to_bytes()
            && observed_lamports >= value.permanent_rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let data_id = hash_data(&data);
    drop(data);
    let semantic_id = value
        .state
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_id = hashv(&[
        PRODUCT_MARKET_REPLAY_ACCOUNT_AUTHENTICATION_DOMAIN_V2,
        account.key.as_ref(),
        program_id.as_ref(),
        &data_id.bytes(),
        &semantic_id.bytes(),
        &observed_lamports.to_le_bytes(),
        &value.permanent_rent_principal_lamports.to_le_bytes(),
        &[value.stored_bump, u8::from(require_writable)],
    ]);
    require_live(authentication_id)?;
    Ok(AuthenticatedMarketLifecycleReplayV2 {
        account: *account.key,
        value,
        observed_lamports,
        writable: require_writable,
        data_id,
        authentication_id,
    })
}

/// Create and hostile-reopen the sole current Market generation owner.
///
/// The bootstrap payer supplies the complete rent principal even when the
/// predictable PDA was prefunded. That prefund is retained as donation. The
/// payer is exactly the immutable FundingTerms lamport refund owner and is
/// repaid only when a bounded action15 stage consumes canonical foundation
/// slot 13. Every later stage reconstructs authority from hostile state.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn bootstrap_current_product_market_generation_v2<'a, L>(
    program_id: &Pubkey,
    physical: &AuthenticatedSeriesPhysicalFounderV5,
    registry: &AuthenticatedRegistryCapabilityV5,
    family_policy: &AuthenticatedMarketFamilyCapabilityPolicyArtifactV1,
    schedule: &MarketFoundationScheduleV4,
    graph_authority: &AuthenticatedCurrentMarketFoundationGraphV4,
    lineage_authority: &L,
    bootstrap_payer: &AccountInfo<'a>,
    replay_account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
) -> Outcome<AuthenticatedProductMarketGenerationV2>
where
    L: AuthenticatedCurrentProductMarketBootstrapLineageV2 + ?Sized,
{
    let graph = graph_authority.graph();
    let market_instance_id = graph_authority.market_instance_id();
    schedule
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let generation = derive_initial_market_generation_v2(
        market_instance_id,
        family_policy.policy_id(),
        RegistryProgramReleaseV2Id::from_bytes(family_policy.registry_release_id().bytes()),
        RegistryCapabilityProfileV4Id::from_bytes(
            family_policy.capability_profile_id().bytes(),
        ),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    graph
        .validate(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market = market_instance_id.bytes();
    let (expected_replay, replay_bump) =
        seeds::product_market_lifecycle_replay_v2_pda(program_id, &market);
    let (expected_root, _) =
        seeds::product_market_lifecycle_root_pda(program_id, &market, generation);
    let slot_index = MarketFoundationSlotV4::ProductReplayAnchor
        .index()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rent = read_rent(rent_sysvar)?;
    let rent_principal_lamports =
        rent.minimum_balance(MARKET_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V2)?;
    require(
        graph_authority.id() != ContentId::ZERO
            && graph_authority.schedule_id() == schedule_id
            && graph_authority.graph_id() == graph_id
            && graph_authority.physical_founder_id() == physical.id()
            && graph_authority.physical_capitalization_id() == physical.capitalization_id()
            && graph_authority.family_policy_authentication_id() == family_policy.id()
            && physical.registry_capability_after_id() == registry.id()
            && family_policy.registry_release_id() == registry.registry_release_id()
            && family_policy.capability_profile_id() == registry.capability_profile_id()
            && bootstrap_payer.is_signer
            && bootstrap_payer.is_writable
            && !bootstrap_payer.executable
            && bootstrap_payer.owner == &SYSTEM_PROGRAM_ID
            && bootstrap_payer.data_len() == 0
            && *bootstrap_payer.key == physical.capitalization().lamport_principal_refund()
            && replay_account.is_writable
            && !replay_account.is_signer
            && !replay_account.executable
            && replay_account.owner == &SYSTEM_PROGRAM_ID
            && replay_account.data_len() == 0
            && *system_program.key == SYSTEM_PROGRAM_ID
            && system_program.executable
            && !system_program.is_writable
            && *replay_account.key == expected_replay
            && replay_account.key != bootstrap_payer.key
            && replay_account.key != system_program.key
            && graph.market_instance_id == market_instance_id
            && graph.generation == generation
            && graph.account(MarketFoundationSlotV4::ProductReplayAnchor)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == replay_account.key.to_bytes()
            && graph.account(MarketFoundationSlotV4::LifecycleRoot)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == expected_root.to_bytes()
            && schedule.slot_principal_lamports[slot_index] == rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let prefund_donation_lamports = replay_account.lamports();
    let payer_lamports_before = bootstrap_payer.lamports();
    let payer_lamports_after = payer_lamports_before
        .checked_sub(rent_principal_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let replay_lamports_after = prefund_donation_lamports
        .checked_add(rent_principal_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let bootstrap_lineage = lineage_authority.bootstrap_lineage();
    lineage_authority.authenticate_current_product_market_bootstrap_lineage_v2(
        program_id,
        physical,
        registry,
        family_policy,
        graph_authority,
        bootstrap_lineage,
    )?;
    let bootstrap_lineage_id = bootstrap_lineage
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let binding = MarketLifecycleGenerationBindingV2 {
        replay_account_id: ContentId::from_bytes(replay_account.key.to_bytes()),
        market_instance_id,
        market_family_capability_policy_id: family_policy.policy_id(),
        market_family_capability_authentication_id: family_policy.id(),
        physical_capitalization_receipt_id: physical.capitalization_id(),
        registry_release_id: RegistryProgramReleaseV2Id::from_bytes(
            registry.registry_release_id().bytes(),
        ),
        capability_profile_id: RegistryCapabilityProfileV4Id::from_bytes(
            registry.capability_profile_id().bytes(),
        ),
        foundation_schedule_id: schedule_id,
        foundation_account_graph_id: graph_id,
        lifecycle_root_account_id: ContentId::from_bytes(expected_root.to_bytes()),
        rent_principal_refund_owner: ContentId::from_bytes(bootstrap_payer.key.to_bytes()),
        neutral_lamport_sink: ContentId::from_bytes(
            physical.capitalization().neutral_lamport_sink().to_bytes(),
        ),
        generation,
        replay_rent_principal_lamports: rent_principal_lamports,
        replay_prefund_donation_lamports: prefund_donation_lamports,
    };
    let bootstrap_receipt_id = hashv(&[
        PRODUCT_MARKET_GENERATION_BOOTSTRAP_DOMAIN_V2,
        program_id.as_ref(),
        &physical.id().bytes(),
        &physical.capitalization_id().bytes(),
        &registry.id().bytes(),
        &family_policy.id().bytes(),
        &family_policy.policy_id().bytes(),
        &bootstrap_lineage_id.bytes(),
        &binding
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
        bootstrap_payer.key.as_ref(),
        replay_account.key.as_ref(),
        rent_sysvar.key.as_ref(),
        &rent.lamports_per_byte_year.to_le_bytes(),
        &rent.exemption_threshold.to_bits().to_le_bytes(),
        &rent_principal_lamports.to_le_bytes(),
        &prefund_donation_lamports.to_le_bytes(),
        &payer_lamports_before.to_le_bytes(),
        &payer_lamports_after.to_le_bytes(),
        &replay_lamports_after.to_le_bytes(),
        &[replay_bump],
    ]);
    require_live(bootstrap_receipt_id)?;
    let initialization = ExactGenerationInitializationV2 {
        binding,
        bootstrap_authority_id: family_policy.id(),
        bootstrap_receipt_id,
    };
    let state = MarketLifecycleReplayV2::initialize(
        &initialization,
        binding,
        bootstrap_lineage,
        family_policy.id(),
        bootstrap_receipt_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;

    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(rent_principal_lamports),
        vec![
            AccountMeta::new(*bootstrap_payer.key, true),
            AccountMeta::new(*replay_account.key, false),
        ],
    );
    invoke(
        &transfer,
        &[
            bootstrap_payer.clone(),
            replay_account.clone(),
            system_program.clone(),
        ],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    require(
        bootstrap_payer.lamports() == payer_lamports_after
            && replay_account.lamports() == replay_lamports_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    allocate_assign_replay_v2(
        program_id,
        replay_account,
        system_program,
        &[
            seeds::SEED_PRODUCT_MARKET_LIFECYCLE_REPLAY_V2,
            &market,
            &[replay_bump],
        ],
    )?;
    let encoded = MarketLifecycleReplayAccountV2 {
        state,
        permanent_rent_principal_lamports: rent_principal_lamports,
        stored_bump: replay_bump,
    };
    {
        let mut data = replay_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(data.iter().all(|byte| *byte == 0), ClutchError::AlreadyInitialized)?;
        encoded.encode(&mut data)?;
    }
    let replay = authenticate_market_lifecycle_replay_v2(
        program_id,
        replay_account,
        market_instance_id,
        true,
    )?;
    require(
        replay.value() == &encoded
            && replay.observed_lamports() == replay_lamports_after
            && replay.state().phase() == MarketLifecycleReplayPhaseV2::Founding,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_MARKET_GENERATION_AUTHENTICATION_DOMAIN_V2,
        program_id.as_ref(),
        &bootstrap_receipt_id.bytes(),
        &family_policy.id().bytes(),
        &physical.id().bytes(),
        &replay.authentication_id().bytes(),
        &replay.data_id().bytes(),
        &replay
            .state()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductMarketGenerationV2 { id, replay })
}

/// Repay the temporary full-rent bootstrap from canonical slot 13, transition
/// the persistent replay body once, and return the only postwrite the Product
/// foundation cursor accepts for ProductReplayAnchor.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn settle_current_product_market_replay_foundation_v2<'a>(
    program_id: &Pubkey,
    authenticated: AuthenticatedMarketLifecycleReplayV2,
    replay_account: &AccountInfo<'a>,
    foundation_vault: &AccountInfo<'a>,
    bootstrap_payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
) -> Outcome<AuthenticatedProductMarketReplayFoundationPostwriteV2> {
    let binding = authenticated.state().binding();
    let market_instance_id = binding.market_instance_id;
    let generation = binding.generation;
    let market = market_instance_id.bytes();
    let (expected_vault, vault_bump) =
        seeds::product_market_foundation_vault_pda(program_id, &market, generation);
    let live_before = authenticate_market_lifecycle_replay_v2(
        program_id,
        replay_account,
        market_instance_id,
        true,
    )?;
    let replay_semantic_before_id = live_before
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    require(
        authenticated.state().phase() == MarketLifecycleReplayPhaseV2::Founding
            && live_before.state() == authenticated.state()
            && live_before.authentication_id() == authenticated.authentication_id()
            && live_before.data_id() == authenticated.data_id()
            && live_before.observed_lamports() == authenticated.observed_lamports()
            && live_before.observed_lamports()
                >= binding
                    .replay_rent_principal_lamports
                    .checked_add(binding.replay_prefund_donation_lamports)
                    .ok_or(ClutchError::Arithmetic)?
            && *foundation_vault.key == expected_vault
            && foundation_vault.is_writable
            && !foundation_vault.is_signer
            && !foundation_vault.executable
            && foundation_vault.owner == &SYSTEM_PROGRAM_ID
            && foundation_vault.data_len() == 0
            && bootstrap_payer.key.to_bytes() == binding.rent_principal_refund_owner.bytes()
            && bootstrap_payer.is_writable
            && !bootstrap_payer.is_signer
            && !bootstrap_payer.executable
            && bootstrap_payer.owner == &SYSTEM_PROGRAM_ID
            && bootstrap_payer.data_len() == 0
            && *system_program.key == SYSTEM_PROGRAM_ID
            && system_program.executable
            && !system_program.is_writable
            && foundation_vault.key != bootstrap_payer.key
            && replay_account.key != foundation_vault.key
            && replay_account.key != bootstrap_payer.key,
        ClutchError::MismatchedState,
    )?;
    let vault_lamports_before = foundation_vault.lamports();
    let vault_lamports_after = vault_lamports_before
        .checked_sub(binding.replay_rent_principal_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let payer_lamports_before = bootstrap_payer.lamports();
    let payer_lamports_after = payer_lamports_before
        .checked_add(binding.replay_rent_principal_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let settlement_receipt_id = hashv(&[
        PRODUCT_MARKET_REPLAY_FOUNDATION_SETTLEMENT_DOMAIN_V2,
        program_id.as_ref(),
        &authenticated.authentication_id().bytes(),
        &authenticated.state().id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?.bytes(),
        replay_account.key.as_ref(),
        &live_before.authentication_id().bytes(),
        &replay_semantic_before_id.bytes(),
        foundation_vault.key.as_ref(),
        bootstrap_payer.key.as_ref(),
        &binding.replay_rent_principal_lamports.to_le_bytes(),
        &vault_lamports_before.to_le_bytes(),
        &vault_lamports_after.to_le_bytes(),
        &payer_lamports_before.to_le_bytes(),
        &payer_lamports_after.to_le_bytes(),
    ]);
    require_live(settlement_receipt_id)?;
    let settlement = ExactFoundationSettlementV2 {
        state_id: replay_semantic_before_id,
        settlement_receipt_id,
    };
    let successor = live_before
        .state()
        .settle_foundation(&settlement, settlement_receipt_id)
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(binding.replay_rent_principal_lamports),
        vec![
            AccountMeta::new(*foundation_vault.key, true),
            AccountMeta::new(*bootstrap_payer.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[
            foundation_vault.clone(),
            bootstrap_payer.clone(),
            system_program.clone(),
        ],
        &[&[
            seeds::SEED_PRODUCT_MARKET_FOUNDATION_VAULT,
            &market,
            &generation.to_le_bytes(),
            &[vault_bump],
        ]],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    require(
        foundation_vault.lamports() == vault_lamports_after
            && bootstrap_payer.lamports() == payer_lamports_after
            && replay_account.lamports() == live_before.observed_lamports(),
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let successor_account = MarketLifecycleReplayAccountV2 {
        state: successor,
        permanent_rent_principal_lamports: binding.replay_rent_principal_lamports,
        stored_bump: live_before.value().stored_bump,
    };
    {
        let mut data = replay_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        successor_account.encode(&mut data)?;
    }
    let live_after = authenticate_market_lifecycle_replay_v2(
        program_id,
        replay_account,
        market_instance_id,
        true,
    )?;
    let replay_semantic_after_id = live_after
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    require(
        live_after.value() == &successor_account
            && live_after.observed_lamports() == live_before.observed_lamports()
            && live_after.authentication_id() != live_before.authentication_id()
            && live_after.data_id() != live_before.data_id()
            && live_after.state().phase() == MarketLifecycleReplayPhaseV2::FoundationSettled,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_MARKET_REPLAY_FOUNDATION_POSTWRITE_DOMAIN_V2,
        program_id.as_ref(),
        &authenticated.authentication_id().bytes(),
        &settlement_receipt_id.bytes(),
        replay_account.key.as_ref(),
        &live_before.authentication_id().bytes(),
        &live_after.authentication_id().bytes(),
        &replay_semantic_before_id.bytes(),
        &replay_semantic_after_id.bytes(),
        foundation_vault.key.as_ref(),
        bootstrap_payer.key.as_ref(),
        &binding.replay_rent_principal_lamports.to_le_bytes(),
        &vault_lamports_before.to_le_bytes(),
        &vault_lamports_after.to_le_bytes(),
        &payer_lamports_before.to_le_bytes(),
        &payer_lamports_after.to_le_bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductMarketReplayFoundationPostwriteV2 {
        id,
        bootstrap_id: authenticated.state().bootstrap_receipt_id(),
        market_instance_id,
        generation,
        replay_account: *replay_account.key,
        replay_observed_lamports: live_after.observed_lamports(),
        replay_authentication_before_id: live_before.authentication_id(),
        replay_authentication_after_id: live_after.authentication_id(),
        replay_semantic_before_id,
        replay_semantic_after_id,
        foundation_vault: *foundation_vault.key,
        bootstrap_payer: *bootstrap_payer.key,
        neutral_lamport_sink: Pubkey::new_from_array(binding.neutral_lamport_sink.bytes()),
        principal_lamports: binding.replay_rent_principal_lamports,
        vault_lamports_before,
        vault_lamports_after,
        payer_lamports_before,
        payer_lamports_after,
        settlement_receipt_id,
        physical_capitalization_receipt_id: binding.physical_capitalization_receipt_id,
        family_policy_artifact_authentication_id:
            binding.market_family_capability_authentication_id,
        foundation_schedule_id: binding.foundation_schedule_id.content_id(),
        foundation_graph_id: binding.foundation_account_graph_id.content_id(),
    })
}

/// Consume Product's exact RootV3/LinkV3 activation and persist the matching
/// permanent replay activation before the atomic action-1 outer returns.
#[inline(never)]
pub(crate) fn activate_current_product_market_replay_v3<'root, 'link>(
    program_id: &Pubkey,
    replay_account: &AccountInfo<'_>,
    replay_before: AuthenticatedMarketLifecycleReplayV2,
    market_activation: AuthenticatedCurrentProductMarketActivationV3<'root, 'link>,
) -> Outcome<AuthenticatedCurrentProductMarketReplayActivationV3<'root, 'link>> {
    let root = market_activation.root();
    let market_instance_id = root.binding().market_instance_id;
    let replay_semantic_before_id = replay_before
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    require(
        replay_before.is_writable()
            && replay_before.account() == *replay_account.key
            && replay_before.state().phase() == MarketLifecycleReplayPhaseV2::FoundationSettled
            && replay_before.state().generation() == root.binding().generation
            && replay_before.state().binding().market_instance_id == market_instance_id,
        ClutchError::MismatchedState,
    )?;
    let successor = replay_before
        .state()
        .activate(
            &ExactMarketActivationV2 {
                replay_semantic_before_id,
                root_semantic_id: root.semantic_id(),
                root_activation_receipt_id: market_activation.id(),
            },
            root.state(),
            market_activation.id(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let successor_account = MarketLifecycleReplayAccountV2 {
        state: successor,
        permanent_rent_principal_lamports:
            replay_before.value().permanent_rent_principal_lamports,
        stored_bump: replay_before.value().stored_bump,
    };
    {
        let mut data = replay_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        successor_account.encode(&mut data)?;
    }
    let replay_after = authenticate_market_lifecycle_replay_v2(
        program_id,
        replay_account,
        market_instance_id,
        true,
    )?;
    require(
        replay_after.value() == &successor_account
            && replay_after.observed_lamports() == replay_before.observed_lamports()
            && replay_after.authentication_id() != replay_before.authentication_id()
            && replay_after.data_id() != replay_before.data_id()
            && replay_after.state().phase() == MarketLifecycleReplayPhaseV2::Active
            && replay_after.state().root_binding_id() == root.binding_id(),
        ClutchError::MismatchedState,
    )?;
    let replay_semantic_after_id = replay_after
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let id = hashv(&[
        b"dragons-clutch/sbf/product-market-replay-activation/v3\0",
        program_id.as_ref(),
        replay_account.key.as_ref(),
        &replay_before.authentication_id().bytes(),
        &replay_after.authentication_id().bytes(),
        &replay_semantic_before_id.bytes(),
        &replay_semantic_after_id.bytes(),
        &market_activation.id().bytes(),
        &root.authentication_id().bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedCurrentProductMarketReplayActivationV3 {
        id,
        market_activation,
        replay_after,
    })
}

fn allocate_assign_replay_v2<'a>(
    program_id: &Pubkey,
    account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(MARKET_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V2),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &allocate,
        &[account.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &assign,
        &[account.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        account.owner == program_id
            && account.data_len() == MARKET_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V2,
        ClutchError::AccountCreationFailed,
    )
}

fn hash_data(data: &[u8]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(&[data]).to_bytes())
}

fn hashv(parts: &[&[u8]]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(parts).to_bytes())
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}
