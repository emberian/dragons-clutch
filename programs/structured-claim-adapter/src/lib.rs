#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Disabled-by-default Solana trust-boundary adapter for structured claims.
//!
//! `clutch-structured-claim-runtime-contract` is the sole owner of descriptor
//! bytes, family-local payloads, roots, replay extensions, and terminal plans.
//! This crate owns the current HoardV2/ClaimLedgerV3/PositionV3 lifecycle join
//! plus work that necessarily lives at the SBF boundary: family admission,
//! deployment hashing, PDA authentication, hostile account projection, exact
//! CPI staging, and post-CPI reconciliation.
//!
//! The default adapter remains disabled. The explicit
//! `profile-successor-chain-attached-dev` wrapper profile admits only actions
//! 1/3/5/6/7/8 through one exact wrapper/base/Token-2022 release join.

mod accounts;
mod custody;
mod current_account_contract;
mod current_lifecycle;
mod envelope;
mod identity;
mod release_manifest;
mod token2022_wire;

pub use accounts::{
    decode_owned_descriptor_v1, AccountRoleV1, BasePositionPdaVerifierV1, RawAccountV1,
    Token2022DecoderV1,
};
pub use custody::{
    prepare_current_structured_position_poststate_v1,
    prepare_current_structured_vault_poststate_v1, AdapterSha256V1, PositionV3WriteV1,
    ReplayV3WriteV1, StructuredCustodyPoststateV1, StructuredVaultPoststateV1,
    MAX_CUSTODY_REPLAY_V3_WRITE_BYTES, POSITION_V3_WRITE_BYTES,
    STRUCTURED_CUSTODY_ACCOUNT_COUNT, STRUCTURED_CUSTODY_DESCRIPTOR_BODY_DOMAIN_V1,
};
pub use current_account_contract::{
    current_structured_account_meta_v1, current_structured_action_contract_v1,
    current_structured_alias_allowed_v1, CurrentStructuredAccountMetaV1,
    CurrentStructuredActionContractV1,
    CurrentStructuredTokenEffectV1, CURRENT_STRUCTURED_ACTION_CONTRACTS_V1,
    IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1, STRUCTURED_COMPACTION_ACCOUNT_COUNT_V1,
    STRUCTURED_CREATE_ACCOUNT_COUNT_V1, STRUCTURED_CURRENT_ACCOUNT_CONTRACT_ID_V1,
    STRUCTURED_CURRENT_ACCOUNT_CONTRACT_LABEL_V1,
    STRUCTURED_DESCRIPTOR_RETIREMENT_ACCOUNT_COUNT_V1, STRUCTURED_FULL_VECTOR_ACCOUNT_COUNT_V1,
    STRUCTURED_TERMINAL_REDEMPTION_ACCOUNT_COUNT_V1,
};
pub use current_lifecycle::{
    finalize_current_compaction_disposition_v1, prepare_current_compact_donation_v1,
    prepare_current_redeem_terminal_v1,
    prepare_current_retire_descriptor_v1, prepare_current_unwrap_full_v1,
    prepare_current_wrap_full_v1,
    CurrentStructuredLiabilitiesV1, CurrentStructuredQuantityAccountsV1,
    CurrentStructuredTransitionPlanV1, CurrentStructuredVaultAccountsV1,
    CURRENT_STRUCTURED_COMPACTION_DISPOSITION_DOMAIN_V1,
    CURRENT_STRUCTURED_POSITION_PROJECTION_DOMAIN_V1, CURRENT_STRUCTURED_TRANSITION_DOMAIN_V1,
    CURRENT_STRUCTURED_TRANSITION_DOMAIN_V2,
};
pub use envelope::{
    admit_runtime_envelope_v1, decode_instruction_v1, StructuredClaimEnvelopeV1,
    ENABLED_STRUCTURED_CLAIM_ACTION_MASK, RESERVED_STRUCTURED_CLAIM_ACTION_MASK,
};
#[cfg(target_os = "solana")]
pub use identity::SolanaPdaVerifierV1;
pub use identity::{
    bind_descriptor_v1, canonical_native_claim_id_v1,
    canonical_series_scoped_wrapper_product_id_v2,
    BoundDescriptorV1, PdaVerifierV1, RuntimeDeploymentsV1, DESCRIPTOR_SEED, MINT_AUTHORITY_SEED,
    MINT_SEED, SERIES_SCOPED_WRAPPER_PRODUCT_DOMAIN_V2, VAULT_OWNER_SEED,
};
pub use release_manifest::{
    joined_structured_action_mask_v1, StructuredCheckedCapabilityManifestV1,
    StructuredReleaseRoleV1, STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1,
    STRUCTURED_BASE_CAPABILITY_MANIFEST_LABEL_V1, STRUCTURED_CHECKED_CAPABILITY_MANIFESTS_V1,
    STRUCTURED_JOINED_RELEASE_ACTION_MASK_V1, STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1,
    STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_LABEL_V1,
    STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1,
    STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_LABEL_V1,
};
pub use token2022_wire::{
    decode_canonical_wrapper_mint_v1, decode_canonical_wrapper_token_v1,
    decode_retired_canonical_wrapper_mint_v1, plan_token_2022_cpi_v1,
    wrapper_mint_parser_plan_v1, wrapper_token_parser_plan_v1, CanonicalToken2022DecoderV1,
    CpiAccountMetaV1, Token2022CpiV1, Token2022InstructionPlanV1, WrapperMintParserPlanV1,
    WrapperTokenParserPlanV1, TOKEN_2022_BASE_ACCOUNT_BYTES,
    TOKEN_2022_IMMUTABLE_OWNER_ACCOUNT_BYTES, TOKEN_2022_INSTRUCTION_DATA_CAPACITY,
};

/// The canonical semantic/runtime contract consumed by this adapter.
pub use clutch_structured_claim_runtime_contract as runtime_contract;

/// Canonical key or digest bytes.
pub type Key = [u8; 32];

/// Exact release/account wiring compiled for the current Structured source.
///
/// The admitted mask is the intersection of three disjoint checked semantic
/// manifests. Each live instruction additionally authenticates the exact
/// `RegistryProgramReleaseV2` Program/ProgramData/hash/slot/locus body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredCurrentReleaseContractV1 {
    /// Actions with one closed current account contract.
    pub implemented_action_mask: u16,
    /// Actions admitted by all exact checked releases.
    pub admitted_action_mask: u16,
    /// Exact current source/account/token-effect contract identity.
    pub account_contract_id: Key,
    /// Exact wrapper capability manifest identity.
    pub wrapper_capability_manifest_id: Key,
    /// Exact base capability manifest identity.
    pub base_capability_manifest_id: Key,
    /// Exact Token-2022 interface manifest identity.
    pub token_2022_capability_manifest_id: Key,
}

/// Current release contract for the unified successor development profile.
pub const STRUCTURED_CURRENT_RELEASE_CONTRACT_V1: StructuredCurrentReleaseContractV1 =
    StructuredCurrentReleaseContractV1 {
        implemented_action_mask: IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1,
        admitted_action_mask: STRUCTURED_JOINED_RELEASE_ACTION_MASK_V1,
        account_contract_id: STRUCTURED_CURRENT_ACCOUNT_CONTRACT_ID_V1,
        wrapper_capability_manifest_id: STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1,
        base_capability_manifest_id: STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1,
        token_2022_capability_manifest_id: STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1,
    };

const _: () = assert!(
    STRUCTURED_CURRENT_RELEASE_CONTRACT_V1.admitted_action_mask
        == IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1
);
const _: () = assert!(
    STRUCTURED_CURRENT_RELEASE_CONTRACT_V1.admitted_action_mask
        & !STRUCTURED_CURRENT_RELEASE_CONTRACT_V1.implemented_action_mask
        == 0
);

/// Deterministic refusal at the structured-claim SBF boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    /// The extension family tag is not structured claims.
    WrongFamily,
    /// The extension family version is not version one.
    WrongFamilyVersion,
    /// The family-local action has no current runtime-contract variant.
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
    /// The current descriptor/Hoard/ClaimLedger/Position authority join does not close.
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
    /// A current successor authority or exact postimage join did not reconstruct.
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
