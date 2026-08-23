// SPDX-License-Identifier: AGPL-3.0-or-later
//! Append-only Market interval history for the reusable `0xab` session cell.
//!
//! `0xab` is not a per-session rent account. Product capitalizes it once and
//! Failure reuses it under one exclusive active-session pin. Every terminal
//! session is folded into this `0xac` history before `0xab` is reset to Idle.
//! The history owns the only aggregate transcript; other roots may retain its
//! authenticated commitment but must not independently reconstruct a second
//! history truth.

use clutch_product_series::{ContentId as ProductContentId, MarketInstanceV2Id};
use sha2::{Digest, Sha256};

use crate::market_policy_v1::{
    FailureMarketAccountIdV1, FailureMarketAdmissionStateV1, FailureMarketFamilyTerminalReceiptIdV1,
};
use crate::market_quote_v1::FailureMarketRecoveryQuoteAdmissionReceiptV1;
use crate::{Error, FailurePolicyBindingId, Result};

const FUNDING_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-interval-funding/v2";
const STATE_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-interval-history-state/v2";
const APPEND_ROOT_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-interval-history-root/v2";
const APPEND_RECEIPT_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-interval-history-append/v2";
const FAMILY_SEAL_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-interval-family-seal/v2";
const CLOSE_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-interval-close/v2";
const MAGIC_V2: [u8; 8] = *b"DCFIHST2";
const VERSION_V2: u16 = 2;
const HEADER_BYTES_V2: usize = 16;
const ID_BYTES_V2: usize = 32;
const IMMUTABLE_ID_COUNT_V2: usize = 8;
const DYNAMIC_ID_COUNT_V2: usize = 5;
const AMOUNT_COUNT_V2: usize = 6;

/// Canonical semantic width inside the 512-byte permanent history account.
/// The Solana adapter owns the four-byte tag/version/bump frame.
pub const FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2: usize = 508;

macro_rules! history_id {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Construct from digest bytes without claiming authenticity.
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Return exact digest bytes.
            pub const fn bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

history_id!(
    FailureMarketIntervalFundingReceiptIdV2,
    "Typed identity of one prepaid reusable-cell/history capitalization."
);
history_id!(
    FailureMarketIntervalHistoryStateIdV2,
    "Typed commitment to one complete interval-history state."
);
history_id!(
    FailureMarketIntervalHistoryRootV2,
    "Typed append-only root over all completed interval sessions."
);
history_id!(
    FailureMarketIntervalHistoryAppendReceiptIdV2,
    "Typed identity of one authenticated terminal-session append."
);
history_id!(
    FailureMarketIntervalFamilySealReceiptIdV2,
    "Typed identity sealing exact history into Failure-family terminality."
);
history_id!(
    FailureMarketIntervalCloseAuthorizationIdV2,
    "Typed exact principal-refund and donation-disposition authorization."
);

/// Complete prepaid rent facts for the reusable cell and permanent history.
/// This projection is not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalFundingFactsV2 {
    /// Shared immutable Failure policy.
    pub failure_policy_binding_id: FailurePolicyBindingId,
    /// Full-width shared economic Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Shared Failure/liveness generation.
    pub generation: u64,
    /// Product private foundation-step receipt.
    pub prepaid_funding_receipt_id: ProductContentId,
    /// Reusable Market-scoped `0xab` cell.
    pub work_account: FailureMarketAccountIdV1,
    /// Permanent append-only Market-scoped `0xac` history.
    pub history_account: FailureMarketAccountIdV1,
    /// Immutable recipient of both exact rent principals.
    pub rent_refund_owner: FailureMarketAccountIdV1,
    /// Immutable sink for prior and later unsolicited lamports.
    pub neutral_sink: FailureMarketAccountIdV1,
    /// Canonical Rent minimum for the reusable cell.
    pub work_rent_principal_lamports: u64,
    /// Canonical Rent minimum for the permanent history.
    pub history_rent_principal_lamports: u64,
    /// Donation observed before Product capitalized the work account.
    pub work_donation_floor_lamports: u64,
    /// Exact work-account post-capitalization balance.
    pub work_observed_balance_lamports: u64,
    /// Donation observed before Product capitalized the history account.
    pub history_donation_floor_lamports: u64,
    /// Exact history-account post-capitalization balance.
    pub history_observed_balance_lamports: u64,
}

/// Private Product authority for exact prepaid account capitalization.
pub trait AuthenticatedFailureMarketIntervalFundingV2 {
    /// Authenticate canonical accounts, Rent, typed debit, and balances.
    fn authenticate_failure_market_interval_funding(
        &self,
        _expected: FailureMarketIntervalFundingFactsV2,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private-field receipt for exact reusable-cell/history capitalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalFundingReceiptV2 {
    id: FailureMarketIntervalFundingReceiptIdV2,
    facts: FailureMarketIntervalFundingFactsV2,
}

impl FailureMarketIntervalFundingReceiptV2 {
    /// Complete capitalization receipt identity.
    pub const fn id(self) -> FailureMarketIntervalFundingReceiptIdV2 {
        self.id
    }

    /// Exact authenticated funding facts.
    pub const fn facts(self) -> FailureMarketIntervalFundingFactsV2 {
        self.facts
    }
}

/// Terminal classification of one subordinate interval session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailureMarketIntervalTerminalDispositionV2 {
    /// Exhaustive interval evaluation produced the accepted Resolution.
    Resolved = 1,
    /// Finite authenticated source/recovery attempts were exhausted.
    Exhausted = 2,
    /// Authenticated source/relation evaluation refused this session.
    Refused = 3,
}

impl FailureMarketIntervalTerminalDispositionV2 {
    fn byte(self) -> u8 {
        match self {
            Self::Resolved => 1,
            Self::Exhausted => 2,
            Self::Refused => 3,
        }
    }
}

/// Complete expected terminal session folded into the permanent history.
/// This projection is not terminal authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalTerminalFactsV2 {
    /// Exact history prestate.
    pub history_before: FailureMarketIntervalHistoryStateIdV2,
    /// Unique session binding retained by the reusable cell.
    pub session_binding_id: ProductContentId,
    /// Private terminal receipt from the session semantic owner.
    pub session_terminal_receipt_id: ProductContentId,
    /// Complete terminal reusable-cell postimage.
    pub terminal_state_commitment: ProductContentId,
    /// Canonical Idle reusable-cell postimage written after this terminal is
    /// folded into history in the same atomic batch.
    pub idle_state_commitment: ProductContentId,
    /// Last exact liveness work receipt, or zero for a zero-work terminal.
    pub last_liveness_work_receipt_id: ProductContentId,
    /// Session classification.
    pub disposition: FailureMarketIntervalTerminalDispositionV2,
    /// Exact number of paid calls consumed by this session.
    pub completed_work_calls: u32,
    /// Exact keeper rewards consumed by this session.
    pub exact_reward_lamports: u64,
}

/// Private semantic authority for one exact terminal session.
pub trait AuthenticatedFailureMarketIntervalTerminalV2 {
    /// Authenticate the terminal cell, Source/Product outcome, liveness joins,
    /// and exact per-session counters.
    fn authenticate_failure_market_interval_terminal(
        &self,
        _expected: FailureMarketIntervalTerminalFactsV2,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Permanent append-only Market interval history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalHistoryV2 {
    failure_policy_binding_id: FailurePolicyBindingId,
    market_instance_id: MarketInstanceV2Id,
    funding_receipt_id: FailureMarketIntervalFundingReceiptIdV2,
    work_account: FailureMarketAccountIdV1,
    history_account: FailureMarketAccountIdV1,
    rent_refund_owner: FailureMarketAccountIdV1,
    neutral_sink: FailureMarketAccountIdV1,
    quote_admission_receipt_id: ProductContentId,
    generation: u64,
    work_rent_principal_lamports: u64,
    history_rent_principal_lamports: u64,
    completed_session_count: u64,
    completed_work_calls: u64,
    exact_reward_lamports: u64,
    history_root: FailureMarketIntervalHistoryRootV2,
    latest_session_binding_id: ProductContentId,
    latest_terminal_receipt_id: ProductContentId,
    latest_terminal_state_commitment: ProductContentId,
    family_terminal_receipt_id: FailureMarketFamilyTerminalReceiptIdV1,
}

impl FailureMarketIntervalHistoryV2 {
    /// Complete state commitment used by stale-checked append authority.
    pub fn id(self) -> Result<FailureMarketIntervalHistoryStateIdV2> {
        let mut body = [0u8; FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2];
        self.encode_into(&mut body)?;
        let mut hasher = Sha256::new();
        hasher.update(STATE_DOMAIN_V2);
        hasher.update(body);
        Ok(FailureMarketIntervalHistoryStateIdV2::from_bytes(
            hasher.finalize().into(),
        ))
    }

    /// Immutable shared Failure policy.
    pub const fn failure_policy_binding_id(self) -> FailurePolicyBindingId {
        self.failure_policy_binding_id
    }

    /// Full-width economic Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact capitalization receipt frozen into this history.
    pub const fn funding_receipt_id(self) -> FailureMarketIntervalFundingReceiptIdV2 {
        self.funding_receipt_id
    }

    /// Shared Failure/liveness generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Exact authenticated Market quote admission.
    pub const fn quote_admission_receipt_id(self) -> ProductContentId {
        self.quote_admission_receipt_id
    }

    /// Reusable Market interval cell.
    pub const fn work_account(self) -> FailureMarketAccountIdV1 {
        self.work_account
    }

    /// Permanent append-only history account.
    pub const fn history_account(self) -> FailureMarketAccountIdV1 {
        self.history_account
    }

    /// Exact number of completed sessions.
    pub const fn completed_session_count(self) -> u64 {
        self.completed_session_count
    }

    /// Exact aggregate paid call count.
    pub const fn completed_work_calls(self) -> u64 {
        self.completed_work_calls
    }

    /// Exact aggregate keeper rewards.
    pub const fn exact_reward_lamports(self) -> u64 {
        self.exact_reward_lamports
    }

    /// Sole append-only interval transcript root.
    pub const fn history_root(self) -> FailureMarketIntervalHistoryRootV2 {
        self.history_root
    }

    /// Latest folded session terminal receipt, or zero when empty.
    pub const fn latest_terminal_receipt_id(self) -> ProductContentId {
        self.latest_terminal_receipt_id
    }

    /// Complete latest terminal reusable-cell postimage.
    pub const fn latest_terminal_state_commitment(self) -> ProductContentId {
        self.latest_terminal_state_commitment
    }

    /// Exhaustive Failure-family receipt, if history has been sealed.
    pub const fn family_terminal_receipt_id(self) -> FailureMarketFamilyTerminalReceiptIdV1 {
        self.family_terminal_receipt_id
    }

    /// Stale-checked commit of one append or family seal.
    pub fn commit_plan(&mut self, plan: FailureMarketIntervalHistoryPlanV2) -> Result<()> {
        self.validate()?;
        if *self != plan.before {
            return Err(Error::StalePlan);
        }
        plan.after.validate()?;
        *self = plan.after;
        Ok(())
    }

    /// Encode every semantic byte and reject noncanonical state.
    pub fn encode_into(
        self,
        output: &mut [u8; FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2],
    ) -> Result<()> {
        self.validate()?;
        output.fill(0);
        output[..8].copy_from_slice(&MAGIC_V2);
        output[8..10].copy_from_slice(&VERSION_V2.to_le_bytes());
        let mut cursor = HEADER_BYTES_V2;
        for id in [
            self.failure_policy_binding_id.bytes(),
            self.market_instance_id.bytes(),
            self.funding_receipt_id.bytes(),
            self.work_account.bytes(),
            self.history_account.bytes(),
            self.rent_refund_owner.bytes(),
            self.neutral_sink.bytes(),
            self.quote_admission_receipt_id.bytes(),
        ] {
            put_id(output, &mut cursor, id)?;
        }
        for value in [
            self.generation,
            self.work_rent_principal_lamports,
            self.history_rent_principal_lamports,
            self.completed_session_count,
            self.completed_work_calls,
            self.exact_reward_lamports,
        ] {
            put_u64(output, &mut cursor, value)?;
        }
        for id in [
            self.history_root.bytes(),
            self.latest_session_binding_id.bytes(),
            self.latest_terminal_receipt_id.bytes(),
            self.latest_terminal_state_commitment.bytes(),
            self.family_terminal_receipt_id.bytes(),
        ] {
            put_id(output, &mut cursor, id)?;
        }
        if output
            .get(cursor..)
            .ok_or(Error::WrongLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::WrongLength);
        }
        Ok(())
    }

    /// Hostile-decode exact permanent history bytes against authenticated
    /// immutable admission and quote receipts.
    pub fn decode_for_admission(
        input: &[u8; FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2],
        admission: FailureMarketAdmissionStateV1,
        quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    ) -> Result<Self> {
        if input[..8] != MAGIC_V2 {
            return Err(Error::BadMagic);
        }
        if input[8..10] != VERSION_V2.to_le_bytes() {
            return Err(Error::BadVersion);
        }
        if input[10..HEADER_BYTES_V2].iter().any(|byte| *byte != 0) {
            return Err(Error::NonCanonicalReserved);
        }
        let mut cursor = HEADER_BYTES_V2;
        let value = Self {
            failure_policy_binding_id: FailurePolicyBindingId::from_bytes(take_id(
                input,
                &mut cursor,
            )?),
            market_instance_id: MarketInstanceV2Id::from_bytes(take_id(input, &mut cursor)?),
            funding_receipt_id: FailureMarketIntervalFundingReceiptIdV2::from_bytes(take_id(
                input,
                &mut cursor,
            )?),
            work_account: FailureMarketAccountIdV1::from_bytes(take_id(input, &mut cursor)?),
            history_account: FailureMarketAccountIdV1::from_bytes(take_id(input, &mut cursor)?),
            rent_refund_owner: FailureMarketAccountIdV1::from_bytes(take_id(input, &mut cursor)?),
            neutral_sink: FailureMarketAccountIdV1::from_bytes(take_id(input, &mut cursor)?),
            quote_admission_receipt_id: ProductContentId::from_bytes(take_id(input, &mut cursor)?),
            generation: take_u64(input, &mut cursor)?,
            work_rent_principal_lamports: take_u64(input, &mut cursor)?,
            history_rent_principal_lamports: take_u64(input, &mut cursor)?,
            completed_session_count: take_u64(input, &mut cursor)?,
            completed_work_calls: take_u64(input, &mut cursor)?,
            exact_reward_lamports: take_u64(input, &mut cursor)?,
            history_root: FailureMarketIntervalHistoryRootV2::from_bytes(take_id(
                input,
                &mut cursor,
            )?),
            latest_session_binding_id: ProductContentId::from_bytes(take_id(input, &mut cursor)?),
            latest_terminal_receipt_id: ProductContentId::from_bytes(take_id(input, &mut cursor)?),
            latest_terminal_state_commitment: ProductContentId::from_bytes(take_id(
                input,
                &mut cursor,
            )?),
            family_terminal_receipt_id: FailureMarketFamilyTerminalReceiptIdV1::from_bytes(
                take_id(input, &mut cursor)?,
            ),
        };
        if input
            .get(cursor..)
            .ok_or(Error::WrongLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalReserved);
        }
        value.validate_against(admission, quote)?;
        Ok(value)
    }

    fn validate(self) -> Result<()> {
        for id in [
            self.failure_policy_binding_id.bytes(),
            self.market_instance_id.bytes(),
            self.funding_receipt_id.bytes(),
            self.work_account.bytes(),
            self.history_account.bytes(),
            self.rent_refund_owner.bytes(),
            self.neutral_sink.bytes(),
            self.quote_admission_receipt_id.bytes(),
        ] {
            require_live(id)?;
        }
        if self.generation == 0
            || self.work_rent_principal_lamports == 0
            || self.history_rent_principal_lamports == 0
            || self.work_account == self.history_account
            || self.work_account == self.rent_refund_owner
            || self.work_account == self.neutral_sink
            || self.history_account == self.rent_refund_owner
            || self.history_account == self.neutral_sink
            || self.rent_refund_owner == self.neutral_sink
        {
            return Err(Error::BindingMismatch);
        }
        let root = self.history_root.bytes() != [0; 32];
        let session = !self.latest_session_binding_id.is_zero();
        let terminal = !self.latest_terminal_receipt_id.is_zero();
        let terminal_state = !self.latest_terminal_state_commitment.is_zero();
        if self.completed_session_count == 0 {
            if root
                || session
                || terminal
                || terminal_state
                || self.completed_work_calls != 0
                || self.exact_reward_lamports != 0
            {
                return Err(Error::BindingMismatch);
            }
        } else if !(root && session && terminal && terminal_state) {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }

    /// Validate the complete self-contained history shape without claiming
    /// account authenticity. Crate-local transition owners use this before a
    /// private adapter authenticates the persisted account.
    pub(crate) fn validate_internal(self) -> Result<()> {
        self.validate()
    }

    fn validate_against(
        self,
        admission: FailureMarketAdmissionStateV1,
        quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    ) -> Result<()> {
        self.validate()?;
        let policy = admission.binding().facts();
        let quote_facts = quote.facts();
        if self.failure_policy_binding_id != admission.binding().id()
            || self.market_instance_id != policy.market_instance_id
            || self.generation != policy.generation
            || self.quote_admission_receipt_id.bytes() != quote.id().bytes()
            || quote_facts.failure_policy_binding_id != self.failure_policy_binding_id
            || self.completed_work_calls > u64::from(quote_facts.maximum_calls)
            || self.exact_reward_lamports > quote_facts.work_principal_lamports
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
}

/// One stale-checked append or family-seal state transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalHistoryPlanV2 {
    before: FailureMarketIntervalHistoryV2,
    after: FailureMarketIntervalHistoryV2,
}

impl FailureMarketIntervalHistoryPlanV2 {
    /// Complete resulting history poststate.
    pub const fn resulting_history(self) -> FailureMarketIntervalHistoryV2 {
        self.after
    }
}

/// Private-field receipt proving one terminal was appended before cell reset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalHistoryAppendReceiptV2 {
    id: FailureMarketIntervalHistoryAppendReceiptIdV2,
    failure_policy_binding_id: FailurePolicyBindingId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    funding_receipt_id: FailureMarketIntervalFundingReceiptIdV2,
    work_account: FailureMarketAccountIdV1,
    history_account: FailureMarketAccountIdV1,
    history_before: FailureMarketIntervalHistoryStateIdV2,
    history_after: FailureMarketIntervalHistoryStateIdV2,
    previous_root: FailureMarketIntervalHistoryRootV2,
    resulting_root: FailureMarketIntervalHistoryRootV2,
    session_binding_id: ProductContentId,
    session_terminal_receipt_id: ProductContentId,
    terminal_state_commitment: ProductContentId,
    idle_state_commitment: ProductContentId,
    completed_session_count: u64,
}

impl FailureMarketIntervalHistoryAppendReceiptV2 {
    /// Complete append receipt identity.
    pub const fn id(self) -> FailureMarketIntervalHistoryAppendReceiptIdV2 {
        self.id
    }

    /// Exact shared Failure policy.
    pub const fn failure_policy_binding_id(self) -> FailurePolicyBindingId {
        self.failure_policy_binding_id
    }

    /// Full-width economic Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Shared Failure/liveness generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Product-authenticated reusable-cell/history capitalization.
    pub const fn funding_receipt_id(self) -> FailureMarketIntervalFundingReceiptIdV2 {
        self.funding_receipt_id
    }

    /// Exact reusable `0xab/v2` cell.
    pub const fn work_account(self) -> FailureMarketAccountIdV1 {
        self.work_account
    }

    /// Exact append-only `0xac/v2` history.
    pub const fn history_account(self) -> FailureMarketAccountIdV1 {
        self.history_account
    }

    /// Exact history prestate.
    pub const fn history_before(self) -> FailureMarketIntervalHistoryStateIdV2 {
        self.history_before
    }

    /// Exact history poststate.
    pub const fn history_after(self) -> FailureMarketIntervalHistoryStateIdV2 {
        self.history_after
    }

    /// Prior append-only root.
    pub const fn previous_root(self) -> FailureMarketIntervalHistoryRootV2 {
        self.previous_root
    }

    /// Resulting append-only root.
    pub const fn resulting_root(self) -> FailureMarketIntervalHistoryRootV2 {
        self.resulting_root
    }

    /// Exact subordinate session folded by this append.
    pub const fn session_binding_id(self) -> ProductContentId {
        self.session_binding_id
    }

    /// Exact terminal folded by this append.
    pub const fn session_terminal_receipt_id(self) -> ProductContentId {
        self.session_terminal_receipt_id
    }

    /// Complete terminal reusable-cell postimage folded by this append.
    pub const fn terminal_state_commitment(self) -> ProductContentId {
        self.terminal_state_commitment
    }

    /// Canonical Idle reusable-cell postimage written by this append/reset.
    pub const fn idle_state_commitment(self) -> ProductContentId {
        self.idle_state_commitment
    }

    /// Resulting one-based completed-session count.
    pub const fn completed_session_count(self) -> u64 {
        self.completed_session_count
    }
}

/// Admit once-capitalized shared interval accounts and canonical empty history.
pub fn admit_failure_market_interval_history_v2<
    A: AuthenticatedFailureMarketIntervalFundingV2 + ?Sized,
>(
    authority: &A,
    admission: FailureMarketAdmissionStateV1,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    facts: FailureMarketIntervalFundingFactsV2,
) -> Result<(
    FailureMarketIntervalHistoryV2,
    FailureMarketIntervalFundingReceiptV2,
)> {
    let policy = admission.binding().facts();
    let quote_facts = quote.facts();
    validate_funding_facts(admission, quote, facts)?;
    authority.authenticate_failure_market_interval_funding(facts)?;
    let mut funding_hasher = Sha256::new();
    funding_hasher.update(FUNDING_DOMAIN_V2);
    hash_funding_facts(&mut funding_hasher, facts);
    let funding_receipt_id =
        FailureMarketIntervalFundingReceiptIdV2::from_bytes(funding_hasher.finalize().into());
    require_live(funding_receipt_id.bytes())?;
    let funding = FailureMarketIntervalFundingReceiptV2 {
        id: funding_receipt_id,
        facts,
    };
    let history = FailureMarketIntervalHistoryV2 {
        failure_policy_binding_id: admission.binding().id(),
        market_instance_id: policy.market_instance_id,
        funding_receipt_id,
        work_account: facts.work_account,
        history_account: facts.history_account,
        rent_refund_owner: facts.rent_refund_owner,
        neutral_sink: facts.neutral_sink,
        quote_admission_receipt_id: ProductContentId::from_bytes(quote.id().bytes()),
        generation: policy.generation,
        work_rent_principal_lamports: facts.work_rent_principal_lamports,
        history_rent_principal_lamports: facts.history_rent_principal_lamports,
        completed_session_count: 0,
        completed_work_calls: 0,
        exact_reward_lamports: 0,
        history_root: FailureMarketIntervalHistoryRootV2::from_bytes([0; 32]),
        latest_session_binding_id: ProductContentId::ZERO,
        latest_terminal_receipt_id: ProductContentId::ZERO,
        latest_terminal_state_commitment: ProductContentId::ZERO,
        family_terminal_receipt_id: FailureMarketFamilyTerminalReceiptIdV1::from_bytes([0; 32]),
    };
    if quote_facts.failure_policy_binding_id != history.failure_policy_binding_id {
        return Err(Error::BindingMismatch);
    }
    history.validate_against(admission, quote)?;
    Ok((history, funding))
}

/// Append one exact terminal session before the reusable cell resets to Idle.
pub fn plan_append_failure_market_interval_history_v2<
    A: AuthenticatedFailureMarketIntervalTerminalV2 + ?Sized,
>(
    authority: &A,
    history: FailureMarketIntervalHistoryV2,
    admission: FailureMarketAdmissionStateV1,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    terminal: FailureMarketIntervalTerminalFactsV2,
) -> Result<(
    FailureMarketIntervalHistoryPlanV2,
    FailureMarketIntervalHistoryAppendReceiptV2,
)> {
    history.validate_against(admission, quote)?;
    if history.family_terminal_receipt_id.bytes() != [0; 32]
        || terminal.history_before != history.id()?
    {
        return Err(Error::WrongPhase);
    }
    validate_terminal_facts(history, terminal)?;
    authority.authenticate_failure_market_interval_terminal(terminal)?;
    let next_count = history
        .completed_session_count
        .checked_add(1)
        .ok_or(Error::BindingMismatch)?;
    let next_calls = history
        .completed_work_calls
        .checked_add(u64::from(terminal.completed_work_calls))
        .ok_or(Error::BindingMismatch)?;
    let next_rewards = history
        .exact_reward_lamports
        .checked_add(terminal.exact_reward_lamports)
        .ok_or(Error::BindingMismatch)?;
    let quote_facts = quote.facts();
    if next_calls > u64::from(quote_facts.maximum_calls)
        || next_rewards > quote_facts.work_principal_lamports
    {
        return Err(Error::BindingMismatch);
    }
    let history_before = history.id()?;
    let mut root_hasher = Sha256::new();
    root_hasher.update(APPEND_ROOT_DOMAIN_V2);
    root_hasher.update(history.failure_policy_binding_id.bytes());
    root_hasher.update(history.market_instance_id.bytes());
    root_hasher.update(history.generation.to_le_bytes());
    root_hasher.update(history_before.bytes());
    root_hasher.update(history.history_root.bytes());
    root_hasher.update(history.completed_session_count.to_le_bytes());
    root_hasher.update(next_count.to_le_bytes());
    hash_terminal_facts(&mut root_hasher, terminal);
    root_hasher.update(next_calls.to_le_bytes());
    root_hasher.update(next_rewards.to_le_bytes());
    let resulting_root =
        FailureMarketIntervalHistoryRootV2::from_bytes(root_hasher.finalize().into());
    require_live(resulting_root.bytes())?;
    let mut after = history;
    after.completed_session_count = next_count;
    after.completed_work_calls = next_calls;
    after.exact_reward_lamports = next_rewards;
    after.history_root = resulting_root;
    after.latest_session_binding_id = terminal.session_binding_id;
    after.latest_terminal_receipt_id = terminal.session_terminal_receipt_id;
    after.latest_terminal_state_commitment = terminal.terminal_state_commitment;
    after.validate_against(admission, quote)?;
    let history_after = after.id()?;
    let mut receipt_hasher = Sha256::new();
    receipt_hasher.update(APPEND_RECEIPT_DOMAIN_V2);
    receipt_hasher.update(history_before.bytes());
    receipt_hasher.update(history_after.bytes());
    receipt_hasher.update(history.history_root.bytes());
    receipt_hasher.update(resulting_root.bytes());
    receipt_hasher.update(terminal.session_terminal_receipt_id.bytes());
    receipt_hasher.update(next_count.to_le_bytes());
    let id =
        FailureMarketIntervalHistoryAppendReceiptIdV2::from_bytes(receipt_hasher.finalize().into());
    require_live(id.bytes())?;
    Ok((
        FailureMarketIntervalHistoryPlanV2 {
            before: history,
            after,
        },
        FailureMarketIntervalHistoryAppendReceiptV2 {
            id,
            failure_policy_binding_id: history.failure_policy_binding_id,
            market_instance_id: history.market_instance_id,
            generation: history.generation,
            funding_receipt_id: history.funding_receipt_id,
            work_account: history.work_account,
            history_account: history.history_account,
            history_before,
            history_after,
            previous_root: history.history_root,
            resulting_root,
            session_binding_id: terminal.session_binding_id,
            session_terminal_receipt_id: terminal.session_terminal_receipt_id,
            terminal_state_commitment: terminal.terminal_state_commitment,
            idle_state_commitment: terminal.idle_state_commitment,
            completed_session_count: next_count,
        },
    ))
}

/// Expected exact aggregate-family seal. This projection is not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalFamilySealFactsV2 {
    /// Exact history prestate.
    pub history_before: FailureMarketIntervalHistoryStateIdV2,
    /// Sole append-only root, possibly zero for an empty history.
    pub history_root: FailureMarketIntervalHistoryRootV2,
    /// Exact completed session count.
    pub completed_session_count: u64,
    /// Exact aggregate paid calls.
    pub completed_work_calls: u64,
    /// Exact aggregate keeper rewards.
    pub exact_reward_lamports: u64,
    /// Exhaustive external Failure-family terminal receipt.
    pub family_terminal_receipt_id: FailureMarketFamilyTerminalReceiptIdV1,
}

/// Private authority proving exhaustive Failure-family terminality.
pub trait AuthenticatedFailureMarketIntervalFamilySealV2 {
    /// Authenticate the family receipt and exact interval-history projection.
    fn authenticate_failure_market_interval_family_seal(
        &self,
        _expected: FailureMarketIntervalFamilySealFactsV2,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private-field receipt sealing history before either account can close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalFamilySealReceiptV2 {
    id: FailureMarketIntervalFamilySealReceiptIdV2,
    facts: FailureMarketIntervalFamilySealFactsV2,
    history_after: FailureMarketIntervalHistoryStateIdV2,
}

impl FailureMarketIntervalFamilySealReceiptV2 {
    /// Complete seal receipt identity.
    pub const fn id(self) -> FailureMarketIntervalFamilySealReceiptIdV2 {
        self.id
    }

    /// Exact sealed family/history facts.
    pub const fn facts(self) -> FailureMarketIntervalFamilySealFactsV2 {
        self.facts
    }

    /// Complete sealed history poststate.
    pub const fn history_after(self) -> FailureMarketIntervalHistoryStateIdV2 {
        self.history_after
    }
}

/// Seal the complete interval history into an exhaustive family receipt.
pub fn plan_seal_failure_market_interval_history_v2<
    A: AuthenticatedFailureMarketIntervalFamilySealV2 + ?Sized,
>(
    authority: &A,
    history: FailureMarketIntervalHistoryV2,
    admission: FailureMarketAdmissionStateV1,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    family_terminal_receipt_id: FailureMarketFamilyTerminalReceiptIdV1,
) -> Result<(
    FailureMarketIntervalHistoryPlanV2,
    FailureMarketIntervalFamilySealReceiptV2,
)> {
    history.validate_against(admission, quote)?;
    require_live(family_terminal_receipt_id.bytes())?;
    if history.family_terminal_receipt_id.bytes() != [0; 32] {
        return Err(Error::WrongPhase);
    }
    let facts = FailureMarketIntervalFamilySealFactsV2 {
        history_before: history.id()?,
        history_root: history.history_root,
        completed_session_count: history.completed_session_count,
        completed_work_calls: history.completed_work_calls,
        exact_reward_lamports: history.exact_reward_lamports,
        family_terminal_receipt_id,
    };
    authority.authenticate_failure_market_interval_family_seal(facts)?;
    let mut after = history;
    after.family_terminal_receipt_id = family_terminal_receipt_id;
    after.validate_against(admission, quote)?;
    let history_after = after.id()?;
    let mut hasher = Sha256::new();
    hasher.update(FAMILY_SEAL_DOMAIN_V2);
    hasher.update(facts.history_before.bytes());
    hasher.update(facts.history_root.bytes());
    hasher.update(facts.completed_session_count.to_le_bytes());
    hasher.update(facts.completed_work_calls.to_le_bytes());
    hasher.update(facts.exact_reward_lamports.to_le_bytes());
    hasher.update(facts.family_terminal_receipt_id.bytes());
    hasher.update(history_after.bytes());
    let id = FailureMarketIntervalFamilySealReceiptIdV2::from_bytes(hasher.finalize().into());
    require_live(id.bytes())?;
    Ok((
        FailureMarketIntervalHistoryPlanV2 {
            before: history,
            after,
        },
        FailureMarketIntervalFamilySealReceiptV2 {
            id,
            facts,
            history_after,
        },
    ))
}

/// Exact reverse-order terminal disposition of the reusable and history accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalClosePlanV2 {
    /// Reusable cell closed first.
    pub work_account: FailureMarketAccountIdV1,
    /// Permanent history closed only after its family seal is consumed.
    pub history_account: FailureMarketAccountIdV1,
    /// Immutable refund recipient for both exact principals.
    pub rent_refund_owner: FailureMarketAccountIdV1,
    /// Exact work rent principal.
    pub work_rent_refund_lamports: u64,
    /// Exact history rent principal.
    pub history_rent_refund_lamports: u64,
    /// Immutable donation sink.
    pub neutral_sink: FailureMarketAccountIdV1,
    /// Entire work-account surplus over principal.
    pub work_donation_lamports: u64,
    /// Entire history-account surplus over principal.
    pub history_donation_lamports: u64,
    /// Exact family seal authorizing reverse dependency close.
    pub family_seal_receipt_id: FailureMarketIntervalFamilySealReceiptIdV2,
    /// Complete close authorization identity.
    pub authorization_id: FailureMarketIntervalCloseAuthorizationIdV2,
}

/// Project exact account movements only after exhaustive family sealing.
pub fn plan_close_failure_market_interval_accounts_v2(
    history: FailureMarketIntervalHistoryV2,
    seal: FailureMarketIntervalFamilySealReceiptV2,
    actual_work_balance_lamports: u64,
    actual_history_balance_lamports: u64,
) -> Result<FailureMarketIntervalClosePlanV2> {
    history.validate()?;
    if seal.history_after != history.id()?
        || seal.facts.family_terminal_receipt_id != history.family_terminal_receipt_id
        || actual_work_balance_lamports < history.work_rent_principal_lamports
        || actual_history_balance_lamports < history.history_rent_principal_lamports
    {
        return Err(Error::BindingMismatch);
    }
    let work_donation_lamports = actual_work_balance_lamports
        .checked_sub(history.work_rent_principal_lamports)
        .ok_or(Error::BindingMismatch)?;
    let history_donation_lamports = actual_history_balance_lamports
        .checked_sub(history.history_rent_principal_lamports)
        .ok_or(Error::BindingMismatch)?;
    let mut hasher = Sha256::new();
    hasher.update(CLOSE_DOMAIN_V2);
    hasher.update(history.id()?.bytes());
    hasher.update(history.work_account.bytes());
    hasher.update(history.history_account.bytes());
    hasher.update(history.rent_refund_owner.bytes());
    hasher.update(history.neutral_sink.bytes());
    hasher.update(history.work_rent_principal_lamports.to_le_bytes());
    hasher.update(history.history_rent_principal_lamports.to_le_bytes());
    hasher.update(work_donation_lamports.to_le_bytes());
    hasher.update(history_donation_lamports.to_le_bytes());
    hasher.update(seal.id.bytes());
    let authorization_id =
        FailureMarketIntervalCloseAuthorizationIdV2::from_bytes(hasher.finalize().into());
    require_live(authorization_id.bytes())?;
    Ok(FailureMarketIntervalClosePlanV2 {
        work_account: history.work_account,
        history_account: history.history_account,
        rent_refund_owner: history.rent_refund_owner,
        work_rent_refund_lamports: history.work_rent_principal_lamports,
        history_rent_refund_lamports: history.history_rent_principal_lamports,
        neutral_sink: history.neutral_sink,
        work_donation_lamports,
        history_donation_lamports,
        family_seal_receipt_id: seal.id,
        authorization_id,
    })
}

fn validate_funding_facts(
    admission: FailureMarketAdmissionStateV1,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    facts: FailureMarketIntervalFundingFactsV2,
) -> Result<()> {
    let policy = admission.binding().facts();
    require_live(facts.prepaid_funding_receipt_id.bytes())?;
    for account in [
        facts.work_account,
        facts.history_account,
        facts.rent_refund_owner,
        facts.neutral_sink,
    ] {
        require_live(account.bytes())?;
    }
    if facts.failure_policy_binding_id != admission.binding().id()
        || facts.market_instance_id != policy.market_instance_id
        || facts.generation != policy.generation
        || quote.facts().failure_policy_binding_id != facts.failure_policy_binding_id
        || facts.work_rent_principal_lamports == 0
        || facts.history_rent_principal_lamports == 0
        || facts.work_observed_balance_lamports
            != facts
                .work_rent_principal_lamports
                .checked_add(facts.work_donation_floor_lamports)
                .ok_or(Error::BindingMismatch)?
        || facts.history_observed_balance_lamports
            != facts
                .history_rent_principal_lamports
                .checked_add(facts.history_donation_floor_lamports)
                .ok_or(Error::BindingMismatch)?
        || facts.work_account == facts.history_account
        || facts.work_account == facts.rent_refund_owner
        || facts.work_account == facts.neutral_sink
        || facts.history_account == facts.rent_refund_owner
        || facts.history_account == facts.neutral_sink
        || facts.rent_refund_owner == facts.neutral_sink
        || facts.work_account == admission.root_funding().facts().root_account_id
        || facts.history_account == admission.root_funding().facts().root_account_id
        || facts.work_account.bytes() == policy.recovery_state_id.bytes()
        || facts.history_account.bytes() == policy.recovery_state_id.bytes()
        || facts.work_account.bytes() == policy.recovery_compartment_account_id.bytes()
        || facts.history_account.bytes() == policy.recovery_compartment_account_id.bytes()
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn validate_terminal_facts(
    history: FailureMarketIntervalHistoryV2,
    terminal: FailureMarketIntervalTerminalFactsV2,
) -> Result<()> {
    for id in [
        terminal.session_binding_id.bytes(),
        terminal.session_terminal_receipt_id.bytes(),
        terminal.terminal_state_commitment.bytes(),
        terminal.idle_state_commitment.bytes(),
    ] {
        require_live(id)?;
    }
    if terminal.session_terminal_receipt_id == history.latest_terminal_receipt_id
        || terminal.session_binding_id == terminal.session_terminal_receipt_id
        || terminal.terminal_state_commitment == terminal.session_terminal_receipt_id
        || terminal.idle_state_commitment.is_zero()
        || terminal.idle_state_commitment == terminal.terminal_state_commitment
        || terminal.idle_state_commitment == terminal.session_terminal_receipt_id
        || (terminal.completed_work_calls == 0
            && (!terminal.last_liveness_work_receipt_id.is_zero()
                || terminal.exact_reward_lamports != 0))
        || (terminal.completed_work_calls != 0
            && (terminal.last_liveness_work_receipt_id.is_zero()
                || terminal.exact_reward_lamports == 0))
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn hash_funding_facts(hasher: &mut Sha256, facts: FailureMarketIntervalFundingFactsV2) {
    hasher.update(facts.failure_policy_binding_id.bytes());
    hasher.update(facts.market_instance_id.bytes());
    hasher.update(facts.generation.to_le_bytes());
    hasher.update(facts.prepaid_funding_receipt_id.bytes());
    hasher.update(facts.work_account.bytes());
    hasher.update(facts.history_account.bytes());
    hasher.update(facts.rent_refund_owner.bytes());
    hasher.update(facts.neutral_sink.bytes());
    hasher.update(facts.work_rent_principal_lamports.to_le_bytes());
    hasher.update(facts.history_rent_principal_lamports.to_le_bytes());
    hasher.update(facts.work_donation_floor_lamports.to_le_bytes());
    hasher.update(facts.work_observed_balance_lamports.to_le_bytes());
    hasher.update(facts.history_donation_floor_lamports.to_le_bytes());
    hasher.update(facts.history_observed_balance_lamports.to_le_bytes());
}

fn hash_terminal_facts(hasher: &mut Sha256, terminal: FailureMarketIntervalTerminalFactsV2) {
    hasher.update(terminal.history_before.bytes());
    hasher.update(terminal.session_binding_id.bytes());
    hasher.update(terminal.session_terminal_receipt_id.bytes());
    hasher.update(terminal.terminal_state_commitment.bytes());
    hasher.update(terminal.idle_state_commitment.bytes());
    hasher.update(terminal.last_liveness_work_receipt_id.bytes());
    hasher.update([terminal.disposition.byte()]);
    hasher.update(terminal.completed_work_calls.to_le_bytes());
    hasher.update(terminal.exact_reward_lamports.to_le_bytes());
}

fn put_id(
    output: &mut [u8; FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2],
    cursor: &mut usize,
    value: [u8; ID_BYTES_V2],
) -> Result<()> {
    let end = cursor.checked_add(ID_BYTES_V2).ok_or(Error::WrongLength)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::WrongLength)?
        .copy_from_slice(&value);
    *cursor = end;
    Ok(())
}

fn take_id(
    input: &[u8; FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2],
    cursor: &mut usize,
) -> Result<[u8; ID_BYTES_V2]> {
    let end = cursor.checked_add(ID_BYTES_V2).ok_or(Error::WrongLength)?;
    let value = input
        .get(*cursor..end)
        .ok_or(Error::WrongLength)?
        .try_into()
        .map_err(|_| Error::WrongLength)?;
    *cursor = end;
    Ok(value)
}

fn put_u64(
    output: &mut [u8; FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2],
    cursor: &mut usize,
    value: u64,
) -> Result<()> {
    let end = cursor.checked_add(8).ok_or(Error::WrongLength)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::WrongLength)?
        .copy_from_slice(&value.to_le_bytes());
    *cursor = end;
    Ok(())
}

fn take_u64(
    input: &[u8; FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2],
    cursor: &mut usize,
) -> Result<u64> {
    let end = cursor.checked_add(8).ok_or(Error::WrongLength)?;
    let bytes = input
        .get(*cursor..end)
        .ok_or(Error::WrongLength)?
        .try_into()
        .map_err(|_| Error::WrongLength)?;
    *cursor = end;
    Ok(u64::from_le_bytes(bytes))
}

fn require_live(bytes: [u8; 32]) -> Result<()> {
    if bytes.iter().all(|byte| *byte == 0) {
        Err(Error::BindingMismatch)
    } else {
        Ok(())
    }
}

const _: () = assert!(
    HEADER_BYTES_V2
        + IMMUTABLE_ID_COUNT_V2 * ID_BYTES_V2
        + AMOUNT_COUNT_V2 * 8
        + DYNAMIC_ID_COUNT_V2 * ID_BYTES_V2
        <= FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2
);

#[cfg(test)]
pub(crate) fn runtime_test_fixture(
    admission: FailureMarketAdmissionStateV1,
) -> (
    FailureMarketIntervalFundingReceiptV2,
    FailureMarketIntervalHistoryV2,
) {
    let policy = admission.binding().facts();
    let facts = FailureMarketIntervalFundingFactsV2 {
        failure_policy_binding_id: admission.binding().id(),
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        prepaid_funding_receipt_id: ProductContentId::from_bytes([201; 32]),
        work_account: FailureMarketAccountIdV1::from_bytes([202; 32]),
        history_account: FailureMarketAccountIdV1::from_bytes([203; 32]),
        rent_refund_owner: FailureMarketAccountIdV1::from_bytes([204; 32]),
        neutral_sink: FailureMarketAccountIdV1::from_bytes([205; 32]),
        work_rent_principal_lamports: 100,
        history_rent_principal_lamports: 200,
        work_donation_floor_lamports: 3,
        work_observed_balance_lamports: 103,
        history_donation_floor_lamports: 5,
        history_observed_balance_lamports: 205,
    };
    let funding = FailureMarketIntervalFundingReceiptV2 {
        id: FailureMarketIntervalFundingReceiptIdV2::from_bytes([206; 32]),
        facts,
    };
    let history = FailureMarketIntervalHistoryV2 {
        failure_policy_binding_id: admission.binding().id(),
        market_instance_id: policy.market_instance_id,
        funding_receipt_id: funding.id,
        work_account: facts.work_account,
        history_account: facts.history_account,
        rent_refund_owner: facts.rent_refund_owner,
        neutral_sink: facts.neutral_sink,
        quote_admission_receipt_id: ProductContentId::from_bytes([207; 32]),
        generation: policy.generation,
        work_rent_principal_lamports: facts.work_rent_principal_lamports,
        history_rent_principal_lamports: facts.history_rent_principal_lamports,
        completed_session_count: 0,
        completed_work_calls: 0,
        exact_reward_lamports: 0,
        history_root: FailureMarketIntervalHistoryRootV2::from_bytes([0; 32]),
        latest_session_binding_id: ProductContentId::ZERO,
        latest_terminal_receipt_id: ProductContentId::ZERO,
        latest_terminal_state_commitment: ProductContentId::ZERO,
        family_terminal_receipt_id: FailureMarketFamilyTerminalReceiptIdV1::from_bytes([0; 32]),
    };
    (funding, history)
}

#[cfg(test)]
pub(crate) fn runtime_test_append(
    history: FailureMarketIntervalHistoryV2,
    session_binding_id: ProductContentId,
    terminal_state_commitment: ProductContentId,
    idle_state_commitment: ProductContentId,
    session_terminal_receipt_id: ProductContentId,
    seed: u8,
) -> (
    FailureMarketIntervalHistoryV2,
    FailureMarketIntervalHistoryAppendReceiptV2,
) {
    let history_before = history.id().unwrap();
    let resulting_root = FailureMarketIntervalHistoryRootV2::from_bytes([seed; 32]);
    let mut after = history;
    after.completed_session_count = history.completed_session_count.checked_add(1).unwrap();
    after.history_root = resulting_root;
    after.latest_session_binding_id = session_binding_id;
    after.latest_terminal_receipt_id = session_terminal_receipt_id;
    after.latest_terminal_state_commitment = terminal_state_commitment;
    let history_after = after.id().unwrap();
    let receipt = FailureMarketIntervalHistoryAppendReceiptV2 {
        id: FailureMarketIntervalHistoryAppendReceiptIdV2::from_bytes([seed.wrapping_add(2); 32]),
        failure_policy_binding_id: history.failure_policy_binding_id,
        market_instance_id: history.market_instance_id,
        generation: history.generation,
        funding_receipt_id: history.funding_receipt_id,
        work_account: history.work_account,
        history_account: history.history_account,
        history_before,
        history_after,
        previous_root: history.history_root,
        resulting_root,
        session_binding_id,
        session_terminal_receipt_id,
        terminal_state_commitment,
        idle_state_commitment,
        completed_session_count: after.completed_session_count,
    };
    (after, receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_history() -> FailureMarketIntervalHistoryV2 {
        FailureMarketIntervalHistoryV2 {
            failure_policy_binding_id: FailurePolicyBindingId::from_bytes([1; 32]),
            market_instance_id: MarketInstanceV2Id::from_bytes([2; 32]),
            funding_receipt_id: FailureMarketIntervalFundingReceiptIdV2::from_bytes([3; 32]),
            work_account: FailureMarketAccountIdV1::from_bytes([4; 32]),
            history_account: FailureMarketAccountIdV1::from_bytes([5; 32]),
            rent_refund_owner: FailureMarketAccountIdV1::from_bytes([6; 32]),
            neutral_sink: FailureMarketAccountIdV1::from_bytes([7; 32]),
            quote_admission_receipt_id: ProductContentId::from_bytes([8; 32]),
            generation: 1,
            work_rent_principal_lamports: 100,
            history_rent_principal_lamports: 200,
            completed_session_count: 0,
            completed_work_calls: 0,
            exact_reward_lamports: 0,
            history_root: FailureMarketIntervalHistoryRootV2::from_bytes([0; 32]),
            latest_session_binding_id: ProductContentId::ZERO,
            latest_terminal_receipt_id: ProductContentId::ZERO,
            latest_terminal_state_commitment: ProductContentId::ZERO,
            family_terminal_receipt_id: FailureMarketFamilyTerminalReceiptIdV1::from_bytes([0; 32]),
        }
    }

    fn completed_history() -> FailureMarketIntervalHistoryV2 {
        let mut history = empty_history();
        history.completed_session_count = 1;
        history.completed_work_calls = 2;
        history.exact_reward_lamports = 40;
        history.history_root = FailureMarketIntervalHistoryRootV2::from_bytes([9; 32]);
        history.latest_session_binding_id = ProductContentId::from_bytes([10; 32]);
        history.latest_terminal_receipt_id = ProductContentId::from_bytes([11; 32]);
        history.latest_terminal_state_commitment = ProductContentId::from_bytes([12; 32]);
        history
    }

    #[test]
    fn history_refuses_partial_overwrite_and_stale_sibling() {
        let mut history = empty_history();
        history.validate().unwrap();
        let mut partial = history;
        partial.completed_session_count = 1;
        partial.history_root = FailureMarketIntervalHistoryRootV2::from_bytes([9; 32]);
        assert_eq!(partial.validate(), Err(Error::BindingMismatch));

        let after = completed_history();
        let plan = FailureMarketIntervalHistoryPlanV2 {
            before: history,
            after,
        };
        history.commit_plan(plan).unwrap();
        assert_eq!(history.commit_plan(plan), Err(Error::StalePlan));
    }

    #[test]
    fn terminal_work_receipt_and_reverse_close_are_exact() {
        let history = completed_history();
        let mut terminal = FailureMarketIntervalTerminalFactsV2 {
            history_before: history.id().unwrap(),
            session_binding_id: ProductContentId::from_bytes([20; 32]),
            session_terminal_receipt_id: ProductContentId::from_bytes([21; 32]),
            terminal_state_commitment: ProductContentId::from_bytes([22; 32]),
            idle_state_commitment: ProductContentId::from_bytes([24; 32]),
            last_liveness_work_receipt_id: ProductContentId::from_bytes([23; 32]),
            disposition: FailureMarketIntervalTerminalDispositionV2::Resolved,
            completed_work_calls: 0,
            exact_reward_lamports: 0,
        };
        assert_eq!(
            validate_terminal_facts(history, terminal),
            Err(Error::BindingMismatch)
        );
        terminal.last_liveness_work_receipt_id = ProductContentId::ZERO;
        validate_terminal_facts(history, terminal).unwrap();

        let mut sealed = history;
        sealed.family_terminal_receipt_id =
            FailureMarketFamilyTerminalReceiptIdV1::from_bytes([25; 32]);
        let seal = FailureMarketIntervalFamilySealReceiptV2 {
            id: FailureMarketIntervalFamilySealReceiptIdV2::from_bytes([26; 32]),
            facts: FailureMarketIntervalFamilySealFactsV2 {
                history_before: history.id().unwrap(),
                history_root: history.history_root,
                completed_session_count: 1,
                completed_work_calls: 2,
                exact_reward_lamports: 40,
                family_terminal_receipt_id: sealed.family_terminal_receipt_id,
            },
            history_after: sealed.id().unwrap(),
        };
        assert_eq!(
            plan_close_failure_market_interval_accounts_v2(sealed, seal, 99, 207),
            Err(Error::BindingMismatch)
        );
        let close = plan_close_failure_market_interval_accounts_v2(sealed, seal, 103, 207).unwrap();
        assert_eq!(close.work_rent_refund_lamports, 100);
        assert_eq!(close.history_rent_refund_lamports, 200);
        assert_eq!(close.work_donation_lamports, 3);
        assert_eq!(close.history_donation_lamports, 7);
    }
}
