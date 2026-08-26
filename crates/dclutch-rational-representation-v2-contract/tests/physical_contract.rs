//! Hostile and positive physical Claims/Token/Custody composition corpus.

use dclutch_claims_svm::{ClaimsReceiptV1, NO_POSITION_REVISION};
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyReceiptV1, CustodyRequestV1, OperationV1,
    ReceiptEvidenceV1,
};
use dclutch_rational_representation_v2_contract::{
    ABSENT_REVISION, ASSET_BYTES_V2, AssetV2, CallerRoleV2, CompletionEvidenceV2, Error,
    REQUEST_HEADER_BYTES_V2, RepresentationActionV2, RepresentationReceiptV2,
    RepresentationRequestHeaderV2, RepresentationRequestV2, TokenEffectStyleV2, finalize, prepare,
};
use dclutch_rational_representation_v2_kernel::{
    ContentAdmissionV2, DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V2,
    DescriptorAdmissionV2, GRAPH_EDGE_BYTES, GRAPH_HEADER_BYTES, GRAPH_MAGIC_V2, GRAPH_NODE_BYTES,
    RepresentationDescriptorV2, RepresentationGraphV2, SCHEMA_VERSION_V2, STRUCTURED_HEADER_BYTES,
    STRUCTURED_MAGIC_V2, StructuredProjectionV2,
};
use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    output
        .get_mut(offset..offset + value.len())
        .expect("fixture offset")
        .copy_from_slice(value);
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    put(output, offset, &value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    put(output, offset, &value.to_le_bytes());
}

fn projection_fixture(native0: u64, supply0: u64, free0: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; STRUCTURED_HEADER_BYTES + 80];
    put(&mut bytes, 0, &STRUCTURED_MAGIC_V2);
    put(&mut bytes, 8, &SCHEMA_VERSION_V2.to_le_bytes());
    put(&mut bytes, 16, &id(4));
    put(&mut bytes, 48, &id(2));
    put(&mut bytes, 80, &id(5));
    put_u32(&mut bytes, 112, 2);
    put_u64(&mut bytes, 120, 10);
    put_u64(&mut bytes, 128, 7);
    put_u64(&mut bytes, 136, 4);
    let values = [3_u64, 7, native0, 6, supply0, 60, 21, 49, free0, 11];
    for (index, value) in values.iter().enumerate() {
        put_u64(&mut bytes, STRUCTURED_HEADER_BYTES + index * 8, *value);
    }
    bytes
}

fn descriptor_fixture() -> Vec<u8> {
    let mut bytes = vec![0_u8; DESCRIPTOR_HEADER_BYTES + 2 * DESCRIPTOR_COEFFICIENT_BYTES];
    put(&mut bytes, 0, &DESCRIPTOR_MAGIC_V2);
    put(&mut bytes, 8, &SCHEMA_VERSION_V2.to_le_bytes());
    put(&mut bytes, 16, &id(3));
    put(&mut bytes, 48, &id(40));
    put(&mut bytes, 80, &id(14));
    put(&mut bytes, 112, &id(2));
    put(&mut bytes, 144, &id(1));
    put(&mut bytes, 176, &id(5));
    put(&mut bytes, 208, &TOKEN_2022_PROGRAM_ID);
    put(&mut bytes, 240, &id(9));
    put_u32(&mut bytes, 272, 2);
    put_u64(&mut bytes, 280, 10);
    put_u64(&mut bytes, DESCRIPTOR_HEADER_BYTES, 3);
    put_u64(
        &mut bytes,
        DESCRIPTOR_HEADER_BYTES + DESCRIPTOR_COEFFICIENT_BYTES,
        7,
    );
    bytes
}

fn descriptor<'a>(bytes: &'a [u8]) -> RepresentationDescriptorV2<'a> {
    RepresentationDescriptorV2::decode(
        bytes,
        DescriptorAdmissionV2 {
            selected_descriptor_id: id(4),
            finalized_descriptor_id: id(4),
            recomputed_descriptor_digest: id(4),
            finalized_descriptor_digest: id(4),
            record_authenticated: true,
        },
    )
    .expect("descriptor")
}

#[derive(Clone, Copy)]
struct GraphNode {
    id: u8,
    rank: u32,
    first_edge: u32,
    edge_count: u32,
    kind: u8,
    parameter: u64,
    exposure: [u64; 2],
}

#[derive(Clone, Copy)]
struct GraphEdge {
    child_id: u8,
    child_index: u32,
    multiplicity: u64,
}

fn graph_fixture() -> Vec<u8> {
    let nodes = [
        GraphNode {
            id: 10,
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            kind: 0,
            parameter: 0,
            exposure: [100, 0],
        },
        GraphNode {
            id: 11,
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            kind: 0,
            parameter: 1,
            exposure: [0, 100],
        },
        GraphNode {
            id: 12,
            rank: 1,
            first_edge: 0,
            edge_count: 1,
            kind: 1,
            parameter: 10,
            exposure: [10, 0],
        },
        GraphNode {
            id: 13,
            rank: 1,
            first_edge: 1,
            edge_count: 1,
            kind: 1,
            parameter: 10,
            exposure: [0, 10],
        },
        GraphNode {
            id: 14,
            rank: 2,
            first_edge: 2,
            edge_count: 2,
            kind: 2,
            parameter: 0,
            exposure: [30, 70],
        },
    ];
    let edges = [
        GraphEdge {
            child_id: 10,
            child_index: 0,
            multiplicity: 1,
        },
        GraphEdge {
            child_id: 11,
            child_index: 1,
            multiplicity: 1,
        },
        GraphEdge {
            child_id: 12,
            child_index: 2,
            multiplicity: 3,
        },
        GraphEdge {
            child_id: 13,
            child_index: 3,
            multiplicity: 7,
        },
    ];
    let mut bytes = vec![
        0_u8;
        GRAPH_HEADER_BYTES
            + nodes.len() * GRAPH_NODE_BYTES
            + edges.len() * GRAPH_EDGE_BYTES
            + nodes.len() * 2 * 8
    ];
    put(&mut bytes, 0, &GRAPH_MAGIC_V2);
    put(&mut bytes, 8, &SCHEMA_VERSION_V2.to_le_bytes());
    put(&mut bytes, 16, &id(3));
    put(&mut bytes, 48, &id(14));
    put_u32(&mut bytes, 80, 2);
    put_u32(&mut bytes, 84, 5);
    put_u32(&mut bytes, 88, 4);
    put_u64(&mut bytes, 96, 100);
    for (index, node) in nodes.iter().enumerate() {
        let offset = GRAPH_HEADER_BYTES + index * GRAPH_NODE_BYTES;
        put(&mut bytes, offset, &id(node.id));
        put_u32(&mut bytes, offset + 32, node.rank);
        put_u32(&mut bytes, offset + 36, node.first_edge);
        put_u32(&mut bytes, offset + 40, node.edge_count);
        *bytes.get_mut(offset + 44).expect("node kind") = node.kind;
        put_u64(&mut bytes, offset + 48, node.parameter);
    }
    let edge_start = GRAPH_HEADER_BYTES + nodes.len() * GRAPH_NODE_BYTES;
    for (index, edge) in edges.iter().enumerate() {
        let offset = edge_start + index * GRAPH_EDGE_BYTES;
        put(&mut bytes, offset, &id(edge.child_id));
        put_u32(&mut bytes, offset + 32, edge.child_index);
        put_u64(&mut bytes, offset + 40, edge.multiplicity);
    }
    let exposure_start = edge_start + edges.len() * GRAPH_EDGE_BYTES;
    for (node_index, node) in nodes.iter().enumerate() {
        for (outcome, value) in node.exposure.iter().enumerate() {
            put_u64(
                &mut bytes,
                exposure_start + (node_index * 2 + outcome) * 8,
                *value,
            );
        }
    }
    bytes
}

fn admission() -> ContentAdmissionV2 {
    ContentAdmissionV2 {
        selected_graph_id: id(3),
        finalized_graph_id: id(3),
        recomputed_graph_digest: id(40),
        finalized_graph_digest: id(40),
        record_authenticated: true,
    }
}

fn assets() -> [AssetV2; 2] {
    [
        AssetV2 {
            shard_mint: id(50),
            actor_shard_account: id(51),
            structured_custody_account: id(52),
            claims_custody_owner: id(53),
            coefficient: 3,
            expected_shard_supply: 30,
            expected_actor_shards: 9,
            expected_structured_shards: 21,
        },
        AssetV2 {
            shard_mint: id(54),
            actor_shard_account: id(55),
            structured_custody_account: id(56),
            claims_custody_owner: id(57),
            coefficient: 7,
            expected_shard_supply: 60,
            expected_actor_shards: 11,
            expected_structured_shards: 49,
        },
    ]
}

fn asset_bytes(rows: &[AssetV2]) -> Vec<u8> {
    let mut output = vec![0_u8; rows.len() * ASSET_BYTES_V2];
    for (index, row) in rows.iter().enumerate() {
        row.encode_into(
            output
                .get_mut(index * ASSET_BYTES_V2..(index + 1) * ASSET_BYTES_V2)
                .expect("asset row"),
        )
        .expect("canonical row");
    }
    output
}

fn header(
    action: RepresentationActionV2,
    selected_outcome: u32,
    asset_count: u32,
) -> RepresentationRequestHeaderV2 {
    let claims = action.uses_claims();
    let terminal = action == RepresentationActionV2::RedeemTerminal;
    let structured = matches!(
        action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    );
    RepresentationRequestHeaderV2 {
        action,
        caller_role: CallerRoleV2::Trading,
        release_set: id(1),
        market: id(2),
        graph_id: id(3),
        descriptor_id: id(4),
        parent_context: id(6),
        actor: id(7),
        receipt_mint: id(5),
        receipt_account: if structured { id(8) } else { [0; 32] },
        representation_authority: id(9),
        token_program: TOKEN_2022_PROGRAM_ID,
        realm: if terminal { id(10) } else { [0; 32] },
        collateral_recipient: if terminal { id(11) } else { [0; 32] },
        expected_representation_revision: 4,
        expected_claims_market_revision: if claims { 10 } else { ABSENT_REVISION },
        expected_actor_position_revision: if claims && !terminal {
            20
        } else {
            ABSENT_REVISION
        },
        expected_custody_position_revision: if claims { 30 } else { ABSENT_REVISION },
        expected_custody_replay_revision: if terminal { 5 } else { ABSENT_REVISION },
        generation: 9,
        quantity: 1,
        denominator: 10,
        expected_receipt_supply: 7,
        outcome_count: 2,
        selected_outcome,
        asset_count,
    }
}

fn graph<'a>(bytes: &'a [u8]) -> RepresentationGraphV2<'a> {
    RepresentationGraphV2::decode(bytes, admission()).expect("graph")
}

fn post_assets(rows: &[(u64, u64, u64)]) -> Vec<u8> {
    let mut output = vec![0_u8; rows.len() * 24];
    for (index, row) in rows.iter().enumerate() {
        let offset = index * 24;
        put_u64(&mut output, offset, row.0);
        put_u64(&mut output, offset + 8, row.1);
        put_u64(&mut output, offset + 16, row.2);
    }
    output
}

#[test]
fn structured_issue_is_token_only_and_exactly_conserved() {
    let projection_bytes = projection_fixture(3, 30, 9);
    let projection = StructuredProjectionV2::decode(&projection_bytes).expect("projection");
    let descriptor_bytes = descriptor_fixture();
    let graph_bytes = graph_fixture();
    let row_bytes = asset_bytes(&assets());
    let request = RepresentationRequestV2::new(
        header(RepresentationActionV2::IssueStructured, u32::MAX, 2),
        &row_bytes,
    )
    .expect("request");
    let mut request_bytes = vec![0_u8; REQUEST_HEADER_BYTES_V2 + row_bytes.len()];
    request
        .encode_into(&mut request_bytes)
        .expect("canonical request bytes");
    assert_eq!(RepresentationRequestV2::decode(&request_bytes), Ok(request));
    assert_eq!(
        RepresentationRequestV2::decode(
            request_bytes
                .get(..request_bytes.len() - 1)
                .expect("truncated request"),
        ),
        Err(Error::InvalidLength)
    );
    let mut hostile_reserved = request_bytes.clone();
    *hostile_reserved.get_mut(12).expect("reserved header byte") = 1;
    assert_eq!(
        RepresentationRequestV2::decode(&hostile_reserved),
        Err(Error::NonCanonical)
    );
    let mut hostile_action = request_bytes;
    *hostile_action.get_mut(10).expect("action byte") = u8::MAX;
    assert_eq!(
        RepresentationRequestV2::decode(&hostile_action),
        Err(Error::NonCanonical)
    );
    let prepared = prepare(
        request,
        descriptor(&descriptor_bytes),
        projection,
        graph(&graph_bytes),
    )
    .expect("prepare");
    let effects: Vec<_> = prepared.token_effects().collect();
    assert_eq!(effects.len(), 3);
    assert_eq!(
        effects
            .first()
            .expect("first")
            .as_ref()
            .expect("effect")
            .style,
        TokenEffectStyleV2::TransferShardToStructured
    );
    assert_eq!(
        effects
            .get(1)
            .expect("second")
            .as_ref()
            .expect("effect")
            .amount,
        7
    );
    assert_eq!(
        effects
            .get(2)
            .expect("third")
            .as_ref()
            .expect("effect")
            .style,
        TokenEffectStyleV2::MintReceipt
    );
    assert_eq!(prepared.claims_plan(id(30), &[]), Ok(None));
    let posts = post_assets(&[(30, 6, 24), (60, 4, 56)]);
    let receipt = finalize(
        prepared,
        CompletionEvidenceV2 {
            request_digest: id(30),
            representation_program: id(31),
            claims_program: id(31),
            claims_plan_digest: [0; 32],
            claims_receipt: None,
            token_effect_digest: id(32),
            post_receipt_supply: 8,
            post_asset_observations: &posts,
            custody_request: None,
            custody_request_digest: [0; 32],
            custody_receipt: None,
            custody_receipt_digest: [0; 32],
            custody_replay_digest: [0; 32],
            post_resource_digest: id(33),
        },
    )
    .expect("finalize");
    assert_eq!(receipt.post_representation_revision(), 5);
    assert_eq!(receipt.payout(), 0);
    let bytes = receipt.to_bytes().expect("receipt bytes");
    assert_eq!(RepresentationReceiptV2::decode(&bytes), Ok(receipt));
}

#[test]
fn denomination_emits_one_canonical_claims_plan_and_shard_mint() {
    let projection_bytes = projection_fixture(3, 30, 9);
    let projection = StructuredProjectionV2::decode(&projection_bytes).expect("projection");
    let descriptor_bytes = descriptor_fixture();
    let graph_bytes = graph_fixture();
    let mut selected = assets()[0];
    selected.expected_actor_shards = 9;
    let rows = asset_bytes(&[selected]);
    let mut request_header = header(RepresentationActionV2::Denominate, 0, 1);
    request_header.quantity = 2;
    let request = RepresentationRequestV2::new(request_header, &rows).expect("request");
    let prepared = prepare(
        request,
        descriptor(&descriptor_bytes),
        projection,
        graph(&graph_bytes),
    )
    .expect("prepare");
    let effect = prepared
        .token_effects()
        .next()
        .expect("effect")
        .expect("exact");
    assert_eq!(effect.style, TokenEffectStyleV2::MintShard);
    assert_eq!(effect.amount, 20);
    let mut quantities = vec![0_u8; prepared.claims_quantity_bytes().expect("width")];
    prepared
        .write_claims_quantities(&mut quantities)
        .expect("one-hot");
    let plan = prepared
        .claims_plan(id(30), &quantities)
        .expect("claims plan")
        .expect("present");
    assert_eq!(plan.quantity(0), Ok(2));
    assert_eq!(plan.quantity(1), Ok(0));
    let claims_receipt =
        ClaimsReceiptV1::new(plan, id(34), id(31), 11, 21, 31, 0, id(35)).expect("claims receipt");
    let posts = post_assets(&[(50, 29, 21)]);
    let receipt = finalize(
        prepared,
        CompletionEvidenceV2 {
            request_digest: id(30),
            representation_program: id(31),
            claims_program: id(31),
            claims_plan_digest: id(34),
            claims_receipt: Some(claims_receipt),
            token_effect_digest: id(36),
            post_receipt_supply: 7,
            post_asset_observations: &posts,
            custody_request: None,
            custody_request_digest: [0; 32],
            custody_receipt: None,
            custody_receipt_digest: [0; 32],
            custody_replay_digest: [0; 32],
            post_resource_digest: id(37),
        },
    )
    .expect("finalize");
    assert_eq!(receipt.payout(), 0);
    receipt.verify_for(request, id(30)).expect("exact receipt");
}

#[test]
fn reconstitution_and_unwrap_are_exact_inverse_effect_shapes() {
    let graph_bytes = graph_fixture();
    let descriptor_bytes = descriptor_fixture();
    let coalesced_projection_bytes = projection_fixture(4, 40, 19);
    let coalesced_projection =
        StructuredProjectionV2::decode(&coalesced_projection_bytes).expect("projection");
    let mut selected = assets()[0];
    selected.expected_shard_supply = 40;
    selected.expected_actor_shards = 19;
    let selected_rows = asset_bytes(&[selected]);
    let request = RepresentationRequestV2::new(
        header(RepresentationActionV2::Reconstitute, 0, 1),
        &selected_rows,
    )
    .expect("reconstitution request");
    let prepared = prepare(
        request,
        descriptor(&descriptor_bytes),
        coalesced_projection,
        graph(&graph_bytes),
    )
    .expect("prepare");
    let effect = prepared
        .token_effects()
        .next()
        .expect("burn")
        .expect("exact burn");
    assert_eq!(effect.style, TokenEffectStyleV2::BurnShard);
    assert_eq!(effect.amount, 10);
    let mut quantities = vec![0_u8; prepared.claims_quantity_bytes().expect("width")];
    prepared
        .write_claims_quantities(&mut quantities)
        .expect("quantities");
    let plan = prepared
        .claims_plan(id(30), &quantities)
        .expect("plan")
        .expect("present");
    let claims_receipt =
        ClaimsReceiptV1::new(plan, id(34), id(31), 11, 31, 21, 0, id(35)).expect("claims receipt");
    let posts = post_assets(&[(30, 9, 21)]);
    finalize(
        prepared,
        CompletionEvidenceV2 {
            request_digest: id(30),
            representation_program: id(31),
            claims_program: id(31),
            claims_plan_digest: id(34),
            claims_receipt: Some(claims_receipt),
            token_effect_digest: id(36),
            post_receipt_supply: 7,
            post_asset_observations: &posts,
            custody_request: None,
            custody_request_digest: [0; 32],
            custody_receipt: None,
            custody_receipt_digest: [0; 32],
            custody_replay_digest: [0; 32],
            post_resource_digest: id(37),
        },
    )
    .expect("reconstitution finalize");

    let projection_bytes = projection_fixture(3, 30, 9);
    let projection = StructuredProjectionV2::decode(&projection_bytes).expect("projection");
    let all_rows = asset_bytes(&assets());
    let unwrap = RepresentationRequestV2::new(
        header(RepresentationActionV2::UnwrapStructured, u32::MAX, 2),
        &all_rows,
    )
    .expect("unwrap request");
    let prepared = prepare(
        unwrap,
        descriptor(&descriptor_bytes),
        projection,
        graph(&graph_bytes),
    )
    .expect("prepare unwrap");
    let effects: Vec<_> = prepared.token_effects().collect();
    assert_eq!(effects.len(), 3);
    assert_eq!(
        effects
            .first()
            .expect("receipt burn")
            .as_ref()
            .expect("effect")
            .style,
        TokenEffectStyleV2::BurnReceipt
    );
    assert_eq!(
        effects
            .get(1)
            .expect("first release")
            .as_ref()
            .expect("effect")
            .style,
        TokenEffectStyleV2::TransferShardFromStructured
    );
    let posts = post_assets(&[(30, 12, 18), (60, 18, 42)]);
    let receipt = finalize(
        prepared,
        CompletionEvidenceV2 {
            request_digest: id(30),
            representation_program: id(31),
            claims_program: id(31),
            claims_plan_digest: [0; 32],
            claims_receipt: None,
            token_effect_digest: id(36),
            post_receipt_supply: 6,
            post_asset_observations: &posts,
            custody_request: None,
            custody_request_digest: [0; 32],
            custody_receipt: None,
            custody_receipt_digest: [0; 32],
            custody_replay_digest: [0; 32],
            post_resource_digest: id(37),
        },
    )
    .expect("unwrap finalize");
    assert_eq!(receipt.payout(), 0);
}

#[test]
fn terminal_winner_burn_claims_and_custody_join_is_exact() {
    let projection_bytes = projection_fixture(4, 40, 19);
    let projection = StructuredProjectionV2::decode(&projection_bytes).expect("projection");
    let descriptor_bytes = descriptor_fixture();
    let graph_bytes = graph_fixture();
    let mut selected = assets()[0];
    selected.expected_shard_supply = 40;
    selected.expected_actor_shards = 19;
    let rows = asset_bytes(&[selected]);
    let request =
        RepresentationRequestV2::new(header(RepresentationActionV2::RedeemTerminal, 0, 1), &rows)
            .expect("request");
    let prepared = prepare(
        request,
        descriptor(&descriptor_bytes),
        projection,
        graph(&graph_bytes),
    )
    .expect("prepare");
    let mut quantities = vec![0_u8; prepared.claims_quantity_bytes().expect("width")];
    prepared
        .write_claims_quantities(&mut quantities)
        .expect("quantities");
    let plan = prepared
        .claims_plan(id(30), &quantities)
        .expect("plan")
        .expect("present");
    let claims_receipt = ClaimsReceiptV1::new(
        plan,
        id(34),
        id(31),
        11,
        31,
        NO_POSITION_REVISION,
        1,
        id(35),
    )
    .expect("claims receipt");
    let custody_request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Claims,
        source_compartment: CompartmentV1::HoardPrincipal,
        destination_compartment: CompartmentV1::External,
        release_set: id(1),
        market: id(2),
        realm: id(10),
        context: id(6),
        caller_program: id(31),
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: [0; 32],
            destination_owner: id(7),
            order: [0; 32],
            parent_request_digest: id(30),
            order_nonce: 0,
            generation: 9,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: id(70),
        destination: id(11),
        source_vault_context: id(6),
        destination_vault_context: [0; 32],
        mint: id(71),
        token_program: TOKEN_2022_PROGRAM_ID,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: 5,
        resulting_revision: 6,
        amount: 1,
        rent_lamports: 0,
    };
    let custody_evidence = ReceiptEvidenceV1 {
        source_before: 10,
        source_after: 9,
        destination_before: 0,
        destination_after: 1,
        poststate_commitment: id(72),
        replay_state_digest: id(73),
    };
    let custody_receipt =
        CustodyReceiptV1::new(custody_request, id(74), custody_evidence).expect("custody receipt");
    let posts = post_assets(&[(30, 9, 21)]);
    let receipt = finalize(
        prepared,
        CompletionEvidenceV2 {
            request_digest: id(30),
            representation_program: id(31),
            claims_program: id(31),
            claims_plan_digest: id(34),
            claims_receipt: Some(claims_receipt),
            token_effect_digest: id(36),
            post_receipt_supply: 7,
            post_asset_observations: &posts,
            custody_request: Some(custody_request),
            custody_request_digest: id(74),
            custody_receipt: Some(custody_receipt),
            custody_receipt_digest: id(75),
            custody_replay_digest: id(73),
            post_resource_digest: id(76),
        },
    )
    .expect("terminal finalize");
    assert_eq!(receipt.payout(), 1);
}

#[test]
fn hostile_partial_reconstitution_replay_and_late_token_post_refuse() {
    let projection_bytes = projection_fixture(3, 30, 9);
    let projection = StructuredProjectionV2::decode(&projection_bytes).expect("projection");
    let descriptor_bytes = descriptor_fixture();
    let graph_bytes = graph_fixture();
    let rows = asset_bytes(&[assets()[0]]);
    let request =
        RepresentationRequestV2::new(header(RepresentationActionV2::Reconstitute, 0, 1), &rows)
            .expect("request");
    assert_eq!(
        prepare(
            request,
            descriptor(&descriptor_bytes),
            projection,
            graph(&graph_bytes),
        ),
        Err(Error::InsufficientBalance)
    );

    let all_rows = asset_bytes(&assets());
    let issue = RepresentationRequestV2::new(
        header(RepresentationActionV2::IssueStructured, u32::MAX, 2),
        &all_rows,
    )
    .expect("issue");
    let mut substituted_coefficients = descriptor_fixture();
    put_u64(&mut substituted_coefficients, DESCRIPTOR_HEADER_BYTES, 4);
    assert_eq!(
        prepare(
            issue,
            descriptor(&substituted_coefficients),
            projection,
            graph(&graph_bytes),
        ),
        Err(Error::ProjectionMismatch)
    );
    let mut substituted_graph = descriptor_fixture();
    put(&mut substituted_graph, 16, &id(99));
    assert_eq!(
        prepare(
            issue,
            descriptor(&substituted_graph),
            projection,
            graph(&graph_bytes),
        ),
        Err(Error::ProjectionMismatch)
    );
    let prepared = prepare(
        issue,
        descriptor(&descriptor_bytes),
        projection,
        graph(&graph_bytes),
    )
    .expect("prepare issue");
    let hostile_posts = post_assets(&[(30, 6, 23), (60, 4, 56)]);
    assert_eq!(
        finalize(
            prepared,
            CompletionEvidenceV2 {
                request_digest: id(30),
                representation_program: id(31),
                claims_program: id(31),
                claims_plan_digest: [0; 32],
                claims_receipt: None,
                token_effect_digest: id(32),
                post_receipt_supply: 8,
                post_asset_observations: &hostile_posts,
                custody_request: None,
                custody_request_digest: [0; 32],
                custody_receipt: None,
                custody_receipt_digest: [0; 32],
                custody_replay_digest: [0; 32],
                post_resource_digest: id(33),
            }
        ),
        Err(Error::TokenMismatch)
    );

    let mut selected = assets()[0];
    selected.expected_actor_shards = 9;
    let selected_rows = asset_bytes(&[selected]);
    let denomination = RepresentationRequestV2::new(
        header(RepresentationActionV2::Denominate, 0, 1),
        &selected_rows,
    )
    .expect("denomination");
    let prepared = prepare(
        denomination,
        descriptor(&descriptor_bytes),
        projection,
        graph(&graph_bytes),
    )
    .expect("prepare");
    let mut quantities = vec![0_u8; prepared.claims_quantity_bytes().expect("width")];
    prepared
        .write_claims_quantities(&mut quantities)
        .expect("quantities");
    let substituted_plan = prepared
        .claims_plan(id(99), &quantities)
        .expect("substituted plan")
        .expect("claims active");
    let substituted_receipt =
        ClaimsReceiptV1::new(substituted_plan, id(34), id(31), 11, 21, 31, 0, id(35))
            .expect("well-formed substituted receipt");
    let denomination_posts = post_assets(&[(40, 19, 21)]);
    assert_eq!(
        finalize(
            prepared,
            CompletionEvidenceV2 {
                request_digest: id(30),
                representation_program: id(31),
                claims_program: id(31),
                claims_plan_digest: id(34),
                claims_receipt: Some(substituted_receipt),
                token_effect_digest: id(36),
                post_receipt_supply: 7,
                post_asset_observations: &denomination_posts,
                custody_request: None,
                custody_request_digest: [0; 32],
                custody_receipt: None,
                custody_receipt_digest: [0; 32],
                custody_replay_digest: [0; 32],
                post_resource_digest: id(37),
            },
        ),
        Err(Error::ClaimsMismatch)
    );
}
