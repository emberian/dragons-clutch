#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact fractional claim shards and flattened representation recipes.
//!
//! This kernel validates borrowed projections and prepares exact transitions.
//! Token programs remain the sole supply/holder owner, Claims remains the sole
//! native/materialized economic owner, and finalized representation records
//! remain the sole recipe owner. No mutable representation ledger exists here.

#[allow(missing_docs)]
mod generated_descriptor;
#[allow(missing_docs)]
mod generated_product_v3;
/// ProductRuntimeV3 admission and exact representation-custody solvency.
pub mod product_v3;

pub use generated_descriptor::{
    DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V3,
    DESCRIPTOR_SCHEMA_VERSION_V3,
};
use generated_descriptor::{
    DESCRIPTOR_DENOMINATOR_OFFSET, DESCRIPTOR_GRAPH_DIGEST_OFFSET, DESCRIPTOR_GRAPH_ID_OFFSET,
    DESCRIPTOR_MAGIC_OFFSET, DESCRIPTOR_MARKET_ID_OFFSET, DESCRIPTOR_OUTCOME_COUNT_OFFSET,
    DESCRIPTOR_RECEIPT_MINT_OFFSET, DESCRIPTOR_RELEASE_SET_ID_OFFSET,
    DESCRIPTOR_RESERVED_HEADER_OFFSET, DESCRIPTOR_RESERVED_OFFSET, DESCRIPTOR_ROOT_ID_OFFSET,
    DESCRIPTOR_TOKEN_PROGRAM_OFFSET, DESCRIPTOR_VERSION_OFFSET,
};

/// Fixed Structured projection header before five runtime-width `u64` vectors.
pub const STRUCTURED_HEADER_BYTES: usize = 144;
/// One scalar in every runtime-width vector.
pub const SCALAR_BYTES: usize = 8;
/// Number of vectors in one Structured projection tail.
pub const STRUCTURED_VECTOR_COUNT: usize = 5;
/// Structured projection magic.
pub const STRUCTURED_MAGIC_V2: [u8; 8] = *b"DCRRSTR2";
/// Canonical graph header bytes.
pub const GRAPH_HEADER_BYTES: usize = 104;
/// Canonical fixed node-record bytes.
pub const GRAPH_NODE_BYTES: usize = 64;
/// Canonical fixed edge-record bytes.
pub const GRAPH_EDGE_BYTES: usize = 48;
/// Canonical graph magic.
pub const GRAPH_MAGIC_V2: [u8; 8] = *b"DCRRGRP2";
/// Canonical finalized-record schema label for [`RepresentationDescriptorV2`].
pub const REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/rational-representation-v2/noncircular-authority-v3";
/// SHA-256 identity of [`REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_PREIMAGE_V3`].
pub const REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3: [u8; 32] = [
    0x63, 0xe4, 0x17, 0xde, 0x63, 0x6d, 0xc1, 0x95, 0xdc, 0xa3, 0xec, 0x0d, 0xaf, 0xdc, 0x6c, 0x10,
    0x59, 0xda, 0xd9, 0x22, 0xe4, 0x8d, 0x27, 0xee, 0x3d, 0x65, 0x60, 0xb6, 0x96, 0x12, 0xbe, 0xb5,
];
/// Canonical finalized-record schema label for [`RepresentationGraphV2`].
pub const REPRESENTATION_GRAPH_SCHEMA_RELEASE_PREIMAGE_V2: &[u8] =
    b"dclutch/schema/rational-representation-graph-v2";
/// SHA-256 identity of [`REPRESENTATION_GRAPH_SCHEMA_RELEASE_PREIMAGE_V2`].
pub const REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2: [u8; 32] = [
    0xbe, 0x69, 0x36, 0xbb, 0xa2, 0x4e, 0xa0, 0xd2, 0xd1, 0x78, 0xfa, 0x65, 0x92, 0x74, 0x8e, 0xa5,
    0xf5, 0xdc, 0x95, 0xdf, 0x9a, 0x72, 0xbb, 0xa8, 0x58, 0x84, 0xa9, 0x27, 0xe2, 0x89, 0xd5, 0x97,
];
/// Implemented schema version.
pub const SCHEMA_VERSION_V2: u16 = 2;

const STRUCTURED_DESCRIPTOR_ID_OFFSET: usize = 16;
const STRUCTURED_MARKET_ID_OFFSET: usize = 48;
const STRUCTURED_RECEIPT_MINT_OFFSET: usize = 80;
const STRUCTURED_OUTCOME_COUNT_OFFSET: usize = 112;
const STRUCTURED_RESERVED_OFFSET: usize = 116;
const STRUCTURED_DENOMINATOR_OFFSET: usize = 120;
const STRUCTURED_RECEIPT_SUPPLY_OFFSET: usize = 128;
const STRUCTURED_REVISION_OFFSET: usize = 136;

const GRAPH_ID_OFFSET: usize = 16;
const GRAPH_ROOT_ID_OFFSET: usize = 48;
const GRAPH_OUTCOME_COUNT_OFFSET: usize = 80;
const GRAPH_NODE_COUNT_OFFSET: usize = 84;
const GRAPH_EDGE_COUNT_OFFSET: usize = 88;
const GRAPH_RESERVED_OFFSET: usize = 92;
const GRAPH_SCALE_OFFSET: usize = 96;

const NODE_ID_OFFSET: usize = 0;
const NODE_RANK_OFFSET: usize = 32;
const NODE_FIRST_EDGE_OFFSET: usize = 36;
const NODE_EDGE_COUNT_OFFSET: usize = 40;
const NODE_KIND_OFFSET: usize = 44;
const NODE_RESERVED_OFFSET: usize = 45;
const NODE_PARAMETER_OFFSET: usize = 48;
const NODE_TRAILING_RESERVED_OFFSET: usize = 56;

const EDGE_CHILD_ID_OFFSET: usize = 0;
const EDGE_CHILD_INDEX_OFFSET: usize = 32;
const EDGE_RESERVED_OFFSET: usize = 36;
const EDGE_MULTIPLICITY_OFFSET: usize = 40;

/// Stable hostile-decode or exact-accounting refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A fixed or runtime-derived byte width differed.
    InvalidLength,
    /// Magic bytes selected another schema.
    InvalidMagic,
    /// The schema version is unsupported.
    UnsupportedVersion,
    /// Reserved bytes or an enum tag were noncanonical.
    NonCanonical,
    /// A required content or account identity was zero.
    ZeroIdentity,
    /// Outcome, node, edge, or scalar width was zero or unrepresentable.
    InvalidWidth,
    /// Denominator or graph scale was zero.
    ZeroDenominator,
    /// A checked sum, product, offset, or revision overflowed.
    ArithmeticOverflow,
    /// Token shard supply did not equal denominator times Claims custody.
    DenominationMismatch,
    /// Structured custody did not equal receipt supply times coefficient.
    StructuredCustodyMismatch,
    /// Shard supply did not partition into Structured custody and free shards.
    ExplicitRemainderMismatch,
    /// No outcome coefficient was positive.
    EmptyRecipe,
    /// An exact action carried zero quantity.
    ZeroQuantity,
    /// Free shard or receipt supply was insufficient for the exact action.
    InsufficientBalance,
    /// Selected and finalized content identities or record evidence differed.
    ContentAdmissionMismatch,
    /// Immutable descriptor, graph root, or exact coefficient payoff differed.
    DescriptorMismatch,
    /// Node identities or `(rank, content_id)` order were noncanonical.
    NonCanonicalNodeOrder,
    /// An edge selected a missing, substituted, future, or unordered child.
    InvalidEdge,
    /// A node kind, rank, or arity was invalid.
    InvalidNode,
    /// A checked flattened native exposure differed from its recipe.
    ExposureMismatch,
    /// The graph root was substituted or not the canonical final node.
    RootMismatch,
    /// A node was not reachable from any parent on the path to the root.
    DisconnectedNode,
}

/// Result alias for this total kernel.
pub type Result<T> = core::result::Result<T, Error>;

/// One authenticated coordinate observation.
///
/// These fields are not persisted by this crate. A physical adapter must read
/// `native_locked` from the canonical Claims Position and all other supply and
/// custody values from exact Token-owned Mint/Account state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoordinateObservation {
    /// Shard atoms required by one Structured receipt atom.
    pub coefficient: u64,
    /// Native Claims atoms held in canonical shard custody.
    pub native_locked: u64,
    /// Token-owned shard Mint supply.
    pub shard_supply: u64,
    /// Token-owned shard balance held by Structured custody.
    pub structured_custody: u64,
    /// Token-owned shard supply outside Structured custody.
    pub explicit_free_shards: u64,
}

/// Scalar header for one adapter-owned ephemeral Structured projection.
///
/// The physical adapter writes this header and exact Token/Claims observations
/// into caller-owned scratch. No field is persisted by this kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredProjectionHeaderV2 {
    /// Immutable representation descriptor identity.
    pub descriptor_id: [u8; 32],
    /// Logical Core Market identity.
    pub market_id: [u8; 32],
    /// Token-owned Structured receipt Mint.
    pub receipt_mint: [u8; 32],
    /// Product-owned runtime outcome width.
    pub outcome_count: u32,
    /// Exact shard denominator.
    pub denominator: u64,
    /// Observed Token-owned receipt supply.
    pub receipt_supply: u64,
    /// Exact adapter replay revision.
    pub revision: u64,
}

/// Borrowed runtime-width Structured accounting projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredProjectionV2<'a> {
    descriptor_id: [u8; 32],
    market_id: [u8; 32],
    receipt_mint: [u8; 32],
    outcome_count: u32,
    denominator: u64,
    receipt_supply: u64,
    revision: u64,
    vectors: &'a [u8],
}

impl<'a> StructuredProjectionV2<'a> {
    /// Initialize exact caller-owned scratch for an ephemeral projection.
    pub fn write_header(output: &mut [u8], header: StructuredProjectionHeaderV2) -> Result<()> {
        if is_zero(&header.descriptor_id)
            || is_zero(&header.market_id)
            || is_zero(&header.receipt_mint)
        {
            return Err(Error::ZeroIdentity);
        }
        if header.outcome_count == 0 {
            return Err(Error::InvalidWidth);
        }
        if header.denominator == 0 {
            return Err(Error::ZeroDenominator);
        }
        let width = usize::try_from(header.outcome_count).map_err(|_| Error::InvalidWidth)?;
        let tail = width
            .checked_mul(STRUCTURED_VECTOR_COUNT)
            .and_then(|value| value.checked_mul(SCALAR_BYTES))
            .ok_or(Error::InvalidLength)?;
        if output.len()
            != STRUCTURED_HEADER_BYTES
                .checked_add(tail)
                .ok_or(Error::InvalidLength)?
        {
            return Err(Error::InvalidLength);
        }
        output.fill(0);
        output
            .get_mut(..8)
            .ok_or(Error::InvalidLength)?
            .copy_from_slice(&STRUCTURED_MAGIC_V2);
        output
            .get_mut(8..10)
            .ok_or(Error::InvalidLength)?
            .copy_from_slice(&SCHEMA_VERSION_V2.to_le_bytes());
        for (offset, value) in [
            (STRUCTURED_DESCRIPTOR_ID_OFFSET, header.descriptor_id),
            (STRUCTURED_MARKET_ID_OFFSET, header.market_id),
            (STRUCTURED_RECEIPT_MINT_OFFSET, header.receipt_mint),
        ] {
            output
                .get_mut(offset..offset + value.len())
                .ok_or(Error::InvalidLength)?
                .copy_from_slice(&value);
        }
        output
            .get_mut(STRUCTURED_OUTCOME_COUNT_OFFSET..STRUCTURED_OUTCOME_COUNT_OFFSET + 4)
            .ok_or(Error::InvalidLength)?
            .copy_from_slice(&header.outcome_count.to_le_bytes());
        for (offset, value) in [
            (STRUCTURED_DENOMINATOR_OFFSET, header.denominator),
            (STRUCTURED_RECEIPT_SUPPLY_OFFSET, header.receipt_supply),
            (STRUCTURED_REVISION_OFFSET, header.revision),
        ] {
            output
                .get_mut(offset..offset + SCALAR_BYTES)
                .ok_or(Error::InvalidLength)?
                .copy_from_slice(&value.to_le_bytes());
        }
        Ok(())
    }

    /// Write one exact observed coordinate into initialized scratch.
    pub fn write_coordinate(
        output: &mut [u8],
        outcome_count: u32,
        outcome: u32,
        coordinate: CoordinateObservation,
    ) -> Result<()> {
        if outcome >= outcome_count {
            return Err(Error::InvalidWidth);
        }
        let width = usize::try_from(outcome_count).map_err(|_| Error::InvalidWidth)?;
        let index = usize::try_from(outcome).map_err(|_| Error::InvalidWidth)?;
        let exact = STRUCTURED_HEADER_BYTES
            .checked_add(
                width
                    .checked_mul(STRUCTURED_VECTOR_COUNT)
                    .and_then(|value| value.checked_mul(SCALAR_BYTES))
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        if output.len() != exact {
            return Err(Error::InvalidLength);
        }
        for (vector, value) in [
            (0_usize, coordinate.coefficient),
            (1_usize, coordinate.native_locked),
            (2_usize, coordinate.shard_supply),
            (3_usize, coordinate.structured_custody),
            (4_usize, coordinate.explicit_free_shards),
        ] {
            let scalar = vector
                .checked_mul(width)
                .and_then(|base| base.checked_add(index))
                .and_then(|value| value.checked_mul(SCALAR_BYTES))
                .and_then(|value| STRUCTURED_HEADER_BYTES.checked_add(value))
                .ok_or(Error::InvalidLength)?;
            output
                .get_mut(scalar..scalar + SCALAR_BYTES)
                .ok_or(Error::InvalidLength)?
                .copy_from_slice(&value.to_le_bytes());
        }
        Ok(())
    }

    /// Hostile-decode and completely validate one exact borrowed projection.
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        if input.len() < STRUCTURED_HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        exact_magic(input, &STRUCTURED_MAGIC_V2)?;
        exact_version(input)?;
        require_zero(input, 10, 6)?;
        let descriptor_id = nonzero_array(input, STRUCTURED_DESCRIPTOR_ID_OFFSET)?;
        let market_id = nonzero_array(input, STRUCTURED_MARKET_ID_OFFSET)?;
        let receipt_mint = nonzero_array(input, STRUCTURED_RECEIPT_MINT_OFFSET)?;
        let outcome_count = u32_at(input, STRUCTURED_OUTCOME_COUNT_OFFSET)?;
        if outcome_count == 0 {
            return Err(Error::InvalidWidth);
        }
        require_zero(input, STRUCTURED_RESERVED_OFFSET, 4)?;
        let denominator = u64_at(input, STRUCTURED_DENOMINATOR_OFFSET)?;
        if denominator == 0 {
            return Err(Error::ZeroDenominator);
        }
        let receipt_supply = u64_at(input, STRUCTURED_RECEIPT_SUPPLY_OFFSET)?;
        let revision = u64_at(input, STRUCTURED_REVISION_OFFSET)?;
        let width = usize::try_from(outcome_count).map_err(|_| Error::InvalidWidth)?;
        let vector_bytes = width
            .checked_mul(SCALAR_BYTES)
            .ok_or(Error::InvalidLength)?;
        let tail_bytes = vector_bytes
            .checked_mul(STRUCTURED_VECTOR_COUNT)
            .ok_or(Error::InvalidLength)?;
        let exact_bytes = STRUCTURED_HEADER_BYTES
            .checked_add(tail_bytes)
            .ok_or(Error::InvalidLength)?;
        if input.len() != exact_bytes {
            return Err(Error::InvalidLength);
        }
        let vectors = subslice(input, STRUCTURED_HEADER_BYTES, tail_bytes)?;
        let projection = Self {
            descriptor_id,
            market_id,
            receipt_mint,
            outcome_count,
            denominator,
            receipt_supply,
            revision,
            vectors,
        };
        projection.validate()?;
        Ok(projection)
    }

    /// Immutable representation descriptor identity.
    pub const fn descriptor_id(self) -> [u8; 32] {
        self.descriptor_id
    }

    /// Logical Core Market identity.
    pub const fn market_id(self) -> [u8; 32] {
        self.market_id
    }

    /// Token-owned Structured receipt Mint.
    pub const fn receipt_mint(self) -> [u8; 32] {
        self.receipt_mint
    }

    /// Product-owned runtime outcome width.
    pub const fn outcome_count(self) -> u32 {
        self.outcome_count
    }

    /// Exact shard denominator.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    /// Token-owned receipt Mint supply observation.
    pub const fn receipt_supply(self) -> u64 {
        self.receipt_supply
    }

    /// Optimistic representation observation revision.
    pub const fn revision(self) -> u64 {
        self.revision
    }

    /// Read one exact coordinate without allocation or unchecked indexing.
    pub fn coordinate(self, outcome: u32) -> Result<CoordinateObservation> {
        if outcome >= self.outcome_count {
            return Err(Error::InvalidWidth);
        }
        let width = usize::try_from(self.outcome_count).map_err(|_| Error::InvalidWidth)?;
        let index = usize::try_from(outcome).map_err(|_| Error::InvalidWidth)?;
        Ok(CoordinateObservation {
            coefficient: vector_scalar(self.vectors, width, 0, index)?,
            native_locked: vector_scalar(self.vectors, width, 1, index)?,
            shard_supply: vector_scalar(self.vectors, width, 2, index)?,
            structured_custody: vector_scalar(self.vectors, width, 3, index)?,
            explicit_free_shards: vector_scalar(self.vectors, width, 4, index)?,
        })
    }

    fn validate(self) -> Result<()> {
        let mut any_coefficient = false;
        let mut outcome = 0_u32;
        while outcome < self.outcome_count {
            let coordinate = self.coordinate(outcome)?;
            any_coefficient |= coordinate.coefficient != 0;
            let expected_shards = self
                .denominator
                .checked_mul(coordinate.native_locked)
                .ok_or(Error::ArithmeticOverflow)?;
            if coordinate.shard_supply != expected_shards {
                return Err(Error::DenominationMismatch);
            }
            let expected_custody = self
                .receipt_supply
                .checked_mul(coordinate.coefficient)
                .ok_or(Error::ArithmeticOverflow)?;
            if coordinate.structured_custody != expected_custody {
                return Err(Error::StructuredCustodyMismatch);
            }
            let partition = coordinate
                .structured_custody
                .checked_add(coordinate.explicit_free_shards)
                .ok_or(Error::ArithmeticOverflow)?;
            if coordinate.shard_supply != partition {
                return Err(Error::ExplicitRemainderMismatch);
            }
            outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if !any_coefficient {
            return Err(Error::EmptyRecipe);
        }
        Ok(())
    }
}

/// Structured receipt transition style.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredAction {
    /// Mint receipt atoms and transfer exact free shards into custody.
    Issue,
    /// Burn receipt atoms and release exact custody shards to Token holders.
    Unwrap,
}

/// One per-outcome Token adapter effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredCoordinateEffect {
    /// Product-owned outcome selector.
    pub outcome: u32,
    /// Exact shard atoms moved between free holder balances and custody.
    pub shard_atoms: u64,
}

/// Completely validated Structured transition plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredPlanV2<'a> {
    projection: StructuredProjectionV2<'a>,
    action: StructuredAction,
    quantity: u64,
    post_receipt_supply: u64,
}

impl<'a> StructuredPlanV2<'a> {
    /// Exact action style.
    pub const fn action(self) -> StructuredAction {
        self.action
    }

    /// Receipt atoms minted or burned.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }

    /// Exact Token Mint supply required after the action.
    pub const fn post_receipt_supply(self) -> u64 {
        self.post_receipt_supply
    }

    /// Borrowed runtime-width iterator of exact shard transfers.
    pub const fn effects(self) -> StructuredEffectIter<'a> {
        StructuredEffectIter {
            projection: self.projection,
            quantity: self.quantity,
            next_outcome: 0,
        }
    }
}

/// Allocation-free iterator of exact Structured coordinate effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredEffectIter<'a> {
    projection: StructuredProjectionV2<'a>,
    quantity: u64,
    next_outcome: u32,
}

impl Iterator for StructuredEffectIter<'_> {
    type Item = Result<StructuredCoordinateEffect>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_outcome >= self.projection.outcome_count {
            return None;
        }
        let outcome = self.next_outcome;
        self.next_outcome = match self.next_outcome.checked_add(1) {
            Some(next) => next,
            None => return Some(Err(Error::ArithmeticOverflow)),
        };
        let coordinate = match self.projection.coordinate(outcome) {
            Ok(value) => value,
            Err(error) => return Some(Err(error)),
        };
        Some(
            self.quantity
                .checked_mul(coordinate.coefficient)
                .ok_or(Error::ArithmeticOverflow)
                .map(|shard_atoms| StructuredCoordinateEffect {
                    outcome,
                    shard_atoms,
                }),
        )
    }
}

/// Prepare an exact Structured receipt issuance.
pub fn prepare_issue<'a>(
    projection: StructuredProjectionV2<'a>,
    quantity: u64,
) -> Result<StructuredPlanV2<'a>> {
    if quantity == 0 {
        return Err(Error::ZeroQuantity);
    }
    let post_receipt_supply = projection
        .receipt_supply
        .checked_add(quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut outcome = 0_u32;
    while outcome < projection.outcome_count {
        let coordinate = projection.coordinate(outcome)?;
        let required = quantity
            .checked_mul(coordinate.coefficient)
            .ok_or(Error::ArithmeticOverflow)?;
        if required > coordinate.explicit_free_shards {
            return Err(Error::InsufficientBalance);
        }
        outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(StructuredPlanV2 {
        projection,
        action: StructuredAction::Issue,
        quantity,
        post_receipt_supply,
    })
}

/// Prepare an exact Structured receipt unwrap.
pub fn prepare_unwrap<'a>(
    projection: StructuredProjectionV2<'a>,
    quantity: u64,
) -> Result<StructuredPlanV2<'a>> {
    if quantity == 0 {
        return Err(Error::ZeroQuantity);
    }
    let post_receipt_supply = projection
        .receipt_supply
        .checked_sub(quantity)
        .ok_or(Error::InsufficientBalance)?;
    Ok(StructuredPlanV2 {
        projection,
        action: StructuredAction::Unwrap,
        quantity,
        post_receipt_supply,
    })
}

/// Exact shard coalescing result. `change_shards` remains a transferable Token
/// balance; it is never converted into a hidden protocol credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Coalescing {
    /// Shard atoms supplied by one or more Token holders.
    pub input_shards: u64,
    /// Exact native Claims atoms which may be reconstituted or paid.
    pub native_claims: u64,
    /// Explicit transferable shard change.
    pub change_shards: u64,
}

/// Coalesce arbitrary shard inputs at one exact denominator.
pub const fn coalesce(denominator: u64, input_shards: u64) -> Result<Coalescing> {
    if denominator == 0 {
        return Err(Error::ZeroDenominator);
    }
    Ok(Coalescing {
        input_shards,
        native_claims: input_shards / denominator,
        change_shards: input_shards % denominator,
    })
}

/// Exact successor for denomination or reconstitution of one coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShardCoordinateSuccessor {
    /// Claims-native custody after the action.
    pub native_locked: u64,
    /// Token shard Mint supply after the action.
    pub shard_supply: u64,
    /// Token shard supply outside Structured custody after the action.
    pub explicit_free_shards: u64,
}

/// Denominate native claims into free shard atoms without changing Structured
/// receipt supply or custody.
pub fn prepare_denominate(
    projection: StructuredProjectionV2<'_>,
    outcome: u32,
    quantity: u64,
) -> Result<ShardCoordinateSuccessor> {
    if quantity == 0 {
        return Err(Error::ZeroQuantity);
    }
    let coordinate = projection.coordinate(outcome)?;
    let shard_atoms = projection
        .denominator
        .checked_mul(quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(ShardCoordinateSuccessor {
        native_locked: coordinate
            .native_locked
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?,
        shard_supply: coordinate
            .shard_supply
            .checked_add(shard_atoms)
            .ok_or(Error::ArithmeticOverflow)?,
        explicit_free_shards: coordinate
            .explicit_free_shards
            .checked_add(shard_atoms)
            .ok_or(Error::ArithmeticOverflow)?,
    })
}

/// Burn one exact multiple of free shard atoms and release native claims.
pub fn prepare_reconstitute(
    projection: StructuredProjectionV2<'_>,
    outcome: u32,
    quantity: u64,
) -> Result<ShardCoordinateSuccessor> {
    if quantity == 0 {
        return Err(Error::ZeroQuantity);
    }
    let coordinate = projection.coordinate(outcome)?;
    let shard_atoms = projection
        .denominator
        .checked_mul(quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(ShardCoordinateSuccessor {
        native_locked: coordinate
            .native_locked
            .checked_sub(quantity)
            .ok_or(Error::InsufficientBalance)?,
        shard_supply: coordinate
            .shard_supply
            .checked_sub(shard_atoms)
            .ok_or(Error::InsufficientBalance)?,
        explicit_free_shards: coordinate
            .explicit_free_shards
            .checked_sub(shard_atoms)
            .ok_or(Error::InsufficientBalance)?,
    })
}

/// Finalized-record authentication observed for one immutable descriptor.
///
/// The adapter owns SHA-256, Record-program ownership, canonical raw/staging
/// PDA checks, and rent exemption. This kernel names those assumptions and
/// requires every identity to join; it never treats an ID echo as finality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescriptorAdmissionV2 {
    /// Descriptor selected by the Market/operator request.
    pub selected_descriptor_id: [u8; 32],
    /// Descriptor content identity in the finalized raw-record coordinates.
    pub finalized_descriptor_id: [u8; 32],
    /// SHA-256 of the exact descriptor bytes recomputed by the adapter.
    pub recomputed_descriptor_digest: [u8; 32],
    /// Digest committed by the finalized record identity.
    pub finalized_descriptor_digest: [u8; 32],
    /// Finalized raw owner/PDA, vacant staging PDA, and rent were authenticated.
    pub record_authenticated: bool,
    /// Claims PDA observed by the request/account adapter after digest finality.
    /// This value is not persisted in the descriptor preimage.
    pub derived_representation_authority: [u8; 32],
    /// The adapter rederived the authority from Claims program plus finalized
    /// descriptor digest and joined the observed request/account coordinate.
    pub authority_derivation_authenticated: bool,
}

/// Borrowed immutable authority for one exact rational representation.
///
/// The descriptor persists no Claims quantities, Token supplies, Token holder
/// balances, or replay revision. Its runtime-width tail contains only the
/// exact coefficients which interpret one receipt atom against a finalized
/// payoff graph. Per-outcome Mint/custody addresses are derived from this
/// content identity by the physical Claims adapter rather than repeated here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepresentationDescriptorV2<'a> {
    descriptor_id: [u8; 32],
    graph_id: [u8; 32],
    graph_digest: [u8; 32],
    root_id: [u8; 32],
    market_id: [u8; 32],
    release_set_id: [u8; 32],
    receipt_mint: [u8; 32],
    token_program: [u8; 32],
    representation_authority: [u8; 32],
    outcome_count: u32,
    denominator: u64,
    coefficients: &'a [u8],
}

impl<'a> RepresentationDescriptorV2<'a> {
    /// Hostile-decode an exact finalized descriptor preimage.
    pub fn decode(input: &'a [u8], admission: DescriptorAdmissionV2) -> Result<Self> {
        if input.len() < DESCRIPTOR_HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if array_at::<8>(input, DESCRIPTOR_MAGIC_OFFSET)? != DESCRIPTOR_MAGIC_V3 {
            return Err(Error::InvalidMagic);
        }
        if u16_at(input, DESCRIPTOR_VERSION_OFFSET)? != DESCRIPTOR_SCHEMA_VERSION_V3 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, DESCRIPTOR_RESERVED_HEADER_OFFSET, 6)?;
        let outcome_count = u32_at(input, DESCRIPTOR_OUTCOME_COUNT_OFFSET)?;
        if outcome_count == 0 {
            return Err(Error::InvalidWidth);
        }
        require_zero(input, DESCRIPTOR_RESERVED_OFFSET, 4)?;
        let coefficient_bytes = usize::try_from(outcome_count)
            .map_err(|_| Error::InvalidWidth)?
            .checked_mul(DESCRIPTOR_COEFFICIENT_BYTES)
            .ok_or(Error::InvalidLength)?;
        if input.len()
            != DESCRIPTOR_HEADER_BYTES
                .checked_add(coefficient_bytes)
                .ok_or(Error::InvalidLength)?
        {
            return Err(Error::InvalidLength);
        }
        validate_descriptor_admission(admission)?;
        let descriptor = Self {
            descriptor_id: admission.selected_descriptor_id,
            graph_id: nonzero_array(input, DESCRIPTOR_GRAPH_ID_OFFSET)?,
            graph_digest: nonzero_array(input, DESCRIPTOR_GRAPH_DIGEST_OFFSET)?,
            root_id: nonzero_array(input, DESCRIPTOR_ROOT_ID_OFFSET)?,
            market_id: nonzero_array(input, DESCRIPTOR_MARKET_ID_OFFSET)?,
            release_set_id: nonzero_array(input, DESCRIPTOR_RELEASE_SET_ID_OFFSET)?,
            receipt_mint: nonzero_array(input, DESCRIPTOR_RECEIPT_MINT_OFFSET)?,
            token_program: nonzero_array(input, DESCRIPTOR_TOKEN_PROGRAM_OFFSET)?,
            representation_authority: admission.derived_representation_authority,
            outcome_count,
            denominator: u64_at(input, DESCRIPTOR_DENOMINATOR_OFFSET)?,
            coefficients: subslice(input, DESCRIPTOR_HEADER_BYTES, coefficient_bytes)?,
        };
        if descriptor.denominator == 0 {
            return Err(Error::ZeroDenominator);
        }
        let mut any_coefficient = false;
        let mut outcome = 0_u32;
        while outcome < descriptor.outcome_count {
            any_coefficient |= descriptor.coefficient(outcome)? != 0;
            outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if !any_coefficient {
            return Err(Error::EmptyRecipe);
        }
        Ok(descriptor)
    }

    /// Exact content identity of the immutable descriptor bytes.
    pub const fn descriptor_id(self) -> [u8; 32] {
        self.descriptor_id
    }

    /// Finalized graph selected by this descriptor.
    pub const fn graph_id(self) -> [u8; 32] {
        self.graph_id
    }

    /// SHA-256 of the exact finalized graph bytes.
    pub const fn graph_digest(self) -> [u8; 32] {
        self.graph_digest
    }

    /// Finalized graph root selected by this descriptor.
    pub const fn root_id(self) -> [u8; 32] {
        self.root_id
    }

    /// Logical Core Market whose native Claims back this representation.
    pub const fn market_id(self) -> [u8; 32] {
        self.market_id
    }

    /// Exact execution release set admitting the representation.
    pub const fn release_set_id(self) -> [u8; 32] {
        self.release_set_id
    }

    /// Token-owned Structured receipt Mint.
    pub const fn receipt_mint(self) -> [u8; 32] {
        self.receipt_mint
    }

    /// Immutable legacy Token or Token-2022 adapter selection.
    pub const fn token_program(self) -> [u8; 32] {
        self.token_program
    }

    /// Claims PDA controlling exact shard and receipt mutations.
    pub const fn representation_authority(self) -> [u8; 32] {
        self.representation_authority
    }

    /// Product-owned runtime outcome width.
    pub const fn outcome_count(self) -> u32 {
        self.outcome_count
    }

    /// Shard atoms backing one native claim atom.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    /// Exact shard atoms required per Structured receipt atom at one outcome.
    pub fn coefficient(self, outcome: u32) -> Result<u64> {
        if outcome >= self.outcome_count {
            return Err(Error::InvalidWidth);
        }
        let offset = usize::try_from(outcome)
            .map_err(|_| Error::InvalidWidth)?
            .checked_mul(DESCRIPTOR_COEFFICIENT_BYTES)
            .ok_or(Error::InvalidLength)?;
        u64_at(self.coefficients, offset)
    }

    /// Join this descriptor to its exact graph/root and prove every
    /// coefficient has the same common-scale native payoff.
    pub fn authenticate_graph(self, graph: RepresentationGraphV2<'_>) -> Result<()> {
        if self.graph_id != graph.graph_id()
            || self.graph_digest != graph.record_digest()
            || self.root_id != graph.root_id()
            || self.outcome_count != graph.outcome_count()
        {
            return Err(Error::DescriptorMismatch);
        }
        let mut outcome = 0_u32;
        while outcome < self.outcome_count {
            let coefficient_payoff = self
                .coefficient(outcome)?
                .checked_mul(graph.scale())
                .ok_or(Error::ArithmeticOverflow)?;
            let root_payoff = graph
                .root_exposure(outcome)?
                .checked_mul(self.denominator)
                .ok_or(Error::ArithmeticOverflow)?;
            if coefficient_payoff != root_payoff {
                return Err(Error::DescriptorMismatch);
            }
            outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }
}

/// Finalized-record authentication observed by the physical adapter.
///
/// The booleans name the unverified boundary explicitly. This kernel validates
/// structure and flattening but deliberately does not pretend to parse Solana
/// Record accounts or implement SHA-256.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContentAdmissionV2 {
    /// Graph selected by the immutable representation descriptor.
    pub selected_graph_id: [u8; 32],
    /// Graph identity in the finalized record.
    pub finalized_graph_id: [u8; 32],
    /// SHA-256 of the exact graph bytes recomputed by the adapter.
    pub recomputed_graph_digest: [u8; 32],
    /// Content digest in the finalized record identity.
    pub finalized_graph_digest: [u8; 32],
    /// Finalized Record program/account owner and PDA were authenticated.
    pub record_authenticated: bool,
}

/// One canonical recipe node kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphNodeKind {
    /// One Product-native claim coordinate at graph-wide scale.
    Native,
    /// One exact denominator of a child atom.
    Shard,
    /// Integer quantities of one or more child atoms.
    Basket,
}

/// Borrowed, structurally and arithmetically validated representation graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepresentationGraphV2<'a> {
    input: &'a [u8],
    graph_id: [u8; 32],
    record_digest: [u8; 32],
    root_id: [u8; 32],
    outcome_count: u32,
    node_count: u32,
    edge_count: u32,
    scale: u64,
    node_offset: usize,
    edge_offset: usize,
    exposure_offset: usize,
}

#[derive(Clone, Copy)]
struct NodeMeta {
    id: [u8; 32],
    rank: u32,
    first_edge: u32,
    edge_count: u32,
    kind: GraphNodeKind,
    parameter: u64,
}

#[derive(Clone, Copy)]
struct EdgeMeta {
    child_id: [u8; 32],
    child_index: u32,
    multiplicity: u64,
}

impl<'a> RepresentationGraphV2<'a> {
    /// Hostile-decode and completely validate one canonical flattened graph.
    pub fn decode(input: &'a [u8], admission: ContentAdmissionV2) -> Result<Self> {
        if input.len() < GRAPH_HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        exact_magic(input, &GRAPH_MAGIC_V2)?;
        exact_version(input)?;
        require_zero(input, 10, 6)?;
        let graph_id = nonzero_array(input, GRAPH_ID_OFFSET)?;
        let root_id = nonzero_array(input, GRAPH_ROOT_ID_OFFSET)?;
        let outcome_count = u32_at(input, GRAPH_OUTCOME_COUNT_OFFSET)?;
        let node_count = u32_at(input, GRAPH_NODE_COUNT_OFFSET)?;
        let edge_count = u32_at(input, GRAPH_EDGE_COUNT_OFFSET)?;
        require_zero(input, GRAPH_RESERVED_OFFSET, 4)?;
        let scale = u64_at(input, GRAPH_SCALE_OFFSET)?;
        if outcome_count == 0 || node_count == 0 {
            return Err(Error::InvalidWidth);
        }
        if scale == 0 {
            return Err(Error::ZeroDenominator);
        }
        validate_content_admission(graph_id, admission)?;
        let nodes = usize::try_from(node_count).map_err(|_| Error::InvalidWidth)?;
        let edges = usize::try_from(edge_count).map_err(|_| Error::InvalidWidth)?;
        let outcomes = usize::try_from(outcome_count).map_err(|_| Error::InvalidWidth)?;
        let node_bytes = nodes
            .checked_mul(GRAPH_NODE_BYTES)
            .ok_or(Error::InvalidLength)?;
        let edge_bytes = edges
            .checked_mul(GRAPH_EDGE_BYTES)
            .ok_or(Error::InvalidLength)?;
        let exposure_scalars = nodes.checked_mul(outcomes).ok_or(Error::InvalidLength)?;
        let exposure_bytes = exposure_scalars
            .checked_mul(SCALAR_BYTES)
            .ok_or(Error::InvalidLength)?;
        let node_offset = GRAPH_HEADER_BYTES;
        let edge_offset = node_offset
            .checked_add(node_bytes)
            .ok_or(Error::InvalidLength)?;
        let exposure_offset = edge_offset
            .checked_add(edge_bytes)
            .ok_or(Error::InvalidLength)?;
        let exact_bytes = exposure_offset
            .checked_add(exposure_bytes)
            .ok_or(Error::InvalidLength)?;
        if input.len() != exact_bytes {
            return Err(Error::InvalidLength);
        }
        let graph = Self {
            input,
            graph_id,
            record_digest: admission.finalized_graph_digest,
            root_id,
            outcome_count,
            node_count,
            edge_count,
            scale,
            node_offset,
            edge_offset,
            exposure_offset,
        };
        graph.validate()?;
        Ok(graph)
    }

    /// Finalized graph identity.
    pub const fn graph_id(self) -> [u8; 32] {
        self.graph_id
    }

    /// SHA-256 of the exact finalized graph bytes authenticated by the adapter.
    pub const fn record_digest(self) -> [u8; 32] {
        self.record_digest
    }

    /// Selected root-node content identity.
    pub const fn root_id(self) -> [u8; 32] {
        self.root_id
    }

    /// Product-native exposure width.
    pub const fn outcome_count(self) -> u32 {
        self.outcome_count
    }

    /// Graph-wide exact integer scale.
    pub const fn scale(self) -> u64 {
        self.scale
    }

    /// Read one precomputed exact root native exposure coordinate.
    pub fn root_exposure(self, outcome: u32) -> Result<u64> {
        if outcome >= self.outcome_count {
            return Err(Error::InvalidWidth);
        }
        self.exposure(self.node_count - 1, outcome)
    }

    fn validate(self) -> Result<()> {
        let mut expected_first_edge = 0_u32;
        let mut index = 0_u32;
        let mut previous: Option<NodeMeta> = None;
        while index < self.node_count {
            let node = self.node(index)?;
            if node.first_edge != expected_first_edge {
                return Err(Error::InvalidEdge);
            }
            expected_first_edge = expected_first_edge
                .checked_add(node.edge_count)
                .ok_or(Error::ArithmeticOverflow)?;
            if expected_first_edge > self.edge_count {
                return Err(Error::InvalidEdge);
            }
            if let Some(left) = previous
                && (node.rank < left.rank || (node.rank == left.rank && node.id <= left.id))
            {
                return Err(Error::NonCanonicalNodeOrder);
            }
            self.validate_unique_id(index, node.id)?;
            self.validate_node(index, node)?;
            previous = Some(node);
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if expected_first_edge != self.edge_count {
            return Err(Error::InvalidEdge);
        }
        let root = self.node(self.node_count - 1)?;
        if root.id != self.root_id {
            return Err(Error::RootMismatch);
        }
        let mut child = 0_u32;
        while child + 1 < self.node_count {
            if !self.has_incoming_edge(child)? {
                return Err(Error::DisconnectedNode);
            }
            child = child.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }

    fn validate_unique_id(self, index: u32, id: [u8; 32]) -> Result<()> {
        let mut prior = 0_u32;
        while prior < index {
            if self.node(prior)?.id == id {
                return Err(Error::NonCanonicalNodeOrder);
            }
            prior = prior.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }

    fn validate_node(self, index: u32, node: NodeMeta) -> Result<()> {
        match node.kind {
            GraphNodeKind::Native => self.validate_native(index, node),
            GraphNodeKind::Shard => self.validate_shard(index, node),
            GraphNodeKind::Basket => self.validate_basket(index, node),
        }
    }

    fn validate_native(self, index: u32, node: NodeMeta) -> Result<()> {
        if node.rank != 0 || node.edge_count != 0 || node.parameter >= u64::from(self.outcome_count)
        {
            return Err(Error::InvalidNode);
        }
        let selected = u32::try_from(node.parameter).map_err(|_| Error::InvalidNode)?;
        let mut outcome = 0_u32;
        while outcome < self.outcome_count {
            let expected = if outcome == selected { self.scale } else { 0 };
            if self.exposure(index, outcome)? != expected {
                return Err(Error::ExposureMismatch);
            }
            outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }

    fn validate_shard(self, index: u32, node: NodeMeta) -> Result<()> {
        if node.parameter <= 1 || node.edge_count != 1 {
            return Err(Error::InvalidNode);
        }
        let edge = self.edge(node.first_edge)?;
        let child = self.validate_child(index, node.rank, edge)?;
        if edge.multiplicity != 1 || node.rank != child.rank + 1 {
            return Err(Error::InvalidNode);
        }
        let mut outcome = 0_u32;
        while outcome < self.outcome_count {
            let scaled = self
                .exposure(index, outcome)?
                .checked_mul(node.parameter)
                .ok_or(Error::ArithmeticOverflow)?;
            if scaled != self.exposure(edge.child_index, outcome)? {
                return Err(Error::ExposureMismatch);
            }
            outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }

    fn validate_basket(self, index: u32, node: NodeMeta) -> Result<()> {
        if node.parameter != 0 || node.edge_count == 0 {
            return Err(Error::InvalidNode);
        }
        let mut edge_ordinal = 0_u32;
        let mut maximum_rank = 0_u32;
        let mut previous_id: Option<[u8; 32]> = None;
        while edge_ordinal < node.edge_count {
            let edge_index = node
                .first_edge
                .checked_add(edge_ordinal)
                .ok_or(Error::ArithmeticOverflow)?;
            let edge = self.edge(edge_index)?;
            let child = self.validate_child(index, node.rank, edge)?;
            if let Some(left) = previous_id
                && edge.child_id <= left
            {
                return Err(Error::InvalidEdge);
            }
            previous_id = Some(edge.child_id);
            maximum_rank = core::cmp::max(maximum_rank, child.rank);
            edge_ordinal = edge_ordinal
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        if node.rank
            != maximum_rank
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?
        {
            return Err(Error::InvalidNode);
        }
        let mut outcome = 0_u32;
        while outcome < self.outcome_count {
            let mut expected = 0_u64;
            let mut ordinal = 0_u32;
            while ordinal < node.edge_count {
                let edge = self.edge(
                    node.first_edge
                        .checked_add(ordinal)
                        .ok_or(Error::ArithmeticOverflow)?,
                )?;
                let contribution = self
                    .exposure(edge.child_index, outcome)?
                    .checked_mul(edge.multiplicity)
                    .ok_or(Error::ArithmeticOverflow)?;
                expected = expected
                    .checked_add(contribution)
                    .ok_or(Error::ArithmeticOverflow)?;
                ordinal = ordinal.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
            if self.exposure(index, outcome)? != expected {
                return Err(Error::ExposureMismatch);
            }
            outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }

    fn validate_child(
        self,
        parent_index: u32,
        parent_rank: u32,
        edge: EdgeMeta,
    ) -> Result<NodeMeta> {
        if edge.multiplicity == 0 || edge.child_index >= parent_index {
            return Err(Error::InvalidEdge);
        }
        let child = self.node(edge.child_index)?;
        if child.id != edge.child_id || child.rank >= parent_rank {
            return Err(Error::InvalidEdge);
        }
        Ok(child)
    }

    fn has_incoming_edge(self, child_index: u32) -> Result<bool> {
        let mut edge_index = 0_u32;
        while edge_index < self.edge_count {
            if self.edge(edge_index)?.child_index == child_index {
                return Ok(true);
            }
            edge_index = edge_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(false)
    }

    fn node(self, index: u32) -> Result<NodeMeta> {
        if index >= self.node_count {
            return Err(Error::InvalidNode);
        }
        let offset = self
            .node_offset
            .checked_add(
                usize::try_from(index)
                    .map_err(|_| Error::InvalidNode)?
                    .checked_mul(GRAPH_NODE_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        let bytes = subslice(self.input, offset, GRAPH_NODE_BYTES)?;
        let id = nonzero_array(bytes, NODE_ID_OFFSET)?;
        let rank = u32_at(bytes, NODE_RANK_OFFSET)?;
        let first_edge = u32_at(bytes, NODE_FIRST_EDGE_OFFSET)?;
        let edge_count = u32_at(bytes, NODE_EDGE_COUNT_OFFSET)?;
        let kind = match byte_at(bytes, NODE_KIND_OFFSET)? {
            0 => GraphNodeKind::Native,
            1 => GraphNodeKind::Shard,
            2 => GraphNodeKind::Basket,
            _ => return Err(Error::NonCanonical),
        };
        require_zero(bytes, NODE_RESERVED_OFFSET, 3)?;
        let parameter = u64_at(bytes, NODE_PARAMETER_OFFSET)?;
        require_zero(bytes, NODE_TRAILING_RESERVED_OFFSET, 8)?;
        Ok(NodeMeta {
            id,
            rank,
            first_edge,
            edge_count,
            kind,
            parameter,
        })
    }

    fn edge(self, index: u32) -> Result<EdgeMeta> {
        if index >= self.edge_count {
            return Err(Error::InvalidEdge);
        }
        let offset = self
            .edge_offset
            .checked_add(
                usize::try_from(index)
                    .map_err(|_| Error::InvalidEdge)?
                    .checked_mul(GRAPH_EDGE_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        let bytes = subslice(self.input, offset, GRAPH_EDGE_BYTES)?;
        let child_id = nonzero_array(bytes, EDGE_CHILD_ID_OFFSET)?;
        let child_index = u32_at(bytes, EDGE_CHILD_INDEX_OFFSET)?;
        require_zero(bytes, EDGE_RESERVED_OFFSET, 4)?;
        let multiplicity = u64_at(bytes, EDGE_MULTIPLICITY_OFFSET)?;
        Ok(EdgeMeta {
            child_id,
            child_index,
            multiplicity,
        })
    }

    fn exposure(self, node: u32, outcome: u32) -> Result<u64> {
        if node >= self.node_count || outcome >= self.outcome_count {
            return Err(Error::InvalidWidth);
        }
        let linear = usize::try_from(node)
            .map_err(|_| Error::InvalidWidth)?
            .checked_mul(usize::try_from(self.outcome_count).map_err(|_| Error::InvalidWidth)?)
            .and_then(|base| base.checked_add(usize::try_from(outcome).ok()?))
            .ok_or(Error::InvalidLength)?;
        let offset = self
            .exposure_offset
            .checked_add(
                linear
                    .checked_mul(SCALAR_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        u64_at(self.input, offset)
    }
}

fn validate_content_admission(graph_id: [u8; 32], admission: ContentAdmissionV2) -> Result<()> {
    if !admission.record_authenticated
        || graph_id != admission.selected_graph_id
        || graph_id != admission.finalized_graph_id
        || is_zero(&admission.recomputed_graph_digest)
        || admission.recomputed_graph_digest != admission.finalized_graph_digest
    {
        return Err(Error::ContentAdmissionMismatch);
    }
    Ok(())
}

fn validate_descriptor_admission(admission: DescriptorAdmissionV2) -> Result<()> {
    if !admission.record_authenticated
        || !admission.authority_derivation_authenticated
        || is_zero(&admission.selected_descriptor_id)
        || is_zero(&admission.derived_representation_authority)
        || admission.selected_descriptor_id != admission.finalized_descriptor_id
        || admission.selected_descriptor_id != admission.recomputed_descriptor_digest
        || admission.recomputed_descriptor_digest != admission.finalized_descriptor_digest
    {
        return Err(Error::ContentAdmissionMismatch);
    }
    Ok(())
}

fn exact_magic(input: &[u8], expected: &[u8; 8]) -> Result<()> {
    if array_at::<8>(input, 0)? != *expected {
        return Err(Error::InvalidMagic);
    }
    Ok(())
}

fn exact_version(input: &[u8]) -> Result<()> {
    if u16_at(input, 8)? != SCHEMA_VERSION_V2 {
        return Err(Error::UnsupportedVersion);
    }
    Ok(())
}

fn vector_scalar(vectors: &[u8], width: usize, vector: usize, index: usize) -> Result<u64> {
    if index >= width || vector >= STRUCTURED_VECTOR_COUNT {
        return Err(Error::InvalidWidth);
    }
    let scalar = vector
        .checked_mul(width)
        .and_then(|base| base.checked_add(index))
        .ok_or(Error::InvalidLength)?;
    u64_at(
        vectors,
        scalar
            .checked_mul(SCALAR_BYTES)
            .ok_or(Error::InvalidLength)?,
    )
}

fn require_zero(input: &[u8], offset: usize, length: usize) -> Result<()> {
    if subslice(input, offset, length)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonical);
    }
    Ok(())
}

fn nonzero_array(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    let value = array_at::<32>(input, offset)?;
    if is_zero(&value) {
        return Err(Error::ZeroIdentity);
    }
    Ok(value)
}

fn is_zero<const N: usize>(value: &[u8; N]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn byte_at(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array_at(input, offset)?))
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array_at(input, offset)?))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array_at(input, offset)?))
}

fn array_at<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    let bytes = subslice(input, offset, N)?;
    bytes.try_into().map_err(|_| Error::InvalidLength)
}

fn subslice(input: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    input
        .get(offset..offset.checked_add(length).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;
    use std::vec::Vec;

    fn put(output: &mut [u8], offset: usize, bytes: &[u8]) {
        output
            .get_mut(offset..offset + bytes.len())
            .expect("fixture offset")
            .copy_from_slice(bytes);
    }

    fn put_u32(output: &mut [u8], offset: usize, value: u32) {
        put(output, offset, &value.to_le_bytes());
    }

    fn put_u64(output: &mut [u8], offset: usize, value: u64) {
        put(output, offset, &value.to_le_bytes());
    }

    fn structured_fixture() -> Vec<u8> {
        let width = 2_usize;
        let mut bytes = vec![0_u8; STRUCTURED_HEADER_BYTES + width * 40];
        put(&mut bytes, 0, &STRUCTURED_MAGIC_V2);
        put(&mut bytes, 8, &SCHEMA_VERSION_V2.to_le_bytes());
        put(&mut bytes, STRUCTURED_DESCRIPTOR_ID_OFFSET, &[1; 32]);
        put(&mut bytes, STRUCTURED_MARKET_ID_OFFSET, &[2; 32]);
        put(&mut bytes, STRUCTURED_RECEIPT_MINT_OFFSET, &[3; 32]);
        put_u32(&mut bytes, STRUCTURED_OUTCOME_COUNT_OFFSET, 2);
        put_u64(&mut bytes, STRUCTURED_DENOMINATOR_OFFSET, 10);
        put_u64(&mut bytes, STRUCTURED_RECEIPT_SUPPLY_OFFSET, 7);
        put_u64(&mut bytes, STRUCTURED_REVISION_OFFSET, 4);
        let tail = STRUCTURED_HEADER_BYTES;
        let vectors = [3_u64, 7, 3, 6, 30, 60, 21, 49, 9, 11];
        for (index, value) in vectors.iter().enumerate() {
            put_u64(&mut bytes, tail + index * 8, *value);
        }
        bytes
    }

    #[test]
    fn adapter_projection_writer_roundtrips_exact_observations() {
        let mut bytes = vec![0_u8; STRUCTURED_HEADER_BYTES + 2 * 5 * SCALAR_BYTES];
        StructuredProjectionV2::write_header(
            &mut bytes,
            StructuredProjectionHeaderV2 {
                descriptor_id: [1; 32],
                market_id: [2; 32],
                receipt_mint: [3; 32],
                outcome_count: 2,
                denominator: 10,
                receipt_supply: 7,
                revision: 4,
            },
        )
        .expect("write exact header");
        for (outcome, coordinate) in [
            CoordinateObservation {
                coefficient: 3,
                native_locked: 3,
                shard_supply: 30,
                structured_custody: 21,
                explicit_free_shards: 9,
            },
            CoordinateObservation {
                coefficient: 7,
                native_locked: 6,
                shard_supply: 60,
                structured_custody: 49,
                explicit_free_shards: 11,
            },
        ]
        .into_iter()
        .enumerate()
        {
            StructuredProjectionV2::write_coordinate(
                &mut bytes,
                2,
                u32::try_from(outcome).expect("fixture outcome"),
                coordinate,
            )
            .expect("write coordinate");
        }
        let projection = StructuredProjectionV2::decode(&bytes).expect("roundtrip projection");
        assert_eq!(
            projection.coordinate(1),
            Ok(CoordinateObservation {
                coefficient: 7,
                native_locked: 6,
                shard_supply: 60,
                structured_custody: 49,
                explicit_free_shards: 11,
            })
        );
    }

    fn admission() -> ContentAdmissionV2 {
        ContentAdmissionV2 {
            selected_graph_id: [6; 32],
            finalized_graph_id: [6; 32],
            recomputed_graph_digest: [7; 32],
            finalized_graph_digest: [7; 32],
            record_authenticated: true,
        }
    }

    fn descriptor_admission() -> DescriptorAdmissionV2 {
        DescriptorAdmissionV2 {
            selected_descriptor_id: [8; 32],
            finalized_descriptor_id: [8; 32],
            recomputed_descriptor_digest: [8; 32],
            finalized_descriptor_digest: [8; 32],
            record_authenticated: true,
            derived_representation_authority: [11; 32],
            authority_derivation_authenticated: true,
        }
    }

    fn descriptor_fixture() -> Vec<u8> {
        let mut bytes = vec![0_u8; DESCRIPTOR_HEADER_BYTES + 2 * DESCRIPTOR_COEFFICIENT_BYTES];
        put(&mut bytes, 0, &DESCRIPTOR_MAGIC_V3);
        put(&mut bytes, 8, &DESCRIPTOR_SCHEMA_VERSION_V3.to_le_bytes());
        put(&mut bytes, DESCRIPTOR_GRAPH_ID_OFFSET, &[6; 32]);
        put(&mut bytes, DESCRIPTOR_GRAPH_DIGEST_OFFSET, &[7; 32]);
        put(&mut bytes, DESCRIPTOR_ROOT_ID_OFFSET, &[5; 32]);
        put(&mut bytes, DESCRIPTOR_MARKET_ID_OFFSET, &[2; 32]);
        put(&mut bytes, DESCRIPTOR_RELEASE_SET_ID_OFFSET, &[9; 32]);
        put(&mut bytes, DESCRIPTOR_RECEIPT_MINT_OFFSET, &[3; 32]);
        put(&mut bytes, DESCRIPTOR_TOKEN_PROGRAM_OFFSET, &[10; 32]);
        put_u32(&mut bytes, DESCRIPTOR_OUTCOME_COUNT_OFFSET, 2);
        put_u64(&mut bytes, DESCRIPTOR_DENOMINATOR_OFFSET, 10);
        put_u64(&mut bytes, DESCRIPTOR_HEADER_BYTES, 3);
        put_u64(
            &mut bytes,
            DESCRIPTOR_HEADER_BYTES + DESCRIPTOR_COEFFICIENT_BYTES,
            7,
        );
        bytes
    }

    #[derive(Clone, Copy)]
    struct NodeFixture {
        id: u8,
        rank: u32,
        first_edge: u32,
        edge_count: u32,
        kind: u8,
        parameter: u64,
        exposure: [u64; 2],
    }

    #[derive(Clone, Copy)]
    struct EdgeFixture {
        child_id: u8,
        child_index: u32,
        multiplicity: u64,
    }

    fn graph_fixture() -> Vec<u8> {
        let nodes = [
            NodeFixture {
                id: 1,
                rank: 0,
                first_edge: 0,
                edge_count: 0,
                kind: 0,
                parameter: 0,
                exposure: [100, 0],
            },
            NodeFixture {
                id: 2,
                rank: 0,
                first_edge: 0,
                edge_count: 0,
                kind: 0,
                parameter: 1,
                exposure: [0, 100],
            },
            NodeFixture {
                id: 3,
                rank: 1,
                first_edge: 0,
                edge_count: 1,
                kind: 1,
                parameter: 10,
                exposure: [10, 0],
            },
            NodeFixture {
                id: 4,
                rank: 1,
                first_edge: 1,
                edge_count: 1,
                kind: 1,
                parameter: 10,
                exposure: [0, 10],
            },
            NodeFixture {
                id: 5,
                rank: 2,
                first_edge: 2,
                edge_count: 2,
                kind: 2,
                parameter: 0,
                exposure: [30, 70],
            },
        ];
        let edges = [
            EdgeFixture {
                child_id: 1,
                child_index: 0,
                multiplicity: 1,
            },
            EdgeFixture {
                child_id: 2,
                child_index: 1,
                multiplicity: 1,
            },
            EdgeFixture {
                child_id: 3,
                child_index: 2,
                multiplicity: 3,
            },
            EdgeFixture {
                child_id: 4,
                child_index: 3,
                multiplicity: 7,
            },
        ];
        let mut bytes = vec![
            0_u8;
            GRAPH_HEADER_BYTES
                + nodes.len() * GRAPH_NODE_BYTES
                + edges.len() * GRAPH_EDGE_BYTES
                + nodes.len() * 2 * SCALAR_BYTES
        ];
        put(&mut bytes, 0, &GRAPH_MAGIC_V2);
        put(&mut bytes, 8, &SCHEMA_VERSION_V2.to_le_bytes());
        put(&mut bytes, GRAPH_ID_OFFSET, &[6; 32]);
        put(&mut bytes, GRAPH_ROOT_ID_OFFSET, &[5; 32]);
        put_u32(&mut bytes, GRAPH_OUTCOME_COUNT_OFFSET, 2);
        put_u32(&mut bytes, GRAPH_NODE_COUNT_OFFSET, 5);
        put_u32(&mut bytes, GRAPH_EDGE_COUNT_OFFSET, 4);
        put_u64(&mut bytes, GRAPH_SCALE_OFFSET, 100);
        let node_start = GRAPH_HEADER_BYTES;
        for (index, node) in nodes.iter().enumerate() {
            let offset = node_start + index * GRAPH_NODE_BYTES;
            put(&mut bytes, offset + NODE_ID_OFFSET, &[node.id; 32]);
            put_u32(&mut bytes, offset + NODE_RANK_OFFSET, node.rank);
            put_u32(&mut bytes, offset + NODE_FIRST_EDGE_OFFSET, node.first_edge);
            put_u32(&mut bytes, offset + NODE_EDGE_COUNT_OFFSET, node.edge_count);
            *bytes.get_mut(offset + NODE_KIND_OFFSET).expect("kind") = node.kind;
            put_u64(&mut bytes, offset + NODE_PARAMETER_OFFSET, node.parameter);
        }
        let edge_start = node_start + nodes.len() * GRAPH_NODE_BYTES;
        for (index, edge) in edges.iter().enumerate() {
            let offset = edge_start + index * GRAPH_EDGE_BYTES;
            put(
                &mut bytes,
                offset + EDGE_CHILD_ID_OFFSET,
                &[edge.child_id; 32],
            );
            put_u32(
                &mut bytes,
                offset + EDGE_CHILD_INDEX_OFFSET,
                edge.child_index,
            );
            put_u64(
                &mut bytes,
                offset + EDGE_MULTIPLICITY_OFFSET,
                edge.multiplicity,
            );
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

    #[test]
    fn explicit_remainders_issue_unwrap_and_coalesce_exactly() {
        let bytes = structured_fixture();
        let projection = StructuredProjectionV2::decode(&bytes).expect("exact projection");
        assert_eq!(projection.denominator(), 10);
        assert_eq!(projection.receipt_supply(), 7);
        let issue = prepare_issue(projection, 1).expect("issue");
        assert_eq!(issue.post_receipt_supply(), 8);
        let effects: Vec<_> = issue.effects().collect();
        assert_eq!(
            effects,
            vec![
                Ok(StructuredCoordinateEffect {
                    outcome: 0,
                    shard_atoms: 3
                }),
                Ok(StructuredCoordinateEffect {
                    outcome: 1,
                    shard_atoms: 7
                }),
            ]
        );
        let unwrap = prepare_unwrap(projection, 2).expect("unwrap");
        assert_eq!(unwrap.post_receipt_supply(), 5);
        assert_eq!(coalesce(10, 9).expect("change").change_shards, 9);
        assert_eq!(
            coalesce(10, 10),
            Ok(Coalescing {
                input_shards: 10,
                native_claims: 1,
                change_shards: 0
            })
        );
    }

    #[test]
    fn denomination_and_reconstitution_preserve_joined_accounting() {
        let bytes = structured_fixture();
        let projection = StructuredProjectionV2::decode(&bytes).expect("exact projection");
        assert_eq!(
            prepare_denominate(projection, 0, 2),
            Ok(ShardCoordinateSuccessor {
                native_locked: 5,
                shard_supply: 50,
                explicit_free_shards: 29,
            })
        );
        assert_eq!(
            prepare_reconstitute(projection, 0, 0),
            Err(Error::ZeroQuantity)
        );
        assert_eq!(
            prepare_reconstitute(projection, 0, 1),
            Err(Error::InsufficientBalance)
        );
        let mut coalesced = bytes.clone();
        let free0 = STRUCTURED_HEADER_BYTES + 8 * 8;
        put_u64(&mut coalesced, free0, 19);
        let shards0 = STRUCTURED_HEADER_BYTES + 4 * 8;
        put_u64(&mut coalesced, shards0, 40);
        let native0 = STRUCTURED_HEADER_BYTES + 2 * 8;
        put_u64(&mut coalesced, native0, 4);
        let projection = StructuredProjectionV2::decode(&coalesced).expect("coalesced projection");
        assert_eq!(
            prepare_reconstitute(projection, 0, 1),
            Ok(ShardCoordinateSuccessor {
                native_locked: 3,
                shard_supply: 30,
                explicit_free_shards: 9,
            })
        );
    }

    #[test]
    fn hostile_hidden_rounding_overflow_and_width_refuse() {
        let mut hidden = structured_fixture();
        let free0 = STRUCTURED_HEADER_BYTES + 8 * 8;
        put_u64(&mut hidden, free0, 0);
        assert_eq!(
            StructuredProjectionV2::decode(&hidden),
            Err(Error::ExplicitRemainderMismatch)
        );
        let mut overflow = structured_fixture();
        put_u64(&mut overflow, STRUCTURED_DENOMINATOR_OFFSET, u64::MAX);
        assert_eq!(
            StructuredProjectionV2::decode(&overflow),
            Err(Error::ArithmeticOverflow)
        );
        let fixture = structured_fixture();
        let truncated = fixture
            .get(..STRUCTURED_HEADER_BYTES)
            .expect("header prefix exists");
        assert_eq!(
            StructuredProjectionV2::decode(truncated),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn canonical_dag_flattens_to_native_exposure() {
        let bytes = graph_fixture();
        let graph = RepresentationGraphV2::decode(&bytes, admission()).expect("valid graph");
        assert_eq!(graph.root_id(), [5; 32]);
        assert_eq!(graph.root_exposure(0), Ok(30));
        assert_eq!(graph.root_exposure(1), Ok(70));
    }

    #[test]
    fn finalized_descriptor_joins_exact_graph_root_and_coefficients() {
        let descriptor_bytes = descriptor_fixture();
        let descriptor =
            RepresentationDescriptorV2::decode(&descriptor_bytes, descriptor_admission())
                .expect("finalized descriptor");
        let graph_bytes = graph_fixture();
        let graph = RepresentationGraphV2::decode(&graph_bytes, admission()).expect("graph");
        descriptor
            .authenticate_graph(graph)
            .expect("exact payoff join");
        assert_eq!(descriptor.descriptor_id(), [8; 32]);
        assert_eq!(descriptor.graph_id(), [6; 32]);
        assert_eq!(descriptor.graph_digest(), [7; 32]);
        assert_eq!(descriptor.root_id(), [5; 32]);
        assert_eq!(descriptor.market_id(), [2; 32]);
        assert_eq!(descriptor.release_set_id(), [9; 32]);
        assert_eq!(descriptor.receipt_mint(), [3; 32]);
        assert_eq!(descriptor.token_program(), [10; 32]);
        assert_eq!(descriptor.representation_authority(), [11; 32]);
        assert_eq!(descriptor.outcome_count(), 2);
        assert_eq!(descriptor.denominator(), 10);
        assert_eq!(descriptor.coefficient(0), Ok(3));
        assert_eq!(descriptor.coefficient(1), Ok(7));
    }

    #[test]
    fn same_width_coefficient_graph_and_record_substitutions_refuse() {
        let graph_bytes = graph_fixture();
        let graph = RepresentationGraphV2::decode(&graph_bytes, admission()).expect("graph");
        let mut coefficient_substitution = descriptor_fixture();
        put_u64(&mut coefficient_substitution, DESCRIPTOR_HEADER_BYTES, 4);
        let descriptor =
            RepresentationDescriptorV2::decode(&coefficient_substitution, descriptor_admission())
                .expect("same-width descriptor");
        assert_eq!(
            descriptor.authenticate_graph(graph),
            Err(Error::DescriptorMismatch)
        );

        let mut graph_substitution = descriptor_fixture();
        put(
            &mut graph_substitution,
            DESCRIPTOR_GRAPH_ID_OFFSET,
            &[12; 32],
        );
        let descriptor =
            RepresentationDescriptorV2::decode(&graph_substitution, descriptor_admission())
                .expect("alternate graph selection");
        assert_eq!(
            descriptor.authenticate_graph(graph),
            Err(Error::DescriptorMismatch)
        );

        let mut hostile_admission = descriptor_admission();
        hostile_admission.finalized_descriptor_id = [13; 32];
        assert_eq!(
            RepresentationDescriptorV2::decode(&descriptor_fixture(), hostile_admission),
            Err(Error::ContentAdmissionMismatch)
        );
    }

    #[test]
    fn hostile_cycle_order_exposure_and_admission_refuse() {
        let bytes = graph_fixture();
        let edge_start = GRAPH_HEADER_BYTES + 5 * GRAPH_NODE_BYTES;
        let mut cycle = bytes.clone();
        let root_edge = edge_start + 2 * GRAPH_EDGE_BYTES;
        put(&mut cycle, root_edge + EDGE_CHILD_ID_OFFSET, &[5; 32]);
        put_u32(&mut cycle, root_edge + EDGE_CHILD_INDEX_OFFSET, 4);
        assert_eq!(
            RepresentationGraphV2::decode(&cycle, admission()),
            Err(Error::InvalidEdge)
        );

        let mut reversed = bytes.clone();
        put(&mut reversed, root_edge + EDGE_CHILD_ID_OFFSET, &[4; 32]);
        put_u32(&mut reversed, root_edge + EDGE_CHILD_INDEX_OFFSET, 3);
        let second = root_edge + GRAPH_EDGE_BYTES;
        put(&mut reversed, second + EDGE_CHILD_ID_OFFSET, &[3; 32]);
        put_u32(&mut reversed, second + EDGE_CHILD_INDEX_OFFSET, 2);
        assert_eq!(
            RepresentationGraphV2::decode(&reversed, admission()),
            Err(Error::InvalidEdge)
        );

        let mut forged = bytes.clone();
        let exposure_start = GRAPH_HEADER_BYTES + 5 * GRAPH_NODE_BYTES + 4 * GRAPH_EDGE_BYTES;
        put_u64(&mut forged, exposure_start + 8 * 8, 31);
        assert_eq!(
            RepresentationGraphV2::decode(&forged, admission()),
            Err(Error::ExposureMismatch)
        );

        let mut substituted = admission();
        substituted.finalized_graph_digest = [8; 32];
        assert_eq!(
            RepresentationGraphV2::decode(&bytes, substituted),
            Err(Error::ContentAdmissionMismatch)
        );
    }

    #[test]
    fn disconnected_and_duplicate_nodes_refuse() {
        let bytes = graph_fixture();
        let mut duplicate = bytes.clone();
        let second_node = GRAPH_HEADER_BYTES + GRAPH_NODE_BYTES;
        put(&mut duplicate, second_node + NODE_ID_OFFSET, &[1; 32]);
        assert_eq!(
            RepresentationGraphV2::decode(&duplicate, admission()),
            Err(Error::NonCanonicalNodeOrder)
        );

        let mut disconnected = bytes.clone();
        let edge_start = GRAPH_HEADER_BYTES + 5 * GRAPH_NODE_BYTES;
        let second_shard_edge = edge_start + GRAPH_EDGE_BYTES;
        put(
            &mut disconnected,
            second_shard_edge + EDGE_CHILD_ID_OFFSET,
            &[1; 32],
        );
        put_u32(
            &mut disconnected,
            second_shard_edge + EDGE_CHILD_INDEX_OFFSET,
            0,
        );
        assert_eq!(
            RepresentationGraphV2::decode(&disconnected, admission()),
            Err(Error::ExposureMismatch)
        );
    }
}
