use clutch_source_plane_v3::ContentId;

use crate::auth::{domain_id, live_id, AuthenticatedSourceRouteV1, RuntimeKey};
use crate::lineage::ReopenAuthorizationV1;
use crate::{Error, Result};

const CREATION_FUNDING_DOMAIN: &[u8] = b"dragons-clutch/source-account-creation-funding/v1";
const CLOSE_FUNDING_DOMAIN: &[u8] = b"dragons-clutch/source-account-close-funding/v1";
const SOURCE_WORK_DOMAIN: &[u8] = b"dragons-clutch/source-work-authorization/v1";
const SOURCE_TERMINAL_DOMAIN: &[u8] = b"dragons-clutch/source-terminal-authorization/v1";

/// Runtime Rent-sysvar quote for one exact account allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentExemptionQuoteV1 {
    /// Digest of the complete authenticated Rent sysvar bytes.
    pub rent_sysvar_id: ContentId,
    /// Account to be allocated/assigned.
    pub account: RuntimeKey,
    /// Exact post-allocation data length.
    pub data_len: u32,
    /// Runtime-computed rent-exempt native balance for `data_len`.
    pub minimum_balance_lamports: u64,
}

impl RentExemptionQuoteV1 {
    /// Validate a nonzero, account-specific runtime quote.
    pub fn validate(&self) -> Result<()> {
        live_id(self.rent_sysvar_id)?;
        self.account.validate()?;
        if self.data_len == 0 || self.minimum_balance_lamports == 0 {
            return Err(Error::FundingMismatch);
        }
        Ok(())
    }
}

/// Persisted exact principal/donation ownership for one Source account generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceAccountFundingLedgerV1 {
    /// Physical account whose balance is partitioned.
    pub account: RuntimeKey,
    /// Monotone reopen generation.
    pub generation: u64,
    /// Payer receiving principal at close; zero exactly when principal is zero.
    pub principal_recipient: RuntimeKey,
    /// Exact payer debit used to reach rent exemption.
    pub payer_principal_lamports: u64,
    /// Balance present before creation, owned only by the neutral sink.
    pub donation_lamports: u64,
    /// Frozen neutral sink.
    pub neutral_sink: RuntimeKey,
    /// Rent-sysvar quote binding.
    pub rent_sysvar_id: ContentId,
    /// Exact allocated data length.
    pub data_len: u32,
    /// Rent-exempt minimum at creation.
    pub rent_exempt_minimum_lamports: u64,
}

impl SourceAccountFundingLedgerV1 {
    /// Validate full partition, including the fully-prefunded zero-principal case.
    pub fn validate(&self) -> Result<()> {
        self.account.validate()?;
        self.neutral_sink.validate()?;
        live_id(self.rent_sysvar_id)?;
        if self.generation == 0
            || self.data_len == 0
            || self.rent_exempt_minimum_lamports == 0
            || self.account == self.neutral_sink
            || (self.payer_principal_lamports == 0) != self.principal_recipient.is_zero()
            || (!self.principal_recipient.is_zero()
                && (self.principal_recipient == self.neutral_sink
                    || self.principal_recipient == self.account))
        {
            return Err(Error::FundingMismatch);
        }
        self.payer_principal_lamports
            .checked_add(self.donation_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    /// Exact accounted post-create balance.
    pub fn accounted_balance_lamports(self) -> Result<u64> {
        self.validate()?;
        self.payer_principal_lamports
            .checked_add(self.donation_lamports)
            .ok_or(Error::ArithmeticOverflow)
    }
}

/// Exact account-creation funding movement and persisted ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountCreationFundingV1 {
    /// Persisted balance ownership.
    pub ledger: SourceAccountFundingLedgerV1,
    /// Observed pre-allocation balance.
    pub account_balance_before: u64,
    /// Exact payer debit; never includes work or fees.
    pub payer_debit_lamports: u64,
    /// Observed post-allocation balance.
    pub account_balance_after: u64,
    /// Durable reopen authorization consumed atomically.
    pub reopen_authorization_id: ContentId,
    /// Content identity of the complete creation movement.
    pub funding_receipt_id: ContentId,
}

/// Derive a prefund-safe rent graph. Existing lamports never grant authority
/// and are never refunded to the creator; only the exact shortfall is principal.
#[allow(clippy::too_many_arguments)]
pub fn plan_source_account_creation(
    route: AuthenticatedSourceRouteV1,
    reopen: ReopenAuthorizationV1,
    quote: RentExemptionQuoteV1,
    payer: RuntimeKey,
    account_balance_before: u64,
    payer_debit_lamports: u64,
    account_balance_after: u64,
) -> Result<AccountCreationFundingV1> {
    quote.validate()?;
    if quote.account != reopen.target_account() {
        return Err(Error::FundingMismatch);
    }
    let required_shortfall = quote
        .minimum_balance_lamports
        .saturating_sub(account_balance_before);
    if payer_debit_lamports != required_shortfall
        || account_balance_before
            .checked_add(payer_debit_lamports)
            .ok_or(Error::ArithmeticOverflow)?
            != account_balance_after
        || account_balance_after < quote.minimum_balance_lamports
    {
        return Err(Error::FundingMismatch);
    }
    let principal_recipient = if payer_debit_lamports == 0 {
        RuntimeKey::ZERO
    } else {
        payer.validate()?;
        if payer == route.neutral_sink() || payer == quote.account {
            return Err(Error::IdentityAlias);
        }
        payer
    };
    let ledger = SourceAccountFundingLedgerV1 {
        account: quote.account,
        generation: reopen.next_generation(),
        principal_recipient,
        payer_principal_lamports: payer_debit_lamports,
        donation_lamports: account_balance_before,
        neutral_sink: route.neutral_sink(),
        rent_sysvar_id: quote.rent_sysvar_id,
        data_len: quote.data_len,
        rent_exempt_minimum_lamports: quote.minimum_balance_lamports,
    };
    ledger.validate()?;
    let mut bytes = [0; 224];
    bytes[..32].copy_from_slice(&ledger.account.bytes());
    bytes[32..40].copy_from_slice(&ledger.generation.to_le_bytes());
    bytes[40..72].copy_from_slice(&ledger.principal_recipient.bytes());
    bytes[72..80].copy_from_slice(&ledger.payer_principal_lamports.to_le_bytes());
    bytes[80..88].copy_from_slice(&ledger.donation_lamports.to_le_bytes());
    bytes[88..120].copy_from_slice(&ledger.neutral_sink.bytes());
    bytes[120..152].copy_from_slice(&ledger.rent_sysvar_id.bytes());
    bytes[152..156].copy_from_slice(&ledger.data_len.to_le_bytes());
    bytes[160..168].copy_from_slice(&ledger.rent_exempt_minimum_lamports.to_le_bytes());
    bytes[168..176].copy_from_slice(&account_balance_before.to_le_bytes());
    bytes[176..184].copy_from_slice(&payer_debit_lamports.to_le_bytes());
    bytes[184..192].copy_from_slice(&account_balance_after.to_le_bytes());
    bytes[192..224].copy_from_slice(&reopen.id().bytes());
    Ok(AccountCreationFundingV1 {
        ledger,
        account_balance_before,
        payer_debit_lamports,
        account_balance_after,
        reopen_authorization_id: reopen.id(),
        funding_receipt_id: domain_id(CREATION_FUNDING_DOMAIN, &bytes),
    })
}

/// Exact once-only source account close split.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountCloseFundingV1 {
    /// Closed physical account.
    pub account: RuntimeKey,
    /// Closed generation.
    pub generation: u64,
    /// Optional principal recipient.
    pub principal_recipient: RuntimeKey,
    /// Exact principal refund; zero in the fully-prefunded case.
    pub payer_refund_lamports: u64,
    /// Frozen neutral sink.
    pub neutral_sink: RuntimeKey,
    /// Entire observed balance above principal.
    pub neutral_surplus_lamports: u64,
    /// Semantic terminal receipt requiring closure.
    pub terminal_receipt_id: ContentId,
    /// Content identity of the complete close movement.
    pub close_receipt_id: ContentId,
}

/// Split the actual closing balance without reclassifying prefunds or surplus.
pub fn plan_source_account_close(
    ledger: SourceAccountFundingLedgerV1,
    actual_balance_lamports: u64,
    terminal_receipt_id: ContentId,
) -> Result<AccountCloseFundingV1> {
    ledger.validate()?;
    live_id(terminal_receipt_id)?;
    if actual_balance_lamports < ledger.payer_principal_lamports {
        return Err(Error::CloseMismatch);
    }
    let neutral_surplus_lamports = actual_balance_lamports
        .checked_sub(ledger.payer_principal_lamports)
        .ok_or(Error::ArithmeticOverflow)?;
    if neutral_surplus_lamports < ledger.donation_lamports {
        return Err(Error::CloseMismatch);
    }
    let mut bytes = [0; 160];
    bytes[..32].copy_from_slice(&ledger.account.bytes());
    bytes[32..40].copy_from_slice(&ledger.generation.to_le_bytes());
    bytes[40..72].copy_from_slice(&ledger.principal_recipient.bytes());
    bytes[72..80].copy_from_slice(&ledger.payer_principal_lamports.to_le_bytes());
    bytes[80..112].copy_from_slice(&ledger.neutral_sink.bytes());
    bytes[112..120].copy_from_slice(&neutral_surplus_lamports.to_le_bytes());
    bytes[120..152].copy_from_slice(&terminal_receipt_id.bytes());
    let close_receipt_id = domain_id(CLOSE_FUNDING_DOMAIN, &bytes);
    Ok(AccountCloseFundingV1 {
        account: ledger.account,
        generation: ledger.generation,
        principal_recipient: ledger.principal_recipient,
        payer_refund_lamports: ledger.payer_principal_lamports,
        neutral_sink: ledger.neutral_sink,
        neutral_surplus_lamports,
        terminal_receipt_id,
        close_receipt_id,
    })
}

/// Closed Source work registry used by heterogeneous liveness receipts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SourceWorkKindV1 {
    /// Authenticate parser/feed/Clock boundary evidence.
    AuthenticateBoundary = 1,
    /// Append one or more authenticated boundaries.
    AppendBoundaryBatch = 2,
    /// Freeze one immutable raw page and advance SourceHead.
    SealRawPage = 3,
    /// Fold one or more immutable pages into WindowWork.
    FoldWindowPages = 4,
    /// Finish WindowSeal and closure evidence.
    SealWindow = 5,
    /// Run one reviewed statistic evaluator step.
    EvaluateStatistic = 6,
    /// Emit an exact source-failure policy handoff.
    FailureHandoff = 7,
    /// Close or reopen one durable mutable generation.
    TerminalLifecycle = 8,
}

/// Immutable join to the liveness Source compartment and its quote schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceWorkScheduleBindingV1 {
    /// Exact heterogeneous work-schedule content identity.
    pub source_work_schedule_id: ContentId,
    /// Runtime liveness policy identity.
    pub liveness_policy_id: ContentId,
    /// Full finite market/Series lifecycle identity.
    pub lifecycle_id: ContentId,
    /// Physical Source liveness compartment.
    pub source_compartment_account: RuntimeKey,
    /// Sole Source semantic owner.
    pub source_compartment_owner: RuntimeKey,
    /// Program that owns every authenticated Source work receipt account.
    pub receipt_account_owner_program: RuntimeKey,
    /// Wallet that capitalized this compartment.
    pub payer: RuntimeKey,
    /// Exact compartment generation.
    pub generation: u64,
    /// Frozen finite call count.
    pub maximum_calls: u32,
    /// Largest one-call ceiling in the heterogeneous schedule.
    pub maximum_lamports_per_call: u64,
    /// Exact dot product of every scheduled call count and ceiling.
    pub work_capital_lamports: u64,
    /// Exact separately refundable liveness-account rent principal.
    pub rent_principal_lamports: u64,
    /// Source calls required on TradingSuccess, ZeroFutureVolume,
    /// SourceFailure, and ResolutionFailure in that order.
    pub terminal_path_calls: [u32; 4],
    /// Exact Source work lamports on those same four terminal paths.
    pub terminal_path_work_lamports: [u64; 4],
}

impl SourceWorkScheduleBindingV1 {
    /// Validate exact route/liveness identities without flattening call costs.
    pub fn validate_against(&self, route: AuthenticatedSourceRouteV1) -> Result<()> {
        live_id(self.source_work_schedule_id)?;
        live_id(self.liveness_policy_id)?;
        live_id(self.lifecycle_id)?;
        self.source_compartment_account.validate()?;
        self.source_compartment_owner.validate()?;
        self.receipt_account_owner_program.validate()?;
        self.payer.validate()?;
        if self.source_work_schedule_id != route.source_work_schedule_id()
            || self.liveness_policy_id != route.liveness_policy_id()
            || self.source_compartment_account != route.source_compartment_account()
            || self.source_compartment_owner != route.source_compartment_owner()
            || self.receipt_account_owner_program != route.adapter_program()
            || self.payer == route.neutral_sink()
            || self.generation == 0
            || self.maximum_calls == 0
            || self.maximum_lamports_per_call == 0
            || self.rent_principal_lamports == 0
        {
            return Err(Error::MismatchedBinding);
        }
        let upper = u64::from(self.maximum_calls)
            .checked_mul(self.maximum_lamports_per_call)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.work_capital_lamports < u64::from(self.maximum_calls)
            || self.work_capital_lamports > upper
        {
            return Err(Error::MismatchedBinding);
        }
        let mut index = 0_usize;
        while index < self.terminal_path_calls.len() {
            let calls = self.terminal_path_calls[index];
            let lamports = self.terminal_path_work_lamports[index];
            if calls == 0
                || lamports == 0
                || calls > self.maximum_calls
                || lamports > self.work_capital_lamports
                || lamports
                    > u64::from(calls)
                        .checked_mul(self.maximum_lamports_per_call)
                        .ok_or(Error::ArithmeticOverflow)?
            {
                return Err(Error::MismatchedBinding);
            }
            index += 1;
        }
        Ok(())
    }
}

/// One semantic Source work receipt mapped to an authenticated per-call ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceWorkAuthorizationV1 {
    /// Work kind under the closed registry.
    pub kind: SourceWorkKindV1,
    /// Exact quote schedule.
    pub source_work_schedule_id: ContentId,
    /// Liveness Source compartment.
    pub source_compartment_account: RuntimeKey,
    /// Source semantic owner.
    pub source_compartment_owner: RuntimeKey,
    /// Physical family-specific work receipt account.
    pub receipt_account_id: RuntimeKey,
    /// Program owning the authenticated work receipt account.
    pub receipt_account_owner_program_id: RuntimeKey,
    /// Full finite lifecycle identity.
    pub lifecycle_id: ContentId,
    /// Compartment generation.
    pub generation: u64,
    /// Exact monotone call ordinal.
    pub call_ordinal: u32,
    /// Authenticated ceiling for this work kind/size.
    pub call_ceiling_lamports: u64,
    /// Underlying semantic transition/evidence receipt.
    pub semantic_receipt_id: ContentId,
    /// Liveness-consumable unique work receipt.
    pub work_receipt_id: ContentId,
}

impl SourceWorkAuthorizationV1 {
    /// Bind one concrete transition to a heterogeneous schedule entry.
    pub fn new(
        route: AuthenticatedSourceRouteV1,
        schedule: SourceWorkScheduleBindingV1,
        kind: SourceWorkKindV1,
        receipt_account_id: RuntimeKey,
        call_ordinal: u32,
        call_ceiling_lamports: u64,
        semantic_receipt_id: ContentId,
    ) -> Result<Self> {
        schedule.validate_against(route)?;
        live_id(semantic_receipt_id)?;
        receipt_account_id.validate()?;
        if receipt_account_id == route.neutral_sink()
            || call_ordinal >= schedule.maximum_calls
            || call_ceiling_lamports == 0
            || call_ceiling_lamports > schedule.maximum_lamports_per_call
        {
            return Err(Error::MismatchedBinding);
        }
        let mut bytes = [0; 248];
        bytes[0] = kind as u8;
        bytes[8..40].copy_from_slice(&schedule.source_work_schedule_id.bytes());
        bytes[40..72].copy_from_slice(&schedule.source_compartment_account.bytes());
        bytes[72..104].copy_from_slice(&schedule.source_compartment_owner.bytes());
        bytes[104..136].copy_from_slice(&receipt_account_id.bytes());
        bytes[136..168].copy_from_slice(&schedule.receipt_account_owner_program.bytes());
        bytes[168..200].copy_from_slice(&schedule.lifecycle_id.bytes());
        bytes[200..208].copy_from_slice(&schedule.generation.to_le_bytes());
        bytes[208..212].copy_from_slice(&call_ordinal.to_le_bytes());
        bytes[216..224].copy_from_slice(&call_ceiling_lamports.to_le_bytes());
        bytes[224..].copy_from_slice(&semantic_receipt_id.bytes());
        Ok(Self {
            kind,
            source_work_schedule_id: schedule.source_work_schedule_id,
            source_compartment_account: schedule.source_compartment_account,
            source_compartment_owner: schedule.source_compartment_owner,
            receipt_account_id,
            receipt_account_owner_program_id: schedule.receipt_account_owner_program,
            lifecycle_id: schedule.lifecycle_id,
            generation: schedule.generation,
            call_ordinal,
            call_ceiling_lamports,
            semantic_receipt_id,
            work_receipt_id: domain_id(SOURCE_WORK_DOMAIN, &bytes),
        })
    }
}

/// Source liveness-account terminal outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SourceTerminalOutcomeV1 {
    /// Source obligations completed under one named terminal path.
    Success = 1,
    /// Source obligations terminated through a checked failure path.
    Failure = 2,
}

/// Family-authenticated terminal receipt projected to the liveness Source compartment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceTerminalAuthorizationV1 {
    /// Success versus checked failure close.
    pub outcome: SourceTerminalOutcomeV1,
    /// Physical terminal receipt account.
    pub receipt_account_id: RuntimeKey,
    /// Program owning that receipt account.
    pub receipt_account_owner_program_id: RuntimeKey,
    /// Source semantic owner.
    pub source_compartment_owner: RuntimeKey,
    /// Full finite lifecycle.
    pub lifecycle_id: ContentId,
    /// Exact quote schedule.
    pub source_work_schedule_id: ContentId,
    /// Source compartment generation.
    pub generation: u64,
    /// Underlying semantic terminal fact.
    pub semantic_terminal_receipt_id: ContentId,
    /// Liveness-consumable terminal receipt ID.
    pub terminal_receipt_id: ContentId,
}

impl SourceTerminalAuthorizationV1 {
    /// Bind a family terminal fact with canonical zero call ordinal/ceiling semantics.
    pub fn new(
        route: AuthenticatedSourceRouteV1,
        schedule: SourceWorkScheduleBindingV1,
        outcome: SourceTerminalOutcomeV1,
        receipt_account_id: RuntimeKey,
        semantic_terminal_receipt_id: ContentId,
    ) -> Result<Self> {
        schedule.validate_against(route)?;
        receipt_account_id.validate()?;
        live_id(semantic_terminal_receipt_id)?;
        if receipt_account_id == route.neutral_sink() {
            return Err(Error::IdentityAlias);
        }
        let mut bytes = [0; 232];
        bytes[0] = outcome as u8;
        bytes[8..40].copy_from_slice(&receipt_account_id.bytes());
        bytes[40..72].copy_from_slice(&schedule.receipt_account_owner_program.bytes());
        bytes[72..104].copy_from_slice(&schedule.source_compartment_owner.bytes());
        bytes[104..136].copy_from_slice(&schedule.lifecycle_id.bytes());
        bytes[136..168].copy_from_slice(&schedule.source_work_schedule_id.bytes());
        bytes[168..176].copy_from_slice(&schedule.generation.to_le_bytes());
        bytes[176..208].copy_from_slice(&semantic_terminal_receipt_id.bytes());
        let terminal_receipt_id = domain_id(SOURCE_TERMINAL_DOMAIN, &bytes);
        Ok(Self {
            outcome,
            receipt_account_id,
            receipt_account_owner_program_id: schedule.receipt_account_owner_program,
            source_compartment_owner: schedule.source_compartment_owner,
            lifecycle_id: schedule.lifecycle_id,
            source_work_schedule_id: schedule.source_work_schedule_id,
            generation: schedule.generation,
            semantic_terminal_receipt_id,
            terminal_receipt_id,
        })
    }

    /// Liveness call ordinal is always zero for terminal receipts.
    pub const fn call_ordinal(self) -> u32 {
        0
    }

    /// Liveness call ceiling is always zero for terminal receipts.
    pub const fn call_ceiling_lamports(self) -> u64 {
        0
    }
}
