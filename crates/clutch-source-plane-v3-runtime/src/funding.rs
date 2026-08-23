use clutch_source_plane_v3::ContentId;

use crate::account::RuntimeAccountHeaderV1;
use crate::auth::{
    account_data_id, domain_id, live_id, AuthenticatedSourceRouteV1, RuntimeAccountViewV1,
    RuntimeDerivedPdaV1, RuntimeKey,
};
use crate::lineage::ReopenAuthorizationV1;
use crate::{Error, Result};
use clutch_source_plane_v3_adapter::{AccountFamilyV3, PdaRecipeV3};

const CREATION_FUNDING_DOMAIN: &[u8] = b"dragons-clutch/source-account-creation-funding/v1";
const CLOSE_FUNDING_DOMAIN: &[u8] = b"dragons-clutch/source-account-close-funding/v1";
const SOURCE_WORK_DOMAIN: &[u8] = b"dragons-clutch/source-work-authorization/v1";
const SOURCE_TERMINAL_DOMAIN: &[u8] = b"dragons-clutch/source-terminal-authorization/v1";
const SOURCE_WORK_SCHEDULE_DOMAIN: &[u8] = b"dragons-clutch/source-work-schedule/v1";
const SOURCE_WORK_SCHEDULE_MAGIC: [u8; 8] = *b"DCSWSV01";
const SOURCE_WORK_RECEIPT_ACCOUNT_DOMAIN: &[u8] = b"dragons-clutch/source-work-receipt-account/v1";
const SOURCE_WORK_RECEIPT_AUTH_DOMAIN: &[u8] =
    b"dragons-clutch/authenticated-source-work-receipt-account/v1";
const SOURCE_WORK_RECEIPT_MAGIC: [u8; 8] = [0x92, 1, b'D', b'C', b'S', b'W', b'R', b'1'];

/// Exact canonical bytes in one persisted Source work or terminal receipt.
pub const SOURCE_WORK_RECEIPT_ACCOUNT_BYTES: usize = 328;
/// Registered main-program account discriminator for Source work receipts.
pub const SOURCE_WORK_RECEIPT_ACCOUNT_TAG: u8 = SOURCE_WORK_RECEIPT_MAGIC[0];
/// Registered main-program account version for Source work receipts.
pub const SOURCE_WORK_RECEIPT_ACCOUNT_VERSION: u8 = 1;
/// Exact canonical bytes defining one heterogeneous Source work schedule.
pub const SOURCE_WORK_SCHEDULE_BYTES: usize = 392;

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

    /// Construct the exact prefund-safe runtime account header.
    pub fn runtime_header(
        self,
        family: AccountFamilyV3,
        bump: u8,
    ) -> Result<RuntimeAccountHeaderV1> {
        self.validate()?;
        let header = RuntimeAccountHeaderV1 {
            family,
            bump,
            principal_recipient: self.principal_recipient,
            payer_principal_lamports: self.payer_principal_lamports,
            donation_floor_lamports: self.donation_lamports,
            generation: self.generation,
        };
        header.validate(self.neutral_sink)?;
        Ok(header)
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

impl SourceWorkKindV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::AuthenticateBoundary),
            2 => Ok(Self::AppendBoundaryBatch),
            3 => Ok(Self::SealRawPage),
            4 => Ok(Self::FoldWindowPages),
            5 => Ok(Self::SealWindow),
            6 => Ok(Self::EvaluateStatistic),
            7 => Ok(Self::FailureHandoff),
            8 => Ok(Self::TerminalLifecycle),
            _ => Err(Error::InvalidCodec),
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::AuthenticateBoundary => 0,
            Self::AppendBoundaryBatch => 1,
            Self::SealRawPage => 2,
            Self::FoldWindowPages => 3,
            Self::SealWindow => 4,
            Self::EvaluateStatistic => 5,
            Self::FailureHandoff => 6,
            Self::TerminalLifecycle => 7,
        }
    }
}

/// Immutable join to the liveness Source compartment and its quote schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceWorkScheduleBindingV1 {
    source_work_schedule_id: ContentId,
    liveness_policy_id: ContentId,
    lifecycle_id: ContentId,
    source_compartment_account: RuntimeKey,
    source_compartment_owner: RuntimeKey,
    receipt_account_owner_program: RuntimeKey,
    payer: RuntimeKey,
    generation: u64,
    maximum_calls: u32,
    maximum_lamports_per_call: u64,
    work_capital_lamports: u64,
    rent_principal_lamports: u64,
    work_kind_calls: [u32; 8],
    work_kind_ceiling_lamports: [u64; 8],
    terminal_path_calls: [u32; 4],
    terminal_path_work_lamports: [u64; 4],
}

impl SourceWorkScheduleBindingV1 {
    /// Construct and content-address every exact schedule/custody field.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        liveness_policy_id: ContentId,
        lifecycle_id: ContentId,
        source_compartment_account: RuntimeKey,
        source_compartment_owner: RuntimeKey,
        receipt_account_owner_program: RuntimeKey,
        payer: RuntimeKey,
        generation: u64,
        maximum_calls: u32,
        maximum_lamports_per_call: u64,
        work_capital_lamports: u64,
        rent_principal_lamports: u64,
        work_kind_calls: [u32; 8],
        work_kind_ceiling_lamports: [u64; 8],
        terminal_path_calls: [u32; 4],
        terminal_path_work_lamports: [u64; 4],
    ) -> Result<Self> {
        let mut value = Self {
            source_work_schedule_id: ContentId::ZERO,
            liveness_policy_id,
            lifecycle_id,
            source_compartment_account,
            source_compartment_owner,
            receipt_account_owner_program,
            payer,
            generation,
            maximum_calls,
            maximum_lamports_per_call,
            work_capital_lamports,
            rent_principal_lamports,
            work_kind_calls,
            work_kind_ceiling_lamports,
            terminal_path_calls,
            terminal_path_work_lamports,
        };
        value.validate_fields_without_stored_id()?;
        value.source_work_schedule_id = value.recomputed_id()?;
        value.validate()?;
        Ok(value)
    }

    /// Hostile-decode one exact canonical schedule body and recompute its ID.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != SOURCE_WORK_SCHEDULE_BYTES
            || input[..8] != SOURCE_WORK_SCHEDULE_MAGIC
            || funding_le_u16(&input[8..10]) != 1
            || input[10..16].iter().any(|byte| *byte != 0)
            || input[220..224].iter().any(|byte| *byte != 0)
        {
            return Err(Error::InvalidCodec);
        }
        let mut terminal_path_calls = [0_u32; 4];
        let mut terminal_path_work_lamports = [0_u64; 4];
        let mut work_kind_calls = [0_u32; 8];
        let mut work_kind_ceiling_lamports = [0_u64; 8];
        let mut index = 0_usize;
        while index < 8 {
            let call_at = 248 + index * 4;
            work_kind_calls[index] = funding_le_u32(&input[call_at..call_at + 4]);
            let ceiling_at = 280 + index * 8;
            work_kind_ceiling_lamports[index] = funding_le_u64(&input[ceiling_at..ceiling_at + 8]);
            index += 1;
        }
        index = 0;
        while index < 4 {
            let call_at = 344 + index * 4;
            terminal_path_calls[index] = funding_le_u32(&input[call_at..call_at + 4]);
            let work_at = 360 + index * 8;
            terminal_path_work_lamports[index] = funding_le_u64(&input[work_at..work_at + 8]);
            index += 1;
        }
        Self::new(
            content_id_at(input, 16),
            content_id_at(input, 48),
            runtime_key_at(input, 80),
            runtime_key_at(input, 112),
            runtime_key_at(input, 144),
            runtime_key_at(input, 176),
            funding_le_u64(&input[208..216]),
            funding_le_u32(&input[216..220]),
            funding_le_u64(&input[224..232]),
            funding_le_u64(&input[232..240]),
            funding_le_u64(&input[240..248]),
            work_kind_calls,
            work_kind_ceiling_lamports,
            terminal_path_calls,
            terminal_path_work_lamports,
        )
    }

    /// Encode one exact canonical schedule body.
    pub fn encode(&self) -> Result<[u8; SOURCE_WORK_SCHEDULE_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; SOURCE_WORK_SCHEDULE_BYTES];
        output[..8].copy_from_slice(&SOURCE_WORK_SCHEDULE_MAGIC);
        output[8..10].copy_from_slice(&1_u16.to_le_bytes());
        output[16..48].copy_from_slice(&self.liveness_policy_id.bytes());
        output[48..80].copy_from_slice(&self.lifecycle_id.bytes());
        output[80..112].copy_from_slice(&self.source_compartment_account.bytes());
        output[112..144].copy_from_slice(&self.source_compartment_owner.bytes());
        output[144..176].copy_from_slice(&self.receipt_account_owner_program.bytes());
        output[176..208].copy_from_slice(&self.payer.bytes());
        output[208..216].copy_from_slice(&self.generation.to_le_bytes());
        output[216..220].copy_from_slice(&self.maximum_calls.to_le_bytes());
        output[224..232].copy_from_slice(&self.maximum_lamports_per_call.to_le_bytes());
        output[232..240].copy_from_slice(&self.work_capital_lamports.to_le_bytes());
        output[240..248].copy_from_slice(&self.rent_principal_lamports.to_le_bytes());
        let mut index = 0_usize;
        while index < 8 {
            let call_at = 248 + index * 4;
            output[call_at..call_at + 4]
                .copy_from_slice(&self.work_kind_calls[index].to_le_bytes());
            let ceiling_at = 280 + index * 8;
            output[ceiling_at..ceiling_at + 8]
                .copy_from_slice(&self.work_kind_ceiling_lamports[index].to_le_bytes());
            index += 1;
        }
        index = 0;
        while index < 4 {
            let call_at = 344 + index * 4;
            output[call_at..call_at + 4]
                .copy_from_slice(&self.terminal_path_calls[index].to_le_bytes());
            let work_at = 360 + index * 8;
            output[work_at..work_at + 8]
                .copy_from_slice(&self.terminal_path_work_lamports[index].to_le_bytes());
            index += 1;
        }
        Ok(output)
    }

    /// Recomputed content identity of the complete schedule body.
    pub fn id(&self) -> Result<ContentId> {
        self.validate()?;
        Ok(self.source_work_schedule_id)
    }

    /// Exact heterogeneous work-schedule content identity.
    pub const fn source_work_schedule_id(self) -> ContentId {
        self.source_work_schedule_id
    }

    /// Runtime liveness policy identity.
    pub const fn liveness_policy_id(self) -> ContentId {
        self.liveness_policy_id
    }

    /// Full finite market/Series lifecycle identity.
    pub const fn lifecycle_id(self) -> ContentId {
        self.lifecycle_id
    }

    /// Physical Source liveness compartment.
    pub const fn source_compartment_account(self) -> RuntimeKey {
        self.source_compartment_account
    }

    /// Sole Source semantic owner.
    pub const fn source_compartment_owner(self) -> RuntimeKey {
        self.source_compartment_owner
    }

    /// Program owning every authenticated Source work receipt account.
    pub const fn receipt_account_owner_program(self) -> RuntimeKey {
        self.receipt_account_owner_program
    }

    /// Wallet that capitalized the Source compartment.
    pub const fn payer(self) -> RuntimeKey {
        self.payer
    }

    /// Exact Source compartment generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Frozen finite call count.
    pub const fn maximum_calls(self) -> u32 {
        self.maximum_calls
    }

    /// Largest one-call ceiling in the heterogeneous schedule.
    pub const fn maximum_lamports_per_call(self) -> u64 {
        self.maximum_lamports_per_call
    }

    /// Exact dot product of scheduled counts and ceilings.
    pub const fn work_capital_lamports(self) -> u64 {
        self.work_capital_lamports
    }

    /// Exact separately refundable liveness-account rent principal.
    pub const fn rent_principal_lamports(self) -> u64 {
        self.rent_principal_lamports
    }

    /// Ordered call counts for the closed [`SourceWorkKindV1`] registry.
    pub const fn work_kind_calls(self) -> [u32; 8] {
        self.work_kind_calls
    }

    /// Ordered per-call ceilings for the closed [`SourceWorkKindV1`] registry.
    pub const fn work_kind_ceiling_lamports(self) -> [u64; 8] {
        self.work_kind_ceiling_lamports
    }

    /// Exact ceiling assigned to one work kind.
    pub const fn ceiling_for(self, kind: SourceWorkKindV1) -> u64 {
        self.work_kind_ceiling_lamports[kind.index()]
    }

    /// Ordered terminal-path Source call counts.
    pub const fn terminal_path_calls(self) -> [u32; 4] {
        self.terminal_path_calls
    }

    /// Ordered terminal-path Source work lamports.
    pub const fn terminal_path_work_lamports(self) -> [u64; 4] {
        self.terminal_path_work_lamports
    }

    /// Validate exact route/liveness identities without flattening call costs.
    pub fn validate_against(&self, route: AuthenticatedSourceRouteV1) -> Result<()> {
        self.validate()?;
        if self.source_work_schedule_id != route.source_work_schedule_id()
            || self.liveness_policy_id != route.liveness_policy_id()
            || self.source_compartment_account != route.source_compartment_account()
            || self.source_compartment_owner != route.source_compartment_owner()
            || self.receipt_account_owner_program != route.adapter_program()
            || self.payer == route.neutral_sink()
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        self.validate_fields()?;
        if self.source_work_schedule_id != self.recomputed_id()? {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    fn validate_fields(&self) -> Result<()> {
        live_id(self.source_work_schedule_id)?;
        self.validate_fields_without_stored_id()
    }

    fn recomputed_id(&self) -> Result<ContentId> {
        self.validate_fields_without_stored_id()?;
        let mut output = [0_u8; SOURCE_WORK_SCHEDULE_BYTES];
        output[..8].copy_from_slice(&SOURCE_WORK_SCHEDULE_MAGIC);
        output[8..10].copy_from_slice(&1_u16.to_le_bytes());
        output[16..48].copy_from_slice(&self.liveness_policy_id.bytes());
        output[48..80].copy_from_slice(&self.lifecycle_id.bytes());
        output[80..112].copy_from_slice(&self.source_compartment_account.bytes());
        output[112..144].copy_from_slice(&self.source_compartment_owner.bytes());
        output[144..176].copy_from_slice(&self.receipt_account_owner_program.bytes());
        output[176..208].copy_from_slice(&self.payer.bytes());
        output[208..216].copy_from_slice(&self.generation.to_le_bytes());
        output[216..220].copy_from_slice(&self.maximum_calls.to_le_bytes());
        output[224..232].copy_from_slice(&self.maximum_lamports_per_call.to_le_bytes());
        output[232..240].copy_from_slice(&self.work_capital_lamports.to_le_bytes());
        output[240..248].copy_from_slice(&self.rent_principal_lamports.to_le_bytes());
        let mut index = 0_usize;
        while index < 8 {
            let call_at = 248 + index * 4;
            output[call_at..call_at + 4]
                .copy_from_slice(&self.work_kind_calls[index].to_le_bytes());
            let ceiling_at = 280 + index * 8;
            output[ceiling_at..ceiling_at + 8]
                .copy_from_slice(&self.work_kind_ceiling_lamports[index].to_le_bytes());
            index += 1;
        }
        index = 0;
        while index < 4 {
            let call_at = 344 + index * 4;
            output[call_at..call_at + 4]
                .copy_from_slice(&self.terminal_path_calls[index].to_le_bytes());
            let work_at = 360 + index * 8;
            output[work_at..work_at + 8]
                .copy_from_slice(&self.terminal_path_work_lamports[index].to_le_bytes());
            index += 1;
        }
        Ok(domain_id(SOURCE_WORK_SCHEDULE_DOMAIN, &output))
    }

    fn validate_fields_without_stored_id(&self) -> Result<()> {
        live_id(self.liveness_policy_id)?;
        live_id(self.lifecycle_id)?;
        self.source_compartment_account.validate()?;
        self.source_compartment_owner.validate()?;
        self.receipt_account_owner_program.validate()?;
        self.payer.validate()?;
        if self.generation == 0
            || self.maximum_calls == 0
            || self.maximum_lamports_per_call == 0
            || self.rent_principal_lamports == 0
        {
            return Err(Error::MismatchedBinding);
        }
        let mut exact_calls = 0_u32;
        let mut exact_work_capital = 0_u64;
        let mut exact_maximum = 0_u64;
        let mut work_index = 0_usize;
        while work_index < self.work_kind_calls.len() {
            let calls = self.work_kind_calls[work_index];
            let ceiling = self.work_kind_ceiling_lamports[work_index];
            if (calls == 0) != (ceiling == 0) {
                return Err(Error::MismatchedBinding);
            }
            exact_calls = exact_calls
                .checked_add(calls)
                .ok_or(Error::ArithmeticOverflow)?;
            exact_work_capital = exact_work_capital
                .checked_add(
                    u64::from(calls)
                        .checked_mul(ceiling)
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            exact_maximum = exact_maximum.max(ceiling);
            work_index += 1;
        }
        if exact_calls != self.maximum_calls
            || exact_work_capital != self.work_capital_lamports
            || exact_maximum != self.maximum_lamports_per_call
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
    kind: SourceWorkKindV1,
    source_work_schedule_id: ContentId,
    source_compartment_account: RuntimeKey,
    source_compartment_owner: RuntimeKey,
    receipt_account_id: RuntimeKey,
    receipt_account_owner_program_id: RuntimeKey,
    lifecycle_id: ContentId,
    generation: u64,
    call_ordinal: u32,
    call_ceiling_lamports: u64,
    semantic_receipt_id: ContentId,
    work_receipt_id: ContentId,
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
            || call_ordinal == 0
            || call_ordinal > schedule.maximum_calls
            || call_ceiling_lamports == 0
            || call_ceiling_lamports != schedule.ceiling_for(kind)
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

    /// Work kind under the closed registry.
    pub const fn kind(self) -> SourceWorkKindV1 {
        self.kind
    }

    /// Exact heterogeneous quote schedule.
    pub const fn source_work_schedule_id(self) -> ContentId {
        self.source_work_schedule_id
    }

    /// Physical liveness Source compartment.
    pub const fn source_compartment_account(self) -> RuntimeKey {
        self.source_compartment_account
    }

    /// Sole Source semantic owner.
    pub const fn source_compartment_owner(self) -> RuntimeKey {
        self.source_compartment_owner
    }

    /// Physical receipt account.
    pub const fn receipt_account_id(self) -> RuntimeKey {
        self.receipt_account_id
    }

    /// Program owning the persisted receipt.
    pub const fn receipt_account_owner_program_id(self) -> RuntimeKey {
        self.receipt_account_owner_program_id
    }

    /// Full finite lifecycle identity.
    pub const fn lifecycle_id(self) -> ContentId {
        self.lifecycle_id
    }

    /// Exact Source compartment generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Exact monotone call ordinal.
    pub const fn call_ordinal(self) -> u32 {
        self.call_ordinal
    }

    /// Authenticated heterogeneous per-call ceiling.
    pub const fn call_ceiling_lamports(self) -> u64 {
        self.call_ceiling_lamports
    }

    /// Underlying semantic transition/evidence receipt.
    pub const fn semantic_receipt_id(self) -> ContentId {
        self.semantic_receipt_id
    }

    /// Liveness-consumable unique work receipt.
    pub const fn id(self) -> ContentId {
        self.work_receipt_id
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
    outcome: SourceTerminalOutcomeV1,
    receipt_account_id: RuntimeKey,
    receipt_account_owner_program_id: RuntimeKey,
    source_compartment_owner: RuntimeKey,
    lifecycle_id: ContentId,
    source_work_schedule_id: ContentId,
    generation: u64,
    semantic_terminal_receipt_id: ContentId,
    terminal_receipt_id: ContentId,
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

    /// Success versus checked failure close.
    pub const fn outcome(self) -> SourceTerminalOutcomeV1 {
        self.outcome
    }

    /// Physical terminal receipt account.
    pub const fn receipt_account_id(self) -> RuntimeKey {
        self.receipt_account_id
    }

    /// Program owning the persisted receipt.
    pub const fn receipt_account_owner_program_id(self) -> RuntimeKey {
        self.receipt_account_owner_program_id
    }

    /// Sole Source semantic owner.
    pub const fn source_compartment_owner(self) -> RuntimeKey {
        self.source_compartment_owner
    }

    /// Full finite lifecycle identity.
    pub const fn lifecycle_id(self) -> ContentId {
        self.lifecycle_id
    }

    /// Exact heterogeneous quote schedule.
    pub const fn source_work_schedule_id(self) -> ContentId {
        self.source_work_schedule_id
    }

    /// Exact Source compartment generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Underlying semantic terminal fact.
    pub const fn semantic_terminal_receipt_id(self) -> ContentId {
        self.semantic_terminal_receipt_id
    }

    /// Liveness-consumable terminal receipt identity.
    pub const fn id(self) -> ContentId {
        self.terminal_receipt_id
    }
}

/// Exhaustive semantic kind persisted in one Source receipt account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SourceReceiptDispositionV1 {
    /// One paid Source work call.
    Work = 1,
    /// Terminal success closes the Source liveness compartment.
    TerminalSuccess = 2,
    /// Checked terminal failure closes the Source liveness compartment.
    TerminalFailure = 3,
}

impl SourceReceiptDispositionV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Work),
            2 => Ok(Self::TerminalSuccess),
            3 => Ok(Self::TerminalFailure),
            _ => Err(Error::InvalidCodec),
        }
    }
}

/// Canonical persisted Source work/terminal receipt body.
///
/// This account is immutable evidence. Native work capital remains solely in
/// the liveness Source compartment; persisting this body never authorizes a
/// second keeper debit or stores a second custody balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceWorkReceiptAccountV1 {
    disposition: SourceReceiptDispositionV1,
    work_kind: Option<SourceWorkKindV1>,
    route_id: ContentId,
    source_work_schedule_id: ContentId,
    source_compartment_account: RuntimeKey,
    source_compartment_owner: RuntimeKey,
    receipt_account_id: RuntimeKey,
    receipt_account_owner_program_id: RuntimeKey,
    lifecycle_id: ContentId,
    generation: u64,
    call_ordinal: u32,
    call_ceiling_lamports: u64,
    semantic_receipt_id: ContentId,
    receipt_id: ContentId,
}

impl SourceWorkReceiptAccountV1 {
    /// Freeze one exact paid-work authorization into its immutable account body.
    pub fn from_work(
        route: AuthenticatedSourceRouteV1,
        authorization: SourceWorkAuthorizationV1,
    ) -> Result<Self> {
        if authorization.source_work_schedule_id() != route.source_work_schedule_id()
            || authorization.source_compartment_account() != route.source_compartment_account()
            || authorization.source_compartment_owner() != route.source_compartment_owner()
            || authorization.receipt_account_owner_program_id() != route.adapter_program()
        {
            return Err(Error::MismatchedBinding);
        }
        let value = Self {
            disposition: SourceReceiptDispositionV1::Work,
            work_kind: Some(authorization.kind()),
            route_id: route.route_id(),
            source_work_schedule_id: authorization.source_work_schedule_id(),
            source_compartment_account: authorization.source_compartment_account(),
            source_compartment_owner: authorization.source_compartment_owner(),
            receipt_account_id: authorization.receipt_account_id(),
            receipt_account_owner_program_id: authorization.receipt_account_owner_program_id(),
            lifecycle_id: authorization.lifecycle_id(),
            generation: authorization.generation(),
            call_ordinal: authorization.call_ordinal(),
            call_ceiling_lamports: authorization.call_ceiling_lamports(),
            semantic_receipt_id: authorization.semantic_receipt_id(),
            receipt_id: authorization.id(),
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Freeze one exact terminal authorization into its immutable account body.
    pub fn from_terminal(
        route: AuthenticatedSourceRouteV1,
        authorization: SourceTerminalAuthorizationV1,
    ) -> Result<Self> {
        if authorization.source_work_schedule_id() != route.source_work_schedule_id()
            || authorization.source_compartment_owner() != route.source_compartment_owner()
            || authorization.receipt_account_owner_program_id() != route.adapter_program()
        {
            return Err(Error::MismatchedBinding);
        }
        let disposition = match authorization.outcome() {
            SourceTerminalOutcomeV1::Success => SourceReceiptDispositionV1::TerminalSuccess,
            SourceTerminalOutcomeV1::Failure => SourceReceiptDispositionV1::TerminalFailure,
        };
        let value = Self {
            disposition,
            work_kind: None,
            route_id: route.route_id(),
            source_work_schedule_id: authorization.source_work_schedule_id(),
            source_compartment_account: route.source_compartment_account(),
            source_compartment_owner: authorization.source_compartment_owner(),
            receipt_account_id: authorization.receipt_account_id(),
            receipt_account_owner_program_id: authorization.receipt_account_owner_program_id(),
            lifecycle_id: authorization.lifecycle_id(),
            generation: authorization.generation(),
            call_ordinal: 0,
            call_ceiling_lamports: 0,
            semantic_receipt_id: authorization.semantic_terminal_receipt_id(),
            receipt_id: authorization.id(),
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Hostile-decode the exact registered account bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != SOURCE_WORK_RECEIPT_ACCOUNT_BYTES
            || input[..8] != SOURCE_WORK_RECEIPT_MAGIC
            || input[1] != SOURCE_WORK_RECEIPT_ACCOUNT_VERSION
            || input[4..16].iter().any(|byte| *byte != 0)
            || input[252..256].iter().any(|byte| *byte != 0)
        {
            return Err(Error::InvalidCodec);
        }
        let disposition = SourceReceiptDispositionV1::decode(input[2])?;
        let work_kind = match disposition {
            SourceReceiptDispositionV1::Work => Some(SourceWorkKindV1::decode(input[3])?),
            SourceReceiptDispositionV1::TerminalSuccess
            | SourceReceiptDispositionV1::TerminalFailure => {
                if input[3] != 0 {
                    return Err(Error::InvalidCodec);
                }
                None
            }
        };
        let value = Self {
            disposition,
            work_kind,
            route_id: content_id_at(input, 16),
            source_work_schedule_id: content_id_at(input, 48),
            source_compartment_account: runtime_key_at(input, 80),
            source_compartment_owner: runtime_key_at(input, 112),
            receipt_account_id: runtime_key_at(input, 144),
            receipt_account_owner_program_id: runtime_key_at(input, 176),
            lifecycle_id: content_id_at(input, 208),
            generation: funding_le_u64(&input[240..248]),
            call_ordinal: funding_le_u32(&input[248..252]),
            call_ceiling_lamports: funding_le_u64(&input[256..264]),
            semantic_receipt_id: content_id_at(input, 264),
            receipt_id: content_id_at(input, 296),
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Encode the exact registered account bytes.
    pub fn encode(&self) -> Result<[u8; SOURCE_WORK_RECEIPT_ACCOUNT_BYTES]> {
        self.validate_shape()?;
        let mut output = [0_u8; SOURCE_WORK_RECEIPT_ACCOUNT_BYTES];
        output[..8].copy_from_slice(&SOURCE_WORK_RECEIPT_MAGIC);
        output[1] = SOURCE_WORK_RECEIPT_ACCOUNT_VERSION;
        output[2] = self.disposition as u8;
        output[3] = self.work_kind.map_or(0, |kind| kind as u8);
        output[16..48].copy_from_slice(&self.route_id.bytes());
        output[48..80].copy_from_slice(&self.source_work_schedule_id.bytes());
        output[80..112].copy_from_slice(&self.source_compartment_account.bytes());
        output[112..144].copy_from_slice(&self.source_compartment_owner.bytes());
        output[144..176].copy_from_slice(&self.receipt_account_id.bytes());
        output[176..208].copy_from_slice(&self.receipt_account_owner_program_id.bytes());
        output[208..240].copy_from_slice(&self.lifecycle_id.bytes());
        output[240..248].copy_from_slice(&self.generation.to_le_bytes());
        output[248..252].copy_from_slice(&self.call_ordinal.to_le_bytes());
        output[256..264].copy_from_slice(&self.call_ceiling_lamports.to_le_bytes());
        output[264..296].copy_from_slice(&self.semantic_receipt_id.bytes());
        output[296..328].copy_from_slice(&self.receipt_id.bytes());
        Ok(output)
    }

    /// Identity of the complete immutable account body.
    pub fn id(&self) -> Result<ContentId> {
        Ok(domain_id(
            SOURCE_WORK_RECEIPT_ACCOUNT_DOMAIN,
            &self.encode()?,
        ))
    }

    /// Work versus terminal disposition.
    pub const fn disposition(self) -> SourceReceiptDispositionV1 {
        self.disposition
    }

    /// Paid work kind, absent on terminal receipts.
    pub const fn work_kind(self) -> Option<SourceWorkKindV1> {
        self.work_kind
    }

    /// Exact authenticated Source route.
    pub const fn route_id(self) -> ContentId {
        self.route_id
    }

    /// Exact heterogeneous quote schedule.
    pub const fn source_work_schedule_id(self) -> ContentId {
        self.source_work_schedule_id
    }

    /// Physical liveness Source compartment.
    pub const fn source_compartment_account(self) -> RuntimeKey {
        self.source_compartment_account
    }

    /// Sole Source semantic owner.
    pub const fn source_compartment_owner(self) -> RuntimeKey {
        self.source_compartment_owner
    }

    /// Physical immutable receipt account.
    pub const fn receipt_account_id(self) -> RuntimeKey {
        self.receipt_account_id
    }

    /// Program owning the persisted receipt.
    pub const fn receipt_account_owner_program_id(self) -> RuntimeKey {
        self.receipt_account_owner_program_id
    }

    /// Full finite lifecycle identity.
    pub const fn lifecycle_id(self) -> ContentId {
        self.lifecycle_id
    }

    /// Exact Source compartment generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Exact paid call ordinal, zero for terminal receipts.
    pub const fn call_ordinal(self) -> u32 {
        self.call_ordinal
    }

    /// Exact per-call ceiling, zero for terminal receipts.
    pub const fn call_ceiling_lamports(self) -> u64 {
        self.call_ceiling_lamports
    }

    /// Underlying source semantic fact.
    pub const fn semantic_receipt_id(self) -> ContentId {
        self.semantic_receipt_id
    }

    /// Liveness-consumable receipt identity.
    pub const fn receipt_id(self) -> ContentId {
        self.receipt_id
    }

    fn validate_shape(&self) -> Result<()> {
        live_id(self.route_id)?;
        live_id(self.source_work_schedule_id)?;
        live_id(self.lifecycle_id)?;
        live_id(self.semantic_receipt_id)?;
        live_id(self.receipt_id)?;
        self.source_compartment_account.validate()?;
        self.source_compartment_owner.validate()?;
        self.receipt_account_id.validate()?;
        self.receipt_account_owner_program_id.validate()?;
        if self.generation == 0
            || self.source_compartment_account == self.receipt_account_id
            || self.source_compartment_owner == self.receipt_account_id
            || self.receipt_account_owner_program_id == self.receipt_account_id
        {
            return Err(Error::MismatchedBinding);
        }
        match self.disposition {
            SourceReceiptDispositionV1::Work => {
                if self.work_kind.is_none()
                    || self.call_ceiling_lamports == 0
                    || self.receipt_id == self.semantic_receipt_id
                {
                    return Err(Error::MismatchedBinding);
                }
            }
            SourceReceiptDispositionV1::TerminalSuccess
            | SourceReceiptDispositionV1::TerminalFailure => {
                if self.work_kind.is_some()
                    || self.call_ordinal != 0
                    || self.call_ceiling_lamports != 0
                {
                    return Err(Error::MismatchedBinding);
                }
            }
        }
        Ok(())
    }

    fn validate_against(
        &self,
        route: AuthenticatedSourceRouteV1,
        schedule: SourceWorkScheduleBindingV1,
    ) -> Result<()> {
        self.validate_shape()?;
        schedule.validate_against(route)?;
        if self.route_id != route.route_id()
            || self.source_work_schedule_id != schedule.source_work_schedule_id
            || self.source_compartment_account != schedule.source_compartment_account
            || self.source_compartment_owner != schedule.source_compartment_owner
            || self.receipt_account_owner_program_id != schedule.receipt_account_owner_program
            || self.lifecycle_id != schedule.lifecycle_id
            || self.generation != schedule.generation
        {
            return Err(Error::MismatchedBinding);
        }
        match self.disposition {
            SourceReceiptDispositionV1::Work => {
                let expected = SourceWorkAuthorizationV1::new(
                    route,
                    schedule,
                    self.work_kind.ok_or(Error::InvalidCodec)?,
                    self.receipt_account_id,
                    self.call_ordinal,
                    self.call_ceiling_lamports,
                    self.semantic_receipt_id,
                )?;
                if expected.id() != self.receipt_id {
                    return Err(Error::MismatchedBinding);
                }
            }
            SourceReceiptDispositionV1::TerminalSuccess
            | SourceReceiptDispositionV1::TerminalFailure => {
                let outcome = if self.disposition == SourceReceiptDispositionV1::TerminalSuccess {
                    SourceTerminalOutcomeV1::Success
                } else {
                    SourceTerminalOutcomeV1::Failure
                };
                let expected = SourceTerminalAuthorizationV1::new(
                    route,
                    schedule,
                    outcome,
                    self.receipt_account_id,
                    self.semantic_receipt_id,
                )?;
                if expected.id() != self.receipt_id {
                    return Err(Error::MismatchedBinding);
                }
            }
        }
        Ok(())
    }
}

/// Runtime-authenticated immutable Source work/terminal receipt account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSourceWorkReceiptV1 {
    account: RuntimeKey,
    account_data_id: ContentId,
    receipt: SourceWorkReceiptAccountV1,
    schedule: SourceWorkScheduleBindingV1,
    pda_bump: u8,
    authentication_id: ContentId,
}

impl AuthenticatedSourceWorkReceiptV1 {
    /// Physical immutable receipt account.
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Digest of the complete persisted account bytes.
    pub const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    /// Canonical persisted receipt body.
    pub const fn receipt(self) -> SourceWorkReceiptAccountV1 {
        self.receipt
    }

    /// Exact schedule used to authenticate lifecycle, custody, and ceiling.
    pub const fn schedule(self) -> SourceWorkScheduleBindingV1 {
        self.schedule
    }

    /// Canonical PDA bump authenticated by the runtime adapter.
    pub const fn pda_bump(self) -> u8 {
        self.pda_bump
    }

    /// Complete owner/PDA/body/schedule authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

/// Authenticate one exact read-only Source receipt account and quote schedule.
pub fn authenticate_source_work_receipt_account(
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
) -> Result<AuthenticatedSourceWorkReceiptV1> {
    if account.owner != route.adapter_program() {
        return Err(Error::WrongOwner);
    }
    if account.executable || account.signer || account.writable {
        return Err(Error::WrongPrivilege);
    }
    let receipt = SourceWorkReceiptAccountV1::decode(account.data)?;
    receipt.validate_against(route, schedule)?;
    if receipt.receipt_account_id() != account.key {
        return Err(Error::WrongAccount);
    }
    let recipe = PdaRecipeV3::source_work_receipt(receipt.receipt_id())?;
    derived_pda.validate_for(
        route.adapter_program(),
        recipe.id()?,
        account.key,
        derived_pda.bump,
    )?;
    let data_id = account_data_id(account.key, account.data)?;
    let mut bytes = [0_u8; 168];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&account.key.bytes());
    bytes[64..96].copy_from_slice(&data_id.bytes());
    bytes[96..128].copy_from_slice(&receipt.id()?.bytes());
    bytes[128..160].copy_from_slice(&receipt.receipt_id().bytes());
    bytes[160] = derived_pda.bump;
    Ok(AuthenticatedSourceWorkReceiptV1 {
        account: account.key,
        account_data_id: data_id,
        receipt,
        schedule,
        pda_bump: derived_pda.bump,
        authentication_id: domain_id(SOURCE_WORK_RECEIPT_AUTH_DOMAIN, &bytes),
    })
}

fn runtime_key_at(input: &[u8], at: usize) -> RuntimeKey {
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&input[at..at + 32]);
    RuntimeKey::from_bytes(bytes)
}

fn content_id_at(input: &[u8], at: usize) -> ContentId {
    ContentId::from_bytes(runtime_key_at(input, at).bytes())
}

fn funding_le_u16(input: &[u8]) -> u16 {
    let mut bytes = [0_u8; 2];
    bytes.copy_from_slice(input);
    u16::from_le_bytes(bytes)
}

fn funding_le_u32(input: &[u8]) -> u32 {
    let mut bytes = [0_u8; 4];
    bytes.copy_from_slice(input);
    u32::from_le_bytes(bytes)
}

fn funding_le_u64(input: &[u8]) -> u64 {
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(input);
    u64::from_le_bytes(bytes)
}
