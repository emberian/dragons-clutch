#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Pure, fixed-capacity contract for dClutch's optional General frequent-batch
//! venue.
//!
//! The crate owns venue semantics, not Solana account access, signatures,
//! hashing, token transfers, or transaction construction. An adapter must
//! authenticate the content identities and transcript steps supplied here,
//! then persist returned state atomically.

#[cfg(test)]
extern crate std;

use core::convert::{TryFrom, TryInto};

use dclutch_capability_contract::{
    CapabilityEntryV1, CapabilityFundingDerivationV1, CapabilityManifestV1, FundingAssetClassV1,
    FundingCompartment as CapabilityFundingCompartment, FundingCustodyObservationV1,
    FundingStateV1,
};
use dclutch_core_contract::{MarketIdentity, MarketRoot, Phase as MarketPhase};

/// Narrow successor-owned General configuration and root contracts.
///
/// This migration re-export does not make the broad V1 venue an authority for
/// V2 bytes. New physical programs depend on `dclutch-general-config-contract`
/// directly.
pub use dclutch_general_config_contract as successor_config;
/// Frequently used V2 successor types re-exported during migration.
pub use dclutch_general_config_contract::{
    GeneralConfigV2, GeneralConfigV2Input, GeneralLifecycleV2, GeneralRootV2,
};

/// Exact width of an opaque content identity.
pub const CONTENT_ID_BYTES: usize = 32;
/// Provisional program-profile bound on claim-basis cells.
///
/// It is neither mathematical nor a product-family limit. A new capability
/// release and capacity-profile identity may lift it while preserving V1
/// Markets and receipts.
pub const MAX_OUTCOMES_V1: usize = 16;
/// Provisional program-profile bound on executions checked in one page.
///
/// A new capability release may change the page envelope without changing the
/// candidate objective or complete-set conservation law.
pub const MAX_EXECUTIONS_PER_PAGE_V1: usize = 4;
/// Exact canonical byte width of [`GeneralConfigV1`].
pub const GENERAL_CONFIG_BYTES: usize = 200;
/// Exact canonical byte width of [`GeneralRootV1`].
pub const GENERAL_ROOT_BYTES: usize = 136;
/// Exact canonical byte width of [`GeneralFundingV1`].
pub const GENERAL_FUNDING_BYTES: usize = 144;
/// Exact canonical byte width of [`BatchRootV1`].
pub const BATCH_ROOT_BYTES: usize = 144;
/// Exact canonical byte width of [`OrderStateV1`].
pub const ORDER_STATE_BYTES: usize = 96;
/// Fixed byte prefix of [`GeneralOrderCustodyV1`] before its exact `N` claim reserves.
pub const GENERAL_ORDER_CUSTODY_BASE_BYTES: usize = 192;
/// Fixed byte prefix of a [`PortfolioOrderV1`] before its exact `N` coefficients.
pub const PORTFOLIO_ORDER_BASE_BYTES: usize = 200;
/// Fixed byte prefix of the noncircular signed-order commitment preimage.
pub const PORTFOLIO_ORDER_SIGNING_BASE_BYTES: usize = 168;
/// Fixed byte prefix of a [`SettlementReceiptV1`] before its exact `N` deltas.
pub const SETTLEMENT_RECEIPT_BASE_BYTES: usize = 176;
/// Fixed byte prefix of [`CandidateSubmissionV1`] before its exact-`N` price and claim vectors.
pub const CANDIDATE_SUBMISSION_BASE_BYTES: usize = 224;
/// Fixed byte prefix of [`CandidateStateV1`] before its exact-N price and cursor vectors.
pub const CANDIDATE_STATE_BASE_BYTES: usize = 440;
/// Fixed byte prefix of [`SettlementCursorV1`] before its four exact-`N` vectors.
pub const SETTLEMENT_CURSOR_BASE_BYTES: usize = 304;
/// Fixed byte prefix of [`CandidatePageV1`] before its exact leading executions.
pub const CANDIDATE_PAGE_BASE_BYTES: usize = 56;

const CONFIG_MAGIC: [u8; 8] = *b"DCLTGEN1";
const GENERAL_ROOT_MAGIC: [u8; 8] = *b"DCLTGRR1";
const GENERAL_FUNDING_MAGIC: [u8; 8] = *b"DCLTGFN1";
const BATCH_ROOT_MAGIC: [u8; 8] = *b"DCLTGBR1";
const ORDER_STATE_MAGIC: [u8; 8] = *b"DCLTGOS1";
const ORDER_CUSTODY_MAGIC: [u8; 8] = *b"DCLTGOC1";
const ORDER_MAGIC: [u8; 8] = *b"DCLTGOR1";
const ORDER_SIGNING_MAGIC: [u8; 8] = *b"DCLTGOM1";
const RECEIPT_MAGIC: [u8; 8] = *b"DCLTGSR1";
const CANDIDATE_SUBMISSION_MAGIC: [u8; 8] = *b"DCLTGCS1";
const CANDIDATE_STATE_MAGIC: [u8; 8] = *b"DCLTGCA1";
const SETTLEMENT_CURSOR_MAGIC: [u8; 8] = *b"DCLTGSC1";
const CANDIDATE_PAGE_MAGIC: [u8; 8] = *b"DCLTGVP1";
const SETTLEMENT_CURSOR_VECTOR_OFFSET: usize = SETTLEMENT_CURSOR_BASE_BYTES;
const SCHEMA_V1: u16 = 1;
const ARTIFACT_PROFILE_V1: u16 = 1;

/// Domain-separated preimage of the General capability-kind identity.
pub const GENERAL_CAPABILITY_KIND_PREIMAGE_V1: &[u8] = b"dclutch/general/capability-kind/v1";
/// Domain-separated preimage of the reviewed frequent-batch release identity.
pub const GENERAL_CAPABILITY_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/general/frequent-batch-release/v1";
/// Domain-separated preimage of the General child-layout schema identity.
pub const GENERAL_CHILD_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/general/child-schema/v1";
/// Domain-separated preimage of the General child-derivation policy identity.
pub const GENERAL_CHILD_DERIVATION_PREIMAGE_V1: &[u8] = b"dclutch/general/child-derivation/v1";
/// Domain-separated generic-record schema label for immutable General config.
pub const GENERAL_CONFIG_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/general-config-v1";

/// PDA seed domain for one capability-bound General funding child.
pub const GENERAL_FUNDING_PDA_DOMAIN_V1: &[u8] = b"dclutch/general-funding/v1";
/// PDA seed domain for one General capability root.
pub const GENERAL_ROOT_PDA_DOMAIN_V1: &[u8] = b"dclutch/general-root/v1";
/// PDA seed domain for one General batch root.
pub const GENERAL_BATCH_PDA_DOMAIN_V1: &[u8] = b"dclutch/general-batch/v1";
/// PDA seed domain for one signed-order replay state.
pub const GENERAL_ORDER_STATE_PDA_DOMAIN_V1: &[u8] = b"dclutch/general-order-state/v1";
/// PDA seed domain for one exact-N order custody state.
pub const GENERAL_ORDER_CUSTODY_PDA_DOMAIN_V1: &[u8] = b"dclutch/general-order-custody/v1";
/// PDA seed domain for one token-program-owned order quote escrow.
pub const GENERAL_QUOTE_ESCROW_PDA_DOMAIN_V1: &[u8] = b"dclutch/general-quote-escrow/v1";
/// PDA seed domain for one submitted candidate and verification cursor.
pub const GENERAL_CANDIDATE_PDA_DOMAIN_V1: &[u8] = b"dclutch/general-candidate/v1";
/// PDA seed domain for one candidate-exclusive immutable page record.
pub const GENERAL_CANDIDATE_PAGE_PDA_DOMAIN_V1: &[u8] = b"dclutch/general-candidate-page/v1";
/// PDA seed domain for one selected-candidate settlement cursor.
pub const GENERAL_SETTLEMENT_CURSOR_PDA_DOMAIN_V1: &[u8] = b"dclutch/general-settle/v1";
/// PDA seed domain for the selected settlement's collateral-token escrow.
pub const GENERAL_SETTLEMENT_ESCROW_PDA_DOMAIN_V1: &[u8] = b"dclutch/general-settle-escrow/v1";
/// Domain-separated initial transcript commitment for one candidate identity.
pub const GENERAL_CANDIDATE_PAGE_CONTENT_DOMAIN_V1: &[u8] =
    b"dclutch/general-candidate-page-content/v1";

/// Refusal returned by the General venue contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// An exact-width byte record had another length.
    InvalidLength,
    /// A record did not carry its canonical magic.
    InvalidMagic,
    /// A schema or artifact profile is not implemented.
    UnsupportedSchema,
    /// Reserved or unused fields were not canonical zeroes.
    NonCanonicalReservedBytes,
    /// Persisted transition fields do not describe a reachable canonical state.
    NonCanonicalState,
    /// A required content identity was the all-zero sentinel.
    ZeroIdentifier,
    /// Two immutable authorities or occurrence generations differed.
    AuthorityMismatch,
    /// A provisional capacity bound was exceeded.
    CapacityExceeded,
    /// A configured capacity or duration was zero.
    ZeroCapacity,
    /// The outcome count was outside the selected V1 envelope.
    InvalidOutcomeCount,
    /// A price scale was zero.
    ZeroPriceScale,
    /// Simplex coordinates did not sum exactly to the configured scale.
    InvalidSimplexPrice,
    /// A price or coefficient was nonzero outside the exact ClaimBasis width.
    NonCanonicalPortfolio,
    /// An atomic portfolio contained no nonzero coefficient.
    EmptyPortfolio,
    /// Checked exact integer arithmetic overflowed.
    ArithmeticOverflow,
    /// A signed token amount did not fit the V1 adapter word.
    TokenAmountOutOfRange,
    /// A slot interval overflowed.
    SlotOverflow,
    /// A transition was not admitted from the current phase.
    InvalidPhase,
    /// A call occurred before or after its immutable slot window.
    OutsideWindow,
    /// A sequence, cursor, page, nonce, or transcript predecessor mismatched.
    CursorMismatch,
    /// A transcript successor was not a nonzero new commitment.
    InvalidTranscriptStep,
    /// Page entries or orders were not in strict canonical order.
    NonCanonicalOrder,
    /// A page count was zero or exceeded the V1 page envelope.
    InvalidPageCount,
    /// An order did not belong to this Market generation and batch.
    OrderBindingMismatch,
    /// An order was expired for the complete settlement window.
    OrderExpired,
    /// An order had been cancelled, consumed, or otherwise was not open.
    OrderUnavailable,
    /// A requested atomic lot fill was zero or exceeded remaining lots.
    InvalidFill,
    /// A portfolio fill violated the order's exact debit limit.
    LimitViolated,
    /// Candidate recomputation did not equal its committed claim.
    CandidateClaimMismatch,
    /// Candidate net inventory was not one virtual complete-set vector.
    IncompleteSetImbalance,
    /// Quote flow did not exactly capitalize or release the complete sets.
    QuoteConservationMismatch,
    /// A Candidate was not the batch's selected best valid submission.
    CandidateNotSelected,
    /// Prefix-carry settlement ended with fractional scale units outstanding.
    RoundingCarryOutstanding,
    /// Hoard principal and complete-set liabilities were unequal.
    HoardInvariantViolation,
    /// Hoard principal was insufficient for a complete-set burn.
    InsufficientHoardPrincipal,
    /// Funding compartment principal was insufficient.
    InsufficientFunding,
    /// A zero funding debit was requested.
    ZeroFundingDebit,
    /// Immutable funding did not equal remaining, spent, and refunded amounts.
    FundingConservationMismatch,
    /// A manifest entry did not select the closed reviewed General release facts.
    UnrecognizedCapability,
    /// Capability funding did not bind or transition canonically into General funding.
    CapabilityFundingMismatch,
    /// A General capability quote selected a forbidden compartment or asset class.
    ExtraneousCapabilityFunding,
    /// Physical General-funding lamports did not equal Rent plus remaining compartments.
    GeneralFundingCustodyMismatch,
    /// Persisted order custody did not bind the signed order or settlement receipt.
    CustodyMismatch,
    /// Locked quote or claim principal was insufficient for an authenticated receipt.
    InsufficientCustody,
    /// Order custody was closed before replay state became unavailable.
    CustodyNotReleasable,
    /// Retirement was attempted before all owned state was quiescent.
    NotQuiescent,
    /// One SDK-free account projection carried the wrong physical privileges.
    InvalidAccountPrivilege,
    /// Two exact frame roles aliased without an explicit V1 alias rule.
    AccountAlias,
    /// An instruction action tag is not recognized by General V1.
    UnknownAction,
    /// An instruction payload was not canonical for its selected action.
    InvalidInstruction,
}

/// Result alias for General contract operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Validated nonzero opaque content identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ContentId([u8; CONTENT_ID_BYTES]);

impl ContentId {
    /// Construct a nonzero identity.
    pub fn new(bytes: [u8; CONTENT_ID_BYTES]) -> Result<Self> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(Error::ZeroIdentifier);
        }
        Ok(Self(bytes))
    }

    /// Return the exact opaque bytes.
    pub const fn to_bytes(self) -> [u8; CONTENT_ID_BYTES] {
        self.0
    }

    /// Borrow the exact opaque bytes.
    pub const fn as_bytes(&self) -> &[u8; CONTENT_ID_BYTES] {
        &self.0
    }
}

/// Exact nonzero Ed25519/SVM public key whose signature owns an order.
///
/// This is not an opaque content digest. An SVM adapter compares these bytes
/// directly to the authenticated signing public key.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct OwnerKeyV1([u8; 32]);

impl OwnerKeyV1 {
    /// Construct one nonzero signed-owner public key.
    pub fn new(bytes: [u8; 32]) -> Result<Self> {
        require_nonzero_key(&bytes)?;
        Ok(Self(bytes))
    }

    /// Return the exact Ed25519/SVM public-key bytes.
    pub const fn to_bytes(self) -> [u8; 32] {
        self.0
    }

    /// Borrow the exact Ed25519/SVM public-key bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// SHA-256 identity of [`GENERAL_CAPABILITY_KIND_PREIMAGE_V1`].
pub const GENERAL_CAPABILITY_KIND_ID_V1: ContentId = ContentId([
    0xcb, 0x8b, 0xc8, 0x7d, 0xbb, 0xf9, 0xee, 0x58, 0xfc, 0xe8, 0x60, 0x01, 0xa1, 0xbb, 0x5d, 0x1f,
    0x2a, 0xf5, 0x7c, 0xec, 0x1d, 0xee, 0xff, 0x86, 0x66, 0x10, 0xca, 0x7f, 0xc4, 0x27, 0x4c, 0xb5,
]);
/// SHA-256 identity of [`GENERAL_CAPABILITY_RELEASE_PREIMAGE_V1`].
pub const GENERAL_CAPABILITY_RELEASE_ID_V1: ContentId = ContentId([
    0x6a, 0xd3, 0x9a, 0xd8, 0xd4, 0x99, 0xbf, 0xa9, 0x46, 0x87, 0xc9, 0x10, 0x54, 0x38, 0x8a, 0x61,
    0xe3, 0x58, 0xbd, 0x95, 0x2e, 0x00, 0xac, 0x35, 0x02, 0xa4, 0x42, 0xbd, 0xe2, 0x76, 0xdc, 0x2f,
]);
/// SHA-256 identity of [`GENERAL_CHILD_SCHEMA_PREIMAGE_V1`].
pub const GENERAL_CHILD_SCHEMA_ID_V1: ContentId = ContentId([
    0xa8, 0x3f, 0xbe, 0xbd, 0x0b, 0x10, 0x56, 0x7a, 0x25, 0xd5, 0xb1, 0xf2, 0xdb, 0x16, 0x18, 0x1b,
    0x88, 0xba, 0x28, 0xf5, 0x54, 0x9d, 0x61, 0xc3, 0xce, 0x02, 0xd6, 0x00, 0xc3, 0x09, 0x60, 0xb9,
]);
/// SHA-256 identity of [`GENERAL_CHILD_DERIVATION_PREIMAGE_V1`].
pub const GENERAL_CHILD_DERIVATION_ID_V1: ContentId = ContentId([
    0x56, 0xd5, 0xff, 0xe7, 0xce, 0x62, 0x82, 0x2c, 0x62, 0x32, 0x11, 0xea, 0x3e, 0x4a, 0x53, 0x3c,
    0xfb, 0x5c, 0xd9, 0xa5, 0x84, 0x60, 0xf7, 0x85, 0xea, 0x52, 0x01, 0x69, 0x97, 0x18, 0x8b, 0xb0,
]);
/// SHA-256 identity of [`GENERAL_CONFIG_SCHEMA_PREIMAGE_V1`].
pub const GENERAL_CONFIG_SCHEMA_ID_V1: ContentId = ContentId([
    0xdc, 0xf6, 0xa5, 0x2b, 0x26, 0x42, 0xeb, 0xd0, 0xeb, 0xcf, 0xfc, 0x7f, 0xf6, 0x82, 0x83, 0x5a,
    0xa7, 0xf8, 0x75, 0xb7, 0xf6, 0x7c, 0xdf, 0xa5, 0x65, 0xc6, 0xa4, 0x2c, 0x79, 0xca, 0x70, 0xf9,
]);

/// Exact PDA seed projection for one active General root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralRootPdaSeedsV1 {
    market: [u8; 32],
    generation_le: [u8; 8],
    config_id: [u8; 32],
}

impl GeneralRootPdaSeedsV1 {
    /// Construct the canonical Market/generation/config root derivation.
    pub fn new(market: [u8; 32], generation: u64, config_id: ContentId) -> Result<Self> {
        require_nonzero_key(&market)?;
        Ok(Self {
            market,
            generation_le: generation.to_le_bytes(),
            config_id: config_id.to_bytes(),
        })
    }

    /// Return `[domain, market, generation_le, config_id]` in canonical order.
    pub fn seed_components(&self) -> [&[u8]; 4] {
        [
            GENERAL_ROOT_PDA_DOMAIN_V1,
            self.market.as_slice(),
            self.generation_le.as_slice(),
            self.config_id.as_slice(),
        ]
    }

    /// Return the authenticated Market key seed.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Return the Market-generation seed.
    pub const fn generation(self) -> u64 {
        u64::from_le_bytes(self.generation_le)
    }

    /// Return the config content-identity seed.
    pub const fn config_id(self) -> [u8; 32] {
        self.config_id
    }
}

/// Exact PDA seed projection for one segregated General funding ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralFundingPdaSeedsV1 {
    market: [u8; 32],
    generation_le: [u8; 8],
    config_id: [u8; 32],
    release_id: [u8; 32],
}

impl GeneralFundingPdaSeedsV1 {
    /// Construct the canonical Market/generation/config/release derivation.
    pub fn new(
        market: [u8; 32],
        generation: u64,
        config_id: ContentId,
        release_id: ContentId,
    ) -> Result<Self> {
        require_nonzero_key(&market)?;
        if release_id != GENERAL_CAPABILITY_RELEASE_ID_V1 {
            return Err(Error::UnrecognizedCapability);
        }
        Ok(Self {
            market,
            generation_le: generation.to_le_bytes(),
            config_id: config_id.to_bytes(),
            release_id: release_id.to_bytes(),
        })
    }

    /// Return `[domain, market, generation_le, config_id, release_id]`.
    pub fn seed_components(&self) -> [&[u8]; 5] {
        [
            GENERAL_FUNDING_PDA_DOMAIN_V1,
            self.market.as_slice(),
            self.generation_le.as_slice(),
            self.config_id.as_slice(),
            self.release_id.as_slice(),
        ]
    }
}

/// Exact PDA seed projection for one General batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralBatchPdaSeedsV1 {
    root: [u8; 32],
    sequence_le: [u8; 8],
}

impl GeneralBatchPdaSeedsV1 {
    /// Construct from the authenticated root key and reserved sequence.
    pub fn new(root: [u8; 32], sequence: u64) -> Result<Self> {
        require_nonzero_key(&root)?;
        Ok(Self {
            root,
            sequence_le: sequence.to_le_bytes(),
        })
    }

    /// Return `[domain, root, sequence_le]` in canonical order.
    pub fn seed_components(&self) -> [&[u8]; 3] {
        [
            GENERAL_BATCH_PDA_DOMAIN_V1,
            self.root.as_slice(),
            self.sequence_le.as_slice(),
        ]
    }
}

/// Exact PDA seed projection for one signed-order replay record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOrderStatePdaSeedsV1 {
    market: [u8; 32],
    generation_le: [u8; 8],
    owner: [u8; 32],
    nonce_le: [u8; 8],
    order_id: [u8; 32],
}

impl GeneralOrderStatePdaSeedsV1 {
    /// Construct the unique replay derivation from one authenticated order.
    pub fn new<const N: usize>(market: [u8; 32], order: PortfolioOrderV1<N>) -> Result<Self> {
        require_nonzero_key(&market)?;
        Ok(Self {
            market,
            generation_le: order.generation().to_le_bytes(),
            owner: order.owner().to_bytes(),
            nonce_le: order.nonce().to_le_bytes(),
            order_id: order.order_id().to_bytes(),
        })
    }

    /// Return `[domain, market, generation_le, owner, nonce_le, order_id]`.
    pub fn seed_components(&self) -> [&[u8]; 6] {
        [
            GENERAL_ORDER_STATE_PDA_DOMAIN_V1,
            self.market.as_slice(),
            self.generation_le.as_slice(),
            self.owner.as_slice(),
            self.nonce_le.as_slice(),
            self.order_id.as_slice(),
        ]
    }
}

/// Exact PDA seed projection for one order custody account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOrderCustodyPdaSeedsV1 {
    order_state: [u8; 32],
}

impl GeneralOrderCustodyPdaSeedsV1 {
    /// Construct from the already-derived replay account key.
    pub fn new(order_state: [u8; 32]) -> Result<Self> {
        require_nonzero_key(&order_state)?;
        Ok(Self { order_state })
    }

    /// Return `[domain, order_state]` in canonical order.
    pub fn seed_components(&self) -> [&[u8]; 2] {
        [
            GENERAL_ORDER_CUSTODY_PDA_DOMAIN_V1,
            self.order_state.as_slice(),
        ]
    }
}

/// Exact PDA seed projection for one token-program-owned quote escrow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralQuoteEscrowPdaSeedsV1 {
    custody: [u8; 32],
}

impl GeneralQuoteEscrowPdaSeedsV1 {
    /// Construct from the already-derived semantic custody account key.
    pub fn new(custody: [u8; 32]) -> Result<Self> {
        require_nonzero_key(&custody)?;
        Ok(Self { custody })
    }

    /// Return `[domain, custody]` in canonical order.
    pub fn seed_components(&self) -> [&[u8]; 2] {
        [GENERAL_QUOTE_ESCROW_PDA_DOMAIN_V1, self.custody.as_slice()]
    }
}

/// Exact PDA seed projection for one submitted candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralCandidatePdaSeedsV1 {
    batch: [u8; 32],
    candidate_id: [u8; 32],
}

impl GeneralCandidatePdaSeedsV1 {
    /// Construct from the authenticated batch key and submission identity.
    pub fn new(batch: [u8; 32], candidate_id: ContentId) -> Result<Self> {
        require_nonzero_key(&batch)?;
        Ok(Self {
            batch,
            candidate_id: candidate_id.to_bytes(),
        })
    }

    /// Return `[domain, batch, candidate_id]` in canonical order.
    pub fn seed_components(&self) -> [&[u8]; 3] {
        [
            GENERAL_CANDIDATE_PDA_DOMAIN_V1,
            self.batch.as_slice(),
            self.candidate_id.as_slice(),
        ]
    }
}

/// Exact PDA seed projection for one immutable candidate page copy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralCandidatePagePdaSeedsV1 {
    candidate: [u8; 32],
    page_id: [u8; 32],
}

impl GeneralCandidatePagePdaSeedsV1 {
    /// Construct from the candidate account and authenticated page content ID.
    pub fn new(candidate: [u8; 32], page_id: ContentId) -> Result<Self> {
        require_nonzero_key(&candidate)?;
        Ok(Self {
            candidate,
            page_id: page_id.to_bytes(),
        })
    }

    /// Return `[domain, candidate, page_id]` in canonical order.
    pub fn seed_components(&self) -> [&[u8]; 3] {
        [
            GENERAL_CANDIDATE_PAGE_PDA_DOMAIN_V1,
            self.candidate.as_slice(),
            self.page_id.as_slice(),
        ]
    }
}

/// Exact PDA seed projection for one selected-candidate settlement cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralSettlementCursorPdaSeedsV1 {
    candidate: [u8; 32],
}

impl GeneralSettlementCursorPdaSeedsV1 {
    /// Construct from the selected candidate account key.
    pub fn new(candidate: [u8; 32]) -> Result<Self> {
        require_nonzero_key(&candidate)?;
        Ok(Self { candidate })
    }

    /// Return `[domain, candidate]` in canonical order.
    pub fn seed_components(&self) -> [&[u8]; 2] {
        [
            GENERAL_SETTLEMENT_CURSOR_PDA_DOMAIN_V1,
            self.candidate.as_slice(),
        ]
    }
}

/// Exact PDA seed projection for the cursor-owned settlement collateral escrow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralSettlementEscrowPdaSeedsV1 {
    settlement_cursor: [u8; 32],
}

impl GeneralSettlementEscrowPdaSeedsV1 {
    /// Construct from the already-derived settlement cursor key.
    pub fn new(settlement_cursor: [u8; 32]) -> Result<Self> {
        require_nonzero_key(&settlement_cursor)?;
        Ok(Self { settlement_cursor })
    }

    /// Return `[domain, settlement_cursor]` in canonical order.
    pub fn seed_components(&self) -> [&[u8]; 2] {
        [
            GENERAL_SETTLEMENT_ESCROW_PDA_DOMAIN_V1,
            self.settlement_cursor.as_slice(),
        ]
    }
}

/// Domain-separated content preimage projection for one canonical page record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralCandidatePageContentPreimageV1<'a> {
    canonical_page_bytes: &'a [u8],
}

impl<'a> GeneralCandidatePageContentPreimageV1<'a> {
    /// Bind exact canonical page bytes for adapter hashing.
    pub const fn new(canonical_page_bytes: &'a [u8]) -> Self {
        Self {
            canonical_page_bytes,
        }
    }

    /// Return `[domain, canonical_page_bytes]` in canonical hash order.
    pub fn components(&self) -> [&[u8]; 2] {
        [
            GENERAL_CANDIDATE_PAGE_CONTENT_DOMAIN_V1,
            self.canonical_page_bytes,
        ]
    }
}

/// Shared header width of every General V1 instruction.
pub const GENERAL_INSTRUCTION_HEADER_BYTES: usize = 16;
/// Canonical General instruction-family magic.
pub const GENERAL_INSTRUCTION_MAGIC_V1: [u8; 8] = *b"DCLTGIN1";
/// Implemented General instruction schema.
pub const GENERAL_INSTRUCTION_SCHEMA_V1: u16 = 1;

/// Closed General V1 instruction action set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GeneralInstructionTagV1 {
    /// Activate from finalized config and create root/funding.
    Activate = 1,
    /// Reserve and create the next collecting batch.
    OpenBatch = 2,
    /// Close collection and enter candidate selection.
    LockBatch = 3,
    /// Admit one signed order with replay and exact custody.
    AdmitOrder = 4,
    /// Cancel one signed order before collection closes.
    CancelOrder = 5,
    /// Close one unavailable order and release residual custody/rent.
    CloseOrder = 6,
    /// Create one permissionless candidate from its canonical submission.
    SubmitCandidate = 7,
    /// Verify one canonical bounded candidate page.
    VerifyCandidatePage = 8,
    /// Finish candidate verification from its persisted cursor.
    FinishCandidate = 9,
    /// Consider one valid candidate for deterministic best-submitted selection.
    ConsiderCandidate = 10,
    /// Freeze the best valid submitted candidate after selection closes.
    LockSelection = 11,
    /// Capitalize and begin the selected candidate's non-expiring settlement.
    BeginSettlement = 12,
    /// Collect one canonical page's negative settlement inputs.
    CollectSettlementPage = 13,
    /// Finish settlement after exact cursor convergence.
    FinishSettlement = 14,
    /// Retire one quiescent batch and its direct child count.
    CloseBatch = 15,
    /// Stop admitting new batches.
    Quiesce = 16,
    /// Discharge terminal funding and close the General account cluster.
    CloseGeneral = 17,
    /// Close one unavailable candidate into its submitter's RentCredit.
    CloseCandidate = 18,
    /// Close one finished settlement cursor into its submitter's RentCredit.
    CloseSettlement = 19,
    /// Perform the sole complete-set split/merge after all inputs converge.
    MaterializeSettlement = 20,
    /// Distribute one canonical page's positive settlement outputs.
    DistributeSettlementPage = 21,
    /// Create one immutable candidate-exclusive page record.
    CreateCandidatePage = 22,
    /// Close one immutable page after it can no longer be consumed.
    CloseCandidatePage = 23,
    /// Reject one abandoned candidate after its immutable deadline.
    RejectCandidate = 24,
    /// Expire a selected candidate before settlement begins.
    ExpireSettlement = 25,
}

impl GeneralInstructionTagV1 {
    fn decode(tag: u8) -> Result<Self> {
        match tag {
            1 => Ok(Self::Activate),
            2 => Ok(Self::OpenBatch),
            3 => Ok(Self::LockBatch),
            4 => Ok(Self::AdmitOrder),
            5 => Ok(Self::CancelOrder),
            6 => Ok(Self::CloseOrder),
            7 => Ok(Self::SubmitCandidate),
            8 => Ok(Self::VerifyCandidatePage),
            9 => Ok(Self::FinishCandidate),
            10 => Ok(Self::ConsiderCandidate),
            11 => Ok(Self::LockSelection),
            12 => Ok(Self::BeginSettlement),
            13 => Ok(Self::CollectSettlementPage),
            14 => Ok(Self::FinishSettlement),
            15 => Ok(Self::CloseBatch),
            16 => Ok(Self::Quiesce),
            17 => Ok(Self::CloseGeneral),
            18 => Ok(Self::CloseCandidate),
            19 => Ok(Self::CloseSettlement),
            20 => Ok(Self::MaterializeSettlement),
            21 => Ok(Self::DistributeSettlementPage),
            22 => Ok(Self::CreateCandidatePage),
            23 => Ok(Self::CloseCandidatePage),
            24 => Ok(Self::RejectCandidate),
            25 => Ok(Self::ExpireSettlement),
            _ => Err(Error::UnknownAction),
        }
    }
}

/// Activation replay beyond authenticated Market, manifest, funding, and config records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivateGeneralV1 {
    /// Direct Market child count required before activation.
    pub expected_market_child_count: u64,
}

/// Batch/generation replay facts shared by batch lifecycle actions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralBatchReplayV1 {
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact General batch sequence.
    pub batch_sequence: u64,
}

/// Candidate content identity plus its sole canonical submission payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubmitGeneralCandidateV1<const N: usize> {
    /// SHA-256 identity of the canonical submission bytes.
    pub candidate_id: ContentId,
    /// Exact candidate submission.
    pub submission: CandidateSubmissionV1<N>,
}

/// Candidate identity plus one immutable page content identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralCandidatePageV1 {
    /// Persisted candidate content identity.
    pub candidate_id: ContentId,
    /// Authenticated immutable page content identity.
    pub page_id: ContentId,
}

/// Candidate-exclusive page creation payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateGeneralCandidatePageV1<const N: usize> {
    /// Persisted candidate content identity.
    pub candidate_id: ContentId,
    /// Domain-separated identity of the exact canonical page bytes.
    pub page_id: ContentId,
    /// Exact immutable page record.
    pub page: CandidatePageV1<N>,
}

/// One hostile-decoded General instruction at exact ClaimBasis width `N`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralInstructionV1<const N: usize> {
    /// Activate root/funding from finalized config and generic capability funding.
    Activate(ActivateGeneralV1),
    /// Open the next batch.
    OpenBatch(GeneralBatchReplayV1),
    /// Lock collection.
    LockBatch(GeneralBatchReplayV1),
    /// Admit one canonical signed order.
    AdmitOrder(PortfolioOrderV1<N>),
    /// Cancel one canonical signed order.
    CancelOrder(PortfolioOrderV1<N>),
    /// Close one canonical signed order's replay and custody.
    CloseOrder(PortfolioOrderV1<N>),
    /// Submit one candidate.
    SubmitCandidate(SubmitGeneralCandidateV1<N>),
    /// Create one immutable candidate page.
    CreateCandidatePage(CreateGeneralCandidatePageV1<N>),
    /// Verify one candidate page.
    VerifyCandidatePage(GeneralCandidatePageV1),
    /// Finish candidate verification.
    FinishCandidate(ContentId),
    /// Consider one valid candidate.
    ConsiderCandidate(ContentId),
    /// Lock candidate selection.
    LockSelection(GeneralBatchReplayV1),
    /// Begin selected-candidate settlement.
    BeginSettlement(ContentId),
    /// Collect one page's negative settlement inputs.
    CollectSettlementPage(GeneralCandidatePageV1),
    /// Perform the sole complete-set materialization boundary.
    MaterializeSettlement(ContentId),
    /// Distribute one page's positive settlement outputs.
    DistributeSettlementPage(GeneralCandidatePageV1),
    /// Finish selected-candidate settlement.
    FinishSettlement(ContentId),
    /// Close one quiescent batch.
    CloseBatch(GeneralBatchReplayV1),
    /// Enter General quiescence for one generation.
    Quiesce(u64),
    /// Close one terminal General cluster for one generation.
    CloseGeneral(u64),
    /// Close one unavailable candidate.
    CloseCandidate(ContentId),
    /// Close one finished settlement cursor.
    CloseSettlement(ContentId),
    /// Close one candidate page and pay its cleanup bounty.
    CloseCandidatePage(GeneralCandidatePageV1),
    /// Reject one abandoned candidate after its deadline.
    RejectCandidate(ContentId),
    /// Expire one selected candidate before physical collection begins.
    ExpireSettlement(ContentId),
}

impl<const N: usize> GeneralInstructionV1<N> {
    /// Decode only the exact action tag after authenticating the common header.
    pub fn decode_tag(bytes: &[u8]) -> Result<GeneralInstructionTagV1> {
        validate_instruction_header::<N>(bytes)?;
        GeneralInstructionTagV1::decode(read_u8(bytes, 10)?)
    }

    /// Decode the exact activation payload without constructing the maximum instruction enum.
    pub fn decode_activate(bytes: &[u8]) -> Result<ActivateGeneralV1> {
        require_instruction_tag::<N>(bytes, GeneralInstructionTagV1::Activate)?;
        exact_len(bytes, GENERAL_INSTRUCTION_HEADER_BYTES + 8)?;
        Ok(ActivateGeneralV1 {
            expected_market_child_count: read_u64(bytes, GENERAL_INSTRUCTION_HEADER_BYTES)?,
        })
    }

    /// Decode one exact generation/batch replay payload for `expected`.
    pub fn decode_batch_replay(
        bytes: &[u8],
        expected: GeneralInstructionTagV1,
    ) -> Result<GeneralBatchReplayV1> {
        if !matches!(
            expected,
            GeneralInstructionTagV1::OpenBatch
                | GeneralInstructionTagV1::LockBatch
                | GeneralInstructionTagV1::LockSelection
                | GeneralInstructionTagV1::CloseBatch
        ) {
            return Err(Error::UnknownAction);
        }
        require_instruction_tag::<N>(bytes, expected)?;
        exact_len(bytes, GENERAL_INSTRUCTION_HEADER_BYTES + 16)?;
        Ok(GeneralBatchReplayV1 {
            generation: read_u64(bytes, GENERAL_INSTRUCTION_HEADER_BYTES)?,
            batch_sequence: read_u64(bytes, GENERAL_INSTRUCTION_HEADER_BYTES + 8)?,
        })
    }

    /// Decode one exact order payload for an order lifecycle action.
    pub fn decode_order(
        bytes: &[u8],
        expected: GeneralInstructionTagV1,
    ) -> Result<PortfolioOrderV1<N>> {
        if !matches!(
            expected,
            GeneralInstructionTagV1::AdmitOrder
                | GeneralInstructionTagV1::CancelOrder
                | GeneralInstructionTagV1::CloseOrder
        ) {
            return Err(Error::UnknownAction);
        }
        require_instruction_tag::<N>(bytes, expected)?;
        let order_bytes = PortfolioOrderV1::<N>::encoded_len()?;
        exact_len(bytes, GENERAL_INSTRUCTION_HEADER_BYTES + order_bytes)?;
        PortfolioOrderV1::decode(subslice(
            bytes,
            GENERAL_INSTRUCTION_HEADER_BYTES,
            order_bytes,
        )?)
    }

    /// Decode the exact candidate-submission payload.
    pub fn decode_candidate_submission(bytes: &[u8]) -> Result<SubmitGeneralCandidateV1<N>> {
        require_instruction_tag::<N>(bytes, GeneralInstructionTagV1::SubmitCandidate)?;
        let submission_bytes = CandidateSubmissionV1::<N>::encoded_len()?;
        exact_len(
            bytes,
            GENERAL_INSTRUCTION_HEADER_BYTES + CONTENT_ID_BYTES + submission_bytes,
        )?;
        Ok(SubmitGeneralCandidateV1 {
            candidate_id: read_id(bytes, GENERAL_INSTRUCTION_HEADER_BYTES)?,
            submission: CandidateSubmissionV1::decode(subslice(
                bytes,
                GENERAL_INSTRUCTION_HEADER_BYTES + CONTENT_ID_BYTES,
                submission_bytes,
            )?)?,
        })
    }

    /// Decode the exact candidate-page creation payload.
    pub fn decode_candidate_page_creation(bytes: &[u8]) -> Result<CreateGeneralCandidatePageV1<N>> {
        require_instruction_tag::<N>(bytes, GeneralInstructionTagV1::CreateCandidatePage)?;
        let prefix = GENERAL_INSTRUCTION_HEADER_BYTES + 2 * CONTENT_ID_BYTES;
        if bytes.len() < prefix + CANDIDATE_PAGE_BASE_BYTES {
            return Err(Error::InvalidLength);
        }
        Ok(CreateGeneralCandidatePageV1 {
            candidate_id: read_id(bytes, GENERAL_INSTRUCTION_HEADER_BYTES)?,
            page_id: read_id(bytes, GENERAL_INSTRUCTION_HEADER_BYTES + CONTENT_ID_BYTES)?,
            page: CandidatePageV1::decode(bytes.get(prefix..).ok_or(Error::InvalidLength)?)?,
        })
    }

    /// Decode one exact candidate/page reference for `expected`.
    pub fn decode_candidate_page_reference(
        bytes: &[u8],
        expected: GeneralInstructionTagV1,
    ) -> Result<GeneralCandidatePageV1> {
        if !matches!(
            expected,
            GeneralInstructionTagV1::VerifyCandidatePage
                | GeneralInstructionTagV1::CollectSettlementPage
                | GeneralInstructionTagV1::DistributeSettlementPage
                | GeneralInstructionTagV1::CloseCandidatePage
        ) {
            return Err(Error::UnknownAction);
        }
        require_instruction_tag::<N>(bytes, expected)?;
        exact_len(
            bytes,
            GENERAL_INSTRUCTION_HEADER_BYTES + 2 * CONTENT_ID_BYTES,
        )?;
        Ok(GeneralCandidatePageV1 {
            candidate_id: read_id(bytes, GENERAL_INSTRUCTION_HEADER_BYTES)?,
            page_id: read_id(bytes, GENERAL_INSTRUCTION_HEADER_BYTES + CONTENT_ID_BYTES)?,
        })
    }

    /// Decode one exact candidate identity for `expected`.
    pub fn decode_candidate_id(
        bytes: &[u8],
        expected: GeneralInstructionTagV1,
    ) -> Result<ContentId> {
        if !matches!(
            expected,
            GeneralInstructionTagV1::FinishCandidate
                | GeneralInstructionTagV1::ConsiderCandidate
                | GeneralInstructionTagV1::BeginSettlement
                | GeneralInstructionTagV1::MaterializeSettlement
                | GeneralInstructionTagV1::FinishSettlement
                | GeneralInstructionTagV1::CloseCandidate
                | GeneralInstructionTagV1::CloseSettlement
                | GeneralInstructionTagV1::RejectCandidate
                | GeneralInstructionTagV1::ExpireSettlement
        ) {
            return Err(Error::UnknownAction);
        }
        require_instruction_tag::<N>(bytes, expected)?;
        exact_len(bytes, GENERAL_INSTRUCTION_HEADER_BYTES + CONTENT_ID_BYTES)?;
        read_id(bytes, GENERAL_INSTRUCTION_HEADER_BYTES)
    }

    /// Decode one exact generation payload for `expected`.
    pub fn decode_generation(bytes: &[u8], expected: GeneralInstructionTagV1) -> Result<u64> {
        if !matches!(
            expected,
            GeneralInstructionTagV1::Quiesce | GeneralInstructionTagV1::CloseGeneral
        ) {
            return Err(Error::UnknownAction);
        }
        require_instruction_tag::<N>(bytes, expected)?;
        exact_len(bytes, GENERAL_INSTRUCTION_HEADER_BYTES + 8)?;
        read_u64(bytes, GENERAL_INSTRUCTION_HEADER_BYTES)
    }

    /// Decode exactly one action, refusing width substitution and trailing bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let tag = Self::decode_tag(bytes)?;
        match tag {
            GeneralInstructionTagV1::Activate => Ok(Self::Activate(Self::decode_activate(bytes)?)),
            GeneralInstructionTagV1::OpenBatch
            | GeneralInstructionTagV1::LockBatch
            | GeneralInstructionTagV1::LockSelection
            | GeneralInstructionTagV1::CloseBatch => {
                let replay = Self::decode_batch_replay(bytes, tag)?;
                Ok(match tag {
                    GeneralInstructionTagV1::OpenBatch => Self::OpenBatch(replay),
                    GeneralInstructionTagV1::LockBatch => Self::LockBatch(replay),
                    GeneralInstructionTagV1::LockSelection => Self::LockSelection(replay),
                    GeneralInstructionTagV1::CloseBatch => Self::CloseBatch(replay),
                    _ => return Err(Error::UnknownAction),
                })
            }
            GeneralInstructionTagV1::AdmitOrder
            | GeneralInstructionTagV1::CancelOrder
            | GeneralInstructionTagV1::CloseOrder => {
                let order = Self::decode_order(bytes, tag)?;
                Ok(match tag {
                    GeneralInstructionTagV1::AdmitOrder => Self::AdmitOrder(order),
                    GeneralInstructionTagV1::CancelOrder => Self::CancelOrder(order),
                    GeneralInstructionTagV1::CloseOrder => Self::CloseOrder(order),
                    _ => return Err(Error::UnknownAction),
                })
            }
            GeneralInstructionTagV1::SubmitCandidate => Ok(Self::SubmitCandidate(
                Self::decode_candidate_submission(bytes)?,
            )),
            GeneralInstructionTagV1::CreateCandidatePage => Ok(Self::CreateCandidatePage(
                Self::decode_candidate_page_creation(bytes)?,
            )),
            GeneralInstructionTagV1::VerifyCandidatePage
            | GeneralInstructionTagV1::CollectSettlementPage
            | GeneralInstructionTagV1::DistributeSettlementPage => {
                let page = Self::decode_candidate_page_reference(bytes, tag)?;
                Ok(match tag {
                    GeneralInstructionTagV1::VerifyCandidatePage => Self::VerifyCandidatePage(page),
                    GeneralInstructionTagV1::CollectSettlementPage => {
                        Self::CollectSettlementPage(page)
                    }
                    GeneralInstructionTagV1::DistributeSettlementPage => {
                        Self::DistributeSettlementPage(page)
                    }
                    _ => return Err(Error::UnknownAction),
                })
            }
            GeneralInstructionTagV1::FinishCandidate
            | GeneralInstructionTagV1::ConsiderCandidate
            | GeneralInstructionTagV1::BeginSettlement
            | GeneralInstructionTagV1::MaterializeSettlement
            | GeneralInstructionTagV1::FinishSettlement
            | GeneralInstructionTagV1::CloseCandidate
            | GeneralInstructionTagV1::CloseSettlement
            | GeneralInstructionTagV1::RejectCandidate
            | GeneralInstructionTagV1::ExpireSettlement => {
                let candidate_id = Self::decode_candidate_id(bytes, tag)?;
                Ok(match tag {
                    GeneralInstructionTagV1::FinishCandidate => Self::FinishCandidate(candidate_id),
                    GeneralInstructionTagV1::ConsiderCandidate => {
                        Self::ConsiderCandidate(candidate_id)
                    }
                    GeneralInstructionTagV1::BeginSettlement => Self::BeginSettlement(candidate_id),
                    GeneralInstructionTagV1::MaterializeSettlement => {
                        Self::MaterializeSettlement(candidate_id)
                    }
                    GeneralInstructionTagV1::FinishSettlement => {
                        Self::FinishSettlement(candidate_id)
                    }
                    GeneralInstructionTagV1::CloseCandidate => Self::CloseCandidate(candidate_id),
                    GeneralInstructionTagV1::CloseSettlement => Self::CloseSettlement(candidate_id),
                    GeneralInstructionTagV1::RejectCandidate => Self::RejectCandidate(candidate_id),
                    GeneralInstructionTagV1::ExpireSettlement => {
                        Self::ExpireSettlement(candidate_id)
                    }
                    _ => return Err(Error::UnknownAction),
                })
            }
            GeneralInstructionTagV1::CloseCandidatePage => Ok(Self::CloseCandidatePage(
                Self::decode_candidate_page_reference(bytes, tag)?,
            )),
            GeneralInstructionTagV1::Quiesce | GeneralInstructionTagV1::CloseGeneral => {
                let generation = Self::decode_generation(bytes, tag)?;
                Ok(match tag {
                    GeneralInstructionTagV1::Quiesce => Self::Quiesce(generation),
                    GeneralInstructionTagV1::CloseGeneral => Self::CloseGeneral(generation),
                    _ => return Err(Error::UnknownAction),
                })
            }
        }
    }

    /// Return the exact wire width for this action.
    pub fn encoded_len(&self) -> Result<usize> {
        match self {
            Self::Activate(_) => Ok(GENERAL_INSTRUCTION_HEADER_BYTES + 8),
            Self::OpenBatch(_)
            | Self::LockBatch(_)
            | Self::LockSelection(_)
            | Self::CloseBatch(_) => Ok(GENERAL_INSTRUCTION_HEADER_BYTES + 16),
            Self::AdmitOrder(_) | Self::CancelOrder(_) | Self::CloseOrder(_) => {
                GENERAL_INSTRUCTION_HEADER_BYTES
                    .checked_add(PortfolioOrderV1::<N>::encoded_len()?)
                    .ok_or(Error::ArithmeticOverflow)
            }
            Self::SubmitCandidate(_) => GENERAL_INSTRUCTION_HEADER_BYTES
                .checked_add(CONTENT_ID_BYTES)
                .and_then(|value| {
                    CandidateSubmissionV1::<N>::encoded_len()
                        .ok()
                        .and_then(|length| value.checked_add(length))
                })
                .ok_or(Error::ArithmeticOverflow),
            Self::CreateCandidatePage(page) => GENERAL_INSTRUCTION_HEADER_BYTES
                .checked_add(2 * CONTENT_ID_BYTES)
                .and_then(|value| {
                    CandidatePageV1::<N>::encoded_len(page.page.execution_count)
                        .ok()
                        .and_then(|length| value.checked_add(length))
                })
                .ok_or(Error::ArithmeticOverflow),
            Self::VerifyCandidatePage(_)
            | Self::CollectSettlementPage(_)
            | Self::DistributeSettlementPage(_)
            | Self::CloseCandidatePage(_) => {
                Ok(GENERAL_INSTRUCTION_HEADER_BYTES + 2 * CONTENT_ID_BYTES)
            }
            Self::FinishCandidate(_)
            | Self::ConsiderCandidate(_)
            | Self::BeginSettlement(_)
            | Self::MaterializeSettlement(_)
            | Self::FinishSettlement(_)
            | Self::CloseCandidate(_)
            | Self::CloseSettlement(_)
            | Self::RejectCandidate(_)
            | Self::ExpireSettlement(_) => Ok(GENERAL_INSTRUCTION_HEADER_BYTES + CONTENT_ID_BYTES),
            Self::Quiesce(_) | Self::CloseGeneral(_) => Ok(GENERAL_INSTRUCTION_HEADER_BYTES + 8),
        }
    }

    /// Encode exactly one action without maximum-width or page padding.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        exact_len(out, self.encoded_len()?)?;
        out.fill(0);
        put(out, 0, &GENERAL_INSTRUCTION_MAGIC_V1);
        put(out, 8, &GENERAL_INSTRUCTION_SCHEMA_V1.to_le_bytes());
        put(out, 10, &[self.tag() as u8]);
        put(
            out,
            11,
            &[u8::try_from(N).map_err(|_| Error::InvalidOutcomeCount)?],
        );
        match self {
            Self::Activate(instruction) => {
                put(
                    out,
                    GENERAL_INSTRUCTION_HEADER_BYTES,
                    &instruction.expected_market_child_count.to_le_bytes(),
                );
            }
            Self::OpenBatch(replay)
            | Self::LockBatch(replay)
            | Self::LockSelection(replay)
            | Self::CloseBatch(replay) => encode_batch_replay(out, *replay),
            Self::AdmitOrder(order) | Self::CancelOrder(order) | Self::CloseOrder(order) => {
                order.encode(
                    out.get_mut(GENERAL_INSTRUCTION_HEADER_BYTES..)
                        .ok_or(Error::InvalidLength)?,
                )?;
            }
            Self::SubmitCandidate(instruction) => {
                put(
                    out,
                    GENERAL_INSTRUCTION_HEADER_BYTES,
                    instruction.candidate_id.as_bytes(),
                );
                instruction.submission.encode(
                    out.get_mut(GENERAL_INSTRUCTION_HEADER_BYTES + CONTENT_ID_BYTES..)
                        .ok_or(Error::InvalidLength)?,
                )?;
            }
            Self::CreateCandidatePage(instruction) => {
                put(
                    out,
                    GENERAL_INSTRUCTION_HEADER_BYTES,
                    instruction.candidate_id.as_bytes(),
                );
                put(
                    out,
                    GENERAL_INSTRUCTION_HEADER_BYTES + CONTENT_ID_BYTES,
                    instruction.page_id.as_bytes(),
                );
                instruction.page.encode(
                    out.get_mut(GENERAL_INSTRUCTION_HEADER_BYTES + 2 * CONTENT_ID_BYTES..)
                        .ok_or(Error::InvalidLength)?,
                )?;
            }
            Self::VerifyCandidatePage(instruction)
            | Self::CollectSettlementPage(instruction)
            | Self::DistributeSettlementPage(instruction)
            | Self::CloseCandidatePage(instruction) => {
                put(
                    out,
                    GENERAL_INSTRUCTION_HEADER_BYTES,
                    instruction.candidate_id.as_bytes(),
                );
                put(
                    out,
                    GENERAL_INSTRUCTION_HEADER_BYTES + CONTENT_ID_BYTES,
                    instruction.page_id.as_bytes(),
                );
            }
            Self::FinishCandidate(candidate_id)
            | Self::ConsiderCandidate(candidate_id)
            | Self::BeginSettlement(candidate_id)
            | Self::MaterializeSettlement(candidate_id)
            | Self::FinishSettlement(candidate_id)
            | Self::CloseCandidate(candidate_id)
            | Self::CloseSettlement(candidate_id)
            | Self::RejectCandidate(candidate_id)
            | Self::ExpireSettlement(candidate_id) => {
                put(
                    out,
                    GENERAL_INSTRUCTION_HEADER_BYTES,
                    candidate_id.as_bytes(),
                );
            }
            Self::Quiesce(generation) | Self::CloseGeneral(generation) => put(
                out,
                GENERAL_INSTRUCTION_HEADER_BYTES,
                &generation.to_le_bytes(),
            ),
        }
        Ok(())
    }

    /// Return the closed action discriminator.
    pub const fn tag(&self) -> GeneralInstructionTagV1 {
        match self {
            Self::Activate(_) => GeneralInstructionTagV1::Activate,
            Self::OpenBatch(_) => GeneralInstructionTagV1::OpenBatch,
            Self::LockBatch(_) => GeneralInstructionTagV1::LockBatch,
            Self::AdmitOrder(_) => GeneralInstructionTagV1::AdmitOrder,
            Self::CancelOrder(_) => GeneralInstructionTagV1::CancelOrder,
            Self::CloseOrder(_) => GeneralInstructionTagV1::CloseOrder,
            Self::SubmitCandidate(_) => GeneralInstructionTagV1::SubmitCandidate,
            Self::CreateCandidatePage(_) => GeneralInstructionTagV1::CreateCandidatePage,
            Self::VerifyCandidatePage(_) => GeneralInstructionTagV1::VerifyCandidatePage,
            Self::FinishCandidate(_) => GeneralInstructionTagV1::FinishCandidate,
            Self::ConsiderCandidate(_) => GeneralInstructionTagV1::ConsiderCandidate,
            Self::LockSelection(_) => GeneralInstructionTagV1::LockSelection,
            Self::BeginSettlement(_) => GeneralInstructionTagV1::BeginSettlement,
            Self::CollectSettlementPage(_) => GeneralInstructionTagV1::CollectSettlementPage,
            Self::MaterializeSettlement(_) => GeneralInstructionTagV1::MaterializeSettlement,
            Self::DistributeSettlementPage(_) => GeneralInstructionTagV1::DistributeSettlementPage,
            Self::FinishSettlement(_) => GeneralInstructionTagV1::FinishSettlement,
            Self::CloseBatch(_) => GeneralInstructionTagV1::CloseBatch,
            Self::Quiesce(_) => GeneralInstructionTagV1::Quiesce,
            Self::CloseGeneral(_) => GeneralInstructionTagV1::CloseGeneral,
            Self::CloseCandidate(_) => GeneralInstructionTagV1::CloseCandidate,
            Self::CloseSettlement(_) => GeneralInstructionTagV1::CloseSettlement,
            Self::CloseCandidatePage(_) => GeneralInstructionTagV1::CloseCandidatePage,
            Self::RejectCandidate(_) => GeneralInstructionTagV1::RejectCandidate,
            Self::ExpireSettlement(_) => GeneralInstructionTagV1::ExpireSettlement,
        }
    }
}

/// Exact width of one SVM account key projection.
pub const GENERAL_ACCOUNT_KEY_BYTES: usize = 32;
/// Canonical System Program key bytes.
pub const GENERAL_SYSTEM_PROGRAM_ID: [u8; 32] = [0; 32];
/// Canonical Rent sysvar key bytes.
pub const GENERAL_RENT_SYSVAR_ID: [u8; 32] = [
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155, 161,
    253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
];
/// Canonical Clock sysvar key bytes.
pub const GENERAL_CLOCK_SYSVAR_ID: [u8; 32] = [
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
];

/// One runtime account projection used by General physical frames.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralAccountMetaV1 {
    /// Exact account key bytes.
    pub key: [u8; GENERAL_ACCOUNT_KEY_BYTES],
    /// Whether the runtime exposed signer privilege.
    pub is_signer: bool,
    /// Whether the runtime exposed writable privilege.
    pub is_writable: bool,
    /// Whether the runtime exposed executable privilege.
    pub is_executable: bool,
}

/// Closed ordered role vocabulary for every General V1 physical frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralAccountRoleV1 {
    /// Permissionless activation signer and physical-creation recipient.
    Activator,
    /// Mutable provider-neutral Market state.
    WritableMarket,
    /// Readonly provider-neutral Market state.
    ReadonlyMarket,
    /// Immutable Realm content account.
    Realm,
    /// Realm-selected settlement mint.
    Mint,
    /// Realm-selected executable token program.
    TokenProgram,
    /// Immutable categorical ClaimBasis content account.
    ClaimBasis,
    /// Immutable capability manifest content account.
    Manifest,
    /// Canonical system-owned zero-lamport staging-cursor vacancy paired with a raw record.
    StagingCursorVacancy,
    /// Mutable generic capability FundingState source.
    CapabilityFunding,
    /// Permanent immutable General-config raw record.
    GeneralConfig,
    /// Vacant or mutable General root PDA.
    WritableRoot,
    /// Readonly General root PDA.
    ReadonlyRoot,
    /// Vacant or mutable segregated General funding PDA.
    WritableGeneralFunding,
    /// Vacant or mutable General batch PDA.
    WritableBatch,
    /// Readonly General batch PDA.
    ReadonlyBatch,
    /// Permissionless work actor and current transaction recipient.
    WorkActor,
    /// Signed order owner and admission rent payer.
    OrderOwnerPayer,
    /// Readonly signed order owner authorizing cancellation.
    OrderOwner,
    /// Vacant or mutable order replay PDA.
    WritableOrderState,
    /// Readonly order replay PDA.
    ReadonlyOrderState,
    /// Vacant or mutable exact-N order custody PDA.
    WritableOrderCustody,
    /// Owner's mutable native Position.
    OwnerPosition,
    /// Owner-controlled settlement-token source.
    QuoteSource,
    /// Token-program-owned quote escrow PDA.
    QuoteEscrow,
    /// Owner's settlement-token release destination.
    QuoteDestination,
    /// Readonly permanent RentCredit.
    ReadonlyRentCredit,
    /// Writable permanent RentCredit receiving exact closed-account rent.
    WritableRentCredit,
    /// Permissionless candidate submitter and rent payer.
    CandidateSubmitter,
    /// Vacant or mutable candidate state PDA.
    WritableCandidate,
    /// Readonly candidate state PDA.
    ReadonlyCandidate,
    /// Vacant or writable immutable candidate-page PDA during create/close.
    WritableCandidatePage,
    /// Readonly immutable candidate-page PDA during verification/collection.
    ReadonlyCandidatePage,
    /// Vacant or mutable settlement cursor PDA.
    WritableSettlementCursor,
    /// Readonly settlement cursor PDA.
    ReadonlySettlementCursor,
    /// Cursor-owned native-claim Position inventory.
    SettlementPosition,
    /// Cursor-owned settlement-token collateral escrow.
    SettlementQuoteEscrow,
    /// Provider-neutral collateral Vault/Hoard token account.
    CollateralVault,
    /// Canonical executable System Program.
    SystemProgram,
    /// Canonical readonly Rent sysvar.
    RentSysvar,
    /// Canonical readonly Clock sysvar.
    ClockSysvar,
}

/// Validated exact ordered frame geometry for one General action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralAccountFrameV1<'a> {
    tag: GeneralInstructionTagV1,
    execution_count: u8,
    accounts: &'a [GeneralAccountMetaV1],
}

impl<'a> GeneralAccountFrameV1<'a> {
    /// Validate exact length, ordered privileges, fixed identities, and aliases.
    pub fn new(
        tag: GeneralInstructionTagV1,
        execution_count: u8,
        accounts: &'a [GeneralAccountMetaV1],
    ) -> Result<Self> {
        let expected = general_frame_account_count(tag, execution_count)?;
        if accounts.len() != expected {
            return Err(Error::InvalidLength);
        }
        for (index, account) in accounts.iter().enumerate() {
            let role = general_frame_role(tag, execution_count, index)?;
            validate_general_account_role(role, *account)?;
        }
        require_general_account_alias_policy(tag, execution_count, accounts)?;
        Ok(Self {
            tag,
            execution_count,
            accounts,
        })
    }

    /// Return the selected action.
    pub const fn tag(self) -> GeneralInstructionTagV1 {
        self.tag
    }

    /// Return the exact leading execution count for page frames, otherwise zero.
    pub const fn execution_count(self) -> u8 {
        self.execution_count
    }

    /// Return the exact ordered account count.
    pub const fn account_count(self) -> usize {
        self.accounts.len()
    }

    /// Borrow the exact validated ordered account projections.
    pub const fn accounts(self) -> &'a [GeneralAccountMetaV1] {
        self.accounts
    }

    /// Return one canonical ordered role.
    pub fn role(self, index: usize) -> Result<GeneralAccountRoleV1> {
        if index >= self.accounts.len() {
            return Err(Error::InvalidLength);
        }
        general_frame_role(self.tag, self.execution_count, index)
    }
}

fn general_frame_account_count(tag: GeneralInstructionTagV1, count: u8) -> Result<usize> {
    let page_count = usize::from(count);
    let fixed = match tag {
        GeneralInstructionTagV1::Activate => 19,
        GeneralInstructionTagV1::OpenBatch => 10,
        GeneralInstructionTagV1::LockBatch => 7,
        GeneralInstructionTagV1::AdmitOrder => 21,
        GeneralInstructionTagV1::CancelOrder => 20,
        GeneralInstructionTagV1::CloseOrder => 18,
        GeneralInstructionTagV1::SubmitCandidate => 10,
        GeneralInstructionTagV1::FinishCandidate => 8,
        GeneralInstructionTagV1::ConsiderCandidate => 8,
        GeneralInstructionTagV1::LockSelection => 7,
        GeneralInstructionTagV1::BeginSettlement => 20,
        GeneralInstructionTagV1::MaterializeSettlement => 18,
        GeneralInstructionTagV1::FinishSettlement => 17,
        GeneralInstructionTagV1::CloseBatch => 7,
        GeneralInstructionTagV1::Quiesce => 1,
        GeneralInstructionTagV1::CloseGeneral => 7,
        GeneralInstructionTagV1::CloseCandidate => 8,
        GeneralInstructionTagV1::CloseSettlement => 18,
        GeneralInstructionTagV1::CreateCandidatePage => 10,
        GeneralInstructionTagV1::CloseCandidatePage => 9,
        GeneralInstructionTagV1::RejectCandidate | GeneralInstructionTagV1::ExpireSettlement => 8,
        GeneralInstructionTagV1::VerifyCandidatePage => {
            require_page_count(count)?;
            return 9usize
                .checked_add(page_count)
                .ok_or(Error::ArithmeticOverflow);
        }
        GeneralInstructionTagV1::CollectSettlementPage => {
            require_page_count(count)?;
            return 18usize
                .checked_add(page_count.checked_mul(4).ok_or(Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow);
        }
        GeneralInstructionTagV1::DistributeSettlementPage => {
            require_page_count(count)?;
            return 19usize
                .checked_add(page_count.checked_mul(2).ok_or(Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow);
        }
    };
    if count != 0 {
        return Err(Error::InvalidPageCount);
    }
    Ok(fixed)
}

fn general_frame_role(
    tag: GeneralInstructionTagV1,
    count: u8,
    index: usize,
) -> Result<GeneralAccountRoleV1> {
    use GeneralAccountRoleV1 as Role;
    let role = match tag {
        GeneralInstructionTagV1::Activate => *[
            Role::Activator,
            Role::WritableMarket,
            Role::Realm,
            Role::ClaimBasis,
            Role::Manifest,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::Mint,
            Role::TokenProgram,
            Role::CapabilityFunding,
            Role::WritableRoot,
            Role::WritableGeneralFunding,
            Role::WritableRentCredit,
            Role::SystemProgram,
            Role::RentSysvar,
            Role::ClockSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::OpenBatch => *[
            Role::WorkActor,
            Role::ReadonlyMarket,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::WritableRoot,
            Role::WritableBatch,
            Role::WritableRentCredit,
            Role::SystemProgram,
            Role::RentSysvar,
            Role::ClockSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::LockBatch => *[
            Role::WorkActor,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::ReadonlyRoot,
            Role::WritableBatch,
            Role::RentSysvar,
            Role::ClockSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::AdmitOrder => *[
            Role::OrderOwnerPayer,
            Role::ReadonlyMarket,
            Role::Realm,
            Role::ClaimBasis,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::Mint,
            Role::TokenProgram,
            Role::ReadonlyRoot,
            Role::ReadonlyBatch,
            Role::WritableOrderState,
            Role::WritableOrderCustody,
            Role::OwnerPosition,
            Role::QuoteSource,
            Role::QuoteEscrow,
            Role::WritableRentCredit,
            Role::SystemProgram,
            Role::RentSysvar,
            Role::ClockSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::CancelOrder => *[
            Role::OrderOwner,
            Role::ReadonlyMarket,
            Role::Realm,
            Role::ClaimBasis,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::ReadonlyRoot,
            Role::ReadonlyBatch,
            Role::WritableOrderState,
            Role::WritableOrderCustody,
            Role::OwnerPosition,
            Role::QuoteEscrow,
            Role::QuoteDestination,
            Role::Mint,
            Role::TokenProgram,
            Role::WritableRentCredit,
            Role::RentSysvar,
            Role::ClockSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::CloseOrder => *[
            Role::ReadonlyMarket,
            Role::Realm,
            Role::ClaimBasis,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::ReadonlyRoot,
            Role::ReadonlyBatch,
            Role::WritableOrderState,
            Role::WritableOrderCustody,
            Role::OwnerPosition,
            Role::QuoteEscrow,
            Role::QuoteDestination,
            Role::Mint,
            Role::TokenProgram,
            Role::WritableRentCredit,
            Role::RentSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::SubmitCandidate => *[
            Role::CandidateSubmitter,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::ReadonlyRoot,
            Role::WritableBatch,
            Role::WritableCandidate,
            Role::WritableRentCredit,
            Role::SystemProgram,
            Role::RentSysvar,
            Role::ClockSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::VerifyCandidatePage => {
            require_page_count(count)?;
            let execution_end = 7usize
                .checked_add(usize::from(count))
                .ok_or(Error::ArithmeticOverflow)?;
            if index < 7 {
                *[
                    Role::WorkActor,
                    Role::ReadonlyRoot,
                    Role::ReadonlyBatch,
                    Role::GeneralConfig,
                    Role::StagingCursorVacancy,
                    Role::WritableCandidate,
                    Role::ReadonlyCandidatePage,
                ]
                .get(index)
                .ok_or(Error::InvalidLength)?
            } else if index < execution_end {
                Role::ReadonlyOrderState
            } else {
                *[Role::RentSysvar, Role::ClockSysvar]
                    .get(index - execution_end)
                    .ok_or(Error::InvalidLength)?
            }
        }
        GeneralInstructionTagV1::FinishCandidate => *[
            Role::WorkActor,
            Role::ReadonlyRoot,
            Role::ReadonlyBatch,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::WritableCandidate,
            Role::RentSysvar,
            Role::ClockSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::ConsiderCandidate => *[
            Role::WorkActor,
            Role::ReadonlyRoot,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::WritableBatch,
            Role::WritableCandidate,
            Role::RentSysvar,
            Role::ClockSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::LockSelection => *[
            Role::WorkActor,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::ReadonlyRoot,
            Role::WritableBatch,
            Role::RentSysvar,
            Role::ClockSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::BeginSettlement => *[
            Role::WorkActor,
            Role::ReadonlyMarket,
            Role::Realm,
            Role::ClaimBasis,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::Mint,
            Role::TokenProgram,
            Role::ReadonlyRoot,
            Role::WritableBatch,
            Role::WritableCandidate,
            Role::WritableSettlementCursor,
            Role::SettlementPosition,
            Role::SettlementQuoteEscrow,
            Role::WritableRentCredit,
            Role::SystemProgram,
            Role::RentSysvar,
            Role::ClockSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::CollectSettlementPage => {
            require_page_count(count)?;
            let execution_end = 17usize
                .checked_add(
                    usize::from(count)
                        .checked_mul(4)
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            if index < 17 {
                *[
                    Role::WorkActor,
                    Role::ReadonlyMarket,
                    Role::Realm,
                    Role::ClaimBasis,
                    Role::GeneralConfig,
                    Role::StagingCursorVacancy,
                    Role::StagingCursorVacancy,
                    Role::StagingCursorVacancy,
                    Role::Mint,
                    Role::TokenProgram,
                    Role::ReadonlyRoot,
                    Role::ReadonlyBatch,
                    Role::WritableCandidate,
                    Role::WritableSettlementCursor,
                    Role::SettlementPosition,
                    Role::SettlementQuoteEscrow,
                    Role::ReadonlyCandidatePage,
                ]
                .get(index)
                .ok_or(Error::InvalidLength)?
            } else if index < execution_end {
                *[
                    Role::WritableOrderState,
                    Role::WritableOrderCustody,
                    Role::OwnerPosition,
                    Role::QuoteEscrow,
                ]
                .get((index - 17) % 4)
                .ok_or(Error::InvalidLength)?
            } else {
                *[Role::RentSysvar]
                    .get(index - execution_end)
                    .ok_or(Error::InvalidLength)?
            }
        }
        GeneralInstructionTagV1::MaterializeSettlement => *[
            Role::WorkActor,
            Role::WritableMarket,
            Role::Realm,
            Role::ClaimBasis,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::Mint,
            Role::TokenProgram,
            Role::CollateralVault,
            Role::ReadonlyRoot,
            Role::ReadonlyBatch,
            Role::WritableCandidate,
            Role::WritableSettlementCursor,
            Role::SettlementPosition,
            Role::SettlementQuoteEscrow,
            Role::RentSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::DistributeSettlementPage => {
            require_page_count(count)?;
            let execution_end = 18usize
                .checked_add(
                    usize::from(count)
                        .checked_mul(2)
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            if index < 18 {
                *[
                    Role::WorkActor,
                    Role::ReadonlyMarket,
                    Role::Realm,
                    Role::ClaimBasis,
                    Role::GeneralConfig,
                    Role::StagingCursorVacancy,
                    Role::StagingCursorVacancy,
                    Role::StagingCursorVacancy,
                    Role::Mint,
                    Role::TokenProgram,
                    Role::ReadonlyRoot,
                    Role::ReadonlyBatch,
                    Role::WritableCandidate,
                    Role::WritableSettlementCursor,
                    Role::SettlementPosition,
                    Role::SettlementQuoteEscrow,
                    Role::WritableCandidatePage,
                    Role::WritableRentCredit,
                ]
                .get(index)
                .ok_or(Error::InvalidLength)?
            } else if index < execution_end {
                *[Role::OwnerPosition, Role::QuoteDestination]
                    .get((index - 18) % 2)
                    .ok_or(Error::InvalidLength)?
            } else {
                *[Role::RentSysvar]
                    .get(index - execution_end)
                    .ok_or(Error::InvalidLength)?
            }
        }
        GeneralInstructionTagV1::FinishSettlement => *[
            Role::WorkActor,
            Role::ReadonlyMarket,
            Role::Realm,
            Role::ClaimBasis,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::Mint,
            Role::TokenProgram,
            Role::ReadonlyRoot,
            Role::WritableBatch,
            Role::WritableCandidate,
            Role::WritableSettlementCursor,
            Role::SettlementPosition,
            Role::SettlementQuoteEscrow,
            Role::RentSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::CloseBatch => *[
            Role::WorkActor,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::WritableRoot,
            Role::WritableBatch,
            Role::WritableRentCredit,
            Role::RentSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::Quiesce => *[Role::WritableRoot]
            .get(index)
            .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::CloseGeneral => *[
            Role::WritableMarket,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::WritableRoot,
            Role::WritableGeneralFunding,
            Role::WritableRentCredit,
            Role::RentSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::CloseCandidate => *[
            Role::WorkActor,
            Role::ReadonlyRoot,
            Role::WritableBatch,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::WritableCandidate,
            Role::WritableRentCredit,
            Role::RentSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::CloseSettlement => *[
            Role::WorkActor,
            Role::ReadonlyMarket,
            Role::Realm,
            Role::ClaimBasis,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::StagingCursorVacancy,
            Role::Mint,
            Role::TokenProgram,
            Role::ReadonlyRoot,
            Role::WritableBatch,
            Role::WritableCandidate,
            Role::WritableSettlementCursor,
            Role::SettlementPosition,
            Role::SettlementQuoteEscrow,
            Role::WritableRentCredit,
            Role::RentSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::CreateCandidatePage => *[
            Role::WorkActor,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::ReadonlyRoot,
            Role::ReadonlyBatch,
            Role::WritableCandidate,
            Role::WritableCandidatePage,
            Role::WritableRentCredit,
            Role::SystemProgram,
            Role::RentSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::CloseCandidatePage => *[
            Role::WorkActor,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::ReadonlyRoot,
            Role::ReadonlyBatch,
            Role::WritableCandidate,
            Role::WritableCandidatePage,
            Role::WritableRentCredit,
            Role::RentSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::RejectCandidate => *[
            Role::WorkActor,
            Role::ReadonlyRoot,
            Role::ReadonlyBatch,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::WritableCandidate,
            Role::RentSysvar,
            Role::ClockSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
        GeneralInstructionTagV1::ExpireSettlement => *[
            Role::WorkActor,
            Role::ReadonlyRoot,
            Role::WritableBatch,
            Role::GeneralConfig,
            Role::StagingCursorVacancy,
            Role::WritableCandidate,
            Role::RentSysvar,
            Role::ClockSysvar,
        ]
        .get(index)
        .ok_or(Error::InvalidLength)?,
    };
    Ok(role)
}

fn validate_general_account_role(
    role: GeneralAccountRoleV1,
    account: GeneralAccountMetaV1,
) -> Result<()> {
    use GeneralAccountRoleV1 as Role;
    let (signer, writable, executable) = match role {
        Role::Activator | Role::WorkActor | Role::OrderOwnerPayer | Role::CandidateSubmitter => {
            (true, true, false)
        }
        Role::OrderOwner => (true, false, false),
        Role::TokenProgram | Role::SystemProgram => (false, false, true),
        Role::WritableMarket
        | Role::CapabilityFunding
        | Role::WritableRoot
        | Role::WritableGeneralFunding
        | Role::WritableBatch
        | Role::WritableOrderState
        | Role::WritableOrderCustody
        | Role::OwnerPosition
        | Role::QuoteSource
        | Role::QuoteEscrow
        | Role::QuoteDestination
        | Role::WritableRentCredit
        | Role::WritableCandidate
        | Role::WritableCandidatePage
        | Role::WritableSettlementCursor
        | Role::SettlementPosition
        | Role::SettlementQuoteEscrow
        | Role::CollateralVault => (false, true, false),
        _ => (false, false, false),
    };
    if account.is_signer != signer
        || account.is_writable != writable
        || account.is_executable != executable
    {
        return Err(Error::InvalidAccountPrivilege);
    }
    match role {
        Role::SystemProgram if account.key != GENERAL_SYSTEM_PROGRAM_ID => {
            Err(Error::InvalidAccountPrivilege)
        }
        Role::RentSysvar if account.key != GENERAL_RENT_SYSVAR_ID => {
            Err(Error::InvalidAccountPrivilege)
        }
        Role::ClockSysvar if account.key != GENERAL_CLOCK_SYSVAR_ID => {
            Err(Error::InvalidAccountPrivilege)
        }
        Role::SystemProgram => Ok(()),
        _ => {
            require_nonzero_key(&account.key)?;
            Ok(())
        }
    }
}

fn require_page_count(count: u8) -> Result<()> {
    if count == 0 || usize::from(count) > MAX_EXECUTIONS_PER_PAGE_V1 {
        Err(Error::InvalidPageCount)
    } else {
        Ok(())
    }
}

fn require_general_account_alias_policy(
    tag: GeneralInstructionTagV1,
    execution_count: u8,
    accounts: &[GeneralAccountMetaV1],
) -> Result<()> {
    for (index, account) in accounts.iter().enumerate() {
        for (other_index, other) in accounts.iter().enumerate().skip(index.saturating_add(1)) {
            if other.key != account.key {
                continue;
            }
            let role = general_frame_role(tag, execution_count, index)?;
            let other_role = general_frame_role(tag, execution_count, other_index)?;
            let repeatable = matches!(
                (role, other_role),
                (
                    GeneralAccountRoleV1::OwnerPosition,
                    GeneralAccountRoleV1::OwnerPosition
                ) | (
                    GeneralAccountRoleV1::QuoteDestination,
                    GeneralAccountRoleV1::QuoteDestination
                )
            );
            if !repeatable {
                return Err(Error::AccountAlias);
            }
        }
    }
    Ok(())
}

/// Adapter-observed current Rent minima for the activated General account cluster.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralActivationCapitalizationV1 {
    root_rent: u64,
    funding_rent: u64,
}

impl GeneralActivationCapitalizationV1 {
    /// Construct from trusted current Rent calculations for the two new accounts.
    pub const fn new(root_rent: u64, funding_rent: u64) -> Self {
        Self {
            root_rent,
            funding_rent,
        }
    }

    /// Return the exact root-account Rent minimum.
    pub const fn root_rent(self) -> u64 {
        self.root_rent
    }

    /// Return the exact funding-account Rent minimum.
    pub const fn funding_rent(self) -> u64 {
        self.funding_rent
    }

    fn total(self) -> Result<u64> {
        self.root_rent
            .checked_add(self.funding_rent)
            .ok_or(Error::ArithmeticOverflow)
    }
}

/// Exact content preimages and immutable identities an activation adapter hashes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralActivationCommitmentsV1<'a> {
    config_id: ContentId,
    config: GeneralConfigV1,
    market_identity_id: ContentId,
    market_identity: MarketIdentity,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'a>,
}

impl<'a> GeneralActivationCommitmentsV1<'a> {
    /// Return the expected config content identity.
    pub const fn config_id(self) -> ContentId {
        self.config_id
    }

    /// Return the exact canonical config preimage.
    pub const fn config(self) -> GeneralConfigV1 {
        self.config
    }

    /// Return the expected Market-identity content identity.
    pub const fn market_identity_id(self) -> ContentId {
        self.market_identity_id
    }

    /// Return the exact canonical Market-identity preimage owner.
    pub const fn market_identity(self) -> MarketIdentity {
        self.market_identity
    }

    /// Return the expected capability-manifest content identity.
    pub const fn manifest_id(self) -> ContentId {
        self.manifest_id
    }

    /// Return the exact canonical manifest preimage.
    pub const fn manifest(self) -> CapabilityManifestV1<'a> {
        self.manifest
    }
}

/// Complete pure plan for one atomic General activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralActivationPlanV1<'a> {
    market_root_after: MarketRoot,
    root: GeneralRootV1,
    funding: GeneralFundingActivationV1,
    capitalization: GeneralActivationCapitalizationV1,
    root_seeds: GeneralRootPdaSeedsV1,
    funding_seeds: GeneralFundingPdaSeedsV1,
    commitments: GeneralActivationCommitmentsV1<'a>,
    creation_recipient: [u8; 32],
}

impl<'a> GeneralActivationPlanV1<'a> {
    /// Return the Market root after registering one direct General child.
    pub const fn market_root_after(self) -> MarketRoot {
        self.market_root_after
    }

    /// Return the newly founded General root with immutable RentCredit beneficiary.
    pub const fn root(self) -> GeneralRootV1 {
        self.root
    }

    /// Return the generic-to-General funding transition and physical source derivation.
    pub const fn funding(self) -> GeneralFundingActivationV1 {
        self.funding
    }

    /// Return current exact account Rent minima used by the plan.
    pub const fn capitalization(self) -> GeneralActivationCapitalizationV1 {
        self.capitalization
    }

    /// Return the active General-root PDA seeds.
    pub const fn root_seeds(self) -> GeneralRootPdaSeedsV1 {
        self.root_seeds
    }

    /// Return the segregated General-funding PDA seeds.
    pub const fn funding_seeds(self) -> GeneralFundingPdaSeedsV1 {
        self.funding_seeds
    }

    /// Return every content hash obligation.
    pub const fn commitments(self) -> GeneralActivationCommitmentsV1<'a> {
        self.commitments
    }

    /// Return the frame-authenticated recipient of physical-creation principal.
    pub const fn creation_recipient(self) -> [u8; 32] {
        self.creation_recipient
    }

    /// Return the exact initial lamports required by the General funding account.
    pub fn general_funding_account_balance(self) -> Result<u64> {
        self.capitalization
            .funding_rent
            .checked_add(self.funding.general_lamports())
            .ok_or(Error::ArithmeticOverflow)
    }
}

/// Plan one exact activation without accepting allocation, status, or beneficiary bytes.
#[allow(clippy::too_many_arguments)]
pub fn activate_general_v1<'a>(
    frame: GeneralAccountFrameV1<'_>,
    instruction: ActivateGeneralV1,
    mut market_root: MarketRoot,
    config_id: ContentId,
    config: GeneralConfigV1,
    market_identity_id: ContentId,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'a>,
    capability_funding: FundingStateV1,
    capability_custody: FundingCustodyObservationV1,
    capitalization: GeneralActivationCapitalizationV1,
    current_slot: u64,
) -> Result<GeneralActivationPlanV1<'a>> {
    if frame.tag != GeneralInstructionTagV1::Activate || frame.execution_count != 0 {
        return Err(Error::InvalidInstruction);
    }
    let accounts = frame.accounts();
    let activator = accounts.first().ok_or(Error::InvalidLength)?.key;
    let market = accounts.get(1).ok_or(Error::InvalidLength)?.key;
    let identity = market_root.identity();
    if market_root.phase() != MarketPhase::Open
        || identity.generation() != config.generation
        || identity.claim_basis_id().to_bytes() != config.claim_basis_id.to_bytes()
        || identity.capability_manifest_id().to_bytes() != manifest_id.to_bytes()
    {
        return Err(Error::AuthorityMismatch);
    }
    market_root
        .register_child(config.generation, instruction.expected_market_child_count)
        .map_err(|_| Error::AuthorityMismatch)?;
    let funding = GeneralFundingV1::activate_from_capability(
        market,
        config_id,
        config,
        manifest_id,
        manifest,
        capability_funding,
        capability_custody,
        current_slot,
    )?;
    if funding.rent_lamports() != capitalization.total()? {
        return Err(Error::CapabilityFundingMismatch);
    }
    let rent_beneficiary = market_root.rent_refund();
    if accounts.get(15).ok_or(Error::InvalidLength)?.key != rent_beneficiary {
        return Err(Error::AuthorityMismatch);
    }
    let root = GeneralRootV1::founding(market, config_id, config.generation, rent_beneficiary)?;
    let root_seeds = GeneralRootPdaSeedsV1::new(market, config.generation, config_id)?;
    let funding_seeds = GeneralFundingPdaSeedsV1::new(
        market,
        config.generation,
        config_id,
        config.capability_release_id,
    )?;
    Ok(GeneralActivationPlanV1 {
        market_root_after: market_root,
        root,
        funding,
        capitalization,
        root_seeds,
        funding_seeds,
        commitments: GeneralActivationCommitmentsV1 {
            config_id,
            config,
            market_identity_id,
            market_identity: identity,
            manifest_id,
            manifest,
        },
        creation_recipient: activator,
    })
}

/// Adapter-observed native custody for the segregated General funding account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralFundingCustodyObservationV1 {
    account_lamports: u64,
    exact_account_rent_lamports: u64,
}

impl GeneralFundingCustodyObservationV1 {
    /// Construct one physical observation without accepting compartment amounts.
    pub fn new(account_lamports: u64, exact_account_rent_lamports: u64) -> Result<Self> {
        if account_lamports < exact_account_rent_lamports {
            return Err(Error::GeneralFundingCustodyMismatch);
        }
        Ok(Self {
            account_lamports,
            exact_account_rent_lamports,
        })
    }

    /// Return all observed account lamports.
    pub const fn account_lamports(self) -> u64 {
        self.account_lamports
    }

    /// Return the current Rent minimum for the exact funding-state width.
    pub const fn exact_account_rent_lamports(self) -> u64 {
        self.exact_account_rent_lamports
    }
}

/// Trusted current Rent and safe precreation balance for a new batch account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchRentObservationV1 {
    /// Current Rent minimum for the exact batch-state width.
    pub exact_batch_rent_lamports: u64,
    /// Balance of a System-owned, nonexecutable, data-empty precreation address.
    pub precreation_lamports: u64,
}

/// Adapter-observed capitalization of one live batch account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchCapitalizationV1 {
    /// Current physical lamports on the batch account.
    pub account_lamports: u64,
    /// Current Rent minimum for the exact batch-state width.
    pub exact_state_rent_lamports: u64,
}

/// Atomic permissionless batch-retirement payout plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchCloseV1 {
    /// Final prepaid reward paid to the permissionless closer.
    pub continuation_reward_lamports: u64,
    /// Batch-account Rent returned to permanent RentCredit.
    pub rent_credit_lamports: u64,
    /// Root-owned permanent RentCredit beneficiary.
    pub rent_beneficiary: [u8; 32],
}

/// Complete pure plan for one caller-capitalized General batch opening.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOpenBatchPlanV1 {
    root_after: GeneralRootV1,
    batch: BatchRootV1,
    batch_seeds: GeneralBatchPdaSeedsV1,
    batch_account_lamports: u64,
    payer_top_up_lamports: u64,
    rent_credit_surplus_lamports: u64,
    rent_beneficiary: [u8; 32],
}

impl GeneralOpenBatchPlanV1 {
    /// Return the root after reserving exactly one sequence and direct child.
    pub const fn root_after(self) -> GeneralRootV1 {
        self.root_after
    }

    /// Return the newly opened canonical collecting batch.
    pub const fn batch(self) -> BatchRootV1 {
        self.batch
    }

    /// Return the exact root/sequence batch derivation.
    pub const fn batch_seeds(self) -> GeneralBatchPdaSeedsV1 {
        self.batch_seeds
    }

    /// Return exact Rent plus all batch-owned continuation work.
    pub const fn batch_account_lamports(self) -> u64 {
        self.batch_account_lamports
    }

    /// Return the caller transfer after crediting safe precreation dust.
    pub const fn payer_top_up_lamports(self) -> u64 {
        self.payer_top_up_lamports
    }

    /// Return safe precreation surplus routed to permanent RentCredit.
    pub const fn rent_credit_surplus_lamports(self) -> u64 {
        self.rent_credit_surplus_lamports
    }

    /// Return the root-owned permanent RentCredit beneficiary.
    pub const fn rent_beneficiary(self) -> [u8; 32] {
        self.rent_beneficiary
    }
}

/// Reserve and capitalize one batch from the opening caller's present lamports.
///
/// The batch account itself owns current Rent and exactly three continuation
/// rewards: collection close, selection close, and terminal retirement. General
/// activation funding therefore remains finite and cannot be mistaken for an
/// unbounded stream of future batch capital.
#[allow(clippy::too_many_arguments)]
pub fn open_general_batch_v1(
    frame: GeneralAccountFrameV1<'_>,
    instruction: GeneralBatchReplayV1,
    config_id: ContentId,
    config: GeneralConfigV1,
    mut root: GeneralRootV1,
    rent: BatchRentObservationV1,
    current_slot: u64,
) -> Result<GeneralOpenBatchPlanV1> {
    if frame.tag != GeneralInstructionTagV1::OpenBatch || frame.execution_count != 0 {
        return Err(Error::InvalidInstruction);
    }
    let accounts = frame.accounts();
    if instruction.generation != config.generation
        || instruction.generation != root.generation
        || instruction.batch_sequence != root.next_batch_sequence
        || root.config_id != config_id
        || root.market != accounts.get(1).ok_or(Error::InvalidLength)?.key
        || root.rent_beneficiary != accounts.get(6).ok_or(Error::InvalidLength)?.key
    {
        return Err(Error::AuthorityMismatch);
    }
    let sequence = root.open_batch()?;
    if sequence != instruction.batch_sequence {
        return Err(Error::CursorMismatch);
    }
    let batch = BatchRootV1::open(config_id, sequence, current_slot, config)?;
    let batch_seeds =
        GeneralBatchPdaSeedsV1::new(accounts.get(4).ok_or(Error::InvalidLength)?.key, sequence)?;
    let batch_account_lamports = rent
        .exact_batch_rent_lamports
        .checked_add(batch.work_remaining_lamports)
        .ok_or(Error::ArithmeticOverflow)?;
    let payer_top_up_lamports = batch_account_lamports.saturating_sub(rent.precreation_lamports);
    let rent_credit_surplus_lamports = rent
        .precreation_lamports
        .saturating_sub(batch_account_lamports);
    Ok(GeneralOpenBatchPlanV1 {
        root_after: root,
        batch,
        batch_seeds,
        batch_account_lamports,
        payer_top_up_lamports,
        rent_credit_surplus_lamports,
        rent_beneficiary: root.rent_beneficiary,
    })
}

/// Immutable capacity and authority contract for one General venue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralConfigV1 {
    capacity_profile_id: ContentId,
    claim_basis_id: ContentId,
    capability_release_id: ContentId,
    generation: u64,
    price_scale: u64,
    collection_slots: u64,
    selection_slots: u64,
    settlement_slots: u64,
    max_orders_per_candidate: u32,
    max_pages_per_candidate: u32,
    continuation_reward_lamports: u64,
    outcome_count: u16,
}

/// Inputs for one immutable [`GeneralConfigV1`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralConfigV1Input {
    /// Identity of the liftable capacity profile.
    pub capacity_profile_id: ContentId,
    /// Exact ClaimBasis content identity from that Market identity.
    pub claim_basis_id: ContentId,
    /// Reviewed General capability release selected by the manifest.
    pub capability_release_id: ContentId,
    /// Immutable occurrence generation.
    pub generation: u64,
    /// Positive integer scale whose simplex coordinates sum exactly.
    pub price_scale: u64,
    /// Positive collection-window width in slots.
    pub collection_slots: u64,
    /// Positive candidate-selection width in slots.
    pub selection_slots: u64,
    /// Positive winning-candidate settlement width in slots.
    pub settlement_slots: u64,
    /// Profile bound on executions in one candidate.
    pub max_orders_per_candidate: u32,
    /// Profile bound on verification/settlement pages.
    pub max_pages_per_candidate: u32,
    /// Exact prepaid native reward for each permissionless candidate continuation.
    pub continuation_reward_lamports: u64,
    /// Exact finite ClaimBasis width.
    pub outcome_count: u16,
}

impl GeneralConfigV1 {
    /// Validate and construct one immutable venue contract.
    pub fn new(input: GeneralConfigV1Input) -> Result<Self> {
        let outcome_count = usize::from(input.outcome_count);
        if !(2..=MAX_OUTCOMES_V1).contains(&outcome_count) {
            return Err(Error::InvalidOutcomeCount);
        }
        if input.capability_release_id != GENERAL_CAPABILITY_RELEASE_ID_V1 {
            return Err(Error::UnrecognizedCapability);
        }
        if input.price_scale == 0 {
            return Err(Error::ZeroPriceScale);
        }
        if input.collection_slots == 0
            || input.selection_slots == 0
            || input.settlement_slots == 0
            || input.max_orders_per_candidate == 0
            || input.max_pages_per_candidate == 0
            || input.continuation_reward_lamports == 0
        {
            return Err(Error::ZeroCapacity);
        }
        let page_capacity = input
            .max_pages_per_candidate
            .checked_mul(
                u32::try_from(MAX_EXECUTIONS_PER_PAGE_V1).map_err(|_| Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)?;
        if input.max_orders_per_candidate > page_capacity {
            return Err(Error::CapacityExceeded);
        }
        Ok(Self {
            capacity_profile_id: input.capacity_profile_id,
            claim_basis_id: input.claim_basis_id,
            capability_release_id: input.capability_release_id,
            generation: input.generation,
            price_scale: input.price_scale,
            collection_slots: input.collection_slots,
            selection_slots: input.selection_slots,
            settlement_slots: input.settlement_slots,
            max_orders_per_candidate: input.max_orders_per_candidate,
            max_pages_per_candidate: input.max_pages_per_candidate,
            continuation_reward_lamports: input.continuation_reward_lamports,
            outcome_count: input.outcome_count,
        })
    }

    /// Decode the exact canonical config preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        exact_len(bytes, GENERAL_CONFIG_BYTES)?;
        if array::<8>(bytes, 0)? != CONFIG_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != SCHEMA_V1 || read_u16(bytes, 10)? != ARTIFACT_PROFILE_V1 {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, 14, 2)?;
        require_zero(bytes, 168, 32)?;
        Self::new(GeneralConfigV1Input {
            outcome_count: read_u16(bytes, 12)?,
            capacity_profile_id: read_id(bytes, 16)?,
            claim_basis_id: read_id(bytes, 48)?,
            capability_release_id: read_id(bytes, 80)?,
            generation: read_u64(bytes, 112)?,
            price_scale: read_u64(bytes, 120)?,
            collection_slots: read_u64(bytes, 128)?,
            selection_slots: read_u64(bytes, 136)?,
            settlement_slots: read_u64(bytes, 144)?,
            max_orders_per_candidate: read_u32(bytes, 152)?,
            max_pages_per_candidate: read_u32(bytes, 156)?,
            continuation_reward_lamports: read_u64(bytes, 160)?,
        })
    }

    /// Encode the exact canonical config preimage.
    pub fn to_bytes(self) -> [u8; GENERAL_CONFIG_BYTES] {
        let mut out = [0u8; GENERAL_CONFIG_BYTES];
        put(&mut out, 0, &CONFIG_MAGIC);
        put(&mut out, 8, &SCHEMA_V1.to_le_bytes());
        put(&mut out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(&mut out, 12, &self.outcome_count.to_le_bytes());
        put(&mut out, 16, self.capacity_profile_id.as_bytes());
        put(&mut out, 48, self.claim_basis_id.as_bytes());
        put(&mut out, 80, self.capability_release_id.as_bytes());
        put(&mut out, 112, &self.generation.to_le_bytes());
        put(&mut out, 120, &self.price_scale.to_le_bytes());
        put(&mut out, 128, &self.collection_slots.to_le_bytes());
        put(&mut out, 136, &self.selection_slots.to_le_bytes());
        put(&mut out, 144, &self.settlement_slots.to_le_bytes());
        put(&mut out, 152, &self.max_orders_per_candidate.to_le_bytes());
        put(&mut out, 156, &self.max_pages_per_candidate.to_le_bytes());
        put(
            &mut out,
            160,
            &self.continuation_reward_lamports.to_le_bytes(),
        );
        out
    }

    /// Return the selected liftable capacity-profile identity.
    pub const fn capacity_profile_id(self) -> ContentId {
        self.capacity_profile_id
    }

    /// Return the exact ClaimBasis identity.
    pub const fn claim_basis_id(self) -> ContentId {
        self.claim_basis_id
    }

    /// Return the selected capability release identity.
    pub const fn capability_release_id(self) -> ContentId {
        self.capability_release_id
    }

    /// Return the immutable occurrence generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the exact simplex scale.
    pub const fn price_scale(self) -> u64 {
        self.price_scale
    }

    /// Return the finite ClaimBasis width.
    pub const fn outcome_count(self) -> u16 {
        self.outcome_count
    }

    /// Return the immutable candidate execution bound.
    pub const fn max_orders_per_candidate(self) -> u32 {
        self.max_orders_per_candidate
    }

    /// Return the immutable candidate page bound.
    pub const fn max_pages_per_candidate(self) -> u32 {
        self.max_pages_per_candidate
    }

    /// Return the immutable candidate-owned permissionless continuation reward.
    pub const fn continuation_reward_lamports(self) -> u64 {
        self.continuation_reward_lamports
    }
}

/// Lifecycle of the General capability root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GeneralPhase {
    /// New batches may be opened.
    Active = 0,
    /// New batches are refused while existing batches converge.
    Quiescing = 1,
    /// Every owned batch has retired and funding may be refunded.
    Terminal = 2,
    /// Funding/rent ownership has been discharged by the adapter.
    Retired = 3,
}

/// Compact mutable root for a General capability child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralRootV1 {
    config_id: ContentId,
    market: [u8; 32],
    rent_beneficiary: [u8; 32],
    generation: u64,
    next_batch_sequence: u64,
    open_batches: u32,
    phase: GeneralPhase,
}

impl GeneralRootV1 {
    /// Decode one exact-width canonical General capability root.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        exact_len(bytes, GENERAL_ROOT_BYTES)?;
        if array::<8>(bytes, 0)? != GENERAL_ROOT_MAGIC {
            return Err(Error::InvalidMagic);
        }
        validate_record_header(bytes)?;
        require_zero(bytes, 13, 3)?;
        require_zero(bytes, 132, 4)?;
        let root = Self {
            phase: decode_general_phase(read_u8(bytes, 12)?)?,
            config_id: read_id(bytes, 16)?,
            market: array::<32>(bytes, 48)?,
            rent_beneficiary: array::<32>(bytes, 80)?,
            generation: read_u64(bytes, 112)?,
            next_batch_sequence: read_u64(bytes, 120)?,
            open_batches: read_u32(bytes, 128)?,
        };
        root.validate()?;
        Ok(root)
    }

    /// Encode one exact-width canonical General capability root.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        exact_len(out, GENERAL_ROOT_BYTES)?;
        self.validate()?;
        out.fill(0);
        put(out, 0, &GENERAL_ROOT_MAGIC);
        put(out, 8, &SCHEMA_V1.to_le_bytes());
        put(out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(out, 12, &[general_phase_tag(self.phase)]);
        put(out, 16, self.config_id.as_bytes());
        put(out, 48, &self.market);
        put(out, 80, &self.rent_beneficiary);
        put(out, 112, &self.generation.to_le_bytes());
        put(out, 120, &self.next_batch_sequence.to_le_bytes());
        put(out, 128, &self.open_batches.to_le_bytes());
        Ok(())
    }

    fn validate(self) -> Result<()> {
        require_nonzero_key(&self.market)?;
        require_nonzero_key(&self.rent_beneficiary)?;
        if u64::from(self.open_batches) > self.next_batch_sequence
            || matches!(self.phase, GeneralPhase::Terminal | GeneralPhase::Retired)
                && self.open_batches != 0
        {
            return Err(Error::NonCanonicalState);
        }
        Ok(())
    }

    /// Found one active General root bound to an authenticated config.
    pub fn founding(
        market: [u8; 32],
        config_id: ContentId,
        generation: u64,
        rent_beneficiary: [u8; 32],
    ) -> Result<Self> {
        require_nonzero_key(&market)?;
        require_nonzero_key(&rent_beneficiary)?;
        Ok(Self {
            config_id,
            market,
            rent_beneficiary,
            generation,
            next_batch_sequence: 0,
            open_batches: 0,
            phase: GeneralPhase::Active,
        })
    }

    /// Validate one signed artifact against the activated root and reusable config.
    pub fn validate_authority(
        self,
        market: [u8; 32],
        claim_basis_id: ContentId,
        generation: u64,
        config: GeneralConfigV1,
    ) -> Result<()> {
        if market != self.market
            || claim_basis_id != config.claim_basis_id
            || generation != self.generation
            || generation != config.generation
        {
            return Err(Error::AuthorityMismatch);
        }
        Ok(())
    }

    /// Reserve the next unique batch sequence.
    pub fn open_batch(&mut self) -> Result<u64> {
        if self.phase != GeneralPhase::Active {
            return Err(Error::InvalidPhase);
        }
        let sequence = self.next_batch_sequence;
        self.next_batch_sequence = sequence.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        self.open_batches = self
            .open_batches
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(sequence)
    }

    /// Stop admitting batches while preserving existing settlement work.
    pub fn request_quiescence(&mut self) -> Result<()> {
        if self.phase != GeneralPhase::Active {
            return Err(Error::InvalidPhase);
        }
        self.phase = GeneralPhase::Quiescing;
        Ok(())
    }

    /// Account for one fully retired owned batch.
    pub fn close_batch(&mut self) -> Result<()> {
        if self.phase == GeneralPhase::Retired || self.open_batches == 0 {
            return Err(Error::InvalidPhase);
        }
        self.open_batches = self
            .open_batches
            .checked_sub(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    /// Enter terminal state after every batch is quiescent and retired.
    pub fn enter_terminal(&mut self) -> Result<()> {
        if self.phase != GeneralPhase::Quiescing || self.open_batches != 0 {
            return Err(Error::NotQuiescent);
        }
        self.phase = GeneralPhase::Terminal;
        Ok(())
    }

    /// Retire only after the canonical segregated funding owner is discharged.
    pub fn retire(&mut self, funding: GeneralFundingV1) -> Result<()> {
        if self.phase != GeneralPhase::Terminal || !funding.is_discharged() {
            return Err(Error::NotQuiescent);
        }
        self.phase = GeneralPhase::Retired;
        Ok(())
    }

    /// Return the current lifecycle phase.
    pub const fn phase(self) -> GeneralPhase {
        self.phase
    }

    /// Return the authenticated config commitment.
    pub const fn config_id(self) -> ContentId {
        self.config_id
    }

    /// Return the immutable provider-neutral Market account key.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Return the immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the permanent RentCredit beneficiary for all General-owned rent.
    pub const fn rent_beneficiary(self) -> [u8; 32] {
        self.rent_beneficiary
    }

    /// Return the exact direct batch-child count.
    pub const fn open_batches(self) -> u32 {
        self.open_batches
    }

    /// Return the next unreserved batch sequence.
    pub const fn next_batch_sequence(self) -> u64 {
        self.next_batch_sequence
    }
}

/// Exact signed coefficient portfolio and immutable execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioOrderV1<const N: usize> {
    market: [u8; 32],
    claim_basis_id: ContentId,
    owner: OwnerKeyV1,
    order_id: ContentId,
    generation: u64,
    batch_sequence: u64,
    nonce: u64,
    valid_until_slot: u64,
    max_lots: u64,
    max_quote_debit_per_lot_numerator: i128,
    coefficients: [i64; N],
    outcome_count: u16,
}

/// Inputs for one atomic coefficient portfolio order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioOrderV1Input<const N: usize> {
    /// Activated provider-neutral Market account key.
    pub market: [u8; 32],
    /// Exact ClaimBasis identity.
    pub claim_basis_id: ContentId,
    /// Exact Ed25519/SVM public key whose signature authorizes this order.
    pub owner: OwnerKeyV1,
    /// Unique content identity of the signed order preimage.
    pub order_id: ContentId,
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact batch sequence in which this order may execute.
    pub batch_sequence: u64,
    /// Owner-scoped replay nonce.
    pub nonce: u64,
    /// Slot through which winning settlement remains valid.
    pub valid_until_slot: u64,
    /// Maximum scalar lots; all coefficients fill by the same scalar.
    pub max_lots: u64,
    /// Upper bound on `dot(coefficients, prices)` per lot, in scale units.
    pub max_quote_debit_per_lot_numerator: i128,
    /// Signed, cell-canonical coefficients for one atomic lot.
    pub coefficients: [i64; N],
    /// Exact ClaimBasis width, which must equal the selected const width.
    pub outcome_count: u16,
}

impl<const N: usize> PortfolioOrderV1<N> {
    /// Validate and construct one atomic portfolio order.
    pub fn new(input: PortfolioOrderV1Input<N>) -> Result<Self> {
        validate_portfolio(&input.coefficients, input.outcome_count, true)?;
        if input.max_lots == 0 {
            return Err(Error::InvalidFill);
        }
        Ok(Self {
            market: input.market,
            claim_basis_id: input.claim_basis_id,
            owner: input.owner,
            order_id: input.order_id,
            generation: input.generation,
            batch_sequence: input.batch_sequence,
            nonce: input.nonce,
            valid_until_slot: input.valid_until_slot,
            max_lots: input.max_lots,
            max_quote_debit_per_lot_numerator: input.max_quote_debit_per_lot_numerator,
            coefficients: input.coefficients,
            outcome_count: input.outcome_count,
        })
    }

    /// Decode the exact canonical signed-order preimage.
    pub fn encoded_len() -> Result<usize> {
        validate_width(N)?;
        PORTFOLIO_ORDER_BASE_BYTES
            .checked_add(N.checked_mul(8).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Return the exact length of the message whose SHA-256 identity is
    /// persisted as `order_id`. The message deliberately excludes its own ID.
    pub fn signing_preimage_len() -> Result<usize> {
        validate_width(N)?;
        PORTFOLIO_ORDER_SIGNING_BASE_BYTES
            .checked_add(N.checked_mul(8).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Decode one exact-width canonical signed-order preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        exact_len(bytes, Self::encoded_len()?)?;
        if array::<8>(bytes, 0)? != ORDER_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != SCHEMA_V1 || read_u16(bytes, 10)? != ARTIFACT_PROFILE_V1 {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, 14, 2)?;
        let mut coefficients = [0i64; N];
        let mut index = 0usize;
        while index < N {
            let offset = 200usize
                .checked_add(index.checked_mul(8).ok_or(Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow)?;
            let target = coefficients.get_mut(index).ok_or(Error::InvalidLength)?;
            *target = read_i64(bytes, offset)?;
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Self::new(PortfolioOrderV1Input {
            outcome_count: read_u16(bytes, 12)?,
            market: array::<32>(bytes, 16)?,
            claim_basis_id: read_id(bytes, 48)?,
            owner: read_owner_key(bytes, 80)?,
            order_id: read_id(bytes, 112)?,
            generation: read_u64(bytes, 144)?,
            batch_sequence: read_u64(bytes, 152)?,
            nonce: read_u64(bytes, 160)?,
            valid_until_slot: read_u64(bytes, 168)?,
            max_lots: read_u64(bytes, 176)?,
            max_quote_debit_per_lot_numerator: read_i128(bytes, 184)?,
            coefficients,
        })
    }

    /// Encode into one exact-width caller-owned signed-order buffer.
    #[allow(clippy::needless_borrow)]
    pub fn encode(&self, mut out: &mut [u8]) -> Result<()> {
        exact_len(out, Self::encoded_len()?)?;
        out.fill(0);
        put(&mut out, 0, &ORDER_MAGIC);
        put(&mut out, 8, &SCHEMA_V1.to_le_bytes());
        put(&mut out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(&mut out, 12, &self.outcome_count.to_le_bytes());
        put(&mut out, 16, &self.market);
        put(&mut out, 48, self.claim_basis_id.as_bytes());
        put(&mut out, 80, self.owner.as_bytes());
        put(&mut out, 112, self.order_id.as_bytes());
        put(&mut out, 144, &self.generation.to_le_bytes());
        put(&mut out, 152, &self.batch_sequence.to_le_bytes());
        put(&mut out, 160, &self.nonce.to_le_bytes());
        put(&mut out, 168, &self.valid_until_slot.to_le_bytes());
        put(&mut out, 176, &self.max_lots.to_le_bytes());
        put(
            &mut out,
            184,
            &self.max_quote_debit_per_lot_numerator.to_le_bytes(),
        );
        for (index, coefficient) in self.coefficients.iter().enumerate() {
            if let Some(offset) = index
                .checked_mul(8)
                .and_then(|part| 200usize.checked_add(part))
            {
                put(&mut out, offset, &coefficient.to_le_bytes());
            }
        }
        Ok(())
    }

    /// Encode the sole noncircular signed message preimage.
    ///
    /// The adapter hashes these exact bytes and requires the digest to equal
    /// [`Self::order_id`], then compares [`Self::owner`] directly with the SVM
    /// signer. The full persisted/order instruction record may carry the
    /// derived ID, but that ID never appears inside its own hash preimage.
    #[allow(clippy::needless_borrow)]
    pub fn encode_signing_preimage(&self, mut out: &mut [u8]) -> Result<()> {
        exact_len(out, Self::signing_preimage_len()?)?;
        out.fill(0);
        put(&mut out, 0, &ORDER_SIGNING_MAGIC);
        put(&mut out, 8, &SCHEMA_V1.to_le_bytes());
        put(&mut out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(&mut out, 12, &self.outcome_count.to_le_bytes());
        put(&mut out, 16, &self.market);
        put(&mut out, 48, self.claim_basis_id.as_bytes());
        put(&mut out, 80, self.owner.as_bytes());
        put(&mut out, 112, &self.generation.to_le_bytes());
        put(&mut out, 120, &self.batch_sequence.to_le_bytes());
        put(&mut out, 128, &self.nonce.to_le_bytes());
        put(&mut out, 136, &self.valid_until_slot.to_le_bytes());
        put(&mut out, 144, &self.max_lots.to_le_bytes());
        put(
            &mut out,
            152,
            &self.max_quote_debit_per_lot_numerator.to_le_bytes(),
        );
        for (index, coefficient) in self.coefficients.iter().enumerate() {
            let offset = index
                .checked_mul(8)
                .and_then(|part| PORTFOLIO_ORDER_SIGNING_BASE_BYTES.checked_add(part))
                .ok_or(Error::ArithmeticOverflow)?;
            put(&mut out, offset, &coefficient.to_le_bytes());
        }
        Ok(())
    }

    /// Return the unique signed-order identity.
    pub const fn order_id(self) -> ContentId {
        self.order_id
    }

    /// Return the exact signing owner key.
    pub const fn owner(self) -> OwnerKeyV1 {
        self.owner
    }

    /// Return the immutable replay nonce.
    pub const fn nonce(self) -> u64 {
        self.nonce
    }

    /// Return the activated Market account key.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Return the exact ClaimBasis commitment.
    pub const fn claim_basis_id(self) -> ContentId {
        self.claim_basis_id
    }

    /// Return the immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the exact batch sequence.
    pub const fn batch_sequence(self) -> u64 {
        self.batch_sequence
    }

    /// Return the last slot through which this order remains executable.
    pub const fn valid_until_slot(self) -> u64 {
        self.valid_until_slot
    }

    /// Return the maximum scalar lots authorized by the signed order.
    pub const fn max_lots(self) -> u64 {
        self.max_lots
    }

    /// Return the maximum quote debit numerator authorized for one lot.
    pub const fn max_quote_debit_per_lot_numerator(self) -> i128 {
        self.max_quote_debit_per_lot_numerator
    }

    /// Return one atomic-lot coefficient vector.
    pub const fn coefficients(self) -> [i64; N] {
        self.coefficients
    }

    /// Compute the exact worst-case quote and native-claim custody required
    /// before this signed order may become live.
    pub fn worst_case_reserve(
        self,
        root: GeneralRootV1,
        config: GeneralConfigV1,
    ) -> Result<GeneralOrderReserveV1<N>> {
        root.validate_authority(self.market, self.claim_basis_id, self.generation, config)?;
        Ok(GeneralOrderReserveV1 {
            quote_atoms: quote_reserve(
                self.max_quote_debit_per_lot_numerator,
                self.max_lots,
                config.price_scale,
            )?,
            claim_atoms: claim_reserve(&self.coefficients, self.max_lots)?,
        })
    }
}

/// Exact present principal that must be locked before order admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOrderReserveV1<const N: usize> {
    quote_atoms: u64,
    claim_atoms: [u64; N],
}

impl<const N: usize> GeneralOrderReserveV1<N> {
    /// Return the ceil-rounded settlement-asset reserve.
    pub const fn quote_atoms(self) -> u64 {
        self.quote_atoms
    }

    /// Borrow exact per-outcome native-claim reserves.
    pub const fn claim_atoms(&self) -> &[u64; N] {
        &self.claim_atoms
    }
}

/// Mutable lifecycle of one adapter-authenticated signed order record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OrderPhase {
    /// At least one lot remains executable.
    Open = 0,
    /// The owner cancelled before the batch lock boundary.
    Cancelled = 1,
    /// Every lot was consumed by one or more settlement receipts.
    Consumed = 2,
    /// An unfilled remainder became unavailable when its batch quiesced.
    Released = 3,
}

/// Replay and partial-fill state for one unique signed order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderStateV1 {
    order_id: ContentId,
    owner: OwnerKeyV1,
    nonce: u64,
    remaining_lots: u64,
    phase: OrderPhase,
}

impl OrderStateV1 {
    /// Decode one exact-width canonical signed-order replay state.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        exact_len(bytes, ORDER_STATE_BYTES)?;
        if array::<8>(bytes, 0)? != ORDER_STATE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        validate_record_header(bytes)?;
        require_zero(bytes, 13, 3)?;
        let state = Self {
            phase: decode_order_phase(read_u8(bytes, 12)?)?,
            order_id: read_id(bytes, 16)?,
            owner: read_owner_key(bytes, 48)?,
            nonce: read_u64(bytes, 80)?,
            remaining_lots: read_u64(bytes, 88)?,
        };
        state.validate()?;
        Ok(state)
    }

    /// Encode one exact-width canonical signed-order replay state.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        exact_len(out, ORDER_STATE_BYTES)?;
        self.validate()?;
        out.fill(0);
        put(out, 0, &ORDER_STATE_MAGIC);
        put(out, 8, &SCHEMA_V1.to_le_bytes());
        put(out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(out, 12, &[order_phase_tag(self.phase)]);
        put(out, 16, self.order_id.as_bytes());
        put(out, 48, self.owner.as_bytes());
        put(out, 80, &self.nonce.to_le_bytes());
        put(out, 88, &self.remaining_lots.to_le_bytes());
        Ok(())
    }

    /// Authenticate this replay state against the immutable signed order that
    /// owns it, including the original lot ceiling.
    pub fn authenticate<const N: usize>(&self, order: PortfolioOrderV1<N>) -> Result<()> {
        self.validate()?;
        if self.order_id != order.order_id
            || self.owner != order.owner
            || self.nonce != order.nonce
            || self.remaining_lots > order.max_lots
        {
            return Err(Error::OrderBindingMismatch);
        }
        Ok(())
    }

    fn validate(self) -> Result<()> {
        match self.phase {
            OrderPhase::Open | OrderPhase::Cancelled | OrderPhase::Released
                if self.remaining_lots == 0 =>
            {
                Err(Error::NonCanonicalState)
            }
            OrderPhase::Consumed if self.remaining_lots != 0 => Err(Error::NonCanonicalState),
            _ => Ok(()),
        }
    }

    /// Open replay state after the adapter authenticates the signature and
    /// reserves the unique `(owner, nonce, order_id)` key.
    pub const fn open<const N: usize>(order: PortfolioOrderV1<N>) -> Self {
        Self {
            order_id: order.order_id,
            owner: order.owner,
            nonce: order.nonce,
            remaining_lots: order.max_lots,
            phase: OrderPhase::Open,
        }
    }

    /// Cancel before the batch's immutable collection close.
    pub fn cancel(
        &mut self,
        owner: OwnerKeyV1,
        now_slot: u64,
        collection_close: u64,
    ) -> Result<()> {
        if owner != self.owner {
            return Err(Error::AuthorityMismatch);
        }
        if self.phase != OrderPhase::Open {
            return Err(Error::OrderUnavailable);
        }
        if now_slot >= collection_close {
            return Err(Error::OutsideWindow);
        }
        self.phase = OrderPhase::Cancelled;
        Ok(())
    }

    /// Make an unfilled remainder unavailable after its immutable batch has
    /// become quiescent. This is not owner cancellation and cannot run while
    /// candidate selection or application remains possible.
    pub fn release_after_batch<const N: usize>(
        &mut self,
        order: PortfolioOrderV1<N>,
        batch: BatchRootV1,
    ) -> Result<()> {
        self.authenticate(order)?;
        if self.phase != OrderPhase::Open
            || batch.phase != BatchPhase::Quiescent
            || batch.sequence != order.batch_sequence
        {
            return Err(Error::InvalidPhase);
        }
        self.phase = OrderPhase::Released;
        Ok(())
    }

    fn validate_snapshot<const N: usize>(
        self,
        order: PortfolioOrderV1<N>,
        fill_lots: u64,
    ) -> Result<()> {
        self.authenticate(order)?;
        if self.phase != OrderPhase::Open {
            return Err(Error::OrderUnavailable);
        }
        if fill_lots == 0 || fill_lots > self.remaining_lots {
            return Err(Error::InvalidFill);
        }
        Ok(())
    }

    fn consume<const N: usize>(
        &mut self,
        order: PortfolioOrderV1<N>,
        fill_lots: u64,
    ) -> Result<()> {
        self.validate_snapshot(order, fill_lots)?;
        self.remaining_lots = self
            .remaining_lots
            .checked_sub(fill_lots)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.remaining_lots == 0 {
            self.phase = OrderPhase::Consumed;
        }
        Ok(())
    }

    /// Return the exact remaining atomic lot count.
    pub const fn remaining_lots(self) -> u64 {
        self.remaining_lots
    }

    /// Return the replay lifecycle phase.
    pub const fn phase(self) -> OrderPhase {
        self.phase
    }

    /// Return the signed-order commitment.
    pub const fn order_id(self) -> ContentId {
        self.order_id
    }
}

/// Exact-N semantic custody for one admitted General order.
///
/// Reserved native claims are owned directly by this record after the adapter
/// debits the signed owner's Position. Reserved quote atoms are physically
/// held by the bound token escrow. Replay availability remains owned solely by
/// [`OrderStateV1`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOrderCustodyV1<const N: usize> {
    order_id: ContentId,
    market: [u8; 32],
    owner: OwnerKeyV1,
    rent_beneficiary: [u8; 32],
    quote_escrow: [u8; 32],
    generation: u64,
    reserved_quote_atoms: u64,
    reserved_claim_atoms: [u64; N],
    outcome_count: u16,
}

/// Atomic semantic effects of admitting one signed order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOrderAdmissionV1<const N: usize> {
    /// Newly opened sole replay state.
    pub order_state: OrderStateV1,
    /// Newly funded exact-N order custody.
    pub custody: GeneralOrderCustodyV1<N>,
    /// Exact quote and Position debits the adapter must commit atomically.
    pub reserve: GeneralOrderReserveV1<N>,
}

/// Exact physical effects of applying one authenticated settlement receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralCustodyConsumptionV1<const N: usize> {
    quote_debit_from_escrow: u64,
    quote_credit_to_owner: u64,
    claim_debits_from_custody: [u64; N],
    claim_credits_to_owner: [u64; N],
}

impl<const N: usize> GeneralCustodyConsumptionV1<N> {
    /// Return settlement atoms consumed from the order's quote escrow.
    pub const fn quote_debit_from_escrow(self) -> u64 {
        self.quote_debit_from_escrow
    }

    /// Return settlement atoms credited to the signed owner.
    pub const fn quote_credit_to_owner(self) -> u64 {
        self.quote_credit_to_owner
    }

    /// Borrow exact native claims consumed from semantic custody.
    pub const fn claim_debits_from_custody(&self) -> &[u64; N] {
        &self.claim_debits_from_custody
    }

    /// Borrow exact native claims credited to the signed owner's Position.
    pub const fn claim_credits_to_owner(&self) -> &[u64; N] {
        &self.claim_credits_to_owner
    }
}

/// Exact residual principal and rent authority emitted when custody closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralCustodyReleaseV1<const N: usize> {
    /// Signed owner receiving every residual reserve.
    pub owner: OwnerKeyV1,
    /// Persisted account receiving custody-record rent.
    pub rent_beneficiary: [u8; 32],
    /// Bound token escrow returning residual settlement atoms and its rent.
    pub quote_escrow: [u8; 32],
    /// Exact residual settlement atoms returned to the owner.
    pub quote_atoms: u64,
    /// Exact residual native claims returned to the owner's Position.
    pub claim_atoms: [u64; N],
}

impl<const N: usize> GeneralOrderCustodyV1<N> {
    /// Return the exact checked encoded length, with no maximum-width padding.
    pub fn encoded_len() -> Result<usize> {
        validate_width(N)?;
        GENERAL_ORDER_CUSTODY_BASE_BYTES
            .checked_add(N.checked_mul(8).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Atomically plan replay reservation and worst-case custody admission.
    pub fn admit(
        order: PortfolioOrderV1<N>,
        root: GeneralRootV1,
        config: GeneralConfigV1,
        rent_beneficiary: [u8; 32],
        quote_escrow: [u8; 32],
    ) -> Result<GeneralOrderAdmissionV1<N>> {
        require_nonzero_key(&rent_beneficiary)?;
        require_nonzero_key(&quote_escrow)?;
        let reserve = order.worst_case_reserve(root, config)?;
        let custody = Self {
            order_id: order.order_id,
            market: order.market,
            owner: order.owner,
            rent_beneficiary,
            quote_escrow,
            generation: order.generation,
            reserved_quote_atoms: reserve.quote_atoms,
            reserved_claim_atoms: reserve.claim_atoms,
            outcome_count: order.outcome_count,
        };
        Ok(GeneralOrderAdmissionV1 {
            order_state: OrderStateV1::open(order),
            custody,
            reserve,
        })
    }

    /// Decode one exact-width canonical order-custody record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        exact_len(bytes, Self::encoded_len()?)?;
        if array::<8>(bytes, 0)? != ORDER_CUSTODY_MAGIC {
            return Err(Error::InvalidMagic);
        }
        validate_record_header(bytes)?;
        require_zero(bytes, 14, 2)?;
        let outcome_count = read_u16(bytes, 12)?;
        if usize::from(outcome_count) != N {
            return Err(Error::InvalidOutcomeCount);
        }
        let rent_beneficiary = array::<32>(bytes, 112)?;
        let quote_escrow = array::<32>(bytes, 144)?;
        require_nonzero_key(&rent_beneficiary)?;
        require_nonzero_key(&quote_escrow)?;
        let mut reserved_claim_atoms = [0u64; N];
        for (index, target) in reserved_claim_atoms.iter_mut().enumerate() {
            let offset = index
                .checked_mul(8)
                .and_then(|part| GENERAL_ORDER_CUSTODY_BASE_BYTES.checked_add(part))
                .ok_or(Error::ArithmeticOverflow)?;
            *target = read_u64(bytes, offset)?;
        }
        Ok(Self {
            order_id: read_id(bytes, 16)?,
            market: array::<32>(bytes, 48)?,
            owner: read_owner_key(bytes, 80)?,
            rent_beneficiary,
            quote_escrow,
            generation: read_u64(bytes, 176)?,
            reserved_quote_atoms: read_u64(bytes, 184)?,
            reserved_claim_atoms,
            outcome_count,
        })
    }

    /// Encode one exact-width canonical order-custody record.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        exact_len(out, Self::encoded_len()?)?;
        self.validate_shape()?;
        out.fill(0);
        put(out, 0, &ORDER_CUSTODY_MAGIC);
        put(out, 8, &SCHEMA_V1.to_le_bytes());
        put(out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(out, 12, &self.outcome_count.to_le_bytes());
        put(out, 16, self.order_id.as_bytes());
        put(out, 48, &self.market);
        put(out, 80, self.owner.as_bytes());
        put(out, 112, &self.rent_beneficiary);
        put(out, 144, &self.quote_escrow);
        put(out, 176, &self.generation.to_le_bytes());
        put(out, 184, &self.reserved_quote_atoms.to_le_bytes());
        for (index, amount) in self.reserved_claim_atoms.iter().enumerate() {
            let offset = index
                .checked_mul(8)
                .and_then(|part| GENERAL_ORDER_CUSTODY_BASE_BYTES.checked_add(part))
                .ok_or(Error::ArithmeticOverflow)?;
            put(out, offset, &amount.to_le_bytes());
        }
        Ok(())
    }

    /// Authenticate immutable bindings and prove reserves never exceed the
    /// signed order's admission ceiling.
    pub fn authenticate(
        &self,
        order: PortfolioOrderV1<N>,
        root: GeneralRootV1,
        config: GeneralConfigV1,
    ) -> Result<()> {
        self.validate_shape()?;
        if self.order_id != order.order_id
            || self.market != order.market
            || self.owner != order.owner
            || self.generation != order.generation
            || self.outcome_count != order.outcome_count
        {
            return Err(Error::CustodyMismatch);
        }
        let ceiling = order.worst_case_reserve(root, config)?;
        if self.reserved_quote_atoms > ceiling.quote_atoms
            || self
                .reserved_claim_atoms
                .iter()
                .zip(ceiling.claim_atoms.iter())
                .any(|(remaining, initial)| remaining > initial)
        {
            return Err(Error::CustodyMismatch);
        }
        Ok(())
    }

    /// Consume one authenticated receipt and advance replay plus custody
    /// atomically. Any refusal leaves both inputs unchanged.
    pub fn apply_receipt(
        &mut self,
        state: &mut OrderStateV1,
        order: PortfolioOrderV1<N>,
        receipt: SettlementReceiptV1<N>,
        root: GeneralRootV1,
        config: GeneralConfigV1,
    ) -> Result<GeneralCustodyConsumptionV1<N>> {
        self.authenticate(order, root, config)?;
        state.authenticate(order)?;
        if receipt.order_id != order.order_id
            || receipt.owner != order.owner
            || receipt.generation != order.generation
            || receipt.batch_sequence != order.batch_sequence
            || receipt.nonce != order.nonce
            || receipt.outcome_count != config.outcome_count
            || receipt.fill_lots == 0
            || receipt.carry_before >= config.price_scale
            || receipt.carry_after >= config.price_scale
        {
            return Err(Error::CustodyMismatch);
        }

        let mut next_state = *state;
        next_state.consume(order, receipt.fill_lots)?;
        if receipt.remaining_lots != next_state.remaining_lots {
            return Err(Error::CustodyMismatch);
        }
        let mut next_custody = *self;
        let (quote_debit_from_escrow, quote_credit_to_owner) =
            split_signed_amount(receipt.quote_delta_atoms)?;
        next_custody.reserved_quote_atoms = next_custody
            .reserved_quote_atoms
            .checked_sub(quote_debit_from_escrow)
            .ok_or(Error::InsufficientCustody)?;

        let mut claim_debits_from_custody = [0u64; N];
        let mut claim_credits_to_owner = [0u64; N];
        for (index, ((coefficient, receipt_delta), reserve)) in order
            .coefficients
            .iter()
            .zip(receipt.outcome_deltas.iter())
            .zip(next_custody.reserved_claim_atoms.iter_mut())
            .enumerate()
        {
            let expected = i128::from(*coefficient)
                .checked_mul(i128::from(receipt.fill_lots))
                .ok_or(Error::ArithmeticOverflow)?;
            let expected = i64::try_from(expected).map_err(|_| Error::TokenAmountOutOfRange)?;
            if *receipt_delta != expected {
                return Err(Error::CustodyMismatch);
            }
            let (debit, credit) = split_signed_amount(*receipt_delta)?;
            *reserve = reserve
                .checked_sub(debit)
                .ok_or(Error::InsufficientCustody)?;
            let debit_target = claim_debits_from_custody
                .get_mut(index)
                .ok_or(Error::ArithmeticOverflow)?;
            *debit_target = debit;
            let credit_target = claim_credits_to_owner
                .get_mut(index)
                .ok_or(Error::ArithmeticOverflow)?;
            *credit_target = credit;
        }

        *state = next_state;
        *self = next_custody;
        Ok(GeneralCustodyConsumptionV1 {
            quote_debit_from_escrow,
            quote_credit_to_owner,
            claim_debits_from_custody,
            claim_credits_to_owner,
        })
    }

    /// Cancel before collection lock and emit the complete residual release in
    /// the same semantic transition.
    #[allow(clippy::too_many_arguments)]
    pub fn cancel_and_release(
        self,
        state: &mut OrderStateV1,
        order: PortfolioOrderV1<N>,
        owner: OwnerKeyV1,
        now_slot: u64,
        collection_close: u64,
        root: GeneralRootV1,
        config: GeneralConfigV1,
    ) -> Result<GeneralCustodyReleaseV1<N>> {
        self.authenticate(order, root, config)?;
        let mut next_state = *state;
        next_state.authenticate(order)?;
        next_state.cancel(owner, now_slot, collection_close)?;
        let release = self.release(next_state)?;
        *state = next_state;
        Ok(release)
    }

    /// Close after batch convergence, releasing any unfilled or limit-price
    /// surplus. A partially filled replay state becomes permanently Released.
    pub fn close_after_batch(
        self,
        state: &mut OrderStateV1,
        order: PortfolioOrderV1<N>,
        batch: BatchRootV1,
        root: GeneralRootV1,
        config: GeneralConfigV1,
    ) -> Result<GeneralCustodyReleaseV1<N>> {
        self.authenticate(order, root, config)?;
        if batch.phase != BatchPhase::Quiescent || batch.sequence != order.batch_sequence {
            return Err(Error::InvalidPhase);
        }
        let mut next_state = *state;
        next_state.authenticate(order)?;
        if next_state.phase == OrderPhase::Open {
            next_state.release_after_batch(order, batch)?;
        }
        let release = self.release(next_state)?;
        *state = next_state;
        Ok(release)
    }

    /// Return the signed owner key.
    pub const fn owner(self) -> OwnerKeyV1 {
        self.owner
    }

    /// Return the persisted rent beneficiary account key.
    pub const fn rent_beneficiary(self) -> [u8; 32] {
        self.rent_beneficiary
    }

    /// Return the bound quote-escrow account key.
    pub const fn quote_escrow(self) -> [u8; 32] {
        self.quote_escrow
    }

    /// Return exact settlement atoms still locked.
    pub const fn reserved_quote_atoms(self) -> u64 {
        self.reserved_quote_atoms
    }

    /// Borrow exact native claims still semantically locked.
    pub const fn reserved_claim_atoms(&self) -> &[u64; N] {
        &self.reserved_claim_atoms
    }

    fn validate_shape(&self) -> Result<()> {
        if usize::from(self.outcome_count) != N {
            return Err(Error::InvalidOutcomeCount);
        }
        require_nonzero_key(&self.rent_beneficiary)?;
        require_nonzero_key(&self.quote_escrow)
    }

    fn release(self, state: OrderStateV1) -> Result<GeneralCustodyReleaseV1<N>> {
        if !matches!(
            state.phase,
            OrderPhase::Cancelled | OrderPhase::Consumed | OrderPhase::Released
        ) {
            return Err(Error::CustodyNotReleasable);
        }
        Ok(GeneralCustodyReleaseV1 {
            owner: self.owner,
            rent_beneficiary: self.rent_beneficiary,
            quote_escrow: self.quote_escrow,
            quote_atoms: self.reserved_quote_atoms,
            claim_atoms: self.reserved_claim_atoms,
        })
    }
}

/// One atomic scalar fill presented in a verification or settlement page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionV1<const N: usize> {
    /// Full signed atomic portfolio preimage.
    pub order: PortfolioOrderV1<N>,
    /// Adapter-authenticated replay state snapshot.
    pub order_state: OrderStateV1,
    /// One scalar applied uniformly to every coefficient.
    pub fill_lots: u64,
}

/// Permissionless candidate submission preimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateSubmissionV1<const N: usize> {
    /// Activated provider-neutral Market account key.
    pub market: [u8; 32],
    /// Exact ClaimBasis identity.
    pub claim_basis_id: ContentId,
    /// Exact signer and permanent rent beneficiary creating this candidate.
    pub submitter: OwnerKeyV1,
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact batch sequence.
    pub batch_sequence: u64,
    /// Last slot at which verification may finish.
    pub valid_until_slot: u64,
    /// Claimed execution count recomputed by the verifier.
    pub claimed_execution_count: u32,
    /// Claimed immutable page count recomputed by linked traversal.
    pub claimed_page_count: u32,
    /// Claimed preference-surplus score recomputed by the verifier.
    pub claimed_score: u128,
    /// First immutable page content ID in the reverse-built linked artifact.
    pub first_page_id: ContentId,
    /// Exact aggregate Rent required to create every candidate-exclusive page copy.
    pub page_rent_reserve_lamports: u64,
    /// Exact Rent for cursor Position and quote-escrow temporary settlement accounts.
    pub settlement_rent_reserve_lamports: u64,
    /// Claimed exact aggregate quote debit numerator.
    pub claimed_total_quote_debit_numerator: i128,
    /// Exact scaled-integer simplex coordinates.
    pub prices: [u64; N],
    /// Claimed exact outcome deltas in canonical ClaimBasis order.
    pub claimed_net_coefficients: [i128; N],
    /// Exact ClaimBasis width, which must equal the selected const width.
    pub outcome_count: u16,
}

impl<const N: usize> CandidateSubmissionV1<N> {
    /// Return the exact checked encoded length, with no maximum-width padding.
    pub fn encoded_len() -> Result<usize> {
        validate_width(N)?;
        CANDIDATE_SUBMISSION_BASE_BYTES
            .checked_add(N.checked_mul(24).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Decode one canonical candidate-submission preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        exact_len(bytes, Self::encoded_len()?)?;
        if array::<8>(bytes, 0)? != CANDIDATE_SUBMISSION_MAGIC {
            return Err(Error::InvalidMagic);
        }
        validate_record_header(bytes)?;
        require_zero(bytes, 14, 2)?;
        let outcome_count = read_u16(bytes, 12)?;
        if usize::from(outcome_count) != N {
            return Err(Error::InvalidOutcomeCount);
        }
        let mut prices = [0u64; N];
        let mut claimed_net_coefficients = [0i128; N];
        let claimed_net_offset = vector_offset(CANDIDATE_SUBMISSION_BASE_BYTES, N, 8)?;
        for (index, (price, net)) in prices
            .iter_mut()
            .zip(claimed_net_coefficients.iter_mut())
            .enumerate()
        {
            let offset = vector_offset(CANDIDATE_SUBMISSION_BASE_BYTES, index, 8)?;
            *price = read_u64(bytes, offset)?;
            *net = read_i128(bytes, vector_offset(claimed_net_offset, index, 16)?)?;
        }
        Ok(Self {
            market: array::<32>(bytes, 16)?,
            claim_basis_id: read_id(bytes, 48)?,
            submitter: read_owner_key(bytes, 80)?,
            generation: read_u64(bytes, 112)?,
            batch_sequence: read_u64(bytes, 120)?,
            valid_until_slot: read_u64(bytes, 128)?,
            claimed_execution_count: read_u32(bytes, 136)?,
            claimed_page_count: read_u32(bytes, 140)?,
            claimed_score: read_u128(bytes, 144)?,
            page_rent_reserve_lamports: read_u64(bytes, 160)?,
            first_page_id: read_id(bytes, 168)?,
            claimed_total_quote_debit_numerator: read_i128(bytes, 200)?,
            settlement_rent_reserve_lamports: read_u64(bytes, 216)?,
            prices,
            claimed_net_coefficients,
            outcome_count,
        })
    }

    /// Encode one canonical candidate-submission preimage.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        exact_len(out, Self::encoded_len()?)?;
        if usize::from(self.outcome_count) != N
            || self.claimed_execution_count == 0
            || self.claimed_page_count == 0
            || self.page_rent_reserve_lamports == 0
            || self.settlement_rent_reserve_lamports == 0
        {
            return Err(Error::CandidateClaimMismatch);
        }
        out.fill(0);
        put(out, 0, &CANDIDATE_SUBMISSION_MAGIC);
        put(out, 8, &SCHEMA_V1.to_le_bytes());
        put(out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(out, 12, &self.outcome_count.to_le_bytes());
        put(out, 16, &self.market);
        put(out, 48, self.claim_basis_id.as_bytes());
        put(out, 80, self.submitter.as_bytes());
        put(out, 112, &self.generation.to_le_bytes());
        put(out, 120, &self.batch_sequence.to_le_bytes());
        put(out, 128, &self.valid_until_slot.to_le_bytes());
        put(out, 136, &self.claimed_execution_count.to_le_bytes());
        put(out, 140, &self.claimed_page_count.to_le_bytes());
        put(out, 144, &self.claimed_score.to_le_bytes());
        put(out, 160, &self.page_rent_reserve_lamports.to_le_bytes());
        put(out, 168, self.first_page_id.as_bytes());
        put(
            out,
            200,
            &self.claimed_total_quote_debit_numerator.to_le_bytes(),
        );
        put(
            out,
            216,
            &self.settlement_rent_reserve_lamports.to_le_bytes(),
        );
        let claimed_net_offset = vector_offset(CANDIDATE_SUBMISSION_BASE_BYTES, N, 8)?;
        for (index, (price, net)) in self
            .prices
            .iter()
            .zip(self.claimed_net_coefficients.iter())
            .enumerate()
        {
            put(
                out,
                vector_offset(CANDIDATE_SUBMISSION_BASE_BYTES, index, 8)?,
                &price.to_le_bytes(),
            );
            put(
                out,
                vector_offset(claimed_net_offset, index, 16)?,
                &net.to_le_bytes(),
            );
        }
        Ok(())
    }
}

/// Verification lifecycle of a candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CandidatePhase {
    /// No verification page has been accepted.
    Submitted = 0,
    /// At least one page has committed progress.
    Verifying = 1,
    /// Every execution and conservation check succeeded.
    Valid = 2,
    /// The batch consumed this candidate's one consideration right.
    Considered = 3,
    /// The candidate cannot be resumed under this immutable identity.
    Rejected = 4,
}

/// Fixed-layout candidate state and committed resumable verifier cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateStateV1<const N: usize> {
    candidate_id: ContentId,
    submission: CandidateSubmissionV1<N>,
    phase: CandidatePhase,
    verified_pages: u32,
    verified_executions: u32,
    last_order_id: Option<ContentId>,
    next_page_id: Option<ContentId>,
    open_page_children: u32,
    page_rent_reserve_remaining: u64,
    settlement_rent_reserve_remaining: u64,
    verification_work_remaining: u64,
    settlement_work_remaining: u64,
    cleanup_work_remaining: u64,
    net_coefficients: [i128; N],
    total_quote_debit_numerator: i128,
    score: u128,
}

/// Adapter-observed capitalization of one candidate state account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateCapitalizationV1 {
    /// Current physical lamports on the candidate state account.
    pub account_lamports: u64,
    /// Current Rent minimum for the exact candidate state width.
    pub exact_state_rent_lamports: u64,
}

/// Atomic creation plan for one immutable candidate-exclusive page record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidatePageCreationV1 {
    page_rent_lamports: u64,
    page_top_up_lamports: u64,
    candidate_refund_lamports: u64,
    page_surplus_refund_lamports: u64,
}

/// Permissionless close plan for one immutable candidate page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidatePageCloseV1 {
    /// Exact cleanup reward paid from the candidate state account.
    pub cleanup_reward_lamports: u64,
    /// Exact page-account lamports returned to the immutable RentCredit.
    pub rent_credit_lamports: u64,
    /// Immutable RentCredit beneficiary.
    pub rent_beneficiary: OwnerKeyV1,
}

/// Permissionless terminal close plan for one candidate state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateCloseV1 {
    /// Exact final cleanup reward paid to the closer.
    pub cleanup_reward_lamports: u64,
    /// Candidate-account Rent and unused work returned to RentCredit.
    pub rent_credit_lamports: u64,
    /// Immutable RentCredit beneficiary.
    pub rent_beneficiary: OwnerKeyV1,
}

/// Trusted Rent minima and safe precreation balances for settlement temporaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementRentObservationV1 {
    /// Exact Rent for `[cursor, Position, quote escrow]`.
    pub exact_rent_lamports: [u64; 3],
    /// System-owned, nonexecutable, data-empty precreation balances in the same order.
    pub precreation_lamports: [u64; 3],
}

/// Physical lamports observed on finished settlement temporary accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementCloseObservationV1 {
    /// Current balances for `[cursor, Position, quote escrow]`.
    pub account_lamports: [u64; 3],
    /// Current Rent minima for those exact accounts.
    pub exact_rent_lamports: [u64; 3],
}

/// Atomic close plan for the empty settlement temporary cluster.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementCloseV1 {
    /// Final selected-settlement reward paid to the permissionless closer.
    pub continuation_reward_lamports: u64,
    /// All temporary-account lamports returned to immutable RentCredit.
    pub rent_credit_lamports: u64,
    /// Immutable candidate RentCredit beneficiary.
    pub rent_beneficiary: OwnerKeyV1,
}

impl CandidatePageCreationV1 {
    /// Return the exact Rent minimum consumed from candidate rent principal.
    pub const fn page_rent_lamports(self) -> u64 {
        self.page_rent_lamports
    }

    /// Return the exact candidate-PDA transfer into the page account.
    pub const fn page_top_up_lamports(self) -> u64 {
        self.page_top_up_lamports
    }

    /// Return unused candidate reserve displaced by safe precreation dust.
    pub const fn candidate_refund_lamports(self) -> u64 {
        self.candidate_refund_lamports
    }

    /// Return precreation surplus removed from the allocated page account.
    pub const fn page_surplus_refund_lamports(self) -> u64 {
        self.page_surplus_refund_lamports
    }
}

/// One immutable bounded candidate page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidatePageV1<const N: usize> {
    /// Zero-based page index committed by linked traversal.
    pub page_index: u32,
    /// Content ID of the next page, or the canonical terminal sentinel.
    pub next_page_id: Option<ContentId>,
    /// Number of leading executions used in the fixed envelope.
    pub execution_count: u8,
    /// Fixed V1 execution envelope.
    pub executions: [Option<ExecutionV1<N>>; MAX_EXECUTIONS_PER_PAGE_V1],
}

impl<const N: usize> CandidatePageV1<N> {
    /// Return the exact wire width for this page's leading execution count.
    pub fn encoded_len(execution_count: u8) -> Result<usize> {
        validate_width(N)?;
        let count = usize::from(execution_count);
        if count == 0 || count > MAX_EXECUTIONS_PER_PAGE_V1 {
            return Err(Error::InvalidPageCount);
        }
        let execution_bytes = PortfolioOrderV1::<N>::encoded_len()?
            .checked_add(ORDER_STATE_BYTES)
            .and_then(|value| value.checked_add(8))
            .ok_or(Error::ArithmeticOverflow)?;
        CANDIDATE_PAGE_BASE_BYTES
            .checked_add(
                count
                    .checked_mul(execution_bytes)
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Decode one exact leading-execution page with no unused wire padding.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < CANDIDATE_PAGE_BASE_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != CANDIDATE_PAGE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        validate_record_header(bytes)?;
        if usize::from(read_u16(bytes, 12)?) != N {
            return Err(Error::InvalidOutcomeCount);
        }
        require_zero(bytes, 20, 4)?;
        let execution_count = read_u8(bytes, 14)?;
        exact_len(bytes, Self::encoded_len(execution_count)?)?;
        let mut executions = [None; MAX_EXECUTIONS_PER_PAGE_V1];
        let order_bytes = PortfolioOrderV1::<N>::encoded_len()?;
        let execution_bytes = order_bytes
            .checked_add(ORDER_STATE_BYTES)
            .and_then(|value| value.checked_add(8))
            .ok_or(Error::ArithmeticOverflow)?;
        for index in 0..usize::from(execution_count) {
            let offset = vector_offset(CANDIDATE_PAGE_BASE_BYTES, index, execution_bytes)?;
            let order_end = offset
                .checked_add(order_bytes)
                .ok_or(Error::ArithmeticOverflow)?;
            let state_end = order_end
                .checked_add(ORDER_STATE_BYTES)
                .ok_or(Error::ArithmeticOverflow)?;
            let execution = ExecutionV1 {
                order: PortfolioOrderV1::decode(subslice(bytes, offset, order_bytes)?)?,
                order_state: OrderStateV1::decode(subslice(bytes, order_end, ORDER_STATE_BYTES)?)?,
                fill_lots: read_u64(bytes, state_end)?,
            };
            *executions.get_mut(index).ok_or(Error::InvalidPageCount)? = Some(execution);
        }
        Ok(Self {
            page_index: read_u32(bytes, 16)?,
            next_page_id: decode_optional_id(bytes, 15, 24)?,
            execution_count,
            executions,
        })
    }

    /// Encode one exact leading-execution page with no unused wire padding.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        exact_len(out, Self::encoded_len(self.execution_count)?)?;
        let count = usize::from(self.execution_count);
        if self.executions.iter().take(count).any(Option::is_none)
            || self.executions.iter().skip(count).any(Option::is_some)
        {
            return Err(Error::NonCanonicalReservedBytes);
        }
        out.fill(0);
        put(out, 0, &CANDIDATE_PAGE_MAGIC);
        put(out, 8, &SCHEMA_V1.to_le_bytes());
        put(out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(
            out,
            12,
            &u16::try_from(N)
                .map_err(|_| Error::InvalidOutcomeCount)?
                .to_le_bytes(),
        );
        put(out, 14, &[self.execution_count]);
        put(out, 16, &self.page_index.to_le_bytes());
        if let Some(next_page_id) = self.next_page_id {
            put(out, 15, &[1]);
            put(out, 24, next_page_id.as_bytes());
        }
        let order_bytes = PortfolioOrderV1::<N>::encoded_len()?;
        let execution_bytes = order_bytes
            .checked_add(ORDER_STATE_BYTES)
            .and_then(|value| value.checked_add(8))
            .ok_or(Error::ArithmeticOverflow)?;
        for (index, execution) in self.executions.iter().take(count).flatten().enumerate() {
            let offset = vector_offset(CANDIDATE_PAGE_BASE_BYTES, index, execution_bytes)?;
            let order_end = offset
                .checked_add(order_bytes)
                .ok_or(Error::ArithmeticOverflow)?;
            let state_end = order_end
                .checked_add(ORDER_STATE_BYTES)
                .ok_or(Error::ArithmeticOverflow)?;
            execution
                .order
                .encode(subslice_mut(out, offset, order_bytes)?)?;
            execution
                .order_state
                .encode(subslice_mut(out, order_end, ORDER_STATE_BYTES)?)?;
            put(out, state_end, &execution.fill_lots.to_le_bytes());
        }
        Ok(())
    }
}

impl<const N: usize> CandidateStateV1<N> {
    /// Return the exact persisted width for this ClaimBasis width.
    pub fn encoded_len() -> Result<usize> {
        validate_width(N)?;
        CANDIDATE_STATE_BASE_BYTES
            .checked_add(N.checked_mul(40).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Decode one canonical candidate and resumable verification cursor.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        exact_len(bytes, Self::encoded_len()?)?;
        if array::<8>(bytes, 0)? != CANDIDATE_STATE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        validate_record_header(bytes)?;
        require_zero(bytes, 12, 4)?;
        let submission_bytes = CandidateSubmissionV1::<N>::encoded_len()?;
        let submission = CandidateSubmissionV1::decode(subslice(bytes, 48, submission_bytes)?)?;
        let cursor = 48usize
            .checked_add(submission_bytes)
            .ok_or(Error::ArithmeticOverflow)?;
        require_zero(bytes, cursor + 2, 6)?;
        require_zero(bytes, cursor + 17, 7)?;
        let last_bytes = array::<32>(bytes, cursor + 24)?;
        let last_order_id = match read_u8(bytes, cursor + 16)? {
            0 => {
                if last_bytes.iter().any(|byte| *byte != 0) {
                    return Err(Error::NonCanonicalReservedBytes);
                }
                None
            }
            1 => Some(ContentId::new(last_bytes)?),
            _ => return Err(Error::NonCanonicalState),
        };
        let next_page_id = decode_optional_id(bytes, cursor + 1, cursor + 56)?;
        require_zero(bytes, cursor + 92, 4)?;
        let net_offset = cursor.checked_add(168).ok_or(Error::ArithmeticOverflow)?;
        let mut net_coefficients = [0i128; N];
        for (index, target) in net_coefficients.iter_mut().enumerate() {
            *target = read_i128(bytes, vector_offset(net_offset, index, 16)?)?;
        }
        let state = Self {
            candidate_id: read_id(bytes, 16)?,
            submission,
            phase: decode_candidate_phase(read_u8(bytes, cursor)?)?,
            verified_pages: read_u32(bytes, cursor + 8)?,
            verified_executions: read_u32(bytes, cursor + 12)?,
            last_order_id,
            next_page_id,
            open_page_children: read_u32(bytes, cursor + 88)?,
            page_rent_reserve_remaining: read_u64(bytes, cursor + 96)?,
            settlement_rent_reserve_remaining: read_u64(bytes, cursor + 104)?,
            verification_work_remaining: read_u64(bytes, cursor + 112)?,
            settlement_work_remaining: read_u64(bytes, cursor + 120)?,
            cleanup_work_remaining: read_u64(bytes, cursor + 128)?,
            net_coefficients,
            total_quote_debit_numerator: read_i128(bytes, cursor + 136)?,
            score: read_u128(bytes, cursor + 152)?,
        };
        state.validate_persisted_shape()?;
        Ok(state)
    }

    /// Encode one canonical candidate and resumable verification cursor.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        exact_len(out, Self::encoded_len()?)?;
        self.validate_persisted_shape()?;
        out.fill(0);
        put(out, 0, &CANDIDATE_STATE_MAGIC);
        put(out, 8, &SCHEMA_V1.to_le_bytes());
        put(out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(out, 16, self.candidate_id.as_bytes());
        let submission_bytes = CandidateSubmissionV1::<N>::encoded_len()?;
        self.submission
            .encode(subslice_mut(out, 48, submission_bytes)?)?;
        let cursor = 48usize
            .checked_add(submission_bytes)
            .ok_or(Error::ArithmeticOverflow)?;
        put(out, cursor, &[candidate_phase_tag(self.phase)]);
        put(out, cursor + 8, &self.verified_pages.to_le_bytes());
        put(out, cursor + 12, &self.verified_executions.to_le_bytes());
        if let Some(next_page_id) = self.next_page_id {
            put(out, cursor + 1, &[1]);
            put(out, cursor + 56, next_page_id.as_bytes());
        }
        if let Some(last_order_id) = self.last_order_id {
            put(out, cursor + 16, &[1]);
            put(out, cursor + 24, last_order_id.as_bytes());
        }
        put(out, cursor + 88, &self.open_page_children.to_le_bytes());
        put(
            out,
            cursor + 96,
            &self.page_rent_reserve_remaining.to_le_bytes(),
        );
        put(
            out,
            cursor + 104,
            &self.settlement_rent_reserve_remaining.to_le_bytes(),
        );
        put(
            out,
            cursor + 112,
            &self.verification_work_remaining.to_le_bytes(),
        );
        put(
            out,
            cursor + 120,
            &self.settlement_work_remaining.to_le_bytes(),
        );
        put(
            out,
            cursor + 128,
            &self.cleanup_work_remaining.to_le_bytes(),
        );
        put(
            out,
            cursor + 136,
            &self.total_quote_debit_numerator.to_le_bytes(),
        );
        put(out, cursor + 152, &self.score.to_le_bytes());
        let net_offset = cursor.checked_add(168).ok_or(Error::ArithmeticOverflow)?;
        for (index, coefficient) in self.net_coefficients.iter().enumerate() {
            put(
                out,
                vector_offset(net_offset, index, 16)?,
                &coefficient.to_le_bytes(),
            );
        }
        Ok(())
    }

    fn validate_persisted_shape(&self) -> Result<()> {
        if usize::from(self.submission.outcome_count) != N
            || self.submission.claimed_execution_count == 0
            || self.submission.claimed_page_count == 0
            || self.open_page_children > self.submission.claimed_page_count
            || self.verified_pages > self.submission.claimed_page_count
        {
            return Err(Error::NonCanonicalState);
        }
        let pristine = self.verified_pages == 0
            && self.verified_executions == 0
            && self.last_order_id.is_none()
            && self.next_page_id == Some(self.submission.first_page_id)
            && self.net_coefficients.iter().all(|value| *value == 0)
            && self.total_quote_debit_numerator == 0
            && self.score == 0;
        match self.phase {
            CandidatePhase::Submitted if !pristine => Err(Error::NonCanonicalState),
            CandidatePhase::Verifying
                if self.verified_pages == 0
                    || self.verified_executions == 0
                    || self.last_order_id.is_none() =>
            {
                Err(Error::NonCanonicalState)
            }
            CandidatePhase::Valid | CandidatePhase::Considered
                if self.verified_pages != self.submission.claimed_page_count
                    || self.verified_executions != self.submission.claimed_execution_count
                    || self.last_order_id.is_none()
                    || self.next_page_id.is_some()
                    || self.page_rent_reserve_remaining != 0
                    || self.score != self.submission.claimed_score
                    || self.net_coefficients != self.submission.claimed_net_coefficients
                    || self.total_quote_debit_numerator
                        != self.submission.claimed_total_quote_debit_numerator =>
            {
                Err(Error::NonCanonicalState)
            }
            CandidatePhase::Rejected
                if self.verified_pages == 0 && !pristine
                    || self.verified_pages > 0
                        && (self.verified_executions == 0 || self.last_order_id.is_none()) =>
            {
                Err(Error::NonCanonicalState)
            }
            _ => Ok(()),
        }
    }

    /// Create a permissionless submitted candidate after exact authority and
    /// simplex validation. Signature and digest authentication are adapter
    /// responsibilities.
    pub fn submit(
        candidate_id: ContentId,
        submission: CandidateSubmissionV1<N>,
        root: GeneralRootV1,
        config: GeneralConfigV1,
        batch: &mut BatchRootV1,
        now_slot: u64,
    ) -> Result<Self> {
        root.validate_authority(
            submission.market,
            submission.claim_basis_id,
            submission.generation,
            config,
        )?;
        if submission.batch_sequence != batch.sequence || batch.phase != BatchPhase::Selecting {
            return Err(Error::InvalidPhase);
        }
        if now_slot < batch.collection_close || now_slot >= batch.selection_close {
            return Err(Error::OutsideWindow);
        }
        if submission.valid_until_slot < batch.selection_close
            || submission.claimed_execution_count == 0
            || submission.claimed_execution_count > config.max_orders_per_candidate
            || submission.claimed_page_count == 0
            || submission.claimed_page_count > config.max_pages_per_candidate
            || submission.outcome_count != config.outcome_count
        {
            return Err(Error::CandidateClaimMismatch);
        }
        validate_prices(
            &submission.prices,
            submission.outcome_count,
            config.price_scale,
        )?;
        let (verification_work_remaining, settlement_work_remaining, cleanup_work_remaining) =
            candidate_initial_work_reserves(
                submission.claimed_page_count,
                config.continuation_reward_lamports,
            )?;
        let next_open_candidates = batch
            .open_candidate_children
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let candidate = Self {
            candidate_id,
            next_page_id: Some(submission.first_page_id),
            submission,
            phase: CandidatePhase::Submitted,
            verified_pages: 0,
            verified_executions: 0,
            last_order_id: None,
            open_page_children: 0,
            page_rent_reserve_remaining: submission.page_rent_reserve_lamports,
            settlement_rent_reserve_remaining: submission.settlement_rent_reserve_lamports,
            verification_work_remaining,
            settlement_work_remaining,
            cleanup_work_remaining,
            net_coefficients: [0; N],
            total_quote_debit_numerator: 0,
            score: 0,
        };
        batch.open_candidate_children = next_open_candidates;
        Ok(candidate)
    }

    /// Check and commit one bounded page. Invalid inputs leave `self`
    /// unchanged because work is performed on a copied cursor first.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_page(
        &mut self,
        page_id: ContentId,
        page: &CandidatePageV1<N>,
        root: &GeneralRootV1,
        config: &GeneralConfigV1,
        batch: &BatchRootV1,
        now_slot: u64,
        capitalization: CandidateCapitalizationV1,
    ) -> Result<u64> {
        self.validate_capitalization(capitalization)?;
        if self.phase != CandidatePhase::Submitted && self.phase != CandidatePhase::Verifying {
            return Err(Error::InvalidPhase);
        }
        if batch.phase != BatchPhase::Selecting
            || batch.sequence != self.submission.batch_sequence
            || now_slot >= batch.selection_close
            || now_slot > self.submission.valid_until_slot
        {
            return Err(Error::OutsideWindow);
        }
        if page.page_index != self.verified_pages || self.next_page_id != Some(page_id) {
            return Err(Error::CursorMismatch);
        }
        let count = usize::from(page.execution_count);
        if count == 0 || count > MAX_EXECUTIONS_PER_PAGE_V1 {
            return Err(Error::InvalidPageCount);
        }
        if page.executions.iter().take(count).any(Option::is_none)
            || page.executions.iter().skip(count).any(Option::is_some)
        {
            return Err(Error::NonCanonicalReservedBytes);
        }
        let next_page_count = self
            .verified_pages
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if next_page_count > config.max_pages_per_candidate {
            return Err(Error::CapacityExceeded);
        }
        let mut next = *self;
        for execution in page.executions.iter().take(count).flatten() {
            next.verify_execution(execution, root, config, batch)?;
        }
        next.verified_pages = next_page_count;
        next.next_page_id = page.next_page_id;
        next.phase = CandidatePhase::Verifying;
        let reward = next.consume_verification(*config)?;
        *self = next;
        Ok(reward)
    }

    fn verify_execution(
        &mut self,
        execution: &ExecutionV1<N>,
        root: &GeneralRootV1,
        config: &GeneralConfigV1,
        batch: &BatchRootV1,
    ) -> Result<()> {
        validate_execution_binding(execution, root, config, batch)?;
        if self
            .last_order_id
            .is_some_and(|last| execution.order.order_id <= last)
        {
            return Err(Error::NonCanonicalOrder);
        }
        execution
            .order_state
            .validate_snapshot(execution.order, execution.fill_lots)?;
        let debit_per_lot = portfolio_dot(
            &execution.order.coefficients,
            &self.submission.prices,
            config.outcome_count,
        )?;
        if debit_per_lot > execution.order.max_quote_debit_per_lot_numerator {
            return Err(Error::LimitViolated);
        }
        let fill = i128::from(execution.fill_lots);
        let debit = debit_per_lot
            .checked_mul(fill)
            .ok_or(Error::ArithmeticOverflow)?;
        self.total_quote_debit_numerator = self
            .total_quote_debit_numerator
            .checked_add(debit)
            .ok_or(Error::ArithmeticOverflow)?;
        let preference_per_lot = execution
            .order
            .max_quote_debit_per_lot_numerator
            .checked_sub(debit_per_lot)
            .ok_or(Error::ArithmeticOverflow)?;
        let preference = preference_per_lot
            .checked_mul(fill)
            .ok_or(Error::ArithmeticOverflow)?;
        let preference_score = u128::try_from(preference).map_err(|_| Error::ArithmeticOverflow)?;
        self.score = self
            .score
            .checked_add(preference_score)
            .ok_or(Error::ArithmeticOverflow)?;
        let width = usize::from(config.outcome_count);
        for (target, coefficient) in self
            .net_coefficients
            .iter_mut()
            .zip(execution.order.coefficients.iter())
            .take(width)
        {
            let delta = i128::from(*coefficient)
                .checked_mul(fill)
                .ok_or(Error::ArithmeticOverflow)?;
            *target = target.checked_add(delta).ok_or(Error::ArithmeticOverflow)?;
        }
        self.verified_executions = self
            .verified_executions
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.verified_executions > config.max_orders_per_candidate {
            return Err(Error::CapacityExceeded);
        }
        self.last_order_id = Some(execution.order.order_id);
        Ok(())
    }

    /// Finish verification and prove the execution set has one exact virtual
    /// complete-set inventory delta and matching quote capitalization.
    pub fn finish_verification(
        &mut self,
        config: GeneralConfigV1,
        batch: BatchRootV1,
        capitalization: CandidateCapitalizationV1,
        now_slot: u64,
    ) -> Result<u64> {
        self.validate_capitalization(capitalization)?;
        if self.phase != CandidatePhase::Verifying {
            return Err(Error::InvalidPhase);
        }
        if batch.phase != BatchPhase::Selecting
            || batch.sequence != self.submission.batch_sequence
            || now_slot >= batch.selection_close
            || now_slot > self.submission.valid_until_slot
        {
            return Err(Error::OutsideWindow);
        }
        if self.next_page_id.is_some()
            || self.verified_pages != self.submission.claimed_page_count
            || self.verified_executions != self.submission.claimed_execution_count
            || self.open_page_children != self.submission.claimed_page_count
            || self.page_rent_reserve_remaining != 0
            || self.score != self.submission.claimed_score
            || self.net_coefficients != self.submission.claimed_net_coefficients
            || self.total_quote_debit_numerator
                != self.submission.claimed_total_quote_debit_numerator
        {
            return Err(Error::CandidateClaimMismatch);
        }
        let width = usize::from(config.outcome_count);
        let complete_set_delta = *self
            .net_coefficients
            .first()
            .ok_or(Error::InvalidOutcomeCount)?;
        if self
            .net_coefficients
            .iter()
            .take(width)
            .any(|value| *value != complete_set_delta)
        {
            return Err(Error::IncompleteSetImbalance);
        }
        let expected_debit = complete_set_delta
            .checked_mul(i128::from(config.price_scale))
            .ok_or(Error::ArithmeticOverflow)?;
        if self.total_quote_debit_numerator != expected_debit {
            return Err(Error::QuoteConservationMismatch);
        }
        let mut next = *self;
        next.phase = CandidatePhase::Valid;
        let reward = next.consume_verification(config)?;
        *self = next;
        Ok(reward)
    }

    /// Permanently reject a timed-out or invalid candidate account.
    pub fn reject(
        &mut self,
        config: GeneralConfigV1,
        capitalization: CandidateCapitalizationV1,
        now_slot: u64,
    ) -> Result<u64> {
        self.validate_capitalization(capitalization)?;
        if self.phase == CandidatePhase::Valid
            || self.phase == CandidatePhase::Considered
            || self.phase == CandidatePhase::Rejected
        {
            return Err(Error::InvalidPhase);
        }
        if now_slot <= self.submission.valid_until_slot {
            return Err(Error::OutsideWindow);
        }
        let mut next = *self;
        next.phase = CandidatePhase::Rejected;
        let reward = next.consume_verification(config)?;
        *self = next;
        Ok(reward)
    }

    /// Return the candidate identity.
    pub const fn candidate_id(self) -> ContentId {
        self.candidate_id
    }

    /// Return the exact verified preference-surplus score.
    pub const fn score(self) -> u128 {
        self.score
    }

    /// Return the verified lifecycle phase.
    pub const fn phase(self) -> CandidatePhase {
        self.phase
    }

    /// Return the next immutable page required by verification.
    pub const fn next_page_id(self) -> Option<ContentId> {
        self.next_page_id
    }

    /// Return the permissionless candidate submitter and rent beneficiary key.
    pub const fn submitter(self) -> OwnerKeyV1 {
        self.submission.submitter
    }

    /// Return the exact batch sequence.
    pub const fn batch_sequence(self) -> u64 {
        self.submission.batch_sequence
    }

    /// Return the exact verified execution count.
    pub const fn verified_executions(self) -> u32 {
        self.verified_executions
    }

    /// Return the exact verified page count.
    pub const fn verified_pages(self) -> u32 {
        self.verified_pages
    }

    /// Return the verified virtual complete-set delta.
    pub const fn complete_set_delta(self) -> i128 {
        self.net_coefficients[0]
    }

    /// Return the immutable first page for both settlement passes.
    pub const fn first_page_id(self) -> ContentId {
        self.submission.first_page_id
    }

    /// Return the exact live immutable page-child count.
    pub const fn open_page_children(self) -> u32 {
        self.open_page_children
    }

    /// Return remaining candidate-owned verification principal.
    pub const fn verification_work_remaining(self) -> u64 {
        self.verification_work_remaining
    }

    /// Return remaining selected-settlement principal.
    pub const fn settlement_work_remaining(self) -> u64 {
        self.settlement_work_remaining
    }

    /// Return remaining unspendable child-cleanup principal.
    pub const fn cleanup_work_remaining(self) -> u64 {
        self.cleanup_work_remaining
    }

    /// Validate exact physical candidate capitalization.
    pub fn validate_capitalization(self, observation: CandidateCapitalizationV1) -> Result<()> {
        let expected = observation
            .exact_state_rent_lamports
            .checked_add(self.page_rent_reserve_remaining)
            .and_then(|amount| amount.checked_add(self.settlement_rent_reserve_remaining))
            .and_then(|amount| amount.checked_add(self.verification_work_remaining))
            .and_then(|amount| amount.checked_add(self.settlement_work_remaining))
            .and_then(|amount| amount.checked_add(self.cleanup_work_remaining))
            .ok_or(Error::ArithmeticOverflow)?;
        if observation.account_lamports != expected {
            return Err(Error::GeneralFundingCustodyMismatch);
        }
        Ok(())
    }

    /// Attach one immutable candidate-exclusive page using exact reserved Rent.
    pub fn create_page(
        &mut self,
        page: CandidatePageV1<N>,
        config: GeneralConfigV1,
        page_rent_lamports: u64,
        precreation_lamports: u64,
        capitalization: CandidateCapitalizationV1,
    ) -> Result<CandidatePageCreationV1> {
        self.validate_capitalization(capitalization)?;
        if !matches!(
            self.phase,
            CandidatePhase::Submitted | CandidatePhase::Verifying
        ) || page.page_index >= self.submission.claimed_page_count
            || page.execution_count == 0
            || usize::from(page.execution_count) > MAX_EXECUTIONS_PER_PAGE_V1
        {
            return Err(Error::InvalidPhase);
        }
        let terminal = page.page_index + 1 == self.submission.claimed_page_count;
        if terminal != page.next_page_id.is_none() {
            return Err(Error::InvalidTranscriptStep);
        }
        let mut next = *self;
        next.open_page_children = next
            .open_page_children
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if next.open_page_children > next.submission.claimed_page_count {
            return Err(Error::CapacityExceeded);
        }
        next.page_rent_reserve_remaining = next
            .page_rent_reserve_remaining
            .checked_sub(page_rent_lamports)
            .ok_or(Error::InsufficientFunding)?;
        let page_top_up_lamports = page_rent_lamports.saturating_sub(precreation_lamports);
        let candidate_refund_lamports = core::cmp::min(precreation_lamports, page_rent_lamports);
        let page_surplus_refund_lamports = precreation_lamports.saturating_sub(page_rent_lamports);
        let plan = CandidatePageCreationV1 {
            page_rent_lamports,
            page_top_up_lamports,
            candidate_refund_lamports,
            page_surplus_refund_lamports,
        };
        let accounted = plan
            .page_top_up_lamports
            .checked_add(plan.candidate_refund_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        if accounted != page_rent_lamports {
            return Err(Error::FundingConservationMismatch);
        }
        let _ = config;
        *self = next;
        Ok(plan)
    }

    /// Close one stored page after rejection or after it can no longer win.
    pub fn close_page(
        &mut self,
        batch: BatchRootV1,
        config: GeneralConfigV1,
        capitalization: CandidateCapitalizationV1,
        page_account_lamports: u64,
    ) -> Result<CandidatePageCloseV1> {
        self.validate_capitalization(capitalization)?;
        if self.open_page_children == 0 {
            return Err(Error::NotQuiescent);
        }
        let selected = batch.best_candidate_id == Some(self.candidate_id);
        let closable = self.phase == CandidatePhase::Rejected
            || matches!(
                batch.phase,
                BatchPhase::Settling | BatchPhase::Applying | BatchPhase::Quiescent
            ) && !selected
            || batch.phase == BatchPhase::Quiescent
                && selected
                && batch.open_settlement_children == 0;
        if !closable {
            return Err(Error::NotQuiescent);
        }
        let mut next = *self;
        next.open_page_children = next
            .open_page_children
            .checked_sub(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let cleanup_reward_lamports = next.consume_cleanup(config)?;
        *self = next;
        Ok(CandidatePageCloseV1 {
            cleanup_reward_lamports,
            rent_credit_lamports: page_account_lamports,
            rent_beneficiary: self.submitter(),
        })
    }

    fn consume_verification(&mut self, config: GeneralConfigV1) -> Result<u64> {
        let reward = config.continuation_reward_lamports;
        self.verification_work_remaining = self
            .verification_work_remaining
            .checked_sub(reward)
            .ok_or(Error::InsufficientFunding)?;
        Ok(reward)
    }

    fn consume_settlement(&mut self, config: GeneralConfigV1) -> Result<u64> {
        let reward = config.continuation_reward_lamports;
        self.settlement_work_remaining = self
            .settlement_work_remaining
            .checked_sub(reward)
            .ok_or(Error::InsufficientFunding)?;
        Ok(reward)
    }

    fn consume_cleanup(&mut self, config: GeneralConfigV1) -> Result<u64> {
        let reward = config.continuation_reward_lamports;
        self.cleanup_work_remaining = self
            .cleanup_work_remaining
            .checked_sub(reward)
            .ok_or(Error::InsufficientFunding)?;
        Ok(reward)
    }
}

fn candidate_initial_work_reserves(
    page_count: u32,
    reward_lamports: u64,
) -> Result<(u64, u64, u64)> {
    let pages = u64::from(page_count);
    let verification = pages
        .checked_add(2)
        .and_then(|steps| steps.checked_mul(reward_lamports))
        .ok_or(Error::ArithmeticOverflow)?;
    let settlement = pages
        .checked_mul(2)
        .and_then(|steps| steps.checked_add(4))
        .and_then(|steps| steps.checked_mul(reward_lamports))
        .ok_or(Error::ArithmeticOverflow)?;
    let cleanup = pages
        .checked_add(1)
        .and_then(|steps| steps.checked_mul(reward_lamports))
        .ok_or(Error::ArithmeticOverflow)?;
    Ok((verification, settlement, cleanup))
}

fn initial_batch_work(config: GeneralConfigV1) -> Result<u64> {
    config
        .continuation_reward_lamports
        .checked_mul(3)
        .ok_or(Error::ArithmeticOverflow)
}

fn expected_batch_work(phase: BatchPhase, reward_lamports: u64) -> Result<u64> {
    let steps = match phase {
        BatchPhase::Collecting => 3,
        BatchPhase::Selecting => 2,
        BatchPhase::Settling | BatchPhase::Applying | BatchPhase::Quiescent => 1,
        BatchPhase::Retired => 0,
    };
    reward_lamports
        .checked_mul(steps)
        .ok_or(Error::ArithmeticOverflow)
}

/// Lifecycle of one frequent batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BatchPhase {
    /// Signed orders may be posted or cancelled.
    Collecting = 0,
    /// Permissionless candidates may be verified and considered.
    Selecting = 1,
    /// The deterministic best valid submitted candidate is being settled.
    Settling = 2,
    /// Hoard conversion is committed and paginated receipts must converge.
    Applying = 3,
    /// No further economic mutation is possible.
    Quiescent = 4,
    /// Owned candidate/receipt/rent state has been discharged.
    Retired = 5,
}

/// Fixed-layout root for one frequent batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchRootV1 {
    config_id: ContentId,
    sequence: u64,
    collection_close: u64,
    selection_close: u64,
    settlement_close: u64,
    considered_candidate_count: u32,
    open_candidate_children: u32,
    open_settlement_children: u8,
    best_candidate_id: Option<ContentId>,
    best_score: u128,
    work_remaining_lamports: u64,
    phase: BatchPhase,
}

impl BatchRootV1 {
    /// Decode one exact-width canonical frequent-batch root.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        exact_len(bytes, BATCH_ROOT_BYTES)?;
        if array::<8>(bytes, 0)? != BATCH_ROOT_MAGIC {
            return Err(Error::InvalidMagic);
        }
        validate_record_header(bytes)?;
        require_zero(bytes, 15, 1)?;
        let best_candidate_bytes = array::<CONTENT_ID_BYTES>(bytes, 88)?;
        let best_candidate_id = match read_u8(bytes, 13)? {
            0 => {
                if best_candidate_bytes.iter().any(|byte| *byte != 0) {
                    return Err(Error::NonCanonicalReservedBytes);
                }
                None
            }
            1 => Some(ContentId::new(best_candidate_bytes)?),
            _ => return Err(Error::NonCanonicalState),
        };
        let batch = Self {
            phase: decode_batch_phase(read_u8(bytes, 12)?)?,
            config_id: read_id(bytes, 16)?,
            sequence: read_u64(bytes, 48)?,
            collection_close: read_u64(bytes, 56)?,
            selection_close: read_u64(bytes, 64)?,
            settlement_close: read_u64(bytes, 72)?,
            considered_candidate_count: read_u32(bytes, 80)?,
            open_candidate_children: read_u32(bytes, 84)?,
            open_settlement_children: read_u8(bytes, 14)?,
            best_candidate_id,
            best_score: read_u128(bytes, 120)?,
            work_remaining_lamports: read_u64(bytes, 136)?,
        };
        batch.validate()?;
        Ok(batch)
    }

    /// Encode one exact-width canonical frequent-batch root.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        exact_len(out, BATCH_ROOT_BYTES)?;
        self.validate()?;
        out.fill(0);
        put(out, 0, &BATCH_ROOT_MAGIC);
        put(out, 8, &SCHEMA_V1.to_le_bytes());
        put(out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(out, 12, &[batch_phase_tag(self.phase)]);
        put(out, 14, &[self.open_settlement_children]);
        put(out, 16, self.config_id.as_bytes());
        put(out, 48, &self.sequence.to_le_bytes());
        put(out, 56, &self.collection_close.to_le_bytes());
        put(out, 64, &self.selection_close.to_le_bytes());
        put(out, 72, &self.settlement_close.to_le_bytes());
        put(out, 80, &self.considered_candidate_count.to_le_bytes());
        put(out, 84, &self.open_candidate_children.to_le_bytes());
        if let Some(candidate_id) = self.best_candidate_id {
            put(out, 13, &[1]);
            put(out, 88, candidate_id.as_bytes());
        }
        put(out, 120, &self.best_score.to_le_bytes());
        put(out, 136, &self.work_remaining_lamports.to_le_bytes());
        Ok(())
    }

    fn validate(self) -> Result<()> {
        if self.collection_close == 0
            || self.collection_close >= self.selection_close
            || self.selection_close >= self.settlement_close
        {
            return Err(Error::NonCanonicalState);
        }
        match (self.considered_candidate_count, self.best_candidate_id) {
            (0, None) if self.best_score == 0 => {}
            (0, _) | (_, None) => return Err(Error::NonCanonicalState),
            (_, Some(_)) => {}
        }
        if self.open_settlement_children > 1
            || self.phase == BatchPhase::Collecting
                && (self.considered_candidate_count != 0
                    || self.open_candidate_children != 0
                    || self.open_settlement_children != 0)
        {
            return Err(Error::NonCanonicalState);
        }
        if matches!(self.phase, BatchPhase::Settling | BatchPhase::Applying)
            && self.best_candidate_id.is_none()
        {
            return Err(Error::NonCanonicalState);
        }
        if self.phase == BatchPhase::Applying && self.open_settlement_children != 1 {
            return Err(Error::NonCanonicalState);
        }
        if self.phase == BatchPhase::Retired
            && (self.open_candidate_children != 0 || self.open_settlement_children != 0)
        {
            return Err(Error::NonCanonicalState);
        }
        if (self.phase == BatchPhase::Retired) != (self.work_remaining_lamports == 0) {
            return Err(Error::NonCanonicalState);
        }
        Ok(())
    }

    /// Create one batch from an already-reserved root sequence.
    pub fn open(
        config_id: ContentId,
        sequence: u64,
        open_slot: u64,
        config: GeneralConfigV1,
    ) -> Result<Self> {
        let collection_close = open_slot
            .checked_add(config.collection_slots)
            .ok_or(Error::SlotOverflow)?;
        let selection_close = collection_close
            .checked_add(config.selection_slots)
            .ok_or(Error::SlotOverflow)?;
        let settlement_close = selection_close
            .checked_add(config.settlement_slots)
            .ok_or(Error::SlotOverflow)?;
        Ok(Self {
            config_id,
            sequence,
            collection_close,
            selection_close,
            settlement_close,
            considered_candidate_count: 0,
            open_candidate_children: 0,
            open_settlement_children: 0,
            best_candidate_id: None,
            best_score: 0,
            work_remaining_lamports: initial_batch_work(config)?,
            phase: BatchPhase::Collecting,
        })
    }

    /// Close collection at or after its immutable boundary.
    pub fn open_selection(
        &mut self,
        config: GeneralConfigV1,
        capitalization: BatchCapitalizationV1,
        now_slot: u64,
    ) -> Result<u64> {
        self.validate_capitalization(config, capitalization)?;
        if self.phase != BatchPhase::Collecting || now_slot < self.collection_close {
            return Err(Error::OutsideWindow);
        }
        let mut next = *self;
        next.phase = BatchPhase::Selecting;
        let reward = next.consume_work(config)?;
        next.validate_against(config)?;
        *self = next;
        Ok(reward)
    }

    /// Consider one fully verified candidate. Higher exact score wins; equal
    /// score uses lexicographically smaller content identity. This is the
    /// **best valid submitted candidate**, not an optimal clearing claim.
    pub fn consider_candidate<const N: usize>(
        &mut self,
        candidate: &mut CandidateStateV1<N>,
        config: GeneralConfigV1,
        capitalization: CandidateCapitalizationV1,
        now_slot: u64,
    ) -> Result<u64> {
        candidate.validate_capitalization(capitalization)?;
        if self.phase != BatchPhase::Selecting || now_slot >= self.selection_close {
            return Err(Error::OutsideWindow);
        }
        if candidate.phase != CandidatePhase::Valid
            || candidate.submission.batch_sequence != self.sequence
        {
            return Err(Error::CandidateClaimMismatch);
        }
        let mut next_batch = *self;
        let mut next_candidate = *candidate;
        next_batch.considered_candidate_count = next_batch
            .considered_candidate_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let replace = match next_batch.best_candidate_id {
            None => true,
            Some(best_id) => {
                candidate.score > self.best_score
                    || (candidate.score == self.best_score && candidate.candidate_id < best_id)
            }
        };
        if replace {
            next_batch.best_candidate_id = Some(candidate.candidate_id);
            next_batch.best_score = candidate.score;
        }
        next_candidate.phase = CandidatePhase::Considered;
        let reward = next_candidate.consume_verification(config)?;
        *self = next_batch;
        *candidate = next_candidate;
        Ok(reward)
    }

    /// Freeze deterministic selection. An empty batch becomes quiescent.
    pub fn close_selection(
        &mut self,
        config: GeneralConfigV1,
        capitalization: BatchCapitalizationV1,
        now_slot: u64,
    ) -> Result<(Option<ContentId>, u64)> {
        self.validate_capitalization(config, capitalization)?;
        if self.phase != BatchPhase::Selecting || now_slot < self.selection_close {
            return Err(Error::OutsideWindow);
        }
        let mut next = *self;
        let selected = match next.best_candidate_id {
            Some(id) => {
                next.phase = BatchPhase::Settling;
                Some(id)
            }
            None => {
                next.phase = BatchPhase::Quiescent;
                None
            }
        };
        let reward = next.consume_work(config)?;
        next.validate_against(config)?;
        *self = next;
        Ok((selected, reward))
    }

    /// Expire a winner only before Hoard conversion and receipt application
    /// begin. An applying settlement cannot time out; segregated liveness
    /// funding drives it to its committed conclusion.
    pub fn expire_unsettled<const N: usize>(
        &mut self,
        candidate: &mut CandidateStateV1<N>,
        config: GeneralConfigV1,
        capitalization: CandidateCapitalizationV1,
        now_slot: u64,
    ) -> Result<u64> {
        candidate.validate_capitalization(capitalization)?;
        if self.phase != BatchPhase::Settling || now_slot <= self.settlement_close {
            return Err(Error::OutsideWindow);
        }
        if self.best_candidate_id != Some(candidate.candidate_id)
            || candidate.phase != CandidatePhase::Considered
        {
            return Err(Error::CandidateNotSelected);
        }
        let mut next_batch = *self;
        let mut next_candidate = *candidate;
        next_batch.phase = BatchPhase::Quiescent;
        let reward = next_candidate.consume_settlement(config)?;
        *self = next_batch;
        *candidate = next_candidate;
        Ok(reward)
    }

    /// Retire and close only after every persisted candidate and settlement child closes.
    pub fn retire(
        &mut self,
        root: &mut GeneralRootV1,
        config: GeneralConfigV1,
        capitalization: BatchCapitalizationV1,
    ) -> Result<BatchCloseV1> {
        self.validate_capitalization(config, capitalization)?;
        if self.config_id != root.config_id {
            return Err(Error::AuthorityMismatch);
        }
        if self.phase != BatchPhase::Quiescent
            || self.open_candidate_children != 0
            || self.open_settlement_children != 0
        {
            return Err(Error::NotQuiescent);
        }
        let mut next_batch = *self;
        let mut next_root = *root;
        next_batch.phase = BatchPhase::Retired;
        let continuation_reward_lamports = next_batch.consume_work(config)?;
        next_batch.validate_against(config)?;
        next_root.close_batch()?;
        let rent_credit_lamports = capitalization
            .account_lamports
            .checked_sub(continuation_reward_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        *self = next_batch;
        *root = next_root;
        Ok(BatchCloseV1 {
            continuation_reward_lamports,
            rent_credit_lamports,
            rent_beneficiary: root.rent_beneficiary,
        })
    }

    /// Return the batch phase.
    pub const fn phase(self) -> BatchPhase {
        self.phase
    }

    /// Return the immutable collection close.
    pub const fn collection_close(self) -> u64 {
        self.collection_close
    }

    /// Return the authenticated config commitment.
    pub const fn config_id(self) -> ContentId {
        self.config_id
    }

    /// Return the immutable batch sequence.
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    /// Return the immutable candidate-selection close.
    pub const fn selection_close(self) -> u64 {
        self.selection_close
    }

    /// Return the immutable pre-application settlement close.
    pub const fn settlement_close(self) -> u64 {
        self.settlement_close
    }

    /// Return the selected candidate, if any.
    pub const fn best_candidate_id(self) -> Option<ContentId> {
        self.best_candidate_id
    }

    /// Return the exact number of live candidate child accounts.
    pub const fn open_candidate_children(self) -> u32 {
        self.open_candidate_children
    }

    /// Return the exact number of live settlement child clusters.
    pub const fn open_settlement_children(self) -> u8 {
        self.open_settlement_children
    }

    /// Return prepaid permissionless work still owned by this batch.
    pub const fn work_remaining_lamports(self) -> u64 {
        self.work_remaining_lamports
    }

    /// Validate the exact phase-derived batch work compartment.
    pub fn validate_against(self, config: GeneralConfigV1) -> Result<()> {
        let expected = expected_batch_work(self.phase, config.continuation_reward_lamports)?;
        if self.work_remaining_lamports != expected {
            return Err(Error::InsufficientFunding);
        }
        Ok(())
    }

    /// Validate physical capitalization against Rent and the one persisted work owner.
    pub fn validate_capitalization(
        self,
        config: GeneralConfigV1,
        observation: BatchCapitalizationV1,
    ) -> Result<()> {
        self.validate_against(config)?;
        let expected = observation
            .exact_state_rent_lamports
            .checked_add(self.work_remaining_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        if observation.account_lamports != expected {
            return Err(Error::InsufficientFunding);
        }
        Ok(())
    }

    fn consume_work(&mut self, config: GeneralConfigV1) -> Result<u64> {
        let reward = config.continuation_reward_lamports;
        self.work_remaining_lamports = self
            .work_remaining_lamports
            .checked_sub(reward)
            .ok_or(Error::InsufficientFunding)?;
        Ok(reward)
    }

    /// Close one candidate child only when it can no longer affect selection or settlement.
    pub fn close_candidate_child<const N: usize>(
        &mut self,
        mut candidate: CandidateStateV1<N>,
        config: GeneralConfigV1,
        capitalization: CandidateCapitalizationV1,
    ) -> Result<CandidateCloseV1> {
        candidate.validate_capitalization(capitalization)?;
        if candidate.batch_sequence() != self.sequence || self.open_candidate_children == 0 {
            return Err(Error::AuthorityMismatch);
        }
        if candidate.open_page_children != 0 {
            return Err(Error::NotQuiescent);
        }
        let selected = self.best_candidate_id == Some(candidate.candidate_id());
        let closable = candidate.phase() == CandidatePhase::Rejected
            || matches!(
                self.phase,
                BatchPhase::Settling | BatchPhase::Applying | BatchPhase::Quiescent
            ) && !selected
            || self.phase == BatchPhase::Quiescent
                && selected
                && self.open_settlement_children == 0;
        if !closable {
            return Err(Error::NotQuiescent);
        }
        let cleanup_reward_lamports = candidate.consume_cleanup(config)?;
        let rent_credit_lamports = capitalization
            .account_lamports
            .checked_sub(cleanup_reward_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        self.open_candidate_children = self
            .open_candidate_children
            .checked_sub(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(CandidateCloseV1 {
            cleanup_reward_lamports,
            rent_credit_lamports,
            rent_beneficiary: candidate.submitter(),
        })
    }
}

/// Exact receipt for one atomic portfolio fill.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptV1<const N: usize> {
    /// Winning candidate commitment.
    pub candidate_id: ContentId,
    /// Signed order commitment.
    pub order_id: ContentId,
    /// Exact signed owner receiving the outcome and quote deltas.
    pub owner: OwnerKeyV1,
    /// Market occurrence generation.
    pub generation: u64,
    /// Batch sequence.
    pub batch_sequence: u64,
    /// Replay nonce.
    pub nonce: u64,
    /// Filled scalar lots.
    pub fill_lots: u64,
    /// Remaining lots after this receipt is atomically applied.
    pub remaining_lots: u64,
    /// Signed integral settlement-asset delta.
    pub quote_delta_atoms: i64,
    /// Prefix carry before the one named rounding boundary.
    pub carry_before: u64,
    /// Prefix carry after the one named rounding boundary.
    pub carry_after: u64,
    /// Signed claim-token deltas in canonical ClaimBasis order.
    pub outcome_deltas: [i64; N],
    /// Exact ClaimBasis width, which must equal the selected const width.
    pub outcome_count: u16,
}

impl<const N: usize> SettlementReceiptV1<N> {
    /// Return the checked exact encoded receipt width for `N` deltas.
    pub fn encoded_len() -> Result<usize> {
        validate_width(N)?;
        SETTLEMENT_RECEIPT_BASE_BYTES
            .checked_add(N.checked_mul(8).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Encode into one exact-width adapter-consumable receipt buffer.
    #[allow(clippy::needless_borrow)]
    pub fn encode(&self, mut out: &mut [u8]) -> Result<()> {
        exact_len(out, Self::encoded_len()?)?;
        validate_portfolio(&self.outcome_deltas, self.outcome_count, false)?;
        out.fill(0);
        put(&mut out, 0, &RECEIPT_MAGIC);
        put(&mut out, 8, &SCHEMA_V1.to_le_bytes());
        put(&mut out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(&mut out, 12, &self.outcome_count.to_le_bytes());
        put(&mut out, 16, self.candidate_id.as_bytes());
        put(&mut out, 48, self.order_id.as_bytes());
        put(&mut out, 80, self.owner.as_bytes());
        put(&mut out, 112, &self.generation.to_le_bytes());
        put(&mut out, 120, &self.batch_sequence.to_le_bytes());
        put(&mut out, 128, &self.nonce.to_le_bytes());
        put(&mut out, 136, &self.fill_lots.to_le_bytes());
        put(&mut out, 144, &self.remaining_lots.to_le_bytes());
        put(&mut out, 152, &self.quote_delta_atoms.to_le_bytes());
        put(&mut out, 160, &self.carry_before.to_le_bytes());
        put(&mut out, 168, &self.carry_after.to_le_bytes());
        for (index, delta) in self.outcome_deltas.iter().enumerate() {
            if let Some(offset) = index
                .checked_mul(8)
                .and_then(|part| 176usize.checked_add(part))
            {
                put(&mut out, offset, &delta.to_le_bytes());
            }
        }
        Ok(())
    }

    /// Decode one exact receipt and reject wrong-width or width-substituted deltas.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        exact_len(bytes, Self::encoded_len()?)?;
        if array::<8>(bytes, 0)? != RECEIPT_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != SCHEMA_V1 || read_u16(bytes, 10)? != ARTIFACT_PROFILE_V1 {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, 14, 2)?;
        let mut outcome_deltas = [0i64; N];
        for (index, target) in outcome_deltas.iter_mut().enumerate() {
            let offset = index
                .checked_mul(8)
                .and_then(|part| 176usize.checked_add(part))
                .ok_or(Error::ArithmeticOverflow)?;
            *target = read_i64(bytes, offset)?;
        }
        let outcome_count = read_u16(bytes, 12)?;
        validate_portfolio(&outcome_deltas, outcome_count, false)?;
        Ok(Self {
            candidate_id: read_id(bytes, 16)?,
            order_id: read_id(bytes, 48)?,
            owner: read_owner_key(bytes, 80)?,
            generation: read_u64(bytes, 112)?,
            batch_sequence: read_u64(bytes, 120)?,
            nonce: read_u64(bytes, 128)?,
            fill_lots: read_u64(bytes, 136)?,
            remaining_lots: read_u64(bytes, 144)?,
            quote_delta_atoms: read_i64(bytes, 152)?,
            carry_before: read_u64(bytes, 160)?,
            carry_after: read_u64(bytes, 168)?,
            outcome_deltas,
            outcome_count,
        })
    }
}

/// Physical phase of one globally balanced, two-pass settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettlementPhaseV1 {
    /// Negative quote and claim deltas are being collected without outputs.
    CollectingInputs = 0,
    /// The sole complete-set mutation has occurred and outputs are being paid.
    DistributingOutputs = 1,
    /// Both exact replays converged and all temporary custody is empty.
    Finished = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SettlementReplayV1<const N: usize> {
    next_page_id: Option<ContentId>,
    pages: u32,
    executions: u32,
    last_order_id: Option<ContentId>,
    rounding_carry: u64,
    net_coefficients: [i128; N],
    total_quote_debit_numerator: i128,
    score: u128,
}

impl<const N: usize> SettlementReplayV1<N> {
    const fn pristine(first_page_id: ContentId) -> Self {
        Self {
            next_page_id: Some(first_page_id),
            pages: 0,
            executions: 0,
            last_order_id: None,
            rounding_carry: 0,
            net_coefficients: [0; N],
            total_quote_debit_numerator: 0,
            score: 0,
        }
    }

    fn validate(self) -> Result<()> {
        if self.pages == 0 {
            if self.executions != 0
                || self.last_order_id.is_some()
                || self.rounding_carry != 0
                || self.net_coefficients.iter().any(|value| *value != 0)
                || self.total_quote_debit_numerator != 0
                || self.score != 0
            {
                return Err(Error::NonCanonicalState);
            }
        } else if self.executions == 0 || self.last_order_id.is_none() {
            return Err(Error::NonCanonicalState);
        }
        Ok(())
    }

    fn matches_candidate(self, candidate: CandidateStateV1<N>) -> Result<()> {
        if self.next_page_id.is_some()
            || self.pages != candidate.verified_pages
            || self.executions != candidate.verified_executions
            || self.net_coefficients != candidate.net_coefficients
            || self.total_quote_debit_numerator != candidate.total_quote_debit_numerator
            || self.score != candidate.score
        {
            return Err(Error::CandidateClaimMismatch);
        }
        if self.rounding_carry != 0 {
            return Err(Error::RoundingCarryOutstanding);
        }
        Ok(())
    }
}

/// Committed settlement cursor and sole owner of temporary output obligations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementCursorV1<const N: usize> {
    candidate_id: ContentId,
    phase: SettlementPhaseV1,
    collection: SettlementReplayV1<N>,
    distribution: SettlementReplayV1<N>,
    claim_outputs_remaining: [u64; N],
    quote_outputs_remaining: u64,
}

/// Atomic capitalization and state plan for beginning selected settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementBeginV1<const N: usize> {
    cursor: SettlementCursorV1<N>,
    reward_lamports: u64,
    temporary_top_up_lamports: [u64; 3],
    candidate_refund_lamports: u64,
    temporary_surplus_refund_lamports: [u64; 3],
}

impl<const N: usize> SettlementBeginV1<N> {
    /// Return the newly initialized minimal settlement cursor.
    pub const fn cursor(self) -> SettlementCursorV1<N> {
        self.cursor
    }

    /// Return the exact begin reward paid to the permissionless actor.
    pub const fn reward_lamports(self) -> u64 {
        self.reward_lamports
    }

    /// Return candidate-funded top-ups for cursor, Position, and quote escrow.
    pub const fn temporary_top_up_lamports(self) -> [u64; 3] {
        self.temporary_top_up_lamports
    }

    /// Return candidate reserve displaced by safe precreation dust.
    pub const fn candidate_refund_lamports(self) -> u64 {
        self.candidate_refund_lamports
    }

    /// Return surplus removed from the three temporary accounts after allocation.
    pub const fn temporary_surplus_refund_lamports(self) -> [u64; 3] {
        self.temporary_surplus_refund_lamports
    }
}

/// Fixed result envelope from one collection or distribution page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementPageResultV1<const N: usize> {
    /// Number of leading results in all fixed arrays.
    pub execution_count: u8,
    /// Required physical settlement-token escrow balance after this page.
    pub quote_inventory_after: u64,
    /// Required ordered settlement Position balances after this page.
    pub claim_inventory_after: [u64; N],
    /// Exact selected-settlement reward released to the permissionless actor.
    pub settlement_reward_lamports: u64,
    /// Distribution-only atomic immutable-page close and cleanup plan.
    pub page_close: Option<CandidatePageCloseV1>,
}

/// One bounded execution plan recomputed from the persisted page and replay prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementExecutionPlanV1<const N: usize> {
    /// Exact post-fill replay state.
    pub order_state: OrderStateV1,
    /// Exact candidate-bound receipt.
    pub receipt: SettlementReceiptV1<N>,
    /// Exact negative-input and positive-output split.
    pub custody_effect: GeneralCustodyConsumptionV1<N>,
}

/// The sole complete-set mutation authorized after all inputs are collected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementMaterializationActionV1 {
    /// No complete-set supply or Hoard collateral changes.
    None,
    /// Deposit exact collateral and split this many complete sets.
    Split(u64),
    /// Merge this many complete sets and withdraw exact collateral.
    Merge(u64),
}

/// Exact adapter plan for the atomic Market/Hoard materialization boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementMaterializationV1<const N: usize> {
    action: SettlementMaterializationActionV1,
    quote_inventory_after: u64,
    claim_inventory_after: [u64; N],
    reward_lamports: u64,
}

impl<const N: usize> SettlementMaterializationV1<N> {
    /// Return the only permitted Market supply transition.
    pub const fn action(self) -> SettlementMaterializationActionV1 {
        self.action
    }

    /// Return the exact settlement-token escrow balance after materialization.
    pub const fn quote_inventory_after(self) -> u64 {
        self.quote_inventory_after
    }

    /// Return the exact Position balances after the sole supply mutation.
    pub const fn claim_inventory_after(self) -> [u64; N] {
        self.claim_inventory_after
    }

    /// Return the exact selected-settlement reward released to the actor.
    pub const fn reward_lamports(self) -> u64 {
        self.reward_lamports
    }
}

impl<const N: usize> SettlementCursorV1<N> {
    /// Return the exact persisted width for this ClaimBasis width.
    pub fn encoded_len() -> Result<usize> {
        validate_width(N)?;
        SETTLEMENT_CURSOR_BASE_BYTES
            .checked_add(N.checked_mul(40).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Decode one canonical selected-candidate settlement cursor.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        exact_len(bytes, Self::encoded_len()?)?;
        if array::<8>(bytes, 0)? != SETTLEMENT_CURSOR_MAGIC {
            return Err(Error::InvalidMagic);
        }
        validate_record_header(bytes)?;
        if usize::from(read_u16(bytes, 12)?) != N {
            return Err(Error::InvalidOutcomeCount);
        }
        require_zero(bytes, 15, 1)?;
        require_zero(bytes, 89, 7)?;
        require_zero(bytes, 209, 7)?;
        require_zero(bytes, 296, 8)?;
        let collection_net = SETTLEMENT_CURSOR_VECTOR_OFFSET;
        let claim_outputs_offset = vector_offset(collection_net, N, 16)?;
        let distribution_net = vector_offset(claim_outputs_offset, N, 8)?;
        let mut collection_coefficients = [0i128; N];
        let mut distribution_coefficients = [0i128; N];
        let mut claim_outputs_remaining = [0u64; N];
        for (index, ((collection, outputs), distribution)) in collection_coefficients
            .iter_mut()
            .zip(claim_outputs_remaining.iter_mut())
            .zip(distribution_coefficients.iter_mut())
            .enumerate()
        {
            *collection = read_i128(bytes, vector_offset(collection_net, index, 16)?)?;
            *outputs = read_u64(bytes, vector_offset(claim_outputs_offset, index, 8)?)?;
            *distribution = read_i128(bytes, vector_offset(distribution_net, index, 16)?)?;
        }
        let cursor = Self {
            candidate_id: read_id(bytes, 16)?,
            phase: decode_settlement_phase(read_u8(bytes, 14)?)?,
            collection: SettlementReplayV1 {
                next_page_id: read_zeroable_id(bytes, 48)?,
                pages: read_u32(bytes, 80)?,
                executions: read_u32(bytes, 84)?,
                last_order_id: decode_optional_id(bytes, 88, 96)?,
                rounding_carry: read_u64(bytes, 128)?,
                net_coefficients: collection_coefficients,
                total_quote_debit_numerator: read_i128(bytes, 136)?,
                score: read_u128(bytes, 152)?,
            },
            distribution: SettlementReplayV1 {
                next_page_id: read_zeroable_id(bytes, 168)?,
                pages: read_u32(bytes, 200)?,
                executions: read_u32(bytes, 204)?,
                last_order_id: decode_optional_id(bytes, 208, 216)?,
                rounding_carry: read_u64(bytes, 248)?,
                net_coefficients: distribution_coefficients,
                total_quote_debit_numerator: read_i128(bytes, 256)?,
                score: read_u128(bytes, 272)?,
            },
            claim_outputs_remaining,
            quote_outputs_remaining: read_u64(bytes, 288)?,
        };
        cursor.validate_persisted_shape()?;
        Ok(cursor)
    }

    /// Encode one canonical selected-candidate settlement cursor.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        exact_len(out, Self::encoded_len()?)?;
        self.validate_persisted_shape()?;
        out.fill(0);
        put(out, 0, &SETTLEMENT_CURSOR_MAGIC);
        put(out, 8, &SCHEMA_V1.to_le_bytes());
        put(out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(
            out,
            12,
            &u16::try_from(N)
                .map_err(|_| Error::InvalidOutcomeCount)?
                .to_le_bytes(),
        );
        put(out, 14, &[settlement_phase_tag(self.phase)]);
        put(out, 16, self.candidate_id.as_bytes());
        encode_replay_base(out, self.collection, 48, 80, 84, 88, 96, 128, 136, 152);
        encode_replay_base(
            out,
            self.distribution,
            168,
            200,
            204,
            208,
            216,
            248,
            256,
            272,
        );
        put(out, 288, &self.quote_outputs_remaining.to_le_bytes());
        let collection_net = SETTLEMENT_CURSOR_VECTOR_OFFSET;
        let claim_outputs_offset = vector_offset(collection_net, N, 16)?;
        let distribution_net = vector_offset(claim_outputs_offset, N, 8)?;
        for (index, ((collection, outputs), distribution)) in self
            .collection
            .net_coefficients
            .iter()
            .zip(self.claim_outputs_remaining.iter())
            .zip(self.distribution.net_coefficients.iter())
            .enumerate()
        {
            put(
                out,
                vector_offset(collection_net, index, 16)?,
                &collection.to_le_bytes(),
            );
            put(
                out,
                vector_offset(claim_outputs_offset, index, 8)?,
                &outputs.to_le_bytes(),
            );
            put(
                out,
                vector_offset(distribution_net, index, 16)?,
                &distribution.to_le_bytes(),
            );
        }
        Ok(())
    }

    fn validate_persisted_shape(&self) -> Result<()> {
        self.collection.validate()?;
        self.distribution.validate()?;
        match self.phase {
            SettlementPhaseV1::CollectingInputs => {
                if self.distribution.pages != 0 {
                    return Err(Error::NonCanonicalState);
                }
            }
            SettlementPhaseV1::DistributingOutputs => {}
            SettlementPhaseV1::Finished => {
                if self
                    .claim_outputs_remaining
                    .iter()
                    .any(|amount| *amount != 0)
                    || self.quote_outputs_remaining != 0
                {
                    return Err(Error::NonCanonicalState);
                }
            }
        }
        Ok(())
    }

    /// Begin collection without moving collateral or mutating Market supply.
    pub fn begin(
        candidate: &mut CandidateStateV1<N>,
        batch: &mut BatchRootV1,
        root: GeneralRootV1,
        config: GeneralConfigV1,
        capitalization: CandidateCapitalizationV1,
        rent: SettlementRentObservationV1,
        now_slot: u64,
    ) -> Result<SettlementBeginV1<N>> {
        candidate.validate_capitalization(capitalization)?;
        root.validate_authority(
            candidate.submission.market,
            candidate.submission.claim_basis_id,
            candidate.submission.generation,
            config,
        )?;
        if candidate.phase != CandidatePhase::Considered
            || batch.phase != BatchPhase::Settling
            || batch.best_candidate_id != Some(candidate.candidate_id)
            || batch.open_settlement_children != 0
        {
            return Err(Error::CandidateNotSelected);
        }
        if now_slot > batch.settlement_close {
            return Err(Error::OutsideWindow);
        }
        let pages = u64::from(candidate.submission.claimed_page_count);
        let expected_settlement = pages
            .checked_mul(2)
            .and_then(|steps| steps.checked_add(4))
            .and_then(|steps| steps.checked_mul(config.continuation_reward_lamports))
            .ok_or(Error::ArithmeticOverflow)?;
        let expected_cleanup = pages
            .checked_add(1)
            .and_then(|steps| steps.checked_mul(config.continuation_reward_lamports))
            .ok_or(Error::ArithmeticOverflow)?;
        if candidate.verification_work_remaining != 0
            || candidate.settlement_work_remaining != expected_settlement
            || candidate.cleanup_work_remaining != expected_cleanup
            || candidate.open_page_children != candidate.submission.claimed_page_count
        {
            return Err(Error::InsufficientFunding);
        }
        let exact_settlement_rent = rent
            .exact_rent_lamports
            .iter()
            .try_fold(0u64, |total, amount| total.checked_add(*amount))
            .ok_or(Error::ArithmeticOverflow)?;
        if candidate.settlement_rent_reserve_remaining != exact_settlement_rent {
            return Err(Error::InsufficientFunding);
        }
        let mut temporary_top_up_lamports = [0u64; 3];
        let mut temporary_surplus_refund_lamports = [0u64; 3];
        let mut candidate_refund_lamports = 0u64;
        for (((exact, precreation), top_up), surplus) in rent
            .exact_rent_lamports
            .iter()
            .zip(rent.precreation_lamports.iter())
            .zip(temporary_top_up_lamports.iter_mut())
            .zip(temporary_surplus_refund_lamports.iter_mut())
        {
            *top_up = exact.saturating_sub(*precreation);
            *surplus = precreation.saturating_sub(*exact);
            candidate_refund_lamports = candidate_refund_lamports
                .checked_add(core::cmp::min(*exact, *precreation))
                .ok_or(Error::ArithmeticOverflow)?;
        }
        let mut next_batch = *batch;
        let mut next_candidate = *candidate;
        next_candidate.settlement_rent_reserve_remaining = 0;
        next_batch.phase = BatchPhase::Applying;
        next_batch.open_settlement_children = 1;
        let pristine = SettlementReplayV1::pristine(candidate.first_page_id());
        let cursor = Self {
            candidate_id: candidate.candidate_id,
            phase: SettlementPhaseV1::CollectingInputs,
            collection: pristine,
            distribution: pristine,
            claim_outputs_remaining: [0; N],
            quote_outputs_remaining: 0,
        };
        let reward = next_candidate.consume_settlement(config)?;
        *batch = next_batch;
        *candidate = next_candidate;
        Ok(SettlementBeginV1 {
            cursor,
            reward_lamports: reward,
            temporary_top_up_lamports,
            candidate_refund_lamports,
            temporary_surplus_refund_lamports,
        })
    }

    /// Collect one page's negative deltas into cursor custody and emit no output.
    #[allow(clippy::too_many_arguments)]
    pub fn collect_page(
        &mut self,
        page_id: ContentId,
        page: &CandidatePageV1<N>,
        candidate: &mut CandidateStateV1<N>,
        root: &GeneralRootV1,
        config: &GeneralConfigV1,
        batch: &BatchRootV1,
        claim_inventory_before: [u64; N],
        quote_inventory_before: u64,
        capitalization: CandidateCapitalizationV1,
    ) -> Result<SettlementPageResultV1<N>> {
        candidate.validate_capitalization(capitalization)?;
        self.require_active(candidate, batch, SettlementPhaseV1::CollectingInputs)?;
        self.authenticate_inventory(claim_inventory_before, quote_inventory_before, config)?;
        let replay = replay_page(
            &self.collection,
            page_id,
            page,
            candidate,
            root,
            config,
            batch,
        )?;
        let mut quote_outputs_remaining = self.quote_outputs_remaining;
        let mut claim_outputs_remaining = self.claim_outputs_remaining;
        for index in 0..usize::from(page.execution_count) {
            let execution = replay_execution_plan(
                &self.collection,
                page_id,
                page,
                candidate,
                root,
                config,
                batch,
                index,
            )?;
            quote_outputs_remaining = quote_outputs_remaining
                .checked_add(execution.custody_effect.quote_credit_to_owner())
                .ok_or(Error::ArithmeticOverflow)?;
            for (credit, outputs) in execution
                .custody_effect
                .claim_credits_to_owner()
                .iter()
                .zip(claim_outputs_remaining.iter_mut())
            {
                *outputs = outputs
                    .checked_add(*credit)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
        }
        let (claims_after, quote_after) = expected_settlement_inventory(
            SettlementPhaseV1::CollectingInputs,
            &replay,
            &claim_outputs_remaining,
            quote_outputs_remaining,
            config,
        )?;
        let reward = candidate.consume_settlement(*config)?;
        self.collection = replay;
        self.claim_outputs_remaining = claim_outputs_remaining;
        self.quote_outputs_remaining = quote_outputs_remaining;
        Ok(SettlementPageResultV1 {
            execution_count: page.execution_count,
            quote_inventory_after: quote_after,
            claim_inventory_after: claims_after,
            settlement_reward_lamports: reward,
            page_close: None,
        })
    }

    /// Recompute one bounded execution from the current page replay prefix.
    ///
    /// This compact per-execution projection keeps the kernel and SVM adapter
    /// below the SVM stack bound without weakening whole-page replay: the
    /// corresponding page transition independently replays every execution
    /// before committing the cursor and one page reward.
    #[allow(clippy::too_many_arguments)]
    pub fn execution_plan(
        &self,
        page_id: ContentId,
        page: &CandidatePageV1<N>,
        candidate: &CandidateStateV1<N>,
        root: &GeneralRootV1,
        config: &GeneralConfigV1,
        batch: &BatchRootV1,
        index: usize,
    ) -> Result<SettlementExecutionPlanV1<N>> {
        let replay = match self.phase {
            SettlementPhaseV1::CollectingInputs => &self.collection,
            SettlementPhaseV1::DistributingOutputs => &self.distribution,
            SettlementPhaseV1::Finished => return Err(Error::InvalidPhase),
        };
        self.require_active(candidate, batch, self.phase)?;
        replay_execution_plan(replay, page_id, page, candidate, root, config, batch, index)
    }

    /// Perform the one atomic complete-set split/merge after all inputs converge.
    #[allow(clippy::too_many_arguments)]
    pub fn materialize(
        &mut self,
        candidate: &mut CandidateStateV1<N>,
        batch: BatchRootV1,
        root: GeneralRootV1,
        config: GeneralConfigV1,
        claim_inventory_before: [u64; N],
        quote_inventory_before: u64,
        capitalization: CandidateCapitalizationV1,
    ) -> Result<SettlementMaterializationV1<N>> {
        candidate.validate_capitalization(capitalization)?;
        root.validate_authority(
            candidate.submission.market,
            candidate.submission.claim_basis_id,
            candidate.submission.generation,
            config,
        )?;
        self.require_active(candidate, &batch, SettlementPhaseV1::CollectingInputs)?;
        self.collection.matches_candidate(*candidate)?;
        self.authenticate_inventory(claim_inventory_before, quote_inventory_before, &config)?;
        let mut next = *self;
        let action = if candidate.complete_set_delta() > 0 {
            let quantity = u64::try_from(candidate.complete_set_delta())
                .map_err(|_| Error::TokenAmountOutOfRange)?;
            SettlementMaterializationActionV1::Split(quantity)
        } else if candidate.complete_set_delta() < 0 {
            let quantity = u64::try_from(
                candidate
                    .complete_set_delta()
                    .checked_neg()
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .map_err(|_| Error::TokenAmountOutOfRange)?;
            SettlementMaterializationActionV1::Merge(quantity)
        } else {
            SettlementMaterializationActionV1::None
        };
        validate_materialization_conservation(
            claim_inventory_before,
            quote_inventory_before,
            next.claim_outputs_remaining,
            next.quote_outputs_remaining,
            action,
        )?;
        next.phase = SettlementPhaseV1::DistributingOutputs;
        let mut next_candidate = *candidate;
        let reward_lamports = next_candidate.consume_settlement(config)?;
        *self = next;
        *candidate = next_candidate;
        Ok(SettlementMaterializationV1 {
            action,
            quote_inventory_after: next.quote_outputs_remaining,
            claim_inventory_after: next.claim_outputs_remaining,
            reward_lamports,
        })
    }

    /// Replay one page again and distribute only its positive deltas.
    #[allow(clippy::too_many_arguments)]
    pub fn distribute_page(
        &mut self,
        page_id: ContentId,
        page: &CandidatePageV1<N>,
        candidate: &mut CandidateStateV1<N>,
        root: &GeneralRootV1,
        config: &GeneralConfigV1,
        batch: &BatchRootV1,
        claim_inventory_before: [u64; N],
        quote_inventory_before: u64,
        page_account_lamports: u64,
        capitalization: CandidateCapitalizationV1,
    ) -> Result<SettlementPageResultV1<N>> {
        candidate.validate_capitalization(capitalization)?;
        self.require_active(candidate, batch, SettlementPhaseV1::DistributingOutputs)?;
        self.authenticate_inventory(claim_inventory_before, quote_inventory_before, config)?;
        let replay = replay_page(
            &self.distribution,
            page_id,
            page,
            candidate,
            root,
            config,
            batch,
        )?;
        let mut quote_outputs_remaining = self.quote_outputs_remaining;
        let mut claim_outputs_remaining = self.claim_outputs_remaining;
        for index in 0..usize::from(page.execution_count) {
            let execution = replay_execution_plan(
                &self.distribution,
                page_id,
                page,
                candidate,
                root,
                config,
                batch,
                index,
            )?;
            quote_outputs_remaining = quote_outputs_remaining
                .checked_sub(execution.custody_effect.quote_credit_to_owner())
                .ok_or(Error::InsufficientCustody)?;
            for (credit, outputs) in execution
                .custody_effect
                .claim_credits_to_owner()
                .iter()
                .zip(claim_outputs_remaining.iter_mut())
            {
                *outputs = outputs
                    .checked_sub(*credit)
                    .ok_or(Error::InsufficientCustody)?;
            }
        }
        let settlement_reward_lamports = checked_candidate_reward(
            candidate.settlement_work_remaining,
            config.continuation_reward_lamports,
        )?;
        let cleanup_reward_lamports = checked_candidate_reward(
            candidate.cleanup_work_remaining,
            config.continuation_reward_lamports,
        )?;
        let open_page_children = candidate
            .open_page_children
            .checked_sub(1)
            .ok_or(Error::ArithmeticOverflow)?;
        candidate.settlement_work_remaining = candidate
            .settlement_work_remaining
            .checked_sub(settlement_reward_lamports)
            .ok_or(Error::InsufficientFunding)?;
        candidate.cleanup_work_remaining = candidate
            .cleanup_work_remaining
            .checked_sub(cleanup_reward_lamports)
            .ok_or(Error::InsufficientFunding)?;
        candidate.open_page_children = open_page_children;
        self.distribution = replay;
        self.quote_outputs_remaining = quote_outputs_remaining;
        self.claim_outputs_remaining = claim_outputs_remaining;
        Ok(SettlementPageResultV1 {
            execution_count: page.execution_count,
            quote_inventory_after: quote_outputs_remaining,
            claim_inventory_after: claim_outputs_remaining,
            settlement_reward_lamports,
            page_close: Some(CandidatePageCloseV1 {
                cleanup_reward_lamports,
                rent_credit_lamports: page_account_lamports,
                rent_beneficiary: candidate.submitter(),
            }),
        })
    }

    /// Finish only after the second replay and all physical custody are empty.
    #[allow(clippy::too_many_arguments)]
    pub fn finish(
        &mut self,
        candidate: &mut CandidateStateV1<N>,
        batch: &mut BatchRootV1,
        root: GeneralRootV1,
        config: GeneralConfigV1,
        claim_inventory: [u64; N],
        quote_inventory: u64,
        capitalization: CandidateCapitalizationV1,
    ) -> Result<u64> {
        candidate.validate_capitalization(capitalization)?;
        root.validate_authority(
            candidate.submission.market,
            candidate.submission.claim_basis_id,
            candidate.submission.generation,
            config,
        )?;
        self.require_active(candidate, batch, SettlementPhaseV1::DistributingOutputs)?;
        self.distribution.matches_candidate(*candidate)?;
        self.authenticate_inventory(claim_inventory, quote_inventory, &config)?;
        if claim_inventory.iter().any(|amount| *amount != 0)
            || quote_inventory != 0
            || self
                .claim_outputs_remaining
                .iter()
                .any(|amount| *amount != 0)
            || self.quote_outputs_remaining != 0
            || candidate.open_page_children != 0
        {
            return Err(Error::InsufficientCustody);
        }
        let mut next = *self;
        let mut next_batch = *batch;
        let mut next_candidate = *candidate;
        next.phase = SettlementPhaseV1::Finished;
        next_batch.phase = BatchPhase::Quiescent;
        let reward = next_candidate.consume_settlement(config)?;
        *self = next;
        *batch = next_batch;
        *candidate = next_candidate;
        Ok(reward)
    }

    /// Close the empty temporary cluster and discharge the exact batch child.
    #[allow(clippy::too_many_arguments)]
    pub fn close(
        self,
        candidate: &mut CandidateStateV1<N>,
        batch: &mut BatchRootV1,
        root: GeneralRootV1,
        config: GeneralConfigV1,
        claim_inventory: [u64; N],
        quote_inventory: u64,
        temporary_accounts: SettlementCloseObservationV1,
        capitalization: CandidateCapitalizationV1,
    ) -> Result<SettlementCloseV1> {
        candidate.validate_capitalization(capitalization)?;
        root.validate_authority(
            candidate.submission.market,
            candidate.submission.claim_basis_id,
            candidate.submission.generation,
            config,
        )?;
        if self.phase != SettlementPhaseV1::Finished
            || candidate.candidate_id != self.candidate_id
            || batch.phase != BatchPhase::Quiescent
            || batch.best_candidate_id != Some(self.candidate_id)
            || batch.open_settlement_children != 1
        {
            return Err(Error::NotQuiescent);
        }
        self.authenticate_inventory(claim_inventory, quote_inventory, &config)?;
        let mut rent_credit_lamports = 0u64;
        for (account_lamports, exact_rent_lamports) in temporary_accounts
            .account_lamports
            .iter()
            .zip(temporary_accounts.exact_rent_lamports.iter())
        {
            if account_lamports < exact_rent_lamports {
                return Err(Error::InsufficientFunding);
            }
            rent_credit_lamports = rent_credit_lamports
                .checked_add(*account_lamports)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        let mut next_candidate = *candidate;
        let reward = next_candidate.consume_settlement(config)?;
        if next_candidate.settlement_work_remaining != 0 {
            return Err(Error::FundingConservationMismatch);
        }
        batch.open_settlement_children = 0;
        *candidate = next_candidate;
        Ok(SettlementCloseV1 {
            continuation_reward_lamports: reward,
            rent_credit_lamports,
            rent_beneficiary: candidate.submitter(),
        })
    }

    /// Return the physical phase.
    pub const fn phase(self) -> SettlementPhaseV1 {
        self.phase
    }

    fn require_active(
        &self,
        candidate: &CandidateStateV1<N>,
        batch: &BatchRootV1,
        phase: SettlementPhaseV1,
    ) -> Result<()> {
        if self.phase != phase
            || candidate.phase != CandidatePhase::Considered
            || candidate.candidate_id != self.candidate_id
            || batch.phase != BatchPhase::Applying
            || batch.best_candidate_id != Some(self.candidate_id)
            || batch.open_settlement_children != 1
        {
            return Err(Error::CandidateNotSelected);
        }
        Ok(())
    }

    fn authenticate_inventory(
        &self,
        claim_inventory: [u64; N],
        quote_inventory: u64,
        config: &GeneralConfigV1,
    ) -> Result<()> {
        let (expected_claims, expected_quote) = self.expected_inventory(config)?;
        if claim_inventory != expected_claims || quote_inventory != expected_quote {
            return Err(Error::CustodyMismatch);
        }
        Ok(())
    }

    fn expected_inventory(&self, config: &GeneralConfigV1) -> Result<([u64; N], u64)> {
        let replay = match self.phase {
            SettlementPhaseV1::CollectingInputs => &self.collection,
            SettlementPhaseV1::DistributingOutputs | SettlementPhaseV1::Finished => {
                &self.distribution
            }
        };
        expected_settlement_inventory(
            self.phase,
            replay,
            &self.claim_outputs_remaining,
            self.quote_outputs_remaining,
            config,
        )
    }
}

fn expected_settlement_inventory<const N: usize>(
    phase: SettlementPhaseV1,
    replay: &SettlementReplayV1<N>,
    claim_outputs_remaining: &[u64; N],
    quote_outputs_remaining: u64,
    config: &GeneralConfigV1,
) -> Result<([u64; N], u64)> {
    match phase {
        SettlementPhaseV1::CollectingInputs => {
            let mut claims = [0u64; N];
            for ((output, net), inventory) in claim_outputs_remaining
                .iter()
                .zip(replay.net_coefficients.iter())
                .zip(claims.iter_mut())
            {
                let amount = i128::from(*output)
                    .checked_sub(*net)
                    .ok_or(Error::ArithmeticOverflow)?;
                *inventory = u64::try_from(amount).map_err(|_| Error::CustodyMismatch)?;
            }
            let signed_quote_numerator = replay
                .total_quote_debit_numerator
                .checked_neg()
                .and_then(|value| value.checked_sub(i128::from(replay.rounding_carry)))
                .ok_or(Error::ArithmeticOverflow)?;
            let scale = i128::from(config.price_scale);
            if signed_quote_numerator.rem_euclid(scale) != 0 {
                return Err(Error::NonCanonicalState);
            }
            let net_quote = signed_quote_numerator.div_euclid(scale);
            let quote = i128::from(quote_outputs_remaining)
                .checked_sub(net_quote)
                .ok_or(Error::ArithmeticOverflow)?;
            Ok((
                claims,
                u64::try_from(quote).map_err(|_| Error::CustodyMismatch)?,
            ))
        }
        SettlementPhaseV1::DistributingOutputs => {
            Ok((*claim_outputs_remaining, quote_outputs_remaining))
        }
        SettlementPhaseV1::Finished => Ok(([0; N], 0)),
    }
}

fn checked_candidate_reward(remaining: u64, reward: u64) -> Result<u64> {
    if reward == 0 || remaining < reward {
        return Err(Error::InsufficientFunding);
    }
    Ok(reward)
}

fn validate_materialization_conservation<const N: usize>(
    claims_before: [u64; N],
    quote_before: u64,
    claims_after: [u64; N],
    quote_after: u64,
    action: SettlementMaterializationActionV1,
) -> Result<()> {
    let quantity = match action {
        SettlementMaterializationActionV1::None => 0,
        SettlementMaterializationActionV1::Split(quantity)
        | SettlementMaterializationActionV1::Merge(quantity) => quantity,
    };
    for (before, after) in claims_before.iter().zip(claims_after.iter()) {
        let expected = match action {
            SettlementMaterializationActionV1::None => *before,
            SettlementMaterializationActionV1::Split(_) => before
                .checked_add(quantity)
                .ok_or(Error::ArithmeticOverflow)?,
            SettlementMaterializationActionV1::Merge(_) => before
                .checked_sub(quantity)
                .ok_or(Error::InsufficientCustody)?,
        };
        if expected != *after {
            return Err(Error::QuoteConservationMismatch);
        }
    }
    let expected_quote = match action {
        SettlementMaterializationActionV1::None => quote_before,
        SettlementMaterializationActionV1::Split(_) => quote_before
            .checked_sub(quantity)
            .ok_or(Error::InsufficientCustody)?,
        SettlementMaterializationActionV1::Merge(_) => quote_before
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?,
    };
    if expected_quote != quote_after {
        return Err(Error::QuoteConservationMismatch);
    }
    Ok(())
}

fn replay_page<const N: usize>(
    replay: &SettlementReplayV1<N>,
    page_id: ContentId,
    page: &CandidatePageV1<N>,
    candidate: &CandidateStateV1<N>,
    root: &GeneralRootV1,
    config: &GeneralConfigV1,
    batch: &BatchRootV1,
) -> Result<SettlementReplayV1<N>> {
    if page.page_index != replay.pages || replay.next_page_id != Some(page_id) {
        return Err(Error::CursorMismatch);
    }
    let count = usize::from(page.execution_count);
    if count == 0 || count > MAX_EXECUTIONS_PER_PAGE_V1 {
        return Err(Error::InvalidPageCount);
    }
    if page.executions.iter().take(count).any(Option::is_none)
        || page.executions.iter().skip(count).any(Option::is_some)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    let mut next = *replay;
    for execution in page.executions.iter().take(count).flatten() {
        replay_execution(&mut next, execution, candidate, root, config, batch)?;
    }
    next.pages = next.pages.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    next.next_page_id = page.next_page_id;
    Ok(next)
}

fn replay_execution<const N: usize>(
    replay: &mut SettlementReplayV1<N>,
    execution: &ExecutionV1<N>,
    candidate: &CandidateStateV1<N>,
    root: &GeneralRootV1,
    config: &GeneralConfigV1,
    batch: &BatchRootV1,
) -> Result<(OrderStateV1, SettlementReceiptV1<N>)> {
    validate_execution_binding(execution, root, config, batch)?;
    if replay
        .last_order_id
        .is_some_and(|last| execution.order.order_id <= last)
    {
        return Err(Error::NonCanonicalOrder);
    }
    let debit_per_lot = portfolio_dot(
        &execution.order.coefficients,
        &candidate.submission.prices,
        config.outcome_count,
    )?;
    if debit_per_lot > execution.order.max_quote_debit_per_lot_numerator {
        return Err(Error::LimitViolated);
    }
    let fill = i128::from(execution.fill_lots);
    let debit = debit_per_lot
        .checked_mul(fill)
        .ok_or(Error::ArithmeticOverflow)?;
    let combined = debit
        .checked_neg()
        .and_then(|value| value.checked_add(i128::from(replay.rounding_carry)))
        .ok_or(Error::ArithmeticOverflow)?;
    let scale = i128::from(config.price_scale);
    let quote_delta_atoms =
        i64::try_from(combined.div_euclid(scale)).map_err(|_| Error::TokenAmountOutOfRange)?;
    let carry_before = replay.rounding_carry;
    let carry_after =
        u64::try_from(combined.rem_euclid(scale)).map_err(|_| Error::ArithmeticOverflow)?;
    let mut outcome_deltas = [0i64; N];
    for ((receipt_delta, net), coefficient) in outcome_deltas
        .iter_mut()
        .zip(replay.net_coefficients.iter_mut())
        .zip(execution.order.coefficients.iter())
        .take(usize::from(config.outcome_count))
    {
        let delta = i128::from(*coefficient)
            .checked_mul(fill)
            .ok_or(Error::ArithmeticOverflow)?;
        *receipt_delta = i64::try_from(delta).map_err(|_| Error::TokenAmountOutOfRange)?;
        *net = net.checked_add(delta).ok_or(Error::ArithmeticOverflow)?;
    }
    let preference = execution
        .order
        .max_quote_debit_per_lot_numerator
        .checked_sub(debit_per_lot)
        .and_then(|value| value.checked_mul(fill))
        .ok_or(Error::ArithmeticOverflow)?;
    replay.score = replay
        .score
        .checked_add(u128::try_from(preference).map_err(|_| Error::ArithmeticOverflow)?)
        .ok_or(Error::ArithmeticOverflow)?;
    replay.total_quote_debit_numerator = replay
        .total_quote_debit_numerator
        .checked_add(debit)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut state = execution.order_state;
    state.consume(execution.order, execution.fill_lots)?;
    replay.rounding_carry = carry_after;
    replay.executions = replay
        .executions
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    replay.last_order_id = Some(execution.order.order_id);
    Ok((
        state,
        SettlementReceiptV1 {
            candidate_id: candidate.candidate_id,
            order_id: execution.order.order_id,
            owner: execution.order.owner,
            generation: config.generation,
            batch_sequence: batch.sequence,
            nonce: execution.order.nonce,
            fill_lots: execution.fill_lots,
            remaining_lots: state.remaining_lots,
            quote_delta_atoms,
            carry_before,
            carry_after,
            outcome_deltas,
            outcome_count: config.outcome_count,
        },
    ))
}

#[allow(clippy::too_many_arguments)]
fn replay_execution_plan<const N: usize>(
    replay: &SettlementReplayV1<N>,
    page_id: ContentId,
    page: &CandidatePageV1<N>,
    candidate: &CandidateStateV1<N>,
    root: &GeneralRootV1,
    config: &GeneralConfigV1,
    batch: &BatchRootV1,
    index: usize,
) -> Result<SettlementExecutionPlanV1<N>> {
    if page.page_index != replay.pages || replay.next_page_id != Some(page_id) {
        return Err(Error::CursorMismatch);
    }
    let count = usize::from(page.execution_count);
    if count == 0 || count > MAX_EXECUTIONS_PER_PAGE_V1 || index >= count {
        return Err(Error::InvalidPageCount);
    }
    if page.executions.iter().take(count).any(Option::is_none)
        || page.executions.iter().skip(count).any(Option::is_some)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    let mut next = *replay;
    for (current, execution) in page.executions.iter().take(index + 1).flatten().enumerate() {
        let (order_state, receipt) =
            replay_execution(&mut next, execution, candidate, root, config, batch)?;
        if current == index {
            return Ok(SettlementExecutionPlanV1 {
                order_state,
                receipt,
                custody_effect: custody_effect_from_receipt(&receipt)?,
            });
        }
    }
    Err(Error::InvalidPageCount)
}

fn custody_effect_from_receipt<const N: usize>(
    receipt: &SettlementReceiptV1<N>,
) -> Result<GeneralCustodyConsumptionV1<N>> {
    let (quote_debit_from_escrow, quote_credit_to_owner) =
        split_signed_amount(receipt.quote_delta_atoms)?;
    let mut claim_debits_from_custody = [0; N];
    let mut claim_credits_to_owner = [0; N];
    for ((delta, debit), credit) in receipt
        .outcome_deltas
        .iter()
        .zip(claim_debits_from_custody.iter_mut())
        .zip(claim_credits_to_owner.iter_mut())
    {
        let (next_debit, next_credit) = split_signed_amount(*delta)?;
        *debit = next_debit;
        *credit = next_credit;
    }
    Ok(GeneralCustodyConsumptionV1 {
        quote_debit_from_escrow,
        quote_credit_to_owner,
        claim_debits_from_custody,
        claim_credits_to_owner,
    })
}

/// Segregated prepaid General work compartments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FundingCompartment {
    /// Progress capital for expiry and convergence calls.
    Liveness = 0,
    /// Metered page verification and settlement work.
    Work = 1,
    /// Fixed successful-candidate or retirement bounties.
    Bounty = 2,
}

/// Immutable quote and mutable conservation state for General funding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralFundingV1 {
    capability_release_id: ContentId,
    committed: [u64; 3],
    remaining: [u64; 3],
    spent: [u64; 3],
    refunded: [u64; 3],
}

/// Auditable funding debit emitted to the adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingDebitV1 {
    /// Segregated source compartment.
    pub compartment: FundingCompartment,
    /// Exact prepaid principal consumed.
    pub amount: u64,
    /// Recipient identity authenticated by the adapter.
    pub recipient: ContentId,
}

/// Exact one-shot transfer plan from generic capability funding into an
/// activated General child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralFundingActivationV1 {
    capability_funding_after: FundingStateV1,
    capability_funding_derivation: CapabilityFundingDerivationV1,
    general_funding: GeneralFundingV1,
    rent_lamports: u64,
    creation_lamports: u64,
    general_lamports: u64,
}

impl GeneralFundingActivationV1 {
    /// Return the capability ledger after every quoted compartment was released.
    pub const fn capability_funding_after(self) -> FundingStateV1 {
        self.capability_funding_after
    }

    /// Return the reusable capability-owned physical source derivation.
    pub const fn capability_funding_derivation(self) -> CapabilityFundingDerivationV1 {
        self.capability_funding_derivation
    }

    /// Return newly founded General-owned liveness/work/bounty funding.
    pub const fn general_funding(self) -> GeneralFundingV1 {
        self.general_funding
    }

    /// Return exact child-rent principal released during activation.
    pub const fn rent_lamports(self) -> u64 {
        self.rent_lamports
    }

    /// Return exact physical-creation principal released during activation.
    pub const fn creation_lamports(self) -> u64 {
        self.creation_lamports
    }

    /// Return exact principal transferred into the General funding child.
    pub const fn general_lamports(self) -> u64 {
        self.general_lamports
    }
}

/// Authenticate one manifest entry as the closed reviewed General V1 release.
pub fn validate_general_capability_entry_v1(
    entry: CapabilityEntryV1,
    config_id: ContentId,
    config: GeneralConfigV1,
) -> Result<()> {
    if entry.kind_id().to_bytes() != GENERAL_CAPABILITY_KIND_ID_V1.to_bytes()
        || entry.release_id().to_bytes() != GENERAL_CAPABILITY_RELEASE_ID_V1.to_bytes()
        || entry.child_schema_id().to_bytes() != GENERAL_CHILD_SCHEMA_ID_V1.to_bytes()
        || entry.child_derivation_id().to_bytes() != GENERAL_CHILD_DERIVATION_ID_V1.to_bytes()
        || entry.config_id().to_bytes() != config_id.to_bytes()
        || entry.capacity_profile_id().to_bytes() != config.capacity_profile_id.to_bytes()
        || config.capability_release_id != GENERAL_CAPABILITY_RELEASE_ID_V1
    {
        return Err(Error::UnrecognizedCapability);
    }
    Ok(())
}

impl GeneralFundingV1 {
    /// Activate the exact manifest-selected General entry and move all mapped
    /// principal out of the generic capability ledger in one atomic plan.
    ///
    /// The immutable quote is the only amount authority: service becomes
    /// General liveness, work remains work, and bounty remains bounty.
    /// Provider and liquidity principal are forbidden. No compartment amount
    /// is accepted from the caller.
    #[allow(clippy::too_many_arguments)]
    pub fn activate_from_capability(
        market: [u8; 32],
        config_id: ContentId,
        config: GeneralConfigV1,
        manifest_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        mut capability_funding: FundingStateV1,
        capability_custody: FundingCustodyObservationV1,
        current_slot: u64,
    ) -> Result<GeneralFundingActivationV1> {
        let capability_manifest_id =
            dclutch_capability_contract::ContentId::new(manifest_id.to_bytes())
                .map_err(|_| Error::CapabilityFundingMismatch)?;
        let capability_funding_derivation = CapabilityFundingDerivationV1::new(
            market,
            config.generation,
            capability_manifest_id,
            manifest,
            capability_funding,
        )
        .map_err(|_| Error::CapabilityFundingMismatch)?;
        capability_funding
            .validate_against(capability_manifest_id, manifest, capability_custody)
            .map_err(|_| Error::CapabilityFundingMismatch)?;
        let entry = manifest
            .entry(capability_funding.entry_index())
            .map_err(|_| Error::CapabilityFundingMismatch)?;
        validate_general_capability_entry_v1(entry, config_id, config)?;
        let quote = entry.funding_quote();
        validate_general_funding_quote_v1(quote)?;

        let activation = capability_funding
            .activate(
                capability_manifest_id,
                manifest,
                capability_custody,
                current_slot,
            )
            .map_err(|_| Error::CapabilityFundingMismatch)?;
        let amounts = quote.amounts();
        let remaining = capability_funding.remaining();
        let released = capability_funding.released();
        if remaining.service() != amounts.service()
            || remaining.work() != amounts.work()
            || remaining.bounty() != amounts.bounty()
            || released.service().amount() != 0
            || released.work().amount() != 0
            || released.bounty().amount() != 0
        {
            return Err(Error::CapabilityFundingMismatch);
        }

        release_capability_compartment(
            &mut capability_funding,
            capability_manifest_id,
            manifest,
            capability_custody.exact_state_rent_lamports(),
            CapabilityFundingCompartment::Service,
            amounts.service().amount(),
        )?;
        release_capability_compartment(
            &mut capability_funding,
            capability_manifest_id,
            manifest,
            capability_custody.exact_state_rent_lamports(),
            CapabilityFundingCompartment::Work,
            amounts.work().amount(),
        )?;
        release_capability_compartment(
            &mut capability_funding,
            capability_manifest_id,
            manifest,
            capability_custody.exact_state_rent_lamports(),
            CapabilityFundingCompartment::Bounty,
            amounts.bounty().amount(),
        )?;
        let closed_custody = FundingCustodyObservationV1::native_only(
            capability_custody.exact_state_rent_lamports(),
            capability_custody.exact_state_rent_lamports(),
        )
        .map_err(|_| Error::CapabilityFundingMismatch)?;
        capability_funding
            .validate_against(capability_manifest_id, manifest, closed_custody)
            .map_err(|_| Error::CapabilityFundingMismatch)?;

        let general_lamports = amounts
            .service()
            .amount()
            .checked_add(amounts.work().amount())
            .and_then(|value| value.checked_add(amounts.bounty().amount()))
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(GeneralFundingActivationV1 {
            capability_funding_after: capability_funding,
            capability_funding_derivation,
            general_funding: Self::founding(
                GENERAL_CAPABILITY_RELEASE_ID_V1,
                amounts.service().amount(),
                amounts.work().amount(),
                amounts.bounty().amount(),
            ),
            rent_lamports: activation.rent_lamports(),
            creation_lamports: activation.creation_lamports(),
            general_lamports,
        })
    }

    /// Decode one exact-width canonical segregated General funding state.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        exact_len(bytes, GENERAL_FUNDING_BYTES)?;
        if array::<8>(bytes, 0)? != GENERAL_FUNDING_MAGIC {
            return Err(Error::InvalidMagic);
        }
        validate_record_header(bytes)?;
        require_zero(bytes, 12, 4)?;
        let funding = Self {
            capability_release_id: read_id(bytes, 16)?,
            committed: [
                read_u64(bytes, 48)?,
                read_u64(bytes, 56)?,
                read_u64(bytes, 64)?,
            ],
            remaining: [
                read_u64(bytes, 72)?,
                read_u64(bytes, 80)?,
                read_u64(bytes, 88)?,
            ],
            spent: [
                read_u64(bytes, 96)?,
                read_u64(bytes, 104)?,
                read_u64(bytes, 112)?,
            ],
            refunded: [
                read_u64(bytes, 120)?,
                read_u64(bytes, 128)?,
                read_u64(bytes, 136)?,
            ],
        };
        funding.validate()?;
        Ok(funding)
    }

    /// Encode one exact-width canonical segregated General funding state.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        exact_len(out, GENERAL_FUNDING_BYTES)?;
        self.validate()?;
        out.fill(0);
        put(out, 0, &GENERAL_FUNDING_MAGIC);
        put(out, 8, &SCHEMA_V1.to_le_bytes());
        put(out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(out, 16, self.capability_release_id.as_bytes());
        for (value, offset) in self.committed.iter().zip([48usize, 56, 64]) {
            put(out, offset, &value.to_le_bytes());
        }
        for (value, offset) in self.remaining.iter().zip([72usize, 80, 88]) {
            put(out, offset, &value.to_le_bytes());
        }
        for (value, offset) in self.spent.iter().zip([96usize, 104, 112]) {
            put(out, offset, &value.to_le_bytes());
        }
        for (value, offset) in self.refunded.iter().zip([120usize, 128, 136]) {
            put(out, offset, &value.to_le_bytes());
        }
        Ok(())
    }

    /// Found segregated prepaid funding. Hoard and future fees are not inputs.
    pub const fn founding(
        capability_release_id: ContentId,
        liveness: u64,
        work: u64,
        bounty: u64,
    ) -> Self {
        Self {
            capability_release_id,
            committed: [liveness, work, bounty],
            remaining: [liveness, work, bounty],
            spent: [0; 3],
            refunded: [0; 3],
        }
    }

    /// Consume present principal from exactly one compartment.
    pub fn debit(
        &mut self,
        compartment: FundingCompartment,
        amount: u64,
        recipient: ContentId,
    ) -> Result<FundingDebitV1> {
        self.consume(compartment, amount)?;
        Ok(FundingDebitV1 {
            compartment,
            amount,
            recipient,
        })
    }

    fn consume(&mut self, compartment: FundingCompartment, amount: u64) -> Result<()> {
        if amount == 0 {
            return Err(Error::ZeroFundingDebit);
        }
        let index = funding_index(compartment);
        let remaining = *self.remaining.get(index).ok_or(Error::InvalidLength)?;
        let next_remaining = remaining
            .checked_sub(amount)
            .ok_or(Error::InsufficientFunding)?;
        let spent = *self.spent.get(index).ok_or(Error::InvalidLength)?;
        let next_spent = spent.checked_add(amount).ok_or(Error::ArithmeticOverflow)?;
        let target_remaining = self.remaining.get_mut(index).ok_or(Error::InvalidLength)?;
        *target_remaining = next_remaining;
        let target_spent = self.spent.get_mut(index).ok_or(Error::InvalidLength)?;
        *target_spent = next_spent;
        self.validate()?;
        Ok(())
    }

    /// Refund all unspent compartments only after General is terminal.
    pub fn refund_terminal(&mut self, phase: GeneralPhase) -> Result<[u64; 3]> {
        if phase != GeneralPhase::Terminal {
            return Err(Error::NotQuiescent);
        }
        let refund = self.remaining;
        for index in 0..3 {
            let value = *refund.get(index).ok_or(Error::InvalidLength)?;
            let remaining = self.remaining.get_mut(index).ok_or(Error::InvalidLength)?;
            *remaining = 0;
            let refunded = self.refunded.get_mut(index).ok_or(Error::InvalidLength)?;
            *refunded = refunded
                .checked_add(value)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        self.validate()?;
        Ok(refund)
    }

    /// Prove immutable compartment conservation.
    pub fn validate(self) -> Result<()> {
        if self.refunded.iter().any(|amount| *amount != 0)
            && self.remaining.iter().any(|amount| *amount != 0)
        {
            return Err(Error::NonCanonicalState);
        }
        for index in 0..3 {
            let remaining = *self.remaining.get(index).ok_or(Error::InvalidLength)?;
            let spent = *self.spent.get(index).ok_or(Error::InvalidLength)?;
            let refunded = *self.refunded.get(index).ok_or(Error::InvalidLength)?;
            let observed = remaining
                .checked_add(spent)
                .and_then(|value| value.checked_add(refunded))
                .ok_or(Error::ArithmeticOverflow)?;
            if observed != *self.committed.get(index).ok_or(Error::InvalidLength)? {
                return Err(Error::FundingConservationMismatch);
            }
        }
        Ok(())
    }

    /// Return true once all prepaid principal has been spent or refunded.
    pub fn is_discharged(self) -> bool {
        self.remaining.iter().all(|amount| *amount == 0)
    }

    /// Return the capability release whose manifest quote funded this state.
    pub const fn capability_release_id(self) -> ContentId {
        self.capability_release_id
    }

    /// Return exact remaining prepaid principal in one compartment.
    pub fn remaining(self, compartment: FundingCompartment) -> Result<u64> {
        self.remaining
            .get(funding_index(compartment))
            .copied()
            .ok_or(Error::InvalidLength)
    }

    /// Return the checked native-lamport total still held for all compartments.
    pub fn remaining_lamports(self) -> Result<u64> {
        self.remaining
            .iter()
            .try_fold(0u64, |total, value| total.checked_add(*value))
            .ok_or(Error::ArithmeticOverflow)
    }
}

fn validate_execution_binding<const N: usize>(
    execution: &ExecutionV1<N>,
    root: &GeneralRootV1,
    config: &GeneralConfigV1,
    batch: &BatchRootV1,
) -> Result<()> {
    root.validate_authority(
        execution.order.market,
        execution.order.claim_basis_id,
        execution.order.generation,
        *config,
    )?;
    if execution.order.batch_sequence != batch.sequence {
        return Err(Error::OrderBindingMismatch);
    }
    if execution.order.valid_until_slot < batch.settlement_close {
        return Err(Error::OrderExpired);
    }
    execution
        .order_state
        .validate_snapshot(execution.order, execution.fill_lots)
}

fn validate_prices<const N: usize>(
    prices: &[u64; N],
    outcome_count: u16,
    price_scale: u64,
) -> Result<()> {
    let width = usize::from(outcome_count);
    if width != N || !(2..=MAX_OUTCOMES_V1).contains(&width) {
        return Err(Error::InvalidOutcomeCount);
    }
    let mut sum = 0u64;
    for price in prices {
        sum = sum.checked_add(*price).ok_or(Error::ArithmeticOverflow)?;
    }
    if sum != price_scale {
        return Err(Error::InvalidSimplexPrice);
    }
    Ok(())
}

fn validate_portfolio<const N: usize>(
    coefficients: &[i64; N],
    outcome_count: u16,
    require_nonzero: bool,
) -> Result<()> {
    let width = usize::from(outcome_count);
    if width != N || !(2..=MAX_OUTCOMES_V1).contains(&width) {
        return Err(Error::InvalidOutcomeCount);
    }
    if require_nonzero && coefficients.iter().all(|coefficient| *coefficient == 0) {
        return Err(Error::EmptyPortfolio);
    }
    Ok(())
}

fn quote_reserve(max_debit_per_lot: i128, max_lots: u64, price_scale: u64) -> Result<u64> {
    if price_scale == 0 {
        return Err(Error::ZeroPriceScale);
    }
    if max_debit_per_lot <= 0 {
        return Ok(0);
    }
    let debit = u128::try_from(max_debit_per_lot).map_err(|_| Error::ArithmeticOverflow)?;
    let numerator = debit
        .checked_mul(u128::from(max_lots))
        .ok_or(Error::ArithmeticOverflow)?;
    let scale = u128::from(price_scale);
    let quotient = numerator / scale;
    let rounded = if numerator % scale == 0 {
        quotient
    } else {
        quotient.checked_add(1).ok_or(Error::ArithmeticOverflow)?
    };
    u64::try_from(rounded).map_err(|_| Error::TokenAmountOutOfRange)
}

fn claim_reserve<const N: usize>(coefficients: &[i64; N], max_lots: u64) -> Result<[u64; N]> {
    let mut reserve = [0u64; N];
    for (coefficient, target) in coefficients.iter().zip(reserve.iter_mut()) {
        if *coefficient < 0 {
            let magnitude = i128::from(*coefficient)
                .checked_neg()
                .ok_or(Error::ArithmeticOverflow)?;
            let amount = magnitude
                .checked_mul(i128::from(max_lots))
                .ok_or(Error::ArithmeticOverflow)?;
            *target = u64::try_from(amount).map_err(|_| Error::TokenAmountOutOfRange)?;
        }
    }
    Ok(reserve)
}

fn split_signed_amount(delta: i64) -> Result<(u64, u64)> {
    if delta < 0 {
        let debit = i128::from(delta)
            .checked_neg()
            .ok_or(Error::ArithmeticOverflow)?;
        Ok((
            u64::try_from(debit).map_err(|_| Error::TokenAmountOutOfRange)?,
            0,
        ))
    } else {
        Ok((
            0,
            u64::try_from(delta).map_err(|_| Error::TokenAmountOutOfRange)?,
        ))
    }
}

fn release_capability_compartment(
    funding: &mut FundingStateV1,
    manifest_id: dclutch_capability_contract::ContentId,
    manifest: CapabilityManifestV1<'_>,
    exact_state_rent_lamports: u64,
    compartment: CapabilityFundingCompartment,
    amount: u64,
) -> Result<()> {
    let state_account_lamports = exact_state_rent_lamports
        .checked_add(funding.remaining().native_lamports_total())
        .ok_or(Error::ArithmeticOverflow)?;
    let custody =
        FundingCustodyObservationV1::native_only(state_account_lamports, exact_state_rent_lamports)
            .map_err(|_| Error::CapabilityFundingMismatch)?;
    let release = funding
        .release(manifest_id, manifest, custody, compartment, amount)
        .map_err(|_| Error::CapabilityFundingMismatch)?;
    if release.compartment() != compartment
        || release.asset_class() != FundingAssetClassV1::NativeLamports
        || release.amount() != amount
    {
        return Err(Error::CapabilityFundingMismatch);
    }
    Ok(())
}

fn validate_general_funding_quote_v1(
    quote: dclutch_capability_contract::FundingQuoteV1,
) -> Result<()> {
    let amounts = quote.amounts();
    let native = [
        amounts.rent(),
        amounts.creation(),
        amounts.work(),
        amounts.bounty(),
        amounts.service(),
    ];
    if quote.realm_collateral().is_some()
        || amounts.realm_collateral_total() != 0
        || native.iter().any(|allocation| {
            allocation.asset_class() != FundingAssetClassV1::NativeLamports
                || allocation.amount() == 0
        })
        || amounts.provider().asset_class() != FundingAssetClassV1::NotApplicable
        || amounts.provider().amount() != 0
        || amounts.liquidity().asset_class() != FundingAssetClassV1::NotApplicable
        || amounts.liquidity().amount() != 0
    {
        return Err(Error::ExtraneousCapabilityFunding);
    }
    Ok(())
}

fn portfolio_dot<const N: usize>(
    coefficients: &[i64; N],
    prices: &[u64; N],
    outcome_count: u16,
) -> Result<i128> {
    if usize::from(outcome_count) != N {
        return Err(Error::InvalidOutcomeCount);
    }
    let mut total = 0i128;
    for (coefficient, price) in coefficients.iter().zip(prices.iter()) {
        let term = i128::from(*coefficient)
            .checked_mul(i128::from(*price))
            .ok_or(Error::ArithmeticOverflow)?;
        total = total.checked_add(term).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(total)
}

fn validate_width(width: usize) -> Result<()> {
    if !(2..=MAX_OUTCOMES_V1).contains(&width) {
        return Err(Error::InvalidOutcomeCount);
    }
    Ok(())
}

fn validate_record_header(bytes: &[u8]) -> Result<()> {
    if read_u16(bytes, 8)? != SCHEMA_V1 || read_u16(bytes, 10)? != ARTIFACT_PROFILE_V1 {
        return Err(Error::UnsupportedSchema);
    }
    Ok(())
}

const fn general_phase_tag(phase: GeneralPhase) -> u8 {
    match phase {
        GeneralPhase::Active => 0,
        GeneralPhase::Quiescing => 1,
        GeneralPhase::Terminal => 2,
        GeneralPhase::Retired => 3,
    }
}

fn decode_general_phase(tag: u8) -> Result<GeneralPhase> {
    match tag {
        0 => Ok(GeneralPhase::Active),
        1 => Ok(GeneralPhase::Quiescing),
        2 => Ok(GeneralPhase::Terminal),
        3 => Ok(GeneralPhase::Retired),
        _ => Err(Error::InvalidPhase),
    }
}

const fn order_phase_tag(phase: OrderPhase) -> u8 {
    match phase {
        OrderPhase::Open => 0,
        OrderPhase::Cancelled => 1,
        OrderPhase::Consumed => 2,
        OrderPhase::Released => 3,
    }
}

fn decode_order_phase(tag: u8) -> Result<OrderPhase> {
    match tag {
        0 => Ok(OrderPhase::Open),
        1 => Ok(OrderPhase::Cancelled),
        2 => Ok(OrderPhase::Consumed),
        3 => Ok(OrderPhase::Released),
        _ => Err(Error::InvalidPhase),
    }
}

const fn candidate_phase_tag(phase: CandidatePhase) -> u8 {
    match phase {
        CandidatePhase::Submitted => 0,
        CandidatePhase::Verifying => 1,
        CandidatePhase::Valid => 2,
        CandidatePhase::Considered => 3,
        CandidatePhase::Rejected => 4,
    }
}

fn decode_candidate_phase(tag: u8) -> Result<CandidatePhase> {
    match tag {
        0 => Ok(CandidatePhase::Submitted),
        1 => Ok(CandidatePhase::Verifying),
        2 => Ok(CandidatePhase::Valid),
        3 => Ok(CandidatePhase::Considered),
        4 => Ok(CandidatePhase::Rejected),
        _ => Err(Error::InvalidPhase),
    }
}

const fn batch_phase_tag(phase: BatchPhase) -> u8 {
    match phase {
        BatchPhase::Collecting => 0,
        BatchPhase::Selecting => 1,
        BatchPhase::Settling => 2,
        BatchPhase::Applying => 3,
        BatchPhase::Quiescent => 4,
        BatchPhase::Retired => 5,
    }
}

fn decode_batch_phase(tag: u8) -> Result<BatchPhase> {
    match tag {
        0 => Ok(BatchPhase::Collecting),
        1 => Ok(BatchPhase::Selecting),
        2 => Ok(BatchPhase::Settling),
        3 => Ok(BatchPhase::Applying),
        4 => Ok(BatchPhase::Quiescent),
        5 => Ok(BatchPhase::Retired),
        _ => Err(Error::InvalidPhase),
    }
}

const fn settlement_phase_tag(phase: SettlementPhaseV1) -> u8 {
    match phase {
        SettlementPhaseV1::CollectingInputs => 0,
        SettlementPhaseV1::DistributingOutputs => 1,
        SettlementPhaseV1::Finished => 2,
    }
}

fn decode_settlement_phase(tag: u8) -> Result<SettlementPhaseV1> {
    match tag {
        0 => Ok(SettlementPhaseV1::CollectingInputs),
        1 => Ok(SettlementPhaseV1::DistributingOutputs),
        2 => Ok(SettlementPhaseV1::Finished),
        _ => Err(Error::InvalidPhase),
    }
}

fn decode_optional_id(
    bytes: &[u8],
    flag_offset: usize,
    id_offset: usize,
) -> Result<Option<ContentId>> {
    let identifier = array::<CONTENT_ID_BYTES>(bytes, id_offset)?;
    match read_u8(bytes, flag_offset)? {
        0 => {
            if identifier.iter().any(|byte| *byte != 0) {
                return Err(Error::NonCanonicalReservedBytes);
            }
            Ok(None)
        }
        1 => Ok(Some(ContentId::new(identifier)?)),
        _ => Err(Error::NonCanonicalState),
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_replay_base<const N: usize>(
    out: &mut [u8],
    replay: SettlementReplayV1<N>,
    transcript_offset: usize,
    pages_offset: usize,
    executions_offset: usize,
    last_flag_offset: usize,
    last_id_offset: usize,
    carry_offset: usize,
    debit_offset: usize,
    score_offset: usize,
) {
    if let Some(next_page_id) = replay.next_page_id {
        put(out, transcript_offset, next_page_id.as_bytes());
    }
    put(out, pages_offset, &replay.pages.to_le_bytes());
    put(out, executions_offset, &replay.executions.to_le_bytes());
    if let Some(last_order_id) = replay.last_order_id {
        put(out, last_flag_offset, &[1]);
        put(out, last_id_offset, last_order_id.as_bytes());
    }
    put(out, carry_offset, &replay.rounding_carry.to_le_bytes());
    put(
        out,
        debit_offset,
        &replay.total_quote_debit_numerator.to_le_bytes(),
    );
    put(out, score_offset, &replay.score.to_le_bytes());
}

const fn funding_index(compartment: FundingCompartment) -> usize {
    match compartment {
        FundingCompartment::Liveness => 0,
        FundingCompartment::Work => 1,
        FundingCompartment::Bounty => 2,
    }
}

fn validate_instruction_header<const N: usize>(bytes: &[u8]) -> Result<()> {
    if bytes.len() < GENERAL_INSTRUCTION_HEADER_BYTES {
        return Err(Error::InvalidLength);
    }
    validate_width(N)?;
    if array::<8>(bytes, 0)? != GENERAL_INSTRUCTION_MAGIC_V1 {
        return Err(Error::InvalidMagic);
    }
    if read_u16(bytes, 8)? != GENERAL_INSTRUCTION_SCHEMA_V1 {
        return Err(Error::UnsupportedSchema);
    }
    if usize::from(read_u8(bytes, 11)?) != N {
        return Err(Error::InvalidOutcomeCount);
    }
    require_zero(bytes, 12, 4)
}

fn require_instruction_tag<const N: usize>(
    bytes: &[u8],
    expected: GeneralInstructionTagV1,
) -> Result<()> {
    if GeneralInstructionV1::<N>::decode_tag(bytes)? != expected {
        return Err(Error::UnknownAction);
    }
    Ok(())
}

fn encode_batch_replay(out: &mut [u8], replay: GeneralBatchReplayV1) {
    put(
        out,
        GENERAL_INSTRUCTION_HEADER_BYTES,
        &replay.generation.to_le_bytes(),
    );
    put(
        out,
        GENERAL_INSTRUCTION_HEADER_BYTES + 8,
        &replay.batch_sequence.to_le_bytes(),
    );
}

fn exact_len(bytes: &[u8], expected: usize) -> Result<()> {
    if bytes.len() != expected {
        return Err(Error::InvalidLength);
    }
    Ok(())
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn subslice(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    let end = offset.checked_add(len).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn subslice_mut(bytes: &mut [u8], offset: usize, len: usize) -> Result<&mut [u8]> {
    let end = offset.checked_add(len).ok_or(Error::InvalidLength)?;
    bytes.get_mut(offset..end).ok_or(Error::InvalidLength)
}

fn vector_offset(base: usize, index: usize, element_bytes: usize) -> Result<usize> {
    index
        .checked_mul(element_bytes)
        .and_then(|part| base.checked_add(part))
        .ok_or(Error::ArithmeticOverflow)
}

fn read_id(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(array(bytes, offset)?)
}

fn read_zeroable_id(bytes: &[u8], offset: usize) -> Result<Option<ContentId>> {
    let identifier = array::<CONTENT_ID_BYTES>(bytes, offset)?;
    if identifier.iter().all(|byte| *byte == 0) {
        Ok(None)
    } else {
        Ok(Some(ContentId::new(identifier)?))
    }
}

fn read_owner_key(bytes: &[u8], offset: usize) -> Result<OwnerKeyV1> {
    OwnerKeyV1::new(array(bytes, offset)?)
}

fn require_nonzero_key(bytes: &[u8; 32]) -> Result<()> {
    if bytes.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentifier)
    } else {
        Ok(())
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}

fn read_u8(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(bytes, offset)?))
}

fn read_u128(bytes: &[u8], offset: usize) -> Result<u128> {
    Ok(u128::from_le_bytes(array(bytes, offset)?))
}

fn read_i64(bytes: &[u8], offset: usize) -> Result<i64> {
    Ok(i64::from_le_bytes(array(bytes, offset)?))
}

fn read_i128(bytes: &[u8], offset: usize) -> Result<i128> {
    Ok(i128::from_le_bytes(array(bytes, offset)?))
}

fn require_zero(bytes: &[u8], offset: usize, len: usize) -> Result<()> {
    let end = offset.checked_add(len).ok_or(Error::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset.saturating_add(value.len())) {
        target.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests;
