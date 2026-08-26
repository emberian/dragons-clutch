//! Complete physical plans behind authenticated recurring-Series actions.
//!
//! The caller supplies only values already authenticated by the common Trading
//! outer: the finalized Series content projection, current Product projection,
//! physical Custody observations, and post-strategy generic effect invocation.
//! This module joins those facts into one plan but performs no CPI or write.
//! Trading executes the returned child calls transactionally and commits the
//! replay candidates only after every immediate receipt accepts.

use dclutch_custody_contract::CustodyRequestV1;
use dclutch_series_v3_kernel::{AccountKeyV3, AuthenticatedProductProjectionV2};
use solana_program::pubkey::Pubkey;

use super::{
    SeriesConsumeCompositionV3,
    artifacts_v3::{
        SeriesArtifactBundleV3, SeriesArtifactErrorV3, SeriesConsumeInvocationV3,
        validate_series_consume_invocation_v3,
    },
    custody_v3::{
        SeriesCustodyPhysicalV3, SeriesCustodyProjectionErrorV3, project_prepare_custody_v3,
        project_terminal_custody_v3,
    },
    lifecycle::OccurrenceCommitPlanV3,
    projector::{AuthenticatedSeriesActionV3, SeriesProjectorErrorV3},
    state::{SeriesStateV3, TicketStateV3},
};

/// Stable refusal from the complete Series physical-plan join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesPhysicalComposerErrorV3 {
    /// Authenticated Series content, replay, schedule, or composition refused.
    Projector(SeriesProjectorErrorV3),
    /// The generic selected effect route did not bind the exact Core proof suffix.
    Artifact(SeriesArtifactErrorV3),
    /// A semantic escrow edge could not project into canonical Custody V1.
    Custody(SeriesCustodyProjectionErrorV3),
    /// The IR-owned Core request differed from the Series semantic composition.
    CoreRequestMismatch,
}

impl From<SeriesProjectorErrorV3> for SeriesPhysicalComposerErrorV3 {
    fn from(value: SeriesProjectorErrorV3) -> Self {
        Self::Projector(value)
    }
}

impl From<SeriesArtifactErrorV3> for SeriesPhysicalComposerErrorV3 {
    fn from(value: SeriesArtifactErrorV3) -> Self {
        Self::Artifact(value)
    }
}

impl From<SeriesCustodyProjectionErrorV3> for SeriesPhysicalComposerErrorV3 {
    fn from(value: SeriesCustodyProjectionErrorV3) -> Self {
        Self::Custody(value)
    }
}

/// Result alias for complete recurring-Series physical plans.
pub type Result<T> = core::result::Result<T, SeriesPhysicalComposerErrorV3>;

/// Prepare replay candidate plus exact Init/Open/Lock Custody requests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesPreparePhysicalPlanV3 {
    replay: OccurrenceCommitPlanV3,
    custody: [CustodyRequestV1; 3],
    ticket_top_up: u64,
    ticket_donation_refund: u64,
}

impl SeriesPreparePhysicalPlanV3 {
    /// Trading-owned replay candidate committed after all three Custody receipts.
    pub const fn replay(self) -> OccurrenceCommitPlanV3 {
        self.replay
    }

    /// Exact InitializeReplay/OpenVault/Transfer requests in execution order.
    pub const fn custody(self) -> [CustodyRequestV1; 3] {
        self.custody
    }

    /// Exact native lamports needed to make the Ticket replay account Rent-exempt.
    pub const fn ticket_top_up(self) -> u64 {
        self.ticket_top_up
    }

    /// Preexisting Ticket-account excess returned instead of reclassified.
    pub const fn ticket_donation_refund(self) -> u64 {
        self.ticket_donation_refund
    }
}

/// Consume composition plus exact Core and terminal Custody calls.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumePhysicalPlanV3<'a> {
    composition: SeriesConsumeCompositionV3,
    core: SeriesConsumeInvocationV3<'a>,
    custody: [CustodyRequestV1; 3],
}

impl<'a> SeriesConsumePhysicalPlanV3<'a> {
    /// Complete semantic Core/Custody/funding/replay composition.
    pub const fn composition(self) -> SeriesConsumeCompositionV3 {
        self.composition
    }

    /// Exact `SeriesCoreRequestV1 || occurrence proof` child instruction.
    pub const fn core(self) -> SeriesConsumeInvocationV3<'a> {
        self.core
    }

    /// Exact transfer-to-Hoard/close-Vault/close-replay Custody requests.
    pub const fn custody(self) -> [CustodyRequestV1; 3] {
        self.custody
    }
}

/// Expire replay candidate plus exact refund and escrow-cleanup Custody calls.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesExpirePhysicalPlanV3 {
    replay: OccurrenceCommitPlanV3,
    custody: [CustodyRequestV1; 3],
}

impl SeriesExpirePhysicalPlanV3 {
    /// Trading-owned replay candidate committed after every Custody receipt.
    pub const fn replay(self) -> OccurrenceCommitPlanV3 {
        self.replay
    }

    /// Exact refund/close-Vault/close-replay requests in execution order.
    pub const fn custody(self) -> [CustodyRequestV1; 3] {
        self.custody
    }
}

/// Compose one Prepare without creating a parallel collateral authority.
#[allow(clippy::too_many_arguments)]
pub fn compose_prepare_physical_v3(
    action: AuthenticatedSeriesActionV3<'_>,
    product: AuthenticatedProductProjectionV2,
    registry_program: AccountKeyV3,
    series: SeriesStateV3,
    now_slot: u64,
    current_ticket_lamports: u64,
    ticket_state_rent: u64,
    custody_physical: SeriesCustodyPhysicalV3,
) -> Result<SeriesPreparePhysicalPlanV3> {
    let (replay, ticket_top_up, ticket_donation_refund) =
        action.plan_prepare(series, now_slot, current_ticket_lamports, ticket_state_rent)?;
    let custody = project_prepare_custody_v3(
        action.plan_prepare_escrow(product, registry_program)?,
        custody_physical,
    )?;
    Ok(SeriesPreparePhysicalPlanV3 {
        replay,
        custody,
        ticket_top_up,
        ticket_donation_refund,
    })
}

/// Compose one Consume through the selected generic Core effect route.
///
/// The returned order is Core Found first, then the three terminal Custody
/// calls, then replay commit. A caller may stage requests in another order but
/// may not persist either replay candidate before every child receipt accepts.
#[allow(clippy::too_many_arguments)]
pub fn compose_consume_physical_v3<'a>(
    action: AuthenticatedSeriesActionV3<'_>,
    bundle: SeriesArtifactBundleV3<'_>,
    invocation: dclutch_effect_kernel::v3::ResolvedInvocationV3,
    ir_request_bank: &'a [u8],
    family_request: &'a [u8],
    product: AuthenticatedProductProjectionV2,
    registry_program: AccountKeyV3,
    ticket_state_key: Pubkey,
    series_bytes: &[u8],
    ticket_state_bytes: &[u8],
    now_slot: u64,
    custody_physical: SeriesCustodyPhysicalV3,
) -> Result<SeriesConsumePhysicalPlanV3<'a>> {
    let composition = action.compose_consume(
        product,
        registry_program,
        ticket_state_key,
        series_bytes,
        ticket_state_bytes,
        now_slot,
    )?;
    let core =
        validate_series_consume_invocation_v3(bundle, invocation, ir_request_bank, family_request)?;
    let expected = composition
        .core_request()
        .encode()
        .map_err(|_| SeriesPhysicalComposerErrorV3::CoreRequestMismatch)?;
    if core.core_request != expected {
        return Err(SeriesPhysicalComposerErrorV3::CoreRequestMismatch);
    }
    let custody = project_terminal_custody_v3(composition.escrow(), custody_physical)?;
    Ok(SeriesConsumePhysicalPlanV3 {
        composition,
        core,
        custody,
    })
}

/// Compose one post-deadline Expire, including complete escrow cleanup.
#[allow(clippy::too_many_arguments)]
pub fn compose_expire_physical_v3(
    action: AuthenticatedSeriesActionV3<'_>,
    product: AuthenticatedProductProjectionV2,
    registry_program: AccountKeyV3,
    ticket_state_key: Pubkey,
    series: SeriesStateV3,
    ticket_state: TicketStateV3,
    now_slot: u64,
    custody_physical: SeriesCustodyPhysicalV3,
) -> Result<SeriesExpirePhysicalPlanV3> {
    let replay = action.plan_expire(ticket_state_key, series, ticket_state, now_slot)?;
    let custody = project_terminal_custody_v3(
        action.plan_expire_escrow(product, registry_program)?,
        custody_physical,
    )?;
    Ok(SeriesExpirePhysicalPlanV3 { replay, custody })
}

#[cfg(test)]
mod tests {
    extern crate std;

    use dclutch_core_contract::ContentId;
    use dclutch_custody_contract::OperationV1;
    use dclutch_series_v3_kernel::{
        SERIES_OCCURRENCE_BYTES_V3, SERIES_TEMPLATE_BYTES_V3, SERIES_TICKET_BYTES_V3, admit_ticket,
        generated, occurrence_content_id, template_content_id,
    };
    use std::{vec, vec::Vec};

    use super::*;
    use crate::series::{
        instruction::{SeriesActionRequestV3, SeriesActionV3, encode_series_action_header_v3},
        projector::authenticate_action_content_v3,
    };

    fn put(output: &mut [u8], offset: usize, value: &[u8]) {
        output
            .get_mut(offset..offset + value.len())
            .expect("fixture field")
            .copy_from_slice(value);
    }

    fn fixture() -> (
        [u8; SERIES_TEMPLATE_BYTES_V3],
        [u8; SERIES_OCCURRENCE_BYTES_V3],
        [u8; SERIES_TICKET_BYTES_V3],
    ) {
        let mut occurrence = generated::SERIES_EXAMPLE_OCCURRENCE_V3;
        put(
            &mut occurrence,
            generated::SERIES_OCCURRENCE_INDEX_OFFSET_V3,
            &0_u32.to_le_bytes(),
        );
        put(
            &mut occurrence,
            generated::SERIES_OCCURRENCE_SCHEDULED_SLOT_OFFSET_V3,
            &100_u64.to_le_bytes(),
        );
        let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence ID");

        let mut template = generated::SERIES_EXAMPLE_TEMPLATE_V3;
        put(
            &mut template,
            generated::SERIES_TEMPLATE_OCCURRENCE_COUNT_OFFSET_V3,
            &1_u32.to_le_bytes(),
        );
        put(
            &mut template,
            generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
            &occurrence_id.to_bytes(),
        );
        let template_id = template_content_id(&template).expect("Template ID");

        let mut ticket = generated::SERIES_EXAMPLE_TICKET_V3;
        put(
            &mut ticket,
            generated::SERIES_TICKET_INDEX_OFFSET_V3,
            &0_u32.to_le_bytes(),
        );
        put(
            &mut ticket,
            generated::SERIES_TICKET_TEMPLATE_OFFSET_V3,
            &template_id.to_bytes(),
        );
        put(
            &mut ticket,
            generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
            &occurrence_id.to_bytes(),
        );
        (template, occurrence, ticket)
    }

    fn action<'a>(
        selected: SeriesActionV3,
        expected_series_revision: u64,
        expected_ticket_revision: u64,
        request_storage: &'a mut Vec<u8>,
        template: &[u8; SERIES_TEMPLATE_BYTES_V3],
        occurrence: &[u8; SERIES_OCCURRENCE_BYTES_V3],
        ticket: &[u8; SERIES_TICKET_BYTES_V3],
    ) -> AuthenticatedSeriesActionV3<'a> {
        let template_id = template_content_id(template).expect("Template ID");
        let occurrence_id = occurrence_content_id(occurrence).expect("occurrence ID");
        let ticket_id = admit_ticket(ticket).expect("Ticket ID").content_id();
        request_storage.extend_from_slice(
            &encode_series_action_header_v3(
                selected,
                template_id,
                Some(occurrence_id),
                Some(ticket_id),
                expected_series_revision,
                expected_ticket_revision,
                0,
            )
            .expect("request"),
        );
        let request = SeriesActionRequestV3::decode(request_storage).expect("decode request");
        authenticate_action_content_v3(request, template, Some(occurrence), Some(ticket))
            .expect("content join")
    }

    fn product(action: AuthenticatedSeriesActionV3<'_>) -> AuthenticatedProductProjectionV2 {
        AuthenticatedProductProjectionV2::new(
            action
                .occurrence()
                .expect("occurrence")
                .occurrence()
                .product_record(),
            ContentId::new([61; 32]).expect("stable Product"),
            ContentId::new([62; 32]).expect("result domain"),
        )
    }

    const fn custody_physical() -> SeriesCustodyPhysicalV3 {
        SeriesCustodyPhysicalV3 {
            caller_program: [1; 32],
            parent_request_digest: [2; 32],
            payer: [3; 32],
            mint: [4; 32],
            token_program: [5; 32],
            founder_source: [6; 32],
            escrow_vault: [7; 32],
            hoard_vault: [8; 32],
            refund_destination: [9; 32],
            replay_rent_lamports: 10,
            vault_rent_lamports: 11,
        }
    }

    #[test]
    fn prepare_and_expire_include_all_custody_edges_before_replay_commit() {
        let (template_bytes, occurrence_bytes, ticket_bytes) = fixture();
        let template =
            dclutch_series_v3_kernel::TemplateV3::decode(&template_bytes).expect("Template");
        let series = SeriesStateV3::new(template.close_rent());
        let mut prepare_request = Vec::new();
        let prepare = action(
            SeriesActionV3::Prepare,
            0,
            0,
            &mut prepare_request,
            &template_bytes,
            &occurrence_bytes,
            &ticket_bytes,
        );
        let prepare_plan = compose_prepare_physical_v3(
            prepare,
            product(prepare),
            AccountKeyV3::new([59; 32]).expect("Registry"),
            series,
            100,
            0,
            12,
            custody_physical(),
        )
        .expect("complete Prepare plan");
        assert_eq!(
            prepare_plan.custody().map(|request| request.operation),
            [
                OperationV1::InitializeReplay,
                OperationV1::OpenVault,
                OperationV1::Transfer,
            ]
        );
        assert!(prepare_plan.ticket_top_up() > 12);
        assert_eq!(prepare_plan.ticket_donation_refund(), 0);

        let prepared_series = prepare_plan.replay().series_after();
        let prepared_ticket = prepare_plan.replay().ticket_after();
        let mut expire_request = Vec::new();
        let expire = action(
            SeriesActionV3::Expire,
            prepared_series.revision(),
            prepared_ticket.revision(),
            &mut expire_request,
            &template_bytes,
            &occurrence_bytes,
            &ticket_bytes,
        );
        let expire_plan = compose_expire_physical_v3(
            expire,
            product(expire),
            AccountKeyV3::new([59; 32]).expect("Registry"),
            Pubkey::new_unique(),
            prepared_series,
            prepared_ticket,
            u64::MAX,
            custody_physical(),
        )
        .expect("complete Expire plan");
        assert_eq!(
            expire_plan.custody().map(|request| request.operation),
            [
                OperationV1::Transfer,
                OperationV1::CloseVault,
                OperationV1::CloseReplay,
            ]
        );
        let (series_after, ticket_after) = expire_plan
            .replay()
            .commit_controller()
            .expect("commit candidate only after receipts");
        assert_ne!(series_after, [0; 64]);
        assert_ne!(ticket_after, [0; 64]);
    }

    #[test]
    fn action_mismatch_refuses_before_any_physical_request_is_returned() {
        let (template, occurrence, ticket) = fixture();
        let mut request = vec![];
        let prepare = action(
            SeriesActionV3::Prepare,
            0,
            0,
            &mut request,
            &template,
            &occurrence,
            &ticket,
        );
        assert!(
            compose_expire_physical_v3(
                prepare,
                product(prepare),
                AccountKeyV3::new([59; 32]).expect("Registry"),
                Pubkey::new_unique(),
                SeriesStateV3::new(1),
                TicketStateV3::prepared(admit_ticket(&ticket).expect("Ticket ID").content_id(),),
                u64::MAX,
                custody_physical(),
            )
            .is_err()
        );
    }
}
