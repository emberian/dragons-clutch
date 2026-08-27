//! Schema-bound lifecycle ProgramSet for selected and compact actions.

use dclutch_capability_program_contract::{
    set_v2::{
        CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, CapabilityProgramSetV2,
        SelectorWidthV2, encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
};
use dclutch_core_contract::ContentId;
use dclutch_rational_representation_v2_contract::AuthenticatedTokenBehaviorV2;
use dclutch_rational_representation_v2_lifecycle_contract::{
    LifecycleActionV2, compact_hot_v4::RationalLifecycleCompactHotLayoutV4,
    hot_v3::RationalLifecycleHotLayoutV3,
};
use dclutch_token_svm::TOKEN_BEHAVIOR_SELECTION_BYTES_V2;
use solana_program::hash::hash;

use crate::{
    Error, RationalLifecycleCompactBundleV4, RationalLifecycleSelectedBundleV5, Result,
    validate_rational_lifecycle_compact_bundle_for_authenticated_selection_v4,
    validate_rational_lifecycle_selected_bundle_for_authenticated_selection_v5,
};

/// Four exact lifecycle descriptors sharing one Token behavior selection.
#[derive(Clone, Copy, Debug)]
pub struct RationalLifecycleProgramSetInputV5<'a> {
    /// Independently authenticated Realm/release Token behavior authority.
    pub authenticated_token_behavior: AuthenticatedTokenBehaviorV2,
    /// Fixed-cardinality receipt activation descriptor.
    pub activate_receipt: &'a RationalLifecycleSelectedBundleV5,
    /// Fixed-cardinality coordinate activation descriptor.
    pub activate_coordinate: &'a RationalLifecycleSelectedBundleV5,
    /// Fixed-cardinality coordinate retirement descriptor.
    pub retire_coordinate: &'a RationalLifecycleSelectedBundleV5,
    /// Descriptor-derived complete receipt-retirement descriptor.
    pub retire_receipt: &'a RationalLifecycleCompactBundleV4,
}

/// Canonical config and four-entry lifecycle ProgramSetV2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalLifecycleProgramSetV5 {
    /// Exact Realm/release-selected Token behavior config bytes.
    pub token_behavior_selection: [u8; TOKEN_BEHAVIOR_SELECTION_BYTES_V2],
    /// SHA-256 identity selected as Hot logical config coordinate one.
    pub token_behavior_selection_id: [u8; 32],
    /// Exact schema-bound CapabilityProgramSetV2 bytes.
    pub program_set: Vec<u8>,
    /// SHA-256 identity selected by the Market capability manifest.
    pub program_set_id: [u8; 32],
}

/// Build the sole four-action lifecycle ProgramSet.
pub fn build_rational_lifecycle_program_set_v5(
    input: RationalLifecycleProgramSetInputV5<'_>,
) -> Result<RationalLifecycleProgramSetV5> {
    validate_input(input)?;
    if RationalLifecycleHotLayoutV3::ACTION != RationalLifecycleCompactHotLayoutV4::ACTION {
        return Err(Error::ArtifactGeometry);
    }
    let entries = [
        entry(input.activate_receipt, LifecycleActionV2::ActivateReceipt)?,
        entry(
            input.activate_coordinate,
            LifecycleActionV2::ActivateCoordinate,
        )?,
        entry(input.retire_coordinate, LifecycleActionV2::RetireCoordinate)?,
        compact_entry(input.retire_receipt)?,
    ];
    let width = encoded_program_set_bytes_v2(entries.len()).map_err(|_| Error::ArtifactGeometry)?;
    let mut program_set = vec![0_u8; width];
    encode_program_set_v2(
        u32::try_from(RationalLifecycleHotLayoutV3::ACTION).map_err(|_| Error::ArtifactGeometry)?,
        SelectorWidthV2::U8,
        &entries,
        &mut program_set,
    )
    .map_err(|_| Error::ArtifactGeometry)?;
    let selection = input.authenticated_token_behavior.selection().to_bytes();
    let output = RationalLifecycleProgramSetV5 {
        token_behavior_selection: selection,
        token_behavior_selection_id: hash(&selection).to_bytes(),
        program_set_id: hash(&program_set).to_bytes(),
        program_set,
    };
    validate_rational_lifecycle_program_set_v5(&output, input)?;
    Ok(output)
}

/// Hostile-decode and bind one ProgramSet to all selected descriptors.
pub fn validate_rational_lifecycle_program_set_v5(
    value: &RationalLifecycleProgramSetV5,
    input: RationalLifecycleProgramSetInputV5<'_>,
) -> Result<()> {
    validate_input(input)?;
    let expected_selection = input.authenticated_token_behavior.selection().to_bytes();
    if value.token_behavior_selection != expected_selection
        || value.token_behavior_selection_id != hash(&expected_selection).to_bytes()
        || value.program_set_id != hash(&value.program_set).to_bytes()
    {
        return Err(Error::ContentIdentity);
    }
    let decoded =
        CapabilityProgramSetV2::decode(&value.program_set).map_err(|_| Error::ArtifactGeometry)?;
    if decoded.selector_offset()
        != u32::try_from(RationalLifecycleHotLayoutV3::ACTION)
            .map_err(|_| Error::ArtifactGeometry)?
        || decoded.selector_width() != SelectorWidthV2::U8
        || decoded.entry_count() != 4
    {
        return Err(Error::ArtifactGeometry);
    }
    for (ordinal, expected) in [
        entry(input.activate_receipt, LifecycleActionV2::ActivateReceipt)?,
        entry(
            input.activate_coordinate,
            LifecycleActionV2::ActivateCoordinate,
        )?,
        entry(input.retire_coordinate, LifecycleActionV2::RetireCoordinate)?,
        compact_entry(input.retire_receipt)?,
    ]
    .iter()
    .enumerate()
    {
        if decoded
            .entry(u16::try_from(ordinal).map_err(|_| Error::ArtifactGeometry)?)
            .map_err(|_| Error::ArtifactGeometry)?
            != *expected
        {
            return Err(Error::ArtifactGeometry);
        }
    }
    Ok(())
}

fn validate_input(input: RationalLifecycleProgramSetInputV5<'_>) -> Result<()> {
    for (bundle, action) in [
        (input.activate_receipt, LifecycleActionV2::ActivateReceipt),
        (
            input.activate_coordinate,
            LifecycleActionV2::ActivateCoordinate,
        ),
        (input.retire_coordinate, LifecycleActionV2::RetireCoordinate),
    ] {
        validate_rational_lifecycle_selected_bundle_for_authenticated_selection_v5(
            bundle,
            input.authenticated_token_behavior,
        )?;
        if bundle.action != action {
            return Err(Error::ActionGeometry);
        }
    }
    validate_rational_lifecycle_compact_bundle_for_authenticated_selection_v4(
        input.retire_receipt,
        input.authenticated_token_behavior,
    )?;
    Ok(())
}

fn entry(
    bundle: &RationalLifecycleSelectedBundleV5,
    action: LifecycleActionV2,
) -> Result<CapabilityProgramSetEntryV2> {
    if bundle.action != action {
        return Err(Error::ActionGeometry);
    }
    Ok(CapabilityProgramSetEntryV2::new(
        u32::from(action.tag()),
        descriptor_reference(&bundle.descriptor)?,
    ))
}

fn compact_entry(bundle: &RationalLifecycleCompactBundleV4) -> Result<CapabilityProgramSetEntryV2> {
    Ok(CapabilityProgramSetEntryV2::new(
        u32::from(LifecycleActionV2::RetireReceipt.tag()),
        descriptor_reference(&bundle.descriptor)?,
    ))
}

fn descriptor_reference(bytes: &[u8]) -> Result<CapabilityDescriptorReferenceV2> {
    Ok(CapabilityDescriptorReferenceV2::new(
        ContentId::new(CAPABILITY_PROGRAM_SCHEMA_ID_V4).map_err(|_| Error::ContentIdentity)?,
        ContentId::new(hash(bytes).to_bytes()).map_err(|_| Error::ContentIdentity)?,
    ))
}
