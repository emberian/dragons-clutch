//! Program-test compatibility adapter for the production Hot projector.
//!
//! The artifact semantics and derivation now live in
//! `dclutch_operator::dealer_hot_projection_v1`. This crate retains only its
//! fixture-shaped release-waist adapter so every campaign consumes the same
//! production implementation.

use dclutch_operator::dealer_hot_projection_v1 as production;
use dclutch_vm::account_profile::v2::AccountProfileV2;
use solana_program::rent::Rent;
use std::cell::RefCell;

use crate::{BuilderError, WaistFactsV1};

pub use production::{
    ContentProjectionKeysV1, DerivedInvocationV1, EngineOutputV1, LifecycleStateDerivationV1,
    ObservedAccountV1, SpanWidthDerivationV1,
};

/// Fixture-compatible admitted candidate callback.
///
/// The production projector owns when the callback runs. This adapter keeps
/// the fixture's richer error type at its public boundary.
pub type AdmittedCandidateProjectorV1<'a> =
    dyn Fn(&mut [u64], &mut [[u8; 32]]) -> Result<(), BuilderError> + 'a;

/// Everything the production register pipeline consumes.
pub struct EngineInputV1<'a> {
    /// Decoded account profile.
    pub profile: AccountProfileV2<'a>,
    /// Request profile bytes.
    pub request_profile_bytes: &'a [u8],
    /// Request profile schema.
    pub request_profile_schema: [u8; 32],
    /// Lifecycle policy bytes.
    pub lifecycle_bytes: &'a [u8],
    /// Transition program bytes.
    pub transition_bytes: &'a [u8],
    /// Effect program bytes.
    pub effect_bytes: &'a [u8],
    /// Effect schema.
    pub effect_schema: [u8; 32],
    /// Selected action.
    pub action: u32,
    /// Whether this is General PlaceOrder.
    pub general_place_order: bool,
    /// Whether General Verify forecast is required.
    pub general_verify_candidate: bool,
    /// Release-waist facts.
    pub waist: WaistFactsV1,
    /// Product-authenticated tail count.
    pub tail_count: u32,
    /// Family request bytes.
    pub family_request: &'a [u8],
    /// Whole Hot instruction data.
    pub instruction_data: &'a [u8],
    /// Optional Ed25519 evidence bytes.
    pub ed25519_evidence: Option<&'a [u8]>,
    /// Hot instruction index.
    pub native_message_instruction_index: u16,
    /// Trusted Clock slot.
    pub clock_slot: u64,
    /// Market identity.
    pub market: [u8; 32],
    /// Market generation.
    pub generation: u64,
    /// Logical account observations.
    pub observations: &'a [ObservedAccountV1],
    /// Content-projection keys.
    pub content_keys: ContentProjectionKeysV1,
    /// Authenticated dynamic span widths.
    pub span_counts: &'a [u32],
    /// Current Rent schedule.
    pub rent: &'a Rent,
}

/// Everything dynamic-span derivation consumes.
pub struct SpanWidthInputV1<'a> {
    /// Decoded account profile.
    pub profile: AccountProfileV2<'a>,
    /// Request profile bytes.
    pub request_profile_bytes: &'a [u8],
    /// Request profile schema.
    pub request_profile_schema: [u8; 32],
    /// Effect bytes.
    pub effect_bytes: &'a [u8],
    /// Effect schema.
    pub effect_schema: [u8; 32],
    /// Strategy bytes.
    pub strategy_bytes: &'a [u8],
    /// Release-waist facts.
    pub waist: WaistFactsV1,
    /// Product-authenticated tail count.
    pub tail_count: u32,
    /// Family request bytes.
    pub family_request: &'a [u8],
    /// Trusted Clock slot.
    pub clock_slot: u64,
}

/// Execute the production Hot projector through the fixture waist adapter.
pub fn run_engine(input: &EngineInputV1<'_>) -> Result<EngineOutputV1, BuilderError> {
    production::run_engine(&production_input(input)).map_err(map_error)
}

/// Execute the production Hot projector with the fixture's admitted callback.
pub(crate) fn run_engine_with_admitted_candidate(
    input: &EngineInputV1<'_>,
    candidate_projector: Option<&AdmittedCandidateProjectorV1<'_>>,
) -> Result<EngineOutputV1, BuilderError> {
    let callback_error = RefCell::new(None);
    let production_projector = candidate_projector.map(|projector| {
        |scalars: &mut [u64], identities: &mut [[u8; 32]]| {
            projector(scalars, identities).map_err(|error| {
                *callback_error.borrow_mut() = Some(error);
                production::HotProjectionErrorV1::Projection("candidate-projector")
            })
        }
    });
    let result = production::run_engine_with_admitted_candidate(
        &production_input(input),
        production_projector
            .as_ref()
            .map(|projector| projector as &production::AdmittedCandidateProjectorV1<'_>),
    );
    if let Some(error) = callback_error.into_inner() {
        return Err(error);
    }
    result.map_err(map_error)
}

fn production_input<'a>(input: &'a EngineInputV1<'a>) -> production::EngineInputV1<'a> {
    production::EngineInputV1 {
        profile: input.profile,
        request_profile_bytes: input.request_profile_bytes,
        request_profile_schema: input.request_profile_schema,
        lifecycle_bytes: input.lifecycle_bytes,
        transition_bytes: input.transition_bytes,
        effect_bytes: input.effect_bytes,
        effect_schema: input.effect_schema,
        action: input.action,
        general_place_order: input.general_place_order,
        general_verify_candidate: input.general_verify_candidate,
        waist: waist(input.waist),
        tail_count: input.tail_count,
        family_request: input.family_request,
        instruction_data: input.instruction_data,
        ed25519_evidence: input.ed25519_evidence,
        native_message_instruction_index: input.native_message_instruction_index,
        clock_slot: input.clock_slot,
        market: input.market,
        generation: input.generation,
        observations: input.observations,
        content_keys: input.content_keys,
        span_counts: input.span_counts,
        rent: input.rent,
    }
}

/// Derive dynamic widths through the production projector.
pub fn derive_dynamic_span_widths(input: &SpanWidthInputV1<'_>) -> Result<Vec<u32>, BuilderError> {
    production::derive_dynamic_span_widths(&span_input(input)).map_err(map_error)
}

/// Derive dynamic widths and their transport span through production code.
pub fn derive_dynamic_span_geometry(
    input: &SpanWidthInputV1<'_>,
) -> Result<SpanWidthDerivationV1, BuilderError> {
    production::derive_dynamic_span_geometry(&span_input(input)).map_err(map_error)
}

fn span_input<'a>(input: &'a SpanWidthInputV1<'a>) -> production::SpanWidthInputV1<'a> {
    production::SpanWidthInputV1 {
        profile: input.profile,
        request_profile_bytes: input.request_profile_bytes,
        request_profile_schema: input.request_profile_schema,
        effect_bytes: input.effect_bytes,
        effect_schema: input.effect_schema,
        strategy_bytes: input.strategy_bytes,
        waist: waist(input.waist),
        tail_count: input.tail_count,
        family_request: input.family_request,
        clock_slot: input.clock_slot,
    }
}

fn waist(value: WaistFactsV1) -> production::HotProjectionWaistV1 {
    production::HotProjectionWaistV1 {
        trading_program: value.trading_program,
        release_set: value.release_set,
    }
}

fn map_error(error: production::HotProjectionErrorV1) -> BuilderError {
    match error {
        production::HotProjectionErrorV1::Profile(line) => BuilderError::Profile(line),
        production::HotProjectionErrorV1::Projection(stage) => BuilderError::Projection(stage),
        production::HotProjectionErrorV1::Spans(stage) => BuilderError::Spans(stage),
        production::HotProjectionErrorV1::Lifecycle(stage) => BuilderError::Lifecycle(stage),
        production::HotProjectionErrorV1::Arithmetic => BuilderError::Arithmetic,
    }
}
