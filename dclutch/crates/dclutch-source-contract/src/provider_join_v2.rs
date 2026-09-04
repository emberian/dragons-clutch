//! Current runtime-width Source-to-Pyth adapter obligation.
//!
//! Product Runtime V2 deliberately moved Source policy into separately
//! finalized records. This obligation rejoins those exact records without
//! recreating the old embedded Source-material authority. It still does not
//! authenticate SVM owners, Loader state, provider messages, or signatures;
//! those remain obligations of the Pyth SVM adapter.

use super::{
    ContentId, Error, NormalizedProviderEvidenceV1, ProviderReleaseV1, PythAdapterConfigV1,
    RecoveryAttemptV2, RecoveryPolicyV2, Result, SourceAccessProfile, SourceMaterialV3,
    SourceSpecV1, StatisticKind, StatisticSpecV1, WindowKind, WindowSpecV1,
};

/// Canonical finalized-record schema for [`SourceSpecV1`].
pub const SOURCE_SPEC_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/source-spec-v1";
/// SHA-256 of [`SOURCE_SPEC_SCHEMA_PREIMAGE_V1`].
pub const SOURCE_SPEC_SCHEMA_ID_V1: [u8; 32] = [
    0xcc, 0xea, 0xf8, 0xdb, 0xc2, 0xac, 0x3a, 0xe8, 0x11, 0xb5, 0x22, 0x19, 0x72, 0x92, 0x9c, 0xf3,
    0xfc, 0x34, 0x13, 0x72, 0x1b, 0x08, 0x0d, 0x56, 0x0f, 0xf9, 0x54, 0xb8, 0xab, 0x81, 0x86, 0xb6,
];
/// Canonical finalized-record schema for [`super::SourceCapacityProfileV1`].
pub const SOURCE_CAPACITY_PROFILE_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/source-capacity-profile-v1";
/// SHA-256 of [`SOURCE_CAPACITY_PROFILE_SCHEMA_PREIMAGE_V1`].
pub const SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1: [u8; 32] = [
    0x92, 0xfa, 0xdd, 0x2f, 0x51, 0x54, 0x82, 0xb7, 0x6e, 0x25, 0x55, 0x52, 0x4d, 0x57, 0x75, 0x3e,
    0x61, 0xcd, 0x42, 0xde, 0x40, 0xa3, 0x98, 0xf9, 0x6a, 0x17, 0x28, 0xc6, 0x4f, 0x28, 0x4e, 0x01,
];
/// Canonical finalized-record schema for [`ProviderReleaseV1`].
pub const PROVIDER_RELEASE_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/provider-release-v1";
/// SHA-256 of [`PROVIDER_RELEASE_SCHEMA_PREIMAGE_V1`].
pub const PROVIDER_RELEASE_SCHEMA_ID_V1: [u8; 32] = [
    0x39, 0x18, 0x9d, 0xfb, 0xce, 0xad, 0x27, 0x3f, 0x6f, 0x0f, 0x5e, 0x70, 0x28, 0x19, 0xe1, 0x88,
    0x58, 0x64, 0xb3, 0x7c, 0x53, 0x36, 0xfe, 0xe4, 0xd2, 0xb1, 0x8e, 0x53, 0xa8, 0x73, 0x9b, 0x13,
];
/// Canonical finalized-record schema for [`PythAdapterConfigV1`].
pub const PYTH_ADAPTER_CONFIG_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/pyth-adapter-config-v1";
/// SHA-256 of [`PYTH_ADAPTER_CONFIG_SCHEMA_PREIMAGE_V1`].
pub const PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1: [u8; 32] = [
    0x1a, 0xaa, 0x19, 0xb1, 0xb8, 0xc4, 0x56, 0xf5, 0xf5, 0x5f, 0x0b, 0xe8, 0x86, 0x41, 0x36, 0x37,
    0x93, 0x20, 0x5e, 0x84, 0x2d, 0x55, 0x87, 0x28, 0xbe, 0x2c, 0x8e, 0x23, 0xbb, 0xd2, 0xf7, 0x65,
];
/// Canonical finalized-record schema for [`WindowSpecV1`].
pub const WINDOW_SPEC_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/window-spec-v1";
/// SHA-256 of [`WINDOW_SPEC_SCHEMA_PREIMAGE_V1`].
pub const WINDOW_SPEC_SCHEMA_ID_V1: [u8; 32] = [
    0x15, 0x8b, 0x5d, 0xd0, 0xe9, 0xf4, 0x15, 0xda, 0x4e, 0xea, 0x16, 0xe9, 0x9f, 0x17, 0x60, 0xdf,
    0xe5, 0xfd, 0x26, 0x2c, 0x7c, 0xcc, 0xaa, 0xa8, 0xf2, 0x81, 0x94, 0xe3, 0x62, 0xc3, 0x54, 0xd7,
];
/// Canonical finalized-record schema for [`StatisticSpecV1`].
pub const STATISTIC_SPEC_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/statistic-spec-v1";
/// SHA-256 of [`STATISTIC_SPEC_SCHEMA_PREIMAGE_V1`].
pub const STATISTIC_SPEC_SCHEMA_ID_V1: [u8; 32] = [
    0x6f, 0xea, 0x99, 0x36, 0xc3, 0xf5, 0xf9, 0x05, 0x8d, 0xde, 0x83, 0x84, 0xb7, 0xe4, 0x50, 0xbf,
    0x48, 0x2b, 0x13, 0xd9, 0x5e, 0xe9, 0xe4, 0x38, 0xa9, 0x41, 0x04, 0xfd, 0x33, 0x1e, 0x3e, 0x51,
];

/// Pure current-ABI obligation handed to the Pyth SVM adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PythProviderAdapterObligationV2 {
    source_spec_id: ContentId,
    source: SourceSpecV1,
    provider_release_id: ContentId,
    provider_release: ProviderReleaseV1,
    adapter_config: PythAdapterConfigV1,
    window: WindowSpecV1,
    statistic: StatisticSpecV1,
}

impl PythProviderAdapterObligationV2 {
    /// Join independently authenticated Runtime V2 Source records.
    ///
    /// Every `*_id` is the authenticated content digest of the adjacent value.
    /// The Product record digest is authenticated separately because
    /// [`SourceMaterialV3`] intentionally owns only that graph root.
    #[allow(clippy::too_many_arguments)]
    pub fn from_authenticated_records(
        material: SourceMaterialV3,
        authenticated_product_record_digest: ContentId,
        source_spec_id: ContentId,
        source: SourceSpecV1,
        provider_release_id: ContentId,
        provider_release: ProviderReleaseV1,
        adapter_config_id: ContentId,
        adapter_config: PythAdapterConfigV1,
        window_spec_id: ContentId,
        window: WindowSpecV1,
        statistic_spec_id: ContentId,
        statistic: StatisticSpecV1,
        failure_policy_release: ContentId,
    ) -> Result<Self> {
        Self::from_authenticated_records_for_profile(
            material,
            authenticated_product_record_digest,
            source_spec_id,
            source,
            provider_release_id,
            provider_release,
            adapter_config_id,
            adapter_config,
            window_spec_id,
            window,
            statistic_spec_id,
            statistic,
            failure_policy_release,
            SourceAccessProfile::PythTerminalOneTransaction,
        )
    }

    /// Join the same terminal-sample graph for the distinct sponsored-push
    /// snapshot transport. This entry point cannot admit a Receiver/PostUpdate
    /// source profile, and the existing entry point cannot admit this one.
    #[allow(clippy::too_many_arguments)]
    pub fn from_authenticated_sponsored_push_records(
        material: SourceMaterialV3,
        authenticated_product_record_digest: ContentId,
        source_spec_id: ContentId,
        source: SourceSpecV1,
        provider_release_id: ContentId,
        provider_release: ProviderReleaseV1,
        adapter_config_id: ContentId,
        adapter_config: PythAdapterConfigV1,
        window_spec_id: ContentId,
        window: WindowSpecV1,
        statistic_spec_id: ContentId,
        statistic: StatisticSpecV1,
        failure_policy_release: ContentId,
    ) -> Result<Self> {
        Self::from_authenticated_records_for_profile(
            material,
            authenticated_product_record_digest,
            source_spec_id,
            source,
            provider_release_id,
            provider_release,
            adapter_config_id,
            adapter_config,
            window_spec_id,
            window,
            statistic_spec_id,
            statistic,
            failure_policy_release,
            SourceAccessProfile::PythSponsoredPushSnapshot,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_authenticated_records_for_profile(
        material: SourceMaterialV3,
        authenticated_product_record_digest: ContentId,
        source_spec_id: ContentId,
        source: SourceSpecV1,
        provider_release_id: ContentId,
        provider_release: ProviderReleaseV1,
        adapter_config_id: ContentId,
        adapter_config: PythAdapterConfigV1,
        window_spec_id: ContentId,
        window: WindowSpecV1,
        statistic_spec_id: ContentId,
        statistic: StatisticSpecV1,
        failure_policy_release: ContentId,
        expected_profile: SourceAccessProfile,
    ) -> Result<Self> {
        material.authenticate_product_record(authenticated_product_record_digest)?;
        material.validate_source_graph(
            source_spec_id,
            source,
            window_spec_id,
            window,
            statistic_spec_id,
            statistic,
            material.recovery_policy(),
            failure_policy_release,
        )?;
        Self::join_authenticated_source(
            source_spec_id,
            source,
            provider_release_id,
            provider_release,
            adapter_config_id,
            adapter_config,
            window,
            statistic,
            expected_profile,
        )
    }

    /// Join the same terminal-sample graph for one funded recovery rung.
    ///
    /// Every check this makes about the MARKET is the check
    /// [`Self::from_authenticated_records`] makes. What differs is which source
    /// the market is asking: the attempt names it, so the material's
    /// `primary_source_spec` edge is replaced by the rung's own pair of
    /// identities, and the window that decides admissibility stays the market's
    /// because the question and its period did not change.
    ///
    /// The access profile is the terminal one-transaction Pyth profile, the
    /// same one the primary route admits. An alternative source is an
    /// alternative FEED, not an alternative transport; a rung that wanted a
    /// different transport would be a different route with a different frame.
    #[allow(clippy::too_many_arguments)]
    pub fn from_authenticated_recovery_records(
        material: SourceMaterialV3,
        authenticated_product_record_digest: ContentId,
        attempt: RecoveryAttemptV2,
        source_spec_id: ContentId,
        source: SourceSpecV1,
        provider_release_id: ContentId,
        provider_release: ProviderReleaseV1,
        adapter_config_id: ContentId,
        adapter_config: PythAdapterConfigV1,
        window_spec_id: ContentId,
        window: WindowSpecV1,
        statistic_spec_id: ContentId,
        statistic: StatisticSpecV1,
        recovery_policy_id: ContentId,
        policy: RecoveryPolicyV2,
        failure_policy_release: ContentId,
    ) -> Result<Self> {
        material.authenticate_product_record(authenticated_product_record_digest)?;
        material.validate_recovery_source_graph(
            attempt,
            source_spec_id,
            source,
            window_spec_id,
            statistic_spec_id,
            statistic,
            recovery_policy_id,
            policy,
            failure_policy_release,
        )?;
        Self::join_authenticated_source(
            source_spec_id,
            source,
            provider_release_id,
            provider_release,
            adapter_config_id,
            adapter_config,
            window,
            statistic,
            SourceAccessProfile::PythTerminalOneTransaction,
        )
    }

    /// The half of the join that is about the SOURCE rather than the market.
    ///
    /// Both entry points above reach it with the same obligation: whichever
    /// source they authenticated must declare this provider release and this
    /// adapter configuration, must be reachable through the expected transport,
    /// and must produce the unit the market's statistic reads. Keeping it in
    /// one place is why a recovery rung cannot accidentally be admitted under a
    /// weaker rule than the primary.
    #[allow(clippy::too_many_arguments)]
    fn join_authenticated_source(
        source_spec_id: ContentId,
        source: SourceSpecV1,
        provider_release_id: ContentId,
        provider_release: ProviderReleaseV1,
        adapter_config_id: ContentId,
        adapter_config: PythAdapterConfigV1,
        window: WindowSpecV1,
        statistic: StatisticSpecV1,
        expected_profile: SourceAccessProfile,
    ) -> Result<Self> {
        source.validate_dependencies(provider_release_id, source.capacity_profile_id())?;
        if source.provider_release_id() != provider_release_id
            || source.adapter_config_id() != adapter_config_id
            || source.access_profile() != expected_profile
            || statistic.source_unit_id() != source.unit_id()
            || statistic.kind() != StatisticKind::TerminalSample
            || statistic.required_samples() != 1
            || window.kind() != WindowKind::Terminal
        {
            return Err(Error::LinkageMismatch);
        }
        Ok(Self {
            source_spec_id,
            source,
            provider_release_id,
            provider_release,
            adapter_config,
            window,
            statistic,
        })
    }

    /// Normalize exact Pyth facts only after the SVM adapter authenticated the
    /// selected provider release, real Receiver update, write authority, and
    /// current Clock. Both time bounds are enforced here so no caller may
    /// supply an already-normalized observation as authority.
    ///
    /// The two bounds answer different questions and carry different refusals.
    /// The schedule bound says whether this publication is *about the period
    /// the market sold*, and failing it is `InvalidObservationSchedule`. The
    /// `max_age`/`max_future_skew` band around this cluster's clock says
    /// whether the publication is one this cluster will still act on, and
    /// failing that is `InvalidPublicationTime`. A fresh publication about the
    /// wrong period and a stale publication about the right one must both
    /// refuse, and an operator reading the log should be able to tell which
    /// happened.
    ///
    /// The schedule bound is [`WindowSpecV1::contains_observation`] — the same
    /// call [`NormalizedProviderEvidenceV1::validate`] makes, so the window
    /// admission has one author and no route carries its own copy. On this
    /// obligation that predicate is exactly the closed `[window.start,
    /// window.end]`, and cannot be anything else: the join above refuses a
    /// window whose kind is not [`WindowKind::Terminal`], and
    /// [`WindowSpecV1::tolerating_cadence`] — the sole mutator of
    /// `cadence_tolerance_seconds`, which every constructor and
    /// [`WindowSpecV1::decode`] pass through — refuses a nonzero tolerance on a
    /// terminal window. The single-snapshot Pyth routes therefore read the
    /// widened predicate at a tolerance that is structurally zero.
    ///
    /// A publication after `window.end` is the *late* case a real provider
    /// cadence produces when nobody submitted in time. It refuses here rather
    /// than resolving the market on a price from after the question closed; the
    /// market's remaining route is the funded failure walk at
    /// `window.end + max_age`.
    #[allow(clippy::too_many_arguments)]
    pub fn normalize_authenticated_update(
        self,
        provider_evidence_id: ContentId,
        provider_feed_id: [u8; 32],
        price: i64,
        confidence: u64,
        exponent: i32,
        publication_unix_seconds: i64,
        current_unix_seconds: i64,
    ) -> Result<NormalizedProviderEvidenceV1> {
        if current_unix_seconds <= 0 {
            return Err(Error::InvalidPublicationTime);
        }
        if !self.window.contains_observation(publication_unix_seconds)? {
            return Err(Error::InvalidObservationSchedule);
        }
        let oldest = current_unix_seconds
            .checked_sub(i64::from(self.window.max_age_seconds()))
            .ok_or(Error::ArithmeticOverflow)?;
        let newest = current_unix_seconds
            .checked_add(i64::from(self.window.max_future_skew_seconds()))
            .ok_or(Error::ArithmeticOverflow)?;
        if publication_unix_seconds < oldest || publication_unix_seconds > newest {
            return Err(Error::InvalidPublicationTime);
        }
        self.finish_normalization(
            provider_evidence_id,
            provider_feed_id,
            price,
            confidence,
            exponent,
            publication_unix_seconds,
        )
    }

    /// Normalize the same Pyth facts for a market standing on a funded recovery
    /// rung.
    ///
    /// The schedule bound is IDENTICAL and is the whole point: an alternative
    /// source answers the same question about the same period, so the
    /// publication still has to fall inside the market's own `[window.start,
    /// window.end]`, and a rung that could answer about a different period would
    /// be a different market.
    ///
    /// What is not identical is the age floor, and it is dropped rather than
    /// widened. `now - max_age` is the primary leg's liveness grace, and the
    /// ladder only ever stands on a rung BECAUSE that grace expired: the crank
    /// that advanced the market is admissible exactly one second after
    /// `window.end + max_age`. Re-applying the floor here would make every rung
    /// structurally unanswerable -- a capture route whose success is not
    /// reachable -- so the honest reading is that it has already done its work
    /// and a later, explicit bound replaces it.
    ///
    /// That replacement is the attempt's own committed deadline, and it is not
    /// enforced here because it is not this record's to enforce.
    /// [`super::SourceResolutionStateV2::resolve_recovery_from_authenticated_domain`]
    /// holds the policy and refuses a capture one second past
    /// `attempt.deadline_unix_seconds()`, which is a per-rung bound the market
    /// committed to at founding and PREPAID. So the leg is still bounded, still
    /// by the market's own record, and by a number a founder chose rather than
    /// by a grace a route inherited.
    ///
    /// The future-skew ceiling stays. Nothing about advancing a ladder makes a
    /// publication from the future admissible.
    #[allow(clippy::too_many_arguments)]
    pub fn normalize_authenticated_recovery_update(
        self,
        provider_evidence_id: ContentId,
        provider_feed_id: [u8; 32],
        price: i64,
        confidence: u64,
        exponent: i32,
        publication_unix_seconds: i64,
        current_unix_seconds: i64,
    ) -> Result<NormalizedProviderEvidenceV1> {
        if current_unix_seconds <= 0 {
            return Err(Error::InvalidPublicationTime);
        }
        if !self.window.contains_observation(publication_unix_seconds)? {
            return Err(Error::InvalidObservationSchedule);
        }
        let newest = current_unix_seconds
            .checked_add(i64::from(self.window.max_future_skew_seconds()))
            .ok_or(Error::ArithmeticOverflow)?;
        if publication_unix_seconds > newest {
            return Err(Error::InvalidPublicationTime);
        }
        self.finish_normalization(
            provider_evidence_id,
            provider_feed_id,
            price,
            confidence,
            exponent,
            publication_unix_seconds,
        )
    }

    /// The half of normalization that is about the READING rather than the
    /// clock: the feed identity, the exponent, and the confidence this market's
    /// adapter configuration admits, in the unit its statistic declares.
    ///
    /// Both legs reach it with the same obligation, which is why a rung cannot
    /// be admitted under a looser reading rule than the primary -- only under a
    /// different clock rule, stated above.
    #[allow(clippy::too_many_arguments)]
    fn finish_normalization(
        self,
        provider_evidence_id: ContentId,
        provider_feed_id: [u8; 32],
        price: i64,
        confidence: u64,
        exponent: i32,
        publication_unix_seconds: i64,
    ) -> Result<NormalizedProviderEvidenceV1> {
        let atoms = self.adapter_config.validate_update(
            provider_feed_id,
            price,
            confidence,
            exponent,
            self.statistic,
        )?;
        Ok(NormalizedProviderEvidenceV1::new(
            self.source_spec_id,
            self.provider_release_id,
            provider_evidence_id,
            self.provider_release.adapter_release_id(),
            self.window.schedule_id(),
            0,
            publication_unix_seconds,
            publication_unix_seconds,
            atoms,
        ))
    }

    /// Source semantic domain that must equal Product's coordinate domain.
    pub const fn source_domain_id(self) -> ContentId {
        self.source.domain_id()
    }

    /// Statistic result unit that must equal Product's result unit.
    pub const fn result_unit_id(self) -> ContentId {
        self.statistic.result_unit_id()
    }

    /// Declared source-to-result decimal shift for this market's observations.
    ///
    /// Routes read the factor from here rather than from the adapter config,
    /// so the number that reaches the selector is the one the market's own
    /// record declares. The adapter's exponent is a *check* against it inside
    /// [`Self::normalize_authenticated_update`], never the source of it.
    pub const fn source_scale_exponent(self) -> i32 {
        self.statistic.source_scale_exponent()
    }

    /// Exact selected provider deployment-release content identity.
    pub const fn provider_deployment_release_id(self) -> ContentId {
        self.provider_release.provider_deployment_release_id()
    }

    /// Exact adapter release selected by Source.
    pub const fn adapter_release_id(self) -> ContentId {
        self.provider_release.adapter_release_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapacityEnvelope, RoundingBoundary, SOURCE_FAILURE_POLICY_RELEASE_ID_V2,
        SourceCapacityProfileV1, StatisticKind, WINDOW_SPEC_CADENCE_TOLERANCE_OFFSET_V1,
    };

    fn id(tag: u8) -> ContentId {
        let mut bytes = [0_u8; 32];
        bytes[0] = tag;
        ContentId::new(bytes).expect("nonzero content ID")
    }

    struct Fixture {
        material: SourceMaterialV3,
        product: ContentId,
        source_id: ContentId,
        source: SourceSpecV1,
        provider_id: ContentId,
        provider: ProviderReleaseV1,
        adapter_id: ContentId,
        adapter: PythAdapterConfigV1,
        window_id: ContentId,
        window: WindowSpecV1,
        statistic_id: ContentId,
        statistic: StatisticSpecV1,
        failure: ContentId,
    }

    fn capacity_profile() -> SourceCapacityProfileV1 {
        SourceCapacityProfileV1::new(CapacityEnvelope::Measured, 1, 0, id(15), id(16), 208, 0)
            .expect("capacity")
    }

    fn fixture() -> Fixture {
        let product = id(1);
        let source_id = id(2);
        let provider_id = id(3);
        let adapter_id = id(4);
        let capacity_id = id(5);
        let window_id = id(6);
        let statistic_id = id(7);
        let failure =
            ContentId::new(SOURCE_FAILURE_POLICY_RELEASE_ID_V2).expect("nonzero failure release");
        let source = SourceSpecV1::new(
            id(8),
            id(9),
            provider_id,
            SourceAccessProfile::PythTerminalOneTransaction,
            adapter_id,
            capacity_id,
        );
        let provider = ProviderReleaseV1::new(id(10), id(11), id(12), id(13), id(19));
        let adapter = PythAdapterConfigV1::new([42; 32], -8, 100).expect("canonical Pyth adapter");
        // A window with real width, because a terminal market sells a period.
        // The fixture used to be `(100, 100)`, which is the shape that made
        // every check below pass while nothing on a real provider cadence
        // could ever satisfy them.
        let window = WindowSpecV1::new(source_id, WindowKind::Terminal, 100, 400, 10, 2, id(14))
            .expect("terminal window");
        let capacity = capacity_profile();
        // Two different unit identities, so the statistic declares a
        // conversion and the only shift the Pyth adapter admits is the feed's
        // own exponent. This fixture used to declare the conversion and leave
        // the number at the identity, which is precisely the shape that paid
        // cohort-14 market B the wrong cell.
        let statistic = StatisticSpecV1::new(
            source.unit_id(),
            id(17),
            -8,
            StatisticKind::TerminalSample,
            RoundingBoundary::ExactRational,
            1,
            0,
            capacity_id,
            id(18),
            capacity,
        )
        .expect("terminal statistic");
        let material = SourceMaterialV3::explicitly_unbounded(
            product,
            source_id,
            window_id,
            statistic_id,
            None,
            failure,
        );
        Fixture {
            material,
            product,
            source_id,
            source,
            provider_id,
            provider,
            adapter_id,
            adapter,
            window_id,
            window,
            statistic_id,
            statistic,
            failure,
        }
    }

    fn obligation(fixture: &Fixture) -> Result<PythProviderAdapterObligationV2> {
        PythProviderAdapterObligationV2::from_authenticated_records(
            fixture.material,
            fixture.product,
            fixture.source_id,
            fixture.source,
            fixture.provider_id,
            fixture.provider,
            fixture.adapter_id,
            fixture.adapter,
            fixture.window_id,
            fixture.window,
            fixture.statistic_id,
            fixture.statistic,
            fixture.failure,
        )
    }

    fn sponsored_obligation(fixture: &Fixture) -> Result<PythProviderAdapterObligationV2> {
        PythProviderAdapterObligationV2::from_authenticated_sponsored_push_records(
            fixture.material,
            fixture.product,
            fixture.source_id,
            fixture.source,
            fixture.provider_id,
            fixture.provider,
            fixture.adapter_id,
            fixture.adapter,
            fixture.window_id,
            fixture.window,
            fixture.statistic_id,
            fixture.statistic,
            fixture.failure,
        )
    }

    fn with_profile(base: &Fixture, profile: SourceAccessProfile) -> Fixture {
        let mut next = fixture();
        next.source = SourceSpecV1::new(
            base.source.domain_id(),
            base.source.unit_id(),
            base.provider_id,
            profile,
            base.adapter_id,
            base.source.capacity_profile_id(),
        );
        next
    }

    /// Two Pyth profiles reach the same provider through the same receiver, and
    /// exactly one comparison keeps them from consuming each other's Sources.
    ///
    /// `SourceAccessProfile` is the tree's provider-extension discriminator —
    /// the only one — and the two constructors above pin their own variant into
    /// `from_authenticated_records_for_profile`, which refuses any Source that
    /// declares the other. That comparison had NO test: neither this module nor
    /// any campaign ever handed one profile's Source to the other's route, so
    /// the seam that makes provider breadth safe was carried entirely by
    /// inspection.
    ///
    /// It matters more the more profiles exist, not less. The direct route and
    /// the sponsored-push route differ in when a candidate may be consumed
    /// relative to the primary deadline; a Source admitted by the wrong one is
    /// a market resolved on evidence it did not buy.
    #[test]
    fn neither_pyth_profile_will_consume_the_other_profiles_source() {
        let base = fixture();
        assert!(
            obligation(&base).is_ok() && sponsored_obligation(&base).is_err(),
            "the fixture's Source declares the direct profile, so only that route joins it"
        );

        let sponsored = with_profile(&base, SourceAccessProfile::PythSponsoredPushSnapshot);
        assert_eq!(
            obligation(&sponsored),
            Err(Error::LinkageMismatch),
            "the direct route refuses a sponsored-push Source"
        );
        assert!(
            sponsored_obligation(&sponsored).is_ok(),
            "and the sponsored-push route accepts exactly that Source"
        );

        assert_eq!(
            sponsored_obligation(&base),
            Err(Error::LinkageMismatch),
            "the sponsored-push route refuses a direct Source"
        );

        // The two families this contract can express at all. Neither reaches a
        // Pyth obligation, and the refusal is the same named one, so a Source
        // that named a foreign family could not be laundered through either
        // Pyth route by an operator who simply presented it.
        for foreign in [
            SourceAccessProfile::RelayedObservationRecord,
            SourceAccessProfile::SharedObservationChild,
        ] {
            let other = with_profile(&base, foreign);
            assert_eq!(obligation(&other), Err(Error::LinkageMismatch));
            assert_eq!(sponsored_obligation(&other), Err(Error::LinkageMismatch));
        }
    }

    #[test]
    fn exact_runtime_graph_normalizes_real_adapter_facts() {
        let fixture = fixture();
        let evidence = obligation(&fixture)
            .expect("joined records")
            .normalize_authenticated_update(id(30), [42; 32], 1_000_000, 5_000, -8, 250, 255)
            .expect("authenticated update");
        assert_eq!(evidence.source_spec_id(), fixture.source_id);
        assert_eq!(evidence.provider_release_id(), fixture.provider_id);
        assert_eq!(
            evidence.adapter_release_id(),
            fixture.provider.adapter_release_id(),
            "the authenticated ProviderRelease selects the adapter"
        );
        assert_eq!(evidence.atoms(), 1_000_000);
    }

    #[test]
    fn parallel_provider_and_product_truth_refuse() {
        let mut altered = fixture();
        altered.material = SourceMaterialV3::explicitly_unbounded(
            id(99),
            altered.source_id,
            altered.window_id,
            altered.statistic_id,
            None,
            altered.failure,
        );
        assert_eq!(obligation(&altered), Err(Error::LinkageMismatch));

        let fixture = fixture();
        assert_eq!(
            PythProviderAdapterObligationV2::from_authenticated_records(
                fixture.material,
                fixture.product,
                fixture.source_id,
                fixture.source,
                id(98),
                fixture.provider,
                fixture.adapter_id,
                fixture.adapter,
                fixture.window_id,
                fixture.window,
                fixture.statistic_id,
                fixture.statistic,
                fixture.failure,
            ),
            Err(Error::LinkageMismatch)
        );
    }

    #[test]
    fn hostile_feed_confidence_and_exponent_refuse() {
        let fixture = fixture();
        let obligation = obligation(&fixture).expect("joined records");
        for result in [
            obligation.normalize_authenticated_update(
                id(30),
                [41; 32],
                1_000_000,
                5_000,
                -8,
                250,
                255,
            ),
            obligation.normalize_authenticated_update(
                id(30),
                [42; 32],
                1_000_000,
                20_000,
                -8,
                250,
                255,
            ),
            obligation.normalize_authenticated_update(
                id(30),
                [42; 32],
                1_000_000,
                5_000,
                -7,
                250,
                255,
            ),
        ] {
            assert!(result.is_err());
        }
    }

    /// The window is a closed period and both of its edges are reachable.
    ///
    /// Under the old one-instant terminal window this test could not exist:
    /// there was one admissible second and it was also both edges, so nothing
    /// distinguished "inside" from "on the boundary" from "unreachable".
    #[test]
    fn both_edges_of_the_terminal_window_admit_an_observation() {
        let fixture = fixture();
        let obligation = obligation(&fixture).expect("joined records");
        for publication in [100, 250, 400] {
            assert!(
                obligation
                    .normalize_authenticated_update(
                        id(30),
                        [42; 32],
                        1_000_000,
                        5_000,
                        -8,
                        publication,
                        publication + 5,
                    )
                    .is_ok(),
                "the window sells [100, 400] and {publication} is in it"
            );
        }
    }

    /// The two time bounds are different questions and say so.
    ///
    /// A publication one second outside either edge is about the wrong period,
    /// and a publication squarely inside the window that this cluster sat on
    /// for longer than `max_age_seconds` is about the right one. Both refuse,
    /// with different refusals, and each is fresh/in-window on the other axis
    /// so exactly one bound can be responsible.
    #[test]
    fn the_window_and_the_freshness_clock_refuse_separately() {
        let fixture = fixture();
        let obligation = obligation(&fixture).expect("joined records");
        // One second before the market started selling.
        assert_eq!(
            obligation.normalize_authenticated_update(
                id(30),
                [42; 32],
                1_000_000,
                5_000,
                -8,
                99,
                105
            ),
            Err(Error::InvalidObservationSchedule)
        );
        // One second after it closed: the late observation a provider cadence
        // straddling the deadline produces. Resolving on this would answer the
        // market with a price from after the question closed.
        assert_eq!(
            obligation.normalize_authenticated_update(
                id(30),
                [42; 32],
                1_000_000,
                5_000,
                -8,
                401,
                405
            ),
            Err(Error::InvalidObservationSchedule)
        );
        // The right period, delivered too late to be acted on.
        assert_eq!(
            obligation.normalize_authenticated_update(
                id(30),
                [42; 32],
                1_000_000,
                5_000,
                -8,
                250,
                300
            ),
            Err(Error::InvalidPublicationTime)
        );
        // The right period, from further in this cluster's future than the
        // window admits skew for.
        assert_eq!(
            obligation.normalize_authenticated_update(
                id(30),
                [42; 32],
                1_000_000,
                5_000,
                -8,
                250,
                247
            ),
            Err(Error::InvalidPublicationTime)
        );
    }

    /// The window admission has one author, and the tolerance cannot reach it.
    ///
    /// `normalize_authenticated_update` reads
    /// [`WindowSpecV1::contains_observation`] rather than re-spelling the
    /// closed interval, so `cadence_tolerance_seconds` is stated once and read
    /// everywhere. On the two single-snapshot Pyth routes that widening is the
    /// identity, and this pins the three independent gates that make it so.
    /// "The two spellings agree" is a fact about the records founded so far;
    /// "the tolerance cannot be raised here" is a fact about the type, and it
    /// is the one worth holding, because the first stops being true the day
    /// someone founds a different window.
    #[test]
    fn a_terminal_window_cannot_reach_the_single_snapshot_route_with_a_tolerance() {
        let terminal = fixture();

        // Gate one: the sole mutator of the tolerance refuses to put one on a
        // terminal window, so no terminal `WindowSpecV1` in memory carries one.
        assert_eq!(terminal.window.kind(), WindowKind::Terminal);
        assert_eq!(terminal.window.cadence_tolerance_seconds(), 0);
        assert_eq!(
            terminal.window.tolerating_cadence(120),
            Err(Error::InvalidWindow)
        );

        // Gate two: `decode` routes through that mutator, so no finalized
        // window record can carry one either -- and `decode` is the exact call
        // the relayed and sponsored routes make on the account bytes.
        let mut hostile = terminal.window.to_bytes();
        crate::put(
            &mut hostile,
            WINDOW_SPEC_CADENCE_TOLERANCE_OFFSET_V1,
            &120_u32.to_le_bytes(),
        );
        assert_eq!(WindowSpecV1::decode(&hostile), Err(Error::InvalidWindow));

        // Gate three: the windows that CAN carry a tolerance are the scheduled
        // ones, and neither single-snapshot join will hold one.
        let scheduled = WindowSpecV1::new(
            terminal.source_id,
            WindowKind::ScheduledInterval,
            100,
            400,
            10,
            2,
            id(14),
        )
        .expect("scheduled window")
        .tolerating_cadence(120)
        .expect("a scheduled window may tolerate a cadence");
        let mut widened = fixture();
        widened.window = scheduled;
        assert_eq!(obligation(&widened), Err(Error::LinkageMismatch));
        assert_eq!(sponsored_obligation(&widened), Err(Error::LinkageMismatch));

        // And the widening is real wherever it is reachable: the band admits
        // `end + tolerance` and refuses one second past it. So the predicate
        // the single-snapshot routes now call is genuinely wider than the
        // interval they used to spell, and it is held at the identity by the
        // three gates above rather than by the predicate.
        assert_eq!(scheduled.contains_observation(400 + 120), Ok(true));
        assert_eq!(scheduled.contains_observation(400 + 121), Ok(false));
        assert_eq!(scheduled.contains_observation(100 - 120), Ok(true));
        assert_eq!(scheduled.contains_observation(100 - 121), Ok(false));
    }

    /// Cohort-13's own window, admitted and refused exactly as before.
    ///
    /// Read off `window_spec_record` `4CM5e6Eq…`: terminal,
    /// `[1788369759, 1788371559]`, `max_age` 7,200, skew 1, tolerance 0. At
    /// that tolerance the widened predicate and the closed interval are the
    /// same set, and this is the control for the change: both edges admit, one
    /// second outside either edge is `InvalidObservationSchedule`, and the
    /// publication the market actually had -- 1788372175, 616 seconds past the
    /// close -- refuses on the schedule bound and not on the freshness one.
    #[test]
    fn the_cohort_thirteen_window_admits_and_refuses_exactly_its_closed_interval() {
        const START: i64 = 1_788_369_759;
        const END: i64 = 1_788_371_559;
        const OBSERVED: i64 = 1_788_372_175;

        let base = fixture();
        let window = WindowSpecV1::new(
            base.source_id,
            WindowKind::Terminal,
            START,
            END,
            7_200,
            1,
            id(14),
        )
        .expect("cohort-13 terminal window");
        assert_eq!(window.cadence_tolerance_seconds(), 0);

        let mut relayed = fixture();
        relayed.window = window;
        let mut sponsored = with_profile(&base, SourceAccessProfile::PythSponsoredPushSnapshot);
        sponsored.window = window;

        // Both single-snapshot routes, because the widening was inert on both.
        for obligation in [
            obligation(&relayed).expect("relayed records"),
            sponsored_obligation(&sponsored).expect("sponsored records"),
        ] {
            let normalize = |publication: i64, clock: i64| {
                obligation.normalize_authenticated_update(
                    id(30),
                    [42; 32],
                    1_000_000,
                    5_000,
                    -8,
                    publication,
                    clock,
                )
            };

            for publication in [START, START + 1, END - 1, END] {
                assert!(
                    normalize(publication, END + 1).is_ok(),
                    "the market sold [{START}, {END}] and {publication} is in it"
                );
            }
            assert_eq!(
                normalize(START - 1, END + 1),
                Err(Error::InvalidObservationSchedule)
            );
            assert_eq!(
                normalize(END + 1, END + 2),
                Err(Error::InvalidObservationSchedule)
            );
            // The real one. Fresh by every clock this cluster runs, and about
            // the wrong period, which is the distinction the two refusals
            // carry.
            assert_eq!(
                normalize(OBSERVED, OBSERVED),
                Err(Error::InvalidObservationSchedule)
            );
        }
    }

    /// A statistic whose declared factor is not one this adapter admits is a
    /// disagreement between the market's own two records, and it refuses by
    /// its own name rather than as a bad observation.
    #[test]
    fn a_mismatched_declared_factor_refuses_by_name() {
        let base = fixture();
        let restate = |source_unit: ContentId, result_unit: ContentId, scale: i32| {
            let mut next = fixture();
            next.statistic = StatisticSpecV1::new(
                source_unit,
                result_unit,
                scale,
                StatisticKind::TerminalSample,
                RoundingBoundary::ExactRational,
                1,
                0,
                next.statistic.capacity_profile_id(),
                next.statistic.evaluator_release_id(),
                capacity_profile(),
            )
            .expect("statistic constructs");
            next
        };
        let normalize = |fixture: &Fixture| {
            obligation(fixture)
                .expect("joined records")
                .normalize_authenticated_update(id(30), [42; 32], 1_000_000, 5_000, -8, 250, 255)
                .map(|evidence| evidence.atoms())
        };

        // The shape the fixture ships and the founding should write: two unit
        // identities and the feed's own exponent between them.
        assert_eq!(normalize(&base), Ok(1_000_000));

        // Cohort-14 market B's founding, exactly: a declared conversion with
        // the number left at the identity. It reached the selector before;
        // now it never leaves the adapter.
        assert_eq!(
            normalize(&restate(base.source.unit_id(), id(17), 0)),
            Err(Error::SourceScaleMismatch)
        );

        // A conversion declared at some other decade is equally refused. The
        // adapter admits exactly one number, not merely a nonzero one.
        assert_eq!(
            normalize(&restate(base.source.unit_id(), id(17), -6)),
            Err(Error::SourceScaleMismatch)
        );

        // The other convention: one identity on both sides declares no
        // conversion, so cuts sit on the feed's atom scale and zero is the
        // only shift admitted -- even though the feed still publishes -8.
        assert_eq!(
            normalize(&restate(base.source.unit_id(), base.source.unit_id(), 0)),
            Ok(1_000_000)
        );

        // And a record cannot even be built claiming a conversion between one
        // unit and itself, so that half of the rule is refused before any
        // adapter is consulted.
        assert_eq!(
            StatisticSpecV1::new(
                id(9),
                id(9),
                -8,
                StatisticKind::TerminalSample,
                RoundingBoundary::ExactRational,
                1,
                0,
                base.statistic.capacity_profile_id(),
                base.statistic.evaluator_release_id(),
                capacity_profile(),
            ),
            Err(Error::NonCanonicalSourceScale)
        );

        // Nor one outside the shift range the emitted Lean bound admits.
        assert_eq!(
            StatisticSpecV1::new(
                id(9),
                id(17),
                dclutch_product_runtime_v2::MAX_SOURCE_SCALE_EXPONENT + 1,
                StatisticKind::TerminalSample,
                RoundingBoundary::ExactRational,
                1,
                0,
                base.statistic.capacity_profile_id(),
                base.statistic.evaluator_release_id(),
                capacity_profile(),
            ),
            Err(Error::NonCanonicalSourceScale)
        );
    }

    /// The four bytes the factor occupies were reserved and enforced zero, so
    /// every statistic written before it decodes at the identity and encodes
    /// back to the same 176 bytes it always had.
    #[test]
    fn a_pre_factor_statistic_is_byte_identical_at_the_identity() {
        let identity = StatisticSpecV1::new(
            id(9),
            id(17),
            0,
            StatisticKind::TerminalSample,
            RoundingBoundary::ExactRational,
            1,
            0,
            id(5),
            id(18),
            capacity_profile(),
        )
        .expect("statistic constructs");
        let bytes = identity.to_bytes();
        assert_eq!(bytes[12..16], [0, 0, 0, 0]);
        assert_eq!(StatisticSpecV1::decode(&bytes), Ok(identity));
        assert_eq!(identity.source_scale_exponent(), 0);

        let declared = StatisticSpecV1::new(
            id(9),
            id(17),
            -8,
            StatisticKind::TerminalSample,
            RoundingBoundary::ExactRational,
            1,
            0,
            id(5),
            id(18),
            capacity_profile(),
        )
        .expect("statistic constructs");
        let declared_bytes = declared.to_bytes();
        assert_eq!(declared_bytes[12..16], (-8_i32).to_le_bytes());
        assert_eq!(StatisticSpecV1::decode(&declared_bytes), Ok(declared));
        // Only those four bytes move.
        assert_eq!(declared_bytes[..12], bytes[..12]);
        assert_eq!(declared_bytes[16..], bytes[16..]);

        // A persisted shift outside the admitted range refuses on decode, not
        // only on construction.
        let mut hostile = bytes;
        hostile[12..16].copy_from_slice(&i32::MIN.to_le_bytes());
        assert_eq!(
            StatisticSpecV1::decode(&hostile),
            Err(Error::NonCanonicalSourceScale)
        );
    }
}
