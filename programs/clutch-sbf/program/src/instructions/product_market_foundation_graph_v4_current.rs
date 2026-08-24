//! Exact current 50-slot Product foundation graph derivation.
//!
//! The graph is not accepted from instruction bytes. It is reconstructed from
//! the physical FundingV5 founder, the immutable family-policy artifact, the
//! pre-root Source occurrence, the QuoteV6-owned ScheduleV4, and the hostile
//! Realm revenue policy. The resulting move-only receipt is the sole graph
//! authority consumed by replay bootstrap and RootV3 family lifecycle writers.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_product_series::{
    derive_initial_market_generation_v2, ContentId, MarketFoundationAccountGraphV4,
    MarketFoundationAccountGraphV4Id, MarketFoundationScheduleV4,
    MarketFoundationScheduleV4Id, MarketFoundationSlotV4, MarketInstanceV2Id,
    RegistryCapabilityProfileV4Id, RegistryProgramReleaseV2Id,
    MARKET_FOUNDATION_SLOT_COUNT_V4,
};
use clutch_solana_layout::Hash32;
use solana_pubkey::Pubkey;

use super::product_market_family_capability_current::
    AuthenticatedMarketFamilyCapabilityPolicyArtifactV1;
use super::product_market_replay_current::AuthenticatedMarketLifecycleReplayV2;
use super::product_series::physical_v5::AuthenticatedSeriesPhysicalFounderV5;
use super::revenue_policy_v2::{
    derive_revenue_market_treasury_v1, AuthenticatedRevenuePolicyRecordV2,
};
use super::source_occurrence_foundation_v1::AuthenticatedPreRootSourceOccurrenceV3;

const CURRENT_MARKET_FOUNDATION_GRAPH_AUTHENTICATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/current-market-foundation-graph-authentication/v4\0";
const PERSISTED_MARKET_FOUNDATION_GRAPH_AUTHENTICATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/persisted-market-foundation-graph-authentication/v4\0";

/// Fixed non-outcome accounts in the compact persisted GraphV4 frame.
pub(crate) const PERSISTED_FOUNDATION_GRAPH_FIXED_ACCOUNT_COUNT_V4: usize = 18;

/// Move-only authority over the exact current 50-slot physical graph.
///
/// Private fields prevent either a graph body or a graph ID from being
/// substituted for the complete authenticated predecessor chain.
#[derive(Debug)]
pub(crate) struct AuthenticatedCurrentMarketFoundationGraphV4 {
    id: ContentId,
    graph: MarketFoundationAccountGraphV4,
    graph_id: MarketFoundationAccountGraphV4Id,
    schedule_id: MarketFoundationScheduleV4Id,
    physical_founder_id: ContentId,
    physical_capitalization_id: ContentId,
    family_policy_authentication_id: ContentId,
    source_occurrence_id: ContentId,
    revenue_record_account: Pubkey,
    revenue_record_semantic_id: ContentId,
}

impl AuthenticatedCurrentMarketFoundationGraphV4 {
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn graph(&self) -> &MarketFoundationAccountGraphV4 {
        &self.graph
    }

    pub(crate) const fn graph_id(&self) -> MarketFoundationAccountGraphV4Id {
        self.graph_id
    }

    pub(crate) const fn schedule_id(&self) -> MarketFoundationScheduleV4Id {
        self.schedule_id
    }

    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.graph.market_instance_id
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.graph.generation
    }

    pub(crate) const fn physical_founder_id(&self) -> ContentId {
        self.physical_founder_id
    }

    pub(crate) const fn physical_capitalization_id(&self) -> ContentId {
        self.physical_capitalization_id
    }

    pub(crate) const fn family_policy_authentication_id(&self) -> ContentId {
        self.family_policy_authentication_id
    }

    pub(crate) const fn source_occurrence_id(&self) -> ContentId {
        self.source_occurrence_id
    }

    pub(crate) const fn revenue_record_account(&self) -> Pubkey {
        self.revenue_record_account
    }

    pub(crate) const fn revenue_record_semantic_id(&self) -> ContentId {
        self.revenue_record_semantic_id
    }
}

/// Narrow interface shared by the instruction-local action-14 graph and the
/// hostile-reconstructed action-15 graph.
///
/// The latter can never be downgraded into the former: bootstrap-only physical
/// getters are deliberately absent.
pub(crate) trait AuthenticatedCurrentMarketFoundationGraphAuthorityV4 {
    fn authentication_id(&self) -> ContentId;
    fn graph(&self) -> &MarketFoundationAccountGraphV4;
    fn graph_id(&self) -> MarketFoundationAccountGraphV4Id;
    fn schedule_id(&self) -> MarketFoundationScheduleV4Id;
    fn market_instance_id(&self) -> MarketInstanceV2Id;
    fn generation(&self) -> u64;
}

impl AuthenticatedCurrentMarketFoundationGraphAuthorityV4
    for AuthenticatedCurrentMarketFoundationGraphV4
{
    fn authentication_id(&self) -> ContentId { self.id }
    fn graph(&self) -> &MarketFoundationAccountGraphV4 { &self.graph }
    fn graph_id(&self) -> MarketFoundationAccountGraphV4Id { self.graph_id }
    fn schedule_id(&self) -> MarketFoundationScheduleV4Id { self.schedule_id }
    fn market_instance_id(&self) -> MarketInstanceV2Id { self.graph.market_instance_id }
    fn generation(&self) -> u64 { self.graph.generation }
}

/// Move-only reconstruction of the exact GraphV4 persisted by MarketReplayV2.
#[derive(Debug)]
pub(crate) struct AuthenticatedPersistedMarketFoundationGraphV4 {
    id: ContentId,
    graph: MarketFoundationAccountGraphV4,
    graph_id: MarketFoundationAccountGraphV4Id,
    schedule_id: MarketFoundationScheduleV4Id,
    replay_account: Pubkey,
    replay_authentication_id: ContentId,
}

impl AuthenticatedPersistedMarketFoundationGraphV4 {
    pub(crate) const fn replay_account(&self) -> Pubkey { self.replay_account }
    pub(crate) const fn replay_authentication_id(&self) -> ContentId {
        self.replay_authentication_id
    }
}

impl AuthenticatedCurrentMarketFoundationGraphAuthorityV4
    for AuthenticatedPersistedMarketFoundationGraphV4
{
    fn authentication_id(&self) -> ContentId { self.id }
    fn graph(&self) -> &MarketFoundationAccountGraphV4 { &self.graph }
    fn graph_id(&self) -> MarketFoundationAccountGraphV4Id { self.graph_id }
    fn schedule_id(&self) -> MarketFoundationScheduleV4Id { self.schedule_id }
    fn market_instance_id(&self) -> MarketInstanceV2Id { self.graph.market_instance_id }
    fn generation(&self) -> u64 { self.graph.generation }
}

fn account_id(account: Pubkey) -> ContentId {
    ContentId::from_bytes(account.to_bytes())
}

fn set_slot(
    account_ids: &mut [ContentId; MARKET_FOUNDATION_SLOT_COUNT_V4],
    slot: MarketFoundationSlotV4,
    account: Pubkey,
) -> Outcome<()> {
    let index = slot
        .index()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        account != Pubkey::default() && account_ids[index].is_zero(),
        ClutchError::MismatchedState,
    )?;
    account_ids[index] = account_id(account);
    Ok(())
}

/// Sole semantic constructor for the canonical 50-slot GraphV4 body. Slots
/// 0..=46 are program-derived from the authenticated Market/generation and
/// ScheduleV4; only General's three chain-authenticated treasury coordinates
/// are supplied for slots 47..=49.
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_current_market_foundation_graph_body_v4(
    program_id: &Pubkey,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    schedule: &MarketFoundationScheduleV4,
    treasury_position: Pubkey,
    treasury_replay: Pubkey,
    treasury_service_ledger: Pubkey,
) -> Outcome<MarketFoundationAccountGraphV4> {
    schedule
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        generation != 0
            && treasury_position != Pubkey::default()
            && treasury_replay != Pubkey::default()
            && treasury_service_ledger != Pubkey::default()
            && treasury_position != treasury_replay
            && treasury_position != treasury_service_ledger
            && treasury_replay != treasury_service_ledger,
        ClutchError::MismatchedState,
    )?;
    let market = market_instance_id.bytes();
    let market_binding = seeds::general_v2_market_binding_pda(program_id, &market).0;
    let market_runtime =
        seeds::general_v2_market_runtime_pda(program_id, &market_binding.to_bytes()).0;
    let resolution = seeds::resolution_v5_pda(program_id, &market).0;
    let fractional_policy =
        seeds::fractional_policy_v3_pda(program_id, &market, &resolution.to_bytes()).0;
    let fractional_ledger =
        seeds::fractional_ledger_v1_pda(program_id, &fractional_policy.to_bytes()).0;
    let mut account_ids = [ContentId::ZERO; MARKET_FOUNDATION_SLOT_COUNT_V4];
    for (slot, account) in [
        (
            MarketFoundationSlotV4::LifecycleRoot,
            seeds::product_market_lifecycle_root_pda(program_id, &market, generation).0,
        ),
        (MarketFoundationSlotV4::MarketBinding, market_binding),
        (MarketFoundationSlotV4::MarketRuntime, market_runtime),
        (MarketFoundationSlotV4::Hoard, seeds::hoard_v2_pda(program_id, &market).0),
        (
            MarketFoundationSlotV4::ClaimLedger,
            seeds::claim_ledger_v3_pda(program_id, &market).0,
        ),
        (
            MarketFoundationSlotV4::FailureAdmissionRoot,
            seeds::failure_market_root_v2_pda(program_id, &market, generation).0,
        ),
        (
            MarketFoundationSlotV4::FailureRuntimeRoot,
            seeds::failure_external_root_pda(program_id, &market, generation).0,
        ),
        (
            MarketFoundationSlotV4::FailureReplay,
            seeds::failure_market_replay_v2_pda(program_id, &market, generation).0,
        ),
        (
            MarketFoundationSlotV4::FailureIntervalWork,
            seeds::failure_market_interval_cell_v2_pda(program_id, &market, generation).0,
        ),
        (
            MarketFoundationSlotV4::FailureIntervalHistory,
            seeds::failure_market_interval_history_v2_pda(program_id, &market, generation).0,
        ),
        (MarketFoundationSlotV4::ResolutionV5, resolution),
        (MarketFoundationSlotV4::FractionalPolicy, fractional_policy),
        (MarketFoundationSlotV4::FractionalLedger, fractional_ledger),
        (
            MarketFoundationSlotV4::ProductReplayAnchor,
            seeds::product_market_lifecycle_replay_v2_pda(program_id, &market).0,
        ),
        (
            MarketFoundationSlotV4::HoardCollateralVault,
            seeds::hoard_token_v2_pda(program_id, &market).0,
        ),
        (MarketFoundationSlotV4::GeneralTreasuryPosition, treasury_position),
        (MarketFoundationSlotV4::GeneralTreasuryReplay, treasury_replay),
        (
            MarketFoundationSlotV4::TreasuryServiceLedger,
            treasury_service_ledger,
        ),
    ] {
        set_slot(&mut account_ids, slot, account)?;
    }
    let mut outcome = 0u8;
    while outcome < schedule.outcome_count {
        set_slot(
            &mut account_ids,
            MarketFoundationSlotV4::OutcomeMint(outcome),
            seeds::outcome_mint_v2_pda(program_id, &market, outcome).0,
        )?;
        set_slot(
            &mut account_ids,
            MarketFoundationSlotV4::OutcomeCustody(outcome),
            seeds::outcome_custody_v1_pda(program_id, &market, generation, outcome).0,
        )?;
        outcome = outcome.checked_add(1).ok_or(ClutchError::Arithmetic)?;
    }
    let graph = MarketFoundationAccountGraphV4 {
        market_instance_id,
        generation,
        foundation_schedule_id: schedule_id,
        account_ids,
    };
    graph
        .validate(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(graph)
}

/// Rebuild the compact physical GraphV4 frame selected by the permanent
/// MarketReplayV2 binding.
///
/// Account order is slots 0..=14, active outcome mints, active outcome
/// custodies, then slots 47..=49. Inactive outcome coordinates remain zero in
/// the canonical graph. No graph bytes, generation, or account key is accepted
/// from instruction data.
#[inline(never)]
pub(crate) fn authenticate_persisted_market_foundation_graph_v4(
    program_id: &Pubkey,
    replay: &AuthenticatedMarketLifecycleReplayV2,
    schedule: &MarketFoundationScheduleV4,
    accounts: &[solana_account_info::AccountInfo<'_>],
) -> Outcome<AuthenticatedPersistedMarketFoundationGraphV4> {
    schedule
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let outcome_count = usize::from(schedule.outcome_count);
    let expected_count = PERSISTED_FOUNDATION_GRAPH_FIXED_ACCOUNT_COUNT_V4
        .checked_add(outcome_count.checked_mul(2).ok_or(ClutchError::Arithmetic)?)
        .ok_or(ClutchError::Arithmetic)?;
    require(accounts.len() == expected_count, ClutchError::AccountCount)?;
    let replay_binding = replay.state().binding();
    require(
        replay_binding.foundation_schedule_id == schedule_id
            && replay_binding.generation == replay.generation()
            && replay_binding.generation != 0,
        ClutchError::MismatchedState,
    )?;
    let mut left = 0usize;
    while left < accounts.len() {
        require(
            !accounts[left].is_signer && !accounts[left].executable,
            ClutchError::MismatchedState,
        )?;
        let mut right = left.checked_add(1).ok_or(ClutchError::Arithmetic)?;
        while right < accounts.len() {
            require(accounts[left].key != accounts[right].key, ClutchError::AccountAlias)?;
            right = right.checked_add(1).ok_or(ClutchError::Arithmetic)?;
        }
        left = left.checked_add(1).ok_or(ClutchError::Arithmetic)?;
    }
    let mint_start = 15usize;
    let custody_start = mint_start.checked_add(outcome_count).ok_or(ClutchError::Arithmetic)?;
    let treasury_start = custody_start.checked_add(outcome_count).ok_or(ClutchError::Arithmetic)?;
    let graph = derive_current_market_foundation_graph_body_v4(
        program_id,
        replay_binding.market_instance_id,
        replay_binding.generation,
        schedule,
        *accounts[treasury_start].key,
        *accounts[treasury_start + 1].key,
        *accounts[treasury_start + 2].key,
    )?;
    let mut index = 0usize;
    while index <= 14 {
        require(
            graph.account_ids[index] == account_id(*accounts[index].key),
            ClutchError::MismatchedState,
        )?;
        index = index.checked_add(1).ok_or(ClutchError::Arithmetic)?;
    }
    index = 0;
    while index < outcome_count {
        require(
            graph.account_ids[15 + index] == account_id(*accounts[mint_start + index].key)
                && graph.account_ids[31 + index]
                    == account_id(*accounts[custody_start + index].key),
            ClutchError::MismatchedState,
        )?;
        index = index.checked_add(1).ok_or(ClutchError::Arithmetic)?;
    }
    let graph_id = graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        graph_id == replay_binding.foundation_account_graph_id
            && graph.account(MarketFoundationSlotV4::ProductReplayAnchor)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == replay.account().to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PERSISTED_MARKET_FOUNDATION_GRAPH_AUTHENTICATION_DOMAIN_V4,
            replay.account().as_ref(),
            &replay.authentication_id().bytes(),
            &replay_binding
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes(),
            &schedule_id.bytes(),
            &graph_id.bytes(),
            &replay_binding.market_instance_id.bytes(),
            &replay_binding.generation.to_le_bytes(),
            &[schedule.outcome_count],
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedPersistedMarketFoundationGraphV4 {
        id,
        graph,
        graph_id,
        schedule_id,
        replay_account: replay.account(),
        replay_authentication_id: replay.authentication_id(),
    })
}

/// Derive the complete current physical graph without accepting caller graph
/// bytes or caller-selected Market/generation coordinates.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn authenticate_current_market_foundation_graph_v4(
    program_id: &Pubkey,
    physical: &AuthenticatedSeriesPhysicalFounderV5,
    family_policy: &AuthenticatedMarketFamilyCapabilityPolicyArtifactV1,
    source: &AuthenticatedPreRootSourceOccurrenceV3,
    schedule: &MarketFoundationScheduleV4,
    revenue: AuthenticatedRevenuePolicyRecordV2,
) -> Outcome<AuthenticatedCurrentMarketFoundationGraphV4> {
    schedule
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let occurrence = source.occurrence();
    let source_facts = source.capitalization().facts();
    let market_instance_id = MarketInstanceV2Id::from_bytes(occurrence.market_instance_id().bytes());
    let generation = derive_initial_market_generation_v2(
        market_instance_id,
        family_policy.policy_id(),
        RegistryProgramReleaseV2Id::from_bytes(family_policy.registry_release_id().bytes()),
        RegistryCapabilityProfileV4Id::from_bytes(
            family_policy.capability_profile_id().bytes(),
        ),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let capitalization = physical.capitalization();
    require(
        market_instance_id.content_id() == source_facts.market_instance_id
            && generation != 0
            && generation == source_facts.generation
            && physical.foundation_schedule_id() == schedule_id
            && physical.series_plan_id().content_id() == occurrence.series_plan_id()
            && physical.series_plan_id().content_id() == source_facts.series_plan_id
            && physical.attachment_plan_id() == occurrence.attachment_plan_id()
            && physical.attachment_plan_id() == source_facts.attachment_plan_id
            && capitalization.funding_quote_id() == source_facts.funding_quote_id
            && physical.registry_release_id() == source_facts.registry_release_id
            && physical.capability_profile_id() == source_facts.capability_profile_id
            && physical.registry_release_id() == family_policy.registry_release_id()
            && physical.capability_profile_id() == family_policy.capability_profile_id()
            && physical.id() == family_policy.physical_founder_id()
            && physical.capitalization_id() == family_policy.physical_capitalization_id()
            && physical.registry_capability_after_id()
                == family_policy.registry_capability_id()
            && physical.attachment_plan_id() == family_policy.attachment_plan_id()
            && physical.collateral_realm_id().bytes() == revenue.realm().bytes(),
        ClutchError::MismatchedState,
    )?;

    let market = market_instance_id.bytes();
    let market_binding = seeds::general_v2_market_binding_pda(program_id, &market).0;
    let market_runtime =
        seeds::general_v2_market_runtime_pda(program_id, &market_binding.to_bytes()).0;
    let treasury = derive_revenue_market_treasury_v1(
        program_id,
        revenue,
        Hash32::from_bytes(market),
        market_runtime,
    )?;

    let graph = derive_current_market_foundation_graph_body_v4(
        program_id,
        market_instance_id,
        generation,
        schedule,
        treasury.treasury_position_account(),
        treasury.treasury_replay_account(),
        treasury.treasury_service_ledger_account(),
    )?;
    let graph_id = graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let revenue_record_semantic_id = ContentId::from_bytes(revenue.record_semantic_id().bytes());
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            CURRENT_MARKET_FOUNDATION_GRAPH_AUTHENTICATION_DOMAIN_V4,
            program_id.as_ref(),
            &physical.id().bytes(),
            &physical.capitalization_id().bytes(),
            &family_policy.id().bytes(),
            &source.id().bytes(),
            revenue.record_account().as_ref(),
            &revenue_record_semantic_id.bytes(),
            &schedule_id.bytes(),
            &graph_id.bytes(),
            &market_instance_id.bytes(),
            &generation.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedCurrentMarketFoundationGraphV4 {
        id,
        graph,
        graph_id,
        schedule_id,
        physical_founder_id: physical.id(),
        physical_capitalization_id: physical.capitalization_id(),
        family_policy_authentication_id: family_policy.id(),
        source_occurrence_id: source.id(),
        revenue_record_account: revenue.record_account(),
        revenue_record_semantic_id,
    })
}

#[cfg(test)]
mod source_contract_tests {
    #[test]
    fn graph_is_derived_without_caller_graph_bytes_or_generation() {
        let source = include_str!("product_market_foundation_graph_v4_current.rs");
        assert!(source.contains("derive_initial_market_generation_v2("));
        assert!(source.contains("product_market_lifecycle_replay_v2_pda(program_id, &market)"));
        assert!(source.contains("MarketFoundationSlotV4::GeneralTreasuryPosition"));
        assert!(source.contains("MarketFoundationSlotV4::TreasuryServiceLedger"));
        assert!(!source.contains("caller_graph"));
        assert!(!source.contains("expected_generation:"));
    }

    #[test]
    fn graph_authority_is_move_only_and_retains_the_full_preimage() {
        let source = include_str!("product_market_foundation_graph_v4_current.rs");
        assert!(source.contains("pub(crate) struct AuthenticatedCurrentMarketFoundationGraphV4"));
        assert!(source.contains("graph: MarketFoundationAccountGraphV4"));
        assert!(!source.contains(
            "#[derive(Clone, Copy, Debug)]\npub(crate) struct AuthenticatedCurrentMarketFoundationGraphV4"
        ));
        assert!(!source.contains("pub(crate) fn into_graph"));
    }
}
