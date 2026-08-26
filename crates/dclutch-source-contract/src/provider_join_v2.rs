//! Current runtime-width Source-to-Pyth adapter obligation.
//!
//! Product Runtime V2 deliberately moved Source policy into separately
//! finalized records. This obligation rejoins those exact records without
//! recreating the old embedded Source-material authority. It still does not
//! authenticate SVM owners, Loader state, provider messages, or signatures;
//! those remain obligations of the Pyth SVM adapter.

use super::{
    ContentId, Error, NormalizedProviderEvidenceV1, PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1,
    ProviderReleaseV1, PythAdapterConfigV1, Result, SourceAccessProfile, SourceMaterialV2,
    SourceSpecV1, StatisticKind, StatisticSpecV1, WindowKind, WindowSpecV1,
};

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
            || provider_release.adapter_release_id().to_bytes()
                != PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1
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
    /// current Clock. The publication window is enforced here so no caller may
    /// supply an already-normalized observation as authority.
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
        if current_unix_seconds <= 0
            || publication_unix_seconds < self.window.start_unix_seconds()
            || publication_unix_seconds > self.window.end_unix_seconds()
        {
            return Err(Error::InvalidPublicationTime);
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
        let provider = ProviderReleaseV1::new(
            id(10),
            ContentId::new(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1).expect("nonzero adapter release"),
            id(11),
            id(12),
            id(13),
        );
        let adapter = PythAdapterConfigV1::new([42; 32], -8, 100).expect("canonical Pyth adapter");
        let window = WindowSpecV1::new(source_id, WindowKind::Terminal, 100, 100, 10, 2, id(14))
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
            .normalize_authenticated_update(id(30), [42; 32], 1_000_000, 5_000, -8, 100, 101)
            .expect("authenticated update");
        assert_eq!(evidence.source_spec_id(), fixture.source_id);
        assert_eq!(evidence.provider_release_id(), fixture.provider_id);
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
    fn hostile_feed_confidence_exponent_and_time_refuse() {
        let fixture = fixture();
        let obligation = obligation(&fixture).expect("joined records");
        for result in [
            obligation.normalize_authenticated_update(
                id(30),
                [41; 32],
                1_000_000,
                5_000,
                -8,
                100,
                101,
            ),
            obligation.normalize_authenticated_update(
                id(30),
                [42; 32],
                1_000_000,
                20_000,
                -8,
                100,
                101,
            ),
            obligation.normalize_authenticated_update(
                id(30),
                [42; 32],
                1_000_000,
                5_000,
                -7,
                100,
                101,
            ),
            obligation.normalize_authenticated_update(
                id(30),
                [42; 32],
                1_000_000,
                5_000,
                -8,
                99,
                101,
            ),
        ] {
            assert!(result.is_err());
        }
    }
}
