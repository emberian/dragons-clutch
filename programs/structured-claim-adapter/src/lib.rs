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
//! Every adapter profile currently keeps every family-local action disabled.
//! The `live-current-wrapper` feature compiles the current implementation seam
//! but does not admit it until Product, deployment-release, and collateral
//! receipts form one exact account plane.

mod accounts;
mod custody;
mod current_account_contract;
mod current_lifecycle;
mod envelope;
mod identity;
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
    current_structured_action_contract_v1, CurrentStructuredActionContractV1,
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

/// Capability-manifest identity required by every wrapper loader release while
/// Structured runtime coordinates remain authority-join-disabled.
pub const STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_LABEL_V1: &str =
    "dragons-clutch/structured-claim-wrapper/non-production-authority-join-disabled/v1";
/// SHA-256 identity of [`STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_LABEL_V1`].
pub const STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1: Key = [
    0x26, 0xd5, 0x38, 0x9b, 0x08, 0x17, 0x9e, 0x2b, 0x8e, 0xc2, 0x1f, 0x67, 0x10, 0x53, 0x8f, 0x11,
    0xdb, 0xe1, 0xfa, 0xa1, 0xaf, 0xe7, 0xad, 0xb2, 0xda, 0x52, 0x0b, 0x03, 0xe3, 0xf8, 0xc0, 0x9c,
];
/// Reviewed central-program profile admitted by the Structured laboratory
/// release-set join. This is the exact `profile-full` manifest identity; the
/// separate Structured feature adds no executable tuple while the join is
/// disabled.
pub const STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1: Key = [
    0x05, 0x1c, 0x8a, 0xde, 0xc7, 0x94, 0x74, 0x2b, 0x76, 0x9f, 0x0f, 0x5a, 0x19, 0xfd, 0xeb, 0x3c,
    0x16, 0x4e, 0xef, 0xf6, 0x66, 0xcf, 0x43, 0x1e, 0x65, 0x4d, 0x3f, 0x9e, 0x4b, 0xc2, 0x93, 0xb0,
];
/// Reviewed Token-2022 interface-manifest identity required by the Structured
/// release-set authenticator. This is distinct from any wrapper/base profile.
pub const STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1: Key = [
    0x00, 0x09, 0xef, 0xd5, 0x4d, 0xd2, 0xf4, 0x43, 0xf1, 0x42, 0x1b, 0xad, 0x0e, 0x46, 0xb5, 0x60,
    0x2d, 0x2c, 0xf9, 0x82, 0xaf, 0x0a, 0x1a, 0xdd, 0xb1, 0xa2, 0x28, 0xd9, 0xba, 0xf8, 0x55, 0x5d,
];

/// Exact release/account wiring compiled for the current Structured source.
///
/// The action set is implemented but the three checked release identities all
/// describe disabled profiles, so `admitted_action_mask` remains zero. A
/// release cannot activate these handlers merely by flipping a feature bit: it
/// must rotate and reauthenticate the exact wrapper/base manifests and ELFs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredCurrentReleaseContractV1 {
    /// Actions with one closed current account contract.
    pub implemented_action_mask: u16,
    /// Actions admitted by the exact checked releases; currently empty.
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

/// Current release contract. Source completeness and release admission remain
/// deliberately disjoint.
pub const STRUCTURED_CURRENT_RELEASE_CONTRACT_V1: StructuredCurrentReleaseContractV1 =
    StructuredCurrentReleaseContractV1 {
        implemented_action_mask: IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1,
        admitted_action_mask: 0,
        account_contract_id: STRUCTURED_CURRENT_ACCOUNT_CONTRACT_ID_V1,
        wrapper_capability_manifest_id: STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1,
        base_capability_manifest_id: STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1,
        token_2022_capability_manifest_id: STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1,
    };

const _: () = assert!(STRUCTURED_CURRENT_RELEASE_CONTRACT_V1.admitted_action_mask == 0);
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
