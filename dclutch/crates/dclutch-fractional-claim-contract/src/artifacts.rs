//! Exact finalized generic-artifact join for the Fractional successor.

use dclutch_account_profile_contract::{
    lifecycle_v3::{SUCCESSOR_SCHEMA_RELEASE_ID, StateLifecyclePolicyV4},
    v2::{AccountProfileV2, SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID},
};
use dclutch_capability_program_contract::v3::{
    CapabilityProgramV3, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID,
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3 as EffectProgramV3, RouteKindV3},
};
use dclutch_execution_strategy_contract::v2::{
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_TERMS_SCHEMA_ID_V1, FractionalTermsAdmissionV1, FractionalTermsV1,
};
use dclutch_request_profile_contract::{RequestProfileV1, validate_request};
use dclutch_token_svm::{TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, TokenBehaviorSelectionV2};
use dclutch_transition_vm::v3::ProgramV3 as TransitionProgramV3;
use sha2::{Digest, Sha256};

use crate::{
    FRACTIONAL_CAPABILITY_KIND_ID_V1, FRACTIONAL_FAMILY_REQUEST_BYTES_V1,
    FRACTIONAL_FAMILY_REQUEST_SCHEMA_ID_V1, FRACTIONAL_ROOT_BYTES_V1, FRACTIONAL_ROOT_SCHEMA_ID_V1,
    FractionalActionV1, FractionalFamilyRequestV1, request::NO_TERMINAL_OUTCOME_V1,
};

/// Authentication of one exact finalized raw/staging Record coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactAdmissionV1 {
    /// Digest carried by the finalized raw Record coordinate.
    pub finalized_digest: [u8; 32],
    /// Raw owner/PDA, vacant staging PDA, digest, and raw rent were authenticated.
    pub record_authenticated: bool,
}

/// Exact bytes of every independently finalized artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalArtifactBytesV1<'a> {
    /// Action-selected CapabilityProgramV3.
    pub descriptor: &'a [u8],
    /// Exact runtime-width terms/config body owned by the Fractional kernel.
    pub terms: &'a [u8],
    /// Realm/release-selected TokenBehaviorV2 record.
    pub token_behavior: &'a [u8],
    /// Runtime account projection.
    pub account_profile: &'a [u8],
    /// Trading-owned root derivation/rent lifecycle.
    pub lifecycle: &'a [u8],
    /// Exact action-specific request checker/projection.
    pub request: &'a [u8],
    /// Interpreted ExecutionStrategyV2 selecting the Transition program.
    pub strategy: &'a [u8],
    /// Exact checked arithmetic transition program.
    pub transition: &'a [u8],
    /// Exact one-Claims-route physical effect program.
    pub effect: &'a [u8],
}

/// Finalized-record authentication for every artifact in declaration order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalArtifactAdmissionsV1 {
    /// Capability descriptor Record.
    pub descriptor: ArtifactAdmissionV1,
    /// Fractional terms/config Record.
    pub terms: ArtifactAdmissionV1,
    /// TokenBehaviorV2 selection Record.
    pub token_behavior: ArtifactAdmissionV1,
    /// AccountProfile Record.
    pub account_profile: ArtifactAdmissionV1,
    /// StateLifecyclePolicyV4 Record.
    pub lifecycle: ArtifactAdmissionV1,
    /// RequestProfile Record.
    pub request: ArtifactAdmissionV1,
    /// ExecutionStrategyV2 Record.
    pub strategy: ArtifactAdmissionV1,
    /// TransitionVM Record.
    pub transition: ArtifactAdmissionV1,
    /// EffectProgram Record.
    pub effect: ArtifactAdmissionV1,
}

/// Release-selected physical child identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalChildProgramsV1 {
    /// Current Registry-selected Claims program.
    pub claims: [u8; 32],
    /// Current Registry-selected Custody program used only behind Claims.
    pub custody: [u8; 32],
    /// Token program selected by the immutable TokenBehaviorV2 record.
    pub token: [u8; 32],
    /// Finalized physical profile binding public child FrameSpec identities.
    pub physical_profile: [u8; 32],
    /// The exact release-set/ProgramData observations were authenticated.
    pub release_authenticated: bool,
}

/// Independently authenticated immutable semantic selections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalArtifactSelectionV1 {
    /// Action-selected finalized CapabilityProgramV3 digest.
    pub descriptor_id: [u8; 32],
    /// Manifest-selected exact Fractional terms digest.
    pub terms_id: [u8; 32],
    /// Logical Core Market identity.
    pub market: [u8; 32],
    /// Immutable Realm identity decoded from Market.
    pub market_realm: [u8; 32],
    /// Finalized Product graph-root digest.
    pub product_record: [u8; 32],
    /// Product-owned ResultDomain digest and ordering.
    pub result_domain: [u8; 32],
    /// Product-authenticated runtime outcome width.
    pub outcome_count: u32,
    /// Immutable release-set identity selected by Market/capability.
    pub release_set: [u8; 32],
    /// Selected child programs and physical frame profile.
    pub children: FractionalChildProgramsV1,
    /// Market/Realm/Product/manifest coordinates were independently authenticated.
    pub semantic_selection_authenticated: bool,
}

/// Complete borrowed artifact bundle after every identity and geometry join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalArtifactBundleV1<'a> {
    /// Checked action request.
    pub family_request: FractionalFamilyRequestV1,
    /// Exact capability descriptor.
    pub descriptor: CapabilityProgramV3,
    /// Kernel-owned terms/config.
    pub terms: FractionalTermsV1<'a>,
    /// Selected TokenBehaviorV2 semantics.
    pub token_behavior: TokenBehaviorSelectionV2,
    /// Runtime account projection.
    pub account_profile: AccountProfileV2<'a>,
    /// Trading root lifecycle/rent policy.
    pub lifecycle: StateLifecyclePolicyV4<'a>,
    /// Action request program.
    pub request_profile: RequestProfileV1<'a>,
    /// Exact interpreted execution strategy.
    pub strategy: ExecutionStrategyProgramV2,
    /// Kernel-specialized transition program.
    pub transition: TransitionProgramV3<'a>,
    /// Sole physical Claims route.
    pub effect: EffectProgramV3<'a>,
    /// Release-selected child program identities.
    pub children: FractionalChildProgramsV1,
}

/// Stable finalized-artifact refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalArtifactErrorV1 {
    /// A selected/finalized/raw digest or Record authentication differed.
    ArtifactIdentity,
    /// Immutable Market/Product/release/child selection was absent or inconsistent.
    SemanticSelection,
    /// Capability descriptor was malformed or selected another family/schema/profile.
    Descriptor,
    /// Kernel-owned terms were malformed or substituted.
    Terms,
    /// TokenBehaviorV2 record or Token program did not match terms/Realm/release.
    TokenBehavior,
    /// Family request was malformed or did not echo authenticated chain facts.
    FamilyRequest,
    /// AccountProfile hostile decoding refused.
    AccountProfile,
    /// State lifecycle hostile decoding or root-account geometry refused.
    Lifecycle,
    /// RequestProfile hostile decoding or request execution refused.
    RequestProfile,
    /// Interpreted ExecutionStrategyV2 selection refused.
    Strategy,
    /// TransitionVM hostile decoding refused.
    Transition,
    /// EffectProgram hostile decoding or Claims-route selection refused.
    Effect,
    /// Account/register/request runtime-width geometry differed.
    Geometry,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, FractionalArtifactErrorV1>;

/// Authenticate one exact action-selected Fractional artifact bundle.
///
/// Every raw digest is recomputed here. `record_authenticated` represents only
/// the small Registry owner/PDA/staging/rent adapter boundary; it cannot replace
/// byte authentication or descriptor joins.
pub fn authenticate_fractional_artifact_bundle_v1<'a>(
    selection: FractionalArtifactSelectionV1,
    admissions: FractionalArtifactAdmissionsV1,
    artifacts: FractionalArtifactBytesV1<'a>,
    family_request_bytes: &[u8],
) -> Result<FractionalArtifactBundleV1<'a>> {
    validate_selection(selection)?;
    require_record(
        selection.descriptor_id,
        admissions.descriptor,
        artifacts.descriptor,
    )?;
    let descriptor = CapabilityProgramV3::decode(artifacts.descriptor)
        .map_err(|_| FractionalArtifactErrorV1::Descriptor)?;
    validate_descriptor(descriptor, selection)?;

    require_record(selection.terms_id, admissions.terms, artifacts.terms)?;
    let terms = FractionalTermsV1::decode(
        artifacts.terms,
        FractionalTermsAdmissionV1 {
            selected_schema_id: FRACTIONAL_TERMS_SCHEMA_ID_V1,
            finalized_schema_id: FRACTIONAL_TERMS_SCHEMA_ID_V1,
            selected_terms_id: selection.terms_id,
            finalized_terms_id: admissions.terms.finalized_digest,
            recomputed_terms_digest: digest(artifacts.terms),
            finalized_terms_digest: admissions.terms.finalized_digest,
            record_authenticated: admissions.terms.record_authenticated,
        },
    )
    .map_err(|_| FractionalArtifactErrorV1::Terms)?;
    if terms.market_id() != selection.market
        || terms.result_domain_id() != selection.result_domain
        || terms.release_set_id() != selection.release_set
        || terms.outcome_count() != selection.outcome_count
        || terms.token_program() != selection.children.token
    {
        return Err(FractionalArtifactErrorV1::SemanticSelection);
    }

    require_record(
        terms.token_behavior_selection_id(),
        admissions.token_behavior,
        artifacts.token_behavior,
    )?;
    let token_behavior = TokenBehaviorSelectionV2::decode_for_authenticated_selection(
        artifacts.token_behavior,
        selection.market_realm,
        selection.release_set,
    )
    .map_err(|_| FractionalArtifactErrorV1::TokenBehavior)?;
    if token_behavior.token_program() != terms.token_program()
        || token_behavior.profile_id() == [0; 32]
    {
        return Err(FractionalArtifactErrorV1::TokenBehavior);
    }

    let account_id = descriptor.account_profile().to_bytes();
    require_record(
        account_id,
        admissions.account_profile,
        artifacts.account_profile,
    )?;
    let account_profile = AccountProfileV2::decode(artifacts.account_profile)
        .map_err(|_| FractionalArtifactErrorV1::AccountProfile)?;

    let lifecycle_id = descriptor.derivation_policy().to_bytes();
    require_record(lifecycle_id, admissions.lifecycle, artifacts.lifecycle)?;
    let lifecycle = StateLifecyclePolicyV4::decode_selected(
        lifecycle_id,
        digest(artifacts.lifecycle),
        artifacts.lifecycle,
    )
    .map_err(|_| FractionalArtifactErrorV1::Lifecycle)?;
    lifecycle
        .validate_account_profile(account_profile)
        .map_err(|_| FractionalArtifactErrorV1::Lifecycle)?;

    let request_id = descriptor.request_profile_program().to_bytes();
    require_record(request_id, admissions.request, artifacts.request)?;
    let request_profile =
        RequestProfileV1::decode_selected(request_id, digest(artifacts.request), artifacts.request)
            .map_err(|_| FractionalArtifactErrorV1::RequestProfile)?;

    let strategy_id = descriptor.transition_program().to_bytes();
    require_record(strategy_id, admissions.strategy, artifacts.strategy)?;
    let strategy = ExecutionStrategyProgramV2::decode(artifacts.strategy)
        .map_err(|_| FractionalArtifactErrorV1::Strategy)?;
    strategy
        .validate_descriptor_selection(content(strategy_id)?, descriptor)
        .map_err(|_| FractionalArtifactErrorV1::Strategy)?;
    if strategy.disposition() != StrategyDispositionV2::Interpreted
        || strategy.transition_schema().to_bytes() != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID
    {
        return Err(FractionalArtifactErrorV1::Strategy);
    }

    let transition_id = strategy.transition_program().to_bytes();
    require_record(transition_id, admissions.transition, artifacts.transition)?;
    let transition = TransitionProgramV3::decode(artifacts.transition)
        .map_err(|_| FractionalArtifactErrorV1::Transition)?;

    let effect_id = descriptor.effect_program().to_bytes();
    require_record(effect_id, admissions.effect, artifacts.effect)?;
    let effect =
        EffectProgramV3::decode_selected(effect_id, digest(artifacts.effect), artifacts.effect)
            .map_err(|_| FractionalArtifactErrorV1::Effect)?;

    let family_request = FractionalFamilyRequestV1::decode(family_request_bytes)
        .map_err(|_| FractionalArtifactErrorV1::FamilyRequest)?;
    validate_family_request(family_request, selection, terms)?;
    validate_request(
        request_profile,
        selection.outcome_count,
        family_request_bytes,
    )
    .map_err(|_| FractionalArtifactErrorV1::RequestProfile)?;
    validate_geometry(
        selection.outcome_count,
        account_profile,
        request_profile,
        transition,
        effect,
    )?;
    if lifecycle
        .action_plan_count(u32::from(family_request.action().byte()))
        .map_err(|_| FractionalArtifactErrorV1::Lifecycle)?
        == 0
    {
        return Err(FractionalArtifactErrorV1::Lifecycle);
    }
    validate_claims_route(effect)?;

    Ok(FractionalArtifactBundleV1 {
        family_request,
        descriptor,
        terms,
        token_behavior,
        account_profile,
        lifecycle,
        request_profile,
        strategy,
        transition,
        effect,
        children: selection.children,
    })
}

fn validate_selection(selection: FractionalArtifactSelectionV1) -> Result<()> {
    if !selection.semantic_selection_authenticated
        || selection.outcome_count == 0
        || [
            selection.descriptor_id,
            selection.terms_id,
            selection.market,
            selection.market_realm,
            selection.product_record,
            selection.result_domain,
            selection.release_set,
            selection.children.claims,
            selection.children.custody,
            selection.children.token,
            selection.children.physical_profile,
        ]
        .iter()
        .any(is_zero)
        || !selection.children.release_authenticated
        || selection.children.claims == selection.children.custody
        || selection.children.claims == selection.children.token
        || selection.children.custody == selection.children.token
    {
        return Err(FractionalArtifactErrorV1::SemanticSelection);
    }
    Ok(())
}

fn validate_descriptor(
    descriptor: CapabilityProgramV3,
    selection: FractionalArtifactSelectionV1,
) -> Result<()> {
    if descriptor.kind().to_bytes() != FRACTIONAL_CAPABILITY_KIND_ID_V1
        || descriptor.config_schema().to_bytes() != FRACTIONAL_TERMS_SCHEMA_ID_V1
        || descriptor.request_schema().to_bytes() != FRACTIONAL_FAMILY_REQUEST_SCHEMA_ID_V1
        || descriptor.root_schema().to_bytes() != FRACTIONAL_ROOT_SCHEMA_ID_V1
        || descriptor.capacity_profile().to_bytes() != selection.children.physical_profile
        || descriptor.request_profile_schema().to_bytes()
            != dclutch_request_profile_contract::SCHEMA_RELEASE_ID
        || descriptor.transition_schema().to_bytes() != EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2
        || descriptor.root_state_bytes()
            != u32::try_from(FRACTIONAL_ROOT_BYTES_V1)
                .map_err(|_| FractionalArtifactErrorV1::Geometry)?
    {
        return Err(FractionalArtifactErrorV1::Descriptor);
    }
    Ok(())
}

fn validate_family_request(
    request: FractionalFamilyRequestV1,
    selection: FractionalArtifactSelectionV1,
    terms: FractionalTermsV1<'_>,
) -> Result<()> {
    let input = request.input();
    if input.release_set != selection.release_set
        || input.market != selection.market
        || input.product_record != selection.product_record
        || input.result_domain != selection.result_domain
        || input.terms != selection.terms_id
        || input.token_behavior != terms.token_behavior_selection_id()
        || (request.action() != FractionalActionV1::ZeroSupplyRetire
            && input.outcome >= selection.outcome_count)
        || (input.terminal_outcome != NO_TERMINAL_OUTCOME_V1
            && input.terminal_outcome >= selection.outcome_count)
    {
        return Err(FractionalArtifactErrorV1::FamilyRequest);
    }
    Ok(())
}

fn validate_geometry(
    outcome_count: u32,
    account: AccountProfileV2<'_>,
    request: RequestProfileV1<'_>,
    transition: TransitionProgramV3<'_>,
    effect: EffectProgramV3<'_>,
) -> Result<()> {
    if request
        .request_bytes(outcome_count)
        .map_err(|_| FractionalArtifactErrorV1::Geometry)?
        != FRACTIONAL_FAMILY_REQUEST_BYTES_V1
        || account.fixed_account_count() != effect.fixed_account_count()
        || account.item_account_stride() != effect.item_account_stride()
        || account.common_scalar_count() != request.common_scalar_count()
        || account.common_scalar_count() != transition.common_scalar_count()
        || account.common_scalar_count() != effect.common_scalar_count()
        || account.item_scalar_stride() != request.item_scalar_stride()
        || account.item_scalar_stride() != transition.item_scalar_stride()
        || account.item_scalar_stride() != effect.item_scalar_stride()
        || account.common_identity_count() != request.common_identity_count()
        || account.common_identity_count() != transition.common_identity_count()
        || account.common_identity_count() != effect.common_identity_count()
        || account.item_identity_stride() != request.item_identity_stride()
        || account.item_identity_stride() != transition.item_identity_stride()
        || account.item_identity_stride() != effect.item_identity_stride()
        || account
            .logical_account_count(outcome_count)
            .map_err(|_| FractionalArtifactErrorV1::Geometry)?
            == 0
    {
        return Err(FractionalArtifactErrorV1::Geometry);
    }
    Ok(())
}

fn validate_claims_route(effect: EffectProgramV3<'_>) -> Result<()> {
    if effect.route_count() != 1 || effect.receipt_dependency_count() != 0 {
        return Err(FractionalArtifactErrorV1::Effect);
    }
    let route = effect
        .route(0)
        .map_err(|_| FractionalArtifactErrorV1::Effect)?;
    if route.role() != FixedRole::Claims
        || !matches!(route.kind(), RouteKindV3::Once | RouteKindV3::AffineOnce)
        || route.receipt_dependency_count() != 0
        || route.fixed_account_count() == 0
        || route.fixed_request_bytes() == 0
    {
        return Err(FractionalArtifactErrorV1::Effect);
    }
    Ok(())
}

fn require_record(selected: [u8; 32], admission: ArtifactAdmissionV1, bytes: &[u8]) -> Result<()> {
    if is_zero(&selected)
        || !admission.record_authenticated
        || selected != admission.finalized_digest
        || selected != digest(bytes)
    {
        return Err(FractionalArtifactErrorV1::ArtifactIdentity);
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn content(bytes: [u8; 32]) -> Result<ContentId> {
    ContentId::new(bytes).map_err(|_| FractionalArtifactErrorV1::ArtifactIdentity)
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

#[allow(dead_code)]
const _: [u8; 32] = CAPABILITY_PROGRAM_SCHEMA_ID;
#[allow(dead_code)]
const _: [u8; 32] = ACCOUNT_PROFILE_SCHEMA_ID;
#[allow(dead_code)]
const _: [u8; 32] = SUCCESSOR_SCHEMA_RELEASE_ID;
#[allow(dead_code)]
const _: [u8; 32] = TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2;
