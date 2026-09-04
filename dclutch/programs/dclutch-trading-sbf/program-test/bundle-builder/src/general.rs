//! General-specific construction over the family-neutral Hot bundle builder.
//!
//! This module owns no General arithmetic. It joins the current semantic owners
//! into one exact pre-executable request, PER ACTION:
//!
//! - [`GeneralRootV2`] supplies the active occurrence generation, revision and
//!   next sequence;
//! - [`GeneralConfigV3`] supplies the immutable price scale and order bound;
//! - [`GeneralBatchOccurrenceTermsV1`] supplies the slot-independent occurrence
//!   identity, while [`GeneralStateAddressSeedsV3`] supplies the published
//!   lifecycle seed order for its Batch PDA;
//! - for an action that names a state the chain already holds,
//!   [`GeneralLocalStateV3`] supplies the exact lifecycle envelope and its
//!   semantic body, decoded here rather than by the campaign. A campaign that
//!   spelled `data[HEADER..]` itself would be a second author for the physical
//!   envelope, which is exactly the debt the seam register forbids.
//!
//! The returned request is then consumed by [`build_general_action_bundle_v1`],
//! which selects the emitted descriptor through the published ProgramSet and
//! executes the ordinary admitted-AOT builder with that action's accelerator-
//! owned candidate projector. A campaign may supply semantic chain corpus, but
//! it cannot type a state identity, bump, accelerator request, caller
//! authority, span width, or account topology.
//!
//! ## Why per action rather than per call site
//!
//! Until 2026-09-04 this module was pinned to `OpenBatch` at two entry points,
//! and a General market could therefore execute exactly one action in any
//! harness. The fifteen actions differ in the STATE they name and in the
//! projector that owns their candidate, and in nothing else this module cares
//! about -- so the dispatch is two matches over [`Action`] and every other line
//! is shared. Actions this module does not derive yet refuse
//! [`BuilderError::UnsupportedRoute`] at a named line rather than being built
//! wrong.

use dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1;
use dclutch_execution_strategy_contract::{decode_register_bank_into, encode_register_bank_into};
use dclutch_general_adapter_contract::{
    collection_v1::{GeneralBatchOccurrenceTermsV1, GeneralBatchOpeningV1, GeneralBatchV1},
    hot_candidate_v3::{
        general_hot_candidate_bank_len_v3, general_hot_environment_from_bank_v3,
        project_general_close_batch_candidate_in_place_v3,
        project_general_open_batch_candidate_in_place_v3,
    },
    local_state_v3::{GeneralLocalStateKindV3, GeneralLocalStateV3},
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

/// Chain-authenticated facts needed to derive one General action's request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralRequestInputV1<'a> {
    /// The action to derive. The ProgramSet remains the authority for the
    /// descriptor join; this is what the request will ASK for.
    pub action: Action,
    /// Exact current General root tail decoded from the composite root.
    pub root: GeneralRootV2,
    /// Address of that composite root, used by the lifecycle seed program.
    pub root_address: Pubkey,
    /// Exact selected config bytes.
    pub config: &'a [u8],
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
    /// Content identity carried in the selected Product record's own
    /// `product_id` field, which is what the AccountProfile projects into
    /// `identity::SELECTION_PRODUCT` and what the batch occurrence therefore
    /// commits to. It is NOT the finalized record digest.
    pub product_id: [u8; 32],
    /// Trading program that owns the root and every General state PDA.
    pub trading_program: Pubkey,
    /// EXACT ACCOUNT DATA of the primary state this action names, as the chain
    /// holds it, for an action that operates on a state that already exists.
    ///
    /// The whole account, not its body: the lifecycle envelope is decoded here
    /// so that one author states where a General state's semantic bytes begin.
    /// `None` for an action whose primary state this execution creates.
    pub primary_state_account: Option<&'a [u8]>,
}

/// Exact subject, lifecycle coordinate and V3 request derived for one action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralRequestV1 {
    /// The action this request asks for.
    pub action: Action,
    /// Slot-independent subject identity carried as the request subject.
    pub subject_id: [u8; 32],
    /// Canonical primary-state PDA selected by the published lifecycle recipe.
    pub primary_state: Pubkey,
    /// Canonical PDA bump encoded in the request.
    pub primary_state_bump: u8,
    /// Exact canonical 64-byte V3 request.
    pub request: [u8; dclutch_general_codec::successor_request_v3::CONTROLLER_REQUEST_BYTES_V3],
}

/// Decode the exact live Batch a request or projector names.
///
/// The kind is checked because a lifecycle envelope carries six of them and a
/// campaign that handed the Order account to a Batch action would otherwise
/// reach [`GeneralBatchV1::decode`] with bytes of the wrong width and be told
/// only that they did not decode.
fn live_batch(account: &[u8]) -> Result<(GeneralLocalStateV3<'_>, GeneralBatchV1), BuilderError> {
    let envelope = GeneralLocalStateV3::decode(account).map_err(|_| BuilderError::Artifact)?;
    if envelope.header().kind != GeneralLocalStateKindV3::Batch {
        return Err(BuilderError::Binding(line!()));
    }
    let batch = GeneralBatchV1::decode(envelope.body()).map_err(|_| BuilderError::Artifact)?;
    Ok((envelope, batch))
}

/// The Batch PDA and its canonical bump for one occurrence under one root.
fn batch_address(
    root_address: Pubkey,
    trading_program: Pubkey,
    occurrence_id: [u8; 32],
) -> Result<(Pubkey, u8), BuilderError> {
    let seeds = GeneralStateAddressSeedsV3::batch(root_address.to_bytes(), occurrence_id)
        .map_err(|_| BuilderError::Artifact)?;
    let slices = seeds.as_slices().map_err(|_| BuilderError::Artifact)?;
    Ok(Pubkey::find_program_address(
        slices.as_slice(),
        &trading_program,
    ))
}

/// Derive the exact pre-executable request for one General action.
///
/// The joins every action shares are checked once, here: nonzero corpus, the
/// root's config identity against the exact selected config bytes, and the
/// root's generation against the config's. What differs per action is the
/// SUBJECT and the STATE, and that is the whole of the match below.
pub fn derive_general_request_v1(
    input: GeneralRequestInputV1<'_>,
) -> Result<GeneralRequestV1, BuilderError> {
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
    let (subject_id, primary_state, primary_state_bump) = match input.action {
        Action::OpenBatch => {
            // The state this execution CREATES: there is nothing on chain to
            // read, and a campaign that supplied one is describing a different
            // execution than the one it asked for.
            if input.primary_state_account.is_some() {
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
            let (batch, bump) =
                batch_address(input.root_address, input.trading_program, occurrence_id)?;
            (occurrence_id, batch, bump)
        }
        Action::CloseBatch => {
            // THE SUBJECT IS READ, NOT PREDICTED. `GeneralBatchV1::batch_id`
            // recomputes the occurrence identity from the batch's own immutable
            // opening, so the request names the batch the chain holds even
            // where a host-side prediction of the opening would have differed.
            let account = input
                .primary_state_account
                .ok_or(BuilderError::Binding(line!()))?;
            let (envelope, batch) = live_batch(account)?;
            let batch_id = batch.batch_id();
            let (address, bump) =
                batch_address(input.root_address, input.trading_program, batch_id)?;
            // The persisted canonical bump and the rederived one are two
            // independent authors for the same byte, and the request witnesses
            // it to the accelerator's admission. Joining them here means a
            // recipe that changed under the market cannot pass a stale witness.
            if envelope.header().bump != bump
                || batch.opening().outcome_count != input.outcome_count
                || batch.opening().market != input.root.market()
                || batch.opening().config_id != input.root.config_id()
                || batch.opening().generation != input.root.generation()
                || batch.opening().product_id != input.product_id
            {
                return Err(BuilderError::Binding(line!()));
            }
            (batch_id, address, bump)
        }
        other => {
            let _ = other;
            return Err(BuilderError::UnsupportedRoute(line!()));
        }
    };
    let request = ControllerRequestV3 {
        action: ControllerActionV3::from(input.action),
        expected_revision: input.root.revision(),
        subject_id: Some(subject_id),
        page_index: 0,
        execution_index: 0,
        manifest_order_index: 0,
        primary_state_bump,
        secondary_state_bump: 0,
        result_state_bump: 0,
    }
    .to_bytes()
    .map_err(|_| BuilderError::Artifact)?;
    Ok(GeneralRequestV1 {
        action: input.action,
        subject_id,
        primary_state,
        primary_state_bump,
        request,
    })
}

/// Semantic chain prestate one action's candidate projector reads.
///
/// A campaign supplies the exact bytes the bank holds; the projector's semantic
/// owner decodes them. Nothing here is optional in the sense of "may be
/// omitted": each action requires exactly what it names, and an action handed
/// the wrong shape refuses at a named line.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GeneralActionPrestateV1<'a> {
    /// Exact account data of the primary state this action operates on, as the
    /// chain holds it, or `None` where this execution creates it.
    pub primary_state_account: Option<&'a [u8]>,
}

/// Build one admitted-AOT General Hot instruction from current artifacts.
///
/// The caller must have obtained `input.scenario.family_request` from
/// [`derive_general_request_v1`]. The selected ProgramSet remains the authority
/// for the descriptor/action join; the guard below merely prevents a General
/// campaign helper from silently being used for another catalogue row, and the
/// action it derives is the one the request asks for rather than one this
/// module names.
pub fn build_general_action_bundle_v1(
    input: &BundleInputV1<'_>,
    admitted: AdmittedAotInputV1<'_>,
    prestate: GeneralActionPrestateV1<'_>,
) -> Result<BuiltAdmittedBundleV1, BuilderError> {
    let request = ControllerRequestV3::decode(input.scenario.family_request)
        .map_err(|_| BuilderError::Artifact)?;
    let action = request
        .action
        .legacy()
        .ok_or(BuilderError::Binding(line!()))?;
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
    let bank_len = general_hot_candidate_bank_len_v3(action, outcome_count)
        .map_err(|_| BuilderError::Projection("general-bank-width"))?;
    // The semantic prestate is decoded ONCE, outside the projector closure: the
    // adoption loop runs the projector up to four times and a body re-decoded
    // per round would make a corpus refusal look like a divergence.
    let batch_body = match action {
        Action::OpenBatch => {
            if prestate.primary_state_account.is_some() {
                return Err(BuilderError::Binding(line!()));
            }
            None
        }
        Action::CloseBatch => {
            let account = prestate
                .primary_state_account
                .ok_or(BuilderError::Binding(line!()))?;
            Some(live_batch(account)?.0.body())
        }
        _ => return Err(BuilderError::UnsupportedRoute(line!())),
    };
    let projector = |scalars: &mut [u64],
                     identities: &mut [[u8; 32]]|
     -> Result<(), BuilderError> {
        let mut bank = vec![0_u8; bank_len];
        encode_register_bank_into(scalars, identities, &mut bank)
            .map_err(|_| BuilderError::Projection("general-bank-encode"))?;
        let environment = general_hot_environment_from_bank_v3(action, &bank, outcome_count)
            .map_err(|_| BuilderError::Projection("general-environment"))?;
        // THE WIRE CANNOT CARRY THE CAUSE AND THE LOG CAN.
        // `BuilderError::Projection` is one `&'static str`, so the
        // accelerator's own `GeneralHotCandidateErrorV3` -- which
        // distinguishes a capacity, a stride, a coordinate and a plan --
        // would otherwise be discarded at the one boundary where it is the
        // whole answer. `pack_frame` already prints its width refusal for
        // the same reason. A campaign reads a validator log first.
        let refused = |stage: &'static str| {
            move |error: dclutch_general_adapter_contract::hot_candidate_v3::GeneralHotCandidateErrorV3| {
                    std::eprintln!(
                        "general candidate projection refused at {stage} for {action:?}: {error:?}"
                    );
                    BuilderError::Projection(stage)
                }
        };
        match action {
            Action::OpenBatch => project_general_open_batch_candidate_in_place_v3(
                root_tail,
                config,
                outcome_count,
                environment,
                request.expected_revision,
                request.subject_id,
                &mut bank,
            )
            .map_err(refused("general-open-batch")),
            Action::CloseBatch => project_general_close_batch_candidate_in_place_v3(
                root_tail,
                batch_body.ok_or(BuilderError::Projection("general-close-batch-prestate"))?,
                config,
                outcome_count,
                environment,
                request.expected_revision,
                request.subject_id,
                &mut bank,
            )
            .map_err(refused("general-close-batch")),
            _ => Err(BuilderError::UnsupportedRoute(line!())),
        }?;
        decode_register_bank_into(&bank, scalars, identities)
            .map_err(|_| BuilderError::Projection("general-bank-decode"))
    };
    let built = build_admitted_bundle_with_candidate_v1(input, admitted, &projector)?;
    if built.bundle.artifacts.action != u32::from(action as u8) {
        return Err(BuilderError::Artifact);
    }
    Ok(built)
}
