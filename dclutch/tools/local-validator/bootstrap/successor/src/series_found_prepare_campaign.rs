//! Physical materialization for the bounded Series Found -> Prepare campaign.
//!
//! This adapter derives addresses only from the selected programs, admitted
//! Series identities and the chain's current Rent quote.  It deliberately
//! does not create or preseed a future child account.  The caller observes
//! each derived vacant coordinate before giving these facts to the canonical
//! Series/Custody/Claims projection owners.

use dclutch_claims::{
    founding_v5::ClaimsFoundingAggregateSeedsV5,
    protocol_position_v2::{ProtocolPositionAdmissionSeedsV2, ProtocolPositionSeedsV2},
};
use dclutch_custody::{
    CallerRoleV1, CompartmentV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyVaultSeedsV1, ProjectedCustodyStateSeedsV2, SOURCE_COMPARTMENT_REPLAY_REVISION_V1,
};
use dclutch_market::{Action, ProjectFoundReceiptV2, Request};
use dclutch_market::{Identity, SeriesFoundingPermitSeedsV1};
use dclutch_operator::series_lifecycle_v3::{
    SeriesLifecycleSnapshotV3, SeriesNextActV3, inspect_series_lifecycle_v3,
};
use dclutch_operator::{
    series_child_bank_v1::{SeriesChildBankInputV1, SeriesChildBankV1, SeriesClaimsPhysicalV1},
    series_founding_children_v1::{
        SeriesFoundingChildrenInputV1, SeriesFoundingClaimsPhysicalV1,
        SeriesProjectedCustodyPhysicalInputV1, derive_series_founding_children_v1,
    },
};
use dclutch_trading::series::replay::TicketStateV3;
use dclutch_trading::series::{
    AccountKeyV3, AuthenticatedProductProjectionV2, TemplateV3, admit_occurrence, admit_ticket,
    pre_founding_series_escrow,
};
use dclutch_trading_sbf::series::{
    custody_v3::SeriesCustodyPhysicalV3,
    operator::{SeriesOccurrenceSnapshotV3, build_expire_v3},
    projected_custody_v3::SeriesProjectedCustodyPhysicalV3,
};
use sha2::{Digest, Sha256};
use solana_program::rent::Rent;
use solana_sdk::pubkey::Pubkey;

use crate::{Error, Result};

/// Inputs whose coordinates are observed before the child Market exists.  The
/// selected capability compiler projects the missing Core/Custody state from
/// these facts; no member is an asserted future account state.
pub(crate) struct SeriesFoundPrepareSelectionInputV1<'a> {
    pub(crate) lifecycle: SeriesLifecycleSnapshotV3<'a>,
    pub(crate) product: AuthenticatedProductProjectionV2,
    pub(crate) registry_program: AccountKeyV3,
    pub(crate) material: SeriesPhysicalMaterialInputV1,
    pub(crate) core_product_graph: [([u8; 32], [u8; 32]); 4],
    pub(crate) core_projection: crate::core_bump_projection::CoreProductGraphProjectionV1,
    pub(crate) core_walk: crate::market::CoreProductGraphWalkV1,
    /// M0 Source-owner principal-cap projection carried by the canonical
    /// future-Market publisher.
    pub(crate) principal_cap_sets: u64,
    pub(crate) linked_basis_record_digest: [u8; 32],
    pub(crate) semantic_basis_id: [u8; 32],
    /// Exact M0 Portfolio coefficient count, carried to the Claims V6 child
    /// because its appended failure escrow derives from the runtime width.
    pub(crate) claim_count: u32,
    pub(crate) claims_rent_principals: [u64; 3],
    pub(crate) permit_bump: u8,
    pub(crate) projected_bump: u8,
    /// Same-snapshot amount in the founder-owned collateral source.  The
    /// compiler derives every Custody receipt commitment from this observation
    /// and the canonical request transitions.
    pub(crate) founder_source_amount: u64,
    /// Present only when emitting immutable selected artifacts.  The typed
    /// preprofile derives the child-bank first so a live hydrator can observe
    /// its account geometry without inventing a placeholder profile.
    pub(crate) geometry: Option<crate::series_source::SeriesObservedGeometryV1>,
    pub(crate) selected_release: dclutch_core_contract::ContentId,
    pub(crate) funding_ledger_slot_count: u16,
    pub(crate) activation_deadline_slot: u64,
    pub(crate) selected_manifest_entry_index: u16,
    pub(crate) ticket_state_account: AccountKeyV3,
}

/// Complete selected payload and the exact parent request bank it binds.
pub(crate) struct CompiledSeriesFoundPrepareSelectionV1 {
    pub(crate) parents: SeriesPrepareParentsV1,
    pub(crate) predicted_core: dclutch_market::CoreState,
    /// Canonical child requests before they are embedded in the selected
    /// artifact closure.  The live geometry hydrator derives the 111 Prepare
    /// account observations from this semantic-owner output before a caller
    /// publishes the selected bytes.
    pub(crate) prepare_children: SeriesChildBankV1,
    pub(crate) selected: crate::model::SelectedCapabilityV1,
    /// Private occurrence-derived words that seed the selected Prepare interpreter.
    pub(crate) derived_prepare:
        dclutch_trading_sbf::series::derived_prepare_v1::SeriesPrepareDerivedRequestsV1,
    pub(crate) consume_request: Vec<u8>,
    pub(crate) prepared_projected_state: dclutch_custody::ProjectedCustodyStateV2,
    pub(crate) prepared_source_replay: dclutch_custody::CustodyReplayV1,
    pub(crate) derived_consume:
        dclutch_trading_sbf::series::derived_terminal_v1::SeriesConsumeDerivedRequestsV1,
    pub(crate) derived_expire:
        dclutch_trading_sbf::series::derived_terminal_v1::SeriesExpireDerivedRequestsV1,
    pub(crate) physical: SeriesPhysicalMaterialV1,
}

/// Require the normalization boundary promised by the Series release owner:
/// changing the provisional parent root may change runtime authorities, but it
/// may not alter any immutable release byte that Registry will publish.
pub(crate) fn require_series_selection_invariance_v1(
    provisional: &CompiledSeriesFoundPrepareSelectionV1,
    actual: &CompiledSeriesFoundPrepareSelectionV1,
) -> Result<()> {
    let left = &provisional.selected;
    let right = &actual.selected;
    if left.program_set_hex != right.program_set_hex
        || left.selected_descriptor_hex != right.selected_descriptor_hex
        || left.config_hex != right.config_hex
        || left.publication_hex != right.publication_hex
        || left.records.len() != right.records.len()
    {
        return Err(Error::new(
            "Series normalized parent root changed immutable selection bytes",
        ));
    }
    for (first, second) in left.records.iter().zip(&right.records) {
        if first.label != second.label
            || first.schema_hex != second.schema_hex
            || first.body_hex != second.body_hex
        {
            return Err(Error::new(
                "Series normalized parent root changed an immutable publication record",
            ));
        }
    }
    Ok(())
}

/// Semantic child-bank facts derived before any account-profile bytes are
/// emitted.  The runtime hydrator consumes this output to map every Prepare
/// coordinate to a real address and observation, then hands the resulting
/// geometry back to [`compile_series_found_prepare_selection_v1`].
///
/// This split deliberately keeps a missing future account out of an
/// ``ObservedGeometry`` placeholder: the bank is already a semantic-owner
/// output, while the profile is necessarily a later validator observation.
pub(crate) struct SeriesFoundPreparePreprofileV1 {
    pub(crate) parents: SeriesPrepareParentsV1,
    pub(crate) predicted_core: dclutch_market::CoreState,
    pub(crate) prepare_children: SeriesChildBankV1,
    pub(crate) derived_prepare:
        dclutch_trading_sbf::series::derived_prepare_v1::SeriesPrepareDerivedRequestsV1,
    pub(crate) consume_request: Vec<u8>,
    pub(crate) prepared_projected_state: dclutch_custody::ProjectedCustodyStateV2,
    pub(crate) prepared_source_replay: dclutch_custody::CustodyReplayV1,
    pub(crate) derived_consume:
        dclutch_trading_sbf::series::derived_terminal_v1::SeriesConsumeDerivedRequestsV1,
    pub(crate) derived_expire:
        dclutch_trading_sbf::series::derived_terminal_v1::SeriesExpireDerivedRequestsV1,
    physical: SeriesPhysicalMaterialV1,
    projected_custody: SeriesProjectedCustodyPhysicalV3,
}

impl SeriesFoundPreparePreprofileV1 {
    /// Expose the canonical physical projection to the later Consume/Expire
    /// geometry owners.  Those owners may name only these semantic-owner
    /// coordinates; they do not reconstruct Custody or Claims addresses.
    pub(crate) const fn physical(&self) -> &SeriesPhysicalMaterialV1 {
        &self.physical
    }

    /// Exact projected-Custody physical state used to derive the child bank.
    pub(crate) const fn projected_custody(&self) -> SeriesProjectedCustodyPhysicalV3 {
        self.projected_custody
    }
}

/// Derive the current occurrence's parent requests, projected physical state,
/// and canonical child bank without requiring a caller-supplied width array.
pub(crate) fn derive_series_found_prepare_preprofile_v1(
    input: &mut SeriesFoundPrepareSelectionInputV1<'_>,
) -> Result<SeriesFoundPreparePreprofileV1> {
    let parents = derive_series_prepare_parents_v1(input.lifecycle)?;
    input.material.prepare_parent_digest = parents.prepare_digest;
    input.material.expire_parent_digest = parents.expire_digest;
    let current = input
        .lifecycle
        .current
        .ok_or_else(|| Error::new("Series selection compiler omitted first occurrence evidence"))?;
    let occurrence = admit_occurrence(
        input.lifecycle.template_bytes,
        current.occurrence_bytes,
        current.siblings,
    )
    .map_err(|_| Error::new("Series selection occurrence refused re-admission"))?;
    let ticket = admit_ticket(current.ticket_bytes)
        .map_err(|_| Error::new("Series selection Ticket refused re-admission"))?;
    let escrow =
        pre_founding_series_escrow(occurrence, ticket, input.product, input.registry_program)
            .map_err(|_| Error::new("Series selection future Market projection refused"))?;
    let identity = escrow.future_market().identity();
    let found = Request::administrative(Action::Found, escrow.generation(), identity.market_id)
        .encode()
        .map_err(|error| Error::new(format!("Series Core Found request: {error:?}")))?;
    let receipt = ProjectFoundReceiptV2::new(
        identity.market_id,
        escrow.generation(),
        identity.realm_id,
        Identity::new(input.material.mint.to_bytes())
            .map_err(|_| Error::new("Series collateral mint identity"))?,
        Identity::new(input.material.token_program.to_bytes())
            .map_err(|_| Error::new("Series token program identity"))?,
        input.material.collateral_release,
        identity.product_record,
        identity.product_id,
        identity.resolution_policy,
        identity.selected_release_set,
        Identity::new(input.material.rent_program.to_bytes())
            .map_err(|_| Error::new("Series Rent program identity"))?,
        input.principal_cap_sets,
        Sha256::digest(found).into(),
    )
    .map_err(|error| Error::new(format!("Series ProjectFound receipt: {error:?}")))?;
    let receipt_digest: [u8; 32] =
        Sha256::digest(receipt.encode().map_err(|error| {
            Error::new(format!("Series ProjectFound receipt encoding: {error:?}"))
        })?)
        .into();
    input.material.projection_receipt_digest = receipt_digest;
    let predicted_core =
        crate::market::predict_core_state_v1(crate::market::PredictedCoreStateInputV1 {
            core: input.material.core,
            registry: Pubkey::new_from_array(input.registry_program.to_bytes()),
            identity,
            product_graph: input.core_product_graph,
            projection: input.core_projection,
            walk: input.core_walk,
            principal_cap_sets: input.principal_cap_sets,
            rent_beneficiary: Identity::new(input.material.rent_credit.to_bytes())
                .map_err(|_| Error::new("Series RentCredit identity"))?,
        })?;
    let physical = materialize_series_found_prepare_v1(input.material.clone())?;
    let expiry = occurrence
        .template()
        .retry_through(escrow.occurrence())
        .map_err(|_| Error::new("Series expiry schedule refused"))?;
    let initialize =
        dclutch_trading_sbf::series::projected_custody_v3::project_prepare_initialize_v3(
            escrow,
            expiry,
            physical.projected,
        )
        .map_err(|_| Error::new("Series projected initialize refused"))?;
    let init_digest: [u8; 32] = Sha256::digest(
        initialize
            .encode()
            .map_err(|_| Error::new("Series projected initialize encoding"))?,
    )
    .into();
    let projected = dclutch_custody::ProjectedCustodyStateV2::initialize(
        initialize,
        receipt,
        input.material.core.to_bytes(),
        receipt_digest,
        init_digest,
        input.lifecycle.now_slot,
        true,
        input.projected_bump,
    )
    .map_err(|_| Error::new("Series projected initialize state refused"))?;
    let open = dclutch_trading_sbf::series::projected_custody_v3::project_prepare_open_hoard_v3(
        escrow,
        expiry,
        physical.projected,
    )
    .map_err(|_| Error::new("Series projected open refused"))?;
    let open_digest: [u8; 32] = Sha256::digest(
        open.encode()
            .map_err(|_| Error::new("Series projected open encoding"))?,
    )
    .into();
    let projected = projected
        .open_hoard(open, open_digest, 0, true)
        .map_err(|_| Error::new("Series projected HoardOpen state refused"))?;
    let [normal_initialize, normal_open, normal_lock] =
        dclutch_trading_sbf::series::custody_v3::project_prepare_custody_v3(
            dclutch_trading::series::escrow::prepare_series_escrow_v3(escrow),
            physical.prepare,
        )
        .map_err(|_| Error::new("Series normal Prepare projection refused"))?;
    let init_request_digest = solana_program::hash::hash(
        &normal_initialize
            .to_bytes()
            .map_err(|_| Error::new("Series normal Initialize encoding"))?,
    )
    .to_bytes();
    let init_poststate = dclutch_custody::custody_poststate_commitment_v1(
        dclutch_custody::CustodyPoststateProjectionV1 {
            request_digest: init_request_digest,
            source: physical.normal_replay.to_bytes(),
            destination: physical.normal_replay.to_bytes(),
            source_before: 0,
            source_after: 0,
            destination_before: 0,
            destination_after: 0,
            rent_lamports: physical.prepare.replay_rent_lamports,
        },
    );
    let open_request_digest = solana_program::hash::hash(
        &normal_open
            .to_bytes()
            .map_err(|_| Error::new("Series normal Open encoding"))?,
    )
    .to_bytes();
    let open_poststate = dclutch_custody::custody_poststate_commitment_v1(
        dclutch_custody::CustodyPoststateProjectionV1 {
            request_digest: open_request_digest,
            source: physical.prepare.escrow_vault,
            destination: physical.prepare.escrow_vault,
            source_before: 0,
            source_after: 0,
            destination_before: 0,
            destination_after: 0,
            rent_lamports: physical.prepare.vault_rent_lamports,
        },
    );
    let source_after = input
        .founder_source_amount
        .checked_sub(escrow.hoard_principal())
        .ok_or_else(|| Error::new("Series founder collateral source was underfunded"))?;
    let lock_request_digest = solana_program::hash::hash(
        &normal_lock
            .to_bytes()
            .map_err(|_| Error::new("Series normal Lock encoding"))?,
    )
    .to_bytes();
    let lock_poststate = dclutch_custody::custody_poststate_commitment_v1(
        dclutch_custody::CustodyPoststateProjectionV1 {
            request_digest: lock_request_digest,
            source: input.material.founder_source.to_bytes(),
            destination: physical.prepare.escrow_vault,
            source_before: input.founder_source_amount,
            source_after,
            destination_before: 0,
            destination_after: escrow.hoard_principal(),
            rent_lamports: 0,
        },
    );
    let replay = dclutch_custody::CustodyReplayV1::initialize(
        normal_initialize,
        init_request_digest,
        init_poststate,
    )
    .and_then(|state| state.advance(normal_open, open_request_digest, open_poststate))
    .and_then(|state| state.advance(normal_lock, lock_request_digest, lock_poststate))
    .map_err(|_| Error::new("Series normal Custody replay projection refused"))?;
    let claims = SeriesFoundingClaimsPhysicalV1 {
        linked_basis_record_digest: input.linked_basis_record_digest,
        semantic_basis_id: input.semantic_basis_id,
        claim_count: input.claim_count,
        aggregate: physical.claims.aggregate.to_bytes(),
        position: physical.claims.position.to_bytes(),
        admission: physical.claims.admission.to_bytes(),
        claims_program: input.material.claims.to_bytes(),
        aggregate_rent_principal: input.claims_rent_principals[0],
        position_rent_principal: input.claims_rent_principals[1],
        admission_rent_principal: input.claims_rent_principals[2],
        observed_aggregate_lamports: physical.claims.aggregate_lamports,
        observed_position_lamports: physical.claims.position_lamports,
        observed_admission_lamports: physical.claims.admission_lamports,
        permit_bump: input.permit_bump,
        // This is the realized Market Hoard's fresh Trading replay, not the
        // Series escrow source `replay` above.  Realization rewrites the
        // projected state in place to this canonical cursor; Custody names
        // the same cursor for its founding source compartment.
        normal_replay_revision: SOURCE_COMPARTMENT_REPLAY_REVISION_V1,
    };
    let founding_input = SeriesFoundingChildrenInputV1 {
        template: input.lifecycle.template_bytes,
        occurrence: current.occurrence_bytes,
        siblings: current.siblings,
        ticket: current.ticket_bytes,
        product: input.product,
        registry_program: input.registry_program,
        projected: SeriesProjectedCustodyPhysicalInputV1 {
            physical: physical.projected,
        },
        principal_cap_sets: input.principal_cap_sets,
        projected_state: projected,
        source_replay: replay,
        source_replay_account: physical.normal_replay.to_bytes(),
        realized_hoard_replay_account: physical.realized_hoard_replay.to_bytes(),
        predicted_core_state: &predicted_core,
        parent_root: input.material.parent_root.to_bytes(),
        trading_program: input.material.trading.to_bytes(),
        rent_program: input.material.rent_program.to_bytes(),
        claims,
    };
    let children = derive_series_founding_children_v1(founding_input)
        .map_err(|error| Error::new(format!("Series founding children refused: {error:?}")))?;
    let after_prepare = input
        .lifecycle
        .series
        .prepare_ticket(input.lifecycle.series.revision())
        .map_err(|error| Error::new(format!("Series native Prepare replay: {error:?}")))?;
    let prepared_ticket = TicketStateV3::prepared(ticket.content_id());
    let consume_snapshot = SeriesOccurrenceSnapshotV3 {
        template_bytes: input.lifecycle.template_bytes,
        occurrence_bytes: current.occurrence_bytes,
        ticket_bytes: current.ticket_bytes,
        siblings: current.siblings,
        series: after_prepare,
        ticket_state: Some(prepared_ticket),
        now_slot: occurrence.occurrence().scheduled_slot(),
    };
    let consume_request = dclutch_trading_sbf::series::operator::build_consume_v3(consume_snapshot)
        .map_err(|error| Error::new(format!("Series native Consume family: {error:?}")))?
        .as_bytes()
        .to_vec();
    let derived_consume =
        dclutch_trading_sbf::series::derived_terminal_v1::derive_series_consume_requests_v1(
            dclutch_trading_sbf::series::derived_terminal_v1::SeriesConsumeDerivedRequestInputV1 {
                family_request: &consume_request,
                snapshot: consume_snapshot,
                founding: founding_input,
                ticket_state_account: input.ticket_state_account,
                permit_account: AccountKeyV3::new(physical.permit.to_bytes())
                    .map_err(|_| Error::new("Series permit identity"))?,
                principal_cap_sets: input.principal_cap_sets,
            },
        )
        .map_err(|error| Error::new(format!("Series derived Consume bank: {error:?}")))?;
    let derived_expire =
        dclutch_trading_sbf::series::derived_terminal_v1::derive_series_expire_requests_v1(
            dclutch_trading_sbf::series::derived_terminal_v1::SeriesExpireDerivedRequestInputV1 {
                family_request: &parents.expire_request,
                snapshot: SeriesOccurrenceSnapshotV3 {
                    now_slot: expiry
                        .checked_add(1)
                        .ok_or_else(|| Error::new("Series expiry slot overflow"))?,
                    ..consume_snapshot
                },
                product: input.product,
                registry_program: input.registry_program,
                parent_root: input.material.parent_root.to_bytes(),
                custody: physical.expire,
                projected: physical.projected,
                principal_cap_sets: input.principal_cap_sets,
            },
        )
        .map_err(|error| Error::new(format!("Series derived Expire bank: {error:?}")))?;
    let bank = SeriesChildBankV1::produce(SeriesChildBankInputV1 {
        template: input.lifecycle.template_bytes,
        occurrence: current.occurrence_bytes,
        siblings: current.siblings,
        ticket: current.ticket_bytes,
        product: input.product,
        registry_program: input.registry_program,
        prepare_custody: physical.prepare,
        expire_custody: physical.expire,
        projected_custody: children.projected_physical,
        claims_physical: SeriesClaimsPhysicalV1 {
            claims_program: input.material.claims.to_bytes(),
            custody_replay: physical.realized_hoard_replay.to_bytes(),
        },
        claims: children.claims,
        permit_expiry: children.permit_expiry,
        ticket_state_account: input.ticket_state_account,
        expected_series_revision: after_prepare.revision(),
        expected_ticket_revision: prepared_ticket.revision(),
    })
    .map_err(|error| Error::new(format!("Series child bank refused: {error:?}")))?;
    let derived_prepare =
        dclutch_trading_sbf::series::derived_prepare_v1::derive_series_prepare_requests_v1(
            dclutch_trading_sbf::series::derived_prepare_v1::SeriesPrepareDerivedRequestInputV1 {
                family_request: &parents.prepare_request,
                snapshot: SeriesOccurrenceSnapshotV3 {
                    template_bytes: input.lifecycle.template_bytes,
                    occurrence_bytes: current.occurrence_bytes,
                    ticket_bytes: current.ticket_bytes,
                    siblings: current.siblings,
                    series: input.lifecycle.series,
                    ticket_state: current.ticket_state,
                    now_slot: input.lifecycle.now_slot,
                },
                product: input.product,
                registry_program: input.registry_program,
                parent_root: input.material.parent_root.to_bytes(),
                custody: physical.prepare,
                projected_custody: children.projected_physical,
                principal_cap_sets: input.principal_cap_sets,
            },
        )
        .map_err(|error| Error::new(format!("Series derived Prepare bank: {error:?}")))?;
    Ok(SeriesFoundPreparePreprofileV1 {
        parents,
        predicted_core,
        prepare_children: bank,
        derived_prepare,
        consume_request,
        prepared_projected_state: projected,
        prepared_source_replay: replay,
        derived_consume,
        derived_expire,
        physical,
        projected_custody: children.projected_physical,
    })
}

/// Compile a Series-selected capability from admitted two-occurrence facts.
///
/// The function deliberately projects only the first occurrence.  The
/// immutable Template still commits both occurrence records, and the release
/// compiler owns the bounded bank for every action of that Template.
pub(crate) fn compile_series_found_prepare_selection_v1(
    mut input: SeriesFoundPrepareSelectionInputV1<'_>,
    consume_shadow_certificate_program: dclutch_core_contract::ContentId,
) -> Result<CompiledSeriesFoundPrepareSelectionV1> {
    let preprofile = derive_series_found_prepare_preprofile_v1(&mut input)?;
    let geometry = input.geometry.ok_or_else(|| {
        Error::new("Series selected compilation requires the hydrated Prepare geometry")
    })?;
    let assembled = crate::series_source::assemble_series_selected_source_v1(
        crate::series_source::SeriesSourceAssemblyV1 {
            lifecycle: input.lifecycle,
            product: input.product,
            registry_program: input.registry_program,
            custody: preprofile.physical.prepare,
            projected_custody: preprofile.projected_custody,
            children: preprofile.prepare_children.clone(),
            geometry,
            consume_shadow_certificate_program,
            selected_release: input.selected_release,
            funding_ledger_slot_count: input.funding_ledger_slot_count,
            activation_deadline_slot: input.activation_deadline_slot,
            selected_manifest_entry_index: input.selected_manifest_entry_index,
        },
    )?;
    Ok(CompiledSeriesFoundPrepareSelectionV1 {
        parents: preprofile.parents,
        predicted_core: preprofile.predicted_core,
        prepare_children: preprofile.prepare_children,
        selected: assembled.selected,
        derived_prepare: preprofile.derived_prepare,
        consume_request: preprofile.consume_request,
        prepared_projected_state: preprofile.prepared_projected_state,
        prepared_source_replay: preprofile.prepared_source_replay,
        physical: preprofile.physical,
        derived_consume: preprofile.derived_consume,
        derived_expire: preprofile.derived_expire,
    })
}

/// Exact parent family requests for the first Series Prepare and its only
/// possible terminal counterpart.  These bytes come from the lifecycle owner;
/// their digests are the values Custody binds into each child request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SeriesPrepareParentsV1 {
    pub(crate) prepare_request: Vec<u8>,
    pub(crate) prepare_digest: [u8; 32],
    pub(crate) expire_request: Vec<u8>,
    pub(crate) expire_digest: [u8; 32],
}

/// Derive both parent request digests from immutable Series evidence and the
/// canonical replay kernel.  The Expire request is projected only after the
/// Prepare transition; it is not presented as an observed account state.
pub(crate) fn derive_series_prepare_parents_v1(
    lifecycle: SeriesLifecycleSnapshotV3<'_>,
) -> Result<SeriesPrepareParentsV1> {
    let report = inspect_series_lifecycle_v3(lifecycle)
        .map_err(|error| Error::new(format!("Series Prepare lifecycle refused: {error:?}")))?;
    let SeriesNextActV3::Ready(prepare) = report.next() else {
        return Err(Error::new("Series lifecycle did not select Prepare"));
    };
    if prepare.action() != dclutch_trading_sbf::series::instruction::SeriesActionV3::Prepare {
        return Err(Error::new("Series lifecycle selected a non-Prepare action"));
    }
    let current = lifecycle
        .current
        .ok_or_else(|| Error::new("Series Prepare omitted current immutable evidence"))?;
    let occurrence = admit_occurrence(
        lifecycle.template_bytes,
        current.occurrence_bytes,
        current.siblings,
    )
    .map_err(|_| Error::new("Series Prepare occurrence refused re-admission"))?;
    let ticket = admit_ticket(current.ticket_bytes)
        .map_err(|_| Error::new("Series Prepare Ticket refused re-admission"))?;
    occurrence
        .require_ticket(ticket.ticket())
        .map_err(|_| Error::new("Series Prepare occurrence/Ticket join refused"))?;
    let template = TemplateV3::decode(lifecycle.template_bytes)
        .map_err(|_| Error::new("Series Prepare Template refused hostile decode"))?;
    let retry_slot = template
        .retry_through(occurrence.occurrence().occurrence())
        .map_err(|_| Error::new("Series Prepare retry schedule refused"))?
        .checked_add(1)
        .ok_or_else(|| Error::new("Series Prepare retry schedule cannot express Expire"))?;
    let after_prepare = lifecycle
        .series
        .prepare_ticket(lifecycle.series.revision())
        .map_err(|_| Error::new("Series Prepare replay transition refused"))?;
    let prepared_ticket = TicketStateV3::prepared(ticket.content_id());
    let expire = build_expire_v3(SeriesOccurrenceSnapshotV3 {
        template_bytes: lifecycle.template_bytes,
        occurrence_bytes: current.occurrence_bytes,
        ticket_bytes: current.ticket_bytes,
        siblings: current.siblings,
        series: after_prepare,
        ticket_state: Some(prepared_ticket),
        now_slot: retry_slot,
    })
    .map_err(|error| Error::new(format!("Series Expire projection refused: {error:?}")))?;
    let prepare_request = prepare.request().as_bytes().to_vec();
    let expire_request = expire.as_bytes().to_vec();
    // Custody's parent field is the SHA-256 of the complete family request,
    // as its physical adapter specifies; it is intentionally distinct from
    // Hot's domain-separated accelerator transcript digest.
    let prepare_digest = solana_program::hash::hash(&prepare_request).to_bytes();
    let expire_digest = solana_program::hash::hash(&expire_request).to_bytes();
    Ok(SeriesPrepareParentsV1 {
        prepare_request,
        prepare_digest,
        expire_request,
        expire_digest,
    })
}

/// Canonical vacant Claims coordinates plus their same-snapshot observations.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SeriesClaimsVacancyV1 {
    pub(crate) aggregate: Pubkey,
    pub(crate) position: Pubkey,
    pub(crate) admission: Pubkey,
    pub(crate) aggregate_lamports: u64,
    pub(crate) position_lamports: u64,
    pub(crate) admission_lamports: u64,
}

/// Inputs which are either immutable admitted identities or direct chain
/// observations.  A caller cannot choose a PDA or a rent value here.
#[derive(Clone, Debug)]
pub(crate) struct SeriesPhysicalMaterialInputV1 {
    pub(crate) trading: Pubkey,
    pub(crate) core: Pubkey,
    pub(crate) custody: Pubkey,
    pub(crate) claims: Pubkey,
    pub(crate) rent_program: Pubkey,
    pub(crate) market: Pubkey,
    pub(crate) release_set: Identity,
    pub(crate) ticket: Identity,
    pub(crate) parent_root: Pubkey,
    pub(crate) payer: Pubkey,
    pub(crate) founder: Pubkey,
    pub(crate) refund_owner: Pubkey,
    pub(crate) founder_source: Pubkey,
    pub(crate) rent_credit: Pubkey,
    pub(crate) mint: Pubkey,
    pub(crate) token_program: Pubkey,
    pub(crate) collateral_release: Identity,
    pub(crate) projection_receipt_digest: [u8; 32],
    pub(crate) prepare_parent_digest: [u8; 32],
    pub(crate) expire_parent_digest: [u8; 32],
    pub(crate) claims_vacancy: SeriesClaimsVacancyV1,
    pub(crate) rent: Rent,
}

/// Derived coordinates and exact Rent minima for one future Series Market.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SeriesPhysicalMaterialV1 {
    pub(crate) prepare: SeriesCustodyPhysicalV3,
    pub(crate) expire: SeriesCustodyPhysicalV3,
    pub(crate) projected: SeriesProjectedCustodyPhysicalV3,
    pub(crate) permit: Pubkey,
    pub(crate) custody_authority: Pubkey,
    /// The normal-Custody SeriesEscrow source replay. Projected Lock consumes
    /// and closes this account before Realize.
    pub(crate) normal_replay: Pubkey,
    /// The projected-State PDA that Realize rewrites in place into the normal
    /// Trading Custody replay for the realized Hoard.
    pub(crate) realized_hoard_replay: Pubkey,
    pub(crate) claims: SeriesClaimsVacancyV1,
}

/// Derive every physical coordinate whose PDA namespace is already fixed by
/// the admitted future Market.  This is a pure projection; callers must
/// verify the returned Claims coordinates are the observed vacant accounts.
pub(crate) fn materialize_series_found_prepare_v1(
    input: SeriesPhysicalMaterialInputV1,
) -> Result<SeriesPhysicalMaterialV1> {
    let market = input.market.to_bytes();
    let release_set = input.release_set.to_bytes();
    let ticket = input.ticket.to_bytes();
    for (label, key) in [
        ("parent Series root", input.parent_root),
        ("Series payer", input.payer),
        ("Series founder", input.founder),
        ("Series refund owner", input.refund_owner),
        ("Series founder source", input.founder_source),
        ("future lifecycle credit", input.rent_credit),
        ("future collateral mint", input.mint),
        ("future token program", input.token_program),
    ] {
        if key == Pubkey::default() {
            return Err(Error::new(format!("materialized {label} was default")));
        }
    }
    if input.projection_receipt_digest == [0; 32]
        || input.prepare_parent_digest == [0; 32]
        || input.expire_parent_digest == [0; 32]
    {
        return Err(Error::new(
            "Series physical material omitted canonical parent digest",
        ));
    }

    let escrow_vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(market, release_set, ticket, CompartmentV1::SeriesEscrow)
            .as_slices(),
        &input.custody,
    )
    .0;
    let context_digest =
        solana_program::hash::hashv(&[dclutch_custody::PROJECTED_HOARD_CONTEXT_DOMAIN_V1, &ticket])
            .to_bytes();
    let hoard_vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            market,
            release_set,
            context_digest,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &input.custody,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(market, release_set).as_slices(),
        &input.custody,
    )
    .0;
    let normal_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(market, release_set, CallerRoleV1::Trading, ticket).as_slices(),
        &input.custody,
    )
    .0;
    let realized_hoard_replay = Pubkey::find_program_address(
        &ProjectedCustodyStateSeedsV2::new(market, release_set, context_digest).as_slices(),
        &input.custody,
    )
    .0;
    let permit = Pubkey::find_program_address(
        &SeriesFoundingPermitSeedsV1::new(
            input.release_set,
            Identity::new(market).map_err(|_| Error::new("future Market identity"))?,
            input.ticket,
        )
        .as_slices(),
        &input.core,
    )
    .0;
    let aggregate = Pubkey::find_program_address(
        &ClaimsFoundingAggregateSeedsV5::new(market)
            .map_err(|_| Error::new("Series Claims aggregate seeds"))?
            .as_slices(),
        &input.claims,
    )
    .0;
    let position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), input.founder.to_bytes())
            .map_err(|_| Error::new("Series Claims position seeds"))?
            .as_slices(),
        &input.claims,
    )
    .0;
    let admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), input.founder.to_bytes())
            .map_err(|_| Error::new("Series Claims admission seeds"))?
            .as_slices(),
        &input.claims,
    )
    .0;
    if input.claims_vacancy.aggregate != aggregate
        || input.claims_vacancy.position != position
        || input.claims_vacancy.admission != admission
    {
        return Err(Error::new(
            "Series Claims vacancy observation was noncanonical",
        ));
    }
    let replay_rent = input
        .rent
        .minimum_balance(dclutch_custody::CUSTODY_REPLAY_BYTES_V1);
    let vault_rent = input
        .rent
        .minimum_balance(dclutch_custody::token_svm::ACCOUNT_BYTES);
    let normal = |parent_request_digest| SeriesCustodyPhysicalV3 {
        caller_program: input.trading.to_bytes(),
        parent_request_digest,
        payer: input.payer.to_bytes(),
        mint: input.mint.to_bytes(),
        token_program: input.token_program.to_bytes(),
        founder_source: input.founder_source.to_bytes(),
        escrow_vault: escrow_vault.to_bytes(),
        hoard_vault: hoard_vault.to_bytes(),
        refund_destination: input.refund_owner.to_bytes(),
        rent_credit: input.rent_credit.to_bytes(),
        replay_rent_lamports: replay_rent,
        vault_rent_lamports: vault_rent,
    };
    Ok(SeriesPhysicalMaterialV1 {
        prepare: normal(input.prepare_parent_digest),
        expire: normal(input.expire_parent_digest),
        projected: SeriesProjectedCustodyPhysicalV3 {
            caller_program: input.trading.to_bytes(),
            core_program: input.core.to_bytes(),
            rent_program: input.rent_program.to_bytes(),
            parent_capability_root: input.parent_root.to_bytes(),
            projection_receipt_digest: input.projection_receipt_digest,
            payer: input.payer.to_bytes(),
            rent_credit: input.rent_credit.to_bytes(),
            hoard_vault: hoard_vault.to_bytes(),
            escrow_vault: escrow_vault.to_bytes(),
            mint: input.mint.to_bytes(),
            token_program: input.token_program.to_bytes(),
            collateral_release: input.collateral_release.to_bytes(),
            projected_state_rent_lamports: input
                .rent
                .minimum_balance(dclutch_custody::PROJECTED_CUSTODY_STATE_BYTES_V2),
            hoard_vault_rent_lamports: vault_rent,
            escrow_replay_rent_lamports: replay_rent,
            escrow_vault_rent_lamports: vault_rent,
        },
        permit,
        custody_authority,
        normal_replay,
        realized_hoard_replay,
        claims: input.claims_vacancy,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    use dclutch_claims::{
        founding_v5::ClaimsFoundingAggregateSeedsV5,
        protocol_position_v2::{ProtocolPositionAdmissionSeedsV2, ProtocolPositionSeedsV2},
    };
    use dclutch_core_contract::ContentId;
    use dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID;
    use dclutch_market::{
        SeriesFoundingPermitSeedsV1,
        capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        capability_program::{
            CapabilityRootHeaderV1, SelectedRecordBumpsV1, v4::CapabilityProgramV4,
        },
        realm::REALM_SCHEMA_RELEASE_ID_V1,
    };
    use dclutch_operator::series_lifecycle_v3::{
        SeriesCurrentOccurrenceV3, SeriesLifecycleSnapshotV3,
    };
    use dclutch_product::admission::{
        PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
    };
    use dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3;
    use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
    use dclutch_registry::release_set::CapabilityExecutionSelectionV1;
    use dclutch_source::{
        PROVIDER_RELEASE_SCHEMA_ID_V1, SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_SPEC_SCHEMA_ID_V1,
    };
    use dclutch_trading::series::replay::{SeriesStateV3, TicketStateSeedsV3};
    use dclutch_trading::series::{
        AuthenticatedProductProjectionV2, TemplateV3, admit_occurrence, admit_ticket,
        pre_founding_series_escrow,
    };
    use dclutch_trading_sbf::series::{
        account_profile_v4::SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4,
        expire_funding_artifacts_v5::SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5,
        prepare_funding_artifacts_v5::SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5,
    };
    use sha2::Sha256;
    use solana_program::hash::hash;

    use crate::{
        core_bump_projection::CoreProductGraphProjectionV1,
        market::{CoreProductGraphWalkV1, record_identity},
        market::{FutureMarketFinalizedRecordV1, FutureMarketImmutablePublicationV1},
        plan::{hex32, pubkey},
        runtime::decode_hex,
        series_founder::{
            PreparedSeriesFounderV1, SeriesOccurrenceFundingV1, SeriesTemplatePolicyV1,
            prepare_series_founder_from_market_v1,
        },
    };

    fn content(hex: &str) -> ContentId {
        ContentId::new(hex32(hex).expect("32-byte content identity"))
            .expect("nonzero content identity")
    }

    pub(crate) fn prepared_founder_with_plan()
    -> (PreparedSeriesFounderV1, crate::model::SuccessorPlan) {
        let (plan, input, mint, founder, refund_owner) =
            crate::market::tests::selected_family_compiler_fixture_v1();
        let id = |byte| ContentId::new([byte; 32]).expect("authored policy identity");
        let prepared = prepare_series_founder_from_market_v1(
            &plan,
            &input,
            mint,
            founder,
            refund_owner,
            SeriesTemplatePolicyV1 {
                product_generator: id(1),
                occurrence_generator: id(2),
                capability_template: id(3),
                product_derivation: id(4),
                occurrence_derivation: id(5),
                capability_derivation: id(6),
                funding_derivation: id(7),
                first_slot: 100,
                period_slots: 10,
                retry_window: 2,
                close_rent: 1,
            },
            [
                SeriesOccurrenceFundingV1 {
                    funding_list: id(8),
                    funds: dclutch_trading::series::FoundingFundsV3::new(9, 2, 3, 4)
                        .expect("first funding"),
                },
                SeriesOccurrenceFundingV1 {
                    funding_list: id(9),
                    funds: dclutch_trading::series::FoundingFundsV3::new(18, 2, 3, 4)
                        .expect("second funding"),
                },
            ],
        )
        .expect("canonical Market preview produces admitted Series leaves");
        (prepared, plan)
    }

    pub(crate) fn prepared_founder() -> PreparedSeriesFounderV1 {
        prepared_founder_with_plan().0
    }

    pub(crate) fn compiler_input<'a>(
        prepared: &'a PreparedSeriesFounderV1,
        parent_root: Pubkey,
        ticket_bytes: &'a [u8],
    ) -> SeriesFoundPrepareSelectionInputV1<'a> {
        let (plan, _, _, _, _) = crate::market::tests::selected_family_compiler_fixture_v1();
        compiler_input_with_plan_at_occurrence(prepared, &plan, parent_root, ticket_bytes, 0)
    }

    pub(crate) fn compiler_input_at_occurrence<'a>(
        prepared: &'a PreparedSeriesFounderV1,
        parent_root: Pubkey,
        ticket_bytes: &'a [u8],
        occurrence_index: usize,
    ) -> SeriesFoundPrepareSelectionInputV1<'a> {
        let (plan, _, _, _, _) = crate::market::tests::selected_family_compiler_fixture_v1();
        compiler_input_with_plan_at_occurrence(
            prepared,
            &plan,
            parent_root,
            ticket_bytes,
            occurrence_index,
        )
    }

    pub(crate) fn compiler_input_with_plan<'a>(
        prepared: &'a PreparedSeriesFounderV1,
        plan: &crate::model::SuccessorPlan,
        parent_root: Pubkey,
        ticket_bytes: &'a [u8],
    ) -> SeriesFoundPrepareSelectionInputV1<'a> {
        compiler_input_with_plan_at_occurrence(prepared, plan, parent_root, ticket_bytes, 0)
    }

    pub(crate) fn compiler_input_with_plan_at_occurrence<'a>(
        prepared: &'a PreparedSeriesFounderV1,
        plan: &crate::model::SuccessorPlan,
        parent_root: Pubkey,
        ticket_bytes: &'a [u8],
        occurrence_index: usize,
    ) -> SeriesFoundPrepareSelectionInputV1<'a> {
        let registry = pubkey(&plan.registry.program_id).expect("Registry program");
        let core = pubkey(&plan.core.program_id).expect("Core program");
        let occurrence = admit_occurrence(
            prepared.admitted.template(),
            &prepared.admitted.occurrences()[occurrence_index],
            &prepared.admitted.siblings()[occurrence_index],
        )
        .expect("first canonical occurrence");
        let ticket = admit_ticket(ticket_bytes).expect("canonical ticket");
        let product = AuthenticatedProductProjectionV2::new(
            content(&prepared.facts.product.product_record),
            content(&prepared.facts.product.stable_product_id),
            content(&prepared.facts.product.result_domain),
        );
        let escrow = pre_founding_series_escrow(
            occurrence,
            ticket,
            product,
            dclutch_trading::series::AccountKeyV3::new(registry.to_bytes()).expect("Registry key"),
        )
        .expect("first canonical escrow");
        let market = Pubkey::new_from_array(escrow.market().to_bytes());
        let founder = Pubkey::new_from_array(escrow.founder().to_bytes());
        // Derive every program-owned coordinate from the selected Market plan:
        // the canonical M0 ProjectFound projection uses the same identities.
        let claims = pubkey(&plan.claims.program_id).expect("Claims program");
        let aggregate = Pubkey::find_program_address(
            &ClaimsFoundingAggregateSeedsV5::new(market.to_bytes())
                .expect("Claims aggregate seeds")
                .as_slices(),
            &claims,
        )
        .0;
        let position = Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), founder.to_bytes())
                .expect("Claims position seeds")
                .as_slices(),
            &claims,
        )
        .0;
        let admission = Pubkey::find_program_address(
            &ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), founder.to_bytes())
                .expect("Claims admission seeds")
                .as_slices(),
            &claims,
        )
        .0;
        let rent = Rent::default();
        let custody = pubkey(&plan.custody.program_id).expect("Custody program");
        let trading = pubkey(&plan.trading.program_id).expect("Trading program");
        let rent_program = pubkey(&plan.rent_credit.program_id).expect("Rent program");
        let ticket_state = Pubkey::find_program_address(
            &TicketStateSeedsV3::new(parent_root.to_bytes(), ticket.content_id()).as_slices(),
            &trading,
        )
        .0;
        let permit_bump = Pubkey::find_program_address(
            &SeriesFoundingPermitSeedsV1::new(
                Identity::new(escrow.release_set().to_bytes()).expect("release set identity"),
                Identity::new(market.to_bytes()).expect("future Market identity"),
                Identity::new(escrow.ticket_id().to_bytes()).expect("Ticket identity"),
            )
            .as_slices(),
            &core,
        )
        .1;
        let template = TemplateV3::decode(prepared.admitted.template()).expect("Template");
        let lifecycle = SeriesLifecycleSnapshotV3 {
            template_bytes: prepared.admitted.template(),
            series: SeriesStateV3::new(template.close_rent()),
            now_slot: 100,
            current: Some(SeriesCurrentOccurrenceV3 {
                occurrence_bytes: &prepared.admitted.occurrences()[occurrence_index],
                ticket_bytes,
                siblings: &prepared.admitted.siblings()[occurrence_index],
                ticket_state: None,
            }),
            terminal_ticket: None,
            observed_root_lamports: 1,
            exact_root_rent: 1,
            rent_sink: None,
        };
        SeriesFoundPrepareSelectionInputV1 {
            lifecycle,
            product,
            registry_program: dclutch_trading::series::AccountKeyV3::new(registry.to_bytes())
                .expect("Registry key"),
            material: SeriesPhysicalMaterialInputV1 {
                trading,
                core,
                custody,
                claims,
                rent_program,
                market,
                release_set: Identity::new(escrow.release_set().to_bytes())
                    .expect("release set identity"),
                ticket: Identity::new(escrow.ticket_id().to_bytes()).expect("Ticket identity"),
                parent_root,
                payer: Pubkey::new_from_array([15; 32]),
                founder,
                refund_owner: Pubkey::new_from_array(escrow.refund_owner().to_bytes()),
                founder_source: Pubkey::new_from_array([16; 32]),
                rent_credit: Pubkey::new_from_array([17; 32]),
                mint: Pubkey::new_from_array([18; 32]),
                token_program: Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
                collateral_release: Identity::new([61; 32]).expect("collateral release"),
                projection_receipt_digest: [1; 32],
                prepare_parent_digest: [2; 32],
                expire_parent_digest: [3; 32],
                claims_vacancy: SeriesClaimsVacancyV1 {
                    aggregate,
                    position,
                    admission,
                    aggregate_lamports: 1,
                    position_lamports: 1,
                    admission_lamports: 1,
                },
                rent: rent.clone(),
            },
            core_product_graph: [
                (
                    PRODUCT_RECORD_SCHEMA_ID_V2,
                    record_identity(&prepared.publication.product),
                ),
                (
                    RESULT_DOMAIN_SCHEMA_ID_V2,
                    record_identity(&prepared.publication.domain),
                ),
                (
                    PORTFOLIO_SCHEMA_ID_V2,
                    record_identity(&prepared.publication.portfolio),
                ),
                (
                    GRADED_BASIS_RECORD_SCHEMA_ID_V3,
                    record_identity(&prepared.publication.basis),
                ),
            ],
            core_projection: CoreProductGraphProjectionV1::Recorded,
            core_walk: CoreProductGraphWalkV1::ProjectedFounding,
            principal_cap_sets: 1,
            linked_basis_record_digest: record_identity(&prepared.publication.basis),
            semantic_basis_id: content(
                &prepared.facts.occurrences[occurrence_index].liability_basis,
            )
            .to_bytes(),
            claim_count: dclutch_product::PortfolioV2::decode(&prepared.publication.portfolio)
                .expect("canonical M0 Portfolio")
                .coefficient_count(),
            claims_rent_principals: [1, 1, 1],
            permit_bump,
            projected_bump: 1,
            founder_source_amount: escrow.hoard_principal(),
            geometry: Some(crate::series_source::SeriesObservedGeometryV1 {
                prepare_fixed_data_lengths: [0; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize],
                prepare_ticket_rent_lamports: rent
                    .minimum_balance(dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3),
                consume_fixed_data_lengths: [0; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
                consume_funding_count: 1,
                expire_fixed_data_lengths: [0; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize],
            }),
            selected_release: template.release_set(),
            funding_ledger_slot_count: 1,
            activation_deadline_slot: 101,
            selected_manifest_entry_index: 0,
            ticket_state_account: dclutch_trading::series::AccountKeyV3::new(
                ticket_state.to_bytes(),
            )
            .expect("Ticket state account"),
        }
    }

    fn derived_parent_root(
        prepared: &PreparedSeriesFounderV1,
        compiled: &CompiledSeriesFoundPrepareSelectionV1,
    ) -> Pubkey {
        let header = compiled_root_header(prepared, compiled);
        Pubkey::find_program_address(
            &header.seeds().as_slices(),
            &Pubkey::new_from_array([13; 32]),
        )
        .0
    }

    pub(crate) fn compiled_root_header(
        prepared: &PreparedSeriesFounderV1,
        compiled: &CompiledSeriesFoundPrepareSelectionV1,
    ) -> CapabilityRootHeaderV1 {
        let descriptor = decode_hex(&compiled.selected.selected_descriptor_hex)
            .expect("selected descriptor hex");
        let descriptor = CapabilityProgramV4::decode(&descriptor).expect("selected descriptor");
        let selection = CapabilityExecutionSelectionV1::new(
            compiled.selected.selected_manifest_entry_index,
            ContentId::new(record_identity(&prepared.publication.manifest))
                .expect("canonical Market manifest identity"),
            descriptor.kind(),
            ContentId::new(
                Sha256::digest(
                    decode_hex(&compiled.selected.program_set_hex).expect("program set hex"),
                )
                .into(),
            )
            .expect("program set identity"),
            ContentId::new(
                Sha256::digest(decode_hex(&compiled.selected.config_hex).expect("config hex"))
                    .into(),
            )
            .expect("config identity"),
        )
        .expect("selected execution projection");
        let parent_market = Pubkey::new_from_array(
            admit_occurrence(
                prepared.admitted.template(),
                &prepared.admitted.occurrences()[0],
                &prepared.admitted.siblings()[0],
            )
            .expect("first occurrence")
            .occurrence()
            .market()
            .to_bytes(),
        );
        let header = CapabilityRootHeaderV1::new(
            TemplateV3::decode(prepared.admitted.template())
                .expect("Template")
                .release_set(),
            parent_market.to_bytes(),
            1,
            selection,
            SelectedRecordBumpsV1::default(),
        )
        .expect("canonical parent root header");
        header
    }

    pub(crate) fn published_record(
        registry: Pubkey,
        schema: [u8; 32],
        body: &[u8],
    ) -> crate::runtime::PublishedRecord {
        let digest = hash(body).to_bytes();
        crate::runtime::PublishedRecord {
            schema,
            digest,
            raw: Pubkey::find_program_address(
                &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
                &registry,
            )
            .0,
            staging: Pubkey::find_program_address(
                &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
                &registry,
            )
            .0,
        }
    }

    pub(crate) fn canonical_m0_publication(
        prepared: &crate::series_founder::PreparedSeriesFounderV1,
        registry: Pubkey,
        market: Pubkey,
        payer: Pubkey,
        rent_credit: Pubkey,
        rent_program: Pubkey,
        core: Pubkey,
        activation: Pubkey,
        source_spec_body: &[u8],
        source_capacity_body: &[u8],
    ) -> FutureMarketImmutablePublicationV1 {
        let realm = published_record(
            registry,
            REALM_SCHEMA_RELEASE_ID_V1,
            &prepared.publication.realm,
        );
        let product = published_record(
            registry,
            PRODUCT_RECORD_SCHEMA_ID_V2,
            &prepared.publication.product,
        );
        let domain = published_record(
            registry,
            RESULT_DOMAIN_SCHEMA_ID_V2,
            &prepared.publication.domain,
        );
        let portfolio = published_record(
            registry,
            PORTFOLIO_SCHEMA_ID_V2,
            &prepared.publication.portfolio,
        );
        let basis = published_record(
            registry,
            GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            &prepared.publication.basis,
        );
        let source = published_record(
            registry,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
            &prepared.publication.source,
        );
        let source_spec = published_record(registry, SOURCE_SPEC_SCHEMA_ID_V1, source_spec_body);
        let source_capacity = published_record(
            registry,
            SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
            source_capacity_body,
        );
        let manifest = published_record(
            registry,
            CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            &prepared.publication.manifest,
        );
        let key = |byte| Pubkey::new_from_array([byte; 32]);
        let core_programdata = crate::upgrade::target_programdata(core);
        let infrastructure = key(7);
        let registry_programdata = key(8);
        let rent_artifact =
            published_record(registry, PROVIDER_RELEASE_SCHEMA_ID_V1, b"rent artifact");
        let registry_artifact = published_record(
            registry,
            PROVIDER_RELEASE_SCHEMA_ID_V1,
            b"registry artifact",
        );
        let rent_programdata = key(9);
        let project_found: [Pubkey; dclutch_market::PROJECT_FOUND_ACCOUNT_COUNT_V2] =
            crate::market::project_found_snapshot_from_closure_v2(
                crate::market::MarketProjectFoundProjectionV1 {
                    payer,
                    market,
                    rent_credit,
                    rent_program,
                    closure: crate::market::MarketProjectFoundClosureV1 {
                        realm,
                        product,
                        domain,
                        portfolio,
                        basis,
                        source,
                        source_spec,
                        source_capacity_profile: source_capacity,
                        manipulation_floor: (key(11), key(12)),
                        manifest,
                        activation,
                        core,
                        core_programdata,
                        registry,
                        infrastructure,
                        registry_artifact: (registry_artifact.raw, registry_artifact.staging),
                        registry_programdata,
                        rent_artifact: (rent_artifact.raw, rent_artifact.staging),
                        rent_programdata,
                        price_gate: None,
                    },
                },
            )
            .expect("canonical ProjectFound frame")
            .try_into()
            .expect("canonical ProjectFound width");
        let series_prepare_records = [
            (realm, prepared.publication.realm.as_slice()),
            (product, prepared.publication.product.as_slice()),
            (domain, prepared.publication.domain.as_slice()),
            (portfolio, prepared.publication.portfolio.as_slice()),
            (basis, prepared.publication.basis.as_slice()),
            (source, prepared.publication.source.as_slice()),
            (source_spec, source_spec_body),
            (source_capacity, source_capacity_body),
            (manifest, prepared.publication.manifest.as_slice()),
        ]
        .into_iter()
        .map(|(published, body)| FutureMarketFinalizedRecordV1 {
            published,
            body: body.to_vec(),
        })
        .collect();
        FutureMarketImmutablePublicationV1 {
            realm,
            product,
            domain,
            portfolio,
            manifest,
            rent_credit,
            project_found,
            principal_cap_sets: 1,
            series_prepare_records,
            series_prepare_vacancies: vec![key(11), key(12)],
        }
    }
    #[test]
    fn preprofile_derives_child_bank_without_geometry_placeholder() {
        let prepared = prepared_founder();
        let mut input = compiler_input(
            &prepared,
            Pubkey::new_unique(),
            &prepared.admitted.tickets()[0],
        );
        input.geometry = None;
        let preprofile = derive_series_found_prepare_preprofile_v1(&mut input)
            .expect("typed preprofile derives before the geometry observer");
        assert!(!preprofile.parents.prepare_request.is_empty());
        assert_eq!(
            preprofile
                .prepare_children
                .prepare_requests()
                .projected_initialize
                .len(),
            dclutch_trading_sbf::series::artifacts_v3::SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3
        );
    }

    #[test]
    fn canonical_market_two_occurrence_series_compiler_accepts_and_normalizes_parent_root() {
        let prepared = prepared_founder();
        let provisional_root = Pubkey::new_unique();
        let certificate = ContentId::new([62; 32]).expect("shadow certificate program");
        let provisional = compile_series_found_prepare_selection_v1(
            compiler_input(&prepared, provisional_root, &prepared.admitted.tickets()[0]),
            certificate,
        )
        .expect("accepted provisional Series selection");
        assert_eq!(provisional.selected.records.len(), 39);
        assert!(!provisional.parents.prepare_request.is_empty());
        assert!(!provisional.parents.expire_request.is_empty());

        let actual_root = derived_parent_root(&prepared, &provisional);
        assert_ne!(actual_root, provisional_root);
        let actual = compile_series_found_prepare_selection_v1(
            compiler_input(&prepared, actual_root, &prepared.admitted.tickets()[0]),
            certificate,
        )
        .expect("accepted derived-parent Series selection");
        require_series_selection_invariance_v1(&provisional, &actual)
            .expect("parent-root normalization preserves immutable publication bytes");

        let hostile = prepared.admitted.tickets()[1].clone();
        let mut bad_input = compiler_input(&prepared, actual_root, &prepared.admitted.tickets()[0]);
        bad_input.lifecycle.current = Some(SeriesCurrentOccurrenceV3 {
            occurrence_bytes: &prepared.admitted.occurrences()[0],
            ticket_bytes: &hostile,
            siblings: &prepared.admitted.siblings()[0],
            ticket_state: None,
        });
        let bad_join = compile_series_found_prepare_selection_v1(bad_input, certificate);
        let Err(error) = bad_join else {
            panic!("second Ticket cannot join first occurrence");
        };
        assert_eq!(
            error.to_string(),
            "Series Prepare lifecycle refused: Content"
        );
    }
}

#[cfg(test)]
#[path = "series_recurrence_tests.rs"]
mod recurrence_tests;
