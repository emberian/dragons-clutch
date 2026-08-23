// SPDX-License-Identifier: AGPL-3.0-or-later

//! Capability-disabled Dealer facility wire and account contract.
//!
//! This module freezes strict payload parsing, the global eight-byte Dealer
//! account envelope, and ordered semantic account roles for the PositionV3
//! funding/activation/unwind/retirement slice. It deliberately exposes no executable
//! handler: capability dispatch refuses every Dealer facility action before
//! reading accounts.

use clutch_dealer_runtime_contract::FixedCodec;
use clutch_solana_layout::registry::DealerFacilityAction;

use crate::error::{ClutchError, Refusal};

/// Exact global Dealer account-envelope bytes.
pub const DEALER_ACCOUNT_ENVELOPE_BYTES_V1: usize = 8;
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

/// Strict disabled-contract error.
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

/// Strict common global account header used only by Dealer tags `0x93..=0x9f`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerAccountEnvelopeV1 {
    /// Global central-registry account tag.
    pub tag: u8,
    /// Global central-registry account version.
    pub version: u8,
    /// PDA bump authenticated against the exact family recipe.
    pub bump: u8,
}

impl DealerAccountEnvelopeV1 {
    /// Decode an exact tag/version and require zero flags/reserved bytes.
    pub fn decode(
        input: &[u8],
        expected_tag: u8,
        expected_version: u8,
    ) -> Result<Self, DealerRuntimeContractErrorV1> {
        if input.len() < DEALER_ACCOUNT_ENVELOPE_BYTES_V1 {
            return Err(DealerRuntimeContractErrorV1::Truncated);
        }
        if input[0] != expected_tag || input[1] != expected_version {
            return Err(DealerRuntimeContractErrorV1::WrongAccountCoordinate);
        }
        if input[3..DEALER_ACCOUNT_ENVELOPE_BYTES_V1]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
        }
        Ok(Self {
            tag: input[0],
            version: input[1],
            bump: input[2],
        })
    }
}

/// Decode one exact pure Dealer body behind its global envelope.
pub fn decode_dealer_account_body_v1<T: FixedCodec>(
    input: &[u8],
    expected_tag: u8,
    expected_version: u8,
) -> Result<(DealerAccountEnvelopeV1, T), DealerRuntimeContractErrorV1> {
    let expected = DEALER_ACCOUNT_ENVELOPE_BYTES_V1
        .checked_add(T::ENCODED_LEN)
        .ok_or(DealerRuntimeContractErrorV1::InvalidField)?;
    if input.len() < expected {
        return Err(DealerRuntimeContractErrorV1::Truncated);
    }
    if input.len() > expected {
        return Err(DealerRuntimeContractErrorV1::TrailingBytes);
    }
    let envelope = DealerAccountEnvelopeV1::decode(input, expected_tag, expected_version)?;
    let body = T::decode(&input[DEALER_ACCOUNT_ENVELOPE_BYTES_V1..])
        .map_err(|_| DealerRuntimeContractErrorV1::InvalidBody)?;
    Ok((envelope, body))
}

/// Encode one exact pure Dealer body behind its strict global envelope.
pub fn encode_dealer_account_body_v1<T: FixedCodec>(
    output: &mut [u8],
    tag: u8,
    version: u8,
    bump: u8,
    body: &T,
) -> Result<(), DealerRuntimeContractErrorV1> {
    let expected = DEALER_ACCOUNT_ENVELOPE_BYTES_V1
        .checked_add(T::ENCODED_LEN)
        .ok_or(DealerRuntimeContractErrorV1::InvalidField)?;
    if output.len() < expected {
        return Err(DealerRuntimeContractErrorV1::Truncated);
    }
    if output.len() > expected {
        return Err(DealerRuntimeContractErrorV1::TrailingBytes);
    }
    output[..DEALER_ACCOUNT_ENVELOPE_BYTES_V1]
        .copy_from_slice(&[tag, version, bump, 0, 0, 0, 0, 0]);
    body.encode_into(&mut output[DEALER_ACCOUNT_ENVELOPE_BYTES_V1..])
        .map_err(|_| DealerRuntimeContractErrorV1::InvalidBody)
}

/// Persisted Dealer account lifetime and close authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerAccountLifetimeV1 {
    /// Immutable content artifact; it has no mutable close route.
    Immutable,
    /// Authoritative live root that shrinks to a permanent tombstone.
    RootToTombstone,
    /// State-counted child with independently refundable rent.
    CountedChild,
    /// Singleton streamed work child with independently refundable rent.
    CountedWork,
    /// Permanent evidence body.
    Permanent,
}

/// Central coordinate/size/lifetime contract consumed by future handlers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerPersistedAccountContractV1 {
    /// Exact central account tag.
    pub tag: u8,
    /// Exact central account version.
    pub version: u8,
    /// Exact total bytes including the eight-byte envelope.
    pub account_bytes: usize,
    /// Semantic lifetime and close owner.
    pub lifetime: DealerAccountLifetimeV1,
}

/// Resolve one exact reserved Dealer account coordinate.
pub const fn persisted_account_contract_v1(
    tag: u8,
    version: u8,
) -> Option<DealerPersistedAccountContractV1> {
    use clutch_solana_layout::registry as registry;
    let (expected_version, account_bytes, lifetime) = match tag {
        registry::DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG => (
            registry::DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION,
            registry::DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES,
            DealerAccountLifetimeV1::Immutable,
        ),
        registry::DEALER_STATE_V2_ACCOUNT_TAG => (
            registry::DEALER_STATE_V2_ACCOUNT_VERSION,
            registry::DEALER_STATE_V2_ACCOUNT_BYTES,
            DealerAccountLifetimeV1::RootToTombstone,
        ),
        registry::DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG => (
            registry::DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION,
            registry::DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES,
            DealerAccountLifetimeV1::CountedChild,
        ),
        registry::DEALER_LP_PAGE_V2_ACCOUNT_TAG => (
            registry::DEALER_LP_PAGE_V2_ACCOUNT_VERSION,
            registry::DEALER_LP_PAGE_V2_ACCOUNT_BYTES,
            DealerAccountLifetimeV1::CountedChild,
        ),
        registry::DEALER_LEASE_V2_ACCOUNT_TAG => (
            registry::DEALER_LEASE_V2_ACCOUNT_VERSION,
            registry::DEALER_LEASE_V2_ACCOUNT_BYTES,
            DealerAccountLifetimeV1::CountedChild,
        ),
        registry::DEALER_SETTLEMENT_POT_V2_ACCOUNT_TAG => (
            registry::DEALER_SETTLEMENT_POT_V2_ACCOUNT_VERSION,
            registry::DEALER_SETTLEMENT_POT_V2_ACCOUNT_BYTES,
            DealerAccountLifetimeV1::CountedChild,
        ),
        registry::DEALER_EPOCH_BINDING_V2_ACCOUNT_TAG => (
            registry::DEALER_EPOCH_BINDING_V2_ACCOUNT_VERSION,
            registry::DEALER_EPOCH_BINDING_V2_ACCOUNT_BYTES,
            DealerAccountLifetimeV1::CountedChild,
        ),
        registry::DEALER_TERMINAL_ALLOCATION_ACCOUNT_TAG => (
            registry::DEALER_TERMINAL_ALLOCATION_ACCOUNT_VERSION,
            registry::DEALER_TERMINAL_ALLOCATION_ACCOUNT_BYTES,
            DealerAccountLifetimeV1::CountedChild,
        ),
        registry::DEALER_CLAIM_WORK_ACCOUNT_TAG => (
            registry::DEALER_CLAIM_WORK_ACCOUNT_VERSION,
            registry::DEALER_CLAIM_WORK_ACCOUNT_BYTES,
            DealerAccountLifetimeV1::CountedWork,
        ),
        registry::DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_TAG => (
            registry::DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_VERSION,
            registry::DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_BYTES,
            DealerAccountLifetimeV1::Permanent,
        ),
        registry::DEALER_EXIT_TICKET_ACCOUNT_TAG => (
            registry::DEALER_EXIT_TICKET_ACCOUNT_VERSION,
            registry::DEALER_EXIT_TICKET_ACCOUNT_BYTES,
            DealerAccountLifetimeV1::CountedChild,
        ),
        _ => return None,
    };
    if version != expected_version {
        return None;
    }
    Some(DealerPersistedAccountContractV1 {
        tag,
        version,
        account_bytes,
        lifetime,
    })
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
    /// Retire target selector; zero outside Retire.
    pub retire_target: u8,
    /// Whether a terminal page close also closes singleton ClaimWork.
    pub terminal_last_page: bool,
    /// Exact share delta for contribution, withdrawal, or queueing.
    pub share_delta: u64,
    /// Facility disambiguator for Initialize; zero otherwise.
    pub facility_nonce: u64,
    /// Bounded row start for Collect/Deliver.
    pub row_start: u16,
    /// Positive bounded row count for Collect/Deliver.
    pub row_count: u16,
}

impl DealerRuntimePayloadV1 {
    /// Decode the exact action-dependent length and canonical inactive fields.
    pub fn decode(
        action: DealerFacilityAction,
        input: &[u8],
    ) -> Result<Self, DealerRuntimeContractErrorV1> {
        let suffix_len = match action {
            DealerFacilityAction::Initialize => 8,
            DealerFacilityAction::CreateLpPage => 4,
            DealerFacilityAction::Contribute | DealerFacilityAction::WithdrawFunding => 12,
            DealerFacilityAction::Collect | DealerFacilityAction::Deliver => 4,
            DealerFacilityAction::QueueExit => 16,
            DealerFacilityAction::Claim | DealerFacilityAction::Retire => 8,
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
            retire_target: 0,
            terminal_last_page: false,
            share_delta: 0,
            facility_nonce: 0,
            row_start: 0,
            row_count: 0,
        };
        match action {
            DealerFacilityAction::Initialize => {
                value.facility_nonce = read_u64(input, 16);
                if value.expected_generation != 1 || value.expected_replay_ordinal != 0 {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            DealerFacilityAction::CreateLpPage => {
                value.page_ordinal = read_u32(input, 16);
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
                if value.row_count == 0 {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            DealerFacilityAction::QueueExit => {
                value.page_ordinal = read_u32(input, 16);
                value.entry_index = input[20];
                value.existing_ticket = decode_bool(input[21])?;
                value.external_liveness = decode_bool(input[22])?;
                if input[23] != 0 {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.share_delta = read_u64(input, 24);
                if value.share_delta == 0 {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            DealerFacilityAction::Claim => {
                value.page_ordinal = read_u32(input, 16);
                value.entry_index = input[20];
                if input[21..24].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
            }
            DealerFacilityAction::Retire => {
                value.retire_target = input[16];
                value.terminal_last_page = decode_bool(input[17])?;
                if !(DEALER_RETIRE_EXIT_TICKET_V1..=DEALER_RETIRE_STATE_ROOT_V1)
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
                    && value.page_ordinal >= clutch_dealer_runtime_contract::MAX_LP_PAGES)
                    || (!page_target && value.page_ordinal != 0)
                {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
                }
            }
            _ => {}
        }
        if usize::from(value.entry_index) >= clutch_dealer_runtime_contract::LP_ENTRIES_PER_PAGE
            || value.page_ordinal >= clutch_dealer_runtime_contract::MAX_LP_PAGES
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
    /// Signer identity with no data-owner requirement.
    Signer,
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
    /// Immutable Dealer quote schedule.
    LivenessSchedule,
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
    /// Immutable selected-candidate artifact.
    SelectedCandidate,
    /// Selected sealed General feed that owns candidate and settlement witness bytes.
    SelectedFeed,
    /// Selected owner-netted fee record.
    FeeRecord,
    /// Authenticated canonical EconomicDomainV2.
    EconomicDomain,
    /// Immutable quantized price-measure policy artifact.
    PriceMeasurePolicy,
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
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
];

const CREATE_FIRST: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::NewPage, DealerMetaOwnerV1::System, false, true),
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
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::TailPage, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::NewPage, DealerMetaOwnerV1::System, false, true),
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
];

const ACTIVATE: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::TailPage, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
];

const SPONSOR_HALT: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
];

const TIMED_CLOSE: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
];

const BIND_EPOCH: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::EpochBinding, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::GeneralEpoch, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::EconomicDomain, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::PriceMeasurePolicy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
];

const LAPSE_EPOCH: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::EpochBinding, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::NeutralSink, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
];

const SELECT_LEASE_BEGIN: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::EpochBinding, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::GeneralEpoch, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::EconomicDomain, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::SelectedCandidate, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::SelectedFeed, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::FeeRecord, DealerMetaOwnerV1::FeeRuntime, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::Lease, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::SettlementPot, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
];

const CANCEL_FUNDING: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::Clock, DealerMetaOwnerV1::ClockSysvar, false, false),
];

const REFUND_SPONSOR: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::SponsorRefundPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
];

const RETIRE_EXIT_TICKET: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ExitTicket, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::NeutralSink, DealerMetaOwnerV1::Signer, false, true),
];

const RETIRE_EMPTY_FIRST_PAGE: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::NeutralSink, DealerMetaOwnerV1::Signer, false, true),
];

const RETIRE_EMPTY_NEXT_PAGE: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::PreviousLpPage, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::NeutralSink, DealerMetaOwnerV1::Signer, false, true),
];

const RETIRE_EPOCH_BINDING: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::EpochBinding, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::NeutralSink, DealerMetaOwnerV1::Signer, false, true),
];

const RETIRE_TERMINAL_PAGE: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::TerminalAllocation, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::ClaimWork, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::NeutralSink, DealerMetaOwnerV1::Signer, false, true),
];

const RETIRE_LAST_TERMINAL_PAGE: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::TerminalAllocation, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::ClaimWork, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::NeutralSink, DealerMetaOwnerV1::Signer, false, true),
];

const RETIRE_FUNDED_DEPENDENCIES: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LivenessSource, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessCandidate, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessClearing, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessSettlement, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessResolution, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessRecovery, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::LivenessPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::NeutralSink, DealerMetaOwnerV1::Signer, false, true),
];

const RETIRE_POSITION_REPLAY: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FundedDependencies, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessRetirement, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::NeutralSink, DealerMetaOwnerV1::Signer, false, true),
];

const RETIRE_STATE_ROOT: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::RentPayer, DealerMetaOwnerV1::Signer, false, true),
    meta(DealerMetaRoleV1::NeutralSink, DealerMetaOwnerV1::Signer, false, true),
];

const CLAIM_TERMINAL: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::LpPosition, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::TerminalAllocation, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::ClaimWork, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
];

const QUEUE_NEW_CALLER: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ExitTicket, DealerMetaOwnerV1::System, false, true),
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
];

const QUEUE_NEW_EXTERNAL: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, true),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ExitTicket, DealerMetaOwnerV1::System, false, true),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::SystemProgram, DealerMetaOwnerV1::System, false, false),
];

const QUEUE_EXISTING_EXTERNAL: &[DealerMetaSpecV1] = &[
    meta(DealerMetaRoleV1::Actor, DealerMetaOwnerV1::Signer, true, false),
    meta(DealerMetaRoleV1::Policy, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::State, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::FacilityPosition, DealerMetaOwnerV1::PositionRuntime, false, false),
    meta(DealerMetaRoleV1::FacilityReplay, DealerMetaOwnerV1::PositionRuntime, false, true),
    meta(DealerMetaRoleV1::LpPage, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::ExitTicket, DealerMetaOwnerV1::SelfProgram, false, true),
    meta(DealerMetaRoleV1::LivenessSchedule, DealerMetaOwnerV1::SelfProgram, false, false),
    meta(DealerMetaRoleV1::LivenessCompartment, DealerMetaOwnerV1::LivenessRuntime, false, true),
    meta(DealerMetaRoleV1::LivenessReceipt, DealerMetaOwnerV1::LivenessRuntime, false, true),
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
        DealerFacilityAction::BindEpoch => Some(BIND_EPOCH),
        DealerFacilityAction::LapseEpoch => Some(LAPSE_EPOCH),
        DealerFacilityAction::SelectLeaseAndBegin => Some(SELECT_LEASE_BEGIN),
        DealerFacilityAction::SponsorHalt => Some(SPONSOR_HALT),
        DealerFacilityAction::TimedClose => Some(TIMED_CLOSE),
        DealerFacilityAction::QueueExit
            if !payload.external_liveness && !payload.existing_ticket =>
        {
            Some(QUEUE_NEW_CALLER)
        }
        DealerFacilityAction::QueueExit if !payload.external_liveness => {
            Some(QUEUE_EXISTING_CALLER)
        }
        DealerFacilityAction::QueueExit if !payload.existing_ticket => Some(QUEUE_NEW_EXTERNAL),
        DealerFacilityAction::QueueExit => Some(QUEUE_EXISTING_EXTERNAL),
        DealerFacilityAction::Claim => Some(CLAIM_TERMINAL),
        DealerFacilityAction::Retire
            if payload.retire_target == DEALER_RETIRE_EXIT_TICKET_V1 =>
        {
            Some(RETIRE_EXIT_TICKET)
        }
        DealerFacilityAction::Retire
            if payload.retire_target == DEALER_RETIRE_EMPTY_LP_PAGE_V1
                && payload.page_ordinal == 0 =>
        {
            Some(RETIRE_EMPTY_FIRST_PAGE)
        }
        DealerFacilityAction::Retire
            if payload.retire_target == DEALER_RETIRE_EMPTY_LP_PAGE_V1 =>
        {
            Some(RETIRE_EMPTY_NEXT_PAGE)
        }
        DealerFacilityAction::Retire
            if payload.retire_target == DEALER_RETIRE_EPOCH_BINDING_V1 =>
        {
            Some(RETIRE_EPOCH_BINDING)
        }
        DealerFacilityAction::Retire
            if payload.retire_target == DEALER_RETIRE_TERMINAL_PAGE_V1
                && payload.terminal_last_page =>
        {
            Some(RETIRE_LAST_TERMINAL_PAGE)
        }
        DealerFacilityAction::Retire
            if payload.retire_target == DEALER_RETIRE_TERMINAL_PAGE_V1 =>
        {
            Some(RETIRE_TERMINAL_PAGE)
        }
        DealerFacilityAction::Retire
            if payload.retire_target == DEALER_RETIRE_FUNDED_DEPENDENCIES_V1 =>
        {
            Some(RETIRE_FUNDED_DEPENDENCIES)
        }
        DealerFacilityAction::Retire
            if payload.retire_target == DEALER_RETIRE_POSITION_REPLAY_V1 =>
        {
            Some(RETIRE_POSITION_REPLAY)
        }
        DealerFacilityAction::Retire
            if payload.retire_target == DEALER_RETIRE_STATE_ROOT_V1 =>
        {
            Some(RETIRE_STATE_ROOT)
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
        if keys[index] == [0; 32] {
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

const fn recipient_alias_allowed_v1(left: DealerMetaRoleV1, right: DealerMetaRoleV1) -> bool {
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

/// Route every centrally allocated Dealer facility action to its reserved-disabled refusal.
///
/// This boundary is deliberately account-free and payload-free. Dispatch calls
/// it before account inspection or payload decoding. The exhaustive match makes
/// a future Dealer action a compile-time review point instead of letting it
/// inherit a partially implemented facility route.
#[inline(never)]
pub fn process_reserved_disabled(action: DealerFacilityAction) -> Result<(), Refusal> {
    match action {
        DealerFacilityAction::Initialize
        | DealerFacilityAction::CreateLpPage
        | DealerFacilityAction::Contribute
        | DealerFacilityAction::WithdrawFunding
        | DealerFacilityAction::Activate
        | DealerFacilityAction::CancelFunding
        | DealerFacilityAction::RefundCancelledSponsor
        | DealerFacilityAction::BindEpoch
        | DealerFacilityAction::LapseEpoch
        | DealerFacilityAction::SelectLeaseAndBegin
        | DealerFacilityAction::Collect
        | DealerFacilityAction::Deliver
        | DealerFacilityAction::FinalizeSettlement
        | DealerFacilityAction::AbortBeforeCollection
        | DealerFacilityAction::QueueExit
        | DealerFacilityAction::SponsorHalt
        | DealerFacilityAction::EnterUnwind
        | DealerFacilityAction::TimedClose
        | DealerFacilityAction::Resolve
        | DealerFacilityAction::Claim
        | DealerFacilityAction::Retire => Err(ClutchError::UnsupportedInstruction.into()),
    }
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

const _: () = assert!(DEALER_ACCOUNT_ENVELOPE_BYTES_V1 == 8);
const _: () = assert!(DEALER_RUNTIME_PAYLOAD_PREFIX_BYTES_V1 == 16);
