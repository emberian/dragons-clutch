//! Bearer V2 basis specialization and hostile substitution corpus.

use dclutch_bearer_v2_contract::{
    BearerAssetIdentityV2, BearerBindingV2, BearerDescriptorV2, BearerResolutionV2, Error, prepare,
};
use dclutch_rational_representation_v2_contract::{
    ABSENT_REVISION, ASSET_BYTES_V2, AffineBatchContextV2, AssetV2, CallerRoleV2,
    Error as RepresentationError, RepresentationActionV2, RepresentationRequestHeaderV2,
    RepresentationRequestV2, TokenEffectStyleV2,
};
use dclutch_rational_representation_v2_kernel::{
    ContentAdmissionV2, CoordinateObservation, DESCRIPTOR_COEFFICIENT_BYTES,
    DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V3, DescriptorAdmissionV2, GRAPH_EDGE_BYTES,
    GRAPH_HEADER_BYTES, GRAPH_MAGIC_V2, GRAPH_NODE_BYTES, RepresentationDescriptorV2,
    RepresentationGraphV2, SCHEMA_VERSION_V2, STRUCTURED_HEADER_BYTES,
    StructuredProjectionHeaderV2, StructuredProjectionV2,
};
use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

const WIDTH: u32 = 3;
const SELECTED: u32 = 1;
const DENOMINATOR: u64 = 10;

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

fn descriptor_fixture(
    graph_id: [u8; 32],
    graph_digest: [u8; 32],
    root: [u8; 32],
    coefficients: [u64; WIDTH as usize],
) -> Vec<u8> {
    let mut bytes =
        vec![0_u8; DESCRIPTOR_HEADER_BYTES + WIDTH as usize * DESCRIPTOR_COEFFICIENT_BYTES];
    put(&mut bytes, 0, &DESCRIPTOR_MAGIC_V3);
    put(&mut bytes, 8, &3_u16.to_le_bytes());
    put(&mut bytes, 16, &graph_id);
    put(&mut bytes, 48, &graph_digest);
    put(&mut bytes, 80, &root);
    put(&mut bytes, 112, &id(2));
    put(&mut bytes, 144, &id(3));
    put(&mut bytes, 176, &id(4));
    put(&mut bytes, 208, &TOKEN_2022_PROGRAM_ID);
    put_u32(&mut bytes, 240, WIDTH);
    put_u64(&mut bytes, 248, DENOMINATOR);
    for (index, coefficient) in coefficients.iter().enumerate() {
        put_u64(
            &mut bytes,
            DESCRIPTOR_HEADER_BYTES + index * DESCRIPTOR_COEFFICIENT_BYTES,
            *coefficient,
        );
    }
    bytes
}

fn descriptor(bytes: &[u8]) -> RepresentationDescriptorV2<'_> {
    RepresentationDescriptorV2::decode(
        bytes,
        DescriptorAdmissionV2 {
            selected_descriptor_id: id(1),
            finalized_descriptor_id: id(1),
            recomputed_descriptor_digest: id(1),
            finalized_descriptor_digest: id(1),
            record_authenticated: true,
            derived_representation_authority: id(5),
            authority_derivation_authenticated: true,
        },
    )
    .expect("descriptor")
}

#[derive(Clone, Copy)]
struct Node {
    id: [u8; 32],
    rank: u32,
    first_edge: u32,
    edge_count: u32,
    kind: u8,
    parameter: u64,
    exposure: [u64; WIDTH as usize],
}

#[derive(Clone, Copy)]
struct Edge {
    child: [u8; 32],
    child_index: u32,
    multiplicity: u64,
}

fn graph_fixture(graph_id: [u8; 32], root: [u8; 32], nodes: &[Node], edges: &[Edge]) -> Vec<u8> {
    let mut bytes = vec![
        0_u8;
        GRAPH_HEADER_BYTES
            + nodes.len() * GRAPH_NODE_BYTES
            + edges.len() * GRAPH_EDGE_BYTES
            + nodes.len() * WIDTH as usize * 8
    ];
    put(&mut bytes, 0, &GRAPH_MAGIC_V2);
    put(&mut bytes, 8, &SCHEMA_VERSION_V2.to_le_bytes());
    put(&mut bytes, 16, &graph_id);
    put(&mut bytes, 48, &root);
    put_u32(&mut bytes, 80, WIDTH);
    put_u32(
        &mut bytes,
        84,
        u32::try_from(nodes.len()).expect("node width"),
    );
    put_u32(
        &mut bytes,
        88,
        u32::try_from(edges.len()).expect("edge width"),
    );
    put_u64(&mut bytes, 96, 100);
    for (index, node) in nodes.iter().enumerate() {
        let offset = GRAPH_HEADER_BYTES + index * GRAPH_NODE_BYTES;
        put(&mut bytes, offset, &node.id);
        put_u32(&mut bytes, offset + 32, node.rank);
        put_u32(&mut bytes, offset + 36, node.first_edge);
        put_u32(&mut bytes, offset + 40, node.edge_count);
        *bytes.get_mut(offset + 44).expect("node kind") = node.kind;
        put_u64(&mut bytes, offset + 48, node.parameter);
    }
    let edge_start = GRAPH_HEADER_BYTES + nodes.len() * GRAPH_NODE_BYTES;
    for (index, edge) in edges.iter().enumerate() {
        let offset = edge_start + index * GRAPH_EDGE_BYTES;
        put(&mut bytes, offset, &edge.child);
        put_u32(&mut bytes, offset + 32, edge.child_index);
        put_u64(&mut bytes, offset + 40, edge.multiplicity);
    }
    let exposure_start = edge_start + edges.len() * GRAPH_EDGE_BYTES;
    for (node_index, node) in nodes.iter().enumerate() {
        for (outcome, exposure) in node.exposure.iter().enumerate() {
            put_u64(
                &mut bytes,
                exposure_start + (node_index * WIDTH as usize + outcome) * 8,
                *exposure,
            );
        }
    }
    bytes
}

fn basis_graph_fixture(graph_id: [u8; 32], root: [u8; 32]) -> Vec<u8> {
    graph_fixture(
        graph_id,
        root,
        &[Node {
            id: root,
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            kind: 0,
            parameter: u64::from(SELECTED),
            exposure: [0, 100, 0],
        }],
        &[],
    )
}

fn non_basis_graph_fixture() -> Vec<u8> {
    let nodes = [
        Node {
            id: id(10),
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            kind: 0,
            parameter: 0,
            exposure: [100, 0, 0],
        },
        Node {
            id: id(11),
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            kind: 0,
            parameter: 1,
            exposure: [0, 100, 0],
        },
        Node {
            id: id(14),
            rank: 1,
            first_edge: 0,
            edge_count: 2,
            kind: 2,
            parameter: 0,
            exposure: [100, 100, 0],
        },
    ];
    let edges = [
        Edge {
            child: id(10),
            child_index: 0,
            multiplicity: 1,
        },
        Edge {
            child: id(11),
            child_index: 1,
            multiplicity: 1,
        },
    ];
    graph_fixture(id(20), id(14), &nodes, &edges)
}

fn graph<'a>(
    bytes: &'a [u8],
    graph_id: [u8; 32],
    graph_digest: [u8; 32],
) -> RepresentationGraphV2<'a> {
    RepresentationGraphV2::decode(
        bytes,
        ContentAdmissionV2 {
            selected_graph_id: graph_id,
            finalized_graph_id: graph_id,
            recomputed_graph_digest: graph_digest,
            finalized_graph_digest: graph_digest,
            record_authenticated: true,
        },
    )
    .expect("graph")
}

fn binding(graph_id: [u8; 32], graph_digest: [u8; 32], root: [u8; 32]) -> BearerBindingV2 {
    BearerBindingV2 {
        descriptor_id: id(1),
        graph_id,
        graph_digest,
        root_id: root,
        market: id(2),
        release_set: id(3),
        receipt_mint: id(4),
        token_program: TOKEN_2022_PROGRAM_ID,
        representation_authority: id(5),
        outcome_count: WIDTH,
        denominator: DENOMINATOR,
        selected_outcome: SELECTED,
    }
}

fn projection_fixture(revision: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; STRUCTURED_HEADER_BYTES + WIDTH as usize * 5 * 8];
    StructuredProjectionV2::write_header(
        &mut bytes,
        StructuredProjectionHeaderV2 {
            descriptor_id: id(1),
            market_id: id(2),
            receipt_mint: id(4),
            outcome_count: WIDTH,
            denominator: DENOMINATOR,
            receipt_supply: 0,
            revision,
        },
    )
    .expect("projection header");
    let coordinates = [
        CoordinateObservation {
            coefficient: 0,
            native_locked: 0,
            shard_supply: 0,
            structured_custody: 0,
            explicit_free_shards: 0,
        },
        CoordinateObservation {
            coefficient: DENOMINATOR,
            native_locked: 3,
            shard_supply: 30,
            structured_custody: 0,
            explicit_free_shards: 30,
        },
        CoordinateObservation {
            coefficient: 0,
            native_locked: 0,
            shard_supply: 0,
            structured_custody: 0,
            explicit_free_shards: 0,
        },
    ];
    for (outcome, coordinate) in coordinates.iter().enumerate() {
        StructuredProjectionV2::write_coordinate(
            &mut bytes,
            WIDTH,
            u32::try_from(outcome).expect("outcome"),
            *coordinate,
        )
        .expect("projection coordinate");
    }
    bytes
}

fn asset(actor_account: [u8; 32]) -> AssetV2 {
    AssetV2 {
        shard_mint: id(30),
        actor_shard_account: actor_account,
        structured_custody_account: id(32),
        claims_custody_owner: id(33),
        coefficient: DENOMINATOR,
        expected_shard_supply: 30,
        expected_actor_shards: 30,
        expected_structured_shards: 0,
    }
}

fn identity(actor_account: [u8; 32]) -> BearerAssetIdentityV2 {
    BearerAssetIdentityV2 {
        shard_mint: id(30),
        actor_shard_account: actor_account,
        structured_custody_account: id(32),
        claims_custody_owner: id(33),
    }
}

fn asset_bytes(asset: AssetV2) -> [u8; ASSET_BYTES_V2] {
    let mut bytes = [0_u8; ASSET_BYTES_V2];
    asset.encode_into(&mut bytes).expect("asset row");
    bytes
}

fn request_header(
    action: RepresentationActionV2,
    actor: [u8; 32],
) -> RepresentationRequestHeaderV2 {
    let terminal = action == RepresentationActionV2::RedeemTerminal;
    RepresentationRequestHeaderV2 {
        action,
        caller_role: CallerRoleV2::Trading,
        release_set: id(3),
        market: id(2),
        graph_id: id(20),
        descriptor_id: id(1),
        parent_context: id(6),
        actor,
        receipt_mint: id(4),
        receipt_account: [0; 32],
        representation_authority: id(5),
        token_program: TOKEN_2022_PROGRAM_ID,
        realm: if terminal { id(7) } else { [0; 32] },
        collateral_recipient: if terminal { id(8) } else { [0; 32] },
        expected_representation_revision: 9,
        expected_claims_market_revision: 10,
        expected_actor_position_revision: if terminal { ABSENT_REVISION } else { 11 },
        expected_custody_position_revision: 12,
        expected_custody_replay_revision: if terminal { 13 } else { ABSENT_REVISION },
        generation: 14,
        quantity: if terminal { 3 } else { 2 },
        denominator: DENOMINATOR,
        expected_receipt_supply: 0,
        outcome_count: WIDTH,
        selected_outcome: SELECTED,
        asset_count: 1,
    }
}

fn authenticated<'a>(descriptor_bytes: &'a [u8], graph_bytes: &'a [u8]) -> BearerDescriptorV2<'a> {
    BearerDescriptorV2::authenticate(
        descriptor(descriptor_bytes),
        graph(graph_bytes, id(20), id(21)),
        binding(id(20), id(21), id(14)),
    )
    .expect("basis descriptor")
}

#[test]
fn basis_vector_actions_conserve_exactly_without_hidden_remainder() {
    let graph_bytes = basis_graph_fixture(id(20), id(14));
    let descriptor_bytes = descriptor_fixture(id(20), id(21), id(14), [0, 10, 0]);
    let projection_bytes = projection_fixture(9);
    let projection = StructuredProjectionV2::decode(&projection_bytes).expect("projection");
    let bearer = authenticated(&descriptor_bytes, &graph_bytes);

    let denominate = bearer
        .denominate_successor(projection, 2)
        .expect("denominate");
    assert_eq!(denominate.native_locked, 5);
    assert_eq!(denominate.shard_supply, 50);
    assert_eq!(denominate.explicit_free_shards, 50);

    let reconstitute = bearer
        .reconstitute_successor(projection, 2)
        .expect("reconstitute");
    assert_eq!(reconstitute.native_locked, 1);
    assert_eq!(reconstitute.shard_supply, 10);
    assert_eq!(reconstitute.explicit_free_shards, 10);

    let coalesced = bearer.coalesce(27).expect("coalesce");
    assert_eq!(coalesced.native_claims, 2);
    assert_eq!(coalesced.change_shards, 7);
    assert_eq!(
        coalesced.input_shards,
        coalesced.native_claims * DENOMINATOR + coalesced.change_shards
    );
}

#[test]
fn all_three_actions_use_the_shared_request_and_transferable_holder_identity() {
    let graph_bytes = basis_graph_fixture(id(20), id(14));
    let descriptor_bytes = descriptor_fixture(id(20), id(21), id(14), [0, 10, 0]);
    let projection_bytes = projection_fixture(9);
    let projection = StructuredProjectionV2::decode(&projection_bytes).expect("projection");
    let bearer = authenticated(&descriptor_bytes, &graph_bytes);

    for (actor, actor_account) in [(id(40), id(41)), (id(42), id(43))] {
        for action in [
            RepresentationActionV2::Denominate,
            RepresentationActionV2::Reconstitute,
            RepresentationActionV2::RedeemTerminal,
        ] {
            let rows = asset_bytes(asset(actor_account));
            let request = RepresentationRequestV2::new(request_header(action, actor), &rows)
                .expect("shared request");
            let resolution = if action == RepresentationActionV2::RedeemTerminal {
                BearerResolutionV2::Resolved { winner: SELECTED }
            } else {
                BearerResolutionV2::Unresolved
            };
            let prepared = prepare(
                bearer,
                request,
                projection,
                identity(actor_account),
                resolution,
            )
            .expect("Bearer action");
            assert_eq!(prepared.request(), request);
            let effect = prepared
                .token_effects()
                .next()
                .expect("one token effect")
                .expect("exact token effect");
            let expected_style = if action == RepresentationActionV2::Denominate {
                TokenEffectStyleV2::MintShard
            } else {
                TokenEffectStyleV2::BurnShard
            };
            assert_eq!(effect.style, expected_style);
            assert_eq!(effect.amount, request.header().quantity * DENOMINATOR);
            assert!(prepared.token_effects().nth(1).is_none());
            if action == RepresentationActionV2::RedeemTerminal {
                assert_eq!(
                    prepared.affine_packet_bytes(),
                    Err(RepresentationError::InvalidActionShape)
                );
            } else {
                let mut packet =
                    vec![0_u8; prepared.affine_packet_bytes().expect("affine packet width")];
                let plan = prepared
                    .write_affine_packet(
                        id(50),
                        Some(AffineBatchContextV2 {
                            product_record_digest: id(51),
                            semantic_basis_id: id(52),
                            linked_basis_record_digest: id(53),
                        }),
                        &mut packet,
                    )
                    .expect("canonical affine packet")
                    .expect("open Bearer action has Claims effect");
                assert_eq!(plan.outcome_count(), WIDTH);
                assert_eq!(plan.position_count(), 2);
                assert_eq!(plan.row_count(), 1);
                let row = plan.row(0).expect("selected affine row");
                assert_eq!(row.outcome(), SELECTED);
                assert_eq!(row.source_delta().magnitude(), request.header().quantity);
                assert_eq!(
                    row.destination_delta().magnitude(),
                    request.header().quantity
                );
            }
        }
    }
}

#[test]
fn same_width_recipe_graph_digest_and_root_substitutions_refuse() {
    let graph_bytes = basis_graph_fixture(id(20), id(14));
    let descriptor_bytes = descriptor_fixture(id(20), id(21), id(14), [0, 10, 0]);

    let non_basis_graph = non_basis_graph_fixture();
    let non_basis_descriptor = descriptor_fixture(id(20), id(21), id(14), [10, 10, 0]);
    assert_eq!(
        BearerDescriptorV2::authenticate(
            descriptor(&non_basis_descriptor),
            graph(&non_basis_graph, id(20), id(21)),
            binding(id(20), id(21), id(14)),
        ),
        Err(Error::NotBasisVector)
    );

    let substituted_id_graph = basis_graph_fixture(id(22), id(14));
    assert_eq!(
        BearerDescriptorV2::authenticate(
            descriptor(&descriptor_bytes),
            graph(&substituted_id_graph, id(22), id(21)),
            binding(id(20), id(21), id(14)),
        ),
        Err(Error::GraphMismatch)
    );
    assert_eq!(
        BearerDescriptorV2::authenticate(
            descriptor(&descriptor_bytes),
            graph(&graph_bytes, id(20), id(23)),
            binding(id(20), id(21), id(14)),
        ),
        Err(Error::GraphMismatch)
    );
    let substituted_root_graph = basis_graph_fixture(id(20), id(15));
    assert_eq!(
        BearerDescriptorV2::authenticate(
            descriptor(&descriptor_bytes),
            graph(&substituted_root_graph, id(20), id(21)),
            binding(id(20), id(21), id(14)),
        ),
        Err(Error::GraphMismatch)
    );
}

#[test]
fn terminal_winner_replay_release_and_asset_substitutions_refuse() {
    let graph_bytes = basis_graph_fixture(id(20), id(14));
    let descriptor_bytes = descriptor_fixture(id(20), id(21), id(14), [0, 10, 0]);
    let projection_bytes = projection_fixture(9);
    let projection = StructuredProjectionV2::decode(&projection_bytes).expect("projection");
    let bearer = authenticated(&descriptor_bytes, &graph_bytes);
    let rows = asset_bytes(asset(id(41)));
    let terminal = RepresentationRequestV2::new(
        request_header(RepresentationActionV2::RedeemTerminal, id(40)),
        &rows,
    )
    .expect("terminal request");

    for hostile in [
        BearerResolutionV2::Unresolved,
        BearerResolutionV2::Resolved { winner: 0 },
        BearerResolutionV2::Resolved { winner: 2 },
    ] {
        assert_eq!(
            prepare(bearer, terminal, projection, identity(id(41)), hostile),
            Err(Error::TerminalMismatch)
        );
    }

    let mut replay_header = request_header(RepresentationActionV2::RedeemTerminal, id(40));
    replay_header.expected_representation_revision = 8;
    let replay_request =
        RepresentationRequestV2::new(replay_header, &rows).expect("replay request");
    assert_eq!(
        prepare(
            bearer,
            replay_request,
            projection,
            identity(id(41)),
            BearerResolutionV2::Resolved { winner: SELECTED },
        ),
        Err(Error::Representation(
            RepresentationError::ProjectionMismatch
        ))
    );

    let mut release_header = request_header(RepresentationActionV2::Denominate, id(40));
    release_header.release_set = id(44);
    let release_request =
        RepresentationRequestV2::new(release_header, &rows).expect("release request");
    assert_eq!(
        prepare(
            bearer,
            release_request,
            projection,
            identity(id(41)),
            BearerResolutionV2::Unresolved,
        ),
        Err(Error::Representation(
            RepresentationError::ProjectionMismatch
        ))
    );

    let mut substituted_identity = identity(id(41));
    substituted_identity.shard_mint = id(45);
    assert_eq!(
        prepare(
            bearer,
            terminal,
            projection,
            substituted_identity,
            BearerResolutionV2::Resolved { winner: SELECTED },
        ),
        Err(Error::AssetMismatch)
    );

    let mut release_binding = binding(id(20), id(21), id(14));
    release_binding.release_set = id(44);
    assert_eq!(
        BearerDescriptorV2::authenticate(
            descriptor(&descriptor_bytes),
            graph(&graph_bytes, id(20), id(21)),
            release_binding,
        ),
        Err(Error::BindingMismatch)
    );
}
