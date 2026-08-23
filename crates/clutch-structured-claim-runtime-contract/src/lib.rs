#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Allocation-free runtime contracts for transferable structured claims.
//!
//! The economic machine lives in `clutch-structured-claim`. This crate owns
//! the adapter-side facts that do not belong there: the exact persisted
//! descriptor image, deployment/basis reconstruction, and an atomic transfer
//! plan over two authenticated base Position projections. It deliberately has
//! no Solana SDK, CPI, hashing implementation, PDA implementation, account
//! memory, or Token-2022 parser. A small SBF adapter must authenticate those
//! boundaries and execute the returned plans exactly.

mod construction;
mod custody_wire;
mod descriptor;
mod market_root;
mod market_projection;
mod position_transfer;
mod recipe;
mod replay_v3;
mod runtime;
mod terminal;
mod wire;

pub use construction::{
    prepare_permanent_identity_funding_v1, PermanentIdentityFundingPlanV1,
    PermanentTargetProjectionV1, WRAPPER_MINT_ACCOUNT_BYTES,
};
pub use custody_wire::{
    decode_position_asset_transfer_payload_v1, PositionAssetTransferAuthorityKindV1,
    PositionAssetTransferPayloadV1, StructuredCustodyCallProjectionV1, GENERAL_V2_FAMILY_TAG,
    GENERAL_V2_FAMILY_VERSION, GENERAL_V2_TRANSFER_POSITION_ASSETS_ACTION,
    POSITION_ASSET_TRANSFER_PAYLOAD_BYTES, STRUCTURED_CUSTODY_CALL_PREIMAGE_BYTES,
    STRUCTURED_CUSTODY_CALL_V1_DOMAIN,
};
pub use descriptor::{
    decode_historical_descriptor_v1, reconstruct_descriptor_identity_v1, DescriptorBasisV1,
    DescriptorIdentityV1, DescriptorStateV1, StructuredClaimDescriptorV2,
    DESCRIPTOR_ACCOUNT_BYTES, DESCRIPTOR_ACCOUNT_TAG, DESCRIPTOR_ACCOUNT_VERSION,
    HISTORICAL_DESCRIPTOR_ACCOUNT_BYTES_V1, HISTORICAL_DESCRIPTOR_ACCOUNT_VERSION_V1,
};
pub use market_root::{
    structured_descriptor_admission_receipt_v1, structured_owner_release_id_v1,
    structured_owner_release_id_v2,
    StructuredMarketRootBindingV1, StructuredMarketRootV1, StructuredProductLineageV1,
    STRUCTURED_DESCRIPTOR_ADMISSION_DOMAIN_V1, STRUCTURED_DESCRIPTOR_TERMINAL_DOMAIN_V1,
    STRUCTURED_MARKET_TERMINAL_DOMAIN_V1, STRUCTURED_MARKET_TERMINAL_PREIMAGE_BYTES_V1,
    STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES, STRUCTURED_MARKET_ROOT_ACCOUNT_TAG,
    STRUCTURED_MARKET_ROOT_ACCOUNT_VERSION, STRUCTURED_MARKET_ROOT_BINDING_BYTES_V1,
    STRUCTURED_MARKET_ROOT_BINDING_DOMAIN_V1, STRUCTURED_OWNER_RELEASE_DOMAIN_V1,
};
pub use market_projection::{
    project_structured_market_v1, StructuredMarketProjectionStateV1,
    StructuredMarketProjectionV1, STRUCTURED_MARKET_PROJECTION_PREIMAGE_BYTES_V1,
    STRUCTURED_MARKET_PROJECTION_V1_DOMAIN,
};
pub use position_transfer::{
    prepare_atomic_position_asset_transfer_v1, AssetTransferPhasePolicyV1,
    AtomicPositionAssetTransferRequestV1, AtomicPositionAssetTransferResultV1,
    PositionProjectionV1,
};
pub use recipe::{
    authenticate_wrapper_recipe_membership_v1, build_wrapper_recipe_membership_v1, WrapperRecipeHashV1,
    WrapperRecipeMembershipV1, WrapperRecipeV1, MAX_WRAPPER_RECIPES_V1,
    MAX_WRAPPER_RECIPE_SLOTS_V1,
    WRAPPER_RECIPE_ID_DOMAIN_V1, WRAPPER_RECIPE_MEMBERSHIP_BYTES_V1,
    WRAPPER_RECIPE_MERKLE_DEPTH_BYTE_V1, WRAPPER_RECIPE_MERKLE_DEPTH_V1,
    WRAPPER_RECIPE_NODE_DOMAIN_V1, WRAPPER_RECIPE_PREIMAGE_BYTES_V1,
    WRAPPER_RECIPE_SET_ID_DOMAIN_V1, WRAPPER_RECIPE_SET_PREIMAGE_BYTES_V1,
};
pub use replay_v3::{
    StructuredClaimReplayDeltaV1, StructuredClaimReplayExtensionStateV1,
    StructuredClaimReplayExtensionV1, StructuredClaimReplayTransitionV1,
    STRUCTURED_CLAIM_REPLAY_DELTA_BYTES_V1, STRUCTURED_CLAIM_REPLAY_DELTA_DOMAIN_V1,
    STRUCTURED_CLAIM_REPLAY_EXTENSION_BYTES_V1, STRUCTURED_CLAIM_REPLAY_EXTENSION_SCHEMA_V1,
};
pub use runtime::{
    prepare_compact_donation_v1, prepare_redeem_terminal_v1, prepare_retire_descriptor_v1,
    prepare_unwrap_canonical_v1, prepare_unwrap_full_v1, prepare_wrap_canonical_v1,
    prepare_wrap_full_v1, AuthenticatedVaultRetirementV1, CanonicalUnwrapRequestV1,
    CanonicalWrapRequestV1, DescriptorRetirementPlanV1, DonationCompactionPlanV1,
    MarketChangingWrapperTransitionPlanV1, StructuredClaimRuntimeAddressesV1,
    TerminalRedemptionPlanV1, VaultMutationRequestV1, WrapperMintProjectionV1,
    WrapperTokenProjectionV1, WrapperTransitionPlanV1,
};
pub use terminal::{
    prepare_structured_descriptor_terminal_v1, StructuredDescriptorTerminalPlanV1,
    StructuredProductWrapperTerminalProjectionV1, StructuredRootCloseDispositionV1,
    STRUCTURED_DESCRIPTOR_ACTIVE_BODY_DOMAIN_V1, STRUCTURED_DESCRIPTOR_CLOSE_RECEIPT_DOMAIN_V1,
    STRUCTURED_DESCRIPTOR_RETIRED_BODY_DOMAIN_V1,
};
pub use wire::{
    decode_structured_claim_payload_v1, CreateDescriptorPayloadV1, StructuredClaimActionV1,
    StructuredClaimPayloadV1, VaultMutationPayloadV1, WrapperQuantityPayloadV1,
    CREATE_DESCRIPTOR_PAYLOAD_BYTES, STRUCTURED_CLAIM_FAMILY_TAG, STRUCTURED_CLAIM_FAMILY_VERSION,
    VAULT_MUTATION_PAYLOAD_BYTES, WRAPPER_QUANTITY_PAYLOAD_BYTES,
};

/// Maximum native outcome width shared with the structured-claim kernel.
pub const MAX_OUTCOMES: usize = clutch_structured_claim::MAX_OUTCOMES;
/// Atomic Realm-collateral or native-Egg quantity.
pub type Amount = u64;

/// Deterministic refusal from a descriptor or position-transfer contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    /// A byte image has the wrong exact width.
    InvalidLength,
    /// A persisted tag or version is not this contract.
    InvalidHeader,
    /// Reserved flags or padding are nonzero.
    NonCanonicalPadding,
    /// A key or digest is absent or aliases a distinct authority role.
    InvalidIdentity,
    /// The descriptor cannot reconstruct the authenticated native claim.
    InvalidClaim,
    /// The descriptor's persisted lifecycle value is unknown.
    InvalidState,
    /// A checked integer operation overflowed.
    ArithmeticOverflow,
    /// A checked integer operation underflowed.
    ArithmeticUnderflow,
    /// A quantity vector carries no asset.
    ZeroQuantity,
    /// A transfer names different markets or unexpected generations.
    DifferentPositionDomain,
    /// A Position projection is closed or structurally invalid.
    InvalidPosition,
    /// The selected phase policy rejects the authenticated market phase.
    InvalidPhase,
    /// Free cash or native Eggs cannot cover the requested debit.
    InsufficientFreeAssets,
    /// A Replay sequence cannot advance exactly once.
    ReplayExhausted,
    /// A prospective result violates exact conservation.
    InvariantViolation,
    /// The authoritative structured-claim economic machine refused the route.
    EconomicTransitionRefused,
    /// The family-local action is unallocated.
    UnknownAction,
    /// A construction target has hostile data, owner, executable, or address state.
    InvalidAccount,
    /// A required base retirement/custody capability is unavailable or mismatched.
    AuthorityUnavailable,
    /// A purpose-owned Replay V3 extension or exact transition join is invalid.
    InvalidReplayExtension,
}

/// Result alias for adapter contracts.
pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn put<const N: usize>(
    output: &mut [u8; N],
    cursor: &mut usize,
    bytes: &[u8],
) -> Result<()> {
    let end = cursor
        .checked_add(bytes.len())
        .ok_or(Error::ArithmeticOverflow)?;
    let destination = output.get_mut(*cursor..end).ok_or(Error::InvalidLength)?;
    destination.copy_from_slice(bytes);
    *cursor = end;
    Ok(())
}

pub(crate) fn take<'a>(input: &'a [u8], cursor: &mut usize, width: usize) -> Result<&'a [u8]> {
    let end = cursor.checked_add(width).ok_or(Error::ArithmeticOverflow)?;
    let value = input.get(*cursor..end).ok_or(Error::InvalidLength)?;
    *cursor = end;
    Ok(value)
}
