use crate::{
    ContentId, Error, EvidenceOnlyRecoveryPolicyV1, MarketGenesisProfileV1, NativeClaimBasisId,
    NativeClaimBasisV1, ProductTemplateV4, Result, SeriesPlanV4, MAX_BASIS_DEGREE, MAX_OUTCOMES,
    MAX_PAYOUTS, MAX_RECOVERY_ATTEMPTS,
};

/// Exact semantic-owner identities admitted by one capability profile.
///
/// This is an ephemeral adapter projection, not a persisted registry codec and
/// not evidence that a registry release was authenticated. A live adapter must
/// construct it only after authenticating the authoritative release manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilitySemanticOwnersV1 {
    /// Exact recurring source-plane contract or release.
    pub source_plane_contract_id: ContentId,
    /// Exact source description.
    pub source_spec_id: ContentId,
    /// Exact source-neutral summary program.
    pub summary_program_id: ContentId,
    /// Exact native claim basis.
    pub native_claim_basis_id: NativeClaimBasisId,
    /// Exact evidence-only recovery policy.
    pub evidence_only_recovery_policy_id: crate::EvidenceOnlyRecoveryPolicyId,
    /// Exact product compiler release.
    pub product_compiler_release_id: ContentId,
    /// Exact order price-grid semantics.
    pub price_grid_id: ContentId,
    /// Exact fee semantics.
    pub fee_policy_id: ContentId,
    /// Exact settlement/evidence relation semantics.
    pub relation_policy_id: ContentId,
    /// Exact score semantics.
    pub score_policy_id: ContentId,
    /// Exact candidate lifecycle semantics.
    pub candidate_lifecycle_policy_id: ContentId,
    /// Exact candidate liveness semantics.
    pub candidate_liveness_policy_id: ContentId,
    /// Exact counted-retirement semantics.
    pub retirement_policy_id: ContentId,
}

impl CapabilitySemanticOwnersV1 {
    pub(crate) fn validate(self) -> Result<()> {
        for id in [
            self.source_plane_contract_id,
            self.source_spec_id,
            self.summary_program_id,
            self.native_claim_basis_id.content_id(),
            self.evidence_only_recovery_policy_id.content_id(),
            self.product_compiler_release_id,
            self.price_grid_id,
            self.fee_policy_id,
            self.relation_policy_id,
            self.score_policy_id,
            self.candidate_lifecycle_policy_id,
            self.candidate_liveness_policy_id,
            self.retirement_policy_id,
        ] {
            id.validate()?;
        }
        Ok(())
    }
}

/// Realm/Profile collateral facts projected by an authenticated adapter.
///
/// These facts remain owned by the immutable Realm/Profile registry artifact.
/// This value only makes the complete pure join explicit and testable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmCollateralProjectionV1 {
    /// Immutable Realm identity.
    pub realm_id: ContentId,
    /// Immutable collateral Profile identity.
    pub profile_id: ContentId,
    /// Exact admitted collateral mint.
    pub collateral_mint: ContentId,
    /// Exact admitted token program.
    pub token_program: ContentId,
    /// Canonical neutral incinerator or disposition account.
    pub neutral_incinerator: ContentId,
    /// Canonical System-owned destination for unowned lamport residue.
    pub neutral_lamport_sink: ContentId,
    /// Maximum per-market collateral cap admitted by this Realm/Profile.
    pub market_collateral_cap_ceiling: u64,
}

impl RealmCollateralProjectionV1 {
    pub(crate) fn validate(self) -> Result<()> {
        for id in [
            self.realm_id,
            self.profile_id,
            self.collateral_mint,
            self.token_program,
            self.neutral_incinerator,
            self.neutral_lamport_sink,
        ] {
            id.validate()?;
        }
        if self.market_collateral_cap_ceiling == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
}

/// Complete market-core projection of one registry capability profile.
///
/// Numeric registry values are opaque inputs allocated by the central
/// registry; this crate allocates none. This type is deliberately not a codec
/// and has no identity. It makes the adapter's complete equality and support
/// obligations explicit, but cannot authenticate the registry release itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryCapabilityProjectionV1 {
    /// Exact central registry release authenticated by the adapter.
    pub registry_release_id: ContentId,
    /// Exact capability profile selected by the market GenesisProfile.
    pub capability_profile_id: ContentId,
    /// Exact admitted statistic registry value.
    pub statistic_registry_value: u16,
    /// Exact admitted coverage-policy registry value.
    pub coverage_policy_registry_value: u16,
    /// Exact admitted ambiguity-policy registry value.
    pub ambiguity_policy_registry_value: u8,
    /// Exact admitted edge-policy registry value.
    pub edge_policy_registry_value: u8,
    /// Exact registry-owned terminal `BURN` disposition value.
    pub burn_terminal_disposition_registry_value: u16,
    /// Whether basis degrees zero through three are executable.
    pub supported_basis_degrees: [bool; 4],
    /// Maximum executable native outcome count.
    pub max_outcome_count: u8,
    /// Maximum executable degree-zero finite payout count.
    pub max_degree_zero_payout_count: u8,
    /// Maximum executable evidence-only recovery attempt count.
    pub max_recovery_attempt_count: u8,
    /// Inclusive minimum coverage-policy parameter.
    pub min_coverage_policy_parameter: u64,
    /// Inclusive maximum coverage-policy parameter.
    pub max_coverage_policy_parameter: u64,
    /// Maximum executable raw observation span.
    pub max_window_span_buckets: u64,
    /// Maximum executable finite Series occurrence count.
    pub max_series_instance_count: u32,
    /// Exact admitted semantic-owner identities.
    pub semantic_owners: CapabilitySemanticOwnersV1,
    /// Exact immutable Realm/Profile collateral projection.
    pub realm_collateral: RealmCollateralProjectionV1,
}

impl RegistryCapabilityProjectionV1 {
    fn validate_shape(self) -> Result<()> {
        self.registry_release_id.validate()?;
        self.capability_profile_id.validate()?;
        self.semantic_owners.validate()?;
        self.realm_collateral.validate()?;
        let hard_max_outcomes = u8::try_from(MAX_OUTCOMES).map_err(|_| Error::InvalidParameter)?;
        let hard_max_payouts = u8::try_from(MAX_PAYOUTS).map_err(|_| Error::InvalidParameter)?;
        let hard_max_attempts =
            u8::try_from(MAX_RECOVERY_ATTEMPTS).map_err(|_| Error::InvalidParameter)?;
        if self.statistic_registry_value == 0
            || self.coverage_policy_registry_value == 0
            || self.ambiguity_policy_registry_value == 0
            || self.edge_policy_registry_value == 0
            || self.burn_terminal_disposition_registry_value == 0
            || self.max_outcome_count == 0
            || self.max_outcome_count > hard_max_outcomes
            || self.max_degree_zero_payout_count > hard_max_payouts
            || self.max_recovery_attempt_count == 0
            || self.max_recovery_attempt_count > hard_max_attempts
            || self.min_coverage_policy_parameter > self.max_coverage_policy_parameter
            || self.max_window_span_buckets == 0
            || self.max_series_instance_count == 0
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Validate the total market-core structural join for one Series.
    ///
    /// Success proves equality with this supplied projection, not that the
    /// projection came from an authentic registry account or release manifest.
    pub fn validate_complete_join(
        &self,
        series: &SeriesPlanV4,
        template: &ProductTemplateV4,
        basis: &NativeClaimBasisV1,
        recovery: &EvidenceOnlyRecoveryPolicyV1,
        genesis: &MarketGenesisProfileV1,
    ) -> Result<()> {
        self.validate_shape()?;
        series.validate_shape()?;
        template.validate_bindings(basis, recovery)?;
        genesis.validate_native_lot(basis)?;

        let owners = self.semantic_owners;
        if series.product_template_id != template.id()?
            || series.market_genesis_profile_id != genesis.id()?
            || self.capability_profile_id != genesis.capability_profile_id
            || self.statistic_registry_value != template.statistic_registry_value
            || self.coverage_policy_registry_value != template.coverage_policy_registry_value
            || self.ambiguity_policy_registry_value != basis.ambiguity_policy_registry_value
            || self.edge_policy_registry_value != basis.edge_policy_registry_value
            || self.burn_terminal_disposition_registry_value
                != genesis.terminal_disposition_registry_value
            || owners.source_plane_contract_id != template.source_plane_contract_id
            || owners.source_spec_id != template.source_spec_id
            || owners.summary_program_id != template.summary_program_id
            || owners.native_claim_basis_id != template.native_claim_basis_id
            || owners.evidence_only_recovery_policy_id != template.evidence_only_recovery_policy_id
            || owners.product_compiler_release_id != template.compiler_release_id
            || owners.price_grid_id != genesis.price_grid_id
            || owners.fee_policy_id != genesis.fee_policy_id
            || owners.relation_policy_id != genesis.relation_policy_id
            || owners.score_policy_id != genesis.score_policy_id
            || owners.candidate_lifecycle_policy_id != genesis.candidate_lifecycle_policy_id
            || owners.candidate_liveness_policy_id != genesis.candidate_liveness_policy_id
            || owners.retirement_policy_id != genesis.retirement_policy_id
            || self.realm_collateral.realm_id != genesis.realm_id
            || self.realm_collateral.profile_id != genesis.profile_id
        {
            return Err(Error::MismatchedArtifact);
        }

        let degree = usize::from(basis.basis_degree);
        if basis.basis_degree > MAX_BASIS_DEGREE
            || !self.supported_basis_degrees[degree]
            || basis.outcome_count > self.max_outcome_count
            || basis.payout_count > self.max_degree_zero_payout_count
            || recovery.attempt_count > self.max_recovery_attempt_count
            || template.coverage_policy_parameter < self.min_coverage_policy_parameter
            || template.coverage_policy_parameter > self.max_coverage_policy_parameter
            || template.window_span_buckets > self.max_window_span_buckets
            || series.instance_count > self.max_series_instance_count
            || series.market_collateral_cap > self.realm_collateral.market_collateral_cap_ceiling
        {
            return Err(Error::UnsupportedCapability);
        }
        Ok(())
    }
}
