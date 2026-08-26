//! Content and replay projection behind the canonical Trading hot outer.
//!
//! The common outer authenticates accounts, finalized-record provenance, the
//! current Trading deployment, and the immutable composite root.  This module
//! then joins the sparse family request to exact Template/Occurrence/Ticket
//! bytes and exposes only action-matched lifecycle planners.

use dclutch_core_contract::ContentId;
use solana_program::pubkey::Pubkey;

use super::{
    AccountKeyV3, AdmittedOccurrenceV3, AdmittedTicketV3, AuthenticatedProductProjectionV2,
    PrepareSeriesEscrowPlanV3, SeriesEscrowEffectV3, SeriesV3Error, TemplateV3,
    admit_occurrence_bytes, admit_ticket, consume_series_escrow_v3, expire_series_escrow_v3,
    instruction::{SeriesActionRequestV3, SeriesActionV3},
    lifecycle::{
        ClosePlanV3, LifecycleErrorV3, OccurrenceCommitPlanV3, PendingFundingPlanV3, RetirePlanV3,
        plan_close, plan_consume, plan_expire, plan_prepare, plan_retire,
    },
    pre_founding_series_escrow, prepare_series_escrow_v3,
    state::{SeriesStateV3, TicketStateV3},
    template_content_id,
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

/// Exact content join selected by one decoded family request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSeriesActionV3<'a> {
    request: SeriesActionRequestV3<'a>,
    template: TemplateV3,
    template_id: ContentId,
    occurrence: Option<AdmittedOccurrenceV3>,
    ticket: Option<AdmittedTicketV3>,
}

impl<'a> AuthenticatedSeriesActionV3<'a> {
    /// Selected Series action.
    pub const fn action(self) -> SeriesActionV3 {
        self.request.action()
    }
    /// Exact finalized Template/config.
    pub const fn template(self) -> TemplateV3 {
        self.template
    }
    /// Exact domain-separated Template identity.
    pub const fn template_id(self) -> ContentId {
        self.template_id
    }
    /// Exact occurrence admission, present only on occurrence actions.
    pub const fn occurrence(self) -> Option<AdmittedOccurrenceV3> {
        self.occurrence
    }
    /// Exact Ticket admission, absent only on root Close.
    pub const fn ticket(self) -> Option<AdmittedTicketV3> {
        self.ticket
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
            self.request.expected_series_revision(),
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
            self.request.expected_series_revision(),
            self.request.expected_ticket_revision(),
            now_slot,
            funding,
        )?)
    }

    /// Project the exact post-Found SeriesEscrow-to-Hoard effect.
    pub fn plan_consume_escrow(
        self,
        product: AuthenticatedProductProjectionV2,
        registry_program: AccountKeyV3,
    ) -> Result<SeriesEscrowEffectV3, SeriesProjectorErrorV3> {
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
            self.request.expected_series_revision(),
            self.request.expected_ticket_revision(),
            now_slot,
        )?)
    }

    /// Project the exact post-deadline SeriesEscrow refund effect.
    pub fn plan_expire_escrow(
        self,
        product: AuthenticatedProductProjectionV2,
        registry_program: AccountKeyV3,
    ) -> Result<SeriesEscrowEffectV3, SeriesProjectorErrorV3> {
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
            self.template.occurrence_count(),
            series,
            ticket_state,
            self.required_ticket()?,
            self.request.expected_series_revision(),
            self.request.expected_ticket_revision(),
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
            self.template,
            series,
            self.request.expected_series_revision(),
            observed_root_lamports,
            exact_root_rent,
        )?)
    }

    fn required_occurrence(self) -> Result<AdmittedOccurrenceV3, SeriesProjectorErrorV3> {
        self.occurrence.ok_or(SeriesProjectorErrorV3::Frame)
    }

    fn required_ticket(self) -> Result<AdmittedTicketV3, SeriesProjectorErrorV3> {
        self.ticket.ok_or(SeriesProjectorErrorV3::Frame)
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
    let template = TemplateV3::decode(template_bytes)?;
    let template_id = template_content_id(template_bytes)?;
    if template_id != request.template() {
        return Err(SeriesProjectorErrorV3::Content);
    }
    let (occurrence, ticket) = match request.action() {
        SeriesActionV3::Prepare | SeriesActionV3::Consume | SeriesActionV3::Expire => {
            let occurrence_bytes = occurrence_bytes.ok_or(SeriesProjectorErrorV3::Frame)?;
            let ticket_bytes = ticket_bytes.ok_or(SeriesProjectorErrorV3::Frame)?;
            let occurrence =
                admit_occurrence_bytes(template_bytes, occurrence_bytes, request.proof_bytes())?;
            let ticket = admit_ticket(ticket_bytes)?;
            if request.occurrence() != Some(occurrence.occurrence_id())
                || request.ticket() != Some(ticket.content_id())
            {
                return Err(SeriesProjectorErrorV3::Content);
            }
            occurrence.require_ticket(ticket.ticket())?;
            (Some(occurrence), Some(ticket))
        }
        SeriesActionV3::Retire => {
            if occurrence_bytes.is_some() {
                return Err(SeriesProjectorErrorV3::Frame);
            }
            let ticket = admit_ticket(ticket_bytes.ok_or(SeriesProjectorErrorV3::Frame)?)?;
            if request.ticket() != Some(ticket.content_id())
                || ticket.ticket().template() != template_id
            {
                return Err(SeriesProjectorErrorV3::Content);
            }
            (None, Some(ticket))
        }
        SeriesActionV3::Close => {
            if occurrence_bytes.is_some() || ticket_bytes.is_some() {
                return Err(SeriesProjectorErrorV3::Frame);
            }
            (None, None)
        }
    };
    Ok(AuthenticatedSeriesActionV3 {
        request,
        template,
        template_id,
        occurrence,
        ticket,
    })
}

#[cfg(test)]
mod tests {
    extern crate std;

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
        let effect = accepted
            .plan_consume_escrow(product, AccountKeyV3::new([59; 32]).expect("Registry"))
            .expect("Consume escrow effect");
        assert_eq!(
            effect.kind(),
            crate::series::SeriesEscrowEffectKindV3::ConsumeIntoHoard
        );
        assert_eq!(effect.expected_revision(), 2);
        assert!(effect.hoard_is_destination());
        assert_eq!(
            accepted.plan_prepare_escrow(product, AccountKeyV3::new([59; 32]).expect("Registry")),
            Err(SeriesProjectorErrorV3::Frame)
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
