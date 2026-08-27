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

use dclutch_claims_svm::founding_v5::{
    CLAIMS_FOUNDING_RECEIPT_BYTES_V5, CLAIMS_FOUNDING_RECEIPT_MAGIC_V5,
};
use dclutch_claims_svm::market_closure_v1::CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1;
use dclutch_custody_contract::{
    CUSTODY_REQUEST_BYTES_V1, PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1,
    PROJECTED_CUSTODY_LOCK_RECEIPT_MAGIC_V1,
};
use dclutch_market_core_codec::{
    Action, CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1, CORE_EFFECT_ENVELOPE_BYTES_V1,
    CapabilityFundingHeaderV1, CoreEffectEnvelopeV1, GENERIC_FOUNDING_REQUEST_BYTES_V1,
    GENERIC_FOUNDING_REQUEST_MAGIC_V1, GenericFoundingRequestV1, PROJECT_FOUND_REQUEST_BYTES_V1,
    PROJECT_FOUND_REQUEST_MAGIC_V1, ProjectFoundRequestV1, REQUEST_BYTES,
    RETIREMENT_BUNDLE_BYTES_V1, Request, SERIES_CORE_REQUEST_BYTES_V1,
    SERIES_CORE_REQUEST_MAGIC_V1, SERIES_PERMIT_EXPIRY_REQUEST_BYTES_V1,
    SERIES_PERMIT_EXPIRY_REQUEST_MAGIC_V1, SeriesCoreRequestV1, SeriesPermitExpiryRequestV1,
};
use dclutch_release_set_contract::{
    CAPABILITY_EXECUTION_SELECTION_BYTES_V1, CapabilityExecutionSelectionV1,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V1, InitializeProtocolInfrastructureV1,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

mod begin_retiring;
mod capability;
mod execute_provider_v3;
mod fixed_role;
mod found;
mod frame;
mod generic_founding_v1;
mod infrastructure;
mod open_market;
mod product_runtime_v2;
mod records;
mod release;
mod resolution;
pub mod retire_v1;
mod series_consume;
mod series_open;
mod series_permit_expiry;

pub use begin_retiring::BEGIN_RETIRING_ACCOUNT_COUNT_V1;
pub use execute_provider_v3::{
    EXECUTE_PROVIDER_ACCOUNT_COUNT_V3, EXECUTE_PROVIDER_PREFIX_BYTES_V3,
};
pub use frame::{FOUND_ACCOUNT_COUNT_V2, INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V1};
pub use generic_founding_v1::{
    GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1, GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1,
    GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1,
};
pub use retire_v1::{RETIREMENT_ACCOUNT_COUNT_V1, RETIREMENT_INSTRUCTION_BYTES_V1};
pub use series_consume::{
    SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V1, SERIES_CONSUME_FOUND_SUFFIX_ACCOUNT_COUNT_V1,
};
pub use series_open::SERIES_OPEN_ACCOUNT_COUNT_V1;
pub use series_permit_expiry::SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1;

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
    Instruction = 0x3000,
    /// Account count, order, privilege, executable flag, or alias refused.
    AccountFrame = 0x3001,
    /// Finalized record owner, PDA, cursor absence, Rent, digest, or schema refused.
    FinalizedRecord = 0x3002,
    /// Realm/Product/result-domain/Market identity linkage refused.
    Reference = 0x3003,
    /// Registry cache, Loader-backed current deployment, or release-set join refused.
    Release = 0x3004,
    /// Core Market PDA, owner, width, phase, or generation refused.
    Market = 0x3005,
    /// RentCredit owner, bytes, PDA, or persisted beneficiary refused.
    RentCredit = 0x3006,
    /// System, Rent, Clock, vacant account, or exact creation plan refused.
    Creation = 0x3007,
    /// Capability manifest entry, FundingState, custody, deadline, or PDA refused.
    Funding = 0x3008,
    /// Canonical release-pinned Core caller authority refused.
    CallerAuthority = 0x3009,
    /// Selected child invocation or immediate return-data producer refused.
    ChildCpi = 0x300A,
    /// Child acknowledgement or post-funding physical delta refused.
    ChildAck = 0x300B,
    /// Generated semantic transition refused.
    Transition = 0x300C,
    /// Commit-last Core state persistence postcheck refused.
    Commit = 0x300D,
    /// Checked arithmetic or bounded conversion refused.
    Arithmetic = 0x300E,
    /// Core bootstrap profile, artifact, Loader, or immutability authority refused.
    Infrastructure = 0x300F,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    CoreSbfError::Instruction as u32 == dclutch_refusal_registry::CORE_REFUSAL_BASE,
    "CoreSbfError must start at its registered refusal band base"
);
const _: () = assert!(
    (CoreSbfError::Infrastructure as u32)
        < dclutch_refusal_registry::CORE_REFUSAL_BASE + dclutch_refusal_registry::BAND_SPAN,
    "CoreSbfError must not run past its registered refusal band"
);

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
    if instruction_data.len() >= SERIES_PERMIT_EXPIRY_REQUEST_BYTES_V1
        && instruction_data.get(..SERIES_PERMIT_EXPIRY_REQUEST_MAGIC_V1.len())
            == Some(SERIES_PERMIT_EXPIRY_REQUEST_MAGIC_V1.as_slice())
    {
        let request_bytes = instruction_data
            .get(..SERIES_PERMIT_EXPIRY_REQUEST_BYTES_V1)
            .ok_or(CoreSbfError::Instruction)?;
        let proof_bytes = instruction_data
            .get(SERIES_PERMIT_EXPIRY_REQUEST_BYTES_V1..)
            .ok_or(CoreSbfError::Instruction)?;
        let request = SeriesPermitExpiryRequestV1::decode(request_bytes)
            .map_err(|_| CoreSbfError::Instruction)?;
        return series_permit_expiry::process(program_id, accounts, request, proof_bytes);
    }
    if instruction_data.len() >= GENERIC_FOUNDING_REQUEST_BYTES_V1
        && instruction_data.get(..GENERIC_FOUNDING_REQUEST_MAGIC_V1.len())
            == Some(GENERIC_FOUNDING_REQUEST_MAGIC_V1.as_slice())
    {
        let request_bytes = instruction_data
            .get(..GENERIC_FOUNDING_REQUEST_BYTES_V1)
            .ok_or(CoreSbfError::Instruction)?;
        let dependency_bytes = instruction_data
            .get(GENERIC_FOUNDING_REQUEST_BYTES_V1..)
            .ok_or(CoreSbfError::Instruction)?;
        let request = GenericFoundingRequestV1::decode(request_bytes)
            .map_err(|_| CoreSbfError::Instruction)?;
        return generic_founding_v1::process(
            program_id,
            accounts,
            request,
            request_bytes,
            dependency_bytes,
        );
    }
    if instruction_data.len() >= SERIES_CORE_REQUEST_BYTES_V1
        && instruction_data.get(..SERIES_CORE_REQUEST_MAGIC_V1.len())
            == Some(SERIES_CORE_REQUEST_MAGIC_V1.as_slice())
    {
        let request_bytes = instruction_data
            .get(..SERIES_CORE_REQUEST_BYTES_V1)
            .ok_or(CoreSbfError::Instruction)?;
        let request =
            SeriesCoreRequestV1::decode(request_bytes).map_err(|_| CoreSbfError::Instruction)?;
        if let Some(dependency_start) = instruction_data
            .len()
            .checked_sub(CLAIMS_FOUNDING_RECEIPT_BYTES_V5)
            .filter(|start| *start >= SERIES_CORE_REQUEST_BYTES_V1)
        {
            let claims_receipt_bytes = instruction_data
                .get(dependency_start..)
                .ok_or(CoreSbfError::Instruction)?;
            if claims_receipt_bytes.get(..CLAIMS_FOUNDING_RECEIPT_MAGIC_V5.len())
                == Some(CLAIMS_FOUNDING_RECEIPT_MAGIC_V5.as_slice())
            {
                let proof_bytes = instruction_data
                    .get(SERIES_CORE_REQUEST_BYTES_V1..dependency_start)
                    .ok_or(CoreSbfError::Instruction)?;
                return series_open::process(
                    program_id,
                    accounts,
                    request,
                    request_bytes,
                    proof_bytes,
                    claims_receipt_bytes,
                );
            }
        }
        if let Some(dependency_start) = instruction_data
            .len()
            .checked_sub(PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1)
            .filter(|start| *start >= SERIES_CORE_REQUEST_BYTES_V1)
        {
            let lock_receipt_bytes = instruction_data
                .get(dependency_start..)
                .ok_or(CoreSbfError::Instruction)?;
            if lock_receipt_bytes.get(..PROJECTED_CUSTODY_LOCK_RECEIPT_MAGIC_V1.len())
                == Some(PROJECTED_CUSTODY_LOCK_RECEIPT_MAGIC_V1.as_slice())
            {
                let proof_bytes = instruction_data
                    .get(SERIES_CORE_REQUEST_BYTES_V1..dependency_start)
                    .ok_or(CoreSbfError::Instruction)?;
                return series_consume::process(
                    program_id,
                    accounts,
                    request,
                    request_bytes,
                    proof_bytes,
                    lock_receipt_bytes,
                );
            }
        }
        return Err(CoreSbfError::Instruction.into());
    }
    if instruction_data.len() == PROJECT_FOUND_REQUEST_BYTES_V1
        && instruction_data.get(..PROJECT_FOUND_REQUEST_MAGIC_V1.len())
            == Some(PROJECT_FOUND_REQUEST_MAGIC_V1.as_slice())
    {
        let projected = ProjectFoundRequestV1::decode(instruction_data)
            .map_err(|_| CoreSbfError::Instruction)?;
        let found_bytes = projected
            .found
            .encode()
            .map_err(|_| CoreSbfError::Instruction)?;
        return found::project(program_id, accounts, projected.found, &found_bytes);
    }
    let request_bytes = instruction_data
        .get(..REQUEST_BYTES)
        .ok_or(CoreSbfError::Instruction)?;
    let request = Request::decode(request_bytes).map_err(|_| CoreSbfError::Instruction)?;
    match request.action {
        Action::Found if instruction_data.len() == REQUEST_BYTES => {
            found::process(program_id, accounts, request)
        }
        Action::BeginRetiring if instruction_data.len() == REQUEST_BYTES => {
            begin_retiring::process(program_id, accounts, request)
        }
        Action::ExecuteProvider
            if instruction_data.len() > execute_provider_v3::EXECUTE_PROVIDER_PREFIX_BYTES_V3 =>
        {
            let provider_data = instruction_data
                .get(REQUEST_BYTES..)
                .ok_or(CoreSbfError::Instruction)?;
            execute_provider_v3::process(
                program_id,
                accounts,
                request,
                request_bytes,
                provider_data,
            )
        }
        Action::OpenMarket
            if instruction_data.len() == open_market::OPEN_MARKET_INSTRUCTION_BYTES_V1 =>
        {
            let custody_bytes = instruction_data
                .get(REQUEST_BYTES..)
                .ok_or(CoreSbfError::Instruction)?;
            open_market::process(program_id, accounts, request, request_bytes, custody_bytes)
        }
        Action::Retire if instruction_data.len() == retire_v1::RETIREMENT_INSTRUCTION_BYTES_V1 => {
            let bundle_start = REQUEST_BYTES;
            let claims_start = bundle_start + RETIREMENT_BUNDLE_BYTES_V1;
            let close_vault_start = claims_start + CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1;
            let close_replay_start = close_vault_start + CUSTODY_REQUEST_BYTES_V1;
            let bundle_bytes = instruction_data
                .get(bundle_start..claims_start)
                .ok_or(CoreSbfError::Instruction)?;
            let claims_request_bytes = instruction_data
                .get(claims_start..close_vault_start)
                .ok_or(CoreSbfError::Instruction)?;
            let close_vault_request_bytes = instruction_data
                .get(close_vault_start..close_replay_start)
                .ok_or(CoreSbfError::Instruction)?;
            let close_replay_request_bytes = instruction_data
                .get(close_replay_start..)
                .ok_or(CoreSbfError::Instruction)?;
            retire_v1::process(
                program_id,
                accounts,
                request,
                request_bytes,
                bundle_bytes,
                claims_request_bytes,
                close_vault_request_bytes,
                close_replay_request_bytes,
            )
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
