//! Hostile Solana account metadata and current parser trust boundaries.

use clutch_retirement::{PositionPurposeV3, PositionV3PdaSeeds};

use crate::runtime_contract::{
    StructuredClaimDescriptorV2, WrapperMintProjectionV1, WrapperTokenProjectionV1,
};
use crate::{Error, Key, Result};

/// One semantic role in the current Structured custody projection.
///
/// Exact outer and base account counts/order are owned by
/// `current_account_contract`; this enum labels only the bounded subset passed
/// into the shared hostile-account projector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AccountRoleV1 {
    /// Structured-claim wrapper executable.
    WrapperProgram,
    /// Wrapper ProgramData selected by the exact release artifact.
    WrapperProgramData,
    /// Base Dragon's Clutch executable.
    BaseProgram,
    /// Base ProgramData selected by the exact release artifact.
    BaseProgramData,
    /// Token-2022 executable.
    Token2022Program,
    /// Token-2022 ProgramData selected by the exact release artifact.
    Token2022ProgramData,
    /// Current transaction actor.
    Actor,
    /// Canonical `0x88/2` descriptor PDA.
    Descriptor,
    /// Wrapper vault-owner PDA used only as a typed CPI signer.
    VaultAuthority,
    /// Immutable Realm selecting collateral semantics.
    Realm,
    /// Immutable Profile selected by the Realm.
    Profile,
    /// Exact sealed CollateralPolicy artifact.
    CollateralPolicy,
    /// Collateral token executable selected by the immutable Profile.
    CollateralTokenProgram,
    /// Immutable General MarketBinding PDA.
    MarketBinding,
    /// Stable General MarketRuntime selected by MarketBinding.
    MarketRuntime,
    /// Source full-width Position V3.
    SourcePositionV3,
    /// Source purpose-owned Replay V3.
    SourceReplayV3,
    /// Destination full-width Position V3.
    DestinationPositionV3,
    /// Destination purpose-owned Replay V3.
    DestinationReplayV3,
    /// Exact NativeClaimBasisV1 Product artifact.
    NativeClaimBasisArtifact,
    /// Exact MarketInstanceV2 preimage artifact.
    MarketInstanceArtifact,
    /// Full-width Hoard V2 aggregate owner.
    HoardV2,
    /// Full-width ClaimLedger V3 aggregate owner.
    ClaimLedgerV3,
}

/// Borrowed Solana account metadata and bytes.
///
/// An SBF composer constructs this directly from `AccountInfo` after its exact
/// route-specific alias and privilege checks. The role is never caller input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawAccountV1<'a> {
    /// Semantic role assigned by the route's exact account list.
    pub role: AccountRoleV1,
    /// Runtime account address.
    pub key: Key,
    /// Runtime account owner.
    pub owner: Key,
    /// Lamports observed before execution.
    pub lamports: u64,
    /// Borrowed exact account data.
    pub data: &'a [u8],
    /// Transaction signer bit.
    pub signer: bool,
    /// Transaction writable bit.
    pub writable: bool,
    /// Runtime executable bit.
    pub executable: bool,
}

/// Decode the canonical live descriptor-v2 body from a wrapper-owned account.
pub fn decode_owned_descriptor_v1(
    wrapper_program: Key,
    expected_address: Key,
    account: &RawAccountV1<'_>,
) -> Result<StructuredClaimDescriptorV2> {
    if account.role != AccountRoleV1::Descriptor
        || account.key != expected_address
        || account.owner != wrapper_program
        || account.executable
        || account.key == [0; 32]
    {
        return Err(Error::InvalidAccounts);
    }
    StructuredClaimDescriptorV2::decode(account.data).map_err(|_| Error::InvalidAccountData)
}

/// Base-owned Position/Replay PDA verifier used by current custody projection.
pub trait BasePositionPdaVerifierV1 {
    /// Verify the canonical full-width Position V3 PDA seed tuple.
    fn verify_position_v3(&self, program: Key, address: Key, seeds: PositionV3PdaSeeds) -> bool;

    /// Verify the exact stable purpose-owned Replay V3 PDA.
    fn verify_replay_v3(
        &self,
        program: Key,
        address: Key,
        position_account: Key,
        purpose: PositionPurposeV3,
        purpose_binding_id: Key,
        stored_bump: u8,
    ) -> bool;
}

/// Named hostile Token-2022 decoder boundary.
pub trait Token2022DecoderV1 {
    /// Decode one exact extension-free mint observation.
    fn decode_mint(
        &self,
        address: Key,
        data: &[u8],
    ) -> core::result::Result<WrapperMintProjectionV1, ()>;

    /// Decode one exact wrapper-token observation.
    fn decode_token(
        &self,
        address: Key,
        data: &[u8],
    ) -> core::result::Result<WrapperTokenProjectionV1, ()>;
}
