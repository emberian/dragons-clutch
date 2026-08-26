//! Exact finalized-artifact join for recurring Series V3.
//!
//! CapabilityProgramSetV1 selects one complete CapabilityProgramV3 from the
//! action byte in the exact 128-byte Series header. The occurrence Merkle path
//! is an independently bounded trailing witness: RequestProfile never treats
//! it as Product-affine data. The selected descriptor then joins the exact
//! AccountProfile, RequestProfile, EffectProgram, ExecutionStrategy, and
//! underlying TransitionVM records. This module authenticates and projects no
//! state; the common Trading V3 outer remains the sole writer and CPI caller.

use dclutch_account_profile_contract::v2::AccountProfileV2;
use dclutch_capability_program_contract::{
    set_v1::{CapabilityProgramSetV1, SelectorWidthV1},
    v3::CapabilityProgramV3,
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3 as EffectProgramV3, ResolvedInvocationV3, RouteKindV3},
};
use dclutch_execution_strategy_contract::v2::{
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyProgramV2,
};
use dclutch_request_profile_contract::{ProjectionRegistersV1, RequestProfileV1, project_atomic};
use dclutch_series_v3_kernel::SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3;
use dclutch_transition_vm::{MAX_IDENTITIES, MAX_SCALARS, v3::ProgramV3 as TransitionProgramV3};
use solana_program::hash::{hash, hashv};

use super::{
    instruction::{SERIES_ACTION_HEADER_BYTES_V3, SeriesActionRequestV3, SeriesActionV3},
    state::SERIES_STATE_BYTES_V3,
};

/// Byte offset of the canonical one-byte action selector in the Series header.
pub const SERIES_ACTION_SELECTOR_OFFSET_V3: u32 = 12;
/// One exact Merkle sibling in the borrowed witness suffix.
pub const SERIES_WITNESS_ITEM_BYTES_V3: usize = 32;
/// Semantic kind label for recurring Series V3 capability programs.
pub const SERIES_SUCCESSOR_KIND_PREIMAGE_V3: &[u8] = b"dclutch/kind/series-v3";
/// Family request schema covers the fixed semantic header, not its proof witness.
pub const SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/series-action-header-v3";
/// Mutable Series root-tail schema label.
pub const SERIES_ROOT_SCHEMA_PREIMAGE_V3: &[u8] = b"dclutch/schema/series-root-v3";
/// Ticket replay-account derivation-policy label.
pub const SERIES_TICKET_DERIVATION_PREIMAGE_V3: &[u8] = b"dclutch/derivation/series-ticket-v3";

/// Exact descriptor-selected raw finalized artifacts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesArtifactBytesV3<'a> {
    /// Canonical action-to-program set bytes.
    pub program_set: &'a [u8],
    /// Action-selected CapabilityProgramV3 bytes.
    pub descriptor: &'a [u8],
    /// Exact runtime AccountProfile bytes.
    pub account_profile: &'a [u8],
    /// Exact 128-byte-header RequestProfile bytes.
    pub request_profile: &'a [u8],
    /// Exact generic ExecutionStrategy V2 bytes.
    pub strategy: &'a [u8],
    /// Exact underlying interpreted TransitionVM V3 bytes.
    pub transition: &'a [u8],
    /// Exact fixed-role/local EffectProgram V3 bytes.
    pub effect: &'a [u8],
}

/// Immutable manifest/root selections authenticated before this join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesArtifactSelectionV3 {
    /// Capability release selecting the exact ProgramSet record.
    pub program_set: [u8; 32],
    /// Manifest config selecting the exact Series Template content identity.
    pub template: ContentId,
}

/// Stable refusal from the complete Series artifact join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesArtifactErrorV3 {
    /// Selected identity was zero or differed from authenticated bytes/content.
    ContentIdentity,
    /// ProgramSet selector geometry or action selection refused.
    ProgramSet,
    /// Full Series request or its exact header/witness split refused.
    Request,
    /// Selected descriptor named another semantic family or schema.
    Descriptor,
    /// AccountProfile hostile decode refused.
    AccountProfile,
    /// RequestProfile hostile decode or header projection refused.
    RequestProfile,
    /// ExecutionStrategy hostile decode or descriptor join refused.
    Strategy,
    /// Underlying TransitionVM hostile decode or Strategy join refused.
    Transition,
    /// EffectProgram hostile decode or Series role grammar refused.
    Effect,
    /// Fixed non-affine account/register/request geometry differed.
    Geometry,
}

/// Result alias for Series V3 artifact joins.
pub type Result<T> = core::result::Result<T, SeriesArtifactErrorV3>;

/// Exact fixed header and independently bounded witness suffix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesRequestSlicesV3<'a> {
    /// Exact 128-byte semantic header consumed by RequestProfile.
    pub header: &'a [u8],
    /// Exact no-leftover Merkle sibling suffix consumed by Series admission.
    pub witness: &'a [u8],
}

/// Fully joined borrowed artifact bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesArtifactBundleV3<'a> {
    /// Hostile-decoded complete family request.
    pub request: SeriesActionRequestV3<'a>,
    /// Explicit header/witness boundary.
    pub slices: SeriesRequestSlicesV3<'a>,
    /// Selected fixed descriptor.
    pub descriptor: CapabilityProgramV3,
    /// Exact non-affine physical account interpreter.
    pub account_profile: AccountProfileV2<'a>,
    /// Exact fixed-header request interpreter.
    pub request_profile: RequestProfileV1<'a>,
    /// Exact acyclic execution strategy.
    pub strategy: ExecutionStrategyProgramV2,
    /// Exact strategy-selected TransitionVM program.
    pub transition: TransitionProgramV3<'a>,
    /// Exact local/fixed-role effect program.
    pub effect: EffectProgramV3<'a>,
}

/// Exact two-slice Core instruction selected by an executed Consume route.
///
/// The common outer owns concatenation into CPI instruction data. Keeping the
/// typed 336-byte Core request and authenticated proof witness separate here
/// prevents the Series adapter from inventing either portion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumeInvocationV3<'a> {
    /// Exact IR-owned `SeriesCoreRequestV1` bytes.
    pub core_request: &'a [u8],
    /// Exact trailing occurrence-proof bytes borrowed from the family request.
    pub witness: &'a [u8],
    /// SHA-256 of `core_request || witness`, which binds the executed child data.
    pub child_request_digest: [u8; 32],
}

impl SeriesConsumeInvocationV3<'_> {
    /// Exact concatenated Core instruction width.
    pub fn child_request_len(self) -> Result<usize> {
        self.core_request
            .len()
            .checked_add(self.witness.len())
            .ok_or(SeriesArtifactErrorV3::Geometry)
    }
}

/// Authenticate and join one complete recurring-Series action bundle.
pub fn authenticate_series_artifacts_v3<'a>(
    selection: SeriesArtifactSelectionV3,
    artifacts: SeriesArtifactBytesV3<'a>,
    family_request: &'a [u8],
) -> Result<SeriesArtifactBundleV3<'a>> {
    require_selected(selection.program_set, artifacts.program_set)?;
    let set = CapabilityProgramSetV1::decode_selected(
        selection.program_set,
        digest(artifacts.program_set),
        artifacts.program_set,
    )
    .map_err(|_| SeriesArtifactErrorV3::ProgramSet)?;
    if set.selector_offset() != SERIES_ACTION_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV1::U8
    {
        return Err(SeriesArtifactErrorV3::ProgramSet);
    }

    let request = SeriesActionRequestV3::decode(family_request)
        .map_err(|_| SeriesArtifactErrorV3::Request)?;
    if request.template() != selection.template {
        return Err(SeriesArtifactErrorV3::ContentIdentity);
    }
    let slices = split_request(request, family_request)?;
    let selected_descriptor = set
        .select(slices.header)
        .map_err(|_| SeriesArtifactErrorV3::ProgramSet)?;
    if selected_descriptor.to_bytes() != digest(artifacts.descriptor) {
        return Err(SeriesArtifactErrorV3::ContentIdentity);
    }
    let descriptor = CapabilityProgramV3::decode(artifacts.descriptor)
        .map_err(|_| SeriesArtifactErrorV3::Descriptor)?;
    validate_descriptor(descriptor)?;

    require_content(
        descriptor.account_profile().to_bytes(),
        artifacts.account_profile,
    )?;
    let account_profile = AccountProfileV2::decode(artifacts.account_profile)
        .map_err(|_| SeriesArtifactErrorV3::AccountProfile)?;
    require_content(
        descriptor.request_profile_program().to_bytes(),
        artifacts.request_profile,
    )?;
    let request_profile = RequestProfileV1::decode_selected(
        descriptor.request_profile_program().to_bytes(),
        digest(artifacts.request_profile),
        artifacts.request_profile,
    )
    .map_err(|_| SeriesArtifactErrorV3::RequestProfile)?;
    validate_and_execute_header(request_profile, slices.header)?;

    let strategy_id = content_id(artifacts.strategy)?;
    let strategy = ExecutionStrategyProgramV2::decode(artifacts.strategy)
        .map_err(|_| SeriesArtifactErrorV3::Strategy)?;
    strategy
        .validate_descriptor_selection(strategy_id, descriptor)
        .map_err(|_| SeriesArtifactErrorV3::Strategy)?;
    require_content(
        strategy.transition_program().to_bytes(),
        artifacts.transition,
    )?;
    if strategy.transition_schema().to_bytes() != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID {
        return Err(SeriesArtifactErrorV3::Transition);
    }
    let transition = TransitionProgramV3::decode(artifacts.transition)
        .map_err(|_| SeriesArtifactErrorV3::Transition)?;

    require_content(descriptor.effect_program().to_bytes(), artifacts.effect)?;
    let effect = EffectProgramV3::decode_selected(
        descriptor.effect_program().to_bytes(),
        digest(artifacts.effect),
        artifacts.effect,
    )
    .map_err(|_| SeriesArtifactErrorV3::Effect)?;
    validate_geometry(account_profile, request_profile, transition, effect)?;
    validate_routes(request.action(), effect)?;

    Ok(SeriesArtifactBundleV3 {
        request,
        slices,
        descriptor,
        account_profile,
        request_profile,
        strategy,
        transition,
        effect,
    })
}

/// Bind one post-strategy Consume invocation to the exact Series proof suffix.
///
/// `invocation` must come from the authenticated selected EffectProgram after
/// the common outer has executed RequestProfile and TransitionVM. This helper
/// deliberately receives no raw register index: the generic interpreter owns
/// that coordinate and [`ResolvedInvocationV3`] is its checked result.
pub fn validate_series_consume_invocation_v3<'a>(
    bundle: SeriesArtifactBundleV3<'_>,
    invocation: ResolvedInvocationV3,
    ir_request_bank: &'a [u8],
    family_request: &'a [u8],
) -> Result<SeriesConsumeInvocationV3<'a>> {
    if bundle.request.action() != SeriesActionV3::Consume
        || invocation.role != FixedRole::Core
        || invocation.kind != RouteKindV3::Once
        || invocation.item.is_some()
        || invocation.repeated_item_count != 0
        || invocation.request_offset != 0
        || invocation.request_len != dclutch_market_core_codec::SERIES_CORE_REQUEST_BYTES_V1
        || ir_request_bank.len() != dclutch_market_core_codec::SERIES_CORE_REQUEST_BYTES_V1
        || family_request.get(..SERIES_ACTION_HEADER_BYTES_V3) != Some(bundle.slices.header)
    {
        return Err(SeriesArtifactErrorV3::Effect);
    }
    let core_request = ir_request_bank
        .get(invocation.request_offset..invocation.request_len)
        .ok_or(SeriesArtifactErrorV3::Effect)?;
    let borrowed = invocation
        .borrowed_witness
        .ok_or(SeriesArtifactErrorV3::Effect)?;
    if borrowed.source_offset() != SERIES_ACTION_HEADER_BYTES_V3
        || borrowed.len() != bundle.slices.witness.len()
    {
        return Err(SeriesArtifactErrorV3::Effect);
    }
    let witness = borrowed
        .slice(family_request)
        .map_err(|_| SeriesArtifactErrorV3::Request)?;
    if witness != bundle.slices.witness
        || witness.len()
            != usize::from(bundle.request.proof_count())
                .checked_mul(SERIES_WITNESS_ITEM_BYTES_V3)
                .ok_or(SeriesArtifactErrorV3::Geometry)?
    {
        return Err(SeriesArtifactErrorV3::Request);
    }
    Ok(SeriesConsumeInvocationV3 {
        core_request,
        witness,
        child_request_digest: hashv(&[core_request, witness]).to_bytes(),
    })
}

fn split_request<'a>(
    request: SeriesActionRequestV3<'_>,
    bytes: &'a [u8],
) -> Result<SeriesRequestSlicesV3<'a>> {
    let header = bytes
        .get(..SERIES_ACTION_HEADER_BYTES_V3)
        .ok_or(SeriesArtifactErrorV3::Request)?;
    let witness = bytes
        .get(SERIES_ACTION_HEADER_BYTES_V3..)
        .ok_or(SeriesArtifactErrorV3::Request)?;
    let expected = usize::from(request.proof_count())
        .checked_mul(SERIES_WITNESS_ITEM_BYTES_V3)
        .ok_or(SeriesArtifactErrorV3::Request)?;
    if witness.len() != expected || witness != request.proof_bytes() {
        return Err(SeriesArtifactErrorV3::Request);
    }
    Ok(SeriesRequestSlicesV3 { header, witness })
}

fn validate_descriptor(descriptor: CapabilityProgramV3) -> Result<()> {
    if descriptor.kind().to_bytes() != digest(SERIES_SUCCESSOR_KIND_PREIMAGE_V3)
        || descriptor.config_schema().to_bytes() != SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3
        || descriptor.request_schema().to_bytes() != digest(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3)
        || descriptor.root_schema().to_bytes() != digest(SERIES_ROOT_SCHEMA_PREIMAGE_V3)
        || descriptor.derivation_policy().to_bytes() != digest(SERIES_TICKET_DERIVATION_PREIMAGE_V3)
        || descriptor.request_profile_schema().to_bytes()
            != dclutch_request_profile_contract::SCHEMA_RELEASE_ID
        || descriptor.transition_schema().to_bytes() != EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2
        || usize::try_from(descriptor.root_state_bytes())
            .map_err(|_| SeriesArtifactErrorV3::Geometry)?
            != SERIES_STATE_BYTES_V3
    {
        return Err(SeriesArtifactErrorV3::Descriptor);
    }
    Ok(())
}

fn validate_and_execute_header(profile: RequestProfileV1<'_>, header: &[u8]) -> Result<()> {
    if profile
        .request_bytes(0)
        .map_err(|_| SeriesArtifactErrorV3::RequestProfile)?
        != SERIES_ACTION_HEADER_BYTES_V3
        || profile.item_request_bytes() != 0
        || profile.item_scalar_stride() != 0
        || profile.item_identity_stride() != 0
    {
        return Err(SeriesArtifactErrorV3::Geometry);
    }
    let scalars = usize::from(profile.common_scalar_count());
    let identities = usize::from(profile.common_identity_count());
    if scalars > MAX_SCALARS || identities > MAX_IDENTITIES {
        return Err(SeriesArtifactErrorV3::Geometry);
    }
    let input_scalars = [0_u64; MAX_SCALARS];
    let input_identities = [[0_u8; 32]; MAX_IDENTITIES];
    let mut scratch_scalars = [0_u64; MAX_SCALARS];
    let mut scratch_identities = [[0_u8; 32]; MAX_IDENTITIES];
    let mut output_scalars = [0_u64; MAX_SCALARS];
    let mut output_identities = [[0_u8; 32]; MAX_IDENTITIES];
    let input_scalars = input_scalars
        .get(..scalars)
        .ok_or(SeriesArtifactErrorV3::Geometry)?;
    let input_identities = input_identities
        .get(..identities)
        .ok_or(SeriesArtifactErrorV3::Geometry)?;
    let scratch_scalars = scratch_scalars
        .get_mut(..scalars)
        .ok_or(SeriesArtifactErrorV3::Geometry)?;
    let scratch_identities = scratch_identities
        .get_mut(..identities)
        .ok_or(SeriesArtifactErrorV3::Geometry)?;
    let output_scalars = output_scalars
        .get_mut(..scalars)
        .ok_or(SeriesArtifactErrorV3::Geometry)?;
    let output_identities = output_identities
        .get_mut(..identities)
        .ok_or(SeriesArtifactErrorV3::Geometry)?;
    project_atomic(
        profile,
        0,
        header,
        ProjectionRegistersV1 {
            input_scalars,
            input_identities,
            scratch_scalars,
            scratch_identities,
            output_scalars,
            output_identities,
        },
    )
    .map_err(|_| SeriesArtifactErrorV3::RequestProfile)
}

fn validate_geometry(
    account: AccountProfileV2<'_>,
    request: RequestProfileV1<'_>,
    transition: TransitionProgramV3<'_>,
    effect: EffectProgramV3<'_>,
) -> Result<()> {
    let common_scalars = account.common_scalar_count();
    let common_identities = account.common_identity_count();
    if account.item_account_stride() != 0
        || account.item_scalar_stride() != 0
        || account.item_identity_stride() != 0
        || request.common_scalar_count() != common_scalars
        || request.common_identity_count() != common_identities
        || transition.common_scalar_count() != common_scalars
        || transition.common_identity_count() != common_identities
        || transition.item_scalar_stride() != 0
        || transition.item_identity_stride() != 0
        || effect.fixed_account_count() != account.fixed_account_count()
        || effect.item_account_stride() != 0
        || effect.common_scalar_count() != common_scalars
        || effect.common_identity_count() != common_identities
        || effect.item_scalar_stride() != 0
        || effect.item_identity_stride() != 0
        || effect.item_operation_count() != 0
    {
        return Err(SeriesArtifactErrorV3::Geometry);
    }
    Ok(())
}

fn validate_routes(action: SeriesActionV3, effect: EffectProgramV3<'_>) -> Result<()> {
    let (count, role, fixed_request) = match action {
        SeriesActionV3::Prepare | SeriesActionV3::Expire => (
            3_u16,
            Some(FixedRole::Custody),
            u32::try_from(dclutch_custody_contract::CUSTODY_REQUEST_BYTES_V1)
                .map_err(|_| SeriesArtifactErrorV3::Geometry)?,
        ),
        SeriesActionV3::Consume => (
            1,
            Some(FixedRole::Core),
            u32::try_from(dclutch_market_core_codec::SERIES_CORE_REQUEST_BYTES_V1)
                .map_err(|_| SeriesArtifactErrorV3::Geometry)?,
        ),
        SeriesActionV3::Retire | SeriesActionV3::Close => (0, None, 0),
    };
    if effect.route_count() != count {
        return Err(SeriesArtifactErrorV3::Effect);
    }
    let mut index = 0_u16;
    while index < count {
        let route = effect
            .route(index)
            .map_err(|_| SeriesArtifactErrorV3::Effect)?;
        if Some(route.role()) != role
            || route.kind() != dclutch_effect_kernel::v3::RouteKindV3::Once
            || route.fixed_request_bytes() != fixed_request
            || route.item_request_bytes() != 0
            || route.borrows_witness() != matches!(action, SeriesActionV3::Consume)
        {
            return Err(SeriesArtifactErrorV3::Effect);
        }
        index = index
            .checked_add(1)
            .ok_or(SeriesArtifactErrorV3::Geometry)?;
    }
    Ok(())
}

fn require_selected(selected: [u8; 32], bytes: &[u8]) -> Result<()> {
    if selected == [0; 32] || selected != digest(bytes) {
        Err(SeriesArtifactErrorV3::ContentIdentity)
    } else {
        Ok(())
    }
}

fn require_content(selected: [u8; 32], bytes: &[u8]) -> Result<()> {
    require_selected(selected, bytes)
}

fn content_id(bytes: &[u8]) -> Result<ContentId> {
    ContentId::new(digest(bytes)).map_err(|_| SeriesArtifactErrorV3::ContentIdentity)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    hash(bytes).to_bytes()
}

#[cfg(test)]
mod tests {
    extern crate std;

    use dclutch_capability_program_contract::v3::CAPABILITY_PROGRAM_V3_BYTES;
    use dclutch_execution_strategy_contract::v2::{
        ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
        EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
        StrategyDispositionV2,
    };
    use dclutch_transition_vm::v3::{RegisterInput, RegisterOutput, execute_fold_atomic};
    use std::{vec, vec::Vec};

    use super::*;
    use crate::series::instruction::encode_series_action_header_v3;

    const FIXTURE_SCALARS: u16 = 4;
    const REQUEST_OPERATIONS: u16 = 2;
    const TRANSITION_OPERATIONS: u16 = 3;

    struct Fixture {
        set: Vec<u8>,
        descriptor: [u8; CAPABILITY_PROGRAM_V3_BYTES],
        account: Vec<u8>,
        request_profile: Vec<u8>,
        strategy:
            [u8; dclutch_execution_strategy_contract::v2::EXECUTION_STRATEGY_PROGRAM_BYTES_V2],
        transition: Vec<u8>,
        effect: Vec<u8>,
        request: Vec<u8>,
        template: ContentId,
    }

    impl Fixture {
        fn artifacts(&self) -> SeriesArtifactBytesV3<'_> {
            SeriesArtifactBytesV3 {
                program_set: &self.set,
                descriptor: &self.descriptor,
                account_profile: &self.account,
                request_profile: &self.request_profile,
                strategy: &self.strategy,
                transition: &self.transition,
                effect: &self.effect,
            }
        }

        fn selection(&self) -> SeriesArtifactSelectionV3 {
            SeriesArtifactSelectionV3 {
                program_set: digest(&self.set),
                template: self.template,
            }
        }
    }

    fn put(output: &mut [u8], offset: usize, value: &[u8]) {
        output
            .get_mut(offset..offset + value.len())
            .expect("fixture slice")
            .copy_from_slice(value);
    }

    fn set_byte(output: &mut [u8], offset: usize, value: u8) {
        *output.get_mut(offset).expect("fixture byte") = value;
    }

    fn id(value: [u8; 32]) -> ContentId {
        ContentId::new(value).expect("nonzero fixture identity")
    }

    fn account_profile() -> Vec<u8> {
        let mut output = vec![0_u8; 48];
        put(&mut output, 0, &dclutch_account_profile_contract::v2::MAGIC);
        put(
            &mut output,
            8,
            &dclutch_account_profile_contract::v2::VERSION.to_le_bytes(),
        );
        put(
            &mut output,
            10,
            &dclutch_account_profile_contract::v2::ARTIFACT_PROFILE.to_le_bytes(),
        );
        put(&mut output, 12, &1_u16.to_le_bytes());
        put(&mut output, 20, &FIXTURE_SCALARS.to_le_bytes());
        output
    }

    fn request_profile(action: SeriesActionV3) -> Vec<u8> {
        let mut output = vec![
            0_u8;
            dclutch_request_profile_contract::HEADER_BYTES
                + usize::from(REQUEST_OPERATIONS)
                    * dclutch_request_profile_contract::OPERATION_BYTES
        ];
        put(&mut output, 0, &dclutch_request_profile_contract::MAGIC);
        put(
            &mut output,
            8,
            &dclutch_request_profile_contract::VERSION.to_le_bytes(),
        );
        put(
            &mut output,
            10,
            &dclutch_request_profile_contract::ARTIFACT_PROFILE.to_le_bytes(),
        );
        put(
            &mut output,
            12,
            &u32::try_from(SERIES_ACTION_HEADER_BYTES_V3)
                .expect("request width")
                .to_le_bytes(),
        );
        put(&mut output, 20, &REQUEST_OPERATIONS.to_le_bytes());
        put(&mut output, 24, &FIXTURE_SCALARS.to_le_bytes());

        let require_action = dclutch_request_profile_contract::HEADER_BYTES;
        set_byte(&mut output, require_action, 0);
        put(
            &mut output,
            require_action + 4,
            &SERIES_ACTION_SELECTOR_OFFSET_V3.to_le_bytes(),
        );
        put(
            &mut output,
            require_action + 12,
            &u64::from(action as u8).to_le_bytes(),
        );

        let project_proof_count =
            require_action + dclutch_request_profile_contract::OPERATION_BYTES;
        set_byte(&mut output, project_proof_count, 5);
        put(&mut output, project_proof_count + 4, &13_u32.to_le_bytes());
        put(&mut output, project_proof_count + 8, &2_u16.to_le_bytes());
        output
    }

    fn transition() -> Vec<u8> {
        let mut output = vec![
            0_u8;
            dclutch_transition_vm::v3::HEADER_BYTES
                + usize::from(TRANSITION_OPERATIONS)
                    * dclutch_transition_vm::v3::INSTRUCTION_BYTES
        ];
        put(&mut output, 0, &dclutch_transition_vm::v3::MAGIC);
        set_byte(&mut output, 4, dclutch_transition_vm::v3::VERSION);
        put(&mut output, 6, &TRANSITION_OPERATIONS.to_le_bytes());
        put(&mut output, 12, &FIXTURE_SCALARS.to_le_bytes());

        let load_offset = dclutch_transition_vm::v3::HEADER_BYTES;
        set_byte(&mut output, load_offset, 0);
        put(&mut output, load_offset + 2, &0_u16.to_le_bytes());
        put(
            &mut output,
            load_offset + 16,
            &u64::try_from(SERIES_ACTION_HEADER_BYTES_V3)
                .expect("header width")
                .to_le_bytes(),
        );

        let load_multiplier = load_offset + dclutch_transition_vm::v3::INSTRUCTION_BYTES;
        set_byte(&mut output, load_multiplier, 0);
        put(&mut output, load_multiplier + 2, &3_u16.to_le_bytes());
        put(
            &mut output,
            load_multiplier + 16,
            &u64::try_from(SERIES_WITNESS_ITEM_BYTES_V3)
                .expect("sibling width")
                .to_le_bytes(),
        );

        let multiply = load_multiplier + dclutch_transition_vm::v3::INSTRUCTION_BYTES;
        set_byte(&mut output, multiply, 17);
        put(&mut output, multiply + 2, &2_u16.to_le_bytes());
        put(&mut output, multiply + 4, &3_u16.to_le_bytes());
        put(&mut output, multiply + 6, &1_u16.to_le_bytes());
        output
    }

    fn effect(action: SeriesActionV3) -> Vec<u8> {
        let (route_count, role, request_width) = match action {
            SeriesActionV3::Prepare | SeriesActionV3::Expire => (
                3_u16,
                4_u8,
                dclutch_custody_contract::CUSTODY_REQUEST_BYTES_V1,
            ),
            SeriesActionV3::Consume => (
                1,
                0,
                dclutch_market_core_codec::SERIES_CORE_REQUEST_BYTES_V1,
            ),
            SeriesActionV3::Retire | SeriesActionV3::Close => (0, 0, 0),
        };
        let route_bytes = usize::from(route_count) * dclutch_effect_kernel::v3::ROUTE_BYTES;
        let request_bytes = usize::from(route_count) * request_width;
        let mut output =
            vec![0_u8; dclutch_effect_kernel::v3::HEADER_BYTES + route_bytes + request_bytes];
        put(&mut output, 0, &dclutch_effect_kernel::v3::MAGIC);
        set_byte(&mut output, 4, dclutch_effect_kernel::v3::VERSION);
        put(&mut output, 6, &route_count.to_le_bytes());
        put(&mut output, 12, &1_u16.to_le_bytes());
        put(&mut output, 16, &FIXTURE_SCALARS.to_le_bytes());
        for route in 0..usize::from(route_count) {
            let offset = dclutch_effect_kernel::v3::HEADER_BYTES
                + route * dclutch_effect_kernel::v3::ROUTE_BYTES;
            set_byte(&mut output, offset, role);
            set_byte(
                &mut output,
                offset + 3,
                u8::from(action == SeriesActionV3::Consume),
            );
            put(&mut output, offset + 8, &1_u16.to_le_bytes());
            put(
                &mut output,
                offset + 16,
                &u32::try_from(request_width)
                    .expect("child request width")
                    .to_le_bytes(),
            );
        }
        output
    }

    fn program_set(action: SeriesActionV3, descriptor: [u8; 32]) -> Vec<u8> {
        let mut output = vec![0_u8; 72];
        put(&mut output, 0, b"DCLTCPS1");
        put(&mut output, 8, &1_u16.to_le_bytes());
        put(&mut output, 10, &1_u16.to_le_bytes());
        put(
            &mut output,
            12,
            &SERIES_ACTION_SELECTOR_OFFSET_V3.to_le_bytes(),
        );
        set_byte(&mut output, 16, 1);
        put(&mut output, 18, &1_u16.to_le_bytes());
        put(&mut output, 32, &u32::from(action as u8).to_le_bytes());
        put(&mut output, 36, &descriptor);
        output
    }

    fn family_request(action: SeriesActionV3, template: ContentId) -> Vec<u8> {
        let occurrence = action.occurrence_bound().then_some(id([42; 32]));
        let ticket = (action != SeriesActionV3::Close).then_some(id([43; 32]));
        let proof_count = u8::from(action.occurrence_bound()) * 2;
        let ticket_revision = match action {
            SeriesActionV3::Prepare | SeriesActionV3::Close => 0,
            SeriesActionV3::Consume | SeriesActionV3::Expire | SeriesActionV3::Retire => 3,
        };
        let header = encode_series_action_header_v3(
            action,
            template,
            occurrence,
            ticket,
            7,
            ticket_revision,
            proof_count,
        )
        .expect("Series header");
        let mut output = vec![0_u8; header.len() + usize::from(proof_count) * 32];
        output
            .get_mut(..header.len())
            .expect("header destination")
            .copy_from_slice(&header);
        for (index, value) in output
            .get_mut(header.len()..)
            .expect("witness destination")
            .iter_mut()
            .enumerate()
        {
            *value = u8::try_from(index + 1).expect("bounded witness byte");
        }
        output
    }

    fn fixture(action: SeriesActionV3) -> Fixture {
        let template = id([41; 32]);
        let account = account_profile();
        let request_profile = request_profile(action);
        let transition = transition();
        let effect = effect(action);
        let strategy = ExecutionStrategyProgramV2::new(
            StrategyDispositionV2::Interpreted,
            id(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID),
            id(digest(&transition)),
            id(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2),
            None,
            id(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2),
            None,
            id(ACCELERATOR_REQUEST_SCHEMA_ID_V2),
            id(ACCELERATOR_ACK_SCHEMA_ID_V2),
        )
        .expect("interpreted strategy")
        .to_bytes();
        let descriptor = CapabilityProgramV3::new(
            id(digest(SERIES_SUCCESSOR_KIND_PREIMAGE_V3)),
            id(SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3),
            id(digest(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3)),
            id(digest(SERIES_ROOT_SCHEMA_PREIMAGE_V3)),
            id(digest(&account)),
            id(digest(SERIES_TICKET_DERIVATION_PREIMAGE_V3)),
            id([90; 32]),
            id(digest(&effect)),
            id(dclutch_request_profile_contract::SCHEMA_RELEASE_ID),
            id(digest(&request_profile)),
            id(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2),
            id(digest(&strategy)),
            u32::try_from(SERIES_STATE_BYTES_V3).expect("root width"),
        )
        .expect("Series descriptor")
        .encode();
        Fixture {
            set: program_set(action, digest(&descriptor)),
            descriptor,
            account,
            request_profile,
            strategy,
            transition,
            effect,
            request: family_request(action, template),
            template,
        }
    }

    fn projected_scalars(fixture: &Fixture, bundle: SeriesArtifactBundleV3<'_>) -> [u64; 4] {
        let input_scalars = [0_u64; 4];
        let input_identities: [[u8; 32]; 0] = [];
        let mut profile_scratch = [0_u64; 4];
        let mut profile_output = [0_u64; 4];
        let mut identity_scratch: [[u8; 32]; 0] = [];
        let mut identity_output: [[u8; 32]; 0] = [];
        project_atomic(
            bundle.request_profile,
            0,
            bundle.slices.header,
            ProjectionRegistersV1 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut profile_scratch,
                scratch_identities: &mut identity_scratch,
                output_scalars: &mut profile_output,
                output_identities: &mut identity_output,
            },
        )
        .expect("request projection");
        let mut transition_scratch = [0_u64; 4];
        let mut transition_output = [0_u64; 4];
        let mut transition_identity_scratch: [[u8; 32]; 0] = [];
        let mut transition_identity_output: [[u8; 32]; 0] = [];
        execute_fold_atomic(
            bundle.transition,
            0,
            RegisterInput {
                scalars: &profile_output,
                identities: &identity_output,
            },
            RegisterOutput {
                scalars: &mut transition_scratch,
                identities: &mut transition_identity_scratch,
            },
            RegisterOutput {
                scalars: &mut transition_output,
                identities: &mut transition_identity_output,
            },
        )
        .expect("strategy transition");
        assert_eq!(
            usize::try_from(*transition_output.get(1).expect("witness-length register"))
                .expect("witness len"),
            fixture.request.len() - SERIES_ACTION_HEADER_BYTES_V3
        );
        transition_output
    }

    #[test]
    fn every_series_action_joins_one_exact_program_set_bundle() {
        for action in [
            SeriesActionV3::Prepare,
            SeriesActionV3::Consume,
            SeriesActionV3::Expire,
            SeriesActionV3::Retire,
            SeriesActionV3::Close,
        ] {
            let fixture = fixture(action);
            let joined = authenticate_series_artifacts_v3(
                fixture.selection(),
                fixture.artifacts(),
                &fixture.request,
            )
            .expect("joined Series artifact bundle");
            assert_eq!(joined.request.action(), action);
            assert_eq!(joined.slices.header.len(), SERIES_ACTION_HEADER_BYTES_V3);
            assert_eq!(
                joined.slices.witness.len(),
                usize::from(joined.request.proof_count()) * 32
            );
            let _ = projected_scalars(&fixture, joined);
        }
    }

    #[test]
    fn consume_borrows_only_the_exact_authenticated_proof_suffix() {
        let fixture = fixture(SeriesActionV3::Consume);
        let bundle = authenticate_series_artifacts_v3(
            fixture.selection(),
            fixture.artifacts(),
            &fixture.request,
        )
        .expect("Consume bundle");
        let scalars = projected_scalars(&fixture, bundle);
        let identities: [[u8; 32]; 0] = [];
        let invocation = bundle
            .effect
            .resolved_invocation(0, 0, 0, &scalars, &identities)
            .expect("resolved Core invocation");
        let core_request = [17_u8; dclutch_market_core_codec::SERIES_CORE_REQUEST_BYTES_V1];
        let selected = validate_series_consume_invocation_v3(
            bundle,
            invocation,
            &core_request,
            &fixture.request,
        )
        .expect("exact borrowed witness");
        assert_eq!(selected.core_request, core_request);
        assert_eq!(
            selected.witness,
            fixture
                .request
                .get(SERIES_ACTION_HEADER_BYTES_V3..)
                .expect("witness")
        );
        assert_eq!(
            selected.child_request_digest,
            hashv(&[&core_request, selected.witness]).to_bytes()
        );

        let mut padded = fixture.request.clone();
        padded.push(0);
        assert_eq!(
            validate_series_consume_invocation_v3(bundle, invocation, &core_request, &padded,),
            Err(SeriesArtifactErrorV3::Request)
        );
    }

    #[test]
    fn action_descriptor_profile_and_witness_substitution_refuse() {
        let fixture = fixture(SeriesActionV3::Prepare);
        let mut wrong_selection = fixture.selection();
        *wrong_selection
            .program_set
            .get_mut(0)
            .expect("selection mutation") ^= 1;
        assert_eq!(
            authenticate_series_artifacts_v3(
                wrong_selection,
                fixture.artifacts(),
                &fixture.request,
            ),
            Err(SeriesArtifactErrorV3::ContentIdentity)
        );

        let mut wrong_descriptor = fixture.descriptor;
        *wrong_descriptor.get_mut(64).expect("descriptor mutation") ^= 1;
        assert_eq!(
            authenticate_series_artifacts_v3(
                fixture.selection(),
                SeriesArtifactBytesV3 {
                    descriptor: &wrong_descriptor,
                    ..fixture.artifacts()
                },
                &fixture.request,
            ),
            Err(SeriesArtifactErrorV3::ContentIdentity)
        );

        let mut wrong_profile = fixture.request_profile.clone();
        *wrong_profile.get_mut(36).expect("profile mutation") ^= 1;
        assert_eq!(
            authenticate_series_artifacts_v3(
                fixture.selection(),
                SeriesArtifactBytesV3 {
                    request_profile: &wrong_profile,
                    ..fixture.artifacts()
                },
                &fixture.request,
            ),
            Err(SeriesArtifactErrorV3::ContentIdentity)
        );

        let mut wrong_action = fixture.request.clone();
        *wrong_action.get_mut(12).expect("action mutation") = SeriesActionV3::Expire as u8;
        assert!(
            authenticate_series_artifacts_v3(
                fixture.selection(),
                fixture.artifacts(),
                &wrong_action,
            )
            .is_err()
        );

        let mut short_witness = fixture.request.clone();
        short_witness.pop();
        assert_eq!(
            authenticate_series_artifacts_v3(
                fixture.selection(),
                fixture.artifacts(),
                &short_witness,
            ),
            Err(SeriesArtifactErrorV3::Request)
        );
    }
}
