// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dealer facility wire and account contract.
//!
//! This module freezes strict payload parsing, the global eight-byte Dealer
//! account envelope, and ordered semantic account roles for the PositionV3
//! funding/activation/unwind/retirement slice. The admitted non-production
//! adapter consumes only actions with complete account contracts; every other
//! action remains refused before reading accounts.

use clutch_dealer_runtime_contract::{
    CoveredDealerTerminalV2, DealerSeriesObligationBindingV1, DealerStateV3, FixedCodec, Id,
};
use clutch_solana_layout::registry::{
    DealerFacilityAction, DEALER_COVERED_SELECTION_ACCOUNT_BYTES,
    DEALER_COVERED_SELECTION_ACCOUNT_TAG, DEALER_COVERED_TERMINAL_ACCOUNT_VERSION,
    DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES, DEALER_SERIES_OBLIGATION_ACCOUNT_TAG,
    DEALER_SERIES_OBLIGATION_ACCOUNT_VERSION,
    DEALER_STATE_V3_ACCOUNT_BYTES, DEALER_STATE_V3_ACCOUNT_TAG, DEALER_STATE_V3_ACCOUNT_VERSION,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require};
use crate::error::{ClutchError, Outcome, Refusal};
use crate::seeds;

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

/// Program-local authority that one exact counted `0xae/v2` postwrite was
/// decoded from its program-owned PDA. Private fields prevent General from
/// substituting a caller-shaped terminal DTO.
pub(crate) struct AuthenticatedCoveredDealerTerminalPostwriteV2 {
    account_id: Id,
    owner_program_id: Id,
    bump: u8,
    terminal: CoveredDealerTerminalV2,
}

/// Private exact account capability for the facility-lifetime Product
/// Series-obligation binding.
pub(crate) struct AuthenticatedDealerSeriesObligationV1 {
    account_id: Id,
    bump: u8,
    binding: DealerSeriesObligationBindingV1,
}

/// Private exact account capability for Product-obligation-counting State V3.
pub(crate) struct AuthenticatedDealerStateV3 {
    account_id: Id,
    bump: u8,
    state: DealerStateV3,
}

impl AuthenticatedDealerStateV3 {
    /// Exact physical State account.
    pub(crate) const fn account_id(&self) -> Id {
        self.account_id
    }

    /// Canonical State PDA bump.
    pub(crate) const fn bump(&self) -> u8 {
        self.bump
    }

    /// Borrow the complete authoritative State body.
    pub(crate) const fn state(&self) -> &DealerStateV3 {
        &self.state
    }
}

/// Authenticate an exact `0x94/v2` Dealer State account and both independently
/// owned rent-principal compartments.
pub(crate) fn authenticate_dealer_state_v3(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
) -> Outcome<AuthenticatedDealerStateV3> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(
        account.data_len() == DEALER_STATE_V3_ACCOUNT_BYTES,
        ClutchError::WrongDataLength,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (envelope, state) = decode_dealer_account_body_v1::<DealerStateV3>(
        &data,
        DEALER_STATE_V3_ACCOUNT_TAG,
        DEALER_STATE_V3_ACCOUNT_VERSION,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(data);
    expect_pda(
        account.key,
        seeds::dealer_state_v2_pda(program_id, &state.base.facility_id.bytes()),
        Some(envelope.bump),
    )?;
    let root_floor = state
        .base
        .rent
        .refundable_live_principal
        .checked_add(state.base.rent.permanent_tombstone_principal)
        .and_then(|value| value.checked_add(state.base.rent.donation_floor))
        .ok_or(ClutchError::Arithmetic)?;
    let upgrade_floor = state
        .product_upgrade_rent
        .refundable_principal
        .checked_add(state.product_upgrade_rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        account.lamports()
            >= root_floor
                .checked_add(upgrade_floor)
                .ok_or(ClutchError::Arithmetic)?,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedDealerStateV3 {
        account_id: Id::from_bytes(account.key.to_bytes()),
        bump: envelope.bump,
        state,
    })
}

impl AuthenticatedDealerSeriesObligationV1 {
    /// Exact authenticated physical account.
    pub(crate) const fn account_id(&self) -> Id {
        self.account_id
    }

    /// Exact canonical PDA bump.
    pub(crate) const fn bump(&self) -> u8 {
        self.bump
    }

    /// Borrow the complete exact body; detached Product-coordinate DTOs are
    /// never minted from this authority.
    pub(crate) const fn binding(&self) -> &DealerSeriesObligationBindingV1 {
        &self.binding
    }
}

/// Authenticate one exact `0xaf/1` Dealer facility obligation account.
pub(crate) fn authenticate_dealer_series_obligation_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
) -> Outcome<AuthenticatedDealerSeriesObligationV1> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(
        account.data_len() == DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES,
        ClutchError::WrongDataLength,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (envelope, binding) =
        decode_dealer_account_body_v1::<DealerSeriesObligationBindingV1>(
            &data,
            DEALER_SERIES_OBLIGATION_ACCOUNT_TAG,
            DEALER_SERIES_OBLIGATION_ACCOUNT_VERSION,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(data);
    expect_pda(
        account.key,
        seeds::dealer_series_obligation_pda(program_id, &binding.key.facility_id.bytes()),
        Some(envelope.bump),
    )?;
    let floor = binding
        .rent
        .refundable_principal
        .checked_add(binding.rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        binding.key.binding_account_id.bytes() == account.key.to_bytes()
            && account.lamports() >= floor,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedDealerSeriesObligationV1 {
        account_id: Id::from_bytes(account.key.to_bytes()),
        bump: envelope.bump,
        binding,
    })
}

impl AuthenticatedCoveredDealerTerminalPostwriteV2 {
    /// Exact authenticated physical attachment account.
    pub(crate) const fn account_id(&self) -> Id {
        self.account_id
    }

    /// Program which owns the authenticated postwrite bytes.
    pub(crate) const fn owner_program_id(&self) -> Id {
        self.owner_program_id
    }

    /// Exact PDA bump authenticated from the envelope and seed tuple.
    pub(crate) const fn bump(&self) -> u8 {
        self.bump
    }

    /// Borrow the exact decoded terminal body; no detached field DTO exists.
    pub(crate) const fn terminal(&self) -> &CoveredDealerTerminalV2 {
        &self.terminal
    }
}

fn authenticate_covered_dealer_terminal_postwrite_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
) -> Outcome<AuthenticatedCoveredDealerTerminalPostwriteV2> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(
        account.data_len() == DEALER_COVERED_SELECTION_ACCOUNT_BYTES,
        ClutchError::WrongDataLength,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (envelope, terminal) = decode_dealer_account_body_v1::<CoveredDealerTerminalV2>(
        &data,
        DEALER_COVERED_SELECTION_ACCOUNT_TAG,
        DEALER_COVERED_TERMINAL_ACCOUNT_VERSION,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(data);
    expect_pda(
        account.key,
        seeds::dealer_covered_selection_pda(
            program_id,
            &terminal.general_epoch_account_id().bytes(),
            &terminal.settlement_candidate_id().bytes(),
        ),
        Some(envelope.bump),
    )?;
    let rent = terminal.rent();
    let floor = rent
        .refundable_principal
        .checked_add(rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        terminal.selection_account_id().bytes() == account.key.to_bytes()
            && terminal.stored_bump() == envelope.bump
            && account.lamports() >= floor,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedCoveredDealerTerminalPostwriteV2 {
        account_id: Id::from_bytes(account.key.to_bytes()),
        owner_program_id: Id::from_bytes(program_id.to_bytes()),
        bump: envelope.bump,
        terminal,
    })
}

/// Authenticate the read-only terminal authority used to enter General
/// retirement without writable privilege theater.
pub(crate) fn authenticate_covered_dealer_terminal_postwrite_readonly_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedCoveredDealerTerminalPostwriteV2> {
    authenticate_covered_dealer_terminal_postwrite_v2(program_id, account, false)
}

/// Authenticate the writable terminal authority used only by General's later
/// counted attachment close.
pub(crate) fn authenticate_covered_dealer_terminal_postwrite_writable_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedCoveredDealerTerminalPostwriteV2> {
    authenticate_covered_dealer_terminal_postwrite_v2(program_id, account, true)
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
    /// Deletable immutable evidence referenced by one accepted Replay intent.
    ReplayReferencedEvidence,
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
        registry::DEALER_STATE_V2_ACCOUNT_TAG => {
            let account_bytes = match version {
                registry::DEALER_STATE_V2_ACCOUNT_VERSION => {
                    registry::DEALER_STATE_V2_ACCOUNT_BYTES
                }
                registry::DEALER_STATE_V3_ACCOUNT_VERSION => {
                    registry::DEALER_STATE_V3_ACCOUNT_BYTES
                }
                _ => return None,
            };
            return Some(DealerPersistedAccountContractV1 {
                tag,
                version,
                account_bytes,
                lifetime: DealerAccountLifetimeV1::RootToTombstone,
            });
        }
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
        registry::DEALER_ACTION_RECEIPT_ACCOUNT_TAG => (
            registry::DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
            registry::DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
            DealerAccountLifetimeV1::ReplayReferencedEvidence,
        ),
        registry::DEALER_COVERED_SELECTION_ACCOUNT_TAG => {
            if version != registry::DEALER_COVERED_SELECTION_ACCOUNT_VERSION
                && version != registry::DEALER_COVERED_TERMINAL_ACCOUNT_VERSION
            {
                return None;
            }
            return Some(DealerPersistedAccountContractV1 {
                tag,
                version,
                account_bytes: registry::DEALER_COVERED_SELECTION_ACCOUNT_BYTES,
                lifetime: DealerAccountLifetimeV1::CountedChild,
            });
        }
        registry::DEALER_SERIES_OBLIGATION_ACCOUNT_TAG => (
            registry::DEALER_SERIES_OBLIGATION_ACCOUNT_VERSION,
            registry::DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES,
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
            | DealerFacilityAction::SelectLeaseAndBegin => 16,
            DealerFacilityAction::Collect | DealerFacilityAction::Deliver => 24,
            DealerFacilityAction::FinalizeSettlement
            | DealerFacilityAction::AbortBeforeCollection => 16,
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
            sponsor_capital_atoms: 0,
            liveness_call_ordinal: 0,
            keeper_payment_lamports: 0,
            row_start: 0,
            row_count: 0,
            book_page_count: 0,
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
            | DealerFacilityAction::RefundCancelledSponsor
            | DealerFacilityAction::BindEpoch
            | DealerFacilityAction::LapseEpoch => {
                value.liveness_call_ordinal = read_u32(input, 16);
                if input[20..24].iter().any(|byte| *byte != 0) {
                    return Err(DealerRuntimeContractErrorV1::NonCanonicalPadding);
                }
                value.keeper_payment_lamports = read_u64(input, 24);
                if value.liveness_call_ordinal == 0 {
                    return Err(DealerRuntimeContractErrorV1::InvalidField);
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
                if input[17..20].iter().any(|byte| *byte != 0) {
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
];

const SELECT_LEASE_BEGIN_FIXED_COUNT: usize = 40;
const SELECT_LEASE_BEGIN: &[DealerMetaSpecV1] = &[
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
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
];

const COLLECT_DELIVER_FIXED_COUNT: usize = 34;
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
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
    meta(DealerMetaRoleV1::OrderPage, DealerMetaOwnerV1::GeneralV2Runtime, false, false),
];

const FINALIZE_ABORT_ACCOUNT_COUNT: usize = 30;
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
        DealerFacilityAction::SelectLeaseAndBegin => Some(
            &SELECT_LEASE_BEGIN
                [..SELECT_LEASE_BEGIN_FIXED_COUNT + usize::from(payload.book_page_count)],
        ),
        DealerFacilityAction::Collect | DealerFacilityAction::Deliver => Some(
            &COLLECT_DELIVER
                [..COLLECT_DELIVER_FIXED_COUNT + usize::from(payload.book_page_count)],
        ),
        DealerFacilityAction::FinalizeSettlement => Some(FINALIZE_SETTLEMENT),
        DealerFacilityAction::AbortBeforeCollection => Some(ABORT_BEFORE_COLLECTION),
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
        DealerFacilityAction::SelectLeaseAndBegin
        | DealerFacilityAction::FinalizeSettlement
        | DealerFacilityAction::AbortBeforeCollection
        | DealerFacilityAction::QueueExit
        | DealerFacilityAction::SponsorHalt
        | DealerFacilityAction::EnterUnwind
        | DealerFacilityAction::TimedClose
        | DealerFacilityAction::Resolve
        | DealerFacilityAction::Claim
        | DealerFacilityAction::Retire => Err(ClutchError::UnsupportedInstruction.into()),
        // Enabled profiles route these actions to the executable handler before
        // reaching this function. Keep direct internal misuse fail-closed.
        DealerFacilityAction::Initialize
        | DealerFacilityAction::CreateLpPage
        | DealerFacilityAction::Contribute
        | DealerFacilityAction::WithdrawFunding
        | DealerFacilityAction::Activate
        | DealerFacilityAction::CancelFunding
        | DealerFacilityAction::RefundCancelledSponsor
        | DealerFacilityAction::BindEpoch
        | DealerFacilityAction::LapseEpoch
        | DealerFacilityAction::Collect
        | DealerFacilityAction::Deliver => {
            Err(ClutchError::UnsupportedInstruction.into())
        }
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
