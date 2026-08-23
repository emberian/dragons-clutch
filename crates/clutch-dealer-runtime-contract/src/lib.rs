// SPDX-License-Identifier: AGPL-3.0-or-later
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Fixed-layout semantic contract for the proposed covered-dealer runtime.
//!
//! This crate is deliberately disabled and Solana-free. It defines canonical
//! bytes, content identities, PDA seed preimages, and local consistency checks;
//! it does not allocate global account tags or intents and it performs no
//! account, Clock, signature, token, CPI, or transfer operation.
//!
//! `DealerStateV1` never persists cash, Egg balances, or per-order settlement
//! allocations. The separately authenticated `DealerFacilityPositionV1` is the
//! sole long-lived pool asset owner while idle. During a lease, SettlementPot
//! is the sole transient selected-leg custody owner and its custody is derived
//! from exact aggregate conservation facts rather than mirrored balance fields.

mod budget;
mod codec;
mod facility;
mod fee_bindings;
mod funding_dependencies;
mod lease;
mod lease_v2;
mod lp_funding;
mod lp_page;
mod pda;
mod policy;
mod pot;
mod pot_v2;
mod rent;
mod root_tombstone;
mod replay;
mod state;
mod state_v2;
mod transitions;

pub use budget::*;
pub use facility::*;
pub use fee_bindings::*;
pub use funding_dependencies::*;
pub use lease::*;
pub use lease_v2::*;
pub use lp_funding::*;
pub use lp_page::*;
pub use pda::*;
pub use policy::*;
pub use pot::*;
pub use pot_v2::*;
pub use rent::*;
pub use root_tombstone::*;
pub use replay::*;
pub use state::*;
pub use state_v2::*;
pub use transitions::*;

use sha2::{Digest, Sha256};

/// Width of every canonical identity.
pub const ID_BYTES: usize = 32;
/// Largest native outcome basis admitted by the selected relation.
pub const MAX_OUTCOMES: usize = 16;
/// Largest exact atom amount admitted by the selected research model.
pub const MAX_ATOMS: u64 = 1_000_000_000_000;
/// Largest exact initial-price denominator admitted by the selected model.
pub const MAX_PRICE_DENOMINATOR: u64 = 1_000_000_000;
/// Entries held by one independently funded LP page.
pub const LP_ENTRIES_PER_PAGE: usize = 16;
/// Hard semantic bound on pages in one dealer graph.
pub const MAX_LP_PAGES: u32 = 4_096;
/// Largest canonical dealer-row set admitted by the selected RelationV2 seam.
pub const MAX_SETTLEMENT_ROWS: u16 = 64;
/// Sentinel for the end of the canonical LP-page chain.
pub const NO_NEXT_LP_PAGE: u32 = u32::MAX;

/// Exact content domain for `DealerPolicyV1`.
pub const DEALER_POLICY_CONTENT_DOMAIN_V1: &[u8] = b"dragons-clutch/dealer-runtime/policy/v1\0";
/// Exact content domain for `DealerFacilityGenesisV1`.
pub const DEALER_FACILITY_GENESIS_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/facility-genesis/v1\0";
/// Exact content domain for `FacilityPositionBindingV1`.
pub const FACILITY_POSITION_BINDING_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/facility-position-binding/v1\0";
/// Exact content domain for `DealerFacilityPositionV1`.
pub const DEALER_FACILITY_POSITION_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/facility-position/v1\0";
/// Exact content domain for `DealerRootTombstoneV1`.
pub const DEALER_ROOT_TOMBSTONE_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/root-tombstone/v1\0";
/// Exact content domain for `DealerLivenessScheduleV1`.
pub const DEALER_LIVENESS_SCHEDULE_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/liveness-schedule/v1\0";
/// Exact content domain for `DealerFundedBudgetDependenciesV1`.
pub const DEALER_FUNDED_DEPENDENCIES_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/funded-dependencies/v1\0";
/// Exact content domain for counted, rent-owned funded dependencies.
pub const DEALER_FUNDED_DEPENDENCIES_CONTENT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/dealer-runtime/funded-dependencies/v2\0";
/// Transcript domain for an authenticated external liveness bundle projection.
pub const DEALER_RUNTIME_LIVENESS_BINDING_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/runtime-liveness-binding/v1\0";
/// Content domain for one typed successful Dealer-action liveness receipt.
pub const DEALER_ACTION_LIVENESS_RECEIPT_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/action-liveness-receipt/v1\0";
/// Initial domain for the canonical sealed LP page-set fold.
pub const DEALER_LP_PAGE_SET_INIT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/lp-page-set/init/v1\0";
/// Per-page domain for the canonical sealed LP page-set fold.
pub const DEALER_LP_PAGE_SET_STEP_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/lp-page-set/step/v1\0";
/// Final domain for the canonical sealed LP page-set fold.
pub const DEALER_LP_PAGE_SET_FINAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/lp-page-set/final/v1\0";
/// Exact content domain for `DealerStateV1`.
pub const DEALER_STATE_CONTENT_DOMAIN_V1: &[u8] = b"dragons-clutch/dealer-runtime/state/v1\0";
/// Exact content domain for authoritative `DealerStateV2`.
pub const DEALER_STATE_CONTENT_DOMAIN_V2: &[u8] = b"dragons-clutch/dealer-runtime/state/v2\0";
/// Exact content domain for `LpPageV1`.
pub const LP_PAGE_CONTENT_DOMAIN_V1: &[u8] = b"dragons-clutch/dealer-runtime/lp-page/v1\0";
/// Exact content domain for `DealerLeaseV1`.
pub const DEALER_LEASE_CONTENT_DOMAIN_V1: &[u8] = b"dragons-clutch/dealer-runtime/lease/v1\0";
/// Exact content domain for external-liveness `DealerLeaseV2`.
pub const DEALER_LEASE_CONTENT_DOMAIN_V2: &[u8] = b"dragons-clutch/dealer-runtime/lease/v2\0";
/// Exact content domain for `SettlementPotV1`.
pub const SETTLEMENT_POT_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/settlement-pot/v1\0";
/// Exact content domain for owner-netted `SettlementPotV2`.
pub const SETTLEMENT_POT_CONTENT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/dealer-runtime/settlement-pot/v2\0";
/// Exact content domain for `DealerRootTombstoneV2`.
pub const DEALER_ROOT_TOMBSTONE_CONTENT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/dealer-runtime/root-tombstone/v2\0";
/// Transcript domain for one selected owner-netted fee record projection.
pub const DEALER_SELECTED_FEE_BINDING_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/selected-fee-binding/v1\0";
/// Transcript domain for candidate-wide completed fee settlement evidence.
pub const DEALER_CANDIDATE_FEE_SETTLEMENT_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/candidate-fee-settlement/v1\0";
/// Transcript domain for an owner-netted selected-record abort/close receipt.
pub const DEALER_SELECTED_FEE_ABORT_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/selected-fee-abort/v1\0";
/// Exact content domain for `FeeBudgetV1`.
pub const FEE_BUDGET_CONTENT_DOMAIN_V1: &[u8] = b"dragons-clutch/dealer-runtime/fee-budget/v1\0";
/// Exact content domain for `LivenessBudgetV1`.
pub const LIVENESS_BUDGET_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/liveness-budget/v1\0";

/// Full-width opaque identity supplied or recomputed at an adapter boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Id([u8; ID_BYTES]);

impl Id {
    /// Canonical inactive padding identity.
    pub const ZERO: Self = Self([0; ID_BYTES]);

    /// Construct an identity from exact bytes without claiming authentication.
    pub const fn from_bytes(bytes: [u8; ID_BYTES]) -> Self {
        Self(bytes)
    }

    /// Return the exact identity bytes.
    pub const fn bytes(self) -> [u8; ID_BYTES] {
        self.0
    }

    /// Whether this is the all-zero identity reserved for padding.
    pub fn is_zero(self) -> bool {
        self.0 == [0; ID_BYTES]
    }

    pub(crate) fn validate_live(self) -> Result<()> {
        if self.is_zero() {
            Err(Error::ZeroIdentity)
        } else {
            Ok(())
        }
    }
}

/// Deterministic refusal from a fixed codec or semantic validator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    /// Input or output was shorter than the exact layout.
    Truncated,
    /// Input or output was longer than the exact layout.
    TrailingBytes,
    /// A local semantic-body magic did not match.
    BadMagic,
    /// A local semantic-body version did not match.
    BadVersion,
    /// Reserved bytes or fixed-width padding were nonzero.
    NonCanonicalPadding,
    /// A required full-width identity was zero.
    ZeroIdentity,
    /// A numeric value or fixed count was outside the admitted domain.
    InvalidParameter,
    /// A lifecycle phase or phase-dependent body was inconsistent.
    InvalidPhase,
    /// An immutable schedule or deadline ordering was invalid.
    InvalidSchedule,
    /// Checked integer arithmetic overflowed.
    ArithmeticOverflow,
    /// A bound identity, generation, or external semantic fact mismatched.
    MismatchedBinding,
    /// A fixed-layout account graph was not exhaustive and canonical.
    InvalidChildGraph,
    /// An LP page was not strictly owner-sorted and canonically padded.
    InvalidLpPage,
    /// Aggregate settlement progress or conservation facts disagreed.
    ConservationFailure,
    /// A runtime action was requested even though this slice is disabled.
    ActionDisabled,
}

/// Result alias for this allocation-free contract.
pub type Result<T> = core::result::Result<T, Error>;

/// Exact fixed-layout encoding and hostile decoding contract.
pub trait FixedCodec: Sized {
    /// Exact canonical body length; shorter and longer buffers both refuse.
    const ENCODED_LEN: usize;

    /// Validate and encode into an exact-length caller-owned buffer.
    fn encode_into(&self, output: &mut [u8]) -> Result<()>;

    /// Decode and validate one exact-length canonical body.
    fn decode(input: &[u8]) -> Result<Self>;

    /// Compute `SHA256(domain || canonical_body)` without allocation.
    fn content_id(&self, domain: &[u8]) -> Result<Id> {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hash_encoded(self, &mut hasher)?;
        Ok(Id::from_bytes(hasher.finalize().into()))
    }
}

fn hash_encoded<T: FixedCodec>(value: &T, hasher: &mut Sha256) -> Result<()> {
    // FixedCodec implementations feed the exact body through this bounded
    // stack buffer. Every V1 body is compile-time asserted at or below the
    // exact largest V1 body rather than consuming an entire SBF stack frame.
    if T::ENCODED_LEN > MAX_SEMANTIC_BODY_BYTES {
        return Err(Error::InvalidParameter);
    }
    let mut bytes = [0u8; MAX_SEMANTIC_BODY_BYTES];
    let body = &mut bytes[..T::ENCODED_LEN];
    value.encode_into(body)?;
    hasher.update(body);
    Ok(())
}

/// Largest semantic body supported by the allocation-free digest helper.
///
/// This is the exact V1 maximum (`LpPageV1`), not the 4,096-byte SBF frame
/// ceiling. Raising it requires a new measured stack review.
pub const MAX_SEMANTIC_BODY_BYTES: usize = 1_208;

/// Planned runtime actions. Every V1 action is deliberately disabled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerRuntimeActionV1 {
    /// Create the immutable policy artifact.
    CreatePolicy = 0,
    /// Initialize state and external Facility Position binding.
    Initialize = 1,
    /// Create one counted LP page.
    CreateLpPage = 2,
    /// Contribute one exact LP capital unit basket.
    Contribute = 3,
    /// Withdraw an exact pre-activation LP capital unit basket.
    WithdrawFunding = 4,
    /// Activate the fully funded dealer.
    Activate = 5,
    /// Cancel stale or insufficient funding.
    CancelFunding = 6,
    /// Refund sponsor capital after valid cancellation.
    RefundCancelledSponsor = 7,
    /// Bind the sole next admitted auction Epoch.
    BindEpoch = 8,
    /// Advance an empty/lapsed Epoch binding without trade.
    LapseEpoch = 9,
    /// Select one final candidate, create its Lease/Pot, and deposit Begin assets.
    SelectLeaseAndBegin = 10,
    /// Collect authenticated aggregate settlement inputs.
    Collect = 11,
    /// Deliver authenticated aggregate settlement outputs.
    Deliver = 12,
    /// Finalize the exact generation transition.
    FinalizeSettlement = 13,
    /// Restore Begin deposits before any row progress and consume the generation.
    AbortBeforeCollection = 14,
    /// Queue LP shares for unwind-only mode.
    QueueExit = 15,
    /// Enter unwind-only mode by sponsor halt.
    SponsorHalt = 16,
    /// Enter unwind-only mode by queue quorum.
    EnterUnwind = 17,
    /// Enter unwind-only mode at the immutable close slot.
    TimedClose = 18,
    /// Resolve with an authenticated payout.
    Resolve = 19,
    /// Deliver one terminal LP claim.
    Claim = 20,
    /// Retire one counted child or root.
    Retire = 21,
}

impl DealerRuntimeActionV1 {
    /// Recover the frozen action coordinate from its exact vector index.
    pub fn from_index(index: usize) -> Result<Self> {
        match index {
            0 => Ok(Self::CreatePolicy),
            1 => Ok(Self::Initialize),
            2 => Ok(Self::CreateLpPage),
            3 => Ok(Self::Contribute),
            4 => Ok(Self::WithdrawFunding),
            5 => Ok(Self::Activate),
            6 => Ok(Self::CancelFunding),
            7 => Ok(Self::RefundCancelledSponsor),
            8 => Ok(Self::BindEpoch),
            9 => Ok(Self::LapseEpoch),
            10 => Ok(Self::SelectLeaseAndBegin),
            11 => Ok(Self::Collect),
            12 => Ok(Self::Deliver),
            13 => Ok(Self::FinalizeSettlement),
            14 => Ok(Self::AbortBeforeCollection),
            15 => Ok(Self::QueueExit),
            16 => Ok(Self::SponsorHalt),
            17 => Ok(Self::EnterUnwind),
            18 => Ok(Self::TimedClose),
            19 => Ok(Self::Resolve),
            20 => Ok(Self::Claim),
            21 => Ok(Self::Retire),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Fail closed until a separately reviewed adapter allocates and enables an action.
pub const fn require_action_enabled(_action: DealerRuntimeActionV1) -> Result<()> {
    Err(Error::ActionDisabled)
}

pub(crate) fn add(left: u64, right: u64) -> Result<u64> {
    left.checked_add(right).ok_or(Error::ArithmeticOverflow)
}

pub(crate) fn mul(left: u64, right: u64) -> Result<u64> {
    left.checked_mul(right).ok_or(Error::ArithmeticOverflow)
}

pub(crate) fn validate_padding_u64(outcome_count: u8, values: &[u64; MAX_OUTCOMES]) -> Result<()> {
    if values[usize::from(outcome_count)..]
        .iter()
        .any(|value| *value != 0)
    {
        Err(Error::NonCanonicalPadding)
    } else {
        Ok(())
    }
}

pub(crate) fn validate_padding_i64(outcome_count: u8, values: &[i64; MAX_OUTCOMES]) -> Result<()> {
    if values[usize::from(outcome_count)..]
        .iter()
        .any(|value| *value != 0)
    {
        Err(Error::NonCanonicalPadding)
    } else {
        Ok(())
    }
}
