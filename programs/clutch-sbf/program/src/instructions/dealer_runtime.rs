// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dealer facility wire and account contract.
//!
//! This module freezes strict payload parsing, the global eight-byte Dealer
//! account envelope, and ordered semantic account roles for the complete
//! PositionV3 funding, custody, execution, unwind, and retirement lifecycle.
//! Successor dispatch consumes actions 1 through 24 through these contracts;
//! action 25 is additionally scoped by its terminal target.

use clutch_dealer_runtime_contract::{
    CoveredDealerTerminalV2, DealerSeriesObligationBindingV3, DealerStateV3, FixedCodec, Id,
};
pub use clutch_solana_layout::dealer_runtime::{
    meta_contract_v1, recipient_alias_allowed_v1, validate_meta_keys_distinct_v1,
    DealerMetaOwnerV1, DealerMetaRoleV1, DealerMetaSpecV1, DealerRuntimeContractErrorV1,
    DealerRuntimePayloadV1, DEALER_RETIRE_ACTIVE_FACILITY_CREDIT_V1,
    DEALER_RETIRE_EMPTY_LP_PAGE_V1, DEALER_RETIRE_EPOCH_BINDING_V1,
    DEALER_RETIRE_EXIT_TICKET_V1, DEALER_RETIRE_FUNDED_DEPENDENCIES_V1,
    DEALER_RETIRE_POSITION_REPLAY_V1, DEALER_RETIRE_STATE_ROOT_V1,
    DEALER_RETIRE_TERMINAL_PAGE_V1, DEALER_RETIRE_UNUSED_FUTURE_CREDIT_V1,
    DEALER_RUNTIME_PAYLOAD_PREFIX_BYTES_V1,
};
use clutch_solana_layout::registry::{
    DealerFacilityAction, DEALER_COVERED_SELECTION_ACCOUNT_BYTES,
    DEALER_COVERED_SELECTION_ACCOUNT_TAG, DEALER_COVERED_TERMINAL_ACCOUNT_VERSION,
    DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES_V3, DEALER_SERIES_OBLIGATION_ACCOUNT_TAG,
    DEALER_SERIES_OBLIGATION_ACCOUNT_VERSION_V3,
    DEALER_STATE_V3_ACCOUNT_BYTES, DEALER_STATE_V3_ACCOUNT_TAG, DEALER_STATE_V3_ACCOUNT_VERSION,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require};
use crate::error::{ClutchError, Outcome, Refusal};
use crate::instructions::artifact::CLOCK_SYSVAR_ID;
use crate::instructions::genesis::{RENT_SYSVAR_ID, SYSTEM_PROGRAM_ID};
use crate::instructions_sysvar::{INSTRUCTIONS_SYSVAR_ID, SYSVAR_OWNER_ID};
use crate::seeds;

/// Exact global Dealer account-envelope bytes.
pub const DEALER_ACCOUNT_ENVELOPE_BYTES_V1: usize = 8;
/// Strict common global account header used by centrally allocated Dealer tags.
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

/// Private exact account capability for the Product RootV3/LinkV3 obligation.
pub(crate) struct AuthenticatedDealerSeriesObligationV3 {
    account_id: Id,
    bump: u8,
    binding: DealerSeriesObligationBindingV3,
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

impl AuthenticatedDealerSeriesObligationV3 {
    /// Exact authenticated physical account.
    pub(crate) const fn account_id(&self) -> Id { self.account_id }

    /// Exact canonical PDA bump.
    pub(crate) const fn bump(&self) -> u8 { self.bump }

    /// Borrow the complete RootV3/LinkV3 body.
    pub(crate) const fn binding(&self) -> &DealerSeriesObligationBindingV3 {
        &self.binding
    }
}

/// Authenticate one exact current `0xaf/v3` Dealer facility obligation.
pub(crate) fn authenticate_dealer_series_obligation_v3(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
) -> Outcome<AuthenticatedDealerSeriesObligationV3> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable { ClutchError::NotWritable } else { ClutchError::UnexpectedWritable },
    )?;
    require(
        account.data_len() == DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES_V3,
        ClutchError::WrongDataLength,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (envelope, binding) =
        decode_dealer_account_body_v1::<DealerSeriesObligationBindingV3>(
            &data,
            DEALER_SERIES_OBLIGATION_ACCOUNT_TAG,
            DEALER_SERIES_OBLIGATION_ACCOUNT_VERSION_V3,
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
    Ok(AuthenticatedDealerSeriesObligationV3 {
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
        registry::DEALER_SERIES_OBLIGATION_ACCOUNT_TAG => {
            if version != registry::DEALER_SERIES_OBLIGATION_ACCOUNT_VERSION_V3 {
                return None;
            }
            return Some(DealerPersistedAccountContractV1 {
                tag,
                version,
                account_bytes: registry::DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES_V3,
                lifetime: DealerAccountLifetimeV1::CountedChild,
            });
        }
        registry::DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_TAG => (
            registry::DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_VERSION,
            registry::DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_BYTES,
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

/// Authenticate the exact effective Solana privileges and owner classes for
/// one current Dealer account contract before any handler reads semantic
/// account data. Permitted recipient aliases use Solana's privilege union;
/// every semantic/state account remains pairwise distinct.
pub(crate) fn authenticate_dealer_meta_contract_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    action: DealerFacilityAction,
    payload: DealerRuntimePayloadV1,
) -> Outcome<()> {
    let contract = meta_contract_v1(action, payload)
        .ok_or(Refusal::Adapter(ClutchError::UnsupportedInstruction))?;
    require(accounts.len() == contract.len(), ClutchError::AccountCount)?;

    let mut index = 0usize;
    while index < accounts.len() {
        require(
            accounts[index].key.to_bytes() != [0; 32]
                || contract[index].role == DealerMetaRoleV1::SystemProgram,
            ClutchError::MismatchedState,
        )?;
        let mut expected_signer = contract[index].signer;
        let mut expected_writable = contract[index].writable;
        let mut peer = 0usize;
        while peer < accounts.len() {
            if peer != index && accounts[peer].key == accounts[index].key {
                require(
                    recipient_alias_allowed_v1(contract[index].role, contract[peer].role),
                    ClutchError::AccountAlias,
                )?;
                expected_signer |= contract[peer].signer;
                expected_writable |= contract[peer].writable;
            }
            peer += 1;
        }
        require(
            accounts[index].is_signer == expected_signer
                && accounts[index].is_writable == expected_writable,
            ClutchError::MismatchedState,
        )?;
        match contract[index].owner {
            DealerMetaOwnerV1::SelfProgram
            | DealerMetaOwnerV1::PositionRuntime
            | DealerMetaOwnerV1::LivenessRuntime
            | DealerMetaOwnerV1::GeneralV2Runtime
            | DealerMetaOwnerV1::FeeRuntime => require(
                accounts[index].owner == program_id && !accounts[index].executable,
                ClutchError::WrongProgramOwner,
            )?,
            DealerMetaOwnerV1::System => {
                if contract[index].role == DealerMetaRoleV1::SystemProgram {
                    require(
                        accounts[index].key == &SYSTEM_PROGRAM_ID && accounts[index].executable,
                        ClutchError::WrongProgramOwner,
                    )?;
                } else {
                    require(
                        accounts[index].owner == &SYSTEM_PROGRAM_ID
                            && !accounts[index].executable,
                        ClutchError::WrongProgramOwner,
                    )?;
                }
            }
            DealerMetaOwnerV1::ClockSysvar => require(
                accounts[index].key == &CLOCK_SYSVAR_ID
                    && accounts[index].owner.to_bytes() == SYSVAR_OWNER_ID
                    && !accounts[index].executable,
                ClutchError::WrongProgramOwner,
            )?,
            DealerMetaOwnerV1::RentSysvar => require(
                accounts[index].key == &RENT_SYSVAR_ID
                    && accounts[index].owner.to_bytes() == SYSVAR_OWNER_ID
                    && !accounts[index].executable,
                ClutchError::WrongProgramOwner,
            )?,
            DealerMetaOwnerV1::InstructionsSysvar => require(
                accounts[index].key.to_bytes() == INSTRUCTIONS_SYSVAR_ID
                    && accounts[index].owner.to_bytes() == SYSVAR_OWNER_ID
                    && !accounts[index].executable,
                ClutchError::WrongProgramOwner,
            )?,
            DealerMetaOwnerV1::ExternalExecutable => require(
                accounts[index].executable,
                ClutchError::WrongProgramOwner,
            )?,
            DealerMetaOwnerV1::AnyReadOnly | DealerMetaOwnerV1::Signer => require(
                !accounts[index].executable,
                ClutchError::MismatchedState,
            )?,
        }
        index += 1;
    }
    Ok(())
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

const _: () = assert!(DEALER_ACCOUNT_ENVELOPE_BYTES_V1 == 8);
const _: () = assert!(DEALER_RUNTIME_PAYLOAD_PREFIX_BYTES_V1 == 16);
const _: () = assert!(
    clutch_solana_layout::dealer_runtime::DEALER_LP_ENTRIES_PER_PAGE_V1
        == clutch_dealer_runtime_contract::LP_ENTRIES_PER_PAGE
);
const _: () = assert!(
    clutch_solana_layout::dealer_runtime::DEALER_MAX_LP_PAGES_V1
        == clutch_dealer_runtime_contract::MAX_LP_PAGES
);
