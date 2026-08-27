//! Current runtime-width Source-to-Pyth adapter obligation.
//!
//! Product Runtime V2 deliberately moved Source policy into separately
//! finalized records. This obligation rejoins those exact records without
//! recreating the old embedded Source-material authority. It still does not
//! authenticate SVM owners, Loader state, provider messages, or signatures;
//! those remain obligations of the Pyth SVM adapter.

use super::{
    ContentId, Error, NormalizedProviderEvidenceV1, ProviderReleaseV1, PythAdapterConfigV1, Result,
    SourceAccessProfile, SourceMaterialV2, SourceSpecV1, StatisticKind, StatisticSpecV1,
    WindowKind, WindowSpecV1,
};

/// Canonical finalized-record schema for [`SourceSpecV1`].
pub const SOURCE_SPEC_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/source-spec-v1";
/// SHA-256 of [`SOURCE_SPEC_SCHEMA_PREIMAGE_V1`].
pub const SOURCE_SPEC_SCHEMA_ID_V1: [u8; 32] = [
    0xcc, 0xea, 0xf8, 0xdb, 0xc2, 0xac, 0x3a, 0xe8, 0x11, 0xb5, 0x22, 0x19, 0x72, 0x92, 0x9c, 0xf3,
    0xfc, 0x34, 0x13, 0x72, 0x1b, 0x08, 0x0d, 0x56, 0x0f, 0xf9, 0x54, 0xb8, 0xab, 0x81, 0x86, 0xb6,
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
    /// [`SourceMaterialV2`] intentionally owns only that graph root.
    #[allow(clippy::too_many_arguments)]
    pub fn from_authenticated_records(
        material: SourceMaterialV2,
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
        source.validate_dependencies(provider_release_id, source.capacity_profile_id())?;
        if source.provider_release_id() != provider_release_id
            || source.adapter_config_id() != adapter_config_id
            || source.access_profile() != SourceAccessProfile::PythTerminalOneTransaction
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
    /// The two bounds answer different questions and carry different refusals,
    /// matching `NormalizedProviderEvidenceV1::validate`. `[window.start,
    /// window.end]` says whether this publication is *about the period the
    /// market sold*, and failing it is `InvalidObservationSchedule`. The
    /// `max_age`/`max_future_skew` band around this cluster's clock says
    /// whether the publication is one this cluster will still act on, and
    /// failing that is `InvalidPublicationTime`. A fresh publication about the
    /// wrong period and a stale publication about the right one must both
    /// refuse, and an operator reading the log should be able to tell which
    /// happened.
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
        if publication_unix_seconds < self.window.start_unix_seconds()
            || publication_unix_seconds > self.window.end_unix_seconds()
        {
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
        let atoms =
            self.adapter_config
                .validate_update(provider_feed_id, price, confidence, exponent)?;
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
        SourceCapacityProfileV1, StatisticKind,
    };

    fn id(tag: u8) -> ContentId {
        let mut bytes = [0_u8; 32];
        bytes[0] = tag;
        ContentId::new(bytes).expect("nonzero content ID")
    }

    struct Fixture {
        material: SourceMaterialV2,
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
        let capacity =
            SourceCapacityProfileV1::new(CapacityEnvelope::Measured, 1, 0, id(15), id(16), 208, 0)
                .expect("capacity");
        let statistic = StatisticSpecV1::new(
            source.unit_id(),
            id(17),
            StatisticKind::TerminalSample,
            RoundingBoundary::ExactRational,
            1,
            0,
            capacity_id,
            id(18),
            capacity,
        )
        .expect("terminal statistic");
        let material =
            SourceMaterialV2::new(product, source_id, window_id, statistic_id, None, failure);
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
        altered.material = SourceMaterialV2::new(
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
}
