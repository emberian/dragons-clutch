//! Hostile coverage for the Lean-owned cross-program physical ABI.

use dclutch_market_core_codec::{
    Binding, CORE_EFFECT_ACK_BYTES_V1, CORE_EFFECT_ENVELOPE_BYTES_V1, CoreEffectAckV1,
    CoreEffectActionV1, CoreEffectEnvelopeV1, CoreMarketViewV1, CoreReferenceObservationV1,
    CoreState, Error, FOUNDING_INTENT_BYTES_V5, FoundingIntentV5, Identity,
    MARKET_CORE_STATE_PDA_DOMAIN_V2, MarketCoreStateSeedsV2, MarketIdentity, Phase, Product,
    Readiness, Realm, ReleaseSet, Role, SERIES_CORE_ACK_BYTES_V1,
    SERIES_CORE_CALLER_AUTHORITY_PDA_DOMAIN_V1, SERIES_CORE_REQUEST_BYTES_V1,
    SERIES_FOUNDING_PERMIT_BYTES_V1, SERIES_FOUNDING_PERMIT_PDA_DOMAIN_V1, SeriesCoreAckV1,
    SeriesCoreActionV1, SeriesCoreCallerSeedsV1, SeriesCoreRequestV1, SeriesFoundingPermitV1,
};
use dclutch_release_set_contract::{CALLER_AUTHORITY_PDA_DOMAIN_V1, ExecutionRoleV1};

fn id(byte: u8) -> Identity {
    Identity::new([byte; 32]).expect("fixture identity is nonzero")
}

fn envelope() -> CoreEffectEnvelopeV1 {
    CoreEffectEnvelopeV1::new(
        CoreEffectActionV1::CreateFund,
        Role::Resolution,
        id(1),
        id(2),
        id(3),
        id(4),
        id(5),
        id(6),
        id(7),
        8,
        9,
        10,
        416,
    )
    .expect("valid envelope fixture")
}

fn ack() -> CoreEffectAckV1 {
    CoreEffectAckV1::new(
        CoreEffectActionV1::CreateFund,
        Role::Resolution,
        id(20),
        id(3),
        id(4),
        id(5),
        id(21),
        id(22),
        9,
        10,
        10,
        12,
    )
    .expect("valid acknowledgment fixture")
}

#[test]
fn effect_envelope_is_exact_role_bound_and_round_trips() {
    let envelope = envelope();
    let bytes = envelope.encode().expect("envelope encodes");
    assert_eq!(bytes.len(), CORE_EFFECT_ENVELOPE_BYTES_V1);
    assert_eq!(CoreEffectEnvelopeV1::decode(&bytes), Ok(envelope));
    assert_eq!(envelope.action(), CoreEffectActionV1::CreateFund);
    assert_eq!(envelope.target_role(), Role::Resolution);
    assert_eq!(envelope.caller_program(), id(1));
    assert_eq!(envelope.caller_authority(), id(2));
    assert_eq!(envelope.release_set(), id(3));
    assert_eq!(envelope.market(), id(4));
    assert_eq!(envelope.context(), id(5));
    assert_eq!(envelope.parent_state_digest(), id(6));
    assert_eq!(envelope.role_request_digest(), id(7));
    assert_eq!(envelope.generation(), 8);
    assert_eq!(envelope.expected_resource_a_revision(), 9);
    assert_eq!(envelope.expected_resource_b_revision(), 10);
    assert_eq!(envelope.role_request_bytes(), 416);
    let release_set = id(3).to_bytes();
    let market = id(4).to_bytes();
    let caller_role = [ExecutionRoleV1::Core as u8];
    let context = id(5).to_bytes();
    let request_digest = id(7).to_bytes();
    let expected_seeds: [&[u8]; 6] = [
        CALLER_AUTHORITY_PDA_DOMAIN_V1,
        release_set.as_slice(),
        market.as_slice(),
        caller_role.as_slice(),
        context.as_slice(),
        request_digest.as_slice(),
    ];
    assert_eq!(
        envelope
            .caller_authority_seeds()
            .expect("valid Core caller authority")
            .as_slices(),
        expected_seeds
    );
    assert_eq!(envelope.validate_role_request(416, id(7)), Ok(()));
    assert_eq!(
        CoreEffectEnvelopeV1::new(
            CoreEffectActionV1::CreateFund,
            Role::Custody,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            8,
            9,
            10,
            416,
        ),
        Err(Error::InvalidCoordinates)
    );
    assert_eq!(
        envelope.validate_role_request(415, id(7)),
        Err(Error::InvalidCoordinates)
    );
    assert_eq!(
        envelope.validate_role_request(416, id(8)),
        Err(Error::InvalidCoordinates)
    );
}

#[test]
fn generic_capability_envelopes_bind_one_non_core_target_role() {
    for action in [
        CoreEffectActionV1::ActivateCapability,
        CoreEffectActionV1::CloseCapability,
    ] {
        assert_eq!(action.fixed_target_role(), None);
        let envelope = CoreEffectEnvelopeV1::new(
            action,
            Role::Trading,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            8,
            9,
            10,
            416,
        )
        .expect("optional capability routes to its selected child role");
        assert_eq!(envelope.target_role(), Role::Trading);
        assert_eq!(
            CoreEffectEnvelopeV1::decode(&envelope.encode().expect("envelope encodes")),
            Ok(envelope)
        );
        assert_eq!(
            CoreEffectEnvelopeV1::new(
                action,
                Role::Core,
                id(1),
                id(2),
                id(3),
                id(4),
                id(5),
                id(6),
                id(7),
                8,
                9,
                10,
                416,
            ),
            Err(Error::InvalidCoordinates)
        );
    }
}

#[test]
fn initialize_claims_is_a_distinct_lean_owned_claims_effect() {
    assert_eq!(
        dclutch_market_core_codec::CORE_EFFECT_INITIALIZE_CLAIMS_ACTION_TAG_V1,
        12
    );
    assert_eq!(
        CoreEffectActionV1::InitializeClaims.fixed_target_role(),
        Some(Role::Claims)
    );
    let value = CoreEffectEnvelopeV1::new(
        CoreEffectActionV1::InitializeClaims,
        Role::Claims,
        id(1),
        id(2),
        id(3),
        id(4),
        id(5),
        id(6),
        id(7),
        8,
        0,
        0,
        416,
    )
    .expect("foundational Claims initialization");
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&value.encode().expect("exact envelope")),
        Ok(value)
    );
    assert_eq!(
        CoreEffectEnvelopeV1::new(
            CoreEffectActionV1::InitializeClaims,
            Role::Trading,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            8,
            0,
            0,
            416,
        ),
        Err(Error::InvalidCoordinates)
    );
}

#[test]
fn effect_envelope_hostile_bytes_are_refused() {
    let bytes = envelope().encode().expect("fixture encodes");
    let short = bytes
        .get(..bytes.len().saturating_sub(1))
        .expect("fixture has a shorter prefix");
    assert_eq!(
        CoreEffectEnvelopeV1::decode(short),
        Err(Error::InvalidLength)
    );
    let mut long = bytes.to_vec();
    long.push(0);
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&long),
        Err(Error::InvalidLength)
    );

    let mut hostile = bytes;
    hostile[0] ^= 1;
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&hostile),
        Err(Error::InvalidMagic)
    );
    let mut hostile = bytes;
    hostile[8] = 2;
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&hostile),
        Err(Error::UnsupportedVersion)
    );
    let mut hostile = bytes;
    hostile[10] = u8::MAX;
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&hostile),
        Err(Error::InvalidTag)
    );
    let mut hostile = bytes;
    hostile[11] = Role::Custody as u8;
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&hostile),
        Err(Error::InvalidCoordinates)
    );
    let mut hostile = bytes;
    hostile[12] = 1;
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile[16..48].fill(0);
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&hostile),
        Err(Error::InvalidIdentity)
    );
    let mut hostile = bytes;
    hostile[268] = 1;
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
}

#[test]
fn acknowledgement_binds_full_effect_and_monotonic_revisions() {
    let envelope = envelope();
    let ack = ack();
    let bytes = ack.encode().expect("acknowledgment encodes");
    assert_eq!(bytes.len(), CORE_EFFECT_ACK_BYTES_V1);
    assert_eq!(CoreEffectAckV1::decode(&bytes), Ok(ack));
    assert_eq!(ack.validate_for(envelope, id(20), id(21)), Ok(()));
    assert_eq!(ack.action(), CoreEffectActionV1::CreateFund);
    assert_eq!(ack.target_role(), Role::Resolution);
    assert_eq!(ack.role_program(), id(20));
    assert_eq!(ack.release_set(), id(3));
    assert_eq!(ack.market(), id(4));
    assert_eq!(ack.context(), id(5));
    assert_eq!(ack.effect_digest(), id(21));
    assert_eq!(ack.post_resource_digest(), id(22));
    assert_eq!(ack.pre_resource_a_revision(), 9);
    assert_eq!(ack.post_resource_a_revision(), 10);
    assert_eq!(ack.pre_resource_b_revision(), 10);
    assert_eq!(ack.post_resource_b_revision(), 12);
    assert_eq!(
        ack.validate_for(envelope, id(23), id(21)),
        Err(Error::InvalidRelease)
    );
    assert_eq!(
        ack.validate_for(envelope, id(20), id(23)),
        Err(Error::InvalidRelease)
    );
    assert_eq!(
        CoreEffectAckV1::new(
            CoreEffectActionV1::CreateFund,
            Role::Resolution,
            id(20),
            id(3),
            id(4),
            id(5),
            id(21),
            id(22),
            9,
            8,
            10,
            12,
        ),
        Err(Error::InvalidCoordinates)
    );
}

#[test]
fn acknowledgement_hostile_bytes_are_refused() {
    let bytes = ack().encode().expect("fixture encodes");
    let short = bytes
        .get(..bytes.len().saturating_sub(1))
        .expect("fixture has a shorter prefix");
    assert_eq!(CoreEffectAckV1::decode(short), Err(Error::InvalidLength));
    let mut hostile = bytes;
    hostile[12] = 1;
    assert_eq!(
        CoreEffectAckV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile[16..48].fill(0);
    assert_eq!(
        CoreEffectAckV1::decode(&hostile),
        Err(Error::InvalidIdentity)
    );
    let mut hostile = bytes;
    hostile[216..224].copy_from_slice(&8_u64.to_le_bytes());
    assert_eq!(
        CoreEffectAckV1::decode(&hostile),
        Err(Error::InvalidCoordinates)
    );
}

fn occurrence(action: SeriesCoreActionV1) -> SeriesCoreRequestV1 {
    SeriesCoreRequestV1::occurrence(
        action,
        id(30),
        id(31),
        id(32),
        id(33),
        id(34),
        id(35),
        id(36),
        id(37),
        38,
        39,
        40,
        41,
        42,
        43,
        44,
    )
    .expect("valid occurrence fixture")
}

fn founding_intent() -> FoundingIntentV5 {
    FoundingIntentV5::new(
        254,
        id(80),
        id(81),
        id(82),
        id(83),
        id(84),
        id(85),
        id(86),
        id(87),
        id(88),
        id(89),
        id(90),
        id(91),
        id(92),
        id(93),
        id(94),
        95,
        96,
        97,
        98,
        4,
        1,
    )
    .expect("valid founding intent")
}

fn founding_permit() -> SeriesFoundingPermitV1 {
    SeriesFoundingPermitV1::new(founding_intent(), id(95), id(96)).expect("valid founding permit")
}

#[test]
fn series_founding_permit_and_intent_are_exact_and_cycle_free() {
    let intent = founding_intent();
    let intent_bytes = intent.encode().expect("intent encodes");
    assert_eq!(intent_bytes.len(), FOUNDING_INTENT_BYTES_V5);
    assert_eq!(FoundingIntentV5::decode(&intent_bytes), Ok(intent));
    assert_eq!(intent.bump(), 254);
    assert_eq!(intent.release_set(), id(80));
    assert_eq!(intent.market(), id(81));
    assert_eq!(intent.product_record(), id(82));
    assert_eq!(intent.source(), id(83));
    assert_eq!(intent.founder(), id(84));
    assert_eq!(intent.ticket_context(), id(85));
    assert_eq!(intent.parent_root(), id(86));
    assert_eq!(intent.projected_replay(), id(87));
    assert_eq!(intent.funding_source(), id(88));
    assert_eq!(intent.hoard(), id(89));
    assert_eq!(intent.projected_request_digest(), id(90));
    assert_eq!(intent.projected_receipt_digest(), id(91));
    assert_eq!(intent.trading_program(), id(92));
    assert_eq!(intent.claims_program(), id(93));
    assert_eq!(intent.rent_credit(), id(94));
    assert_eq!(intent.generation(), 95);
    assert_eq!(intent.quantity(), 96);
    assert_eq!(intent.basis_scale(), 97);
    assert_eq!(intent.expiry_slot(), 98);
    assert_eq!(intent.projected_resulting_revision(), 4);
    assert_eq!(intent.normal_replay_revision(), 1);

    let permit = founding_permit();
    let permit_bytes = permit.encode().expect("permit encodes");
    assert_eq!(permit_bytes.len(), SERIES_FOUNDING_PERMIT_BYTES_V1);
    assert_eq!(SeriesFoundingPermitV1::decode(&permit_bytes), Ok(permit));
    assert_eq!(permit.intent(), intent);
    assert_eq!(permit.claims_intent_digest(), id(95));
    assert_eq!(permit.claims_request_digest(), id(96));
    assert_eq!(
        permit.verify_for_intent_and_request(intent, id(95), id(96)),
        Ok(())
    );

    let release = id(80).to_bytes();
    let market = id(81).to_bytes();
    let ticket = id(85).to_bytes();
    let expected: [&[u8]; 4] = [
        SERIES_FOUNDING_PERMIT_PDA_DOMAIN_V1.as_slice(),
        release.as_slice(),
        market.as_slice(),
        ticket.as_slice(),
    ];
    assert_eq!(permit.seeds().as_slices(), expected);
}

#[test]
fn series_founding_permit_refuses_hostile_aliases_and_substitutions() {
    let permit = founding_permit();
    let bytes = permit.encode().expect("permit encodes");
    let short = bytes
        .get(..bytes.len().saturating_sub(1))
        .expect("permit has a shorter prefix");
    assert_eq!(
        SeriesFoundingPermitV1::decode(short),
        Err(Error::InvalidLength)
    );
    let mut hostile = bytes;
    hostile[0] ^= 1;
    assert_eq!(
        SeriesFoundingPermitV1::decode(&hostile),
        Err(Error::InvalidMagic)
    );
    let mut hostile = bytes;
    hostile[11] = 1;
    assert_eq!(
        SeriesFoundingPermitV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile[496..528].copy_from_slice(&id(92).to_bytes());
    assert_eq!(
        SeriesFoundingPermitV1::decode(&hostile),
        Err(Error::InvalidCoordinates)
    );
    let mut hostile = bytes;
    hostile[600..608].copy_from_slice(&2_u64.to_le_bytes());
    assert_eq!(
        SeriesFoundingPermitV1::decode(&hostile),
        Err(Error::InvalidCoordinates)
    );
    let mut hostile = bytes;
    hostile[272..304].copy_from_slice(&id(89).to_bytes());
    assert_eq!(
        SeriesFoundingPermitV1::decode(&hostile),
        Err(Error::InvalidCoordinates)
    );
    assert_eq!(
        permit.verify_for_intent_and_request(founding_intent(), id(97), id(96)),
        Err(Error::InvalidCoordinates)
    );
    assert_eq!(
        permit.verify_for_intent_and_request(founding_intent(), id(95), id(97)),
        Err(Error::InvalidCoordinates)
    );
}

#[test]
fn series_occurrence_request_is_exact_and_round_trips() {
    for action in [
        SeriesCoreActionV1::Prepare,
        SeriesCoreActionV1::Consume,
        SeriesCoreActionV1::Expire,
    ] {
        let request = occurrence(action);
        let bytes = request.encode().expect("request encodes");
        assert_eq!(bytes.len(), SERIES_CORE_REQUEST_BYTES_V1);
        assert_eq!(SeriesCoreRequestV1::decode(&bytes), Ok(request));
        assert_eq!(request.action(), action);
        assert_eq!(request.release_set(), id(30));
        assert_eq!(request.template(), id(31));
        assert_eq!(request.ticket(), Some(id(32)));
        assert_eq!(request.market(), Some(id(33)));
        assert_eq!(request.realm(), Some(id(34)));
        assert_eq!(request.product(), Some(id(35)));
        assert_eq!(request.beneficiary(), id(36));
        assert_eq!(request.founder(), Some(id(37)));
        assert_eq!(request.occurrence_index(), 38);
        assert_eq!(request.market_generation(), Some(39));
        assert_eq!(request.expected_series_revision(), 39);
        assert_eq!(request.expected_ticket_revision(), 40);
        assert_eq!(request.market_rent(), 41);
        assert_eq!(request.capability_rent(), 42);
        assert_eq!(request.work(), 43);
        assert_eq!(request.hoard_principal(), 44);
        assert_eq!(request.series_close_rent(), 0);
    }
}

#[test]
fn series_close_is_disjoint_from_occurrence_shape() {
    let close =
        SeriesCoreRequestV1::close(id(30), id(31), id(36), 39, 44).expect("valid close fixture");
    let bytes = close.encode().expect("close encodes");
    assert_eq!(SeriesCoreRequestV1::decode(&bytes), Ok(close));
    assert_eq!(close.action(), SeriesCoreActionV1::Close);
    assert_eq!(close.ticket(), None);
    assert_eq!(close.market(), None);
    assert_eq!(close.realm(), None);
    assert_eq!(close.product(), None);
    assert_eq!(close.founder(), None);
    assert_eq!(close.market_generation(), None);
    assert_eq!(close.series_close_rent(), 44);

    let mut hostile = bytes;
    hostile[80] = 1;
    assert_eq!(
        SeriesCoreRequestV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile[240] = 1;
    assert_eq!(
        SeriesCoreRequestV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile[272] = 1;
    assert_eq!(
        SeriesCoreRequestV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile[288] = 1;
    assert_eq!(
        SeriesCoreRequestV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
}

#[test]
fn series_generation_and_caller_pda_are_exact_to_the_request() {
    let request = SeriesCoreRequestV1::occurrence(
        SeriesCoreActionV1::Prepare,
        id(30),
        id(31),
        id(32),
        id(33),
        id(34),
        id(35),
        id(36),
        id(37),
        u32::MAX,
        39,
        40,
        41,
        42,
        43,
        44,
    )
    .expect("maximum occurrence remains representable");
    assert_eq!(request.market_generation(), Some(u64::from(u32::MAX) + 1));

    let template = id(31).to_bytes();
    let request_digest = id(50).to_bytes();
    let expected: [&[u8]; 3] = [
        SERIES_CORE_CALLER_AUTHORITY_PDA_DOMAIN_V1.as_slice(),
        template.as_slice(),
        request_digest.as_slice(),
    ];
    let seeds = SeriesCoreCallerSeedsV1::new(request, id(50));
    assert_eq!(seeds.as_slices(), expected);
    assert_ne!(
        seeds,
        SeriesCoreCallerSeedsV1::new(request, id(51)),
        "a substituted request digest must select another caller PDA",
    );
}

#[test]
fn market_state_pda_commits_every_immutable_coordinate() {
    let identity = MarketIdentity {
        market_id: id(40),
        realm_id: id(41),
        product_record: id(42),
        product_id: id(43),
        resolution_policy: id(44),
        capability_manifest: id(48),
        selected_release_set: id(45),
        registry_program: id(49),
        generation: 46,
    };
    let realm = id(41).to_bytes();
    let product_record = id(42).to_bytes();
    let product_id = id(43).to_bytes();
    let resolution = id(44).to_bytes();
    let capability_manifest = id(48).to_bytes();
    let release_set = id(45).to_bytes();
    let registry_program = id(49).to_bytes();
    let generation = 46_u64.to_le_bytes();
    let expected: [&[u8]; 9] = [
        MARKET_CORE_STATE_PDA_DOMAIN_V2.as_slice(),
        realm.as_slice(),
        product_record.as_slice(),
        product_id.as_slice(),
        resolution.as_slice(),
        capability_manifest.as_slice(),
        release_set.as_slice(),
        registry_program.as_slice(),
        generation.as_slice(),
    ];
    assert_eq!(MarketCoreStateSeedsV2::new(identity).as_slices(), expected);

    let mut substituted = identity;
    substituted.generation = 47;
    assert_ne!(
        MarketCoreStateSeedsV2::new(identity),
        MarketCoreStateSeedsV2::new(substituted),
    );
    substituted = identity;
    substituted.product_record = id(51);
    assert_ne!(
        MarketCoreStateSeedsV2::new(identity),
        MarketCoreStateSeedsV2::new(substituted),
    );
    substituted = identity;
    substituted.product_id = id(52);
    assert_ne!(
        MarketCoreStateSeedsV2::new(identity),
        MarketCoreStateSeedsV2::new(substituted),
    );
    substituted = identity;
    substituted.registry_program = id(50);
    assert_ne!(
        MarketCoreStateSeedsV2::new(identity),
        MarketCoreStateSeedsV2::new(substituted),
    );
    substituted = identity;
    substituted.resolution_policy = id(47);
    assert_ne!(
        MarketCoreStateSeedsV2::new(identity),
        MarketCoreStateSeedsV2::new(substituted),
    );
}

fn core_state_for_view() -> CoreState {
    CoreState {
        phase: Phase::Open,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity: MarketIdentity {
            market_id: id(40),
            realm_id: id(41),
            product_record: id(47),
            product_id: id(46),
            resolution_policy: id(56),
            capability_manifest: id(67),
            selected_release_set: id(45),
            registry_program: id(49),
            generation: 7,
        },
        outstanding_capabilities: 0,
        rent_beneficiary: id(66),
        terminal_receipt: None,
    }
}

fn references_for_view() -> CoreReferenceObservationV1 {
    CoreReferenceObservationV1 {
        realm: Realm {
            realm_id: id(41),
            collateral_mint: id(42),
            token_program: id(43),
            collateral_release: id(44),
        },
        product: Product {
            product_record: id(47),
            product_id: id(46),
            result_domain: id(57),
            portfolio: id(58),
            coordinate_domain: id(59),
            result_unit: id(65),
            claim_basis: id(48),
            liability_basis: id(68),
            representation_release: id(69),
            mapping_release: id(75),
            outcome_count: 3,
        },
        release_set: ReleaseSet {
            release_set_id: id(45),
            bindings: [
                Binding {
                    program: id(50),
                    artifact_release: id(60),
                    semantic_release: id(70),
                },
                Binding {
                    program: id(51),
                    artifact_release: id(61),
                    semantic_release: id(71),
                },
                Binding {
                    program: id(52),
                    artifact_release: id(62),
                    semantic_release: id(72),
                },
                Binding {
                    program: id(53),
                    artifact_release: id(63),
                    semantic_release: id(73),
                },
                Binding {
                    program: id(54),
                    artifact_release: id(64),
                    semantic_release: id(74),
                },
            ],
        },
        realm_record_authenticated: true,
        product_graph_authenticated: true,
        release_set_record_authenticated: true,
        claims_aggregate_derivation_authenticated: true,
    }
}

#[test]
fn core_market_view_joins_logical_market_and_distinct_claims_aggregate() {
    let state = core_state_for_view();
    let references = references_for_view();
    let view = CoreMarketViewV1::authenticate(state, id(40), id(57), references)
        .expect("exact Core coordinates join");
    assert_eq!(view.market(), id(40));
    assert_eq!(view.claims_aggregate(), id(57));
    assert_ne!(view.market(), view.claims_aggregate());
    assert_eq!(view.realm(), references.realm);
    assert_eq!(view.product(), references.product);
    assert_eq!(view.release_set(), references.release_set);
    assert_eq!(view.generation(), 7);
    assert_eq!(view.phase(), Phase::Open);
    assert_eq!(
        CoreMarketViewV1::authenticate(state, id(57), id(57), references),
        Err(Error::InvalidCoordinates)
    );
    assert_eq!(
        CoreMarketViewV1::authenticate(state, id(40), id(40), references),
        Err(Error::InvalidCoordinates)
    );
    assert_eq!(
        CoreMarketViewV1::authenticate(
            state,
            id(40),
            id(57),
            CoreReferenceObservationV1 {
                product_graph_authenticated: false,
                ..references
            },
        ),
        Err(Error::InvalidCoordinates)
    );
}

#[test]
fn series_acknowledgement_binds_request_generation_and_core_poststate() {
    let request = occurrence(SeriesCoreActionV1::Consume);
    let ack = SeriesCoreAckV1::new(request, id(51), id(52), id(53));
    let bytes = ack.encode().expect("Series acknowledgment encodes");
    assert_eq!(bytes.len(), SERIES_CORE_ACK_BYTES_V1);
    assert_eq!(SeriesCoreAckV1::decode(&bytes), Ok(ack));
    assert_eq!(ack.action(), SeriesCoreActionV1::Consume);
    assert_eq!(ack.core_program(), id(51));
    assert_eq!(ack.release_set(), id(30));
    assert_eq!(ack.template(), id(31));
    assert_eq!(ack.ticket(), Some(id(32)));
    assert_eq!(ack.market(), Some(id(33)));
    assert_eq!(ack.request_digest(), id(52));
    assert_eq!(ack.post_resource_digest(), id(53));
    assert_eq!(ack.market_generation(), Some(39));
    assert_eq!(ack.expected_series_revision(), 39);
    assert_eq!(ack.expected_ticket_revision(), 40);
    assert_eq!(ack.validate_for(request, id(51), id(52), id(53)), Ok(()));
    assert_eq!(
        ack.validate_for(request, id(54), id(52), id(53)),
        Err(Error::InvalidRelease)
    );
    assert_eq!(
        ack.validate_for(request, id(51), id(54), id(53)),
        Err(Error::InvalidRelease)
    );
    assert_eq!(
        ack.validate_for(request, id(51), id(52), id(54)),
        Err(Error::InvalidRelease)
    );

    let close_request =
        SeriesCoreRequestV1::close(id(30), id(31), id(36), 39, 0).expect("valid close");
    let close = SeriesCoreAckV1::new(close_request, id(51), id(52), id(53));
    assert_eq!(
        SeriesCoreAckV1::decode(&close.encode().expect("close acknowledgment encodes")),
        Ok(close)
    );
    assert_eq!(close.ticket(), None);
    assert_eq!(close.market(), None);
    assert_eq!(close.market_generation(), None);
    assert_eq!(close.expected_ticket_revision(), 0);
}

#[test]
fn series_acknowledgement_hostile_bytes_and_partial_shapes_are_refused() {
    let request = occurrence(SeriesCoreActionV1::Prepare);
    let bytes = SeriesCoreAckV1::new(request, id(51), id(52), id(53))
        .encode()
        .expect("acknowledgment encodes");
    let short = bytes
        .get(..bytes.len().saturating_sub(1))
        .expect("fixture has a shorter prefix");
    assert_eq!(SeriesCoreAckV1::decode(short), Err(Error::InvalidLength));
    let mut long = bytes.to_vec();
    long.push(0);
    assert_eq!(SeriesCoreAckV1::decode(&long), Err(Error::InvalidLength));

    let mut hostile = bytes;
    hostile[0] ^= 1;
    assert_eq!(SeriesCoreAckV1::decode(&hostile), Err(Error::InvalidMagic));
    let mut hostile = bytes;
    hostile[8] = 2;
    assert_eq!(
        SeriesCoreAckV1::decode(&hostile),
        Err(Error::UnsupportedVersion)
    );
    let mut hostile = bytes;
    hostile[10] = u8::MAX;
    assert_eq!(SeriesCoreAckV1::decode(&hostile), Err(Error::InvalidTag));
    let mut hostile = bytes;
    hostile[11] = 1;
    assert_eq!(
        SeriesCoreAckV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile[16..48].fill(0);
    assert_eq!(
        SeriesCoreAckV1::decode(&hostile),
        Err(Error::InvalidIdentity)
    );
    for range in [112..144, 144..176, 240..248] {
        let mut hostile = bytes;
        hostile
            .get_mut(range)
            .expect("hostile occurrence field is in bounds")
            .fill(0);
        assert_eq!(
            SeriesCoreAckV1::decode(&hostile),
            Err(Error::InvalidCoordinates)
        );
    }

    let close_request =
        SeriesCoreRequestV1::close(id(30), id(31), id(36), 39, 0).expect("valid close");
    let close = SeriesCoreAckV1::new(close_request, id(51), id(52), id(53))
        .encode()
        .expect("close acknowledgment encodes");
    for range in [112..113, 144..145, 240..241, 256..257] {
        let mut hostile = close;
        hostile
            .get_mut(range)
            .expect("hostile close field is in bounds")
            .fill(1);
        assert_eq!(
            SeriesCoreAckV1::decode(&hostile),
            Err(Error::InvalidCoordinates)
        );
    }
}

#[test]
fn series_hostile_tags_and_inactive_fields_are_refused_without_invented_minima() {
    assert_eq!(
        SeriesCoreRequestV1::occurrence(
            SeriesCoreActionV1::Close,
            id(30),
            id(31),
            id(32),
            id(33),
            id(34),
            id(35),
            id(36),
            id(37),
            38,
            39,
            40,
            41,
            42,
            43,
            44,
        ),
        Err(Error::InvalidCoordinates)
    );
    let zero_close = SeriesCoreRequestV1::close(id(30), id(31), id(36), 39, 0)
        .expect("Series owns the exact zero close-rent fact");
    assert_eq!(
        SeriesCoreRequestV1::decode(&zero_close.encode().expect("zero close encodes")),
        Ok(zero_close)
    );
    let zero_occurrence = SeriesCoreRequestV1::occurrence(
        SeriesCoreActionV1::Prepare,
        id(30),
        id(31),
        id(32),
        id(33),
        id(34),
        id(35),
        id(36),
        id(37),
        38,
        39,
        40,
        0,
        0,
        0,
        44,
    )
    .expect("Series owns exact zero occurrence compartments");
    assert_eq!(
        SeriesCoreRequestV1::decode(&zero_occurrence.encode().expect("zero request encodes")),
        Ok(zero_occurrence)
    );
    let bytes = occurrence(SeriesCoreActionV1::Consume)
        .encode()
        .expect("fixture encodes");
    let mut hostile = bytes;
    hostile[10] = u8::MAX;
    assert_eq!(
        SeriesCoreRequestV1::decode(&hostile),
        Err(Error::InvalidTag)
    );
    let mut hostile = bytes;
    hostile[11] = 1;
    assert_eq!(
        SeriesCoreRequestV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile[328] = 1;
    assert_eq!(
        SeriesCoreRequestV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
}
