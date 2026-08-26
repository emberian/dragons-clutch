//! Stateless semantic plan for Shadow-AOT and differential execution.
//!
//! The caller first authenticates physical Registry, Product, Clock, and
//! account observations. This evaluator then owns only the immutable Series
//! join and candidate semantics. It cannot access accounts, invoke a program,
//! or commit a write.

use dclutch_market_core_codec::SeriesCoreRequestV1;

use crate::{
    AccountKeyV3, AuthenticatedProductProjectionV2, PrefoundingSeriesEscrowV3,
    plan::{SeriesReplayActionV3, SeriesReplayWitnessV3, evaluate_replay_v3},
    pre_founding_series_escrow,
    replay::SeriesStateV3,
    request::{AdmittedSeriesActionV3, SeriesActionV3, admit_series_action_v3},
    series_core_consume_request,
};

/// Stable refusal from stateless Series Shadow evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesShadowErrorV3 {
    /// Family request or immutable record admission refused.
    Content,
    /// Required typed Product, Registry, or Ticket-account fact was absent.
    Observation,
    /// Mutable replay prestate or optimistic revision refused.
    Replay,
    /// Current Clock slot did not admit the selected scheduled action.
    Schedule,
    /// Core or SeriesEscrow projection refused.
    Effect,
}

/// Result alias for stateless Series Shadow evaluation.
pub type Result<T> = core::result::Result<T, SeriesShadowErrorV3>;

/// Typed observations supplied only after physical authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesShadowObservationsV3 {
    /// Independently authenticated Product Runtime V2 projection.
    pub product: Option<AuthenticatedProductProjectionV2>,
    /// Current Registry program identity.
    pub registry_program: Option<AccountKeyV3>,
    /// Exact Trading-owned mutable Ticket-state account coordinate.
    pub ticket_state_account: Option<AccountKeyV3>,
    /// Current Clock slot.
    pub now_slot: u64,
}

/// Borrowed immutable bodies and mutable replay prestates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesShadowInputV3<'a> {
    /// Complete exact family request.
    pub family_request: &'a [u8],
    /// Finalized Template body.
    pub template: &'a [u8],
    /// Finalized occurrence body for occurrence-bound actions.
    pub occurrence: Option<&'a [u8]>,
    /// Finalized Ticket body for every action except Close.
    pub ticket: Option<&'a [u8]>,
    /// Exact current Trading-owned Series-root state.
    pub series_state: &'a [u8],
    /// Exact current Ticket replay state except for Prepare and Close.
    pub ticket_state: Option<&'a [u8]>,
    /// Typed physical observations.
    pub observations: SeriesShadowObservationsV3,
}

/// Semantic child-effect profile reproduced by the generic interpreter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesShadowEffectKindV3 {
    /// Retire and Close have no child-role effect.
    None,
    /// Initialize replay, open SeriesEscrow, and lock collateral.
    Prepare,
    /// Atomically credit projected Hoard and close SeriesEscrow, then found.
    Consume,
    /// Refund collateral, then close escrow resources.
    Expire,
}

/// Compact semantic child-effect projection.
///
/// The canonical three-edge escrow sequence is derived from `kind` and
/// `escrow` by the sole Series kernel constructors. It is not copied three
/// times into this plan value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesShadowEffectsV3 {
    kind: SeriesShadowEffectKindV3,
    core: Option<SeriesCoreRequestV1>,
    escrow: Option<PrefoundingSeriesEscrowV3>,
}

impl SeriesShadowEffectsV3 {
    /// Selected semantic effect profile.
    pub const fn kind(self) -> SeriesShadowEffectKindV3 {
        self.kind
    }

    /// Exact Core request, present only for Consume.
    pub const fn core(self) -> Option<SeriesCoreRequestV1> {
        self.core
    }

    /// Exact pre-founding SeriesEscrow projection for occurrence actions.
    pub const fn escrow(self) -> Option<PrefoundingSeriesEscrowV3> {
        self.escrow
    }
}

/// Complete stateless semantic result for one Series action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesShadowPlanV3<'a> {
    admitted: AdmittedSeriesActionV3<'a>,
    replay: SeriesReplayWitnessV3,
    effects: SeriesShadowEffectsV3,
}

impl<'a> SeriesShadowPlanV3<'a> {
    /// Exact immutable action admission.
    pub const fn admitted(self) -> AdmittedSeriesActionV3<'a> {
        self.admitted
    }

    /// Candidate root/Ticket replay transition.
    pub const fn replay(self) -> SeriesReplayWitnessV3 {
        self.replay
    }

    /// Complete semantic child-effect projection.
    pub const fn effects(self) -> SeriesShadowEffectsV3 {
        self.effects
    }
}

/// Evaluate one complete Series action without state or CPI authority.
pub fn evaluate_series_shadow_v3(input: SeriesShadowInputV3<'_>) -> Result<SeriesShadowPlanV3<'_>> {
    let admitted = admit_series_action_v3(
        input.family_request,
        input.template,
        input.occurrence,
        input.ticket,
    )
    .map_err(|_| SeriesShadowErrorV3::Content)?;
    let request = admitted.request();
    let template = admitted.template();
    let series = SeriesStateV3::decode(input.series_state, template.occurrence_count())
        .map_err(|_| SeriesShadowErrorV3::Replay)?;
    let ticket_record = admitted.ticket().map(|ticket| ticket.content_id());
    let replay_action = match request.action() {
        SeriesActionV3::Prepare => SeriesReplayActionV3::Prepare {
            ticket_record: ticket_record.ok_or(SeriesShadowErrorV3::Content)?,
        },
        SeriesActionV3::Consume => SeriesReplayActionV3::Consume {
            ticket_record: ticket_record.ok_or(SeriesShadowErrorV3::Content)?,
            expected_ticket_revision: request.expected_ticket_revision(),
        },
        SeriesActionV3::Expire => SeriesReplayActionV3::Expire {
            ticket_record: ticket_record.ok_or(SeriesShadowErrorV3::Content)?,
            expected_ticket_revision: request.expected_ticket_revision(),
        },
        SeriesActionV3::Retire => SeriesReplayActionV3::Retire {
            ticket_record: ticket_record.ok_or(SeriesShadowErrorV3::Content)?,
            expected_ticket_revision: request.expected_ticket_revision(),
        },
        SeriesActionV3::Close => SeriesReplayActionV3::Close,
    };
    let replay = evaluate_replay_v3(
        replay_action,
        template.occurrence_count(),
        request.expected_series_revision(),
        input.series_state,
        input.ticket_state,
    )
    .map_err(|_| SeriesShadowErrorV3::Replay)?;
    let effects = occurrence_effects(input.observations, admitted, series)?;
    Ok(SeriesShadowPlanV3 {
        admitted,
        replay,
        effects,
    })
}

fn occurrence_effects(
    observations: SeriesShadowObservationsV3,
    admitted: AdmittedSeriesActionV3<'_>,
    series: SeriesStateV3,
) -> Result<SeriesShadowEffectsV3> {
    let request = admitted.request();
    if matches!(
        request.action(),
        SeriesActionV3::Retire | SeriesActionV3::Close
    ) {
        return Ok(SeriesShadowEffectsV3 {
            kind: SeriesShadowEffectKindV3::None,
            core: None,
            escrow: None,
        });
    }
    let occurrence = admitted
        .required_occurrence()
        .map_err(|_| SeriesShadowErrorV3::Content)?;
    let ticket = admitted
        .required_ticket()
        .map_err(|_| SeriesShadowErrorV3::Content)?;
    if series.next_occurrence() != occurrence.occurrence().occurrence() {
        return Err(SeriesShadowErrorV3::Replay);
    }
    let scheduled = occurrence.occurrence().scheduled_slot();
    let retry_through = admitted
        .template()
        .retry_through(occurrence.occurrence().occurrence())
        .map_err(|_| SeriesShadowErrorV3::Schedule)?;
    match request.action() {
        SeriesActionV3::Prepare if observations.now_slot > retry_through => {
            return Err(SeriesShadowErrorV3::Schedule);
        }
        SeriesActionV3::Consume
            if observations.now_slot < scheduled || observations.now_slot > retry_through =>
        {
            return Err(SeriesShadowErrorV3::Schedule);
        }
        SeriesActionV3::Expire if observations.now_slot <= retry_through => {
            return Err(SeriesShadowErrorV3::Schedule);
        }
        SeriesActionV3::Prepare | SeriesActionV3::Consume | SeriesActionV3::Expire => {}
        SeriesActionV3::Retire | SeriesActionV3::Close => {
            return Err(SeriesShadowErrorV3::Content);
        }
    }
    let product = observations
        .product
        .ok_or(SeriesShadowErrorV3::Observation)?;
    let registry_program = observations
        .registry_program
        .ok_or(SeriesShadowErrorV3::Observation)?;
    let escrow = pre_founding_series_escrow(occurrence, ticket, product, registry_program)
        .map_err(|_| SeriesShadowErrorV3::Effect)?;
    match request.action() {
        SeriesActionV3::Prepare => Ok(SeriesShadowEffectsV3 {
            kind: SeriesShadowEffectKindV3::Prepare,
            core: None,
            escrow: Some(escrow),
        }),
        SeriesActionV3::Consume => {
            let ticket_state_account = observations
                .ticket_state_account
                .ok_or(SeriesShadowErrorV3::Observation)?;
            let core = series_core_consume_request(
                occurrence,
                ticket,
                product,
                ticket_state_account,
                request.expected_series_revision(),
                request.expected_ticket_revision(),
            )
            .map_err(|_| SeriesShadowErrorV3::Effect)?;
            Ok(SeriesShadowEffectsV3 {
                kind: SeriesShadowEffectKindV3::Consume,
                core: Some(core),
                escrow: Some(escrow),
            })
        }
        SeriesActionV3::Expire => Ok(SeriesShadowEffectsV3 {
            kind: SeriesShadowEffectKindV3::Expire,
            core: None,
            escrow: Some(escrow),
        }),
        SeriesActionV3::Retire | SeriesActionV3::Close => Err(SeriesShadowErrorV3::Content),
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use dclutch_core_contract::ContentId;
    use sha2::{Digest, Sha256};
    use std::vec::Vec;

    use super::*;
    use crate::{
        OccurrenceV3, generated, occurrence_content_id, plan::ReplayCandidateV3,
        replay::TicketStateV3, request::encode_series_action_header_v3, template_content_id,
        ticket_content_id,
    };

    const HASH_SEPARATOR: [u8; 1] = [0];

    fn key(byte: u8) -> AccountKeyV3 {
        AccountKeyV3::new([byte; 32]).expect("nonzero account key")
    }

    fn put<const N: usize>(target: &mut [u8], offset: usize, value: &[u8; N]) {
        target
            .get_mut(offset..offset + N)
            .expect("fixture field")
            .copy_from_slice(value);
    }

    fn projection_root(
        occurrence_id: ContentId,
        mut occurrence: u32,
        siblings: &[[u8; 32]],
    ) -> [u8; 32] {
        let mut node = occurrence_id.to_bytes();
        for sibling in siblings {
            let mut hasher = Sha256::new();
            hasher.update(generated::SERIES_PROJECTION_NODE_DOMAIN_V3);
            hasher.update(HASH_SEPARATOR);
            if occurrence & 1 == 0 {
                hasher.update(node);
                hasher.update(sibling);
            } else {
                hasher.update(sibling);
                hasher.update(node);
            }
            node = hasher.finalize().into();
            occurrence >>= 1;
        }
        node
    }

    struct Fixture {
        template: [u8; generated::SERIES_TEMPLATE_BYTES_V3],
        occurrence: [u8; generated::SERIES_OCCURRENCE_BYTES_V3],
        ticket: [u8; generated::SERIES_TICKET_BYTES_V3],
        request: Vec<u8>,
        series_state: [u8; crate::replay::SERIES_STATE_BYTES_V3],
        ticket_state: [u8; crate::replay::SERIES_TICKET_STATE_BYTES_V3],
        product: AuthenticatedProductProjectionV2,
        scheduled_slot: u64,
        template_id: ContentId,
        occurrence_id: ContentId,
        ticket_id: ContentId,
    }

    impl Fixture {
        fn new() -> Self {
            let mut template = generated::SERIES_EXAMPLE_TEMPLATE_V3;
            let occurrence = generated::SERIES_EXAMPLE_OCCURRENCE_V3;
            let mut ticket = generated::SERIES_EXAMPLE_TICKET_V3;
            let siblings = [[90_u8; 32], [91_u8; 32]];
            let decoded_occurrence = OccurrenceV3::decode(&occurrence).expect("occurrence");
            assert_eq!(decoded_occurrence.occurrence(), 1);
            let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence ID");
            let root = projection_root(occurrence_id, decoded_occurrence.occurrence(), &siblings);
            put(
                &mut template,
                generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
                &root,
            );
            let template_id = template_content_id(&template).expect("Template ID");
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
            let market = decoded_occurrence.market().to_bytes();
            put(
                &mut ticket,
                generated::SERIES_TICKET_MARKET_OFFSET_V3,
                &market,
            );
            let ticket_id = ticket_content_id(&ticket).expect("Ticket ID");
            let header = encode_series_action_header_v3(
                SeriesActionV3::Consume,
                template_id,
                Some(occurrence_id),
                Some(ticket_id),
                4,
                0,
                2,
            )
            .expect("request header");
            let mut request = Vec::from(header);
            request.extend_from_slice(&siblings[0]);
            request.extend_from_slice(&siblings[1]);
            let decoded_template = crate::TemplateV3::decode(&template).expect("Template");
            let occurrence_count = decoded_template.occurrence_count();
            let series_state = SeriesStateV3::new(decoded_template.close_rent())
                .prepare_ticket(0)
                .expect("prepare occurrence zero")
                .settle_current(1, occurrence_count)
                .expect("settle occurrence zero")
                .retire_ticket(2)
                .expect("retire occurrence zero")
                .prepare_ticket(3)
                .expect("prepare occurrence one")
                .encode(occurrence_count)
                .expect("Series state");
            let ticket_state = TicketStateV3::prepared(ticket_id).encode();
            let product_record = ContentId::new(decoded_occurrence.product_record().to_bytes())
                .expect("Product record");
            Self {
                template,
                occurrence,
                ticket,
                request,
                series_state,
                ticket_state,
                product: AuthenticatedProductProjectionV2::new(
                    product_record,
                    ContentId::new([61; 32]).expect("stable Product ID"),
                    ContentId::new([62; 32]).expect("result domain"),
                ),
                scheduled_slot: decoded_occurrence.scheduled_slot(),
                template_id,
                occurrence_id,
                ticket_id,
            }
        }

        fn input(&self) -> SeriesShadowInputV3<'_> {
            SeriesShadowInputV3 {
                family_request: &self.request,
                template: &self.template,
                occurrence: Some(&self.occurrence),
                ticket: Some(&self.ticket),
                series_state: &self.series_state,
                ticket_state: Some(&self.ticket_state),
                observations: SeriesShadowObservationsV3 {
                    product: Some(self.product),
                    registry_program: Some(key(59)),
                    ticket_state_account: Some(key(72)),
                    now_slot: self.scheduled_slot,
                },
            }
        }
    }

    #[test]
    fn consume_shadow_joins_content_replay_schedule_and_effects() {
        let fixture = Fixture::new();
        let plan = evaluate_series_shadow_v3(fixture.input()).expect("Shadow plan");
        assert_eq!(plan.admitted().request().action(), SeriesActionV3::Consume);
        assert!(matches!(
            plan.replay().series(),
            ReplayCandidateV3::Replace(_)
        ));
        assert!(matches!(
            plan.replay().ticket(),
            ReplayCandidateV3::Replace(_)
        ));
        assert_eq!(plan.effects().kind(), SeriesShadowEffectKindV3::Consume);
        let escrow = plan.effects().escrow().expect("escrow projection");
        let core = plan.effects().core().expect("Core request");
        assert_eq!(core.hoard_principal(), escrow.hoard_principal());
        assert_eq!(
            core.market().expect("Core Market").to_bytes(),
            escrow.market().to_bytes()
        );
    }

    #[test]
    fn shadow_refuses_substitution_stale_replay_and_missing_observations() {
        let fixture = Fixture::new();

        let mut substituted_request = fixture.request.clone();
        *substituted_request.last_mut().expect("proof byte") ^= 1;
        let mut input = fixture.input();
        input.family_request = &substituted_request;
        assert_eq!(
            evaluate_series_shadow_v3(input),
            Err(SeriesShadowErrorV3::Content)
        );

        let stale_header = encode_series_action_header_v3(
            SeriesActionV3::Consume,
            fixture.template_id,
            Some(fixture.occurrence_id),
            Some(fixture.ticket_id),
            4,
            1,
            2,
        )
        .expect("stale request header");
        let mut stale_request = Vec::from(stale_header);
        stale_request.extend_from_slice(fixture.request.get(128..).expect("proof"));
        let mut stale_replay = fixture.input();
        stale_replay.family_request = &stale_request;
        assert_eq!(
            evaluate_series_shadow_v3(stale_replay),
            Err(SeriesShadowErrorV3::Replay)
        );

        let mut missing_product = fixture.input();
        missing_product.observations.product = None;
        assert_eq!(
            evaluate_series_shadow_v3(missing_product),
            Err(SeriesShadowErrorV3::Observation)
        );

        let mut early = fixture.input();
        early.observations.now_slot = fixture.scheduled_slot - 1;
        assert_eq!(
            evaluate_series_shadow_v3(early),
            Err(SeriesShadowErrorV3::Schedule)
        );
    }

    #[test]
    fn product_record_substitution_refuses_before_effect_projection() {
        let fixture = Fixture::new();
        let mut input = fixture.input();
        input.observations.product = Some(AuthenticatedProductProjectionV2::new(
            ContentId::new([99; 32]).expect("substituted Product record"),
            fixture.product.stable_product_id(),
            fixture.product.result_domain(),
        ));
        assert_eq!(
            evaluate_series_shadow_v3(input),
            Err(SeriesShadowErrorV3::Effect)
        );
    }

    #[test]
    fn fixture_is_canonical_and_not_a_parallel_wire() {
        let fixture = Fixture::new();
        assert_eq!(
            crate::TemplateV3::decode(&fixture.template)
                .expect("Template")
                .occurrence_count(),
            3
        );
        assert_eq!(
            crate::admit_ticket(&fixture.ticket)
                .expect("Ticket")
                .ticket()
                .occurrence(),
            1
        );
        let projection = crate::future_market_projection(
            crate::admit_occurrence_bytes(
                &fixture.template,
                &fixture.occurrence,
                fixture.request.get(128..).expect("proof"),
            )
            .expect("occurrence"),
            fixture.product,
            key(59),
        )
        .expect("future Market");
        assert_eq!(
            projection.committed_address(),
            OccurrenceV3::decode(&fixture.occurrence)
                .expect("occurrence")
                .market()
        );
    }
}
