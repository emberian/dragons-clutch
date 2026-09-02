//! General-specific construction over the family-neutral Hot bundle builder.
//!
//! This module owns no General arithmetic. It joins three current semantic
//! owners into the exact pre-executable `OpenBatch` request:
//!
//! - [`GeneralRootV2`] supplies the active occurrence generation, revision and
//!   next sequence;
//! - [`GeneralConfigV3`] supplies the immutable price scale and order bound;
//! - [`GeneralBatchOccurrenceTermsV1`] supplies the slot-independent occurrence
//!   identity, while [`GeneralStateAddressSeedsV3`] supplies the published
//!   lifecycle seed order for its Batch PDA.
//!
//! The returned request is then consumed by [`build_general_open_batch_bundle_v1`],
//! which selects the emitted `OpenBatch` descriptor through the published
//! ProgramSet and executes the ordinary admitted-AOT builder. A campaign may
//! supply semantic chain corpus, but it cannot type a batch identity, bump,
//! accelerator request, caller authority, span width, or account topology.

use dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1;
use dclutch_execution_strategy_contract::{decode_register_bank_into, encode_register_bank_into};
use dclutch_general_adapter_contract::{
    collection_v1::{GeneralBatchOccurrenceTermsV1, GeneralBatchOpeningV1},
    hot_candidate_v3::{
        general_hot_candidate_bank_len_v3, general_hot_environment_from_bank_v3,
        project_general_open_batch_candidate_in_place_v3,
    },
    state_seeds_v3::GeneralStateAddressSeedsV3,
};
use dclutch_general_codec::{
    Action,
    successor_request_v3::{ControllerActionV3, ControllerRequestV3},
};
use dclutch_general_config_contract::{GeneralRootV2, v3::GeneralConfigV3};
use solana_program::pubkey::Pubkey;

use crate::{
    BuilderError,
    admitted::AdmittedAotInputV1,
    bundle::{BuiltAdmittedBundleV1, BundleInputV1, build_admitted_bundle_with_candidate_v1},
};

/// Chain-authenticated facts needed to derive one `OpenBatch` request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOpenBatchRequestInputV1<'a> {
    /// Exact current General root tail decoded from the composite root.
    pub root: GeneralRootV2,
    /// Address of that composite root, used by the lifecycle seed program.
    pub root_address: Pubkey,
    /// Exact selected config bytes.
    pub config: &'a [u8],
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
    /// Content identity carried in the selected Product record's own
    /// `product_id` field, which is what the OpenBatch AccountProfile projects
    /// into `identity::SELECTION_PRODUCT` and what the batch occurrence
    /// therefore commits to. It is NOT the finalized record digest.
    pub product_id: [u8; 32],
    /// Trading program that owns the root and Batch PDA.
    pub trading_program: Pubkey,
}

/// Exact occurrence, lifecycle coordinate and V3 request derived for OpenBatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOpenBatchRequestV1 {
    /// Slot-independent occurrence identity carried as the request subject.
    pub occurrence_id: [u8; 32],
    /// Canonical Batch PDA selected by the published lifecycle recipe.
    pub batch: Pubkey,
    /// Canonical PDA bump encoded in the request.
    pub batch_bump: u8,
    /// Exact canonical 64-byte V3 request.
    pub request: [u8; dclutch_general_codec::successor_request_v3::CONTROLLER_REQUEST_BYTES_V3],
}

/// Derive the exact pre-executable `OpenBatch` request from authenticated facts.
pub fn derive_general_open_batch_request_v1(
    input: GeneralOpenBatchRequestInputV1<'_>,
) -> Result<GeneralOpenBatchRequestV1, BuilderError> {
    let config = GeneralConfigV3::decode(input.config).map_err(|_| BuilderError::Artifact)?;
    if input.root_address == Pubkey::default()
        || input.trading_program == Pubkey::default()
        || input.outcome_count == 0
        || input.product_id == [0; 32]
        || input.root.config_id() != solana_program::hash::hash(input.config).to_bytes()
        || input.root.generation() != config.generation()
    {
        return Err(BuilderError::Binding(line!()));
    }
    let occurrence = GeneralBatchOccurrenceTermsV1::new(GeneralBatchOpeningV1 {
        outcome_count: input.outcome_count,
        sequence: input.root.next_batch_sequence(),
        generation: input.root.generation(),
        market: input.root.market(),
        product_id: input.product_id,
        config_id: input.root.config_id(),
        price_scale: config.price_scale(),
        collection_close_slot: 0,
        settlement_close_slot: 0,
        max_orders: config.max_orders_per_candidate(),
    })
    .map_err(|_| BuilderError::Artifact)?;
    let occurrence_id = occurrence.occurrence_id();
    let seeds = GeneralStateAddressSeedsV3::batch(input.root_address.to_bytes(), occurrence_id)
        .map_err(|_| BuilderError::Artifact)?;
    let seed_slices = seeds.as_slices().map_err(|_| BuilderError::Artifact)?;
    let (batch, batch_bump) =
        Pubkey::find_program_address(seed_slices.as_slice(), &input.trading_program);
    let request = ControllerRequestV3 {
        action: ControllerActionV3::OpenBatch,
        expected_revision: input.root.revision(),
        subject_id: Some(occurrence_id),
        page_index: 0,
        execution_index: 0,
        manifest_order_index: 0,
        primary_state_bump: batch_bump,
        secondary_state_bump: 0,
        result_state_bump: 0,
    }
    .to_bytes()
    .map_err(|_| BuilderError::Artifact)?;
    Ok(GeneralOpenBatchRequestV1 {
        occurrence_id,
        batch,
        batch_bump,
        request,
    })
}

/// Build one admitted-AOT `OpenBatch` Hot instruction from current artifacts.
///
/// The caller must have obtained `input.scenario.family_request` from
/// [`derive_general_open_batch_request_v1`]. The selected ProgramSet remains the
/// authority for the descriptor/action join; this guard merely prevents a
/// General campaign helper from silently being used for another catalogue row.
pub fn build_general_open_batch_bundle_v1(
    input: &BundleInputV1<'_>,
    admitted: AdmittedAotInputV1<'_>,
) -> Result<BuiltAdmittedBundleV1, BuilderError> {
    let request = ControllerRequestV3::decode(input.scenario.family_request)
        .map_err(|_| BuilderError::Artifact)?;
    if request.action.legacy() != Some(Action::OpenBatch) {
        return Err(BuilderError::Binding(line!()));
    }
    let config = GeneralConfigV3::decode(input.set.config).map_err(|_| BuilderError::Artifact)?;
    let root_tail = input
        .fixed
        .root
        .account
        .data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(BuilderError::Binding(line!()))?;
    GeneralRootV2::decode(root_tail).map_err(|_| BuilderError::Binding(line!()))?;
    let outcome_count = input.scenario.tail_count;
    let bank_len = general_hot_candidate_bank_len_v3(Action::OpenBatch, outcome_count)
        .map_err(|_| BuilderError::Projection("general-bank-width"))?;
    let projector =
        |scalars: &mut [u64], identities: &mut [[u8; 32]]| -> Result<(), BuilderError> {
            let mut bank = vec![0_u8; bank_len];
            encode_register_bank_into(scalars, identities, &mut bank)
                .map_err(|_| BuilderError::Projection("general-bank-encode"))?;
            let environment =
                general_hot_environment_from_bank_v3(Action::OpenBatch, &bank, outcome_count)
                    .map_err(|_| BuilderError::Projection("general-environment"))?;
            project_general_open_batch_candidate_in_place_v3(
                root_tail,
                config,
                outcome_count,
                environment,
                request.expected_revision,
                request.subject_id,
                &mut bank,
            )
            .map_err(|_| BuilderError::Projection("general-open-batch"))?;
            decode_register_bank_into(&bank, scalars, identities)
                .map_err(|_| BuilderError::Projection("general-bank-decode"))
        };
    let built = build_admitted_bundle_with_candidate_v1(input, admitted, &projector)?;
    if built.bundle.artifacts.action != u32::from(Action::OpenBatch as u8) {
        return Err(BuilderError::Artifact);
    }
    Ok(built)
}
