// SPDX-License-Identifier: AGPL-3.0-or-later

//! Sole hostile-byte and ordered-account grammar for current Dealer requests.
//!
//! Both the SBF adapter and off-chain operator consume this module directly;
//! neither is permitted to restate action payloads, role order, privilege bits,
//! or the narrowly allowed payer aliases.

use crate::{registry::DealerFacilityAction, MAX_OUTCOMES};

/// Wire-pinned LP entries per Dealer page.
pub const DEALER_LP_ENTRIES_PER_PAGE_V1: usize = 16;
/// Wire-pinned maximum Dealer LP page count.
pub const DEALER_MAX_LP_PAGES_V1: u32 = 4_096;

/// Common expected-generation and Replay-ordinal payload prefix.
pub const DEALER_RUNTIME_PAYLOAD_PREFIX_BYTES_V1: usize = 16;
/// Retire selector for one ExitTicket.
pub const DEALER_RETIRE_EXIT_TICKET_V1: u8 = 1;
/// Retire selector for one empty pre-activation LP tail.
pub const DEALER_RETIRE_EMPTY_LP_PAGE_V1: u8 = 2;
/// Retire selector for one Dealer Epoch binding.
pub const DEALER_RETIRE_EPOCH_BINDING_V1: u8 = 3;
/// Retire selector for one terminal page/allocation pair.
pub const DEALER_RETIRE_TERMINAL_PAGE_V1: u8 = 4;
/// Retire selector for the funded-dependency child.
pub const DEALER_RETIRE_FUNDED_DEPENDENCIES_V1: u8 = 5;
/// Retire selector for canonical PositionV3 plus ReplayV3.
pub const DEALER_RETIRE_POSITION_REPLAY_V1: u8 = 6;
/// Retire selector for the live State into its permanent tombstone.
pub const DEALER_RETIRE_STATE_ROOT_V1: u8 = 7;
/// Atomically terminalize a live facility a6 credit with current Product.
pub const DEALER_RETIRE_ACTIVE_FACILITY_CREDIT_V1: u8 = 8;
/// Atomically close never-consumed 0xbc funding with current Product.
pub const DEALER_RETIRE_UNUSED_FUTURE_CREDIT_V1: u8 = 9;

/// Strict Dealer wire/account-contract error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerRuntimeContractErrorV1 {
    /// Payload or account bytes were shorter than the exact contract.
    Truncated,
    /// Payload or account bytes were longer than the exact contract.
    TrailingBytes,
    /// A global account tag or version mismatched.
    WrongAccountCoordinate,
    /// Reserved bytes or envelope flags were nonzero.
    NonCanonicalPadding,
    /// A numeric selector was outside its action domain.
    InvalidField,
    /// The pure Dealer body codec refused.
    InvalidBody,
}


/// Frozen action-specific payload after the extension family/action envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerRuntimePayloadV1 {
    /// State generation the transaction expects to consume.
    pub expected_generation: u64,
    /// Replay ordinal the transaction expects to consume.
    pub expected_replay_ordinal: u64,
    /// Page ordinal for page-scoped actions; zero otherwise.
    pub page_ordinal: u32,
    /// Entry index for owner-scoped actions; zero otherwise.
    pub entry_index: u8,
    /// Whether an ExitTicket already exists; false otherwise.
    pub existing_ticket: bool,
    /// Whether QueueExit is externally funded; false otherwise.
    pub external_liveness: bool,
    /// Whether the facility already owns its Product Series obligation.
    pub existing_series_admission: bool,
    /// Retire target selector; zero outside Retire.
    pub retire_target: u8,
    /// Whether a terminal page close also closes singleton ClaimWork.
    pub terminal_last_page: bool,
    /// Exact share delta for contribution, withdrawal, or queueing.
    pub share_delta: u64,
    /// Facility disambiguator for Initialize; zero otherwise.
    pub facility_nonce: u64,
    /// Sponsor cash capital moved by Initialize; zero otherwise.
    pub sponsor_capital_atoms: u64,
    /// Generic-runtime monotone call ordinal for a funded action.
    pub liveness_call_ordinal: u32,
    /// Actual successful keeper payment, bounded by the immutable action quote.
    pub keeper_payment_lamports: u64,
    /// Bounded row start for Collect/Deliver.
    pub row_start: u16,
    /// Positive bounded row count for Collect/Deliver.
    pub row_count: u16,
    /// Exact active OrderPage V5 prefix for CoveredDealer selection.
    pub book_page_count: u8,
    /// Exact current GEN1 Replay sequence for the LP Position receiving a claim.
    pub expected_general_replay_sequence: u64,
    /// Exact a5 sequence consumed only by Resolve's bounded vector.
    pub expected_fractional_ledger_sequence: u64,
    /// Founding a6 sequence consumed only by Resolve.
    pub expected_fractional_credit_sequence: u64,
    /// Exact active vector width consumed only by Resolve.
    pub resolution_outcome_count: u8,
    /// Exact outcome-ordered facility inventory consumed by Resolve.
    pub resolution_quantities: [u64; MAX_OUTCOMES],
}

impl DealerRuntimePayloadV1 {
    /// Decode the exact action-dependent length and canonical inactive fields.
    pub fn decode(
        action: DealerFacilityAction,
        input: &[u8],
    ) -> Result<Self, DealerRuntimeContractErrorV1> {
        let suffix_len = match action {
            DealerFacilityAction::Initialize => 32,
            DealerFacilityAction::CreateLpPage => 20,
            DealerFacilityAction::Contribute | DealerFacilityAction::WithdrawFunding => 12,
            DealerFacilityAction::Activate
            | DealerFacilityAction::CancelFunding
            | DealerFacilityAction::RefundCancelledSponsor
            | DealerFacilityAction::BindEpoch
            | DealerFacilityAction::LapseEpoch
            | DealerFacilityAction::SelectLeaseAndBegin
            | DealerFacilityAction::EnterUnwind
            | DealerFacilityAction::TimedClose => 16,
            DealerFacilityAction::SponsorHalt => 8,
            DealerFacilityAction::Collect | DealerFacilityAction::Deliver => 24,
            DealerFacilityAction::FinalizeSettlement
            | DealerFacilityAction::AbortBeforeCollection => 16,
            DealerFacilityAction::QueueExit => 32,
            DealerFacilityAction::Claim => 32,
            DealerFacilityAction::Resolve => 168,
            DealerFacilityAction::Retire => {
                if input.len() < DEALER_RUNTIME_PAYLOAD_PREFIX_BYTES_V1 + 2 {
                    return Err(DealerRuntimeContractErrorV1::Truncated);
                }
                if matches!(
                    input[16],
                    DEALER_RETIRE_ACTIVE_FACILITY_CREDIT_V1
                        | DEALER_RETIRE_UNUSED_FUTURE_CREDIT_V1
                ) {
                    24
                } else {
                    8
                }
            }
            _ => 0,
        };
        let expected = DEALER_RUNTIME_PAYLOAD_PREFIX_BYTES_V1 + suffix_len;
        if input.len() < expected {
            return Err(DealerRuntimeContractErrorV1::Truncated);
        }
        if input.len() > expected {
            return Err(DealerRuntimeContractErrorV1::TrailingBytes);
        }
        let expected_generation = read_u64(input, 0);
        let expected_replay_ordinal = read_u64(input, 8);
        if expected_generation == 0 {
            return Err(DealerRuntimeContractErrorV1::InvalidField);
        }
        let mut value = Self {
            expected_generation,
            expected_replay_ordinal,
            page_ordinal: 0,
            entry_index: 0,
            existing_ticket: false,
            external_liveness: false,
            existing_series_admission: false,
            retire_target: 0,
            terminal_last_page: false,
            share_delta: 0,
            facility_nonce: 0,
            sponsor_capital_atoms: 0,
            liveness_call_ordinal: 0,
            keeper_payment_lamports: 0,
            row_start: 0,
            row_count: 0,
            book_page_count: 0,
            expected_general_replay_sequence: 0,
            expected_fractional_ledger_sequence: 0,
            expected_fractional_credit_sequence: 0,
            resolution_outcome_count: 0,
            resolution_quantities: [0; MAX_OUTCOMES],
        };
        match action {
            DealerFacilityAction::Initialize => {
                value.facility_nonce = read_u64(input, 16);
                value.sponsor_capital_atoms = read_u64(input, 24);
                value.liveness_call_ordinal = read_u32(input, 32);
                if input[36..40].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.keeper_payment_lamports = read_u64(input, 40);
                if value.expected_generation != 1
                    || value.expected_replay_ordinal != 0
                    || value.sponsor_capital_atoms == 0
                    || value.liveness_call_ordinal == 0
                {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            DealerFacilityAction::Activate
            | DealerFacilityAction::CancelFunding
            | DealerFacilityAction::RefundCancelledSponsor => {
                value.liveness_call_ordinal = read_u32(input, 16);
                if input[20..24].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.keeper_payment_lamports = read_u64(input, 24);
                if value.liveness_call_ordinal == 0 {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            DealerFacilityAction::BindEpoch | DealerFacilityAction::LapseEpoch => {
                value.liveness_call_ordinal = read_u32(input, 16);
                value.existing_series_admission = decode_bool(input[20])?;
                if input[21..24].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.keeper_payment_lamports = read_u64(input, 24);
                if value.liveness_call_ordinal == 0 {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            DealerFacilityAction::EnterUnwind | DealerFacilityAction::TimedClose => {
                value.liveness_call_ordinal = read_u32(input, 16);
                value.existing_series_admission = decode_bool(input[20])?;
                if input[21..24].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.keeper_payment_lamports = read_u64(input, 24);
                if value.liveness_call_ordinal == 0 {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            DealerFacilityAction::SponsorHalt => {
                value.existing_series_admission = decode_bool(input[16])?;
                if input[17..24].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
            }
            DealerFacilityAction::FinalizeSettlement
            | DealerFacilityAction::AbortBeforeCollection => {
                value.liveness_call_ordinal = read_u32(input, 16);
                if input[20..24].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.keeper_payment_lamports = read_u64(input, 24);
                if value.liveness_call_ordinal == 0 {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            DealerFacilityAction::CreateLpPage => {
                value.page_ordinal = read_u32(input, 16);
                value.liveness_call_ordinal = read_u32(input, 20);
                if input[24..28].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.keeper_payment_lamports = read_u64(input, 28);
                if value.liveness_call_ordinal == 0 {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            DealerFacilityAction::SelectLeaseAndBegin => {
                value.book_page_count = input[16];
                value.existing_series_admission = decode_bool(input[17])?;
                if input[18..20].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.liveness_call_ordinal = read_u32(input, 20);
                value.keeper_payment_lamports = read_u64(input, 24);
                if !(1..=4).contains(&value.book_page_count)
                    || value.liveness_call_ordinal == 0
                {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            DealerFacilityAction::Contribute | DealerFacilityAction::WithdrawFunding => {
                value.page_ordinal = read_u32(input, 16);
                value.share_delta = read_u64(input, 20);
                if value.share_delta == 0 {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            DealerFacilityAction::Collect | DealerFacilityAction::Deliver => {
                value.row_start = read_u16(input, 16);
                value.row_count = read_u16(input, 18);
                value.book_page_count = input[20];
                if input[21..24].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.liveness_call_ordinal = read_u32(input, 24);
                if input[28..32].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.keeper_payment_lamports = read_u64(input, 32);
                // One Reservation/Position/Replay incarnation is mutated per
                // instruction.  Batching rows would require a second trusted
                // delimiter and permit owner-crossing partial writes.
                if value.row_count != 1
                    || !(1..=4).contains(&value.book_page_count)
                    || value.liveness_call_ordinal == 0
                {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            DealerFacilityAction::QueueExit => {
                value.page_ordinal = read_u32(input, 16);
                value.entry_index = input[20];
                value.existing_ticket = decode_bool(input[21])?;
                value.external_liveness = decode_bool(input[22])?;
                value.existing_series_admission = decode_bool(input[23])?;
                value.share_delta = read_u64(input, 24);
                value.liveness_call_ordinal = read_u32(input, 32);
                if input[36..40].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.keeper_payment_lamports = read_u64(input, 40);
                if value.share_delta == 0
                    || (value.external_liveness && value.liveness_call_ordinal == 0)
                    || (!value.external_liveness
                        && (value.liveness_call_ordinal != 0
                            || value.keeper_payment_lamports != 0))
                {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            DealerFacilityAction::Claim => {
                value.page_ordinal = read_u32(input, 16);
                value.entry_index = input[20];
                if input[21..24].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.expected_general_replay_sequence = read_u64(input, 24);
                value.liveness_call_ordinal = read_u32(input, 32);
                if input[36..40].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.keeper_payment_lamports = read_u64(input, 40);
                if value.expected_general_replay_sequence == 0
                    || value.liveness_call_ordinal == 0
                {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            DealerFacilityAction::Resolve => {
                value.expected_fractional_ledger_sequence = read_u64(input, 16);
                value.expected_fractional_credit_sequence = read_u64(input, 24);
                value.resolution_outcome_count = input[32];
                if input[33..40].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                let mut index = 0usize;
                let mut any = false;
                while index < MAX_OUTCOMES {
                    value.resolution_quantities[index] = read_u64(input, 40 + index * 8);
                    any |= value.resolution_quantities[index] != 0;
                    index += 1;
                }
                value.liveness_call_ordinal = read_u32(input, 168);
                if input[172..176].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.keeper_payment_lamports = read_u64(input, 176);
                if value.expected_replay_ordinal == 0
                    || value.expected_fractional_ledger_sequence == 0
                    || value.expected_fractional_credit_sequence != 1
                    || value.resolution_outcome_count == 0
                    || usize::from(value.resolution_outcome_count) > MAX_OUTCOMES
                    || !any
                    || value.liveness_call_ordinal == 0
                {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
                let mut tail = usize::from(value.resolution_outcome_count);
                while tail < MAX_OUTCOMES {
                    if value.resolution_quantities[tail] != 0 {
                        return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                    }
                    tail += 1;
                }
            }
            DealerFacilityAction::Retire => {
                value.retire_target = input[16];
                value.terminal_last_page = decode_bool(input[17])?;
                if !(DEALER_RETIRE_EXIT_TICKET_V1..=DEALER_RETIRE_UNUSED_FUTURE_CREDIT_V1)
                    .contains(&value.retire_target)
                {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
                if input[18..20].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                if value.retire_target != DEALER_RETIRE_TERMINAL_PAGE_V1
                    && value.terminal_last_page
                {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
                value.page_ordinal = read_u32(input, 20);
                let page_target = matches!(
                    value.retire_target,
                    DEALER_RETIRE_EXIT_TICKET_V1
                        | DEALER_RETIRE_EMPTY_LP_PAGE_V1
                        | DEALER_RETIRE_TERMINAL_PAGE_V1
                );
                if (page_target
                    && value.page_ordinal >= DEALER_MAX_LP_PAGES_V1)
                    || (!page_target && value.page_ordinal != 0)
                {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
                if matches!(
                    value.retire_target,
                    DEALER_RETIRE_ACTIVE_FACILITY_CREDIT_V1
                        | DEALER_RETIRE_UNUSED_FUTURE_CREDIT_V1
                ) {
                    value.liveness_call_ordinal = read_u32(input, 24);
                    if input[28..32].iter().any(|byte| *byte != 0) {
                        return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                    }
                    value.keeper_payment_lamports = read_u64(input, 32);
                    if value.liveness_call_ordinal == 0 {
                        return Err(DealerRuntimeContractErrorV1::InvalidField);
                    }
                }
            }
            _ => {}
        }
        if usize::from(value.entry_index) >= DEALER_LP_ENTRIES_PER_PAGE_V1
            || value.page_ordinal >= DEALER_MAX_LP_PAGES_V1
                && action_uses_page(action)
        {
            return Err(DealerRuntimeContractErrorV1::InvalidField);
        }
        Ok(value)
    }
}

/// Runtime owner or sysvar class required for one ordered account role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerMetaOwnerV1 {
    /// Arbitrary non-executable read-only data, authenticated by signed bytes.
    AnyReadOnly,
    /// The deployed Dragon's Clutch program.
    SelfProgram,
    /// Canonical shared PositionV3/Replay owner authenticated by its adapter.
    PositionRuntime,
    /// Canonical external liveness runtime.
    LivenessRuntime,
    /// Authenticated General V2 market/Epoch runtime.
    GeneralV2Runtime,
    /// Canonical owner-netted fee runtime.
    FeeRuntime,
    /// System Program or a system-owned not-yet-created PDA.
    System,
    /// Clock sysvar at its exact well-known address.
    ClockSysvar,
    /// Rent sysvar at its exact well-known address.
    RentSysvar,
    /// Instructions sysvar at its exact well-known address.
    InstructionsSysvar,
    /// Signer identity with no data-owner requirement.
    Signer,
    /// External executable whose exact identity and loader state are checked
    /// by its owning adapter.
    ExternalExecutable,
}

/// Semantic role of one ordered Dealer instruction account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerMetaRoleV1 {
    /// Transaction signer/keeper.
    Actor,
    /// Immutable Dealer policy.
    Policy,
    /// Authoritative Dealer StateV2.
    State,
    /// Canonical facility PositionV3.
    FacilityPosition,
    /// Canonical facility ReplayV3.
    FacilityReplay,
    /// Counted funded-dependency child.
    FundedDependencies,
    /// One-shot future facility Fractional-credit rent owner.
    FutureCreditFunding,
    /// Immutable Dealer quote schedule.
    LivenessSchedule,
    /// Immutable generic runtime-liveness policy.
    LivenessPolicy,
    /// Selected external liveness compartment.
    LivenessCompartment,
    /// Source liveness compartment used by exhaustive initialization.
    LivenessSource,
    /// Candidate liveness compartment used by exhaustive initialization.
    LivenessCandidate,
    /// Clearing liveness compartment used by exhaustive initialization.
    LivenessClearing,
    /// Settlement liveness compartment used by exhaustive initialization.
    LivenessSettlement,
    /// Resolution liveness compartment used by exhaustive initialization.
    LivenessResolution,
    /// Retirement liveness compartment used by exhaustive initialization.
    LivenessRetirement,
    /// Recovery liveness compartment used by exhaustive initialization.
    LivenessRecovery,
    /// Typed successful liveness receipt.
    LivenessReceipt,
    /// Current or previous LP tail page.
    TailPage,
    /// Newly created successor LP page.
    NewPage,
    /// Ordinary LP PositionV3.
    LpPosition,
    /// Ordinary owner PositionV3 participating in one settlement row.
    GeneralPosition,
    /// Canonical General-purpose Replay V3 for the row Position.
    GeneralReplay,
    /// Canonical rent-owned General Reservation V9 for the row.
    Reservation,
    /// Sponsor funding PositionV3.
    SponsorPosition,
    /// Immutable sponsor refund-recipient identity.
    SponsorRefundRecipient,
    /// Immutable page containing an exit owner.
    LpPage,
    /// Previous LP page restored as tail during reverse close.
    PreviousLpPage,
    /// Unique owner-scoped ExitTicket.
    ExitTicket,
    /// Immutable terminal allocation for one LP page.
    TerminalAllocation,
    /// Singleton terminal allocation/claim/page-close work owner.
    ClaimWork,
    /// Newly created or existing Dealer Epoch-binding account.
    EpochBinding,
    /// Newly created or existing Dealer Lease V2 account.
    Lease,
    /// Newly created or existing Dealer SettlementPot V2 account.
    SettlementPot,
    /// Authenticated General V2 Epoch.
    GeneralEpoch,
    /// Authenticated General V2 candidate Window.
    GeneralWindow,
    /// Immutable selected-candidate artifact.
    SelectedCandidate,
    /// Selected sealed General feed that owns candidate and settlement witness bytes.
    SelectedFeed,
    /// Counted General SettlementRoot selected-candidate owner.
    SettlementRoot,
    /// Immutable General MarketBinding V2.
    MarketBinding,
    /// Immutable Realm selecting the collateral Profile.
    Realm,
    /// Immutable collateral Profile V2.
    CollateralProfile,
    /// Exact content-addressed collateral policy.
    CollateralPolicy,
    /// Profile-selected executable collateral token program.
    CollateralTokenProgram,
    /// Exact linked loader ProgramData observed in this instruction.
    CollateralTokenProgramData,
    /// Canonical General MarketRuntime V3.
    MarketRuntime,
    /// Full-width Market Hoard V2 liability and custody owner.
    Hoard,
    /// Full-width native ClaimLedger V3.
    ClaimLedger,
    /// Exact active and unresolved shared Product Market root.
    ProductMarketRoot,
    /// Permanent current Product Market replay/generation owner.
    ProductMarketReplay,
    /// Immutable Product Market family-capability policy artifact.
    MarketFamilyCapabilityPolicy,
    /// Current Product SeriesRegistry V2 selecting release/profile/bundle.
    SeriesRegistry,
    /// Current executable Dragon's Clutch program observed by its loader.
    CurrentProgram,
    /// Exact linked ProgramData for the current Registry release.
    CurrentProgramData,
    /// Content-addressed Registry ReleaseV2 artifact.
    RegistryRelease,
    /// Content-addressed Registry Capability ProfileV4 artifact.
    CapabilityProfile,
    /// Signed immutable CoveredDealer quote admission bytes.
    QuoteAdmission,
    /// Instructions sysvar proving the immediately preceding Ed25519 signature.
    InstructionsSysvar,
    /// Realm-selected scalar price grid.
    PriceGrid,
    /// Full-width Product MarketInstance V2 artifact.
    MarketInstance,
    /// Product template artifact owning the exact basis identity.
    ProductTemplate,
    /// Native claim-basis artifact.
    NativeBasis,
    /// Market genesis artifact owning coordinate bounds and policy identities.
    MarketGenesis,
    /// Immutable batch-policy body selected by MarketBinding V2.
    BatchPolicy,
    /// Immutable revenue-policy record selected by the fee record.
    RevenuePolicy,
    /// Realm-owned record binding the revenue-policy digest and treasury.
    RevenuePolicyRecord,
    /// Selected owner-netted fee record.
    FeeRecord,
    /// Canonical fee closure manifest paired with the terminal receipt.
    FeeClosureManifest,
    /// Canonical candidate-wide fee terminal receipt.
    FeeTerminalReceipt,
    /// Authenticated canonical EconomicDomainV2.
    EconomicDomain,
    /// Immutable quantized price-measure policy artifact.
    PriceMeasurePolicy,
    /// Newly materialized counted CoveredDealer selection certificate.
    CoveredSelection,
    /// Counted facility-lifetime Product Series obligation binding.
    SeriesObligation,
    /// Exact Product per-Series Market link whose Dealer latch advances once.
    SeriesMarketLink,
    /// Finalized full-width Resolution V5.
    Resolution,
    /// Canonical a4/v3 Fractional policy.
    FractionalPolicy,
    /// Canonical a5/v1 aggregate numerator ledger.
    FractionalLedger,
    /// Fresh facility-owned a6/v2 exact-remainder credit.
    FacilityCredit,
    /// Current V5 compiler bundle selected by the Product link.
    CompilerBundle,
    /// Current V4 attachment selecting the Dealer facility plan.
    Attachment,
    /// Canonical frozen General OrderPage V5.
    OrderPage,
    /// Sole refundable-rent recipient.
    RentPayer,
    /// Present-funding/rent recipient retained by one liveness compartment.
    LivenessPayer,
    /// Canonical neutral sink.
    NeutralSink,
    /// Sponsor refund-recipient PositionV3.
    SponsorRefundPosition,
    /// Clock sysvar.
    Clock,
    /// Rent sysvar.
    Rent,
    /// System Program.
    SystemProgram,
}

/// Exact metadata requirement for one account position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerMetaSpecV1 {
    /// Ordered semantic role.
    pub role: DealerMetaRoleV1,
    /// Required data owner class.
    pub owner: DealerMetaOwnerV1,
    /// Whether the account must sign.
    pub signer: bool,
    /// Whether the account must be writable.
    pub writable: bool,
}

const fn meta(
    role: DealerMetaRoleV1,
    owner: DealerMetaOwnerV1,
    signer: bool,
    writable: bool,
) -> DealerMetaSpecV1 {
    DealerMetaSpecV1 {
        role,
        owner,
        signer,
        writable,
    }
}

const INITIALIZE: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::SponsorRefundRecipient, DealerMetaOwnerV1::Signer, false, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::SponsorPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
    meta(DealerMetaRoleV1::Realm, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralProfile, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgram, DealerMetaOwnerV1::ExternalExecutable, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgramData, DealerMetaOwnerV1::AnyReadOnly, false, false),
    meta(DealerMetaRoleV1::MarketBinding, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketRuntime, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketInstance, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::Hoard, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ClaimLedger, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::GeneralReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FutureCreditFunding, DealerMetaOwnerV1::System, false, true),
];

const CREATE_FIRST: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::NewPage, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
];

const CREATE_NEXT: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::TailPage, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::NewPage, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
];

const LP_TRANSFER: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::LpPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::TailPage, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::Realm, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralProfile, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgram, DealerMetaOwnerV1::ExternalExecutable, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgramData, DealerMetaOwnerV1::AnyReadOnly, false, false),
    meta(DealerMetaRoleV1::MarketBinding, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketRuntime, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketInstance, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::Hoard, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ClaimLedger, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::GeneralReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
];

const ACTIVATE: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::TailPage, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
];

const SPONSOR_HALT: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::SelfProgram, false, false),
];

const TIMED_CLOSE: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::SelfProgram, false, false),
];

const ENTER_UNWIND: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::SelfProgram, false, false),
];

const BIND_EPOCH: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::EpochBinding, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::GeneralEpoch, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::GeneralWindow, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::EconomicDomain, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::SelfProgram, false, false),
];

const LAPSE_EPOCH: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::EpochBinding, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::NeutralSink, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::SelfProgram, false, false),
];

const SELECT_LEASE_BEGIN_FIXED_COUNT: usize = 53;
const SELECT_LEASE_BEGIN_FIRST: [DealerMetaSpecV1; 57] = [
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::EpochBinding, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::SettlementRoot, DealerMetaOwnerV1::GeneralV2Runtime, false, true),
    meta(DealerMetaRoleV1::SelectedFeed, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::EconomicDomain, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketBinding, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::QuoteAdmission, DealerMetaOwnerV1::AnyReadOnly, false, false),
    meta(DealerMetaRoleV1::InstructionsSysvar, DealerMetaOwnerV1::InstructionsSysvar, false, false),
    meta(DealerMetaRoleV1::PriceGrid, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::MarketInstance, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ProductTemplate, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::NativeBasis, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::PriceMeasurePolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::MarketGenesis, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::BatchPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::RevenuePolicy, DealerMetaOwnerV1::AnyReadOnly, false, false),
    meta(DealerMetaRoleV1::RevenuePolicyRecord, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::FeeRecord, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CoveredSelection, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::Lease, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::SettlementPot, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
    meta(DealerMetaRoleV1::Realm, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralProfile, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgram, DealerMetaOwnerV1::ExternalExecutable, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgramData, DealerMetaOwnerV1::AnyReadOnly, false, false),
    meta(DealerMetaRoleV1::MarketRuntime, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::Hoard, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ClaimLedger, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ProductMarketRoot, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::ProductMarketReplay, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::MarketFamilyCapabilityPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::SeriesMarketLink, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
];

const fn select_lease_begin_existing_contract() -> [DealerMetaSpecV1; 57] {
    let mut contract = SELECT_LEASE_BEGIN_FIRST;
    contract[48] = meta(
        DealerMetaRoleV1::ProductMarketRoot,
        DealerMetaOwnerV1::SelfProgram,
        false,
        false,
    );
    contract[51] = meta(
        DealerMetaRoleV1::SeriesMarketLink,
        DealerMetaOwnerV1::SelfProgram,
        false,
        false,
    );
    contract[52] = meta(
        DealerMetaRoleV1::SeriesObligation,
        DealerMetaOwnerV1::SelfProgram,
        false,
        false,
    );
    contract
}

const SELECT_LEASE_BEGIN_EXISTING: [DealerMetaSpecV1; 57] =
    select_lease_begin_existing_contract();

const COLLECT_DELIVER_FIXED_COUNT: usize = 43;
const COLLECT_DELIVER: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::EpochBinding, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::CoveredSelection, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::Lease, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::SettlementPot, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::SettlementRoot, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::SelectedFeed, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::EconomicDomain, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::Reservation, DealerMetaOwnerV1::GeneralV2Runtime, false, true),
    meta(DealerMetaRoleV1::GeneralPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::GeneralReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
    meta(DealerMetaRoleV1::MarketBinding, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::PriceGrid, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::MarketGenesis, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::Realm, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralProfile, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgram, DealerMetaOwnerV1::ExternalExecutable, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgramData, DealerMetaOwnerV1::AnyReadOnly, false, false),
    meta(DealerMetaRoleV1::MarketRuntime, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketInstance, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::Hoard, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ClaimLedger, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
];

const FINALIZE_ABORT_ACCOUNT_COUNT: usize = 40;
const fn finalize_abort_contract(finalize: bool) -> [DealerMetaSpecV1; FINALIZE_ABORT_ACCOUNT_COUNT] {
    [
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::EpochBinding, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, finalize),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, !finalize),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::CoveredSelection, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::Lease, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::SettlementPot, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FeeClosureManifest, DealerMetaOwnerV1::FeeRuntime, false, false),
    meta(DealerMetaRoleV1::FeeTerminalReceipt, DealerMetaOwnerV1::FeeRuntime, false, false),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::NeutralSink, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::Realm, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralProfile, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgram, DealerMetaOwnerV1::ExternalExecutable, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgramData, DealerMetaOwnerV1::AnyReadOnly, false, false),
    meta(DealerMetaRoleV1::MarketBinding, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketRuntime, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketInstance, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::Hoard, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ClaimLedger, DealerMetaOwnerV1::SelfProgram, false, false),
    ]
}
const FINALIZE_SETTLEMENT: &[DealerMetaSpecV1] = &finalize_abort_contract(true);
const ABORT_BEFORE_COLLECTION: &[DealerMetaSpecV1] = &finalize_abort_contract(false);

const CANCEL_FUNDING: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
];

const REFUND_SPONSOR: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::SponsorRefundPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
    meta(DealerMetaRoleV1::Realm, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralProfile, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgram, DealerMetaOwnerV1::ExternalExecutable, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgramData, DealerMetaOwnerV1::AnyReadOnly, false, false),
    meta(DealerMetaRoleV1::MarketBinding, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketRuntime, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketInstance, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::Hoard, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ClaimLedger, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::GeneralReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
];

const RETIRE_ACTIVE_FACILITY_CREDIT: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::NeutralSink, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::Realm, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralProfile, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgram, DealerMetaOwnerV1::ExternalExecutable, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgramData, DealerMetaOwnerV1::AnyReadOnly, false, false),
    meta(DealerMetaRoleV1::MarketBinding, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketRuntime, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketInstance, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::Hoard, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ClaimLedger, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::Resolution, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::FractionalPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::FractionalLedger, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityCredit, DealerMetaOwnerV1::SelfProgram, false, true),
];

/// Current terminal cut when Resolve never consumed Dealer's 0xbc/v1 owner.
const RETIRE_UNUSED_FUTURE_CREDIT: &[DealerMetaSpecV1] = &[
    RETIRE_ACTIVE_FACILITY_CREDIT[0], RETIRE_ACTIVE_FACILITY_CREDIT[1],
    RETIRE_ACTIVE_FACILITY_CREDIT[2], RETIRE_ACTIVE_FACILITY_CREDIT[3],
    RETIRE_ACTIVE_FACILITY_CREDIT[4], RETIRE_ACTIVE_FACILITY_CREDIT[5],
    RETIRE_ACTIVE_FACILITY_CREDIT[6], RETIRE_ACTIVE_FACILITY_CREDIT[7],
    RETIRE_ACTIVE_FACILITY_CREDIT[8], RETIRE_ACTIVE_FACILITY_CREDIT[9],
    RETIRE_ACTIVE_FACILITY_CREDIT[10], RETIRE_ACTIVE_FACILITY_CREDIT[11],
    RETIRE_ACTIVE_FACILITY_CREDIT[12], RETIRE_ACTIVE_FACILITY_CREDIT[13],
    RETIRE_ACTIVE_FACILITY_CREDIT[14], RETIRE_ACTIVE_FACILITY_CREDIT[15],
    RETIRE_ACTIVE_FACILITY_CREDIT[16], RETIRE_ACTIVE_FACILITY_CREDIT[17],
    RETIRE_ACTIVE_FACILITY_CREDIT[18], RETIRE_ACTIVE_FACILITY_CREDIT[19],
    RETIRE_ACTIVE_FACILITY_CREDIT[20], RETIRE_ACTIVE_FACILITY_CREDIT[21],
    RETIRE_ACTIVE_FACILITY_CREDIT[22], RETIRE_ACTIVE_FACILITY_CREDIT[23],
    RETIRE_ACTIVE_FACILITY_CREDIT[24], RETIRE_ACTIVE_FACILITY_CREDIT[25],
    RETIRE_ACTIVE_FACILITY_CREDIT[26], RETIRE_ACTIVE_FACILITY_CREDIT[27],
    RETIRE_ACTIVE_FACILITY_CREDIT[28], RETIRE_ACTIVE_FACILITY_CREDIT[29],
    RETIRE_ACTIVE_FACILITY_CREDIT[30], RETIRE_ACTIVE_FACILITY_CREDIT[31],
    RETIRE_ACTIVE_FACILITY_CREDIT[32], RETIRE_ACTIVE_FACILITY_CREDIT[33],
    meta(DealerMetaRoleV1::ClaimLedger, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::FutureCreditFunding, DealerMetaOwnerV1::SelfProgram, false, true),
];

const CLAIM_TERMINAL: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::LpPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::GeneralReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::TerminalAllocation, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::ClaimWork, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
    meta(DealerMetaRoleV1::Realm, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralProfile, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgram, DealerMetaOwnerV1::ExternalExecutable, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgramData, DealerMetaOwnerV1::AnyReadOnly, false, false),
    meta(DealerMetaRoleV1::MarketBinding, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketRuntime, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketInstance, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::Hoard, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ClaimLedger, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::SelfProgram, false, false),
];

/// Atomic Dealer Resolve/Fractional vector frame. Fractional owns roles 27..40;
/// Dealer writes no Position bytes
/// after the private Fractional receipt returns.
const RESOLVE_FACILITY_VECTOR: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::ClaimWork, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::FutureCreditFunding, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::NeutralSink, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
    meta(DealerMetaRoleV1::ProductMarketRoot, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::SeriesMarketLink, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::Realm, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralProfile, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgram, DealerMetaOwnerV1::ExternalExecutable, false, false),
    meta(DealerMetaRoleV1::CollateralTokenProgramData, DealerMetaOwnerV1::AnyReadOnly, false, false),
    meta(DealerMetaRoleV1::MarketBinding, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketRuntime, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::MarketInstance, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::Hoard, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::ClaimLedger, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::Resolution, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::FractionalPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::FractionalLedger, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityCredit, DealerMetaOwnerV1::System, false, true),
];

const QUEUE_NEW_CALLER: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ExitTicket, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
];

const QUEUE_NEW_CALLER_PRE_ADMISSION: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ExitTicket, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
];

const QUEUE_EXISTING_CALLER: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ExitTicket, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::SelfProgram, false, false),
];

const QUEUE_NEW_EXTERNAL: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ExitTicket, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::SelfProgram, false, false),
];

const QUEUE_EXISTING_EXTERNAL: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ExitTicket, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessPolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Rent, DealerMetaOwnerV1::RentSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
    meta(DealerMetaRoleV1::SeriesObligation, DealerMetaOwnerV1::SelfProgram, false, false),
];

/// Return the frozen exact meta order for implemented pure-core actions.
///
/// `None` means the action remains allocated but has no complete handler
/// contract and therefore cannot be enabled by any profile.
pub fn meta_contract_v1(
    action: DealerFacilityAction,
    payload: DealerRuntimePayloadV1,
) -> Option<&'static [DealerMetaSpecV1]> {
    match action {
        DealerFacilityAction::Initialize => Some(INITIALIZE),
        DealerFacilityAction::CreateLpPage if payload.page_ordinal == 0 => Some(CREATE_FIRST),
        DealerFacilityAction::CreateLpPage => Some(CREATE_NEXT),
        DealerFacilityAction::Contribute | DealerFacilityAction::WithdrawFunding => {
            Some(LP_TRANSFER)
        }
        DealerFacilityAction::Activate => Some(ACTIVATE),
        DealerFacilityAction::CancelFunding => Some(CANCEL_FUNDING),
        DealerFacilityAction::RefundCancelledSponsor => Some(REFUND_SPONSOR),
        DealerFacilityAction::BindEpoch if payload.existing_series_admission => Some(BIND_EPOCH),
        DealerFacilityAction::BindEpoch => Some(&BIND_EPOCH[..BIND_EPOCH.len() - 1]),
        DealerFacilityAction::LapseEpoch if payload.existing_series_admission => Some(LAPSE_EPOCH),
        DealerFacilityAction::LapseEpoch => Some(&LAPSE_EPOCH[..LAPSE_EPOCH.len() - 1]),
        DealerFacilityAction::SelectLeaseAndBegin => {
            let contract = if payload.existing_series_admission {
                &SELECT_LEASE_BEGIN_EXISTING
            } else {
                &SELECT_LEASE_BEGIN_FIRST
            };
            Some(&contract[..SELECT_LEASE_BEGIN_FIXED_COUNT + usize::from(payload.book_page_count)])
        }
        DealerFacilityAction::Collect | DealerFacilityAction::Deliver => Some(
            &COLLECT_DELIVER
                [..COLLECT_DELIVER_FIXED_COUNT + usize::from(payload.book_page_count)],
        ),
        DealerFacilityAction::FinalizeSettlement => Some(FINALIZE_SETTLEMENT),
        DealerFacilityAction::AbortBeforeCollection => Some(ABORT_BEFORE_COLLECTION),
        DealerFacilityAction::SponsorHalt if payload.existing_series_admission => {
            Some(SPONSOR_HALT)
        }
        DealerFacilityAction::SponsorHalt => Some(&SPONSOR_HALT[..SPONSOR_HALT.len() - 1]),
        DealerFacilityAction::EnterUnwind if payload.existing_series_admission => {
            Some(ENTER_UNWIND)
        }
        DealerFacilityAction::EnterUnwind => Some(&ENTER_UNWIND[..ENTER_UNWIND.len() - 1]),
        DealerFacilityAction::TimedClose if payload.existing_series_admission => {
            Some(TIMED_CLOSE)
        }
        DealerFacilityAction::TimedClose => Some(&TIMED_CLOSE[..TIMED_CLOSE.len() - 1]),
        DealerFacilityAction::QueueExit
            if !payload.external_liveness
                && !payload.existing_ticket
                && payload.existing_series_admission =>
        {
            Some(QUEUE_NEW_CALLER)
        }
        DealerFacilityAction::QueueExit
            if !payload.external_liveness && !payload.existing_ticket =>
        {
            Some(QUEUE_NEW_CALLER_PRE_ADMISSION)
        }
        DealerFacilityAction::QueueExit
            if !payload.external_liveness && payload.existing_series_admission =>
        {
            Some(QUEUE_EXISTING_CALLER)
        }
        DealerFacilityAction::QueueExit if !payload.external_liveness => {
            Some(&QUEUE_EXISTING_CALLER[..QUEUE_EXISTING_CALLER.len() - 1])
        }
        DealerFacilityAction::QueueExit
            if !payload.existing_ticket && payload.existing_series_admission =>
        {
            Some(QUEUE_NEW_EXTERNAL)
        }
        DealerFacilityAction::QueueExit if !payload.existing_ticket => {
            Some(&QUEUE_NEW_EXTERNAL[..QUEUE_NEW_EXTERNAL.len() - 1])
        }
        DealerFacilityAction::QueueExit if payload.existing_series_admission => {
            Some(QUEUE_EXISTING_EXTERNAL)
        }
        DealerFacilityAction::QueueExit => {
            Some(&QUEUE_EXISTING_EXTERNAL[..QUEUE_EXISTING_EXTERNAL.len() - 1])
        }
        DealerFacilityAction::Claim => Some(CLAIM_TERMINAL),
        DealerFacilityAction::Resolve => Some(RESOLVE_FACILITY_VECTOR),
        DealerFacilityAction::Retire
            if payload.retire_target == DEALER_RETIRE_ACTIVE_FACILITY_CREDIT_V1 =>
        {
            Some(RETIRE_ACTIVE_FACILITY_CREDIT)
        }
        DealerFacilityAction::Retire
            if payload.retire_target == DEALER_RETIRE_UNUSED_FUTURE_CREDIT_V1 =>
        {
            Some(RETIRE_UNUSED_FUTURE_CREDIT)
        }
        _ => None,
    }
}

/// Enforce the V1 alias rule: every ordered runtime account is nonzero and
/// semantic-owner accounts are pairwise distinct. Recipient slots may alias
/// one another or the keeper because independent rent/liveness principals can
/// legitimately share a payer; the pure transition still proves every exact
/// credit independently and forbids every payer/sink collapse.
pub fn validate_meta_keys_distinct_v1(
    contract: &[DealerMetaSpecV1],
    keys: &[[u8; 32]],
) -> Result<(), DealerRuntimeContractErrorV1> {
    if keys.len() < contract.len() {
        return Err(DealerRuntimeContractErrorV1::Truncated);
    }
    if keys.len() > contract.len() {
        return Err(DealerRuntimeContractErrorV1::TrailingBytes);
    }
    let mut index = 0usize;
    while index < keys.len() {
        if keys[index] == [0; 32] && contract[index].role != DealerMetaRoleV1::SystemProgram {
            return Err(DealerRuntimeContractErrorV1::InvalidField);
        }
        let mut prior = 0usize;
        while prior < index {
            if keys[index] == keys[prior]
                && !recipient_alias_allowed_v1(contract[index].role, contract[prior].role)
            {
                return Err(DealerRuntimeContractErrorV1::InvalidField);
            }
            prior += 1;
        }
        index += 1;
    }
    Ok(())
}


/// Whether two ordered roles may intentionally share one physical recipient.
///
/// Every non-recipient semantic owner remains pairwise distinct.
pub const fn recipient_alias_allowed_v1(left: DealerMetaRoleV1, right: DealerMetaRoleV1) -> bool {
    matches!(
        (left, right),
        (DealerMetaRoleV1::RentPayer, DealerMetaRoleV1::RentPayer)
            | (DealerMetaRoleV1::LivenessPayer, DealerMetaRoleV1::LivenessPayer)
            | (DealerMetaRoleV1::RentPayer, DealerMetaRoleV1::LivenessPayer)
            | (DealerMetaRoleV1::LivenessPayer, DealerMetaRoleV1::RentPayer)
            | (DealerMetaRoleV1::Actor, DealerMetaRoleV1::RentPayer)
            | (DealerMetaRoleV1::RentPayer, DealerMetaRoleV1::Actor)
            | (DealerMetaRoleV1::Actor, DealerMetaRoleV1::LivenessPayer)
            | (DealerMetaRoleV1::LivenessPayer, DealerMetaRoleV1::Actor)
    )
}


const fn action_uses_page(action: DealerFacilityAction) -> bool {
    matches!(
        action,
        DealerFacilityAction::CreateLpPage
            | DealerFacilityAction::Contribute
            | DealerFacilityAction::WithdrawFunding
            | DealerFacilityAction::QueueExit
            | DealerFacilityAction::Claim
    )
}

fn decode_bool(value: u8) -> Result<bool, DealerRuntimeContractErrorV1> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(DealerRuntimeContractErrorV1::InvalidField),
    }
}

fn read_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}

fn read_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}

fn read_u64(input: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
        input[offset + 4],
        input[offset + 5],
        input[offset + 6],
        input[offset + 7],
    ])
}

impl DealerMetaOwnerV1 {
    /// Return the stable operator label for this owner class.
    pub const fn label(self) -> &'static str {
        match self {
            Self::AnyReadOnly => "any-read-only",
            Self::SelfProgram => "self-program",
            Self::PositionRuntime => "position-runtime",
            Self::LivenessRuntime => "liveness-runtime",
            Self::GeneralV2Runtime => "general-v2-runtime",
            Self::FeeRuntime => "fee-runtime",
            Self::System => "system",
            Self::ClockSysvar => "clock-sysvar",
            Self::RentSysvar => "rent-sysvar",
            Self::InstructionsSysvar => "instructions-sysvar",
            Self::Signer => "signer",
            Self::ExternalExecutable => "external-executable",
        }
    }
}

impl DealerMetaRoleV1 {
    /// Return the stable operator label for this ordered role.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Actor => "actor",
            Self::Policy => "policy",
            Self::State => "state",
            Self::FacilityPosition => "facility-position",
            Self::FacilityReplay => "facility-replay",
            Self::FundedDependencies => "funded-dependencies",
            Self::FutureCreditFunding => "future-credit-funding",
            Self::LivenessSchedule => "liveness-schedule",
            Self::LivenessPolicy => "liveness-policy",
            Self::LivenessCompartment => "liveness-compartment",
            Self::LivenessSource => "liveness-source",
            Self::LivenessCandidate => "liveness-candidate",
            Self::LivenessClearing => "liveness-clearing",
            Self::LivenessSettlement => "liveness-settlement",
            Self::LivenessResolution => "liveness-resolution",
            Self::LivenessRetirement => "liveness-retirement",
            Self::LivenessRecovery => "liveness-recovery",
            Self::LivenessReceipt => "liveness-receipt",
            Self::TailPage => "tail-page",
            Self::NewPage => "new-page",
            Self::LpPosition => "lp-position",
            Self::GeneralPosition => "general-position",
            Self::GeneralReplay => "general-replay",
            Self::Reservation => "reservation",
            Self::SponsorPosition => "sponsor-position",
            Self::SponsorRefundRecipient => "sponsor-refund-recipient",
            Self::LpPage => "lp-page",
            Self::PreviousLpPage => "previous-lp-page",
            Self::ExitTicket => "exit-ticket",
            Self::TerminalAllocation => "terminal-allocation",
            Self::ClaimWork => "claim-work",
            Self::EpochBinding => "epoch-binding",
            Self::Lease => "lease",
            Self::SettlementPot => "settlement-pot",
            Self::GeneralEpoch => "general-epoch",
            Self::GeneralWindow => "general-window",
            Self::SelectedCandidate => "selected-candidate",
            Self::SelectedFeed => "selected-feed",
            Self::SettlementRoot => "settlement-root",
            Self::MarketBinding => "market-binding",
            Self::Realm => "realm",
            Self::CollateralProfile => "collateral-profile",
            Self::CollateralPolicy => "collateral-policy",
            Self::CollateralTokenProgram => "collateral-token-program",
            Self::CollateralTokenProgramData => "collateral-token-program-data",
            Self::MarketRuntime => "market-runtime",
            Self::Hoard => "hoard",
            Self::ClaimLedger => "claim-ledger",
            Self::ProductMarketRoot => "product-market-root",
            Self::ProductMarketReplay => "product-market-replay",
            Self::MarketFamilyCapabilityPolicy => "market-family-capability-policy",
            Self::SeriesRegistry => "series-registry",
            Self::CurrentProgram => "current-program",
            Self::CurrentProgramData => "current-program-data",
            Self::RegistryRelease => "registry-release",
            Self::CapabilityProfile => "capability-profile",
            Self::QuoteAdmission => "quote-admission",
            Self::InstructionsSysvar => "instructions-sysvar",
            Self::PriceGrid => "price-grid",
            Self::MarketInstance => "market-instance",
            Self::ProductTemplate => "product-template",
            Self::NativeBasis => "native-basis",
            Self::MarketGenesis => "market-genesis",
            Self::BatchPolicy => "batch-policy",
            Self::RevenuePolicy => "revenue-policy",
            Self::RevenuePolicyRecord => "revenue-policy-record",
            Self::FeeRecord => "fee-record",
            Self::FeeClosureManifest => "fee-closure-manifest",
            Self::FeeTerminalReceipt => "fee-terminal-receipt",
            Self::EconomicDomain => "economic-domain",
            Self::PriceMeasurePolicy => "price-measure-policy",
            Self::CoveredSelection => "covered-selection",
            Self::SeriesObligation => "series-obligation",
            Self::SeriesMarketLink => "series-market-link",
            Self::Resolution => "resolution",
            Self::FractionalPolicy => "fractional-policy",
            Self::FractionalLedger => "fractional-ledger",
            Self::FacilityCredit => "facility-credit",
            Self::CompilerBundle => "compiler-bundle",
            Self::Attachment => "attachment",
            Self::OrderPage => "order-page",
            Self::RentPayer => "rent-payer",
            Self::LivenessPayer => "liveness-payer",
            Self::NeutralSink => "neutral-sink",
            Self::SponsorRefundPosition => "sponsor-refund-position",
            Self::Clock => "clock",
            Self::Rent => "rent",
            Self::SystemProgram => "system-program",
        }
    }
}
