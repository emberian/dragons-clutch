//! Compact Source material rooted in Product Runtime V2.

use core::convert::TryInto;

use super::{
    ContentId, Error, Result, SourceSpecV1, StatisticSpecV1, WindowSpecV1,
    generated_source_material_v2::{
        SOURCE_MATERIAL_V2_BYTES, SOURCE_MATERIAL_V2_FAILURE_POLICY_RELEASE_OFFSET,
        SOURCE_MATERIAL_V2_MAGIC, SOURCE_MATERIAL_V2_MAGIC_OFFSET,
        SOURCE_MATERIAL_V2_PRIMARY_SOURCE_SPEC_OFFSET,
        SOURCE_MATERIAL_V2_PRODUCT_RECORD_DIGEST_OFFSET, SOURCE_MATERIAL_V2_RECOVERY_POLICY_OFFSET,
        SOURCE_MATERIAL_V2_RECOVERY_PRESENT_OFFSET, SOURCE_MATERIAL_V2_RESERVED_OFFSET,
        SOURCE_MATERIAL_V2_SCHEMA_VERSION, SOURCE_MATERIAL_V2_STATISTIC_SPEC_OFFSET,
        SOURCE_MATERIAL_V2_VERSION_OFFSET, SOURCE_MATERIAL_V2_WINDOW_SPEC_OFFSET,
    },
};

const ID_BYTES: usize = 32;
const RESERVED_BYTES: usize = 5;

/// Immutable compact Source policy rooted in one authenticated Product Runtime V2 record.
///
/// This record deliberately contains no Market, generation, stable Product ID,
/// result-domain digest, partition count, or partition cells. The Product
/// Runtime V2 reader derives those facts from `product_record_digest`; mutable
/// Source state later binds `{market, generation, material_digest}`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceMaterialV2 {
    product_record_digest: ContentId,
    primary_source_spec: ContentId,
    window_spec: ContentId,
    statistic_spec: ContentId,
    recovery_policy: Option<ContentId>,
    failure_policy_release: ContentId,
}

impl SourceMaterialV2 {
    /// Construct one compact Source graph root.
    #[must_use]
    pub const fn new(
        product_record_digest: ContentId,
        primary_source_spec: ContentId,
        window_spec: ContentId,
        statistic_spec: ContentId,
        recovery_policy: Option<ContentId>,
        failure_policy_release: ContentId,
    ) -> Self {
        Self {
            product_record_digest,
            primary_source_spec,
            window_spec,
            statistic_spec,
            recovery_policy,
            failure_policy_release,
        }
    }

    /// Hostile-decode one exact 208-byte V2 record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != SOURCE_MATERIAL_V2_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, SOURCE_MATERIAL_V2_MAGIC_OFFSET)? != SOURCE_MATERIAL_V2_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, SOURCE_MATERIAL_V2_VERSION_OFFSET)?)
            != SOURCE_MATERIAL_V2_SCHEMA_VERSION
        {
            return Err(Error::UnsupportedSchema);
        }
        if bytes
            .get(
                SOURCE_MATERIAL_V2_RESERVED_OFFSET
                    ..SOURCE_MATERIAL_V2_RESERVED_OFFSET + RESERVED_BYTES,
            )
            .ok_or(Error::InvalidLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalReservedBytes);
        }
        let recovery_bytes = array::<ID_BYTES>(bytes, SOURCE_MATERIAL_V2_RECOVERY_POLICY_OFFSET)?;
        let recovery_policy = match byte(bytes, SOURCE_MATERIAL_V2_RECOVERY_PRESENT_OFFSET)? {
            0 if recovery_bytes.iter().all(|byte| *byte == 0) => None,
            1 => Some(ContentId::new(recovery_bytes)?),
            _ => return Err(Error::NonCanonicalSourceMaterial),
        };
        Ok(Self::new(
            content(bytes, SOURCE_MATERIAL_V2_PRODUCT_RECORD_DIGEST_OFFSET)?,
            content(bytes, SOURCE_MATERIAL_V2_PRIMARY_SOURCE_SPEC_OFFSET)?,
            content(bytes, SOURCE_MATERIAL_V2_WINDOW_SPEC_OFFSET)?,
            content(bytes, SOURCE_MATERIAL_V2_STATISTIC_SPEC_OFFSET)?,
            recovery_policy,
            content(bytes, SOURCE_MATERIAL_V2_FAILURE_POLICY_RELEASE_OFFSET)?,
        ))
    }

    /// Encode the exact canonical V2 record.
    #[must_use]
    pub fn to_bytes(self) -> [u8; SOURCE_MATERIAL_V2_BYTES] {
        let mut output = [0_u8; SOURCE_MATERIAL_V2_BYTES];
        put(
            &mut output,
            SOURCE_MATERIAL_V2_MAGIC_OFFSET,
            &SOURCE_MATERIAL_V2_MAGIC,
        );
        put(
            &mut output,
            SOURCE_MATERIAL_V2_VERSION_OFFSET,
            &SOURCE_MATERIAL_V2_SCHEMA_VERSION.to_le_bytes(),
        );
        put(
            &mut output,
            SOURCE_MATERIAL_V2_PRODUCT_RECORD_DIGEST_OFFSET,
            self.product_record_digest.as_bytes(),
        );
        put(
            &mut output,
            SOURCE_MATERIAL_V2_PRIMARY_SOURCE_SPEC_OFFSET,
            self.primary_source_spec.as_bytes(),
        );
        put(
            &mut output,
            SOURCE_MATERIAL_V2_WINDOW_SPEC_OFFSET,
            self.window_spec.as_bytes(),
        );
        put(
            &mut output,
            SOURCE_MATERIAL_V2_STATISTIC_SPEC_OFFSET,
            self.statistic_spec.as_bytes(),
        );
        if let Some(recovery) = self.recovery_policy {
            output[SOURCE_MATERIAL_V2_RECOVERY_PRESENT_OFFSET] = 1;
            put(
                &mut output,
                SOURCE_MATERIAL_V2_RECOVERY_POLICY_OFFSET,
                recovery.as_bytes(),
            );
        }
        put(
            &mut output,
            SOURCE_MATERIAL_V2_FAILURE_POLICY_RELEASE_OFFSET,
            self.failure_policy_release.as_bytes(),
        );
        output
    }

    /// Require the exact Product Runtime V2 record selected by this Source material.
    pub fn authenticate_product_record(self, authenticated_digest: ContentId) -> Result<()> {
        if authenticated_digest == self.product_record_digest {
            Ok(())
        } else {
            Err(Error::LinkageMismatch)
        }
    }

    /// Validate the Source-owned record graph after the adapter authenticates
    /// each named finalized record by its content identity.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_source_graph(
        self,
        source_spec_id: ContentId,
        source: SourceSpecV1,
        window_spec_id: ContentId,
        window: WindowSpecV1,
        statistic_spec_id: ContentId,
        statistic: StatisticSpecV1,
        recovery_policy: Option<ContentId>,
        failure_policy_release: ContentId,
    ) -> Result<()> {
        if source_spec_id != self.primary_source_spec
            || window_spec_id != self.window_spec
            || statistic_spec_id != self.statistic_spec
            || recovery_policy != self.recovery_policy
            || failure_policy_release != self.failure_policy_release
            || statistic.source_unit_id() != source.unit_id()
        {
            return Err(Error::LinkageMismatch);
        }
        window.validate_source(source_spec_id)
    }

    /// Exact Product Runtime V2 record content digest.
    #[must_use]
    pub const fn product_record_digest(self) -> ContentId {
        self.product_record_digest
    }
    /// Primary SourceSpec content identity; that record owns provider/config/capacity facts.
    #[must_use]
    pub const fn primary_source_spec(self) -> ContentId {
        self.primary_source_spec
    }
    /// Window/freshness policy content identity.
    #[must_use]
    pub const fn window_spec(self) -> ContentId {
        self.window_spec
    }
    /// Statistic and rounding policy content identity.
    #[must_use]
    pub const fn statistic_spec(self) -> ContentId {
        self.statistic_spec
    }
    /// Optional ordered recovery-policy content identity.
    #[must_use]
    pub const fn recovery_policy(self) -> Option<ContentId> {
        self.recovery_policy
    }
    /// Release defining exhaustion-to-explicit-failure semantics.
    #[must_use]
    pub const fn failure_policy_release(self) -> ContentId {
        self.failure_policy_release
    }
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn content(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(array(bytes, offset)?)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(value.iter().copied()) {
        *destination = source;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_source_material_v2::{
        SOURCE_MATERIAL_V2_EXAMPLE, SOURCE_MATERIAL_V2_REFUSAL_CORPUS,
        SOURCE_MATERIAL_V2_REFUSAL_COUNT, SOURCE_MATERIAL_V2_ZERO_RECOVERY_EXAMPLE,
    };
    use crate::{
        CapacityEnvelope, RoundingBoundary, SourceAccessProfile, SourceCapacityProfileV1,
        StatisticKind,
    };

    fn id(tag: u8) -> ContentId {
        let mut bytes = [0_u8; 32];
        bytes[0] = tag;
        ContentId::new(bytes).expect("nonzero ID")
    }

    #[test]
    fn lean_generated_examples_agree() {
        let expected = SourceMaterialV2::new(id(1), id(2), id(3), id(4), Some(id(5)), id(6));
        assert_eq!(expected.to_bytes(), SOURCE_MATERIAL_V2_EXAMPLE);
        assert_eq!(
            SourceMaterialV2::decode(&SOURCE_MATERIAL_V2_EXAMPLE),
            Ok(expected)
        );
        let without_recovery = SourceMaterialV2::new(id(1), id(2), id(3), id(4), None, id(6));
        assert_eq!(
            without_recovery.to_bytes(),
            SOURCE_MATERIAL_V2_ZERO_RECOVERY_EXAMPLE
        );
        assert_eq!(
            SourceMaterialV2::decode(&SOURCE_MATERIAL_V2_ZERO_RECOVERY_EXAMPLE),
            Ok(without_recovery)
        );
    }

    #[test]
    fn lean_generated_refusal_corpus_fails_closed() {
        assert_eq!(
            SOURCE_MATERIAL_V2_REFUSAL_CORPUS.len(),
            SOURCE_MATERIAL_V2_REFUSAL_COUNT
        );
        for hostile in SOURCE_MATERIAL_V2_REFUSAL_CORPUS {
            assert!(SourceMaterialV2::decode(&hostile).is_err());
        }
    }

    #[test]
    fn exact_product_root_substitution_refuses() {
        let material = SourceMaterialV2::new(id(1), id(2), id(3), id(4), Some(id(5)), id(6));
        assert_eq!(material.authenticate_product_record(id(1)), Ok(()));
        assert_eq!(
            material.authenticate_product_record(id(7)),
            Err(Error::LinkageMismatch)
        );
    }

    #[test]
    fn source_graph_substitutions_refuse() {
        let capacity =
            SourceCapacityProfileV1::new(CapacityEnvelope::Measured, 1, 1, id(20), id(21), 208, 0)
                .expect("capacity");
        let source = SourceSpecV1::new(
            id(30),
            id(31),
            id(32),
            SourceAccessProfile::PythTerminalOneTransaction,
            id(33),
            id(34),
        );
        let window = WindowSpecV1::new(id(2), crate::WindowKind::Terminal, 3, 9, 1, 0, id(35))
            .expect("window");
        let statistic = StatisticSpecV1::new(
            id(31),
            id(36),
            StatisticKind::TerminalSample,
            RoundingBoundary::ExactRational,
            1,
            0,
            id(34),
            id(37),
            capacity,
        )
        .expect("statistic");
        let material = SourceMaterialV2::new(id(1), id(2), id(3), id(4), Some(id(5)), id(6));
        let valid = || {
            material.validate_source_graph(
                id(2),
                source,
                id(3),
                window,
                id(4),
                statistic,
                Some(id(5)),
                id(6),
            )
        };
        assert_eq!(valid(), Ok(()));
        assert_eq!(
            material.validate_source_graph(
                id(7),
                source,
                id(3),
                window,
                id(4),
                statistic,
                Some(id(5)),
                id(6)
            ),
            Err(Error::LinkageMismatch)
        );
        assert_eq!(
            material.validate_source_graph(
                id(2),
                source,
                id(7),
                window,
                id(4),
                statistic,
                Some(id(5)),
                id(6)
            ),
            Err(Error::LinkageMismatch)
        );
        assert_eq!(
            material.validate_source_graph(
                id(2),
                source,
                id(3),
                window,
                id(7),
                statistic,
                Some(id(5)),
                id(6)
            ),
            Err(Error::LinkageMismatch)
        );
        assert_eq!(
            material.validate_source_graph(
                id(2),
                source,
                id(3),
                window,
                id(4),
                statistic,
                None,
                id(6)
            ),
            Err(Error::LinkageMismatch)
        );
        assert_eq!(
            material.validate_source_graph(
                id(2),
                source,
                id(3),
                window,
                id(4),
                statistic,
                Some(id(5)),
                id(7)
            ),
            Err(Error::LinkageMismatch)
        );
    }

    #[test]
    fn encode_decode_is_deterministic_and_canonical() {
        let material = SourceMaterialV2::new(id(11), id(12), id(13), id(14), None, id(15));
        let bytes = material.to_bytes();
        assert_eq!(SourceMaterialV2::decode(&bytes), Ok(material));
        assert_eq!(
            SourceMaterialV2::decode(&bytes).expect("decode").to_bytes(),
            bytes
        );
    }
}
