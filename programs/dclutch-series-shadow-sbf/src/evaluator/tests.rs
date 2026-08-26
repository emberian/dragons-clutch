extern crate std;

use std::vec::Vec;

use dclutch_account_profile_contract::AccountObservationV1;
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::shadow_v3::{
    ShadowArtifactTupleV3, ShadowExecutionDigestsV3, ShadowRequestV3, ShadowRuntimeShapeV3,
};
use dclutch_series_v3_kernel::{
    AuthenticatedProductProjectionV2, OccurrenceV3, generated, occurrence_content_id,
    replay::{SeriesStateV3, TicketStateV3},
    request::{SeriesActionV3, encode_series_action_header_v3},
    template_content_id, ticket_content_id,
};
use sha2::{Digest, Sha256};

use super::*;

const HASH_SEPARATOR: [u8; 1] = [0];

fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("nonzero fixture identity")
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

struct SemanticFixture {
    template: [u8; generated::SERIES_TEMPLATE_BYTES_V3],
    occurrence: [u8; generated::SERIES_OCCURRENCE_BYTES_V3],
    ticket: [u8; generated::SERIES_TICKET_BYTES_V3],
    request: Vec<u8>,
    series_state: [u8; dclutch_series_v3_kernel::replay::SERIES_STATE_BYTES_V3],
    ticket_state: [u8; dclutch_series_v3_kernel::replay::SERIES_TICKET_STATE_BYTES_V3],
    clock: [u8; 40],
    product: AuthenticatedProductProjectionV2,
    market: [u8; 32],
    root_bytes: [u8; 32],
    template_id_bytes: [u8; 32],
    product_record_bytes: [u8; 32],
    registry_program_bytes: [u8; 32],
    trading_program_bytes: [u8; 32],
}

impl SemanticFixture {
    fn new() -> Self {
        let mut template = generated::SERIES_EXAMPLE_TEMPLATE_V3;
        let occurrence = generated::SERIES_EXAMPLE_OCCURRENCE_V3;
        let mut ticket = generated::SERIES_EXAMPLE_TICKET_V3;
        let siblings = [[90_u8; 32], [91_u8; 32]];
        let decoded_occurrence = OccurrenceV3::decode(&occurrence).expect("occurrence");
        let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence identity");
        let root = projection_root(occurrence_id, decoded_occurrence.occurrence(), &siblings);
        put(
            &mut template,
            generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
            &root,
        );
        let template_id = template_content_id(&template).expect("Template identity");
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
        let ticket_id = ticket_content_id(&ticket).expect("Ticket identity");
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
        let decoded_template =
            dclutch_series_v3_kernel::TemplateV3::decode(&template).expect("Template");
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
        let product_record =
            ContentId::new(decoded_occurrence.product_record().to_bytes()).expect("Product record");
        let mut clock = [0_u8; 40];
        put(
            &mut clock,
            0,
            &decoded_occurrence.scheduled_slot().to_le_bytes(),
        );
        Self {
            template,
            occurrence,
            ticket,
            request,
            series_state,
            ticket_state,
            clock,
            product: AuthenticatedProductProjectionV2::new(product_record, id(61), id(62)),
            market,
            root_bytes: id(10).to_bytes(),
            template_id_bytes: template_id.to_bytes(),
            product_record_bytes: product_record.to_bytes(),
            registry_program_bytes: id(59).to_bytes(),
            trading_program_bytes: id(60).to_bytes(),
        }
    }

    fn shadow(&self) -> ShadowRequestV3<'_> {
        ShadowRequestV3 {
            release_set: id(1),
            market: ContentId::new(self.market).expect("Market identity"),
            root: id(10),
            registry_program: id(59),
            trading_program: id(60),
            accelerator_program: id(61),
            artifacts: ShadowArtifactTupleV3 {
                capability_program: id(20),
                account_profile: id(21),
                request_profile: id(22),
                transition: id(23),
                effect: id(24),
                strategy: id(25),
                certificate: id(26),
            },
            invocation_context: id(30),
            digests: ShadowExecutionDigestsV3 {
                runtime_observations: id(31),
                family_request: id(32),
                interpreted_candidate: id(33),
                interpreted_effect: id(34),
            },
            shape: ShadowRuntimeShapeV3 {
                tail_count: 3,
                account_count: 158,
                scalar_count: 5,
                identity_count: 1,
            },
            family_request: &self.request,
        }
    }

    fn observations<'a>(
        &'a self,
        series_state: &'a [u8],
        ticket_state: &'a [u8],
        ticket: &'a [u8],
        root_key: &'a [u8; 32],
    ) -> Vec<AccountObservationV1<'a>> {
        let mut observations =
            vec![
                AccountObservationV1::new(&[200_u8; 32], &[201_u8; 32], 1, &[], false, false, false);
                SERIES_CLOCK_COORDINATE_V4 + 1
            ];
        let mut set = |coordinate, value| {
            *observations
                .get_mut(coordinate)
                .expect("logical coordinate") = value;
        };
        set(
            SERIES_ROOT_COORDINATE_V4,
            AccountObservationV1::new(
                root_key,
                &self.trading_program_bytes,
                1,
                series_state,
                false,
                true,
                false,
            ),
        );
        set(
            SERIES_CONFIG_COORDINATE_V4,
            AccountObservationV1::new(
                &self.template_id_bytes,
                &[201_u8; 32],
                1,
                &self.template,
                false,
                false,
                false,
            ),
        );
        set(
            SERIES_PRODUCT_COORDINATE_V4,
            AccountObservationV1::new(
                &self.product_record_bytes,
                &[201_u8; 32],
                1,
                &[],
                false,
                false,
                false,
            ),
        );
        set(
            SERIES_REGISTRY_COORDINATE_V4,
            AccountObservationV1::new(
                &self.registry_program_bytes,
                &[202_u8; 32],
                1,
                &[],
                false,
                false,
                true,
            ),
        );
        set(
            SERIES_TRADING_COORDINATE_V4,
            AccountObservationV1::new(
                &self.trading_program_bytes,
                &[202_u8; 32],
                1,
                &[],
                false,
                false,
                true,
            ),
        );
        set(
            SERIES_MARKET_COORDINATE_V4,
            AccountObservationV1::new(&self.market, &[203_u8; 32], 1, &[], false, true, false),
        );
        set(
            SERIES_TICKET_STATE_COORDINATE_V4,
            AccountObservationV1::new(
                &[72_u8; 32],
                &self.trading_program_bytes,
                1,
                ticket_state,
                false,
                true,
                false,
            ),
        );
        set(
            SERIES_TEMPLATE_RAW_COORDINATE_V4,
            AccountObservationV1::new(
                &self.template_id_bytes,
                &[201_u8; 32],
                1,
                &self.template,
                false,
                false,
                false,
            ),
        );
        set(
            SERIES_OCCURRENCE_RAW_COORDINATE_V4,
            AccountObservationV1::new(&[73_u8; 32], &[201_u8; 32], 1, &self.occurrence, false, false, false),
        );
        set(
            SERIES_TICKET_RAW_COORDINATE_V4,
            AccountObservationV1::new(&[74_u8; 32], &[201_u8; 32], 1, ticket, false, false, false),
        );
        set(
            SERIES_CLOCK_COORDINATE_V4,
            AccountObservationV1::new(&[75_u8; 32], &[202_u8; 32], 1, &self.clock, false, false, false),
        );
        observations
    }

    fn facts(&self) -> SeriesShadowAuthenticatedFactsV4 {
        SeriesShadowAuthenticatedFactsV4 {
            product: self.product,
            now_slot: u64::from_le_bytes(
                self.clock
                    .get(..8)
                    .expect("slot bytes")
                    .try_into()
                    .expect("slot width"),
            ),
        }
    }
}

#[test]
fn semantic_input_is_derived_from_the_authenticated_runtime_vector() {
    let fixture = SemanticFixture::new();
    let observations = fixture.observations(
        &fixture.series_state,
        &fixture.ticket_state,
        &fixture.ticket,
        &fixture.root_bytes,
    );
    let expected = evaluate_semantic_core_request(fixture.shadow(), &observations, fixture.facts())
        .expect("exact semantic Core request");
    let decoded = dclutch_market_core_codec::SeriesCoreRequestV1::decode(&expected)
        .expect("Core request hostile decode");
    assert_eq!(decoded.expected_series_revision(), 4);
    assert_eq!(decoded.expected_ticket_revision(), 0);
    assert_eq!(
        decoded.market().expect("occurrence Market").to_bytes(),
        fixture.market
    );
}

#[test]
fn replay_root_ticket_product_and_account_substitution_refuse() {
    let fixture = SemanticFixture::new();
    let shadow = fixture.shadow();

    let mut stale_root = fixture.series_state;
    *stale_root.last_mut().expect("root byte") ^= 1;
    let observations = fixture.observations(
        &stale_root,
        &fixture.ticket_state,
        &fixture.ticket,
        &fixture.root_bytes,
    );
    assert_eq!(
        evaluate_semantic_core_request(shadow, &observations, fixture.facts()),
        Err(SeriesShadowAotErrorV4::Semantic)
    );

    let substituted_ticket_state = TicketStateV3::prepared(id(99)).encode();
    let observations = fixture.observations(
        &fixture.series_state,
        &substituted_ticket_state,
        &fixture.ticket,
        &fixture.root_bytes,
    );
    assert_eq!(
        evaluate_semantic_core_request(shadow, &observations, fixture.facts()),
        Err(SeriesShadowAotErrorV4::Semantic)
    );

    let mut substituted_ticket = fixture.ticket;
    *substituted_ticket.last_mut().expect("Ticket byte") ^= 1;
    let observations = fixture.observations(
        &fixture.series_state,
        &fixture.ticket_state,
        &substituted_ticket,
        &fixture.root_bytes,
    );
    assert_eq!(
        evaluate_semantic_core_request(shadow, &observations, fixture.facts()),
        Err(SeriesShadowAotErrorV4::Semantic)
    );

    let observations = fixture.observations(
        &fixture.series_state,
        &fixture.ticket_state,
        &fixture.ticket,
        &fixture.root_bytes,
    );
    let hostile_product = SeriesShadowAuthenticatedFactsV4 {
        product: AuthenticatedProductProjectionV2::new(
            id(98),
            fixture.product.stable_product_id(),
            fixture.product.result_domain(),
        ),
        now_slot: fixture.facts().now_slot,
    };
    assert_eq!(
        evaluate_semantic_core_request(shadow, &observations, hostile_product),
        Err(SeriesShadowAotErrorV4::Runtime)
    );

    let observations = fixture.observations(
        &fixture.series_state,
        &fixture.ticket_state,
        &fixture.ticket,
        &[97_u8; 32],
    );
    assert_eq!(
        evaluate_semantic_core_request(shadow, &observations, fixture.facts()),
        Err(SeriesShadowAotErrorV4::Runtime)
    );
}

#[test]
fn both_core_routes_must_equal_the_kernel_projection() {
    let expected = [0x44_u8; SERIES_CORE_REQUEST_BYTES_V1];
    let mut request_bank = [0_u8; 2 * SERIES_CORE_REQUEST_BYTES_V1];
    request_bank
        .get_mut(..SERIES_CORE_REQUEST_BYTES_V1)
        .expect("Found request")
        .copy_from_slice(&expected);
    request_bank
        .get_mut(SERIES_CORE_REQUEST_BYTES_V1..)
        .expect("Open request")
        .copy_from_slice(&expected);
    let blank = ShadowResolvedRouteV3 {
        role: ShadowRouteRoleV3::Custody,
        kind: ShadowRouteKindV3::Once,
        item: None,
        fixed_account_start: 0,
        fixed_account_count: 1,
        item_account_start: 0,
        item_account_count: 0,
        item_account_stride: 0,
        repeated_item_count: 0,
        request_offset: 0,
        request_len: 1,
        borrowed_witness: None,
        receipt_dependency: None,
        receipt_dependency_count: 0,
        receipt_dependencies_digest: [0; 32],
    };
    let mut routes = [blank; 5];
    *routes
        .get_mut(SERIES_CORE_FOUND_ROUTE_V4)
        .expect("Found route") = ShadowResolvedRouteV3 {
        role: ShadowRouteRoleV3::Core,
        request_offset: 0,
        request_len: u32::try_from(SERIES_CORE_REQUEST_BYTES_V1).expect("Core width"),
        ..blank
    };
    *routes
        .get_mut(SERIES_CORE_OPEN_ROUTE_V4)
        .expect("Open route") = ShadowResolvedRouteV3 {
        role: ShadowRouteRoleV3::Core,
        request_offset: u32::try_from(SERIES_CORE_REQUEST_BYTES_V1).expect("Core offset"),
        request_len: u32::try_from(SERIES_CORE_REQUEST_BYTES_V1).expect("Core width"),
        ..blank
    };
    assert_eq!(
        require_core_request_equivalence(&request_bank, &routes, &expected),
        Ok(())
    );
    *request_bank.last_mut().expect("Open byte") ^= 1;
    assert_eq!(
        require_core_request_equivalence(&request_bank, &routes, &expected),
        Err(SeriesShadowAotErrorV4::Semantic)
    );
}
