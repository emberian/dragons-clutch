//! Exact General batch collection: the half that produces what settlement consumes.
//!
//! The seven settlement actions operate on a Candidate that names a `batch_id`
//! and on Execution rows that name an `order_id`.  Before this module both were
//! free 32-byte parameters: `batch_id` was a literal in every test, and
//! [`AuthenticatedOrderTermsV2`] -- the value the streamed verifier trusts for
//! `max_lots` and `max_quote_debit_per_lot` -- was constructed only by tests.
//! Nothing bound either to a batch that was opened or to an order a maker
//! actually placed.
//!
//! This module supplies the two missing immutable records and the three pure
//! transitions over them.  A batch is a *window*, not a ledger: it counts its
//! orders and bounds them, but it does not enumerate them.  Each order is an
//! independent content-addressed record, and the Candidate carries the exact
//! execution set naming them.  That is deliberately the weakest binding that
//! still refuses a substituted batch, a substituted order, or an order admitted
//! outside the window it was signed for.
//!
//! Authorization is not in this module.  These are pure record transitions; the
//! privileges that make them safe -- the maker signing their own order, the
//! writable root, the vacant successor PDA -- are declared by General's
//! AccountProfile and enforced by the family-neutral Trading executor.  See
//! `docs/decisions/0009-general-batch-collection.md`.

use dclutch_general_config_contract::root::{GeneralRootV2, RootError};
use sha2::{Digest, Sha256};

use crate::runtime_verify::AuthenticatedOrderTermsV2;
use crate::runtime_width::{CandidateHeaderV2, ExecutionHeaderV2};

/// Exact immutable batch prefix, whose digest is the canonical `batch_id`.
pub const GENERAL_BATCH_PREFIX_BYTES_V1: usize = 160;
/// Exact total batch record width, immutable prefix then mutable tail.
pub const GENERAL_BATCH_BYTES_V1: usize = 224;
/// Exact fixed bytes before one order's two runtime-width per-lot tails.
pub const GENERAL_ORDER_HEADER_BYTES_V1: usize = 160;

const BATCH_MAGIC: [u8; 8] = *b"DCGBAT01";
const ORDER_MAGIC: [u8; 8] = *b"DCGORD01";
const VERSION: u16 = 1;
const BATCH_PHASE: u8 = 20;
const ORDER_PHASE: u8 = 21;

const STATUS_COLLECTING: u8 = 1;
const STATUS_CLOSED: u8 = 2;

/// Canonical PDA seed domain for one General batch record.
pub const GENERAL_BATCH_PDA_DOMAIN_V1: &[u8] = b"dclutch-general-batch-v1";
/// Canonical PDA seed domain for one General order record.
pub const GENERAL_ORDER_PDA_DOMAIN_V1: &[u8] = b"dclutch-general-order-v1";

// Both domains are under the 32-byte Solana seed limit. M-32 in the aspiration
// ledger records a 33-byte General domain that made a whole transition dead at
// runtime; only Custody carried the assertion that would have caught it. These
// two carry it.
const _: () = assert!(
    GENERAL_BATCH_PDA_DOMAIN_V1.len() <= 32,
    "GENERAL_BATCH_PDA_DOMAIN_V1 must fit one Solana PDA seed"
);
const _: () = assert!(
    GENERAL_ORDER_PDA_DOMAIN_V1.len() <= 32,
    "GENERAL_ORDER_PDA_DOMAIN_V1 must fit one Solana PDA seed"
);

/// Stable refusal from General batch collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralCollectionErrorV1 {
    /// A byte slice had another exact width.
    InvalidLength,
    /// Magic, version, phase, or reserved bytes were noncanonical.
    InvalidHeader,
    /// A checked count, coordinate, or quantity calculation overflowed.
    ArithmeticOverflow,
    /// A required identity or coordinate was zero.
    ZeroIdentity,
    /// The status byte named no canonical batch status.
    InvalidStatus,
    /// The batch was not collecting when an order or a close was offered.
    NotCollecting,
    /// The batch was still collecting when a candidate was offered.
    NotClosed,
    /// The batch already holds its immutable maximum number of orders.
    BatchFull,
    /// A record belonged to another market, generation, batch, or width.
    Substitution,
    /// The maker could not reserve this order's exact worst-case obligation.
    Unfunded,
    /// The order does not outlive the window it must settle in.
    Expired,
    /// The batch collection window has closed for this slot.
    OutsideWindow,
    /// The root refused the batch-count transition.
    Root(RootError),
}

impl From<RootError> for GeneralCollectionErrorV1 {
    fn from(value: RootError) -> Self {
        Self::Root(value)
    }
}

/// Result alias for General batch collection.
pub type GeneralCollectionResultV1<T> = core::result::Result<T, GeneralCollectionErrorV1>;

/// Canonical batch status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BatchStatusV1 {
    /// Signed orders may still be admitted.
    Collecting = STATUS_COLLECTING,
    /// The order set is final and candidates may name this batch.
    Closed = STATUS_CLOSED,
}

impl BatchStatusV1 {
    fn decode(value: u8) -> GeneralCollectionResultV1<Self> {
        match value {
            STATUS_COLLECTING => Ok(Self::Collecting),
            STATUS_CLOSED => Ok(Self::Closed),
            _ => Err(GeneralCollectionErrorV1::InvalidStatus),
        }
    }

    /// Return the canonical one-byte status tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }
}

/// Immutable batch coordinates fixed when the batch opens.
///
/// Their digest is the `batch_id` every Candidate, verifier cursor and
/// selection cursor in the settlement half already carries. Fixing the identity
/// to exactly these fields -- and to none of the mutable counters -- is what
/// lets the identity be computed once, at open, and stay stable while orders
/// arrive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralBatchOpeningV1 {
    /// Runtime outcome width shared by every order and candidate.
    pub outcome_count: u32,
    /// Exact root batch sequence this batch consumed.
    pub sequence: u64,
    /// Immutable Market generation.
    pub generation: u64,
    /// Canonical Core Market account key.
    pub market: [u8; 32],
    /// Product content identity fixing the outcome domain.
    pub product_id: [u8; 32],
    /// Immutable `GeneralConfigV3` identity.
    pub config_id: [u8; 32],
    /// Sole exact denominator for every candidate price simplex.
    pub price_scale: u64,
    /// Slot at and after which no further order may be admitted.
    pub collection_close_slot: u64,
    /// Slot every admitted order must remain valid through.
    pub settlement_close_slot: u64,
    /// Immutable maximum number of admitted orders.
    pub max_orders: u32,
}

/// Mutable batch counters advanced by admission and closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralBatchStateV1 {
    /// Current canonical status.
    pub status: BatchStatusV1,
    /// Number of admitted orders.
    pub order_count: u32,
    /// Root revision consumed by the open.
    pub opened_root_revision: u64,
    /// Root revision consumed by the close; zero while collecting.
    pub closed_root_revision: u64,
    /// Sum of every admitted order's exact worst-case quote obligation.
    pub committed_quote_reserve: u64,
}

/// One complete General batch: immutable opening then mutable counters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralBatchV1 {
    opening: GeneralBatchOpeningV1,
    state: GeneralBatchStateV1,
}

impl GeneralBatchV1 {
    /// Open one batch against the live root, consuming its exact next sequence.
    ///
    /// This is the first non-test caller of [`GeneralRootV2::open_batch`]. The
    /// root's own replay guards -- exact revision and exact next sequence --
    /// remain the sole authority for whether the open may happen at all; this
    /// function refuses its own coordinates first so a refused batch never
    /// advances the root.
    pub fn open(
        root: &mut GeneralRootV2,
        opening: GeneralBatchOpeningV1,
        expected_revision: u64,
        current_slot: u64,
    ) -> GeneralCollectionResultV1<Self> {
        validate_opening(opening)?;
        if opening.sequence != root.next_batch_sequence()
            || opening.market != root.market()
            || opening.config_id != root.config_id()
            || opening.generation != root.generation()
        {
            return Err(GeneralCollectionErrorV1::Substitution);
        }
        if current_slot >= opening.collection_close_slot {
            return Err(GeneralCollectionErrorV1::OutsideWindow);
        }
        // Refuse before the root moves: `open_batch` is the only mutation here.
        root.open_batch(expected_revision, opening.sequence)?;
        Ok(Self {
            opening,
            state: GeneralBatchStateV1 {
                status: BatchStatusV1::Collecting,
                order_count: 0,
                opened_root_revision: expected_revision,
                closed_root_revision: 0,
                committed_quote_reserve: 0,
            },
        })
    }

    /// Hostile-decode one exact 224-byte batch record.
    pub fn decode(bytes: &[u8]) -> GeneralCollectionResultV1<Self> {
        if bytes.len() != GENERAL_BATCH_BYTES_V1 {
            return Err(GeneralCollectionErrorV1::InvalidLength);
        }
        require_header(bytes, &BATCH_MAGIC, BATCH_PHASE)?;
        require_zero(bytes, 148, 4)?;
        require_zero(bytes, 161, 3)?;
        require_zero(bytes, 192, 32)?;
        let opening = GeneralBatchOpeningV1 {
            outcome_count: read_u32(bytes, 12)?,
            sequence: read_u64(bytes, 16)?,
            generation: read_u64(bytes, 24)?,
            market: read_array(bytes, 32)?,
            product_id: read_array(bytes, 64)?,
            config_id: read_array(bytes, 96)?,
            price_scale: read_u64(bytes, 128)?,
            collection_close_slot: read_u64(bytes, 136)?,
            max_orders: read_u32(bytes, 144)?,
            settlement_close_slot: read_u64(bytes, 152)?,
        };
        validate_opening(opening)?;
        let state = GeneralBatchStateV1 {
            status: BatchStatusV1::decode(read_u8(bytes, 160)?)?,
            order_count: read_u32(bytes, 164)?,
            opened_root_revision: read_u64(bytes, 168)?,
            closed_root_revision: read_u64(bytes, 176)?,
            committed_quote_reserve: read_u64(bytes, 184)?,
        };
        let value = Self { opening, state };
        value.validate()?;
        Ok(value)
    }

    /// Encode the exact canonical batch layout.
    #[must_use]
    pub fn to_bytes(self) -> [u8; GENERAL_BATCH_BYTES_V1] {
        let mut output = [0_u8; GENERAL_BATCH_BYTES_V1];
        put(&mut output, 0, &BATCH_MAGIC);
        put(&mut output, 8, &VERSION.to_le_bytes());
        output[10] = BATCH_PHASE;
        put(&mut output, 12, &self.opening.outcome_count.to_le_bytes());
        put(&mut output, 16, &self.opening.sequence.to_le_bytes());
        put(&mut output, 24, &self.opening.generation.to_le_bytes());
        put(&mut output, 32, &self.opening.market);
        put(&mut output, 64, &self.opening.product_id);
        put(&mut output, 96, &self.opening.config_id);
        put(&mut output, 128, &self.opening.price_scale.to_le_bytes());
        put(
            &mut output,
            136,
            &self.opening.collection_close_slot.to_le_bytes(),
        );
        put(&mut output, 144, &self.opening.max_orders.to_le_bytes());
        put(
            &mut output,
            152,
            &self.opening.settlement_close_slot.to_le_bytes(),
        );
        output[160] = self.state.status.tag();
        put(&mut output, 164, &self.state.order_count.to_le_bytes());
        put(
            &mut output,
            168,
            &self.state.opened_root_revision.to_le_bytes(),
        );
        put(
            &mut output,
            176,
            &self.state.closed_root_revision.to_le_bytes(),
        );
        put(
            &mut output,
            184,
            &self.state.committed_quote_reserve.to_le_bytes(),
        );
        output
    }

    /// Validate every cross-field invariant of one complete batch.
    pub fn validate(self) -> GeneralCollectionResultV1<()> {
        validate_opening(self.opening)?;
        if self.state.order_count > self.opening.max_orders {
            return Err(GeneralCollectionErrorV1::BatchFull);
        }
        match self.state.status {
            BatchStatusV1::Collecting => {
                if self.state.closed_root_revision != 0 {
                    return Err(GeneralCollectionErrorV1::InvalidStatus);
                }
            }
            BatchStatusV1::Closed => {
                if self.state.closed_root_revision <= self.state.opened_root_revision {
                    return Err(GeneralCollectionErrorV1::InvalidStatus);
                }
            }
        }
        if self.state.opened_root_revision == 0 {
            return Err(GeneralCollectionErrorV1::ZeroIdentity);
        }
        Ok(())
    }

    /// Return the canonical `batch_id`: the digest of the immutable prefix.
    ///
    /// Only the prefix is hashed, so the identity a Candidate carries is fixed
    /// at open and cannot be moved by admitting an order.
    #[must_use]
    pub fn batch_id(self) -> [u8; 32] {
        let bytes = self.to_bytes();
        let mut hasher = Sha256::new();
        hasher.update(&bytes[..GENERAL_BATCH_PREFIX_BYTES_V1]);
        hasher.finalize().into()
    }

    /// Admit one signed order, reserving its exact worst-case obligation.
    ///
    /// The order must already be bound to this exact batch identity; the maker's
    /// authorization is the transaction signature the AccountProfile requires at
    /// the owner coordinate, not a field of the record.
    pub fn admit(
        &mut self,
        order: GeneralOrderV1<'_>,
        funding: MakerFundingV1<'_>,
        current_slot: u64,
    ) -> GeneralCollectionResultV1<[u8; 32]> {
        if self.state.status != BatchStatusV1::Collecting {
            return Err(GeneralCollectionErrorV1::NotCollecting);
        }
        if current_slot >= self.opening.collection_close_slot {
            return Err(GeneralCollectionErrorV1::OutsideWindow);
        }
        let header = order.header();
        if header.batch_id != self.batch_id()
            || header.market != self.opening.market
            || header.generation != self.opening.generation
            || header.outcome_count != self.opening.outcome_count
        {
            return Err(GeneralCollectionErrorV1::Substitution);
        }
        // An order that expires before settlement closes is a promise the batch
        // cannot keep: the candidate that fills it may legitimately settle at
        // any slot up to the window's end.
        if header.valid_until_slot < self.opening.settlement_close_slot {
            return Err(GeneralCollectionErrorV1::Expired);
        }
        if self.state.order_count >= self.opening.max_orders {
            return Err(GeneralCollectionErrorV1::BatchFull);
        }
        let quote_reserve = order.quote_reserve()?;
        if funding.owner_id != header.owner_id
            || funding.available_quote < quote_reserve
            || funding.available_claims.len() != usize_from_u32(self.opening.outcome_count)?
        {
            return Err(GeneralCollectionErrorV1::Unfunded);
        }
        for outcome in 0..self.opening.outcome_count {
            let required = order.claim_reserve(outcome)?;
            let available = *funding
                .available_claims
                .get(usize_from_u32(outcome)?)
                .ok_or(GeneralCollectionErrorV1::InvalidLength)?;
            if available < required {
                return Err(GeneralCollectionErrorV1::Unfunded);
            }
        }
        let next_count = self
            .state
            .order_count
            .checked_add(1)
            .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)?;
        let next_reserve = self
            .state
            .committed_quote_reserve
            .checked_add(quote_reserve)
            .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)?;
        self.state.order_count = next_count;
        self.state.committed_quote_reserve = next_reserve;
        Ok(order.order_id())
    }

    /// Close the batch against the live root, making its order set final.
    ///
    /// This is the first non-test caller of [`GeneralRootV2::close_batch`].
    pub fn close(
        &mut self,
        root: &mut GeneralRootV2,
        expected_revision: u64,
    ) -> GeneralCollectionResultV1<[u8; 32]> {
        if self.state.status != BatchStatusV1::Collecting {
            return Err(GeneralCollectionErrorV1::NotCollecting);
        }
        if self.opening.market != root.market() || self.opening.generation != root.generation() {
            return Err(GeneralCollectionErrorV1::Substitution);
        }
        let next_revision = expected_revision
            .checked_add(1)
            .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)?;
        root.close_batch(expected_revision)?;
        self.state.status = BatchStatusV1::Closed;
        self.state.closed_root_revision = next_revision;
        Ok(self.batch_id())
    }

    /// Whether closing this batch truncates no maker's opportunity to place.
    ///
    /// A full batch can admit nothing further, so closing it early takes nothing
    /// from anyone. That is the one condition under which a permissionless close
    /// before `collection_close_slot` is not a griefing vector.
    #[must_use]
    pub const fn close_is_permissionless(self, current_slot: u64) -> bool {
        current_slot >= self.opening.collection_close_slot
            || self.state.order_count >= self.opening.max_orders
    }

    /// Immutable opening coordinates.
    #[must_use]
    pub const fn opening(self) -> GeneralBatchOpeningV1 {
        self.opening
    }

    /// Mutable counters.
    #[must_use]
    pub const fn state(self) -> GeneralBatchStateV1 {
        self.state
    }
}

/// One maker's authenticated reservable balance at admission.
///
/// This value is not an authority. The physical layer produces it from accounts
/// its AccountProfile authenticated; this module only compares it against the
/// order's exact worst case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MakerFundingV1<'a> {
    /// Maker identity the balances belong to.
    pub owner_id: [u8; 32],
    /// Reservable quote atoms.
    pub available_quote: u64,
    /// Reservable claim atoms, one per runtime outcome.
    pub available_claims: &'a [u64],
}

/// Fixed fields of one immutable signed portfolio order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOrderHeaderV1 {
    /// Runtime outcome width.
    pub outcome_count: u32,
    /// Owner-scoped replay nonce.
    pub nonce: u64,
    /// Maker identity; the account that must sign the placement.
    pub owner_id: [u8; 32],
    /// Canonical Core Market account key.
    pub market: [u8; 32],
    /// Exact immutable identity of the batch this order may execute in.
    pub batch_id: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Candidate-wide maximum fill.
    pub max_lots: u64,
    /// Candidate-wide maximum derived quote debit per filled lot.
    pub max_quote_debit_per_lot: u64,
    /// Last slot at which this order may still be settled.
    pub valid_until_slot: u64,
}

/// Borrowed immutable order record with two `u64[N]` per-lot tails.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOrderV1<'a> {
    bytes: &'a [u8],
    header: GeneralOrderHeaderV1,
}

impl<'a> GeneralOrderV1<'a> {
    /// Hostile-decode one exact `160 + 16N` order record.
    pub fn decode(bytes: &'a [u8]) -> GeneralCollectionResultV1<Self> {
        if bytes.len() < GENERAL_ORDER_HEADER_BYTES_V1 {
            return Err(GeneralCollectionErrorV1::InvalidLength);
        }
        require_header(bytes, &ORDER_MAGIC, ORDER_PHASE)?;
        let header = GeneralOrderHeaderV1 {
            outcome_count: read_u32(bytes, 12)?,
            nonce: read_u64(bytes, 16)?,
            owner_id: read_array(bytes, 32)?,
            market: read_array(bytes, 64)?,
            batch_id: read_array(bytes, 96)?,
            generation: read_u64(bytes, 128)?,
            max_lots: read_u64(bytes, 136)?,
            max_quote_debit_per_lot: read_u64(bytes, 144)?,
            valid_until_slot: read_u64(bytes, 152)?,
        };
        if bytes.len() != general_order_len_v1(header.outcome_count)? {
            return Err(GeneralCollectionErrorV1::InvalidLength);
        }
        validate_order_header(header)?;
        let value = Self { bytes, header };
        // A degenerate order that moves no claim in either direction is not a
        // portfolio order; it would occupy a coordinate and a rent-bearing
        // account while being unfillable.
        if !value.has_claim_movement()? {
            return Err(GeneralCollectionErrorV1::ZeroIdentity);
        }
        Ok(value)
    }

    /// Encode canonical order bytes into a caller-owned exact-width buffer.
    pub fn encode_into(
        header: GeneralOrderHeaderV1,
        receive_per_lot: &[u64],
        deliver_per_lot: &[u64],
        output: &mut [u8],
    ) -> GeneralCollectionResultV1<()> {
        validate_order_header(header)?;
        let count = usize_from_u32(header.outcome_count)?;
        if receive_per_lot.len() != count || deliver_per_lot.len() != count {
            return Err(GeneralCollectionErrorV1::InvalidLength);
        }
        if output.len() != general_order_len_v1(header.outcome_count)? {
            return Err(GeneralCollectionErrorV1::InvalidLength);
        }
        output.fill(0);
        put(output, 0, &ORDER_MAGIC);
        put(output, 8, &VERSION.to_le_bytes());
        output[10] = ORDER_PHASE;
        put(output, 12, &header.outcome_count.to_le_bytes());
        put(output, 16, &header.nonce.to_le_bytes());
        put(output, 32, &header.owner_id);
        put(output, 64, &header.market);
        put(output, 96, &header.batch_id);
        put(output, 128, &header.generation.to_le_bytes());
        put(output, 136, &header.max_lots.to_le_bytes());
        put(output, 144, &header.max_quote_debit_per_lot.to_le_bytes());
        put(output, 152, &header.valid_until_slot.to_le_bytes());
        let deliver_base = deliver_tail_base(header.outcome_count)?;
        for outcome in 0..count {
            let offset = outcome
                .checked_mul(8)
                .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)?;
            put(
                output,
                GENERAL_ORDER_HEADER_BYTES_V1
                    .checked_add(offset)
                    .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)?,
                &receive_per_lot[outcome].to_le_bytes(),
            );
            put(
                output,
                deliver_base
                    .checked_add(offset)
                    .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)?,
                &deliver_per_lot[outcome].to_le_bytes(),
            );
        }
        // Encode then hostile-decode our own candidate: the same total-function
        // discipline the three artifact encoders carry.
        decode_checked(output)
    }

    /// Return fixed order coordinates.
    #[must_use]
    pub const fn header(self) -> GeneralOrderHeaderV1 {
        self.header
    }

    /// Return the canonical `order_id`: the digest of the whole record.
    #[must_use]
    pub fn order_id(self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.bytes);
        hasher.finalize().into()
    }

    /// Return one exact claim quantity received per filled lot.
    pub fn receive_per_lot(self, index: u32) -> GeneralCollectionResultV1<u64> {
        self.tail(GENERAL_ORDER_HEADER_BYTES_V1, index)
    }

    /// Return one exact claim quantity delivered per filled lot.
    pub fn deliver_per_lot(self, index: u32) -> GeneralCollectionResultV1<u64> {
        self.tail(deliver_tail_base(self.header.outcome_count)?, index)
    }

    /// Exact worst-case quote obligation if this order fills completely.
    pub fn quote_reserve(self) -> GeneralCollectionResultV1<u64> {
        self.header
            .max_quote_debit_per_lot
            .checked_mul(self.header.max_lots)
            .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)
    }

    /// Exact worst-case claim obligation at one outcome if this order fills.
    pub fn claim_reserve(self, index: u32) -> GeneralCollectionResultV1<u64> {
        self.deliver_per_lot(index)?
            .checked_mul(self.header.max_lots)
            .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)
    }

    /// Project the exact terms the streamed candidate verifier consumes.
    ///
    /// This is the join the collection half exists to supply.  Before it,
    /// [`AuthenticatedOrderTermsV2`] had no producer outside tests, so the
    /// verifier's `max_lots` and quote-limit discipline rested on a value the
    /// caller simply asserted.  Here the terms are a projection of a record
    /// whose own digest is the `order_id` they carry.
    #[must_use]
    pub fn terms(self) -> AuthenticatedOrderTermsV2 {
        AuthenticatedOrderTermsV2 {
            order_id: self.order_id(),
            owner_id: self.header.owner_id,
            nonce: self.header.nonce,
            max_lots: self.header.max_lots,
            max_quote_debit_per_lot: self.header.max_quote_debit_per_lot,
        }
    }

    /// Return exact canonical order bytes.
    #[must_use]
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    fn tail(self, base: usize, index: u32) -> GeneralCollectionResultV1<u64> {
        if index >= self.header.outcome_count {
            return Err(GeneralCollectionErrorV1::InvalidLength);
        }
        let offset = base
            .checked_add(
                usize_from_u32(index)?
                    .checked_mul(8)
                    .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)?,
            )
            .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)?;
        read_u64(self.bytes, offset)
    }

    fn has_claim_movement(self) -> GeneralCollectionResultV1<bool> {
        for outcome in 0..self.header.outcome_count {
            if self.receive_per_lot(outcome)? != 0 || self.deliver_per_lot(outcome)? != 0 {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Return exact `160 + 16N` bytes for one order record.
pub fn general_order_len_v1(outcome_count: u32) -> GeneralCollectionResultV1<usize> {
    if outcome_count == 0 {
        return Err(GeneralCollectionErrorV1::InvalidLength);
    }
    usize_from_u32(outcome_count)?
        .checked_mul(16)
        .and_then(|tail| GENERAL_ORDER_HEADER_BYTES_V1.checked_add(tail))
        .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)
}

/// Require one Candidate to name exactly this closed batch's domain.
///
/// The settlement half already compares `batch_id` between its own records; what
/// it could not do was check that the identity denotes a batch that was opened
/// and closed at all.  Because the identity commits to market, generation,
/// product and price scale, one digest comparison decides substitution across
/// all four at once.
pub fn authenticate_batch_candidate_v1(
    batch: GeneralBatchV1,
    candidate: CandidateHeaderV2,
) -> GeneralCollectionResultV1<()> {
    if batch.state().status != BatchStatusV1::Closed {
        return Err(GeneralCollectionErrorV1::NotClosed);
    }
    let opening = batch.opening();
    if candidate.batch_id != batch.batch_id()
        || candidate.product_id != opening.product_id
        || candidate.outcome_count != opening.outcome_count
        || candidate.price_scale != opening.price_scale
    {
        return Err(GeneralCollectionErrorV1::Substitution);
    }
    Ok(())
}

/// Authenticate one Execution row against the immutable order it names.
///
/// Returns the exact [`AuthenticatedOrderTermsV2`] the streamed verifier
/// requires.  Every compact field the row repeats is checked against the record,
/// and the record's own digest is checked against the `order_id` the row claims,
/// so a row cannot import terms from an order it does not name.
pub fn authenticate_order_execution_v1(
    batch: GeneralBatchV1,
    order: GeneralOrderV1<'_>,
    execution: ExecutionHeaderV2,
) -> GeneralCollectionResultV1<AuthenticatedOrderTermsV2> {
    if batch.state().status != BatchStatusV1::Closed {
        return Err(GeneralCollectionErrorV1::NotClosed);
    }
    let header = order.header();
    if header.batch_id != batch.batch_id()
        || header.outcome_count != execution.outcome_count
        || header.owner_id != execution.owner_id
        || header.nonce != execution.nonce
        || header.max_lots != execution.max_lots
    {
        return Err(GeneralCollectionErrorV1::Substitution);
    }
    let terms = order.terms();
    if terms.order_id != execution.order_id {
        return Err(GeneralCollectionErrorV1::Substitution);
    }
    if execution.lots == 0 || execution.lots > header.max_lots {
        return Err(GeneralCollectionErrorV1::Substitution);
    }
    Ok(terms)
}

/// Hostile-decode a candidate encoding without borrowing it into the result.
fn decode_checked(bytes: &[u8]) -> GeneralCollectionResultV1<()> {
    GeneralOrderV1::decode(bytes).map(|_| ())
}

fn validate_opening(opening: GeneralBatchOpeningV1) -> GeneralCollectionResultV1<()> {
    if is_zero(&opening.market) || is_zero(&opening.product_id) || is_zero(&opening.config_id) {
        return Err(GeneralCollectionErrorV1::ZeroIdentity);
    }
    if opening.outcome_count == 0
        || opening.price_scale == 0
        || opening.max_orders == 0
        || opening.generation == 0
    {
        return Err(GeneralCollectionErrorV1::ZeroIdentity);
    }
    if opening.settlement_close_slot <= opening.collection_close_slot {
        return Err(GeneralCollectionErrorV1::OutsideWindow);
    }
    Ok(())
}

fn validate_order_header(header: GeneralOrderHeaderV1) -> GeneralCollectionResultV1<()> {
    if is_zero(&header.owner_id) || is_zero(&header.market) || is_zero(&header.batch_id) {
        return Err(GeneralCollectionErrorV1::ZeroIdentity);
    }
    if header.outcome_count == 0 || header.max_lots == 0 || header.generation == 0 {
        return Err(GeneralCollectionErrorV1::ZeroIdentity);
    }
    Ok(())
}

fn deliver_tail_base(outcome_count: u32) -> GeneralCollectionResultV1<usize> {
    usize_from_u32(outcome_count)?
        .checked_mul(8)
        .and_then(|tail| GENERAL_ORDER_HEADER_BYTES_V1.checked_add(tail))
        .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)
}

fn require_header(bytes: &[u8], magic: &[u8; 8], phase: u8) -> GeneralCollectionResultV1<()> {
    if bytes.get(..8) != Some(magic.as_slice())
        || read_u16(bytes, 8)? != VERSION
        || read_u8(bytes, 10)? != phase
        || read_u8(bytes, 11)? != 0
    {
        return Err(GeneralCollectionErrorV1::InvalidHeader);
    }
    Ok(())
}

fn require_zero(bytes: &[u8], offset: usize, length: usize) -> GeneralCollectionResultV1<()> {
    let end = offset
        .checked_add(length)
        .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)?;
    if bytes
        .get(offset..end)
        .ok_or(GeneralCollectionErrorV1::InvalidLength)?
        .iter()
        .all(|value| *value == 0)
    {
        Ok(())
    } else {
        Err(GeneralCollectionErrorV1::InvalidHeader)
    }
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn usize_from_u32(value: u32) -> GeneralCollectionResultV1<usize> {
    usize::try_from(value).map_err(|_| GeneralCollectionErrorV1::ArithmeticOverflow)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset.saturating_add(value.len())) {
        target.copy_from_slice(value);
    }
}

fn read_u8(bytes: &[u8], offset: usize) -> GeneralCollectionResultV1<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(GeneralCollectionErrorV1::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> GeneralCollectionResultV1<u16> {
    Ok(u16::from_le_bytes(read_fixed::<2>(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> GeneralCollectionResultV1<u32> {
    Ok(u32::from_le_bytes(read_fixed::<4>(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> GeneralCollectionResultV1<u64> {
    Ok(u64::from_le_bytes(read_fixed::<8>(bytes, offset)?))
}

fn read_array(bytes: &[u8], offset: usize) -> GeneralCollectionResultV1<[u8; 32]> {
    read_fixed::<32>(bytes, offset)
}

fn read_fixed<const WIDTH: usize>(
    bytes: &[u8],
    offset: usize,
) -> GeneralCollectionResultV1<[u8; WIDTH]> {
    let end = offset
        .checked_add(WIDTH)
        .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)?;
    let slice = bytes
        .get(offset..end)
        .ok_or(GeneralCollectionErrorV1::InvalidLength)?;
    <[u8; WIDTH]>::try_from(slice).map_err(|_| GeneralCollectionErrorV1::InvalidLength)
}

#[cfg(test)]
mod tests;
