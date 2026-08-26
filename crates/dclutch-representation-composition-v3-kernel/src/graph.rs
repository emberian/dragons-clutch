//! Canonical bounded DAG encoding and direct flattening validation.

use crate::CompositionDescriptorV3;
use crate::abi::{
    COMPOSITION_GRAPH_HEADER_BYTES_V3, COMPOSITION_GRAPH_MAGIC_V3, COMPOSITION_SCHEMA_VERSION_V3,
    Error, MAX_COMPOSITION_EDGES_V3, MAX_COMPOSITION_NODES_V3, MAX_COMPOSITION_OUTCOMES_V3,
    MAX_COMPOSITION_TERMS_V3, MIN_COMPOSITION_OUTCOMES_V3, RecordAdmissionV3, Result, array_at,
    gcd_u128, nonzero_array, put, require_zero, slice, u16_at, u32_at, u64_at,
    validate_record_admission,
};

/// Exact fixed width of one canonical graph node.
pub const COMPOSITION_NODE_BYTES_V3: usize = 80;
/// Exact fixed width of one canonical graph edge.
pub const COMPOSITION_EDGE_BYTES_V3: usize = 48;
/// Exact fixed width of one canonical sparse payoff term.
pub const COMPOSITION_TERM_BYTES_V3: usize = 16;

/// Graph-header byte-layout authority.
pub struct GraphLayoutV3;

impl GraphLayoutV3 {
    /// Magic offset.
    pub const MAGIC: usize = 0;
    /// Schema-version offset.
    pub const VERSION: usize = 8;
    /// Reserved header offset.
    pub const RESERVED_HEADER: usize = 10;
    /// Stable graph identity offset.
    pub const GRAPH_ID: usize = 16;
    /// Sole canonical root identity offset.
    pub const ROOT_ID: usize = 48;
    /// Exhaustive native width offset.
    pub const OUTCOME_COUNT: usize = 80;
    /// Node-count offset.
    pub const NODE_COUNT: usize = 84;
    /// Edge-count offset.
    pub const EDGE_COUNT: usize = 88;
    /// Total sparse-term count offset.
    pub const TERM_COUNT: usize = 92;
    /// Sole root table index offset.
    pub const ROOT_INDEX: usize = 96;
    /// Reserved tail offset.
    pub const RESERVED_TAIL: usize = 100;
}

/// Node-record byte-layout authority.
pub struct NodeLayoutV3;

impl NodeLayoutV3 {
    /// Content identity offset.
    pub const ID: usize = 0;
    /// Exact topological rank offset.
    pub const RANK: usize = 32;
    /// First owned edge offset.
    pub const FIRST_EDGE: usize = 36;
    /// Owned edge-count offset.
    pub const EDGE_COUNT: usize = 40;
    /// First owned flattened term offset.
    pub const FIRST_TERM: usize = 44;
    /// Owned flattened term-count offset.
    pub const TERM_COUNT: usize = 48;
    /// Node-kind offset.
    pub const KIND: usize = 52;
    /// Reserved kind padding offset.
    pub const RESERVED_KIND: usize = 53;
    /// Native outcome offset; zero for composed nodes.
    pub const NATIVE_OUTCOME: usize = 56;
    /// Reserved scalar padding offset.
    pub const RESERVED_SCALAR: usize = 60;
    /// Exact recipe divisor offset.
    pub const RECIPE_DIVISOR: usize = 64;
    /// Canonical flattened common denominator offset.
    pub const FLATTENED_DENOMINATOR: usize = 72;
}

/// Edge-record byte-layout authority.
pub struct EdgeLayoutV3;

impl EdgeLayoutV3 {
    /// Child content identity offset.
    pub const CHILD_ID: usize = 0;
    /// Child node-table index offset.
    pub const CHILD_INDEX: usize = 32;
    /// Reserved offset.
    pub const RESERVED: usize = 36;
    /// Nonzero exact coefficient offset.
    pub const COEFFICIENT: usize = 40;
}

/// Sparse-term byte-layout authority.
pub struct TermLayoutV3;

impl TermLayoutV3 {
    /// Native outcome offset.
    pub const OUTCOME: usize = 0;
    /// Reserved offset.
    pub const RESERVED: usize = 4;
    /// Positive canonical numerator offset.
    pub const NUMERATOR: usize = 8;
}

/// Canonical graph node kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CompositionNodeKindV3 {
    /// One unit of one exhaustive native outcome.
    Native = 0,
    /// A nonnegative exact rational composition of earlier nodes.
    Compose = 1,
}

impl CompositionNodeKindV3 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Native),
            1 => Ok(Self::Compose),
            _ => Err(Error::NonCanonical),
        }
    }
}

/// One canonical sparse native payoff term.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SparseTermV3 {
    /// Exhaustive native outcome coordinate.
    pub outcome: u32,
    /// Positive numerator over the node common denominator.
    pub numerator: u64,
}

/// One node supplied to the atomic canonical encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionNodeInputV3 {
    /// Stable content identity.
    pub id: [u8; 32],
    /// Exact longest-path rank from a native leaf.
    pub rank: u32,
    /// First edge owned by this node.
    pub first_edge: u32,
    /// Number of edges owned by this node.
    pub edge_count: u32,
    /// First flattened term owned by this node.
    pub first_term: u32,
    /// Number of flattened terms owned by this node.
    pub term_count: u32,
    /// Node kind.
    pub kind: CompositionNodeKindV3,
    /// Native outcome for [`CompositionNodeKindV3::Native`], otherwise zero.
    pub native_outcome: u32,
    /// Positive divisor applied after the weighted child sum.
    pub recipe_divisor: u64,
    /// GCD-normalized flattened common denominator.
    pub flattened_denominator: u64,
}

/// One edge supplied to the atomic canonical encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionEdgeInputV3 {
    /// Exact child content identity.
    pub child_id: [u8; 32],
    /// Exact earlier child table index.
    pub child_index: u32,
    /// Positive exact integer coefficient.
    pub coefficient: u64,
}

/// Borrowed complete graph supplied to the atomic encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionGraphInputV3<'a> {
    /// Stable graph identity.
    pub graph_id: [u8; 32],
    /// Sole root node identity.
    pub root_id: [u8; 32],
    /// Exhaustive native width.
    pub outcome_count: u32,
    /// Canonically ordered nodes.
    pub nodes: &'a [CompositionNodeInputV3],
    /// Canonically ordered, node-owned edges.
    pub edges: &'a [CompositionEdgeInputV3],
    /// Canonically ordered, node-owned flattened terms.
    pub terms: &'a [SparseTermV3],
}

#[derive(Clone, Copy)]
struct NodeV3 {
    id: [u8; 32],
    rank: u32,
    first_edge: u32,
    edge_count: u32,
    first_term: u32,
    term_count: u32,
    kind: CompositionNodeKindV3,
    native_outcome: u32,
    recipe_divisor: u64,
    flattened_denominator: u64,
}

#[derive(Clone, Copy)]
struct EdgeV3 {
    child_id: [u8; 32],
    child_index: u32,
    coefficient: u64,
}

/// Hostile-decoded canonical composition graph.
#[derive(Clone, Copy)]
pub struct CompositionGraphV3<'a> {
    bytes: &'a [u8],
    graph_id: [u8; 32],
    root_id: [u8; 32],
    outcome_count: u32,
    node_count: u32,
    edge_count: u32,
    term_count: u32,
    root_index: u32,
}

impl<'a> CompositionGraphV3<'a> {
    /// Decode and independently validate an admitted graph against its descriptor.
    pub fn decode(
        input: &'a [u8],
        descriptor: CompositionDescriptorV3,
        admission: RecordAdmissionV3,
    ) -> Result<Self> {
        validate_record_admission(admission, descriptor.graph_id(), descriptor.graph_digest())?;
        let graph = Self::decode_structural(input)?;
        if graph.graph_id != descriptor.graph_id()
            || graph.root_id != descriptor.root_id()
            || graph.outcome_count != descriptor.outcome_count()
            || graph.node_count != descriptor.node_count()
            || graph.edge_count != descriptor.edge_count()
            || graph.term_count != descriptor.term_count()
            || graph.root()?.flattened_denominator != descriptor.root_denominator()
        {
            return Err(Error::CompositionMismatch);
        }
        Ok(graph)
    }

    fn decode_structural(input: &'a [u8]) -> Result<Self> {
        if input.len() < COMPOSITION_GRAPH_HEADER_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if array_at::<8>(input, GraphLayoutV3::MAGIC)? != COMPOSITION_GRAPH_MAGIC_V3 {
            return Err(Error::InvalidMagic);
        }
        if u16_at(input, GraphLayoutV3::VERSION)? != COMPOSITION_SCHEMA_VERSION_V3 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, GraphLayoutV3::RESERVED_HEADER, 6)?;
        require_zero(input, GraphLayoutV3::RESERVED_TAIL, 12)?;
        let graph = Self {
            bytes: input,
            graph_id: nonzero_array(input, GraphLayoutV3::GRAPH_ID)?,
            root_id: nonzero_array(input, GraphLayoutV3::ROOT_ID)?,
            outcome_count: u32_at(input, GraphLayoutV3::OUTCOME_COUNT)?,
            node_count: u32_at(input, GraphLayoutV3::NODE_COUNT)?,
            edge_count: u32_at(input, GraphLayoutV3::EDGE_COUNT)?,
            term_count: u32_at(input, GraphLayoutV3::TERM_COUNT)?,
            root_index: u32_at(input, GraphLayoutV3::ROOT_INDEX)?,
        };
        graph.validate_header()?;
        graph.validate_nodes()?;
        Ok(graph)
    }

    /// Exact admitted graph bytes.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Stable graph identity.
    pub const fn graph_id(self) -> [u8; 32] {
        self.graph_id
    }

    /// Sole canonical root identity.
    pub const fn root_id(self) -> [u8; 32] {
        self.root_id
    }

    /// Exhaustive native outcome width.
    pub const fn outcome_count(self) -> u32 {
        self.outcome_count
    }

    /// Exact node count.
    pub const fn node_count(self) -> u32 {
        self.node_count
    }

    /// Exact edge count.
    pub const fn edge_count(self) -> u32 {
        self.edge_count
    }

    /// Exact total sparse-term count.
    pub const fn term_count(self) -> u32 {
        self.term_count
    }

    /// Root flattened denominator.
    pub fn root_denominator(self) -> Result<u64> {
        Ok(self.root()?.flattened_denominator)
    }

    /// Exact byte segment of the root's canonical sparse payoff.
    pub fn root_term_bytes(self) -> Result<&'a [u8]> {
        let root = self.root()?;
        let start = self.term_offset(root.first_term)?;
        let length = usize_from_u32(root.term_count)?
            .checked_mul(COMPOSITION_TERM_BYTES_V3)
            .ok_or(Error::InvalidLength)?;
        slice(self.bytes, start, length)
    }

    /// Root sparse term count.
    pub fn root_term_count(self) -> Result<u32> {
        Ok(self.root()?.term_count)
    }

    /// Read one root sparse term.
    pub fn root_term(self, index: u32) -> Result<SparseTermV3> {
        let root = self.root()?;
        if index >= root.term_count {
            return Err(Error::InvalidLength);
        }
        self.term(
            root.first_term
                .checked_add(index)
                .ok_or(Error::ArithmeticOverflow)?,
        )
    }

    fn validate_header(self) -> Result<()> {
        if self.outcome_count < MIN_COMPOSITION_OUTCOMES_V3
            || self.outcome_count > MAX_COMPOSITION_OUTCOMES_V3
        {
            return Err(Error::InvalidOutcome);
        }
        if self.node_count == 0
            || self.node_count > MAX_COMPOSITION_NODES_V3
            || self.edge_count > MAX_COMPOSITION_EDGES_V3
            || self.term_count == 0
            || self.term_count > MAX_COMPOSITION_TERMS_V3
        {
            return Err(Error::CapacityExceeded);
        }
        if self.root_index != self.node_count.checked_sub(1).ok_or(Error::InvalidNode)? {
            return Err(Error::AmbiguousRoot);
        }
        if self.bytes.len()
            != composition_graph_bytes_v3(self.node_count, self.edge_count, self.term_count)?
        {
            return Err(Error::InvalidLength);
        }
        Ok(())
    }

    fn validate_nodes(self) -> Result<()> {
        let mut edge_cursor = 0_u32;
        let mut term_cursor = 0_u32;
        let mut index = 0_u32;
        while index < self.node_count {
            let node = self.node(index)?;
            if node.first_edge != edge_cursor || node.first_term != term_cursor {
                return Err(Error::NonCanonical);
            }
            edge_cursor = edge_cursor
                .checked_add(node.edge_count)
                .ok_or(Error::ArithmeticOverflow)?;
            term_cursor = term_cursor
                .checked_add(node.term_count)
                .ok_or(Error::ArithmeticOverflow)?;
            if edge_cursor > self.edge_count || term_cursor > self.term_count {
                return Err(Error::InvalidLength);
            }
            self.validate_node_order(index, node)?;
            self.validate_node(index, node)?;
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if edge_cursor != self.edge_count || term_cursor != self.term_count {
            return Err(Error::InvalidLength);
        }
        let root = self.root()?;
        if root.id != self.root_id {
            return Err(Error::AmbiguousRoot);
        }
        self.validate_root_reachability()
    }

    fn validate_node_order(self, index: u32, node: NodeV3) -> Result<()> {
        if index == 0 {
            return Ok(());
        }
        let mut prior_index = 0_u32;
        while prior_index < index {
            if self.node(prior_index)?.id == node.id {
                return Err(Error::DuplicateOrUnorderedNode);
            }
            prior_index = prior_index
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        let prior = self.node(index.checked_sub(1).ok_or(Error::ArithmeticOverflow)?)?;
        if node.rank < prior.rank || (node.rank == prior.rank && node.id <= prior.id) {
            return Err(Error::DuplicateOrUnorderedNode);
        }
        Ok(())
    }

    fn validate_node(self, index: u32, node: NodeV3) -> Result<()> {
        if node.term_count == 0 || node.flattened_denominator == 0 || node.recipe_divisor == 0 {
            return Err(Error::InvalidNode);
        }
        self.validate_sparse_terms(node)?;
        match node.kind {
            CompositionNodeKindV3::Native => self.validate_native(index, node),
            CompositionNodeKindV3::Compose => self.validate_composed(index, node),
        }
    }

    fn validate_sparse_terms(self, node: NodeV3) -> Result<()> {
        let mut index = 0_u32;
        let mut prior_outcome = None;
        let mut normalization = u128::from(node.flattened_denominator);
        while index < node.term_count {
            let term = self.term(
                node.first_term
                    .checked_add(index)
                    .ok_or(Error::ArithmeticOverflow)?,
            )?;
            if term.outcome >= self.outcome_count
                || term.numerator == 0
                || prior_outcome.is_some_and(|prior| term.outcome <= prior)
            {
                return Err(Error::NonCanonicalPayoff);
            }
            normalization = gcd_u128(normalization, u128::from(term.numerator));
            prior_outcome = Some(term.outcome);
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if normalization != 1 {
            return Err(Error::NonCanonicalPayoff);
        }
        Ok(())
    }

    fn validate_native(self, index: u32, node: NodeV3) -> Result<()> {
        if node.rank != 0
            || node.edge_count != 0
            || node.term_count != 1
            || node.native_outcome >= self.outcome_count
            || node.recipe_divisor != 1
            || node.flattened_denominator != 1
        {
            return Err(Error::InvalidNode);
        }
        let term = self.term(node.first_term)?;
        if term.outcome != node.native_outcome || term.numerator != 1 {
            return Err(Error::CompositionMismatch);
        }
        let mut prior_index = 0_u32;
        while prior_index < index {
            let prior = self.node(prior_index)?;
            if prior.kind == CompositionNodeKindV3::Native
                && prior.native_outcome == node.native_outcome
            {
                return Err(Error::DuplicateOrUnorderedNode);
            }
            prior_index = prior_index
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }

    fn validate_composed(self, index: u32, node: NodeV3) -> Result<()> {
        if node.rank == 0 || node.edge_count == 0 || node.native_outcome != 0 {
            return Err(Error::InvalidNode);
        }
        let mut edge_index = 0_u32;
        let mut prior_child_id = None;
        let mut expected_rank = 0_u32;
        let mut recipe_gcd = u128::from(node.recipe_divisor);
        let mut lcm = 1_u128;
        while edge_index < node.edge_count {
            let edge = self.edge(
                node.first_edge
                    .checked_add(edge_index)
                    .ok_or(Error::ArithmeticOverflow)?,
            )?;
            if edge.coefficient == 0
                || edge.child_index >= index
                || prior_child_id.is_some_and(|prior| edge.child_id <= prior)
            {
                return Err(Error::InvalidEdge);
            }
            let child = self.node(edge.child_index)?;
            if child.id != edge.child_id {
                return Err(Error::InvalidEdge);
            }
            expected_rank =
                expected_rank.max(child.rank.checked_add(1).ok_or(Error::ArithmeticOverflow)?);
            recipe_gcd = gcd_u128(recipe_gcd, u128::from(edge.coefficient));
            lcm = checked_lcm(lcm, u128::from(child.flattened_denominator))?;
            prior_child_id = Some(edge.child_id);
            edge_index = edge_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if node.rank != expected_rank || recipe_gcd != 1 {
            return Err(Error::InvalidNode);
        }
        let raw_denominator = lcm
            .checked_mul(u128::from(node.recipe_divisor))
            .ok_or(Error::ArithmeticOverflow)?;
        let mut payoff_gcd = raw_denominator;
        let mut nonzero_count = 0_u32;
        let mut outcome = 0_u32;
        while outcome < self.outcome_count {
            let numerator = self.direct_numerator(node, outcome, lcm)?;
            if numerator != 0 {
                payoff_gcd = gcd_u128(payoff_gcd, numerator);
                nonzero_count = nonzero_count
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if nonzero_count != node.term_count {
            return Err(Error::CompositionMismatch);
        }
        let normalized_denominator = raw_denominator
            .checked_div(payoff_gcd)
            .ok_or(Error::ArithmeticOverflow)?;
        let normalized_denominator =
            u64::try_from(normalized_denominator).map_err(|_| Error::ArithmeticOverflow)?;
        if normalized_denominator != node.flattened_denominator {
            return Err(Error::CompositionMismatch);
        }
        let mut term_index = 0_u32;
        outcome = 0;
        while outcome < self.outcome_count {
            let numerator = self.direct_numerator(node, outcome, lcm)?;
            if numerator != 0 {
                let normalized = numerator
                    .checked_div(payoff_gcd)
                    .ok_or(Error::ArithmeticOverflow)?;
                let normalized =
                    u64::try_from(normalized).map_err(|_| Error::ArithmeticOverflow)?;
                let expected = self.term(
                    node.first_term
                        .checked_add(term_index)
                        .ok_or(Error::ArithmeticOverflow)?,
                )?;
                if expected.outcome != outcome || expected.numerator != normalized {
                    return Err(Error::CompositionMismatch);
                }
                term_index = term_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
            outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }

    fn direct_numerator(self, node: NodeV3, outcome: u32, lcm: u128) -> Result<u128> {
        let mut sum = 0_u128;
        let mut edge_index = 0_u32;
        while edge_index < node.edge_count {
            let edge = self.edge(
                node.first_edge
                    .checked_add(edge_index)
                    .ok_or(Error::ArithmeticOverflow)?,
            )?;
            let child = self.node(edge.child_index)?;
            let child_numerator = self.node_numerator(child, outcome)?;
            let scale = lcm
                .checked_div(u128::from(child.flattened_denominator))
                .ok_or(Error::ArithmeticOverflow)?;
            let contribution = u128::from(edge.coefficient)
                .checked_mul(child_numerator)
                .and_then(|value| value.checked_mul(scale))
                .ok_or(Error::ArithmeticOverflow)?;
            sum = sum
                .checked_add(contribution)
                .ok_or(Error::ArithmeticOverflow)?;
            edge_index = edge_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(sum)
    }

    fn node_numerator(self, node: NodeV3, outcome: u32) -> Result<u128> {
        let mut index = 0_u32;
        while index < node.term_count {
            let term = self.term(
                node.first_term
                    .checked_add(index)
                    .ok_or(Error::ArithmeticOverflow)?,
            )?;
            if term.outcome == outcome {
                return Ok(u128::from(term.numerator));
            }
            if term.outcome > outcome {
                return Ok(0);
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(0)
    }

    fn validate_root_reachability(self) -> Result<()> {
        let mut candidate = 0_u32;
        while candidate < self.node_count {
            let mut incoming = 0_u32;
            let mut edge_index = 0_u32;
            while edge_index < self.edge_count {
                if self.edge(edge_index)?.child_index == candidate {
                    incoming = incoming.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
                }
                edge_index = edge_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
            if (candidate == self.root_index && incoming != 0)
                || (candidate != self.root_index && incoming == 0)
            {
                return Err(Error::AmbiguousRoot);
            }
            candidate = candidate.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }

    fn root(self) -> Result<NodeV3> {
        self.node(self.root_index)
    }

    fn node(self, index: u32) -> Result<NodeV3> {
        if index >= self.node_count {
            return Err(Error::InvalidNode);
        }
        let offset = self.node_offset(index)?;
        let bytes = slice(self.bytes, offset, COMPOSITION_NODE_BYTES_V3)?;
        Ok(NodeV3 {
            id: nonzero_array(bytes, NodeLayoutV3::ID)?,
            rank: u32_at(bytes, NodeLayoutV3::RANK)?,
            first_edge: u32_at(bytes, NodeLayoutV3::FIRST_EDGE)?,
            edge_count: u32_at(bytes, NodeLayoutV3::EDGE_COUNT)?,
            first_term: u32_at(bytes, NodeLayoutV3::FIRST_TERM)?,
            term_count: u32_at(bytes, NodeLayoutV3::TERM_COUNT)?,
            kind: CompositionNodeKindV3::decode(
                *bytes.get(NodeLayoutV3::KIND).ok_or(Error::InvalidLength)?,
            )?,
            native_outcome: u32_at(bytes, NodeLayoutV3::NATIVE_OUTCOME)?,
            recipe_divisor: u64_at(bytes, NodeLayoutV3::RECIPE_DIVISOR)?,
            flattened_denominator: u64_at(bytes, NodeLayoutV3::FLATTENED_DENOMINATOR)?,
        })
        .and_then(|node| {
            require_zero(bytes, NodeLayoutV3::RESERVED_KIND, 3)?;
            require_zero(bytes, NodeLayoutV3::RESERVED_SCALAR, 4)?;
            Ok(node)
        })
    }

    fn edge(self, index: u32) -> Result<EdgeV3> {
        if index >= self.edge_count {
            return Err(Error::InvalidEdge);
        }
        let offset = self.edge_offset(index)?;
        let bytes = slice(self.bytes, offset, COMPOSITION_EDGE_BYTES_V3)?;
        require_zero(bytes, EdgeLayoutV3::RESERVED, 4)?;
        Ok(EdgeV3 {
            child_id: nonzero_array(bytes, EdgeLayoutV3::CHILD_ID)?,
            child_index: u32_at(bytes, EdgeLayoutV3::CHILD_INDEX)?,
            coefficient: u64_at(bytes, EdgeLayoutV3::COEFFICIENT)?,
        })
    }

    fn term(self, index: u32) -> Result<SparseTermV3> {
        if index >= self.term_count {
            return Err(Error::NonCanonicalPayoff);
        }
        let offset = self.term_offset(index)?;
        let bytes = slice(self.bytes, offset, COMPOSITION_TERM_BYTES_V3)?;
        require_zero(bytes, TermLayoutV3::RESERVED, 4)?;
        Ok(SparseTermV3 {
            outcome: u32_at(bytes, TermLayoutV3::OUTCOME)?,
            numerator: u64_at(bytes, TermLayoutV3::NUMERATOR)?,
        })
    }

    fn node_offset(self, index: u32) -> Result<usize> {
        table_offset(
            COMPOSITION_GRAPH_HEADER_BYTES_V3,
            index,
            COMPOSITION_NODE_BYTES_V3,
        )
    }

    fn edge_offset(self, index: u32) -> Result<usize> {
        let nodes_bytes = usize_from_u32(self.node_count)?
            .checked_mul(COMPOSITION_NODE_BYTES_V3)
            .ok_or(Error::InvalidLength)?;
        table_offset(
            COMPOSITION_GRAPH_HEADER_BYTES_V3
                .checked_add(nodes_bytes)
                .ok_or(Error::InvalidLength)?,
            index,
            COMPOSITION_EDGE_BYTES_V3,
        )
    }

    fn term_offset(self, index: u32) -> Result<usize> {
        let nodes_bytes = usize_from_u32(self.node_count)?
            .checked_mul(COMPOSITION_NODE_BYTES_V3)
            .ok_or(Error::InvalidLength)?;
        let edges_bytes = usize_from_u32(self.edge_count)?
            .checked_mul(COMPOSITION_EDGE_BYTES_V3)
            .ok_or(Error::InvalidLength)?;
        table_offset(
            COMPOSITION_GRAPH_HEADER_BYTES_V3
                .checked_add(nodes_bytes)
                .and_then(|value| value.checked_add(edges_bytes))
                .ok_or(Error::InvalidLength)?,
            index,
            COMPOSITION_TERM_BYTES_V3,
        )
    }
}

/// Return the exact graph width for validated table counts.
pub fn composition_graph_bytes_v3(
    node_count: u32,
    edge_count: u32,
    term_count: u32,
) -> Result<usize> {
    if node_count == 0
        || node_count > MAX_COMPOSITION_NODES_V3
        || edge_count > MAX_COMPOSITION_EDGES_V3
        || term_count == 0
        || term_count > MAX_COMPOSITION_TERMS_V3
    {
        return Err(Error::CapacityExceeded);
    }
    let nodes = usize_from_u32(node_count)?
        .checked_mul(COMPOSITION_NODE_BYTES_V3)
        .ok_or(Error::InvalidLength)?;
    let edges = usize_from_u32(edge_count)?
        .checked_mul(COMPOSITION_EDGE_BYTES_V3)
        .ok_or(Error::InvalidLength)?;
    let terms = usize_from_u32(term_count)?
        .checked_mul(COMPOSITION_TERM_BYTES_V3)
        .ok_or(Error::InvalidLength)?;
    COMPOSITION_GRAPH_HEADER_BYTES_V3
        .checked_add(nodes)
        .and_then(|value| value.checked_add(edges))
        .and_then(|value| value.checked_add(terms))
        .ok_or(Error::InvalidLength)
}

/// Encode a graph atomically, then run the same structural/direct proof as decode.
pub fn encode_composition_graph_v3_atomic(
    input: CompositionGraphInputV3<'_>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let node_count = u32::try_from(input.nodes.len()).map_err(|_| Error::CapacityExceeded)?;
    let edge_count = u32::try_from(input.edges.len()).map_err(|_| Error::CapacityExceeded)?;
    let term_count = u32::try_from(input.terms.len()).map_err(|_| Error::CapacityExceeded)?;
    let length = composition_graph_bytes_v3(node_count, edge_count, term_count)?;
    if scratch.len() != length || output.len() != length {
        return Err(Error::InvalidLength);
    }
    if input.outcome_count < MIN_COMPOSITION_OUTCOMES_V3
        || input.outcome_count > MAX_COMPOSITION_OUTCOMES_V3
    {
        return Err(Error::InvalidOutcome);
    }
    scratch.fill(0);
    put(scratch, GraphLayoutV3::MAGIC, &COMPOSITION_GRAPH_MAGIC_V3)?;
    put(
        scratch,
        GraphLayoutV3::VERSION,
        &COMPOSITION_SCHEMA_VERSION_V3.to_le_bytes(),
    )?;
    put(scratch, GraphLayoutV3::GRAPH_ID, &input.graph_id)?;
    put(scratch, GraphLayoutV3::ROOT_ID, &input.root_id)?;
    for (offset, value) in [
        (GraphLayoutV3::OUTCOME_COUNT, input.outcome_count),
        (GraphLayoutV3::NODE_COUNT, node_count),
        (GraphLayoutV3::EDGE_COUNT, edge_count),
        (GraphLayoutV3::TERM_COUNT, term_count),
        (
            GraphLayoutV3::ROOT_INDEX,
            node_count.checked_sub(1).ok_or(Error::InvalidNode)?,
        ),
    ] {
        put(scratch, offset, &value.to_le_bytes())?;
    }
    for (index, node) in input.nodes.iter().enumerate() {
        let offset = table_offset(
            COMPOSITION_GRAPH_HEADER_BYTES_V3,
            u32_from_usize(index)?,
            COMPOSITION_NODE_BYTES_V3,
        )?;
        put(scratch, offset + NodeLayoutV3::ID, &node.id)?;
        for (field_offset, value) in [
            (NodeLayoutV3::RANK, node.rank),
            (NodeLayoutV3::FIRST_EDGE, node.first_edge),
            (NodeLayoutV3::EDGE_COUNT, node.edge_count),
            (NodeLayoutV3::FIRST_TERM, node.first_term),
            (NodeLayoutV3::TERM_COUNT, node.term_count),
            (NodeLayoutV3::NATIVE_OUTCOME, node.native_outcome),
        ] {
            put(scratch, offset + field_offset, &value.to_le_bytes())?;
        }
        put(scratch, offset + NodeLayoutV3::KIND, &[node.kind as u8])?;
        put(
            scratch,
            offset + NodeLayoutV3::RECIPE_DIVISOR,
            &node.recipe_divisor.to_le_bytes(),
        )?;
        put(
            scratch,
            offset + NodeLayoutV3::FLATTENED_DENOMINATOR,
            &node.flattened_denominator.to_le_bytes(),
        )?;
    }
    let edge_base = COMPOSITION_GRAPH_HEADER_BYTES_V3
        .checked_add(
            input
                .nodes
                .len()
                .checked_mul(COMPOSITION_NODE_BYTES_V3)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    for (index, edge) in input.edges.iter().enumerate() {
        let offset = table_offset(edge_base, u32_from_usize(index)?, COMPOSITION_EDGE_BYTES_V3)?;
        put(scratch, offset + EdgeLayoutV3::CHILD_ID, &edge.child_id)?;
        put(
            scratch,
            offset + EdgeLayoutV3::CHILD_INDEX,
            &edge.child_index.to_le_bytes(),
        )?;
        put(
            scratch,
            offset + EdgeLayoutV3::COEFFICIENT,
            &edge.coefficient.to_le_bytes(),
        )?;
    }
    let term_base = edge_base
        .checked_add(
            input
                .edges
                .len()
                .checked_mul(COMPOSITION_EDGE_BYTES_V3)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    for (index, term) in input.terms.iter().enumerate() {
        let offset = table_offset(term_base, u32_from_usize(index)?, COMPOSITION_TERM_BYTES_V3)?;
        put(
            scratch,
            offset + TermLayoutV3::OUTCOME,
            &term.outcome.to_le_bytes(),
        )?;
        put(
            scratch,
            offset + TermLayoutV3::NUMERATOR,
            &term.numerator.to_le_bytes(),
        )?;
    }
    let graph = CompositionGraphV3::decode_structural(scratch)?;
    if graph.graph_id != input.graph_id || graph.root_id != input.root_id {
        return Err(Error::CompositionMismatch);
    }
    output.copy_from_slice(scratch);
    Ok(())
}

fn checked_lcm(left: u128, right: u128) -> Result<u128> {
    if left == 0 || right == 0 {
        return Err(Error::ArithmeticOverflow);
    }
    left.checked_div(gcd_u128(left, right))
        .and_then(|value| value.checked_mul(right))
        .ok_or(Error::ArithmeticOverflow)
}

fn usize_from_u32(value: u32) -> Result<usize> {
    usize::try_from(value).map_err(|_| Error::InvalidLength)
}

fn u32_from_usize(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::CapacityExceeded)
}

fn table_offset(base: usize, index: u32, width: usize) -> Result<usize> {
    base.checked_add(
        usize_from_u32(index)?
            .checked_mul(width)
            .ok_or(Error::InvalidLength)?,
    )
    .ok_or(Error::InvalidLength)
}
