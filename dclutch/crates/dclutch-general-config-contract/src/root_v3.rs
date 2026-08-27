//! Runtime-width General activation behind the common Core/Trading boundary.
//!
//! This successor path admits only [`GeneralConfigV3`](crate::v3::GeneralConfigV3).
//! The manifest release is the complete CapabilityProgramSet identity. The
//! action-selected descriptor remains downstream finalized content and cannot
//! become a second activation authority.

use dclutch_capability_contract::{
    ActivationPolicy, CapabilityManifestV1, FundingCustodyObservationV1, FundingStateV1,
    FundingStatus,
};
use dclutch_core_contract::{ContentId, MarketRoot, Phase};

use crate::{
    root::{
        DustSafeRootCreationV2, GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_ROOT_SCHEMA_ID_V2,
        GeneralActivationDispositionV2, GeneralLifecycleV2, GeneralRootV2, RootError, RootResult,
    },
    v3::GeneralConfigV3,
};

impl GeneralRootV2 {
    /// Require exact V3 root/config/Market context before consuming policy.
    #[allow(clippy::too_many_arguments)]
    pub fn require_hot_context_v3(
        self,
        market_key: [u8; 32],
        market_generation: u64,
        market_claim_basis_id: [u8; 32],
        authenticated_config_id: [u8; 32],
        authenticated_program_set_id: [u8; 32],
        config: GeneralConfigV3,
        expected_revision: u64,
    ) -> RootResult<()> {
        if self.lifecycle() != GeneralLifecycleV2::Active
            || self.market() != market_key
            || self.config_id() != authenticated_config_id
            || self.generation() != market_generation
            || self.revision() != expected_revision
        {
            return Err(RootError::CoordinateMismatch);
        }
        config.require_market(market_generation, market_claim_basis_id)?;
        config.require_program_set(authenticated_program_set_id)?;
        Ok(())
    }
}

/// General-owned V3 result behind a Core-authenticated activation envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOwnedActivationV3 {
    disposition: GeneralActivationDispositionV2,
    root_state: GeneralRootV2,
    funding_after: FundingStateV1,
    creation: DustSafeRootCreationV2,
}

impl GeneralOwnedActivationV3 {
    /// Whether the adapter creates the root or proves exact replay.
    pub const fn disposition(self) -> GeneralActivationDispositionV2 {
        self.disposition
    }

    /// Exact mutable General root tail to store or preserve.
    pub const fn root_state(self) -> GeneralRootV2 {
        self.root_state
    }

    /// Exact capability-funding poststate owned by General.
    pub const fn funding_after(self) -> FundingStateV1 {
        self.funding_after
    }

    /// Dust-safe root funding split; zero on replay.
    pub const fn creation(self) -> DustSafeRootCreationV2 {
        self.creation
    }
}

/// Complete V3 activation plan including Core-owned Market poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralActivationV3 {
    disposition: GeneralActivationDispositionV2,
    root: GeneralRootV2,
    market_after: MarketRoot,
    funding_after: FundingStateV1,
    creation: DustSafeRootCreationV2,
    manifest_entry_index: u16,
}

impl GeneralActivationV3 {
    /// Whether physical root creation is required.
    pub const fn disposition(self) -> GeneralActivationDispositionV2 {
        self.disposition
    }

    /// Exact General root poststate.
    pub const fn root(self) -> GeneralRootV2 {
        self.root
    }

    /// Exact Core-owned Market poststate.
    pub const fn market_after(self) -> MarketRoot {
        self.market_after
    }

    /// Exact General-owned FundingState poststate.
    pub const fn funding_after(self) -> FundingStateV1 {
        self.funding_after
    }

    /// Dust-safe root funding split; zero on replay.
    pub const fn creation(self) -> DustSafeRootCreationV2 {
        self.creation
    }

    /// Unique manifest entry selected by kind and config.
    pub const fn manifest_entry_index(self) -> u16 {
        self.manifest_entry_index
    }
}

/// Activate General-owned V3 state after Core authenticates the envelope.
#[allow(clippy::too_many_arguments)]
pub fn activate_general_owned_v3(
    market_key: [u8; 32],
    generation: u64,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
    config_id: ContentId,
    config: GeneralConfigV3,
    funding: FundingStateV1,
    custody: FundingCustodyObservationV1,
    current_slot: u64,
    exact_root_rent_lamports: u64,
    precreation_lamports: u64,
    existing_root_state: Option<GeneralRootV2>,
) -> RootResult<GeneralOwnedActivationV3> {
    if zero(&market_key) || exact_root_rent_lamports == 0 {
        return Err(RootError::ZeroIdentity);
    }
    if config.generation() != generation || funding.entry_index() != entry_index {
        return Err(RootError::CapabilityMismatch);
    }
    let entry = manifest.entry(entry_index)?;
    require_entry(entry, config_id, config)?;
    let expected = GeneralRootV2::active(market_key, config_id.to_bytes(), generation)?;
    if let Some(present) = existing_root_state {
        if present != expected || precreation_lamports != exact_root_rent_lamports {
            return Err(RootError::ActivationReplayMismatch);
        }
        funding.validate_against(manifest_id, manifest, custody)?;
        if funding.status() != FundingStatus::Active || funding.activation_slot() != current_slot {
            return Err(RootError::ActivationReplayMismatch);
        }
        return Ok(GeneralOwnedActivationV3 {
            disposition: GeneralActivationDispositionV2::Idempotent,
            root_state: present,
            funding_after: funding,
            creation: DustSafeRootCreationV2::new(0, 0),
        });
    }
    let mut funding_after = funding;
    let debit = funding_after.activate(manifest_id, manifest, custody, current_slot)?;
    if debit.rent_lamports() != exact_root_rent_lamports || debit.creation_lamports() != 0 {
        return Err(RootError::ActivationFundingMismatch);
    }
    Ok(GeneralOwnedActivationV3 {
        disposition: GeneralActivationDispositionV2::Create,
        root_state: expected,
        funding_after,
        creation: DustSafeRootCreationV2::new(exact_root_rent_lamports, precreation_lamports),
    })
}

/// Plan Market, root, and FundingState V3 poststates atomically.
#[allow(clippy::too_many_arguments)]
pub fn plan_general_activation_v3(
    market_key: [u8; 32],
    market: MarketRoot,
    expected_market_child_count: u64,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    config_id: ContentId,
    config: GeneralConfigV3,
    funding: FundingStateV1,
    custody: FundingCustodyObservationV1,
    current_slot: u64,
    exact_root_rent_lamports: u64,
    precreation_lamports: u64,
    existing_root: Option<GeneralRootV2>,
) -> RootResult<GeneralActivationV3> {
    if zero(&market_key) || exact_root_rent_lamports == 0 {
        return Err(RootError::ZeroIdentity);
    }
    market.validate()?;
    let identity = market.identity();
    if identity.capability_manifest_id() != manifest_id {
        return Err(RootError::CapabilityMismatch);
    }
    config.require_market(identity.generation(), identity.claim_basis_id().to_bytes())?;
    let (entry_index, entry) = select_entry(manifest, config_id)?;
    require_entry(entry, config_id, config)?;
    match (entry.activation_policy(), market.phase()) {
        (ActivationPolicy::RequiredAtFounding, Phase::Founding)
        | (ActivationPolicy::PrepaidLazy, Phase::Founding | Phase::Open) => {}
        _ => return Err(RootError::ActivationPhaseMismatch),
    }
    let expected = GeneralRootV2::active(market_key, config_id.to_bytes(), identity.generation())?;
    if let Some(present) = existing_root {
        if present != expected {
            return Err(RootError::ActivationReplayMismatch);
        }
        funding.validate_against(manifest_id, manifest, custody)?;
        if funding.status() != FundingStatus::Active || funding.entry_index() != entry_index {
            return Err(RootError::ActivationReplayMismatch);
        }
        return Ok(GeneralActivationV3 {
            disposition: GeneralActivationDispositionV2::Idempotent,
            root: present,
            market_after: market,
            funding_after: funding,
            creation: DustSafeRootCreationV2::new(0, 0),
            manifest_entry_index: entry_index,
        });
    }
    if funding.entry_index() != entry_index {
        return Err(RootError::CapabilityMismatch);
    }
    let mut funding_after = funding;
    let debit = funding_after.activate(manifest_id, manifest, custody, current_slot)?;
    if debit.rent_lamports() != exact_root_rent_lamports || debit.creation_lamports() != 0 {
        return Err(RootError::ActivationFundingMismatch);
    }
    let mut market_after = market;
    market_after.register_child(identity.generation(), expected_market_child_count)?;
    Ok(GeneralActivationV3 {
        disposition: GeneralActivationDispositionV2::Create,
        root: expected,
        market_after,
        funding_after,
        creation: DustSafeRootCreationV2::new(exact_root_rent_lamports, precreation_lamports),
        manifest_entry_index: entry_index,
    })
}

fn select_entry(
    manifest: CapabilityManifestV1<'_>,
    config_id: ContentId,
) -> RootResult<(u16, dclutch_capability_contract::CapabilityEntryV1)> {
    let kind = ContentId::new(GENERAL_CAPABILITY_KIND_ID_V1)?;
    let mut selected = None;
    let mut index = 0_u16;
    while index < manifest.entry_count() {
        let entry = manifest.entry(index)?;
        if entry.kind_id() == kind && entry.config_id() == config_id {
            if selected.is_some() {
                return Err(RootError::CapabilityAmbiguous);
            }
            selected = Some((index, entry));
        }
        index = index.checked_add(1).ok_or(RootError::Arithmetic)?;
    }
    selected.ok_or(RootError::CapabilityMissing)
}

fn require_entry(
    entry: dclutch_capability_contract::CapabilityEntryV1,
    config_id: ContentId,
    config: GeneralConfigV3,
) -> RootResult<()> {
    if entry.kind_id().to_bytes() != GENERAL_CAPABILITY_KIND_ID_V1
        || entry.release_id().to_bytes() != config.program_set_id()
        || entry.config_id() != config_id
        || entry.capacity_profile_id().to_bytes() != config.capacity_profile_id()
        || entry.child_schema_id().to_bytes() != GENERAL_ROOT_SCHEMA_ID_V2
    {
        Err(RootError::CapabilityMismatch)
    } else {
        Ok(())
    }
}

fn zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use dclutch_capability_contract::{
        CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1, FundingAmountsV1,
        FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_core_contract::MarketIdentity;

    use super::*;
    use crate::v3::GeneralConfigV3Input;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn content(value: u8) -> ContentId {
        ContentId::new(id(value)).expect("nonzero content")
    }

    fn config_for_program_set(program_set_id: [u8; 32]) -> GeneralConfigV3 {
        GeneralConfigV3::new(GeneralConfigV3Input {
            capacity_profile_id: id(0x41),
            claim_basis_id: id(0x42),
            program_set_id,
            generation: 7,
            price_scale: 100,
            collection_slots: 10,
            selection_slots: 11,
            settlement_slots: 12,
            max_orders_per_candidate: u32::MAX,
            max_pages_per_candidate: u32::MAX,
            continuation_reward_lamports: 5,
            selection_policy_id: id(0x33),
            quote_surplus_beneficiary: id(0x43),
        })
        .expect("config")
    }

    fn quote() -> FundingQuoteV1 {
        let rent = CompartmentFundingV1::native_lamports(100).expect("rent");
        FundingQuoteV1::new(
            FundingAmountsV1::new(
                rent,
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            )
            .expect("amounts"),
            None,
        )
        .expect("quote")
    }

    fn entry(config_id: ContentId, config: GeneralConfigV3) -> CapabilityEntryV1 {
        CapabilityEntryV1::new(
            content_id(GENERAL_CAPABILITY_KIND_ID_V1),
            content_id(config.program_set_id()),
            config_id,
            content_id(config.capacity_profile_id()),
            content_id(GENERAL_ROOT_SCHEMA_ID_V2),
            content(0x82),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote(),
        )
        .expect("entry")
    }

    fn content_id(value: [u8; 32]) -> ContentId {
        ContentId::new(value).expect("content")
    }

    #[test]
    fn v3_activation_binds_program_set_and_replays_exactly() {
        let config = config_for_program_set(id(0x23));
        let config_id = content(0x51);
        let manifest_id = content(0x52);
        let entry = entry(config_id, config);
        let mut storage = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest = CapabilityManifestV1::encode_into(&[entry], &mut storage).expect("manifest");
        let pending = FundingCustodyObservationV1::native_only(120, 20).expect("custody");
        let funding = FundingStateV1::new(manifest_id, manifest, 0, pending).expect("funding");
        let created = activate_general_owned_v3(
            id(0x11),
            7,
            manifest_id,
            manifest,
            0,
            config_id,
            config,
            funding,
            pending,
            9,
            100,
            40,
            None,
        )
        .expect("activation");
        assert_eq!(
            created.disposition(),
            GeneralActivationDispositionV2::Create
        );
        assert_eq!(created.funding_after().status(), FundingStatus::Active);
        assert_eq!(created.creation().funding_top_up_lamports(), 60);
        let active = FundingCustodyObservationV1::native_only(20, 20).expect("active custody");
        let replay = activate_general_owned_v3(
            id(0x11),
            7,
            manifest_id,
            manifest,
            0,
            config_id,
            config,
            created.funding_after(),
            active,
            9,
            100,
            100,
            Some(created.root_state()),
        )
        .expect("replay");
        assert_eq!(
            replay.disposition(),
            GeneralActivationDispositionV2::Idempotent
        );
        assert_eq!(replay.root_state(), created.root_state());

        let funding_before = funding;
        assert_eq!(
            activate_general_owned_v3(
                id(0x11),
                7,
                manifest_id,
                manifest,
                0,
                config_id,
                config_for_program_set(id(0x24)),
                funding,
                pending,
                9,
                100,
                40,
                None,
            ),
            Err(RootError::CapabilityMismatch)
        );
        assert_eq!(funding, funding_before);
    }

    #[test]
    fn core_plan_and_hot_context_use_v3_without_product_width() {
        let config = config_for_program_set(id(0x23));
        let config_id = content(0x51);
        let manifest_id = content(0x52);
        let identity = MarketIdentity::new(
            content(0x61),
            content(0x62),
            content(0x42),
            content(0x64),
            manifest_id,
            7,
        );
        let market = MarketRoot::founding(identity, id(0x71)).expect("market");
        let entry = entry(config_id, config);
        let mut storage = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest = CapabilityManifestV1::encode_into(&[entry], &mut storage).expect("manifest");
        let custody = FundingCustodyObservationV1::native_only(120, 20).expect("custody");
        let funding = FundingStateV1::new(manifest_id, manifest, 0, custody).expect("funding");
        let plan = plan_general_activation_v3(
            id(0x11),
            market,
            0,
            manifest_id,
            manifest,
            config_id,
            config,
            funding,
            custody,
            9,
            100,
            40,
            None,
        )
        .expect("plan");
        assert_eq!(plan.market_after().outstanding_children(), 1);
        assert_eq!(plan.manifest_entry_index(), 0);
        assert_eq!(
            plan.root().require_hot_context_v3(
                id(0x11),
                7,
                id(0x42),
                config_id.to_bytes(),
                config.program_set_id(),
                config,
                1,
            ),
            Ok(())
        );
        for product_width in [1_u32, 258] {
            assert!(product_width > 0);
            assert_eq!(config.require_program_set(id(0x23)), Ok(()));
        }
    }
}
