//! Acyclic Source policy rooted in one exact principal-policy selection.

use core::convert::TryInto;

use super::{
    ContentId, Error, ManipulationFloorV1, MarketPrincipalCapSetsV1, Result,
    SourceCapacityProfileV1, SourceSpecV1, StatisticSpecV1, WindowSpecV1,
    derive_market_principal_cap,
    generated_source_material_v3::{
        SOURCE_MATERIAL_V3_BOUNDED_BY_FLOOR_TAG, SOURCE_MATERIAL_V3_BYTES,
        SOURCE_MATERIAL_V3_EXPLICITLY_UNBOUNDED_TAG,
        SOURCE_MATERIAL_V3_FAILURE_POLICY_RELEASE_OFFSET, SOURCE_MATERIAL_V3_MAGIC,
        SOURCE_MATERIAL_V3_MAGIC_OFFSET, SOURCE_MATERIAL_V3_MANIPULATION_FLOOR_OFFSET,
        SOURCE_MATERIAL_V3_PRIMARY_SOURCE_SPEC_OFFSET, SOURCE_MATERIAL_V3_PRINCIPAL_POLICY_OFFSET,
        SOURCE_MATERIAL_V3_PRODUCT_RECORD_DIGEST_OFFSET, SOURCE_MATERIAL_V3_RECOVERY_POLICY_OFFSET,
        SOURCE_MATERIAL_V3_RECOVERY_PRESENT_OFFSET, SOURCE_MATERIAL_V3_RESERVED_OFFSET,
        SOURCE_MATERIAL_V3_SCHEMA_VERSION, SOURCE_MATERIAL_V3_STATISTIC_SPEC_OFFSET,
        SOURCE_MATERIAL_V3_VERSION_OFFSET, SOURCE_MATERIAL_V3_WINDOW_SPEC_OFFSET,
    },
};

const ID_BYTES: usize = 32;
const RESERVED_BYTES: usize = 4;

/// The sole principal-policy selection carried by [`SourceMaterialV3`].
///
/// The bounded form owns one exact floor content identity. It does not accept
/// any other floor that merely shares the same Source, adapter, and collateral
/// bindings. The unbounded form owns no floor at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourcePrincipalPolicyV1 {
    /// This Source material deliberately admits every representable set count.
    ExplicitlyUnbounded,
    /// This Source material selects one exact manipulation-floor record.
    BoundedByFloor(ContentId),
}

/// Immutable Source graph root with an acyclic principal-policy binding.
///
/// Its content graph is `CapacityProfile(κ) -> SourceSpec(profile) ->
/// ManipulationFloor(source) -> SourceMaterialV3(source, floor, policy)`. The
/// capacity profile never points back to the floor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceMaterialV3 {
    product_record_digest: ContentId,
    primary_source_spec: ContentId,
    window_spec: ContentId,
    statistic_spec: ContentId,
    recovery_policy: Option<ContentId>,
    failure_policy_release: ContentId,
    principal_policy: SourcePrincipalPolicyV1,
}

impl SourceMaterialV3 {
    /// Construct a bounded Source root selecting one exact manipulation floor.
    #[must_use]
    pub const fn bounded_by_floor(
        product_record_digest: ContentId,
        primary_source_spec: ContentId,
        window_spec: ContentId,
        statistic_spec: ContentId,
        recovery_policy: Option<ContentId>,
        failure_policy_release: ContentId,
        manipulation_floor: ContentId,
    ) -> Self {
        Self {
            product_record_digest,
            primary_source_spec,
            window_spec,
            statistic_spec,
            recovery_policy,
            failure_policy_release,
            principal_policy: SourcePrincipalPolicyV1::BoundedByFloor(manipulation_floor),
        }
    }

    /// Construct a Source root that explicitly selects unbounded principal.
    #[must_use]
    pub const fn explicitly_unbounded(
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
            principal_policy: SourcePrincipalPolicyV1::ExplicitlyUnbounded,
        }
    }

    /// Hostile-decode one exact canonical 240-byte V3 record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != SOURCE_MATERIAL_V3_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, SOURCE_MATERIAL_V3_MAGIC_OFFSET)? != SOURCE_MATERIAL_V3_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, SOURCE_MATERIAL_V3_VERSION_OFFSET)?)
            != SOURCE_MATERIAL_V3_SCHEMA_VERSION
        {
            return Err(Error::UnsupportedSchema);
        }
        if bytes
            .get(
                SOURCE_MATERIAL_V3_RESERVED_OFFSET
                    ..SOURCE_MATERIAL_V3_RESERVED_OFFSET + RESERVED_BYTES,
            )
            .ok_or(Error::InvalidLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalReservedBytes);
        }

        let recovery_bytes = array::<ID_BYTES>(bytes, SOURCE_MATERIAL_V3_RECOVERY_POLICY_OFFSET)?;
        let recovery_policy = match byte(bytes, SOURCE_MATERIAL_V3_RECOVERY_PRESENT_OFFSET)? {
            0 if recovery_bytes.iter().all(|byte| *byte == 0) => None,
            1 => Some(ContentId::new(recovery_bytes)?),
            _ => return Err(Error::NonCanonicalSourceMaterial),
        };
        let floor_bytes = array::<ID_BYTES>(bytes, SOURCE_MATERIAL_V3_MANIPULATION_FLOOR_OFFSET)?;
        let principal_policy = match byte(bytes, SOURCE_MATERIAL_V3_PRINCIPAL_POLICY_OFFSET)? {
            SOURCE_MATERIAL_V3_EXPLICITLY_UNBOUNDED_TAG
                if floor_bytes.iter().all(|byte| *byte == 0) =>
            {
                SourcePrincipalPolicyV1::ExplicitlyUnbounded
            }
            SOURCE_MATERIAL_V3_BOUNDED_BY_FLOOR_TAG => {
                SourcePrincipalPolicyV1::BoundedByFloor(ContentId::new(floor_bytes)?)
            }
            _ => return Err(Error::NonCanonicalSourceMaterial),
        };

        Ok(Self {
            product_record_digest: content(bytes, SOURCE_MATERIAL_V3_PRODUCT_RECORD_DIGEST_OFFSET)?,
            primary_source_spec: content(bytes, SOURCE_MATERIAL_V3_PRIMARY_SOURCE_SPEC_OFFSET)?,
            window_spec: content(bytes, SOURCE_MATERIAL_V3_WINDOW_SPEC_OFFSET)?,
            statistic_spec: content(bytes, SOURCE_MATERIAL_V3_STATISTIC_SPEC_OFFSET)?,
            recovery_policy,
            failure_policy_release: content(
                bytes,
                SOURCE_MATERIAL_V3_FAILURE_POLICY_RELEASE_OFFSET,
            )?,
            principal_policy,
        })
    }

    /// Encode the exact canonical V3 record.
    #[must_use]
    pub fn to_bytes(self) -> [u8; SOURCE_MATERIAL_V3_BYTES] {
        let mut output = [0_u8; SOURCE_MATERIAL_V3_BYTES];
        put(
            &mut output,
            SOURCE_MATERIAL_V3_MAGIC_OFFSET,
            &SOURCE_MATERIAL_V3_MAGIC,
        );
        put(
            &mut output,
            SOURCE_MATERIAL_V3_VERSION_OFFSET,
            &SOURCE_MATERIAL_V3_SCHEMA_VERSION.to_le_bytes(),
        );
        put(
            &mut output,
            SOURCE_MATERIAL_V3_PRODUCT_RECORD_DIGEST_OFFSET,
            self.product_record_digest.as_bytes(),
        );
        put(
            &mut output,
            SOURCE_MATERIAL_V3_PRIMARY_SOURCE_SPEC_OFFSET,
            self.primary_source_spec.as_bytes(),
        );
        put(
            &mut output,
            SOURCE_MATERIAL_V3_WINDOW_SPEC_OFFSET,
            self.window_spec.as_bytes(),
        );
        put(
            &mut output,
            SOURCE_MATERIAL_V3_STATISTIC_SPEC_OFFSET,
            self.statistic_spec.as_bytes(),
        );
        if let Some(recovery) = self.recovery_policy {
            output[SOURCE_MATERIAL_V3_RECOVERY_PRESENT_OFFSET] = 1;
            put(
                &mut output,
                SOURCE_MATERIAL_V3_RECOVERY_POLICY_OFFSET,
                recovery.as_bytes(),
            );
        }
        put(
            &mut output,
            SOURCE_MATERIAL_V3_FAILURE_POLICY_RELEASE_OFFSET,
            self.failure_policy_release.as_bytes(),
        );
        match self.principal_policy {
            SourcePrincipalPolicyV1::ExplicitlyUnbounded => {
                output[SOURCE_MATERIAL_V3_PRINCIPAL_POLICY_OFFSET] =
                    SOURCE_MATERIAL_V3_EXPLICITLY_UNBOUNDED_TAG;
            }
            SourcePrincipalPolicyV1::BoundedByFloor(floor) => {
                output[SOURCE_MATERIAL_V3_PRINCIPAL_POLICY_OFFSET] =
                    SOURCE_MATERIAL_V3_BOUNDED_BY_FLOOR_TAG;
                put(
                    &mut output,
                    SOURCE_MATERIAL_V3_MANIPULATION_FLOOR_OFFSET,
                    floor.as_bytes(),
                );
            }
        }
        output
    }

    /// Require the exact Product Runtime record selected by this Source root.
    pub fn authenticate_product_record(self, authenticated_digest: ContentId) -> Result<()> {
        if authenticated_digest == self.product_record_digest {
            Ok(())
        } else {
            Err(Error::LinkageMismatch)
        }
    }

    /// Validate the Source-owned record graph after the adapter authenticates
    /// every named finalized record by its content identity.
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

    /// Authenticate the acyclic principal graph and derive the runtime set cap.
    ///
    /// The composing adapter has authenticated each supplied record preimage by
    /// the adjacent content ID. This kernel then checks every graph edge before
    /// projecting. A bounded material requires the one selected floor ID; a
    /// different, larger floor with identical bindings refuses. An explicitly
    /// unbounded material requires `None`, so no unused floor can be smuggled
    /// into that policy.
    #[allow(clippy::too_many_arguments)]
    pub fn derive_principal_cap_sets(
        self,
        authenticated_source_spec_id: ContentId,
        source: SourceSpecV1,
        authenticated_capacity_profile_id: ContentId,
        capacity_profile: SourceCapacityProfileV1,
        authenticated_floor: Option<(ContentId, ManipulationFloorV1)>,
        market_collateral_unit_id: ContentId,
        basis_scale: u64,
    ) -> Result<MarketPrincipalCapSetsV1> {
        if self.primary_source_spec != authenticated_source_spec_id
            || source.capacity_profile_id() != authenticated_capacity_profile_id
        {
            return Err(Error::LinkageMismatch);
        }
        let capacity = capacity_profile.principal_capacity()?;
        match (self.principal_policy, authenticated_floor) {
            (SourcePrincipalPolicyV1::ExplicitlyUnbounded, None) => {
                if basis_scale == 0 {
                    return Err(Error::ZeroCapacity);
                }
                Ok(MarketPrincipalCapSetsV1::Unbounded)
            }
            (
                SourcePrincipalPolicyV1::BoundedByFloor(selected_floor_id),
                Some((authenticated_floor_id, floor)),
            ) if selected_floor_id == authenticated_floor_id => derive_market_principal_cap(
                capacity,
                floor,
                authenticated_source_spec_id,
                source,
                market_collateral_unit_id,
            )?
            .in_complete_sets(basis_scale),
            _ => Err(Error::LinkageMismatch),
        }
    }

    /// Exact Product Runtime record content digest.
    #[must_use]
    pub const fn product_record_digest(self) -> ContentId {
        self.product_record_digest
    }

    /// Primary SourceSpec content identity.
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

    /// The sole principal-policy selection.
    #[must_use]
    pub const fn principal_policy(self) -> SourcePrincipalPolicyV1 {
        self.principal_policy
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
    use crate::{
        BONDING_CURVE_FLOOR_DERIVATION_ID_V1, CapacityEnvelope, ManipulationFloorBasis,
        SourceAccessProfile,
        generated_source_material_v3::{
            SOURCE_MATERIAL_V3_BOUNDED_EXAMPLE, SOURCE_MATERIAL_V3_REFUSAL_CORPUS,
            SOURCE_MATERIAL_V3_REFUSAL_COUNT, SOURCE_MATERIAL_V3_UNBOUNDED_EXAMPLE,
        },
    };

    fn id(tag: u8) -> ContentId {
        let mut bytes = [0_u8; 32];
        bytes[0] = tag;
        ContentId::new(bytes).expect("nonzero ID")
    }

    fn material_bounded(floor: ContentId) -> SourceMaterialV3 {
        SourceMaterialV3::bounded_by_floor(id(1), id(2), id(3), id(4), Some(id(5)), id(6), floor)
    }

    fn source(capacity_profile: ContentId) -> SourceSpecV1 {
        SourceSpecV1::new(
            id(20),
            id(21),
            id(22),
            SourceAccessProfile::RelayedObservationRecord,
            id(23),
            capacity_profile,
        )
    }

    fn capacity() -> SourceCapacityProfileV1 {
        SourceCapacityProfileV1::new(CapacityEnvelope::Provisional, 1, 0, id(30), id(31), 208, 0)
            .expect("capacity")
            .bounding_principal(1, 4)
            .expect("κ")
    }

    fn floor(source_spec: ContentId, atoms: u64) -> ManipulationFloorV1 {
        ManipulationFloorV1::new(
            ManipulationFloorBasis::CurveDerived,
            source_spec,
            id(23),
            id(40),
            ContentId::new(BONDING_CURVE_FLOOR_DERIVATION_ID_V1).expect("derivation"),
            atoms,
        )
    }

    #[test]
    fn lean_generated_examples_agree() {
        let bounded = material_bounded(id(7));
        assert_eq!(bounded.to_bytes(), SOURCE_MATERIAL_V3_BOUNDED_EXAMPLE);
        assert_eq!(SourceMaterialV3::decode(&bounded.to_bytes()), Ok(bounded));

        let unbounded =
            SourceMaterialV3::explicitly_unbounded(id(1), id(2), id(3), id(4), None, id(6));
        assert_eq!(unbounded.to_bytes(), SOURCE_MATERIAL_V3_UNBOUNDED_EXAMPLE);
        assert_eq!(
            SourceMaterialV3::decode(&unbounded.to_bytes()),
            Ok(unbounded)
        );
    }

    #[test]
    fn lean_generated_refusal_corpus_fails_closed() {
        assert_eq!(
            SOURCE_MATERIAL_V3_REFUSAL_CORPUS.len(),
            SOURCE_MATERIAL_V3_REFUSAL_COUNT
        );
        for hostile in SOURCE_MATERIAL_V3_REFUSAL_CORPUS {
            assert!(SourceMaterialV3::decode(&hostile).is_err());
        }
    }

    #[test]
    fn exact_floor_selection_refuses_pick_largest_substitution() {
        let source_id = id(2);
        let profile_id = id(32);
        let selected_floor_id = id(41);
        let larger_floor_id = id(42);
        let material = material_bounded(selected_floor_id);
        let selected = floor(source_id, 1_000);
        let larger = floor(source_id, 1_000_000);

        assert_eq!(
            material.derive_principal_cap_sets(
                source_id,
                source(profile_id),
                profile_id,
                capacity(),
                Some((selected_floor_id, selected)),
                id(40),
                100,
            ),
            Ok(MarketPrincipalCapSetsV1::Bounded(2))
        );
        assert_eq!(
            material.derive_principal_cap_sets(
                source_id,
                source(profile_id),
                profile_id,
                capacity(),
                Some((larger_floor_id, larger)),
                id(40),
                100,
            ),
            Err(Error::LinkageMismatch)
        );
    }

    #[test]
    fn every_acyclic_graph_edge_is_checked_before_projection() {
        let source_id = id(2);
        let profile_id = id(32);
        let selected_floor_id = id(41);
        let material = material_bounded(selected_floor_id);
        let selected = floor(source_id, 1_000);

        assert_eq!(
            material.derive_principal_cap_sets(
                id(99),
                source(profile_id),
                profile_id,
                capacity(),
                Some((selected_floor_id, selected)),
                id(40),
                1,
            ),
            Err(Error::LinkageMismatch)
        );
        assert_eq!(
            material.derive_principal_cap_sets(
                source_id,
                source(profile_id),
                id(99),
                capacity(),
                Some((selected_floor_id, selected)),
                id(40),
                1,
            ),
            Err(Error::LinkageMismatch)
        );
        assert_eq!(
            material.derive_principal_cap_sets(
                source_id,
                source(profile_id),
                profile_id,
                capacity(),
                Some((selected_floor_id, selected)),
                id(99),
                1,
            ),
            Err(Error::LinkageMismatch)
        );
    }

    #[test]
    fn explicit_unboundedness_neither_defaults_nor_accepts_a_floor() {
        let source_id = id(2);
        let profile_id = id(32);
        let material =
            SourceMaterialV3::explicitly_unbounded(id(1), source_id, id(3), id(4), None, id(6));
        assert_eq!(
            material.derive_principal_cap_sets(
                source_id,
                source(profile_id),
                profile_id,
                capacity(),
                None,
                id(40),
                100,
            ),
            Ok(MarketPrincipalCapSetsV1::Unbounded)
        );
        assert_eq!(
            material.derive_principal_cap_sets(
                source_id,
                source(profile_id),
                profile_id,
                capacity(),
                Some((id(41), floor(source_id, 1_000_000))),
                id(40),
                100,
            ),
            Err(Error::LinkageMismatch)
        );
        assert_eq!(
            material.derive_principal_cap_sets(
                source_id,
                source(profile_id),
                profile_id,
                capacity(),
                None,
                id(40),
                0,
            ),
            Err(Error::ZeroCapacity)
        );
    }
}
