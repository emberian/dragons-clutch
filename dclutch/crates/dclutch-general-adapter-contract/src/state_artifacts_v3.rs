//! Action-selected General local-state lifecycle artifacts.
//!
//! These artifacts derive every nonroot state address from authenticated
//! General registers and let the generic Trading lifecycle adapter own the
//! canonical bump, current-Rent principal, immutable RentCredit beneficiary,
//! state key, and Trading owner.  Family evaluation never supplies those
//! protected values.

use dclutch_account_profile_contract::lifecycle_v3::{
    ACTION_PLAN_BYTES, CURRENT_RENT_QUOTE_BYTES_V5, HEADER_BYTES, IMMUTABLE_IDENTITY_BINDING_BYTES,
    PROTECTED_OUTPUT_BYTES, RECIPE_BYTES, SEED_BYTES, StateLifecyclePolicyV4,
    StateLifecyclePolicyV5,
    encode::{
        LifecycleAccountCoordinateV3, LifecycleCurrentRentQuoteInputV5, LifecycleGuardInputV3,
        LifecycleImmutableIdentityBindingInputV4, LifecycleOperationInputV3, LifecyclePlanInputV3,
        LifecycleProtectedOutputsInputV3, LifecycleRecipeInputV3, LifecycleRefundSourceInputV3,
        LifecycleRegisterCoordinateV3, LifecycleSeedInputV3, encode_lifecycle_policy_v4_atomic,
        encode_lifecycle_policy_v5_atomic,
    },
};
use dclutch_claims_svm::{
    liability_basis_state_v2::LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
    protocol_position_v2::PROTOCOL_POSITION_ADMISSION_BYTES_V2,
};
use dclutch_custody_contract::CUSTODY_REPLAY_BYTES_V1;
use dclutch_general_codec::Action;

use crate::{
    candidate_v1::GENERAL_CANDIDATE_BYTES_V1,
    collection_v1::{
        GENERAL_BATCH_BYTES_V1, GENERAL_ORDER_ROW_BASE_V1, GENERAL_ORDER_ROW_STRIDE_V1,
        GeneralBatchLayoutV1, GeneralOrderLayoutV1,
    },
    hot_candidate_v3::{identity, scalar},
    local_state_v3::{GENERAL_LOCAL_STATE_HEADER_BYTES_V3, GeneralLocalStateLayoutV3},
    release_v3::GENERAL_ACTIONS_V5,
    runtime_selection::{RUNTIME_SELECTION_CURSOR_BYTES_V2, RuntimeSelectionLayoutV2},
    runtime_verify::{RUNTIME_VERIFIER_HEADER_BYTES_V2, RuntimeVerifierLayoutV2},
    runtime_width::{
        SETTLEMENT_CURSOR_HEADER_BYTES_V2, SettlementCursorLayoutV2,
        VERIFIED_CANDIDATE_HEADER_BYTES_V2,
    },
    state_seeds_v3::{
        GENERAL_CANCEL_ORDER_SEED_START_V3, GENERAL_CANCEL_STATE_SEED_TABLE_V3,
        GENERAL_CLOSE_STATE_SEED_TABLE_V3, GENERAL_CLOSE_TERMINAL_SEED_START_V3,
        GENERAL_VERIFY_RESULT_SEED_START_V3, GENERAL_VERIFY_STATE_SEED_TABLE_V3,
        GENERAL_VERIFY_VERIFIER_SEED_START_V3, GeneralStateRecipeV3,
    },
};

/// First action-selected nonroot General state account.
pub const GENERAL_PRIMARY_STATE_ACCOUNT_V3: u16 = 5;
/// Close-only terminal record following the settlement state.
pub const GENERAL_TERMINAL_STATE_ACCOUNT_V3: u16 = 6;
/// Payer coordinate for every non-Close lifecycle.
pub const GENERAL_PRIMARY_PAYER_ACCOUNT_V3: u16 = 6;
/// RentCredit coordinate for every non-Close lifecycle.
pub const GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3: u16 = 7;
/// Close-only terminal-record payer coordinate.
pub const GENERAL_CLOSE_PAYER_ACCOUNT_V3: u16 = 7;
/// Shared Close terminal-create and settlement-close RentCredit coordinate.
pub const GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3: u16 = 8;
/// VerifyCandidateRow streamed verifier local state.
pub const GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3: u16 = 6;
/// VerifyCandidateRow raw conditional `VerifiedCandidateV2` result.
pub const GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3: u16 = 7;
/// Permissionless Verify caller and create payer.
pub const GENERAL_VERIFY_PAYER_ACCOUNT_V3: u16 = 8;
/// Permanent RentCredit for verifier/result state creation.
pub const GENERAL_VERIFY_RENT_CREDIT_ACCOUNT_V3: u16 = 9;

/// Semantic owner of one readonly General evaluator input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralReadonlyEvidenceKindV3 {
    /// Signed order terms: the exact identity-covered image of the record a
    /// PlaceOrder admission writes -- the 160-byte fixed header then the
    /// interleaved per-outcome rows, with no mutable window. The maker's
    /// transaction signature is what endorses these bytes; every register the
    /// admission consumes is projected from them, and the record the effect
    /// writes is therefore exactly what the maker signed.
    OrderTerms,
    /// Canonical closed Batch local-state image selected by the candidate.
    ClosedBatch,
    /// Runtime-width CandidateV2 content image whose own digest is submitted.
    CandidateImage,
    /// Immutable runtime-width Candidate page selected by the request.
    CandidatePage,
    /// Exact escrowed General order backing the selected execution row.
    EscrowedOrder,
    /// Canonical Submitted GeneralCandidateV1 body endorsed by the solver.
    SubmittedCandidate,
    /// Immutable interpreted best-valid-submitted-candidate policy.
    SelectionPolicy,
    /// Newly submitted complete verified-candidate record.
    SubmittedVerifiedCandidate,
    /// Frozen selection cursor naming the sole winning verified candidate.
    FrozenSelection,
    /// Terminal runtime verifier cursor used to initialize settlement.
    RuntimeVerifier,
    /// Selected complete verified-candidate record.
    SelectedVerifiedCandidate,
    /// Exact settlement order manifest emitted by candidate verification.
    SettlementManifest,
}

/// One action-selected readonly evidence account and its logical coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReadonlyEvidenceV3 {
    /// AccountProfile coordinate in the expanded logical runtime frame.
    pub coordinate: u16,
    /// Exact semantic record consumed by the stateless evaluator.
    pub kind: GeneralReadonlyEvidenceKindV3,
}

// The seed literals and the exact seed ORDER used to live here as four private
// constants and three inline tables. They now live in `state_seeds_v3`, which
// this module consumes, because a policy that gets one of them subtly wrong
// AUTHENTICATES and then derives the wrong addresses -- and a private constant
// forces every other side (the accelerator program-test did exactly this) to
// restate it. See that module's header for the full argument.

/// Stable refusal from General lifecycle-artifact generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralStateArtifactErrorV3 {
    /// A checked physical width overflowed.
    Geometry,
    /// The generic lifecycle semantic owner refused the candidate.
    Lifecycle(dclutch_account_profile_contract::lifecycle_v3::Error),
    /// The action is a declared protocol selector with no authored artifacts.
    UnauthoredAction,
}

/// Product/release-authenticated child widths needed by Initialize rent quotes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralChildRentWidthsV5 {
    /// Exact LiabilityBasis Position bytes, including the Product-N balance tail.
    pub position: u32,
    /// Exact selected Token or Token-2022 vault-account bytes.
    pub custody_vault: u32,
    /// Exact raw terminal `VerifiedCandidateV2` bytes for Product N.
    pub verified_candidate: u32,
}

impl GeneralChildRentWidthsV5 {
    /// Derive the exact Position width from Product N and join the selected vault width.
    pub fn new(outcome_count: u32, custody_vault: u32) -> Result<Self> {
        let position = u32::try_from(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2)
            .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?
            .checked_add(
                outcome_count
                    .checked_mul(8)
                    .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
            )
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?;
        let verified_candidate = u32::try_from(VERIFIED_CANDIDATE_HEADER_BYTES_V2)
            .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?
            .checked_add(
                outcome_count
                    .checked_mul(16)
                    .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
            )
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?;
        if outcome_count == 0 || custody_vault == 0 {
            return Err(GeneralStateArtifactErrorV3::Geometry);
        }
        Ok(Self {
            position,
            custody_vault,
            verified_candidate,
        })
    }
}

/// Result alias for General state artifacts.
pub type Result<T> = core::result::Result<T, GeneralStateArtifactErrorV3>;

/// Exact lifecycle artifact width for one General action.
pub fn general_state_lifecycle_bytes_v3(action: Action) -> Result<usize> {
    general_state_lifecycle_bytes(action, false)
}

/// Exact Lifecycle V5 artifact width for one successor General action.
pub fn general_state_lifecycle_bytes_v5(action: Action) -> Result<usize> {
    general_state_lifecycle_bytes(action, true)
}

fn general_state_lifecycle_bytes(action: Action, current_rent_v5: bool) -> Result<usize> {
    // An unauthored action had no plans, no seeds and no recipes, and so
    // computed a header-only width and an encodable header-only policy. The
    // artifact join refuses it downstream -- `action_plan_count` is zero -- but
    // an EMITTER that produces a lifecycle artifact for an action with no
    // triple is exactly the shape the seven were written out to prevent, and an
    // if/else on `Close` is a catch-all wearing a condition.
    require_authored(action)?;
    let (recipes, seeds, plans) = lifecycle_counts(action);
    recipes
        .checked_mul(RECIPE_BYTES)
        .and_then(|value| value.checked_add(seeds.checked_mul(SEED_BYTES)?))
        .and_then(|value| value.checked_add(plans.checked_mul(ACTION_PLAN_BYTES)?))
        .and_then(|value| value.checked_add(plans.checked_mul(PROTECTED_OUTPUT_BYTES)?))
        .and_then(|value| {
            value.checked_add(
                lifecycle_binding_count(action).checked_mul(IMMUTABLE_IDENTITY_BINDING_BYTES)?,
            )
        })
        .and_then(|value| {
            value.checked_add(
                if current_rent_v5 {
                    lifecycle_current_rent_quote_count(action)
                } else {
                    0
                }
                .checked_mul(CURRENT_RENT_QUOTE_BYTES_V5)?,
            )
        })
        .and_then(|value| HEADER_BYTES.checked_add(value))
        .ok_or(GeneralStateArtifactErrorV3::Geometry)
}

/// Number of exact readonly evaluator inputs selected by one action.
#[must_use]
pub const fn general_readonly_evidence_count_v3(action: Action) -> u16 {
    match action {
        Action::SubmitCandidate => 3,
        Action::VerifyCandidateRow => 5,
        Action::CloseCandidate => 1,
        // The batch pair's evaluator inputs are the Hot prefix itself: the
        // root tail and the config record, both projected by the
        // AccountProfile rather than read as evidence accounts. ReleaseOrder
        // is the same shape one state further in: its whole input is the
        // order record it refunds.
        Action::OpenBatch | Action::CloseBatch | Action::CancelOrder | Action::ReleaseOrder => 0,
        // The signed terms the admission projects everything from.
        Action::PlaceOrder => 1,
        Action::Consider => 2,
        // The batch whose selection window this freeze claims is over. See
        // `GeneralTransitionV3.lean`'s `.freeze` arm: the deadline is the
        // batch's own collection close plus the config's selection window, and
        // the batch record is the only account that carries the first term.
        Action::Freeze => 1,
        Action::InitializeSettlement => 3,
        Action::Collect | Action::Distribute => 2,
        Action::Materialize | Action::Close => 1,
    }
}

/// Return one exact readonly evaluator input.
pub fn general_readonly_evidence_v3(
    action: Action,
    index: u16,
) -> Result<GeneralReadonlyEvidenceV3> {
    let base = general_readonly_evidence_start_v3(action);
    let kind = match (action, index) {
        (Action::SubmitCandidate, 0) => GeneralReadonlyEvidenceKindV3::ClosedBatch,
        (Action::SubmitCandidate, 1) => GeneralReadonlyEvidenceKindV3::CandidateImage,
        (Action::SubmitCandidate, 2) => GeneralReadonlyEvidenceKindV3::SubmittedCandidate,
        (Action::VerifyCandidateRow, 0) => GeneralReadonlyEvidenceKindV3::ClosedBatch,
        (Action::VerifyCandidateRow, 1) => GeneralReadonlyEvidenceKindV3::CandidateImage,
        (Action::VerifyCandidateRow, 2) => GeneralReadonlyEvidenceKindV3::CandidatePage,
        (Action::VerifyCandidateRow, 3) => GeneralReadonlyEvidenceKindV3::EscrowedOrder,
        (Action::VerifyCandidateRow, 4) => GeneralReadonlyEvidenceKindV3::SettlementManifest,
        (Action::CloseCandidate, 0) => GeneralReadonlyEvidenceKindV3::ClosedBatch,
        (Action::Freeze, 0) => GeneralReadonlyEvidenceKindV3::ClosedBatch,
        (Action::Consider, 0) => GeneralReadonlyEvidenceKindV3::SelectionPolicy,
        (Action::Consider, 1) => GeneralReadonlyEvidenceKindV3::SubmittedVerifiedCandidate,
        (Action::InitializeSettlement, 0) => GeneralReadonlyEvidenceKindV3::FrozenSelection,
        (Action::InitializeSettlement, 1) => GeneralReadonlyEvidenceKindV3::RuntimeVerifier,
        (Action::InitializeSettlement, 2)
        | (Action::Collect | Action::Materialize | Action::Distribute | Action::Close, 0) => {
            GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate
        }
        (Action::Collect | Action::Distribute, 1) => {
            GeneralReadonlyEvidenceKindV3::SettlementManifest
        }
        (Action::PlaceOrder, 0) => GeneralReadonlyEvidenceKindV3::OrderTerms,
        _ => return Err(GeneralStateArtifactErrorV3::Geometry),
    };
    Ok(GeneralReadonlyEvidenceV3 {
        coordinate: base
            .checked_add(index)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
        kind,
    })
}

/// First readonly evidence coordinate after local lifecycle accounts.
#[must_use]
pub const fn general_readonly_evidence_start_v3(action: Action) -> u16 {
    general_system_program_base_v3(action)
}

/// Whether this action's profile carries the System program at all.
///
/// `CloseCandidate` alone declares `TrustedBuiltinIdentityV2::None`, so its
/// `RESULT_OWNER` register is never populated and a `RequireKey` against it
/// would be a guard nothing can satisfy -- which is what
/// `every_account_guard_names_a_register_the_input_bank_carries` refuses, and
/// it refused this on the first run. The account FOLLOWS the identity: the one
/// action with no System identity gets no System account, and the two halves
/// stay consistent rather than one of them being written for the other's sake.
pub const fn general_declares_system_program_v3(action: Action) -> bool {
    !matches!(action, Action::CloseCandidate)
}

/// The System-program coordinate: the LAST runtime account, or none.
///
/// APPENDED, not inserted. The first attempt put it at the end of the state
/// prefix, which is a perfectly good place for it semantically and moved every
/// evidence and child coordinate by one -- and the cost of that showed up
/// immediately as five harness failures whose assertions were about
/// coordinates, not about the System program at all. Appending moves NOTHING:
/// every existing coordinate keeps its index, and the only figures that change
/// are counts, which is the smallest true statement of what this adds.
pub const fn general_system_program_account_v3(action: Action) -> Option<u16> {
    if general_declares_system_program_v3(action) {
        Some(crate::effect_artifacts_v3::general_effect_account_count_before_system_v3(action))
    } else {
        None
    }
}

/// The System program, as an ACCOUNT rather than only as an identity.
///
/// WHY THIS COORDINATE EXISTS. `apply_lifecycle_creates_v3` invokes System to
/// allocate and assign every state a lifecycle plan creates, and it looks for
/// the program among the profile-declared RUNTIME accounts -- executable,
/// non-signer, non-writable. This profile declared
/// `TrustedBuiltinIdentityV2::SystemProgram`, which populates an identity
/// REGISTER, and named no account anywhere: a grep for `system` in
/// `account_rules_v3.rs` returned zero rules. So the release handed the
/// transition a System identity to compare against and never handed the commit
/// a System account to invoke, and every General action that creates state
/// refused `0x4005 Commit` at the first conjunct of that function. Measured
/// 2026-09-02 on OpenBatch at N=2: `system_present = 0` against twelve runtime
/// accounts.
///
/// Direct's ordinary profile has carried both halves since it was written --
/// `SYSTEM_PROGRAM_ACCOUNT` with an opaque executable rule and a `RequireKey`
/// against its trusted System identity -- and this is that pattern, not a new
/// convention.
///
/// PLACED LAST IN THE STATE PREFIX, immediately before the readonly evidence,
/// because that is the only position where no existing coordinate moves: the
/// named state constants above keep their values, and everything after it --
/// evidence, children, the effect account count, the profile's fixed count and
/// the scratch-page insertion point -- is DERIVED from this function and moves
/// with it.
const fn general_system_program_base_v3(action: Action) -> u16 {
    match action {
        Action::SubmitCandidate => 8,
        Action::VerifyCandidateRow => 10,
        Action::CloseCandidate => 8,
        // Two states, a payer and a rent credit before the children.
        Action::Close | Action::PlaceOrder | Action::CancelOrder => 9,
        Action::OpenBatch
        | Action::CloseBatch
        | Action::ReleaseOrder
        | Action::Consider
        | Action::Freeze
        | Action::InitializeSettlement
        | Action::Collect
        | Action::Materialize
        | Action::Distribute => 8,
    }
}

/// First child-route account after action-selected readonly evidence.
pub const fn general_child_account_start_v3(action: Action) -> u16 {
    general_readonly_evidence_start_v3(action) + general_readonly_evidence_count_v3(action)
}

/// The most recipes, plans or protected outputs one General action declares.
///
/// VerifyCandidateRow sets all three: Candidate, Verifier and the raw result.
pub const GENERAL_ACTION_MAX_RECIPES_V5: usize = 3;

/// The most immutable identity bindings one General action declares.
///
/// Consider and Freeze set it, at five apiece.
pub const GENERAL_ACTION_MAX_BINDINGS_V5: usize = 5;

/// The most current-Rent quotes one General action declares.
///
/// InitializeSettlement and PlaceOrder set it, at four apiece -- the escrow
/// Position, its admission record, the Custody replay, and the vault.
pub const GENERAL_ACTION_MAX_QUOTES_V5: usize = 4;

/// One General action's complete lifecycle declaration, before it is encoded.
///
/// WHY THIS TYPE EXISTS. Each of the five builders below used to encode the
/// arrays it had just built and return bytes, so the only way to ask what an
/// action declares was to encode a whole policy and decode it again. That was
/// adequate while every action had its own artifact; it is not adequate now,
/// because the family publishes ONE policy and the family builder has to
/// CONCATENATE fifteen of these with their table indices rebased. Returning the
/// declaration rather than its encoding keeps the per-action builders the sole
/// author of every value and gives the family builder something to join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralActionLifecycleShapeV5 {
    recipes: [LifecycleRecipeInputV3; GENERAL_ACTION_MAX_RECIPES_V5],
    recipe_count: usize,
    seeds: &'static [LifecycleSeedInputV3<'static>],
    plans: [LifecyclePlanInputV3; GENERAL_ACTION_MAX_RECIPES_V5],
    protected: [Option<LifecycleProtectedOutputsInputV3>; GENERAL_ACTION_MAX_RECIPES_V5],
    plan_count: usize,
    bindings: [LifecycleImmutableIdentityBindingInputV4; GENERAL_ACTION_MAX_BINDINGS_V5],
    binding_count: usize,
}

impl GeneralActionLifecycleShapeV5 {
    /// The action's PDA recipes, in canonical order.
    #[must_use]
    pub fn recipes(&self) -> &[LifecycleRecipeInputV3] {
        self.recipes.get(..self.recipe_count).unwrap_or(&[])
    }

    /// The action's sole seed table, whose windows its recipes index.
    #[must_use]
    pub const fn seeds(&self) -> &'static [LifecycleSeedInputV3<'static>] {
        self.seeds
    }

    /// The action's lifecycle plans, in canonical order.
    #[must_use]
    pub fn plans(&self) -> &[LifecyclePlanInputV3] {
        self.plans.get(..self.plan_count).unwrap_or(&[])
    }

    /// The protected outputs, one slot per plan.
    #[must_use]
    pub fn protected(&self) -> &[Option<LifecycleProtectedOutputsInputV3>] {
        self.protected.get(..self.plan_count).unwrap_or(&[])
    }

    /// The action's immutable identity bindings, in canonical order.
    #[must_use]
    pub fn bindings(&self) -> &[LifecycleImmutableIdentityBindingInputV4] {
        self.bindings.get(..self.binding_count).unwrap_or(&[])
    }
}

/// Collect one builder's arrays into the shape the family policy joins.
fn action_shape(
    recipes: &[LifecycleRecipeInputV3],
    seeds: &'static [LifecycleSeedInputV3<'static>],
    plans: &[LifecyclePlanInputV3],
    protected: &[Option<LifecycleProtectedOutputsInputV3>],
    bindings: &[LifecycleImmutableIdentityBindingInputV4],
) -> Result<GeneralActionLifecycleShapeV5> {
    let first_recipe = *recipes
        .first()
        .ok_or(GeneralStateArtifactErrorV3::Geometry)?;
    let first_plan = *plans.first().ok_or(GeneralStateArtifactErrorV3::Geometry)?;
    if recipes.len() > GENERAL_ACTION_MAX_RECIPES_V5
        || plans.len() > GENERAL_ACTION_MAX_RECIPES_V5
        || protected.len() != plans.len()
        || bindings.len() > GENERAL_ACTION_MAX_BINDINGS_V5
    {
        return Err(GeneralStateArtifactErrorV3::Geometry);
    }
    let mut shape = GeneralActionLifecycleShapeV5 {
        recipes: [first_recipe; GENERAL_ACTION_MAX_RECIPES_V5],
        recipe_count: recipes.len(),
        seeds,
        plans: [first_plan; GENERAL_ACTION_MAX_RECIPES_V5],
        protected: [None; GENERAL_ACTION_MAX_RECIPES_V5],
        plan_count: plans.len(),
        bindings: [EMPTY_BINDING_V4; GENERAL_ACTION_MAX_BINDINGS_V5],
        binding_count: bindings.len(),
    };
    for (slot, recipe) in recipes.iter().enumerate() {
        *shape
            .recipes
            .get_mut(slot)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)? = *recipe;
    }
    for (slot, plan) in plans.iter().enumerate() {
        *shape
            .plans
            .get_mut(slot)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)? = *plan;
        *shape
            .protected
            .get_mut(slot)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)? = *protected
            .get(slot)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?;
    }
    for (slot, binding) in bindings.iter().enumerate() {
        *shape
            .bindings
            .get_mut(slot)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)? = *binding;
    }
    Ok(shape)
}

/// The placeholder an unused binding slot holds; never encoded.
const EMPTY_BINDING_V4: LifecycleImmutableIdentityBindingInputV4 =
    LifecycleImmutableIdentityBindingInputV4 {
        plan: 0,
        data_offset: 0,
        canonical: LifecycleRegisterCoordinateV3::common(0),
    };

/// Read one General action's complete lifecycle declaration.
pub fn general_action_lifecycle_shape_v5(action: Action) -> Result<GeneralActionLifecycleShapeV5> {
    require_authored(action)?;
    if action == Action::Close {
        close_shape(action)
    } else if action == Action::CloseCandidate {
        close_candidate_shape(action)
    } else if action == Action::VerifyCandidateRow {
        verify_candidate_row_shape(action)
    } else if matches!(action, Action::PlaceOrder | Action::CancelOrder) {
        batch_and_order_shape(action)
    } else {
        primary_shape(action)
    }
}

/// Generate one complete protected-output lifecycle policy atomically.
pub fn encode_general_state_lifecycle_v3_atomic(
    action: Action,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let expected = general_state_lifecycle_bytes_v3(action)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(GeneralStateArtifactErrorV3::Geometry);
    }
    let shape = general_action_lifecycle_shape_v5(action)?;
    encode_lifecycle_policy_v4_atomic(
        shape.recipes(),
        shape.seeds(),
        shape.plans(),
        shape.protected(),
        shape.bindings(),
        scratch,
        output,
    )
    .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
    StateLifecyclePolicyV4::decode_selected([1; 32], [1; 32], output)
        .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
    Ok(())
}

/// Generate one complete General Lifecycle V5 policy with current-Rent quotes.
///
/// Only Initialize declares child-creation quotes. Other actions still select
/// the V5 schema with an empty quote table, preventing a V4 fallback in the
/// successor artifact chain.
///
/// THIS IS THE PER-ACTION ARTIFACT AND IT IS NO LONGER WHAT A RELEASE PUBLISHES.
/// It remains the single author of every value the family policy carries -- and
/// the control the family builder is proved against, action by action -- but a
/// Market selects [`encode_general_family_state_lifecycle_v5_atomic`], because a
/// capability manifest entry pins ONE `child_derivation_id` and fifteen policies
/// have fifteen digests.
pub fn encode_general_state_lifecycle_v5_atomic(
    action: Action,
    child_widths: Option<GeneralChildRentWidthsV5>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let expected = general_state_lifecycle_bytes_v5(action)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(GeneralStateArtifactErrorV3::Geometry);
    }
    let quotes = general_current_rent_quotes_v5(action, child_widths)?;
    let shape = general_action_lifecycle_shape_v5(action)?;
    encode_lifecycle_policy_v5_atomic(
        shape.recipes(),
        shape.seeds(),
        shape.plans(),
        shape.protected(),
        shape.bindings(),
        quotes.as_slice(action),
        scratch,
        output,
    )
    .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
    StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], output)
        .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
    Ok(())
}

/// Recipes, seeds, plans, bindings and quotes the ONE family policy carries.
///
/// The union over the fifteen actions, read off the same two count functions
/// each action's own artifact is sized from, so a policy width and a family
/// width can never disagree about what an action declares.
const fn general_family_lifecycle_counts_v5() -> (usize, usize, usize, usize, usize) {
    let mut recipes = 0;
    let mut seeds = 0;
    let mut plans = 0;
    let mut bindings = 0;
    let mut quotes = 0;
    let mut index = 0;
    while index < GENERAL_ACTIONS_V5.len() {
        let action = GENERAL_ACTIONS_V5[index];
        let (action_recipes, action_seeds, action_plans) = lifecycle_counts(action);
        recipes += action_recipes;
        seeds += action_seeds;
        plans += action_plans;
        bindings += lifecycle_binding_count(action);
        quotes += lifecycle_current_rent_quote_count(action);
        index += 1;
    }
    (recipes, seeds, plans, bindings, quotes)
}

/// Recipes in the family policy.
pub const GENERAL_FAMILY_RECIPE_COUNT_V5: usize = general_family_lifecycle_counts_v5().0;

/// Seeds in the family policy.
pub const GENERAL_FAMILY_SEED_COUNT_V5: usize = general_family_lifecycle_counts_v5().1;

/// Plans -- and therefore protected-output slots -- in the family policy.
pub const GENERAL_FAMILY_PLAN_COUNT_V5: usize = general_family_lifecycle_counts_v5().2;

/// Immutable identity bindings in the family policy.
pub const GENERAL_FAMILY_BINDING_COUNT_V5: usize = general_family_lifecycle_counts_v5().3;

/// Current-Rent quote declarations in the family policy.
pub const GENERAL_FAMILY_QUOTE_COUNT_V5: usize = general_family_lifecycle_counts_v5().4;

/// Exact width of the one lifecycle policy the General family publishes.
#[must_use]
pub const fn general_family_state_lifecycle_bytes_v5() -> usize {
    HEADER_BYTES
        + GENERAL_FAMILY_RECIPE_COUNT_V5 * RECIPE_BYTES
        + GENERAL_FAMILY_SEED_COUNT_V5 * SEED_BYTES
        + GENERAL_FAMILY_PLAN_COUNT_V5 * (ACTION_PLAN_BYTES + PROTECTED_OUTPUT_BYTES)
        + GENERAL_FAMILY_BINDING_COUNT_V5 * IMMUTABLE_IDENTITY_BINDING_BYTES
        + GENERAL_FAMILY_QUOTE_COUNT_V5 * CURRENT_RENT_QUOTE_BYTES_V5
}

/// One quote declaration together with the action that authored it.
#[cfg(not(target_os = "solana"))]
#[derive(Clone, Copy)]
struct FamilyQuoteV5 {
    value: LifecycleCurrentRentQuoteInputV5,
    order: (u16, u32),
}

/// Generate THE General family lifecycle policy: one artifact, fifteen actions.
///
/// # Off-chain only, and not by preference
///
/// A release compiler is authored by tooling and never by a program: the hot
/// path DECODES this artifact against the digest its descriptor names and has no
/// reason to build one. Compiling it for SBF anyway is not free -- the union's
/// six tables are about 5.9 KiB of stack in one frame, and `cargo-build-sbf`
/// emitted a stack-frame overwrite diagnostic for exactly this symbol, which the
/// frameguard ratchet refuses. `activation_bundle_v1` is gated for the same
/// reason one level up, at the manifest.
///
/// # Why one policy and not fifteen
///
/// `CapabilityProgramV4::validate_selection` requires the selected descriptor's
/// `derivation_policy` to equal the manifest entry's `child_derivation_id`, and
/// `artifacts_v3::validate_descriptor` requires that same `derivation_policy` to
/// be the digest of the descriptor's own lifecycle policy. A Market's capability
/// manifest holds ONE entry per capability root -- it is keyed by `kind_id` and
/// strictly ascending in it, `MAX_CAPABILITIES_V1` is 16, and
/// `CapabilityRootHeaderV1` persists ONE `entry_index` fixed at activation -- so
/// a root can bind exactly one such identity. Fifteen per-action policies have
/// fifteen digests, so a founded General Market could execute exactly one
/// action: cohort-15's activated, and its OpenBatch refused
/// `0x4015 DescriptorManifestEntry` after 128,724 CU on that conjunct alone.
///
/// This is the union of what the fifteen actions declare, joined the way Direct
/// joined its registered Sell and Buy. Every table is CONCATENATED with its
/// indices rebased rather than merged: an action's recipes keep their own seed
/// window, its plans keep their own recipes, its bindings keep their own plans.
/// Nothing here decides what an action declares; the per-action builders above
/// remain the sole author, and `general_action_lifecycle_shape_v5` is what this
/// reads.
///
/// # The two joins that are not concatenation
///
/// The quote table is sorted by `(destination, action)`, not by action, because
/// InitializeSettlement and PlaceOrder create the same four rent-bearing
/// children and therefore quote the same four registers. Each declaration
/// carries its action, so no other action projects rent for a child it never
/// opens and fourteen register banks are unchanged by this policy existing.
///
/// The per-action AccountProfile join is the caller's:
/// `StateLifecyclePolicyV5::validate_account_profile_for_action` is the only
/// sound reading of a policy whose actions present different frames, and General
/// presents fifteen -- fixed account counts run from 9 to 103.
#[cfg(not(target_os = "solana"))]
pub fn encode_general_family_state_lifecycle_v5_atomic(
    child_widths: GeneralChildRentWidthsV5,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let expected = general_family_state_lifecycle_bytes_v5();
    if scratch.len() != expected || output.len() != expected {
        return Err(GeneralStateArtifactErrorV3::Geometry);
    }
    let seed_placeholder = LifecycleSeedInputV3::CanonicalBump;
    let first = general_action_lifecycle_shape_v5(
        *GENERAL_ACTIONS_V5
            .first()
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
    )?;
    let mut recipes = [*first
        .recipes()
        .first()
        .ok_or(GeneralStateArtifactErrorV3::Geometry)?;
        GENERAL_FAMILY_RECIPE_COUNT_V5];
    let mut seeds = [seed_placeholder; GENERAL_FAMILY_SEED_COUNT_V5];
    let mut plans = [*first
        .plans()
        .first()
        .ok_or(GeneralStateArtifactErrorV3::Geometry)?;
        GENERAL_FAMILY_PLAN_COUNT_V5];
    let mut protected = [None; GENERAL_FAMILY_PLAN_COUNT_V5];
    let mut bindings = [EMPTY_BINDING_V4; GENERAL_FAMILY_BINDING_COUNT_V5];
    let mut quotes = [FamilyQuoteV5 {
        value: LifecycleCurrentRentQuoteInputV5 {
            exact_data_len: 0,
            scalar_destination: 0,
            action: None,
        },
        order: (0, 0),
    }; GENERAL_FAMILY_QUOTE_COUNT_V5];
    let (mut recipe_next, mut seed_next, mut plan_next, mut binding_next, mut quote_next) =
        (0_usize, 0_usize, 0_usize, 0_usize, 0_usize);

    for action in GENERAL_ACTIONS_V5 {
        let shape = general_action_lifecycle_shape_v5(action)?;
        let recipe_base = narrow_table_index(recipe_next)?;
        let seed_base = narrow_table_index(seed_next)?;
        let plan_base = narrow_table_index(plan_next)?;
        for recipe in shape.recipes() {
            let mut rebased = *recipe;
            rebased.seed_start = rebased
                .seed_start
                .checked_add(seed_base)
                .ok_or(GeneralStateArtifactErrorV3::Geometry)?;
            *recipes
                .get_mut(recipe_next)
                .ok_or(GeneralStateArtifactErrorV3::Geometry)? = rebased;
            recipe_next += 1;
        }
        for seed in shape.seeds() {
            *seeds
                .get_mut(seed_next)
                .ok_or(GeneralStateArtifactErrorV3::Geometry)? = *seed;
            seed_next += 1;
        }
        for (slot, plan) in shape.plans().iter().enumerate() {
            let mut rebased = *plan;
            rebased.recipe = rebased
                .recipe
                .checked_add(recipe_base)
                .ok_or(GeneralStateArtifactErrorV3::Geometry)?;
            *plans
                .get_mut(plan_next)
                .ok_or(GeneralStateArtifactErrorV3::Geometry)? = rebased;
            *protected
                .get_mut(plan_next)
                .ok_or(GeneralStateArtifactErrorV3::Geometry)? = *shape
                .protected()
                .get(slot)
                .ok_or(GeneralStateArtifactErrorV3::Geometry)?;
            plan_next += 1;
        }
        for binding in shape.bindings() {
            let mut rebased = *binding;
            rebased.plan = rebased
                .plan
                .checked_add(plan_base)
                .ok_or(GeneralStateArtifactErrorV3::Geometry)?;
            *bindings
                .get_mut(binding_next)
                .ok_or(GeneralStateArtifactErrorV3::Geometry)? = rebased;
            binding_next += 1;
        }
        let selected_widths = if matches!(
            action,
            Action::InitializeSettlement | Action::PlaceOrder | Action::VerifyCandidateRow
        ) {
            Some(child_widths)
        } else {
            None
        };
        let action_quotes = general_current_rent_quotes_v5(action, selected_widths)?;
        for quote in action_quotes.as_slice(action) {
            let mut scoped = *quote;
            scoped.action = Some(action as u32);
            *quotes
                .get_mut(quote_next)
                .ok_or(GeneralStateArtifactErrorV3::Geometry)? = FamilyQuoteV5 {
                value: scoped,
                order: (scoped.scalar_destination, action as u32),
            };
            quote_next += 1;
        }
    }
    if recipe_next != GENERAL_FAMILY_RECIPE_COUNT_V5
        || seed_next != GENERAL_FAMILY_SEED_COUNT_V5
        || plan_next != GENERAL_FAMILY_PLAN_COUNT_V5
        || binding_next != GENERAL_FAMILY_BINDING_COUNT_V5
        || quote_next != GENERAL_FAMILY_QUOTE_COUNT_V5
    {
        return Err(GeneralStateArtifactErrorV3::Geometry);
    }
    sort_family_quotes(&mut quotes);
    let mut quote_values = [LifecycleCurrentRentQuoteInputV5 {
        exact_data_len: 0,
        scalar_destination: 0,
        action: None,
    }; GENERAL_FAMILY_QUOTE_COUNT_V5];
    for (slot, quote) in quotes.iter().enumerate() {
        *quote_values
            .get_mut(slot)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)? = quote.value;
    }
    encode_lifecycle_policy_v5_atomic(
        &recipes,
        &seeds,
        &plans,
        &protected,
        &bindings,
        &quote_values,
        scratch,
        output,
    )
    .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
    StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], output)
        .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
    Ok(())
}

/// Order the family quote table by `(destination, action)`, in place.
#[cfg(not(target_os = "solana"))]
///
/// An insertion sort over nine entries, written out because this crate allocates
/// nothing and because the canonical order is a property of the artifact rather
/// than of whatever a library sort happens to do with equal keys -- there are no
/// equal keys, and the encoder refuses the policy if two ever appear.
fn sort_family_quotes(quotes: &mut [FamilyQuoteV5; GENERAL_FAMILY_QUOTE_COUNT_V5]) {
    let mut index = 1;
    while index < quotes.len() {
        let mut position = index;
        while position > 0 && quotes[position - 1].order > quotes[position].order {
            quotes.swap(position - 1, position);
            position -= 1;
        }
        index += 1;
    }
}

/// Narrow a family table offset to the width one encoded index declares.
#[cfg(not(target_os = "solana"))]
fn narrow_table_index(value: usize) -> Result<u16> {
    u16::try_from(value).map_err(|_| GeneralStateArtifactErrorV3::Geometry)
}

/// Whose rent each General state carries, decision 0021's byte five.
///
/// ONE AUTHOR FOR BOTH PLANS THAT NAME IT. The candidate's create and its close
/// have to agree -- the close re-reads the create's recorded answer and a
/// `Credit` close re-derives it from the market's wallet -- so the two are one
/// function rather than two literals that could drift apart in a patch.
///
/// EVERY GENERAL STATE BUT ONE IS A SHARED STRUCTURE OF THE MARKET. A batch
/// window, a selection cursor, a settlement cursor and its terminal records are
/// opened by whoever cranks them and belong to the market for as long as they
/// exist, which is exactly the case decision 0021 keeps on `Credit`: if the
/// rent followed whoever paid, a stranger cranking one open would walk away
/// owning the rent of something the market depends on.
///
/// THE CANDIDATE IS THE EXCEPTION AND ITS OWN CLOSE PLAN ALREADY SAID SO --
/// "the Candidate's immutable lifecycle beneficiary is the solver", written
/// beside a `Credit` tag that made it the market's wallet instead. A submission
/// is one solver's own object: the solver funds its rent and its whole work
/// escrow, its address is derived from the candidate they published, and
/// `project_general_submit_candidate_in_place_v3` requires
/// `identity::PRIMARY_BENEFICIARY == submitted_opening.solver_id`. Under
/// `Credit` the lifecycle preplan writes the market's RentCredit wallet into
/// that register, so the conjunct could hold only if the market's sponsor were
/// the solver -- which is to say SubmitCandidate was UNEXECUTABLE for every
/// permissionless solver, and measurably so: an instrumented replay of the
/// campaign's own bundle found that register "held the lifecycle's own
/// beneficiary rather than the solver" (`44c0ccf19`). `Payer` makes the
/// preplan write `*payer.key`, and the conjunct becomes the real check it
/// reads as: the account that paid IS the solver the record names, which the
/// transition states a second time as `identity_eq(PAYER, OWNER)`.
///
/// The Order recipe stays `Credit` deliberately. Nothing joins its beneficiary
/// to the maker today, and moving a refund identity with no conjunct asking for
/// it would be a change of economics with no reader.
const fn general_state_refund_source_v3(
    recipe: GeneralStateRecipeV3,
) -> LifecycleRefundSourceInputV3 {
    match recipe {
        GeneralStateRecipeV3::Candidate => LifecycleRefundSourceInputV3::Payer,
        GeneralStateRecipeV3::Selection
        | GeneralStateRecipeV3::Settlement
        | GeneralStateRecipeV3::Terminal
        | GeneralStateRecipeV3::Batch
        | GeneralStateRecipeV3::Order
        | GeneralStateRecipeV3::Verifier
        | GeneralStateRecipeV3::VerifiedCandidate => LifecycleRefundSourceInputV3::Credit,
    }
}

fn primary_shape(action: Action) -> Result<GeneralActionLifecycleShapeV5> {
    let state_recipe = GeneralStateRecipeV3::primary_for_action(action);
    // `data_base` is the account's fixed byte width; the stride is the
    // per-outcome tail past it. A fixed-width state carries a zero stride.
    let semantic_bytes = match state_recipe {
        GeneralStateRecipeV3::Selection => RUNTIME_SELECTION_CURSOR_BYTES_V2,
        GeneralStateRecipeV3::Settlement | GeneralStateRecipeV3::Terminal => {
            SETTLEMENT_CURSOR_HEADER_BYTES_V2
        }
        GeneralStateRecipeV3::Batch => GENERAL_BATCH_BYTES_V1,
        // The order record's fixed span: header and mutable window; the
        // per-outcome rows are the stride past it.
        GeneralStateRecipeV3::Order => GENERAL_ORDER_ROW_BASE_V1,
        GeneralStateRecipeV3::Candidate => GENERAL_CANDIDATE_BYTES_V1,
        GeneralStateRecipeV3::Verifier => RUNTIME_VERIFIER_HEADER_BYTES_V2,
        GeneralStateRecipeV3::VerifiedCandidate => VERIFIED_CANDIDATE_HEADER_BYTES_V2,
    };
    let data_base = u32::try_from(
        GENERAL_LOCAL_STATE_HEADER_BYTES_V3
            .checked_add(semantic_bytes)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
    )
    .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?;
    // The count and the bump ordinal are READ OFF the recipe's own seed table
    // rather than written down beside it, so a seed added to that table cannot
    // leave this policy declaring a truncated seed program.
    let recipe = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
        seed_start: 0,
        seed_count: state_recipe.seed_count(),
        bump_offset: state_recipe.bump_offset(),
        data_base,
        data_stride: match state_recipe {
            GeneralStateRecipeV3::Selection | GeneralStateRecipeV3::Batch => 0,
            GeneralStateRecipeV3::Candidate => 0,
            GeneralStateRecipeV3::Verifier => 40,
            GeneralStateRecipeV3::VerifiedCandidate => 16,
            GeneralStateRecipeV3::Order => order_row_stride()?,
            GeneralStateRecipeV3::Settlement | GeneralStateRecipeV3::Terminal => {
                SettlementCursorLayoutV2::inventory_stride()
            }
        },
    }];
    let plan = [LifecyclePlanInputV3 {
        action: action as u32,
        operation: LifecycleOperationInputV3::AuthenticateOrCreate,
        recipe: 0,
        payer: Some(LifecycleAccountCoordinateV3::fixed(
            GENERAL_PRIMARY_PAYER_ACCOUNT_V3,
        )),
        rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
            GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3,
        )),
        principal: Some(LifecycleRegisterCoordinateV3::common(scalar_u16(
            scalar::PRIMARY_PRINCIPAL_OBSERVATION,
        )?)),
        beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity_u16(
            identity::PRIMARY_BENEFICIARY_OBSERVATION,
        )?)),
        refund_source: general_state_refund_source_v3(state_recipe),
        guard: LifecycleGuardInputV3::Always,
    }];
    let protected = [Some(primary_protected()?)];
    let bindings = selection_or_settlement_bindings(action)?;
    let seeds = state_recipe.lifecycle_seeds();
    action_shape(&recipe, seeds, &plan, &protected, bindings.as_slice())
}

/// VerifyCandidateRow's three-state policy.
///
/// Candidate is an existing envelope, Verifier is the sole resumable envelope,
/// and Result is a raw immutable `VerifiedCandidateV2`. The result plan is the
/// only guarded plan: a nonterminal row cannot allocate it, while a terminal
/// row must create it at the exact current-Rent width declared by Lifecycle V5.
fn verify_candidate_row_shape(action: Action) -> Result<GeneralActionLifecycleShapeV5> {
    let candidate_recipe = GeneralStateRecipeV3::Candidate;
    let verifier_recipe = GeneralStateRecipeV3::Verifier;
    let result_recipe = GeneralStateRecipeV3::VerifiedCandidate;
    let candidate_base = u32::try_from(
        GENERAL_LOCAL_STATE_HEADER_BYTES_V3
            .checked_add(GENERAL_CANDIDATE_BYTES_V1)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
    )
    .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?;
    let verifier_base = u32::try_from(
        GENERAL_LOCAL_STATE_HEADER_BYTES_V3
            .checked_add(RUNTIME_VERIFIER_HEADER_BYTES_V2)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
    )
    .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?;
    let result_base = u32::try_from(VERIFIED_CANDIDATE_HEADER_BYTES_V2)
        .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?;
    let recipes = [
        LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
            seed_start: 0,
            seed_count: candidate_recipe.seed_count(),
            bump_offset: candidate_recipe.bump_offset(),
            data_base: candidate_base,
            data_stride: 0,
        },
        LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3),
            seed_start: GENERAL_VERIFY_VERIFIER_SEED_START_V3,
            seed_count: verifier_recipe.seed_count(),
            bump_offset: verifier_recipe.bump_offset(),
            data_base: verifier_base,
            data_stride: 40,
        },
        LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3),
            seed_start: GENERAL_VERIFY_RESULT_SEED_START_V3,
            seed_count: result_recipe.seed_count(),
            bump_offset: result_recipe.bump_offset(),
            data_base: result_base,
            data_stride: 16,
        },
    ];
    let plans = [
        LifecyclePlanInputV3 {
            action: action as u32,
            operation: LifecycleOperationInputV3::Authenticate,
            recipe: 0,
            payer: None,
            rent_credit: None,
            principal: None,
            beneficiary: None,
            refund_source: LifecycleRefundSourceInputV3::Credit,
            guard: LifecycleGuardInputV3::Always,
        },
        LifecyclePlanInputV3 {
            action: action as u32,
            operation: LifecycleOperationInputV3::Create,
            recipe: 2,
            payer: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_VERIFY_PAYER_ACCOUNT_V3,
            )),
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_VERIFY_RENT_CREDIT_ACCOUNT_V3,
            )),
            principal: Some(LifecycleRegisterCoordinateV3::common(scalar_u16(
                scalar::RESULT_PRINCIPAL_OBSERVATION,
            )?)),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity_u16(
                identity::RESULT_BENEFICIARY_OBSERVATION,
            )?)),
            refund_source: LifecycleRefundSourceInputV3::Credit,
            guard: LifecycleGuardInputV3::ScalarEq {
                source: LifecycleRegisterCoordinateV3::common(scalar_u16(scalar::VERIFY_TERMINAL)?),
                expected: 1,
            },
        },
        LifecyclePlanInputV3 {
            action: action as u32,
            operation: LifecycleOperationInputV3::AuthenticateOrCreate,
            recipe: 1,
            payer: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_VERIFY_PAYER_ACCOUNT_V3,
            )),
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_VERIFY_RENT_CREDIT_ACCOUNT_V3,
            )),
            principal: Some(LifecycleRegisterCoordinateV3::common(scalar_u16(
                scalar::TERMINAL_PRINCIPAL_OBSERVATION,
            )?)),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity_u16(
                identity::TERMINAL_BENEFICIARY_OBSERVATION,
            )?)),
            refund_source: LifecycleRefundSourceInputV3::Credit,
            guard: LifecycleGuardInputV3::Always,
        },
    ];
    // Lifecycle policy order is canonical by operation tag, so Result/Create
    // precedes Verifier/AuthenticateOrCreate even though its state coordinate
    // follows Verifier in the physical frame.
    let protected = [None, None, Some(terminal_protected()?)];
    // Only AuthenticateOrCreate may carry an immutable binding. Candidate is
    // a mandatory Authenticate plan and its content joins in the evaluator;
    // Verifier binds its immutable candidate field whenever the live branch is
    // selected, while creation writes that same canonical field atomically.
    let bindings = [binding(
        2,
        RuntimeVerifierLayoutV2::candidate_id(),
        identity::CANDIDATE,
    )?];
    action_shape(
        &recipes,
        &GENERAL_VERIFY_STATE_SEED_TABLE_V3,
        &plans,
        &protected,
        &bindings,
    )
}

/// CloseCandidate's single-state close policy.
///
/// The Candidate's immutable lifecycle beneficiary is the solver. Effects pay
/// the cleanup crank and return the unspent verification compartment first;
/// this Close plan then returns only the historical rent principal to that
/// same solver and leaves the Candidate account vacant.
fn close_candidate_shape(action: Action) -> Result<GeneralActionLifecycleShapeV5> {
    let recipe_kind = GeneralStateRecipeV3::Candidate;
    let data_base = u32::try_from(
        GENERAL_LOCAL_STATE_HEADER_BYTES_V3
            .checked_add(GENERAL_CANDIDATE_BYTES_V1)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
    )
    .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?;
    let recipes = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
        seed_start: 0,
        seed_count: recipe_kind.seed_count(),
        bump_offset: recipe_kind.bump_offset(),
        data_base,
        data_stride: 0,
    }];
    let plans = [LifecyclePlanInputV3 {
        action: action as u32,
        operation: LifecycleOperationInputV3::Close,
        recipe: 0,
        payer: None,
        rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
            GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3,
        )),
        principal: Some(LifecycleRegisterCoordinateV3::common(scalar_u16(
            scalar::PRIMARY_PRINCIPAL_OBSERVATION,
        )?)),
        beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity_u16(
            identity::PRIMARY_BENEFICIARY_OBSERVATION,
        )?)),
        refund_source: general_state_refund_source_v3(recipe_kind),
        guard: LifecycleGuardInputV3::Always,
    }];
    // A Close plan consumes authenticated observations and emits no lifecycle
    // protected outputs. Those outputs are canonical only for
    // AuthenticateOrCreate; carrying them here made the first-class
    // CloseCandidate V5 artifact unencodable.
    let protected = [None];
    // Lifecycle immutable bindings are defined only for
    // AuthenticateOrCreate protected outputs. CloseCandidate is a Close plan;
    // its Candidate/solver agreement is authenticated by the account-profile
    // projections and semantic transition before this plan runs, while its PDA
    // identity is rederived from the canonical seed table here.
    let bindings = [];
    action_shape(
        &recipes,
        recipe_kind.lifecycle_seeds(),
        &plans,
        &protected,
        &bindings,
    )
}

fn close_shape(action: Action) -> Result<GeneralActionLifecycleShapeV5> {
    let settlement_base = u32::try_from(
        GENERAL_LOCAL_STATE_HEADER_BYTES_V3
            .checked_add(SETTLEMENT_CURSOR_HEADER_BYTES_V2)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
    )
    .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?;
    // Close closes the settlement state and creates the terminal record, so it
    // declares two recipes over one seed table. Both windows, the table, and
    // the offset where the second window begins all come from `state_seeds_v3`
    // -- writing the eleven-entry table out here again is what made the seed
    // order restatable in the first place.
    let settlement_recipe = GeneralStateRecipeV3::Settlement;
    let terminal_recipe = GeneralStateRecipeV3::Terminal;
    let recipes = [
        LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
            seed_start: 0,
            seed_count: settlement_recipe.seed_count(),
            bump_offset: settlement_recipe.bump_offset(),
            data_base: settlement_base,
            data_stride: SettlementCursorLayoutV2::inventory_stride(),
        },
        LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(GENERAL_TERMINAL_STATE_ACCOUNT_V3),
            seed_start: GENERAL_CLOSE_TERMINAL_SEED_START_V3,
            seed_count: terminal_recipe.seed_count(),
            bump_offset: terminal_recipe.bump_offset(),
            data_base: settlement_base,
            data_stride: SettlementCursorLayoutV2::inventory_stride(),
        },
    ];
    // Same action entries remain canonical by operation tag: Close precedes
    // AuthenticateOrCreate. Creation is nevertheless applied before Effects
    // by the generic Trading lifecycle adapter.
    let plans = [
        LifecyclePlanInputV3 {
            action: action as u32,
            operation: LifecycleOperationInputV3::Close,
            recipe: 0,
            payer: None,
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3,
            )),
            principal: Some(LifecycleRegisterCoordinateV3::common(scalar_u16(
                scalar::PRIMARY_PRINCIPAL_OBSERVATION,
            )?)),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity_u16(
                identity::PRIMARY_BENEFICIARY_OBSERVATION,
            )?)),
            refund_source: LifecycleRefundSourceInputV3::Credit,
            guard: LifecycleGuardInputV3::Always,
        },
        LifecyclePlanInputV3 {
            action: action as u32,
            operation: LifecycleOperationInputV3::AuthenticateOrCreate,
            recipe: 1,
            payer: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_CLOSE_PAYER_ACCOUNT_V3,
            )),
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3,
            )),
            principal: Some(LifecycleRegisterCoordinateV3::common(scalar_u16(
                scalar::TERMINAL_PRINCIPAL_OBSERVATION,
            )?)),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity_u16(
                identity::TERMINAL_BENEFICIARY_OBSERVATION,
            )?)),
            refund_source: LifecycleRefundSourceInputV3::Credit,
            guard: LifecycleGuardInputV3::Always,
        },
    ];
    let protected = [None, Some(terminal_protected()?)];
    let bindings = close_bindings()?;
    action_shape(
        &recipes,
        &GENERAL_CLOSE_STATE_SEED_TABLE_V3,
        &plans,
        &protected,
        &bindings,
    )
}

/// CancelOrder's two-recipe policy: the batch window and the order record.
///
/// The shape is `encode_close` with both plans authenticating rather than one
/// closing: recipe zero is the batch at the primary coordinate, recipe one the
/// order at the secondary, over one combined ten-entry seed table so neither
/// window's order can be restated.
fn batch_and_order_shape(action: Action) -> Result<GeneralActionLifecycleShapeV5> {
    let batch_recipe = GeneralStateRecipeV3::Batch;
    let order_recipe = GeneralStateRecipeV3::Order;
    let batch_base = u32::try_from(
        GENERAL_LOCAL_STATE_HEADER_BYTES_V3
            .checked_add(GENERAL_BATCH_BYTES_V1)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
    )
    .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?;
    let order_base = u32::try_from(
        GENERAL_LOCAL_STATE_HEADER_BYTES_V3
            .checked_add(GENERAL_ORDER_ROW_BASE_V1)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
    )
    .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?;
    let recipes = [
        LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
            seed_start: 0,
            seed_count: batch_recipe.seed_count(),
            bump_offset: batch_recipe.bump_offset(),
            data_base: batch_base,
            data_stride: 0,
        },
        LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(GENERAL_TERMINAL_STATE_ACCOUNT_V3),
            seed_start: GENERAL_CANCEL_ORDER_SEED_START_V3,
            seed_count: order_recipe.seed_count(),
            bump_offset: order_recipe.bump_offset(),
            data_base: order_base,
            data_stride: order_row_stride()?,
        },
    ];
    let plans = [
        LifecyclePlanInputV3 {
            action: action as u32,
            operation: LifecycleOperationInputV3::AuthenticateOrCreate,
            recipe: 0,
            payer: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_CLOSE_PAYER_ACCOUNT_V3,
            )),
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3,
            )),
            principal: Some(LifecycleRegisterCoordinateV3::common(scalar_u16(
                scalar::PRIMARY_PRINCIPAL_OBSERVATION,
            )?)),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity_u16(
                identity::PRIMARY_BENEFICIARY_OBSERVATION,
            )?)),
            refund_source: LifecycleRefundSourceInputV3::Credit,
            guard: LifecycleGuardInputV3::Always,
        },
        LifecyclePlanInputV3 {
            action: action as u32,
            operation: LifecycleOperationInputV3::AuthenticateOrCreate,
            recipe: 1,
            payer: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_CLOSE_PAYER_ACCOUNT_V3,
            )),
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3,
            )),
            principal: Some(LifecycleRegisterCoordinateV3::common(scalar_u16(
                scalar::TERMINAL_PRINCIPAL_OBSERVATION,
            )?)),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity_u16(
                identity::TERMINAL_BENEFICIARY_OBSERVATION,
            )?)),
            refund_source: LifecycleRefundSourceInputV3::Credit,
            guard: LifecycleGuardInputV3::Always,
        },
    ];
    let protected = [Some(primary_protected()?), Some(terminal_protected()?)];
    let bindings = selection_or_settlement_bindings(action)?;
    action_shape(
        &recipes,
        &GENERAL_CANCEL_STATE_SEED_TABLE_V3,
        &plans,
        &protected,
        bindings.as_slice(),
    )
}

struct CurrentRentQuoteBufferV5 {
    values: [LifecycleCurrentRentQuoteInputV5; 4],
}

impl CurrentRentQuoteBufferV5 {
    fn as_slice(&self, action: Action) -> &[LifecycleCurrentRentQuoteInputV5] {
        if matches!(action, Action::InitializeSettlement | Action::PlaceOrder) {
            &self.values
        } else if action == Action::VerifyCandidateRow {
            &self.values[..1]
        } else {
            &[]
        }
    }
}

fn general_current_rent_quotes_v5(
    action: Action,
    child_widths: Option<GeneralChildRentWidthsV5>,
) -> Result<CurrentRentQuoteBufferV5> {
    let child_widths = match (action, child_widths) {
        (
            Action::InitializeSettlement | Action::PlaceOrder | Action::VerifyCandidateRow,
            Some(widths),
        ) if widths.position != 0 && widths.custody_vault != 0 => widths,
        (Action::InitializeSettlement | Action::PlaceOrder | Action::VerifyCandidateRow, _)
        | (_, Some(_)) => {
            return Err(GeneralStateArtifactErrorV3::Geometry);
        }
        (_, None) => GeneralChildRentWidthsV5 {
            // These inactive values are never serialized because every other
            // action has an empty V5 quote table.
            position: 0,
            custody_vault: 0,
            verified_candidate: 0,
        },
    };
    Ok(CurrentRentQuoteBufferV5 {
        values: [
            LifecycleCurrentRentQuoteInputV5 {
                exact_data_len: if action == Action::VerifyCandidateRow {
                    child_widths.verified_candidate
                } else {
                    child_widths.position
                },
                scalar_destination: scalar_u16(if action == Action::VerifyCandidateRow {
                    scalar::RESULT_PRINCIPAL_OBSERVATION
                } else {
                    scalar::POSITION_RENT_PRINCIPAL
                })?,
                action: None,
            },
            LifecycleCurrentRentQuoteInputV5 {
                exact_data_len: u32::try_from(PROTOCOL_POSITION_ADMISSION_BYTES_V2)
                    .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?,
                scalar_destination: scalar_u16(scalar::ADMISSION_RENT_PRINCIPAL)?,
                action: None,
            },
            LifecycleCurrentRentQuoteInputV5 {
                exact_data_len: u32::try_from(CUSTODY_REPLAY_BYTES_V1)
                    .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?,
                scalar_destination: scalar_u16(scalar::CUSTODY_REPLAY_RENT_LAMPORTS)?,
                action: None,
            },
            LifecycleCurrentRentQuoteInputV5 {
                exact_data_len: child_widths.custody_vault,
                scalar_destination: scalar_u16(scalar::CUSTODY_VAULT_RENT_LAMPORTS)?,
                action: None,
            },
        ],
    })
}

const fn lifecycle_current_rent_quote_count(action: Action) -> usize {
    match action {
        Action::SubmitCandidate => 0,
        Action::VerifyCandidateRow => 1,
        Action::CloseCandidate => 0,
        Action::InitializeSettlement => 4,
        Action::OpenBatch
        | Action::CloseBatch
        | Action::CancelOrder
        | Action::ReleaseOrder
        | Action::Consider
        | Action::Freeze
        | Action::Collect
        | Action::Materialize
        | Action::Distribute
        | Action::Close => 0,
        // The admission creates the same four rent-bearing children the
        // settlement initialization does: the escrow Position, its admission
        // record, the Custody replay, and the vault.
        Action::PlaceOrder => 4,
    }
}

fn primary_protected() -> Result<LifecycleProtectedOutputsInputV3> {
    Ok(LifecycleProtectedOutputsInputV3 {
        created: scalar_u16(scalar::PRIMARY_CREATED)?,
        bump_observation: scalar_u16(scalar::PRIMARY_BUMP_OBSERVATION)?,
        bump: scalar_u16(scalar::PRIMARY_CANONICAL_BUMP)?,
        historical_rent_principal: scalar_u16(scalar::PRIMARY_RENT_PRINCIPAL)?,
        beneficiary: identity_u16(identity::PRIMARY_BENEFICIARY)?,
        state: identity_u16(identity::PRIMARY_STATE)?,
        owner: identity_u16(identity::PRIMARY_OWNER)?,
    })
}

fn terminal_protected() -> Result<LifecycleProtectedOutputsInputV3> {
    Ok(LifecycleProtectedOutputsInputV3 {
        created: scalar_u16(scalar::TERMINAL_CREATED)?,
        bump_observation: scalar_u16(scalar::TERMINAL_BUMP_OBSERVATION)?,
        bump: scalar_u16(scalar::TERMINAL_CANONICAL_BUMP)?,
        historical_rent_principal: scalar_u16(scalar::TERMINAL_RENT_PRINCIPAL)?,
        beneficiary: identity_u16(identity::TERMINAL_BENEFICIARY)?,
        state: identity_u16(identity::TERMINAL_STATE)?,
        owner: identity_u16(identity::TERMINAL_OWNER)?,
    })
}

struct BindingBufferV4 {
    values: [LifecycleImmutableIdentityBindingInputV4; 5],
    len: usize,
}

impl BindingBufferV4 {
    fn as_slice(&self) -> &[LifecycleImmutableIdentityBindingInputV4] {
        self.values.get(..self.len).unwrap_or(&[])
    }
}

fn selection_or_settlement_bindings(action: Action) -> Result<BindingBufferV4> {
    let empty = LifecycleImmutableIdentityBindingInputV4 {
        plan: 0,
        data_offset: 0,
        canonical: LifecycleRegisterCoordinateV3::common(0),
    };
    let mut output = BindingBufferV4 {
        values: [empty; 5],
        len: lifecycle_binding_count(action),
    };
    if action == Action::SubmitCandidate {
        return Ok(output);
    }
    if matches!(action, Action::OpenBatch | Action::CloseBatch) {
        // The batch record's three immutable identities, bound to the
        // canonical registers the AccountProfile projects from the root, the
        // Product record, and the root's config selection. A live batch whose
        // bytes disagree with any of the three is a substituted window.
        for (slot, (offset, canonical)) in [
            (GeneralBatchLayoutV1::MARKET, identity::MARKET),
            (
                GeneralBatchLayoutV1::PRODUCT_ID,
                identity::SELECTION_PRODUCT,
            ),
            (GeneralBatchLayoutV1::CONFIG_ID, identity::GENERAL_CONFIG_ID),
        ]
        .into_iter()
        .enumerate()
        {
            output.values[slot] = binding(
                0,
                u32::try_from(offset).map_err(|_| GeneralStateArtifactErrorV3::Geometry)?,
                canonical,
            )?;
        }
        return Ok(output);
    }
    if action == Action::ReleaseOrder {
        output.values[0] = binding(
            0,
            u32::try_from(GeneralOrderLayoutV1::MARKET)
                .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?,
            identity::MARKET,
        )?;
        return Ok(output);
    }
    if action == Action::PlaceOrder {
        for (slot, (offset, canonical)) in [
            (GeneralBatchLayoutV1::MARKET, identity::MARKET),
            (
                GeneralBatchLayoutV1::PRODUCT_ID,
                identity::SELECTION_PRODUCT,
            ),
            (GeneralBatchLayoutV1::CONFIG_ID, identity::GENERAL_CONFIG_ID),
        ]
        .into_iter()
        .enumerate()
        {
            output.values[slot] = binding(
                0,
                u32::try_from(offset).map_err(|_| GeneralStateArtifactErrorV3::Geometry)?,
                canonical,
            )?;
        }
        return Ok(output);
    }
    if action == Action::CancelOrder {
        // The batch record's three independently-sourced identities on the
        // batch plan, exactly as the batch pair binds them, and the order
        // record's Market on the order plan. The order's batch bytes need no
        // binding: the batch address itself is DERIVED from the register the
        // profile projects out of those bytes, so a substituted window cannot
        // even be presented.
        for (slot, (offset, canonical)) in [
            (GeneralBatchLayoutV1::MARKET, identity::MARKET),
            (
                GeneralBatchLayoutV1::PRODUCT_ID,
                identity::SELECTION_PRODUCT,
            ),
            (GeneralBatchLayoutV1::CONFIG_ID, identity::GENERAL_CONFIG_ID),
        ]
        .into_iter()
        .enumerate()
        {
            output.values[slot] = binding(
                0,
                u32::try_from(offset).map_err(|_| GeneralStateArtifactErrorV3::Geometry)?,
                canonical,
            )?;
        }
        output.values[3] = binding(
            1,
            u32::try_from(GeneralOrderLayoutV1::MARKET)
                .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?,
            identity::MARKET,
        )?;
        return Ok(output);
    }
    if matches!(action, Action::Consider | Action::Freeze) {
        for (slot, (body_offset, canonical)) in [
            (
                RuntimeSelectionLayoutV2::product_id(),
                identity::SELECTION_PRODUCT,
            ),
            (
                RuntimeSelectionLayoutV2::batch_id(),
                identity::SELECTION_BATCH,
            ),
            (
                RuntimeSelectionLayoutV2::policy_id(),
                identity::SELECTION_POLICY,
            ),
            (
                RuntimeSelectionLayoutV2::best_candidate_id(),
                identity::CANDIDATE,
            ),
            (
                RuntimeSelectionLayoutV2::best_verified_digest(),
                identity::BEST_VERIFIED_DIGEST,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            output.values[slot] = binding(0, body_offset, canonical)?;
        }
    } else {
        output.values[0] = binding(
            0,
            SettlementCursorLayoutV2::candidate_id(),
            identity::CANDIDATE,
        )?;
    }
    Ok(output)
}

fn close_bindings() -> Result<[LifecycleImmutableIdentityBindingInputV4; 1]> {
    Ok([binding(
        1,
        SettlementCursorLayoutV2::candidate_id(),
        identity::CANDIDATE,
    )?])
}

fn order_row_stride() -> Result<u32> {
    u32::try_from(GENERAL_ORDER_ROW_STRIDE_V1).map_err(|_| GeneralStateArtifactErrorV3::Geometry)
}

fn binding(
    plan: u16,
    body_offset: u32,
    canonical: u32,
) -> Result<LifecycleImmutableIdentityBindingInputV4> {
    Ok(LifecycleImmutableIdentityBindingInputV4 {
        plan,
        data_offset: GeneralLocalStateLayoutV3::body()
            .checked_add(body_offset)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
        canonical: LifecycleRegisterCoordinateV3::common(identity_u16(canonical)?),
    })
}

/// Refuse one action whose artifact triple has not been authored.
const fn require_authored(action: Action) -> Result<()> {
    if crate::effect_artifacts_v3::general_action_artifacts_authored_v3(action) {
        Ok(())
    } else {
        Err(GeneralStateArtifactErrorV3::UnauthoredAction)
    }
}

const fn lifecycle_counts(action: Action) -> (usize, usize, usize) {
    match action {
        Action::SubmitCandidate => (1, 4, 1),
        Action::VerifyCandidateRow => (3, 12, 3),
        Action::CloseCandidate => (1, 4, 1),
        Action::OpenBatch | Action::CloseBatch | Action::ReleaseOrder => (1, 5, 1),
        // Two recipes over one ten-entry table: the batch window and the
        // order record.
        Action::PlaceOrder | Action::CancelOrder => (2, 10, 2),
        // Five since 2026-09-04: the selection recipe is keyed by the batch
        // identity as well as the root, so a market can select in more than one
        // batch. See `GENERAL_SELECTION_STATE_RECIPE_V3`.
        Action::Consider | Action::Freeze => (1, 5, 1),
        Action::InitializeSettlement
        | Action::Collect
        | Action::Materialize
        | Action::Distribute => (1, 5, 1),
        Action::Close => (2, 11, 2),
    }
}

const fn lifecycle_binding_count(action: Action) -> usize {
    match action {
        Action::SubmitCandidate => 0,
        Action::VerifyCandidateRow => 1,
        Action::CloseCandidate => 0,
        Action::OpenBatch | Action::CloseBatch => 3,
        // The order record's Market alone: it is the sole coordinate with an
        // INDEPENDENT frame source (the root tail). The owner and batch bytes
        // have no second authority in this frame -- the record itself is what
        // fills their registers -- so binding them would compare a value to
        // itself.
        Action::ReleaseOrder => 1,
        // The batch record's three independent identities on plan zero, and
        // the order record's Market on plan one.
        Action::CancelOrder => 4,
        // The batch trio alone: the order record is VACANT at admission, so a
        // byte binding against it would refuse the create it guards.
        Action::PlaceOrder => 3,
        Action::Consider | Action::Freeze => 5,
        Action::InitializeSettlement
        | Action::Collect
        | Action::Materialize
        | Action::Distribute => 1,
        Action::Close => 1,
    }
}

fn scalar_u16(value: u32) -> Result<u16> {
    u16::try_from(value).map_err(|_| GeneralStateArtifactErrorV3::Geometry)
}

fn identity_u16(value: u32) -> Result<u16> {
    u16::try_from(value).map_err(|_| GeneralStateArtifactErrorV3::Geometry)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::*;

    const ACTIONS: [Action; 15] = [
        Action::Consider,
        Action::Freeze,
        Action::InitializeSettlement,
        Action::Collect,
        Action::Materialize,
        Action::Distribute,
        Action::Close,
        Action::OpenBatch,
        Action::PlaceOrder,
        Action::CancelOrder,
        Action::CloseBatch,
        Action::SubmitCandidate,
        Action::VerifyCandidateRow,
        Action::ReleaseOrder,
        Action::CloseCandidate,
    ];

    #[test]
    fn all_actions_encode_exact_canonical_lifecycle_geometry() {
        for action in ACTIONS {
            let width = general_state_lifecycle_bytes_v3(action).expect("width");
            let mut scratch = vec![0_u8; width];
            let mut output = vec![0x55_u8; width];
            encode_general_state_lifecycle_v3_atomic(action, &mut scratch, &mut output)
                .expect("lifecycle artifact");
            let policy =
                StateLifecyclePolicyV4::decode_selected([1; 32], [1; 32], &output).expect("decode");
            let two_state = matches!(
                action,
                Action::Close
                    | Action::PlaceOrder
                    | Action::CancelOrder
                    | Action::VerifyCandidateRow
            );
            assert_eq!(
                policy.action_plan_count(action as u32),
                Ok(if action == Action::VerifyCandidateRow {
                    3
                } else if two_state {
                    2
                } else {
                    1
                })
            );
            assert_eq!(
                policy
                    .action_plan(action as u32, u16::from(two_state))
                    .expect("selected")
                    .uses_canonical_bump(),
                Ok(true)
            );
        }
    }

    /// THE FAMILY POLICY IS THE UNION, AND ONLY THE PER-ACTION JOIN READS IT.
    ///
    /// Two facts, stated together because either alone is misleading. The union
    /// carries every action's declaration -- so its per-action plan and quote
    /// counts must equal exactly what that action's own artifact declares, or
    /// the union changed something on the way in. And the WHOLE-POLICY join must
    /// REFUSE it: General's Selection, Settlement, Batch, Order and Candidate
    /// recipes all name fixed slot 5, so asking whether all twenty recipes fit
    /// one action's frame is a question with no true answer, and a join that
    /// said yes would be a check that cannot fail.
    ///
    /// The second half is why `artifacts_v3` and `hot_v3/seal.rs` use
    /// `validate_account_profile_for_action`, and why a caller that selects a
    /// plan without attaching the action-scoped evidence gets the wrong answer
    /// from `require_join`'s fallback rather than no answer.
    #[test]
    fn the_family_policy_joins_per_action_and_the_whole_policy_form_refuses_it() {
        use dclutch_account_profile_contract::v2::AccountProfileV2;

        use crate::account_rules_v3::{
            GeneralExternalAccountWidthsV3, encode_general_account_profile_v3_atomic,
            general_account_profile_bytes_v3,
        };

        // The eleven widths a release publishes, as the compiler derives them;
        // this test cares only that they are the same for every action.
        let widths = GeneralExternalAccountWidthsV3 {
            linked_basis_prefix: 256,
            result_domain: 192,
            rent_sysvar: 17,
            core_market: 368,
            activation_cache: 1_288,
            upgradeable_program: 36,
            trading_programdata_prefix: 45,
            claims_programdata_prefix: 45,
            core_programdata_prefix: 45,
            realm_record: 112,
            rent_credit: 128,
        };
        let child = GeneralChildRentWidthsV5::new(4, 165).expect("child widths");
        let family_width = general_family_state_lifecycle_bytes_v5();
        let mut scratch = vec![0_u8; family_width];
        let mut family = vec![0_u8; family_width];
        encode_general_family_state_lifecycle_v5_atomic(child, &mut scratch, &mut family)
            .expect("family policy");
        let policy =
            StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &family).expect("decode");

        let mut refused = 0_usize;
        for action in ACTIONS {
            let profile_width = general_account_profile_bytes_v3(action).expect("profile width");
            let mut profile_scratch = vec![0_u8; profile_width];
            let mut profile_bytes = vec![0_u8; profile_width];
            encode_general_account_profile_v3_atomic(
                action,
                widths,
                &mut profile_scratch,
                &mut profile_bytes,
            )
            .expect("account profile");
            let profile = AccountProfileV2::decode(&profile_bytes).expect("profile");

            assert_eq!(
                policy.validate_account_profile_for_action(profile, action as u32),
                Ok(()),
                "{action:?} must join the family policy for its own frame"
            );
            if policy.validate_account_profile(profile).is_err() {
                refused += 1;
            }

            // The union declares exactly what this action's own artifact does.
            let own_width = general_state_lifecycle_bytes_v5(action).expect("per-action width");
            let mut own_scratch = vec![0_u8; own_width];
            let mut own = vec![0_u8; own_width];
            let selected = if matches!(
                action,
                Action::InitializeSettlement | Action::PlaceOrder | Action::VerifyCandidateRow
            ) {
                Some(child)
            } else {
                None
            };
            encode_general_state_lifecycle_v5_atomic(action, selected, &mut own_scratch, &mut own)
                .expect("per-action policy");
            let per_action =
                StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &own).expect("decode");
            assert_eq!(
                policy.action_plan_count(action as u32),
                per_action.action_plan_count(action as u32),
                "{action:?} plan count moved in the union"
            );
            assert_eq!(
                policy.action_current_rent_quote_count(action as u32),
                Ok(per_action.current_rent_quote_count()),
                "{action:?} rent quote count moved in the union"
            );
            assert_ne!(family, own, "{action:?} must not BE the family policy");
        }
        assert_eq!(
            refused,
            ACTIONS.len(),
            "the whole-policy join must refuse the family policy for every action"
        );
    }

    #[test]
    fn submit_candidate_lifecycle_is_one_fixed_candidate_recipe_at_runtime_widths() {
        let action = Action::SubmitCandidate;
        let width = general_state_lifecycle_bytes_v5(action).expect("lifecycle width");
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0x55_u8; width];
        encode_general_state_lifecycle_v5_atomic(action, None, &mut scratch, &mut output)
            .expect("SubmitCandidate lifecycle");
        let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &output)
            .expect("canonical lifecycle");
        assert_eq!(policy.action_plan_count(action as u32), Ok(1));
        let plan = policy
            .action_plan(action as u32, 0)
            .expect("candidate create plan");
        assert_eq!(plan.seed_count(), Ok(4));
        assert_eq!(plan.uses_canonical_bump(), Ok(true));
        for count in [1_u32, 258] {
            assert_eq!(
                plan.target_data_bytes(count),
                Ok(u32::try_from(
                    GENERAL_LOCAL_STATE_HEADER_BYTES_V3 + GENERAL_CANDIDATE_BYTES_V1,
                )
                .expect("fixed candidate width")),
            );
        }
    }

    #[test]
    fn close_candidate_is_one_unprotected_close_over_the_canonical_candidate_recipe() {
        let action = Action::CloseCandidate;
        let width = general_state_lifecycle_bytes_v5(action).expect("lifecycle width");
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0x55_u8; width];
        encode_general_state_lifecycle_v5_atomic(action, None, &mut scratch, &mut output)
            .expect("CloseCandidate lifecycle");
        let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &output)
            .expect("canonical lifecycle");
        assert_eq!(policy.action_plan_count(action as u32), Ok(1));
        let close = policy.action_plan(action as u32, 0).expect("close plan");
        assert_eq!(
            close.operation(),
            dclutch_account_profile_contract::lifecycle_v3::LifecycleOperationV3::Close
        );
        assert_eq!(close.protected_outputs(), Ok(None));
        assert_eq!(close.immutable_identity_binding_count(), Ok(0));
        assert_eq!(close.seed_count(), Ok(4));
        assert_eq!(close.uses_canonical_bump(), Ok(true));
        assert_eq!(
            close.target_data_bytes(1),
            Ok(
                u32::try_from(GENERAL_LOCAL_STATE_HEADER_BYTES_V3 + GENERAL_CANDIDATE_BYTES_V1)
                    .expect("candidate width")
            )
        );
    }

    #[test]
    fn verify_lifecycle_is_exact_three_state_conditional_raw_result_at_n_one_and_258() {
        use dclutch_account_profile_contract::lifecycle_v3::LifecycleOperationV3;

        let action = Action::VerifyCandidateRow;
        for outcome_count in [1_u32, 258] {
            let widths = GeneralChildRentWidthsV5::new(outcome_count, 165)
                .expect("authenticated runtime widths");
            let width = general_state_lifecycle_bytes_v5(action).expect("lifecycle width");
            let mut scratch = vec![0_u8; width];
            let mut output = vec![0x55_u8; width];
            encode_general_state_lifecycle_v5_atomic(
                action,
                Some(widths),
                &mut scratch,
                &mut output,
            )
            .expect("VerifyCandidateRow lifecycle");
            let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &output)
                .expect("canonical lifecycle");
            assert_eq!(policy.action_plan_count(action as u32), Ok(3));

            let candidate = policy.action_plan(action as u32, 0).expect("candidate");
            let result = policy.action_plan(action as u32, 1).expect("result");
            let verifier = policy.action_plan(action as u32, 2).expect("verifier");
            assert_eq!(candidate.operation(), LifecycleOperationV3::Authenticate);
            assert_eq!(result.operation(), LifecycleOperationV3::Create);
            assert_eq!(
                verifier.operation(),
                LifecycleOperationV3::AuthenticateOrCreate
            );
            assert_eq!(candidate.protected_outputs(), Ok(None));
            assert_eq!(result.protected_outputs(), Ok(None));
            assert!(verifier.protected_outputs().expect("protected").is_some());
            assert_eq!(candidate.seed_count(), Ok(4));
            assert_eq!(verifier.seed_count(), Ok(4));
            assert_eq!(result.seed_count(), Ok(4));
            assert_eq!(candidate.uses_canonical_bump(), Ok(true));
            assert_eq!(verifier.uses_canonical_bump(), Ok(true));
            assert_eq!(result.uses_canonical_bump(), Ok(true));
            assert_eq!(
                candidate.target_data_bytes(outcome_count),
                Ok(u32::try_from(
                    GENERAL_LOCAL_STATE_HEADER_BYTES_V3 + GENERAL_CANDIDATE_BYTES_V1,
                )
                .expect("candidate width"))
            );
            assert_eq!(
                verifier.target_data_bytes(outcome_count),
                Ok(u32::try_from(
                    GENERAL_LOCAL_STATE_HEADER_BYTES_V3
                        + RUNTIME_VERIFIER_HEADER_BYTES_V2
                        + 40 * usize::try_from(outcome_count).expect("N"),
                )
                .expect("verifier width"))
            );
            assert_eq!(
                result.target_data_bytes(outcome_count),
                Ok(widths.verified_candidate)
            );
            assert_eq!(policy.current_rent_quote_count(), 1);
            let quote = policy.current_rent_quote(0).expect("result rent quote");
            assert_eq!(quote.exact_data_len(), widths.verified_candidate);
            assert_eq!(
                quote.scalar_destination().index(),
                u16::try_from(scalar::RESULT_PRINCIPAL_OBSERVATION).expect("result scalar")
            );

            // Plan one is the canonical Create entry. Its guard is the wire's
            // ScalarEq form over VERIFY_TERMINAL == 1, pinned here so an Always
            // guard cannot silently allocate a nonterminal certificate.
            let guard_offset =
                HEADER_BYTES + 3 * RECIPE_BYTES + 12 * SEED_BYTES + ACTION_PLAN_BYTES + 24;
            assert_eq!(output[guard_offset], 1);
            assert_eq!(
                u16::from_le_bytes([output[guard_offset + 2], output[guard_offset + 3]]),
                u16::try_from(scalar::VERIFY_TERMINAL).expect("guard scalar")
            );
            assert_eq!(
                u64::from_le_bytes(
                    output[guard_offset + 4..guard_offset + 12]
                        .try_into()
                        .expect("guard expected"),
                ),
                1
            );
        }
    }

    #[test]
    fn v5_binds_exact_initialize_child_rent_widths_at_n_one_and_258() {
        for outcome_count in [1_u32, 258] {
            let widths = GeneralChildRentWidthsV5::new(outcome_count, 165)
                .expect("authenticated child widths");
            for action in ACTIONS {
                let width = general_state_lifecycle_bytes_v5(action).expect("V5 width");
                let mut scratch = vec![0_u8; width];
                let mut output = vec![0x55_u8; width];
                let quoted = matches!(
                    action,
                    Action::InitializeSettlement | Action::PlaceOrder | Action::VerifyCandidateRow
                );
                encode_general_state_lifecycle_v5_atomic(
                    action,
                    quoted.then_some(widths),
                    &mut scratch,
                    &mut output,
                )
                .expect("V5 lifecycle artifact");
                let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &output)
                    .expect("V5 decode");
                let expected_count = if action == Action::VerifyCandidateRow {
                    1
                } else if quoted {
                    4
                } else {
                    0
                };
                assert_eq!(policy.current_rent_quote_count(), expected_count);
                if matches!(action, Action::InitializeSettlement | Action::PlaceOrder) {
                    for (ordinal, (data_len, destination)) in [
                        (widths.position, scalar::POSITION_RENT_PRINCIPAL),
                        (
                            u32::try_from(PROTOCOL_POSITION_ADMISSION_BYTES_V2)
                                .expect("admission width"),
                            scalar::ADMISSION_RENT_PRINCIPAL,
                        ),
                        (
                            u32::try_from(CUSTODY_REPLAY_BYTES_V1).expect("replay width"),
                            scalar::CUSTODY_REPLAY_RENT_LAMPORTS,
                        ),
                        (widths.custody_vault, scalar::CUSTODY_VAULT_RENT_LAMPORTS),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let quote = policy
                            .current_rent_quote(u16::try_from(ordinal).expect("ordinal"))
                            .expect("quote");
                        assert_eq!(quote.exact_data_len(), data_len);
                        assert_eq!(
                            quote.scalar_destination().index(),
                            u16::try_from(destination).expect("bounded destination")
                        );
                    }
                }
            }
        }
        assert_eq!(
            GeneralChildRentWidthsV5::new(0, 165),
            Err(GeneralStateArtifactErrorV3::Geometry)
        );
        assert_eq!(
            GeneralChildRentWidthsV5::new(1, 0),
            Err(GeneralStateArtifactErrorV3::Geometry)
        );
    }

    #[test]
    fn v5_refuses_absent_initialize_or_extraneous_noninitialize_widths() {
        for (action, widths) in [
            (Action::InitializeSettlement, None),
            (
                Action::Freeze,
                Some(GeneralChildRentWidthsV5::new(1, 165).expect("widths")),
            ),
        ] {
            let width = general_state_lifecycle_bytes_v5(action).expect("width");
            let mut scratch = vec![0_u8; width];
            let mut output = vec![0x55_u8; width];
            let before = output.clone();
            assert_eq!(
                encode_general_state_lifecycle_v5_atomic(action, widths, &mut scratch, &mut output,),
                Err(GeneralStateArtifactErrorV3::Geometry)
            );
            assert_eq!(output, before);
        }
    }

    #[test]
    fn all_actions_select_exact_readonly_evidence_before_child_routes() {
        // Unchanged by the System program, deliberately: it is APPENDED as the
        // last runtime account, so no evidence or child coordinate moves.
        let expected = [
            (Action::SubmitCandidate, 8, 3, 11),
            (Action::VerifyCandidateRow, 10, 5, 15),
            (Action::Consider, 8, 2, 10),
            (Action::Freeze, 8, 1, 9),
            (Action::InitializeSettlement, 8, 3, 11),
            (Action::Collect, 8, 2, 10),
            (Action::Materialize, 8, 1, 9),
            (Action::Distribute, 8, 2, 10),
            (Action::Close, 9, 1, 10),
        ];
        for (action, start, count, child) in expected {
            assert_eq!(general_readonly_evidence_start_v3(action), start);
            assert_eq!(general_readonly_evidence_count_v3(action), count);
            assert_eq!(general_child_account_start_v3(action), child);
            for index in 0..count {
                assert_eq!(
                    general_readonly_evidence_v3(action, index)
                        .expect("selected evidence")
                        .coordinate,
                    start + index
                );
            }
            assert_eq!(
                general_readonly_evidence_v3(action, count),
                Err(GeneralStateArtifactErrorV3::Geometry)
            );
        }
    }

    #[test]
    fn nonexact_capacity_preserves_output() {
        let action = Action::Close;
        let width = general_state_lifecycle_bytes_v3(action).expect("width");
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0x55_u8; width - 1];
        let before = output.clone();
        assert_eq!(
            encode_general_state_lifecycle_v3_atomic(action, &mut scratch, &mut output),
            Err(GeneralStateArtifactErrorV3::Geometry)
        );
        assert_eq!(output, before);
    }
}
