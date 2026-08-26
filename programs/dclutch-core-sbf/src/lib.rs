#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Isolated authenticated SBF adapter for the sparse canonical Market Core.
//!
//! The generated Market Core interpreter remains the semantic owner. This
//! crate owns only the Solana trust boundary: exact account frames, finalized
//! record/PDA joins, Registry/Loader-backed role reauthentication, prepaid
//! account creation, child CPI provenance, and commit-last persistence.

extern crate alloc;

use dclutch_market_core_codec::{
    Action, CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1, CORE_EFFECT_ENVELOPE_BYTES_V1,
    CapabilityFundingHeaderV1, CoreEffectEnvelopeV1, REQUEST_BYTES, Request,
};
use dclutch_release_set_contract::{
    CAPABILITY_EXECUTION_SELECTION_BYTES_V1, CapabilityExecutionSelectionV1,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V1, InitializeProtocolInfrastructureV1,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

mod capability;
mod fixed_role;
mod found;
mod frame;
mod infrastructure;
mod open_market;
mod product_runtime_v2;
mod records;
mod release;
mod resolution;

pub use frame::{FOUND_ACCOUNT_COUNT_V2, INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V1};

/// Exact instruction prefix shared by all Core actions.
pub const CORE_REQUEST_PREFIX_BYTES_V1: usize = REQUEST_BYTES;
/// Exact prefix for a generic capability action before child-owned bytes.
pub const CAPABILITY_PREFIX_BYTES_V1: usize = REQUEST_BYTES + CORE_EFFECT_ENVELOPE_BYTES_V1;
/// Exact generic capability semantic prefix before family-owned request bytes.
pub const CAPABILITY_ROLE_PREFIX_BYTES_V1: usize =
    CAPABILITY_EXECUTION_SELECTION_BYTES_V1 + CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1;

/// Stable refusal from the isolated Core SBF trust boundary.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreSbfError {
    /// Instruction bytes or action-specific inactive fields refused.
    Instruction = 0,
    /// Account count, order, privilege, executable flag, or alias refused.
    AccountFrame = 1,
    /// Finalized record owner, PDA, cursor absence, Rent, digest, or schema refused.
    FinalizedRecord = 2,
    /// Realm/Product/result-domain/Market identity linkage refused.
    Reference = 3,
    /// Registry cache, Loader-backed current deployment, or release-set join refused.
    Release = 4,
    /// Core Market PDA, owner, width, phase, or generation refused.
    Market = 5,
    /// RentCredit owner, bytes, PDA, or persisted beneficiary refused.
    RentCredit = 6,
    /// System, Rent, Clock, vacant account, or exact creation plan refused.
    Creation = 7,
    /// Capability manifest entry, FundingState, custody, deadline, or PDA refused.
    Funding = 8,
    /// Canonical release-pinned Core caller authority refused.
    CallerAuthority = 9,
    /// Selected child invocation or immediate return-data producer refused.
    ChildCpi = 10,
    /// Child acknowledgement or post-funding physical delta refused.
    ChildAck = 11,
    /// Generated semantic transition refused.
    Transition = 12,
    /// Commit-last Core state persistence postcheck refused.
    Commit = 13,
    /// Checked arithmetic or bounded conversion refused.
    Arithmetic = 14,
    /// Core bootstrap profile, artifact, Loader, or immutability authority refused.
    Infrastructure = 15,
}

impl From<CoreSbfError> for ProgramError {
    fn from(value: CoreSbfError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Execute one supported sparse Core transition.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() == INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V1 {
        InitializeProtocolInfrastructureV1::decode(instruction_data)
            .map_err(|_| CoreSbfError::Instruction)?;
        return infrastructure::process_initialize(program_id, accounts);
    }
    let request_bytes = instruction_data
        .get(..REQUEST_BYTES)
        .ok_or(CoreSbfError::Instruction)?;
    let request = Request::decode(request_bytes).map_err(|_| CoreSbfError::Instruction)?;
    match request.action {
        Action::Found if instruction_data.len() == REQUEST_BYTES => {
            found::process(program_id, accounts, request)
        }
        Action::OpenMarket
            if instruction_data.len() == open_market::OPEN_MARKET_INSTRUCTION_BYTES_V1 =>
        {
            let custody_bytes = instruction_data
                .get(REQUEST_BYTES..)
                .ok_or(CoreSbfError::Instruction)?;
            open_market::process(program_id, accounts, request, request_bytes, custody_bytes)
        }
        Action::ActivateCapability | Action::CloseCapability => {
            let envelope_end = CAPABILITY_PREFIX_BYTES_V1;
            let envelope_bytes = instruction_data
                .get(REQUEST_BYTES..envelope_end)
                .ok_or(CoreSbfError::Instruction)?;
            let role_request = instruction_data
                .get(envelope_end..)
                .ok_or(CoreSbfError::Instruction)?;
            let selection_bytes = role_request
                .get(..CAPABILITY_EXECUTION_SELECTION_BYTES_V1)
                .ok_or(CoreSbfError::Instruction)?;
            let header_end = CAPABILITY_ROLE_PREFIX_BYTES_V1;
            let header_bytes = role_request
                .get(CAPABILITY_EXECUTION_SELECTION_BYTES_V1..header_end)
                .ok_or(CoreSbfError::Instruction)?;
            let family_request = role_request
                .get(header_end..)
                .ok_or(CoreSbfError::Instruction)?;
            if family_request.is_empty() {
                return Err(CoreSbfError::Instruction.into());
            }
            let envelope = CoreEffectEnvelopeV1::decode(envelope_bytes)
                .map_err(|_| CoreSbfError::Instruction)?;
            let selection = CapabilityExecutionSelectionV1::decode(selection_bytes)
                .map_err(|_| CoreSbfError::Instruction)?;
            let funding_header = CapabilityFundingHeaderV1::decode(header_bytes)
                .map_err(|_| CoreSbfError::Instruction)?;
            capability::process(
                program_id,
                accounts,
                request,
                envelope,
                envelope_bytes,
                role_request,
                selection,
                funding_header,
            )
        }
        Action::VerifyReadiness | Action::AdmitTerminal | Action::Retire
            if instruction_data.len() == resolution::RESOLUTION_CORE_INSTRUCTION_BYTES_V1 =>
        {
            let envelope_end = CAPABILITY_PREFIX_BYTES_V1;
            let envelope_bytes = instruction_data
                .get(REQUEST_BYTES..envelope_end)
                .ok_or(CoreSbfError::Instruction)?;
            let role_request = instruction_data
                .get(envelope_end..)
                .ok_or(CoreSbfError::Instruction)?;
            let envelope = CoreEffectEnvelopeV1::decode(envelope_bytes)
                .map_err(|_| CoreSbfError::Instruction)?;
            resolution::process(
                program_id,
                accounts,
                request,
                envelope,
                envelope_bytes,
                role_request,
            )
        }
        _ => Err(CoreSbfError::Instruction.into()),
    }
}

#[cfg(test)]
mod tests;
