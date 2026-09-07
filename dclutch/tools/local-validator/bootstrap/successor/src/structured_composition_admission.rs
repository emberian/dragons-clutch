//! Same-finalized Structured composition admission after Registry publication.
//!
//! Publishing bytes is not admission.  This adapter reacquires the four
//! Product records, four composition records, and Rational execution descriptor
//! alongside both executable programs and the Rent sysvar, then lends the
//! complete observation to the pure operator.

use dclutch_claims::composition::{
    COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3, COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
    COMPOSITION_GRAPH_SCHEMA_ID_V3, COMPOSITION_TRANSLATION_SCHEMA_ID_V3,
};
use dclutch_operator::{
    ObservedAccount,
    observation::decode_rent,
    representation_composition::{
        CompositionAdmissionPlanV3, CompositionChainObservationV3, FinalizedRecordObservationV3,
        ProductCompositionObservationV3, RepresentationCompositionObservationV3,
        build_composition_admission_plan_v3,
    },
};
use dclutch_product::admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3;
use solana_sdk::pubkey::Pubkey;
use solana_sdk_ids::sysvar;

use crate::{
    Error, Result,
    campaign::CampaignTerminalEvidenceV1,
    plan::{hex32, pubkey},
    rpc::Rpc,
    runtime::PublishedRecord,
};

/// The exact account order held by a reauthenticated composition snapshot.
const RECORD_COUNT_V1: usize = 9;
const PROGRAMS_V1: usize = 2;
const RECORD_START_V1: usize = PROGRAMS_V1 + 1;

#[derive(Clone, Copy, Debug)]
struct RecordPairV1 {
    schema: [u8; 32],
    raw: Pubkey,
    staging: Pubkey,
}

/// Owned records whose borrows can build the operator's admission plan.
pub(crate) struct StructuredCompositionAdmissionSnapshotV1 {
    registry: Pubkey,
    claims: Pubkey,
    records: [RecordPairV1; RECORD_COUNT_V1],
    accounts: Vec<ObservedAccount>,
    slot: u64,
}

impl StructuredCompositionAdmissionSnapshotV1 {
    /// One finalized slot shared by every account in the admitted plan.
    pub(crate) const fn slot(&self) -> u64 {
        self.slot
    }

    /// Build the pure composition admission plan from this exact snapshot.
    pub(crate) fn plan(&self) -> Result<CompositionAdmissionPlanV3<'_>> {
        let rent = decode_rent(self.account(2)?)
            .map_err(|error| Error::new(format!("Structured composition Rent: {error:?}")))?;
        let record = |index| self.record(index, &rent);
        let observed = CompositionChainObservationV3 {
            registry_program: self.account(0)?,
            claims_program: self.account(1)?,
            product: ProductCompositionObservationV3 {
                product: record(0)?,
                result_domain: record(1)?,
                portfolio: record(2)?,
                product_basis: record(3)?,
            },
            representation: RepresentationCompositionObservationV3 {
                execution_descriptor: record(8)?,
                descriptor: record(4)?,
                graph: record(5)?,
                translation: record(6)?,
                exposure: record(7)?,
            },
        };
        build_composition_admission_plan_v3(observed)
            .map_err(|error| Error::new(format!("Structured composition admission: {error:?}")))
    }

    fn account(&self, index: usize) -> Result<&ObservedAccount> {
        self.accounts
            .get(index)
            .ok_or_else(|| Error::new("Structured composition snapshot width changed"))
    }

    fn record(
        &self,
        index: usize,
        rent: &solana_program::rent::Rent,
    ) -> Result<FinalizedRecordObservationV3<'_>> {
        let pair = self
            .records
            .get(index)
            .ok_or_else(|| Error::new("Structured composition record index differs"))?;
        let offset = RECORD_START_V1 + index * 2;
        let raw = self.account(offset)?;
        let staging = self.account(offset + 1)?;
        if raw.key != pair.raw || staging.key != pair.staging {
            return Err(Error::new(
                "Structured composition record coordinate changed in snapshot",
            ));
        }
        Ok(FinalizedRecordObservationV3 {
            schema_id: pair.schema,
            raw,
            staging,
            raw_rent_minimum: rent.minimum_balance(raw.data.len()),
        })
    }
}

/// Reacquire every composition input in one finalized RPC response.
pub(crate) fn hydrate_structured_composition_admission_v1(
    rpc: &mut Rpc,
    registry: Pubkey,
    claims: Pubkey,
    evidence: &CampaignTerminalEvidenceV1,
    published: &[PublishedRecord; 7],
    minimum_slot: u64,
) -> Result<StructuredCompositionAdmissionSnapshotV1> {
    let founding = [
        report_record(
            registry,
            evidence,
            "product_record",
            PRODUCT_RECORD_SCHEMA_ID_V2,
        )?,
        report_record(
            registry,
            evidence,
            "result_domain_record",
            RESULT_DOMAIN_SCHEMA_ID_V2,
        )?,
        report_record(
            registry,
            evidence,
            "portfolio_record",
            PORTFOLIO_SCHEMA_ID_V2,
        )?,
        report_record(
            registry,
            evidence,
            "linked_liability_basis_record",
            GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        )?,
    ];
    let composition = [
        pair_from_published(published, 0, COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3)?,
        pair_from_published(published, 1, COMPOSITION_GRAPH_SCHEMA_ID_V3)?,
        pair_from_published(published, 2, COMPOSITION_TRANSLATION_SCHEMA_ID_V3)?,
        pair_from_published(published, 3, COMPOSITION_EXPOSURE_SCHEMA_ID_V3)?,
    ];
    let execution = pair_from_published(
        published,
        6,
        dclutch_claims::rational_kernel::REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
    )?;
    let records: [RecordPairV1; RECORD_COUNT_V1] = [
        founding[0],
        founding[1],
        founding[2],
        founding[3],
        composition[0],
        composition[1],
        composition[2],
        composition[3],
        execution,
    ];
    let mut addresses = vec![registry, claims, sysvar::rent::ID];
    addresses.extend(records.iter().flat_map(|pair| [pair.raw, pair.staging]));
    let (observation, accounts) =
        rpc.finalized_observed_accounts_admitting_vacant(&addresses, minimum_slot)?;
    if accounts.len() != addresses.len() {
        return Err(Error::new(
            "Structured composition finalized snapshot changed width",
        ));
    }
    if accounts[0].key != registry
        || accounts[1].key != claims
        || accounts[2].key != sysvar::rent::ID
    {
        return Err(Error::new(
            "Structured composition program or Rent coordinate changed",
        ));
    }
    Ok(StructuredCompositionAdmissionSnapshotV1 {
        registry,
        claims,
        records,
        accounts,
        slot: observation.slot,
    })
}

fn report_record(
    registry: Pubkey,
    evidence: &CampaignTerminalEvidenceV1,
    label: &str,
    schema: [u8; 32],
) -> Result<RecordPairV1> {
    let row = evidence
        .accounts
        .get(label)
        .ok_or_else(|| Error::new(format!("Structured composition report omitted {label}")))?;
    let digest = hex32(&row.data_sha256)?;
    let (raw, staging) =
        crate::structured_claims_producer::record_coordinates_v1(registry, schema, digest)?;
    if pubkey(&row.address)? != raw {
        return Err(Error::new(format!(
            "Structured composition report {label} address differs from its Registry coordinate"
        )));
    }
    Ok(RecordPairV1 {
        schema,
        raw,
        staging,
    })
}

fn pair_from_published(
    published: &[PublishedRecord; 7],
    index: usize,
    schema: [u8; 32],
) -> Result<RecordPairV1> {
    let row = published
        .get(index)
        .ok_or_else(|| Error::new("Structured composition publication record is absent"))?;
    if row.schema != schema {
        return Err(Error::new(
            "Structured composition publication schema differs from canonical order",
        ));
    }
    Ok(RecordPairV1 {
        schema,
        raw: row.raw,
        staging: row.staging,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_order_refuses_a_same_width_wrong_schema() {
        let records = std::array::from_fn(|index| PublishedRecord {
            schema: if index == 0 { [9; 32] } else { [8; 32] },
            digest: [7; 32],
            raw: Pubkey::new_from_array([index as u8 + 1; 32]),
            staging: Pubkey::new_from_array([index as u8 + 9; 32]),
        });
        let error = pair_from_published(&records, 0, COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3)
            .expect_err("wrong first schema must not become a descriptor");
        assert_eq!(
            error.to_string(),
            "Structured composition publication schema differs from canonical order"
        );
    }
}
