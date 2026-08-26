//! Content and replay projection behind the canonical Trading hot outer.
//!
//! The common outer authenticates accounts, finalized-record provenance, the
//! current Trading deployment, and the immutable composite root.  This module
//! then joins the sparse family request to exact Template/Occurrence/Ticket
//! bytes and exposes only action-matched lifecycle planners.

use dclutch_core_contract::ContentId;
use solana_program::pubkey::Pubkey;

use super::{
    AdmittedOccurrenceV2, AdmittedTicketV2, SeriesV2Error, TemplateV2, admit_occurrence,
    admit_ticket,
    instruction::{SeriesActionRequestV2, SeriesActionV2},
    lifecycle::{
        ClosePlanV2, LifecycleErrorV2, OccurrenceCommitPlanV2, PendingFundingPlanV2, RetirePlanV2,
        plan_close, plan_consume, plan_expire, plan_prepare, plan_retire,
    },
    state::{SeriesStateV2, TicketStateV2},
    template_content_id,
};

/// Refusal from the Series hot content/projector boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesProjectorErrorV2 {
    /// Action-specific finalized content accounts were missing or extraneous.
    Frame,
    /// A content hash, Template projection, or Ticket join refused.
    Content,
    /// Schedule, replay, funding, or Core-request planning refused.
    Lifecycle(LifecycleErrorV2),
}

impl From<SeriesV2Error> for SeriesProjectorErrorV2 {
    fn from(_: SeriesV2Error) -> Self {
        Self::Content
    }
}

impl From<LifecycleErrorV2> for SeriesProjectorErrorV2 {
    fn from(value: LifecycleErrorV2) -> Self {
        Self::Lifecycle(value)
    }
}

/// Exact content join selected by one decoded family request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSeriesActionV2<'a> {
    request: SeriesActionRequestV2<'a>,
    template: TemplateV2,
    template_id: ContentId,
    occurrence: Option<AdmittedOccurrenceV2>,
    ticket: Option<AdmittedTicketV2>,
}

impl<'a> AuthenticatedSeriesActionV2<'a> {
    /// Selected Series action.
    pub const fn action(self) -> SeriesActionV2 {
        self.request.action()
    }
    /// Exact finalized Template/config.
    pub const fn template(self) -> TemplateV2 {
        self.template
    }
    /// Exact domain-separated Template identity.
    pub const fn template_id(self) -> ContentId {
        self.template_id
    }
    /// Exact occurrence admission, present only on occurrence actions.
    pub const fn occurrence(self) -> Option<AdmittedOccurrenceV2> {
        self.occurrence
    }
    /// Exact Ticket admission, absent only on root Close.
    pub const fn ticket(self) -> Option<AdmittedTicketV2> {
        self.ticket
    }

    /// Plan one dust-tolerant replay-account preparation.
    pub fn plan_prepare(
        self,
        series: SeriesStateV2,
        now_slot: u64,
        current_ticket_lamports: u64,
        ticket_state_rent: u64,
    ) -> Result<(OccurrenceCommitPlanV2, u64, u64), SeriesProjectorErrorV2> {
        if self.action() != SeriesActionV2::Prepare {
            return Err(SeriesProjectorErrorV2::Frame);
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

    /// Plan one atomic Ticket-to-Found consumption.
    #[allow(clippy::too_many_arguments)]
    pub fn plan_consume(
        self,
        ticket_state_key: Pubkey,
        series: SeriesStateV2,
        ticket_state: TicketStateV2,
        now_slot: u64,
        funding: PendingFundingPlanV2,
    ) -> Result<OccurrenceCommitPlanV2, SeriesProjectorErrorV2> {
        if self.action() != SeriesActionV2::Consume {
            return Err(SeriesProjectorErrorV2::Frame);
        }
        Ok(plan_consume(
            self.required_occurrence()?,
            self.required_ticket()?,
            ticket_state_key,
            series,
            ticket_state,
            self.request.expected_series_revision(),
            self.request.expected_ticket_revision(),
            now_slot,
            funding,
        )?)
    }

    /// Plan one exact expiry after the retry deadline.
    pub fn plan_expire(
        self,
        ticket_state_key: Pubkey,
        series: SeriesStateV2,
        ticket_state: TicketStateV2,
        now_slot: u64,
    ) -> Result<OccurrenceCommitPlanV2, SeriesProjectorErrorV2> {
        if self.action() != SeriesActionV2::Expire {
            return Err(SeriesProjectorErrorV2::Frame);
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

    /// Plan deletion of one terminal replay account.
    pub fn plan_retire(
        self,
        series: SeriesStateV2,
        ticket_state: TicketStateV2,
        observed_ticket_lamports: u64,
    ) -> Result<RetirePlanV2, SeriesProjectorErrorV2> {
        if self.action() != SeriesActionV2::Retire {
            return Err(SeriesProjectorErrorV2::Frame);
        }
        Ok(plan_retire(
            series,
            ticket_state,
            self.required_ticket()?,
            self.request.expected_series_revision(),
            observed_ticket_lamports,
        )?)
    }

    /// Plan terminal root close without fabricating a Market authority.
    pub fn plan_close(
        self,
        series: SeriesStateV2,
        observed_root_lamports: u64,
        exact_root_rent: u64,
    ) -> Result<ClosePlanV2, SeriesProjectorErrorV2> {
        if self.action() != SeriesActionV2::Close {
            return Err(SeriesProjectorErrorV2::Frame);
        }
        Ok(plan_close(
            self.template,
            series,
            self.request.expected_series_revision(),
            observed_root_lamports,
            exact_root_rent,
        )?)
    }

    fn required_occurrence(self) -> Result<AdmittedOccurrenceV2, SeriesProjectorErrorV2> {
        self.occurrence.ok_or(SeriesProjectorErrorV2::Frame)
    }

    fn required_ticket(self) -> Result<AdmittedTicketV2, SeriesProjectorErrorV2> {
        self.ticket.ok_or(SeriesProjectorErrorV2::Frame)
    }
}

/// Join one sparse request to its exact finalized semantic records.
///
/// Finalized-record owner/PDA/cursor/Rent authentication is performed by the
/// common outer before it passes these borrowed bytes.  Extraneous accounts
/// are refused just as strongly as missing ones so terminal actions cannot
/// smuggle an occurrence proof or substitute an unrelated Ticket.
pub fn authenticate_action_content_v2<'a>(
    request: SeriesActionRequestV2<'a>,
    template_bytes: &[u8],
    occurrence_bytes: Option<&[u8]>,
    ticket_bytes: Option<&[u8]>,
) -> Result<AuthenticatedSeriesActionV2<'a>, SeriesProjectorErrorV2> {
    let template = TemplateV2::decode(template_bytes)?;
    let template_id = template_content_id(template_bytes)?;
    if template_id != request.template() {
        return Err(SeriesProjectorErrorV2::Content);
    }
    let (occurrence, ticket) = match request.action() {
        SeriesActionV2::Prepare | SeriesActionV2::Consume | SeriesActionV2::Expire => {
            let occurrence_bytes = occurrence_bytes.ok_or(SeriesProjectorErrorV2::Frame)?;
            let ticket_bytes = ticket_bytes.ok_or(SeriesProjectorErrorV2::Frame)?;
            let mut siblings = [[0_u8; 32]; 32];
            let proof = request
                .copy_proof_into(&mut siblings)
                .map_err(|_| SeriesProjectorErrorV2::Content)?;
            let occurrence = admit_occurrence(template_bytes, occurrence_bytes, proof)?;
            let ticket = admit_ticket(ticket_bytes)?;
            if request.occurrence() != Some(occurrence.occurrence_id())
                || request.ticket() != Some(ticket.content_id())
            {
                return Err(SeriesProjectorErrorV2::Content);
            }
            occurrence.require_ticket(ticket.ticket())?;
            (Some(occurrence), Some(ticket))
        }
        SeriesActionV2::Retire => {
            if occurrence_bytes.is_some() {
                return Err(SeriesProjectorErrorV2::Frame);
            }
            let ticket = admit_ticket(ticket_bytes.ok_or(SeriesProjectorErrorV2::Frame)?)?;
            if request.ticket() != Some(ticket.content_id())
                || ticket.ticket().template() != template_id
            {
                return Err(SeriesProjectorErrorV2::Content);
            }
            (None, Some(ticket))
        }
        SeriesActionV2::Close => {
            if occurrence_bytes.is_some() || ticket_bytes.is_some() {
                return Err(SeriesProjectorErrorV2::Frame);
            }
            (None, None)
        }
    };
    Ok(AuthenticatedSeriesActionV2 {
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
        generated,
        instruction::{SeriesActionV2, encode_series_action_header_v2},
        occurrence_content_id,
    };

    fn put(output: &mut [u8], offset: usize, value: &[u8]) {
        output
            .get_mut(offset..offset + value.len())
            .expect("fixture field")
            .copy_from_slice(value);
    }

    fn occurrence_fixture() -> (Vec<u8>, [u8; 400], [u8; 320], [u8; 256]) {
        let mut occurrence = generated::SERIES_EXAMPLE_OCCURRENCE_V2;
        put(
            &mut occurrence,
            generated::SERIES_OCCURRENCE_INDEX_OFFSET_V2,
            &0_u32.to_le_bytes(),
        );
        put(
            &mut occurrence,
            generated::SERIES_OCCURRENCE_SCHEDULED_SLOT_OFFSET_V2,
            &100_u64.to_le_bytes(),
        );
        let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence ID");

        let mut template = generated::SERIES_EXAMPLE_TEMPLATE_V2;
        put(
            &mut template,
            generated::SERIES_TEMPLATE_OCCURRENCE_COUNT_OFFSET_V2,
            &1_u32.to_le_bytes(),
        );
        put(
            &mut template,
            generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V2,
            &occurrence_id.to_bytes(),
        );
        let template_id = template_content_id(&template).expect("Template ID");

        let mut ticket = generated::SERIES_EXAMPLE_TICKET_V2;
        put(
            &mut ticket,
            generated::SERIES_TICKET_INDEX_OFFSET_V2,
            &0_u32.to_le_bytes(),
        );
        put(
            &mut ticket,
            generated::SERIES_TICKET_TEMPLATE_OFFSET_V2,
            &template_id.to_bytes(),
        );
        put(
            &mut ticket,
            generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V2,
            &occurrence_id.to_bytes(),
        );
        let ticket_id = admit_ticket(&ticket).expect("Ticket ID").content_id();
        let header = encode_series_action_header_v2(
            SeriesActionV2::Consume,
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
        let decoded = SeriesActionRequestV2::decode(&request).expect("decode");
        let accepted =
            authenticate_action_content_v2(decoded, &template, Some(&occurrence), Some(&ticket))
                .expect("content join");
        assert_eq!(accepted.action(), SeriesActionV2::Consume);
        assert_eq!(
            accepted.occurrence().expect("occurrence").template_id(),
            accepted.template_id()
        );

        let mut substituted = request;
        *substituted.get_mut(48).expect("occurrence identity") ^= 1;
        let substituted = SeriesActionRequestV2::decode(&substituted).expect("hostile decodes");
        assert_eq!(
            authenticate_action_content_v2(
                substituted,
                &template,
                Some(&occurrence),
                Some(&ticket),
            ),
            Err(SeriesProjectorErrorV2::Content)
        );
    }

    #[test]
    fn close_refuses_extraneous_occurrence_or_ticket_content() {
        let (_, template, occurrence, ticket) = occurrence_fixture();
        let template_id = template_content_id(&template).expect("Template ID");
        let close =
            encode_series_action_header_v2(SeriesActionV2::Close, template_id, None, None, 8, 0, 0)
                .expect("close");
        let close = SeriesActionRequestV2::decode(&close).expect("close decode");
        assert!(authenticate_action_content_v2(close, &template, None, None).is_ok());
        assert_eq!(
            authenticate_action_content_v2(close, &template, Some(&occurrence), Some(&ticket)),
            Err(SeriesProjectorErrorV2::Frame)
        );
    }
}
