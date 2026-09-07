//! Series selected-capability publication payload.

use dclutch_market::capability_activation::{
    activation_account_profile_schema_v1, activation_effect_schema_v1,
};
use dclutch_market::capability_program::{
    set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4,
};
use dclutch_operator::series_current_source_v1::SeriesFoundPrepareActivationV1;
use dclutch_trading::series::request::SeriesActionV3;
use dclutch_trading_sbf::series::activation_bundle_v1::{
    series_activation_descriptor_schema_v1, series_activation_funding_plan_v1,
};
use dclutch_vm::{
    account_profile::v3::SCHEMA_RELEASE_ID_V3 as ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V3,
    effect::v5::SCHEMA_RELEASE_ID_V5 as EFFECT_SCHEMA_RELEASE_ID_V5,
    request_profile::SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_RELEASE_ID_V1,
    v3::SCHEMA_RELEASE_ID as TRANSITION_SCHEMA_RELEASE_ID_V3,
};

use crate::{
    Error, Result,
    model::{SelectedCapabilityRecordV1, SelectedCapabilityV1},
    plan::hex,
};

/// Serialize an activation-capable Series release for the generic Market seam.
pub(crate) fn series_selected_capability_payload_v1(
    compiled: &SeriesFoundPrepareActivationV1,
    template_bytes: &[u8],
    exact_root_rent: u64,
    activation_deadline_slot: u64,
    selected_manifest_entry_index: u16,
) -> Result<SelectedCapabilityV1> {
    let template = dclutch_trading::series::TemplateV3::decode(template_bytes)
        .map_err(|_| Error::new("Series payload Template refused hostile decode"))?;
    let funding = series_activation_funding_plan_v1(template, exact_root_rent)
        .map_err(|error| Error::new(format!("Series activation funding: {error:?}")))?;
    if funding.creation_compartment() == 0 {
        return Err(Error::new("Series Template declared zero close principal"));
    }
    let mut records = Vec::new();
    for (index, action) in [
        SeriesActionV3::Prepare,
        SeriesActionV3::Consume,
        SeriesActionV3::Expire,
        SeriesActionV3::Retire,
        SeriesActionV3::Close,
    ]
    .into_iter()
    .enumerate()
    {
        let artifact = compiled.source().source().action_artifacts(action);
        let label = ["prepare", "consume", "expire", "retire", "close"][index];
        records.extend([
            record(
                format!("series_{label}_account_profile_record"),
                ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V3,
                artifact.account_profile(),
            ),
            record(
                format!("series_{label}_request_profile_record"),
                REQUEST_PROFILE_SCHEMA_RELEASE_ID_V1,
                artifact.request_profile(),
            ),
            record(
                format!("series_{label}_lifecycle_record"),
                dclutch_vm::account_profile::lifecycle_v3::CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5,
                artifact.lifecycle(),
            ),
            record(
                format!("series_{label}_strategy_record"),
                dclutch_market::execution_strategy::v2::EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                &compiled.release().strategies[index],
            ),
            record(
                format!("series_{label}_transition_record"),
                TRANSITION_SCHEMA_RELEASE_ID_V3,
                artifact.transition(),
            ),
            record(
                format!("series_{label}_effect_record"),
                EFFECT_SCHEMA_RELEASE_ID_V5,
                artifact.effect(),
            ),
            record(
                format!("series_{label}_descriptor_record"),
                CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4,
                &compiled.release().descriptors[index],
            ),
        ]);
    }
    let activation = compiled.activation();
    records.extend([
        record(
            "series_activation_account_profile_record".into(),
            activation_account_profile_schema_v1(),
            &activation.account_profile,
        ),
        record(
            "series_activation_effect_record".into(),
            activation_effect_schema_v1(),
            &activation.effect,
        ),
        record(
            "series_activation_descriptor_record".into(),
            series_activation_descriptor_schema_v1(),
            &activation.descriptor,
        ),
        record(
            "series_program_set_record".into(),
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            compiled.activation_program_set(),
        ),
    ]);
    Ok(SelectedCapabilityV1 {
        family: "series".into(),
        program_set_hex: hex(compiled.activation_program_set()),
        selected_descriptor_hex: hex(&compiled.release().descriptors[0]),
        config_hex: hex(template_bytes),
        publication_hex: hex(compiled.activation_program_set()),
        records,
        activation_deadline_slot,
        root_rent_minimum_lamports: funding.rent_compartment(),
        creation_principal_lamports: funding.creation_compartment(),
        selected_manifest_entry_index,
    })
}

fn record(label: String, schema: [u8; 32], body: &[u8]) -> SelectedCapabilityRecordV1 {
    SelectedCapabilityRecordV1 {
        label,
        schema_hex: hex(&schema),
        body_hex: hex(body),
    }
}
