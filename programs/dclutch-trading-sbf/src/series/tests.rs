use super::*;
use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{MarketCoreStateSeedsV2, MarketIdentity, SeriesCoreActionV1};
use solana_program::{hash::hashv, pubkey::Pubkey};

const HASH_SEPARATOR: [u8; 1] = [0];

fn key(tag: u8) -> Pubkey {
    let mut bytes = [0_u8; 32];
    *bytes.first_mut().expect("fixed-width key") = tag;
    Pubkey::new_from_array(bytes)
}

fn put<const N: usize>(bytes: &mut [u8], offset: usize, value: &[u8; N]) {
    bytes
        .get_mut(offset..offset.checked_add(N).expect("fixture offset"))
        .expect("fixture field")
        .copy_from_slice(value);
}

fn projection_root(occurrence_id: ContentId, index: u32, siblings: &[[u8; 32]]) -> [u8; 32] {
    let mut node = occurrence_id.to_bytes();
    let mut cursor = index;
    for sibling in siblings {
        node = if cursor & 1 == 0 {
            hashv(&[
                &generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
                &HASH_SEPARATOR,
                &node,
                sibling,
            ])
            .to_bytes()
        } else {
            hashv(&[
                &generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
                &HASH_SEPARATOR,
                sibling,
                &node,
            ])
            .to_bytes()
        };
        cursor >>= 1;
    }
    node
}

#[derive(Clone)]
struct Fixture {
    template: [u8; SERIES_TEMPLATE_BYTES_V3],
    occurrence: [u8; SERIES_OCCURRENCE_BYTES_V3],
    ticket: [u8; SERIES_TICKET_BYTES_V3],
    siblings: [[u8; 32]; 2],
    funding: [Pubkey; 2],
    core_program: Pubkey,
    registry_program: Pubkey,
    product: AuthenticatedProductProjectionV2,
}

impl Fixture {
    fn new() -> Self {
        let mut template = generated::SERIES_EXAMPLE_TEMPLATE_V3;
        let mut occurrence = generated::SERIES_EXAMPLE_OCCURRENCE_V3;
        let mut ticket = generated::SERIES_EXAMPLE_TICKET_V3;
        let siblings = [key(90).to_bytes(), key(91).to_bytes()];
        let funding = [key(70), key(71)];
        let funding_id = funding_list_id(&funding).expect("funding list");
        put(
            &mut occurrence,
            generated::SERIES_OCCURRENCE_FUNDING_LIST_OFFSET_V3,
            &funding_id.to_bytes(),
        );

        let core_program = key(60);
        let registry_program = key(59);
        let occurrence_value = OccurrenceV3::decode(&occurrence).expect("occurrence before market");
        let template_value = TemplateV3::decode(&template).expect("template before root");
        let product = AuthenticatedProductProjectionV2::new(
            occurrence_value.product_record(),
            ContentId::new([57; 32]).expect("stable Product"),
            ContentId::new([58; 32]).expect("result domain"),
        );
        let identity = MarketIdentity {
            market_id: core_pubkey_identity(key(61)).expect("temporary market"),
            realm_id: core_identity(template_value.realm()).expect("realm"),
            product_record: core_identity(product.product_record()).expect("Product record"),
            product_id: core_identity(product.stable_product_id()).expect("stable Product"),
            resolution_policy: core_identity(occurrence_value.resolution_policy())
                .expect("resolution"),
            capability_manifest: core_identity(occurrence_value.capability_manifest())
                .expect("manifest"),
            selected_release_set: core_identity(template_value.release_set()).expect("release"),
            registry_program: core_pubkey_identity(registry_program).expect("Registry"),
            generation: u64::from(occurrence_value.occurrence()) + 1,
        };
        let seeds = MarketCoreStateSeedsV2::new(identity);
        let market = Pubkey::find_program_address(&seeds.as_slices(), &core_program).0;
        put(
            &mut occurrence,
            generated::SERIES_OCCURRENCE_MARKET_OFFSET_V3,
            &market.to_bytes(),
        );

        let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence id");
        let root = projection_root(occurrence_id, 1, &siblings);
        put(
            &mut template,
            generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
            &root,
        );
        let template_id = template_content_id(&template).expect("template id");
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
        put(
            &mut ticket,
            generated::SERIES_TICKET_MARKET_OFFSET_V3,
            &market.to_bytes(),
        );
        put(
            &mut ticket,
            generated::SERIES_TICKET_FUNDING_LIST_OFFSET_V3,
            &funding_id.to_bytes(),
        );
        Self {
            template,
            occurrence,
            ticket,
            siblings,
            funding,
            core_program,
            registry_program,
            product,
        }
    }

    fn recommit_occurrence(&mut self) {
        let occurrence_id = occurrence_content_id(&self.occurrence).expect("occurrence id");
        let occurrence = OccurrenceV3::decode(&self.occurrence).expect("occurrence");
        let root = projection_root(occurrence_id, occurrence.occurrence(), &self.siblings);
        put(
            &mut self.template,
            generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
            &root,
        );
        let template_id = template_content_id(&self.template).expect("template id");
        put(
            &mut self.ticket,
            generated::SERIES_TICKET_TEMPLATE_OFFSET_V3,
            &template_id.to_bytes(),
        );
        put(
            &mut self.ticket,
            generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
            &occurrence_id.to_bytes(),
        );
        put(
            &mut self.ticket,
            generated::SERIES_TICKET_MARKET_OFFSET_V3,
            &occurrence.market().to_bytes(),
        );
    }

    fn admit(&self) -> AdmittedOccurrenceV3 {
        admit_occurrence(&self.template, &self.occurrence, &self.siblings)
            .expect("admitted occurrence")
    }
}

#[test]
fn lean_vectors_hostile_decode_and_exact_admission() {
    assert!(TemplateV3::decode(&generated::SERIES_EXAMPLE_TEMPLATE_V3).is_ok());
    assert!(OccurrenceV3::decode(&generated::SERIES_EXAMPLE_OCCURRENCE_V3).is_ok());
    assert!(TicketV3::decode(&generated::SERIES_EXAMPLE_TICKET_V3).is_ok());

    let fixture = Fixture::new();
    let admitted = fixture.admit();
    let ticket = TicketV3::decode(&fixture.ticket).expect("ticket");
    assert_eq!(
        admitted.occurrence().liability_basis().to_bytes().first(),
        Some(&16)
    );
    assert_eq!(
        admitted
            .occurrence()
            .rational_representation()
            .to_bytes()
            .first(),
        Some(&17)
    );
    admitted.require_ticket(ticket).expect("exact ticket");
    require_funding_list(admitted.occurrence(), &fixture.funding).expect("exact funding");
    require_market_pda(
        admitted,
        fixture.product,
        &fixture.core_program,
        &fixture.registry_program,
    )
    .expect("exact Market");
}

#[test]
fn header_width_reserved_and_zero_identity_refuse() {
    let fixture = Fixture::new();
    assert_eq!(
        TemplateV3::decode(
            fixture
                .template
                .get(..fixture.template.len() - 1)
                .expect("short Template")
        ),
        Err(SeriesV3Error::Length)
    );
    let mut template = fixture.template;
    *template.first_mut().expect("Template magic") ^= 1;
    assert_eq!(TemplateV3::decode(&template), Err(SeriesV3Error::Header));
    let mut occurrence = fixture.occurrence;
    *occurrence
        .get_mut(generated::SERIES_OCCURRENCE_RESERVED_OFFSET_V3)
        .expect("reserved byte") = 1;
    assert_eq!(
        OccurrenceV3::decode(&occurrence),
        Err(SeriesV3Error::Header)
    );
    let mut ticket = fixture.ticket;
    ticket[generated::SERIES_TICKET_FOUNDER_OFFSET_V3
        ..generated::SERIES_TICKET_FOUNDER_OFFSET_V3 + 32]
        .fill(0);
    assert_eq!(TicketV3::decode(&ticket), Err(SeriesV3Error::Identity));
}

#[test]
fn schedule_overflow_and_wrong_scheduled_slot_refuse() {
    let fixture = Fixture::new();
    let mut overflow = fixture.template;
    put(
        &mut overflow,
        generated::SERIES_TEMPLATE_FIRST_SLOT_OFFSET_V3,
        &u64::MAX.to_le_bytes(),
    );
    assert_eq!(TemplateV3::decode(&overflow), Err(SeriesV3Error::Schedule));

    let mut wrong = fixture.clone();
    put(
        &mut wrong.occurrence,
        generated::SERIES_OCCURRENCE_SCHEDULED_SLOT_OFFSET_V3,
        &111_u64.to_le_bytes(),
    );
    wrong.recommit_occurrence();
    assert_eq!(
        admit_occurrence(&wrong.template, &wrong.occurrence, &wrong.siblings),
        Err(SeriesV3Error::Commitment)
    );
}

#[test]
fn product_basis_and_representation_substitution_refuse() {
    let fixture = Fixture::new();
    for offset in [
        generated::SERIES_OCCURRENCE_PRODUCT_RECORD_OFFSET_V3,
        generated::SERIES_OCCURRENCE_LIABILITY_BASIS_OFFSET_V3,
        generated::SERIES_OCCURRENCE_RATIONAL_REPRESENTATION_OFFSET_V3,
    ] {
        let mut occurrence = fixture.occurrence;
        *occurrence.get_mut(offset).expect("occurrence field") ^= 0x40;
        assert_eq!(
            admit_occurrence(&fixture.template, &occurrence, &fixture.siblings),
            Err(SeriesV3Error::Commitment)
        );
    }
}

#[test]
fn merkle_sibling_length_value_and_order_refuse() {
    let fixture = Fixture::new();
    assert_eq!(
        admit_occurrence(
            &fixture.template,
            &fixture.occurrence,
            fixture.siblings.get(..1).expect("short proof")
        ),
        Err(SeriesV3Error::Commitment)
    );
    let mut changed = fixture.siblings;
    *changed
        .first_mut()
        .expect("first sibling")
        .first_mut()
        .expect("first sibling byte") ^= 1;
    assert_eq!(
        admit_occurrence(&fixture.template, &fixture.occurrence, &changed),
        Err(SeriesV3Error::Commitment)
    );
    let swapped = [
        *fixture.siblings.get(1).expect("second sibling"),
        *fixture.siblings.first().expect("first sibling"),
    ];
    assert_eq!(
        admit_occurrence(&fixture.template, &fixture.occurrence, &swapped),
        Err(SeriesV3Error::Commitment)
    );
}

#[test]
fn market_pda_commits_every_core_identity_coordinate() {
    let fixture = Fixture::new();
    let admitted = fixture.admit();
    require_market_pda(
        admitted,
        fixture.product,
        &fixture.core_program,
        &fixture.registry_program,
    )
    .expect("exact Market");
    assert_eq!(
        require_market_pda(
            admitted,
            fixture.product,
            &key(62),
            &fixture.registry_program
        ),
        Err(SeriesV3Error::Market)
    );
    assert_eq!(
        require_market_pda(admitted, fixture.product, &fixture.core_program, &key(58)),
        Err(SeriesV3Error::Market)
    );

    let mut substituted = fixture.clone();
    *substituted
        .occurrence
        .get_mut(generated::SERIES_OCCURRENCE_MARKET_OFFSET_V3)
        .expect("Market byte") ^= 1;
    substituted.recommit_occurrence();
    assert_eq!(
        require_market_pda(
            substituted.admit(),
            substituted.product,
            &substituted.core_program,
            &substituted.registry_program,
        ),
        Err(SeriesV3Error::Market)
    );
}

#[test]
fn funding_list_is_ordered_bounded_nonzero_and_alias_free() {
    let fixture = Fixture::new();
    let original = funding_list_id(&fixture.funding).expect("funding");
    let first = *fixture.funding.first().expect("first funding");
    let second = *fixture.funding.get(1).expect("second funding");
    let reversed = funding_list_id(&[second, first]).expect("reversed funding");
    assert_ne!(original, reversed);
    assert_eq!(
        funding_list_id(&[first, first]),
        Err(SeriesV3Error::Funding)
    );
    assert_eq!(funding_list_id(&[]), Err(SeriesV3Error::Funding));
    assert_eq!(
        funding_list_id(&[Pubkey::default()]),
        Err(SeriesV3Error::Funding)
    );
    assert_eq!(
        require_funding_list(fixture.admit().occurrence(), &[second]),
        Err(SeriesV3Error::Funding)
    );
}

#[test]
fn ticket_substitution_and_funding_mismatch_refuse() {
    let fixture = Fixture::new();
    let admitted = fixture.admit();
    let ticket = TicketV3::decode(&fixture.ticket).expect("ticket");
    admitted.require_ticket(ticket).expect("exact");
    for offset in [
        generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
        generated::SERIES_TICKET_MARKET_OFFSET_V3,
        generated::SERIES_TICKET_FUNDING_LIST_OFFSET_V3,
        generated::SERIES_TICKET_HOARD_PRINCIPAL_OFFSET_V3,
    ] {
        let mut bytes = fixture.ticket;
        *bytes.get_mut(offset).expect("Ticket field") ^= 1;
        let changed = TicketV3::decode(&bytes).expect("changed ticket decodes");
        assert_eq!(
            admitted.require_ticket(changed),
            Err(SeriesV3Error::Commitment)
        );
    }
}

#[test]
fn compartment_total_overflow_refuses_without_reclassification() {
    let fixture = Fixture::new();
    let mut occurrence = fixture.occurrence;
    for offset in [
        generated::SERIES_OCCURRENCE_HOARD_PRINCIPAL_OFFSET_V3,
        generated::SERIES_OCCURRENCE_MARKET_RENT_OFFSET_V3,
        generated::SERIES_OCCURRENCE_CAPABILITY_NATIVE_OFFSET_V3,
        generated::SERIES_OCCURRENCE_FOUNDING_WORK_OFFSET_V3,
    ] {
        put(&mut occurrence, offset, &u64::MAX.to_le_bytes());
    }
    assert_eq!(
        OccurrenceV3::decode(&occurrence),
        Err(SeriesV3Error::Funding)
    );

    let mut ticket = fixture.ticket;
    for offset in [
        generated::SERIES_TICKET_HOARD_PRINCIPAL_OFFSET_V3,
        generated::SERIES_TICKET_MARKET_RENT_OFFSET_V3,
        generated::SERIES_TICKET_CAPABILITY_NATIVE_OFFSET_V3,
        generated::SERIES_TICKET_FOUNDING_WORK_OFFSET_V3,
    ] {
        put(&mut ticket, offset, &u64::MAX.to_le_bytes());
    }
    assert_eq!(TicketV3::decode(&ticket), Err(SeriesV3Error::Funding));
}

#[test]
fn collateral_principal_is_never_added_to_native_lamports() {
    let fixture = Fixture::new();
    let mut occurrence = fixture.occurrence;
    put(
        &mut occurrence,
        generated::SERIES_OCCURRENCE_HOARD_PRINCIPAL_OFFSET_V3,
        &u64::MAX.to_le_bytes(),
    );
    let value = OccurrenceV3::decode(&occurrence).expect("collateral is a separate asset");
    assert_eq!(value.funds().hoard_principal(), u64::MAX);
    assert_eq!(value.funds().checked_native_total(), Ok(90));

    put(
        &mut occurrence,
        generated::SERIES_OCCURRENCE_MARKET_RENT_OFFSET_V3,
        &u64::MAX.to_le_bytes(),
    );
    assert_eq!(
        OccurrenceV3::decode(&occurrence),
        Err(SeriesV3Error::Funding)
    );
}

#[test]
fn prepare_and_expire_commit_under_controller_without_fabricating_core() {
    let fixture = Fixture::new();
    let admitted = fixture.admit();
    let admitted_ticket = admit_ticket(&fixture.ticket).expect("Ticket identity");
    let initial = state::SeriesStateV3::new(admitted.template().close_rent());
    let occurrence_zero = initial
        .prepare_ticket(0)
        .expect("synthetic occurrence zero");
    let occurrence_zero = occurrence_zero
        .settle_current(1, admitted.template().occurrence_count())
        .expect("settle zero");
    let series = occurrence_zero
        .retire_ticket(2)
        .expect("retire zero replay");
    assert_eq!(series.next_occurrence(), admitted.occurrence().occurrence());

    let ticket_state_key = key(72);
    let (prepare, top_up, dust_refund) = lifecycle::plan_prepare(
        admitted,
        admitted_ticket,
        series,
        3,
        admitted.occurrence().scheduled_slot() - 1,
        3,
        5,
    )
    .expect("prepare candidate");
    assert_eq!(top_up, 92);
    assert_eq!(dust_refund, 0);
    assert_eq!(prepare.series_after().revision(), 4);
    assert_eq!(prepare.ticket_after().revision(), 0);
    assert_eq!(prepare.core_request(), None);

    let expire = lifecycle::plan_expire(
        admitted,
        admitted_ticket,
        ticket_state_key,
        prepare.series_after(),
        prepare.ticket_after(),
        4,
        0,
        admitted
            .template()
            .retry_through(admitted.occurrence().occurrence())
            .expect("retry")
            + 1,
    )
    .expect("expiry candidate");
    assert_eq!(expire.native_from_ticket(), 90);
    assert_eq!(expire.core_request(), None);
    assert_eq!(
        lifecycle::plan_expire(
            admitted,
            admitted_ticket,
            ticket_state_key,
            expire.series_after(),
            expire.ticket_after(),
            5,
            1,
            u64::MAX,
        ),
        Err(lifecycle::LifecycleErrorV3::Replay)
    );

    let (series_bytes, ticket_bytes) = expire
        .commit_controller()
        .expect("controller commit after direct effects");
    assert_eq!(
        state::SeriesStateV3::decode(&series_bytes, admitted.template().occurrence_count())
            .expect("Series candidate"),
        expire.series_after()
    );
    assert_eq!(
        state::TicketStateV3::decode(&ticket_bytes).expect("Ticket candidate"),
        expire.ticket_after()
    );

    let request = core_request(
        admitted,
        fixture.product,
        SeriesCoreActionV1::Consume,
        admitted_ticket,
        ticket_state_key,
        4,
        0,
    )
    .expect("hostile unrelated Core request");
    let request_bytes = request.encode().expect("request bytes");
    let request_digest = core_identity(
        ContentId::new(solana_program::hash::hash(&request_bytes).to_bytes())
            .expect("request digest"),
    )
    .expect("Core digest");
    let post_digest =
        core_identity(ContentId::new([88; 32]).expect("post digest")).expect("Core post digest");
    let core_program = core_pubkey_identity(fixture.core_program).expect("Core program");
    let ack = dclutch_market_core_codec::SeriesCoreAckV1::new(
        request,
        core_program,
        request_digest,
        post_digest,
    );
    assert_eq!(
        expire.commit_after_ack(ack, core_program, request_digest, post_digest),
        Err(lifecycle::LifecycleErrorV3::CoreAck)
    );
}

#[test]
fn core_request_uses_exact_v3_ticket_and_compartments() {
    let fixture = Fixture::new();
    let admitted = fixture.admit();
    let ticket = admit_ticket(&fixture.ticket).expect("ticket");
    let request = core_request(
        admitted,
        fixture.product,
        SeriesCoreActionV1::Consume,
        ticket,
        key(72),
        8,
        11,
    )
    .expect("Core request");
    assert_eq!(
        request.release_set().to_bytes(),
        admitted.template().release_set().to_bytes()
    );
    assert_eq!(
        request.template().to_bytes(),
        admitted.template_id().to_bytes()
    );
    assert_eq!(
        request.market().expect("market").to_bytes(),
        admitted.occurrence().market().to_bytes()
    );
    assert_eq!(
        request.product().expect("product").to_bytes(),
        admitted.occurrence().product_record().to_bytes()
    );
    assert_eq!(
        request.occurrence_index(),
        admitted.occurrence().occurrence()
    );
    assert_eq!(
        request.market_rent(),
        admitted.occurrence().funds().market_rent()
    );
    assert_eq!(
        request.capability_rent(),
        admitted.occurrence().funds().capability_native()
    );
    assert_eq!(
        request.work(),
        admitted.occurrence().funds().founding_work()
    );
    assert_eq!(
        request.hoard_principal(),
        admitted.occurrence().funds().hoard_principal()
    );
    assert_eq!(
        core_request(
            admitted,
            fixture.product,
            SeriesCoreActionV1::Close,
            ticket,
            key(72),
            8,
            11,
        ),
        Err(SeriesV3Error::Action)
    );
    assert_eq!(
        core_request(
            admitted,
            fixture.product,
            SeriesCoreActionV1::Prepare,
            ticket,
            key(72),
            8,
            11,
        ),
        Err(SeriesV3Error::Action)
    );
}

#[test]
fn checked_in_series_v3_constants_are_exact_lean_output() {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = std::process::Command::new("lake")
        .args(["build", "DClutchSemantics.SeriesOccurrenceV3Abi"])
        .current_dir(&formal)
        .output()
        .expect("build Series V3 semantic ABI");
    assert!(
        build.status.success(),
        "Series V3 semantic build failed: {}",
        std::string::String::from_utf8_lossy(&build.stderr)
    );
    let output = std::process::Command::new("lake")
        .args(["env", "lean", "--run", "EmitSeriesOccurrenceV3Rust.lean"])
        .current_dir(&formal)
        .output()
        .expect("run Series V3 Lean generator");
    assert!(
        output.status.success(),
        "Series V3 generator failed: {}",
        std::string::String::from_utf8_lossy(&output.stderr)
    );
    let checked_in =
        std::fs::read(manifest.join("../../crates/dclutch-series-v3-kernel/src/generated.rs"))
            .expect("read checked-in Series V3 constants");
    assert_eq!(output.stdout, checked_in, "regenerate Series V3 constants");
}
