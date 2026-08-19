#![no_std]
#![forbid(unsafe_code)]

//! Hostile state model for the R4 failure-payout decision.
//!
//! The selected policy has no numeric failure vector. Missing evidence freezes
//! new exposure and enters recoverable degradation; a finite independently
//! prepaid repair budget is spent or neutralized, after which later valid
//! evidence may still resolve the market. Claim principal never pays for work.
//! New native bearer units represent a conservative universal exact lot, so
//! ordinary Token-2022 transfers cannot create fractional bearer units.
//!
//! This is MODEL-ONLY. `actual_external_tokens` stands for current authenticated
//! Token-2022 mint supply; a real adapter still owes mint/account/PDA/program
//! authentication, holder authorization, atomic CPI, and exact post-state reads.

pub const MAX_OUTCOMES: usize = 16;
/// Abstract identity for the canonical SDK incinerator in this pure model.
/// The SBF adapter must bind this role to `solana_sdk_ids::incinerator::ID`.
pub const REPAIR_NEUTRAL_INCINERATOR: u64 = u64::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidConfig,
    InvalidWeights,
    WrongPhase,
    ZeroQuantity,
    NonIntegralLot,
    InsufficientCash,
    InsufficientClaims,
    InsufficientRepairReserve,
    RepairWindowClosed,
    RepairDeadlineNotReached,
    ImpossibleExternalIncrease,
    ExternalTruthMismatch,
    StaleExternalTruth,
    NoFailurePayout,
    OutstandingClaims,
    OutstandingCash,
    OutstandingCredits,
    OutstandingRepairWork,
    OutstandingDependencies,
    ArithmeticOverflow,
    InvariantViolation,
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Active,
    DegradedRecoverable,
    RecoveryDormant,
    Resolved,
    Terminal,
}

/// One integer-simplex payout vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayoutVector {
    pub outcome_count: u8,
    pub denominator: u64,
    pub weights: [u64; MAX_OUTCOMES],
}

impl PayoutVector {
    pub fn validate(&self) -> Result<()> {
        let count = usize::from(self.outcome_count);
        if !(2..=MAX_OUTCOMES).contains(&count) || self.denominator == 0 {
            return Err(Error::InvalidWeights);
        }
        let mut sum = 0_u64;
        let mut i = 0_usize;
        while i < MAX_OUTCOMES {
            if i < count {
                if self.weights[i] > self.denominator {
                    return Err(Error::InvalidWeights);
                }
                sum = sum
                    .checked_add(self.weights[i])
                    .ok_or(Error::ArithmeticOverflow)?;
            } else if self.weights[i] != 0 {
                return Err(Error::InvalidWeights);
            }
            i += 1;
        }
        if sum != self.denominator {
            return Err(Error::InvalidWeights);
        }
        Ok(())
    }
}

/// Directional redistribution created by replacing one successful vector with
/// a fixed failure vector. Because both vectors sum to the same denominator,
/// every unequal pair has at least one positive and one negative coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FallbackDelta {
    pub has_gainer: bool,
    pub has_loser: bool,
}

pub fn fallback_delta(failure: PayoutVector, success: PayoutVector) -> Result<FallbackDelta> {
    failure.validate()?;
    success.validate()?;
    if failure.outcome_count != success.outcome_count || failure.denominator != success.denominator
    {
        return Err(Error::InvalidWeights);
    }
    let mut has_gainer = false;
    let mut has_loser = false;
    let mut i = 0_usize;
    while i < usize::from(failure.outcome_count) {
        has_gainer |= failure.weights[i] > success.weights[i];
        has_loser |= failure.weights[i] < success.weights[i];
        i += 1;
    }
    Ok(FallbackDelta {
        has_gainer,
        has_loser,
    })
}

/// Fixed-width economic state. External supplies are Token-2022 token atoms;
/// each token atom represents `universal_lot` raw native claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Model {
    pub phase: Phase,
    pub outcome_count: u8,
    pub denominator: u64,
    pub universal_lot: u64,
    pub internal_raw: [u64; MAX_OUTCOMES],
    pub actual_external_tokens: [u64; MAX_OUTCOMES],
    pub observed_external_tokens: [u64; MAX_OUTCOMES],
    pub resolved_weights: [u64; MAX_OUTCOMES],

    /// Actual collateral atoms held by the Hoard token account.
    pub hoard: u64,
    /// Retained claim backing. This is never a repair, rent, or reward pool.
    pub locked: u64,
    /// Aggregate owner cash. Reserved cash, if modeled, is a subset of this.
    pub cash: u64,
    /// Direct unowned Hoard donations, distinct from burn-created slack.
    pub direct_surplus: u64,

    /// Explicit imported credit liability in numerator units. V1 creates none.
    pub credit_numerator_total: u64,

    /// Independent SOL repair compartment.
    pub repair_initial: u64,
    /// Physical lamports still held in the modeled repair reserve account.
    pub repair_account_balance: u64,
    pub repair_reserved: u64,
    pub repair_keeper_paid: u64,
    pub repair_payer_refund: u64,
    pub repair_neutral_incinerated: u64,
    pub repair_neutral_sink: u64,
    pub repair_attempt_limit: u16,
    pub repair_attempts_used: u16,

    /// Whole collateral atoms destroyed under the terminal burn policy.
    pub terminal_collateral_burned: u64,

    /// Terminal prerequisites modeled only as hostile aggregate gates. Their
    /// component lifecycles remain separate runtime work.
    pub open_reservations: u16,
    pub source_archive_references: u16,
    pub unclosed_refundable_accounts: u16,
}

impl Model {
    /// Create an exact-lot market. Requiring `universal_lot` to be a multiple
    /// of `D` is conservative for every integer-simplex vector.
    pub fn new(
        outcome_count: u8,
        denominator: u64,
        universal_lot: u64,
        initial_cash: u64,
        repair_reserve: u64,
        repair_attempt_limit: u16,
    ) -> Result<Self> {
        if !(2..=MAX_OUTCOMES as u8).contains(&outcome_count)
            || denominator == 0
            || universal_lot == 0
            || !universal_lot.is_multiple_of(denominator)
            || repair_reserve == 0
            || repair_attempt_limit == 0
        {
            return Err(Error::InvalidConfig);
        }
        let model = Self {
            phase: Phase::Active,
            outcome_count,
            denominator,
            universal_lot,
            internal_raw: [0; MAX_OUTCOMES],
            actual_external_tokens: [0; MAX_OUTCOMES],
            observed_external_tokens: [0; MAX_OUTCOMES],
            resolved_weights: [0; MAX_OUTCOMES],
            hoard: initial_cash,
            locked: 0,
            cash: initial_cash,
            direct_surplus: 0,
            credit_numerator_total: 0,
            repair_initial: repair_reserve,
            repair_account_balance: repair_reserve,
            repair_reserved: repair_reserve,
            repair_keeper_paid: 0,
            repair_payer_refund: 0,
            repair_neutral_incinerated: 0,
            repair_neutral_sink: REPAIR_NEUTRAL_INCINERATOR,
            repair_attempt_limit,
            repair_attempts_used: 0,
            terminal_collateral_burned: 0,
            open_reservations: 0,
            source_archive_references: 0,
            unclosed_refundable_accounts: 0,
        };
        model.check()?;
        Ok(model)
    }

    pub fn check(&self) -> Result<()> {
        let count = usize::from(self.outcome_count);
        if !(2..=MAX_OUTCOMES).contains(&count)
            || self.denominator == 0
            || self.universal_lot == 0
            || !self.universal_lot.is_multiple_of(self.denominator)
            || self.repair_initial == 0
            || self.repair_neutral_sink != REPAIR_NEUTRAL_INCINERATOR
            || self.repair_attempt_limit == 0
            || self.repair_attempts_used > self.repair_attempt_limit
        {
            return Err(Error::InvariantViolation);
        }
        let accounted_hoard = self
            .locked
            .checked_add(self.cash)
            .and_then(|v| v.checked_add(self.direct_surplus))
            .ok_or(Error::ArithmeticOverflow)?;
        if accounted_hoard != self.hoard {
            return Err(Error::InvariantViolation);
        }
        let accounted_repair = self
            .repair_reserved
            .checked_add(self.repair_keeper_paid)
            .and_then(|v| v.checked_add(self.repair_payer_refund))
            .and_then(|v| v.checked_add(self.repair_neutral_incinerated))
            .ok_or(Error::ArithmeticOverflow)?;
        if accounted_repair != self.repair_initial {
            return Err(Error::InvariantViolation);
        }
        if self.repair_account_balance != self.repair_reserved {
            return Err(Error::InvariantViolation);
        }

        let mut conservative = [0_u64; MAX_OUTCOMES];
        let mut i = 0_usize;
        while i < MAX_OUTCOMES {
            if i >= count {
                if self.internal_raw[i] != 0
                    || self.actual_external_tokens[i] != 0
                    || self.observed_external_tokens[i] != 0
                    || self.resolved_weights[i] != 0
                {
                    return Err(Error::InvariantViolation);
                }
                i += 1;
                continue;
            }
            if !self.internal_raw[i].is_multiple_of(self.universal_lot)
                || self.actual_external_tokens[i] > self.observed_external_tokens[i]
            {
                return Err(Error::InvariantViolation);
            }
            let observed_raw = self.observed_external_tokens[i]
                .checked_mul(self.universal_lot)
                .ok_or(Error::ArithmeticOverflow)?;
            conservative[i] = self.internal_raw[i]
                .checked_add(observed_raw)
                .ok_or(Error::ArithmeticOverflow)?;
            i += 1;
        }

        match self.phase {
            Phase::Active | Phase::DegradedRecoverable | Phase::RecoveryDormant => {
                if self.resolved_weights.iter().any(|weight| *weight != 0)
                    || self.credit_numerator_total != 0
                {
                    return Err(Error::InvariantViolation);
                }
                let mut required = 0_u64;
                let mut j = 0_usize;
                while j < count {
                    required = required.max(conservative[j]);
                    j += 1;
                }
                if self.locked < required {
                    return Err(Error::InvariantViolation);
                }
            }
            Phase::Resolved => {
                let vector = self.resolution_vector();
                vector.validate()?;
                let mut numerator = u128::from(self.credit_numerator_total);
                let mut j = 0_usize;
                while j < count {
                    let term = u128::from(conservative[j])
                        .checked_mul(u128::from(self.resolved_weights[j]))
                        .ok_or(Error::ArithmeticOverflow)?;
                    numerator = numerator
                        .checked_add(term)
                        .ok_or(Error::ArithmeticOverflow)?;
                    j += 1;
                }
                let backing = u128::from(self.locked)
                    .checked_mul(u128::from(self.denominator))
                    .ok_or(Error::ArithmeticOverflow)?;
                if backing < numerator {
                    return Err(Error::InvariantViolation);
                }
            }
            Phase::Terminal => {
                if self.hoard != 0
                    || self.locked != 0
                    || self.cash != 0
                    || self.direct_surplus != 0
                    || self.credit_numerator_total != 0
                    || self.repair_reserved != 0
                    || self.open_reservations != 0
                    || self.source_archive_references != 0
                    || self.unclosed_refundable_accounts != 0
                    || self.internal_raw.iter().any(|quantity| *quantity != 0)
                    || self
                        .actual_external_tokens
                        .iter()
                        .any(|quantity| *quantity != 0)
                    || self
                        .observed_external_tokens
                        .iter()
                        .any(|quantity| *quantity != 0)
                {
                    return Err(Error::InvariantViolation);
                }
            }
        }
        Ok(())
    }

    pub fn split(&mut self, raw_quantity: u64) -> Result<()> {
        self.check()?;
        if self.phase != Phase::Active {
            return Err(Error::WrongPhase);
        }
        self.require_lot(raw_quantity)?;
        if self.cash < raw_quantity {
            return Err(Error::InsufficientCash);
        }
        let mut next = *self;
        next.cash -= raw_quantity;
        next.locked = next
            .locked
            .checked_add(raw_quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut i = 0_usize;
        while i < usize::from(next.outcome_count) {
            next.internal_raw[i] = next.internal_raw[i]
                .checked_add(raw_quantity)
                .ok_or(Error::ArithmeticOverflow)?;
            i += 1;
        }
        next.check()?;
        *self = next;
        Ok(())
    }

    /// Complete-set exit remains exact before resolution and in either failure
    /// state. It does not invent an individual-outcome failure payout.
    pub fn merge_complete_set(&mut self, raw_quantity: u64) -> Result<()> {
        self.check()?;
        if self.phase == Phase::Terminal {
            return Err(Error::WrongPhase);
        }
        self.require_lot(raw_quantity)?;
        let mut i = 0_usize;
        while i < usize::from(self.outcome_count) {
            if self.internal_raw[i] < raw_quantity {
                return Err(Error::InsufficientClaims);
            }
            i += 1;
        }
        if self.locked < raw_quantity {
            return Err(Error::InvariantViolation);
        }
        let mut next = *self;
        let mut j = 0_usize;
        while j < usize::from(next.outcome_count) {
            next.internal_raw[j] -= raw_quantity;
            j += 1;
        }
        next.locked -= raw_quantity;
        next.cash = next
            .cash
            .checked_add(raw_quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        next.check()?;
        *self = next;
        Ok(())
    }

    /// Move internal raw claims into ordinary Token-2022 bearer token atoms.
    /// The bridge stays exact in degraded/dormant states so holders may
    /// voluntarily aggregate; it is closed only after resolution/terminality.
    pub fn materialize(&mut self, outcome: u8, raw_quantity: u64) -> Result<()> {
        self.check()?;
        if !matches!(
            self.phase,
            Phase::Active | Phase::DegradedRecoverable | Phase::RecoveryDormant
        ) {
            return Err(Error::WrongPhase);
        }
        let i = self.outcome(outcome)?;
        self.require_lot(raw_quantity)?;
        if self.internal_raw[i] < raw_quantity {
            return Err(Error::InsufficientClaims);
        }
        let token_quantity = raw_quantity / self.universal_lot;
        let mut next = *self;
        next.internal_raw[i] -= raw_quantity;
        next.actual_external_tokens[i] = next.actual_external_tokens[i]
            .checked_add(token_quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        next.observed_external_tokens[i] = next.observed_external_tokens[i]
            .checked_add(token_quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        next.check()?;
        *self = next;
        Ok(())
    }

    /// Burn whole bearer token atoms into an internal Position. A real adapter
    /// must authenticate the presented holder account and exact Token-2022 burn.
    pub fn dematerialize(&mut self, outcome: u8, token_quantity: u64) -> Result<()> {
        self.check()?;
        if !matches!(
            self.phase,
            Phase::Active | Phase::DegradedRecoverable | Phase::RecoveryDormant
        ) {
            return Err(Error::WrongPhase);
        }
        if token_quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        if self.actual_external_tokens != self.observed_external_tokens {
            return Err(Error::StaleExternalTruth);
        }
        let i = self.outcome(outcome)?;
        if self.actual_external_tokens[i] < token_quantity {
            return Err(Error::InsufficientClaims);
        }
        let raw_quantity = token_quantity
            .checked_mul(self.universal_lot)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut next = *self;
        next.actual_external_tokens[i] -= token_quantity;
        next.observed_external_tokens[i] -= token_quantity;
        next.internal_raw[i] = next.internal_raw[i]
            .checked_add(raw_quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        next.check()?;
        *self = next;
        Ok(())
    }

    /// Simulate an ordinary holder burn outside the protocol. Actual
    /// Token-2022 supply falls; the program cache remains conservatively stale.
    pub fn direct_bearer_burn(&mut self, outcome: u8, token_quantity: u64) -> Result<()> {
        self.check()?;
        let i = self.outcome(outcome)?;
        if token_quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        if self.actual_external_tokens[i] < token_quantity {
            return Err(Error::InsufficientClaims);
        }
        let mut next = *self;
        next.actual_external_tokens[i] -= token_quantity;
        next.check()?;
        *self = next;
        Ok(())
    }

    /// Synchronize the complete authoritative mint-supply vector. Any increase
    /// above the last program-observed value is an impossible mint and refuses.
    pub fn synchronize_external_truth(&mut self, authoritative: [u64; MAX_OUTCOMES]) -> Result<()> {
        self.authenticate_external_vector(&authoritative)?;
        self.check()?;
        let mut next = *self;
        let mut i = 0_usize;
        while i < MAX_OUTCOMES {
            next.observed_external_tokens[i] = authoritative[i];
            i += 1;
        }
        next.check()?;
        *self = next;
        Ok(())
    }

    pub fn enter_degraded(&mut self) -> Result<()> {
        self.check()?;
        if self.phase != Phase::Active {
            return Err(Error::WrongPhase);
        }
        let mut next = *self;
        next.phase = Phase::DegradedRecoverable;
        next.check()?;
        *self = next;
        Ok(())
    }

    /// Pay one accepted, unsuccessful repair attempt from the SOL work reserve.
    /// The Hoard and every claim field are invariant.
    pub fn pay_failed_repair(&mut self, keeper_payment: u64) -> Result<()> {
        self.check()?;
        if self.phase != Phase::DegradedRecoverable {
            return Err(Error::WrongPhase);
        }
        if keeper_payment == 0 {
            return Err(Error::ZeroQuantity);
        }
        if self.repair_attempts_used == self.repair_attempt_limit {
            return Err(Error::RepairWindowClosed);
        }
        if self.repair_reserved < keeper_payment {
            return Err(Error::InsufficientRepairReserve);
        }
        let mut next = *self;
        next.repair_reserved -= keeper_payment;
        next.repair_account_balance -= keeper_payment;
        next.repair_keeper_paid = next
            .repair_keeper_paid
            .checked_add(keeper_payment)
            .ok_or(Error::ArithmeticOverflow)?;
        next.repair_attempts_used += 1;
        if next.repair_attempts_used == next.repair_attempt_limit {
            next.neutralize_repair_residue()?;
            next.phase = Phase::RecoveryDormant;
        }
        next.check()?;
        *self = next;
        Ok(())
    }

    /// Close a finite repair window. Failure residue goes to the immutable
    /// neutral disposition, never to creator, resolver, claimant, or treasury.
    pub fn close_repair_window(&mut self, authenticated_deadline_reached: bool) -> Result<()> {
        self.check()?;
        if self.phase != Phase::DegradedRecoverable {
            return Err(Error::WrongPhase);
        }
        if !authenticated_deadline_reached {
            return Err(Error::RepairDeadlineNotReached);
        }
        let mut next = *self;
        next.neutralize_repair_residue()?;
        next.phase = Phase::RecoveryDormant;
        next.check()?;
        *self = next;
        Ok(())
    }

    /// No call can select a payout merely because repair ended.
    pub const fn failure_payout(&self) -> Result<u64> {
        Err(Error::NoFailurePayout)
    }

    /// Resolve only from a fully validated evidence vector. In Active mode the
    /// repair payment must be zero; in dormant mode any submission is caller-
    /// funded because the finite reserve has already terminated.
    pub fn resolve_from_evidence(
        &mut self,
        vector: PayoutVector,
        authoritative_external: [u64; MAX_OUTCOMES],
        keeper_payment: u64,
    ) -> Result<()> {
        vector.validate()?;
        self.authenticate_external_vector(&authoritative_external)?;
        self.check()?;
        if vector.outcome_count != self.outcome_count || vector.denominator != self.denominator {
            return Err(Error::InvalidWeights);
        }
        match self.phase {
            Phase::Active if keeper_payment == 0 => {}
            Phase::DegradedRecoverable => {
                if keeper_payment > self.repair_reserved {
                    return Err(Error::InsufficientRepairReserve);
                }
            }
            Phase::RecoveryDormant if keeper_payment == 0 => {}
            _ => return Err(Error::WrongPhase),
        }

        let mut next = *self;
        next.synchronize_external_truth(authoritative_external)?;
        if matches!(next.phase, Phase::Active | Phase::DegradedRecoverable) {
            if next.phase == Phase::DegradedRecoverable {
                next.repair_reserved -= keeper_payment;
                next.repair_account_balance -= keeper_payment;
                next.repair_keeper_paid = next
                    .repair_keeper_paid
                    .checked_add(keeper_payment)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            next.repair_payer_refund = next
                .repair_payer_refund
                .checked_add(next.repair_reserved)
                .ok_or(Error::ArithmeticOverflow)?;
            next.repair_reserved = 0;
            next.repair_account_balance = 0;
        }
        next.resolved_weights = vector.weights;
        next.phase = Phase::Resolved;
        next.check()?;
        *self = next;
        Ok(())
    }

    pub fn redeem_internal(&mut self, outcome: u8, raw_quantity: u64) -> Result<u64> {
        self.check()?;
        if self.phase != Phase::Resolved {
            return Err(Error::WrongPhase);
        }
        let i = self.outcome(outcome)?;
        self.require_lot(raw_quantity)?;
        if self.internal_raw[i] < raw_quantity {
            return Err(Error::InsufficientClaims);
        }
        let payout = self.exact_payout(raw_quantity, self.resolved_weights[i])?;
        if self.locked < payout {
            return Err(Error::InvariantViolation);
        }
        let mut next = *self;
        next.internal_raw[i] -= raw_quantity;
        next.locked -= payout;
        next.cash = next
            .cash
            .checked_add(payout)
            .ok_or(Error::ArithmeticOverflow)?;
        next.check()?;
        *self = next;
        Ok(payout)
    }

    pub fn redeem_external(&mut self, outcome: u8, token_quantity: u64) -> Result<u64> {
        self.check()?;
        if self.phase != Phase::Resolved {
            return Err(Error::WrongPhase);
        }
        if token_quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        if self.actual_external_tokens != self.observed_external_tokens {
            return Err(Error::StaleExternalTruth);
        }
        let i = self.outcome(outcome)?;
        if self.actual_external_tokens[i] < token_quantity {
            return Err(Error::InsufficientClaims);
        }
        let raw = token_quantity
            .checked_mul(self.universal_lot)
            .ok_or(Error::ArithmeticOverflow)?;
        let payout = self.exact_payout(raw, self.resolved_weights[i])?;
        if self.locked < payout || self.hoard < payout {
            return Err(Error::InvariantViolation);
        }
        let mut next = *self;
        next.actual_external_tokens[i] -= token_quantity;
        next.observed_external_tokens[i] -= token_quantity;
        next.locked -= payout;
        next.hoard -= payout;
        next.check()?;
        *self = next;
        Ok(payout)
    }

    pub fn withdraw_cash(&mut self, quantity: u64) -> Result<()> {
        self.check()?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        if self.cash < quantity || self.hoard < quantity {
            return Err(Error::InsufficientCash);
        }
        let mut next = *self;
        next.cash -= quantity;
        next.hoard -= quantity;
        next.check()?;
        *self = next;
        Ok(())
    }

    pub fn donate_hoard(&mut self, quantity: u64) -> Result<()> {
        self.check()?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        let mut next = *self;
        next.hoard = next
            .hoard
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        next.direct_surplus = next
            .direct_surplus
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        next.check()?;
        *self = next;
        Ok(())
    }

    /// Release unused work funding only when no claim liability remains. This
    /// is cancellation of unnecessary work, not a data-failure refund.
    pub fn release_repair_after_zero_claims(&mut self) -> Result<()> {
        self.check()?;
        if self.actual_external_tokens != self.observed_external_tokens {
            return Err(Error::StaleExternalTruth);
        }
        if !self.claims_are_authoritatively_zero() {
            return Err(Error::OutstandingClaims);
        }
        let mut next = *self;
        next.repair_payer_refund = next
            .repair_payer_refund
            .checked_add(next.repair_reserved)
            .ok_or(Error::ArithmeticOverflow)?;
        next.repair_reserved = 0;
        next.repair_account_balance = 0;
        if matches!(next.phase, Phase::Active | Phase::DegradedRecoverable) {
            next.phase = Phase::RecoveryDormant;
        }
        next.check()?;
        *self = next;
        Ok(())
    }

    /// Burn all remaining whole collateral atoms only after every owner claim,
    /// cash balance, imported numerator credit, and repair reservation is zero.
    pub fn terminal_burn_surplus(&mut self) -> Result<u64> {
        self.check()?;
        if self.actual_external_tokens != self.observed_external_tokens {
            return Err(Error::StaleExternalTruth);
        }
        if !self.claims_are_authoritatively_zero() || self.locked_claim_cache_nonzero() {
            return Err(Error::OutstandingClaims);
        }
        if self.cash != 0 {
            return Err(Error::OutstandingCash);
        }
        if self.credit_numerator_total != 0 {
            return Err(Error::OutstandingCredits);
        }
        if self.repair_reserved != 0 {
            return Err(Error::OutstandingRepairWork);
        }
        if self.open_reservations != 0
            || self.source_archive_references != 0
            || self.unclosed_refundable_accounts != 0
        {
            return Err(Error::OutstandingDependencies);
        }
        let amount = self.hoard;
        let mut next = *self;
        next.terminal_collateral_burned = next
            .terminal_collateral_burned
            .checked_add(amount)
            .ok_or(Error::ArithmeticOverflow)?;
        next.hoard = 0;
        next.locked = 0;
        next.direct_surplus = 0;
        next.phase = Phase::Terminal;
        next.check()?;
        *self = next;
        Ok(amount)
    }

    fn resolution_vector(&self) -> PayoutVector {
        PayoutVector {
            outcome_count: self.outcome_count,
            denominator: self.denominator,
            weights: self.resolved_weights,
        }
    }

    fn outcome(&self, outcome: u8) -> Result<usize> {
        if outcome >= self.outcome_count {
            return Err(Error::InvalidWeights);
        }
        Ok(usize::from(outcome))
    }

    fn require_lot(&self, quantity: u64) -> Result<()> {
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        if !quantity.is_multiple_of(self.universal_lot) {
            return Err(Error::NonIntegralLot);
        }
        Ok(())
    }

    fn exact_payout(&self, raw_quantity: u64, weight: u64) -> Result<u64> {
        let numerator = u128::from(raw_quantity)
            .checked_mul(u128::from(weight))
            .ok_or(Error::ArithmeticOverflow)?;
        let denominator = u128::from(self.denominator);
        if numerator % denominator != 0 {
            return Err(Error::NonIntegralLot);
        }
        u64::try_from(numerator / denominator).map_err(|_| Error::ArithmeticOverflow)
    }

    fn neutralize_repair_residue(&mut self) -> Result<()> {
        self.repair_neutral_incinerated = self
            .repair_neutral_incinerated
            .checked_add(self.repair_reserved)
            .ok_or(Error::ArithmeticOverflow)?;
        self.repair_reserved = 0;
        self.repair_account_balance = 0;
        Ok(())
    }

    fn authenticate_external_vector(&self, authoritative: &[u64; MAX_OUTCOMES]) -> Result<()> {
        if authoritative != &self.actual_external_tokens {
            return Err(Error::ExternalTruthMismatch);
        }
        let mut i = 0_usize;
        while i < MAX_OUTCOMES {
            if self.actual_external_tokens[i] > self.observed_external_tokens[i] {
                return Err(Error::ImpossibleExternalIncrease);
            }
            i += 1;
        }
        Ok(())
    }

    fn claims_are_authoritatively_zero(&self) -> bool {
        self.internal_raw.iter().all(|quantity| *quantity == 0)
            && self
                .actual_external_tokens
                .iter()
                .all(|quantity| *quantity == 0)
    }

    fn locked_claim_cache_nonzero(&self) -> bool {
        self.observed_external_tokens
            .iter()
            .any(|quantity| *quantity != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector(denominator: u64, left: u64) -> PayoutVector {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[0] = left;
        weights[1] = denominator - left;
        PayoutVector {
            outcome_count: 2,
            denominator,
            weights,
        }
    }

    fn supplies(left: u64, right: u64) -> [u64; MAX_OUTCOMES] {
        let mut values = [0_u64; MAX_OUTCOMES];
        values[0] = left;
        values[1] = right;
        values
    }

    #[test]
    fn every_unequal_fixed_fallback_has_a_gainer_and_loser() {
        let mut d = 1_u64;
        while d <= 32 {
            let mut f = 0_u64;
            while f <= d {
                let mut v = 0_u64;
                while v <= d {
                    let delta = fallback_delta(vector(d, f), vector(d, v)).unwrap();
                    if f == v {
                        assert_eq!(
                            delta,
                            FallbackDelta {
                                has_gainer: false,
                                has_loser: false
                            }
                        );
                    } else {
                        assert!(delta.has_gainer && delta.has_loser);
                    }
                    v += 1;
                }
                f += 1;
            }
            d += 1;
        }
    }

    #[test]
    fn mandatory_repair_cannot_be_admitted_with_zero_reserve() {
        assert_eq!(Model::new(2, 4, 4, 8, 0, 1), Err(Error::InvalidConfig));
    }

    #[test]
    fn zero_volume_and_team_disappearance_never_touch_principal() {
        let mut m = Model::new(2, 8, 8, 80, 21, 3).unwrap();
        m.split(16).unwrap();
        m.enter_degraded().unwrap();
        let principal = (m.hoard, m.locked, m.cash, m.internal_raw);
        m.pay_failed_repair(7).unwrap();
        m.pay_failed_repair(7).unwrap();
        m.pay_failed_repair(7).unwrap();
        assert_eq!(m.phase, Phase::RecoveryDormant);
        assert_eq!((m.hoard, m.locked, m.cash, m.internal_raw), principal);
        assert_eq!(m.failure_payout(), Err(Error::NoFailurePayout));
        assert_eq!(m.repair_keeper_paid, 21);
        assert_eq!(m.repair_reserved, 0);
        assert_eq!(m.repair_account_balance, 0);
        assert_eq!(m.repair_neutral_incinerated, 0);
        assert_eq!(m.repair_neutral_sink, REPAIR_NEUTRAL_INCINERATOR);
    }

    #[test]
    fn repair_shortfall_refuses_atomically_instead_of_using_hoard() {
        let mut m = Model::new(2, 4, 4, 40, 5, 2).unwrap();
        m.split(8).unwrap();
        m.enter_degraded().unwrap();
        let before = m;
        assert_eq!(
            m.pay_failed_repair(6),
            Err(Error::InsufficientRepairReserve)
        );
        assert_eq!(m, before);
    }

    #[test]
    fn repair_window_cannot_close_before_authenticated_deadline() {
        let mut m = Model::new(2, 4, 4, 8, 5, 2).unwrap();
        m.enter_degraded().unwrap();
        let before = m;
        assert_eq!(
            m.close_repair_window(false),
            Err(Error::RepairDeadlineNotReached)
        );
        assert_eq!(m, before);
        m.close_repair_window(true).unwrap();
        assert_eq!(m.repair_neutral_incinerated, 5);
        assert_eq!(m.repair_account_balance, 0);
    }

    #[test]
    fn releasing_active_repair_reserve_closes_new_exposure() {
        let mut m = Model::new(2, 4, 4, 8, 5, 2).unwrap();
        m.release_repair_after_zero_claims().unwrap();
        assert_eq!(m.phase, Phase::RecoveryDormant);
        assert_eq!(m.repair_payer_refund, 5);
        let before = m;
        assert_eq!(m.split(4), Err(Error::WrongPhase));
        assert_eq!(m, before);
    }

    #[test]
    fn dormant_market_accepts_later_valid_evidence_without_resurrecting_budget() {
        let mut m = Model::new(2, 4, 4, 20, 9, 2).unwrap();
        m.split(4).unwrap();
        m.enter_degraded().unwrap();
        m.close_repair_window(true).unwrap();
        assert_eq!(m.repair_neutral_incinerated, 9);
        assert_eq!(m.repair_account_balance, 0);
        m.resolve_from_evidence(vector(4, 3), [0; MAX_OUTCOMES], 0)
            .unwrap();
        assert_eq!(m.phase, Phase::Resolved);
        assert_eq!(m.repair_neutral_incinerated, 9);
    }

    #[test]
    fn exact_lot_token_units_survive_transfer_and_fractional_weights() {
        let mut m = Model::new(2, 8, 8, 32, 1, 1).unwrap();
        m.split(8).unwrap();
        let before = m;
        assert_eq!(m.materialize(0, 1), Err(Error::NonIntegralLot));
        assert_eq!(m, before);
        m.materialize(0, 8).unwrap();
        assert_eq!(m.actual_external_tokens[0], 1);
        m.resolve_from_evidence(vector(8, 3), supplies(1, 0), 0)
            .unwrap();
        assert_eq!(m.repair_payer_refund, 1);
        assert_eq!(m.repair_reserved, 0);
        assert_eq!(m.redeem_external(0, 1).unwrap(), 3);
    }

    #[test]
    fn dormant_bearers_can_reaggregate_a_complete_set_without_failure_payout() {
        let mut m = Model::new(2, 4, 4, 8, 3, 1).unwrap();
        m.split(4).unwrap();
        m.materialize(0, 4).unwrap();
        m.materialize(1, 4).unwrap();
        m.enter_degraded().unwrap();
        m.close_repair_window(true).unwrap();
        m.dematerialize(0, 1).unwrap();
        m.dematerialize(1, 1).unwrap();
        m.merge_complete_set(4).unwrap();
        assert_eq!(m.internal_raw, [0; MAX_OUTCOMES]);
        assert_eq!(m.actual_external_tokens, [0; MAX_OUTCOMES]);
        assert_eq!(m.locked, 0);
        assert_eq!(m.cash, 8);
        assert_eq!(m.failure_payout(), Err(Error::NoFailurePayout));
    }

    #[test]
    fn direct_bearer_burn_is_forfeiture_and_retains_backing() {
        let mut m = Model::new(2, 4, 4, 8, 1, 1).unwrap();
        m.split(4).unwrap();
        m.materialize(0, 4).unwrap();
        let locked = m.locked;
        m.direct_bearer_burn(0, 1).unwrap();
        assert_eq!(m.actual_external_tokens[0], 0);
        assert_eq!(m.observed_external_tokens[0], 1);
        assert_eq!(m.locked, locked);
        m.synchronize_external_truth([0; MAX_OUTCOMES]).unwrap();
        assert_eq!(m.locked, locked);
    }

    #[test]
    fn supplied_external_vector_cannot_forge_a_mint_increase() {
        let mut m = Model::new(2, 4, 4, 8, 1, 1).unwrap();
        m.split(4).unwrap();
        let before = m;
        assert_eq!(
            m.resolve_from_evidence(vector(4, 4), supplies(1, 0), 0),
            Err(Error::ExternalTruthMismatch)
        );
        assert_eq!(m, before);
    }

    #[test]
    fn supplied_external_vector_cannot_forge_a_live_bearer_downward() {
        let mut m = Model::new(2, 4, 4, 8, 1, 1).unwrap();
        m.split(4).unwrap();
        m.materialize(0, 4).unwrap();
        let before = m;
        assert_eq!(
            m.resolve_from_evidence(vector(4, 4), [0; MAX_OUTCOMES], 0),
            Err(Error::ExternalTruthMismatch)
        );
        assert_eq!(m, before);
        assert_eq!(m.actual_external_tokens[0], 1);
    }

    #[test]
    fn actual_impossible_token2022_increase_has_a_distinct_refusal() {
        let mut m = Model::new(2, 4, 4, 8, 1, 1).unwrap();
        m.actual_external_tokens[0] = 1;
        let before = m;
        assert_eq!(
            m.synchronize_external_truth(supplies(1, 0)),
            Err(Error::ImpossibleExternalIncrease)
        );
        assert_eq!(m, before);
    }

    #[test]
    fn abandoned_bearer_claim_prevents_terminal_disposition_indefinitely() {
        let mut m = Model::new(2, 4, 4, 12, 3, 1).unwrap();
        m.split(4).unwrap();
        m.materialize(0, 4).unwrap();
        m.enter_degraded().unwrap();
        m.close_repair_window(true).unwrap();
        m.withdraw_cash(8).unwrap();
        assert_eq!(m.terminal_burn_surplus(), Err(Error::OutstandingClaims));
        assert_eq!(m.phase, Phase::RecoveryDormant);
        assert_eq!(m.actual_external_tokens[0], 1);
    }

    #[test]
    fn fractional_credit_is_liability_not_terminal_remainder() {
        let mut m = Model::new(2, 4, 4, 1, 1, 1).unwrap();
        m.resolved_weights = vector(4, 1).weights;
        m.phase = Phase::Resolved;
        m.locked = 1;
        m.cash = 0;
        m.credit_numerator_total = 1;
        m.check().unwrap();
        assert_eq!(m.terminal_burn_surplus(), Err(Error::OutstandingCredits));
        assert_eq!(m.hoard, 1);
    }

    #[test]
    fn terminal_burn_gets_only_unowned_value_after_every_owner_exits() {
        let mut m = Model::new(2, 4, 4, 12, 5, 1).unwrap();
        m.split(4).unwrap();
        m.donate_hoard(3).unwrap();
        m.resolve_from_evidence(vector(4, 4), [0; MAX_OUTCOMES], 0)
            .unwrap();
        assert_eq!(m.redeem_internal(0, 4).unwrap(), 4);
        assert_eq!(m.redeem_internal(1, 4).unwrap(), 0);
        m.withdraw_cash(12).unwrap();
        m.release_repair_after_zero_claims().unwrap();
        assert_eq!(m.terminal_burn_surplus().unwrap(), 3);
        assert_eq!(m.phase, Phase::Terminal);
        assert_eq!(m.terminal_collateral_burned, 3);
        assert_eq!(m.hoard, 0);
    }

    #[test]
    fn holder_burn_backing_can_only_be_destroyed_after_authoritative_zero() {
        let mut m = Model::new(2, 4, 4, 8, 2, 1).unwrap();
        m.split(4).unwrap();
        m.materialize(0, 4).unwrap();
        m.materialize(1, 4).unwrap();
        m.direct_bearer_burn(0, 1).unwrap();
        m.direct_bearer_burn(1, 1).unwrap();
        m.synchronize_external_truth([0; MAX_OUTCOMES]).unwrap();
        m.withdraw_cash(4).unwrap();
        m.release_repair_after_zero_claims().unwrap();
        assert_eq!(m.terminal_burn_surplus().unwrap(), 4);
        assert_eq!(m.terminal_collateral_burned, 4);
    }

    #[test]
    fn terminal_burn_refuses_each_unclosed_dependency_class() {
        let mut base = Model::new(2, 4, 4, 0, 1, 1).unwrap();
        base.release_repair_after_zero_claims().unwrap();

        let mut reservation = base;
        reservation.open_reservations = 1;
        assert_eq!(
            reservation.terminal_burn_surplus(),
            Err(Error::OutstandingDependencies)
        );

        let mut source = base;
        source.source_archive_references = 1;
        assert_eq!(
            source.terminal_burn_surplus(),
            Err(Error::OutstandingDependencies)
        );

        let mut rent = base;
        rent.unclosed_refundable_accounts = 1;
        assert_eq!(
            rent.terminal_burn_surplus(),
            Err(Error::OutstandingDependencies)
        );

        assert_eq!(base.terminal_burn_surplus().unwrap(), 0);
        assert_eq!(base.phase, Phase::Terminal);
    }
}
