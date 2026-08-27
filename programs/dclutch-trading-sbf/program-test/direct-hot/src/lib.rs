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

use dclutch_capability_program_contract::{
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
use dclutch_custody_contract::CustodyReplayLayoutV1;
use dclutch_direct_codec::{
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
};
use dclutch_product_runtime_v2::{
    DOMAIN_CUT_BYTES, DOMAIN_HEADER_BYTES, PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_HEADER_BYTES,
};
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_registry_contract::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
use dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES;
use dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;
use sha2::{Digest, Sha256};

/// Capacity-profile identity used only by this reproducible ProgramTest fixture.
pub const DIRECT_HOT_FIXTURE_CAPACITY_PROFILE_V5: [u8; 32] = [0x44; 32];
/// Exact descriptor identity emitted for the fixed fixture capacity profile.
pub const DIRECT_HOT_FIXTURE_DESCRIPTOR_ID_V5: [u8; 32] = [
    0xfb, 0x41, 0x92, 0x0a, 0x61, 0x5e, 0xb8, 0x64, 0x32, 0xd7, 0xf9, 0x48, 0xf3, 0x5b, 0xa0, 0x43,
    0xb5, 0x57, 0x35, 0x6d, 0x6e, 0xc6, 0x86, 0x12, 0x6f, 0x52, 0xb6, 0x18, 0x82, 0x85, 0x68, 0x76,
];
/// Exact one-entry ProgramSet identity selecting the fixture descriptor.
pub const DIRECT_HOT_FIXTURE_PROGRAM_SET_ID_V5: [u8; 32] = [
    0x0c, 0x3e, 0xb4, 0xa2, 0xb9, 0x53, 0x4e, 0xf2, 0xad, 0x5e, 0xee, 0xbe, 0xae, 0x95, 0xbf, 0x23,
    0x3b, 0xa2, 0xd1, 0x29, 0x07, 0x1c, 0x0c, 0xf5, 0x8c, 0xc3, 0x00, 0x18, 0x6e, 0x76, 0x97, 0x91,
];
/// Superseded over-wide domain AccountProfile identity used only for hostile refusal evidence.
pub const STALE_DIRECT_ACCOUNT_PROFILE_ID_V3: [u8; 32] = [
    0x3c, 0xb3, 0x57, 0xd3, 0x16, 0xd7, 0x6d, 0x73, 0xe4, 0x62, 0xc0, 0x36, 0xd7, 0x64, 0x86, 0xef,
    0x42, 0x7d, 0x4d, 0x71, 0xec, 0x44, 0x64, 0xf3, 0xee, 0x3b, 0x2c, 0x15, 0x88, 0xd3, 0xe8, 0xb4,
];

const CLAIMS_ROW_BYTES: usize = 8;
const CLAIMS_MARKET_HEADER_BYTES: usize = 256;
const CLAIMS_POSITION_HEADER_BYTES: usize = 128;
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
pub fn build_direct_hot_artifact_fixture_v5(
    deployment: DirectHotDeploymentWidthsV5,
) -> Result<DirectHotArtifactFixtureV5, DirectHotFixtureErrorV5> {
    let logical_data_lengths = direct_logical_data_lengths_v5(deployment)?;
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
        dclutch_account_profile_contract::lifecycle_v3::SUCCESSOR_SCHEMA_RELEASE_ID,
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
) -> Result<Vec<u32>, DirectHotFixtureErrorV5> {
    let mut output = vec![0_u32; usize::from(DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3)];
    put_width(
        &mut output,
        0,
        dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(dclutch_direct_codec::successor::DIRECT_ROOT_STATE_BYTES_V1)
            .ok_or(DirectHotFixtureErrorV5::InvalidWidth)?,
    )?;
    put_width(
        &mut output,
        1,
        dclutch_direct_codec::successor::DIRECT_EXECUTION_CONFIG_BYTES_V1,
    )?;
    put_width(&mut output, 2, PRODUCT_RECORD_BYTES_V2)?;
    put_width(
        &mut output,
        3,
        PORTFOLIO_HEADER_BYTES
            .checked_add(
                3_usize
                    .checked_mul(PORTFOLIO_COEFFICIENT_BYTES)
                    .ok_or(DirectHotFixtureErrorV5::InvalidWidth)?,
            )
            .ok_or(DirectHotFixtureErrorV5::InvalidWidth)?,
    )?;
    put_width(
        &mut output,
        4,
        dclutch_product_payoff_v2_codec::runtime_v3::BASIS_HEADER_BYTES_V3,
    )?;
    for coordinate in [5_usize, 8] {
        put_width(
            &mut output,
            coordinate,
            dclutch_direct_codec::successor::DIRECT_MAKER_REPLAY_BYTES_V1,
        )?;
    }
    put_width(&mut output, 7, LIFECYCLE_RENT_CREDIT_BYTES_V2)?;
    // Coordinate 10 is the executable Rent program that owns the credit. Its
    // rule is opaque, so this width is descriptive only and never pinned.
    put_width(&mut output, 10, LOADER_V3_PROGRAM_BYTES)?;
    put_width(
        &mut output,
        13,
        CLAIMS_MARKET_HEADER_BYTES
            .checked_add(
                3_usize
                    .checked_mul(CLAIMS_ROW_BYTES)
                    .ok_or(DirectHotFixtureErrorV5::InvalidWidth)?,
            )
            .ok_or(DirectHotFixtureErrorV5::InvalidWidth)?,
    )?;
    alias_width(&mut output, 14, 4)?;
    put_width(&mut output, 16, PRODUCT_RECORD_BYTES_V2)?;
    put_width(
        &mut output,
        18,
        DOMAIN_HEADER_BYTES
            .checked_sub(
                2_usize
                    .checked_mul(DOMAIN_CUT_BYTES)
                    .ok_or(DirectHotFixtureErrorV5::InvalidWidth)?,
            )
            .and_then(|base| base.checked_add(3_usize.checked_mul(DOMAIN_CUT_BYTES)?))
            .ok_or(DirectHotFixtureErrorV5::InvalidWidth)?,
    )?;
    alias_width(&mut output, 20, 3)?;
    *output
        .get_mut(22)
        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)? = 17;
    put_width(&mut output, 23, dclutch_market_core_codec::STATE_BYTES)?;
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
        put_width(
            &mut output,
            coordinate,
            CLAIMS_POSITION_HEADER_BYTES
                .checked_add(
                    3_usize
                        .checked_mul(CLAIMS_ROW_BYTES)
                        .ok_or(DirectHotFixtureErrorV5::InvalidWidth)?,
                )
                .ok_or(DirectHotFixtureErrorV5::InvalidWidth)?,
        )?;
    }
    alias_width(&mut output, 35, 23)?;
    alias_width(&mut output, 36, 24)?;
    alias_width(&mut output, 37, 25)?;
    alias_width(&mut output, 38, 26)?;
    alias_width(&mut output, 39, 27)?;
    put_width(&mut output, 40, dclutch_realm_contract::REALM_BYTES)?;
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
    use dclutch_account_profile_contract::v2::AccountProfileV2;
    use dclutch_direct_codec::{
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
        let fixture = build_direct_hot_artifact_fixture_v5(real_widths()).expect("fixture");
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
        let fixture = build_direct_hot_artifact_fixture_v5(real_widths()).expect("fixture");
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
        assert_eq!(profile.common_scalar_count(), 65);
    }

    #[test]
    fn stale_profile_and_lifecycle_v4_substitutions_refuse() {
        let fixture = build_direct_hot_artifact_fixture_v5(real_widths()).expect("fixture");
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

    #[test]
    fn placeholder_programdata_width_is_not_a_profile_authority() {
        let real = build_direct_hot_artifact_fixture_v5(real_widths()).expect("real fixture");
        let placeholder = build_direct_hot_artifact_fixture_v5(
            DirectHotDeploymentWidthsV5::new(1, 1, 1).expect("placeholder widths"),
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
        let mut fixture = build_direct_hot_artifact_fixture_v5(real_widths()).expect("fixture");
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
