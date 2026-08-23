#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Disabled-by-default Solana trust-boundary adapter for structured claims.
//!
//! `clutch-structured-claim-runtime-contract` is the sole owner of the
//! descriptor bytes, family-local payloads, and economic state transitions.
//! This crate does not restate those DTOs. It owns only work that necessarily
//! lives at the SBF boundary: family admission, deployment hashing, PDA
//! authentication, hostile Solana account projection, exact CPI staging, and
//! post-CPI reconciliation.
//!
//! The default adapter keeps every family-local action disabled. The distinct
//! `live-current-wrapper` build admits exactly actions 1 through 5 for the
//! separately deployed wrapper ELF; terminal coordinates still refuse after
//! reading only the three-byte extension header.

mod accounts;
mod custody;
mod current_lifecycle;
mod envelope;
mod handler;
mod identity;
mod token2022_wire;

pub use accounts::{
    authenticate_general_base_position_v3_v1, authenticate_structured_claim_base_position_v3_v1,
    authenticate_token_2022_mint_v1, authenticate_token_2022_token_v1,
    decode_owned_descriptor_v1, AccountAccessV1, AccountFrameV1, AccountProgramsV1,
    AccountRoleV1, AuthenticatedBasePositionV3, AuthenticatedTokenMintV1,
    AuthenticatedTokenV1, BasePositionPdaVerifierV1, RawAccountV1, Token2022DecoderV1,
    MAX_ROUTE_ACCOUNTS,
};
pub use custody::{
    authenticate_structured_custody_call_v1, prepare_structured_custody_call_v1, AdapterSha256V1,
    prepare_current_structured_position_poststate_v1, AuthenticatedStructuredCustodyCallV1,
    BasePositionTransferCpiV1, CpiAccountMetaV1, PositionV3WriteV1, ReplayV3WriteV1,
    StructuredCustodyPdaVerifierV1, StructuredCustodyPoststateV1, StructuredCustodyScratchV1,
    BASE_POSITION_TRANSFER_CPI_BYTES, MAX_CUSTODY_REPLAY_V3_WRITE_BYTES,
    POSITION_V3_WRITE_BYTES, STRUCTURED_CUSTODY_ACCOUNT_COUNT,
    STRUCTURED_CUSTODY_CLAIM_LEDGER_BODY_DOMAIN_V1, STRUCTURED_CUSTODY_DESCRIPTOR_BODY_DOMAIN_V1,
    STRUCTURED_CUSTODY_HOARD_BODY_DOMAIN_V1, STRUCTURED_CUSTODY_MARKET_BINDING_BODY_DOMAIN_V1,
    STRUCTURED_CUSTODY_MARKET_RUNTIME_BODY_DOMAIN_V1,
};
pub use current_lifecycle::{
    prepare_current_compact_donation_v1, prepare_current_redeem_terminal_v1,
    prepare_current_unwrap_full_v1, prepare_current_wrap_full_v1,
    CurrentStructuredLiabilitiesV1, CurrentStructuredQuantityAccountsV1,
    CurrentStructuredTransitionPlanV1, CurrentStructuredVaultAccountsV1,
    CURRENT_STRUCTURED_POSITION_PROJECTION_DOMAIN_V1, CURRENT_STRUCTURED_TRANSITION_DOMAIN_V1,
};
pub use envelope::{
    admit_runtime_envelope_v1, decode_instruction_v1, StructuredClaimEnvelopeV1,
    ENABLED_STRUCTURED_CLAIM_ACTION_MASK, RESERVED_STRUCTURED_CLAIM_ACTION_MASK,
};
pub use handler::{
    authenticate_base_vault_creation_v1, authenticate_base_vault_retirement_v1,
    authenticate_structured_terminal_v1, BaseCapabilityVerifierV1,
    BaseVaultCreationEvidenceV1, BoundBaseVaultCreationV1, BoundBaseVaultRetirementV1,
    BoundStructuredTerminalV1, StructuredTerminalEvidenceV1, StructuredTerminalVerifierV1,
    Token2022CpiV1,
};
#[cfg(target_os = "solana")]
pub use identity::SolanaPdaVerifierV1;
pub use identity::{
    bind_descriptor_v1, canonical_native_claim_id_v1,
    canonical_series_scoped_wrapper_product_id_v2,
    BoundDescriptorV1, PdaVerifierV1, RuntimeDeploymentsV1, DESCRIPTOR_SEED, MINT_AUTHORITY_SEED,
    MINT_SEED, SERIES_SCOPED_WRAPPER_PRODUCT_DOMAIN_V2, VAULT_OWNER_SEED,
};
pub use token2022_wire::{
    decode_canonical_wrapper_mint_v1, decode_canonical_wrapper_token_v1,
    plan_token_2022_cpi_v1, wrapper_mint_parser_plan_v1, wrapper_token_parser_plan_v1,
    CanonicalToken2022DecoderV1, Token2022InstructionPlanV1, WrapperMintParserPlanV1,
    WrapperTokenParserPlanV1, TOKEN_2022_BASE_ACCOUNT_BYTES,
    TOKEN_2022_IMMUTABLE_OWNER_ACCOUNT_BYTES, TOKEN_2022_INSTRUCTION_DATA_CAPACITY,
};

/// The canonical semantic/runtime contract consumed by this adapter.
pub use clutch_structured_claim_runtime_contract as runtime_contract;

/// Canonical key or digest bytes.
pub type Key = [u8; 32];

/// Deterministic refusal at the structured-claim SBF boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    /// The extension family tag is not structured claims.
    WrongFamily,
    /// The extension family version is not version one.
    WrongFamilyVersion,
    /// The family-local action is not allocated by the canonical contract.
    UnknownAction,
    /// This allocated action has no runtime capability in the current ELF.
    CapabilityDisabled,
    /// The instruction envelope or action payload has a hostile exact length.
    InvalidInstruction,
    /// Account roles, ordering, aliases, owners, or privileges are invalid.
    InvalidAccounts,
    /// Hostile persisted account bytes failed their canonical owner codec.
    InvalidAccountData,
    /// A deployment or upgradeable-loader observation is not exact.
    InvalidDeployment,
    /// A native-claim or wrapper-product digest is not the canonical digest.
    DigestMismatch,
    /// A descriptor, mint, authority, Position, or Replay PDA is not canonical.
    PdaMismatch,
    /// The authenticated base Market/Terms/Hoard/kernel/supply join does not close.
    BaseClosureMismatch,
    /// The Token-2022 parser boundary refused the mint or holder account.
    Token2022Boundary,
    /// A base-program construction or retirement capability is absent or mismatched.
    BaseCapabilityUnavailable,
    /// Exact shortfall, state, or transition arithmetic failed.
    Arithmetic,
    /// An executed CPI receipt differs from the completely staged operation.
    ReceiptMismatch,
    /// Re-read authoritative accounts differ from the staged post-state.
    PostStateMismatch,
    /// The canonical runtime contract refused the request.
    Runtime(runtime_contract::Error),
    /// A Product artifact, MarketBinding, or Position V3 purpose join failed.
    ProductBoundary,
    /// The ephemeral structured-custody call did not reconstruct exactly.
    CustodyAuthorityMismatch,
}

impl From<runtime_contract::Error> for Error {
    fn from(value: runtime_contract::Error) -> Self {
        match value {
            runtime_contract::Error::InvalidLength => Self::InvalidInstruction,
            runtime_contract::Error::UnknownAction => Self::UnknownAction,
            other => Self::Runtime(other),
        }
    }
}

/// Result alias for the SBF boundary.
pub type Result<T> = core::result::Result<T, Error>;

pub(crate) const fn is_zero(key: &Key) -> bool {
    *key == [0; 32]
}
