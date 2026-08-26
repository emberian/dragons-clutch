//! Content and replay projection behind the canonical Trading hot outer.
//!
//! The common outer authenticates accounts, finalized-record provenance, the
//! current Trading deployment, and the immutable composite root.  This module
//! then joins the sparse family request to exact Template/Occurrence/Ticket
//! bytes and exposes only action-matched lifecycle planners.

use dclutch_core_contract::ContentId;
use dclutch_series_v3_kernel::request::{AdmittedSeriesActionV3, admit_series_action_v3};
use solana_program::pubkey::Pubkey;

use super::{
    AccountKeyV3, AdmittedOccurrenceV3, AdmittedTicketV3, AuthenticatedProductProjectionV2,
    ConsumeSeriesEscrowPlanV3, PrepareSeriesEscrowPlanV3, SeriesConsumeCompositionErrorV3,
    SeriesConsumeCompositionV3, SeriesV3Error, TemplateV3, TerminalSeriesEscrowPlanV3,
    compose_series_consume_v3, consume_series_escrow_v3, expire_series_escrow_v3,
    instruction::{SeriesActionRequestV3, SeriesActionV3, SeriesInstructionErrorV3},
    lifecycle::{
        ClosePlanV3, LifecycleErrorV3, OccurrenceCommitPlanV3, PendingFundingPlanV3, RetirePlanV3,
        plan_close, plan_consume, plan_expire, plan_prepare, plan_retire,
    },
    pre_founding_series_escrow, prepare_series_escrow_v3,
    state::{SeriesStateV3, TicketStateV3},
};

/// Refusal from the Series hot content/projector boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesProjectorErrorV3 {
    /// Action-specific finalized content accounts were missing or extraneous.
    Frame,
    /// A content hash, Template projection, or Ticket join refused.
    Content,
    /// Schedule, replay, funding, or Core-request planning refused.
    Lifecycle(LifecycleErrorV3),
    /// The joint Core/Custody/funding/replay composition refused.
    Composition(SeriesConsumeCompositionErrorV3),
}

impl From<SeriesV3Error> for SeriesProjectorErrorV3 {
    fn from(_: SeriesV3Error) -> Self {
        Self::Content
    }
}

impl From<LifecycleErrorV3> for SeriesProjectorErrorV3 {
    fn from(value: LifecycleErrorV3) -> Self {
        Self::Lifecycle(value)
    }
}

impl From<SeriesConsumeCompositionErrorV3> for SeriesProjectorErrorV3 {
    fn from(value: SeriesConsumeCompositionErrorV3) -> Self {
        Self::Composition(value)
    }
}

/// Exact content join selected by one decoded family request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSeriesActionV3<'a> {
    admitted: AdmittedSeriesActionV3<'a>,
}

impl<'a> AuthenticatedSeriesActionV3<'a> {
    /// Selected Series action.
    pub const fn action(self) -> SeriesActionV3 {
        self.admitted.action()
    }
    /// Exact finalized Template/config.
    pub const fn template(self) -> TemplateV3 {
        self.admitted.template()
    }
    /// Exact domain-separated Template identity.
    pub const fn template_id(self) -> ContentId {
        self.admitted.template_id()
    }
    /// Exact occurrence admission, present only on occurrence actions.
    pub const fn occurrence(self) -> Option<AdmittedOccurrenceV3> {
        self.admitted.occurrence()
    }
    /// Exact Ticket admission, absent only on root Close.
    pub const fn ticket(self) -> Option<AdmittedTicketV3> {
        self.admitted.ticket()
    }

    /// Plan one dust-tolerant replay-account preparation.
    pub fn plan_prepare(
        self,
        series: SeriesStateV3,
        now_slot: u64,
        current_ticket_lamports: u64,
        ticket_state_rent: u64,
    ) -> Result<(OccurrenceCommitPlanV3, u64, u64), SeriesProjectorErrorV3> {
        if self.action() != SeriesActionV3::Prepare {
            return Err(SeriesProjectorErrorV3::Frame);
        }
        Ok(plan_prepare(
            self.required_occurrence()?,
            self.required_ticket()?,
            series,
            self.admitted.request().expected_series_revision(),
            now_slot,
            current_ticket_lamports,
            ticket_state_rent,
        )?)
    }

    /// Project the canonical pre-founding Custody replay and collateral lock.
    pub fn plan_prepare_escrow(
        self,
        product: AuthenticatedProductProjectionV2,
        registry_program: AccountKeyV3,
    ) -> Result<PrepareSeriesEscrowPlanV3, SeriesProjectorErrorV3> {
        if self.action() != SeriesActionV3::Prepare {
            return Err(SeriesProjectorErrorV3::Frame);
        }
        let escrow = pre_founding_series_escrow(
            self.required_occurrence()?,
            self.required_ticket()?,
            product,
            registry_program,
        )?;
        Ok(prepare_series_escrow_v3(escrow))
    }

    /// Plan one atomic Ticket-to-Found consumption.
    #[allow(clippy::too_many_arguments)]
    pub fn plan_consume(
        self,
        product: AuthenticatedProductProjectionV2,
        ticket_state_key: Pubkey,
        series: SeriesStateV3,
        ticket_state: TicketStateV3,
        now_slot: u64,
        funding: PendingFundingPlanV3,
    ) -> Result<OccurrenceCommitPlanV3, SeriesProjectorErrorV3> {
        if self.action() != SeriesActionV3::Consume {
            return Err(SeriesProjectorErrorV3::Frame);
        }
        Ok(plan_consume(
            self.required_occurrence()?,
            self.required_ticket()?,
            product,
            ticket_state_key,
            series,
            ticket_state,
            self.admitted.request().expected_series_revision(),
            self.admitted.request().expected_ticket_revision(),
            now_slot,
            funding,
        )?)
    }

    /// Join raw replay prestates to the exact Core, escrow, and funding effects.
    #[allow(clippy::too_many_arguments)]
    pub fn compose_consume(
        self,
        product: AuthenticatedProductProjectionV2,
        registry_program: AccountKeyV3,
        ticket_state_key: Pubkey,
        series_bytes: &[u8],
        ticket_state_bytes: &[u8],
        now_slot: u64,
    ) -> Result<SeriesConsumeCompositionV3, SeriesProjectorErrorV3> {
        if self.action() != SeriesActionV3::Consume {
            return Err(SeriesProjectorErrorV3::Frame);
        }
        Ok(compose_series_consume_v3(
            self.required_occurrence()?,
            self.required_ticket()?,
            product,
            registry_program,
            AccountKeyV3::new(ticket_state_key.to_bytes())?,
            series_bytes,
            ticket_state_bytes,
            now_slot,
            self.admitted.request().expected_series_revision(),
            self.admitted.request().expected_ticket_revision(),
        )?)
    }

    /// Project the exact post-Found SeriesEscrow-to-Hoard effect.
    pub fn plan_consume_escrow(
        self,
        product: AuthenticatedProductProjectionV2,
        registry_program: AccountKeyV3,
    ) -> Result<ConsumeSeriesEscrowPlanV3, SeriesProjectorErrorV3> {
        if self.action() != SeriesActionV3::Consume {
            return Err(SeriesProjectorErrorV3::Frame);
        }
        let escrow = pre_founding_series_escrow(
            self.required_occurrence()?,
            self.required_ticket()?,
            product,
            registry_program,
        )?;
        Ok(consume_series_escrow_v3(escrow))
    }

    /// Plan one exact expiry after the retry deadline.
    pub fn plan_expire(
        self,
        ticket_state_key: Pubkey,
        series: SeriesStateV3,
        ticket_state: TicketStateV3,
        now_slot: u64,
    ) -> Result<OccurrenceCommitPlanV3, SeriesProjectorErrorV3> {
        if self.action() != SeriesActionV3::Expire {
            return Err(SeriesProjectorErrorV3::Frame);
        }
        Ok(plan_expire(
            self.required_occurrence()?,
            self.required_ticket()?,
            ticket_state_key,
            series,
            ticket_state,
            self.admitted.request().expected_series_revision(),
            self.admitted.request().expected_ticket_revision(),
            now_slot,
        )?)
    }

    /// Project the exact post-deadline SeriesEscrow refund effect.
    pub fn plan_expire_escrow(
        self,
        product: AuthenticatedProductProjectionV2,
        registry_program: AccountKeyV3,
    ) -> Result<TerminalSeriesEscrowPlanV3, SeriesProjectorErrorV3> {
        if self.action() != SeriesActionV3::Expire {
            return Err(SeriesProjectorErrorV3::Frame);
        }
        let escrow = pre_founding_series_escrow(
            self.required_occurrence()?,
            self.required_ticket()?,
            product,
            registry_program,
        )?;
        Ok(expire_series_escrow_v3(escrow))
    }

    /// Plan deletion of one terminal replay account.
    pub fn plan_retire(
        self,
        series: SeriesStateV3,
        ticket_state: TicketStateV3,
        observed_ticket_lamports: u64,
    ) -> Result<RetirePlanV3, SeriesProjectorErrorV3> {
        if self.action() != SeriesActionV3::Retire {
            return Err(SeriesProjectorErrorV3::Frame);
        }
        Ok(plan_retire(
            self.admitted.template().occurrence_count(),
            series,
            ticket_state,
            self.required_ticket()?,
            self.admitted.request().expected_series_revision(),
            self.admitted.request().expected_ticket_revision(),
            observed_ticket_lamports,
        )?)
    }

    /// Plan terminal root close without fabricating a Market authority.
    pub fn plan_close(
        self,
        series: SeriesStateV3,
        observed_root_lamports: u64,
        exact_root_rent: u64,
    ) -> Result<ClosePlanV3, SeriesProjectorErrorV3> {
        if self.action() != SeriesActionV3::Close {
            return Err(SeriesProjectorErrorV3::Frame);
        }
        Ok(plan_close(
            self.admitted.template(),
            series,
            self.admitted.request().expected_series_revision(),
            observed_root_lamports,
            exact_root_rent,
        )?)
    }

    fn required_occurrence(self) -> Result<AdmittedOccurrenceV3, SeriesProjectorErrorV3> {
        self.admitted
            .required_occurrence()
            .map_err(|_| SeriesProjectorErrorV3::Frame)
    }

    fn required_ticket(self) -> Result<AdmittedTicketV3, SeriesProjectorErrorV3> {
        self.admitted
            .required_ticket()
            .map_err(|_| SeriesProjectorErrorV3::Frame)
    }
}

/// Join one sparse request to its exact finalized semantic records.
///
/// Finalized-record owner/PDA/cursor/Rent authentication is performed by the
/// common outer before it passes these borrowed bytes.  Extraneous accounts
/// are refused just as strongly as missing ones so terminal actions cannot
/// smuggle an occurrence proof or substitute an unrelated Ticket.
pub fn authenticate_action_content_v3<'a>(
    request: SeriesActionRequestV3<'a>,
    template_bytes: &[u8],
    occurrence_bytes: Option<&[u8]>,
    ticket_bytes: Option<&[u8]>,
) -> Result<AuthenticatedSeriesActionV3<'a>, SeriesProjectorErrorV3> {
    let admitted = admit_series_action_v3(
        request.bytes(),
        template_bytes,
        occurrence_bytes,
        ticket_bytes,
    )
    .map_err(|error| match error {
        SeriesInstructionErrorV3::Action => SeriesProjectorErrorV3::Frame,
        SeriesInstructionErrorV3::Encoding
        | SeriesInstructionErrorV3::Identity
        | SeriesInstructionErrorV3::Proof
        | SeriesInstructionErrorV3::ImmutableContent => SeriesProjectorErrorV3::Content,
    })?;
    if admitted.request() != request {
        return Err(SeriesProjectorErrorV3::Content);
    }
    Ok(AuthenticatedSeriesActionV3 { admitted })
}

#[cfg(test)]
mod tests {
    extern crate std;

    use dclutch_series_v3_kernel::{admit_ticket, template_content_id};
    use std::vec::Vec;

    use super::*;
    use crate::series::{
        SERIES_OCCURRENCE_BYTES_V3, SERIES_TEMPLATE_BYTES_V3, SERIES_TICKET_BYTES_V3, generated,
        instruction::{SeriesActionV3, encode_series_action_header_v3},
        occurrence_content_id,
    };

    fn put(output: &mut [u8], offset: usize, value: &[u8]) {
        output
            .get_mut(offset..offset + value.len())
            .expect("fixture field")
            .copy_from_slice(value);
    }

    fn occurrence_fixture() -> (
        Vec<u8>,
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
        let ticket_id = admit_ticket(&ticket).expect("Ticket ID").content_id();
        let header = encode_series_action_header_v3(
            SeriesActionV3::Consume,
            template_id,
            Some(occurrence_id),
            Some(ticket_id),
            4,
            5,
            0,
        )
        .expect("request");
        (Vec::from(header), template, occurrence, ticket)
    }

    #[test]
    fn occurrence_request_joins_all_three_exact_content_records() {
        let (request, template, occurrence, ticket) = occurrence_fixture();
        let decoded = SeriesActionRequestV3::decode(&request).expect("decode");
        let accepted =
            authenticate_action_content_v3(decoded, &template, Some(&occurrence), Some(&ticket))
                .expect("content join");
        assert_eq!(accepted.action(), SeriesActionV3::Consume);
        assert_eq!(
            accepted.occurrence().expect("occurrence").template_id(),
            accepted.template_id()
        );

        let mut substituted = request.clone();
        *substituted.get_mut(48).expect("occurrence identity") ^= 1;
        let substituted = SeriesActionRequestV3::decode(&substituted).expect("hostile decodes");
        assert_eq!(
            authenticate_action_content_v3(
                substituted,
                &template,
                Some(&occurrence),
                Some(&ticket),
            ),
            Err(SeriesProjectorErrorV3::Content)
        );

        let product = AuthenticatedProductProjectionV2::new(
            accepted
                .occurrence()
                .expect("occurrence")
                .occurrence()
                .product_record(),
            ContentId::new([61; 32]).expect("stable Product"),
            ContentId::new([62; 32]).expect("result domain"),
        );
        let consume = accepted
            .plan_consume_escrow(product, AccountKeyV3::new([59; 32]).expect("Registry"))
            .expect("Consume escrow effect");
        assert_eq!(consume.source_replay_revision(), 3);
        assert_eq!(
            consume.amount(),
            accepted
                .required_ticket()
                .expect("ticket")
                .ticket()
                .funds()
                .hoard_principal()
        );
        assert_eq!(
            accepted.plan_prepare_escrow(product, AccountKeyV3::new([59; 32]).expect("Registry")),
            Err(SeriesProjectorErrorV3::Frame)
        );
    }

    #[test]
    fn authenticated_consume_exposes_one_atomic_stateless_composition() {
        let (_, template_bytes, occurrence_bytes, ticket_bytes) = occurrence_fixture();
        let template_id = template_content_id(&template_bytes).expect("Template ID");
        let occurrence_id = occurrence_content_id(&occurrence_bytes).expect("occurrence ID");
        let admitted_ticket = admit_ticket(&ticket_bytes).expect("Ticket");
        let request = encode_series_action_header_v3(
            SeriesActionV3::Consume,
            template_id,
            Some(occurrence_id),
            Some(admitted_ticket.content_id()),
            1,
            0,
            0,
        )
        .expect("Consume request");
        let accepted = authenticate_action_content_v3(
            SeriesActionRequestV3::decode(&request).expect("request decode"),
            &template_bytes,
            Some(&occurrence_bytes),
            Some(&ticket_bytes),
        )
        .expect("content join");
        let product = AuthenticatedProductProjectionV2::new(
            accepted
                .occurrence()
                .expect("occurrence")
                .occurrence()
                .product_record(),
            ContentId::new([61; 32]).expect("stable Product"),
            ContentId::new([62; 32]).expect("result domain"),
        );
        let template = accepted.template();
        let series = SeriesStateV3::new(template.close_rent())
            .prepare_ticket(0)
            .expect("prepared root")
            .encode(template.occurrence_count())
            .expect("root bytes");
        let ticket_state = TicketStateV3::prepared(admitted_ticket.content_id()).encode();
        let composition = accepted
            .compose_consume(
                product,
                AccountKeyV3::new([59; 32]).expect("Registry"),
                Pubkey::new_unique(),
                &series,
                &ticket_state,
                100,
            )
            .expect("atomic composition");
        assert_eq!(
            composition.core_request().action(),
            dclutch_market_core_codec::SeriesCoreActionV1::Consume
        );
        assert_eq!(composition.escrow().source_replay_revision(), 3);
        assert_eq!(
            composition.escrow().amount(),
            composition.core_request().hoard_principal()
        );
    }

    #[test]
    fn close_refuses_extraneous_occurrence_or_ticket_content() {
        let (_, template, occurrence, ticket) = occurrence_fixture();
        let template_id = template_content_id(&template).expect("Template ID");
        let close =
            encode_series_action_header_v3(SeriesActionV3::Close, template_id, None, None, 8, 0, 0)
                .expect("close");
        let close = SeriesActionRequestV3::decode(&close).expect("close decode");
        assert!(authenticate_action_content_v3(close, &template, None, None).is_ok());
        assert_eq!(
            authenticate_action_content_v3(close, &template, Some(&occurrence), Some(&ticket)),
            Err(SeriesProjectorErrorV3::Frame)
        );
    }
}
