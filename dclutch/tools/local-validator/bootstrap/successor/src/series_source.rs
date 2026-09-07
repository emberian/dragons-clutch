//! Typed assembly of the current Series source for a local-validator founder.
//!
//! This is the host boundary between finalized chain observations and the
//! canonical Series release owners. It accepts neither a caller-authored
//! request byte bank nor generated example records: immutable Template,
//! occurrence and Ticket evidence are re-admitted by the producer, while the
//! opaque child bank comes from `dclutch_operator`'s semantic owners.

use dclutch_core_contract::ContentId;
use dclutch_operator::{
    series_child_bank_v1::SeriesChildBankV1,
    series_current_source_v1::{
        SeriesFoundPrepareActivationV1, SeriesFoundPrepareInputV1,
        construct_series_found_prepare_activation_v1,
    },
    series_lifecycle_v3::SeriesLifecycleSnapshotV3,
};
use dclutch_trading::series::{AccountKeyV3, AuthenticatedProductProjectionV2, TemplateV3};
use dclutch_trading_sbf::series::{
    account_profile_v4::SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4,
    custody_v3::SeriesCustodyPhysicalV3,
    expire_funding_artifacts_v5::{
        SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5, SeriesExpireAccountProfileInputV5,
    },
    prepare_funding_artifacts_v5::{
        SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5, SeriesPrepareAccountProfileInputV5,
    },
    projected_custody_v3::SeriesProjectedCustodyPhysicalV3,
    release_v5::SeriesCurrentReleaseInputV5,
};

use crate::{
    Error, Result, model::SelectedCapabilityV1,
    series_market::series_selected_capability_payload_v1,
};

/// Finalized data-width and Rent observations used only by the current
/// account-profile owners. The fixed arrays retain the protocol-owned
/// coordinate order; each value is the observed account length at that
/// coordinate, never a ProgramTest default.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SeriesObservedGeometryV1 {
    pub(crate) prepare_fixed_data_lengths: [u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize],
    pub(crate) prepare_ticket_rent_lamports: u64,
    pub(crate) consume_fixed_data_lengths: [u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
    pub(crate) consume_funding_count: u32,
    pub(crate) expire_fixed_data_lengths: [u32; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize],
}

impl SeriesObservedGeometryV1 {
    /// Hand the Prepare profile owner its coordinate-ordered observations.
    fn prepare_profile(&self) -> SeriesPrepareAccountProfileInputV5<'_> {
        SeriesPrepareAccountProfileInputV5 {
            fixed_data_lengths: &self.prepare_fixed_data_lengths,
        }
    }

    /// Hand the Consume profile owner its fixed-coordinate observations.
    fn consume_widths(&self) -> &[u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4] {
        &self.consume_fixed_data_lengths
    }

    /// Hand the Expire profile owner its coordinate-ordered observations.
    fn expire_profile(&self) -> SeriesExpireAccountProfileInputV5<'_> {
        SeriesExpireAccountProfileInputV5 {
            fixed_data_lengths: &self.expire_fixed_data_lengths,
        }
    }
}

/// Authenticated observations and semantic-owner output required to publish
/// the Series-selected capability. `children` is opaque: callers can only
/// obtain it from `SeriesChildBankV1::produce`, which derives each child
/// request from its owner and the admitted immutable records.
pub(crate) struct SeriesSourceAssemblyV1<'a> {
    /// Finalized Template, current occurrence/Ticket proof, root replay, slot
    /// and exact root Rent observation.
    pub(crate) lifecycle: SeriesLifecycleSnapshotV3<'a>,
    /// Product facts authenticated by the immutable occurrence.
    pub(crate) product: AuthenticatedProductProjectionV2,
    /// Current Registry identity used by the canonical future-Market plan.
    pub(crate) registry_program: AccountKeyV3,
    /// Current normal-Custody physical observations for the Prepare parent.
    pub(crate) custody: SeriesCustodyPhysicalV3,
    /// Current projected-Custody physical observations for that same parent.
    pub(crate) projected_custody: SeriesProjectedCustodyPhysicalV3,
    /// Complete child requests produced by their semantic owners.
    pub(crate) children: SeriesChildBankV1,
    /// Current observed widths and Rent targets in the protocol-owned frames.
    pub(crate) geometry: SeriesObservedGeometryV1,
    /// Current selected Consume accelerator certificate program.
    pub(crate) consume_shadow_certificate_program: ContentId,
    /// Current selected release committed by the immutable Template.
    pub(crate) selected_release: ContentId,
    /// Current ledger geometry selected for the activation bundle.
    pub(crate) funding_ledger_slot_count: u16,
    /// Current activation expiry selected by the founder plan.
    pub(crate) activation_deadline_slot: u64,
    /// Current Registry entry that selects this capability.
    pub(crate) selected_manifest_entry_index: u16,
}

/// Canonical Series source and the generic selected-capability publication
/// payload derived from exactly the same finalized evidence.
pub(crate) struct SeriesSelectedSourceV1 {
    pub(crate) activation: SeriesFoundPrepareActivationV1,
    pub(crate) selected: SelectedCapabilityV1,
}

/// Build the complete V5 source, Found→Prepare activation, and selected
/// capability payload from typed observations.
///
/// The Template release selection is checked before the opaque bank enters
/// the canonical source constructor. That constructor re-admits the
/// Template/occurrence/Ticket and compares its canonical Prepare custody
/// projection against this bank, making a substituted immutable record fail
/// before Registry publication.
pub(crate) fn assemble_series_selected_source_v1(
    input: SeriesSourceAssemblyV1<'_>,
) -> Result<SeriesSelectedSourceV1> {
    let template = selected_template_v1(input.lifecycle.template_bytes, input.selected_release)?;
    validate_observed_rent_v1(
        input.lifecycle.observed_root_lamports,
        input.lifecycle.exact_root_rent,
        input.geometry.prepare_ticket_rent_lamports,
        input.geometry.consume_funding_count,
    )?;

    let release = SeriesCurrentReleaseInputV5 {
        template: dclutch_trading::series::template_content_id(input.lifecycle.template_bytes)
            .map_err(|_| Error::new("Series source Template content identity refused"))?,
        template_occurrence_count: template.occurrence_count(),
        consume_shadow_certificate_program: input.consume_shadow_certificate_program,
        prepare_profile: input.geometry.prepare_profile(),
        prepare_requests: input.children.prepare_requests(),
        prepare_ticket_rent_lamports: input.geometry.prepare_ticket_rent_lamports,
        consume_observed_data_lengths: input.geometry.consume_widths(),
        consume_requests: input.children.consume_requests(),
        consume_funding_count: input.geometry.consume_funding_count,
        expire_profile: input.geometry.expire_profile(),
        expire_requests: input.children.expire_requests(),
    };
    let activation = construct_series_found_prepare_activation_v1(
        SeriesFoundPrepareInputV1 {
            lifecycle: input.lifecycle,
            product: input.product,
            registry_program: input.registry_program,
            custody: input.custody,
            projected_custody: input.projected_custody,
            release,
        },
        input.funding_ledger_slot_count,
    )
    .map_err(|error| Error::new(format!("Series source construction refused: {error:?}")))?;
    let selected = series_selected_capability_payload_v1(
        &activation,
        input.lifecycle.template_bytes,
        input.lifecycle.exact_root_rent,
        input.activation_deadline_slot,
        input.selected_manifest_entry_index,
    )?;
    Ok(SeriesSelectedSourceV1 {
        activation,
        selected,
    })
}

/// Authenticate the selected release against the immutable Template before
/// any semantic child output can become a Registry record.
fn selected_template_v1(template_bytes: &[u8], selected_release: ContentId) -> Result<TemplateV3> {
    let template = TemplateV3::decode(template_bytes)
        .map_err(|_| Error::new("Series source Template refused hostile decode"))?;
    if template.release_set() != selected_release {
        return Err(Error::new(
            "Series source selected release differs from immutable Template",
        ));
    }
    Ok(template)
}

/// Refuse absent or underfunded same-snapshot Rent observations before they
/// are handed to the V5 profile and activation owners.
fn validate_observed_rent_v1(
    observed_root_lamports: u64,
    exact_root_rent: u64,
    ticket_rent_lamports: u64,
    consume_funding_count: u32,
) -> Result<()> {
    if exact_root_rent == 0 || observed_root_lamports < exact_root_rent {
        return Err(Error::new(
            "Series source root Rent observation was absent or underfunded",
        ));
    }
    if ticket_rent_lamports == 0 {
        return Err(Error::new("Series source Ticket Rent observation was zero"));
    }
    if consume_funding_count == 0 {
        return Err(Error::new(
            "Series source Consume funding observation was zero",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_core_contract::ContentId;
    use dclutch_trading::series::{
        AccountKeyV3,
        encode::{TemplateRecordInputV3, encode_template_v3},
    };

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero content identity")
    }

    fn two_occurrence_template() -> [u8; dclutch_trading::series::SERIES_TEMPLATE_BYTES_V3] {
        encode_template_v3(TemplateRecordInputV3 {
            realm: id(1),
            release_set: id(2),
            product_generator: id(3),
            occurrence_generator: id(4),
            capability_template: id(5),
            product_derivation: id(6),
            occurrence_derivation: id(7),
            capability_derivation: id(8),
            funding_derivation: id(9),
            projection_root: id(10),
            refund_owner: AccountKeyV3::new([11; 32]).expect("nonzero refund owner"),
            occurrence_count: 2,
            first_slot: 100,
            period_slots: 10,
            retry_window: 1,
            close_rent: 1,
        })
        .expect("two occurrence Template")
    }

    #[test]
    fn accepts_two_occurrence_template_only_with_its_selected_release() {
        let template = two_occurrence_template();
        let admitted = selected_template_v1(&template, id(2)).expect("accepted selection");
        assert_eq!(admitted.occurrence_count(), 2);
        assert_eq!(admitted.release_set(), id(2));
    }

    #[test]
    fn substituted_selected_release_refuses_before_child_bank_use() {
        let template = two_occurrence_template();
        assert_eq!(
            selected_template_v1(&template, id(12))
                .expect_err("substituted selected release must refuse")
                .to_string(),
            "Series source selected release differs from immutable Template"
        );
    }

    #[test]
    fn absent_or_underfunded_rent_observations_refuse() {
        assert_eq!(
            validate_observed_rent_v1(4, 5, 1, 1)
                .expect_err("underfunded root must refuse")
                .to_string(),
            "Series source root Rent observation was absent or underfunded"
        );
        assert_eq!(
            validate_observed_rent_v1(5, 5, 0, 1)
                .expect_err("zero Ticket rent must refuse")
                .to_string(),
            "Series source Ticket Rent observation was zero"
        );
    }
}
