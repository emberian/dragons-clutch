//! Shared real-ELF fixture authority for Direct through Trading Hot.
//!
//! This crate builds the exact finalized Direct artifact family from public
//! semantic-owner encoders.  The Registry continuation campaign supplies its
//! already-authenticated release cache, program identities, and deployment
//! accounts; this module never creates a second release truth.

#![forbid(unsafe_code)]

pub mod chain;
pub mod fixture;
pub mod waist;

use dclutch_market::capability_program::{
    set_v2::{
        CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, SelectorWidthV2,
        encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::{
        CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_PROGRAM_OFFSET,
        CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET,
        SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_custody::CustodyReplayLayoutV1;
use dclutch_trading::{
    execution_v3::DirectExecutionActionV3,
    ordinary_account_artifacts_v3::DirectInlineOrdinaryAccountProfileInputV3,
    ordinary_bundle_v4::{
        DirectInlineOrdinaryHotBundleErrorV4, DirectInlineOrdinaryHotBundleInputV4,
        DirectInlineOrdinaryHotBundleV4, build_direct_inline_ordinary_hot_bundle_v4,
        validate_direct_inline_ordinary_hot_bundle_v4,
    },
    ordinary_effect_artifacts_v3::{
        DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3, DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3,
    },
    ordinary_geometry_v3::DirectOrdinaryGeometryV3,
};
use dclutch_product::admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_registry::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
use dclutch_registry::svm::LOADER_V3_PROGRAM_BYTES;
use dclutch_market::rent::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;
use sha2::{Digest, Sha256};

/// Capacity-profile identity used only by this reproducible ProgramTest fixture.
pub const DIRECT_HOT_FIXTURE_CAPACITY_PROFILE_V5: [u8; 32] = [0x44; 32];
/// Exact descriptor identity emitted for the fixed fixture capacity profile.
pub const DIRECT_HOT_FIXTURE_DESCRIPTOR_ID_V5: [u8; 32] = [
    0xa2, 0x5b, 0x14, 0x80, 0x0a, 0xe7, 0x07, 0x66, 0xf3, 0x1d, 0xc2, 0x46, 0xdf, 0x73, 0x30, 0x1c,
    0x63, 0xd2, 0x93, 0xf8, 0xf6, 0x46, 0x0c, 0x61, 0x1f, 0xf2, 0x74, 0x15, 0xd9, 0x9b, 0x8f, 0x08,
];
/// Exact one-entry ProgramSet identity selecting the fixture descriptor.
pub const DIRECT_HOT_FIXTURE_PROGRAM_SET_ID_V5: [u8; 32] = [
    0xf0, 0x84, 0x25, 0x0e, 0x3d, 0xe5, 0x97, 0x12, 0x49, 0x38, 0xcb, 0x27, 0xc1, 0x38, 0x3f, 0x28,
    0x82, 0xf6, 0x9f, 0xe3, 0x56, 0xa5, 0x39, 0x55, 0xcc, 0xc9, 0xa5, 0x6d, 0xeb, 0x9a, 0xd8, 0x99,
];
/// Superseded over-wide domain AccountProfile identity used only for hostile refusal evidence.
pub const STALE_DIRECT_ACCOUNT_PROFILE_ID_V3: [u8; 32] = [
    0x3c, 0xb3, 0x57, 0xd3, 0x16, 0xd7, 0x6d, 0x73, 0xe4, 0x62, 0xc0, 0x36, 0xd7, 0x64, 0x86, 0xef,
    0x42, 0x7d, 0x4d, 0x71, 0xec, 0x44, 0x64, 0xf3, 0xee, 0x3b, 0x2c, 0x15, 0x88, 0xd3, 0xe8, 0xb4,
];

const TOKEN_MINT_BYTES: u32 = 82;
const TOKEN_ACCOUNT_BYTES: u32 = 165;

/// Exact Loader ProgramData observations selected by the Registry fixture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectHotDeploymentWidthsV5 {
    /// Exact current Trading ProgramData account width, including Loader header.
    pub trading_programdata_bytes: u32,
    /// Exact current Claims ProgramData account width, including Loader header.
    pub claims_programdata_bytes: u32,
    /// Exact current Core ProgramData account width, including Loader header.
    pub core_programdata_bytes: u32,
}

impl DirectHotDeploymentWidthsV5 {
    /// Construct checked nonempty real deployment observations.
    pub fn new(
        trading_programdata_bytes: usize,
        claims_programdata_bytes: usize,
        core_programdata_bytes: usize,
    ) -> Result<Self, DirectHotFixtureErrorV5> {
        let value = Self {
            trading_programdata_bytes: checked_nonzero_width(trading_programdata_bytes)?,
            claims_programdata_bytes: checked_nonzero_width(claims_programdata_bytes)?,
            core_programdata_bytes: checked_nonzero_width(core_programdata_bytes)?,
        };
        Ok(value)
    }
}

/// Exact artifact and logical-account fixture shared by direct and continuation tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectHotArtifactFixtureV5 {
    /// Exact logical Profile13 account widths before physical packing.
    pub logical_data_lengths: Vec<u32>,
    /// Exact six-artifact CapabilityProgramV4 bundle.
    pub bundle: DirectInlineOrdinaryHotBundleV4,
    /// Exact one-entry ProgramSetV2 selecting `bundle.descriptor`.
    pub program_set: Vec<u8>,
    /// SHA-256 of the descriptor bytes.
    pub descriptor_id: [u8; 32],
    /// SHA-256 of the ProgramSet bytes.
    pub program_set_id: [u8; 32],
}

/// Stable refusal from fixture construction or hostile mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectHotFixtureErrorV5 {
    /// A host or account width was zero or exceeded the protocol u32 coordinate.
    InvalidWidth,
    /// Canonical Direct artifact generation refused the supplied geometry.
    Artifact(DirectInlineOrdinaryHotBundleErrorV4),
    /// Canonical ProgramSet encoding refused the descriptor selection.
    ProgramSet,
}

/// Build the exact current Direct fixed-topology artifacts using real deployment widths.
///
/// The geometry decides only what account observations this fixture STATES.
/// It does not decide the artifacts: every runtime-width coordinate is an
/// affine rule the runtime resolves against the transaction's own Product
/// tail, so the emitted bundle is the same bytes -- and the same content
/// identities -- at every geometry. `the_artifacts_are_the_same_bytes_at_every_geometry`
/// is that claim, executed.
pub fn build_direct_hot_artifact_fixture_v5(
    deployment: DirectHotDeploymentWidthsV5,
    geometry: DirectOrdinaryGeometryV3,
) -> Result<DirectHotArtifactFixtureV5, DirectHotFixtureErrorV5> {
    let logical_data_lengths = direct_logical_data_lengths_v5(deployment, geometry)?;
    let bundle = build_direct_inline_ordinary_hot_bundle_v4(DirectInlineOrdinaryHotBundleInputV4 {
        account_profile: DirectInlineOrdinaryAccountProfileInputV3 {
            logical_data_lengths: &logical_data_lengths,
        },
        capacity_profile: DIRECT_HOT_FIXTURE_CAPACITY_PROFILE_V5,
    })
    .map_err(DirectHotFixtureErrorV5::Artifact)?;
    let descriptor_id = digest(&bundle.descriptor);
    let entry = CapabilityProgramSetEntryV2::new(
        DirectExecutionActionV3::InlineOrdinary as u32,
        CapabilityDescriptorReferenceV2::new(
            content(CAPABILITY_PROGRAM_SCHEMA_ID_V4)?,
            content(descriptor_id)?,
        ),
    );
    let mut program_set = vec![
        0_u8;
        encoded_program_set_bytes_v2(1)
            .map_err(|_| DirectHotFixtureErrorV5::ProgramSet)?
    ];
    encode_program_set_v2(12, SelectorWidthV2::U32, &[entry], &mut program_set)
        .map_err(|_| DirectHotFixtureErrorV5::ProgramSet)?;
    let program_set_id = digest(&program_set);
    Ok(DirectHotArtifactFixtureV5 {
        logical_data_lengths,
        bundle,
        program_set,
        descriptor_id,
        program_set_id,
    })
}

/// Replace the selected AccountProfile identity with the superseded pre-Profile13 ID.
#[must_use]
pub fn with_stale_account_profile_id_v5(
    mut bundle: DirectInlineOrdinaryHotBundleV4,
) -> DirectInlineOrdinaryHotBundleV4 {
    copy32(
        &mut bundle.descriptor,
        CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_PROGRAM_OFFSET,
        STALE_DIRECT_ACCOUNT_PROFILE_ID_V3,
    );
    bundle
}

/// Replace the selected lifecycle schema with the superseded V4 schema.
#[must_use]
pub fn with_stale_lifecycle_v4_schema(
    mut bundle: DirectInlineOrdinaryHotBundleV4,
) -> DirectInlineOrdinaryHotBundleV4 {
    copy32(
        &mut bundle.descriptor,
        CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET,
        dclutch_vm::account_profile::lifecycle_v3::SUCCESSOR_SCHEMA_RELEASE_ID,
    );
    bundle
}

/// Revalidate one exact artifact fixture under its canonical capacity profile.
pub fn validate_direct_hot_artifact_fixture_v5(
    bundle: &DirectInlineOrdinaryHotBundleV4,
) -> Result<(), DirectInlineOrdinaryHotBundleErrorV4> {
    validate_direct_inline_ordinary_hot_bundle_v4(bundle, DIRECT_HOT_FIXTURE_CAPACITY_PROFILE_V5)
}

fn direct_logical_data_lengths_v5(
    deployment: DirectHotDeploymentWidthsV5,
    geometry: DirectOrdinaryGeometryV3,
) -> Result<Vec<u32>, DirectHotFixtureErrorV5> {
    let mut output = vec![0_u32; usize::from(DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3)];
    put_width(
        &mut output,
        0,
        dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(dclutch_trading::successor::DIRECT_ROOT_STATE_BYTES_V1)
            .ok_or(DirectHotFixtureErrorV5::InvalidWidth)?,
    )?;
    put_width(
        &mut output,
        1,
        dclutch_trading::successor::DIRECT_EXECUTION_CONFIG_BYTES_V1,
    )?;
    put_width(&mut output, 2, PRODUCT_RECORD_BYTES_V2)?;
    put_geometry_width(&mut output, 3, geometry.portfolio_record_bytes())?;
    put_width(
        &mut output,
        4,
        dclutch_product::payoff::runtime_v3::BASIS_HEADER_BYTES_V3,
    )?;
    for coordinate in [5_usize, 8] {
        put_width(
            &mut output,
            coordinate,
            dclutch_trading::successor::DIRECT_MAKER_REPLAY_BYTES_V1,
        )?;
    }
    put_width(&mut output, 7, LIFECYCLE_RENT_CREDIT_BYTES_V2)?;
    // Coordinate 10 is the executable Rent program that owns the credit. Its
    // rule is opaque, so this width is descriptive only and never pinned.
    put_width(&mut output, 10, LOADER_V3_PROGRAM_BYTES)?;
    put_geometry_width(&mut output, 13, geometry.claims_aggregate_record_bytes())?;
    alias_width(&mut output, 14, 4)?;
    put_width(&mut output, 16, PRODUCT_RECORD_BYTES_V2)?;
    put_geometry_width(&mut output, 18, geometry.result_domain_record_bytes())?;
    alias_width(&mut output, 20, 3)?;
    *output
        .get_mut(22)
        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)? = 17;
    put_width(&mut output, 23, dclutch_market::STATE_BYTES)?;
    put_width(&mut output, 24, ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)?;
    for coordinate in [25_usize, 26, 28, 30] {
        put_width(&mut output, coordinate, LOADER_V3_PROGRAM_BYTES)?;
    }
    *output
        .get_mut(27)
        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)? = deployment.trading_programdata_bytes;
    *output
        .get_mut(29)
        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)? = deployment.claims_programdata_bytes;
    *output
        .get_mut(31)
        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)? = deployment.core_programdata_bytes;
    for coordinate in [32_usize, 33] {
        put_geometry_width(
            &mut output,
            coordinate,
            geometry.claims_position_record_bytes(),
        )?;
    }
    alias_width(&mut output, 35, 23)?;
    alias_width(&mut output, 36, 24)?;
    alias_width(&mut output, 37, 25)?;
    alias_width(&mut output, 38, 26)?;
    alias_width(&mut output, 39, 27)?;
    put_width(&mut output, 40, dclutch_market::realm::REALM_BYTES)?;
    put_width(&mut output, 42, CustodyReplayLayoutV1::BYTES)?;
    *output
        .get_mut(43)
        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)? = TOKEN_MINT_BYTES;
    *output
        .get_mut(44)
        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)? = TOKEN_ACCOUNT_BYTES;
    *output
        .get_mut(45)
        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)? = TOKEN_ACCOUNT_BYTES;
    *output
        .get_mut(47)
        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)? =
        checked_nonzero_width(LOADER_V3_PROGRAM_BYTES)?;
    *output
        .get_mut(73)
        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)? = TOKEN_ACCOUNT_BYTES;
    for (account, representative) in [
        (49, 23),
        (50, 24),
        (51, 25),
        (52, 26),
        (53, 27),
        (54, 40),
        (55, 41),
        (56, 42),
        (57, 43),
        (58, 44),
        (59, 45),
        (60, 46),
        (61, 47),
        (63, 23),
        (64, 24),
        (65, 25),
        (66, 26),
        (67, 27),
        (68, 40),
        (69, 41),
        (70, 42),
        (71, 43),
        (72, 44),
        (74, 46),
        (75, 47),
        (77, 23),
        (78, 24),
        (79, 25),
        (80, 26),
        (81, 27),
        (82, 40),
        (83, 41),
        (84, 42),
        (85, 43),
        (86, 44),
        (87, 73),
        (88, 46),
        (89, 47),
    ] {
        alias_width(&mut output, account, representative)?;
    }
    // Coordinate 90 is the release-selected Custody program the four Custody
    // routes are invoked through. Its rule is opaque, so this width is
    // descriptive only and never pinned; ProgramTest installs the real
    // upgradeable-loader record for this key.
    put_width(
        &mut output,
        usize::from(DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3),
        LOADER_V3_PROGRAM_BYTES,
    )?;
    Ok(output)
}

fn checked_nonzero_width(value: usize) -> Result<u32, DirectHotFixtureErrorV5> {
    let output = u32::try_from(value).map_err(|_| DirectHotFixtureErrorV5::InvalidWidth)?;
    if output == 0 {
        return Err(DirectHotFixtureErrorV5::InvalidWidth);
    }
    Ok(output)
}

fn put_width(
    output: &mut [u32],
    coordinate: usize,
    value: usize,
) -> Result<(), DirectHotFixtureErrorV5> {
    *output
        .get_mut(coordinate)
        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)? =
        u32::try_from(value).map_err(|_| DirectHotFixtureErrorV5::InvalidWidth)?;
    Ok(())
}

/// Write one geometry-derived record width, or carry its refusal.
fn put_geometry_width(
    output: &mut [u32],
    coordinate: usize,
    value: Result<u32, dclutch_trading::ordinary_geometry_v3::DirectOrdinaryGeometryErrorV3>,
) -> Result<(), DirectHotFixtureErrorV5> {
    *output
        .get_mut(coordinate)
        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)? =
        value.map_err(|_| DirectHotFixtureErrorV5::InvalidWidth)?;
    Ok(())
}

fn alias_width(
    output: &mut [u32],
    coordinate: usize,
    representative: usize,
) -> Result<(), DirectHotFixtureErrorV5> {
    let value = *output
        .get(representative)
        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)?;
    *output
        .get_mut(coordinate)
        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)? = value;
    Ok(())
}

fn content(value: [u8; 32]) -> Result<ContentId, DirectHotFixtureErrorV5> {
    ContentId::new(value).map_err(|_| DirectHotFixtureErrorV5::ProgramSet)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn copy32(output: &mut [u8], offset: usize, value: [u8; 32]) {
    if let Some(destination) = output.get_mut(offset..offset.saturating_add(32)) {
        destination.copy_from_slice(&value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_vm::account_profile::v2::AccountProfileV2;
    use dclutch_trading::{
        ordinary_artifacts_v3::{
            DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_ID_V3, DIRECT_INLINE_ORDINARY_STRATEGY_ID_V3,
            DIRECT_INLINE_ORDINARY_TRANSITION_ID_V3,
        },
        ordinary_bundle_v4::{
            DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_ID_V3, DIRECT_INLINE_ORDINARY_EFFECT_ID_V4,
            DIRECT_INLINE_ORDINARY_LIFECYCLE_ID_V5,
        },
    };

    fn real_widths() -> DirectHotDeploymentWidthsV5 {
        DirectHotDeploymentWidthsV5::new(1_141_117, 971_053, 934_037)
            .expect("real ProgramData widths")
    }

    #[test]
    fn final_artifact_and_set_identities_are_fresh() {
        let fixture = build_direct_hot_artifact_fixture_v5(
            real_widths(),
            DirectOrdinaryGeometryV3::CANONICAL,
        )
        .expect("fixture");
        assert_eq!(fixture.descriptor_id, DIRECT_HOT_FIXTURE_DESCRIPTOR_ID_V5);
        assert_eq!(fixture.program_set_id, DIRECT_HOT_FIXTURE_PROGRAM_SET_ID_V5);
        assert_eq!(
            digest(&fixture.bundle.account_profile),
            DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_ID_V3
        );
        assert_eq!(
            digest(&fixture.bundle.lifecycle_policy),
            DIRECT_INLINE_ORDINARY_LIFECYCLE_ID_V5
        );
        assert_eq!(
            digest(&fixture.bundle.effect),
            DIRECT_INLINE_ORDINARY_EFFECT_ID_V4
        );
        assert_eq!(
            digest(&fixture.bundle.transition),
            DIRECT_INLINE_ORDINARY_TRANSITION_ID_V3
        );
        assert_eq!(
            digest(&fixture.bundle.request_profile),
            DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_ID_V3
        );
        assert_eq!(
            digest(&fixture.bundle.strategy),
            DIRECT_INLINE_ORDINARY_STRATEGY_ID_V3
        );
        validate_direct_hot_artifact_fixture_v5(&fixture.bundle).expect("bundle");
    }

    #[test]
    fn real_widths_bind_exact_rent_activation_and_program_geometry() {
        let fixture = build_direct_hot_artifact_fixture_v5(
            real_widths(),
            DirectOrdinaryGeometryV3::CANONICAL,
        )
        .expect("fixture");
        assert_eq!(fixture.logical_data_lengths.get(7), Some(&128));
        assert_eq!(
            fixture.logical_data_lengths.get(10),
            Some(&u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("Rent program width"))
        );
        assert_eq!(fixture.logical_data_lengths.get(18), Some(&256));
        assert_eq!(
            fixture.logical_data_lengths.get(24),
            Some(
                &u32::try_from(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1).expect("activation width")
            )
        );
        assert_eq!(fixture.logical_data_lengths.get(27), Some(&1_141_117));
        assert_eq!(fixture.logical_data_lengths.get(29), Some(&971_053));
        assert_eq!(fixture.logical_data_lengths.get(31), Some(&934_037));
        assert_eq!(
            fixture
                .logical_data_lengths
                .get(usize::from(DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3)),
            Some(&u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("Custody program width"))
        );
        let profile = AccountProfileV2::decode(&fixture.bundle.account_profile).expect("profile");
        assert_eq!(profile.fixed_account_count(), 91);
        assert_eq!(profile.dynamic_fixed_span_count(), 0);
        assert_eq!(profile.common_scalar_count(), 68);
    }

    #[test]
    fn stale_profile_and_lifecycle_v4_substitutions_refuse() {
        let fixture = build_direct_hot_artifact_fixture_v5(
            real_widths(),
            DirectOrdinaryGeometryV3::CANONICAL,
        )
        .expect("fixture");
        assert_eq!(
            validate_direct_hot_artifact_fixture_v5(&with_stale_account_profile_id_v5(
                fixture.bundle
            )),
            Err(DirectInlineOrdinaryHotBundleErrorV4::Descriptor)
        );
        assert_eq!(
            validate_direct_hot_artifact_fixture_v5(&with_stale_lifecycle_v4_schema(
                fixture.bundle
            )),
            Err(DirectInlineOrdinaryHotBundleErrorV4::Descriptor)
        );
    }

    /// The whole artifact set, and every identity it carries, is the same at
    /// every market geometry.
    ///
    /// This is the executable form of the family's central geometry claim, at
    /// the layer that actually installs accounts on a chain. If a founder's
    /// market has four outcomes rather than three, it does not need its own
    /// emission, its own descriptor, its own ProgramSet or its own seal -- it
    /// selects the SAME ones, and the Hot executor resolves every
    /// runtime-width rule against the transaction's own Product tail.
    #[test]
    fn the_artifacts_are_the_same_bytes_at_every_geometry() {
        let canonical = build_direct_hot_artifact_fixture_v5(
            real_widths(),
            DirectOrdinaryGeometryV3::CANONICAL,
        )
        .expect("canonical fixture");
        for outcomes in 2..=16_u32 {
            let geometry =
                DirectOrdinaryGeometryV3::from_outcome_count(outcomes).expect("geometry");
            let emitted =
                build_direct_hot_artifact_fixture_v5(real_widths(), geometry).expect("fixture");
            assert_eq!(
                emitted.bundle, canonical.bundle,
                "the artifact bundle moved at {outcomes} outcomes"
            );
            assert_eq!(emitted.program_set, canonical.program_set);
            assert_eq!(emitted.descriptor_id, DIRECT_HOT_FIXTURE_DESCRIPTOR_ID_V5);
            assert_eq!(emitted.program_set_id, DIRECT_HOT_FIXTURE_PROGRAM_SET_ID_V5);
            // The observations DO move -- that is the point. What does not
            // move is what the artifacts say about them.
            assert_ne!(
                emitted.logical_data_lengths == canonical.logical_data_lengths,
                outcomes != DirectOrdinaryGeometryV3::CANONICAL.outcome_count()
            );
        }
    }

    #[test]
    fn placeholder_programdata_width_is_not_a_profile_authority() {
        let real = build_direct_hot_artifact_fixture_v5(
            real_widths(),
            DirectOrdinaryGeometryV3::CANONICAL,
        )
        .expect("real fixture");
        let placeholder = build_direct_hot_artifact_fixture_v5(
            DirectHotDeploymentWidthsV5::new(1, 1, 1).expect("placeholder widths"),
            DirectOrdinaryGeometryV3::CANONICAL,
        )
        .expect("opaque fixture");
        assert_eq!(
            real.bundle.account_profile,
            placeholder.bundle.account_profile
        );
        assert_eq!(real.descriptor_id, placeholder.descriptor_id);
        // The Registry checked-release campaign must still reject placeholder
        // ProgramData contents before entering Trading; Profile13 deliberately
        // does not make Loader deployment byte length a semantic artifact ID.
    }

    #[test]
    fn superseded_rentcredit_width_and_zero_programdata_refuse() {
        assert_eq!(
            DirectHotDeploymentWidthsV5::new(0, 971_053, 934_037),
            Err(DirectHotFixtureErrorV5::InvalidWidth)
        );
        let mut fixture = build_direct_hot_artifact_fixture_v5(
            real_widths(),
            DirectOrdinaryGeometryV3::CANONICAL,
        )
        .expect("fixture");
        // The V1 RentCredit width the profile used to pin; the adapter
        // authenticates 128 bytes of LifecycleRentCreditV2.
        *fixture.logical_data_lengths.get_mut(7).expect("RentCredit") = 48;
        assert_eq!(
            build_direct_inline_ordinary_hot_bundle_v4(DirectInlineOrdinaryHotBundleInputV4 {
                account_profile: DirectInlineOrdinaryAccountProfileInputV3 {
                    logical_data_lengths: &fixture.logical_data_lengths,
                },
                capacity_profile: DIRECT_HOT_FIXTURE_CAPACITY_PROFILE_V5,
            }),
            Err(DirectInlineOrdinaryHotBundleErrorV4::AccountProfile)
        );
    }
}
