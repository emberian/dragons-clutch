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
use dclutch_sha256_adapter::{digest, digestv};

use crate::runtime_verify::AuthenticatedOrderTermsV2;
use crate::runtime_width::{CandidateHeaderV2, ExecutionV2, VerifiedCandidateHeaderV2};

/// Exact immutable batch prefix, whose digest is the canonical `batch_id`.
pub const GENERAL_BATCH_PREFIX_BYTES_V1: usize = 160;
/// Exact total batch record width, immutable prefix then mutable tail.
pub const GENERAL_BATCH_BYTES_V1: usize = 224;
/// Exact immutable fixed-header bytes of one order record.
pub const GENERAL_ORDER_HEADER_BYTES_V1: usize = 160;
/// Exact mutable escrow-state window between the header and the per-lot rows.
pub const GENERAL_ORDER_STATE_BYTES_V1: usize = 32;
/// Exact fixed offset of the mutable escrow-state window.
///
/// THE WIRE REPAIR THIS IS: the state block used to trail the two runtime-width
/// per-lot tails at `160 + 16N`, an offset no fixed-offset EffectProgram write
/// can address because `N` is a runtime width. Every mutable byte now lives at
/// a fixed coordinate, and the runtime-width rows follow it. The identity
/// digest masks exactly this window -- see [`general_order_identity_v1`] --
/// the same construction [`crate::candidate_v1::general_candidate_identity_v1`]
/// uses for a self-describing record.
pub const GENERAL_ORDER_STATE_OFFSET_V1: usize = GENERAL_ORDER_HEADER_BYTES_V1;
/// Exact fixed offset of the first per-outcome `(receive, deliver)` row.
pub const GENERAL_ORDER_ROW_BASE_V1: usize =
    GENERAL_ORDER_STATE_OFFSET_V1 + GENERAL_ORDER_STATE_BYTES_V1;
/// Exact byte stride of one per-outcome `(receive, deliver)` row.
///
/// The two per-lot quantities are INTERLEAVED per outcome rather than laid out
/// as two whole tails, because a second tail would begin at `base + 8N` -- a
/// runtime-width offset again -- while an interleaved row gives both fields a
/// fixed base and a fixed stride, which is exactly the shape one affine
/// per-item EffectProgram write can produce.
pub const GENERAL_ORDER_ROW_STRIDE_V1: usize = 16;
/// Offset of `receive_per_lot` inside one per-outcome row.
pub const GENERAL_ORDER_ROW_RECEIVE_OFFSET_V1: usize = 0;
/// Offset of `deliver_per_lot` inside one per-outcome row.
pub const GENERAL_ORDER_ROW_DELIVER_OFFSET_V1: usize = 8;

const BATCH_MAGIC: [u8; 8] = *b"DCGBAT01";
const ORDER_MAGIC: [u8; 8] = *b"DCGORD01";
const VERSION: u16 = 1;
const BATCH_PHASE: u8 = 20;
const ORDER_PHASE: u8 = 21;

const STATUS_COLLECTING: u8 = 1;
const STATUS_CLOSED: u8 = 2;

const ORDER_PHASE_PLACED: u8 = 1;
const ORDER_PHASE_CANCELLED: u8 = 2;
const ORDER_PHASE_RELEASED: u8 = 3;

/// Canonical byte coordinates of one batch record.
///
/// The hostile decoder and encoder below are the authority for accepting and
/// producing the complete wire; these exist so the OpenBatch/CloseBatch
/// artifact builders can write the same bytes without restating the layout,
/// and both codec directions read them so a moved field moves everywhere.
pub struct GeneralBatchLayoutV1;

impl GeneralBatchLayoutV1 {
    /// Record magic.
    pub const MAGIC: usize = 0;
    /// Record ABI version.
    pub const VERSION: usize = 8;
    /// Record phase byte.
    pub const PHASE: usize = 10;
    /// Runtime outcome width.
    pub const OUTCOME_COUNT: usize = 12;
    /// Exact root batch sequence consumed by the open.
    pub const SEQUENCE: usize = 16;
    /// Immutable Market generation.
    pub const GENERATION: usize = 24;
    /// Canonical Core Market key.
    pub const MARKET: usize = 32;
    /// Product content identity.
    pub const PRODUCT_ID: usize = 64;
    /// Immutable `GeneralConfigV3` identity.
    pub const CONFIG_ID: usize = 96;
    /// Sole candidate simplex denominator.
    pub const PRICE_SCALE: usize = 128;
    /// Admission window close slot.
    pub const COLLECTION_CLOSE_SLOT: usize = 136;
    /// Immutable admission bound.
    pub const MAX_ORDERS: usize = 144;
    /// Settlement window close slot.
    pub const SETTLEMENT_CLOSE_SLOT: usize = 152;
    /// Mutable status byte.
    pub const STATUS: usize = 160;
    /// Mutable admitted-order count.
    pub const ORDER_COUNT: usize = 164;
    /// Root revision consumed by the open.
    pub const OPENED_ROOT_REVISION: usize = 168;
    /// Root revision consumed by the close; zero while collecting.
    pub const CLOSED_ROOT_REVISION: usize = 176;
    /// Exact escrowed quote of every live admitted order.
    pub const COMMITTED_QUOTE_RESERVE: usize = 184;
    /// Mutable cancelled-order count.
    pub const CANCELLED_COUNT: usize = 192;

    /// Little-endian record magic as one register-width word.
    #[must_use]
    pub const fn magic_u64() -> u64 {
        u64::from_le_bytes(BATCH_MAGIC)
    }

    /// Exact record ABI version.
    #[must_use]
    pub const fn version_value() -> u16 {
        VERSION
    }

    /// Exact record phase byte.
    #[must_use]
    pub const fn phase_value() -> u8 {
        BATCH_PHASE
    }
}

/// Canonical byte coordinates of one order record.
///
/// The hostile decoder and encoder below are the authority for accepting and
/// producing the complete wire; these exist so the order-action artifact
/// builders can write the same bytes without restating the layout, and both
/// codec directions read them so a moved field moves everywhere.
pub struct GeneralOrderLayoutV1;

impl GeneralOrderLayoutV1 {
    /// Record magic.
    pub const MAGIC: usize = 0;
    /// Record ABI version.
    pub const VERSION: usize = 8;
    /// Record phase byte.
    pub const PHASE: usize = 10;
    /// Runtime outcome width.
    pub const OUTCOME_COUNT: usize = 12;
    /// Owner-scoped replay nonce.
    pub const NONCE: usize = 16;
    /// Maker identity; the account that must sign the placement.
    pub const OWNER_ID: usize = 32;
    /// Canonical Core Market key.
    pub const MARKET: usize = 64;
    /// Exact immutable identity of the batch this order may execute in.
    pub const BATCH_ID: usize = 96;
    /// Immutable Market generation.
    pub const GENERATION: usize = 128;
    /// Candidate-wide maximum fill.
    pub const MAX_LOTS: usize = 136;
    /// Candidate-wide maximum derived quote debit per filled lot.
    pub const MAX_QUOTE_DEBIT_PER_LOT: usize = 144;
    /// Last slot at which this order may still be settled.
    pub const VALID_UNTIL_SLOT: usize = 152;
    /// Mutable escrow phase byte.
    pub const STATE_PHASE: usize = GENERAL_ORDER_STATE_OFFSET_V1;
    /// Mutable admission slot.
    pub const STATE_ADMITTED_SLOT: usize = GENERAL_ORDER_STATE_OFFSET_V1 + 8;
    /// Mutable release slot; zero while placed.
    pub const STATE_RELEASED_SLOT: usize = GENERAL_ORDER_STATE_OFFSET_V1 + 16;

    /// Little-endian record magic as one register-width word.
    #[must_use]
    pub const fn magic_u64() -> u64 {
        u64::from_le_bytes(ORDER_MAGIC)
    }

    /// Exact record ABI version.
    #[must_use]
    pub const fn version_value() -> u16 {
        VERSION
    }

    /// Exact record phase byte.
    #[must_use]
    pub const fn phase_value() -> u8 {
        ORDER_PHASE
    }
}

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
    /// The order phase did not admit this escrow transition.
    InvalidOrderPhase,
    /// A cancellation was offered by an identity that is not the maker.
    NotTheMaker,
    /// The escrow could not fund the movement an authenticated row requires.
    EscrowShortfall,
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

/// Mutable batch counters advanced by admission, cancellation and closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralBatchStateV1 {
    /// Current canonical status.
    pub status: BatchStatusV1,
    /// Number of admitted orders. Cancellation never returns a coordinate:
    /// the envelope counts admissions, so a maker cannot churn placements to
    /// exhaust another maker's opportunity and then recover the slot.
    pub order_count: u32,
    /// Root revision consumed by the open.
    pub opened_root_revision: u64,
    /// Root revision consumed by the close; zero while collecting.
    pub closed_root_revision: u64,
    /// Exact escrowed quote of every live admitted order.
    ///
    /// Admission adds one order's worst case; cancellation removes exactly what
    /// that order escrowed. Because escrow is real -- the atoms moved into the
    /// order's own Custody vault at admission -- this counter is the sum of
    /// balances actually held, not a promise, and
    /// [`authenticate_batch_verified_candidate_v1`] bounds a candidate's whole
    /// quote debit by it.
    pub committed_quote_reserve: u64,
    /// Number of admitted orders whose maker cancelled before the close.
    pub cancelled_count: u32,
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
                cancelled_count: 0,
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
        require_zero(bytes, 196, 28)?;
        let opening = GeneralBatchOpeningV1 {
            outcome_count: read_u32(bytes, GeneralBatchLayoutV1::OUTCOME_COUNT)?,
            sequence: read_u64(bytes, GeneralBatchLayoutV1::SEQUENCE)?,
            generation: read_u64(bytes, GeneralBatchLayoutV1::GENERATION)?,
            market: read_array(bytes, GeneralBatchLayoutV1::MARKET)?,
            product_id: read_array(bytes, GeneralBatchLayoutV1::PRODUCT_ID)?,
            config_id: read_array(bytes, GeneralBatchLayoutV1::CONFIG_ID)?,
            price_scale: read_u64(bytes, GeneralBatchLayoutV1::PRICE_SCALE)?,
            collection_close_slot: read_u64(bytes, GeneralBatchLayoutV1::COLLECTION_CLOSE_SLOT)?,
            max_orders: read_u32(bytes, GeneralBatchLayoutV1::MAX_ORDERS)?,
            settlement_close_slot: read_u64(bytes, GeneralBatchLayoutV1::SETTLEMENT_CLOSE_SLOT)?,
        };
        validate_opening(opening)?;
        let state = GeneralBatchStateV1 {
            status: BatchStatusV1::decode(read_u8(bytes, GeneralBatchLayoutV1::STATUS)?)?,
            order_count: read_u32(bytes, GeneralBatchLayoutV1::ORDER_COUNT)?,
            opened_root_revision: read_u64(bytes, GeneralBatchLayoutV1::OPENED_ROOT_REVISION)?,
            closed_root_revision: read_u64(bytes, GeneralBatchLayoutV1::CLOSED_ROOT_REVISION)?,
            committed_quote_reserve: read_u64(
                bytes,
                GeneralBatchLayoutV1::COMMITTED_QUOTE_RESERVE,
            )?,
            cancelled_count: read_u32(bytes, GeneralBatchLayoutV1::CANCELLED_COUNT)?,
        };
        let value = Self { opening, state };
        value.validate()?;
        Ok(value)
    }

    /// Encode the exact canonical batch layout.
    #[must_use]
    pub fn to_bytes(self) -> [u8; GENERAL_BATCH_BYTES_V1] {
        let mut output = [0_u8; GENERAL_BATCH_BYTES_V1];
        put(&mut output, GeneralBatchLayoutV1::MAGIC, &BATCH_MAGIC);
        put(
            &mut output,
            GeneralBatchLayoutV1::VERSION,
            &VERSION.to_le_bytes(),
        );
        output[GeneralBatchLayoutV1::PHASE] = BATCH_PHASE;
        put(
            &mut output,
            GeneralBatchLayoutV1::OUTCOME_COUNT,
            &self.opening.outcome_count.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralBatchLayoutV1::SEQUENCE,
            &self.opening.sequence.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralBatchLayoutV1::GENERATION,
            &self.opening.generation.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralBatchLayoutV1::MARKET,
            &self.opening.market,
        );
        put(
            &mut output,
            GeneralBatchLayoutV1::PRODUCT_ID,
            &self.opening.product_id,
        );
        put(
            &mut output,
            GeneralBatchLayoutV1::CONFIG_ID,
            &self.opening.config_id,
        );
        put(
            &mut output,
            GeneralBatchLayoutV1::PRICE_SCALE,
            &self.opening.price_scale.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralBatchLayoutV1::COLLECTION_CLOSE_SLOT,
            &self.opening.collection_close_slot.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralBatchLayoutV1::MAX_ORDERS,
            &self.opening.max_orders.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralBatchLayoutV1::SETTLEMENT_CLOSE_SLOT,
            &self.opening.settlement_close_slot.to_le_bytes(),
        );
        output[GeneralBatchLayoutV1::STATUS] = self.state.status.tag();
        put(
            &mut output,
            GeneralBatchLayoutV1::ORDER_COUNT,
            &self.state.order_count.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralBatchLayoutV1::OPENED_ROOT_REVISION,
            &self.state.opened_root_revision.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralBatchLayoutV1::CLOSED_ROOT_REVISION,
            &self.state.closed_root_revision.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralBatchLayoutV1::COMMITTED_QUOTE_RESERVE,
            &self.state.committed_quote_reserve.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralBatchLayoutV1::CANCELLED_COUNT,
            &self.state.cancelled_count.to_le_bytes(),
        );
        output
    }

    /// Validate every cross-field invariant of one complete batch.
    pub fn validate(self) -> GeneralCollectionResultV1<()> {
        validate_opening(self.opening)?;
        if self.state.order_count > self.opening.max_orders
            || self.state.cancelled_count > self.state.order_count
        {
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
        digest(
            bytes
                .get(..GENERAL_BATCH_PREFIX_BYTES_V1)
                .unwrap_or_default(),
        )
    }

    /// Admit one signed order, ESCROWING its exact worst-case obligation.
    ///
    /// The order must already be bound to this exact batch identity; the maker's
    /// authorization is the transaction signature the AccountProfile requires at
    /// the owner coordinate, not a field of the record.
    ///
    /// The returned [`OrderEscrowV1`] is the movement admission *requires*: it
    /// is not advisory. Decision 0009 §2 recorded the collect-time debit as a
    /// live credit regression -- a maker could place a funded order, spend the
    /// collateral, and strand the whole candidate at `Collect`. Admission now
    /// moves the atoms, so the only balance settlement can ever be short of is
    /// one the protocol is already holding.
    pub fn admit(
        &mut self,
        order: GeneralOrderV1<'_>,
        funding: MakerFundingV1<'_>,
        current_slot: u64,
    ) -> GeneralCollectionResultV1<OrderEscrowV1> {
        if self.state.status != BatchStatusV1::Collecting {
            return Err(GeneralCollectionErrorV1::NotCollecting);
        }
        if current_slot >= self.opening.collection_close_slot {
            return Err(GeneralCollectionErrorV1::OutsideWindow);
        }
        // The record must say it was admitted at THIS slot: a placement writes
        // its own admission slot, and accepting a record that names another one
        // would let a replayed encoding re-enter a later batch window.
        if order.state().phase != GeneralOrderPhaseV1::Placed {
            return Err(GeneralCollectionErrorV1::InvalidOrderPhase);
        }
        if order.state().admitted_slot != current_slot {
            return Err(GeneralCollectionErrorV1::Substitution);
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
        Ok(OrderEscrowV1 {
            order_id: order.order_id(),
            owner_id: header.owner_id,
            outcome_count: self.opening.outcome_count,
            quote_atoms: quote_reserve,
            direction: EscrowDirectionV1::Deposit,
        })
    }

    /// Cancel one live order and return the exact refund the maker is owed.
    ///
    /// Only the maker may cancel, and only while the batch is still collecting:
    /// after the close the order set is final and a candidate may already have
    /// been built against it. `owner_id` is the identity the AccountProfile
    /// required a signature at; this transition binds it to the record.
    ///
    /// The refund is the whole escrow, exactly. It is exact without any ledger
    /// because the escrow is the order's OWN Custody vault and Claims Position:
    /// a maker can never be paid out of another maker's collateral, and that is
    /// structural rather than an invariant something has to maintain.
    pub fn cancel(
        &mut self,
        order: GeneralOrderV1<'_>,
        owner_id: [u8; 32],
        current_slot: u64,
    ) -> GeneralCollectionResultV1<OrderEscrowV1> {
        if self.state.status != BatchStatusV1::Collecting {
            return Err(GeneralCollectionErrorV1::NotCollecting);
        }
        if current_slot >= self.opening.collection_close_slot {
            return Err(GeneralCollectionErrorV1::OutsideWindow);
        }
        let header = order.header();
        if header.batch_id != self.batch_id() {
            return Err(GeneralCollectionErrorV1::Substitution);
        }
        if header.owner_id != owner_id {
            return Err(GeneralCollectionErrorV1::NotTheMaker);
        }
        if order.state().phase != GeneralOrderPhaseV1::Placed {
            return Err(GeneralCollectionErrorV1::InvalidOrderPhase);
        }
        let quote_reserve = order.quote_reserve()?;
        let next_reserve = self
            .state
            .committed_quote_reserve
            .checked_sub(quote_reserve)
            .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)?;
        let next_cancelled = self
            .state
            .cancelled_count
            .checked_add(1)
            .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)?;
        if next_cancelled > self.state.order_count {
            return Err(GeneralCollectionErrorV1::ArithmeticOverflow);
        }
        self.state.committed_quote_reserve = next_reserve;
        self.state.cancelled_count = next_cancelled;
        Ok(OrderEscrowV1 {
            order_id: order.order_id(),
            owner_id: header.owner_id,
            outcome_count: self.opening.outcome_count,
            quote_atoms: quote_reserve,
            direction: EscrowDirectionV1::Refund,
        })
    }

    /// Release one live order's residual escrow after the batch's window ends.
    ///
    /// Permissionless, like every other verb whose only effect is to return a
    /// maker's own collateral: withholding a refund is the griefing vector, and
    /// requiring the maker's signature is what would enable it.
    ///
    /// The residual is deliberately NOT computed here. Whatever a winning
    /// candidate collected already left the order's vault and Position at
    /// `Collect`; what remains IS the refund. Returning a computed number would
    /// be a second authority over a balance the chain already holds exactly.
    pub fn release(
        &self,
        order: GeneralOrderV1<'_>,
        current_slot: u64,
    ) -> GeneralCollectionResultV1<OrderEscrowV1> {
        if current_slot < self.opening.settlement_close_slot {
            return Err(GeneralCollectionErrorV1::OutsideWindow);
        }
        let header = order.header();
        if header.batch_id != self.batch_id() {
            return Err(GeneralCollectionErrorV1::Substitution);
        }
        if order.state().phase != GeneralOrderPhaseV1::Placed {
            return Err(GeneralCollectionErrorV1::InvalidOrderPhase);
        }
        Ok(OrderEscrowV1 {
            order_id: order.order_id(),
            owner_id: header.owner_id,
            outcome_count: self.opening.outcome_count,
            quote_atoms: 0,
            direction: EscrowDirectionV1::Residual,
        })
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

/// Which way one authenticated escrow movement runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EscrowDirectionV1 {
    /// Admission moves the maker's worst case into the order's own escrow.
    Deposit,
    /// Cancellation returns the whole escrow before the batch closes.
    Refund,
    /// Post-window release returns whatever settlement did not consume.
    Residual,
}

/// One exact escrow movement an order-lifecycle transition requires.
///
/// The escrow is addressed by the order's own content identity on both sides:
/// the Custody vault is `(market, release_set, order_id, Settlement)` and the
/// Claims Position is `(market, order_id)`. Nothing else in the family names
/// that pair, so an order's collateral is reachable by exactly the transitions
/// that quote this value.
///
/// `quote_atoms` is exact for [`EscrowDirectionV1::Deposit`] and
/// [`EscrowDirectionV1::Refund`]. For [`EscrowDirectionV1::Residual`] it is
/// zero and MEANS zero-is-not-the-amount: the physical layer moves the observed
/// balance, because after a settlement the escrow's own balance is the only
/// exact statement of what is left.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderEscrowV1 {
    /// Content identity of the order whose escrow this is.
    pub order_id: [u8; 32],
    /// Maker identity on the external side of the movement.
    pub owner_id: [u8; 32],
    /// Runtime outcome width of the claim leg.
    pub outcome_count: u32,
    /// Exact quote atoms, or zero when the balance itself is the amount.
    pub quote_atoms: u64,
    /// Which way the movement runs.
    pub direction: EscrowDirectionV1,
}

/// Lifecycle phase of one placed order's escrow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GeneralOrderPhaseV1 {
    /// Escrowed and executable by a candidate naming this order.
    Placed = ORDER_PHASE_PLACED,
    /// The maker cancelled before the batch closed; escrow returned in full.
    Cancelled = ORDER_PHASE_CANCELLED,
    /// The batch's window ended and the residual escrow was returned.
    Released = ORDER_PHASE_RELEASED,
}

impl GeneralOrderPhaseV1 {
    fn decode(value: u8) -> GeneralCollectionResultV1<Self> {
        match value {
            ORDER_PHASE_PLACED => Ok(Self::Placed),
            ORDER_PHASE_CANCELLED => Ok(Self::Cancelled),
            ORDER_PHASE_RELEASED => Ok(Self::Released),
            _ => Err(GeneralCollectionErrorV1::InvalidOrderPhase),
        }
    }

    /// Return the canonical one-byte phase tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }
}

/// Mutable escrow state in the fixed window between header and rows.
///
/// This window -- and only this window -- is what `order_id` masks, exactly as
/// a candidate identity masks its own 32 identity bytes. That is what lets an
/// order carry a lifecycle at all: a cancellation must not move the identity a
/// candidate, a manifest and a settlement row all name, and it must reach the
/// bytes it flips at a FIXED offset, because an EffectProgram write has no
/// runtime-width arithmetic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOrderStateV1 {
    /// Current escrow phase.
    pub phase: GeneralOrderPhaseV1,
    /// Slot the order was admitted at.
    pub admitted_slot: u64,
    /// Slot the escrow was returned at; zero while placed.
    pub released_slot: u64,
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

/// Borrowed order record: a fixed immutable header, a fixed mutable escrow
/// window, then the immutable per-outcome rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOrderV1<'a> {
    bytes: &'a [u8],
    header: GeneralOrderHeaderV1,
    state: GeneralOrderStateV1,
}

impl<'a> GeneralOrderV1<'a> {
    /// Hostile-decode one exact `192 + 16N` order record.
    pub fn decode(bytes: &'a [u8]) -> GeneralCollectionResultV1<Self> {
        if bytes.len() < GENERAL_ORDER_ROW_BASE_V1 {
            return Err(GeneralCollectionErrorV1::InvalidLength);
        }
        require_header(bytes, &ORDER_MAGIC, ORDER_PHASE)?;
        require_zero(bytes, 24, 8)?;
        let header = GeneralOrderHeaderV1 {
            outcome_count: read_u32(bytes, GeneralOrderLayoutV1::OUTCOME_COUNT)?,
            nonce: read_u64(bytes, GeneralOrderLayoutV1::NONCE)?,
            owner_id: read_array(bytes, GeneralOrderLayoutV1::OWNER_ID)?,
            market: read_array(bytes, GeneralOrderLayoutV1::MARKET)?,
            batch_id: read_array(bytes, GeneralOrderLayoutV1::BATCH_ID)?,
            generation: read_u64(bytes, GeneralOrderLayoutV1::GENERATION)?,
            max_lots: read_u64(bytes, GeneralOrderLayoutV1::MAX_LOTS)?,
            max_quote_debit_per_lot: read_u64(
                bytes,
                GeneralOrderLayoutV1::MAX_QUOTE_DEBIT_PER_LOT,
            )?,
            valid_until_slot: read_u64(bytes, GeneralOrderLayoutV1::VALID_UNTIL_SLOT)?,
        };
        if bytes.len() != general_order_len_v1(header.outcome_count)? {
            return Err(GeneralCollectionErrorV1::InvalidLength);
        }
        validate_order_header(header)?;
        require_zero(bytes, GENERAL_ORDER_STATE_OFFSET_V1 + 1, 7)?;
        require_zero(bytes, GENERAL_ORDER_STATE_OFFSET_V1 + 24, 8)?;
        let state = GeneralOrderStateV1 {
            phase: GeneralOrderPhaseV1::decode(read_u8(bytes, GeneralOrderLayoutV1::STATE_PHASE)?)?,
            admitted_slot: read_u64(bytes, GeneralOrderLayoutV1::STATE_ADMITTED_SLOT)?,
            released_slot: read_u64(bytes, GeneralOrderLayoutV1::STATE_RELEASED_SLOT)?,
        };
        validate_order_state(state)?;
        let value = Self {
            bytes,
            header,
            state,
        };
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
        state: GeneralOrderStateV1,
        output: &mut [u8],
    ) -> GeneralCollectionResultV1<()> {
        validate_order_header(header)?;
        validate_order_state(state)?;
        let count = usize_from_u32(header.outcome_count)?;
        if receive_per_lot.len() != count || deliver_per_lot.len() != count {
            return Err(GeneralCollectionErrorV1::InvalidLength);
        }
        if output.len() != general_order_len_v1(header.outcome_count)? {
            return Err(GeneralCollectionErrorV1::InvalidLength);
        }
        output.fill(0);
        put(output, GeneralOrderLayoutV1::MAGIC, &ORDER_MAGIC);
        put(
            output,
            GeneralOrderLayoutV1::VERSION,
            &VERSION.to_le_bytes(),
        );
        output[GeneralOrderLayoutV1::PHASE] = ORDER_PHASE;
        put(
            output,
            GeneralOrderLayoutV1::OUTCOME_COUNT,
            &header.outcome_count.to_le_bytes(),
        );
        put(
            output,
            GeneralOrderLayoutV1::NONCE,
            &header.nonce.to_le_bytes(),
        );
        put(output, GeneralOrderLayoutV1::OWNER_ID, &header.owner_id);
        put(output, GeneralOrderLayoutV1::MARKET, &header.market);
        put(output, GeneralOrderLayoutV1::BATCH_ID, &header.batch_id);
        put(
            output,
            GeneralOrderLayoutV1::GENERATION,
            &header.generation.to_le_bytes(),
        );
        put(
            output,
            GeneralOrderLayoutV1::MAX_LOTS,
            &header.max_lots.to_le_bytes(),
        );
        put(
            output,
            GeneralOrderLayoutV1::MAX_QUOTE_DEBIT_PER_LOT,
            &header.max_quote_debit_per_lot.to_le_bytes(),
        );
        put(
            output,
            GeneralOrderLayoutV1::VALID_UNTIL_SLOT,
            &header.valid_until_slot.to_le_bytes(),
        );
        for outcome in 0..count {
            let row = order_row_offset(outcome)?;
            put(
                output,
                row + GENERAL_ORDER_ROW_RECEIVE_OFFSET_V1,
                &receive_per_lot[outcome].to_le_bytes(),
            );
            put(
                output,
                row + GENERAL_ORDER_ROW_DELIVER_OFFSET_V1,
                &deliver_per_lot[outcome].to_le_bytes(),
            );
        }
        put(
            output,
            GeneralOrderLayoutV1::STATE_PHASE,
            &[state.phase.tag()],
        );
        put(
            output,
            GeneralOrderLayoutV1::STATE_ADMITTED_SLOT,
            &state.admitted_slot.to_le_bytes(),
        );
        put(
            output,
            GeneralOrderLayoutV1::STATE_RELEASED_SLOT,
            &state.released_slot.to_le_bytes(),
        );
        // Encode then hostile-decode our own candidate: the same total-function
        // discipline the three artifact encoders carry.
        decode_checked(output)
    }

    /// Return fixed order coordinates.
    #[must_use]
    pub const fn header(self) -> GeneralOrderHeaderV1 {
        self.header
    }

    /// Return the mutable escrow state.
    #[must_use]
    pub const fn state(self) -> GeneralOrderStateV1 {
        self.state
    }

    /// Return the canonical `order_id`: the masked digest of the record.
    ///
    /// The mutable escrow window is excluded on purpose. A cancellation writes
    /// that window, and if it were in the digest the identity a candidate, a
    /// manifest and every settlement row carry would move underneath them.
    #[must_use]
    pub fn order_id(self) -> [u8; 32] {
        general_order_identity_v1(self.bytes).unwrap_or([0; 32])
    }

    /// Write this order's successor escrow state into an exact-width buffer.
    ///
    /// The immutable bytes are copied verbatim, so a lifecycle write can never
    /// be the vehicle for substituting an order's terms.
    pub fn encode_successor_state_into(
        self,
        state: GeneralOrderStateV1,
        output: &mut [u8],
    ) -> GeneralCollectionResultV1<()> {
        validate_order_state(state)?;
        if output.len() != self.bytes.len() {
            return Err(GeneralCollectionErrorV1::InvalidLength);
        }
        if state.phase == self.state.phase {
            return Err(GeneralCollectionErrorV1::InvalidOrderPhase);
        }
        if self.state.phase != GeneralOrderPhaseV1::Placed {
            return Err(GeneralCollectionErrorV1::InvalidOrderPhase);
        }
        if state.admitted_slot != self.state.admitted_slot
            || state.released_slot < self.state.admitted_slot
        {
            return Err(GeneralCollectionErrorV1::Substitution);
        }
        output.copy_from_slice(self.bytes);
        put(
            output,
            GeneralOrderLayoutV1::STATE_PHASE,
            &[state.phase.tag()],
        );
        put(
            output,
            GeneralOrderLayoutV1::STATE_RELEASED_SLOT,
            &state.released_slot.to_le_bytes(),
        );
        let successor = GeneralOrderV1::decode(output)?;
        if successor.order_id() != self.order_id() || successor.state() != state {
            return Err(GeneralCollectionErrorV1::Substitution);
        }
        Ok(())
    }

    /// Return one exact claim quantity received per filled lot.
    pub fn receive_per_lot(self, index: u32) -> GeneralCollectionResultV1<u64> {
        self.row_field(GENERAL_ORDER_ROW_RECEIVE_OFFSET_V1, index)
    }

    /// Return one exact claim quantity delivered per filled lot.
    pub fn deliver_per_lot(self, index: u32) -> GeneralCollectionResultV1<u64> {
        self.row_field(GENERAL_ORDER_ROW_DELIVER_OFFSET_V1, index)
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

    fn row_field(self, field_offset: usize, index: u32) -> GeneralCollectionResultV1<u64> {
        if index >= self.header.outcome_count {
            return Err(GeneralCollectionErrorV1::InvalidLength);
        }
        let offset = order_row_offset(usize_from_u32(index)?)?
            .checked_add(field_offset)
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

/// Return exact `192 + 16N` bytes for one whole order record.
pub fn general_order_len_v1(outcome_count: u32) -> GeneralCollectionResultV1<usize> {
    if outcome_count == 0 {
        return Err(GeneralCollectionErrorV1::InvalidLength);
    }
    usize_from_u32(outcome_count)?
        .checked_mul(GENERAL_ORDER_ROW_STRIDE_V1)
        .and_then(|rows| GENERAL_ORDER_ROW_BASE_V1.checked_add(rows))
        .ok_or(GeneralCollectionErrorV1::ArithmeticOverflow)
}

/// Return the canonical identity of one order record.
///
/// The digest covers every byte EXCEPT the 32-byte mutable escrow window,
/// which is masked by construction here -- the fixed header and the
/// per-outcome rows are hashed as one split message. This is what lets the
/// mutable state live at a fixed offset (where an EffectProgram write can
/// reach it) while the identity stays pinned to exactly the bytes the maker
/// signed.
pub fn general_order_identity_v1(order_bytes: &[u8]) -> GeneralCollectionResultV1<[u8; 32]> {
    let head = order_bytes
        .get(..GENERAL_ORDER_STATE_OFFSET_V1)
        .ok_or(GeneralCollectionErrorV1::InvalidLength)?;
    let rows = order_bytes
        .get(GENERAL_ORDER_ROW_BASE_V1..)
        .ok_or(GeneralCollectionErrorV1::InvalidLength)?;
    Ok(digestv(&[head, rows]))
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

/// Require one verified candidate to fit inside the escrow the batch holds.
///
/// This is the check escrow-at-admission makes meaningful and that nothing
/// could state before it. `committed_quote_reserve` used to be a sum of
/// promises; it is now the sum of balances the protocol is holding in the
/// orders' own vaults, so a candidate whose total debit exceeds it is one that
/// could not be paid, and it is refused before any settlement account is
/// created rather than stranding at the first short `Collect`.
pub fn authenticate_batch_verified_candidate_v1(
    batch: GeneralBatchV1,
    verified: VerifiedCandidateHeaderV2,
) -> GeneralCollectionResultV1<()> {
    if batch.state().status != BatchStatusV1::Closed {
        return Err(GeneralCollectionErrorV1::NotClosed);
    }
    let opening = batch.opening();
    if verified.batch_id != batch.batch_id()
        || verified.product_id != opening.product_id
        || verified.outcome_count != opening.outcome_count
        || verified.price_scale != opening.price_scale
    {
        return Err(GeneralCollectionErrorV1::Substitution);
    }
    if verified.quote_debit > batch.state().committed_quote_reserve {
        return Err(GeneralCollectionErrorV1::EscrowShortfall);
    }
    Ok(())
}

/// Authenticate one Execution row against the immutable order it names.
///
/// Returns the exact [`AuthenticatedOrderTermsV2`] the streamed verifier
/// requires.  Every compact field the row repeats is checked against the record,
/// and the record's own digest is checked against the `order_id` the row claims,
/// so a row cannot import terms from an order it does not name.
///
/// **The per-lot vectors are checked here too, and that is not cosmetic.** The
/// verifier accumulates `claim_input = deliver_per_lot * lots` and
/// `claim_output = receive_per_lot * lots` from the vectors the CANDIDATE PAGE
/// carries, and [`AuthenticatedOrderTermsV2`] has no coordinate for either. So
/// while only the compact header fields were bound, a candidate author could
/// fill a maker's order with a portfolio the maker never signed -- the digest
/// matched, `max_lots` matched, and the claims moved were whatever the row said.
/// Nothing else in the family closed this: the row's vectors are re-read from
/// the same page on every step, so they were self-consistent and wrong
/// together. Binding them to the record is also what makes the admission escrow
/// a bound at all, since the escrowed claim reserve is computed from the
/// record's `deliver_per_lot` and would otherwise bound nothing the row does.
pub fn authenticate_order_execution_v1(
    batch: GeneralBatchV1,
    order: GeneralOrderV1<'_>,
    execution: ExecutionV2<'_>,
) -> GeneralCollectionResultV1<AuthenticatedOrderTermsV2> {
    if batch.state().status != BatchStatusV1::Closed {
        return Err(GeneralCollectionErrorV1::NotClosed);
    }
    let header = order.header();
    let execution_header = execution.header();
    if header.batch_id != batch.batch_id()
        || header.outcome_count != execution_header.outcome_count
        || header.owner_id != execution_header.owner_id
        || header.nonce != execution_header.nonce
        || header.max_lots != execution_header.max_lots
    {
        return Err(GeneralCollectionErrorV1::Substitution);
    }
    // A cancelled or released order has no escrow behind it. Refusing here is
    // what stops a candidate built while the batch was collecting from settling
    // against collateral its maker has already been refunded.
    if order.state().phase != GeneralOrderPhaseV1::Placed {
        return Err(GeneralCollectionErrorV1::InvalidOrderPhase);
    }
    let terms = order.terms();
    if terms.order_id != execution_header.order_id {
        return Err(GeneralCollectionErrorV1::Substitution);
    }
    if execution_header.lots == 0 || execution_header.lots > header.max_lots {
        return Err(GeneralCollectionErrorV1::Substitution);
    }
    for outcome in 0..header.outcome_count {
        if execution
            .receive_per_lot(outcome)
            .map_err(|_| GeneralCollectionErrorV1::InvalidLength)?
            != order.receive_per_lot(outcome)?
            || execution
                .deliver_per_lot(outcome)
                .map_err(|_| GeneralCollectionErrorV1::InvalidLength)?
                != order.deliver_per_lot(outcome)?
        {
            return Err(GeneralCollectionErrorV1::Substitution);
        }
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

fn validate_order_state(state: GeneralOrderStateV1) -> GeneralCollectionResultV1<()> {
    match state.phase {
        GeneralOrderPhaseV1::Placed => {
            if state.released_slot != 0 {
                return Err(GeneralCollectionErrorV1::InvalidOrderPhase);
            }
        }
        GeneralOrderPhaseV1::Cancelled | GeneralOrderPhaseV1::Released => {
            if state.released_slot < state.admitted_slot {
                return Err(GeneralCollectionErrorV1::InvalidOrderPhase);
            }
        }
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

fn order_row_offset(index: usize) -> GeneralCollectionResultV1<usize> {
    index
        .checked_mul(GENERAL_ORDER_ROW_STRIDE_V1)
        .and_then(|rows| GENERAL_ORDER_ROW_BASE_V1.checked_add(rows))
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
