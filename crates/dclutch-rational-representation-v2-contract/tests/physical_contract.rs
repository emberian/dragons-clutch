//! Hostile and positive physical Claims/Token/Custody composition corpus.

use dclutch_claims_svm::affine_batch_v2::{AffineBatchReceiptV2, DeltaDirectionV2};
use dclutch_claims_svm::lbv2_terminal_v2::{
    LBV2_TERMINAL_ABSENT_CUSTODY_REVISION_V2, Lbv2TerminalRedeemReceiptV2,
    Lbv2TerminalRedeemRequestInputV2, Lbv2TerminalRedeemRequestV2,
};
use dclutch_rational_representation_v2_contract::{
    ABSENT_REVISION, ASSET_BYTES_V2, AffineBatchContextV2, AssetV2, CallerRoleV2,
    CompletionEvidenceV2, Error, RATIONAL_ASSET_ACCOUNT_COUNT_V2, RATIONAL_BASE_ACCOUNT_COUNT_V2,
    RATIONAL_TERMINAL_ACCOUNT_COUNT_V2, REQUEST_HEADER_BYTES_V2, RepresentationActionV2,
    RepresentationReceiptV2, RepresentationRequestHeaderV2, RepresentationRequestV2,
    TokenEffectStyleV2, finalize, prepare,
};
use dclutch_rational_representation_v2_kernel::{
    ContentAdmissionV2, DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V3,
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
    put(&mut bytes, 0, &DESCRIPTOR_MAGIC_V3);
    put(&mut bytes, 8, &3_u16.to_le_bytes());
    put(&mut bytes, 16, &id(3));
    put(&mut bytes, 48, &id(40));
    put(&mut bytes, 80, &id(14));
    put(&mut bytes, 112, &id(2));
    put(&mut bytes, 144, &id(1));
    put(&mut bytes, 176, &id(5));
    put(&mut bytes, 208, &TOKEN_2022_PROGRAM_ID);
    put_u32(&mut bytes, 240, 2);
    put_u64(&mut bytes, 248, 10);
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
            derived_representation_authority: id(9),
            authority_derivation_authenticated: true,
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
    let context = AffineBatchContextV2 {
        product_record_digest: id(80),
        semantic_basis_id: id(81),
        linked_basis_record_digest: id(82),
    };
    assert_eq!(
        prepared.write_affine_packet(id(30), None, &mut []),
        Ok(None)
    );
    assert_eq!(
        prepared.write_affine_packet(id(30), Some(context), &mut []),
        Err(Error::InvalidActionShape)
    );
    let posts = post_assets(&[(30, 6, 24), (60, 4, 56)]);
    let receipt = finalize(
        prepared,
        CompletionEvidenceV2 {
            request_digest: id(30),
            representation_program: id(31),
            claims_program: id(31),
            affine_packet_digest: [0; 32],
            affine_packet: None,
            affine_context: None,
            affine_receipt: None,
            terminal_request: None,
            terminal_request_digest: [0; 32],
            terminal_receipt: None,
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
fn denomination_emits_one_canonical_affine_packet_and_shard_mint() {
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
    let context = AffineBatchContextV2 {
        product_record_digest: id(80),
        semantic_basis_id: id(81),
        linked_basis_record_digest: id(82),
    };
    let mut hostile_context_bytes =
        vec![0_u8; prepared.affine_packet_bytes().expect("packet width")];
    assert_eq!(
        prepared.write_affine_packet(
            id(30),
            Some(AffineBatchContextV2 {
                product_record_digest: [0; 32],
                ..context
            }),
            &mut hostile_context_bytes,
        ),
        Err(Error::ClaimsMismatch)
    );
    let mut packet_bytes = vec![0_u8; prepared.affine_packet_bytes().expect("packet width")];
    let packet = prepared
        .write_affine_packet(id(30), Some(context), &mut packet_bytes)
        .expect("affine packet")
        .expect("present");
    assert_eq!(packet.position_count(), 2);
    assert_eq!(packet.row_count(), 1);
    assert_eq!(packet.position(0).expect("actor").owner(), id(7));
    assert_eq!(packet.position(1).expect("custody").owner(), id(53));
    let row = packet.row(0).expect("row");
    assert_eq!(row.outcome(), 0);
    assert_eq!(row.source_position_index(), 0);
    assert_eq!(row.destination_position_index(), 1);
    assert_eq!(row.aggregate_delta().direction(), DeltaDirectionV2::Neutral);
    assert_eq!(row.source_delta().direction(), DeltaDirectionV2::Debit);
    assert_eq!(row.source_delta().magnitude(), 2);
    assert_eq!(
        row.destination_delta().direction(),
        DeltaDirectionV2::Credit
    );
    assert_eq!(row.destination_delta().magnitude(), 2);
    let affine_receipt = AffineBatchReceiptV2::new(packet, id(34), id(35), id(31), id(36), 11)
        .expect("affine receipt");
    let posts = post_assets(&[(50, 29, 21)]);
    let receipt = finalize(
        prepared,
        CompletionEvidenceV2 {
            request_digest: id(30),
            representation_program: id(31),
            claims_program: id(31),
            affine_packet_digest: id(34),
            affine_packet: Some(packet),
            affine_context: Some(context),
            affine_receipt: Some(affine_receipt),
            terminal_request: None,
            terminal_request_digest: [0; 32],
            terminal_receipt: None,
            token_effect_digest: id(37),
            post_receipt_supply: 7,
            post_asset_observations: &posts,
            custody_request: None,
            custody_request_digest: [0; 32],
            custody_receipt: None,
            custody_receipt_digest: [0; 32],
            custody_replay_digest: [0; 32],
            post_resource_digest: id(38),
        },
    )
    .expect("finalize");
    assert_eq!(receipt.payout(), 0);
    assert_eq!(receipt.affine_packet_digest(), id(34));
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
    assert_eq!(
        effect.authority,
        id(7),
        "shard burn authority is the observed Token holder, not the mint authority"
    );
    assert_eq!(effect.amount, 10);
    let context = AffineBatchContextV2 {
        product_record_digest: id(80),
        semantic_basis_id: id(81),
        linked_basis_record_digest: id(82),
    };
    let mut packet_bytes = vec![0_u8; prepared.affine_packet_bytes().expect("packet width")];
    let packet = prepared
        .write_affine_packet(id(30), Some(context), &mut packet_bytes)
        .expect("affine packet")
        .expect("present");
    let row = packet.row(0).expect("row");
    assert_eq!(row.source_position_index(), 1);
    assert_eq!(row.destination_position_index(), 0);
    let affine_receipt = AffineBatchReceiptV2::new(packet, id(34), id(35), id(31), id(36), 11)
        .expect("affine receipt");
    let posts = post_assets(&[(30, 9, 21)]);
    finalize(
        prepared,
        CompletionEvidenceV2 {
            request_digest: id(30),
            representation_program: id(31),
            claims_program: id(31),
            affine_packet_digest: id(34),
            affine_packet: Some(packet),
            affine_context: Some(context),
            affine_receipt: Some(affine_receipt),
            terminal_request: None,
            terminal_request_digest: [0; 32],
            terminal_receipt: None,
            token_effect_digest: id(37),
            post_receipt_supply: 7,
            post_asset_observations: &posts,
            custody_request: None,
            custody_request_digest: [0; 32],
            custody_receipt: None,
            custody_receipt_digest: [0; 32],
            custody_replay_digest: [0; 32],
            post_resource_digest: id(38),
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
            .first()
            .expect("receipt burn")
            .as_ref()
            .expect("effect")
            .authority,
        id(7),
        "receipt burn authority is the observed Token holder, not the mint authority"
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
            affine_packet_digest: [0; 32],
            affine_packet: None,
            affine_context: None,
            affine_receipt: None,
            terminal_request: None,
            terminal_request_digest: [0; 32],
            terminal_receipt: None,
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
fn terminal_requires_typed_lbv2_evidence_and_accepts_exact_zero_payout() {
    let projection_bytes = projection_fixture(4, 40, 19);
    let projection = StructuredProjectionV2::decode(&projection_bytes).expect("projection");
    let descriptor_bytes = descriptor_fixture();
    let graph_bytes = graph_fixture();
    let mut selected = assets()[0];
    selected.expected_shard_supply = 40;
    selected.expected_actor_shards = 19;
    let rows = asset_bytes(&[selected]);
    let mut request_header = header(RepresentationActionV2::RedeemTerminal, 0, 1);
    request_header.expected_custody_replay_revision = ABSENT_REVISION;
    let request =
        RepresentationRequestV2::new(request_header, &rows).expect("request remains decodable");
    let prepared = prepare(
        request,
        descriptor(&descriptor_bytes),
        projection,
        graph(&graph_bytes),
    )
    .expect("prepare");
    assert_eq!(
        prepared.affine_packet_bytes(),
        Err(Error::InvalidActionShape)
    );
    let posts = post_assets(&[(30, 9, 21)]);
    assert_eq!(
        finalize(
            prepared,
            CompletionEvidenceV2 {
                request_digest: id(30),
                representation_program: id(31),
                claims_program: id(31),
                affine_packet_digest: [0; 32],
                affine_packet: None,
                affine_context: None,
                affine_receipt: None,
                terminal_request: None,
                terminal_request_digest: [0; 32],
                terminal_receipt: None,
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
        ),
        Err(Error::ClaimsMismatch)
    );
    let terminal_request = Lbv2TerminalRedeemRequestV2::new(Lbv2TerminalRedeemRequestInputV2 {
        release_set: id(1),
        market: id(2),
        product_record_digest: id(80),
        semantic_product_id: id(81),
        semantic_basis_id: id(82),
        linked_basis_record_digest: id(83),
        terminal_coordinate_digest: id(84),
        owner: id(53),
        protocol_position: id(85),
        claims_program: id(31),
        custody_request_digest: [0; 32],
        candidate_digest: id(86),
        terminal_numerator: 0,
        terminal_denominator: 1,
        claim_index: 0,
        pre_market_revision: 10,
        post_market_revision: 11,
        pre_position_revision: 30,
        post_position_revision: 31,
        debit_quantity: 1,
        evaluated_payout: 0,
        pre_custody_revision: LBV2_TERMINAL_ABSENT_CUSTODY_REVISION_V2,
        post_custody_revision: LBV2_TERMINAL_ABSENT_CUSTODY_REVISION_V2,
    })
    .expect("typed terminal request");
    let terminal_request_digest = id(87);
    let terminal_receipt = Lbv2TerminalRedeemReceiptV2::new(
        terminal_request,
        terminal_request_digest,
        [0; 32],
        [0; 32],
        id(88),
    )
    .expect("typed terminal receipt");
    let receipt = finalize(
        prepared,
        CompletionEvidenceV2 {
            request_digest: id(30),
            representation_program: id(31),
            claims_program: id(31),
            affine_packet_digest: [0; 32],
            affine_packet: None,
            affine_context: None,
            affine_receipt: None,
            terminal_request: Some(&terminal_request),
            terminal_request_digest,
            terminal_receipt: Some(&terminal_receipt),
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
    .expect("typed terminal completion");
    assert_eq!(receipt.payout(), 0);
    assert_eq!(receipt.post_claims_market_revision(), 11);
    assert_eq!(receipt.post_custody_position_revision(), 31);
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
                affine_packet_digest: [0; 32],
                affine_packet: None,
                affine_context: None,
                affine_receipt: None,
                terminal_request: None,
                terminal_request_digest: [0; 32],
                terminal_receipt: None,
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
    let context = AffineBatchContextV2 {
        product_record_digest: id(80),
        semantic_basis_id: id(81),
        linked_basis_record_digest: id(82),
    };
    let mut packet_bytes = vec![0_u8; prepared.affine_packet_bytes().expect("packet width")];
    let substituted_packet = prepared
        .write_affine_packet(id(99), Some(context), &mut packet_bytes)
        .expect("substituted packet")
        .expect("affine active");
    let substituted_receipt =
        AffineBatchReceiptV2::new(substituted_packet, id(34), id(35), id(31), id(36), 11)
            .expect("well-formed substituted receipt");
    let denomination_posts = post_assets(&[(40, 19, 21)]);
    assert_eq!(
        finalize(
            prepared,
            CompletionEvidenceV2 {
                request_digest: id(30),
                representation_program: id(31),
                claims_program: id(31),
                affine_packet_digest: id(34),
                affine_packet: Some(substituted_packet),
                affine_context: Some(context),
                affine_receipt: Some(substituted_receipt),
                terminal_request: None,
                terminal_request_digest: [0; 32],
                terminal_receipt: None,
                token_effect_digest: id(37),
                post_receipt_supply: 7,
                post_asset_observations: &denomination_posts,
                custody_request: None,
                custody_request_digest: [0; 32],
                custody_receipt: None,
                custody_receipt_digest: [0; 32],
                custody_replay_digest: [0; 32],
                post_resource_digest: id(38),
            },
        ),
        Err(Error::ClaimsMismatch)
    );

    let mut canonical_packet_bytes =
        vec![0_u8; prepared.affine_packet_bytes().expect("packet width")];
    let canonical_packet = prepared
        .write_affine_packet(id(30), Some(context), &mut canonical_packet_bytes)
        .expect("canonical packet")
        .expect("affine active");
    let canonical_receipt =
        AffineBatchReceiptV2::new(canonical_packet, id(34), id(35), id(31), id(36), 11)
            .expect("canonical receipt");
    let substituted_context = AffineBatchContextV2 {
        product_record_digest: id(83),
        ..context
    };
    assert_eq!(
        finalize(
            prepared,
            CompletionEvidenceV2 {
                request_digest: id(30),
                representation_program: id(31),
                claims_program: id(31),
                affine_packet_digest: id(34),
                affine_packet: Some(canonical_packet),
                affine_context: Some(substituted_context),
                affine_receipt: Some(canonical_receipt),
                terminal_request: None,
                terminal_request_digest: [0; 32],
                terminal_receipt: None,
                token_effect_digest: id(37),
                post_receipt_supply: 7,
                post_asset_observations: &denomination_posts,
                custody_request: None,
                custody_request_digest: [0; 32],
                custody_receipt: None,
                custody_receipt_digest: [0; 32],
                custody_replay_digest: [0; 32],
                post_resource_digest: id(38),
            },
        ),
        Err(Error::ClaimsMismatch)
    );
}

#[test]
fn physical_account_width_is_sparse_for_selected_actions() {
    let selected_rows = asset_bytes(&[assets()[0]]);
    let denominate = RepresentationRequestV2::new(
        header(RepresentationActionV2::Denominate, 0, 1),
        &selected_rows,
    )
    .expect("denominate");
    assert_eq!(
        denominate.physical_account_count(),
        Ok(RATIONAL_BASE_ACCOUNT_COUNT_V2 + RATIONAL_ASSET_ACCOUNT_COUNT_V2)
    );

    let terminal = RepresentationRequestV2::new(
        header(RepresentationActionV2::RedeemTerminal, 0, 1),
        &selected_rows,
    )
    .expect("terminal");
    assert_eq!(
        terminal.physical_account_count(),
        Ok(RATIONAL_BASE_ACCOUNT_COUNT_V2
            + RATIONAL_ASSET_ACCOUNT_COUNT_V2
            + RATIONAL_TERMINAL_ACCOUNT_COUNT_V2)
    );

    let all_rows = asset_bytes(&assets());
    let issue = RepresentationRequestV2::new(
        header(RepresentationActionV2::IssueStructured, u32::MAX, 2),
        &all_rows,
    )
    .expect("issue");
    assert_eq!(
        issue.physical_account_count(),
        Ok(RATIONAL_BASE_ACCOUNT_COUNT_V2 + 2 * RATIONAL_ASSET_ACCOUNT_COUNT_V2)
    );
}
